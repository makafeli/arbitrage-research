# BUILD SPEC — ARB-070 · v3 monochrome redesign (option B: "v3 jas, app inhoud")

Shared contract for every worker on this ticket. Read fully before editing. If the spec and the
code disagree, stop and report to the captain; do not invent.

Sources of truth, in order: this file → `docs/redesign-handoff/VERSCHIL-RAPPORT-v3.md` (decisions,
"A = v3 / B = current", the captain's advice per point is the decision) →
`docs/redesign-handoff/VERLIES-CHECK-v3.md` (every current component and where it lands) →
`docs/arbitrage-mobbin-v3/` (visual reference only: `index.html`, `assets/styles.css`, `assets/app.js`).

## 0. Hard rules (all workers)

- Scope: `apps/web` only. No Rust, API, worker, `planning/`, `specs/`, `.github/` changes. No new npm deps.
- **Copy is frozen.** Every heading, label, button text, notice, badge text, `aria-label`, `id` and `htmlFor`
  that exists today stays byte-identical. Badges stay UPPERCASE mono (`API CONNECTED`, `CAPTURED DATA ONLY`,
  `IMMUTABLE`, …). Do not add v3 marketing copy ("Net after costs", slogans, preview chips, breadcrumbs, search).
- **DOM contracts tests depend on** (never change): `nav[aria-label="Primary navigation"]` containing one
  `<button>` per page (exactly one such nav in the DOM — the mobile capsule is the same element, repositioned by
  CSS); `nav[aria-label="Trading mode"]` with buttons `Paper trading` / `Real trading` (`aria-pressed`);
  buttons `Change password`, `Sign out`; text `API CONNECTED` / `STALE / LAST KNOWN`; select `View chain`
  (`#connected-chain`); headings `Paper trading overview`, `Sign in`, `Set your password`,
  `Real trading is not available`; button `Start real trading` (disabled); `a.skip[href="#main"]`; `main#main`;
  `div.sr[role=status][aria-live=polite]`; all `aria-labelledby`/`aria-label` regions in components.
- Theme: **light is the default**. `body.dark` switches to dark. Toggle button `aria-label` =
  `Switch to dark theme` / `Switch to light theme` (visible text `Dark theme` / `Light theme`).
  `body.light` no longer exists. (One spec, `shell-acceptance.spec.ts`, is updated for this by the tests worker.)
- Every colour and font-family in CSS references a token in `tokens.css`. Token **names stay**; values change.
- Reduced motion: all transitions/animations off under `prefers-reduced-motion: reduce`.
- Focus: visible 2px outline (`--color-focus`) with offset on every interactive element.
- No `console.log`. No `dangerouslySetInnerHTML`. No hash router. No `localStorage` except the existing
  Owner-overview language key.
- Never run `git add/commit/stash/checkout/rebase` — the captain commits. Edit only your write paths.
- Before you finish: `npm test` and `npm run build` (in `apps/web`) must pass. Run
  `npm run test:browser` and report failures with the reason (expected, e.g. "needs tab click" vs a real bug).
  Do not edit spec files unless they are in your write paths.

## 1. Tokens (`apps/web/src/tokens.css`) — values only, names unchanged

```
:root (light default)                      body.dark
--color-paper      #ffffff                 oklch(20% 0.005 260)
--color-paper-2    #f3f3f3   (v3 --soft)   oklch(23.5% 0.005 260)
--color-paper-3    #e9e9e9                 oklch(27.5% 0.005 260)
--color-ink        #141414   (v3 --ink)    oklch(95% 0 0)
--color-ink-2      #6b6b6b                 oklch(76% 0 0)
--color-rule       #f0f0f0   (v3 hairline) oklch(31% 0.005 260)
--color-rule-2     #dcdcdc                 oklch(38% 0.005 260)
--color-accent     var(--color-ink)        var(--color-ink)      (monochrome: accent = ink)
--color-accent-ink var(--color-paper)      var(--color-paper)
--color-warn       oklch(52% 0.13 70)      oklch(82% 0.13 80)    (kept: amber for STALE + Owner health pill)
--color-danger     oklch(52% 0.13 25)      oklch(82% 0.13 25)
--color-focus      var(--color-ink)
--color-scrim      oklch(10% 0 0 / 0.45)   oklch(0% 0 0 / 0.7)
--shadow-lift      0 1px 2px oklch(0% 0 0 / 0.06)   0 1px 2px oklch(0% 0 0 / 0.4)
--font-display     "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif   (Space Grotesk removed)
--font-body        same as display
--font-mono        unchanged
--text-display     clamp(2rem, 1.4rem + 1.8vw, 3rem)      (v3 h1 48 → 32)
--tracking-display 0        --tracking-mono 0.04em
--radius-chip      9999px   --radius-input 12px   --radius-card 16px   (+ new --radius-panel 24px)
spacing, type scale (except display), easing, durations, --touch: unchanged
color-scheme: light on :root, dark on body.dark
```

Owner-overview health pill keeps red/amber/green (`.pill.red/.amber/.green`) — the only colour pills.

## 2. Shell DOM (`ConnectedApp.tsx`, owned by the shell worker)

```
a.skip[href=#main]
div.shell
  header.topbar                         ← floating 76px bar, sticky top, rounded 9999, hairline, paper bg
    div.brand  span.brandmark(aria-hidden, "↗" in a black tile) + "Arbitrage."  (text "Arbitrage." exact)
    nav.nav[aria-label="Primary navigation"]   7 buttons (Overview … System), aria-current="page" on active
    nav.trading-modes[aria-label="Trading mode"]   segment pill, 2 buttons, aria-pressed (unchanged)
    div.topactions
      span.pill   "PAPER TRADING" | "REAL TRADING"
      button      "Change password"                                    (disabled when unresolvedRequest)
      button[aria-label="Switch to dark theme"|"Switch to light theme"]   text "Dark theme"|"Light theme"
      button      "Sign out"
  main#main.workspace
    (PasswordSettings inline panel — unchanged position, restyled)
    section.panel.real-trading-panel (unchanged content)
    p#unresolved-mode-note.notice (unchanged)
    div.pagehead  h1[ref, tabIndex=-1] + p.subtitle + div.filter (View chain select)   ← h1 stays the page name
    div.notice.error-notice (unchanged)
    section.panel[role=status] loading (unchanged)
    section.controlbar[aria-label="Connected session control summary"]   ← the v3 "safety strip": one row,
        pill + statustext + tiny + .session-states + command chips + "Stop all loaded sessions (n)" + "Refresh"
    <page component>   (one per page, see §4)
    footer.footer      (unchanged text)
    div.sr[role=status][aria-live=polite][aria-atomic=true]
RecordedDialog (outside .shell, unchanged)
```

Sidebar is removed. `.subbrand`, `.navlabel`, `.sidefoot` go away (copy "TRADING WORKSPACE", "Paper workspace"
labels were decoration; dropping is allowed — VERSCHIL point 3).

Breakpoints (v3): 1600 / 1480 / **1280** (capsule; v3 used 1119 but the app has 7 pages + 3 account actions) / 940 / 760 / 359.
- ≥1281: header holds brand + nav + segment + actions in one row (nav and action buttons compact, `white-space: nowrap`; the PAPER TRADING pill is hidden below 1600 —
  the banner strip already carries it). `Change password`,
  `Sign out` must still resolve by name).
