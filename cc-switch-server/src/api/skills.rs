//! Skill 管理端点。薄转发到 cc_switch_core::services::skill::SkillService。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use cc_switch_core::app_config::{InstalledSkill, UnmanagedSkill};
use cc_switch_core::services::skill::{SkillBackupEntry, SkillRepo, SkillService};

use super::{ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(list))
        .route("/unmanaged", get(scan_unmanaged))
        .route("/backups", get(list_backups))
}

async fn list(State(state): State<ApiState>) -> ApiResult<Json<Vec<InstalledSkill>>> {
    let db = state.app_state.db.clone();
    let skills = tokio::task::spawn_blocking(move || SkillService::get_all_installed(&db))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(skills))
}

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

pub async fn add_repo(
    State(state): State<ApiState>,
    Json(repo): Json<SkillRepo>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || db.save_skill_repo(&repo))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

pub async fn remove_repo(
    State(state): State<ApiState>,
    Path((owner, name)): Path<(String, String)>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || db.delete_skill_repo(&owner, &name))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}
