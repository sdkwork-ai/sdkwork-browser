//! LLM runtime for the browser agent.
//!
//! Transport selection is explicit and validated at startup:
//!
//! - default (`SDKWORK_BROWSER_AGENT_LLM_MODE` unset) is **`http`**, using
//!   `BROWSER_AGENT_LLM_URL`. This is a recorded, reviewable exception to
//!   MODULE_SPEC.md §4.2: the completion chain is provided by the gateway's LLM
//!   surface, and `sdkwork-llm` has no in-process provider yet (Phase 2), so
//!   the browser cannot complete in-process today. The exception lives in
//!   `specs/backend-module-call-boundary.allow.json` with its reason.
//! - `SDKWORK_BROWSER_AGENT_LLM_MODE=in_process` is the explicit opt-in for the
//!   in-process port, `sdkwork-llm-spi::LanguageModelPort::generate`. It needs
//!   a host-injected provider ([`LlmRuntime::in_process`]); without one the
//!   call fails with an actionable error instead of silently degrading.
//!
//! Whichever transport is selected is validated at startup when LLM inference
//! is enabled (`BROWSER_AGENT_MODE=llm`), so a missing endpoint fails on boot
//! rather than on the first business request (MODULE_SPEC.md §4.2.3).

use std::sync::Arc;
use std::time::Duration;

use sdkwork_llm_spi::LanguageModelCommand;

pub use sdkwork_llm_spi::LanguageModelPort;

use crate::llm_http_adapter;

/// Default budget for a single completion when the host sets none.
pub const DEFAULT_LLM_TIMEOUT_MS: u64 = 30_000;
const MIN_LLM_TIMEOUT_MS: u64 = 1_000;
const MAX_LLM_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_MODEL: &str = "sdkwork-browser";
const DEFAULT_LLM_MODE: LlmMode = LlmMode::Http;

const LLM_MODE_ENV: &str = "SDKWORK_BROWSER_AGENT_LLM_MODE";
const LLM_MODEL_ENV: &str = "BROWSER_AGENT_LLM_MODEL";
const LLM_TIMEOUT_ENV: &str = "BROWSER_AGENT_LLM_TIMEOUT_MS";

/// Transport selected for the browser agent LLM call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LlmMode {
    /// Call `sdkwork-llm-spi::LanguageModelPort` in this process (explicit
    /// opt-in; requires a registered provider).
    InProcess,
    /// Call the LLM surface over HTTP (default; requires an endpoint).
    Http,
}

impl LlmMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Http => "http",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "in_process" | "in-process" | "inprocess" | "embedded" => Some(Self::InProcess),
            "http" | "remote" => Some(Self::Http),
            _ => None,
        }
    }
}

/// Resolved configuration of the browser agent LLM runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmRuntimeConfig {
    pub mode: LlmMode,
    pub http_url: Option<String>,
    pub model: String,
    pub timeout: Duration,
}

impl LlmRuntimeConfig {
    /// Resolves configuration from the process environment (strict).
    pub fn from_env() -> Result<Self, LlmConfigError> {
        Self::resolve(&|name| std::env::var(name).ok())
    }

    fn resolve<F: Fn(&str) -> Option<String>>(get: &F) -> Result<Self, LlmConfigError> {
        let mode = Self::resolve_mode(get)?;
        let http_url = llm_http_adapter::configured_url(get);

        if mode == LlmMode::Http {
            let url = http_url.clone().ok_or(LlmConfigError::HttpModeMissingUrl)?;
            if !is_http_url(&url) {
                return Err(LlmConfigError::InvalidHttpUrl { url });
            }
        }

        Ok(Self {
            mode,
            http_url,
            model: Self::resolve_model(get),
            timeout: Self::resolve_timeout(get)?,
        })
    }

