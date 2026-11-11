# 包类型内容规范

本文按 `plugins_type` 汇总不同包类型在发布时必须提供的内容，并给出 chat 与 design
资源包的详细要求。字段定义见 [dytes-json.md](./dytes-json.md)，目录布局见
[manifest.md](./manifest.md)，内容结构见 [content.md](./content.md)。

判定只看 `plugins_type`，**不**看 `type`。`type` 仍表示 Dytes 包类型（设计插件通常为
`plugins`），**不得**把 `chat_*` / `design_*` 写进 `type`。

## 1. 总表

| `plugins_type` | `skills/` | `prompts.json` | `taskflow.json` | `mcp.json` | 说明 |
| --- | --- | --- | --- | --- | --- |
| `chat_think` | **必须**（≥1） | 可选 | 可选 | 可选 | 见 §2 |
| `chat_reply` | **必须**（≥1） | 可选 | 可选 | 可选 | 见 §2 |
| `chat_plugins` | 可选 | 可选 | **必须**（≥1） | 可选 | 见 §2 |
| `design_*` | `use_ai = 1` 时**必须**（≥1） | 可选 | 可选 | **必须** | 见 §3 |
| `sys_plugins` | 可选 | 可选 | 可选 | 可选 | 无额外要求 |
| `plugins` / `plugins_coreldraw` | 可选 | 可选 | 可选 | 可选 | 兼容旧清单 |
| 空 / 未知 | 可选 | 可选 | 可选 | 可选 | 不同步 kv |

表中约定：

- “必须（≥1）”的 `skills/`：包根至少一个非空 `*.md`，空目录或空文件不算；
- “必须（≥1）”的 `taskflow.json`：顶层数组或 `{ "taskflows": [...] }`，至少一个带
  `uid`（或 `name`）的元素；
- `mcp.json`：包根 `mcp.json` 或 `mcp/mcp.json` 均认可，包根优先；
- 表中的“必须”是**发布要求**。`install_plugin` / `update_plugin` 只校验清单与库文件，
  不会因为缺少 `skills/`、`taskflow.json` 或 `mcp.json` 拒绝安装；
- 当前平台库文件始终**必须**存在，与是否提供 `mcp.json` 无关（缺失返回 `500300`）；
- `plugins_type` 与 kv 集合的映射见 [dytes-json.md](./dytes-json.md) 第 5.1 节。

## 2. chat 资源包

适用于 `plugins_type` 为 `chat_think`、`chat_reply`、`chat_plugins` 的包。

### 2.1 类型与 kv

| `plugins_type` | 定位 | kv 集合 | `skills/` | `taskflow.json` | `mcp.json` |
| --- | --- | --- | --- | --- | --- |
| `chat_think` | 聊天 think 阶段 | `chat_think` | **必须** | 可选 | 可选 |
| `chat_reply` | 聊天 reply 阶段 | `chat_reply` | **必须** | 可选 | 可选 |
| `chat_plugins` | 聊天插件能力 | `chat_plugins` | 可选 | **必须** | 可选 |

- `plugins_type` 同时决定提示词是否同步到 kv；没有 `kv_lib` 时跳过同步，不影响安装；
- 三类 chat 资源包**都允许**没有 `mcp.json`。该豁免只适用于 chat 资源包；`design_*` 包
  必须提供（见 §3）。

### 2.2 `skills/`（`chat_think` / `chat_reply` 必须）

- `skills/` **必须**直接位于包根，至少包含一个非空 `*.md`；
- 每个 `.md` **必须**只描述一个技能，文件去 `.md` 后作为技能 `_id`，**必须**全局唯一；
- 只有空 `skills/` 目录或空文件**不**算满足要求（空文件会被安装流程跳过）；
  文件读取失败会让安装返回文件错误；
- 技能结构、命名、长度与禁用内容见 [content.md](./content.md) 第 2 节。

### 2.3 `taskflow.json`（`chat_plugins` 必须）

- 文件**必须**位于包根，顶层是数组或 `{ "taskflows": [...] }`；
- **必须**至少包含一个任务流元素，元素**必须**带 `uid`（或 `name`）作为合并键；
- 元素**可以**不写 `plugin_uuid`，安装流程会自动补为当前插件；
- 任务流结构与文案要求见 [content.md](./content.md) 第 4 节。

### 2.4 `mcp.json`（可选）

