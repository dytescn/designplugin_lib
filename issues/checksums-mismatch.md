# [P1] `dytes.json` 的 `mcp.md` checksum 与实际文件不一致

- 发现日期：2026-11-10
- 包：`plugin_lib`（仓库 `designplugin_lib`，`type: sys`）
- 线上版本：`1.0.16`（本地清单当前为 `1.0.17`）
- 文件：`system/plugin_lib/1.0.16/windows/mcp.md`

## 现象

使用 `dytes install` / `dytes update` 安装 Piksel 时：

```text
sys plugin_lib: 校验失败，已删除文件: .../libary/mcp.md:
文件大小不匹配（期望 18788，实际 19314）
```

清单 `dytes.json` 记录：

```json
"checksums": {
  "mcp.md": {
    "size": 18788,
    "sha256": "8d1bfc3c5865fe5cc1884be420c98f0cbfe9ab75745d2b962420ee1b31045d1e"
  }
}
```

实际下载的文件：

```text
size:   19314
sha256: 0b1308630ffd8048714c4346df376ce047ab19f4d7c023bbd28f90c816a557cb
```

## 影响

- `dytes` 校验失败后删除 `mcp.md`，`plugin_lib` 安装中止；
- `plugin_lib.dll` 未安装，插件数据库/技能/提示词等能力不可用；
- `dytes install` 最终返回非 0。

## 修复建议

1. 基于最终上传的 `mcp.md` 重新生成 `checksums.size` / `checksums.sha256`；
2. 发布新的 patch 版本（当前线上 `1.0.16`，建议发布 `1.0.18`，不要覆盖已有版本路径）；
3. 刷新 `packages.funxdata.com` 对应 CDN 缓存；
4. 使用 `dytes update --type sys` 验证不再出现大小/哈希不匹配。

## 复现

```bash
curl -fsSL "https://packages.funxdata.com/system/plugin_lib/1.0.16/dytes.json" | grep -A8 checksums
curl -fsSL "https://packages.funxdata.com/system/plugin_lib/1.0.16/windows/mcp.md" | wc -c
```
