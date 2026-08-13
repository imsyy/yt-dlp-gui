//! 应用内置与系统可执行文件的路径解析。

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use super::source::{get_cli_tool_path, get_tool_source, ToolSource};

/// 构建应用数据目录下的可执行文件路径。
fn get_managed_executable_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    std::fs::create_dir_all(&app_data).map_err(|e| format!("err_create_dir:{}", e))?;
    Ok(app_data.join(file_name))
}

/// 在系统 PATH 中查找可执行文件
/// 使用 `which` crate 而非派生子进程，避免 Windows 控制台代码页（GBK 等）
/// 输出非 UTF-8 时解析失败；同时自动处理 PATHEXT 等平台细节。
fn find_system_executable(name: &str) -> Option<PathBuf> {
    if let Some(path) = which::which(name).ok().filter(|path| path.exists()) {
        return Some(path);
    }

    // GUI 应用在 macOS 上通常拿不到 shell 初始化后的 PATH，需要显式探测包管理器目录。
    #[cfg(target_os = "macos")]
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"] {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "linux")]
    for dir in ["/usr/local/bin", "/usr/bin", "/snap/bin"] {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn resolve_executable_path(
    managed_path: PathBuf,
    system_name: &str,
    source: ToolSource,
) -> PathBuf {
    match source {
        ToolSource::Managed => managed_path,
        ToolSource::System => {
            find_system_executable(system_name).unwrap_or_else(|| PathBuf::from(system_name))
        }
    }
}

/// 获取应用管理的 yt-dlp 路径（应用数据目录）
pub fn get_managed_ytdlp_path(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        get_managed_executable_path(app, "yt-dlp.exe")
    } else {
        get_managed_executable_path(app, "yt-dlp")
    }
}

/// 获取 yt-dlp 可执行文件路径
pub fn get_ytdlp_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = get_cli_tool_path("yt-dlp") {
        return Ok(path);
    }
    let managed_path = get_managed_ytdlp_path(app)?;
    Ok(resolve_executable_path(
        managed_path,
        "yt-dlp",
        get_tool_source("yt-dlp")?,
    ))
}

/// 获取应用管理的 Deno 路径（应用数据目录）
pub fn get_managed_deno_path(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        get_managed_executable_path(app, "deno.exe")
    } else {
        get_managed_executable_path(app, "deno")
    }
}

/// 获取 Deno 可执行文件路径
pub fn get_deno_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = get_cli_tool_path("deno") {
        return Ok(path);
    }
    let managed_path = get_managed_deno_path(app)?;
    Ok(resolve_executable_path(
        managed_path,
        "deno",
        get_tool_source("deno")?,
    ))
}

pub fn get_managed_ffmpeg_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    let dir = app_data.join("ffmpeg");
    std::fs::create_dir_all(&dir).map_err(|e| format!("err_create_dir:{}", e))?;
    Ok(dir)
}

pub fn get_managed_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    Ok(get_managed_ffmpeg_dir(app)?.join(name))
}

pub fn get_managed_ffprobe_path(app: &AppHandle) -> Result<PathBuf, String> {
    let name = if cfg!(target_os = "windows") {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    Ok(get_managed_ffmpeg_dir(app)?.join(name))
}

pub fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_executable_path(
        get_managed_ffmpeg_path(app)?,
        "ffmpeg",
        get_tool_source("ffmpeg")?,
    ))
}

pub fn get_ffprobe_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_executable_path(
        get_managed_ffprobe_path(app)?,
        "ffprobe",
        get_tool_source("ffmpeg")?,
    ))
}

/// 获取 Cookie 文件路径（存放在应用数据目录下）
pub fn get_cookie_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    Ok(app_data.join("cookies.txt"))
}

/// 获取 yt-dlp 插件目录路径
pub fn get_plugin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    Ok(app_data.join("yt-dlp-plugins"))
}
