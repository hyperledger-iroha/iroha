import { blake2b256 } from "./blake2b.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

const encoder = new TextEncoder();
const KEY = encoder.encode("iroha:contract-state:key:v1\0");
const VALUE = encoder.encode("iroha:contract-state:value:v1\0");
const LEAF = encoder.encode("iroha:merkle-map:leaf:v1\0");
const BRANCH = encoder.encode("iroha:merkle-map:branch:v1\0");
const ROOT = encoder.encode("iroha:merkle-map:root:v1\0");
const HASH_LITERAL = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u;
const MAX_VALUE_BYTES = 1024 * 1024;
const MAX_WIRE_BYTES = MAX_VALUE_BYTES + 128 * 1024;

function bytes(value, length, field, maximum = null) {
  const input = value instanceof Uint8Array ? value : Array.isArray(value) ? value : null;
  if (input === null || (length !== null && input.length !== length)
      || (maximum !== null && input.length > maximum)
      || Array.from(input).some((item) => !Number.isInteger(item) || item < 0 || item > 255)) {
    throw new TypeError(`${field} must be ${length ?? "a bounded sequence of"} bytes`);
  }
  return Uint8Array.from(input);
}

function concat(...parts) {
  const result = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

function hash(...parts) {
  const digest = Uint8Array.from(blake2b256(concat(...parts)));
  digest[31] |= 1;
  return digest;
}

function hashBytes(value, field) {
  if (typeof value !== "string") {
    const decoded = bytes(value, 32, field);
    if ((decoded[31] & 1) === 0) throw new TypeError(`${field} lacks the Iroha hash marker`);
    return decoded;
  }
  const match = HASH_LITERAL.exec(value);
  if (!match || computeHashLiteralCrc("hash", match[1]) !== match[2]) {
    throw new TypeError(`${field} must be a canonical Iroha hash literal`);
  }
  const decoded = Uint8Array.from(match[1].match(/.{2}/gu), (pair) => Number.parseInt(pair, 16));
  if ((decoded[31] & 1) === 0) throw new TypeError(`${field} lacks the Iroha hash marker`);
  return decoded;
}

function path(value, field) {
  if (typeof value !== "string" || value.length === 0
      || value.length > 16 * 1024 || encoder.encode(value).length > 16 * 1024
      || value !== value.normalize("NFC")
      || /[\s\p{Cc}\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069@#$]/u.test(value)) {
    throw new TypeError(`${field} must be one canonical physical StatePath`);
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const low = value.charCodeAt(index + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) {
        throw new TypeError(`${field} contains a lone UTF-16 surrogate`);
      }
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      throw new TypeError(`${field} contains a lone UTF-16 surrogate`);
    }
  }
  return value;
}

function u64(value, field) {
  let number;
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) {
    number = BigInt(value);
  } else if (typeof value === "bigint" && value >= 0n) {
    number = value;
  } else if (typeof value === "string" && value.length <= 20 && /^(0|[1-9][0-9]*)$/u.test(value)) {
    number = BigInt(value);
  } else {
    throw new TypeError(`${field} must be an exact unsigned 64-bit integer`);
  }
  if (number > 0xffffffffffffffffn) throw new TypeError(`${field} exceeds u64`);
  return number;
}

function le(number, length) {
  const result = new Uint8Array(length);
  for (let index = 0; index < length; index += 1) {
    result[index] = Number((number >> BigInt(index * 8)) & 0xffn);
  }
  return result;
}

function equal(a, b) {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

function keyBit(key, bit) {
  return (key[Math.floor(bit / 8)] & (0x80 >> (bit % 8))) !== 0;
}

function prefix(key, bit) {
  const result = Uint8Array.from(key);
  const index = Math.floor(bit / 8);
  result[index] = bit % 8 === 0 ? 0 : result[index] & (0xff << (8 - bit % 8));
  result.fill(0, index + 1);
  return result;
}

/**
 * Verify exact raw-value inclusion under an independently trusted accumulated
 * contract-state root. The caller must authenticate that root through v2
 * finality; this function does not treat an execution-witness root as authority.
 */
function verifyChecked(proof, expectedPath, trustedRoot) {
  if (proof === null || typeof proof !== "object" || Array.isArray(proof)
      || Object.keys(proof).some((key) => !["version", "path", "value", "leaf_count", "steps"].includes(key))
      || proof.version !== 1 || !Array.isArray(proof.steps) || proof.steps.length > 256) {
    return false;
  }
  const requested = path(expectedPath, "expectedPath");
  const physical = path(proof.path, "proof.path");
  if (physical !== requested) return false;
  const value = bytes(proof.value, null, "proof.value", MAX_VALUE_BYTES);
  const count = u64(proof.leaf_count, "proof.leaf_count");
  if (count === 0n || (count === 1n) !== (proof.steps.length === 0)) return false;
  const key = hash(KEY, encoder.encode(physical));
  const valueHash = hash(VALUE, value);
  let current = hash(LEAF, key, valueHash);
  let previous = -1;
  const steps = proof.steps.map((step) => {
    if (step === null || typeof step !== "object" || Array.isArray(step)
        || Object.keys(step).some((key) => !["bit", "prefix", "sibling"].includes(key))
        || !Number.isInteger(step.bit)
        || step.bit < 0 || step.bit > 255 || step.bit <= previous) return null;
    previous = step.bit;
    const rawPrefix = bytes(step.prefix, 32, "proof.steps.prefix");
    if (!equal(rawPrefix, prefix(key, step.bit))) return null;
    return { bit: step.bit, prefix: rawPrefix, sibling: hashBytes(step.sibling, "proof.steps.sibling") };
  });
  if (steps.includes(null)) return false;
  for (let index = steps.length - 1; index >= 0; index -= 1) {
    const step = steps[index];
    const left = keyBit(key, step.bit) ? step.sibling : current;
    const right = keyBit(key, step.bit) ? current : step.sibling;
    current = hash(BRANCH, le(BigInt(step.bit), 2), step.prefix, left, right);
  }
  return equal(hash(ROOT, le(count, 8), current), hashBytes(trustedRoot, "trustedRoot"));
}

export function verifyContractStateValueInclusionV1(proof, expectedPath, trustedRoot) {
  try {
    return verifyChecked(proof, expectedPath, trustedRoot);
  } catch {
    return false;
  }
}

/** Decode duplicate-key-free Torii JSON and verify membership under a trusted root. */
export function verifyContractStateValueInclusionJsonV1(payload, expectedPath, trustedRoot) {
  try {
    if (typeof payload !== "string" && !(payload instanceof Uint8Array)) return false;
    if (payload.length > MAX_WIRE_BYTES) return false;
    const source = typeof payload === "string"
      ? payload
      : new TextDecoder("utf-8", { fatal: true }).decode(payload);
    if (encoder.encode(source).length > MAX_WIRE_BYTES) return false;
    return verifyContractStateValueInclusionV1(
      parseStrictLosslessIntegerJson(source, "contract-state proof"),
      expectedPath,
      trustedRoot,
    );
  } catch {
    return false;
  }
}
