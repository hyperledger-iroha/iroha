import { blake2b256 } from "../blake2b.js";
import {
  isCanonicalKotodamaEntrypoint as isCanonicalEntrypointName,
  isCanonicalKotodamaIdentifier as isCanonicalIdentifier,
} from "../kotodamaIdentifiers.js";
import { analyzeEntrypointValueTypeV1 } from "../entrypointSchema.js";

const CONTRACT_HASH_DOMAIN = new TextEncoder().encode("iroha:ivm:contract-artifact:v1\0");
const DIAGNOSTIC_PHASES = new Set([
  "lex",
  "parse",
  "resolve",
  "semantic",
  "lowering",
  "artifact",
]);
const DIAGNOSTIC_SEVERITIES = new Set(["error", "warning"]);
const MANIFEST_ENTRYPOINT_KINDS = new Set([
  "Kotoage",
  "View",
  "Hajimari",
  "Kaizen",
]);
const MAX_DIAGNOSTICS = 64;
// Keep the allocation boundary independent from ledger admission. The exact
// deployable image limit is checked after the fixed IVM header is available.
const MAX_ARTIFACT_BYTES = 4 * 1024 * 1024;
const MAX_IVM_CODE_REGION_BYTES = 0x0010_0000;
const MAX_WIRE_JSON_BYTES = 16 * 1024 * 1024;
const MAX_MANIFEST_ITEMS = 65_536;
const MAX_ENTRYPOINT_PARAMETERS = 13;
const MAX_ENTRYPOINT_WORDS = 13;
const MAX_STRING_BYTES = 1024 * 1024;
const MAX_SOURCE_PATH_BYTES = 4096;
const MAX_JSON_DEPTH = 64;
const MAX_JSON_NODES = 65_536;
const U32_MAX = 0xffff_ffff;
const UTF8_ENCODER = new TextEncoder();
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });
// IVM ABI v1 authenticates the syscall descriptor directly in the fixed
// header: 17 execution bytes followed by the canonical 32-byte ABI hash.
const IVM_EXECUTION_HEADER_BYTES = 17;
const IVM_ABI_HASH_BYTES = 32;
const IVM_HEADER_BYTES = IVM_EXECUTION_HEADER_BYTES + IVM_ABI_HASH_BYTES;
const NORITO_FRAME_HEADER_BYTES = 40;
const TYPED_ARRAY_PROTOTYPE = Object.getPrototypeOf(Uint8Array.prototype);
const TYPED_ARRAY_TAG_GETTER = Object.getOwnPropertyDescriptor(
  TYPED_ARRAY_PROTOTYPE,
  Symbol.toStringTag,
)?.get;
const TYPED_ARRAY_BUFFER_GETTER = Object.getOwnPropertyDescriptor(
  TYPED_ARRAY_PROTOTYPE,
  "buffer",
)?.get;
const TYPED_ARRAY_BYTE_OFFSET_GETTER = Object.getOwnPropertyDescriptor(
  TYPED_ARRAY_PROTOTYPE,
  "byteOffset",
)?.get;
const TYPED_ARRAY_BYTE_LENGTH_GETTER = Object.getOwnPropertyDescriptor(
  TYPED_ARRAY_PROTOTYPE,
  "byteLength",
)?.get;
// `Archived<EmbeddedContractInterfaceV1>` is at most 8-byte aligned, and the
// 40-byte NRT0 header is already aligned. The Rust decoder requires this exact
// schema padding rather than the looser unknown-schema 64-byte fallback.
const NORITO_EMBEDDED_INTERFACE_PADDING_BYTES = 0;
const NORITO_COMPACT_LENGTHS_FLAG = 0x02;
const NORITO_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const NORITO_CRC64_POLYNOMIAL = 0xc96c5795d7870f42n;
const EMBEDDED_INTERFACE_SCHEMA_HASH = Uint8Array.from([
  0x42, 0x78, 0xc4, 0x14, 0x19, 0x7d, 0x68, 0xd9,
  0xcb, 0xb2, 0xda, 0xde, 0xa7, 0x40, 0x23, 0x87,
]);
const NORITO_CRC64_TABLE = Object.freeze(
  Array.from({ length: 256 }, (_, index) => {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 0n
        ? crc >> 1n
        : (crc >> 1n) ^ NORITO_CRC64_POLYNOMIAL;
    }
    return crc;
  }),
);

function isRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireRecord(value, label) {
  let record;
  try {
    record = isRecord(value);
  } catch {
    throw new TypeError(`${label} must be a plain data-only object`);
  }
  if (!record) {
    throw new TypeError(`${label} must be an object`);
  }
  let keys;
  try {
    keys = Reflect.ownKeys(value);
  } catch {
    throw new TypeError(`${label} must be a plain data-only object`);
  }
  for (const key of keys) {
    if (typeof key !== "string") {
      throw new TypeError(`${label} must not contain symbol fields`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !("value" in descriptor) || !descriptor.enumerable) {
      throw new TypeError(`${label}.${key} must be an enumerable data property`);
    }
  }
  return value;
}

function snapshotRecord(value, label) {
  requireRecord(value, label);
  let descriptors;
  try {
    descriptors = Object.getOwnPropertyDescriptors(value);
  } catch {
    throw new TypeError(`${label} must be a stable plain data-only object`);
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(descriptors)) {
    const descriptor = descriptors[key];
    if (typeof key !== "string" || !("value" in descriptor) || !descriptor.enumerable) {
      throw new TypeError(`${label} must be a stable plain data-only object`);
    }
    snapshot[key] = descriptor.value;
  }
  return snapshot;
}

function requireExactKeys(value, keys, label) {
  requireRecord(value, label);
  const actual = Reflect.ownKeys(value).sort();
  const expected = [...keys].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} has an invalid field set`);
  }
}

function requireDenseArray(value, label, maximum = MAX_MANIFEST_ITEMS) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${label} must be an array`);
  }
  let length;
  let keys;
  try {
    const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
    length = lengthDescriptor?.value;
    keys = Reflect.ownKeys(value);
  } catch {
    throw new TypeError(`${label} must be a dense data-only array`);
  }
  if (!Number.isSafeInteger(length) || length < 0 || length > maximum) {
    throw new RangeError(`${label} must contain at most ${maximum} items`);
  }
  if (keys.some((key) => typeof key !== "string")) {
    throw new TypeError(`${label} must not contain symbol fields`);
  }
  const elementKeys = keys.filter((key) => key !== "length");
  if (elementKeys.length !== length) {
    throw new TypeError(`${label} must be a dense array without extra fields`);
  }
  for (let index = 0; index < length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (descriptor === undefined || !("value" in descriptor) || !descriptor.enumerable) {
      throw new TypeError(`${label} must be a dense data-only array`);
    }
  }
  return value;
}

