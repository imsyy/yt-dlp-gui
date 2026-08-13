//! yt-dlp 插件的安装与卸载。

use crate::{commands::support, utils};
use tauri::AppHandle;

use super::support::DOWNLOAD_TIMEOUT;

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
    let target = support::validate_path_within(&plugin_dir, &file_path)?;
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
