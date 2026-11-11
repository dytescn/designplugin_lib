#### install

安装一个插件目录：登记插件主表、库文件，并导入技能、提示词、任务流和 MCP 能力。

##### 传入

一个插件包目录，例如 `./plugins/<name>/`，目录内至少包含：

```text
<package>/
├── dytes.json          # 必填，插件清单
├── <lib>.dll|.so|.dylib
├── skills/*.md         # 按类型：chat_think、chat_reply 必须；design_* 且 use_ai=1 必须
├── prompts.json        # 可选，提示词
├── taskflow.json       # 按类型：chat_plugins 必须
└── mcp.json            # 按类型：design_* 必须；chat_* 可选；其他可选
```

> 发布层面的内容要求：
> - `plugins_type` 为 `chat_think` / `chat_reply` 时必须带 `skills/`（至少一个有效 `.md`），
>   `chat_plugins` 时必须带 `taskflow.json`（至少一个元素）；`mcp.json` 对三者可选；
> - `plugins_type` 以 `design_` 开头时必须带 `mcp.json`；`design_*` 且 `use_ai = 1` 时
>   必须带 `skills/`。
>
> 安装流程本身不校验这些内容，缺少时只是对应数据为空。详见
> [包类型内容规范](../docs/plugin-spec/package-types.md)。

##### 前置校验

- 目录必须存在且可读。
- `dytes.json` 必须存在并能解析为 JSON 对象。
- `dytes.json.uuid`、`name` 不能为空。

##### 过程

1. **登记插件主表**

   读取 `dytes.json`：
   - `uuid` → `plugins.uuid`（主键）
   - `name` → `plugins.name`，`description` → `plugins.desc`
   - `plugins_type` → `plugins.plugins_type`
   - `version` → `plugins.version`
   - `target` → `plugins.target`，缺省 `common`
   - `type` / 其他业务字段按清单原样对应

   按当前运行系统确定 `platform`（`win` / `mac` / `linux`），并写入 `plugins.platform`。

   `install_path` 写插件包目录（例如 `./plugins/<name>`）。

   重复安装同一 `uuid` 时按主键更新，不新增行；`created_at` 保持不变，刷新 `updated_at`。

   版本判断：
   - 已安装且 `plugins.version` 不低于 `dytes.json.version`：跳过安装，返回 `skipped: true`，不再写入技能、提示词、任务流和 MCP 能力。
   - 已安装但 `plugins.version` 低于包版本：按更新流程覆盖。
   - 未安装：正常安装并写入 `plugins.version`。

2. **定位库文件**

   从 `dytes.json` 的 `platform-exports` 中取当前平台的一组导出：

   | 平台 | 匹配扩展名 |
   | --- | --- |
   | Windows | `.dll` |
   | Linux | `.so` |
   | macOS | `.dylib` |

   - 只接受与当前平台扩展名完全匹配的文件。
   - 有多个候选时，优先与插件 `name` 同名的文件。
   - 找不到匹配文件时安装失败，返回明确错误。
   - 记录库文件名到安装结果；`plugins.install_path` 仍是包目录。

3. **录入 skills**

   若包内存在 `skills/` 目录：
   - 逐个读取 `*.md` 文件。
   - 每个文件生成一条 designdb `skills` 文档（字段见 `db.md`）：
     - `_id` / `id`：文件名去掉 `.md`；同一插件内必须唯一。
     - `plugin_uuid`：当前插件 `uuid`。
     - `name`：文件首行一级标题（`# ...`），没有则用文件名。
     - `content`：文件正文。
     - `category`：缺省 `plugin`。
     - `trigger`、`params_schema`：文件内没有对应声明时留空 / `{}`。
     - `is_system` 默认 0，`is_enabled` 默认 1，`sort` 按文件名升序。
   - 已存在同 `_id` 的技能按内容更新，并递增 `version`。

4. **录入 prompts**

   若包内存在 `prompts.json`：
   - 支持直接是数组，或 `{ "prompts": [...] }`。
   - 读取数组后，整体写入该插件在 designdb `prompts` 集合中的文档：
     - `_id` / `plugin_uuid`：当前插件 `uuid`。
     - `prompts`：数组原样保存。
   - 重复安装时整体覆盖，不保留旧数组中的元素。

   写入 designdb 后，如果 `kv_lib` 可用，再按 `plugins_type` 把 prompts 同步到对应 kv 分类；kv 记录保存每个条目的 `content`。首次安装视为全量写入，已存在的同 key 记录按 `content` / `rank` / `meta` 对比结果更新；空数组会清理该插件的旧记录，类型切换会清理其他集合（规则见 `update.md`）。

5. **录入 taskflow**

   若包内存在 `taskflow.json`：
   - 支持直接是数组，或 `{ "taskflows": [...] }`。
   - 读取 designdb 中 `taskflow` 集合的唯一文档，按元素 `uid`（或 `name`）合并：
     - 已存在相同 `uid` / `name` 的元素就替换。
     - 不存在则追加。
   - 不删除集合中其他插件贡献的元素。

6. **录入 MCP 能力**

   若包内存在 `mcp.json`（或 `mcp/mcp.json`）：
   - 读取 `tools` 数组。
   - 每个 tool 生成一条 `plugin_functions` 记录：
     - `uuid`：优先取 `tool.uuid` / `tool.id`；没有时使用 `{plugin_uuid}:{tool.name}`。
     - `plugin_uuid`：当前插件 `uuid`。
     - `ffi_symbol`：`tool.ffi_symbol`，缺省 `tool.name`。
     - `title`：`tool.title`，缺省 `tool.name`。
     - `description`：`tool.description`，缺省空字符串。
     - `params_schema`：`tool.inputSchema`，缺省 `{"type":"object"}`。
     - `params_example`：`tool.inputExample`，可选。
     - `return_schema`：`tool.outputSchema`，缺省 `{"type":"object"}`。
     - `return_example`：`tool.outputExample`，可选。
     - `use_ai`：`tool.use_ai`，缺省继承 `plugins.use_ai`。
   - 已存在同 `uuid` 的能力按内容更新，不新增重复行。

##### 幂等与失败

- 安装流程必须可重复执行：所有写入按主键 upsert。
- 任一必填文件解析失败时立即返回错误，不继续后续步骤。
- 可选文件缺失时跳过对应步骤，不算失败。
- 已写入的数据不回滚；修正输入后重新执行即可。

##### 返回

```json
{
  "plugin_uuid": "…",
  "name": "…",
  "version": "1.0.0",
  "install_path": "./plugins/<name>",
  "platform": "win",
  "lib_file": "plugin.dll",
  "skipped": false,
  "counts": {
    "skills": 3,
    "prompts": 4,
    "taskflows": 1,
    "functions": 5
  }
}
```

- `counts` 为本轮成功写入的数量。
- 库文件缺失、`dytes.json` 非法等错误返回非 200 的业务码和可读信息。