- 三类 chat 资源包**可以**不提供 `mcp.json`；没有文件表示不声明 MCP 能力，
  不属于缺失、错误或包不完整；
- 提供 `mcp.json` 时，内容**必须**满足 [content.md](./content.md) 第 5 节；
- 升级行为与空 `tools` 的清理语义见 §4；需要清空能力时**应当**显式提供空 `tools`，
  不要依赖删除文件。

### 2.5 `prompts.json`（可选）

- 不是 chat 资源包的必需内容，但 `chat_think` / `chat_reply` 通常通过它声明提示词；
  没有该文件时不会写入提示词；
- 顶层**必须**是数组或 `{ "prompts": [...] }`，元素**必须**含稳定 `id`（或 `name`）
  与非空 `content`；
- 写入后按 `plugins_type` 同步到对应 kv 集合（见 [data-model.md](../data-model.md) 第 4 节）；
- 升级时新包没有 `prompts.json` 会保留原提示词，不会清空。

### 2.6 示例

`chat_think` / `chat_reply` 字段相同，仅 `plugins_type` 不同：

```json
{
  "name": "chat_think_design",
  "uuid": "0d6f4a2e-8c31-4f5b-9a27-1b3c5e7d9f01",
  "version": "1.0.0",
  "description": "think 阶段的设计检索词与技能",
  "type": "plugins",
  "plugins_type": "chat_think",
  "target": "common",
  "use_ai": 1,
  "platform-exports": {
    "windows": { "./target/release/chat_think_design.dll": "chat_think_design.dll" },
    "linux": { "./target/release/libchat_think_design.so": "libchat_think_design.so" },
    "macos": { "./target/release/libchat_think_design.dylib": "libchat_think_design.dylib" }
  }
}
```

配套内容：`skills/*.md` 至少一个；`mcp.json` 可省略。

`chat_plugins` 只需把 `plugins_type` 改为 `chat_plugins`，并把配套内容改为
`taskflow.json` 至少一个元素；`skills/` 与 `mcp.json` 均可选。

## 3. design 资源包

适用于 `plugins_type` 以 `design_` 开头的包（具体取值由产品约定，如 `design_plugins`）。

### 3.1 `mcp.json`（必须）

- **必须**在包根提供 `mcp.json`，或提供 `mcp/mcp.json`（包根优先）；
- 文件**必须**是合法 JSON 对象，顶层 `tools` **必须**是数组；
- tool 的字段与文案要求见 [content.md](./content.md) 第 5 节；
- 空 `tools` 数组虽然满足“存在 `mcp.json`”的字面要求，但表示不声明任何能力；
  发布前**必须**确认这是预期行为，升级清理行为见 §4。

### 3.2 `skills/`（`use_ai = 1` 时必须）

- `use_ai` 按 [dytes-json.md](./dytes-json.md) 的规则归一化：`1`、`"1"`、`"true"`、
  `"yes"`、`"on"` 视为 `1`；`0`、`"0"`、`"false"`、`"no"`、`"off"`、无法识别的值视为 `0`；
- `use_ai = 1` 时，`skills/` **必须**直接位于包根，至少包含一个非空 `*.md`；
- 空目录、空文件**不**算满足要求；空文件会被安装流程跳过；
- 每个 `.md` **必须**只描述一个技能，文件去 `.md` 后作为技能 `_id`，**必须**全局唯一；
- `use_ai = 0` 或不写 `use_ai` 时，技能可选。

### 3.3 示例

```json
{
  "name": "design_layout",
  "uuid": "b5e3c7a9-2d41-4c86-9f70-3a8e1d6f2c54",
  "version": "1.0.0",
  "description": "设计软件内的版式与栅格能力",
  "type": "plugins",
  "plugins_type": "design_plugins",
  "target": "common",
  "use_ai": 1,
  "platform-exports": {
    "windows": { "./target/release/design_layout.dll": "design_layout.dll" },
    "linux": { "./target/release/libdesign_layout.so": "libdesign_layout.so" },
    "macos": { "./target/release/libdesign_layout.dylib": "libdesign_layout.dylib" }
  }
}
```

配套内容：`mcp.json` 必须；示例中 `use_ai` 为 `1`，因此 `skills/` 也必须至少一个有效技能。

## 4. 升级与 `prune`

