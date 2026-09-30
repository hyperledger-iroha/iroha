import { spawnSync } from "node:child_process";

const SERVER_PATH = __SERVER_PATH__;
const EXPECTED =
  "AUTH_REQUIRE_EXTERNAL_SHARED_STATE is enabled but globalThis.__soracloudSharedStateAdapter is not configured";

const result = spawnSync(process.execPath, [SERVER_PATH, "--port=0"], {
  env: {
    ...process.env,
    AUTH_MODE: "strict",
    NODE_ENV: "production",
    SESSION_HMAC_KEY: "0123456789abcdef0123456789abcdef0123456789abcdef",
    AUTH_CAPABILITY_MAP_JSON: "{\"1111111111111111111111111111111111111111111111111111111111111111\":[\"pii.records.read\"]}",
    AUTH_REQUIRE_EXTERNAL_SHARED_STATE: "1",
    PUBLIC_BASE_URL: "http://127.0.0.1"
  },
  encoding: "utf8",
  timeout: 3000
});

if (result.error && result.error.code === "ETIMEDOUT") {
  console.error("pii-app server did not fail-closed within timeout for missing external state adapter");
  process.exit(1);
}
if (result.error && result.error.code !== "ETIMEDOUT") {
  console.error(result.error.stack ?? String(result.error));
  process.exit(1);
}
if (result.status === 0) {
  console.error("pii-app server unexpectedly started without required external shared state adapter");
  process.exit(1);
}
const logs = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
if (!logs.includes(EXPECTED)) {
  console.error(`missing expected external-state requirement error. logs=${logs}`);
  process.exit(1);
}
