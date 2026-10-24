//! GET：查询类接口。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{
    self, GetCategoriesReq, GetConfigsReq, GetFunctionsReq, GetInstallReq, GetLibsReq,
    GetPluginReq, GetVersionsReq,
};
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn get_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input(json_in, json_out, plugin::get_plugins)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetPluginReq, _>(json_in, json_out, plugin::get_plugin)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_categories(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetCategoriesReq, _>(json_in, json_out, plugin::get_categories)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_versions(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetVersionsReq, _>(json_in, json_out, plugin::get_versions)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_configs(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetConfigsReq, _>(json_in, json_out, plugin::get_configs)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_install(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetInstallReq, _>(json_in, json_out, plugin::get_install)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_libs(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetLibsReq, _>(json_in, json_out, plugin::get_libs)
}

#[unsafe(no_mangle)]
pub extern "C" fn get_functions(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetFunctionsReq, _>(json_in, json_out, plugin::get_functions)
}
