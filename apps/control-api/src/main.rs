//! Single-operator, research-only control service. Secrets are never logged.
use control_api::{AppState, ServerConfig, router};
use std::error::Error;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.is_empty() || args.len() == 2 && args[0] == "--invite-owner") {
        return Err("Usage: control-api [--invite-owner EMAIL]".into());
    }
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
    if args.len() == 2 {
        use std::io::IsTerminal;
        // No sensitive link in service logs, redirected output or CI artifacts.
        if !std::io::stdout().is_terminal() {
            return Err("Owner invitation requires an interactive private terminal".into());
        }
        let link =
            control_api::create_owner_invitation(&store, &settings.public_origin, &args[1]).await?;
        println!("Private, one-use account setup/recovery link (expires in 30 minutes):\n{link}");
        return Ok(());
    }
    control_api::seed_owner_bootstrap(&store, &settings.public_origin).await?;
    control_api::require_owner_access(&store, &settings).await?;
    settings
        .register_configurations(&store)
        .await
        .map_err(|_| "Configuration registration failed; details redacted")?;
    let address = settings.listen_address;
    let state = AppState::new(store, settings);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Research control API listening on {address}; live execution unavailable");
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    Ok(())
}

async fn shutdown() {
    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("Could not install shutdown handler");
    }
}
