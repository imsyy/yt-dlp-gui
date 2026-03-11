//! yt-dlp 输出解析模块

/// 进度信息
pub struct ProgressInfo {
    pub percent: f64,
    pub speed: String,
    pub eta: String,
    pub downloaded: String,
    pub total: String,
}

/// 解析 --progress-template 输出的 JSON 进度行
/// 格式: PROGRESS_JSON:{"percent":" 45.2%","speed":"2.50MiB/s","eta":"00:11","downloaded":"22.68MiB","total":"50.35MiB"}
pub fn parse_progress_json(line: &str) -> Option<ProgressInfo> {
    let json_str = line.strip_prefix("PROGRESS_JSON:")?;
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

    // _percent_str 格式为 " 45.2%" 或 "100%"，去掉 % 和空格后解析为数字
    let percent_str = v["percent"].as_str().unwrap_or("0%");
    let percent: f64 = percent_str
        .trim()
        .trim_end_matches('%')
        .parse()
        .unwrap_or(0.0);

    let speed = clean_field(v["speed"].as_str());
    let eta = clean_field(v["eta"].as_str());
    let downloaded = clean_field(v["downloaded"].as_str());
    let total = clean_field(v["total"].as_str());

    Some(ProgressInfo {
        percent,
        speed,
        eta,
        downloaded,
        total,
    })
}

/// 解析 ffmpeg 输出中的 time= 字段，返回已处理的秒数
/// 格式: frame= 1234 fps=128 ... time=00:02:29.65 ...
pub fn parse_ffmpeg_time(line: &str) -> Option<f64> {
    let time_start = line.find("time=")?;
    let after = &line[time_start + 5..];
    let time_str = after.split_whitespace().next()?;
    // 格式: HH:MM:SS.xx 或 -HH:MM:SS.xx (负值表示尚未开始)
    if time_str.starts_with('-') || time_str == "N/A" {
        return None;
    }
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let s: f64 = parts[2].parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

/// 清理 yt-dlp 输出字段：移除 NA/Unknown 等无效值
fn clean_field(s: Option<&str>) -> String {
    match s {
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty()
                || trimmed == "NA"
                || trimmed == "Unknown"
                || trimmed.contains("N/A")
            {
                String::new()
            } else {
                trimmed.to_string()
            }
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_progress_json_valid_input_returns_info() {
        let line = r#"PROGRESS_JSON:{"percent":" 45.2%","speed":"2.50MiB/s","eta":"00:11","downloaded":"22.68MiB","total":"50.35MiB"}"#;
        let info = parse_progress_json(line).unwrap();
        assert!((info.percent - 45.2).abs() < 0.01);
        assert_eq!(info.speed, "2.50MiB/s");
        assert_eq!(info.eta, "00:11");
        assert_eq!(info.downloaded, "22.68MiB");
        assert_eq!(info.total, "50.35MiB");
    }

    #[test]
    fn parse_progress_json_100_percent_returns_full() {
        let line = r#"PROGRESS_JSON:{"percent":"100%","speed":"","eta":"","downloaded":"50.35MiB","total":"50.35MiB"}"#;
        let info = parse_progress_json(line).unwrap();
        assert!((info.percent - 100.0).abs() < 0.01);
    }

    #[test]
    fn parse_progress_json_na_values_returns_empty_strings() {
        let line = r#"PROGRESS_JSON:{"percent":"0%","speed":"NA","eta":"Unknown","downloaded":"N/A","total":"N/A"}"#;
        let info = parse_progress_json(line).unwrap();
        assert!(info.speed.is_empty());
        assert!(info.eta.is_empty());
        assert!(info.downloaded.is_empty());
        assert!(info.total.is_empty());
    }

    #[test]
    fn parse_progress_json_no_prefix_returns_none() {
        assert!(parse_progress_json("some random line").is_none());
    }

    #[test]
    fn parse_progress_json_invalid_json_returns_none() {
        assert!(parse_progress_json("PROGRESS_JSON:{not valid json}").is_none());
    }

    #[test]
    fn parse_ffmpeg_time_valid_input_returns_seconds() {
        let line = "frame= 1234 fps=128 q=28.0 size=   15360kB time=00:02:29.65 bitrate= 840.2kbits/s speed=2.13x";
        let secs = parse_ffmpeg_time(line).unwrap();
        assert!((secs - 149.65).abs() < 0.01);
    }

    #[test]
    fn parse_ffmpeg_time_zero_returns_zero() {
        let line = "frame=    1 fps=0.0 q=0.0 size=       0kB time=00:00:00.00 bitrate=N/A speed=N/A";
        let secs = parse_ffmpeg_time(line).unwrap();
        assert!((secs - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_ffmpeg_time_negative_returns_none() {
        let line = "frame=    0 fps=0.0 q=0.0 size=       0kB time=-577014:32:22.77 bitrate=N/A speed=N/A";
        assert!(parse_ffmpeg_time(line).is_none());
    }

    #[test]
    fn parse_ffmpeg_time_na_returns_none() {
        let line = "frame=    0 fps=0.0 q=0.0 size=       0kB time=N/A bitrate=N/A speed=N/A";
        assert!(parse_ffmpeg_time(line).is_none());
    }

    #[test]
    fn parse_ffmpeg_time_no_time_field_returns_none() {
        assert!(parse_ffmpeg_time("some random line without time field").is_none());
    }

    #[test]
    fn clean_field_none_returns_empty() {
        assert!(clean_field(None).is_empty());
    }

    #[test]
    fn clean_field_empty_string_returns_empty() {
        assert!(clean_field(Some("")).is_empty());
        assert!(clean_field(Some("   ")).is_empty());
    }

    #[test]
    fn clean_field_na_values_returns_empty() {
        assert!(clean_field(Some("NA")).is_empty());
        assert!(clean_field(Some("Unknown")).is_empty());
        assert!(clean_field(Some("N/A")).is_empty());
    }

    #[test]
    fn clean_field_valid_value_returns_trimmed() {
        assert_eq!(clean_field(Some("2.50MiB/s")), "2.50MiB/s");
        assert_eq!(clean_field(Some("  00:11  ")), "00:11");
    }
}
