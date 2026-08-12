import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import url from "node:url";

const AUTH_MESSAGE_VERSION = "soracloud.auth.challenge.v1";
const AUTH_STATE_SCHEMA_VERSION = "soracloud.auth.state.v1";
const AUTH_CHALLENGE_PREFIX = "/state/auth/challenges";
const AUTH_CHALLENGE_EXPIRED_PREFIX = `${AUTH_CHALLENGE_PREFIX}/_meta/expired`;
const AUTH_CHALLENGE_CONSUME_LOCK_PREFIX = `${AUTH_CHALLENGE_PREFIX}/_meta/consume_locks`;
const AUTH_SESSION_PREFIX = "/state/auth/sessions";
const AUTH_MODE = normalizeAuthMode(process.env.AUTH_MODE ?? "strict");
const IS_PRODUCTION = (process.env.NODE_ENV ?? "development").trim() === "production";
const AUTH_REQUIRE_EXTERNAL_SHARED_STATE = parseBooleanEnv(
  "AUTH_REQUIRE_EXTERNAL_SHARED_STATE",
  process.env.AUTH_REQUIRE_EXTERNAL_SHARED_STATE,
  AUTH_MODE === "strict" || IS_PRODUCTION
);
const AUTH_SESSION_TTL_SECS = parsePositiveIntEnv(
  "AUTH_SESSION_TTL_SECS",
  process.env.AUTH_SESSION_TTL_SECS,
  900,
  60,
  86400
);
const AUTH_CHALLENGE_TTL_SECS = parsePositiveIntEnv(
  "AUTH_CHALLENGE_TTL_SECS",
  process.env.AUTH_CHALLENGE_TTL_SECS,
  120,
  5,
  900
);
const AUTH_SESSION_TTL_MS = AUTH_SESSION_TTL_SECS * 1000;
const AUTH_CHALLENGE_TTL_MS = AUTH_CHALLENGE_TTL_SECS * 1000;
const AUTH_CHALLENGE_EXPIRED_TTL_MS = Math.max(AUTH_CHALLENGE_TTL_MS, 30000);
const AUTH_CHALLENGE_CONSUME_LOCK_TTL_MS = Math.max(AUTH_CHALLENGE_TTL_MS, 15000);
const PUBLIC_BASE_URL = (process.env.PUBLIC_BASE_URL ?? "").trim();
const PUBLIC_BASE_ORIGIN = parsePublicOrigin(PUBLIC_BASE_URL);
const STATE_FILE_PATH = resolveStateFilePath();
const STATE_FILE_LOCK_DIR = `${STATE_FILE_PATH}.lock`;
const STATE_FILE_LOCK_STALE_MS = 30000;
const STATE_FILE_LOCK_TIMEOUT_MS = 5000;
const SESSION_HMAC_KEY = resolveSessionHmacKey();
const SHARED_STATE_ADAPTER = resolveSharedStateAdapter();

if (IS_PRODUCTION && !AUTH_REQUIRE_EXTERNAL_SHARED_STATE) {
  throw new Error("AUTH_REQUIRE_EXTERNAL_SHARED_STATE cannot be disabled in production mode");
}

function normalizeAuthMode(value) {
  const normalized = String(value ?? "strict").trim().toLowerCase();
  if (normalized !== "strict" && normalized !== "dev") {
    throw new Error(`AUTH_MODE must be strict or dev, got: ${value}`);
  }
  return normalized;
}

function parsePositiveIntEnv(name, rawValue, fallbackValue, minValue, maxValue) {
  const source = rawValue ?? String(fallbackValue);
  const value = Number.parseInt(source, 10);
  if (!Number.isFinite(value) || value < minValue || value > maxValue) {
    throw new Error(`${name} must be an integer in [${minValue}, ${maxValue}]`);
  }
  return value;
}

