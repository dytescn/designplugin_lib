# `dytes.json` 规范

`dytes.json` 是插件包的清单文件，位于包根目录。本文规定字段类型、必填性、缺省值、
与 `plugin_lib` 安装流程的关系，以及常见错误。目录布局与库文件规则见
[manifest.md](./manifest.md)。

## 1. 基本要求

- 文件**必须**位于包根，文件名固定为 `dytes.json`；
- 内容**必须**是 UTF-8 编码、合法 JSON，顶层**必须**是对象；
- `uuid`、`name` **必须**是非空字符串；缺失或类型错误时安装直接失败（`400103`）；
- 清单中**不得**出现绝对路径、密钥、令牌、个人信息；
- 允许存在未列出的扩展字段，但 `plugin_lib` 不会读取它们，**不得**依赖其生效。

## 2. 字段分层

`dytes.json` 同时服务于 Dytes 安装器和 `plugin_lib`。两者读取的字段不同：

| 使用方 | 读取的字段 |
| --- | --- |
| `plugin_lib` 的 `install_plugin` / `update_plugin` | `uuid`、`name`、`version`、`description`、`plugins_type`、`target`、`use_ai`、`platform-exports` |
| Dytes 安装器 | `type`、`platform-exports`、`init`、`dependencies-sys`、`checksums` 等 |
| 两者都不读取 | `keywords`、`author`、`license` 等展示 / 元数据字段 |

- **不得**把 Dytes 安装器的 `type` 当作 `plugin_lib` 的 `plugins_type`；
- 需要提示词同步 kv 时，**必须**显式写 `plugins_type`；
- 设计插件的 `type` **应当**为 `plugins`；`sys` 包不在本规范范围。

## 3. 字段总表

| 字段 | 类型 | 必填 | `plugin_lib` 读取 | 缺省 / 说明 |
| --- | --- | --- | --- | --- |
| `name` | string | 是 | 是 | 无缺省；写入 `plugins.name`，并参与库文件匹配 |
| `uuid` | string | 是 | 是 | 无缺省；写入 `plugins.uuid`，作为所有关联数据的主键 |
| `version` | string | 是 | 是 | 缺失或非字符串按 `0.0.0`；语义化版本 |
| `description` | string | 应当 | 是 | 非字符串按空字符串；写入 `plugins.desc` |
| `type` | string | 应当 | 否 | Dytes 包类型，设计插件用 `plugins`，sys 包用 `sys` |
| `plugins_type` | string | 条件 | 是 | 空字符串；需要 kv 同步时必须是映射表中的值；`chat_*` / `design_*` 取值决定必需的包内容，见 5.2 |
| `target` | string | 应当 | 是 | 空字符串按 `common`；写入 `plugins.target` |
| `use_ai` | bool / number / string | 应当 | 是 | 无法识别时按 `0`；`mcp.json` 能力未声明时继承；`design_*` 包为 `1` 时必须带 `skills/` |
| `platform-exports` | object | 应当 | 是 | 平台 → 库文件映射；缺失时回退扫描包根一层；找不到当前平台库文件时 `500300` |
| `keywords` | array | 可选 | 否 | 展示 / 搜索元数据 |
| `author` | string | 可选 | 否 | 作者或组织 |
| `license` | string | 可选 | 否 | 许可证标识 |
| `init` | array | 可选 | 否 | Dytes 安装后执行的初始化脚本（sys 包常用） |
| `dependencies-sys` | object | 可选 | 否 | Dytes 的 sys 依赖声明 |
| `checksums` | object | 可选 | 否 | 发布工具生成的校验信息；不得手工篡改 |

## 4. 标识字段

### 4.1 `uuid`

- **必须**是非空字符串，**必须**符合 UUID 格式，推荐 v4；
- **必须**全局唯一，发布后保持稳定，**不得**复用其他插件的 UUID；
- **不得**用 `name`、目录名或版本号代替；
- 重复安装 / 更新时依靠它匹配已安装插件；修改 UUID 会被视为新插件。

### 4.2 `name`

- **必须**是非空字符串，**应当**使用小写 snake_case（`[a-z0-9_-]+`）；
- **应当**与库文件名匹配：
  - Windows：`<name>.dll`
  - Linux：`lib<name>.so`
  - macOS：`lib<name>.dylib`
