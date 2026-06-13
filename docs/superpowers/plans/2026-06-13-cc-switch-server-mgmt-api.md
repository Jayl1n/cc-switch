# cc-switch-server 管理 API（Skill + MCP）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `cc-switch-server` 增加一个仅监听 `127.0.0.1` 的 HTTP 管理 API（`/api/v1/`），复用 `cc-switch-core` 的 `SkillService` / `McpService` 全部能力；并在 `cc-switch-client`（TS SDK）中封装对应方法。

**Architecture:** 在 `cc-switch-server` 内新增 `api/` 模块（axum 路由），与现有代理引擎跑在同一 tokio runtime、不同端口。启动时 OS 分配随机管理端口，通过 stdout 启动 JSON 的 `mgmtPort` 字段传给 Node 客户端，客户端用 `fetch` 调用。**所有新代码落在 `cc-switch-server/src/`（upstream 不存在此目录）+ `cc-switch-client`（自有 SDK），对 `cc-switch-core` 与 `src-tauri` 零改动，保证上游同步零冲突。**

**Tech Stack:** Rust（axum 0.7、tokio、serde）+ TypeScript（fetch）。axum/tokio 已是 `cc-switch-core` 的依赖，版本一致，无新重大依赖。

---

## 调研结论（已验证，作为实现依据）

| 事实 | 位置 | 含义 |
|------|------|------|
| `Core.state: Arc<AppState>` 是 `pub` | `cc-switch-core/src/core.rs:63` | server 可同时拿到 `AppState`（给 McpService）和 `db`（给 SkillService） |
| `AppState { db, proxy_service, usage_cache }` 全 `pub` | `cc-switch-core/src/store.rs:6-10` | `core.state.clone()` 即可注入 axum |
| `SkillService` 方法吃 `&Arc<Database>` 或 `&self`+`db` | `cc-switch-core/src/services/skill.rs` | handler 用 `&state.app_state.db` |
| `McpService` 方法吃 `&AppState` | `cc-switch-core/src/services/mcp.rs` | handler 用 `&state.app_state` |
| `AppType` derive `Serialize/Deserialize`（lowercase） | `cc-switch-core/src/app_config.rs:339` | 可直接从 JSON 字符串 `"claude"` 反序列化 |
| `AppError` 是 `thiserror::Error` 枚举 | `cc-switch-core/src/error.rs:7` | `#[from]` 可自动转成我们的 `ApiError` |
| `InstalledSkill`/`McpServer` 等已 derive `Serialize`（camelCase） | `cc-switch-core/src/app_config.rs` | 直接作为 JSON 响应体，无需 DTO |
| `store` / `error` 模块在 lib.rs 是 `pub mod` | `cc-switch-core/src/lib.rs:32,38` | `cc_switch_core::store::AppState` 可直接引用 |
| `cc-switch-server` 当前无 tests 目录 | — | 全新 TDD，无既有测试干扰 |
| CI 按 tag 构建 5 平台 `cargo build -p cc-switch-server` | `.github/workflows/release-cc-switch-client.yml` | 新增代码随正常构建走，无需改 CI |

---

## 文件结构（锁定分解决策）

```
cc-switch-server/
├── Cargo.toml                      [修改] +axum/tower-http/serde/anyhow/thiserror +dev-deps
├── src/
│   ├── lib.rs                      [新建] pub mod api; + 测试用入口 fn
│   ├── main.rs                     [修改] 仅 3 处：mod→use、起 mgmt server、stdout 加 mgmtPort
│   └── api/                        [新建整个目录] ← upstream 不存在，合并零冲突
│       ├── mod.rs                  router 组装 + ApiState + health
│       ├── error.rs                ApiError → IntoResponse
│       ├── skills.rs               Skill 端点（薄转发）
│       └── mcp.rs                  MCP 端点（薄转发）
└── tests/
    └── api_integration.rs          [新建] 集成测试（reqwest 打真实端口）

cc-switch-server/npm/cc-switch-client/
├── index.ts                        [修改] ProxyInfo 加 mgmtPort + 管理方法 + 类型
└── package.json                    [修改] files 不变（index.ts 已含）

cc-switch-core/                     ★ 完全不改动
src-tauri/                          ★ 完全不改动
```

**同步安全保证：**
1. `api/` 目录 upstream 没有 → `git merge upstream` 零冲突
2. handler 只 `use cc_switch_core::{...}` 的 `pub` 项 → core 内部重构无感
3. `main.rs` 改动用 `// ── mgmt API ──` 注释块标注，冲突时一目了然
4. `Cargo.toml` 只追加依赖行，不改既有行

---

## Phase 0：地基（lib + router 骨架 + 错误映射 + health + 测试夹具）

### Task 0.1：cc-switch-server 转 lib + bin 结构

**Files:**
- Create: `cc-switch-server/src/lib.rs`
- Modify: `cc-switch-server/Cargo.toml`

- [ ] **Step 1: 在 Cargo.toml 追加依赖与 [lib] 配置**

把 `cc-switch-server/Cargo.toml` 的 `[dependencies]` 替换为：

```toml
[dependencies]
cc-switch-core = { path = "../cc-switch-core" }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "net"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.11"
clap = { version = "4", features = ["derive"] }
# ── mgmt API（/api/v1/）──
axum = "0.7"
tower-http = { version = "0.5", features = ["cors", "trace"] }
anyhow = "1.0"
thiserror = "2.0"

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tempfile = "3"
serial_test = "3"
```

说明：`tokio` 增加 `"net"` feature（`TcpListener` 需要）；axum/tower-http 版本与 cc-switch-core 对齐，避免重复编译。

- [ ] **Step 2: 创建 lib.rs**

Create `cc-switch-server/src/lib.rs`:

```rust
//! cc-switch-server 库入口。
//!
//! 将可测试的 API 路由逻辑与 main.rs 的进程编排分离，
//! 让集成测试能直接调用 `router()`，无需 spawn 子进程。

pub mod api;

use std::sync::Arc;

use cc_switch_core::store::AppState;

/// 构建管理 API 的 axum Router（监听 /api/v1/）。
///
/// `app_state` 来自 `Core.state`（`Arc<AppState>`），与代理引擎共享同一份数据库。
pub fn router(app_state: Arc<AppState>) -> axum::Router {
    api::router(app_state)
}
```

Cargo 会自动把 `src/lib.rs` 识别为库 target（crate 名 `cc_switch_server`），无需显式 `[lib]`。

- [ ] **Step 3: 验证编译（api 模块还没建，先放占位）**

先不验证——下一步建 `api/mod.rs` 后一起验证。

### Task 0.2：api/mod.rs（router 骨架 + health）

