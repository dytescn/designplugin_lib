//! POST 别名：cyctron 用 `<method>_<FFI-Symbol>` 查找符号，
//! 而浏览器 fetch 不能给 GET 带 body，因此查询/更新/部分更新统一走 POST。

use crate::utils::cjson::with_input;
use crate::utils::plugin::{
    self, DeleteSkillCategoryReq, DeleteSkillReq, GetCategoriesReq, GetConfigsReq,
    GetFunctionsReq, GetInstallReq, GetLibsReq, GetPluginReq, GetPluginsReq,
    GetSkillCategoriesReq, GetSkillReq, GetSkillsReq, GetVersionsReq, PatchConfigReq,
    PatchFunctionReq, PatchInstallReq, PatchPluginReq, UpdateCategoryReq, UpdateConfigReq,
    UpdatePluginReq, UpdateSkillCategoryReq, UpdateSkillReq, UpdateVersionReq,
};
use std::ffi::{c_char, c_int};

macro_rules! post_alias {
    ($name:ident, $target:ident, $req:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(json_in: *const c_char, json_out: *mut c_char) -> c_int {
            with_input::<$req, _>(json_in, json_out, plugin::$target)
        }
    };
}

// GET
post_alias!(post_get_plugins, get_plugins, GetPluginsReq);
post_alias!(post_get_plugin, get_plugin, GetPluginReq);
post_alias!(post_get_categories, get_categories, GetCategoriesReq);
post_alias!(post_get_versions, get_versions, GetVersionsReq);
post_alias!(post_get_configs, get_configs, GetConfigsReq);
post_alias!(post_get_install, get_install, GetInstallReq);
post_alias!(post_get_libs, get_libs, GetLibsReq);
post_alias!(post_get_functions, get_functions, GetFunctionsReq);

// PATCH
post_alias!(post_patch_plugin, patch_plugin, PatchPluginReq);
post_alias!(post_patch_install, patch_install, PatchInstallReq);
post_alias!(post_patch_config, patch_config, PatchConfigReq);
post_alias!(post_patch_function, patch_function, PatchFunctionReq);

// UPDATE
post_alias!(post_update_plugin, update_plugin, UpdatePluginReq);
post_alias!(post_update_category, update_category, UpdateCategoryReq);
post_alias!(post_update_version, update_version, UpdateVersionReq);
post_alias!(post_update_config, update_config, UpdateConfigReq);

// 技能
post_alias!(
    post_get_skill_categories,
    get_skill_categories,
    GetSkillCategoriesReq
);
post_alias!(post_get_skills, get_skills, GetSkillsReq);
post_alias!(post_get_skill, get_skill, GetSkillReq);
post_alias!(post_update_skill, update_skill, UpdateSkillReq);
post_alias!(
    post_update_skill_category,
    update_skill_category,
    UpdateSkillCategoryReq
);
post_alias!(post_delete_skill, delete_skill, DeleteSkillReq);
post_alias!(
    post_delete_skill_category,
    delete_skill_category,
    DeleteSkillCategoryReq
);
