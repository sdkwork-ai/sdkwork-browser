//! HTTP transport for the browser agent LLM call (explicit opt-in only).
//!
//! The browser agent reaches the LLM in-process by default through
//! `sdkwork-llm-spi`'s `LanguageModelPort` contract. This module is the only
//! place that reads `BROWSER_AGENT_LLM_URL`, and it is reachable only when
//! `SDKWORK_BROWSER_AGENT_LLM_MODE=http` was validated at startup by
//! [`crate::llm_runtime::LlmRuntime::from_env`]. MODULE_SPEC.md §4.2 allows this
//! residual read only with a written reason:
//! `specs/backend-module-call-boundary.allow.json`.
//!
//! The HTTP request/response shapes here are the historical wire format of the
//! browser LLM call and are preserved byte-for-byte: an OpenAI-style
//! `{ model, messages: [system, user] }` request and a single buffered JSON
//! body response (no SSE).

use std::time::Duration;

use serde_json::{json, Value};

use crate::llm_runtime::{LlmCompletion, LlmRuntimeConfig, LlmRuntimeError};

/// Endpoint variable of the legacy HTTP transport (opt-in only).
pub(crate) const HTTP_URL_ENV: &str = "BROWSER_AGENT_LLM_URL";

/// Optional bearer credential of the legacy HTTP transport (opt-in only).
pub(crate) const HTTP_API_KEY_ENV: &str = "BROWSER_AGENT_LLM_API_KEY";

/// Reads the configured HTTP endpoint from an environment lookup.
pub(crate) fn configured_url<F: Fn(&str) -> Option<String>>(get: &F) -> Option<String> {
    get(HTTP_URL_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

struct HttpRequest {
    endpoint: String,
    api_key: Option<String>,
    model: String,
    body: Value,
    timeout: Duration,
}

/// Performs one buffered (non-streamed) chat completion over HTTP.
pub(crate) async fn complete(
    config: &LlmRuntimeConfig,
    user_message: &str,
    page_title: &str,
    page_url: &str,
) -> Result<LlmCompletion, LlmRuntimeError> {
    let endpoint = config
        .http_url
        .clone()
        .ok_or_else(|| LlmRuntimeError::HttpTransport("endpoint is not configured".into()))?;

    let body = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt(page_title, page_url),
            },
            { "role": "user", "content": user_message },
        ],
    });

    let request = HttpRequest {
        endpoint,
        api_key: std::env::var(HTTP_API_KEY_ENV).ok(),
        model: config.model.clone(),
        body,
        timeout: config.timeout,
    };

    // `ureq` is a blocking client: run it on the runtime's blocking pool so the
    // async caller never parks a worker thread on network I/O. The outer
    // timeout bounds the await even when the blocking task wedges; the task is
    // detached after a timeout and finishes at the socket timeout.
    let joined = tokio::task::spawn_blocking(move || send(request));
    match tokio::time::timeout(config.timeout, joined).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_error)) => Err(LlmRuntimeError::HttpTransport(format!(
            "request task failed: {join_error}"
        ))),
        Err(_) => Err(LlmRuntimeError::HttpTimeout(config.timeout)),
    }
}

fn send(request: HttpRequest) -> Result<LlmCompletion, LlmRuntimeError> {
    let agent = ureq::AgentBuilder::new().timeout(request.timeout).build();
    let mut writer = agent
        .post(&request.endpoint)
        .set("Content-Type", "application/json");
    if let Some(api_key) = request.api_key.as_deref() {
        writer = writer.set("Authorization", &format!("Bearer {api_key}"));
    }

    // `ureq` reports 4xx/5xx as `Err(Error::Status(..))`; mapping every error to
    // the transport message keeps the historical `llmError` text unchanged.
    let response = writer
        .send_json(request.body)
        .map_err(|error| LlmRuntimeError::HttpTransport(error.to_string()))?;

    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(LlmRuntimeError::HttpStatus {
            status,
            body: response.into_string().unwrap_or_default(),
        });
    }

    let payload: Value = response
        .into_json()
        .map_err(|error| LlmRuntimeError::HttpDecode(error.to_string()))?;

    let content = extract_content(&payload).ok_or_else(|| {
        LlmRuntimeError::HttpMissingContent(payload.to_string())
    })?;

    Ok(LlmCompletion {
        content,
        model: request.model,
        endpoint: request.endpoint,
    })
}

fn system_prompt(page_title: &str, page_url: &str) -> String {
    format!(
        "You are SDKWork Browser Agent. Current page title: {page_title}. Current URL: {page_url}. Reply concisely with actionable browsing help."
    )
}

fn extract_content(payload: &Value) -> Option<String> {
    payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| payload.get("message").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| payload.get("reply").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| payload.get("content").and_then(Value::as_str).map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_style_content() {
        let payload = json!({
            "choices": [{ "message": { "content": "Summarize the page." } }]
        });
        assert_eq!(
            extract_content(&payload).as_deref(),
            Some("Summarize the page.")
        );
    }

    #[test]
    fn extracts_simple_message_field() {
        let payload = json!({ "message": "Navigate to docs." });
        assert_eq!(
            extract_content(&payload).as_deref(),
            Some("Navigate to docs.")
        );
    }

    #[test]
    fn reads_only_an_explicit_http_endpoint() {
        let get = |name: &str| match name {
            HTTP_URL_ENV => Some("  https://llm.example.test/v1/chat  ".to_string()),
            _ => None,
        };
        assert_eq!(
            configured_url(&get).as_deref(),
            Some("https://llm.example.test/v1/chat")
        );

        let blank = |_name: &str| Some("   ".to_string());
        assert_eq!(configured_url(&blank), None);

        let missing = |_name: &str| None;
        assert_eq!(configured_url(&missing), None);
    }

    #[test]
    fn system_prompt_keeps_page_identity() {
        let prompt = system_prompt("Guide", "https://docs.sdkwork.local/guide");
        assert!(prompt.contains("Guide"));
        assert!(prompt.contains("https://docs.sdkwork.local/guide"));
    }
}
