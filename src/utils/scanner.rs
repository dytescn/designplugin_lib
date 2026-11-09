//! 扫描已安装的设计插件目录，登记到 plugins / plugin_versions / plugin_libs / plugin_functions。
//!
//! 目录约定：`./plugins/<name>/` 下放 DLL 与可选的 `mcp.json` + 工具 JSON。
//! 版本从 `./dytes.lock` 读取；没有 `mcp.json` 时只登记包与库。

use crate::utils::config::PLUGIN_DB;
use crate::utils::kv;

/// 设计插件动态库扩展名（与 cyctron 的 FFI_EXT 保持一致）。
#[cfg(windows)]
const LIB_EXT: &str = "dll";
#[cfg(target_os = "linux")]
const LIB_EXT: &str = "so";
#[cfg(target_os = "macos")]
const LIB_EXT: &str = "dylib";
use crate::utils::db::{self, quote};
use crate::utils::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
pub struct ScanReq {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub persist: Option<bool>,
}

pub fn scan(req: ScanReq) -> AppResult<Value> {
    let dir = req.dir.unwrap_or_else(|| "./plugins".to_string());
    let persist = req.persist.unwrap_or(true);
    let versions = read_lock_versions("./dytes.lock");
    if persist {
        ensure_use_ai_columns()?;
    }

    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(json!({
            "found": 0, "added": 0, "updated": 0, "libs": 0, "functions": 0,
            "skills": 0, "kv_records": 0, "kv_failed": 0, "kv_errors": [],
            "partial": false
        }));
    };

    let mut packages: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    packages.sort();

    let mut found = 0;
    let mut added = 0;
    let mut updated = 0;
    let mut libs = 0;
    let mut functions = 0;
    let mut skills = 0;
    let mut kv_records = 0;
    let mut kv_errors: Vec<Value> = Vec::new();

    for package in packages {
        found += 1;
        let name = package
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }

        let version = versions
            .get(&name)
            .cloned()
            .unwrap_or_else(|| "0.0.0".to_string());

        if !persist {
            libs += count_libs(&package);
            functions += count_functions(&package);
            continue;
        }

        let (plugin_uuid, was_added) = upsert_plugin(
            &name,
            &package,
            read_use_ai(&package),
            manifest_uuid(&package).as_deref(),
        )?;
        if was_added {
            added += 1;
        } else {
            updated += 1;
        }
        upsert_version(&plugin_uuid, &version)?;
        libs += scan_libs(&plugin_uuid, &package)?;
        functions += scan_functions(&plugin_uuid, &package)?;
        apply_use_ai(&plugin_uuid, read_use_ai(&package))?;
        skills += import_skills(&plugin_uuid, &package)?;
        match import_kv_resources(&plugin_uuid, &name, &package) {
            Ok(count) => kv_records += count,
            Err(error) => {
                // plugins.db 已登记；kv 导入失败不阻断安装，但要在响应中显式上报。
                eprintln!("[scan] 导入 kv 失败 ({name}): {error}");
                kv_errors.push(json!({
                    "plugin": name,
                    "error": error.to_string(),
                }));
            }
        }
    }

    Ok(json!({
        "found": found,
        "added": added,
        "updated": updated,
        "libs": libs,
        "functions": functions,
        "skills": skills,
        "kv_records": kv_records,
        "kv_failed": kv_errors.len(),
        "kv_errors": kv_errors,
        "partial": !kv_errors.is_empty()
    }))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

fn as_str(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn read_lock_versions(path: &str) -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let Ok(text) = fs::read_to_string(path) else {
        return versions;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return versions;
    };
    let Some(packages) = value.get("package").and_then(Value::as_array) else {
        return versions;
    };
    for package in packages {
        let Some(name) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = package.get("version").and_then(Value::as_str) else {
            continue;
        };
        versions.insert(name.to_string(), version.to_string());
    }
    versions
}

