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
