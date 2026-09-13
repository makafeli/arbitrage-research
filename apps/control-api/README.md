# Authenticated research control API

This Rust/Axum process implements the private operator HTTP boundary and uses PostgreSQL
for configuration snapshots, immutable research sessions, revisioned command intent and
receipts. It supports the operations in [`../../specs/openapi.yaml`](../../specs/openapi.yaml).

The service does **not** collect market data, quote pools, sign transactions or broadcast.
`GET /v1/capabilities` reports those process limitations explicitly. Research workers may
admit durable decision records; the API reads those records from PostgreSQL. Synthetic and
manually constructed datasets retain their explicit provenance and cannot appear as captured
market data. The API never substitutes dashboard fixtures for a missing database result.

## Start locally

Use the [PostgreSQL development environment](../../deploy/README.md), then supply the
connection string and a randomly generated operator secret through your process environment.
Do not put real credentials into tracked configuration, shell history or browser storage.
For example, the following reads both values from the terminal without echoing them:

```sh
read -r -s -p 'PostgreSQL connection URL: ' ARB_DATABASE_URL
export ARB_DATABASE_URL
read -r -s -p 'Operator secret (32+ random bytes encoded as text): ' ARB_OPERATOR_SECRET
export ARB_OPERATOR_SECRET
export ARB_CONFIG_FILES=config/research.example.toml
export ARB_PUBLIC_ORIGIN=http://127.0.0.1:5173
export ARB_ALLOW_INSECURE_LOOPBACK=true
cargo run -p control-api
```

The HTTP listener defaults to loopback. The port is selected from `ARB_API_PORT`, then
`PORT`, then 8080. `ARB_API_BIND_IP=0.0.0.0` or `::` enables a container listener only
when `ARB_PUBLIC_ORIGIN` is explicitly configured to an HTTPS origin and the insecure
cookie exception is disabled. Keep the API container on the private service network;
publish the dashboard reverse proxy as the single browser origin. Binding a container
interface does not enable live execution or bypass operator authentication.
The explicit insecure exception is accepted only for an HTTP `localhost`, `127.0.0.1` or
`[::1]` origin. For HTTPS behind a local reverse proxy, omit the exception and configure
`ARB_PUBLIC_ORIGIN` to the exact HTTPS origin. The default is `https://localhost:8443`.
The proxy must route `/v1` to the loopback service and enforce HTTPS on the public boundary.
No CORS origins are granted by the API. A local Vite proxy must preserve the configured browser Origin.

The shipped configuration intentionally disables both networks. You can authenticate and
inspect capabilities, but cannot create a session for a disabled network. Session creation
requires a registered, validated configuration containing the requested mode, enabled network
and supported strategy IDs. Structural configuration acceptance is not a claim that RPC,
registry qualification or trading readiness has been established. `ARB_CONFIG_FILES` accepts
up to 16 comma-separated paths. Changing a configuration requires restarting the API and
creating a new session with its new digest; existing session snapshots stay immutable.

Startup checks the database, applies embedded migrations, and saves immutable validated
configuration snapshots. Startup errors redact connection details and secret values.

## Authentication and request contract

- `POST /v1/auth/login` receives `{ "operator_secret": "..." }` with the exact configured
  `Origin`. It generates independent random 256-bit session and CSRF tokens. The response
  sets an `HttpOnly; SameSite=Strict; Path=/` cookie with `Secure` by default and returns
  `{ "operator_id": "operator", "csrf_token": "...", "expires_at": "..." }`.
- `expires_at` is Unix epoch seconds as a decimal string. Sessions expire after eight hours.
  At most eight sessions exist in the bounded in-memory store. Restarting the API revokes
  all browser sessions; durable trading/research session records remain in PostgreSQL.
- `GET /v1/auth/session` restores the CSRF token from an existing cookie after page reload.
  The dashboard keeps the token only in memory. The operator secret is never returned.
- Every authenticated mutation requires exact `Origin` and `X-CSRF-Token` values. Duplicate
  values, missing headers, wrong tokens and foreign origins are rejected before storage calls.
