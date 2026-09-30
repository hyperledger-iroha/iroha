
import http from "node:http";

const portArg = process.argv.find((value) => value.startsWith("--port="));
const port = Number(portArg?.slice("--port=".length) ?? process.env.PORT ?? "8788");
const CAPABILITY_MAP = parseCapabilityMap(process.env.AUTH_CAPABILITY_MAP_JSON ?? "", true);

const consentState = new Map();
const retentionRuns = [];

async function handlePiiAppRequest(req, res) {
  cleanupExpiredAuthRecords();

  if (req.url === "/pii/api/healthz") {
    sendJson(res, 200, { ok: true });
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/auth/challenge") {
    await handleAuthChallenge(req, res);
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/auth/login") {
    await handleAuthLogin(req, res, CAPABILITY_MAP);
    return;
  }

  if (req.method === "GET" && req.url === "/pii/api/auth/me") {
    handleAuthMe(req, res, CAPABILITY_MAP);
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/auth/logout") {
    handleAuthLogout(req, res);
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/consent/grant") {
    try {
      const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "pii.consent.grant");
      if (!session) {
        return;
      }
      const body = await readJson(req);
      const subjectId = requireTrimmedString(body.subject_id, "subject_id");
      const scope = requireTrimmedString(body.scope, "scope");
      const key = `${subjectId}:${scope}`;
      consentState.set(key, {
        status: "granted",
        updated_at_unix_ms: Date.now(),
        updated_by: session.principal
      });
      sendJson(res, 200, { status: "granted", scope, subject_id: subjectId });
    } catch (error) {
      sendAuthError(res, 400, "INVALID_REQUEST", error.message);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/consent/revoke") {
    try {
      const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "pii.consent.revoke");
      if (!session) {
        return;
      }
      const body = await readJson(req);
      const subjectId = requireTrimmedString(body.subject_id, "subject_id");
      const scope = requireTrimmedString(body.scope, "scope");
      const key = `${subjectId}:${scope}`;
      consentState.set(key, {
        status: "revoked",
        updated_at_unix_ms: Date.now(),
        updated_by: session.principal
      });
      sendJson(res, 200, { status: "revoked", scope, subject_id: subjectId });
    } catch (error) {
      sendAuthError(res, 400, "INVALID_REQUEST", error.message);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/records/retention/sweep") {
    try {
      const session = requireAuthenticatedSession(
        req,
        res,
        CAPABILITY_MAP,
        "pii.records.retention.sweep"
      );
      if (!session) {
        return;
      }
      const body = await readJson(req);
      const jurisdiction = requireTrimmedString(body.jurisdiction, "jurisdiction");
      const policyVersion = requireTrimmedString(body.policy_version, "policy_version");
      const run = {
        jurisdiction,
        planned_actions: 0,
        policy_version: policyVersion,
        run_id: crypto.randomUUID(),
        started_at_unix_ms: Date.now(),
        started_by: session.principal
      };
      retentionRuns.push(run);
      sendJson(res, 200, run);
    } catch (error) {
      sendAuthError(res, 400, "INVALID_REQUEST", error.message);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/pii/api/records/delete") {
    try {
      const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "pii.records.delete");
      if (!session) {
        return;
      }
      const body = await readJson(req);
      const subjectId = requireTrimmedString(body.subject_id, "subject_id");
      const reason = requireTrimmedString(body.reason, "reason");
      sendJson(res, 202, {
        reason,
        status: "accepted",
        subject_id: subjectId,
        ticket_id: crypto.randomUUID(),
        requested_by: session.principal
      });
    } catch (error) {
      sendAuthError(res, 400, "INVALID_REQUEST", error.message);
    }
    return;
  }

  if (req.method === "GET" && req.url === "/pii/api/consent/state") {
    const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "pii.records.read");
    if (!session) {
      return;
    }
    sendJson(res, 200, {
      requested_by: session.principal,
      entries: Array.from(consentState.entries())
    });
    return;
  }

  if (req.method === "GET" && req.url === "/pii/api/retention/runs") {
    const session = requireAuthenticatedSession(req, res, CAPABILITY_MAP, "pii.records.read");
    if (!session) {
      return;
    }
    sendJson(res, 200, {
      requested_by: session.principal,
      runs: retentionRuns
    });
    return;
  }

  sendJson(res, 404, { code: "NOT_FOUND", error: "not found" });
}

const server = http.createServer((req, res) => {
  handlePiiAppRequest(req, res).catch((error) => sendInternalError(res, error));
});

server.listen(port, "0.0.0.0", () => {
  const address = server.address();
  const boundPort = typeof address === "object" && address ? address.port : port;
  // eslint-disable-next-line no-console
  console.log(`pii api listening on :${boundPort}`);
});
