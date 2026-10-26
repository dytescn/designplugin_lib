//! 插件、分类、版本、配置、安装状态、动态库与能力的数据逻辑。
//!
//! 全部 SQL 在 Rust 内拼装并转义单引号；不直接链接 SQLite。
//! 删除 / 安装 / 扫描属于 M4，本模块先覆盖 M0–M3 的 CRUD。

use crate::utils::cjson::flag;
use crate::utils::config::{self, PLUGIN_DB};
use crate::utils::db::{self, int_opt, like_pattern, quote, quote_opt, query_i64};
use crate::utils::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::utils::config::plugin_dir;

const PLUGIN_STATUSES: &[&str] = &["draft", "review", "published", "offline", "deprecated"];
const INSTALL_STATUSES: &[&str] = &["installing", "installed", "running", "stopped", "broken"];
const EMBED_STATUSES: &[i64] = &[0, 1, 2, 3];
const MAX_SKILL_NAME_CHARS: usize = 120;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 500;
const MAX_SKILL_CONTENT_CHARS: usize = 8000;
const SKILL_TEMPLATE_TOKENS: &[&str] = &[
    "{{base_system_prompt}}",
    "{{categories}}",
    "{{skills}}",
    "{{rules}}",
];

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn page_params(page: Option<u32>, page_size: Option<u32>) -> (u32, u32, u32) {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 200);
    let offset = (page - 1).saturating_mul(page_size);
    (page, page_size, offset)
}

fn require_text(value: &str, field: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::param(format!("{field} is required")));
    }
    Ok(())
}

fn validate_one_of(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::param(format!(
            "{field} must be one of: {}",
            allowed.join(", ")
        )))
    }
}

fn validate_optional(value: Option<&str>, allowed: &[&str], field: &str) -> AppResult<()> {
    match value.filter(|v| !v.is_empty()) {
        Some(v) => validate_one_of(v, allowed, field),
        None => Ok(()),
    }
}

fn validate_skill_fields(
    name: &str,
    description: Option<&str>,
    content: Option<&str>,
    params_schema: Option<&Value>,
) -> AppResult<()> {
    if name.chars().count() > MAX_SKILL_NAME_CHARS {
        return Err(AppError::param(format!(
            "name 不能超过 {MAX_SKILL_NAME_CHARS} 个字符"
        )));
    }
    if let Some(description) = description
        && description.chars().count() > MAX_SKILL_DESCRIPTION_CHARS
    {
        return Err(AppError::param(format!(
            "description 不能超过 {MAX_SKILL_DESCRIPTION_CHARS} 个字符"
        )));
    }
    if let Some(content) = content {
        if content.chars().count() > MAX_SKILL_CONTENT_CHARS {
            return Err(AppError::param(format!(
                "content 不能超过 {MAX_SKILL_CONTENT_CHARS} 个字符"
            )));
        }
        if content
            .chars()
            .any(|c| c == '\0' || (c.is_control() && c != '\n' && c != '\r' && c != '\t'))
        {
            return Err(AppError::param("content 不能包含控制字符"));
        }
        for token in SKILL_TEMPLATE_TOKENS {
            if content.contains(token) {
                return Err(AppError::param(format!(
                    "content 不能包含模板占位符: {token}"
                )));
            }
        }
    }
    if let Some(schema) = params_schema
        && !schema.is_object()
    {
        return Err(AppError::param("params_schema 必须是 JSON 对象"));
    }
    Ok(())
}

fn as_i64(row: &Value, key: &str) -> i64 {
    row.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn as_str<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key).and_then(Value::as_str)
}

fn require_row(sql: &str, what: &str) -> AppResult<Value> {
    db::query(PLUGIN_DB, sql)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found(what.to_string()))
}

fn require_plugin(uuid: &str) -> AppResult<Value> {
    require_row(
        &format!("SELECT * FROM plugins WHERE uuid = {} LIMIT 1", quote(uuid)),
        &format!("插件不存在: {uuid}"),
    )
}

/// `version`（字符串）或 `version_id`（数字）→ `plugin_configs.version_id`。
/// 未指定时为 0（通用配置）。
fn resolve_version_id(
    plugin_uuid: &str,
    version_id: Option<i64>,
    version: Option<&str>,
) -> AppResult<i64> {
    if let Some(id) = version_id {
        return Ok(id);
    }

    let Some(version) = version.filter(|v| !v.is_empty()) else {
        return Ok(0);
    };

    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_versions WHERE plugin_uuid = {} AND version = {} LIMIT 1",
            quote(plugin_uuid),
            quote(version),
        ),
    )?;

    rows.first()
        .and_then(|row| row.get("id"))
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::not_found(format!("版本不存在: {version}")))
}

