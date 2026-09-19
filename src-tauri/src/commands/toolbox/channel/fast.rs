//! 详情快查：绕开 yt-dlp 子进程开销的直接 HTTP 抓取。
//!
//! 背景：原 enrich 任务把 N 个 URL 分片塞进 yt-dlp 进程逐个串行抓取，
//! 每个进程有 Python 启动开销（约 0.5~2s），一个慢视频会拖住整片 20 个，
//! 且无日期的行被静默丢弃、无永久/瞬时失败区分，导致“慢 + 缺失多”。
//!
//! 本模块提供两条平台专属快查路径：
//! - YouTube：直抓 watch 页 HTML（gzip 后约 150KB），读
//!   `<meta itemprop="datePublished">` 精确时间戳。注意：不要改用 Innertube
//!   `player` 接口——实测 WEB_EMBEDDED_PLAYER 会被
//!   `EMBEDDER_IDENTITY_DENIED` 全量拒绝（0/615），而 ANDROID/IOS 客户端的
//!   响应里根本不带 microformat 日期；watch 页是唯一兼具“免登录可达 +
//!   精确日期”的轻量路径（与 yt-dlp 的 player 路径数据源相同，都是
//!   ytInitialPlayerResponse/microformat，只是换了个更少被限流的入口）；
//! - B 站：直调官方 `x/web-interface/view` 接口读 `pubdate`，
//!   单请求约 200ms，比 yt-dlp 解析整页快一个数量级。
//!
//! 失败分两类：永久失败（私有/删除/无 Cookie 的登录限制）不再重试；
//! 瞬时失败（超时/429/5xx/机器人验证）最多重试一次，仍失败的才交给
//! enrich.rs 的 yt-dlp 兜底轮（且仅兜底这些行，而非全量重扫）。

use super::extract::{civil_days, BILIBILI_USER_AGENT};
use crate::db::channels::EnrichUpdate;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// YouTube 详情页：用 video_id 拼 canonical URL，避免存量脏 URL（/shorts/ 等）影响抓取。
const YT_WATCH_URL: &str = "https://www.youtube.com/watch?v=";
const YT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const YT_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

const BILI_VIEW_URL: &str = "https://api.bilibili.com/x/web-interface/view";

const FAST_TIMEOUT_SECS: u64 = 15;
/// 瞬时失败最多补查一次，避免风控下越重试越慢。
const FAST_MAX_ATTEMPTS: usize = 2;
/// DB 批量回填阈值：攒够一批再写，减少锁竞争。
const FAST_DB_FLUSH_SIZE: usize = 20;

/// 单视频抓取结果：调用方据此计数、回填、决策是否兜底。
pub(crate) enum FetchOutcome {
    Fixed(EnrichUpdate),
    /// 私有/删除/无 Cookie 登录限制等，重试无意义
    Permanent,
    /// 超时/限流/服务端错误，可兜底或重试
    Transient(String),
}

/// 快查阶段汇总：供 enrich.rs 决策兜底与终态。
pub(crate) struct FastOutcome {
    pub done: usize,
    pub fixed: usize,
    pub permanent: usize,
    /// 需要 yt-dlp 兜底的行（行主键 id, video_id, url）
    pub retry_targets: Vec<(String, String, String)>,
    /// 首个瞬时错误采样，全失败时用于可读报错
    pub err_sample: Option<String>,
}

/// "YYYY-MM-DD"（内嵌 JSON 的 uploadDate）-> 毫秒时间戳（UTC 零点）。
pub(crate) fn parse_yt_date_ymd(date: &str) -> Option<i64> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    // 用 get 而非直接切片：脏输入含多字节字符时直接返回 None，不 panic
    let year: i32 = date.get(0..4)?.parse().ok()?;
    let month: u32 = date.get(5..7)?.parse().ok()?;
    let day: u32 = date.get(8..10)?.parse().ok()?;
    civil_days(year, month, day).map(|days| days * 86400 * 1000)
}

/// B 站 video_id 分类：BV 号走 bvid 参数，纯数字走 aid 参数。
pub(crate) fn bili_query_param(video_id: &str) -> (&'static str, &str) {
    if video_id.starts_with("BV") || video_id.starts_with("bv") {
        ("bvid", video_id)
    } else if !video_id.is_empty() && video_id.bytes().all(|b| b.is_ascii_digit()) {
        ("aid", video_id)
    } else {
        // 历史脏数据兜底：按 bvid 试一次，不行即永久失败
        ("bvid", video_id)
    }
}