- **不得**包含空格、中文、`/`、`\`、`..` 或版本号；
- 名称变更会导致按 `name` 查找已安装插件的流程失败；升级时**应当**保持稳定。

### 4.3 `version`

- **必须**是非空字符串，格式为 `MAJOR.MINOR.PATCH`；
- 每次发布**必须**递增；已安装版本不低于包版本时安装会返回 `skipped: true`；
- `v` 前缀会在比较时被去掉，但规范**不推荐**使用；
- `-beta`、`-rc` 等后缀在版本比较时被忽略；`1.0.0` 与 `1.0.0-beta` 视为相等；
- **不得**使用数字类型（如 `1`），否则会按 `0.0.0` 处理；
- **不得**使用空字符串；发布版本**必须**与 git tag / 发布记录一致。

## 5. 分类与行为字段

### 5.1 `type` 与 `plugins_type`

| 字段 | 用途 | 示例 |
| --- | --- | --- |
| `type` | Dytes 安装器识别包类型 | `plugins`、`sys` |
| `plugins_type` | `plugin_lib` 写入 `plugins.plugins_type`，决定 kv 同步集合 | `chat_plugins`、`sys_plugins` |

- **不得**用 `type` 代替 `plugins_type`；
- `plugins_type` 为空或未知值时不会同步 kv，但插件、技能、提示词、任务流仍会正常写入；
  如果是从其他类型切换过来，已知 kv 集合中该插件的旧记录会被清理；
- 需要 kv 同步时，取值**必须**是：

| `plugins_type` | kv 集合 |
| --- | --- |
| `chat_think` | `chat_think` |
| `chat_reply` | `chat_reply` |
| `chat_plugins` | `chat_plugins` |
| `sys_plugins` | `sys_plugins` |
| `plugins` / `plugins_coreldraw` | `chat_plugin` |

### 5.2 按 `plugins_type` 的包内容要求

发布时按 `plugins_type` 判定必需内容（`prompts.json` 对所有类型都是可选）：

| `plugins_type` | `skills/` | `taskflow.json` | `mcp.json` |
| --- | --- | --- | --- |
| `chat_think` | **必须**（≥1） | 可选 | 可选 |
| `chat_reply` | **必须**（≥1） | 可选 | 可选 |
| `chat_plugins` | 可选 | **必须**（≥1） | 可选 |
| `design_*` | `use_ai = 1` 时**必须**（≥1） | 可选 | **必须** |

其他类型（`sys_plugins`、`plugins`、`plugins_coreldraw`、空 / 未知）没有额外内容要求。

- “必须（≥1）”：包根至少一个非空 `*.md`，或至少一个带 `uid`（或 `name`）的任务流元素；
- `mcp.json`：包根 `mcp.json` 或 `mcp/mcp.json` 均认可，包根优先；
- 表中的“必须”是**发布要求**。`install_plugin` / `update_plugin` 只校验清单与库文件，
  不会因为缺少 `skills/`、`taskflow.json` 或 `mcp.json` 拒绝安装。

完整总表（含 `prompts.json`）、示例、升级与 `prune`、常见错误见
[package-types.md](./package-types.md)。

### 5.3 `target`

- **必须**是字符串；空字符串或缺失时按 `common`；
- 用于标识目标设计软件；具体取值由产品约定，**应当**与插件实际支持范围一致；
- **不得**用 `target` 表示运行平台（运行平台由 `platform-exports` 与当前系统决定）。

### 5.4 `use_ai`

- 推荐使用 `true` / `false` 或 `1` / `0`；
- 只有 `1`（或 `true`、`"1"`、`"true"`、`"yes"`、`"on"`、数值 `1.0`）视为开启；
- 其他已提供的值，包括 `0`、`false`、`"0"`、`"false"`、`"no"`、`"off"`、`2`、`-1`、
  浮点数、无法识别的字符串、数组 / 对象，均按 `0` 处理；
- `dytes.json.use_ai` 缺失或显式 `null` 时按 `0`；部分更新接口中的显式 `null`
  表示不修改该字段；
- `mcp.json` 中单个 tool 缺少 `use_ai` 或为 `null` 时继承插件值；
- `design_*` 包中该值归一化为 `1` 时，发布包**必须**带 `skills/`（见 5.2 节）；
- 涉及隐私数据时**不应**开启。

## 6. `platform-exports`

推荐结构：

```json
{
  "platform-exports": {
    "windows": {
      "./target/release/demo_design.dll": "demo_design.dll"
    },
    "linux": {
      "./target/release/libdemo_design.so": "libdemo_design.so"
    },
    "macos": {
      "./target/release/libdemo_design.dylib": "libdemo_design.dylib"
    }
  }
}
```

规则：

- 键**必须**是 `windows`、`linux`、`macos`，**不得**写 `win`、`mac`、`osx`；
- 每个值**必须**是对象；对象的键是源路径，值是包内目标文件名；
- 安装时只取**文件名**：值以当前扩展名结尾时用值，否则键以扩展名结尾时用键；
- 目标文件**必须**在包根实际存在；安装不复制文件；
- 多个候选时，优先文件名（去扩展名）等于 `name` 或 `lib<name>` 的文件，否则取第一个；
- `platform-exports` 缺失或当前平台没有候选时，回退扫描**包根一层**的库文件；
- 找不到当前平台的库文件时安装失败（`500300`），且不会写入插件主表；
- 包内**不得**使用绝对路径；**不得**只声明部分平台却声称支持全部平台。

### 6.1 关于 MCP-only 包

当前 `install_plugin` / `update_plugin` **无条件**定位当前平台库文件。因此即使包只提供
`mcp.json` 能力，也**必须**在包根放置当前平台的库文件（可以是占位文件），否则返回 `500300`。
安装流程不会加载该文件，但会校验其存在。

需要区分“没有 `mcp.json`”和“没有库文件”：

- `mcp.json` 是否必需取决于 `plugins_type`：`design_*` 包**必须**提供；
  `chat_think` / `chat_reply` / `chat_plugins` 包**可以**不提供；其他类型可选。
  完整对照见 [package-types.md](./package-types.md)；
- 没有 `mcp.json` 不影响安装，也不代表包损坏；缺少库文件才会导致 `500300`。

> 如果产品要求真正的“无库安装”，需要先修改实现并同步更新本文档；
> 在此之前不要把“MCP-only 可以没有库文件”作为发布依据。

## 7. 完整示例

使用 `chat_plugins` 分类的设计插件（配套内容必须包含 `taskflow.json`）：

```json
{
  "name": "demo_design",
  "uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  "version": "1.0.0",
  "description": "根据画布尺寸给出栅格、边距与导出建议",
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

最小字段集（仍须满足库文件要求）：

```json
{
  "name": "demo_design",
  "uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  "version": "1.0.0",
  "platform-exports": {
    "windows": { "./target/release/demo_design.dll": "demo_design.dll" }
  }
}
```

sys 包示例（仅供对照，不在本规范范围）：

```json
{
  "name": "plugin_lib",
  "uuid": "8b1c9907-692d-473e-8676-459aec1ec6ba",
  "version": "1.0.20",
  "description": "design plugin data interface library",
  "type": "sys",
  "platform-exports": {
    "windows": { "./target/release/plugin_lib.dll": "plugin_lib.dll" }
  },
  "init": ["./setup.json"],
  "dependencies-sys": {
    "database": "1.0.4",
    "designdb": "1.0.2",
    "kv_lib": "0.1.4"
  }
}
```

## 8. 常见错误

| 写法 | 实际结果 |
| --- | --- |
| `uuid` 缺失、为空或为数字 | `400103`，安装终止 |
| `name` 缺失、为空或为数字 | `400103`，安装终止 |
| `version` 写成数字 `1` | 按 `0.0.0` 处理，可能导致版本比较异常 |
| `version` 为空字符串 | 写入空版本，版本记录与比较不可靠 |
| 只写 `"type": "chat_plugins"` | 不触发 kv 同步；必须写 `plugins_type` |
| `plugins_type` 拼写错误 | 不同步 kv；插件数据仍写入 |
| `platform-exports` 键写成 `win` | 当前平台找不到候选，回退扫描包根 |
| `platform-exports` 值为目录路径 | 只取文件名；包根不存在时 `500300` |
| 包根有多个同扩展名库文件 | 优先 `name` / `lib<name>`，否则取第一个，可能选错 |
| `version` 未递增 | `skipped: true`，不写入任何数据 |
| 清单含未知字段 | 安装忽略，不会报错，也不会生效 |
| 清单含绝对路径 | 安装可能仍能工作，但包不可移植，规范禁止 |

包类型内容相关的错误（缺少技能 / 任务流 / MCP 能力）见
[package-types.md](./package-types.md) 第 6 节。

## 9. 校验清单

- [ ] 文件是 UTF-8、合法 JSON，顶层是对象
- [ ] `uuid` 是非空字符串、全局唯一、发布后未变更
- [ ] `name` 是非空字符串，与库文件名匹配
- [ ] `version` 是非空字符串、语义化、高于已安装版本、与发布版本一致
- [ ] `type` 为 `plugins`（或 sys 包对应的 `sys`）
- [ ] 需要 kv 同步时 `plugins_type` 命中映射表
- [ ] 按 `plugins_type` 确认包类型内容要求，符合 [package-types.md](./package-types.md)
- [ ] `target` 与插件实际支持范围一致
- [ ] `use_ai` 为明确的布尔值或 `0` / `1`
- [ ] `platform-exports` 键为 `windows` / `linux` / `macos`，每个声明的平台都有实际库文件
- [ ] 没有绝对路径、密钥、令牌或个人信息

## 相关文档

- [清单与文件布局规范](./manifest.md)
- [包类型内容规范](./package-types.md)
- [字段与安装校验规范](./validation.md)
- [内容规范](./content.md)
- [安全规范](./security.md)
- [发布检查清单](./checklist.md)
- [插件包结构与插入规范](../plugin-package.md)
