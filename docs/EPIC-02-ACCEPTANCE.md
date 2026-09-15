# EPIC-02: platform contracts and integrated observation

The original epic is #5, with nine original tasks ARB-007 through ARB-015.
ARB-007/#20 and ARB-008/#22 were already accepted. This continuation works
through the remaining original requirements; it creates no replacement issues.
The identity predecessor, EPIC-01/#2 and ARB-003/#16, is now accepted. A tested
identity catalogue remains distinct from a runtime-ready market capture.

## Configuration, journal, lifecycle, API and capture acceptance

The following is the criterion-level review map. It becomes acceptance evidence
only with actual current-source test results, reviewed integration and verified
native prerequisite states. Existing historical production/campaign conditions
are not silently used to expand these original platform-contract tasks.

### ARB-009 / #23: immutable configuration

`arb-config` validates TOML and revalidates persisted effective JSON, including
default expansion, exact integer strings, capability boundaries and maximum
canonical size. `FrozenSessionConfig` has private identity fields and no mutation
API. A replacement requires STOPPED and creates a new configuration binding;
changing mode or starting asset never changes the old snapshot. The new
`platform_contract` suite covers each non-STOPPED state, changed starting assets,
mode changes, immutable prior serialization, canonical equivalent serialization
and secret-redacted errors on persisted input. Existing twelve config tests cover
inert examples, unsupported capabilities, units, ordering and size limits.

Accepted predecessor requirements are ARB-003/#16 and ARB-008/#22. Real provider
activation, market completeness and transaction simulation remain downstream,
not extra requirements added to this original configuration contract.

### ARB-010 / #24: durable journal and migrations

`arb-storage` serializes creation and command retries, scopes idempotency by
operator/session, compares complete payloads and rejects stale revisions.
Acceptance writes PENDING; the API cannot manufacture a worker APPLIED result.
Immutable configuration/session fields and append-only audit constraints are
also enforced in PostgreSQL. Existing `postgres.rs` tests cover concurrent
creation, concurrent command retries, changed payload conflicts, stale revisions,
query pagination, immutable state and database constraints.

The new `arb-control/tests/platform_recovery.rs` test starts a real OS subprocess,
commits a START command, writes a synchronization marker and then terminates the
process before its first command-application tick. A fresh connection must see
exactly the original PENDING receipt and same-key retry. The disposable lease is
expired explicitly; production claim/recovery methods must reject the old pending
intent and return STOPPED without changing mode/network/configuration. Both
Base/Solana and OBSERVE/PAPER combinations are exercised. The separately listed
child-entry test is a subprocess harness entry point, not independent evidence.

`migrations/README.md`, the operations/recovery documentation and existing
restore drill preserve audit history, reject incompatible state and require
isolated restoration rather than destructive down-migration. Those are the
original rollback/recovery-instruction criteria. Actual production backup
capacity, deployment access and operational disaster-recovery acceptance remain
at their existing operational tickets, not falsely completed here.

### ARB-011 / #25: worker fences and recovery

`ControlWorker` applies local generation/epoch fences before durable ACK, closes
the gate on cancellation or failure, and never reopens it merely because a
later poll succeeds. Existing PostgreSQL tests exercise delayed ACK, duplicate
commands, old-generation rejection, outstanding-attempt DRAINING, separate
session stop-all outcomes, lease takeover, cancelled database operations and
restart with unresolved attempts. The real subprocess test above extends process
loss coverage; the existing worker executable tests cover blocked RPC, stop and
late captured results. Restart is RECOVERING then STOPPED, never an implicit
START or LIVE arm.

The original scope concerns those lifecycle semantics. Automatic economic fills
and complete atomic simulations are not prerequisites for demonstrating an
unresolved-attempt fixture or a research cancellation fence. They remain later
paper/simulation requirements. A previously emitted RPC cannot be recalled.

### ARB-012 / #26: authenticated API contract

The shared API has bounded typed requests, authentication, origin and CSRF checks,
operator-scoped idempotency, stable error codes and correlation IDs. Unsupported
live-only actions fail explicitly. Request acceptance is still PENDING until a
worker applies it. Full source tests include the real PostgreSQL HTTP contract,
strict schema fixtures and the actual TypeScript-client/Rust-HTTP service smoke,
not only a mock router. Secret material is excluded from error/log projections.

The new admission fix below retains the original overall limit and preserves
command capacity during bulk-read saturation. This improves an actual original
isolation requirement; no new endpoint or privilege is introduced. A live market
screenshot is not required to accept these API semantics; the real recorded
observation demonstration remains in ARB-015/#29.

### ARB-014 / #28: input provenance, integrity and expiry

Versioned `arb-capture` manifests locate raw/normalized inputs with digest,
network/configuration/build/adapter/schema context. Unsupported versions, corrupt
hashes, missing input, unsafe filesystem paths and malformed Unicode fail closed
or remain an explicit reported gap. Synthetic/manually constructed provenance is
not promoted to recorded market evidence. Historical expiry metadata is separate
from physical file availability and never rewritten to make an old input fresh.

The previously merged export-audit bridge in `docs/25-EXPORT-CAPTURE-AUDIT.md`
already binds an authenticated complete export reference set to a local per-volume
audit request/report. It refuses different export hashes and omitted references.
The older native issue note describing that bridge as missing is obsolete.
Actual Rust-writer/Python-reader integration, export binding, missing/expired
objects, corrupt versions/hashes and immutable historical output are exercised
by the existing capture, audit, recovery and browser/TypeScript suites.

