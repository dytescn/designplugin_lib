// plugin_lib 示例调用：安装示例包并核对写入结果。
//
// 运行前提：
//   1. 已执行 cargo build --release；
//   2. .dytes/libary/ 中有 plugin_lib.dll、database.dll、libdesigndb.dll、
//      kv_lib.dll（文件名按平台调整）；
//   3. docs/examples/demo-plugin/ 中放入了当前平台的示例库文件
//      （如 demo_design.dll），否则 install_plugin 会报找不到库文件。
//
// 运行（cwd 必须是 .dytes，依赖库按 ./libary/* 加载）：
//   cd .dytes
//   deno run --allow-ffi --allow-read --allow-write ../docs/examples/ffi/call.ts
//
// 也可以传入插件包目录（相对当前 cwd）：
//   deno run --allow-ffi --allow-read --allow-write ../docs/examples/ffi/call.ts ../docs/examples/demo-plugin

const OUT_BUF_SIZE = 65_536;

const libName = Deno.build.os === "windows"
  ? "plugin_lib.dll"
  : Deno.build.os === "darwin"
  ? "libplugin_lib.dylib"
  : "libplugin_lib.so";

// 脚本位于 docs/examples/ffi/，向上三级回到仓库根，再进入 .dytes。
const libUrl = new URL(`../../../.dytes/libary/${libName}`, import.meta.url);

const dylib = Deno.dlopen(libUrl, {
  install_plugin: { parameters: ["pointer", "pointer"], result: "i32" },
  get_plugins: { parameters: ["pointer", "pointer"], result: "i32" },
  get_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  get_skill: { parameters: ["pointer", "pointer"], result: "i32" },
  get_functions: { parameters: ["pointer", "pointer"], result: "i32" },
  get_prompts: { parameters: ["pointer", "pointer"], result: "i32" },
  get_taskflow: { parameters: ["pointer", "pointer"], result: "i32" },
});

type SymbolName = keyof typeof dylib.symbols;
type FfiFn = (jsonIn: Deno.PointerValue, jsonOut: Deno.PointerValue) => number;

interface FfiResponse<T> {
  code: number;
  data: T;
  msg: string;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function call<T>(symbol: SymbolName, input: unknown): FfiResponse<T> {
  const fn = dylib.symbols[symbol] as unknown as FfiFn;
  const inputBytes = encoder.encode(JSON.stringify(input) + "\0");
  const output = new Uint8Array(OUT_BUF_SIZE);

  const ret = fn(
    Deno.UnsafePointer.of(inputBytes),
    Deno.UnsafePointer.of(output),
  );
  if (ret !== 0) {
    throw new Error(`${symbol} 调用失败: ret=${ret}`);
  }

  const end = output.indexOf(0);
  const text = decoder.decode(end === -1 ? output : output.subarray(0, end));
  return JSON.parse(text) as FfiResponse<T>;
}

function ensureOk<T>(symbol: SymbolName, response: FfiResponse<T>): T {
  if (response.code !== 200) {
    throw new Error(`${symbol} 失败 (${response.code}): ${response.msg}`);
  }
  return response.data;
}

const packageDir = Deno.args[0] ??
  "../docs/examples/demo-plugin";

try {
  console.log(`安装插件包: ${packageDir}`);
  const installed = ensureOk(
    "install_plugin",
    call<{ plugin_uuid: string; skipped: boolean }>("install_plugin", {
      dir: packageDir,
    }),
  );
  console.log("install_plugin:", installed);

  const pluginUuid = installed.plugin_uuid;

  const plugin = ensureOk(
    "get_plugins",
    call<Record<string, unknown>>("get_plugins", { uuid: pluginUuid }),
  );
  console.log("plugin:", {
    uuid: plugin.uuid,
    name: plugin.name,
    version: plugin.version,
    installed: plugin.installed,
    install_path: plugin.install_path,
    plugins_type: plugin.plugins_type,
  });

  const skills = ensureOk(
    "get_skills",
    call<{ list: Array<Record<string, unknown>>; total: number }>("get_skills", {
      plugin_uuid: pluginUuid,
      page: 1,
      page_size: 50,
    }),
  );
  console.log("skills:", {
    total: skills.total,
    list: skills.list.map((skill) => ({
      id: skill.id,
      name: skill.name,
      category: skill.category,
      sort: skill.sort,
      version: skill.version,
    })),
  });

  const firstSkillId = skills.list[0]?.id;
  if (typeof firstSkillId === "string") {
    const skill = ensureOk(
      "get_skill",
      call<Record<string, unknown>>("get_skill", { id: firstSkillId }),
    );
    console.log("skill detail:", {
      id: skill.id,
      name: skill.name,
      content_length: typeof skill.content === "string" ? skill.content.length : 0,
    });
  }

  const functions = ensureOk(
    "get_functions",
    call<{ list: Array<Record<string, unknown>>; total: number }>("get_functions", {
      plugin_uuid: pluginUuid,
      page: 1,
      page_size: 50,
    }),
  );
  console.log("functions:", {
    total: functions.total,
    list: functions.list.map((item) => ({
      uuid: item.uuid,
      ffi_symbol: item.ffi_symbol,
      title: item.title,
      use_ai: item.use_ai,
    })),
  });

  const prompts = ensureOk(
    "get_prompts",
    call<{ plugin_uuid: string; prompts: unknown[] }>("get_prompts", {
      plugin_uuid: pluginUuid,
    }),
  );
  console.log("prompts:", {
    plugin_uuid: prompts.plugin_uuid,
    count: prompts.prompts.length,
    names: prompts.prompts.map((item) =>
      (item as { name?: string }).name
    ),
  });

  const taskflow = ensureOk(
    "get_taskflow",
    call<{ taskflows: Array<Record<string, unknown>> }>("get_taskflow", {}),
  );
  console.log("taskflow:", {
    count: taskflow.taskflows.length,
    items: taskflow.taskflows.map((item) => ({
      uid: item.uid,
      name: item.name,
      plugin_uuid: item.plugin_uuid,
    })),
  });
} finally {
  dylib.close();
}
