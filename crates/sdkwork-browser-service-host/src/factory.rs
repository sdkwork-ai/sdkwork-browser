use sdkwork_browser_engine_api_service::BrowserEngineId;
use sdkwork_browser_platform_service::{BrowserConfig, BrowserPlatform};

pub struct BrowserRuntimeFactory;

impl BrowserRuntimeFactory {
    /// Resolves a [`BrowserPlatform`] from YAML-style engine id (PRD §8).
    pub fn create_platform(engine: &str) -> Result<BrowserPlatform, FactoryError> {
        let config = BrowserConfig {
            engine: engine.to_string(),
        };
        if config.engine_id().is_none() {
            return Err(FactoryError::InvalidEngine(engine.to_string()));
        }
        BrowserPlatform::bootstrap(config).map_err(FactoryError::Platform)
    }

    pub fn create_webview_platform() -> Result<BrowserPlatform, FactoryError> {
        BrowserPlatform::bootstrap(BrowserConfig::webview()).map_err(FactoryError::Platform)
    }

    pub fn create_servo_platform() -> Result<BrowserPlatform, FactoryError> {
        BrowserPlatform::bootstrap(BrowserConfig::servo()).map_err(FactoryError::Platform)
    }

    pub fn create_cef_platform() -> Result<BrowserPlatform, FactoryError> {
        BrowserPlatform::bootstrap(BrowserConfig::cef()).map_err(FactoryError::Platform)
    }

    pub fn engine_id_from_profile(profile: BrowserEngineId) -> &'static str {
        profile.as_str()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FactoryError {
    #[error("invalid engine: {0}")]
    InvalidEngine(String),
    #[error(transparent)]
    Platform(#[from] sdkwork_browser_platform_service::PlatformError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn factory_builds_platform_per_engine_yaml() {
        use sdkwork_browser_engine_api_service::{
            BROWSER_ENGINE_CEF, BROWSER_ENGINE_SERVO, BROWSER_ENGINE_WEBVIEW,
        };
        for engine in [BROWSER_ENGINE_WEBVIEW, BROWSER_ENGINE_SERVO, BROWSER_ENGINE_CEF] {
            let mut platform = BrowserRuntimeFactory::create_platform(engine).unwrap();
            platform.start_engine().await.unwrap();
            assert_eq!(platform.active_engine_id(), Some(engine));
        }
    }
}
