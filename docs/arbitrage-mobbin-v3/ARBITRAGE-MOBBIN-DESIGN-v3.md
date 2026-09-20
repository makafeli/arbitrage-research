# Arbitrage Research: Mobbin direction

**Version:** 3.0  
**Date:** 19 September 2026  
**Status:** interactive offline design proposal, not a production deployment  
**Predecessor:** Arbitrage Research / Atlas redesign, version 2.0  
**Requested visual basis:** `DESIGN-mobbin.md`, supplied by the user  
**Deliverables:** standalone HTML, editable source, design handoff, browser previews and executable QA

## 1. The next-version decision

Keep the research workflows. Replace the visual language and application shell.

Version 3 uses a white canvas, near-black ink, a quiet neutral surface ladder, floating pill navigation, rounded containers and stronger typography. The product should feel like a carefully organized research library, not a terminal and not a promotional trading dashboard.

The previous version's black/yellow brand treatment is removed. Light mode is the first-load default. Dark mode remains available as an explicitly designed adaptation; the supplied reference does not define a complete operational dark theme.

The six application destinations remain Overview, Opportunities, Experiments, Runs, Strategies and System. Research data, command acknowledgement and hypothetical accounting still represent different things. A new visual system is not permission to change those meanings.

## 2. What was actually used

| Material | Role in this version |
|---|---|
| `DESIGN-mobbin.md` | Primary visual reference. Its tokens, geometry, typography, navigation, table and form descriptions informed the new interface. A verbatim copy is included in `references/`. |
| `Arbitrage-Research-Redesign.html` | The executable version-2 baseline: existing routes, synthetic fixtures, interactions and event handling. |
| `ARBITRAGE-ATLAS-DESIGN.md` | Prior project handoff and workflow constraints. A copy is retained as historical context in `references/`. |
| Existing version-2 QA script | Starting point for regression checks, extended for the new shell, gallery/list control, themes and viewport range. |

This iteration does not claim a new review of the original 19 screenshots, current GitHub source, current Railway deployments or current production state. No external source research was needed to apply the supplied design reference. All operational text and fixtures inherited from the previous prototype remain demonstration material, not newly verified facts about production.

## 3. Source-derived choices and deliberate adaptations

| Reference description | Version-3 use | Classification |
|---|---|---|
| White canvas with `#141414` ink | Default page and primary action palette | Source-derived |
| `#f3f3f3` soft surfaces, `#f0f0f0` fields, `#e0e0e0` stronger borders | Navigation, evidence readiness, inputs and outlined controls | Source-derived |
| Stadium-shaped controls | Primary navigation, buttons, filters, tab tracks and view switch | Source-derived |
| 24px containers and 16px input corners | Panels, inspectors, sign-in card, form fields and warning blocks | Source-derived |
| 30% icon-tile corners | Product and chain tiles; token illustrations in the optional card view | Source-derived geometry, adapted subject matter |
| Saans at 652 / 456 / 300 | CSS requests those weights; installed-font fallback remains available | Source-derived intent, environment-dependent rendering |
| Zero tracking and sentence-case titles | Natural letter spacing and declarative headings ending in periods | Source-derived |
| Blue reserved for commercial emphasis | No blue accent is used, because this workspace has no relevant commercial promotion | Source-derived restriction |
| Floating centered navigation | One detached sticky application header replaces the fixed left sidebar | Product adaptation of the navigation pattern |
| Large editorial heading scale | Application headings use 48px desktop and 36px phone rather than an 80px marketing hero | Explicit density adaptation |
| Inverse footer with rounded geometry | Compact dark evidence/snapshot footer in light mode | Source-derived pattern, reduced scope |
| No dedicated marketing success/error palette | Operational states use explicit words, icons and hierarchy, without arbitrary green/red outcome colors | Product adaptation; meaning remains in text |
| No complete dark-mode specification | A neutral inverse theme preserves the same structure and interactions | Original extension, not presented as a Mobbin token set |