/// 读取插件包 `dytes.json` 的 `use_ai`；文件缺失或字段非法时返回 `None`。
fn read_use_ai(package: &Path) -> Option<i64> {
    let text = fs::read_to_string(package.join("dytes.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    use_ai_from_manifest(&value)
}

/// 读取插件包 `dytes.json` 的 `uuid`；缺失或非法时返回 `None`。
pub(crate) fn manifest_uuid(package: &Path) -> Option<String> {
    let text = fs::read_to_string(package.join("dytes.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("uuid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|uuid| Uuid::parse_str(uuid).is_ok())
        .map(str::to_string)
}

/// 从 `dytes.json` 的 JSON 内容解析 `use_ai`（供安装 / 扫描共用）。
pub(crate) fn use_ai_from_manifest(value: &Value) -> Option<i64> {
    match value.get("use_ai") {
        Some(Value::Bool(value)) => Some(i64::from(*value)),
        Some(Value::Number(value)) => Some(value.as_i64().unwrap_or(0).clamp(0, 1)),
        Some(Value::String(value)) if value == "true" || value == "1" => Some(1),
        Some(Value::String(value)) if value == "false" || value == "0" => Some(0),
        _ => None,
    }
}

/// 旧库可能没有 `use_ai` 列；scan 前补列，避免 UPDATE / INSERT 失败。
pub(crate) fn ensure_use_ai_columns() -> AppResult<()> {
    ensure_column("plugins", "use_ai", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column("plugin_functions", "use_ai", "INTEGER DEFAULT 1")?;
    ensure_column("plugin_functions", "tags", "TEXT")?;
    Ok(())
}

fn ensure_column(table: &str, column: &str, ddl: &str) -> AppResult<()> {
    let rows = db::query(PLUGIN_DB, &format!("PRAGMA table_info({table})"))?;
    let exists = rows
        .iter()
        .any(|row| row.get("name").and_then(Value::as_str) == Some(column));
    if !exists {
        db::update(
            PLUGIN_DB,
            &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
        )?;
    }
    Ok(())
}

/// 把插件级 `use_ai` 强制写入该插件的全部能力函数。
fn apply_use_ai(plugin_uuid: &str, use_ai: Option<i64>) -> AppResult<()> {
    let Some(use_ai) = use_ai else {
        return Ok(());
    };
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_functions SET use_ai = {use_ai} \
             WHERE lib_uuid IN (SELECT uuid FROM plugin_libs WHERE plugin_uuid = {})",
            quote(plugin_uuid)
        ),
    )?;
    Ok(())
}

fn upsert_plugin(
    name: &str,
    package: &Path,
    use_ai: Option<i64>,
    package_uuid: Option<&str>,
) -> AppResult<(String, bool)> {
    let install_path = package.to_string_lossy().to_string();
    let timestamp = now();

    let existing = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugins WHERE name = {} AND deleted_at IS NULL LIMIT 1",
            quote(name)
        ),
    )?;

    if let Some(row) = existing.first() {
        let uuid = as_str(row, "uuid");
        let use_ai_sql = use_ai
            .map(|value| format!(", use_ai = {value}"))
            .unwrap_or_default();
        db::update(
            PLUGIN_DB,
            &format!(
                "UPDATE plugins SET installed = 1, install_path = {}, updated_at = {timestamp}{use_ai_sql} WHERE uuid = {}",
                quote(&install_path),
                quote(&uuid)
            ),
        )?;
        return Ok((uuid, false));
    }

    // 新安装优先使用包清单 uuid；旧包无 uuid 时回退生成本地 uuid。
    let uuid = package_uuid
        .filter(|value| Uuid::parse_str(value).is_ok())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    db::execute(
        PLUGIN_DB,
        &format!(
            "INSERT INTO plugins (uuid, name, \"desc\", status, public, created_at, updated_at, \
             installed, install_path, enabled, plugin_type, target, use_ai) \
             VALUES ({}, {}, {}, 'published', 0, {timestamp}, {timestamp}, 1, {}, 1, 'dll', 'common', {})",
            quote(&uuid),
            quote(name),
            quote(name),
            quote(&install_path),
            use_ai.unwrap_or(0)
        ),
    )?;

    Ok((uuid, true))
}

