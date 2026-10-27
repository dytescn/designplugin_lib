//! 设计插件安装 / 卸载。
//!
//! 安装：从 `design_plugins/<name>/dytes.json` 读取清单，下载 exports 到 `./plugins/<name>/`，
//! 然后交给 `scanner::scan` 登记数据库。cyctron 直接加载 `./plugins/<name>/` 下的动态库，
//! 不再复制到 `./libary/`（该目录只放 sys 能力库，例如 `plugin_lib` 自身）。
//! 卸载：删除安装目录；旧安装遗留在 `./libary/` 的同名副本仅在内容一致时清理。
//! 库/函数记录会被删除，插件与版本记录保留（`installed = 0`）。

use crate::utils::cloud;
use crate::utils::config::PLUGIN_DB;
use crate::utils::db::{self, quote, quote_opt};
use crate::utils::error::{AppError, AppResult};
use crate::utils::scanner::{self, ScanReq};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
pub struct InstallReq {
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UninstallReq {
    #[serde(default)]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

pub fn install(req: InstallReq) -> AppResult<Value> {
    let name = resolve_name(req.name.as_deref(), req.plugin_uuid.as_deref())?;
    validate_name(&name)?;
    let started_at = now();

    match install_inner(&name) {
        Ok((value, plugin_uuid, version, install_path)) => {
            record_install(
                &plugin_uuid,
                &version,
                &install_path,
                "installed",
                started_at,
                None,
            )?;
            Ok(value)
        }
        Err(error) => {
            if let Ok(Some(plugin_uuid)) = find_plugin_uuid(&name) {
                let version = req.version.unwrap_or_else(|| "unknown".to_string());
                let install_path = format!("./plugins/{name}");
                let _ = record_install(
                    &plugin_uuid,
                    &version,
                    &install_path,
                    "failed",
                    started_at,
                    Some(&error.msg),
                );
            }
            Err(error)
        }
    }
}

fn install_inner(name: &str) -> AppResult<(Value, String, String, String)> {
    let manifest_key = format!("design_plugins/{name}/dytes.json");
    let text = cloud::fetch_text(&manifest_key)?;
    let manifest: Value = serde_json::from_str(&text)
        .map_err(|error| AppError::param(format!("解析 {manifest_key} 失败: {error}")))?;

    let remote_version = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0")
        .to_string();

    let platform_exports = manifest
        .get("platform-exports")
        .and_then(Value::as_object)
        .and_then(|value| value.get("windows"))
        .and_then(Value::as_object);
    let exports = platform_exports
        .or_else(|| manifest.get("exports").and_then(Value::as_object))
        .ok_or_else(|| AppError::param(format!("{name} 缺少 exports")))?;

    let base = if platform_exports.is_some() {
        format!("design_plugins/{name}/{remote_version}/windows/")
    } else {
        format!("design_plugins/{name}/{remote_version}/")
    };

    let package_dir = format!("./plugins/{name}");
    fs::create_dir_all(&package_dir)
        .map_err(|error| AppError::db(format!("创建 {package_dir} 失败: {error}")))?;

    let mut downloads = Vec::new();
    let mut files = Vec::new();
    for (_local, remote) in exports {
        let Some(remote_name) = remote.as_str() else {
            continue;
        };
        let file_name = Path::new(remote_name)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if file_name.is_empty() || file_name != remote_name {
            continue;
        }
        downloads.push((format!("{base}{file_name}"), format!("{package_dir}/{file_name}")));
        files.push(file_name.to_string());
    }

    cloud::download_objects(&downloads)?;

    // 保存云端清单，scan 需要读取 use_ai / targets 等字段（G-47）。
    fs::write(format!("{package_dir}/dytes.json"), &text)
        .map_err(|error| AppError::db(format!("保存 dytes.json 失败: {error}")))?;

    let use_ai = scanner::use_ai_from_manifest(&manifest);
    scanner::ensure_use_ai_columns()?;
    let plugin_uuid = upsert_plugin(name, &remote_version, &package_dir, use_ai)?;
    scanner::scan(ScanReq {
        dir: Some("./plugins".to_string()),
        persist: Some(true),
    })?;

    let value = json!({
        "plugin_uuid": plugin_uuid,
        "name": name,
        "version": remote_version,
        "install_path": package_dir,
        "files": files,
    });
    Ok((value, plugin_uuid, remote_version, package_dir))
}

pub fn uninstall(req: UninstallReq) -> AppResult<Value> {
    let name = resolve_name(req.name.as_deref(), req.plugin_uuid.as_deref())?;
    validate_name(&name)?;

    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugins WHERE name = {} AND deleted_at IS NULL LIMIT 1",
            quote(&name)
        ),
    )?;
    let Some(row) = rows.first() else {
        return Err(AppError::not_found(format!("插件不存在: {name}")));
    };
    let plugin_uuid = as_str(row, "uuid");

