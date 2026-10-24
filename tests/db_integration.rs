//! 真实数据库集成测试（需要 `database.dll`）。
//!
//! 未设置 `PLUGIN_LIB_DATABASE` 时自动跳过，避免 CI 依赖本机路径：
//!
//! ```bash
//! PLUGIN_LIB_DATABASE='D:\codes\github\funxdata\database\target\release\database.dll' \
//!   cargo test --test db_integration -- --nocapture
//! ```
//!
//! 正式建表由 `setup.ts` 负责；测试只创建用到的表，避免依赖 HTTP 服务。
//! `info.setting` 由 `setting_ui` 创建，这里按同样的结构模拟。

use plugin_lib::utils::db;
use plugin_lib::{
    get_configs, get_plugin, get_plugins, patch_config, patch_install, patch_plugin, post_category,
    post_config, post_plugin, post_version,
};
use serde_json::Value;
use std::ffi::{CStr, CString, c_char};

const OUT_LEN: usize = 65_536;

const PLUGINS_SCHEMA: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS plugins (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid         TEXT UNIQUE NOT NULL,
        ouid         TEXT,
        flow_uid     TEXT,
        type_uid     TEXT,
        category_id  TEXT,
        name         TEXT,
        "desc"       TEXT,
        create_by    TEXT,
        cover        TEXT,
        status       TEXT DEFAULT 'draft',
        public       INTEGER DEFAULT 0,
        updated_by   TEXT,
        created_at   INTEGER,
        updated_at   INTEGER,
        deleted_at   INTEGER,
        installed    INTEGER DEFAULT 0,
        install_path TEXT,
        enabled      INTEGER DEFAULT 1,
        plugin_type  TEXT DEFAULT 'dll',
        target       TEXT DEFAULT 'common'
    )"#,
    r#"CREATE TABLE IF NOT EXISTS plugin_categories (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid         TEXT UNIQUE NOT NULL,
        type_name    TEXT UNIQUE,
        icon         TEXT,
        parent_uuid  TEXT,
        sort_order   INTEGER DEFAULT 0,
        created_by   TEXT,
        updated_by   TEXT,
        created_at   INTEGER,
        updated_at   INTEGER,
        deleted_at   INTEGER
    )"#,
    r#"CREATE TABLE IF NOT EXISTS plugin_versions (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        plugin_uuid    TEXT,
        version        TEXT,
        changelog      TEXT,
        file_path      TEXT,
        file_size      INTEGER,
        file_hash      TEXT,
        file_type      TEXT DEFAULT 'zip',
        min_engine_ver TEXT,
        max_engine_ver TEXT,
        status         TEXT DEFAULT 'active',
        is_latest      INTEGER DEFAULT 0,
        created_by     TEXT,
        created_at     INTEGER,
        updated_at     INTEGER,
        deleted_at     INTEGER,
        UNIQUE(plugin_uuid, version)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS plugin_configs (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        plugin_uuid   TEXT,
        version_id    INTEGER DEFAULT 0,
        param_key     TEXT,
        param_type    TEXT,
        default_value TEXT,
        required      INTEGER DEFAULT 0,
        description   TEXT,
        options       TEXT,
        sort_order    INTEGER DEFAULT 0,
        created_at    INTEGER,
        updated_at    INTEGER,
        UNIQUE(plugin_uuid, version_id, param_key)
    )"#,
    r#"CREATE TABLE IF NOT EXISTS plugin_installs (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        plugin_uuid  TEXT NOT NULL,
        version      TEXT NOT NULL,
        install_path TEXT NOT NULL,
        file_hash    TEXT,
        enabled      INTEGER DEFAULT 1,
        status       TEXT DEFAULT 'installed',
        started_at   INTEGER,
        last_error   TEXT,
        created_at   INTEGER,
        updated_at   INTEGER,
        UNIQUE(plugin_uuid, version)
    )"#,
];

const SETTING_SCHEMA: &str = r#"CREATE TABLE IF NOT EXISTS setting (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    key         TEXT NOT NULL UNIQUE,
    title       TEXT,
    value       TEXT,
    value_type  TEXT DEFAULT 'string',
    category    TEXT DEFAULT 'general',
    description TEXT,
    created_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)"#;

fn setup_schema() {
    for sql in PLUGINS_SCHEMA {
        db::execute("./plugins", sql).unwrap();
    }
    db::execute("./info", SETTING_SCHEMA).unwrap();
}

fn call(func: extern "C" fn(*const c_char, *mut c_char) -> i32, body: &str) -> Value {
    let input = CString::new(body).unwrap();
    let mut out = vec![0u8; OUT_LEN];

    let ret = func(input.as_ptr(), out.as_mut_ptr() as *mut c_char);
    assert_eq!(ret, 0, "FFI return code");

    let text = unsafe { CStr::from_ptr(out.as_ptr() as *const c_char) }
        .to_string_lossy()
        .to_string();

    serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid response: {e}: {text}"))
}

