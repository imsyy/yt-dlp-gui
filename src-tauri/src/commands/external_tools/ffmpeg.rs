//! FFmpeg 与 FFprobe 的探测、安装和升级。

use crate::utils;
use tauri::AppHandle;

use super::support::{
    build_tool_status, download_to_file_with_progress, emit_tool_progress, executable_temp_path,
    replace_executable, verify_executable,
};
use super::ToolStatus;

#[tauri::command]
pub async fn get_ffmpeg_status(app: AppHandle) -> Result<ToolStatus, String> {
    let ffmpeg_path = utils::get_ffmpeg_path(&app)?;
    let ffprobe_path = utils::get_ffprobe_path(&app)?;
    let managed_path = utils::get_managed_ffmpeg_path(&app)?;
    let mut status = build_tool_status("ffmpeg", ffmpeg_path, managed_path, "-version").await?;
    // ffmpeg 与 ffprobe 必须同时存在才算装好（缺 ffprobe 就无法完成合并/探测）。
    // ffprobe 没有单独探测版本的必要，因此 runnable 只要求它存在即可。
    let ffprobe_present = ffprobe_path.exists();
    status.installed = status.installed && ffprobe_present;
    status.runnable = status.runnable && ffprobe_present;
    Ok(status)
}

/// 下载或验证中途失败时清掉两份临时文件，避免半成品留在应用数据目录。
async fn cleanup_temps(temps: &[std::path::PathBuf]) {
    for pending in temps {
        let _ = tokio::fs::remove_file(pending).await;
    }
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

    // 两个文件各占总进度的一半
    for (index, ((_, url), temp)) in urls.iter().zip(temps.iter()).enumerate() {
        if let Err(e) = download_to_file_with_progress(
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
            cleanup_temps(&temps).await;
            return Err(e);
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temp, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("err_set_permissions:{}", e))?;
        }

        if let Err(detail) = verify_executable(temp, "-version").await {
            cleanup_temps(&temps).await;
            return Err(format!("err_validate_ffmpeg:{}", detail));
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