fn build_fast_client(proxy: Option<&str>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(FAST_TIMEOUT_SECS));
    if let Some(p) = proxy {
        if !p.trim().is_empty() {
            let reqwest_proxy =
                reqwest::Proxy::all(p).map_err(|e| format!("err_proxy_config:{}", e))?;
            builder = builder.proxy(reqwest_proxy);
        }
    }
    builder
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))
}

/// 从 HTML 里截取 `pat` 之后、`end` 之前的第一段文本（watch 页是压缩单行，`find` 足够）。
fn html_capture<'a>(html: &'a str, pat: &str, end: char) -> Option<&'a str> {
    let start = html.find(pat)? + pat.len();
    let rest = html.get(start..)?;
    let len = rest.find(end)?;
    rest.get(..len)
}

/// `<meta ... content="...">` 的 HTML 转义还原（标题里常见 &amp; &quot; 等）。
fn html_unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(idx) = rest.find('&') {
        out.push_str(&rest[..idx]);
        rest = &rest[idx..];
        let (entity, next) = if let Some(semi) = rest.find(';') {
            (&rest[..semi + 1], &rest[semi + 1..])
        } else {
            out.push_str(rest);
            rest = "";
            continue;
        };
        match entity {
            "&amp;" => out.push('&'),
            "&quot;" => out.push('"'),
            "&#39;" | "&apos;" => out.push('\''),
            "&lt;" => out.push('<'),
            "&gt;" => out.push('>'),
            _ if entity.starts_with("&#") => {
                let num = entity[2..entity.len() - 1].trim_start_matches('x');
                let code = if entity[2..].starts_with('x') {
                    u32::from_str_radix(num, 16).ok()
                } else {
                    num.parse::<u32>().ok()
                };
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => out.push_str(entity),
                }
            }
            _ => out.push_str(entity),
        }
        rest = next;
    }
    out.push_str(rest);
    out
}

