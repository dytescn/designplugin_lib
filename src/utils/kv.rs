//! `kv_lib` 的 dlopen 封装：把插件资源导入 chat_think / chat_reply / chat_plugin。
//!
//! plugin_lib 只通过 FFI 调 kv_lib，不直接读写 kv 的 SQLite 分片。
//! kv_lib 缺失时导入为 best-effort，不阻断插件安装。

use crate::utils::error::{AppError, AppResult};
use libloading::Library;
use serde_json::{Value, json};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::OnceLock;

#[cfg(windows)]
const KV_LIB: &str = "./libary/kv_lib.dll";
#[cfg(target_os = "linux")]
const KV_LIB: &str = "./libary/kv_lib.so";
#[cfg(target_os = "macos")]
const KV_LIB: &str = "./libary/kv_lib.dylib";

const OUT_BUF_SIZE: usize = 65536;
const BATCH: usize = 200;
/// kv_lib 分页上限为 100；超出会返回 400102。
const LIST_PAGE_SIZE: usize = 100;

type FfiFn = unsafe extern "C" fn(*const c_char, *mut c_char) -> c_int;

static LIB: OnceLock<Result<Library, String>> = OnceLock::new();

/// kv_lib 是否已安装（只检查文件，不加载）。
pub fn available() -> bool {
    Path::new(KV_LIB).is_file()
}

fn library() -> AppResult<&'static Library> {
    let cell = LIB.get_or_init(|| {
        // SAFETY: kv_lib 由本应用安装，加载与符号调用在同一进程内完成。
        unsafe { Library::new(KV_LIB) }.map_err(|error| format!("加载 {KV_LIB} 失败: {error}"))
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
        .map_err(|_| AppError::param("kv_lib 请求 JSON 含 NUL"))?;
    let mut output = vec![0u8; OUT_BUF_SIZE];
    let ret = unsafe { function(request.as_ptr(), output.as_mut_ptr() as *mut c_char) };

    let text = unsafe { CStr::from_ptr(output.as_ptr() as *const c_char) }
        .to_str()
        .map_err(|_| AppError::db("kv_lib 响应非 UTF-8"))?;
    if text.is_empty() {
        return Err(AppError::db(format!("kv_lib {symbol} 返回空（ret={ret}）")));
    }

    let value: Value = serde_json::from_str(text)
        .map_err(|error| AppError::db(format!("kv_lib {symbol} 响应解析失败: {error}")))?;
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 200 && code != 0 {
        return Err(AppError::db(format!(
            "kv_lib {symbol} 失败 ({code}): {}",
            value
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )));
    }
    Ok(value.get("data").cloned().unwrap_or(Value::Null))
}

/// 创建或更新集合；已存在时只更新传入字段。
pub fn ensure_collection(code: &str, name: &str, description: &str, sort: i64) -> AppResult<()> {
    call(
        "post_kv_collection",
        json!({
            "action": "put",
            "code": code,
            "name": name,
            "description": description,
            "dim": 1024,
            "model": "qwen3-embed",
            "shard_max_mb": 16,
            "index_min_rows": 0,
            "state": 0,
            "sort": sort,
        }),
    )?;
    Ok(())
}

fn list_request(collection: &str, prefix: &str, page: usize) -> Value {
    json!({
        "action": "list",
        "collection": collection,
        "prefix": prefix,
        "page": page,
        "page_size": LIST_PAGE_SIZE,
        "with_value": false,
    })
}

fn list_keys(collection: &str, prefix: &str) -> AppResult<Vec<String>> {
    let mut keys = Vec::new();
    let mut page = 1usize;
    loop {
        let data = call("post_kv_record", list_request(collection, prefix, page))?;
        let list = data
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for item in &list {
            if let Some(key) = item.get("key").and_then(Value::as_str) {
                keys.push(key.to_string());
            }
        }
        let total = data.get("total").and_then(Value::as_i64).unwrap_or(0) as usize;
        if list.is_empty() || keys.len() >= total {
            break;
        }
        page += 1;
    }
    Ok(keys)
}

fn delete_keys(collection: &str, keys: &[String]) -> AppResult<()> {
    for chunk in keys.chunks(BATCH) {
        call(
            "post_kv_record",
            json!({
                "action": "delete",
                "collection": collection,
                "keys": chunk,
            }),
        )?;
    }
    Ok(())
}

fn put_items(collection: &str, items: &[Value]) -> AppResult<()> {
    for chunk in items.chunks(BATCH) {
        let data = call(
            "post_kv_record",
            json!({
                "action": "put",
                "collection": collection,
                "items": chunk,
            }),
        )?;
        let failed = data.get("failed").and_then(Value::as_i64).unwrap_or(0);
        if failed > 0 {
            return Err(AppError::db(format!(
                "kv_lib 写入 {collection} 失败 {failed} 条: {}",
                data.get("errors").cloned().unwrap_or(Value::Null)
            )));
        }
    }
    Ok(())
}

/// 用 `items` 替换 `prefix` 下的全部记录，保证重扫幂等。
pub fn replace_records(collection: &str, prefix: &str, items: &[Value]) -> AppResult<usize> {
    let old = list_keys(collection, prefix)?;
    if !old.is_empty() {
        delete_keys(collection, &old)?;
    }
    put_items(collection, items)?;
    Ok(items.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_request_uses_kv_lib_page_limit() {
        let request = list_request("chat_think", "prefix:", 2);
        assert_eq!(request["page"], 2);
        assert_eq!(request["page_size"], 100);
        assert!(request["page_size"].as_u64().unwrap() <= 100);
    }
}
