//! API assembly for sdkwork-browser.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
//! SDKWORK-ASSEMBLY-LIB-CUSTOM: exports beyond the canonical materializer template.

mod bootstrap;
mod generated;

pub use bootstrap::{assemble_api_router, ApiAssembly, ApiAssemblyRuntime, assemble_api_router_runtime, assemble_api_router_with_pool, web_module, web_module_with_pool};

pub use sdkwork_routes_browser_app_api::APP_API_PREFIX;
pub use sdkwork_routes_browser_backend_api::BACKEND_API_PREFIX;

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
