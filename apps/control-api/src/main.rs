//! Single-operator, research-only control service. Secrets are never logged.
use control_api::{AppState, ServerConfig, router};
use std::error::Error;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings =
        ServerConfig::from_env().map_err(|error| format!("API configuration: {error}"))?;
    let database_url = std::env::var("ARB_DATABASE_URL")
        .map_err(|_| "ARB_DATABASE_URL must be supplied through the process environment")?;
    let store = arb_storage::Store::connect(&database_url)
        .await
        .map_err(|_| "Database unavailable; credentials and connection details redacted")?;
    store
        .migrate()
        .await
        .map_err(|_| "Database migration failed; details redacted")?;
    settings
        .register_configurations(&store)
        .await
        .map_err(|_| "Configuration registration failed; details redacted")?;
    let address = settings.listen_address;
    let state = AppState::new(store, settings);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Research control API listening on {address}; live execution unavailable");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Could not install shutdown handler");
    }
}
