import { x25519 } from "@noble/curves/ed25519";
import { blake2b } from "@noble/hashes/blake2b";

const encoder = new TextEncoder();
const SID_PREFIX = encoder.encode("iroha-connect|sid|");
const CONNECT_URI_VERSION = "1";
const CONNECT_URI_SCHEME = "iroha://connect";
const DEFAULT_CONNECT_LAUNCH_PROTOCOL = "irohaconnect";
const DEFAULT_TORII_BASE_URL = "https://taira.sora.org";

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${name} must not be empty`);
  }
  return trimmed;
}

function toUint8Array(value, name) {
  if (value instanceof Uint8Array) {
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value)) {
    return Uint8Array.from(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    const hex = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed;
    if (/^[0-9a-fA-F]+$/.test(hex) && hex.length % 2 === 0) {
      const out = new Uint8Array(hex.length / 2);
      for (let index = 0; index < out.length; index += 1) {
        out[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
      }
      return out;
    }
  }
  throw new TypeError(`${name} must be binary data`);
}

function concatBytes(...parts) {
  const length = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function randomBytes(length) {
  const cryptoImpl = globalThis.crypto;
  if (!cryptoImpl || typeof cryptoImpl.getRandomValues !== "function") {
    throw new Error("Web Crypto getRandomValues() is required");
  }
  const out = new Uint8Array(length);
  cryptoImpl.getRandomValues(out);
  return out;
}

function bytesToBase64(bytes) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let output = "";
  let index = 0;
  while (index < bytes.length) {
    const a = bytes[index++] ?? 0;
    const b = bytes[index++] ?? 0;
    const c = bytes[index++] ?? 0;
    const triplet = (a << 16) | (b << 8) | c;
    output += alphabet[(triplet >> 18) & 0x3f];
    output += alphabet[(triplet >> 12) & 0x3f];
    output += index - 1 > bytes.length ? "=" : alphabet[(triplet >> 6) & 0x3f];
    output += index > bytes.length ? "=" : alphabet[triplet & 0x3f];
  }
  const remainder = bytes.length % 3;
  if (remainder === 1) {
    return `${output.slice(0, -2)}==`;
  }
  if (remainder === 2) {
    return `${output.slice(0, -1)}=`;
  }
  return output;
}

export function toHex(bytes) {
  return Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("");
}

export function toBase64Url(bytes) {
  return bytesToBase64(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function baseUrlFromLocation() {
  if (typeof window === "undefined") {
    return DEFAULT_TORII_BASE_URL;
  }
  return `${window.location.protocol}//${window.location.host}`;
}

function normalizeConnectRole(role, name = "role") {
  if (role === "app" || role === "wallet") {
    return role;
  }
  throw new TypeError(`${name} must be 'app' or 'wallet'`);
}

function normalizeOptionalString(value) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function normalizeConnectProtocol(value, name = "protocol") {
  const normalized = requireNonEmptyString(value, name);
  return normalized.endsWith(":") ? normalized.slice(0, -1) : normalized;
}

function buildConnectUri(sidBase64Url, chainId, node, role, token = null) {
  const params = new URLSearchParams({
    sid: sidBase64Url,
    chain_id: chainId,
    v: CONNECT_URI_VERSION,
    role: normalizeConnectRole(role),
  });
  if (node) {
    params.set("node", node);
  }
  if (token) {
    params.set("token", requireNonEmptyString(token, "token"));
  }
  return `${CONNECT_URI_SCHEME}?${params.toString()}`;
}

export function buildConnectWebSocketUrl(baseUrl, sid, role = "app") {
  const normalizedBaseUrl = requireNonEmptyString(baseUrl, "baseUrl");
  const normalizedSid = requireNonEmptyString(sid, "sid");
  const normalizedRole = normalizeConnectRole(role);
  const url = new URL("/v1/connect/ws", `${normalizedBaseUrl}/`);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.searchParams.set("sid", normalizedSid);
  url.searchParams.set("role", normalizedRole);
  return url.toString();
}

