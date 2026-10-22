//! 平台与外部工具运行设置命令。

use crate::utils;

/// 获取当前运行平台
#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

/// 设置三个外部工具各自的来源；选择是严格的，不再隐式回退到另一来源。
#[tauri::command]
pub fn set_tool_sources(
    ytdlp: String,
    deno: String,
    ffmpeg: String,
    ytdlp_local_path: Option<String>,
) -> Result<(), String> {
    utils::set_tool_sources(
        &ytdlp,
        &deno,
        &ffmpeg,
        &ytdlp_local_path.unwrap_or_default(),
    )
}

#[tauri::command]
pub fn set_js_runtime(runtime: String) -> Result<(), String> {
    utils::set_js_runtime(&runtime)
}

#[tauri::command]
pub async fn get_js_runtime_status(app: tauri::AppHandle) -> Result<super::ToolStatus, String> {
    let runtime = utils::get_js_runtime();
    if runtime == "deno" {
        return super::get_deno_status(app).await;
    }
    let path = utils::get_js_runtime_path(&app)?;
    let version_arg = if runtime == "quickjs" {
        "--help"
    } else {
        "--version"
    };
    Ok(super::support::probe_tool_status(runtime, path, version_arg, "system", false, false).await)
}

/// 设置 YouTube 提取器参数（PO Token / visitor_data），用于绕过 YouTube 403 / 限流
#[tauri::command]
pub fn set_youtube_extractor_args(po_token: String, visitor_data: String) -> Result<(), String> {
    utils::set_youtube_extractor_args(&po_token, &visitor_data)
}
