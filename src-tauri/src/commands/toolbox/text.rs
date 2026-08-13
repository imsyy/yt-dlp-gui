//! 工具箱文本下载与保存。

use crate::commands::support::build_http_client;

/// 下载 URL 文本内容并返回（用于前端获取字幕文本做合并处理）
#[tauri::command]
pub async fn tool_download_text(url: String, proxy: Option<String>) -> Result<String, String> {
    let client = build_http_client(proxy.as_deref())?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("err_download_failed:{}", e))?;

    if !response.status().is_success() {
        return Err(format!("err_download_failed:HTTP {}", response.status()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("err_read_text:{}", e))
}

/// 将文本内容保存到指定文件路径
#[tauri::command]
pub async fn tool_save_text_to_file(content: String, file_path: String) -> Result<(), String> {
    // 路径安全检查：阻止写入系统关键路径
    let path = std::path::Path::new(&file_path);
    if file_path.contains("..") {
        return Err("err_path_traversal".to_string());
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("err_create_dir:{}", e))?;
    }

    tokio::fs::write(&file_path, &content)
        .await
        .map_err(|e| format!("err_save_file:{}", e))?;

    Ok(())
}