function parseBooleanEnv(name, rawValue, fallbackValue) {
  if (rawValue === undefined || rawValue === null || String(rawValue).trim().length === 0) {
    return fallbackValue;
  }
  const normalized = String(rawValue).trim().toLowerCase();
  if (normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on") {
    return true;
  }
  if (normalized === "0" || normalized === "false" || normalized === "no" || normalized === "off") {
    return false;
  }
  throw new Error(`${name} must be boolean (true/false/1/0)`);
}

function parsePublicOrigin(raw) {
  if (!raw) {
    return "";
  }
  try {
    return new URL(raw).origin;
  } catch (error) {
    throw new Error(`PUBLIC_BASE_URL is invalid: ${error.message}`);
  }
}

function resolveStateFilePath() {
  const explicitPath = (process.env.SORACLOUD_SHARED_STATE_FILE ?? "").trim();
  if (explicitPath.length > 0) {
    return path.resolve(explicitPath);
  }
  const moduleDir = path.dirname(url.fileURLToPath(import.meta.url));
  return path.resolve(moduleDir, "..", ".soracloud-shared", "auth_state.json");
}

function resolveSessionHmacKey() {
  const key = (process.env.SESSION_HMAC_KEY ?? "").trim();
  if (key.length >= 32) {
    return key;
  }
  if (IS_PRODUCTION || AUTH_MODE === "strict") {
    throw new Error(
      "SESSION_HMAC_KEY must be set to at least 32 characters in strict/production mode"
    );
  }
  return "dev-only-session-hmac-key-change-before-production";
}

function resolveSharedStateAdapter() {
  const adapter = globalThis.__soracloudSharedStateAdapter;
  if (!adapter) {
    if (AUTH_REQUIRE_EXTERNAL_SHARED_STATE) {
      throw new Error(
        "AUTH_REQUIRE_EXTERNAL_SHARED_STATE is enabled but globalThis.__soracloudSharedStateAdapter is not configured"
      );
    }
    return null;
  }

  for (const method of ["get", "put", "delete", "entries", "putIfAbsent"]) {
    if (typeof adapter[method] !== "function") {
      throw new Error(`globalThis.__soracloudSharedStateAdapter.${method} must be a function`);
    }
  }
  return adapter;
}

function canonicalizeJsonValue(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => canonicalizeJsonValue(entry));
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const out = {};
  for (const key of Object.keys(value).sort()) {
    out[key] = canonicalizeJsonValue(value[key]);
  }
  return out;
}

function stableJsonStringify(value) {
  return JSON.stringify(canonicalizeJsonValue(value));
}

function sleepSync(ms) {
  const buffer = new SharedArrayBuffer(4);
  Atomics.wait(new Int32Array(buffer), 0, 0, ms);
}

function removeStaleAuthStateLock(nowMs) {
  try {
    const stats = fs.statSync(STATE_FILE_LOCK_DIR);
    if (Number(stats.mtimeMs) + STATE_FILE_LOCK_STALE_MS <= nowMs) {
      fs.rmSync(STATE_FILE_LOCK_DIR, { recursive: true, force: true });
    }
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return;
    }
    throw error;
  }
}

function withAuthStateFileLock(operation) {
  const directory = path.dirname(STATE_FILE_PATH);
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  const deadlineMs = Date.now() + STATE_FILE_LOCK_TIMEOUT_MS;
  let locked = false;
  while (!locked) {
    try {
      fs.mkdirSync(STATE_FILE_LOCK_DIR, { mode: 0o700 });
      try {
        fs.writeFileSync(
          path.join(STATE_FILE_LOCK_DIR, "owner.json"),
          stableJsonStringify({ created_at_unix_ms: Date.now(), pid: process.pid }),
          { mode: 0o600 }
        );
      } catch (error) {
        fs.rmSync(STATE_FILE_LOCK_DIR, { recursive: true, force: true });
        throw error;
      }
      locked = true;
    } catch (error) {
      if (!error || error.code !== "EEXIST") {
        throw error;
      }
      const nowMs = Date.now();
      removeStaleAuthStateLock(nowMs);
      if (nowMs >= deadlineMs) {
        throw new Error("timed out waiting for auth state file lock");
      }
      sleepSync(10);
    }
  }

  try {
    return operation();
  } finally {
    fs.rmSync(STATE_FILE_LOCK_DIR, { recursive: true, force: true });
  }
}

