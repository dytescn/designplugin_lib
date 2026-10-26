//! SSE 符号：供 cyctron 的 `/sse?lib=plugin_lib&symbol=check_updates_sse` 调用。
//!
//! 自定义 SSE 模式会把本函数的输出原样作为事件 `data`，
//! webdx 的 `unwrapSseData` 会取出其中的 `data` 字段。

use crate::utils::cjson::with_input;
use crate::utils::updates;
use serde_json::Value;
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn check_updates_sse(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input(json_in, json_out, |_req: Value| updates::check_updates())
}
