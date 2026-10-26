// designplugin_lib/setup.ts —— 初始化 plugins.db（幂等）
//
// 两种执行方式：
//   1. deno run -A setup.ts
//   2. update.exe 安装 sys 包后静态提取本文件的 fetch 请求并发送
//
// 为兼容 update 的 oxc 提取器：只使用顶层 await fetch + 字面量参数，
// 不使用函数、循环、条件判断或 PRAGMA 查询。
// 注意：补列 ALTER 在新建库上会因列已存在而返回错误，update 不检查响应体，会继续后续请求。
// 说明：只初始化 plugins.db；info.db 的 setting 表由 setting_ui 负责创建。

const API_URL = "http://127.0.0.1:44944/database";
const DB_PATH = "./plugins";

// ========== 1. 建表 ==========

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugins (
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
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_categories (
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
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_versions (
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
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_configs (
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
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_installs (
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
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_relations (
      id            INTEGER PRIMARY KEY AUTOINCREMENT,
      plugin_uuid   TEXT NOT NULL,
      related_uuid  TEXT NOT NULL,
      relation_type TEXT NOT NULL,
      created_at    INTEGER,
      UNIQUE(plugin_uuid, related_uuid, relation_type)
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_libs (
      id            INTEGER PRIMARY KEY AUTOINCREMENT,
      uuid          TEXT UNIQUE NOT NULL,
      plugin_uuid   TEXT NOT NULL,
      version       TEXT,
      name          TEXT NOT NULL,
      file_name     TEXT NOT NULL,
      platform      TEXT NOT NULL DEFAULT 'windows',
      target        TEXT DEFAULT 'common',
      install_path  TEXT,
      libary_path   TEXT,
      file_hash     TEXT,
      file_size     INTEGER,
      is_local      INTEGER DEFAULT 0,
      is_registered INTEGER DEFAULT 0,
      load_ok       INTEGER DEFAULT -1,
      last_check_at INTEGER,
      created_at    INTEGER,
      updated_at    INTEGER,
      UNIQUE(plugin_uuid, platform, name)
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS plugin_functions (
      id              INTEGER PRIMARY KEY AUTOINCREMENT,
      uuid            TEXT UNIQUE NOT NULL,
      lib_uuid        TEXT NOT NULL,
      lib_name        TEXT NOT NULL,
      symbol          TEXT NOT NULL,
      method          TEXT NOT NULL DEFAULT 'post',
      ffi_symbol      TEXT NOT NULL,
      title           TEXT NOT NULL,
      description     TEXT NOT NULL,
      category        TEXT,
      params_schema   TEXT NOT NULL,
      params_example  TEXT,
      return_schema   TEXT NOT NULL,
      return_example  TEXT,
      error_codes     TEXT,
      side_effects    INTEGER DEFAULT 0,
      exposed_mcp     INTEGER DEFAULT 1,
      is_local        INTEGER DEFAULT 0,
      use_ai          INTEGER DEFAULT 1,
      embed_status    INTEGER DEFAULT 0,
      embed_model     TEXT,
      embed_text_hash TEXT,
      embedded_at     INTEGER,
      source          TEXT,
      source_hash     TEXT,
      created_at      INTEGER,
      updated_at      INTEGER,
      UNIQUE(lib_uuid, symbol)
    )`,
  }),
});

// ========== 2. 补列（旧库；新建库会返回“列已存在”，可忽略） ==========

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugins ADD COLUMN installed INTEGER DEFAULT 0`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugins ADD COLUMN install_path TEXT`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugins ADD COLUMN enabled INTEGER DEFAULT 1`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugins ADD COLUMN plugin_type TEXT DEFAULT 'dll'`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugins ADD COLUMN target TEXT DEFAULT 'common'`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `ALTER TABLE plugin_functions ADD COLUMN use_ai INTEGER DEFAULT 1`,
  }),
});

// ========== 3. 索引 ==========

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugins_category ON plugins(category_id)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugins_target ON plugins(target)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugins_status ON plugins(status)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugins_name ON plugins(name)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_categories_parent ON plugin_categories(parent_uuid)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_versions_latest ON plugin_versions(plugin_uuid, is_latest)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_configs_plugin ON plugin_configs(plugin_uuid, version_id)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_installs_plugin ON plugin_installs(plugin_uuid, status)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_relations_type ON plugin_relations(plugin_uuid, relation_type)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_libs_plugin ON plugin_libs(plugin_uuid)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_libs_name ON plugin_libs(name)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_libs_local ON plugin_libs(is_local, platform)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_functions_lib ON plugin_functions(lib_uuid)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_functions_lookup ON plugin_functions(lib_name, ffi_symbol)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `DROP INDEX IF EXISTS idx_plugin_functions_local`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_functions_local ON plugin_functions(is_local, embed_status, use_ai)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_functions_status ON plugin_functions(embed_status)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_plugin_functions_cat ON plugin_functions(category)`,
  }),
});

// 存量重复数据会让唯一索引创建失败；update 不中断，Deno 下可查看响应或后续再处理。
await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE UNIQUE INDEX IF NOT EXISTS uq_plugin_configs_key ON plugin_configs(plugin_uuid, version_id, param_key)`,
  }),
});


// ========== 4. 技能表（skills / skill_categories） ==========

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS skill_categories (
      code        TEXT PRIMARY KEY NOT NULL,
      name        TEXT NOT NULL DEFAULT '',
      description TEXT NOT NULL DEFAULT '',
      sort        INTEGER NOT NULL DEFAULT 0,
      created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE TABLE IF NOT EXISTS skills (
      id            TEXT PRIMARY KEY NOT NULL,
      name          TEXT NOT NULL DEFAULT '',
      description   TEXT NOT NULL DEFAULT '',
      category      TEXT NOT NULL DEFAULT 'General',
      content       TEXT NOT NULL DEFAULT '',
      trigger       TEXT,
      params_schema TEXT,
      is_system     INTEGER NOT NULL DEFAULT 0,
      is_enabled    INTEGER NOT NULL DEFAULT 1,
      sort          INTEGER NOT NULL DEFAULT 0,
      version       INTEGER NOT NULL DEFAULT 1,
      created_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    )`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_skills_category_sort ON skills(category, sort)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `CREATE INDEX IF NOT EXISTS idx_skills_enabled ON skills(is_enabled)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `INSERT OR IGNORE INTO skill_categories (code, name, sort)
        VALUES ('general', '通用', 10)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `INSERT OR IGNORE INTO skill_categories (code, name, sort)
        VALUES ('design', '设计软件', 20)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `INSERT OR IGNORE INTO skill_categories (code, name, sort)
        VALUES ('material', '素材库', 30)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `INSERT OR IGNORE INTO skill_categories (code, name, sort)
        VALUES ('plugin', '本地插件', 40)`,
  }),
});

await fetch(API_URL, {
  method: "POST",
  headers: { "Content-Type": "application/libary", "FFI-Symbol": "db_update" },
  body: JSON.stringify({
    path: DB_PATH,
    sql: `INSERT OR IGNORE INTO skill_categories (code, name, sort)
        VALUES ('safety', '安全与权限', 90)`,
  }),
});
