//! 任务流业务：整体读写任务流数组。

use crate::utils::cjson::with_input;
use crate::utils::taskflow::{self, GetTaskflowReq, UpdateTaskflowReq};
use std::ffi::{c_char, c_int};

/* input: {}
   output: {taskflows: [...]} */
#[unsafe(no_mangle)]
pub extern "C" fn get_taskflow(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetTaskflowReq, _>(json_in, json_out, taskflow::get_taskflow)
}

/* input: {taskflows: [...]}（整体覆盖）
   output: {taskflows: [...]} */
#[unsafe(no_mangle)]
pub extern "C" fn update_taskflow(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateTaskflowReq, _>(json_in, json_out, taskflow::update_taskflow)
}
