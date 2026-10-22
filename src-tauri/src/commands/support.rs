//! 命令处理器共享的 yt-dlp、HTTP 与路径辅助函数

use crate::platform::process::ProcessRegistry;
use crate::utils;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

/// 默认 HTTP 请求超时时间（5 分钟）
const HTTP_TIMEOUT: Duration = Duration::from_secs(300);

/// 向参数列表追加 Cookie 和代理相关参数
pub fn append_cookie_proxy_args(
    args: &mut Vec<String>,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) {
    if let Some(cf) = cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.to_string());
        }
    }
    if let Some(browser) = cookie_browser {
        if !browser.is_empty() {
            args.push("--cookies-from-browser".to_string());
            args.push(browser.to_string());
        }
    }
    if let Some(p) = proxy {
        if !p.is_empty() {
            args.push("--proxy".to_string());
            args.push(p.to_string());
        }
    }
}

/// 构建带可选代理和超时的 HTTP 客户端
pub fn build_http_client(proxy: Option<&str>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(HTTP_TIMEOUT);
    if let Some(p) = proxy {
        if !p.is_empty() {
            let reqwest_proxy =
                reqwest::Proxy::all(p).map_err(|e| format!("err_proxy_config:{}", e))?;
            builder = builder.proxy(reqwest_proxy);
        }
    }
    builder
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))
}