    let libs = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT file_name, install_path FROM plugin_libs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    for lib in &libs {
        remove_legacy_libary_copy(&as_str(lib, "file_name"), &as_str(lib, "install_path"));
    }

    let package_dir = format!("./plugins/{name}");
    if Path::new(&package_dir).exists() {
        fs::remove_dir_all(&package_dir)
            .map_err(|error| AppError::db(format!("删除 {package_dir} 失败: {error}")))?;
    }

    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_functions WHERE lib_uuid IN \
             (SELECT uuid FROM plugin_libs WHERE plugin_uuid = {})",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_libs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_installs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugins SET installed = 0, install_path = NULL, updated_at = {} WHERE uuid = {}",
            now(),
            quote(&plugin_uuid)
        ),
    )?;

    Ok(json!({
        "plugin_uuid": plugin_uuid,
        "name": name,
        "uninstalled": true,
    }))
}

/// 物理删除插件元数据与安装文件；返回 function uuid 供 chatllm 清理向量。
pub fn delete(req: UninstallReq) -> AppResult<Value> {
    let name = resolve_name(req.name.as_deref(), req.plugin_uuid.as_deref())?;
    validate_name(&name)?;

    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugins WHERE name = {} AND deleted_at IS NULL LIMIT 1",
            quote(&name)
        ),
    )?;
    let Some(row) = rows.first() else {
        return Err(AppError::not_found(format!("插件不存在: {name}")));
    };
    let plugin_uuid = as_str(row, "uuid").to_string();

    let function_rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugin_functions WHERE lib_uuid IN \
             (SELECT uuid FROM plugin_libs WHERE plugin_uuid = {})",
            quote(&plugin_uuid)
        ),
    )?;
    let function_uuids: Vec<String> = function_rows
        .iter()
        .map(|row| as_str(row, "uuid"))
        .filter(|uuid| !uuid.is_empty())
        .collect();

    let libs = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT file_name, install_path FROM plugin_libs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    for lib in &libs {
        remove_legacy_libary_copy(&as_str(lib, "file_name"), &as_str(lib, "install_path"));
    }
    let package_dir = format!("./plugins/{name}");
    if Path::new(&package_dir).exists() {
        fs::remove_dir_all(&package_dir)
            .map_err(|error| AppError::db(format!("删除 {package_dir} 失败: {error}")))?;
    }

    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_installs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_configs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_versions WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_relations WHERE plugin_uuid = {} OR related_uuid = {}",
            quote(&plugin_uuid),
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_functions WHERE lib_uuid IN \
             (SELECT uuid FROM plugin_libs WHERE plugin_uuid = {})",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!(
            "DELETE FROM plugin_libs WHERE plugin_uuid = {}",
            quote(&plugin_uuid)
        ),
    )?;
    db::delete(
        PLUGIN_DB,
        &format!("DELETE FROM plugins WHERE uuid = {}", quote(&plugin_uuid)),
    )?;

    Ok(json!({
        "plugin_uuid": plugin_uuid,
        "name": name,
        "deleted": true,
        "function_uuids": function_uuids,
    }))
}

/// 清理旧安装复制到 `./libary/` 的副本。
///
/// 只有 legacy 文件与插件目录中的 DLL 内容完全一致时才删除，避免误删同名
/// 的系统能力库（例如 `database.dll`）。
fn remove_legacy_libary_copy(file_name: &str, install_path: &str) {
    if file_name.is_empty() || file_name.contains('/') || file_name.contains('\\') {
        return;
    }
    let legacy = format!("./libary/{file_name}");
    remove_file_if_identical(Path::new(&legacy), Path::new(install_path));
}

fn remove_file_if_identical(legacy: &Path, installed: &Path) {
    let (Ok(legacy_bytes), Ok(installed_bytes)) = (fs::read(legacy), fs::read(installed)) else {
        return;
    };
    if legacy_bytes == installed_bytes {
        let _ = fs::remove_file(legacy);
    }
}

