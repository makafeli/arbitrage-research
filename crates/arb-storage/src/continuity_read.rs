//! Read-only continuity diagnostics, never a transaction-bound execution gate.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionContinuity {
    pub schema_version: String,
    pub assessment_kind: String,
    pub authorizes_execution: bool,
    pub trace_id: String,
    pub session_id: String,
    pub observation_id: String,
    pub network_id: String,
    pub checked_at: String,
    pub policy_version: String,
    pub continuity_status: String,
    pub capture_count: String,
    pub bound_count: String,
    pub invalidation_reasons: Vec<String>,
}

impl Store {
    /// One MVCC statement reads the current projection and matching invalidations.
    /// Historical decision bytes and their recorded timestamps are not rewritten.
    /// The result is diagnostic metadata, not a substitute for the locked gate.
    pub async fn decision_continuity(
        &self,
        operator: &str,
        session: &str,
        observation: &str,
    ) -> Result<DecisionContinuity, StoreError> {
        bounded(operator, 128, "invalid operator")?;
        bounded(session, 128, "invalid session")?;
        bounded(observation, 128, "invalid observation")?;
        let row = sqlx::query(
            r#"SELECT v.*, s.network_id, statement_timestamp() AS checked_at,
            (SELECT COALESCE(jsonb_agg(reasons.reason ORDER BY reasons.reason),'[]'::jsonb)
             FROM (
              SELECT DISTINCT i.reason
              FROM decision_traces d
              CROSS JOIN LATERAL jsonb_array_elements(CASE
               WHEN jsonb_typeof(d.payload->'capture_refs')='array'
               THEN d.payload->'capture_refs' ELSE '[]'::jsonb END) r
              JOIN capture_ingestion_dependencies c
               ON c.operator_id=d.operator_id AND c.session_id=d.session_id
               AND c.capture_id=r->>'capture_id'
               AND c.manifest_digest=r->>'manifest_digest'
               AND c.manifest_digest=r->>'snapshot_id'
              JOIN ingestion_invalidations i
               ON i.operator_id=c.operator_id AND i.stream_id=c.stream_id
              WHERE d.trace_id=v.trace_id AND d.operator_id=v.operator_id
             ) reasons) AS invalidation_reasons
            FROM decision_ingestion_validity v
            JOIN research_sessions s
             ON s.operator_id=v.operator_id AND s.session_id=v.session_id
            WHERE v.operator_id=$1 AND v.session_id=$2 AND v.observation_id=$3"#,
        )
        .bind(operator)
        .bind(session)
        .bind(observation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let capture_count: i64 = row.try_get("capture_count")?;
        let bound_count: i64 = row.try_get("bound_count")?;
        let network: String = row.try_get("network_id")?;
        let status: String = row.try_get("continuity_status")?;
        let policy: String = row.try_get("policy_version")?;
        let reasons: Vec<String> = serde_json::from_value(row.try_get("invalidation_reasons")?)
            .map_err(|_| StoreError::CorruptState)?;
        if !(0..=64).contains(&capture_count)
            || !(0..=capture_count).contains(&bound_count)
            || !matches!(network.as_str(), "base-mainnet" | "solana-mainnet")
            || policy != "base-capture-continuity-v1"
            || !matches!(status.as_str(), "NO_KNOWN_INVALIDATION" | "INVALIDATED" | "UNTRACKED" | "UNVERIFIABLE")
            || reasons.len() > 4
            || reasons.windows(2).any(|pair| pair[0] >= pair[1])
            || reasons.iter().any(|reason| !matches!(reason.as_str(), "CONTINUITY_LOST" | "PROVIDER_FAILURE" | "RESOURCE_LIMIT" | "INVALID_INPUT"))
            || (status == "NO_KNOWN_INVALIDATION" && (network != "base-mainnet" || capture_count == 0 || bound_count != capture_count || !reasons.is_empty()))
            || (status == "INVALIDATED" && (network != "base-mainnet" || bound_count == 0 || reasons.is_empty()))
            || (status == "UNTRACKED" && (network != "base-mainnet" || (capture_count > 0 && bound_count == capture_count) || !reasons.is_empty()))
        {
            return Err(StoreError::CorruptState);
        }
        Ok(DecisionContinuity {
            schema_version: "1.0.0".into(),
            assessment_kind: "CONTINUITY_ONLY".into(),
            authorizes_execution: false,
            trace_id: row.try_get("trace_id")?,
            session_id: row.try_get("session_id")?,
            observation_id: row.try_get("observation_id")?,
            network_id: network,
            checked_at: row.try_get::<DateTime<Utc>, _>("checked_at")?
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            policy_version: policy,
            continuity_status: status,
            capture_count: capture_count.to_string(),
            bound_count: bound_count.to_string(),
            invalidation_reasons: reasons,
        })
    }
}