function snapshotDenseArray(value, label, maximum = MAX_MANIFEST_ITEMS) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${label} must be an array`);
  }
  let descriptors;
  try {
    descriptors = Object.getOwnPropertyDescriptors(value);
  } catch {
    throw new TypeError(`${label} must be a stable dense data-only array`);
  }
  const length = descriptors.length?.value;
  if (!Number.isSafeInteger(length) || length < 0 || length > maximum) {
    throw new RangeError(`${label} must contain at most ${maximum} items`);
  }
  if (Reflect.ownKeys(descriptors).length !== length + 1) {
    throw new TypeError(`${label} must be a dense array without extra fields`);
  }
  const snapshot = [];
  for (let index = 0; index < length; index += 1) {
    const descriptor = descriptors[index];
    if (descriptor === undefined || !("value" in descriptor) || !descriptor.enumerable) {
      throw new TypeError(`${label} must be a stable dense data-only array`);
    }
    snapshot.push(descriptor.value);
  }
  return snapshot;
}

function validateUnicodeScalarString(value) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!Number.isInteger(next) || next < 0xdc00 || next > 0xdfff) return false;
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function requireString(value, label, { allowEmpty = false, maximum = MAX_STRING_BYTES } = {}) {
  if (typeof value !== "string" || (!allowEmpty && value.length === 0)) {
    throw new TypeError(`${label} must be ${allowEmpty ? "a" : "a non-empty"} string`);
  }
  if (!validateUnicodeScalarString(value)) {
    throw new TypeError(`${label} must contain valid Unicode scalar values`);
  }
  if (UTF8_ENCODER.encode(value).length > maximum) {
    throw new RangeError(`${label} exceeds the ${maximum}-byte limit`);
  }
  return value;
}

function requireUnsignedInteger(value, maximum, label) {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new TypeError(`${label} must be an unsigned safe integer in 0..${maximum}`);
  }
  return value;
}

function requireNullableString(value, label, options) {
  return value === null ? null : requireString(value, label, options);
}

function requireStringArray(value, label, maximum = MAX_MANIFEST_ITEMS) {
  requireDenseArray(value, label, maximum);
  return value.map((entry, index) => requireString(entry, `${label}[${index}]`));
}

function validateBoundedJson(value, label) {
  const stack = [{ value, depth: 0, label }];
  let nodes = 0;
  while (stack.length !== 0) {
    const current = stack.pop();
    nodes += 1;
    if (nodes > MAX_JSON_NODES) {
      throw new RangeError(`${label} exceeds the ${MAX_JSON_NODES}-node JSON limit`);
    }
    if (current.depth > MAX_JSON_DEPTH) {
      throw new RangeError(`${label} exceeds the ${MAX_JSON_DEPTH}-level JSON depth limit`);
    }
    const item = current.value;
    if (item === null || typeof item === "boolean") continue;
    if (typeof item === "string") {
      requireString(item, current.label, { allowEmpty: true });
      continue;
    }
    if (typeof item === "number") {
      if (!Number.isFinite(item)) {
        throw new TypeError(`${current.label} must contain only finite JSON numbers`);
      }
      continue;
    }
    if (Array.isArray(item)) {
      requireDenseArray(item, current.label);
      for (let index = item.length - 1; index >= 0; index -= 1) {
        stack.push({
          value: item[index],
          depth: current.depth + 1,
          label: `${current.label}[${index}]`,
        });
      }
      continue;
    }
    requireRecord(item, current.label);
    for (const key of Reflect.ownKeys(item)) {
      requireString(key, `${current.label} key`, { maximum: MAX_SOURCE_PATH_BYTES });
      stack.push({
        value: item[key],
        depth: current.depth + 1,
        label: `${current.label}.${key}`,
      });
    }
  }
  return value;
}

function parseJson(raw, label) {
  requireString(raw, label, { allowEmpty: true, maximum: MAX_WIRE_JSON_BYTES });
  try {
    return JSON.parse(raw);
  } catch {
    throw new TypeError(`${label} is not valid JSON`);
  }
}

function normalizeHashHex(value, label) {
  if (typeof value !== "string") {
    throw new TypeError(`Kotodama compiler response is missing ${label}`);
  }
  if (/^[0-9a-fA-F]{64}$/u.test(value)) {
    return requireIrohaHashMarker(value.toLowerCase(), label);
  }
  const literal = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(value);
  if (literal === null) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid or noncanonical ${label}`,
    );
  }
  const [, body, checksum] = literal;
  const expected = crc16Literal("hash", body);
  if (checksum !== expected) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid ${label} checksum; expected ${expected}`,
    );
  }
  return requireIrohaHashMarker(body.toLowerCase(), label);
}

function requireIrohaHashMarker(hex, label) {
  if ((Number.parseInt(hex.slice(-2), 16) & 1) !== 1) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid ${label} marker bit`,
    );
  }
  return hex;
}

function crc16Literal(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let index = 0; index < 8; index += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  };
  for (const byte of UTF8_ENCODER.encode(tag)) {
    processByte(byte);
  }
  processByte(0x3a);
  for (const byte of UTF8_ENCODER.encode(body)) {
    processByte(byte);
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}

function toHex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function snapshotUint8Array(value) {
  if (!ArrayBuffer.isView(value)) return null;
  try {
    if (TYPED_ARRAY_TAG_GETTER.call(value) !== "Uint8Array") return null;
    const buffer = TYPED_ARRAY_BUFFER_GETTER.call(value);
    const byteOffset = TYPED_ARRAY_BYTE_OFFSET_GETTER.call(value);
    const byteLength = TYPED_ARRAY_BYTE_LENGTH_GETTER.call(value);
    if (byteLength > MAX_ARTIFACT_BYTES) {
      throw new RangeError(
        `Kotodama compiler artifactBytes must contain 1..${MAX_ARTIFACT_BYTES} bytes`,
      );
    }
    return new Uint8Array(buffer, byteOffset, byteLength).slice();
  } catch (error) {
    if (error instanceof RangeError) throw error;
    throw new TypeError("Kotodama compiler artifactBytes must be a readable Uint8Array");
  }
}

function normalizeArtifactBytes(value) {
  let bytes;
  const byteView = snapshotUint8Array(value);
  if (byteView !== null) {
    bytes = byteView;
  } else if (Array.isArray(value)) {
    const snapshot = snapshotDenseArray(
      value,
      "Kotodama compiler artifactBytes",
      MAX_ARTIFACT_BYTES,
    );
    if (snapshot.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
      throw new TypeError("Kotodama compiler artifactBytes must contain only bytes");
    }
    bytes = Uint8Array.from(snapshot);
  } else {
    throw new TypeError("Kotodama compiler response is missing artifactBytes");
  }
  if (bytes.length === 0 || bytes.length > MAX_ARTIFACT_BYTES) {
    throw new TypeError(
      `Kotodama compiler artifactBytes must contain 1..${MAX_ARTIFACT_BYTES} bytes`,
    );
  }
  return bytes;
}

function readU32Le(bytes, offset, label) {
  if (offset < 0 || offset + 4 > bytes.length) {
    throw new TypeError(`${label} is truncated`);
  }
  return (
    bytes[offset] |
    (bytes[offset + 1] << 8) |
    (bytes[offset + 2] << 16) |
    (bytes[offset + 3] * 0x1000000)
  ) >>> 0;
}

function readU32Be(bytes, offset, label) {
  if (offset < 0 || offset + 4 > bytes.length) {
    throw new TypeError(`${label} is truncated`);
  }
  return (
    bytes[offset] * 0x1000000 +
    (bytes[offset + 1] << 16) +
    (bytes[offset + 2] << 8) +
    bytes[offset + 3]
  ) >>> 0;
}

function readU64Le(bytes, offset, label) {
  if (offset < 0 || offset + 8 > bytes.length) {
    throw new TypeError(`${label} is truncated`);
  }
  let value = 0n;
  for (let index = 7; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[offset + index]);
  }
  return value;
}