function readAuthStateSnapshot() {
  try {
    const raw = fs.readFileSync(STATE_FILE_PATH, "utf8");
    if (raw.trim().length === 0) {
      return { schema_version: AUTH_STATE_SCHEMA_VERSION, records: {} };
    }
    const parsed = JSON.parse(raw);
    if (
      !parsed ||
      typeof parsed !== "object" ||
      parsed.schema_version !== AUTH_STATE_SCHEMA_VERSION ||
      !parsed.records ||
      typeof parsed.records !== "object" ||
      Array.isArray(parsed.records)
    ) {
      throw new Error("invalid auth state snapshot shape");
    }
    return parsed;
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return { schema_version: AUTH_STATE_SCHEMA_VERSION, records: {} };
    }
    throw error;
  }
}

function writeAuthStateSnapshot(snapshot) {
  const directory = path.dirname(STATE_FILE_PATH);
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  const tmpPath = `${STATE_FILE_PATH}.${process.pid}.tmp`;
  fs.writeFileSync(tmpPath, stableJsonStringify(snapshot), { mode: 0o600 });
  fs.renameSync(tmpPath, STATE_FILE_PATH);
}

function stateGet(key) {
  if (SHARED_STATE_ADAPTER) {
    const value = SHARED_STATE_ADAPTER.get(key);
    if (value === undefined || value === null) {
      return null;
    }
    return canonicalizeJsonValue(value);
  }
  const snapshot = readAuthStateSnapshot();
  return snapshot.records[key] ?? null;
}

function statePut(key, value) {
  const canonical = canonicalizeJsonValue(value);
  if (SHARED_STATE_ADAPTER) {
    SHARED_STATE_ADAPTER.put(key, canonical);
    return;
  }
  withAuthStateFileLock(() => {
    const snapshot = readAuthStateSnapshot();
    snapshot.records[key] = canonical;
    writeAuthStateSnapshot(snapshot);
  });
}

function statePutIfAbsent(key, value) {
  const canonical = canonicalizeJsonValue(value);
  if (SHARED_STATE_ADAPTER) {
    const inserted = SHARED_STATE_ADAPTER.putIfAbsent(key, canonical);
    if (typeof inserted !== "boolean") {
      throw new Error("shared state adapter putIfAbsent(key, value) must return boolean");
    }
    return inserted;
  }
  return withAuthStateFileLock(() => {
    const snapshot = readAuthStateSnapshot();
    if (Object.prototype.hasOwnProperty.call(snapshot.records, key)) {
      return false;
    }
    snapshot.records[key] = canonical;
    writeAuthStateSnapshot(snapshot);
    return true;
  });
}

function stateDelete(key) {
  if (SHARED_STATE_ADAPTER) {
    SHARED_STATE_ADAPTER.delete(key);
    return;
  }
  withAuthStateFileLock(() => {
    const snapshot = readAuthStateSnapshot();
    if (Object.prototype.hasOwnProperty.call(snapshot.records, key)) {
      delete snapshot.records[key];
      writeAuthStateSnapshot(snapshot);
    }
  });
}

function stateEntries(prefix) {
  if (SHARED_STATE_ADAPTER) {
    const rawEntries = SHARED_STATE_ADAPTER.entries(prefix);
    if (!Array.isArray(rawEntries)) {
      throw new Error("shared state adapter entries(prefix) must return [key, value][]");
    }
    const entries = [];
    for (const entry of rawEntries) {
      if (!Array.isArray(entry) || entry.length !== 2) {
        throw new Error("shared state adapter entries(prefix) must return [key, value][]");
      }
      const key = String(entry[0] ?? "").trim();
      if (key.length === 0) {
        throw new Error("shared state adapter entry keys must be non-empty strings");
      }
      if (!key.startsWith(prefix)) {
        continue;
      }
      entries.push([key, canonicalizeJsonValue(entry[1])]);
    }
    entries.sort((left, right) => left[0].localeCompare(right[0]));
    return entries;
  }
  const snapshot = readAuthStateSnapshot();
  const entries = [];
  for (const key of Object.keys(snapshot.records).sort()) {
    if (key.startsWith(prefix)) {
      entries.push([key, snapshot.records[key]]);
    }
  }
  return entries;
}

