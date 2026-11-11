# 清单与文件布局规范

## 1. 目录布局

```text
<package>/
├── dytes.json                 # 必须
├── <name>.dll | lib<name>.so | lib<name>.dylib   # 必须（MCP-only 包使用占位库）
├── skills/                    # 按类型：chat_think、chat_reply 必须；design_* 且 use_ai=1 必须
│   └── <plugin>_<skill>.md
├── prompts.json               # 可选
├── taskflow.json              # 按类型：chat_plugins 必须
└── mcp.json                   # 按类型：design_* 必须；chat_* 可选；其他可选；也支持 mcp/mcp.json
```

强制要求：

- **必须**在包根提供 `dytes.json`，且能解析为 JSON 对象；
- **必须**提供当前构建平台的库文件；当前安装流程无条件校验库文件，
  MCP-only 包也需要放置占位库（见 [dytes-json.md](./dytes-json.md#61-关于-mcp-only-包)）；
- `skills/` **必须**直接位于包根，只读取其中的 `*.md`，不递归子目录；
- 包类型决定必需内容：`chat_think` / `chat_reply` 必须带 `skills/`，`chat_plugins` 必须带
  `taskflow.json`，`design_*` 必须带 `mcp.json` 且 `use_ai = 1` 时必须带 `skills/`；
  完整对照见 [package-types.md](./package-types.md)；
- 所有 JSON 文件**必须**是 UTF-8、合法 JSON；
- 包**不得**依赖包外的相对路径文件。安装流程只读取包，不复制、不移动文件，
  因此包**必须**已经位于最终安装目录（如 `./plugins/<name>/`）。

禁止放入包内：

- SQLite 数据库、日志、缓存、构建中间产物；
- API Key、令牌、私钥、证书、`.env`；
- 用户数据、聊天记录、带个人信息的截图；
- 指向开发者本机的绝对路径。

## 2. 标识规范

| 标识 | 要求 |
| --- | --- |
| `uuid` | **必须**全局唯一、发布后保持稳定、**不得**复用；推荐 UUID v4 |
| `name` | **必须**非空且稳定；推荐小写 snake_case；**应当**与库文件名匹配 |
| `version` | **必须**使用 `MAJOR.MINOR.PATCH`；每次发布**必须**递增；**不得**覆盖已发布版本 |
| 技能 `_id` | **必须**全局唯一；格式见 [content.md](./content.md) |
| 能力 `uuid` | **必须**全局唯一；推荐 `{plugin_uuid}:{name}` |
| 任务流 `uid` | **必须**在全部插件范围内唯一且稳定 |

- `name` **不应**包含空格、中文、路径分隔符或版本号；
- 发布时 `dytes.json.version` **必须**与 git tag / 发布版本一致；
- **不得**用 `name` 代替 `uuid` 作为跨版本关联键。

## 3. `dytes.json` 字段摘要

完整规范（类型、必填性、缺省值、示例与常见错误）见 [dytes-json.md](./dytes-json.md)。
本节只保留与包布局相关的摘要。

| 字段 | 要求 | 说明 |
| --- | --- | --- |
| `uuid` | 必须 | 插件主键，技能 / 提示词 / 任务流 / 能力的归属键 |
| `name` | 必须 | 插件名，用于库文件匹配与 kv 元数据 |
| `version` | 必须 | 语义化版本；缺省会被当作 `0.0.0` |
| `description` | 应当 | 一句话说明插件能力，避免营销文案 |
| `type` | 应当 | 供 dytes 安装器识别包类型（如 `plugins`、`sys`） |
| `plugins_type` | 条件必须 | 需要提示词同步 kv 时**必须**填写；同时决定包类型内容要求，见 [package-types.md](./package-types.md) |
| `target` | 应当 | 目标软件；缺省 `common` |
| `use_ai` | 应当 | 是否参与 AI 召回；缺省 `0`；`mcp.json` 未声明时继承；`design_*` 包为 `1` 时必须带 `skills/` |
| `platform-exports` | 应当 | 当前平台库文件映射；键为 `windows` / `linux` / `macos`；缺失时回退扫描包根 |

`plugins_type` 与 kv 集合的映射：

| 取值 | kv 集合 |
| --- | --- |
| `chat_think` | `chat_think` |
| `chat_reply` | `chat_reply` |
| `chat_plugins` | `chat_plugins` |
| `sys_plugins` | `sys_plugins` |
| `plugins` / `plugins_coreldraw` | `chat_plugin` |
| 空或其他 | 不同步；类型切换时清理其他集合 |

注意：

- `type` 与 `plugins_type` **不得**混用。安装流程只读取 `plugins_type`；
  写 `"type": "chat_plugins"` 不会触发 kv 同步。
- `plugins_type` **不得**填写未知值并期望同步；需要同步时**必须**使用上表中的值。
- `platform-exports` 的键**必须**是 `windows` / `linux` / `macos`，扩展名**必须**匹配
  `.dll` / `.so` / `.dylib`；值**必须**是包内实际存在的文件名。
- 清单中**不得**写入绝对路径；安装流程只取文件名，但绝对路径会导致包不可移植。

## 4. 平台库文件规范

| 平台 | 推荐文件名 | 扩展名 |
| --- | --- | --- |
| Windows | `<name>.dll` | `.dll` |
| Linux | `lib<name>.so` | `.so` |
| macOS | `lib<name>.dylib` | `.dylib` |

- 包内**应当**只放当前平台的一个库文件，避免多候选导致选择不确定；
- 文件名去扩展名后**应当**等于 `name` 或 `lib<name>`，这是安装流程的优先匹配规则；
- 库文件**必须**来自可复现的 release 构建，**不得**提交调试版或带本机路径的产物；
- 动态库的运行时依赖**必须**随包或随 sys 包提供，**不得**依赖开发机上的绝对路径；
- 安装流程不会加载库文件，但会在安装前校验其存在；缺少时返回 `500300`。

## 5. 版本与升级规范

- 版本比较规则：去掉 `v` 前缀，按 `.` 分段，每段取 `-` 前部分；数字按数值比较。
  `1.0.10 > 1.0.9`；`1.0.0` 与 `1.0.0-beta` 视为相等。
- 已安装版本不低于包版本时，安装 / 更新会返回 `skipped: true` 且不写入任何数据。
  **不得**依赖重复安装来“刷新”数据；需要重写时**必须**提升 `version`。
- `update_plugin` 默认 `prune = true`：
  - 新包缺少 `skills/` 目录时，会删除该插件的全部技能；`chat_think` / `chat_reply`
    升级包以及 `use_ai = 1` 的 `design_*` 升级包**必须**继续提供 `skills/`；
  - `mcp.json` 存在但 `tools` 为空时，会删除该插件的全部能力；
  - 新包缺少 `prompts.json` / `taskflow.json` / `mcp.json` 时，保留原数据；但
    `chat_plugins` 升级包仍**必须**提供 `taskflow.json`，`design_*` 升级包仍**必须**
    提供 `mcp.json`，否则发布不合规。
  升级包**必须**包含完整内容，或显式传 `prune: false`。详细行为见
  [package-types.md](./package-types.md) 第 4 节。
- **不得**在同一个版本号下修改已发布内容；**不得**删除已发布版本目录。

## 6. 禁止项

- 在包内放置数据库、密钥、用户数据、日志；
- 清单中使用绝对路径或指向包外文件；
- 使用其他插件已占用的 `uuid`、技能 `_id`、能力 `uuid`、任务流 `uid`；
- 用 `type` 冒充 `plugins_type`；
- 只放部分平台库文件却在清单中声明全部平台；
- 依赖 `skipped` 行为完成升级；
- 在未确认 `prune` 语义的情况下删除可选文件。

## 相关文档

- [`dytes.json` 规范](./dytes-json.md)
- [包类型内容规范](./package-types.md)
- [字段与安装校验规范](./validation.md)
- [内容规范](./content.md)
- [安全规范](./security.md)
- [发布检查清单](./checklist.md)
- [插件包结构与插入规范](../plugin-package.md)
