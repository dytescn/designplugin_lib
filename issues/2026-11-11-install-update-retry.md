# [P1] 安装/更新失败后无法重试，版本记录可能丢失 `is_latest`

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：`src/utils/package.rs`
  - `install`：`upsert_plugin`（约 77 行）在 `sync_*`（90–95 行）之前
  - `update`：`upsert_plugin`（约 174 行）在 `sync_*`（187–192 行）之前
  - `record_version`（约 522 行）
- 状态：待商议，未修改代码

## 现象

1. `install` / `update` 都会先调用 `upsert_plugin`，把 `plugins.version` 更新为新包版本；
   之后才同步技能、提示词、任务流、能力。
2. 如果后续任一步骤失败（例如 `prompts.json` 结构错误、`mcp.json` 的 tool 缺少
   `description`、kv 写入失败、designdb 超 64 KiB），主表版本已经是新版本。
3. 再次执行同一个包时，`installed_plugin` / `find_installed` 读到的新版本不低于包版本，
   直接返回 `skipped: true`，不会重新同步失败的数据。调用方无法通过重试修复。
4. `record_version` 在 `install` / `update` 中都写作 `let _ = record_version(...)`，错误被忽略。
   它先执行 `UPDATE plugin_versions SET is_latest = 0`，再 `INSERT` 新版本；
   如果 `INSERT` 因 `UNIQUE(plugin_uuid, version)` 冲突失败，所有旧版本的 `is_latest`
   已被清零，最终没有 latest 版本记录。
5. 相关小问题：`update` 的 `updated.prompts` 在 `prompts.json` 为空数组时返回 `0`，
   但实际已整体覆盖为空；返回值无法区分“没有文件”和“写入空数组”。

## 影响

- 安装/更新失败后无法通过同一版本重试，只能手工改版本号或清理数据；
- `plugin_versions` 的 latest 状态可能损坏；
- 调用方根据 `updated.prompts` 判断是否写入时，空数组场景会误判；
- 与文档“任一步骤失败后修正输入即可重试”的描述不一致。

## 建议（待商议）

1. 把 `plugins.version` 的更新和 `record_version` 移到所有 `sync_*` 成功之后；
   或者失败时回滚主表版本字段。
2. `record_version` 返回错误并处理，不要 `let _ =`；先插入新版本，再清零其他 `is_latest`，
   或在事务内完成。
3. 增加显式的 `force` / `retry` 参数，允许同版本强制重跑同步。
4. `updated.prompts` 改为布尔或 `Option` 语义，区分“未提供文件”和“提供空数组”。

## 需要确认

- 是否允许同一版本强制重跑？这会改变当前 `skipped` 契约。
- 是否引入 SQLite 事务？当前实现明确“无事务、失败保留已完成步骤”，需要权衡兼容性。
- `plugin_versions` 的写入失败是否应让安装整体失败？

## 复现思路

1. 准备一个通过 `dytes.json` / 库文件校验、但 `prompts.json` 结构错误（如 `{"prompts": {}}`）
   的包；
2. 执行 `install_plugin` 或 `update_plugin`，确认返回错误；
3. 修正 `prompts.json` 后再次执行同一版本，确认返回 `skipped: true`；
4. 检查 `plugin_versions` 中该插件的 `is_latest` 是否全部为 0。
