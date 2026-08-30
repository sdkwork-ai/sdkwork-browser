# SDKWork Browser
repository-kind: application

**Browser Runtime Platform** — not a browser app, not a single engine, but a pluggable **Browser Engine Standard** + **Browser Engine Registry** for the SDKWork ecosystem.

## Positioning (PRD)

- sdkwork-specs First
- Engine Pluggable (`webview` | `servo` | `cef`)
- SDK First / Component First
- Agent Native (`sdkwork-agent-runtime`)
- MCP Native (`sdkwork-mcp-runtime`)

## Architecture

```text
Browser API (sdkwork-browser-platform-service)
        │
Browser Engine Registry
        │
  webview / servo / cef  engines
```

Switch engines via YAML — no business code changes:

```yaml
browser:
  engine: webview   # or servo | cef
```

Profiles: `etc/profiles/browser.*.yaml`

## Surfaces

| PRD | SDKWork path |
| --- | --- |
| desktop | `apps/sdkwork-browser-pc` (+ `apps/desktop` index) |
| mobile | `apps/sdkwork-browser-h5`, `apps/sdkwork-browser-flutter-mobile` |

## SDK families (V2+)

`sdks/sdkwork-browser-{rust,java,node,python}-sdk/`

## Development

```powershell
pnpm install
pnpm verify
cargo test --workspace
```

Docs: [`docs/prd-alignment.md`](docs/prd-alignment.md) · [`docs/adr/ADR-002-browser-engine-standard.md`](docs/adr/ADR-002-browser-engine-standard.md)

## Documentation Canon

- [docs/README.md](docs/README.md)
- [docs/product/prd/PRD.md](docs/product/prd/PRD.md)
- [docs/architecture/tech/TECH_ARCHITECTURE.md](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Application Roots

- [apps directory index](apps/README.md)
