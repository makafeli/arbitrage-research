# Data and API contracts

Status: v0.2 target contract baseline, 12 September 2026. The accompanying OpenAPI document covers the proposed initial control surface; the JSON Schema covers an opportunity record. The current foundation implements only the scope explicitly recorded in [implementation status](12-IMPLEMENTATION-STATUS.md), and the dashboard demo is not an API-conformance result. The contract version changes only when its wire format changes, independently of this document version.

## Contract ownership

The Rust domain types are the authority for financial calculations. The versioned HTTP/event schemas are the authority at process boundaries. TypeScript clients are generated from the API schema and may format amounts, but must never independently decide trade eligibility. CI must check generated bindings and schema compatibility. Chain adapters own decoding and protocol mathematics; they must implement the domain invariants below.

Use internal network identifiers `base-mainnet` and `solana-mainnet`. These are application registry keys, not CAIP identifiers. The registry must separately record the EVM chain ID or Solana network/genesis identity, provider identity, native fee asset, and finality policy. Verify the connected network before accepting any state or signing request. Initial experiments default to USDC-start cycles on each chain, with separate native fee reserves; WETH/wSOL-start experiments use a new immutable configuration and session. A token is identified by network plus contract/mint address; a ticker is a display property and never an identity.

## Exact values and time

Transfer amounts cross JSON boundaries as decimal strings of integers in the asset's smallest unit. Signed result amounts use signed integer strings. Asset metadata contains the scale, immutable registry revision, and verification provenance. On-chain calculations use checked integers of sufficient width and each protocol's exact rounding. Reporting conversions use a declared decimal or rational representation; binary floating-point must not determine spending, thresholds, repayment, or route eligibility.

Store both UTC timestamps for investigation and monotonic elapsed durations for latency measurements within a process. Do not subtract monotonic timestamps from different hosts. Each observation records source, receive time, block/slot identity, block hash where applicable, sequence or write version, commitment/finality level and adapter version. A timestamp alone does not prove two pool states were simultaneously executable.

Registry examples containing `fixture:` identifiers are synthetic test data and are rejected by all production registry loaders. The example opportunity intentionally contains no real pool, router, program or wallet address.

## Core records

| Record | Identity and required contents | Invariant |
| --- | --- | --- |
| Asset | Network, address, decimals, token behavior, registry revision, provenance | A symbol cannot authorize an asset; unknown behavior blocks execution. |
| Pool | Network, venue family/version, canonical pool address, token IDs, fee and tick metadata, program/code identity | Validation includes pool owner/factory/program and supported token features. |
| Snapshot | Snapshot ID, state references, source, receive time, consistency result, completeness | Missing or conflicting state invalidates dependent work. |
| Opportunity | Run/strategy/route IDs, ordered legs, sizes, snapshot, quote, cost model, evidence label | All legs belong to one network; final asset equals initial asset. |
| Evaluation | Opportunity ID, model/build version, inputs, output, reason codes, observed latency | Repeatable with the recorded state; every rejection is explainable. |
| Experiment | Frozen universe, period, route limits, budget, data policy, cost/inclusion scenarios, config hash | Comparisons cannot silently change their assumptions mid-run. |
| Session | Immutable network and mode, strategy/config versions, worker assignment | Mode changes create a new session after the preceding session is stopped. |
| Control command | Command ID, idempotency key, desired revision, action, issuer, accepted/applied timestamps | Receipt is different from a worker applying the command. |
| Execution intent | Unique intent ID, session, route, policy revision, reserved capital and transaction identifier | Durable before transmission; one intent owns its execution attempt history. |
| Attempt | Intent ID, signed bytes/hash/signature, nonce/blockhash context, send attempts, receipts, outcome | Uncertain transmission does not justify a new independent trade. |
| Ledger entry | Asset, debit/credit, fee type, transaction reference, provisional/final status | Reorganizations create explicit reversals or corrections, not overwritten history. |

