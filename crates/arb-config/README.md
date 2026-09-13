# arb-config

`ValidatedConfig::from_toml` loads the versioned research configuration with unknown
fields rejected at every level. `from_effective_json` applies the same validation and
canonicalization to a recorded snapshot without resolving credentials or contacting
a provider. Both parsers reject input larger than 1 MiB. The shipped `config/research.example.toml` remains
valid and inert: both networks are disabled, allowlists are empty, live budgets are
zero and signing/broadcast capabilities are unavailable.

Validated fields are private and expose read-only getters. `digest()` returns
`sha256:<64 lowercase hex characters>` over compact JSON with fixed struct field
order, effective defaults and sorted set-like collections. `effective_json()` is the
exact hashed serialization. Formatting, comments and set order do not change a hash;
changing effective settings does. This is an application-specific versioned canonical
form, not a claim of generic RFC 8785 JSON canonicalization.

Only `UNCONFIGURED` and `env:UPPERCASE_VARIABLE_NAME` secret references are allowed.
The crate neither reads the environment nor resolves credentials. It rejects inline
URLs/values, and parser errors report a line and static explanation without echoing
input. Semantic errors identify the field and required invariant. `name()` on a secret
reference returns the complete `env:NAME` reference, not a resolved value.

An enabled network requires canonical same-network asset/pool allowlists, at least two
pools/assets, a configured RPC reference and a `registry_qualification_digest` pointing
to independently qualified registry evidence. PAPER/REPLAY additionally need positive
unique trade sizes and an explicit network `starting_asset_id` from the allowlist.
Enabled Solana requires a canonical 32-byte base58 genesis identity; Base requires
chain ID 8453. These syntax/binding checks cannot prove actual chain ownership or
freshness. The adapter must match the recorded registry digest, check the returned
chain/genesis and qualify owners/programs/assets before granting readiness.

Optional additions to schema 0.1.0 are defaulted in the effective snapshot:

- `research.strategy_ids = ["cyclic-exact-in-2leg-v1"]`.
- Per network `registry_qualification_digest` and `starting_asset_id`, initially absent.
- `storage.capture_retention_days = 30`.
- `[simulation]` with `delay_scenarios_ms = [0, 50, 100, 250]`,
  `fee_buffer_bps = 1000`, and `maximum_state_age_ms = 1000`.

These are disclosed initial laboratory assumptions, not measured execution latency,
fee predictions or a permission to label candidates executable.

`freeze_session(network)` creates an immutable mode/network/starting-asset/digest
binding. Replacing it requires a STOPPED prior state and a new session record. The
crate never resets a running session or mutates its configuration in place.

```sh
cargo test -p arb-config
```

The test suite covers inert effective defaults, canonical hashes, unsafe capability
and limit rejection, fixture/unqualified registry rejection, secret redaction,
immutable session replacement and effective JSON revalidation. Runtime registry/provider qualification and persistent
API enforcement have their own integration checks.

A network can explicitly opt in to `[networks.base.chain_freshness]` or
`[networks.solana.chain_freshness]` with `version = "finalized-chain-time-v1"` and
`max_chain_age_ms` in `1..=86400000`. `chain_freshness(network)` returns this immutable
policy. An absent policy is omitted from effective JSON, preserving historical
configuration bytes and digests; absence makes no chain-time freshness claim.
The separate `config/chain-freshness.example.toml` leaves both networks disabled
and labels its per-chain limits as research assumptions rather than measured lag.
Changing the policy changes the configuration digest and requires a new session.
