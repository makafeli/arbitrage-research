//! Math-only exact-input estimates using the pinned Apache-2.0 Orca Core 1.0.4.
//!
//! This historical core supports static fees and legacy SPL Token amounts here.
//! Its result grants no executable/simulation evidence and does not qualify a provider.
use crate::{PoolSnapshot, key};
use arb_adapter_api::{AdapterError, Result, StateContext};
use orca_whirlpools_core::{
    MAX_SQRT_PRICE, MIN_SQRT_PRICE, TickArrayFacade, TickArrays, TickFacade, WhirlpoolFacade,
    sqrt_price_to_tick_index, swap_quote_by_input_token, tick_index_to_sqrt_price,
};
use serde::Serialize;
use std::collections::BTreeSet;

pub const MATH_ENGINE: &str = "orca_whirlpools_core=1.0.4;static-fee;legacy-spl";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExactInputMathQuote {
    pub input_asset: String,
    pub output_asset: String,
    pub amount_in: String,
    pub amount_out: String,
    /// Included in the computed output already; do not subtract this a second time.
    pub pool_fee_in_input_asset: String,
    pub math_engine: &'static str,
    pub evidence: &'static str,
}

fn atomic(value: &str) -> Result<u128> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AdapterError("noncanonical unsigned amount in quote state"));
    }
    value
        .parse()
        .map_err(|_| AdapterError("quote state amount overflow"))
}
fn signed_atomic(value: &str) -> Result<i128> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || value == "-0"
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AdapterError("noncanonical signed amount in quote state"));
    }
    value
        .parse()
        .map_err(|_| AdapterError("quote state signed amount overflow"))
}
fn facade(snapshot: &PoolSnapshot) -> Result<(WhirlpoolFacade, TickArrays)> {
    let state = &snapshot.state;
    key(&snapshot.pool)?;
    key(&state.mint_a)?;
    key(&state.mint_b)?;
    if !matches!(snapshot.context, StateContext::Solana { .. }) || state.mint_a == state.mint_b {
        return Err(AdapterError("invalid Solana math snapshot identities"));
    }
    if state.tick_spacing == 0 || state.fee_tier_index != state.tick_spacing {
        return Err(AdapterError(
            "adaptive fees require unsupported oracle math",
        ));
    }
    if snapshot.tick_arrays.is_empty() || snapshot.tick_arrays.len() > 6 {
        return Err(AdapterError("quote requires one to six fixed tick arrays"));
    }
    let price = atomic(&state.sqrt_price_x64)?;
    let liquidity = atomic(&state.liquidity)?;
    if !(MIN_SQRT_PRICE..=MAX_SQRT_PRICE).contains(&price)
        || liquidity == 0
        || !(-443636..=443636).contains(&state.tick_current_index)
    {
        return Err(AdapterError("invalid or empty Whirlpool quote state"));
    }
    let calculated_tick = sqrt_price_to_tick_index(price);
    if calculated_tick != state.tick_current_index
        && !(calculated_tick == state.tick_current_index + 1
            && tick_index_to_sqrt_price(calculated_tick) == price)
    {
        return Err(AdapterError(
            "Whirlpool tick and square-root price disagree",
        ));
    }
    let pool = WhirlpoolFacade {
        tick_spacing: state.tick_spacing,
        fee_rate: state.fee_rate,
        protocol_fee_rate: state.protocol_fee_rate,
        liquidity,
        sqrt_price: price,
        tick_current_index: state.tick_current_index,
        ..WhirlpoolFacade::default()
    };
    let span = i32::from(state.tick_spacing) * 88;
    let mut arrays = Vec::new();
    let mut addresses = BTreeSet::new();
    for array in &snapshot.tick_arrays {
        key(&array.address)?;
        if !addresses.insert(array.address.clone())
            || array.start_tick_index % span != 0
            || i64::from(array.start_tick_index) < -443636 - i64::from(span)
            || array.start_tick_index > 443636
        {
            return Err(AdapterError(
                "duplicate or invalid quote tick-array identity/range",
            ));
        }
        let mut ticks = [TickFacade::default(); 88];
        let mut seen = BTreeSet::new();
        for tick in &array.initialized_ticks {
            let offset = i64::from(tick.index) - i64::from(array.start_tick_index);
            if offset < 0
                || offset >= i64::from(span)
                || offset % i64::from(state.tick_spacing) != 0
                || !(-443636..=443636).contains(&tick.index)
                || !seen.insert(tick.index)
            {
                return Err(AdapterError(
                    "duplicate or out-of-range initialized quote tick",
                ));
            }
            let gross = atomic(&tick.liquidity_gross)?;
            let net = signed_atomic(&tick.liquidity_net)?;
            if gross == 0 || net.unsigned_abs() > gross {
                return Err(AdapterError("invalid quote tick liquidity"));
            }
            ticks[offset as usize / usize::from(state.tick_spacing)] = TickFacade {
                initialized: true,
                liquidity_gross: gross,
                liquidity_net: net,
                ..TickFacade::default()
            };
        }
        arrays.push(TickArrayFacade {
            start_tick_index: array.start_tick_index,
            ticks,
        });
    }
    arrays.sort_by_key(|a| a.start_tick_index);
    if arrays
        .windows(2)
        .any(|a| a[1].start_tick_index - a[0].start_tick_index != span)
    {
        return Err(AdapterError("quote tick arrays are not contiguous"));
    }
    let first = arrays[0].start_tick_index.max(-443636);
    let last = (arrays
        .last()
        .ok_or(AdapterError("missing quote tick arrays"))?
        .start_tick_index
        + span
        - 1)
    .min(443636);
    if state.tick_current_index < first || state.tick_current_index > last {
        return Err(AdapterError(
            "current tick is outside supplied quote arrays",
        ));
    }
    let arrays = match arrays.as_slice() {
        [a] => TickArrays::One(*a),
        [a, b] => TickArrays::Two(*a, *b),
        [a, b, c] => TickArrays::Three(*a, *b, *c),
        [a, b, c, d] => TickArrays::Four(*a, *b, *c, *d),
        [a, b, c, d, e] => TickArrays::Five(*a, *b, *c, *d, *e),
        [a, b, c, d, e, f] => TickArrays::Six(*a, *b, *c, *d, *e, *f),
        _ => return Err(AdapterError("unsupported tick-array count")),
    };
    Ok((pool, arrays))
}

