//! Skill 管理端点（Phase 1-2 逐步填充）。

use axum::routing::get;
use axum::Router;

use super::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/placeholder", get(|| async { "todo" }))
}
