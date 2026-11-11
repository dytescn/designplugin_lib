# 内容规范

本文规定技能、提示词、任务流与 MCP 能力的标识、结构与文案要求。
字段如何落库见 [plugin-package.md](../plugin-package.md)。

## 1. 通用标识

- 所有 ID **必须**稳定、全局唯一，发布后**不得**随文案修改；
- ID **必须**使用小写字母、数字、下划线或连字符（`[a-z0-9_-]+`），
  **不得**使用中文、空格、斜杠、冒号或点号；
- ID **应当**带插件名前缀，降低跨插件冲突；
- 同一插件内的名称**不应**重复；不同插件之间的名称可以重复，但 ID 不可以；
- 文案**应当**使用简体中文，术语在同一插件内保持一致。

## 2. 技能（`skills/*.md`）

`plugins_type` 为 `chat_think` 或 `chat_reply` 的包，`skills/` 是**必需内容**：
发布时**必须**至少包含一个有效技能文件。其他类型默认可以没有技能；例外是
`design_*` 包在 `use_ai = 1` 时**必须**带技能。完整对照见
[package-types.md](./package-types.md)。

### 2.1 文件与标识

- 文件名**必须**全局唯一，推荐 `<plugin_name>_<skill_name>.md`；
- 文件名去 `.md` 后就是技能 `_id`，**不得**依赖安装流程做去重或改写；
- 每个文件**必须**只有一个技能，**不得**在一个文件里放多个技能；
- 文件**必须**非空；空文件会被跳过，并可能在 `update` 时导致旧技能被清理。

### 2.2 结构

```markdown
# 技能名称

一句话说明技能解决什么问题。

## 触发条件

用户可能说出的词或场景。

## 输入

需要用户或调用方提供的信息。

## 执行步骤

1. 可执行的步骤。
2. 可执行的步骤。

## 输出要求

输出的结构、单位、边界与失败处理。
```

- 第一个 `# ` 标题**必须**是技能名，安装时写入 `name`；
- **不得**写 YAML frontmatter；安装流程不解析 frontmatter，它会原样进入 `content`；
- 正文**应当**包含：适用场景、触发条件、输入、步骤、输出、边界；
- 步骤**必须**可执行，**不得**只写“根据情况处理”这类空泛描述。

### 2.3 限制

| 字段 | 限制 | 说明 |
| --- | --- | --- |
| `name` | ≤ 120 字符 | 来自首个 `# ` 标题 |
| `description` | ≤ 500 字符 | 安装时为空，后续可通过 FFI 补写 |
| `content` | ≤ 1 MiB（1,048,576 字节，按 UTF-8 编码后的文件大小） | 发布检查按此规范；当前 FFI 实现仍为 8000 字符，待同步 |
| 控制字符 | 不得包含 | 允许 `\n`、`\r`、`\t` |

- `skills/*.md` 文件（含标题行）按 UTF-8 编码后**必须** ≤ 1 MiB；
- 超过 1 MiB 的技能文件**不得**发布，安装后也不应通过 FFI 更新为更大内容；
- 正文**不得**包含 `{{base_system_prompt}}`、`{{categories}}`、`{{skills}}`、`{{rules}}`
  四个模板占位符；
- 正文**不得**包含密钥、令牌、个人信息或本机绝对路径；
- 需要 `trigger`、`params_schema`、`description` 等结构化字段时，安装后**必须**通过
  `patch_skills` 补写，**不得**在 Markdown 中自造字段。

## 3. 提示词（`prompts.json`）

- 顶层**必须**是数组，或 `{ "prompts": [...] }`；`prompts` 字段**必须**是数组；
- 每个元素**必须**是 JSON 对象；
- 每个元素**必须**至少包含：
  - `id` 或 `name`：稳定标识，用于 kv key；
  - `content`：非空提示词正文。
- 安装 / 更新限制：
  - `prompts.json` **不存在**：保留原提示词，不写入；
  - `[]` 或 `{ "prompts": [] }`：显式清空，属于合法输入；
  - 顶层不是数组、`prompts` 不是数组、元素不是对象、`content` 不是非空字符串：
    **必须**返回错误（`400103`），**不得**当作空数组或空内容继续；
  - 不允许用 `{ "prompts": null }`、`{ "prompts": {} }`、`{}` 等结构“清空”数据；
    要清空必须显式写空数组。
- 推荐字段：`category`、`sort`、`is_enabled`；
- 每个 prompt 的 `content` 去首尾空白后按 UTF-8 编码**必须** ≤ 150 字节
  （对应 50 个汉字，每汉字 3 字节；英文、数字、标点按实际字节计入）；
- 超过 150 字节的提示词**不得**发布；安装、`update_prompts` 与 kv 同步前会校验；
- `id` **必须**稳定：修改文案时**不得**更换 `id`，否则 kv 会视为“删除旧记录 + 新增记录”；
- `content` **不得**包含模板占位符、控制字符、密钥或用户数据；
- 提示词**不得**要求模型忽略系统指令、泄露系统提示、绕过权限或执行任意命令；
- 提示词**不得**依赖“部分更新”：`update_prompts` 与安装流程都是整体覆盖，
  每次**必须**提供完整数组。

示例：

```json
{
  "prompts": [
    {
      "id": "layout-advisor",
      "name": "版式顾问",
      "content": "根据画布尺寸和目标媒介给出栅格、边距与视觉层级建议；参数必须带单位。",
      "category": "design",
      "sort": 10,
      "is_enabled": 1
    }
  ]
}
```

