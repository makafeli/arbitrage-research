# Arbitrage Research — Atlas redesign

**Version:** 2.0 design proposal · 19 September 2026  
**Target:** the existing `arbitrage-research/apps/web` application  
**Visual reference:** `Trust-Atlas-DESIGN.md`  
**Delivery:** interactive offline prototype, source files, responsive previews and browser checks  
**Status:** proposed interface direction; not merged, deployed, or connected to production

## 1. The design decision

Make Arbitrage a **research workspace**, not a wall of API responses and not a speculative trading terminal.

The first screen should answer three questions: **What is running? What needs attention? What does the evidence actually establish?** Detailed configuration, retained captures, accounting entries and cost assumptions remain available, but no longer demand equal attention on every page.

Use the Trust Atlas reference for its neutral black surfaces, restrained yellow accent, precise type and evidence-conscious hierarchy. Do not import its large editorial heroes or certificate-market page structure into an operational application. Keep the Arbitrage product name, existing six primary destinations, account model and backend contracts.

The substantive redesign is the separation of workflows. Colour is secondary.

### What changes

- A compact shell replaces repeated navigation, workspace, account and control panels.
- Overview becomes a summary and triage surface, not a duplicate of Runs.
- Opportunities separates captured records, decisions, cost research and exports.
- Runs separates session operation from hypothetical accounting.
- An evidence inspector replaces oversized centre-screen dialogs; on phones it uses the full screen.
- Account actions move out of every page's main toolbar into a preferences panel.
- Mobile gets persistent, labelled navigation and compact record summaries.

### What does not change

The application remains a private, research-first workspace. Paper accounting stays hypothetical. Session mode and configuration remain immutable. API connectivity is not evidence of worker health or feed freshness. Requests remain distinct from applied worker states. Unknown costs do not become zero, and positive gross quotes do not become realized returns.

## 2. Source basis and limits

The design was developed against the 19 supplied screenshots: the production light overview, dark/light overview pairs, sign-in and recovery-help pairs, captured-record and decision-detail dialogs, Opportunities, Experiments and Runs on desktop/mobile.

The accompanying source basis is:

| Source | How it was used |
|---|---|
| `Trust-Atlas-DESIGN.md`, version 1.0, 14 September 2026 | Requested visual reference: black/yellow, surface hierarchy, typography, evidence discipline, responsive patterns. |
| `README(8).md`, “Redesign handoff — apps/web”, 19 September 2026 | Current screenshot and source-file inventory; describes `main` after PR #192, Cobalt/ARB-069. Establishes synthetic fixtures and hard redesign rules. |
| `ARBITRAGE-RESEARCH-HANDOFF-2026-09-18.md` | Product and implementation context. Its dated operational snapshot is not treated as a current production-status check. |
| `05-UX-DESIGN.md` | Evidence-first inspection, immutable experiments and asynchronous control semantics. |
| `apps/web/src/ConnectedApp.tsx`, first 180 requested lines, read from GitHub | Confirms the existing React shell, imported production components, retained workspaces, command receipts and polling approach. This was not a complete repository audit. |

The README references individual `00-shell.md` through `08-real-trading.md` documents. Those individual documents were not retrieved; the attempted repository path for `00-shell.md` returned 404. Do not treat this package as a line-by-line transcription of those missing documents.

The screenshots contain synthetic test fixtures, not actual market results. The prototype deliberately preserves that distinction. It makes no production requests, accepts no credentials and operates no workers.

## 3. Diagnosis of the supplied interface

### Repeated chrome hides the task

On mobile, the product identity, workspace switch, six navigation links, private-account toolbar, paper notice, page title, chain selector and large API/control panel appear before the working content. The problem is not simply insufficient screen width: the hierarchy is repeated too many times.

**Change:** use one compact app bar, one persistent paper-mode notice, one page heading and a small contextual action group. Place account management behind the account menu. Put global technical status in System, with specific warnings surfaced where they affect a decision.

### Every session is a large, equally weighted panel