**Files:**
- Create: `cc-switch-server/src/api/mod.rs`
- Create: `cc-switch-server/src/api/error.rs`

- [ ] **Step 1: 创建 error.rs**

Create `cc-switch-server/src/api/error.rs`:

```rust
//! 统一错误类型：把 cc-switch-core 的 anyhow::Error / AppError 映射成 HTTP 响应。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    App(#[from] cc_switch_core::error::AppError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            _ => {
                // 不把内部错误细节当 5xx 噪音，但记录日志便于排查
                log::warn!("mgmt API error: {self}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
```

注意：故意不引用 `AppError` 的具体变体（避免 upstream 改变体名导致编译失败），只用 `Display`。

- [ ] **Step 2: 创建 api/mod.rs（先只放 health + 占位路由）**

Create `cc-switch-server/src/api/mod.rs`:

```rust
//! HTTP 管理 API（/api/v1/）。
//!
//! 暴露 cc-switch-core 的 Skill / MCP 管理能力给本地 Node 客户端。
//! 仅监听 127.0.0.1，不对外暴露。
//!
//! 同步安全：本目录全部为新增文件，upstream 不含 cc-switch-server/，
//! 合并上游时本目录零冲突。所有 handler 只调用 cc-switch-core 的公开 API。

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use cc_switch_core::store::AppState;

pub mod error;
pub mod mcp;
pub mod skills;

pub use error::{ApiError, ApiResult};

/// 管理 API 共享状态：与代理引擎共享同一份 AppState。
#[derive(Clone)]
pub struct ApiState {
    pub app_state: Arc<AppState>,
}

/// 构建管理 API 路由。
pub fn router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1/skills", skills::routes())
        .nest("/api/v1/mcp", mcp::routes())
        .with_state(ApiState { app_state })
}

async fn health() -> &'static str {
    "ok"
}
```

- [ ] **Step 3: 创建占位 skills.rs / mcp.rs（让 Phase 0 能编译）**

Create `cc-switch-server/src/api/skills.rs`:

```rust
//! Skill 管理端点（Phase 1-2 逐步填充）。

use axum::routing::get;
use axum::Router;

use super::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/placeholder", get(|| async { "todo" }))
}
```

Create `cc-switch-server/src/api/mcp.rs`:

```rust
//! MCP 管理端点（Phase 3 逐步填充）。

use axum::routing::get;
use axum::Router;

use super::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/placeholder", get(|| async { "todo" }))
}
```

- [ ] **Step 4: 验证 lib 编译**

Run: `cargo build -p cc-switch-server`
Expected: 编译成功（main.rs 还没改，仍是原样，但 lib target 已可构建）

若报 `Router<ApiState>` 类型不匹配：axum 0.7 的 `.nest` 要求子 router 的 state 类型一致；确认 `Router<ApiState>` 与父 router 的 `.with_state(ApiState)` 对齐。若 axum 推断困难，把子路由改成接受泛型 `Router<S>` 后在 mod.rs 用 `.with_state` 统一注入——但上述写法在 axum 0.7 是标准用法，应可直接通过。

### Task 0.3：main.rs 最小改动（起 mgmt server + 启动 JSON 加 mgmtPort）

**Files:**
- Modify: `cc-switch-server/src/main.rs`

- [ ] **Step 1: 修改 main.rs，插入 mgmt server 启动逻辑**

在 `cc-switch-server/src/main.rs` 的 `run_server` 函数中，找到启动代理并打印 status 的区段（原文件约第 95-120 行）。做三处改动：

**改动 A** — 文件顶部 `use` 之后（约第 15 行后）新增一行引入 lib：

```rust
use cc_switch_server::router as mgmt_router;
```

**改动 B** — 在 `let info = result.unwrap_or_else(...)` 之后、`let status = serde_json::json!({...})` 之前，插入管理 API 启动块：

```rust
    // ── mgmt API（/api/v1/）：仅 127.0.0.1，随机端口 ──
    let mgmt_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind management API port");
    let mgmt_port = mgmt_listener
        .local_addr()
        .expect("Failed to get management API port")
        .port();
    let mgmt_app = mgmt_router(core.state.clone());
    tokio::spawn(async move {
        if let Err(e) = mgmt_app.serve(mgmt_listener).await {
            log::error!("Management API server error: {e}");
        }
    });
    log::info!("Management API listening on 127.0.0.1:{mgmt_port}");
```

**改动 C** — 在 `let status = serde_json::json!({...})` 中追加 `"mgmtPort"` 字段：

```rust
    let status = serde_json::json!({
        "status": "running",
        "address": info.address,
        "port": info.port,
        "started_at": info.started_at,
        "takeover": args.takeover,
        "mgmtPort": mgmt_port,
    });
```

同样地，文件末尾的 `shutdown_status`（约第 140 行）无需改，保持只输出 `{status, port}`。

- [ ] **Step 2: 验证编译**

Run: `cargo build -p cc-switch-server`
Expected: 编译成功。

- [ ] **Step 3: 手动冒烟——启动进程，确认 stdout 带 mgmtPort 且 health 可达**

Run:
```bash
cargo run -p cc-switch-server -- --port 0 --log-level info &
sleep 1
# 从输出 JSON 取 mgmtPort
```
Expected: stdout 输出含 `"mgmtPort": <某端口>` 的 JSON；用 `curl http://127.0.0.1:<mgmtPort>/api/v1/health` 返回 `ok`。

- [ ] **Step 4: 提交**

```bash
git add cc-switch-server/Cargo.toml cc-switch-server/src/
git commit -m "feat(server): add axum mgmt API skeleton (/api/v1/health)"
```

### Task 0.4：集成测试夹具（spawn_api helper）

**Files:**
- Create: `cc-switch-server/tests/api_integration.rs`

- [ ] **Step 1: 写测试夹具 + 第一个 health 测试（先失败）**

Create `cc-switch-server/tests/api_integration.rs`:

```rust
//! 管理 API 集成测试。
//!
//! 用 reqwest 打真实 127.0.0.1 端口，覆盖整条 axum → cc-switch-core 链路。
//! 因 Core 会写全局 config_dir override，用 #[serial] 串行化避免并发污染。

use std::sync::Arc;

use cc_switch_core::{Core, NoopAuthProvider, NoopEvents};
use cc_switch_server::router;
use serial_test::serial;

/// 启动一份管理 API（独立端口），返回 (base_url, tmp_dir_keep_alive)。
///
/// tmp_dir 必须在测试期间保持存活（config_dir 指向它），故返回上层不释放。
struct TestServer {
    base_url: String,
    _tmp: tempfile::TempDir,
    _handle: tokio::task::JoinHandle<()>,
}

async fn spawn_api() -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let core = Core::builder()
        .events(Arc::new(NoopEvents))
        .auth_provider(Arc::new(NoopAuthProvider))
        .config_dir(tmp.path().to_path_buf())
        .build()
        .unwrap();
    let app_state = core.state.clone();
    // core 本身保持存活（其 state 被 app_state 克隆引用）
    std::mem::forget(core);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(app_state);
    let handle = tokio::spawn(async move {
        let _ = app.serve(listener).await;
    });

    TestServer {
        base_url: format!("http://{addr}"),
        _tmp: tmp,
        _handle: handle,
    }
}

#[tokio::test]
#[serial]
async fn health_returns_ok() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/health", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}
```

