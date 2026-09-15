//! Strict Base V3 notification decoding. Events invalidate snapshots; they do
//! not constitute complete quote state or independent proof of provider truth.
use crate::{PoolRegistry, address, i24word, quantity, u32word, unsigned};
use arb_adapter_api::{AdapterError, Result};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ABI signatures derived from IUniswapV3PoolEvents at crate::SOURCE_COMMIT.
pub const INITIALIZE: &str = "0x98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95";
pub const MINT: &str = "0x7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde";
pub const COLLECT: &str = "0x70935338e69775456a85ddef226c395fb668b63fa0115f5f20610b388e6ca9c0";
pub const BURN: &str = "0x0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c";
pub const SWAP: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";
pub const FLASH: &str = "0xbdbdb71d7860376ba52b25a5028beea23581364a40522f6bcfb86bb1f2dca633";
pub const ORACLE_CAPACITY: &str =
    "0xac49e518f90a358f652e4400164f05a5d8f7e35e7747279bc3a93dbf584e125a";
pub const PROTOCOL_FEE: &str = "0x973d8d92bb299f4af6ce49b52a8adb85ae46b9f214c4c4fc06ac77401237b133";
pub const COLLECT_PROTOCOL: &str =
    "0x596b573906218d3411850b26a6b437d6c4522fdb43d2d2386263f86d50b8b151";

/// A header has identity and ancestry, but is not intrinsically finalized.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockHeader {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp_seconds: u64,
}
impl BlockHeader {
    /// Require complete non-pending identity fields and canonical RPC quantities.
    pub fn from_rpc(value: &Value) -> Result<Self> {
        Ok(Self {
            number: quantity(&value["number"])?,
            hash: fixed_hex(&value["hash"], 32)?,
            parent_hash: fixed_hex(&value["parentHash"], 32)?,
            timestamp_seconds: quantity(&value["timestamp"])?,
        })
    }

    /// Validate a persisted/publicly constructed checkpoint before any RPC call.
    pub fn validate(&self) -> Result<()> {
        fixed_hex(&Value::String(self.hash.clone()), 32)?;
        fixed_hex(&Value::String(self.parent_hash.clone()), 32)?;
        Ok(())
    }

    /// Compare semantic header identities, accepting hex case differences only.
    pub fn same_block(&self, other: &Self) -> bool {
        self.number == other.number
            && self.timestamp_seconds == other.timestamp_seconds
            && self.hash.eq_ignore_ascii_case(&other.hash)
            && self.parent_hash.eq_ignore_ascii_case(&other.parent_hash)
    }
}

/// Notification relation only. Even an extension still requires anchored reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadChange {
    Duplicate,
    Extension,
    Gap {
        first_missing: u64,
        last_missing: u64,
    },
    ReorgOrOutOfOrder,
}

/// Detect skipped/reordered heads; never turn a newHeads notification into finality.
pub fn head_change(previous: &BlockHeader, next: &BlockHeader) -> Result<HeadChange> {
    previous.validate()?;
    next.validate()?;
    if previous.same_block(next) {
        return Ok(HeadChange::Duplicate);
    }
    if next.number <= previous.number
        || next.timestamp_seconds < previous.timestamp_seconds
        || next.hash.eq_ignore_ascii_case(&previous.hash)
    {
        return Ok(HeadChange::ReorgOrOutOfOrder);
    }
    let following = previous
        .number
        .checked_add(1)
        .ok_or(AdapterError("head height overflow"))?;
    if next.number > following {
        return Ok(HeadChange::Gap {
            first_missing: following,
            last_missing: next.number - 1,
        });
    }
    Ok(if next.parent_hash.eq_ignore_ascii_case(&previous.hash) {
        HeadChange::Extension
    } else {
        HeadChange::ReorgOrOutOfOrder
    })
}

/// Decode a newHeads payload bound to its active subscription ID.
pub fn head_notification(value: &Value, subscription: &str) -> Result<BlockHeader> {
    BlockHeader::from_rpc(notification_result(value, subscription)?)
}

/// Decode a logs payload; `removed` is retained, never silently discarded.
pub fn log_notification(value: &Value, subscription: &str, pool: &PoolRegistry) -> Result<PoolLog> {
    decode_log(notification_result(value, subscription)?, pool)
}

fn notification_result<'a>(value: &'a Value, subscription: &str) -> Result<&'a Value> {
    if subscription.is_empty()
        || subscription.len() > 256
        || subscription.bytes().any(|b| b.is_ascii_control())
        || value["jsonrpc"] != "2.0"
        || value["method"] != "eth_subscription"
        || value.get("id").is_some()
        || value.get("error").is_some()
        || value["params"]["subscription"].as_str() != Some(subscription)
    {
        return Err(AdapterError(
            "invalid or mismatched EVM subscription notification",
        ));
    }
    value["params"]
        .get("result")
        .ok_or(AdapterError("missing EVM notification result"))
}

