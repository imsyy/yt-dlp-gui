//! FFmpeg 与 FFprobe 的探测、安装和升级。

use crate::utils;
use futures_util::StreamExt;
use std::path::Path;
use tauri::AppHandle;

use super::support::{
    build_tool_status, emit_tool_progress, executable_temp_path, replace_executable,
    DOWNLOAD_TIMEOUT,
};
use super::ToolStatus;

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