    /// Non-aborting resolution for composition paths that are not startup
    /// (registry/test wiring). The resolved mode is preserved, so the default
    /// stays `http` even when its endpoint is missing.
    fn lenient_resolve<F: Fn(&str) -> Option<String>>(get: &F) -> Self {
        Self {
            mode: Self::resolve_mode(get).unwrap_or(DEFAULT_LLM_MODE),
            http_url: llm_http_adapter::configured_url(get),
            model: Self::resolve_model(get),
            timeout: Self::resolve_timeout(get)
                .unwrap_or_else(|_| Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS)),
        }
    }

    /// Configuration of the explicit in-process opt-in: no endpoint is needed.
    pub fn in_process_from_env() -> Self {
        Self {
            mode: LlmMode::InProcess,
            http_url: None,
            model: Self::resolve_model(&|name| std::env::var(name).ok()),
            timeout: Self::resolve_timeout(&|name| std::env::var(name).ok())
                .unwrap_or_else(|_| Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS)),
        }
    }

    fn resolve_mode<F: Fn(&str) -> Option<String>>(get: &F) -> Result<LlmMode, LlmConfigError> {
        let raw = get(LLM_MODE_ENV)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_LLM_MODE.as_str().to_string());
        LlmMode::parse(&raw).ok_or(LlmConfigError::UnsupportedMode { mode: raw })
    }

    fn resolve_model<F: Fn(&str) -> Option<String>>(get: &F) -> String {
        get(LLM_MODEL_ENV)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    }

    fn resolve_timeout<F: Fn(&str) -> Option<String>>(
        get: &F,
    ) -> Result<Duration, LlmConfigError> {
        match get(LLM_TIMEOUT_ENV) {
            None => Ok(Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS)),
            Some(raw) => {
                let parsed = raw
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| LlmConfigError::InvalidTimeout { value: raw.clone() })?;
                if !(MIN_LLM_TIMEOUT_MS..=MAX_LLM_TIMEOUT_MS).contains(&parsed) {
                    return Err(LlmConfigError::InvalidTimeout { value: raw });
                }
                Ok(Duration::from_millis(parsed))
            }
        }
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

/// Startup validation failure of the LLM transport configuration.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LlmConfigError {
    #[error("unsupported LLM mode '{mode}': expected 'http' (default) or 'in_process' (explicit opt-in)")]
    UnsupportedMode { mode: String },
    #[error(
        "LLM transport is 'http' and requires an endpoint configured at startup (or SDKWORK_BROWSER_AGENT_LLM_MODE=in_process with a registered in-process provider)"
    )]
    HttpModeMissingUrl,
    #[error("the HTTP LLM endpoint must be an http(s) URL: '{url}'")]
    InvalidHttpUrl { url: String },
    #[error("the LLM timeout must be an integer number of milliseconds in [1000, 120000]: '{value}'")]
    InvalidTimeout { value: String },
}

/// Completed (buffered) LLM answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmCompletion {
    pub content: String,
    pub model: String,
    pub endpoint: String,
}

/// Failure of one LLM completion.
#[derive(Debug, thiserror::Error)]
pub enum LlmRuntimeError {
    #[error(
        "in-process LLM mode is an explicit opt-in but no LanguageModelPort provider is registered: sdkwork-llm has no completion provider yet (Phase 2); unset SDKWORK_BROWSER_AGENT_LLM_MODE and configure the HTTP endpoint, or inject a provider with LlmRuntime::in_process"
    )]
    InProcessProviderUnavailable,
    #[error("in-process LLM call failed: {0}")]
    InProcessCall(#[from] sdkwork_llm_spi::LlmSpiError),
    #[error("in-process LLM call timed out after {0:?}")]
    InProcessTimeout(Duration),
    #[error("LLM request failed: {0}")]
    HttpTransport(String),
    #[error("LLM endpoint returned HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("LLM response was not JSON: {0}")]
    HttpDecode(String),
    #[error("LLM response missing content: {0}")]
    HttpMissingContent(String),
    #[error("LLM request timed out after {0:?}")]
    HttpTimeout(Duration),
}

