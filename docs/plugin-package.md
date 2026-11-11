# 插件包结构与插入规范

本文规定一个插件包如何组织，以及 `install_plugin` / `update_plugin` 会如何把包内数据写入
`plugins.db`、designdb 与 kv。示例见 [examples/demo-plugin](./examples/demo-plugin/)。
面向插件作者的强制 / 推荐条款见 [插件规范](./plugin-spec/README.md)；本文侧重系统实际行为。

## 1. 适用范围

- 设计插件（含动态库）与 MCP-only 资源包；当前实现下两者都必须提供当前平台库文件
  （MCP-only 包可以使用占位库）；
- 通过 `install_plugin` 首次安装、通过 `update_plugin` 升级的包；
- 不覆盖 `plugin_lib` 自身作为 sys 包的发布清单（其 `dytes.json` 字段更少，由 dytes 安装器消费）。

> 安装流程只**读取**插件包，不复制、不移动任何文件。`install_path` 记录的是请求里的 `dir` 原样。
> 调用方应先把包放到最终目录（如 `./plugins/<name>/`），再调用安装接口。

## 2. 目录结构

```text
<package>/
├── dytes.json              # 必填，插件清单
├── <lib>.dll | .so | .dylib  # 必填（当前实现无条件校验；MCP-only 包也需占位库）
├── skills/                 # 按类型：chat_think、chat_reply 必须；design_* 且 use_ai=1 必须
│   ├── demo_design_layout.md
│   └── demo_design_export.md
├── prompts.json            # 可选，提示词数组
├── taskflow.json           # 按类型：chat_plugins 必须
└── mcp.json                # 按类型：design_* 必须；chat_* 可选；其他可选；也支持 mcp/mcp.json
```

- `skills/` 必须直接位于包根目录，不递归子目录；
- 库文件可以位于包根目录（推荐），也可以是 `platform-exports` 中声明的目标文件名；
- 所有 JSON 文件必须是 UTF-8 编码、合法 JSON。

### 2.1 按包类型的内容要求

发布时按 `plugins_type` 判定必需内容：

- `chat_think` / `chat_reply`：必须带 `skills/`（至少一个有效技能）；
- `chat_plugins`：必须带 `taskflow.json`（至少一个任务流元素）；
- `chat_*`：可以没有 `mcp.json`；
- `design_*`：必须带 `mcp.json`（包根或 `mcp/mcp.json`），且 `tools` 是数组；
  `design_*` 的 `use_ai` 归一化为 `1` 时还必须带 `skills/`（至少一个有效技能）。

完整规范见 [plugin-spec/package-types.md](./plugin-spec/package-types.md)。安装流程只校验
清单与库文件，缺少技能 / 任务流 / 能力时只会让对应数据为空或保留旧值；升级时缺少
`skills/` 会触发默认 `prune` 清理全部技能，缺少 `taskflow.json` 或 `mcp.json` 则保留原数据。

## 3. `dytes.json`

安装流程实际读取以下字段：

| 字段 | 必填 | 写入位置 / 用途 | 缺省 |
| --- | --- | --- | --- |
| `uuid` | 是 | `plugins.uuid`，同时作为技能 / 提示词 / 任务流 / 能力的归属键 | — |
| `name` | 是 | `plugins.name`；库文件同名优先匹配；kv 记录的 `plugin_name` | — |
| `version` | 否 | `plugins.version`；与已安装版本比较 | `0.0.0` |
| `description` | 否 | `plugins.desc` | 空字符串 |
| `plugins_type` | 否 | `plugins.plugins_type`；提示词同步 kv 时用于选择集合 | 空字符串 |
| `target` | 否 | `plugins.target` | `common` |
| `use_ai` | 否 | `plugins.use_ai`；`mcp.json` 工具未声明 `use_ai` 时继承 | `0` |
| `platform-exports` | 否 | 定位当前平台库文件；缺省时回退扫描包根目录 | — |

注意事项：

