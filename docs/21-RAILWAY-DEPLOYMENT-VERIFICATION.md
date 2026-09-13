# Railway deployment verification

Verified on 13 September 2026 for Git commit `be061b7078e063c98f33b79384506c1b48961370`.

## Applied production foundation

- Railway project: `arbitrage-research` (`c7cea88d-24b8-45dc-9602-7c0c272f1da7`)
- Environment: `production` (`5ff4044a-b0ac-4b02-b9cd-888cd91a3fee`)
- Region: Railway EU West (`europe-west4-drams3a`)
- Public dashboard: <https://web-production-b911a.up.railway.app>
- Private services: `control-api` and `postgres`
- Deferred services: Base and Solana research workers

The applied IaC plan added exactly `postgres`, `control-api` and `web`, with zero changes or removals. PostgreSQL deployment `c75b3574-976e-4118-a4b4-ef8bf5fe498f`, API deployment `dcbc10a6-9674-47c9-b98c-d46f9fbf5e5c`, and web deployment `438d6d39-2176-4150-b98c-736ce4433d5c` each reached `SUCCESS`.

## CI/CD gate

Both GitHub-backed services follow `makafeli/arbitrage-research` branch `main` and have Railway `source.checkSuites=true`. The repository workflow `Validate project` runs on pushes to `main`. Its `specifications`, `rust`, `containers` and `web` jobs all succeeded for the deployed commit in GitHub Actions run `34752684136`.

Railway's check-suite gate holds a deployment in `WAITING` while GitHub Actions runs. A failed workflow causes Railway to skip that deployment; successful workflows allow it to proceed. The setting is represented in `.railway/railway.ts` so a later IaC apply cannot silently disable the gate.

## Runtime verification

- `GET /healthz` on the public dashboard returned `web-process-ready`.
- Login through the same-origin `/v1/auth/login` proxy returned an operator session, CSRF token and expiry.
- Authenticated `GET /v1/health` returned `{"status":"OK","version":"0.2.0"}`, proving the dashboard proxy, private API and PostgreSQL connection were available together.
- The generated operator secret is stored only as a Railway shared variable; its value is not committed.
- Only `web` has a public domain. The API and database have no public domain or TCP proxy.
- No research worker, signing service, RPC credential, funded account or live-trading capability was deployed or activated.

## Reverification

For a later release, bind the evidence to the exact Railway deployment IDs and `RAILWAY_GIT_COMMIT_SHA`. A green GitHub run establishes the build gate; it does not replace Railway deployment status and live health checks.
