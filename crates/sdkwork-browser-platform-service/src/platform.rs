use sdkwork_browser_agent_service::{
    BrowserAgentChatRequest, BrowserAgentChatResponse, BrowserAgentChatStreamResponse,
    BrowserAgentService, BrowserAiActionRequest, BrowserAiActionResult, BrowserPageContext,
    LlmConfigError, TabGroupSuggestion,
};
use sdkwork_browser_engine_api_service::BrowserEngine;
use sdkwork_browser_engine_api_service::{
    BROWSER_ENGINE_CEF, BROWSER_ENGINE_SERVO, BROWSER_ENGINE_WEBVIEW,
};
use sdkwork_browser_engine_cef_service::{plan_surface, CefSurfaceDescriptor};
use sdkwork_browser_engine_registry_service::{
    bootstrap_default_registry, BrowserEngineRegistry, RegistryError,
};
use sdkwork_browser_mcp_service::{
    BrowserMcpService, McpToolDescriptor, McpToolInvokeRequest, McpToolInvokeResult,
};
use sdkwork_browser_plugin_service::bootstrap_default_plugins;
use sdkwork_browser_tab_service::BrowserTabService;
use sdkwork_utils_rust::{
    offset_limit_page_from_iter, offset_list_page_data, OffsetListPageParams, SdkWorkPageData,
};
use uuid::Uuid;

/// Maximum retained live HTML override bytes to avoid OOM on large pages.
pub const MAX_LIVE_HTML_BYTES: usize = 5 * 1024 * 1024;

use crate::config::BrowserConfig;

pub struct BrowserPlatform {
    config: BrowserConfig,
    registry: BrowserEngineRegistry,
    tabs: BrowserTabService,
    agent: BrowserAgentService,
    mcp: BrowserMcpService,
    active_engine: Option<Box<dyn BrowserEngine>>,
    active_tab_id: Option<Uuid>,
    live_html_override: Option<String>,
}

impl BrowserPlatform {
    /// Builds the platform, validating the agent LLM transport at startup
    /// (MODULE_SPEC.md §4.2: an HTTP opt-in must fail here, never on the first
    /// business request).
    pub fn bootstrap(config: BrowserConfig) -> Result<Self, PlatformError> {
        Ok(Self {
            config,
            registry: bootstrap_default_registry(),
            tabs: BrowserTabService::new(),
            agent: BrowserAgentService::try_new()?,
            mcp: BrowserMcpService::new(),
            active_engine: None,
            active_tab_id: None,
            live_html_override: None,
        })
    }

