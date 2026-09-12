# arb-domain

Implemented: an immutable-mode research session, a pure lifecycle reducer, an
unsigned 256-bit canonical decimal amount boundary, and a necessary evidence/mode
check. The reducer returns a new value, so rejected operations leave the old value
unchanged. Session fields are private; a session's mode cannot be changed in place.

`request` models acceptance, leaving the observed state and local gate unchanged.
`begin_fence` models the serialized worker-local fence, invalidates the calculation
generation and enters `PAUSING` for ordinary states. `acknowledge` then marks the
command `APPLIED`. Applied STOP is `DRAINING` while attempts remain unresolved and
`STOPPED` only after resolution. STOP during `RECOVERING` or `FAULTED` acknowledges
the local fence while preserving that blocking state. START and RESUME require an
explicit readiness result; its actual dependency/freshness/limits checks remain
future integration work. Feeds and read-only reconciliation are outside the reducer.

The reducer has **no durable command store, receipt history, API idempotency,
process coordination, real transport fence, signer epoch revocation or transaction
finality verification**. A future service must serialize it with its actual gates
and persist transitions. `record_attempt` and `resolve_attempt` are generic
in-memory accounting transitions; only evidence-backed callers may resolve a real
attempt. A timeout alone cannot resolve it. A future durable service must mark a
superseded receipt explicitly; this reducer retains only the current command.

The LIVE enum exists for contract vocabulary, but research session construction
rejects LIVE and every DISARM command. There is no signer/broadcast implementation.
`validate_evidence_mode` prevents research output from using `REALIZED`; it does
not establish simulation success or estimated-executable eligibility. The full
evidence validator remains planned.

`AtomicAmount` accepts canonical decimal base units from zero through `2^256 - 1`
and preserves them as strings. It performs no protocol arithmetic. Venue math must
implement its own checked widths, intermediates, rounding and fees; V3 calculations
can require intermediates wider than 256 bits. Decimal token scaling and signed
PnL are not implemented here. The future API serializer must emit this type as a
JSON string, never a JSON number.

```sh
cargo test -p arb-domain
```

The test suite covers gate/ack separation, stopping with unresolved work, recovery,
fault preservation, stale revisions, superseding stop, readiness, exact large
amounts, unsupported LIVE/DISARM and invalid research evidence. Tests are authored
but were not run locally because the Rust toolchain was unavailable.
