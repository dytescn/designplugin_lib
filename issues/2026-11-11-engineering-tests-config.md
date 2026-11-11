# [P2/P3] 工程与测试：索引缺失、`tests/` 为空、发布流程不跑测试、kv 配置硬编码

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：
  - `setup.json` 表与索引定义
  - `tests/`（当前为空）
  - `.github/workflows/dytes-release.yml`
  - `src/utils/kv.rs`、`src/utils/designdb.rs`
- 状态：待商议，未修改代码

## 现象

### 索引与约束

- `plugin_functions` 只有 `ffi_symbol` 索引，没有 `plugin_uuid` 索引。
  `get_functions` 按 `plugin_uuid` 查询、`sync_functions` 按 `plugin_uuid` 删除，
  数据量大时会全表扫描。
- `plugins` 有 `target` / `status` / `name` 索引，但没有 `deleted_at` 或
  `(deleted_at, created_at)` 组合索引；`get_plugins` 的默认条件包含
  `deleted_at IS NULL`，列表又按 `created_at DESC` 排序。
- `plugins.name` 没有唯一约束；`get_plugin` 按 `name` 查询取第一条，同名插件行为不确定。
- `plugin_functions` 没有 `(plugin_uuid, ffi_symbol)` 唯一约束，同一插件可声明重复符号。

### 测试

- `tests/` 目录为空；Rust 侧只有 `db.rs`、`cjson.rs`、`plugin.rs` 等少量内联单元测试。
- 安装/更新流程（`src/utils/package.rs`）没有单元测试或集成测试，
  包括本目录 issues 中提到的 JSON 结构错误、prune、kv 同步、64 KiB 等路径。
- `.github/workflows/dytes-release.yml` 只有构建与发布步骤，没有 `cargo test` /
  Deno 测试步骤；发布前不会自动发现回归。
- `ts/` 有 skills / prompts / taskflow 测试，但没有 plugins / functions / install 的测试。

### 配置与路径

- `kv::ensure_collection` 硬编码：

  ```rust
  "dim": 1024,
  "model": "qwen3-embed",
  ```

  如果部署环境的 kv 模型/维度不同，同步可能失败。
- `database_path()` 支持 `PLUGIN_LIB_DATABASE` 覆盖；`designdb.rs` / `kv.rs` 的库路径
  是编译期常量，不支持环境变量，测试和联调不便。
- `designdb::call` / `kv::call` 不检查 `ret`，缓冲区不足时只能通过空响应或解析失败
  间接判断，错误信息不够明确。

## 影响

- 数据量增长后查询/删除变慢；
- 同名插件、重复能力符号可能产生歧义；
- 关键安装/更新流程缺少回归保护，容易再次引入“升级清空数据”类问题；
- 不同环境中 kv 同步可能因模型/维度不一致失败；
- 依赖库路径不可配置，限制测试与部署灵活性。

## 建议（待商议）

1. 在 `setup.json` 增加：
   - `idx_plugin_functions_plugin ON plugin_functions(plugin_uuid)`；
   - `idx_plugins_deleted_created ON plugins(deleted_at, created_at)`（按实际查询调整）；
   - 视产品语义决定是否为 `plugins.name`、`plugin_functions(plugin_uuid, ffi_symbol)`
     增加唯一约束（可能影响存量数据，需要迁移方案）。
2. 为 `package.rs` 增加临时目录集成测试，覆盖：
   - 安装/更新成功；
   - 同版本 skipped；
   - `prune` 行为；
   - JSON 结构非法；
   - kv 不可用/空 prompts；
   - 64 KiB 边界。
3. 在 release workflow 的 `prepare` 或 `publish` 前加入 `cargo test` 与 Deno 测试。
4. kv 的 `dim` / `model` 改为配置项（环境变量或 kv_lib 查询），不要在代码中写死。
5. 为 designdb / kv 增加路径环境变量，与 database 保持一致；`call` 检查 `ret`
   并返回明确的缓冲区不足错误。
6. 明确未使用的表（`plugin_categories`、`plugin_configs`、`plugin_installs`、
   `plugin_relations`、`plugin_libs`）是保留还是可以裁剪，避免维护面扩大。

## 需要确认

- 是否为 `plugins.name` / `plugin_functions(plugin_uuid, ffi_symbol)` 增加唯一约束？
  需要先清理存量重复数据。
- kv 的模型/维度是否固定为 `qwen3-embed` / 1024，还是应由部署配置决定？
- 发布流程是否允许因测试失败而阻塞？当前 workflow 没有测试步骤。

## 验证思路

- 用 `EXPLAIN QUERY PLAN` 检查 `plugin_functions` 按 `plugin_uuid` 查询是否走索引；
- 在空 `tests/` 目录加入一个最小安装/更新测试，确认 CI 能执行；
- 用不同 `dim` / `model` 的 kv_lib 验证同步失败场景；
- 设置环境变量覆盖 designdb / kv 路径，验证是否生效。
