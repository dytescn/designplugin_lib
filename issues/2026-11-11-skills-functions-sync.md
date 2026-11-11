# [P2] 技能与能力同步：排序不更新、计数不准确、UUID 冲突会接管其他插件能力

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：`src/utils/package.rs`
  - `sync_skills`（约 601 行）
  - `sync_functions`（约 943 行）
  - `read_skill_files`（约 551 行）
- 状态：待商议，未修改代码

## 现象

### 技能

1. `sync_skills` 判断是否需要更新时，只比较 `content`、`name`、`category`：

   ```rust
   let same = doc.get("content") == Some(file.content)
       && doc.get("name") == Some(file.name)
       && doc.get("category") == Some("plugin");
   ```

   `sort` 由文件名排序序号决定。新增/删除技能文件会改变其他技能的 `sort`，
   但已有技能内容不变时不会更新 `sort`，导致 designdb 中的排序与文件名顺序不一致。

2. 新技能直接 `designdb::insert`，不预检 `_id` 是否已被其他插件占用。
   技能 `_id` 全局唯一，冲突会发生在主表写入之后，安装/更新失败并留下部分状态。
   `post_skills` 有冲突检查，但安装流程没有复用。

3. `read_skill_files` 的 `sort` 使用原始遍历索引：

   ```rust
   for (index, path) in paths.iter().enumerate() { ... sort: index as i64 * 10 }
   ```

   空文件 / 空 `id` 被跳过时，后续技能的 `sort` 会出现空档。不是功能错误，但排序值不稳定。

### 能力

1. `sync_functions` 对已存在的能力无条件 `updated += 1` 并调用 `update_function`，
   即使内容完全没变。结果是每次 `update_plugin` 的能力 `updated` 计数都不为 0，
   `updated_at` 也每次刷新，与技能“内容不变不更新”的行为不一致。

2. 能力 `uuid` 在 `plugin_functions` 中全局唯一。`sync_functions` 发现同 `uuid` 已存在时，
   直接调用 `update_function` 并把 `plugin_uuid` 改成当前插件，可能把其他插件的能力
   接管过来。没有校验原归属，也没有冲突错误。

3. `sync_functions` 的 `description` 缺省为空字符串：

   ```rust
   let description = tool.get("description").and_then(Value::as_str).unwrap_or("");
   ```

   随后调用 `plugin_ops::post_function` / `update_function`，而它们要求 `description`
   非空。因此 `mcp.json` 中缺少 `description` 的 tool 会让安装/更新在写主表之后失败，
   且无法按同一版本重试（见 `2026-11-11-install-update-retry.md`）。

4. `post_function` / `update_function` / `patch_function` 没有校验 `params_schema` /
   `return_schema` 必须是 JSON 对象；`patch_function` 可以写入 `null`。
   安装流程也是直接序列化 tool 中的 schema。

## 影响

- 技能排序与文件顺序不一致，`get_skills` 的排序结果不稳定；
- 技能 ID 冲突导致安装半途失败；
- 能力更新计数与 `updated_at` 失去“是否实际变化”的含义；
- 能力 UUID 冲突可能静默转移归属，影响其他插件；
- 缺少 `description` 的 tool 触发延迟失败；
- 非法 schema 写入后，调用方读取时可能得到数组/字符串/null。

## 建议（待商议）

1. `same` 比较加入 `sort`；或每次同步都重算 sort 并仅在变化时更新。
2. 技能插入前按 `_id` 查询归属：不存在则插入；存在且属于其他插件则返回 `400409`；
   存在且属于当前插件则按更新处理。
3. 能力同步先比较现有行内容，只有变化才更新并计数；或明确“每次 update 都刷新”并调整文档。
4. 能力 `uuid` 已存在时校验 `plugin_uuid`：
   - 属于当前插件：更新；
   - 属于其他插件：返回冲突，要求显式迁移；
   - 或者提供明确的“接管”选项。
5. `description` 缺失时使用 `name` / `ffi_symbol` 作为缺省，或在读取 `mcp.json` 时返回
   `400103`；不要等到写主表后才失败。
6. 统一校验 schema 是 JSON 对象，覆盖 post / update / patch 与安装流程。

## 需要确认

- 能力 UUID 冲突是否允许“接管”？如果允许，是否需要审计/告警？
- `mcp.json` 的 tool 缺少 `description` 时，是拒绝发布还是自动生成描述？
- 能力每次 update 都刷新 `updated_at` 是否是预期语义？

## 复现思路

1. 安装两个技能文件，交换文件名顺序后升级，比较 designdb 中 `sort` 是否变化；
2. 用另一个插件已占用的技能 `_id` 安装新包，观察失败位置与残留数据；
3. 安装同一 `mcp.json` 两次（版本递增），检查 `updated.functions` 是否始终大于 0；
4. 在两个插件中使用同一能力 `uuid`，检查后安装的插件是否改变原能力归属；
5. `mcp.json` 中删除某个 tool 的 `description`，执行 update，观察失败与重试行为。
