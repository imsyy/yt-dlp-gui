/// Tauri 命令模块
///
/// 按功能域拆分:
/// - setup: 平台信息、yt-dlp/Deno 安装管理
/// - video: 视频信息获取、Cookie 管理
/// - download: 下载任务控制

mod setup;
mod video;
mod download;

pub use setup::*;
pub use video::*;
pub use download::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ========== 共享类型 ==========

/// yt-dlp 安装状态
#[derive(serde::Serialize)]
pub struct YtdlpStatus {
    pub installed: bool,
    pub version: String,
    pub path: String,
}

/// Deno 安装状态
#[derive(serde::Serialize)]
pub struct DenoStatus {
    pub installed: bool,
    pub version: String,
    pub path: String,
}

/// 下载进程信息（运行时状态）
pub struct DownloadProcessInfo {
    /// 进程 PID
    pub pid: u32,
    /// 是否已被用户取消
    pub cancelled: bool,
    /// 从 stdout 解析到的输出文件路径（作为备选）
    pub output_files: Vec<String>,
    /// 下载目录
    pub download_dir: String,
    /// 临时文件路径，用于存储 --print-to-file 写出的最终文件路径
    pub filepath_file: Option<String>,
}

/// 下载状态管理（全局共享）
pub struct DownloadState {
    pub processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// 下载任务参数（从前端传入）
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadParams {
    pub id: String,
    pub url: String,
    pub download_dir: String,
    pub download_mode: String,
    pub video_format: Option<String>,
    pub audio_format: Option<String>,
    pub cookie_file: Option<String>,
    pub embed_subs: bool,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub no_merge: bool,
    pub recode_format: Option<String>,
    pub limit_rate: Option<String>,
    pub subtitles: Vec<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub no_playlist: bool,
    pub playlist_items: Option<String>,
}

// ========== 平台常量 ==========

/// Windows: 隐藏控制台窗口标志
#[cfg(target_os = "windows")]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;
