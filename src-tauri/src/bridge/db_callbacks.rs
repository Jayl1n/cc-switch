//! Database change callback registration.
//!
//! Bridges the core's callback system to the actual auto-sync services
//! that were previously hardcoded in database/mod.rs.

/// Register WebDAV and S3 auto-sync as database change callbacks.
///
/// Call this before `Database::init()` to ensure the SQLite update hook
/// can notify sync services of data changes.
pub fn register_auto_sync_callbacks() {
    cc_switch_core::database::set_db_change_callbacks(vec![
        crate::services::webdav_auto_sync::notify_db_changed,
        crate::services::s3_auto_sync::notify_db_changed,
    ]);
}
