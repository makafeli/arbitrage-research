# Dashboard and UX

The user selected the existing [dashboard design](https://github.com/makafeli/arbitrage-research/blob/main/design/dashboard-wireframe.html) as the implementation reference. The React application in `apps/web` follows its quiet navy surfaces, lime accent, dark/light themes, six destinations and equal Solana/Base emphasis.

The application has explicit Demo and Connected modes. Demo fixtures remain labeled synthetic. Connected mode uses authenticated API records for session creation, durable command receipts, raw/grouped decisions, coverage and virtual paper accounts. Captured records retain their own origin, so a connected view is not evidence that all its inputs came from a live market. Current browser acceptance and any pending CI are recorded in [integration verification](https://github.com/makafeli/arbitrage-research/blob/main/docs/17-RESEARCH-INTEGRATION-VERIFICATION.md).

## Screens

| Screen | Primary purpose |
|---|---|
| Overview | Mode/state, chain coverage, data quality and recent observations |
| Opportunities | Filter candidates, inspect route/state/costs and explain rejection |
| Experiments | Create an immutable configured research session; comparable campaign analysis remains a target |
| Runs | Inspect controls and immutable virtual accounts, balances, journal and reservations |
| Strategies | Explain allowed routes, sizes, assets and readiness |
| System | Show feed/service health, freshness and gaps |

The chain selector filters the displayed research. It must not start, stop or reconfigure workers. Scope changes belong to a new run/configuration flow.

## Information priority

Mode, observed state, freshness and the stop control remain visible. Each financial value identifies its evidence and denomination. Missing costs display Unknown or Excluded; they are not silently zero. Only reconciled LIVE outcomes can appear in realized totals. Paper results never do.

Opportunity details show route order, pool/state references, size, gross estimate, costs and eligibility reasons. Fees and price impact already included in a quote are labeled rather than subtracted twice. Scenario inputs are visually distinct from observed facts. Display transaction explorer links only for real transaction references.

## Controls

Create session records a specific immutable configuration and mode; it does not issue START. START is a separate command after worker recovery. Pause closes evaluation/admission/submission, while feeds and reconciliation continue. Resume revalidates fresh state rather than reusing stale queued trades. Stop shows PENDING until the worker applies the local fence, then DRAINING if previously dispatched outcomes remain unresolved, and STOPPED only after resolution.

Demo transitions are local examples; the connected UI uses durable command and observed worker responses. API acceptance and worker application are separate fields. A disconnected worker is unavailable, not presumed stopped. Current workers perform reads and candidate evaluation only. STOP fences new evaluations; raw acquisition can continue, and standalone capture CLIs are outside session controls. The later live design also cannot recall a transaction already sent.

Virtual-account creation is available only for a STOPPED PAPER session with matching enabled configuration. An unresolved creation locks the original session, amounts and idempotency key for retry. Bounded JSON exports describe selected received records; they do not claim complete collection, a full ledger audit or complete raw retention. See [Using research and paper accounts](./Using-Research-and-Paper-Accounts.md).

## Accessibility and review

Use semantic landmarks, real buttons, associated labels, visible focus, a skip link and a polite live region. Dialogs need an accessible title, Escape dismissal, focus containment and restoration. Color always has a text equivalent. Reduced motion and the light theme receive the same functional coverage as dark mode.

Production acceptance includes 320, 390, 768 and 1440 pixel views, 200% zoom, keyboard-only use, status contrast and opportunity detail dialogs. Narrow screens stack chain panels and turn the opportunity table into readable cards. Source checks and mocked events cannot establish visual or assistive-technology behavior.

The [full UX specification](https://github.com/makafeli/arbitrage-research/blob/main/docs/05-UX-DESIGN.md) is the review contract. The validation record states which browser checks actually ran.
