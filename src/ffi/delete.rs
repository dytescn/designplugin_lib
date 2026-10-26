//! DELETE：技能 / 技能分类的物理删除。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{self, DeleteSkillCategoryReq, DeleteSkillReq};
use std::ffi::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn delete_skill(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<DeleteSkillReq, _>(json_in, json_out, plugin::delete_skill)
}

#[unsafe(no_mangle)]
pub extern "C" fn delete_skill_category(json_in: *const c_char, json_out: *mut c_char) -> c_int {
    with_input::<DeleteSkillCategoryReq, _>(
        json_in,
        json_out,
        plugin::delete_skill_category,
    )
}