function noritoCrc64(bytes) {
  let crc = NORITO_CRC64_MASK;
  for (const byte of bytes) {
    crc = NORITO_CRC64_TABLE[Number((crc ^ BigInt(byte)) & 0xffn)] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ NORITO_CRC64_MASK);
}

function equalBytes(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function readCompactLength(bytes, state, label) {
  let value = 0n;
  let shift = 0n;
  const start = state.offset;
  for (;;) {
    if (state.offset >= bytes.length || state.offset - start >= 8) {
      throw new TypeError(`${label} contains a truncated or oversized compact length`);
    }
    const byte = bytes[state.offset];
    state.offset += 1;
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      if (state.offset - start > 1 && byte === 0) {
        throw new TypeError(`${label} contains a noncanonical compact length`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new RangeError(`${label} compact length exceeds the safe integer range`);
      }
      return Number(value);
    }
    shift += 7n;
  }
}

function readCompactField(bytes, state, label) {
  const length = readCompactLength(bytes, state, `${label}.length`);
  const end = state.offset + length;
  if (end > bytes.length) {
    throw new TypeError(`${label} payload is truncated`);
  }
  const field = bytes.subarray(state.offset, end);
  state.offset = end;
  return field;
}

function decodeEmbeddedString(field, label) {
  const state = { offset: 0 };
  const encoded = readCompactField(field, state, label);
  if (state.offset !== field.length) {
    throw new TypeError(`${label} contains trailing bytes`);
  }
  try {
    return UTF8_DECODER.decode(encoded);
  } catch {
    throw new TypeError(`${label} is not valid UTF-8`);
  }
}

function validateEmbeddedInterfaceFrame(frame, manifest, headerMode, abiHashHex) {
  const label = "Kotodama embedded contract interface";
  if (frame.length < NORITO_FRAME_HEADER_BYTES) {
    throw new TypeError(`${label} is shorter than its Norito frame header`);
  }
  if (!equalBytes(frame.subarray(0, 4), Uint8Array.from([0x4e, 0x52, 0x54, 0x30]))) {
    throw new TypeError(`${label} is not an NRT0 frame`);
  }
  if (frame[4] !== 0 || frame[5] !== 0) {
    throw new TypeError(`${label} uses an unsupported Norito version`);
  }
  if (!equalBytes(frame.subarray(6, 22), EMBEDDED_INTERFACE_SCHEMA_HASH)) {
    throw new TypeError(`${label} has the wrong Norito schema hash`);
  }
  if (frame[22] !== 0 || frame[39] !== NORITO_COMPACT_LENGTHS_FLAG) {
    throw new TypeError(`${label} must use canonical uncompressed compact-length framing`);
  }
  const payloadLength = readU64Le(frame, 23, `${label} payload length`);
  if (payloadLength === 0n || payloadLength > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new TypeError(`${label} has an invalid payload length`);
  }
  const safePayloadLength = Number(payloadLength);
  const paddingLength = frame.length - NORITO_FRAME_HEADER_BYTES - safePayloadLength;
  if (paddingLength !== NORITO_EMBEDDED_INTERFACE_PADDING_BYTES) {
    throw new TypeError(`${label} has a noncanonical alignment padding length`);
  }
  const payloadOffset = NORITO_FRAME_HEADER_BYTES + paddingLength;
  if (frame.subarray(NORITO_FRAME_HEADER_BYTES, payloadOffset).some((byte) => byte !== 0)) {
    throw new TypeError(`${label} contains non-zero alignment padding`);
  }
  const payload = frame.subarray(payloadOffset);
  if (payload.length !== safePayloadLength) {
    throw new TypeError(`${label} payload is truncated`);
  }
  if (noritoCrc64(payload) !== readU64Le(frame, 31, `${label} CRC64`)) {
    throw new TypeError(`${label} has an invalid CRC64`);
  }

  const state = { offset: 0 };
  const fields = Array.from({ length: 9 }, (_, index) =>
    readCompactField(payload, state, `${label}.field${index}`));
  if (state.offset !== payload.length) {
    throw new TypeError(`${label} contains trailing or unknown fields`);
  }
  const embeddedName = decodeEmbeddedString(fields[0], `${label}.seiyaku_name`);
  const embeddedFingerprint = decodeEmbeddedString(
    fields[1],
    `${label}.compiler_fingerprint`,
  );
  if (fields[2].length !== IVM_ABI_HASH_BYTES || toHex(fields[2]) !== abiHashHex) {
    throw new TypeError(
      "Kotodama embedded contract interface ABI hash does not match the compiler response",
    );
  }
  const embeddedFeatures = readU64Le(fields[3], 0, `${label}.features_bitmap`);
  if (fields[3].length !== 8 || embeddedFeatures > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new TypeError(`${label}.features_bitmap is not a canonical safe u64`);
  }
  if (
    embeddedName !== manifest.seiyaku_name ||
    embeddedFingerprint !== manifest.compiler_fingerprint ||
    Number(embeddedFeatures) !== manifest.features_bitmap
  ) {
    throw new TypeError(
      "Kotodama manifest identity/capabilities do not match the embedded contract interface",
    );
  }
  if (Number(embeddedFeatures) !== (headerMode & 0x03)) {
    throw new TypeError(
      "Kotodama embedded contract capabilities do not match the IVM execution header",
    );
  }

  const optionPresent = (field, optionLabel) => {
    if (field.length === 1 && field[0] === 0) return false;
    if (field.length >= 3 && field[0] === 1) {
      const optionState = { offset: 1 };
      readCompactField(field, optionState, `${optionLabel}.value`);
      if (optionState.offset === field.length) return true;
    }
    throw new TypeError(`${optionLabel} has a noncanonical option envelope`);
  };
  const vectorCount = (field, vectorLabel) => {
    const count = readU64Le(field, 0, `${vectorLabel}.count`);
    if (count > BigInt(MAX_MANIFEST_ITEMS)) {
      throw new RangeError(`${vectorLabel} exceeds the ${MAX_MANIFEST_ITEMS}-item limit`);
    }
    const vectorState = { offset: 8 };
    for (let index = 0; index < Number(count); index += 1) {
      readCompactField(field, vectorState, `${vectorLabel}[${index}]`);
    }
    if (vectorState.offset !== field.length) {
      throw new TypeError(`${vectorLabel} has trailing or missing vector bytes`);
    }
    return Number(count);
  };
  const expectedAccessHints = manifest.access_set_hints !== null;
  if (optionPresent(fields[4], `${label}.access_set_hints`) !== expectedAccessHints) {
    throw new TypeError("Kotodama manifest access hints do not match the embedded interface");
  }
  for (const [fieldIndex, manifestValue, fieldLabel] of [
    [5, manifest.kotoba ?? [], "kotoba"],
    [6, manifest.entrypoints, "entrypoints"],
    [7, manifest.states, "states"],
    [8, manifest.error_codes ?? [], "error_codes"],
  ]) {
    if (vectorCount(fields[fieldIndex], `${label}.${fieldLabel}`) !== manifestValue.length) {
      throw new TypeError(
        `Kotodama manifest ${fieldLabel} count does not match the embedded interface`,
      );
    }
  }
}

function validateLiteralSection(bytes, start) {
  const label = "Kotodama IVM literal section";
  if (start + 16 > bytes.length) throw new TypeError(`${label} is truncated`);
  const count = readU32Le(bytes, start + 4, `${label} count`);
  const padding = readU32Le(bytes, start + 8, `${label} padding`);
  const dataLength = readU32Le(bytes, start + 12, `${label} data length`);
  if (count > 0x1_0000 || padding > 3) {
    throw new TypeError(`${label} has invalid bounds`);
  }
  const entriesLength = count * 8;
  const dataStart = start + 16 + entriesLength;
  const dataEnd = dataStart + dataLength;
  const codeOffset = dataEnd + padding;
  if (dataEnd < dataStart || codeOffset > bytes.length) {
    throw new TypeError(`${label} exceeds the artifact bounds`);
  }
  const expectedPadding = (4 - ((start - IVM_HEADER_BYTES + 16 + entriesLength + dataLength) % 4)) % 4;
  if (
    padding !== expectedPadding ||
    bytes.subarray(dataEnd, codeOffset).some((byte) => byte !== 0)
  ) {
    throw new TypeError(`${label} uses noncanonical alignment padding`);
  }
  const descriptors = [];
  for (let index = 0; index < count; index += 1) {
    const descriptor = readU64Le(
      bytes,
      start + 16 + index * 8,
      `${label} descriptor ${index}`,
    );
    const kind = Number(descriptor >> 56n);
    const relativeOffsetBigInt = descriptor & 0x00ff_ffff_ffff_ffffn;
    if (relativeOffsetBigInt > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError(`${label} descriptor ${index} offset is invalid`);
    }
    const relativeOffset = Number(relativeOffsetBigInt);
    const absoluteOffset = start + relativeOffset;
    if (
      (kind !== 0 && kind !== 1) ||
      relativeOffset < 16 + entriesLength ||
      absoluteOffset < dataStart ||
      absoluteOffset >= dataEnd
    ) {
      throw new TypeError(`${label} descriptor ${index} is invalid`);
    }
    if (
      descriptors.length !== 0 &&
      absoluteOffset <= descriptors[descriptors.length - 1].absoluteOffset
    ) {
      throw new TypeError(`${label} descriptor targets must be strictly increasing`);
    }
    descriptors.push({ kind, absoluteOffset });
  }
  if (descriptors.length === 0) {
    if (dataLength !== 0) {
      throw new TypeError(`${label} cannot contain unindexed literal data`);
    }
  } else if (descriptors[0].absoluteOffset !== dataStart) {
    throw new TypeError(`${label} first descriptor must target the first data byte`);
  }
  for (let index = 0; index < descriptors.length; index += 1) {
    const { kind, absoluteOffset } = descriptors[index];
    const end = descriptors[index + 1]?.absoluteOffset ?? dataEnd;
    const literal = bytes.subarray(absoluteOffset, end);
    if (kind === 0) {
      validatePointerLiteralV1(literal, `${label} descriptor ${index}`);
    } else if (literal.length !== 8) {
      throw new TypeError(`${label} i64 descriptor ${index} must contain exactly 8 bytes`);
    }
  }
  return codeOffset;
}

function validatePointerLiteralV1(bytes, label) {
  if (bytes.length < 39) {
    throw new TypeError(`${label} pointer TLV is truncated`);
  }
  const typeId = (bytes[0] << 8) | bytes[1];
  const allowedType =
    (typeId >= 0x0001 && typeId <= 0x000f) ||
    (typeId >= 0x0011 && typeId <= 0x0013);
  if (!allowedType) {
    throw new TypeError(`${label} pointer TLV type is not allowed by ABI v1`);
  }
  if (bytes[2] !== 1) {
    throw new TypeError(`${label} pointer TLV must use version 1`);
  }
  const payloadLength = readU32Be(bytes, 3, `${label} pointer TLV length`);
  const expectedLength = 7 + payloadLength + 32;
  if (bytes.length !== expectedLength) {
    throw new TypeError(`${label} pointer TLV length does not match its envelope`);
  }
  const payload = bytes.subarray(7, 7 + payloadLength);
  const expectedHash = blake2b256(payload);
  expectedHash[expectedHash.length - 1] |= 1;
  if (!equalBytes(bytes.subarray(7 + payloadLength), expectedHash)) {
    throw new TypeError(`${label} pointer TLV payload hash is invalid`);
  }
}

function validateCompiledArtifactV1(bytes, manifest, abiHashHex) {
  const label = "Kotodama compiler artifact";
  if (bytes.length < IVM_HEADER_BYTES + 8 + NORITO_FRAME_HEADER_BYTES + 4) {
    throw new TypeError(`${label} is too short to be a deployable IVM contract`);
  }
  if (bytes.length - IVM_HEADER_BYTES > MAX_IVM_CODE_REGION_BYTES) {
    throw new RangeError(
      `${label} post-header image exceeds the ${MAX_IVM_CODE_REGION_BYTES}-byte IVM code-memory limit`,
    );
  }
  if (!equalBytes(bytes.subarray(0, 4), Uint8Array.from([0x49, 0x56, 0x4d, 0x00]))) {
    throw new TypeError(`${label} has invalid IVM header magic`);
  }
  if (bytes[4] !== 1 || bytes[5] !== 1 || (bytes[6] & ~0x03) !== 0 || bytes[7] > 64) {
    throw new TypeError(`${label} has unsupported IVM execution metadata`);
  }
  if (bytes[16] !== 1) {
    throw new TypeError(`${label} must use IVM ABI version 1`);
  }
  if (toHex(bytes.subarray(IVM_EXECUTION_HEADER_BYTES, IVM_HEADER_BYTES)) !== abiHashHex) {
    throw new TypeError(`${label} authenticated ABI hash does not match abiHash`);
  }
  if (!equalBytes(
    bytes.subarray(IVM_HEADER_BYTES, IVM_HEADER_BYTES + 4),
    Uint8Array.from([0x43, 0x4e, 0x54, 0x52]),
  )) {
    throw new TypeError(`${label} is missing its required CNTR interface section`);
  }
  const interfaceLength = readU32Le(
    bytes,
    IVM_HEADER_BYTES + 4,
    `${label} CNTR length`,
  );
  const interfaceStart = IVM_HEADER_BYTES + 8;
  const interfaceEnd = interfaceStart + interfaceLength;
  if (interfaceLength === 0 || interfaceEnd < interfaceStart || interfaceEnd > bytes.length) {
    throw new TypeError(`${label} has an invalid CNTR interface length`);
  }
  validateEmbeddedInterfaceFrame(
    bytes.subarray(interfaceStart, interfaceEnd),
    manifest,
    bytes[6],
    abiHashHex,
  );
  let codeOffset = interfaceEnd;
  if (equalBytes(bytes.subarray(codeOffset, codeOffset + 4), Uint8Array.from([0x44, 0x42, 0x47, 0x31]))) {
    throw new TypeError(`${label} must keep DBG1 metadata in authenticated sidecars`);
  }
  if (equalBytes(bytes.subarray(codeOffset, codeOffset + 4), Uint8Array.from([0x4c, 0x54, 0x4c, 0x42]))) {
    codeOffset = validateLiteralSection(bytes, codeOffset);
  }
  const codeLength = bytes.length - codeOffset;
  if (codeLength <= 0 || codeLength % 4 !== 0) {
    throw new TypeError(`${label} must contain a non-empty word-aligned instruction stream`);
  }
}

function artifactHashHex(artifactBytes) {
  const input = new Uint8Array(CONTRACT_HASH_DOMAIN.length + artifactBytes.length);
  input.set(CONTRACT_HASH_DOMAIN);
  input.set(artifactBytes, CONTRACT_HASH_DOMAIN.length);
  const digest = blake2b256(input);
  // `iroha_crypto::Hash::prehashed` reserves the low bit of the final byte.
  digest[digest.length - 1] |= 1;
  return toHex(digest);
}

/**
 * Verify a detached compiler artifact and manifest at a deployment boundary.
 *
 * This intentionally reuses the same strict V1 checks as the compiler-client
 * response normalizer: the complete domain-separated code identity, canonical
 * manifest fields, authenticated ABI header, CNTR frame, literal section, and
 * word-aligned executable stream must all agree before upload instructions are
 * built.
 */
export function verifyCompiledContractArtifact(
  artifactBytes,
  manifest,
  codeHash,
  abiHash,
) {
  const normalizedArtifact = normalizeArtifactBytes(artifactBytes);
  const normalizedManifest = snapshotRecord(
    manifest,
    "Kotodama deployment manifest",
  );
  const codeHashHex = normalizeHashHex(codeHash, "codeHash");
  const abiHashHex = normalizeHashHex(abiHash, "abiHash");
  if (artifactHashHex(normalizedArtifact) !== codeHashHex) {
    throw new Error("Kotodama compiler artifact bytes do not match codeHash");
  }
  if (
    normalizeHashHex(normalizedManifest.code_hash, "manifest code_hash") !==
    codeHashHex
  ) {
    throw new Error("Kotodama compiler manifest code_hash does not match the artifact");
  }
  if (
    normalizeHashHex(normalizedManifest.abi_hash, "manifest abi_hash") !==
    abiHashHex
  ) {
    throw new Error("Kotodama compiler manifest abi_hash does not match abiHash");
  }
  validateCompilerManifest(normalizedManifest);
  validateCompiledArtifactV1(
    normalizedArtifact,
    normalizedManifest,
    abiHashHex,
  );
  return Object.freeze({
    artifactBytes: normalizedArtifact,
    manifest: normalizedManifest,
    codeHashHex,
    abiHashHex,
  });
}

function requireCanonicalBase64(value, label) {
  requireString(value, label);
  if (
    value.length % 4 !== 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)
  ) {
    throw new TypeError(`${label} must be exact standard-base64`);
  }
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  if (value.endsWith("==") && (alphabet.indexOf(value.at(-3)) & 0x0f) !== 0) {
    throw new TypeError(`${label} must use canonical base64 padding bits`);
  }
  if (value.endsWith("=") && !value.endsWith("==") && (alphabet.indexOf(value.at(-2)) & 0x03) !== 0) {
    throw new TypeError(`${label} must use canonical base64 padding bits`);
  }
  return value;
}

