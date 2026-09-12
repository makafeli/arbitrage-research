# UX and interface specification

Status: v0.3 approved design and TARGET product behavior. The selected `design/dashboard-wireframe.html` remains the visual reference. `apps/web` now contains explicit Demo and Connected modes: Demo uses labeled examples, while Connected authenticates to durable controls, decision/coverage queries and virtual-account endpoints. Browser and API validation for the current source is tracked in [integration verification](17-RESEARCH-INTEGRATION-VERIFICATION.md); pending checks are not claimed as completed. The full comparison, simulation and live behaviors below remain gated requirements.

## Current connected experience

Connected mode presents raw decision pages, grouped observations, coverage, trace inspection, PAPER account balances/journal/reservations and bounded JSON export of selected received data. Unknown net amounts and missing execution denominators stay visibly unavailable. Page counts are not full-history or collection coverage. A PAPER account is initialized only on a stopped PAPER session; an unresolved create request retains its original idempotency key for retry. Session command receipt remains separate from worker acknowledgement. See the [operating guide](../wiki/Using-Research-and-Paper-Accounts.md) for the current flow.

## 1. Product experience

The interface should help the operator answer three questions: which observations deserve investigation, which strategies withstand realistic execution assumptions, and whether the system is operating as intended. Financial totals are useful only when the interface explains their evidence, costs, period and uncertainty.

The first usable version prioritizes comparison and investigation. A single overview should communicate the current mode, run state, chains being observed, freshness and outstanding work. Selecting an opportunity should reveal its route, inputs, estimated costs and reason for acceptance or rejection. Starting a research run should not require configuring a wallet. Phase-one adapters cover Uniswap V3 on Base and Orca Whirlpools on Solana; additional venues remain capability-gated extensions.

Use plain operational language. Avoid “winning strategy,” “guaranteed return,” performance scores without definitions, or annualized returns extrapolated from a short run. A chain with more observations is not automatically the better strategy. Comparisons must disclose unequal data coverage and different operating assumptions.

## 2. Information architecture

The primary navigation has six destinations. Keep the selected workspace, mode and system state visible across all six.

| Destination | Primary question | Essential content |
|---|---|---|
| Overview | What is happening, and what needs attention? | Run status; chain coverage; observation funnel; unresolved outcomes; most recent opportunities. |
| Opportunities | Why did this route qualify or fail? | Filterable observations; evidence stage; route detail; costs; rejection reasons; timestamps. |
| Experiments | Which assumptions or configurations perform better? | Comparable run cohorts; configuration differences; common observation window; outcome distributions; coverage gaps. |
| Runs | What did the worker do? | Run lifecycle; immutable configuration reference; start/end times; checkpoints; pending and reconciled outcomes. |
| Strategies | What is allowed to run? | Versioned strategy configurations; chain and venue scope; token allowlist; sizing; limits; paper/live eligibility. |
| System | Can we trust these observations? | Feed status; block/slot lag; reconnects; decoding errors; clock health; service health; signer status when applicable. |

Global chain filters affect the research content and retain their selection during navigation. A filter changes the view; it does not change which workers run. Worker scope is configured explicitly in Runs or Strategies. Show this distinction next to the filter and in the start preview.

## 3. Evidence vocabulary

Every opportunity and aggregate must identify its evidence stage. These stages describe strength of evidence, not a guaranteed progression to execution.

| Label | Meaning | Required supporting detail |
|---|---|---|
| Candidate | A quote or route calculation identifies an observation for evaluation; no complete transaction simulation is established. | Source state, time, input amount and screening version. |
| Simulated | The complete intended atomic transaction successfully simulated against identified state. Economic outcome may still be negative. | Simulator version, state reference, cost treatment, result and limitations. |
| Estimated executable | A fully simulated route also passes fresh coherent state, funding, fee and guard checks under a named delay/inclusion scenario. Inclusion remains uncertain. | Freshness, limits, liquidity, submission assumptions and estimated net range. |
| Realized | A submitted trade has a reconciled on-chain outcome meeting the configured confirmation policy. | Transaction reference, actual received amounts, fees, reconciliation state and confirmation policy. |

