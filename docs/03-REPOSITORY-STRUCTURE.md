# Repository structure

Status: research integration source, 12 September 2026. The repository includes domain/configuration, storage/control, capture/registry/adapters, scheduler/engine/paper crates, API/controlled-worker/replay applications, a connected React dashboard and Railway definitions. The comprehensive tree below remains a TARGET structure with future components. The current map and [implementation status](12-IMPLEMENTATION-STATUS.md) identify existing boundaries; [integration verification](17-RESEARCH-INTEGRATION-VERIFICATION.md) records pending CI separately.

## Current implementation map

| Paths | Current responsibility |
|---|---|
| `crates/arb-domain`, `arb-config` | Exact amounts, identities, evidence/decision types and immutable validated configuration |
| `crates/arb-adapter-api`, `arb-capture`, `arb-registry` | Read-only RPC, bounded immutable bundles, exact registry digest and pool selection |
| `crates/arb-evm`, `arb-solana` | Bounded decoding and candidate-only protocol math; deployed qualification remains separate |
| `crates/arb-scheduler`, `arb-engine` | Bounded generation-aware evaluation and same-network two-pool decisions |
| `crates/arb-paper`, `arb-storage`, `arb-control` | Exact virtual ledger, PostgreSQL history and durable worker admission fences |
| `apps/control-api`, `apps/research-worker` | Authenticated control/research endpoints and controlled OBSERVE/PAPER capture/evaluation |
| `apps/evm-worker`, `apps/solana-worker`, `apps/replay` | Standalone capture tools and offline retained-input verification/evaluation |
| `apps/web` | Explicit Demo/Connected UI, decision/coverage inspection and virtual-account views |
| `.railway/railway.ts`, `deploy/RAILWAY.md`, `deploy/Dockerfile.*` | Prepared Railway service graph and containers; no deployment claim |
| `crates/arb-evm/tests/reference` | Pinned npm oracle used only to reproduce synthetic differential fixtures |

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
    research-worker/
      Cargo.toml
      src/main.rs
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
    arb-config/
    arb-adapter-api/
    arb-capture/
    arb-registry/
    arb-scheduler/
    arb-paper/
    arb-control/
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
  .railway/
    railway.ts
  deploy/
    RAILWAY.md
    Dockerfile.{api,web,worker}
    compose.dev.yaml
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

Brace notation abbreviates siblings; it does not denote literal filenames. `apps/web` uses npm with `package-lock.json`; `crates/arb-evm/tests/reference` is a separate development-only npm oracle package. The current Rust members are declared in the root Cargo workspace. The eventual target also includes the planned system-test and execution components shown above; their appearance in this tree does not mean they exist. A future Solana program may need an isolated pinned build workspace; that decision follows the execution spike. Until then, `programs/solana/README.md` records the capability gate and no placeholder program is deployed.

The `planning` directory owns stable epic/ticket IDs and import metadata. GitHub issue numbers are remote identities recorded by the setup workflow; they do not replace stable ticket IDs. `wiki` contains concise onboarding/navigation source, while detailed requirements remain canonical in `docs`. A Wiki publisher must rewrite sibling `.md` links for native GitHub Wiki routes.

Each shared crate follows `Cargo.toml`, `src/lib.rs`, focused modules, `tests/` for integration tests and `benches/` where a measured performance question exists. Avoid splitting every adapter into a crate immediately; keep protocol modules together until build isolation or ownership warrants a split.

## 2. Target crate responsibilities

The table below retains the planned execution decomposition. Current equivalents are listed above: `arb-control` owns worker fencing, `arb-paper` owns virtual accounting, and concrete adapter math is called directly by the current pure `arb-engine`. Proposed `arb-risk`, `arb-sim`, execution and signing crates are not implied to be implemented.

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

