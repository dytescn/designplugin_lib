# [P2/P3] FFI 语义与 TS 封装：软删插件可被修改、`updateSkill` 可能更新错误的 id

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：
  - `src/utils/plugin.rs` `require_plugin`（约 128 行）、`delete_plugin`（约 537 行）
  - `src/utils/plugin.rs` `post_skill` / `post_function` / `update_function`
  - `ts/ffi.ts` `updateSkill` / `patchSkill`（约 119–125 行）、`postSkill` / `importSkills`
- 状态：待商议，未修改代码

## 现象

### 插件软删语义

- 表里有 `deleted_at`，`get_plugins` / `get_plugin` 查询会过滤 `deleted_at IS NULL`；
- 但 `require_plugin` 没有过滤：

  ```rust
  SELECT * FROM plugins WHERE uuid = {} LIMIT 1
  ```

  因此 `update_plugins` / `patch_plugins` 可以修改已被软删的插件；
- `delete_plugin` 按 `name` / `uuid` 查询时也没有过滤 `deleted_at`；
- 当前 `delete_plugins` 是物理删除，所以“软删”只可能来自其他模块直接写表，
  语义不统一。

### FFI 数据归属

- `post_skill` / `post_function` 只校验 `plugin_uuid` 非空，不校验对应的 `plugins` 行存在，
  可以创建孤儿技能/能力；
- `update_function` 可以直接把 `plugin_uuid` 改成另一个值，没有校验目标插件是否存在，
  也没有校验原归属；可能跨插件移动能力；
- `delete_function` 只按 `uuid` 删除，不校验插件归属（文档已说明）。

### TS 封装

- `updateSkill` / `patchSkill`：

  ```ts
  return unwrap<Skill>("update_skills", { id, ...patch });
  ```

  `patch` 类型是 `Partial<Skill>`，允许包含 `id`；如果调用方传入 `patch.id`，
  会覆盖参数 `id`，导致更新到另一个技能。`patchSkill` 同理。
- `postSkill(skill: Skill)` 的 `category` 在 TS 接口里是可选的，但 Rust `PostSkillReq`
  要求 `category` 必填；类型检查通过、运行时 `400103`。
- `importSkills` 对每个技能先 `getSkills` 再 `update`/`post`，逐条往返，性能较差且非原子；
  中途失败会留下部分导入。
- `ts/ffi.ts` 只封装了 skills / prompts / taskflow，没有 plugins / functions /
  install_plugin / update_plugin 的封装，前端需要自行拼装。

## 影响

- 已软删插件仍可被更新，可能出现“查询不到但能改”的状态；
- 孤儿技能/能力无法通过插件查询到，清理困难；
- 能力可能被跨插件移动，影响归属与权限；
- TS 调用方可能误更新其他技能，且编译器不会提醒；
- 批量导入失败后状态不完整。

## 建议（待商议）

1. `require_plugin` / `delete_plugin` 统一加 `deleted_at IS NULL`，并明确软删/物理删除的
   唯一入口；如果软删不打算使用，考虑移除 `deleted_at` 过滤或补软删接口。
2. `post_skill` / `post_function` 校验 `plugin_uuid` 对应插件存在且未删除；
   `update_function` 校验目标 `plugin_uuid` 存在，并明确是否允许迁移。
3. TS 的 `updateSkill` / `patchSkill` 参数改为 `Omit<Partial<Skill>, "id">` 或在函数内
   剔除 `id`；`postSkill` 的 `category` 改为必填，或提供默认值。
4. `importSkills` 考虑批量接口或明确“逐条导入、失败不回滚”；至少返回失败条目。
5. 如前端需要，补充 plugins / functions / install 的 TS 封装与类型。

## 需要确认

- `deleted_at` 是保留字段还是实际软删机制？当前代码只过滤、不写入。
- 能力是否允许跨插件迁移？如果允许，是否需要审计字段？
- 技能/能力是否必须依附已安装插件？如果需要，FFI 校验应统一。

## 复现思路

1. 直接通过 SQL 把某插件 `deleted_at` 设为非空，再调用 `update_plugins` / `patch_plugins`，
   确认仍可更新；
2. `post_skills` 传一个不存在的 `plugin_uuid`，确认创建成功；
3. 在两个插件间用同一能力 `uuid` 调用 `update_function`，确认 `plugin_uuid` 被改；
4. TS 调用 `updateSkill("a", { id: "b", title: "x" })`，确认更新的是 `b`。
