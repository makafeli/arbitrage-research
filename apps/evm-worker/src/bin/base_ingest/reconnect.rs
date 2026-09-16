//! Explicit, invocation-wide retries for transient read-only transport failures.
//! No provider selection, persistent HALTED rearm, or enlarged per-attempt quota.
use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc};
use serde_json::Value;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub struct Budget {
    maximum: u8,
    used: u8,
}
impl Budget {
    pub fn from_environment() -> Result<Self, &'static str> {
        let setting = match std::env::var("ARB_INGEST_MAX_RECONNECTS") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => return Err("INVALID_RECONNECT_BOUND"),
        };
        Self::parse(setting.as_deref())
    }
    fn parse(value: Option<&str>) -> Result<Self, &'static str> {
        let maximum = match value {
            None | Some("0") => 0,
            Some("1") => 1,
            Some("2") => 2,
            Some("3") => 3,
            _ => return Err("INVALID_RECONNECT_BOUND"),
        };
        Ok(Self { maximum, used: 0 })
    }
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.used >= self.maximum {
            return None;
        }
        let delay = Duration::from_secs(1 << self.used);
        self.used += 1;
        Some(delay)
    }
    pub fn used(&self) -> u8 {
        self.used
    }
}

/// Retain only a classification of the last failed call, not its body or URL.
/// Successful partial-filter cleanup must not erase the original failure.
/// A later *failed* cleanup can make the attempt non-retryable conservatively.
pub struct ObservedRpc<R> {
    inner: R,
    transient: bool,
}
impl<R> ObservedRpc<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            transient: false,
        }
    }
    pub fn transient(&self) -> bool {
        self.transient
    }
}
impl<R: ReadRpc> ReadRpc for ObservedRpc<R> {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value, AdapterError> {
        self.inner.call(method, params).inspect_err(|error| {
            // This allowlist deliberately excludes 429 until Retry-After can be
            // represented, 401/403, JSON-RPC errors and every quota/identity error.
            self.transient = matches!(
                error.0,
                "RPC transport failed (endpoint redacted)"
                    | "RPC response read failed"
                    | "RPC HTTP error: provider server failure (details redacted)"
            );
        })
    }
}

pub async fn wait(delay: Duration, cancelled: &AtomicBool) -> Result<(), &'static str> {
    let until = tokio::time::Instant::now() + delay;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err("CANCELLED");
        }
        let remaining = until.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        tokio::time::sleep(remaining.min(Duration::from_millis(100))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_never_retries_and_bounds_are_canonical() {
        assert!(Budget::parse(None).unwrap().next_delay().is_none());
        for input in ["", "01", " 1", "+1", "4", "256", "-1", "1.0"] {
            assert!(Budget::parse(Some(input)).is_err());
        }
        for maximum in 0..=3 {
            let mut budget = Budget::parse(Some(&maximum.to_string())).unwrap();
            for used in 0..maximum {
                assert_eq!(budget.next_delay(), Some(Duration::from_secs(1 << used)));
            }
            assert!(budget.next_delay().is_none());
            assert!(budget.next_delay().is_none());
            assert_eq!(budget.used(), maximum as u8);
        }
    }

    struct Reply(Option<&'static str>);
    impl ReadRpc for Reply {
        fn call(&mut self, _: ReadMethod, _: Value) -> Result<Value, AdapterError> {
            self.0
                .map_or(Ok(json!(true)), |message| Err(AdapterError(message)))
        }
    }
    #[test]
    fn only_transient_errors_qualify_and_successful_cleanup_preserves_them() {
        for message in [
            "RPC transport failed (endpoint redacted)",
            "RPC response read failed",
            "RPC HTTP error: provider server failure (details redacted)",
        ] {
            let mut rpc = ObservedRpc::new(Reply(Some(message)));
            assert!(!rpc.transient());
            assert!(rpc.call(ReadMethod::EthChainId, json!([])).is_err());
            assert!(rpc.transient());
            rpc.inner.0 = None;
            assert!(
                rpc.call(ReadMethod::EthUninstallFilter, json!(["0x1"]))
                    .is_ok()
            );
            assert!(rpc.transient());
            rpc.inner.0 = Some("RPC request quota exhausted");
            assert!(rpc.call(ReadMethod::EthChainId, json!([])).is_err());
            assert!(!rpc.transient());
        }
    }
    #[test]
    fn throttling_access_data_and_quota_errors_never_retry() {
        for message in [
            "RPC HTTP error: rate limited (details redacted)",
            "RPC HTTP error: access refused (details redacted)",
            "RPC HTTP error (details redacted)",
            "RPC request quota exhausted",
            "capture RPC deadline exceeded",
            "RPC response or capture exceeds byte quota",
            "malformed JSON-RPC response",
            "RPC response is not UTF-8",
            "JSON-RPC identity mismatch or error (details redacted)",
        ] {
            let mut rpc = ObservedRpc::new(Reply(Some(message)));
            assert!(rpc.call(ReadMethod::EthChainId, json!([])).is_err());
            assert!(
                !rpc.transient(),
                "unexpected retry classification: {message}"
            );
        }
    }
    #[tokio::test]
    async fn cancelled_backoff_does_not_wait_or_reopen_admission() {
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            wait(Duration::from_secs(4), &cancelled).await,
            Err("CANCELLED")
        );
        assert_eq!(wait(Duration::ZERO, &cancelled).await, Err("CANCELLED"));
        cancelled.store(false, Ordering::SeqCst);
        assert_eq!(wait(Duration::ZERO, &cancelled).await, Ok(()));
    }
}
