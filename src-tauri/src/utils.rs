use std::path::PathBuf;
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
        .get_or_init(|| RwLock::new(BinaryPathResolveMode::AppOnly));
    lock.read()
        .map(|v| *v)
        .unwrap_or(BinaryPathResolveMode::AppOnly)
}

/// 设置二进制路径解析模式
/// - app-only: 仅使用应用管理路径（默认；保证「检测更新」始终对实际使用的副本生效）
/// - system-preferred: 优先系统安装路径，其次应用管理路径
pub fn set_binary_path_resolve_mode(mode: &str) -> Result<(), String> {
    let parsed = match mode {
        "system-preferred" => BinaryPathResolveMode::SystemPreferred,
        "app-only" => BinaryPathResolveMode::AppOnly,
        _ => return Err(format!("err_invalid_path_mode:{}", mode)),
    };

    let lock = BINARY_PATH_RESOLVE_MODE
        .get_or_init(|| RwLock::new(BinaryPathResolveMode::AppOnly));
    let mut guard = lock
        .write()
        .map_err(|e| format!("err_set_path_mode:{}", e))?;
    *guard = parsed;
    Ok(())
}

// ========== YouTube extractor 参数（po_token / visitor_data）==========

#[derive(Default, Clone)]
struct YoutubeExtractorArgs {
    po_token: String,
    visitor_data: String,
}

static YOUTUBE_EXTRACTOR_ARGS: OnceLock<RwLock<YoutubeExtractorArgs>> = OnceLock::new();

fn youtube_args_lock() -> &'static RwLock<YoutubeExtractorArgs> {
    YOUTUBE_EXTRACTOR_ARGS.get_or_init(|| RwLock::new(YoutubeExtractorArgs::default()))
}

/// 设置 YouTube PO Token / visitor_data；空字符串表示清除。
/// 用于绕过 YouTube 403 / 限流（详见 yt-dlp wiki: Extractors > YouTube）。
pub fn set_youtube_extractor_args(po_token: &str, visitor_data: &str) -> Result<(), String> {
    let mut guard = youtube_args_lock()
        .write()
        .map_err(|e| format!("err_set_youtube_args:{}", e))?;
    guard.po_token = po_token.trim().to_string();
    guard.visitor_data = visitor_data.trim().to_string();
    Ok(())
}

/// 根据当前 PO Token / visitor_data 构建 yt-dlp `--extractor-args` 参数；
/// 两个值都为空时返回空 vec（不追加参数）。
pub fn build_youtube_extractor_args() -> Vec<String> {
    let guard = match youtube_args_lock().read() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    let mut parts: Vec<String> = Vec::new();
    if !guard.po_token.is_empty() {
        parts.push(format!("po_token={}", guard.po_token));
    }
    if !guard.visitor_data.is_empty() {
        parts.push(format!("visitor_data={}", guard.visitor_data));
    }
    if parts.is_empty() {
        return vec![];
    }
    vec![
        "--extractor-args".to_string(),
        format!("youtube:{}", parts.join(";")),
    ]
}

// ========== FFmpeg 目录 ==========

static FFMPEG_DIR: OnceLock<RwLock<String>> = OnceLock::new();

fn ffmpeg_dir_lock() -> &'static RwLock<String> {
    FFMPEG_DIR.get_or_init(|| RwLock::new(String::new()))
}

/// 设置 FFmpeg 所在目录；空字符串表示清除（交由 yt-dlp 自行在 PATH 中查找）。
pub fn set_ffmpeg_dir(dir: &str) -> Result<(), String> {
    let mut guard = ffmpeg_dir_lock()
        .write()
        .map_err(|e| format!("err_set_ffmpeg_dir:{}", e))?;
    *guard = dir.trim().to_string();
    Ok(())
}

/// 若用户设置了 FFmpeg 目录，返回 `--ffmpeg-location` 参数
pub fn build_ffmpeg_location_args() -> Vec<String> {
    let guard = match ffmpeg_dir_lock().read() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    if guard.is_empty() {
        return vec![];
    }
    vec!["--ffmpeg-location".to_string(), guard.clone()]
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

/// 在系统 PATH 中查找可执行文件
/// 使用 `which` crate 而非派生子进程，避免 Windows 控制台代码页（GBK 等）
/// 输出非 UTF-8 时解析失败；同时自动处理 PATHEXT 等平台细节。
fn find_system_executable(name: &str) -> Option<PathBuf> {
    which::which(name).ok().filter(|path| path.exists())
}

/// 解析可执行文件路径
/// - system-preferred: 优先系统安装路径，其次应用管理路径
/// - app-only: 仅应用管理路径
fn resolve_executable_path(managed_path: PathBuf, system_name: &str) -> PathBuf {
    match get_path_resolve_mode() {
        BinaryPathResolveMode::AppOnly => managed_path,
        BinaryPathResolveMode::SystemPreferred => {
            find_system_executable(system_name).unwrap_or(managed_path)
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
    Ok(resolve_executable_path(managed_path, "yt-dlp"))
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
    Ok(resolve_executable_path(managed_path, "deno"))
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
