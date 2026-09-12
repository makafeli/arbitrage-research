# Local development services

`compose.dev.yaml` starts PostgreSQL 17.11 on loopback port 5432 with a persistent named volume. It is a development convenience. Production deployment, TLS, backup/restore qualification and operations gates remain ARB-044 work.

Prerequisites: Docker with Compose, the pinned repository Rust toolchain, and Node 24 for the dashboard. Supply a development-only password in the process environment:

```sh
read -r -s -p 'Development database password: ' ARB_DEV_DATABASE_PASSWORD
export ARB_DEV_DATABASE_PASSWORD
docker compose -f deploy/compose.dev.yaml up -d --wait
```

Use database `arb_dev`, user `arb_dev`, host `127.0.0.1`, port `5432` when supplying `ARB_DATABASE_URL` to the [API](../apps/control-api/README.md). URL-encode special password characters in the connection URL. The API applies embedded migrations at startup. Start the [dashboard](../apps/web/README.md) separately; its local proxy connects to the API.

Stopping the database preserves its volume:

```sh
docker compose -f deploy/compose.dev.yaml stop
```

Database shutdown is not an acknowledged session STOP. Stop controlled sessions and verify their worker acknowledgements before stopping infrastructure. Standalone capture CLIs have their own process lifecycle.

For database integration tests use a **separate disposable database** and set `TEST_DATABASE_URL`. Tests migrate the database and create namespaced operator/session rows; never point them at a research or production database. Discard the test database after the run. CI provides an isolated PostgreSQL service automatically and requires these tests to run.

The image version follows the [PostgreSQL supported-version policy](https://www.postgresql.org/support/versioning/). CI service setup follows the [GitHub PostgreSQL service guide](https://docs.github.com/en/actions/tutorials/use-containerized-services/create-postgresql-service-containers). Pin and review an image digest, establish backups, set resource limits for the actual host and run recovery drills before production deployment. This Compose file has not been exercised in the authoring environment, which has no Docker daemon; PostgreSQL integration behavior is validated by CI.
