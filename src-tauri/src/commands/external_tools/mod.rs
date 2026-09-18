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
///
/// `installed` 与 `runnable` 是两件事，不能混为一谈：
/// 可执行文件存在但跑不起来（缺少动态库、架构不符、权限不足等）时，
/// 若把 `installed` 置为 false，界面会显示「未安装」并引导用户反复下载，
/// 而文件其实一直在那里。因此这里拆成两个字段，由界面分别呈现。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub installed: bool,
    /// 可执行文件能成功启动并输出版本号
    pub runnable: bool,
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
