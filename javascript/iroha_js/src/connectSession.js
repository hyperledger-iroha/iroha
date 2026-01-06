import { Buffer } from "node:buffer";
import { createHash, generateKeyPairSync, randomBytes } from "node:crypto";

const SID_PREFIX = Buffer.from("iroha-connect|sid|");
const SID_LENGTH = 32;
const NONCE_LENGTH = 16;
const X25519_KEY_LENGTH = 32;
const CONNECT_URI_VERSION = "1";

/**
 * Generate a Connect session identifier deterministically.
 * @param {{ chainId: string; appPublicKey: BinaryLike; nonce?: BinaryLike | null }} options
 * @returns {{ sidBytes: Buffer; sidBase64Url: string; nonce: Buffer }}
 */
export function generateConnectSid(options = {}) {
  if (!options || typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  const chainId = requireNonEmptyString(options.chainId, "chainId");
  const publicKey = normalizeBinary(options.appPublicKey, "appPublicKey", X25519_KEY_LENGTH);
  const nonce =
    options.nonce === undefined || options.nonce === null
      ? randomBytes(NONCE_LENGTH)
      : normalizeBinary(options.nonce, "nonce", NONCE_LENGTH);
  const hash = createHash("blake2b512");
  hash.update(SID_PREFIX);
  hash.update(Buffer.from(chainId, "utf8"));
  hash.update(publicKey);
  hash.update(nonce);
  const digest = hash.digest().subarray(0, SID_LENGTH);
  const sidBytes = Buffer.from(digest);
  return {
    sidBytes,
    sidBase64Url: toBase64Url(sidBytes),
    nonce,
  };
}

/**
 * Create a Connect session preview by minting an X25519 keypair, nonce, and session URIs.
 * @param {{ chainId: string; node?: string | null; nonce?: BinaryLike | null; appKeyPair?: { publicKey: BinaryLike; privateKey: BinaryLike } }} options
 * @returns {{
 *   chainId: string;
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
  const chainId = requireNonEmptyString(options.chainId, "chainId");
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
    chainId,
    appPublicKey: appKeyPair.publicKey,
    nonce,
  });
  return {
    chainId,
    node,
    sidBytes: sidResult.sidBytes,
    sidBase64Url: sidResult.sidBase64Url,
    nonce,
    appKeyPair,
    walletUri: buildConnectUri("connect", sidResult.sidBase64Url, chainId, node),
    appUri: buildConnectUri("connect/app", sidResult.sidBase64Url, chainId, node),
  };
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

function buildConnectUri(path, sidBase64Url, chainId, node) {
  const params = new URLSearchParams();
  params.set("sid", sidBase64Url);
  params.set("chain_id", chainId);
  if (node) {
    params.set("node", node);
  }
  params.set("v", CONNECT_URI_VERSION);
  return `iroha://${path}?${params.toString()}`;
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
    return Buffer.from(value);
  }
  if (typeof value === "string") {
    return decodeStringBinary(value, name);
  }
  if (value && typeof value.length === "number") {
    return Buffer.from(Array.from(value));
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
