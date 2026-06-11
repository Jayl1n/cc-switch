//! Auth provider trait — abstracts Copilot/Codex OAuth token management.
//!
//! In the desktop app, this is implemented by wrapping
//! `CopilotAuthManager` and `CodexOAuthManager` (which use Tauri state).
//!
//! In embedded mode, use `NoopAuthProvider` which returns `None` for all
//! OAuth-related queries — the proxy will reject requests that require
//! Copilot/Codex OAuth authentication (expected behavior if you don't
//! need those providers).

/// Authentication provider trait for managed OAuth flows.
///
/// The proxy's `RequestForwarder` calls these methods when it encounters
/// a provider whose auth strategy is `GitHubCopilot` or `CodexOAuth`.
///
/// When the implementor returns `None`, the forwarder will fall back to
/// the static API key from the provider config, or return an auth error
/// if no valid credential is available.
pub trait AuthProvider: Send + Sync + 'static {
    /// Obtain a valid Copilot token for the given account.
    ///
    /// `account_id` is `Some(id)` when a specific multi-account binding
    /// is configured, or `None` to use the default account.
    ///
    /// Returns `Ok(token)` on success, `Err(message)` on failure.
    fn get_copilot_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, String>;

    /// Obtain the dynamic API endpoint for Copilot (supports enterprise GHES).
    ///
    /// Returns `None` to use the provider's configured `base_url`.
    fn get_copilot_endpoint(&self, account_id: Option<&str>) -> Option<String>;

    /// Obtain a valid Codex OAuth access token for the given ChatGPT account.
    ///
    /// Returns `Ok(token)` on success, `Err(message)` on failure.
    fn get_codex_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, String>;

    /// Resolve the ChatGPT account ID for the Codex OAuth session headers.
    ///
    /// Returns `None` to skip the `ChatGPT-Account-Id` header.
    fn get_codex_account_id(&self, account_id: Option<&str>) -> Option<String>;

    /// Check if a given Copilot model is an OpenAI vendor model (for header routing).
    fn is_copilot_openai_vendor_model(
        &self,
        account_id: Option<&str>,
        model_id: &str,
    ) -> bool;
}

/// No-op auth provider for embedded / headless mode.
///
/// All methods return errors indicating that managed OAuth is not available.
/// Providers using standard API key auth will continue to work normally.
pub struct NoopAuthProvider;

impl AuthProvider for NoopAuthProvider {
    fn get_copilot_token(
        &self,
        _account_id: Option<&str>,
    ) -> Result<String, String> {
        Err("GitHub Copilot auth not available (embedded mode)".to_string())
    }

    fn get_copilot_endpoint(&self, _account_id: Option<&str>) -> Option<String> {
        None
    }

    fn get_codex_token(
        &self,
        _account_id: Option<&str>,
    ) -> Result<String, String> {
        Err("Codex OAuth not available (embedded mode)".to_string())
    }

    fn get_codex_account_id(&self, _account_id: Option<&str>) -> Option<String> {
        None
    }

    fn is_copilot_openai_vendor_model(
        &self,
        _account_id: Option<&str>,
        _model_id: &str,
    ) -> bool {
        false
    }
}
