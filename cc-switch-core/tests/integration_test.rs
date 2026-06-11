//! Integration tests for cc-switch-core's public API.
//!
//! Validates that `Core` can be initialized with `NoopEvents` + `NoopAuthProvider`
//! and that the proxy can start/stop without requiring Tauri.

use std::sync::Arc;
use cc_switch_core::{Core, NoopAuthProvider, NoopEvents};

/// Helper: create a Core with an isolated temp directory.
fn make_core() -> Core {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path = tmp.path().to_path_buf();
    std::fs::create_dir_all(&path).expect("mkdir");
    std::mem::forget(tmp);

    Core::builder()
        .config_dir(path)
        .events(Arc::new(NoopEvents))
        .auth_provider(Arc::new(NoopAuthProvider))
        .build()
        .expect("Core::build()")
}

/// Helper: create a Core whose database has proxy port set to 0 (ephemeral).
/// Must be called from within a tokio runtime (use in #[tokio::test] functions).
async fn make_core_ephemeral_port() -> Core {
    let core = make_core();
    let db = core.db();
    let mut config = db.get_proxy_config().await.expect("get_proxy_config");
    config.listen_port = 0;
    db.update_proxy_config(config).await.expect("update_proxy_config");
    core
}

/// Test that Core::builder() builds successfully with default settings.
#[test]
fn test_core_init_default() {
    let core = make_core();
    let _db = core.db();
}

/// Test that Core::init() with a custom config dir works.
#[test]
fn test_core_init_custom_dir() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let path = tmp.path().to_path_buf();
    std::mem::forget(tmp);

    let core = Core::init(Some(path));
    assert!(core.is_ok(), "Core::init() failed: {:?}", core.err());
}

/// Test that the proxy can start and stop on an ephemeral port.
#[tokio::test]
async fn test_proxy_start_stop() {
    let core = make_core_ephemeral_port().await;

    let info = core
        .start_proxy()
        .await
        .expect("start_proxy() failed");
    assert!(!info.address.is_empty(), "proxy address should not be empty");
    assert!(info.port > 0, "proxy port should be > 0, got {}", info.port);

    core.stop_proxy().await.expect("stop_proxy() failed");
}

/// Test that proxy_service() returns a usable reference.
#[test]
fn test_proxy_service_access() {
    let core = make_core();
    let _service = core.proxy_service();
}

/// Test that double-stop returns an error.
#[tokio::test]
async fn test_double_stop_error() {
    let core = make_core_ephemeral_port().await;

    core.start_proxy().await.expect("start_proxy()");
    core.stop_proxy().await.expect("first stop_proxy()");

    let result = core.stop_proxy().await;
    assert!(result.is_err(), "second stop should return error");
}
