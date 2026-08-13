//! 下载任务命令域。

mod arguments;
mod control;
mod files;
mod lifecycle;
mod model;
mod output;
mod parser;

pub use control::*;
pub use files::*;
pub use lifecycle::*;
pub use model::DownloadState;
