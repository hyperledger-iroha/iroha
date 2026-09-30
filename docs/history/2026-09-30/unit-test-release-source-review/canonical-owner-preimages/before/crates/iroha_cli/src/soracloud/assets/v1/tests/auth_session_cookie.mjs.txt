
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const forwardedReq = {
  headers: {
    "x-forwarded-proto": "https,http",
    "x-forwarded-host": "example.test, proxy.local",
    host: "fallback.test"
  }
};
assert(requestOrigin(forwardedReq) === "https://example.test", "forwarded origin should use first forwarded values");
assert(shouldUseSecureCookie(forwardedReq) === true, "forwarded https should require secure cookies");

const plainReq = { headers: { host: "fallback.test" } };
assert(requestOrigin(plainReq) === "http://fallback.test", "plain host should fall back to http origin");
assert(shouldUseSecureCookie(plainReq) === false, "plain http request should not require secure cookies");

const parsedCookies = parseCookies("ignored; session=sess%2Etoken; theme=dark=mode; empty=");
assert(parsedCookies.session === "sess.token", "session cookie should be decoded");
assert(parsedCookies.theme === "dark=mode", "cookie values may contain equals signs");
assert(parsedCookies.empty === "", "empty cookie values should be retained");

const sessionId = "session-1";
const token = signSessionToken(sessionId);
assert(verifySessionToken(token) === sessionId, "signed session token should verify");
assert(verifySessionToken(`${token}bad`) === null, "tampered session token should be rejected");
assert(verifySessionToken("missing-dot") === null, "malformed session token should be rejected");

const setCookie = buildSetCookieHeader(forwardedReq, token);
assert(setCookie.includes("session="), "set-cookie should include the session token");
assert(setCookie.includes("HttpOnly"), "set-cookie should be HttpOnly");
assert(setCookie.includes("SameSite=Strict"), "set-cookie should be SameSite=Strict");
assert(setCookie.includes("Secure"), "forwarded https set-cookie should be Secure");
const clearCookie = buildClearCookieHeader(forwardedReq);
assert(clearCookie.includes("Max-Age=0"), "clear-cookie should expire the session");
assert(clearCookie.includes("Secure"), "forwarded https clear-cookie should be Secure");

statePut(sessionStateKey(sessionId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  session_id: sessionId,
  principal: "principal-1",
  capabilities: ["pii.records.read"],
  expires_at_unix_ms: Date.now() + 60000,
  origin: "https://example.test"
});
const session = getSessionFromRequest({
  headers: {
    cookie: `session=${encodeURIComponent(token)}`,
    "x-forwarded-proto": "https",
    "x-forwarded-host": "example.test"
  }
});
assert(session?.session_id === sessionId, "matching session cookie and origin should load the session");
assert(
  getSessionFromRequest({ headers: { cookie: `session=${encodeURIComponent(token)}`, host: "fallback.test" } }) === null,
  "origin mismatch should reject an otherwise valid session token"
);

const expiredSessionId = "expired-session";
const expiredToken = signSessionToken(expiredSessionId);
statePut(sessionStateKey(expiredSessionId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  session_id: expiredSessionId,
  principal: "principal-1",
  capabilities: [],
  expires_at_unix_ms: Date.now() - 1,
  origin: ""
});
assert(
  getSessionFromRequest({ headers: { cookie: `session=${encodeURIComponent(expiredToken)}`, host: "fallback.test" } }) === null,
  "expired sessions should not authenticate"
);
assert(stateGet(sessionStateKey(expiredSessionId)) === null, "expired session lookup should delete the state record");
