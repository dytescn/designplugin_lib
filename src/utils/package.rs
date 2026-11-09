//! 插件包安装 / 更新。
//!
//! 按 `install.md` / `update.md` 的流程读取一个插件包目录：
//! `dytes.json` → plugins 主表，平台库文件，`skills/*.md`、`prompts.json`、
//! `taskflow.json`、`mcp.json` → designdb / plugin_functions / kv_lib。

use crate::utils::db::{self, quote, PLUGIN_DB};
use crate::utils::designdb;
use crate::utils::error::{AppError, AppResult};
use crate::utils::kv;
use crate::utils::plugin::{self as plugin_ops, PostFunctionReq, UpdateFunctionReq};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default, Deserialize)]
pub struct InstallPackageReq {
    #[serde(default)]
    pub dir: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdatePackageReq {
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prune: Option<bool>,
}

pub fn install(req: InstallPackageReq) -> AppResult<Value> {
    let package = validate_package(&req.dir)?;
    let manifest = read_manifest(&package)?;
    let uuid = require_manifest_text(&manifest, "uuid")?;
    let name = require_manifest_text(&manifest, "name")?;
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0")
        .to_string();
    let plugins_type = manifest
        .get("plugins_type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let target = manifest
        .get("target")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("common")
        .to_string();
    let use_ai = flag_value(manifest.get("use_ai")).unwrap_or(0);
    if let Some((installed_name, installed_version)) = installed_plugin(&uuid)?
        && compare_versions(&installed_version, &version) != Ordering::Less
    {
        return Ok(json!({
            "plugin_uuid": uuid,
            "name": installed_name,
            "version": installed_version,
            "skipped": true,
            "reason": "installed version is not lower than package version",
        }));
    }

    let platform = platform_code();
    let lib_file = locate_lib(&manifest, &package, &name)?;

    upsert_plugin(
        &uuid,
        &name,
        &manifest,
        &plugins_type,
        &version,
        &target,
        &platform,
        &req.dir,
        use_ai,
    )?;
    let _ = record_version(&uuid, &version);

    let (skills_added, skills_updated, _) = sync_skills(&package, &uuid, false)?;
    let prompts = sync_prompts(&package, &uuid, &name, &plugins_type)?;
    let (taskflows_added, taskflows_updated, _) = sync_taskflows(&package, &uuid, false)?;
    let (functions_added, functions_updated, _) =
        sync_functions(&package, &uuid, use_ai, false)?;

    Ok(json!({
        "plugin_uuid": uuid,
        "name": name,
        "version": version,
        "install_path": req.dir,
        "platform": platform,
        "lib_file": lib_file,
        "skipped": false,
        "counts": {
            "skills": skills_added + skills_updated,
            "prompts": prompts,
            "taskflows": taskflows_added + taskflows_updated,
            "functions": functions_added + functions_updated,
        }
    }))
}

pub fn update(req: UpdatePackageReq) -> AppResult<Value> {
    let package = validate_package(&req.dir)?;
    let manifest = read_manifest(&package)?;
    let manifest_uuid = manifest
        .get("uuid")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let manifest_name = require_manifest_text(&manifest, "name")?;

    let (uuid, from_version) = find_installed(
        req.plugin_uuid
            .as_deref()
            .filter(|value| !value.trim().is_empty()),
        req.name
            .as_deref()
            .filter(|value| !value.trim().is_empty()),
        &manifest_uuid,
        &manifest_name,
    )?;

    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0")
        .to_string();

    if let Some(current) = from_version.as_deref()
        && compare_versions(current, &version) != Ordering::Less
    {
        return Ok(json!({
            "plugin_uuid": uuid,
            "name": manifest_name,
            "from_version": current,
            "to_version": version,
            "skipped": true,
            "reason": "installed version is not lower than package version",
            "updated": { "skills": 0, "prompts": 0, "taskflows": 0, "functions": 0 },
            "added": { "skills": 0, "taskflows": 0, "functions": 0 },
            "removed": { "skills": 0, "taskflows": 0, "functions": 0 },
        }));
    }

    let plugins_type = manifest
        .get("plugins_type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let target = manifest
        .get("target")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("common")
        .to_string();
    let use_ai = flag_value(manifest.get("use_ai")).unwrap_or(0);
    let platform = platform_code();
    let lib_file = locate_lib(&manifest, &package, &manifest_name)?;
    let prune = req.prune.unwrap_or(true);

    upsert_plugin(
        &uuid,
        &manifest_name,
        &manifest,
        &plugins_type,
        &version,
        &target,
        &platform,
        &req.dir,
        use_ai,
    )?;

    let (skills_added, skills_updated, skills_removed) =
        sync_skills(&package, &uuid, prune)?;
    let prompts = sync_prompts(&package, &uuid, &manifest_name, &plugins_type)?;
    let (taskflows_added, taskflows_updated, taskflows_removed) =
        sync_taskflows(&package, &uuid, prune)?;
    let (functions_added, functions_updated, functions_removed) =
        sync_functions(&package, &uuid, use_ai, prune)?;

    let _ = record_version(&uuid, &version);

    Ok(json!({
        "plugin_uuid": uuid,
        "name": manifest_name,
        "from_version": from_version.unwrap_or_default(),
        "to_version": version,
        "install_path": req.dir,
        "platform": platform,
        "lib_file": lib_file,
        "skipped": false,
        "updated": {
            "skills": skills_updated,
            "prompts": if prompts > 0 { 1 } else { 0 },
            "taskflows": taskflows_updated,
            "functions": functions_updated,
        },
        "added": {
            "skills": skills_added,
            "taskflows": taskflows_added,
            "functions": functions_added,
        },
        "removed": {
            "skills": skills_removed,
            "taskflows": taskflows_removed,
            "functions": functions_removed,
        }
    }))
}

fn validate_package(dir: &str) -> AppResult<PathBuf> {
    if dir.trim().is_empty() {
        return Err(AppError::param("dir is required"));
    }
    let path = PathBuf::from(dir);
    if !path.is_dir() {
        return Err(AppError::file(format!("插件目录不存在: {dir}")));
    }
    Ok(path)
}

fn read_manifest(package: &Path) -> AppResult<Value> {
    let path = package.join("dytes.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 {} 失败: {error}", path.display())))?;
    if !value.is_object() {
        return Err(AppError::param("dytes.json 必须是 JSON 对象"));
    }
    Ok(value)
}

fn require_manifest_text(manifest: &Value, key: &str) -> AppResult<String> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::param(format!("dytes.json 缺少 {key}")))
}