/// 把 TEXT 列的 JSON 内容还原成对象 / 数组，便于前端直接消费。
fn expand_json_fields(row: &mut Value, fields: &[&str]) {
    let Some(obj) = row.as_object_mut() else {
        return;
    };

    for field in fields {
        let Some(text) = obj.get(*field).and_then(Value::as_str) else {
            continue;
        };
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            obj.insert((*field).to_string(), parsed);
        }
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn int_list(value: &Option<Value>) -> Option<Vec<i64>> {
    let value = value.as_ref()?;
    match value {
        Value::Array(items) => {
            let list: Vec<i64> = items
                .iter()
                .filter_map(|item| match item {
                    Value::Number(n) => n.as_i64(),
                    Value::String(s) => s.parse::<i64>().ok(),
                    _ => None,
                })
                .collect();
            (!list.is_empty()).then_some(list)
        }
        Value::Number(n) => n.as_i64().map(|v| vec![v]),
        Value::String(s) => s.parse::<i64>().ok().map(|v| vec![v]),
        _ => None,
    }
}

fn latest_installs(uuids: &[String]) -> AppResult<HashMap<String, Value>> {
    if uuids.is_empty() {
        return Ok(HashMap::new());
    }

    let in_list = uuids.iter().map(|u| quote(u)).collect::<Vec<_>>().join(", ");
    let rows = db::query(
        PLUGIN_DB,
        &format!("SELECT * FROM plugin_installs WHERE plugin_uuid IN ({in_list}) ORDER BY id DESC"),
    )?;

    let mut map = HashMap::new();
    for row in rows {
        if let Some(uuid) = as_str(&row, "plugin_uuid") {
            map.entry(uuid.to_string()).or_insert(row);
        }
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// 请求结构
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct GetPluginsReq {
    #[serde(default, alias = "category_id")]
    pub category: Option<String>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetPluginReq {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetCategoriesReq {
    #[serde(default)]
    pub parent_uuid: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetVersionsReq {
    #[serde(default)]
    pub plugin_uuid: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetConfigsReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub version_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetInstallReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetLibsReq {
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub local: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetFunctionsReq {
    #[serde(default)]
    pub uuids: Option<Vec<String>>,
    #[serde(default)]
    pub lib_uuid: Option<String>,
    #[serde(default)]
    pub lib_name: Option<String>,
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub local: Option<Value>,
    #[serde(default)]
    pub use_ai: Option<Value>,
    #[serde(default)]
    pub embed_status: Option<Value>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetSkillCategoriesReq {}

#[derive(Debug, Default, Deserialize)]
pub struct GetSkillsReq {
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub enabled: Option<Value>,
    #[serde(default)]
    pub with_content: Option<bool>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetSkillReq {
    #[serde(default)]
    pub id: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct PostPluginReq {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ouid: Option<String>,
    #[serde(default)]
    pub flow_uid: Option<String>,
    #[serde(default)]
    pub type_uid: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
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
    pub plugin_type: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub enabled: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PostCategoryReq {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub type_name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
    #[serde(default)]
    pub created_by: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PostVersionReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
    #[serde(default)]
    pub file_hash: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub min_engine_ver: Option<String>,
    #[serde(default)]
    pub max_engine_ver: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub is_latest: Option<Value>,
    #[serde(default)]
    pub created_by: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PostConfigReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub param_key: String,
    #[serde(default)]
    pub version_id: Option<i64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub param_type: Option<String>,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub required: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdatePluginReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ouid: Option<String>,
    #[serde(default)]
    pub flow_uid: Option<String>,
    #[serde(default)]
    pub type_uid: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
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
    pub plugin_type: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateCategoryReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub type_name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
    #[serde(default)]
    pub updated_by: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateVersionReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
    #[serde(default)]
    pub file_hash: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub min_engine_ver: Option<String>,
    #[serde(default)]
    pub max_engine_ver: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub is_latest: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateConfigReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub param_key: String,
    #[serde(default)]
    pub param_type: Option<String>,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub required: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchPluginReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub enabled: Option<Value>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchInstallReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub install_path: Option<String>,
    #[serde(default)]
    pub enabled: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchConfigReq {
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub version_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchFunctionReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub use_ai: Option<Value>,
    #[serde(default)]
    pub embed_status: Option<Value>,
    #[serde(default)]
    pub embed_model: Option<String>,
    #[serde(default)]
    pub embed_text_hash: Option<String>,
    #[serde(default)]
    pub embedded_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

pub fn get_plugins(req: GetPluginsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);

    let mut conditions = vec!["deleted_at IS NULL".to_string()];

    if let Some(category) = req.category.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("category_id = {}", quote(category)));
    }
    if let Some(keyword) = req.keyword.as_deref().filter(|v| !v.is_empty()) {
        let pattern = like_pattern(keyword);
        conditions.push(format!(
            "(name LIKE {pattern} ESCAPE '\\' OR \"desc\" LIKE {pattern} ESCAPE '\\')"
        ));
    }
    if let Some(status) = req.status.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("status = {}", quote(status)));
    }
    if let Some(target) = req.target.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("target = {}", quote(target)));
    }

    let where_sql = format!(" WHERE {}", conditions.join(" AND "));

    let total = query_i64(
        PLUGIN_DB,
        &format!("SELECT COUNT(*) AS total FROM plugins{where_sql}"),
        "total",
    )?;

    let mut list = db::query(
        PLUGIN_DB,
        &format!("SELECT * FROM plugins{where_sql} ORDER BY id DESC LIMIT {page_size} OFFSET {offset}"),
    )?;

    let uuids: Vec<String> = list
        .iter()
        .filter_map(|row| as_str(row, "uuid").map(ToString::to_string))
        .collect();
    let installs = latest_installs(&uuids)?;

    for row in &mut list {
        let uuid = as_str(row, "uuid").unwrap_or_default().to_string();
        let install = installs.get(&uuid).cloned().unwrap_or(Value::Null);
        if let Some(obj) = row.as_object_mut() {
            obj.insert("install".to_string(), install);
        }
    }

    Ok(json!({
        "list": list,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

pub fn get_plugin(req: GetPluginReq) -> AppResult<Value> {
    let (field, value) = match (
        req.uuid.as_deref().filter(|v| !v.is_empty()),
        req.name.as_deref().filter(|v| !v.is_empty()),
    ) {
        (Some(uuid), _) => ("uuid", uuid),
        (None, Some(name)) => ("name", name),
        (None, None) => return Err(AppError::param("uuid or name is required")),
    };

    let plugin = require_row(
        &format!(
            "SELECT * FROM plugins WHERE {field} = {} AND deleted_at IS NULL LIMIT 1",
            quote(value)
        ),
        &format!("插件不存在: {value}"),
    )?;

    let uuid = as_str(&plugin, "uuid").unwrap_or_default().to_string();

    let versions = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_versions WHERE plugin_uuid = {} ORDER BY is_latest DESC, id DESC",
            quote(&uuid)
        ),
    )?;

    let installs = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_installs WHERE plugin_uuid = {} ORDER BY id DESC",
            quote(&uuid)
        ),
    )?;

    Ok(json!({
        "plugin": plugin,
        "versions": versions,
        "installs": installs,
    }))
}

pub fn get_categories(req: GetCategoriesReq) -> AppResult<Value> {
    let mut conditions = vec!["deleted_at IS NULL".to_string()];

    if let Some(parent) = req.parent_uuid.as_deref() {
        if parent.is_empty() {
            conditions.push("(parent_uuid IS NULL OR parent_uuid = '')".to_string());
        } else {
            conditions.push(format!("parent_uuid = {}", quote(parent)));
        }
    }

    let list = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_categories WHERE {} ORDER BY sort_order ASC, id ASC",
            conditions.join(" AND ")
        ),
    )?;

    Ok(json!({ "list": list }))
}

pub fn get_versions(req: GetVersionsReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;

    let list = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_versions WHERE plugin_uuid = {} ORDER BY is_latest DESC, id DESC",
            quote(&req.plugin_uuid)
        ),
    )?;

    Ok(json!({ "list": list }))
}

pub fn get_configs(req: GetConfigsReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    let version_id = resolve_version_id(&req.plugin_uuid, req.version_id, req.version.as_deref())?;

    let sql = if version_id > 0 {
        format!(
            "SELECT * FROM plugin_configs WHERE plugin_uuid = {} AND version_id IN (0, {version_id}) \
             ORDER BY sort_order ASC, id ASC",
            quote(&req.plugin_uuid)
        )
    } else {
        format!(
            "SELECT * FROM plugin_configs WHERE plugin_uuid = {} AND version_id = 0 \
             ORDER BY sort_order ASC, id ASC",
            quote(&req.plugin_uuid)
        )
    };

    let rows = db::query(PLUGIN_DB, &sql)?;

    // 同一 param_key 同时存在通用配置和版本配置时，版本配置优先。
    let mut chosen: Vec<Value> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();

    for row in rows {
        let key = as_str(&row, "param_key").unwrap_or_default().to_string();
        let row_version = as_i64(&row, "version_id");

        match positions.get(&key).copied() {
            Some(index) => {
                if version_id > 0 && row_version == version_id {
                    chosen[index] = row;
                }
            }
            None => {
                positions.insert(key, chosen.len());
                chosen.push(row);
            }
        }
    }

    let user_values = config::get_user_configs(&req.plugin_uuid)?;

    for row in &mut chosen {
        expand_json_fields(row, &["options"]);
        let key = as_str(row, "param_key").unwrap_or_default().to_string();
        let default_value = row.get("default_value").cloned().unwrap_or(Value::Null);

        let (value, source) = match user_values.get(&key) {
            Some(user) => (Value::String(user.clone()), "user"),
            None => (default_value, "default"),
        };

        if let Some(obj) = row.as_object_mut() {
            obj.insert("value".to_string(), value);
            obj.insert("source".to_string(), Value::String(source.to_string()));
        }
    }

    Ok(json!({ "configs": chosen }))
}

pub fn get_install(req: GetInstallReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;

    let filter = match req.version.as_deref().filter(|v| !v.is_empty()) {
        Some(version) => format!(" AND version = {}", quote(version)),
        None => String::new(),
    };

    let install = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_installs WHERE plugin_uuid = {}{filter} ORDER BY id DESC LIMIT 1",
            quote(&req.plugin_uuid)
        ),
    )?
    .into_iter()
    .next()
    .unwrap_or(Value::Null);

    Ok(json!({ "install": install }))
}

pub fn get_libs(req: GetLibsReq) -> AppResult<Value> {
    let mut conditions = Vec::new();

    if let Some(plugin_uuid) = req.plugin_uuid.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("plugin_uuid = {}", quote(plugin_uuid)));
    }
    if let Some(local) = flag(&req.local) {
        conditions.push(format!("is_local = {local}"));
    }

    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let list = db::query(
        PLUGIN_DB,
        &format!("SELECT * FROM plugin_libs{where_sql} ORDER BY name ASC, id ASC"),
    )?;

    Ok(json!({ "list": list }))
}

