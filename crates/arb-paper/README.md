# arb-paper

Exact hypothetical costs and a virtual portfolio reducer for ARB-025/ARB-026. This
crate has no RPC, signing or broadcasting capability and does not label results as
realized trades. The code is a core library; connecting it to complete transaction
simulation, persistent PostgreSQL transactions, workers and dashboard reports remains
integration work.

## Reproducible costs

`evaluate_costs` starts from an exact quote whose pool fees and impact are already
included. Its expense vocabulary intentionally excludes pool fees and price impact,
so those components cannot be deducted again. Duplicate explicit expense components
are rejected. Required Base components are network execution and L1 data; required
Solana components are network execution, priority fee and relay tip. Use an explicit
known zero with provenance when a scenario does not use an optional component.

Native amounts retain their asset identity and exact units. A conversion declares
starting-asset base units per expense-asset base units, timestamp and reference;
cost conversion uses a 512-bit intermediate and rounds upward. Same-token costs can
use direct unit identity. Missing fee, funding or valuation inputs produce no
complete transaction-net result. The gross after quote-included costs remains
available without implying complete execution economics.

The output contains separate gross, transaction-net and fully allocated results.
Operating allocation requires a disclosed method, version and reference. An
unallocated overhead has no fabricated zero allocation. Every result is denominated
in its identified starting asset, with no guaranteed conversion from USDC units to
USD. Negative outcomes and fees for modeled failed attempts remain visible.

## Virtual inventory

`AccountingAsset::Token(AssetId)` is distinct from `AccountingAsset::Native(NetworkId)`.
A reservation requires both token principal and native fee inventory; a fees-only
wallet cannot satisfy principal. Initial balances belong to one immutable run.
Resetting capital requires creating a different run, preserving the earlier journal.

`PaperRun::apply` validates a command against a cloned next state and commits the
in-memory change only after every check passes. Therefore a fee reservation failure
cannot leave principal partly reserved. A single owner serializes mutation; shared
threads use a Mutex/actor. This is not a cross-process or database transaction claim.

Known success spends reserved principal, credits the returned starting asset and
spends the explicit native fee. An included atomic failure releases principal but
spends the fee. A positively modeled non-inclusion releases both reservations.
Unknown outcomes retain both; a timeout never frees capital. A fee above the reserved
budget rejects settlement and retains the reservation for explicit handling.

Every transfer writes paired debit/credit postings in the same asset. Journal events
include run, network, sequence, command identity and body. Retrying an identical
command key/body returns the original event; changing its body conflicts. Replaying
a complete ordered journal reconstructs the state and recomputes each posting,
rejecting tampering, sequence gaps and mixed runs. Prefix replay models restart from
persisted events; disk fsync, transaction atomicity and recovery from torn external
writes belong to the storage integration.

The reducer clones its journal while applying a command to keep mutation atomic.
This is a correctness baseline for bounded experiments. Large-capture performance
and a transactional storage-backed implementation must be measured before long runs;
it is not the final high-throughput accounting path.

```sh
cargo test -p arb-paper
```

Tests cover hand-calculated Base and Solana costs, explicit missing versus zero,
negative failure scenarios, separate overhead, concurrent competing reservations,
failed reservation atomicity, exact scenario settlement, unknown outcomes,
idempotent prefix replay, tampered history and bounded conservation properties.
Actual persistent crash/restart integration is not claimed by those tests.
