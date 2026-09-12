# Arbitrage Research

A Rust-first research platform for investigating same-chain arbitrage on Base and Solana, with an approved React dashboard design and a staged path from observation to paper experiments. It is intended for one private operator. The GitHub repository is the project delivery source; application access and repository visibility are separate settings.

Current source includes bounded read-only capture, exact two-pool quote research, retained-input replay, PostgreSQL controls and virtual accounts, and a connected dashboard beside the synthetic demo. It does not submit transactions, automatically settle paper trades or establish market profitability. Shipped network configurations remain disabled until their actual providers and pool identities are qualified. Read [implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) for scope and [integration verification](https://github.com/makafeli/arbitrage-research/blob/main/docs/17-RESEARCH-INTEGRATION-VERIFICATION.md) for the current evidence and pending CI.

## Start here

- [Getting started](./Getting-Started.md): repository, development commands and deployment preparation.
- [Using research and paper accounts](./Using-Research-and-Paper-Accounts.md): actual session controls, candidate evidence, virtual capital and exports.
- [Product and roadmap](./Product-and-Roadmap.md): outcomes, scope, milestones and work tracking.
- [Architecture](./Architecture.md): Rust boundaries, data flow and durable control design.
- [Dashboard and UX](./Dashboard-and-UX.md): approved design, screens, evidence and controls.
- [Paper trading and evidence](./Paper-Trading-and-Evidence.md): what each result establishes and how comparison works.
- [Operations and safety](./Operations-and-Safety.md): startup/stop/recovery and future live gates.
- [Development workflow](./Development-Workflow.md): tickets, reviews, validation and documentation ownership.

## Canonical project sources

| Source | Purpose |
|---|---|
| [Repository README](https://github.com/makafeli/arbitrage-research/blob/main/README.md) | Current runnable entry points and project orientation |
| [Product requirements](https://github.com/makafeli/arbitrage-research/blob/main/docs/01-PRD.md) | Functional and nonfunctional acceptance |
| [Implementation handoff](https://github.com/makafeli/arbitrage-research/blob/main/docs/10-BUILD-HANDOFF.md) | Team responsibilities and first integrated slice |
| [Structured backlog](https://github.com/makafeli/arbitrage-research/blob/main/planning/backlog.json) | Stable epic/ticket IDs, dependencies, scope and acceptance |
| [Issues](https://github.com/makafeli/arbitrage-research/issues) | Published work items and evidence links |
| [Integration verification](https://github.com/makafeli/arbitrage-research/blob/main/docs/17-RESEARCH-INTEGRATION-VERIFICATION.md) | Current test evidence, review corrections and remaining acceptance |
| [Railway runbook](https://github.com/makafeli/arbitrage-research/blob/main/deploy/RAILWAY.md) | Selected hosting layout and preparation; no deployment is claimed |
| [Dashboard reference](https://github.com/makafeli/arbitrage-research/blob/main/design/dashboard-wireframe.html) | Original approved visual/interaction reference |

## Documentation conventions

Detailed requirements in `docs` are authoritative. These Wiki pages summarize and navigate them; if a summary is ambiguous, follow the linked requirement and correct the summary. Financial examples are synthetic unless explicitly backed by a captured run and provenance. A planned feature is not a completed feature.

Wiki source lives in the repository's `wiki` directory. Sibling `.md` links work while browsing that source. Publishing to native GitHub Wiki converts those links to Wiki page routes; canonical document links already use absolute repository URLs. Source preparation and native Wiki publication are separate actions. The setup record states what actually published and any unavailable GitHub features.

## Publication status

[Verified repository setup and remaining native Project/Wiki steps](https://github.com/makafeli/arbitrage-research/blob/main/docs/13-GITHUB-SETUP-STATUS.md). All published issue links are in [the GitHub issue index](https://github.com/makafeli/arbitrage-research/blob/main/planning/GITHUB-ISSUES.md).
