//! Exact-input Uniswap V3 integer math over a finite captured bitmap window.
//!
//! Adapted from the MIT-licensed Uniswap v3-sdk 3.11.0, pinned commit
//! 4e16fe8e56c8c26541545f138c89133794c7ce72. Copyright (c) 2021 Uniswap Labs.
//! Full license and provenance: third-party/UNISWAP-V3-SDK-3.11.0.md.
//! Results remain CANDIDATE evidence; provider qualification, token behavior,
//! gas costs and transaction simulation are separate admission gates.

use crate::{PoolRegistry, PoolSnapshot};
use arb_adapter_api::{AdapterError, Result, StateContext};
use arb_domain::AtomicAmount;
use primitive_types::{U256, U512};
use serde::Serialize;
use std::collections::BTreeMap;

pub const MATH_ENGINE: &str =
    "uniswap-v3-sdk=3.11.0;4e16fe8e56c8c26541545f138c89133794c7ce72;checked-rust-exact-input";
const MIN_TICK: i32 = -887272;
const MAX_TICK: i32 = 887272;
// At most 16 captured words, 256 initialized ticks and one boundary per word.
const MAX_STEPS: usize = 16 * 257 + 2;
const FEE_DENOMINATOR: u32 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExactInputMathQuote {
    pub input_asset: String,
    pub output_asset: String,
    pub amount_in: AtomicAmount,
    pub amount_out: AtomicAmount,
    /// Already reflected in amount_out. Do not subtract this fee twice.
    pub pool_fee_in_input_asset: AtomicAmount,
    pub ending_sqrt_price_x96: AtomicAmount,
    pub ticks_crossed: u32,
    pub math_engine: &'static str,
    pub evidence: &'static str,
}

fn overflow() -> AdapterError {
    AdapterError("Uniswap V3 arithmetic overflow")
}
fn add(a: U256, b: U256) -> Result<U256> {
    a.checked_add(b).ok_or_else(overflow)
}
fn sub(a: U256, b: U256) -> Result<U256> {
    a.checked_sub(b)
        .ok_or(AdapterError("Uniswap V3 arithmetic underflow"))
}
fn narrow(value: U512) -> Result<U256> {
    U256::try_from(value).map_err(|_| overflow())
}
fn mul_div(a: U256, b: U256, divisor: U256, round_up: bool) -> Result<U256> {
    if divisor.is_zero() {
        return Err(AdapterError("Uniswap V3 division by zero"));
    }
    let product = a.full_mul(b);
    let divisor = U512::from(divisor);
    let quotient = narrow(product / divisor)?;
    if round_up && !(product % divisor).is_zero() {
        add(quotient, U256::one())
    } else {
        Ok(quotient)
    }
}
fn q96() -> U256 {
    U256::one() << 96
}
fn atomic(value: U256) -> Result<AtomicAmount> {
    value.to_string().parse().map_err(|_| overflow())
}
fn unsigned(value: &str) -> Result<u128> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AdapterError("noncanonical quote liquidity"));
    }
    value.parse().map_err(|_| overflow())
}
fn signed(value: &str) -> Result<i128> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || value == "-0"
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AdapterError("noncanonical quote liquidity delta"));
    }
    value.parse().map_err(|_| overflow())
}

// Multipliers and rounding follow the pinned MIT SDK's TickMath algorithm.
fn sqrt_at_tick(tick: i32) -> Result<U256> {
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        return Err(AdapterError("Uniswap V3 tick out of bounds"));
    }
    const FACTORS: [&str; 20] = [
        "fffcb933bd6fad37aa2d162d1a594001", "fff97272373d413259a46990580e213a",
        "fff2e50f5f656932ef12357cf3c7fdcc", "ffe5caca7e10e4e61c3624eaa0941cd0",
        "ffcb9843d60f6159c9db58835c926644", "ff973b41fa98c081472e6896dfb254c0",
        "ff2ea16466c96a3843ec78b326b52861", "fe5dee046a99a2a811c461f1969c3053",
        "fcbe86c7900a88aedcffc83b479aa3a4", "f987a7253ac413176f2b074cf7815e54",
        "f3392b0822b70005940c7a398e4b70f3", "e7159475a2c29b7443b29c7fa6e889d9",
        "d097f3bdfd2022b8845ad8f792aa5825", "a9f746462d870fdf8a65dc1f90e061e5",
        "70d869a156d2a1b890bb3df62baf32f7", "31be135f97d08fd981231505542fcfa6",
        "9aa508b5b7a84e1c677de54f3e99bc9", "5d6af8dedb81196699c329225ee604",
        "2216e584f5fa1ea926041bedfe98", "48a170391f7dc42444e8fa2",
    ];
    let mut ratio = U256::one() << 128;
    for (bit, factor) in FACTORS.iter().enumerate() {
        if tick.unsigned_abs() & (1 << bit) != 0 {
            let factor = U256::from_str_radix(factor, 16).map_err(|_| overflow())?;
            ratio = narrow(ratio.full_mul(factor) >> 128)?;
        }
    }
    if tick > 0 {
        ratio = U256::MAX / ratio;
    }
    let quotient = ratio >> 32;
    if ratio & ((U256::one() << 32) - U256::one()) != U256::zero() {
        add(quotient, U256::one())
    } else {
        Ok(quotient)
    }
}

