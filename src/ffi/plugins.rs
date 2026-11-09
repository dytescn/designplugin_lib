//! 插件业务：仅暴露插件主表的查询、新增、部分更新、全量更新与删除。

use crate::ffi::types::{
    DeletePluginReq, GetPluginReq, GetPluginsReq, PatchPluginReq, PostPluginReq, UpdatePluginReq,
};
use crate::utils::cjson::with_input;
use crate::utils::error::{AppError, AppResult};
use crate::utils::plugin;
use serde_json::{Value, from_value};
use std::ffi::{c_char, c_int};

fn has_text(req: &Value, key: &str) -> bool {
    req.get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

/// 查询插件：带 `uuid` / `name` 返回详情，否则返回分页列表。
fn query(req: Value) -> AppResult<Value> {
    if has_text(&req, "uuid") || has_text(&req, "name") {
        let req: GetPluginReq =
            from_value(req).map_err(|e| AppError::param(format!("invalid request: {e}")))?;
        plugin::get_plugin(req)
    } else {
        let req: GetPluginsReq =
            from_value(req).map_err(|e| AppError::param(format!("invalid request: {e}")))?;
        plugin::get_plugins(req)
    }
}

/* input: 列表 {page?, page_size?, keyword?, status?, target?, platform?}
          详情 {uuid} 或 {name}
   output: 列表 {list, total, page, page_size}
           详情 插件行 */
#[unsafe(no_mangle)]
pub extern "C" fn get_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input(json_in, json_out, query)
}

/* input: {name（必填）, uuid?, ouid?, plugins_type?, desc?, create_by?, cover?, status?,
           public?, updated_by?, target?, platform?, mask?, enabled?}
   output: {uuid} */
#[unsafe(no_mangle)]
pub extern "C" fn post_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostPluginReq, _>(json_in, json_out, plugin::post_plugin)
}

/* input: {uuid（必填）, enabled?, status?, cover?, mask?}（至少一个）
   output: {updated: true} */
#[unsafe(no_mangle)]
pub extern "C" fn patch_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PatchPluginReq, _>(json_in, json_out, plugin::patch_plugin)
}

/* input: {uuid（必填）, name（必填）, ouid?, plugins_type?, desc?, create_by?, cover?,
           status?, public?, updated_by?, target?, platform?, mask?}
   output: {updated: true} */
#[unsafe(no_mangle)]
pub extern "C" fn update_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdatePluginReq, _>(json_in, json_out, plugin::update_plugin)
}

/* input: {plugin_uuid} 或 {name}
   output: {plugin_uuid, name, deleted: true} */
#[unsafe(no_mangle)]
pub extern "C" fn delete_plugins(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<DeletePluginReq, _>(json_in, json_out, plugin::delete_plugin)
}
