//! 外部工具管理共用的状态探测、进度通知与原子替换逻辑。

use crate::utils;
#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
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
    let output = tokio::process::Command::new(path)
        .arg(version_arg)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() && !output.stdout.is_empty() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        Err(format!("exit_code:{}", output.status.code().unwrap_or(-1)))
    } else {
        Err(detail)
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
    let installed = path.exists();
    if !installed {
        return Ok(ToolStatus {
            installed: false,
            runnable: false,
            version: String::new(),
            path: path.to_string_lossy().to_string(),
            source: source.to_string(),
            is_managed: !has_cli_override && configured_source == utils::ToolSource::Managed,
            can_update: false,
        });
    }

    let mut cmd = tokio::process::Command::new(&path);
    cmd.arg(version_arg)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_run_tool:{}:{}", tool, e))?;
    let raw = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let first_line = String::from_utf8_lossy(raw)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    // 文件存在不等于能跑：`--version` 失败说明这个二进制不可用（缺动态库、
    // 架构不符、权限不足等）。此时不要把 installed 拉回 false，否则界面会
    // 显示「未安装」并引导用户反复下载一个其实就在磁盘上的文件。
    let runnable = output.status.success();
    let version = if runnable {
        parse_version(tool, &first_line)
    } else {
        // 跑不起来时 stdout/stderr 通常是报错文本，塞进「版本」字段会误导用户
        String::new()
    };

    Ok(ToolStatus {
        installed: true,
        runnable,
        version,
        path: path.to_string_lossy().to_string(),
        source: source.to_string(),
        is_managed: path == managed_path,
        can_update: !has_cli_override
            && (configured_source == utils::ToolSource::Managed || tool != "ffmpeg"),
    })
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
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressThrottle;

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