function validateSourceLocation(value, label, { nullable = false } = {}) {
  if (nullable && value.source_id === null) {
    for (const key of ["byte_start", "byte_end", "line", "column"]) {
      if (value[key] !== null) {
        throw new TypeError(`${label} must use one consistent nullable source location`);
      }
    }
    if (value.source_path !== null) {
      throw new TypeError(`${label}.source_path must be null without a source location`);
    }
    return;
  }
  requireNullableString(value.source_path, `${label}.source_path`, {
    maximum: MAX_SOURCE_PATH_BYTES,
  });
  requireUnsignedInteger(value.source_id, U32_MAX, `${label}.source_id`);
  requireUnsignedInteger(value.byte_start, U32_MAX, `${label}.byte_start`);
  requireUnsignedInteger(value.byte_end, U32_MAX, `${label}.byte_end`);
  requireUnsignedInteger(value.line, U32_MAX, `${label}.line`);
  requireUnsignedInteger(value.column, U32_MAX, `${label}.column`);
  if (value.byte_start > value.byte_end) {
    throw new TypeError(`${label} must use a forward UTF-8 byte range`);
  }
}

function validateSourceMapEntry(value, index) {
  const label = `source-map sidecar entry ${index}`;
  requireExactKeys(
    value,
    [
      "function_name",
      "pc_start",
      "pc_end",
      "source_path",
      "source_id",
      "byte_start",
      "byte_end",
      "line",
      "column",
    ],
    label,
  );
  requireString(value.function_name, `${label}.function_name`);
  requireUnsignedInteger(value.pc_start, Number.MAX_SAFE_INTEGER, `${label}.pc_start`);
  requireUnsignedInteger(value.pc_end, Number.MAX_SAFE_INTEGER, `${label}.pc_end`);
  if (value.pc_start > value.pc_end) {
    throw new TypeError(`${label} must use a forward PC range`);
  }
  validateSourceLocation(value, label);
}

