# designplugin_lib MCP 服务

> 状态：接口文档（对应 `designplugin_lib` / 包名 `plugin_lib` 1.0.7 的 FFI）
> 更新日期：2026-10-28
> 打包产物：`target/release/plugin_lib.dll`
> 底层接口：`get_*`、`post_*`、`patch_*`、`update_*`、`delete_*`、`check_updates_sse`

本文把 `designplugin_lib` 的 FFI 整理为 MCP 工具：工具名、输入结构、错误语义、
POST 别名与实现注意事项。MCP server 由宿主实现，底层通过 FFI 调用
`plugin_lib.dll`，也可以直接依赖本 crate 的 `rlib`。

`designplugin_lib` 只负责插件元数据、安装状态、动态库 / 能力登记、技能与扫描；
插件能力本身由各插件 DLL / MCP 提供。

---

## 1. 工具映射规则

| 项 | 规则 |
| --- | --- |
| 工具名 | `plugin_` + 规范 FFI-Symbol（如 `plugin_get_plugins`） |
| 底层符号 | 规范符号本身（如 `get_plugins`） |
| HTTP（可选） | `POST /plugin_lib` + `FFI-Symbol: get_plugins` → `post_get_plugins` |
| 输出外壳 | `{ "code": 200, "data": {}, "msg": "success" }` |
| 只读工具 | `get_*` / `get_skill*` |
| 写工具 | `post_*` / `patch_*` / `update_*` / `delete_*` |

> **规范符号 vs POST 别名**：`get_*`、`patch_*`、`update_*`、`delete_skill*` 在
> cyctron 中无法直接用 GET 传 body，因此仓库提供了 `post_get_*`、`post_patch_*`、
> `post_update_*`、`post_delete_skill*` 别名（见 §6）。MCP 只暴露规范工具，
> 由 server 决定调用规范符号还是别名。

---

## 2. 调用约定

### 2.1 FFI

```c
int get_plugins(const char *json_in, char *json_out);
```

| 项 | 约定 |
| --- | --- |
| `json_in` | NUL 结尾 UTF-8 JSON；空对象 `{}` 表示无参数 |
| `json_out` | 至少 65536 字节（含结尾 NUL） |
| 返回值 | `0` 已写入；`-1` 输出指针为空；`-2` 输出超过 64 KiB |
| 业务状态 | 看 JSON 的 `code` 字段 |
| 字段命名 | `snake_case` |
| panic | 捕获后返回 `500299 internal panic` |

### 2.2 错误码

| code | 含义 |
| --- | --- |
| `200` | 成功 |
| `400100` | `json_in` 为空 |
| `400101` | 入参不是合法 UTF-8 |
| `400102` | JSON 为空 / 解析失败 |
| `400103` | 必填字段为空 / 非法参数 |
| `400404` | 资源不存在 |
| `400409` | 唯一键冲突 / 已存在 |
| `500201` | 数据库加载 / 查询 / 写入失败 |
| `500299` | 内部 panic |
| `500300` | 能力库 / 文件相关失败 |

MCP 映射建议：

- `4001xx` / `4004xx` / `400409` → 工具结果 `isError: true`，`content` 使用 `msg`；
- `5002xx` / `5003xx` → 工具结果 `isError: true`，并记录 server 日志；
- FFI 返回 `-1` / `-2` → server 内部错误。

---

## 3. 输入结构

### 3.1 插件列表 `GetPluginsReq`

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `category` | string | 分类；别名 `category_id` |
| `keyword` | string | 名称 / 描述模糊匹配 |
| `status` | string | 状态过滤 |
| `target` | string | 目标软件（`common` / `coreldraw` / `vectorcraft`） |
| `page` | int | 默认 1 |
| `page_size` | int | 默认 20 |

