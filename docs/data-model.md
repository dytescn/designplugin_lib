# 数据模型与存储规范

本文说明 `plugin_lib` 的数据落在哪里、字段如何解释，以及写入时必须保持的幂等与清理规则。
表结构以 `setup.json` 为准，行为以 `src/utils/*.rs` 为准。

## 1. 存储拓扑

```text
调用方（plugin_ui / 测试脚本）
        │  FFI: JSON in / JSON out
        ▼
plugin_lib (cdylib)
        ├── dlopen ./libary/database.dll     → ./database/plugins.db（SQLite）
        ├── dlopen ./libary/libdesigndb.dll  → skills / prompts / taskflow 对象集
        └── dlopen ./libary/kv_lib.dll       → 提示词检索记录（可选）
```

- 所有依赖库路径相对**当前工作目录**；cwd 错误会导致 `500201`。
- `plugin_lib` 不直接链接 SQLite，也不直接管理设计软件的启动与运行状态。
- 三类存储的职责不同：SQLite 存插件、能力与版本；designdb 存文档型内容；kv 存可检索的提示词副本。

## 2. SQLite：`plugins.db`

数据库由 `database` 服务管理，首次使用前必须执行 `setup.json` 初始化（`dytes install sys plugin_lib`
或经 cyctron 执行），否则读写会返回 `500201`（`no such table`）。`setup.json` 是幂等的，
可以重复执行以补列 / 建索引。

### 2.1 `plugins` — 插件主表

| 字段 | 类型 | 说明 | 由谁写入 |
| --- | --- | --- | --- |
| `uuid` | TEXT PK | 插件 UUID | `post_plugins` / 安装 |
| `ouid` | TEXT | 外部 / 组织 UUID | `post_plugins` / `update_plugins` |
| `plugins_type` | TEXT | 插件分类，影响 kv 同步 | `post_plugins` / `update_plugins` / 安装 |
| `version` | TEXT | 插件版本 | `post_plugins` / `update_plugins` / 安装 |
| `name` | TEXT | 插件名 | 同上 |
| `"desc"` | TEXT | 描述；SQL 中必须加双引号 | 同上 |
| `create_by` | TEXT | 创建者 | `post_plugins` / `update_plugins` |
| `cover` | TEXT | 封面路径 / URL | `post_plugins` / `update_plugins` / `patch_plugins` |
| `status` | TEXT | 发布状态，默认 `draft` | `post_plugins` / `patch_plugins` / `update_plugins` |
| `public` | INTEGER | 是否公开（0/1） | `post_plugins` / `update_plugins` |
| `updated_by` | TEXT | 更新者 | `post_plugins` / `update_plugins` |
| `created_at` | INTEGER | 创建时间（Unix 秒） | 服务端 |
| `updated_at` | INTEGER | 更新时间（Unix 秒） | 服务端 |
| `deleted_at` | INTEGER | 软删除标记；查询时过滤，但删除接口是物理删除 | 当前代码不写入 |
| `installed` | INTEGER | 是否已安装（0/1） | 安装流程置 `1` |
| `install_path` | TEXT | 安装目录，安装时记录请求的 `dir` 原样 | 安装流程 |
| `enabled` | INTEGER | 是否启用（0/1），默认 `1` | `post_plugins` / 安装 / `patch_plugins` |
| `target` | TEXT | 目标软件，默认 `common` | `post_plugins` / `update_plugins` / 安装 |
| `platform` | TEXT | 运行平台代码：`win` / `mac` / `linux` | `post_plugins` / `update_plugins` / 安装 |
| `use_ai` | INTEGER | 是否参与 AI 召回（0/1） | 安装流程 |
| `mask` | INTEGER | 权限位 | `post_plugins` / `update_plugins` / `patch_plugins` |

索引：`idx_plugins_target(target)`、`idx_plugins_status(status)`、`idx_plugins_name(name)`。

写入要点：

- 安装时新行固定 `installed = 1`、`enabled = 1`、`mask = 0`、`status = 'draft'`；
  再次安装只更新 `plugins_type`、`version`、`name`、`desc`、`target`、`platform`、
  `install_path`、`installed`、`use_ai`、`updated_at`，保留 `created_at`、`status`、`enabled`、`mask`。
- `status` 取值：`draft`、`review`、`published`、`offline`、`deprecated`。
- 查询接口过滤 `deleted_at IS NULL`；`delete_plugins` 直接 `DELETE`，不会写 `deleted_at`。
- `install_path` 不做路径规范化，写入什么就是什么。

