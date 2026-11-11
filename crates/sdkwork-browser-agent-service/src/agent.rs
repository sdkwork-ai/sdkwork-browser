use sdkwork_browser_abstraction_service::{BrowserPlugin, BrowserPluginKind};
use sdkwork_browser_mcp_service::McpConnector;
use sdkwork_browser_tab_service::BrowserTab;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::llm_runtime::{LanguageModelPort, LlmConfigError, LlmMode, LlmRuntime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentCapability {
    PageUnderstanding,
    PageAction,
    FormFill,
    AutoLogin,
    AutoTest,
    DataCapture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPageContext {
    pub tab_id: Option<String>,
    pub url: String,
    pub title: String,
    pub html_excerpt: String,
    pub visible_text: String,
    pub meta_description: Option<String>,
    pub word_count_estimate: u32,
    pub engine_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAiActionRequest {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAiActionResult {
    pub code: String,
    pub message: String,
    pub action: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAgentChatRequest {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAgentChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAgentChatResponse {
    pub code: String,
    pub reply: BrowserAgentChatMessage,
    #[serde(default)]
    pub suggested_actions: Vec<String>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroupSuggestion {
    pub tab_id: String,
    pub group_id: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAgentChatStreamResponse {
    pub code: String,
    pub chunks: Vec<String>,
    pub reply: BrowserAgentChatMessage,
    #[serde(default)]
    pub suggested_actions: Vec<String>,
}

pub struct BrowserAgentService {
    runtime_id: &'static str,
    runtime_mode: String,
    llm: LlmRuntime,
}

impl BrowserAgentService {
    /// Startup constructor: validates the LLM transport configuration and
    /// fails before the first business request (MODULE_SPEC.md §4.2).
    pub fn try_new() -> Result<Self, LlmConfigError> {
        Ok(Self::with_llm_runtime(
            Self::runtime_mode_from_env(),
            LlmRuntime::from_env()?,
        ))
    }

    /// In-process constructor with a host-provided `LanguageModelPort`.
    pub fn with_language_model(
        provider: Arc<dyn LanguageModelPort>,
    ) -> Result<Self, LlmConfigError> {
        Ok(Self::with_llm_runtime(
            Self::runtime_mode_from_env(),
            LlmRuntime::from_env()?.with_language_model(provider),
        ))
    }

    /// Lenient constructor for registry/test composition. Startup paths use
    /// [`Self::try_new`]; here a bad LLM opt-in leaves the rules engine active
    /// instead of failing construction.
    pub fn new() -> Self {
        Self::with_llm_runtime(Self::runtime_mode_from_env(), LlmRuntime::lenient_from_env())
    }

    /// Explicit constructor used by hosts and tests.
    pub fn with_llm_runtime(runtime_mode: impl Into<String>, llm: LlmRuntime) -> Self {
        Self {
            runtime_id: "sdkwork-agent-runtime",
            runtime_mode: runtime_mode.into(),
            llm,
        }
    }

    fn runtime_mode_from_env() -> String {
        std::env::var("BROWSER_AGENT_MODE").unwrap_or_else(|_| "rules".into())
    }

    pub fn runtime_id(&self) -> &str {
        self.runtime_id
    }

    pub fn runtime_mode(&self) -> &str {
        &self.runtime_mode
    }

    pub fn llm_runtime(&self) -> &LlmRuntime {
        &self.llm
    }

    pub fn capabilities(&self) -> &'static [AgentCapability] {
        &[
            AgentCapability::PageUnderstanding,
            AgentCapability::PageAction,
        ]
    }

    pub fn build_page_context(
        &self,
        tab: Option<&BrowserTab>,
        html: Option<&str>,
        engine_id: Option<&str>,
    ) -> BrowserPageContext {
        let url = tab.map(|t| t.url.clone()).unwrap_or_default();
        let title = html
            .and_then(title_from_html)
            .or_else(|| tab.map(|t| t.title.clone()))
            .unwrap_or_else(|| page_title_from_url(&url));
        let html_excerpt = html
            .map(excerpt_html)
            .unwrap_or_else(|| format!("No live DOM yet for {url}"));
        let visible_text = html.map(visible_text_from_html).unwrap_or_default();
        let meta_description = html.and_then(meta_description_from_html);
        let word_count_estimate = visible_text
            .split_whitespace()
            .count()
            .max(html_excerpt.split_whitespace().count()) as u32;

        BrowserPageContext {
            tab_id: tab.map(|t| t.id.to_string()),
            url,
            title,
            html_excerpt,
            visible_text,
            meta_description,
            word_count_estimate,
            engine_id: engine_id.map(str::to_owned),
        }
    }

    pub fn execute_action(
        &self,
        request: &BrowserAiActionRequest,
        context: &BrowserPageContext,
        connectors: &[McpConnector],
    ) -> BrowserAiActionResult {
        let normalized = request.action.to_ascii_lowercase().replace('-', "");
        match normalized.as_str() {
            "navigate" | "open" | "goto" => BrowserAiActionResult {
                code: "OK".into(),
                message: format!(
                    "Navigation planned to {}",
                    request
                        .target_url
                        .as_deref()
                        .unwrap_or(context.url.as_str())
                ),
                action: request.action.clone(),
                data: json!({
                    "targetUrl": request.target_url,
                    "engineId": request.engine_id,
                }),
            },
            "summarize" | "summary" => BrowserAiActionResult {
                code: "OK".into(),
                message: self.summarize_context(context),
                action: request.action.clone(),
                data: json!({
                    "title": context.title,
                    "url": context.url,
                    "wordCount": context.word_count_estimate,
                    "metaDescription": context.meta_description,
                }),
            },
            "grouptabs" | "autogroup" => BrowserAiActionResult {
                code: "OK".into(),
                message: "Tab groups will be assigned from URL topics.".into(),
                action: request.action.clone(),
                data: json!({ "strategy": "domain-topic" }),
            },
            "listmcp" | "connectors" => BrowserAiActionResult {
                code: "OK".into(),
                message: format!("{} MCP connectors available.", connectors.len()),
                action: request.action.clone(),
                data: json!({
                    "connectors": connectors.iter().map(|c| json!({
                        "id": c.id,
                        "displayName": c.display_name,
                    })).collect::<Vec<_>>(),
                }),
            },
            _ => BrowserAiActionResult {
                code: "UNSUPPORTED".into(),
                message: format!(
                    "Unknown action '{}'. Try navigate, summarize, groupTabs, or listMcp.",
                    request.action
                ),
                action: request.action.clone(),
                data: json!({}),
            },
        }
    }

    pub async fn chat(
        &self,
        request: &BrowserAgentChatRequest,
        context: &BrowserPageContext,
        connectors: &[McpConnector],
    ) -> BrowserAgentChatResponse {
        let message = request.message.trim();
        if self.should_use_llm() {
            match self.llm.complete(message, &context.title, &context.url).await {
                Ok(completion) => {
                    return BrowserAgentChatResponse {
                        code: "OK".into(),
                        reply: BrowserAgentChatMessage {
                            role: "assistant".into(),
                            content: completion.content,
                            action: None,
                        },
                        suggested_actions: vec![
                            "summarize".into(),
                            "groupTabs".into(),
                            "navigate".into(),
                            "listMcp".into(),
                        ],
                        data: {
                            let mut data = self.runtime_metadata();
                            if let Some(object) = data.as_object_mut() {
                                object.insert("llmModel".into(), json!(completion.model));
                                object.insert("llmEndpoint".into(), json!(completion.endpoint));
                                object.insert(
                                    "llmProvider".into(),
                                    json!(self.llm.provider_label()),
                                );
                            }
                            data
                        },
                    };
                }
                Err(error) => {
                    let mut response = self.chat_rules(message, context, connectors);
                    if let Some(object) = response.data.as_object_mut() {
                        object.insert("llmError".into(), json!(error.to_string()));
                        object.insert("llmFallback".into(), json!(true));
                    }
                    response.reply.content =
                        format!("{} (LLM fallback: rules engine.)", response.reply.content);
                    return response;
                }
            }
        }

        self.chat_rules(message, context, connectors)
    }

    fn should_use_llm(&self) -> bool {
        self.runtime_mode.eq_ignore_ascii_case("llm") && self.llm.is_configured()
    }

    fn chat_rules(
        &self,
        message: &str,
        context: &BrowserPageContext,
        connectors: &[McpConnector],
    ) -> BrowserAgentChatResponse {
        let lower = message.to_ascii_lowercase();

        if lower.contains("summarize") || lower.contains("summary") {
            return BrowserAgentChatResponse {
                code: "OK".into(),
                reply: BrowserAgentChatMessage {
                    role: "assistant".into(),
                    content: self.summarize_context(context),
                    action: Some("summarize".into()),
                },
                suggested_actions: vec!["summarize".into(), "groupTabs".into()],
                data: json!({ "contextUrl": context.url }),
            };
        }

        if lower.contains("group") && lower.contains("tab") {
            return BrowserAgentChatResponse {
                code: "OK".into(),
                reply: BrowserAgentChatMessage {
                    role: "assistant".into(),
                    content:
                        "I can cluster tabs by domain topic (workspace, dev-tools, reference, browsing)."
                            .into(),
                    action: Some("groupTabs".into()),
                },
                suggested_actions: vec!["groupTabs".into()],
                data: json!({}),
            };
        }

        if lower.contains("navigate") || lower.contains("open ") || lower.starts_with("go ") {
            return BrowserAgentChatResponse {
                code: "OK".into(),
                reply: BrowserAgentChatMessage {
                    role: "assistant".into(),
                    content: format!(
                        "Use the address bar or ask me to open a URL. Current page: {}",
                        context.url
                    ),
                    action: Some("navigate".into()),
                },
                suggested_actions: vec!["navigate".into()],
                data: json!({ "currentUrl": context.url }),
            };
        }

        if lower.contains("mcp") || lower.contains("connector") {
            let names: Vec<_> = connectors.iter().map(|c| c.display_name.as_str()).collect();
            return BrowserAgentChatResponse {
                code: "OK".into(),
                reply: BrowserAgentChatMessage {
                    role: "assistant".into(),
                    content: format!("Connected MCP tools: {}.", names.join(", ")),
                    action: Some("listMcp".into()),
                },
                suggested_actions: vec!["listMcp".into()],
                data: json!({ "connectors": names }),
            };
        }

        BrowserAgentChatResponse {
            code: "OK".into(),
            reply: BrowserAgentChatMessage {
                role: "assistant".into(),
                content: format!(
                    "I'm your SDKWork browsing agent on \"{}\". I can summarize pages, navigate, auto-group tabs, and invoke MCP connectors.{}",
                    context.title,
                    self.llm_mode_hint()
                ),
                action: None,
            },
            suggested_actions: vec![
                "summarize".into(),
                "groupTabs".into(),
                "navigate".into(),
                "listMcp".into(),
            ],
            data: {
                let mut data = json!({
                    "capabilities": self.capabilities().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>(),
                });
                if let Some(object) = data.as_object_mut() {
                    object.extend(self.runtime_metadata().as_object().cloned().unwrap_or_default());
                }
                data
            },
        }
    }

    fn runtime_metadata(&self) -> Value {
        let mut metadata = json!({
            "runtimeId": self.runtime_id,
            "runtimeMode": self.runtime_mode,
            "llmEndpointConfigured": self.llm.is_configured(),
        });
        if let (Some(error), Some(object)) = (self.llm.config_error(), metadata.as_object_mut()) {
            object.insert("llmConfigError".into(), json!(error));
        }
        metadata
    }

    pub fn runtime_diagnostics(&self, mcp_connector_count: usize) -> Value {
        crate::kernel_diagnostics::build_runtime_diagnostics(
            self.runtime_id,
            &self.runtime_mode,
            crate::kernel_diagnostics::capability_count(&self.capabilities()),
            mcp_connector_count,
            self.llm.is_configured().then(|| self.llm.provider_label()),
        )
    }

    fn llm_mode_hint(&self) -> &'static str {
        if !self.runtime_mode.eq_ignore_ascii_case("llm") || self.llm.is_configured() {
            return "";
        }
        match self.llm.mode() {
            LlmMode::InProcess => {
                " No in-process LanguageModelPort provider is registered in this process; set SDKWORK_BROWSER_AGENT_LLM_MODE=http with an endpoint for HTTP inference."
            }
            LlmMode::Http => {
                " The HTTP LLM opt-in is incomplete; check SDKWORK_BROWSER_AGENT_LLM_MODE and its endpoint variable."
            }
        }
    }

    pub async fn chat_stream(
        &self,
        request: &BrowserAgentChatRequest,
        context: &BrowserPageContext,
        connectors: &[McpConnector],
    ) -> BrowserAgentChatStreamResponse {
        let response = self.chat(request, context, connectors).await;
        let chunks = response
            .reply
            .content
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        BrowserAgentChatStreamResponse {
            code: response.code,
            chunks,
            reply: response.reply,
            suggested_actions: response.suggested_actions,
        }
    }

    pub fn suggest_tab_groups(tabs: &[BrowserTab]) -> Vec<TabGroupSuggestion> {
        tabs.iter()
            .map(|tab| {
                let (group_id, label) = topic_for_url(&tab.url);
                TabGroupSuggestion {
                    tab_id: tab.id.to_string(),
                    group_id: group_id.into(),
                    label: label.into(),
                }
            })
            .collect()
    }

    fn summarize_context(&self, context: &BrowserPageContext) -> String {
        let host = hostname(&context.url).unwrap_or_else(|| "this site".into());
        let meta = context
            .meta_description
            .as_deref()
            .unwrap_or("No meta description.");
        format!(
            "Page \"{}\" on {host} (~{} words). {meta} Excerpt: {}",
            context.title, context.word_count_estimate, context.html_excerpt
        )
    }
}

impl Default for BrowserAgentService {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserPlugin for BrowserAgentService {
    fn id(&self) -> &str {
        "sdkwork-browser-agent"
    }

    fn kind(&self) -> BrowserPluginKind {
        BrowserPluginKind::Agent
    }
}

fn excerpt_html(html: &str) -> String {
    let collapsed = visible_text_from_html(html);
    if collapsed.len() > 280 {
        format!("{}…", &collapsed[..280])
    } else {
        collapsed
    }
}

fn visible_text_from_html(html: &str) -> String {
    html.replace("<html>", "")
        .replace("</html>", "")
        .replace("<head>", " ")
        .replace("</head>", " ")
        .replace("<body>", " ")
        .replace("</body>", " ")
        .replace("<main>", " ")
        .replace("</main>", " ")
        .replace("<p>", " ")
        .replace("</p>", " ")
        .replace("<title>", " ")
        .replace("</title>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn meta_description_from_html(html: &str) -> Option<String> {
    let marker = "content=\"";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

fn title_from_html(html: &str) -> Option<String> {
    let start = html.find("<title>")? + "<title>".len();
    let end = html[start..].find("</title>")? + start;
    let title = html[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn page_title_from_url(url: &str) -> String {
    hostname(url)
        .map(|host| format!("Page on {host}"))
        .unwrap_or_else(|| "Untitled".into())
}

fn hostname(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    without_scheme.split('/').next().map(str::to_owned)
}

fn topic_for_url(url: &str) -> (&'static str, &'static str) {
    let host = hostname(url).unwrap_or_default().to_ascii_lowercase();
    if host.contains("github")
        || host.contains("gitlab")
        || host.contains("stackoverflow")
        || host.contains("npmjs")
    {
        ("dev-tools", "Development")
    } else if host.contains("docs") || host.contains("wiki") || host.contains("readthedocs") {
        ("reference", "Reference")
    } else if host.contains("localhost") || host.contains("sdkwork") || host.contains("127.0.0.1") {
        ("workspace", "Workspace")
    } else if host.is_empty() {
        ("browsing", "Browsing")
    } else {
        ("browsing", "Browsing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sdkwork_browser_tab_service::BrowserTabService;
    use sdkwork_llm_spi::{LanguageModelCommand, LlmSpiError, LlmSpiResult};
    use std::time::Duration;

    enum StubReply {
        Ok(String),
        Failed { port: String, message: String },
    }

    struct StubLanguageModel {
        reply: StubReply,
    }

    #[async_trait]
    impl LanguageModelPort for StubLanguageModel {
        fn provider_code(&self) -> &str {
            "stub-language-model"
        }

        async fn generate(&self, _command: LanguageModelCommand) -> LlmSpiResult<String> {
            match &self.reply {
                StubReply::Ok(text) => Ok(text.clone()),
                StubReply::Failed { port, message } => Err(LlmSpiError::PortOperationFailed {
                    port: port.clone(),
                    message: message.clone(),
                }),
            }
        }
    }

    fn in_process_agent(reply: StubReply) -> BrowserAgentService {
        let runtime = LlmRuntime::from_config(
            crate::llm_runtime::LlmRuntimeConfig {
                mode: LlmMode::InProcess,
                http_url: None,
                model: "stub-model".into(),
                timeout: Duration::from_secs(5),
            },
            Some(Arc::new(StubLanguageModel { reply })),
        );
        BrowserAgentService::with_llm_runtime("llm", runtime)
    }

    #[test]
    fn summarizes_page_context() {
        let mut tabs = BrowserTabService::new();
        let tab = tabs.open("Docs", "https://docs.sdkwork.local/guide").clone();
        let agent = BrowserAgentService::new();
        let context = agent.build_page_context(
            Some(&tab),
            Some("<html><head><title>Guide</title><meta name=\"description\" content=\"SDKWork browser guide\"></head><body><main>SDKWork browser guide content</main></body></html>"),
            Some("webview"),
        );
        assert_eq!(context.meta_description.as_deref(), Some("SDKWork browser guide"));
        let result = agent.execute_action(
            &BrowserAiActionRequest {
                action: "summarize".into(),
                target_url: None,
                tab_id: None,
                message: None,
                engine_id: None,
            },
            &context,
            &[],
        );
        assert_eq!(result.code, "OK");
        assert!(result.message.contains("Guide"));
    }

    #[tokio::test]
    async fn chat_suggests_grouping() {
        let agent = BrowserAgentService::new();
        let context = agent.build_page_context(None, None, None);
        let response = agent
            .chat(
                &BrowserAgentChatRequest {
                    message: "group my tabs".into(),
                    tab_id: None,
                },
                &context,
                &[],
            )
            .await;
        assert_eq!(response.reply.action.as_deref(), Some("groupTabs"));
    }

    #[tokio::test]
    async fn chat_stream_returns_word_chunks() {
        let agent = BrowserAgentService::new();
        let context = agent.build_page_context(None, None, None);
        let stream = agent
            .chat_stream(
                &BrowserAgentChatRequest {
                    message: "hello agent".into(),
                    tab_id: None,
                },
                &context,
                &[],
            )
            .await;
        assert!(stream.chunks.len() >= 3);
    }

    #[tokio::test]
    async fn in_process_language_model_answers_without_http() {
        let agent = in_process_agent(StubReply::Ok("In-process answer.".into()));
        let context = agent.build_page_context(None, None, None);
        let response = agent
            .chat(
                &BrowserAgentChatRequest {
                    message: "hello".into(),
                    tab_id: None,
                },
                &context,
                &[],
            )
            .await;
        assert_eq!(response.reply.content, "In-process answer.");
        assert_eq!(response.data["llmProvider"], "in_process");
        assert_eq!(response.data["llmModel"], "stub-model");
        assert_eq!(
            response.data["llmEndpoint"],
            "in-process:stub-language-model"
        );
        assert_eq!(response.data["llmEndpointConfigured"], true);
    }

    #[tokio::test]
    async fn in_process_failure_keeps_the_rules_fallback_and_reports_the_error() {
        let agent = in_process_agent(StubReply::Failed {
            port: "LanguageModelPort".into(),
            message: "provider offline".into(),
        });
        let context = agent.build_page_context(None, None, None);
        let response = agent
            .chat(
                &BrowserAgentChatRequest {
                    message: "group my tabs".into(),
                    tab_id: None,
                },
                &context,
                &[],
            )
            .await;
        assert_eq!(response.reply.action.as_deref(), Some("groupTabs"));
        assert_eq!(response.data["llmFallback"], true);
        assert!(response.data["llmError"]
            .as_str()
            .is_some_and(|error| error.starts_with("in-process LLM call failed:")));
    }

    #[test]
    fn assigns_topic_groups_by_domain() {
        let mut tabs = BrowserTabService::new();
        tabs.open("GH", "https://github.com/org/repo");
        tabs.open("Local", "http://localhost:8080");
        let groups = BrowserAgentService::suggest_tab_groups(tabs.list());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].group_id, "dev-tools");
        assert_eq!(groups[1].group_id, "workspace");
    }

    #[test]
    fn try_new_fails_at_startup_when_the_http_opt_in_lacks_an_endpoint() {
        let previous_mode = std::env::var("SDKWORK_BROWSER_AGENT_LLM_MODE").ok();
        let previous_url = std::env::var("BROWSER_AGENT_LLM_URL").ok();
        std::env::set_var("SDKWORK_BROWSER_AGENT_LLM_MODE", "http");
        std::env::remove_var("BROWSER_AGENT_LLM_URL");

        let error = match BrowserAgentService::try_new() {
            Ok(_) => panic!("startup must reject an incomplete opt-in"),
            Err(error) => error,
        };
        assert_eq!(error, LlmConfigError::HttpModeMissingUrl);

        match previous_mode {
            Some(value) => std::env::set_var("SDKWORK_BROWSER_AGENT_LLM_MODE", value),
            None => std::env::remove_var("SDKWORK_BROWSER_AGENT_LLM_MODE"),
        }
        if let Some(value) = previous_url {
            std::env::set_var("BROWSER_AGENT_LLM_URL", value);
        }
    }

    #[tokio::test]
    async fn exposes_runtime_mode_metadata() {        std::env::set_var("BROWSER_AGENT_MODE", "llm");
        let agent = BrowserAgentService::new();
        assert_eq!(agent.runtime_mode(), "llm");
        let context = agent.build_page_context(None, None, None);
        let response = agent
            .chat(
                &BrowserAgentChatRequest {
                    message: "hello".into(),
                    tab_id: None,
                },
                &context,
                &[],
            )
            .await;
        assert_eq!(response.data["runtimeMode"], "llm");
        std::env::remove_var("BROWSER_AGENT_MODE");
    }
}

