# Dashboard design system — v0.2

The approved visual reference is [dashboard-wireframe.html](dashboard-wireframe.html). The React implementation lives in [apps/web](../apps/web/README.md). This update preserves the reference's quiet dark shell, lime primary action, six destinations, peer Solana/Base panels and inspectable opportunities. It clarifies that workspace controls operate a group of independent per-network sessions.

## Tokens

CSS variables in `apps/web/src/styles.css` are the current token source. Colors never carry status meaning alone.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | `#0b1118` | `#f4f6f8` | Application canvas |
| `--panel` | `#111b26` | `#ffffff` | Navigation and cards |
| `--raised` | `#172330` | `#eef2f6` | Selected navigation and secondary surfaces |
| `--line` | `#2a3a4b` | `#d6dfe8` | Dividers and outlines |
| `--text` | `#e9eff6` | `#152331` | Primary text |
| `--muted` | `#a4b4c5` | `#526476` | Supporting text |
| `--accent` | `#b6e779` | `#345a15` | Primary action and paper evidence |
| `--accentink` | `#182510` | `#ffffff` | Text on primary action |
| `--blue` | `#8cbcff` | `#285aa3` | Links, focus and simulated evidence |
| `--amber` | `#f2cb7d` | `#845604` | Synthetic notice and quality attention |

Use the system sans-serif stack, 15px base copy and approximately 1.5 line height. Page titles are 30px desktop and 27px compact; card titles are 17px. Financial amounts use tabular numerals. Maintain 4/8px spacing rhythm, approximately 20px card padding, 11px panel radii and 8px button radii.

## Layout and navigation

Desktop has a 220px sidebar and a centered content area up to 1480px. The six primary destinations are Overview, Opportunities, Experiments, Runs, Strategies and System. The mode, synthetic banner, grouped control state and selected chain view remain visible across navigation.

Navigation wraps into a compact row on small screens. Chain panels stack at narrow widths. Opportunity rows become cards, preserving route, evidence, net denomination, quality and Inspect action. Touch controls have at least 44px height; avoid horizontal scrolling by allowing supporting text to wrap. Target browser QA widths: 320, 390, 768 and 1440px. Responsive CSS is implemented; browser acceptance results belong in the validation report, not inferred from the CSS.

## Reusable components

| Component | Responsibility |
|---|---|
| App shell | Navigation, theme, chain-view filter, persistent provenance notice |
| Grouped controls | Demonstrate commands to two separate per-network sessions |
| Chain panels | Same-cohort funnel counts, fixed capture time, coverage and quality |
| Opportunity table/cards | Route, evidence, input, net USDC range, quality and inspection |
| Evidence badge | Candidate, Simulated or Estimated executable text with supporting color |
| Native opportunity dialog | Route and fictional cost detail, evidence caveat, close and focus restoration |
| Runs view | Per-session state and command acknowledgment; separate draining scenario |
| System view | Illustrative freshness and recovery requirements; explicit disconnected status |

## Evidence and financial language

Every view remains marked synthetic in this scaffold. Fixtures are UI illustrations, not simulator artifacts. Paper examples never have Realized evidence. Candidate costs remain Unknown when not evaluated. UI amounts use USDC explicitly; USDC is not assumed equal to USD. Pool fees and impact are included in the fictional route quote; additional costs are listed separately to avoid double counting.

In the eventual connected application, provenance must come from validated backend records. Do not infer evidence from a positive number, a successful quote, a button click or elapsed time. Only successful full atomic-transaction simulation can support Simulated; Estimated executable also requires all configured eligibility checks and a named scenario. Keep rejection, execution state and evidence separate.

## Command behavior

A session belongs to one immutable network and mode. The dashboard demonstrates a run group containing one Solana PAPER session and one Base PAPER session. View-chain filtering never changes either session's scope.

Start/Pause/Resume currently apply local demo state. Stop first shows PENDING and PAUSING, then separate illustrative acknowledgments at 650ms for Solana and 1050ms for Base. These timers are a UI demonstration; production must use authoritative worker acknowledgments with no timer-based assumption of success. A group can show one stopped session while another command is still pending. In the no-attempt paper demo, acknowledgment immediately permits STOPPED.

The Runs screen has a separate synthetic prior-submission example. After its admission fence is applied it remains DRAINING while an outcome is unresolved, then becomes STOPPED only after illustrative reconciliation. A stop cannot recall emitted transactions. No blockchain operation exists in this scaffold.

## Accessibility and interaction

Use semantic landmarks, visible keyboard focus, associated labels, an actual navigation landmark and `aria-current` on the selected destination. The skip link targets main content. Page navigation moves focus to the main title; theme changes preserve the selected view. Important updates are announced through one polite live region.

The opportunity dialog uses `showModal()` for native focus containment and inert background behavior. Close and Escape dismiss it; the previous Inspect control regains focus. Preserve reduced-motion preferences and text enlargement. Browser tests cover navigation, themes, dialog focus, session controls, independent stop acknowledgments and narrow-width overflow. Full screen-reader and WCAG 2.2 AA review remains a delivery gate; passing a smoke test does not establish complete conformance.

## Delivery boundary

The implemented frontend has no API client, market connection, wallet input, signer, transaction broadcasting or persistent run storage. Reloading resets the local demo. Dashboard-to-Rust integration, authentication, server-authoritative controls, streaming updates and real exports remain backlog work. Local dependency/build/test evidence is recorded in [the frontend README](../apps/web/README.md).
