import { spawn } from "node:child_process";
import crypto from "node:crypto";

const SERVER_PATH = __SERVER_PATH__;
const FORWARDED_PROTO = "https";
const FORWARDED_HOST = "pii-auth.example.internal";
const REQUEST_TIMEOUT_MS = 60000;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function startServer(envOverrides) {
  const child = spawn(process.execPath, [SERVER_PATH, "--port=0"], {
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

function childExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
}

async function waitForExit(child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (!childExited(child) && Date.now() < deadline) {
    await sleep(25);
  }
}

async function stopServer(server) {
  if (!server || !server.child || childExited(server.child)) {
    return;
  }
  server.child.kill("SIGTERM");
  await waitForExit(server.child, 800);
  if (!childExited(server.child)) {
    server.child.kill("SIGKILL");
    await waitForExit(server.child, 1500);
  }
}

async function waitForListeningPort(server) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const match = server.logs().match(/pii api listening on :(\d+)/);
    if (match) {
      return Number(match[1]);
    }
    if (childExited(server.child)) {
      throw new Error(`server exited before listen: ${server.logs()}`);
    }
    await sleep(25);
  }
  throw new Error(`server did not report a listening port: ${server.logs()}`);
}

async function waitForHealth(server, port) {
  for (let attempt = 0; attempt < 400; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/pii/api/healthz`, {
        headers: { connection: "close" }
      });
      if (response.status === 200) {
        return;
      }
    } catch {
      // keep retrying while process boots
    }
    if (childExited(server.child)) {
      throw new Error(`server exited before healthcheck: ${server.logs()}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`server failed healthcheck on port ${port}: ${server.logs()}`);
}

async function jsonRequest(port, method, route, body, headers = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(new Error(`${method} ${route} request timed out`)),
    REQUEST_TIMEOUT_MS
  );
  const init = { method, headers: { ...headers, connection: "close" }, signal: controller.signal };
  if (body !== undefined) {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  let response;
  try {
    response = await fetch(`http://127.0.0.1:${port}${route}`, init);
  } catch (error) {
    throw new Error(
      `${method} ${route} request failed on port ${port}: ${error?.stack ?? String(error)}`
    );
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
      SESSION_HMAC_KEY: "0123456789abcdef0123456789abcdef0123456789abcdef",
      AUTH_SESSION_TTL_SECS: "900",
      AUTH_CHALLENGE_TTL_SECS: "120",
      AUTH_CAPABILITY_MAP_JSON: JSON.stringify({ [publicKeyHex]: ["pii.records.read"] }),
      AUTH_REQUIRE_EXTERNAL_SHARED_STATE: "0",
      PUBLIC_BASE_URL: ""
    };

    server = startServer(env);
    const port = await waitForListeningPort(server);
    await waitForHealth(server, port);

    const challenge = await jsonRequest(
      port,
      "POST",
      "/pii/api/auth/challenge",
      { public_key: publicKeyHex },
      {
        "x-forwarded-proto": FORWARDED_PROTO,
        "x-forwarded-host": FORWARDED_HOST
      }
    );
    assert(challenge.status === 200, `challenge failed: ${JSON.stringify(challenge)}`);

    const signature = crypto
      .sign(null, Buffer.from(challenge.body.message, "utf8"), privateKey)
      .toString("hex");

    const mismatch = await jsonRequest(port, "POST", "/pii/api/auth/login", {
      public_key: publicKeyHex,
      challenge_id: challenge.body.challenge_id,
      signature
    });
    assert(mismatch.status === 401, `origin mismatch should fail: ${JSON.stringify(mismatch)}`);
    assert(
      mismatch.body?.code === "AUTH_ORIGIN_MISMATCH",
      `origin mismatch code mismatch: ${JSON.stringify(mismatch.body)}`
    );

    const aligned = await jsonRequest(
      port,
      "POST",
      "/pii/api/auth/login",
      {
        public_key: publicKeyHex,
        challenge_id: challenge.body.challenge_id,
        signature
      },
      {
        "x-forwarded-proto": FORWARDED_PROTO,
        "x-forwarded-host": FORWARDED_HOST
      }
    );
    assert(
      aligned.status === 200,
      `login with matching origin should succeed: ${JSON.stringify(aligned)}`
    );
    assert(aligned.setCookie && aligned.setCookie.includes("Secure"), "matching origin login should set Secure cookie");
    assert(aligned.setCookie.includes("HttpOnly"), "session cookie must be HttpOnly");
    assert(aligned.setCookie.includes("SameSite=Strict"), "session cookie must be SameSite=Strict");
    const sessionCookie = aligned.setCookie.split(";")[0];

    const matchingOriginState = await jsonRequest(
      port,
      "GET",
      "/pii/api/consent/state",
      undefined,
      {
        cookie: sessionCookie,
        "x-forwarded-proto": FORWARDED_PROTO,
        "x-forwarded-host": FORWARDED_HOST
      }
    );
    assert(
      matchingOriginState.status === 200,
      `pii route should succeed when session/request origin match: ${JSON.stringify(matchingOriginState)}`
    );

    const mismatchedOriginState = await jsonRequest(
      port,
      "GET",
      "/pii/api/consent/state",
      undefined,
      { cookie: sessionCookie }
    );
    assert(
      mismatchedOriginState.status === 401,
      `session origin mismatch should fail on authenticated request: ${JSON.stringify(mismatchedOriginState)}`
    );
    assert(
      mismatchedOriginState.body?.code === "AUTH_REQUIRED",
      `session origin mismatch should surface AUTH_REQUIRED: ${JSON.stringify(mismatchedOriginState.body)}`
    );
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error?.stack ?? String(error));
  process.exit(1);
});