## 4. 任务流（`taskflow.json`）

`plugins_type` 为 `chat_plugins` 的包，`taskflow.json` 是**必需内容**：
发布时**必须**至少包含一个任务流元素。其他类型可以没有任务流。完整对照见
[package-types.md](./package-types.md)。

- 顶层**必须**是数组，或 `{ "taskflows": [...] }`；`taskflows` 字段**必须**是数组；
- 每个元素**必须**是 JSON 对象；
- 每个元素**必须**包含 `uid` 或 `name`，作为跨版本合并键；
- 安装 / 更新限制：
  - `taskflow.json` **不存在**：保留原任务流，不修改；
  - `[]` 或 `{ "taskflows": [] }`：显式清空当前插件的任务流元素，属于合法输入；
  - 顶层不是数组、`taskflows` 不是数组、元素不是对象：**必须**返回错误（`400103`），
    **不得**当作空数组继续；
  - 不允许用 `{ "taskflows": null }`、`{ "taskflows": {} }`、`{}` 等结构“清空”；
  - `uid` / `name` 的缺失、重复、跨插件冲突等严格校验规则见
    [2026-11-11-taskflow-keyless-prune.md](../../issues/2026-11-11-taskflow-keyless-prune.md)；
- `uid` **应当**全局唯一并带插件前缀（如 `<plugin_name>-<flow>`）；
- 元素**可以**包含 `plugin_uuid`；安装流程会在缺失时自动补为当前插件；
- **不得**依赖 FFI `update_taskflow` 做单插件增量：它是全局整体覆盖；
- 升级时**必须**保留仍需要的元素，或显式使用 `prune: false`；
- 元素**不得**包含用户数据、密钥或绝对路径。

示例：

```json
[
  {
    "uid": "demo-design-layout-check",
    "name": "版式检查",
    "desc": "检查栅格、边距与对齐是否符合当前项目规范"
  }
]
```

## 5. MCP 能力（`mcp.json`）

`mcp.json` 是否必需取决于 `plugins_type`：

- `design_*` 包**必须**提供；
- `chat_think` / `chat_reply` / `chat_plugins` 包**可以**不提供，没有文件表示
  不声明 MCP 能力，不属于缺失或不合规；
- 其他类型可选。

完整对照见 [package-types.md](./package-types.md)。

安装 / 更新限制：

- 文件不存在：`chat_*` 包允许；`design_*` 包发布时必须提供。文件不存在时保留原有能力，
  不按“空能力”处理；
- `{ "tools": [] }`：显式清空该插件能力，属于合法输入；
- 顶层不是对象、缺少 `tools`、`tools` 不是数组、tool 不是对象：
  **必须**返回错误（`400103`），**不得**当作空能力继续；
- `name`、`title`、`description` 必须是非空字符串；`inputSchema` / `outputSchema`
  如果提供，必须是 JSON 对象；
- 不允许用 `{ "tools": null }`、`{ "tools": {} }`、`{}` 等结构“清空”能力；
  要清空必须显式写 `{ "tools": [] }`。

提供该文件时，每个 tool **必须**满足：

| 字段 | 要求 |
| --- | --- |
| `name` | 必须非空、稳定；作为工具名与默认 `ffi_symbol` |
| `uuid` / `id` | 建议显式提供，全局唯一；缺省为 `{plugin_uuid}:{name}` |
| `ffi_symbol` | 有对应实现时**必须**与实际导出符号一致 |
| `title` | 必须非空，简洁描述能力 |
| `description` | 必须非空，说明输入、输出与副作用 |
| `inputSchema` | 建议提供；必须是 JSON Schema 对象 |
| `outputSchema` | 建议提供；必须是 JSON Schema 对象 |
| `inputExample` / `outputExample` | 建议提供；**必须**与对应 schema 一致 |
| `use_ai` | 建议显式声明 0/1；缺省继承插件 `use_ai` |

规范要求：

- 一个 tool **必须**只做一件事，**不得**把多个动作塞进一个工具；
- 有文件、网络、命令执行等副作用时，`description` **必须**明确写出；
- schema **必须**是对象，**不得**用数组或字符串代替；
- 示例**不得**包含真实密钥、真实用户数据或生产环境地址；
- 能力 `uuid` 发布后**不得**变更；重命名工具时**应当**保留原 `uuid` 或明确迁移；
- 没有实际实现的能力**不得**声明 `ffi_symbol`；MCP-only 资源包可以只提供描述性能力。

示例：

```json
{
  "tools": [
    {
      "name": "demo_design_tokens",
      "ffi_symbol": "demo_design_tokens",
      "title": "读取设计令牌",
      "description": "返回当前文档的颜色、字体与间距令牌，不修改文档。",
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

## 6. 文案质量

- 标题**应当**是名词短语，能独立说明用途；
- 描述**应当**说明“做什么、何时用、输出什么”，避免“强大”“智能”等空泛词；
- 示例**必须**可复制执行，路径**必须**使用相对路径或占位符；
- 同一概念**必须**使用同一术语，避免中英文混用造成歧义；
- **不得**在文案中承诺未实现的能力。

## 相关文档

- [清单与文件布局规范](./manifest.md)
- [包类型内容规范](./package-types.md)
- [字段与安装校验规范](./validation.md)
- [安全规范](./security.md)
- [发布检查清单](./checklist.md)
