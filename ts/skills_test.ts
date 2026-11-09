// Deno 集成测试：通过 FFI 验证 skills 的增删改查。
//
// 运行（cwd 必须是 .dytes）：
//   cd .dytes && deno test --allow-ffi --allow-read --allow-write ../ts/skills_test.ts

import {
  close,
  deleteSkill,
  getSkill,
  getSkills,
  importSkills,
  patchSkill,
  type Skill,
} from "./ffi.ts";

const dataUrl = new URL("../datasets/skills.json", import.meta.url);
const skills = JSON.parse(await Deno.readTextFile(dataUrl)).skills as Skill[];

function assert(ok: boolean, message: string): void {
  if (!ok) {
    throw new Error(message);
  }
}

function cleanup(): void {
  for (const skill of skills) {
    try {
      deleteSkill(skill.id);
    } catch {
      // 不存在时忽略
    }
  }
}

Deno.test("skills 可通过 FFI 完成增删改查", () => {
  cleanup();

  const imported = importSkills(skills);
  assert(imported.created === skills.length, "首次导入应全部新增");
  assert(imported.updated === 0, "首次导入不应有更新");

  const first = skills[0];
  const listed = getSkills({
    plugin_uuid: first.plugin_uuid,
    with_content: true,
    page_size: 200,
  });
  const expected = skills.filter((s) =>
    s.plugin_uuid === first.plugin_uuid
  ).length;
  assert(listed.total === expected, `按插件查询应返回 ${expected} 条`);

  const updated = patchSkill(first.id, { name: "测试改名", is_enabled: 0 });
  assert(updated.name === "测试改名", "name 未更新");
  assert(updated.is_enabled === 0, "is_enabled 未更新");
  assert(updated.version === 2, "version 应递增到 2");
  assert(updated.content === first.content, "patch 不应清空未修改字段");

  const fetched = getSkill(first.id);
  assert(fetched.name === "测试改名", "重新读取的 name 不正确");
  assert(fetched.plugin_uuid === first.plugin_uuid, "plugin_uuid 丢失");

  const again = importSkills([first]);
  assert(again.updated === 1, "重复导入应走更新");
  const afterImport = getSkill(first.id);
  assert(afterImport.version === 3, "重复导入应递增 version");

  deleteSkill(first.id);
  const afterDelete = getSkills({ ids: [first.id] });
  assert(afterDelete.total === 0, "删除后不应再查到该技能");

  cleanup();
});

Deno.test("cleanup", () => {
  cleanup();
  close();
});
