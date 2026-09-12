# Repository structure

Status: parallel implementation on 12 September 2026. The repository includes Rust domain/configuration, storage/control, capture/adapters, scheduler/paper primitives, API/observation worker/replay applications, a connected React dashboard and Railway deployment definitions. The comprehensive tree below is the target structure, including future components. It is not a claim that every path or service exists. See [implementation status](12-IMPLEMENTATION-STATUS.md) for what the current scaffold contains.

## 1. Target monorepo

```text
arbitrage-research/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  README.md
  SECURITY.md
  CONTRIBUTING.md
  deny.toml
  rustfmt.toml
  .env.example
  .gitignore
  apps/
    control-api/
      Cargo.toml
      src/{main.rs,auth.rs,routes.rs,events.rs}
      tests/
    evm-worker/
      Cargo.toml
      src/{main.rs,bootstrap.rs}
      tests/
    solana-worker/
      Cargo.toml
      src/{main.rs,bootstrap.rs}
      tests/
    signer/
      Cargo.toml
      src/{main.rs,policy.rs,keystore.rs,server.rs}
      tests/
    replay/
      Cargo.toml
      src/{main.rs,manifest.rs,report.rs}
      tests/
    web/
      package.json
      package-lock.json
      src/
        app/
        features/{overview,opportunities,experiments,runs,strategies,system}/
        components/
        api/generated/
        styles/
      tests/{unit,e2e}/
  crates/
    arb-domain/
    arb-adapter-api/
    arb-engine/
    arb-risk/
    arb-sim/
    arb-execution/
    arb-storage/
    arb-control-protocol/
    arb-signing-api/
    arb-evm/
    arb-solana/
    arb-telemetry/
  contracts/
    evm/
      foundry.toml
      src/{ArbitrageExecutor.sol,adapters,interfaces,libraries}/
      test/{unit,fuzz,invariant,fork}/
      script/
      deployments/
  programs/
    solana/
      README.md
  migrations/
  config/
    defaults.toml
    chains/{base.toml,solana.toml}
    strategies/
    scenarios/
  specs/
    openapi.yaml
    events.schema.json
    configuration.schema.json
    signer-protocol.md
  fixtures/
    manifests/
    evm/
    solana/
    replays/
  tests/
    system/
      Cargo.toml
      src/lib.rs
      tests/
    fault-injection/
  benchmarks/
    manifests/
    reports/
  deploy/
    compose.yaml
    containers/
    monitoring/
    runbooks/
  scripts/
    generate-api-client.sh
    verify-fixtures.sh
  docs/
    01-PRD.md
    02-ARCHITECTURE.md
    ...
    12-IMPLEMENTATION-STATUS.md
  design/
    dashboard-wireframe.html
  planning/
    backlog.json
  wiki/
    Home.md
    _Sidebar.md
    ...
  .github/workflows/
    ci.yaml
    contracts.yaml
    release.yaml
```

Brace notation abbreviates siblings; it does not denote literal filenames. `apps/web` is the sole JavaScript package initially and uses npm with `package-lock.json`. The root Cargo workspace currently contains the smaller foundation listed in [implementation status](12-IMPLEMENTATION-STATUS.md); the following inclusion rule applies to the eventual target. The root Cargo workspace includes all Rust applications, shared crates and `tests/system`. A future Solana program may need an isolated pinned build workspace; that decision follows the execution spike. Until then, `programs/solana/README.md` records the capability gate and no placeholder program is deployed.

The `planning` directory owns stable epic/ticket IDs and import metadata. GitHub issue numbers are remote identities recorded by the setup workflow; they do not replace stable ticket IDs. `wiki` contains concise onboarding/navigation source, while detailed requirements remain canonical in `docs`. A Wiki publisher must rewrite sibling `.md` links for native GitHub Wiki routes.

Each shared crate follows `Cargo.toml`, `src/lib.rs`, focused modules, `tests/` for integration tests and `benches/` where a measured performance question exists. Avoid splitting every adapter into a crate immediately; keep protocol modules together until build isolation or ownership warrants a split.

## 2. Crate responsibilities

