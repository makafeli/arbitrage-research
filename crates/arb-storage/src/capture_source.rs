//! Explicit producer bindings. Neither a matching source nor a saved capture is a fill.
use super::*;
use arb_domain::DecisionCaptureRef;
use std::collections::HashSet;

/// The source is immutable per session; its complete registry/origin binding is checked.
#[derive(Clone, Debug)]
pub struct CaptureSourceBinding {
    pub stream_id: String,
    pub binding: IngestionBinding,
}

impl CaptureSourceBinding {
    pub fn validate(&self) -> Result<(), StoreError> {
        self.binding.validate()?;
        if self.stream_id.is_empty()
            || self.stream_id.len() > 128
            || !self
                .stream_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
        {
            return Err(StoreError::InvalidInput("invalid capture source identity"));
        }
        Ok(())
    }
}

/// Only named database-boundary failures are translated; unexpected database errors survive.
pub(crate) fn continuity_error(error: sqlx::Error) -> StoreError {
    match error.as_database_error().and_then(|e| e.code()).as_deref() {
        Some("22023") => StoreError::InvalidInput("invalid capture source reference"),
        Some("55000" | "P0002" | "55P03" | "40001" | "40P01") => {
            StoreError::Conflict("capture source continuity unavailable")
        }
        _ => error.into(),
    }
}

async fn check_source(
    tx: &mut Transaction<'_, Postgres>,
    operator: &str,
    expected: &CaptureSourceBinding,
) -> Result<(), StoreError> {
    let row = sqlx::query("SELECT binding,state FROM ingestion_streams WHERE operator_id=$1 AND stream_id=$2 FOR SHARE")
        .bind(operator).bind(&expected.stream_id).fetch_optional(&mut **tx).await.map_err(continuity_error)?
        .ok_or(StoreError::Conflict("capture source is missing"))?;
    let actual: IngestionBinding =
        serde_json::from_value(row.try_get("binding")?).map_err(|_| StoreError::CorruptState)?;
    actual.validate().map_err(|_| StoreError::CorruptState)?;
    if actual != expected.binding || row.try_get::<String, _>("state")? != "ACTIVE" {
        return Err(StoreError::Conflict(
            "capture source binding or state differs",
        ));
    }
    Ok(())
}