// Bounded integer search is independent of the SDK's logarithm implementation.
fn tick_at_sqrt(price: U256) -> Result<i32> {
    if price < sqrt_at_tick(MIN_TICK)? || price >= sqrt_at_tick(MAX_TICK)? {
        return Err(AdapterError("Uniswap V3 square-root price out of bounds"));
    }
    let (mut low, mut high) = (MIN_TICK, MAX_TICK);
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if sqrt_at_tick(middle)? <= price {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(low)
}

fn amount0_delta(a: U256, b: U256, liquidity: u128, round_up: bool) -> Result<U256> {
    let (a, b) = (a.min(b), a.max(b));
    let numerator = U256::from(liquidity) << 96;
    let first = mul_div(numerator, sub(b, a)?, b, round_up)?;
    mul_div(first, U256::one(), a, round_up)
}
fn amount1_delta(a: U256, b: U256, liquidity: u128, round_up: bool) -> Result<U256> {
    mul_div(U256::from(liquidity), sub(a.max(b), a.min(b))?, q96(), round_up)
}
fn next_sqrt_from_input(
    price: U256,
    liquidity: u128,
    amount: U256,
    zero_for_one: bool,
) -> Result<U256> {
    if liquidity == 0 || price.is_zero() {
        return Err(AdapterError("cannot move quote price without liquidity"));
    }
    if amount.is_zero() {
        return Ok(price);
    }
    if zero_for_one {
        let numerator = U256::from(liquidity) << 96;
        // Preserve the protocol's uint256 overflow fallback and its rounding:
        // evaluating the wider exact fraction unconditionally is not equivalent.
        if let Some(denominator) = amount
            .checked_mul(price)
            .and_then(|product| numerator.checked_add(product))
        {
            return mul_div(numerator, price, denominator, true);
        }
        let denominator = add(numerator / price, amount)?;
        mul_div(numerator, U256::one(), denominator, true)
    } else {
        let next = add(price, mul_div(amount, q96(), U256::from(liquidity), false)?)?;
        if next.bits() > 160 {
            return Err(AdapterError("Uniswap V3 price exceeds uint160"));
        }
        Ok(next)
    }
}

struct SwapStep {
    price: U256,
    amount_in: U256,
    amount_out: U256,
    fee: U256,
}
fn compute_step(
    current: U256,
    target: U256,
    liquidity: u128,
    remaining: U256,
    fee: u32,
) -> Result<SwapStep> {
    let zero_for_one = current >= target;
    let available = mul_div(
        remaining,
        U256::from(FEE_DENOMINATOR - fee),
        U256::from(FEE_DENOMINATOR),
        false,
    )?;
    let required = if zero_for_one {
        amount0_delta(target, current, liquidity, true)?
    } else {
        amount1_delta(current, target, liquidity, true)?
    };
    let price = if available >= required {
        target
    } else {
        next_sqrt_from_input(current, liquidity, available, zero_for_one)?
    };
    if price < current.min(target) || price > current.max(target) {
        return Err(AdapterError("Uniswap V3 quote step escaped price interval"));
    }
    let amount_in = if price == target {
        required
    } else if zero_for_one {
        amount0_delta(price, current, liquidity, true)?
    } else {
        amount1_delta(current, price, liquidity, true)?
    };
    let amount_out = if zero_for_one {
        amount1_delta(price, current, liquidity, false)?
    } else {
        amount0_delta(current, price, liquidity, false)?
    };
    let fee = if price != target {
        sub(remaining, amount_in)?
    } else {
        mul_div(amount_in, U256::from(fee), U256::from(FEE_DENOMINATOR - fee), true)?
    };
    Ok(SwapStep { price, amount_in, amount_out, fee })
}

struct ValidatedState {
    price: U256,
    liquidity: u128,
    words: BTreeMap<i16, U256>,
    ticks: BTreeMap<i32, i128>,
}
fn validate(snapshot: &PoolSnapshot, registry: &PoolRegistry) -> Result<ValidatedState> {
    registry.validate()?;
    if !snapshot.pool.eq_ignore_ascii_case(&registry.pool)
        || !matches!(snapshot.context, StateContext::Evm { .. })
        || !(MIN_TICK..=MAX_TICK).contains(&snapshot.tick)
        || registry.tick_spacing >= 16384
    {
        return Err(AdapterError("unsupported or inconsistent Uniswap V3 quote identity"));
    }
    let bytes = crate::hex_bytes(&snapshot.sqrt_price_x96_hex)?;
    if bytes.len() != 20 {
        return Err(AdapterError("Uniswap V3 price must be exact uint160 bytes"));
    }
    let price = U256::from_big_endian(&bytes);
    let calculated_tick = tick_at_sqrt(price)?;
    if calculated_tick != snapshot.tick
        && !(calculated_tick == snapshot.tick + 1 && sqrt_at_tick(calculated_tick)? == price)
    {
        return Err(AdapterError("Uniswap V3 tick and square-root price disagree"));
    }
    let liquidity = unsigned(&snapshot.liquidity)?;
    if liquidity == 0 {
        return Err(AdapterError("empty initial Uniswap V3 liquidity"));
    }
    let expected_words = i32::from(registry.bitmap_word_max) - i32::from(registry.bitmap_word_min) + 1;
    if snapshot.bitmap_words.len() != expected_words as usize {
        return Err(AdapterError("incomplete Uniswap V3 captured bitmap window"));
    }
    // Truncation toward zero gives the first/last usable ticks, as in V3.
    let first_usable = MIN_TICK / registry.tick_spacing * registry.tick_spacing;
    let last_usable = MAX_TICK / registry.tick_spacing * registry.tick_spacing;
    let usable_ticks = (last_usable - first_usable) / registry.tick_spacing + 1;
    let max_gross = u128::MAX / usable_ticks as u128;
    let mut words = BTreeMap::new();
    let mut ticks = BTreeMap::new();
    for word_index in registry.bitmap_word_min..=registry.bitmap_word_max {
        let bytes = crate::hex_bytes(snapshot.bitmap_words.get(&word_index)
            .ok_or(AdapterError("missing Uniswap V3 captured bitmap word"))?)?;
        if bytes.len() != 32 {
            return Err(AdapterError("Uniswap V3 bitmap must be exact uint256 bytes"));
        }
        let bitmap = U256::from_big_endian(&bytes);
        for bit in 0..256 {
            if !bitmap.bit(bit) {
                continue;
            }
            let index = (i64::from(word_index) * 256 + bit as i64) * i64::from(registry.tick_spacing);
            if !(i64::from(MIN_TICK)..=i64::from(MAX_TICK)).contains(&index) {
                return Err(AdapterError("initialized Uniswap V3 tick outside protocol bounds"));
            }
            let index = index as i32;
            let state = snapshot.initialized_ticks.get(&index)
                .ok_or(AdapterError("missing initialized Uniswap V3 tick state"))?;
            let gross = unsigned(&state.liquidity_gross)?;
            let net = signed(&state.liquidity_net)?;
            if gross == 0 || gross > max_gross || net.unsigned_abs() > gross || net == i128::MIN {
                return Err(AdapterError("invalid Uniswap V3 tick liquidity"));
            }
            ticks.insert(index, net);
        }
        words.insert(word_index, bitmap);
    }
    if ticks.len() != snapshot.initialized_ticks.len() {
        return Err(AdapterError("Uniswap V3 tick absent from captured bitmap"));
    }
    Ok(ValidatedState { price, liquidity, words, ticks })
}

// Word boundaries are real swap steps, even when uninitialized. Skipping empty
// words and jumping straight to an initialized tick changes fee/delta rounding.
fn next_tick(
    words: &BTreeMap<i16, U256>,
    tick: i32,
    spacing: i32,
    zero_for_one: bool,
) -> Result<(i32, bool)> {
    let compressed = tick.div_euclid(spacing) + i32::from(!zero_for_one);
    let word_index = i16::try_from(compressed.div_euclid(256)).map_err(|_| overflow())?;
    let bit = compressed.rem_euclid(256) as usize;
    let word = words.get(&word_index)
        .ok_or(AdapterError("input exceeds captured Uniswap V3 tick coverage"))?;
    let found = if zero_for_one {
        (0..=bit).rev().find(|position| word.bit(*position))
    } else {
        (bit..256).find(|position| word.bit(*position))
    };
    let position = found.unwrap_or(if zero_for_one { 0 } else { 255 });
    let index = (i64::from(word_index) * 256 + position as i64) * i64::from(spacing);
    let index = index.clamp(i64::from(MIN_TICK), i64::from(MAX_TICK)) as i32;
    Ok((index, found.is_some()))
}
fn cross_liquidity(liquidity: u128, net: i128, zero_for_one: bool) -> Result<u128> {
    let subtract = (net >= 0) == zero_for_one;
    if subtract {
        liquidity.checked_sub(net.unsigned_abs())
    } else {
        liquidity.checked_add(net.unsigned_abs())
    }.ok_or(AdapterError("Uniswap V3 liquidity crossing overflows or underflows"))
}

/// Computes a full exact-input quote within the captured V3 bitmap window.
/// Registry metadata is explicit to preserve the persisted snapshot v1 format.
/// This function never promotes acquisition quality or simulation evidence.
pub fn quote_exact_input_math(
    snapshot: &PoolSnapshot,
    registry: &PoolRegistry,
    amount_in: AtomicAmount,
    zero_for_one: bool,
) -> Result<ExactInputMathQuote> {
    let mut state = validate(snapshot, registry)?;
    let input = U256::from_dec_str(amount_in.as_str()).map_err(|_| overflow())?;
    let max_signed = U256::MAX >> 1;
    if input.is_zero() || input > max_signed {
        return Err(AdapterError("Uniswap V3 exact input must fit positive int256"));
    }
    let edge_tick = if zero_for_one {
        i64::from(registry.bitmap_word_min) * 256 * i64::from(registry.tick_spacing)
    } else {
        (i64::from(registry.bitmap_word_max) * 256 + 255) * i64::from(registry.tick_spacing)
    }.clamp(i64::from(MIN_TICK), i64::from(MAX_TICK)) as i32;
    let captured_limit = sqrt_at_tick(edge_tick)?;
    let limit = if zero_for_one {
        captured_limit.max(add(sqrt_at_tick(MIN_TICK)?, U256::one())?)
    } else {
        captured_limit.min(sub(sqrt_at_tick(MAX_TICK)?, U256::one())?)
    };
    if (zero_for_one && limit >= state.price) || (!zero_for_one && limit <= state.price) {
        return Err(AdapterError("no captured Uniswap V3 price range in quote direction"));
    }
    let mut tick = snapshot.tick;
    let mut remaining = input;
    let mut output = U256::zero();
    let mut fees = U256::zero();
    let mut ticks_crossed = 0;
    for _ in 0..MAX_STEPS {
        if remaining.is_zero() {
            if output.is_zero() {
                return Err(AdapterError("Uniswap V3 quote output rounds to zero"));
            }
            return Ok(ExactInputMathQuote {
                input_asset: if zero_for_one { &registry.token0 } else { &registry.token1 }.to_lowercase(),
                output_asset: if zero_for_one { &registry.token1 } else { &registry.token0 }.to_lowercase(),
                amount_in,
                amount_out: atomic(output)?,
                pool_fee_in_input_asset: atomic(fees)?,
                ending_sqrt_price_x96: atomic(state.price)?,
                ticks_crossed,
                math_engine: MATH_ENGINE,
                evidence: "CANDIDATE",
            });
        }
        if state.price == limit {
            return Err(AdapterError("input exceeds captured Uniswap V3 price limit"));
        }
        let (next_tick_index, initialized) = next_tick(&state.words, tick, registry.tick_spacing, zero_for_one)?;
        let next_price = sqrt_at_tick(next_tick_index)?;
        let target = if zero_for_one { next_price.max(limit) } else { next_price.min(limit) };
        if (zero_for_one && target > state.price) || (!zero_for_one && target < state.price) {
            return Err(AdapterError("Uniswap V3 next tick moves in wrong direction"));
        }
        let previous = (remaining, state.price, tick);
        let step = compute_step(state.price, target, state.liquidity, remaining, registry.fee)?;
        remaining = sub(remaining, add(step.amount_in, step.fee)?)?;
        output = add(output, step.amount_out)?;
        if output > max_signed {
            return Err(AdapterError("Uniswap V3 output exceeds signed accounting bound"));
        }
        fees = add(fees, step.fee)?;
        state.price = step.price;
        if state.price == next_price {
            if initialized {
                let net = *state.ticks.get(&next_tick_index)
                    .ok_or(AdapterError("missing Uniswap V3 liquidity crossing"))?;
                state.liquidity = cross_liquidity(state.liquidity, net, zero_for_one)?;
                ticks_crossed += 1;
            }
            tick = if zero_for_one { next_tick_index - 1 } else { next_tick_index };
        } else if state.price != previous.1 {
            tick = tick_at_sqrt(state.price)?;
        }
        if previous == (remaining, state.price, tick) {
            return Err(AdapterError("Uniswap V3 quote step made no progress"));
        }
    }
    Err(AdapterError("Uniswap V3 captured-window step budget exhausted"))
}

#[cfg(test)]
mod tests;