### 2.2 `plugin_functions` — 能力表

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uuid` | TEXT PK | 能力主键；安装时缺省为 `{plugin_uuid}:{name}` |
| `plugin_uuid` | TEXT | 所属插件 |
| `ffi_symbol` | TEXT | 实际导出符号 |
| `title` | TEXT | 能力标题 |
| `description` | TEXT | 能力说明 |
| `params_schema` | TEXT | 入参 JSON Schema（数据库中是序列化字符串） |
| `params_example` | TEXT | 入参示例，可空 |
| `return_schema` | TEXT | 返回 JSON Schema（序列化字符串） |
| `return_example` | TEXT | 返回示例，可空 |
| `use_ai` | INTEGER | 是否参与 AI 召回（0/1） |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |

索引：`idx_plugin_functions_lookup(ffi_symbol)`。

写入要点：

- 四个 JSON 字段写入前用 `serde_json` 序列化为字符串；读取时自动反序列化。
- 安装 / 更新的 `mcp.json` 缺省 `params_schema` / `return_schema` 为 `{"type":"object"}`；
  单条 `post_function` 缺省为 `{}`。
- `post_function` 只检查 `uuid` 是否重复；安装流程按 `uuid` upsert。
- `update_plugin` 的 `prune = true` 会删除该插件下、新 `tools` 中不存在的能力。

### 2.3 `plugin_versions` — 版本记录

| 字段 | 说明 |
| --- | --- |
| `id` | 自增主键 |
| `plugin_uuid` | 所属插件 |
| `version` | 版本号 |
| `status` | 安装流程写 `active` |
| `is_latest` | 安装 / 更新时先全部置 `0`，再为本次版本置 `1` |
| `created_at` / `updated_at` | Unix 秒 |
| `changelog` / `file_path` / `file_size` / `file_hash` / `file_type` / `min_engine_ver` / `max_engine_ver` | 当前安装流程不写，保持 NULL 或默认值 |

约束：`UNIQUE(plugin_uuid, version)`；索引 `idx_plugin_versions_latest(plugin_uuid, is_latest)`。

- 每次成功安装 / 更新都会插入一条记录，并重置同插件的 `is_latest`。
- 版本不高于已安装版本时流程直接 `skipped`，不会新增版本记录。
- 同一版本重复写入会触发唯一约束；正常流程由版本比较提前拦截。

### 2.4 已建表但当前未由 FFI 读写

`setup.json` 还创建了以下表，但当前 Rust 实现没有对应导出接口。不要假设它们会自动维护：

| 表 | 用途（表结构语义） |
| --- | --- |
| `plugin_categories` | 插件分类树 |
| `plugin_configs` | 插件配置定义 |
| `plugin_installs` | 安装记录 / 错误状态 |
| `plugin_relations` | 插件关联关系 |
| `plugin_libs` | 动态库文件登记 |
| `skill_categories` | 技能分类字典（setup 预置 `general`、`design`、`material`、`plugin`、`safety`） |

需要这些数据时，先确认是否有其他模块负责写入，或由人工评估是否新增 FFI；
不要绕过接口直接改表。

## 3. designdb 对象集

`libdesigndb` 提供对象文档存储。本项目固定使用三组 `path` / `collection`：

| 逻辑对象 | `path` | `collection` | 文档粒度 |
| --- | --- | --- | --- |
| 技能 | `skills` | `skills` | 每个技能一条 |
| 提示词 | `prompts` | `prompts` | 每个插件一条 |
| 任务流 | `taskflow` | `taskflow` | 整个集合一条 |

### 3.1 `skills` 文档

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` / `id` | TEXT | 技能 ID，全局唯一；安装时取文件名 |
| `plugin_uuid` | TEXT | 所属插件 |
| `name` | TEXT | 技能名；安装时取首个 `# ` 标题 |
| `description` | TEXT | 描述；安装时为空 |
| `category` | TEXT | 分类 code；安装时固定 `plugin` |
| `content` | TEXT | 技能正文；安装时为文件全文 |
| `trigger` | TEXT | 触发条件；安装时为空 |
| `params_schema` | OBJECT | 入参 JSON Schema；安装时为 `{}` |
| `is_system` | INTEGER | 是否内置（0/1） |
| `is_enabled` | INTEGER | 是否启用（0/1） |
| `sort` | INTEGER | 升序排序；安装时为文件序号 × 10 |
| `version` | INTEGER | 内容版本；新插入为 1，更新时 +1 |
| `created_at` / `updated_at` | INTEGER | Unix 秒 |

- `_id` 是全局主键，不同插件不能使用相同技能 ID。
- 安装写入与 FFI 写入共用该集合；`get_skills` 只返回摘要，`get_skill` 返回完整文档（含 `content`）。
- 更新时只有 `content`、`name`、`category` 一项与包内不同才递增 `version`。

### 3.2 `prompts` 文档

