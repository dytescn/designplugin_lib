# [P2] 清单校验与库文件定位：`exists()` 误判目录、更新不校验 manifest UUID

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 位置：`src/utils/package.rs`
  - `locate_lib`（约 295–352 行）
  - `update`（约 113–196 行）
  - `read_manifest` / `require_manifest_text`（约 240–280 行）
- 状态：待商议，未修改代码

## 现象

1. `locate_lib` 过滤候选文件时使用：

   ```rust
   package.join(&candidate).is_file() || package.join(&candidate).exists()
   ```

   `exists()` 对目录也返回 true。因此 `platform-exports` 指向一个同名目录时，
   会被当作库文件候选，最终返回目录名，而不是报“库文件不存在”。

2. `update_plugin` 通过 `find_installed` 找到已安装插件后，直接使用找到的 `uuid`
   调用 `upsert_plugin`，没有校验 `dytes.json.uuid` 与已安装 `uuid` 是否一致。
   如果包内 UUID 写错、或调用方传了另一个 `plugin_uuid`，可能用 A 包的内容更新 B 插件。

3. `read_manifest` 只校验顶层是 JSON 对象；`require_manifest_text` 只校验 `uuid` / `name`
   非空。以下规范要求都没有校验：
   - `uuid` 是否符合 UUID 格式；
   - `version` 是否是语义化版本；
   - `name` 是否符合字符集（无空格、中文、路径分隔符、版本号）；
   - `platform-exports` 是否是对象、平台键是否正确、值是否是文件名；
   - `plugins_type` 是否是已知取值（未知目前按不同步 kv 处理，可能是有意的）。

4. `platform-exports` 结构错误（例如当前平台的值是字符串而不是对象）时，
   `locate_lib` 会静默回退扫描包根，而不是提示清单错误。

## 影响

- 目录被误判为库文件，安装成功但运行时无法加载；
- 错误包可能覆盖其他插件的数据，且没有明确错误；
- 不符合规范的清单可以进入系统，问题推迟到运行或发布后；
- 文档中的强制条款与代码实际校验不一致。

## 建议（待商议）

1. `locate_lib` 只使用 `is_file()`；目录候选直接排除。
2. `update_plugin` 增加 `manifest_uuid` 与目标插件 `uuid` 的一致性校验；
   如果不一致，返回 `400103` 或明确要求 `force`。
3. 明确“安装时校验”与“发布检查”的边界：
   - 如果安装也应强制规范，补充 UUID / 版本 / 名称 / `platform-exports` 校验；
   - 如果只做发布检查，在文档中明确“安装不校验这些字段”，并给发布工具提供校验入口。
4. `platform-exports` 结构错误时返回明确错误，不要静默回退；只有字段缺失或当前平台
   没有条目时才回退扫描。

## 需要确认

- `update_plugin` 是否允许通过请求参数把包安装到另一个 UUID（当前 `find_installed`
  支持按请求 `plugin_uuid` / `name` 查找）？
- UUID 变更是否应视为新插件（需要显式安装）？
- 安装时强制 UUID / 版本格式是否会影响现有存量包？需要兼容策略。

## 复现思路

1. 在包根创建一个与 `platform-exports` 目标同名的目录，执行安装，观察是否返回该目录名；
2. 安装插件 A 后，用 `uuid` 属于 B 的包调用 `update_plugin`（或传 `plugin_uuid` 指向 B），
   观察是否更新了 B；
3. 写入非法 `uuid` / `version` / `platform-exports`，确认安装是否接受。