fn platform_code() -> &'static str {
    if cfg!(windows) {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn export_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn lib_ext() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn locate_lib(manifest: &Value, package: &Path, name: &str) -> AppResult<String> {
    let ext = lib_ext();
    let mut candidates = Vec::new();

    if let Some(exports) = manifest
        .get("platform-exports")
        .and_then(Value::as_object)
        .and_then(|map| map.get(export_platform()))
        .and_then(Value::as_object)
    {
        for (source, dest) in exports {
            let dest = dest.as_str().unwrap_or("");
            if dest.ends_with(ext) {
                candidates.push(dest.to_string());
            } else if source.ends_with(ext) {
                candidates.push(source.to_string());
            }
        }
    }

    if candidates.is_empty()
        && let Ok(entries) = fs::read_dir(package)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            let text = path.to_string_lossy();
            if path.is_file() && text.ends_with(ext) {
                candidates.push(file_name(&text));
            }
        }
    }

    let existing: Vec<String> = candidates
        .into_iter()
        .filter(|candidate| {
            let candidate = file_name(candidate);
            package.join(&candidate).is_file() || package.join(&candidate).exists()
        })
        .collect();

    if existing.is_empty() {
        return Err(AppError::file(format!(
            "未找到当前平台（{}）的库文件: {}",
            platform_code(),
            ext
        )));
    }

    let preferred = existing.iter().find(|candidate| {
        let stem = Path::new(candidate)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        stem == name || stem == format!("lib{name}")
    });

    Ok(file_name(preferred.unwrap_or(&existing[0])))
}

