//! PATCH：部分更新接口。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{self, PatchConfigReq, PatchFunctionReq, PatchInstallReq, PatchPluginReq};
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn patch_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchPluginReq, _>(json_in, json_out, plugin::patch_plugin)
}

#[unsafe(no_mangle)]
pub extern "C" fn patch_install(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchInstallReq, _>(json_in, json_out, plugin::patch_install)
}

#[unsafe(no_mangle)]
pub extern "C" fn patch_config(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchConfigReq, _>(json_in, json_out, plugin::patch_config)
}

#[unsafe(no_mangle)]
pub extern "C" fn patch_function(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchFunctionReq, _>(json_in, json_out, plugin::patch_function)
}
