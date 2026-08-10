import { Buffer } from "node:buffer";
import { generateKeyPairSync, randomBytes } from "node:crypto";
import { blake2b } from "@noble/hashes/blake2b";
import { networkIdBytes } from "./networkId.js";

const SID_PREFIX = Buffer.from("iroha-connect|sid|");
const SID_LENGTH = 32;
const NONCE_LENGTH = 16;
const X25519_KEY_LENGTH = 32;
const CONNECT_URI_VERSION = "1";
const CONNECT_URI_SCHEME = "iroha://connect";
const CONNECT_RESPONSE_FIELDS = new Set([
  "sid",
  "network_id",
  "app_pk",
  "nonce",
  "wallet_uri",
  "app_uri",
  "token_app",
  "token_wallet",
  "token_management",
  "token_relay",
]);
const CONNECT_URI_FIELDS = new Set([
  "sid",
  "network_id",
  "app_pk",
  "nonce",
  "node",
  "v",
  "role",
  "token",
  "relay",
]);
const CONNECT_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;

/**
 * Generate a Connect session identifier deterministically.
 * @param {{ networkId: import("./networkId.js").NetworkId; appPublicKey: BinaryLike; nonce?: BinaryLike | null }} options
 * @returns {{ sidBytes: Buffer; sidBase64Url: string; nonce: Buffer }}
 */
