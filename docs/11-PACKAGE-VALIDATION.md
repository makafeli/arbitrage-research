# Foundation verification record

Date: 12 September 2026. Version: 0.2. All three jobs in the [final code validation run](https://github.com/makafeli/arbitrage-research/actions/runs/34709107929) passed against code commit `36e6c35706927a3cc30a522f5b16cf1957e1aa9f`. Later handoff changes contain documentation, publication metadata and captured previews only. This record validates the foundation and synthetic dashboard, not an arbitrage strategy or production execution system.

## Verified checks

| Area | Actual evidence | Result |
|---|---|---|
| Rust build and lint | Pinned Rust 1.90.0, committed Cargo.lock; Clippy across all workspace targets with warnings denied | Passed |
| Rust formatting | `cargo fmt --all -- --check` | Passed after applying the CI formatter patch |
| Rust domain behavior | `cargo test --workspace --locked` | All 19 tests passed, including exact amounts, immutable modes, pending/applied control, stop/drain/recovery and non-live evidence restrictions |
| Frontend build | Node 24, `npm ci`, application and test TypeScript checks, Vite production build | Passed with the committed npm lockfile |
| Frontend unit/model behavior | `npm test` | All six tests passed |
| Browser interactions | Playwright Chromium, `npm run test:browser` | All ten scenarios passed |
| Contract fixtures | `python scripts/validate_specs.py` | Seven operation IDs and local refs resolved; valid fixtures/arithmetic passed; twelve invalid evidence/quantity examples rejected |
| GitHub importer | `python scripts/test_github_bootstrap.py` | All twelve offline regression tests passed |
| Project integrity | `python scripts/validate_project.py` | Eight epics, 68 tasks, acyclic resolved dependency graph, required metadata, local document links, parsed JSON/TOML/Python and five Cargo members passed |
| Repository setup | [Successful planning workflow](https://github.com/makafeli/arbitrage-research/actions/runs/34708957300) and independent GitHub issue metadata reads | 76 issues, 23 custom labels, eight milestones, 68 parent/child links and 219 blocked-by links verified |

The contract checker implements the schema keywords exercised by this package; it is not a complete JSON Schema or OpenAPI conformance validator. A flag in a valid example does not prove actual chain state, full transaction simulation or economic eligibility.

## Browser and visual evidence

The ten browser scenarios cover all six screens, both themes, chain filtering without scope changes, opportunity detail, initial focus, Tab/Shift+Tab containment, Escape/focus restoration, separate session acknowledgements and the synthetic DRAINING example. The four viewport scenarios exercise 320, 390, 768 and 1440 pixels and detect both page-level and internal card/label overflow. Tested flows reject uncaught JavaScript errors and attempted external connections.

CI captured desktop/mobile previews and the approved reference. Visual review confirmed the retained dark palette, lime accent, sidebar, peer chain panels and opportunity table. It identified and resolved a clipped mobile financial label. Browser execution identified and resolved modal tab wrapping and explicit initial focus after opening.

The verified [desktop preview](../design/dashboard-desktop.png) and [mobile preview](../design/dashboard-mobile.png) are committed. Full screenshots and traces are in the run's dashboard artifact while its retention period lasts. This is bounded Chromium and visual verification; other browsers, screen-reader testing and accessibility conformance certification remain future acceptance work.

## Setup recovery evidence

The first importer run created all 76 issues, then received an issue collection that omitted five known IDs. The replacement index originally lost those IDs. Refresh now individually verifies known issues absent from a collection and saves the new index only after identity/marker checks succeed. Four added regressions cover omission after creation, omission after restart without duplicate creation, failed verification preserving state and changed-marker rejection. The succeeding workflow verified the existing issues and finished native relationships.

## Scope of review

Product, architecture, engineering, UX and operations specialists reviewed bounded parts of the handoff. The review resolved single-network session scope, stop during recovery/fault, exact-amount boundaries, evidence classification, simulation/funding gates and dependency scheduling. These checks are not an independent production execution-security audit.

The authenticated control API, durable storage, market adapters, complete transaction simulation and paper accounting remain unimplemented. No provider, wallet key, funded account, production executor, signer or live broadcast system is configured. Current dashboard data is synthetic and paper examples never become REALIZED returns.

The [GitHub setup status](13-GITHUB-SETUP-STATUS.md) separately records the remaining owner Project and native Wiki publication steps. Their prepared source is not evidence that those remote resources exist.
