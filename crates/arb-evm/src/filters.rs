//! Bounded node-local HTTP filters. Notifications are hints, not finalized input.
//! No implicit reconnect, background task, raw error output or cursor mutation.
use crate::{
    PoolRegistry,
    events::{PoolLog, decode_log, fixed_hex},
};
use arb_adapter_api::{ReadMethod, ReadRpc};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_NOTIFICATIONS: usize = 512;
const MAX_POLLS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterError {
    Cancelled,
    Provider,
    InvalidScope,
    InvalidResponse,
    LimitExceeded,
    Closed,
}
impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base filter failed: {self:?}")
    }
}
impl std::error::Error for FilterError {}
type Result<T> = std::result::Result<T, FilterError>;

/// Even an empty hint set requires a finalized catch-up from the durable cursor.
/// Removed logs refer to speculative notification history, not accepted batches.
#[derive(Debug)]
pub struct FilterHints {
    pub block_hashes: Vec<String>,
    pub logs: Vec<PoolLog>,
}

/// Two opaque node filter IDs bound to one validated pool set. Caller retains
/// endpoint affinity and explicitly closes filters; Drop never sends requests.
pub struct PoolFilters {
    ids: [Option<String>; 2],
    pools: Vec<PoolRegistry>,
    polls: usize,
    failed: bool,
}

fn request(
    rpc: &mut impl ReadRpc,
    method: ReadMethod,
    params: Value,
    cancel: &AtomicBool,
) -> Result<Value> {
    if cancel.load(Ordering::SeqCst) {
        return Err(FilterError::Cancelled);
    }
    rpc.call(method, params).map_err(|_| FilterError::Provider)
}

fn filter_id(value: Value) -> Result<String> {
    let text = value.as_str().ok_or(FilterError::InvalidResponse)?;
    let digits = text
        .strip_prefix("0x")
        .ok_or(FilterError::InvalidResponse)?;
    if digits.is_empty()
        || digits.len() > 64
        || (digits.len() > 1 && digits.starts_with('0'))
        || !digits.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(FilterError::InvalidResponse);
    }
    Ok(text.to_ascii_lowercase())
}

impl PoolFilters {
    /// Creates only block/log filters. Any known partial allocation is closed on
    /// failure; unknown IDs after a lost response expire at the node, not retried.
    pub fn open(
        rpc: &mut impl ReadRpc,
        pools: &[PoolRegistry],
        cancel: &AtomicBool,
    ) -> Result<Self> {
        if pools.is_empty()
            || pools.len() > 8
            || pools.iter().any(|p| p.validate().is_err())
            || pools
                .iter()
                .map(|p| p.pool.to_lowercase())
                .collect::<BTreeSet<_>>()
                .len()
                != pools.len()
        {
            return Err(FilterError::InvalidScope);
        }
        if request(rpc, ReadMethod::EthChainId, json!([]), cancel)? != "0x2105" {
            return Err(FilterError::InvalidScope);
        }
        let mut filters = Self {
            ids: [None, None],
            pools: pools.to_vec(),
            polls: 0,
            failed: false,
        };
        let opened = (|| {
            filters.ids[0] = Some(filter_id(request(
                rpc,
                ReadMethod::EthNewBlockFilter,
                json!([]),
                cancel,
            )?)?);
            let addresses: Vec<_> = pools.iter().map(|p| p.pool.to_lowercase()).collect();
            let logs = filter_id(request(
                rpc,
                ReadMethod::EthNewFilter,
                json!([{"fromBlock":"latest", "toBlock":"latest", "address":addresses}]),
                cancel,
            )?)?;
            if filters.ids[0].as_ref() == Some(&logs) {
                return Err(FilterError::InvalidResponse);
            }
            filters.ids[1] = Some(logs);
            if cancel.load(Ordering::SeqCst) {
                return Err(FilterError::Cancelled);
            }
            Ok(())
        })();
        if let Err(error) = opened {
            let _ = filters.close(rpc);
            return Err(error);
        }
        Ok(filters)
    }

    /// A failed or cancelled poll permanently fences this pair. Recreate only
    /// through an explicit new invocation and recover from the stored checkpoint.
    pub fn poll(&mut self, rpc: &mut impl ReadRpc, cancel: &AtomicBool) -> Result<FilterHints> {
        if self.failed || self.ids.iter().any(Option::is_none) {
            return Err(FilterError::Closed);
        }
        self.failed = true;
        if self.polls >= MAX_POLLS {
            return Err(FilterError::LimitExceeded);
        }
        self.polls += 1;
        let hashes = request(
            rpc,
            ReadMethod::EthGetFilterChanges,
            json!([self.ids[0]]),
            cancel,
        )?;
        let hashes = hashes.as_array().ok_or(FilterError::InvalidResponse)?;
        if hashes.len() > MAX_NOTIFICATIONS {
            return Err(FilterError::LimitExceeded);
        }
        let block_hashes = hashes
            .iter()
            .map(|v| fixed_hex(v, 32).map_err(|_| FilterError::InvalidResponse))
            .collect::<Result<Vec<_>>>()?;
        let events = request(
            rpc,
            ReadMethod::EthGetFilterChanges,
            json!([self.ids[1]]),
            cancel,
        )?;
        let events = events.as_array().ok_or(FilterError::InvalidResponse)?;
        if events.len() > MAX_NOTIFICATIONS {
            return Err(FilterError::LimitExceeded);
        }
        let mut logs = Vec::with_capacity(events.len());
        for event in events {
            let address = event["address"]
                .as_str()
                .ok_or(FilterError::InvalidResponse)?;
            let pool = self
                .pools
                .iter()
                .find(|p| p.pool.eq_ignore_ascii_case(address))
                .ok_or(FilterError::InvalidScope)?;
            logs.push(decode_log(event, pool).map_err(|_| FilterError::InvalidResponse)?);
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(FilterError::Cancelled);
        }
        self.failed = false;
        Ok(FilterHints { block_hashes, logs })
    }

    /// At most two cleanup calls, including after a failed poll. Every known ID
    /// is attempted once even if its sibling fails. No hidden network in Drop.
    /// False is a valid node answer for an already-absent filter.
    pub fn close(&mut self, rpc: &mut impl ReadRpc) -> Result<()> {
        self.failed = true;
        let mut error = None;
        for id in &mut self.ids {
            if let Some(id) = id.take() {
                match rpc.call(ReadMethod::EthUninstallFilter, json!([id])) {
                    Ok(value) if value.is_boolean() => (),
                    Ok(_) => {
                        error.get_or_insert(FilterError::InvalidResponse);
                    }
                    Err(_) => {
                        error.get_or_insert(FilterError::Provider);
                    }
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
}
