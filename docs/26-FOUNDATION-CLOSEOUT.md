# Foundation closeout against the original backlog

Review date: 14 September 2026. Existing issues only: ARB-002 (#15),
ARB-004 (#17), ARB-007 (#20), ARB-008 (#22). No scope/dependency edits.
Reviewer is the implementing assistant acting as the user-authorized integrating
maintainer under CONTRIBUTING.md, not an independent security reviewer.

## Why the count stayed high

The original backlog contains 68 task specifications and eight parent epics.
Only three original tasks were accepted at the starting checkpoint. The 74 open
issues therefore comprise 65 original tasks, eight epics and one additional
agent-runtime setup issue. Eighteen original tasks are explicitly gated later
live/pilot/expansion work (M6/M7); six are campaign/assessment tasks (M5).
Among the 68 tasks, the starting register has 22 implementations awaiting
acceptance, 11 in progress, 32 planned and three completed. These categories
are not equally sized and are not percentages of product completion.

Some later status notes asked for more than the original ticket. The dependency
graph then spread that extra blocking condition through almost the whole
foundation. In particular, ARB-002 explicitly permits documenting a blocking
provider gap; ARB-004 asks for operating assumptions and a sample estimate, not
proof of production throughput. ARB-007/008 are build/domain contracts, not
market-qualification tickets. Their original dependencies still apply, in order.
Real campaign and production gaps remain on the existing qualification,
deployment and campaign tickets, not deleted or silently accepted.

## ARB-002: bounded provider feasibility decision

Proposed paid RPC ceiling: **EUR 0 per month** for the current development and
feasibility phase. No paid plan, account, credit or API key is provisioned.
The repository owner `makafeli` is the required cost approver; no additional
paid-purchase authority or approved paid cost owner is inferred. Any paid
purchase is blocked until that owner explicitly approves both a named cost
owner and a ceiling. This meets the original pre-purchase condition without
inventing a procurement approval. Existing Railway hosting spend is separate.

### Primary-source shortlist, retrieved 14 September 2026

| Candidate | Access and published behavior | Decision for this phase |
|---|---|---|
| Base official public RPC | The network reference documents `mainnet.base.org` and chain ID 8453. No account is needed for this bounded diagnostic. Numeric service limits, historical depth and SLA are not established by that page. | Sample only; no sustained or complete-state qualification assumed. |
| Alchemy Base RPC | Official quickstart requires an account API key. Throughput is account-scoped; batching has a separately documented contract. Exact plan allowances, archival depth, redistribution terms and reconnect behavior must be checked for the chosen account. | Documentation-only alternative, not sampled because no authorized key/plan was supplied. No purchase. |
| Solana official public RPC | Public mainnet endpoint is documented. The published limits include 100 requests/10s/IP and 40 per method/10s, explicitly subject to change. Public service is not intended for production applications. HTTP 403 and 429 represent access/rate refusal, not chain inactivity. | Sample only; stop at access/rate refusal. No production entitlement assumed. |
| Helius Solana RPC/streaming | Official documentation describes standard subscriptions and provider-specific extensions, account keys and metered streaming. Reconnection requires explicit client handling; automatic complete historical recovery is not assumed. | Documentation-only alternative without an authorized key/plan. No stream opened, purchase or benchmark claim. |

Sources:
- https://docs.base.org/get-started/connect-to-base
- https://www.alchemy.com/docs/reference/base-api-quickstart
- https://www.alchemy.com/docs/reference/throughput
- https://www.alchemy.com/docs/reference/batch-requests
- https://solana.com/docs/references/clusters
- https://solana.com/docs/rpc/http/getprogramaccounts
- https://www.helius.dev/docs/rpc/websocket
- https://eips.ethereum.org/EIPS/eip-1898

Only documented standard HTTP requests are required by the current read-only
slice. Base code/state queries are pinned by block hash with requireCanonical;
unsupported pinning is a blocking feature gap, never a fallback to latest.
Solana requests retain finalized account context and bounded discovery slices.
Discovery slices do not contain complete amount-specific tick/quote state.
Websocket/gRPC subscriptions, arbitrary historical RPC retention and automatic
reconnect backfill are **not qualified or silently required**. The current
fallback is a recorded unavailable interval, not fabricated zero opportunities.
Provider data-retention/redistribution rights and sustained service limits are
unverified, so no campaign service or third-party data redistribution is approved.

`qualification-fixtures/probe_qualification.py` reuses the existing reviewed
public-only transport and addresses. Default invocation performs no network I/O.
An explicit diagnostic uses three samples per chain, at most 24 sequential
read-only requests in total, at most 2 MiB per response and ten-second request
timeouts. It stops a chain on 403/429 and never tries alternate endpoints.
Chains are sampled independently so one failure does not suppress evidence from
the other. Exact public requests/success responses, response hashes, HTTP status,
UTC time and measured duration are retained. Error bodies are removed. Statistics
use nearest-rank p50/p95/p99 **per method**, separating success from failures.
Small-n failure timing is not useful RPC latency, throughput or an SLA. Socket
timeouts are not a hard whole-process deadline; CI adds a job-level deadline.

**Observed-result section:** The opened-PR diagnostic artifact must be read and
its actual outcome attached before this assessment is accepted. Configuration,
mock tests or a successful diagnostic process do not themselves prove provider
availability. Even successful samples leave full tick coverage, mainnet program
qualification, archival/reconnect behavior and sustained service unqualified.
The valid assessment outcome may be **blocking gaps documented** as expressly
permitted by the original criterion; it cannot be reported as a qualified provider.

## ARB-004: reproducible assumptions and transparent 24-hour budget

`benchmark-environment.json` defines a Linux x86_64 host hypothesis with 8 logical
CPUs, 16 GiB RAM and 256 GiB storage. The five separate service budgets cover
Base, Solana, API, PostgreSQL and web. Each chain has a private single-writer
capture volume; API/database remain private behind the web's same-origin proxy.
The deployment platform is the already-established Railway target in
`21-RAILWAY-DEPLOYMENT-VERIFICATION.md`. The manifest is **not an applied Railway
resource change**, a paid order, measured capacity, or a claim that workers run.
The tracked Dockerfiles, pinned Rust toolchain and Cargo lock identify the
reproducible software inputs. Record actual runner/deployment hardware and source
SHA with every benchmark, and compare it with this declared target.

The illustrative rate is two networks x 0.5 batches/second x two per-pool
bundles, giving **172,800 captures/day**. Per retained capture: 65,536 raw bytes,
2,048 manifest bytes and 4,096 summary bytes. Shared transcript copies are counted
for each retained bundle; no compression or free provider archive is assumed.

| Component | Formula | Bytes |
|---|---|---:|
| Raw, one day | 172800 x 65536 | 11324620800 |
| Manifests, one day | 172800 x 2048 | 353894400 |
| Summaries, one day | 172800 x 4096 | 707788800 |
| Online raw | One day | 11324620800 |
| Online manifests | 30 days | 10616832000 |
| Online summaries and index/row allowance | 30 days x 2 | 42467328000 |
| Seven daily capture-backup sets | 7 x (raw + manifest + summary per day) | 86704128000 |
| Total before reserve | Sum of online and backup allocations | 151112908800 |
| Total including 30% reserve, rounded up | ceil(total x 13 / 10) | 196446781440 |

The estimate is about 183 GiB including reserve, below the declared 256 GiB.
It is a sample input-rate model, not an assertion of actual storage size/cost.
The backup allowance is for daily capture sets, **not seven full copies of the
entire 30-day database**. Full-database snapshots, WAL growth, replication,
provider fees and transfer charges require separate measured operational sizing
before enabling a campaign. Capacity/cost/retention roles are named in the
manifest. No Redis, Kafka or Kubernetes dependency is introduced.

After online raw expiry, retained manifests can still identify original objects
and hashes, but cannot reconstruct missing bytes or prove a fresh replay. A
backup only restores replay eligibility after actual restoration, integrity and
compatibility checks. Summaries preserve reported numbers, not missing market
inputs. Audit reports keep expiry and physical availability distinct. Initial
queue (64/network) and RPC-deadline (10s) values are hypotheses to calibrate,
not globally applied configuration or universal latency/throughput guarantees.

## ARB-007 and ARB-008: criterion-level source review

These two implementations are already present on main. Acceptance requires
fresh exact-source full CI and accepted predecessors, not new market features.

| Original criterion | Existing implementation and required evidence |
|---|---|
| ARB-007 clean Linux build and research checks | `Cargo.toml` has 17 research workspace members; Rust 1.90.0 is pinned; the normal specifications/rust/web/containers jobs must pass for the reviewed head. |
| ARB-007 real Cargo lockfile | `Cargo.lock` is generated/resolved through Cargo in recorded CI; subsequent SHA2/Base64 compatibility PRs retain checked package checksums. `cargo metadata --locked` and full locked build validate the graph. |
| ARB-007 no shipped key backend/broadcast entry point | Workspace packages contain no signer application or wallet/key backend. `arb-adapter-api` exposes an allowlisted read-only RpcMethod enum, not arbitrary outbound method names; `wire_enum_cannot_deserialize_broadcast` rejects submit names. Underlying generic HTTP libraries or administrator access are not claimed impossible. |
| ARB-007 required tools fail visibly | CI installs the pinned toolchain and real PostgreSQL, builds the API before worker integration and runs mandatory tests. Missing TEST_DATABASE_URL is an explicit failure, not a skip; optional preparation steps do not replace required job results. |
| ARB-008 large JSON values remain strings | `AtomicAmount`/`SignedAmount` use canonical decimal strings, private validated representations and custom serde; `wire_amounts_remain_strings` covers unsafe-JavaScript-sized values. |
| ARB-008 invalid identities/route/decimals/overflow rejected | Network-address/mint constructors, distinct fixture identity, two-leg route continuity and checked U256/U512 arithmetic; boundary and route-mutation tests. |
| ARB-008 no evidence promotion | Opportunity/lifecycle invariants reject REALIZED in PAPER/REPLAY and SIMULATED without complete-plan evidence; arithmetic-is-not-simulation and research-invariant tests. |
| ARB-008 unknowns stay explicit | Nullable costs, DATA_UNAVAILABLE, unresolved attempt identity and UNKNOWN outcomes stay separate from known zero/failure. Decision and lifecycle tests preserve this. |

Original ARB-003/009, deployment, adapter, economic, simulation, campaign and
independent live-review requirements remain unaccepted. A valid type cannot
prove honest inputs, provider truth or profitable transactions.

## Acceptance sequencing

Record actual probe findings, final CI and named self-review, then merge. Use the
existing closeout preflight and current checked native criteria in order:
ARB-002, ARB-004, ARB-007, ARB-008. Mark each completed only after its real issue
closure, then synchronize the register. Do not close later blocked tickets,
create replacement issues or rewrite the original criteria to improve counts.