    pub fn with_default_config() -> Result<Self, PlatformError> {
        Self::bootstrap(BrowserConfig::default())
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    pub fn registry(&self) -> &BrowserEngineRegistry {
        &self.registry
    }

    pub fn tabs(&self) -> &BrowserTabService {
        &self.tabs
    }

    pub fn tabs_mut(&mut self) -> &mut BrowserTabService {
        &mut self.tabs
    }

    pub fn agent(&self) -> &BrowserAgentService {
        &self.agent
    }

    pub fn mcp(&self) -> &BrowserMcpService {
        &self.mcp
    }

    pub fn plugin_registry(&self) -> sdkwork_browser_abstraction_service::BrowserPluginRegistry {
        bootstrap_default_plugins()
    }

    pub async fn start_engine(&mut self) -> Result<(), PlatformError> {
        let engine_id = self
            .config
            .engine_id()
            .ok_or_else(|| PlatformError::InvalidEngine(self.config.engine.clone()))?;
        let mut engine = self.registry.get(engine_id.as_str())?;
        engine.initialize().await.map_err(PlatformError::Engine)?;
        self.active_engine = Some(engine);
        Ok(())
    }

    pub fn active_engine_id(&self) -> Option<&str> {
        self.active_engine.as_ref().map(|e| e.engine_id())
    }

    pub async fn load_url(&mut self, url: &str) -> Result<(), PlatformError> {
        let engine = self
            .active_engine
            .as_mut()
            .ok_or(PlatformError::EngineNotStarted)?;
        engine.load_url(url).await.map_err(PlatformError::Engine)?;

        let title = tab_title_from_url(url);
        if let Some(tab_id) = self.active_tab_id {
            if self.tabs_mut().set_url(tab_id, title.clone(), url) {
                return Ok(());
            }
        }

        let tab = self.tabs_mut().open(title, url);
        self.active_tab_id = Some(tab.id);
        Ok(())
    }

    pub async fn open_url_in_new_tab(&mut self, url: &str) -> Result<(), PlatformError> {
        let engine = self
            .active_engine
            .as_mut()
            .ok_or(PlatformError::EngineNotStarted)?;
        engine.load_url(url).await.map_err(PlatformError::Engine)?;
        let tab = self.tabs_mut().open(tab_title_from_url(url), url);
        self.active_tab_id = Some(tab.id);
        Ok(())
    }

    pub fn set_active_tab(&mut self, tab_id: Uuid) -> bool {
        if self.tabs.list().iter().any(|tab| tab.id == tab_id) {
            self.active_tab_id = Some(tab_id);
            return true;
        }
        false
    }

    pub fn active_tab(&self) -> Option<&sdkwork_browser_tab_service::BrowserTab> {
        if let Some(tab_id) = self.active_tab_id {
            return self.tabs.list().iter().find(|tab| tab.id == tab_id);
        }
        self.tabs.list().last()
    }

    pub fn set_live_html(&mut self, html: Option<String>) {
        self.live_html_override = html.map(|value| truncate_live_html(value));
    }

    pub fn list_mcp_tools(&self) -> Vec<McpToolDescriptor> {
        self.mcp.list_tools()
    }

    pub fn invoke_mcp_tool(&self, request: McpToolInvokeRequest) -> McpToolInvokeResult {
        self.mcp.invoke_tool(&request)
    }

    pub async fn page_context(&self) -> Result<BrowserPageContext, PlatformError> {
        let html_owned = if let Some(html) = &self.live_html_override {
            Some(html.clone())
        } else if let Some(engine) = self.active_engine.as_ref() {
            engine.get_html().await.ok()
        } else {
            None
        };
        Ok(self.agent.build_page_context(
            self.active_tab(),
            html_owned.as_deref(),
            self.active_engine_id(),
        ))
    }

    pub async fn execute_ai_action(
        &mut self,
        request: BrowserAiActionRequest,
    ) -> Result<BrowserAiActionResult, PlatformError> {
        let context = self.page_context().await?;
        let connectors = self.mcp.connectors().to_vec();
        let result = self
            .agent
            .execute_action(&request, &context, &connectors);

        self.apply_ai_side_effects(&request, &result).await?;
        Ok(result)
    }

    pub async fn agent_chat(
        &self,
        request: BrowserAgentChatRequest,
    ) -> Result<BrowserAgentChatResponse, PlatformError> {
        let context = self.page_context().await?;
        Ok(self
            .agent
            .chat(&request, &context, self.mcp.connectors())
            .await)
    }

    pub async fn agent_chat_stream(
        &self,
        request: BrowserAgentChatRequest,
    ) -> Result<BrowserAgentChatStreamResponse, PlatformError> {
        let context = self.page_context().await?;
        Ok(self
            .agent
            .chat_stream(&request, &context, self.mcp.connectors())
            .await)
    }

    pub fn auto_group_tabs(&mut self) -> Vec<TabGroupSuggestion> {
        let suggestions = BrowserAgentService::suggest_tab_groups(self.tabs.list());
        for suggestion in &suggestions {
            if let Ok(tab_id) = Uuid::parse_str(&suggestion.tab_id) {
                self.tabs_mut().set_group(tab_id, &suggestion.group_id);
            }
        }
        suggestions
    }

    async fn apply_ai_side_effects(
        &mut self,
        request: &BrowserAiActionRequest,
        result: &BrowserAiActionResult,
    ) -> Result<(), PlatformError> {
        if result.code != "OK" {
            return Ok(());
        }

        let normalized = request.action.to_ascii_lowercase().replace('-', "");
        match normalized.as_str() {
            "navigate" | "open" | "goto" => {
                if let Some(url) = request.target_url.as_deref() {
                    self.load_url(url).await?;
                }
            }
            "grouptabs" | "autogroup" => {
                self.auto_group_tabs();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn snapshot(&self) -> BrowserPlatformSnapshot {
        BrowserPlatformSnapshot {
            configured_engine: self.config.engine.clone(),
            active_engine_id: self.active_engine_id().map(str::to_owned),
            engine_started: self.active_engine.is_some(),
            active_tab_id: self.active_tab_id.map(|id| id.to_string()),
            agent_runtime_id: self.agent.runtime_id().to_string(),
            mcp_connectors: self
                .mcp
                .connectors()
                .iter()
                .map(|connector| McpConnectorSnapshot {
                    id: connector.id.clone(),
                    display_name: connector.display_name.clone(),
                })
                .collect(),
            tabs: self
                .tabs
                .list()
                .iter()
                .map(BrowserTabSnapshot::from_tab)
                .collect(),
        }
    }

    pub fn list_operator_engines(&self) -> Vec<BrowserEngineDescriptor> {
        self.collect_operator_engines()
    }

    pub fn list_operator_engines_page(
        &self,
        params: OffsetListPageParams,
    ) -> SdkWorkPageData<BrowserEngineDescriptor> {
        paginate_offset(self.collect_operator_engines(), params)
    }

    pub fn list_operator_sessions(&self) -> Vec<BrowserOperatorSession> {
        self.collect_operator_sessions()
    }

    pub fn list_operator_sessions_page(
        &self,
        params: OffsetListPageParams,
    ) -> SdkWorkPageData<BrowserOperatorSession> {
        paginate_offset(self.collect_operator_sessions(), params)
    }

    fn collect_operator_engines(&self) -> Vec<BrowserEngineDescriptor> {
        self.registry
            .list_engine_ids()
            .into_iter()
            .map(|id| {
                let package_size_mb = match id.as_str() {
                    BROWSER_ENGINE_WEBVIEW => 10,
                    BROWSER_ENGINE_SERVO => 50,
                    BROWSER_ENGINE_CEF => 300,
                    _ => 0,
                };
                BrowserEngineDescriptor {
                    id: id.clone(),
                    display_name: id.clone(),
                    package_size_mb,
                    active: self
                        .active_engine_id()
                        .is_some_and(|active| active == id),
                    configured: self.config.engine == id,
                }
            })
            .collect()
    }

    fn collect_operator_sessions(&self) -> Vec<BrowserOperatorSession> {
        let observed_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        vec![BrowserOperatorSession {
            session_id: "local".into(),
            kind: "persistent".into(),
            active_engine_id: self.active_engine_id().map(str::to_owned),
            active_tab_id: self.active_tab_id.map(|id| id.to_string()),
            tab_count: self.tabs.list().len() as u32,
            agent_runtime_id: self.agent.runtime_id().to_string(),
            runtime_mode: self.agent.runtime_mode().to_string(),
            mcp_connector_count: self.mcp.connectors().len() as u32,
            observed_at_unix,
        }]
    }

    pub fn agent_runtime_diagnostics(&self) -> serde_json::Value {
        self.agent
            .runtime_diagnostics(self.mcp.connectors().len())
    }

    pub async fn switch_engine(&mut self, engine: &str) -> Result<(), PlatformError> {
        self.active_engine = None;
        self.config.engine = engine.to_string();
        self.start_engine().await
    }

    pub fn cef_surface_descriptor(
        &self,
        parent_window_label: &str,
        width: u32,
        height: u32,
    ) -> Option<CefSurfaceDescriptor> {
        let engine_id = self.active_engine_id().unwrap_or(self.config.engine.as_str());
        if engine_id != BROWSER_ENGINE_CEF {
            return None;
        }

        let binding = if self.active_engine.is_some() {
            if sdkwork_browser_engine_cef_service::is_native_runtime_initialized() {
                "cef-rs"
            } else {
                "cef-stub"
            }
        } else {
            "cef-stub"
        };

        let native_initialized = sdkwork_browser_engine_cef_service::is_native_runtime_initialized();

        Some(plan_surface(
            binding,
            native_initialized,
            parent_window_label,
            width,
            height,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserEngineDescriptor {
    pub id: String,
    pub display_name: String,
    pub package_size_mb: u32,
    pub active: bool,
    pub configured: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserOperatorSession {
    pub session_id: String,
    pub kind: String,
    pub active_engine_id: Option<String>,
    pub active_tab_id: Option<String>,
    pub tab_count: u32,
    pub agent_runtime_id: String,
    pub runtime_mode: String,
    pub mcp_connector_count: u32,
    pub observed_at_unix: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserPlatformSnapshot {
    pub configured_engine: String,
    pub active_engine_id: Option<String>,
    pub engine_started: bool,
    pub active_tab_id: Option<String>,
    pub agent_runtime_id: String,
    pub mcp_connectors: Vec<McpConnectorSnapshot>,
    pub tabs: Vec<BrowserTabSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpConnectorSnapshot {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserTabSnapshot {
    pub id: String,
    pub title: String,
    pub url: String,
    pub pin_state: String,
    pub group_id: Option<String>,
}

impl BrowserTabSnapshot {
    fn from_tab(tab: &sdkwork_browser_tab_service::BrowserTab) -> Self {
        Self {
            id: tab.id.to_string(),
            title: tab.title.clone(),
            url: tab.url.clone(),
            pin_state: format!("{:?}", tab.pin_state).to_lowercase(),
            group_id: tab.group_id.clone(),
        }
    }
}

fn tab_title_from_url(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| "New Tab".to_string())
}

fn truncate_live_html(html: String) -> String {
    if html.len() <= MAX_LIVE_HTML_BYTES {
        return html;
    }
    let mut truncated = html;
    truncated.truncate(MAX_LIVE_HTML_BYTES);
    truncated
}

fn paginate_offset<T: Clone>(
    items: Vec<T>,
    params: OffsetListPageParams,
) -> SdkWorkPageData<T> {
    let total_items = items.len() as i64;
    let limit = params.page_size as usize;
    let offset = params.offset as usize;
    let page = offset_limit_page_from_iter(items.into_iter(), limit, offset);
    offset_list_page_data(page.items, total_items, params)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlatformError {
    #[error("invalid engine config: {0}")]
    InvalidEngine(String),
    #[error("engine not started")]
    EngineNotStarted,
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Engine(#[from] sdkwork_browser_engine_api_service::BrowserEngineError),
    #[error(transparent)]
    LlmConfig(#[from] LlmConfigError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_browser_engine_api_service::{BROWSER_ENGINE_CEF, BROWSER_ENGINE_WEBVIEW};

    #[tokio::test]
    async fn load_url_reuses_active_tab() {
        let mut platform = BrowserPlatform::bootstrap(BrowserConfig::webview()).unwrap();
        platform.start_engine().await.unwrap();
        platform.load_url("https://sdkwork.local/").await.unwrap();
        platform.load_url("https://example.com/").await.unwrap();
        assert_eq!(platform.tabs().list().len(), 1);
        assert_eq!(platform.tabs().list()[0].url, "https://example.com/");
    }

    #[tokio::test]
    async fn platform_starts_webview_engine_from_config() {
        let mut platform = BrowserPlatform::bootstrap(BrowserConfig::webview()).unwrap();
        platform.start_engine().await.unwrap();
        assert_eq!(platform.active_engine_id(), Some(BROWSER_ENGINE_WEBVIEW));
        platform.load_url("https://sdkwork.local/").await.unwrap();
        assert_eq!(platform.tabs().list().len(), 1);
    }

    #[tokio::test]
    async fn cef_surface_available_when_cef_engine_active() {
        let mut platform = BrowserPlatform::bootstrap(BrowserConfig::cef()).unwrap();
        platform.start_engine().await.unwrap();
        let surface = platform
            .cef_surface_descriptor("main", 1024, 768)
            .expect("cef surface");
        assert_eq!(surface.binding, "cef-stub");
        assert_eq!(surface.width, 1024);
        assert_eq!(surface.height, 768);
        assert_eq!(surface.embed_state, "stub");
    }

    #[tokio::test]
    async fn ai_action_navigate_opens_tab() {
        let mut platform = BrowserPlatform::bootstrap(BrowserConfig::webview()).unwrap();
        platform.start_engine().await.unwrap();
        let result = platform
            .execute_ai_action(BrowserAiActionRequest {
                action: "navigate".into(),
                target_url: Some("https://sdkwork.local/docs".into()),
                tab_id: None,
                message: None,
                engine_id: None,
            })
            .await
            .unwrap();
        assert_eq!(result.code, "OK");
        assert_eq!(platform.tabs().list().len(), 1);
    }

    #[tokio::test]
    async fn auto_group_tabs_assigns_group_ids() {
        let mut platform = BrowserPlatform::bootstrap(BrowserConfig::webview()).unwrap();
        platform.start_engine().await.unwrap();
        platform.load_url("https://github.com/org/repo").await.unwrap();
        platform
            .open_url_in_new_tab("http://localhost:8080")
            .await
            .unwrap();
        platform.auto_group_tabs();
        let groups: Vec<_> = platform
            .tabs()
            .list()
            .iter()
            .filter_map(|tab| tab.group_id.clone())
            .collect();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn list_operator_engines_includes_defaults() {
        let platform = BrowserPlatform::bootstrap(BrowserConfig::webview()).unwrap();
        let engines = platform.list_operator_engines();
        assert!(engines.iter().any(|engine| engine.id == BROWSER_ENGINE_WEBVIEW));
        assert!(engines.iter().any(|engine| engine.id == BROWSER_ENGINE_CEF));
    }
}
