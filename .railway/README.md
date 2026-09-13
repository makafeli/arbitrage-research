# Railway configuration

Railway is the user's selected deployment platform. The checked-in TypeScript uses the current [Railway IaC interface](https://docs.railway.com/infrastructure-as-code/reference). Read [the deployment runbook](../deploy/RAILWAY.md) before planning against a Railway environment.

This file defines a dedicated `arbitrage-research` foundation with PostgreSQL, a private API and a dashboard service. It does not describe the user's unrelated Railway projects. It creates no public domain, secrets, active capture worker or funded account by itself.

Both GitHub-backed services enable Railway's `checkSuites` gate. A push to `main` therefore remains in Railway's `WAITING` state until the repository's push-triggered GitHub Actions workflows succeed; a failed workflow skips the deployment. Keep this setting enabled on every GitHub-backed production service.

Use Node.js 22 or newer and Railway CLI 5.42.1 or newer. Install the pinned authoring dependency before planning or applying the graph:

```sh
npm ci --prefix .railway
railway config plan
```

The configuration must be evaluated with the installed Railway CLI against the intended project and environment; local TypeScript syntax validation alone does not validate a Railway plan. Current deployment evidence is recorded separately in [the Railway deployment verification](../docs/21-RAILWAY-DEPLOYMENT-VERIFICATION.md).
