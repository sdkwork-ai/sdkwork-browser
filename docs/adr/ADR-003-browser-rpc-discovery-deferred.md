# ADR-003: Browser RPC and Discovery Integration Deferred

Status: accepted (deferred integration)  
Owner: sdkwork-browser  
Date: 2026-06-19  
Specs: RPC_SPEC.md, DEPLOYMENT_SPEC.md, ARCHITECTURE_DECISION_SPEC.md, TEST_SPEC.md

## Context

SDKWork Browser ships:

- HTTP route manifests via `sdkwork-routes-browser-app-api` and `sdkwork-routes-browser-backend-api`
- `sdkwork-web-framework` bootstrap for future HTTP gateway integration
- Persistence contracts via `sdkwork-platform-browser-repository-sqlx` and `sdkwork-database`
- Desktop-first runtime (Tauri/native engines) with optional cloud HTTP surface

There is **no hosted gRPC service process** and no `apis/rpc/` proto authority in this workspace yet.

## Decision

**Defer `sdkwork-discovery` integration until the first browser RPC service host ships.**

Until then:

1. Keep HTTP as the only remote transport contract (`apis/app-api/`, `apis/backend-api/`).
2. Do **not** add `sdkwork-discovery` to root `Cargo.toml` or `agent.workflow.json`.
3. Document phased adoption in this ADR and `specs/README.md`.

## Phase 1 — First hosted RPC service (prerequisite for discovery)

Ship one runnable gRPC host when browser cloud sync or distributed agent runtime requires RPC.

Requirements:

- Proto authority under `apis/rpc/`
- Generated `sdkwork-browser-rpc-sdk`
- Thin tonic server using existing domain services (no raw SQLx in RPC adapters)

## Phase 2 — Discovery registration

After Phase 1 host runs in split-services topology:

- Checkout `sdkwork-discovery` in workflow dependencies
- Register RPC host via discovery client crate
- Update topology profiles with discovery-backed service URLs

## Verification

- `pnpm run test:governance` asserts discovery is documented as deferred
- No `sdkwork-discovery` dependency in workspace manifests until Phase 2
