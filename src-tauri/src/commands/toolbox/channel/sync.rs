//! 频道 CRUD 与列表同步任务（flat-playlist 流式抓取）。

use super::enrich::{spawn_enrich_job, EnrichJobParams};
use super::extract::{
    clean_channel_url, detect_platform, extract_channel_avatar, extract_channel_banner,
    extract_channel_id, extract_channel_title, extract_published_at, extract_video_thumbnail,
    now_millis,
};
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
///
/// 注意：tauri 命令的签名即前端 `invoke` 的 IPC 契约，参数与前端载荷一对一映射，
/// 不得为降参数个数而合并，此处是有意为之的例外。
#[allow(clippy::too_many_arguments)]
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
        // 本轮新收录的 video_id：同步成功后自动链式校准它们的精确日期
        let mut new_video_ids: Vec<String> = Vec::new();
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
                "--extractor-args".to_string(),
                "youtubetab:approximate_date".to_string(),
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
                let mut last_emit_ms = now_millis();

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

                    // 精度标记规则：
                    // - YouTube：一律 approx。tab 渲染层没有真实日期，
                    //   youtubetab:approximate_date 只是由“N 年/月前”反推，
                    //   月日会漂到抓取当天，必须靠校准任务回填；
                    // - 其他平台：flat 条目自带真实时间戳（pubdate 等），
                    //   有值即视为 exact，只有缺失才需要后续校准。
                    //   这样 B 站等平台不会被重复逐条抓取。
                    let published_at = extract_published_at(&entry);
                    let published_accuracy = if channel.platform == "youtube" || published_at.is_none()
                    {
                        "approx"
                    } else {
                        "exact"
                    }
                    .to_string();

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
                            new_video_ids.push(video_id.clone());
                            existing_ids.insert(video_id.clone());
                        }
                    } else if !is_known {
                        new_synced_count += 1;
                        new_video_ids.push(video_id.clone());
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
                        published_accuracy,
                        content_type,
                        created_at: now_ms,
                    });

                    let should_emit = batch.len() >= 15 || (now_ms - last_emit_ms >= 300 && !batch.is_empty());
                    if should_emit {
                        let _ = upsert_channel_videos_batch(&db_state, &batch);
                        batch.clear();
                        last_emit_ms = now_ms;
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
                    message: None,
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
                    message: None,
                },
            );
            // 全自动链路：同步成功后顺手校准日期（后台任务，前端已有进度监听）。
            // 增量同步只校准本轮新增（便宜）；全量同步校准全频道未精确的行，
            // 存量数据的精确化就靠点一次全量同步完成，无需额外手动入口。
            if !is_incremental || !new_video_ids.is_empty() {
                let auto_ids: Option<&[String]> =
                    if is_incremental { Some(&new_video_ids) } else { None };
                // 已有校准任务在跑等情况：静默跳过，不打断同步完成态
                let _ = spawn_enrich_job(
                    &app_handle,
                    EnrichJobParams {
                        channel_id: channel_id.as_str(),
                        channel_platform: channel.platform.as_str(),
                        video_ids: auto_ids,
                        cookie_file,
                        cookie_browser,
                        proxy,
                        concurrency: Some(6),
                    },
                );
            }
        }
    });

    Ok(())
}

/// 查询当前正在同步的频道 ID 列表（供底栏与页面重进时恢复状态）
#[tauri::command]
pub async fn channel_sync_active() -> Result<Vec<String>, String> {
    get_active_syncs()
        .lock()
        .map(|syncs| syncs.keys().cloned().collect())
        .map_err(|e| format!("Failed to acquire lock: {}", e))
}
