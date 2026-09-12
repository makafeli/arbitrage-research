# Getting started

Clone [makafeli/arbitrage-research](https://github.com/makafeli/arbitrage-research) and read the root README for executable commands and pinned prerequisites. [Implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) distinguishes current source from the full product; [integration verification](https://github.com/makafeli/arbitrage-research/blob/main/docs/17-RESEARCH-INTEGRATION-VERIFICATION.md) records checks and remaining acceptance.

```sh
git clone https://github.com/makafeli/arbitrage-research.git
cd arbitrage-research
```

## Review the dashboard

The original design is `design/dashboard-wireframe.html`. The React/TypeScript application is in `apps/web`; use its README for development and package checks. Demo mode uses visibly synthetic fixtures and local command transitions. Connected mode authenticates to the control API and reads actual stored sessions, receipts, decisions, coverage and virtual accounts. A stored record can itself have synthetic provenance; connection to the API does not turn it into market evidence.

The chain selector filters the view and does not change worker scope. No trading wallet is needed for either research interface. Follow [Using research and paper accounts](./Using-Research-and-Paper-Accounts.md) for the current controls and evidence limits.

## Work on the Rust implementation

Use the pinned toolchain and Cargo workspace at the repository root. Baseline checks are:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked -p control-api
export ARB_TEST_CONTROL_API_BIN="$(pwd)/target/debug/control-api"
cargo test --locked --workspace
python scripts/validate_project.py
```

The full Rust test suite requires a disposable PostgreSQL database configured through `TEST_DATABASE_URL`, plus the built API executable path in `ARB_TEST_CONTROL_API_BIN`; follow the root README and CI setup. These commands describe how to validate a checkout, not evidence that its exact commit passed. Rust 1.90 CI, database behavior and browser/container checks must be recorded against that commit. Earlier foundation test counts do not verify the current integration.

Research examples ship with disabled networks and empty allowlists. They are inert templates, not verified market identities. A `fixture:` address cannot become a production allowlist entry. Before enabling capture, independently qualify actual registry identities, provider behavior and supported pool state, then freeze the matching configuration and registry digests.

## Prepare the selected host

Railway is the selected host. The prepared definition contains a public web origin, private API and PostgreSQL; separately qualified research workers need their own persistent capture volumes and no public ports. It has not been deployed by this implementation. Use the [Railway runbook](https://github.com/makafeli/arbitrage-research/blob/main/deploy/RAILWAY.md) to review the project/environment, variables, volumes and service plan. A healthy API with the shipped disabled configurations correctly rejects market sessions.

## Choose the next issue

Use the [structured backlog](https://github.com/makafeli/arbitrage-research/blob/main/planning/backlog.json) and published issue. Source now connects capture, exact quote math, durable controls, virtual accounts and the UI; acceptance still depends on the relevant integrated tests and qualification evidence. Provider/deployed-pool qualification, complete atomic-route simulation, automatic paper scenarios and a comparable observation campaign remain separate work.

Local development needs no trading key or funded wallet. Read-only provider credentials, when required, belong in the configured environment-secret mechanism and must stay out of commits, logs and exports. The research applications expose no signing or transaction-broadcast capability.

## Orient yourself

| Location | What to inspect |
|---|---|
| `apps/web` | Explicit Demo/Connected UI, session controls, decision inspection and virtual accounts |
| `apps/research-worker` | Controlled OBSERVE/PAPER capture and candidate evaluation |
| `apps/evm-worker`, `apps/solana-worker`, `apps/replay` | Standalone capture tools and offline verification/evaluation |
| `crates` | Exact domain/math, capture, registry, scheduler, engine, ledger, storage and control boundaries |
| `docs` and `specs` | Canonical requirements, current verification and API/data contracts |
| `config` | Inert examples and explicit capability defaults |
| `deploy` | Container definitions and Railway preparation |
| `planning` and `wiki` | Stable work definitions, publication metadata and onboarding source |

See [development workflow](./Development-Workflow.md) for acceptance, review and evidence requirements.
