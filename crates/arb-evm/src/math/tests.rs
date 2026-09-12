use super::*;
use crate::TickState;
use arb_adapter_api::SnapshotQuality;

fn fixture(tick: i32, liquidity: u128, minimum: i16, maximum: i16) -> (PoolSnapshot, PoolRegistry) {
    let registry = PoolRegistry {
        schema_version: 1,
        pool: format!("0x{:040x}", 3),
        token0: format!("0x{:040x}", 1),
        token1: format!("0x{:040x}", 2),
        fee: 3000,
        tick_spacing: 10,
        pool_runtime_sha256: format!("sha256:{}", "0".repeat(64)),
        factory_runtime_sha256: format!("sha256:{}", "0".repeat(64)),
        qualification_reference: "synthetic math fixture; not deployment qualification".into(),
        bitmap_word_min: minimum,
        bitmap_word_max: maximum,
    };
    let snapshot = PoolSnapshot {
        pool: registry.pool.clone(),
        context: StateContext::Evm {
            block_number: 0,
            block_hash: format!("0x{}", "0".repeat(64)),
            parent_hash: format!("0x{}", "0".repeat(64)),
            block_timestamp_seconds: 0,
            finality: "finalized".into(),
        },
        sqrt_price_x96_hex: format!("0x{:040x}", sqrt_at_tick(tick).unwrap()),
        tick,
        liquidity: liquidity.to_string(),
        bitmap_words: (minimum..=maximum)
            .map(|index| (index, format!("0x{:064x}", 0)))
            .collect(),
        initialized_ticks: BTreeMap::new(),
        quality: SnapshotQuality {
            coherent: false,
            complete_for_quote: false,
            quote_implementation_qualified: false,
            observed_at_ms: 0,
            max_age_ms: 2000,
            reasons: vec!["synthetic mathematical fixture".into()],
        },
    };
    (snapshot, registry)
}
fn initialize(
    snapshot: &mut PoolSnapshot,
    registry: &PoolRegistry,
    tick: i32,
    gross: u128,
    net: i128,
) {
    let compressed = tick.div_euclid(registry.tick_spacing);
    let index = compressed.div_euclid(256) as i16;
    let bit = compressed.rem_euclid(256) as usize;
    let value =
        U256::from_str_radix(snapshot.bitmap_words[&index].trim_start_matches("0x"), 16).unwrap();
    snapshot
        .bitmap_words
        .insert(index, format!("0x{:064x}", value | (U256::one() << bit)));
    snapshot.initialized_ticks.insert(
        tick,
        TickState {
            liquidity_gross: gross.to_string(),
            liquidity_net: net.to_string(),
        },
    );
}

#[test]
fn tick_math_golden_boundaries_and_integer_inverse() {
    assert_eq!(sqrt_at_tick(MIN_TICK).unwrap().to_string(), "4295128739");
    assert_eq!(sqrt_at_tick(0).unwrap(), q96());
    assert_eq!(
        sqrt_at_tick(MAX_TICK).unwrap().to_string(),
        "1461446703485210103287273052203988822378723970342"
    );
    assert_eq!(
        sqrt_at_tick(-1).unwrap().to_string(),
        "79224201403219477170569942574"
    );
    assert_eq!(
        sqrt_at_tick(1).unwrap().to_string(),
        "79232123823359799118286999568"
    );
    for tick in [
        MIN_TICK,
        -524288,
        -2560,
        -1,
        0,
        1,
        2560,
        524288,
        MAX_TICK - 1,
    ] {
        let price = sqrt_at_tick(tick).unwrap();
        assert_eq!(tick_at_sqrt(price).unwrap(), tick);
        if tick > MIN_TICK {
            assert_eq!(tick_at_sqrt(price - U256::one()).unwrap(), tick - 1);
        }
    }
    assert!(sqrt_at_tick(MIN_TICK - 1).is_err());
    assert!(sqrt_at_tick(MAX_TICK + 1).is_err());
    assert!(tick_at_sqrt(sqrt_at_tick(MAX_TICK).unwrap()).is_err());
    assert!(tick_at_sqrt(U256::zero()).is_err());
}