The original session cards repeat state, revision, health, hashes, explanations and four control buttons. Overview and Runs repeat much of this material. Disabled actions create noise alongside the actions that matter.

**Change:** show compact session rows on Overview and Runs. Open a session inspector for the complete state and command surface. A draining session or revision discrepancy is discoverable in the row; full hashes and receipt histories do not dominate the overview.

### Opportunities contains several applications on one page

Captured market records, decision observations, frozen exports, capture audit, manual cost research and grouped observation windows currently form one very long reading sequence.

**Change:** keep the parent name Opportunities, but separate the four tasks. A selected decision can hand off to Cost research without making a form permanently occupy the evidence list.

### Dialogs start with metadata rather than the conclusion

The decision detail's source-continuity section can fill a mobile viewport before the selected result is visible. The captured record dialog forces users to interpret many fields before reaching important limitations.

**Change:** result and limitation first; scope and amount next; technical provenance in a labelled tab. Critical caveats remain next to their values, not hidden in the provenance tab.

## 4. Information architecture

Retain these primary destinations:

| Destination | Primary question | Main content |
|---|---|---|
| Overview | What needs my attention? | Scoped counts, concise session states, unresolved-outcome notice, evidence-readiness summary, latest captured records. |
| Opportunities | What does this observation support? | Captured records, decision evidence, cost research, exports and capture audit. |
| Experiments | What immutable session am I creating? | Validated configuration, network, reference and explicit source-configuration review. |
| Runs | What is the session doing and what is in its ledger? | Session controls, hypothetical balances, journal and reservations. |
| Strategies | Which configuration can I use? | Read-only validated configuration register and immutable details. |
| System | Which part of the data/control chain is working? | API, worker, collection health, adapters and capabilities, independently described. |

Real trading remains a separate, visibly unavailable workspace. Opening it never enables execution or changes an existing session's mode. Do not add a wallet-connect call to action, funding flow or simulated unlock button.

### Opportunities subnavigation

| Tab | Existing functionality to preserve |
|---|---|
| Captured records | Recorded table/card view, route and chain, evidence status, exact quantities, observation time, record inspector, local filters, pagination. |
| Decision evidence | Explicit session and origin scope, stored observation totals, raw decision page, grouped observation windows, detail inspector, page exports and pagination. |
| Cost research | Selected observation context, scenario identity/version/reference, all cost components, native valuation, funding/overhead treatment, review acknowledgement, retained assessment history. |
| Exports & audit | Frozen session preparation/downloads, fixed export identity and timestamp, capture-audit request/import, retained audit results, trust limits, dependency availability. |

The prototype demonstrates the visual pattern for these areas. Production must retain the existing full cost, audit and pagination functionality; the shorter prototype is not permission to delete fields.

### Runs subnavigation

| Tab | Existing functionality to preserve |
|---|---|
| Research sessions | All loaded sessions, chain-view scope, selected session, permitted commands, pending/rejected/uncertain receipts and reconciliation status. |
| Paper ledger | Explicit session and run selectors, retained runs, immutable initial amounts, free/reserved/total quantities by asset, hypothetical-run creation and exports. |
| Journal | Original command and double-entry records, exact asset identities/amounts, dates, paging and scoped exports. |
| Reservations | Original requested principal/native fees, reconciliation state, unresolved amounts, page scope and history. |

A research session, an experiment reference, a hypothetical accounting run, an observation and a captured market record are different entities. Never merge them into an ambiguous generic “run”.

## 5. Visual system

### Tokens