| Crate | Responsibility and boundary |
|---|---|
| `arb-domain` | Chain and asset IDs, checked amounts, route and plan types, provenance, costs and outcome vocabulary. No network, database, clock or Tokio runtime. |
| `arb-adapter-api` | Interfaces for state ingestion, exact quoting, transaction preparation and reconciliation; explicit capability declarations. |
| `arb-engine` | Route indexing, sizing orchestration, invalidation, evaluation scheduling and deterministic candidate selection. |
| `arb-risk` | Eligibility, nonconflicting principal/fee reservations, actual spending-account funding and authority, exposure limits and authorization checks; receives facts and policy rather than fetching them. |
| `arb-sim` | Virtual inventory, delayed fills, failure scenarios, seeded replay and model confidence. Cannot request a signature. |
| `arb-execution` | Worker lifecycle, intent transitions, journal coordination, submission fence, recovery and transport orchestration. |
| `arb-storage` | PostgreSQL repositories, migrations integration, journal transactions, durable command access and pre-call dispatch-start records defaulting to `UNKNOWN`. |
| `arb-control-protocol` | Versioned commands/status/events; immutable `OBSERVE`, `PAPER`, `REPLAY`, `LIVE` modes; revisions, acknowledgements and lifecycle states including `PAUSED`. |
| `arb-signing-api` | Versioned signer messages, approved plan schema and client transport; contains no private-key implementation. |
| `arb-evm` | Alloy integration, block tracking, venue math, ABI encoding, gas estimates, nonces and receipts. |
| `arb-solana` | Account ingestion, supported pool math, instruction encoding, account constraints, fees, blockhash validity and confirmation. |
| `arb-telemetry` | Structured traces and metrics helpers with mandatory redaction. |

The chain adapters expose different capabilities instead of pretending every venue supports the same pool model or flash-loan flow. Each venue module provides a decoder, exact quote implementation, transaction builder, fixture manifest and readiness declaration. Unsupported token extensions and unusual transfer behavior are rejected unless explicitly implemented and tested.

## 3. Dependency rules

Dependencies point toward domain types and narrow interfaces. `arb-domain` cannot import the database, network clients, execution crate or UI protocol. `arb-risk` cannot call RPC. `arb-engine` consumes adapter interfaces and does not depend on either concrete chain adapter. `arb-sim` shares calculations with the engine without importing signer clients or submission implementations.

`arb-execution` defines the persistence and dispatch interfaces it needs; `arb-storage` and chain adapters implement them. Concrete applications compose implementations, avoiding a dependency cycle. Telemetry is injected or attached at the application boundary so deterministic functions remain testable.

The signer binary is the only application allowed to depend on a private-key backend. Chain workers use the restricted signing API. Control API and web packages cannot import signer internals or obtain signed transaction bytes. Generated browser types expose identifiers and redacted status, not complete execution payloads.

The OpenAPI specification is the contract source for generated TypeScript clients. Generation must be reproducible and CI checks for drift. Blockchain ABI bindings derive from pinned contract artifacts, with deployed address, chain ID, compiler settings, bytecode hash and source revision recorded together.

## 4. Fixtures and tests

Protocol arithmetic tests live beside their implementations. Use property tests for monotonic bounds and overflow behavior, and differential tests against the actual protocol math. Core tests inject state, clock and randomness; they must not depend on current market prices.

Fixture manifests record chain, block/hash or slot/context, program versions, capture method, completeness, schema and content digest. Compressed raw recordings reside outside Git once large; manifests remain versioned. No credentials or private keys appear in fixtures. Never use fixtures with unknown provenance to claim replay accuracy.

Contract tests cover callback authorization, repayment, allowed operations and final balances; fork suites verify integrated venues. Rust integration tests cover duplicate commands, pause/stop races, in-flight signer responses, pending disarm revocation, stale revisions, insufficient principal or spending authority, conflicting fee reservations, inconsistent state in every mode, journal failure, dispatch-start crash recovery, uncertain RPC responses, reorgs and recovery. `tests/system` exercises the assembled services with fake adapters and isolated infrastructure. Browser tests verify visible state and control semantics, without live signing access.

Benchmarks report machine, build profile, dependency revision, dataset, route universe, concurrency and percentile distributions. Store evaluator and replay workloads in crate `benches/`; keep comparable manifests and reports in `benchmarks/`. A new benchmark is justified by a real bottleneck or regression question.

## 5. Configuration and delivery

Checked-in configuration contains safe defaults and secret references. Secrets are injected outside Git. Worker sessions persist a resolved configuration digest so results remain attributable after edits. Changing session mode requires `STOPPED` and a new session; pause/resume preserves the existing mode. USDC-start routes are the initial default; WETH-start and wrapped-SOL-start routes require separate explicit configuration. Chain addresses and program IDs are verified per network before an adapter can be enabled.

Implementation CI runs formatting, linting, Rust tests, protocol differential checks, contract tests, API generation checks and the relevant browser tests. Scheduled or manual integration jobs use pinned fork heights and explicitly configured providers. Release images are pinned by digest and include dependency inventories.

An operator should receive a documented paper-mode startup, health check, stop procedure, backup/restore procedure and recovery drill before live functionality is enabled. Repository scaffolding must not include funded keys, an automatically enabled live configuration or a deployment script that broadcasts by default.
