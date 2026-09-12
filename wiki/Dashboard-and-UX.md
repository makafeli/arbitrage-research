# Dashboard and UX

The user selected the existing [dashboard design](https://github.com/makafeli/arbitrage-research/blob/main/design/dashboard-wireframe.html) as the implementation reference. The React application in `apps/web` follows its quiet navy surfaces, lime accent, dark/light themes, six destinations and equal Solana/Base emphasis.

The current application displays synthetic fixtures. Its counts, opportunities, timestamps and command transitions explain intended behavior; they are not live market observations or trading results. The synthetic label must remain visible wherever those examples appear.

## Screens

| Screen | Primary purpose |
|---|---|
| Overview | Mode/state, chain coverage, data quality and recent observations |
| Opportunities | Filter candidates, inspect route/state/costs and explain rejection |
| Experiments | Compare equivalent windows and assumptions without inventing a winning chain |
| Runs | Inspect immutable configuration, controls and unresolved outcomes |
| Strategies | Explain allowed routes, sizes, assets and readiness |
| System | Show feed/service health, freshness and gaps |

The chain selector filters the displayed research. It must not start, stop or reconfigure workers. Scope changes belong to a new run/configuration flow.

## Information priority

Mode, observed state, freshness and the stop control remain visible. Each financial value identifies its evidence and denomination. Missing costs display Unknown or Excluded; they are not silently zero. Only reconciled LIVE outcomes can appear in realized totals. Paper results never do.

Opportunity details show route order, pool/state references, size, gross estimate, costs and eligibility reasons. Fees and price impact already included in a quote are labeled rather than subtracted twice. Scenario inputs are visually distinct from observed facts. Display transaction explorer links only for real transaction references.

## Controls

Start creates/starts a specific immutable session. Pause closes evaluation/admission/submission, while feeds and reconciliation continue. Resume revalidates fresh state rather than reusing stale queued trades. Stop shows PENDING until the worker applies the local fence, then DRAINING if previously dispatched outcomes remain unresolved, and STOPPED only after resolution.

A demo timer may illustrate that flow; production UI must use durable command and observed worker responses. API acceptance and worker application are separate fields. A disconnected worker is unavailable, not presumed stopped. Stopping cannot recall a transaction already sent.

## Accessibility and review

Use semantic landmarks, real buttons, associated labels, visible focus, a skip link and a polite live region. Dialogs need an accessible title, Escape dismissal, focus containment and restoration. Color always has a text equivalent. Reduced motion and the light theme receive the same functional coverage as dark mode.

Production acceptance includes 320, 390, 768 and 1440 pixel views, 200% zoom, keyboard-only use, status contrast and opportunity detail dialogs. Narrow screens stack chain panels and turn the opportunity table into readable cards. Source checks and mocked events cannot establish visual or assistive-technology behavior.

The [full UX specification](https://github.com/makafeli/arbitrage-research/blob/main/docs/05-UX-DESIGN.md) is the review contract. The validation record states which browser checks actually ran.
