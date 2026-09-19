# Design — Arbitrage Research dashboard

Locked design system for `apps/web`. Future Hallmark runs read this file first;
pages defer to it. Amend intentionally — the file is the rule.

## Genre
modern-minimal — the dev-tool / instrument-panel register. No decoration, one
signal colour, function carries every page.

## Macrostructure family
- App pages (every route in `apps/web`): **Workbench**. Small functional
  headings, data blocks separated by hairlines and gap, no hero, no enrichment.
  Variation knobs: section rhythm (panel grid vs. table vs. facts list) and the
  optional mono eyebrow above a heading.
- Marketing pages: none exist. If one is added, use Marquee Hero with this
  theme and open a `## Variants` section here first.
- Content pages: none.

## Theme
Custom, Cobalt-family. Cool graphite ground by default, one electric cobalt
signal, an engineered near-white light variant behind `body.light`.
Axes · dark cool graphite paper / grotesk-sans display / electric cobalt accent.

Dark (default, `:root`):
- `--color-paper`      oklch(20% 0.016 260)
- `--color-paper-2`    oklch(23.5% 0.016 260)
- `--color-paper-3`    oklch(27.5% 0.017 260)
- `--color-ink`        oklch(95% 0.008 250)
- `--color-ink-2`      oklch(76% 0.014 252)
- `--color-rule`       oklch(31% 0.018 258)
- `--color-rule-2`     oklch(38% 0.02 258)
- `--color-accent`     oklch(74% 0.16 256)
- `--color-accent-ink` oklch(16% 0.03 258)
- `--color-warn`       oklch(82% 0.13 80)
- `--color-focus`      = `--color-accent`

Light (`body.light`):
- `--color-paper`      oklch(98.5% 0.004 250)
- `--color-paper-2`    oklch(96.5% 0.005 250)
- `--color-paper-3`    oklch(94% 0.006 250)
- `--color-ink`        oklch(24% 0.02 258)
- `--color-ink-2`      oklch(42% 0.018 257)
- `--color-rule`       oklch(89% 0.008 252)
- `--color-rule-2`     oklch(82% 0.01 252)
- `--color-accent`     oklch(50% 0.20 256)
- `--color-accent-ink` oklch(99% 0.002 250)
- `--color-warn`       oklch(52% 0.13 70)

Accent budget: ≤ 5 % of any viewport — active nav item, primary button, focus
ring, links, the status dot, progress fill, accent-coloured badges. Never a
flood, never a gradient.

## Typography
- Display: Space Grotesk, weight 600 (500 for metric values), style normal
- Body:    Inter, weight 400 (500 for emphasis)
- Mono:    JetBrains Mono, weight 400/500 — numbers, hashes, badges, eyebrows,
           table heads, kbd
- Display tracking: -0.02em
- Type scale anchor: `--text-display` = clamp(1.75rem, 1.3rem + 1.4vw, 2.25rem)
- No italic headings. No uppercase outside mono labels and the honesty badges.

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
- Reduced-motion fallback: every transition and animation off.

## Microinteractions stance
- Silent success. No toasts, no confetti, no celebratory colour.
- Hover transition 120 ms · focus delay 0 ms.
- Status is spoken by the mono badges (PAPER TRADING, HYPOTHETICAL, CAPTURED
  DATA ONLY, …). Those badges are a project requirement and stay uppercase.

## CTA voice
- Primary: solid `--color-accent` fill, `--color-accent-ink` text, 6 px radius,
  `--space-xs` × `--space-md` padding, weight 500, names the action
  ("Start session", "Sign in").
- Secondary: 1 px `--color-rule-2` border on `--color-paper-2`, same radius and
  padding. Hover moves to `--color-paper-3`.
- Text buttons: accent colour, underline on hover, no border.

## Per-page allowances
- App pages MUST NOT use enrichment — function carries the page.
- Surfaces are hairline-bordered (`--color-rule`), 10 px radius, no shadow.
  The only shadow in the app is the dialog's 1 px lift; the backdrop scrim does
  the rest.

## What pages MUST share
- The wordmark: `↗ Arbitrage` in Space Grotesk 600, glyph in accent.
- The accent colour and its ≤ 5 % budget.
- The three fonts and the type scale.
- The CTA voice above.
- The heading rhythm: optional mono eyebrow → display heading → one-line
  subtitle in `--color-ink-2`.

## Exports
`apps/web/src/tokens.css` is the source of truth. Ask "extend design.md with
Tailwind exports" (or DTCG / shadcn) if another consumer needs them.