fn upsert_plugin(
    uuid: &str,
    name: &str,
    manifest: &Value,
    plugins_type: &str,
    version: &str,
    target: &str,
    platform: &str,
    install_path: &str,
    use_ai: i64,
) -> AppResult<()> {
    let description = manifest
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let timestamp = now();
    let existing = db::query(
        PLUGIN_DB,
        &format!("SELECT uuid FROM plugins WHERE uuid = {} LIMIT 1", quote(uuid)),
    )?;

    if existing.is_empty() {
        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT INTO plugins \
                 (uuid, plugins_type, version, name, \"desc\", target, platform, install_path, \
                  installed, enabled, mask, use_ai, status, created_at, updated_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 1, 1, 0, {use_ai}, 'draft', \
                         {timestamp}, {timestamp})",
                quote(uuid),
                quote(plugins_type),
                quote(version),
                quote(name),
                quote(description),
                quote(target),
                quote(platform),
                quote(install_path),
            ),
        )?;
    } else {
        db::update(
            PLUGIN_DB,
            &format!(
                "UPDATE plugins SET plugins_type = {}, version = {}, name = {}, \"desc\" = {}, \
                 target = {}, platform = {}, install_path = {}, installed = 1, \
                 use_ai = {use_ai}, updated_at = {timestamp} WHERE uuid = {}",
                quote(plugins_type),
                quote(version),
                quote(name),
                quote(description),
                quote(target),
                quote(platform),
                quote(install_path),
                quote(uuid),
            ),
        )?;
    }

    Ok(())
}

fn find_installed(
    uuid: Option<&str>,
    name: Option<&str>,
    manifest_uuid: &str,
    manifest_name: &str,
) -> AppResult<(String, Option<String>)> {
    let (field, value) = match (uuid, name) {
        (Some(uuid), _) => ("uuid", uuid),
        (None, Some(name)) => ("name", name),
        (None, None) if !manifest_uuid.is_empty() => ("uuid", manifest_uuid),
        (None, None) => ("name", manifest_name),
    };

    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid, name, version FROM plugins WHERE {field} = {} AND deleted_at IS NULL LIMIT 1",
            quote(value)
        ),
    )?;
    let Some(row) = rows.first() else {
        return Err(AppError::not_found(format!("插件未安装: {value}")));
    };
    let uuid = row
        .get("uuid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let version = row
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current_version(&uuid));
    Ok((uuid, version))
}

fn installed_plugin(uuid: &str) -> AppResult<Option<(String, String)>> {
    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT name, version FROM plugins WHERE uuid = {} AND deleted_at IS NULL LIMIT 1",
            quote(uuid)
        ),
    )?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let name = row
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let version = row
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current_version(uuid))
        .unwrap_or_default();
    Ok(Some((name, version)))
}

fn compare_versions(current: &str, incoming: &str) -> Ordering {
    let parts = |value: &str| -> Vec<String> {
        value
            .trim()
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.split('-').next().unwrap_or("").to_string())
            .collect()
    };
    let current = parts(current);
    let incoming = parts(incoming);
    let len = current.len().max(incoming.len());

    for index in 0..len {
        let a = current.get(index).map(String::as_str).unwrap_or("");
        let b = incoming.get(index).map(String::as_str).unwrap_or("");
        let order = match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            _ => a.cmp(b),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn current_version(uuid: &str) -> Option<String> {
    db::query(
        PLUGIN_DB,
        &format!(
            "SELECT version FROM plugin_versions WHERE plugin_uuid = {} \
             ORDER BY id DESC LIMIT 1",
            quote(uuid)
        ),
    )
    .ok()?
    .first()
    .and_then(|row| row.get("version"))
    .and_then(Value::as_str)
    .map(str::to_string)
}

fn record_version(uuid: &str, version: &str) -> AppResult<()> {
    let timestamp = now();
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugin_versions SET is_latest = 0 WHERE plugin_uuid = {}",
            quote(uuid)
        ),
    )?;
    db::execute(
        PLUGIN_DB,
        &format!(
            "INSERT INTO plugin_versions \
             (plugin_uuid, version, status, is_latest, created_at, updated_at) \
             VALUES ({}, {}, 'active', 1, {timestamp}, {timestamp})",
            quote(uuid),
            quote(version),
        ),
    )?;
    Ok(())
}

