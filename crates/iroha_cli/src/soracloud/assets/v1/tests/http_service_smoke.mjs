import { spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";

const SERVER_PATH = __SERVER_PATH__;
const APP_DATA_DIR = __APP_DATA_DIR__;

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

async function assertInvalidServiceVersionFails(serviceVersion) {
  const port = await freePort();
  const server = startServer(port, serviceVersion);
  try {
    await waitForExit(server.child, 800);
    assert(
      server.child.exitCode !== null,
      "server accepted a missing service version"
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
    await waitForHealth(port);

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
