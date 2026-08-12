import { spawnSync } from "node:child_process";

const SERVER_PATH = __SERVER_PATH__;
const EXPECTED = "SESSION_HMAC_KEY must be set to at least 32 characters in strict/production mode";

const result = spawnSync(process.execPath, [SERVER_PATH, "--port=0"], {
  env: {
    ...process.env,
    AUTH_MODE: "strict",
    NODE_ENV: "development",
    SESSION_HMAC_KEY: "too-short",
    AUTH_CAPABILITY_MAP_JSON: "{}",
    PUBLIC_BASE_URL: "http://127.0.0.1"
  },
  encoding: "utf8",
  timeout: 3000
});

if (result.error && result.error.code === "ETIMEDOUT") {
  console.error("server did not fail-closed within timeout for weak SESSION_HMAC_KEY");
  process.exit(1);
}
if (result.error && result.error.code !== "ETIMEDOUT") {
  console.error(result.error.stack ?? String(result.error));
  process.exit(1);
}
if (result.status === 0) {
  console.error("server unexpectedly started with weak SESSION_HMAC_KEY");
  process.exit(1);
}
const logs = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
if (!logs.includes(EXPECTED)) {
  console.error(`missing expected startup error. logs=${logs}`);
  process.exit(1);
}
