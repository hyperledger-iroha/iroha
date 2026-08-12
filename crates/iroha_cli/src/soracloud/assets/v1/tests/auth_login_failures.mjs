
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

async function assertLoginError(body, expectedStatus, expectedCode, headers = { host: "clinic.test" }) {
  const res = resCapture();
  await handleAuthLogin(jsonReq(body, headers), res, new Map());
  assert(res.status === expectedStatus, `expected ${expectedStatus}, got ${res.status}: ${res.body}`);
  assert(res.json().code === expectedCode, `expected ${expectedCode}, got ${res.body}`);
}

const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
const publicKeyHex = publicKeyHexFromSpki(publicKey.export({ format: "der", type: "spki" }));
const { publicKey: otherPublicKey } = crypto.generateKeyPairSync("ed25519");
const otherPublicKeyHex = publicKeyHexFromSpki(otherPublicKey.export({ format: "der", type: "spki" }));
const signaturePlaceholder = "00".repeat(64);

await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: crypto.randomUUID(),
    signature: signaturePlaceholder
  },
  401,
  "AUTH_CHALLENGE_NOT_FOUND"
);

const expiredId = crypto.randomUUID();
statePut(challengeExpiredStateKey(expiredId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: expiredId,
  marked_at_unix_ms: Date.now()
});
await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: expiredId,
    signature: signaturePlaceholder
  },
  401,
  "AUTH_CHALLENGE_EXPIRED"
);

const mismatchId = crypto.randomUUID();
statePut(challengeStateKey(mismatchId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: mismatchId,
  public_key: otherPublicKeyHex,
  expires_at_unix_ms: Date.now() + 60000,
  used_at_unix_ms: null,
  origin: "http://clinic.test"
});
await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: mismatchId,
    signature: signaturePlaceholder
  },
  401,
  "AUTH_CHALLENGE_PRINCIPAL_MISMATCH"
);

const replayedId = crypto.randomUUID();
statePut(challengeStateKey(replayedId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: replayedId,
  public_key: publicKeyHex,
  expires_at_unix_ms: Date.now() + 60000,
  used_at_unix_ms: Date.now(),
  origin: "http://clinic.test"
});
await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: replayedId,
    signature: signaturePlaceholder
  },
  401,
  "AUTH_CHALLENGE_REPLAYED"
);

const originMismatchId = crypto.randomUUID();
statePut(challengeStateKey(originMismatchId), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: originMismatchId,
  public_key: publicKeyHex,
  expires_at_unix_ms: Date.now() + 60000,
  used_at_unix_ms: null,
  origin: "https://clinic.test"
});
await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: originMismatchId,
    signature: signaturePlaceholder
  },
  401,
  "AUTH_ORIGIN_MISMATCH",
  { host: "clinic.test" }
);

const invalidSignatureId = crypto.randomUUID();
const challenge = {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: invalidSignatureId,
  public_key: publicKeyHex,
  nonce: "nonce",
  issued_at_unix_ms: Date.now(),
  expires_at_unix_ms: Date.now() + 60000,
  used_at_unix_ms: null,
  origin: "http://clinic.test"
};
statePut(challengeStateKey(invalidSignatureId), challenge);
await assertLoginError(
  {
    public_key: publicKeyHex,
    challenge_id: invalidSignatureId,
    signature: signaturePlaceholder
  },
  401,
  "AUTH_SIGNATURE_INVALID"
);

const validButNoCapabilitiesId = crypto.randomUUID();
const validChallenge = {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: validButNoCapabilitiesId,
  public_key: publicKeyHex,
  nonce: "nonce-2",
  issued_at_unix_ms: Date.now(),
  expires_at_unix_ms: Date.now() + 60000,
  used_at_unix_ms: null,
  origin: "http://clinic.test"
};
statePut(challengeStateKey(validButNoCapabilitiesId), validChallenge);
const validSignature = crypto
  .sign(null, Buffer.from(canonicalChallengeMessage(validChallenge), "utf8"), privateKey)
  .toString("hex");
const loginRes = resCapture();
await handleAuthLogin(
  jsonReq({
    public_key: publicKeyHex,
    challenge_id: validButNoCapabilitiesId,
    signature: validSignature
  }),
  loginRes,
  new Map()
);
assert(loginRes.status === 200, `login without mapped capabilities should still mint a session: ${loginRes.body}`);
assert(JSON.stringify(loginRes.json().capabilities) === "[]", "unmapped principals should receive no capabilities");

const sessionCookie = loginRes.headers["set-cookie"].split(";")[0];
const capabilityMapRequired = resCapture();
handleAuthMe(
  { headers: { cookie: sessionCookie, host: "clinic.test" } },
  capabilityMapRequired,
  new Map(),
  "pii.records.read"
);
assert(capabilityMapRequired.status === 403, `empty capability map should fail: ${capabilityMapRequired.body}`);
assert(capabilityMapRequired.json().code === "AUTH_CAPABILITY_MAP_REQUIRED", "empty capability map should return AUTH_CAPABILITY_MAP_REQUIRED");
