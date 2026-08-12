
import http from "node:http";

const portArg = process.argv.find((value) => value.startsWith("--port="));
const port = Number(portArg?.slice("--port=".length) ?? process.env.PORT ?? "8787");
const CAPABILITY_MAP = parseCapabilityMap(process.env.AUTH_CAPABILITY_MAP_JSON ?? "", false);

async function handleWebappRequest(req, res) {
  cleanupExpiredAuthRecords();

  if (req.url === "/api/healthz") {
    sendJson(res, 200, { ok: true });
    return;
  }

  if (req.method === "POST" && req.url === "/api/auth/challenge") {
    await handleAuthChallenge(req, res);
    return;
  }

  if (req.method === "POST" && req.url === "/api/auth/login") {
    await handleAuthLogin(req, res, CAPABILITY_MAP);
    return;
  }

  if (req.method === "GET" && req.url === "/api/auth/me") {
    handleAuthMe(req, res, CAPABILITY_MAP);
    return;
  }

  if (req.method === "POST" && req.url === "/api/auth/logout") {
    handleAuthLogout(req, res);
    return;
  }

  if (req.method === "GET" && req.url === "/api/private/state") {
    const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "webapp.session.read");
    if (!session) {
      return;
    }
    sendJson(res, 200, {
      capabilities: session.capabilities,
      principal: session.principal,
      session_id: session.session_id
    });
    return;
  }

  sendJson(res, 404, { code: "NOT_FOUND", error: "not found" });
}

const server = http.createServer((req, res) => {
  handleWebappRequest(req, res).catch((error) => sendInternalError(res, error));
});

server.listen(port, "0.0.0.0", () => {
  const address = server.address();
  const boundPort = typeof address === "object" && address ? address.port : port;
  // eslint-disable-next-line no-console
  console.log(`api listening on :${boundPort}`);
});
