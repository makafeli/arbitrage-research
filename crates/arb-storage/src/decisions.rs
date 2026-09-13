use super::*;
use arb_domain::{
    DatasetOrigin, DecisionTrace, Evidence, NetworkId, OpportunityRecord, SourceKind,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredDecisionTrace {
    pub trace_id: String,
    pub recorded_at: String,
    pub trace: DecisionTrace,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionTracePage {
    pub items: Vec<StoredDecisionTrace>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionGroup {
    pub grouping_version: String,
    pub grouping_key: String,
    pub window_start_ms: u64,
    pub dataset_origin: DatasetOrigin,
    pub source_kind: SourceKind,
    pub raw_observations: String,
    pub quoted_candidates: String,
    pub rejected: String,
    pub no_route: String,
    pub data_unavailable: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionGroupPage {
    pub items: Vec<DecisionGroup>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionCoverage {
    pub session_id: String,
    pub raw_observations: String,
    pub quoted_candidates: String,
    pub rejected: String,
    pub no_route: String,
    pub data_unavailable: String,
    pub unique_opportunity_groups: String,
    pub eligible_attempts: Option<String>,
    pub reconciled_transactions: Option<String>,
    pub execution_accounting_available: bool,
    pub collection_completeness: String,
    pub coverage_window_start_ms: Option<u64>,
    pub coverage_window_end_ms: Option<u64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpportunityFilter {
    pub session_id: Option<String>,
    pub network_id: Option<NetworkId>,
    pub evidence_label: Option<Evidence>,
    pub source_kind: Option<SourceKind>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityPage {
    pub items: Vec<OpportunityRecord>,
    pub next_cursor: Option<String>,
}

impl Store {
    /// Bounded append-only observation batch. Content-addressed identities deduplicate
    /// exact retries; grouping is reporting metadata and never removes raw decisions.
    pub async fn append_decision_traces(
        &self,
        claim: &WorkerClaim,
        generation: u64,
        traces: &[DecisionTrace],
    ) -> Result<Vec<StoredDecisionTrace>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let result = self
            .append_decision_traces_in_tx(&mut tx, claim, generation, traces)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub(super) async fn append_decision_traces_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        claim: &WorkerClaim,
        generation: u64,
        traces: &[DecisionTrace],
    ) -> Result<Vec<StoredDecisionTrace>, StoreError> {
        if traces.is_empty() || traces.len() > 64 {
            return Err(StoreError::InvalidInput(
                "decision batch must contain 1..64 traces",
            ));
        }
        if serde_json::to_vec(traces)
            .map_err(|_| StoreError::InvalidInput("invalid decision batch"))?
            .len()
            > 262144
        {
            return Err(StoreError::InvalidInput("decision batch exceeds 256 KiB"));
        }
        let session = worker::locked_worker(tx, claim).await?;
        let state = lifecycle(&session)?;
        if !state.allows_evaluation() || state.generation() != generation {
            return Err(StoreError::Conflict("decision generation is fenced"));
        }
        let config: String = session.try_get("configuration_digest")?;
        let experiment: String = session.try_get("experiment_id")?;
        let strategies: Value = session.try_get("strategy_ids")?;
        let network_key = match claim.network_id.as_str() {
            "base-mainnet" => "base",
            "solana-mainnet" => "solana",
            _ => return Err(StoreError::CorruptState),
        };
        let policy: Option<Value> = sqlx::query_scalar("SELECT snapshot #> ARRAY['networks',$3,'chain_freshness'] FROM configuration_snapshots WHERE operator_id=$1 AND configuration_digest=$2")
            .bind(&claim.operator_id).bind(&config).bind(network_key).fetch_one(&mut **tx).await?;
        let mut result = Vec::with_capacity(traces.len());
        for trace in traces {
            if trace.session_id != claim.session_id
                || trace.generation != generation
                || trace.configuration_digest != config
                || trace.experiment_id != experiment
                || trace.network_id.as_str() != claim.network_id
                || trace.mode != state.mode()
                || !strategies.as_array().is_some_and(|a| {
                    a.iter()
                        .any(|v| v.as_str() == Some(trace.strategy_id.as_str()))
                })
            {
                return Err(StoreError::Conflict(
                    "decision scope differs from immutable session",
                ));
            }
            validate_frozen_policy(trace, policy.as_ref()).map_err(|_| {
                StoreError::InvalidInput(
                    "decision chain policy differs from immutable configuration",
                )
            })?;
            let digest = payload_digest(trace)?;
            if let Some(existing) = sqlx::query(
                "SELECT *,(SELECT c.snapshot #> ARRAY['networks',CASE decision_traces.payload->>'network_id' WHEN 'base-mainnet' THEN 'base' WHEN 'solana-mainnet' THEN 'solana' END,'chain_freshness'] FROM configuration_snapshots c WHERE c.operator_id=decision_traces.operator_id AND c.configuration_digest=decision_traces.configuration_digest) AS configuration_chain_freshness,(SELECT s.network_id FROM research_sessions s WHERE s.session_id=decision_traces.session_id AND s.operator_id=decision_traces.operator_id) AS session_network_id FROM decision_traces WHERE session_id=$1 AND observation_id=$2",
            )
            .bind(&claim.session_id)
            .bind(&trace.observation_id)
            .fetch_optional(&mut **tx)
            .await?
            {
                if existing.try_get::<String, _>("payload_digest")? != digest {
                    return Err(StoreError::Conflict("decision identity payload changed"));
                }
                result.push(decision_record(&existing)?);
                continue;
            }
            trace
                .validate()
                .map_err(|_| StoreError::InvalidInput("invalid sealed decision trace"))?;
            for capture in &trace.capture_refs {
                let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM capture_admissions c JOIN research_attempts a ON a.attempt_id=c.attempt_id WHERE c.session_id=$1 AND c.capture_id=$2 AND c.manifest_digest=$3 AND c.generation=$4 AND a.admitted_worker_epoch=$5)").bind(&claim.session_id).bind(&capture.capture_id).bind(&capture.manifest_digest).bind(i64::try_from(generation).map_err(|_|StoreError::CorruptState)?).bind(claim.epoch).fetch_one(&mut **tx).await?;
                if !exists {
                    return Err(StoreError::Conflict(
                        "decision capture is not admitted in this generation",
                    ));
                }
            }
            let row=sqlx::query("INSERT INTO decision_traces(trace_id,operator_id,session_id,observation_id,payload_digest,configuration_digest,generation,observed_at_unix_ms,result_status,grouping_version,grouping_key,window_start_ms,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING *,(SELECT c.snapshot #> ARRAY['networks',CASE decision_traces.payload->>'network_id' WHEN 'base-mainnet' THEN 'base' WHEN 'solana-mainnet' THEN 'solana' END,'chain_freshness'] FROM configuration_snapshots c WHERE c.operator_id=decision_traces.operator_id AND c.configuration_digest=decision_traces.configuration_digest) AS configuration_chain_freshness,(SELECT s.network_id FROM research_sessions s WHERE s.session_id=decision_traces.session_id AND s.operator_id=decision_traces.operator_id) AS session_network_id").bind(Uuid::now_v7().to_string()).bind(&claim.operator_id).bind(&claim.session_id).bind(&trace.observation_id).bind(digest).bind(&trace.configuration_digest).bind(i64::try_from(generation).map_err(|_|StoreError::CorruptState)?).bind(i64::try_from(trace.observed_at_unix_ms).map_err(|_|StoreError::InvalidInput("decision timestamp out of range"))?).bind(trace.result.status()).bind(&trace.grouping.version).bind(&trace.grouping.key).bind(i64::try_from(trace.grouping.window_start_ms).map_err(|_|StoreError::InvalidInput("group timestamp out of range"))?).bind(serde_json::to_value(trace).map_err(|_|StoreError::CorruptState)?).fetch_one(&mut **tx).await?;
            result.push(decision_record(&row)?);
        }
        audit(tx,&claim.session_id,&claim.operator_id,"DECISIONS_RECORDED",None,json!({"observations":result.iter().map(|r|&r.trace.observation_id).collect::<Vec<_>>(),"generation":generation.to_string(),"evidence":"CANDIDATE_OR_REJECTION_ONLY"})).await?;
        Ok(result)
    }
    pub async fn get_decision_trace(
        &self,
        operator: &str,
        observation_id: &str,
    ) -> Result<StoredDecisionTrace, StoreError> {
        let row =
            sqlx::query("SELECT *,(SELECT c.snapshot #> ARRAY['networks',CASE decision_traces.payload->>'network_id' WHEN 'base-mainnet' THEN 'base' WHEN 'solana-mainnet' THEN 'solana' END,'chain_freshness'] FROM configuration_snapshots c WHERE c.operator_id=decision_traces.operator_id AND c.configuration_digest=decision_traces.configuration_digest) AS configuration_chain_freshness,(SELECT s.network_id FROM research_sessions s WHERE s.session_id=decision_traces.session_id AND s.operator_id=decision_traces.operator_id) AS session_network_id FROM decision_traces WHERE operator_id=$1 AND observation_id=$2")
                .bind(operator)
                .bind(observation_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        decision_record(&row)
    }
    pub async fn list_decision_traces(
        &self,
        operator: &str,
        session_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionTracePage, StoreError> {
        validate_trace_page(cursor, limit)?;
        self.get_session(operator, session_id).await?;
        let rows=sqlx::query("SELECT *,(SELECT c.snapshot #> ARRAY['networks',CASE decision_traces.payload->>'network_id' WHEN 'base-mainnet' THEN 'base' WHEN 'solana-mainnet' THEN 'solana' END,'chain_freshness'] FROM configuration_snapshots c WHERE c.operator_id=decision_traces.operator_id AND c.configuration_digest=decision_traces.configuration_digest) AS configuration_chain_freshness,(SELECT s.network_id FROM research_sessions s WHERE s.session_id=decision_traces.session_id AND s.operator_id=decision_traces.operator_id) AS session_network_id FROM decision_traces WHERE operator_id=$1 AND session_id=$2 AND ($3::text IS NULL OR trace_id>$3) ORDER BY trace_id LIMIT $4").bind(operator).bind(session_id).bind(cursor).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
        let more = rows.len() > limit as usize;
        let items: Vec<_> = rows
            .iter()
            .take(limit as usize)
            .map(decision_record)
            .collect::<Result<_, _>>()?;
        let next_cursor = if more {
            items.last().map(|r| r.trace_id.clone())
        } else {
            None
        };
        Ok(DecisionTracePage { items, next_cursor })
    }
    pub async fn list_decision_groups(
        &self,
        operator: &str,
        session_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionGroupPage, StoreError> {
        if !(1..=100).contains(&limit) {
            return Err(StoreError::InvalidInput("limit must be 1..100"));
        }
        if let Some(c) = cursor
            && !canonical_digest(c)
        {
            return Err(StoreError::InvalidInput("invalid group cursor"));
        }
        self.get_session(operator, session_id).await?;
        let rows=sqlx::query("SELECT grouping_version,grouping_key,window_start_ms,payload->>'dataset_origin' AS origin,payload->>'source_kind' AS source,count(*) AS raw,count(*) FILTER(WHERE result_status='QUOTED') AS quoted,count(*) FILTER(WHERE result_status='REJECTED') AS rejected,count(*) FILTER(WHERE result_status='NO_ROUTE') AS no_route,count(*) FILTER(WHERE result_status='DATA_UNAVAILABLE') AS unavailable FROM decision_traces WHERE operator_id=$1 AND session_id=$2 AND ($3::text IS NULL OR grouping_key>$3) GROUP BY grouping_version,grouping_key,window_start_ms,payload->>'dataset_origin',payload->>'source_kind' ORDER BY grouping_key LIMIT $4").bind(operator).bind(session_id).bind(cursor).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
        let more = rows.len() > limit as usize;
        let items: Vec<_> = rows
            .iter()
            .take(limit as usize)
            .map(|r| {
                Ok(DecisionGroup {
                    grouping_version: r.try_get("grouping_version")?,
                    grouping_key: r.try_get("grouping_key")?,
                    window_start_ms: r.try_get::<i64, _>("window_start_ms")? as u64,
                    dataset_origin: serde_json::from_value(json!(
                        r.try_get::<String, _>("origin")?
                    ))
                    .map_err(|_| StoreError::CorruptState)?,
                    source_kind: serde_json::from_value(json!(r.try_get::<String, _>("source")?))
                        .map_err(|_| StoreError::CorruptState)?,
                    raw_observations: count(r, "raw")?,
                    quoted_candidates: count(r, "quoted")?,
                    rejected: count(r, "rejected")?,
                    no_route: count(r, "no_route")?,
                    data_unavailable: count(r, "unavailable")?,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        let next_cursor = if more {
            items.last().map(|r| r.grouping_key.clone())
        } else {
            None
        };
        Ok(DecisionGroupPage { items, next_cursor })
    }
    pub async fn decision_coverage(
        &self,
        operator: &str,
        session_id: &str,
    ) -> Result<DecisionCoverage, StoreError> {
        self.get_session(operator, session_id).await?;
        let row=sqlx::query("SELECT count(*) AS raw,count(*) FILTER(WHERE result_status='QUOTED') AS quoted,count(*) FILTER(WHERE result_status='REJECTED') AS rejected,count(*) FILTER(WHERE result_status='NO_ROUTE') AS no_route,count(*) FILTER(WHERE result_status='DATA_UNAVAILABLE') AS unavailable,count(DISTINCT grouping_key) FILTER(WHERE result_status='QUOTED') AS unique_groups,min(observed_at_unix_ms) AS window_start,max(observed_at_unix_ms) AS window_end FROM decision_traces WHERE operator_id=$1 AND session_id=$2").bind(operator).bind(session_id).fetch_one(&self.pool).await?;
        Ok(DecisionCoverage {
            session_id: session_id.into(),
            raw_observations: count(&row, "raw")?,
            quoted_candidates: count(&row, "quoted")?,
            rejected: count(&row, "rejected")?,
            no_route: count(&row, "no_route")?,
            data_unavailable: count(&row, "unavailable")?,
            unique_opportunity_groups: count(&row, "unique_groups")?,
            eligible_attempts: None,
            reconciled_transactions: None,
            execution_accounting_available: false,
            collection_completeness: "UNKNOWN".into(),
            coverage_window_start_ms: row
                .try_get::<Option<i64>, _>("window_start")?
                .map(|v| v as u64),
            coverage_window_end_ms: row
                .try_get::<Option<i64>, _>("window_end")?
                .map(|v| v as u64),
        })
    }
    pub async fn list_opportunities(
        &self,
        operator: &str,
        filter: OpportunityFilter,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<OpportunityPage, StoreError> {
        validate_trace_page(cursor, limit)?;
        if let Some(session) = &filter.session_id {
            self.get_session(operator, session).await?;
        }
        let evidence = filter.evidence_label.and_then(|e| {
            serde_json::to_value(e)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
        });
        let source = filter.source_kind.and_then(|s| {
            serde_json::to_value(s)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
        });
        // Apply eligibility/source filters BEFORE LIMIT, preserving complete page counts.
        let rows=sqlx::query("SELECT *,(SELECT c.snapshot #> ARRAY['networks',CASE decision_traces.payload->>'network_id' WHEN 'base-mainnet' THEN 'base' WHEN 'solana-mainnet' THEN 'solana' END,'chain_freshness'] FROM configuration_snapshots c WHERE c.operator_id=decision_traces.operator_id AND c.configuration_digest=decision_traces.configuration_digest) AS configuration_chain_freshness,(SELECT s.network_id FROM research_sessions s WHERE s.session_id=decision_traces.session_id AND s.operator_id=decision_traces.operator_id) AS session_network_id FROM decision_traces WHERE operator_id=$1 AND result_status='QUOTED' AND ($2::text IS NULL OR session_id=$2) AND ($3::text IS NULL OR payload->>'network_id'=$3) AND ($4::text IS NULL OR $4='CANDIDATE') AND ($5::text IS NULL OR payload->>'source_kind'=$5) AND ($6::text IS NULL OR trace_id>$6) ORDER BY trace_id LIMIT $7").bind(operator).bind(filter.session_id).bind(filter.network_id.map(|n|n.as_str())).bind(evidence).bind(source).bind(cursor).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
        let more = rows.len() > limit as usize;
        let mut items = Vec::new();
        let mut last = None;
        for row in rows.iter().take(limit as usize) {
            let record = decision_record(row)?;
            items.push(
                record
                    .trace
                    .to_opportunity()
                    .map_err(|_| StoreError::CorruptState)?
                    .ok_or(StoreError::CorruptState)?,
            );
            last = Some(record.trace_id);
        }
        Ok(OpportunityPage {
            items,
            next_cursor: if more { last } else { None },
        })
    }
}
fn canonical_digest(v: &str) -> bool {
    v.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn validate_trace_page(cursor: Option<&str>, limit: u32) -> Result<(), StoreError> {
    if !(1..=100).contains(&limit) {
        return Err(StoreError::InvalidInput("limit must be 1..100"));
    }
    if let Some(c) = cursor {
        Uuid::parse_str(c).map_err(|_| StoreError::InvalidInput("invalid cursor"))?;
    }
    Ok(())
}
fn count(row: &PgRow, key: &str) -> Result<String, StoreError> {
    Ok(row.try_get::<i64, _>(key)?.to_string())
}
pub(super) fn decision_record(row: &PgRow) -> Result<StoredDecisionTrace, StoreError> {
    let trace: DecisionTrace =
        serde_json::from_value(row.try_get("payload")?).map_err(|_| StoreError::CorruptState)?;
    trace.validate().map_err(|_| StoreError::CorruptState)?;
    let policy: Option<Value> = row.try_get("configuration_chain_freshness")?;
    validate_frozen_policy(&trace, policy.as_ref())?;
    if trace.network_id.as_str() != row.try_get::<String, _>("session_network_id")?
        || payload_digest(&trace)? != row.try_get::<String, _>("payload_digest")?
        || trace.observation_id != row.try_get::<String, _>("observation_id")?
        || trace.session_id != row.try_get::<String, _>("session_id")?
        || trace.configuration_digest != row.try_get::<String, _>("configuration_digest")?
        || trace.generation != row.try_get::<i64, _>("generation")? as u64
        || trace.observed_at_unix_ms != row.try_get::<i64, _>("observed_at_unix_ms")? as u64
        || trace.result.status() != row.try_get::<String, _>("result_status")?
        || trace.grouping.key != row.try_get::<String, _>("grouping_key")?
        || trace.grouping.version != row.try_get::<String, _>("grouping_version")?
        || trace.grouping.window_start_ms != row.try_get::<i64, _>("window_start_ms")? as u64
    {
        return Err(StoreError::CorruptState);
    }
    Ok(StoredDecisionTrace {
        trace_id: row.try_get("trace_id")?,
        recorded_at: row.try_get::<DateTime<Utc>, _>("recorded_at")?.to_rfc3339(),
        trace,
    })
}

/// The immutable snapshot binds optional freshness policy independently of the
/// observation hash. Re-sealing a report cannot change or remove this policy.
fn validate_frozen_policy(trace: &DecisionTrace, policy: Option<&Value>) -> Result<(), StoreError> {
    let policy = policy
        .filter(|value| !value.is_null())
        .map(|value| serde_json::from_value::<arb_domain::ChainFreshnessPolicy>(value.clone()))
        .transpose()
        .map_err(|_| StoreError::CorruptState)?;
    if let Some(policy) = &policy {
        policy.validate().map_err(|_| StoreError::CorruptState)?;
    }
    match (policy.as_ref(), trace.chain_freshness.as_ref()) {
        (None, None) => Ok(()),
        (Some(policy), Some(report)) if policy == &report.policy => Ok(()),
        _ => Err(StoreError::CorruptState),
    }
}
