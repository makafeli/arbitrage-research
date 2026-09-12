//! Read-only acquisition boundary. This transport exposes no signing or broadcast method.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fmt,
    io::Read,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError(pub &'static str);
impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AdapterError {}
pub type Result<T> = std::result::Result<T, AdapterError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Chain {
    BaseMainnet,
    SolanaMainnet,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ReadMethod {
    EthChainId,
    EthGetBlockByNumber,
    EthGetCode,
    EthCall,
    GetGenesisHash,
    GetMultipleAccounts,
}
impl ReadMethod {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::EthChainId => "eth_chainId",
            Self::EthGetBlockByNumber => "eth_getBlockByNumber",
            Self::EthGetCode => "eth_getCode",
            Self::EthCall => "eth_call",
            Self::GetGenesisHash => "getGenesisHash",
            Self::GetMultipleAccounts => "getMultipleAccounts",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcRecord {
    pub sequence: u64,
    pub method: ReadMethod,
    pub params: Value,
    /// Exact UTF-8 response body; never includes endpoint URLs, request headers or RPC errors.
    pub response: String,
}

pub trait ReadRpc {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value>;
}

/// Synchronous, one-request-at-a-time capture transport. Not a latency-critical worker runtime.
/// HTTP is allowed only for loopback fixture servers; production RPC requires HTTPS.
pub struct HttpReadRpc {
    endpoint: reqwest::Url,
    client: reqwest::blocking::Client,
    max_response_bytes: usize,
    max_capture_bytes: usize,
    retained_bytes: usize,
    max_requests: usize,
    requests_sent: usize,
    started: Instant,
    request_timeout: Duration,
    records: Vec<RpcRecord>,
}
impl HttpReadRpc {
    pub fn new(
        endpoint: &str,
        timeout: Duration,
        max_response_bytes: usize,
        max_requests: usize,
    ) -> Result<Self> {
        let url =
            reqwest::Url::parse(endpoint).map_err(|_| AdapterError("invalid RPC endpoint"))?;
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || timeout.is_zero()
            || timeout > Duration::from_secs(60)
            || !(1024..=8 * 1024 * 1024).contains(&max_response_bytes)
            || !(1..=4096).contains(&max_requests)
        {
            return Err(AdapterError("unsafe endpoint or unbounded RPC limits"));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AdapterError("RPC client initialization failed"))?;
        Ok(Self {
            endpoint: url,
            client,
            max_response_bytes,
            max_capture_bytes: 64 * 1024 * 1024,
            retained_bytes: 0,
            max_requests,
            requests_sent: 0,
            started: Instant::now(),
            request_timeout: timeout,
            records: Vec::new(),
        })
    }
    /// Begin another independently replayable pool transcript. Session request,
    /// byte and elapsed-time limits remain cumulative across all pool captures.
    /// Requests are synchronous, so no previous response can arrive after reset.
    pub fn take_records(&mut self) -> Vec<RpcRecord> {
        std::mem::take(&mut self.records)
    }
    pub fn into_records(self) -> Vec<RpcRecord> {
        self.records
    }
}
impl ReadRpc for HttpReadRpc {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value> {
        if self.requests_sent >= self.max_requests {
            return Err(AdapterError("RPC request quota exhausted"));
        }
        let remaining = Duration::from_secs(60)
            .checked_sub(self.started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or(AdapterError("capture RPC deadline exceeded"))?;
        self.requests_sent += 1;
        let sequence = self.records.len() as u64;
        let request =
            json!({"jsonrpc":"2.0", "id":sequence, "method":method.wire_name(), "params":params});
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&request)
            .timeout(remaining.min(self.request_timeout))
            .send()
            .map_err(|_| AdapterError("RPC transport failed (endpoint redacted)"))?;
        if !response.status().is_success() {
            return Err(AdapterError("RPC HTTP error (details redacted)"));
        }
        let mut bytes = Vec::new();
        response
            .take(self.max_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AdapterError("RPC response read failed"))?;
        if bytes.len() > self.max_response_bytes
            || self.retained_bytes + bytes.len() > self.max_capture_bytes
        {
            return Err(AdapterError("RPC response or capture exceeds byte quota"));
        }
        let response =
            String::from_utf8(bytes).map_err(|_| AdapterError("RPC response is not UTF-8"))?;
        let parsed: Value = serde_json::from_str(&response)
            .map_err(|_| AdapterError("malformed JSON-RPC response"))?;
        if parsed.get("jsonrpc") != Some(&json!("2.0"))
            || parsed.get("id") != Some(&json!(sequence))
            || parsed.get("error").is_some()
            || parsed.get("result").is_none()
            || parsed.as_object().is_none_or(|o| {
                o.keys()
                    .any(|k| !matches!(k.as_str(), "jsonrpc" | "id" | "result"))
            })
        {
            return Err(AdapterError(
                "JSON-RPC identity mismatch or error (details redacted)",
            ));
        }
        let result = parsed["result"].clone();
        self.retained_bytes += response.len();
        self.records.push(RpcRecord {
            sequence,
            method,
            params,
            response,
        });
        Ok(result)
    }
}

/// Strict transcript playback consumes calls in acquisition order and compares complete parameters.
pub struct TranscriptRpc {
    records: std::collections::VecDeque<RpcRecord>,
    next: u64,
}
impl TranscriptRpc {
    pub fn new(records: Vec<RpcRecord>) -> Self {
        Self {
            records: records.into(),
            next: 0,
        }
    }
    pub fn finish(self) -> Result<()> {
        if self.records.is_empty() {
            Ok(())
        } else {
            Err(AdapterError("unconsumed transcript records"))
        }
    }
}
impl ReadRpc for TranscriptRpc {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value> {
        let record = self
            .records
            .pop_front()
            .ok_or(AdapterError("missing transcript record"))?;
        if record.sequence != self.next || record.method != method || record.params != params {
            return Err(AdapterError("transcript sequence or request mismatch"));
        }
        let value: Value = serde_json::from_str(&record.response)
            .map_err(|_| AdapterError("malformed transcript response"))?;
        if value["jsonrpc"] != "2.0"
            || value["id"] != self.next
            || value.get("error").is_some()
            || value.get("result").is_none()
        {
            return Err(AdapterError("invalid transcript RPC result"));
        }
        self.next += 1;
        Ok(value["result"].clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum StateContext {
    Evm {
        block_number: u64,
        block_hash: String,
        parent_hash: String,
        block_timestamp_seconds: u64,
        finality: String,
    },
    Solana {
        slot: u64,
        genesis_hash: String,
        commitment: String,
        account_context: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotQuality {
    pub coherent: bool,
    pub complete_for_quote: bool,
    pub quote_implementation_qualified: bool,
    pub observed_at_ms: u64,
    pub max_age_ms: u64,
    pub reasons: Vec<String>,
}
impl SnapshotQuality {
    pub fn ineligibility(&self, now_ms: u64, invalidated: bool) -> Vec<String> {
        let mut reasons = self.reasons.clone();
        if !self.coherent {
            reasons.push("incoherent-state".into());
        }
        if !self.complete_for_quote {
            reasons.push("missing-required-quote-state".into());
        }
        if !self.quote_implementation_qualified {
            reasons.push("quote-implementation-unqualified".into());
        }
        if self.max_age_ms == 0
            || now_ms < self.observed_at_ms
            || now_ms - self.observed_at_ms > self.max_age_ms
        {
            reasons.push("stale-or-invalid-clock".into());
        }
        if invalidated {
            reasons.push("chain-context-invalidated".into());
        }
        reasons
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draining_records_preserves_transport_budgets_and_deadline() {
        let mut rpc = HttpReadRpc::new("http://127.0.0.1:9", Duration::from_secs(1), 1024, 1).unwrap();
        rpc.requests_sent = 1;
        rpc.retained_bytes = 123;
        let started = rpc.started;
        assert!(rpc.take_records().is_empty());
        assert_eq!(rpc.started, started);
        assert_eq!(rpc.retained_bytes, 123);
        assert_eq!(rpc.call(ReadMethod::EthChainId, json!([])).unwrap_err().0, "RPC request quota exhausted");
        rpc.requests_sent = 0;
        rpc.started = Instant::now().checked_sub(Duration::from_secs(61)).unwrap();
        assert!(rpc.take_records().is_empty());
        assert_eq!(rpc.call(ReadMethod::EthChainId, json!([])).unwrap_err().0, "capture RPC deadline exceeded");
    }
    #[test]
    fn wire_enum_cannot_deserialize_broadcast() {
        assert!(serde_json::from_str::<ReadMethod>("\"sendTransaction\"").is_err());
    }
    #[test]
    fn rejects_insecure_and_credential_endpoints() {
        for url in [
            "http://example.com",
            "https://name:secret@example.com",
            "file:///tmp/rpc",
        ] {
            assert!(HttpReadRpc::new(url, Duration::from_secs(2), 4096, 1).is_err());
        }
    }
    #[test]
    fn transcript_rejects_mismatch_and_leftovers() {
        let record = RpcRecord {
            sequence: 0,
            method: ReadMethod::EthChainId,
            params: json!([]),
            response: json!({"jsonrpc":"2.0", "id":0, "result":"0x2105"}).to_string(),
        };
        assert!(TranscriptRpc::new(vec![record.clone()]).finish().is_err());
        assert!(
            TranscriptRpc::new(vec![record.clone()])
                .call(ReadMethod::GetGenesisHash, json!([]))
                .is_err()
        );
        let mut rpc = TranscriptRpc::new(vec![record]);
        assert_eq!(
            rpc.call(ReadMethod::EthChainId, json!([])).unwrap(),
            "0x2105"
        );
        rpc.finish().unwrap();
    }
    #[test]
    fn quality_rejects_partial_stale_future_and_rollback() {
        let q = SnapshotQuality {
            coherent: true,
            complete_for_quote: true,
            quote_implementation_qualified: true,
            observed_at_ms: 100,
            max_age_ms: 10,
            reasons: vec![],
        };
        assert!(q.ineligibility(105, false).is_empty());
        for (now, rollback) in [(99, false), (111, false), (105, true)] {
            assert!(!q.ineligibility(now, rollback).is_empty());
        }
    }
}
