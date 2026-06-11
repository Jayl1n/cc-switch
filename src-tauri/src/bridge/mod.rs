//! Bridge module — adapts cc-switch-core traits to Tauri desktop app.
//!
//! Implements:
//! - `CoreEvents for tauri::AppHandle` — emits events to the frontend
//! - `AuthProvider` for Copilot/Codex OAuth — wraps Tauri state managers
//! - DB change callback registration

pub mod events;
pub mod auth;
pub mod db_callbacks;

pub use events::TauriEvents;
pub use auth::TauriAuthProvider;