/// All authoritative amounts are exact decimal strings, including signed int256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum PoolEvent {
    Initialize {
        sqrt_price_x96: String,
        tick: i32,
    },
    Mint {
        sender: String,
        owner: String,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: String,
        amount0: String,
        amount1: String,
    },
    Collect {
        owner: String,
        recipient: String,
        tick_lower: i32,
        tick_upper: i32,
        amount0: String,
        amount1: String,
    },
    Burn {
        owner: String,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: String,
        amount0: String,
        amount1: String,
    },
    Swap {
        sender: String,
        recipient: String,
        amount0: String,
        amount1: String,
        sqrt_price_x96: String,
        liquidity: String,
        tick: i32,
    },
    Flash {
        sender: String,
        recipient: String,
        amount0: String,
        amount1: String,
        paid0: String,
        paid1: String,
    },
    IncreaseObservationCardinalityNext {
        old: u32,
        new: u32,
    },
    SetFeeProtocol {
        token0_old: u32,
        token1_old: u32,
        token0_new: u32,
        token1_new: u32,
    },
    CollectProtocol {
        sender: String,
        recipient: String,
        amount0: String,
        amount1: String,
    },
}

/// Transaction/log identity is retained to reject duplicates and conflicting data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolLog {
    pub pool: String,
    pub block_number: u64,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub log_index: u64,
    pub removed: bool,
    pub decoded: PoolEvent,
}

pub(crate) fn fixed_hex(value: &Value, bytes: usize) -> Result<String> {
    let text = value
        .as_str()
        .ok_or(AdapterError("missing fixed EVM hex value"))?;
    if text.len() != 2 + 2 * bytes
        || !text.starts_with("0x")
        || !text.as_bytes()[2..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(AdapterError("invalid fixed EVM hex length or encoding"));
    }
    Ok(text.to_ascii_lowercase())
}

fn uint(bytes: &[u8], width: usize) -> Result<String> {
    Ok(U256::from_big_endian(unsigned(bytes, width)?).to_string())
}
fn int256(bytes: &[u8]) -> String {
    let number = U256::from_big_endian(bytes);
    if bytes[0] & 128 == 0 {
        number.to_string()
    } else {
        format!("-{}", (!number).overflowing_add(U256::one()).0)
    }
}
fn event_address(bytes: &[u8]) -> Result<String> {
    // An indexed owner/recipient may be zero; only the approved emitter is nonzero.
    Ok(format!("0x{}", hex::encode(unsigned(bytes, 20)?)))
}
fn tick(bytes: &[u8]) -> Result<i32> {
    let value = i24word(bytes)?;
    if !(-887272..=887272).contains(&value) {
        return Err(AdapterError("event tick outside V3 bounds"));
    }
    Ok(value)
}
fn tick_range(topics: &[Vec<u8>], spacing: i32) -> Result<(i32, i32)> {
    let lower = tick(&topics[2])?;
    let upper = tick(&topics[3])?;
    if lower >= upper || lower % spacing != 0 || upper % spacing != 0 {
        return Err(AdapterError("invalid V3 position tick range"));
    }
    Ok((lower, upper))
}
fn price(bytes: &[u8]) -> Result<String> {
    let number = U256::from_big_endian(unsigned(bytes, 20)?);
    // Width/range validation only: no claim of independent TickMath equivalence.
    let maximum = U256::from_dec_str("1461446703485210103287273052203988822378723970342")
        .expect("constant V3 sqrt-price bound");
    if number < U256::from(4295128739_u64) || number >= maximum {
        return Err(AdapterError("event sqrt price outside V3 bounds"));
    }
    Ok(number.to_string())
}

