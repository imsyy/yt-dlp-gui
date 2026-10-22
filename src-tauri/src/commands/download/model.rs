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
    pub errors: DownloadErrors,
}

/// Keep diagnostics bounded even for playlists with thousands of failures.
#[derive(Default)]
pub struct DownloadErrors {
    count: usize,
    recent: Vec<String>,
}

impl DownloadErrors {
    pub fn record(&mut self, line: &str) {
        self.count += 1;
        if self.recent.len() == 10 {
            self.recent.remove(0);
        }
        self.recent.push(line.to_string());
    }

    pub fn summary(&self) -> Option<String> {
        match self.count {
            0 => None,
            1 => self.recent.first().cloned(),
            count => Some(format!(
                "err_download_errors:{}\n{}",
                count,
                self.recent.join("\n")
            )),
        }
    }
}

#[cfg(test)]
mod error_tests {
    use super::DownloadErrors;

    #[test]
    fn keeps_single_errors_compatible_and_summarizes_multiple_failures() {
        let mut errors = DownloadErrors::default();
        assert_eq!(errors.summary(), None);
        errors.record("ERROR: [youtube] first: unavailable");
        assert_eq!(
            errors.summary().unwrap(),
            "ERROR: [youtube] first: unavailable"
        );
        errors.record("ERROR: [youtube] second: HTTP Error 403");
        let summary = errors.summary().unwrap();
        assert!(summary.starts_with("err_download_errors:2\n"));
        assert!(summary.contains("first: unavailable"));
        assert!(summary.contains("second: HTTP Error 403"));
    }

    #[test]
    fn counts_all_errors_but_only_keeps_the_latest_ten_details() {
        let mut errors = DownloadErrors::default();
        for index in 0..100 {
            errors.record(&format!("ERROR: video-{} failed", index));
        }
        let summary = errors.summary().unwrap();
        assert!(summary.starts_with("err_download_errors:100\n"));
        assert_eq!(summary.lines().count(), 11);
        assert!(summary.contains("video-99 failed"));
        assert!(!summary.contains("video-89 failed"));
    }
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
#[derive(Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DownloadParams {
    pub id: String,
    pub url: String,
    pub download_dir: String,
    pub download_mode: String,
    pub video_format: Option<String>,
    pub video_has_audio: bool,
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
    /// 下载封面为单独文件（--write-thumbnail），与嵌入封面相互独立
    pub write_thumbnail: bool,
    /// 下载简介为单独文件（--write-description）
    pub write_description: bool,
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
    /// 重新混流 / 合并封装格式（--remux-video / --merge-output-format）
    pub remux_format: Option<String>,
    pub limit_rate: Option<String>,
    /// 自定义 FFmpeg 后处理参数（--postprocessor-args）
    pub ffmpeg_args: Option<String>,
    /// 自定义 yt-dlp 附加参数（如 --sleep-requests、--sleep-interval）
    pub custom_args: Option<String>,
    pub subtitles: Vec<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub no_playlist: bool,
    pub playlist_items: Option<String>,
    /// 从开始下载直播流（--live-from-start）
    pub live_from_start: bool,
}
