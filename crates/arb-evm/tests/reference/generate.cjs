'use strict';
// Executes the published SDK, not a JavaScript port of our Rust implementation.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const JSBI = require('jsbi');
const { Pool, TickMath, SqrtPriceMath, SwapMath, TickListDataProvider } = require('@uniswap/v3-sdk');

const SOURCE_COMMIT = '4e16fe8e56c8c26541545f138c89133794c7ce72';
const sdkVersion = require('@uniswap/v3-sdk/package.json').version;
assert.equal(sdkVersion, '3.11.0', 'oracle version changed');
assert.equal(require('jsbi/package.json').version, '3.2.5', 'integer dependency changed');
const bigint = value => JSBI.BigInt(value.toString());
const text = value => value.toString();
const Q96 = 1n << 96n;
const L = 1_000_000_000_000n;
const MAX_STEPS = 4096;
const FEE_SPACING = { 100: 1, 500: 10, 3000: 60, 10000: 200 };
const rangeTicks = (liquidity, bound = 6000) => [
  { index: -bound, liquidity_net: liquidity.toString() },
  { index: bound, liquidity_net: (-liquidity).toString() }
];
const crossingTicks = [
  [-6000, L], [-180, L / 4n], [-120, -L / 4n], [-60, L / 2n],
  [60, -L / 2n], [120, -L / 4n], [180, L / 4n], [6000, -L]
].map(([index, net]) => ({ index, liquidity_net: net.toString() }));
const gapTicks = [
  [-6000, L], [-120, -L], [-60, L], [60, -L], [120, L], [6000, -L]
].map(([index, net]) => ({ index, liquidity_net: net.toString() }));

function makeCase(name, options = {}) {
  const fee = options.fee_pips ?? 3000;
  const spacing = FEE_SPACING[fee];
  assert(spacing, 'use an official standard fee and tick spacing');
  const tick = options.tick ?? 0;
  const liquidity = options.liquidity ?? L;
  const ticks = (options.ticks ?? rangeTicks(liquidity)).map(t => {
    const net = BigInt(t.liquidity_net);
    return { index: t.index, liquidity_gross: (net < 0n ? -net : net).toString(),
      liquidity_net: net.toString() };
  }).sort((a, b) => a.index - b.index);
  const wordMap = new Map();
  const indexes = ticks.map(t => Math.floor(Math.floor(t.index / spacing) / 256));
  const minWord = Math.min(...indexes, Math.floor(Math.floor(tick / spacing) / 256));
  const maxWord = Math.max(...indexes, Math.floor((Math.floor(tick / spacing) + 1) / 256));
  assert(maxWord - minWord < 16, 'fixtures must fit the supported sixteen-word capture window');
  for (let word = minWord; word <= maxWord; word++) wordMap.set(word, 0n);
  let netSum = 0n;
  let active = 0n;
  const minUsable = Math.trunc(-887272 / spacing) * spacing;
  const maxUsable = Math.trunc(887272 / spacing) * spacing;
  const maxGross = ((1n << 128n) - 1n) / BigInt((maxUsable - minUsable) / spacing + 1);
  for (const t of ticks) {
    assert.equal(t.index % spacing, 0, 'unusable tick');
    assert(BigInt(t.liquidity_gross) <= maxGross, 'per-tick maximum liquidity exceeded');
    netSum += BigInt(t.liquidity_net);
    if (t.index <= tick) active += BigInt(t.liquidity_net);
    const compressed = t.index / spacing;
    const word = Math.floor(compressed / 256);
    const bit = compressed - word * 256;
    wordMap.set(word, wordMap.get(word) | (1n << BigInt(bit)));
  }
  assert.equal(netSum, 0n, 'synthetic full tick list must balance');
  assert.equal(active, liquidity, 'synthetic active liquidity must match the tick list');
  return {
    name, tick, sqrt_price: text(TickMath.getSqrtRatioAtTick(tick)),
    liquidity: liquidity.toString(), tick_spacing: spacing, fee_pips: fee,
    words: Array.from(wordMap, ([index, bitmap]) => ({ index, bitmap: bitmap.toString() })),
    ticks, amount_in: (options.amount_in ?? 1_000_000n).toString(),
    zero_for_one: options.zero_for_one ?? true
  };
}

