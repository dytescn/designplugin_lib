//! 设计插件更新检查。
//!
//! 本地版本来自 `./dytes.lock`（`type = design_plugins`），
//! 云端版本来自 `design_plugins/packages.json`。
//! 返回每个已安装插件是否需要更新；未安装的云端包不进入结果。

use crate::utils::cloud;
use crate::utils::config::PLUGIN_DB;
use crate::utils::db;
use crate::utils::error::{AppError, AppResult};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn check_updates() -> AppResult<Value> {
    let remote_text = cloud::fetch_text("design_plugins/packages.json")?;
    let remote: Value = serde_json::from_str(&remote_text)
        .map_err(|error| AppError::param(format!("解析 design_plugins/packages.json 失败: {error}")))?;
    let packages = remote
        .get("packages")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let local = read_local_versions("./dytes.lock");
    let mut updates = Vec::new();

    for (name, local_version) in &local {
        let Some(remote_value) = packages.get(name) else {
            continue;
        };
        let remote_version = remote_value.as_str().unwrap_or("0.0.0");
        updates.push(json!({
            "name": name,
            "local_version": local_version,
            "remote_version": remote_version,
            "needs_update": compare(remote_version, local_version),
        }));
    }

    updates.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .cmp(&right.get("name").and_then(Value::as_str))
    });

    let count = updates
        .iter()
        .filter(|item| item.get("needs_update").and_then(Value::as_bool).unwrap_or(false))
        .count();

    Ok(json!({
        "checked_at": now(),
        "count": count,
        "updates": updates,
    }))
}

/// 本地版本优先读取数据库（plugin_lib 安装后写入），再回退 `dytes.lock`。
fn read_local_versions(path: &str) -> HashMap<String, String> {
    let mut versions = HashMap::new();

    if let Ok(rows) = db::query(
        PLUGIN_DB,
        "SELECT p.name AS name, v.version AS version FROM plugins p \
         JOIN plugin_versions v ON v.plugin_uuid = p.uuid AND v.is_latest = 1 \
         WHERE p.installed = 1",
    ) {
        for row in rows {
            let Some(name) = row.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(version) = row.get("version").and_then(Value::as_str) else {
                continue;
            };
            versions.insert(name.to_string(), version.to_string());
        }
    }

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
        let package_type = package.get("type").and_then(Value::as_str).unwrap_or("");
        if package_type != "design_plugins" {
            continue;
        }
        let Some(name) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = package.get("version").and_then(Value::as_str) else {
            continue;
        };
        versions
            .entry(name.to_string())
            .or_insert_with(|| version.to_string());
    }

    versions
}

fn compare(remote: &str, local: &str) -> bool {
    match (semver::Version::parse(remote), semver::Version::parse(local)) {
        (Ok(remote), Ok(local)) => remote > local,
        _ => remote != local,
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}
