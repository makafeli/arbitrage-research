# arb-domain

Deterministic types and invariants shared by research workers, control and paper
accounting. This crate performs no I/O, signing or broadcasting.

## Exact financial values

`AtomicAmount` is a canonical unsigned decimal string bounded to `2^256 - 1`.
JSON numbers, exponent notation, signs, leading zeroes and overflow are rejected.
Checked addition, subtraction, multiplication, rescaling and division report errors;
`checked_mul_div` uses a 512-bit intermediate and requires explicit `Down`, `Up` or
`Exact` rounding. These primitives do not implement a venue's swap formula.
`SignedAmount` preserves losses with the same full unsigned magnitude on each side
of zero. `Decimals` validates integer scales in 0..=255; a scale operation can still
fail if its required integer intermediate exceeds the supported width.

## Identity and evidence

Production `AssetId` and `PoolId` are network plus canonical address, serialized as
`base-mainnet:0x...` or `solana-mainnet:<base58 address>`. EVM address keys normalize
to lowercase; this is not an EIP-55 checksum or deployment verification. Solana keys
must decode canonically to 32 nonzero bytes. Tickers and fixture IDs cannot enter
these constructors. Native cost currency uses `base-mainnet:native` or
`solana-mainnet:native` in opportunity cost records; it is separate from a wrapped
token and cannot enter a trading `AssetId`. `FixtureId` is a separate type for explicitly synthetic data.
`Route::new` validates the initial two-leg cycle, one network, continuity and distinct
pools, including when deserializing a route.

`OpportunityRecord` matches the existing opportunity JSON wire shape and is an
**untrusted DTO until validated**. `validate_research` prohibits LIVE publication.
`validate` checks research/REALIZED separation, declared complete atomic simulation
and exact-plan evidence, additional estimated-executable gates, route and identity
relationships, and exact output-minus-input-minus-explicit-cost arithmetic. The
shared synthetic example round-trips without changing JSON values. The validator
checks consistency of supplied evidence; it does not prove an actual RPC simulation
occurred or that a ledger was reconciled. Future producers must supply that evidence.

`CostEstimate::Missing` and `SubmissionOutcome::Unknown` retain unknown inputs and
outcomes explicitly. Missing costs do not imply zero. Quote-included pool fees and
impact are not represented as additional deductible costs.

## Lifecycle and recovery

Session mode is immutable. `request` records acceptance without changing the worker
gate; `begin_fence` closes the gate; `acknowledge` records an applied command. STOP
with outstanding work becomes DRAINING; it becomes STOPPED after positive resolution.
STOP does not clear RECOVERING or FAULTED. LIVE and DISARM are unavailable.

`SessionSnapshot` serializes private reducer state. `Session::restore_research`
validates it before import. A coordinator may restore that reducer snapshot, but a
worker process restart must call `restart_research`, which increments the generation,
closes its gate, rejects pending admission commands and enters RECOVERING. Stored
RUNNING state is never automatic permission to resume. Timeouts never resolve an
outstanding attempt.

The reducer has no process transport fence or durable store of its own. Integration
must serialize gate transitions and persist receipts; the domain alone cannot prove
a remote worker applied a command or prevent a stale process from broadcasting.

```sh
cargo test -p arb-domain
```

Current coverage includes exact arithmetic boundary/exhaustive bounded properties,
route mutation properties, shared JSON example round-trip, simulation/evidence gates,
lifecycle controls, corrupt snapshots and restart fencing. The local and CI execution evidence is recorded separately from service integration
guarantees.

Chain-time evidence uses an explicit `finalized-chain-time-v1` research policy and
DecisionTrace 1.1. The report recomputes each source's age from its recorded chain
timestamp and the trace's UTC reference plus monotonic elapsed duration. Timestamp,
policy, context, ordered capture bindings and exact arithmetic are validated; bounded
UTC values reject overflow rather than becoming zero. Aggregate precedence is FUTURE,
UNKNOWN, STALE, WITHIN_POLICY; an empty source set is UNKNOWN. Known future timestamps
have a null age. A non-WITHIN_POLICY report requires its matching public failure reason
in the result and cannot be sealed as QUOTED. `CHAIN_TIME_STALE`, `CHAIN_TIME_FUTURE`
and `CHAIN_TIME_UNAVAILABLE` in result reasons or diagnostics require that exact
aggregate report status; an absent report or a contradictory additional status is
rejected even if the trace is resealed. `CHAIN_TIME_INVALID` remains an assessment
error code and does not assert an assessed report status. Source kind distinguishes a captured finalized Base
block timestamp from an estimated Solana block time. No status implies chain rollback
tracking, provider qualification or evidence above CANDIDATE.

Legacy DecisionTrace 1.0 omits `chain_freshness`; both old trace hashes and the independent
historical cost fixture remain unchanged. Freshness reports require schema 1.1 and the
new calculation version. Explicit null reports are invalid; absence is reserved for
legacy records. The original opportunity projection still never claims fresh coherent
state, even when the new declared time assumption passes.
