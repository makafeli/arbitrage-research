#!/usr/bin/env python3
"""Generate the reviewable delivery backlog; no network or GitHub mutations."""
from pathlib import Path
import json
from collections import Counter

ROOT = Path(__file__).resolve().parent
TICKETS = []

def ticket(number, milestone, title, role, priority, estimate, dependencies, requirements, problem, scope, acceptance, verification, refs, exclusions=""):
    tid = f"ARB-{number:03d}"
    marker = f"<!-- arb-ticket:{tid} -->"
    area = {"M0": "planning", "M1": "platform", "M2": "adapters", "M3": "simulation", "M4": "dashboard", "M5": "research", "M6": "live", "M7": "expansion"}[milestone]
    deps = [f"ARB-{d:03d}" if isinstance(d, int) else d for d in dependencies]
    gate = "research" if milestone < "M6" else "operator-live-approval" if milestone == "M6" else "operator-pilot-or-expansion-approval"
    body = "\n".join([
        marker,
        f"## {tid}: {title}",
        "",
        f"**Milestone:** {milestone} · **Epic:** EPIC-{int(milestone[1:])+1:02d} · **Priority:** {priority} · **Responsible role:** {role}",
        f"**Estimate:** {estimate} working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.",
        "",
        "### Problem and intended outcome", problem,
        "", "### Scope", *[f"- {s}" for s in scope],
        "", "### Acceptance criteria", *[f"- [ ] {s}" for s in acceptance],
        "", "### Verification and review evidence", *[f"- {s}" for s in verification],
        "", "### Dependencies and release gate",
        "- Prerequisites: " + (", ".join(deps) if deps else "none; this issue establishes prerequisites for later work") + ".",
        "- " + ("Research work only. Signing, production execution deployment, funding and live activation are outside this milestone." if gate == "research" else "This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope."),
        "", "### References",
        "- Requirements: " + ", ".join(requirements) + ".",
        *[f"- `{r}`" for r in refs],
        "", "### Scope boundaries",
        exclusions or "Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.",
        "",
    ])
    TICKETS.append({"id": tid, "kind": "ticket", "title": title, "milestone": milestone, "epic_id": f"EPIC-{int(milestone[1:])+1:02d}", "labels": ["type:task", f"phase:{milestone.lower()}", f"area:{area}", f"priority:{priority.lower()}", "status:planned"] + (["gate:live-review"] if milestone >= "M6" else []), "role": role, "priority": priority, "estimate_days": estimate, "dependencies": deps, "requirement_ids": requirements, "status": "planned", "release_gate": gate, "marker": marker, "references": refs, "body": body})

PRD = "docs/01-PRD.md"
ARCH = "docs/02-ARCHITECTURE.md"
MODEL = "docs/04-TRADING-AND-PAPER-MODEL.md"
UX = "docs/05-UX-DESIGN.md"
SEC = "docs/06-SECURITY-OPERATIONS-AND-TESTING.md"
PLAN = "docs/07-DELIVERY-PLAN.md"
API = "docs/08-DATA-AND-API-CONTRACTS.md"
DEC = "docs/09-DECISIONS-AND-OPEN-QUESTIONS.md"

# M0: decisions and prerequisites.
ticket(1,"M0","Ratify research scope, success measures and release gates","Product Owner","P0","1-2",[],["PRD-F01","PRD-F09","PRD-F14"],
"Turn the proposed platform into a bounded research commitment with a decision-maker for unresolved assumptions. Acceptance is useful learning, including a negative economic result.",
["Record the private single-operator scope, Rust backend, Solana/Base comparison and USDC-starting two-leg routes.","Define research-preview, executable-paper, observation-campaign and optional-live releases with separate exit evidence.","Assign decision roles and an owner/due milestone to each open question; no real-person assignment without consent."],
["PRD and decision register agree on supported markets and exclusions.","M3 requires one complete atomic route simulation per chain under declared virtual funding; missing capability permits only quote-research preview.","Live activation requires a later concrete operator decision and never follows automatically from paper results.","Initial observation success measures report coverage, reproducibility and total costs without an income target."],
["Product Owner and Technical Lead review a requirements-to-milestones checklist.","Publish dated decisions and unresolved assumptions with consequences for dependent tickets."],[PRD,PLAN,DEC])

ticket(2,"M0","Qualify data providers and set a spending envelope","Operations engineer","P0","2-3",[1],["PRD-F03","NFR-02","NFR-05"],
"Adapter feasibility depends on access to historical and coherent live state. Establish what each provider actually supplies and what it costs before buying services.",
["Compare read-only Base RPC and Solana RPC/streaming candidates using representative pool/account queries.","Document rate limits, retention, subscriptions, batch/context semantics, reconnect behavior and data terms.","Record a proposed monthly ceiling and an approved cost owner before any paid purchase."],
["At least one provider per chain yields the complete required state for the candidate pool family, or a blocking gap is recorded.","Provider-specific gaps are distinguished from chain inactivity.","Credentials use environment/secret references and never appear in fixtures, issue bodies or exports.","No undocumented provider feature is required by the initial vertical slice."],
["Attach redacted sample request/response provenance and measured request-latency distributions.","Record official provider documentation URLs and retrieval date alongside the shortlist."],[ARCH,SEC,DEC])

ticket(3,"M0","Verify initial chain, token and venue identities","Chain engineers","P0","2-4",[1,2],["PRD-F02","PRD-F04","NFR-01"],
"Tickers and plausible addresses are insufficient to establish a tradeable universe. Build an evidence-backed registry for a deliberately small supported pool set.",
["Verify Base Uniswap V3 and Solana Orca Whirlpool program/contract identities from official deployment records and chain observations.","Identify USDC/WETH and USDC/wSOL pools, owners, decimals, fee parameters and liquidity suitability.","Record source provenance, code/program versions and unsupported token behavior."],
["Every candidate asset is keyed by network plus address/mint and every pool records exact asset identities.","No fixture: identifier can pass a production registry validator.","At least two distinct eligible pools per chain are sought; absence is recorded and blocks two-pool experiments rather than fabricating liquidity.","Same-venue routes are permitted; Sushi/Raydium and additional token behaviors remain unqualified expansion scope."],
["Cross-check registry entries against primary deployment documentation and read-only chain state.","Review evidence with the opposite chain engineer or Technical Lead."],[PRD,ARCH,DEC])

ticket(4,"M0","Set deployment, retention and benchmark assumptions","Technical Lead / Operations","P1","1-2",[1,2],["PRD-F13","NFR-02","NFR-03","NFR-08"],
"Performance targets and storage plans have no meaning without an operating environment. Define a repeatable research host profile and capture policy.",
["Choose a documented Linux architecture, CPU/RAM/storage profile and hosted-provider connection topology.","Budget raw capture, summaries, backups and replay manifests separately.","Define preliminary queue/freshness budgets as hypotheses to measure in M1."],
["The deployment profile can run independent Base/Solana workers, API, database and dashboard.","Retention explains which historical claims cease to be reproducible after raw data expiry.","No universal microsecond or throughput guarantee is asserted.","Capacity and cost owners are documented; Redis/Kafka/Kubernetes are not introduced without a measured need."],
["Publish a reproducible benchmark-environment manifest.","Review a sample 24-hour storage estimate with transparent input rates and assumptions."],[ARCH,SEC,DEC])

ticket(5,"M0","Establish threat model, license provenance and paper boundaries","Security reviewer / Technical Lead","P0","2-3",[1],["PRD-F16","NFR-06"],
"The original untrusted script must not become a production dependency. Establish trust boundaries and a research package that cannot move funds.",
["Model provider data, imported fixtures, dependencies, operator browser, API and worker boundaries.","Record provenance/license review requirements for third-party libraries and protocol code.","Define a build/deployment boundary excluding private-key loading, transaction signing and broadcast from research artifacts."],
["The uploaded script is excluded from trusted execution code.","Provider data and imported files are validated as untrusted inputs.","Paper and replay services cannot acquire signing or submission capabilities through an ordinary configuration toggle.","Threat findings have severity, owner role and a milestone gate; unresolved findings are visible."],
["Review dependency graph and process/capability diagrams.","Provide an attack-surface checklist covering logs, exports, authentication and fixture import."],[SEC,ARCH,PRD])

ticket(6,"M0","Create delivery conventions, repository governance and acceptance workflow","Technical Lead / Product Owner","P1","1-2",[1,5],["NFR-06","NFR-08"],
"Make the design actionable without treating ticket creation as completed engineering. Establish durable issue IDs and evidence-based review conventions.",
["Publish repository README, contribution guide, issue/PR templates, wiki navigation and stable backlog markers.","Define labels, milestones and project fields for priority, role, dependency and release gate.","Document branch/CI protection proposals and who can accept a release."],
["All eight epics and their work tickets have stable ARB/EPIC IDs and resolvable prerequisites.","A PR template requests the concrete behavior, relevant requirement IDs and actual validation evidence.","Project setup can be rerun without duplicating issues or board entries.","Native GitHub features that cannot be configured by available permissions are explicitly marked pending; no nonexistent board/wiki is reported as complete."],
["Validate backlog JSON and check all dependency IDs and cycles.","Exercise the bootstrap in a dry-run or inspect an execution manifest without mutating unrelated repositories."],[PLAN,"planning/BACKLOG.md","planning/backlog.json"])

# M1: shared platform and first vertical slice.
ticket(7,"M1","Build the Rust workspace and reproducible research CI","Technical Lead","P0","2-3",[4,5,6],["NFR-01","NFR-06","NFR-08"],
"A coherent workspace gives later adapters one trusted build baseline. Repository scaffolding is accepted only after compilation and required checks run in a named environment.",
["Create modular Cargo workspace applications/crates with documented ownership and dependency direction.","Pin a supported toolchain, commit Cargo.lock and configure formatting, linting and meaningful tests.","Separate research build targets from deferred signer and live submission packages."],
["A clean checkout compiles and runs research checks on the documented Linux CI environment.","The actual lockfile is generated by Cargo, not invented.","Research dependency inspection proves no key backend or broadcast entry point is shipped.","CI exposes failures rather than silently skipping unavailable required tools."],
["Attach successful CI run and toolchain/dependency versions.","Run formatting, clippy and workspace tests appropriate to implemented packages; record absent local toolchains honestly."],[ARCH,"docs/03-REPOSITORY-STRUCTURE.md",SEC])

