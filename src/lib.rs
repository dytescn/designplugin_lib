//! plugin_lib：设计插件数据接口库。
//!
//! 只做数据层：插件 / 分类 / 版本 / 配置 / 安装状态 / 动态库 / FFI 能力。
//! 设计软件的发现与启动由 `designsoft` 负责，向量与检索由 `chatllm` 负责。

pub mod ffi;
pub mod utils;

pub use crate::ffi::aliases::*;
pub use crate::ffi::delete::*;
pub use crate::ffi::get::*;
pub use crate::ffi::patch::*;
pub use crate::ffi::post::*;
pub use crate::ffi::sse::*;
pub use crate::ffi::update::*;
