//! # CC Switch Core — Public API
//!
//! The `Core` struct is the main entry point for embedding CC Switch's
//! capabilities into your own project.
//!
//! ## Quick Start
//!
//! ```no_run
//! use cc_switch_core::{Core, NoopEvents, NoopAuthProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let core = Core::builder()
//!         .events(std::sync::Arc::new(NoopEvents))
//!         .auth_provider(std::sync::Arc::new(NoopAuthProvider))
//!         .build()?;
//!
//!     let info = core.start_proxy().await?;
//!     println!("Proxy running on {}:{}", info.address, info.port);
//!
//!     // Your app runs here...
//!     // core.stop_proxy().await?;
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use crate::auth_provider::AuthProvider;
use crate::config::set_app_config_dir_override;
use crate::database::Database;
use crate::events::CoreEvents;
use crate::proxy::types::ProxyServerInfo;
use crate::services::proxy::ProxyService;
use crate::store::AppState;

/// Initialization error type.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Database initialization failed: {0}")]
    Database(String),

    #[error("Proxy error: {0}")]
    Proxy(String),

    #[error("{0}")]
    Other(String),
}

impl From<crate::error::AppError> for CoreError {
    fn from(err: crate::error::AppError) -> Self {
        CoreError::Other(err.to_string())
    }
}

/// The main entry point for cc-switch-core.
///
/// Holds all services and provides methods to start/stop the proxy,
/// manage providers, MCP servers, skills, prompts, etc.
pub struct Core {
    /// Application state (database + proxy service)
    pub state: Arc<AppState>,
    /// Event callback
    events: Arc<dyn CoreEvents>,
    /// Auth provider (Copilot/Codex OAuth)
    auth_provider: Option<Arc<dyn AuthProvider>>,
}

impl Core {
    /// Create a new `Core` with default settings.
    ///
    /// Uses `~/.cc-switch/` as the config directory.
    /// Use `Core::builder()` for custom configuration.
    pub fn init(config_dir: Option<PathBuf>) -> Result<Self, CoreError> {
        let mut builder = Core::builder();
        if let Some(dir) = config_dir {
            builder = builder.config_dir(dir);
        }
        builder.build()
    }

    /// Create a builder for custom configuration.
    pub fn builder() -> CoreBuilder {
        CoreBuilder {
            config_dir: None,
            events: Arc::new(crate::events::NoopEvents),
            auth_provider: None,
        }
    }

    /// Start the local proxy server.
    ///
    /// The proxy handles request routing, provider failover, model mapping,
    /// thinking optimization, and usage statistics for all supported AI tools.
    pub async fn start_proxy(&self) -> Result<ProxyServerInfo, CoreError> {
        self.state
            .proxy_service
            .start()
            .await
            .map_err(CoreError::Proxy)
    }

    /// Start the proxy with Live config takeover.
    ///
    /// This writes proxy addresses into tool config files (Claude, Codex, Gemini),
    /// backing up the originals for later restoration.
    pub async fn start_proxy_with_takeover(&self) -> Result<ProxyServerInfo, CoreError> {
        self.state
            .proxy_service
            .start_with_takeover()
            .await
            .map_err(CoreError::Proxy)
    }

    /// Stop the proxy server.
    pub async fn stop_proxy(&self) -> Result<(), CoreError> {
        self.state
            .proxy_service
            .stop()
            .await
            .map_err(CoreError::Proxy)
    }

    /// Stop the proxy and restore original Live configs.
    pub async fn stop_proxy_with_restore(&self) -> Result<(), CoreError> {
        self.state
            .proxy_service
            .stop_with_restore()
            .await
            .map_err(CoreError::Proxy)
    }

    /// Get a reference to the database for direct queries.
    pub fn db(&self) -> &Arc<Database> {
        &self.state.db
    }

    /// Get a reference to the proxy service.
    pub fn proxy_service(&self) -> &ProxyService {
        &self.state.proxy_service
    }
}

/// Builder for custom `Core` configuration.
pub struct CoreBuilder {
    config_dir: Option<PathBuf>,
    events: Arc<dyn CoreEvents>,
    auth_provider: Option<Arc<dyn AuthProvider>>,
}

impl CoreBuilder {
    /// Set a custom config directory (default: `~/.cc-switch/`).
    pub fn config_dir(mut self, dir: PathBuf) -> Self {
        self.config_dir = Some(dir);
        self
    }

    /// Set a custom config directory, only if `Some`.
    pub fn maybe_config_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.config_dir = dir;
        self
    }

    /// Set the event callback handler.
    ///
    /// Use `NoopEvents` for silent mode, or provide your own implementation
    /// to receive provider-switched, usage-recorded, and sync-status events.
    pub fn events(mut self, events: Arc<dyn CoreEvents>) -> Self {
        self.events = events;
        self
    }

    /// Set the auth provider for Copilot/Codex OAuth.
    ///
    /// Use `NoopAuthProvider` if you don't need managed OAuth flows.
    pub fn auth_provider(mut self, provider: Arc<dyn AuthProvider>) -> Self {
        self.auth_provider = Some(provider);
        self
    }

    /// Build and initialize the `Core`.
    pub fn build(self) -> Result<Core, CoreError> {
        // Set config dir override if provided
        if let Some(dir) = self.config_dir {
            std::fs::create_dir_all(&dir)
                .map_err(|e| CoreError::Other(format!("Failed to create config dir: {e}")))?;
            set_app_config_dir_override(Some(dir));
        }

        // Initialize database
        let db = Database::init().map_err(|e| CoreError::Database(e.to_string()))?;
        let db = Arc::new(db);

        let state = Arc::new(AppState::new(db));

        // Wire events into proxy service
        state.proxy_service.set_events(self.events.clone());

        Ok(Core {
            state,
            events: self.events,
            auth_provider: self.auth_provider,
        })
    }
}