pub fn get_functions(req: GetFunctionsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);
    let mut conditions = Vec::new();

    if let Some(uuids) = req.uuids.as_ref().filter(|v| !v.is_empty()) {
        let in_list = uuids.iter().map(|u| quote(u)).collect::<Vec<_>>().join(", ");
        conditions.push(format!("uuid IN ({in_list})"));
    }
    if let Some(lib_uuid) = req.lib_uuid.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("lib_uuid = {}", quote(lib_uuid)));
    }
    if let Some(lib_name) = req.lib_name.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("lib_name = {}", quote(lib_name)));
    }
    if let Some(plugin_uuid) = req.plugin_uuid.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!(
            "lib_uuid IN (SELECT uuid FROM plugin_libs WHERE plugin_uuid = {})",
            quote(plugin_uuid)
        ));
    }
    if let Some(local) = flag(&req.local) {
        conditions.push(format!("is_local = {local}"));
    }
    if let Some(use_ai) = flag(&req.use_ai) {
        if !matches!(use_ai, 0 | 1) {
            return Err(AppError::param("use_ai must be 0 or 1"));
        }
        conditions.push(format!("use_ai = {use_ai}"));
    }
    if let Some(statuses) = int_list(&req.embed_status).filter(|v| !v.is_empty()) {
        if statuses.len() == 1 {
            conditions.push(format!("embed_status = {}", statuses[0]));
        } else {
            let list = statuses
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("embed_status IN ({list})"));
        }
    }
    if let Some(category) = req.category.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("category = {}", quote(category)));
    }

    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let total = query_i64(
        PLUGIN_DB,
        &format!("SELECT COUNT(*) AS total FROM plugin_functions{where_sql}"),
        "total",
    )?;

    let mut list = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_functions{where_sql} ORDER BY id ASC \
             LIMIT {page_size} OFFSET {offset}"
        ),
    )?;

    for row in &mut list {
        expand_json_fields(
            row,
            &[
                "params_schema",
                "params_example",
                "return_schema",
                "return_example",
                "error_codes",
            ],
        );
    }

    Ok(json!({
        "list": list,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

// ---------------------------------------------------------------------------
// POST
// ---------------------------------------------------------------------------

pub fn post_plugin(req: PostPluginReq) -> AppResult<Value> {
    require_text(&req.name, "name")?;
    validate_optional(req.status.as_deref(), PLUGIN_STATUSES, "status")?;

    let uuid = req
        .uuid
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(new_uuid);

    if !db::query(
        PLUGIN_DB,
        &format!("SELECT id FROM plugins WHERE uuid = {} LIMIT 1", quote(&uuid)),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!("插件 uuid 已存在: {uuid}")));
    }

    let timestamp = now();
    let sql = format!(
        "INSERT INTO plugins \
         (uuid, ouid, flow_uid, type_uid, category_id, name, \"desc\", create_by, cover, \
          status, public, updated_by, created_at, updated_at, installed, install_path, \
          enabled, plugin_type, target) \
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {timestamp}, {timestamp}, \
                 0, NULL, {}, {}, {})",
        quote(&uuid),
        quote_opt(req.ouid.as_deref()),
        quote_opt(req.flow_uid.as_deref()),
        quote_opt(req.type_uid.as_deref()),
        quote_opt(req.category_id.as_deref()),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        quote_opt(req.create_by.as_deref()),
        quote_opt(req.cover.as_deref()),
        quote(req.status.as_deref().unwrap_or("draft")),
        req.public_flag.unwrap_or(0),
        quote_opt(req.updated_by.as_deref()),
        req.enabled.unwrap_or(1),
        quote(req.plugin_type.as_deref().unwrap_or("dll")),
        quote(req.target.as_deref().unwrap_or("common")),
    );

    db::execute(PLUGIN_DB, &sql)?;
    Ok(json!({ "uuid": uuid }))
}

pub fn post_category(req: PostCategoryReq) -> AppResult<Value> {
    require_text(&req.type_name, "type_name")?;

    if !db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_categories WHERE type_name = {} LIMIT 1",
            quote(&req.type_name)
        ),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!(
            "分类名称已存在: {}",
            req.type_name
        )));
    }

    let uuid = req
        .uuid
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(new_uuid);
    let timestamp = now();

    let sql = format!(
        "INSERT INTO plugin_categories \
         (uuid, type_name, icon, parent_uuid, sort_order, created_by, created_at, updated_at) \
         VALUES ({}, {}, {}, {}, {}, {}, {timestamp}, {timestamp})",
        quote(&uuid),
        quote(&req.type_name),
        quote_opt(req.icon.as_deref()),
        quote_opt(req.parent_uuid.as_deref()),
        req.sort_order.unwrap_or(0),
        quote_opt(req.created_by.as_deref()),
    );

    db::execute(PLUGIN_DB, &sql)?;
    Ok(json!({ "uuid": uuid }))
}