async function quote(input) {
  const ticks = input.ticks.map(t => ({
    index: t.index, liquidityGross: bigint(t.liquidity_gross), liquidityNet: bigint(t.liquidity_net)
  }));
  const provider = new TickListDataProvider(ticks, input.tick_spacing);
  const availableWords = new Set(input.words.map(w => w.index));
  const calls = [];
  const crossings = [];
  const originalNext = provider.nextInitializedTickWithinOneWord.bind(provider);
  const originalGet = provider.getTick.bind(provider);
  provider.nextInitializedTickWithinOneWord = async (tick, lte, spacing) => {
    const compressed = Math.floor(tick / spacing);
    const word = Math.floor((compressed + (lte ? 0 : 1)) / 256);
    assert(availableWords.has(word), input.name + ': oracle left the captured word window');
    assert(calls.length < MAX_STEPS, input.name + ': oracle exceeded bounded step count');
    calls.push(word);
    return originalNext(tick, lte, spacing);
  };
  provider.getTick = async tick => {
    crossings.push(tick);
    return originalGet(tick);
  };
  // The constructor also configures currencies and prices, which are unrelated
  // to swap arithmetic. Invoke the unchanged published private swap method.
  const pool = Object.create(Pool.prototype);
  pool.sqrtRatioX96 = bigint(input.sqrt_price);
  pool.liquidity = bigint(input.liquidity);
  pool.tickCurrent = input.tick;
  pool.fee = input.fee_pips;
  pool.tickDataProvider = provider;
  assert.equal(pool.tickSpacing, input.tick_spacing);
  assert.equal(typeof pool.swap, 'function', 'published SDK no longer exposes its compiled swap loop');
  let consumed = 0n;
  let amountOut = 0n;
  let fees = 0n;
  let steps = 0;
  const compute = SwapMath.computeSwapStep;
  SwapMath.computeSwapStep = function (...args) {
    assert(++steps <= MAX_STEPS, 'bounded oracle step count');
    const result = compute.apply(SwapMath, args);
    // Observation only: return every original value unchanged.
    consumed += BigInt(text(result[1])) + BigInt(text(result[3]));
    amountOut += BigInt(text(result[2]));
    fees += BigInt(text(result[3]));
    return result;
  };
  let result;
  try {
    result = await pool.swap(input.zero_for_one, bigint(input.amount_in));
  } finally {
    SwapMath.computeSwapStep = compute;
  }
  assert.equal(consumed.toString(), input.amount_in, input.name + ': require full input consumption');
  assert.equal((-BigInt(text(result.amountCalculated))).toString(), amountOut.toString());
  return { ...input, ...(amountOut === 0n ? { expected_rejection: 'zero-output' } : {}), expected: {
    amount_in: consumed.toString(), amount_out: amountOut.toString(),
    fee: fees.toString(), ending_sqrt_price: text(result.sqrtRatioX96),
    ending_tick: result.tickCurrent, liquidity: text(result.liquidity),
    initialized_ticks_crossed: crossings.length
  }, oracle_trace: { steps, visited_words: calls, initialized_ticks: crossings } };
}

function primitive(name, sqrt, liquidity, amount, zeroForOne) {
  return {
    name, operation: 'next_sqrt_from_input', sqrt_price: sqrt.toString(),
    liquidity: liquidity.toString(), amount_in: amount.toString(),
    zero_for_one: zeroForOne,
    expected_sqrt_price: text(SqrtPriceMath.getNextSqrtPriceFromInput(
      bigint(sqrt), bigint(liquidity), bigint(amount), zeroForOne
    ))
  };
}

async function generate() {
  const definitions = [
    makeCase('constant_liquidity_token0_to_token1'),
    makeCase('constant_liquidity_token1_to_token0', { zero_for_one: false }),
    makeCase('one_unit_input_is_consumed_as_rounded_fee', { amount_in: 1n }),
    makeCase('fee_rounding_low_fee', { fee_pips: 500, amount_in: 2001n }),
    makeCase('fee_rounding_high_fee', { fee_pips: 10000, amount_in: 101n, zero_for_one: false }),
    makeCase('multiple_initialized_ticks_down', { ticks: crossingTicks, liquidity: 3n * L / 2n, amount_in: 20_000_000_000n }),
    makeCase('multiple_initialized_ticks_up', { ticks: crossingTicks, liquidity: 3n * L / 2n, amount_in: 20_000_000_000n, zero_for_one: false }),
    makeCase('zero_liquidity_gap_down', { ticks: gapTicks, amount_in: 10_000_000_000n }),
    makeCase('zero_liquidity_gap_up', { ticks: gapTicks, amount_in: 10_000_000_000n, zero_for_one: false }),
    makeCase('empty_bitmap_words_down', { fee_pips: 100, ticks: rangeTicks(L, 2000), amount_in: 25_000_000_000n }),
    makeCase('empty_bitmap_words_up', { fee_pips: 100, ticks: rangeTicks(L, 2000), amount_in: 25_000_000_000n, zero_for_one: false }),
    makeCase('negative_word_floor_division_down', { fee_pips: 100, ticks: rangeTicks(L, 2000), tick: -257, amount_in: 25_000_000_000n }),
    makeCase('negative_non_aligned_tick_up', { ticks: crossingTicks, liquidity: L, tick: -61, amount_in: 10_000_000_000n, zero_for_one: false }),
    makeCase('large_exact_integer_amount', { liquidity: 10n ** 29n, amount_in: 10n ** 24n })
  ];
  const cases = [];
  for (const input of definitions) cases.push(await quote(input));
  const max128 = (1n << 128n) - 1n;
  const highPrice = BigInt(text(TickMath.MAX_SQRT_RATIO)) - 1n;
  const overflowAmount = 1n << 100n;
  assert(overflowAmount * highPrice > (1n << 256n) - 1n, 'must exercise uint256 product overflow fallback');
  const primitives = [
    primitive('token0_product_overflow_fallback', highPrice, max128, overflowAmount, true),
    primitive('token0_zero_amount_identity', Q96, L, 0n, true),
    primitive('token1_round_down', Q96, L, 1234567n, false)
  ];
  return { schema_version: 1, source_commit: SOURCE_COMMIT, fixture_origin: 'synthetic', cases, primitives };
}

async function main() {
  const mode = process.argv[2];
  assert(['--write', '--check'].includes(mode), 'usage: node generate.cjs --write|--check');
  const result = await generate();
  const encoded = JSON.stringify(result, null, 2) + '\n';
  const target = path.join(__dirname, 'golden.json');
  if (mode === '--write') {
    fs.writeFileSync(target, encoded, { flag: 'w' });
  } else {
    assert.equal(fs.readFileSync(target, 'utf8'), encoded, 'committed oracle fixture differs: regenerate and review');
  }
  console.log(JSON.stringify({
    oracle: '@uniswap/v3-sdk@3.11.0', source_commit: SOURCE_COMMIT,
    fixture_origin: 'synthetic', cases: result.cases.length, primitives: result.primitives.length,
    sha256: crypto.createHash('sha256').update(encoded).digest('hex'),
    action: mode.substring(2)
  }));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