export function createConnectSessionPreview(options = {}) {
  if (!options || typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  const chainId = requireNonEmptyString(options.chainId, "chainId");
  const node =
    options.node === undefined || options.node === null
      ? null
      : requireNonEmptyString(options.node, "node");
  const privateKey = options.appKeyPair?.privateKey
    ? toUint8Array(options.appKeyPair.privateKey, "appKeyPair.privateKey")
    : x25519.utils.randomPrivateKey();
  if (privateKey.length !== 32) {
    throw new RangeError(`appKeyPair.privateKey must be 32 bytes (received ${privateKey.length})`);
  }
  const publicKey = options.appKeyPair?.publicKey
    ? toUint8Array(options.appKeyPair.publicKey, "appKeyPair.publicKey")
    : x25519.getPublicKey(privateKey);
  if (publicKey.length !== 32) {
    throw new RangeError(`appKeyPair.publicKey must be 32 bytes (received ${publicKey.length})`);
  }
  const nonce =
    options.nonce === undefined || options.nonce === null
      ? randomBytes(16)
      : toUint8Array(options.nonce, "nonce");
  if (nonce.length !== 16) {
    throw new RangeError(`nonce must be 16 bytes (received ${nonce.length})`);
  }
  const sidBytes = blake2b(concatBytes(SID_PREFIX, encoder.encode(chainId), publicKey, nonce), {
    dkLen: 32,
  });
  const sidBase64Url = toBase64Url(sidBytes);
  const toriiBaseUrl = node || baseUrlFromLocation();
  return {
    chainId,
    node,
    sidBytes,
    sidBase64Url,
    nonce,
    appKeyPair: {
      publicKey,
      privateKey,
    },
    walletUri: buildConnectUri(sidBase64Url, chainId, node, "wallet"),
    appUri: buildConnectUri(sidBase64Url, chainId, node, "app"),
    wsUrl: buildConnectWebSocketUrl(toriiBaseUrl, sidBase64Url, "app"),
    createdAt: Date.now(),
  };
}

function getFetch(fetchImpl) {
  const resolved = fetchImpl ?? globalThis.fetch;
  if (typeof resolved !== "function") {
    throw new Error("fetch implementation is required");
  }
  return resolved;
}

export async function registerConnectSession(baseUrl, sid, options = {}) {
  const response = await getFetch(options.fetchImpl)(
    new URL("/v1/connect/session", `${requireNonEmptyString(baseUrl, "baseUrl")}/`),
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(
        options.node === undefined || options.node === null
          ? { sid: requireNonEmptyString(sid, "sid") }
          : { sid: requireNonEmptyString(sid, "sid"), node: requireNonEmptyString(options.node, "node") },
      ),
    },
  );
  if (!response.ok) {
    const message = await response.text().catch(() => response.statusText);
    throw new Error(`${response.status} ${response.statusText}: ${message || "unable to create connect session"}`);
  }
  return response.json();
}

export async function deleteConnectSession(baseUrl, sid, options = {}) {
  const response = await getFetch(options.fetchImpl)(
    new URL(
      `/v1/connect/session/${encodeURIComponent(requireNonEmptyString(sid, "sid"))}`,
      `${requireNonEmptyString(baseUrl, "baseUrl")}/`,
    ),
    {
      method: "DELETE",
      headers: {
        Accept: "application/json",
      },
    },
  );
  if (!response.ok && response.status !== 404) {
    const message = await response.text().catch(() => response.statusText);
    throw new Error(`${response.status} ${response.statusText}: ${message || "unable to delete connect session"}`);
  }
}

export function buildConnectTokenProtocol(token) {
  const normalized = requireNonEmptyString(token, "token");
  return `iroha-connect.token.v1.${toBase64Url(encoder.encode(normalized))}`;
}

export function resolveConnectLaunchUri(role, preview = null, session = null) {
  const normalizedRole = normalizeConnectRole(role);
  const sessionUri =
    normalizedRole === "wallet"
      ? normalizeOptionalString(session?.wallet_uri)
      : normalizeOptionalString(session?.app_uri);
  if (sessionUri) {
    return sessionUri;
  }
  return normalizedRole === "wallet"
    ? normalizeOptionalString(preview?.walletUri) ?? ""
    : normalizeOptionalString(preview?.appUri) ?? "";
}

export function rewriteConnectUriProtocol(
  uri,
  protocol = DEFAULT_CONNECT_LAUNCH_PROTOCOL,
) {
  const parsed = new URL(requireNonEmptyString(uri, "uri"));
  parsed.protocol = `${normalizeConnectProtocol(protocol)}:`;
  return parsed.toString();
}

export function resolveConnectLaunchUriForProtocol(
  role,
  preview = null,
  session = null,
  protocol = DEFAULT_CONNECT_LAUNCH_PROTOCOL,
) {
  const launchUri = resolveConnectLaunchUri(role, preview, session);
  if (!launchUri) {
    return "";
  }
  return rewriteConnectUriProtocol(launchUri, protocol);
}

function mergeConnectProtocols(protocols, tokenProtocol) {
  if (protocols === undefined || protocols === null) {
    return [tokenProtocol];
  }
  if (typeof protocols === "string") {
    return protocols === tokenProtocol ? [protocols] : [tokenProtocol, protocols];
  }
  if (Array.isArray(protocols)) {
    const normalized = protocols.map((protocol, index) =>
      requireNonEmptyString(protocol, `protocols[${index}]`)
    );
    return normalized.includes(tokenProtocol)
      ? normalized
      : [tokenProtocol, ...normalized];
  }
  throw new TypeError("protocols must be a string or array");
}

export function openConnectWebSocket(baseUrl, sid, token, role = "app", options = {}) {
  const WebSocketImpl = options.webSocketImpl ?? globalThis.WebSocket;
  if (typeof WebSocketImpl !== "function") {
    throw new Error("WebSocket implementation is required");
  }
  return new WebSocketImpl(
    buildConnectWebSocketUrl(baseUrl, sid, role),
    mergeConnectProtocols(options.protocols, buildConnectTokenProtocol(token)),
  );
}