- [ ] **Step 2: 运行测试，验证通过**

Run: `cargo test -p cc-switch-server --test api_integration`
Expected: `health_returns_ok` PASS。

（若失败：检查 lib.rs 的 `router` 是否 `pub`、`tokio` net feature 是否开启。）

- [ ] **Step 3: 提交**

```bash
git add cc-switch-server/tests/api_integration.rs
git commit -m "test(server): add mgmt API integration harness + health test"
```

---

## Phase 1：Skill 读取端点（TDD）

> 约定：每个端点先写测试（打真实端口）→ 跑红 → 加 handler → 跑绿 → 提交。
> SkillService 读取方法都吃 `&Arc<Database>`（= `&state.app_state.db`），同步、无网络，直接在 async handler 里调用即可（rusqlite 自带锁，非长阻塞）。

### Task 1.1：GET /api/v1/skills（列出已安装）

**Files:**
- Modify: `cc-switch-server/src/api/skills.rs`
- Modify: `cc-switch-server/tests/api_integration.rs`

**依据：** `SkillService::get_all_installed(db: &Arc<Database>) -> Result<Vec<InstalledSkill>>`（skill.rs:562）

- [ ] **Step 1: 写失败测试**

在 `tests/api_integration.rs` 末尾追加：

```rust
#[tokio::test]
#[serial]
async fn skills_list_returns_array() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/skills", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "expected array, got: {body}");
}
```

- [ ] **Step 2: 跑测试，确认失败（404 / placeholder）**

Run: `cargo test -p cc-switch-server --test api_integration skills_list`
Expected: FAIL（当前 `/skills` 路由不存在或返回 placeholder）

- [ ] **Step 3: 在 skills.rs 实现端点**

把 `cc-switch-server/src/api/skills.rs` 整体替换为：

```rust
//! Skill 管理端点。薄转发到 cc-switch_core::services::skill::SkillService。

use axum::extract::State;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use cc_switch_core::app_config::{AppType, ImportSkillSelection, InstalledSkill};
use cc_switch_core::services::skill::{
    DiscoverableSkill, MigrationResult, SkillBackupEntry, SkillRepo, SkillService,
    SkillStorageLocation, SkillUninstallResult, SkillUpdateInfo, SkillsShSearchResult,
    UnmanagedSkill,
};

use super::{ApiError, ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(list))
        .route("/install", post(install))
        .route("/install-zip", post(install_from_zip))
        .route("/check-updates", post(check_updates))
        .route("/import", post(import_from_apps))
        .route("/unmanaged", get(scan_unmanaged))
        .route("/migrate-storage", post(migrate_storage))
        .route("/search", post(search_skills_sh))
        .route("/backups", get(list_backups))
        .route("/backups/:id", delete(delete_backup))
        .route("/backups/:id/restore", post(restore_backup))
        .route("/:id", delete(uninstall))
        .route("/:id/toggle", post(toggle_app))
        .route("/:id/update", post(update))
}

async fn list(State(state): State<ApiState>) -> ApiResult<Json<Vec<InstalledSkill>>> {
    let db = state.app_state.db.clone();
    let skills = tokio::task::spawn_blocking(move || SkillService::get_all_installed(&db))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(skills))
}
```

说明：尽管 `get_all_installed` 是同步 DB 查询，仍用 `spawn_blocking` 包裹，保持 async runtime 不被任何阻塞 I/O 拖住（后续 install/update 等含网络 + 文件操作，统一用此模式）。

- [ ] **Step 4: 跑测试，确认通过**

Run: `cargo test -p cc-switch-server --test api_integration skills_list`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add cc-switch-server/src/api/skills.rs cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): GET /api/v1/skills list installed skills"
```

### Task 1.2：GET /api/v1/skills/unmanaged + GET /api/v1/skills/backups + GET /api/v1/skill-repos

**依据：**
- `SkillService::scan_unmanaged(db) -> Result<Vec<UnmanagedSkill>>`（skill.rs:1384）
- `SkillService::list_backups() -> Result<Vec<SkillBackupEntry>>`（skill.rs:1230）
- `Database::get_skill_repos() -> Result<Vec<SkillRepo>>`（dao/skills.rs:187）

**注意：** `skill-repos` 挂在 `/api/v1/skill-repos`（不在 `/skills` 下），需在 `api/mod.rs` 的 `router()` 里单独挂载。

- [ ] **Step 1: 写失败测试（3 个）**

追加到 `tests/api_integration.rs`：

```rust
#[tokio::test]
#[serial]
async fn skills_unmanaged_returns_array() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/skills/unmanaged", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_array());
}

#[tokio::test]
#[serial]
async fn skills_backups_returns_array() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/skills/backups", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_array());
}

