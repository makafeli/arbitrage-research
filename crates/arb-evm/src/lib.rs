//! Bounded Base Uniswap V3 acquisition and candidate-only exact quote math.
pub mod math;
use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, Result, SnapshotQuality, StateContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const BASE_CHAIN_ID: u64 = 8453;
pub const UNISWAP_V3_FACTORY: &str = "0x33128a8fc17869897dce68ed026d694621f6fdfd";
pub const SOURCE_COMMIT: &str = "d0831dc6b8a318df3872b6d68f6de135c9f3ec29";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolRegistry {
    pub schema_version: u32,
    pub pool: String,
    pub token0: String,
    pub token1: String,
    pub fee: u32,
    pub tick_spacing: i32,
    /// SHA-256 over eth_getCode bytes. Must come from independently reviewed deployment qualification.
    pub pool_runtime_sha256: String,
    pub factory_runtime_sha256: String,
    pub qualification_reference: String,
    pub bitmap_word_min: i16,
    pub bitmap_word_max: i16,
}
impl PoolRegistry {
    pub fn validate(&self) -> Result<()> {
        address(&self.pool)?;
        address(&self.token0)?;
        address(&self.token1)?;
        if self.schema_version != 1
            || self.token0.to_lowercase() >= self.token1.to_lowercase()
            || self.pool.eq_ignore_ascii_case(&self.token0)
            || self.pool.eq_ignore_ascii_case(&self.token1)
            || self.fee == 0
            || self.fee >= 1_000_000
            || self.tick_spacing <= 0
            || self.tick_spacing > 32767
            || self.bitmap_word_min > self.bitmap_word_max
            || i32::from(self.bitmap_word_max) - i32::from(self.bitmap_word_min) > 15
            || self.qualification_reference.is_empty()
            || self.qualification_reference.len() > 256
            || !valid_sha(&self.pool_runtime_sha256)
            || !valid_sha(&self.factory_runtime_sha256)
        {
            return Err(AdapterError("invalid or unqualified Base pool registry"));
        }
        let low = i64::from(self.bitmap_word_min) * 256 * i64::from(self.tick_spacing);
        let high = (i64::from(self.bitmap_word_max) * 256 + 255) * i64::from(self.tick_spacing);
        if low < -887272 - 256 * i64::from(self.tick_spacing)
            || high > 887272 + 256 * i64::from(self.tick_spacing)
        {
            return Err(AdapterError("tick bitmap range outside protocol bounds"));
        }
        Ok(())
    }
}
fn valid_sha(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(|h| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()))
}
fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    let s = s
        .strip_prefix("0x")
        .ok_or(AdapterError("missing EVM hex prefix"))?;
    hex::decode(s).map_err(|_| AdapterError("malformed EVM hex"))
}
fn address(s: &str) -> Result<()> {
    let b = hex_bytes(s)?;
    if b.len() != 20 || b.iter().all(|x| *x == 0) {
        Err(AdapterError("invalid EVM address"))
    } else {
        Ok(())
    }
}
fn quantity(v: &Value) -> Result<u64> {
    let s = v
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .ok_or(AdapterError("invalid EVM quantity"))?;
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) {
        return Err(AdapterError("noncanonical EVM quantity"));
    }
    u64::from_str_radix(s, 16).map_err(|_| AdapterError("EVM quantity overflow"))
}
fn word(v: &Value, count: usize) -> Result<Vec<u8>> {
    let b = hex_bytes(v.as_str().ok_or(AdapterError("invalid ABI result"))?)?;
    if b.len() != 32 * count {
        Err(AdapterError("unexpected ABI result length"))
    } else {
        Ok(b)
    }
}
fn unsigned(b: &[u8], width: usize) -> Result<&[u8]> {
    if b.len() != 32 || b[..32 - width].iter().any(|x| *x != 0) {
        return Err(AdapterError("ABI unsigned integer out of bounds"));
    }
    Ok(&b[32 - width..])
}
fn u32word(b: &[u8], width: usize) -> Result<u32> {
    Ok(unsigned(b, width)?
        .iter()
        .fold(0_u32, |n, x| (n << 8) | u32::from(*x)))
}
fn i24word(b: &[u8]) -> Result<i32> {
    if b.len() != 32 {
        return Err(AdapterError("invalid int24 word"));
    }
    let negative = b[29] & 128 != 0;
    let sign = if negative { 255 } else { 0 };
    if b[..29].iter().any(|x| *x != sign) {
        return Err(AdapterError("ABI int24 sign extension mismatch"));
    }
    let n = (i32::from(b[29]) << 16) | (i32::from(b[30]) << 8) | i32::from(b[31]);
    Ok(if negative { n - (1 << 24) } else { n })
}
fn abi_address(v: &Value) -> Result<String> {
    let b = word(v, 1)?;
    let result = format!("0x{}", hex::encode(unsigned(&b, 20)?));
    address(&result)?;
    Ok(result)
}
fn signed_arg(value: i32) -> String {
    let mut b = [if value < 0 { 255 } else { 0 }; 32];
    b[28..].copy_from_slice(&value.to_be_bytes());
    hex::encode(b)
}
fn uint_arg(value: u32) -> String {
    let mut b = [0; 32];
    b[28..].copy_from_slice(&value.to_be_bytes());
    hex::encode(b)
}
fn address_arg(value: &str) -> String {
    format!("{:0>64}", value.trim_start_matches("0x").to_lowercase())
}
fn call(rpc: &mut impl ReadRpc, target: &str, data: String, context: &Value) -> Result<Value> {
    rpc.call(
        ReadMethod::EthCall,
        json!([{"to":target,"data":data},context]),
    )
}
fn code_matches(
    rpc: &mut impl ReadRpc,
    target: &str,
    expected: &str,
    context: &Value,
) -> Result<()> {
    let code = rpc.call(ReadMethod::EthGetCode, json!([target, context]))?;
    let bytes = hex_bytes(
        code.as_str()
            .ok_or(AdapterError("invalid runtime code result"))?,
    )?;
    if bytes.is_empty() || format!("sha256:{:x}", Sha256::digest(&bytes)) != expected.to_lowercase()
    {
        return Err(AdapterError("runtime code identity mismatch"));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TickState {
    pub liquidity_gross: String,
    pub liquidity_net: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolSnapshot {
    pub pool: String,
    pub context: StateContext,
    /// Exact Q64.96 integer represented as a hexadecimal string (never float).
    pub sqrt_price_x96_hex: String,
    pub tick: i32,
    pub liquidity: String,
    pub bitmap_words: BTreeMap<i16, String>,
    pub initialized_ticks: BTreeMap<i32, TickState>,
    pub quality: SnapshotQuality,
}
/// Maximum pools in one atomic acquisition attempt. Larger route universes must be
/// divided into independent attempts; snapshots from those attempts cannot be merged.
pub const MAX_CAPTURE_POOLS: usize = 8;

struct CaptureAnchor {
    state: StateContext,
    rpc_context: Value,
    number: u64,
    block_hash: String,
}

fn capture_anchor(rpc: &mut impl ReadRpc) -> Result<CaptureAnchor> {
    let block = rpc.call(ReadMethod::EthGetBlockByNumber, json!(["finalized", false]))?;
    let number = quantity(&block["number"])?;
    let block_hash = block["hash"]
        .as_str()
        .ok_or(AdapterError("missing block hash"))?
        .to_string();
    let parent_hash = block["parentHash"]
        .as_str()
        .ok_or(AdapterError("missing parent hash"))?
        .to_string();
    if hex_bytes(&block_hash)?.len() != 32 || hex_bytes(&parent_hash)?.len() != 32 {
        return Err(AdapterError("invalid block hashes"));
    }
    let timestamp = quantity(&block["timestamp"])?;
    let context = json!({"blockHash":block_hash,"requireCanonical":true});
    Ok(CaptureAnchor {
        state: StateContext::Evm {
            block_number: number,
            block_hash: block_hash.clone(),
            parent_hash,
            block_timestamp_seconds: timestamp,
            finality: "finalized".into(),
        },
        rpc_context: context,
        number,
        block_hash,
    })
}

fn verify_canonical(rpc: &mut impl ReadRpc, anchor: &CaptureAnchor) -> Result<()> {
    let number = anchor.number;
    let block_hash = &anchor.block_hash;
    let canonical = rpc.call(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{number:x}"), false]),
    )?;
    if canonical["hash"].as_str() != Some(block_hash.as_str()) {
        return Err(AdapterError("Base block invalidated during acquisition"));
    }
    Ok(())
}

/// Every eth_call and eth_getCode is pinned by block hash with requireCanonical=true.
/// No fallback to latest or block-number state is permitted when EIP-1898 is unsupported.
/// This legacy entry point retains the original single-pool RPC transcript ordering.
pub fn capture_pool(
    rpc: &mut impl ReadRpc,
    registry: &PoolRegistry,
    observed_at_ms: u64,
) -> Result<PoolSnapshot> {
    registry.validate()?;
    if quantity(&rpc.call(ReadMethod::EthChainId, json!([]))?)? != BASE_CHAIN_ID {
        return Err(AdapterError("wrong EVM chain"));
    }
    let anchor = capture_anchor(rpc)?;
    let snapshot = capture_pool_at(rpc, registry, observed_at_ms, &anchor)?;
    verify_canonical(rpc, &anchor)?;
    Ok(snapshot)
}

/// Capture 1..=8 distinct Base pools against one finalized block hash. Every
/// registry is validated before I/O, and the complete batch is discarded if any
/// pool fails or the anchor is no longer canonical after the last pool read.
/// Output order is registry order. Provider errors never cause a latest fallback.
pub fn capture_pools(
    rpc: &mut impl ReadRpc,
    registries: &[PoolRegistry],
    observed_at_ms: u64,
) -> Result<Vec<PoolSnapshot>> {
    if registries.is_empty() || registries.len() > MAX_CAPTURE_POOLS {
        return Err(AdapterError("Base capture batch requires 1..=8 pools"));
    }
    let mut pools = BTreeSet::new();
    for registry in registries {
        registry.validate()?;
        if !pools.insert(registry.pool.to_lowercase()) {
            return Err(AdapterError("duplicate Base capture pool"));
        }
    }
    if quantity(&rpc.call(ReadMethod::EthChainId, json!([]))?)? != BASE_CHAIN_ID {
        return Err(AdapterError("wrong EVM chain"));
    }
    let anchor = capture_anchor(rpc)?;
    let snapshots = registries
        .iter()
        .map(|registry| capture_pool_at(rpc, registry, observed_at_ms, &anchor))
        .collect::<Result<Vec<_>>>()?;
    verify_canonical(rpc, &anchor)?;
    Ok(snapshots)
}

fn capture_pool_at(
    rpc: &mut impl ReadRpc,
    registry: &PoolRegistry,
    observed_at_ms: u64,
    anchor: &CaptureAnchor,
) -> Result<PoolSnapshot> {
    let context = &anchor.rpc_context;
    code_matches(
        rpc,
        UNISWAP_V3_FACTORY,
        &registry.factory_runtime_sha256,
        context,
    )?;
    code_matches(rpc, &registry.pool, &registry.pool_runtime_sha256, context)?;
    let factory = abi_address(&call(rpc, &registry.pool, "0xc45a0155".into(), context)?)?;
    if factory != UNISWAP_V3_FACTORY {
        return Err(AdapterError("unexpected pool factory"));
    }
    for (selector, expected) in [
        ("0x0dfe1681", &registry.token0),
        ("0xd21220a7", &registry.token1),
    ] {
        if abi_address(&call(rpc, &registry.pool, selector.into(), context)?)?
            != expected.to_lowercase()
        {
            return Err(AdapterError("pool token identity mismatch"));
        }
    }
    let fee = word(&call(rpc, &registry.pool, "0xddca3f43".into(), context)?, 1)?;
    if u32word(&fee, 3)? != registry.fee {
        return Err(AdapterError("pool fee mismatch"));
    }
    let spacing = word(&call(rpc, &registry.pool, "0xd0c93a7c".into(), context)?, 1)?;
    if i24word(&spacing)? != registry.tick_spacing {
        return Err(AdapterError("pool tick spacing mismatch"));
    }
    let get_pool = format!(
        "0x1698ee82{}{}{}",
        address_arg(&registry.token0),
        address_arg(&registry.token1),
        uint_arg(registry.fee)
    );
    if abi_address(&call(rpc, UNISWAP_V3_FACTORY, get_pool, context)?)?
        != registry.pool.to_lowercase()
    {
        return Err(AdapterError("pool not registered in verified factory"));
    }
    let slot0 = word(&call(rpc, &registry.pool, "0x3850c7bd".into(), context)?, 7)?;
    let sqrt_price_x96_hex = format!("0x{}", hex::encode(unsigned(&slot0[..32], 20)?));
    let tick = i24word(&slot0[32..64])?;
    if !(-887272..=887272).contains(&tick) || slot0[..32].iter().all(|x| *x == 0) {
        return Err(AdapterError("uninitialized or invalid pool price"));
    }
    for i in [2, 3, 4] {
        unsigned(&slot0[i * 32..(i + 1) * 32], 2)?;
    }
    unsigned(&slot0[160..192], 1)?;
    if u32word(&slot0[192..224], 1)? != 1 {
        return Err(AdapterError("pool is locked"));
    }
    let liq = word(&call(rpc, &registry.pool, "0x1a686502".into(), context)?, 1)?;
    let liquidity = u128::from_be_bytes(
        unsigned(&liq, 16)?
            .try_into()
            .map_err(|_| AdapterError("invalid liquidity"))?,
    )
    .to_string();
    let mut bitmap_words = BTreeMap::new();
    let mut initialized_ticks = BTreeMap::new();
    for position in registry.bitmap_word_min..=registry.bitmap_word_max {
        let bitmap = word(
            &call(
                rpc,
                &registry.pool,
                format!("0x5339c296{}", signed_arg(i32::from(position))),
                context,
            )?,
            1,
        )?;
        bitmap_words.insert(position, format!("0x{}", hex::encode(&bitmap)));
        for bit in 0..256_i32 {
            if bitmap[31 - (bit as usize / 8)] & (1 << (bit % 8)) == 0 {
                continue;
            }
            let index = (i32::from(position) * 256 + bit) * registry.tick_spacing;
            if !(-887272..=887272).contains(&index) {
                return Err(AdapterError("bitmap initializes out-of-bounds tick"));
            }
            let state = word(
                &call(
                    rpc,
                    &registry.pool,
                    format!("0xf30dba93{}", signed_arg(index)),
                    context,
                )?,
                8,
            )?;
            let gross = u128::from_be_bytes(
                unsigned(&state[..32], 16)?
                    .try_into()
                    .map_err(|_| AdapterError("invalid tick gross liquidity"))?,
            );
            let net_word = &state[32..64];
            let negative = net_word[16] & 128 != 0;
            if net_word[..16]
                .iter()
                .any(|x| *x != if negative { 255 } else { 0 })
            {
                return Err(AdapterError("invalid tick net liquidity"));
            }
            let net = i128::from_be_bytes(
                net_word[16..]
                    .try_into()
                    .map_err(|_| AdapterError("invalid tick net liquidity"))?,
            );
            if gross == 0 || net.unsigned_abs() > gross || u32word(&state[224..256], 1)? != 1 {
                return Err(AdapterError("bitmap/tick initialization mismatch"));
            }
            initialized_ticks.insert(
                index,
                TickState {
                    liquidity_gross: gross.to_string(),
                    liquidity_net: net.to_string(),
                },
            );
        }
    }
    Ok(PoolSnapshot {
        pool: registry.pool.to_lowercase(),
        context: anchor.state.clone(),
        sqrt_price_x96_hex,
        tick,
        liquidity,
        bitmap_words,
        initialized_ticks,
        quality: SnapshotQuality {
            coherent: true,
            complete_for_quote: false,
            quote_implementation_qualified: false,
            observed_at_ms,
            max_age_ms: 2000,
            reasons: vec![
                "bounded-tick-window-only".into(),
                "token-behavior-unqualified".into(),
                "quote-math-unqualified".into(),
            ],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_abi_and_overflow_are_checked() {
        let n = hex::decode(signed_arg(-10)).unwrap();
        assert_eq!(i24word(&n).unwrap(), -10);
        let mut bad = n;
        bad[0] = 0;
        assert!(i24word(&bad).is_err());
        assert!(unsigned(&[255; 32], 16).is_err());
    }
    #[test]
    fn rejects_malformed_abi() {
        assert!(word(&json!("0x00"), 1).is_err());
        assert!(abi_address(&json!(format!("0x{}", "0".repeat(64)))).is_err());
        assert!(quantity(&json!("0x00")).is_err());
    }
    #[test]
    fn registry_rejects_fake_identity() {
        let r = PoolRegistry {
            schema_version: 1,
            pool: "fixture:pool".into(),
            token0: "fixture:a".into(),
            token1: "fixture:b".into(),
            fee: 500,
            tick_spacing: 10,
            pool_runtime_sha256: String::new(),
            factory_runtime_sha256: String::new(),
            qualification_reference: String::new(),
            bitmap_word_min: 0,
            bitmap_word_max: 0,
        };
        assert!(r.validate().is_err());
    }
    struct WrongChain;
    impl ReadRpc for WrongChain {
        fn call(&mut self, _: ReadMethod, _: Value) -> Result<Value> {
            Ok(json!("0x1"))
        }
    }
    #[test]
    fn wrong_chain_fails_before_pool_reads() {
        let r = PoolRegistry {
            schema_version: 1,
            pool: format!("0x{}", "3".repeat(40)),
            token0: format!("0x{}", "1".repeat(40)),
            token1: format!("0x{}", "2".repeat(40)),
            fee: 500,
            tick_spacing: 10,
            pool_runtime_sha256: format!("sha256:{}", "a".repeat(64)),
            factory_runtime_sha256: format!("sha256:{}", "b".repeat(64)),
            qualification_reference: "manual-test".into(),
            bitmap_word_min: 0,
            bitmap_word_max: 0,
        };
        assert_eq!(
            capture_pool(&mut WrongChain, &r, 100).unwrap_err().0,
            "wrong EVM chain"
        );
    }
}