pub fn post_version(req: PostVersionReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.version, "version")?;
    require_plugin(&req.plugin_uuid)?;

    let duplicated = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_versions WHERE plugin_uuid = {} AND version = {} LIMIT 1",
            quote(&req.plugin_uuid),
            quote(&req.version)
        ),
    )?;
    if !duplicated.is_empty() {
        return Err(AppError::conflict(format!(
            "版本已存在: {} {}",
            req.plugin_uuid, req.version
        )));
    }

    let is_latest = flag(&req.is_latest).unwrap_or(0);
    if is_latest == 1 {
        db::update(
            PLUGIN_DB,
            &format!(
                "UPDATE plugin_versions SET is_latest = 0, updated_at = {} WHERE plugin_uuid = {}",
                now(),
                quote(&req.plugin_uuid)
            ),
        )?;
    }

    let timestamp = now();
    let sql = format!(
        "INSERT INTO plugin_versions \
         (plugin_uuid, version, changelog, file_path, file_size, file_hash, file_type, \
          min_engine_ver, max_engine_ver, status, is_latest, created_by, created_at, updated_at) \
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {is_latest}, {}, {timestamp}, {timestamp})",
        quote(&req.plugin_uuid),
        quote(&req.version),
        quote_opt(req.changelog.as_deref()),
        quote_opt(req.file_path.as_deref()),
        int_opt(req.file_size),
        quote_opt(req.file_hash.as_deref()),
        quote(req.file_type.as_deref().unwrap_or("zip")),
        quote_opt(req.min_engine_ver.as_deref()),
        quote_opt(req.max_engine_ver.as_deref()),
        quote(req.status.as_deref().unwrap_or("active")),
        quote_opt(req.created_by.as_deref()),
    );

    db::execute(PLUGIN_DB, &sql)?;

    let id = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_versions WHERE plugin_uuid = {} AND version = {} LIMIT 1",
            quote(&req.plugin_uuid),
            quote(&req.version)
        ),
    )?
    .first()
    .map(|row| as_i64(row, "id"))
    .unwrap_or(0);

    Ok(json!({ "id": id }))
}

pub fn post_config(req: PostConfigReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.param_key, "param_key")?;
    require_plugin(&req.plugin_uuid)?;

    let version_id = resolve_version_id(&req.plugin_uuid, req.version_id, req.version.as_deref())?;

    let duplicated = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_configs WHERE plugin_uuid = {} AND version_id = {version_id} \
             AND param_key = {} LIMIT 1",
            quote(&req.plugin_uuid),
            quote(&req.param_key)
        ),
    )?;
    if !duplicated.is_empty() {
        return Err(AppError::conflict(format!(
            "配置项已存在: {}",
            req.param_key
        )));
    }

    let timestamp = now();
    let required = flag(&req.required).unwrap_or(0);

    let sql = format!(
        "INSERT INTO plugin_configs \
         (plugin_uuid, version_id, param_key, param_type, default_value, required, \
          description, options, sort_order, created_at, updated_at) \
         VALUES ({}, {version_id}, {}, {}, {}, {required}, {}, {}, {}, {timestamp}, {timestamp})",
        quote(&req.plugin_uuid),
        quote(&req.param_key),
        quote(req.param_type.as_deref().unwrap_or("string")),
        quote_opt(req.default_value.as_deref()),
        quote_opt(req.description.as_deref()),
        quote_opt(req.options.as_deref()),
        req.sort_order.unwrap_or(0),
    );

    db::execute(PLUGIN_DB, &sql)?;

    let id = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_configs WHERE plugin_uuid = {} AND version_id = {version_id} \
             AND param_key = {} LIMIT 1",
            quote(&req.plugin_uuid),
            quote(&req.param_key)
        ),
    )?
    .first()
    .map(|row| as_i64(row, "id"))
    .unwrap_or(0);

    Ok(json!({ "id": id }))
}

