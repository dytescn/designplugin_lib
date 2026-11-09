# plugin_lib 数据表

> SQLite 表由 `setup.json` 创建；`skills`、`prompts`、`taskflow` 存于 designdb 对象集。
> FFI 中的 **func** 对应 SQLite 表 `plugin_functions`，由扫描器写入，接口只负责查询与部分更新。

## plugins — 插件主表

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uuid` | TEXT PRIMARY KEY NOT NULL | 插件 UUID |
| `ouid` | TEXT | 外部 / 组织 UUID |
| `plugins_type` | TEXT | 类型 UUID |
| `version` | TEXT | 插件版本，对应 `dytes.json.version` |
| `name` | TEXT | 插件名 |
| `desc` | TEXT | 描述；SQL 中字段名需写成 `"desc"` |
| `create_by` | TEXT | 创建者 |
| `cover` | TEXT | 封面路径 / URL |
| `status` | TEXT DEFAULT 'draft' | 发布状态 |
| `public` | INTEGER DEFAULT 0 | 是否公开（0/1） |
| `updated_by` | TEXT | 更新者 |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |
| `deleted_at` | INTEGER | 软删除时间（Unix 秒）；非空即视为已删除 |
| `installed` | INTEGER DEFAULT 0 | 是否已安装（0/1） |
| `install_path` | TEXT | 安装目录 |
| `enabled` | INTEGER DEFAULT 1 | 是否启用（0/1） |
| `target` | TEXT DEFAULT 'common' | 目标软件 |
| `platform` | TEXT NOT NULL DEFAULT 'win' | 支持的系统：`mac` / `win` / `linux` |
| `use_ai` | INTEGER NOT NULL DEFAULT 0 | 是否参与 AI 召回（0/1） |
| `mask` | INTEGER NOT NULL DEFAULT 0 | 权限编码（Linux 权限位，如 `0755`） |

- 主键：`uuid`。
- 索引：`idx_plugins_target(target)`、`idx_plugins_status(status)`、`idx_plugins_name(name)`。
- 旧库迁移补列：`installed`、`install_path`、`enabled`、`target`、`use_ai`。
- `status` 取值：`draft`、`review`、`published`、`offline`、`deprecated`。

### plugins 对应 FFI

| 方法 | 请求字段 | 说明 |
| --- | --- | --- |
| `get_plugins` | `keyword`、`status`、`target`、`platform`、`page`、`page_size` | 只返回 `deleted_at IS NULL`；带 `uuid` / `name` 时返回详情 |
| `post_plugins` | `name`（必填）、`uuid`、`ouid`、`plugins_type`、`version`、`desc`、`create_by`、`cover`、`status`、`public`、`updated_by`、`target`、`platform`、`mask`、`enabled` | `uuid` 省略时自动生成 |
| `patch_plugins` | `uuid`（必填）、`enabled`、`status`、`cover`、`mask` | 部分更新 |
| `update_plugins` | `uuid`、`name`（必填）、`ouid`、`plugins_type`、`version`、`desc`、`create_by`、`cover`、`status`、`public`、`updated_by`、`target`、`platform`、`mask` | 全量更新 |
| `delete_plugins` | `plugin_uuid` 或 `name` | 删除插件并清理关联数据 |

## func — 方法回调表（`plugin_functions`）

代码与 FFI 中简称 **func**，实际表名是 `plugin_functions`，通过 `plugin_uuid` 归属插件。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uuid` | TEXT PRIMARY KEY NOT NULL | 方法编号 |
| `plugin_uuid` | TEXT NOT NULL | 所属插件 UUID |
| `ffi_symbol` | TEXT NOT NULL | 实际导出的 FFI 符号 |
| `title` | TEXT NOT NULL | 能力标题 |
| `description` | TEXT NOT NULL | 能力说明 |
| `params_schema` | TEXT NOT NULL | 入参 JSON Schema |
| `params_example` | TEXT | 入参示例（JSON） |
| `return_schema` | TEXT NOT NULL | 返回 JSON Schema |
| `return_example` | TEXT | 返回示例（JSON） |
| `use_ai` | INTEGER NOT NULL DEFAULT 0 | 是否参与 AI 召回（0/1） |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |

- 主键：`uuid`。
- 索引：`idx_plugin_functions_lookup(ffi_symbol)`。
- 读取时 `params_schema`、`params_example`、`return_schema`、`return_example` 会展开为 JSON。

### func 对应 FFI

| 方法 | 请求字段 | 说明 |
| --- | --- | --- |
| `get_functions` | `uuids`、`plugin_uuid`、`page`、`page_size` | `plugin_uuid` 用于按所属插件过滤；返回 `{list,total,page,page_size}` |
| `post_function` | `uuid`、`plugin_uuid`、`ffi_symbol`、`title`、`description`（均必填）、`params_schema`、`params_example`、`return_schema`、`return_example`、`use_ai` | 新增能力，`uuid` 重复时返回冲突 |
| `update_function` | 同 `post_function` | 全量更新 |
| `patch_function` | `uuid`（必填）、`plugin_uuid`、`ffi_symbol`、`title`、`description`、`params_schema`、`params_example`、`return_schema`、`return_example`、`use_ai` | 部分更新，至少传一个字段 |
| `delete_function` | `uuid`（必填） | 删除能力 |

> func 数据可通过上述 FFI 增删改查，扫描器也可能写入。

## skills — designdb 对象集

技能不使用 SQLite 表，统一存放在 designdb 的 `skills` 对象集（`path = skills`、`collection = skills`）。每条文档一个技能，通过 `plugin_uuid` 关联所属插件；整个项目只使用这一个对象集。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` / `id` | TEXT | 技能 ID |
| `plugin_uuid` | TEXT | 所属插件 UUID |
| `name` | TEXT | 技能名 |
| `description` | TEXT | 描述 |
| `category` | TEXT | 分类 code |
| `content` | TEXT | 技能正文 |
| `trigger` | TEXT | 触发条件 |
| `params_schema` | OBJECT | 入参 JSON Schema |
| `is_system` | INTEGER | 是否内置（0/1） |
| `is_enabled` | INTEGER | 是否启用（0/1） |
| `sort` | INTEGER | 升序排序 |
| `version` | INTEGER | 版本号 |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |

> `setup.json` 不再创建 `skills` / `plugin_skills` 表；增删改由 FFI 直接写入 designdb。

## prompts — designdb 对象集

提示词不使用 SQLite 表，按 `plugin_uuid` 存放在 designdb 的 `prompts` 对象集（`path = prompts`、`collection = prompts`）。每个插件一条文档，`prompts` 字段直接是数组，不提供单条操作。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` / `plugin_uuid` | TEXT | 所属插件 UUID（文档主键） |
| `prompts` | ARRAY | 该插件的提示词数组，元素结构由调用方定义 |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |

> 通过 `get_prompts` 读取、`update_prompts` 整体覆盖；`setup.json` 不创建 prompts 表。

## taskflow — designdb 对象集

任务流不使用 SQLite 表，存放在 designdb 的 `taskflow` 对象集（`path = taskflow`、`collection = taskflow`）。整个集合只有一条文档，`taskflows` 字段直接是数组，不提供单条操作。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` | TEXT | 固定为 `taskflow` |
| `taskflows` | ARRAY | 任务流数组，元素结构由调用方定义 |
| `created_at` | INTEGER | 创建时间（Unix 秒） |
| `updated_at` | INTEGER | 更新时间（Unix 秒） |

> 通过 `get_taskflow` 读取、`update_taskflow` 整体覆盖；`setup.json` 不创建 taskflow 表。
