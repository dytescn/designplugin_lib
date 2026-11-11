# plugin_lib 文档

本目录面向两类读者：

- **插件作者**：按规范组织插件包，让 `install_plugin` / `update_plugin` 能把数据正确写入
  `plugins.db`、designdb 与 kv；
- **调用方（`plugin_ui` 等）**：按 FFI 约定调用查询、新增、更新、删除接口。

文档内容以当前仓库代码为准（`plugin_lib` 1.0.20）。`README.md` 中的早期里程碑说明可能落后于实现，
出现冲突时按本目录和 `src/ffi/*.rs` 修正。

## 文档地图

| 文档 | 内容 | 读者 |
| --- | --- | --- |
| [plugin-package.md](./plugin-package.md) | 插件包目录、`dytes.json`、技能 / 提示词 / 任务流 / MCP 能力的插入规则 | 插件作者 |
| [ffi-api.md](./ffi-api.md) | FFI 调用约定、22 个导出符号、请求 / 响应、错误码 | 调用方 |
| [data-model.md](./data-model.md) | SQLite 表、designdb 集合、kv 同步与幂等规则 | 插件作者、后端 |
| [plugin-spec/](./plugin-spec/README.md) | 插件规范：清单、包类型、内容、安全、发布检查清单 | 插件作者 |
| [examples/](./examples/README.md) | 可直接复制的示例插件包与调用脚本 | 所有人 |

建议阅读顺序：插件作者先看 `plugin-spec/` 与 `plugin-package.md`，再看 `examples/`；
发布前按 `plugin-spec/checklist.md` 逐项检查。调用方先看 `ffi-api.md`，
需要理解落库位置时看 `data-model.md`。

## 文档写作规范

1. **事实来源优先级**
   `src/ffi/*.rs`、`src/utils/*.rs`、`setup.json` > `skills/*.md` > 根 `README.md`。
   文档描述的行为必须能在代码中找到对应实现；无法确认的内容写“待确认”，不要猜测。
2. **文件与标题**
   文件名使用小写 kebab-case（如 `plugin-package.md`），一级标题使用中文短语。
   每篇文档至少包含“目的 / 适用范围”，涉及流程时给出步骤，涉及字段时给出表格。
3. **示例**
   示例必须可以直接复制执行。路径统一写成相对仓库根或明确说明运行目录，
   不写开发者本机绝对路径，不写真实密钥、令牌或生产数据。
4. **字段与代码**
   JSON 字段名、FFI 符号名、表名、集合名保持英文原样，不做翻译。
   涉及默认值时必须写出缺省值，避免“可选”掩盖实际行为。
5. **变更同步**
   修改 FFI 符号、`setup.json` 表结构、`dytes.json` 读取逻辑或安装流程时，同步更新本目录对应文档与示例。
   按 `skills/dev.md`，FFI 方法名与数据库结构的调整必须先由人工确认，不能只改文档。
6. **示例数据**
   示例 UUID、技能 ID、能力 UUID 必须可辨识为演示数据，且不要与真实插件冲突。
   技能 `_id` 在 designdb 中全局唯一，示例统一带 `demo_` 前缀。

## 核心概念

| 术语 | 含义 |
| --- | --- |
| 插件包 | 一个目录，至少包含 `dytes.json` 与当前平台动态库，可选技能、提示词、任务流、MCP 能力 |
| 安装（install） | 读取插件包并写入插件主表、技能、提示词、任务流、能力；可新增插件 |
| 更新（update） | 只处理已安装插件；版本更高时覆盖数据，并按 `prune` 清理包内已移除的条目 |
| 插入（insert） | 泛指把数据写入 SQLite / designdb；包安装是批量插入，`post_*` 是单条插入 |
| 技能（skill） | designdb `skills` 集合中的一条文档，通过 `plugin_uuid` 归属插件 |
| 提示词（prompt） | designdb `prompts` 集合中每个插件一条文档，`prompts` 字段是数组 |
| 任务流（taskflow） | designdb `taskflow` 集合的唯一文档，`taskflows` 字段是所有插件共享的数组 |
| 能力（func） | SQLite `plugin_functions` 表中的一行，描述一个可调用的 FFI 能力 |
| designdb | `libdesigndb` 动态库提供的对象文档存储；本项目使用 `skills`、`prompts`、`taskflow` 三个集合 |
| kv | `kv_lib` 提供的检索记录存储；提示词可按 `plugins_type` 同步过去 |

## 维护信息

- 代码版本：`plugin_lib` 1.0.20（`Cargo.toml` / `dytes.json`）
- 最后核对日期：2026-11-11
- 构建与测试命令见根 `README.md` 与 `skills/testenv.md`。