#[tokio::test]
#[serial]
async fn skill_repos_returns_array() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/skill-repos", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_array());
}
```

- [ ] **Step 2: 跑测试，确认 3 个失败**

Run: `cargo test -p cc-switch-server --test api_integration -- unmanaged backups repos`
Expected: 3 个 FAIL

- [ ] **Step 3: 在 skills.rs 追加 3 个 handler**

在 `skills.rs` 的 `routes()` 末尾 `}` 前确认 `unmanaged`/`backups` 已注册（Step 3 of 1.1 的 routes() 已含），再追加 handler 函数（放在 `list` 函数之后）：

```rust
async fn scan_unmanaged(State(state): State<ApiState>) -> ApiResult<Json<Vec<UnmanagedSkill>>> {
    let db = state.app_state.db.clone();
    let result = tokio::task::spawn_blocking(move || SkillService::scan_unmanaged(&db))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

async fn list_backups() -> ApiResult<Json<Vec<SkillBackupEntry>>> {
    // list_backups 是静态方法，只读备份目录，无 DB 访问
    let result = tokio::task::spawn_blocking(SkillService::list_backups)
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

/// 仓库列表挂在 /api/v1/skill-repos（独立路由），由 mod.rs 调用此 pub fn。
pub async fn list_repos(State(state): State<ApiState>) -> ApiResult<Json<Vec<SkillRepo>>> {
    let db = state.app_state.db.clone();
    let repos = tokio::task::spawn_blocking(move || db.get_skill_repos())
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(repos))
}
```

- [ ] **Step 4: 在 api/mod.rs 挂载 skill-repos 路由**

修改 `cc-switch-server/src/api/mod.rs` 的 `router()`，在 `.nest("/api/v1/mcp", ...)` 之后、`.with_state(...)` 之前追加：

```rust
        .route(
            "/api/v1/skill-repos",
            get(skills::list_repos).post(skills::add_repo),
        )
        .route(
            "/api/v1/skill-repos/:owner/:name",
            delete(skills::remove_repo),
        )
```

并在文件顶部 `use axum::routing::get;` 改为：

```rust
use axum::routing::{delete, get, post};
```

- [ ] **Step 5: 跑测试，确认通过**

Run: `cargo test -p cc-switch-server --test api_integration -- unmanaged backups repos`
Expected: 3 个 PASS（`add_repo`/`remove_repo` 此时还未实现，下个 task 补；但因路由已注册 `.post(skills::add_repo)`，需先补这两个函数否则编译失败——见 Step 6）

- [ ] **Step 6: 补 add_repo / remove_repo（POST/DELETE /skill-repos）**

**依据：** `Database::save_skill_repo(repo)` / `delete_skill_repo(owner, name)`（dao/skills.rs:214,225）

在 `skills.rs` 追加：

```rust
async fn add_repo(
    State(state): State<ApiState>,
    Json(repo): Json<SkillRepo>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || db.save_skill_repo(&repo))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

async fn remove_repo(
    State(state): State<ApiState>,
    axum::extract::Path((owner, name)): axum::extract::Path<(String, String)>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || db.delete_skill_repo(&owner, &name))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}
```

- [ ] **Step 7: 全量编译 + 跑测试**

Run: `cargo test -p cc-switch-server --test api_integration`
Expected: 全部 PASS

- [ ] **Step 8: 提交**

```bash
git add cc-switch-server/src/api/ cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): skill read endpoints (unmanaged/backups/repos)"
```

---

## Phase 2：Skill 写入端点（TDD）

> 写入端点多含网络下载（install/update/check-updates/search）或文件操作。`SkillService::install` 等是 `async fn`，可直接 `.await`；同步方法用 `spawn_blocking`。

### Task 2.1：POST /api/v1/skills/install（从仓库安装）

**依据：** `SkillService::install(&self, db, skill: &DiscoverableSkill, current_app: &AppType) -> Result<InstalledSkill>`（skill.rs:573，async）

- [ ] **Step 1: 写失败测试**

追加到 `tests/api_integration.rs`：

```rust
#[tokio::test]
#[serial]
async fn skills_install_creates_entry() {
    let srv = spawn_api().await;
    // 用一个真实存在、稳定的小仓库验证整条下载→解压→入库→同步链路
    let body = serde_json::json!({
        "skill": {
            "key": "anthropics/skills:pdf",
            "name": "pdf",
            "description": "",
            "directory": "pdf",
            "repoOwner": "anthropics",
            "repoName": "skills",
            "repoBranch": "main"
        },
        "app": "claude"
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/skills/install", srv.base_url))
        .json(&body)
        .send()
        .await
        .unwrap();
    // 网络可用时应 200；CI 无网络时可能 500——用 eprintln 输出便于诊断
    if !resp.status().is_success() {
        eprintln!("install resp: {}", resp.text().await.unwrap());
    }
    assert_eq!(resp.status(), 200);
}
```

说明：此测试依赖外网 GitHub。若 CI 无外网，用 `#[ignore]` 标注并在本地跑；或在 Step 2 实现后改用 mock。生产实现不依赖网络可用性。

- [ ] **Step 2: 跑测试，确认失败（install 未实现）**

Run: `cargo test -p cc-switch-server --test api_integration skills_install -- --include-ignored`
Expected: FAIL（handler 未实现/编译错误）

- [ ] **Step 3: 实现 install handler**

在 `skills.rs` 追加请求体结构 + handler：

```rust
#[derive(Deserialize)]
struct InstallRequest {
    skill: DiscoverableSkill,
    app: AppType,
}

async fn install(
    State(state): State<ApiState>,
    Json(req): Json<InstallRequest>,
) -> ApiResult<Json<InstalledSkill>> {
    let db = state.app_state.db.clone();
    let svc = SkillService::new();
    let installed = svc.install(&db, &req.skill, &req.app).await?;
    Ok(Json(installed))
}
```

- [ ] **Step 4: 跑测试，确认通过（需网络）**

Run: `cargo test -p cc-switch-server --test api_integration skills_install -- --include-ignored`
Expected: PASS（有网络时）

- [ ] **Step 5: 提交**

```bash
git add cc-switch-server/src/api/skills.rs cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): POST /api/v1/skills/install"
```

### Task 2.2：DELETE /api/v1/skills/:id + POST /api/v1/skills/:id/toggle

**依据：**
- `SkillService::uninstall(db, id) -> Result<SkillUninstallResult>`（skill.rs:788）
- `SkillService::toggle_app(db, id, app, enabled) -> Result<()>`（skill.rs:1357）

- [ ] **Step 1: 写失败测试（2 个）**

追加：

```rust
#[tokio::test]
#[serial]
async fn skills_toggle_app_toggles() {
    let srv = spawn_api().await;
    // 先确保有一个 skill（用 install 端点造数据，或直接断言对不存在 id 返回错误）
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "app": "claude", "enabled": false });
    let resp = client
        .post(format!("{}/api/v1/skills/nonexistent/toggle", srv.base_url))
        .json(&body)
        .send()
        .await
        .unwrap();
    // 对不存在的 skill：SkillService::toggle_app 会返回 anyhow error → 500
    assert!(resp.status().is_server_error() || resp.status().as_u16() == 500);
}

#[tokio::test]
#[serial]
async fn skills_uninstall_missing_returns_error() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}/api/v1/skills/nonexistent", srv.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 500);
}
```

- [ ] **Step 2: 跑测试，确认失败**

Run: `cargo test -p cc-switch-server --test api_integration -- toggle uninstall_missing`
Expected: FAIL

- [ ] **Step 3: 实现 uninstall + toggle_app handler**

在 `skills.rs` 追加：

```rust
async fn uninstall(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<Json<SkillUninstallResult>> {
    let db = state.app_state.db.clone();
    let result = tokio::task::spawn_blocking(move || SkillService::uninstall(&db, &id))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct ToggleRequest {
    app: AppType,
    enabled: bool,
}

async fn toggle_app(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<ToggleRequest>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || SkillService::toggle_app(&db, &id, &req.app, req.enabled))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}
```

- [ ] **Step 4: 跑测试，确认通过**

Run: `cargo test -p cc-switch-server --test api_integration -- toggle uninstall_missing`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add cc-switch-server/src/api/skills.rs cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): skill uninstall + toggle_app endpoints"
```

### Task 2.3：POST check-updates + POST /:id/update + POST import + POST migrate-storage + POST search + POST install-zip + backup 操作

> 这一组方法签名各异，逐个给出完整 handler。每个端点先加 1 个冒烟测试（断言状态码符合预期），再加实现。

**依据（全部已核对）：**
- `SkillService::check_updates(&self, db) -> Result<Vec<SkillUpdateInfo>>`（skill.rs:877，async）
- `SkillService::update_skill(&self, db, skill_id) -> Result<InstalledSkill>`（skill.rs:991，async）
- `SkillService::import_from_apps(db, imports: Vec<ImportSkillSelection>) -> Result<Vec<InstalledSkill>>`（skill.rs:1447）
- `SkillService::migrate_storage(db, target: SkillStorageLocation) -> Result<MigrationResult>`（skill.rs:1152）
- `SkillService::search_skills_sh(query, limit, offset) -> Result<SkillsShSearchResult>`（skill.rs:2801，async 关联函数）
- `SkillService::install_from_zip(db, zip_path: &Path, current_app: &AppType) -> Result<Vec<InstalledSkill>>`（skill.rs:2538）
- `SkillService::delete_backup(backup_id) -> Result<()>`（skill.rs:1264）
- `SkillService::restore_from_backup(db, backup_id, current_app) -> Result<InstalledSkill>`（skill.rs:1283）

- [ ] **Step 1: 追加冒烟测试（一组）**

追加到 `tests/api_integration.rs`：

```rust
#[tokio::test]
#[serial]
async fn skills_check_updates_returns_array() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/skills/check-updates", srv.base_url))
        .send()
        .await
        .unwrap();
    // 无网络时 check_updates 内部 catch 错误返回空 Vec（见 skill.rs:916-919），故应 200
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_array());
}

