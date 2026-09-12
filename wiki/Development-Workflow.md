# Development workflow

Use small reviewable changes tied to the [structured backlog](https://github.com/makafeli/arbitrage-research/blob/main/planning/backlog.json). A ticket specifies the problem, scope, dependencies, role, acceptance and evidence. The [implementation handoff](https://github.com/makafeli/arbitrage-research/blob/main/docs/10-BUILD-HANDOFF.md) defines team responsibilities and the first integrated slice.

## Work-item lifecycle

A stable ticket ID survives import and project migration. GitHub issue numbers and project item IDs are remote publication metadata. Keep dependencies in the canonical backlog; reference the corresponding issue links after publication. Epics M0–M7 group required outcomes, while individual tickets remain the unit of implementation acceptance.

Select unblocked work, confirm its dependencies have actual evidence, create a branch and update the issue as implementation progresses. If a spike disproves an assumption, record the result and adjust dependent scope. A legitimate “unsupported” result is better than silently widening a capability claim.

The initial code scaffold does not automatically close implementation tickets. Durable commands need restart/idempotency evidence; adapters need protocol fixtures; paper needs complete accounting/simulation gates; UI needs rendered interaction/accessibility evidence.

## Pull requests

Lead with the concrete problem and resulting behavior. Include the stable ticket and PRD IDs, changed capability boundary, representative input fixtures, checks actually run and remaining limitations. Preserve literal data units and identifiers. Avoid claiming performance improvements without a named workload, build and comparable measurements.

Review domain arithmetic and state/control changes separately from visual changes where that improves clarity. Require independent review for later signing/execution policy and fund-flow code. Another generated review is useful feedback, not an independent security audit.

## Validation

Rust checks cover formatting, linting and meaningful unit/integration behavior. Protocol math uses golden, boundary and differential fixtures from pinned sources. Replay tests inject time/state and never fetch current prices to fill old gaps. Fault tests exercise unknown dispatch, restart, stop races, stale revisions and invalid state.

Frontend checks include type/build validation and behavior appropriate to the change. Production UI acceptance also requires a real browser at narrow/wide widths, keyboard use, zoom, theme contrast, stale/disconnected states and correct command acknowledgements. A source assertion cannot establish visual layout.

Report not-run checks honestly. Document toolchain/environment constraints and use CI to provide missing evidence before accepting the ticket. Passing a mock test does not satisfy an integrated worker, chain or security gate.

## Documentation ownership

| Source | Owns |
|---|---|
| `docs/01-PRD.md` | Product behavior and acceptance requirements |
| `docs/02-ARCHITECTURE.md` and `03-REPOSITORY-STRUCTURE.md` | Target boundaries and source organization |
| `docs/04-TRADING-AND-PAPER-MODEL.md` | Financial/evidence and experiment methodology |
| `docs/05-UX-DESIGN.md` | Approved experience and browser acceptance |
| `docs/06-SECURITY-OPERATIONS-AND-TESTING.md` | Operational controls, safety and release evidence |
| `docs/07-DELIVERY-PLAN.md` and `planning` | Milestones, stable tickets, dependencies and delivery tracking |
| `docs/08-DATA-AND-API-CONTRACTS.md` and `specs` | Versioned data/process contracts |
| `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md` | Decisions, rationale and revisit triggers |
| `docs/12-IMPLEMENTATION-STATUS.md` | Source-present versus implemented/verified capability boundary |
| `wiki` | Concise navigation and onboarding summaries |

When behavior changes, update its canonical source and relevant Wiki summary in the same pull request. Record actual repository/issue/Project/Wiki publication separately from generated files. Preparing an import script is not proof that a native GitHub feature exists.
