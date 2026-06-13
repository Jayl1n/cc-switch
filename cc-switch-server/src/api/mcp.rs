//! MCP 管理端点。薄转发到 cc_switch_core::services::mcp::McpService。

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use cc_switch_core::app_config::{AppType, McpServer};
use cc_switch_core::services::mcp::McpService;

use super::{ApiError, ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/servers", get(list_servers).post(upsert_server))
        .route("/servers/:id", delete(delete_server))
        .route("/servers/:id/toggle", post(toggle_app))
        .route("/sync", post(sync_all))
        .route("/import/:app", post(import_from_app))
}

async fn list_servers(State(state): State<ApiState>) -> ApiResult<Json<Vec<McpServer>>> {
    let app_state = state.app_state.clone();
    // get_all_servers 返回 IndexMap；用类型推断 + into_values 避免引入 indexmap 依赖
    let servers =
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
struct McpToggleRequest {
    app: AppType,
    enabled: bool,
}

async fn toggle_app(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<McpToggleRequest>,
) -> ApiResult<Json<()>> {
    let app_state = state.app_state.clone();
    tokio::task::spawn_blocking(move || {
        McpService::toggle_app(&app_state, &id, req.app, req.enabled)
    })
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
    // （AppType 实现了 FromStr，Err = AppError，见 cc-switch-core/src/app_config.rs:395）
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
