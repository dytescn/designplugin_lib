// designplugin_lib/ts/import_skills.ts —— 把 dataset/chat-skills 种子导入 plugins.db。
//
// 用法（在 designplugin_lib 仓库根目录）：
//   deno run -A ts/import_skills.ts --dataset=../../dytes_sys_operations/dataset/chat-skills
//   deno run -A ts/import_skills.ts --dataset=... --dry-run

import {
  datasetPath,
  ensureSkillSchema,
  execute,
  loadJson,
  parseArgs,
  quote,
  seedFromJson,
  writeSkill,
} from "./lib.ts";

const args = parseArgs();
const base = datasetPath(args);
const dryRun = args["dry-run"] === "true";

const categoriesJson = loadJson(`${base}/categories.json`);
const indexJson = loadJson(`${base}/skills_index.json`);
const contentJson = loadJson(`${base}/skills_content.json`);

const categories = (categoriesJson.categories ?? []) as Array<Record<string, unknown>>;
const index = (indexJson.skills ?? []) as Array<Record<string, unknown>>;
const content = (contentJson.skills ?? []) as Array<Record<string, unknown>>;

const { skills } = seedFromJson(categories, index, content);
const categoryCodes = new Set(categories.map((item) => String(item.id ?? "")));

if (dryRun) {
  console.log(`dry-run: categories=${categories.length} skills=${skills.length}`);
  console.log(skills.slice(0, 3).map((item) => `${item.id} (${item.category})`).join("\n"));
  Deno.exit(0);
}

await ensureSkillSchema();

for (const category of categories) {
  const code = String(category.id ?? "");
  if (!code) continue;
  await execute(`INSERT OR IGNORE INTO skill_categories (code, name, description, sort)
    VALUES (${quote(code)}, ${quote(category.name ?? code)}, ${quote(category.description ?? "")}, ${Number(category.sort ?? 0)})`);
}

for (const skill of skills) {
  if (!categoryCodes.has(skill.category)) {
    await execute(`INSERT OR IGNORE INTO skill_categories (code, name, description, sort)
      VALUES (${quote(skill.category)}, ${quote(skill.category)}, '', 0)`);
    categoryCodes.add(skill.category);
  }
  await writeSkill(skill);
}

console.log(`导入完成：categories=${categories.length} skills=${skills.length}（${base}）`);
