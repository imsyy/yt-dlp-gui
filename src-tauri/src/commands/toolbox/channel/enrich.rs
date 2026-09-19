//! 发布日期校准任务：对精度非 exact 的归档视频回填精确日期。
//!
//! 数据流：命令/同步收尾 -> spawn_enrich_job 注册并起后台任务 ->
//! 快查阶段（YouTube watch 页直连 / B 站官方 API 直连，高并发）->
//! 兜底阶段（剩余行才走 yt-dlp 分片抓取）-> channel-enrich-progress 事件推进 ->
//! 终态 completed/error/cancelled。ACTIVE_ENRICHS 表项跨阶段持有，任务结束才释放。
//!
//! 两阶段设计的原因：yt-dlp 子进程有秒级启动开销且分片内串行，
//! 一个慢视频拖住整片；直接 HTTP 单请求 300~800ms、16 并发，
//! 实测 200 条从分钟级降到十秒级。失败分永久（私有/删除/无 Cookie 登录限制，
//! 不再重试）与瞬时（超时/限流，补查一次后进兜底），避免全量多轮空转。

use super::extract::{extract_published_at, extract_video_thumbnail, BILIBILI_USER_AGENT};
use super::fast::{run_fast, FastJobParams};
use crate::commands::support::append_cookie_proxy_args;
use crate::db::channels::{
    get_channel, get_enrich_targets, update_video_enriched_fields, EnrichTarget, EnrichUpdate,
};
use crate::db::DatabaseState;
use crate::platform::process::ProcessRegistry;
use crate::utils;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncBufReadExt;

#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;

/// 正在进行的日期校准任务控制表 (channel_id -> 取消标志)
static ACTIVE_ENRICHS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn get_active_enriches() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_ENRICHS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 日期校准进度事件载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelEnrichProgressPayload {
    pub channel_id: String,
    pub status: String, // "enriching" | "completed" | "error" | "cancelled"
    pub total: usize,
    pub done: usize,
    pub fixed: usize,
    pub message: Option<String>,
}

/// 校准分片大小：通用平台每批塞进同一个 yt-dlp 进程的 URL 数
const ENRICH_CHUNK_SIZE: usize = 20;
/// 兜底分片大小：快查剩下的问题行更小分片隔离，单个坏链最多拖住 10 个
const FALLBACK_CHUNK_SIZE: usize = 10;
/// 通用平台兜底轮数上限（含首轮）：首轮后仍有模糊行则自动重查
const ENRICH_MAX_ROUNDS: usize = 3;
/// 轮间休眠秒数，给可能的限流留喘息
const ENRICH_ROUND_INTERVAL_SECS: u64 = 3;
/// 兜底阶段并发 worker 数上限（run_id 编号与收尾清理都按此值展开）
const ENRICH_MAX_WORKERS: usize = 8;
/// 分片失败时保留的 stderr 尾部行数（用于错误采样）
const STDERR_TAIL_LINES: usize = 30;

