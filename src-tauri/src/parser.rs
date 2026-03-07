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
