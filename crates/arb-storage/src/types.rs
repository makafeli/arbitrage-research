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
    #[error("storage unavailable ({})", database_error_label(.0))]
    Database(#[from] sqlx::Error),
    #[error("migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("persisted state failed validation")]
    CorruptState,
}

/// A coarse, fixed-vocabulary classification of a `sqlx::Error` for logs. Never
/// includes the DSN, host, user, SQL text or the driver's raw message — only our
/// own static labels, the database's SQLSTATE code, or `std::io::ErrorKind`'s
/// `Debug` name, none of which can carry a secret (they are all fixed enums).
fn database_error_label(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::PoolTimedOut => "pool timed out".to_owned(),
        sqlx::Error::PoolClosed => "pool closed".to_owned(),
        sqlx::Error::WorkerCrashed => "worker crashed".to_owned(),
        sqlx::Error::Io(io_error) => format!("io {:?}", io_error.kind()),
        sqlx::Error::Tls(_) => "tls".to_owned(),
        sqlx::Error::Protocol(_) => "protocol".to_owned(),
        sqlx::Error::Configuration(_) => "configuration".to_owned(),
        sqlx::Error::RowNotFound => "row not found".to_owned(),
        sqlx::Error::TypeNotFound { .. } => "type not found".to_owned(),
        sqlx::Error::ColumnNotFound(_) => "column not found".to_owned(),
        sqlx::Error::ColumnDecode { .. } => "column decode".to_owned(),
        sqlx::Error::Decode(_) => "decode".to_owned(),
        sqlx::Error::Encode(_) => "encode".to_owned(),
        sqlx::Error::Database(database_error) => format!(
            "database {}",
            database_error
                .code()
                .map(|code| code.into_owned())
                .unwrap_or_else(|| "unknown".to_owned())
        ),
        _ => "other".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::error::{DatabaseError, ErrorKind};
    use std::{borrow::Cow, fmt};

    #[derive(Debug)]
    struct FakeDatabaseError {
        code: Option<&'static str>,
        message: &'static str,
    }

    impl fmt::Display for FakeDatabaseError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for FakeDatabaseError {}

    impl DatabaseError for FakeDatabaseError {
        fn message(&self) -> &str {
            self.message
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            self.code.map(Cow::Borrowed)
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[test]
    fn database_error_label_reports_coarse_kind_without_leaking_the_raw_message() {
        let pool_timeout = StoreError::from(sqlx::Error::PoolTimedOut);
        assert_eq!(
            pool_timeout.to_string(),
            "storage unavailable (pool timed out)"
        );

        let raw_message = "internal connection string user=arb password=leak";
        let protocol = StoreError::from(sqlx::Error::Protocol(raw_message.to_owned()));
        let rendered = protocol.to_string();
        assert_eq!(rendered, "storage unavailable (protocol)");
        assert!(!rendered.contains(raw_message));

        let db_error = StoreError::from(sqlx::Error::Database(Box::new(FakeDatabaseError {
            code: Some("23505"),
            message: raw_message,
        })));
        let rendered = db_error.to_string();
        assert_eq!(rendered, "storage unavailable (database 23505)");
        assert!(!rendered.contains(raw_message));

        let unknown_code = StoreError::from(sqlx::Error::Database(Box::new(FakeDatabaseError {
            code: None,
            message: raw_message,
        })));
        // A missing code still falls back to a fixed label, never the raw message.
        assert_eq!(
            unknown_code.to_string(),
            "storage unavailable (database unknown)"
        );

        let io_error = StoreError::from(sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            raw_message,
        )));
        let rendered = io_error.to_string();
        assert_eq!(rendered, "storage unavailable (io ConnectionReset)");
        assert!(!rendered.contains(raw_message));

        let row_not_found = StoreError::from(sqlx::Error::RowNotFound);
        assert_eq!(
            row_not_found.to_string(),
            "storage unavailable (row not found)"
        );
    }
}
