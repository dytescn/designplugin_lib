//! 方法回调 / 能力业务：查询、新增、全量更新、部分更新与删除。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{
    self, DeleteFunctionReq, GetFunctionsReq, PatchFunctionReq, PostFunctionReq, UpdateFunctionReq,
};
use std::ffi::{c_char, c_int};

/* input: {uuids?, plugin_uuid?, page?, page_size?}
   output: {list, total, page, page_size} */
#[unsafe(no_mangle)]
pub extern "C" fn get_functions(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetFunctionsReq, _>(json_in, json_out, plugin::get_functions)
}

/* input: {uuid（必填）, plugin_uuid（必填）, ffi_symbol（必填）, title（必填）,
           description（必填）, params_schema?, params_example?, return_schema?, return_example?,
           use_ai?}
   output: 新增后的能力行 */
#[unsafe(no_mangle)]
pub extern "C" fn post_function(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostFunctionReq, _>(json_in, json_out, plugin::post_function)
}

/* input: {uuid（必填）, plugin_uuid（必填）, ffi_symbol（必填）, title（必填）,
           description（必填）, params_schema?, params_example?, return_schema?, return_example?,
           use_ai?}
   output: 更新后的能力行 */
#[unsafe(no_mangle)]
pub extern "C" fn update_function(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateFunctionReq, _>(json_in, json_out, plugin::update_function)
}

/* input: {uuid（必填）, plugin_uuid?, ffi_symbol?, title?, description?, params_schema?,
           params_example?, return_schema?, return_example?, use_ai?}（至少一个）
   output: 更新后的能力行 */
#[unsafe(no_mangle)]
pub extern "C" fn patch_function(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchFunctionReq, _>(json_in, json_out, plugin::patch_function)
}

/* input: {uuid（必填）}
   output: {uuid, deleted: true} */
#[unsafe(no_mangle)]
pub extern "C" fn delete_function(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<DeleteFunctionReq, _>(json_in, json_out, plugin::delete_function)
}
