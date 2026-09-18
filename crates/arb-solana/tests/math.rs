use arb_adapter_api::{SnapshotQuality, StateContext};
use arb_solana::{
    FixedTickArray, PoolSnapshot, TickState, WhirlpoolState,
    math::{quote_exact_input_math, quote_two_leg_cycle_math},
};
fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}
fn snapshot(pool: u8) -> PoolSnapshot {
    PoolSnapshot {
        pool: key(pool),
        context: StateContext::Solana {
            slot: 100,
            genesis_hash: key(9),
            commitment: "finalized".into(),
            account_context: "single-getMultipleAccounts-response".into(),
        },
        block_time_seconds: None,
        state: WhirlpoolState {
            whirlpools_config: key(8),
            tick_spacing: 64,
            fee_tier_index: 64,
            fee_rate: 3000,
            protocol_fee_rate: 0,
            liquidity: "1000000000000".into(),
            sqrt_price_x64: "18446744073709551616".into(),
            tick_current_index: 0,
            mint_a: key(3),
            mint_b: key(4),
            vault_a: key(5),
            vault_b: key(6),
        },
        tick_arrays: vec![
            FixedTickArray {
                address: key(10),
                start_tick_index: -5632,
                initialized_ticks: vec![],
            },
            FixedTickArray {
                address: key(11),
                start_tick_index: 0,
                initialized_ticks: vec![],
            },
        ],
        account_write_provenance: "manually-constructed".into(),
        quality: SnapshotQuality {
            coherent: false,
            complete_for_quote: false,
            quote_implementation_qualified: false,
            observed_at_ms: 100,
            max_age_ms: 1000,
            reasons: vec!["manual-fixture".into()],
        },
    }
}
#[test]
fn exact_integer_quote_matches_independent_constant_liquidity_vector() {
    // L=10^12, sqrt(P)=2^64, input=10000, fee=3000/10^6.
    // fee=ceil(10000*3000/10^6)=30; net input=9970.
    // Constant-liquidity output floors strictly below 9970 to 9969 in either direction.
    let s = snapshot(1);
    for direction in [true, false] {
        let q = quote_exact_input_math(&s, 10000, direction).unwrap();
        assert_eq!(q.amount_in, "10000");
        assert_eq!(q.amount_out, "9969");
        assert_eq!(q.pool_fee_in_input_asset, "30");
        assert_eq!(q.evidence, "CANDIDATE");
        // Exact input/output asset identities are the full mint addresses, never a
        // ticker or symbol, so a same-looking symbol cannot be matched by accident.
        let (expected_input, expected_output) = if direction {
            (s.state.mint_a.clone(), s.state.mint_b.clone())
        } else {
            (s.state.mint_b.clone(), s.state.mint_a.clone())
        };
        assert_eq!(q.input_asset, expected_input);
        assert_eq!(q.output_asset, expected_output);
    }
    assert!(!s.quality.quote_implementation_qualified);
}
#[test]
fn empty_liquidity_is_rejected_before_entering_core_math() {
    let mut s = snapshot(1);
    s.state.liquidity = "0".into();
    assert!(quote_exact_input_math(&s, 10000, true).is_err());
}
#[test]
fn cycle_composition_keeps_pool_fees_once_and_no_net_profit_claim() {
    let q = quote_two_leg_cycle_math(&snapshot(1), &snapshot(2), 10000, true).unwrap();
    assert_eq!(q.first_leg.amount_out, q.second_leg.amount_in);
    assert_eq!(q.second_leg.amount_out, "9938");
    assert_eq!(q.gross_delta_starting_asset, "-62");
    assert_eq!(q.evidence, "CANDIDATE");
}
#[test]
fn adaptive_fees_missing_arrays_and_partial_coverage_fail_closed() {
    let mut s = snapshot(1);
    s.state.fee_tier_index = 128;
    assert!(quote_exact_input_math(&s, 10000, true).is_err());
    let mut s = snapshot(1);
    s.tick_arrays.remove(0);
    assert!(quote_exact_input_math(&s, 10000, true).is_err());
    let s = snapshot(1);
    assert!(quote_exact_input_math(&s, u64::MAX, true).is_err());
}
#[test]
fn rejects_inconsistent_price_ticks_zero_and_overflow() {
    let mut s = snapshot(1);
    s.state.tick_current_index = 10;
    assert!(quote_exact_input_math(&s, 10000, true).is_err());
    let s = snapshot(1);
    assert!(quote_exact_input_math(&s, 0, true).is_err());
    let mut s = snapshot(1);
    s.state.liquidity = "340282366920938463463374607431768211456".into();
    assert!(quote_exact_input_math(&s, 1, true).is_err());
}
#[test]
fn rejects_cycle_context_mismatch_repeated_pool_and_disconnected_assets() {
    let first = snapshot(1);
    assert!(quote_two_leg_cycle_math(&first, &first, 10000, true).is_err());
    let mut second = snapshot(2);
    if let StateContext::Solana { slot, .. } = &mut second.context {
        *slot += 1;
    }
    assert!(quote_two_leg_cycle_math(&first, &second, 10000, true).is_err());
    let mut second = snapshot(2);
    second.state.mint_a = key(7);
    assert!(quote_two_leg_cycle_math(&first, &second, 10000, true).is_err());
}
#[test]
fn tick_crossing_changes_output_and_duplicate_tick_data_is_rejected() {
    let plain = snapshot(1);
    let mut crossing = snapshot(1);
    crossing.tick_arrays[0].initialized_ticks.push(TickState {
        index: -64,
        liquidity_gross: "500000000000".into(),
        liquidity_net: "500000000000".into(),
    });
    let plain = quote_exact_input_math(&plain, 5_000_000_000, true).unwrap();
    let quote = quote_exact_input_math(&crossing, 5_000_000_000, true).unwrap();
    assert!(quote.amount_out.parse::<u64>().unwrap() < plain.amount_out.parse::<u64>().unwrap());
    let duplicate = crossing.tick_arrays[0].initialized_ticks[0].clone();
    crossing.tick_arrays[0].initialized_ticks.push(duplicate);
    assert!(quote_exact_input_math(&crossing, 10000, true).is_err());
}

