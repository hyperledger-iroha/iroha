
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function publicKeyHexFromSpki(spkiDer) {
  return Buffer.from(spkiDer).subarray(-32).toString("hex");
}

function jsonReq(body, headers = { host: "clinic.test" }) {
  const encoded = Buffer.from(JSON.stringify(body), "utf8");
  return {
    headers,
    [Symbol.asyncIterator]: async function* () {
      yield encoded;
    }
  };
}

function emptyReq(headers = { host: "clinic.test" }) {
  return {
    headers,
    [Symbol.asyncIterator]: async function* () {}
  };
}

function resCapture() {
  return {
    status: null,
    headers: {},
    body: "",
    writeHead(status, headers = {}) {
      this.status = status;
      this.headers = headers;
    },
    end(body = "") {
      this.body += body ?? "";
    },
    json() {
      return this.body.length > 0 ? JSON.parse(this.body) : null;
    }
  };
}

const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
const publicKeyHex = publicKeyHexFromSpki(publicKey.export({ format: "der", type: "spki" }));
const capabilityMap = parseCapabilityMap(JSON.stringify({
  [publicKeyHex]: ["pii.records.read", "pii.consent.grant"]
}), true);

const challengeRes = resCapture();
await handleAuthChallenge(jsonReq({ public_key: publicKeyHex }), challengeRes);
assert(challengeRes.status === 200, `challenge should succeed: ${challengeRes.body}`);
const challenge = challengeRes.json();
assert(challenge.public_key === publicKeyHex, "challenge principal mismatch");
assert(challenge.message.includes(`challenge_id=${challenge.challenge_id}`), "challenge message should include challenge id");
assert(stateGet(challengeStateKey(challenge.challenge_id))?.public_key === publicKeyHex, "challenge state should be persisted");

const signature = crypto
  .sign(null, Buffer.from(challenge.message, "utf8"), privateKey)
  .toString("hex");
const loginRes = resCapture();
await handleAuthLogin(
  jsonReq({
    public_key: publicKeyHex,
    challenge_id: challenge.challenge_id,
    signature
  }),
  loginRes,
  capabilityMap
);
assert(loginRes.status === 200, `login should succeed: ${loginRes.body}`);
const login = loginRes.json();
assert(
  JSON.stringify(login.capabilities) === JSON.stringify(["pii.consent.grant", "pii.records.read"]),
  `login capabilities should be sorted: ${loginRes.body}`
);
assert(loginRes.headers["set-cookie"]?.includes("session="), "login should set a session cookie");
assert(stateGet(challengeConsumeLockStateKey(challenge.challenge_id)) === null, "login should release consume lock");

const sessionCookie = loginRes.headers["set-cookie"].split(";")[0];
const sessionToken = sessionCookie.slice("session=".length);
const sessionId = verifySessionToken(decodeURIComponent(sessionToken));
assert(sessionId, "login cookie should contain a valid signed session token");
assert(stateGet(sessionStateKey(sessionId))?.principal === publicKeyHex, "login should persist session state");

const meRes = resCapture();
handleAuthMe({ headers: { cookie: sessionCookie, host: "clinic.test" } }, meRes, capabilityMap, "pii.records.read");
assert(meRes.status === 200, `auth me should succeed: ${meRes.body}`);
assert(meRes.json().principal === publicKeyHex, "auth me principal mismatch");

const forbiddenRes = resCapture();
handleAuthMe({ headers: { cookie: sessionCookie, host: "clinic.test" } }, forbiddenRes, capabilityMap, "pii.records.delete");
assert(forbiddenRes.status === 403, `missing capability should be forbidden: ${forbiddenRes.body}`);
assert(forbiddenRes.json().code === "AUTH_FORBIDDEN", "missing capability should return AUTH_FORBIDDEN");

const logoutRes = resCapture();
handleAuthLogout({ headers: { cookie: sessionCookie, host: "clinic.test" } }, logoutRes);
assert(logoutRes.status === 204, "logout should return no-content");
assert(logoutRes.headers["set-cookie"]?.includes("Max-Age=0"), "logout should clear the session cookie");
assert(stateGet(sessionStateKey(sessionId)) === null, "logout should delete session state");

const afterLogoutRes = resCapture();
handleAuthMe({ headers: { cookie: sessionCookie, host: "clinic.test" } }, afterLogoutRes, capabilityMap);
assert(afterLogoutRes.status === 401, `logged out session should not authenticate: ${afterLogoutRes.body}`);
assert(afterLogoutRes.json().code === "AUTH_REQUIRED", "logged out session should return AUTH_REQUIRED");

const emptyBody = await readJson(emptyReq());
assert(JSON.stringify(emptyBody) === "{}", "empty JSON request body should decode as an object");
