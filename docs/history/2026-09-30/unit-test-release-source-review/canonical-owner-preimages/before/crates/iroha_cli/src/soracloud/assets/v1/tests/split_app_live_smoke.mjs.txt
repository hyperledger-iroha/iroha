import { spawn } from "node:child_process";

const SERVER_PATH = __SERVER_PATH__;
const SHARED_CACHE_DIR = __SHARED_CACHE_DIR__;
const SEARCH_SESSIONS_DIR = __SEARCH_SESSIONS_DIR__;
const COLLECTOR_STATE_DIR = __COLLECTOR_STATE_DIR__;
const RUNTIME_CACHE_DIR = __RUNTIME_CACHE_DIR__;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function startServer() {
  const child = spawn(process.execPath, [SERVER_PATH], {
    env: {
      ...process.env,
      PORT: "0",
      SORACLOUD_HTTP_PORT: "0",
      SORACLOUD_LEASE_VOLUME_SHARED_CACHE_DIR: SHARED_CACHE_DIR,
      SORACLOUD_LEASE_VOLUME_SEARCH_SESSIONS_DIR: SEARCH_SESSIONS_DIR,
      SORACLOUD_LEASE_VOLUME_COLLECTOR_STATE_DIR: COLLECTOR_STATE_DIR,
      SORACLOUD_LEASE_VOLUME_RUNTIME_CACHE_DIR: RUNTIME_CACHE_DIR
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
    const match = server.logs().match(/listening on (\d+)/);
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
      const response = await fetch(`http://127.0.0.1:${port}/health`, {
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
    await sleep(25);
  }
  throw new Error(`server failed healthcheck on port ${port}: ${server.logs()}`);
}

async function jsonRequest(port, method, route, body) {
  const init = { method, headers: { connection: "close" } };
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

async function textRequest(port, route) {
  const response = await fetch(`http://127.0.0.1:${port}${route}`, {
    headers: { connection: "close" }
  });
  return {
    status: response.status,
    body: await response.text()
  };
}

async function main() {
  let server = null;
  try {
    server = startServer();
    const port = await waitForListeningPort(server);
    await waitForHealth(server, port);

    const search = await jsonRequest(port, "POST", "/search", { origin: "SYD" });
    assert(search.status === 202, `search failed: ${JSON.stringify(search)}`);
    assert(search.body.result.query.origin.value === "SYD", "search should retain trusted request fields");
    assert(search.body.result.query.destination.value === null, "search should not fabricate destination values");

    const events = await textRequest(port, `/search/${search.body.search_id}/events`);
    assert(events.status === 200, `sse failed: ${JSON.stringify(events)}`);
    assert(events.body.includes("event: snapshot"), "sse must emit snapshot event");
    assert(events.body.includes("event: done"), "sse must emit done event");

    const airports = await jsonRequest(port, "GET", "/airports/search?q=tok");
    assert(airports.status === 200, `airports failed: ${JSON.stringify(airports)}`);

    const filters = await jsonRequest(port, "GET", "/filters/metadata");
    assert(filters.status === 200, `filters failed: ${JSON.stringify(filters)}`);

    const luxury = await jsonRequest(port, "GET", "/luxury/catalog");
    assert(luxury.status === 200, `luxury failed: ${JSON.stringify(luxury)}`);

    const links = await jsonRequest(port, "POST", "/links/resolve", { offer_id: "offer-1" });
    assert(links.status === 200, `links failed: ${JSON.stringify(links)}`);
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
