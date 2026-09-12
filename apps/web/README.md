# Arbitrage Research dashboard

React and TypeScript implementation of the approved [dashboard reference](../../design/dashboard-wireframe.html), using the [design system](../../design/DESIGN-SYSTEM.md). The default **Demo** contains labelled fictional data. **Connected** displays authenticated, persisted API evidence. Connection errors never substitute demo records.

## Local and production setup

Use Node.js 24 or later and the committed lockfile:

```sh
cd apps/web
npm ci
npm run dev
```

Vite serves http://127.0.0.1:5173 and proxies /v1 to http://127.0.0.1:8080, preserving the browser Origin. The API needs ARB_PUBLIC_ORIGIN=http://127.0.0.1:5173 and explicit ARB_ALLOW_INSECURE_LOOPBACK=true for local development. Production uses a same-origin HTTPS reverse proxy, an HttpOnly Secure cookie and the private control API; npm preview is not a production proxy. Provision the operator secret outside the browser.

```sh
npm test
npm run build
npx playwright install --with-deps chromium
npm run test:browser
```

Authentication uses /v1/auth/session and login/logout. CSRF tokens remain in memory. Operator credentials are never saved in browser storage. Closing the page, signing out or opening Demo does not stop workers.

## Connected controls

The six approved destinations, light/dark themes and responsive styles remain available. Sessions show immutable configuration, mode and network, exact desired/applied revisions, observed state, heartbeat, health and outstanding attempts. API connectivity does not establish feed freshness or worker health.

Start, Pause, Resume and Stop issue revisioned requests. A 202 remains PENDING until a service receipt reports worker application. A timer cannot apply a command. Stop can supersede a pending non-Stop request. Definitive rejection remains visible; uncertain delivery retains the exact body and idempotency key for retry.

Stop-all targets eligible loaded sessions independently of the chain view filter, with explicit page limitations and per-session results. Separately launched capture CLIs have their own lifecycle. Stale snapshots disable Start/Pause/Resume; an authenticated Stop request remains possible using the last known revision. A rejection does not claim a successful stop.

Existing session/opportunity/capability refreshes run every five seconds, with at most five pending receipts per cycle, round-robin, and bounded failure backoff to 30 seconds. Each request has a ten-second client timeout. Requests abort on leaving Connected. Research explorers load on explicit selection, pagination or refresh and add no background polling loop.

Experiment creation selects an exact server-validated configuration, enabled network and registered strategies. Creation does not send Start. A session can remain RECOVERING until a worker reconciles it. The browser cannot edit or invent a configuration digest.

## Persisted decision evidence

Opportunities contains two explicitly separated views:

- **Recorded opportunities** requests only CAPTURED_MARKET_DATA projections. The client rejects synthetic substitutions. Schema 1.1 requires RECORDED_LIVE origin and a null net when external costs are incomplete; legacy schema 1.0 remains supported. Failed simulations remain rejected. Exact integer amounts never become floating-point balances.
- **Decision evidence explorer** reads stored observations, including persisted synthetic or manually constructed datasets with explicit origin badges. These are API records, not generated fallback data. Every observation retains its network, frozen configuration, calculation version, capture references, grouping and result.

The explorer selects a named session. The chain filter changes session choices, not the already selected scope. The origin filter applies only to the loaded observation page. Session aggregate counts include all origins and distinguish raw observations, quoted candidates, unique quoted groups, rejections, no-route and data-unavailable results. Execution counters are **Unknown** when unavailable. Stored time bounds do not establish continuous collection completeness or a census of arbitrage.

QUOTED is a gross candidate, including negative gross deltas. External costs, full transaction simulation and executable profit remain unestablished. The detail inspector shows the durable trace wrapper, calculation inputs, exact asset units, origin, input age and capture references. Missing input age stays Unknown. Capture references do not prove raw artifacts are still retained.

Lists request at most 25 records and expose bounded previous/next navigation (up to 100 pages per navigation session). They never silently fetch an entire history. Switching scope removes the previous scope's rows immediately. Refresh errors retain only the same scope's snapshot and label it stale.

## Hypothetical paper accounting

Runs provides a paper accounting workspace when the API advertises paper_ledger. It displays retained run identities and frozen configuration, exact free/reserved/total balances by asset, immutable initial inventory, journal postings, and reservation history. Native fee inventory stays separate from token principal. Original reservation budgets are labelled as request ceilings; current run balances establish the inventory still reserved.

