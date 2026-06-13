//! MCP 管理端点（Phase 3 逐步填充）。

use axum::routing::get;
use axum::Router;

use super::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/placeholder", get(|| async { "todo" }))
}
