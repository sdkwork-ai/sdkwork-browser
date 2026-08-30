# ADR-002: Browser Engine Standard and Registry

- Status: accepted
- Date: 2026-06-19
- Supersedes: partial refinement of ADR-001 runtime profile model

## Context

PRD defines SDKWORK Browser as a **Browser Runtime Platform** with pluggable engines (`webview`, `servo`, `cef`) behind a **Browser Engine Standard**, not as a monolithic browser product.

## Decision

1. **`BrowserEngine` trait** lives in `sdkwork-browser-engine-api-service` (PRD `browser-engine-api`).
2. **Registry** lives in `sdkwork-browser-engine-registry-service` with `register` / `get` / `bootstrap_default_registry`.
3. **Engine implementations**:
   - `sdkwork-browser-engine-webview-service` (V1 default, ~10MB)
   - `sdkwork-browser-engine-cef-service` (V3 enterprise, ~200MB, `cef` feature for binding)
   - `sdkwork-browser-engine-servo-service` (V4 roadmap, ~80MB stub in V1)
4. **Browser API** (top layer) is `sdkwork-browser-platform-service` — config YAML, tabs, agent, MCP.
5. **Runtime host** is `sdkwork-browser-service-host` — builds `BrowserPlatform` from engine id.
6. YAML config (`browser.engine`) in `etc/profiles/browser.*.yaml` switches engines without business code changes.

## SDKWork naming map (PRD → compliant crate)

| PRD crate | SDKWork crate |
| --- | --- |
| browser-engine-api | `sdkwork-browser-engine-api-service` |
| browser-engine-registry | `sdkwork-browser-engine-registry-service` |
| browser-engine-webview | `sdkwork-browser-engine-webview-service` |
| browser-engine-cef | `sdkwork-browser-engine-cef-service` |
| browser-engine-servo | `sdkwork-browser-engine-servo-service` |
| browser-core | `sdkwork-browser-platform-service` |
| browser-runtime | `sdkwork-browser-service-host` |
| browser-agent | `sdkwork-browser-agent-service` |
| browser-mcp | `sdkwork-browser-mcp-service` |

Forbidden names (`browser-core`, `browser-runtime` as crate suffixes) are not used.

## Roadmap alignment

| Version | Deliverable |
| --- | --- |
| V1 | WebView engine + platform API |
| V2 | SDK families (rust/java/node/python) |
| V3 | CEF binding |
| V4 | Servo binding |
| V5 | Multi-engine dynamic switch (registry + config — scaffold complete in V1) |
