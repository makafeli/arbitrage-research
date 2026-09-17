# Anchored Base session registration

Existing #58 / ARB-044. This connects the prepared Base profile to the persisted
research session and API catalog. It is not full paper settlement or live trading.

The owner has completed account and Base RPC setup. PR146 prepared the reviewed
two-pool profile on the private worker volume using 27 actual read-only RPC calls.
The concrete deployment/result is recorded on #58; secrets never leave Railway.

## Explicit session command

Set ARB_BASE_PROFILE_DIGEST to the validated configuration digest from the retained
preparation result. With the existing operator and database environment, run:

```sh
worker-entrypoint worker-session --register /data/runtime/base-v1
```

The production Rust parsers revalidate the Base-only OBSERVE profile, registry
binding, capture path/quota and supplied digest before any database connection.
Registration uses existing Store methods and one fixed operator-scoped idempotency
key. The database connection requires TLS. No migration, source initialization,
command, wallet or provider call is performed. A new session stays RECOVERING until
its eventual worker performs recovery; it is not shown as a running scanner.

`--status` with the same directory only reads the existing creation receipt. The
same profile reuses the original session, including its current state, without
resetting it. Changed profile payloads conflict. Other existing Base sessions need
explicit selection and are not silently adopted or duplicated. Concurrent requests
with the same key converge through existing Store serialization. A failed write
can leave the immutable configuration registered but cannot partially create a
session. A timeout means outcome unknown: inspect/retry with the SAME key.

The private single-writer volume and trusted environment are deployment
prerequisites. This is an internal administrator CLI, not a public endpoint.

## Matching API catalog

The API image accepts an optional set of three non-secret settings:
ARB_BASE_PROFILE_TOML, ARB_BASE_PROFILE_REGISTRY and ARB_BASE_PROFILE_DIGEST. All or
none must be present. The first two are the exact prepared file contents, not a
provider endpoint. Registry byte order matters to its content digest.

The entrypoint writes a private ephemeral copy, runs the same Rust profile checker,
verifies the external configuration digest and appends the validated TOML to the
existing ARB_CONFIG_FILES. Original account/login and CLI arguments are retained.
Bad or partial settings stop startup instead of silently selecting the inert
profile. `api-entrypoint --check-profile` validates offline without login/database
access. No Base API key is needed by or copied to the control API.

## Verification and limits

The required container drill uses a synthetic prepared profile and isolated
PostgreSQL, verifies encrypted registration, exact reuse, concurrent idempotency,
wrong-anchor refusal and no source/command/account writes. It also checks the
shipped API profile wrapper with the same files, including mismatch and incomplete
settings. Actual CI/deployment outcomes belong on the PR and #58, not inferred from
this document. Local container execution may be unavailable.

The eventual source-backed worker must still initialize a genuinely absent source,
recover to the matching captured block and prove hosted START/STOP/restart. No
registration automatically performs those actions or makes paper trades executable.