fn record_install(
    plugin_uuid: &str,
    version: &str,
    install_path: &str,
    status: &str,
    started_at: i64,
    last_error: Option<&str>,
) -> AppResult<()> {
    let timestamp = now();
    let version = if version.trim().is_empty() {
        "unknown"
    } else {
        version
    };
    db::execute(
        PLUGIN_DB,
        &format!(
            "INSERT INTO plugin_installs \
             (plugin_uuid, version, install_path, file_hash, enabled, status, started_at, last_error, created_at, updated_at) \
             VALUES ({}, {}, {}, NULL, 1, {}, {started_at}, {}, {timestamp}, {timestamp}) \
             ON CONFLICT(plugin_uuid, version) DO UPDATE SET \
             install_path = excluded.install_path, status = excluded.status, \
             started_at = excluded.started_at, last_error = excluded.last_error, \
             updated_at = excluded.updated_at",
            quote(plugin_uuid),
            quote(version),
            quote(install_path),
            quote(status),
            quote_opt(last_error),
        ),
    )
}

fn find_plugin_uuid(name: &str) -> AppResult<Option<String>> {
    let rows = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugins WHERE name = {} AND deleted_at IS NULL LIMIT 1",
            quote(name)
        ),
    )?;
    Ok(rows
        .first()
        .map(|row| as_str(row, "uuid"))
        .filter(|uuid| !uuid.is_empty()))
}

fn resolve_name(name: Option<&str>, uuid: Option<&str>) -> AppResult<String> {
    if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
        return Ok(name.to_string());
    }
    let Some(uuid) = uuid.filter(|value| !value.trim().is_empty()) else {
        return Err(AppError::param("name 或 plugin_uuid 必填"));
    };

    let rows = db::query(
        PLUGIN_DB,
        &format!("SELECT name FROM plugins WHERE uuid = {} LIMIT 1", quote(uuid)),
    )?;
    rows.first()
        .and_then(|row| row.get("name"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .ok_or_else(|| AppError::not_found(format!("插件不存在: {uuid}")))
}

fn validate_name(name: &str) -> AppResult<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(AppError::param(format!("非法插件名: {name}")));
    }
    Ok(())
}

fn upsert_plugin(
    name: &str,
    version: &str,
    package_dir: &str,
    use_ai: Option<i64>,
) -> AppResult<String> {
    let timestamp = now();
    let existing = db::query(
        PLUGIN_DB,
        &format!(
            "SELECT uuid FROM plugins WHERE name = {} AND deleted_at IS NULL LIMIT 1",
            quote(name)
        ),
    )?;

    let plugin_uuid = if let Some(row) = existing.first() {
        as_str(row, "uuid")
    } else {
        let uuid = Uuid::new_v4().to_string();
        db::execute(
            PLUGIN_DB,
            &format!(
                "INSERT INTO plugins (uuid, name, \"desc\", status, public, created_at, updated_at, \
                 installed, install_path, enabled, plugin_type, target, use_ai) \
                 VALUES ({}, {}, {}, 'published', 0, {timestamp}, {timestamp}, 0, NULL, 1, 'dll', 'common', {})",
                quote(&uuid),
                quote(name),
                quote(name),
                use_ai.unwrap_or(0)
            ),
        )?;
        uuid
    };

    let use_ai_sql = use_ai
        .map(|value| format!(", use_ai = {value}"))
        .unwrap_or_default();
    db::update(
        PLUGIN_DB,
        &format!(
            "UPDATE plugins SET installed = 1, install_path = {}, updated_at = {timestamp}{use_ai_sql} WHERE uuid = {}",
            quote(package_dir),
            quote(&plugin_uuid)
        ),
    )?;

    upsert_version(&plugin_uuid, version)?;
    Ok(plugin_uuid)
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

fn as_str(row: &Value, key: &str) -> String {
    row.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_libary_copy_removed_only_when_identical() {
        let dir = std::env::temp_dir().join(format!("plugin_lib-legacy-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let installed = dir.join("plugin.dll");
        let legacy = dir.join("legacy.dll");

        fs::write(&installed, b"same-bytes").expect("write installed");
        fs::write(&legacy, b"same-bytes").expect("write legacy");
        remove_file_if_identical(&legacy, &installed);
        assert!(!legacy.exists(), "identical legacy copy should be removed");

        fs::write(&legacy, b"different-bytes").expect("write legacy");
        remove_file_if_identical(&legacy, &installed);
        assert!(legacy.exists(), "different file must be kept");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_name_with_separator_is_ignored() {
        remove_legacy_libary_copy("../database.dll", "./plugins/x/x.dll");
        remove_legacy_libary_copy("sub/dll.dll", "./plugins/x/x.dll");
        assert!(!Path::new("./libary/sub/dll.dll").exists());
    }
}
