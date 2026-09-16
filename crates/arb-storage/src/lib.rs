//! Durable research sessions and revisioned operator intent. No signing or broadcast.
mod accounts;
pub use accounts::*;
mod collection;
mod ingestion;
pub use ingestion::*;
mod costs;
mod decisions;
mod exports;
pub use collection::*;
pub use costs::*;
pub use exports::*;
mod paper;
mod types;
pub use decisions::*;
pub use paper::*;
mod worker;

pub use types::*;
pub use worker::{WorkerClaim, WorkerUpdate};

use arb_domain::{Action, Mode, Session, SessionSnapshot};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::{PgPoolOptions, PgRow},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        Ok(Self {
            pool: PgPoolOptions::new().max_connections(8).connect(url).await?,
        })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("../../migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn health(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Register a validated, redacted configuration. The caller owns config schema validation.
    /// A digest cannot be reassigned to different content, even by another retry.
    pub async fn save_configuration(
        &self,
        operator: &str,
        digest: &str,
        snapshot: Value,
    ) -> Result<(), StoreError> {
        bounded(operator, 200, "invalid operator")?;
        bounded(digest, 200, "invalid configuration digest")?;
        if serde_json::to_vec(&snapshot)
            .map_err(|_| StoreError::InvalidInput("invalid snapshot"))?
            .len()
            > 1_048_576
        {
            return Err(StoreError::InvalidInput(
                "configuration snapshot exceeds 1 MiB",
            ));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO configuration_snapshots(operator_id,configuration_digest,snapshot) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(operator).bind(digest).bind(&snapshot).execute(&mut *tx).await?;
        let saved: Value = sqlx::query_scalar("SELECT snapshot FROM configuration_snapshots WHERE operator_id=$1 AND configuration_digest=$2")
            .bind(operator).bind(digest).fetch_one(&mut *tx).await?;
        if saved != snapshot {
            return Err(StoreError::Conflict(
                "configuration digest already identifies another snapshot",
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    /// Read an existing creation receipt before checking a newly deployed registry.
    /// Authenticated caller scope and the entire original payload remain mandatory.
    pub async fn replay_session_creation(
        &self,
        operator: &str,
        key: &str,
        input: &NewSession,
    ) -> Result<Option<SessionRecord>, StoreError> {
        bounded(operator, 200, "invalid operator")?;
        bounded(key, 200, "invalid idempotency key")?;
        validate_new_session(input)?;
        let row=sqlx::query("SELECT k.payload_digest,s.* FROM session_creation_keys k JOIN research_sessions s ON s.session_id=k.session_id AND s.operator_id=k.operator_id WHERE k.operator_id=$1 AND k.idempotency_key=$2")
            .bind(operator).bind(key).fetch_optional(&self.pool).await?;
        match row {
            Some(row) => {
                if row.try_get::<String, _>("payload_digest")? != payload_digest(input)? {
                    return Err(StoreError::Conflict("idempotency payload changed"));
                }
                Ok(Some(session_record(&row)?))
            }
            None => Ok(None),
        }
    }

    pub async fn create_session(
        &self,
        operator: &str,
        key: &str,
        input: NewSession,
    ) -> Result<SessionRecord, StoreError> {
        bounded(operator, 200, "invalid operator")?;
        bounded(key, 200, "invalid idempotency key")?;
        validate_new_session(&input)?;
        let digest = payload_digest(&input)?;
        let mut tx = self.pool.begin().await?;
        // Database transaction lock serializes first inserts for an operator. Hash collisions
        // cause harmless extra contention, never authorization or idempotency collisions.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("session-create:{operator}"))
            .execute(&mut *tx)
            .await?;
        if let Some(row) = sqlx::query("SELECT payload_digest,session_id FROM session_creation_keys WHERE operator_id=$1 AND idempotency_key=$2")
            .bind(operator).bind(key).fetch_optional(&mut *tx).await? {
            if row.try_get::<String,_>("payload_digest")? != digest { return Err(StoreError::Conflict("idempotency payload changed")); }
            let id: String = row.try_get("session_id")?;
            let row = sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2")
                .bind(operator).bind(id).fetch_one(&mut *tx).await?;
            return session_record(&row);
        }
        let configured: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM configuration_snapshots WHERE operator_id=$1 AND configuration_digest=$2)")
            .bind(operator).bind(&input.configuration_digest).fetch_one(&mut *tx).await?;
        if !configured {
            return Err(StoreError::InvalidInput("configuration is not registered"));
        }
        let lifecycle = Session::new_research(parse_mode(&input.mode)?).map_err(control_error)?;
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query("INSERT INTO research_sessions(session_id,operator_id,network_id,mode,configuration_digest,experiment_id,strategy_ids,observed_state,lifecycle) VALUES($1,$2,$3,$4,$5,$6,$7,'RECOVERING',$8) RETURNING *")
            .bind(&id).bind(operator).bind(&input.network_id).bind(&input.mode).bind(&input.configuration_digest)
            .bind(&input.experiment_id).bind(serde_json::json!(input.strategy_ids)).bind(snapshot_value(&lifecycle)?).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO session_creation_keys(operator_id,idempotency_key,payload_digest,session_id) VALUES($1,$2,$3,$4)")
            .bind(operator).bind(key).bind(digest).bind(&id).execute(&mut *tx).await?;
        audit(
            &mut tx,
            &id,
            operator,
            "SESSION_CREATED",
            None,
            serde_json::json!({"mode":input.mode,"network_id":input.network_id}),
        )
        .await?;
        tx.commit().await?;
        session_record(&row)
    }

    pub async fn get_session(&self, operator: &str, id: &str) -> Result<SessionRecord, StoreError> {
        let row =
            sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2")
                .bind(operator)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        session_record(&row)
    }

    /// Internal worker lookup of the immutable, operator-scoped experiment binding.
    pub async fn get_session_experiment_id(
        &self,
        operator: &str,
        session_id: &str,
    ) -> Result<String, StoreError> {
        sqlx::query_scalar(
            "SELECT experiment_id FROM research_sessions WHERE operator_id=$1 AND session_id=$2",
        )
        .bind(operator)
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
    }

    /// Stable, bounded keyset pagination. New sessions may appear before an existing cursor.
    pub async fn list_sessions_page(
        &self,
        operator: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionPage, StoreError> {
        if !(1..=100).contains(&limit) {
            return Err(StoreError::InvalidInput("limit must be 1..100"));
        }
        if let Some(cursor) = cursor {
            Uuid::parse_str(cursor).map_err(|_| StoreError::InvalidInput("invalid cursor"))?;
        }
        let rows = sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND ($2::text IS NULL OR session_id>$2) ORDER BY session_id LIMIT $3")
            .bind(operator).bind(cursor).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
        let has_more = rows.len() > limit as usize;
        let items: Vec<_> = rows
            .iter()
            .take(limit as usize)
            .map(session_record)
            .collect::<Result<_, _>>()?;
        let next_cursor = if has_more {
            items.last().map(|s| s.session_id.clone())
        } else {
            None
        };
        Ok(SessionPage { items, next_cursor })
    }

    pub async fn issue_command(
        &self,
        operator: &str,
        id: &str,
        key: &str,
        input: NewCommand,
    ) -> Result<CommandReceipt, StoreError> {
        bounded(key, 200, "invalid idempotency key")?;
        let action = parse_action(&input.action)?;
        if input
            .reason
            .as_ref()
            .is_some_and(|v| v.chars().count() > 500)
        {
            return Err(StoreError::InvalidInput("reason too long"));
        }
        let revision = parse_revision(&input.expected_revision)?;
        let digest = payload_digest(&input)?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2 FOR UPDATE",
        )
        .bind(operator)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StoreError::NotFound)?;
        if let Some(existing) = sqlx::query("SELECT * FROM control_commands WHERE operator_id=$1 AND session_id=$2 AND idempotency_key=$3")
            .bind(operator).bind(id).bind(key).fetch_optional(&mut *tx).await? {
            if existing.try_get::<String,_>("payload_digest")? != digest { return Err(StoreError::Conflict("idempotency payload changed")); }
            return command_record(&existing);
        }
        let lifecycle = lifecycle(&row)?;
        let next = lifecycle.request(action, revision).map_err(control_error)?;
        if next.desired_revision() > i64::MAX as u64 {
            return Err(StoreError::Conflict("revision exhausted"));
        }
        if action == Action::Stop {
            let superseded:Vec<String>=sqlx::query_scalar("UPDATE control_commands SET status='SUPERSEDED' WHERE session_id=$1 AND status='PENDING' RETURNING command_id")
                .bind(id).fetch_all(&mut *tx).await?;
            for command_id in superseded {
                audit(&mut tx,id,operator,"COMMAND_SUPERSEDED",Some(&command_id),serde_json::json!({"superseding_revision":next.desired_revision().to_string(),"action":"STOP"})).await?;
            }
        }
        let command_id = Uuid::new_v4().to_string();
        let command = sqlx::query("INSERT INTO control_commands(command_id,operator_id,session_id,idempotency_key,payload_digest,action,revision,status,reason,outstanding_attempts) VALUES($1,$2,$3,$4,$5,$6,$7,'PENDING',$8,$9) RETURNING *")
            .bind(&command_id).bind(operator).bind(id).bind(key).bind(digest).bind(&input.action)
            .bind(next.desired_revision() as i64).bind(input.reason).bind(i64::from(next.outstanding())).fetch_one(&mut *tx).await?;
        persist_lifecycle(&mut tx, id, &next).await?;
        audit(&mut tx,id,operator,"COMMAND_ACCEPTED",Some(&command_id),serde_json::json!({"revision":next.desired_revision().to_string(),"action":input.action})).await?;
        tx.commit().await?;
        command_record(&command)
    }

    pub async fn get_command(
        &self,
        operator: &str,
        id: &str,
    ) -> Result<CommandReceipt, StoreError> {
        let row =
            sqlx::query("SELECT * FROM control_commands WHERE operator_id=$1 AND command_id=$2")
                .bind(operator)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        command_record(&row)
    }
}

fn bounded(value: &str, max: usize, message: &'static str) -> Result<(), StoreError> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err(StoreError::InvalidInput(message))
    } else {
        Ok(())
    }
}
fn validate_new_session(value: &NewSession) -> Result<(), StoreError> {
    parse_mode(&value.mode)?;
    if !matches!(value.network_id.as_str(), "base-mainnet" | "solana-mainnet") {
        return Err(StoreError::InvalidInput("unsupported network"));
    }
    bounded(
        &value.configuration_digest,
        200,
        "invalid configuration digest",
    )?;
    bounded(&value.experiment_id, 200, "invalid experiment id")?;
    if value.strategy_ids.is_empty() || value.strategy_ids.len() > 32 {
        return Err(StoreError::InvalidInput("strategy count must be 1..32"));
    }
    let mut unique = std::collections::HashSet::new();
    for strategy in &value.strategy_ids {
        bounded(strategy, 200, "invalid strategy id")?;
        if !unique.insert(strategy) {
            return Err(StoreError::InvalidInput("duplicate strategy"));
        }
    }
    Ok(())
}
fn parse_mode(v: &str) -> Result<Mode, StoreError> {
    match v {
        "OBSERVE" => Ok(Mode::Observe),
        "PAPER" => Ok(Mode::Paper),
        "REPLAY" => Ok(Mode::Replay),
        "LIVE" => Err(StoreError::CapabilityUnavailable),
        _ => Err(StoreError::InvalidInput("invalid mode")),
    }
}
fn parse_action(v: &str) -> Result<Action, StoreError> {
    match v {
        "START" => Ok(Action::Start),
        "PAUSE" => Ok(Action::Pause),
        "RESUME" => Ok(Action::Resume),
        "STOP" => Ok(Action::Stop),
        "DISARM" => Err(StoreError::CapabilityUnavailable),
        _ => Err(StoreError::InvalidInput("invalid action")),
    }
}
fn parse_revision(v: &str) -> Result<u64, StoreError> {
    if v.is_empty() || (v.len() > 1 && v.starts_with('0')) || !v.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(StoreError::InvalidInput("invalid revision"));
    }
    let n = v
        .parse::<u64>()
        .map_err(|_| StoreError::InvalidInput("revision out of range"))?;
    if n > i64::MAX as u64 {
        return Err(StoreError::InvalidInput("revision out of range"));
    }
    Ok(n)
}
fn payload_digest<T: serde::Serialize>(v: &T) -> Result<String, StoreError> {
    let bytes = serde_json::to_vec(v).map_err(|_| StoreError::CorruptState)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
fn control_error(error: arb_domain::ControlError) -> StoreError {
    match error {
        arb_domain::ControlError::CapabilityUnavailable => StoreError::CapabilityUnavailable,
        arb_domain::ControlError::RevisionConflict => StoreError::Conflict("stale revision"),
        arb_domain::ControlError::CommandPending => StoreError::Conflict("command pending"),
        arb_domain::ControlError::CounterOverflow => StoreError::Conflict("counter exhausted"),
        _ => StoreError::Conflict("invalid lifecycle transition"),
    }
}
fn lifecycle(row: &PgRow) -> Result<Session, StoreError> {
    let value: Value = row.try_get("lifecycle")?;
    let snapshot: SessionSnapshot =
        serde_json::from_value(value).map_err(|_| StoreError::CorruptState)?;
    let session = Session::restore_research(snapshot).map_err(|_| StoreError::CorruptState)?;
    if parse_mode(&row.try_get::<String, _>("mode")?)? != session.mode()
        || row.try_get::<String, _>("observed_state")? != state_name(&session)?
        || row.try_get::<i64, _>("desired_revision")? as u64 != session.desired_revision()
        || row.try_get::<i64, _>("applied_revision")? as u64 != session.applied_revision()
        || row.try_get::<i64, _>("outstanding_attempts")? as u64 != u64::from(session.outstanding())
        || row.try_get::<i64, _>("generation")? as u64 != session.generation()
        || row.try_get::<bool, _>("local_fence")? != session.local_fence_engaged()
    {
        return Err(StoreError::CorruptState);
    }
    Ok(session)
}
fn snapshot_value(session: &Session) -> Result<Value, StoreError> {
    serde_json::to_value(session.snapshot()).map_err(|_| StoreError::CorruptState)
}
fn state_name(session: &Session) -> Result<String, StoreError> {
    serde_json::to_value(session.state())
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(StoreError::CorruptState)
}
async fn persist_lifecycle(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    s: &Session,
) -> Result<(), StoreError> {
    let desired = i64::try_from(s.desired_revision()).map_err(|_| StoreError::CorruptState)?;
    let applied = i64::try_from(s.applied_revision()).map_err(|_| StoreError::CorruptState)?;
    let generation = i64::try_from(s.generation()).map_err(|_| StoreError::CorruptState)?;
    sqlx::query("UPDATE research_sessions SET lifecycle=$2,observed_state=$3,desired_revision=$4,applied_revision=$5,outstanding_attempts=$6,generation=$7,local_fence=$8 WHERE session_id=$1")
        .bind(id).bind(snapshot_value(s)?).bind(state_name(s)?).bind(desired).bind(applied).bind(i64::from(s.outstanding())).bind(generation).bind(s.local_fence_engaged()).execute(&mut **tx).await?;
    Ok(())
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    operator: &str,
    kind: &str,
    command: Option<&str>,
    detail: Value,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO control_audit_events(session_id,operator_id,event_kind,command_id,detail) VALUES($1,$2,$3,$4,$5)").bind(id).bind(operator).bind(kind).bind(command).bind(detail).execute(&mut **tx).await?;
    Ok(())
}
fn session_record(row: &PgRow) -> Result<SessionRecord, StoreError> {
    lifecycle(row)?;
    let last: Option<DateTime<Utc>> = row.try_get("last_heartbeat_at")?;
    let lease: Option<DateTime<Utc>> = row.try_get("lease_until")?;
    Ok(SessionRecord {
        session_id: row.try_get("session_id")?,
        network_id: row.try_get("network_id")?,
        mode: row.try_get("mode")?,
        observed_state: row.try_get("observed_state")?,
        health: if lease.is_some_and(|v| v > Utc::now()) {
            "DEGRADED"
        } else if last.is_some() {
            "UNREACHABLE"
        } else {
            "UNKNOWN"
        }
        .to_owned(),
        desired_revision: row.try_get::<i64, _>("desired_revision")?.to_string(),
        applied_revision: row.try_get::<i64, _>("applied_revision")?.to_string(),
        outstanding_attempts: u32::try_from(row.try_get::<i64, _>("outstanding_attempts")?)
            .map_err(|_| StoreError::CorruptState)?,
        execution_authorized: false,
        last_heartbeat_at: last.map(|v| v.to_rfc3339()),
        configuration_digest: row.try_get("configuration_digest")?,
    })
}
fn command_record(row: &PgRow) -> Result<CommandReceipt, StoreError> {
    Ok(CommandReceipt {
        command_id: row.try_get("command_id")?,
        session_id: row.try_get("session_id")?,
        revision: row.try_get::<i64, _>("revision")?.to_string(),
        status: row.try_get("status")?,
        action: row.try_get("action")?,
        accepted_at: row.try_get::<DateTime<Utc>, _>("accepted_at")?.to_rfc3339(),
        applied_at: row
            .try_get::<Option<DateTime<Utc>>, _>("applied_at")?
            .map(|v| v.to_rfc3339()),
        outstanding_attempts: u32::try_from(row.try_get::<i64, _>("outstanding_attempts")?)
            .map_err(|_| StoreError::CorruptState)?,
        fence_effective: row.try_get("fence_effective")?,
        signer_revocation_status: "NOT_APPLICABLE".to_owned(),
    })
}

#[cfg(test)]
mod digest_compatibility_tests {
    use super::*;

    #[test]
    fn payload_digest_retains_unprefixed_serialized_json_golden_values() {
        // Fixed Python hashlib values for the exact JSON bytes, not raw strings.
        for (value, expected) in [
            (
                serde_json::Value::Null,
                "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b",
            ),
            (
                serde_json::json!([]),
                "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            ),
            (
                serde_json::json!("abc"),
                "6cc43f858fbb763301637b5af970e2a46b46f461f27e5a0f41e009c59b827b25",
            ),
        ] {
            let actual = payload_digest(&value).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 64);
            assert!(!actual.starts_with("sha256:"));
        }
    }

    #[test]
    fn payload_digest_keeps_negative_unknown_and_full_width_amounts() {
        let unknown = serde_json::json!({"unknown": null, "net_minor": "-123"});
        assert_eq!(
            payload_digest(&unknown).unwrap(),
            "1b4bcfcf84b6bf1fa4f9930289c18efd488184b33e95eadef78d0ee75d974d9c"
        );
        let large = serde_json::json!({
            "network": "solana-mainnet", "amount": "340282366920938463463374607431768211455"
        });
        assert_eq!(
            payload_digest(&large).unwrap(),
            "b95a481822d51ba54b48c1dcbff732c69bc9f16989588c29bb191ba4a673fbf6"
        );
    }
}