/// 解析 `2009-10-24T23:57:33-07:00` / `...Z` / `YYYY-MM-DD` 为毫秒时间戳。
/// datePublished meta 用的是带时区的完整 ISO 时间，比纯日期更精确。
fn parse_iso_datetime(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 10 {
        return None;
    }
    let date_part = s.get(..10)?;
    let bytes = date_part.as_bytes();
    // 分隔符不对（非 YYYY-MM-DD）直接判负；月/日合法性交给 civil_days
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i32 = date_part.get(0..4)?.parse().ok()?;
    let month: u32 = date_part.get(5..7)?.parse().ok()?;
    let day: u32 = date_part.get(8..10)?.parse().ok()?;
    let mut millis = civil_days(year, month, day)? * 86400 * 1000;
    let rest = s.get(10..).unwrap_or("").trim();
    if rest.is_empty() {
        return Some(millis);
    }
    if !rest.starts_with('T') {
        return None;
    }
    // 时间部分：HH:MM:SS[.frac][Z|±HH:MM|±HHMM]
    let time_end = rest.find(['Z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = (rest[1..time_end].trim(), rest[time_end..].trim());
    let mut parts = clock.split(':');
    let hh: i64 = parts.next()?.parse().ok()?;
    let mm: i64 = parts.next()?.parse().ok()?;
    let ss_frac = parts.next()?;
    let ss: i64 = ss_frac.split('.').next()?.parse().ok()?;
    if !(0..24).contains(&hh) || !(0..60).contains(&mm) || !(0..61).contains(&ss) {
        return None;
    }
    millis += (hh * 3600 + mm * 60 + ss) * 1000;
    if zone.is_empty() || zone == "Z" {
        return Some(millis);
    }
    let sign = match zone.as_bytes().first()? {
        b'+' => 1i64,
        b'-' => -1i64,
        _ => return None,
    };
    let digits: String = zone[1..].chars().filter(|c| *c != ':').collect();
    if digits.len() != 4 {
        return None;
    }
    let zh: i64 = digits[0..2].parse().ok()?;
    let zm: i64 = digits[2..4].parse().ok()?;
    if zh > 14 || zm > 59 {
        return None;
    }
    Some(millis - sign * (zh * 3600 + zm * 60) * 1000)
}

/// 解析 watch 页 HTML 为待回填的行更新。
/// 错误前缀决定去向：`permanent:` 不再重试；`transient:` 进 yt-dlp 兜底。
fn parse_youtube_watch(row_id: &str, html: &str) -> Result<EnrichUpdate, String> {
    // 1) 精确日期：优先 datePublished meta（带时区），退化到内嵌 JSON 的 uploadDate/publishDate
    let mut published_at: Option<i64> = None;
    for pat in [
        r#"itemprop="datePublished" content=""#,
        r#"itemprop="uploadDate" content=""#,
    ] {
        if let Some(raw) = html_capture(html, pat, '"') {
            if let Some(ts) = parse_iso_datetime(raw) {
                published_at = Some(ts);
                break;
            }
        }
    }
    if published_at.is_none() {
        for pat in [r#""uploadDate":""#, r#""publishDate":""#] {
            if let Some(raw) = html_capture(html, pat, '"') {
                if let Some(ts) = parse_yt_date_ymd(raw) {
                    published_at = Some(ts);
                    break;
                }
            }
        }
    }
    let Some(published_at) = published_at else {
        // 无日期：先看是不是明确救不回的，再看是不是登录墙/机器人验证
        if html.contains(r#""status":"PRIVATE""#) || html.contains("This video is private") {
            return Err("permanent:PRIVATE".to_string());
        }
        if html.contains("LOGIN_REQUIRED") || html.contains("Sign in to confirm your age") {
            return Err("transient:LOGIN_REQUIRED:age_gate".to_string());
        }
        if !html.contains("ytInitialPlayerResponse")
            && (html.contains("not a bot") || html.contains("Sign in to confirm"))
        {
            return Err("transient:bot_check".to_string());
        }
        return Err("transient:no_upload_date".to_string());
    };
    // 2) 标题：og:title / title meta（HTML 转义需还原）；封面沿用 flat 存量，不在这里覆写
    let title = html_capture(html, r#"<meta name="title" content=""#, '"')
        .or_else(|| html_capture(html, r#"<meta property="og:title" content=""#, '"'))
        .map(html_unescape)
        .unwrap_or_default();
    // 3) 时长 / 播放量：内嵌 player JSON 里的字符串数字
    let duration = html_capture(html, r#""lengthSeconds":""#, '"')
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| *d > 0.0);
    let view_count =
        html_capture(html, r#""viewCount":""#, '"').and_then(|s| s.parse::<i64>().ok());
    Ok(EnrichUpdate {
        id: row_id.to_string(),
        published_at,
        title,
        // 封面沿用 flat 存量（DB 侧对空串不覆盖）
        thumbnail: String::new(),
        duration,
        view_count,
    })
}

async fn fetch_youtube_one(client: &reqwest::Client, row_id: &str, video_id: &str) -> FetchOutcome {
    // 用 video_id 拼 canonical watch URL：存量 url 可能是 /shorts/ 等形态，统一入口更稳
    let url = format!("{}{}", YT_WATCH_URL, video_id);
    let mut last_err = String::new();
    for _ in 0..FAST_MAX_ATTEMPTS {
        let resp = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, YT_USER_AGENT)
            .header(reqwest::header::ACCEPT_LANGUAGE, YT_ACCEPT_LANGUAGE)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("transient:http:{}", e);
                if e.is_timeout() || e.is_connect() {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    continue;
                }
                break;
            }
        };
        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            last_err = format!("transient:status:{}", status);
            tokio::time::sleep(Duration::from_millis(1000)).await;
            continue;
        }
        // 404/410：ID 非法或已彻底下架，重试无意义；403 大多是机器人验证/地区限制，
        // 留给 yt-dlp 兜底（它走另一套客户端 + 可带 Cookie/代理）
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
            return FetchOutcome::Permanent;
        }
        let html = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                last_err = format!("transient:decode:{}", e);
                break;
            }
        };
        match parse_youtube_watch(row_id, &html) {
            Ok(update) => return FetchOutcome::Fixed(update),
            Err(e) => {
                if e.starts_with("permanent:") {
                    return FetchOutcome::Permanent;
                }
                last_err = e;
                // bot_check 值得立刻补查一次（可能是偶发验证）；登录墙/缺日期直接进兜底
                if last_err.contains("bot_check") {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
                break;
            }
        }
    }
    if last_err.is_empty() {
        last_err = "transient:unknown".to_string();
    }
    FetchOutcome::Transient(last_err)
}

