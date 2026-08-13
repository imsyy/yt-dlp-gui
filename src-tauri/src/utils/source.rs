//! 外部工具来源与 YouTube 提取器运行配置。

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

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
static CLI_TOOL_PATHS: OnceLock<RwLock<(Option<PathBuf>, Option<PathBuf>)>> = OnceLock::new();

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

fn cli_tool_paths_lock() -> &'static RwLock<(Option<PathBuf>, Option<PathBuf>)> {
    CLI_TOOL_PATHS.get_or_init(|| RwLock::new((None, None)))
}

pub fn set_cli_tool_path(tool: &str, path: PathBuf) -> Result<(), String> {
    let mut guard = cli_tool_paths_lock()
        .write()
        .map_err(|e| format!("err_set_cli_tool_path:{}", e))?;
    match tool {
        "yt-dlp" => guard.0 = Some(path),
        "deno" => guard.1 = Some(path),
        _ => return Err(format!("err_unknown_tool:{}", tool)),
    }
    Ok(())
}

pub fn get_cli_tool_path(tool: &str) -> Option<PathBuf> {
    let guard = cli_tool_paths_lock().read().ok()?;
    match tool {
        "yt-dlp" => guard.0.clone(),
        "deno" => guard.1.clone(),
        _ => None,
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
        // 当前 yt-dlp 要求 PO Token 明确绑定到播放器客户端和请求上下文。
        // GVS 是实际媒体流请求；未绑定的旧写法可能能列出格式，却在下载时返回 403。
        parts.push("player_client=mweb".to_string());
        parts.push(format!("po_token=mweb.gvs+{}", guard.po_token));
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
