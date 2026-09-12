# Arbitrage Research dashboard

React + TypeScript implementation of the approved [dashboard reference](../../design/dashboard-wireframe.html), using the [design system](../../design/DESIGN-SYSTEM.md). It has two explicit workspaces: **Demo** with labelled fictional data, and **Connected** with authenticated research API records. A connection error never substitutes demo records into Connected mode.

## Run locally

Use Node.js 24 or later and the committed dependency lockfile.

```sh
cd apps/web
npm ci
npm run dev
```

Vite serves `http://127.0.0.1:5173` and proxies `/v1` to `http://127.0.0.1:8080`, preserving the browser Origin header. Configure the control API with `ARB_PUBLIC_ORIGIN=http://127.0.0.1:5173` and explicit `ARB_ALLOW_INSECURE_LOOPBACK=true` only for local development; production uses HTTPS and a Secure cookie. Provision the API operator secret outside the browser. The default application opens the standalone Demo; select **Connect API** to use the private service.

Production serves the static `dist/` assets and proxies `/v1` to the private control API on the same HTTPS origin. `npm run preview` serves static assets only; it is not a production proxy or an authentication boundary. Do not point the client at arbitrary cross-origin API hosts.

```sh
npm test
npm run typecheck
npm run build
```

Browser checks require the Chromium build matching the committed Playwright version:

```sh
npx playwright install --with-deps chromium
npm run build
npm run test:browser
```

## Connected workflows

- The six approved destinations, dark/light themes and responsive layout remain available.
- The typed client uses `/v1/auth/session`, login/logout, capabilities, sessions, commands and opportunities. Cookies are HttpOnly at the API; the browser client retains CSRF tokens only in memory and never writes operator credentials to browser storage.
- Sessions show immutable mode/network/configuration, desired/applied revisions, observed state, heartbeat, worker health and unresolved attempts. API connectivity is distinct from feed quality.
- Start, Pause, Resume and Stop issue revisioned, idempotent requests to individual sessions. A 202 response remains **PENDING** until an API receipt reports a worker acknowledgement. No timer changes a Connected command to APPLIED.
- A STOP can supersede a pending non-STOP command. Transport uncertainty retains the exact request and idempotency key for a deliberate retry. A definitive rejection stays visible.
- Stop-all means all eligible **loaded sessions**, regardless of the chain view filter. The UI explicitly warns when other sessions exist outside the loaded page. Fanout results stay attached to each session; this is not an atomic global stop. Separately launched capture CLIs have their own lifecycle.
- Stale reads preserve the last snapshot. Start/pause/resume are disabled while stale. Stop remains available when authenticated, using the last known revision; a rejected or uncertain request does not claim a successful stop.
- Pending receipts are polled in batches of at most five, round-robin, every five seconds. Snapshot refreshes use three reads per cycle, with bounded exponential backoff to 30 seconds after failures. Requests time out after ten seconds and are aborted when leaving Connected mode. Polling has no authority to infer worker application.
- Experiment creation selects a server-registered configuration digest, an enabled network and the registered strategy IDs. The server validates the request. Mode and configuration cannot be edited after session creation. New sessions can remain **RECOVERING** until a worker reconciles them; creation never sends START.
- Opportunity records must declare `CAPTURED_MARKET_DATA`. Runtime checks reject synthetic records, malformed integer amounts and inconsistent success evidence. Failed simulations display rejection, and incomplete costs display **Unknown**. Values remain exact integer minor units because the current opportunity contract does not expose start-asset decimals.
- The inspector shows route/pool identities, cost valuation references, source, state reference, age at evaluation, finality, simulation, plan digest, scenario and eligibility checks. A captured-data response alone does not prove profitability or current executability.

Closing the page, signing out or opening the Demo does not stop workers. Pending commands are durable on the server. The browser's recently issued receipt list and uncertain retry keys currently last only for the mounted Connected workspace; after a reload, inspect authoritative session revisions before issuing another command. Uncertain session creation must be reconciled by the operator before creating another experiment with equivalent settings.

## Deliberate limits

This UI integration is a foundation for ARB-037, ARB-038 and ARB-039, not completion of all their acceptance criteria. The current control API reports market capture unavailable. Full experiment settings (principal, fee reserves, asset/size selection and scenarios) are not exposed by that API yet: the form requires review of the registered source configuration and cannot pretend to render those missing values. Full-history pagination, comparison, portfolio ledger, coverage timeline, exports and recorded end-to-end decision evidence remain later work. No research form contains LIVE, wallet, key-import, signing or broadcast controls.

All default demo fixtures remain fictional with a fixed 12 September 2026 timestamp. Demo timers operate only in `DemoApp` and its local lifecycle model. The Connected app imports no fixture data and uses no simulated worker acknowledgement.

## Code boundaries

| Path | Responsibility |
|---|---|
| `src/App.tsx` | Explicit Demo/Connected choice |
| `src/DemoApp.tsx` | Original synthetic dashboard and local demonstration controls |
| `src/ConnectedApp.tsx` | Authenticated snapshots, session controls and configuration-based creation |
| `src/api/client.ts` | Same-origin API transport, runtime DTO checks and typed contracts |
| `src/components/RecordedOpportunities.tsx` | Captured-record table, evidence and provenance inspector |
| `src/components/OpportunityTable.tsx`, `OpportunityDialog.tsx`, `ResearchViews.tsx` | Original synthetic presentation |
| `src/domain/` | Pure local demonstration transitions and fictional observations |
| `src/styles.css` | Approved tokens and responsive styles |
| `tests/api.test.ts` | Authentication, precision, receipt, provenance and rejection checks |
| `tests/browser/connected.spec.ts` | Stubbed API outage, delayed ACK, uncertain retry, partial fanout, creation, login and responsive scenarios |

## Validation record

Local `npm run build`, application/test TypeScript checks and all 12 Node tests passed during this implementation. The ten original browser scenarios are preserved and ten Connected scenarios were added. Their first local run was blocked before launch because the matching Chromium executable was absent; a bounded installation attempt timed out. That attempt is **not browser acceptance evidence**. Run the repository browser CI and inspect its screenshots before marking responsive/interaction acceptance complete.

An additional `npm run test:service` interoperability smoke test uses the actual typed client against an isolated loopback Rust API and empty PostgreSQL database, with a Node cookie/Origin shim. It requires `ARB_OPERATOR_SECRET`, optional `ARB_API_SMOKE_URL`, and the API-matching `ARB_PUBLIC_ORIGIN`. It covers login/session/CSRF, capabilities, records, configuration denial and logout; it does not test browser cookie policy. It was not run locally because a native PostgreSQL service is unavailable in this environment.

The earlier scaffold's [verified CI run](https://github.com/makafeli/arbitrage-research/actions/runs/34709107929) and committed [desktop](../../design/dashboard-desktop.png)/[mobile](../../design/dashboard-mobile.png) images cover the original Demo only. Current Connected behavior requires its own CI evidence. Manual screen-reader, zoom and contrast review, plus a real recorded decision trace, remain outstanding.
