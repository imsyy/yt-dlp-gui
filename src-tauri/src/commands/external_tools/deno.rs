//! Deno 的探测、安装和升级。

use crate::utils;
#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
use tauri::AppHandle;

use super::support::{
    build_tool_status, download_to_file_with_progress, emit_tool_progress, executable_temp_path,
    replace_executable, verify_executable,
};
use super::ToolStatus;

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

    // 官方发行的是 zip 包，先整包落到同目录临时文件，再从中解压出二进制。
    // 这里不能用 executable_temp_path：它会把 ".download.zip" 当成扩展名后缀拼错。
    let deno_file_name = deno_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("err_invalid_executable_path")?;
    let zip_path = deno_path.with_file_name(format!("{}.download.zip", deno_file_name));

    download_to_file_with_progress(
        &app,
        "deno",
        operation,
        utils::get_deno_download_url(),
        &zip_path,
        0.0,
        100.0,
    )
    .await?;

    emit_tool_progress(&app, "deno", operation, "installing", None);

    // 先解压到临时文件，验证成功后才替换现有 Deno。
    let zip_path_clone = zip_path.clone();
    let temp_path_clone = temp_path.clone();
    let deno_bin_name = if cfg!(target_os = "windows") {
        "deno.exe"
    } else {
        "deno"
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
        return Err(format!("err_validate_deno:{}", detail));
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
