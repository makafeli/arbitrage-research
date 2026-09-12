# Offline lifecycle demonstration

Recorded-market replay is not implemented. With no argument, this binary exits
with status **2** to make that limitation explicit. One non-trading example is
available:

```sh
cargo run -p replay -- --lifecycle-demo
```

This fixed synthetic example starts a REPLAY-mode in-memory session, records a
fictional unresolved attempt, requests STOP, applies the local fence, shows
`DRAINING` and then resolves the fictional attempt to reach `STOPPED`. It has no
dataset, market prices, PnL, network I/O or persistence. Its output is not a paper
trading or profitability result.

The actual replay engine remains planned: manifest and digest verification,
deterministic clocks, ordered recorded events, complete historical coverage,
seeded scenarios, virtual inventory and evidence-backed exports.