This is capture provenance and reportability, not a claim of economic replay,
complete real-market tick coverage or a production restore. Those limits remain
visible. #23 and #24 must be accepted before this task closes.

## ARB-013 / #27: concrete read-isolation correction

Before this change, every authenticated API request used the same 64-slot
semaphore and 300-request/minute counter. Non-export dashboard reads could occupy
all slots or exhaust the rate budget, preventing a control request from reaching
its handler. The separate two-export limit did not protect other dashboard reads.

Bulk reads now acquire a capacity-four sub-budget **before** global admission.
This leaves capacity within the Store's eight database connections while retaining
the original 64-request total bound. Cheap capability/authentication/status and
command-receipt requests are excluded from the bulk group. The same per-session
300-request total limit remains, with at most 240 bulk-read admissions per minute;
refused bulk retries cannot consume the remaining 60 control/status admissions.
This is not an unlimited STOP bypass, protection from a malicious authenticated
operator, or a guarantee against shared-host/database failure.

Owned semaphore permits release on timeout, cancellation and failed global
admission. No result claims APPLIED merely because STOP was admitted. Regression
cases hold all bulk reads indefinitely, overflow both the read and retry budgets,
then assert a new STOP is accepted as PENDING without reaching the blocked reads.
A separate test retains the original global cap and proves a rejected global
admission returns its previously acquired read permit. The older capacity test
is retained under the stricter read bound; no test is disabled to pass.

This corrects one real isolation gap. Existing scheduler fixed bounds, twelve
network/stage metric rows, monotonic timing populations and slow-sink tests remain.
Full application-stage instrumentation and the recorded integration profile are
separate still-open ARB-013 requirements, not established by these admission tests.

## Whole-epic boundary and next integration

ARB-015/#29 expressly requires a genuine recorded observation-to-control trace,
including exact versions, negative/rejected outcomes, stop ACK, restart-to-STOPPED
and replay. Its original ARB-016/018/022/023 dependencies remain authoritative,
including both chain adapters through the coherent snapshot/math/route chain.
An identity-only report cannot be relabelled as raw quote input to close this.

The present source change is reviewed and tested as a coherent platform-contract
wave. The epic remains open until every required child and the four original
whole-epic criteria have actual evidence. No new provider request, secrets read,
paid service, production deployment, wallet operation, signing or broadcasting
is introduced. Reviewer is the implementing/root assistant under the established
solo-maintainer convention, not an invented independent reviewer or agent team.

## Verified native acceptance checkpoint, 15 September 2026

PR #125 is merged as `d78635df4a4f63d4bc2d702e611f52b580ec757c` with tree
`c7ab8a8ed05e8758959808cb0acf987b893ba6f5`, identical to tested source
`c9933f3b9582cc6ab38204ba3ddf01dbcfff65b0`. Source reviews 5203567914 and
5203647170 identify the implementing Technical Lead under the solo-maintainer
convention. CodeRabbit confirmed both workflow corrections and resolved them;
no new independent safety certification is claimed.

All five source-head workflows passed: full project 34905430286, platform
34905430399, delivery 34905430391, recovery 34905430459 and preparation
34905430331. The four core jobs include actual Rust/PostgreSQL/process/HTTP
execution, web/browser checks, specifications and all three container builds.
The test inputs remain labelled synthetic; this is not market performance.

Native #23 and #24 had already closed on 14 September before the interrupted
conversation resumed. On 15 September, the original criteria and accepted
predecessors were checked, the existing offline closeout verifier returned
EVIDENCE_PACKET_CONSISTENT, and #25, #26 and #28 were explicitly closed at
05:05:06, 05:09:01 and 05:13:09 UTC respectively. Those resumed packets used a
complete prior snapshot with explicitly identified current native state/checklist
projections: non-atomic, not a fabricated fresh authenticated snapshot. The final
metadata reconciliation independently reads the current issue set before publication.

The five platform tasks and earlier #20/#22 make seven of nine EPIC-02 tasks
accepted. #27 and #29 remain open with their unchanged original requirements,
including #29's M2 adapter/snapshot/route dependencies. No epic criterion or child
is excluded to reduce counts. Previous remaining-acceptance notes remain in the
reconciliation history. This checkpoint does not declare the epic, an executable
paper release, a deployed recovery drill or a live service complete.

## Scheduler acceptance synchronized on 15 September 2026

Native #27 closed at 11:17:27 UTC before the current recorded-slice continuation.
Its four original criteria and accepted #20/#22/#25 dependencies are preserved.
Final PR127 head `b460a82b87389c7a6be2e1a0d507bda9d0f269e4` and merged main
`ecabdfab7a045398994f421f1ffa624b7bd7d27b` share tree
`4bfb5ff4d3c5cee0c7b49095869517e4692948f5`. Full project34961520133 and
pipeline34961520116 passed along with the other three required workflows.
Four isolation cases and all eight existing controlled-capture cases ran without
ignored/filtered cases. The held RESEARCH attempt stayed pending through both
STOP acknowledgements and was suppressed after release; production deadlines
were unchanged. Root review5209212132 and CodeRabbit's confirmed corrections are
recorded in the native issue. This synchronization is not counted as a new closure.

Eight of the nine epic children are therefore accepted. #29's genuine recorded
observation/control/replay demonstration and its original M2 prerequisites still
remain. The new orchestration is not an acceptance shortcut or a production
activation. Earlier sections retain their dated implementation/verification scope.