function parseCookies(headerValue = "") {
  const cookies = Object.create(null);
  for (const entry of headerValue.split(";")) {
    const [rawKey, ...rest] = entry.trim().split("=");
    if (!rawKey || rest.length === 0) {
      continue;
    }
    cookies[rawKey] = decodeURIComponent(rest.join("="));
  }
  return cookies;
}

function timingSafeEqualText(left, right) {
  const a = Buffer.from(String(left), "utf8");
  const b = Buffer.from(String(right), "utf8");
  if (a.length !== b.length) {
    return false;
  }
  return crypto.timingSafeEqual(a, b);
}

function requireTrimmedString(value, fieldName) {
  if (typeof value !== "string") {
    throw new Error(`${fieldName} must be a string`);
  }
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    throw new Error(`${fieldName} must not be empty`);
  }
  return trimmed;
}

function decodeHexStrict(value, expectedBytes, fieldName) {
  const normalized = requireTrimmedString(value, fieldName).toLowerCase();
  if (!/^[0-9a-f]+$/.test(normalized) || normalized.length !== expectedBytes * 2) {
    throw new Error(`${fieldName} must be ${expectedBytes} bytes of hex`);
  }
  const bytes = Buffer.from(normalized, "hex");
  if (bytes.length !== expectedBytes) {
    throw new Error(`${fieldName} must be ${expectedBytes} bytes of hex`);
  }
  return { hex: normalized, bytes };
}

function normalizePublicKey(value, fieldName = "public_key") {
  return decodeHexStrict(value, 32, fieldName).hex;
}

function parseCapabilityMap(raw, requireNonEmpty) {
  if (!raw || raw.trim().length === 0) {
    if (requireNonEmpty) {
      throw new Error("AUTH_CAPABILITY_MAP_JSON must be provided for private endpoints");
    }
    return new Map();
  }
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`AUTH_CAPABILITY_MAP_JSON is invalid JSON: ${error.message}`);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("AUTH_CAPABILITY_MAP_JSON must be an object");
  }
  const out = new Map();
  for (const [rawPrincipal, rawCapabilities] of Object.entries(parsed)) {
    const principal = normalizePublicKey(rawPrincipal, "AUTH_CAPABILITY_MAP_JSON principal");
    if (!Array.isArray(rawCapabilities) || rawCapabilities.length === 0) {
      throw new Error("AUTH_CAPABILITY_MAP_JSON values must be non-empty string arrays");
    }
    const normalizedCapabilities = [];
    for (const capability of rawCapabilities) {
      const normalizedCapability = requireTrimmedString(
        capability,
        "AUTH_CAPABILITY_MAP_JSON capability"
      );
      normalizedCapabilities.push(normalizedCapability);
    }
    normalizedCapabilities.sort();
    out.set(principal, Array.from(new Set(normalizedCapabilities)));
  }
  if (requireNonEmpty && out.size === 0) {
    throw new Error("AUTH_CAPABILITY_MAP_JSON must define at least one principal");
  }
  return out;
}

function requestOrigin(req) {
  if (PUBLIC_BASE_ORIGIN) {
    return PUBLIC_BASE_ORIGIN;
  }
  const forwardedProto = req.headers["x-forwarded-proto"];
  const proto =
    typeof forwardedProto === "string" && forwardedProto.trim().length > 0
      ? forwardedProto.split(",")[0].trim()
      : "http";
  const forwardedHost = req.headers["x-forwarded-host"];
  const host =
    typeof forwardedHost === "string" && forwardedHost.trim().length > 0
      ? forwardedHost.split(",")[0].trim()
      : req.headers.host ?? "";
  if (!host) {
    return "";
  }
  return `${proto}://${host}`;
}

