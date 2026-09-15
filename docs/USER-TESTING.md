# First user test: what works and how to run it

This guide describes the research build, not a live trading product. It uses the
existing main-branch workflow and the existing `ARB_BASE_RPC_URL` secret. Do not
create another provider account, paste a key into an issue, fund a wallet, or
interpret a candidate quote as a trade.

## Choose the right test

| Test | Available boundary |
|---|---|
| Explore the dashboard interactively | The local Demo is available now. It is labelled synthetic; it does not test the provider or backend. |
| Exercise real Base inputs end to end | The explicit recorded-slice workflow is available now. It checks actual capture, persisted decisions, replay, STOP, restart and the authenticated browser, then terminates its disposable services. |
| Operate a continuous Base/Solana scanner | Not accepted yet. Subscription/reconnect integration, durable checkpoints and rollback/gap invalidation, qualified snapshots and both adapters remain required. |
| Automatically simulate a paper-trading account | Not accepted yet. Existing ledger screens and reservations do not constitute automatic fills, complete transaction simulation or profitability evidence. |
| Trade real funds | Not available or authorized. Signing and broadcasting are outside this research build. |

EPIC-02 has eight accepted original children, but the last integration task #29
also depends on original M2 tasks outside that epic. Eight out of nine is not an
89% estimate for the whole product. A successful first test does not close those
gates; their current native issues remain authoritative.

## Real-data test with no local installation

1. Sign in to GitHub as `makafeli` and open
   [Recorded Base observation slice](https://github.com/makafeli/arbitrage-research/actions/workflows/recorded-base-slice.yml).
2. Select **Run workflow**, choose **main**, then start a new run. Do not select
   **Re-run jobs** on an old pull-request run: its provider job can remain skipped.
3. Verify **both** `offline` and `recorded` complete successfully. Offline success
   with `recorded` skipped means no real-data experiment occurred.
4. Open the run's **Artifacts** section and save `recorded-base-slice-evidence`.
   Unpack it into a new local directory. Do not mix outputs from different runs.
5. Inspect the result and images described below. Record the run URL/source SHA
   with any finding so a later investigation uses the correct code and inputs.

The workflow reads the already-configured repository secret only in the explicit
collection step. Builds and ordinary PR tests do not receive the provider key.
There is no subscription purchase, automatic retry loop, wallet or trade. Reads
consume the provider's existing request allowance; run one test at a time and do
not repeatedly retry a failed provider step. A changed contract, unavailable
input or quota failure is a result to diagnose, not a reason to weaken validation.

### What a passing test contains

| File | Inspect |
|---|---|
| `result.json` | Status `RECORDED_ROUTE_CONTROL_REPLAY_VERIFIED`, source/configuration/build identifiers, positive `matched_decisions`, and `execution_authorized: false`. |
| `result.json` control records | START and STOP each have separate PENDING acceptance and APPLIED receipts. `stopped` and `restart` have actual STOPPED state; restart preserves the session and configuration. |
| `decisions.json` | Exact token units, route order, capture references and visible unknown costs. Negative or rejected routes are valid research findings, not a failed profit target. |
| `replay.json` | `network_requests: 0`, `dataset_origin: RECORDED_LIVE` and the same decisions checked by the harness. |
| `recorded-overview.png` and `recorded-decisions.png` | Actual authenticated dashboard output for that run, not a mock or a persistent public live-feed page. |
| `browser.json` | Correct session/origin/configuration, no HTTP mocks, and no browser command mutations beyond login. |
| `coverage.json` / `collection-attempts.json` | Actual captured interval and failures/gaps. Counts describe this bounded run, not the whole market. |
| `SHA256SUMS` | File integrity within this retained artifact. A hash is not independent proof of provider honesty. |

On macOS, from the extracted artifact directory, check file integrity with:

```sh
shasum -a 256 -c SHA256SUMS
```

A failed run may retain `collection-failure.json`; that diagnosis is not a passing
experiment. `EXPORT_READY` means the bounded export checks passed, not that all
research requirements passed. Do not upload raw private configuration or keys in
a bug report. Evidence artifacts expire after 30 days, so preserve needed copies.

The historical successful run is
[34977582214](https://github.com/makafeli/arbitrage-research/actions/runs/34977582214).
Its exact source and limitations are in [the result record](RECORDED-BASE-SLICE-RESULT.md).
It produced two negative gross candidate routes, and replay matched both without
network I/O. Its roughly 39.5-second acquisition/evaluation age is not useful
low-latency trading evidence. Reuse that evidence for review rather than running
another provider experiment solely to obtain a screenshot.

## Interactive dashboard test on your own computer

Use Node.js 24 or later and a clean checkout of this repository:

```sh
cd apps/web
npm ci
npm run dev
```

Open `http://127.0.0.1:5173`. Exercise the six sections, both themes, narrow-screen
layout and chain filters in **Demo**. Synthetic labels must remain visible.
This preview requires neither an RPC URL nor a wallet. Stop the local Vite
process with Ctrl+C when done.

**Connect API** does not start or install a backend. Connected testing also needs
the separately configured API, PostgreSQL and session/worker setup described in
[the dashboard README](../apps/web/README.md). The CI recorded test launches and
cleans up those disposable services automatically, but does not leave an
interactive server running on your computer or in Railway. Closing a Connected
browser tab is not a STOP command for separately running workers.

## Railway is a separate deployment check

[The deployment record](21-RAILWAY-DEPLOYMENT-VERIFICATION.md) identifies the
existing project and public dashboard. Its last recorded verification was on
13 September 2026; it does not prove the currently deployed SHA or health. In that
verified deployment, only web/API/PostgreSQL were deployed and research workers
were deferred. GitHub CI success does not establish a later Railway rollout or
an active live feed. Recheck deployment IDs, current commit, authenticated health
and worker presence before describing the hosted environment as ready for
connected market testing. Do not provision a duplicate project.

## Next release boundary

Current #30 work adds event parsing and bounded canonical-hash log recovery.
Those are components, not a running subscription or durable checkpoint service.
The next useful release target is a continuously observable research session with
explicit gaps, recovered cursors, rollback invalidation, qualified snapshot and
route inputs, and responsive STOP under provider failure. The original tickets
#30/#32/#36/#37 and their prerequisites must supply that evidence. Complete atomic
simulation and automated paper settlement remain later gates. None is marked
complete solely because this test guide or the backfill module exists.