ticket(8,"M1","Implement exact amounts, identities and evidence invariants","Engine engineer","P0","3-5",[7],["PRD-F01","PRD-F02","PRD-F06","NFR-01"],
"Errors in units and evidence labels can make a false profit look valid. Create exact, validated domain types shared by every chain and API.",
["Implement chain/asset/pool/route identifiers, raw unsigned amounts, signed P&L and explicit decimals.","Define immutable session modes, lifecycle enums and opportunity evidence types.","Use checked integer/fixed-point operations with explicit overflow and rounding errors."],
["JSON round-trips values above JavaScript's safe integer range as strings.","Cross-network route continuity, ticker-only identity, invalid decimals and overflow are rejected.","PAPER or REPLAY cannot emit REALIZED; local arithmetic alone cannot emit SIMULATED.","Missing costs and unknown submission outcomes have explicit variants rather than numeric zero or failed defaults."],
["Run boundary/property tests for amounts and route invariants.","Validate shared examples and API schemas against the Rust serialization shape."],[API,"specs/opportunity.schema.json",MODEL])

ticket(9,"M1","Implement immutable validated experiment configuration","Backend engineer","P0","2-3",[3,8],["PRD-F01","PRD-F02","NFR-08"],
"A configuration must identify the exact experiment and fail safely before workers start. Implement a validated research configuration with clear capability errors.",
["Parse TOML into typed provider, network, asset, route, size, delay, fee and retention settings.","Compute configuration version/digest and preserve defaults in the effective configuration.","Reject unsupported combinations and fixture identities in production registries."],
["A session's mode, network, starting asset and configuration version cannot mutate in place.","Changing mode or starting asset requires a stopped prior session and a new session configuration.","The shipped example remains inert: networks disabled, empty allowlists and no live permission.","Errors identify the field and reason without exposing credential values."],
["Run valid/invalid configuration fixtures for units, limits, capabilities and secret redaction.","Verify canonical configuration hashes remain stable across equivalent serialization."],["config/research.example.toml",PRD,API])

ticket(10,"M1","Add PostgreSQL migrations, durable sessions and command journal","Backend engineer","P0","3-5",[8,9],["PRD-F11","PRD-F13","NFR-04"],
"Operator intent must survive retries and restarts. Store configurations, sessions, revisions and command application outcomes durably before acknowledging control requests.",
["Implement reviewed migrations for configuration snapshots, sessions, commands and audit events.","Persist command idempotency keys scoped to an authenticated operator/session.","Separate desired command, PENDING acceptance and worker-applied acknowledgement."],
["Duplicate requests return the same command and cannot create duplicate state transitions.","Stale expected revisions produce an explicit conflict.","Restart preserves commands and observed status without inferring APPLIED from API acceptance.","Migration rollback/recovery instructions preserve audit history and make compatibility limitations explicit."],
["Run database integration tests for concurrent duplicates and stale revisions.","Interrupt the process between command persistence and worker acknowledgement; verify correct recovery."],[API,ARCH,SEC])

ticket(11,"M1","Implement worker lifecycle, cancellation fences and restart recovery","Backend engineer / QA","P0","3-5",[10],["PRD-F11","NFR-03","NFR-04"],
"Start and stop must reflect actual worker behavior. Implement session-local control fences and state transitions that remain meaningful during slow or lost communication.",
["Implement RECOVERING, STOPPED, RUNNING, PAUSING, PAUSED, DRAINING and FAULTED transitions.","Close evaluation/admission gates on pause or stop, cancel unsent generation-tagged work, and keep feeds/reconciliation active.","Use durable worker acknowledgement and boot into RECOVERING then STOPPED."],
["PENDING remains visible until the worker applies and acknowledges the local fence.","Stop reaches DRAINING while prior emitted attempts are unresolved, then STOPPED; a paper run with no outstanding attempts can stop directly after application.","Old-generation queued results cannot be admitted after the fence.","Stop-all reports each session's command separately and makes no atomic global-stop claim.","Restart never automatically starts or arms a LIVE session."],
["Run deterministic lifecycle tests for delayed ACK, disconnected worker, duplicate commands, cancellation and recovery.","Inject an unresolved attempt fixture and verify DRAINING cannot be mistaken for complete settlement."],[PRD,ARCH,SEC])

ticket(12,"M1","Serve the authenticated control API and schema-consistent errors","Backend engineer","P0","3-4",[8,10,11],["PRD-F11","PRD-F12","PRD-F16"],
"The dashboard needs one reliable control contract. Implement research API operations with authentication, idempotency and command status semantics.",
["Implement health, session creation/list/detail, commands/status and opportunity queries from the OpenAPI contract.","Add single-operator authentication, CSRF protection for cookie-authenticated mutation and request size/rate limits.","Keep network credentials and internals out of browser responses."],
["API requests and responses validate against the committed schema.","Unauthenticated and cross-site mutation attempts fail without changing sessions.","DISARM/live-only actions in a research session return explicit forbidden/capability errors.","A command response never claims STOPPED solely because HTTP acceptance succeeded.","Errors include stable codes and correlation IDs while redacting secrets."],
["Run contract/integration cases for success, invalid input, revision conflict, idempotency and authentication failures.","Verify origin/CSRF handling and sensitive response/log redaction."],["specs/openapi.yaml",API,SEC])

ticket(13,"M1","Add bounded scheduling, stage telemetry and chain isolation","Systems engineer","P1","3-4",[7,8,11],["NFR-02","NFR-03","NFR-05"],
"Low latency requires bounded work and visible data age. Introduce explicit queues and CPU scheduling so a slow consumer cannot silently invalidate results.",
["Use Tokio for I/O and bounded CPU workers for quote/evaluation jobs.","Define per-stage deadlines, queue limits, generation tags and drop/resync policies.","Instrument ingestion, snapshot, quote, simulation, persistence and API stages with correlation IDs."],
["No unbounded task creation or unbounded spawn_blocking path exists for incoming market work.","Queue age and stale/drop reasons are exported by chain and stage.","Slow dashboard/analytics reads cannot block worker evaluation or control acknowledgement.","A blocked Solana worker does not stall the Base worker's control loop, and vice versa."],
["Run overload tests with a deliberately slow consumer and show bounded memory/work queues.","Capture an initial p50/p95/p99 stage profile on the named host without treating it as an SLA."],[ARCH,SEC])

ticket(14,"M1","Implement versioned capture manifests and fixture provenance","Data engineer","P1","2-3",[8,9,10],["PRD-F08","PRD-F13","NFR-08"],
"A stored opportunity is not reproducible unless its inputs and versions can be located. Define the capture manifest before observations begin accumulating.",
["Persist normalized/raw references, provider provenance, chain state, ingestion ordering and timestamps.","Hash capture objects and include schema, adapter, configuration and build versions.","Tag synthetic, recorded-live and manually constructed fixture origins explicitly."],
["Every decision can locate its declared input bundle or report why it cannot.","Corrupt hashes, missing objects and incompatible versions are rejected or explicitly quarantined.","Synthetic fixtures are never counted in market performance reports.","Retention metadata marks when raw inputs expire without rewriting the historical result."],
["Round-trip and corrupt a representative manifest to verify integrity and missing-input handling.","Review a capture export for provenance completeness and secret leakage."],[API,MODEL,SEC])

ticket(15,"M1","Deliver the first observation-to-control vertical slice","Technical Lead / QA","P0","2-3",[12,13,14,16,18,22,23],["PRD-F03","PRD-F06","PRD-F11","PRD-F12"],
"Prove the architecture on one real recorded chain observation after the shared platform and both quote adapters are available. Base is the demonstration chain for this integrated trace; this is not an earlier Base-only release or a claim of better economics.",
["Connect verified pool capture, a coherent snapshot, exact route evaluation, persistence and API presentation.","Show a recorded CANDIDATE decision with state age and excluded costs.","Exercise stop acknowledgement, restart and input replay from the same decision trace."],
["The demo identifies exact source/config/build versions and contains no invented market values.","A negative or rejected route is accepted as a valid demonstration result.","The operator sees command PENDING then APPLIED and actual worker state.","A restart returns STOPPED; gaps and missing data remain visible.","The integration gate records that both quote adapters are prerequisites; the Base demonstration does not by itself claim the dual-chain failure/qualification gate has passed."],
["Attach a redacted trace/capture and runbook reproduction steps.","Record measured stage latency and the stop/restart exercise outcome."],[PLAN,"docs/10-BUILD-HANDOFF.md",PRD])

# M2: real market adapters and route math.
ticket(16,"M2","Implement Base Uniswap V3 read-only ingestion","EVM engineer","P0","4-6",[2,3,8,9,14],["PRD-F02","PRD-F03","NFR-08"],
"Acquire the complete inputs required by supported Base pools without mixing unrelated blocks. Decode real on-chain data using verified identities.",
["Implement initial pool discovery from the approved manifest, block/log subscriptions and required storage/tick reads.","Pin deployment/ABI provenance and decode exact protocol units.","Capture block number/hash, parent relationships, provider context and pool update evidence."],
["All approved pool state needed for quoting is available and attributable to one coherent block reference.","Unexpected contract identity, malformed event and missing tick data fail explicitly.","Reconnect backfill is bounded and identifies data gaps.","The adapter advertises decode/read capabilities only until quote and simulation qualification complete."],
["Replay representative real capture fixtures and compare decoded values with official ABI/state queries.","Inject missing logs, malformed data and provider reconnects."],[ARCH,PRD,API])

ticket(17,"M2","Implement Solana Orca Whirlpool read-only ingestion","Solana engineer","P0","4-6",[2,3,8,9,14],["PRD-F02","PRD-F03","NFR-08"],
"Whirlpool quotes require validated accounts and tick arrays with defensible context. Capture enough state to make completeness and consistency explicit.",
["Read/subscribe to approved Whirlpool, vault, mint and tick-array accounts with program ownership checks.","Record slot/context, account write provenance and stream reconnect boundaries.","Enumerate supported token programs/extensions and reject unsupported behavior."],
["Pool and dependent accounts validate owner/program and configured asset identities.","Missing arrays or inconsistent account context cannot be treated as a complete valid quote state.","Provider semantics for account coherence are documented and unresolved coherence becomes a rejection reason.","Account decode version and source provenance are stored with captures."],
["Compare decoded representative account fixtures with the pinned official layout/reference implementation.","Test owner mismatch, unavailable arrays, duplicate/out-of-order writes and reconnect gaps."],[ARCH,PRD,API])

ticket(18,"M2","Build coherent snapshots, freshness gates and rollback handling","Chain engineers","P0","4-6",[16,17],["PRD-F03","PRD-F06","NFR-04"],
"A fresh quote built from inconsistent state is unreliable. Centralize state-completeness and chain-specific rollback rules before opportunity eligibility.",
["Build immutable per-chain snapshot IDs with completeness, coherence and age metadata.","Invalidate affected quotes on Base reorgs and Solana rollback/context changes.","Separate observation timestamps, chain state and comparison-window timestamps."],
["Stale, incomplete, forked or inconsistent state cannot produce ESTIMATED_EXECUTABLE.","A rollback invalidates dependent work and preserves an audit reason.","Each chain's finality/context policy is explicit and versioned.","No cross-chain shared atomic snapshot is implied by similar wall-clock times."],
["Replay gaps, reorgs, out-of-order updates and mismatched dependent-account fixtures.","Show a previously valid snapshot becomes ineligible without rewriting its captured provenance."],[MODEL,ARCH,PRD])