fn code(value: &Value) -> i64 {
    value.get("code").and_then(Value::as_i64).unwrap_or(-1)
}

fn data(value: &Value) -> &Value {
    value.get("data").unwrap_or(&Value::Null)
}

#[test]
fn crud_roundtrip() {
    if std::env::var("PLUGIN_LIB_DATABASE")
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        eprintln!("skip: PLUGIN_LIB_DATABASE not set");
        return;
    }

    // database.dll 用相对 cwd 的 ./database，切到独立临时目录避免污染仓库。
    let original = std::env::current_dir().unwrap();
    let workdir = std::env::temp_dir().join(format!("plugin_lib_it_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workdir);
    std::fs::create_dir_all(&workdir).unwrap();
    std::env::set_current_dir(&workdir).unwrap();

    setup_schema();

    // 1. 分类 + 插件
    let category = call(post_category, r#"{"type_name":"单元测试分类"}"#);
    assert_eq!(code(&category), 200, "{category}");
    let category_uuid = data(&category)["uuid"].as_str().unwrap().to_string();

    let plugin = call(
        post_plugin,
        &format!(
            r#"{{"name":"cdrimex","category_id":"{category_uuid}","status":"published","target":"coreldraw","desc":"CorelDRAW 导出插件"}}"#
        ),
    );
    assert_eq!(code(&plugin), 200, "{plugin}");
    let plugin_uuid = data(&plugin)["uuid"].as_str().unwrap().to_string();

    // 2. 列表（含安装状态合并字段）
    let list = call(get_plugins, r#"{"page":1,"page_size":10}"#);
    assert_eq!(code(&list), 200, "{list}");
    assert_eq!(data(&list)["total"].as_i64(), Some(1));
    assert_eq!(data(&list)["list"][0]["name"].as_str(), Some("cdrimex"));
    assert!(data(&list)["list"][0].get("install").is_some());

    // 3. 版本
    let version = call(
        post_version,
        &format!(
            r#"{{"plugin_uuid":"{plugin_uuid}","version":"1.0.0","file_path":"cdrimex.zip","file_hash":"abc","is_latest":true}}"#
        ),
    );
    assert_eq!(code(&version), 200, "{version}");
    assert!(data(&version)["id"].as_i64().unwrap() > 0);

    let detail = call(get_plugin, &format!(r#"{{"uuid":"{plugin_uuid}"}}"#));
    assert_eq!(code(&detail), 200, "{detail}");
    assert_eq!(data(&detail)["versions"].as_array().unwrap().len(), 1);

    // 4. 配置：默认值 → 用户值
    let config = call(
        post_config,
        &format!(
            r#"{{"plugin_uuid":"{plugin_uuid}","param_key":"output_dir","param_type":"string","default_value":"./out","description":"导出目录"}}"#
        ),
    );
    assert_eq!(code(&config), 200, "{config}");

    let configs = call(
        get_configs,
        &format!(r#"{{"plugin_uuid":"{plugin_uuid}"}}"#),
    );
    assert_eq!(code(&configs), 200, "{configs}");
    assert_eq!(
        data(&configs)["configs"][0]["value"].as_str(),
        Some("./out")
    );
    assert_eq!(
        data(&configs)["configs"][0]["source"].as_str(),
        Some("default")
    );

    let saved = call(
        patch_config,
        &format!(r#"{{"plugin_uuid":"{plugin_uuid}","key":"output_dir","value":"D:/out"}}"#),
    );
    assert_eq!(code(&saved), 200, "{saved}");

    let configs = call(
        get_configs,
        &format!(r#"{{"plugin_uuid":"{plugin_uuid}"}}"#),
    );
    assert_eq!(
        data(&configs)["configs"][0]["value"].as_str(),
        Some("D:/out")
    );
    assert_eq!(
        data(&configs)["configs"][0]["source"].as_str(),
        Some("user")
    );

    // 5. 部分更新
    let disabled = call(
        patch_plugin,
        &format!(r#"{{"uuid":"{plugin_uuid}","enabled":false,"status":"offline"}}"#),
    );
    assert_eq!(code(&disabled), 200, "{disabled}");

    let detail = call(get_plugin, &format!(r#"{{"uuid":"{plugin_uuid}"}}"#));
    assert_eq!(data(&detail)["plugin"]["enabled"].as_i64(), Some(0));
    assert_eq!(data(&detail)["plugin"]["status"].as_str(), Some("offline"));

    // 6. 没有安装记录时 patch_install 返回 400404
    let missing = call(
        patch_install,
        &format!(r#"{{"plugin_uuid":"{plugin_uuid}"}}"#),
    );
    assert_eq!(code(&missing), 400404, "{missing}");

    std::env::set_current_dir(original).unwrap();
    let _ = std::fs::remove_dir_all(&workdir);
}
