//! Append-only manual assumptions applied to immutable candidate decisions.
use super::*;
use arb_domain::DecisionTrace;
use arb_paper::{CostAssessment, CostScenario, assess_cost_scenario};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewCostAssessment {
    pub observation_id: String,
    pub scenario: CostScenario,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredCostAssessment {
    pub record_id: String,
    pub recorded_at: String,
    pub assessment: CostAssessment,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CostAssessmentPage {
    pub items: Vec<StoredCostAssessment>,
    pub next_cursor: Option<String>,
}

impl Store {
    /// The request supplies assumptions only. Amounts, route, network, quote,
    /// configuration and evidence are derived from the authenticated stored source.
    pub async fn create_cost_assessment(
        &self,
        operator: &str,
        session_id: &str,
        key: &str,
        input: NewCostAssessment,
    ) -> Result<StoredCostAssessment, StoreError> {
        bounded(key, 128, "invalid idempotency key")?;
        validate_cost_observation(&input.observation_id)?;
        if serde_json::to_vec(&input)
            .map_err(|_| StoreError::CorruptState)?
            .len()
            > 16 * 1024
        {
            return Err(StoreError::InvalidInput("cost assumptions exceed 16 KiB"));
        }
        let request_digest = payload_digest(&input)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout = '8s'")
            .execute(&mut *tx)
            .await?;
        // Serializes exact retries without locking the worker's lifecycle row.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("cost-assessment:{operator}:{session_id}:{key}"))
            .execute(&mut *tx)
            .await?;
        let session =
            sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2")
                .bind(operator)
                .bind(session_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(StoreError::NotFound)?;
        if let Some(row) = sqlx::query("SELECT * FROM cost_assessments WHERE operator_id=$1 AND session_id=$2 AND idempotency_key=$3")
            .bind(operator).bind(session_id).bind(key).fetch_optional(&mut *tx).await? {
            if row.try_get::<String,_>("request_digest")? != request_digest {
                return Err(StoreError::Conflict("idempotency payload changed"));
            }
            return cost_record_in_tx(&mut tx, &row).await;
        }
        let source = sqlx::query("SELECT * FROM decision_traces WHERE operator_id=$1 AND session_id=$2 AND observation_id=$3")
            .bind(operator).bind(session_id).bind(&input.observation_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
        let decision = decisions::decision_record(&source)?;
        if decision.trace.configuration_digest
            != session.try_get::<String, _>("configuration_digest")?
            || decision.trace.experiment_id != session.try_get::<String, _>("experiment_id")?
            || decision.trace.network_id.as_str() != session.try_get::<String, _>("network_id")?
        {
            return Err(StoreError::CorruptState);
        }
        let assessment = assess_cost_scenario(&decision.trace, &input.scenario).map_err(|_| {
            StoreError::InvalidInput("invalid cost scenario or source is not a quoted candidate")
        })?;
        let row = sqlx::query("INSERT INTO cost_assessments(record_id,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,idempotency_key,request_digest,payload_digest,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING *")
            .bind(Uuid::now_v7().to_string()).bind(operator).bind(session_id).bind(&decision.trace_id)
            .bind(&input.observation_id).bind(&decision.trace.configuration_digest).bind(&decision.trace.experiment_id)
            .bind(decision.trace.network_id.as_str()).bind(key).bind(request_digest).bind(payload_digest(&assessment)?)
            .bind(serde_json::to_value(&assessment).map_err(|_| StoreError::CorruptState)?).fetch_one(&mut *tx).await?;
        let record = cost_record(&row, &decision)?;
        // Record identifiers only: operator keys and assumptions never enter audit text.
        audit(&mut tx, session_id, operator, "COST_ASSESSMENT_RECORDED", None,
            json!({"record_id":record.record_id,"observation_id":input.observation_id,"evidence":"CANDIDATE","assumptions":"MANUALLY_CONSTRUCTED","execution_authorized":false})).await?;
        tx.commit().await?;
        Ok(record)
    }

    pub async fn get_cost_assessment(
        &self,
        operator: &str,
        session_id: &str,
        record_id: &str,
    ) -> Result<StoredCostAssessment, StoreError> {
        validate_cost_page(Some(record_id), 1)?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM cost_assessments WHERE operator_id=$1 AND session_id=$2 AND record_id=$3")
            .bind(operator).bind(session_id).bind(record_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
        cost_record_in_tx(&mut tx, &row).await
    }

    pub async fn list_cost_assessments(
        &self,
        operator: &str,
        session_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CostAssessmentPage, StoreError> {
        validate_cost_page(cursor, limit)?;
        self.get_session(operator, session_id).await?;
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query("SELECT * FROM cost_assessments WHERE operator_id=$1 AND session_id=$2 AND ($3::text IS NULL OR record_id>$3) ORDER BY record_id LIMIT $4")
            .bind(operator).bind(session_id).bind(cursor).bind(i64::from(limit)+1).fetch_all(&mut *tx).await?;
        let more = rows.len() > limit as usize;
        let mut items = Vec::with_capacity(limit as usize);
        for row in rows.iter().take(limit as usize) {
            items.push(cost_record_in_tx(&mut tx, row).await?);
        }
        let next_cursor = if more {
            items.last().map(|v| v.record_id.clone())
        } else {
            None
        };
        Ok(CostAssessmentPage { items, next_cursor })
    }
}

pub fn validate_cost_observation(observation: &str) -> Result<(), StoreError> {
    if !observation.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err(StoreError::InvalidInput("invalid observation identity"));
    }
    Ok(())
}
fn validate_cost_page(cursor: Option<&str>, limit: u32) -> Result<(), StoreError> {
    if !(1..=100).contains(&limit) {
        return Err(StoreError::InvalidInput("limit must be 1..100"));
    }
    if let Some(cursor) = cursor {
        let parsed =
            Uuid::parse_str(cursor).map_err(|_| StoreError::InvalidInput("invalid cursor"))?;
        if parsed.to_string() != cursor {
            return Err(StoreError::InvalidInput("invalid cursor"));
        }
    }
    Ok(())
}
async fn cost_record_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    row: &PgRow,
) -> Result<StoredCostAssessment, StoreError> {
    let source = sqlx::query(
        "SELECT * FROM decision_traces WHERE operator_id=$1 AND session_id=$2 AND trace_id=$3",
    )
    .bind(row.try_get::<String, _>("operator_id")?)
    .bind(row.try_get::<String, _>("session_id")?)
    .bind(row.try_get::<String, _>("source_trace_id")?)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::CorruptState)?;
    cost_record(row, &decisions::decision_record(&source)?)
}
/// Used by frozen export with the already validated source from the same snapshot.
pub(super) fn cost_record(
    row: &PgRow,
    decision: &StoredDecisionTrace,
) -> Result<StoredCostAssessment, StoreError> {
    let assessment: CostAssessment =
        serde_json::from_value(row.try_get("payload")?).map_err(|_| StoreError::CorruptState)?;
    let trace: &DecisionTrace = &decision.trace;
    if decision.trace_id != row.try_get::<String, _>("source_trace_id")?
        || trace.observation_id != row.try_get::<String, _>("observation_id")?
        || trace.session_id != row.try_get::<String, _>("session_id")?
        || trace.configuration_digest != row.try_get::<String, _>("configuration_digest")?
        || trace.experiment_id != row.try_get::<String, _>("experiment_id")?
        || trace.network_id.as_str() != row.try_get::<String, _>("network_id")?
        || payload_digest(&assessment)? != row.try_get::<String, _>("payload_digest")?
    {
        return Err(StoreError::CorruptState);
    }
    let expected =
        assess_cost_scenario(trace, &assessment.scenario).map_err(|_| StoreError::CorruptState)?;
    if serde_json::to_value(&expected).map_err(|_| StoreError::CorruptState)?
        != serde_json::to_value(&assessment).map_err(|_| StoreError::CorruptState)?
        || payload_digest(&NewCostAssessment {
            observation_id: trace.observation_id.clone(),
            scenario: assessment.scenario.clone(),
        })? != row.try_get::<String, _>("request_digest")?
    {
        return Err(StoreError::CorruptState);
    }
    Ok(StoredCostAssessment {
        record_id: row.try_get("record_id")?,
        recorded_at: row.try_get::<DateTime<Utc>, _>("recorded_at")?.to_rfc3339(),
        assessment,
    })
}
