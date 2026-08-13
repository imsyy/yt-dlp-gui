//! 后端共享基础设施，按来源配置、路径、命令参数和发行地址分层。

mod arguments;
mod paths;
mod releases;
mod source;

pub use arguments::*;
pub use paths::*;
pub use releases::*;
pub use source::*;
