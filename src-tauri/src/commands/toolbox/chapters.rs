//! 视频章节查询。

use crate::commands::support;
use serde_json::Value;
use tauri::AppHandle;

/// 获取视频章节信息（chapters 字段）
#[tauri::command]
pub async fn tool_fetch_chapters(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let info = support::run_ytdlp_json(
        &app,
        &url,
        &["--no-check-formats", "--no-playlist"],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await?;

    Ok(serde_json::json!({
        "title": info.get("title").cloned().unwrap_or(Value::Null),
        "duration": info.get("duration").cloned().unwrap_or(Value::Null),
        "chapters": info.get("chapters").cloned().unwrap_or(Value::Array(vec![])),
    }))
}