// ---------------------------------------------------------------------------
// UPDATE（全量）
// ---------------------------------------------------------------------------

pub fn update_plugin(req: UpdatePluginReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    require_text(&req.name, "name")?;
    validate_optional(req.status.as_deref(), PLUGIN_STATUSES, "status")?;
    require_plugin(&req.uuid)?;

    let sql = format!(
        "UPDATE plugins SET ouid = {}, flow_uid = {}, type_uid = {}, category_id = {}, \
         name = {}, \"desc\" = {}, create_by = {}, cover = {}, status = {}, public = {}, \
         updated_by = {}, plugin_type = {}, target = {}, updated_at = {} WHERE uuid = {}",
        quote_opt(req.ouid.as_deref()),
        quote_opt(req.flow_uid.as_deref()),
        quote_opt(req.type_uid.as_deref()),
        quote_opt(req.category_id.as_deref()),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        quote_opt(req.create_by.as_deref()),
        quote_opt(req.cover.as_deref()),
        quote(req.status.as_deref().unwrap_or("draft")),
        req.public_flag.unwrap_or(0),
        quote_opt(req.updated_by.as_deref()),
        quote(req.plugin_type.as_deref().unwrap_or("dll")),
        quote(req.target.as_deref().unwrap_or("common")),
        now(),
        quote(&req.uuid),
    );

    db::update(PLUGIN_DB, &sql)?;
    Ok(json!({ "updated": true }))
}

pub fn update_category(req: UpdateCategoryReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    require_text(&req.type_name, "type_name")?;
    require_row(
        &format!(
            "SELECT id FROM plugin_categories WHERE uuid = {} LIMIT 1",
            quote(&req.uuid)
        ),
        &format!("分类不存在: {}", req.uuid),
    )?;

    let sql = format!(
        "UPDATE plugin_categories SET type_name = {}, icon = {}, parent_uuid = {}, \
         sort_order = {}, updated_by = {}, updated_at = {} WHERE uuid = {}",
        quote(&req.type_name),
        quote_opt(req.icon.as_deref()),
        quote_opt(req.parent_uuid.as_deref()),
        req.sort_order.unwrap_or(0),
        quote_opt(req.updated_by.as_deref()),
        now(),
        quote(&req.uuid),
    );

    db::update(PLUGIN_DB, &sql)?;
    Ok(json!({ "updated": true }))
}

pub fn update_version(req: UpdateVersionReq) -> AppResult<Value> {
    if req.id <= 0 {
        return Err(AppError::param("id is required"));
    }
    require_text(&req.version, "version")?;

    let existing = require_row(
        &format!("SELECT * FROM plugin_versions WHERE id = {} LIMIT 1", req.id),
        &format!("版本不存在: {}", req.id),
    )?;

    let is_latest = flag(&req.is_latest).unwrap_or(0);
    let plugin_uuid = as_str(&existing, "plugin_uuid").unwrap_or_default().to_string();

    if is_latest == 1 && !plugin_uuid.is_empty() {
        db::update(
            PLUGIN_DB,
            &format!(
                "UPDATE plugin_versions SET is_latest = 0, updated_at = {} WHERE plugin_uuid = {}",
                now(),
                quote(&plugin_uuid)
            ),
        )?;
    }

    let sql = format!(
        "UPDATE plugin_versions SET version = {}, changelog = {}, file_path = {}, \
         file_size = {}, file_hash = {}, file_type = {}, min_engine_ver = {}, \
         max_engine_ver = {}, status = {}, is_latest = {is_latest}, updated_at = {} WHERE id = {}",
        quote(&req.version),
        quote_opt(req.changelog.as_deref()),
        quote_opt(req.file_path.as_deref()),
        int_opt(req.file_size),
        quote_opt(req.file_hash.as_deref()),
        quote(req.file_type.as_deref().unwrap_or("zip")),
        quote_opt(req.min_engine_ver.as_deref()),
        quote_opt(req.max_engine_ver.as_deref()),
        quote(req.status.as_deref().unwrap_or("active")),
        now(),
        req.id,
    );

    db::update(PLUGIN_DB, &sql)?;
    Ok(json!({ "updated": true }))
}

pub fn update_config(req: UpdateConfigReq) -> AppResult<Value> {
    if req.id <= 0 {
        return Err(AppError::param("id is required"));
    }
    require_text(&req.param_key, "param_key")?;
    require_row(
        &format!("SELECT id FROM plugin_configs WHERE id = {} LIMIT 1", req.id),
        &format!("配置项不存在: {}", req.id),
    )?;

    let required = flag(&req.required).unwrap_or(0);

    let sql = format!(
        "UPDATE plugin_configs SET param_key = {}, param_type = {}, default_value = {}, \
         required = {required}, description = {}, options = {}, sort_order = {}, \
         updated_at = {} WHERE id = {}",
        quote(&req.param_key),
        quote(req.param_type.as_deref().unwrap_or("string")),
        quote_opt(req.default_value.as_deref()),
        quote_opt(req.description.as_deref()),
        quote_opt(req.options.as_deref()),
        req.sort_order.unwrap_or(0),
        now(),
        req.id,
    );

    db::update(PLUGIN_DB, &sql)?;
    Ok(json!({ "updated": true }))
}

// ---------------------------------------------------------------------------
// PATCH（部分更新）
// ---------------------------------------------------------------------------