ticket(19,"M2","Qualify exact Uniswap V3 quote arithmetic","EVM engineer","P0","4-6",[8,16,18],["PRD-F04","NFR-01"],
"Concentrated-liquidity rounding and tick traversal determine whether a spread exists. Implement exact-input quotes against pinned protocol behavior.",
["Implement supported tick traversal, liquidity transitions, fees and amount rounding using checked integers.","Expose quoted output, included pool fees/price impact and explicit failure reasons.","Bound tick traversal and reject incomplete coverage rather than extrapolating."],
["Golden and protocol differential cases agree within the protocol's exact rounding rules.","Zero liquidity, tick boundaries, maximum supported amounts and arithmetic overflow are tested.","Fees already reflected in output are labeled as included, not charged again downstream.","Unsupported V2/V4 or other pool models never silently route through this implementation."],
["Publish a fixture corpus with pinned official implementation/source versions and expected outputs.","Run boundary, randomized differential and incomplete-tick tests."],[MODEL,ARCH,SEC])

ticket(20,"M2","Qualify exact Orca Whirlpool quote arithmetic","Solana engineer","P0","4-6",[8,17,18],["PRD-F04","NFR-01"],
"Whirlpool tick arrays and token units require chain-specific correctness. Implement exact-input quotes for the explicitly supported account/token configuration.",
["Implement or bind audited/pinned reference arithmetic with documented provenance and compatible licenses.","Handle tick-array transitions, fees, liquidity changes and prescribed rounding.","Reject unsupported token extensions and incomplete traversal inputs."],
["The adapter matches pinned official reference behavior for all qualification cases.","Boundary amounts, tick crossings, empty liquidity and unavailable arrays produce expected results.","The output declares included fees/impact and exact input/output asset units.","Token behavior outside the supported matrix cannot be accidentally accepted by matching a ticker."],
["Run golden/randomized differential cases against the pinned protocol implementation.","Review overflow and truncation behavior at serialization/math boundaries."],[MODEL,ARCH,SEC])

ticket(21,"M2","Enforce adapter capability readiness and registry activation","Technical Lead / Chain engineers","P0","2-3",[3,9,19,20],["PRD-F02","PRD-F04","PRD-F06"],
"An adapter that decodes a pool is not necessarily ready to simulate or trade it. Make readiness explicit in the registry and service contract.",
["Implement decode, quote, build, full-simulate and submit capability declarations by supported pool/token combination.","Tie activation to qualified registry versions and validation evidence.","Expose unsupported capabilities in API/operator diagnostics."],
["Configuration cannot enable a capability that the selected adapter has not qualified.","Research builds never advertise submit capability.","Adding another venue requires code/protocol validation, not only a TOML entry.","A registry with no eligible routes starts observation safely and explains the lack of coverage."],
["Test capability mismatch and changed token/program identity against activation validation.","Review the generated supported-venue matrix."],[PRD,ARCH,DEC])

ticket(22,"M2","Implement bounded distinct-pool cyclic route discovery","Engine engineer","P0","3-5",[8,9,19,20,21],["PRD-F04","NFR-02","NFR-03"],
"Discover only routes the implementation can evaluate correctly. Start with finite USDC cycles through two distinct supported pools.",
["Build chain-local directed asset/pool graphs from activated registry snapshots.","Enumerate USDC→WETH→USDC and USDC→wSOL→USDC across distinct eligible pools and configured sizes.","Apply queue/deadline bounds and deterministic route ordering."],
["Every route closes to its starting asset and never crosses networks.","The same pool cannot occupy both legs of a Phase 1 cycle.","Unknown pool models, three-leg routes and unconfigured sizes are rejected.","A cancelled or expired generation cannot publish an admissible new opportunity.","No-route results retain diagnostics rather than inventing market opportunities."],
["Use small graph fixtures with known valid, duplicate and invalid cycles.","Run high-pool-count stress tests against route/queue bounds."],[PRD,MODEL,ARCH])

ticket(23,"M2","Persist decision traces, rejections and opportunity deduplication","Engine engineer / Data engineer","P1","3-4",[10,14,18,22],["PRD-F04","PRD-F06","PRD-F12","PRD-F13"],
"Repeated observations must not inflate opportunity counts or hide rejected cases. Persist a trace that explains each result and groups persistent opportunities transparently.",
["Record snapshot, route, amount, quote method, checks, rejection reason and correlation IDs.","Define a versioned grouping key and persistence window while preserving raw observations.","Separate observations, unique opportunities, eligible attempts and reconciled transactions in query schemas."],
["Every displayed number can be traced to a calculation version and input state.","Grouping never removes rejected observations from research denominators.","Positive and negative quoted routes remain CANDIDATE until stronger evidence exists.","Queries distinguish zero qualifying routes from provider downtime or missing captures."],
["Replay repeated and changing-spread fixtures to verify grouping boundaries.","Compare aggregate counts to raw records and inspect one end-to-end decision trace."],[API,MODEL,PRD])

ticket(24,"M2","Run dual-chain adapter qualification and chaos gate","QA / Chain engineers","P0","3-5",[13,15,19,20,21,23],["PRD-F03","NFR-01","NFR-02","NFR-04"],
"Establish both adapters as reliable inputs before interpreting paper economics. Combine protocol agreement with real capture and failure behavior.",
["Run both qualification corpora and replay representative live captures.","Exercise provider failures, gaps, overload, rollbacks and malformed account/contract data.","Publish support matrix, open defects and measured freshness/latency budgets."],
["Each enabled pool type passes golden/differential and boundary checks.","Stale or incoherent state cannot pass executable eligibility in any supported path.","Known gaps and excluded assets/pools are visible to the operator.","M2 exits only when both chain adapters meet their documented support claims."],
["Attach CI reports, corpus/build hashes and named-host latency distributions.","Technical Lead signs off the qualification matrix; unresolved correctness defects block dependent release claims."],[PLAN,SEC,MODEL])

# M3: paper execution, full simulation and reproducibility.
ticket(25,"M3","Implement complete cost ledger and reproducible valuation","Engine engineer","P0","4-6",[8,23],["PRD-F05","PRD-F10","NFR-01"],
"A gross spread is not net economics. Record transaction costs, assumptions and reference conversions without double counting or treating missing fees as zero.",
["Represent DEX fees/impact, network fees, Base L1-data components where applicable, Solana priority fees/tips, funding and failure scenarios.","Store native-currency raw units with timestamped conversion provenance.","Separate gross, transaction-net and fully allocated operating results."],
["Quoted output's included pool fees and price impact are not subtracted again.","Unknown execution/funding/valuation inputs block complete-cost eligibility.","USDC-unit outcomes do not silently claim guaranteed USD value.","Operating overhead is separately allocated and its method/version is disclosed.","Negative outcomes and failed-attempt scenario costs remain in reports."],
["Run hand-calculated cost fixtures for both chains, unavailable inputs and signed P&L.","Verify aggregate ledger conservation and reference-conversion reproduction."],[MODEL,API,PRD])

ticket(26,"M3","Build virtual portfolios, principal and fee reservations","Engine engineer","P0","4-6",[10,25],["PRD-F07","PRD-F10","PRD-F17"],
"Paper fills cannot all use the same balance. Implement exact virtual accounting with separate trade principal and native-fee budgets.",
["Create immutable initial balances per new paper run and a journal of reservations/releases/settlements.","Reserve USDC principal and ETH/SOL fees independently for competing hypothetical attempts.","Expose free, reserved and total balances without valuing inventory changes as arbitrage profit."],
["Concurrent candidates cannot reserve more principal or fee balance than available.","A fees-only balance does not satisfy trade principal requirements.","Rejections, expiries and modeled failures release or spend reservations according to the explicit scenario.","A portfolio reset creates a new run instead of rewriting prior results.","Ledger replay reconstructs balances exactly."],
["Run concurrent reservation and crash/restart ledger tests.","Use a conflicting-opportunity fixture to prove mutually exclusive capital is respected."],[MODEL,PRD,SEC])

ticket(27,"M3","Implement deterministic offline replay and compatibility checks","Engine engineer","P0","4-6",[14,19,20,23],["PRD-F08","NFR-08"],
"Reproduce decisions using the state that was available at the time. Replay must remain deterministic and must not fill historical gaps from today's RPC.",
["Inject clock/event ordering and pin capture/config/adapter/build versions.","Provide CLI replay selection by manifest/run and deterministic result export.","Reject incompatible schemas and incomplete captures with explicit reasons."],
["The same inputs produce the same raw amounts, checks and trace ordering.","Offline replay makes no external network calls.","Missing inputs block full reproducibility claims rather than being silently reconstructed.","Local mathematical replay remains CANDIDATE regardless of a positive result.","Replay logs identify any platform/version limitations affecting determinism."],
["Run replay twice in isolated environments and compare canonical outputs.","Deny network access and remove a capture object to verify correct behavior."],[MODEL,API,SEC])

ticket(28,"M3","Build research-only Base transaction plans and atomic guard artifact","EVM engineer","P0","5-7",[19,21,25],["PRD-F06","PRD-F07","NFR-01"],
"Executable-paper evidence requires a complete intended atomic transaction, not two standalone quote calls. Build the Base plan and minimal local execution artifact.",
["Encode supported pool swaps, exact inputs, minimum outputs, deadline and route/pool allowlists.","Implement final starting-asset balance/profit guard with explicit pre/post accounting and reentrancy/callback authorization.","Create local-fork deployment fixtures and declared virtual funding/allowance setup."],
["The complete route executes atomically in the test harness and reverts when required final-balance/route guards fail.","Execution bytes and guard parameters produce a canonical plan digest.","Spending-account principal and allowances are explicit; a funded fee account is insufficient.","No production deployment, private key or real broadcast is included.","Tests cover unauthorized callbacks, unsupported targets and insufficient final balance."],
["Run contract/local-fork positive and rejection fixtures, including unprofitable synthetic states.","Review artifact provenance and attack surfaces; this research artifact is not an independent security audit."],[MODEL,ARCH,SEC])