impl Store {
    /// Called after STOPPED recovery and before any capture request. A stored requirement
    /// cannot be silently removed or changed by a later process environment.
    pub async fn configure_capture_ingestion_source(
        &self,
        claim: &WorkerClaim,
        source: Option<&CaptureSourceBinding>,
    ) -> Result<(), StoreError> {
        if let Some(source) = source {
            source.validate()?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        let session = worker::locked_worker(&mut tx, claim).await?;
        if session.try_get::<String, _>("observed_state")? != "STOPPED"
            || lifecycle(&session)?.allows_evaluation()
        {
            return Err(StoreError::Conflict(
                "capture source configuration requires stopped recovery",
            ));
        }
        let existing: Option<String> = sqlx::query_scalar("SELECT stream_id FROM session_ingestion_sources WHERE operator_id=$1 AND session_id=$2")
            .bind(&claim.operator_id).bind(&claim.session_id).fetch_optional(&mut *tx).await?;
        match source {
            None if existing.is_some() => {
                return Err(StoreError::Conflict(
                    "configured capture source is required on restart",
                ));
            }
            None => {}
            Some(source) => {
                if claim.network_id != "base-mainnet"
                    || existing.as_ref().is_some_and(|s| s != &source.stream_id)
                {
                    return Err(StoreError::Conflict(
                        "capture source differs from immutable session",
                    ));
                }
                check_source(&mut tx, &claim.operator_id, source).await?;
                if existing.is_none() {
                    sqlx::query("INSERT INTO session_ingestion_sources(operator_id,session_id,stream_id) VALUES($1,$2,$3)")
                        .bind(&claim.operator_id).bind(&claim.session_id).bind(&source.stream_id)
                        .execute(&mut *tx).await.map_err(continuity_error)?;
                    audit(
                        &mut tx,
                        &claim.session_id,
                        &claim.operator_id,
                        "CAPTURE_SOURCE_CONFIGURED",
                        None,
                        json!({"stream_id":source.stream_id,"policy":"base-capture-continuity-v1"}),
                    )
                    .await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Register a fully verified producer batch atomically. Raw file/context verification
    /// belongs to the producer; this boundary checks immutable scope, admission and source.
    pub async fn bind_captures_to_ingestion(
        &self,
        claim: &WorkerClaim,
        generation: u64,
        source: &CaptureSourceBinding,
        checkpoint: &IngestionHead,
        captures: &[DecisionCaptureRef],
    ) -> Result<(), StoreError> {
        source.validate()?;
        if captures.is_empty()
            || captures.len() > arb_domain::MAX_PUBLICATION_CAPTURE_REFS
            || captures.len() != source.binding.pool_addresses.len()
        {
            return Err(StoreError::InvalidInput("incomplete capture source batch"));
        }
        let mut unique = HashSet::new();
        for capture in captures {
            let digest = capture.manifest_digest.strip_prefix("sha256:");
            if capture.capture_id.is_empty()
                || capture.capture_id.len() > 256
                || capture.capture_id.chars().any(char::is_control)
                || !unique.insert(&capture.capture_id)
                || capture.snapshot_id != capture.manifest_digest
                || !digest.is_some_and(|s| {
                    s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            {
                return Err(StoreError::InvalidInput(
                    "invalid capture source references",
                ));
            }
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        let session = worker::locked_worker(&mut tx, claim).await?;
        let state = lifecycle(&session)?;
        if !state.allows_evaluation()
            || state.generation() != generation
            || claim.network_id != "base-mainnet"
        {
            return Err(StoreError::Conflict("capture generation is fenced"));
        }
        let configured: Option<String> = sqlx::query_scalar("SELECT stream_id FROM session_ingestion_sources WHERE operator_id=$1 AND session_id=$2")
            .bind(&claim.operator_id).bind(&claim.session_id).fetch_optional(&mut *tx).await?;
        if configured.as_deref() != Some(source.stream_id.as_str()) {
            return Err(StoreError::Conflict(
                "capture source is not configured for this session",
            ));
        }
        check_source(&mut tx, &claim.operator_id, source).await?;
        let checkpoint = serde_json::to_value(checkpoint)
            .map_err(|_| StoreError::InvalidInput("invalid capture checkpoint"))?;
        let batches = sqlx::query("SELECT revision,payload_digest FROM ingestion_batches WHERE operator_id=$1 AND stream_id=$2 AND checkpoint=$3 ORDER BY revision LIMIT 2")
            .bind(&claim.operator_id).bind(&source.stream_id).bind(&checkpoint).fetch_all(&mut *tx).await?;
        if batches.len() != 1 {
            return Err(StoreError::Conflict(
                "captured checkpoint has no unique ingestion batch",
            ));
        }
        let revision: i64 = batches[0].try_get("revision")?;
        let digest: String = batches[0].try_get("payload_digest")?;
        let _: Value = sqlx::query_scalar("SELECT require_current_ingestion_snapshot($1,$2,$3,$4)")
            .bind(&claim.operator_id)
            .bind(&source.stream_id)
            .bind(revision)
            .bind(&digest)
            .fetch_one(&mut *tx)
            .await
            .map_err(continuity_error)?;
        let generation = i64::try_from(generation)
            .map_err(|_| StoreError::InvalidInput("invalid capture generation"))?;
        let mut inserted = 0usize;
        for capture in captures {
            let admitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM capture_admissions c JOIN research_attempts a ON a.attempt_id=c.attempt_id WHERE c.session_id=$1 AND c.capture_id=$2 AND c.manifest_digest=$3 AND c.generation=$4 AND a.admitted_worker_epoch=$5)")
                .bind(&claim.session_id).bind(&capture.capture_id).bind(&capture.manifest_digest).bind(generation).bind(claim.epoch)
                .fetch_one(&mut *tx).await?;
            if !admitted {
                return Err(StoreError::Conflict(
                    "capture source batch contains an unadmitted capture",
                ));
            }
            let existing = sqlx::query("SELECT manifest_digest,stream_id,snapshot_revision,snapshot_digest,checkpoint FROM capture_ingestion_dependencies WHERE operator_id=$1 AND session_id=$2 AND capture_id=$3")
                .bind(&claim.operator_id).bind(&claim.session_id).bind(&capture.capture_id).fetch_optional(&mut *tx).await?;
            if let Some(row) = existing {
                if row.try_get::<String, _>("manifest_digest")? != capture.manifest_digest
                    || row.try_get::<String, _>("stream_id")? != source.stream_id
                    || row.try_get::<i64, _>("snapshot_revision")? != revision
                    || row.try_get::<String, _>("snapshot_digest")? != digest
                    || row.try_get::<Value, _>("checkpoint")? != checkpoint
                {
                    return Err(StoreError::Conflict("capture source association changed"));
                }
                continue;
            }
            sqlx::query("INSERT INTO capture_ingestion_dependencies(operator_id,session_id,capture_id,manifest_digest,stream_id,snapshot_revision,snapshot_digest,checkpoint) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                .bind(&claim.operator_id).bind(&claim.session_id).bind(&capture.capture_id).bind(&capture.manifest_digest)
                .bind(&source.stream_id).bind(revision).bind(&digest).bind(&checkpoint)
                .execute(&mut *tx).await.map_err(continuity_error)?;
            inserted += 1;
        }
        if inserted > 0 {
            audit(&mut tx, &claim.session_id, &claim.operator_id, "CAPTURES_BOUND_TO_INGESTION", None,
                json!({"stream_id":source.stream_id,"revision":revision.to_string(),"snapshot_digest":digest,"captures":captures,"policy":"base-capture-continuity-v1"})).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