/// 日期校准共用的 yt-dlp 参数（经实测确认）：
/// - `-j` 多 URL 单进程输出一行一个 JSON，`--ignore-errors` 让坏链不中断整批；
/// - 只做详情提取不碰媒体流，`--no-check-formats`/`--no-playlist`/
///   `--no-flat-playlist` 砍掉无关请求；
/// - 不传 deno/ffmpeg/plugin/po-token 参数，纯日期提取不需要它们。
fn base_enrich_args() -> Vec<String> {
    [
        "-j",
        "--ignore-config",
        "--color",
        "never",
        "--no-warnings",
        "--socket-timeout",
        "15",
        "--retries",
        "2",
        "--extractor-retries",
        "1",
        "--no-check-formats",
        "--no-playlist",
        "--no-flat-playlist",
        "--ignore-errors",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// YouTube 详情抓取的额外参数：
/// `player_client=android,web_embedded` 走免挑战客户端，无需 JS 运行时/PO Token
/// 即可返回 microformat 的精确 upload_date（tv 客户端实测会报
/// "The page needs to be reloaded"，不可用）；web_embedded 顺带给“允许嵌入的
/// 年龄限制视频”留一条免登录活路（实测部分视频仍需登录，见下）；
/// `player_skip=webpage` 砍掉 watch 网页请求（日期来自 player API，不受影响），
/// configs 保留以保证匿名请求可用。
///
/// 年龄限制视频说明（已用 yt-dlp 自带测试用例实测）：
/// 网页上能看到日期，是因为年龄确认页 HTML 里带了展示文本；但 yt-dlp 的日期
/// 取自 Innertube player API 的 microformat，未登录时该接口对年龄限制视频直接
/// 返回 LOGIN_REQUIRED，不带任何日期字段——这就是“看得到却抓不到”的原因。
/// 允许嵌入的视频有时能被 web_embedded 绕过；其余一律需要登录 Cookie，
/// 本任务已透传 Cookie/代理参数，在设置里配好 Cookie 即可校准这类视频。
fn build_enrich_args(urls: &[String]) -> Vec<String> {
    let mut args = base_enrich_args();
    args.push("--extractor-args".to_string());
    args.push("youtube:player_client=android,web_embedded;player_skip=webpage".to_string());
    for url in urls {
        args.push(url.clone());
    }
    args
}

/// 日期校准任务的工作单元：(行主键 id, 视频 video_id, 详情页 URL)
type EnrichItem = (String, String, String);
type EnrichChunk = Vec<EnrichItem>;

/// 单个校准 worker 顺序消费分片；放在函数外以便 tokio::spawn 'static 约束。
/// stats 为 (已处理数, 已修正数, 失败分片数)。
struct EnrichWorkerCtx {
    app_handle: AppHandle,
    run_id: String,
    ytdlp_path: std::path::PathBuf,
    queue: Arc<Mutex<VecDeque<EnrichChunk>>>,
    cancel_flag: Arc<AtomicBool>,
    stats: Arc<Mutex<(usize, usize, usize)>>,
    total: usize,
    /// 之前轮次已累计的进度（进度条跨轮连续显示，分母始终是首轮总数）
    done_base: usize,
    fixed_base: usize,
    /// 首个分片级错误采样（stderr 尾部 ERROR 行），全失败时用于可读报错
    err_sample: Arc<Mutex<Option<String>>>,
    channel_id: String,
    channel_platform: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
}

async fn enrich_worker(ctx: EnrichWorkerCtx) {
    let EnrichWorkerCtx {
        app_handle,
        run_id,
        ytdlp_path,
        queue,
        cancel_flag,
        stats,
        total,
        done_base,
        fixed_base,
        err_sample,
        channel_id,
        channel_platform,
        cookie_file,
        cookie_browser,
        proxy,
    } = ctx;
    // 非 YouTube 站点没有 player_client 概念，用通用参数回退（仍比重抓整个同步快，
    // 因为是按需逐视频而非全部分区扫描）。
    let is_youtube = channel_platform == "youtube";
    loop {
        let chunk: Option<EnrichChunk> = queue.lock().map(|mut q| q.pop_front()).unwrap_or(None);
        let Some(chunk) = chunk else { break };
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let mut args = if is_youtube {
            build_enrich_args(
                &chunk
                    .iter()
                    .map(|(_, _, url)| url.clone())
                    .collect::<Vec<_>>(),
            )
        } else {
            // 非 YouTube 站点没有 player_client 概念，用共用参数即可
            let mut generic = base_enrich_args();
            for (_, _, url) in &chunk {
                generic.push(url.clone());
            }
            // B 站风控与其他平台不同，仅该平台附加浏览器 UA
            if channel_platform == "bilibili" {
                generic.push("--user-agent".to_string());
                generic.push(BILIBILI_USER_AGENT.to_string());
            }
            generic
        };
        append_cookie_proxy_args(
            &mut args,
            cookie_file.as_deref(),
            cookie_browser.as_deref(),
            proxy.as_deref(),
        );

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
            Err(_) => {
                // 本分片启动失败：整批计为已处理但未修正，避免卡死进度
                let (done, fixed, _) = match stats.lock() {
                    Ok(mut s) => {
                        s.0 += chunk.len();
                        s.2 += 1;
                        *s
                    }
                    Err(_) => (chunk.len(), 0, 1),
                };
                let _ = app_handle.emit(
                    "channel-enrich-progress",
                    ChannelEnrichProgressPayload {
                        channel_id: channel_id.clone(),
                        status: "enriching".to_string(),
                        total,
                        done: (done_base + done).min(total),
                        fixed: fixed_base + fixed,
                        message: None,
                    },
                );
                continue;
            }
        };

        if let Some(pid) = child.id() {
            app_handle.state::<ProcessRegistry>().register(&run_id, pid);
        }

        // stderr 必须与 stdout 同时消费：整批硬失败（20 条全带 traceback）时输出
        // 会超过管道缓冲区，只读 stdout 会让子进程卡在写、wait() 永不返回。
        // --no-warnings 下输出量小，只留尾部 STDERR_TAIL_LINES 行做错误采样。
        let stderr_task = child.stderr.take().map(|stderr| {
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stderr).lines();
                let mut tail: Vec<String> = Vec::new();
                while let Ok(Some(line)) = reader.next_line().await {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.remove(0);
                    }
                    tail.push(trimmed.to_string());
                }
                tail
            })
        });

        // video_id -> 行主键 id，用于把逐行 JSON 结果映射回数据库行
        let id_map: HashMap<&str, &str> = chunk
            .iter()
            .map(|(id, video_id, _)| (video_id.as_str(), id.as_str()))
            .collect();
        // 详情 JSON 里除了日期，标题/时长/播放量/封面也比 flat 条目全，顺手一起回填
        let mut updates: Vec<EnrichUpdate> = Vec::new();

        if let Some(stdout) = child.stdout.take() {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
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
                let Some(video_id) = entry.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let (Some(row_id), Some(published_at)) =
                    (id_map.get(video_id), extract_published_at(&entry))
                else {
                    continue;
                };
                updates.push(EnrichUpdate {
                    id: row_id.to_string(),
                    published_at,
                    title: entry
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    thumbnail: extract_video_thumbnail(&entry),
                    duration: entry
                        .get("duration")
                        .and_then(Value::as_f64)
                        .filter(|d| *d > 0.0),
                    view_count: entry.get("view_count").and_then(Value::as_i64),
                });
            }
        }

        app_handle.state::<ProcessRegistry>().take(&run_id);
        let exit_ok = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let stderr_tail = match stderr_task {
            Some(task) => task.await.unwrap_or_default(),
            None => Vec::new(),
        };

        // 分片级失败时用 stderr 尾部错误行做可读原因
        if !exit_ok {
            let sample = stderr_tail
                .iter()
                .rev()
                .find(|l| l.contains("ERROR"))
                .or_else(|| stderr_tail.last())
                .cloned();
            if let Some(sample) = sample {
                if let Ok(mut slot) = err_sample.lock() {
                    if slot.is_none() {
                        *slot = Some(sample);
                    }
                }
            }
        }

        let mut fixed_now = 0;
        if !updates.is_empty() && !cancel_flag.load(Ordering::Relaxed) {
            let db_state = app_handle.state::<DatabaseState>();
            if let Ok(n) = update_video_enriched_fields(&db_state, &updates) {
                fixed_now = n;
            }
        }

        let (done, fixed, _) = match stats.lock() {
            Ok(mut s) => {
                s.0 += chunk.len();
                s.1 += fixed_now;
                if !exit_ok {
                    s.2 += 1;
                }
                *s
            }
            Err(_) => (chunk.len(), fixed_now, usize::from(!exit_ok)),
        };
        let _ = app_handle.emit(
            "channel-enrich-progress",
            ChannelEnrichProgressPayload {
                channel_id: channel_id.clone(),
                status: "enriching".to_string(),
                total,
                done: (done_base + done).min(total),
                fixed: fixed_base + fixed,
                message: None,
            },
        );
    }
}

