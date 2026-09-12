//! Synthetic differential vectors executed by the pinned official Uniswap SDK.
//! No fixture is presented as an RPC capture or a qualified deployment.
use arb_adapter_api::{SnapshotQuality, StateContext};
use arb_domain::AtomicAmount;
use arb_evm::{PoolRegistry, PoolSnapshot, TickState, math::quote_exact_input_math};
use primitive_types::U256;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Reference {
    schema_version: u32,
    source_commit: String,
    fixture_origin: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    tick: i32,
    sqrt_price: String,
    liquidity: String,
    tick_spacing: i32,
    fee_pips: u32,
    words: Vec<Word>,
    ticks: Vec<Tick>,
    amount_in: AtomicAmount,
    zero_for_one: bool,
    expected_rejection: Option<String>,
    expected: Expected,
}
#[derive(Deserialize)]
struct Word {
    index: i16,
    bitmap: String,
}
#[derive(Deserialize)]
struct Tick {
    index: i32,
    liquidity_gross: String,
    liquidity_net: String,
}
#[derive(Deserialize)]
struct Expected {
    amount_in: AtomicAmount,
    amount_out: AtomicAmount,
    fee: AtomicAmount,
    ending_sqrt_price: AtomicAmount,
    initialized_ticks_crossed: u32,
}
fn integer(value: &str) -> U256 {
    if let Some(hex) = value.strip_prefix("0x") {
        U256::from_str_radix(hex, 16).unwrap()
    } else {
        U256::from_dec_str(value).unwrap()
    }
}

#[test]
fn exact_quotes_match_pinned_official_sdk_differential_vectors() {
    let reference: Reference = serde_json::from_str(include_str!("reference/golden.json")).unwrap();
    assert_eq!(reference.schema_version, 1);
    assert_eq!(reference.source_commit, "4e16fe8e56c8c26541545f138c89133794c7ce72");
    assert_eq!(reference.fixture_origin, "synthetic");
    assert!(!reference.cases.is_empty(), "reference vectors must not be empty");
    for case in reference.cases {
        let minimum = case.words.iter().map(|word| word.index).min().unwrap();
        let maximum = case.words.iter().map(|word| word.index).max().unwrap();
        let registry = PoolRegistry {
            schema_version: 1,
            pool: format!("0x{:040x}", 3),
            token0: format!("0x{:040x}", 1),
            token1: format!("0x{:040x}", 2),
            fee: case.fee_pips,
            tick_spacing: case.tick_spacing,
            pool_runtime_sha256: format!("sha256:{}", "0".repeat(64)),
            factory_runtime_sha256: format!("sha256:{}", "0".repeat(64)),
            qualification_reference: "official SDK synthetic differential fixture".into(),
            bitmap_word_min: minimum,
            bitmap_word_max: maximum,
        };
        let words: BTreeMap<_, _> = case.words.iter()
            .map(|word| (word.index, format!("0x{:064x}", integer(&word.bitmap))))
            .collect();
        assert_eq!(words.len(), case.words.len(), "duplicate reference word");
        let ticks: BTreeMap<_, _> = case.ticks.iter().map(|tick| (tick.index, TickState {
            liquidity_gross: tick.liquidity_gross.clone(),
            liquidity_net: tick.liquidity_net.clone(),
        })).collect();
        assert_eq!(ticks.len(), case.ticks.len(), "duplicate reference tick");
        let snapshot = PoolSnapshot {
            pool: registry.pool.clone(),
            context: StateContext::Evm {
                block_number: 0,
                block_hash: format!("0x{}", "0".repeat(64)),
                parent_hash: format!("0x{}", "0".repeat(64)),
                block_timestamp_seconds: 0,
                finality: "finalized".into(),
            },
            sqrt_price_x96_hex: format!("0x{:040x}", integer(&case.sqrt_price)),
            tick: case.tick,
            liquidity: case.liquidity,
            bitmap_words: words,
            initialized_ticks: ticks,
            quality: SnapshotQuality {
                coherent: false,
                complete_for_quote: false,
                quote_implementation_qualified: false,
                observed_at_ms: 0,
                max_age_ms: 2000,
                reasons: vec!["synthetic official-SDK differential fixture".into()],
            },
        };
        let result = quote_exact_input_math(&snapshot, &registry, case.amount_in, case.zero_for_one);
        if let Some(rejection) = case.expected_rejection {
            assert_eq!(rejection, "zero-output", "{}: unknown rejection", case.name);
            assert!(case.expected.amount_out.is_zero(), "{}: reference output", case.name);
            assert_eq!(case.expected.fee, case.expected.amount_in, "{}: input consumed as fee", case.name);
            assert_eq!(result.unwrap_err().0, "Uniswap V3 quote output rounds to zero", "{}", case.name);
            continue;
        }
        let actual = result.unwrap_or_else(|error| panic!("{}: {}", case.name, error.0));
        assert_eq!(actual.amount_in, case.expected.amount_in, "{}: full input", case.name);
        assert_eq!(actual.amount_out, case.expected.amount_out, "{}: output", case.name);
        assert_eq!(actual.pool_fee_in_input_asset, case.expected.fee, "{}: fee included", case.name);
        assert_eq!(actual.ending_sqrt_price_x96, case.expected.ending_sqrt_price, "{}: price", case.name);
        assert_eq!(actual.ticks_crossed, case.expected.initialized_ticks_crossed, "{}: crossings", case.name);
        assert_eq!(actual.evidence, "CANDIDATE");
        assert!(!snapshot.quality.quote_implementation_qualified);
    }
}