/// 解析 B 站 view 接口响应为待回填的行更新。
fn parse_bilibili_view(row_id: &str, resp: &Value) -> Result<EnrichUpdate, String> {
    let code = resp.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 0 {
        let msg = resp
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        // -404/62002/62004：稿件不可见（删除/私有），重试无意义
        if code == -404 || code == 62002 || code == 62004 || code == 404 {
            return Err(format!("permanent:bili:{}:{}", code, msg));
        }
        return Err(format!("transient:bili:{}:{}", code, msg));
    }
    let data = resp
        .get("data")
        .ok_or("transient:bili:empty_data".to_string())?;
    let pubdate = data
        .get("pubdate")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .filter(|t| *t > 0)
        .ok_or("transient:bili:no_pubdate".to_string())?;
    let title = data
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let thumbnail = data
        .get("pic")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let duration = data
        .get("duration")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .map(|s| s as f64)
        .filter(|d| *d > 0.0);
    let view_count = data
        .get("stat")
        .and_then(|s| s.get("view"))
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
    Ok(EnrichUpdate {
        id: row_id.to_string(),
        published_at: pubdate * 1000,
        title,
        thumbnail,
        duration,
        view_count,
    })
}

async fn fetch_bilibili_one(
    client: &reqwest::Client,
    row_id: &str,
    video_id: &str,
) -> FetchOutcome {
    let (key, val) = bili_query_param(video_id);
    let url = format!("{}?{}={}", BILI_VIEW_URL, key, val);
    let mut last_err = String::new();
    for _ in 0..FAST_MAX_ATTEMPTS {
        let resp = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, BILIBILI_USER_AGENT)
            .header(reqwest::header::REFERER, "https://www.bilibili.com/")
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("transient:http:{}", e);
                tokio::time::sleep(Duration::from_millis(800)).await;
                continue;
            }
        };
        // 412 风控：短暂退避后补查一次
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
            || resp.status().is_server_error()
            || resp.status().as_u16() == 412
        {
            last_err = format!("transient:status:{}", resp.status());
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("transient:decode:{}", e);
                break;
            }
        };
        match parse_bilibili_view(row_id, &body) {
            Ok(update) => return FetchOutcome::Fixed(update),
            Err(e) => {
                if e.starts_with("permanent:") {
                    return FetchOutcome::Permanent;
                }
                last_err = e;
                // -412 类瞬时错误值得再试一次，其他业务错误直接出
                if last_err.contains("412") {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break;
            }
        }
    }
    if last_err.is_empty() {
        last_err = "transient:unknown".to_string();
    }
    FetchOutcome::Transient(last_err)
}

/// 快查任务参数束：收拢调用点的一长串参数（与 enrich.rs 的 EnrichJobParams 同风格）。
pub(crate) struct FastJobParams<'a> {
    pub channel_id: &'a str,
    /// "youtube" | "bilibili"：决定抓取实现、默认并发与登录限制的处理方式
    pub kind: &'static str,
    /// (行主键 id, video_id, url)
    pub targets: Vec<(String, String, String)>,
    pub proxy: Option<String>,
    pub has_cookie: bool,
    pub concurrency: Option<usize>,
    pub total: usize,
    pub cancel_flag: Arc<AtomicBool>,
}