ticket(29,"M3","Build research-only Solana transaction plans and final balance guard","Solana engineer","P0","5-7",[20,21,25],["PRD-F06","PRD-F07","NFR-01"],
"A Solana quote is insufficient to prove the complete instruction sequence is valid. Build a full supported route with enforceable atomic output conditions.",
["Construct complete swap instructions, account metas, compute-budget instructions and required token accounts.","Implement or qualify a minimal final-balance guard program where the selected execution design needs one.","Document virtual account funding, owners, authority and simulation constraints."],
["Both swap legs and the guard are part of one complete intended transaction plan.","Account ownership, token program, authority, writable set, limits and program allowlists are validated.","Plan bytes/digest include compute/fee/guard parameters and exact relevant accounts.","Insufficient final balance or invalid account/program identity fails atomically in the harness.","No production program deployment, private-key signing or broadcast is introduced."],
["Run full-plan local validator/simulation success and rejection fixtures under declared virtual funding.","Review compute/account-size bounds and guard bypass cases."],[MODEL,ARCH,SEC])

ticket(30,"M3","Implement complete Base simulation and exact-plan evidence","EVM engineer","P0","3-5",[18,27,28],["PRD-F06","PRD-F07"],
"Promote evidence only when the complete Base transaction successfully simulates against identified state. Preserve limits of fork/state-override assumptions.",
["Execute the exact recorded plan on an isolated block-pinned fork/read-only simulation environment.","Capture state, funding/allowance overrides, artifact versions, logs, balance changes and fee estimates.","Bind success to the exact plan digest and simulation state."],
["Successful full execution may earn SIMULATED; quote math or failed simulation never does.","Changed route/amount/guard/fee plan invalidates prior exact-plan evidence.","Unavailable historical/fork state or missing required overrides remains a capability gap.","Synthetic or overridden funding assumptions are visible and excluded from realized market claims."],
["Run success, revert, missing allowance, stale state and digest-mismatch cases.","Reproduce one complete simulation from the stored manifest without signing a production transaction."],[MODEL,API,SEC])

ticket(31,"M3","Implement complete Solana simulation and exact-plan evidence","Solana engineer","P0","3-5",[18,27,29],["PRD-F06","PRD-F07"],
"Promote evidence only from the complete Solana transaction under recorded account/state assumptions. RPC simulation and local fixtures have different limitations that must remain visible.",
["Implement read-only full-transaction simulation with pinned context where supported and an isolated local fixture path where needed.","Record exact instructions, account versions, virtual funding, compute consumption, program logs and result.","Tie evidence to plan digest and coherent-state policy."],
["Successful complete simulation may earn SIMULATED; instruction fragments and mathematical replay cannot.","Missing accounts, guard failure and unsupported token behavior produce explicit rejection evidence.","Provider inability to simulate declared virtual funding is not hidden; the UI shows the capability limitation.","No signature-capable key store or submission endpoint is introduced."],
["Run success/rejection cases and changed-account/changed-plan invalidation cases.","Reproduce one route simulation with all assumptions and relevant context retained."],[MODEL,API,SEC])

ticket(32,"M3","Model delay, inclusion and failure scenarios without false certainty","Research engineer","P0","3-5",[25,26,27,30,31],["PRD-F05","PRD-F07","PRD-F09"],
"Immediate quote fills overstate paper results. Apply explicit delay/future-state scenarios and account for rejection or lost opportunity without inventing win probabilities.",
["Evaluate configured quote-to-attempt delays against later captured state when available.","Define named inclusion/failure scenarios, expiry conditions and conservative external fee assumptions.","Track censored/missing-future-state cases separately from failures or zero profit."],
["Every modeled outcome names its delay/inclusion assumptions and available data window.","No unsupported fill probability or confidence interval is presented as measured fact.","Scenarios consume conflicting capital consistently and include rejected/expired attempts in denominators.","Changing a scenario creates a new experiment version rather than rewriting the original result."],
["Use fixtures where a quoted spread disappears, worsens or survives before the scenario delay.","Inspect missing-future-state and simultaneous-capital-conflict results."],[MODEL,PRD,API])

ticket(33,"M3","Enforce estimated-executable evidence eligibility","Engine engineer / QA","P0","2-3",[24,25,26,30,31,32],["PRD-F06","PRD-F07","PRD-F17"],
"A simulated transaction is still an estimate. Centralize the complete eligibility predicate and explain every missing prerequisite.",
["Require successful exact-plan simulation, fresh/coherent/complete state and complete costs.","Require principal/native-fee reservations, supported atomic guards, limits and a named delay/inclusion scenario.","Return structured pass/fail reasons for each check and preserve the weaker evidence when eligibility fails."],
["Removing any required predicate prevents ESTIMATED_EXECUTABLE.","A stale simulation, changed plan or absent fee valuation fails eligibility.","PAPER and REPLAY never progress to REALIZED.","The same eligibility function serves API, exports and dashboard; display logic cannot upgrade evidence."],
["Run a table of one-predicate-at-a-time negative cases plus a fully qualified case per chain.","Validate the resulting objects against opportunity.schema.json."],["specs/opportunity.schema.json",MODEL,PRD])

ticket(34,"M3","Implement fair comparison cohorts and holdout analysis","Research engineer / Product Owner","P1","3-4",[23,25,26,32,33],["PRD-F09","PRD-F10","PRD-F18"],
"Different coverage and capital assumptions can make chain rankings misleading. Build comparison cohorts with explicit matched windows and a separate native opportunity view.",
["Match observation windows, starting/reference assets, capital, route sizes and declared scenario assumptions.","Deduplicate persistent opportunities and enforce eligible sample/coverage rules.","Reserve time-based holdout periods and separate exploratory tuning from evaluation."],
["Comparisons show usable hours, gaps, excluded data, opportunity counts and scenario definitions.","Insufficient overlapping coverage suppresses ranking and explains why.","Chain-native opportunity sets are visible separately from matched experiments.","No current 'busiest' or 'best' chain claim is inferred from unmatched counts or synthetic data."],
["Test uneven uptime, no-opportunity periods, missing fee inputs and mismatched capital fixtures.","Review a sample methodology report with the Product Owner."],[MODEL,PRD,PLAN])

ticket(35,"M3","Pass executable-paper release gate and financial correctness review","QA / Technical Lead","P0","3-5",[24,26,27,30,31,33,34],["PRD-F05","PRD-F06","PRD-F07","PRD-F08","NFR-01"],
"Determine what the implementation may truthfully claim. A two-chain executable-paper release requires complete simulation capability, exact accounting and recoverable controls.",
["Run qualification suites and a representative end-to-end paper/replay scenario on both chains.","Review evidence transitions, assumptions, accounting conservation and excluded costs.","Publish a capability-based release decision with outstanding defects."],
["At least one supported atomic route per chain fully simulates under declared virtual funding, with success and rejection evidence.","Missing full simulation on either chain restricts release to clearly labeled quote-research preview.","No signing, key loading or transaction broadcast path is included in the research release.","Synthetic success fixtures prove capability only and never populate market-return reports.","All blocking arithmetic, evidence or reservation defects are resolved before exit."],
["Attach corpus/build/config hashes, actual CI results and an acceptance checklist.","Technical Lead and Product Owner record supported claims and remaining limitations."],[PLAN,SEC,MODEL])

# M4: approved dashboard, control UX and operational product.
ticket(36,"M4","Implement the approved dashboard shell and design tokens","Frontend engineer / UI Designer","P1","3-4",[6,8],["NFR-07","PRD-F12"],
"Turn the approved prototype into a maintainable application while preserving its readable research-focused hierarchy. The shell must make demo data unmistakable.",
["Implement React/TypeScript shell, dark/light tokens, sidebar and responsive layout using design/dashboard-wireframe.html as the baseline.","Provide Overview, Opportunities, Experiments, Runs, Strategies and System routes/views.","Keep mode, backend connection state, data age and session controls visible."],
["Both chain panels retain equal visual weight and the lime/navy design reference is recognizable.","Every synthetic/demo dataset is explicitly labeled and cannot be mistaken for captured market performance.","A chain filter only filters the view; it never changes worker scope.","No wallet-connect, private-key input or live-arm shortcut appears in the research UI."],
["Compare actual browser screenshots at desktop and narrow widths against the reference.","Run TypeScript/build checks and record any unavailable browser verification honestly."],["design/dashboard-wireframe.html",UX,PRD])

ticket(37,"M4","Connect dashboard queries, provenance and data-quality states","Frontend engineer / Backend engineer","P1","3-4",[12,23,36],["PRD-F03","PRD-F06","PRD-F12"],
"Replace demo-only presentation with clearly sourced backend data and honest loading/offline states. Every result should reveal its evidence and freshness.",
["Integrate typed API queries and controlled refresh/subscription behavior.","Display evidence badges, state references, age, provider health and coverage gaps.","Build an opportunity inspector with route, cost breakdown, checks and provenance."],
["Loading, empty, zero-opportunity, missing-data and disconnected states are distinct.","An unavailable cost displays unknown with a reason rather than zero.","A failed simulation is shown as a rejection and never receives a success evidence badge.","Disconnect freezes/labels the last known snapshot and does not continue animating invented activity."],
["Run frontend integration cases with valid, stale, incomplete, rejected and disconnected API fixtures.","Inspect one real recorded decision end-to-end against its API trace."],[UX,API,MODEL])

ticket(38,"M4","Implement experiment creation, immutable settings and capability feedback","Frontend engineer / Product Owner","P1","3-4",[9,12,21,36],["PRD-F01","PRD-F02","PRD-F07"],
"Operators must know what experiment they are starting and what the current adapters can support. Provide configuration review before creating a research session.",
["Build OBSERVE/PAPER/REPLAY setup with chain, registry, starting asset, sizes, virtual balances and scenario selection.","Show effective defaults, configuration digest and capability validation results.","Create a new session for changed modes or starting assets."],
["Unsupported pool models/token behavior cannot be selected as if supported.","The review screen distinguishes virtual principal from native fee reserves and displays assumptions.","Editing a completed/running experiment cannot mutate its recorded configuration.","No research form can enable LIVE by changing a hidden field or URL parameter."],
["Run keyboard-driven valid and invalid creation flows with API validation errors.","Verify immutable historical settings after creating a revised experiment."],[UX,PRD,API])

ticket(39,"M4","Implement start, pause, resume and stop acknowledgement UX","Frontend engineer / QA","P0","3-4",[11,12,36,37],["PRD-F11","PRD-F12","NFR-07"],
"The control must tell the operator whether a command is accepted or actually applied. Preserve truthful behavior through delayed worker responses and unresolved attempts.",
["Present per-session desired command, command status, observed state and last worker acknowledgement.","Implement stop-all as a visible per-session fanout with partial outcomes.","Show PAUSING/PENDING, applied fence, DRAINING and STOPPED meanings with accessible status updates."],
["API acceptance never immediately renders a completed stop without worker evidence.","Pause gates new work while feeds/reconciliation continue; resume requires a valid paused session.","DRAINING explains that previously emitted attempts remain unresolved and cannot be recalled.","Delayed/lost ACK and partial stop-all failures remain visible with safe retry/idempotency behavior.","A local UI/demo control cannot claim to stop a backend worker."],
["Run control integration tests with delayed ACK, duplicate clicks, disconnect and unresolved-attempt fixtures.","Verify keyboard focus, live status announcements and primary stop access on narrow screens."],[UX,PRD,SEC])

