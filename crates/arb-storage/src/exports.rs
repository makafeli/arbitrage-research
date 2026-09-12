//! Complete bounded database snapshots. Raw capture files and provider credentials
//! are deliberately outside this database read; their availability remains unknown.
use super::*;
use arb_paper::PortfolioReservation;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EXPORT_ROWS: u64 = 10_000;
pub const MAX_EXPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchExport {
    pub schema_version: String,
    pub export_id: String,
    pub exported_at: String,
    pub content_sha256: String,
    pub snapshot: ExportSnapshot,
    pub methodology: ExportMethodology,
    pub data: ExportData,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportSnapshot {
    pub isolation: String,
    pub scope: String,
    pub collection_completeness: String,
    pub source_counts: ExportSourceCounts,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportSourceCounts {
    pub decisions: String,
    pub paper_runs: String,
    pub paper_journal_events: String,
    pub capture_catalog_entries: String,
    pub collection_attempts: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportMethodology {
    pub amounts: String,
    pub asset_decimals: String,
    pub costs: String,
    pub configuration_snapshot: String,
    pub raw_artifacts: String,
    pub hash_format: String,
    pub journal_projection: String,
    pub execution_authorized: bool,
    pub max_source_rows: String,
    pub max_bytes: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub session: SessionRecord,
    pub experiment_id: String,
    pub strategy_ids: Vec<String>,
    pub decision_coverage: DecisionCoverage,
    pub decisions: Vec<StoredDecisionTrace>,
    pub paper_runs: Vec<ExportPaperRun>,
    pub capture_dependencies: Vec<ExportCaptureDependency>,
    pub collection_attempts: Vec<StoredCollectionAttempt>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportPaperRun {
    pub run: PaperRunRecord,
    pub journal: Vec<ExportPaperEvent>,
    pub reservations: Vec<PortfolioReservation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportPaperEvent {
    pub event_id: String,
    pub recorded_at: String,
    pub source_payload_sha256: String,
    pub event: arb_paper::JournalEvent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportCaptureDependency {
    pub capture_id: String,
    pub manifest_digest: String,
    pub snapshot_ids: Vec<String>,
    pub catalog_status: String,
    pub raw_artifact_status: String,
    pub expiration_status: String,
    pub generation: Option<String>,
    pub admitted_at: Option<String>,
}

impl Store {
    /// Freeze all reads before the first source count. Concurrent worker activity
    /// can neither change the selected journal revision nor make counts disagree.
    /// Refuse the whole export if any bound is exceeded; never silently truncate.
    pub async fn export_session(
        &self,
        operator: &str,
        session_id: &str,
    ) -> Result<ResearchExport, StoreError> {
        let (mut tx, session, exported_at) = begin_export(self, operator, session_id).await?;
        let result =
            export_in_snapshot(&mut tx, operator, session_id, &session, exported_at).await?;
        tx.commit().await?;
        Ok(result)
    }
}

async fn begin_export(
    store: &Store,
    operator: &str,
    session_id: &str,
) -> Result<(Transaction<'static, Postgres>, PgRow, DateTime<Utc>), StoreError> {
    let mut tx = store.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '8s'")
        .execute(&mut *tx)
        .await?;
    let session =
        sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2")
            .bind(operator)
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::NotFound)?;
    let exported_at = sqlx::query_scalar("SELECT transaction_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    Ok((tx, session, exported_at))
}

async fn export_in_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    operator: &str,
    session_id: &str,
    session: &PgRow,
    exported_at: DateTime<Utc>,
) -> Result<ResearchExport, StoreError> {
    // Count and size in SQL before loading any payload. These queries share the
    // same MVCC snapshot as every subsequent row and derived account projection.
    let count_row = sqlx::query(
        "SELECT (SELECT count(*) FROM decision_traces WHERE operator_id=$1 AND session_id=$2) AS decisions,
         (SELECT count(*) FROM paper_runs WHERE operator_id=$1 AND session_id=$2) AS paper_runs,
         (SELECT count(*) FROM paper_journal j JOIN paper_runs r USING(run_id) WHERE r.operator_id=$1 AND r.session_id=$2) AS paper_journal_events,
         (SELECT count(*) FROM capture_admissions c JOIN research_sessions s USING(session_id) WHERE s.operator_id=$1 AND c.session_id=$2) AS capture_catalog_entries,
         (SELECT count(*) FROM collection_attempts WHERE operator_id=$1 AND session_id=$2) AS collection_attempts,
         (SELECT coalesce(sum(octet_length(payload::text)),0)::bigint FROM decision_traces WHERE operator_id=$1 AND session_id=$2)
         +(SELECT coalesce(sum(octet_length(j.payload::text)),0)::bigint FROM paper_journal j JOIN paper_runs r USING(run_id) WHERE r.operator_id=$1 AND r.session_id=$2) AS payload_bytes"
    ).bind(operator).bind(session_id).fetch_one(&mut **tx).await?;
    let counts = ExportSourceCounts {
        decisions: export_count(&count_row, "decisions")?,
        paper_runs: export_count(&count_row, "paper_runs")?,
        paper_journal_events: export_count(&count_row, "paper_journal_events")?,
        capture_catalog_entries: export_count(&count_row, "capture_catalog_entries")?,
        collection_attempts: export_count(&count_row, "collection_attempts")?,
    };
    let total = [
        "decisions",
        "paper_runs",
        "paper_journal_events",
        "capture_catalog_entries",
        "collection_attempts",
    ]
    .into_iter()
    .try_fold(0_u64, |n, field| {
        let value = u64::try_from(count_row.try_get::<i64, _>(field)?)
            .map_err(|_| StoreError::CorruptState)?;
        n.checked_add(value).ok_or(StoreError::CorruptState)
    })?;
    if total > MAX_EXPORT_ROWS
        || count_row.try_get::<i64, _>("payload_bytes")? > MAX_EXPORT_BYTES as i64
    {
        return Err(StoreError::ExportLimitExceeded);
    }
    let rows = sqlx::query(
        "SELECT * FROM decision_traces WHERE operator_id=$1 AND session_id=$2 ORDER BY trace_id",
    )
    .bind(operator)
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;
    let decisions = rows
        .iter()
        .map(decisions::decision_record)
        .collect::<Result<Vec<_>, _>>()?;
    let mut capture_dependencies = BTreeMap::<(String, String), ExportCaptureDependency>::new();
    for record in &decisions {
        for capture in &record.trace.capture_refs {
            let dependency = capture_dependencies
                .entry((capture.capture_id.clone(), capture.manifest_digest.clone()))
                .or_insert_with(|| ExportCaptureDependency {
                    capture_id: capture.capture_id.clone(),
                    manifest_digest: capture.manifest_digest.clone(),
                    snapshot_ids: Vec::new(),
                    catalog_status: "MISSING".into(),
                    raw_artifact_status: "NOT_VERIFIED".into(),
                    expiration_status: "UNKNOWN".into(),
                    generation: None,
                    admitted_at: None,
                });
            if !dependency.snapshot_ids.contains(&capture.snapshot_id) {
                dependency.snapshot_ids.push(capture.snapshot_id.clone());
            }
        }
    }
    // Whitelist catalog metadata: artifact_path may contain provider credentials
    // or local filesystem details and is never selected or serialized.
    let captures = sqlx::query("SELECT c.capture_id,c.manifest_digest,c.generation,c.admitted_at FROM capture_admissions c JOIN research_sessions s USING(session_id) WHERE s.operator_id=$1 AND c.session_id=$2 ORDER BY c.capture_id")
        .bind(operator).bind(session_id).fetch_all(&mut **tx).await?;
    for row in &captures {
        let capture_id: String = row.try_get("capture_id")?;
        let manifest_digest: String = row.try_get("manifest_digest")?;
        let dependency = capture_dependencies
            .entry((capture_id.clone(), manifest_digest.clone()))
            .or_insert_with(|| ExportCaptureDependency {
                capture_id,
                manifest_digest,
                snapshot_ids: Vec::new(),
                catalog_status: "PRESENT".into(),
                raw_artifact_status: "NOT_VERIFIED".into(),
                expiration_status: "UNKNOWN".into(),
                generation: None,
                admitted_at: None,
            });
        dependency.catalog_status = "PRESENT".into();
        dependency.generation = Some(row.try_get::<i64, _>("generation")?.to_string());
        dependency.admitted_at = Some(row.try_get::<DateTime<Utc>, _>("admitted_at")?.to_rfc3339());
    }
    for dependency in capture_dependencies.values_mut() {
        dependency.snapshot_ids.sort();
    }
    let runs = sqlx::query(
        "SELECT * FROM paper_runs WHERE operator_id=$1 AND session_id=$2 ORDER BY run_id",
    )
    .bind(operator)
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut paper_runs = Vec::with_capacity(runs.len());
    let mut journal_count = 0_usize;
    for row in &runs {
        let journal_rows = sqlx::query("SELECT j.* FROM paper_journal j JOIN paper_runs r USING(run_id) WHERE r.operator_id=$1 AND r.session_id=$2 AND j.run_id=$3 ORDER BY j.sequence")
            .bind(operator).bind(session_id).bind(row.try_get::<String, _>("run_id")?).fetch_all(&mut **tx).await?;
        let restored = paper::restore_paper_rows(row, &journal_rows)?;
        let journal = journal_rows
            .iter()
            .map(paper::stored_paper_event)
            .collect::<Result<Vec<_>, _>>()?;
        journal_count += journal.len();
        let journal = journal
            .into_iter()
            .map(project_journal_event)
            .collect::<Result<Vec<_>, _>>()?;
        let mut reservations = restored.reservations();
        for reservation in &mut reservations {
            reservation.attempt_id = opaque_identifier(&reservation.attempt_id);
        }
        reservations.sort_by(|a, b| a.attempt_id.cmp(&b.attempt_id));
        paper_runs.push(ExportPaperRun {
            run: paper::project_paper(row, &restored)?,
            journal,
            reservations,
        });
    }
    let collection_attempts = sqlx::query("SELECT * FROM collection_attempts WHERE operator_id=$1 AND session_id=$2 ORDER BY attempt_id")
        .bind(operator).bind(session_id).fetch_all(&mut **tx).await?
        .iter().map(collection::collection_record).collect::<Result<Vec<_>, _>>()?;
    if counts.decisions != decisions.len().to_string()
        || counts.paper_runs != paper_runs.len().to_string()
        || counts.paper_journal_events != journal_count.to_string()
        || counts.capture_catalog_entries != captures.len().to_string()
        || counts.collection_attempts != collection_attempts.len().to_string()
    {
        return Err(StoreError::CorruptState);
    }
    validate_collection_links(tx, operator, session_id, &collection_attempts, &decisions).await?;
    let coverage = frozen_coverage(session_id, &decisions);
    let mut result = ResearchExport {
        schema_version: "1.0.0".into(),
        export_id: Uuid::new_v4().to_string(),
        exported_at: exported_at.to_rfc3339(),
        content_sha256: String::new(),
        snapshot: ExportSnapshot {
            isolation: "REPEATABLE_READ".into(),
            scope: "COMPLETE_STORED_SESSION".into(),
            collection_completeness: "UNKNOWN".into(),
            source_counts: counts,
        },
        methodology: ExportMethodology {
            amounts: "BASE_UNIT_INTEGER_STRINGS".into(),
            asset_decimals: "NOT_RETAINED_IN_DATABASE".into(),
            costs: "UNKNOWN_COSTS_REMAIN_NULL".into(),
            configuration_snapshot: "DIGEST_ONLY".into(),
            raw_artifacts: "REFERENCED_NOT_INCLUDED_OR_VERIFIED".into(),
            hash_format: "SHA256_SORTED_KEY_COMPACT_JSON_SNAPSHOT_METHODOLOGY_DATA_V1".into(),
            execution_authorized: false,
            journal_projection: "REDACTED_REPLAYABLE_ACCOUNTING_PROJECTION".into(),
            max_source_rows: MAX_EXPORT_ROWS.to_string(),
            max_bytes: MAX_EXPORT_BYTES.to_string(),
        },
        data: ExportData {
            session: session_record(session)?,
            experiment_id: session.try_get("experiment_id")?,
            strategy_ids: serde_json::from_value(session.try_get("strategy_ids")?)
                .map_err(|_| StoreError::CorruptState)?,
            decision_coverage: coverage,
            decisions,
            paper_runs,
            capture_dependencies: capture_dependencies.into_values().collect(),
            collection_attempts,
        },
    };
    result.content_sha256 = export_content_digest(&result)?;
    if serde_json::to_vec(&result)
        .map_err(|_| StoreError::CorruptState)?
        .len()
        > MAX_EXPORT_BYTES
    {
        return Err(StoreError::ExportLimitExceeded);
    }
    Ok(result)
}

async fn validate_collection_links(
    tx: &mut Transaction<'_, Postgres>,
    operator: &str,
    session_id: &str,
    attempts: &[StoredCollectionAttempt],
    decisions: &[StoredDecisionTrace],
) -> Result<(), StoreError> {
    let observations: BTreeMap<_, _> = decisions
        .iter()
        .map(|d| (d.trace.observation_id.as_str(), d))
        .collect();
    let mut expected = BTreeSet::new();
    let mut associated = BTreeSet::new();
    for attempt in attempts {
        for observation in &attempt.decision_observation_ids {
            let decision = observations
                .get(observation.as_str())
                .ok_or(StoreError::CorruptState)?;
            if decision.trace.session_id != attempt.session_id
                || decision.trace.generation.to_string() != attempt.generation
                || decision.trace.configuration_digest != attempt.configuration_digest
                || decision.trace.experiment_id != attempt.experiment_id
                || decision.trace.network_id.as_str() != attempt.network_id
                || !associated.insert(observation.as_str())
            {
                return Err(StoreError::CorruptState);
            }
            expected.insert((attempt.attempt_id.clone(), observation.clone()));
        }
    }
    // Bound even corrupt links pointing outside the exported source set. Normal
    // writes admit each trace to one same-session attempt in one transaction.
    let links=sqlx::query("SELECT a.attempt_id,d.observation_id,(d.operator_id=a.operator_id AND d.session_id=a.session_id) AS same_scope FROM collection_attempts a JOIN collection_decision_links l ON l.attempt_id=a.attempt_id JOIN decision_traces d ON d.trace_id=l.trace_id WHERE a.operator_id=$1 AND a.session_id=$2 ORDER BY a.attempt_id,d.trace_id LIMIT $3")
        .bind(operator).bind(session_id).bind(MAX_EXPORT_ROWS as i64+1).fetch_all(&mut **tx).await?;
    if links.len() > decisions.len() || links.len() != expected.len() {
        return Err(StoreError::CorruptState);
    }
    let mut actual = BTreeSet::new();
    for row in links {
        if !row.try_get::<bool, _>("same_scope")? {
            return Err(StoreError::CorruptState);
        }
        actual.insert((
            row.try_get::<String, _>("attempt_id")?,
            row.try_get::<String, _>("observation_id")?,
        ));
    }
    if actual != expected {
        return Err(StoreError::CorruptState);
    }
    Ok(())
}
fn opaque_identifier(value: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(value.as_bytes()))
}
fn project_journal_event(source: StoredPaperEvent) -> Result<ExportPaperEvent, StoreError> {
    // Freeform reasons and operator idempotency keys are not public research data.
    // Stable pseudonyms preserve cross-event accounting joins without leaking keys.
    let source_payload_sha256 = format!("sha256:{}", payload_digest(&source.event)?);
    let mut projected = serde_json::to_value(source.event).map_err(|_| StoreError::CorruptState)?;
    let command_id = projected["command_id"]
        .as_str()
        .ok_or(StoreError::CorruptState)?;
    projected["command_id"] = json!(opaque_identifier(command_id));
    let command = &mut projected["command"];
    match command["kind"].as_str().ok_or(StoreError::CorruptState)? {
        "INITIALIZE" => {}
        "RESERVE" => {
            let id = command["request"]["attempt_id"]
                .as_str()
                .ok_or(StoreError::CorruptState)?;
            command["request"]["attempt_id"] = json!(opaque_identifier(id));
        }
        "MARK_UNKNOWN" => {
            let id = command["attempt_id"]
                .as_str()
                .ok_or(StoreError::CorruptState)?;
            command["attempt_id"] = json!(opaque_identifier(id));
            command["reason"] = json!("REDACTED_FREEFORM_TEXT");
        }
        "RESOLVE" => {
            let id = command["attempt_id"]
                .as_str()
                .ok_or(StoreError::CorruptState)?;
            command["attempt_id"] = json!(opaque_identifier(id));
            if command["outcome"]["kind"] == "NOT_INCLUDED" {
                command["outcome"]["reason"] = json!("REDACTED_FREEFORM_TEXT");
            }
        }
        _ => return Err(StoreError::CorruptState),
    }
    Ok(ExportPaperEvent {
        event_id: source.event_id,
        recorded_at: source.recorded_at,
        source_payload_sha256,
        event: serde_json::from_value(projected).map_err(|_| StoreError::CorruptState)?,
    })
}
fn export_count(row: &PgRow, field: &str) -> Result<String, StoreError> {
    Ok(u64::try_from(row.try_get::<i64, _>(field)?)
        .map_err(|_| StoreError::CorruptState)?
        .to_string())
}
/// Hash the frozen content only. The unique request ID and request timestamp are
/// excluded; the same source snapshot has the same independently checkable hash.
pub fn export_content_digest(export: &ResearchExport) -> Result<String, StoreError> {
    let canonical =
        json!({"snapshot":export.snapshot,"methodology":export.methodology,"data":export.data});
    Ok(format!("sha256:{}", payload_digest(&canonical)?))
}
fn frozen_coverage(session_id: &str, records: &[StoredDecisionTrace]) -> DecisionCoverage {
    let count = |status| {
        records
            .iter()
            .filter(|r| r.trace.result.status() == status)
            .count()
            .to_string()
    };
    let unique: BTreeSet<_> = records
        .iter()
        .filter(|r| r.trace.result.status() == "QUOTED")
        .map(|r| &r.trace.grouping.key)
        .collect();
    DecisionCoverage {
        session_id: session_id.into(),
        raw_observations: records.len().to_string(),
        quoted_candidates: count("QUOTED"),
        rejected: count("REJECTED"),
        no_route: count("NO_ROUTE"),
        data_unavailable: count("DATA_UNAVAILABLE"),
        unique_opportunity_groups: unique.len().to_string(),
        eligible_attempts: None,
        reconciled_transactions: None,
        execution_accounting_available: false,
        collection_completeness: "UNKNOWN".into(),
        coverage_window_start_ms: records.iter().map(|r| r.trace.observed_at_unix_ms).min(),
        coverage_window_end_ms: records.iter().map(|r| r.trace.observed_at_unix_ms).max(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_domain::NetworkId;
    use arb_paper::{AccountingAsset, InitialBalance};

    #[test]
    fn committed_export_contract_fixture_matches_canonical_digest() {
        // The committed contract fixture hash is independently calculated in
        // Python. This checks both wire compatibility and canonical hashing.
        let export: ResearchExport =
            serde_json::from_str(include_str!("../../../specs/research-export.example.json"))
                .unwrap();
        assert_eq!(
            export_content_digest(&export).unwrap(),
            export.content_sha256
        );
    }

    #[tokio::test]
    async fn repeatable_read_counts_and_rows_exclude_writes_after_snapshot_start() {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is mandatory; export snapshot test must not skip");
        let store = Store::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        let operator = format!("snapshot-export-{}", Uuid::new_v4());
        let digest = format!("sha256:{}", "c".repeat(64));
        store.save_configuration(&operator,&digest,json!({"deployment":{"mode":"PAPER"},"networks":{"base":{"enabled":true,"verified_asset_ids":[]}}})).await.unwrap();
        let session = store
            .create_session(
                &operator,
                "session",
                NewSession {
                    network_id: "base-mainnet".into(),
                    mode: "PAPER".into(),
                    configuration_digest: digest,
                    experiment_id: "snapshot-test".into(),
                    strategy_ids: vec!["test".into()],
                },
            )
            .await
            .unwrap();
        let claim = store
            .claim_worker(
                &operator,
                &session.session_id,
                "base-mainnet",
                "snapshot-worker",
                60,
            )
            .await
            .unwrap();
        store.complete_worker_recovery(&claim).await.unwrap();
        let (mut tx, row, time) = begin_export(&store, &operator, &session.session_id)
            .await
            .unwrap();
        let isolation: String = sqlx::query_scalar("SHOW transaction_isolation")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let read_only: String = sqlx::query_scalar("SHOW transaction_read_only")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(isolation, "repeatable read");
        assert_eq!(read_only, "on");
        // A different pooled connection commits an initialized run after the
        // snapshot exists, before any export source count/payload query is made.
        let created = store
            .create_paper_run(
                &operator,
                &session.session_id,
                "after-snapshot",
                NewPaperRun {
                    initial_balances: vec![InitialBalance {
                        asset: AccountingAsset::Native(NetworkId::BaseMainnet),
                        amount: 99_u64.into(),
                    }],
                },
            )
            .await
            .unwrap();
        let frozen = export_in_snapshot(&mut tx, &operator, &session.session_id, &row, time)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(frozen.snapshot.source_counts.paper_runs, "0");
        assert_eq!(frozen.snapshot.source_counts.paper_journal_events, "0");
        assert!(frozen.data.paper_runs.is_empty());
        let later = store
            .export_session(&operator, &session.session_id)
            .await
            .unwrap();
        assert_eq!(later.snapshot.source_counts.paper_runs, "1");
        assert_eq!(later.snapshot.source_counts.paper_journal_events, "1");
        assert_eq!(later.data.paper_runs[0].run.run_id, created.run_id);
        assert_ne!(frozen.content_sha256, later.content_sha256);
    }
}
