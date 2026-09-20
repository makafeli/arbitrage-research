# Design — Arbitrage Research dashboard

Locked design system for `apps/web`. Future Hallmark runs read this file first;
pages defer to it. Amend intentionally — the file is the rule.

## Genre
modern-minimal — the dev-tool / instrument-panel register. No decoration, no
colour signal beyond ink, function carries every page.

## Macrostructure family
- App pages (every route in `apps/web`): **Workbench**. Small functional
  headings, data blocks separated by hairlines and gap, no hero, no enrichment.
  Variation knobs: section rhythm (panel grid vs. table vs. facts list) and the
  optional mono eyebrow above a heading.
- Navigation primitives inside a page: **tabs** (`role=tablist`, arrow-key
  roving focus) split a page's blocks when they don't all fit one scroll, and
  **right-side drawers** (`dialog.drawer`) inspect one record without leaving
  the page. No third pattern; do not invent accordions or nested modals.
- Marketing pages: none exist. If one is added, use Marquee Hero with this
  theme and open a `## Variants` section here first.
- Content pages: none.

## Theme
Monochrome. Ink on paper, no accent hue — the only non-neutral colour is the
warn tone, kept for STALE badges and the Owner-overview health pill.
Axes · flat white paper / grotesk-free sans display / ink-on-paper accent.

Light (default, `:root`):
- `--color-paper`      #ffffff
- `--color-paper-2`    #f3f3f3
- `--color-paper-3`    #e9e9e9
- `--color-ink`        #141414
- `--color-ink-2`      #6b6b6b
- `--color-rule`       #f0f0f0
- `--color-rule-2`     #dcdcdc
- `--color-accent`     = `--color-ink`
- `--color-accent-ink` = `--color-paper`
- `--color-warn`       oklch(52% 0.13 70)
- `--color-ok`         oklch(52% 0.13 145)
- `--color-danger`     oklch(52% 0.13 25)
- `--color-focus`      = `--color-ink`

Dark (`body.dark`):
- `--color-paper`      oklch(20% 0.005 260)
- `--color-paper-2`    oklch(23.5% 0.005 260)
- `--color-paper-3`    oklch(27.5% 0.005 260)
- `--color-ink`        oklch(95% 0 0)
- `--color-ink-2`      oklch(76% 0 0)
- `--color-rule`       oklch(31% 0.005 260)
- `--color-rule-2`     oklch(38% 0.005 260)
- `--color-accent`     = `--color-ink`
- `--color-accent-ink` = `--color-paper`
- `--color-warn`       oklch(82% 0.13 80)
- `--color-ok`         oklch(82% 0.13 145)
- `--color-danger`     oklch(82% 0.13 25)

Accent budget: monochrome by default — "accent" is ink-on-paper contrast, not
a hue. Three exceptions only: `--color-warn` on the STALE badge,
`--color-danger` on the error notice's left rule and title, and the
Owner-overview health pill (`.pill.red/.amber/.green` on `--color-danger`,
`--color-warn`, `--color-ok`), which keeps its traffic-light colour because it
is the one place the owner reads status by colour alone at a glance. Never a
flood, never a gradient, never introduced anywhere else.

## Typography
- Display: Inter, weight 600, style normal
- Body:    Inter, weight 400 (500 for emphasis)
- Mono:    JetBrains Mono, weight 400/500 — numbers, hashes, badges, eyebrows,
           table heads, kbd
- Display tracking: 0 (all tracking is 0 except mono labels, which keep a
  small positive tracking for legibility at caps)
- Type scale anchor: `--text-display` = clamp(2rem, 1.4rem + 1.8vw, 3rem)
- No italic headings. No uppercase outside mono labels and the honesty
  badges, which stay UPPERCASE mono — a project requirement, not a style
  choice (`API CONNECTED`, `CAPTURED DATA ONLY`, `IMMUTABLE`, …).

## Spacing
4-point named scale in `apps/web/src/tokens.css` (`--space-3xs` … `--space-4xl`).
Pages use named tokens, never raw pixel values, except hairline widths and the
44 px touch-target floor.

## Motion
- Easings: `--ease-out` cubic-bezier(0.16, 1, 0.3, 1) · `--ease-inout`
  cubic-bezier(0.65, 0, 0.35, 1)
- Durations: `--dur-fast` 120ms · `--dur-base` 180ms · `--dur-slow` 240ms
- Reveal pattern: none. The dashboard is composed; data appears instantly.
- Hover: background / border-colour shift only. Focus rings appear instantly.
- Drawers slide in 180ms via `transform` only, from `translateX(100%)` using
  `@starting-style`; no motion under reduced motion.
- Reduced-motion fallback: every transition and animation off.

## Microinteractions stance
- Silent success. No toasts, no confetti, no celebratory colour.
- Hover transition 120 ms · focus delay 0 ms.
- Status is spoken by the mono badges (PAPER TRADING, HYPOTHETICAL, CAPTURED
  DATA ONLY, …). Those badges are a project requirement and stay uppercase.
- No data enrichment: never show a metric, health card, or readiness signal
  the API cannot back. A tile that would always read "Unknown" is decoration,
  not a feature.

## CTA voice
- Primary: ink-filled pill (`button.primary`) — `--color-ink` fill,
  `--color-paper` text, radius 9999px, `--space-xs` × `--space-md` padding,
  weight 500, names the action ("Start session", "Sign in").
- Secondary / default: ghost pill — transparent fill, 1 px `--color-rule-2`
  border, radius 9999px. Hover moves to `--color-paper-2`.
- Text buttons: ink colour, underline on hover, no border.

## Radii
Chips, pills and nav buttons use `--radius-chip` (9999px, full stadium).
Inputs use `--radius-input` (12px); inset surfaces (`.fact`, notices, table
wrappers, cards inside a panel) use `--radius-card` (16px); panels (`.panel`) use `--radius-panel` (24px).
No other radius values.

## Shell
- Brand: a black tile holding "↗" (aria-hidden) plus the wordmark
  "Arbitrage." (the trailing period is part of the wordmark, not sentence
  punctuation).
- ≥1281px: a floating `header.topbar` — sticky top, rounded pill, hairline
  border, paper background — holds the brand, primary nav, the trading-mode
  segment, and account actions in one row.
- ≤1280px: the primary nav becomes a fixed bottom capsule (rounded pill,
  paper background, hairline, shadow), horizontally scrollable, every item
  reachable by scroll — no overflow "more" menu.

## Per-page allowances
- App pages MUST NOT use enrichment — function carries the page.
- Surfaces are hairline-bordered (`--color-rule`), 24px radius for panels /
  16px for inset cards, no shadow. The only shadows in the app are the
  drawer/dialog lift and the floating shell surfaces; the backdrop scrim does
  the rest.
- A page with more content than one scroll comfortably holds splits into
  tabs; a page needs one record inspected in isolation, it opens a drawer.
  Do not add a third navigation primitive.

## What pages MUST share
- The wordmark: black tile "↗" + "Arbitrage." in Inter 600.
- The monochrome palette and the single reserved warn tone.
- The three fonts and the type scale.
- The CTA voice above.
- The heading rhythm: optional mono eyebrow → display heading → one-line
  subtitle in `--color-ink-2`.
- Accessibility: focus rings on every interactive element
  (`:focus-visible { outline: 2px solid var(--color-focus) }`), reduced
  motion disables all transitions/animations, colour never carries meaning
  alone (badges and pills always pair colour with text).

## Exports
`apps/web/src/tokens.css` is the source of truth. Ask "extend design.md with
Tailwind exports" (or DTCG / shadcn) if another consumer needs them.
