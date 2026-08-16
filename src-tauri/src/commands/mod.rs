//! Tauri 命令模块
//!
//! 按功能域拆分:
//! - support: 命令处理器共享辅助函数
//! - external_tools: 平台信息与 yt-dlp/Deno/FFmpeg 安装管理
//! - video: 视频信息获取、Cookie 管理
//! - download: 下载任务控制
//! - toolbox: 工具箱命令（封面、字幕、弹幕等）

mod download;
mod external_tools;
mod proxy;
pub(crate) mod support;
mod toolbox;
mod video;

// 使用 glob 导出：Tauri generate_handler! 宏需要访问 __cmd__ 隐藏项
pub use download::*;
pub use external_tools::*;
pub use proxy::*;
pub use toolbox::*;
pub use video::*;

// ========== 平台常量 ==========

/// Windows: 隐藏控制台窗口标志
#[cfg(target_os = "windows")]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;
