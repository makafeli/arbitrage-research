# Security, operations and testing

Status: v0.3 TARGET security and operations requirements. Implemented research controls include authenticated operator access, bounded read-only capture, immutable configuration, PostgreSQL generation/lease fencing and virtual-account journaling. Complete execution/signing controls below remain future work; no independent audit or production deployment is implied. Railway is the selected host. See [implementation status](12-IMPLEMENTATION-STATUS.md) and [integration verification](17-RESEARCH-INTEGRATION-VERIFICATION.md) for current scope and pending CI.

## 1. Security objectives and trust boundaries

Protect signing keys, limit the authority of each component, preserve an accurate trade ledger, and stop new submissions predictably. Rust reduces classes of memory errors; it does not establish correct trading economics, safe dependencies, trustworthy counterparties or secure contracts.

Treat market feeds, RPC responses, token metadata, pool accounts, contract return values and externally constructed transactions as untrusted inputs. Treat the strategy worker as potentially compromised when designing the signer. A separate process on the same host limits accidental exposure but does not protect against a host administrator or kernel compromise.

| Threat | Required control | Residual limitation |
|---|---|---|
| Malicious token, pool, router or program | Explicit chain-specific allowlists; known adapter versions; decode and validate every instruction or call | Allowed dependencies can still contain defects or change through upgrades |
| Forged quote or stale state | Record provenance and block/slot; validate freshness and simulate the complete transaction | Simulation cannot guarantee inclusion or unchanged execution state |
| Compromised worker requests harmful signature | Independent signer policy, bounded authority, durable intent checks | Compromise can consume the remaining permitted risk budget |
| Duplicate workers or ambiguous submission | Single leader, fencing at submission, durable transaction identity, reconciliation | Chain and provider availability can delay resolution |
| Stolen browser session or exposed API | Private network access, authenticated sessions, role checks, CSRF protection | Host or operator-device compromise remains material |
| Misleading paper profits | Separate ledgers, explicit inclusion assumptions, fees and failure costs | Counterfactual execution cannot be observed directly |

## 2. Paper deployment and live signing boundary

The paper deployment contains no private trading key, signer service or transaction-broadcast capability. Workers depend on a read/simulate interface; live submission belongs to a separate component excluded from the paper image. Use an RPC gateway that permits the required read and simulation methods and rejects broadcast methods. Restrict worker egress to that gateway and approved data feeds. An application-level `paper=true` flag alone is insufficient.

For live operation, provision a separate limited-balance identity per chain. Keep withdrawal or treasury authority outside routine strategy workers. Limit wallet funding and permitted losses independently: flash loans do not remove fee exposure, approval risk or contract defects. Never import the originally supplied script or keys into this trusted boundary without a separate review.

A signer accepts a fully specified transaction plus an immutable intent reference, current worker epoch and policy version. It independently decodes the exact bytes to be signed. It must not sign an opaque hash supplied by the worker. Its policy checks:

- Chain identity, sender/fee payer, approved executor and every permitted downstream router or program.
- Input assets, amounts, recipient accounts, minimum outputs, repayment obligations and validity deadline.
- Maximum native value, gas or compute budget, priority fees and explicit tips; reserve the maximum permitted spend against per-intent and rolling budgets.
- Current run state and fencing epoch; durable, unique intent identity; approved simulation and configuration references.
- Unsupported instruction types, token extensions, account changes, authorization/delegation requests or unexpected writable accounts: reject.

Resolve Solana address lookup tables before validating the complete instruction/account list. Decode EVM calldata through trusted adapters, including nested routes; validating only the outer router address is insufficient. Narrow supported transaction formats rather than accepting unknown extensions. The submitter repeats the run-state and epoch checks because a signed transaction can outlive its worker.

Keep secrets out of source, images, command arguments, telemetry and dashboard responses. The prepared Railway definition uses service-scoped secret/environment references; the browser receives neither database nor RPC credentials. Local development can use restricted files or environment injection. A reference or mount is not proof of encrypted storage, rotation or recovery testing; configure and verify those operational controls separately.