The reference contains illustrative modal/toast shadow descriptions alongside its broader shadow-free rules. This version chooses flat, shadow-free treatment for dialogs and controls. It does not claim that every illustrative example in the reference is identical.

## 4. Information architecture

### Overview

The page answers: what is loaded, what needs attention and what can be concluded?

The summary strip presents four scoped outputs: loaded research sessions, sessions needing attention, captured records on the loaded page and net after costs. These are not lifetime totals, performance metrics or a profitability score. The word **Unknown** remains a valid, prominent result.

A session panel sits beside a softly filled evidence-readiness panel. The session panel leads with an actual fixture exception when present, then lists observed states. The readiness panel separately labels retained capture, failed simulation, incomplete explicit costs and unestablished execution eligibility. This is not a progress score.

The latest-evidence section follows. It preserves exact minor units and an inspection action instead of promoting gross delta into a headline return.

### Opportunities

Retain four workspaces:

- **Captured records:** search, evidence-state filter, list/card view, record inspection and loaded-page scope.
- **Decision evidence:** observation scope, the stored decision, grouped windows, source continuity and inspection.
- **Cost research:** explicit assumptions, missing/zero/known cost distinctions, exact integer arithmetic and a separate hypothetical result.
- **Exports & audit:** locally frozen sample export and the inherited audit presentation contract.

The new List / Cards control changes presentation only. It preserves search and evidence filters. Both presentations inspect the same record and preserve rejection, unknown net, exact quantities and provenance. The prototype has one captured fixture; the gallery does not invent extra records to fill its grid.

### Experiments

Keep configuration selection, immutable mode/network/configuration, an explicit experiment reference and a review acknowledgement. The review panel is softly filled, not a second competing primary form.

Creating a local preview session produces a recovering session. It does not start a worker, operate a wallet or submit a transaction. A separate, clearly labelled preview action can simulate readiness.

### Runs

Retain Research sessions, Paper ledger, Journal and Reservations as separate tabs. Paper accounting does not become an implied execution result.

### Strategies and System

Strategies remains a read-only configuration registry. System separates API presentation, observed worker state, collection evidence and adapter capabilities. One apparently healthy component does not establish health or freshness for the others.

### Account and real trading

Account preferences are behind the desktop avatar and mobile More menu. The sign-in page is redesigned, but credential inputs remain deliberately disabled in the offline prototype. The real-trading workspace remains unavailable. No interface action can unlock live execution.

## 5. Shell and responsive composition

### Desktop, 1120px and above

Use one sticky floating header, visually detached from the viewport edges. It contains the Arbitrage identity, all six primary destinations, search, theme control and account preferences. An active navigation item is a white pill on the soft track, not a colored underline.

The content column is capped at 1440px including its side padding; the header is capped at 1392px. Normal content padding is 40–48px. The header is 76px high and sits 16px below the viewport top when sticky.

The context row identifies the private workspace, active page and synthetic preview. The page heading and contextual controls follow. Neither account controls nor full technical configuration should dominate the content column.

### Tablet, 761–1119px

Collapse the desktop destination list. A floating bottom capsule supplies Overview, Evidence, Runs and More. More contains every remaining destination and account action.

The main layout becomes single-column where paired panels would compress record names or evidence explanations. At intermediate widths, summary statistics can remain four across. Forms stack earlier than on a desktop; inputs are not shrunk to preserve an unnecessary second column.

### Phone, 760px and below

Use a 64px floating header, 20px content padding and the persistent four-item navigation capsule. At 320px, reduce content padding to 16px, not the touch-target sizes.

The summary becomes two by two. Session rows retain name, explanation, observed state and a 44px inspector control. Repeated mode metadata is removed from the compact row, but the selected session's mode remains visible in its inspector.

