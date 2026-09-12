# Railway configuration

Railway is the user's selected deployment platform. The checked-in TypeScript uses the current [Railway IaC interface](https://docs.railway.com/infrastructure-as-code/reference). Read [the deployment runbook](../deploy/RAILWAY.md) before planning against a Railway environment.

This file defines a dedicated `arbitrage-research` foundation with PostgreSQL, a private API and a dashboard service. It does not describe the user's unrelated Railway projects. It creates no public domain, secrets, active capture worker or funded account by itself.

No Railway deployment or authenticated infrastructure plan has been executed during authoring. The configuration must be evaluated with the installed Railway CLI against the intended project and environment; local TypeScript syntax validation alone does not validate a Railway plan.
