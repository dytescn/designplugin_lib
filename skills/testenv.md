# plugin_lib 测试规范

本项目的测试分两条线：**依赖动态库的集成测试用 Deno + FFI**，**不依赖动态库的纯逻辑测试用 `cargo test`**。`src/` 只放实现代码，不放测试模块。

## 1. 测试环境目录：`./.dytes`

`.dytes/` 是本地测试环境，已被 `.gitignore` 忽略，不提交到仓库。

```text
.dytes/
├── database/        # SQLite 数据库文件（如 plugins.db）
└── libary/          # 测试用动态库
    ├── plugin_lib.dll      # 本项目构建产物
    ├── database.dll        # 依赖：database
    ├── kv_lib.dll          # 依赖：kv_lib
    └── libdesigndb.dll     # 依赖：designdb
```

- 测试产生的数据库、配置等临时数据一律放在 `.dytes/` 下，不要写到仓库其他目录。
- 每次构建后，把产物覆盖到 `.dytes/libary/`，保证测试加载的是最新代码。

## 2. 构建与放置库文件

开发完成后打包成库文件，放入 `./.dytes/libary/`：

```bash
cargo build --release
```

| 平台 | 构建产物 | 放置位置 |
| --- | --- | --- |
| Windows | `target/release/plugin_lib.dll` | `.dytes/libary/plugin_lib.dll` |
| Linux | `target/release/libplugin_lib.so` | `.dytes/libary/plugin_lib.so` |
| macOS | `target/release/libplugin_lib.dylib` | `.dytes/libary/plugin_lib.dylib` |

依赖库（`database`、`kv_lib`、`libdesigndb`）同样放在 `.dytes/libary/`，缺库会导致调用失败。

## 3. TS 直接以 FFI 方式调用

集成测试在 `ts/` 下编写 `.ts` 文件，通过 Deno 的 `Deno.dlopen` 直接加载 `.dytes/libary/` 中的库，不经过任何中间封装层。

### FFI 调用约定

- 每个导出函数签名为 `(json_in: *const c_char, json_out: *mut c_char) -> c_int`。
- `json_in`：请求 JSON 的 C 字符串（以 `\0` 结尾）。
- `json_out`：调用方提供的输出缓冲区，大小固定 **64 KiB**（`OUT_BUF_SIZE = 65_536`）。
- 返回值：`0` 表示已写入响应；`-1` 表示输出指针为空；`-2` 表示响应超出缓冲区。
- 响应为 JSON 外壳：`{ "code": 200, "data": {...}, "msg": "success" }`，业务错误时 `code` 非 200。
- 可调用符号见各 `src/ffi/*.rs` 中的 `pub extern "C" fn`，如 `get_plugins`、`post_plugins`、`get_skills` 等。`Deno.dlopen` 只暴露声明过的符号，测试用到的每个符号都要在 `symbols` 中登记。

### 示例

```ts
// ts/ffi.ts
const OUT_BUF_SIZE = 65_536;

const libName =
  Deno.build.os === "windows"
    ? "plugin_lib.dll"
    : Deno.build.os === "darwin"
    ? "libplugin_lib.dylib"
    : "libplugin_lib.so";

const libPath = new URL(`../.dytes/libary/${libName}`, import.meta.url);
const dylib = Deno.dlopen(libPath, {
  get_plugins: { parameters: ["pointer", "pointer"], result: "i32" },
});

const encoder = new TextEncoder();
const decoder = new TextDecoder();

type FfiFn = (jsonIn: Deno.PointerValue, jsonOut: Deno.PointerValue) => number;

export function call(symbol: string, input: unknown): unknown {
  const fn = dylib.symbols[symbol] as FfiFn;
  const inputBytes = encoder.encode(JSON.stringify(input) + "\0");
  const out = new Uint8Array(OUT_BUF_SIZE);

  const ret = fn(Deno.UnsafePointer.of(inputBytes), Deno.UnsafePointer.of(out));
  if (ret !== 0) {
    throw new Error(`${symbol} failed: ret=${ret}`);
  }

  const end = out.indexOf(0);
  const text = decoder.decode(end === -1 ? out : out.subarray(0, end));
  return JSON.parse(text);
}
```

测试文件使用 `Deno.test`，断言返回的 `code === 200` 及 `data` 字段：

```ts
// ts/plugins_test.ts
import { call } from "./ffi.ts";

Deno.test("get_plugins 返回分页列表", () => {
  const res = call("get_plugins", { page: 1, page_size: 10 });
  if (res.code !== 200) throw new Error(res.msg);
  if (!Array.isArray(res.data.list)) throw new Error("list 不是数组");
});
```

运行：

```bash
deno test --allow-ffi --allow-read --allow-write ts/
```

## 4. 不依赖 FFI 的测试放 `tests/`

不需要加载动态库、不关联 FFI 的方法（纯函数、参数解析、数据转换等）用 Rust 集成测试放在 `tests/` 下：

```bash
cargo test --test <name>
```

### 硬性约束

- **`src/` 目录中的文件不允许出现测试模块化代码**，包括 `#[cfg(test)] mod tests { ... }` 和内联 `#[test]`。
- 测试代码只允许出现在 `tests/`（Rust）和 `ts/`（Deno）中。
- 已有 `src/` 内的测试需迁移到 `tests/` 后再继续开发。