struct SkillFile {
    id: String,
    name: String,
    content: String,
    sort: i64,
}

fn read_skill_files(package: &Path) -> AppResult<Vec<SkillFile>> {
    let dir = package.join("skills");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", dir.display())))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md")
        })
        .collect();
    paths.sort();

    let mut files = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let content = fs::read_to_string(path)
            .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", path.display())))?;
        if content.trim().is_empty() {
            continue;
        }
        let name = content
            .lines()
            .find_map(|line| {
                line.strip_prefix("# ")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| id.clone());
        files.push(SkillFile {
            id,
            name,
            content,
            sort: index as i64 * 10,
        });
    }
    Ok(files)
}

fn sync_skills(package: &Path, uuid: &str, prune: bool) -> AppResult<(usize, usize, usize)> {
    let files = read_skill_files(package)?;
    let existing = designdb::query_all(json!({ "plugin_uuid": uuid }))?;
    let mut existing_map = HashMap::new();
    for doc in existing {
        if let Some(id) = doc.get("_id").or_else(|| doc.get("id")).and_then(Value::as_str) {
            existing_map.insert(id.to_string(), doc);
        }
    }

    let timestamp = now();
    let mut added = 0;
    let mut updated = 0;

    for file in &files {
        match existing_map.get(&file.id) {
            Some(doc) => {
                let same = doc.get("content").and_then(Value::as_str)
                    == Some(file.content.as_str())
                    && doc.get("name").and_then(Value::as_str) == Some(file.name.as_str())
                    && doc.get("category").and_then(Value::as_str) == Some("plugin");
                if !same {
                    designdb::update(
                        json!({ "_id": file.id }),
                        json!({
                            "$set": {
                                "plugin_uuid": uuid,
                                "name": file.name,
                                "content": file.content,
                                "category": "plugin",
                                "sort": file.sort,
                                "updated_at": timestamp,
                            },
                            "$inc": { "version": 1 }
                        }),
                    )?;
                    updated += 1;
                }
            }
            None => {
                designdb::insert(json!({
                    "_id": file.id,
                    "id": file.id,
                    "plugin_uuid": uuid,
                    "name": file.name,
                    "description": "",
                    "category": "plugin",
                    "content": file.content,
                    "trigger": "",
                    "params_schema": {},
                    "is_system": 0,
                    "is_enabled": 1,
                    "sort": file.sort,
                    "version": 1,
                    "created_at": timestamp,
                    "updated_at": timestamp,
                }))?;
                added += 1;
            }
        }
    }

    let mut removed = 0;
    if prune {
        let keep: HashSet<&str> = files.iter().map(|file| file.id.as_str()).collect();
        for id in existing_map.keys() {
            if !keep.contains(id.as_str()) {
                designdb::delete(json!({ "_id": id }))?;
                removed += 1;
            }
        }
    }

    Ok((added, updated, removed))
}

fn read_prompts(package: &Path) -> AppResult<Option<Vec<Value>>> {
    let path = package.join("prompts.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 {} 失败: {error}", path.display())))?;
    let list = value
        .as_array()
        .cloned()
        .or_else(|| value.get("prompts").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    Ok(Some(list))
}

fn sync_prompts(
    package: &Path,
    uuid: &str,
    name: &str,
    plugins_type: &str,
) -> AppResult<usize> {
    let Some(prompts) = read_prompts(package)? else {
        return Ok(0);
    };
    let count = prompts.len();
    designdb::set_prompts_doc(uuid, prompts.clone())?;
    sync_prompts_kv(uuid, name, plugins_type, &prompts)?;
    Ok(count)
}

