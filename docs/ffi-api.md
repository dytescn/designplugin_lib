# FFI 接口规范

本文描述 `plugin_lib` 对外导出的调用约定、符号清单、请求 / 响应结构与错误码。
接口清单以 `src/ffi/*.rs` 中的 `pub extern "C" fn` 为准。

## 1. 调用方式

### 1.1 经 cyctron HTTP 调用

```text
POST http://127.0.0.1:44944/plugin_lib
Content-Type: application/libary
FFI-Symbol: <符号名>

<请求 JSON>
```

示例：

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_plugins" \
  -d '{"page":1,"page_size":20}'
```

### 1.2 进程内 `dlopen`

```ts
const libName = Deno.build.os === "windows"
  ? "plugin_lib.dll"
  : Deno.build.os === "darwin"
  ? "libplugin_lib.dylib"
  : "libplugin_lib.so";

const dylib = Deno.dlopen(new URL(`../../../.dytes/libary/${libName}`, import.meta.url), {
  get_plugins: { parameters: ["pointer", "pointer"], result: "i32" },
});
```

完整脚本见 [examples/ffi/call.ts](./examples/ffi/call.ts)。

> `plugin_lib` 用相对路径 `./libary/database.dll`、`./libary/libdesigndb.dll` 加载依赖，
> 这些路径相对**当前工作目录**。调用进程的 cwd 必须是包含 `libary/` 的安装根目录；
> 本地测试时通常是 `.dytes/`。`PLUGIN_LIB_DATABASE` 环境变量只覆盖 database 库路径。

## 2. ABI 约定

每个导出符号的签名一致：

```c
int32_t symbol(const char *json_in, char *json_out);
```

| 项目 | 约定 |
| --- | --- |
| `json_in` | NUL 结尾的 UTF-8 JSON 字符串 |
| `json_out` | 调用方分配的缓冲区，大小至少 **65536 字节**（64 KiB）。库间调用（`database` / `kv_lib` / `designdb`）的缓冲区约定见 [skills/ffi-buffer.md](../skills/ffi-buffer.md)，不属于本对外 ABI |
| 返回值 `0` | 已写入响应，继续检查响应中的 `code` |
| 返回值 `-1` | `json_out` 为空指针 |
| 返回值 `-2` | 响应序列化后达到或超过 64 KiB，未写入 |
| 业务错误 | 仍返回 `0`，由响应 JSON 的 `code` / `msg` 表示 |
| 响应文本 | NUL 结尾的 JSON，调用方按第一个 `0` 截断 |

## 3. 响应外壳与错误码

成功：

```json
{ "code": 200, "data": { }, "msg": "success" }
```

失败：

```json
{ "code": 400103, "data": {}, "msg": "name is required" }
```

| `code` | 含义 | 典型场景 |
| --- | --- | --- |
| `200` | 成功 | — |
| `400100` | 输入指针为空 | `json_in` 为 NULL |
| `400101` | 输入不是 UTF-8 | 非法字节序列 |
| `400102` | JSON 为空或解析失败 | 请求体为空、语法错误、字段类型不匹配 |
| `400103` | 参数错误 | 必填字段为空、枚举值非法、无字段可更新 |
| `400404` | 未找到 | 插件 / 能力 / 技能不存在 |
| `400409` | 冲突 | 插件 UUID、能力 UUID、技能 ID 已存在 |
| `500201` | 数据库失败 | 依赖库加载失败、SQL 执行失败 |
| `500300` | 文件失败 | 插件目录、`dytes.json`、库文件读取失败 |
| `500299` | 内部 panic | 不应出现；出现时按缺陷处理 |

## 4. 通用约定

- **字段命名**：请求与响应使用 snake_case；`desc` 与 `public` 是历史字段名，保持原样。
- **分页**：`page` 默认 `1`，最小值 `1`；`page_size` 默认 `20`，取值被限制在 `1..=200`。
- **布尔归一**：接受 `true/false`、`1/0`、`"true"/"false"/"yes"/"no"/"on"/"off"`，
  统一转成 `0/1`。只有 `1`（或 `true` / `"true"` / `"yes"` / `"on"`、数值 `1.0`）视为开启，
  其他已提供的值（包括 `2`、`-1`、浮点数、无法识别的字符串、数组 / 对象）一律按 `0`。
  字段缺失或显式 `null` 视为未提供，部分更新时不修改。
  `post_plugins` 的 `enabled`，以及 `post_plugins` / `update_plugins` 的 `public`、`mask`
  是整数类型，不参与该归一；其中 `mask` 是权限位，按整数处理。
- **时间戳**：`created_at` / `updated_at` 由服务端写入 Unix 秒，调用方不传。
- **JSON 展开**：`plugin_functions` 的 `params_schema`、`params_example`、`return_schema`、
  `return_example` 在数据库中是 TEXT，读取时自动解析为对象 / 数组。
- **物理删除**：`delete_*` 都是直接删除记录，不是软删除。`plugins` 表虽然有 `deleted_at`，
  但查询过滤与 `delete_plugins` 分别是“过滤”和“物理删除”。
- **返回值统一包在 `data` 中**：列表接口返回 `{list,total,page,page_size}`，详情接口返回单行文档。
- **64 KiB 上限**：单次响应不能达到 64 KiB（对外 FFI）。列表接口应减小 `page_size`；
  `get_skills` 会先读出全部匹配文档再分页，数据量大时尤其需要注意。库间调用
  （`database` / `kv_lib` / `designdb`）按 [skills/ffi-buffer.md](../skills/ffi-buffer.md)
  的目标 1024 KiB 规划，当前尚未迁移。

## 5. 接口清单

### 5.1 插件（`plugins` 表）

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `get_plugins` | 列表：`keyword?`、`status?`、`target?`、`platform?`、`page?`、`page_size?`；详情：`uuid?` 或 `name?` | 列表 `{list,total,page,page_size}`；详情为插件行 |
| `post_plugins` | `name`（必填）、`uuid?`、`ouid?`、`plugins_type?`、`version?`、`desc?`、`create_by?`、`cover?`、`status?`、`public?`、`updated_by?`、`target?`、`platform?`、`mask?`、`enabled?` | `{uuid}` |
| `patch_plugins` | `uuid`（必填）、`enabled?`、`status?`、`cover?`、`mask?`（至少一个） | `{updated:true}` |
| `update_plugins` | `uuid`、`name`（均必填）、`ouid?`、`plugins_type?`、`version?`、`desc?`、`create_by?`、`cover?`、`status?`、`public?`、`updated_by?`、`target?`、`platform?`、`mask?` | `{updated:true}` |
| `delete_plugins` | `plugin_uuid?` 或 `name?` | `{plugin_uuid,name,deleted:true}` |

说明：

- `get_plugins` 只要请求中出现非空字符串 `uuid` 或 `name` 就返回详情，且 `uuid` 优先；
  详情查询只过滤 `deleted_at IS NULL`，不校验 `status` 等条件。
- 列表按 `created_at DESC` 排序；`keyword` 同时匹配 `name` 与 `"desc"`（LIKE，自动转义通配符）。
- `post_plugins` 省略 `uuid` 时自动生成 UUID v4；重复 UUID 返回 `400409`。
- `status` 只允许 `draft`、`review`、`published`、`offline`、`deprecated`。
- 新建插件固定写入 `installed = 0`、`install_path = NULL`、`enabled = 1`、`target = 'common'`、
  `platform = 'win'`、`mask = 0`；省略 `status` 时为 `draft`。
- `update_plugins` 对请求中列出的主表字段做覆盖更新，但不修改 `enabled`、`installed`、
  `install_path`、`use_ai`、`created_at`；未传的可选字段会被写为 `NULL` 或默认值，调用前先读取原值。
- `delete_plugins` 只删除 `plugins` 行与 `plugin_functions`，不清理技能、提示词、任务流与版本记录。

### 5.2 能力（`plugin_functions` 表）

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `get_functions` | `uuids?`、`plugin_uuid?`、`page?`、`page_size?` | `{list,total,page,page_size}` |
| `post_function` | `uuid`、`plugin_uuid`、`ffi_symbol`、`title`、`description`（均必填）、`params_schema?`、`params_example?`、`return_schema?`、`return_example?`、`use_ai?` | 新增后的能力行 |
| `update_function` | 同 `post_function`，全部必填字段必须提供 | 更新后的能力行 |
| `patch_function` | `uuid`（必填）、其余字段可选，至少一个 | 更新后的能力行 |
| `delete_function` | `uuid`（必填） | `{uuid,deleted:true}` |

说明：

- 列表按 `uuid ASC` 排序；四个 JSON 字段在响应中已展开。
- `post_function` / `update_function` 的 `params_schema`、`return_schema` 缺省为 `{}`；
  `use_ai` 缺省为 `0`。包安装流程的缺省值不同（`{"type":"object"}`），见
  [plugin-package.md](./plugin-package.md)。
- `patch_function` 的 `use_ai` 使用统一布尔归一：`1` 为开启，其他已提供值为 `0`，
  不再对非 `0/1` 返回 `400103`。
- `delete_function` 物理删除，不校验能力是否属于某个插件。

### 5.3 技能（designdb `skills` 集合）

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `get_skills` | `categories?`、`ids?`、`plugin_uuid?`、`enabled?`、`page?`、`page_size?` | `{list,total,page,page_size}`，列表项为摘要，不含 `content` |
| `get_skill` | `id`（必填） | 完整技能文档（含 `content`） |
| `post_skills` | `id`、`plugin_uuid`、`name`、`category`（均必填）、`description?`、`content?`、`trigger?`、`params_schema?`、`is_system?`、`is_enabled?`、`sort?`、`version?` | 新增后的技能文档 |
| `update_skills` | `id`（必填）、其余字段可选，至少一个 | 更新后的技能文档（`version + 1`） |
| `patch_skills` | 同 `update_skills` | 同 `update_skills` |
| `delete_skills` | `id`（必填） | `{id,deleted:true}` |

说明：

- `get_skills` **只返回摘要列表**，不包含 `content`。传 `with_content: true` 会返回
  `400103`；需要内容时先获取 `id` / `total`，再调用 `get_skill` 单条读取。
- 排序为 `is_system` 降序（系统技能在前）→ `category` 升序 → `sort` 升序 → `_id` 升序。
- `categories`、`ids` 使用 `$in` 过滤；`enabled` 经布尔归一后过滤 `is_enabled`。
- `post_skills` 的 `id` 在 designdb 中**全局唯一**，冲突返回 `400409`。
- `params_schema` 必须是 JSON 对象，否则 `400103`。
- `post_skills` / `update_skills` **当前实现**校验：`name` ≤ 120 字符、`description` ≤ 500 字符、
  `content` ≤ 8000 字符；`content` 不能含 NUL、除 `\n` `\r` `\t` 外的控制字符，也不能包含
  `{{base_system_prompt}}`、`{{categories}}`、`{{skills}}`、`{{rules}}`。
- 规范目标：`content` ≤ **1 MiB（1,048,576 字节，UTF-8）**；当前实现仍为 8000 字符，
  待同步到规范。
- `update_skills` 与 `patch_skills` 是同一实现：只更新传入的字段，每次都 `version + 1`；
  `id` 不存在返回 `400404`，没有任何可更新字段返回 `400103`。
- `delete_skills` 只按 `id` 删除，不校验 `plugin_uuid`。

### 5.4 提示词（designdb `prompts` 集合）

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `get_prompts` | `plugin_uuid`（必填） | `{plugin_uuid,prompts}`；无文档时 `prompts: []` |
| `update_prompts` | `plugin_uuid`（必填）、`prompts`（数组，必填） | 读回后的 `{plugin_uuid,prompts}` |

说明：

- 每个 `plugin_uuid` 一条文档；`prompts` 是数组，没有单条增删改接口。
- `update_prompts` 是整体覆盖；空数组同样会覆盖为空。
- 数组中的每个元素必须是 JSON 对象，否则 `400103`。元素结构由调用方约定，
  常用字段为 `id`、`name`、`content`、`category`、`sort`、`is_enabled`。
- 每个 prompt 的 `content` 去首尾空白后按 UTF-8 编码 ≤ **150 字节**
  （对应 50 个汉字；英文、数字、标点按实际字节计入）；安装与 `update_prompts` 使用同一校验。
- 该接口只写 designdb，不触发 kv 同步；kv 同步发生在包安装 / 更新流程。

### 5.5 任务流（designdb `taskflow` 集合）

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `get_taskflow` | `{}` | `{taskflows}`；无文档时 `taskflows: []` |
| `update_taskflow` | `taskflows`（数组，必填） | 读回后的 `{taskflows}` |

说明：

- 整个集合只有一条文档，`_id` 固定为 `taskflow`。
- `update_taskflow` 是**全局整体覆盖**，会替换所有插件贡献的任务流元素。
  单插件增量请使用 `install_plugin` / `update_plugin`。
- 数组元素必须是 JSON 对象，否则 `400103`。

### 5.6 安装与更新

| 符号 | 请求 | 返回 |
| --- | --- | --- |
| `install_plugin` | `dir`（必填） | 安装统计，见 [plugin-package.md](./plugin-package.md#10-返回结构) |
| `update_plugin` | `dir`（必填）、`plugin_uuid?`、`name?`、`prune?` | 更新统计，见 [plugin-package.md](./plugin-package.md#10-返回结构) |

这两个接口会批量写入 SQLite、designdb 与 kv，规则、默认值和清理行为详见
[插件包结构与插入规范](./plugin-package.md)。

## 6. 变更规范

- 现有符号名、请求字段与返回结构属于对外契约，**不得随意重命名或删除**；
- 新增 FFI 方法、修改表结构必须先由人工确认（见 `skills/dev.md`）；
- 修改实现后同步更新本文档与 `docs/examples/`，并至少执行 `cargo test` 与相关 Deno 集成测试；
- 新增符号时，同时更新：`src/ffi/` 导出、`ts/ffi.ts` 的 `symbols`（如使用）、本文档接口表与示例。