ticket(40,"M4","Build run history, portfolio ledger and comparison views","Frontend engineer / Research engineer","P1","3-4",[26,34,37,38],["PRD-F09","PRD-F10","PRD-F13"],
"Give the operator a way to explain results and compare experiments without collapsing evidence levels or accounting categories.",
["Show immutable run metadata, virtual balances/reservations and journaled balance movements.","Present gross, transaction-net and fully allocated results separately.","Build matched-chain comparison views with coverage, scenarios, holdout labels and excluded data."],
["Paper outcomes remain visibly hypothetical and never appear in a realized-profit total.","Inventory valuation movement, transfers and arbitrage outcomes are separate.","Insufficient comparable data produces an explanation instead of a winner badge.","Expired raw captures show a reproducibility limitation in run history."],
["Verify totals against ledger/API fixtures including losses and missing conversion rates.","Conduct an operator walkthrough explaining why one route was rejected and one comparison was withheld."],[UX,MODEL,PRD])

ticket(41,"M4","Implement reproducible JSON/CSV exports and redaction","Data engineer / Frontend engineer","P1","2-3",[23,27,34,40],["PRD-F13","PRD-F16","PRD-F18"],
"Research must remain inspectable outside the dashboard. Export amounts and methodology without losing precision or leaking credentials.",
["Export run metadata, summaries, decision records, ledger entries and capture references.","Include raw integer amounts, decimals, timezone, evidence labels, versions and assumptions.","Escape spreadsheet-formula-leading cells and redact provider/authentication secrets."],
["Large amounts round-trip without numeric precision loss.","Exports disclose unavailable inputs, excluded observations and hypothetical outcome labels.","CSV output cannot execute injected formulas when opened in common spreadsheet software.","Expired or missing capture dependencies are identified rather than omitted silently."],
["Round-trip JSON/CSV fixtures and compare raw amounts and row counts with the source query.","Test formula injection, quotes/newlines and credential redaction cases."],[PRD,API,SEC])

ticket(42,"M4","Instrument system health, alerts and actionable diagnostics","Operations engineer / Frontend engineer","P1","3-4",[13,24,37],["PRD-F03","PRD-F12","NFR-05"],
"The operator needs to distinguish a quiet market from broken infrastructure. Expose service health, backlog and data quality with actionable alert conditions.",
["Build System view metrics for providers, freshness, queue age/depth, drops, worker heartbeat and storage.","Define alert thresholds after measured baselines and show alert delivery failures.","Link diagnostics to redacted correlated traces and remediation runbooks."],
["No secret or signed payload is included in telemetry.","Alert conditions distinguish service outage, state incoherence and zero opportunity.","A blocked analytics query does not block worker control or data ingestion.","Every operational alert names the affected chain/session, last evidence and next safe investigation step."],
["Inject provider outage, stale snapshots, queue overload and alert transport failure.","Verify metrics/trace labels have bounded cardinality and useful correlation."],[SEC,ARCH,UX])

ticket(43,"M4","Pass dashboard accessibility, responsiveness and failure-state review","UI/UX Designer / QA","P1","2-3",[37,38,39,40,41,42],["NFR-07","PRD-F11","PRD-F12"],
"The approved appearance must remain usable when data or controls are under stress. Validate actual browser behavior rather than only static markup.",
["Review keyboard navigation, focus management, dialogs, color contrast and status announcements.","Inspect desktop/narrow layouts, long addresses, large amounts and dense cost tables.","Test empty, loading, offline, pending, rejected and partial-control states."],
["Core monitoring and stop workflows work without a mouse and without relying only on color.","Dialog focus opens/closes predictably and returns to its trigger.","Narrow layouts preserve mode, stale-data indication and control access.","Actual browser/test versions, screenshots and unresolved accessibility findings are recorded."],
["Run automated accessibility checks plus manual keyboard/screen-reader-oriented review.","Capture representative browser screenshots and a checklist of fixes; static checks alone do not satisfy this ticket."],[UX,PRD])

ticket(44,"M4","Ship the research deployment with backups and recovery proof","Operations engineer / QA","P0","3-5",[35,39,41,42,43],["PRD-F13","PRD-F16","NFR-04","NFR-06"],
"Package a usable research release that can be restored after failure and cannot move funds. Verify deployment behavior in the documented environment.",
["Provide local/private deployment configuration, health checks and operator setup instructions.","Implement database/capture backup, restore verification and retention jobs.","Run graceful shutdown, abrupt restart and degraded-provider exercises."],
["A clean environment can start the inert research deployment using documented steps.","Restore reconstructs configurations, commands, ledger and replay references with known retention limitations.","Restart returns workers to STOPPED and preserves unapplied commands for explicit handling.","The delivered process graph has no signer/broadcast capability and no public unauthenticated controls.","Release notes list actual test results, supported venues and remaining capability gaps."],
["Perform a restore drill into an isolated environment and compare manifests/record counts.","Record research release artifact hashes and the end-to-end recovery/stop evidence."],[SEC,PLAN,ARCH])

# M5: learn from real observations and decide whether to continue.
ticket(45,"M5","Register the observation campaign and analysis protocol","Product Owner / Research engineer","P0","2-3",[34,44],["PRD-F09","PRD-F18"],
"Predefine how the experiment will be assessed so strategy tuning does not quietly rewrite the success criteria. Calendar duration alone is insufficient.",
["Record configured chains/pools, capital, sizes, scenarios, cost allocation and comparison windows.","Define usable coverage requirements, holdout periods, exclusion policy and stopping conditions.","Plan an initial roughly 30-calendar-day campaign subject to adequate coverage and costs."],
["The protocol is versioned before collecting the primary evaluation window.","Exploratory tuning and held-out evaluation periods are labeled separately.","Coverage shortfalls extend or invalidate comparisons instead of becoming zero-opportunity evidence.","No campaign automatically escalates from paper to live."],
["Review the protocol with Product Owner, Technical Lead and operator.","Publish a dry-run report proving all required data/denominators are available."],[MODEL,PLAN,PRD])

ticket(46,"M5","Operate the campaign and keep a data-quality incident log","Operations engineer","P0","3-5",[45],["PRD-F03","PRD-F13","NFR-05"],
"A credible economics report depends on knowing where observations were usable. Run the protocol and document incidents and exclusions as they occur.",
["Monitor both chains, provider spending, storage, freshness and replay-sample availability.","Record outages, reconnect gaps, configuration changes and market-universe changes.","Keep daily/periodic coverage summaries and link remediation tickets."],
["Each campaign day identifies usable coverage per chain and reasons for exclusions.","Configuration changes split experiment versions and do not contaminate holdout data.","Synthetic tests stay segregated from campaign records.","Coverage/cost stopping conditions are enforced and operator-visible."],
["Sample raw captures against summary counts throughout the campaign.","Review the incident log and coverage report before analysis acceptance."],[SEC,MODEL,PLAN],"Estimate covers setup and periodic operator effort; the observation window is separate elapsed time, normally 4–6 weeks and longer if coverage is insufficient.")

ticket(47,"M5","Audit replay samples and investigate accounting or model discrepancies","QA / Research engineer","P0","3-5",[46],["PRD-F05","PRD-F08","PRD-F10","NFR-01"],
"Look for errors that a positive paper result could conceal. Reproduce stratified samples and reconcile reported economic components before drawing conclusions.",
["Sample accepted, rejected, expired, negative and missing-data cases across both chains and varied conditions.","Recompute quotes/costs from retained captures and verify ledger/aggregate consistency.","Track discrepancies by severity, affected data range and remediation."],
["Sampling methodology and sample count are disclosed.","Unresolved correctness discrepancies exclude affected results from decision-quality reports.","Missing captures are reported as unavailable evidence, not assumed reproduction successes.","Corrections produce new versioned analysis outputs with links to prior results."],
["Publish replay hashes, sampled case references and difference summaries.","Have a reviewer other than the primary calculation author inspect representative cases where feasible."],[MODEL,SEC,PLAN])

ticket(48,"M5","Produce the comparative economics and feasibility report","Research engineer / Product Owner","P0","3-5",[46,47],["PRD-F05","PRD-F09","PRD-F18"],
"Answer which tested configuration merits further work using recorded evidence and total costs. A finding of no credible advantage is a successful research outcome.",
["Report matched and native opportunity views, net outcomes, delay sensitivity, coverage and operating costs.","Discuss competition/inclusion uncertainty and limits of the paper model.","Compare continuing, narrowing, expanding observation or stopping the project."],
["The report identifies the exact protocol/data/config/version window and excludes synthetic fixtures.","No global busiest-chain claim is inferred from this limited market universe.","Estimated profits are labeled hypothetical and losses/failed scenarios are retained.","The recommendation names uncertainties that would change the conclusion and links reproducible exports."],
["Reconcile tables against exported records and disclose aggregation methods.","Product Owner reviews whether the report answers the agreed research questions."],[MODEL,PLAN,PRD])

ticket(49,"M5","Run operational and research-security readiness review","Technical Lead / Security reviewer","P0","2-4",[44,47],["PRD-F16","NFR-04","NFR-06"],
"Assess the sustained deployment and recovery evidence before proposing more responsibility. Fix weaknesses surfaced during the observation campaign.",
["Review authentication, secrets, backups, incident handling, dependency posture and retention.","Repeat only the recovery/failure drills affected by actual changes or unresolved incidents.","Publish remaining limitations and an owner for each remediation."],
["No unresolved research-critical security or accounting defect is hidden by release status.","Provider credentials remain absent from captures, logs and exports.","Restore and stop/restart procedures match current deployment behavior.","Review scope explicitly distinguishes research readiness from independent live-execution approval."],
["Attach current security/recovery findings and evidence references.","Verify remediation on realistic failure cases rather than checklist assertions alone."],[SEC,PLAN])

ticket(50,"M5","Record the go, revise or stop decision and optional live prerequisites","Operator / Product Owner","P0","1-2",[48,49],["PRD-F14","PRD-F17"],
"Keep the decision to risk real capital separate from technical progress. Present concrete evidence and choices for the operator.",
["Record whether to stop, extend research, narrow scope or consider one bounded live strategy.","For live consideration, identify capital/loss/fee ceilings, custody, venue permissions, review budget and target chain.","List evidence gaps and prerequisite approvals without enabling any execution path."],
["The chosen action and rationale cite the feasibility and readiness reports.","A positive paper result alone cannot satisfy the live gate.","Unanswered custody, limits, review or funding questions keep live work unarmed.","A stop decision includes export/retention and infrastructure shutdown instructions."],
["Operator records an explicit dated decision; agents may prepare but cannot substitute their approval.","Check downstream live tickets remain gated to the approved scope."],[PLAN,DEC,SEC])

