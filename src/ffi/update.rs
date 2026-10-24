//! UPDATE：全量更新接口。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{
    self, UpdateCategoryReq, UpdateConfigReq, UpdatePluginReq, UpdateVersionReq,
};
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn update_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdatePluginReq, _>(json_in, json_out, plugin::update_plugin)
}

#[unsafe(no_mangle)]
pub extern "C" fn update_category(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateCategoryReq, _>(json_in, json_out, plugin::update_category)
}

#[unsafe(no_mangle)]
pub extern "C" fn update_version(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateVersionReq, _>(json_in, json_out, plugin::update_version)
}

#[unsafe(no_mangle)]
pub extern "C" fn update_config(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateConfigReq, _>(json_in, json_out, plugin::update_config)
}
