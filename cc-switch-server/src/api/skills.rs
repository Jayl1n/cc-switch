//! Skill 管理端点。薄转发到 cc_switch_core::services::skill::SkillService。

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use cc_switch_core::app_config::InstalledSkill;
use cc_switch_core::services::skill::SkillService;

use super::{ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new().route("/", get(list))
}

async fn list(State(state): State<ApiState>) -> ApiResult<Json<Vec<InstalledSkill>>> {
    let db = state.app_state.db.clone();
    let skills = tokio::task::spawn_blocking(move || SkillService::get_all_installed(&db))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(skills))
}
