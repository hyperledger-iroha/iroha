import { sha256 } from "@noble/hashes/sha2";

import { blake2b256 } from "./blake2b.js";

export const IVM_PROGRAM_HEADER_LENGTH = 49;
/** Default ledger limit for one complete deployed IVM artifact. */
export const IVM_ARTIFACT_MAX_BYTES = 4 * 1024 * 1024;
const CONTRACT_CODE_HASH_DOMAIN = new TextEncoder().encode(
  "iroha:ivm:contract-artifact:v1\0",
);

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
const Uint8ArrayIntrinsic = Uint8Array;
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
const sharedArrayBufferByteLengthGetter =
  typeof SharedArrayBuffer === "undefined"
    ? null
    : Object.getOwnPropertyDescriptor(
        SharedArrayBuffer.prototype,
        "byteLength",
      ).get;

function isSharedBuffer(value) {
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

function getArrayBufferViewInfo(value) {
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

function assertArtifactByteLength(byteLength) {
  if (byteLength > IVM_ARTIFACT_MAX_BYTES) {
    throw new RangeError(
      `IVM artifact exceeds the ${IVM_ARTIFACT_MAX_BYTES}-byte limit`,
    );
  }
}

function copyArrayBufferBytes(buffer, byteOffset, byteLength) {
  const source = new Uint8ArrayIntrinsic(buffer, byteOffset, byteLength);
  const copy = new Uint8ArrayIntrinsic(byteLength);
  Reflect.apply(typedArraySet, copy, [source]);
  return copy;
}

function artifactBytes(value) {
  if (isSharedBuffer(value)) {
    throw new TypeError("artifact must not be backed by SharedArrayBuffer");
  }
  if (isArrayBuffer(value)) {
    const byteLength = arrayBufferByteLengthGetter.call(value);
    assertArtifactByteLength(byteLength);
    return copyArrayBufferBytes(value, 0, byteLength);
  }
  const view = getArrayBufferViewInfo(value);
  if (view !== null) {
    if (isSharedBuffer(view.buffer)) {
      throw new TypeError("artifact must not be backed by SharedArrayBuffer");
    }
    assertArtifactByteLength(view.byteLength);
    return copyArrayBufferBytes(
      view.buffer,
      view.byteOffset,
      view.byteLength,
    );
  }
  throw new TypeError(
    "artifact must be a Uint8Array, ArrayBuffer, or ArrayBuffer view",
  );
}

function bytesToHex(bytes) {
  let hex = "";
  for (const byte of bytes) hex += byte.toString(16).padStart(2, "0");
  return hex;
}

/**
 * Compute both identities required by proved deployed-contract submission.
 * `codeHashHex` matches the current ledger/Core domain-separated hash of the
 * complete deployable artifact. `artifactSha256Hex` is an independent digest
 * over the same complete byte image.
 */
export function computeIvmArtifactHashes(artifact) {
  const bytes = artifactBytes(artifact);
  const byteLength = typedArrayByteLengthGetter.call(bytes);
  if (byteLength < IVM_PROGRAM_HEADER_LENGTH) {
    throw new RangeError(
      `IVM artifact must contain at least the ${IVM_PROGRAM_HEADER_LENGTH}-byte program header`,
    );
  }
  if (
    bytes[0] !== 0x49 ||
    bytes[1] !== 0x56 ||
    bytes[2] !== 0x4d ||
    bytes[3] !== 0x00
  ) {
    throw new TypeError("IVM artifact has an invalid program header magic");
  }
  const codeHashInput = new Uint8ArrayIntrinsic(
    CONTRACT_CODE_HASH_DOMAIN.length + byteLength,
  );
  Reflect.apply(typedArraySet, codeHashInput, [CONTRACT_CODE_HASH_DOMAIN]);
  Reflect.apply(typedArraySet, codeHashInput, [bytes, CONTRACT_CODE_HASH_DOMAIN.length]);
  const rawCodeHash = blake2b256(codeHashInput);
  const codeHash = copyArrayBufferBytes(
    typedArrayBufferGetter.call(rawCodeHash),
    typedArrayByteOffsetGetter.call(rawCodeHash),
    typedArrayByteLengthGetter.call(rawCodeHash),
  );
  codeHash[codeHash.length - 1] |= 1;
  return {
    codeHashHex: bytesToHex(codeHash),
    artifactSha256Hex: bytesToHex(sha256(bytes)),
  };
}
