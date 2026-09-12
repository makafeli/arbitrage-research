//! Durable acquisition/evaluation telemetry, separate from execution and drain accounting.
use super::*;
use arb_domain::DecisionTrace;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectionPurpose {
    Readiness,
    Research,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectionOutcome {
    InProgress,
    ReadinessCompleted,
    DecisionsRecorded,
    AcquisitionFailed,
    EvaluationFailed,
    DeadlineExceeded,
    Suppressed,
    WorkerCancelled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectionReason {
    ProviderUnavailable,
    InputValidationFailed,
    CaptureStorageUnavailable,
    ResourceLimit,
    AcquisitionUnavailable,
    AcquisitionDeadline,
    EvaluationRejected,
    EvaluationDeadline,
    GenerationFenced,
    WorkerShutdown,
    TaskFailed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionFinish {
    pub outcome: CollectionOutcome,
    pub reason: Option<CollectionReason>,
    pub captured_pools: u32,
    pub elapsed_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredCollectionAttempt {
    pub attempt_id: String,
    pub session_id: String,
    pub network_id: String,
    pub configuration_digest: String,
    pub experiment_id: String,
    pub generation: String,
    pub worker_epoch: String,
    pub purpose: CollectionPurpose,
    pub outcome: CollectionOutcome,
    pub reason: Option<CollectionReason>,
    pub captured_pools: u32,
    pub decision_rows: String,
    pub decision_observation_ids: Vec<String>,
    pub elapsed_ms: Option<u64>,
    pub started_at: String,
    pub finished_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionAttemptPage {
    pub items: Vec<StoredCollectionAttempt>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionCoverage {
    pub session_id: String,
    pub attempts_started: String,
    pub readiness_attempts: String,
    pub research_attempts: String,
    pub in_progress: String,
    pub readiness_completed: String,
    pub decisions_recorded: String,
    pub acquisition_failed: String,
    pub evaluation_failed: String,
    pub deadline_exceeded: String,
    pub suppressed: String,
    pub worker_cancelled: String,
    pub decision_rows_recorded: String,
    pub denominator: String,
    pub collection_completeness: String,
    pub window_start_at: Option<String>,
    pub window_end_at: Option<String>,
}

impl Store {
    /// The caller must await this commit before beginning capture I/O. Retrying the same
    /// UUID and immutable scope returns its original row, including any terminal evidence.
    pub async fn begin_collection_attempt(
        &self,
        claim: &WorkerClaim,
        attempt_id: &str,
        generation: u64,
        purpose: CollectionPurpose,
    ) -> Result<StoredCollectionAttempt, StoreError> {
        validate_attempt_id(attempt_id)?;
        let generation = i64::try_from(generation)
            .map_err(|_| StoreError::InvalidInput("collection generation out of range"))?;
        let mut tx = self.pool.begin().await?;
        let session = worker::locked_worker(&mut tx, claim).await?;
        let configuration_digest: String = session.try_get("configuration_digest")?;
        let experiment_id: String = session.try_get("experiment_id")?;
        let digest = payload_digest(&(
            claim.operator_id.as_str(),
            claim.session_id.as_str(),
            claim.network_id.as_str(),
            claim.worker_id.as_str(),
            claim.epoch,
            generation,
            purpose,
            &configuration_digest,
            &experiment_id,
        ))?;
        if let Some(existing) = sqlx::query("SELECT * FROM collection_attempts WHERE attempt_id=$1")
            .bind(attempt_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            if existing.try_get::<String, _>("start_digest")? != digest {
                return Err(StoreError::Conflict("collection attempt identity changed"));
            }
            return collection_record(&existing);
        }
        let state = lifecycle(&session)?;
        if state.generation() != generation as u64
            || (purpose == CollectionPurpose::Research && !state.allows_evaluation())
        {
            return Err(StoreError::Conflict(
                "collection start generation is fenced",
            ));
        }
        if !matches!(state.mode(), Mode::Observe | Mode::Paper) {
            return Err(StoreError::CapabilityUnavailable);
        }
        let row = sqlx::query("INSERT INTO collection_attempts(attempt_id,operator_id,session_id,network_id,configuration_digest,experiment_id,generation,worker_epoch,worker_id,purpose,start_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING *")
            .bind(attempt_id).bind(&claim.operator_id).bind(&claim.session_id).bind(&claim.network_id)
            .bind(configuration_digest).bind(experiment_id)
            .bind(generation).bind(claim.epoch).bind(&claim.worker_id).bind(enum_name(purpose)?).bind(digest)
            .fetch_one(&mut *tx).await?;
        let result = collection_record(&row)?;
        tx.commit().await?;
        Ok(result)
    }

    /// Terminal failure/cancellation/readiness evidence may arrive after STOP or lease
    /// replacement, but only from the exact original claim. This never admits a decision,
    /// changes lifecycle state, refreshes a lease, or infers a timeout from missing evidence.
    pub async fn finish_collection_attempt(
        &self,
        claim: &WorkerClaim,
        attempt_id: &str,
        finish: CollectionFinish,
    ) -> Result<StoredCollectionAttempt, StoreError> {
        validate_finish(&finish, false)?;
        let mut tx = self.pool.begin().await?;
        let attempt = locked_collection(&mut tx, claim, attempt_id).await?;
        validate_purpose(&attempt, &finish)?;
        let result = finish_in_tx(&mut tx, &attempt, &finish, &[]).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Decision admission and successful terminal evidence commit atomically. Normal
    /// lease/generation/capture admission checks still apply; retries with changed data
    /// conflict. If the commit never happened, an earlier start remains IN_PROGRESS.
    pub async fn append_collection_decision_traces(
        &self,
        claim: &WorkerClaim,
        attempt_id: &str,
        generation: u64,
        traces: &[DecisionTrace],
        finish: CollectionFinish,
    ) -> Result<Vec<StoredDecisionTrace>, StoreError> {
        validate_finish(&finish, true)?;
        let mut tx = self.pool.begin().await?;
        // Keep lock order session -> collection identical to starts and control operations.
        worker::locked_worker(&mut tx, claim).await?;
        let attempt = locked_collection(&mut tx, claim, attempt_id).await?;
        validate_purpose(&attempt, &finish)?;
        if attempt.try_get::<i64, _>("generation")? as u64 != generation {
            return Err(StoreError::Conflict(
                "collection decision generation changed",
            ));
        }
        let ids: Vec<_> = traces.iter().map(|t| t.observation_id.clone()).collect();
        if ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
            return Err(StoreError::InvalidInput(
                "duplicate collection decision identity",
            ));
        }
        // Validate terminal identity before admitting anything, including a changed retry.
        let digest = payload_digest(&(&finish, &ids))?;
        if let Some(saved) = attempt.try_get::<Option<String>, _>("finish_digest")?
            && saved != digest
        {
            return Err(StoreError::Conflict("collection terminal evidence changed"));
        }
        let decisions = self
            .append_decision_traces_in_tx(&mut tx, claim, generation, traces)
            .await?;
        for decision in &decisions {
            sqlx::query("INSERT INTO collection_decision_links(attempt_id,trace_id) VALUES($1,$2) ON CONFLICT(trace_id) DO NOTHING")
                .bind(attempt_id).bind(&decision.trace_id).execute(&mut *tx).await?;
            let owner: String = sqlx::query_scalar(
                "SELECT attempt_id FROM collection_decision_links WHERE trace_id=$1",
            )
            .bind(&decision.trace_id)
            .fetch_one(&mut *tx)
            .await?;
            if owner != attempt_id {
                return Err(StoreError::Conflict(
                    "decision already belongs to another collection attempt",
                ));
            }
        }
        finish_in_tx(&mut tx, &attempt, &finish, &ids).await?;
        tx.commit().await?;
        Ok(decisions)
    }

    pub async fn list_collection_attempts(
        &self,
        operator: &str,
        session_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CollectionAttemptPage, StoreError> {
        if !(1..=100).contains(&limit) {
            return Err(StoreError::InvalidInput("limit must be 1..100"));
        }
        if let Some(cursor) = cursor {
            validate_attempt_id(cursor)?;
        }
        self.get_session(operator, session_id).await?;
        let rows = sqlx::query("SELECT * FROM collection_attempts WHERE operator_id=$1 AND session_id=$2 AND ($3::text IS NULL OR attempt_id>$3) ORDER BY attempt_id LIMIT $4")
            .bind(operator).bind(session_id).bind(cursor).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
        let more = rows.len() > limit as usize;
        let items = rows
            .iter()
            .take(limit as usize)
            .map(collection_record)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = more.then(|| {
            items
                .last()
                .expect("nonempty bounded page")
                .attempt_id
                .clone()
        });
        Ok(CollectionAttemptPage { items, next_cursor })
    }

    pub async fn collection_coverage(
        &self,
        operator: &str,
        session_id: &str,
    ) -> Result<CollectionCoverage, StoreError> {
        self.get_session(operator, session_id).await?;
        let mut tx = self.pool.begin().await?;
        let result = collection_coverage_in_tx(&mut tx, operator, session_id).await?;
        tx.commit().await?;
        Ok(result)
    }
}

pub(super) async fn collection_coverage_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    operator: &str,
    session_id: &str,
) -> Result<CollectionCoverage, StoreError> {
    let row = sqlx::query("SELECT count(*) AS attempts_started,count(*) FILTER(WHERE purpose='READINESS') AS readiness_attempts,count(*) FILTER(WHERE purpose='RESEARCH') AS research_attempts,count(*) FILTER(WHERE outcome='IN_PROGRESS') AS in_progress,count(*) FILTER(WHERE outcome='READINESS_COMPLETED') AS readiness_completed,count(*) FILTER(WHERE outcome='DECISIONS_RECORDED') AS decisions_recorded,count(*) FILTER(WHERE outcome='ACQUISITION_FAILED') AS acquisition_failed,count(*) FILTER(WHERE outcome='EVALUATION_FAILED') AS evaluation_failed,count(*) FILTER(WHERE outcome='DEADLINE_EXCEEDED') AS deadline_exceeded,count(*) FILTER(WHERE outcome='SUPPRESSED') AS suppressed,count(*) FILTER(WHERE outcome='WORKER_CANCELLED') AS worker_cancelled,coalesce(sum(decision_rows),0)::bigint AS decision_rows_recorded,min(started_at) AS window_start_at,max(coalesce(finished_at,started_at)) AS window_end_at FROM collection_attempts WHERE operator_id=$1 AND session_id=$2")
        .bind(operator).bind(session_id).fetch_one(&mut **tx).await?;
    let count =
        |key: &str| -> Result<String, StoreError> { Ok(row.try_get::<i64, _>(key)?.to_string()) };
    Ok(CollectionCoverage {
        session_id: session_id.into(),
        attempts_started: count("attempts_started")?,
        readiness_attempts: count("readiness_attempts")?,
        research_attempts: count("research_attempts")?,
        in_progress: count("in_progress")?,
        readiness_completed: count("readiness_completed")?,
        decisions_recorded: count("decisions_recorded")?,
        acquisition_failed: count("acquisition_failed")?,
        evaluation_failed: count("evaluation_failed")?,
        deadline_exceeded: count("deadline_exceeded")?,
        suppressed: count("suppressed")?,
        worker_cancelled: count("worker_cancelled")?,
        decision_rows_recorded: count("decision_rows_recorded")?,
        denominator: "RECORDED_COLLECTION_ATTEMPTS".into(),
        collection_completeness: "UNKNOWN".into(),
        window_start_at: row
            .try_get::<Option<DateTime<Utc>>, _>("window_start_at")?
            .map(|v| v.to_rfc3339()),
        window_end_at: row
            .try_get::<Option<DateTime<Utc>>, _>("window_end_at")?
            .map(|v| v.to_rfc3339()),
    })
}

async fn locked_collection(
    tx: &mut Transaction<'_, Postgres>,
    claim: &WorkerClaim,
    attempt_id: &str,
) -> Result<PgRow, StoreError> {
    validate_attempt_id(attempt_id)?;
    sqlx::query("SELECT * FROM collection_attempts WHERE attempt_id=$1 AND operator_id=$2 AND session_id=$3 AND network_id=$4 AND worker_id=$5 AND worker_epoch=$6 FOR UPDATE")
        .bind(attempt_id).bind(&claim.operator_id).bind(&claim.session_id).bind(&claim.network_id).bind(&claim.worker_id).bind(claim.epoch)
        .fetch_optional(&mut **tx).await?.ok_or(StoreError::NotFound)
}
fn validate_attempt_id(id: &str) -> Result<(), StoreError> {
    let parsed = Uuid::parse_str(id)
        .map_err(|_| StoreError::InvalidInput("invalid collection attempt UUID"))?;
    if parsed.to_string() != id {
        return Err(StoreError::InvalidInput(
            "noncanonical collection attempt UUID",
        ));
    }
    Ok(())
}
fn validate_finish(finish: &CollectionFinish, decisions: bool) -> Result<(), StoreError> {
    use CollectionOutcome as O;
    use CollectionReason as R;
    if finish.captured_pools > 8 || finish.elapsed_ms > 86_400_000 {
        return Err(StoreError::InvalidInput(
            "collection terminal bounds exceeded",
        ));
    }
    let valid = match (finish.outcome, finish.reason) {
        (O::ReadinessCompleted, None) => !decisions,
        (O::DecisionsRecorded, None) => decisions,
        (
            O::AcquisitionFailed,
            Some(
                R::ProviderUnavailable
                | R::InputValidationFailed
                | R::CaptureStorageUnavailable
                | R::ResourceLimit
                | R::AcquisitionUnavailable
                | R::TaskFailed,
            ),
        ) => !decisions,
        (O::EvaluationFailed, Some(R::EvaluationRejected | R::ResourceLimit | R::TaskFailed)) => {
            !decisions
        }
        (O::DeadlineExceeded, Some(R::AcquisitionDeadline | R::EvaluationDeadline)) => !decisions,
        (O::Suppressed, Some(R::GenerationFenced)) => !decisions,
        (O::WorkerCancelled, Some(R::WorkerShutdown)) => !decisions,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(StoreError::InvalidInput(
            "invalid collection terminal outcome and reason",
        ))
    }
}
fn validate_purpose(row: &PgRow, finish: &CollectionFinish) -> Result<(), StoreError> {
    let purpose: CollectionPurpose = parse_enum(row.try_get("purpose")?)?;
    if (finish.outcome == CollectionOutcome::ReadinessCompleted
        && purpose != CollectionPurpose::Readiness)
        || (matches!(
            finish.outcome,
            CollectionOutcome::DecisionsRecorded | CollectionOutcome::EvaluationFailed
        ) && purpose != CollectionPurpose::Research)
        || (finish.reason == Some(CollectionReason::EvaluationDeadline)
            && purpose != CollectionPurpose::Research)
    {
        return Err(StoreError::Conflict(
            "collection outcome differs from attempt purpose",
        ));
    }
    Ok(())
}
async fn finish_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    row: &PgRow,
    finish: &CollectionFinish,
    ids: &[String],
) -> Result<StoredCollectionAttempt, StoreError> {
    let digest = payload_digest(&(finish, ids))?;
    if let Some(saved) = row.try_get::<Option<String>, _>("finish_digest")? {
        if saved != digest {
            return Err(StoreError::Conflict("collection terminal evidence changed"));
        }
        return collection_record(row);
    }
    let updated = sqlx::query("UPDATE collection_attempts SET outcome=$2,reason=$3,captured_pools=$4,decision_rows=$5,elapsed_ms=$6,finish_digest=$7,decision_observation_ids=$8,finished_at=clock_timestamp() WHERE attempt_id=$1 RETURNING *")
        .bind(row.try_get::<String,_>("attempt_id")?).bind(enum_name(finish.outcome)?).bind(finish.reason.map(enum_name).transpose()?)
        .bind(finish.captured_pools as i32).bind(ids.len() as i64).bind(finish.elapsed_ms as i64).bind(digest).bind(json!(ids))
        .fetch_one(&mut **tx).await?;
    collection_record(&updated)
}
fn enum_name<T: Serialize>(value: T) -> Result<String, StoreError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(StoreError::CorruptState)
}
fn parse_enum<T: serde::de::DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_value(json!(value)).map_err(|_| StoreError::CorruptState)
}
pub(super) fn collection_record(row: &PgRow) -> Result<StoredCollectionAttempt, StoreError> {
    let attempt_id: String = row.try_get("attempt_id")?;
    validate_attempt_id(&attempt_id).map_err(|_| StoreError::CorruptState)?;
    let purpose: CollectionPurpose = parse_enum(row.try_get("purpose")?)?;
    let expected_start = payload_digest(&(
        row.try_get::<String, _>("operator_id")?,
        row.try_get::<String, _>("session_id")?,
        row.try_get::<String, _>("network_id")?,
        row.try_get::<String, _>("worker_id")?,
        row.try_get::<i64, _>("worker_epoch")?,
        row.try_get::<i64, _>("generation")?,
        purpose,
        row.try_get::<String, _>("configuration_digest")?,
        row.try_get::<String, _>("experiment_id")?,
    ))?;
    if row.try_get::<String, _>("start_digest")? != expected_start {
        return Err(StoreError::CorruptState);
    }
    let outcome = parse_enum(row.try_get("outcome")?)?;
    let reason = row
        .try_get::<Option<String>, _>("reason")?
        .map(parse_enum)
        .transpose()?;
    let captured_pools = u32::try_from(row.try_get::<i32, _>("captured_pools")?)
        .map_err(|_| StoreError::CorruptState)?;
    let elapsed_ms = row
        .try_get::<Option<i64>, _>("elapsed_ms")?
        .map(u64::try_from)
        .transpose()
        .map_err(|_| StoreError::CorruptState)?;
    let ids: Vec<String> = serde_json::from_value(row.try_get("decision_observation_ids")?)
        .map_err(|_| StoreError::CorruptState)?;
    if ids.len() > 64
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
        || ids.iter().any(|id| {
            !id.strip_prefix("sha256:").is_some_and(|h| {
                h.len() == 64
                    && h.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        })
        || (outcome == CollectionOutcome::DecisionsRecorded && ids.is_empty())
        || (outcome != CollectionOutcome::DecisionsRecorded && !ids.is_empty())
    {
        return Err(StoreError::CorruptState);
    }
    if outcome == CollectionOutcome::InProgress {
        if elapsed_ms.is_some()
            || captured_pools != 0
            || reason.is_some()
            || !ids.is_empty()
            || row.try_get::<i64, _>("decision_rows")? != 0
            || row.try_get::<Option<String>, _>("finish_digest")?.is_some()
            || row
                .try_get::<Option<DateTime<Utc>>, _>("finished_at")?
                .is_some()
        {
            return Err(StoreError::CorruptState);
        }
    } else if elapsed_ms.is_none()
        || row
            .try_get::<Option<DateTime<Utc>>, _>("finished_at")?
            .is_none()
    {
        return Err(StoreError::CorruptState);
    }
    if let Some(elapsed_ms) = elapsed_ms {
        let finish = CollectionFinish {
            outcome,
            reason,
            captured_pools,
            elapsed_ms,
        };
        validate_finish(&finish, outcome == CollectionOutcome::DecisionsRecorded)
            .map_err(|_| StoreError::CorruptState)?;
        validate_purpose(row, &finish).map_err(|_| StoreError::CorruptState)?;
        if row.try_get::<Option<String>, _>("finish_digest")?
            != Some(payload_digest(&(&finish, &ids))?)
            || ids.len() as i64 != row.try_get::<i64, _>("decision_rows")?
        {
            return Err(StoreError::CorruptState);
        }
    }
    Ok(StoredCollectionAttempt {
        attempt_id: row.try_get("attempt_id")?,
        session_id: row.try_get("session_id")?,
        network_id: row.try_get("network_id")?,
        configuration_digest: row.try_get("configuration_digest")?,
        experiment_id: row.try_get("experiment_id")?,
        generation: row.try_get::<i64, _>("generation")?.to_string(),
        worker_epoch: row.try_get::<i64, _>("worker_epoch")?.to_string(),
        purpose: parse_enum(row.try_get("purpose")?)?,
        outcome,
        reason,
        captured_pools,
        decision_rows: row.try_get::<i64, _>("decision_rows")?.to_string(),
        decision_observation_ids: ids,
        elapsed_ms,
        started_at: row.try_get::<DateTime<Utc>, _>("started_at")?.to_rfc3339(),
        finished_at: row
            .try_get::<Option<DateTime<Utc>>, _>("finished_at")?
            .map(|v| v.to_rfc3339()),
    })
}