Use separate fields for processing state, evidence stage and result. Failed complete transaction simulations retain the Candidate evidence stage with a simulation-failed result. A rejected candidate is not a failed transaction. A successful blockchain transaction is not automatically a profitable arbitrage. “Pending,” “expired,” “reverted,” “unresolved” and “no trade” must remain distinguishable.

Only reconciled live outcomes populate realized trade totals. Paper quote-only results remain candidates; only complete successful atomic transaction simulations receive the Simulated label. Paper outcomes never populate realized results. Trade-level net results and operating profit have different cost scope; infrastructure, unallocated failed attempts and other overhead appear separately. Missing costs show “Unknown” or “Excluded,” never zero.

## 4. Overview and comparison

Use a restrained application shell, a concise status strip and two peer chain panels. Solana and Base receive equal visual weight. Each panel shows observation window, valid observation coverage, freshness at capture, evaluated routes and eligibility counts. Provide data-quality text such as “Partial coverage: reconnect gap” next to the relevant metric.

The observation funnel can show candidate count, simulations completed and estimated executable results for a defined cohort. Stages must use the same denominator and period. Do not sum sequential stages into a total. Realized outcomes belong in a distinct live section; in a paper-only run, show “Not applicable — paper mode.”

Use one primary action, “Start paper run,” with secondary pause and stop controls becoming available when appropriate. Include a persistent evidence label and time scope above financial information. The prototype uses no chart of fabricated historical profits: a few labeled values and inspectable examples explain the design more honestly.

## 5. Opportunity table and detail

Desktop column priority: route and chain; evidence; input size; estimated net range; observation time or age; quality; detail action. Include the denomination in amount headers. Additional fields—pool addresses, block hashes, gas units and account lists—belong in details or an export.

An opportunity detail panel shows the route in order, state references, input amount, gross estimate, individually named costs, resulting range and eligibility explanation. If a quote already incorporates pool fees or price impact, say so and avoid subtracting them again. Display scenario assumptions separately from measured facts. A rejected route should lead with the rejection reason and show the failed condition.

Use “Inspect” for opening details; avoid a trade button in paper mode. Detail links to a run, strategy version and comparison cohort become available in the real product. Addresses are selectable and copyable, with full values available without hover. A transaction explorer link appears only when an actual transaction reference exists.

## 6. Experiment flow

An operator selects chain/venue coverage, token allowlists, a defined observation period and a baseline configuration. The product summarizes the scope before creating the run. Each experiment stores an immutable configuration version and assumption set.

Compare cohorts over a shared period wherever possible. Show sample size, coverage and exposure alongside results. Present observed spread, simulated net outcome and eligibility rate as separate measures. Explain that transaction inclusion cannot be inferred merely from a profitable quote. Make latency and fee assumptions editable through new experiment versions, preserving earlier results.

No single score should declare a chain the winner. The decision view should expose tradeoffs: opportunities surviving stricter assumptions, size capacity, data quality, operating costs and outcome uncertainty. Export the assumptions with results so a downloaded figure remains interpretable.

## 7. Run controls and recovery

The start flow creates a stopped session with a specific immutable configuration snapshot, then sends an explicit START command. The UI may compose these two steps. The label identifies paper or live mode. Pausing closes new evaluation and submission gates; data observation and reconciliation continue. Resuming revalidates state and evaluates fresh opportunities rather than submitting an old queue. Stopping closes admission, drains or cancels unsubmitted work, persists a checkpoint and continues necessary reconciliation before final closure.

The interface must distinguish “Stop requested,” “Stopping — outcomes pending” and “Stopped.” An accepted stop command is pending until the worker acknowledges its admission fence as applied; DRAINING continues if submitted outcomes remain unresolved. Modes OBSERVE, PAPER, REPLAY and LIVE are immutable per session. A stop request cannot recall a transaction already submitted to a network. Show pending identifiers and last checked time; permit the operator to inspect them while reconciliation continues. Process failure or an unknown outcome is not a successful stop acknowledgment.

