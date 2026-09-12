# Getting started

Clone [makafeli/arbitrage-research](https://github.com/makafeli/arbitrage-research) and read the root README for the current executable commands and pinned prerequisites. The [implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) distinguishes current source from the full planned product.

```sh
git clone https://github.com/makafeli/arbitrage-research.git
cd arbitrage-research
```

## Review the dashboard

The original design is `design/dashboard-wireframe.html`, a self-contained local reference. The React/TypeScript implementation is in `apps/web`. Start the development application using that package's documented script. Run the relevant package checks before changing the interface.

The six views use explicitly synthetic examples. A demonstration start/pause/stop transition operates only on the local demo state. It is not a command to a blockchain worker, durable acknowledgement, balance change or trading result. The chain filter changes the view, not worker scope. No wallet is required to review the design.

## Work on the Rust foundation

Use the toolchain and Cargo workspace declared at the repository root. The baseline Rust checks are:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

These are instructions for a development environment with Rust installed, not a statement that they ran during scaffold generation. A check result belongs in CI or the validation record. If lockfiles or toolchain components need resolution, follow the README and first-build ticket rather than assuming the initial handoff is a reproducibly built release.

Research configuration examples have networks disabled and empty allowlists. They provide a fail-closed starting point, not verified market identities. A `fixture:` address is synthetic and cannot become a production allowlist entry.

## Choose the next issue

Use the [structured backlog](https://github.com/makafeli/arbitrage-research/blob/main/planning/backlog.json) and the corresponding published issue. Begin with unblocked qualification/foundation work, then complete one integrated observation slice. Adapter qualification, durable control storage and real UI integration remain explicit work items even when related scaffolding exists.

Local development needs no production trading key or funded wallet. Provider credentials become necessary only when implementing a selected read/simulate integration; use the configured secret mechanism and keep them out of commits, logs and exports. Live execution belongs to later milestones and cannot be activated through the demo.

## Orient yourself

| Location | What to inspect |
|---|---|
| `apps/web` | React shell, screens, fixtures and local interaction model |
| `apps` and `crates` | Rust applications and shared types; exact implemented scope varies by milestone |
| `docs` | Canonical PRD, architecture, economics, UX, operations and handoff |
| `specs` | Proposed HTTP and opportunity contracts; implementation may cover a subset |
| `config` | Inert research examples and explicit capability defaults |
| `planning` | Stable work-item definitions and publication metadata |
| `wiki` | These navigational pages and their source |

See [development workflow](./Development-Workflow.md) for acceptance, branch/review practice and evidence requirements.
