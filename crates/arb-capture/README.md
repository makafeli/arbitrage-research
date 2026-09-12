# Capture bundles

Schema 1 binds origin, chain context, provider alias, adapter source commit, binary and effective configuration digests, ordering, timestamps, retention expiry, completeness and per-object byte lengths/SHA-256 checksums. A capture is one new directory containing `manifest.json`, `registry.json`, `rpc.json`, `snapshot.json` and `effective-config.json`. Generic library users may store other flat JSON object names; replay supports the four specified input objects.

Publication creates `INCOMPLETE`, syncs each object, publishes the manifest last, syncs the directory, removes the marker and syncs again. Errors leave an incomplete directory or lack a committed manifest. Existing captures cannot be overwritten by this API. The reader rejects the incomplete marker, unsupported schema, missing or oversized files, checksum failures, expired raw retention, path traversal and non-regular input files. SHA-256 establishes integrity against a recorded digest; it does not authenticate an author or RPC provider.

`write_bundle` enforces a **per-bundle** quota capped at 64 MiB, not an aggregate capture-directory quota. Global retention cleanup, disk-pressure forecasting and decision-to-capture database links remain integration work. Retention expiry is immutable in the manifest; `load_bundle` refuses expired input without rewriting historic results. The directory must be privately owned by the operator: this API is not a hostile shared-filesystem sandbox.

`Origin::Synthetic`, `ManuallyConstructed` and `RecordedLive` remain distinct. Recorded-live means state acquired from a real RPC; it does not mean a live trade, a qualified quote or a performance measurement. Only a downstream validated report can decide whether a recorded input is fit for analysis.
