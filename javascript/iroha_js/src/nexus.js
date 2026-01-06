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

function normalizeBytes(input, field) {
  if (input instanceof Uint8Array && !Buffer.isBuffer(input)) {
    return Buffer.from(input);
  }
  if (Buffer.isBuffer(input)) {
    return input;
  }
  if (typeof input === "string") {
    try {
      return decodeBase64Strict(input);
    } catch {
      throw new TypeError(`${field} must be Buffer | Uint8Array | base64 string`);
    }
  }
  if (Array.isArray(input)) {
    return Buffer.from(input);
  }
  throw new TypeError(`${field} must be Buffer | Uint8Array | base64 string`);
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
    const laneId = Number(envelope?.lane_id);
    const dataspaceId = Number(envelope?.dataspace_id);
    const blockHeight = Number(envelope?.block_height);
    if (
      Number.isNaN(laneId) ||
      Number.isNaN(dataspaceId) ||
      Number.isNaN(blockHeight)
    ) {
      throw new TypeError(
        `envelope ${i} missing numeric lane_id, dataspace_id, or block_height`,
      );
    }
    const key = `${laneId}:${dataspaceId}:${blockHeight}`;
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