#[test]
fn full_width_multiply_divide_rounding_and_overflow() {
    assert_eq!(
        mul_div(U256::MAX, U256::MAX, U256::MAX, false).unwrap(),
        U256::MAX
    );
    assert_eq!(
        mul_div(U256::from(5), U256::from(3), U256::from(2), false).unwrap(),
        U256::from(7)
    );
    assert_eq!(
        mul_div(U256::from(5), U256::from(3), U256::from(2), true).unwrap(),
        U256::from(8)
    );
    assert!(mul_div(U256::MAX, U256::MAX, U256::one(), false).is_err());
    assert!(mul_div(U256::one(), U256::one(), U256::zero(), false).is_err());
}

#[test]
fn exact_constant_liquidity_quotes_include_fees_in_both_directions() {
    let (snapshot, registry) = fixture(0, 1_000_000_000_000, -1, 1);
    for direction in [true, false] {
        let quote =
            quote_exact_input_math(&snapshot, &registry, AtomicAmount::from(10_000), direction)
                .unwrap();
        assert_eq!(quote.amount_out.as_str(), "9969");
        assert_eq!(quote.pool_fee_in_input_asset.as_str(), "30");
        assert_eq!(quote.amount_in.as_str(), "10000");
        assert_eq!(quote.ticks_crossed, 0);
        assert_eq!(quote.evidence, "CANDIDATE");
        assert_eq!(
            quote.input_asset,
            if direction {
                registry.token0.clone()
            } else {
                registry.token1.clone()
            }
        );
    }
    assert!(!snapshot.quality.complete_for_quote);
    assert!(!snapshot.quality.quote_implementation_qualified);
}

#[test]
fn negative_tick_compression_and_empty_word_boundaries_are_exact() {
    let mut words = BTreeMap::from([
        (-2, U256::zero()),
        (-1, U256::one() << 255),
        (0, U256::one()),
    ]);
    assert_eq!(next_tick(&words, -1, 10, true).unwrap(), (-10, true));
    assert_eq!(next_tick(&words, -1, 10, false).unwrap(), (0, true));
    assert_eq!(next_tick(&words, -11, 10, true).unwrap(), (-2560, false));
    words.insert(-1, U256::zero());
    assert_eq!(next_tick(&words, -1, 10, false).unwrap(), (0, true));
    assert_eq!(next_tick(&words, -1, 10, true).unwrap(), (-2560, false));
    assert_eq!(next_tick(&words, -2561, 10, false).unwrap(), (-10, false));
    assert!(next_tick(&words, 2560, 10, false).is_err());
}

#[test]
fn crossing_exact_current_boundary_obeys_snapshot_tick_side() {
    let (mut left, registry) = fixture(0, 1_000_000_000_000, -1, 1);
    initialize(&mut left, &registry, 0, 100_000_000, 100_000_000);
    let quote = quote_exact_input_math(&left, &registry, AtomicAmount::from(10_000), true).unwrap();
    assert_eq!(quote.ticks_crossed, 1);
    let mut already_crossed = left.clone();
    already_crossed.tick = -1;
    already_crossed.liquidity = (1_000_000_000_000u128 - 100_000_000).to_string();
    let after = quote_exact_input_math(
        &already_crossed,
        &registry,
        AtomicAmount::from(10_000),
        true,
    )
    .unwrap();
    assert_eq!(after.ticks_crossed, 0);
    assert_eq!(after.amount_out, quote.amount_out);
    assert_eq!(after.ending_sqrt_price_x96, quote.ending_sqrt_price_x96);
}

#[test]
fn full_input_is_required_and_extreme_inputs_terminate_at_both_window_edges() {
    let (snapshot, registry) = fixture(0, 1_000_000_000_000, -1, 1);
    let huge: AtomicAmount = u128::MAX.to_string().parse().unwrap();
    for direction in [true, false] {
        let error =
            quote_exact_input_math(&snapshot, &registry, huge.clone(), direction).unwrap_err();
        assert!(error.0.contains("captured"), "{}", error.0);
    }
    let (edge, narrow_registry) = fixture(0, 1_000_000_000_000, 0, 0);
    assert!(
        quote_exact_input_math(&edge, &narrow_registry, AtomicAmount::from(10_000), true).is_err()
    );
    assert!(quote_exact_input_math(&edge, &narrow_registry, huge, false).is_err());
}

