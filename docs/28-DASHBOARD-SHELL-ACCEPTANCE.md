# ARB-036 / #50: research dashboard shell

This review concerns the original dashboard shell only. Recorded-market data,
complete paper accounting and operational campaign acceptance belong to their
existing tickets; they are not prerequisites added to this UI-shell scope.

## Change and original criteria

The display theme is now owned by the common App shell. Previously, switching
from Connected back to Demo remounted DemoApp with `light=false`, silently
resetting a user's light theme. Both modes now receive one theme value and one
toggle callback. This state is display-only and is not persisted with credentials,
research data, session configuration or market scope. Authentication boundaries,
uncertain-request safeguards and demo/connected separation are unchanged.

| Original criterion | Source and verification |
|---|---|
| Equal chain weight and recognizable lime/navy reference | Existing shared ChainPanels renders both networks with the same component/tokens. New browser checks measure panel geometry at 320, 390, 768 and 1440 pixels in both themes and retain screenshots. Compare these and the existing reference screenshot against design/dashboard-wireframe.html. |
| Demo datasets explicitly labelled | All six demo views retain the fixed synthetic banner, PAPER MODE DEMO and per-record/section provenance. New checks traverse all views in both themes and confirm those labels stay visible. Connected-mode tests refuse demo substitution on authorization/service failure. |
| Chain filter is view-only | New demo and connected HTTP-stub scenarios monitor outgoing requests and assert filtering issues no mutation and leaves the two session states intact. Existing lifecycle tests distinguish a filter from explicitly scoped commands. |
| No wallet/key/live shortcut | The shell has no wallet-connect, private-key import or live-arm control. Browser checks cover the demo views; source review distinguishes the Connected operator-login password from a wallet/private-key field. Existing capability contracts reject LIVE. |

Overview, Opportunities, Experiments, Runs, Strategies and System remain intact.
Mode, connection, synthetic capture time/data age and scoped session controls are
retained. No redesign, external font, wallet integration or new frontend package
is introduced. ARB-006/#19 and ARB-008/#22 are the original dependency gates.

## Verification and limits

`npm run build`, `npm test`, and `npm run test:browser` run in the existing Node24
CI environment. Sixteen added browser scenarios cover both themes at four widths,
mode-switch theme continuity and connected view-only filtering. The existing
suite supplies further lifecycle, service-error, command-acknowledgment, desktop
and narrow-layout coverage. Screenshots and typed/service-backed checks must be
inspected for the final published source before closure. Their mere presence in
this document is not a passing run or a completed visual review.

The implementing assistant is the named root reviewer, not an independent UI
engineer or automated accessibility auditor. This ticket does not certify WCAG,
all screen readers, real provider evidence or full acceptance of every dashboard
feature. Final source/review/CI and native closure evidence are attached to #50.

## Resume correction: compact mode labels

The initial PR source `d957f609` ran 83 browser scenarios: 79 passed and four
failed at 320/390 pixels in both themes. The original compact stylesheet hid
every `.topactions .pill`, including PAPER MODE DEMO and CONNECTED MODE. The
visibility assertions correctly caught a real shell defect; they are retained.
The hiding rule is removed, while existing flex wrapping handles narrow widths.
Transition and authenticated view-only filter tests now run at all four widths,
so both modes retain labels without changing session or authorization state.

The failure report artifact 10350665443 was verified by SHA-256 before inspection.
The same source's Rust/PostgreSQL/client-HTTP, specifications and all container
jobs passed. Corrected-source results and screenshots must be checked separately;
this record does not transfer that earlier green evidence to new source.
