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
cost conversion uses a 512-bit intermediate and rounds upward. Costs in the exact
starting token must use `SAME_ASSET`; a conversion ratio cannot discount or inflate
the units of an identical currency. Missing fee, funding or valuation inputs produce no
complete transaction-net result. The gross after quote-included costs remains
available without implying complete execution economics.

The output contains separate gross, transaction-net and fully allocated results.
Operating allocation requires a disclosed method, version and reference. An
unallocated overhead has no fabricated zero allocation. Every result is denominated
in its identified starting asset, with no guaranteed conversion from USDC units to
USD. Negative outcomes and fees for modeled failed attempts remain visible.

## Immutable manual cost assessments

`assess_cost_scenario(&DecisionTrace, &CostScenario)` adds an explicitly
`MANUALLY_CONSTRUCTED` scenario to one validated, sealed `QUOTED` decision. The
caller supplies no replacement quote, network, starting asset or amount. Binding
retains the observation, complete decision digest, session, experiment, generation,
configuration and calculation versions, original dataset origin, network, asset,
exact input/output and historical observation time. Even a recorded market quote
with every declared cost known retains `CANDIDATE` evidence after assessment.

The versioned scenario retains its name, version, provenance identifier, all
expense declarations, funding assumption and overhead policy. Its maximum
valuation age is 1–86,400,000 milliseconds. Known valuations must be positive in
time, no later than the historical decision, and no older than that declared age.
No wall clock or current market price can replace historical inputs. Amounts are
canonical decimal strings, conversion ratios have positive exact numerator and
denominator, and all typed request objects reject unknown fields. Scenario labels
are bounded ASCII identifiers; references accept these identifiers or strict
lowercase `sha256:` digests. They are not free text or provider URLs.

All seven expense categories are bounded and unique. Base execution explicitly
includes its priority portion; a separate priority component can only be omitted
or declared zero. Solana base execution explicitly excludes its separately declared
priority fee, while Base L1 data can only be omitted or declared zero on Solana.
Execution, L1 data, priority and relay fee components require the network's native
currency, distinct from wrapped tokens. Funding, account setup and other costs may
use declared same-network native or token units. An account setup amount is a
manual expense assumption; it does not model recoverable rent, refund timing or
capital lockup.

Every applicable omitted component—including relay, funding, setup and other
costs—is retained in the report as `SCENARIO_COMPONENT_NOT_DECLARED` with an unknown
value. The transaction net remains null until all applicable expenses and the
funding assumption are known. Explicit zero has provenance and a valuation; zero
with missing valuation remains unknown. An `OTHER` declaration is still the user's
bounded assumption, not proof that every real protocol fee has been modeled.
Overhead remains separate and produces no fully allocated net when not allocated
or unknown. Negative net results are retained exactly.

All hashes use SHA-256 over compact UTF-8 JSON with recursively sorted object keys;
array order is retained. `scenario_digest` hashes the complete typed scenario and
`binding.decision_digest` hashes the complete sealed trace, including its own
observation identifier. `assessment_id` hashes the complete assessment with its
`assessment_id` field set to the empty string. `CostAssessment::replay` validates
the trace and recomputes every bound field, amount, unknown, digest, version and
evidence label before exact comparison. Changing a retained report or replacing
the decision with another sealed decision fails replay.

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