# M6: optional live implementation, always gated.
ticket(51,"M6","Approve a concrete live execution design and limit specification","Architect / Operator","P0","2-4",[50],["PRD-F14","PRD-F17","NFR-06"],
"Define a narrow execution responsibility before introducing signing. Translate the operator's go decision into an exact design and enforceable permissions.",
["Select one initial reviewed chain/strategy, funding source, signer custody and submission mechanism.","Specify per-trade principal, inventory, fee, daily loss and pending-attempt limits with valuation/finality rules.","Document spending account/executor balances, allowances, allowed targets/programs and recovery ownership."],
["All live limits have explicit units, time/reset semantics and fail-closed behavior.","Fees and trading principal are separately funded at the actual authorized spending locations.","Approved scope excludes automatic multi-chain expansion or flash-loan activation.","The operator approves the exact proposal; this ticket's creation or engineering completion does not fund or arm it."],
["Threat-model the complete funds and authority flow with an independent reviewer.","Publish an approved configuration template containing no secrets or funded addresses unless intentionally chosen by the operator."],[ARCH,SEC,DEC])

ticket(52,"M6","Implement isolated signer policies, epochs and durable disarm","Security engineer","P0","5-8",[51],["PRD-F14","PRD-F16","PRD-F17"],
"A signer must enforce narrow transaction permissions independently of the dashboard. Disarm must revoke future signing authority durably, not merely hide a button.",
["Separate signer process/credentials and authenticate caller/session/configuration/epoch.","Validate decoded transaction targets, chain, values, guards, limits and permitted signing envelopes.","Persist epoch revocation before acknowledging DISARM; quarantine late signing responses after worker fences."],
["The signer refuses arbitrary payload signing and out-of-policy targets/amounts.","DISARM remains PENDING while the signer cannot durably acknowledge revocation.","APPLIED pause/stop is distinguished from signer revocation; already admitted operations may return bytes which are quarantined.","Restart cannot restore a revoked epoch or automatically arm a session.","Keys never enter browser, API logs, fixtures or ordinary exports."],
["Run policy-bypass, replayed-request, revoked-epoch, unavailable-signer and late-response tests.","Review IPC authentication and custody implementation with the independent reviewer."],[SEC,ARCH,PRD])

ticket(53,"M6","Harden execution artifacts and enforce live atomic constraints","Chain engineer / Security reviewer","P0","6-10",[28,29,51],["PRD-F14","PRD-F17","NFR-01","NFR-06"],
"Research simulation artifacts need production-specific hardening and review before they can hold approvals or enforce real transaction limits.",
["Harden only the selected chain's executor/guard and route encoders for the approved live scope.","Review callback/account/program authority, token behavior, allowances, reentrancy and asset accounting.","Produce reproducible builds, deployment manifest and verification instructions without deploying until approved."],
["Final-balance and minimum-output guards prevent violating approved transaction-level conditions.","Unauthorized targets, callbacks, accounts and unsupported token behavior fail atomically.","Upgrade/admin controls and any withdrawal/recovery authority are explicit and reviewed.","Artifacts are reproducible and traceable to source/build hashes.","No statement equates a transaction-level guard with guaranteed account-level profit after external fees."],
["Run adversarial fork/local-validator tests, invariant/fuzz suites and malformed-route cases.","Provide artifacts and threat assumptions for independent review before deployment/funding."],[SEC,ARCH,MODEL])

ticket(54,"M6","Implement live inventory, fee reservations and submission-time risk checks","Engine engineer","P0","4-6",[26,51],["PRD-F15","PRD-F17"],
"Live balance and limit checks must be performed against the account that actually spends funds. Prevent overlapping attempts from consuming the same principal or fee budget.",
["Reconcile actual chain balances, allowances/authority and reserved funds by account/executor.","Check live limits at submission admission using durable state and fresh fee bounds.","Block submissions on unavailable balances, unsettled discrepancies or exhausted caps."],
["Available fee balance alone cannot authorize a principal-consuming route.","Concurrent attempts cannot exceed principal, native-fee or pending-attempt limits.","Loss/fee limits use the approved accounting/finality policy and cannot be reset by a worker restart.","Changing limits requires a new approved configuration/revision and is audited.","State unavailability fails closed for new live admissions while reconciliation continues."],
["Run concurrency, stale-balance, allowance-loss, fee-spike and crash-recovery cases.","Verify ledger invariants against a local-chain/fork account model."],[MODEL,SEC,PRD])

ticket(55,"M6","Implement durable intent, signed-payload and dispatch-start journals","Backend engineer","P0","4-6",[10,52,54],["PRD-F15","NFR-04"],
"A network timeout does not prove a transaction was not sent. Persist enough state before each send to recover unknown outcomes without duplicating trades.",
["Journal intent/reservation and immutable transaction plan before signing.","Persist signed bytes/hash/reference and a synchronous dispatch-start UNKNOWN record before every network send.","Define recovery for each crash boundary and protected handling of signed payloads."],
["No network dispatch occurs before its durable dispatch-start record.","Signed bytes are treated as sensitive executable authority and excluded from general logs/exports.","A timeout or crash after dispatch-start remains UNKNOWN until reconciled.","Retries reuse a reviewed policy and never blindly re-sign/re-submit a replacement trade.","Analytics latency cannot bypass or replace the critical durable journal."],
["Fault-inject every boundary from reservation through returned network response.","Prove recovery enumerates every possibly emitted attempt, including transport exceptions and missing responses."],[SEC,ARCH,PRD])

ticket(56,"M6","Integrate the approved chain-specific submission transport","Chain engineer","P0","4-6",[53,55],["PRD-F15","PRD-F17","NFR-02"],
"Submit only approved exact plans through one explicit transport and preserve uncertain outcomes. Transport acceptance is not inclusion or profit.",
["Integrate the selected Base or Solana submission path with configured fee/tip bounds and expiry policy.","Bind admission to current command fence, signer epoch, plan digest and risk reservations.","Record transport responses, attempts and externally assigned identifiers without trusting them as final settlement."],
["No send begins after the local fence is applied, except an already in-flight network call whose outcome remains tracked.","Rejected/timeout/ambiguous responses map to explicit attempt states.","Nonce or blockhash validity and replacement policy are chain-specific and tested.","Submission responses cannot emit REALIZED evidence.","A second chain/transport remains disabled until separately qualified."],
["Use deterministic transport faults and isolated chain environments to exercise every response class.","Measure decision-to-dispatch timing separately from inclusion/settlement delays."],[ARCH,SEC,MODEL])

ticket(57,"M6","Reconcile pending, unknown, failed and finalized live outcomes","Chain engineer / Backend engineer","P0","5-8",[55,56],["PRD-F10","PRD-F15","NFR-04"],
"Turn chain evidence into reliable account outcomes without mistaking a missing response for failure. Settlement and rollback rules must be explicit per supported chain.",
["Track emitted attempts through pending, unknown, inclusion, failure, expiry and configured finality.","Reconcile actual balance deltas, gas/fees/tips and reservations into the ledger.","Handle Base reorgs or Solana commitment/rollback changes under the approved policy."],
["REALIZED is issued only after configured settlement and balance/cost reconciliation; losses remain valid outcomes.","Unknown attempts retain reservations according to the reviewed policy until resolved.","Unexplained balance discrepancies halt new submissions for the affected account.","Restart reconstructs all outstanding attempts before permitting new admission.","The UI/export exposes settlement policy and any later correction/rollback."],
["Run timeout-then-included, expired, failed, reorged and discrepant-balance scenarios.","Reconcile local/fork ledger totals against independent chain queries."],[SEC,MODEL,PRD])

ticket(58,"M6","Implement fencing, crash recovery and controlled takeover","Systems engineer / QA","P0","4-6",[11,52,55,57],["PRD-F11","PRD-F15","NFR-04"],
"Lease expiry cannot invalidate a signed transaction. Prevent simultaneous live authority and make takeover a reconciled, controlled operation.",
["Implement worker ownership/fencing tokens and a documented old-process termination/isolation procedure.","On restart or takeover, revoke/verify signer authority and reconcile outstanding attempts before rearming.","Preserve stop/pause/disarm semantics through unreachable components."],
["No automatic live takeover occurs based only on lease timeout.","Previously signed bytes and in-flight sends remain tracked even after signer epoch revocation.","A new worker cannot admit live work before prior ownership is fenced and unresolved attempts are reconciled per policy.","Boot remains unarmed STOPPED after RECOVERING; manual approved arming is required.","Stop-all exposes partial progress without claiming globally instantaneous revocation."],
["Exercise split-brain, long process pause, signer outage, network partition and crash during send.","Run the recovery procedure with a reviewer acting as the operator."],[SEC,ARCH,PRD])

ticket(59,"M6","Add explicit live readiness, arming and incident-control UX","Frontend engineer / Security engineer","P0","3-5",[39,51,52,54,57,58],["PRD-F14","PRD-F15","PRD-F17","NFR-07"],
"The future live interface must present the exact responsibility being accepted. Add gated readiness and arming only after backend enforcement exists.",
["Show selected chain, signer identity reference, executor version, route permissions, capital/fee/loss limits and configuration hash.","Require an authenticated explicit operator arm action with current readiness evidence.","Display stop, disarm, pending/unknown attempts, settlement policy and reconciliation alerts."],
["A PAPER session cannot mutate into LIVE; arming creates/uses a separate approved live session.","Missing review, signer acknowledgement, balances or limits blocks arming with specific reasons.","Stop and DISARM are distinct and their acknowledgements accurately reflect worker/signer application.","The UI never promises recall of emitted transactions or guaranteed net profit.","Authentication/CSRF and stale-revision checks cover every live mutation."],
["Run security and UI integration cases for stale approval, changed config, signer outage and partial disarm.","Complete keyboard/narrow-screen review of incident controls."],[UX,SEC,PRD])

ticket(60,"M6","Complete independent execution review and adversarial remediation","Independent reviewer / Implementers","P0","5-10",[52,53,54,55,56,57,58,59],["PRD-F16","PRD-F17","NFR-06"],
"Obtain qualified review outside implementation authorship before any funded execution. Automated agents and passing tests do not substitute for this gate.",
["Review signer, artifacts, allowances, authority, journal, transport, accounting, fencing and UI activation paths.","Classify findings and remediate/retest affected behavior.","Publish a scoped report with exact source/artifact versions and residual risks."],
["No unresolved critical/high finding affecting approved funds/authority remains at the live gate.","Review covers the exact intended deployment/configuration rather than an obsolete prototype.","Regression evidence addresses each fixed finding.","Review limitations and retained risks are visible to the operator.","Independent reviewer identity/scope is real and cannot be claimed by agent self-review."],
["Attach review deliverables, remediation commits and focused regression results.","Re-run affected adversarial scenarios; expand tests when findings justify it."],[SEC,PLAN],"Estimate is internal review/remediation coordination effort. External review pricing, reviewer availability and elapsed time are separate; findings can increase implementation effort.")