fn upsert_version(plugin_uuid: &str, version: &str) -> AppResult<()> {
    let timestamp = now();
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_versions SET is_latest = 0 WHERE plugin_uuid = {} AND version != {}",
            quote(plugin_uuid),
            quote(version)
        ),
    )?;

    let existing = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT id FROM plugin_versions WHERE plugin_uuid = {} AND version = {} LIMIT 1",
            quote(plugin_uuid),
            quote(version)
        ),
    )?;

    if existing.is_empty() {
        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT INTO plugin_versions (plugin_uuid, version, status, is_latest, created_at, updated_at) \
                 VALUES ({}, {}, 'active', 1, {timestamp}, {timestamp})",
                quote(plugin_uuid),
                quote(version)
            ),
        )?;
    } else {
        db::update(
            PLUGIN_DB,
            &format!(
                "UPDATE plugin_versions SET is_latest = 1, status = 'active', updated_at = {timestamp} \
                 WHERE plugin_uuid = {} AND version = {}",
                quote(plugin_uuid),
                quote(version)
            ),
        )?;
    }

    Ok(())
}

fn count_libs(package: &Path) -> i64 {
    fs::read_dir(package)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|value| value.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case(LIB_EXT))
                        .unwrap_or(false)
                })
                .count() as i64
        })
        .unwrap_or(0)
}

fn scan_libs(plugin_uuid: &str, package: &Path) -> AppResult<i64> {
    let entries = fs::read_dir(package)
        .map_err(|error| AppError::db(format!("读取插件目录失败: {error}")))?;

    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let is_dll = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|ext| ext.eq_ignore_ascii_case(LIB_EXT))
            .unwrap_or(false);
        if !is_dll || file_name.is_empty() {
            continue;
        }

        let lib_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&file_name)
            .to_string();
        let install_path = path.to_string_lossy().to_string();
        // 设计插件直接从 `./plugins/<name>/` 加载；`libary_path` 列保留兼容，
        // 现在记录实际加载路径，is_registered 以该文件是否存在为准。
        let load_path = install_path.clone();
        let is_registered = Path::new(&install_path).exists();
        let file_size = fs::metadata(&path)
            .map(|meta| meta.len() as i64)
            .unwrap_or(0);
        let file_hash = sha256_file(&path).unwrap_or_default();
        let timestamp = now();

        let existing = db::query(
            PLUGIN_DB,
            &format!(
                "SELECT uuid FROM plugin_libs WHERE plugin_uuid = {} AND platform = 'windows' AND name = {} LIMIT 1",
                quote(plugin_uuid),
                quote(&lib_name)
            ),
        )?;

        if let Some(row) = existing.first() {
            db::update(
                PLUGIN_DB,
                &format!(
                    "UPDATE plugin_libs SET file_name = {}, install_path = {}, libary_path = {}, \
                     file_hash = {}, file_size = {}, is_local = 1, is_registered = {}, updated_at = {timestamp} \
                     WHERE uuid = {}",
                    quote(&file_name),
                    quote(&install_path),
                    quote(&load_path),
                    quote(&file_hash),
                    file_size,
                    i64::from(is_registered),
                    quote(&as_str(row, "uuid"))
                ),
            )?;
        } else {
            let lib_uuid = Uuid::new_v4().to_string();
            db::execute(
                PLUGIN_DB,
                &format!(
                    "INSERT INTO plugin_libs (uuid, plugin_uuid, name, file_name, platform, target, \
                     install_path, libary_path, file_hash, file_size, is_local, is_registered, \
                     load_ok, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, 'windows', 'common', {}, {}, {}, {}, 1, {}, -1, {timestamp}, {timestamp})",
                    quote(&lib_uuid),
                    quote(plugin_uuid),
                    quote(&lib_name),
                    quote(&file_name),
                    quote(&install_path),
                    quote(&load_path),
                    quote(&file_hash),
                    file_size,
                    i64::from(is_registered)
                ),
            )?;
        }

        count += 1;
    }

    Ok(count)
}

