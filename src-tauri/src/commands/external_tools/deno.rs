//! Deno 的探测、安装和升级。

use crate::utils;
#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

use super::support::{
    build_tool_status, emit_tool_progress, executable_temp_path, replace_executable,
    DOWNLOAD_TIMEOUT,
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
    .inspect_err(|_| {
        let _ = std::fs::remove_file(&zip_path);
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
