//! FFI 冒烟测试：不需要 database.dll，验证参数与响应外壳。

use plugin_lib::get_plugins;
use std::ffi::{CStr, CString, c_char};

const OUT_LEN: usize = 65_536;

fn call(
    func: extern "C" fn(*const c_char, *mut c_char) -> i32,
    input: Option<&str>,
) -> (i32, String) {
    let input = input.map(|text| CString::new(text).unwrap());
    let mut out = vec![0u8; OUT_LEN];

    let ret = func(
        input
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr()),
        out.as_mut_ptr() as *mut c_char,
    );

    let text = unsafe { CStr::from_ptr(out.as_ptr() as *const c_char) }
        .to_string_lossy()
        .to_string();

    (ret, text)
}

#[test]
fn null_input_returns_parameter_error() {
    let (ret, body) = call(get_plugins, None);
    assert_eq!(ret, 0);
    assert!(body.contains("\"code\":400100"), "{body}");
}

#[test]
fn invalid_json_returns_400102() {
    let (ret, body) = call(get_plugins, Some("{not-json"));
    assert_eq!(ret, 0);
    assert!(body.contains("\"code\":400102"), "{body}");
}

#[test]
fn valid_input_is_accepted_before_database_access() {
    // page_size 被规范化；数据库不可用时返回数据库错误而不是参数错误。
    let (ret, body) = call(get_plugins, Some(r#"{"page":0,"page_size":1000}"#));
    assert_eq!(ret, 0);
    assert!(
        body.contains("\"code\":200") || body.contains("\"code\":500201"),
        "{body}"
    );
}
