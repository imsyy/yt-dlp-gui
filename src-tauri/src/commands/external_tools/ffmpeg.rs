//! FFmpeg 与 FFprobe 的探测、安装和升级。

use crate::utils;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey, RegValue};

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

#[tauri::command]
pub fn add_ffmpeg_to_path(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };

        let ffmpeg_path = utils::get_ffmpeg_path(&app)?;
        if !ffmpeg_path.is_file() {
            return Err("err_ffmpeg_not_found".to_string());
        }
        let ffmpeg_dir = ffmpeg_path
            .parent()
            .ok_or_else(|| "err_ffmpeg_path_invalid".to_string())?
            .to_path_buf();
        let normalized_dir = ffmpeg_dir
            .to_string_lossy()
            .trim_end_matches(|character| character == '\\' || character == '/')
            .to_string();
        let environment = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .or_else(|_| {
                RegKey::predef(HKEY_CURRENT_USER)
                    .create_subkey("Environment")
                    .map(|(key, _)| key)
            })
            .map_err(|e| format!("err_open_user_environment:{}", e))?;
        let current_user_path: String = environment.get_value("Path").unwrap_or_default();
        let path_value_type = environment
            .get_raw_value("Path")
            .map(|value| value.vtype)
            .unwrap_or(REG_EXPAND_SZ);
        let already_registered = std::env::split_paths(std::ffi::OsStr::new(&current_user_path))
            .any(|entry| {
                entry
                    .to_string_lossy()
                    .trim_end_matches(|character| character == '\\' || character == '/')
                    .eq_ignore_ascii_case(&normalized_dir)
            });

        if !already_registered {
            let mut entries: Vec<_> =
                std::env::split_paths(std::ffi::OsStr::new(&current_user_path)).collect();
            entries.push(ffmpeg_dir.clone());
            let updated_path = std::env::join_paths(entries)
                .map_err(|e| format!("err_join_user_path:{}", e))?;
            let path_value = updated_path.to_string_lossy().to_string();
            let bytes = path_value
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            environment
                .set_raw_value(
                    "Path",
                    &RegValue {
                        vtype: if path_value_type == REG_EXPAND_SZ {
                            REG_EXPAND_SZ
                        } else {
                            REG_SZ
                        },
                        bytes,
                    },
                )
                .map_err(|e| format!("err_update_user_path:{}", e))?;
        }

        let current_process_path = std::env::var_os("PATH").unwrap_or_default();
        let mut process_entries: Vec<_> = std::env::split_paths(&current_process_path).collect();
        let process_has_dir = process_entries.iter().any(|entry| {
            entry
                .to_string_lossy()
                .trim_end_matches(|character| character == '\\' || character == '/')
                .eq_ignore_ascii_case(&normalized_dir)
        });
        if !process_has_dir {
            process_entries.push(ffmpeg_dir);
            let updated_process_path = std::env::join_paths(process_entries)
                .map_err(|e| format!("err_join_process_path:{}", e))?;
            std::env::set_var("PATH", updated_process_path);
        }

        let environment_name: Vec<u16> = "Environment\0".encode_utf16().collect();
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                environment_name.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }

        Ok(if already_registered {
            "already-present".to_string()
        } else {
            "added".to_string()
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("err_windows_only".to_string())
    }
}