These are the proposal's implemented tokens. They adapt, rather than reproduce mechanically, the Trust Atlas reference.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--canvas` | `#0a0a0a` | `#fafaf8` | Main background |
| `--sidebar` | `#0e0e0e` | `#f2f2ef` | Navigation shell |
| `--soft` | `#121212` | `#f2f2ef` | Main panels |
| `--card` | `#161616` | `#ffffff` | Elevated content |
| `--raised` | `#202020` | `#eaeae5` | Hover/secondary surfaces |
| `--ink` | `#f5f5f5` | `#191a16` | Headings/strong values |
| `--body` | `#c7c7c7` | `#42443d` | Body text |
| `--muted` | `#a0a0a0` | `#64665e` | Secondary labels |
| `--hairline` | `#292929` | `#dedfd7` | Quiet dividers |
| `--strong-line` | `#414141` | `#a2a59a` | Input/control boundaries |
| `--accent` | `#faff69` | `#faff69` | One primary action, selected indicators |
| `--accent-ink` | `#11120b` | `#11120b` | Text on yellow |
| `--accent-wash` | `#222416` | `#f2f3d9` | Selected navigation background |
| `--warning` | `#e7bc7b` | `#845200` | Exceptions, always labelled |
| `--danger` | `#ffaaa3` | `#b02e23` | Destructive operation text |

Yellow replaces the earlier Cobalt direction. It is not a background for every metric. Keep the original accent-budget intent: approximately five percent or less of a normal application viewport. The reference's “one brand colour” is preserved; semantic warning/destructive colours communicate exceptions, not additional branding. No green profitability spectacle, rainbow chain badges, gradients, decorative textures or glows.

A neutral, unfilled state label can communicate a healthy or running state. Successful operations do not need toast notifications or celebration. Use inline, persistent state feedback.

### Typography and geometry

Use the local `Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Arial, sans-serif` stack. Use `"JetBrains Mono", ui-monospace, SFMono-Regular, Consolas, monospace` for exact integers, hashes, identifiers and compact evidence labels. No bundled font binaries or required external font requests are included.

Page heading: 32px desktop, 28px phone; weight 600. Section title: 16px; body baseline 14px. Secondary content is usually 12px. Very small mono labels are metadata, never the only location of a material limitation. On phones, form inputs use 16px, and essential error/decision explanations remain readable without zooming.

Use tabular numerals for comparable amounts. Right-align numeric table columns. Do not use monospace for long explanatory paragraphs. Uppercase is reserved for compact mono labels and badges.

Panel radius is 10px; control radius is 6px. Shadows are unnecessary for ordinary panels. Surface contrast, dividers and spacing establish hierarchy. A 160ms transition is sufficient for hover states; honour reduced-motion settings.

## 6. Desktop composition

The normal desktop sidebar is 224px. It tightens to 196px between 761px and 1200px and expands to 242px above 1600px. The toolbar is 64px high. Main padding is normally 34px horizontally, and content is capped at 1600px.

Overview uses this composition:

```text
┌──────────────────┬───────────────────────────────────────────────────────┐
│ Arbitrage        │ Research / Overview         Search · Preview · Theme │
│                  ├───────────────────────────────────────────────────────┤
│ Paper / Real     │ Paper trading: research/hypothetical scope            │
│                  │                                                       │
│ Overview         │ Paper trading overview       Chain   New experiment  │
│ Opportunities    │                                                       │
│ Experiments      │ Sessions | Attention | Captured | Net after costs    │
│ Runs             │                                                       │
│                  │ ┌ Research sessions ─────────┐ ┌ Evidence readiness ┐ │
│ Strategies       │ │ Exception needing review  │ │ Captured           │ │
│ System           │ │ Compact observed rows     │ │ Simulation         │ │
│                  │ │ Open session for controls │ │ Costs / eligibility│ │
│                  │ └───────────────────────────┘ └────────────────────┘ │
│                  │                                                       │
│ Preview status   │ Latest captured evidence: compact table + Inspect    │
│ Account          │                                                       │
└──────────────────┴───────────────────────────────────────────────────────┘
```

The metrics are scoped summaries, not invented business KPIs. “3 research sessions” means three loaded sessions in the current view. “1 captured record” is a page count, not a census of opportunities. “Needs attention” describes unresolved attempts or a revision discrepancy; it is not a count of profitable routes or a worker-failure score.

Evidence readiness is a list of distinct checks, not a percentage gauge. Captured evidence can exist while simulation fails and costs remain incomplete. Do not turn this ladder into a fabricated universal qualification score.

Overview should not contain the full ledger, audit form, cost form or configuration editor. Use a narrow alert only when it has an actual subject and an inspection destination.

