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
        let _ = axum::serve(listener, app).await;
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

#[tokio::test]
#[serial]
#[ignore = "requires network: downloads a real github repo"]
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
    let status = resp.status();
    if !status.is_success() {
        eprintln!("install resp: {}", resp.text().await.unwrap());
    }
    assert_eq!(status, 200);
}

#[tokio::test]
#[serial]
async fn skills_toggle_app_missing_returns_error() {
    let srv = spawn_api().await;
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "app": "claude", "enabled": false });
    let resp = client
        .post(format!("{}/api/v1/skills/nonexistent/toggle", srv.base_url))
        .json(&body)
        .send()
        .await
        .unwrap();
    // SkillService::toggle_app 对不存在的 id 返回 anyhow error → 500
    assert_eq!(resp.status(), 500);
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
