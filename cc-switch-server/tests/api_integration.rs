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