### 3.2 插件详情 `GetPluginReq`

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uuid` | string | 插件 uuid |
| `name` | string | 插件名 |

至少一个，否则 `400103`。

### 3.3 分类 / 版本 / 配置 / 安装

| 结构 | 字段 |
| --- | --- |
| `GetCategoriesReq` | `parent_uuid?` |
| `GetVersionsReq` | `plugin_uuid`（必填） |
| `GetConfigsReq` | `plugin_uuid`（必填）、`version?`、`version_id?` |
| `GetInstallReq` | `plugin_uuid`（必填）、`version?` |

### 3.4 动态库 / 能力 `GetLibsReq` / `GetFunctionsReq`

`GetLibsReq`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `plugin_uuid` | string | 可选 |
| `local` | bool / 0/1 | 是否本地库 |

`GetFunctionsReq`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uuids` | string[] | 指定能力 uuid |
| `lib_uuid` / `lib_name` | string | 指定动态库 |
| `plugin_uuid` | string | 指定插件 |
| `local` | bool / 0/1 | 本地能力 |
| `use_ai` | bool / 0/1 | 是否参与 AI 召回 |
| `embed_status` | int / int[] | 向量化状态 |
| `category` | string | 能力分类 |
| `page` / `page_size` | int | 分页 |

### 3.5 技能 `GetSkillsReq` / `GetSkillReq`

`GetSkillsReq`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `categories` | string[] | 技能分类 |
| `ids` | string[] | 技能 id |
| `plugin_uuid` | string | 插件 |
| `enabled` | bool / 0/1 | 是否启用 |
| `with_content` | bool | 是否返回正文 |
| `page` / `page_size` | int | 分页 |

`GetSkillReq`：`id`（必填）。
`GetSkillCategoriesReq`：无参数。

### 3.6 新增 `Post*Req`

| 结构 | 必填 | 可选 |
| --- | --- | --- |
| `PostPluginReq` | `name` | `uuid`、`ouid`、`flow_uid`、`type_uid`、`category_id`、`desc`、`create_by`、`cover`、`status`、`public`、`updated_by`、`plugin_type`、`target`、`enabled` |
| `PostCategoryReq` | `type_name` | `uuid`、`icon`、`parent_uuid`、`sort_order`、`created_by` |
| `PostVersionReq` | `plugin_uuid`、`version` | `changelog`、`file_path`、`file_size`、`file_hash`、`file_type`、`min_engine_ver`、`max_engine_ver`、`status`、`is_latest`、`created_by` |
| `PostConfigReq` | `plugin_uuid`、`param_key` | `version_id`、`version`、`param_type`、`default_value`、`required`、`description`、`options`、`sort_order` |
| `PostSkillReq` | `id`、`name`、`category` | `description`、`content`、`trigger`、`params_schema`、`is_system`、`is_enabled`、`sort`、`version` |
| `PostSkillCategoryReq` | `code`、`name` | `description`、`sort` |
| `ScanReq` | — | `dir`（默认 `./plugins`）、`persist`（默认 `true`） |
| `InstallReq` | `plugin_uuid` 或 `name` | `version`、`file_path` |
| `UninstallReq` | `plugin_uuid` 或 `name` | — |

### 3.7 部分更新 `Patch*Req`

| 结构 | 必填 | 可选 |
| --- | --- | --- |
| `PatchPluginReq` | `uuid` | `enabled`、`status`、`category_id`、`cover` |
| `PatchInstallReq` | `plugin_uuid` | `version`、`status`、`install_path`、`enabled` |
| `PatchConfigReq` | `plugin_uuid`、`key`、`value` | `version`、`version_id` |
| `PatchFunctionReq` | `uuid` | `use_ai`、`embed_status`、`embed_model`、`embed_text_hash`、`embedded_at` |

### 3.8 全量更新 `Update*Req`

| 结构 | 必填 | 可选 |
| --- | --- | --- |
| `UpdatePluginReq` | `uuid`、`name` | 同 `PostPluginReq`（不含 `enabled`） |
| `UpdateCategoryReq` | `uuid`、`type_name` | `icon`、`parent_uuid`、`sort_order`、`updated_by` |
| `UpdateVersionReq` | `id`、`version` | 同 `PostVersionReq`（不含 `plugin_uuid`） |
| `UpdateConfigReq` | `id`、`param_key` | `param_type`、`default_value`、`required`、`description`、`options`、`sort_order` |
| `UpdateSkillReq` | `id` | `name`、`description`、`category`、`content`、`trigger`、`params_schema`、`is_system`、`is_enabled`、`sort` |
| `UpdateSkillCategoryReq` | `code` | `name`、`description`、`sort` |

### 3.9 删除 `Delete*Req`

| 结构 | 必填 |
| --- | --- |
| `DeleteSkillReq` | `id` |
| `DeleteSkillCategoryReq` | `code` |

---

## 4. 工具清单（36）

### 4.1 读取（11）