Wide captured-record tables become cards; other intrinsically tabular technical data may scroll within their own containers. Tabs remain horizontally scrollable pill tracks. Essential caveats stay in the visible task, not exclusively in an off-screen tab.

The evidence inspector occupies the entire phone viewport. Its header and footer remain available while the content scrolls. Desktop inspectors are detached, rounded panels on the right with 16px outside margins.

Safe-area insets are reserved around the bottom navigation and dialog controls. The page has sufficient trailing padding for the final content to scroll above fixed navigation.

## 6. Visual tokens

### Core palette

| Token | Light | Dark extension |
|---|---|---|
| Canvas | `#ffffff` | `#141414` |
| Ink | `#141414` | `#f5f5f5` |
| Body | `#262626` | `#e0e0e0` |
| Muted text | `#707070` | `#aaaaaa` |
| Soft surface | `#f3f3f3` | `#202020` |
| Input field | `#f0f0f0` | `#262626` |
| Soft hairline | `#f0f0f0` | `#2b2b2b` |
| Control boundary | `#e0e0e0` | `#3e3e3e` |
| Primary action | `#141414` | `#f5f5f5` |
| Primary action text | `#ffffff` | `#141414` |
| Focus ring | `#141414` | `#ffffff` |

The reference's faint text value is not used as the sole carrier of critical evidence, input labels or warnings. No yellow, commercial blue, gradients or decorative color bands are introduced.

### Typography

The stack is `"Saans", Inter, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif`. Font binaries are not included, fetched or redistributed. The standalone file uses a locally installed match or a system fallback. The exact appearance therefore depends on the fonts installed by the viewer.

| Role | Size | Requested weight | Notes |
|---|---:|---:|---|
| Page heading | 48px desktop / 36px phone | 652 | Sentence case; 1.1 line-height |
| Main panel heading | 21–24px | 652 | Compact application scale |
| Main metric | 48–56px | 652 | Tabular numerals; no animated counters |
| Lead paragraph | 15–16px | 300 | Lighter visual counterpoint |
| Body | 14–15px | 456 | 1.43–1.5 line-height |
| Buttons | 13–14px | 600 | 44–48px targets |
| Supporting metadata | 11–13px | 456 | Not the only location for a material caveat |
| Exact integers / identifiers | 11–13px | Normal monospace | Product-specific data exception |

Uppercase literal API enum values may remain in technical details. Brand headings and navigation do not adopt an all-caps style. No negative letter-spacing is applied.

### Geometry and spacing

Use a 4/8px spacing rhythm. Principal content panels and desktop inspectors use 24px corners; inputs and compact internal notices use 16px. Pill controls use 9999px. Icon tiles use a 30% radius.

Ordinary controls are at least 44px high. The larger desktop call to action and phone inputs are 46–48px. The prototype uses tighter section rhythm than the marketing reference because this is an operational workspace, not a promotional page.

## 7. Component behavior

### Buttons and state feedback

Primary actions are ink pills. Secondary actions are white or soft pills. A state-changing command retains its named target, clear verb and confirmation. Do not use color alone to communicate destructive scope.

Successful local actions use persistent state and screen-reader announcements. No profit counters, animated tickers or decorative success toasts are added.

### Forms

Resting fields are filled, borderless and 16px-rounded. Focus uses an ink outline. Missing cost inputs remain missing. Explicit zero is a separate selection. Huge integer strings wrap where displayed, rather than being silently rounded or visually truncated.

### Evidence inspector

The opening summary says what failed and what remains unknown. Route/cost and provenance tabs provide detail without moving the material limitation away from the result. Source type remains visible. The same record remains selected when tabs change.

Desktop drawers are 660px maximum width with a dimmed backdrop and no decorative shadow. At phone widths they become full-screen. Escape closes the inspector and focus returns to the originating control when it remains present.

### Search

Search is local to the demonstration pages, sessions and record. Cmd/Ctrl+K and `/` open it. It is not a global database search and performs no network retrieval.

