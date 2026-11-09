//! 插件、分类、版本、配置、安装状态、动态库与能力的数据逻辑。
//!
//! 全部 SQL 在 Rust 内拼装并转义单引号；不直接链接 SQLite。
//! 删除 / 安装 / 扫描属于 M4，本模块先覆盖 M0–M3 的 CRUD。

use crate::ffi::types::{
    DeletePluginReq, GetPluginReq, GetPluginsReq, PatchPluginReq, PostPluginReq, UpdatePluginReq,
};
use crate::utils::cjson::flag;
use crate::utils::db::{self, like_pattern, quote, quote_opt, query_i64, PLUGIN_DB};
use crate::utils::designdb;
use crate::utils::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

const PLUGIN_STATUSES: &[&str] = &["draft", "review", "published", "offline", "deprecated"];
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

#[cfg(test)]
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

// ---------------------------------------------------------------------------
// 请求结构
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct GetFunctionsReq {
    #[serde(default)]
    pub uuids: Option<Vec<String>>,
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GetSkillsReq {
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub plugin_uuid: Option<String>,
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
pub struct PostFunctionReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub ffi_symbol: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub params_schema: Option<Value>,
    #[serde(default)]
    pub params_example: Option<Value>,
    #[serde(default)]
    pub return_schema: Option<Value>,
    #[serde(default)]
    pub return_example: Option<Value>,
    #[serde(default)]
    pub use_ai: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateFunctionReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub plugin_uuid: String,
    #[serde(default)]
    pub ffi_symbol: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub params_schema: Option<Value>,
    #[serde(default)]
    pub params_example: Option<Value>,
    #[serde(default)]
    pub return_schema: Option<Value>,
    #[serde(default)]
    pub return_example: Option<Value>,
    #[serde(default)]
    pub use_ai: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchFunctionReq {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub ffi_symbol: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub params_schema: Option<Value>,
    #[serde(default)]
    pub params_example: Option<Value>,
    #[serde(default)]
    pub return_schema: Option<Value>,
    #[serde(default)]
    pub return_example: Option<Value>,
    #[serde(default)]
    pub use_ai: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DeleteFunctionReq {
    #[serde(default)]
    pub uuid: String,
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

pub fn get_plugins(req: GetPluginsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);

    let mut conditions = vec!["deleted_at IS NULL".to_string()];

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
    if let Some(platform) = req.platform.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("platform = {}", quote(platform)));
    }

    let where_sql = format!(" WHERE {}", conditions.join(" AND "));

    let total = query_i64(
        PLUGIN_DB,
        &format!("SELECT COUNT(*) AS total FROM plugins{where_sql}"),
        "total",
    )?;

    let list = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT * FROM plugins{where_sql} ORDER BY created_at DESC LIMIT {page_size} OFFSET {offset}"
        ),
    )?;

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

    require_row(
        &format!(
            "SELECT * FROM plugins WHERE {field} = {} AND deleted_at IS NULL LIMIT 1",
            quote(value)
        ),
        &format!("插件不存在: {value}"),
    )
}

