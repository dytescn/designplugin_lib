# 插件规范

本目录规定插件作者**应当如何**组织与发布插件包。它回答“怎么做才算合规”；
[插件包结构与插入规范](../plugin-package.md) 回答“系统实际如何读取和落库”。
两者配合使用：规范条款高于实现细节；如发现规范与代码行为不一致，以代码为准并修正规范或代码。

## 规范文件

| 文件 | 内容 |
| --- | --- |
| [dytes-json.md](./dytes-json.md) | `dytes.json` 字段类型、必填性、缺省值、示例与常见错误 |
| [manifest.md](./manifest.md) | 目录布局、标识、版本、平台库文件；`dytes.json` 摘要 |
| [package-types.md](./package-types.md) | 按 `plugins_type` 汇总内容要求：`chat_*`、`design_*` 与通用类型 |
| [validation.md](./validation.md) | 字段格式、长度、枚举、唯一性与安装校验的统一严格规则 |
| [content.md](./content.md) | 技能、提示词、任务流、MCP 能力的内容与标识规范 |
| [security.md](./security.md) | 路径、权限、提示词安全、隐私与供应链 |
| [checklist.md](./checklist.md) | 发布前逐项检查清单与验证命令 |

建议阅读顺序：`dytes-json.md` → `manifest.md` → `package-types.md` → `validation.md` →
`content.md` → `security.md` → `checklist.md`。

## 规范等级

本文使用以下关键词：

| 关键词 | 含义 |
| --- | --- |
| **必须** / **不得** | 强制要求；违反即视为不合规，不应发布 |
| **应当** / **不应** | 推荐要求；确有必要偏离时，需要说明理由 |
| **可以** | 可选做法 |

## 适用范围

- 设计插件包（包含当前平台动态库）；
- MCP-only 资源包（只提供 `mcp.json` 能力；当前安装流程仍要求当前平台库文件存在，
  需要放置占位库，见 [dytes-json.md](./dytes-json.md#61-关于-mcp-only-包)）；
- chat 资源包（`plugins_type` 为 `chat_think` / `chat_reply` / `chat_plugins`）：
  可以没有 `mcp.json`；`chat_think` / `chat_reply` 必须带 `skills/`，
  `chat_plugins` 必须带 `taskflow.json`；
- design 资源包（`plugins_type` 以 `design_` 开头）：必须带 `mcp.json`；
  `design_*` 且 `use_ai` 为 `1` 时必须带 `skills/`；
- 通过 `install_plugin` / `update_plugin` 安装或升级的包。

各类型的完整内容要求与通用类型对照见 [package-types.md](./package-types.md)。

以下内容不在本规范范围内：`plugin_lib` 自身作为 sys 包的发布清单、设计软件的发现与启动、
向量化与语义检索实现。

## 与现有文档的关系

| 文档 | 定位 |
| --- | --- |
| [plugin-package.md](../plugin-package.md) | 字段映射、安装 / 更新流程、`prune` 与返回结构的实现说明 |
| [ffi-api.md](../ffi-api.md) | 调用约定、符号清单、错误码 |
| [data-model.md](../data-model.md) | SQLite / designdb / kv 的存储结构 |
| [examples/](../examples/README.md) | 可直接运行的示例包与调用脚本 |
| 本目录 | 面向插件作者的强制 / 推荐条款与发布检查清单 |

## 规范版本

- 规范版本：`1.0`
- 对应代码版本：`plugin_lib` 1.0.20
- 最后核对日期：2026-11-11
- 变更规则：新增或收紧条款时，同步更新 `checklist.md` 与相关示例；涉及 FFI 或表结构的变更，
  按 `skills/dev.md` 先由人工确认。
