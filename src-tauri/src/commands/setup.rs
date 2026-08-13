//! 平台信息、yt-dlp 和 Deno 安装管理

use crate::utils;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

use super::common;
use super::{ToolProgress, ToolStatus};

/// HTTP 下载超时时间（30 分钟，用于大文件下载）
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(1800);

/// 为可执行文件生成同目录临时路径，确保最终替换不会跨文件系统。
fn executable_temp_path(target: &Path, suffix: &str) -> Result<PathBuf, String> {
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
fn replace_executable(temp_path: &Path, target_path: &Path) -> Result<(), String> {
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

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

// ========== 平台信息 ==========

/// 获取当前运行平台
#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

/// 设置三个外部工具各自的来源；选择是严格的，不再隐式回退到另一来源。
#[tauri::command]
pub fn set_tool_sources(ytdlp: String, deno: String, ffmpeg: String) -> Result<(), String> {
    utils::set_tool_sources(&ytdlp, &deno, &ffmpeg)
}

/// 设置 YouTube 提取器参数（PO Token / visitor_data），用于绕过 YouTube 403 / 限流
#[tauri::command]
pub fn set_youtube_extractor_args(po_token: String, visitor_data: String) -> Result<(), String> {
    utils::set_youtube_extractor_args(&po_token, &visitor_data)
}

// ========== 工具状态与进度 ==========

fn emit_tool_progress(
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

async fn build_tool_status(
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
    let version = if tool == "ffmpeg" {
        first_line
            .strip_prefix("ffmpeg version ")
            .unwrap_or(&first_line)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else if tool == "deno" {
        first_line
            .strip_prefix("deno ")
            .unwrap_or(&first_line)
            .to_string()
    } else {
        first_line
    };

    Ok(ToolStatus {
        installed: output.status.success(),
        version,
        path: path.to_string_lossy().to_string(),
        source: source.to_string(),
        is_managed: path == managed_path,
        can_update: !has_cli_override
            && (configured_source == utils::ToolSource::Managed || tool != "ffmpeg"),
    })
}

// ========== yt-dlp 管理 ==========

/// 获取 yt-dlp 安装状态和版本
#[tauri::command]
pub async fn get_ytdlp_status(app: AppHandle) -> Result<ToolStatus, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    let managed_path = utils::get_managed_ytdlp_path(&app)?;
    build_tool_status("yt-dlp", ytdlp_path, managed_path, "--version").await
}

async fn download_ytdlp_impl(app: AppHandle, operation: &str) -> Result<(), String> {
    emit_tool_progress(&app, "yt-dlp", operation, "downloading", Some(0.0));
    let ytdlp_path = utils::get_managed_ytdlp_path(&app)?;
    let temp_path = executable_temp_path(&ytdlp_path, "download")?;
    let url = utils::get_ytdlp_download_url();

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

    let _ = tokio::fs::remove_file(&temp_path).await;
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(format!("err_download_error:{}", e));
            }
        };
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!("err_write_error:{}", e));
        }

        downloaded += chunk.len() as u64;
        let percent = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        let _ = app.emit(
            "ytdlp-download-progress",
            serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total_size,
            }),
        );
        emit_tool_progress(&app, "yt-dlp", operation, "downloading", Some(percent));
    }

    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("err_flush_file:{}", e))?;
    drop(file);

    if total_size > 0 && downloaded != total_size {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!(
            "err_download_incomplete:expected={},actual={}",
            total_size, downloaded
        ));
    }

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    // PyInstaller 可执行文件只有真正启动后才能确认内嵌归档完整。
    let validation = tokio::process::Command::new(&temp_path)
        .arg("--version")
        .output()
        .await;
    match validation {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {}
        Ok(output) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!(
                "err_validate_ytdlp:{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!("err_validate_ytdlp:{}", e));
        }
    }

    emit_tool_progress(&app, "yt-dlp", operation, "installing", None);
    replace_executable(&temp_path, &ytdlp_path)?;
    emit_tool_progress(&app, "yt-dlp", operation, "complete", Some(100.0));

    Ok(())
}

