//! Private single-operator account persistence. Password KDF is an API concern.
//! Invitations require administrative database access, expire and are single-use.
use super::{Store, StoreError};
use sqlx::Row;

// Credential material deliberately has no Debug or Serialize implementation.
pub struct OperatorAccount {
    pub email: String,
    pub salt: Vec<u8>,
    pub password_hash: Vec<u8>,
    pub auth_version: i64,
}
fn email_valid(email: &str) -> bool {
    (3..=254).contains(&email.len())
        && email.is_ascii()
        && email == email.to_lowercase()
        && email.bytes().all(|b| b > 32 && b < 127)
        && email.split('@').count() == 2
        && email.split('@').all(|p| !p.is_empty())
}
fn credentials_valid(salt: &[u8], password_hash: &[u8]) -> Result<(), StoreError> {
    if salt.len() != 32 || password_hash.len() != 32 {
        return Err(StoreError::InvalidInput("invalid account credentials"));
    }
    Ok(())
}
impl Store {
    /// Seeds the origin-bound public verifier exactly once, only before account
    /// activation. Source contains no bearer token. Expiry is never renewed.
    pub async fn seed_operator_invitation(
        &self,
        token_hash: &[u8],
        expires_at: &str,
    ) -> Result<bool, StoreError> {
        if token_hash.len() != 32 {
            return Err(StoreError::InvalidInput("invalid bootstrap verifier"));
        }
        let mut tx = self.pool.begin().await?;
        let version: i64 = sqlx::query_scalar(
            "SELECT auth_version FROM operator_account WHERE singleton FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        if version != 0 {
            return Ok(false);
        }
        let result=sqlx::query("INSERT INTO operator_account_invitation(singleton,token_hash,email,account_version,expires_at) SELECT true,$1,NULL,0,CAST($2 AS timestamptz) WHERE CAST($2 AS timestamptz)>clock_timestamp() AND CAST($2 AS timestamptz)<=clock_timestamp()+interval '48 hours' ON CONFLICT(singleton) DO NOTHING")
            .bind(token_hash).bind(expires_at).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }
    /// True only when an account exists or a current, unexpired invitation can
    /// provision it. This public readiness fact never exposes credential material.
    pub async fn operator_access_ready(&self) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT auth_version > 0 OR EXISTS(SELECT 1 FROM operator_account_invitation i WHERE i.singleton AND i.account_version=a.auth_version AND i.expires_at>clock_timestamp()) FROM operator_account a WHERE a.singleton"
        ).fetch_one(&self.pool).await?)
    }
    pub async fn operator_account(&self) -> Result<Option<OperatorAccount>, StoreError> {
        let row = sqlx::query("SELECT email,password_salt,password_hash,auth_version FROM operator_account WHERE singleton")
            .fetch_one(&self.pool).await?;
        let email: Option<String> = row.try_get("email")?;
        Ok(email.map(|email| OperatorAccount {
            email,
            salt: row.get("password_salt"),
            password_hash: row.get("password_hash"),
            auth_version: row.get("auth_version"),
        }))
    }
    pub async fn operator_auth_version(&self) -> Result<i64, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT auth_version FROM operator_account WHERE singleton")
                .fetch_one(&self.pool)
                .await?,
        )
    }
    /// Called only by the explicit administrative CLI, never a public endpoint.
    /// Reissuing revokes the previous link, but does not change the current login.
    pub async fn invite_operator(&self, email: &str, token_hash: &[u8]) -> Result<(), StoreError> {
        if !email_valid(email) || token_hash.len() != 32 {
            return Err(StoreError::InvalidInput("invalid account invitation"));
        }
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT email,auth_version FROM operator_account WHERE singleton FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        let existing: Option<String> = row.try_get("email")?;
        if existing.as_deref().is_some_and(|value| value != email) {
            return Err(StoreError::Conflict("account identity differs"));
        }
        let version: i64 = row.try_get("auth_version")?;
        sqlx::query("INSERT INTO operator_account_invitation(singleton,token_hash,email,account_version,expires_at) VALUES(true,$1,$2,$3,clock_timestamp()+interval '30 minutes') ON CONFLICT(singleton) DO UPDATE SET token_hash=EXCLUDED.token_hash,email=EXCLUDED.email,account_version=EXCLUDED.account_version,expires_at=EXCLUDED.expires_at,created_at=clock_timestamp()")
            .bind(token_hash).bind(email).bind(version).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Credential installation and one-time-token consumption are atomic. A
    /// concurrent claim cannot replace the winner or revive an older password.
    pub async fn redeem_operator_invitation(
        &self,
        token_hash: &[u8],
        email: &str,
        salt: &[u8],
        password_hash: &[u8],
    ) -> Result<i64, StoreError> {
        credentials_valid(salt, password_hash)?;
        if !email_valid(email) || token_hash.len() != 32 {
            return Err(StoreError::InvalidInput("invalid account invitation"));
        }
        let mut tx = self.pool.begin().await?;
        let version: i64 = sqlx::query_scalar(
            "SELECT auth_version FROM operator_account WHERE singleton FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM operator_account_invitation WHERE singleton AND token_hash=$1 AND (email=$2 OR (email IS NULL AND account_version=0)) AND account_version=$3 AND expires_at>clock_timestamp())")
            .bind(token_hash).bind(email).bind(version).fetch_one(&mut *tx).await?;
        if !allowed {
            return Err(StoreError::Conflict("invitation unavailable"));
        }
        let next = version.checked_add(1).ok_or(StoreError::CorruptState)?;
        sqlx::query("UPDATE operator_account SET email=$1,password_salt=$2,password_hash=$3,auth_version=$4,changed_at=clock_timestamp() WHERE singleton")
            .bind(email).bind(salt).bind(password_hash).bind(next).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM operator_account_invitation WHERE singleton")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(next)
    }
    pub async fn change_operator_password(
        &self,
        expected_version: i64,
        salt: &[u8],
        password_hash: &[u8],
    ) -> Result<(), StoreError> {
        credentials_valid(salt, password_hash)?;
        if expected_version <= 0 {
            return Err(StoreError::InvalidInput("account not activated"));
        }
        let mut tx = self.pool.begin().await?;
        let changed=sqlx::query("UPDATE operator_account SET password_salt=$1,password_hash=$2,auth_version=auth_version+1,changed_at=clock_timestamp() WHERE singleton AND auth_version=$3")
            .bind(salt).bind(password_hash).bind(expected_version).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(StoreError::Conflict("account changed"));
        }
        sqlx::query("DELETE FROM operator_account_invitation WHERE singleton")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
