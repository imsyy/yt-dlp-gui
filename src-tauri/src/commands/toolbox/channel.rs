//! 频道/主播视频归档工具后端命令实现。

use crate::commands::support::{append_cookie_proxy_args, run_ytdlp_json_tracked};
use crate::db::channels::{
    delete_channel, get_channel, get_channel_existing_video_ids, get_channel_videos_page,
    list_channels, update_channel_sync_status, upsert_channel, upsert_channel_videos_batch,
    ChannelRecord, ChannelVideoRecord, ChannelVideosPage, ChannelVideosQuery,
};
use crate::db::DatabaseState;
use crate::platform::process::ProcessRegistry;
use crate::utils;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncBufReadExt;

#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;

/// 正在进行的频道同步任务控制表 (channel_id -> 取消标志)
static ACTIVE_SYNCS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn get_active_syncs() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_SYNCS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 频道同步进度事件载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSyncProgressPayload {
    pub channel_id: String,
    pub status: String, // "syncing" | "completed" | "error" | "cancelled"
    pub total_synced: usize,
    pub new_synced: usize,
    pub current_tab: Option<String>,
    pub message: Option<String>,
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 规范化频道基础 URL
fn clean_channel_url(url: &str) -> String {
    let mut cleaned = url.trim().trim_end_matches('/').to_string();
    for suffix in [
        "/videos",
        "/shorts",
        "/streams",
        "/featured",
        "/podcasts",
        "/playlists",
        "/community",
        "/about",
    ] {
        if cleaned.ends_with(suffix) {
            cleaned.truncate(cleaned.len() - suffix.len());
            break;
        }
    }
    cleaned
}

/// 检测平台类型
fn detect_platform(url: &str, info: &Value) -> String {
    let lower_url = url.to_lowercase();
    if lower_url.contains("youtube.com") || lower_url.contains("youtu.be") {
        return "youtube".to_string();
    }
    if lower_url.contains("bilibili.com") {
        return "bilibili".to_string();
    }
    if lower_url.contains("twitch.tv") {
        return "twitch".to_string();
    }
    if let Some(extractor) = info.get("extractor_key").and_then(Value::as_str) {
        let ext_lower = extractor.to_lowercase();
        if ext_lower.contains("youtube") {
            return "youtube".to_string();
        }
        if ext_lower.contains("bili") {
            return "bilibili".to_string();
        }
        return extractor.to_string();
    }
    "generic".to_string()
}

/// 提取频道唯一 ID
fn extract_channel_id(info: &Value, url: &str, platform: &str) -> String {
    if let Some(id) = info.get("channel_id").and_then(Value::as_str) {
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if let Some(id) = info.get("uploader_id").and_then(Value::as_str) {
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if let Some(id) = info.get("id").and_then(Value::as_str) {
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if platform == "bilibili" {
        if let Some(pos) = url.find("space.bilibili.com/") {
            let sub = &url[pos + "space.bilibili.com/".len()..];
            let mid: String = sub.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !mid.is_empty() {
                return format!("bilibili:{}", mid);
            }
        }
    }
    clean_channel_url(url)
}

/// 提取频道名称
fn extract_channel_title(info: &Value, url: &str) -> String {
    if let Some(name) = info.get("channel").and_then(Value::as_str) {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    if let Some(name) = info.get("uploader").and_then(Value::as_str) {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    if let Some(name) = info.get("title").and_then(Value::as_str) {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    clean_channel_url(url)
}

/// 提取频道头像
fn extract_channel_avatar(info: &Value) -> String {
    if let Some(thumbs) = info.get("thumbnails").and_then(Value::as_array) {
        // 优先查找 id 或 url 包含 avatar 的项
        for t in thumbs {
            if let Some(id) = t.get("id").and_then(Value::as_str) {
                if id.to_lowercase().contains("avatar") {
                    if let Some(u) = t.get("url").and_then(Value::as_str) {
                        return u.to_string();
                    }
                }
            }
        }
        // 其次查找正方形缩略图（width == height）
        for t in thumbs {
            let w = t.get("width").and_then(Value::as_i64);
            let h = t.get("height").and_then(Value::as_i64);
            if let (Some(w_val), Some(h_val)) = (w, h) {
                if w_val > 0 && w_val == h_val {
                    if let Some(u) = t.get("url").and_then(Value::as_str) {
                        return u.to_string();
                    }
                }
            }
        }
        // 兜底返回偏好最高的缩略图
        if let Some(last) = thumbs.last() {
            if let Some(u) = last.get("url").and_then(Value::as_str) {
                return u.to_string();
            }
        }
    }
    if let Some(thumb) = info.get("thumbnail").and_then(Value::as_str) {
        return thumb.to_string();
    }
    String::new()
}

/// 提取频道 Banner
fn extract_channel_banner(info: &Value) -> String {
    if let Some(thumbs) = info.get("thumbnails").and_then(Value::as_array) {
        for t in thumbs {
            if let Some(id) = t.get("id").and_then(Value::as_str) {
                if id.to_lowercase().contains("banner") {
                    if let Some(u) = t.get("url").and_then(Value::as_str) {
                        return u.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// 提取视频条目最佳缩略图
fn extract_video_thumbnail(entry: &Value) -> String {
    if let Some(thumbs) = entry.get("thumbnails").and_then(Value::as_array) {
        if let Some(best) = thumbs.iter().max_by_key(|t| {
            let w = t.get("width").and_then(Value::as_i64).unwrap_or(0);
            let h = t.get("height").and_then(Value::as_i64).unwrap_or(0);
            w * h
        }) {
            if let Some(u) = best.get("url").and_then(Value::as_str) {
                return u.to_string();
            }
        }
        if let Some(last) = thumbs.last() {
            if let Some(u) = last.get("url").and_then(Value::as_str) {
                return u.to_string();
            }
        }
    }
    entry
        .get("thumbnail")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// 解析发布日期字符串 (YYYYMMDD) 为毫秒时间戳
fn parse_upload_date(date_str: &str) -> Option<i64> {
    if date_str.len() == 8 {
        let year: i32 = date_str[0..4].parse().ok()?;
        let month: u32 = date_str[4..6].parse().ok()?;
        let day: u32 = date_str[6..8].parse().ok()?;
        if month < 1 || month > 12 || day < 1 || day > 31 {
            return None;
        }
        let y = if month <= 2 { year - 1 } else { year } as i64;
        let m = if month <= 2 { month + 9 } else { month - 3 } as i64;
        let d = day as i64;
        let days = 365 * y + y / 4 - y / 100 + y / 400 + (m * 306 + 5) / 10 + (d - 1) - 719468;
        Some(days * 86400 * 1000)
    } else {
        None
    }
}

/// 添加并解析新频道/主播
#[tauri::command]
pub async fn channel_add(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<ChannelRecord, String> {
    let raw_url = url.trim();
    if raw_url.is_empty() {
        return Err("err_channel_url_empty".to_string());
    }

    let cleaned_url = clean_channel_url(raw_url);

    // 首先尝试轻量提取 (--playlist-items 0)，快速获取频道元数据而无需遍历条目
    let info = match run_ytdlp_json_tracked(
        &app,
        None,
        &cleaned_url,
        &["--no-check-formats", "--playlist-items", "0"],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
    {
        Ok(val) => val,
        Err(_) => {
            // 若 playlist-items 0 不受该站点 extractor 支持，回退到 playlist-end 1
            run_ytdlp_json_tracked(
                &app,
                None,
                &cleaned_url,
                &["--no-check-formats", "--playlist-end", "1"],
                cookie_file.as_deref(),
                cookie_browser.as_deref(),
                proxy.as_deref(),
            )
            .await?
        }
    };

    let platform = detect_platform(&cleaned_url, &info);
    let channel_id = extract_channel_id(&info, &cleaned_url, &platform);
    let title = extract_channel_title(&info, &cleaned_url);
    let uploader = info
        .get("uploader")
        .and_then(Value::as_str)
        .unwrap_or(&title)
        .to_string();
    let uploader_id = info
        .get("uploader_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let avatar = extract_channel_avatar(&info);
    let banner = extract_channel_banner(&info);
    let description = info
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let subscriber_count = info
        .get("channel_follower_count")
        .or_else(|| info.get("subscriber_count"))
        .and_then(Value::as_i64);

    let canonical_url = info
        .get("channel_url")
        .or_else(|| info.get("uploader_url"))
        .or_else(|| info.get("webpage_url"))
        .and_then(Value::as_str)
        .map(clean_channel_url)
        .unwrap_or_else(|| cleaned_url.clone());

    let db = app.state::<DatabaseState>();
    let existing = get_channel(&db, &channel_id)?;

    let now = now_millis();
    let record = ChannelRecord {
        id: channel_id,
        url: canonical_url,
        title,
        uploader,
        uploader_id,
        avatar,
        banner,
        description,
        platform,
        subscriber_count,
        video_count: existing.as_ref().map(|c| c.video_count).unwrap_or(0),
        last_synced_at: existing.as_ref().and_then(|c| c.last_synced_at),
        sync_status: "idle".to_string(),
        sync_error: None,
        created_at: existing.as_ref().map(|c| c.created_at).unwrap_or(now),
    };

    upsert_channel(&db, &record)?;
    Ok(record)
}

/// 查询所有保存的频道
#[tauri::command]
pub async fn channel_list(db: State<'_, DatabaseState>) -> Result<Vec<ChannelRecord>, String> {
    list_channels(&db)
}

/// 查询指定频道详情
#[tauri::command]
pub async fn channel_get(
    db: State<'_, DatabaseState>,
    channel_id: String,
) -> Result<Option<ChannelRecord>, String> {
    get_channel(&db, &channel_id)
}

/// 删除频道及归档视频
#[tauri::command]
pub async fn channel_delete(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    channel_id: String,
) -> Result<(), String> {
    // 若正在同步中，先强制取消
    let _ = channel_sync_cancel(app, channel_id.clone()).await;
    delete_channel(&db, &channel_id)
}

/// 分页查询频道下的视频列表
#[tauri::command]
pub async fn channel_get_videos(
    db: State<'_, DatabaseState>,
    query: ChannelVideosQuery,
) -> Result<ChannelVideosPage, String> {
    get_channel_videos_page(&db, &query)
}

/// 取消指定频道的同步
#[tauri::command]
pub async fn channel_sync_cancel(app: AppHandle, channel_id: String) -> Result<(), String> {
    if let Ok(mut syncs) = get_active_syncs().lock() {
        if let Some(flag) = syncs.remove(&channel_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
    let run_id = format!("channel_sync_{}", channel_id);
    if let Some(pid) = app.state::<ProcessRegistry>().take(&run_id) {
        let _ = crate::platform::process::kill_process(pid);
    }
    let db = app.state::<DatabaseState>();
    let _ = update_channel_sync_status(&db, &channel_id, "idle", None);

    let _ = app.emit(
        "channel-sync-progress",
        ChannelSyncProgressPayload {
            channel_id,
            status: "cancelled".to_string(),
            total_synced: 0,
            new_synced: 0,
            current_tab: None,
            message: Some("Sync cancelled by user".to_string()),
        },
    );
    Ok(())
}

/// 启动频道视频同步（后台异步流式抓取与增量更新）
#[tauri::command]
pub async fn channel_sync_start(
    app: AppHandle,
    channel_id: String,
    mode: String, // "incremental" | "full"
    tabs: Option<Vec<String>>,
    sleep_interval: Option<f64>,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<(), String> {
    let db = app.state::<DatabaseState>();
    let channel = get_channel(&db, &channel_id)?
        .ok_or_else(|| format!("err_channel_not_found:{}", channel_id))?;

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut syncs = get_active_syncs()
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        if syncs.contains_key(&channel_id) {
            return Err("err_channel_already_syncing".to_string());
        }
        syncs.insert(channel_id.clone(), cancel_flag.clone());
    }

    update_channel_sync_status(&db, &channel_id, "syncing", None)?;

    let app_handle = app.clone();
    let is_incremental = mode == "incremental";

    tokio::spawn(async move {
        let db_state = app_handle.state::<DatabaseState>();
        let run_id = format!("channel_sync_{}", channel_id);

        // 确定需要拉取的 URL 列表与内容类型标签
        let base_url = clean_channel_url(&channel.url);
        let sync_targets: Vec<(String, String)> = if channel.platform == "youtube" {
            let configured_tabs = tabs.unwrap_or_else(|| {
                vec![
                    "videos".to_string(),
                    "shorts".to_string(),
                    "streams".to_string(),
                ]
            });
            let mut targets = Vec::new();
            for tab in configured_tabs {
                let tab_name = tab.trim().to_lowercase();
                if !tab_name.is_empty() {
                    targets.push((tab_name.clone(), format!("{}/{}", base_url, tab_name)));
                }
            }
            if targets.is_empty() {
                targets.push(("videos".to_string(), format!("{}/videos", base_url)));
            }
            targets
        } else {
            vec![("video".to_string(), channel.url.clone())]
        };

        // 增量模式下预先加载已存视频 ID 集合
        let mut existing_ids: HashSet<String> = if is_incremental {
            get_channel_existing_video_ids(&db_state, &channel_id).unwrap_or_default()
        } else {
            HashSet::new()
        };

        let mut total_synced_count = 0usize;
        let mut new_synced_count = 0usize;
        let mut encountered_error: Option<String> = None;

        let ytdlp_path = match utils::get_ytdlp_path(&app_handle) {
            Ok(p) => p,
            Err(e) => {
                let _ = update_channel_sync_status(&db_state, &channel_id, "error", Some(&e));
                if let Ok(mut syncs) = get_active_syncs().lock() {
                    syncs.remove(&channel_id);
                }
                return;
            }
        };

        for (tab_name, target_url) in &sync_targets {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let _ = app_handle.emit(
                "channel-sync-progress",
                ChannelSyncProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "syncing".to_string(),
                    total_synced: total_synced_count,
                    new_synced: new_synced_count,
                    current_tab: Some(tab_name.clone()),
                    message: Some(format!("Syncing {}...", tab_name)),
                },
            );

            let mut args = vec![
                "--flat-playlist".to_string(),
                "-j".to_string(),
                "--ignore-config".to_string(),
                "--color".to_string(),
                "never".to_string(),
                "--no-warnings".to_string(),
                "--socket-timeout".to_string(),
                "20".to_string(),
                "--retries".to_string(),
                "3".to_string(),
                "--extractor-retries".to_string(),
                "2".to_string(),
            ];

            if let Some(sleep) = sleep_interval {
                if sleep > 0.0 {
                    args.push("--sleep-requests".to_string());
                    args.push(format!("{:.1}", sleep));
                }
            }

            args.extend(utils::build_js_runtime_args(&app_handle));
            args.extend(utils::build_ffmpeg_location_args(&app_handle));
            args.extend(utils::build_plugin_args(&app_handle));
            args.extend(utils::build_youtube_extractor_args());
            append_cookie_proxy_args(
                &mut args,
                cookie_file.as_deref(),
                cookie_browser.as_deref(),
                proxy.as_deref(),
            );
            args.push(target_url.clone());

            let mut cmd = tokio::process::Command::new(&ytdlp_path);
            cmd.args(&args)
                .env("PYTHONUTF8", "1")
                .env("PYTHONIOENCODING", "utf-8")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    encountered_error = Some(format!("Failed to spawn yt-dlp: {}", e));
                    break;
                }
            };

            if let Some(pid) = child.id() {
                app_handle.state::<ProcessRegistry>().register(&run_id, pid);
            }

            let stdout = child.stdout.take();
            if let Some(stdout) = stdout {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                let mut batch: Vec<ChannelVideoRecord> = Vec::new();
                let mut consecutive_existing_count = 0usize;

                while let Ok(Some(line)) = reader.next_line().await {
                    if cancel_flag.load(Ordering::Relaxed) {
                        let _ = child.kill().await;
                        break;
                    }

                    let line_trimmed = line.trim();
                    if line_trimmed.is_empty() || !line_trimmed.starts_with('{') {
                        continue;
                    }

                    let entry: Value = match serde_json::from_str(line_trimmed) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let video_id = match entry.get("id").and_then(Value::as_str) {
                        Some(id) if !id.is_empty() => id.to_string(),
                        _ => continue,
                    };

                    let title = entry
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let video_url = entry
                        .get("url")
                        .or_else(|| entry.get("webpage_url"))
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_else(|| {
                            if channel.platform == "youtube" {
                                format!("https://www.youtube.com/watch?v={}", video_id)
                            } else {
                                video_id.clone()
                            }
                        });

                    let duration = entry
                        .get("duration")
                        .and_then(Value::as_f64)
                        .filter(|d| *d > 0.0);
                    let view_count = entry.get("view_count").and_then(Value::as_i64);
                    let thumbnail = extract_video_thumbnail(&entry);

                    let published_at = entry
                        .get("timestamp")
                        .and_then(Value::as_i64)
                        .map(|s| s * 1000)
                        .or_else(|| {
                            entry
                                .get("upload_date")
                                .and_then(Value::as_str)
                                .and_then(parse_upload_date)
                        });

                    let content_type = if tab_name == "shorts" || video_url.contains("/shorts/") {
                        "short".to_string()
                    } else if tab_name == "streams"
                        || entry
                            .get("live_status")
                            .and_then(Value::as_str)
                            .map(|s| s.contains("live"))
                            .unwrap_or(false)
                    {
                        "stream".to_string()
                    } else {
                        "video".to_string()
                    };

                    let is_known = existing_ids.contains(&video_id);

                    if is_incremental {
                        if is_known {
                            consecutive_existing_count += 1;
                            // 增量模式下连续命中 5 条已有记录，判定已对齐历史数据，提前中断当前 Tab
                            if consecutive_existing_count >= 5 {
                                let _ = child.kill().await;
                                break;
                            }
                        } else {
                            consecutive_existing_count = 0;
                            new_synced_count += 1;
                            existing_ids.insert(video_id.clone());
                        }
                    } else if !is_known {
                        new_synced_count += 1;
                        existing_ids.insert(video_id.clone());
                    }

                    total_synced_count += 1;
                    let now_ms = now_millis();
                    batch.push(ChannelVideoRecord {
                        id: format!("{}_{}", channel_id, video_id),
                        channel_id: channel_id.clone(),
                        video_id,
                        url: video_url,
                        title,
                        thumbnail,
                        duration,
                        view_count,
                        published_at,
                        content_type,
                        created_at: now_ms,
                    });

                    if batch.len() >= 100 {
                        let _ = upsert_channel_videos_batch(&db_state, &batch);
                        batch.clear();
                        let _ = app_handle.emit(
                            "channel-sync-progress",
                            ChannelSyncProgressPayload {
                                channel_id: channel_id.clone(),
                                status: "syncing".to_string(),
                                total_synced: total_synced_count,
                                new_synced: new_synced_count,
                                current_tab: Some(tab_name.clone()),
                                message: None,
                            },
                        );
                    }
                }

                if !batch.is_empty() {
                    let _ = upsert_channel_videos_batch(&db_state, &batch);
                    batch.clear();
                }
            }

            app_handle.state::<ProcessRegistry>().take(&run_id);
            let _ = child.wait().await;
        }

        // 清理当前同步任务状态
        if let Ok(mut syncs) = get_active_syncs().lock() {
            syncs.remove(&channel_id);
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = update_channel_sync_status(&db_state, &channel_id, "idle", None);
            let _ = app_handle.emit(
                "channel-sync-progress",
                ChannelSyncProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "cancelled".to_string(),
                    total_synced: total_synced_count,
                    new_synced: new_synced_count,
                    current_tab: None,
                    message: Some("Sync cancelled".to_string()),
                },
            );
        } else if let Some(err) = encountered_error {
            let _ = update_channel_sync_status(&db_state, &channel_id, "error", Some(&err));
            let _ = app_handle.emit(
                "channel-sync-progress",
                ChannelSyncProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "error".to_string(),
                    total_synced: total_synced_count,
                    new_synced: new_synced_count,
                    current_tab: None,
                    message: Some(err),
                },
            );
        } else {
            let _ = update_channel_sync_status(&db_state, &channel_id, "completed", None);
            let _ = app_handle.emit(
                "channel-sync-progress",
                ChannelSyncProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "completed".to_string(),
                    total_synced: total_synced_count,
                    new_synced: new_synced_count,
                    current_tab: None,
                    message: Some(format!(
                        "Sync completed. Total: {}, New: {}",
                        total_synced_count, new_synced_count
                    )),
                },
            );
        }
    });

    Ok(())
}
