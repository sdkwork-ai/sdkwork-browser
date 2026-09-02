mod gateway_manifest;

use gateway_manifest::browser_gateway_public_path_prefixes;
use sdkwork_api_browser_assembly::assemble_api_router_runtime;
use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_database_pool_for_audiences,
    iam_web_request_context_resolver_from_env, IamAuditEmitter, IamSecurityEventEmitter,
};
use sdkwork_web_bootstrap::ApiModuleRegistry;
use std::sync::Arc;

const APPLICATION_ID: &str = "sdkwork-browser";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let bind = std::env::var("BROWSER_APP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let runtime = assemble_api_router_runtime().await?;
    let assembly = runtime.contribution;
    let environment = std::env::var("SDKWORK_ENVIRONMENT")
        .or_else(|_| std::env::var("SDKWORK_BROWSER_ENVIRONMENT"))
        .unwrap_or_else(|_| "development".to_owned());
    let production = matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "prod" | "production"
    );
    let resolver = if production {
        iam_web_request_context_resolver_from_database_pool_for_audiences(
            runtime.database_pool.clone(),
            &[APPLICATION_ID],
        )
        .await?
    } else {
        iam_web_request_context_resolver_from_env().await
    };
    let mut framework = build_web_framework_builder(
        resolver,
        assembly.route_manifest.clone(),
        browser_gateway_public_path_prefixes(),
    );
    if production {
        let postgres_pool = runtime
            .database_pool
            .as_postgres()
            .cloned()
            .ok_or("production Browser gateway requires PostgreSQL")?;
        framework = framework
            .audit_emitter(Arc::new(IamAuditEmitter::new(
                postgres_pool.clone(),
                APPLICATION_ID,
                environment.clone(),
            )))
            .security_event_emitter(Arc::new(IamSecurityEventEmitter::new(
                postgres_pool,
                environment,
            )));
    }
    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_modules(vec![assembly]);
    let router = module_registry
        .try_compose("SDKWork Browser API")?
        .into_hosted(framework)
        .router;

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "sdkwork-browser standalone-gateway listening (app-api + backend-api)");
    axum::serve(listener, router).await?;
    Ok(())
}
