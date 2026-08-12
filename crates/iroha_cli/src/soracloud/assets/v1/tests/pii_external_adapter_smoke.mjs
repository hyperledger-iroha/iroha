import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";

const SERVER_PATH = __SERVER_PATH__;

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

function createAdapter() {
  const records = new Map();
  return {
    get(key) {
      return records.has(key) ? records.get(key) : null;
    },
    put(key, value) {
      records.set(key, value);
    },
    putIfAbsent(key, value) {
      if (records.has(key)) {
        return false;
      }
      records.set(key, value);
      return true;
    },
    delete(key) {
      records.delete(key);
    },
    entries(prefix) {
      return Array.from(records.entries()).filter(([key]) => key.startsWith(prefix));
    }
  };
}

async function waitForHealth(port) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
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
    15000
  );
  const init = { method, headers: { ...headers }, signal: controller.signal };
  if (body !== undefined) {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
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
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  const publicKeyHex = publicKeyHexFromSpki(
    publicKey.export({ format: "der", type: "spki" })
  );
  const port = await freePort();

  process.env.AUTH_MODE = "strict";
  process.env.NODE_ENV = "production";
  process.env.SESSION_HMAC_KEY = "0123456789abcdef0123456789abcdef0123456789abcdef";
  process.env.AUTH_SESSION_TTL_SECS = "900";
  process.env.AUTH_CHALLENGE_TTL_SECS = "120";
  process.env.AUTH_CAPABILITY_MAP_JSON = JSON.stringify({ [publicKeyHex]: ["pii.records.read"] });
  process.env.AUTH_REQUIRE_EXTERNAL_SHARED_STATE = "1";
  process.env.PUBLIC_BASE_URL = "http://127.0.0.1";

  globalThis.__soracloudSharedStateAdapter = createAdapter();
  process.argv.push(`--port=${port}`);
  await import(SERVER_PATH);
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
  assert(login.setCookie && login.setCookie.includes("session="), "login must set session cookie");

  const sessionCookie = login.setCookie.split(";")[0];
  const readableState = await jsonRequest(
    port,
    "GET",
    "/pii/api/consent/state",
    undefined,
    { cookie: sessionCookie }
  );
  assert(
    readableState.status === 200,
    `pii.records.read route should succeed with adapter-backed session: ${JSON.stringify(readableState)}`
  );

  const defaultFile = path.resolve(path.dirname(SERVER_PATH), "..", ".soracloud-shared", "auth_state.json");
  assert(!fs.existsSync(defaultFile), `external adapter path should not write fallback state file: ${defaultFile}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error?.stack ?? String(error));
    process.exit(1);
  });
