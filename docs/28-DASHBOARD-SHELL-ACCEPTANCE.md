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

## Accepted original scope: 14 September 2026

PR #113 is merged as `f3bf4c0dca575e8730763957da2aecef6b3fa8a8`.
Its tree `425699ff47805dd357afe9a0ac231e3a5115866c` exactly matches CI's
synthetic merge `cac25cb367beaaf757bfbe3048fc2817b28118d1`, combining PR head
`1de55d5125615f5b4cbf5a1f445ed9f33ce60b87` with main `6dce9c99`.
Full project run 34854979450, Delivery 34854979611, Recovery 34854979496
and Prepare Rust 34854979347 passed. The actual browser report has 91 passed
with no skipped, failed or flaky cases, including all sixteen shell scenarios.

The implementing root visually inspected eight shell screenshots (four widths,
both themes), eight connected/disconnected screenshots and both committed
reference PNGs. Panel geometry, theme hierarchy, visible provenance/mode and
contained controls satisfy the original shell criteria. Browser artifact
10352362455 has verified SHA-256
`dfb348c74d72c1f12c11c8764d05a651a0e6a07c362c200a99e11d58978b1eff`.
The intermediate connected-label strict-locator ambiguity was corrected by
selecting the header label, without removing visibility assertions.

[Source review](https://github.com/makafeli/arbitrage-research/pull/113#pullrequestreview-5198899886)
and [tested-tree addendum](https://github.com/makafeli/arbitrage-research/pull/113#pullrequestreview-5198937543)
record the reviewer and limits. CodeRabbit acknowledged the visual evidence and
resolved its own thread; no independent accessibility certification is claimed.
Original predecessors #19/#22 were read back as completed. The original native
checklist was checked with criterion-specific evidence, and the existing
`delivery.py closeout` returned `EVIDENCE_PACKET_CONSISTENT` before #50 was
explicitly closed as completed at 14:35:04 UTC on 14 September 2026.

The registry synchronization changes only ARB-036, preserving all other 67 task
records and the original scope/dependency graph. #27 and broader data, campaign,
provider and production requirements remain open. No new issue or deployment.