Recommended relational tables follow these entities, with partitioning only where measured event volume justifies it. Use unique constraints on command idempotency scope, session command revision, signer/account nonce reservations, and transaction identifiers. A changing Solana blockhash is not itself a business-level duplicate protection mechanism: retain the intent identity across replacements.

## Evidence labels and values

`CANDIDATE` means a route quote exists. `SIMULATED` means the supported complete transaction was successfully simulated at recorded state. `ESTIMATED_EXECUTABLE` additionally requires coherent fresh state, complete costs, a supported atomic route with the required final-balance guard, valid principal and native-fee reservations, and successful complete simulation tied to the exact plan digest. It then applies a named delay and inclusion scenario and remains an estimate. Virtual inventory and simulation funding overrides are explicit paper assumptions; they cannot establish live funding eligibility. `REALIZED` requires transaction evidence and ledger reconciliation. Each label is stored with its supporting evidence rather than inferred from a positive number.

An ordinary paper evaluation may remain `CANDIDATE` if transaction construction or simulation is unsupported. A same-state profitable simulation is not a fill. A live result can be provisional until finality; the UI must expose that separately. Failed included transactions can have realized negative fee results. Pending or unknown transactions do not become zero-profit successes.

Cost records distinguish quoted output, explicit costs and informational breakdowns. Pool fees and price impact already incorporated into quoted output are not subtracted again. Native gas and priority fees require a timestamped conversion into the starting asset when presenting a net result. Fixed operating costs belong in run economics and must not be hidden inside an unexplained per-trade figure. A missing conversion or fee estimate produces an incomplete result, never an implicit zero cost.

A paper record cannot contain a production signature, a live fill count or a `REALIZED` label. The JSON Schema implements the mode/label distinction; adapter validation implements the additional network, route and monetary relationships that JSON Schema alone cannot establish.

## Worker controls

Modes are `OBSERVE`, `PAPER`, `REPLAY`, `LIVE`. The first production deployment permits the first three and contains no live signer or broadcaster. The schema reserves the `LIVE` value for a later capability-gated build; accepting a value in a schema does not grant execution capability.

Observed states are `RECOVERING`, `STOPPED`, `RUNNING`, `PAUSING`, `PAUSED`, `DRAINING`, `FAULTED`.

| Action | Preconditions | Applied meaning |
| --- | --- | --- |
| START | STOPPED, reconciliation complete, current configuration and capabilities valid | New evaluations may enter; LIVE additionally needs a separately armed, bounded permit. |
| PAUSE | RUNNING | Admission and future local signing/submission are fenced; feeds and outstanding reconciliation continue. Stable state PAUSED. |
| RESUME | PAUSED, health and revision checks pass | Admissions resume. A revoked or expired live permit is not recreated. |
| STOP | Any assigned session state | New admissions and future local submissions are fenced; unresolved work drains before STOPPED. |
| DISARM | Any state of a LIVE session | The worker submission fence is effective and the separate signer has durably acknowledged epoch revocation. Outstanding outcomes still need reconciliation. |

The server persists the command and advances the desired revision in a transaction. The worker verifies its fencing epoch, closes or opens the relevant local gate, and acknowledges the applied revision. An HTTP 202 response means `PENDING`, not that the gate has changed. The UI waits for `APPLIED`, and can then show `DRAINING` while previously emitted transactions remain unresolved. Applied stop cannot recall bytes already sent to a provider or a transaction already included on chain. An already admitted signing operation may complete after the worker fence; its returned bytes must be quarantined and cannot be submitted. DISARM remains PENDING until both the worker fence and durable signer-epoch revocation are acknowledged. An unavailable signer therefore produces a visible pending revocation, even when the worker fence is already effective.

The broadcaster rechecks the session revision and active lease immediately before each send under the worker's serialized submission gate. The signer validates policy and epoch independently. On loss of ownership or heartbeat, the local watchdog and expiring permits prohibit new operations through cooperating worker/signer gates. Permit deadlines are checked immediately before each action, with the permitted duration fixed in the reviewed deployment policy. These checks cannot invalidate signed bytes already held elsewhere or prove that a partitioned or compromised former worker cannot broadcast. Initial live operation has no automatic failover: ownership transfer requires fencing or terminating the former worker and reconciling outstanding intents. Restart enters RECOVERING then STOPPED, with no automatic live rearming.

