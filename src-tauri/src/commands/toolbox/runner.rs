//! 工具箱命令共用的 yt-dlp 执行器。

use crate::{
    commands::{support::append_cookie_proxy_args, support::extract_ytdlp_error, CREATE_NO_WINDOW},
    utils,
};
use tauri::AppHandle;

/// 通用工具命令执行器（--skip-download 模式，不下载视频本身）
pub(super) async fn run_ytdlp_tool(
    app: &AppHandle,
    url: &str,
    download_dir: &str,
    extra_args: Vec<String>,
    cookie_file: Option<&str>,
    cookie_browser: Option<&str>,
    proxy: Option<&str>,
) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

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
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        Err(extract_ytdlp_error(&stderr))
    }
}
