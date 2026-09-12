# Solana Orca Whirlpool acquisition

`capture_pool` checks the configured genesis identity and retrieves the approved Whirlpool, mints, vaults, fixed tick arrays, program and program-data account in **one** finalized `getMultipleAccounts` response. It validates owners, executable flags, the program-data address/hash and the configured asset/vault identities. Only legacy SPL Token mints of exactly 82 bytes and initialized, unfrozen vaults of exactly 165 bytes are supported. Token-2022/extensions are rejected. Tick arrays must have the fixed layout, correct parent pool, contiguous starts, consistent initialized flags and bounded signed liquidity.

The context slot and the absence of write/stream provenance are stored explicitly. `coherent`, `complete_for_quote` and `quote_implementation_qualified` remain **false** for acquired snapshots. This polling reader has no independent proof of provider bank context or reconnect history. Dynamic tick arrays, adaptive fee oracle state, continuous subscriptions, rollback invalidation, public deployment qualification and representative recorded-market fixtures remain ARB-017/018/019 work. A program-data hash pins all serialized bytes, including the upgrade metadata; an upgrade requires requalification.

The operator supplies actual verified pool, mint, vault, tick-array, program-data and genesis identities; no fabricated fixture registry qualifies as market evidence. Structural checks alone cannot certify a configuration as Solana mainnet.

Primary source checks on 2026-09-12:

- [Official Whirlpool repository](https://github.com/orca-so/whirlpools/tree/408c945fef4c49ab70def4303377cfaf8f0f3c99) publishes program identity `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`.
- [Whirlpool account layout](https://github.com/orca-so/whirlpools/blob/408c945fef4c49ab70def4303377cfaf8f0f3c99/programs/whirlpool/src/state/whirlpool.rs), [fixed tick-array layout](https://github.com/orca-so/whirlpools/blob/408c945fef4c49ab70def4303377cfaf8f0f3c99/programs/whirlpool/src/state/fixed_tick_array.rs), and [tick layout](https://github.com/orca-so/whirlpools/blob/408c945fef4c49ab70def4303377cfaf8f0f3c99/programs/whirlpool/src/state/tick.rs) define the decoded byte offsets and integer units. The decoder uses interface/layout facts and does not vendor current protocol code.
- [Solana getMultipleAccounts](https://solana.com/docs/rpc/http/getmultipleaccounts) defines request ordering, account response and context slot. `minContextSlot` does not select an exact historical bank.
- [Official SPL Token layout](https://github.com/solana-program/token/blob/main/interface/src/state.rs) defines mint/vault sizes and fields.

`tests/fixtures` contains **manually constructed**, non-market data, including fabricated identities and program bytes.

## Math-only static-fee slice

`math::quote_exact_input_math` uses exact pinned Apache-2.0 `orca_whirlpools_core` 1.0.4, with floating-point helpers disabled. `math::quote_two_leg_cycle_math` composes two distinct pools with matching context and returning assets. Both produce **CANDIDATE** values only. Pool fees and impact are already included; the reported cycle gross delta excludes external transaction costs.

This historical reference supports static fee seeds and one to six contiguous fixed tick arrays. Adaptive fees, missing/out-of-range ticks, wrong current tick/price, zero/overflow inputs and partial input consumption fail explicitly. Acquired snapshot qualification flags remain false. Independent integer fixtures and tick-crossing tests establish the wrapper's covered behavior; comparisons with current deployed program transactions remain required. See [dependency provenance and license](../../third-party/ORCA-CORE-1.0.4.md).