| 工具 | 规范符号 | 说明 | 输入 |
| --- | --- | --- | --- |
| `plugin_get_plugins` | `get_plugins` | 插件分页列表 | `GetPluginsReq` |
| `plugin_get_plugin` | `get_plugin` | 单个插件（`uuid` / `name`） | `GetPluginReq` |
| `plugin_get_categories` | `get_categories` | 分类列表 | `GetCategoriesReq` |
| `plugin_get_versions` | `get_versions` | 版本列表 | `GetVersionsReq` |
| `plugin_get_configs` | `get_configs` | 配置定义 + 当前值 | `GetConfigsReq` |
| `plugin_get_install` | `get_install` | 本机安装记录 | `GetInstallReq` |
| `plugin_get_libs` | `get_libs` | 动态库列表 | `GetLibsReq` |
| `plugin_get_functions` | `get_functions` | 能力清单 | `GetFunctionsReq` |
| `plugin_get_skill_categories` | `get_skill_categories` | 技能分类列表 | `{}` |
| `plugin_get_skills` | `get_skills` | 技能列表 | `GetSkillsReq` |
| `plugin_get_skill` | `get_skill` | 技能正文 | `GetSkillReq` |

### 4.2 新增（12）

| 工具 | 规范符号 | 说明 | 输入 |
| --- | --- | --- | --- |
| `plugin_post_plugin` | `post_plugin` | 新增插件 | `PostPluginReq` |
| `plugin_post_category` | `post_category` | 新增分类 | `PostCategoryReq` |
| `plugin_post_version` | `post_version` | 新增版本 | `PostVersionReq` |
| `plugin_post_config` | `post_config` | 新增配置定义 | `PostConfigReq` |
| `plugin_post_skill` | `post_skill` | 新增技能 | `PostSkillReq` |
| `plugin_post_skill_category` | `post_skill_category` | 新增技能分类 | `PostSkillCategoryReq` |
| `plugin_post_scan` | `post_scan` | 扫描 `./plugins` 并登记 | `ScanReq` |
| `plugin_post_check_updates` | `post_check_updates` | 调用 `dytes update --check` 检查更新 | `{}` |
| `plugin_post_install` | `post_install` | 通过 `dytes install plugin` 安装插件 | `InstallReq` |
| `plugin_post_uninstall` | `post_uninstall` | 卸载插件 | `UninstallReq` |
| `plugin_post_update` | `post_update` | 通过 `dytes install plugin` **重新安装**最新版本 | `InstallReq` |
| `plugin_post_delete` | `post_delete` | 物理删除插件与文件 | `UninstallReq` |

> `plugin_post_update` 是“重新安装”；更新插件元数据请用 `plugin_update_plugin`。

### 4.3 部分更新（4）

| 工具 | 规范符号 | 说明 | 输入 |
| --- | --- | --- | --- |
| `plugin_patch_plugin` | `patch_plugin` | 局部更新插件 | `PatchPluginReq` |
| `plugin_patch_install` | `patch_install` | 局部更新安装状态 | `PatchInstallReq` |
| `plugin_patch_config` | `patch_config` | 保存配置值 | `PatchConfigReq` |
| `plugin_patch_function` | `patch_function` | 回写向量 / 召回状态 | `PatchFunctionReq` |

### 4.4 全量更新（6）

| 工具 | 规范符号 | 说明 | 输入 |
| --- | --- | --- | --- |
| `plugin_update_plugin` | `update_plugin` | 更新插件元数据 | `UpdatePluginReq` |
| `plugin_update_category` | `update_category` | 更新分类 | `UpdateCategoryReq` |
| `plugin_update_version` | `update_version` | 更新版本 | `UpdateVersionReq` |
| `plugin_update_config` | `update_config` | 更新配置定义 | `UpdateConfigReq` |
| `plugin_update_skill` | `update_skill` | 更新技能 | `UpdateSkillReq` |
| `plugin_update_skill_category` | `update_skill_category` | 更新技能分类 | `UpdateSkillCategoryReq` |

### 4.5 删除（2）

| 工具 | 规范符号 | 说明 | 输入 |
| --- | --- | --- | --- |
| `plugin_delete_skill` | `delete_skill` | 物理删除技能 | `DeleteSkillReq` |
| `plugin_delete_skill_category` | `delete_skill_category` | 物理删除技能分类 | `DeleteSkillCategoryReq` |

