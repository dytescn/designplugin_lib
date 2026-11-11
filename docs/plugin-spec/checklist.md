# 发布检查清单

发布前逐项确认。未通过的项目**必须**修复或记录豁免理由。
字段细节见 [dytes-json.md](./dytes-json.md)，包类型要求见 [package-types.md](./package-types.md)，
字段与安装的严格校验见 [validation.md](./validation.md)，安装行为见
[plugin-package.md](../plugin-package.md)。

## 1. 文件与清单

- [ ] 包根有 `dytes.json`，是合法 JSON 对象
- [ ] `dytes.json` 的字段类型、缺省值与平台映射符合 [dytes-json.md](./dytes-json.md)
- [ ] 当前平台库文件存在，扩展名与平台匹配
- [ ] `platform-exports` 覆盖所有声明平台，值为包内实际文件名
- [ ] 包内没有数据库、日志、缓存、密钥、私钥、用户数据
- [ ] 包已位于最终安装目录，安装后不需要再复制文件
- [ ] 所有 JSON 为 UTF-8 且合法
- [ ] `skills/` 只含一层 `.md`

## 2. 包类型内容

按 [package-types.md](./package-types.md) 的总表确认当前 `plugins_type` 的必需内容：

- [ ] `chat_think` / `chat_reply`：`skills/` 至少一个有效技能文件
- [ ] `chat_plugins`：`taskflow.json` 至少一个任务流元素
- [ ] `chat_*`：已确认是否需要 `mcp.json`（可以没有，不属于缺失）
- [ ] `design_*`：提供了 `mcp.json`（或 `mcp/mcp.json`），且 `tools` 是数组
- [ ] `design_*` 且 `use_ai = 1`：`skills/` 至少一个有效技能文件

## 3. 标识与版本

- [ ] `uuid` 全局唯一，且与上一版本一致（升级场景）
- [ ] `name` 稳定，库文件名与其匹配（`<name>.dll` / `lib<name>.so`）
- [ ] `version` 符合 `MAJOR.MINOR.PATCH`，高于已安装版本
- [ ] `version` 与 git tag / 发布版本一致
- [ ] 需要 kv 同步时填写了合法的 `plugins_type`
- [ ] 没有把 `type` 当作 `plugins_type`
- [ ] 没有复用已发布版本号或覆盖已发布文件

## 4. 技能

- [ ] 每个 `.md` 只有一个技能，文件名全局唯一且带插件前缀
- [ ] 首个 `# ` 标题就是期望的技能名
- [ ] 没有 YAML frontmatter
- [ ] 正文包含触发条件、输入、步骤、输出、边界
- [ ] `name` ≤ 120、`description` ≤ 500、`content` ≤ 1 MiB（1,048,576 字节，UTF-8）
- [ ] 不含控制字符与四个模板占位符
- [ ] 需要 `trigger` / `params_schema` 时已计划通过 `patch_skills` 补写

## 5. 提示词

- [ ] 顶层是数组或 `{ "prompts": [...] }`，`prompts` 是数组
- [ ] 每个元素是对象，含稳定 `id`（或 `name`）与非空 `content`
- [ ] 每个 prompt 的 `content` 去首尾空白后按 UTF-8 编码 ≤ 150 字节（对应 50 个汉字）
- [ ] 结构非法时安装/更新会报错，不会按空数组或空内容处理
- [ ] 已区分“文件不存在（保留）”与“空数组（清空）”
- [ ] 文案修改没有更换 `id`
- [ ] 没有越权指令、密钥、用户数据或模板占位符
- [ ] 已确认整体覆盖语义，数组内容是完整的

## 6. 任务流

- [ ] 顶层是数组或 `{ "taskflows": [...] }`，`taskflows` 是数组
- [ ] 每个元素是对象，含全局唯一且稳定的 `uid`（或 `name`）
- [ ] 结构非法时安装/更新会报错，不会按空数组处理
- [ ] 已区分“文件不存在（保留）”与“空数组（清空当前插件元素）”
- [ ] 已确认 `prune` 行为，升级包包含仍需要的元素
- [ ] 没有使用 FFI `update_taskflow` 做单插件增量

## 7. MCP 能力

- [ ] 顶层是对象，`tools` 是数组
- [ ] 缺少 `tools` / `tools` 非数组 / tool 非对象时安装/更新会报错，不会按空能力处理
- [ ] 每个 tool 是对象，有非空 `name`、`title`、`description`
- [ ] `uuid` 全局唯一且发布后不变；缺省值符合 `{plugin_uuid}:{name}`
- [ ] `ffi_symbol` 与实际导出符号一致
- [ ] `inputSchema` / `outputSchema` 如果提供，是 JSON Schema 对象
- [ ] 示例与 schema 一致，不含真实数据
- [ ] 一个 tool 只做一件事；副作用已在描述中说明
- [ ] `use_ai` 取值明确；空 `tools` 的清理行为已确认

## 8. 安全

- [ ] `dir` 不通过 `..` 越出插件根目录
- [ ] 公共插件没有设置 `is_system = 1`
- [ ] `mask` 遵循最小权限；提权有说明
- [ ] 没有下载并执行未校验的代码或库
- [ ] 依赖版本已声明，checksums 校验通过
- [ ] 触及安全评审条件时已完成评审

## 9. 验证

- [ ] `cargo test` 通过
- [ ] 在隔离环境执行一次 `install_plugin`
- [ ] `get_plugins` 能查到插件，`installed = 1`
- [ ] `get_skills`（摘要）数量、`_id`、`plugin_uuid` 正确；`get_skill` 能按 `id` 读取完整内容
- [ ] `get_prompts` 返回完整数组
- [ ] `get_taskflow` 中当前插件元素带 `plugin_uuid`，未覆盖其他插件
- [ ] `get_functions` 的 `ffi_symbol`、schema 正确
- [ ] 需要 kv 时确认 `{plugin_uuid}:prompt:` 前缀记录正确
- [ ] 升级场景确认 `prune` 没有误删数据

## 10. 评审记录

| 项目 | 内容 |
| --- | --- |
| 插件名 / UUID | |
| 版本 | |
| 评审日期 | |
| 评审人 | |
| 结论 | 通过 / 有条件通过 / 不通过 |
| 豁免项与理由 | |
| 备注 | |

## 相关文档

- [插件规范总览](./README.md)
- [清单与文件布局规范](./manifest.md)
- [包类型内容规范](./package-types.md)
- [内容规范](./content.md)
- [安全规范](./security.md)
