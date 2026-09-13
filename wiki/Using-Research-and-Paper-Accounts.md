# Using research and paper accounts

The connected dashboard inspects retained research and virtual accounting. Current OBSERVE and PAPER workers capture read-only state and calculate bounded two-pool CANDIDATE decisions. They do not send transactions, simulate a complete atomic transaction or turn a positive quote into a paper fill. Demo mode remains visibly synthetic. See [implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) and [integration verification](https://github.com/makafeli/arbitrage-research/blob/main/docs/19-ANCHORED-CAPTURE-AND-COST-VERIFICATION.md) for current acceptance evidence.

## Prepare a session

1. Use a validated, enabled configuration and the exact qualified pool registry. Both shipped networks are disabled; demo identities and decoder fixtures are not a production registry. Register the configuration with the API and resolve its database/RPC secret references only in the relevant service environment.
2. In Connected mode, authenticate with the operator secret. In Experiments, select the registered configuration, enabled network and experiment reference, then review the immutable mode and configuration digest. Creating the session does not start a worker or issue START.
3. Run one controlled research worker for that session using the same frozen configuration, registry digest and mode. Its recovery reaches STOPPED before any explicit START. Use the [worker README](https://github.com/makafeli/arbitrage-research/blob/main/apps/research-worker/README.md) and [Railway runbook](https://github.com/makafeli/arbitrage-research/blob/main/deploy/RAILWAY.md) for environment references and dedicated persistent capture storage.
4. For virtual capital, use a PAPER configuration and initialize a run while its session is STOPPED as described below. OBSERVE and PAPER are immutable choices; switching modes requires a new configured session.
5. Issue START for that named session when ready. The worker needs recent successful acquisition before it can apply START. Inspect its durable receipt and observed state. HTTP connectivity alone cannot establish worker or market readiness.

The chain selector filters visible records. It does not reconfigure, start or stop either network. No wallet key, token approval or trading capital is needed for these research functions.

## Understand START, PAUSE and STOP

| Display | What it establishes |
|---|---|
| PENDING command | The API accepted the request; the worker has not yet acknowledged its effective fence/state |
| APPLIED receipt | The worker acknowledged the requested operation at its recorded revision |
| RUNNING | Current-generation research can be admitted within configured bounds |
| PAUSED or STOPPED | Candidate evaluation/admission is fenced; raw acquisition can continue |
| RECOVERING or FAULTED | New work cannot be treated as ready; inspect the recorded reason and recovery state |

Late read responses can produce retained unadmitted raw bundles. They are not newly admitted decisions or financial outcomes. To stop all provider reads, first verify the requested session fence, then stop the worker process. Standalone `evm-worker` and `solana-worker` capture commands are outside dashboard session controls. Process shutdown or browser disconnection is not an APPLIED receipt; a replacement owner recovers fenced and needs a fresh START.

## Read decisions and coverage

The decision explorer keeps raw observations separate from deterministic groups of repeated observations. Inspect the selected record's mode, origin, network, configuration digest, capture/state references, age and rejection reason. SYNTHETIC and MANUALLY_CONSTRUCTED records remain such even when served by the real API; RECORDED_LIVE means retained market input, not a fresh or executed trade.

Exact local quotes include supported protocol pool fees and price impact. External execution costs remain unknown and the net amount is `null`. Do not subtract pool fees twice or interpret unavailable costs as zero. Successful arithmetic remains CANDIDATE evidence; rejected or incomplete calculations cannot supply an invented output.

Coverage counts describe stored raw records and groups. Collection completeness is `UNKNOWN`; eligible execution attempts and reconciled transactions are unavailable. Empty history does not prove a working collector found zero market opportunities. Stored observation bounds do not establish uninterrupted uptime, complete venue coverage or a success rate. Run or chain rankings remain unavailable without comparable coverage, valuation and simulation evidence.

## Inspect collection diagnostics

In System, select the session to inspect its registered collection attempts. READINESS batches can continue while stopped; RESEARCH batches are evaluated under an admitted generation. Provider/input failures, evaluation failures/deadlines, deliberate suppression and shutdown are distinct outcomes. IN_PROGRESS means no terminal evidence is retained; it may be active work or a crashed attempt, and is never counted as success. Recorded decision IDs connect successful batches to their exact observations.

A zero-error count does not establish uninterrupted collection. The denominator is registered batches only, and the operational attempt record has no asserted market-input provenance. Check provider configuration, worker logs/heartbeat and capture files according to the named diagnostic. Automated alert delivery and complete chain-lag/queue/storage monitoring remain future acceptance work.

## Assess hypothetical costs

For a QUOTED observation, choose Assess hypothetical costs. Enter a named version of your manual assumptions, native fee amounts and exact historical valuation into the starting asset. Leave unavailable inputs missing; enter zero only when that is your explicit assumption. The valuation timestamp is checked against the original decision time, not the current clock. Funding and overhead are separately disclosed.

Saving creates an immutable assessment with source and scenario hashes. The history retains exact positive, negative and unavailable results. Known manual costs remain CANDIDATE evidence and do not update the original quote or paper balance. If delivery is uncertain, retry the unchanged request using the retained idempotency key. A full reload loses browser pending state; inspect history before creating an equivalent new assessment. The bounded session export includes the stored assessments and their source decisions.

## Initialize virtual capital

In Runs, choose a PAPER session in the Paper accounting workspace. The API must advertise account creation, expose the matching enabled configuration and observe the session as STOPPED. A concurrent pending command can block creation even when an earlier page still shows STOPPED.

Select a validated token principal asset and enter exact integer minor units. Enter the separate native fee inventory explicitly, also in integer minor units, then review the configuration and amounts. No decimal precision or price conversion is inferred. A zero native balance means no fee inventory; it does not mean network fees are free. The current form initializes one token principal and the separate native fee asset.

Creation produces a new immutable initial-capital run. Prior runs and their initial balances remain retained. Inspect its free, reserved and total balances, original initial balances, journal and reservation history. All amounts remain HYPOTHETICAL. Starting the PAPER worker creates research decisions only; it does not automatically reserve or settle this capital. The public API offers no arbitrary settlement or ledger-reset control.

If creation delivery is uncertain, retry the same request. The dashboard retains its original session, amounts and idempotency key while that request is unresolved; changing pages does not reset the request. A full reload or closing the app discards the browser's pending-request state. Inspect authoritative retained runs before creating equivalent new work after that loss. A timeout is not evidence that creation failed.

## Export or replay evidence

Page JSON buttons still export only their displayed scope. Use the session snapshot export to fetch all defined stored research records together, up to 10,000 source rows and 8 MiB. It includes decisions, paper runs and exact journal/account data, collection attempts, manual cost assessments and capture references from one consistent database snapshot. Both JSON and CSV downloads use that same fetched response. The snapshot hash and source counts support external checks; exceeding a bound refuses the whole export rather than silently truncating it.

Paper journal freeform reasons are removed and command/attempt IDs become stable pseudonyms, with original payload hashes retained. Financial amounts remain exact strings. The CSV quotes separators/newlines and guards formula-leading scalar cells. Each payload_json cell contains its exact JSON record; parse it to recover authoritative identities and amounts without guessing which apostrophes are protective. Asset decimals are unavailable in the retained database model. Provider settings, authentication material, local paths and raw market files are excluded. Catalog PRESENT does not prove file availability or expiry: those remain NOT_VERIFIED/UNKNOWN until the actual raw bundle is checked. This is a frozen database export, not a complete historical replay package.

Offline replay separately verifies exact retained manifests, configuration, registry and raw request/response order before re-decoding and evaluation. `replay --evaluate-captures` requires an explicit modeled historical input age and preserves original provenance. It makes no network requests and does not reconstruct unrecorded arrival times, complete transaction simulation or counterfactual paper fills. See [replay usage](https://github.com/makafeli/arbitrage-research/blob/main/apps/replay/README.md).

## Current acquisition limits

A controlled worker owns one network and captures 1–8 allowlisted pools in a batch, with at most 4,096 read requests, 64 MiB of retained responses, a five-second request timeout and a 60-second cumulative transport deadline. Evaluation also obeys a bounded processing deadline and the frozen freshness policy. These are limits, not achieved latency or provider reliability measurements.

Pool-set capture uses one finalized Base block hash for the complete batch, including a final canonical check, or one finalized Solana account-union response with at most 100 accounts. Each version-2 bundle preserves the complete original batch transcript; replay validates every selected pool. A common anchor does not qualify a provider or establish Solana bank/write coherence, chain-state lag or stream continuity. Those remain acceptance work.

Capture quotas include old and incomplete bundles. Retention metadata does not automatically delete files. A healthy API, passing synthetic fixture or stored candidate is insufficient to claim an operating market campaign or positive returns.
