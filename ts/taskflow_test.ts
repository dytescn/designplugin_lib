// Deno 集成测试：验证 taskflow 整体读写。
//
// 运行（cwd 必须是 .dytes）：
//   cd .dytes && deno test --allow-ffi --allow-read --allow-write ../ts/taskflow_test.ts

import { close, getTaskflow, updateTaskflow } from "./ffi.ts";

const dataUrl = new URL("../datasets/taskflow.json", import.meta.url);
const dataset = JSON.parse(await Deno.readTextFile(dataUrl)) as unknown[];

function assert(ok: boolean, message: string): void {
  if (!ok) {
    throw new Error(message);
  }
}

Deno.test("taskflow 整体覆盖", () => {
  updateTaskflow([{ name: "第一步", desc: "描述", uid: "step-1" }]);
  const read = getTaskflow();
  assert(read.taskflows.length === 1, "写入后应为 1 条");
  assert(
    (read.taskflows[0] as { uid?: string }).uid === "step-1",
    "uid 不正确",
  );

  updateTaskflow([]);
  assert(getTaskflow().taskflows.length === 0, "应支持写入空数组");
});

Deno.test("datasets/taskflow.json 可导入", () => {
  const result = updateTaskflow(dataset);
  assert(
    result.taskflows.length === dataset.length,
    "导入后的数量不正确",
  );
});

Deno.test("cleanup", () => {
  updateTaskflow([]);
  close();
});
