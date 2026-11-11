//! In-process-first LLM runtime for the browser agent.
//!
//! MODULE_SPEC.md §4.2 requires a backend module to reach a sibling SDKWork
//! module in-process. The browser agent therefore calls the LLM module through
//! its own contract crate, `sdkwork-llm-spi`, via
//! [`LanguageModelPort::generate`] on the shared process — no HTTP hop by
//! default.
//!
//! Current state of the sibling contract (recorded, not worked around): the
//! only completion signature `sdkwork-llm` exposes is
//!
//! ```text
//! async fn generate(&self, command: LanguageModelCommand) -> LlmSpiResult<String>;
//! ```
//!
//! and **no crate in `sdkwork-llm` implements `LanguageModelPort`** yet
//! (`sdkwork-llm-plugin-reference-profiles::ReferenceLlmRuntime` implements the
//! eleven memory/retrieval ports but not this one). A host that owns an
//! implementation injects it with [`LlmRuntime::with_language_model`]; without
//! one, the in-process runtime reports [`LlmRuntime::is_configured`] as
//! `false` and the agent keeps its rules engine. The legacy HTTP transport is
//! an explicit opt-in (`SDKWORK_BROWSER_AGENT_LLM_MODE=http`) and is rejected
//! at startup when `BROWSER_AGENT_LLM_URL` is missing.

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

const LLM_MODE_ENV: &str = "SDKWORK_BROWSER_AGENT_LLM_MODE";
const LLM_MODEL_ENV: &str = "BROWSER_AGENT_LLM_MODEL";
const LLM_TIMEOUT_ENV: &str = "BROWSER_AGENT_LLM_TIMEOUT_MS";

/// Transport selected for the browser agent LLM call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LlmMode {
    /// Call `sdkwork-llm-spi::LanguageModelPort` in this process (default).
    InProcess,
    /// Call a sibling/external endpoint over HTTP (explicit opt-in only).
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
    /// Resolves configuration from the process environment.
    pub fn from_env() -> Result<Self, LlmConfigError> {
        Self::resolve(|name| std::env::var(name).ok())
    }

    fn resolve<F: Fn(&str) -> Option<String>>(get: F) -> Result<Self, LlmConfigError> {
        let mode_raw = get(LLM_MODE_ENV)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| LlmMode::InProcess.as_str().to_string());
        let mode = LlmMode::parse(&mode_raw)
            .ok_or_else(|| LlmConfigError::UnsupportedMode { mode: mode_raw })?;

        let http_url = llm_http_adapter::configured_url(&get);
        if mode == LlmMode::Http {
            let url = http_url.clone().ok_or(LlmConfigError::HttpModeMissingUrl)?;
            if !is_http_url(&url) {
                return Err(LlmConfigError::InvalidHttpUrl { url });
            }
        }

        let model = get(LLM_MODEL_ENV)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        let timeout = match get(LLM_TIMEOUT_ENV) {
            None => Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS),
            Some(raw) => {
                let parsed = raw
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| LlmConfigError::InvalidTimeout { value: raw.clone() })?;
                if !(MIN_LLM_TIMEOUT_MS..=MAX_LLM_TIMEOUT_MS).contains(&parsed) {
                    return Err(LlmConfigError::InvalidTimeout { value: raw });
                }
                Duration::from_millis(parsed)
            }
        };

        Ok(Self {
            mode,
            http_url,
            model,
            timeout,
        })
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

/// Startup validation failure of the LLM transport configuration.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LlmConfigError {
    #[error("unsupported LLM mode '{mode}': expected 'in_process' (default) or 'http'")]
    UnsupportedMode { mode: String },
    #[error("LLM mode 'http' requires an endpoint configured at startup")]
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
        "no in-process LLM provider is registered: sdkwork-llm exposes the LanguageModelPort contract but no implementation"
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
/// The call is not an SSE/streamed consumer on either transport — the legacy
/// HTTP path read a single JSON body, and `sdkwork-llm` has no streaming
/// completion signature — so this port deliberately exposes one completion
/// rather than a token stream.
pub struct LlmRuntime {
    config: LlmRuntimeConfig,
    provider: Option<Arc<dyn LanguageModelPort>>,
    config_error: Option<String>,
}