function shouldUseSecureCookie(req) {
  if (PUBLIC_BASE_ORIGIN.startsWith("https://")) {
    return true;
  }
  const forwardedProto = req.headers["x-forwarded-proto"];
  return typeof forwardedProto === "string" && forwardedProto.includes("https");
}

function challengeStateKey(challengeId) {
  return `${AUTH_CHALLENGE_PREFIX}/${challengeId}`;
}

function challengeExpiredStateKey(challengeId) {
  return `${AUTH_CHALLENGE_EXPIRED_PREFIX}/${challengeId}`;
}

function isChallengeExpiredStateKey(key) {
  return key.startsWith(`${AUTH_CHALLENGE_EXPIRED_PREFIX}/`);
}

function challengeConsumeLockStateKey(challengeId) {
  return `${AUTH_CHALLENGE_CONSUME_LOCK_PREFIX}/${challengeId}`;
}

function isChallengeConsumeLockStateKey(key) {
  return key.startsWith(`${AUTH_CHALLENGE_CONSUME_LOCK_PREFIX}/`);
}

function sessionStateKey(sessionId) {
  return `${AUTH_SESSION_PREFIX}/${sessionId}`;
}

function canonicalChallengeMessage(challenge) {
  return [
    AUTH_MESSAGE_VERSION,
    `challenge_id=${challenge.challenge_id}`,
    `public_key=${challenge.public_key}`,
    `nonce=${challenge.nonce}`,
    `issued_at_unix_ms=${challenge.issued_at_unix_ms}`,
    `expires_at_unix_ms=${challenge.expires_at_unix_ms}`,
    `origin=${challenge.origin}`
  ].join("\n");
}

function verifyEd25519Signature(publicKeyHex, signatureHex, message) {
  const publicKey = decodeHexStrict(publicKeyHex, 32, "public_key");
  const signature = decodeHexStrict(signatureHex, 64, "signature");
  const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
  const derPublicKey = Buffer.concat([spkiPrefix, publicKey.bytes]);
  const verifierKey = crypto.createPublicKey({ key: derPublicKey, format: "der", type: "spki" });
  return crypto.verify(null, Buffer.from(message, "utf8"), verifierKey, signature.bytes);
}

function sendLoginChallengeFailureIfInvalid(req, res, challengeId, challenge, publicKey, nowMs) {
  if (!challenge || typeof challenge !== "object") {
    const expiredMarker = stateGet(challengeExpiredStateKey(challengeId));
    if (expiredMarker && typeof expiredMarker === "object") {
      sendAuthError(res, 401, "AUTH_CHALLENGE_EXPIRED", "challenge expired");
      return true;
    }
    sendAuthError(res, 401, "AUTH_CHALLENGE_NOT_FOUND", "challenge not found");
    return true;
  }

  const expiresAt = Number(challenge.expires_at_unix_ms);
  if (!Number.isFinite(expiresAt) || expiresAt <= nowMs) {
    statePut(challengeExpiredStateKey(challengeId), {
      schema_version: AUTH_STATE_SCHEMA_VERSION,
      challenge_id: challengeId,
      expires_at_unix_ms: Number.isFinite(expiresAt) && expiresAt > 0 ? expiresAt : nowMs,
      marked_at_unix_ms: nowMs
    });
    stateDelete(challengeStateKey(challengeId));
    sendAuthError(res, 401, "AUTH_CHALLENGE_EXPIRED", "challenge expired");
    return true;
  }
  if (challenge.used_at_unix_ms !== null && challenge.used_at_unix_ms !== undefined) {
    sendAuthError(res, 401, "AUTH_CHALLENGE_REPLAYED", "challenge already used");
    return true;
  }
  if (!timingSafeEqualText(challenge.public_key, publicKey)) {
    sendAuthError(
      res,
      401,
      "AUTH_CHALLENGE_PRINCIPAL_MISMATCH",
      "challenge principal mismatch"
    );
    return true;
  }

  const currentOrigin = requestOrigin(req);
  if (challenge.origin && !timingSafeEqualText(challenge.origin, currentOrigin)) {
    sendAuthError(res, 401, "AUTH_ORIGIN_MISMATCH", "request origin mismatch");
    return true;
  }

  return false;
}