## 3. Execution policy and dependency changes

Start with an explicit supported-token list. Reject fee-on-transfer, rebasing and other nonstandard balance behavior until an adapter implements and tests it. For Solana, inspect token-program ownership and extensions; for EVM, inspect code identity and proxy/upgrade relationships. Record reviewed code or deployment identities. A detected upgrade pauses affected routes until compatibility is re-established.

The EVM executor and any custom Solana program must verify authorized invocation, trusted callback origin, route targets, loan asset and amount, recipient ownership, deadlines, and actual start/end balances. Verify repayment and the required asset surplus directly. An atomic transaction that reverts can still incur transaction costs. On-chain asset surplus and the operator's final net profit are separate checks because off-chain costs and some fee components are outside that balance comparison.

Approve only required spenders and amounts; account for tokens requiring allowance resets. Avoid arbitrary external calls and persistent unlimited approvals in the initial design. On-chain emergency pause blocks new execution when invoked; it does not reverse settled transactions or recall transactions already distributed to the network. Ordinary smart contracts do not run a background loop that the dashboard can terminate.

Pin the Rust toolchain, application lockfile, contract compiler, JavaScript lockfile and container image digests. Build releases from a reviewed commit using locked dependencies; record artifact hashes, build parameters and dependency inventories. Check advisories and review changes to cryptography, parsers, signing, RPC and pool math before upgrades. Reproducibility must be tested rather than inferred from a successful build. Cargo's lockfile records exact dependency resolution. [Cargo lockfile guidance](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html)

## 4. Control API and operator access

Expose the dashboard and API through a private VPN or private interface with TLS. PostgreSQL, worker control channels, metrics and signing endpoints stay private. Use authenticated sessions with secure, HttpOnly, SameSite cookies, explicit CSRF protection for mutations and origin checks for browser connections. Rate-limit authentication and mutation endpoints.

Define viewer, operator and administrator permissions even if one person initially holds all roles. Viewers inspect results; operators start, pause and stop allowed strategies; administrators change endpoints, adapters and limits. Record the actor, requested action, old/new configuration hashes, timestamp and resulting run epoch. Never place credentials in audit records. Only designated configuration fields are editable through the UI; no shell-command, arbitrary RPC-method or arbitrary transaction editor is exposed.

Live enablement is explicit for each chain and strategy. Session modes `OBSERVE`, `PAPER`, `REPLAY` and `LIVE` are immutable; switching mode creates a new session. Browser reconnection or service restart cannot promote paper into live mode. Existing live configuration still passes recovery, reconciliation and policy checks before processing resumes.

## 5. Worker lifecycle and stop semantics

| State | Required behavior |
|---|---|
| `STOPPED` | No new submissions; all existing intents resolved under the configured settlement policy |
| `RECOVERING` | Obtain leadership, restore state, reconcile unresolved transactions, validate providers and configuration |
| `RUNNING` | Evaluate and submit within the current policy, mode and fencing epoch |
| `PAUSING` | Close evaluation/submission gates and discard unsubmitted candidates |
| `PAUSED` | Gates remain closed; feeds and reconciliation continue; resumption requires fresh state |
| `DRAINING` | No new strategy submissions; continue tracking already submitted or uncertain transactions |
| `FAULTED` | Submission blocked; expose cause and required recovery steps; retain reconciliation when possible |

Pause and stop requests are idempotent. API acceptance is `PENDING`; `APPLIED` confirms the worker's effective local fence against new admissions and dispatch. The coordinator durably invalidates the submission epoch before acknowledgement. Transport calls already started remain in flight. Previously admitted signing may complete; returned bytes are quarantined and cannot dispatch. `DISARM` additionally requires durable acknowledgement of separate signer-epoch revocation; until then, revocation remains pending. Pause reaches `PAUSED`; stop proceeds to `DRAINING`.

For stop, remain `DRAINING` while outcomes are unresolved or provisional; enter `STOPPED` after configured settlement. Show fencing or reconciliation failures explicitly. Browser timeouts and force-killing workers cannot establish transaction cancellation. Observers continue tracking outstanding transactions where possible.

