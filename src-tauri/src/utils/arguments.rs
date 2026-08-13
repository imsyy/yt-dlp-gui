//! 传递给 yt-dlp 的外部工具与插件参数。

use tauri::AppHandle;

use super::paths::{get_deno_path, get_ffmpeg_path, get_managed_deno_path, get_plugin_dir};

/// 将当前选择的 FFmpeg 目录显式交给 yt-dlp，解决 GUI 进程 PATH 不完整的问题。
pub fn build_ffmpeg_location_args(app: &AppHandle) -> Vec<String> {
    let Ok(ffmpeg_path) = get_ffmpeg_path(app) else {
        return vec![];
    };
    if !ffmpeg_path.exists() {
        return vec![];
    }
    let location = ffmpeg_path
        .parent()
        .unwrap_or(&ffmpeg_path)
        .to_string_lossy()
        .to_string();
    vec!["--ffmpeg-location".to_string(), location]
}

/// 如果插件目录存在，返回 --plugin-dirs 参数
pub fn build_plugin_args(app: &AppHandle) -> Vec<String> {
    if let Ok(plugin_dir) = get_plugin_dir(app) {
        if plugin_dir.exists() {
            return vec![
                "--plugin-dirs".to_string(),
                plugin_dir.to_string_lossy().to_string(),
            ];
        }
    }
    vec![]
}

/// 如果 Deno 已安装，返回 JS 运行时参数
pub fn build_js_runtime_args(app: &AppHandle) -> Vec<String> {
    // “系统 Deno”可能在 GUI 进程继承的 PATH 中不可见。此时如果应用管理版本
    // 已存在，仍用它完成 YouTube 的 JS challenge，避免静默退化为仅 360p 合并流。
    let deno_path = get_deno_path(app)
        .ok()
        .filter(|path| path.exists())
        .or_else(|| get_managed_deno_path(app).ok().filter(|path| path.exists()));
    if let Some(deno_path) = deno_path {
        return vec![
            "--js-runtimes".to_string(),
            format!("deno:{}", deno_path.to_string_lossy()),
        ];
    }
    vec![]
}
