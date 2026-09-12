# Offline acquisition verification

The replay executable re-decodes a captured Base or Solana RPC transcript entirely offline. It verifies the supplied manifest digest, object lengths/hashes, retention, schema and sequence coverage, consumes the transcript with exact method/parameter matching, and compares the resulting snapshot and context with the recorded snapshot.

```sh
cargo run --locked -p replay -- --verify-capture /path/to/capture --manifest-digest sha256:EXPECTED_HASH
```

Use the digest emitted at capture time or stored in the durable admission record. Hash agreement checks integrity against that reference; it does not authenticate an author by itself. Recorded-live captures also revalidate effective configuration, mode/network, registry digest, pool/asset allowlists and supported adapter format. This path never resolves credential references or creates an HTTP client. Missing data cannot be backfilled from current chain state.

The report says `ACQUISITION_REDECODE_MATCHED` and retains the fixture/recorded origin, protocol source revision, original build digest and current replay build digest. It explicitly reports quote replay, paper P&L and full transaction simulation as unavailable. A matching decoded state is capture-layer evidence; ARB-027 economic replay remains incomplete.

The CLI uses the current clock to enforce raw-retention expiry. The library accepts an injected clock for deterministic tests. Redirect the JSON output if a durable verification record is needed. Raw captured data is not printed automatically.

## Synthetic control example

```sh
cargo run --locked -p replay -- --lifecycle-demo
```

This separate in-memory example records a fictional unresolved attempt, requests STOP, applies the fence, shows DRAINING and resolves it to STOPPED. It contains no prices, P&L, persistence or market data.

## Verification

`cargo test -p replay` re-decodes both manually constructed protocol fixtures, checks repeatability and rejects altered normalized state even after a bundle is rehashed, changed configuration/source identity, sequence mismatch and expired inputs. These tests establish acquisition decoding integrity. Exact quote evaluation, complete transaction plans, seeded scenarios and virtual ledger integration remain separate work.