export function generateConnectSid(options = {}) {
  if (!options || typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  const networkId = options.networkId;
  const exactNetworkId = networkIdBytes(networkId, "networkId");
  const publicKey = normalizeBinary(options.appPublicKey, "appPublicKey", X25519_KEY_LENGTH);
  const nonce =
    options.nonce === undefined || options.nonce === null
      ? randomBytes(NONCE_LENGTH)
      : normalizeBinary(options.nonce, "nonce", NONCE_LENGTH);
  const digest = blake2b(
    Buffer.concat([SID_PREFIX, Buffer.from(exactNetworkId), publicKey, nonce]),
    { dkLen: SID_LENGTH },
  );
  const sidBytes = Buffer.from(digest);
  return {
    sidBytes,
    sidBase64Url: toBase64Url(sidBytes),
    nonce,
  };
}

/**
 * Create a Connect session preview by minting an X25519 keypair, nonce, and session URIs.
 * @param {{ networkId: import("./networkId.js").NetworkId; node?: string | null; nonce?: BinaryLike | null; appKeyPair?: { publicKey: BinaryLike; privateKey: BinaryLike } }} options
 * @returns {{
 *   networkId: import("./networkId.js").NetworkId;
 *   node: string | null;
 *   sidBytes: Buffer;
 *   sidBase64Url: string;
 *   nonce: Buffer;
 *   appKeyPair: { publicKey: Buffer; privateKey: Buffer };
 *   walletUri: string;
 *   appUri: string;
 * }}
 */
export function createConnectSessionPreview(options = {}) {
  if (!options || typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  const networkId = options.networkId;
  networkIdBytes(networkId, "networkId");
  const node =
    options.node === undefined || options.node === null
      ? null
      : requireNonEmptyString(options.node, "node");
  const appKeyPair = normalizeKeyPair(options.appKeyPair);
  const nonce =
    options.nonce === undefined || options.nonce === null
      ? randomBytes(NONCE_LENGTH)
      : normalizeBinary(options.nonce, "nonce", NONCE_LENGTH);
  const sidResult = generateConnectSid({
    networkId,
    appPublicKey: appKeyPair.publicKey,
    nonce,
  });
  return {
    networkId,
    node,
    sidBytes: sidResult.sidBytes,
    sidBase64Url: sidResult.sidBase64Url,
    nonce,
    appKeyPair,
    walletUri: buildConnectUri(
      sidResult.sidBase64Url,
      networkId,
      appKeyPair.publicKey,
      nonce,
      node,
      "wallet",
    ),
    appUri: buildConnectUri(
      sidResult.sidBase64Url,
      networkId,
      appKeyPair.publicKey,
      nonce,
      node,
      "app",
    ),
  };
}

/**
 * Fail closed when Torii substitutes any committed session field or deep-link
 * authorization value.
 * @param {Record<string, unknown>} session Parsed response (or a normalized
 * response carrying the original object as `raw`).
 * @param {{sid: string; networkId: string; appPk: string; nonce: string; node?: string | null}} expected
 */
export function validateConnectSessionResponseIdentity(session, expected) {
  if (!session || typeof session !== "object" || !expected || typeof expected !== "object") {
    throw new TypeError("Connect session response and expected identity must be objects");
  }
  const raw = session.raw && typeof session.raw === "object" ? session.raw : session;
  for (const field of Object.keys(raw)) {
    if (!CONNECT_RESPONSE_FIELDS.has(field)) {
      throw new Error(`Connect session response contains unsupported field ${field}`);
    }
  }
  const identity = {
    sid: requireExactString(raw.sid, "session.sid"),
    networkId: requireExactString(raw.network_id, "session.network_id"),
    appPk: requireExactString(raw.app_pk, "session.app_pk"),
    nonce: requireExactString(raw.nonce, "session.nonce"),
  };
  if (
    identity.sid !== expected.sid
    || identity.networkId !== expected.networkId
    || identity.appPk !== expected.appPk
    || identity.nonce !== expected.nonce
  ) {
    throw new Error("Torii substituted the canonical Connect session identity");
  }
  const tokens = {
    app: requireConnectToken(raw.token_app, "session.token_app"),
    wallet: requireConnectToken(raw.token_wallet, "session.token_wallet"),
    management: requireConnectToken(raw.token_management, "session.token_management"),
    relay: requireConnectToken(raw.token_relay, "session.token_relay"),
  };
  validateConnectLaunchUri(raw.wallet_uri, "wallet", identity, expected.node, tokens.wallet, tokens.relay);
  validateConnectLaunchUri(raw.app_uri, "app", identity, expected.node, tokens.app, tokens.relay);
}

function validateConnectLaunchUri(value, role, identity, node, token, relay) {
  const literal = requireExactString(value, `session.${role}_uri`);
  let parsed;
  try {
    parsed = new URL(literal);
  } catch {
    throw new Error(`session.${role}_uri must be an absolute Connect URI`);
  }
  if (
    parsed.protocol !== "iroha:"
    || parsed.host !== "connect"
    || parsed.pathname !== ""
    || parsed.hash !== ""
  ) {
    throw new Error(`session.${role}_uri must use iroha://connect without a path or fragment`);
  }
  const query = new Map();
  for (const [key, entry] of parsed.searchParams) {
    if (!CONNECT_URI_FIELDS.has(key)) {
      throw new Error(`session.${role}_uri contains unsupported parameter ${key}`);
    }
    if (query.has(key)) {
      throw new Error(`session.${role}_uri contains duplicate parameter ${key}`);
    }
    query.set(key, entry);
  }
  for (const field of CONNECT_URI_FIELDS) {
    if (!query.has(field)) {
      throw new Error(`session.${role}_uri is missing required parameter ${field}`);
    }
  }
  const expectedValues = {
    sid: identity.sid,
    network_id: identity.networkId,
    app_pk: identity.appPk,
    nonce: identity.nonce,
    node: node ?? "",
    v: CONNECT_URI_VERSION,
    role,
    token,
    relay,
  };
  for (const [field, expectedValue] of Object.entries(expectedValues)) {
    if (query.get(field) !== expectedValue) {
      throw new Error(`session.${role}_uri substituted Connect parameter ${field}`);
    }
  }
}

function requireExactString(value, name) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${name} must be an exact non-empty string`);
  }
  return value;
}

function requireConnectToken(value, name) {
  const token = requireExactString(value, name);
  if (!CONNECT_TOKEN_PATTERN.test(token)) {
    throw new TypeError(`${name} must be a canonical 32-byte unpadded base64url token`);
  }
  return token;
}

function normalizeKeyPair(pair) {
  if (!pair) {
    const generated = generateX25519KeyPair();
    return {
      publicKey: generated.publicKey,
      privateKey: generated.privateKey,
    };
  }
  if (typeof pair !== "object") {
    throw new TypeError("appKeyPair must be an object");
  }
  return {
    publicKey: normalizeBinary(pair.publicKey, "appKeyPair.publicKey", X25519_KEY_LENGTH),
    privateKey: normalizeBinary(pair.privateKey, "appKeyPair.privateKey", X25519_KEY_LENGTH),
  };
}

function generateX25519KeyPair() {
  const { publicKey, privateKey } = generateKeyPairSync("x25519");
  const jwkPublic = publicKey.export({ format: "jwk" });
  const jwkPrivate = privateKey.export({ format: "jwk" });
  if (!jwkPublic?.x || !jwkPrivate?.d) {
    throw new Error("Failed to export x25519 key material");
  }
  return {
    publicKey: Buffer.from(jwkPublic.x, "base64url"),
    privateKey: Buffer.from(jwkPrivate.d, "base64url"),
  };
}

function normalizeConnectRole(role, name = "role") {
  if (role === "app" || role === "wallet") {
    return role;
  }
  throw new TypeError(`${name} must be 'app' or 'wallet'`);
}

function buildConnectUri(sidBase64Url, networkId, appPublicKey, nonce, node, role) {
  const params = new URLSearchParams();
  params.set("sid", sidBase64Url);
  params.set("network_id", networkId.toString());
  params.set("app_pk", toBase64Url(appPublicKey));
  params.set("nonce", toBase64Url(nonce));
  if (node) {
    params.set("node", node);
  }
  params.set("v", CONNECT_URI_VERSION);
  params.set("role", normalizeConnectRole(role));
  return `${CONNECT_URI_SCHEME}?${params.toString()}`;
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${name} must not be empty`);
  }
  return trimmed;
}