/// spawn_enrich_job 的参数束：把 8 个参数收拢，调用点只构造一次。
pub(crate) struct EnrichJobParams<'a> {
    pub channel_id: &'a str,
    pub channel_platform: &'a str,
    pub video_ids: Option<&'a [String]>,
    pub cookie_file: Option<String>,
    pub cookie_browser: Option<String>,
    pub proxy: Option<String>,
    pub concurrency: Option<usize>,
}

/// 启动发布日期校准：对精度非 exact 的归档视频逐个抓取详情并回填精确日期。
/// 返回需要校准的视频总数（0 表示无需校准）。与 channel_sync_start 相互独立，
/// 共用同一张表的写入由 upsert 的精度保护语义保证安全。
/// 调用方：同步成功收尾的自动链式调用，以及手动查漏补缺命令。
pub(crate) fn spawn_enrich_job(app: &AppHandle, params: EnrichJobParams) -> Result<usize, String> {
    let EnrichJobParams {
        channel_id,
        channel_platform,
        video_ids,
        cookie_file,
        cookie_browser,
        proxy,
        concurrency,
    } = params;
    let db = app.state::<DatabaseState>();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut enriches = get_active_enriches()
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        if enriches.contains_key(channel_id) {
            return Err("err_channel_already_syncing".to_string());
        }
        enriches.insert(channel_id.to_string(), cancel_flag.clone());
    }

    let targets = match get_enrich_targets(&db, channel_id, video_ids) {
        Ok(t) => t,
        Err(e) => {
            if let Ok(mut enriches) = get_active_enriches().lock() {
                enriches.remove(channel_id);
            }
            return Err(e);
        }
    };
    if targets.is_empty() {
        if let Ok(mut enriches) = get_active_enriches().lock() {
            enriches.remove(channel_id);
        }
        return Ok(0);
    }
    let total = targets.len();

    // 后续要 move 进 'static 后台任务，转为 owned
    let channel_id = channel_id.to_string();
    let channel_platform = channel_platform.to_string();
    let video_ids_owned: Option<Vec<String>> = video_ids.map(|ids| ids.to_vec());

    let app_handle = app.clone();
    tokio::spawn(async move {
        let db_state = app_handle.state::<DatabaseState>();
        let err_sample = Arc::new(Mutex::new(None::<String>));
        // 启动即推送：首个结果回来前（快查约 1s 内就有）前端也能立刻转圈+禁用按钮，
        // 与同步任务“点下就有反馈”的体验对齐
        let _ = app_handle.emit(
            "channel-enrich-progress",
            ChannelEnrichProgressPayload {
                channel_id: channel_id.clone(),
                status: "enriching".to_string(),
                total,
                done: 0,
                fixed: 0,
                message: None,
            },
        );
        // 两阶段：快查（直接 HTTP，高并发）-> 兜底（仅剩行走 yt-dlp 单轮）。
        // 通用平台无快查路径，走原多轮 yt-dlp。
        // ACTIVE_ENRICHS 表项跨阶段持有，期间手动再点会被拒绝，前端据此禁用菜单项。
        let mut acc_done = 0usize;
        let mut acc_fixed = 0usize;
        let mut acc_permanent = 0usize;
        let (mut last_failed, mut last_chunks) = (0usize, 0usize);
        let mut first_targets: Option<Vec<EnrichTarget>> = None;
        let mut max_rounds = ENRICH_MAX_ROUNDS;
        let mut chunk_size = ENRICH_CHUNK_SIZE;
        let has_cookie = cookie_file
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            || cookie_browser
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

        if channel_platform == "youtube" || channel_platform == "bilibili" {
            let fast_inputs: Vec<(String, String, String)> = targets
                .into_iter()
                .map(|t| (t.id, t.video_id, t.url))
                .collect();
            // 客户端构造失败（如代理配错）才用得上：快查一行没跑就直接全量兜底
            let fallback_inputs = fast_inputs.clone();
            let fast_outcome = run_fast(
                &app_handle,
                FastJobParams {
                    channel_id: &channel_id,
                    kind: if channel_platform == "youtube" {
                        "youtube"
                    } else {
                        "bilibili"
                    },
                    targets: fast_inputs,
                    proxy: proxy.clone(),
                    has_cookie,
                    concurrency,
                    total,
                    cancel_flag: cancel_flag.clone(),
                },
            )
            .await;
            match fast_outcome {
                Ok(o) => {
                    acc_done = o.done;
                    acc_fixed = o.fixed;
                    acc_permanent = o.permanent;
                    if let Some(sample) = o.err_sample {
                        if let Ok(mut slot) = err_sample.lock() {
                            *slot = Some(sample);
                        }
                    }
                    let retry_count = o.retry_targets.len();
                    if retry_count > 0 && !cancel_flag.load(Ordering::Relaxed) {
                        // 兜底行在快查阶段已计过一次 done，这里扣掉：
                        // 否则兜底还没跑，进度就已经顶到 total/total。
                        acc_done = acc_done.saturating_sub(retry_count);
                        first_targets = Some(
                            o.retry_targets
                                .into_iter()
                                .map(|(id, video_id, url)| EnrichTarget { id, video_id, url })
                                .collect(),
                        );
                    }
                }
                Err(e) => {
                    if let Ok(mut slot) = err_sample.lock() {
                        *slot = Some(e);
                    }
                    if !cancel_flag.load(Ordering::Relaxed) {
                        first_targets = Some(
                            fallback_inputs
                                .into_iter()
                                .map(|(id, video_id, url)| EnrichTarget { id, video_id, url })
                                .collect(),
                        );
                    }
                }
            }
            // 快查已覆盖绝大多数行：兜底最多一轮，且用更小分片隔离问题行
            max_rounds = 1;
            chunk_size = FALLBACK_CHUNK_SIZE;
        } else {
            first_targets = Some(targets);
        }
        let mut round = 0usize;
        // 兜底阶段：需要 yt-dlp 且确有剩余行时才起进程；
        // 快查已全搞定（或已取消）则跳过，yt-dlp 缺失也不影响快查成果。
        let need_fallback = first_targets
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
            && !cancel_flag.load(Ordering::Relaxed);
        let ytdlp_path = if need_fallback {
            match utils::get_ytdlp_path(&app_handle) {
                Ok(p) => Some(p),
                Err(e) => {
                    if acc_fixed == 0 && acc_permanent == 0 {
                        if let Ok(mut enriches) = get_active_enriches().lock() {
                            enriches.remove(&channel_id);
                        }
                        let _ = app_handle.emit(
                            "channel-enrich-progress",
                            ChannelEnrichProgressPayload {
                                channel_id: channel_id.clone(),
                                status: "error".to_string(),
                                total,
                                done: acc_done.min(total),
                                fixed: 0,
                                message: Some(e),
                            },
                        );
                        return;
                    }
                    // 快查已有成果：yt-dlp 缺失只影响剩余行，按完成收尾
                    None
                }
            }
        } else {
            None
        };
        while round < max_rounds {
            let Some(ytdlp_path) = ytdlp_path.clone() else {
                break;
            };
            round += 1;
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            let round_targets: Vec<EnrichTarget> = match first_targets.take() {
                Some(t) => t,
                None => {
                    match get_enrich_targets(&db_state, &channel_id, video_ids_owned.as_deref()) {
                        Ok(t) if !t.is_empty() => t,
                        _ => break,
                    }
                }
            };

            // 每批 chunk_size 个 URL 塞进同一个 yt-dlp 进程，摊薄启动开销
            let queue = Arc::new(Mutex::new(
                round_targets
                    .into_iter()
                    .map(|t| (t.id, t.video_id, t.url))
                    .collect::<Vec<EnrichItem>>()
                    .chunks(chunk_size)
                    .map(|c| c.to_vec())
                    .collect::<VecDeque<EnrichChunk>>(),
            ));
            let chunk_count = queue.lock().map(|q| q.len()).unwrap_or(0);
            let worker_count = concurrency
                .unwrap_or(4)
                .clamp(1, ENRICH_MAX_WORKERS)
                .min(chunk_count.max(1));
            let stats = Arc::new(Mutex::new((0usize, 0usize, 0usize)));
            let mut handles = Vec::with_capacity(worker_count);
            for idx in 0..worker_count {
                let run_id = format!("channel_enrich_{}_{}", channel_id, idx);
                handles.push(tokio::spawn(enrich_worker(EnrichWorkerCtx {
                    app_handle: app_handle.clone(),
                    run_id,
                    ytdlp_path: ytdlp_path.clone(),
                    queue: queue.clone(),
                    cancel_flag: cancel_flag.clone(),
                    stats: stats.clone(),
                    total,
                    done_base: acc_done,
                    fixed_base: acc_fixed,
                    err_sample: err_sample.clone(),
                    channel_id: channel_id.clone(),
                    channel_platform: channel_platform.clone(),
                    cookie_file: cookie_file.clone(),
                    cookie_browser: cookie_browser.clone(),
                    proxy: proxy.clone(),
                })));
            }
            for handle in handles {
                let _ = handle.await;
            }

            let (done, fixed, failed) = stats.lock().map(|s| *s).unwrap_or((0, 0, 0));
            acc_done += done;
            acc_fixed += fixed;
            last_failed = failed;
            last_chunks = chunk_count;
            if fixed == 0 {
                // 本轮无进展：剩余基本是私有/删除等永久失败，重试无意义，直接停
                break;
            }
            if get_enrich_targets(&db_state, &channel_id, video_ids_owned.as_deref())
                .map(|v| v.len())
                .unwrap_or(0)
                == 0
            {
                break;
            }
            if round >= max_rounds {
                break;
            }
            // 轮间休眠，给可能的限流留喘息；取消会被下一轮顶部的检查捕获
            tokio::time::sleep(std::time::Duration::from_secs(ENRICH_ROUND_INTERVAL_SECS)).await;
        }

        if let Ok(mut enriches) = get_active_enriches().lock() {
            enriches.remove(&channel_id);
        }
        // 取消时顺手清理可能残留的 pid 登记（worker 正常退出时已 take）
        for idx in 0..ENRICH_MAX_WORKERS {
            app_handle
                .state::<ProcessRegistry>()
                .take(&format!("channel_enrich_{}_{}", channel_id, idx));
        }

        let done = acc_done.min(total);
        let fixed = acc_fixed;
        let failed_chunks = last_failed;
        let chunk_count = last_chunks;
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = app_handle.emit(
                "channel-enrich-progress",
                ChannelEnrichProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "cancelled".to_string(),
                    total,
                    done,
                    fixed,
                    message: None,
                },
            );
        } else if fixed == 0 && acc_permanent == 0 && failed_chunks >= chunk_count.max(1) {
            // 网络级全失败（如断网/代理配错）且无任何永久跳过行：
            // 优先用 stderr/HTTP 采样出的真实原因，
            // 兜底才是通用错误码（前端会对两者做友好化翻译）。
            // 注意：fixed==0 但有永久跳过行（全是私有/删除）是正常完成，不是错误。
            let detail = err_sample
                .lock()
                .ok()
                .and_then(|slot| slot.clone())
                .unwrap_or_else(|| "err_enrich_all_chunks_failed".to_string());
            let _ = app_handle.emit(
                "channel-enrich-progress",
                ChannelEnrichProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "error".to_string(),
                    total,
                    done,
                    fixed,
                    message: Some(detail),
                },
            );
        } else {
            let _ = app_handle.emit(
                "channel-enrich-progress",
                ChannelEnrichProgressPayload {
                    channel_id: channel_id.clone(),
                    status: "completed".to_string(),
                    total,
                    done,
                    fixed,
                    message: None,
                },
            );
        }
    });

    Ok(total)
}