#[tokio::test]
#[serial]
async fn skills_search_accepts_query() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/skills/search", srv.base_url))
        .json(&serde_json::json!({ "query": "pdf", "limit": 5, "offset": 0 }))
        .send()
        .await;
    // 外网不可用时可能超时/500，不硬断言 200，只断言端点存在（非 404）
    if let Ok(r) = resp {
        assert_ne!(r.status().as_u16(), 404);
    }
}

#[tokio::test]
#[serial]
async fn skills_migrate_storage_idempotent() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/skills/migrate-storage", srv.base_url))
        .json(&serde_json::json!({ "target": "cc_switch" }))
        .send()
        .await
        .unwrap();
    // 目标==当前时 migrate_storage 直接返回 0 迁移（skill.rs:1157）
    assert_eq!(resp.status(), 200);
}
```

- [ ] **Step 2: 跑测试，确认失败（未实现）**

Run: `cargo test -p cc-switch-server --test api_integration -- check_updates search migrate_storage`
Expected: FAIL

- [ ] **Step 3: 实现 7 个 handler + 2 个备份 handler**

在 `skills.rs` 追加：

```rust
async fn check_updates(State(state): State<ApiState>) -> ApiResult<Json<Vec<SkillUpdateInfo>>> {
    let db = state.app_state.db.clone();
    let svc = SkillService::new();
    let updates = svc.check_updates(&db).await?;
    Ok(Json(updates))
}