fn sync_prompts_kv(
    uuid: &str,
    name: &str,
    plugins_type: &str,
    prompts: &[Value],
) -> AppResult<usize> {
    if !kv::available() {
        return Ok(0);
    }
    let Some((collection, label, description, sort)) = kv_collection(plugins_type) else {
        return Ok(0);
    };

    let records: Vec<(String, String)> = prompts
        .iter()
        .filter_map(|prompt| {
            prompt_content(prompt).map(|content| (prompt_key(uuid, prompt, content), content.to_string()))
        })
        .collect();
    if records.is_empty() {
        return Ok(0);
    }

    kv::ensure_collection(collection, label, description, sort)?;
    let prefix = format!("{uuid}:prompt:");
    let old = kv::list_records(collection, &prefix)?;
    let old_map: HashMap<String, String> = old.into_iter().collect();
    let new_keys: HashSet<&str> = records.iter().map(|(key, _)| key.as_str()).collect();
    let meta = json!({ "plugin_uuid": uuid, "plugin_name": name, "kind": "prompt" });

    let mut items = Vec::new();
    for (index, (key, content)) in records.iter().enumerate() {
        if old_map.get(key).map(String::as_str) == Some(content.as_str()) {
            continue;
        }
        items.push(json!({
            "key": key,
            "value": content,
            "text": content,
            "meta": meta.clone(),
            "rank": index as i64 + 1,
            "force": true,
        }));
    }

    if !items.is_empty() {
        kv::put_records(collection, &items)?;
    }

    let stale: Vec<String> = old_map
        .keys()
        .filter(|key| !new_keys.contains(key.as_str()))
        .cloned()
        .collect();
    if !stale.is_empty() {
        kv::delete_records(collection, &stale)?;
    }

    Ok(items.len() + stale.len())
}

fn kv_collection(plugins_type: &str) -> Option<(&'static str, &'static str, &'static str, i64)> {
    match plugins_type {
        "chat_think" => Some((
            "chat_think",
            "聊天思考",
            "think 阶段检索词与技能",
            10,
        )),
        "chat_reply" => Some((
            "chat_reply",
            "聊天回复",
            "reply 阶段检索词与技能",
            20,
        )),
        "chat_plugins" => Some((
            "chat_plugins",
            "聊天插件",
            "chat_plugins 分类插件的能力描述",
            30,
        )),
        "sys_plugins" => Some((
            "sys_plugins",
            "系统插件",
            "sys_plugins 分类插件的能力描述",
            40,
        )),
        "plugins" | "plugins_coreldraw" => Some((
            "chat_plugin",
            "聊天插件",
            "chat 分类插件的能力描述",
            30,
        )),
        _ => None,
    }
}

fn prompt_content(prompt: &Value) -> Option<&str> {
    prompt
        .as_str()
        .or_else(|| prompt.get("content").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn prompt_key(uuid: &str, prompt: &Value, content: &str) -> String {
    let ident = prompt
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| prompt.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| content_hash(content));
    let ident = ident.replace([':', '/', '\\'], "_");
    format!("{uuid}:prompt:{ident}")
}

fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_taskflows(package: &Path) -> AppResult<Option<Vec<Value>>> {
    let path = package.join("taskflow.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 {} 失败: {error}", path.display())))?;
    let list = value
        .as_array()
        .cloned()
        .or_else(|| value.get("taskflows").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    Ok(Some(list))
}

