//! 频道归档的纯解析辅助函数：URL 规范化、平台识别、元数据与日期提取。
//! 无副作用，可单测。

use serde_json::Value;

/// B 站风控严（默认 UA 易被 412 拦截），仅 B 站请求附加的浏览器 UA。
/// 实测：同机房出口下默认 UA 扫空间列表 412，加此 UA 后正常。
pub(crate) const BILIBILI_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

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

/// 年月日 -> 自 Unix 纪元起的天数（各日期解析共用，非法月/日返回 None）。
pub(crate) fn civil_days(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = if month <= 2 { month + 9 } else { month - 3 } as i64;
    let d = day as i64;
    Some(365 * y + y / 4 - y / 100 + y / 400 + (m * 306 + 5) / 10 + (d - 1) - 719468)
}

/// 解析发布日期字符串 (YYYYMMDD) 为毫秒时间戳（UTC 零点）
pub(crate) fn parse_upload_date(date_str: &str) -> Option<i64> {
    if date_str.len() != 8 {
        return None;
    }
    // 用 get 而非直接切片：脏输入含多字节字符时返回 None，不 panic
    let year: i32 = date_str.get(0..4)?.parse().ok()?;
    let month: u32 = date_str.get(4..6)?.parse().ok()?;
    let day: u32 = date_str.get(6..8)?.parse().ok()?;
    civil_days(year, month, day).map(|days| days * 86400 * 1000)
}

/// 从 B 站空间首条完整视频 JSON 中提取作者信息。
/// 背景：空间页轻量提取（playlist-items 0）无任何元数据，标题会回退成 URL，
/// 只能多抓一条完整视频，用其 uploader 字段回填（视频作者即空间主人）。
/// 返回 (频道标题, 作者, 作者 ID)，频道标题沿用作者名（视频标题不能代表频道）。
pub(crate) fn extract_space_author(entry: &Value) -> Option<(String, String, String)> {
    let uploader = entry
        .get("uploader")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?;
    let uploader_id = entry
        .get("uploader_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((
        uploader.to_string(),
        uploader.to_string(),
        uploader_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_upload_date() {
        assert_eq!(parse_upload_date("20240501"), Some(1714521600000));
        assert_eq!(parse_upload_date("19700101"), Some(0));
        // 非 YYYYMMDD 形态与非法月日
        assert!(parse_upload_date("2024-05-01").is_none());
        assert!(parse_upload_date("20241301").is_none());
        assert!(parse_upload_date("").is_none());
        // 多字节脏输入不得 panic（len 为 8 但不构成合法年月日）
        assert!(parse_upload_date("2024😀").is_none());
    }

    #[test]
    fn test_extract_space_author() {
        let entry = json!({
            "id": "BV1vCYD6QEFH",
            "title": "某个视频标题",
            "uploader": "某某UP主",
            "uploader_id": "384135088",
        });
        assert_eq!(
            extract_space_author(&entry),
            Some((
                "某某UP主".to_string(),
                "某某UP主".to_string(),
                "384135088".to_string(),
            ))
        );
        // 无作者字段：无法回填
        assert!(extract_space_author(&json!({"title": "x"})).is_none());
        // 空白作者名：视为缺失
        assert!(extract_space_author(&json!({"uploader": "  "})).is_none());
    }
}