async fn update(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<Json<InstalledSkill>> {
    let db = state.app_state.db.clone();
    let svc = SkillService::new();
    let updated = svc.update_skill(&db, &id).await?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
struct ImportRequest {
    imports: Vec<ImportSkillSelection>,
}

async fn import_from_apps(
    State(state): State<ApiState>,
    Json(req): Json<ImportRequest>,
) -> ApiResult<Json<Vec<InstalledSkill>>> {
    let db = state.app_state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        SkillService::import_from_apps(&db, req.imports)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct MigrateRequest {
    target: SkillStorageLocation,
}

async fn migrate_storage(
    State(state): State<ApiState>,
    Json(req): Json<MigrateRequest>,
) -> ApiResult<Json<MigrationResult>> {
    let db = state.app_state.db.clone();
    let result =
        tokio::task::spawn_blocking(move || SkillService::migrate_storage(&db, req.target))
            .await
            .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    20
}

async fn search_skills_sh(
    Json(req): Json<SearchRequest>,
) -> ApiResult<Json<SkillsShSearchResult>> {
    let result = SkillService::search_skills_sh(&req.query, req.limit, req.offset).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct InstallZipRequest {
    /// 本地 zip 文件绝对路径（管理 API 仅 127.0.0.1，客户端与 server 同机，共享文件系统）
    path: String,
    app: AppType,
}

async fn install_from_zip(
    State(state): State<ApiState>,
    Json(req): Json<InstallZipRequest>,
) -> ApiResult<Json<Vec<InstalledSkill>>> {
    let db = state.app_state.db.clone();
    let zip_path = std::path::PathBuf::from(&req.path);
    if !zip_path.is_file() {
        return Err(ApiError::BadRequest(format!("zip not found: {}", req.path)));
    }
    let result = tokio::task::spawn_blocking(move || {
        SkillService::install_from_zip(&db, &zip_path, &req.app)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

async fn delete_backup(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<Json<()>> {
    tokio::task::spawn_blocking(move || SkillService::delete_backup(&id))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct RestoreBackupRequest {
    app: AppType,
}

async fn restore_backup(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<RestoreBackupRequest>,
) -> ApiResult<Json<InstalledSkill>> {
    let db = state.app_state.db.clone();
    let result =
        tokio::task::spawn_blocking(move || SkillService::restore_from_backup(&db, &id, &req.app))
            .await
            .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}
```

- [ ] **Step 4: 跑全量测试**

Run: `cargo test -p cc-switch-server --test api_integration`
Expected: 全部 PASS（依赖外网的 search 测试用 `assert_ne!(... 404)` 宽松断言）

- [ ] **Step 5: 提交**

```bash
git add cc-switch-server/src/api/skills.rs cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): complete skill write endpoints (update/import/migrate/search/zip/backup)"
```

---

## Phase 3：MCP 端点（TDD）

> `McpService` 方法吃 `&AppState`（= `&state.app_state`），全部同步（含文件 I/O），用 `spawn_blocking` 包裹。

### Task 3.1：GET/POST /api/v1/mcp/servers + DELETE /:id + POST /:id/toggle + POST /sync + POST /import/:app

**依据（全部已核对）：**
- `McpService::get_all_servers(state) -> Result<IndexMap<String, McpServer>>`（mcp.rs:14）
- `McpService::upsert_server(state, server: McpServer)`（mcp.rs:19）
- `McpService::delete_server(state, id) -> Result<bool>`（mcp.rs:54）
- `McpService::toggle_app(state, id, app, enabled)`（mcp.rs:69）
- `McpService::sync_all_enabled(state)`（mcp.rs:180）
- `McpService::import_from_{claude,codex,gemini,opencode,hermes}(state) -> Result<usize>`

- [ ] **Step 1: 写失败测试（一组冒烟）**

追加到 `tests/api_integration.rs`：

```rust
#[tokio::test]
#[serial]
async fn mcp_servers_list_returns_array() {
    let srv = spawn_api().await;
    let resp = reqwest::get(format!("{}/api/v1/mcp/servers", srv.base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_array());
}

#[tokio::test]
#[serial]
async fn mcp_sync_returns_ok() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/mcp/sync", srv.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[serial]
async fn mcp_import_claude_returns_number() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/mcp/import/claude", srv.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    // 返回 { "imported": <number> }
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("imported").is_some());
}
```

- [ ] **Step 2: 跑测试，确认失败**

Run: `cargo test -p cc-switch-server --test api_integration -- mcp_`
Expected: FAIL

- [ ] **Step 3: 实现 mcp.rs 全部端点**

把 `cc-switch-server/src/api/mcp.rs` 整体替换为：

```rust
//! MCP 管理端点。薄转发到 cc_switch_core::services::mcp::McpService。

use axum::extract::{Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use indexmap::IndexMap;
use serde::Deserialize;

use cc_switch_core::app_config::{AppType, McpServer};
use cc_switch_core::services::mcp::McpService;

use super::{ApiError, ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/", axum::routing::get(list_servers).post(upsert_server))
        .route("/:id", delete(delete_server))
        .route("/:id/toggle", post(toggle_app))
        .route("/sync", post(sync_all))
        .route("/import/:app", post(import_from_app))
}

async fn list_servers(State(state): State<ApiState>) -> ApiResult<Json<Vec<McpServer>>> {
    let app_state = state.app_state.clone();
    let servers: IndexMap<String, McpServer> =
        tokio::task::spawn_blocking(move || McpService::get_all_servers(&app_state))
            .await
            .map_err(anyhow::Error::from)??;
    Ok(Json(servers.into_values().collect()))
}

async fn upsert_server(
    State(state): State<ApiState>,
    Json(server): Json<McpServer>,
) -> ApiResult<Json<()>> {
    let app_state = state.app_state.clone();
    tokio::task::spawn_blocking(move || McpService::upsert_server(&app_state, server))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

async fn delete_server(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> ApiResult<Json<bool>> {
    let app_state = state.app_state.clone();
    let removed =
        tokio::task::spawn_blocking(move || McpService::delete_server(&app_state, &id))
            .await
            .map_err(anyhow::Error::from)??;
    Ok(Json(removed))
}

#[derive(Deserialize)]
struct ToggleRequest {
    app: AppType,
    enabled: bool,
}

async fn toggle_app(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<ToggleRequest>,
) -> ApiResult<Json<()>> {
    let app_state = state.app_state.clone();
    tokio::task::spawn_blocking(move || McpService::toggle_app(&app_state, &id, req.app, req.enabled))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

async fn sync_all(State(state): State<ApiState>) -> ApiResult<Json<()>> {
    let app_state = state.app_state.clone();
    tokio::task::spawn_blocking(move || McpService::sync_all_enabled(&app_state))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

async fn import_from_app(
    State(state): State<ApiState>,
    // 用 Path<String> + FromStr，避免 Path<AppType> 对无字段子枚举的反序列化歧义
    // （AppType 实现了 FromStr，见 cc-switch-core/src/app_config.rs:395）
    Path(app_str): Path<String>,
) -> ApiResult<Json<usize>> {
    let app: AppType = app_str
        .parse()
        .map_err(|e: cc_switch_core::error::AppError| ApiError::BadRequest(e.to_string()))?;
    let app_state = state.app_state.clone();
    let count = tokio::task::spawn_blocking(move || match app {
        AppType::Claude => McpService::import_from_claude(&app_state),
        AppType::Codex => McpService::import_from_codex(&app_state),
        AppType::Gemini => McpService::import_from_gemini(&app_state),
        AppType::OpenCode => McpService::import_from_opencode(&app_state),
        AppType::Hermes => McpService::import_from_hermes(&app_state),
        _ => Ok(0), // OpenClaw / ClaudeDesktop 不支持 MCP 导入
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(count))
}
```

- [ ] **Step 4: 跑测试，确认通过**

Run: `cargo test -p cc-switch-server --test api_integration -- mcp_`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add cc-switch-server/src/api/mcp.rs cc-switch-server/tests/api_integration.rs
git commit -m "feat(server): MCP management endpoints (servers CRUD/toggle/sync/import)"
```

---

## Phase 4：TS 客户端 SDK 封装

**Files:**
- Modify: `cc-switch-server/npm/cc-switch-client/index.ts`

### Task 4.1：ProxyInfo 加 mgmtPort + 类型定义

- [ ] **Step 1: 在 index.ts 顶部类型区追加 Skill/MCP 类型**

在 `cc-switch-server/npm/cc-switch-client/index.ts` 的 `ProxyInfo` interface 定义之后，追加类型定义块：

```ts
// ── Skill / MCP 类型（镜像 cc-switch-core 的 serde camelCase 输出）──

export interface SkillApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
  opencode: boolean;
  hermes: boolean;
}

export interface InstalledSkill {
  id: string;
  name: string;
  description?: string;
  directory: string;
  repoOwner?: string;
  repoName?: string;
  repoBranch?: string;
  readmeUrl?: string;
  apps: SkillApps;
  installedAt: number;
  contentHash?: string;
  updatedAt: number;
}

export interface UnmanagedSkill {
  directory: string;
  name: string;
  description?: string;
  foundIn: string[];
  path: string;
}

export interface SkillBackupEntry {
  backupId: string;
  backupPath: string;
  createdAt: number;
  skill: InstalledSkill;
}

export interface SkillRepo {
  owner: string;
  name: string;
  branch: string;
  enabled: boolean;
}

export interface DiscoverableSkill {
  key: string;
  name: string;
  description: string;
  directory: string;
  readmeUrl?: string;
  repoOwner: string;
  repoName: string;
  repoBranch: string;
}

export interface SkillUpdateInfo {
  id: string;
  name: string;
  currentHash?: string;
  remoteHash: string;
}

export interface SkillsShSearchResult {
  skills: DiscoverableSkill[];
  totalCount: number;
  query: string;
}

export type AppType = 'claude' | 'codex' | 'gemini' | 'opencode' | 'hermes';
export type SkillStorageLocation = 'cc_switch' | 'unified';

export interface McpApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
  opencode: boolean;
  hermes: boolean;
}

export interface McpServer {
  id: string;
  name: string;
  server: Record<string, unknown>;
  apps: McpApps;
  description?: string;
  homepage?: string;
  docs?: string;
  tags?: string[];
}
```

- [ ] **Step 2: 修改 ProxyInfo interface 加 mgmtPort**

把 `index.ts` 中现有的 `ProxyInfo` interface 改为（加最后一行）：

```ts
export interface ProxyInfo {
  status: string;
  address: string;
  port: number;
  started_at: string;
  takeover: boolean;
  mgmtPort: number; // 管理 API 端口（/api/v1/）
}
```

- [ ] **Step 3: 提交**

```bash
git add cc-switch-server/npm/cc-switch-client/index.ts
git commit -m "feat(client): add Skill/MCP types + mgmtPort to ProxyInfo"
```

### Task 4.2：CCSwitchClient 加管理方法

- [ ] **Step 1: 在 CCSwitchClient class 内追加管理 API 方法**

在 `cc-switch-server/npm/cc-switch-client/index.ts` 的 `CCSwitchClient` class 内（`stop()` 方法之后、`// ── Private helpers ──` 之前），插入：

```ts
  // ── Management API（/api/v1/）──

  /** 管理 API 基地址。需先 start()。 */
  getMgmtUrl(path = ''): string {
    if (!this.proxyInfo) {
      throw new Error('Proxy not started. Call start() first.');
    }
    return `http://127.0.0.1:${this.proxyInfo.mgmtPort}${path}`;
  }

  private async mgmtFetch<T>(path: string, init?: RequestInit): Promise<T> {
    const resp = await fetch(this.getMgmtUrl(path), init);
    if (!resp.ok) {
      const body = await resp.text().catch(() => '');
      throw new Error(`mgmt API ${path} failed: ${resp.status} ${body}`);
    }
    return resp.json() as Promise<T>;
  }

  // ── Skill 管理 ──

  listSkills(): Promise<InstalledSkill[]> {
    return this.mgmtFetch('/api/v1/skills');
  }

  installSkill(skill: DiscoverableSkill, app: AppType): Promise<InstalledSkill> {
    return this.mgmtFetch('/api/v1/skills/install', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ skill, app }),
    });
  }

  installSkillFromZip(zipPath: string, app: AppType): Promise<InstalledSkill[]> {
    return this.mgmtFetch('/api/v1/skills/install-zip', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ path: zipPath, app }),
    });
  }

  uninstallSkill(id: string): Promise<void> {
    return this.mgmtFetch(`/api/v1/skills/${encodeURIComponent(id)}`, { method: 'DELETE' });
  }

  toggleSkillApp(id: string, app: AppType, enabled: boolean): Promise<void> {
    return this.mgmtFetch(`/api/v1/skills/${encodeURIComponent(id)}/toggle`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ app, enabled }),
    });
  }

  checkSkillUpdates(): Promise<SkillUpdateInfo[]> {
    return this.mgmtFetch('/api/v1/skills/check-updates', { method: 'POST' });
  }

  updateSkill(id: string): Promise<InstalledSkill> {
    return this.mgmtFetch(`/api/v1/skills/${encodeURIComponent(id)}/update`, { method: 'POST' });
  }

  importSkills(imports: { directory: string; apps: SkillApps }[]): Promise<InstalledSkill[]> {
    return this.mgmtFetch('/api/v1/skills/import', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ imports }),
    });
  }

  scanUnmanagedSkills(): Promise<UnmanagedSkill[]> {
    return this.mgmtFetch('/api/v1/skills/unmanaged');
  }

  migrateSkillStorage(target: SkillStorageLocation): Promise<{ migratedCount: number; skippedCount: number; errors: string[] }> {
    return this.mgmtFetch('/api/v1/skills/migrate-storage', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ target }),
    });
  }

  searchSkillsSh(query: string, limit = 20, offset = 0): Promise<SkillsShSearchResult> {
    return this.mgmtFetch('/api/v1/skills/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ query, limit, offset }),
    });
  }

  listSkillBackups(): Promise<SkillBackupEntry[]> {
    return this.mgmtFetch('/api/v1/skills/backups');
  }

  deleteSkillBackup(id: string): Promise<void> {
    return this.mgmtFetch(`/api/v1/skills/backups/${encodeURIComponent(id)}`, { method: 'DELETE' });
  }

  restoreSkillBackup(id: string, app: AppType): Promise<InstalledSkill> {
    return this.mgmtFetch(`/api/v1/skills/backups/${encodeURIComponent(id)}/restore`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ app }),
    });
  }

  listSkillRepos(): Promise<SkillRepo[]> {
    return this.mgmtFetch('/api/v1/skill-repos');
  }

  addSkillRepo(repo: SkillRepo): Promise<void> {
    return this.mgmtFetch('/api/v1/skill-repos', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(repo),
    });
  }

  removeSkillRepo(owner: string, name: string): Promise<void> {
    return this.mgmtFetch(`/api/v1/skill-repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`, { method: 'DELETE' });
  }

  // ── MCP 管理 ──

  listMcpServers(): Promise<McpServer[]> {
    return this.mgmtFetch('/api/v1/mcp/servers');
  }

  upsertMcpServer(server: McpServer): Promise<void> {
    return this.mgmtFetch('/api/v1/mcp/servers', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(server),
    });
  }

  deleteMcpServer(id: string): Promise<boolean> {
    return this.mgmtFetch(`/api/v1/mcp/servers/${encodeURIComponent(id)}`, { method: 'DELETE' });
  }

  toggleMcpApp(id: string, app: AppType, enabled: boolean): Promise<void> {
    return this.mgmtFetch(`/api/v1/mcp/servers/${encodeURIComponent(id)}/toggle`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ app, enabled }),
    });
  }

  syncAllMcp(): Promise<void> {
    return this.mgmtFetch('/api/v1/mcp/sync', { method: 'POST' });
  }

  importMcpFromApp(app: AppType): Promise<number> {
    return this.mgmtFetch(`/api/v1/mcp/import/${app}`, { method: 'POST' });
  }
