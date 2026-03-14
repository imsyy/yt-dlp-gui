use std::path::PathBuf;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy)]
enum BinaryPathResolveMode {
    SystemPreferred,
    AppOnly,
}

static BINARY_PATH_RESOLVE_MODE: OnceLock<RwLock<BinaryPathResolveMode>> = OnceLock::new();

fn get_path_resolve_mode() -> BinaryPathResolveMode {
    let lock = BINARY_PATH_RESOLVE_MODE
        .get_or_init(|| RwLock::new(BinaryPathResolveMode::SystemPreferred));
    lock.read()
        .map(|v| *v)
        .unwrap_or(BinaryPathResolveMode::SystemPreferred)
}

/// 设置二进制路径解析模式
/// - system-preferred: 优先系统安装路径，其次应用管理路径（默认）
/// - app-only: 仅使用应用管理路径
pub fn set_binary_path_resolve_mode(mode: &str) -> Result<(), String> {
    let parsed = match mode {
        "system-preferred" => BinaryPathResolveMode::SystemPreferred,
        "app-only" => BinaryPathResolveMode::AppOnly,
        _ => return Err(format!("err_invalid_path_mode:{}", mode)),
    };

    let lock = BINARY_PATH_RESOLVE_MODE
        .get_or_init(|| RwLock::new(BinaryPathResolveMode::SystemPreferred));
    let mut guard = lock
        .write()
        .map_err(|e| format!("err_set_path_mode:{}", e))?;
    *guard = parsed;
    Ok(())
}

/// 构建应用数据目录下的可执行文件路径
fn get_managed_executable_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    std::fs::create_dir_all(&app_data).map_err(|e| format!("err_create_dir:{}", e))?;
    Ok(app_data.join(file_name))
}

/// 自动探测系统中可执行文件路径（which/where）
fn find_system_executable(names: &[&str]) -> Option<PathBuf> {
    let command = if cfg!(target_os = "windows") {
        "where.exe"
    } else {
        "which"
    };

    let output = Command::new(command).args(names).output().ok()?;
    if !output.status.success() {
        return None;
    }

    // TODO: 这里在编码非 UTF-8 的系统上可能会有问题
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_matches('"'))
        .map(PathBuf::from)
        .find(|path| path.exists())
}

/// 解析可执行文件路径
/// - system-preferred: 优先系统安装路径，其次应用管理路径
/// - app-only: 仅应用管理路径
fn resolve_executable_path(managed_path: PathBuf, system_names: &[&str]) -> PathBuf {
    match get_path_resolve_mode() {
        BinaryPathResolveMode::AppOnly => managed_path,
        BinaryPathResolveMode::SystemPreferred => {
            if let Some(path) = find_system_executable(system_names) {
                return path;
            }
            managed_path
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
    let managed_path = get_managed_ytdlp_path(app)?;
    let system_names: &[&str] = &["yt-dlp"];

    Ok(resolve_executable_path(managed_path, system_names))
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
    let managed_path = get_managed_deno_path(app)?;
    let system_names: &[&str] = &["deno"];

    Ok(resolve_executable_path(managed_path, system_names))
}

/// 获取 Cookie 文件路径（存放在应用数据目录下）
pub fn get_cookie_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    Ok(app_data.join("cookies.txt"))
}

/// 获取 yt-dlp 下载地址（根据平台）
pub fn get_ytdlp_download_url() -> &'static str {
    if cfg!(target_os = "windows") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    }
}

/// 获取 yt-dlp 插件目录路径
pub fn get_plugin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
    Ok(app_data.join("yt-dlp-plugins"))
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
    if let Ok(deno_path) = get_deno_path(app) {
        if deno_path.exists() {
            return vec![
                "--js-runtimes".to_string(),
                format!("deno:{}", deno_path.to_string_lossy()),
            ];
        }
    }
    vec![]
}

/// 获取 Deno 下载地址（根据平台和架构）
pub fn get_deno_download_url() -> &'static str {
    if cfg!(target_os = "windows") {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-apple-darwin.zip"
        }
    } else {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"
    }
}