/// 下载 yt-dlp 可执行文件
#[tauri::command]
pub async fn download_ytdlp(app: AppHandle) -> Result<(), String> {
    download_ytdlp_impl(app, "install").await
}

/// 更新当前选择的 yt-dlp；应用版本原子替换，系统版本使用其内置更新器。
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let source = utils::get_tool_source("yt-dlp")?;
    if source == utils::ToolSource::Managed {
        download_ytdlp_impl(app, "update").await?;
        return Ok("Updated managed yt-dlp".to_string());
    }

    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

    emit_tool_progress(&app, "yt-dlp", "update", "updating", None);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.arg("-U")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("err_start_update:{}", e))?;

    let stdout = child.stdout.take().ok_or("err_capture_stdout")?;
    let stderr = child.stderr.take().ok_or("err_capture_stderr")?;

    let app_clone = app.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_clone.emit("ytdlp-update-log", &line);
            output.push_str(&line);
            output.push('\n');
        }
        output
    });

    let app_clone2 = app.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_clone2.emit("ytdlp-update-log", &line);
            output.push_str(&line);
            output.push('\n');
        }
        output
    });

    let stdout_out = stdout_handle.await.unwrap_or_default();
    let stderr_out = stderr_handle.await.unwrap_or_default();

    let status = child
        .wait()
        .await
        .map_err(|e| format!("err_process:{}", e))?;

    if status.success() {
        emit_tool_progress(&app, "yt-dlp", "update", "complete", Some(100.0));
        Ok(format!("{}\n{}", stdout_out, stderr_out).trim().to_string())
    } else {
        Err(format!("err_update_failed:{}", stderr_out.trim()))
    }
}

// ========== yt-dlp 插件管理 ==========

/// 检查插件是否已安装（通过相对路径判断文件是否存在）
#[tauri::command]
pub async fn check_plugin_installed(app: AppHandle, file_path: String) -> Result<bool, String> {
    let plugin_dir = utils::get_plugin_dir(&app)?;
    Ok(plugin_dir.join(&file_path).exists())
}

/// 卸载 yt-dlp 插件（删除指定文件）
#[tauri::command]
pub async fn uninstall_plugin(app: AppHandle, file_path: String) -> Result<(), String> {
    let plugin_dir = utils::get_plugin_dir(&app)?;
    // 路径安全验证：确保目标文件在插件目录内，防止路径遍历攻击
    let target = common::validate_path_within(&plugin_dir, &file_path)?;
    if target.exists() {
        tokio::fs::remove_file(&target)
            .await
            .map_err(|e| format!("err_delete_file:{}", e))?;
    }
    Ok(())
}

