//! 频道/主播视频归档工具后端命令实现。
//!
//! 按职责拆分：
//! - extract: 纯解析辅助函数（URL/平台/元数据/日期，无副作用）
//! - fast: 直接 HTTP 详情快查（watch 页 / B 站官方 API，无副作用，可单测）
//! - sync: 频道 CRUD 与列表同步任务
//! - enrich: 发布日期校准任务（快查优先，yt-dlp 兜底）

mod enrich;
mod extract;
mod fast;
mod sync;

pub use enrich::*;
pub use sync::*;