pub fn get_functions(req: GetFunctionsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);
    let mut conditions = Vec::new();

    if let Some(uuids) = req.uuids.as_ref().filter(|v| !v.is_empty()) {
        let in_list = uuids.iter().map(|u| quote(u)).collect::<Vec<_>>().join(", ");
        conditions.push(format!("uuid IN ({in_list})"));
    }
    if let Some(plugin_uuid) = req.plugin_uuid.as_deref().filter(|v| !v.is_empty()) {
        conditions.push(format!("plugin_uuid = {}", quote(plugin_uuid)));
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
            "SELECT * FROM plugin_functions{where_sql} ORDER BY uuid ASC \
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
        &format!("SELECT uuid FROM plugins WHERE uuid = {} LIMIT 1", quote(&uuid)),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!("插件 uuid 已存在: {uuid}")));
    }

    let timestamp = now();
    let sql = format!(
        "INSERT INTO plugins \
         (uuid, ouid, plugins_type, version, name, \"desc\", create_by, cover, \
          status, public, updated_by, created_at, updated_at, installed, install_path, \
          enabled, target, platform, mask) \
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {timestamp}, {timestamp}, \
                 0, NULL, {}, {}, {}, {})",
        quote(&uuid),
        quote_opt(req.ouid.as_deref()),
        quote_opt(req.plugins_type.as_deref()),
        quote(req.version.as_deref().unwrap_or("")),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        quote_opt(req.create_by.as_deref()),
        quote_opt(req.cover.as_deref()),
        quote(req.status.as_deref().unwrap_or("draft")),
        req.public_flag.unwrap_or(0),
        quote_opt(req.updated_by.as_deref()),
        req.enabled.unwrap_or(1),
        quote(req.target.as_deref().unwrap_or("common")),
        quote(req.platform.as_deref().unwrap_or("win")),
        req.mask.unwrap_or(0),
    );

    db::execute(PLUGIN_DB, &sql)?;
    Ok(json!({ "uuid": uuid }))
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
        "UPDATE plugins SET ouid = {}, plugins_type = {}, version = {}, name = {}, \"desc\" = {}, \
         create_by = {}, cover = {}, status = {}, public = {}, updated_by = {}, \
         target = {}, platform = {}, mask = {}, updated_at = {} WHERE uuid = {}",
        quote_opt(req.ouid.as_deref()),
        quote_opt(req.plugins_type.as_deref()),
        quote(req.version.as_deref().unwrap_or("")),
        quote(&req.name),
        quote_opt(req.description.as_deref()),
        quote_opt(req.create_by.as_deref()),
        quote_opt(req.cover.as_deref()),
        quote(req.status.as_deref().unwrap_or("draft")),
        req.public_flag.unwrap_or(0),
        quote_opt(req.updated_by.as_deref()),
        quote(req.target.as_deref().unwrap_or("common")),
        quote(req.platform.as_deref().unwrap_or("win")),
        req.mask.unwrap_or(0),
        now(),
        quote(&req.uuid),
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
    if let Some(cover) = req.cover.as_deref() {
        sets.push(format!("cover = {}", quote(cover)));
    }
    if let Some(mask) = req.mask {
        sets.push(format!("mask = {mask}"));
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

pub fn delete_plugin(req: DeletePluginReq) -> AppResult<Value> {
    let (field, value) = match (
        req.plugin_uuid.as_deref().filter(|v| !v.is_empty()),
        req.name.as_deref().filter(|v| !v.is_empty()),
    ) {
        (Some(uuid), _) => ("uuid", uuid),
        (None, Some(name)) => ("name", name),
        (None, None) => return Err(AppError::param("plugin_uuid or name is required")),
    };

    let plugin = require_row(
        &format!(
            "SELECT uuid, name FROM plugins WHERE {field} = {} LIMIT 1",
            quote(value)
        ),
        &format!("插件不存在: {value}"),
    )?;
    let uuid = as_str(&plugin, "uuid").unwrap_or_default().to_string();
    let name = as_str(&plugin, "name").unwrap_or_default().to_string();

    db::delete(
        PLUGIN_DB,
        &format!("DELETE FROM plugins WHERE uuid = {}", quote(&uuid)),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_functions WHERE plugin_uuid = {}",
            quote(&uuid)
        ),
    )?;

    Ok(json!({ "plugin_uuid": uuid, "name": name, "deleted": true }))
}

fn get_function(uuid: &str) -> AppResult<Value> {
    let mut row = require_row(
        &format!(
            "SELECT * FROM plugin_functions WHERE uuid = {} LIMIT 1",
            quote(uuid)
        ),
        &format!("能力不存在: {uuid}"),
    )?;
    expand_json_fields(
        &mut row,
        &[
            "params_schema",
            "params_example",
            "return_schema",
            "return_example",
        ],
    );
    Ok(row)
}

fn function_json(value: &Option<Value>, default: &str) -> String {
    value
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| default.to_string())
}