## 7. Mobile behaviour

At 760px and below, hide the fixed sidebar. Use a 60px sticky app bar with brand, theme and search. A fixed bottom bar contains **Overview, Evidence, Runs, More**. More opens the other destinations, workspace selection, account and preview controls. “Evidence” is a short mobile label for the existing Opportunities page; it does not create another data entity.

Bottom navigation is navigation only. Start, stop, create and other state-changing actions do not belong there.

Use 18px page padding, reduced to 13px below 360px. Account for device safe-area insets. Add bottom content padding greater than the navigation height. Core controls have at least 44px touch targets. Essential meaning cannot depend on hover.

Overview uses a two-by-two summary grid followed by the session exception/rows. Wide recorded-evidence tables become compact cards with route, decision, amount, time and Inspect. Other intrinsically tabular technical data may scroll inside its own labelled container; it must not widen the page.

The mobile evidence inspector uses the entire viewport. Its header and close control remain available while its body scrolls; footer controls remain reachable. It is not a desktop modal squeezed into a 350px rectangle with nested scrollbars.

Tabs scroll horizontally when necessary. Selected state and a clear baseline make the active workspace apparent. Keyboard arrows, Home and End work for tabs. A state-critical warning belongs in the active task, not solely in an offscreen tab.

## 8. Inspection and progressive disclosure

### Captured market record

The inspector opens with the evidence verdict: for the supplied fixture, **Rejected — simulation failed**, followed by **Net result is unknown** and the reason that costs are incomplete. This is followed by identity, immutable mode/network, start asset, exact input/output/gross delta, unknown net and observation time.

Three tabs organize the rest:

1. **Summary:** decision, numerical scope and relevant limitation.
2. **Route & costs:** ordered route legs, complete component breakdown, explicit exclusions and eligibility checks.
3. **Provenance:** block/state/capture/provider references, original timestamps, finality and source notes.

Putting provenance in a tab does not remove source type from the summary. “Synthetic”, “captured”, “hypothetical” and applicable staleness remain visible near the result. Full addresses and hashes must be selectable and accessible without hover.

### Decision evidence

Begin with the selected observation's result and original gross quote. Keep the decision identity distinct from the separate captured record. Source continuity has its own tab and independently labelled check time. “No known source invalidation” does not mean current pricing, complete tick coverage, successful simulation or executable profit.

### Session inspector

Show observed state, explicit session scope, health and heartbeat, revision pair, unresolved attempts and immutable configuration. Follow with permitted controls and command receipts. Do not manufacture a pending command from a revision gap alone.

The offline prototype includes labelled buttons for **simulating** acknowledgement/readiness/reconciliation. These are design test controls, not production features, and must never be copied into the connected application.

## 9. Control and evidence invariants

### Commands

Keep the existing `ControlApi`, command types and `availableActions` logic. A confirmed operation creates a request with the correct target, expected revision and idempotency key. An API receipt can remain pending while the last observed worker state remains unchanged.

Preserve sending, pending, applied, rejected and delivery-uncertain states. An uncertain retry reuses the same request identity and payload; it does not create new work. Poll command receipts independently of market-data failures. Preserve retained mutation state during authentication changes and workspace navigation according to the existing application contract.

A stop request does not recall a transmitted transaction. DRAINING remains distinct from STOPPED while outcomes are unresolved. A browser close, logout or page transition must not be described as stopping workers. Separately launched capture processes retain their independent lifecycle.

The chain selector is a **view filter**. It never changes configuration, mode or bulk-stop scope. A bulk-stop confirmation lists the exact loaded targets, including targets outside the current filter. It must not claim to affect unknown sessions outside a paginated loaded set.

### Numbers and provenance

Store exact quantities as strings and use integer/decimal-safe arithmetic. Never round large raw integers through JavaScript `Number`. Do not turn `100000000` into “100 USDC” without a validated decimal scale and asset identity. Where the API supplies only raw minor units, label them as such.

Missing, unknown and explicit zero are different states. A gross quote minus known partial fees is not a complete net estimate. Native costs require explicit unit conversion, ratio, valuation time and applicable age rules. Fees already included in a quote must not be deducted twice.