/// 手动查漏补缺：重新抓取本频道所有仍是模糊日期的视频（同步收尾的自动链路只跑一遍，
/// 私有/删除/网络抖动导致失败的行会留下，需用户手动再跑一次）。前端放在同步下拉菜单里。
#[tauri::command]
pub async fn channel_enrich_start(
    app: AppHandle,
    channel_id: String,
    video_ids: Option<Vec<String>>,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
    concurrency: Option<usize>,
) -> Result<usize, String> {
    let db = app.state::<DatabaseState>();
    let channel = get_channel(&db, &channel_id)?
        .ok_or_else(|| format!("err_channel_not_found:{}", channel_id))?;
    spawn_enrich_job(
        &app,
        EnrichJobParams {
            channel_id: channel_id.as_str(),
            channel_platform: channel.platform.as_str(),
            video_ids: video_ids.as_deref(),
            cookie_file,
            cookie_browser,
            proxy,
            concurrency,
        },
    )
}

/// 取消指定频道的日期校准任务（最终 cancelled 事件由后台任务收尾时发出）
#[tauri::command]
pub async fn channel_enrich_cancel(app: AppHandle, channel_id: String) -> Result<(), String> {
    if let Ok(mut enriches) = get_active_enriches().lock() {
        if let Some(flag) = enriches.remove(&channel_id) {
            flag.store(true, Ordering::SeqCst);
        } else {
            return Ok(());
        }
    }
    for idx in 0..ENRICH_MAX_WORKERS {
        let run_id = format!("channel_enrich_{}_{}", channel_id, idx);
        if let Some(pid) = app.state::<ProcessRegistry>().take(&run_id) {
            let _ = crate::platform::process::kill_process(pid);
        }
    }
    Ok(())
}

/// 查询当前正在日期校准的频道 ID 列表（供底栏与页面重进时恢复状态）
#[tauri::command]
pub async fn channel_enrich_active() -> Result<Vec<String>, String> {
    get_active_enriches()
        .lock()
        .map(|enriches| enriches.keys().cloned().collect())
        .map_err(|e| format!("Failed to acquire lock: {}", e))
}
