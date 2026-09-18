// Repository-backed foundation; evaluate with Railway CLI, not the browser build.
// Existing unrelated Railway projects are outside this configuration's scope.
// Prepared worker status: docs/BASE-SESSION-REGISTRATION.md.
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
    deploy: { preDeployCommand: ["api-entrypoint --check-profile"] },
    env: {
      RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.api",
      ARB_DATABASE_URL: db.env.DATABASE_URL,
      ARB_OPERATOR_SECRET: ctx.shared.ARB_OPERATOR_SECRET,
      ARB_PUBLIC_ORIGIN: ctx.shared.ARB_PUBLIC_ORIGIN,
      ARB_API_BIND_IP: "::",
      ARB_API_PORT: "8080",
      ARB_ALLOW_INSECURE_LOOPBACK: "false",
      ARB_CONFIG_FILES: "/app/config/research.example.toml",
      // Exact prepared profile, not an RPC URL or account credential.
      ARB_BASE_PROFILE_TOML: ctx.shared.ARB_BASE_PROFILE_TOML,
      ARB_BASE_PROFILE_REGISTRY: ctx.shared.ARB_BASE_PROFILE_REGISTRY,
      ARB_BASE_PROFILE_DIGEST: ctx.shared.ARB_BASE_PROFILE_DIGEST,
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
  // Existing source/session only. First-use source initialization is a separate explicit deployment action.
  // Preserve the owner's separately managed ARB_BASE_RPC_URL when planning changes.
  const baseCaptures = volume("base-research-captures", {
    region: "europe-west4-drams3a",
    sizeMB: 1024,
  });
  const baseWorker = service("base-research-worker", {
    source: github("makafeli/arbitrage-research", {
      branch: "main",
      checkSuites: true,
    }),
    start: "worker-entrypoint worker-launch-base --start",
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
      ARB_BASE_PROFILE_DIGEST: ctx.shared.ARB_BASE_PROFILE_DIGEST,
    },
  });
  return project("arbitrage-research", {
    resources: [db, api, web, baseWorker, baseCaptures],
  });
});
