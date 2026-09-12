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
