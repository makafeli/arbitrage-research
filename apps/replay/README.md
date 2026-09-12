# Offline replay

`replay --verify-capture DIRECTORY --manifest-digest sha256:HASH` verifies retained capture hashes, exact request/response order, protocol source revision, frozen configuration authorization where required, and deterministic re-decoding. It makes zero network requests. The original single-pool capture format remains supported; bounded pool-set captures select their pool from the exact retained registry document.

`replay --evaluate-captures REQUEST.json` additionally requires valid frozen configuration for every origin, re-decodes every capture, then runs bounded research math using the same engine as the controlled worker. It reports CANDIDATE decisions, explicit rejection reasons and historical timing policy. It does not simulate complete transactions, settle virtual trades or establish current protocol/provider qualification.

## Evaluation request

Supply a regular JSON file at most 1 MiB with no unknown fields:

```json
{
  "schema_version": 1,
  "session_id": "original-research-session",
  "experiment_id": "original-experiment",
  "strategy_id": "cyclic-exact-in-2leg-v1",
  "network_id": "base-mainnet",
  "generation": 1,
  "observed_at_unix_ms": 1780000000000,
  "input_age_ms": 100,
  "captures": [
    {"path": "/data/captures/first", "manifest_digest": "sha256:<64 lowercase hex digits>"},
    {"path": "/data/captures/second", "manifest_digest": "sha256:<64 lowercase hex digits>"}
  ]
}
```

The manifest placeholders must be replaced with actual expected hashes. Capture paths resolve relative to the process working directory, unless absolute. At most eight retained captures are accepted. Their frozen configuration digests must agree; the engine checks their network, provenance, pool uniqueness and chain context. Original configuration is never an instruction to resolve secrets or connect to RPC.

`input_age_ms` is explicit historical elapsed time from the batch observation. It has no default and must be below 65,000 ms. This is a user-supplied modeled timing scenario; the retained manifests alone do not prove actual batch latency. Reproduce the original stored decision's age and metadata to compare deterministic outputs. A different historical age is a different scenario and may change rejection status. The frozen freshness threshold still applies. Present wall time is used only for raw-retention validation, never to silently make historical quotes fresh.

Output preserves SYNTHETIC, MANUALLY_CONSTRUCTED or RECORDED_LIVE origin and exact integer strings. Gross route math includes protocol pool fees and price impact. External costs remain unknown, so the engine's opportunity projection has a null net amount. Captured input alone is not proof of executable profit.

Decoder-only synthetic fixtures with arbitrary nonvalidated configuration remain usable for `--verify-capture`; they cannot be promoted to economic evidence by changing their origin label. Raw objects that have expired, mismatch their digests or cannot be deterministically re-decoded are rejected.

`replay --lifecycle-demo` remains an explicitly synthetic in-memory control-state demonstration.