/// Decode the complete static ABI of each V3 pool event; reject unknown topics,
/// bad padding/widths, malformed positions and pending metadata before admission.
pub fn decode_log(value: &Value, registry: &PoolRegistry) -> Result<PoolLog> {
    registry.validate()?;
    let pool = fixed_hex(&value["address"], 20)?;
    address(&pool)?;
    if !pool.eq_ignore_ascii_case(&registry.pool) {
        return Err(AdapterError("event emitter is not the approved pool"));
    }
    let topics = value["topics"]
        .as_array()
        .ok_or(AdapterError("event topics missing"))?;
    if topics.is_empty() || topics.len() > 4 {
        return Err(AdapterError("invalid event topic count"));
    }
    let signature = fixed_hex(&topics[0], 32)?;
    let (topic_count, words) = match signature.as_str() {
        INITIALIZE => (1, 2),
        MINT => (4, 4),
        COLLECT => (4, 3),
        BURN => (4, 3),
        SWAP => (3, 5),
        FLASH => (3, 4),
        ORACLE_CAPACITY => (1, 2),
        PROTOCOL_FEE => (1, 4),
        COLLECT_PROTOCOL => (3, 2),
        _ => return Err(AdapterError("unsupported V3 event signature")),
    };
    if topics.len() != topic_count {
        return Err(AdapterError("event indexed argument count mismatch"));
    }
    let topics: Vec<Vec<u8>> = topics
        .iter()
        .map(|t| {
            let text = fixed_hex(t, 32)?;
            hex::decode(&text[2..]).map_err(|_| AdapterError("invalid event topic hex"))
        })
        .collect::<Result<_>>()?;
    let data = fixed_hex(&value["data"], words * 32)?;
    let bytes = hex::decode(&data[2..]).map_err(|_| AdapterError("invalid event data hex"))?;
    let word = |index: usize| &bytes[index * 32..(index + 1) * 32];
    let decoded = match signature.as_str() {
        INITIALIZE => PoolEvent::Initialize {
            sqrt_price_x96: price(word(0))?,
            tick: tick(word(1))?,
        },
        MINT => {
            let (tick_lower, tick_upper) = tick_range(&topics, registry.tick_spacing)?;
            PoolEvent::Mint {
                sender: event_address(word(0))?,
                owner: event_address(&topics[1])?,
                tick_lower,
                tick_upper,
                liquidity: uint(word(1), 16)?,
                amount0: uint(word(2), 32)?,
                amount1: uint(word(3), 32)?,
            }
        }
        COLLECT => {
            let (tick_lower, tick_upper) = tick_range(&topics, registry.tick_spacing)?;
            PoolEvent::Collect {
                owner: event_address(&topics[1])?,
                recipient: event_address(word(0))?,
                tick_lower,
                tick_upper,
                amount0: uint(word(1), 16)?,
                amount1: uint(word(2), 16)?,
            }
        }
        BURN => {
            let (tick_lower, tick_upper) = tick_range(&topics, registry.tick_spacing)?;
            PoolEvent::Burn {
                owner: event_address(&topics[1])?,
                tick_lower,
                tick_upper,
                liquidity: uint(word(0), 16)?,
                amount0: uint(word(1), 32)?,
                amount1: uint(word(2), 32)?,
            }
        }
        SWAP => PoolEvent::Swap {
            sender: event_address(&topics[1])?,
            recipient: event_address(&topics[2])?,
            amount0: int256(word(0)),
            amount1: int256(word(1)),
            sqrt_price_x96: price(word(2))?,
            liquidity: uint(word(3), 16)?,
            tick: tick(word(4))?,
        },
        FLASH => PoolEvent::Flash {
            sender: event_address(&topics[1])?,
            recipient: event_address(&topics[2])?,
            amount0: uint(word(0), 32)?,
            amount1: uint(word(1), 32)?,
            paid0: uint(word(2), 32)?,
            paid1: uint(word(3), 32)?,
        },
        ORACLE_CAPACITY => PoolEvent::IncreaseObservationCardinalityNext {
            old: u32word(word(0), 2)?,
            new: u32word(word(1), 2)?,
        },
        PROTOCOL_FEE => PoolEvent::SetFeeProtocol {
            token0_old: u32word(word(0), 1)?,
            token1_old: u32word(word(1), 1)?,
            token0_new: u32word(word(2), 1)?,
            token1_new: u32word(word(3), 1)?,
        },
        COLLECT_PROTOCOL => PoolEvent::CollectProtocol {
            sender: event_address(&topics[1])?,
            recipient: event_address(&topics[2])?,
            amount0: uint(word(0), 16)?,
            amount1: uint(word(1), 16)?,
        },
        _ => unreachable!("supported event checked above"),
    };
    Ok(PoolLog {
        pool,
        block_number: quantity(&value["blockNumber"])?,
        block_hash: fixed_hex(&value["blockHash"], 32)?,
        transaction_hash: fixed_hex(&value["transactionHash"], 32)?,
        transaction_index: quantity(&value["transactionIndex"])?,
        log_index: quantity(&value["logIndex"])?,
        removed: value["removed"]
            .as_bool()
            .ok_or(AdapterError("missing event removal status"))?,
        decoded,
    })
}
