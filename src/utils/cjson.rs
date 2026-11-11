//! JSON 输入解析与 FFI 输出，统一 `{code,data,msg}` 外壳。

use crate::utils::error::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::ffi::{CStr, c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// FFI 输出缓冲区大小（cyctron 约定 64 KiB）。
pub const OUT_BUF_SIZE: usize = 65_536;

fn write_json_out(json_out: *mut c_char, value: &Value) -> c_int {
    if json_out.is_null() {
        return -1;
    }

    let text = value.to_string();
    let bytes = text.as_bytes();
    if bytes.len() >= OUT_BUF_SIZE {
        return -2;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), json_out as *mut u8, bytes.len());
        *json_out.add(bytes.len()) = 0;
    }

    0
}

pub fn write_error(json_out: *mut c_char, code: i32, msg: &str) -> c_int {
    write_json_out(
        json_out,
        &json!({ "code": code, "data": {}, "msg": msg }),
    )
}

pub fn write_success(json_out: *mut c_char, data: Value) -> c_int {
    write_json_out(
        json_out,
        &json!({ "code": 200, "data": data, "msg": "success" }),
    )
}

/// 解析 C 字符串 JSON；空指针 / 非 UTF-8 / 非法 JSON 都转成业务错误。
#[allow(clippy::not_unsafe_ptr_arg_deref)] // FFI 入口，指针有效性由调用方（cyctron）保证
pub fn parse_input<T: DeserializeOwned>(json_in: *const c_char) -> AppResult<T> {
    if json_in.is_null() {
        return Err(AppError::new(400100, "json_in is null"));
    }

    let text = unsafe { CStr::from_ptr(json_in) }
        .to_str()
        .map_err(|e| AppError::new(400101, format!("invalid utf-8: {e}")))?;

    if text.trim().is_empty() {
        return Err(AppError::new(400102, "json body is empty"));
    }

    serde_json::from_str(text)
        .map_err(|e| AppError::new(400102, format!("invalid json: {e}")))
}

/// FFI 入口统一包装：解析 → 执行业务 → 写响应；panic 不会穿透 C ABI。
pub fn with_input<T, F>(json_in: *const c_char, json_out: *mut c_char, handler: F) -> c_int
where
    T: DeserializeOwned,
    F: FnOnce(T) -> AppResult<Value>,
{
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let req: T = parse_input(json_in)?;
        handler(req)
    }));

    match outcome {
        Ok(Ok(data)) => write_success(json_out, data),
        Ok(Err(e)) => write_error(json_out, e.code, &e.msg),
        Err(_) => write_error(json_out, 500299, "internal panic"),
    }
}

/// 把 JSON 的 bool / number / string 归一成 0/1。
///
/// 只有 `1`、`true`、`"1"`、`"true"`、`"yes"`、`"on"`（以及数值 `1.0`）视为开启；
/// 其他已提供的值一律为 `0`。字段缺失或显式 `null` 返回 `None`，
/// 便于部分更新区分“未提供”和“显式关闭”。
pub fn flag_value(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    let enabled = match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() == Some(1.0),
        Value::String(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Value::Null => return None,
        _ => false,
    };
    Some(i64::from(enabled))
}

/// 兼容旧调用方的引用参数形式，语义与 [`flag_value`] 一致。
pub fn flag(value: &Option<Value>) -> Option<i64> {
    flag_value(value.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn parse_rejects_null_and_bad_json() {
        assert_eq!(parse_input::<Value>(std::ptr::null()).unwrap_err().code, 400100);

        let bad = CString::new("{oops").unwrap();
        assert_eq!(parse_input::<Value>(bad.as_ptr()).unwrap_err().code, 400102);

        let ok = CString::new("{}").unwrap();
        assert!(parse_input::<Value>(ok.as_ptr()).is_ok());
    }

    #[test]
    fn flag_accepts_bool_number_and_string() {
        assert_eq!(flag(&Some(json!(true))), Some(1));
        assert_eq!(flag(&Some(json!(false))), Some(0));
        assert_eq!(flag(&Some(json!(1))), Some(1));
        assert_eq!(flag(&Some(json!(1.0))), Some(1));
        assert_eq!(flag(&Some(json!(0))), Some(0));
        assert_eq!(flag(&Some(json!(2))), Some(0));
        assert_eq!(flag(&Some(json!(-1))), Some(0));
        assert_eq!(flag(&Some(json!(1.5))), Some(0));
        assert_eq!(flag(&Some(json!("1"))), Some(1));
        assert_eq!(flag(&Some(json!("true"))), Some(1));
        assert_eq!(flag(&Some(json!("yes"))), Some(1));
        assert_eq!(flag(&Some(json!("on"))), Some(1));
        assert_eq!(flag(&Some(json!("false"))), Some(0));
        assert_eq!(flag(&Some(json!("2"))), Some(0));
        assert_eq!(flag(&Some(json!([]))), Some(0));
        assert_eq!(flag(&Some(json!({}))), Some(0));
        assert_eq!(flag(&Some(json!(null))), None);
        assert_eq!(flag(&None), None);
        assert_eq!(flag_value(None), None);
        assert_eq!(flag_value(Some(&json!(null))), None);
    }

    #[test]
    fn success_and_error_fit_the_contract() {
        let mut buf = vec![0u8; OUT_BUF_SIZE];
        let ret = write_success(buf.as_mut_ptr() as *mut c_char, json!({"a": 1}));
        assert_eq!(ret, 0);
        let text = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_str()
            .unwrap()
            .to_string();
        assert!(text.contains("\"code\":200"));
        assert!(text.contains("\"data\":{\"a\":1}"));
    }
}