pub fn post_function(req: PostFunctionReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.ffi_symbol, "ffi_symbol")?;
    require_text(&req.title, "title")?;
    require_text(&req.description, "description")?;

    if !db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugin_functions WHERE uuid = {} LIMIT 1",
            quote(&req.uuid)
        ),
    )?
    .is_empty()
    {
        return Err(AppError::conflict(format!(
            "能力 uuid 已存在: {}",
            req.uuid
        )));
    }

    let timestamp = now();
    let params_schema = function_json(&req.params_schema, "{}");
    let return_schema = function_json(&req.return_schema, "{}");
    let params_example = req.params_example.as_ref().map(ToString::to_string);
    let return_example = req.return_example.as_ref().map(ToString::to_string);
    let use_ai = flag(&req.use_ai).unwrap_or(0);

    let sql = format!(
        "INSERT INTO plugin_functions \
         (uuid, plugin_uuid, ffi_symbol, title, description, params_schema, params_example, \
          return_schema, return_example, use_ai, created_at, updated_at) \
         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {use_ai}, {timestamp}, {timestamp})",
        quote(&req.uuid),
        quote(&req.plugin_uuid),
        quote(&req.ffi_symbol),
        quote(&req.title),
        quote(&req.description),
        quote(&params_schema),
        quote_opt(params_example.as_deref()),
        quote(&return_schema),
        quote_opt(return_example.as_deref()),
    );
    db::execute(PLUGIN_DB, &sql)?;
    get_function(&req.uuid)
}

pub fn update_function(req: UpdateFunctionReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.ffi_symbol, "ffi_symbol")?;
    require_text(&req.title, "title")?;
    require_text(&req.description, "description")?;
    get_function(&req.uuid)?;

    let params_schema = function_json(&req.params_schema, "{}");
    let return_schema = function_json(&req.return_schema, "{}");
    let params_example = req.params_example.as_ref().map(ToString::to_string);
    let return_example = req.return_example.as_ref().map(ToString::to_string);
    let use_ai = flag(&req.use_ai).unwrap_or(0);

    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_functions SET plugin_uuid = {}, ffi_symbol = {}, title = {}, \
             description = {}, params_schema = {}, params_example = {}, return_schema = {}, \
             return_example = {}, use_ai = {use_ai}, updated_at = {} WHERE uuid = {}",
            quote(&req.plugin_uuid),
            quote(&req.ffi_symbol),
            quote(&req.title),
            quote(&req.description),
            quote(&params_schema),
            quote_opt(params_example.as_deref()),
            quote(&return_schema),
            quote_opt(return_example.as_deref()),
            now(),
            quote(&req.uuid),
        ),
    )?;
    get_function(&req.uuid)
}

pub fn patch_function(req: PatchFunctionReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    get_function(&req.uuid)?;

    let mut sets = Vec::new();

    if let Some(value) = req.plugin_uuid.as_deref() {
        require_text(value, "plugin_uuid")?;
        sets.push(format!("plugin_uuid = {}", quote(value)));
    }
    if let Some(value) = req.ffi_symbol.as_deref() {
        require_text(value, "ffi_symbol")?;
        sets.push(format!("ffi_symbol = {}", quote(value)));
    }
    if let Some(value) = req.title.as_deref() {
        require_text(value, "title")?;
        sets.push(format!("title = {}", quote(value)));
    }
    if let Some(value) = req.description.as_deref() {
        require_text(value, "description")?;
        sets.push(format!("description = {}", quote(value)));
    }
    if let Some(value) = req.params_schema.as_ref() {
        sets.push(format!("params_schema = {}", quote(&value.to_string())));
    }
    if let Some(value) = req.params_example.as_ref() {
        sets.push(format!("params_example = {}", quote(&value.to_string())));
    }
    if let Some(value) = req.return_schema.as_ref() {
        sets.push(format!("return_schema = {}", quote(&value.to_string())));
    }
    if let Some(value) = req.return_example.as_ref() {
        sets.push(format!("return_example = {}", quote(&value.to_string())));
    }
    if let Some(use_ai) = flag(&req.use_ai) {
        if !matches!(use_ai, 0 | 1) {
            return Err(AppError::param("use_ai must be 0 or 1"));
        }
        sets.push(format!("use_ai = {use_ai}"));
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

    get_function(&req.uuid)
}

pub fn delete_function(req: DeleteFunctionReq) -> AppResult<Value> {
    require_text(&req.uuid, "uuid")?;
    get_function(&req.uuid)?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_functions WHERE uuid = {}",
            quote(&req.uuid)
        ),
    )?;
    Ok(json!({ "uuid": req.uuid, "deleted": true }))
}

