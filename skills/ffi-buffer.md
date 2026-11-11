# 库间 FFI 缓冲区约定

本文件记录 `plugin_lib` 与依赖动态库之间的 `json_out` 缓冲区约定。
**当前只记录目标，代码尚未修改；后续按本文件逐步迁移。**

## 1. 适用范围

| 调用方向 | 示例 | 约定 |
| --- | --- | --- |
| 库与库之间 | `plugin_lib` → `database` / `kv_lib` / `designdb` | 目标 **1024 KiB**，可按实际需要扩大 |
| 对外 FFI | `cyctron` / `plugin_ui` → `plugin_lib` | 继续按现有 64 KiB 约定，或另行评审 |

两类缓冲区**不得共用同一个常量**。修改库间缓冲区属于 FFI 契约调整，
按 [dev.md](./dev.md) 的规则先由人工确认。

## 2. 当前实现（2026-11-11）

| 调用 | 当前常量 | 当前值 | 目标 | 备注 |
| --- | --- | --- | --- | --- |
| `plugin_lib` → `database` | `src/utils/cjson.rs` `OUT_BUF_SIZE` | 64 KiB | 1024 KiB | 目前与对外 FFI 共用常量，需拆分 |
| `plugin_lib` → `kv_lib` | `src/utils/kv.rs` `OUT_BUF_SIZE` | 64 KiB | 1024 KiB | 需确认 kv_lib 侧输出上限 |
| `plugin_lib` → `designdb` | `src/utils/designdb.rs` `OUT_BUF_SIZE` | 64 KiB | 1024 KiB | designdb 1.0.3 可写 16 MiB，见 §3 |
| 对外 FFI | `src/utils/cjson.rs` `OUT_BUF_SIZE` | 64 KiB | 保持 / 另行评审 | 不属于库间调用 |

依赖库自身的当前上限：

| 库 | 当前版本 | 输出上限 | 说明 |
| --- | --- | --- | --- |
| `database` | 1.0.4 | 约 64 KiB | `src/utils/cjson.rs` 中 `bytes.len() >= 65536` |
| `kv_lib` | 0.1.4 | 约 64 KiB | FFI 输出上限 64 KiB；单条 `value` 上限 256 KiB |
| `designdb` | 1.0.3 | 16 MiB | `MAX_RESPONSE_BYTES = 16 * 1024 * 1024`，调用方需分配 `+1` 字节 |

> `designdb` 1.0.3 与 1024 KiB 目标存在冲突：如果 plugin_lib 只分配 1024 KiB，
> 而 designdb 返回 1–16 MiB 的响应，会发生越界。升级 designdb 依赖前必须先解决：
> 由 designdb 限制响应大小、增加缓冲区大小协商，或对 designdb 单独使用更大的缓冲区（例外）。

## 3. 迁移顺序（待执行）

1. 为 `database`、`kv_lib`、`designdb` 分别定义独立常量，不要复用
   `cjson::OUT_BUF_SIZE`。
2. 先与各依赖库确认其最大响应，再按 **1024 KiB** 分配库间缓冲区；
   如果某库明确需要更大（例如 designdb 的 16 MiB），单独记录为例外并说明理由。
3. 更新 `docs/ffi-api.md`、`docs/data-model.md`、`skills/testenv.md` 中相关的
   缓冲区说明；对外 FFI 的 64 KiB 约定不要被误改。
4. 增加边界测试：接近目标上限、超过目标上限、以及依赖库返回更大响应的场景。
5. 完成后再升级 `dytes.json` / `dytes.lock` 中的依赖版本。

## 4. 相关 issue

- [2026-11-11-designdb-64k.md](../issues/2026-11-11-designdb-64k.md)：designdb 16 MiB
  与 plugin_lib 缓冲区的联动问题。
