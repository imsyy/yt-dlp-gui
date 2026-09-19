//! 频道归档的纯解析辅助函数：URL 规范化、平台识别、元数据与日期提取。
//! 无副作用，可单测。

use serde_json::Value;

pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 规范化频道基础 URL
pub(crate) fn clean_channel_url(url: &str) -> String {
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
pub(crate) fn detect_platform(url: &str, info: &Value) -> String {
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
pub(crate) fn extract_channel_id(info: &Value, url: &str, platform: &str) -> String {
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
pub(crate) fn extract_channel_title(info: &Value, url: &str) -> String {
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
pub(crate) fn extract_channel_avatar(info: &Value) -> String {
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
pub(crate) fn extract_channel_banner(info: &Value) -> String {
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
pub(crate) fn extract_video_thumbnail(entry: &Value) -> String {
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

/// 从 yt-dlp 视频详情 JSON 中提取发布日期（毫秒时间戳）。
/// 优先级：timestamp > upload_date > release_timestamp > release_date。
/// 注意：YouTube 的 timestamp 经常缺失（InnerTube API 不提供精确发布时间，
/// 只能拿到 microformat 的 upload_date 日期），调用方必须保留这个回退链。
pub(crate) fn extract_published_at(entry: &Value) -> Option<i64> {
    entry
        .get("timestamp")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .map(|s| s * 1000)
        .or_else(|| {
            entry
                .get("upload_date")
                .and_then(Value::as_str)
                .and_then(parse_upload_date)
        })
        .or_else(|| {
            entry
                .get("release_timestamp")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .map(|s| s * 1000)
        })
        .or_else(|| {
            entry
                .get("release_date")
                .and_then(Value::as_str)
                .and_then(parse_upload_date)
        })
}

/// 解析发布日期字符串 (YYYYMMDD) 为毫秒时间戳
fn parse_upload_date(date_str: &str) -> Option<i64> {
    if date_str.len() == 8 {
        let year: i32 = date_str[0..4].parse().ok()?;
        let month: u32 = date_str[4..6].parse().ok()?;
        let day: u32 = date_str[6..8].parse().ok()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
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
