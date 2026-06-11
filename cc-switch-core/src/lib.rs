//! # CC Switch Core
//!
//! Embeddable core library providing AI coding tool proxy routing,
//! provider management, failover, and configuration management.
//!
//! ## Quick Start
//!
//! ```no_run
//! use cc_switch_core::{Core, ProxyConfig, NoopEvents, NoopAuthProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let core = Core::init(None)?;
//!     let info = core.start_proxy(ProxyConfig::default()).await?;
//!     println!("Proxy running on {}:{}", info.address, info.port);
//!     Ok(())
//! }
//! ```

// ── Public API entry point ──
pub mod core;

// ── Trait abstractions (Phase 1) ──
pub mod events;
pub mod auth_provider;

pub use core::{Core, CoreBuilder, CoreError};
pub use events::{CoreEvents, NoopEvents, ProxyFlags, SyncStatus};
pub use auth_provider::{AuthProvider, NoopAuthProvider};

// ── Core types & config ──
pub mod error;
pub mod config;
pub mod settings;
pub mod provider;
pub mod provider_defaults;
pub mod app_config;
pub mod store;
pub mod init_status;

// ── MCP (top-level re-export for services/mcp.rs) ──
pub mod mcp;

// ── Usage script ──
pub mod usage_script;

// ── App-specific config parsers ──
pub mod claude_desktop_config;
pub mod claude_mcp;
pub mod claude_plugin;
pub mod codex_config;
pub mod gemini_config;
pub mod gemini_mcp;
pub mod openclaw_config;
pub mod opencode_config;
pub mod hermes_config;
pub mod codex_history_migration;

// ── Prompts ──
pub mod prompt;
pub mod prompt_files;

// ── Database ──
pub mod database;

// ── Usage events ──
pub mod usage_events;

// ── Proxy engine ──
pub mod proxy;

// ── Services ──
pub mod services;
