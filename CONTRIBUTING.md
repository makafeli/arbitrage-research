# Contributing

Start with an open issue from the delivery backlog and the linked requirements. The project is currently a research foundation. Keep each pull request small enough to explain its behavior and verify the acceptance criteria; a directory or stub alone does not complete a capability.

## Local development

Use the commands and supported toolchain documented in the relevant application README. Run the frontend checks when changing the dashboard, Rust formatting/lint/test checks when changing Rust, and project specification validation when changing contracts or planning. CI supplies a clean environment, but a workflow file is not a passing run.

Never commit credentials, wallet material, provider URLs containing credentials, real capture data with access restrictions, build output or local setup state. Use synthetic fixtures that are unmistakably separate from observations. The example research configuration is deliberately inert.

## Branches and review

Use `feat/ARB-xxx-description`, `fix/ARB-xxx-description` or `docs/ARB-xxx-description`. Reference the issue and PRD requirement IDs in the pull request. Explain the user-visible behavior, the relevant invariants and the checks actually run. Identify skipped checks and remaining capability gaps. Link representative fixtures or traces where they substantiate the change.

The intended baseline is a review before merging to `main`, passing relevant CI and resolved review comments. Repository rules must be configured and verified separately; this document does not enforce branch protection. For a solo-maintainer research phase, record the review and its limitations instead of inventing an independent reviewer. Future live execution requires independent review of the signing, transaction and recovery boundaries.

## Evidence and lifecycle invariants

- Use asset identities and integer minor units, never tickers or floating-point arithmetic for authoritative balances.
- Local arithmetic is CANDIDATE evidence. SIMULATED requires successful simulation of the complete intended atomic transaction with a matching plan.
- ESTIMATED_EXECUTABLE remains hypothetical and requires all documented freshness, cost, capital, atomicity and scenario prerequisites.
- PAPER never becomes REALIZED. Uncertain submission outcomes remain UNKNOWN until reconciled.
- An accepted control command is PENDING until applied by its worker. Stop cannot revoke previously emitted transactions.
- Do not add keys, broadcast endpoints or live activation to paper binaries. Live mode is a separate milestone and review gate.

## Issue maintenance

Preserve each ticket's stable `ARB-xxx` or `EPIC-xx` identifier and machine marker when editing issue bodies. The setup script uses those markers to reconcile existing issues rather than duplicate them. Keep canonical ticket specifications in `planning/backlog.json` synchronized with material scope changes.

Milestone completion depends on acceptance evidence, not elapsed dates or the number of closed tasks. Estimates are planning ranges. Findings from adapter qualification or paper observation may change later scope.