function normalizeBinary(value, name, expectedLength) {
  const buffer = toBuffer(value, name);
  if (expectedLength !== undefined && buffer.length !== expectedLength) {
    throw new RangeError(
      `${name} must be ${expectedLength} bytes (received ${buffer.length} bytes)`,
    );
  }
  return buffer;
}

function normalizeByteArray(value, name) {
  const bytes = Array.from(value);
  const normalized = bytes.map((entry, index) => {
    if (!Number.isInteger(entry) || entry < 0 || entry > 0xff) {
      throw new TypeError(`${name}[${index}] must be a byte`);
    }
    return entry;
  });
  return Buffer.from(normalized);
}

function toBuffer(value, name) {
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (Array.isArray(value)) {
    return normalizeByteArray(value, name);
  }
  if (typeof value === "string") {
    return decodeStringBinary(value, name);
  }
  if (value && typeof value.length === "number") {
    return normalizeByteArray(value, name);
  }
  throw new TypeError(`${name} must be binary data`);
}

function decodeStringBinary(input, name) {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new TypeError(`${name} must not be empty`);
  }
  const hexPrefixed = trimmed.startsWith("0x") || trimmed.startsWith("0X");
  const hexBody = hexPrefixed ? trimmed.slice(2) : trimmed;
  if (/^[0-9a-fA-F]+$/.test(hexBody) && hexBody.length % 2 === 0) {
    return Buffer.from(hexBody, "hex");
  }
  try {
    return decodeBase64UrlStrict(trimmed, name);
  } catch {
    throw new TypeError(`${name} must be hex or base64 data`);
  }
}

function decodeBase64UrlStrict(value, name) {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  let padded = normalized;
  const paddingIndex = normalized.indexOf("=");
  if (paddingIndex !== -1) {
    const head = normalized.slice(0, paddingIndex);
    const padding = normalized.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      throw new TypeError(`${name} must be hex or base64 data`);
    }
    if (normalized.length % 4 !== 0) {
      throw new TypeError(`${name} must be hex or base64 data`);
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(normalized) || normalized.length % 4 === 1) {
      throw new TypeError(`${name} must be hex or base64 data`);
    }
    const padLength = (4 - (normalized.length % 4)) % 4;
    padded = normalized + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    throw new TypeError(`${name} must be hex or base64 data`);
  }
  return decoded;
}

function toBase64Url(buffer) {
  return Buffer.from(buffer)
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}
