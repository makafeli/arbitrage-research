# Browser QA: Arbitrage Research / Mobbin v3

Run against the delivered standalone source on 19 September 2026.

## Recorded results

| Measure | Result |
|---|---|
| Assertions | 48 passed / 48 total |
| Layout combinations | 204 passed / 204 total |
| JavaScript runtime errors | 0 |
| External HTTP(S) requests | 0 |
| Browser-rendered preview images | 40 |

## Grid

The layout sweep covers 17 page/tab/presentation variants at 320, 390, 768, 1024, 1280 and 1440 CSS pixels in both light and dark modes. It measures page-level horizontal overflow. Wide technical tables may intentionally scroll within their own container.

Additional inspector assertions check that the dialog fits each tested viewport in both themes. The same evidence record is inspected from both List and Cards views.

## Functional assertions

- PASS: Light is the first-load default.
- PASS: Primary action follows the ink palette.
- PASS: Primary navigation changes page.
- PASS: Record search empty state.
- PASS: Record detail leads with rejection and unknown net.
- PASS: Escape returns focus to the record control.
- PASS: Global search works.
- PASS: Tabs support arrow keys.
- PASS: Card layout uses the same filtered record.
- PASS: Card layout retains an unknown net and rejection.
- PASS: Card opens the same evidence inspector.
- PASS: List switch preserves the query.
- PASS: Request stays pending without changing observed state.
- PASS: Only explicit simulated acknowledgement applies start.
- PASS: Draining retains unresolved outcomes.
- PASS: Separate reconciliation produces stopped.
- PASS: Bulk stop enumerates all loaded scope despite chain filter.
- PASS: Create requires an acknowledgement and reference.
- PASS: New session never auto-starts.
- PASS: Incomplete costs produce Unknown.
- PASS: Explicit zero costs are different from missing.
- PASS: Arbitrary-size integer arithmetic preserves precision.
- PASS: Fractional minor units remain invalid.
- PASS: Saving assumption does not change source quote.
- PASS: Frozen export isolated from later model changes.
- PASS: Download is a labelled synthetic frozen JSON object.
- PASS: CSV contains exact integers and unknown net.
- PASS: Recovery does not claim an email was sent.
- PASS: Real execution remains disabled.
- PASS: Inspector fits the dark 320px viewport.
- PASS: Inspector fits the dark 390px viewport.
- PASS: Inspector fits the dark 768px viewport.
- PASS: Inspector fits the dark 1024px viewport.
- PASS: Inspector fits the dark 1280px viewport.
- PASS: Inspector fits the dark 1440px viewport.
- PASS: Inspector fits the light 320px viewport.
- PASS: Inspector fits the light 390px viewport.
- PASS: Inspector fits the light 768px viewport.
- PASS: Inspector fits the light 1024px viewport.
- PASS: Inspector fits the light 1280px viewport.
- PASS: Inspector fits the light 1440px viewport.
- PASS: Overview preview state: stale.
- PASS: Overview preview state: error.
- PASS: Overview preview state: empty.
- PASS: Overview preview state: loading.
- PASS: No JavaScript runtime errors.
- PASS: No external requests.
- PASS: No page-level horizontal overflow across tested layouts.

## What these checks do not establish

This is Chromium/Playwright testing of an offline HTML prototype. It is not Safari or Firefox certification, a physical-device test, a screen-reader audit, a security review or a connected backend test. Passing the overflow sweep does not prove every possible long-value, localization or future record combination is covered.

Fixtures are synthetic. No production API, wallet, account, worker or deployment was used. The modal and command tests verify local demonstration state only. The event listener records HTTP(S) requests initiated during this run; this is not a general network-security assessment.

Read `results.json` for the recorded checks and all layout rows. Rebuild and rerun `test_prototype.py` after changing source files.