function cleanupExpiredAuthRecords(nowMs = Date.now()) {
  for (const [key, challenge] of stateEntries(AUTH_CHALLENGE_PREFIX)) {
    if (isChallengeExpiredStateKey(key)) {
      const markedAt = Number(challenge?.marked_at_unix_ms ?? 0);
      if (!Number.isFinite(markedAt) || markedAt + AUTH_CHALLENGE_EXPIRED_TTL_MS <= nowMs) {
        stateDelete(key);
      }
      continue;
    }
    if (isChallengeConsumeLockStateKey(key)) {
      const expiresAt = Number(challenge?.expires_at_unix_ms ?? 0);
      if (!Number.isFinite(expiresAt) || expiresAt <= nowMs) {
        stateDelete(key);
      }
      continue;
    }
    const expiresAt = Number(challenge?.expires_at_unix_ms ?? 0);
    if (!Number.isFinite(expiresAt) || expiresAt <= nowMs) {
      const challengeId =
        typeof challenge?.challenge_id === "string" ? challenge.challenge_id.trim() : "";
      if (challengeId.length > 0) {
        statePut(challengeExpiredStateKey(challengeId), {
          schema_version: AUTH_STATE_SCHEMA_VERSION,
          challenge_id: challengeId,
          expires_at_unix_ms:
            Number.isFinite(expiresAt) && expiresAt > 0 ? expiresAt : nowMs,
          marked_at_unix_ms: nowMs
        });
      }
      stateDelete(key);
    }
  }
  for (const [key, session] of stateEntries(AUTH_SESSION_PREFIX)) {
    const expiresAt = Number(session?.expires_at_unix_ms ?? 0);
    if (!Number.isFinite(expiresAt) || expiresAt <= nowMs) {
      stateDelete(key);
    }
  }
}

function acquireChallengeConsumeLock(challengeId, nowMs = Date.now()) {
  const lockKey = challengeConsumeLockStateKey(challengeId);
  const existing = stateGet(lockKey);
  const existingExpiresAt = Number(existing?.expires_at_unix_ms ?? 0);
  if (existing && Number.isFinite(existingExpiresAt) && existingExpiresAt <= nowMs) {
    stateDelete(lockKey);
  }
  const owner = crypto.randomUUID();
  const inserted = statePutIfAbsent(lockKey, {
    schema_version: AUTH_STATE_SCHEMA_VERSION,
    challenge_id: challengeId,
    owner,
    created_at_unix_ms: nowMs,
    expires_at_unix_ms: nowMs + AUTH_CHALLENGE_CONSUME_LOCK_TTL_MS
  });
  if (!inserted) {
    return null;
  }
  return { challenge_id: challengeId, owner };
}

function releaseChallengeConsumeLock(lockHandle) {
  if (!lockHandle || typeof lockHandle !== "object") {
    return;
  }
  const challengeId =
    typeof lockHandle.challenge_id === "string" ? lockHandle.challenge_id.trim() : "";
  const owner = typeof lockHandle.owner === "string" ? lockHandle.owner : "";
  if (!challengeId || !owner) {
    return;
  }
  const lockKey = challengeConsumeLockStateKey(challengeId);
  const current = stateGet(lockKey);
  if (!current || typeof current !== "object" || typeof current.owner !== "string") {
    return;
  }
  if (!timingSafeEqualText(current.owner, owner)) {
    return;
  }
  stateDelete(lockKey);
}

function signSessionToken(sessionId) {
  const mac = crypto.createHmac("sha256", SESSION_HMAC_KEY).update(sessionId).digest("hex");
  return `${sessionId}.${mac}`;
}