fn count_functions(package: &Path) -> i64 {
    let path = package.join("mcp.json");
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return 0;
    };
    value
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| tools.len() as i64)
        .unwrap_or(0)
}

fn upsert_category(category: &str, timestamp: i64) -> AppResult<()> {
    db::execute(
        PLUGIN_DB,
        &format!(
            "INSERT OR IGNORE INTO plugin_categories              (uuid, type_name, sort_order, created_at, updated_at)              VALUES ({}, {}, 0, {timestamp}, {timestamp})",
            quote(&Uuid::new_v4().to_string()),
            quote(category)
        ),
    )?;
    Ok(())
}

fn scan_functions(plugin_uuid: &str, package: &Path) -> AppResult<i64> {
    let mcp_path = package.join("mcp.json");
    if !mcp_path.exists() {
        return Ok(0);
    }

    let text = fs::read_to_string(&mcp_path)
        .map_err(|error| AppError::db(format!("读取 mcp.json 失败: {error}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 mcp.json 失败: {error}")))?;

    let Some(tools) = value.get("tools").and_then(Value::as_array) else {
        return Ok(0);
    };

    let lib_rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid, name FROM plugin_libs WHERE plugin_uuid = {} \
             ORDER BY id ASC LIMIT 1",
            quote(plugin_uuid)
        ),
    )?;
    // MCP-only 资源包没有动态库：用 plugin_uuid 作为 lib_uuid，函数仍然可见。
    let (lib_uuid, lib_name) = match lib_rows.first() {
        Some(row) => (as_str(row, "uuid"), as_str(row, "name")),
        None => (
            plugin_uuid.to_string(),
            package
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("mcp")
                .to_string(),
        ),
    };
    let manifest = read_manifest(package);
    let manifest_category = manifest
        .as_ref()
        .and_then(|value| value.get("category"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let manifest_tags = manifest
        .as_ref()
        .and_then(|value| value.get("tags"))
        .cloned()
        .unwrap_or(Value::Null);
    let timestamp = now();

    let mut count = 0;
    for tool in tools {
        let Some(file_name) = tool.as_str() else {
            continue;
        };
        if file_name.contains('/') || file_name.contains('\\') {
            continue;
        }

        let tool_path = package.join(file_name);
        let Ok(tool_text) = fs::read_to_string(&tool_path) else {
            continue;
        };
        let Ok(tool_value) = serde_json::from_str::<Value>(&tool_text) else {
            continue;
        };

        let Some(symbol) = tool_value.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = tool_value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(symbol);
        let category = tool_value
            .get("category")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(&manifest_category)
            .to_string();
        let tags = tool_value
            .get("tags")
            .cloned()
            .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
            .unwrap_or_else(|| manifest_tags.clone());
        if !category.is_empty() {
            upsert_category(&category, timestamp)?;
        }
        let params_schema = tool_value
            .get("inputSchema")
            .map(ToString::to_string)
            .unwrap_or_else(|| "{\"type\":\"object\"}".to_string());
        let return_schema = tool_value
            .get("outputSchema")
            .map(ToString::to_string)
            .unwrap_or_else(|| "{\"type\":\"object\"}".to_string());

        let existing = db::query(
            PLUGIN_DB,
            &format!(
                "SELECT uuid FROM plugin_functions WHERE lib_uuid = {} AND symbol = {} LIMIT 1",
                quote(&lib_uuid),
                quote(symbol)
            ),
        )?;

        if let Some(row) = existing.first() {
            db::update(
                PLUGIN_DB,
                &format!(
                    "UPDATE plugin_functions SET lib_name = {}, ffi_symbol = {}, title = {}, \
                     description = {}, category = {}, tags = {}, params_schema = {}, \
                     return_schema = {}, is_local = 1, source = 'mcp.json', \
                     updated_at = {timestamp} WHERE uuid = {}",
                    quote(&lib_name),
                    quote(symbol),
                    quote(symbol),
                    quote(description),
                    quote(&category),
                    quote(&tags.to_string()),
                    quote(&params_schema),
                    quote(&return_schema),
                    quote(&as_str(row, "uuid"))
                ),
            )?;
        } else {
            let function_uuid = Uuid::new_v4().to_string();
            db::execute(
                PLUGIN_DB,
                &format!(
                    "INSERT INTO plugin_functions (uuid, lib_uuid, lib_name, symbol, method, \
                     ffi_symbol, title, description, category, tags, params_schema, \
                     return_schema, side_effects, exposed_mcp, is_local, embed_status, source, \
                     created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, 'post', {}, {}, {}, {}, {}, {}, {}, 0, 1, 1, 0, \
                     'mcp.json', {timestamp}, {timestamp})",
                    quote(&function_uuid),
                    quote(&lib_uuid),
                    quote(&lib_name),
                    quote(symbol),
                    quote(symbol),
                    quote(symbol),
                    quote(description),
                    quote(&category),
                    quote(&tags.to_string()),
                    quote(&params_schema),
                    quote(&return_schema)
                ),
            )?;
        }

        count += 1;
    }

    Ok(count)
}

/// 导入插件随包 `skills.json`：索引 / 正文写 `skills`，归属写 `plugin_skills`。
///
/// 该步骤幂等：重复 scan 会更新技能内容并递增 `version`，不会重复插入归属行。
fn import_skills(plugin_uuid: &str, package: &Path) -> AppResult<i64> {
    let path = package.join("skills.json");
    if !path.exists() {
        return Ok(0);
    }

    let text = fs::read_to_string(&path)
        .map_err(|error| AppError::db(format!("读取 skills.json 失败: {error}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 skills.json 失败: {error}")))?;
    let Some(skills) = value.get("skills").and_then(Value::as_array) else {
        return Ok(0);
    };

    let mut count = 0;
    for skill in skills {
        let id = skill
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let content = skill.get("content").and_then(Value::as_str).unwrap_or("");
        if content.trim().is_empty() {
            continue;
        }

        let name = skill.get("name").and_then(Value::as_str).unwrap_or(&id);
        let description = skill
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let category = skill
            .get("category")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("plugin");
        let trigger = skill.get("trigger").and_then(Value::as_str).unwrap_or("");
        let params_schema = skill
            .get("params_schema")
            .map(ToString::to_string)
            .unwrap_or_else(|| "{}".to_string());
        let is_system = skill.get("is_system").and_then(Value::as_i64).unwrap_or(0);
        let is_enabled = skill.get("is_enabled").and_then(Value::as_i64).unwrap_or(1);
        let sort = skill.get("sort").and_then(Value::as_i64).unwrap_or(0);

        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT OR IGNORE INTO skill_categories (code, name, sort) VALUES ({}, {}, 100)",
                quote(category),
                quote(category)
            ),
        )?;

        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT INTO skills (id, name, description, category, content, trigger, \
                 params_schema, is_system, is_enabled, sort, version, created_at, updated_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 1, datetime('now'), datetime('now')) \
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, \
                 description = excluded.description, category = excluded.category, \
                 content = excluded.content, trigger = excluded.trigger, \
                 params_schema = excluded.params_schema, is_system = excluded.is_system, \
                 is_enabled = excluded.is_enabled, sort = excluded.sort, \
                 version = skills.version + 1, updated_at = datetime('now')",
                quote(&id),
                quote(name),
                quote(description),
                quote(category),
                quote(content),
                quote(trigger),
                quote(&params_schema),
                is_system,
                is_enabled,
                sort
            ),
        )?;

        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT INTO plugin_skills (plugin_uuid, skill_id, version, is_owned, created_at) \
                 VALUES ({}, {}, 1, 1, datetime('now')) \
                 ON CONFLICT(plugin_uuid, skill_id) DO UPDATE SET version = excluded.version, \
                 is_owned = excluded.is_owned",
                quote(plugin_uuid),
                quote(&id)
            ),
        )?;

        // 完整内容写 designdb（best-effort，缺库时不影响安装 / 扫描）。
        if let Err(error) = crate::utils::designdb::upsert_skill(skill) {
            eprintln!("[scan] 写 designdb skills 失败 ({id}): {error}");
        }

        count += 1;
    }

    Ok(count)
}

