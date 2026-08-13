//! 视频信息获取与 Cookie 管理

use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

use super::support;

// ========== Cookie 管理 ==========

/// 保存 Cookie 文本（Netscape 格式）到应用数据目录
#[tauri::command]
pub async fn save_cookie_text(app: AppHandle, text: String) -> Result<String, String> {
    let cookie_path = utils::get_cookie_path(&app)?;
    let normalized = normalize_netscape_cookie_file(&text)?;
    let temporary = cookie_path.with_extension("txt.tmp");
    tokio::fs::write(&temporary, normalized.as_bytes())
        .await
        .map_err(|e| format!("err_save_cookie:{}", e))?;
    if tokio::fs::rename(&temporary, &cookie_path).await.is_err() {
        let _ = tokio::fs::remove_file(&cookie_path).await;
        tokio::fs::rename(&temporary, &cookie_path)
            .await
            .map_err(|e| format!("err_save_cookie:{}", e))?;
    }
    Ok(cookie_path.to_string_lossy().to_string())
}

fn normalize_netscape_cookie_file(text: &str) -> Result<String, String> {
    let mut lines = Vec::new();
    let mut has_header = false;
    let mut cookie_count = 0usize;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if index == 0 && matches!(line, "# Netscape HTTP Cookie File" | "# HTTP Cookie File") {
            has_header = true;
        }
        if line.is_empty() || (line.starts_with('#') && !line.starts_with("#HttpOnly_")) {
            lines.push(line.to_string());
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 7
            || fields[0].is_empty()
            || !matches!(fields[1], "TRUE" | "FALSE")
            || !fields[2].starts_with('/')
            || !matches!(fields[3], "TRUE" | "FALSE")
            || fields[4].parse::<i64>().is_err()
            || fields[5].is_empty()
        {
            return Err(format!("err_invalid_cookie_line:{}", index + 1));
        }
        cookie_count += 1;
        lines.push(line.to_string());
    }
    if !has_header {
        return Err("err_invalid_cookie_header".into());
    }
    if cookie_count == 0 {
        return Err("err_empty_cookie_file".into());
    }
    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::normalize_netscape_cookie_file;

    #[test]
    fn validates_and_normalizes_cookie_file() {
        let input = "# Netscape HTTP Cookie File\r\n.example.com\tTRUE\t/\tTRUE\t0\tsid\tvalue\r\n";
        let normalized = normalize_netscape_cookie_file(input).unwrap();
        assert_eq!(
            normalized,
            "# Netscape HTTP Cookie File\n.example.com\tTRUE\t/\tTRUE\t0\tsid\tvalue\n"
        );
    }

    #[test]
    fn rejects_incomplete_cookie_rows() {
        let input = "# Netscape HTTP Cookie File\n.example.com\tTRUE\t/\tTRUE\t0\tsid";
        assert!(normalize_netscape_cookie_file(input).is_err());
    }
}

// ========== 视频信息 ==========

/// 使用 yt-dlp -J 获取视频元信息（标题、格式列表、字幕等）
#[tauri::command]
pub async fn fetch_video_info(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    support::run_ytdlp_json(
        &app,
        &url,
        &[],
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}