function validateBudgetEntry(value, index) {
  const label = `budget sidecar entry ${index}`;
  requireExactKeys(
    value,
    [
      "function_name",
      "pc_start",
      "pc_end",
      "bytecode_bytes",
      "bytecode_words",
      "frame_bytes",
      "jump_span_words",
      "jump_range_risk",
      "source_path",
      "source_id",
      "byte_start",
      "byte_end",
      "line",
      "column",
    ],
    label,
  );
  requireString(value.function_name, `${label}.function_name`);
  requireUnsignedInteger(value.pc_start, Number.MAX_SAFE_INTEGER, `${label}.pc_start`);
  requireUnsignedInteger(value.pc_end, Number.MAX_SAFE_INTEGER, `${label}.pc_end`);
  for (const key of ["bytecode_bytes", "bytecode_words", "frame_bytes", "jump_span_words"]) {
    requireUnsignedInteger(value[key], U32_MAX, `${label}.${key}`);
  }
  if (typeof value.jump_range_risk !== "boolean") {
    throw new TypeError(`${label}.jump_range_risk must be a boolean`);
  }
  if (value.pc_start > value.pc_end) {
    throw new TypeError(`${label} must use a forward PC range`);
  }
  validateSourceLocation(value, label, { nullable: true });
}

function parseSidecar(raw, kind, artifactHash) {
  const label = `${kind} sidecar`;
  const sidecar = requireRecord(parseJson(raw, label), label);
  const expectedKeys = kind === "budget"
    ? ["sidecar_version", "kind", "artifact_hash", "entries", "access_hint_diagnostics"]
    : ["sidecar_version", "kind", "artifact_hash", "entries"];
  requireExactKeys(sidecar, expectedKeys, label);
  if (
    sidecar.sidecar_version !== 1 ||
    sidecar.kind !== kind ||
    normalizeHashHex(sidecar.artifact_hash, `${kind} artifact hash`) !== artifactHash
  ) {
    throw new Error(`Kotodama compiler returned an invalid or mismatched ${kind} sidecar`);
  }
  requireDenseArray(sidecar.entries, `${label}.entries`);
  sidecar.entries.forEach(kind === "source-map" ? validateSourceMapEntry : validateBudgetEntry);
  if (kind === "budget") {
    requireExactKeys(
      sidecar.access_hint_diagnostics,
      ["state_wildcards", "isi_wildcards", "literal_trigger_spec_decode_failures"],
      `${label}.access_hint_diagnostics`,
    );
    for (const key of [
      "state_wildcards",
      "isi_wildcards",
      "literal_trigger_spec_decode_failures",
    ]) {
      requireUnsignedInteger(
        sidecar.access_hint_diagnostics[key],
        Number.MAX_SAFE_INTEGER,
        `${label}.access_hint_diagnostics.${key}`,
      );
    }
  }
  return sidecar.entries;
}

