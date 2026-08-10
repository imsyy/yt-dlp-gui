//! 工具箱命令：封面下载、字幕下载、直播弹幕获取

use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

use super::common::{self, append_cookie_proxy_args, build_http_client, extract_ytdlp_error};

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

/// 通用工具命令执行器（--skip-download 模式，不下载视频本身）
async fn run_ytdlp_tool(
    app: &AppHandle,
    url: &str,
    download_dir: &str,
    extra_args: Vec<String>,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

    let mut args = vec![
        "--skip-download".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--windows-filenames".to_string(),
        "--no-warnings".to_string(),
        "--socket-timeout".to_string(),
        "15".to_string(),
        "--retries".to_string(),
        "3".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_plugin_args(app));
    args.extend(utils::build_youtube_extractor_args());
    args.extend(utils::build_ffmpeg_location_args());

    let output_template = std::path::PathBuf::from(download_dir)
        .join("%(title).200s.%(ext)s")
        .to_string_lossy()
        .to_string();
    args.push("-o".to_string());
    args.push(output_template);

    args.extend(extra_args);
    append_cookie_proxy_args(&mut args, cookie_file, cookie_browser, proxy);
    args.push(url.to_string());

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_run_ytdlp:{}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        Err(extract_ytdlp_error(&stderr))
    }
}

/// 轻量获取视频封面列表（跳过格式检查，速度更快）
#[tauri::command]
pub async fn tool_fetch_thumbnails(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    common::run_ytdlp_json(
        &app,
        &url,
        &["--no-check-formats", "--no-playlist"],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}

/// 将指定 URL 的图片下载到指定文件路径（另存为）
#[tauri::command]
pub async fn tool_save_thumbnail(
    url: String,
    file_path: String,
    proxy: Option<String>,
) -> Result<(), String> {
    let client = build_http_client(proxy.as_deref())?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("err_download_thumbnail:{}", e))?;

    if !response.status().is_success() {
        return Err(format!("err_download_thumbnail:HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("err_read_thumbnail_data:{}", e))?;

    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("err_create_dir:{}", e))?;
    }

    tokio::fs::write(&file_path, &bytes)
        .await
        .map_err(|e| format!("err_save_file:{}", e))?;

    Ok(())
}

/// 下载视频封面图
#[tauri::command]
pub async fn tool_download_thumbnail(
    app: AppHandle,
    url: String,
    download_dir: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    run_ytdlp_tool(
        &app,
        &url,
        &download_dir,
        vec![
            "--write-thumbnail".to_string(),
            "--convert-thumbnails".to_string(),
            "jpg".to_string(),
        ],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}

/// 获取视频章节信息（chapters 字段）
#[tauri::command]
pub async fn tool_fetch_chapters(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let info = common::run_ytdlp_json(
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
    let info = common::run_ytdlp_json(
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

    let info = common::run_ytdlp_json(
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
    args.extend(utils::build_plugin_args(&app));
    args.extend(utils::build_ffmpeg_location_args());
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