ticket(61,"M6","Prepare the bounded live pilot release and final approval packet","Technical Lead / Operator","P0","2-4",[50,60],["PRD-F14","PRD-F15","PRD-F17"],
"Make the final live decision concrete and reviewable. Assemble exact artifacts, configuration, limits, funding steps and recovery proof without activating prematurely.",
["Produce reproducible release/deployment hashes and approved configuration for one limited pilot.","Provide transaction review, funding/allowance steps, stop/disarm/recovery and incident escalation procedures.","Record independent review status, dry-run evidence and explicit operator acceptance conditions."],
["Every deployed artifact and policy can be matched to reviewed source and build output.","An operator can identify principal, fee reserve, maximum intended exposure and withdrawal authority before funding.","No deployment/funding/arm step is performed merely by merging this ticket.","The final activation packet requires explicit operator authorization for its exact version and limits.","Any unready gate leaves the pilot unavailable."],
["Conduct a tabletop drill and an unfunded isolated execution rehearsal.","Technical Lead verifies the packet and the operator records a go/no-go decision."],[PLAN,SEC,DEC])

# M7: optional pilot and separately scoped expansion.
ticket(62,"M7","Deploy and run one explicitly approved bounded live pilot","Operator / Operations engineer","P0","3-5",[61],["PRD-F14","PRD-F15","PRD-F17"],
"Test the reviewed execution path under narrowly bounded real conditions only after the final operator approval. The pilot prioritizes reconciled behavior over trading volume.",
["Deploy/verify only the approved artifact and configuration, then fund the declared principal/fee locations under approved limits.","Arm manually and observe a limited window/attempt budget with immediate incident control.","Retain every submission, unknown outcome, reconciliation and operator action."],
["Deployment, funding and arming match the exact written operator approval.","Limits are enforced and any discrepancy/unresolved critical incident halts new submissions.","All emitted attempts reconcile or remain explicitly unresolved; none disappear from the report.","The pilot does not automatically scale capital, add routes or enable the second chain.","Stop/disarm outcomes and residual pending exposure are verified at pilot end."],
["Compare actual chain transactions/balance changes with the durable journal and ledger.","Run the approved closeout checklist and preserve incident evidence."],[PLAN,SEC,MODEL])

ticket(63,"M7","Evaluate the pilot and approve continuation, reduction or shutdown","Product Owner / Operator","P0","2-3",[62],["PRD-F09","PRD-F10","PRD-F17"],
"Use actual reconciled outcomes to assess the gap between paper assumptions and reality. Continued operation is a separate decision from completing a pilot.",
["Compare realized fees, inclusion, failures, latency and P&L against the pre-registered paper scenarios.","Review incidents, unresolved outcomes, operational burden and fully allocated costs.","Propose continue, revise, reduce, stop or separately approve expansion."],
["Only reconciled settled transactions contribute to realized totals and the finality policy is disclosed.","Unresolved attempts and accounting differences remain explicit blockers where material.","No extrapolated guaranteed income is reported from a small sample.","Any increase in exposure or supported scope requires updated limits/review and operator approval."],
["Reconcile the report against chain evidence and exports.","Operator records the next decision with artifact/configuration versions and bounded scope."],[MODEL,PLAN,SEC])

ticket(64,"M7","Qualify a second venue per chain for research coverage","Chain engineers","P1","5-8",[35,48,50],["PRD-F02","PRD-F04","PRD-F06"],
"Broaden observable opportunities only after the first experiment is reliable. Candidate Sushi V2/Base and Raydium CPMM/Solana integrations require deployment and liquidity verification.",
["Verify official program/contract identities, liquidity, token behavior and data availability for one candidate at a time.","Implement pool-specific decode/quote/capture and capability qualification.","Add cross-venue two-leg routes with separate experiment versions and fair comparison coverage."],
["The expansion review records an actual qualified venue/pool set or rejects an unsuitable candidate.","Exact arithmetic passes golden/differential fixtures before activation.","Quote capability does not imply full simulation or live readiness.","Research expansion remains signing-free; live support needs the relevant M6 review and operator approval."],
["Run the new venue's full adapter corpus and cross-venue route fixtures.","Measure whether additional coverage changes conclusions without mixing unmatched time windows."],[PRD,ARCH,MODEL],"This is optional research expansion and does not depend on running a live pilot. Either named candidate may be replaced or rejected after current primary-source/deployment verification.")

ticket(65,"M7","Add reviewed token universes and optional three-leg research routes","Engine engineer / Chain engineers","P2","4-7",[64],["PRD-F01","PRD-F02","PRD-F04","NFR-03"],
"Additional coins require qualified token behavior, liquidity and computational bounds. Expand the route engine deliberately instead of allowing arbitrary ticker lists.",
["Introduce a reviewed token registry expansion with decimals, program/contract behavior and risk exclusions.","Implement bounded three-leg discovery only for supported pool types and explicitly configured experiments.","Extend paper balance, cost and replay tests for varied decimals and starting assets."],
["Unsupported rebasing, transfer-fee or extension behavior is rejected unless specifically implemented and qualified.","Every new route closes in the configured starting asset and stays within one chain.","Search complexity, queue limits and freshness budgets remain bounded.","New starting assets require new sessions and separate comparison/valuation assumptions.","No expanded route receives executable evidence without its own exact full-plan simulation and eligibility checks."],
["Run multi-decimal/token-behavior fixtures and bounded graph stress tests.","Review experiment comparability and additional risk assumptions before activation."],[PRD,MODEL,ARCH])

ticket(66,"M7","Qualify an additional chain through the adapter and operations contract","Architect / Chain engineer","P2","6-10",[48,50,64],["PRD-F02","PRD-F03","PRD-F06","NFR-04"],
"The modular design can support more chains, but a shared interface does not erase chain-specific correctness or operations work. Select one new chain based on evidence and cost.",
["Evaluate data access, liquidity, transaction/fee model, finality, language/SDK fit and operating budget.","Implement the selected chain's read/quote/capture/replay capabilities and lifecycle isolation.","Qualify full simulation separately and document any unsupported execution capability."],
["Selection cites current primary sources and an explicit experiment hypothesis, not an unverified popularity ranking.","The chain meets the same identity, arithmetic, freshness, evidence and recovery gates as Base/Solana.","The dashboard can show the chain without pretending cross-chain atomicity.","Paper/live capability levels remain separate and initial activation remains research-only."],
["Run a complete adapter qualification corpus and one end-to-end observation/replay slice.","Record measured resource cost and operational/finality limitations."],[ARCH,MODEL,PLAN],"Bridging and cross-chain arbitrage remain out of scope. This ticket is a bounded feasibility/initial adapter slice; complex chains may require a separately estimated implementation epic.")

ticket(67,"M7","Assess and implement an optional atomic funding adapter","EVM or Solana engineer / Security reviewer","P2","5-9",[51,53,60,63],["PRD-F14","PRD-F17"],
"Flash loans or other atomic funding add lender-specific authority and repayment conditions. Add one only if measured strategy economics justify the complexity.",
["Verify a real lender/deployment, asset liquidity, fees and callback/repayment semantics from primary sources and chain state.","Model funding fees and exact repayment into the complete transaction and final-balance guard.","Implement isolated fixtures and submit changes for fresh independent review before any live use."],
["Repayment is enforced atomically and unauthorized lender callbacks/targets are rejected.","Fees and capital assumptions are reflected once in economics and evidence eligibility.","Insufficient liquidity or unavailable lender support prevents activation.","The new adapter does not inherit prior live approval; updated artifacts, limits and operator approval are required.","Prefunded paper and existing approved operation continue to function without this adapter."],
["Run repayment-failure, callback-spoof, fee-change and insufficient-liquidity tests.","Attach an incremental independent review and an updated economic justification."],[MODEL,SEC,PLAN],"Optional later scope. Flash loans are not required for initial paper/live operation, and this estimate excludes substantial audit remediation or new lender-program development.")

ticket(68,"M7","Establish maintenance, change control and safe project closeout","Technical Lead / Operations / Operator","P1","2-4",[44,50],["PRD-F13","PRD-F16","NFR-04","NFR-06","NFR-08"],
"The project needs an end state whether it becomes useful software or a stopped experiment. Preserve evidence and prevent stale credentials or permissions from outliving operations.",
["Define dependency/provider/program change review, registry requalification and periodic restore checks.","Document research shutdown and, if live was approved, stop/disarm/reconcile/withdraw/revoke procedures with operator authority.","Archive reports, manifests and decisions under the retention policy and cancel approved unused infrastructure subscriptions."],
["Protocol/program changes invalidate affected qualification until reviewed.","Closing a research deployment exports evidence and removes credentials without claiming historical data is fully reproducible after expiry.","Closing live operation cannot treat unknown submissions as settled or skip operator-authorized custody/allowance handling.","Repository documentation states support status, unresolved issues and how to reproduce retained results.","Recurring work has an owner/cadence; scheduled external actions are created only when explicitly authorized."],
["Run a tabletop closeout from both research-only and unresolved-live-attempt scenarios.","Verify archival integrity and current restore instructions."],[SEC,PLAN,DEC])

MILESTONES = [
 ("M0","Scope, provenance and operating assumptions","Representative pool data is obtainable; registry/protocol scope and costs are understood; decisions have owners.","Product Owner / Technical Lead",[]),
 ("M1","Rust platform and first observation slice","Clean research build, durable commands, restart-to-STOPPED and a versioned real observation decision trace are demonstrated.","Technical Lead",["EPIC-01"]),
 ("M2","Qualified Solana and Base market adapters","Both initial pool families pass exact arithmetic and failure-state qualification; invalid state cannot pass eligibility.","Chain engineering leads",["EPIC-02"]),
 ("M3","Executable-paper simulation, accounting and replay","One complete supported atomic route per chain fully simulates under declared virtual funding; evidence, cost and reservation gates pass. Otherwise only quote-research preview is releasable.","Engine Lead / QA",["EPIC-02","EPIC-03"]),
 ("M4","Approved dashboard and research operations","Controls, comparison, exports, accessibility and research deployment recovery pass; no signer or broadcast capability ships.","Product Owner / Technical Lead",["EPIC-02","EPIC-03","EPIC-04"]),
 ("M5","Observation campaign and feasibility decision","Adequate documented coverage, sampled replay/accounting review and a candid economics report support a recorded go/revise/stop decision.","Product Owner / Operator",["EPIC-03","EPIC-04","EPIC-05"]),
 ("M6","Optional reviewed bounded live execution","Exact live configuration, independent review, signer/submission/reconciliation failure drills and operator decision are complete before activation.","Technical Lead / Operator",["EPIC-06"]),
 ("M7","Optional pilot, measured expansion and closeout","Each pilot/expansion has its own qualifying evidence and required approval; actual outcomes reconcile and long-term maintenance or shutdown is documented.","Operator / Technical Lead",["EPIC-06"]),
]