New run creation additionally requires paper_run_creation, a STOPPED PAPER session, its enabled validated configuration, a configured token principal asset and a separate native fee asset. The form requires a positive exact token amount and an explicit native amount (zero is allowed). The user reviews the immutable session, digest and amounts before creating a new ledger. The server atomically checks pending command status. A rejected or superseded command can leave unequal revisions without blocking an otherwise stopped session. Older ledgers are retained.

Uncertain creation locks the original session and settings. Retrying sends the same body and idempotency key, including after navigating between Connected pages or changing the chain view filter. A later rejected retry does not resolve an earlier uncertain delivery or replace its key. A successful creation can prepare another **new** run; it does not reset or overwrite prior inventory. No settlement, reservation mutation, wallet, key import, signing, approval or broadcast controls exist.

All accounting is **HYPOTHETICAL**. There are no invented fills, portfolio conversions, return percentages or realized-profit totals. Run comparisons remain unavailable until coverage, valuation and simulation evidence can support them. Run balances and journal/reservation pages are separate received snapshots; their timestamps and run revision remain explicit.

## Bounded evidence exports

The explorer exports the current filtered observation page or one selected durable trace. Paper accounting exports one selected run or one loaded journal page. JSON retains exact decimal strings, known contract fields, scope, snapshot-received time and export time. Each export is bounded to 2 MiB and performs no additional fetch.

Exports are limited evidence selections, not a full-history audit or a profitability claim. Journal export deliberately includes command kind and postings, omitting original command inputs. Capture references do not establish artifact retention. CSV, bulk export, comparison reports, portfolio valuation and complete audit packages remain outside this implementation.

## Code boundaries

| Path | Responsibility |
|---|---|
| src/App.tsx | Explicit Demo/Connected selection |
| src/DemoApp.tsx | Original fictional dashboard and local demonstration controls |
| src/ConnectedApp.tsx | Authentication, session control and configuration-based creation |
| src/api/client.ts | Same-origin transport, CSRF, idempotency and scoped resource methods |
| src/api/research.ts | Strict research DTO parsing and known-field projections |
| src/components/DecisionExplorer.tsx | Persisted observations, grouping, coverage and evidence inspection |
| src/components/PaperWorkspace.tsx | Immutable paper initialization, balances, journal and reservations |
| src/components/ResearchShared.tsx | Accessible detail modal, page controls, export and snapshot state |
| src/hooks/useResearchResource.ts | Abortable, scope-safe reads and bounded cursor history |
| src/domain/researchExport.ts | Exact, bounded JSON export envelope |
| src/components/RecordedOpportunities.tsx | Captured-only opportunity table and inspector |
| tests/research.test.ts | Exact units, origin/coverage contracts, scoped transport and exports |
| tests/browser/research.spec.ts | Stubbed persisted research flows and responsive screenshots |

## Verification and remaining acceptance

The original Demo and Connected browser regressions remain intact. This round adds seven Node scenarios and ten browser scenarios for persisted evidence, origin and unknown-value semantics, bounded exports, paging/outage behavior, hypothetical balances/journal, immutable creation retry across navigation, keyboard focus and 320/390/1440px layouts. Fixtures are explicitly test data, not market evidence.

The round 2 source was reconstructed through GitHub while the execution environment was offline, then atomically materialized and verified after recovery. Local application/test TypeScript checks, the Vite production build and all 20 Node tests passed. Playwright successfully discovered all 30 browser scenarios; the matching Chromium executable is absent locally, so browser execution and screenshot acceptance remain CI requirements. Stable screenshot files are test-results/**/research-dashboard-{320,390,1440}.png. Screenshots contain only contract fixtures. A separate API engineer reviewed scope cancellation, exact arithmetic, origin/net consistency, immutable retry identity, revision-gap behavior and export completeness; that source review does not replace browser or real-service tests.

The service smoke test uses the real typed client against an isolated Rust API and PostgreSQL service with a Node cookie/Origin shim. It validates transport interoperability, not browser cookie policy. Its existing authentication/configuration checks remain; the new browser flows use HTTP contract fixtures. Real persisted capture-to-decision-to-browser evidence, complete simulated execution/accounting, manual screen-reader/zoom review and broader supported-chain datasets remain separate acceptance requirements.

Capabilities mean implemented API functionality, not populated data or a running market provider. A false API-process collection flag does not establish that separate workers are absent. The dashboard does not claim an operational collector from capability flags alone. Mode switching is disabled while commands or creations have unresolved delivery, and normal Connected navigation and re-authentication preserve their original keys. Pending commands and paper creations are durable server-side, but the browser's uncertain request keys currently last only while Connected remains mounted. After reload or sign-out with an uncertain creation, the operator must inspect authoritative records and reconcile that request before submitting equivalent new work.