The emergency control blocks further signing/submission and reports the independently confirmed control state. In-flight transactions retain their normal reconciliation path. Live controls require the product's explicit live enablement flow and configured permissions; changing a global filter or opening a page cannot enable live trading.

The demo reproduces Start, Pause, Resume and Stop as local interface states only. Its Runs screen includes a separate synthetic pending-submission scenario to demonstrate why stopping admissions and resolving an existing transaction are different events.

## 8. Visual and interaction system

Use a quiet dark default with a full light theme. Neutral surfaces, fine dividers and generous spacing support dense information without a terminal-like wall of text. Reserve the accent for the selected location or primary action. Pair every status color with a text label.

Use a system font stack, consistent 4/8-pixel spacing steps, approximately 14–16-pixel body text, clear section hierarchy and tabular numerals for values. Financial signs are explicit. Avoid animated profit counters, decorative speed claims and continuously moving tickers. Dates include timezone; relative ages have an accessible absolute time.

Controls provide immediate acknowledgment, then show the resulting server-confirmed state. Disable invalid transitions with a visible explanation when needed. Empty, loading, partial, stale and unavailable states must be designed separately. Preserve the last reliable observation and its timestamp during a feed outage; never make cached values look current.

## 9. Accessibility and mobile

Target WCAG 2.2 AA. Use semantic landmarks, associated form labels, real buttons, visible keyboard focus and a skip link. Status changes use a polite live region without announcing every market update. Dialogs have an accessible title, keyboard containment, Escape dismissal and focus restoration. Important information is accessible without hover or color discrimination.

At narrow widths, navigation becomes a wrapping compact row; chain panels stack. The opportunity list becomes cards showing route, chain, evidence, net estimate and quality, with the full record available in details. Controls wrap without clipping. Maintain approximately 44-pixel touch targets and support browser text enlargement. Do not shrink financial text to fit extra columns.

Honor reduced-motion preferences. Theme selection must preserve text contrast, form visibility and focus. Production testing should include 320-, 390-, 768- and 1440-pixel widths, keyboard-only operation, screen-reader labels and 200% zoom.

## 10. Prototype scope and acceptance

`design/dashboard-wireframe.html` is self-contained and makes no network requests. All numbers, routes, times, histories and outcomes are synthetic examples. No wallet connection, private-key input, signing, transaction submission or real market observation is implemented.

Reviewers can navigate all six screens, filter by chain, inspect opportunities, switch theme and exercise illustrative run controls. A persistent synthetic-data label remains visible, and individual research panels identify their examples. Acceptance requires working keyboard interactions, correct chain filtering, consistent labels, no false realized returns, no external dependencies and clear pending-outcome behavior. This prototype is the approved design reference. The React implementation carries that design into separate Demo and Connected components. Connected mode uses authenticated backend records and real worker command receipts; full browser/accessibility evidence belongs to the current CI checkpoint. Local demonstration transitions must never be presented as a durable worker result.


## 11. Implementation and review contract

Maintain the reference design: muted navy surfaces, lime primary action, dark and light themes, six primary destinations, equal Solana/Base panels, clear evidence labels and inspectable cost detail. Keep the synthetic-data badge visible in the demo. Any change to visual tokens, primary navigation or control placement is reviewed against this reference and documented in the ticket.

The React implementation has an explicit data-source boundary: Demo components use labeled local fixtures, while Connected components consume validated API responses and display connectivity and command status. Preserve that separation as the product grows. A frontend-only timer can illustrate pending/application behavior but cannot satisfy the durable acknowledgement requirement. The chain filter changes the displayed content only.

Production acceptance combines source checks with a rendered browser review at 320, 390, 768 and 1440 pixels, keyboard navigation, 200% zoom, theme contrast and dialog focus restoration. Passing a JavaScript syntax check or a mocked interaction test does not establish those visual/accessibility results. Attach browser evidence before closing the corresponding UI tickets.