/// The browser agent's LLM port: one buffered chat completion.
///
/// The call is not an SSE/streamed consumer on either transport — the HTTP path
/// reads a single JSON body, and `sdkwork-llm` has no streaming completion
/// signature — so this port deliberately exposes one completion rather than a
/// token stream.
pub struct LlmRuntime {
    config: LlmRuntimeConfig,
    provider: Option<Arc<dyn LanguageModelPort>>,
    config_error: Option<String>,
}

impl LlmRuntime {
    /// Startup constructor: validates the selected transport. The default
    /// `http` mode fails here when its endpoint is missing.
    pub fn from_env() -> Result<Self, LlmConfigError> {
        Ok(Self {
            config: LlmRuntimeConfig::from_env()?,
            provider: None,
            config_error: None,
        })
    }

    /// Non-startup constructor for registry/test composition: a bad opt-in is
    /// recorded instead of failing construction. Startup paths use
    /// [`Self::from_env`].
    pub fn lenient_from_env() -> Self {
        match LlmRuntimeConfig::from_env() {
            Ok(config) => Self {
                config,
                provider: None,
                config_error: None,
            },
            Err(error) => Self {
                config: LlmRuntimeConfig::lenient_resolve(&|name| std::env::var(name).ok()),
                provider: None,
                config_error: Some(error.to_string()),
            },
        }
    }

    /// Explicitly selects the in-process opt-in with a host-provided provider.
    pub fn in_process(provider: Arc<dyn LanguageModelPort>) -> Self {
        Self {
            config: LlmRuntimeConfig::in_process_from_env(),
            provider: Some(provider),
            config_error: None,
        }
    }

    /// Builds a runtime from an explicit configuration and provider.
    pub fn from_config(
        config: LlmRuntimeConfig,
        provider: Option<Arc<dyn LanguageModelPort>>,
    ) -> Self {
        Self {
            config,
            provider,
            config_error: None,
        }
    }

    /// Attaches the in-process LLM provider (host composition or tests).
    #[must_use]
    pub fn with_language_model(mut self, provider: Arc<dyn LanguageModelPort>) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn mode(&self) -> LlmMode {
        self.config.mode
    }

    /// Label reported as `llmProvider` in chat responses.
    pub fn provider_label(&self) -> &'static str {
        self.config.mode.as_str()
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub fn timeout(&self) -> Duration {
        self.config.timeout
    }

    pub fn config_error(&self) -> Option<&str> {
        self.config_error.as_deref()
    }

    /// True when a completion can actually be attempted.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        match self.config.mode {
            LlmMode::InProcess => self.provider.is_some(),
            LlmMode::Http => self.config.http_url.is_some(),
        }
    }

    /// Runs one buffered completion with a bounded await.
    pub async fn complete(
        &self,
        user_message: &str,
        page_title: &str,
        page_url: &str,
    ) -> Result<LlmCompletion, LlmRuntimeError> {
        match self.config.mode {
            LlmMode::InProcess => self.complete_in_process(user_message, page_title, page_url).await,
            LlmMode::Http => {
                llm_http_adapter::complete(&self.config, user_message, page_title, page_url).await
            }
        }
    }

    async fn complete_in_process(
        &self,
        user_message: &str,
        page_title: &str,
        page_url: &str,
    ) -> Result<LlmCompletion, LlmRuntimeError> {
        let provider = self
            .provider
            .as_ref()
            .ok_or(LlmRuntimeError::InProcessProviderUnavailable)?;
        let provider_code = provider.provider_code().to_string();
        let command = LanguageModelCommand {
            prompt: in_process_prompt(user_message, page_title, page_url),
        };

        match tokio::time::timeout(self.config.timeout, provider.generate(command)).await {
            Ok(Ok(content)) => Ok(LlmCompletion {
                content,
                model: self.config.model.clone(),
                endpoint: format!("in-process:{provider_code}"),
            }),
            Ok(Err(error)) => Err(LlmRuntimeError::InProcessCall(error)),
            Err(_) => Err(LlmRuntimeError::InProcessTimeout(self.config.timeout)),
        }
    }
}

