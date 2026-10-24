//! POST：新增接口。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{self, PostCategoryReq, PostConfigReq, PostPluginReq, PostVersionReq};
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn post_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostPluginReq, _>(json_in, json_out, plugin::post_plugin)
}

#[unsafe(no_mangle)]
pub extern "C" fn post_category(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostCategoryReq, _>(json_in, json_out, plugin::post_category)
}

#[unsafe(no_mangle)]
pub extern "C" fn post_version(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostVersionReq, _>(json_in, json_out, plugin::post_version)
}

#[unsafe(no_mangle)]
pub extern "C" fn post_config(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostConfigReq, _>(json_in, json_out, plugin::post_config)
}
