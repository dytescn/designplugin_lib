// skills 管理 CLI：通过 FFI 调用 .dytes/libary/plugin_lib 管理 skills。
//
// 运行（cwd 必须是 .dytes，插件库按 ./libary/* 加载依赖）：
//   cd .dytes
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts import
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts list
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts get <id>
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts update <id> '{"name":"新名字"}'
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts patch <id> '{"is_enabled":0}'
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts delete <id>
//   deno run --allow-ffi --allow-read --allow-write --allow-env ../ts/skills.ts reset

import {
  close,
  deleteSkill,
  getSkill,
  getSkills,
  importSkills,
  patchSkill,
  updateSkill,
  type Skill,
} from "./ffi.ts";

const dataUrl = new URL("../datasets/skills.json", import.meta.url);

async function loadSkills(): Promise<Skill[]> {
  const text = await Deno.readTextFile(dataUrl);
  const value = JSON.parse(text);
  if (!Array.isArray(value.skills)) {
    throw new Error(`${dataUrl.pathname} 缺少 skills 数组`);
  }
  return value.skills as Skill[];
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

function parsePatch(text: string | undefined): Partial<Skill> {
  if (!text) {
    return {};
  }
  return JSON.parse(text) as Partial<Skill>;
}

const [command = "help", ...args] = Deno.args;

try {
  switch (command) {
    case "import": {
      const skills = await loadSkills();
      print(importSkills(skills));
      break;
    }
    case "reset": {
      const skills = await loadSkills();
      for (const skill of skills) {
        try {
          deleteSkill(skill.id);
        } catch {
          // 不存在时忽略
        }
      }
      print(importSkills(skills));
      break;
    }
    case "list": {
      const pluginUuid = args[0];
      print(
        getSkills({
          ...(pluginUuid ? { plugin_uuid: pluginUuid } : {}),
          page_size: 200,
        }),
      );
      break;
    }
    case "get": {
      print(getSkill(requireArg(args, 0, "get <id>")));
      break;
    }
    case "update": {
      const id = requireArg(args, 0, 'update <id> <json>');
      print(updateSkill(id, parsePatch(args[1])));
      break;
    }
    case "patch": {
      const id = requireArg(args, 0, 'patch <id> <json>');
      print(patchSkill(id, parsePatch(args[1])));
      break;
    }
    case "delete": {
      print(deleteSkill(requireArg(args, 0, "delete <id>")));
      break;
    }
    default: {
      console.log(`用法: deno run ... ts/skills.ts <命令> [参数]

命令:
  import                     导入 datasets/skills.json（存在则更新）
  reset                      删除数据集中的技能后重新导入
  list [plugin_uuid]         列出技能
  get <id>                   查看单条技能
  update <id> <json>         全量更新指定字段
  patch <id> <json>          部分更新指定字段
  delete <id>                删除技能`);
    }
  }
} finally {
  close();
}
