//! 外部工具管理共用的状态探测、进度通知与原子替换逻辑。

#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
use crate::utils;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::{ToolProgress, ToolStatus};

/// HTTP 下载超时时间（30 分钟，用于大文件下载）
pub(super) const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(1800);

/// 进度事件节流：进度每前进 1% 或每 200ms 才推送一次。
const PROGRESS_MIN_STEP: f64 = 1.0;
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(200);

/// 下载进度节流器。
///
/// 子进程/网络的 chunk 粒度远小于 UI 需要的刷新粒度，逐 chunk 推送会让
/// 大文件下载产生上千次跨 IPC 调用。这里做「百分比步进 + 时间窗」双重节流，
/// 保证进度既不平滑丢失、也不高频刷屏；流程末尾的 complete 事件由调用方单独发送。
struct ProgressThrottle {
    last_percent: f64,
    last_emit: Instant,
}

impl ProgressThrottle {
    fn new() -> Self {
        Self {
            // 初始值取负无穷，确保首个进度一定被推送
            last_percent: f64::NEG_INFINITY,
            last_emit: Instant::now(),
        }
    }

    fn should_emit(&mut self, percent: f64) -> bool {
        if percent - self.last_percent >= PROGRESS_MIN_STEP
            || self.last_emit.elapsed() >= PROGRESS_MIN_INTERVAL
        {
            self.last_percent = percent;
            self.last_emit = Instant::now();
            return true;
        }
        false
    }
}

/// 为可执行文件生成同目录临时路径，确保最终替换不会跨文件系统。
pub(super) fn executable_temp_path(target: &Path, suffix: &str) -> Result<PathBuf, String> {
    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("err_invalid_executable_path")?;
    let extension = target.extension().and_then(|s| s.to_str());
    let name = match extension {
        Some(ext) => format!("{}.{}.{}", stem, suffix, ext),
        None => format!("{}.{}", stem, suffix),
    };
    Ok(target.with_file_name(name))
}

/// 用已验证的临时文件替换正式文件；Windows 上保留可恢复备份，避免先删后换。
pub(super) fn replace_executable(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let backup_path = executable_temp_path(target_path, "backup")?;
        let _ = std::fs::remove_file(&backup_path);
        if target_path.exists() {
            std::fs::rename(target_path, &backup_path)
                .map_err(|e| format!("err_backup_executable:{}", e))?;
        }
        if let Err(e) = std::fs::rename(temp_path, target_path) {
            if backup_path.exists() {
                let _ = std::fs::rename(&backup_path, target_path);
            }
            return Err(format!("err_replace_executable:{}", e));
        }
        let _ = std::fs::remove_file(backup_path);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(temp_path, target_path).map_err(|e| format!("err_replace_executable:{}", e))
    }
}

pub(super) fn emit_tool_progress(
    app: &AppHandle,
    tool: &str,
    operation: &str,
    stage: &str,
    percent: Option<f64>,
) {
    let _ = app.emit(
        "tool-operation-progress",
        ToolProgress {
            tool: tool.to_string(),
            operation: operation.to_string(),
            stage: stage.to_string(),
            percent,
        },
    );
}

/// 流式下载到目标文件，带分段进度推送与全错误路径的临时文件清理。
///
/// `start_percent` / `span` 把单次下载映射到总进度的某个区间，用于一个工具
/// 需要下载多个文件时（如 FFmpeg 的 ffmpeg + ffprobe 各占 50%）。
///
/// 失败时保证删除 `target`，调用方无需再自行清理下载产物。
pub(super) async fn download_to_file_with_progress(
    app: &AppHandle,
    tool: &str,
    operation: &str,
    url: &str,
    target: &Path,
    start_percent: f64,
    span: f64,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("err_download_failed:{}", e))?
        .error_for_status()
        .map_err(|e| format!("err_download_http_status:{}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 先清掉可能残留的只读旧文件，避免 File::create 因权限失败
    let _ = tokio::fs::remove_file(target).await;
    let mut file = tokio::fs::File::create(target)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;

    let mut throttle = ProgressThrottle::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(target).await;
                return Err(format!("err_download_error:{}", e));
            }
        };
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(target).await;
            return Err(format!("err_write_error:{}", e));
        }

        downloaded += chunk.len() as u64;
        let fraction = if total_size > 0 {
            downloaded as f64 / total_size as f64
        } else {
            0.0
        };
        let percent = start_percent + fraction * span;
        if throttle.should_emit(percent) {
            emit_tool_progress(app, tool, operation, "downloading", Some(percent));
        }
    }

    if let Err(e) = tokio::io::AsyncWriteExt::shutdown(&mut file).await {
        drop(file);
        let _ = tokio::fs::remove_file(target).await;
        return Err(format!("err_flush_file:{}", e));
    }
    drop(file);

    // 服务端给了 Content-Length 才校验完整性；分块传输下无法判断
    if total_size > 0 && downloaded != total_size {
        let _ = tokio::fs::remove_file(target).await;
        return Err(format!(
            "err_download_incomplete:expected={},actual={}",
            total_size, downloaded
        ));
    }
    Ok(())
}