// ---------------------------------------------------------------------------
// 技能（designdb 的 skills 对象集）
// ---------------------------------------------------------------------------

fn skill_summary(doc: &Value) -> Value {
    json!({
        "id": doc.get("id").or_else(|| doc.get("_id")).cloned().unwrap_or(Value::Null),
        "plugin_uuid": doc.get("plugin_uuid").cloned().unwrap_or(Value::Null),
        "name": doc.get("name").cloned().unwrap_or(Value::Null),
        "description": doc.get("description").cloned().unwrap_or(Value::Null),
        "category": doc.get("category").cloned().unwrap_or(Value::Null),
        "trigger": doc.get("trigger").cloned().unwrap_or(Value::Null),
        "is_system": doc.get("is_system").cloned().unwrap_or(json!(0)),
        "is_enabled": doc.get("is_enabled").cloned().unwrap_or(json!(1)),
        "sort": doc.get("sort").cloned().unwrap_or(json!(0)),
        "version": doc.get("version").cloned().unwrap_or(json!(1)),
        "created_at": doc.get("created_at").cloned().unwrap_or(Value::Null),
        "updated_at": doc.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

pub fn get_skills(req: GetSkillsReq) -> AppResult<Value> {
    let (page, page_size, offset) = page_params(req.page, req.page_size);
    let mut conditions = Vec::new();

    if let Some(categories) = req.categories.as_ref().filter(|v| !v.is_empty()) {
        conditions.push(json!({ "category": { "$in": categories } }));
    }
    if let Some(ids) = req.ids.as_ref().filter(|v| !v.is_empty()) {
        conditions.push(json!({ "_id": { "$in": ids } }));
    }
    if let Some(plugin_uuid) = req
        .plugin_uuid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        conditions.push(json!({ "plugin_uuid": plugin_uuid }));
    }
    if let Some(enabled) = flag(&req.enabled) {
        conditions.push(json!({ "is_enabled": enabled }));
    }

    let filter = match conditions.len() {
        0 => json!({}),
        1 => conditions.remove(0),
        _ => json!({ "$and": conditions }),
    };

    let mut docs = designdb::query_all(filter)?;
    docs.sort_by(|a, b| {
        let key = |doc: &Value| {
            (
                doc.get("is_system").and_then(Value::as_i64).unwrap_or(0),
                doc.get("category")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                doc.get("sort").and_then(Value::as_i64).unwrap_or(0),
                doc.get("_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            )
        };
        let (a_system, a_category, a_sort, a_id) = key(a);
        let (b_system, b_category, b_sort, b_id) = key(b);
        b_system
            .cmp(&a_system)
            .then_with(|| a_category.cmp(&b_category))
            .then_with(|| a_sort.cmp(&b_sort))
            .then_with(|| a_id.cmp(&b_id))
    });

    let total = docs.len();
    let with_content = req.with_content.unwrap_or(false);
    let list = docs
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .map(|doc| if with_content { doc } else { skill_summary(&doc) })
        .collect::<Vec<_>>();

    Ok(json!({
        "list": list,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

pub fn get_skill(req: GetSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    designdb::get_skill_doc(req.id.trim())?
        .ok_or_else(|| AppError::not_found(format!("技能不存在: {}", req.id)))
}

#[derive(Debug, Default, Deserialize)]
pub struct PostSkillReq {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub plugin_uuid: String,
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
    pub plugin_uuid: Option<String>,
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

pub fn post_skill(req: PostSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    require_text(&req.plugin_uuid, "plugin_uuid")?;
    require_text(&req.name, "name")?;
    require_text(&req.category, "category")?;
    validate_skill_fields(
        &req.name,
        req.description.as_deref(),
        req.content.as_deref(),
        req.params_schema.as_ref(),
    )?;

    let id = req.id.trim();
    if designdb::get_skill_doc(id)?.is_some() {
        return Err(AppError::conflict(format!("技能 id 已存在: {id}")));
    }

    let timestamp = now();
    let doc = json!({
        "_id": id,
        "id": id,
        "plugin_uuid": req.plugin_uuid.trim(),
        "name": req.name,
        "description": req.description.unwrap_or_default(),
        "category": req.category,
        "content": req.content.unwrap_or_default(),
        "trigger": req.trigger.unwrap_or_default(),
        "params_schema": req.params_schema.unwrap_or_else(|| json!({})),
        "is_system": flag(&req.is_system).unwrap_or(0),
        "is_enabled": flag(&req.is_enabled).unwrap_or(1),
        "sort": req.sort.unwrap_or(0),
        "version": req.version.unwrap_or(1),
        "created_at": timestamp,
        "updated_at": timestamp,
    });
    designdb::insert(doc)?;
    get_skill(GetSkillReq { id: id.to_string() })
}

pub fn update_skill(req: UpdateSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    let id = req.id.trim();
    let existing = designdb::get_skill_doc(id)?
        .ok_or_else(|| AppError::not_found(format!("技能不存在: {id}")))?;
    validate_skill_fields(
        req.name.as_deref().unwrap_or(""),
        req.description.as_deref(),
        req.content.as_deref(),
        req.params_schema.as_ref(),
    )?;

    let mut set = serde_json::Map::new();
    if let Some(plugin_uuid) = req.plugin_uuid.as_deref() {
        require_text(plugin_uuid, "plugin_uuid")?;
        set.insert("plugin_uuid".to_string(), json!(plugin_uuid));
    }
    if let Some(name) = req.name.as_deref() {
        require_text(name, "name")?;
        set.insert("name".to_string(), json!(name));
    }
    if let Some(description) = req.description.as_deref() {
        set.insert("description".to_string(), json!(description));
    }
    if let Some(category) = req.category.as_deref() {
        require_text(category, "category")?;
        set.insert("category".to_string(), json!(category));
    }
    if let Some(content) = req.content.as_deref() {
        set.insert("content".to_string(), json!(content));
    }
    if let Some(trigger) = req.trigger.as_deref() {
        set.insert("trigger".to_string(), json!(trigger));
    }
    if let Some(params_schema) = req.params_schema.as_ref() {
        set.insert("params_schema".to_string(), params_schema.clone());
    }
    if let Some(is_system) = flag(&req.is_system) {
        set.insert("is_system".to_string(), json!(is_system));
    }
    if let Some(is_enabled) = flag(&req.is_enabled) {
        set.insert("is_enabled".to_string(), json!(is_enabled));
    }
    if let Some(sort) = req.sort {
        set.insert("sort".to_string(), json!(sort));
    }

    if set.is_empty() {
        return Err(AppError::param("no fields to update"));
    }
    let version = existing.get("version").and_then(Value::as_i64).unwrap_or(1);
    set.insert("version".to_string(), json!(version + 1));
    set.insert("updated_at".to_string(), json!(now()));

    designdb::update(
        json!({ "_id": id }),
        json!({ "$set": Value::Object(set) }),
    )?;
    get_skill(GetSkillReq { id: id.to_string() })
}

pub fn delete_skill(req: DeleteSkillReq) -> AppResult<Value> {
    require_text(&req.id, "id")?;
    let id = req.id.trim();
    if designdb::get_skill_doc(id)?.is_none() {
        return Err(AppError::not_found(format!("技能不存在: {id}")));
    }
    designdb::delete(json!({ "_id": id }))?;
    Ok(json!({ "id": req.id, "deleted": true }))
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
    fn skill_requests_parse_with_defaults() {
        let req: GetSkillsReq = serde_json::from_str("{}").unwrap();
        assert!(req.categories.is_none());
        assert!(req.with_content.is_none());
        assert!(req.plugin_uuid.is_none());

        let req: GetSkillsReq =
            serde_json::from_str(r#"{"plugin_uuid":"plugin-1","with_content":true}"#).unwrap();
        assert_eq!(req.plugin_uuid.as_deref(), Some("plugin-1"));
        assert_eq!(req.with_content, Some(true));

        let skill: PostSkillReq = serde_json::from_str(
            r#"{"id":"skill-x","plugin_uuid":"plugin-1","name":"X","category":"design"}"#,
        )
        .unwrap();
        assert_eq!(skill.id, "skill-x");
        assert_eq!(skill.plugin_uuid, "plugin-1");
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
