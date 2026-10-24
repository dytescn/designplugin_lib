//! 对外 FFI 接口：只放参数解析、调用与响应，业务逻辑在 `utils`。

pub mod aliases;
pub mod get;
pub mod patch;
pub mod post;
pub mod update;
