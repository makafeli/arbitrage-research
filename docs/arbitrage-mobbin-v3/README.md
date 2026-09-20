# Arbitrage Research / Mobbin v3

An interactive, offline next-version design based on the supplied `DESIGN-mobbin.md`.

## Open

Open `Arbitrage-Research-Mobbin-v3.html` in a modern browser. It contains all markup, CSS and JavaScript and makes no external requests. Nothing is installed or connected. Light mode is the default; the moon/sun control changes theme. The theme preference is stored locally when the browser permits it. Fixture edits otherwise last only for the open page.

The editable multi-file version starts at `index.html`. Its generated stylesheet is already included. To serve it locally, run `python -m http.server 8080` from this directory and open `http://localhost:8080`.

## Explore

Use the six navigation destinations. On smaller screens, More contains the remaining destinations and preferences. Search is available through its header button, Cmd/Ctrl+K or `/`.

Opportunities includes Captured records, Decision evidence, Cost research and Exports & audit. The new List / Cards switch preserves the current filters. Both views inspect the same single synthetic captured record. No extra opportunities are invented for a fuller gallery.

Runs includes Research sessions, Paper ledger, Journal and Reservations. Session commands demonstrate local request versus acknowledgement states. These actions never operate a worker or execute trades.

Open the Design v3 / Synthetic preview control for loaded, loading, stale, error and empty-state demonstrations. Account preferences includes the sign-in design, which deliberately does not accept credentials. The separate real-trading route remains unavailable.

## Modify and rebuild

Edit `assets/mobbin.css` for the v3 visual system and `assets/app.js` for the local prototype views. `foundation.css` retains earlier component mechanics; it is not a second recommended production theme.

```sh
python tools/build_standalone.py
```

This rebuilds `assets/styles.css` and the standalone HTML. No JavaScript package install is required to use or rebuild the prototype.

## Browser checks

QA requires Python, Playwright and a Chromium installation. With those installed:

```sh
python tools/build_standalone.py
CHROMIUM_PATH=/path/to/chromium python qa/test_prototype.py
```

The script defaults to `/usr/bin/chromium`. It injects the local HTML into an isolated browser page, records network requests and writes the assertion results, layout results and screenshots. Read the generated `qa/QA-REPORT.md` and `qa/results.json` for the checks run for this delivery. The report is a delivery summary; regenerate it when changing the checks or results.

## Typography and assets

The stylesheet requests locally installed Saans, then Inter and system fallbacks. No commercial or other font binary is bundled. No external font service, screenshot service, analytics service, image dependency or API is used.

## Limits

All displayed records and states are synthetic UI fixtures inherited from the prior prototype. Retained production terminology does not verify the current behavior or state of a real deployment. Some audit, account and ledger-creation controls demonstrate a presentation contract rather than a complete production implementation. Inspect the design handoff before integrating.

This package does not change GitHub, Railway, accounts, wallets, balances, workers or deployments. It is an offline design prototype and handoff.
