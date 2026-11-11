# [P2] 内容长度规范：prompt ≤ 50 个汉字（150 字节）、skill content ≤ 1 MiB

- 发现日期：2026-11-11
- 代码版本：`plugin_lib` 1.0.20
- 状态：prompt 规则已实现；skill content 1 MiB 仍待同步
- 相关文档：`docs/plugin-spec/content.md`、`docs/plugin-spec/checklist.md`、
  `docs/plugin-package.md`、`docs/ffi-api.md`

## 规范要求

1. `prompts.json` 中每个 prompt 的 `content` 去首尾空白后，按 UTF-8 编码 **≤ 150 字节**
   （对应 50 个汉字，每汉字 3 字节；英文、数字、标点按实际字节计入）。
2. `skills/*.md` 的文件内容（含标题行）按 UTF-8 编码后 **≤ 1 MiB（1,048,576 字节）**。

## 当前代码

| 校验点 | 当前行为 |
| --- | --- |
| `src/utils/prompts.rs` `validate_prompt_list` | 新增：对象校验 + `content` 去空白后 ≤ 150 字节 |
| 安装流程 `sync_prompts` | 写 designdb / kv 前调用 `validate_prompt_list` |
| `update_prompts` FFI | 写 designdb 前调用 `validate_prompt_list` |
| `src/utils/plugin.rs` `MAX_SKILL_CONTENT_CHARS` | 仍为 `8000` 字符，不是 1 MiB |
| `post_skills` / `update_skills` | 仍用 8000 字符限制校验 `content` |
| 安装流程 `sync_skills` | 不校验技能内容长度，直接写入 |
| 错误消息 | 带位置，如 `prompts[1].content 不能超过 150 字节（50 个汉字）` |
| 测试 | `cargo test` 18 个通过，覆盖 50/51 汉字、150/151 字节、非对象 |

## 仍待修改

1. 将技能 `content` 限制从“8000 字符”改为 **1 MiB UTF-8 字节**；
   安装流程与 `post_skills` / `update_skills` 使用同一校验函数。
2. 评估存量数据：已安装的 prompt 如果超过 150 字节，更新时会被拒绝；
   需要确认是否允许读取、仅在更新时拒绝，或提供迁移工具。
3. 大技能（超过 8000 字符但小于 1 MiB）当前会被 FFI 拒绝；需要同步后再放开。

## 待确认

- skill 的 1 MiB 按 **1 MiB（1,048,576 字节）** 处理，还是 1,000,000 字节？
- 存量超限 prompt 是允许读取、仅在更新时拒绝，还是安装 / 更新都要拦截？
- 技能 1 MiB 是文件大小还是 `content` 字段字节数？两者当前基本一致（content 是文件全文）。

## 复现思路

1. 写入一个 50 汉字（150 字节）的 prompt，安装 / `update_prompts` 应通过；
2. 写入一个 51 汉字（153 字节）的 prompt，应返回 `400103`；
3. 写入一个超过 8000 字符但小于 1 MiB 的技能，通过 `update_skills` 更新，确认当前仍被
   8000 字符限制拒绝；
4. 写入一个超过 1 MiB 的技能文件，确认安装流程未校验。