| 字段 | 说明 |
| --- | --- |
| `_id` | 等于 `plugin_uuid` |
| `plugin_uuid` | 所属插件 |
| `prompts` | 提示词数组，元素结构由调用方定义 |
| `created_at` | 首次创建时间；后续覆盖不改 |
| `updated_at` | 每次整体覆盖时刷新 |

- 不存在文档时，`get_prompts` 返回空数组。
- `update_prompts` 与安装流程都是整体覆盖该数组，不提供单条操作。
- 安装流程只写 designdb 与可选的 kv，不修改其他插件的文档。

### 3.3 `taskflow` 文档

| 字段 | 说明 |
| --- | --- |
| `_id` | 固定 `taskflow` |
| `taskflows` | 所有插件共享的任务流数组 |
| `created_at` / `updated_at` | 首次创建 / 最近写入时间 |

- 集合中只有一条文档。
- 安装 / 更新按元素 `uid`（其次 `name`）合并，并给元素补 `plugin_uuid`。
- FFI `update_taskflow` 是整体覆盖，会替换所有插件的元素；不要在单插件场景使用。

### 3.4 读取约束

- `plugin_lib` 当前按 64 KiB 缓冲区读取 designdb；designdb 1.0.3 自身的响应上限为
  16 MiB。库间缓冲区目标与迁移计划见 [skills/ffi-buffer.md](../skills/ffi-buffer.md)
  （目标 1024 KiB，尚未迁移）。
- 内部扫描按 `_id` 升序、每页 20 条读取。
- `get_skills` 对外只返回摘要，不返回 `content`；内部仍会先读取匹配文档再排序、分页，
  数据量大时应缩小查询范围。需要内容时用 `get_skill` 按 `id` 单条读取。
- 排序规则：`is_system` 降序 → `category` 升序 → `sort` 升序 → `_id` 升序。

## 4. kv 提示词同步

仅在包安装 / 更新流程中、且 `kv_lib` 可用时执行。

| `plugins_type` | kv 集合 | 集合说明 |
| --- | --- | --- |
| `chat_think` | `chat_think` | 聊天 think 阶段 |
| `chat_reply` | `chat_reply` | 聊天 reply 阶段 |
| `chat_plugins` | `chat_plugins` | 聊天插件能力描述 |
| `sys_plugins` | `sys_plugins` | 系统插件能力描述 |
| `plugins` / `plugins_coreldraw` | `chat_plugin` | 兼容旧清单 |
| 其他 / 空 | — | 不同步；类型切换时清理其他集合 |

