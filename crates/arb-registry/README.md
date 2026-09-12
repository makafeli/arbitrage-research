# Registry documents

Capture and offline replay share one bounded document parser. Existing single-pool JSON remains supported unchanged. A version 1 pool set wraps 1–8 protocol registries:

```json
{"schema_version":1,"network_id":"base-mainnet","pools":["<full protocol registry objects>"]}
```

The example above illustrates the wrapper only; strings are not valid pool entries. Every pool and token must occur in the frozen configuration allowlists. The configuration binds the SHA-256 digest of the complete original document. Base addresses compare without case; Solana addresses compare exactly. Duplicate pools, unknown fields, duplicate struct fields, excess bytes and cross-network documents are rejected.

Each capture retains the full document in its existing `registry.json` object; `snapshot.json.pool` selects the pool. Pool-set captures use separate adapter version identifiers while legacy capture bytes and version identifiers remain unchanged. Structural authorization does not establish current protocol equivalence, provider qualification, quote eligibility or executable arbitrage.
