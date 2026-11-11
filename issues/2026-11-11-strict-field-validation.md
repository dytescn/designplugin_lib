# [P1] 统一严格字段校验：非法输入直接报错，不用空值代替

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 状态：规范草案已建立，代码待实施
- 统一规范：[docs/plugin-spec/validation.md](../docs/plugin-spec/validation.md)
- 覆盖 issue：字段结构、字段格式、唯一性、跨插件归属、长度限制、安装前置校验

## 目标

把 issues 中分散的“字段随便写也能过”的问题统一成一套规则：

1. 已知字段严格校验类型、格式、枚举、长度、唯一性和归属；
2. 结构错误、字段非法直接返回错误，不用空数组 / 空对象 / 默认值代替；
3. 区分“文件不存在”“显式空”“结构错误”；
4. 安装 / 更新先校验、后写入，避免部分安装；
5. 包安装与 FFI 直接写入使用同一套字段规则。

## 统一规则摘要

| 对象 | 关键限制 |
| --- | --- |
| `dytes.json` | `uuid` 必须 UUID；`name` 建议 `[a-z0-9_-]+`；`version` 必须语义化；`plugins_type` 必须命中映射或 `design_*`；`platform-exports` 结构严格，目标文件必须是文件 |
| `skills/*.md` | 文件 id `[a-z0-9_-]+`；content ≤ 1 MiB；name ≤ 120；description ≤ 500；全局唯一；控制字符 / 模板占位符禁用 |
| `prompts.json` | 顶层/元素严格；content 非空且 ≤ 150 字节（50 汉字）；id/name 合并键；重复键冲突 |
| `taskflow.json` | 顶层/元素严格；必须有 `uid` 或 `name`；重复键、跨插件冲突报错；`plugin_uuid` 必须匹配当前插件 |
| `mcp.json` | 顶层必须是对象；`tools` 必须是数组；tool 必须有非空 `name` / `title` / `description`；schema 必须是对象；uuid/name 不重复 |
| FFI 写入 | 与包安装使用同一套字段规则；`plugin_uuid` 必须指向已安装插件；能力/技能不得静默接管其他插件 |

完整字段表见 [validation.md](../docs/plugin-spec/validation.md)。

## 与现有 issue 的对应关系

| issue | 统一规范章节 | 处理状态 |
| --- | --- | --- |
| `2026-11-11-package-json-structure.md` | §5 / §6 / §7 | 文档已更新，代码待同步 |
| `2026-11-11-taskflow-keyless-prune.md` | §6 | 讨论中 |
| `2026-11-11-manifest-locate-uuid.md` | §3 | 待实施 |
| `2026-11-11-skills-functions-sync.md` | §4 / §7 | 待实施 |
| `2026-11-11-ffi-ts-semantics.md` | §8 | 待实施 |
| `2026-11-11-content-limits.md` | §4 / §5 | prompt 已实现，skill 1 MiB 待实施 |
| `2026-11-11-install-update-retry.md` | §1 先校验后写入 | 待实施 |
| `2026-11-11-flag-use-ai.md` | §3 / §7 | 已处理（文件已删除） |
| `2026-11-11-kv-sync.md` | 非字段校验 | 已处理 |
| `2026-11-11-designdb-64k.md` | 缓冲区 / 结构性问题 | 未全部修复 |
| `2026-11-11-engineering-tests-config.md` | 测试与基础设施 | 待实施 |

## 建议实施顺序

1. **校验层与错误模型**：定义 `validate_*` 函数、错误码、错误路径格式。
2. **安装前置校验**：在写主表、designdb、kv 之前读取并校验所有包文件。
3. **`dytes.json`**：UUID、name、version、plugins_type、platform-exports。
4. **skills**：1 MiB、id/name/description、全局唯一、控制字符/模板。
5. **prompts**：结构、元素、150 字节、重复键（当前已有 150 字节校验）。
6. **taskflow**：结构、key、重复、plugin_uuid、跨插件冲突。
7. **mcp**：结构、tool 必填字段、schema、uuid/name 冲突。
8. **FFI 写入路径**：复用同一套校验；插件归属检查。
9. **测试**：结构错误、缺失文件、显式空、重复键、跨插件冲突、超长字段。

## 待确认

- 未知扩展字段：允许忽略，还是严格模式全部拒绝？
- `plugins_type` 未知值：严格模式是否直接报错？
- `name` / `uid` / tool `name` 的字符集和长度上限。
- 存量数据（超长、无 key 任务流、超 150 字节 prompt、未知字段）如何兼容或迁移。
- 是否提供 `strict: false` 兼容开关。

## 约束

本 issue 只记录统一规范和实施计划；除已有改动外，本轮不直接改代码。