The chain adapters expose different capabilities instead of pretending every venue supports the same pool model or flash-loan flow. The complete target venue module provides a decoder, exact quote implementation, transaction builder, fixture manifest and readiness declaration; current research adapters do not expose transaction builders. Unsupported token extensions and unusual transfer behavior are rejected unless explicitly implemented and tested.

## 3. Dependency rules

Dependencies point toward domain types and narrow interfaces. `arb-domain` cannot import the database, network clients, execution crate or UI protocol. `arb-risk` cannot call RPC. The target engine can consume narrow adapter interfaces as more protocols arrive. The current pure `arb-engine` directly depends on the two concrete math adapters; it performs no RPC or database access, and applications retain all transport/admission ownership. `arb-sim` shares calculations with the engine without importing signer clients or submission implementations.

In the future execution decomposition, `arb-execution` defines the persistence and dispatch interfaces it needs; `arb-storage` and chain adapters implement them. Concrete applications compose implementations, avoiding a dependency cycle. Telemetry is injected or attached at the application boundary so deterministic functions remain testable.

When live capability is implemented, the signer binary must be the only application allowed to depend on a private-key backend. Future live chain workers use the restricted signing API. Control API and web packages cannot import signer internals or obtain signed transaction bytes. Browser types must expose identifiers and redacted status, not complete execution payloads.

The OpenAPI specification is the API contract source. The current TypeScript client uses explicit runtime validation; full client generation and a reproducible drift check remain targets. Blockchain ABI bindings derive from pinned contract artifacts, with deployed address, chain ID, compiler settings, bytecode hash and source revision recorded together.

## 4. Fixtures and tests

Protocol arithmetic tests live beside their implementations. Use property tests for monotonic bounds and overflow behavior, and differential tests against the actual protocol math. Core tests inject state, clock and randomness; they must not depend on current market prices.

Fixture manifests record chain, block/hash or slot/context, program versions, capture method, completeness, schema and content digest. Compressed raw recordings reside outside Git once large; manifests remain versioned. No credentials or private keys appear in fixtures. Never use fixtures with unknown provenance to claim replay accuracy.

Current Rust tests cover protocol fixtures, malformed/incomplete capture, durable commands and decision/account idempotency, generation fencing and virtual-inventory invariants. Current browser tests target visible research state and controls without signing access; the integration record identifies checks actually executed. Future contract/fork tests must cover callback authorization, repayment, permitted operations and final balances. The planned execution/system suites must also cover in-flight signer responses, pending disarm revocation, spending authority, dispatch-start crashes, uncertain outcomes, reorgs and reconciliation. These future suites are requirements, not existing test evidence.

Benchmarks report machine, build profile, dependency revision, dataset, route universe, concurrency and percentile distributions. Store evaluator and replay workloads in crate `benches/`; keep comparable manifests and reports in `benchmarks/`. A new benchmark is justified by a real bottleneck or regression question.

## 5. Configuration and delivery

Checked-in configuration contains safe defaults and secret references. Secrets are injected outside Git. Worker sessions persist a resolved configuration digest so results remain attributable after edits. Changing session mode requires `STOPPED` and a new session; pause/resume preserves the existing mode. USDC-start routes are the initial default; WETH-start and wrapped-SOL-start routes require separate explicit configuration. Chain addresses and program IDs are verified per network before an adapter can be enabled.

Current CI definitions cover formatting, linting, Rust/PostgreSQL integration, protocol reference checks, API/schema validation, browser tests and prepared containers; [the integration record](17-RESEARCH-INTEGRATION-VERIFICATION.md) states which checkpoint actually passed. Contract/executor tests become required when those future capabilities are implemented. Future provider/fork jobs must use pinned state and explicitly configured providers. Production releases must pin image digests and include dependency inventories; the present container definitions use named versions and require that release review.

An operator should receive a documented paper-mode startup, health check, stop procedure, backup/restore procedure and recovery drill before live functionality is enabled. Repository scaffolding must not include funded keys, an automatically enabled live configuration or a deployment script that broadcasts by default.