function validateEntrypointType(value, label, maximumWords = MAX_ENTRYPOINT_WORDS) {
  validateBoundedJson(value, label);
  const analysis = analyzeEntrypointValueTypeV1(value, label);
  if (analysis.wordCount > maximumWords) {
    throw new TypeError(`${label} exceeds the ${maximumWords}-word V1 ABI limit`);
  }
  return analysis;
}

function validateArgumentSchema(value, params, label) {
  requireExactKeys(value, ["fields"], label);
  requireDenseArray(value.fields, `${label}.fields`, MAX_ENTRYPOINT_PARAMETERS);
  if (value.fields.length === 0 || value.fields.length !== params.length) {
    throw new TypeError(`${label}.fields must exactly match the declared parameters`);
  }
  const names = new Set();
  let words = 0;
  value.fields.forEach((field, index) => {
    const fieldLabel = `${label}.fields[${index}]`;
    requireExactKeys(field, ["name", "ty"], fieldLabel);
    if (!isCanonicalIdentifier(field.name) || names.has(field.name)) {
      throw new TypeError(`${fieldLabel}.name must be unique and canonical`);
    }
    names.add(field.name);
    const analysis = validateEntrypointType(field.ty, `${fieldLabel}.ty`);
    words += analysis.wordCount;
    if (field.name !== params[index].name || analysis.canonicalName !== params[index].type_name) {
      throw new TypeError(`${fieldLabel} does not match its declared parameter`);
    }
  });
  if (words > MAX_ENTRYPOINT_WORDS) {
    throw new TypeError(`${label} exceeds the ${MAX_ENTRYPOINT_WORDS}-word V1 ABI limit`);
  }
}

function validateDynamicAccessHints(value, label) {
  requireDenseArray(value, label);
  value.forEach((hint, index) => {
    const hintLabel = `${label}[${index}]`;
    requireExactKeys(hint, ["base_key", "key_type", "bound_kind", "max_keys"], hintLabel);
    requireString(hint.base_key, `${hintLabel}.base_key`);
    requireString(hint.key_type, `${hintLabel}.key_type`);
    requireString(hint.bound_kind, `${hintLabel}.bound_kind`);
    requireUnsignedInteger(hint.max_keys, U32_MAX, `${hintLabel}.max_keys`);
  });
}

function validateAccessSetHints(value, label) {
  if (value === null) return;
  requireExactKeys(
    value,
    ["read_keys", "write_keys", "dynamic_reads", "dynamic_writes"],
    label,
  );
  requireStringArray(value.read_keys, `${label}.read_keys`);
  requireStringArray(value.write_keys, `${label}.write_keys`);
  validateDynamicAccessHints(value.dynamic_reads, `${label}.dynamic_reads`);
  validateDynamicAccessHints(value.dynamic_writes, `${label}.dynamic_writes`);
}

function validateTriggerRepeats(value, label) {
  requireRecord(value, label);
  const keys = Reflect.ownKeys(value);
  if (keys.length !== 1 || !["Indefinitely", "Exactly"].includes(keys[0])) {
    throw new TypeError(`${label} must contain exactly one canonical repeat policy`);
  }
  if (keys[0] === "Indefinitely") {
    if (value.Indefinitely !== null) {
      throw new TypeError(`${label}.Indefinitely must be null`);
    }
  } else {
    requireUnsignedInteger(value.Exactly, U32_MAX, `${label}.Exactly`);
  }
}

function validateTriggers(value, entrypointName, label) {
  requireDenseArray(value, label);
  const ids = new Set();
  value.forEach((trigger, index) => {
    const triggerLabel = `${label}[${index}]`;
    requireExactKeys(
      trigger,
      ["id", "repeats", "filter", "authority", "metadata", "callback"],
      triggerLabel,
    );
    if (!isCanonicalIdentifier(trigger.id) || ids.has(trigger.id)) {
      throw new TypeError(`${triggerLabel}.id must be unique and canonical`);
    }
    ids.add(trigger.id);
    validateTriggerRepeats(trigger.repeats, `${triggerLabel}.repeats`);
    requireCanonicalBase64(trigger.filter, `${triggerLabel}.filter`);
    requireNullableString(trigger.authority, `${triggerLabel}.authority`);
    requireRecord(trigger.metadata, `${triggerLabel}.metadata`);
    validateBoundedJson(trigger.metadata, `${triggerLabel}.metadata`);
    requireExactKeys(trigger.callback, ["namespace", "entrypoint"], `${triggerLabel}.callback`);
    requireNullableString(trigger.callback.namespace, `${triggerLabel}.callback.namespace`);
    if (!isCanonicalEntrypointName(trigger.callback.entrypoint)) {
      throw new TypeError(`${triggerLabel}.callback.entrypoint must be canonical`);
    }
    if (trigger.callback.namespace === null && trigger.callback.entrypoint !== entrypointName) {
      throw new TypeError(`${triggerLabel}.callback must target its declaring entrypoint`);
    }
  });
}