#[test]
fn liquidity_crossing_underflow_and_overflow_fail_closed() {
    let (mut lower, registry) = fixture(0, 1000, -1, 1);
    initialize(&mut lower, &registry, 0, 2000, 2000);
    let error =
        quote_exact_input_math(&lower, &registry, AtomicAmount::from(100), true).unwrap_err();
    assert_eq!(
        error.0,
        "Uniswap V3 liquidity crossing overflows or underflows"
    );

    let (mut higher, registry) = fixture(0, u128::MAX, -1, 1);
    initialize(&mut higher, &registry, 10, 1, 1);
    let error = quote_exact_input_math(
        &higher,
        &registry,
        u128::MAX.to_string().parse().unwrap(),
        false,
    )
    .unwrap_err();
    assert_eq!(
        error.0,
        "Uniswap V3 liquidity crossing overflows or underflows"
    );

    let (mut reverse, registry) = fixture(0, 1000, -1, 1);
    initialize(&mut reverse, &registry, 10, 2000, -2000);
    let error =
        quote_exact_input_math(&reverse, &registry, AtomicAmount::from(100), false).unwrap_err();
    assert_eq!(
        error.0,
        "Uniswap V3 liquidity crossing overflows or underflows"
    );
}

#[test]
fn zero_liquidity_gap_is_traversed_without_unbounded_work() {
    let (mut snapshot, registry) = fixture(0, 1000, -1, 1);
    initialize(&mut snapshot, &registry, 10, 1000, -1000);
    initialize(&mut snapshot, &registry, 20, 1000, 1000);
    let quote =
        quote_exact_input_math(&snapshot, &registry, AtomicAmount::from(10), false).unwrap();
    assert_eq!(quote.ticks_crossed, 2);
    assert!(!quote.amount_out.is_zero());
    assert_eq!(quote.amount_in.as_str(), "10");
}

#[test]
fn all_bitmap_ticks_and_window_words_must_be_present_and_consistent() {
    let (mut snapshot, registry) = fixture(0, 1000, -1, 1);
    initialize(&mut snapshot, &registry, 10, 100, 100);
    let mut missing_tick = snapshot.clone();
    missing_tick.initialized_ticks.clear();
    assert!(
        quote_exact_input_math(&missing_tick, &registry, AtomicAmount::from(10), false).is_err()
    );
    let mut extra_tick = snapshot.clone();
    extra_tick.initialized_ticks.insert(
        20,
        TickState {
            liquidity_gross: "100".into(),
            liquidity_net: "100".into(),
        },
    );
    assert!(quote_exact_input_math(&extra_tick, &registry, AtomicAmount::from(10), false).is_err());
    let mut gap = snapshot.clone();
    gap.bitmap_words.remove(&0);
    assert!(quote_exact_input_math(&gap, &registry, AtomicAmount::from(10), false).is_err());
    let mut malformed = snapshot.clone();
    malformed.bitmap_words.insert(0, "0x02".into());
    assert!(quote_exact_input_math(&malformed, &registry, AtomicAmount::from(10), false).is_err());
}

#[test]
fn identity_schema_integer_and_price_validation_fail_closed() {
    let (snapshot, registry) = fixture(0, 1000, -1, 1);
    for amount in [AtomicAmount::zero(), U256::MAX.to_string().parse().unwrap()] {
        assert!(quote_exact_input_math(&snapshot, &registry, amount, false).is_err());
    }
    assert!(quote_exact_input_math(&snapshot, &registry, AtomicAmount::from(1), false).is_err());
    for liquidity in [
        "01",
        "-1",
        "+1",
        "0",
        "340282366920938463463374607431768211456",
    ] {
        let mut bad = snapshot.clone();
        bad.liquidity = liquidity.into();
        assert!(quote_exact_input_math(&bad, &registry, AtomicAmount::from(10), false).is_err());
    }
    let mut wrong_price = snapshot.clone();
    wrong_price.tick = -2;
    assert!(
        quote_exact_input_math(&wrong_price, &registry, AtomicAmount::from(10), false).is_err()
    );
    let mut wrong_pool = snapshot.clone();
    wrong_pool.pool = registry.token0.clone();
    assert!(quote_exact_input_math(&wrong_pool, &registry, AtomicAmount::from(10), false).is_err());
    let mut unknown_schema = registry.clone();
    unknown_schema.schema_version = 2;
    assert!(
        quote_exact_input_math(&snapshot, &unknown_schema, AtomicAmount::from(10), false).is_err()
    );
    let mut unsupported_spacing = registry.clone();
    unsupported_spacing.tick_spacing = 16384;
    assert!(
        quote_exact_input_math(
            &snapshot,
            &unsupported_spacing,
            AtomicAmount::from(10),
            false
        )
        .is_err()
    );
}

