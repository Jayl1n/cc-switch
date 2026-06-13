//! Skill 管理端点。薄转发到 cc_switch_core::services::skill::SkillService。

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use cc_switch_core::app_config::{AppType, InstalledSkill, UnmanagedSkill};
use cc_switch_core::services::skill::{
    DiscoverableSkill, ImportSkillSelection, MigrationResult, SkillBackupEntry, SkillRepo,
    SkillService, SkillStorageLocation, SkillUninstallResult, SkillUpdateInfo,
    SkillsShSearchResult,
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

async fn uninstall(
    State(state): State<ApiState>,
    Path(id): Path<String>,
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
    Path(id): Path<String>,
    Json(req): Json<ToggleRequest>,
) -> ApiResult<Json<()>> {
    let db = state.app_state.db.clone();
    tokio::task::spawn_blocking(move || SkillService::toggle_app(&db, &id, &req.app, req.enabled))
        .await
        .map_err(anyhow::Error::from)??;
    Ok(Json(()))
}

async fn check_updates(State(state): State<ApiState>) -> ApiResult<Json<Vec<SkillUpdateInfo>>> {
    let db = state.app_state.db.clone();
    let svc = SkillService::new();
    let updates = svc.check_updates(&db).await?;
    Ok(Json(updates))
}

async fn update(
    State(state): State<ApiState>,
    Path(id): Path<String>,
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
    let result = tokio::task::spawn_blocking(move || SkillService::import_from_apps(&db, req.imports))
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

async fn delete_backup(Path(id): Path<String>) -> ApiResult<Json<()>> {
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
    Path(id): Path<String>,
    Json(req): Json<RestoreBackupRequest>,
) -> ApiResult<Json<InstalledSkill>> {
    let db = state.app_state.db.clone();
    let result =
        tokio::task::spawn_blocking(move || SkillService::restore_from_backup(&db, &id, &req.app))
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