function validateCompilerEntrypoint(entry, index, names, lifecycleKinds) {
  const label = `Kotodama manifest entrypoint ${index}`;
  requireExactKeys(
    entry,
    [
      "name",
      "kind",
      "params",
      "argument_schema",
      "return_type",
      "return_schema",
      "permission",
      "read_keys",
      "write_keys",
      "access_hints_complete",
      "access_hints_skipped",
      "triggers",
    ],
    label,
  );
  requireExactKeys(entry.kind, ["kind", "value"], `${label}.kind`);
  if (!isCanonicalEntrypointName(entry.name)) {
    throw new TypeError(
      `${label}.name is not a canonical V1 identifier or branded lifecycle selector`,
    );
  }
  if (names.has(entry.name)) {
    throw new TypeError(`Kotodama manifest contains duplicate entrypoint ${entry.name}`);
  }
  names.add(entry.name);
  if (!MANIFEST_ENTRYPOINT_KINDS.has(entry.kind.kind)) {
    throw new TypeError(`${label}.kind must be Kotoage, View, Hajimari, or Kaizen`);
  }
  if (entry.kind.value !== null) {
    throw new TypeError(`${label}.kind.value must be null`);
  }
  const lifecycleKind =
    entry.name === "hajimari" || entry.name === "始まり"
      ? "Hajimari"
      : entry.name === "kaizen" || entry.name === "改善"
        ? "Kaizen"
        : null;
  if (
    (lifecycleKind === null && ["Hajimari", "Kaizen"].includes(entry.kind.kind)) ||
    (lifecycleKind !== null && entry.kind.kind !== lifecycleKind)
  ) {
    throw new TypeError(`${label}.kind does not match its branded lifecycle selector`);
  }
  if (entry.kind.kind === "Kotoage" && (typeof entry.permission !== "string" || entry.permission.trim() === "")) {
    throw new TypeError(`${label} kotoage/言挙げ is missing caller authorization`);
  }
  if (["Hajimari", "Kaizen"].includes(entry.kind.kind) && entry.permission !== null) {
    throw new TypeError(`${label} hajimari/始まり and kaizen/改善 must use runtime authorization`);
  }
  if (entry.permission !== null) requireString(entry.permission, `${label}.permission`);
  if (lifecycleKind !== null) {
    if (lifecycleKinds.has(lifecycleKind)) {
      throw new TypeError(`Kotodama manifest contains duplicate ${lifecycleKind} entrypoints`);
    }
    lifecycleKinds.add(lifecycleKind);
  }

  requireDenseArray(entry.params, `${label}.params`, MAX_ENTRYPOINT_PARAMETERS);
  const paramNames = new Set();
  entry.params.forEach((param, paramIndex) => {
    const paramLabel = `${label}.params[${paramIndex}]`;
    requireExactKeys(param, ["name", "type_name"], paramLabel);
    if (!isCanonicalIdentifier(param.name) || paramNames.has(param.name)) {
      throw new TypeError(`${paramLabel}.name must be unique and canonical`);
    }
    paramNames.add(param.name);
    requireString(param.type_name, `${paramLabel}.type_name`);
  });
  if (entry.params.length === 0) {
    if (entry.argument_schema !== null) {
      throw new TypeError(`${label}.argument_schema must be null without parameters`);
    }
  } else {
    if (entry.argument_schema === null) {
      throw new TypeError(`${label}.argument_schema is required for declared parameters`);
    }
    validateArgumentSchema(entry.argument_schema, entry.params, `${label}.argument_schema`);
  }
  if ((entry.return_type === null) !== (entry.return_schema === null)) {
    throw new TypeError(`${label} return_type and return_schema must be present together`);
  }
  if (entry.return_schema !== null) {
    requireString(entry.return_type, `${label}.return_type`);
    const analysis = validateEntrypointType(entry.return_schema, `${label}.return_schema`);
    if (analysis.canonicalName !== entry.return_type) {
      throw new TypeError(`${label}.return_type does not match return_schema`);
    }
  }
  requireStringArray(entry.read_keys, `${label}.read_keys`);
  requireStringArray(entry.write_keys, `${label}.write_keys`);
  if (entry.access_hints_complete !== null && typeof entry.access_hints_complete !== "boolean") {
    throw new TypeError(`${label}.access_hints_complete must be a boolean or null`);
  }
  requireStringArray(entry.access_hints_skipped, `${label}.access_hints_skipped`);
  validateTriggers(entry.triggers, entry.name, `${label}.triggers`);
}

function validateCompilerManifestStates(states) {
  requireDenseArray(states, "Kotodama manifest states");
  const names = new Set();
  states.forEach((state, index) => {
    const label = `Kotodama manifest state ${index}`;
    requireExactKeys(state, ["name", "type_name"], label);
    if (!isCanonicalIdentifier(state.name, { declaration: true })) {
      throw new TypeError(`${label}.name is not canonical`);
    }
    if (names.has(state.name)) {
      throw new TypeError(`Kotodama manifest contains duplicate state ${state.name}`);
    }
    names.add(state.name);
    requireString(state.type_name, `${label}.type_name`);
  });
}

function validateCompilerManifestErrorCodes(errorCodes) {
  if (errorCodes === null) return;
  requireDenseArray(errorCodes, "Kotodama manifest error_codes");
  const paths = new Set();
  const codes = new Set();
  errorCodes.forEach((errorCode, index) => {
    const label = `Kotodama manifest error code ${index}`;
    requireExactKeys(errorCode, ["namespace", "name", "code"], label);
    if (
      !isCanonicalIdentifier(errorCode.namespace, { declaration: true }) ||
      !isCanonicalIdentifier(errorCode.name)
    ) {
      throw new TypeError(`${label} must use canonical namespace and variant identifiers`);
    }
    if (!Number.isSafeInteger(errorCode.code) || errorCode.code <= 0 || errorCode.code > U32_MAX) {
      throw new TypeError(`${label}.code must be a non-zero u32`);
    }
    const path = `${errorCode.namespace}::${errorCode.name}`;
    if (paths.has(path) || codes.has(errorCode.code)) {
      throw new TypeError(`Kotodama manifest contains a duplicate error path or code at ${path}`);
    }
    paths.add(path);
    codes.add(errorCode.code);
  });
}

function validateKotoba(value) {
  if (value === null) return;
  requireDenseArray(value, "Kotodama manifest kotoba");
  const messageIds = new Set();
  value.forEach((entry, index) => {
    const label = `Kotodama manifest kotoba[${index}]`;
    requireExactKeys(entry, ["msg_id", "translations"], label);
    requireString(entry.msg_id, `${label}.msg_id`);
    if (messageIds.has(entry.msg_id)) {
      throw new TypeError(`Kotodama manifest kotoba contains duplicate msg_id ${entry.msg_id}`);
    }
    messageIds.add(entry.msg_id);
    requireDenseArray(entry.translations, `${label}.translations`);
    const languages = new Set();
    entry.translations.forEach((translation, translationIndex) => {
      const translationLabel = `${label}.translations[${translationIndex}]`;
      requireExactKeys(translation, ["lang", "text"], translationLabel);
      requireString(translation.lang, `${translationLabel}.lang`);
      requireString(translation.text, `${translationLabel}.text`, { allowEmpty: true });
      if (languages.has(translation.lang)) {
        throw new TypeError(`${label} contains duplicate language ${translation.lang}`);
      }
      languages.add(translation.lang);
    });
  });
}

function validateProvenance(value) {
  if (value === null) return;
  // The canonical compiler currently emits no provenance. Accepting a
  // syntactically plausible signer/signature pair without verifying the exact
  // signed message and public-key algorithm would turn untrusted metadata into
  // a false authenticity claim. A later signed-manifest version must add full
  // cryptographic verification before this boundary accepts it.
  throw new TypeError(
    "Kotodama manifest provenance must be null until signed provenance is verifiable",
  );
}

function validateCompilerManifest(manifest) {
  requireRecord(manifest, "Kotodama manifest");
  if (Object.hasOwn(manifest, "contract_name")) {
    throw new TypeError(
      "Kotodama manifest must use seiyaku_name; contract_name is not a V1 field",
    );
  }
  requireExactKeys(
    manifest,
    [
      "seiyaku_name",
      "code_hash",
      "abi_hash",
      "compiler_fingerprint",
      "features_bitmap",
      "access_set_hints",
      "entrypoints",
      "states",
      "error_codes",
      "kotoba",
      "provenance",
    ],
    "Kotodama manifest",
  );
  if (!isCanonicalIdentifier(manifest.seiyaku_name, { declaration: true })) {
    throw new TypeError(
      "Kotodama manifest seiyaku_name must be a canonical V1 declaration identifier",
    );
  }
  requireString(manifest.compiler_fingerprint, "Kotodama manifest compiler_fingerprint");
  requireUnsignedInteger(
    manifest.features_bitmap,
    Number.MAX_SAFE_INTEGER,
    "Kotodama manifest features_bitmap",
  );
  validateAccessSetHints(manifest.access_set_hints, "Kotodama manifest access_set_hints");
  requireDenseArray(manifest.entrypoints, "Kotodama manifest entrypoints");
  const names = new Set();
  const lifecycleKinds = new Set();
  manifest.entrypoints.forEach((entry, index) =>
    validateCompilerEntrypoint(entry, index, names, lifecycleKinds));
  validateCompilerManifestStates(manifest.states);
  validateCompilerManifestErrorCodes(manifest.error_codes);
  validateKotoba(manifest.kotoba);
  validateProvenance(manifest.provenance);
}

