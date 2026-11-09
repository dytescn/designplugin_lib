// Deno 集成测试：验证 prompts 按 plugin_uuid 整体读写。
//
// 运行（cwd 必须是 .dytes）：
//   cd .dytes && deno test --allow-ffi --allow-read --allow-write ../ts/prompts_test.ts

import {
  close,
  getPrompts,
  importPromptSets,
  updatePrompts,
  type PromptSet,
} from "./ffi.ts";

const dataUrl = new URL("../datasets/prompts.json", import.meta.url);
const sets = JSON.parse(await Deno.readTextFile(dataUrl)).prompts as PromptSet[];
const testPlugin = "test-plugin-prompts";

function assert(ok: boolean, message: string): void {
  if (!ok) {
    throw new Error(message);
  }
}

Deno.test("prompts 按 plugin_uuid 整体覆盖", () => {
  const written = updatePrompts(testPlugin, [
    { name: "第一条", content: "a" },
    { name: "第二条", content: "b" },
  ]);
  assert(written.prompts.length === 2, "写入后应返回 2 条");

  const read = getPrompts(testPlugin);
  assert(read.plugin_uuid === testPlugin, "plugin_uuid 不正确");
  assert(read.prompts.length === 2, "读取结果应为 2 条");

  updatePrompts(testPlugin, [{ name: "只剩一条" }]);
  assert(getPrompts(testPlugin).prompts.length === 1, "覆盖后应只剩 1 条");

  updatePrompts(testPlugin, []);
  assert(getPrompts(testPlugin).prompts.length === 0, "应支持写入空数组");
});

Deno.test("datasets/prompts.json 可导入", () => {
  const result = importPromptSets(sets);
  assert(result.plugins === sets.length, "插件数量不正确");
  for (const set of sets) {
    const read = getPrompts(set.plugin_uuid);
    assert(
      read.prompts.length === set.prompts.length,
      `${set.plugin_uuid} 的提示词数量不正确`,
    );
  }
});

Deno.test("cleanup", () => {
  updatePrompts(testPlugin, []);
  close();
});
