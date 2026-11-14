import { spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";

const SERVER_PATH = __SERVER_PATH__;
const APP_DATA_DIR = __APP_DATA_DIR__;
// Hang guard only. A start is judged by what the process does (it exits or it
// answers /health), never by how long a busy host takes to schedule it.
const STARTUP_HANG_GUARD_MS = 120_000;

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

function startServer(port, serviceVersion) {
  const env = {
    ...process.env,
    PORT: String(port),
    SORACLOUD_HTTP_PORT: String(port),
    SORACLOUD_LEASE_VOLUME_APP_DATA_DIR: APP_DATA_DIR
  };
  delete env.SORACLOUD_SERVICE_VERSION;
  if (serviceVersion !== undefined) {
    env.SORACLOUD_SERVICE_VERSION = serviceVersion;
  }
  const child = spawn(process.execPath, [SERVER_PATH], {
    env,
    stdio: ["ignore", "pipe", "pipe"]
  });
  let logs = "";
  child.stdout.on("data", (chunk) => {
    logs += chunk.toString("utf8");
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk.toString("utf8");
  });
  // "close" follows the process exit and the end of both output streams, so
  // the captured logs are complete once it has fired.
  let closed = false;
  child.once("close", () => {
    closed = true;
  });
  return { child, logs: () => logs, closed: () => closed };
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

// Observe how the started process settles its own startup: "exited" once it
// has terminated and its output is drained, "serving" once /health answers.
// A process that has done neither is still starting, so no fixed wait may
// stand in for either verdict; "undecided" reports only the hang guard.
async function startupOutcome(server, port) {
  const deadline = Date.now() + STARTUP_HANG_GUARD_MS;
  while (Date.now() < deadline) {
    if (server.closed()) {
      return "exited";
    }
    try {
      const response = await fetch(`http://127.0.0.1:${port}/health`);
      if (response.status === 200) {
        return "serving";
      }
    } catch {
      // not listening: the process is still booting or is about to exit
    }
    await sleep(25);
  }
  return "undecided";
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

async function assertInvalidServiceVersionFails(serviceVersion) {
  const port = await freePort();
  const server = startServer(port, serviceVersion);
  try {
    const outcome = await startupOutcome(server, port);
    assert(
      outcome !== "serving",
      "server accepted a missing service version"
    );
    assert(
      outcome === "exited",
      `server neither refused nor served a missing service version: ${server.logs()}`
    );
    assert(
      server.child.exitCode !== 0,
      "server exited successfully without a service version"
    );
    assert(
      server.logs().includes("SORACLOUD_SERVICE_VERSION is required"),
      "startup error must name the required service-version input"
    );
  } finally {
    await stopServer(server);
  }
}

async function main() {
  let server = null;
  try {
    await assertInvalidServiceVersionFails(undefined);
    await assertInvalidServiceVersionFails("");
    const port = await freePort();
    server = startServer(port, "1.0.0");
    const outcome = await startupOutcome(server, port);
    assert(
      outcome === "serving",
      `server failed to start on port ${port} (${outcome}): ${server.logs()}`
    );

    const health = await jsonRequest(port, "GET", "/health");
    assert(health.status === 200, `health failed: ${JSON.stringify(health)}`);
    assert(
      health.body.service_version === "1.0.0",
      "health must expose the deployed service version"
    );
    assert(
      health.body.lease_volumes.app_data === APP_DATA_DIR,
      "health must expose app_data lease path"
    );

    const echo = await jsonRequest(port, "POST", "/echo", { message: "hello" });
    assert(echo.status === 200, `echo failed: ${JSON.stringify(echo)}`);
    assert(echo.body.body.message === "hello", "echo body must round-trip request JSON");
    assert(fs.existsSync(`${APP_DATA_DIR}/last-echo.json`), "echo must persist its last payload");
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