- ≤1280: `nav.nav` becomes a **fixed bottom capsule** (rounded 9999, paper bg, hairline, shadow), horizontally
  scrollable (`overflow-x:auto`, `scrollbar-width:none`), **all 7 items** stay in it (no "More" dialog —
  ponytail: scroll instead, tests click every item at 320px). `main` gets bottom padding ≥ 88px. Header keeps brand + segment + actions on one row down to 761px.
  ≤760: header wraps to two rows — row 1 brand + segment (space-between), row 2 the three action buttons (nowrap each, row may wrap). No icon-only buttons.
- ≤760: pagehead stacks; `.filter` full width; controlbar wraps.
- ≤359: brand text hidden (tile only), segment buttons shorter text via CSS is NOT allowed (copy frozen) →
  keep full text, allow wrap.
- No horizontal overflow at 320 / 375 / 390 / 414 / 768 / 1440 (tests assert ≤1px).

## 3. Primitives (`styles.css`, owned by the styles worker; class names below are the contract)

| Class | Look |
|---|---|
| `.panel` | paper bg, 1px `--color-rule`, radius `--radius-panel` (24), padding 24 (16 ≤760) |
| `.panel.inset` / `.fact` | `--color-paper-2` bg, radius `--radius-card`, no border |
| `.pill` | mono, uppercase, `--text-xs`, tracking `--tracking-mono`, radius 9999, 1px rule; `.blue` = ink bg + paper text; `.amber` = warn bg-tint + warn text; `.red/.green` only Owner overview |
| `button` default | ghost: transparent, 1px `--color-rule-2`, radius 9999, height ≥ 36, `--touch` on ≤760 |
| `button.primary` | ink bg, paper text, radius 9999 (v3 black pill CTA) |
| `button.textbutton`, `.cellbtn` | underline-on-hover text buttons (unchanged semantics) |
| `input, select, textarea` | 1px rule-2, radius `--radius-input`, 40px tall, paper bg |
| `.tabs` | `div.tabs[role=tablist]` > `button[role=tab][aria-selected]`; selected = ink bg + paper text pill; others ghost |
| `.tabpanel` | `div[role=tabpanel]`, `hidden` when inactive, `margin-top: var(--space-lg)` |
| `dialog.drawer` | right-side drawer 660px (100% ≤760), full height, paper bg, radius 24 on the left edge, `::backdrop` = scrim; slide-in 180ms, none under reduced motion |
| `.dialoghead` | title row inside drawers: h2 + close button |
| `.drawer-tabs` | `.tabs` inside a drawer |
| `.sectionhead` | h2 + p + optional pill, flex, wraps |
| `.cards` | grid auto-fill minmax(320px,1fr) |
| `.listrow` | row with hairline bottom, wraps ≤760 |
| `.facts`, `.research-facts`, `.research-metrics`, `.chainmetrics` | grid of `.fact`; 2 cols ≤900, 1 col ≤540 |
| `table`, `.research-table`, `.tablewrap`, `.research-scroll` | hairline rows, mono numerics, horizontal scroll container with `tabindex=0` kept |
| `.notice` | `--color-paper-2` bg, radius `--radius-card`, 1px rule; `.error-notice` = danger left border 3px |
| `.stats` | NEW: strip of `.fact` tiles for Overview mini-stats (grid 4 → 2 → 1) |
| `.steps` | NEW: Experiments: `ol.steps > li` with `span.stepnum` "01"/"02"/"03" mono before the existing labels |
| `.controlbar` | v3 safety strip: paper-2 bg, radius 16, flex wrap, gap 12 |
| `.topbar`, `.brand`, `.brandmark`, `.nav`, `.trading-modes`, `.topactions` | see §2 |
| `.account-screen`, `.account-card`, `.account-top`, `.account-aside` | Sign-in split: left aside (brand + 3 one-line facts from the current sign-in copy), right card ≥1120; stacked below |