### View switch

List and Cards are a two-option control with `aria-pressed`. Filters and the record identity are retained. Changing the view does not freeze new evidence, re-evaluate a quote or refresh source timestamps.

## 8. Non-negotiable research semantics

Preserve these rules when porting the interface into production:

1. Synthetic fixtures are labelled. Do not connect a live endpoint to a still-labelled fixture without a deliberate data-layer change.
2. A command request is not worker acknowledgement. Sending, pending, applied, rejected and delivery-uncertain states remain distinct in the production contract.
3. Draining is not stopped while outcomes remain unresolved. Closing a tab is not a stop command.
4. A view filter does not change a session's mode, configuration or explicitly enumerated bulk-control targets.
5. A session, captured market record, decision observation, hypothesis and accounting run are separate entities.
6. Unknown, missing and zero remain distinct. A gross quote is not a complete net estimate or a realized return.
7. Exact minor-unit quantities remain integer strings. `BigInt` is used in the demo arithmetic; no token decimals are guessed.
8. A saved hypothetical cost scenario does not mutate its original observation or automatically book a paper result.
9. A frozen export is a copy of the selected synthetic sample. It is not a verified full database export.
10. No action in this prototype contacts an API, changes credentials, submits transactions or modifies production infrastructure.

The new visual layer does not alter those semantics. All acknowledgement/readiness/reconciliation buttons labelled as simulations are prototype test controls, not features to copy into the connected product.

## 9. Source structure

```text
arbitrage-mobbin-v3/
  Arbitrage-Research-Mobbin-v3.html  # Self-contained edition
  index.html                      # Editable multi-file entry point
  ARBITRAGE-MOBBIN-DESIGN-v3.md     # This document
  README.md
  CHANGELOG.md
  assets/
    app.js                        # Original local behavior plus v3 shell/views
    foundation.css                # Retained v2 component mechanics
    mobbin.css                    # V3 tokens, styling and responsive adaptation
    styles.css                    # Generated combined stylesheet
  references/
    DESIGN-mobbin.md               # User-supplied reference, unchanged
    ARBITRAGE-ATLAS-DESIGN-v2.md    # Prior handoff, historical context
  previews/                       # Browser-rendered views, not generated mockups
  qa/
    test_prototype.py
    results.json
    QA-REPORT.md
  tools/
    build_standalone.py
```

The retained foundation stylesheet is a prototype migration aid. Do not introduce two competing theme systems into the production application. During implementation, port the final computed tokens and components into the existing component/style architecture and remove superseded rules.

## 10. Suggested implementation order

**Shell and tokens:** introduce the new palette, geometry and responsive navigation while keeping current routes and state containers intact. Confirm that every destination is available at the tablet navigation breakpoint.

**Read-only views:** port Overview, captured evidence, strategy registry and System. Preserve data scope, provenance and all empty/stale/error states before modifying forms.

**Inspectors and forms:** port summary-first inspection, cost inputs and immutable-session review. Preserve focus behavior, validation and exact quantities.

**Command surfaces:** restyle the existing command components without replacing their backend receipts, idempotency or acknowledgement logic with this prototype's simplified local state.

**Verification:** repeat the existing application tests and add visual, keyboard, narrow-screen, long-value and authentication-transition tests. Run connected end-to-end testing separately; offline visual QA is not proof of production behavior.

## 11. Validation and boundaries

The included QA script checks the standalone code in Chromium using Playwright. `qa/results.json` is the actual recorded result; `qa/QA-REPORT.md` summarizes it. The test grid covers both themes, 320–1440px viewports and application tab variants. Browser-rendered screenshots are included for inspection.

This is not a formal accessibility certification, an on-device Safari test, a security audit or a production integration test. No claim is made that the original full application has been converted, merged or deployed. Current backend fields, authentication policies and deployment state require their own verification during integration.
