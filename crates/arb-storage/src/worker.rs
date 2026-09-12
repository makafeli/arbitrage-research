use super::*;

/// Capability for a cooperating research worker. This is not a live signing permit.
#[derive(Clone, Debug)]
pub struct WorkerClaim {
    pub(crate) operator_id: String,
    pub(crate) session_id: String,
    pub(crate) worker_id: String,
    pub(crate) epoch: i64,
    pub(crate) network_id: String,
    pub(crate) lease_seconds: u32,
}

impl WorkerClaim {
    pub fn epoch(&self) -> i64 {
        self.epoch
    }
    pub fn lease_seconds(&self) -> u32 {
        self.lease_seconds
    }
}

#[derive(Clone, Debug)]
pub struct WorkerUpdate {
    pub session: SessionRecord,
    pub generation: u64,
    pub gate_open: bool,
    pub command: Option<CommandReceipt>,
    pub attempt_id: Option<String>,
}

impl Store {
    /// A replacement can claim only an expired cooperating RESEARCH lease. Every claim
    /// rejects unacknowledged old commands and boots fenced into RECOVERING.
    pub async fn claim_worker(
        &self,
        operator: &str,
        id: &str,
        network: &str,
        worker: &str,
        lease_seconds: u32,
    ) -> Result<WorkerClaim, StoreError> {
        bounded(worker, 200, "invalid worker id")?;
        if !(1..=60).contains(&lease_seconds) {
            return Err(StoreError::InvalidInput("lease must be 1..60 seconds"));
        }
        let mut tx = self.pool.begin().await?;
        let row=sqlx::query("SELECT *, lease_until > clock_timestamp() AS lease_active FROM research_sessions WHERE operator_id=$1 AND session_id=$2 FOR UPDATE")
            .bind(operator).bind(id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
        if row.try_get::<String, _>("network_id")? != network {
            return Err(StoreError::Conflict("worker network mismatch"));
        }
        if row
            .try_get::<Option<bool>, _>("lease_active")?
            .unwrap_or(false)
        {
            return Err(StoreError::Conflict("worker lease is active"));
        }
        let next = lifecycle(&row)?.restart_research().map_err(control_error)?;
        let epoch = row
            .try_get::<i64, _>("worker_epoch")?
            .checked_add(1)
            .ok_or(StoreError::Conflict("worker epoch exhausted"))?;
        let rejected:Vec<String>=sqlx::query_scalar("UPDATE control_commands SET status='REJECTED' WHERE session_id=$1 AND status='PENDING' RETURNING command_id").bind(id).fetch_all(&mut *tx).await?;
        for command_id in rejected {
            audit(
                &mut tx,
                id,
                operator,
                "COMMAND_REJECTED",
                Some(&command_id),
                serde_json::json!({"reason":"worker restart","replacement_epoch":epoch}),
            )
            .await?;
        }
        persist_lifecycle(&mut tx, id, &next).await?;
        sqlx::query("UPDATE research_sessions SET worker_id=$2,worker_epoch=$3,lease_until=clock_timestamp()+make_interval(secs=>$4),last_heartbeat_at=clock_timestamp() WHERE session_id=$1")
            .bind(id).bind(worker).bind(epoch).bind(f64::from(lease_seconds)).execute(&mut *tx).await?;
        audit(
            &mut tx,
            id,
            operator,
            "WORKER_RECOVERING",
            None,
            serde_json::json!({"worker_id":worker,"epoch":epoch,"generation":next.generation()}),
        )
        .await?;
        tx.commit().await?;
        Ok(WorkerClaim {
            operator_id: operator.to_owned(),
            session_id: id.to_owned(),
            worker_id: worker.to_owned(),
            epoch,
            network_id: network.to_owned(),
            lease_seconds,
        })
    }

    pub async fn complete_worker_recovery(
        &self,
        claim: &WorkerClaim,
    ) -> Result<WorkerUpdate, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let current = lifecycle(&row)?;
        let outstanding: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM research_attempts WHERE session_id=$1 AND status='OUTSTANDING'",
        )
        .bind(&claim.session_id)
        .fetch_one(&mut *tx)
        .await?;
        if outstanding != i64::from(current.outstanding()) {
            return Err(StoreError::CorruptState);
        }
        let next = current.complete_recovery().map_err(control_error)?;
        persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
        refresh_lease(&mut tx, claim).await?;
        audit(
            &mut tx,
            &claim.session_id,
            &claim.operator_id,
            "WORKER_STOPPED",
            None,
            serde_json::json!({"epoch":claim.epoch}),
        )
        .await?;
        let updated = sqlx::query("SELECT * FROM research_sessions WHERE session_id=$1")
            .bind(&claim.session_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        update(&updated, &next, None)
    }

