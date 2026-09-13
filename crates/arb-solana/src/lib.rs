//! Orca Whirlpool legacy SPL-token and fixed tick-array decoder. No transaction construction.
pub mod math;

use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, Result, SnapshotQuality, StateContext};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
pub const SOURCE_COMMIT: &str = "408c945fef4c49ab70def4303377cfaf8f0f3c99";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolRegistry {
    pub schema_version: u32,
    pub expected_genesis_hash: String,
    pub pool: String,
    pub whirlpools_config: String,
    pub program_data: String,
    pub program_data_sha256: String,
    pub mint_a: String,
    pub mint_b: String,
    pub decimals_a: u8,
    pub decimals_b: u8,
    pub vault_a: String,
    pub vault_b: String,
    pub tick_arrays: Vec<String>,
    pub qualification_reference: String,
}
impl PoolRegistry {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || self.tick_arrays.is_empty()
            || self.tick_arrays.len() > 16
            || self.qualification_reference.is_empty()
            || self.qualification_reference.len() > 256
            || !self
                .program_data_sha256
                .strip_prefix("sha256:")
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(AdapterError("invalid or unqualified Solana registry"));
        }
        key(&self.expected_genesis_hash)?;
        key(&self.whirlpools_config)?;
        let addresses = self.addresses();
        let mut seen = BTreeSet::new();
        for address in addresses {
            key(&address)?;
            if !seen.insert(address) {
                return Err(AdapterError("duplicate Solana registry account"));
            }
        }
        if self.mint_a == self.mint_b {
            return Err(AdapterError("identical pool mints"));
        }
        Ok(())
    }
    pub fn addresses(&self) -> Vec<String> {
        let mut a = vec![
            self.pool.clone(),
            self.vault_a.clone(),
            self.vault_b.clone(),
            self.mint_a.clone(),
            self.mint_b.clone(),
            WHIRLPOOL_PROGRAM.into(),
            self.program_data.clone(),
        ];
        a.extend(self.tick_arrays.iter().cloned());
        a
    }
}
fn key(s: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|_| AdapterError("invalid base58 key"))?;
    let b: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AdapterError("invalid Solana key length"))?;
    if b == [0; 32] {
        return Err(AdapterError("zero Solana key"));
    }
    Ok(b)
}
fn pubkey(bytes: &[u8]) -> String {
    bs58::encode(bytes).into_string()
}
fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}
fn u128_at(data: &[u8], offset: usize) -> u128 {
    let mut b = [0; 16];
    b.copy_from_slice(&data[offset..offset + 16]);
    u128::from_le_bytes(b)
}
fn i32_at(data: &[u8], offset: usize) -> i32 {
    let mut b = [0; 4];
    b.copy_from_slice(&data[offset..offset + 4]);
    i32::from_le_bytes(b)
}
fn discriminator(name: &str) -> [u8; 8] {
    let mut b = [0; 8];
    b.copy_from_slice(&Sha256::digest(format!("account:{name}"))[..8]);
    b
}
fn account_data(value: &Value, owner: &str, executable: bool) -> Result<Vec<u8>> {
    if value["owner"] != owner || value["executable"] != executable {
        return Err(AdapterError("Solana account owner or executable mismatch"));
    }
    let array = value["data"]
        .as_array()
        .ok_or(AdapterError("missing account bytes"))?;
    if array.len() != 2 || array[1] != "base64" {
        return Err(AdapterError("unsupported account encoding"));
    }
    let data = STANDARD
        .decode(
            array[0]
                .as_str()
                .ok_or(AdapterError("invalid account bytes"))?,
        )
        .map_err(|_| AdapterError("invalid base64 account data"))?;
    if data.len() > 8 * 1024 * 1024 {
        return Err(AdapterError("account byte limit exceeded"));
    }
    Ok(data)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhirlpoolState {
    pub whirlpools_config: String,
    pub tick_spacing: u16,
    pub fee_tier_index: u16,
    pub fee_rate: u16,
    pub protocol_fee_rate: u16,
    pub liquidity: String,
    pub sqrt_price_x64: String,
    pub tick_current_index: i32,
    pub mint_a: String,
    pub mint_b: String,
    pub vault_a: String,
    pub vault_b: String,
}
pub fn decode_whirlpool(data: &[u8]) -> Result<WhirlpoolState> {
    if data.len() != 653 || data[..8] != discriminator("Whirlpool") {
        return Err(AdapterError(
            "unsupported Whirlpool layout or discriminator",
        ));
    }
    let tick_spacing = u16_at(data, 41);
    let sqrt_price = u128_at(data, 65);
    let tick = i32_at(data, 81);
    if tick_spacing == 0 || sqrt_price == 0 || !(-443636..=443636).contains(&tick) {
        return Err(AdapterError("invalid Whirlpool tick or price"));
    }
    Ok(WhirlpoolState {
        whirlpools_config: pubkey(&data[8..40]),
        tick_spacing,
        fee_tier_index: u16_at(data, 43),
        fee_rate: u16_at(data, 45),
        protocol_fee_rate: u16_at(data, 47),
        liquidity: u128_at(data, 49).to_string(),
        sqrt_price_x64: sqrt_price.to_string(),
        tick_current_index: tick,
        mint_a: pubkey(&data[101..133]),
        vault_a: pubkey(&data[133..165]),
        mint_b: pubkey(&data[181..213]),
        vault_b: pubkey(&data[213..245]),
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TickState {
    pub index: i32,
    pub liquidity_gross: String,
    pub liquidity_net: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixedTickArray {
    pub address: String,
    pub start_tick_index: i32,
    pub initialized_ticks: Vec<TickState>,
}
pub fn decode_fixed_tick_array(data: &[u8], pool: &str, spacing: u16) -> Result<FixedTickArray> {
    if data.len() != 9988
        || data[..8] != discriminator("TickArray")
        || pubkey(&data[9956..9988]) != pool
    {
        return Err(AdapterError(
            "unsupported tick array layout or pool identity",
        ));
    }
    let start = i32_at(data, 8);
    let span = i32::from(spacing) * 88;
    if spacing == 0
        || start % span != 0
        || i64::from(start) < -443636_i64 - i64::from(span)
        || start > 443636
    {
        return Err(AdapterError("invalid tick array start"));
    }
    let mut ticks = Vec::new();
    for i in 0..88 {
        let offset = 12 + i * 113;
        let initialized = data[offset];
        if initialized > 1 {
            return Err(AdapterError("invalid tick initialized flag"));
        }
        let gross = u128_at(data, offset + 17);
        let mut net_bytes = [0; 16];
        net_bytes.copy_from_slice(&data[offset + 1..offset + 17]);
        let net = i128::from_le_bytes(net_bytes);
        if initialized == 1 {
            let index = start + i as i32 * i32::from(spacing);
            if gross == 0 || net.unsigned_abs() > gross || !(-443636..=443636).contains(&index) {
                return Err(AdapterError("invalid initialized tick"));
            }
            ticks.push(TickState {
                index,
                liquidity_gross: gross.to_string(),
                liquidity_net: net.to_string(),
            });
        } else if gross != 0 || net != 0 {
            return Err(AdapterError("uninitialized tick has liquidity"));
        }
    }
    Ok(FixedTickArray {
        address: String::new(),
        start_tick_index: start,
        initialized_ticks: ticks,
    })
}
fn validate_mint(data: &[u8], decimals: u8) -> Result<()> {
    if data.len() != 82
        || data[44] != decimals
        || data[45] != 1
        || !matches!(&data[..4], [0, 0, 0, 0] | [1, 0, 0, 0])
        || !matches!(&data[46..50], [0, 0, 0, 0] | [1, 0, 0, 0])
    {
        return Err(AdapterError(
            "unsupported token mint, decimals or extension",
        ));
    }
    Ok(())
}
fn validate_vault(data: &[u8], mint: &str, owner: &str) -> Result<()> {
    if data.len() != 165
        || pubkey(&data[..32]) != mint
        || pubkey(&data[32..64]) != owner
        || data[108] != 1
        || data[72..76] != [0; 4]
        || data[129..133] != [0; 4]
        || !matches!(&data[109..113], [0, 0, 0, 0] | [1, 0, 0, 0])
    {
        return Err(AdapterError(
            "unsupported, frozen or mismatched token vault",
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolSnapshot {
    pub pool: String,
    pub context: StateContext,
    pub state: WhirlpoolState,
    pub tick_arrays: Vec<FixedTickArray>,
    pub account_write_provenance: String,
    pub quality: SnapshotQuality,
}
/// All required accounts are returned in one finalized getMultipleAccounts response.
/// Its context slot alone does not independently establish bank/stream coherence.
pub fn capture_pool(
    rpc: &mut impl ReadRpc,
    registry: &PoolRegistry,
    observed_at_ms: u64,
) -> Result<PoolSnapshot> {
    registry.validate()?;
    if rpc.call(ReadMethod::GetGenesisHash, json!([]))? != registry.expected_genesis_hash {
        return Err(AdapterError("Solana genesis identity mismatch"));
    }
    let addresses = registry.addresses();
    let response = rpc.call(
        ReadMethod::GetMultipleAccounts,
        json!([addresses,{"encoding":"base64","commitment":"finalized"}]),
    )?;
    let slot = response["context"]["slot"]
        .as_u64()
        .ok_or(AdapterError("missing Solana context slot"))?;
    let accounts = response["value"]
        .as_array()
        .ok_or(AdapterError("missing Solana accounts"))?;
    if accounts.len() != addresses.len() || accounts.iter().any(Value::is_null) {
        return Err(AdapterError("missing required Solana account"));
    }
    decode_pool_accounts(
        registry,
        &accounts.iter().collect::<Vec<_>>(),
        slot,
        observed_at_ms,
    )
}

/// Maximum pools and unique accounts admitted into one response. Chunking a
/// larger request would lose the shared response context and is not permitted.
pub const MAX_CAPTURE_POOLS: usize = 8;
pub const MAX_CAPTURE_ACCOUNTS: usize = 100;

/// Capture distinct pools using exactly one finalized getMultipleAccounts result.
/// Shared program, mint and other accounts are requested only once, in order of
/// first appearance. Internal references select each pool's accounts; the actual
/// RPC response remains intact for recording/replay. A context slot still does
/// not establish independent provider bank coherence, so quality stays unqualified.
pub fn capture_pools(
    rpc: &mut impl ReadRpc,
    registries: &[PoolRegistry],
    observed_at_ms: u64,
) -> Result<Vec<PoolSnapshot>> {
    if registries.is_empty() || registries.len() > MAX_CAPTURE_POOLS {
        return Err(AdapterError("Solana capture batch requires 1..=8 pools"));
    }
    let genesis = &registries[0].expected_genesis_hash;
    let mut pools = BTreeSet::new();
    let mut addresses = Vec::new();
    let mut positions = BTreeMap::new();
    let mut pool_positions = Vec::with_capacity(registries.len());
    for registry in registries {
        registry.validate()?;
        if &registry.expected_genesis_hash != genesis {
            return Err(AdapterError(
                "mixed Solana genesis identities in capture batch",
            ));
        }
        if !pools.insert(&registry.pool) {
            return Err(AdapterError("duplicate Solana capture pool"));
        }
        let mut selected = Vec::new();
        for address in registry.addresses() {
            let index = *positions.entry(address.clone()).or_insert_with(|| {
                let index = addresses.len();
                addresses.push(address);
                index
            });
            selected.push(index);
        }
        pool_positions.push(selected);
    }
    if addresses.len() > MAX_CAPTURE_ACCOUNTS {
        return Err(AdapterError(
            "Solana capture batch exceeds 100 unique accounts",
        ));
    }
    if rpc.call(ReadMethod::GetGenesisHash, json!([]))?.as_str() != Some(genesis.as_str()) {
        return Err(AdapterError("Solana genesis identity mismatch"));
    }
    let response = rpc.call(
        ReadMethod::GetMultipleAccounts,
        json!([addresses,{"encoding":"base64","commitment":"finalized"}]),
    )?;
    let slot = response["context"]["slot"]
        .as_u64()
        .ok_or(AdapterError("missing Solana context slot"))?;
    let accounts = response["value"]
        .as_array()
        .ok_or(AdapterError("missing Solana accounts"))?;
    if accounts.len() != addresses.len() || accounts.iter().any(Value::is_null) {
        return Err(AdapterError("missing required Solana account"));
    }
    registries
        .iter()
        .zip(pool_positions)
        .map(|(registry, positions)| {
            let selected = positions
                .iter()
                .map(|index| &accounts[*index])
                .collect::<Vec<_>>();
            decode_pool_accounts(registry, &selected, slot, observed_at_ms)
        })
        .collect()
}

fn decode_pool_accounts(
    registry: &PoolRegistry,
    accounts: &[&Value],
    slot: u64,
    observed_at_ms: u64,
) -> Result<PoolSnapshot> {
    let program = account_data(accounts[5], UPGRADEABLE_LOADER, true)?;
    if program.len() != 36
        || program[..4] != [2, 0, 0, 0]
        || pubkey(&program[4..36]) != registry.program_data
    {
        return Err(AdapterError("Whirlpool program data identity mismatch"));
    }
    let program_data = account_data(accounts[6], UPGRADEABLE_LOADER, false)?;
    if program_data.len() < 45
        || program_data[..4] != [3, 0, 0, 0]
        || format!("sha256:{:x}", Sha256::digest(&program_data))
            != registry.program_data_sha256.to_lowercase()
    {
        return Err(AdapterError("Whirlpool program data hash mismatch"));
    }
    let pool = decode_whirlpool(&account_data(accounts[0], WHIRLPOOL_PROGRAM, false)?)?;
    if pool.whirlpools_config != registry.whirlpools_config
        || pool.mint_a != registry.mint_a
        || pool.mint_b != registry.mint_b
        || pool.vault_a != registry.vault_a
        || pool.vault_b != registry.vault_b
    {
        return Err(AdapterError("Whirlpool registry identity mismatch"));
    }
    for (index, mint) in [(1, &registry.mint_a), (2, &registry.mint_b)] {
        validate_vault(
            &account_data(accounts[index], TOKEN_PROGRAM, false)?,
            mint,
            &registry.pool,
        )?;
    }
    for (index, decimals) in [(3, registry.decimals_a), (4, registry.decimals_b)] {
        validate_mint(
            &account_data(accounts[index], TOKEN_PROGRAM, false)?,
            decimals,
        )?;
    }
    let mut tick_arrays = Vec::new();
    let mut starts = BTreeSet::new();
    for (index, address) in registry.tick_arrays.iter().enumerate() {
        let mut array = decode_fixed_tick_array(
            &account_data(accounts[index + 7], WHIRLPOOL_PROGRAM, false)?,
            &registry.pool,
            pool.tick_spacing,
        )?;
        if !starts.insert(array.start_tick_index) {
            return Err(AdapterError("duplicate tick array range"));
        }
        array.address = address.clone();
        tick_arrays.push(array);
    }
    tick_arrays.sort_by_key(|a| a.start_tick_index);
    if tick_arrays
        .windows(2)
        .any(|w| w[1].start_tick_index - w[0].start_tick_index != i32::from(pool.tick_spacing) * 88)
    {
        return Err(AdapterError("missing intermediate tick array"));
    }
    Ok(PoolSnapshot {
        pool: registry.pool.clone(),
        context: StateContext::Solana {
            slot,
            genesis_hash: registry.expected_genesis_hash.clone(),
            commitment: "finalized".into(),
            account_context: "single-getMultipleAccounts-response".into(),
        },
        state: pool,
        tick_arrays,
        account_write_provenance: "unavailable-in-poll-response".into(),
        quality: SnapshotQuality {
            coherent: false,
            complete_for_quote: false,
            quote_implementation_qualified: false,
            observed_at_ms,
            max_age_ms: 2000,
            reasons: vec![
                "provider-bank-context-unqualified".into(),
                "bounded-tick-window-only".into(),
                "quote-math-unqualified".into(),
            ],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pool_bytes() -> Vec<u8> {
        let mut d = vec![0; 653];
        d[..8].copy_from_slice(&discriminator("Whirlpool"));
        d[41..43].copy_from_slice(&64_u16.to_le_bytes());
        d[43..45].copy_from_slice(&64_u16.to_le_bytes());
        d[49..65].copy_from_slice(&1000_u128.to_le_bytes());
        d[65..81].copy_from_slice(&(1_u128 << 64).to_le_bytes());
        d[81..85].copy_from_slice(&(-2_i32).to_le_bytes());
        for (n, start) in [8, 101, 133, 181, 213].iter().enumerate() {
            d[*start..*start + 32].fill(n as u8 + 1);
        }
        d
    }
    #[test]
    fn decodes_pinned_fixed_layout_exactly() {
        let p = decode_whirlpool(&pool_bytes()).unwrap();
        assert_eq!(p.tick_spacing, 64);
        assert_eq!(p.sqrt_price_x64, "18446744073709551616");
        assert_eq!(p.liquidity, "1000");
        assert_eq!(p.tick_current_index, -2);
        assert_ne!(p.mint_a, p.mint_b);
    }
    #[test]
    fn rejects_truncated_and_unknown_layouts() {
        let mut d = pool_bytes();
        d.pop();
        assert!(decode_whirlpool(&d).is_err());
        let mut d = pool_bytes();
        d[0] ^= 1;
        assert!(decode_whirlpool(&d).is_err());
    }
    #[test]
    fn rejects_owner_encoding_and_token_extensions() {
        assert!(
            account_data(
                &json!({"owner":"attacker","executable":false,"data":["","base64"]}),
                WHIRLPOOL_PROGRAM,
                false
            )
            .is_err()
        );
        assert!(validate_mint(&[0; 83], 6).is_err());
        assert!(validate_vault(&[0; 165], "a", "b").is_err());
    }
    #[test]
    fn fixed_tick_array_has_parent_and_boolean_checks() {
        let pool = pubkey(&[1; 32]);
        let mut d = vec![0; 9988];
        d[..8].copy_from_slice(&discriminator("TickArray"));
        d[9956..].fill(1);
        assert!(decode_fixed_tick_array(&d, &pool, 64).is_ok());
        assert!(decode_fixed_tick_array(&d, &pubkey(&[2; 32]), 64).is_err());
        d[12] = 2;
        assert!(decode_fixed_tick_array(&d, &pool, 64).is_err());
    }
}
