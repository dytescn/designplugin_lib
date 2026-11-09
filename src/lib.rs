//! plugin_lib：设计插件数据接口库。
//!
//! 只做数据层：插件、能力（func）、技能、提示词、任务流与插件包安装/更新。
//! 设计软件的发现与启动由 `designsoft` 负责，向量与检索由 `chatllm` 负责。

pub mod ffi;
pub mod utils;