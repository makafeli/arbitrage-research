# Capture-pair research engine

The engine deterministically enumerates directed two-leg cycles through distinct pools on one configured network, using only the frozen start asset and exact configured base-unit sizes. Base uses the pinned V3 integer math; Solana uses the pinned historical Orca Core static-fee and legacy-SPL model. Pool fees are included once in quoted output. External costs remain unknown, so the resulting OpportunityRecord 1.1 has a null net estimate.

Input captures must already have their manifest bytes, registry bytes, snapshot contents, network identities, origin and session/configuration binding verified by the capture layer. The engine rechecks allowlists, typed pool/asset continuity, origin, configuration digest and pair context. It does not authenticate a provider or prove that two independent state reads constitute an atomic transaction.

The compile-time policy identified by calculation version `capture-pair-research-v1;bounds8x63;group1000` allows at most eight input pools, sixty-three route-size calculations and sixty-four output traces in a 256 KiB JSON batch. A smaller configured evaluation queue lowers the calculation count. A reserved final DATA_UNAVAILABLE trace explicitly marks capacity exhaustion; raw earlier rejections and quotes are retained. Configuration schema and historical canonical configuration hashes do not change.

The generation/admission/deadline gate is checked before discovery, between quote legs and before returning any batch. Input age is elapsed monotonic time from the original batch observation, including capture and queue waiting; staleness produces an explicit rejection. Cancellation or deadline expiry returns no partial batch. The caller must repeat lease and generation checks in the database transaction that stores the batch. The synchronous engine creates no queues or detached jobs.

Every quote is CANDIDATE, including losses. No evidence is promoted to SIMULATED or ESTIMATED_EXECUTABLE. Current deployed-program equivalence, token-behavior qualification, full transaction building/simulation, funding reservations and submission remain unavailable. A registry digest or supplied snapshot quality boolean cannot enable these capabilities.

Tests cover actual Base mathematical quotes, exact configured sizes and stable ordering, context/staleness rejection, missing coverage, explicit route/evaluation capacity, empty input and no-route denominators, provenance/configuration spoofing and generation/deadline cancellation between legs. Actual provider qualification, current Orca deployment parity, full atomic simulation and execution are separate acceptance gates.

When the selected network has an explicit frozen chain-freshness policy, calculation
version `capture-pair-research-v2;bounds8x63;group1000;finalized-chain-time-v1` emits
DecisionTrace schema 1.1 with an immutable `chain_freshness` report. Each source binds
a capture ID to its exact finalized Base block number/hash/parent or finalized Solana
slot/genesis/account context. Base uses the captured block timestamp; Solana uses an
estimated block time, and a missing estimate remains unknown. A shared account response
and an estimated timestamp do not establish provider qualification or write coherence.

The reference UTC is the original batch observation plus monotonic elapsed processing
time, not the latest capture receipt. Sources in the future have null ages and are
explicitly rejected. Missing, future and stale chain times block initial batch quoting
with DATA_UNAVAILABLE; crossing the configured limit while quoting rejects the route.
A terminal gate reassesses every earlier QUOTED trace before the completed batch leaves
the engine, preventing a slow later route from preserving an expired earlier quote.
Conflicting Solana timestamps for the same shared context are rejected when opted in.
The supplied timestamps and policy remain research assumptions; no rollback/history
tracker or provider qualification is inferred.

The legacy absent-policy path keeps schema 1.0, its calculation version and omitted
report bytes. Its input age means local processing age only. Both legacy and opt-in
opportunity projections keep `state_fresh_and_coherent` false, CANDIDATE evidence,
unknown external-cost net and no simulation or execution capability.