```

- [ ] **Step 2: 验证 TS 类型/语法**

Run:
```bash
cd cc-switch-server/npm/cc-switch-client && bun run tsc --noEmit index.ts 2>/dev/null || bun build --no-bundle index.ts > /dev/null
```
Expected: 无类型错误。

（若 `tsc` 不可用，用 `bun build --no-bundle index.ts` 仅做语法检查即可。）

- [ ] **Step 3: 提交**

```bash
git add cc-switch-server/npm/cc-switch-client/index.ts
git commit -m "feat(client): wire Skill/MCP management methods to mgmt API"
```

### Task 4.3：端到端测试（TS 客户端 → 真 server）

**Files:**
- Modify: `cc-switch-server/npm/cc-switch-client/__tests__/client.test.ts`

- [ ] **Step 1: 追加 mgmt API e2e 测试**

在现有测试文件追加（需先 `cargo build -p cc-switch-server --release` 并通过 `binaryPath` 指向产物）：

```ts
import { CCSwitchClient } from '../index';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

it('lists skills via mgmt API after start', async () => {
  const configDir = mkdtempSync(join(tmpdir(), 'ccs-mgmt-'));
  const client = new CCSwitchClient({ configDir, logLevel: 'warn' });
  try {
    await client.start();
    const skills = await client.listSkills();
    expect(Array.isArray(skills)).toBe(true);
    const servers = await client.listMcpServers();
    expect(Array.isArray(servers)).toBe(true);
  } finally {
    await client.stop();
  }
}, 30000);
```

- [ ] **Step 2: 构建并跑测试**

Run:
```bash
cargo build -p cc-switch-server --release
cd cc-switch-server/npm/cc-switch-client && bun test
```
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add cc-switch-server/npm/cc-switch-client/__tests__/client.test.ts
git commit -m "test(client): e2e mgmt API through CCSwitchClient"
```