### 4.6 SSE（1）

| 工具 / 资源 | 符号 | 说明 |
| --- | --- | --- |
| `plugin_check_updates_sse` | `check_updates_sse` | 供 cyctron `/sse?lib=plugin_lib&symbol=check_updates_sse` 使用，输出作为事件 `data` |

> SSE 不是普通工具；建议作为 MCP 资源或宿主侧事件源，不建议放进 `tools/list`。

---

## 5. 输出

统一外壳：

```json
{ "code": 200, "data": {}, "msg": "success" }
```

常见 `data`：

| 工具 | `data` |
| --- | --- |
| `get_plugins` | `{list, page, page_size, total}` |
| `get_plugin` | `{plugin, versions, installs}` |
| `get_categories` | `{list}` |
| `get_versions` | `{list}` |
| `get_configs` | `{configs}` |
| `get_install` | `{install}` |
| `get_libs` | `{list}` |
| `get_functions` | `{list, page, page_size, total}` |
| `get_skill_categories` | `{list}` |
| `get_skills` / `get_skill` | 技能记录 / 列表 |
| `post_scan` | `{found, added, updated, libs, functions, skills}` |
| `post_install` / `post_update` | 安装结果（含插件 uuid、版本、路径） |
| `post_check_updates` | 更新检查结果 |
| `post_delete` | 删除结果与 `function_uuids` |

以源码与 `dytes_sys_operations` 的 `sysinfo/sys/designplugin_lib/instro.md` 为准。

---

## 6. POST 别名（23）

浏览器不能给 GET 携带 body，因此 cyctron 侧使用 POST 别名：

| 规范符号 | POST 别名 |
| --- | --- |
| `get_plugins` | `post_get_plugins` |
| `get_plugin` | `post_get_plugin` |
| `get_categories` | `post_get_categories` |
| `get_versions` | `post_get_versions` |
| `get_configs` | `post_get_configs` |
| `get_install` | `post_get_install` |
| `get_libs` | `post_get_libs` |
| `get_functions` | `post_get_functions` |
| `get_skill_categories` | `post_get_skill_categories` |
| `get_skills` | `post_get_skills` |
| `get_skill` | `post_get_skill` |
| `patch_plugin` | `post_patch_plugin` |
| `patch_install` | `post_patch_install` |
| `patch_config` | `post_patch_config` |
| `patch_function` | `post_patch_function` |
| `update_plugin` | `post_update_plugin` |
| `update_category` | `post_update_category` |
| `update_version` | `post_update_version` |
| `update_config` | `post_update_config` |
| `update_skill` | `post_update_skill` |
| `update_skill_category` | `post_update_skill_category` |
| `delete_skill` | `post_delete_skill` |
| `delete_skill_category` | `post_delete_skill_category` |

MCP server 可以只调用规范符号；如果走 cyctron HTTP，需要使用别名。

---

## 7. MCP 工具 Schema 示例

### 7.1 插件列表

```json
{
  "name": "plugin_get_plugins",
  "description": "分页查询设计插件，支持分类、关键词、状态与目标软件过滤。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "category": { "type": "string" },
      "keyword": { "type": "string" },
      "status": { "type": "string" },
      "target": { "type": "string", "enum": ["common", "coreldraw", "vectorcraft"] },
      "page": { "type": "integer", "minimum": 1, "default": 1 },
      "page_size": { "type": "integer", "minimum": 1, "default": 20 }
    },
    "additionalProperties": false
  }
}
```

### 7.2 扫描

```json
{
  "name": "plugin_post_scan",
  "description": "扫描已安装插件目录，登记动态库、能力与技能。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "dir": { "type": "string", "default": "./plugins" },
      "persist": { "type": "boolean", "default": true }
    },
    "additionalProperties": false
  }
}
```

### 7.3 安装 / 卸载

```json
{
  "name": "plugin_post_install",
  "description": "通过 dytes 安装设计插件；plugin_uuid 或 name 至少一个。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "plugin_uuid": { "type": "string" },
      "name": { "type": "string" },
      "version": { "type": "string" },
      "file_path": { "type": "string" }
    },
    "anyOf": [
      { "required": ["plugin_uuid"] },
      { "required": ["name"] }
    ],
    "additionalProperties": false
  }
}
```

### 7.4 配置