pub fn patch_plugin(req: PatchPluginReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    validate_optional(req.status.as_deref(), PLUGIN_STATUSES, "status")?;
    require_plugin(&req.uuid)?;

    let mut sets = Vec::new();

    if let Some(enabled) = flag(&req.enabled) {
        sets.push(format!("enabled = {enabled}"));
    }
    if let Some(status) = req.status.as_deref().filter(|v| !v.is_empty()) {
        sets.push(format!("status = {}", quote(status)));
    }
    if let Some(category_id) = req.category_id.as_deref() {
        sets.push(format!("category_id = {}", quote(category_id)));
    }
    if let Some(cover) = req.cover.as_deref() {
        sets.push(format!("cover = {}", quote(cover)));
    }

    if sets.is_empty() {
        return Err(AppError::param("no fields to update"));
    }

    sets.push(format!("updated_at = {}", now()));

    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugins SET {} WHERE uuid = {}",
            sets.join(", "),
            quote(&req.uuid)
        ),
    )?;

    Ok(json!({ "updated": true }))
}

pub fn patch_install(req: PatchInstallReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    validate_optional(req.status.as_deref(), INSTALL_STATUSES, "status")?;

    let filter = match req.version.as_deref().filter(|v| !v.is_empty()) {
        Some(version) => format!(" AND version = {}", quote(version)),
        None => String::new(),
    };

    let existing = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_installs WHERE plugin_uuid = {}{filter} ORDER BY id DESC LIMIT 1",
            quote(&req.plugin_uuid)
        ),
    )?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::not_found(format!("安装记录不存在: {}", req.plugin_uuid)))?;

    let current_path = as_str(&existing, "install_path").unwrap_or_default().to_string();
    let status = req.status.as_deref().filter(|v| !v.is_empty());
    let mut sets = Vec::new();

    if let Some(status) = status {
        sets.push(format!("status = {}", quote(status)));
        if status == "running" {
            sets.push(format!("started_at = {}", now()));
        }
        if status != "broken" {
            sets.push("last_error = NULL".to_string());
        }
    }
    if let Some(path) = req.install_path.as_deref() {
        sets.push(format!("install_path = {}", quote(path)));
    }
    if let Some(enabled) = flag(&req.enabled) {
        sets.push(format!("enabled = {enabled}"));
    }

    if sets.is_empty() {
        return Err(AppError::param("no fields to update"));
    }

    sets.push(format!("updated_at = {}", now()));

    let install_id = as_i64(&existing, "id");
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_installs SET {} WHERE id = {install_id}",
            sets.join(", ")
        ),
    )?;

    // 兼容字段：plugins.installed / install_path 以 plugin_installs 为准同步。
    if let Some(status) = status {
        let effective_path = req
            .install_path
            .clone()
            .unwrap_or_else(|| current_path.clone());

        if matches!(status, "installed" | "running") {
            db::update(
                PLUGIN_DB,
                &format!(
                    "UPDATE plugins SET installed = 1, install_path = {}, updated_at = {} WHERE uuid = {}",
                    quote(&effective_path),
                    now(),
                    quote(&req.plugin_uuid)
                ),
            )?;
        } else {
            db::update(
                PLUGIN_DB,
                &format!(
                    "UPDATE plugins SET installed = 0, updated_at = {} WHERE uuid = {}",
                    now(),
                    quote(&req.plugin_uuid)
                ),
            )?;
        }
    }

    Ok(json!({ "updated": true }))
}

pub fn patch_config(req: PatchConfigReq) -> AppResult<Value> {
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.key, "key")?;

    let definitions = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugin_configs WHERE plugin_uuid = {} AND param_key = {} \
             ORDER BY version_id DESC, id DESC",
            quote(&req.plugin_uuid),
            quote(&req.key)
        ),
    )?;

    if definitions.is_empty() {
        return Err(AppError::not_found(format!(
            "配置项不存在: {}",
            req.key
        )));
    }

    let version_id = resolve_version_id(&req.plugin_uuid, req.version_id, req.version.as_deref())?;
    let definition = definitions
        .iter()
        .find(|row| as_i64(row, "version_id") == version_id)
        .unwrap_or(&definitions[0]);

    let value_type = as_str(definition, "param_type").unwrap_or("string");
    let title = as_str(definition, "description")
        .filter(|v| !v.is_empty())
        .unwrap_or(&req.key);
    let value = value_text(&req.value);

    config::set_user_config(&req.plugin_uuid, &req.key, &value, value_type, title)?;
    Ok(json!({ "saved": true }))
}

pub fn patch_function(req: PatchFunctionReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    require_row(
        &format!(
            "SELECT id FROM plugin_functions WHERE uuid = {} LIMIT 1",
            quote(&req.uuid)
        ),
        &format!("能力不存在: {}", req.uuid),
    )?;

    let mut sets = Vec::new();

    if let Some(use_ai) = flag(&req.use_ai) {
        if !matches!(use_ai, 0 | 1) {
            return Err(AppError::param("use_ai must be 0 or 1"));
        }
        sets.push(format!("use_ai = {use_ai}"));
    }
    if let Some(status) = flag(&req.embed_status) {
        if !EMBED_STATUSES.contains(&status) {
            return Err(AppError::param(
                "embed_status must be one of: 0, 1, 2, 3",
            ));
        }
        sets.push(format!("embed_status = {status}"));
    }
    if let Some(model) = req.embed_model.as_deref() {
        sets.push(format!("embed_model = {}", quote(model)));
    }
    if let Some(hash) = req.embed_text_hash.as_deref() {
        sets.push(format!("embed_text_hash = {}", quote(hash)));
    }
    if let Some(embedded_at) = req.embedded_at {
        sets.push(format!("embedded_at = {embedded_at}"));
    }

    if sets.is_empty() {
        return Err(AppError::param("no fields to update"));
    }

    sets.push(format!("updated_at = {}", now()));

    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_functions SET {} WHERE uuid = {}",
            sets.join(", "),
            quote(&req.uuid)
        ),
    )?;

    Ok(json!({ "updated": true }))
}

