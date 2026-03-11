//! 命令模块共享辅助函数

use crate::utils;
use serde_json::Value;
use std::time::Duration;
use tauri::AppHandle;

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

/// 运行 yt-dlp -J 并解析 JSON 输出（用于获取视频信息、封面列表、字幕列表等）
pub async fn run_ytdlp_json(
    app: &AppHandle,
    url: &str,
    extra_args: &[&str],
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<Value, String> {
    let ytdlp_path = utils::get_ytdlp_path(app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

    let mut args = vec![
        "-J".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
    ];
    for a in extra_args {
        args.push(a.to_string());
    }
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_plugin_args(app));
    append_cookie_proxy_args(&mut args, cookie_file, cookie_browser, proxy);
    args.push(url.to_string());

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_run_ytdlp:{}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 优先从 stdout 解析 JSON（yt-dlp 可能在 stderr 输出警告但仍成功）
    if let Some(json_str) = stdout
        .lines()
        .find(|line| line.trim_start().starts_with('{'))
    {
        return serde_json::from_str(json_str)
            .map_err(|e| format!("err_parse_video_info:{}", e));
    }

    // 未找到 JSON，从 stderr 提取 ERROR 行作为错误信息
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(extract_ytdlp_error(&stderr))
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

/// 验证文件路径安全性（防止路径遍历攻击）
/// 确保解析后的路径位于 base_dir 之下
pub fn validate_path_within(base_dir: &std::path::Path, relative_path: &str) -> Result<std::path::PathBuf, String> {
    let target = base_dir.join(relative_path);
    // 标准化路径，消除 .. 等相对路径组件
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("err_resolve_path:{}", e))?;
    // 对于可能不存在的路径，检查其父目录
    let target_for_check = if target.exists() {
        target
            .canonicalize()
            .map_err(|e| format!("err_resolve_path:{}", e))?
    } else {
        // 如果目标不存在，检查其父目录是否在基础目录内
        let parent = target.parent().ok_or("err_invalid_path")?;
        if !parent.exists() {
            // 如果父目录也不存在，至少检查路径组件中没有 ..
            if relative_path.contains("..") {
                return Err("err_path_traversal".to_string());
            }
            return Ok(base_dir.join(relative_path));
        }
        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| format!("err_resolve_path:{}", e))?;
        if !canonical_parent.starts_with(&canonical_base) {
            return Err("err_path_traversal".to_string());
        }
        return Ok(base_dir.join(relative_path));
    };
    if !target_for_check.starts_with(&canonical_base) {
        return Err("err_path_traversal".to_string());
    }
    Ok(target_for_check)
}
