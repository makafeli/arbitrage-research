//! Request-duration contract tests against the actual router and disposable store.
use super::*;
use axum::body::{Body, to_bytes};
use tower::ServiceExt;

#[tokio::test]
async fn request_timing_is_correlated_bounded_and_does_not_change_authentication() {
    let database = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL is required for API timing acceptance");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let secret = "synthetic-timing-password-at-least-32-bytes";
    let origin = "http://127.0.0.1:5173";
    let app = router(AppState::new(
        store,
        ServerConfig {
            listen_address: "127.0.0.1:8080".parse().unwrap(),
            public_origin: origin.into(),
            allow_insecure_loopback: true,
            operator_secret_hash: hash(secret),
            adapter_support: support::AdapterSupport::empty(),
            configurations: vec![],
        },
    ));
    let request = Request::builder()
        .uri("/v1/capabilities")
        .body(Body::empty())
        .unwrap();
    let denied = app.clone().oneshot(request).await.unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    check_headers(&denied);
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("Origin", origin)
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({"operator_secret": secret}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    check_headers(&login);
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let allowed = app
        .oneshot(
            Request::builder()
                .uri("/v1/capabilities")
                .header("Cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    check_headers(&allowed);
    let bytes = to_bytes(allowed.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["live_execution"], false);
    assert!(!String::from_utf8(bytes.to_vec()).unwrap().contains(secret));
}

fn check_headers(response: &Response) {
    let timing = response.headers()["server-timing"].to_str().unwrap();
    let numeric = timing.strip_prefix("api;dur=").unwrap();
    let (milliseconds, micros) = numeric.split_once('.').unwrap();
    assert!(milliseconds.parse::<u128>().is_ok());
    assert_eq!(micros.len(), 3);
    assert!(micros.bytes().all(|b| b.is_ascii_digit()));
    assert!(!timing.contains("secret"));
    assert!(
        response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .starts_with("req-")
    );
}
