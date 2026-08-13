use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolSource {
    Managed,
    System,
}

impl ToolSource {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "managed" => Ok(Self::Managed),
            "system" => Ok(Self::System),
            _ => Err(format!("err_invalid_tool_source:{}", value)),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Copy)]
struct ToolSources {
    ytdlp: ToolSource,
    deno: ToolSource,
    ffmpeg: ToolSource,
}

impl Default for ToolSources {
    fn default() -> Self {
        Self {
            ytdlp: ToolSource::Managed,
            deno: ToolSource::Managed,
            ffmpeg: ToolSource::System,
        }
    }
}

static TOOL_SOURCES: OnceLock<RwLock<ToolSources>> = OnceLock::new();

fn tool_sources_lock() -> &'static RwLock<ToolSources> {
    TOOL_SOURCES.get_or_init(|| RwLock::new(ToolSources::default()))
}

pub fn set_tool_sources(ytdlp: &str, deno: &str, ffmpeg: &str) -> Result<(), String> {
    let sources = ToolSources {
        ytdlp: ToolSource::parse(ytdlp)?,
        deno: ToolSource::parse(deno)?,
        ffmpeg: ToolSource::parse(ffmpeg)?,
    };
    let mut guard = tool_sources_lock()
        .write()
        .map_err(|e| format!("err_set_tool_sources:{}", e))?;
    *guard = sources;
    Ok(())
}

pub fn get_tool_source(tool: &str) -> Result<ToolSource, String> {
    let guard = tool_sources_lock()
        .read()
        .map_err(|e| format!("err_get_tool_sources:{}", e))?;
    match tool {
        "yt-dlp" => Ok(guard.ytdlp),
        "deno" => Ok(guard.deno),
        "ffmpeg" => Ok(guard.ffmpeg),
        _ => Err(format!("err_unknown_tool:{}", tool)),
    }
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
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-pc-windows-msvc.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-apple-darwin.zip"
        }
    } else {
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-unknown-linux-gnu.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"
        }
    }
}

pub fn get_ffmpeg_download_urls() -> [(&'static str, &'static str); 2] {
    let platform = if cfg!(target_os = "windows") {
        "win32-x64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "darwin-arm64"
        } else {
            "darwin-x64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux-arm64"
    } else if cfg!(target_arch = "arm") {
        "linux-arm"
    } else {
        "linux-x64"
    };

    match platform {
        "darwin-arm64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-darwin-arm64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-darwin-arm64"),
        ],
        "darwin-x64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-darwin-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-darwin-x64"),
        ],
        "linux-arm64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-arm64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-arm64"),
        ],
        "linux-arm" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-arm"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-arm"),
        ],
        "win32-x64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-win32-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-win32-x64"),
        ],
        _ => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-x64"),
        ],
    }
}