/// 按 `plugins_type` 把 prompts / skills / MCP 能力导入 kv。
///
/// - `chat_think` → `chat_think`
/// - `chat_reply` → `chat_reply`
/// - `chat_plugins` → `chat_plugins`（仓库分类，供 `call_chat` 直接检索）
/// - `sys_plugins` → `sys_plugins`（系统插件分类）
/// - `plugins` / `plugins_coreldraw` → `chat_plugin`（兼容旧清单）
///
/// kv_lib 未安装时返回 0；已安装但写入失败时返回错误。
fn import_kv_resources(plugin_uuid: &str, name: &str, package: &Path) -> AppResult<i64> {
    if !kv::available() {
        return Ok(0);
    }
    let Some(manifest) = read_manifest(package) else {
        return Ok(0);
    };
    let plugins_type = manifest
        .get("plugins_type")
        .and_then(Value::as_str)
        .unwrap_or("");

    match plugins_type {
        "chat_think" => import_chat_resources(
            plugin_uuid,
            name,
            package,
            "chat_think",
            "聊天思考",
            "think 阶段检索词与技能",
            10,
        ),
        "chat_reply" => import_chat_resources(
            plugin_uuid,
            name,
            package,
            "chat_reply",
            "聊天回复",
            "reply 阶段检索词与技能",
            20,
        ),
        "chat_plugins" => import_chat_plugins(
            plugin_uuid,
            name,
            package,
            "chat_plugins",
            "聊天插件",
            "chat_plugins 分类插件的能力描述",
            30,
        ),
        "sys_plugins" => import_chat_plugins(
            plugin_uuid,
            name,
            package,
            "sys_plugins",
            "系统插件",
            "sys_plugins 分类插件的能力描述",
            40,
        ),
        "plugins" | "plugins_coreldraw" => import_chat_plugins(
            plugin_uuid,
            name,
            package,
            "chat_plugin",
            "聊天插件",
            "chat 分类插件的能力描述",
            30,
        ),
        _ => Ok(0),
    }
}

