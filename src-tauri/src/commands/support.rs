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
    run_ytdlp_json_tracked(app, None, url, extra_args, cookie_file, cookie_browser, proxy).await
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
    for a in extra_args {
        args.push(a.to_string());
    }
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_ffmpeg_location_args(app));
    args.extend(utils::build_plugin_args(app));
    args.extend(utils::build_youtube_extractor_args());
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
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if start < end {
                if let Ok(val) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                    return Some(val);
                }
            }
        }
        let candidate = &trimmed[start..];
        let mut de = serde_json::Deserializer::from_str(candidate);
        use serde::Deserialize;
        if let Ok(val) = Value::deserialize(&mut de) {
            return Some(val);
        }
    }
    None
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
}

