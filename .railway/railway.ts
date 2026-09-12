// Repository-backed foundation; evaluate with Railway CLI, not the browser build.
// Existing unrelated Railway projects are outside this configuration's scope.
import { defineRailway, github, postgres, project, service } from "railway/iac";

export default defineRailway((ctx) => {
  const db = postgres("postgres");
  const api = service("control-api", {
    source: github("makafeli/arbitrage-research", { branch: "main" }),
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
    source: github("makafeli/arbitrage-research", { branch: "main" }),
    replicas: 1,
    healthcheck: "/healthz",
    env: {
      RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.web",
      PORT: "8080",
      ARB_API_HOST: api.env.RAILWAY_PRIVATE_DOMAIN,
    },
  });
  // Stateful chain workers are added after configuration/registry qualification;
  // each requires its own persistent volume and immutable OBSERVE session.
  return project("arbitrage-research", { resources: [db, api, web] });
});