- 记录 key：`{plugin_uuid}:prompt:{标识}`，标识优先 `id`、其次 `name`、最后 `content` 的
  SHA-256 前 16 位；标识中的 `:` `/` `\` 替换为 `_`。
- 记录 `value` 与 `text` 都取 `content`；`meta` 包含 `plugin_uuid`、`plugin_name`、
  `kind = "prompt"`；`rank` 为数组序号 + 1。写入使用 `force: false`：
  内容变化时 `kv_lib` 依据 `text_hash` 重新向量化，仅 `rank` / `meta` 变化时不会重新嵌入。
- 同步只处理 `{plugin_uuid}:prompt:` 前缀：
  - 内容、`rank`、`meta` 都未变：跳过；
  - 内容变化：写入并重新向量化；
  - 仅 `rank` / `meta` 变化：更新记录，不重新向量化；
  - 新增：写入；
  - 新数组中已不存在：删除对应旧 key。
- `prompts.json` 是空数组时，删除该插件在目标集合中的全部记录，与 designdb 的
  “整体覆盖为空”保持一致。
- 类型切换时，安装 / 更新会先读取旧的 `plugins_type`，扫描已知 kv 集合（排除新集合）
  并清理该插件的记录；新包没有 `prompts.json` 时，会用 designdb 中已有的提示词迁移到新集合；
  旧类型为空或未知时同样扫描已知集合，避免删除后重装或手工改类型留下旧记录。
- `kv_lib` 不可用时跳过，不影响 designdb 写入；kv 写入失败会返回错误，可重试。

## 5. 写入规则与幂等

### 5.1 主键与 upsert

| 数据 | 主键 | 安装 / 更新行为 |
| --- | --- | --- |
| 插件 | `plugins.uuid` | 存在则更新，不存在则插入 |
| 版本 | `(plugin_uuid, version)` | 每次成功安装插入，`is_latest` 重置 |
| 能力 | `plugin_functions.uuid` | 存在则全量更新，否则插入 |
| 技能 | `skills._id` | 存在则按内容差异更新，否则插入 |
| 提示词 | `prompts._id = plugin_uuid` | 整体覆盖（或首次插入） |
| 任务流 | `taskflow._id = "taskflow"` | 合并后整体写回 |

### 5.2 时间与版本

- 所有时间戳为 Unix 秒，由服务端生成。
- `created_at` 在更新时保留；`updated_at` 每次实际写入时刷新。
- 技能 `version` 仅在内容实际变化时 +1；FFI `update_skills` / `patch_skills` 每次调用都 +1。
- 插件版本比较规则见 [plugin-package.md](./plugin-package.md#93-版本比较)。

### 5.3 清理范围

| 操作 | 清理对象 |
| --- | --- |
| `update_plugin`，`prune = true`（默认） | 当前插件的技能、任务流元素、能力 |
| `update_plugin`，`prune = false` | 不清理 |
| `update_plugin`，新包无 `skills/` | 视为无技能，默认清理该插件全部技能 |
| `update_plugin`，新包无 `prompts.json` | 保留原提示词；若 `plugins_type` 变化，迁移到新 kv 集合 |
| `update_plugin`，新包无 `taskflow.json` | 保留原任务流元素 |
| `update_plugin`，新包无 `mcp.json` | 保留原能力 |
| `delete_plugins` | 物理删除插件行与能力；不清理技能、提示词、任务流、版本记录 |
| `delete_skills` | 按 `id` 删除单条技能，不校验插件归属 |

发布层面还要求：`chat_think` / `chat_reply` 包必须带 `skills/`，`chat_plugins` 包必须带
`taskflow.json`，`chat_*` 的 `mcp.json` 可选；`design_*` 包必须带 `mcp.json`，
且 `use_ai = 1` 时必须带 `skills/`。完整对照见
[package-types.md](./plugin-spec/package-types.md)。这些内容要求由发布检查保证，
安装 / 更新流程本身不校验。

### 5.4 事务与并发

- 安装 / 更新**没有事务**，任一步骤失败前已写入的数据保留，不回滚。
- 失败后不要假设数据处于旧状态；修正输入后重新执行，或按返回错误定位已完成的步骤。
- 同一插件的安装 / 更新应串行执行；并发写入没有额外锁保护。

## 6. 枚举与布尔字段

| 字段 | 取值 |
| --- | --- |
| `plugins.status` | `draft` / `review` / `published` / `offline` / `deprecated` |
| `plugins.platform` | `win` / `mac` / `linux` |
| `plugins.target` | 由清单定义，缺省 `common` |
| `is_system`、`is_enabled`、`use_ai`、`installed`、`enabled`、`public` | `0` / `1` |
| `mask` | 整数权限位，安装默认 `0` |

FFI 入参中的布尔字段接受 `true/false`、`1/0`、`"true"/"false"/"yes"/"no"/"on"/"off"`。

## 7. 实现约束

- SQL 全部在 Rust 内拼装：字符串经 `quote()` 转义单引号；LIKE 经 `like_pattern()`
  转义 `%`、`_`、`\` 并追加 `ESCAPE '\'`。调用方不应自行拼 SQL。
- 对外 FFI 单次响应上限 64 KiB，超限返回 `-2`；库间调用（`database` / `kv_lib` /
  `designdb`）的缓冲区目标为 1024 KiB，见 [skills/ffi-buffer.md](../skills/ffi-buffer.md)，
  当前尚未迁移。designdb 1.0.3 自身的响应上限为 16 MiB，与 1024 KiB 目标仍需协商。
- `plugin_lib` 不负责向量化；`use_ai` 只是召回标记，实际检索由 `chatllm` 负责。
- 提示词 kv 同步依赖 `plugins_type`，该字段为空时不会写入新记录；类型切换或旧类型未知时
  仍会扫描已知集合并清理该插件的记录。Dytes 清单中的 `type` 不会自动映射为 `plugins_type`。

## 8. 一致性检查清单

写入或升级后，按顺序核对：

- [ ] `get_plugins` 能看到插件，`installed = 1`、`version` 正确
- [ ] `get_functions` 的能力 `plugin_uuid`、`uuid`、`ffi_symbol` 正确，JSON Schema 已展开
- [ ] `get_skills` 返回摘要数量、`_id`、`plugin_uuid` 正确；`get_skill` 能按 `id` 取到完整 `content`
- [ ] `get_prompts` 返回完整数组，`plugin_uuid` 正确
- [ ] `get_taskflow` 中当前插件的元素带 `plugin_uuid`，且未覆盖其他插件元素
- [ ] 需要 kv 时，`plugins_type` 命中映射，key 前缀为 `{plugin_uuid}:prompt:`
- [ ] 升级后确认 `prune` 没有误删技能 / 任务流元素 / 能力

## 相关文档

- [插件包结构与插入规范](./plugin-package.md)
- [FFI 接口规范](./ffi-api.md)
