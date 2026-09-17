// Repository-backed foundation; evaluate with Railway CLI, not the browser build.
// Existing unrelated Railway projects are outside this configuration's scope.
// Prepared worker status: docs/RAILWAY-WORKER-PREPARATION.md.
import { defineRailway, github, postgres, project, service, volume } from "railway/iac";

export default defineRailway((ctx) => {
  const db = postgres("postgres");
  const api = service("control-api", {
    source: github("makafeli/arbitrage-research", {
      branch: "main",
      checkSuites: true,
    }),
    replicas: 1,
    healthcheck: "/healthz",
    healthcheckTimeout: 60,
    env: {
      RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.api",
      ARB_DATABASE_URL: db.env.DATABASE_URL,
      ARB_OPERATOR_SECRET: ctx.shared.ARB_OPERATOR_SECRET,
      ARB_PUBLIC_ORIGIN: ctx.shared.ARB_PUBLIC_ORIGIN,
      ARB_API_BIND_IP: "::",
      ARB_API_PORT: "8080",
      ARB_ALLOW_INSECURE_LOOPBACK: "false",
      ARB_CONFIG_FILES: "/app/config/research.example.toml",
    },
  });
  const web = service("web", {
    source: github("makafeli/arbitrage-research", {
      branch: "main",
      checkSuites: true,
    }),
    replicas: 1,
    healthcheck: "/healthz",
    env: {
      RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.web",
      PORT: "8080",
      ARB_API_HOST: api.env.RAILWAY_PRIVATE_DOMAIN,
    },
  });
  // One-shot storage and read-only metadata inspection; never starts research.
  // Runtime activation requires a separately reviewed configuration/session.
  const baseCaptures = volume("base-research-captures", {
    region: "europe-west4-drams3a",
    sizeMB: 1024,
  });
  const baseWorker = service("base-research-worker", {
    source: github("makafeli/arbitrage-research", {
      branch: "main",
      checkSuites: true,
    }),
    start: "worker-entrypoint worker-readiness-check",
    replicas: { "europe-west4-drams3a": 1 },
    build: { dockerfilePath: "deploy/Dockerfile.worker" },
    deploy: {
      restartPolicyType: "NEVER",
      // Enforced independently of the provider's native Wait for CI toggle.
      preDeployCommand: ["python3 -I /usr/local/lib/arb/worker_ci_gate.py"],
    },
    volumeMounts: { "/data": baseCaptures },
    env: {
      ARB_DATABASE_URL: db.env.DATABASE_URL,
      ARB_INGEST_DATABASE_URL: db.env.DATABASE_URL,
      ARB_OPERATOR_ID: "operator",
      ARB_INGEST_OPERATOR_ID: "operator",
      ARB_RPC_MIN_INTERVAL_MS: "75",
    },
  });
  // Keep prepared resources in the full graph, so later plans do not omit them.
  return project("arbitrage-research", {
    resources: [db, api, web, baseWorker, baseCaptures],
  });
});