function verifySessionToken(token) {
  const [sessionId, mac] = String(token ?? "").split(".");
  if (!sessionId || !mac || !/^[0-9a-f]+$/.test(mac)) {
    return null;
  }
  const expectedMac = crypto.createHmac("sha256", SESSION_HMAC_KEY).update(sessionId).digest("hex");
  if (!timingSafeEqualText(mac, expectedMac)) {
    return null;
  }
  return sessionId;
}

function buildSetCookieHeader(req, token) {
  let cookie = `session=${encodeURIComponent(token)}; HttpOnly; Path=/; SameSite=Strict`;
  if (shouldUseSecureCookie(req)) {
    cookie += "; Secure";
  }
  return cookie;
}

function buildClearCookieHeader(req) {
  let cookie = "session=; HttpOnly; Path=/; Max-Age=0; SameSite=Strict";
  if (shouldUseSecureCookie(req)) {
    cookie += "; Secure";
  }
  return cookie;
}

function getSessionFromRequest(req) {
  const cookies = parseCookies(req.headers.cookie ?? "");
  const token = cookies.session;
  if (!token) {
    return null;
  }
  const sessionId = verifySessionToken(token);
  if (!sessionId) {
    return null;
  }
  const record = stateGet(sessionStateKey(sessionId));
  if (!record || typeof record !== "object") {
    return null;
  }
  const nowMs = Date.now();
  if (Number(record.expires_at_unix_ms) <= nowMs) {
    stateDelete(sessionStateKey(sessionId));
    return null;
  }
  const currentOrigin = requestOrigin(req);
  if (record.origin && !timingSafeEqualText(record.origin, currentOrigin)) {
    return null;
  }
  return record;
}

function requireAuthenticatedSession(req, res, capabilityMap, requiredCapability) {
  const session = getSessionFromRequest(req);
  if (!session) {
    sendAuthError(res, 401, "AUTH_REQUIRED", "authentication required");
    return null;
  }
  if (!requiredCapability) {
    return session;
  }
  if (!capabilityMap || capabilityMap.size === 0) {
    sendAuthError(res, 403, "AUTH_CAPABILITY_MAP_REQUIRED", "capability map is required");
    return null;
  }
  if (!session.capabilities.includes(requiredCapability)) {
    sendAuthError(res, 403, "AUTH_FORBIDDEN", "missing required capability", {
      required_capability: requiredCapability
    });
    return null;
  }
  return session;
}

async function readJson(req) {
  let body = "";
  for await (const chunk of req) {
    body += chunk.toString("utf8");
    if (body.length > 65536) {
      throw new Error("request body too large");
    }
  }
  if (body.trim().length === 0) {
    return {};
  }
  try {
    return JSON.parse(body);
  } catch {
    throw new Error("invalid JSON payload");
  }
}

function sendJson(res, status, body, extraHeaders = {}) {
  const headers = Object.assign(
    {
      "content-type": "application/json; charset=utf-8"
    },
    extraHeaders
  );
  res.writeHead(status, headers);
  res.end(stableJsonStringify(body));
}

function sendAuthError(res, status, code, error, extra = {}) {
  sendJson(res, status, Object.assign({ code, error }, extra));
}

function sendInternalError(res, error) {
  // eslint-disable-next-line no-console
  console.error(error?.stack ?? String(error));
  if (res.headersSent) {
    res.destroy(error);
    return;
  }
  sendAuthError(res, 500, "INTERNAL_SERVER_ERROR", "internal server error");
}

async function handleAuthChallenge(req, res) {
  try {
    const body = await readJson(req);
    const publicKey = normalizePublicKey(body.public_key, "public_key");
    cleanupExpiredAuthRecords();
    const nowMs = Date.now();
    const challenge = {
      schema_version: AUTH_STATE_SCHEMA_VERSION,
      challenge_id: crypto.randomUUID(),
      public_key: publicKey,
      nonce: crypto.randomBytes(16).toString("hex"),
      issued_at_unix_ms: nowMs,
      expires_at_unix_ms: nowMs + AUTH_CHALLENGE_TTL_MS,
      used_at_unix_ms: null,
      origin: requestOrigin(req)
    };
    statePut(challengeStateKey(challenge.challenge_id), challenge);
    sendJson(res, 200, {
      auth_message_version: AUTH_MESSAGE_VERSION,
      challenge_id: challenge.challenge_id,
      expires_at_unix_ms: challenge.expires_at_unix_ms,
      issued_at_unix_ms: challenge.issued_at_unix_ms,
      message: canonicalChallengeMessage(challenge),
      nonce: challenge.nonce,
      public_key: challenge.public_key
    });
  } catch (error) {
    sendAuthError(res, 400, "INVALID_REQUEST", error.message);
  }
}