```json
{
  "name": "plugin_patch_config",
  "description": "保存插件配置值。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "plugin_uuid": { "type": "string", "minLength": 1 },
      "key": { "type": "string", "minLength": 1 },
      "value": {},
      "version": { "type": "string" },
      "version_id": { "type": "integer" }
    },
    "required": ["plugin_uuid", "key", "value"],
    "additionalProperties": false
  }
}
```

### 7.5 技能

```json
{
  "name": "plugin_post_skill",
  "description": "新增技能；id、name、category 必填。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string", "minLength": 1 },
      "name": { "type": "string", "minLength": 1 },
      "description": { "type": "string" },
      "category": { "type": "string", "minLength": 1 },
      "content": { "type": "string" },
      "trigger": { "type": "string" },
      "params_schema": { "type": "object" },
      "is_system": { "type": ["boolean", "integer"] },
      "is_enabled": { "type": ["boolean", "integer"] },
      "sort": { "type": "integer" },
      "version": { "type": "integer" }
    },
    "required": ["id", "name", "category"],
    "additionalProperties": false
  }
}
```

---

## 8. 实现指南

### 8.1 FFI 调用

```c
int get_plugins(const char *json_in, char *json_out);
```

```rust
let lib = unsafe { Library::new("plugin_lib.dll")? };
let f: Symbol<unsafe extern "C" fn(*const c_char, *mut c_char) -> i32> =
    unsafe { lib.get(b"get_plugins\0")? };
let input = CString::new(r#"{"page":1,"page_size":20}"#)?;
let mut out = vec![0 as c_char; 64 * 1024];
let ret = unsafe { f(input.as_ptr(), out.as_mut_ptr()) };
assert_eq!(ret, 0);
```

### 8.2 缓冲区

- 固定 **64 KiB**（含 NUL）；
- `-2` 时扩容重试或报内部错误；
- `-1` 表示 `json_out` 为空，属于 server 编码错误。

### 8.3 依赖与启动

- 依赖 `database.dll`（`plugins.db`）与 `libostfil.dll`；
- 安装 / 扫描会读写 `plugins/` 与 `plugins.db`，建议启动后先 `plugin_post_scan`（或确认已扫描）；
- `plugin_post_install` / `post_update` / `post_check_updates` 会调用外部 `dytes`，
  需要网络与可用的 `dytes` 可执行文件：
  - 优先使用 `DYTES_BIN` 指定的路径；
  - 其次查找安装根目录下的 `dytes.exe` / `dytes`；
  - 最后查找 `PATH`。

### 8.4 并发

- 安装 / 卸载 / 删除 / 扫描会修改文件与数据库，建议全局串行；
- `get_*` 可以并发，但都走同步 SQL；
- `patch_function` 可能被 `chatllm` 并发回写，建议按 `uuid` 限流。

### 8.5 安全

- `plugin_post_install` 会通过 dytes 下载并安装 DLL，属于供应链风险；建议白名单与用户确认；
- `plugin_post_delete` 会物理删除安装目录与元数据，需二次确认；
- 技能正文可能包含系统提示词，输出应遵循宿主权限策略。

---

## 9. 资源与提示词（可选）

| URI | 内容 |
| --- | --- |
| `plugin://catalog` | 插件列表 + 分类（`get_plugins` / `get_categories` 缓存） |
| `plugin://installed` | 本机安装状态（`get_install`） |
| `plugin://skills` | 技能索引（`get_skills`） |

提示词示例：

```json
{
  "name": "install-design-plugin",
  "description": "按名称安装设计插件并扫描能力",
  "arguments": [
    { "name": "name", "required": true },
    { "name": "version", "required": false }
  ]
}
```

---

## 10. 版本与兼容

| 项 | 值 |
| --- | --- |
| 包名 / 版本 | `plugin_lib` / 1.0.7 |
| 发布产物 | `plugin_lib.dll`、`setup.ts` |
| 规范工具数 | 36（不含 23 个 POST 别名） |
| 平台 | Windows（`cdylib`） |
| 输出缓冲 | 64 KiB |
| 数据库 | `plugins.db` |

升级时：

- 新增工具保持规范符号与 `plugin_` 前缀；
- 新增字段保持向后兼容；
- 错误码只新增，不复用；
- 新增 GET / PATCH 接口时同步补 POST 别名，供浏览器与 cyctron 使用。
