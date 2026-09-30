import { spawn } from "node:child_process";
import crypto from "node:crypto";
import net from "node:net";

const SERVER_PATH = __SERVER_PATH__;
const STATE_FILE = __STATE_FILE__;
const REQUEST_TIMEOUT_MS = 60000;

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

function startServer(port, envOverrides) {
  const child = spawn(process.execPath, [SERVER_PATH, `--port=${port}`], {
    env: { ...process.env, ...envOverrides },
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
      const response = await fetch(`http://127.0.0.1:${port}/pii/api/healthz`);
      if (response.status === 200) {
        return;
      }
    } catch {
      // keep retrying while process boots
    }
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`server failed healthcheck on port ${port}`);
}

async function jsonRequest(port, method, route, body, headers = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(new Error(`${method} ${route} request timed out`)),
    REQUEST_TIMEOUT_MS
  );
  const init = { method, headers: { ...headers } };
  if (body !== undefined) {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  init.signal = controller.signal;
  let response;
  try {
    response = await fetch(`http://127.0.0.1:${port}${route}`, init);
  } finally {
    clearTimeout(timeout);
  }
  const text = await response.text();
  const setCookie = typeof response.headers.getSetCookie === "function"
    ? response.headers.getSetCookie()[0] ?? null
    : response.headers.get("set-cookie");
  return {
    status: response.status,
    body: text.length > 0 ? JSON.parse(text) : null,
    setCookie
  };
}

function publicKeyHexFromSpki(spkiDer) {
  return Buffer.from(spkiDer).subarray(-32).toString("hex");
}

async function main() {
  let server = null;
  try {
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  const publicKeyHex = publicKeyHexFromSpki(
    publicKey.export({ format: "der", type: "spki" })
  );
  const env = {
    AUTH_MODE: "strict",
    NODE_ENV: "development",
    SESSION_HMAC_KEY: "abcdef0123456789abcdef0123456789abcdef0123456789",
    AUTH_SESSION_TTL_SECS: "900",
    AUTH_CHALLENGE_TTL_SECS: "120",
    AUTH_CAPABILITY_MAP_JSON: JSON.stringify({ [publicKeyHex]: ["pii.records.read"] }),
    AUTH_REQUIRE_EXTERNAL_SHARED_STATE: "0",
    PUBLIC_BASE_URL: "http://127.0.0.1",
    SORACLOUD_SHARED_STATE_FILE: STATE_FILE
  };

  const port = await freePort();
  server = startServer(port, env);
  await waitForHealth(port);

  const challenge = await jsonRequest(port, "POST", "/pii/api/auth/challenge", {
    public_key: publicKeyHex
  });
  assert(challenge.status === 200, `challenge failed: ${JSON.stringify(challenge)}`);
  const signature = crypto
    .sign(null, Buffer.from(challenge.body.message, "utf8"), privateKey)
    .toString("hex");
  const login = await jsonRequest(port, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature
  });
  assert(login.status === 200, `login failed: ${JSON.stringify(login)}`);
  assert(login.setCookie && login.setCookie.includes("session="), "login must set cookie");
  const sessionCookie = login.setCookie.split(";")[0];

  const replay = await jsonRequest(port, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature
  });
  assert(replay.status === 401, `challenge replay should fail: ${JSON.stringify(replay)}`);
  assert(
    replay.body?.code === "AUTH_CHALLENGE_REPLAYED",
    `challenge replay code mismatch: ${JSON.stringify(replay.body)}`
  );

  const forbiddenGrant = await jsonRequest(
    port,
    "POST",
    "/pii/api/consent/grant",
    { subject_id: "subject-1", scope: "records.read" },
    { cookie: sessionCookie }
  );
  assert(forbiddenGrant.status === 403, `missing capability should return 403: ${JSON.stringify(forbiddenGrant)}`);
  assert(
    forbiddenGrant.body?.code === "AUTH_FORBIDDEN",
    `missing capability code mismatch: ${JSON.stringify(forbiddenGrant.body)}`
  );
  assert(
    forbiddenGrant.body?.required_capability === "pii.consent.grant",
    "forbidden payload should include required capability"
  );

  const forbiddenRevoke = await jsonRequest(
    port,
    "POST",
    "/pii/api/consent/revoke",
    { subject_id: "subject-1", scope: "records.read" },
    { cookie: sessionCookie }
  );
  assert(
    forbiddenRevoke.status === 403,
    `missing revoke capability should return 403: ${JSON.stringify(forbiddenRevoke)}`
  );
  assert(
    forbiddenRevoke.body?.required_capability === "pii.consent.revoke",
    `revoke required capability mismatch: ${JSON.stringify(forbiddenRevoke.body)}`
  );

  const forbiddenSweep = await jsonRequest(
    port,
    "POST",
    "/pii/api/records/retention/sweep",
    { jurisdiction: "us", policy_version: "v1" },
    { cookie: sessionCookie }
  );
  assert(
    forbiddenSweep.status === 403,
    `missing sweep capability should return 403: ${JSON.stringify(forbiddenSweep)}`
  );
  assert(
    forbiddenSweep.body?.required_capability === "pii.records.retention.sweep",
    `sweep required capability mismatch: ${JSON.stringify(forbiddenSweep.body)}`
  );

  const forbiddenDelete = await jsonRequest(
    port,
    "POST",
    "/pii/api/records/delete",
    { subject_id: "subject-1", reason: "request" },
    { cookie: sessionCookie }
  );
  assert(
    forbiddenDelete.status === 403,
    `missing delete capability should return 403: ${JSON.stringify(forbiddenDelete)}`
  );
  assert(
    forbiddenDelete.body?.required_capability === "pii.records.delete",
    `delete required capability mismatch: ${JSON.stringify(forbiddenDelete.body)}`
  );

  const readableState = await jsonRequest(
    port,
    "GET",
    "/pii/api/consent/state",
    undefined,
    { cookie: sessionCookie }
  );
  assert(readableState.status === 200, `pii.records.read route should succeed: ${JSON.stringify(readableState)}`);

  const readableRuns = await jsonRequest(
    port,
    "GET",
    "/pii/api/retention/runs",
    undefined,
    { cookie: sessionCookie }
  );
  assert(
    readableRuns.status === 200,
    `pii.records.read retention view should succeed: ${JSON.stringify(readableRuns)}`
  );

  const unauthenticatedDelete = await jsonRequest(port, "POST", "/pii/api/records/delete", {
    subject_id: "subject-1",
    reason: "request"
  });
  assert(
    unauthenticatedDelete.status === 401,
    `missing session should return 401: ${JSON.stringify(unauthenticatedDelete)}`
  );
  assert(
    unauthenticatedDelete.body?.code === "AUTH_REQUIRED",
    `missing session code mismatch: ${JSON.stringify(unauthenticatedDelete.body)}`
  );
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error?.stack ?? String(error));
  process.exit(1);
});
