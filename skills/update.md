#### update

更新已安装插件：用新包目录覆盖插件信息、库文件以及技能、提示词、任务流和 MCP 能力。

##### 传入

- 插件包目录（同安装），或
- 已安装插件的 `plugin_uuid` / `name`，配合新包目录。

##### 前置条件

- 插件必须已经安装，`plugins` 中存在对应 `uuid`。
- 新包目录中的 `dytes.json` 必须存在且可解析。
- 升级包必须满足 [包类型内容规范](../docs/plugin-spec/package-types.md) 的发布要求；
  安装流程不强制校验这些内容，缺少时只会让对应数据为空或保留旧值。
- 版本判断：
  - `plugins.version` 不低于 `dytes.json.version`：跳过更新，返回 `skipped: true`。
  - `plugins.version` 低于包版本：执行更新。
  - 版本为空（旧数据）时按需要更新处理。

##### 与 install 的区别

- install 可以新增插件；update 只更新已存在的插件。
- update 会清理“包内已移除”的数据（技能、MCP 能力、任务流元素）。
- 已存在数据的 `created_at` 保留，`updated_at` 刷新，技能 `version` 递增。

##### 过程

1. **定位插件**

   优先按 `dytes.json.uuid` 查找；找不到时按 `name` 查找。两者都不存在时返回“插件未安装”。

2. **更新插件主表**

   用新 `dytes.json` 更新：
   - `name`、`desc`、`plugins_type`、`target`、`platform`。
   - `install_path` 指向新包目录。
   - `updated_at` 刷新。
   - `created_at`、`mask`、`installed`、`enabled` 等安装状态字段保持不变（除非清单显式提供）。

3. **更新库文件**

   - 按当前平台选择新的 `.dll` / `.so` / `.dylib`。
   - 新库文件找不到时更新失败，保留旧库记录不变。
   - 库文件名变化时，以新文件为准。

4. **同步 skills**

   - 读取新包 `skills/*.md`。
   - 已存在同 `_id` 的技能：更新内容、名称、分类，`version + 1`。
   - 新文件：新增技能。
   - 该插件下、但新包中已不存在的技能：默认删除；可通过 `prune=false` 保留。
   - 因此 `chat_think` / `chat_reply` 以及 `use_ai = 1` 的 `design_*` 升级包必须继续提供
     `skills/`，否则技能会被清空且发布不合规。

5. **同步 prompts**

   - 新包有 `prompts.json`：整体覆盖该插件在 designdb `prompts` 集合中的数组。
   - 新包没有 `prompts.json`：保留原 prompts，不清空。
   - 同步完成后，如果 `kv_lib` 可用，再按 `plugins_type` 同步到对应 kv 分类：

     | `plugins_type` | kv 分类 |
     | --- | --- |
     | `chat_think` | `chat_think` |
     | `chat_reply` | `chat_reply` |
     | `chat_plugins` | `chat_plugins` |
     | `sys_plugins` | `sys_plugins` |
     | `plugins` / `plugins_coreldraw` | `chat_plugin`（兼容旧清单） |
     | 其他 | 不同步；类型切换时清理其他集合 |

   - kv 中每条 prompt 记录保存的是该条目的 `content`：记录的 `value` / `text` 都取 `content`。
   - prompt key 使用 `{plugin_uuid}:prompt:{标识}`，标识优先取条目的 `id`，其次 `name`，最后 `content` 哈希；旧数据使用 `{plugin_uuid}:prompt:{index}` 时需要先按 content 对齐，再迁移到稳定标识。
   - 更新时先读取该 `plugin_uuid` 前缀下的现有记录，比较 `content`、`rank`、`meta`：
     - 三者都未变：保留原记录，不重写、不重新向量化。
     - `content` 变化：更新记录并重新向量化。
     - 仅 `rank` / `meta` 变化：更新记录，不重新向量化（写入使用 `force: false`）。
     - 新增 content：写入新 key。
     - 旧 content 在新 prompts 中已不存在：从 kv 中删除对应记录。
   - `prompts.json` 是空数组时，删除该插件在目标集合中的全部记录。
   - `plugins_type` 切换时，安装 / 更新会读取旧类型并扫描已知集合（排除新集合）清理该插件的记录；新包没有 `prompts.json` 时，会使用 designdb 中已有的提示词迁移到新集合；旧类型为空或未知时同样扫描已知集合。
   - 对比只处理 `{plugin_uuid}:prompt:` 前缀，不影响其他插件或其他类型的记录。
   - `kv_lib` 未安装时跳过该步骤，本地 prompts 仍正常更新；kv 写入失败返回错误，可重试。

6. **同步 taskflow**

   - 按元素 `uid`（或 `name`）更新，新元素追加。
   - 新包中已移除的元素默认删除；可通过 `prune=false` 保留。
   - 只处理与当前插件相关的元素，不影响其他插件。

7. **同步 MCP 能力（`plugin_functions`）**

   - 按 `uuid` upsert 新包中的 tools。
   - 该插件下、新包中已不存在的 `plugin_functions` 记录默认删除，保证能力列表与包内容一致。
   - `created_at` 保留，`updated_at` 刷新。
   - `design_*` 升级包必须继续提供 `mcp.json`；缺少文件时保留原能力，但发布不合规。
     新包 `mcp.json` 存在但 `tools` 为空时，会清理该插件的全部能力。

8. **版本记录**

   - 新版本号写入安装记录（如存在版本表）并随结果返回。
   - 更新完成后 `installed = 1`，`install_path` 指向当前包目录。

##### 幂等与失败

- 重复执行同一新包，结果一致：内容不变、版本不重复递增（技能 `version` 仅在内容实际变化时递增）。
- 任一步骤失败时返回错误；已完成的步骤保留，修正后可重试。
- 库文件缺失、清单非法等属于致命错误，不执行后续步骤。

##### 返回

```json
{
  "plugin_uuid": "…",
  "name": "…",
  "from_version": "1.0.0",
  "to_version": "1.0.1",
  "install_path": "./plugins/<name>",
  "platform": "windows",
  "lib_file": "plugin.dll",
  "skipped": false,
  "updated": {
    "skills": 2,
    "prompts": 1,
    "taskflows": 1,
    "functions": 3
  },
  "added": {
    "skills": 1,
    "functions": 2
  },
  "removed": {
    "skills": 1,
    "functions": 1
  }
}
```

- `updated` / `added` / `removed` 分别表示更新、新增、清理的数量。
- 未安装、清单非法、库文件缺失等返回非 200 业务码。
