//! Authenticated read side and immutable virtual-capital initialization.
//! No reserve, resolve, settle, sign or send mutation is exposed here.
use super::*;
use arb_paper::AccountingAsset;
use arb_storage::{
    CollectionAttemptPage, CollectionCoverage, DecisionCoverage, DecisionGroupPage,
    DecisionTracePage, NewPaperRun, OpportunityFilter, OpportunityPage, PaperJournalPage,
    PaperReservationPage, PaperRunPage, PaperRunRecord, ResearchExport, StoredDecisionTrace,
};

#[derive(Clone, Serialize)]
pub struct PaperAssetChoice {
    pub network_id: String,
    pub asset: AccountingAsset,
}

#[async_trait]
pub(crate) trait ResearchStore: Send + Sync {
    async fn export_session(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<ResearchExport, StoreError>;
    async fn collection_coverage(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<CollectionCoverage, StoreError>;
    async fn collection_attempts(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CollectionAttemptPage, StoreError>;
    async fn list_decisions(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionTracePage, StoreError>;
    async fn get_decision(
        &self,
        operator: &str,
        observation: &str,
    ) -> Result<StoredDecisionTrace, StoreError>;
    async fn decision_groups(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionGroupPage, StoreError>;
    async fn decision_coverage(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<DecisionCoverage, StoreError>;
    async fn opportunities(
        &self,
        operator: &str,
        filter: OpportunityFilter,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<OpportunityPage, StoreError>;
    async fn replay_paper_creation(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: &NewPaperRun,
    ) -> Result<Option<PaperRunRecord>, StoreError>;
    async fn create_paper_run(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: NewPaperRun,
    ) -> Result<PaperRunRecord, StoreError>;
    async fn get_paper_run(&self, operator: &str, run: &str) -> Result<PaperRunRecord, StoreError>;
    async fn list_paper_runs(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperRunPage, StoreError>;
    async fn paper_journal(
        &self,
        operator: &str,
        run: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperJournalPage, StoreError>;
    async fn paper_reservations(
        &self,
        operator: &str,
        run: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperReservationPage, StoreError>;
}

#[async_trait]
impl ResearchStore for Store {
    async fn export_session(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<ResearchExport, StoreError> {
        Store::export_session(self, operator, session).await
    }
    async fn collection_coverage(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<CollectionCoverage, StoreError> {
        Store::collection_coverage(self, operator, session).await
    }
    async fn collection_attempts(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CollectionAttemptPage, StoreError> {
        Store::list_collection_attempts(self, operator, session, cursor, limit).await
    }
    async fn list_decisions(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionTracePage, StoreError> {
        Store::list_decision_traces(self, operator, session, cursor, limit).await
    }
    async fn get_decision(
        &self,
        operator: &str,
        observation: &str,
    ) -> Result<StoredDecisionTrace, StoreError> {
        Store::get_decision_trace(self, operator, observation).await
    }
    async fn decision_groups(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<DecisionGroupPage, StoreError> {
        Store::list_decision_groups(self, operator, session, cursor, limit).await
    }
    async fn decision_coverage(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<DecisionCoverage, StoreError> {
        Store::decision_coverage(self, operator, session).await
    }
    async fn opportunities(
        &self,
        operator: &str,
        filter: OpportunityFilter,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<OpportunityPage, StoreError> {
        Store::list_opportunities(self, operator, filter, cursor, limit).await
    }
    async fn replay_paper_creation(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: &NewPaperRun,
    ) -> Result<Option<PaperRunRecord>, StoreError> {
        Store::replay_paper_run_creation(self, operator, session, key, input).await
    }
    async fn create_paper_run(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: NewPaperRun,
    ) -> Result<PaperRunRecord, StoreError> {
        Store::create_paper_run(self, operator, session, key, input).await
    }
    async fn get_paper_run(&self, operator: &str, run: &str) -> Result<PaperRunRecord, StoreError> {
        Store::get_paper_run(self, operator, run).await
    }
    async fn list_paper_runs(
        &self,
        operator: &str,
        session: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperRunPage, StoreError> {
        Store::list_paper_runs(self, operator, session, cursor, limit).await
    }
    async fn paper_journal(
        &self,
        operator: &str,
        run: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperJournalPage, StoreError> {
        Store::list_paper_journal(self, operator, run, cursor, limit).await
    }
    async fn paper_reservations(
        &self,
        operator: &str,
        run: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaperReservationPage, StoreError> {
        Store::list_paper_reservations(self, operator, run, cursor, limit).await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyQuery {}

fn validate_session_path(session: &str, id: &RequestId) -> Result<(), ApiError> {
    if session.is_empty() || session.len() > 128 {
        return Err(ApiError::invalid(id));
    }
    Ok(())
}

pub(super) async fn export_session(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<EmptyQuery>, QueryRejection>,
) -> Result<Json<ResearchExport>, ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    query.map_err(|_| ApiError::invalid(&id))?;
    validate_session_path(&session, &id)?;
    // Heavy analytical reads may use at most two connections per API process.
    // Refuse immediately rather than queue ahead of lifecycle/control requests.
    let _permit = state.0.exports.try_acquire().map_err(|_| {
        ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "EXPORT_BUSY",
            "Two session exports are already running",
            &id,
        )
    })?;
    state
        .0
        .store
        .export_session("operator", &session)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}

pub(super) async fn collection_coverage(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<EmptyQuery>, QueryRejection>,
) -> Result<Json<CollectionCoverage>, ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    query.map_err(|_| ApiError::invalid(&id))?;
    validate_session_path(&session, &id)?;
    state
        .0
        .store
        .collection_coverage("operator", &session)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}

pub(super) async fn collection_attempts(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<Pagination>, QueryRejection>,
) -> Result<Json<CollectionAttemptPage>, ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    validate_session_path(&session, &id)?;
    let limit = validate_page(&query, &id)?;
    if let Some(cursor) = &query.cursor
        && (cursor.len() != 36
            || !cursor.bytes().enumerate().all(|(position, byte)| {
                if matches!(position, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                }
            }))
    {
        return Err(ApiError::invalid(&id));
    }
    state
        .0
        .store
        .collection_attempts("operator", &session, query.cursor.as_deref(), limit)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SessionQuery {
    session_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
}
fn session_page(query: &SessionQuery, id: &RequestId) -> Result<u32, ApiError> {
    if query.session_id.is_empty() || query.session_id.len() > 128 {
        return Err(ApiError::invalid(id));
    }
    validate_page(
        &Pagination {
            cursor: query.cursor.clone(),
            limit: query.limit,
        },
        id,
    )
}
pub(super) async fn list_decisions(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    query: Result<Query<SessionQuery>, QueryRejection>,
) -> Result<Json<DecisionTracePage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = session_page(&query, &id)?;
    state
        .0
        .store
        .list_decisions(
            "operator",
            &query.session_id,
            query.cursor.as_deref(),
            limit,
        )
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn get_decision(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<StoredDecisionTrace>, ApiError> {
    let Path(observation) = path.map_err(|_| ApiError::invalid(&id))?;
    state
        .0
        .store
        .get_decision("operator", &observation)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn decision_groups(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    query: Result<Query<SessionQuery>, QueryRejection>,
) -> Result<Json<DecisionGroupPage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = session_page(&query, &id)?;
    state
        .0
        .store
        .decision_groups(
            "operator",
            &query.session_id,
            query.cursor.as_deref(),
            limit,
        )
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CoverageQuery {
    session_id: String,
}
pub(super) async fn decision_coverage(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    query: Result<Query<CoverageQuery>, QueryRejection>,
) -> Result<Json<DecisionCoverage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    if query.session_id.is_empty() || query.session_id.len() > 128 {
        return Err(ApiError::invalid(&id));
    }
    state
        .0
        .store
        .decision_coverage("operator", &query.session_id)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn create_paper_run(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<NewPaperRun>, JsonRejection>,
) -> Result<(StatusCode, Json<PaperRunRecord>), ApiError> {
    let Path(session_id) = path.map_err(|_| ApiError::invalid(&id))?;
    let key = idempotency(&headers, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    // Replay durable acceptance before checking today's registration or lifecycle.
    if let Some(existing) = state
        .0
        .store
        .replay_paper_creation("operator", &session_id, &key, &input)
        .await
        .map_err(|error| ApiError::store(error, &id))?
    {
        return Ok((StatusCode::CREATED, Json(existing)));
    }
    let session = state
        .0
        .store
        .get_session("operator", &session_id)
        .await
        .map_err(|error| ApiError::store(error, &id))?;
    if session.mode != "PAPER" {
        return Err(ApiError::store(StoreError::CapabilityUnavailable, &id));
    }
    let config = state
        .0
        .config
        .configurations
        .iter()
        .find(|config| config.configuration_digest == session.configuration_digest)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "UNKNOWN_CONFIGURATION",
                "Configuration digest is not registered",
                &id,
            )
        })?;
    if config.mode != "PAPER"
        || !config.enabled_networks.contains(&session.network_id)
        || input.initial_balances.is_empty()
        || input.initial_balances.len() > 32
    {
        return Err(ApiError::invalid(&id));
    }
    let mut seen = std::collections::HashSet::new();
    for balance in &input.initial_balances {
        if !seen.insert(balance.asset.clone())
            || !config.paper_assets.iter().any(|choice| {
                choice.network_id == session.network_id && choice.asset == balance.asset
            })
        {
            return Err(ApiError::invalid(&id));
        }
    }
    // Storage atomically checks STOPPED, pending command status and frozen config.
    // A desired/applied revision difference alone does not mean a command is pending.
    state
        .0
        .store
        .create_paper_run("operator", &session_id, &key, input)
        .await
        .map(|run| (StatusCode::CREATED, Json(run)))
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn get_paper_run(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<PaperRunRecord>, ApiError> {
    let Path(run) = path.map_err(|_| ApiError::invalid(&id))?;
    state
        .0
        .store
        .get_paper_run("operator", &run)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn list_paper_runs(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<Pagination>, QueryRejection>,
) -> Result<Json<PaperRunPage>, ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = validate_page(&query, &id)?;
    state
        .0
        .store
        .list_paper_runs("operator", &session, query.cursor.as_deref(), limit)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn paper_journal(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<Pagination>, QueryRejection>,
) -> Result<Json<PaperJournalPage>, ApiError> {
    let Path(run) = path.map_err(|_| ApiError::invalid(&id))?;
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = validate_page(&query, &id)?;
    state
        .0
        .store
        .paper_journal("operator", &run, query.cursor.as_deref(), limit)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
pub(super) async fn paper_reservations(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<Pagination>, QueryRejection>,
) -> Result<Json<PaperReservationPage>, ApiError> {
    let Path(run) = path.map_err(|_| ApiError::invalid(&id))?;
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = validate_page(&query, &id)?;
    state
        .0
        .store
        .paper_reservations("operator", &run, query.cursor.as_deref(), limit)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
