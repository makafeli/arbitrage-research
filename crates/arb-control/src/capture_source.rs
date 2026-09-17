//! Source association shares the existing cancellation and worker-epoch boundary.
use super::*;
use arb_storage::{CaptureSourceBinding, IngestionHead};

impl ControlWorker {
    /// Bind a source only after explicit STOPPED recovery, never from an open gate.
    pub async fn configure_capture_ingestion_source(
        &self,
        source: Option<&CaptureSourceBinding>,
    ) -> Result<(), StoreError> {
        let mut gate = self.gate.lock().await;
        if gate.open || gate.recovery_required || Instant::now() >= gate.lease_deadline {
            gate.open = false;
            return Err(StoreError::Conflict("capture source configuration requires recovered worker"));
        }
        gate.recovery_required = true;
        let result = self.store.configure_capture_ingestion_source(&self.claim, source).await;
        if result.is_ok() { gate.recovery_required = false; }
        result
    }

    /// Cancellation, generation and lease fences also apply to producer associations.
    pub async fn bind_captures_to_ingestion(
        &self,
        work: WorkGeneration,
        source: &CaptureSourceBinding,
        checkpoint: &IngestionHead,
        captures: &[arb_domain::DecisionCaptureRef],
    ) -> Result<(), StoreError> {
        let mut gate = self.gate.lock().await;
        if Instant::now() >= gate.lease_deadline { gate.open = false; }
        if !gate.open || work.generation != gate.generation || work.epoch != self.claim.epoch() {
            return Err(StoreError::Conflict("stale or fenced capture binding"));
        }
        gate.open = false;
        gate.recovery_required = true;
        let result = self.store.bind_captures_to_ingestion(&self.claim, work.generation, source, checkpoint, captures).await;
        if result.is_ok() {
            gate.recovery_required = false;
            gate.open = true;
        }
        result
    }
}
