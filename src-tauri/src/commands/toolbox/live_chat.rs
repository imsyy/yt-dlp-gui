//! 直播聊天下载、流式解析与 JSONL 缓存。

use crate::{
    commands::support::{append_cookie_proxy_args, extract_ytdlp_error, run_ytdlp_capture},
    utils,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LiveChatMessage {
    pub idx: usize,
    pub time: String,
    pub timestamp_usec: i64,
    pub author: String,
    pub channel_id: String,
    pub message: String,
    pub msg_type: String,
    pub amount: String,
}

fn extract_runs_text(value: Option<&Value>) -> Option<String> {
    let text = value?
        .as_array()?
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn parse_live_chat_line(line: &str) -> Option<LiveChatMessage> {
    let root: Value = serde_json::from_str(line).ok()?;
    let actions = root
        .pointer("/replayChatItemAction/actions")
        .and_then(Value::as_array)?;
    let item = actions.iter().find_map(|action| {
        action
            .pointer("/addChatItemAction/item")
            .or_else(|| action.pointer("/addLiveChatTickerItemAction/item"))
    })?;
    let (renderer, msg_type) = if let Some(renderer) = item.get("liveChatTextMessageRenderer") {
        (renderer, "text")
    } else if let Some(renderer) = item.get("liveChatPaidMessageRenderer") {
        (renderer, "paid")
    } else if let Some(renderer) = item.get("liveChatMembershipItemRenderer") {
        (renderer, "membership")
    } else {
        return None;
    };

    let author = renderer
        .pointer("/authorName/simpleText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let channel_id = renderer
        .get("authorExternalChannelId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let timestamp_usec = renderer
        .get("timestampUsec")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let time = renderer
        .pointer("/timestampText/simpleText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let message = extract_runs_text(renderer.pointer("/message/runs"))
        .or_else(|| extract_runs_text(renderer.pointer("/headerSubtext/runs")))
        .unwrap_or_default();
    let amount = renderer
        .pointer("/purchaseAmountText/simpleText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    Some(LiveChatMessage {
        idx: 0,
        time,
        timestamp_usec,
        author,
        channel_id,
        message,
        msg_type: msg_type.to_owned(),
        amount,
    })
}

async fn download_live_chat(
    app: &AppHandle,
    run_id: &str,
    url: &str,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join(format!(
        "ytdlp-livechat-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|error| format!("err_create_dir:{error}"))?;

    let mut args = vec![
        "--skip-download".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--no-warnings".to_string(),
        "--socket-timeout".to_string(),
        "15".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--write-subs".to_string(),
        "--sub-langs".to_string(),
        "live_chat".to_string(),
        "-o".to_string(),
        temp_dir
            .join("%(title).200s.%(ext)s")
            .to_string_lossy()
            .to_string(),
    ];
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_ffmpeg_location_args(app));
    args.extend(utils::build_plugin_args(app));
    append_cookie_proxy_args(&mut args, cookie_file, cookie_browser, proxy);
    args.push(url.to_owned());

    let output = match run_ytdlp_capture(app, Some(run_id), args).await {
        Ok(output) => output,
        Err(error) => {
            // 子进程未能启动或等待失败时也要清掉刚建的临时目录，避免残留
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return Err(error);
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return Err(extract_ytdlp_error(&stderr));
    }
    Ok(temp_dir)
}

async fn find_chat_file(directory: &Path) -> Result<PathBuf, String> {
    let mut entries = tokio::fs::read_dir(directory)
        .await
        .map_err(|error| format!("err_read_livechat:{error}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| format!("err_read_livechat:{error}"))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("live_chat") && name.ends_with(".json") {
            return Ok(entry.path());
        }
    }
    Err("err_livechat_not_found".into())
}

pub(crate) async fn fetch_live_chat_to_jsonl(
    app: &AppHandle,
    url: &str,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
    run_id: &str,
) -> Result<(String, i64), String> {
    let temp_dir = download_live_chat(app, run_id, url, cookie_file, cookie_browser, proxy).await?;
    let result = async {
        let source = find_chat_file(&temp_dir).await?;
        let relative = format!("tool-results/livechat/{run_id}.jsonl");
        let final_path = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join(&relative);
        let part_path = final_path.with_extension("jsonl.part");
        if let Some(parent) = final_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| error.to_string())?;
        }
        let input = tokio::fs::File::open(source)
            .await
            .map_err(|error| format!("err_read_livechat:{error}"))?;
        let mut lines = BufReader::new(input).lines();
        let mut output = tokio::fs::File::create(&part_path)
            .await
            .map_err(|error| error.to_string())?;
        let mut total = 0usize;
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|error| format!("err_read_livechat:{error}"))?
        {
            let Some(mut message) = parse_live_chat_line(&line) else {
                continue;
            };
            message.idx = total;
            let mut encoded = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
            encoded.push(b'\n');
            output
                .write_all(&encoded)
                .await
                .map_err(|error| error.to_string())?;
            total += 1;
        }
        output.flush().await.map_err(|error| error.to_string())?;
        drop(output);
        if total == 0 {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err("err_livechat_empty".into());
        }
        tokio::fs::rename(part_path, final_path)
            .await
            .map_err(|error| error.to_string())?;
        Ok((relative, total as i64))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(temp_dir).await;
    result
}