#[test]
fn both_tick_window_edges_terminate_with_large_unfilled_amounts() {
    let s = snapshot(1);
    for direction in [true, false] {
        assert!(quote_exact_input_math(&s, u64::MAX, direction).is_err());
    }
    // At a range's lower boundary there is no captured downward price interval.
    let mut lower = snapshot(1);
    lower.tick_arrays.remove(0);
    assert!(quote_exact_input_math(&lower, 10_000, true).is_err());
    // At a range's upper boundary there is no captured upward price interval.
    let mut upper = snapshot(1);
    upper.state.tick_current_index = 5631;
    upper.state.sqrt_price_x64 = orca_whirlpools_core::tick_index_to_sqrt_price(5631).to_string();
    assert!(quote_exact_input_math(&upper, 10_000, false).is_err());
}

#[test]
fn malformed_liquidity_crossings_fail_before_entering_core_math() {
    let mut underflow = snapshot(1);
    underflow.tick_arrays[0].initialized_ticks.push(TickState {
        index: -64,
        liquidity_gross: "2000000000000".into(),
        liquidity_net: "2000000000000".into(),
    });
    assert!(quote_exact_input_math(&underflow, 5_000_000_000, true).is_err());
    let mut overflow = snapshot(1);
    overflow.state.liquidity = u128::MAX.to_string();
    overflow.tick_arrays[0].initialized_ticks.push(TickState {
        index: -64,
        liquidity_gross: "1".into(),
        liquidity_net: "-1".into(),
    });
    assert!(quote_exact_input_math(&overflow, 5_000_000_000, true).is_err());
}
