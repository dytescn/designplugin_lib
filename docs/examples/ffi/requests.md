# HTTP 调用示例

`plugin_lib` 由 cyctron 暴露为 `POST /plugin_lib`。请求头必须包含
`Content-Type: application/libary` 与 `FFI-Symbol`，请求体是 JSON。

以下命令在 Git Bash / Linux / macOS 下可直接执行；Windows 的 PowerShell 请使用
`curl.exe`，并注意引号转义。

## 1. 安装示例包

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: install_plugin" \
  -d '{"dir":"./plugins/demo_design"}'
```

成功时返回：

```json
{
  "code": 200,
  "data": {
    "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
    "name": "demo_design",
    "version": "1.0.0",
    "install_path": "./plugins/demo_design",
    "platform": "win",
    "lib_file": "demo_design.dll",
    "skipped": false,
    "counts": { "skills": 2, "prompts": 2, "taskflows": 2, "functions": 1 }
  },
  "msg": "success"
}
```

## 2. 查询插件

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_plugins" \
  -d '{"uuid":"6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10"}'
```

## 3. 查询技能

列表只返回摘要，不包含 `content`：

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_skills" \
  -d '{"plugin_uuid":"6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10","page":1,"page_size":20}'
```

需要内容时，按 `id` 单条读取：

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_skill" \
  -d '{"id":"demo_design_layout"}'
```

## 4. 查询能力、提示词、任务流

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_functions" \
  -d '{"plugin_uuid":"6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10","page":1,"page_size":20}'

curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_prompts" \
  -d '{"plugin_uuid":"6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10"}'

curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: get_taskflow" \
  -d '{}'
```

## 5. 单条新增技能

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: post_skills" \
  -d '{
    "id": "demo_design_manual_skill",
    "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
    "name": "手动技能",
    "category": "design",
    "description": "通过 FFI 单条写入的技能",
    "content": "这是一条通过 post_skills 写入的技能正文。",
    "trigger": "用户提到手动技能",
    "params_schema": {"type": "object"},
    "is_enabled": 1,
    "sort": 30
  }'
```

## 6. 整体覆盖提示词

```bash
curl -X POST http://127.0.0.1:44944/plugin_lib \
  -H "Content-Type: application/libary" \
  -H "FFI-Symbol: update_prompts" \
  -d '{
    "plugin_uuid": "6f0d2f3a-4b8c-4d2e-9a71-0c5b8e2d4a10",
    "prompts": [
      {
        "id": "layout-advisor",
        "name": "版式顾问",
        "content": "根据画布尺寸给出栅格与边距建议。",
        "category": "design",
        "sort": 10,
        "is_enabled": 1
      }
    ]
  }'
```

> `update_prompts` 是整体覆盖。该接口只写 designdb，不同步 kv；kv 同步发生在
> `install_plugin` / `update_plugin` 流程中，且要求 `plugins_type` 命中映射。

## 7. 错误响应

业务错误仍使用 HTTP 200，错误在响应体的 `code` / `msg` 中：

```json
{
  "code": 400409,
  "data": {},
  "msg": "技能 id 已存在: demo_design_manual_skill"
}
```

调用方必须同时检查传输层状态与响应体的 `code`。
