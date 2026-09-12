//! Development liveness process only. The authenticated /v1 API is unimplemented.

use axum::{Router, http::header, routing::get};
use std::{error::Error, net::Ipv4Addr};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = Router::new().route(
        "/healthz",
        get(|| async {
            (
                [(header::CONTENT_TYPE, "application/json")],
                concat!(
                    "{\"status\":\"OK\",\"service\":\"control-api-scaffold\",",
                    "\"version\":\"",
                    env!("CARGO_PKG_VERSION"),
                    "\",",
                    "\"trading_available\":false,\"persistence_available\":false}"
                ),
            )
        }),
    );
    // Fixed loopback binding: this unauthenticated liveness harness must not
    // become a public control surface through an environment override.
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 8080)).await?;
    eprintln!("Development liveness only: http://127.0.0.1:8080/healthz");
    eprintln!("No sessions, persistence, market feeds, signing or trading are implemented.");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("Could not install Ctrl-C handler: {error}");
    }
}