Typography: h1 `--text-display`, weight 600, letter-spacing 0; h2 `--text-xl` 600; h3 `--text-lg` 600;
body `--text-base`; `.tiny` `--text-sm` `--color-ink-2`; `.mono` `--font-mono`.

Keep every existing class in `styles.css` that a component still uses (list them with
`grep -o 'className="[^"]*"' -r src | sort -u`). Remove `.sidebar`, `.subbrand`, `.navlabel`, `.sidefoot`,
`.demo`, `.login-panel` only if no TSX references them.

## 4. Page components (Wave 2; one worker per row, disjoint files)

Shell worker (Wave 1) extracts these from `ConnectedApp.tsx` into `src/components/pages/` so Wave 2 can own them.
Each page component receives what it needs via props (the same values ConnectedApp passes today) and renders
exactly the JSX that lives in ConnectedApp today — extraction is a pure move, no visual changes in Wave 1.

| Page | File(s) owned in Wave 2 | v3 structure to build |
|---|---|---|
| Overview | `pages/OverviewPage.tsx`, `components/SessionCard.tsx`, `components/RecordedOpportunities.tsx`, `components/OpportunityTable.tsx` | `.stats` strip (sessions loaded, running, unresolved commands, records on page — all from existing props, no new data), then Research sessions cards, then Recorded opportunities table; record `<dialog>` becomes `.drawer` |
| Opportunities | `pages/OpportunitiesPage.tsx`, `components/DecisionExplorer.tsx`, `components/CostAssessmentWorkspace.tsx` | `Tabs`: `captured` (Recorded opportunities) · `decisions` (session select + filters + table + groups) · `costs` (CostAssessmentWorkspace) · `exports` (FrozenSessionExport + CaptureAuditPanel). DecisionExplorer stays mounted once (state kept); "Assess hypothetical costs for …" switches to the costs tab; decision `ResearchDialog` becomes `.drawer` with the existing Summary/Continuity content (order unchanged) |
| Runs | `pages/RunsPage.tsx`, `components/PaperWorkspace.tsx` | `Tabs`: `sessions` (session cards + create hypothetical paper run) · `ledger` · `journal` · `reservations`; FrozenSessionExport stays on `sessions` |
| System | `pages/SystemPage.tsx`, `components/AdapterSupport.tsx`, `components/CollectionHealth.tsx`, `components/CapabilitiesPanel.tsx` | `Tabs`: `adapters` · `collection` · `capabilities`; capabilities tab = existing facts + NEW capability-gate table (one row per boolean in `/v1/capabilities`, value `Available`/`Unavailable`, mono key) |
| Experiments + Strategies + Real | `pages/ExperimentsPage.tsx`, `components/SessionCreation.tsx`, `pages/StrategiesPage.tsx`, `components/RealTradingPanel.tsx` | Experiments: `ol.steps` 01 Validated configuration · 02 Session network + Experiment reference · 03 review + create (labels unchanged); Strategies: `.listrow` list → `table.research-table` (Mode · Networks · Digest (mono) · Strategies · IMMUTABLE pill); Real: panel restyle only |
| Sign-in + Owner overview | `components/AccountAccess.tsx`, `components/OwnerOverview.tsx` | Sign-in split layout (aside + card), `PasswordSettings` restyle; Owner overview: primitives only, no structure change |

