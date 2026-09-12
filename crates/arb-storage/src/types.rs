use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewSession {
    pub network_id: String,
    pub mode: String,
    pub configuration_digest: String,
    pub experiment_id: String,
    pub strategy_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewCommand {
    pub action: String,
    pub expected_revision: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub network_id: String,
    pub mode: String,
    pub observed_state: String,
    pub health: String,
    pub desired_revision: String,
    pub applied_revision: String,
    pub outstanding_attempts: u32,
    pub execution_authorized: bool,
    pub last_heartbeat_at: Option<String>,
    pub configuration_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CommandReceipt {
    pub command_id: String,
    pub session_id: String,
    pub revision: String,
    pub status: String,
    pub action: String,
    pub accepted_at: String,
    pub applied_at: Option<String>,
    pub outstanding_attempts: u32,
    pub fence_effective: bool,
    pub signer_revocation_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionPage {
    pub items: Vec<SessionRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("complete session export exceeds bounded limits")]
    ExportLimitExceeded,
    #[error("record not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("this research deployment has no requested execution capability")]
    CapabilityUnavailable,
    #[error("storage unavailable")]
    Database(#[from] sqlx::Error),
    #[error("migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("persisted state failed validation")]
    CorruptState,
}
