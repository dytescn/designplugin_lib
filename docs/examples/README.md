# 示例

本目录提供一套可直接复制、改造成自己插件的示例，以及 FFI 调用脚本。
示例中的 UUID、技能 ID、能力 UUID 都是演示数据，不代表真实插件。

## 目录结构

```text
examples/
├── README.md
├── demo-plugin/                 # 最小插件包
│   ├── dytes.json               # 清单：uuid / name / version / platform-exports
│   ├── skills/
│   │   ├── demo_design_layout.md
│   │   └── demo_design_export.md
│   ├── prompts.json             # 提示词数组
│   ├── taskflow.json            # 任务流数组
│   └── mcp.json                 # MCP 能力定义
└── ffi/
    ├── call.ts                  # Deno FFI：安装并核对结果
    └── requests.md              # cyctron HTTP 调用示例
```

## 1. 使用前提

### 1.1 构建并准备依赖库

```bash
cargo build --release
```

把构建产物与依赖库放进本地测试目录：

```text
.dytes/libary/
├── plugin_lib.dll        # 或 libplugin_lib.so / libplugin_lib.dylib
├── database.dll          # 或 database.so / database.dylib
├── libdesigndb.dll       # 或 libdesigndb.so / libdesigndb.dylib
└── kv_lib.dll            # 可选，缺失时跳过 kv 同步
```

依赖库文件名以实际安装的 `database`、`designdb`、`kv_lib` 为准。

### 1.2 初始化数据库

`install_plugin` 会直接读写 `plugins` 表，首次运行前必须先初始化 `plugins.db`：

```bash
dytes install sys plugin_lib
```

或经 cyctron 执行本仓库的 `setup.json`（建表 / 补列 / 建索引，幂等）。
未初始化时会返回 `500201`，错误信息类似 `no such table: plugins`。

### 1.3 放入示例库文件

示例包不包含二进制。安装流程会校验当前平台的库文件是否存在，因此需要先放入一个
占位或真实的动态库，并与 `dytes.json` 的文件名一致：

| 平台 | 放到 `demo-plugin/` 的文件 |
| --- | --- |
| Windows | `demo_design.dll` |
| Linux | `libdemo_design.so` |
| macOS | `libdemo_design.dylib` |

本地只想验证数据写入时，可以把任意一个同扩展名的动态库复制过去并改名；
真实插件应放入自己的构建产物。安装流程不会加载该库，只记录文件名。

## 2. 运行 Deno 示例

`plugin_lib` 通过相对路径 `./libary/*` 加载依赖，因此**工作目录必须是 `.dytes`**：

```bash
cd .dytes
deno run --allow-ffi --allow-read --allow-write ../docs/examples/ffi/call.ts
```

脚本默认安装 `../docs/examples/demo-plugin`，也可以显式传入目录：

```bash
deno run --allow-ffi --allow-read --allow-write ../docs/examples/ffi/call.ts ../docs/examples/demo-plugin
```

预期输出（字段顺序可能不同）：

```text
安装插件包: ../docs/examples/demo-plugin
install_plugin: {
  plugin_uuid: "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
  name: "demo_design",
  version: "1.0.0",
  install_path: "../docs/examples/demo-plugin",
  platform: "win",
  lib_file: "demo_design.dll",
  skipped: false,
  counts: { skills: 2, prompts: 2, taskflows: 2, functions: 1 }
}
plugin: { uuid: "...", name: "demo_design", version: "1.0.0", installed: 1, ... }
skills: { total: 2, list: [ ... ] }
functions: { total: 1, list: [ ... ] }
prompts: { plugin_uuid: "...", count: 2, names: [ "版式顾问", "导出顾问" ] }
taskflow: { count: 2, items: [ { uid: "demo-design-layout-check", ... } ] }
```

再次运行同一版本会返回 `skipped: true`，这是版本比较的预期行为。
要重新写入数据，先升级 `dytes.json` 的 `version`，或换一个 `uuid`。

> 示例的 `plugins_type` 是 `chat_plugins`，安装时会尝试同步提示词到 kv。
> 按 chat 资源包发布要求，`chat_plugins` 必须带 `taskflow.json`（示例已提供至少一个元素）；
> `mcp.json` 是可选的，示例提供它只是为了演示能力声明。规范见
> [插件规范](../plugin-spec/package-types.md)。
> 如果本机没有配置 kv_lib 的向量模型，可能看到 `gguf_init_from_file` /
> `llama_model_load` 日志；插件数据仍会写入，kv 同步结果取决于 kv_lib 配置。

## 3. 运行 HTTP 示例

先确保 cyctron 与 database 服务已启动（`http://127.0.0.1:44944`），然后按
[ffi/requests.md](./ffi/requests.md) 中的 curl 命令调用。注意把 `dir` 改为实际的插件包路径；
cyctron 进程的工作目录决定相对路径的基准。

## 4. 改造成自己的插件

按顺序修改：

1. `dytes.json`
   - `uuid`：换成自己的唯一 UUID；
   - `name`：换成插件名，并让库文件名与其匹配（`<name>.dll` 或 `lib<name>.so`）；
   - `version`：从 `1.0.0` 开始，升级时递增；
   - `plugins_type`：需要提示词同步 kv 时填写映射表中的值，否则留空；
   - `platform-exports`：改成自己的库文件路径与文件名。
2. `skills/*.md`
   - 文件名改成全局唯一，建议 `<插件名>_<技能名>.md`；
   - 首个 `# ` 标题就是写入后的技能名；
   - 不要写 YAML frontmatter；需要 `trigger` / `params_schema` 时，安装后调用 `patch_skills`。
3. `prompts.json` / `taskflow.json` / `mcp.json`
   - 按 [插件包结构与插入规范](../plugin-package.md) 的字段映射调整；
   - 注意空数组在 `update_plugin` 中的清理含义。
4. 放入自己的动态库，然后执行 `install_plugin`。

发布前按 [插件规范](../plugin-spec/README.md) 与
[发布检查清单](../plugin-spec/checklist.md) 逐项核对。

## 5. 常见问题

| 现象 | 排查 |
| --- | --- |
| `install_plugin` 返回 `500300` | `dir` 不是目录、缺少 `dytes.json`，或当前平台库文件不存在 |
| 返回 `400103`，提示缺少 `uuid` / `name` | `dytes.json` 顶层字段缺失或为空字符串 |
| 返回 `skipped: true` | 已安装版本不低于包版本；升级 `version` 或更换 `uuid` |
| `500201` 加载依赖失败 | 运行目录不是包含 `libary/` 的安装根目录，或缺少 database / designdb 库 |
| 技能安装失败，提示 UUID/主键冲突 | 技能文件名与其他插件重复；`_id` 是全局唯一的 |
| 提示词没有进入 kv | `plugins_type` 为空或不在映射表中；或 `kv_lib` 不可用 |
| 升级后技能 / 能力消失 | `prune` 默认 `true`，新包缺少对应文件；保留文件或传 `prune: false` |
| 返回 `-2` | 响应超过 64 KiB；减小 `page_size` 或缩小查询范围 |

## 6. 清理示例数据

示例没有提供删除脚本。需要清理时通过 FFI 调用：

```json
{ "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10" }
```

使用 `FFI-Symbol: delete_plugins`。注意该接口只删除插件主表行与能力，
技能、提示词、任务流需要分别用 `delete_skills`、`update_prompts`（写空数组）、
`update_taskflow`（移除对应元素）处理。`update_prompts` / `update_taskflow` 都是整体覆盖，
操作前先读取现有数据，避免影响其他插件。
