//! 字幕查询、下载与保存。

use crate::commands::support::{self, build_http_client};
use serde_json::Value;
use tauri::AppHandle;

use super::runner::run_ytdlp_tool;

/// 获取视频可用字幕列表（返回 subtitles + automatic_captions）
/// 支持单视频和合集：合集 URL 时聚合所有 entry 的字幕（同语言取首个出现的 entry）。
#[tauri::command]
pub async fn tool_fetch_subtitles(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let info = support::run_ytdlp_json(
        &app,
        &url,
        &["--no-check-formats"],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await?;

    let is_playlist = info.get("_type").and_then(Value::as_str) == Some("playlist");
    if is_playlist {
        if let Some(entries) = info.get("entries").and_then(Value::as_array) {
            return Ok(serde_json::json!({
                "title": info.get("title").cloned().unwrap_or(Value::Null),
                "subtitles": aggregate_subtitle_map(entries, "subtitles"),
                "automatic_captions": aggregate_subtitle_map(entries, "automatic_captions"),
            }));
        }
    }

    // 单视频：直接取 root 字段
    Ok(serde_json::json!({
        "title": info.get("title").cloned().unwrap_or(Value::Null),
        "subtitles": info.get("subtitles").cloned().unwrap_or(Value::Object(Default::default())),
        "automatic_captions": info.get("automatic_captions").cloned().unwrap_or(Value::Object(Default::default())),
    }))
}

/// 聚合 playlist 各 entry 的字幕到一个并集；同语言取首个出现的 entry 的 tracks。
fn aggregate_subtitle_map(entries: &[Value], field: &str) -> Value {
    let mut merged = serde_json::Map::new();
    for entry in entries {
        let Some(map) = entry.get(field).and_then(Value::as_object) else {
            continue;
        };
        for (lang, tracks) in map {
            if !merged.contains_key(lang) {
                if let Some(arr) = tracks.as_array() {
                    if !arr.is_empty() {
                        merged.insert(lang.clone(), tracks.clone());
                    }
                }
            }
        }
    }
    Value::Object(merged)
}

/// 下载单个字幕文件并另存为
#[tauri::command]
pub async fn tool_save_subtitle(
    url: String,
    file_path: String,
    proxy: Option<String>,
) -> Result<(), String> {
    let client = build_http_client(proxy.as_deref())?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("err_download_subtitle:{}", e))?;

    if !response.status().is_success() {
        return Err(format!("err_download_subtitle:HTTP {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("err_read_subtitle_data:{}", e))?;

    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("err_create_dir:{}", e))?;
    }

    tokio::fs::write(&file_path, &text)
        .await
        .map_err(|e| format!("err_save_file:{}", e))?;

    Ok(())
}

/// 下载视频字幕文件
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn tool_download_subtitles(
    app: AppHandle,
    url: String,
    download_dir: String,
    sub_langs: String,
    write_auto_subs: bool,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    let mut extra = vec![
        "--write-subs".to_string(),
        "--sub-langs".to_string(),
        sub_langs,
    ];
    if write_auto_subs {
        extra.push("--write-auto-subs".to_string());
    }
    run_ytdlp_tool(
        &app,
        &url,
        &download_dir,
        extra,
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}
