// designplugin_lib/ts/migrate_skills.ts —— 旧 chatllm skills 迁移到 plugins.db。
//
// 来源：
//   - database/dialogue.db 的 skills 表（旧索引 / 完整字段）
//   - designdb/skills 的 skills 集合（正文）
//   - dataset/chat-skills 种子（分类与缺失字段兜底）
// 目标：plugins.db 的 skills / skill_categories；同时补 plugin_functions.use_ai。
//
// 用法（在 designplugin_lib 仓库根目录）：
//   deno run -A ts/migrate_skills.ts --dataset=../../dytes_sys_operations/dataset/chat-skills
//   deno run -A ts/migrate_skills.ts ... --dry-run

import {
  datasetPath,
  DESIGNDB_URL,
  ensureSkillSchema,
  execute,
  loadJson,
  migrateUseAi,
  parseArgs,
  PLUGIN_DB,
  query,
  quote,
  seedFromJson,
  writeSkill,
  type SkillRow,
} from "./lib.ts";

const args = parseArgs();
const base = datasetPath(args);
const dryRun = args["dry-run"] === "true";
const legacyDb = args["legacy-db"] ?? "./dialogue";

const categoriesJson = loadJson(`${base}/categories.json`);
const indexJson = loadJson(`${base}/skills_index.json`);
const contentJson = loadJson(`${base}/skills_content.json`);

const categorySeeds = (categoriesJson.categories ?? []) as Array<Record<string, unknown>>;
const index = (indexJson.skills ?? []) as Array<Record<string, unknown>>;
const content = (contentJson.skills ?? []) as Array<Record<string, unknown>>;
const seeded = seedFromJson(categorySeeds, index, content).skills;

const merged = new Map<string, SkillRow>();
for (const skill of seeded) merged.set(skill.id, skill);

// 旧 SQLite skills 表（不存在时跳过）。
let legacyRows: Array<Record<string, unknown>> = [];
try {
  legacyRows = await query("SELECT * FROM skills", legacyDb);
  console.log(`旧 dialogue.skills: ${legacyRows.length} 行`);
} catch (error) {
  console.warn(`[skip] 旧 dialogue.skills 不可读: ${(error as Error).message}`);
}

for (const row of legacyRows) {
  const id = String(row.id ?? "");
  if (!id) continue;
  const current = merged.get(id);
  merged.set(id, {
    id,
    name: String(row.name ?? current?.name ?? id),
    description: String(row.description ?? current?.description ?? ""),
    category: String(row.category ?? current?.category ?? "general"),
    content: String(row.content ?? current?.content ?? ""),
    trigger: (row.trigger ?? current?.trigger ?? null) as string | null,
    params_schema: row.params_schema ?? row.params_json ?? current?.params_schema ?? {},
    is_system: Number(row.is_system ?? current?.is_system ?? 0),
    is_enabled: Number(row.is_enabled ?? current?.is_enabled ?? 1),
    sort: Number(row.sort ?? current?.sort ?? 0),
    version: Number(row.version ?? current?.version ?? 1),
  });
}

// 旧 designdb skills 集合（正文）。
try {
  const response = await fetch(DESIGNDB_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/libary",
      "FFI-Symbol": "db_query",
    },
    body: JSON.stringify({
      path: "skills",
      collection: "skills",
      filter: {},
      limit: 1000,
    }),
  });
  const json = (await response.json().catch(() => null)) as
    | Record<string, unknown>
    | null;
  const data = json?.data as unknown;
  const docs = Array.isArray(data)
    ? (data as Array<Record<string, unknown>>)
    : ((data as Record<string, unknown> | null)?.list as
      | Array<Record<string, unknown>>
      | undefined) ?? [];
  console.log(`旧 designdb.skills: ${docs.length} 条`);

  for (const doc of docs) {
    const id = String(doc._id ?? doc.id ?? "");
    if (!id) continue;
    const current = merged.get(id);
    merged.set(id, {
      id,
      name: String(doc.name ?? current?.name ?? id),
      description: String(doc.description ?? current?.description ?? ""),
      category: String(doc.category ?? current?.category ?? "general"),
      content: String(doc.content ?? current?.content ?? ""),
      trigger: (doc.trigger ?? current?.trigger ?? null) as string | null,
      params_schema: doc.params_schema ?? current?.params_schema ?? {},
      is_system: Number(doc.is_system ?? current?.is_system ?? 0),
      is_enabled: Number(doc.is_enabled ?? current?.is_enabled ?? 1),
      sort: Number(doc.sort ?? current?.sort ?? 0),
      version: Number(doc.version ?? current?.version ?? 1),
    });
  }
} catch (error) {
  console.warn(`[skip] 旧 designdb.skills 不可读: ${(error as Error).message}`);
}

const skills = [...merged.values()];
const categoryCodes = new Set(categorySeeds.map((item) => String(item.id ?? "")));
for (const skill of skills) {
  if (!categoryCodes.has(skill.category)) {
    categoryCodes.add(skill.category);
  }
}

console.log(`合并结果：categories=${categoryCodes.size} skills=${skills.length}`);
if (dryRun) {
  console.log(skills.slice(0, 5).map((item) => `${item.id} (${item.category})`).join("\n"));
  Deno.exit(0);
}

await ensureSkillSchema();
await migrateUseAi();

for (const category of categorySeeds) {
  const code = String(category.id ?? "");
  if (!code) continue;
  await execute(`INSERT OR IGNORE INTO skill_categories (code, name, description, sort)
    VALUES (${quote(code)}, ${quote(category.name ?? code)}, ${quote(category.description ?? "")}, ${Number(category.sort ?? 0)})`);
}
for (const code of categoryCodes) {
  const known = categorySeeds.some((item) => String(item.id ?? "") === code);
  if (known) continue;
  await execute(`INSERT OR IGNORE INTO skill_categories (code, name, description, sort)
    VALUES (${quote(code)}, ${quote(code)}, '', 0)`);
}

for (const skill of skills) {
  await writeSkill(skill);
}

console.log(`迁移完成：${PLUGIN_DB} skills=${skills.length}`);
