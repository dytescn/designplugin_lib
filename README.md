# plugin_lib

`designplugin_lib` 仓库的产品名是 `plugin_lib`：设计插件的**数据接口库**（Rust `cdylib`）。

- 对 `plugin_ui` 提供插件、分类、版本、配置、安装状态、动态库与 FFI 能力的 `get / post / update / patch`；
- 通过 `dlopen` 加载 `./libary/database.dll` 读写 `./database/plugins.db`（不直接链接 SQLite）；
- 不负责设计软件的发现 / 启动 / 运行状态（由 `designsoft` 独立负责）；
- 不提供向量表与语义检索（归 `chatllm`），只提供能力清单与向量化状态。

需求与表设计的权威文档在 `dytes_sys_operations` 仓库：

- `design_plugins/database.md`（表结构、索引、迁移、状态机）
- `feature/007-plugin-ui.md`（接口清单与联调顺序）

## 构建

```bash
cargo build --release
cargo test
cargo clippy --all-targets
```

产物：`target/release/plugin_lib.dll`，由 `update.exe` 安装到 `libary/plugin_lib.dll`。

## 初始化

```bash
deno run -A setup.ts
```

前置：`database` 服务已在 `http://127.0.0.1:44944` 运行。脚本幂等，只初始化 `plugins.db`
（建表 / 补列 / 建索引）；`info.db` 的 `setting` 表由 `setting_ui` 创建，`plugin_lib` 只读写。

- `update.exe` 安装 sys 包后会自动执行 `libary/plugin_lib/setup.ts`，并把云端 `dytes.json`
  与 `setup.ts` 保存到 `libary/plugin_lib/`；
- 也可手动执行 `deno run -A setup.ts`。

> `setup.ts` 只使用顶层 `await fetch` + 字面量参数，以兼容 update 的 oxc 静态提取；
> 不要改成函数封装、循环或条件判断，否则安装器无法提取请求。

## 调用

```text
POST http://127.0.0.1:44944/plugin_lib
Content-Type: application/libary
FFI-Symbol: get_plugins
Body: {"page":1,"page_size":20}
```

响应统一为 `{ "code": 200, "data": {...}, "msg": "success" }`。

## 当前实现范围

- 初始化：`setup.ts` 建表 / 补列 / 建索引（幂等；Rust 侧 `post_migrate` 已移除）
- M1 查询：`get_plugins`、`get_plugin`、`get_categories`、`get_versions`、`get_configs`、`get_install`、`get_libs`、`get_functions`
- M2 新增：`post_plugin`、`post_category`、`post_version`、`post_config`
- M3 更新：`update_plugin`、`update_category`、`update_version`、`update_config`、`patch_plugin`、`patch_install`、`patch_config`、`patch_function`

待实现：`post_install`、`post_uninstall`、`post_delete`、`post_scan`（M4）。

## 配置值约定

- 配置定义（默认值、类型、选项）存 `plugins.db` 的 `plugin_configs`；
- 用户修改值存 `info.db` 的 `setting`（`category = 'plugin'`，`key = 'plugin.config.<plugin_uuid>.<param_key>'`）；该表由 `setting_ui` 创建，`plugin_lib` 只读写、不建表；
- `get_configs` 返回 `value`（用户值优先）与 `source`（`user` / `default`）。
