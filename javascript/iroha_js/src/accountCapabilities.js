// SPDX-License-Identifier: Apache-2.0
import { CRYPTO_ALGORITHMS } from "./cryptoAlgorithms.js";
import { NetworkId } from "./networkId.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

const RESPONSE_MAX_BYTES = 4 * 1024;
const ALGORITHMS = new Set(Object.values(CRYPTO_ALGORITHMS));
const FIELDS = ["schema_version", "network_id", "network_prefix", "allowed_signing", "default_signing"];
const CONTEXT = "account capabilities response";

/** Decode the closed, account-free first-release bootstrap capability contract. */
export function normalizeAccountCapabilitiesV1(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)
    || Object.keys(value).length !== FIELDS.length
    || FIELDS.some((key) => !Object.hasOwn(value, key))) {
    throw new TypeError(`${CONTEXT} must contain exactly the V1 fields`);
  }
  if (value.schema_version !== 1) throw new TypeError(`${CONTEXT} schema_version must be 1`);
  if (typeof value.network_id !== "string") {
    throw new TypeError(`${CONTEXT} network_id must be a canonical NetworkId literal`);
  }
  const networkId = NetworkId.parse(value.network_id);
  if (networkId.literal !== value.network_id) {
    throw new TypeError(`${CONTEXT} network_id must use its exact canonical spelling`);
  }
  if (!Number.isInteger(value.network_prefix) || value.network_prefix < 0 || value.network_prefix > 0xffff) {
    throw new TypeError(`${CONTEXT} network_prefix must be a u16 integer`);
  }
  if (!Array.isArray(value.allowed_signing) || value.allowed_signing.length === 0
    || value.allowed_signing.length > ALGORITHMS.size
    || value.allowed_signing.some((algorithm) => !ALGORITHMS.has(algorithm))
    || new Set(value.allowed_signing).size !== value.allowed_signing.length) {
    throw new TypeError(`${CONTEXT} allowed_signing must contain unique canonical algorithms`);
  }
  // This is explicit protocol policy, never a preference inferred from list order or hashing.
  if (value.default_signing !== "ed25519" || !value.allowed_signing.includes("ed25519")) {
    throw new TypeError(`${CONTEXT} must advertise the admitted V1 Ed25519 default`);
  }
  return Object.freeze({
    schema_version: 1,
    network_id: networkId.literal,
    network_prefix: value.network_prefix,
    allowed_signing: Object.freeze([...value.allowed_signing]),
    default_signing: "ed25519",
  });
}

function cancelBody(body, reason) {
  try {
    Promise.resolve(body?.cancel(reason)).catch(() => {});
  } catch {
    // Cancellation must not replace the original transport or validation error.
  }
}

/** Read a bounded, abortable JSON capability response without consuming arbitrary error bodies. */
export async function readAccountCapabilitiesResponseV1(response, { signal } = {}) {
  let reader;
  let onAbort;
  let complete = false;
  try {
    if (response?.redirected === true) throw new TypeError(`${CONTEXT} must not be redirected`);
    if (response?.status !== 200) throw new Error(`${CONTEXT} requires HTTP 200; received ${response?.status}`);
    const contentType = response.headers?.get("content-type");
    if (typeof contentType !== "string" || !/^application\/json(?:\s*;|$)/iu.test(contentType)) {
      throw new TypeError(`${CONTEXT} must use application/json`);
    }
    const rawLength = response.headers.get("content-length");
    const declaredLength = rawLength === null ? null : Number(rawLength);
    if (rawLength !== null && (!/^(?:0|[1-9][0-9]*)$/u.test(rawLength)
      || !Number.isSafeInteger(declaredLength) || declaredLength > RESPONSE_MAX_BYTES)) {
      throw new RangeError(`${CONTEXT} exceeds its ${RESPONSE_MAX_BYTES}-byte limit`);
    }
    if (typeof response.body?.getReader !== "function") {
      throw new TypeError(`${CONTEXT} requires a bounded byte stream`);
    }
    reader = response.body.getReader();
    const interrupted = new Promise((_, reject) => {
      onAbort = () => {
        const reason = signal.reason ?? new Error(`${CONTEXT} was aborted`);
        cancelBody(reader, reason);
        reject(reason);
      };
      signal?.addEventListener("abort", onAbort, { once: true });
    });
    if (signal?.aborted) onAbort();
    const chunks = [];
    let length = 0;
    while (true) {
      const part = await Promise.race([reader.read(), interrupted]);
      if (signal?.aborted) throw signal.reason ?? new Error(`${CONTEXT} was aborted`);
      if (part.done) break;
      if (!(part.value instanceof Uint8Array) || chunks.length >= RESPONSE_MAX_BYTES) {
        throw new TypeError(`${CONTEXT} returned an invalid or excessively fragmented byte stream`);
      }
      if (part.value.byteLength > RESPONSE_MAX_BYTES - length) {
        throw new RangeError(`${CONTEXT} exceeds its ${RESPONSE_MAX_BYTES}-byte limit`);
      }
      length += part.value.byteLength;
      chunks.push(part.value.slice());
    }
    if (declaredLength !== null && declaredLength !== length) {
      throw new TypeError(`${CONTEXT} Content-Length does not match its body`);
    }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    const result = normalizeAccountCapabilitiesV1(parseStrictLosslessIntegerJson(text, CONTEXT));
    complete = true;
    return result;
  } finally {
    if (onAbort) signal?.removeEventListener("abort", onAbort);
    if (!complete) cancelBody(reader ?? response?.body, CONTEXT);
    try {
      reader?.releaseLock();
    } catch {
      // An aborted transport may still be completing its pending read cancellation.
    }
  }
}
