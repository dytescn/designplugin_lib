//! 插件接口的请求参数结构。

use serde::Deserialize;
use serde_json::Value;

/// 插件列表查询。
#[derive(Debug, Default, Deserialize)]
pub struct GetPluginsReq {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

/// 插件详情查询。
#[derive(Debug, Default, Deserialize)]
pub struct GetPluginReq {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// 新增插件。
#[derive(Debug, Default, Deserialize)]
pub struct PostPluginReq {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ouid: Option<String>,
    #[serde(default)]
    pub plugins_type: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "desc")]
    pub description: Option<String>,
    #[serde(default)]
    pub create_by: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "public")]
    pub public_flag: Option<i64>,
    #[serde(default)]
    pub updated_by: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub mask: Option<i64>,
    #[serde(default)]
    pub enabled: Option<i64>,
}

/// 全量更新插件。
#[derive(Debug, Default, Deserialize)]
pub struct UpdatePluginReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ouid: Option<String>,
    #[serde(default)]
    pub plugins_type: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "desc")]
    pub description: Option<String>,
    #[serde(default)]
    pub create_by: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "public")]
    pub public_flag: Option<i64>,
    #[serde(default)]
    pub updated_by: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub mask: Option<i64>,
}

/// 部分更新插件。
#[derive(Debug, Default, Deserialize)]
pub struct PatchPluginReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub enabled: Option<Value>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub mask: Option<i64>,
}

/// 删除插件。
#[derive(Debug, Default, Deserialize)]
pub struct DeletePluginReq {
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}
