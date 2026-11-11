use serde_json::{json, Value};

use crate::AgentCapability;

pub fn build_runtime_diagnostics(
    runtime_id: &str,
    runtime_mode: &str,
    capability_count: usize,
    mcp_connector_count: usize,
    llm_provider: Option<&str>,
) -> Value {
    let llm_configured = llm_provider.is_some();
    let mut degraded_capabilities: Vec<String> = Vec::new();
    if runtime_mode.eq_ignore_ascii_case("llm") && !llm_configured {
        degraded_capabilities.push("llm_inference".into());
    }

    let state = if degraded_capabilities.is_empty() {
        "ready"
    } else {
        "degraded"
    };

    let mut provider_diagnostics = vec![
        json!({
            "provider_id": "browser-rules-engine",
            "provider_family": "model",
            "provider_version": "0.1.0",
            "typed_registered": true,
            "health": { "status": "ready" },
            "capabilities": ["chat", "page_understanding"],
        }),
        json!({
            "provider_id": "browser-mcp-bridge",
            "provider_family": "mcp",
            "provider_version": "0.1.0",
            "typed_registered": true,
            "health": { "status": if mcp_connector_count > 0 { "ready" } else { "idle" } },
            "capabilities": ["tools/list", "tools/call"],
        }),
    ];

    if let Some(provider) = llm_provider {
        let provider_id = match provider {
            "http" => "browser-http-llm",
            _ => "browser-in-process-llm",
        };
        provider_diagnostics.push(json!({
            "provider_id": provider_id,
            "provider_family": "model",
            "provider_version": "0.1.0",
            "typed_registered": true,
            "health": { "status": "ready" },
            "capabilities": ["chat"],
        }));
    }

    json!({
        "schema_version": "agent_runtime_diagnostics.v1",
        "runtime_id": runtime_id,
        "agent_id": "sdkwork-browser-agent",
        "state": state,
        "provider_count": provider_diagnostics.len(),
        "capability_count": capability_count,
        "typed_provider_count": provider_diagnostics.len(),
        "manifest_only_provider_count": 0,
        "missing_required_capabilities": [],
        "degraded_capabilities": degraded_capabilities,
        "provider_diagnostics": provider_diagnostics,
        "runtime_mode": runtime_mode,
    })
}

pub fn capability_count(capabilities: &[AgentCapability]) -> usize {
    capabilities.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_ready_in_rules_mode() {
        let payload = build_runtime_diagnostics("sdkwork-agent-runtime", "rules", 6, 2, None);
        assert_eq!(payload["state"], "ready");
        assert_eq!(payload["schema_version"], "agent_runtime_diagnostics.v1");
        assert_eq!(payload["provider_count"], 2);
    }

    #[test]
    fn diagnostics_degraded_when_llm_missing_endpoint() {
        let payload = build_runtime_diagnostics("sdkwork-agent-runtime", "llm", 6, 2, None);
        assert_eq!(payload["state"], "degraded");
        assert!(payload["degraded_capabilities"]
            .as_array()
            .is_some_and(|items| items.iter().any(|v| v == "llm_inference")));
    }

    #[test]
    fn diagnostics_expose_the_in_process_llm_provider() {
        let payload =
            build_runtime_diagnostics("sdkwork-agent-runtime", "llm", 6, 2, Some("in_process"));
        assert_eq!(payload["state"], "ready");
        assert_eq!(payload["provider_count"], 3);
        assert!(payload["provider_diagnostics"]
            .as_array()
            .is_some_and(|items| items.iter().any(|v| v["provider_id"] == "browser-in-process-llm")));
    }
}