#[test]
fn tick_liquidity_canonical_net_and_protocol_cap_are_validated() {
    let (mut snapshot, registry) = fixture(0, 1000, -1, 1);
    initialize(&mut snapshot, &registry, 10, 100, 10);
    for net in [
        "-0",
        "+1",
        "01",
        "-01",
        "101",
        "-101",
        "-170141183460469231731687303715884105728",
    ] {
        let mut bad = snapshot.clone();
        bad.initialized_ticks.get_mut(&10).unwrap().liquidity_net = net.into();
        assert!(quote_exact_input_math(&bad, &registry, AtomicAmount::from(10), false).is_err());
    }
    snapshot
        .initialized_ticks
        .get_mut(&10)
        .unwrap()
        .liquidity_gross = u128::MAX.to_string();
    assert!(quote_exact_input_math(&snapshot, &registry, AtomicAmount::from(10), false).is_err());
}

#[test]
fn protocol_price_limits_reject_partial_input() {
    for direction in [true, false] {
        let tick = if direction {
            MIN_TICK + 1
        } else {
            MAX_TICK - 1
        };
        let word = tick.div_euclid(10).div_euclid(256) as i16;
        let (snapshot, registry) = fixture(tick, 1_000_000_000_000, word, word);
        let error = quote_exact_input_math(
            &snapshot,
            &registry,
            u128::MAX.to_string().parse().unwrap(),
            direction,
        )
        .unwrap_err();
        assert!(error.0.contains("captured"), "{}", error.0);
    }
}

#[test]
fn next_sqrt_uint256_overflow_fallback_is_exact_and_bounded() {
    let price = sqrt_at_tick(MAX_TICK - 1).unwrap();
    let amount = U256::one() << 100;
    assert!(amount.checked_mul(price).is_none());
    let result = next_sqrt_from_input(price, u128::MAX, amount, true).unwrap();
    let numerator = U256::from(u128::MAX) << 96;
    // Independent rational identity for the documented uint256 overflow fallback.
    let divisor = numerator / price + amount;
    let expected = numerator / divisor
        + if (numerator % divisor).is_zero() {
            U256::zero()
        } else {
            U256::one()
        };
    assert_eq!(result, expected);
    assert!(result < price);
    assert!(next_sqrt_from_input(price, u128::MAX, U256::MAX, false).is_err());
}

#[test]
fn next_sqrt_matches_pinned_official_sdk_primitive_vectors() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/reference/golden.json")).unwrap();
    let primitives = reference["primitives"].as_array().unwrap();
    assert!(
        !primitives.is_empty(),
        "official primitive vectors must not be empty"
    );
    for vector in primitives {
        assert_eq!(
            vector["operation"].as_str().unwrap(),
            "next_sqrt_from_input"
        );
        let price = U256::from_dec_str(vector["sqrt_price"].as_str().unwrap()).unwrap();
        let liquidity: u128 = vector["liquidity"].as_str().unwrap().parse().unwrap();
        let amount = U256::from_dec_str(vector["amount_in"].as_str().unwrap()).unwrap();
        let direction = vector["zero_for_one"].as_bool().unwrap();
        let expected = U256::from_dec_str(vector["expected_sqrt_price"].as_str().unwrap()).unwrap();
        assert_eq!(
            next_sqrt_from_input(price, liquidity, amount, direction).unwrap(),
            expected,
            "{}",
            vector["name"].as_str().unwrap()
        );
    }
}
