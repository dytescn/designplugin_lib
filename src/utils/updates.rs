//! 设计插件更新检查。
//!
//! 检查交给 `dytes update --type design_plugins --check --json`，本库不再
//! 直接读取云端 `packages.json`。dytes 会同时检查 `dytes.json` 声明的依赖
//! 与 `dytes.lock` 中已安装的插件。

use crate::utils::dytes;
use crate::utils::error::{AppError, AppResult};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn check_updates() -> AppResult<Value> {
    let root = std::env::current_dir()
        .map_err(|error| AppError::file(format!("读取当前目录失败: {error}")))?;

    let output = dytes::run(
        &root,
        &["update", "--type", "design_plugins", "--check", "--json"],
    )?;

    match output.code {
        // 0 = 无更新；4 = 有可用更新，JSON 仍写入 stdout。
        Some(0) | Some(4) => {}
        Some(code) => {
            return Err(AppError::file(format!(
                "dytes update --check 失败（退出码 {code}）: {}",
                output.stderr.trim()
            )))
        }
        None => {
            return Err(AppError::file(
                "dytes update --check 被信号终止".to_string(),
            ))
        }
    }

    let mut report = dytes::parse_json(&output)?;
    if !report.is_object() {
        return Err(AppError::file(
            "dytes 返回的检查结果不是 JSON 对象".to_string(),
        ));
    }
    if report.get("checked_at").is_none() {
        report["checked_at"] = serde_json::json!(now());
    }
    Ok(report)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}
