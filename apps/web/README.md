# Arbitrage Research dashboard

React + TypeScript implementation of the approved [dashboard reference](../../design/dashboard-wireframe.html), using the [v0.2 design system](../../design/DESIGN-SYSTEM.md). It is a local synthetic demo and project scaffold. It does not connect to the Rust service or any market, wallet or blockchain.

## Run locally

Use Node.js 24 or later and npm. Dependencies are exact-pinned and the generated lockfile is committed.

```sh
cd apps/web
npm ci
npm run dev
```

Open the local URL printed by Vite, normally `http://127.0.0.1:5173`. The development and preview servers bind to loopback by default.

```sh
npm test
npm run typecheck
npm run build
npm run preview
```

The build emits `dist/`; it is generated output and is not committed. `npm run preview` normally serves the build at `http://127.0.0.1:4173`.

Browser smoke tests require Playwright Chromium:

```sh
npx playwright install --with-deps chromium
npm run build
npm run test:browser
```

## Implemented interactions

- Six destinations: Overview, Opportunities, Experiments, Runs, Strategies and System.
- Dark and light themes; responsive sidebar and opportunity cards.
- View-chain filtering that never changes session scope.
- Synthetic opportunity details with evidence, named assumptions and net USDC denomination.
- A local run group containing two independent per-network PAPER sessions.
- Start, Pause, Resume and Stop demonstration. Stop stays PENDING until separate illustrative session acknowledgments; one session may stop before the other.
- A separate Runs scenario showing DRAINING after an applied fence and STOPPED only after illustrative resolution.
- Skip link, focus-visible controls, status announcements and a native modal dialog with Escape/Close dismissal and focus restoration.

The fixture timestamp is fixed at 12 September 2026, 12:00 UTC. Example figures are fictional and do not update from a feed. Evidence labels illustrate the product vocabulary; this interface has not performed a transaction simulation. Unknown costs remain Unknown and no paper result is presented as Realized.

## Code boundaries

| Path | Responsibility |
|---|---|
| `src/App.tsx` | Application shell and local demonstration orchestration |
| `src/components/OpportunityTable.tsx` | Responsive observations and evidence badges |
| `src/components/OpportunityDialog.tsx` | Inspectable fixture and cost detail |
| `src/components/ResearchViews.tsx` | Chain panels and secondary destinations |
| `src/domain/lifecycle.ts` | Pure local state transitions and immutable demo scope |
| `src/domain/fixtures.ts` | Typed fictional observations and chain summaries |
| `src/styles.css` | Approved visual tokens, dark/light themes and responsive layout |
| `tests/lifecycle.test.ts` | Meaningful transition, scope and evidence tests without a browser |
| `tests/browser/` | Playwright interaction and responsive smoke checks |

Local timers illustrate worker acknowledgments. Production controls must use authoritative acknowledgments from the API and workers; elapsed time must never become an applied command. The frontend must preserve per-session control results and cannot promise an atomic all-worker stop. The REST contract in `../../specs/openapi.yaml` is the integration target, not an implemented browser client.

Mode switching, live enablement, key handling, durable sessions, authentication, real feeds, replay execution, editable strategies and exports are not implemented here. Session state resets on reload. Vite performs TSX compilation without an additional React plugin; development updates can reload the page and reset the demo.

## Verified validation

The [final GitHub CI run](https://github.com/makafeli/arbitrage-research/actions/runs/34709107929) passed application and test TypeScript checking, the production Vite build, all six model tests and all ten Chromium browser scenarios. Node 24 and the committed npm lockfile were used.

Browser coverage includes six views, themes, filtering, initial modal focus, Tab/Shift+Tab containment, Escape/focus restoration, independent stop acknowledgements and draining. Viewport checks cover 320, 390, 768 and 1440 pixels, including card/label clipping inside wrappers. Verified [desktop](../../design/dashboard-desktop.png) and [mobile](../../design/dashboard-mobile.png) previews are committed. The [verification record](../../docs/11-PACKAGE-VALIDATION.md) describes complete scope and limitations.

Manual acceptance still includes screen-reader review, 200% zoom, contrast verification in both themes, and the planned loading/stale/unavailable states when API integration exists. The production bundle contains only the synthetic local demo.

## Dependency references

Versions were resolved from the npm package registry on 12 September 2026 and pinned, rather than inferred solely from cached release articles. React 19.3.0, Vite 8.3.0 and TypeScript 7.0.2 are the installed scaffold versions. The [React documentation](https://react.dev/learn) describes the UI model, the [Vite guide](https://vite.dev/guide/) describes development/build requirements, and [TypeScript documentation](https://www.typescriptlang.org/docs/) describes the type-checking configuration. Dependency upgrades should be reviewed with the lockfile and the same checks.