/// 运行 `--version` 验证刚下载的可执行文件确实可启动。
///
/// PyInstaller 打包的 yt-dlp 与静态构建的 FFmpeg 都只有真正启动后才能确认
/// 内嵌归档/依赖完整，因此替换正式文件前必须实测一次。
/// 失败时返回原始错误详情，由调用方拼接自己的错误码并清理临时文件。
pub(super) async fn verify_executable(path: &Path, version_arg: &str) -> Result<(), String> {
    probe_version(path, version_arg, "").await.map(|_| ())
}

async fn probe_version(path: &Path, version_arg: &str, tool: &str) -> Result<String, String> {
    let mut command = tokio::process::Command::new(path);
    command
        .arg(version_arg)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .map_err(|_| "version probe timed out after 10 seconds".to_string())?
        .map_err(|e| e.to_string())?;
    version_probe_result(output, tool)
}

fn version_probe_result(output: std::process::Output, tool: &str) -> Result<String, String> {
    let raw = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let text = String::from_utf8_lossy(raw).trim().to_string();
    // QuickJS intentionally exits with 1 after printing its help/version header.
    // Accept that one case only when it really identifies a QuickJS runtime.
    let quickjs_help = tool == "quickjs"
        && output.status.code() == Some(1)
        && quickjs_version(text.lines().next().unwrap_or_default()).is_some();
    if (output.status.success() || quickjs_help) && !text.is_empty() {
        Ok(text)
    } else if !text.is_empty() {
        Err(text)
    } else {
        Err(format!("exit_code:{}", output.status.code().unwrap_or(-1)))
    }
}

pub(super) async fn build_tool_status(
    tool: &str,
    path: PathBuf,
    managed_path: PathBuf,
    version_arg: &str,
) -> Result<ToolStatus, String> {
    let configured_source = utils::get_tool_source(tool)?;
    let has_cli_override = utils::get_cli_tool_path(tool).is_some();
    let source = if has_cli_override {
        "custom"
    } else {
        configured_source.as_str()
    };
    let is_managed = !has_cli_override
        && configured_source == utils::ToolSource::Managed
        && path == managed_path;
    let can_update = can_update_tool(tool, configured_source, has_cli_override);
    Ok(probe_tool_status(tool, path, version_arg, source, is_managed, can_update).await)
}

fn can_update_tool(tool: &str, source: utils::ToolSource, has_cli_override: bool) -> bool {
    !has_cli_override
        && match source {
            utils::ToolSource::Managed => true,
            utils::ToolSource::System => tool != "ffmpeg",
            utils::ToolSource::Local => false,
        }
}

pub(super) async fn probe_tool_status(
    tool: &str,
    path: PathBuf,
    version_arg: &str,
    source: &str,
    is_managed: bool,
    can_update: bool,
) -> ToolStatus {
    let installed = path.is_file();
    let probe = if installed {
        probe_version(&path, version_arg, tool).await
    } else {
        Err(format!("err_tool_not_installed:{}", tool))
    };
    let runnable = probe.is_ok();
    let error = if installed {
        probe.as_ref().err().cloned()
    } else {
        None
    };
    ToolStatus {
        installed,
        runnable,
        version: probe
            .map(|text| parse_version(tool, text.lines().next().unwrap_or_default()))
            .unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        source: source.to_string(),
        is_managed,
        can_update: runnable && can_update,
        error,
    }
}

/// 从工具 `--version` 的首行输出里提取纯版本号
fn parse_version(tool: &str, first_line: &str) -> String {
    if tool == "ffmpeg" {
        first_line
            .strip_prefix("ffmpeg version ")
            .unwrap_or(first_line)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else if tool == "deno" {
        first_line
            .strip_prefix("deno ")
            .unwrap_or(first_line)
            .to_string()
    } else if tool == "quickjs" {
        quickjs_version(first_line)
            .unwrap_or(first_line)
            .to_string()
    } else {
        first_line.to_string()
    }
}

fn quickjs_version(first_line: &str) -> Option<&str> {
    let version = first_line
        .strip_prefix("QuickJS version ")
        .or_else(|| first_line.strip_prefix("QuickJS-ng version "))?
        .split_whitespace()
        .next()?;
    (version.bytes().next()?.is_ascii_digit()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b'-'))
    .then_some(version)
}

