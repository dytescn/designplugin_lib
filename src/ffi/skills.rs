//! 技能业务：技能的查询、新增、更新与删除。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{
    self, DeleteSkillReq, GetSkillsReq, PostSkillReq, UpdateSkillReq,
};
use std::ffi::{c_char, c_int};

/* input: {categories?, ids?, plugin_uuid?, enabled?, with_content?, page?, page_size?}
   output: {list, total, page, page_size} */
#[unsafe(no_mangle)]
pub extern "C" fn get_skills(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<GetSkillsReq, _>(json_in, json_out, plugin::get_skills)
}

/* input: {id（必填）, plugin_uuid（必填）, name（必填）, category（必填）, description?,
           content?, trigger?, params_schema?, is_system?, is_enabled?, sort?, version?}
   output: 新增后的技能文档 */
#[unsafe(no_mangle)]
pub extern "C" fn post_skills(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<PostSkillReq, _>(json_in, json_out, plugin::post_skill)
}

/* input: {id（必填）, plugin_uuid?, name?, description?, category?, content?, trigger?,
           params_schema?, is_system?, is_enabled?, sort?}
   output: 更新后的技能文档（version 自增） */
#[unsafe(no_mangle)]
pub extern "C" fn update_skills(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateSkillReq, _>(json_in, json_out, plugin::update_skill)
}

/* input: {id（必填）, plugin_uuid?, name?, description?, category?, content?, trigger?,
           params_schema?, is_system?, is_enabled?, sort?}
   output: 更新后的技能文档（version 自增） */
#[unsafe(no_mangle)]
pub extern "C" fn patch_skills(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<UpdateSkillReq, _>(json_in, json_out, plugin::update_skill)
}

/* input: {id}
   output: {id, deleted: true} */
#[unsafe(no_mangle)]
pub extern "C" fn delete_skills(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<DeleteSkillReq, _>(json_in, json_out, plugin::delete_skill)
}
