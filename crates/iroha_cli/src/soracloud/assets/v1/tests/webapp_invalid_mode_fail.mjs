import { spawnSync } from "node:child_process";

const SERVER_PATH = __SERVER_PATH__;
const EXPECTED = "AUTH_MODE must be strict or dev, got: permissive";

const result = spawnSync(process.execPath, [SERVER_PATH, "--port=0"], {
  env: {
    ...process.env,
    AUTH_MODE: "permissive",
    NODE_ENV: "development",
    SESSION_HMAC_KEY: "0123456789abcdef0123456789abcdef0123456789abcdef",
    AUTH_CAPABILITY_MAP_JSON: "{}",
    PUBLIC_BASE_URL: "http://127.0.0.1"
  },
  encoding: "utf8",
  timeout: 3000
});

if (result.error && result.error.code === "ETIMEDOUT") {
  console.error("server did not fail-closed within timeout for invalid AUTH_MODE");
  process.exit(1);
}
if (result.error && result.error.code !== "ETIMEDOUT") {
  console.error(result.error.stack ?? String(result.error));
  process.exit(1);
}
if (result.status === 0) {
  console.error("server unexpectedly started with invalid AUTH_MODE");
  process.exit(1);
}
const logs = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
if (!logs.includes(EXPECTED)) {
  console.error(`missing expected invalid AUTH_MODE error. logs=${logs}`);
  process.exit(1);
}
