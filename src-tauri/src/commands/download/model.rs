//! 下载任务参数与运行时状态。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    /// 时间裁剪的片段时长（秒），用于计算 ffmpeg 处理进度
    pub clip_duration: Option<f64>,
    /// yt-dlp 输出的最后一条 ERROR，用于替代无意义的退出码错误。
    pub last_error: Option<String>,
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
    /// 从浏览器读取 Cookie 的浏览器名称
    pub cookie_browser: Option<String>,
    /// 代理地址
    pub proxy: Option<String>,
    /// 文件名模板
    pub output_template: Option<String>,
    /// 并发分片数
    pub concurrent_fragments: Option<u32>,
    /// 不覆盖已有文件
    pub no_overwrites: bool,
    pub embed_subs: bool,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    /// 嵌入章节标记
    pub embed_chapters: bool,
    /// 移除赞助片段（SponsorBlock）
    pub sponsorblock_remove: bool,
    /// 提取音频模式（-x）
    pub extract_audio: bool,
    /// 音频转换格式（--audio-format）
    pub audio_convert_format: Option<String>,
    pub no_merge: bool,
    pub recode_format: Option<String>,
    pub limit_rate: Option<String>,
    /// 自定义 FFmpeg 后处理参数（--postprocessor-args）
    pub ffmpeg_args: Option<String>,
    pub subtitles: Vec<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub no_playlist: bool,
    pub playlist_items: Option<String>,
}