async function handleAuthLogin(req, res, capabilityMap) {
  try {
    const body = await readJson(req);
    const publicKey = normalizePublicKey(body.public_key, "public_key");
    const challengeId = requireTrimmedString(body.challenge_id, "challenge_id");
    const signature = requireTrimmedString(body.signature, "signature");
    cleanupExpiredAuthRecords();

    const challengeKey = challengeStateKey(challengeId);
    let nowMs = Date.now();
    let challenge = stateGet(challengeKey);
    if (sendLoginChallengeFailureIfInvalid(req, res, challengeId, challenge, publicKey, nowMs)) {
      return;
    }

    let canonicalMessage = canonicalChallengeMessage(challenge);
    let signatureValid = verifyEd25519Signature(publicKey, signature, canonicalMessage);
    if (!signatureValid) {
      sendAuthError(res, 401, "AUTH_SIGNATURE_INVALID", "signature verification failed");
      return;
    }

    const consumeLock = acquireChallengeConsumeLock(challengeId);
    if (!consumeLock) {
      sendAuthError(res, 401, "AUTH_CHALLENGE_REPLAYED", "challenge already used");
      return;
    }

    try {
      nowMs = Date.now();
      challenge = stateGet(challengeKey);
      if (sendLoginChallengeFailureIfInvalid(req, res, challengeId, challenge, publicKey, nowMs)) {
        return;
      }

      canonicalMessage = canonicalChallengeMessage(challenge);
      signatureValid = verifyEd25519Signature(publicKey, signature, canonicalMessage);
      if (!signatureValid) {
        sendAuthError(res, 401, "AUTH_SIGNATURE_INVALID", "signature verification failed");
        return;
      }

      challenge.used_at_unix_ms = nowMs;
      statePut(challengeKey, challenge);

      const principal = publicKey;
      const capabilities = (capabilityMap.get(principal) ?? []).slice().sort();
      const sessionId = crypto.randomUUID();
      const session = {
        schema_version: AUTH_STATE_SCHEMA_VERSION,
        session_id: sessionId,
        principal,
        capabilities,
        issued_at_unix_ms: nowMs,
        expires_at_unix_ms: nowMs + AUTH_SESSION_TTL_MS,
        origin: challenge.origin
      };
      statePut(sessionStateKey(sessionId), session);

      const token = signSessionToken(sessionId);
      sendJson(
        res,
        200,
        {
          capabilities,
          principal,
          session_expires_at_unix_ms: session.expires_at_unix_ms
        },
        { "set-cookie": buildSetCookieHeader(req, token) }
      );
    } finally {
      releaseChallengeConsumeLock(consumeLock);
    }
  } catch (error) {
    sendAuthError(res, 400, "INVALID_REQUEST", error.message);
  }
}

function handleAuthMe(req, res, capabilityMap, requiredCapability = null) {
  cleanupExpiredAuthRecords();
  const session = requireAuthenticatedSession(req, res, capabilityMap, requiredCapability);
  if (!session) {
    return;
  }
  sendJson(res, 200, {
    capabilities: session.capabilities,
    principal: session.principal,
    session_expires_at_unix_ms: session.expires_at_unix_ms
  });
}

function handleAuthLogout(req, res) {
  cleanupExpiredAuthRecords();
  const session = getSessionFromRequest(req);
  if (session && session.session_id) {
    stateDelete(sessionStateKey(session.session_id));
  }
  res.writeHead(204, { "set-cookie": buildClearCookieHeader(req) });
  res.end();
}