- **`type` 与 `plugins_type` 不是同一个字段。** 安装流程只读取 `plugins_type`。Dytes 清单里的
  `"type": "plugins"` 不会写入 `plugins_type`；需要 kv 同步时必须显式写 `plugins_type`
  （如 `chat_plugins` / `sys_plugins` / `chat_think` / `chat_reply`）。
- `keywords`、`author`、`license`、`init`、`dependencies-sys` 等清单字段不影响本次安装。
- 安装新插件时主表固定写入：`installed = 1`、`enabled = 1`、`mask = 0`、`status = 'draft'`；
  `cover`、`ouid`、`create_by`、`public` 等保持空值。已有插件再次安装时，只更新
  `plugins_type`、`version`、`name`、`desc`、`target`、`platform`、`install_path`、`installed`、
  `use_ai`、`updated_at`，不覆盖 `status` / `enabled` / `mask` / `created_at`。

示例：

```json
{
  "name": "demo_design",
  "uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  "version": "1.0.0",
  "description": "plugin_lib 文档示例插件",
  "type": "plugins",
  "plugins_type": "chat_plugins",
  "target": "common",
  "use_ai": 1,
  "platform-exports": {
    "windows": { "./target/release/demo_design.dll": "demo_design.dll" },
    "linux": { "./target/release/libdemo_design.so": "libdemo_design.so" },
    "macos": { "./target/release/libdemo_design.dylib": "libdemo_design.dylib" }
  }
}
```

## 4. 库文件定位规则

按当前运行系统选择扩展名：

| 运行系统 | `platform` 写入值 | `platform-exports` 键 | 扩展名 |
| --- | --- | --- | --- |
| Windows | `win` | `windows` | `.dll` |
| Linux | `linux` | `linux` | `.so` |
| macOS | `mac` | `macos` | `.dylib` |

定位顺序：

1. 读取 `platform-exports[当前平台]` 的每个键值对：值（目标文件名）或键（源路径）以当前扩展名结尾时，
   取该文件名作为候选；
2. `platform-exports` 缺失或没有候选时，扫描包根目录下扩展名匹配的文件；
3. 候选必须在包目录内实际存在；
4. 多个候选时优先文件名（去扩展名）等于 `name` 或 `lib<name>` 的文件，否则取第一个；
5. 没有任何可用文件时安装失败，错误码 `500300`。