## 6. Crash consistency and transaction identity

PostgreSQL is authoritative for intents, risk reservations, leadership epochs and transaction records. Critical journal commits remain in the submission path; analytics are asynchronous. Each account/chain has one signing/submission leader. Signer and submission gates reject stale epochs or unverifiable authority. Lease expiry cannot revoke externally held signed bytes. Automatic live takeover is excluded: fence or terminate the former worker and reconcile outstanding attempts before transferring ownership.

The submission sequence is:

1. Commit the intent, unique strategy decision identifier, reserved risk budget, policy version and expected chain state.
2. Build, simulate and authorize the exact transaction; sign it through the bounded signer.
3. Commit its hash/signature, signed payload or protected recoverable reference, and nonce/blockhash validity before broadcast.
4. Before **every transport call**, commit a dispatch-start record with intent, transaction identity, provider, epoch and attempt sequence. Default its outcome to `UNKNOWN`. Send through the fenced gate; record acknowledgement or ambiguous failure and reconcile that identity.

A crash after sending but before recording the response produces an unknown outcome. A timeout, missing receipt or absence from one provider's mempool is not proof that nothing happened. Recovery queries independent chain views where available and restores risk reservations until resolved. Do not automatically create a fresh transaction for the same economic action.

For EVM, reconcile account nonce and transaction history, and associate any authorized replacement with the original intent and nonce. For Solana, retain the original signature and `lastValidBlockHeight`; establish expiry and check historical execution before considering a new signature. Re-signing while the original remains valid can create two executable transactions. [Solana retry guidance](https://solana.com/developers/cookbook/transactions/retry)

The MVP performs no blind automatic rebroadcast or replacement after recovery. A later bounded retry policy may resend identical signed bytes while still valid, provided the original opportunity, limits and stop state permit it. A stop blocks further propagation initiated by this system; providers may already be rebroadcasting bytes they received.

## 7. Settlement, reconciliation and accounting

Keep observation, submission, provisional inclusion, finalized settlement and economic result as distinct fields. Separate paper and live ledgers permanently. Attribute each result to its input snapshots, assumptions, configuration, software revision and actual fee records. Failed included transactions contribute costs; rejected local candidates do not invent network fees.

Use chain-specific settlement semantics. Base preconfirmation, L2 inclusion and stronger settlement stages are distinct; the withdrawal challenge period is not the settlement delay for an ordinary Base swap. [Base finality documentation](https://docs.base.org/specifications/transactions/transaction-finality)

Store canonical block hashes or slots and finality status. A reorg removes provisional ledger effects, invalidates dependent state and triggers reconciliation. Preserve the original observations and correction records for auditability. Dashboard profit distinguishes estimated, provisional and settled values. Do not free disputed capital reservations or carry provisional earnings into withdrawable-profit figures automatically.

## 8. Deployment, capacity and retention

Railway is the selected research host. The prepared web service exposes HTTPS and proxies to one private API replica; PostgreSQL remains private. Add one independently supervised worker per qualified research session, each with a dedicated persistent capture volume and no public listener. Browser sessions are process-local to the API, so restart requires reauthentication; research records remain in PostgreSQL. Do not share writable capture volumes or assume horizontal worker scaling preserves ownership. Region latency, volume permissions, quotas, backups and restore remain deployment checks. See [the Railway runbook](../deploy/RAILWAY.md); prepared definitions do not imply an applied deployment. Any future live signer topology requires its own reviewed deployment design.

Use non-root containers, read-only application filesystems where practical, dropped capabilities, bounded resources and no Docker socket mounts. Separate database, captures and logs onto quota-controlled storage. Keep CPU-heavy replay jobs from starving live data ingestion.

Capture the selected pools and events needed to reproduce decisions, rather than whole-chain traffic. Sample at least 24 hours before sizing: retained bytes ≈ measured bytes/second × 86,400 × retention days, multiplied by measured indexing/replication overhead, plus backups and operating headroom. Measure peaks separately from the average.

Initial proposed retention: detailed selected-pool captures 7 days, reproducibility bundles for evaluated decisions 30 days, and compact trade/configuration/audit records 12 months. These are configurable product defaults subject to capacity measurements. Bound every queue, cache, log stream and capture writer. Drop replaceable telemetry first; if required market updates are lost, invalidate affected state and resnapshot. Never silently drop intents or ledger events.

## 9. Backup, monitoring and operating targets

Use encrypted off-host database backups and WAL archiving; test restoration into an isolated environment with signing disabled. Back up configuration and deployment manifests separately. Restoring an old database must begin in `RECOVERING`, since previously sent transactions may be absent from the restored snapshot. Keys require a separate encrypted recovery procedure and are excluded from ordinary diagnostic bundles.

Monitor feed age, slot/block divergence, reconnects, event drops, queue depth, stage latency distributions, simulation errors, signer rejection reasons, intent age, nonce gaps, provisional settlement age, reconciliation differences, resource pressure and remaining fee budgets. Alerts link to the affected chain, run and intent. Never put secrets or unbounded token addresses into metric labels.

Proposed, unmeasured targets: healthy-host stop fencing acknowledged within 2 seconds; critical local faults visible within 30 seconds; 24-hour soak with bounded memory and no lost durable intents; backup recovery point within 15 minutes and isolated restore within 60 minutes. These are acceptance targets, not achieved service guarantees. Market-data freshness thresholds are chain/strategy-specific and must be calibrated from observations.

## 10. Operational runbooks

| Incident | Immediate behavior | Recovery condition |
|---|---|---|
| Provider outage or inconsistent heads | Fence affected submissions; reconnect with bounded backoff; preserve unknown intents | Trusted fresh snapshot and reconciled outstanding transactions |
| Reorg | Invalidate affected state and provisional accounting; pause dependent routes | Canonical state rebuilt and ledger corrections completed |
| EVM nonce gap | Stop that account's new submissions; inspect pending and canonical history | Every preceding nonce explained; any replacement explicitly linked |
| Signer unavailable | Reject new signing requests; continue observations and reconciliation | Signer identity, policy and fencing validated |
| Disk full or database unavailable | Block submissions before durable records become impossible; shed optional capture | Storage healthy, integrity checked and recovery completed |
| Suspected key or adapter compromise | Fence submissions and disable affected authority; preserve evidence | Clean credentials/deployment and reviewed resolution; move remaining assets only through the separate recovery procedure |
| Routine restart | Fence, drain where possible, snapshot and restart into recovery | Leadership, provider health and unresolved intents checked |

## 11. Verification and release evidence

| Test layer | Required evidence |
|---|---|
| Math and parsing | Integer overflow, rounding boundaries, decimals, malformed inputs and pool calculations compared with independently sourced protocol fixtures |
| Property and fuzz tests | Conservation/repayment invariants, route bounds, parser robustness and rejection of unsupported transaction structures |
| Chain integration | Pinned EVM fork fixtures; local Solana validator/program fixtures; complete swap and callback paths, failures and balance deltas |
| Paper/replay | Deterministic replay, no future-state leakage, timestamped observations, fees, deduplication and reproducible assumption changes |
| Signer and policy | Reject altered recipients, excessive fees, stale epochs, hidden nested calls, unexpected accounts and duplicate intents |
| Recovery | Crash injection before/after each durable commit and send; unknown outcomes, competing workers, reorgs, expiry and restored backups |
| Load and UI | Recorded peak streams, bounded backpressure, realistic dataset sizes, accurate stop states and stale-data indicators |

CI runs formatting, linting, meaningful Rust/contract tests, dependency checks and interface/schema compatibility checks on the pinned toolchains. Integration jobs publish fixture identities and logs. Before any live release, review signer enforcement, executor/program authorization, adapter assumptions, fund flows and recovery evidence independently of their authors. No audit, successful simulation or Rust implementation establishes that profitable execution is guaranteed. Record unresolved findings, owners and the exact restricted scope of each release.