EPICS = []
for milestone,title,gate,role,deps in MILESTONES:
    eid=f"EPIC-{int(milestone[1:])+1:02d}"
    children=[t for t in TICKETS if t["milestone"]==milestone]
    minimum=sum(int(t["estimate_days"].split("-")[0]) for t in children)
    maximum=sum(int(t["estimate_days"].split("-")[1]) for t in children)
    marker=f"<!-- arb-ticket:{eid} -->"
    body="\n".join([marker,f"## {eid}: {title}","",f"**Milestone:** {milestone} · **Accountable role:** {role} · **Status:** planned", "", "### Product outcome", gate, "", "### Scope and child tickets",*[f"- [ ] {t['id']} — {t['title']}" for t in children],"", "### Acceptance and evidence",f"- [ ] {gate}","- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.","- [ ] Requirement and capability claims match code, tests, observed results and release notes.","- [ ] Blocking correctness, control, recovery or security findings are resolved before release.","", "### Dependencies and scheduling", "Prerequisite epics: "+(", ".join(deps) if deps else "none")+". Ticket-level prerequisites are authoritative; stages may overlap.", f"Child estimates total {minimum}-{maximum} working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.","M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.","", "### Scope/approval boundary","Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.","", "### References",f"- `{PLAN}`",f"- `{PRD}`","- `planning/BACKLOG.md`",""])
    EPICS.append({"id":eid,"kind":"epic","title":title,"milestone":milestone,"labels":["type:epic",f"phase:{milestone.lower()}","status:planned"],"role":role,"priority":"P0" if milestone!="M7" else "P1","estimate_days":f"{minimum}-{maximum}","estimate_is_rollup":True,"dependencies":deps,"status":"planned","release_gate":gate,"marker":marker,"children":[t["id"] for t in children],"body":body})

def validate():
    items=EPICS+TICKETS
    by_id={x["id"]:x for x in items}
    assert len(by_id)==len(items)
    assert len(EPICS)==8 and len(TICKETS)==68
    for x in items:
        assert x["body"].startswith(x["marker"])
        assert x["status"]=="planned"
        assert len(x["body"])>800
        for d in x["dependencies"]:
            assert d in by_id, (x["id"],d)
            assert d!=x["id"]
    visiting=set();done=set()
    def visit(i):
        if i in done:return
        assert i not in visiting, f"dependency cycle at {i}"
        visiting.add(i)
        for d in by_id[i]["dependencies"]:visit(d)
        visiting.remove(i);done.add(i)
    for i in by_id:visit(i)
    reqs={r for x in TICKETS for r in x["requirement_ids"]}
    assert all(f"PRD-F{i:02d}" in reqs for i in range(1,19))
    assert all(f"NFR-{i:02d}" in reqs for i in range(1,9))
    return {"epics":len(EPICS),"tickets":len(TICKETS),"issues":len(items),"dependency_cycles":0,"requirements_covered":len(reqs),"all_planned":True,"milestone_counts":dict(Counter(t["milestone"] for t in TICKETS))}

def write():
    check=validate()
    data={"version":"0.2.0","generated_date":"2026-09-12","project":"Arbitrage Research","status":"proposed delivery backlog; acceptance evidence remains required","estimate_unit":"working person-days across responsible roles; epic totals are rollups, not additional effort","epics":EPICS,"tickets":TICKETS}
    (ROOT/"backlog.json").write_text(json.dumps(data,indent=2)+"\n")
    (ROOT/"validation.json").write_text(json.dumps(check,indent=2)+"\n")
    lines=["# End-to-end delivery backlog","","Version 0.2.0 · 12 September 2026 · **8 epics and 68 work tickets (76 GitHub issues)**. Every item is planned. Existing scaffolding or a checked-in design is not proof that acceptance criteria have passed. The canonical machine-readable source is [backlog.json](backlog.json); [generate_backlog.py](generate_backlog.py) regenerates this package without network calls.","","## Scope and operating rules","","The initial product is a private Rust-first research platform for Solana and Base, using a React/TypeScript dashboard based on [the approved prototype](../design/dashboard-wireframe.html). Research includes OBSERVE, PAPER and REPLAY; it does not sign or broadcast. Initial routes are USDC-starting two-leg cycles through distinct qualified pools. Every added venue, token behavior, route shape or chain needs explicit qualification.","","M0–M4 implement the research product. M5 gathers evidence and records a go/revise/stop decision. M6 is optional live implementation and review. M7 groups an optional pilot and individually gated expansions/maintenance. M7 research expansion is permitted after its own research decision and does not require a live pilot. Ticket creation is never permission to deploy execution artifacts, fund accounts, use signing keys or activate LIVE.","","## Milestones and effort rollups","","| Milestone / epic | Work tickets | Proposed person-days | Exit gate |","| --- | ---: | ---: | --- |"]
    for e in EPICS:
        lines.append(f"| {e['milestone']} / {e['id']} | {len(e['children'])} | {e['estimate_days']} | {e['release_gate']} |")
    research=[t for t in TICKETS if t["milestone"]<="M4"]
    lo=sum(int(t["estimate_days"].split("-")[0]) for t in research)
    hi=sum(int(t["estimate_days"].split("-")[1]) for t in research)
    lines += ["",f"The detailed M0–M4 ticket estimates total **{lo}–{hi} working person-days across engineering, product, design, QA and operations roles**. This is a refinement input, not a promised schedule. Earlier high-level guidance was 22–36 engineering person-weeks plus 3–5 support person-weeks; the detailed range should be reconciled after provider/adapter spikes and actual CI qualification. Do not add epic rollups on top of ticket effort. Do not equate summed person-days with elapsed days.","","Assume two senior engineers with part-time product/UX/QA/operations support. Protocol arithmetic, state capture and complete transaction simulation dominate uncertainty. The observation campaign adds roughly 4–6 elapsed weeks and can extend for poor coverage. Live review availability, external review fees/remediation, infrastructure subscriptions, chain fees and trading capital are separate. Optional expansion estimates are scoped qualification slices and may expose further work; they are not commitments to support any arbitrary chain/token.","","## Dependency and gate conventions","","Dependencies in `backlog.json` are stable issue IDs. GitHub publishing should resolve them to actual links while retaining `<!-- arb-ticket:ARB-001 -->` markers for idempotency. Ticket prerequisites are authoritative. Epic order communicates stage-level release sequencing, not a rule that nobody may start dependent design work early.","","The first integrated trace (ARB-015) deliberately consumes shared snapshot and route work listed under M2, which in this backlog requires both quote adapters. Base is the demonstration chain, not an earlier Base-only release. Those component tickets do not depend on ARB-015, so the graph remains acyclic. The paired-chain failure/qualification gate ARB-024 does depend on the integrated trace. UI work can start with the approved design and typed contracts while adapters are in progress.","","```mermaid","flowchart TD","  Decisions[\"M0: Scope and provenance\"] --> Platform[\"M1: Shared platform\"]","  Platform --> Adapters[\"M2: Qualified adapters\"]","  Platform --> UI[\"M4: Dashboard integration\"]","  Adapters --> Paper[\"M3: Full simulation and paper\"]","  Paper --> UI","  UI --> Study[\"M5: Observation and decision\"]","  Study --> Research[\"M7: Optional research expansion\"]","  Study --> Live[\"M6: Reviewed live capability\"]","  Live --> Pilot[\"M7: Approved pilot\"]","```","","## Acceptance policy","","- Evidence labels remain CANDIDATE, SIMULATED, ESTIMATED_EXECUTABLE and REALIZED with their strict meanings. Local math/replay is CANDIDATE; full successful exact-plan simulation is required for SIMULATED. Paper never emits REALIZED.","- Executable-paper comparison requires at least one fully simulatable supported atomic route per chain under declared virtual funding plus complete costs, coherent/fresh state, principal/native-fee reservations, limits and atomic guards. Without the capability, release a clearly labeled quote-research preview only.","- Start/pause/stop distinguish command acceptance/PENDING from durable worker application/APPLIED. DRAINING retains unresolved previously emitted attempts. No control recalls a transaction already sent.","- Critical live journals record intent, signed payload and dispatch-start UNKNOWN before every network send. Signer DISARM needs durable epoch acknowledgement. Lease expiry does not revoke signed bytes and cannot alone authorize automatic takeover.","- Close an issue only after its acceptance evidence exists. An agent review is useful design input and does not count as independent execution security review.","","## Ticket index","","Each body is fully specified in [backlog.json](backlog.json) and also rendered in [TICKETS.md](TICKETS.md). Roles express responsibilities; no real person is assigned automatically.","","| ID | Milestone | Priority | Responsible role | Estimate | Title / prerequisites |","| --- | --- | --- | --- | --- | --- |"]
    for t in TICKETS:
        deps=", ".join(t["dependencies"]) or "none"
        lines.append(f"| {t['id']} | {t['milestone']} | {t['priority']} | {t['role']} | {t['estimate_days']} d | {t['title']}<br>Requires: {deps} |")
    lines += ["","## Requirement coverage","","| Requirement | Ticket evidence |","| --- | --- |"]
    reqs=sorted({r for t in TICKETS for r in t["requirement_ids"]})
    for r in reqs:
        lines.append(f"| {r} | "+", ".join(t["id"] for t in TICKETS if r in t["requirement_ids"])+" |")
    lines += ["","## Completion and verification record","","The generator checks unique IDs, required metadata, stable issue markers, valid prerequisites, absence of dependency cycles, 18 functional requirements and eight nonfunctional requirements. [validation.json](validation.json) records package integrity only; it does not claim software tests, GitHub publication, browser review, protocol correctness or trading safety have passed.","","Open decision roles, acceptance gates and assumptions are in [the PRD](../docs/01-PRD.md), [delivery plan](../docs/07-DELIVERY-PLAN.md) and [decision register](../docs/09-DECISIONS-AND-OPEN-QUESTIONS.md). Keep these documents and ticket scope synchronized when decisions change.",""]
    (ROOT/"BACKLOG.md").write_text("\n".join(lines))
    (ROOT/"TICKETS.md").write_text("# Complete issue specifications\n\nVersion 0.2.0. Generated from the canonical backlog definition. All issues are planned; acceptance evidence and optional live authorization remain outstanding.\n\n"+"\n---\n\n".join(e["body"] for e in EPICS+TICKETS))
    print(json.dumps(check,indent=2))

if __name__=="__main__":
    write()
