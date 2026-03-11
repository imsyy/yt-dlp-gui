//! 视频信息获取与 Cookie 管理

use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

use super::common;

// ========== Cookie 管理 ==========

/// 保存 Cookie 文本（Netscape 格式）到应用数据目录
#[tauri::command]
pub async fn save_cookie_text(app: AppHandle, text: String) -> Result<String, String> {
    let cookie_path = utils::get_cookie_path(&app)?;
    tokio::fs::write(&cookie_path, text.as_bytes())
        .await
        .map_err(|e| format!("err_save_cookie:{}", e))?;
    Ok(cookie_path.to_string_lossy().to_string())
}

// ========== 视频信息 ==========

/// 使用 yt-dlp -J 获取视频元信息（标题、格式列表、字幕等）
#[tauri::command]
pub async fn fetch_video_info(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    common::run_ytdlp_json(
        &app,
        &url,
        &[],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}
