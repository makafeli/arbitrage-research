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
async fn with_isolated<F, Fut>(test: F)
where
    F: FnOnce(Store, sqlx::PgPool) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let url =
        std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL required for account tests");
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let id = RequestId("account-test".into());
    let schema = format!("account_test_{}", &random_token(&id).unwrap()[..20]);
    // PostgreSQL identifiers cannot be bound parameters. The name is generated
    // solely from a fixed prefix and random hex, never from external input.
    assert!(
        schema
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let connection_schema = schema.clone();
    let opened = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let sql = format!("SET search_path TO {connection_schema}");
            Box::pin(async move {
                sqlx::query(&sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await;
    let pool = match opened {
        Ok(pool) => pool,
        Err(error) => {
            sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&admin)
                .await
                .unwrap();
            panic!("isolated test connection failed: {error}");
        }
    };
    let test_pool = pool.clone();
    let result = tokio::spawn(async move {
        sqlx::raw_sql(include_str!(
            "../../../migrations/0006_operator_account.sql"
        ))
        .execute(&test_pool)
        .await
        .unwrap();
        test(Store::from_pool(test_pool.clone()), test_pool).await;
    })
    .await;
    // Closing the shared pool also closes every Store clone used by the test.
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    if let Err(error) = result {
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        panic!("isolated account test was cancelled");
    }
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
    with_isolated(|store, _pool| async move {
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
    })
    .await;
}
#[tokio::test]
async fn guessed_wrong_email_expired_and_replayed_invites_never_replace_owner() {
    with_isolated(|store, pool| async move {
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
    })
    .await;
}
#[tokio::test]
async fn password_change_requires_current_password_csrf_and_revokes_other_process_sessions() {
    with_isolated(|store, _pool| async move {
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
    })
    .await;
}
#[tokio::test]
async fn recovery_link_is_single_use_and_revokes_current_sessions_only_on_redemption() {
    with_isolated(|store, _pool| async move {
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
    })
    .await;
}
#[tokio::test]
async fn first_owner_seed_never_renews_overwrites_or_reopens_registration() {
    with_isolated(|store, pool| async move {
        let expiry: String =
            sqlx::query_scalar("SELECT (clock_timestamp()+interval '1 hour')::text")
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
    })
    .await;
}
#[tokio::test]
async fn account_endpoints_reject_cross_origin_and_unauthenticated_password_change() {
    with_isolated(|store, _pool| async move {
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
    })
    .await;
}

#[tokio::test]
async fn exact_rate_limit_returns_retry_after_without_a_wall_clock_race() {
    with_isolated(|store, _pool| async move {
        let state = AppState::new(
            store,
            ServerConfig {
                listen_address: "127.0.0.1:8080".parse().unwrap(),
                public_origin: ORIGIN.into(),
                allow_insecure_loopback: true,
                operator_secret_hash: hash(SECRET),
                configurations: vec![],
                adapter_support: support::AdapterSupport::empty(),
            },
        );
        let id = RequestId("controlled-rate-test".into());
        let at = Instant::now();
        let client = account::RateClient(Some("192.0.2.10".parse().unwrap()));
        for _ in 0..10 {
            assert!(
                account::limit_attempt_at(
                    &state,
                    &id,
                    account::AuthFlow::SignIn,
                    client,
                    hash(EMAIL),
                    at
                )
                .is_ok()
            );
        }
        let response = account::limit_attempt_at(
            &state,
            &id,
            account::AuthFlow::SignIn,
            client,
            hash(EMAIL),
            at,
        )
        .err()
        .unwrap()
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "60");
        assert!(
            account::limit_attempt_at(
                &state,
                &id,
                account::AuthFlow::Activation,
                client,
                hash("private-link"),
                at
            )
            .is_ok()
        );
        assert!(
            account::limit_attempt_at(
                &state,
                &id,
                account::AuthFlow::Password,
                client,
                hash("authenticated-session"),
                at
            )
            .is_ok()
        );
    })
    .await;
}

#[tokio::test]
async fn bootstrap_reports_expiry_but_preserves_current_invitation_and_activated_account() {
    with_isolated(|store, pool| async move {
        let expired: String =
            sqlx::query_scalar("SELECT (clock_timestamp()-interval '1 hour')::text")
                .fetch_one(&pool)
                .await
                .unwrap();
        let valid: String =
            sqlx::query_scalar("SELECT (clock_timestamp()+interval '1 hour')::text")
                .fetch_one(&pool)
                .await
                .unwrap();
        let token = "b".repeat(64);
        let digest = hash(&token)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let metadata = |expiry: &str| {
            serde_json::to_vec(&json!({
                "schema_version":1, "origin":ORIGIN, "token_sha256":digest, "expires_at":expiry
            }))
            .unwrap()
        };
        assert!(
            account::seed_owner_bootstrap_bytes(&store, ORIGIN, &metadata(&expired))
                .await
                .is_err()
        );
        assert!(!store.operator_access_ready().await.unwrap());
        account::seed_owner_bootstrap_bytes(&store, ORIGIN, &metadata(&valid))
            .await
            .unwrap();
        // A source restart does not replace or renew the already valid invitation.
        account::seed_owner_bootstrap_bytes(&store, ORIGIN, &metadata(&expired))
            .await
            .unwrap();
        let api = app(store.clone());
        let response = post(
            &api,
            "/v1/auth/activate",
            json!({"email":EMAIL,"password":PASSWORD,"token":token}),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        account::seed_owner_bootstrap_bytes(&store, ORIGIN, &metadata(&expired))
            .await
            .unwrap();
        assert_eq!(store.operator_auth_version().await.unwrap(), 1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operator_account_invitation")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    })
    .await;
}
