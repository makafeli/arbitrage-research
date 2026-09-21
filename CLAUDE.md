# CLAUDE.md

This project's context for all AI tools lives in [AGENTS.md](./AGENTS.md).
Claude Code loads it via the import below:

@AGENTS.md

## Claude Code notes (things not in AGENTS.md)

### Where things are

- GitHub: `makafeli/arbitrage-research`, default branch `main`. Branch names: `feat|fix|docs/ARB-xxx-description`. Never push to `main` directly; open a PR.
- Railway: project `arbitrage-research` (`c7cea88d-24b8-45dc-9602-7c0c272f1da7`), environment `production` (`5ff4044a-b0ac-4b02-b9cd-888cd91a3fee`), region `europe-west4`. Services: `web`, `control-api`, `base-research-worker`, `postgres`. Railway builds from GitHub `main`; never upload a local tree with the Railway CLI.
- Operational handoff (Dutch): `docs/ARBITRAGE-RESEARCH-HANDOFF-2026-09-18.md`. Read §2, §6, §7 and §9.4 before touching production or the worker. Live operations ticket: issue #58.
- Runbooks: `docs/BASE-WORKER-LAUNCH.md`, `docs/POSTGRES-LEAF-REPAIR.md`, `deploy/RAILWAY.md`.
- Backtesting and historical-evidence plan: `docs/ARBITRAGE-BACKTESTING-DATA-HANDOFF-2026-09-21.md` (work packages BT-01…BT-08, gates A–D). A proposal, not executed; BT-ids are not issue numbers.

### Local checks (mirror `.github/workflows/ci.yml`)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --locked -p control-api
cargo test --workspace --locked      # needs TEST_DATABASE_URL + ARB_TEST_CONTROL_API_BIN, see README
python scripts/validate_specs.py && python scripts/validate_project.py
npm test --prefix apps/web && npm run build --prefix apps/web
cd contracts/base-guard && forge fmt --check && forge build --sizes && forge test -vv   # foundry 1.8.3, offline
```

Rust is pinned to 1.90.0 in `rust-toolchain.toml` (rustup). Web needs Node 24+. `TEST_DATABASE_URL` must never point at production. Skipping a test for a missing PostgreSQL or Docker is not a passing test.

### Handoff boundaries (in addition to AGENTS.md)

- Never print, log, or commit secrets. Do not list Railway variables (`ARB_BASE_RPC_URL`, `ARB_DATABASE_URL`, ...) in chat.
- Do not disable TLS, CI-gate or source checks. Do not reset a HALTED source, migrate an OBSERVE session to PAPER/LIVE, or raise provider budgets to make a test pass.
- Do not rebuild what PR #148 (launcher) and PR #149 (cert repair) already merged.
- `worker-entrypoint worker-launch-base --initialize-and-start` is one-time only; later starts use `--start`. Neither sends a research START.
- Base-first is a priority, not an approved Base-only scope. Keep the Base+Solana acceptance criteria unless the owner records a scope change.

### Skills

`.claude/skills/` holds the JavaScript-Mastery-Pro set, pinned in `skills-lock.json`:
`/scope` → `/architect` → `/develop` → `/test` → `/check` → `/document` → `/sync`. `/debug` for bugs, `/audit` for context gaps.

### Hooks (`.claude/settings.json`)

- `hooks/guard-prod.py` blocks force-push, push to `main`, Railway mutations (`up`, redeploy, restart, delete, restore, GraphQL mutations) and Railway secret printing (`variables`, `run`, `shell`), also through subshells, `sh -c`, `eval` and wrappers. `railway ssh`, `connect`, `api` queries and `volume` reads are open for #58 maintenance (owner decision 2026-09-18). Run blocked commands yourself in a terminal. Test: `python3 .claude/hooks/test_guard_prod.py` (also in CI).
- `hooks/rustfmt.sh` formats `.rs` files after every edit.