#[cfg(test)]
mod tests {
    use super::{
        can_update_tool, parse_version, probe_tool_status, version_probe_result, ProgressThrottle,
    };

    #[test]
    fn local_and_cli_ytdlp_cannot_be_updated_by_the_app() {
        use crate::utils::ToolSource;
        assert!(!can_update_tool("yt-dlp", ToolSource::Local, false));
        assert!(!can_update_tool("yt-dlp", ToolSource::Managed, true));
        assert!(!can_update_tool("yt-dlp", ToolSource::System, true));
        assert!(can_update_tool("yt-dlp", ToolSource::Managed, false));
        assert!(can_update_tool("yt-dlp", ToolSource::System, false));
    }

    fn probe_output(code: i32, text: &str) -> std::process::Output {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;

        std::process::Output {
            #[cfg(windows)]
            status: std::process::ExitStatus::from_raw(code as u32),
            #[cfg(unix)]
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: text.as_bytes().to_vec(),
        }
    }

    #[test]
    fn runtime_status_keeps_installation_distinct_from_execution_failure() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let executable = std::env::current_exe().unwrap();
        let broken = runtime.block_on(probe_tool_status(
            "node",
            executable.clone(),
            "--unknown-runtime-probe-option",
            "system",
            false,
            true,
        ));
        assert!(broken.installed);
        assert!(!broken.runnable);
        assert!(broken.version.is_empty());
        assert!(!broken.can_update);
        assert!(broken.error.is_some());

        let working = runtime.block_on(probe_tool_status(
            "node", executable, "--help", "system", false, false,
        ));
        assert!(working.installed);
        assert!(working.runnable);
        assert_eq!(working.source, "system");
        assert!(!working.can_update);
        assert!(!working.is_managed);
        assert!(working.error.is_none());
    }

    #[test]
    fn missing_runtime_has_no_version_or_update_action() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("yt-dlp-missing-runtime-{}", std::process::id()));
        let status = runtime.block_on(probe_tool_status(
            "bun",
            path,
            "--version",
            "system",
            false,
            true,
        ));
        assert!(!status.installed);
        assert!(!status.runnable);
        assert!(status.version.is_empty());
        assert!(!status.can_update);
        assert!(status.error.is_none());
    }

    #[test]
    fn quickjs_version_label_does_not_include_help_text() {
        assert_eq!(
            parse_version("quickjs", "QuickJS version 2025-09-13"),
            "2025-09-13"
        );
        assert_eq!(
            parse_version("quickjs", "QuickJS-ng version 0.11.0"),
            "0.11.0"
        );
    }

    #[test]
    fn quickjs_help_exit_code_is_accepted_only_with_a_valid_version_header() {
        assert!(version_probe_result(
            probe_output(1, "QuickJS version 2025-09-13\nusage: qjs [options]"),
            "quickjs"
        )
        .is_ok());
        assert!(version_probe_result(
            probe_output(1, "QuickJS-ng version 0.11.0\nusage: qjs [options]"),
            "quickjs"
        )
        .is_ok());
        assert!(
            version_probe_result(probe_output(1, "failed to load library"), "quickjs").is_err()
        );
        assert!(
            version_probe_result(probe_output(1, "QuickJS version unknown"), "quickjs").is_err()
        );
        assert!(
            version_probe_result(probe_output(2, "QuickJS version 2025-09-13"), "quickjs").is_err()
        );
        assert!(
            version_probe_result(probe_output(1, "QuickJS version 2025-09-13"), "node").is_err()
        );
    }

    #[test]
    fn throttle_pushes_first_progress_immediately() {
        let mut throttle = ProgressThrottle::new();
        // 首个进度必须推送，否则进度条会一直停在 0
        assert!(throttle.should_emit(0.0));
    }

    #[test]
    fn throttle_swallows_sub_percent_steps() {
        let mut throttle = ProgressThrottle::new();
        assert!(throttle.should_emit(10.0));
        // 时间窗内不足 1% 的步进被吞掉，避免逐 chunk 刷屏
        assert!(!throttle.should_emit(10.4));
        assert!(!throttle.should_emit(10.9));
        // 跨过 1% 才重新推送
        assert!(throttle.should_emit(11.0));
    }

    #[test]
    fn throttle_treats_stalled_progress_as_noop() {
        let mut throttle = ProgressThrottle::new();
        assert!(throttle.should_emit(50.0));
        // 进度原地不动时不得反复推送同一个值
        assert!(!throttle.should_emit(50.0));
    }
}
