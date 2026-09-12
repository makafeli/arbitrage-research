# Foundation verification record

Date: 12 September 2026. Version: 0.2. This record covers the project scaffold, synthetic dashboard, contracts and setup automation. It does not validate a trading strategy or a production execution system.

## Checks completed locally

| Area | Evidence | Result and limits |
|---|---|---|
| Frontend dependencies | Exact direct versions and `apps/web/package-lock.json` | Installed successfully with Node 24.19.0; reproducible npm dependency resolution is checked in |
| Frontend build | `npm run build` | TypeScript strict check and Vite production build passed; this is the synthetic interface |
| Frontend lifecycle | `npm test` | Six tests passed: network scope, independent stop acknowledgements, stale acknowledgement behavior, pause/resume, draining and fictional evidence |
| Browser test discovery | `npm run test:browser -- --list` | Browser scenarios parse and are discoverable; execution is a separate CI check |
| Contract fixtures | `python scripts/validate_specs.py` | Seven operation IDs and local references resolved; valid opportunity/command fixtures and arithmetic passed; twelve invalid evidence/quantity examples rejected |
| Setup importer | `python scripts/test_github_bootstrap.py` | Eight offline regression tests passed, including rerun preservation and uncertain-create handling |
| Project structure | `python scripts/validate_project.py` | Eight epics and 68 tickets form an acyclic graph; required metadata, local document links, JSON/TOML/Python syntax and five workspace member paths passed |
| Rust manifests | TOML/member source inspection | Five workspace members resolve to source; nineteen Rust tests are authored, awaiting compilation and execution |

The contract checker intentionally implements only the schema keywords exercised in this package. It is not a complete JSON Schema or OpenAPI conformance validator. A structural evidence flag cannot prove actual chain state or a successful transaction simulation.

## Checks pending at initial publication

The workspace has no local cargo/rustc/rustfmt installation. GitHub CI is configured to resolve and retain Cargo.lock, check formatting, run Clippy and execute Rust tests. A formatted source patch is retained for review if the first format check fails. A workflow definition is not evidence of a passing run.

A bounded attempt to install the local Chromium headless browser timed out. GitHub CI is configured to install Chromium and run the dashboard's browser scenarios, retaining screenshots and test output. Until a run is inspected, responsive rendering, keyboard/focus behavior and real-browser interactions remain unverified. The design targets accessibility; no conformance certification is claimed.

The structural project validation passed after all delivery files were present. GitHub workflow/configuration YAML also parsed successfully. The final GitHub handoff records actual remote setup and CI results separately.

## Review findings incorporated

- The dashboard's grouped Base/Solana controls now model two independent sessions and stop acknowledgements. A chain view filter cannot change either session's scope.
- The Rust reference reducer accepts a stop fence during recovery or fault without claiming recovery is complete; it preserves the blocking state.
- Stop acceptance remains PENDING until the relevant local fence applies. Unresolved attempts keep an applied stop in DRAINING.
- The importer preserves issue comments, operator workflow state and edits outside managed body blocks. Uncertain creates are rediscovered before a further write.
- Synthetic results remain clearly labelled and cannot become REALIZED paper returns.

These are bounded code/design reviews by collaborating agents. They are not an independent production security audit.

## Publication and implementation limits

The repository contains a functional synthetic dashboard and a Rust foundation. The authenticated control API, durable storage, market adapters, complete transaction simulation and paper accounting are not implemented. No RPC provider, key, funded wallet, production executor, signer or live broadcast system is configured.

Native GitHub issues/milestones/relationships are created only when their setup phase succeeds. Wiki source in `wiki/` is distinct from native Wiki publication; the owner Project has its own authentication requirements. Consult the actual setup result rather than inferring remote state from the presence of templates.