Shared, **no page worker edits**: `ResearchShared.tsx`, `ResearchViews.tsx`, `FrozenSessionExport.tsx`,
`CaptureAuditPanel.tsx`, `ChainFreshnessEvidence.tsx`, `DecisionContinuityEvidence.tsx`, `api/*`, `domain/*`,
`styles.css`, `tokens.css`, `ConnectedApp.tsx`, `App.tsx`, `ui/Tabs.tsx`. Need a change there → report to captain.
Page-specific CSS (only if a primitive truly does not cover it): `src/components/pages/<page>.css`, imported by
that page component.

## 5. `Tabs` primitive (`src/components/ui/Tabs.tsx`, shell worker)

```tsx
export type Tab<K extends string> = { key: K; label: string };
export function Tabs<K extends string>({ label, tabs, value, onChange }: { label: string; tabs: readonly Tab<K>[]; value: K; onChange: (k: K) => void })
// renders div.tabs[role=tablist][aria-label=label] > button[role=tab][id=`tab-${key}`][aria-selected][aria-controls=`panel-${key}`][tabIndex=selected?0:-1]
// ArrowLeft/Right/Home/End move selection and focus (roving tabindex). No context, no compound API.
export function TabPanel<K extends string>({ tab, value, children }: …)
// renders div.tabpanel[role=tabpanel][id=`panel-${tab}`][aria-labelledby=`tab-${tab}`][hidden={tab !== value}]
```
Tab labels (sentence case, these are new strings): Opportunities `Captured` · `Decisions` · `Costs` · `Exports`;
Runs `Sessions` · `Ledger` · `Journal` · `Reservations`; System `Adapters` · `Collection` · `Capabilities`.

## 6. Tests (Wave 3, one worker owns `apps/web/tests/browser/*.spec.ts` and `docs/redesign-handoff/tools/shots.spec.ts`)

Allowed edits only: (a) insert `getByRole('tab', { name })` clicks where content moved behind a tab,
(b) `shell-acceptance.spec.ts`: `light` → `dark` semantics (`Switch to dark theme`, `classList.contains('dark')`),
(c) `shots.spec.ts`: `Light theme` → `Dark theme`. Any other failure is a bug → report, do not patch the test.

## 7. Acceptance (ticket)

`npm test`, `npm run build`, `npm run test:browser` green · no horizontal scroll at 320/375/390/414/768/1440 ·
all 83 VERLIES-CHECK components present · copy/badges/headings unchanged · every colour/font via token ·
light default + dark toggle · keyboard: tabs arrow keys, drawers trap focus + Esc, skip link · no new deps ·
no Rust/API/worker changes · screenshots regenerated in `docs/redesign-handoff/screenshots/` · handoff docs 00–09
updated to the new shell.
