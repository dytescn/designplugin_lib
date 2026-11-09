//! 提示词业务：按 `plugin_uuid` 整体读写提示词数组。

use crate::utils::cjson::with_input;
use crate::utils::prompts::{self, GetPromptsReq, UpdatePromptsReq};
use std::ffi::{c_char, c_int};

/* input: {plugin_uuid（必填）}
   output: {plugin_uuid, prompts: [...]} */
#[unsafe(no_mangle)]
pub extern "C" fn get_prompts(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetPromptsReq, _>(json_in, json_out, prompts::get_prompts)
}

/* input: {plugin_uuid（必填）, prompts: [...]}（整体覆盖）
   output: {plugin_uuid, prompts: [...]} */
#[unsafe(no_mangle)]
pub extern "C" fn update_prompts(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdatePromptsReq, _>(json_in, json_out, prompts::update_prompts)
}
