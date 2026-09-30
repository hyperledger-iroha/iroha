import { spawn } from "node:child_process";
import crypto from "node:crypto";
import net from "node:net";

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

async function waitForListeningPort(server) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const match = server.logs().match(/api listening on :(\d+)/);
    if (match) {
      return Number(match[1]);
    }
    if (server.child.exitCode !== null) {
      throw new Error(`server exited before listen: ${server.logs()}`);
    }
    await sleep(25);
  }
  throw new Error(`server did not report a listening port: ${server.logs()}`);
}

async function waitForHealth(port) {
  for (let attempt = 0; attempt < 160; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/api/healthz`);
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
      AUTH_CAPABILITY_MAP_JSON: "{}",
      AUTH_REQUIRE_EXTERNAL_SHARED_STATE: "0",
      PUBLIC_BASE_URL: "http://127.0.0.1"
    };

    server = startServer(env);
    const port = await waitForListeningPort(server);
    await waitForHealth(port);

    const challenge = await jsonRequest(port, "POST", "/api/auth/challenge", {
      public_key: publicKeyHex
    });
    assert(challenge.status === 200, `challenge failed: ${JSON.stringify(challenge)}`);
    const signature = crypto
      .sign(null, Buffer.from(challenge.body.message, "utf8"), privateKey)
      .toString("hex");
    const login = await jsonRequest(port, "POST", "/api/auth/login", {
      public_key: publicKeyHex,
      challenge_id: challenge.body.challenge_id,
      signature
    });
    assert(login.status === 200, `login failed: ${JSON.stringify(login)}`);
    assert(login.setCookie && login.setCookie.includes("session="), "login must set session cookie");
    const sessionCookie = login.setCookie.split(";")[0];

    const me = await jsonRequest(port, "GET", "/api/auth/me", undefined, {
      cookie: sessionCookie
    });
    assert(me.status === 200, `auth me should still succeed: ${JSON.stringify(me)}`);

    const privateState = await jsonRequest(port, "GET", "/api/private/state", undefined, {
      cookie: sessionCookie
    });
    assert(
      privateState.status === 403,
      `private route must fail when capability map is empty: ${JSON.stringify(privateState)}`
    );
    assert(
      privateState.body?.code === "AUTH_CAPABILITY_MAP_REQUIRED",
      `capability-map-required code mismatch: ${JSON.stringify(privateState.body)}`
    );
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error?.stack ?? String(error));
  process.exit(1);
});