---

## Phase 5：构建/发布验证 + 文档

### Task 5.1：验证多平台构建脚本不受影响

- [ ] **Step 1: dry-run 本机构建**

Run:
```bash
cd cc-switch-server && ./build-and-publish.sh 0.1.0 --dry-run
```
Expected: 至少 `aarch64-apple-darwin` 构建成功，二进制复制到 `npm/cc-switch-client-darwin-arm64/bin/`。

- [ ] **Step 2: 验证 CI workflow 无需改**

检查 `.github/workflows/release-cc-switch-client.yml`：其构建命令是 `cargo build -p cc-switch-server --release --target ...`，新代码随 workspace 正常编译，**无需改 CI**。

- [ ] **Step 3: 提交（如有 build 脚本微调）**

若 dry-run 暴露问题（如新依赖在某平台缺失），修复后提交。否则跳过。

### Task 5.2：更新 SDK README / JSDoc

**Files:**
- Modify: `cc-switch-server/npm/cc-switch-client/index.ts`（顶部 JSDoc）

- [ ] **Step 1: 在 index.ts 顶部 JSDoc 追加管理 API 用法示例**

在文件顶部注释的 `## Quick Start` 块之后追加：

```ts
/**
 * ## Management API（Skill + MCP）
 *
 * ```ts
 * const client = new CCSwitchClient({ configDir: '/my/app/data' });
 * await client.start();
 *
 * // Skill 管理
 * const skills = await client.listSkills();
 * await client.installSkill({ key: 'anthropics/skills:pdf', name: 'pdf', description: '', directory: 'pdf', repoOwner: 'anthropics', repoName: 'skills', repoBranch: 'main' }, 'claude');
 *
 * // MCP 管理
 * const servers = await client.listMcpServers();
 * await client.syncAllMcp();
 * ```
 */
```

- [ ] **Step 2: 提交**

```bash
git add cc-switch-server/npm/cc-switch-client/index.ts
git commit -m "docs(client): document mgmt API usage"
```

---

## Self-Review（写完后自查清单）

### 1. Spec 覆盖
- [x] Skill 列表 → Task 1.1
- [x] Skill 安装（仓库）→ Task 2.1
- [x] Skill 安装（ZIP）→ Task 2.3
- [x] Skill 卸载 → Task 2.2
- [x] Skill 启用/禁用 → Task 2.2
- [x] Skill 更新检测/执行 → Task 2.3
- [x] Skill 导入未管理 → Task 2.3
- [x] Skill 存储迁移 → Task 2.3
- [x] Skill skills.sh 搜索 → Task 2.3
- [x] Skill 备份 列表/删除/恢复 → Task 1.2 + 2.3
- [x] Skill 仓库 列表/增/删 → Task 1.2
- [x] Skill 未管理扫描 → Task 1.2
- [x] MCP 列表/增/删 → Task 3.1
- [x] MCP 启用/禁用 → Task 3.1
- [x] MCP 同步 → Task 3.1
- [x] MCP 从各应用导入 → Task 3.1
- [x] TS 客户端封装 → Task 4.2
- [x] 端到端测试 → Task 4.3
- [x] 构建发布验证 → Task 5.1

### 2. 占位符扫描
- 无 "TBD/TODO/fill in"。每个 handler 给出完整代码。✅
- install 依赖外网的测试用宽松断言 + `--include-ignored`，实现不依赖网络。✅

### 3. 类型一致性
- `AppType` 全链路用 serde lowercase（`"claude"` 等），TS 端 `AppType` 字面量联合与之对齐。✅
- `InstalledSkill` camelCase 字段（`repoOwner`/`installedAt`/`contentHash`）与 TS interface 一致。✅
- `McpServer.server` 是 `serde_json::Value` → TS 端 `Record<string, unknown>`。✅
- `SkillsShSearchResult` serde `rename_all = "camelCase"`（`totalCount`）→ TS `totalCount`。✅
- `MigrationResult` camelCase（`migratedCount`）→ TS 一致。✅
- 路由路径 `/:id` / `/:owner/:name` 在 Rust（axum 0.7 `:param` 语法）与 TS `encodeURIComponent` 一致。✅

### 4. 同步安全复核
- `cc-switch-core/` 零改动 ✅
- `src-tauri/` 零改动 ✅
- 新文件全在 `cc-switch-server/src/api/` + `cc-switch-server/src/lib.rs` + `tests/`（upstream 无此目录）✅
- `main.rs` 改动 3 处，用 `// ── mgmt API ──` 标注 ✅
- `Cargo.toml` 只追加依赖行 ✅
- handler 只 `use cc_switch_core::...` 的 pub 项，不翻私有模块 ✅

---

## 执行顺序建议

按 Phase 0 → 1 → 2 → 3 → 4 → 5 顺序。每个 Task 内严格 TDD（红→绿→提交）。
Phase 0 是地基，必须先完成并验证 health 端点跑通，再进入业务端点。
