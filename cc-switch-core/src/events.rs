//! Core event trait — abstracts Tauri's emit/listen system.
//!
//! In the desktop app, this is implemented by `tauri::AppHandle`.
//! In embedded mode, use `NoopEvents` (all no-ops) or provide your own
//! implementation to receive callbacks.

use serde::Serialize;

/// Sync backend status payload
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub backend: String,
    pub status: String,
    pub message: Option<String>,
    pub timestamp: i64,
}

/// Core event callback trait.
///
/// All methods have default no-op implementations so implementors
/// only need to override the events they care about.
pub trait CoreEvents: Send + Sync + 'static {
    /// Emitted when a provider is auto-switched by the failover system.
    fn emit_provider_switched(
        &self,
        _app: &str,
        _provider_id: &str,
        _provider_name: &str,
    ) {
        // no-op default
    }

    /// Emitted when a new usage log record is written to the database.
    fn emit_usage_log_recorded(&self) {
        // no-op default
    }

    /// Emitted when a sync backend (WebDAV/S3) status changes.
    fn emit_sync_status(&self, _status: &SyncStatus) {
        // no-op default
    }

    /// Emitted when proxy flags (running / takeover state) change.
    fn emit_proxy_flags_changed(&self, _flags: &ProxyFlags) {
        // no-op default
    }
}

/// Proxy running / takeover state flags (mirrors frontend event payload).
#[derive(Debug, Clone, Serialize)]
pub struct ProxyFlags {
    pub running: bool,
    pub claude: bool,
    pub codex: bool,
    pub gemini: bool,
    pub opencode: bool,
    pub openclaw: bool,
}

/// No-op event handler for embedded / headless mode.
///
/// All callbacks are silent — suitable when you don't need real-time
/// event notifications.
pub struct NoopEvents;

impl CoreEvents for NoopEvents {}
