import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";

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

function startReplica(port, envOverrides) {
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

async function stopReplica(replica) {
  if (!replica || !replica.child || replica.child.exitCode !== null) {
    return;
  }
  replica.child.kill("SIGTERM");
  await waitForExit(replica.child, 800);
  if (replica.child.exitCode === null) {
    replica.child.kill("SIGKILL");
    await waitForExit(replica.child, 1500);
  }
}

async function waitForHealth(port, route) {
  for (let attempt = 0; attempt < 160; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}${route}`);
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
  let replicaA = null;
  let replicaB = null;
  try {
  fs.mkdirSync(path.dirname(STATE_FILE), { recursive: true });
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

  const portA = await freePort();
  replicaA = startReplica(portA, env);
  await waitForHealth(portA, "/pii/api/healthz");

  const challenge = await jsonRequest(portA, "POST", "/pii/api/auth/challenge", {
    public_key: publicKeyHex
  });
  assert(challenge.status === 200, `challenge failed: ${JSON.stringify(challenge)}`);
  const expectedChallengeMessage = [
    challenge.body.auth_message_version,
    `challenge_id=${challenge.body.challenge_id}`,
    `public_key=${challenge.body.public_key}`,
    `nonce=${challenge.body.nonce}`,
    `issued_at_unix_ms=${challenge.body.issued_at_unix_ms}`,
    `expires_at_unix_ms=${challenge.body.expires_at_unix_ms}`,
    "origin=http://127.0.0.1"
  ].join("\n");
  assert(
    challenge.body.message === expectedChallengeMessage,
    `challenge message must be canonical and deterministic: ${JSON.stringify(challenge.body)}`
  );

  const { publicKey: otherPublicKey } = crypto.generateKeyPairSync("ed25519");
  const otherPublicKeyHex = publicKeyHexFromSpki(
    otherPublicKey.export({ format: "der", type: "spki" })
  );
  const principalMismatch = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: otherPublicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature: "00".repeat(64)
  });
  assert(
    principalMismatch.status === 401,
    `challenge principal mismatch should fail: ${JSON.stringify(principalMismatch)}`
  );
  assert(
    principalMismatch.body?.code === "AUTH_CHALLENGE_PRINCIPAL_MISMATCH",
    `challenge principal mismatch code mismatch: ${JSON.stringify(principalMismatch.body)}`
  );

  const malformed = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature: "00".repeat(64)
  });
  assert(malformed.status === 401, `malformed signature should fail: ${JSON.stringify(malformed)}`);
  assert(
    malformed.body?.code === "AUTH_SIGNATURE_INVALID",
    `malformed signature code mismatch: ${JSON.stringify(malformed.body)}`
  );

  const unknown = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: crypto.randomUUID(),
    signature: "00".repeat(64)
  });
  assert(unknown.status === 401, `unknown challenge should fail: ${JSON.stringify(unknown)}`);
  assert(
    unknown.body?.code === "AUTH_CHALLENGE_NOT_FOUND",
    `unknown challenge code mismatch: ${JSON.stringify(unknown.body)}`
  );

  const signature = crypto
    .sign(null, Buffer.from(challenge.body.message, "utf8"), privateKey)
    .toString("hex");
  const login = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature
  });
  assert(login.status === 200, `login failed: ${JSON.stringify(login)}`);
  assert(login.setCookie && login.setCookie.includes("session="), "login must set session cookie");
  assert(login.setCookie.includes("HttpOnly"), "session cookie must be HttpOnly");
  assert(login.setCookie.includes("SameSite=Strict"), "session cookie must be SameSite=Strict");
  const sessionCookie = login.setCookie.split(";")[0];

  const me = await jsonRequest(portA, "GET", "/pii/api/auth/me", undefined, {
    cookie: sessionCookie
  });
  assert(me.status === 200, `auth me should succeed on replica A: ${JSON.stringify(me)}`);
  assert(me.body?.principal === publicKeyHex, "auth me principal mismatch");

  const replay = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: challenge.body.challenge_id,
    signature
  });
  assert(replay.status === 401, `challenge replay should fail: ${JSON.stringify(replay)}`);
  assert(
    replay.body?.code === "AUTH_CHALLENGE_REPLAYED",
    `challenge replay code mismatch: ${JSON.stringify(replay.body)}`
  );

  const expiringChallenge = await jsonRequest(portA, "POST", "/pii/api/auth/challenge", {
    public_key: publicKeyHex
  });
  assert(expiringChallenge.status === 200, "expiring challenge should be issued");
  const expiringSnapshot = JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
  const challengeKey = `/state/auth/challenges/${expiringChallenge.body.challenge_id}`;
  expiringSnapshot.records[challengeKey].expires_at_unix_ms = Date.now() - 1;
  fs.writeFileSync(STATE_FILE, JSON.stringify(expiringSnapshot));
  const expiringSignature = crypto
    .sign(null, Buffer.from(expiringChallenge.body.message, "utf8"), privateKey)
    .toString("hex");
  const expired = await jsonRequest(portA, "POST", "/pii/api/auth/login", {
    public_key: publicKeyHex,
    challenge_id: expiringChallenge.body.challenge_id,
    signature: expiringSignature
  });
  assert(expired.status === 401, `expired challenge should be rejected: ${JSON.stringify(expired)}`);
  assert(
    expired.body?.code === "AUTH_CHALLENGE_EXPIRED",
    `unexpected expired challenge code: ${JSON.stringify(expired.body)}`
  );

  const stateOnReplicaA = await jsonRequest(
    portA,
    "GET",
    "/pii/api/consent/state",
    undefined,
    { cookie: sessionCookie }
  );
  assert(
    stateOnReplicaA.status === 200,
    `authorized read should succeed on replica A: ${JSON.stringify(stateOnReplicaA)}`
  );

  const stateSnapshot = JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
  const hasSessionRecord = Object.keys(stateSnapshot.records).some((key) =>
    key.startsWith("/state/auth/sessions/")
  );
  assert(hasSessionRecord, "shared auth state must persist session records");

  const portB = await freePort();
  replicaB = startReplica(portB, env);
  await waitForHealth(portB, "/pii/api/healthz");
  const sharedSession = await jsonRequest(
    portB,
    "GET",
    "/pii/api/consent/state",
    undefined,
    { cookie: sessionCookie }
  );
  assert(
    sharedSession.status === 200,
    `replica session continuation should succeed: ${JSON.stringify(sharedSession)}`
  );
  assert(sharedSession.body?.requested_by === publicKeyHex, "shared session principal mismatch");

  const logout = await jsonRequest(portB, "POST", "/pii/api/auth/logout", undefined, {
    cookie: sessionCookie
  });
  assert(logout.status === 204, `logout failed: ${JSON.stringify(logout)}`);
  assert(logout.setCookie && logout.setCookie.includes("Max-Age=0"), "logout must clear cookie");
  assert(logout.setCookie.includes("HttpOnly"), "logout cookie must stay HttpOnly");
  assert(logout.setCookie.includes("SameSite=Strict"), "logout cookie must be SameSite=Strict");

  const postLogout = await jsonRequest(
    portA,
    "GET",
    "/pii/api/consent/state",
    undefined,
    { cookie: sessionCookie }
  );
  assert(
    postLogout.status === 401,
    `session should be invalidated across replicas after logout: ${JSON.stringify(postLogout)}`
  );
  assert(
    postLogout.body?.code === "AUTH_REQUIRED",
    `post-logout code mismatch: ${JSON.stringify(postLogout.body)}`
  );
  } finally {
    await stopReplica(replicaB);
    await stopReplica(replicaA);
  }
}

main().catch((error) => {
  console.error(error?.stack ?? String(error));
  process.exit(1);
});
