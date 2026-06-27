# Changelog

`cc-switch-client` 所有显著变更记录于此。
格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.4] - 2026-06-27

同步上游 `cc-switch-core` v3.16.4，并修复 headless 二进制的 rustls TLS panic。

### Fixed
- **rustls CryptoProvider panic**（关键修复）：`cc-switch-server` 转发 HTTPS 上游请求时，因未安装进程级 CryptoProvider，rustls 0.23 的无参 `ClientConfig::builder()` panic（"no process-level CryptoProvider available"），导致连接断开 → 502 fetch failed。现在：
  - `hyper_client.rs` 改用 `builder_with_provider(ring)` 显式指定 provider（自包含，不依赖外部 install）。
  - `main.rs` 启动时 `install_default()`（对齐桌面端），覆盖 reqwest/hyper-rustls 等所有 TLS 路径。
- **上游 zstd 解压**（#3817）：上游压缩的错误响应体现在能正确解压，不再因 `from_utf8` 失败而丢失限流/鉴权等错误详情。

### Added
- **本地代理请求覆盖**（#4589）：供应商支持 `local_proxy_request_overrides` 自定义请求头和请求体（深度合并），含保护字段白名单（authorization、content-type 等不可覆盖）、`stream` 字段保护、Copilot 跳过。
- **统一解压模块** `proxy/content_encoding.rs`（gzip/deflate/zstd），forwarder 与 response_processor 共用。
- 新增 `zstd = "0.13"` 依赖。

### Changed
- 同步上游 v3.16.4 的 118 个未改动文件 + provider.rs（`LocalProxyRequestOverrides` 类型）+ database/mod.rs 等。

## [0.1.3] - 2026-06-24

本次同步上游 `cc-switch-core` 源码（v3.16.3），将桌面端核心模块的修复与新功能回流到独立 `cc-switch-server` 二进制。

### Added
- **Codex 统一会话历史开关**：官方 Codex 供应商的 live 配置在落盘前注入共享 `custom` 路由，开关即时生效（无需等下次切换）。新增 `inject/strip/apply_codex_unified_session_bucket` 及对应的会话历史迁移引擎（`codex_history_migration.rs`）。
- **新增 `reapply_current_codex_official_live`**：统一会话开关变更后立即重写当前官方 Codex 供应商的 live 配置。
- **MCP 路径推导重构**：`get_claude_mcp_path` 支持词法规范化路径比较，并在 Windows WSL UNC 默认目录下使用拆分式 `.claude.json` 路径。
- 将 `CHANGELOG.md` 纳入 npm 发布文件列表（`package.json` 的 `files` 字段）。

### Fixed
- **Codex 系供应商接管保留 `ANTHROPIC_AUTH_TOKEN` 占位符**（#3784）：`ManagedAccount` 接管策略新增 `keep_auth_token` 标志，Codex 系（含仅凭 base_url 识别的）注入占位符避免 Claude Code 弹登录提示；Copilot 维持仅 `API_KEY` 占位（#1049）。
- Volcengine Ark 用量查询（AK/SK 签名）、Chat API 跳过缺失函数名的 tool call、Codex OAuth auth token 在 takeover 时保留（#3789）等多项上游修复同步回流。

### Changed
- 同步 104 个未改动文件 + 22 个上游改动文件到 `cc-switch-core`；13 个解耦文件（去 Tauri 依赖）重新应用机械替换规则并手动 review。

## [0.1.2] - 2026-06-23 ⚠️ 误发版本（请勿使用）

0.1.2 是一次误发：仅更新了 `CHANGELOG.md`，**未同步上游 `cc-switch-core` 源码**，二进制与 0.1.1 等价。已发布的包保留在 npm 上但不应被使用，实际改动见 0.1.3。

本次同步上游 `cc-switch-core` 源码（v3.16.3），将桌面端核心模块的修复与新功能回流到独立 `cc-switch-server` 二进制。

### Added
- **Codex 统一会话历史开关**：官方 Codex 供应商的 live 配置在落盘前注入共享 `custom` 路由，开关即时生效（无需等下次切换）。新增 `inject/strip/apply_codex_unified_session_bucket` 及对应的会话历史迁移引擎（`codex_history_migration.rs`）。
- **新增 `reapply_current_codex_official_live`**：统一会话开关变更后立即重写当前官方 Codex 供应商的 live 配置。
- **MCP 路径推导重构**：`get_claude_mcp_path` 支持词法规范化路径比较，并在 Windows WSL UNC 默认目录下使用拆分式 `.claude.json` 路径。
- 将 `CHANGELOG.md` 纳入 npm 发布文件列表（`package.json` 的 `files` 字段）。

### Fixed
- **Codex 系供应商接管保留 `ANTHROPIC_AUTH_TOKEN` 占位符**（#3784）：`ManagedAccount` 接管策略新增 `keep_auth_token` 标志，Codex 系（含仅凭 base_url 识别的）注入占位符避免 Claude Code 弹登录提示；Copilot 维持仅 `API_KEY` 占位（#1049）。
- Volcengine Ark 用量查询（AK/SK 签名）、Chat API 跳过缺失函数名的 tool call、Codex OAuth auth token 在 takeover 时保留（#3789）等多项上游修复同步回流。

### Changed
- 同步 104 个未改动文件 + 22 个上游改动文件到 `cc-switch-core`；13 个解耦文件（去 Tauri 依赖）重新应用机械替换规则并手动 review。

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