// ---------------------------------------------------------------------------
// 技能（skills / skill_categories）
// ---------------------------------------------------------------------------

fn require_skill_category(code: &str) -> AppResult<()> {
    require_text(code, "category")?;
    if db::query(
        PLUGIN_DB,
        &format!(
            "SELECT code FROM skill_categories WHERE code = {} LIMIT 1",
            quote(code)
        ),
    )?
    .is_empty()
    {
        return Err(AppError::param(format!("技能分类不存在: {code}")));
    }
    Ok(())
}

pub fn get_skill_categories(_req: GetSkillCategoriesReq) -> AppResult<Value> {
    let list = db::query(
        PLUGIN_DB,
        "SELECT * FROM skill_categories ORDER BY sort ASC, code ASC",
    )?;
    Ok(json!({ "list": list }))
}

pub fn get_skills(req: GetSkillsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);
    let mut conditions = Vec::new();

    if let Some(categories) = req.categories.as_ref().filter(|v| !v.is_empty()) {
        let list = categories
            .iter()
            .map(|value| quote(value))
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("category IN ({list})"));
    }
    if let Some(ids) = req.ids.as_ref().filter(|v| !v.is_empty()) {
        let list = ids
            .iter()
            .map(|value| quote(value))
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("id IN ({list})"));
    }
    if let Some(enabled) = flag(&req.enabled) {
        conditions.push(format!("is_enabled = {enabled}"));
    }

    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let total = query_i64(
        PLUGIN_DB,
        &format!("SELECT COUNT(*) AS total FROM skills{where_sql}"),
        "total",
    )?;

    let with_content = req.with_content.unwrap_or(false);
    let columns = if with_content {
        "*"
    } else {
        "id, name, description, category, trigger, is_system, is_enabled, sort, version, created_at, updated_at"
    };
    let mut list = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT {columns} FROM skills{where_sql} \
             ORDER BY is_system DESC, category ASC, sort ASC, id ASC \
             LIMIT {page_size} OFFSET {offset}"
        ),
    )?;
    if with_content {
        for row in &mut list {
            expand_json_fields(row, &["params_schema"]);
        }
    }

    Ok(json!({
        "list": list,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

pub fn get_skill(req: GetSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    let mut row = require_row(
        &format!("SELECT * FROM skills WHERE id = {} LIMIT 1", quote(&req.id)),
        &format!("技能不存在: {}", req.id),
    )?;
    expand_json_fields(&mut row, &["params_schema"]);
    Ok(row)
}

#[derive(Debug, Default, Deserialize)]
pub struct PostSkillReq {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub params_schema: Option<Value>,
    #[serde(default)]
    pub is_system: Option<Value>,
    #[serde(default)]
    pub is_enabled: Option<Value>,
    #[serde(default)]
    pub sort: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateSkillReq {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub params_schema: Option<Value>,
    #[serde(default)]
    pub is_system: Option<Value>,
    #[serde(default)]
    pub is_enabled: Option<Value>,
    #[serde(default)]
    pub sort: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DeleteSkillReq {
    #[serde(default)]
    pub id: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct PostSkillCategoryReq {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateSkillCategoryReq {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DeleteSkillCategoryReq {
    #[serde(default)]
    pub code: String,
}

pub fn post_skill(req: PostSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    require_text(&req.name, "name")?;
    require_skill_category(&req.category)?;
    validate_skill_fields(
        &req.name,
        req.description.as_deref(),
        req.content.as_deref(),
        req.params_schema.as_ref(),
    )?;

    if !db::query(
        PLUGIN_DB,
        &format!("SELECT id FROM skills WHERE id = {} LIMIT 1", quote(&req.id)),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!("技能 id 已存在: {}", req.id)));
    }

    let params_schema = req
        .params_schema
        .clone()
        .unwrap_or_else(|| json!({}))
        .to_string();
    let sql = format!(
        "INSERT INTO skills \
         (id, name, description, category, content, trigger, params_schema, \
          is_system, is_enabled, sort, version) \
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
        quote(&req.id),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        quote(&req.category),
        quote_opt(req.content.as_deref()),
        quote_opt(req.trigger.as_deref()),
        quote(&params_schema),
        flag(&req.is_system).unwrap_or(0),
        flag(&req.is_enabled).unwrap_or(1),
        req.sort.unwrap_or(0),
        req.version.unwrap_or(1),
    );
    db::execute(PLUGIN_DB, &sql)?;
    get_skill(GetSkillReq { id: req.id })
}

pub fn update_skill(req: UpdateSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    require_row(
        &format!("SELECT id FROM skills WHERE id = {} LIMIT 1", quote(&req.id)),
        &format!("技能不存在: {}", req.id),
    )?;
    validate_skill_fields(
        req.name.as_deref().unwrap_or(""),
        req.description.as_deref(),
        req.content.as_deref(),
        req.params_schema.as_ref(),
    )?;

    let mut sets = Vec::new();
    if let Some(name) = req.name.as_deref() {
        require_text(name, "name")?;
        sets.push(format!("name = {}", quote(name)));
    }
    if let Some(description) = req.description.as_deref() {
        sets.push(format!("description = {}", quote(description)));
    }
    if let Some(category) = req.category.as_deref() {
        require_skill_category(category)?;
        sets.push(format!("category = {}", quote(category)));
    }
    if let Some(content) = req.content.as_deref() {
        sets.push(format!("content = {}", quote(content)));
    }
    if let Some(trigger) = req.trigger.as_deref() {
        sets.push(format!("trigger = {}", quote(trigger)));
    }
    if let Some(params_schema) = req.params_schema.as_ref() {
        sets.push(format!(
            "params_schema = {}",
            quote(&params_schema.to_string())
        ));
    }
    if let Some(is_system) = flag(&req.is_system) {
        sets.push(format!("is_system = {is_system}"));
    }
    if let Some(is_enabled) = flag(&req.is_enabled) {
        sets.push(format!("is_enabled = {is_enabled}"));
    }
    if let Some(sort) = req.sort {
        sets.push(format!("sort = {sort}"));
    }

    if sets.is_empty() {
        return Err(AppError::param("no fields to update"));
    }
    sets.push("version = version + 1".to_string());
    sets.push("updated_at = CURRENT_TIMESTAMP".to_string());

    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE skills SET {} WHERE id = {}",
            sets.join(", "),
            quote(&req.id)
        ),
    )?;
    get_skill(GetSkillReq { id: req.id })
}

pub fn delete_skill(req: DeleteSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    require_row(
        &format!("SELECT id FROM skills WHERE id = {} LIMIT 1", quote(&req.id)),
        &format!("技能不存在: {}", req.id),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!("DELETE FROM skills WHERE id = {}", quote(&req.id)),
    )?;
    Ok(json!({ "id": req.id, "deleted": true }))
}

pub fn post_skill_category(req: PostSkillCategoryReq) -> AppResult<Value> {
    require_text(&req.code, "code")?;
    require_text(&req.name, "name")?;
    if !db::query(
        PLUGIN_DB,
        &format!(
            "SELECT code FROM skill_categories WHERE code = {} LIMIT 1",
            quote(&req.code)
        ),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!(
            "技能分类已存在: {}",
            req.code
        )));
    }
    let sql = format!(
        "INSERT INTO skill_categories (code, name, description, sort) \
         VALUES ({}, {}, {}, {})",
        quote(&req.code),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        req.sort.unwrap_or(0),
    );
    db::execute(PLUGIN_DB, &sql)?;
    Ok(json!({ "code": req.code }))
}

pub fn update_skill_category(req: UpdateSkillCategoryReq) -> AppResult<Value> {
    require_text(&req.code, "code")?;
    require_row(
        &format!(
            "SELECT code FROM skill_categories WHERE code = {} LIMIT 1",
            quote(&req.code)
        ),
        &format!("技能分类不存在: {}", req.code),
    )?;

    let mut sets = Vec::new();
    if let Some(name) = req.name.as_deref() {
        require_text(name, "name")?;
        sets.push(format!("name = {}", quote(name)));
    }
    if let Some(description) = req.description.as_deref() {
        sets.push(format!("description = {}", quote(description)));
    }
    if let Some(sort) = req.sort {
        sets.push(format!("sort = {sort}"));
    }
    if sets.is_empty() {
        return Err(AppError::param("no fields to update"));
    }
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE skill_categories SET {} WHERE code = {}",
            sets.join(", "),
            quote(&req.code)
        ),
    )?;
    Ok(json!({ "code": req.code }))
}

