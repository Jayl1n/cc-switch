//! CoreEvents implementation for tauri::AppHandle.
//!
//! Forwards core events to the Tauri frontend via `app.emit()`.

use cc_switch_core::events::{CoreEvents, ProxyFlags, SyncStatus};
use serde_json::json;
use std::sync::Arc;
use tauri::Emitter;

/// Wrapper that implements CoreEvents by forwarding to Tauri's event system.
pub struct TauriEvents {
    pub app: tauri::AppHandle,
}

impl CoreEvents for TauriEvents {
    fn emit_provider_switched(
        &self,
        app: &str,
        provider_id: &str,
        provider_name: &str,
    ) {
        let event_data = json!({
            "appType": app,
            "providerId": provider_id,
            "providerName": provider_name,
            "source": "failover"
        });
        if let Err(e) = self.app.emit("provider-switched", event_data) {
            log::error!("[Bridge] emit provider-switched failed: {e}");
        }
    }

    fn emit_usage_log_recorded(&self) {
        if let Err(e) = self.app.emit("usage-log-recorded", ()) {
            log::warn!("[Bridge] emit usage-log-recorded failed: {e}");
        }
    }

    fn emit_sync_status(&self, status: &SyncStatus) {
        let event_name = format!("{}-sync-status-updated", status.backend);
        let payload = json!({
            "source": "auto",
            "status": status.status,
            "error": status.message,
        });
        if let Err(e) = self.app.emit(&event_name, payload) {
            log::debug!("[Bridge] emit {} failed: {e}", event_name);
        }
    }

    fn emit_proxy_flags_changed(&self, _flags: &ProxyFlags) {
        // Desktop app handles this via tray.rs, no-op here
    }
}

/// Create an Arc<dyn CoreEvents> from a Tauri AppHandle.
pub fn make_events(app: tauri::AppHandle) -> Arc<dyn CoreEvents> {
    Arc::new(TauriEvents { app })
}
