# SDKWORK Browser — PRD alignment index

Product: **Browser Runtime Platform** + **Browser Engine Standard** + **Browser Engine Registry**

Authoritative specs:

- PRD capabilities → [`technical-architecture.md`](./technical-architecture.md)
- Engine Standard → [`adr/ADR-002-browser-engine-standard.md`](./adr/ADR-002-browser-engine-standard.md)
- SDKWork naming → [`adr/ADR-001-dual-browser-runtime.md`](./adr/ADR-001-dual-browser-runtime.md)

## V1 delivered (this repository)

- [x] `BrowserEngine` trait (`sdkwork-browser-engine-api-service`)
- [x] Engine registry with `webview` / `servo` / `cef`
- [x] WebView engine stub (default)
- [x] CEF + Servo engine stubs for registry parity
- [x] `BrowserPlatform` API (`sdkwork-browser-platform-service`)
- [x] YAML config profiles (`etc/profiles/browser.*.yaml`)
- [x] Tab / bookmark / history / download / cookie / session / storage modules
- [x] Agent + MCP service ports
- [x] Tri-surface apps (PC / H5 / Flutter)
- [x] SDK family placeholders (rust/java/node/python)

## Roadmap (PRD §14)

| Phase | Focus | Status |
| --- | --- | --- |
| V1 | WebView runtime | Scaffold + tests |
| V2 | SDK integration | SDK dirs created |
| V3 | CEF runtime | Stub; `cef` feature gate |
| V4 | Servo runtime | Stub |
| V5 | Multi-engine switch | Registry + UI + YAML ready |