impl LlmRuntime {
    /// Startup constructor: fails when the HTTP opt-in lacks configuration.
    pub fn from_env() -> Result<Self, LlmConfigError> {
        Ok(Self {
            config: LlmRuntimeConfig::from_env()?,
            provider: None,
            config_error: None,
        })
    }

    /// Non-startup constructor for registry/test composition: a bad opt-in
    /// leaves the rules engine active instead of failing construction. Startup
    /// paths use [`Self::from_env`].
    pub fn lenient_from_env() -> Self {
        match LlmRuntimeConfig::from_env() {
            Ok(config) => Self {
                config,
                provider: None,
                config_error: None,
            },
            Err(error) => Self {
                config: LlmRuntimeConfig {
                    mode: LlmMode::InProcess,
                    http_url: None,
                    model: DEFAULT_MODEL.to_string(),
                    timeout: Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS),
                },
                provider: None,
                config_error: Some(error.to_string()),
            },
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

    #[test]
    fn defaults_to_in_process_without_http_configuration() {
        let resolved = LlmRuntimeConfig::resolve(|_| None).expect("default config");
        assert_eq!(resolved.mode, LlmMode::InProcess);
        assert!(resolved.http_url.is_none());
        assert_eq!(resolved.model, "sdkwork-browser");
        assert_eq!(resolved.timeout, Duration::from_millis(DEFAULT_LLM_TIMEOUT_MS));
    }

    #[test]
    fn http_opt_in_requires_an_endpoint_at_startup() {
        let error = LlmRuntimeConfig::resolve(|name| {
            (name == LLM_MODE_ENV).then(|| "http".to_string())
        })
        .expect_err("http without endpoint must fail");
        assert_eq!(error, LlmConfigError::HttpModeMissingUrl);
    }

    #[test]
    fn http_opt_in_rejects_non_http_endpoints() {
        let error = LlmRuntimeConfig::resolve(|name| match name {
            LLM_MODE_ENV => Some("http".to_string()),
            other => (other == llm_http_adapter::HTTP_URL_ENV)
                .then(|| "llm.example.test".to_string()),
        })
        .expect_err("non-http endpoint must fail");
        assert!(matches!(error, LlmConfigError::InvalidHttpUrl { .. }));
    }

    #[test]
    fn http_opt_in_accepts_configured_endpoint() {
        let resolved = LlmRuntimeConfig::resolve(|name| match name {
            LLM_MODE_ENV => Some("HTTP".to_string()),
            other => (other == llm_http_adapter::HTTP_URL_ENV)
                .then(|| "https://llm.example.test/v1/chat".to_string()),
        })
        .expect("valid http config");
        assert_eq!(resolved.mode, LlmMode::Http);
        assert_eq!(
            resolved.http_url.as_deref(),
            Some("https://llm.example.test/v1/chat")
        );
        let runtime = LlmRuntime::from_config(resolved, None);
        assert!(runtime.is_configured());
        assert_eq!(runtime.provider_label(), "http");
    }

    #[test]
    fn rejects_unknown_transport_mode() {
        let error = LlmRuntimeConfig::resolve(|name| {
            (name == LLM_MODE_ENV).then(|| "grpc".to_string())
        })
        .expect_err("unknown mode must fail");
        assert!(matches!(error, LlmConfigError::UnsupportedMode { .. }));
    }

    #[test]
    fn in_process_without_a_provider_is_not_configured() {
        let runtime = LlmRuntime::from_config(config(LlmMode::InProcess, None, 1_000), None);
        assert!(!runtime.is_configured());
    }

    #[tokio::test]
    async fn in_process_provider_receives_the_page_aware_prompt() {
        let provider = RecordingLanguageModel::new();
        let runtime = LlmRuntime::from_config(config(LlmMode::InProcess, None, 1_000), None)
            .with_language_model(provider.clone());
        assert!(runtime.is_configured());

        let completion = runtime
            .complete("how do I read the docs?", "Guide", "https://docs.sdkwork.local/guide")
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
    }

    #[tokio::test]
    async fn in_process_completion_is_bounded_by_the_configured_timeout() {
        let runtime =
            LlmRuntime::from_config(config(LlmMode::InProcess, None, 5), Some(Arc::new(NeverAnswers)));
        let error = runtime
            .complete("hello", "Guide", "https://docs.sdkwork.local")
            .await
            .expect_err("timeout");
        assert!(matches!(error, LlmRuntimeError::InProcessTimeout(_)));
    }
}