function validatePosition(value, label) {
  requireExactKeys(value, ["line", "column"], label);
  if (
    !Number.isSafeInteger(value.line) ||
    value.line < 1 ||
    !Number.isSafeInteger(value.column) ||
    value.column < 1
  ) {
    throw new TypeError(`${label} must contain one-based safe-integer line and column values`);
  }
}

function validateSpan(value, label) {
  requireExactKeys(
    value,
    ["package_identity", "source", "start", "end", "byte_range"],
    label,
  );
  requireNullableString(value.package_identity, `${label}.package_identity`, {
    maximum: MAX_SOURCE_PATH_BYTES,
  });
  requireNullableString(value.source, `${label}.source`, { maximum: MAX_SOURCE_PATH_BYTES });
  validatePosition(value.start, `${label}.start`);
  validatePosition(value.end, `${label}.end`);
  const startsAfterEnd =
    value.start.line > value.end.line ||
    (value.start.line === value.end.line && value.start.column > value.end.column);
  if (startsAfterEnd) {
    throw new TypeError(`${label} must be a forward half-open range`);
  }
  if (value.byte_range !== null) {
    requireExactKeys(value.byte_range, ["start", "end"], `${label}.byte_range`);
    if (
      !Number.isSafeInteger(value.byte_range.start) ||
      value.byte_range.start < 0 ||
      !Number.isSafeInteger(value.byte_range.end) ||
      value.byte_range.end < value.byte_range.start
    ) {
      throw new TypeError(`${label}.byte_range must be a forward safe-integer byte range`);
    }
  }
}

function validateDiagnostic(value, index) {
  const label = `Kotodama diagnostic ${index}`;
  requireExactKeys(
    value,
    ["code", "severity", "phase", "message", "primary_span", "labels", "notes", "help", "fix"],
    label,
  );
  if (typeof value.code !== "string" || !/^[EK][A-Z0-9_]+$/.test(value.code)) {
    throw new TypeError(`${label}.code is not a stable Kotodama diagnostic code`);
  }
  if (!DIAGNOSTIC_SEVERITIES.has(value.severity)) {
    throw new TypeError(`${label}.severity is invalid`);
  }
  if (!DIAGNOSTIC_PHASES.has(value.phase)) {
    throw new TypeError(`${label}.phase is invalid`);
  }
  requireString(value.message, `${label}.message`);
  if (value.primary_span !== null) {
    validateSpan(value.primary_span, `${label}.primary_span`);
  }
  requireDenseArray(value.labels, `${label}.labels`);
  value.labels.forEach((entry, labelIndex) => {
    const entryLabel = `${label}.labels[${labelIndex}]`;
    requireExactKeys(entry, ["span", "message"], entryLabel);
    validateSpan(entry.span, `${entryLabel}.span`);
    requireString(entry.message, `${entryLabel}.message`, { allowEmpty: true });
  });
  requireStringArray(value.notes, `${label}.notes`);
  requireNullableString(value.help, `${label}.help`, { allowEmpty: true });
  if (value.fix !== null) {
    requireExactKeys(value.fix, ["span", "replacement"], `${label}.fix`);
    validateSpan(value.fix.span, `${label}.fix.span`);
    requireString(value.fix.replacement, `${label}.fix.replacement`, { allowEmpty: true });
  }
}

function parseDiagnostics(raw) {
  const diagnostics = parseJson(raw, "Kotodama diagnosticsJson");
  requireDenseArray(diagnostics, "Kotodama diagnostics", MAX_DIAGNOSTICS);
  if (diagnostics.length === 0) {
    throw new TypeError("failed Kotodama compilation must return a non-empty diagnostic array");
  }
  diagnostics.forEach(validateDiagnostic);
  if (!diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    throw new TypeError("failed Kotodama compilation must contain at least one error diagnostic");
  }
  return diagnostics;
}

/** Validate and normalize one successful canonical Rust compiler wire output. */
export function normalizeCompilerOutput(output) {
  output = snapshotRecord(output, "Kotodama compiler output");
  requireExactKeys(
    output,
    [
      "artifactBytes",
      "manifestJson",
      "codeHash",
      "abiHash",
      "sourceMapJson",
      "budgetReportJson",
    ],
    "Kotodama compiler output",
  );
  const artifactBytes = normalizeArtifactBytes(output.artifactBytes);
  const codeHashHex = normalizeHashHex(output.codeHash, "codeHash");
  const abiHashHex = normalizeHashHex(output.abiHash, "abiHash");
  const actualCodeHash = artifactHashHex(artifactBytes);
  if (actualCodeHash !== codeHashHex) {
    throw new Error("Kotodama compiler artifact bytes do not match codeHash");
  }

  const manifest = requireRecord(
    parseJson(output.manifestJson, "Kotodama manifestJson"),
    "Kotodama manifest",
  );
  if (normalizeHashHex(manifest.code_hash, "manifest code_hash") !== codeHashHex) {
    throw new Error("Kotodama compiler manifest code_hash does not match the artifact");
  }
  if (normalizeHashHex(manifest.abi_hash, "manifest abi_hash") !== abiHashHex) {
    throw new Error("Kotodama compiler manifest abi_hash does not match abiHash");
  }
  validateCompilerManifest(manifest);
  validateCompiledArtifactV1(artifactBytes, manifest, abiHashHex);

  const sourceMap = parseSidecar(output.sourceMapJson, "source-map", codeHashHex);
  const budgetReport = parseSidecar(output.budgetReportJson, "budget", codeHashHex);
  if (sourceMap.length !== budgetReport.length) {
    throw new TypeError("Kotodama compiler sidecars must describe the same functions");
  }
  sourceMap.forEach((sourceEntry, index) => {
    const budgetEntry = budgetReport[index];
    if (
      sourceEntry.function_name !== budgetEntry.function_name ||
      sourceEntry.pc_start !== budgetEntry.pc_start ||
      sourceEntry.pc_end !== budgetEntry.pc_end
    ) {
      throw new TypeError(
        `Kotodama compiler sidecar entry ${index} function identity does not match`,
      );
    }
  });
  return {
    artifactBytes,
    codeHashHex,
    abiHashHex,
    compilerFingerprint: manifest.compiler_fingerprint ?? "kotodama_lang",
    manifest,
    sourceMap,
    budgetReport,
  };
}

/**
 * Normalize the canonical Rust `Result<CompileOutput, DiagnosticBundle>` envelope.
 * Compiler failures remain structured data; malformed/internal failures throw.
 */
export function normalizeCompilerResult(result) {
  result = snapshotRecord(result, "Kotodama compiler result");
  requireExactKeys(
    result,
    ["ok", "output", "diagnosticsJson"],
    "Kotodama compiler result",
  );
  if (result.ok === true) {
    if (result.diagnosticsJson !== null) {
      throw new TypeError(
        "successful Kotodama compilation must contain an exact null diagnosticsJson sentinel",
      );
    }
    return { ok: true, output: normalizeCompilerOutput(result.output) };
  }
  if (result.ok === false) {
    if (result.output !== null) {
      throw new TypeError(
        "failed Kotodama compilation must contain an exact null output sentinel",
      );
    }
    return { ok: false, diagnostics: parseDiagnostics(result.diagnosticsJson) };
  }
  throw new TypeError("Kotodama compiler result.ok must be a boolean");
}
