import { spawnSync } from "node:child_process";

const SERVER_PATH = __SERVER_PATH__;
const EXPECTED = "globalThis.__soracloudSharedStateAdapter.putIfAbsent must be a function";

const result = spawnSync(
  process.execPath,
  [
    "--input-type=module",
    "--eval",
    `
      process.env.AUTH_MODE = "strict";
      process.env.NODE_ENV = "production";
      process.env.SESSION_HMAC_KEY = "0123456789abcdef0123456789abcdef0123456789abcdef";
      process.env.AUTH_CAPABILITY_MAP_JSON = "{}";
      process.env.AUTH_REQUIRE_EXTERNAL_SHARED_STATE = "1";
      process.env.PUBLIC_BASE_URL = "http://127.0.0.1";
      globalThis.__soracloudSharedStateAdapter = {
        get: () => null,
        put: () => {},
        delete: () => {},
        entries: () => []
      };
      await import(${JSON.stringify(SERVER_PATH)});
    `
  ],
  { encoding: "utf8", timeout: 3000 }
);

if (result.error && result.error.code === "ETIMEDOUT") {
  console.error("server did not fail-closed within timeout for invalid external adapter shape");
  process.exit(1);
}
if (result.error && result.error.code !== "ETIMEDOUT") {
  console.error(result.error.stack ?? String(result.error));
  process.exit(1);
}
if (result.status === 0) {
  console.error("server unexpectedly started with malformed external state adapter");
  process.exit(1);
}
const logs = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
if (!logs.includes(EXPECTED)) {
  console.error(`missing expected invalid-adapter-shape error. logs=${logs}`);
  process.exit(1);
}
