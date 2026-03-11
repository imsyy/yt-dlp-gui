//! 平台信息、yt-dlp 和 Deno 安装管理

use crate::utils;
use futures_util::StreamExt;
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

use super::common;
use super::{DenoStatus, YtdlpStatus};

/// HTTP 下载超时时间（30 分钟，用于大文件下载）
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(1800);

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

// ========== yt-dlp 管理 ==========

/// 获取 yt-dlp 安装状态和版本
#[tauri::command]
pub async fn get_ytdlp_status(app: AppHandle) -> Result<YtdlpStatus, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;

    if !ytdlp_path.exists() {
        return Ok(YtdlpStatus {
            installed: false,
            version: String::new(),
            path: ytdlp_path.to_string_lossy().to_string(),
        });
    }

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.arg("--version")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_run_ytdlp:{}", e))?;

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(YtdlpStatus {
        installed: true,
        version,
        path: ytdlp_path.to_string_lossy().to_string(),
    })
}

/// 下载 yt-dlp 可执行文件
#[tauri::command]
pub async fn download_ytdlp(app: AppHandle) -> Result<(), String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    let url = utils::get_ytdlp_download_url();

    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("err_download_failed:{}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&ytdlp_path)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("err_download_error:{}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("err_write_error:{}", e))?;

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
    }

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    Ok(())
}

/// 更新 yt-dlp 到最新版本
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

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
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| format!("err_read_zip:{}", e))?;

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
pub async fn get_deno_status(app: AppHandle) -> Result<DenoStatus, String> {
    let deno_path = utils::get_deno_path(&app)?;

    if !deno_path.exists() {
        return Ok(DenoStatus {
            installed: false,
            version: String::new(),
            path: deno_path.to_string_lossy().to_string(),
        });
    }

    let mut cmd = tokio::process::Command::new(&deno_path);
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().await;

    match output {
        Ok(out) if out.status.success() => {
            let version_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let version = version_str
                .lines()
                .next()
                .unwrap_or("")
                .replace("deno ", "")
                .trim()
                .to_string();
            Ok(DenoStatus {
                installed: true,
                version,
                path: deno_path.to_string_lossy().to_string(),
            })
        }
        _ => Ok(DenoStatus {
            installed: true,
            version: String::new(),
            path: deno_path.to_string_lossy().to_string(),
        }),
    }
}

/// 下载 Deno 可执行文件（从 zip 解压）
#[tauri::command]
pub async fn download_deno(app: AppHandle) -> Result<(), String> {
    let deno_path = utils::get_deno_path(&app)?;
    let url = utils::get_deno_download_url();

    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("err_download_failed:{}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 下载 zip 到临时文件
    let zip_path = deno_path.with_extension("zip");
    let mut file = tokio::fs::File::create(&zip_path)
        .await
        .map_err(|e| format!("err_create_file:{}", e))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("err_download_error:{}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("err_write_error:{}", e))?;

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
    }

    // 确保文件写入完成
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("err_flush_file:{}", e))?;
    drop(file);

    // 解压 deno 可执行文件
    let zip_path_clone = zip_path.clone();
    let deno_path_clone = deno_path.clone();
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
                let mut outfile = std::fs::File::create(&deno_path_clone)
                    .map_err(|e| format!("err_create_file:{}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("err_extract_deno:{}", e))?;
                return Ok(());
            }
        }
        Err(format!("err_not_found_in_zip:{}", deno_bin_name))
    })
    .await
    .map_err(|e| format!("err_task:{}", e))??;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&deno_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    // 清理 zip 文件
    let _ = tokio::fs::remove_file(&zip_path).await;

    Ok(())
}
