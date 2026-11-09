// plugin_lib 的 Deno FFI 封装：调用 .dytes/libary 下的动态库管理 skills。
//
// 依赖库（libdesigndb 等）由 plugin_lib 按 `./libary/*` 相对当前工作目录加载，
// 因此运行脚本时 cwd 必须是 `.dytes`：
//   cd .dytes && deno run --allow-ffi --allow-read --allow-write ../ts/skills.ts

const OUT_BUF_SIZE = 65_536;

const libName =
  Deno.build.os === "windows"
    ? "plugin_lib.dll"
    : Deno.build.os === "darwin"
    ? "libplugin_lib.dylib"
    : "libplugin_lib.so";

const libUrl = new URL(`../.dytes/libary/${libName}`, import.meta.url);

const dylib = Deno.dlopen(libUrl, {
  get_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  post_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  update_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  patch_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  delete_skills: { parameters: ["pointer", "pointer"], result: "i32" },
  get_prompts: { parameters: ["pointer", "pointer"], result: "i32" },
  update_prompts: { parameters: ["pointer", "pointer"], result: "i32" },
  get_taskflow: { parameters: ["pointer", "pointer"], result: "i32" },
  update_taskflow: { parameters: ["pointer", "pointer"], result: "i32" },
});

const encoder = new TextEncoder();
const decoder = new TextDecoder();

type FfiFn = (jsonIn: Deno.PointerValue, jsonOut: Deno.PointerValue) => number;
type SymbolName = keyof typeof dylib.symbols;

export interface Skill {
  id: string;
  plugin_uuid: string;
  name: string;
  description?: string;
  category?: string;
  content?: string;
  trigger?: string;
  params_schema?: Record<string, unknown>;
  is_system?: number;
  is_enabled?: number;
  sort?: number;
  version?: number;
  created_at?: number;
  updated_at?: number;
}

export interface SkillList {
  list: Skill[];
  total: number;
  page: number;
  page_size: number;
}

export interface FfiResponse<T> {
  code: number;
  data: T;
  msg: string;
}

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

function unwrap<T>(symbol: SymbolName, input: unknown): T {
  const response = call<T>(symbol, input);
  if (response.code !== 200) {
    throw new Error(`${symbol} 失败 (${response.code}): ${response.msg}`);
  }
  return response.data;
}

export function getSkills(
  params: {
    plugin_uuid?: string;
    categories?: string[];
    ids?: string[];
    enabled?: boolean | number;
    with_content?: boolean;
    page?: number;
    page_size?: number;
  } = {},
): SkillList {
  return unwrap<SkillList>("get_skills", params);
}

export function getSkill(id: string): Skill {
  const result = getSkills({ ids: [id], with_content: true, page_size: 1 });
  const skill = result.list[0];
  if (!skill) {
    throw new Error(`技能不存在: ${id}`);
  }
  return skill;
}

export function postSkill(skill: Skill): Skill {
  return unwrap<Skill>("post_skills", skill);
}

export function updateSkill(id: string, patch: Partial<Skill>): Skill {
  return unwrap<Skill>("update_skills", { id, ...patch });
}

export function patchSkill(id: string, patch: Partial<Skill>): Skill {
  return unwrap<Skill>("patch_skills", { id, ...patch });
}

export function deleteSkill(id: string): { id: string; deleted: boolean } {
  return unwrap<{ id: string; deleted: boolean }>("delete_skills", { id });
}

export interface PromptSet {
  plugin_uuid: string;
  prompts: unknown[];
}

/** 读取某个插件的提示词数组；不存在时返回空数组。 */
export function getPrompts(plugin_uuid: string): PromptSet {
  return unwrap<PromptSet>("get_prompts", { plugin_uuid });
}

/** 整体覆盖某个插件的提示词数组。 */
export function updatePrompts(
  plugin_uuid: string,
  prompts: unknown[],
): PromptSet {
  return unwrap<PromptSet>("update_prompts", { plugin_uuid, prompts });
}

/** 批量导入提示词：每个 plugin_uuid 整体覆盖一次。 */
export function importPromptSets(sets: PromptSet[]): {
  plugins: number;
  prompts: number;
} {
  let plugins = 0;
  let prompts = 0;
  for (const set of sets) {
    updatePrompts(set.plugin_uuid, set.prompts);
    plugins += 1;
    prompts += set.prompts.length;
  }
  return { plugins, prompts };
}

export interface TaskflowSet {
  taskflows: unknown[];
}

/** 读取任务流数组；不存在时返回空数组。 */
export function getTaskflow(): TaskflowSet {
  return unwrap<TaskflowSet>("get_taskflow", {});
}

/** 整体覆盖任务流数组。 */
export function updateTaskflow(taskflows: unknown[]): TaskflowSet {
  return unwrap<TaskflowSet>("update_taskflow", { taskflows });
}

/** 批量导入：已存在则更新，不存在则新增。 */
export function importSkills(skills: Skill[]): {
  created: number;
  updated: number;
} {
  let created = 0;
  let updated = 0;

  for (const skill of skills) {
    const existing = getSkills({
      ids: [skill.id],
      with_content: true,
      page_size: 1,
    }).list[0];

    if (existing) {
      updateSkill(skill.id, skill);
      updated += 1;
    } else {
      postSkill(skill);
      created += 1;
    }
  }

  return { created, updated };
}

export function close(): void {
  dylib.close();
}
