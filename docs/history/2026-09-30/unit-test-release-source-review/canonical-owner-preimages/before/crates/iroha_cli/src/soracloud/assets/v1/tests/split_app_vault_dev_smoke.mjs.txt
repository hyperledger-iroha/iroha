import { spawn } from "node:child_process";
import net from "node:net";

const SERVER_PATH = __SERVER_PATH__;
const STATE_FILE = __STATE_FILE__;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function freePort() {
  return await new Promise((resolve, reject) => {
    const probe = net.createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      const port = typeof address === "object" && address ? address.port : 0;
      probe.close((closeError) => {
        if (closeError) {
          reject(closeError);
          return;
        }
        resolve(port);
      });
    });
  });
}

function startServer(port) {
  const child = spawn(process.execPath, [SERVER_PATH], {
    env: {
      ...process.env,
      PORT: String(port),
      SORACLOUD_HTTP_PORT: String(port),
      SORACLOUD_VAULT_DEV_STATE_FILE: STATE_FILE
    },
    stdio: ["ignore", "pipe", "pipe"]
  });
  let logs = "";
  child.stdout.on("data", (chunk) => {
    logs += chunk.toString("utf8");
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk.toString("utf8");
  });
  return { child, logs: () => logs };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForExit(child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (child.exitCode === null && Date.now() < deadline) {
    await sleep(25);
  }
}

async function stopServer(server) {
  if (!server || !server.child || server.child.exitCode !== null) {
    return;
  }
  server.child.kill("SIGTERM");
  await waitForExit(server.child, 800);
  if (server.child.exitCode === null) {
    server.child.kill("SIGKILL");
    await waitForExit(server.child, 1500);
  }
}

async function waitForHealth(port) {
  for (let attempt = 0; attempt < 160; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/health`);
      if (response.status === 200) {
        return;
      }
    } catch {
      // keep retrying while process boots
    }
    await sleep(25);
  }
  throw new Error(`server failed healthcheck on port ${port}`);
}

async function jsonRequest(port, method, route, body) {
  const init = { method, headers: {} };
  if (body !== undefined) {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  const response = await fetch(`http://127.0.0.1:${port}${route}`, init);
  const text = await response.text();
  return {
    status: response.status,
    body: text.length > 0 ? JSON.parse(text) : null
  };
}

async function main() {
  let server = null;
  try {
    const port = await freePort();
    server = startServer(port);
    await waitForHealth(port);

    const me = await jsonRequest(port, "GET", "/auth/me");
    assert(me.status === 200, `auth me failed: ${JSON.stringify(me)}`);
    assert(me.body.wallet === "dev-wallet", "vault dev shim should expose default wallet");
    assert(me.body.authenticated === true, "vault dev shim should start authenticated");

    const challenge = await jsonRequest(port, "POST", "/auth/challenge", { wallet: "dev-wallet" });
    assert(challenge.status === 200, `challenge failed: ${JSON.stringify(challenge)}`);
    assert(typeof challenge.body.challenge_id === "string", "challenge id missing");

    const preferencesPut = await jsonRequest(port, "PUT", "/v1/user/preferences", {
      preferences: { home_airport: "BNE", cabin_preference: "business" }
    });
    assert(preferencesPut.status === 200, `preferences put failed: ${JSON.stringify(preferencesPut)}`);
    assert(preferencesPut.body.preferences.home_airport === "BNE", "preferences should persist");

    const preferencesGet = await jsonRequest(port, "GET", "/v1/user/preferences");
    assert(preferencesGet.status === 200, `preferences get failed: ${JSON.stringify(preferencesGet)}`);
    assert(preferencesGet.body.preferences.cabin_preference === "business", "preferences get mismatch");

    const savedSearchPut = await jsonRequest(port, "POST", "/v1/user/saved-searches", {
      query: { origin: "BNE", destination: "HND" }
    });
    assert(savedSearchPut.status === 200, `saved search failed: ${JSON.stringify(savedSearchPut)}`);

    const savedSearches = await jsonRequest(port, "GET", "/v1/user/saved-searches");
    assert(savedSearches.status === 200, `saved searches failed: ${JSON.stringify(savedSearches)}`);
    assert(savedSearches.body.saved_searches.length === 1, "saved search should be retained");
    assert(
      savedSearches.body.saved_searches[0].query.destination === "HND",
      "saved search destination mismatch"
    );

    const logout = await jsonRequest(port, "POST", "/auth/logout");
    assert(logout.status === 200, `logout failed: ${JSON.stringify(logout)}`);
    assert(logout.body.authenticated === false, "logout should clear auth state");
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
