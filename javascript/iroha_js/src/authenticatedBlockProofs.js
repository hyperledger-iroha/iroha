import { Buffer } from "node:buffer";

import { getNativeBinding } from "./native.js";
import { networkIdBytes } from "./networkId.js";

export const AUTHENTICATED_BLOCK_PROOFS_VERSION_V1 = 1;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 = 32 * 1024 * 1024;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1 = 9 * 1024 * 1024;
export const AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1 = 16 * 1024 * 1024;

const INPUT_KEYS = new Set([
  "version",
  "networkId",
  "trustedContextId",
  "expectedEntryHash",
  "previousFinalityProofNorito",
  "finalityProofNorito",
  "executedBlockWire",
  "blockProofsNorito",
]);
const REQUIRED_INPUT_KEYS = [
  "version",
  "networkId",
  "trustedContextId",
  "expectedEntryHash",
  "finalityProofNorito",
  "executedBlockWire",
  "blockProofsNorito",
];
const LOWER_HEX_32_PATTERN = /^[0-9a-f]{64}$/u;
const POSITIVE_DECIMAL_PATTERN = /^[1-9][0-9]*$/u;
const arrayBufferByteLengthGetter = Object.getOwnPropertyDescriptor(
  ArrayBuffer.prototype,
  "byteLength",
).get;
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedArrayBufferGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "buffer",
).get;
const typedArrayByteOffsetGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteOffset",
).get;
const typedArrayByteLengthGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteLength",
).get;
const typedArraySet = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "set",
).value;
const dataViewBufferGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "buffer",
).get;
const dataViewByteOffsetGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "byteOffset",
).get;
const dataViewByteLengthGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "byteLength",
).get;
const bufferToString = Buffer.prototype.toString;
const sharedArrayBufferByteLengthGetter =
  typeof SharedArrayBuffer === "undefined"
    ? null
    : Object.getOwnPropertyDescriptor(
        SharedArrayBuffer.prototype,
        "byteLength",
      ).get;

