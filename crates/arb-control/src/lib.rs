//! Session-local research control with durable acknowledgements and cancellation fences.
//! Chain capture CLIs are separate processes until explicitly integrated with this runner.
use arb_storage::{Store, StoreError, WorkerClaim, WorkerUpdate};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkGeneration {
    generation: u64,
    epoch: i64,
}

impl WorkGeneration { pub fn generation(&self) -> u64 { self.generation } }
#[derive(Debug)]
struct Gate {
    generation: u64,
    open: bool,
    lease_deadline: Instant,
    recovery_required: bool,
}

/// Not Clone: one control owner; work admission handles may share its gated reference.
/// Admission, command application and error fencing all serialize on the same async mutex.
pub struct ControlWorker {
    store: Store,
    claim: WorkerClaim,
    gate: Arc<Mutex<Gate>>,
}

impl ControlWorker {
    pub async fn claim(
        store: Store,
        operator: &str,
        session: &str,
        network: &str,
        worker: &str,
        lease_seconds: u32,
    ) -> Result<Self, StoreError> {
        let deadline = Instant::now() + Duration::from_secs(u64::from(lease_seconds));
        let claim = store
            .claim_worker(operator, session, network, worker, lease_seconds)
            .await?;
        Ok(Self {
            store,
            claim,
            gate: Arc::new(Mutex::new(Gate {
                generation: 0,
                open: false,
                lease_deadline: deadline,
                recovery_required: false,
            })),
        })
    }

