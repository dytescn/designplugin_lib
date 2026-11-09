//! `libdesigndb.dll` 的 dlopen 封装：skills、prompts 与 taskflow 对象集。
//!
//! - `skills`：每条文档一个技能，通过 `plugin_uuid` 关联所属插件。
//! - `prompts`：每个 `plugin_uuid` 一条文档，文档的 `prompts` 字段直接是数组。
//! - `taskflow`：整个集合一条文档，`taskflows` 字段直接是数组。
//!
//! 各类对象集的 `path` 与 `collection` 都与名称一致，整个项目各只使用一个。

use crate::utils::error::{AppError, AppResult};
use libloading::Library;
use serde_json::{Value, json};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
const DESIGNDB_LIB: &str = "./libary/libdesigndb.dll";
#[cfg(target_os = "linux")]
const DESIGNDB_LIB: &str = "./libary/libdesigndb.so";
#[cfg(target_os = "macos")]
const DESIGNDB_LIB: &str = "./libary/libdesigndb.dylib";

/// 与 designdb 的 `write_json_out` 保持一致。
const OUT_BUF_SIZE: usize = 65536;

pub const SKILLS_PATH: &str = "skills";
pub const SKILLS_COLLECTION: &str = "skills";
pub const PROMPTS_PATH: &str = "prompts";
pub const PROMPTS_COLLECTION: &str = "prompts";
pub const TASKFLOW_PATH: &str = "taskflow";
pub const TASKFLOW_COLLECTION: &str = "taskflow";
pub const TASKFLOW_ID: &str = "taskflow";

/// designdb 单次响应上限 64 KiB，分页扫描时取 20 条留出余量。
const SCAN_PAGE_SIZE: u64 = 20;

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

fn data_array(value: &Value) -> Vec<Value> {
    value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn query_at(
    path: &str,
    collection: &str,
    filter: Value,
    sort: Option<Value>,
    skip: Option<u64>,
    limit: Option<u64>,
) -> AppResult<Vec<Value>> {
    let mut request = json!({
        "path": path,
        "collection": collection,
        "filter": filter,
    });
    if let Some(sort) = sort {
        request["sort"] = sort;
    }
    if let Some(skip) = skip {
        request["skip"] = json!(skip);
    }
    if let Some(limit) = limit {
        request["limit"] = json!(limit);
    }
    Ok(data_array(&call("post_db_query", request)?))
}

fn query_all_at(path: &str, collection: &str, filter: Value) -> AppResult<Vec<Value>> {
    let mut all = Vec::new();
    let mut skip = 0u64;
    loop {
        let batch = query_at(
            path,
            collection,
            filter.clone(),
            Some(json!({ "_id": 1 })),
            Some(skip),
            Some(SCAN_PAGE_SIZE),
        )?;
        let count = batch.len();
        all.extend(batch);
        if count < SCAN_PAGE_SIZE as usize {
            break;
        }
        skip += SCAN_PAGE_SIZE;
    }
    Ok(all)
}

fn query_one_at(path: &str, collection: &str, filter: Value) -> AppResult<Option<Value>> {
    Ok(query_at(path, collection, filter, None, None, Some(1))?
        .into_iter()
        .next())
}

fn insert_at(path: &str, collection: &str, doc: Value) -> AppResult<()> {
    call(
        "post_db_insert",
        json!({
            "path": path,
            "collection": collection,
            "doc": doc,
        }),
    )?;
    Ok(())
}

fn update_at(path: &str, collection: &str, filter: Value, update: Value) -> AppResult<()> {
    call(
        "post_db_update",
        json!({
            "path": path,
            "collection": collection,
            "filter": filter,
            "update": update,
        }),
    )?;
    Ok(())
}

fn delete_at(path: &str, collection: &str, filter: Value) -> AppResult<()> {
    call(
        "post_db_delete",
        json!({
            "path": path,
            "collection": collection,
            "filter": filter,
        }),
    )?;
    Ok(())
}

/// 按过滤条件查询技能文档；`sort` / `skip` / `limit` 为空时不传给 designdb。
pub fn query(
    filter: Value,
    sort: Option<Value>,
    skip: Option<u64>,
    limit: Option<u64>,
) -> AppResult<Vec<Value>> {
    query_at(SKILLS_PATH, SKILLS_COLLECTION, filter, sort, skip, limit)
}

/// 分页读取全部匹配的技能文档，避免单次响应超过 64 KiB。
pub fn query_all(filter: Value) -> AppResult<Vec<Value>> {
    query_all_at(SKILLS_PATH, SKILLS_COLLECTION, filter)
}

/// 读取第一条匹配的技能文档。
pub fn query_one(filter: Value) -> AppResult<Option<Value>> {
    query_one_at(SKILLS_PATH, SKILLS_COLLECTION, filter)
}

pub fn insert(doc: Value) -> AppResult<()> {
    insert_at(SKILLS_PATH, SKILLS_COLLECTION, doc)
}

pub fn update(filter: Value, update: Value) -> AppResult<()> {
    update_at(SKILLS_PATH, SKILLS_COLLECTION, filter, update)
}

pub fn delete(filter: Value) -> AppResult<()> {
    delete_at(SKILLS_PATH, SKILLS_COLLECTION, filter)
}

/// 读取一条技能文档；不存在时返回 `None`。
pub fn get_skill_doc(id: &str) -> AppResult<Option<Value>> {
    query_one(json!({ "_id": id }))
}

/// 读取某个插件的提示词文档；不存在时返回 `None`。
pub fn get_prompts_doc(plugin_uuid: &str) -> AppResult<Option<Value>> {
    query_one_at(
        PROMPTS_PATH,
        PROMPTS_COLLECTION,
        json!({ "_id": plugin_uuid }),
    )
}

/// 整体写入某个插件的提示词数组：不存在则新增，存在则覆盖。
pub fn set_prompts_doc(plugin_uuid: &str, prompts: Vec<Value>) -> AppResult<()> {
    let timestamp = now();
    let existing = get_prompts_doc(plugin_uuid)?;

    match existing {
        Some(_) => update_at(
            PROMPTS_PATH,
            PROMPTS_COLLECTION,
            json!({ "_id": plugin_uuid }),
            json!({ "$set": { "prompts": prompts, "updated_at": timestamp } }),
        ),
        None => insert_at(
            PROMPTS_PATH,
            PROMPTS_COLLECTION,
            json!({
                "_id": plugin_uuid,
                "plugin_uuid": plugin_uuid,
                "prompts": prompts,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ),
    }
}

/// 读取任务流文档；不存在时返回 `None`。
pub fn get_taskflow_doc() -> AppResult<Option<Value>> {
    query_one_at(
        TASKFLOW_PATH,
        TASKFLOW_COLLECTION,
        json!({ "_id": TASKFLOW_ID }),
    )
}

/// 整体写入任务流数组：不存在则新增，存在则覆盖。
pub fn set_taskflow_doc(taskflows: Vec<Value>) -> AppResult<()> {
    let timestamp = now();
    match get_taskflow_doc()? {
        Some(_) => update_at(
            TASKFLOW_PATH,
            TASKFLOW_COLLECTION,
            json!({ "_id": TASKFLOW_ID }),
            json!({ "$set": { "taskflows": taskflows, "updated_at": timestamp } }),
        ),
        None => insert_at(
            TASKFLOW_PATH,
            TASKFLOW_COLLECTION,
            json!({
                "_id": TASKFLOW_ID,
                "taskflows": taskflows,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ),
    }
}
