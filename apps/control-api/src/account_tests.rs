//! Actual PostgreSQL and HTTP account tests; no provider or real user credentials.
use super::*;
use axum::body::{Body, to_bytes};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
const ORIGIN: &str = "http://127.0.0.1:5173";
const SECRET: &str = "synthetic-bootstrap-only-secret-00000000";
const EMAIL: &str = "owner@example.test";
const PASSWORD: &str = "correct horse battery staple";
const NEXT: &str = "another complete password phrase";
async fn isolated() -> (Store, sqlx::PgPool) {
    let url =
        std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL required for account tests");
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let id = RequestId("account-test".into());
    let schema = format!("account_test_{}", &random_token(&id).unwrap()[..20]);
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let sql = format!("SET search_path TO {schema}");
            Box::pin(async move {
                sqlx::query(&sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/0006_operator_account.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    (Store::from_pool(pool.clone()), pool)
}
fn app(store: Store) -> Router {
    router(AppState::new(
        store,
        ServerConfig {
            listen_address: "127.0.0.1:8080".parse().unwrap(),
            public_origin: ORIGIN.into(),
            allow_insecure_loopback: true,
            operator_secret_hash: hash(SECRET),
            configurations: vec![],
            adapter_support: support::AdapterSupport::empty(),
        },
    ))
}
async fn post(
    app: &Router,
    path: &str,
    body: Value,
    credentials: Option<&(String, String)>,
) -> Response {
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("origin", ORIGIN)
        .header("content-type", "application/json");
    if let Some((cookie, csrf)) = credentials {
        request = request
            .header("cookie", cookie)
            .header("x-csrf-token", csrf);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn credentials(response: Response) -> (String, String) {
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.unwrap()).unwrap();
    assert_eq!(value["operator_id"], "operator");
    assert!(value.get("password_hash").is_none());
    (cookie, value["csrf_token"].as_str().unwrap().to_owned())
}
async fn auth(app: &Router, cookie: &str) -> StatusCode {
    app.clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/auth/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}
async fn activate(store: &Store, app: &Router) {
    let link = create_owner_invitation(store, ORIGIN, EMAIL).await.unwrap();
    let token = link.split("#activate=").nth(1).unwrap();
    assert_eq!(
        post(
            app,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":PASSWORD,"token":token}),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}
#[tokio::test]
async fn account_survives_new_api_and_bootstrap_cannot_bypass_it() {
    let (store, _) = isolated().await;
    let before = app(store.clone());
    let legacy = credentials(
        post(
            &before,
            "/v1/auth/login",
            json!({"operator_secret":SECRET}),
            None,
        )
        .await,
    )
    .await;
    activate(&store, &before).await;
    assert_eq!(auth(&before, &legacy.0).await, StatusCode::UNAUTHORIZED);
    let after = app(store.clone());
    assert_eq!(
        post(
            &after,
            "/v1/auth/login",
            json!({"operator_secret":SECRET}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let login = credentials(
        post(
            &after,
            "/v1/auth/sign-in",
            json!({"email":"OWNER@example.test","password":PASSWORD}),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(auth(&after, &login.0).await, StatusCode::OK);
    let row = store.operator_account().await.unwrap().unwrap();
    assert_eq!(row.email, EMAIL);
    assert_eq!(row.auth_version, 1);
    assert_eq!(row.salt.len(), 32);
    assert_eq!(row.password_hash.len(), 32);
    assert_ne!(row.password_hash.as_slice(), PASSWORD.as_bytes());
}
#[tokio::test]
async fn guessed_wrong_email_expired_and_replayed_invites_never_replace_owner() {
    let (store, pool) = isolated().await;
    let api = app(store.clone());
    let link = create_owner_invitation(&store, ORIGIN, EMAIL)
        .await
        .unwrap();
    let token = link.split('=').next_back().unwrap();
    for (e, t) in [
        (EMAIL, "1".repeat(64)),
        ("attacker@example.test", token.into()),
    ] {
        assert_eq!(
            post(
                &api,
                "/v1/auth/activate",
                json!({"email":e,"password":PASSWORD,"token":t}),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    sqlx::query(
        "UPDATE operator_account_invitation SET expires_at=clock_timestamp()-interval '1 second'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        post(
            &api,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":PASSWORD,"token":token}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(store.operator_account().await.unwrap().is_none());
    activate(&store, &api).await;
    assert_eq!(
        post(
            &api,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":NEXT,"token":token}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(store.operator_auth_version().await.unwrap(), 1);
}
#[tokio::test]
async fn password_change_requires_current_password_csrf_and_revokes_other_process_sessions() {
    let (store, _) = isolated().await;
    let a = app(store.clone());
    activate(&store, &a).await;
    let b = app(store.clone());
    let first = credentials(
        post(
            &a,
            "/v1/auth/sign-in",
            json!({"email":EMAIL,"password":PASSWORD}),
            None,
        )
        .await,
    )
    .await;
    let other = credentials(
        post(
            &b,
            "/v1/auth/sign-in",
            json!({"email":EMAIL,"password":PASSWORD}),
            None,
        )
        .await,
    )
    .await;
    let wrong_csrf = (first.0.clone(), "invalid".into());
    assert_eq!(
        post(
            &a,
            "/v1/auth/password",
            json!({"current_password":PASSWORD,"new_password":NEXT}),
            Some(&wrong_csrf)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &a,
            "/v1/auth/password",
            json!({"current_password":"wrong","new_password":NEXT}),
            Some(&first)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &a,
            "/v1/auth/password",
            json!({"current_password":PASSWORD,"new_password":NEXT}),
            Some(&first)
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(auth(&b, &other.0).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        post(
            &b,
            "/v1/auth/sign-in",
            json!({"email":EMAIL,"password":PASSWORD}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let new = credentials(
        post(
            &b,
            "/v1/auth/sign-in",
            json!({"email":EMAIL,"password":NEXT}),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(auth(&b, &new.0).await, StatusCode::OK);
}
#[tokio::test]
async fn recovery_link_is_single_use_and_revokes_current_sessions_only_on_redemption() {
    let (store, _) = isolated().await;
    let a = app(store.clone());
    activate(&store, &a).await;
    let b = app(store.clone());
    let old = credentials(
        post(
            &b,
            "/v1/auth/sign-in",
            json!({"email":EMAIL,"password":PASSWORD}),
            None,
        )
        .await,
    )
    .await;
    let link = create_owner_invitation(&store, ORIGIN, EMAIL)
        .await
        .unwrap();
    let token = link.split('=').next_back().unwrap();
    assert_eq!(auth(&b, &old.0).await, StatusCode::OK);
    assert_eq!(
        post(
            &a,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":NEXT,"token":token}),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(auth(&b, &old.0).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        post(
            &a,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":PASSWORD,"token":token}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        create_owner_invitation(&store, ORIGIN, "other@example.test")
            .await
            .is_err()
    );
}
#[tokio::test]
async fn first_owner_seed_never_renews_overwrites_or_reopens_registration() {
    let (store, pool) = isolated().await;
    let expiry: String = sqlx::query_scalar("SELECT (clock_timestamp()+interval '1 hour')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    let token = "a".repeat(64);
    let digest = hash(&token);
    assert!(
        store
            .seed_operator_invitation(&digest, &expiry)
            .await
            .unwrap()
    );
    assert!(
        !store
            .seed_operator_invitation(&hash("other"), &expiry)
            .await
            .unwrap()
    );
    let (a, b) = tokio::join!(
        store.redeem_operator_invitation(&digest, EMAIL, &[1; 32], &[2; 32]),
        store.redeem_operator_invitation(&digest, "other@example.test", &[3; 32], &[4; 32])
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(store.operator_auth_version().await.unwrap(), 1);
    assert!(
        !store
            .seed_operator_invitation(&digest, &expiry)
            .await
            .unwrap()
    );
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM operator_account_invitation")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 0);
}
#[tokio::test]
async fn account_endpoints_reject_cross_origin_unauthenticated_change_and_excess_attempts() {
    let (store, _) = isolated().await;
    let api = app(store);
    let response = api
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/activate")
                .header("origin", "https://evil.example")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        post(
            &api,
            "/v1/auth/password",
            json!({"current_password":PASSWORD,"new_password":NEXT}),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    for _ in 0..10 {
        assert_eq!(
            post(
                &api,
                "/v1/auth/sign-in",
                json!({"email":EMAIL,"password":"wrong"}),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let limited = post(
        &api,
        "/v1/auth/sign-in",
        json!({"email":EMAIL,"password":PASSWORD}),
        None,
    )
    .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()[header::RETRY_AFTER], "60");
}