/// 快查编排：高并发逐视频抓取、批量回填、节流上报进度。
/// has_cookie 为 true 时，YouTube 的 LOGIN_REQUIRED 会进入 retry_targets
/// （yt-dlp 兜底轮可用 Cookie 救回）；否则直接计为永久跳过。
pub(crate) async fn run_fast(
    app: &AppHandle,
    params: FastJobParams<'_>,
) -> Result<FastOutcome, String> {
    use crate::db::channels::update_video_enriched_fields;
    use crate::db::DatabaseState;

    let FastJobParams {
        channel_id,
        kind,
        targets,
        proxy,
        has_cookie,
        concurrency,
        total,
        cancel_flag,
    } = params;
    let is_youtube = kind == "youtube";
    // YouTube 详情页请求较重：用户传参按 2 倍放大后夹在 [8,16]；
    // B 站官方接口轻量，并发 10 即可跑满且不易触发 412。
    let workers = if is_youtube {
        concurrency.map(|c| (c * 2).clamp(8, 16)).unwrap_or(12)
    } else {
        concurrency.map(|c| c.clamp(4, 12)).unwrap_or(10)
    };
    let client = build_fast_client(proxy.as_deref())?;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(workers));
    let mut join_set = tokio::task::JoinSet::new();
    for (row_id, video_id, url) in targets {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        let permit_owner = semaphore.clone();
        let client = client.clone();
        let row_id_c = row_id.clone();
        let video_id_c = video_id.clone();
        join_set.spawn(async move {
            let _permit = match permit_owner.acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    return (
                        row_id_c,
                        video_id_c,
                        url,
                        FetchOutcome::Transient(format!("transient:semaphore:{}", e)),
                    );
                }
            };
            let outcome = if kind == "youtube" {
                fetch_youtube_one(&client, &row_id_c, &video_id_c).await
            } else {
                fetch_bilibili_one(&client, &row_id_c, &video_id_c).await
            };
            (row_id_c, video_id_c, url, outcome)
        });
    }

    let db_state = app.state::<DatabaseState>();
    let mut done = 0usize;
    let mut fixed = 0usize;
    let mut permanent = 0usize;
    let mut retry_targets = Vec::new();
    let mut err_sample: Option<String> = None;
    let mut pending: Vec<EnrichUpdate> = Vec::new();
    let mut last_emit = tokio::time::Instant::now();

    while let Some(joined) = join_set.join_next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            join_set.abort_all();
            break;
        }
        let (row_id, video_id, url, outcome) = match joined {
            Ok(v) => v,
            Err(e) => {
                // 任务 panic（理论上不可达：任务内无 unwrap）：计为已处理但无法定位，
                // 只留错误采样，不进兜底（无法确定是哪一行）。
                done += 1;
                if err_sample.is_none() {
                    err_sample = Some(format!("transient:join:{}", e));
                }
                continue;
            }
        };
        done += 1;
        match outcome {
            FetchOutcome::Fixed(update) => {
                fixed += 1;
                pending.push(update);
                if pending.len() >= FAST_DB_FLUSH_SIZE {
                    let batch = std::mem::take(&mut pending);
                    let _ = update_video_enriched_fields(&db_state, &batch);
                }
            }
            FetchOutcome::Permanent => {
                permanent += 1;
            }
            FetchOutcome::Transient(err) => {
                // YouTube 登录限制：无 Cookie 时救不回，直接永久跳过；
                // 有 Cookie 时留给 yt-dlp 兜底（它会透传 Cookie）。
                // 判据是 parse_youtube_watch 产出的 LOGIN_REQUIRED 标记。
                if is_youtube && err.contains("LOGIN_REQUIRED") && !has_cookie {
                    permanent += 1;
                } else {
                    if err_sample.is_none() {
                        err_sample = Some(err.clone());
                    }
                    retry_targets.push((row_id, video_id, url));
                }
            }
        }
        if done.is_multiple_of(10) || last_emit.elapsed() >= Duration::from_millis(500) {
            last_emit = tokio::time::Instant::now();
            let _ = app.emit(
                "channel-enrich-progress",
                super::enrich::ChannelEnrichProgressPayload {
                    channel_id: channel_id.to_string(),
                    status: "enriching".to_string(),
                    total,
                    done: done.min(total),
                    fixed,
                    message: None,
                },
            );
        }
    }
    // 收尾：把不足一批的零头写回。取消时也写——已抓到的数据是有效的，
    // 丢掉反而会让 cancelled 事件里的 fixed 计数与 DB 对不上。
    if !pending.is_empty() {
        let batch = std::mem::take(&mut pending);
        let _ = update_video_enriched_fields(&db_state, &batch);
    }

    Ok(FastOutcome {
        done,
        fixed,
        permanent,
        retry_targets,
        err_sample,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_yt_date_ymd() {
        assert_eq!(parse_yt_date_ymd("2024-05-01"), Some(1714521600000));
        assert_eq!(parse_yt_date_ymd("1970-01-01"), Some(0));
        assert!(parse_yt_date_ymd("2024-13-01").is_none());
        assert!(parse_yt_date_ymd("20240501").is_none());
        assert!(parse_yt_date_ymd("").is_none());
    }

    #[test]
    fn test_parse_iso_datetime() {
        // 2009-10-24T23:57:33-07:00 == 2009-10-25T06:57:33Z
        let ts = parse_iso_datetime("2009-10-24T23:57:33-07:00").unwrap();
        assert_eq!(
            ts / 86400000 * 86400000,
            parse_yt_date_ymd("2009-10-25").unwrap()
        );
        assert_eq!(ts % 86400000, (6 * 3600 + 57 * 60 + 33) * 1000);
        assert_eq!(
            parse_iso_datetime("2024-05-01T12:00:00Z").unwrap() % 86400000,
            12 * 3600 * 1000
        );
        assert_eq!(
            parse_iso_datetime("2024-05-01").unwrap(),
            parse_yt_date_ymd("2024-05-01").unwrap()
        );
        assert!(parse_iso_datetime("not-a-date").is_none());
        assert!(parse_iso_datetime("2024-13-01T00:00:00Z").is_none());
    }

    #[test]
    fn test_html_unescape() {
        assert_eq!(html_unescape("A &amp; B &quot;C&quot;"), "A & B \"C\"");
        assert_eq!(html_unescape("it&#39;s"), "it's");
    }

    const WATCH_OK: &str = r#"<!DOCTYPE html><html><head><meta name="title" content="Rick &amp; Roll"><meta itemprop="datePublished" content="2009-10-24T23:57:33-07:00"><meta itemprop="uploadDate" content="2009-10-24T23:57:33-07:00"></head><body><script>var ytInitialPlayerResponse = {"videoDetails":{"videoId":"dQw4w9WgXcQ","title":"x","lengthSeconds":"213","viewCount":"1817620462"},"microformat":{"playerMicroformatRenderer":{"uploadDate":"2009-10-25"}}};</script></body></html>"#;

    #[test]
    fn test_parse_youtube_watch_ok() {
        let update = parse_youtube_watch("row1", WATCH_OK).unwrap();
        assert_eq!(update.id, "row1");
        // meta 的 ISO 时间优先于内嵌 JSON 的纯日期
        assert_eq!(
            update.published_at % 86400000,
            (6 * 3600 + 57 * 60 + 33) * 1000
        );
        assert_eq!(update.title, "Rick & Roll");
        assert_eq!(update.thumbnail, "");
        assert_eq!(update.duration, Some(213.0));
        assert_eq!(update.view_count, Some(1817620462));
    }

    #[test]
    fn test_parse_youtube_watch_json_date_fallback() {
        let html = r#"<html><head></head><body><script>var ytInitialPlayerResponse = {"microformat":{"playerMicroformatRenderer":{"uploadDate":"2024-05-01"}}};</script></body></html>"#;
        let update = parse_youtube_watch("row1", html).unwrap();
        assert_eq!(
            update.published_at,
            parse_yt_date_ymd("2024-05-01").unwrap()
        );
    }

    #[test]
    fn test_parse_youtube_watch_private_is_permanent() {
        let html = r#"<html><body>{"playabilityStatus":{"status":"PRIVATE","reason":"Private video"}}This video is private</body></html>"#;
        let err = parse_youtube_watch("row1", html).unwrap_err();
        assert_eq!(err, "permanent:PRIVATE");
    }

    #[test]
    fn test_parse_youtube_watch_age_gate_is_transient_login() {
        let html = r#"<html><body>{"playabilityStatus":{"status":"LOGIN_REQUIRED"}}Sign in to confirm your age</body></html>"#;
        let err = parse_youtube_watch("row1", html).unwrap_err();
        assert!(err.starts_with("transient:LOGIN_REQUIRED"));
    }

    #[test]
    fn test_parse_youtube_watch_bot_check_is_transient() {
        let html = r#"<html><body>Sign in to confirm you are not a bot</body></html>"#;
        let err = parse_youtube_watch("row1", html).unwrap_err();
        assert_eq!(err, "transient:bot_check");
    }

    #[test]
    fn test_parse_youtube_watch_missing_date_is_transient() {
        let html = r#"<html><head><meta name="title" content="x"></head><body>var ytInitialPlayerResponse = {};</body></html>"#;
        let err = parse_youtube_watch("row1", html).unwrap_err();
        assert_eq!(err, "transient:no_upload_date");
    }

    #[test]
    fn test_parse_bilibili_view_ok() {
        let resp = json!({
            "code": 0, "message": "0",
            "data": {
                "title": "稿件", "pic": "http://pic",
                "pubdate": 1580377255, "duration": 486,
                "stat": {"view": 1419319}
            }
        });
        let update = parse_bilibili_view("row1", &resp).unwrap();
        assert_eq!(update.id, "row1");
        assert_eq!(update.published_at, 1580377255 * 1000);
        assert_eq!(update.title, "稿件");
        assert_eq!(update.thumbnail, "http://pic");
        assert_eq!(update.duration, Some(486.0));
        assert_eq!(update.view_count, Some(1419319));
    }

    #[test]
    fn test_parse_bilibili_deleted_is_permanent() {
        let resp = json!({"code": 62002, "message": "稿件不可见", "data": {}});
        let err = parse_bilibili_view("row1", &resp).unwrap_err();
        assert!(err.starts_with("permanent:"));
    }

    #[test]
    fn test_bili_query_param() {
        assert_eq!(bili_query_param("BV117411r7R1").0, "bvid");
        assert_eq!(bili_query_param("85440373").0, "aid");
    }
}