/// 下载并安装 yt-dlp 插件（zip 格式，自动解压到插件目录）
#[tauri::command]
pub async fn install_plugin(app: AppHandle, url: String) -> Result<(), String> {
    let plugin_dir = utils::get_plugin_dir(&app)?;

    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("err_download_failed:{}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("err_download_error:{}", e))?;

    // 解压 zip，保留 yt_dlp_plugins/ 内的目录结构
    let plugin_dir_clone = plugin_dir.clone();
    tokio::task::spawn_blocking(move || {
        let cursor = std::io::Cursor::new(bytes);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| format!("err_read_zip:{}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("err_read_zip_entry:{}", e))?;
            let name = entry.name().to_string();

            // 只提取 yt_dlp_plugins/ 下的 .py 文件，保留子目录结构
            if let Some(rel) = name.strip_prefix("yt_dlp_plugins/") {
                if !rel.is_empty() && !entry.is_dir() {
                    let out_path = plugin_dir_clone.join("yt_dlp_plugins").join(rel);
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("err_create_dir:{}", e))?;
                    }
                    let mut outfile = std::fs::File::create(&out_path)
                        .map_err(|e| format!("err_create_file:{}", e))?;
                    std::io::copy(&mut entry, &mut outfile)
                        .map_err(|e| format!("err_write_error:{}", e))?;
                }
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("err_task:{}", e))??;

    Ok(())
}

// ========== Deno 管理 ==========

/// 获取 Deno 安装状态和版本
#[tauri::command]
pub async fn get_deno_status(app: AppHandle) -> Result<ToolStatus, String> {
    let deno_path = utils::get_deno_path(&app)?;
    let managed_path = utils::get_managed_deno_path(&app)?;
    build_tool_status("deno", deno_path, managed_path, "--version").await
}

async fn download_deno_impl(app: AppHandle, operation: &str) -> Result<(), String> {
    emit_tool_progress(&app, "deno", operation, "downloading", Some(0.0));
    let deno_path = utils::get_managed_deno_path(&app)?;
    let temp_path = executable_temp_path(&deno_path, "download")?;
    let url = utils::get_deno_download_url();

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

    // 下载 zip 到临时文件
    let deno_file_name = deno_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("err_invalid_executable_path")?;
    let zip_path = deno_path.with_file_name(format!("{}.download.zip", deno_file_name));
    let _ = tokio::fs::remove_file(&zip_path).await;
    let _ = tokio::fs::remove_file(&temp_path).await;
    let mut file = tokio::fs::File::create(&zip_path)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(&zip_path).await;
                return Err(format!("err_download_error:{}", e));
            }
        };
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&zip_path).await;
            return Err(format!("err_write_error:{}", e));
        }

        downloaded += chunk.len() as u64;
        let percent = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };
        let _ = app.emit(
            "deno-download-progress",
            serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total_size,
            }),
        );
        emit_tool_progress(&app, "deno", operation, "downloading", Some(percent));
    }

    // 确保文件写入完成
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("err_flush_file:{}", e))?;
    drop(file);

    if total_size > 0 && downloaded != total_size {
        let _ = tokio::fs::remove_file(&zip_path).await;
        return Err(format!(
            "err_download_incomplete:expected={},actual={}",
            total_size, downloaded
        ));
    }

    emit_tool_progress(&app, "deno", operation, "installing", None);

    // 先解压到临时文件，验证成功后才替换现有 Deno。
    let zip_path_clone = zip_path.clone();
    let temp_path_clone = temp_path.clone();
    let deno_bin_name = if cfg!(target_os = "windows") {
        "deno.exe"
    } else {
        "deno"
    };

    tokio::task::spawn_blocking(move || {
        let file =
            std::fs::File::open(&zip_path_clone).map_err(|e| format!("err_open_zip:{}", e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("err_read_zip:{}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("err_read_zip_entry:{}", e))?;
            let name = entry.name().to_lowercase();
            if name == deno_bin_name || name.ends_with(&format!("/{}", deno_bin_name)) {
                let mut outfile = std::fs::File::create(&temp_path_clone)
                    .map_err(|e| format!("err_create_file:{}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("err_extract_deno:{}", e))?;
                return Ok(());
            }
        }
        Err(format!("err_not_found_in_zip:{}", deno_bin_name))
    })
    .await
    .map_err(|e| format!("err_task:{}", e))?
    .map_err(|e| {
        let _ = std::fs::remove_file(&zip_path);
        e
    })?;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    let validation = tokio::process::Command::new(&temp_path)
        .arg("--version")
        .output()
        .await;
    match validation {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {}
        Ok(output) => {
            let _ = tokio::fs::remove_file(&zip_path).await;
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!(
                "err_validate_deno:{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&zip_path).await;
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!("err_validate_deno:{}", e));
        }
    }

    replace_executable(&temp_path, &deno_path)?;
    let _ = tokio::fs::remove_file(&zip_path).await;
    emit_tool_progress(&app, "deno", operation, "complete", Some(100.0));

    Ok(())
}

/// 下载 Deno 可执行文件（从 zip 解压）
#[tauri::command]
pub async fn download_deno(app: AppHandle) -> Result<(), String> {
    download_deno_impl(app, "install").await
}

#[tauri::command]
pub async fn update_deno(app: AppHandle) -> Result<String, String> {
    if utils::get_tool_source("deno")? == utils::ToolSource::Managed {
        download_deno_impl(app, "update").await?;
        return Ok("Updated managed Deno".to_string());
    }

    let deno_path = utils::get_deno_path(&app)?;
    if !deno_path.exists() {
        return Err("err_deno_not_installed".to_string());
    }
    emit_tool_progress(&app, "deno", "update", "updating", None);
    let mut cmd = tokio::process::Command::new(&deno_path);
    cmd.arg("upgrade");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_update_deno:{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "err_update_deno:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    emit_tool_progress(&app, "deno", "update", "complete", Some(100.0));
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ========== FFmpeg 管理 ==========

#[tauri::command]
pub async fn get_ffmpeg_status(app: AppHandle) -> Result<ToolStatus, String> {
    let ffmpeg_path = utils::get_ffmpeg_path(&app)?;
    let ffprobe_path = utils::get_ffprobe_path(&app)?;
    let managed_path = utils::get_managed_ffmpeg_path(&app)?;
    let mut status = build_tool_status("ffmpeg", ffmpeg_path, managed_path, "-version").await?;
    status.installed = status.installed && ffprobe_path.exists();
    Ok(status)
}

async fn download_file_with_progress(
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
    let mut downloaded = 0u64;
    let mut file = tokio::fs::File::create(target)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("err_download_error:{}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("err_write_error:{}", e))?;
        downloaded += chunk.len() as u64;
        let fraction = if total_size > 0 {
            downloaded as f64 / total_size as f64
        } else {
            0.0
        };
        emit_tool_progress(
            app,
            tool,
            operation,
            "downloading",
            Some(start_percent + fraction * span),
        );
    }
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("err_flush_file:{}", e))?;
    if total_size > 0 && downloaded != total_size {
        return Err(format!(
            "err_download_incomplete:expected={},actual={}",
            total_size, downloaded
        ));
    }
    Ok(())
}

async fn download_ffmpeg_impl(app: AppHandle, operation: &str) -> Result<(), String> {
    let dir = utils::get_managed_ffmpeg_dir(&app)?;
    let urls = utils::get_ffmpeg_download_urls();
    let ffmpeg_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let ffprobe_name = if cfg!(target_os = "windows") {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    let targets = [dir.join(ffmpeg_name), dir.join(ffprobe_name)];
    let temps = [
        executable_temp_path(&targets[0], "download")?,
        executable_temp_path(&targets[1], "download")?,
    ];

    for (index, ((_, url), temp)) in urls.iter().zip(temps.iter()).enumerate() {
        let _ = tokio::fs::remove_file(&temp).await;
        if let Err(e) = download_file_with_progress(
            &app,
            "ffmpeg",
            operation,
            url,
            temp,
            index as f64 * 50.0,
            50.0,
        )
        .await
        {
            for pending in &temps {
                let _ = tokio::fs::remove_file(pending).await;
            }
            return Err(e);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temp, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("err_set_permissions:{}", e))?;
        }
        let validation = tokio::process::Command::new(temp)
            .arg("-version")
            .output()
            .await
            .map_err(|e| format!("err_validate_ffmpeg:{}", e))?;
        if !validation.status.success() {
            for pending in &temps {
                let _ = tokio::fs::remove_file(pending).await;
            }
            return Err(format!(
                "err_validate_ffmpeg:{}",
                String::from_utf8_lossy(&validation.stderr).trim()
            ));
        }
    }

    emit_tool_progress(&app, "ffmpeg", operation, "installing", None);
    for (temp, target) in temps.iter().zip(targets.iter()) {
        replace_executable(temp, target)?;
    }
    emit_tool_progress(&app, "ffmpeg", operation, "complete", Some(100.0));
    Ok(())
}

#[tauri::command]
pub async fn download_ffmpeg(app: AppHandle) -> Result<(), String> {
    download_ffmpeg_impl(app, "install").await
}

#[tauri::command]
pub async fn update_ffmpeg(app: AppHandle) -> Result<(), String> {
    if utils::get_tool_source("ffmpeg")? != utils::ToolSource::Managed {
        return Err("err_system_ffmpeg_update_managed_externally".to_string());
    }
    download_ffmpeg_impl(app, "update").await
}
