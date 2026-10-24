//! `./info` 的 `setting` 读写：插件设置与用户配置值。
//!
//! 约定（与 `setting_ui` 一致）：`setting.key` 全局唯一，`category = 'plugin'`；
//! 插件配置的用户值 key 为 `plugin.config.<plugin_uuid>.<param_key>`。

use crate::utils::db::{self, quote};
use crate::utils::error::AppResult;
use std::collections::HashMap;

pub const PLUGIN_DB: &str = "./plugins";
pub const INFO_DB: &str = "./info";

const USER_CONFIG_PREFIX: &str = "plugin.config.";

pub fn user_config_key(plugin_uuid: &str, param_key: &str) -> String {
    format!("{USER_CONFIG_PREFIX}{plugin_uuid}.{param_key}")
}

fn prefix_for(plugin_uuid: &str) -> String {
    format!("{USER_CONFIG_PREFIX}{plugin_uuid}.")
}

/// 读取单个设置值。
pub fn get_setting(key: &str) -> AppResult<Option<String>> {
    let rows = db::query(
        INFO_DB,
        &format!("SELECT value FROM setting WHERE key = {} LIMIT 1", quote(key)),
    )?;

    Ok(rows
        .first()
        .and_then(|row| row.get("value"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string))
}

/// 写入单个设置值（UPSERT，保留用户已有值之外的其他字段由调用方决定）。
pub fn set_setting(
    key: &str,
    title: &str,
    value: &str,
    value_type: &str,
    category: &str,
    description: &str,
) -> AppResult<()> {
    let sql = format!(
        "INSERT INTO setting (key, title, value, value_type, category, description, created_at, updated_at) \
         VALUES ({}, {}, {}, {}, {}, {}, datetime('now'), datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET title = excluded.title, value = excluded.value, \
         value_type = excluded.value_type, category = excluded.category, \
         description = excluded.description, updated_at = excluded.updated_at",
        quote(key),
        quote(title),
        quote(value),
        quote(value_type),
        quote(category),
        quote(description),
    );

    db::execute(INFO_DB, &sql)
}

/// 读取某插件的全部用户配置值：`param_key -> value`。
pub fn get_user_configs(plugin_uuid: &str) -> AppResult<HashMap<String, String>> {
    let prefix = prefix_for(plugin_uuid);

    // 用 substr 精确匹配前缀，避免 LIKE 把 `_` / `%` 当通配符。
    let sql = format!(
        "SELECT key, value FROM setting WHERE category = 'plugin' AND substr(key, 1, {}) = {}",
        prefix.chars().count(),
        quote(&prefix),
    );

    let rows = db::query(INFO_DB, &sql)?;
    let mut map = HashMap::new();

    for row in rows {
        let Some(key) = row.get("key").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(value) = row.get("value").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(param_key) = key.strip_prefix(&prefix) {
            map.insert(param_key.to_string(), value.to_string());
        }
    }

    Ok(map)
}

/// 保存单个用户配置值。
pub fn set_user_config(
    plugin_uuid: &str,
    param_key: &str,
    value: &str,
    value_type: &str,
    title: &str,
) -> AppResult<()> {
    let key = user_config_key(plugin_uuid, param_key);
    set_setting(
        &key,
        title,
        value,
        value_type,
        "plugin",
        "插件配置值（由 plugin_lib 写入）",
    )
}

/// 插件安装根目录，缺省 `./storage/plugins`。
pub fn plugin_dir() -> String {
    get_setting("plugin_dir")
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "./storage/plugins".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_config_key_is_prefixed_and_stable() {
        assert_eq!(
            user_config_key("p-1", "output_dir"),
            "plugin.config.p-1.output_dir"
        );
        assert_eq!(prefix_for("p-1"), "plugin.config.p-1.");
    }
}
