// prompts 管理 CLI：按 plugin_uuid 整体读写提示词数组。
//
// 运行（cwd 必须是 .dytes，插件库按 ./libary/* 加载依赖）：
//   cd .dytes
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/prompts.ts import
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/prompts.ts list
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/prompts.ts get <plugin_uuid>
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/prompts.ts update <plugin_uuid> '[{"name":"..."}]'

import {
  close,
  getPrompts,
  importPromptSets,
  updatePrompts,
  type PromptSet,
} from "./ffi.ts";

const dataUrl = new URL("../datasets/prompts.json", import.meta.url);

async function loadPromptSets(): Promise<PromptSet[]> {
  const text = await Deno.readTextFile(dataUrl);
  const value = JSON.parse(text);
  if (!Array.isArray(value.prompts)) {
    throw new Error(`${dataUrl.pathname} 缺少 prompts 数组`);
  }
  return value.prompts as PromptSet[];
}

function print(value: unknown): void {
  console.log(JSON.stringify(value, null, 2));
}

function requireArg(args: string[], index: number, usage: string): string {
  const value = args[index];
  if (!value) {
    throw new Error(`用法: ${usage}`);
  }
  return value;
}

const [command = "help", ...args] = Deno.args;

try {
  switch (command) {
    case "import": {
      print(importPromptSets(await loadPromptSets()));
      break;
    }
    case "list": {
      const sets = await loadPromptSets();
      print(sets.map((set) => getPrompts(set.plugin_uuid)));
      break;
    }
    case "get": {
      print(getPrompts(requireArg(args, 0, "get <plugin_uuid>")));
      break;
    }
    case "update": {
      const pluginUuid = requireArg(args, 0, 'update <plugin_uuid> <json>');
      const prompts = JSON.parse(requireArg(args, 1, 'update <plugin_uuid> <json>'));
      if (!Array.isArray(prompts)) {
        throw new Error("prompts 必须是数组");
      }
      print(updatePrompts(pluginUuid, prompts));
      break;
    }
    default: {
      console.log(`用法: deno run ... ts/prompts.ts <命令> [参数]

命令:
  import                             导入 datasets/prompts.json（按插件整体覆盖）
  list                               读取数据集中各插件的提示词
  get <plugin_uuid>                  读取指定插件的提示词数组
  update <plugin_uuid> <json数组>    整体覆盖指定插件的提示词数组`);
    }
  }
} finally {
  close();
}