/// `LanguageModelCommand` carries only `prompt`, so the browser merges its
/// system guidance and the user turn into one prompt.
fn in_process_prompt(user_message: &str, page_title: &str, page_url: &str) -> String {
    format!(
        "System: You are SDKWork Browser Agent. Current page title: {page_title}. Current URL: {page_url}. Reply concisely with actionable browsing help.\n\nUser: {user_message}"
    )
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use async_trait::async_trait;
    use sdkwork_llm_spi::LlmSpiResult;

    use super::*;

    struct RecordingLanguageModel {
        seen: std::sync::Mutex<Option<String>>,
    }

    impl RecordingLanguageModel {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                seen: std::sync::Mutex::new(None),
            })
        }

        fn prompt(&self) -> Option<String> {
            self.seen.lock().ok().and_then(|guard| guard.clone())
        }
    }

    #[async_trait]
    impl LanguageModelPort for RecordingLanguageModel {
        fn provider_code(&self) -> &str {
            "recording-language-model"
        }

        async fn generate(&self, command: LanguageModelCommand) -> LlmSpiResult<String> {
            if let Ok(mut guard) = self.seen.lock() {
                *guard = Some(command.prompt.clone());
            }
            Ok("Navigate to the guide from the page navigation.".to_string())
        }
    }

    struct NeverAnswers;

    #[async_trait]
    impl LanguageModelPort for NeverAnswers {
        fn provider_code(&self) -> &str {
            "never-answers"
        }

        async fn generate(&self, _command: LanguageModelCommand) -> LlmSpiResult<String> {
            pending::<LlmSpiResult<String>>().await
        }
    }

    fn config(mode: LlmMode, http_url: Option<&str>, timeout_ms: u64) -> LlmRuntimeConfig {
        LlmRuntimeConfig {
            mode,
            http_url: http_url.map(str::to_owned),
            model: "test-model".to_string(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    fn only_url(url: Option<&str>) -> impl Fn(&str) -> Option<String> + '_ {
        move |name| (name == llm_http_adapter::HTTP_URL_ENV).then(|| url.map(str::to_owned)).flatten()
    }

    #[test]
    fn default_mode_is_http_and_requires_an_endpoint_at_startup() {
        let error = LlmRuntimeConfig::resolve(&|_| None)
            .expect_err("default http without endpoint must fail at startup");
        assert_eq!(error, LlmConfigError::HttpModeMissingUrl);
    }

    #[test]
    fn default_mode_is_http_once_the_endpoint_is_configured() {
        let resolved = LlmRuntimeConfig::resolve(&only_url(Some("https://llm.example.test/v1/chat")))
            .expect("default http with endpoint");
        assert_eq!(resolved.mode, LlmMode::Http);
        assert_eq!(
            resolved.http_url.as_deref(),
            Some("https://llm.example.test/v1/chat")
        );
        assert_eq!(resolved.model, "sdkwork-browser");
        assert_eq!(
            resolved.timeout,
            Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS)
        );
    }

    #[test]
    fn in_process_mode_is_selected_only_explicitly() {
        // Unset mode stays http (never silently in-process).
        let default_error = LlmRuntimeConfig::resolve(&|_| None)
            .expect_err("unset mode must not select in-process");
        assert_eq!(default_error, LlmConfigError::HttpModeMissingUrl);

        let explicit = LlmRuntimeConfig::resolve(&|name| {
            (name == LLM_MODE_ENV).then(|| "in_process".to_string())
        })
        .expect("explicit in-process needs no endpoint");
        assert_eq!(explicit.mode, LlmMode::InProcess);
        assert!(explicit.http_url.is_none());
    }

    #[test]
    fn http_mode_requires_an_endpoint_at_startup() {
        let error = LlmRuntimeConfig::resolve(&|name| {
            (name == LLM_MODE_ENV).then(|| "http".to_string())
        })
        .expect_err("http without endpoint must fail");
        assert_eq!(error, LlmConfigError::HttpModeMissingUrl);
    }

    #[test]
    fn http_mode_rejects_non_http_endpoints() {
        let error = LlmRuntimeConfig::resolve(&|name| match name {
            LLM_MODE_ENV => Some("http".to_string()),
            other => (other == llm_http_adapter::HTTP_URL_ENV)
                .then(|| "llm.example.test".to_string()),
        })
        .expect_err("non-http endpoint must fail");
        assert!(matches!(error, LlmConfigError::InvalidHttpUrl { .. }));
    }

    #[test]
    fn rejects_unknown_transport_mode() {
        let error = LlmRuntimeConfig::resolve(&|name| {
            (name == LLM_MODE_ENV).then(|| "grpc".to_string())
        })
        .expect_err("unknown mode must fail");
        assert!(matches!(error, LlmConfigError::UnsupportedMode { .. }));
    }

    #[test]
    fn lenient_resolution_keeps_the_http_default() {
        let config = LlmRuntimeConfig::lenient_resolve(&|_| None);
        assert_eq!(config.mode, LlmMode::Http);
        assert!(config.http_url.is_none());
    }

    #[test]
    fn in_process_without_a_provider_is_not_configured() {
        let runtime = LlmRuntime::from_config(config(LlmMode::InProcess, None, 1_000), None);
        assert!(!runtime.is_configured());
        assert_eq!(runtime.provider_label(), "in_process");
    }

    #[tokio::test]
    async fn in_process_provider_receives_the_page_aware_prompt() {
        let provider = RecordingLanguageModel::new();
        let runtime = LlmRuntime::from_config(config(LlmMode::InProcess, None, 1_000), None)
            .with_language_model(provider.clone());
        assert!(runtime.is_configured());

        let completion = runtime
            .complete(
                "how do I read the docs?",
                "Guide",
                "https://docs.sdkwork.local/guide",
            )
            .await
            .expect("in-process completion");
        assert!(completion.content.contains("Navigate"));
        assert_eq!(completion.model, "test-model");
        assert_eq!(completion.endpoint, "in-process:recording-language-model");

        let prompt = provider.prompt().expect("prompt recorded");
        assert!(prompt.contains("how do I read the docs?"));
        assert!(prompt.contains("Guide"));
        assert!(prompt.contains("https://docs.sdkwork.local/guide"));
    }

    #[tokio::test]
    async fn in_process_without_a_provider_fails_without_an_http_fallback() {
        let runtime = LlmRuntime::from_config(config(LlmMode::InProcess, None, 1_000), None);
        let error = runtime
            .complete("hello", "Guide", "https://docs.sdkwork.local")
            .await
            .expect_err("no provider");
        assert!(matches!(
            error,
            LlmRuntimeError::InProcessProviderUnavailable
        ));
        // The failure must tell the operator what to do.
        let message = error.to_string();
        assert!(message.contains("explicit opt-in"));
        assert!(message.contains("SDKWORK_BROWSER_AGENT_LLM_MODE"));
    }

    #[tokio::test]
    async fn in_process_completion_is_bounded_by_the_configured_timeout() {
        let runtime = LlmRuntime::from_config(
            config(LlmMode::InProcess, None, 5),
            Some(Arc::new(NeverAnswers)),
        );
        let error = runtime
            .complete("hello", "Guide", "https://docs.sdkwork.local")
            .await
            .expect_err("timeout");
        assert!(matches!(error, LlmRuntimeError::InProcessTimeout(_)));
    }
}
