//! 工具箱命令共用的 yt-dlp 执行器。

use crate::{
    commands::support::{append_cookie_proxy_args, extract_ytdlp_error, run_ytdlp_capture},
    utils,
};
use tauri::AppHandle;

/// 通用工具命令执行器（--skip-download 模式，不下载视频本身）
///
/// `run_id` 非空时把子进程 pid 登记进 `ProcessRegistry`，使任务可以被取消；
/// 进程创建、环境变量与 pid 登记/注销的样板统一由 `run_ytdlp_capture` 负责。
///
/// 参数较多是 yt-dlp 调用面的客观需要（目标、输出目录、附加参数、三类网络凭据），
/// 与 `tool_fetch_*` 系列命令保持同一形状，便于对照阅读。
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_ytdlp_tool(
    app: &AppHandle,
    run_id: Option<&str>,
    url: &str,
    download_dir: &str,
    extra_args: Vec<String>,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<String, String> {
    let mut args = vec![
        "--skip-download".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--windows-filenames".to_string(),
        "--no-warnings".to_string(),
        "--socket-timeout".to_string(),
        "15".to_string(),
        "--retries".to_string(),
        "3".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_ffmpeg_location_args(app));
    args.extend(utils::build_plugin_args(app));
    args.extend(utils::build_youtube_extractor_args());

    let output_template = std::path::PathBuf::from(download_dir)
        .join("%(title).200s.%(ext)s")
        .to_string_lossy()
        .to_string();
    args.push("-o".to_string());
    args.push(output_template);

    args.extend(extra_args);
    append_cookie_proxy_args(&mut args, cookie_file, cookie_browser, proxy);
    args.push(url.to_string());

    let output = run_ytdlp_capture(app, run_id, args).await?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(extract_ytdlp_error(&stderr))
    }
}
