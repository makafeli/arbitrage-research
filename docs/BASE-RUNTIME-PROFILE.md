# Explicit Base runtime profile preparation

Existing work: #58 / ARB-044, using the identity and worker contracts already in main.
This is not completion of the original release, paper settlement or live trading.

## Operator input

The owner has supplied the existing ARB_BASE_RPC_URL privately to the Railway
base-research-worker. No new account or login step is required. Presence of the
variable is not proof of endpoint correctness; the preparation below actually
queries that endpoint. No fallback provider or purchased service is used.

## Finite action

After exact-source CI/review/merge, the integrator can run the existing private
worker service with `worker-entrypoint worker-prepare-base`. Its existing pre-deploy
CI gate remains mandatory. This explicit command performs at most one bounded Base
identity observation using the existing collector: 48 requests maximum, 24 MiB
total response cap, ten-second per-request timeout, existing 400-ms pacing and a
300-second process alarm. These are read-only JSON-RPC methods, never orders.

The exact reviewed factory, WETH/USDC identities and two Uniswap v3 pools must
match docs/registries/initial-identities.json. A wrong chain, changed identity,
provider error or canonical recheck failure stops preparation. Successful output
contains only constructed configuration, known registry fields, hashes and fixed
status fields. It never includes the endpoint, key or raw provider responses.

The current bitmap word is selected for each pool using floor division of the
observed tick by spacing and 256. This does not invent wider tick coverage or
attest full quote/transaction eligibility. The profile is OBSERVE with a 1-USDC
candidate input size, a 128-MiB capture quota below the existing 1-GB volume, the
existing research-only settings and no enabled Solana or signing/broadcast path.

`worker-profile-check --check CONFIG REGISTRY` uses the production Rust
ValidatedConfig and RegistryDocument parsers. It verifies the exact registry/config
binding, Base-only OBSERVE scope, secret references and capture path/quota without
contacting a database or provider. The main research-worker default remains intact.

## Files and restart behavior

The entrypoint creates /data/runtime privately for UID/GID10001, like the existing
capture directory, and rejects path indirection before privileged setup. One
nonblocking file lock serializes preparation. Exclusive writes, fsync and an atomic
directory rename publish /data/runtime/base-v1 only after validation. Failed
attempts remain under separate private directories. Complete profile files use
mode0600 and are never overwritten automatically.

The immutable profile contains configuration.toml, registry.json, the matching
array ingestion-registry.json and profile.json. Reinvocation verifies all retained
file digests and re-runs offline Rust validation; it does not contact a provider
again and explicitly reports current_rpc_verified=false. Changed/incomplete files
block rather than being silently replaced. The private single-writer volume remains
an operational prerequisite, not a guarantee against a hostile same-UID writer.

## Verification and next integration

Offline tests cover scope, identity changes, private paths, contention, atomic
publication, failure preservation, digest changes and zero-request reuse. The
mandatory container test disables networking and exercises the shipped Python
preparer plus actual Rust binary on synthetic inputs. Test fixtures are not live
provider qualification. Exact CI, source and Railway results are recorded on the
PR and existing #58 rather than inferred from this document.

Preparation creates no database session or source and issues no START. The
integrator still must register this same immutable profile with API and worker,
create/select the research session via the application, explicitly initialize the
source and test actual acquisition/control/restart. Source catch-up limits,
invalidation and publication fences remain. Full simulation and automatic virtual
settlement remain existing tasks. Real trading is not activated.