    /// Control coordinator must hold its local gate exclusively across this call, apply
    /// the callback before commit, and fence on ANY error before releasing the gate.
    /// The HTTP service must never call this method on behalf of a worker.
    pub async fn apply_pending<F>(
        &self,
        claim: &WorkerClaim,
        ready: bool,
        apply_local: F,
    ) -> Result<WorkerUpdate, StoreError>
    where
        F: FnOnce(&Session) -> Result<(), StoreError>,
    {
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let current = lifecycle(&row)?;
        let pending =
            sqlx::query("SELECT * FROM control_commands WHERE session_id=$1 AND status='PENDING'")
                .bind(&claim.session_id)
                .fetch_optional(&mut *tx)
                .await?;
        let (mut next, receipt) = if let Some(command) = pending {
            let revision = command.try_get::<i64, _>("revision")? as u64;
            let action = parse_action(&command.try_get::<String, _>("action")?)?;
            // A not-ready worker leaves START/RESUME PENDING, without opening its gate.
            if matches!(action, Action::Start | Action::Resume) && !ready {
                apply_local(&current)?;
                refresh_lease(&mut tx, claim).await?;
                tx.commit().await?;
                return update(&row, &current, Some(command_record(&command)?));
            }
            let fenced = if matches!(action, Action::Pause | Action::Stop) {
                current.begin_fence(revision).map_err(control_error)?
            } else {
                current
            };
            let next = fenced.acknowledge(revision, ready).map_err(control_error)?;
            persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
            let id: String = command.try_get("command_id")?;
            let applied=sqlx::query("UPDATE control_commands SET status='APPLIED',applied_at=clock_timestamp(),outstanding_attempts=$2,fence_effective=$3 WHERE command_id=$1 RETURNING *")
                .bind(&id).bind(i64::from(next.outstanding())).bind(matches!(action,Action::Pause|Action::Stop)&&next.local_fence_engaged()).fetch_one(&mut *tx).await?;
            audit(&mut tx,&claim.session_id,&claim.operator_id,"COMMAND_APPLIED",Some(&id),serde_json::json!({"epoch":claim.epoch,"generation":next.generation(),"state":state_name(&next)?})).await?;
            (next, Some(command_record(&applied)?))
        } else {
            (current, None)
        };
        if !ready && next.allows_evaluation() {
            next = next.fault().map_err(control_error)?;
            persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
            audit(
                &mut tx,
                &claim.session_id,
                &claim.operator_id,
                "WORKER_FAULTED",
                None,
                serde_json::json!({"reason":"research readiness lost"}),
            )
            .await?;
        }
        apply_local(&next)?;
        refresh_lease(&mut tx, claim).await?;
        let row = sqlx::query("SELECT * FROM research_sessions WHERE session_id=$1")
            .bind(&claim.session_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        update(&row, &next, receipt)
    }

    /// Journal a synthetic/paper attempt admitted under the coordinator's local gate.
    /// This provides no transaction execution capability.
    pub async fn record_research_attempt(
        &self,
        claim: &WorkerClaim,
        generation: u64,
    ) -> Result<WorkerUpdate, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let current = lifecycle(&row)?;
        if generation != current.generation() {
            return Err(StoreError::Conflict("stale work generation"));
        }
        let next = current.record_attempt().map_err(control_error)?;
        persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
        let attempt_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO research_attempts(attempt_id,session_id,generation,admitted_worker_epoch,status) VALUES($1,$2,$3,$4,'OUTSTANDING')")
            .bind(&attempt_id).bind(&claim.session_id).bind(i64::try_from(generation).map_err(|_|StoreError::CorruptState)?).bind(claim.epoch).execute(&mut *tx).await?;
        audit(&mut tx,&claim.session_id,&claim.operator_id,"RESEARCH_ATTEMPT_ADMITTED",None,serde_json::json!({"attempt_id":attempt_id,"generation":generation,"outstanding":next.outstanding()})).await?;
        let row = sqlx::query("SELECT * FROM research_sessions WHERE session_id=$1")
            .bind(&claim.session_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        let mut result = update(&row, &next, None)?;
        result.attempt_id = Some(attempt_id);
        Ok(result)
    }

    /// Admit an already committed immutable read-only capture. Its research attempt is
    /// created and resolved in this transaction; no financial outcome is represented.
    pub async fn record_capture_admission(
        &self,
        claim: &WorkerClaim,
        generation: u64,
        capture_id: &str,
        manifest_digest: &str,
        artifact_path: &str,
    ) -> Result<WorkerUpdate, StoreError> {
        bounded(capture_id, 100, "invalid capture id")?;
        bounded(artifact_path, 4096, "invalid artifact path")?;
        if !manifest_digest.strip_prefix("sha256:").is_some_and(|v| {
            v.len() == 64
                && v.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(StoreError::InvalidInput("invalid manifest digest"));
        }
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let current = lifecycle(&row)?;
        if current.mode() != Mode::Observe {
            return Err(StoreError::CapabilityUnavailable);
        }
        if !current.allows_evaluation() || generation != current.generation() {
            return Err(StoreError::Conflict("capture generation is fenced"));
        }
        if let Some(existing) =
            sqlx::query("SELECT * FROM capture_admissions WHERE session_id=$1 AND capture_id=$2")
                .bind(&claim.session_id)
                .bind(capture_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            if existing.try_get::<String, _>("manifest_digest")? != manifest_digest
                || existing.try_get::<String, _>("artifact_path")? != artifact_path
                || existing.try_get::<i64, _>("generation")? as u64 != generation
            {
                return Err(StoreError::Conflict("capture admission payload changed"));
            }
            let mut result = update(&row, &current, None)?;
            result.attempt_id = Some(existing.try_get("attempt_id")?);
            return Ok(result);
        }
        let attempt_id = Uuid::new_v4().to_string();
        let generation = i64::try_from(generation).map_err(|_| StoreError::CorruptState)?;
        sqlx::query("INSERT INTO research_attempts(attempt_id,session_id,generation,admitted_worker_epoch,status,resolution_evidence,resolved_at) VALUES($1,$2,$3,$4,'RESOLVED',$5,clock_timestamp())")
            .bind(&attempt_id).bind(&claim.session_id).bind(generation).bind(claim.epoch).bind(format!("read-only capture durable: {manifest_digest}")).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO capture_admissions(session_id,capture_id,attempt_id,manifest_digest,artifact_path,generation) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&claim.session_id).bind(capture_id).bind(&attempt_id).bind(manifest_digest).bind(artifact_path).bind(generation).execute(&mut *tx).await?;
        audit(&mut tx,&claim.session_id,&claim.operator_id,"CAPTURE_ADMITTED",None,serde_json::json!({"attempt_id":attempt_id,"capture_id":capture_id,"manifest_digest":manifest_digest,"generation":generation,"evidence":"raw observation; not opportunity or financial outcome"})).await?;
        tx.commit().await?;
        let mut result = update(&row, &current, None)?;
        result.attempt_id = Some(attempt_id);
        Ok(result)
    }

    /// Only positive reconciliation evidence should invoke this, never a timeout.
    pub async fn resolve_research_attempt(
        &self,
        claim: &WorkerClaim,
        attempt_id: &str,
        evidence: &str,
    ) -> Result<WorkerUpdate, StoreError> {
        bounded(evidence, 500, "resolution evidence required")?;
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let attempt = sqlx::query(
            "SELECT * FROM research_attempts WHERE session_id=$1 AND attempt_id=$2 FOR UPDATE",
        )
        .bind(&claim.session_id)
        .bind(attempt_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StoreError::NotFound)?;
        if attempt.try_get::<String, _>("status")? == "RESOLVED" {
            if attempt
                .try_get::<Option<String>, _>("resolution_evidence")?
                .as_deref()
                != Some(evidence)
            {
                return Err(StoreError::Conflict("resolution evidence changed"));
            }
            let mut result = update(&row, &lifecycle(&row)?, None)?;
            result.attempt_id = Some(attempt_id.to_owned());
            return Ok(result);
        }
        let next = lifecycle(&row)?.resolve_attempt().map_err(control_error)?;
        sqlx::query("UPDATE research_attempts SET status='RESOLVED',resolution_evidence=$2,resolved_at=clock_timestamp() WHERE attempt_id=$1")
            .bind(attempt_id).bind(evidence).execute(&mut *tx).await?;
        persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
        audit(&mut tx,&claim.session_id,&claim.operator_id,"RESEARCH_ATTEMPT_RESOLVED",None,serde_json::json!({"attempt_id":attempt_id,"evidence":evidence,"outstanding":next.outstanding()})).await?;
        let row = sqlx::query("SELECT * FROM research_sessions WHERE session_id=$1")
            .bind(&claim.session_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        update(&row, &next, None)
    }

    /// Bounded recovery discovery. Resolve this batch, then request the next; no
    /// attempt is inferred failed from its age, a timeout, or a worker replacement.
    pub async fn outstanding_research_attempt_ids(
        &self,
        claim: &WorkerClaim,
        limit: u32,
    ) -> Result<Vec<String>, StoreError> {
        if !(1..=1000).contains(&limit) {
            return Err(StoreError::InvalidInput("attempt limit must be 1..1000"));
        }
        let mut tx = self.pool.begin().await?;
        locked_worker(&mut tx, claim).await?;
        let ids=sqlx::query_scalar("SELECT attempt_id FROM research_attempts WHERE session_id=$1 AND status='OUTSTANDING' ORDER BY attempt_id LIMIT $2")
            .bind(&claim.session_id).bind(i64::from(limit)).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(ids)
    }

    pub async fn fault_worker(
        &self,
        claim: &WorkerClaim,
        reason: &str,
    ) -> Result<WorkerUpdate, StoreError> {
        bounded(reason, 500, "fault reason required")?;
        let mut tx = self.pool.begin().await?;
        let row = locked_worker(&mut tx, claim).await?;
        let next = lifecycle(&row)?.fault().map_err(control_error)?;
        let rejected:Vec<String>=sqlx::query_scalar("UPDATE control_commands SET status='REJECTED' WHERE session_id=$1 AND status='PENDING' RETURNING command_id").bind(&claim.session_id).fetch_all(&mut *tx).await?;
        for command_id in rejected {
            audit(
                &mut tx,
                &claim.session_id,
                &claim.operator_id,
                "COMMAND_REJECTED",
                Some(&command_id),
                serde_json::json!({"reason":"worker fault"}),
            )
            .await?;
        }
        persist_lifecycle(&mut tx, &claim.session_id, &next).await?;
        audit(
            &mut tx,
            &claim.session_id,
            &claim.operator_id,
            "WORKER_FAULTED",
            None,
            serde_json::json!({"reason":reason}),
        )
        .await?;
        let row = sqlx::query("SELECT * FROM research_sessions WHERE session_id=$1")
            .bind(&claim.session_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        update(&row, &next, None)
    }
}

async fn locked_worker(
    tx: &mut Transaction<'_, Postgres>,
    claim: &WorkerClaim,
) -> Result<PgRow, StoreError> {
    let row=sqlx::query("SELECT *, lease_until > clock_timestamp() AS lease_active FROM research_sessions WHERE operator_id=$1 AND session_id=$2 FOR UPDATE")
        .bind(&claim.operator_id).bind(&claim.session_id).fetch_optional(&mut **tx).await?.ok_or(StoreError::NotFound)?;
    if row.try_get::<Option<String>, _>("worker_id")?.as_deref() != Some(claim.worker_id.as_str())
        || row.try_get::<i64, _>("worker_epoch")? != claim.epoch
        || row.try_get::<String, _>("network_id")? != claim.network_id
        || !row
            .try_get::<Option<bool>, _>("lease_active")?
            .unwrap_or(false)
    {
        return Err(StoreError::Conflict("worker lease lost"));
    }
    Ok(row)
}
async fn refresh_lease(
    tx: &mut Transaction<'_, Postgres>,
    claim: &WorkerClaim,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()+make_interval(secs=>$2),last_heartbeat_at=clock_timestamp() WHERE session_id=$1")
        .bind(&claim.session_id).bind(f64::from(claim.lease_seconds)).execute(&mut **tx).await?;
    Ok(())
}
fn update(
    row: &PgRow,
    session: &Session,
    command: Option<CommandReceipt>,
) -> Result<WorkerUpdate, StoreError> {
    Ok(WorkerUpdate {
        session: session_record(row)?,
        generation: session.generation(),
        gate_open: session.allows_evaluation(),
        command,
        attempt_id: None,
    })
}