fn read_manifest(package: &Path) -> Option<Value> {
    let text = fs::read_to_string(package.join("dytes.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn import_chat_resources(
    plugin_uuid: &str,
    name: &str,
    package: &Path,
    collection: &str,
    label: &str,
    description: &str,
    sort: i64,
) -> AppResult<i64> {
    let prefix = format!("{plugin_uuid}:");
    let mut items = Vec::new();

    for (index, prompt) in prompt_values(package, name).iter().enumerate() {
        items.push(json!({
            "key": format!("{prefix}prompt:{index}"),
            "value": prompt,
            "text": prompt,
            "meta": { "plugin_uuid": plugin_uuid, "plugin_name": name, "kind": "prompt" },
            "rank": index as i64 + 1,
            "force": true,
        }));
    }

    for (index, (skill_id, content)) in skill_values(package, name).iter().enumerate() {
        if content.trim().is_empty() || content.len() > 60_000 {
            continue;
        }
        items.push(json!({
            "key": format!("{prefix}skill:{skill_id}"),
            "value": content,
            "text": content,
            "meta": { "plugin_uuid": plugin_uuid, "plugin_name": name, "kind": "skill" },
            "rank": index as i64 + 1,
            "force": true,
        }));
    }

    if items.is_empty() {
        return Ok(0);
    }

    kv::ensure_collection(collection, label, description, sort)?;
    kv::replace_records(collection, &prefix, &items).map(|count| count as i64)
}

fn prompt_values(package: &Path, name: &str) -> Vec<String> {
    let mut candidates = vec![
        package.join("prompts.json"),
        package.join(format!("{name}.json")),
    ];
    if let Ok(entries) = fs::read_dir(package.join("prompts")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                candidates.push(path);
            }
        }
    }

    for path in candidates {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let values = extract_prompt_values(&value);
        if !values.is_empty() {
            return values;
        }
    }
    Vec::new()
}

