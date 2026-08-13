//! 直播聊天下载与解析。

use crate::{
    commands::{support::append_cookie_proxy_args, support::extract_ytdlp_error, CREATE_NO_WINDOW},
    utils,
};
use serde_json::Value;
use tauri::AppHandle;

/// 直播弹幕消息
#[derive(serde::Serialize, Clone)]
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

/// 从单行 JSONL 解析出一条弹幕消息
fn parse_live_chat_line(line: &str) -> Option<LiveChatMessage> {
    let v: Value = serde_json::from_str(line).ok()?;
    let actions = v
        .pointer("/replayChatItemAction/actions")
        .and_then(|a| a.as_array())?;

    for action in actions {
        let item = action.pointer("/addChatItemAction/item")?;

        let (renderer, msg_type) = if let Some(r) = item.get("liveChatTextMessageRenderer") {
            (r, "text")
        } else if let Some(r) = item.get("liveChatPaidMessageRenderer") {
            (r, "paid")
        } else if let Some(r) = item.get("liveChatMembershipItemRenderer") {
            (r, "membership")
        } else {
            continue;
        };

        let author = renderer
            .pointer("/authorName/simpleText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let channel_id = renderer
            .get("authorExternalChannelId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let timestamp_usec = renderer
            .get("timestampUsec")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let time = renderer
            .pointer("/timestampText/simpleText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let message = extract_runs_text(renderer.pointer("/message/runs"))
            .or_else(|| extract_runs_text(renderer.pointer("/headerSubtext/runs")))
            .unwrap_or_default();

        let amount = if msg_type == "paid" {
            renderer
                .pointer("/purchaseAmountText/simpleText")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        return Some(LiveChatMessage {
            idx: 0,
            time,
            timestamp_usec,
            author,
            channel_id,
            message,
            msg_type: msg_type.to_string(),
            amount,
        });
    }
    None
}

/// 从 runs 数组中提取拼接文本
fn extract_runs_text(runs: Option<&Value>) -> Option<String> {
    let arr = runs?.as_array()?;
    let text: String = arr
        .iter()
        .filter_map(|r| {
            r.get("text")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// 获取直播弹幕数据（下载到临时目录，解析后返回结构化数据）
#[tauri::command]
pub async fn tool_fetch_live_chat(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Vec<LiveChatMessage>, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

    // 创建临时目录
    let temp_dir = std::env::temp_dir().join(format!(
        "ytdlp-livechat-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("err_create_dir:{}", e))?;

    let temp_path = temp_dir.to_string_lossy().to_string();
    let output_template = format!("{}/%(title).200s.%(ext)s", temp_path);

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
        output_template,
    ];
    args.extend(utils::build_js_runtime_args(&app));
    args.extend(utils::build_ffmpeg_location_args(&app));
    args.extend(utils::build_plugin_args(&app));
    append_cookie_proxy_args(
        &mut args,
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    );
    args.push(url);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match cmd.output().await {
        Ok(output) => output,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return Err(format!("err_run_ytdlp:{}", e));
        }
    };

    if !output.status.success() {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(extract_ytdlp_error(&stderr));
    }

    // 解析完成后统一清理临时目录
    let result = parse_live_chat_dir(&temp_dir).await;
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

/// 从临时目录中查找并解析 live_chat 文件
async fn parse_live_chat_dir(dir: &std::path::Path) -> Result<Vec<LiveChatMessage>, String> {
    let mut chat_file = None;
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("err_read_dir:{}", e))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("err_read_file_list:{}", e))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("live_chat") && name.ends_with(".json") {
            chat_file = Some(entry.path());
            break;
        }
    }

    let chat_file = chat_file.ok_or("err_livechat_not_found".to_string())?;

    let content = tokio::fs::read_to_string(&chat_file)
        .await
        .map_err(|e| format!("err_read_livechat:{}", e))?;

    let mut messages: Vec<LiveChatMessage> =
        content.lines().filter_map(parse_live_chat_line).collect();

    for (i, msg) in messages.iter_mut().enumerate() {
        msg.idx = i;
    }

    if messages.is_empty() {
        return Err("err_livechat_empty".to_string());
    }

    Ok(messages)
}