| 新包情况 | 默认行为 | 发布要求 |
| --- | --- | --- |
| `chat_think` / `chat_reply` 缺少 `skills/` | 清理该插件全部技能 | 不合规，**必须**提供 |
| `chat_plugins` 缺少 `taskflow.json` | 保留原任务流元素 | 不合规，**必须**提供 |
| `chat_*` 缺少 `mcp.json` | 保留原能力 | 合规，可以没有 |
| `design_*` 缺少 `mcp.json` | 保留原能力 | 不合规，**必须**提供 |
| 任意类型 `mcp.json` 存在但 `tools` 为空 | 清理该插件全部能力 | 文件合规，能力为空需确认 |
| `design_*` 且 `use_ai = 1` 但缺少 `skills/` | 清理该插件全部技能 | 不合规，**必须**提供 |
| `design_*` 且 `use_ai = 0` 缺少 `skills/` | 清理该插件全部技能 | 合规，可以没有 |
| 缺少 `prompts.json` | 保留原提示词 | 可以没有 |
| 非 `chat_plugins` 类型缺少 `taskflow.json` | 保留原任务流元素 | 可以没有 |

- 升级包**必须**包含仍需要的技能 / 任务流，不要依赖旧数据“自动保留”绕过发布要求；
- `design_*` 包**不得**为保留旧能力而删除 `mcp.json`，需要保留时传 `prune: false`；
- 需要避免清理时**可以**传 `prune: false`，但这不改变发布内容要求。

## 5. 发布校验

先按 §1 总表确认当前 `plugins_type` 的必需内容，再逐项检查：

- [ ] `plugins_type` 正确，且没有把 `chat_*` / `design_*` 写进 `type`；
- [ ] `chat_think` / `chat_reply`：`skills/` 至少一个有效技能；
- [ ] `chat_plugins`：`taskflow.json` 至少一个任务流元素；
- [ ] `chat_*`：`mcp.json` 按需提供，缺失时确认这是预期行为；
- [ ] `design_*`：提供了 `mcp.json`，且 `tools` 是数组；
- [ ] `design_*` 且 `use_ai = 1`：`skills/` 至少一个有效技能；
- [ ] `design_*` 且 `tools` 为空：确认不声明能力及升级清理是预期行为；
- [ ] 当前平台库文件存在，`platform-exports` 与实际文件一致；
- [ ] `uuid` / `name` / `version` 符合 [dytes-json.md](./dytes-json.md)；
- [ ] 升级包包含完整内容，并确认 `prune` 的影响。

> 这些内容要求由发布检查保证。安装 / 更新流程只校验清单与库文件，缺少技能、
> 任务流或 MCP 能力时只会让对应数据为空或保留旧值。

## 6. 常见错误

| 写法 | 结果 |
| --- | --- |
| `"type": "chat_think"`，没有 `plugins_type` | 不触发 kv 同步，也不按 chat 资源包处理 |
| `"type": "design_plugins"`，没有 `plugins_type` | 不按 `design_*` 处理，也不触发对应规则 |
| `chat_think` / `chat_reply` 没有 `skills/` | 安装成功但无技能；发布不合规 |
| `chat_plugins` 没有 `taskflow.json` | 安装成功但无任务流；发布不合规 |
| `chat_*` 没有 `mcp.json` | 合规；不声明 MCP 能力 |
| `design_*` 没有 `mcp.json` | 安装成功但无能力；发布不合规 |
| `mcp.json` 写成 `{ "tools": {} }` | 结构非法，能力读取为空 |
| `design_*` 包 `use_ai: 1` 但没有 `skills/` | 安装成功但无技能；发布不合规 |
| `design_*` 包 `use_ai: "true"` 但没有 `skills/` | 归一化为 `1`，仍要求技能 |
| `design_*` 包 `use_ai` 缺失且没有 `skills/` | 归一化为 `0`，技能可选 |
| `skills/` 只有空目录或空文件 | 视为没有有效技能；发布不合规 |
| `taskflow.json` 是空数组 | 视为没有任务流元素；发布不合规 |
| `chat_*` 为清空能力而删除 `mcp.json` | 不会清空，旧能力保留；应提供空 `tools` |
| 忘记放当前平台库文件 | 安装失败 `500300`，与 `mcp.json` 无关 |

## 相关文档

- [`dytes.json` 规范](./dytes-json.md)
- [清单与文件布局规范](./manifest.md)
- [字段与安装校验规范](./validation.md)
- [内容规范](./content.md)
- [发布检查清单](./checklist.md)
- [插件包结构与插入规范](../plugin-package.md)
- [数据模型与存储规范](../data-model.md)