/// Return exact integer math for the supplied state. A successful result is CANDIDATE only.
/// Fail if the historical core exhausts the captured tick window or consumes partial input.
/// Only snapshots decoded from the supported legacy-token acquisition path are accepted
/// by an integrated worker; direct callers must not assert token qualification from this helper.
pub fn quote_exact_input_math(
    snapshot: &PoolSnapshot,
    amount_in: u64,
    a_to_b: bool,
) -> Result<ExactInputMathQuote> {
    if amount_in == 0 {
        return Err(AdapterError("zero quote input"));
    }
    let (pool, arrays) = facade(snapshot)?;
    let quote = swap_quote_by_input_token(amount_in, a_to_b, 0, pool, arrays, None, None)
        .map_err(|_| AdapterError("quote math rejected state, amount or captured tick coverage"))?;
    if quote.token_in != amount_in {
        return Err(AdapterError("partial quote input consumption"));
    }
    if quote.token_est_out == 0 {
        return Err(AdapterError("quote output rounds to zero"));
    }
    let (input, output) = if a_to_b {
        (&snapshot.state.mint_a, &snapshot.state.mint_b)
    } else {
        (&snapshot.state.mint_b, &snapshot.state.mint_a)
    };
    Ok(ExactInputMathQuote {
        input_asset: input.clone(),
        output_asset: output.clone(),
        amount_in: quote.token_in.to_string(),
        amount_out: quote.token_est_out.to_string(),
        pool_fee_in_input_asset: quote.trade_fee.to_string(),
        math_engine: MATH_ENGINE,
        evidence: "CANDIDATE",
    })
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TwoLegMathQuote {
    pub first_pool: String,
    pub second_pool: String,
    pub first_leg: ExactInputMathQuote,
    pub second_leg: ExactInputMathQuote,
    /// Includes both pool fees/impact. Excludes all external fees and is not net profit.
    pub gross_delta_starting_asset: String,
    pub evidence: &'static str,
}
/// Compose two distinct same-context pools returning to the original asset.
/// Equal slot context is necessary here, but does not prove an atomic or coherent market snapshot.
pub fn quote_two_leg_cycle_math(
    first: &PoolSnapshot,
    second: &PoolSnapshot,
    amount_in: u64,
    first_a_to_b: bool,
) -> Result<TwoLegMathQuote> {
    if first.pool == second.pool || first.context != second.context {
        return Err(AdapterError(
            "cycle requires distinct pools and identical chain context",
        ));
    }
    let first_leg = quote_exact_input_math(first, amount_in, first_a_to_b)?;
    let second_a_to_b = if second.state.mint_a == first_leg.output_asset
        && second.state.mint_b == first_leg.input_asset
    {
        true
    } else if second.state.mint_b == first_leg.output_asset
        && second.state.mint_a == first_leg.input_asset
    {
        false
    } else {
        return Err(AdapterError(
            "cycle assets do not connect and return to start",
        ));
    };
    let intermediate = first_leg
        .amount_out
        .parse()
        .map_err(|_| AdapterError("intermediate amount overflow"))?;
    let second_leg = quote_exact_input_math(second, intermediate, second_a_to_b)?;
    let output: u64 = second_leg
        .amount_out
        .parse()
        .map_err(|_| AdapterError("output amount overflow"))?;
    Ok(TwoLegMathQuote {
        first_pool: first.pool.clone(),
        second_pool: second.pool.clone(),
        first_leg,
        second_leg,
        gross_delta_starting_asset: (i128::from(output) - i128::from(amount_in)).to_string(),
        evidence: "CANDIDATE",
    })
}
