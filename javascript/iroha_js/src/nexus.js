import { Buffer } from "node:buffer";
import { getNativeBinding } from "./native.js";

function ensureNative(methodName) {
  const native = getNativeBinding();
  if (!native || typeof native[methodName] !== "function") {
    throw new Error(
      `native iroha_js_host binding missing ${methodName}; run \`npm run build:native\` before calling Nexus helpers.`,
    );
  }
  return native;
}

function normalizeByteArray(value, field) {
  const bytes = value.map((entry, index) => {
    const numeric = Number(entry);
    if (!Number.isInteger(numeric) || numeric < 0 || numeric > 0xff) {
      throw new TypeError(`${field}[${index}] must be a byte`);
    }
    return numeric;
  });
  return Buffer.from(bytes);
}

function normalizeBytes(input, field) {
  if (input instanceof Uint8Array && !Buffer.isBuffer(input)) {
    return Buffer.from(input);
  }
  if (Buffer.isBuffer(input)) {
    return input;
  }
  if (ArrayBuffer.isView(input)) {
    return Buffer.from(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) {
    return Buffer.from(input);
  }
  if (typeof input === "string") {
    try {
      return decodeBase64Strict(input);
    } catch {
      throw new TypeError(
        `${field} must be Buffer | ArrayBufferView | ArrayBuffer | base64 string`,
      );
    }
  }
  if (Array.isArray(input)) {
    return normalizeByteArray(input, field);
  }
  throw new TypeError(
    `${field} must be Buffer | ArrayBufferView | ArrayBuffer | base64 string`,
  );
}

function normalizeRelayInteger(value, label) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value) || !Number.isSafeInteger(value)) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    if (value < 0) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    return BigInt(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    if (/^0x[0-9a-fA-F]+$/.test(trimmed) || /^[0-9]+$/.test(trimmed)) {
      const parsed = BigInt(trimmed);
      if (parsed < 0n) {
        throw new TypeError(`${label} must be a non-negative integer`);
      }
      return parsed;
    }
  }
  throw new TypeError(`${label} must be a non-negative integer`);
}

function decodeBase64Strict(value) {
  const compact = value.replace(/\s+/g, "");
  if (compact.length === 0) {
    throw new Error("payload is empty");
  }
  let padded = compact;
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      throw new Error("invalid base64 payload");
    }
    if (compact.length % 4 !== 0) {
      throw new Error("invalid base64 payload");
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(compact) || compact.length % 4 === 1) {
      throw new Error("invalid base64 payload");
    }
    const padLength = (4 - (compact.length % 4)) % 4;
    padded = compact + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    throw new Error("invalid base64 payload");
  }
  return decoded;
}

/**
 * Return a deterministic relay envelope fixture and a tampered copy for testing.
 */
export function laneRelayEnvelopeSample() {
  const native = ensureNative("laneRelayEnvelopeSample");
  const sample = native.laneRelayEnvelopeSample();
  return {
    valid: Buffer.from(sample.valid),
    tampered: Buffer.from(sample.tampered),
  };
}

/**
 * Verify a Norito-encoded relay envelope emitted by `/v1/sumeragi/status`.
 */
export function verifyLaneRelayEnvelope(envelope) {
  const native = ensureNative("verifyLaneRelayEnvelope");
  native.verifyLaneRelayEnvelope(normalizeBytes(envelope, "envelope"));
}

/**
 * Verify a relay envelope provided as JSON.
 */
export function verifyLaneRelayEnvelopeJson(envelope) {
  const native = ensureNative("verifyLaneRelayEnvelopeJson");
  const payload =
    typeof envelope === "string" ? envelope : JSON.stringify(envelope ?? {});
  native.verifyLaneRelayEnvelopeJson(payload);
}

/**
 * Verify a batch of relay envelopes and reject duplicate lane/dataspace/height tuples.
 */
export function verifyLaneRelayEnvelopes(envelopes) {
  if (!Array.isArray(envelopes)) {
    throw new TypeError("envelopes must be an array");
  }
  const seen = new Set();
  for (let i = 0; i < envelopes.length; i += 1) {
    const envelope = envelopes[i];
    verifyLaneRelayEnvelopeJson(envelope);
    let record = envelope;
    if (typeof envelope === "string") {
      try {
        record = JSON.parse(envelope);
      } catch {
        throw new TypeError(`envelope ${i} must be valid JSON`);
      }
    }
    const laneId = normalizeRelayInteger(record?.lane_id, `envelope ${i}.lane_id`);
    const dataspaceId = normalizeRelayInteger(
      record?.dataspace_id,
      `envelope ${i}.dataspace_id`,
    );
    const blockHeight = normalizeRelayInteger(
      record?.block_height,
      `envelope ${i}.block_height`,
    );
    const key = `${laneId.toString()}:${dataspaceId.toString()}:${blockHeight.toString()}`;
    if (seen.has(key)) {
      throw new Error(
        `duplicate relay envelope for lane ${laneId}, dataspace ${dataspaceId}, height ${blockHeight}`,
      );
    }
    seen.add(key);
  }
}

/**
 * Decode a relay envelope into a JSON object for inspection.
 */
export function decodeLaneRelayEnvelope(envelope) {
  const native = ensureNative("decodeLaneRelayEnvelope");
  const jsonString = native.decodeLaneRelayEnvelope(normalizeBytes(envelope, "envelope"));
  return JSON.parse(jsonString);
}

/**
 * Compute the settlement hash for a JSON `LaneBlockCommitment`.
 */
export function laneSettlementHash(settlement) {
  const native = ensureNative("laneSettlementHash");
  const payload =
    typeof settlement === "string" ? settlement : JSON.stringify(settlement ?? {});
  return native.laneSettlementHash(payload);
}
