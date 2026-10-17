//! Bun 的探测、安装和升级。

use crate::utils;
#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
use tauri::AppHandle;

use super::support::{
    build_tool_status, download_to_file_with_progress, emit_tool_progress, executable_temp_path,
    replace_executable, verify_executable,
};
use super::ToolStatus;

/// 获取 Bun 安装状态和版本
#[tauri::command]
pub async fn get_bun_status(app: AppHandle) -> Result<ToolStatus, String> {
    let bun_path = utils::get_bun_path(&app)?;
    let managed_path = utils::get_managed_bun_path(&app)?;
    build_tool_status("bun", bun_path, managed_path, "--version").await
}

async fn download_bun_impl(app: AppHandle, operation: &str) -> Result<(), String> {
    emit_tool_progress(&app, "bun", operation, "downloading", Some(0.0));
    let bun_path = utils::get_managed_bun_path(&app)?;
    let temp_path = executable_temp_path(&bun_path, "download")?;

    // Bun 官方发行的是 zip 包，先整包落到同目录临时文件，再从中解压出二进制。
    let bun_file_name = bun_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("err_invalid_executable_path")?;
    let zip_path = bun_path.with_file_name(format!("{}.download.zip", bun_file_name));

    download_to_file_with_progress(
        &app,
        "bun",
        operation,
        utils::get_bun_download_url(),
        &zip_path,
        0.0,
        100.0,
    )
    .await?;

    emit_tool_progress(&app, "bun", operation, "installing", None);

    // 先解压到临时文件，验证成功后才替换现有 Bun。
    let zip_path_clone = zip_path.clone();
    let temp_path_clone = temp_path.clone();
    let bun_bin_name = if cfg!(target_os = "windows") {
        "bun.exe"
    } else {
        "bun"
    };

    let extracted = tokio::task::spawn_blocking(move || {
        let file =
            std::fs::File::open(&zip_path_clone).map_err(|e| format!("err_open_zip:{}", e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("err_read_zip:{}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("err_read_zip_entry:{}", e))?;
            // 条目名只用于匹配，解压目标固定为 temp_path，因此不存在路径逃逸
            let name = entry.name().to_lowercase();
            if name == bun_bin_name || name.ends_with(&format!("/{}", bun_bin_name)) {
                let mut outfile = std::fs::File::create(&temp_path_clone)
                    .map_err(|e| format!("err_create_file:{}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("err_extract_bun:{}", e))?;
                return Ok(());
            }
        }
        Err(format!("err_not_found_in_zip:{}", bun_bin_name))
    })
    .await
    .map_err(|e| format!("err_task:{}", e))?;

    if let Err(detail) = extracted {
        let _ = tokio::fs::remove_file(&zip_path).await;
        return Err(detail);
    }

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    if let Err(detail) = verify_executable(&temp_path, "--version").await {
        let _ = tokio::fs::remove_file(&zip_path).await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!("err_validate_bun:{}", detail));
    }

    replace_executable(&temp_path, &bun_path)?;
    let _ = tokio::fs::remove_file(&zip_path).await;
    emit_tool_progress(&app, "bun", operation, "complete", Some(100.0));

    Ok(())
}

/// 下载 Bun 可执行文件（从 zip 解压）
#[tauri::command]
pub async fn download_bun(app: AppHandle) -> Result<(), String> {
    download_bun_impl(app, "install").await
}

#[tauri::command]
pub async fn update_bun(app: AppHandle) -> Result<String, String> {
    if utils::get_tool_source("bun")? == utils::ToolSource::Managed {
        download_bun_impl(app, "update").await?;
        return Ok("Updated managed Bun".to_string());
    }

    let bun_path = utils::get_bun_path(&app)?;
    if !bun_path.exists() {
        return Err("err_bun_not_installed".to_string());
    }
    emit_tool_progress(&app, "bun", "update", "updating", None);
    let mut cmd = tokio::process::Command::new(&bun_path);
    cmd.arg("upgrade");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_update_bun:{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "err_update_bun:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    emit_tool_progress(&app, "bun", "update", "complete", Some(100.0));
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
