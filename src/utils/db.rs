//! `database.dll` 的 dlopen 封装。
//!
//! 约定（见 `funxdata/database`）：请求 `{path, sql}`，响应 `{code, data, msg}`；
//! `path` 是库名，实际文件为 `./database/<name>.db`。

/// 插件库名；`database` 服务中对应 `./database/plugins.db`。
/// database 1.0.5+ 的逻辑库名只允许 `[A-Za-z0-9_-]`。
pub const PLUGIN_DB: &str = "plugins";

use crate::utils::error::{AppError, AppResult};
use libloading::{Library, Symbol};
use once_cell::sync::OnceCell;
use serde_json::{Value, json};
use std::ffi::{CStr, CString, c_char, c_int};

/// database 1.0.5+ 约定 `json_out` 至少 1 MiB。
const DB_OUT_BUF_SIZE: usize = 1024 * 1024;

type DbFn = unsafe extern "C" fn(*const c_char, *mut c_char) -> c_int;

/// database 库路径；可用环境变量 `PLUGIN_LIB_DATABASE` 覆盖（测试/联调）。
pub fn database_path() -> String {
    if let Ok(path) = std::env::var("PLUGIN_LIB_DATABASE")
        && !path.trim().is_empty()
    {
        return path;
    }

    #[cfg(windows)]
    {
        "./libary/database.dll".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "./libary/database.so".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "./libary/database.dylib".to_string()
    }
}

static DB_LIB: OnceCell<Result<Library, String>> = OnceCell::new();

fn library() -> AppResult<&'static Library> {
    let entry = DB_LIB.get_or_init(|| {
        let path = database_path();
        // SAFETY: database.dll 由本应用安装，加载与符号调用都在同一进程内完成。
        unsafe { Library::new(&path) }.map_err(|e| format!("加载 {path} 失败: {e}"))
    });

    entry
        .as_ref()
        .map_err(|e| AppError::db(e.clone()))
}

fn call(symbol: &str, db: &str, sql: &str) -> AppResult<Value> {
    let lib = library()?;

    // SAFETY: 符号类型与 database 的 C ABI 一致。
    let func: Symbol<'_, DbFn> = unsafe { lib.get(symbol.as_bytes()) }
        .map_err(|e| AppError::db(format!("dlsym {symbol} 失败: {e}")))?;

    let request = json!({ "path": db, "sql": sql });
    let request = CString::new(request.to_string())
        .map_err(|_| AppError::param("sql 包含非法 NUL 字符"))?;

    let mut out = vec![0u8; DB_OUT_BUF_SIZE];
    // SAFETY: 入参为 NUL 结尾的 JSON，出参缓冲区长度满足 cyctron 约定。
    let ret = unsafe { func(request.as_ptr(), out.as_mut_ptr() as *mut c_char) };

    if ret != 0 {
        return Err(AppError::db(format!(
            "database {symbol} 返回 {ret}（输出缓冲区不足或指针为空）"
        )));
    }

    let text = unsafe { CStr::from_ptr(out.as_ptr() as *const c_char) }
        .to_str()
        .map_err(|e| AppError::db(format!("database {symbol} 响应非 UTF-8: {e}")))?;

    let value: Value = serde_json::from_str(text)
        .map_err(|e| AppError::db(format!("database {symbol} 响应解析失败: {e}")))?;

    let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 200 {
        let msg = value
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("unknown database error");
        return Err(AppError::db(format!("database {symbol} ({code}): {msg}")));
    }

    Ok(value)
}

fn rows(value: &Value) -> Vec<Value> {
    value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 查询，返回行数组。
pub fn query(db: &str, sql: &str) -> AppResult<Vec<Value>> {
    Ok(rows(&call("db_query", db, sql)?))
}

/// 执行写语句（INSERT / CREATE / ALTER / DROP）。
pub fn execute(db: &str, sql: &str) -> AppResult<()> {
    call("db_insert", db, sql).map(|_| ())
}

/// 执行 UPDATE。
pub fn update(db: &str, sql: &str) -> AppResult<()> {
    call("db_update", db, sql).map(|_| ())
}

/// 执行 DELETE。
pub fn delete(db: &str, sql: &str) -> AppResult<()> {
    call("db_delete", db, sql).map(|_| ())
}

/// SQL 字符串字面量：转义单引号并加引号。
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// 可空字符串：`NULL` 或带引号字面量。
pub fn quote_opt(value: Option<&str>) -> String {
    match value {
        Some(v) => quote(v),
        None => "NULL".to_string(),
    }
}

/// LIKE 模式：转义 `%`、`_`、`\` 与单引号，调用方负责追加 `ESCAPE '\'`。
pub fn like_pattern(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('\'', "''");
    format!("'%{escaped}%'")
}

/// 单值整数查询（如 COUNT(*)）。
pub fn query_i64(db: &str, sql: &str, column: &str) -> AppResult<i64> {
    let rows = query(db, sql)?;
    Ok(rows
        .first()
        .and_then(|r| r.get(column))
        .and_then(Value::as_i64)
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_single_quotes() {
        assert_eq!(quote("a'b"), "'a''b'");
        assert_eq!(quote_opt(None), "NULL");
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("a_b%c"), "'%a\\_b\\%c%'");
    }
}