fn extract_prompt_values(value: &Value) -> Vec<String> {
    let list = value
        .as_array()
        .or_else(|| value.get("prompts").and_then(Value::as_array));
    list.map(|list| {
        list.iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

fn skill_values(package: &Path, name: &str) -> Vec<(String, String)> {
    let mut values = Vec::new();

    if let Ok(text) = fs::read_to_string(package.join("skills.json")) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let Some(skills) = value.get("skills").and_then(Value::as_array) {
                for skill in skills {
                    let id = skill
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("skill")
                        .to_string();
                    if let Some(content) = skill.get("content").and_then(Value::as_str) {
                        values.push((id, content.to_string()));
                    }
                }
            }
        }
    }

    if values.is_empty() {
        let path = package.join(format!("{name}.md"));
        if let Ok(content) = fs::read_to_string(path) {
            values.push(("main".to_string(), content));
        }
    }

    values
}

fn import_chat_plugins(
    plugin_uuid: &str,
    name: &str,
    package: &Path,
    collection: &str,
    label: &str,
    description: &str,
    sort: i64,
) -> AppResult<i64> {
    let text = match fs::read_to_string(package.join("mcp.json")) {
        Ok(text) => text,
        Err(_) => return Ok(0),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[scan] 解析 mcp.json 失败 ({name}): {error}");
            return Ok(0);
        }
    };
    let Some(tools) = value.get("tools").and_then(Value::as_array) else {
        return Ok(0);
    };

    let prefix = format!("{plugin_uuid}:");
    let mut items = Vec::new();
    for tool in tools {
        let Some(file_name) = tool.as_str() else {
            continue;
        };
        if file_name.contains('/') || file_name.contains('\\') {
            continue;
        }
        let Ok(text) = fs::read_to_string(package.join(file_name)) else {
            continue;
        };
        let Ok(tool_value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(symbol) = tool_value.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = tool_value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(symbol);
        items.push(json!({
            "key": format!("{prefix}tool:{symbol}"),
            "value": description,
            "text": format!("{symbol} {description}"),
            "meta": {
                "plugin": name,
                // chat 插件固定导出 post_chat；mcp tool 名只用于能力描述。
                "ffi_symbol": "chat",
                "tool": symbol,
                "plugin_uuid": plugin_uuid,
                "category": tool_value.get("category").and_then(Value::as_str).unwrap_or(""),
                "tags": tool_value.get("tags").cloned().unwrap_or(Value::Null),
            },
            "force": true,
        }));
    }

    if items.is_empty() {
        return Ok(0);
    }

    kv::ensure_collection(collection, label, description, sort)?;
    kv::replace_records(collection, &prefix, &items).map(|count| count as i64)
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path).map_err(|error| AppError::db(format!("读取文件失败: {error}")))?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_ai_from_manifest_accepts_supported_shapes() {
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": true})), Some(1));
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": false})), Some(0));
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": 1})), Some(1));
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": 0})), Some(0));
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": "true"})), Some(1));
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": "0"})), Some(0));
        assert_eq!(use_ai_from_manifest(&json!({})), None);
        assert_eq!(use_ai_from_manifest(&json!({"use_ai": "maybe"})), None);
    }
}
