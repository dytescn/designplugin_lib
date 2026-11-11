# [P1] `prompts.json` / `taskflow.json` / `mcp.json` 结构错误被静默当作空内容

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：`src/utils/package.rs`
  - `read_prompts`（约 677 行）
  - `read_taskflows`（约 834 行）
  - `read_tools`（约 925 行）
- 状态：文档已更新，代码待同步

## 规范更新（2026-11-11，仅文档）

已更新 `docs/plugin-spec/content.md`、`docs/plugin-spec/checklist.md`、
`docs/plugin-package.md`，新增以下安装限制：

| 文件 | 文件不存在 | 显式空 | 结构 / 字段非法 |
| --- | --- | --- | --- |
| `prompts.json` | 保留原提示词 | `[]` / `{ "prompts": [] }` 清空 | 返回 `400103`，不得按空数组 |
| `taskflow.json` | 保留原任务流 | `[]` / `{ "taskflows": [] }` 清空当前插件元素 | 返回 `400103`，不得按空数组 |
| `mcp.json` | `chat_*` 允许并保留原能力；`design_*` 发布必须提供 | `{ "tools": [] }` 清空 | 返回 `400103`，不得按空能力 |

补充规则：

- `prompts` / `taskflows` 字段存在时**必须**是数组；`null`、对象、字符串都视为结构错误。
- 每个 prompt / taskflow 元素**必须**是对象；prompt 的 `content` **必须**是非空字符串。
- `mcp.json` 顶层**必须**是对象，`tools` **必须**是数组；tool **必须**是对象，
  `name` / `title` / `description` **必须**非空，schema 如果提供**必须**是对象。
- 结构错误不得退化为空数组 / 空能力；要清空必须显式写空数组。
- 建议在写主表、designdb、kv 之前完成结构校验，避免部分安装。
- taskflow 的 `uid` / `name`、重复键、跨插件归属等严格规则仍需按
  `2026-11-11-taskflow-keyless-prune.md` 继续讨论。

### 代码同步待办

1. `read_prompts` / `read_taskflows` / `read_tools` 区分“文件不存在”和“结构非法”。
2. 结构非法返回 `400103`，不进入覆盖 / prune 分支。
3. 在写主表、designdb、kv 之前完成校验。
4. `mcp.json` 的 tool 必填字段与 schema 类型校验。
5. 补充结构错误、空数组、缺失文件的测试。

## 现象

三个读取函数在 JSON 能解析、但顶层结构不符合预期时，都会退化为空数组：

```rust
// read_prompts
let list = value.as_array().cloned()
    .or_else(|| value.get("prompts").and_then(Value::as_array).cloned())
    .unwrap_or_default();   // 结构错误 => []
```

`read_taskflows`、`read_tools` 同理。随后：

- `sync_prompts` 会用空数组整体覆盖 designdb 的提示词文档；
- `sync_taskflows` 在 `prune = true`（默认）时会删除当前插件全部任务流元素；
- `sync_functions` 在 `prune = true` 时会删除当前插件全部能力。

即：写错字段名、把数组写成对象、`tools` 写成对象等，都不会报解析错误，而是按“空内容”
执行清理。安装/更新流程只会在主表写入之后才走到这些读取，进一步放大问题。

## 影响

- 升级包中的一次笔误可能清空线上提示词、任务流或 MCP 能力；
- 调用方看到的是成功或后续错误，难以定位是结构错误；
- 与文档“顶层必须是数组或 `{...}`”的强制要求不一致，代码没有拒绝非法结构。

## 原始建议（仅供对照）

1. 区分“文件不存在”（返回 `None`）和“文件存在但结构非法”（返回 `400103`）。
2. 对 `prompts.json` / `taskflow.json` / `mcp.json` 校验顶层结构，非法时中止更新，
   而不是按空数组继续。
3. 如果产品希望兼容旧格式，至少返回明确警告或 `counts` 标记，避免静默清空。
4. `mcp.json` 的 `tools` 不是数组时，建议报错；当前文档已说明结构错误会清空能力，
   但这是高风险行为。

## 已确认 / 待确认

已确认（已写入规范）：

- 结构非法统一返回 `400103`，不得按空数组 / 空能力处理；
- `{ "prompts": null }`、`{ "prompts": {} }`、`{ "taskflows": null }`、
  `{ "tools": null }`、`{}` 等都是结构错误；
- 只有显式空数组（`[]`、`{ "prompts": [] }`、`{ "taskflows": [] }`、
  `{ "tools": [] }`）才表示清空；
- 文件不存在表示保留原数据；
- `mcp.json` 缺少 `tools` 或 `tools` 非数组是错误，不是“没有能力”。

待确认：

- 是否需要在安装/更新写主表之前完成全部结构校验（避免部分安装）；
- 是否保留对旧格式的兼容窗口或 warnings 字段；
- taskflow 的 key/归属严格规则仍需按 `2026-11-11-taskflow-keyless-prune.md` 讨论。

## 复现思路

1. `prompts.json` 写入 `{"prompts": {}}`，执行 `update_plugin`，观察 designdb 提示词被清空；
2. `taskflow.json` 写入 `{"taskflows": {}}`，执行 `update_plugin`，观察当前插件任务流元素被删除；
3. `mcp.json` 写入 `{"tools": {}}`，执行 `update_plugin`，观察能力被删除。