    pub async fn complete_recovery(&self) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        gate.open = false;
        gate.recovery_required = true;
        let deadline = self.deadline();
        let result = self.store.complete_worker_recovery(&self.claim).await;
        match &result {
            Ok(update) => {
                gate.recovery_required = false;
                install(&mut gate, update, deadline)
            }
            Err(_) => {
                gate.open = false;
                gate.recovery_required = true;
            }
        };
        result
    }

    /// Poll command intent. A successful API request alone never opens/closes this gate.
    /// Lock remains held until PostgreSQL commit, so no admission observes an early ACK.
    pub async fn tick(&self, ready: bool) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        if gate.recovery_required {
            return Err(StoreError::Conflict("worker recovery required"));
        }
        gate.open = false;
        gate.recovery_required = true;
        let deadline = self.deadline();
        let result = self
            .store
            .apply_pending(&self.claim, ready, |session| {
                gate.generation = session.generation();
                Ok(())
            })
            .await;
        match &result {
            Ok(update) => {
                gate.recovery_required = false;
                install(&mut gate, update, deadline)
            }
            Err(_) => {
                gate.open = false;
                gate.recovery_required = true;
            }
        };
        result
    }

    pub async fn generation(&self) -> Result<WorkGeneration, StoreError> {
        let mut gate = self.gate.lock().await;
        if Instant::now() >= gate.lease_deadline {
            gate.open = false;
        }
        if !gate.open {
            return Err(StoreError::Conflict("evaluation gate closed"));
        }
        Ok(WorkGeneration {
            generation: gate.generation,
            epoch: self.claim.epoch(),
        })
    }

    /// Generation tagging applies when work is QUEUED. Returning a result to a later
    /// generation cannot bypass the gate. This records only a research attempt.
    pub async fn admit_research_result(
        &self,
        work: WorkGeneration,
    ) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        if Instant::now() >= gate.lease_deadline {
            gate.open = false;
        }
        if !gate.open || work.generation != gate.generation || work.epoch != self.claim.epoch() {
            return Err(StoreError::Conflict("stale or fenced research work"));
        }
        gate.open = false;
        gate.recovery_required = true;
        let result = self
            .store
            .record_research_attempt(&self.claim, work.generation)
            .await;
        if let Ok(update) = &result {
            gate.recovery_required = false;
            gate.open = update.gate_open;
        }
        result
    }

    /// The capture bundle must already be fsynced. A fenced late result remains an
    /// unadmitted artifact; it is never labelled an opportunity or paper fill.
    pub async fn admit_capture_manifest(
        &self,
        work: WorkGeneration,
        capture_id: &str,
        manifest_digest: &str,
        artifact_path: &str,
    ) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        if Instant::now() >= gate.lease_deadline {
            gate.open = false;
        }
        if !gate.open || work.generation != gate.generation || work.epoch != self.claim.epoch() {
            return Err(StoreError::Conflict("stale or fenced capture"));
        }
        gate.open = false;
        gate.recovery_required = true;
        let result = self
            .store
            .record_capture_admission(
                &self.claim,
                work.generation,
                capture_id,
                manifest_digest,
                artifact_path,
            )
            .await;
        if let Ok(update) = &result {
            gate.recovery_required = false;
            gate.open = update.gate_open;
        }
        result
    }

    pub async fn admit_decision_traces(&self, work: WorkGeneration, traces: &[arb_domain::DecisionTrace]) -> Result<Vec<arb_storage::StoredDecisionTrace>, StoreError> {
        let mut gate=self.gate.lock().await;
        if Instant::now()>=gate.lease_deadline { gate.open=false; }
        if !gate.open || work.generation!=gate.generation || work.epoch!=self.claim.epoch() { return Err(StoreError::Conflict("stale or fenced decisions")); }
        gate.open=false; gate.recovery_required=true;
        let result=self.store.append_decision_traces(&self.claim,work.generation,traces).await;
        if result.is_ok() { gate.recovery_required=false; gate.open=true; }
        result
    }
    /// Internal virtual accounting. HTTP clients cannot reserve or settle funds.
    pub async fn apply_paper_command(&self, work: Option<WorkGeneration>, run_id: &str, key: &str, command: arb_paper::PaperCommand) -> Result<arb_storage::StoredPaperEvent, StoreError> {
        let mut gate=self.gate.lock().await;
        if Instant::now()>=gate.lease_deadline { gate.open=false; }
        if matches!(&command,arb_paper::PaperCommand::Reserve{..}) && (!gate.open || !work.is_some_and(|w|w.generation==gate.generation && w.epoch==self.claim.epoch())) { return Err(StoreError::Conflict("stale or fenced paper reservation")); }
        let was_open=gate.open; let was_recovery=gate.recovery_required;
        gate.open=false; gate.recovery_required=true;
        let result=self.store.apply_paper_command(&self.claim,gate.generation,run_id,key,command).await;
        let known_rejection=matches!(&result,Err(StoreError::Conflict("paper command violates exact inventory or run invariants"))|Err(StoreError::InvalidInput(_)));
        if result.is_ok() || known_rejection { gate.recovery_required=was_recovery; gate.open=was_open&&!was_recovery; }
        result
    }

    /// Reconciliation deliberately continues while paused/stopped/draining.
    pub async fn resolve_research_attempt(
        &self,
        attempt_id: &str,
        evidence: &str,
    ) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        let was_recovery_required = gate.recovery_required;
        gate.open = false;
        gate.recovery_required = true;
        let result = self
            .store
            .resolve_research_attempt(&self.claim, attempt_id, evidence)
            .await;
        if let Ok(update) = &result {
            gate.recovery_required = was_recovery_required;
            gate.open = update.gate_open && !gate.recovery_required;
            gate.generation = update.generation;
        };
        result
    }

    pub async fn outstanding_research_attempt_ids(
        &self,
        limit: u32,
    ) -> Result<Vec<String>, StoreError> {
        self.store
            .outstanding_research_attempt_ids(&self.claim, limit)
            .await
    }

    pub async fn fault(&self, reason: &str) -> Result<WorkerUpdate, StoreError> {
        let mut gate = self.gate.lock().await;
        gate.open = false;
        gate.recovery_required = true;
        let result = self.store.fault_worker(&self.claim, reason).await;
        if let Ok(update) = &result {
            gate.generation = update.generation;
        }
        result
    }

    /// Local cancellation on shutdown closes admission immediately; API observed state
    /// remains its durable last observation until a worker command/recovery changes it.
    pub async fn fence_local(&self) {
        let mut gate = self.gate.lock().await;
        gate.open = false;
        gate.recovery_required = true;
    }
    fn deadline(&self) -> Instant {
        Instant::now() + Duration::from_secs(u64::from(self.claim.lease_seconds()))
    }
}
fn install(gate: &mut Gate, update: &WorkerUpdate, deadline: Instant) {
    gate.generation = update.generation;
    gate.open = update.gate_open && !gate.recovery_required;
    gate.lease_deadline = deadline;
}