- `POST /v1/auth/logout` revokes the server session and expires the cookie.
- At most 64 request handlers run concurrently; additional requests receive 503
  `CAPACITY_EXCEEDED`. Each handler has a 15-second deadline. A 504 `REQUEST_TIMEOUT`
  means completion is uncertain: a database commit might already have occurred. Retry
  session creation or commands with the **same** idempotency key and payload. Cancellation
  releases the concurrency permit and never asserts that durable intent was cancelled.
- Requests are limited to 16 KiB. Login attempts have a global fixed-window limit of 10 per
  minute, including failures; each authenticated session has 300 requests per minute.
  A limited request returns 429 with `Retry-After: 60`. Restart resets process-local limits;
  the reverse proxy should enforce deployment-level connection limits.
- Responses use `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, and a generated
  `X-Request-ID`. JSON errors contain stable `code`, `message`, and matching `request_id`.
  SQL details, supplied secrets, RPC references and full configuration snapshots are omitted.

## Durable controls

`POST /v1/sessions` requires `Idempotency-Key` and returns a new **RECOVERING** session.
A worker must reconcile it before it can report STOPPED and become eligible for START.
The request includes `network_id`, `mode`, `configuration_digest`, `experiment_id` and
nonempty allowlisted `strategy_ids`. LIVE creation returns 403. An identical idempotent
retry returns the original session; reusing a key with a different payload returns 409.

`POST /v1/sessions/{session_id}/commands` durably accepts revisioned START, PAUSE, RESUME
or STOP intent. A 202 **PENDING** receipt means accepted into storage; it never means the
worker has applied the command. Poll `/v1/commands/{command_id}` and the session record.
The API cannot manufacture a worker fence or clear outstanding attempts. DISARM returns
403 because live signing is unavailable. Retry uncertain HTTP outcomes with the same key
and payload; do not fabricate a new command after a timeout.

Session and opportunity pages accept limits from 1 to 100. Session cursors are stable
opaque pagination values returned by the server; they are not authorization tokens.
The `/v1/health` route checks database readiness. The separate public `/healthz` route
is only process liveness; neither route asserts provider or worker readiness.

Ctrl-C stops the HTTP process. It does not issue STOP to workers. To stop research,
submit STOP while the API is running and verify the applied receipt and observed state.

## Persisted decisions and hypothetical portfolios

`GET /v1/decisions?session_id=...` returns pages of `{trace_id, recorded_at, trace}`.
`GET /v1/decisions/{observation_id}` returns the same durable wrapper. Trace outcomes
remain QUOTED, REJECTED, NO_ROUTE or DATA_UNAVAILABLE. QUOTED is CANDIDATE evidence:
the output includes pool fees and price impact, while missing external costs leave net
unavailable. A negative gross difference remains a recorded candidate, not a winning trade.
All routes are scoped to the authenticated operator, including cursor pages and details.

`GET /v1/opportunities` projects only eligible stored QUOTED records. Network, session,
evidence and `source_kind` filters run before pagination; rejected rows cannot consume a
page or hide later eligible records. Use `source_kind=CAPTURED_MARKET_DATA` for a captured
data view. Version 1.1 includes `dataset_origin`; SYNTHETIC and MANUALLY_CONSTRUCTED
use SYNTHETIC_FIXTURE and fixture identities. Unknown external costs produce
`net_after_explicit_costs_minor:null`, never a fabricated zero or gross-as-net amount.

`GET /v1/decision-groups?session_id=...` provides paginated counts by reproducible
grouping key with explicit origin. `GET /v1/decision-coverage?session_id=...` counts
raw observations and their outcomes separately from distinct quoted groups. An empty
session has zero recorded observations and null coverage window boundaries. Collection
completeness is UNKNOWN; this does not prove zero arbitrage in a chain or period.
Execution-accounting fields are null with `execution_accounting_available:false`.
Capture metadata and paper journal rows are not reconciled transactions.

`POST /v1/sessions/{session_id}/paper-runs` accepts only `initial_balances` with typed
token/native asset identities and integer minor-unit strings. It requires the same
Origin, CSRF and idempotency controls as session creation. The session must be PAPER,
STOPPED and without a pending command. Assets must belong to its registered, frozen
configuration; the authenticated capability response exposes those `paper_assets`
without secrets or RPC endpoints. Initial virtual capital and configuration are immutable:
start a new run to change them. The same key and payload replays the accepted run even
after API restart or removal of that configuration from current choices. Changed payloads
conflict. A revision difference alone is not treated as proof of a pending command.

`GET /v1/sessions/{session_id}/paper-runs` lists runs. `GET /v1/paper-runs/{run_id}`
returns balances consistent with the reported revision, including original virtual
balances, free/reserved/total amounts, HYPOTHETICAL evidence and execution_authorized=false.
The `/journal` and `/reservations` child routes are paginated reads. Reservation fee
budgets are original request budgets, not current remaining inventory. No HTTP endpoint
can reserve, resolve, settle, sign, send, reset or promote a result to REALIZED.

All pages accept limits 1..100 and returned cursors only. Decision-history and paper-ledger
capability flags describe implemented API functionality, not provider readiness or populated
data. Browser authentication still expires on restart; durable records remain.

## Verification

Run `cargo test -p control-api` for HTTP-boundary tests covering cookie flags, token rotation,
authentication, CSRF/origin rejection, logout, rate limits, bounds, configuration validation,
LIVE/DISARM denial, durable-store error mapping, idempotency forwarding and PENDING STOP semantics.
Most cases use an isolated fake store to exercise the HTTP boundary. Tokio virtual-time
tests prove deadline and concurrency bounds without waiting 15 real seconds. A separate
`TEST_DATABASE_URL` integration case runs the real HTTP router against PostgreSQL and
`ControlWorker`, proving PENDING → APPLIED STOP, idempotency and revision conflicts,
operator isolation, durable receipts across API restart, and old-cookie revocation.
It fails if the database environment is absent; CI supplies it. Unit-only runs must
explicitly filter out this integration case rather than reporting a skipped case as passing. The `arb-storage` PostgreSQL
integration tests separately verify actual durability, idempotency, revision races,
worker acknowledgement and database constraints. HTTP mock success does not establish
PostgreSQL behavior or live market capability.

Additional PostgreSQL HTTP tests exercise persisted research filtering, rejection counts,
pagination, origin separation, immutable paper creation and cross-operator denial. These
cases require TEST_DATABASE_URL and must be executed in CI; source presence or successful
mock tests alone are not evidence of database behavior.

## Collection diagnostics and frozen export

Authenticated operator-scoped GET routes expose `/v1/sessions/{session_id}/collection-attempts`, `/collection-coverage` and `/export`. Attempts use canonical UUID cursors and limits 1–100; coverage and export accept no query parameters. `collection_telemetry` and `session_export` capability flags advertise these features. Recorded attempt counts include READINESS and RESEARCH separately, with exact decision associations; unresolved starts and market/schedule completeness remain explicitly unknown.

The export takes a read-only REPEATABLE READ snapshot of the six defined research datasets (decisions, paper runs, journal events, capture catalog entries, collection attempts and cost assessments). It refuses more than 10,000 source rows or 8 MiB with 413; two exports may run per process, with additional requests returning 429 EXPORT_BUSY. Normal request deadlines remain 15 seconds, and each export SQL statement has an 8-second bound. All responses retain authentication, redacted errors and no-store. Configuration content, local artifact paths and raw files are excluded; capture availability/expiry and missing decimals are disclosed. Paper journal reasons are removed and command/attempt identifiers are pseudonymized while exact accounting and original payload hashes remain. The [contract](../../docs/08-DATA-AND-API-CONTRACTS.md) and [verified checkpoint](../../docs/18-COLLECTION-AND-EXPORT-VERIFICATION.md) define hash formats, source scope and remaining acceptance.


## Immutable manual cost assessments

`POST /v1/sessions/{session_id}/cost-assessments` accepts `{observation_id, scenario}`
and requires authentication, exact Origin, CSRF and Idempotency-Key. The source observation
must be a stored QUOTED decision in this operator's selected session. The server derives the
historical amount, output, assets, network, configuration, experiment and source digest;
clients cannot supply replacement quotes or evidence. The response is HTTP 201 with
`{record_id, recorded_at, assessment}`. An identical key and payload returns that exact
record, including after restart; changed payloads return 409. A malformed source ID or
invalid scenario returns 400; absent and out-of-scope source records return 404.

`GET /v1/sessions/{session_id}/cost-assessments` lists records with limits 1–100 and
canonical UUID cursors. `GET /v1/sessions/{session_id}/cost-assessments/{record_id}` returns
one record. Every read rederives the assessment from its immutable source decision and
scenario, including hashes, negative values and unknown costs. These operations need no
running worker or re-registration of the historical configuration. API authentication
still expires at process restart. Each creation SQL statement has an 8-second timeout;
the existing 15-second HTTP deadline and 16 KiB request limit apply.

Scenarios are explicit MANUALLY_CONSTRUCTED assumptions. Native fees and token balances
remain distinct; native-to-starting-token conversions require declared exact ratios and
historical timestamps. An expense in the exact starting token must use SAME_ASSET; a
ratio cannot discount an amount already denominated in that currency. Applicable omitted
expenses remain unknown and keep net null.
Base network execution includes priority fees and keeps L1 data separate; Solana keeps
base/signature, priority and relay-tip components separate. Neither an optimistic net nor
complete assumptions changes CANDIDATE evidence, decision records, paper balances or
transaction eligibility. No settlement or execution endpoint is added.

Export schema 1.1.0 includes `data.cost_assessments` and its source count in the same
bounded database snapshot. `methodology.costs` states
`QUOTED_COSTS_UNKNOWN_MANUAL_ASSESSMENTS_SEPARATE`; original opportunity net remains null.
The canonical V1 hash algorithm is unchanged and now covers the sixth dataset. Retained
assessments preserve source bindings and safe provenance labels without exposing operator
idempotency keys, provider URLs or freeform credential-bearing text. Full transaction
simulation, measured fee collection and automatic paper fills remain separate work.

## Adapter support and declared chain-time policy

`GET /v1/adapter-support` is an authenticated, read-only startup catalog. Its schema is
`1.0.0`, catalog version `immutable-scope-v1`. Each immutable configuration reports both
networks and the compiled `arb_engine::capabilities` result. `LOADED_AUTHORIZED` means
only that a locally loaded registry matches the configuration digest, enabled network,
allowlisted pools/assets and expected genesis identity. It does not qualify an RPC provider,
deployed bytecode, token behavior, transaction construction, simulation or submission.
All qualified execution capability flags remain false.

Optionally set `ARB_SUPPORT_REGISTRY_FILES` to comma-separated `network=path` entries,
for example `base-mainnet=/app/config/base-registry.json,solana-mainnet=/app/config/solana-registry.json`.
There may be at most 16 entries, each document at most 1 MiB and eight pools. Several
registry digests for the same network support different frozen configurations. Duplicate
network/digest documents and malformed explicitly configured files fail startup with a
redacted error. Omit the variable to expose `NOT_LOADED`. Registry paths, raw contents,
qualification-reference text, provider references and credentials are never returned.
No RPC requests or filesystem reads occur when serving the endpoint.

The catalog distinguishes declared identities from loaded, structurally authorized pool,
token and program relationships. Missing, disabled, digest-mismatched and out-of-scope
registries expose reason codes and no pool relationships. For declared allowlists exceeding
eight pools or 16 assets, both identity arrays are empty, exact counts remain visible,
and `identities_expanded=false` with `DECLARED_SCOPE_NOT_EXPANDED`; authorization still
checks the full frozen lists. Larger existing configurations and repeated equivalent
configuration files continue to start. The catalog coalesces duplicate configuration digests.

The optional immutable per-network policy reports `CONFIGURED` with
`{ "version": "finalized-chain-time-v1", "max_chain_age_ms": ... }`, otherwise
`NOT_CONFIGURED`. A configured threshold is an operator research assumption. Runtime
freshness evidence is returned on decision schema 1.1.0; legacy decisions retain schema
1.0.0 with no new report field. Storage binds a new report's policy exactly to its scoped
immutable snapshot and rejects attempts to remove or change that policy by resealing a
trace. Historical legacy snapshots with no policy continue to read unchanged.
