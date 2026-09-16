# Web interface: Paper trading and Real trading

Open the existing dashboard, activate your private account with the one-time link
supplied to the owner, and sign in with your own email and password. The public
Demo/Connected choice has been removed. See [account access](ACCOUNT-ACCESS.md)
for activation, password changes and private recovery.

**Paper trading** exposes retained research sessions, decisions and hypothetical
accounting with their actual evidence labels. It does not invent trades, fills,
profits or a running market feed. OBSERVE and REPLAY retain their original record
modes rather than being falsely relabelled PAPER. **Real trading** is a separate
visibly unavailable workspace; this build cannot submit real orders.

The owner can still run the existing explicit GitHub workflow **Recorded Base
observation slice** on `main` for a bounded real-data test. Both `offline` and
`recorded` must run and pass. Its disposable API/database are not the Railway
production database, and results are not automatically transferred to the hosted
workspace. It uses the existing provider secret and consumes the existing request
allowance. Run one experiment at a time. Do not repeat failed provider requests
without diagnosis or treat negative candidates as a failed profit target.

Its artifact `recorded-base-slice-evidence` contains `result.json`, `decisions.json`,
`replay.json`, control receipts, capture/coverage records and screenshots. A passing
result has `RECORDED_ROUTE_CONTROL_REPLAY_VERIFIED`, matched decisions, zero replay
network requests and `execution_authorized: false`. A screenshot is not a service
left running after CI. Artifacts expire after 30 days; preserve required evidence.
The recorded browser harness authenticates its disposable automation context via
the real backend; it no longer drives the removed operator-secret web form.

For local development use Node24, `cd apps/web`, `npm ci`, `npm run dev`. The
interface now opens on sign-in, not sample data. The existing API and PostgreSQL
must be configured for account-backed interactive use. Do not seed the production
invitation into a different origin or commit private access links.

Continuous data qualification, derived-state rollback, dashboard worker control,
automatic paper settlement and reviewed live execution remain separate original
ticket gates. This login delivery does not close them or activate production
workers. Authentication, account persistence and actual hosted deployment state
must be distinguished from historical bounded capture evidence.
