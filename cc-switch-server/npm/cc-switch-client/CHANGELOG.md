# Changelog

`cc-switch-client` 所有显著变更记录于此。
格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.1] - 2026-06-13

### Added
新增完整的**本地 HTTP 管理 API**（`/api/v1/`，仅监听 `127.0.0.1`），复用 `cc-switch-core` 的 `SkillService` / `McpService`。TS SDK 新增 20+ 个类型化异步方法；`start()` 返回的 `ProxyInfo` 新增 `mgmtPort: number`。

**Skill 管理**
- `listSkills()`、`listUnmanagedSkills()`、`listSkillBackups()`
- `installSkill(discovered, app)`、`installSkillFromZip(path, app)`、`uninstallSkill(id)`
- `toggleSkillApp(id, app, enabled)`、`checkSkillUpdates()`、`updateSkill(id)`
- `importSkills(imports)`、`migrateSkillStorage(target)`、`searchSkills(query, limit, offset)`
- `restoreSkillBackup(backupId, app)`、`deleteSkillBackup(backupId)`
- `listSkillRepos()`、`addSkillRepo(repo)`、`removeSkillRepo(owner, name)`

**MCP 管理**
- `listMcpServers()`、`upsertMcpServer(server)`、`toggleMcpApp(id, app, enabled)`
- `syncMcp()`、`importMcpFromApp(app)`

**HTTP 端点**（`/api/v1/`）

| 分组 | 方法 | 路径 |
|---|---|---|
| 健康 | GET | `/health` |
| Skill 读 | GET | `/skills`、`/skills/unmanaged`、`/skills/backups` |
| Skill 写 | POST | `/skills/install`、`/skills/install-zip`、`/skills/check-updates`、`/skills/import`、`/skills/migrate-storage`、`/skills/search`、`/skills/backups/:id/restore`、`/skills/:id/toggle`、`/skills/:id/update` |
| Skill 写 | DELETE | `/skills/:id`、`/skills/backups/:id` |
| Skill 仓库 | GET / POST | `/skill-repos` |
| Skill 仓库 | DELETE | `/skill-repos/:owner/:name` |
| MCP | GET / POST | `/mcp/servers` |
| MCP | POST | `/mcp/servers/:id/toggle`、`/mcp/sync`、`/mcp/import/:app` |
| MCP | DELETE | `/mcp/servers/:id` |

### Changed
- CI：linux-arm64 交叉编译改用 `openssl-sys` 的 `vendored` 特性（容器缺目标架构 `libssl-dev`）。
- CI：发布步骤改用 `fs.readFileSync` 读取 `package.json`，修复 `require()` 路径解析错误。
- CI：平台包发布失败不再被 `|| echo WARN` 静默吞掉，改为逐个尝试 + 任一失败 `exit 1` 并打 `::error::`。

### Notes
- 包发布的是 `.ts` 源码（`main` / `types` 均为 `index.ts`，`import ... from 'bun'`），运行环境需支持 TS + bun API。
- 管理 API 仅 `127.0.0.1`；`info.mgmtPort` 由 server 启动 JSON 消息提供。
- `installSkillFromZip(path)` 传**本机 zip 绝对路径**（同机共享文件系统，非 multipart）。
- `AppType = 'claude' | 'codex' | 'gemini' | 'opencode' | 'hermes'`；`SkillStorageLocation = 'cc_switch' | 'unified'`。
- server 端新代码全在 `cc-switch-server/src/api/`（`mod.rs` / `skills.rs` / `mcp.rs` / `error.rs`）+ `src/lib.rs`，对 `cc-switch-core` 与 `src-tauri` 零改动。

## [0.1.0] - 2026-06-11

### Added
- 初始发布。`CCSwitchClient` 提供基础代理生命周期管理：`start()` / `stop()` / `getProxyUrl()` / `getPort()` / `getInfo()` / `isRunning()`。
- 平台二进制通过 `optionalDependencies` 按本机架构自动拉取（darwin-arm64 / darwin-x64 / linux-arm64 / linux-x64 / win32-x64）。
- `CCSwitchConfig`：`binaryPath` / `configDir` / `port` / `listenAddr` / `takeover` / `logLevel` / `startTimeout`。
