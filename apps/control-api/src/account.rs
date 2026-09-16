//! Account login and private activation, independent of trading capabilities.
use super::*;
use ring::pbkdf2;
use std::num::NonZeroU32;

const ITERATIONS: u32 = 600_000;
fn iterations() -> NonZeroU32 {
    NonZeroU32::new(ITERATIONS).expect("nonzero KDF work factor")
}
pub(super) fn unauthorized(id: &RequestId) -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "AUTHENTICATION_FAILED",
        "Email, password or access link is not valid",
        id,
    )
}
pub(super) fn limit_attempt(state: &AppState, id: &RequestId) -> Result<(), ApiError> {
    let mut auth = state.0.auth.lock().map_err(|_| ApiError::unavailable(id))?;
    let window = now() / 60;
    if auth.login_window != window {
        auth.login_window = window;
        auth.login_attempts = 0;
    }
    auth.login_attempts = auth.login_attempts.saturating_add(1);
    if auth.login_attempts > LOGIN_ATTEMPTS_PER_MINUTE {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many sign-in attempts. Please try again in a minute.",
            id,
        ));
    }
    Ok(())
}
fn email(value: &str) -> Option<String> {
    let s = value.trim().to_ascii_lowercase();
    if !(3..=254).contains(&s.len())
        || !s.is_ascii()
        || s.bytes().any(|b| b <= 32 || b >= 127)
        || s.split('@').count() != 2
        || s.split('@').any(str::is_empty)
    {
        return None;
    }
    Some(s)
}
fn new_password_valid(password: &str) -> bool {
    (15..=128).contains(&password.chars().count()) && password.len() <= 512
}
fn input_password_valid(password: &str) -> bool {
    !password.is_empty() && password.len() <= 512
}
fn salt(id: &RequestId) -> Result<[u8; 32], ApiError> {
    let mut value = [0; 32];
    getrandom::fill(&mut value).map_err(|_| ApiError::unavailable(id))?;
    Ok(value)
}
async fn derive(
    state: &AppState,
    id: &RequestId,
    password: String,
    salt: [u8; 32],
) -> Result<[u8; 32], ApiError> {
    let permit = state
        .0
        .password_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Sign-in is busy; please try again shortly",
                id,
            )
        })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut output = [0; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations(),
            &salt,
            password.as_bytes(),
            &mut output,
        );
        output
    })
    .await
    .map_err(|_| ApiError::unavailable(id))
}
async fn verify(
    state: &AppState,
    id: &RequestId,
    password: String,
    account: Option<arb_storage::OperatorAccount>,
    entered_email: Option<String>,
) -> Result<Option<i64>, ApiError> {
    let permit = state
        .0
        .password_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Sign-in is busy; please try again shortly",
                id,
            )
        })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // Missing accounts and incorrect usernames perform the same bounded KDF.
        let (salt, expected) = account
            .as_ref()
            .map(|a| (a.salt.as_slice(), a.password_hash.as_slice()))
            .unwrap_or((&[0; 32], &[0; 32]));
        let valid = pbkdf2::verify(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations(),
            salt,
            password.as_bytes(),
            expected,
        )
        .is_ok();
        account
            .filter(|a| valid && entered_email.as_ref().is_none_or(|e| e == &a.email))
            .map(|a| a.auth_version)
    })
    .await
    .map_err(|_| ApiError::unavailable(id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignIn {
    email: String,
    password: String,
}
pub(super) async fn sign_in(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    body: Result<Json<SignIn>, JsonRejection>,
) -> Result<Response, ApiError> {
    limit_attempt(&state, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    let email = email(&input.email).ok_or_else(|| unauthorized(&id))?;
    if !input_password_valid(&input.password) {
        return Err(unauthorized(&id));
    }
    let account = state
        .0
        .store
        .operator_account()
        .await
        .map_err(|_| ApiError::unavailable(&id))?;
    let version = verify(&state, &id, input.password, account, Some(email))
        .await?
        .ok_or_else(|| unauthorized(&id))?;
    new_auth_session(&state, &id, version)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Activate {
    email: String,
    password: String,
    token: String,
}
pub(super) async fn activate(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    body: Result<Json<Activate>, JsonRejection>,
) -> Result<Response, ApiError> {
    limit_attempt(&state, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    let email = email(&input.email).ok_or_else(|| ApiError::invalid(&id))?;
    if !new_password_valid(&input.password) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PASSWORD_POLICY",
            "Use 15 to 128 characters for your password",
            &id,
        ));
    }
    if input.token.len() != 64 || !input.token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(unauthorized(&id));
    }
    let salt = salt(&id)?;
    let digest = derive(&state, &id, input.password, salt).await?;
    state
        .0
        .store
        .redeem_operator_invitation(&hash(&input.token), &email, &salt, &digest)
        .await
        .map_err(|e| match e {
            StoreError::Conflict(_) => unauthorized(&id),
            _ => ApiError::unavailable(&id),
        })?;
    state
        .0
        .auth
        .lock()
        .map_err(|_| ApiError::unavailable(&id))?
        .sessions
        .clear();
    // Explicitly sign in after activation. An uncertain response can be reconciled
    // by trying the chosen credentials rather than reusing a consumed token.
    Ok(StatusCode::NO_CONTENT.into_response())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChangePassword {
    current_password: String,
    new_password: String,
}
pub(super) async fn change_password(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    Extension(identity): Extension<Identity>,
    body: Result<Json<ChangePassword>, JsonRejection>,
) -> Result<Response, ApiError> {
    limit_attempt(&state, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    if !input_password_valid(&input.current_password) || !new_password_valid(&input.new_password) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PASSWORD_POLICY",
            "Use 15 to 128 characters for your new password",
            &id,
        ));
    }
    let account = state
        .0
        .store
        .operator_account()
        .await
        .map_err(|_| ApiError::unavailable(&id))?;
    let version = verify(&state, &id, input.current_password, account, None)
        .await?
        .ok_or_else(|| unauthorized(&id))?;
    if version != identity.auth_version {
        return Err(unauthorized(&id));
    }
    let salt = salt(&id)?;
    let digest = derive(&state, &id, input.new_password, salt).await?;
    state
        .0
        .store
        .change_operator_password(version, &salt, &digest)
        .await
        .map_err(|_| ApiError::unavailable(&id))?;
    state
        .0
        .auth
        .lock()
        .map_err(|_| ApiError::unavailable(&id))?
        .sessions
        .clear();
    Ok(StatusCode::NO_CONTENT.into_response())
}
/// Explicit administrative operation run in the existing private API service.
/// The returned link is a credential. Never send it to logs, tickets or CI output.
pub async fn create_owner_invitation(
    store: &Store,
    origin: &str,
    owner_email: &str,
) -> Result<String, &'static str> {
    let owner_email = email(owner_email).ok_or("Invalid owner email")?;
    let id = RequestId("private-owner-invitation".into());
    let token = random_token(&id).map_err(|_| "Invitation generation failed")?;
    store
        .invite_operator(&owner_email, &hash(&token))
        .await
        .map_err(|_| "Invitation could not be created; account email must match")?;
    Ok(format!("{origin}/#activate={token}"))
}