pub fn delete_skill_category(req: DeleteSkillCategoryReq) -> AppResult<Value> {
    require_text(&req.code, "code")?;
    require_row(
        &format!(
            "SELECT code FROM skill_categories WHERE code = {} LIMIT 1",
            quote(&req.code)
        ),
        &format!("技能分类不存在: {}", req.code),
    )?;
    let used = query_i64(
        PLUGIN_DB,
        &format!(
            "SELECT COUNT(*) AS total FROM skills WHERE category = {}",
            quote(&req.code)
        ),
        "total",
    )?;
    if used > 0 {
        return Err(AppError::conflict(format!(
            "技能分类仍被 {used} 条技能引用: {}",
            req.code
        )));
    }
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM skill_categories WHERE code = {}",
            quote(&req.code)
        ),
    )?;
    Ok(json!({ "code": req.code, "deleted": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_defaults_and_clamps() {
        assert_eq!(page_params(None, None), (1, 20, 0));
        assert_eq!(page_params(Some(0), Some(0)), (1, 1, 0));
        assert_eq!(page_params(Some(3), Some(500)), (3, 200, 400));
    }

    #[test]
    fn status_validation_rejects_unknown_values() {
        assert!(validate_one_of("published", PLUGIN_STATUSES, "status").is_ok());
        assert!(validate_one_of("installed", PLUGIN_STATUSES, "status").is_err());
        assert!(validate_optional(None, PLUGIN_STATUSES, "status").is_ok());
        assert!(validate_optional(Some(""), PLUGIN_STATUSES, "status").is_ok());
    }

    #[test]
    fn embed_status_accepts_scalar_and_array() {
        assert_eq!(int_list(&Some(json!(2))), Some(vec![2]));
        assert_eq!(int_list(&Some(json!([0, 2]))), Some(vec![0, 2]));
        assert_eq!(int_list(&Some(json!("1"))), Some(vec![1]));
        assert_eq!(int_list(&Some(json!([]))), None);
    }

    #[test]
    fn value_text_flattens_non_strings() {
        assert_eq!(value_text(&json!("x")), "x");
        assert_eq!(value_text(&json!(2)), "2");
        assert_eq!(value_text(&json!(null)), "");
    }

    #[test]
    fn skill_requests_parse_with_defaults() {
        let req: GetSkillsReq = serde_json::from_str("{}").unwrap();
        assert!(req.categories.is_none());
        assert!(req.with_content.is_none());

        let skill: PostSkillReq =
            serde_json::from_str(r#"{"id":"skill-x","name":"X","category":"design"}"#).unwrap();
        assert_eq!(skill.id, "skill-x");
        assert_eq!(skill.category, "design");
        assert!(skill.is_enabled.is_none());

        let delete: DeleteSkillReq = serde_json::from_str(r#"{"id":"skill-x"}"#).unwrap();
        assert_eq!(delete.id, "skill-x");
    }

    #[test]
    fn skill_validation_rejects_long_or_template_content() {
        let long = "x".repeat(MAX_SKILL_CONTENT_CHARS + 1);
        assert!(validate_skill_fields("n", None, Some(&long), None).is_err());
        assert!(validate_skill_fields("n", None, Some("{{rules}}"), None).is_err());
        assert!(validate_skill_fields("n", None, None, Some(&json!([1]))).is_err());
        assert!(validate_skill_fields("n", None, Some("ok"), Some(&json!({}))).is_ok());
    }
}