An All workers stop is a fan-out of independently idempotent per-session commands; the UI shows every acknowledgement and outstanding count, and cannot claim a global stop until all relevant worker fences are acknowledged. It has no cross-chain atomic guarantee. DISARM is a reserved LIVE operation and returns a capability error in the initial research-only deployment.

For concurrent operators or multiple browser tabs, requests carry `expected_revision`. Stale revisions return 409. Reusing an idempotency key with identical request contents returns the original receipt; changing contents returns 409. A command may be `SUPERSEDED` by a later STOP/DISARM but never misreported as applied. A timed-out HTTP request can be queried or retried with the same key; no new command key is necessary.

## API boundary

The initial OpenAPI surface includes health/status, session creation, session inspection, lifecycle commands, command receipts and opportunity queries. Routes are authenticated. The health response discloses minimal status. A private network is the initial deployment boundary; authenticated sessions, origin/CSRF defenses for mutations, and secret redaction remain required.

The sample API intentionally exposes no private-key upload, wallet withdrawal, arbitrary transaction signing, or live arming endpoint. Later live arming must be designed around an independently enforced, expiring permit bound to network, wallet, strategies, targets, fee/notional limits, session and policy revision. It is a separate reviewable implementation milestone.

Pagination uses stable cursors and an upper page limit. Opportunity queries return mode and evidence label on each row. Event streaming is a later transport for the same models; first implementation may poll status with a declared update interval. When streaming is added, sequence cursors and snapshot resynchronization must handle gaps. Frontend disconnection has no authority to resume or change a worker.

## Retention and storage

Persist the exact state needed to reproduce a bounded set of evaluated routes, rather than promising a full-chain archive. Measure event bytes and rate during an initial capture; daily raw storage is `events_per_second × average_bytes × 86,400`, before indexes, compression and replicas. State snapshot/delta dependencies and deletion rules must remain replayable. If a replay window is incomplete, label it incomplete instead of filling missing records from future data.

Retain control/audit history and execution reconciliation independently from disposable telemetry. Analytics writes may be batched, but the critical intent, nonce/reservation and signed-transaction journal must commit before broadcast. Before each transport call, also commit a dispatch-start record with intent, hash/signature, provider, authorization epoch and attempt sequence. Until its result is known, treat that attempt as potentially emitted. The extra durability latency is an explicit design choice. Paper result writes do not receive permission to backpressure live submission indefinitely; bounded queues surface dropped analytics and mark affected experiments incomplete.

## Acceptance examples

1. Two clients issue commands at revision 12: one succeeds at revision 13; the other receives 409 and refreshes.
2. STOP returns PENDING while the worker is unreachable. The interface must not display STOPPED. Local watchdog/permit expiry forbids further cooperative submission but does not prove that previously signed bytes cannot be emitted. Takeover remains blocked until the former process is fenced and earlier intents are reconciled.
3. A process crashes after journal commit and before provider response. Recovery finds the same hash/signature and reconciles it; it does not assume the trade failed.
4. Paper output is supplied with a REALIZED label. Schema/domain validation rejects it.
5. A 256-bit token amount passes through the API and browser unchanged as a string.
6. A Base pool snapshot is rolled back. Dependent candidates and provisional ledger results are invalidated or corrected with traceable events.
7. The UI sees an applied STOP with unresolved transactions. It displays DRAINING and the outstanding count, keeping the command acknowledgement visible.

See [architecture](02-ARCHITECTURE.md), [trading model](04-TRADING-AND-PAPER-MODEL.md), [security and operations](06-SECURITY-OPERATIONS-AND-TESTING.md), and [OpenAPI](../specs/openapi.yaml).
