//! 外部工具的探测、安装和升级命令。

mod deno;
mod ffmpeg;
mod plugins;
mod settings;
mod support;
mod updates;
mod ytdlp;

pub use deno::*;
pub use ffmpeg::*;
pub use plugins::*;
pub use settings::*;
pub use updates::*;
pub use ytdlp::*;

/// 外部工具安装状态。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub installed: bool,
    pub version: String,
    pub path: String,
    pub source: String,
    pub is_managed: bool,
    pub can_update: bool,
}

/// 工具安装或更新进度。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolProgress {
    pub tool: String,
    pub operation: String,
    pub stage: String,
    pub percent: Option<f64>,
}