fn taskflow_key(item: &Value) -> Option<String> {
    item.get("uid")
        .or_else(|| item.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn sync_taskflows(
    package: &Path,
    uuid: &str,
    prune: bool,
) -> AppResult<(usize, usize, usize)> {
    let Some(incoming) = read_taskflows(package)? else {
        return Ok((0, 0, 0));
    };

    let mut list = designdb::get_taskflow_doc()?
        .and_then(|doc| doc.get("taskflows").cloned())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();

    let incoming_keys: HashSet<String> = incoming.iter().filter_map(taskflow_key).collect();
    let mut added = 0;
    let mut updated = 0;

    for item in incoming {
        let mut item = item;
        if let Some(object) = item.as_object_mut() {
            object
                .entry("plugin_uuid".to_string())
                .or_insert_with(|| json!(uuid));
        }
        match taskflow_key(&item) {
            Some(key) => match list
                .iter()
                .position(|old| taskflow_key(old).as_deref() == Some(key.as_str()))
            {
                Some(index) => {
                    if list[index] != item {
                        list[index] = item;
                        updated += 1;
                    }
                }
                None => {
                    list.push(item);
                    added += 1;
                }
            },
            None => {
                list.push(item);
                added += 1;
            }
        }
    }

    let mut removed = 0;
    if prune {
        let before = list.len();
        list.retain(|item| {
            let owned = item.get("plugin_uuid").and_then(Value::as_str) == Some(uuid);
            if !owned {
                return true;
            }
            taskflow_key(item)
                .map(|key| incoming_keys.contains(&key))
                .unwrap_or(false)
        });
        removed = before - list.len();
    }

    designdb::set_taskflow_doc(list)?;
    Ok((added, updated, removed))
}

fn read_tools(package: &Path) -> AppResult<Option<Vec<Value>>> {
    let candidates = [package.join("mcp.json"), package.join("mcp").join("mcp.json")];
    let Some(path) = candidates.iter().find(|path| path.is_file()) else {
        return Ok(None);
    };
    let text = fs::read_to_string(path)
        .map_err(|error| AppError::file(format!("读取 {} 失败: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 {} 失败: {error}", path.display())))?;
    Ok(Some(
        value
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    ))
}

fn sync_functions(
    package: &Path,
    uuid: &str,
    plugin_use_ai: i64,
    prune: bool,
) -> AppResult<(usize, usize, usize)> {
    let Some(tools) = read_tools(package)? else {
        return Ok((0, 0, 0));
    };

    let mut keep = HashSet::new();
    let mut added = 0;
    let mut updated = 0;

    for tool in &tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| tool.get("ffi_symbol").and_then(Value::as_str))
            .unwrap_or("")
            .trim();
        if name.is_empty() {
            continue;
        }
        let uuid_value = tool
            .get("uuid")
            .or_else(|| tool.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{uuid}:{name}"));
        keep.insert(uuid_value.clone());

        let ffi_symbol = tool.get("ffi_symbol").and_then(Value::as_str).unwrap_or(name);
        let title = tool.get("title").and_then(Value::as_str).unwrap_or(name);
        let description = tool.get("description").and_then(Value::as_str).unwrap_or("");
        let params_schema = tool
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));
        let params_example = tool.get("inputExample").cloned();
        let return_schema = tool
            .get("outputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));
        let return_example = tool.get("outputExample").cloned();
        let use_ai = flag_value(tool.get("use_ai")).unwrap_or(plugin_use_ai);

        let exists = !db::query(
            PLUGIN_DB,
            &format!(
                "SELECT uuid FROM plugin_functions WHERE uuid = {} LIMIT 1",
                quote(&uuid_value)
            ),
        )?
        .is_empty();

        if exists {
            plugin_ops::update_function(UpdateFunctionReq {
                uuid: uuid_value,
                plugin_uuid: uuid.to_string(),
                ffi_symbol: ffi_symbol.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                params_schema: Some(params_schema),
                params_example,
                return_schema: Some(return_schema),
                return_example,
                use_ai: Some(json!(use_ai)),
            })?;
            updated += 1;
        } else {
            plugin_ops::post_function(PostFunctionReq {
                uuid: uuid_value,
                plugin_uuid: uuid.to_string(),
                ffi_symbol: ffi_symbol.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                params_schema: Some(params_schema),
                params_example,
                return_schema: Some(return_schema),
                return_example,
                use_ai: Some(json!(use_ai)),
            })?;
            added += 1;
        }
    }

    let mut removed = 0;
    if prune {
        let rows = db::query(
            PLUGIN_DB,
            &format!(
                "SELECT uuid FROM plugin_functions WHERE plugin_uuid = {}",
                quote(uuid)
            ),
        )?;
        for row in rows {
            if let Some(id) = row.get("uuid").and_then(Value::as_str)
                && !keep.contains(id)
            {
                db::delete(
                    PLUGIN_DB,
                    &format!("DELETE FROM plugin_functions WHERE uuid = {}", quote(id)),
                )?;
                removed += 1;
            }
        }
    }

    Ok((added, updated, removed))
}

fn flag_value(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Number(value) => value.as_i64(),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(1),
            "0" | "false" | "no" | "off" => Some(0),
            _ => None,
        },
        _ => None,
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