A manual assumption remains manual after saving. A frozen export is immutable, and downloading it does not refresh it. A source-status check does not mutate the historical observation. A balance is not a return. A hypothetical ledger entry is not an actual fill.

The production honesty labels remain: `PAPER TRADING`, `HYPOTHETICAL`, `CAPTURED DATA ONLY`, `PERSISTED EVIDENCE`, `MANUAL ASSUMPTIONS`, `RECORDED ATTEMPTS`, `FROZEN EXPORT`, `IMMUTABLE`, `STALE / LAST KNOWN`, and `API CONNECTED` when an actual qualifying API exchange supports it.

### Copy discipline

Treat the prototype's shorter headings and organizing descriptions as proposed editorial changes. Preserve the original substantive caution text and scope meaning during implementation. Moving a statement into a disclosure is acceptable only when the summary still exposes its material consequence. Do not bury costs-incomplete, execution-unavailable, stale or unresolved-outcome warnings.

## 10. Forms and unavailable features

Experiments becomes a three-part form: select configuration; define session/reference; review immutable settings. On desktop a right-hand summary follows the form; on mobile it becomes the final review section. Creation does not automatically start a worker. Configuration changes reset the acknowledgement.

The prototype binds its sample network to the selected configuration. In production, preserve the API's actual valid network/configuration combinations; the prototype does not establish a new validation rule. Source-only details must be explicitly identified as not exposed by the API instead of invented in the summary.

Cost research places inputs beside a persistent result summary. Each component distinguishes missing, explicit zero and known manual amount. Invalid integer input cannot produce a numerical result. Preserve all production scenario identifiers, versions, timestamps, valuation and overhead fields: the simplified fixture's six already-valued inputs are not a replacement for that pipeline.

Sign-in uses the same visual system but a quieter, narrower form. Preserve the existing account setup, password and recovery semantics. Do not add public registration or imply that clicking help sends email. The offline prototype disables credential inputs and provides an explicitly labelled preview entry instead.

Strategies and System are not embellished with unsupported uptime, throughput or vendor coverage. Unsupported capabilities are distinguished from errors and empty data. Real trading remains disabled with an explanation, not an enticing mock trading screen.

## 11. Production integration plan

Implement within the existing **React 19 + Vite + TypeScript** frontend. Do not replace it with this standalone JavaScript prototype, change frameworks, or rebuild the Rust/API layer.

| Existing file / component | Proposed presentation work |
|---|---|
| `src/tokens.css` | Introduce the neutral/yellow dark and light tokens, retaining semantic meanings. |
| `src/styles.css` | Shell, responsive navigation, compact rows, tabs, drawers, forms, tables and focus styles. |
| `src/ConnectedApp.tsx` | Reshape the shell and Overview; preserve authentication, snapshots, command state and subscriptions. |
| `components/RecordedOpportunities.tsx` | Table/card presentations plus the record inspector. |
| `components/DecisionExplorer.tsx` | Present subworkspaces without discarding selected scope, async work, audit results or retained forms. |
| `components/PaperWorkspace.tsx` | Separate ledger, journal and reservations while preserving original run/session state. |
| `components/CollectionHealth.tsx` | Compact health summaries with complete diagnostics in context. |
| `components/AdapterSupport.tsx` | Read-only capabilities and adapter availability. |
| `components/AccountAccess.tsx` | Sign-in and account layout only; keep authentication/recovery behaviour. |
| `src/api/*` | Preserve existing contracts; no visual redesign reason to rewrite the client. |

Do not implement against `DemoApp.tsx`, `ResearchViews.tsx`, `OpportunityTable.tsx` or `OpportunityDialog.tsx`; the supplied README identifies those as non-production paths.

The application currently uses page state rather than a router. A routing rewrite is not necessary. A shareable location can be added only after retained-state behaviour is verified. Keep subworkspaces mounted, or lift their state before conditionally unmounting them. A tab switch must not lose uncertain request keys, unfinished assessments, loaded export identities or journal selection.

