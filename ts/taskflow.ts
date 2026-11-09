// taskflow 管理 CLI：整体读写任务流数组。
//
// 运行（cwd 必须是 .dytes，插件库按 ./libary/* 加载依赖）：
//   cd .dytes
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/taskflow.ts import
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/taskflow.ts get
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/taskflow.ts update '[{"name":"...","desc":"...","uid":"..."}]'

import { close, getTaskflow, updateTaskflow } from "./ffi.ts";

const dataUrl = new URL("../datasets/taskflow.json", import.meta.url);

async function loadTaskflows(): Promise<unknown[]> {
  const value = JSON.parse(await Deno.readTextFile(dataUrl));
  if (!Array.isArray(value)) {
    throw new Error(`${dataUrl.pathname} 必须是数组`);
  }
  return value;
}

function print(value: unknown): void {
  console.log(JSON.stringify(value, null, 2));
}

const [command = "help", ...args] = Deno.args;

try {
  switch (command) {
    case "import": {
      print(updateTaskflow(await loadTaskflows()));
      break;
    }
    case "get": {
      print(getTaskflow());
      break;
    }
    case "update": {
      const raw = args[0];
      if (!raw) {
        throw new Error("用法: update <json数组>");
      }
      const taskflows = JSON.parse(raw);
      if (!Array.isArray(taskflows)) {
        throw new Error("taskflows 必须是数组");
      }
      print(updateTaskflow(taskflows));
      break;
    }
    default: {
      console.log(`用法: deno run ... ts/taskflow.ts <命令> [参数]

命令:
  import                 导入 datasets/taskflow.json（整体覆盖）
  get                    读取任务流数组
  update <json数组>      整体覆盖任务流数组`);
    }
  }
} finally {
  close();
}