/// Load public, origin-bound first-owner verification metadata, never a password.
/// A missing file is an unprovisioned private deployment, not open registration.
pub async fn seed_owner_bootstrap(store: &Store, origin: &str) -> Result<(), &'static str> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Bootstrap {
        schema_version: u32,
        origin: String,
        token_sha256: String,
        expires_at: String,
    }
    let path = std::env::var("ARB_OWNER_BOOTSTRAP_FILE")
        .unwrap_or_else(|_| "config/owner-bootstrap.json".into());
    let bytes = match std::fs::read(path) {
        Ok(b) if b.len() <= 2048 => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        _ => return Err("Owner bootstrap metadata unavailable"),
    };
    let value: Bootstrap =
        serde_json::from_slice(&bytes).map_err(|_| "Owner bootstrap metadata invalid")?;
    if value.origin != origin {
        return Ok(());
    }
    if value.schema_version != 1
        || value.token_sha256.len() != 64
        || !value.token_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Owner bootstrap verifier invalid");
    }
    let digest: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&value.token_sha256[i..i + 2], 16))
        .collect::<Result<_, _>>()
        .map_err(|_| "Owner bootstrap verifier invalid")?;
    store
        .seed_operator_invitation(&digest, &value.expires_at)
        .await
        .map_err(|_| "Owner bootstrap could not be stored")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_policy_allows_long_unicode_passphrases_without_truncation() {
        assert!(!new_password_valid("short"));
        assert!(new_password_valid("a long passphrase is welcome"));
        assert!(new_password_valid(&"界".repeat(15)));
        assert!(!new_password_valid(&"x".repeat(129)));
        assert_eq!(
            email(" Owner@Example.COM ").as_deref(),
            Some("owner@example.com")
        );
        assert!(email("bad@@example.com").is_none());
    }
    #[test]
    fn kdf_matches_independent_python_vector_and_refuses_wrong_password() {
        let mut result = [0; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations(),
            &[7; 32],
            b"correct horse battery staple",
            &mut result,
        );
        let hex = result
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "59a9d543010c4762aac49a99f88ebb60af42c55eb3a773ef6e5b98312a567b96"
        );
        assert!(
            pbkdf2::verify(
                pbkdf2::PBKDF2_HMAC_SHA256,
                iterations(),
                &[7; 32],
                b"wrong",
                &result
            )
            .is_err()
        );
    }
}