function isPlainObject(value) {
  if (value === null || typeof value !== "object") return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function snapshotInput(input) {
  if (!isPlainObject(input)) {
    throw new TypeError("authenticated BlockProofs input must be a plain object");
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(input)) {
    if (typeof key !== "string" || !INPUT_KEYS.has(key)) {
      throw new TypeError(
        `authenticated BlockProofs input contains unknown field ${String(key)}`,
      );
    }
    const descriptor = Object.getOwnPropertyDescriptor(input, key);
    if (
      descriptor === undefined ||
      !descriptor.enumerable ||
      !("value" in descriptor)
    ) {
      throw new TypeError(
        `authenticated BlockProofs input.${key} must be an enumerable data property`,
      );
    }
    snapshot[key] = descriptor.value;
  }
  for (const key of REQUIRED_INPUT_KEYS) {
    if (!(key in snapshot)) {
      throw new TypeError(`authenticated BlockProofs input is missing required field ${key}`);
    }
  }
  return snapshot;
}

function isSharedArrayBuffer(value) {
  if (sharedArrayBufferByteLengthGetter === null) return false;
  try {
    sharedArrayBufferByteLengthGetter.call(value);
    return true;
  } catch {
    return false;
  }
}

function isArrayBuffer(value) {
  try {
    arrayBufferByteLengthGetter.call(value);
    return true;
  } catch {
    return false;
  }
}

function arrayBufferViewInfo(value) {
  try {
    return {
      buffer: typedArrayBufferGetter.call(value),
      byteOffset: typedArrayByteOffsetGetter.call(value),
      byteLength: typedArrayByteLengthGetter.call(value),
    };
  } catch {
    try {
      return {
        buffer: dataViewBufferGetter.call(value),
        byteOffset: dataViewByteOffsetGetter.call(value),
        byteLength: dataViewByteLengthGetter.call(value),
      };
    } catch {
      return null;
    }
  }
}

function copyBoundedBytes(value, context, maximumBytes, exactBytes = null) {
  if (isSharedArrayBuffer(value)) {
    throw new TypeError(`${context} must not be backed by SharedArrayBuffer`);
  }
  let buffer;
  let byteOffset;
  let byteLength;
  if (isArrayBuffer(value)) {
    buffer = value;
    byteOffset = 0;
    byteLength = arrayBufferByteLengthGetter.call(value);
  } else {
    const view = arrayBufferViewInfo(value);
    if (view === null) {
      throw new TypeError(`${context} must be an ArrayBuffer or ArrayBuffer view`);
    }
    if (isSharedArrayBuffer(view.buffer)) {
      throw new TypeError(`${context} must not be backed by SharedArrayBuffer`);
    }
    ({ buffer, byteOffset, byteLength } = view);
  }
  if (exactBytes === null && (byteLength === 0 || byteLength > maximumBytes)) {
    throw new RangeError(`${context} must contain 1..${maximumBytes} bytes`);
  }
  if (exactBytes !== null && byteLength !== exactBytes) {
    throw new RangeError(`${context} must contain exactly ${exactBytes} bytes`);
  }
  const source = new Uint8Array(buffer, byteOffset, byteLength);
  const copy = new Uint8Array(byteLength);
  Reflect.apply(typedArraySet, copy, [source]);
  return Buffer.from(copy.buffer, copy.byteOffset, copy.byteLength);
}

function normalizeInput(input) {
  input = snapshotInput(input);
  if (input.version !== AUTHENTICATED_BLOCK_PROOFS_VERSION_V1) {
    throw new RangeError(
      `authenticated BlockProofs version must be ${AUTHENTICATED_BLOCK_PROOFS_VERSION_V1}`,
    );
  }
  networkIdBytes(input.networkId, "authenticated BlockProofs networkId");
  const trustedContextId = copyBoundedBytes(
    input.trustedContextId,
    "authenticated BlockProofs trustedContextId",
    32,
    32,
  );
  if ((trustedContextId[31] & 1) !== 1) {
    throw new TypeError(
      "authenticated BlockProofs trustedContextId must carry the Iroha hash marker bit",
    );
  }
  const expectedEntryHash = copyBoundedBytes(
    input.expectedEntryHash,
    "authenticated BlockProofs expectedEntryHash",
    32,
    32,
  );
  if ((expectedEntryHash[31] & 1) !== 1) {
    throw new TypeError(
      "authenticated BlockProofs expectedEntryHash must carry the Iroha hash marker bit",
    );
  }
  const previousFinalityProofNorito =
    input.previousFinalityProofNorito === undefined ||
    input.previousFinalityProofNorito === null
      ? null
      : copyBoundedBytes(
          input.previousFinalityProofNorito,
          "authenticated BlockProofs previousFinalityProofNorito",
          AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1,
        );
  return {
    version: input.version,
    networkId: input.networkId.literal,
    trustedContextId,
    expectedEntryHash,
    previousFinalityProofNorito,
    finalityProofNorito: copyBoundedBytes(
      input.finalityProofNorito,
      "authenticated BlockProofs finalityProofNorito",
      AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1,
    ),
    executedBlockWire: copyBoundedBytes(
      input.executedBlockWire,
      "authenticated BlockProofs executedBlockWire",
      AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
    ),
    blockProofsNorito: copyBoundedBytes(
      input.blockProofsNorito,
      "authenticated BlockProofs blockProofsNorito",
      AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1,
    ),
  };
}

function normalizeHex32(value, context) {
  if (typeof value !== "string" || !LOWER_HEX_32_PATTERN.test(value)) {
    throw new Error(`native authenticated BlockProofs ${context} is not lowercase hex32`);
  }
  return value;
}

function normalizeVerdict(value, expectedEntryHash) {
  if (!isPlainObject(value)) {
    throw new Error("native authenticated BlockProofs verifier returned a malformed verdict");
  }
  if (typeof value.valid !== "boolean") {
    throw new Error("native authenticated BlockProofs verdict has no boolean valid field");
  }
  const expectedCode = value.valid ? "valid" : "block_proofs_mismatch";
  if (value.code !== expectedCode) {
    throw new Error("native authenticated BlockProofs verdict code is inconsistent");
  }
  if (
    typeof value.blockHeight !== "string" ||
    !POSITIVE_DECIMAL_PATTERN.test(value.blockHeight)
  ) {
    throw new Error("native authenticated BlockProofs blockHeight is not a positive decimal");
  }
  const heightContextIdHex = normalizeHex32(
    value.heightContextIdHex,
    "heightContextIdHex",
  );
  if ((Number.parseInt(heightContextIdHex.slice(-2), 16) & 1) !== 1) {
    throw new Error("native authenticated BlockProofs context id is not a marked Iroha hash");
  }
  const entryHashHex = normalizeHex32(value.entryHashHex, "entryHashHex");
  const expectedEntryHashHex = Reflect.apply(bufferToString, expectedEntryHash, ["hex"]);
  if (entryHashHex !== expectedEntryHashHex) {
    throw new Error(
      "native authenticated BlockProofs verdict is not bound to expectedEntryHash",
    );
  }
  return Object.freeze({
    valid: value.valid,
    code: value.code,
    blockHeight: value.blockHeight,
    blockHashHex: normalizeHex32(value.blockHashHex, "blockHashHex"),
    executedBlockWireHashHex: normalizeHex32(
      value.executedBlockWireHashHex,
      "executedBlockWireHashHex",
    ),
    entryHashHex,
    heightContextIdHex,
  });
}

/**
 * Verify canonical Torii `BlockProofs` through native Sumeragi-v2 finality.
 *
 * The caller must pin `networkId` and `trustedContextId` outside the Torii
 * response and provide the originally requested `expectedEntryHash`, never a
 * hash copied from `BlockProofs`. Supplying `previousFinalityProofNorito`
 * advances exactly one cryptographically linked height; omitting it verifies
 * the target as the initially pinned context. No JavaScript-created structural
 * anchor is used.
 */
export async function verifyAuthenticatedBlockProofsV1(input) {
  const normalized = normalizeInput(input);
  const native = getNativeBinding();
  if (typeof native?.blockProofsVerifyAuthenticatedV1 !== "function") {
    throw new Error(
      "native binding is missing blockProofsVerifyAuthenticatedV1; rebuild iroha_js_host for this SDK version",
    );
  }
  return normalizeVerdict(
    await native.blockProofsVerifyAuthenticatedV1(normalized),
    normalized.expectedEntryHash,
  );
}
