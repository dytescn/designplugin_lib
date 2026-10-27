//! `libdesigndb.dll` 的 dlopen 封装：保存技能完整内容。
//!
//! 技能索引 / 基础信息在 `plugins.db.skills`，完整 `skills.json` 的每一条技能
//! 以 `_id = skill.id` 写入 `./designdb/skills` 的 `skills` 集合，供回读与迁移。
//! 该库缺失或写入失败时不阻断 scan / install（调用方按 best-effort 处理）。

use crate::utils::error::{AppError, AppResult};
use libloading::Library;
use serde_json::{Value, json};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;

#[cfg(windows)]
const DESIGNDB_LIB: &str = "./libary/libdesigndb.dll";
#[cfg(not(windows))]
const DESIGNDB_LIB: &str = "./libary/libdesigndb.so";

const OUT_BUF_SIZE: usize = 65536;
const PATH: &str = "skills";
const COLLECTION: &str = "skills";

type FfiFn = unsafe extern "C" fn(*const c_char, *mut c_char) -> c_int;

static LIB: OnceLock<Result<Library, String>> = OnceLock::new();

fn library() -> AppResult<&'static Library> {
    let cell = LIB.get_or_init(|| {
        // SAFETY: libdesigndb 由本应用安装，加载与符号调用在同一进程内完成。
        unsafe { Library::new(DESIGNDB_LIB) }
            .map_err(|error| format!("加载 {DESIGNDB_LIB} 失败: {error}"))
    });
    match cell {
        Ok(library) => Ok(library),
        Err(error) => Err(AppError::db(error.clone())),
    }
}

fn call(symbol: &str, request: Value) -> AppResult<Value> {
    let library = library()?;
    let function: libloading::Symbol<'_, FfiFn> = unsafe { library.get(symbol.as_bytes()) }
        .map_err(|error| AppError::db(format!("dlsym {symbol} 失败: {error}")))?;

    let request = CString::new(request.to_string())
        .map_err(|_| AppError::param("designdb 请求 JSON 含 NUL"))?;
    let mut output = vec![0u8; OUT_BUF_SIZE];
    let ret = unsafe { function(request.as_ptr(), output.as_mut_ptr() as *mut c_char) };

    let text = unsafe { CStr::from_ptr(output.as_ptr() as *const c_char) }
        .to_str()
        .map_err(|_| AppError::db("designdb 响应非 UTF-8"))?;
    if text.is_empty() {
        return Err(AppError::db(format!(
            "designdb {symbol} 返回空（ret={ret}）"
        )));
    }

    let value: Value = serde_json::from_str(text)
        .map_err(|error| AppError::db(format!("designdb {symbol} 响应解析失败: {error}")))?;
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 200 && code != 0 {
        return Err(AppError::db(format!(
            "designdb {symbol} 失败 ({code}): {}",
            value
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )));
    }
    Ok(value)
}

/// 写入 / 覆盖一条技能完整内容；`_id` 使用技能 id。
pub fn upsert_skill(skill: &Value) -> AppResult<()> {
    let id = skill
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() {
        return Err(AppError::param("技能缺少 id"));
    }

    let doc = json!({
        "_id": id,
        "id": id,
        "name": skill.get("name").cloned().unwrap_or(Value::Null),
        "description": skill.get("description").cloned().unwrap_or(Value::Null),
        "category": skill.get("category").cloned().unwrap_or(Value::Null),
        "content": skill.get("content").cloned().unwrap_or(Value::Null),
        "trigger": skill.get("trigger").cloned().unwrap_or(Value::Null),
        "params_schema": skill.get("params_schema").cloned().unwrap_or(Value::Null),
        "is_system": skill.get("is_system").cloned().unwrap_or(json!(0)),
        "is_enabled": skill.get("is_enabled").cloned().unwrap_or(json!(1)),
        "sort": skill.get("sort").cloned().unwrap_or(json!(0)),
        "version": skill.get("version").cloned().unwrap_or(json!(1)),
    });

    // 先删后插，避免依赖 update 的局部更新语义。
    let _ = call(
        "post_db_delete",
        json!({ "path": PATH, "collection": COLLECTION, "filter": { "_id": id } }),
    );
    call(
        "post_db_insert",
        json!({ "path": PATH, "collection": COLLECTION, "doc": doc }),
    )?;
    Ok(())
}

/// 读取一条技能完整内容；不存在时返回 `None`。
pub fn get_skill_doc(id: &str) -> AppResult<Option<Value>> {
    let value = call(
        "post_db_query",
        json!({
            "path": PATH,
            "collection": COLLECTION,
            "filter": { "_id": id },
            "limit": 1,
        }),
    )?;
    Ok(value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .cloned())
}