Continue capability gating through `/v1/capabilities`: `decision_history`, `paper_ledger`, `paper_run_creation`, `collection_telemetry`, `session_export`, `cost_assessments`, `adapter_support`. A navigation link is not authority to invoke an operation.

Retain the documented five-second polling and stale-after-fifteen-second behaviour unless changed in a separate reviewed requirement. Keep API freshness, historical observation time and worker heartbeat separate. Do not use a single green “Live” indicator as a replacement.

### Suggested delivery sequence

1. **Shell and tokens:** dark/light, desktop/mobile, account menu; no data-flow changes.
2. **Overview and session inspection:** compact summaries and preserved control semantics.
3. **Evidence inspector:** summary-first, route/cost and provenance tabs.
4. **Opportunities and Runs workspaces:** split the long pages without unmounting important state.
5. **Forms, configuration, System and account polish:** preserve full validation/capability coverage.
6. **Regression review:** screenshot matrix, keyboard/touch, API errors, receipt transitions, exports and authentication.

Review each increment as a presentation change. Do not couple this work to credential changes, Railway settings, trading activation or a new backend feature.

## 12. Accessibility and state acceptance

Use semantic navigation and headings, a skip link, explicit form labels and a visible focus ring. Focus the page heading after navigation. Modal dialogs require a descriptive title, focus containment, Escape close and focus restoration. Tabs require keyboard navigation and an associated panel.

Errors and command progress need polite or urgent announcements appropriate to their severity. Avoid announcing the whole interface on each poll. Colour is never the only indicator. Respect reduced motion and prevent fixed controls from covering the final content.

Production acceptance must cover:

- Dark and light at 320, 390, 768, 1024 and 1440px, including long IDs, empty lists and long translations where applicable.
- No page-level horizontal overflow; permitted table scrolling is local.
- Empty, loading, stale, partial, unsupported, error and retained-last-known views.
- Sending, pending, applied, rejected and delivery-uncertain commands without false state changes.
- Bulk control under filters and pagination, including targets outside the view.
- Draining sessions that remain unresolved after a stop request.
- Exact very-large integers, invalid fractional minor units and missing-versus-zero costs.
- Frozen export identity, honest download scope and no silent refresh.
- Form acknowledgement reset, immutable session configuration and no automatic start.
- Authentication revocation without losing the retained uncertain-mutation context.
- Native-dialog keyboard behaviour, phone safe areas and touch targets.

The included `qa/results.json` records the tests actually run against this prototype. It is not production acceptance, formal accessibility certification, a Safari/iOS device test or an integration test of the control API.

## 13. Prototype boundaries

The prototype has working navigation, theme switching, local search/filtering, tabbed inspection, local session-state demonstrations, form gating, exact integer scenario calculation, and synthetic JSON/CSV downloads.

It does **not** implement live market data, account login, production polling, backend session creation, complete paper-run creation, the full native-valuation form, real capture-audit import/verification, full export manifests, automatic settlement or real execution. Some secondary research tabs use fixed layout fixtures when the preview-state selector changes; that limitation is explained inside the selector.

Only the theme preference is persisted locally. Other interactions live in page memory and reset on reload. No package-provided font files, third-party assets, analytics or external requests are required.

## 14. Coding-agent brief

> Redesign the existing `apps/web` production interface using this Atlas design proposal and the supplied screenshots. Keep React/Vite, the existing API client, capability gates, exact numeric types, immutable session semantics, authentication and asynchronous command receipts. Treat the HTML as a visual/interaction reference, never as a backend replacement. Implement the shell and Overview first, then session/evidence inspectors, then the Opportunities and Runs tabs. Retain subworkspace state across navigation. Preserve substantive warnings and unknown values. Do not copy fixture data, simulated worker acknowledgements or preview authentication into production. Add regression tests for pending versus applied commands, draining outcomes, view-filter versus command scope, exact quantities, frozen exports and mobile overflow. Do not modify Railway, credentials, wallets or execution permissions as part of this redesign. Report precisely which files changed and which tests passed.
