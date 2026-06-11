//! AuthProvider implementation for the Tauri desktop app.
//!
//! Wraps CopilotAuthManager and CodexOAuthManager (stored in Tauri state)
//! behind the AuthProvider trait so the core proxy can use them.

use cc_switch_core::auth_provider::AuthProvider;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::commands::{CodexOAuthState, CopilotAuthState};
use crate::proxy::providers::codex_oauth_auth::CodexOAuthManager;
use crate::proxy::providers::copilot_auth::CopilotAuthManager;

/// AuthProvider backed by Tauri's managed state (Copilot + Codex OAuth managers).
pub struct TauriAuthProvider {
    copilot: Arc<RwLock<CopilotAuthManager>>,
    codex: Arc<RwLock<CodexOAuthManager>>,
}

impl TauriAuthProvider {
    pub fn new(
        copilot: Arc<RwLock<CopilotAuthManager>>,
        codex: Arc<RwLock<CodexOAuthManager>>,
    ) -> Self {
        Self { copilot, codex }
    }

    /// Create from Tauri's managed state.
    pub fn from_app(app: &tauri::AppHandle) -> Self {
        use tauri::Manager;
        let copilot_state = app.state::<CopilotAuthState>();
        let codex_state = app.state::<CodexOAuthState>();
        Self {
            copilot: copilot_state.0.clone(),
            codex: codex_state.0.clone(),
        }
    }

    /// Wrap in Arc<dyn AuthProvider> for injection into the core.
    pub fn into_provider(self) -> Arc<dyn AuthProvider> {
        Arc::new(self)
    }
}

impl AuthProvider for TauriAuthProvider {
    fn get_copilot_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, String> {
        // NOTE: This is a synchronous trait method, but CopilotAuthManager is async.
        // We use tokio's block_in_place to bridge. This is safe because we're always
        // called from within a tokio runtime (proxy handlers are async).
        let copilot = self.copilot.clone();
        let account_id = account_id.map(|s| s.to_string());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let auth = copilot.read().await;
                match account_id {
                    Some(id) => auth.get_valid_token_for_account(&id).await,
                    None => auth.get_valid_token().await,
                }
            })
        })
        .map_err(|e| e.to_string())
    }

    fn get_copilot_endpoint(&self, account_id: Option<&str>) -> Option<String> {
        let copilot = self.copilot.clone();
        let account_id = account_id.map(|s| s.to_string());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let auth = copilot.read().await;
                match account_id {
                    Some(id) => Some(auth.get_api_endpoint(&id).await),
                    None => Some(auth.get_default_api_endpoint().await),
                }
            })
        })
    }

    fn get_codex_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, String> {
        let codex = self.codex.clone();
        let account_id = account_id.map(|s| s.to_string());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let auth = codex.read().await;
                match account_id {
                    Some(id) => auth.get_valid_token_for_account(&id).await,
                    None => auth.get_valid_token().await,
                }
            })
        })
        .map_err(|e| e.to_string())
    }

    fn get_codex_account_id(&self, account_id: Option<&str>) -> Option<String> {
        let codex = self.codex.clone();
        let account_id = account_id.map(|s| s.to_string());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let auth = codex.read().await;
                match account_id {
                    Some(id) => Some(id),
                    None => auth.default_account_id().await,
                }
            })
        })
    }

    fn is_copilot_openai_vendor_model(
        &self,
        account_id: Option<&str>,
        model_id: &str,
    ) -> bool {
        let copilot = self.copilot.clone();
        let account_id = account_id.map(|s| s.to_string());
        let model_id = model_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let auth = copilot.read().await;
                let vendor_result = match account_id.as_deref() {
                    Some(id) => auth.get_model_vendor_for_account(id, &model_id).await,
                    None => auth.get_model_vendor(&model_id).await,
                };
                match vendor_result {
                    Ok(Some(vendor)) => vendor.eq_ignore_ascii_case("openai"),
                    _ => false,
                }
            })
        })
    }
}
