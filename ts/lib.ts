// designplugin_lib/ts/lib.ts —— 迁移 / 导入脚本的共享工具。
//
// 这些脚本不在 update 的 setup.ts 静态提取范围内，可直接用 Deno 运行：
//   deno run -A ts/import_skills.ts --dataset=../../dytes_sys_operations/dataset/chat-skills

export const API_URL = "http://127.0.0.1:44944/database";
export const DESIGNDB_URL = "http://127.0.0.1:44944/libdesigndb";
export const PLUGIN_DB = "./plugins";

export interface SkillRow {
  id: string;
  name: string;
  description: string;
  category: string;
  content: string;
  trigger: string | null;
  params_schema: unknown;
  is_system: number;
  is_enabled: number;
  sort: number;
  version: number;
}

export function quote(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "number") return String(value);
  if (typeof value === "boolean") return value ? "1" : "0";
  return `'${String(value).replace(/'/g, "''")}'`;
}

async function call(
  url: string,
  symbol: string,
  body: unknown,
): Promise<Record<string, unknown>> {
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/libary",
      "FFI-Symbol": symbol,
    },
    body: JSON.stringify(body),
  });
  const json = (await response.json().catch(() => null)) as
    | Record<string, unknown>
    | null;
  if (!json) {
    throw new Error(`${symbol} 返回非 JSON（HTTP ${response.status}）`);
  }
  const code = Number(json.code ?? -1);
  if (code !== 200 && code !== 0) {
    throw new Error(`${symbol} 失败 (code=${code}): ${json.msg ?? ""}`);
  }
  return json;
}

export async function execute(sql: string, db = PLUGIN_DB): Promise<void> {
  await call(API_URL, "db_update", { path: db, sql });
}

export async function query(sql: string, db = PLUGIN_DB): Promise<Array<Record<string, unknown>>> {
  const json = await call(API_URL, "db_query", { path: db, sql });
  const data = json.data;
  if (Array.isArray(data)) return data as Array<Record<string, unknown>>;
  if (data && typeof data === "object") {
    const object = data as Record<string, unknown>;
    for (const key of ["list", "rows"]) {
      if (Array.isArray(object[key])) {
        return object[key] as Array<Record<string, unknown>>;
      }
    }
  }
  return [];
}

export async function tryExecute(sql: string, db = PLUGIN_DB): Promise<boolean> {
  try {
    await execute(sql, db);
    return true;
  } catch (error) {
    console.warn(`[skip] ${sql.slice(0, 80)}: ${(error as Error).message}`);
    return false;
  }
}

export async function ensureSkillSchema(): Promise<void> {
  await execute(`CREATE TABLE IF NOT EXISTS skill_categories (
    code        TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    sort        INTEGER NOT NULL DEFAULT 0,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
  )`);
  await execute(`CREATE TABLE IF NOT EXISTS skills (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL DEFAULT '',
    description   TEXT NOT NULL DEFAULT '',
    category      TEXT NOT NULL DEFAULT 'General',
    content       TEXT NOT NULL DEFAULT '',
    trigger       TEXT,
    params_schema TEXT,
    is_system     INTEGER NOT NULL DEFAULT 0,
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    sort          INTEGER NOT NULL DEFAULT 0,
    version       INTEGER NOT NULL DEFAULT 1,
    created_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
  )`);
  await execute(
    `CREATE INDEX IF NOT EXISTS idx_skills_category_sort ON skills(category, sort)`,
  );
  await execute(
    `CREATE INDEX IF NOT EXISTS idx_skills_enabled ON skills(is_enabled)`,
  );
}

export async function migrateUseAi(): Promise<void> {
  await tryExecute("ALTER TABLE plugin_functions ADD COLUMN use_ai INTEGER DEFAULT 1");
  await tryExecute("UPDATE plugin_functions SET use_ai = 1 WHERE use_ai IS NULL");
  await tryExecute("DROP INDEX IF EXISTS idx_plugin_functions_local");
  await tryExecute(
    "CREATE INDEX IF NOT EXISTS idx_plugin_functions_local ON plugin_functions(is_local, embed_status, use_ai)",
  );
}

export function loadJson(path: string): Record<string, unknown> {
  return JSON.parse(Deno.readTextFileSync(path)) as Record<string, unknown>;
}

export function parseArgs(): Record<string, string> {
  const args: Record<string, string> = {};
  for (const raw of Deno.args) {
    if (!raw.startsWith("--")) continue;
    const [key, ...rest] = raw.slice(2).split("=");
    args[key] = rest.length ? rest.join("=") : "true";
  }
  return args;
}

export function datasetPath(args: Record<string, string>): string {
  return args.dataset ?? "./dataset/chat-skills";
}

export async function writeSkill(skill: SkillRow): Promise<void> {
  const params = JSON.stringify(skill.params_schema ?? {});
  await execute(`INSERT INTO skills (
      id, name, description, category, content, trigger, params_schema,
      is_system, is_enabled, sort, version, updated_at
    ) VALUES (
      ${quote(skill.id)}, ${quote(skill.name)}, ${quote(skill.description)},
      ${quote(skill.category)}, ${quote(skill.content)}, ${quote(skill.trigger)},
      ${quote(params)}, ${skill.is_system}, ${skill.is_enabled}, ${skill.sort},
      ${skill.version}, CURRENT_TIMESTAMP
    )
    ON CONFLICT(id) DO UPDATE SET
      name = excluded.name,
      description = excluded.description,
      category = excluded.category,
      content = excluded.content,
      trigger = excluded.trigger,
      params_schema = excluded.params_schema,
      is_system = excluded.is_system,
      is_enabled = excluded.is_enabled,
      sort = excluded.sort,
      version = excluded.version,
      updated_at = CURRENT_TIMESTAMP`);
}

export function seedFromJson(
  categories: Array<Record<string, unknown>>,
  index: Array<Record<string, unknown>>,
  content: Array<Record<string, unknown>>,
): { categories: Array<Record<string, unknown>>; skills: SkillRow[] } {
  const contentById = new Map<string, Record<string, unknown>>();
  for (const item of content) {
    const id = String(item.id ?? item._id ?? "");
    if (id) contentById.set(id, item);
  }

  const skills: SkillRow[] = [];
  for (const item of index) {
    const id = String(item.id ?? "");
    if (!id) continue;
    const detail = contentById.get(id) ?? {};
    skills.push({
      id,
      name: String(item.name ?? detail.name ?? id),
      description: String(item.description ?? detail.description ?? ""),
      category: String(item.category ?? detail.category ?? "general"),
      content: String(detail.content ?? ""),
      trigger: detail.trigger == null ? null : String(detail.trigger),
      params_schema: detail.params_schema ?? {},
      is_system: Number(item.is_system ?? 0),
      is_enabled: Number(item.is_enabled ?? 1),
      sort: Number(item.sort ?? 0),
      version: Number(item.version ?? detail.version ?? 1),
    });
  }
  return { categories, skills };
}
