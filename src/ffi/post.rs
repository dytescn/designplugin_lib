//! POST：新增接口。

use crate::utils::cjson::with_input;
use crate::utils::installer::{self, InstallReq, UninstallReq};
use crate::utils::plugin::{
    self, PostCategoryReq, PostConfigReq, PostPluginReq, PostSkillCategoryReq, PostSkillReq,
    PostVersionReq,
};
use crate::utils::scanner::{self, ScanReq};
use crate::utils::updates;
use serde_json::Value;
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

#[unsafe(no_mangle)]
pub extern "C" fn post_skill(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostSkillReq, _>(json_in, json_out, plugin::post_skill)
}

#[unsafe(no_mangle)]
pub extern "C" fn post_skill_category(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostSkillCategoryReq, _>(
        json_in,
        json_out,
        plugin::post_skill_category,
    )
}

/// 扫描已安装的设计插件目录并登记到数据库。
#[unsafe(no_mangle)]
pub extern "C" fn post_scan(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<ScanReq, _>(json_in, json_out, scanner::scan)
}

/// 检查设计插件更新（同步接口；SSE 见 `check_updates_sse`）。
#[unsafe(no_mangle)]
pub extern "C" fn post_check_updates(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input(json_in, json_out, |_req: Value| updates::check_updates())
}

/// 安装设计插件（从云端下载并登记）。
#[unsafe(no_mangle)]
pub extern "C" fn post_install(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<InstallReq, _>(json_in, json_out, installer::install)
}

/// 卸载设计插件（删除安装目录、库/函数记录，并清理内容一致的旧 libary 副本）。
#[unsafe(no_mangle)]
pub extern "C" fn post_uninstall(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UninstallReq, _>(json_in, json_out, installer::uninstall)
}

/// 更新设计插件（重新下载当前云端版本）。
#[unsafe(no_mangle)]
pub extern "C" fn post_update(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<InstallReq, _>(json_in, json_out, installer::install)
}

/// 物理删除插件元数据与安装文件。
#[unsafe(no_mangle)]
pub extern "C" fn post_delete(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UninstallReq, _>(json_in, json_out, installer::delete)
}