/// 启动 yt-dlp 子进程并等待结束，返回完整输出。
///
/// 统一处理各调用点都需要的那套样板：UTF-8 环境变量、置空 stdin（tokio 的 spawn
/// 默认继承 stdin，会让 yt-dlp 等待输入挂住）、隐藏 Windows 控制台窗口、
/// `kill_on_drop` 兜底回收，以及 `run_id` 非空时的 pid 登记/注销。
///
/// pid 的注销放在等待结果之外无条件执行：若写在 `?` 之后，等待报错时 pid 会永久
/// 残留在注册表里，同一个 `run_id` 再也无法被取消。把这段逻辑收进单一函数，
/// 调用方就无法再写漏。
pub async fn run_ytdlp_capture(
    app: &AppHandle,
    run_id: Option<&str>,
    args: Vec<String>,
) -> Result<std::process::Output, String> {
    let ytdlp_path = utils::get_ytdlp_path(app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

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

    let child = cmd.spawn().map_err(|e| format!("err_run_ytdlp:{}", e))?;
    if let Some(run_id) = run_id {
        if let Some(pid) = child.id() {
            app.state::<ProcessRegistry>().register(run_id, pid);
        }
    }

    let output = child.wait_with_output().await;
    if let Some(run_id) = run_id {
        app.state::<ProcessRegistry>().take(run_id);
    }
    output.map_err(|e| format!("err_run_ytdlp:{}", e))
}

/// 运行 yt-dlp -J 并解析 JSON 输出（用于获取视频信息、封面列表、字幕列表等）
pub async fn run_ytdlp_json(
    app: &AppHandle,
    url: &str,
    extra_args: &[&str],
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<Value, String> {
    run_ytdlp_json_tracked(
        app,
        None,
        url,
        extra_args,
        cookie_file,
        cookie_browser,
        proxy,
    )
    .await
}

/// 同 `run_ytdlp_json`，额外把子进程 pid 登记到 `run_id` 名下，供工具任务取消时终止。
pub async fn run_ytdlp_json_tracked(
    app: &AppHandle,
    run_id: Option<&str>,
    url: &str,
    extra_args: &[&str],
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<Value, String> {
    let mut args = vec![
        "-J".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--no-warnings".to_string(),
        // 网络异常时快速失败：默认 retries=10、无 socket 超时，会卡住几分钟
        "--socket-timeout".to_string(),
        "15".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--extractor-retries".to_string(),
        "2".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_ffmpeg_location_args(app));
    args.extend(utils::build_plugin_args(app));
    args.extend(utils::build_youtube_extractor_args());
    for a in extra_args {
        args.push(a.to_string());
    }
    append_cookie_proxy_args(&mut args, cookie_file, cookie_browser, proxy);
    args.push(url.to_string());

    let output = run_ytdlp_capture(app, run_id, args).await?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 优先从 stdout 解析 JSON（yt-dlp 可能在 stderr 输出警告但仍成功，且可能输出多行格式化 JSON）
    if let Some(val) = parse_json_from_stdout(&stdout) {
        return Ok(val);
    }

    // 未找到 JSON，从 stderr 提取 ERROR 行作为错误信息
    let stderr = String::from_utf8_lossy(&output.stderr);
    let err = extract_ytdlp_error(&stderr);
    if err.is_empty() {
        let stdout_err = extract_ytdlp_error(&stdout);
        if !stdout_err.is_empty() {
            return Err(stdout_err);
        }
        Err("err_parse_video_info:empty_json".to_string())
    } else {
        Err(err)
    }
}

/// 从 yt-dlp stdout 输出中健壮地解析 JSON（支持单行、多行格式化及夹带日志输出）
pub fn parse_json_from_stdout(stdout: &str) -> Option<Value> {
    let trimmed = stdout.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }

    let mut remaining = trimmed;
    let mut fallback = None;
    while let Some(start) = remaining.find('{') {
        let candidate = &remaining[start..];
        let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
        let result = stream.next();
        if matches!(&result, Some(Err(error)) if error.is_eof()) {
            // A truncated JSON document can contain complete nested objects.
            // Do not return one of those as if it were the missing outer value.
            break;
        }
        if let Some(Ok(value)) = result {
            // All current callers expect -J metadata. Ignore valid JSON log
            // objects such as {} when a video or playlist follows them.
            let is_metadata = value.get("formats").is_some_and(Value::is_array)
                || value.get("entries").is_some_and(Value::is_array)
                || (value.get("id").is_some_and(Value::is_string)
                    && value.get("title").is_some_and(Value::is_string));
            if is_metadata {
                return Some(value);
            }
            if fallback.is_none() {
                fallback = Some(value);
            }
            // Let serde handle nesting, escaped quotes and braces in strings;
            // never treat part of a valid object as a separate candidate.
            remaining = &candidate[stream.byte_offset()..];
        } else {
            // A log message can contain an unmatched or non-JSON opening brace.
            // Resume after it so a later complete metadata object is still read.
            remaining = &candidate[1..];
        }
    }
    fallback
}

/// 从 yt-dlp stderr 输出中提取错误信息
pub fn extract_ytdlp_error(stderr: &str) -> String {
    let error_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("ERROR:")).collect();
    if error_lines.is_empty() {
        stderr.trim().to_string()
    } else {
        error_lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_single_line() {
        let stdout = r#"{"id": "abc123", "title": "Test Video"}"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "abc123");
        assert_eq!(parsed["title"], "Test Video");
    }

    #[test]
    fn test_parse_json_multiline() {
        let stdout = r#"{
  "id": "multi123",
  "title": "Multiline Video",
  "formats": [
    {"format_id": "137", "height": 1080}
  ]
}"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "multi123");
        assert_eq!(parsed["formats"][0]["height"], 1080);
    }

    #[test]
    fn test_parse_json_with_surrounding_warnings() {
        let stdout = r#"[youtube] Extracting URL: https://example.com/watch?v=123
WARNING: [youtube] Some warning occurred
{
  "id": "123",
  "title": "Warning Handled"
}
[download] Some trailing info
"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "123");
        assert_eq!(parsed["title"], "Warning Handled");
    }

    #[test]
    fn test_parse_json_invalid() {
        let stdout = "ERROR: Video unavailable";
        assert!(parse_json_from_stdout(stdout).is_none());
    }

    #[test]
    fn skips_malformed_braces_in_surrounding_logs() {
        let stdout = "WARNING: 使用 {proxy}\nWARNING: unmatched {\n\
            {\"id\":\"video\",\"title\":\"Actual video\"}\nfinished {cleanup}";
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "video");
    }

    #[test]
    fn prefers_metadata_after_valid_json_log_objects() {
        let stdout = r#"DEBUG: {}
DEBUG: {"message":"a quoted {\"id\":\"fake\",\"title\":\"log\"}"}
{"id":"video","title":"Actual video","formats":[{"format_id":"137"}]}
DEBUG: {"done":true}
"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "video");
        assert_eq!(parsed["formats"][0]["format_id"], "137");
    }

    #[test]
    fn preserves_multiline_video_and_braces_inside_strings() {
        let stdout = r#"DEBUG: {}
{
  "id": "video",
  "title": "Braces } and { with a \"quote\"",
  "formats": [{"format_id":"137","http_headers":{"User-Agent":"test {agent}"}}]
}
WARNING: trailing {noise}
"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["title"], "Braces } and { with a \"quote\"");
        assert_eq!(
            parsed["formats"][0]["http_headers"]["User-Agent"],
            "test {agent}"
        );
    }

    #[test]
    fn keeps_multiline_playlist_and_nested_entry_formats_together() {
        let stdout = r#"DEBUG: {not JSON}
{
  "_type": "playlist",
  "id": "playlist",
  "title": "Playlist",
  "entries": [
    {"id":"first","title":"First","formats":[{"format_id":"137"}]},
    {"id":"second","title":"Second","formats":[{"format_id":"140"}]}
  ]
}
DEBUG: {}
"#;
        let parsed = parse_json_from_stdout(stdout).unwrap();
        assert_eq!(parsed["id"], "playlist");
        assert_eq!(parsed["entries"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["entries"][1]["formats"][0]["format_id"], "140");
    }

    #[test]
    fn retains_generic_json_compatibility_without_metadata() {
        assert_eq!(parse_json_from_stdout("{}"), Some(serde_json::json!({})));
        assert_eq!(
            parse_json_from_stdout(r#"[1, {"message":"ok"}]"#),
            Some(serde_json::json!([1, {"message":"ok"}]))
        );
        assert_eq!(
            parse_json_from_stdout(r#"DEBUG: {"message":"ok"} trailing {noise}"#),
            Some(serde_json::json!({"message":"ok"}))
        );
    }

    #[test]
    fn does_not_extract_nested_objects_from_truncated_metadata() {
        assert!(parse_json_from_stdout(
            r#"{"id":"video","title":"Video","formats":[{"format_id":"137"}]"#
        )
        .is_none());
        assert!(parse_json_from_stdout(
            r#"{"_type":"playlist","entries":[{"id":"video","title":"Video"}]"#
        )
        .is_none());
    }
}
