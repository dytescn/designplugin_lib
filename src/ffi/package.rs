//! 插件包安装 / 更新：按目录读取 dytes.json 并同步插件数据。

use crate::utils::cjson::with_input;
use crate::utils::package::{self, InstallPackageReq, UpdatePackageReq};
use std::ffi::{c_char, c_int};

/* input: {dir（必填）}
   output: {plugin_uuid, name, version, install_path, platform, lib_file, counts} */
#[unsafe(no_mangle)]
pub extern "C" fn install_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<InstallPackageReq, _>(json_in, json_out, package::install)
}

/* input: {dir（必填）, plugin_uuid?, name?, prune?}
   output: {plugin_uuid, name, from_version, to_version, install_path, platform,
            lib_file, updated, added, removed} */
#[unsafe(no_mangle)]
pub extern "C" fn update_plugin(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdatePackageReq, _>(json_in, json_out, package::update)
}
