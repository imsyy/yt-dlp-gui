//! 视频信息获取与 Cookie 管理

use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

use super::support;

// Cookie 管理

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
    use super::{fetch_thumbnail, normalize_netscape_cookie_file};
    use std::io::{BufRead, BufReader, Write};

    fn image_proxy(response: &'static [u8]) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
            }
            reader.into_inner().write_all(response).unwrap();
            request
        });
        (proxy, server)
    }

    #[test]
    fn thumbnail_preview_uses_proxy_and_returns_binary_bytes() {
        use tauri::ipc::{InvokeResponseBody, IpcResponse};
        let (proxy, server) =
            image_proxy(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let response = runtime
            .block_on(fetch_thumbnail(
                "http://thumbnail.invalid/cover".into(),
                Some(proxy),
            ))
            .unwrap();
        assert!(
            matches!(response.body().unwrap(), InvokeResponseBody::Raw(bytes) if bytes == b"abc")
        );
        assert!(server
            .join()
            .unwrap()
            .starts_with("GET http://thumbnail.invalid/cover "));
    }

    #[test]
    fn thumbnail_preview_rejects_oversized_responses_and_non_http_urls() {
        let (proxy, server) = image_proxy(
            b"HTTP/1.1 200 OK\r\nContent-Length: 10485761\r\nConnection: close\r\n\r\n",
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let response = runtime.block_on(fetch_thumbnail(
            "http://thumbnail.invalid/cover".into(),
            Some(proxy),
        ));
        assert!(response.err().unwrap().contains("exceeds 10 MiB"));
        server.join().unwrap();
        assert!(runtime
            .block_on(fetch_thumbnail("file:///image.jpg".into(), None))
            .is_err());
    }

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

// 视频信息

/// 使用 yt-dlp -J 获取视频元信息（标题、格式列表、字幕等）
#[tauri::command]
pub async fn fetch_video_info(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    cookie_browser: Option<String>,
    proxy: Option<String>,
    custom_args: Option<String>,
) -> Result<Value, String> {
    let custom_args = utils::split_command_line_args(custom_args.as_deref().unwrap_or_default())?;
    let extra_args: Vec<&str> = custom_args.iter().map(String::as_str).collect();
    support::run_ytdlp_json(
        &app,
        &url,
        &extra_args,
        cookie_file.as_deref(),
        cookie_browser.as_deref(),
        proxy.as_deref(),
    )
    .await
}

/// Webview image requests do not inherit yt-dlp's SOCKS/HTTP proxy settings.
#[tauri::command]
pub async fn fetch_thumbnail(
    url: String,
    proxy: Option<String>,
) -> Result<tauri::ipc::Response, String> {
    use futures_util::StreamExt;
    let parsed = url::Url::parse(&url).map_err(|e| format!("err_download_thumbnail:{}", e))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("err_download_thumbnail:unsupported URL scheme".into());
    }
    const MAX_THUMBNAIL_BYTES: usize = 10 * 1024 * 1024;
    let response = support::build_http_client(proxy.as_deref())?
        .get(parsed)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("err_download_thumbnail:{}", e))?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_THUMBNAIL_BYTES as u64)
    {
        return Err("err_download_thumbnail:image exceeds 10 MiB".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("err_read_thumbnail_data:{}", e))?;
        if bytes.len() + chunk.len() > MAX_THUMBNAIL_BYTES {
            return Err("err_download_thumbnail:image exceeds 10 MiB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(tauri::ipc::Response::new(bytes))
}
