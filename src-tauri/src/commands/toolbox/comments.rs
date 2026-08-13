//! 视频评论查询。

use crate::commands::support;
use serde_json::Value;
use tauri::AppHandle;

/// 视频评论
#[derive(serde::Serialize, Clone)]
pub struct VideoComment {
    pub id: String,
    /// 父评论 ID（顶级评论为 "root"）
    pub parent: String,
    pub author: String,
    pub author_id: String,
    pub text: String,
    /// Unix 时间戳（秒）
    pub timestamp: i64,
    pub like_count: i64,
    pub is_favorited: bool,
    pub author_is_uploader: bool,
}

/// 评论排序方式
fn comment_sort_value(sort: &str) -> &'static str {
    match sort {
        "top" => "top",
        _ => "new",
    }
}

/// 获取视频评论（仅支持 YouTube；其他站点可能没有 comments 字段）
#[tauri::command]
pub async fn tool_fetch_comments(
    app: AppHandle,
    url: String,
    max_comments: u32,
    sort: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let max_str = max_comments.to_string();
    let extractor_arg = format!(
        "youtube:max_comments={};comment_sort={}",
        max_str,
        comment_sort_value(&sort)
    );

    let info = support::run_ytdlp_json(
        &app,
        &url,
        &[
            "--no-check-formats",
            "--no-playlist",
            "--write-comments",
            "--extractor-args",
            &extractor_arg,
        ],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await?;

    let comments_raw = info
        .get("comments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let comments: Vec<VideoComment> = comments_raw
        .into_iter()
        .map(|c| VideoComment {
            id: c
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            parent: c
                .get("parent")
                .and_then(Value::as_str)
                .unwrap_or("root")
                .to_string(),
            author: c
                .get("author")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            author_id: c
                .get("author_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            text: c
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            timestamp: c.get("timestamp").and_then(Value::as_i64).unwrap_or(0),
            like_count: c.get("like_count").and_then(Value::as_i64).unwrap_or(0),
            is_favorited: c
                .get("is_favorited")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            author_is_uploader: c
                .get("author_is_uploader")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect();

    Ok(serde_json::json!({
        "title": info.get("title").cloned().unwrap_or(Value::Null),
        "comment_count": info.get("comment_count").cloned().unwrap_or(Value::Null),
        "comments": comments,
    }))
}
