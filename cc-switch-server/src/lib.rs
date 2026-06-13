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