> 当前安装流程**无条件**定位库文件：MCP-only 包也必须在包根放置当前平台库文件
> （可以是占位文件），否则返回 `500300`。详见
> [dytes.json 规范](./plugin-spec/dytes-json.md#61-关于-mcp-only-包)。

## 5. `skills/*.md` 插入规范

每个 `.md` 文件生成 designdb `skills` 集合中的一条文档。

| 文档字段 | 取值规则 |
| --- | --- |
| `_id` / `id` | 文件名去掉 `.md`；去空白后不能为空 |
| `plugin_uuid` | `dytes.json.uuid` |
| `name` | 文件第一个 `# ` 开头的标题行；没有则用文件名 |
| `content` | 文件全文（包含标题行） |
| `description` | 固定空字符串 |
| `category` | 固定 `plugin` |
| `trigger` | 固定空字符串 |
| `params_schema` | 固定 `{}` |
| `is_system` | `0` |
| `is_enabled` | `1` |
| `sort` | 文件按名称升序排序后的序号 × 10 |
| `version` | 新插入为 `1`；内容变化时 `+1` |

必须遵守：

- **文件名即全局主键。** `_id` 在整个 designdb 中唯一，不是“每个插件内唯一”。建议使用
  `<插件名>_<技能名>.md`，字符限定为小写字母、数字、下划线或连字符。
  两个插件的技能文件同名时，第二次安装会因主键冲突失败。
- 不要写 YAML frontmatter。安装流程不解析 frontmatter，它会原样进入 `content`；
  需要结构化字段时，安装后调用 `patch_skills` 补写。
- 空文件会被跳过；跳过后若处于 `update` 且 `prune = true`，对应的旧技能会被删除。
- 安装流程不做长度 / 字符校验。发布规范要求：
  `name` ≤ 120 字符、`description` ≤ 500 字符、
  `content` ≤ 1 MiB（1,048,576 字节，按 UTF-8 编码后的文件大小）；
  不包含 NUL、除 `\n` `\r` `\t` 外的控制字符，以及
  `{{base_system_prompt}}`、`{{categories}}`、`{{skills}}`、`{{rules}}` 四个模板占位符。
  当前 FFI 实现仍按 8000 字符校验 `content`，需后续同步到 1 MiB。

示例：

```markdown
# 版式布局助手

当用户需要版式、栅格、间距或视觉层级建议时使用本技能。

## 触发条件

用户提到版式、排版、布局、栅格、留白。

## 执行步骤

1. 读取画布尺寸与出血线。
2. 给出 12 栅格、边距与基线建议。
3. 说明不同媒介下的适配差异。
```

写入行为：

- 已存在同 `_id`（且属于当前插件）的技能：仅当 `content`、`name` 或 `category` 与包内不一致时更新，
  并执行 `version + 1`、刷新 `updated_at`；
- 内容完全一致时不写入，不递增版本；
- `install_plugin` 不清理旧技能；`update_plugin` 在 `prune = true`（默认）时删除
  **当前插件**下、新包中已不存在的技能；
- `update_plugin` 时如果新包**没有 `skills/` 目录**，会视为“包内没有技能”，
  `prune = true` 将删除该插件的全部技能。只想更新库文件时请保留原技能文件，或传 `prune: false`。

## 6. `prompts.json` 插入规范

支持两种顶层结构：

```json
[
  { "id": "layout-advisor", "name": "版式顾问", "content": "……", "category": "design", "sort": 10, "is_enabled": 1 }
]
```

```json
{
  "prompts": [
    { "id": "layout-advisor", "name": "版式顾问", "content": "……", "category": "design", "sort": 10, "is_enabled": 1 }
  ]
}
```

- 文件存在时，整体覆盖该 `plugin_uuid` 在 designdb `prompts` 集合中的文档；
- 文件不存在时，`update_plugin` 保留原有提示词，`install_plugin` 不写入；
- 顶层不是数组、也没有 `prompts` 数组，或 `prompts` 不是数组时：规范要求安装 / 更新失败
  （`400103`），**不得**当作空数组覆盖；当前实现仍会按空数组处理，待同步；
- `[]` 或 `{ "prompts": [] }` 是显式的“清空”语义，与“文件不存在”不同；
- 每个元素必须是对象，`content` 必须是非空字符串；不满足时安装 / 更新失败，
  不得静默丢弃该元素；
- 发布规范要求每个 prompt 的 `content` 去首尾空白后按 UTF-8 编码 ≤ 150 字节
  （对应 50 个汉字，每汉字 3 字节；英文、数字、标点按实际字节计入）；
  安装流程与 `update_prompts` 已使用同一校验；

提示词同步到 kv（仅当 `plugins_type` 命中映射且 `kv_lib` 可用）：

| `plugins_type` | kv 集合 |
| --- | --- |
| `chat_think` | `chat_think` |
| `chat_reply` | `chat_reply` |
| `chat_plugins` | `chat_plugins` |
| `sys_plugins` | `sys_plugins` |
| `plugins` / `plugins_coreldraw` | `chat_plugin` |
| 其他或空 | 不同步；类型切换时清理其他集合 |

- 记录的 `key` 为 `{plugin_uuid}:prompt:{标识}`；标识优先 `id`，其次 `name`，最后为 `content`
  的 SHA-256 前 16 位十六进制；标识中的 `:` `/` `\` 会被替换为 `_`；
- 记录的 `value` 和 `text` 都取 `content`；`rank` 为数组序号 + 1；`meta` 包含
  `plugin_uuid`、`plugin_name`、`kind = "prompt"`；
- 同步只比较 `{plugin_uuid}:prompt:` 前缀下的记录：
  - 内容、`rank`、`meta` 都未变：跳过；
  - 内容变化：写入并重新向量化；
  - 仅 `rank` / `meta` 变化：更新记录，不重新向量化；
  - 新增：写入；
  - 新数组中已不存在的旧 key：删除；
- `prompts.json` 是空数组时，删除该插件在目标 kv 集合中的全部记录；
- `plugins_type` 切换时，扫描已知集合（排除新集合）并清理该插件的记录；新包没有 `prompts.json` 时，
  会使用 designdb 中已有的提示词迁移到新集合；旧类型为空或未知时同样扫描已知集合；
- `kv_lib` 不可用时跳过 kv 同步，本地 prompts 仍正常写入。

## 7. `taskflow.json` 插入规范

支持数组或 `{ "taskflows": [...] }`。所有插件共享 designdb `taskflow` 集合中的唯一文档。

```json
[
  { "uid": "demo-design-layout-check", "name": "版式检查", "desc": "检查栅格与边距是否符合规范" },
  { "uid": "demo-design-export", "name": "导出检查", "desc": "按目标媒介检查导出参数" }
]
```

合并规则：

- 顶层必须是数组或 `{ "taskflows": [...] }`，`taskflows` 必须是数组；否则规范要求
  安装 / 更新失败（`400103`），不得当作空数组；当前实现仍会按空数组处理，待同步；
- `taskflow.json` 不存在时不修改；`[]` / `{ "taskflows": [] }` 是显式清空；
- 每个元素必须是对象；不满足时安装 / 更新失败，不得静默追加；
- 合并键优先 `uid`，没有时用 `name`；
- 已存在同键元素则替换，不存在则追加；
- 元素没有 `plugin_uuid` 时自动补为当前插件 UUID（已有则不覆盖）；
- 没有 `uid` 也没有 `name` 的元素：规范要求拒绝（`400103`），不得直接追加；
  当前实现仍会追加，严格校验规则见
  [2026-11-11-taskflow-keyless-prune.md](../issues/2026-11-11-taskflow-keyless-prune.md)；
- `install_plugin` 只合并不删除；
- `update_plugin` 在 `prune = true` 时删除 `plugin_uuid` 等于当前插件、且键不在新包中的元素；
  其他插件的元素不受影响；没有键的当前插件元素也会被删除；

> 不要用 FFI `update_taskflow` 做单插件增量：它是**整体覆盖**，会替换所有插件的任务流。
> 单插件增量请使用包安装 / 更新流程。

## 8. `mcp.json` 插入规范

查找顺序：包根 `mcp.json` 优先，其次 `mcp/mcp.json`。只读取顶层 `tools` 数组。

安装 / 更新限制：

- 文件不存在：`chat_*` 包允许并保留原能力；`design_*` 包发布时必须提供；
- `{ "tools": [] }`：显式清空该插件能力；
- 顶层不是对象、缺少 `tools`、`tools` 不是数组、tool 不是对象：规范要求安装 / 更新失败
  （`400103`），不得当作空能力；当前实现仍会按空能力处理，待同步；
- tool 的 `name`、`title`、`description` 必须是非空字符串；`inputSchema` /
  `outputSchema` 如果提供，必须是 JSON 对象；不满足时安装 / 更新失败。

> 文件是否存在影响能力是否被同步；发布层面，`design_*` 包**必须**提供 `mcp.json`，
> `chat_*` 包可以没有，完整对照见
> [plugin-spec/package-types.md](./plugin-spec/package-types.md)。

每个 tool 映射为 `plugin_functions` 的一行：

| tool 字段 | 说明 | 缺省 |
| --- | --- | --- |
| `name` | 必须非空；为空时规范要求报错（当前实现仍会跳过该条） | — |
| `uuid` / `id` | 能力主键 | `{plugin_uuid}:{name}` |
| `ffi_symbol` | 实际导出符号 | `name` |
| `title` | 必须非空 | — |
| `description` | 必须非空（当前实现仍允许空字符串） | — |
| `inputSchema` | 如果提供必须是 JSON Schema 对象 | `{"type":"object"}` |
| `inputExample` | 入参示例 | 无 |
| `outputSchema` | 如果提供必须是 JSON Schema 对象 | `{"type":"object"}` |
| `outputExample` | 返回示例 | 无 |
| `use_ai` | 是否参与 AI 召回（0/1） | 继承 `dytes.json.use_ai` |

示例：

```json
{
  "tools": [
    {
      "name": "demo_design_tokens",
      "ffi_symbol": "demo_design_tokens",
      "title": "读取设计令牌",
      "description": "返回当前文档的颜色、字体与间距令牌",
      "inputSchema": {
        "type": "object",
        "properties": { "scope": { "type": "string" } }
      },
      "inputExample": { "scope": "document" },
      "outputSchema": {
        "type": "object",
        "properties": { "tokens": { "type": "object" } }
      },
      "outputExample": { "tokens": { "spacing": [4, 8, 16, 24, 32] } },
      "use_ai": 1
    }
  ]
}
```

写入行为：

- 能力 UUID 已存在时执行全量更新，不存在时新增；
- `install_plugin` 不清理旧能力；
- `update_plugin` 在 `prune = true` 时删除当前插件下、新 `tools` 中已不存在的能力；
- `{ "tools": [] }` 是显式清空；**结构错误**（顶层不是对象、缺少 `tools`、
  `tools` 不是数组、tool 不是对象）规范要求安装 / 更新失败，**不得**按空能力清理；
  当前实现仍会把结构错误当空能力处理，待同步；
- 需要保留旧能力时，不要删除 `mcp.json`（文件不存在会保留原能力），或传 `prune: false`；
  `design_*` 包**不得**删除 `mcp.json`（发布要求），需要保留旧能力时只能传 `prune: false`。

## 9. 安装与更新流程

前置条件：`plugins.db` 已初始化。首次使用需执行 `dytes install sys plugin_lib`，或经 cyctron
执行本仓库的 `setup.json`（建表 / 补列 / 建索引，幂等）。未初始化时，安装会在查询
`plugins` 表时返回 `500201`（`no such table: plugins`）。

### 9.1 `install_plugin`

请求：

```json
{ "dir": "./plugins/demo_design" }
```

流程：

1. 校验 `dir` 是非空、存在的目录；
2. 读取并解析 `dytes.json`，要求是 JSON 对象，`uuid` / `name` 非空；
3. 读取 `version`、`plugins_type`、`target`、`use_ai`；
4. 按 `uuid` 查询已安装插件（`deleted_at IS NULL`）。若已安装且已装版本**不低于**包版本，
   直接返回 `skipped: true`，不再写任何数据；
5. 定位当前平台库文件；失败则终止，此时尚未写插件主表；
6. upsert `plugins` 主表，并写入 `plugin_versions`（旧记录 `is_latest` 置 0）；
7. 依次同步技能、提示词（含 kv）、任务流、MCP 能力；
8. 返回统计。

### 9.2 `update_plugin`

请求：

```json
{
  "dir": "./plugins/demo_design",
  "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  "name": "demo_design",
  "prune": true
}
```

- 查找已安装插件的顺序：请求 `plugin_uuid` → 请求 `name` → `dytes.json.uuid` → `dytes.json.name`；
  都找不到时返回 `400404`；
- 版本不低于当前版本时返回 `skipped: true`；
- `prune` 默认 `true`；传 `false` 可保留包内已移除的技能、任务流元素与能力；
- 更新只作用于已安装插件，不会新增插件。

### 9.3 版本比较

- 去掉前缀 `v`，按 `.` 分段；每段取 `-` 之前的部分；
- 两段都能解析为整数时按数值比较，否则按字符串比较；
- 缺失的段视为空字符串；例如 `1.0.10 > 1.0.9`；`1.0.0` 与 `1.0.0-beta` 视为相等
  （比较时只取 `-` 之前的部分，预发布后缀不参与排序）。

## 10. 返回结构

`install_plugin` 成功：

```json
{
  "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  "name": "demo_design",
  "version": "1.0.0",
  "install_path": "./plugins/demo_design",
  "platform": "win",
  "lib_file": "demo_design.dll",
  "skipped": false,
  "counts": { "skills": 2, "prompts": 2, "taskflows": 2, "functions": 1 }
}
```

- `counts.skills` / `taskflows` / `functions` = 新增 + 更新数量；`counts.prompts` = 写入的条数。

`install_plugin` 跳过：

```json
{
  "plugin_uuid": "…",
  "name": "…",
  "version": "1.0.0",
  "skipped": true,
  "reason": "installed version is not lower than package version"
}
```

`update_plugin` 成功：

```json
{
  "plugin_uuid": "…",
  "name": "demo_design",
  "from_version": "1.0.0",
  "to_version": "1.0.1",
  "install_path": "./plugins/demo_design",
  "platform": "win",
  "lib_file": "demo_design.dll",
  "skipped": false,
  "updated": { "skills": 1, "prompts": 1, "taskflows": 1, "functions": 1 },
  "added": { "skills": 0, "taskflows": 0, "functions": 0 },
  "removed": { "skills": 0, "taskflows": 0, "functions": 0 }
}
```

- `updated.prompts` 是 `0` 或 `1`（表示是否整体重写了提示词文档），不是条数；
- 跳过的 update 也会返回全零的 `updated` / `added` / `removed`。

## 11. 幂等、失败与清理

- 所有写入按主键 upsert，同一包重复安装结果一致；技能 `version` 只在内容实际变化时递增。
- 流程**没有事务**：某一步失败时，之前已写入的数据保留，不回滚；修正输入后重新执行即可。
- 以下情况立即失败且不继续：`dir` 无效、`dytes.json` 缺失/非法、缺少 `uuid`/`name`、
  当前平台库文件缺失（发生在写主表之前）。
- 技能 / 提示词 / 任务流 / 能力的写入失败发生在主表写入之后，此时插件已登记。
- `delete_plugins` 会物理删除 `plugins` 行与该插件的 `plugin_functions`，
  但**不会**删除 designdb 中的技能、提示词、任务流，也不会删除 `plugin_versions` 记录。
  需要彻底清理时由调用方另行处理。

## 12. 包发布前检查清单

- [ ] `dytes.json` 有非空且全局唯一的 `uuid`、`name`、合法的 `version`
- [ ] 需要 kv 同步时写了 `plugins_type`，且取值在映射表中
- [ ] `chat_think` / `chat_reply` 包提供了 `skills/`（至少一个有效技能）
- [ ] `chat_plugins` 包提供了 `taskflow.json`（至少一个任务流元素）
- [ ] `chat_*` 包确认是否需要 `mcp.json`（可以没有）
- [ ] `design_*` 包提供了 `mcp.json`，且 `use_ai = 1` 时提供了 `skills/`
- [ ] `platform-exports` 覆盖所有目标平台，文件名与包内实际库文件一致
- [ ] 库文件名与插件 `name` 匹配（`<name>.dll` 或 `lib<name>.so`），避免选错候选
- [ ] 每个技能文件名全局唯一，且首个 `# ` 标题就是期望的技能名
- [ ] 技能内容不使用模板占位符，长度在 FFI 校验范围内
- [ ] `prompts.json` / `taskflow.json` / `mcp.json` 结构合法；确认空数组的清理含义
- [ ] 升级前确认目标 `prune` 行为，避免误删技能或能力
- [ ] 先执行 `install_plugin`，再用 `get_plugins` / `get_skills` / `get_prompts` / `get_taskflow` /
      `get_functions` 核对落库结果

## 相关文档

- [插件规范](./plugin-spec/README.md)
- [包类型内容规范](./plugin-spec/package-types.md)
- [FFI 接口规范](./ffi-api.md)
- [数据模型与存储规范](./data-model.md)
- [示例插件包](./examples/README.md)
