import { sha256 } from "@noble/hashes/sha2";

import { blake2b256 } from "./blake2b.js";

export const IVM_PROGRAM_HEADER_LENGTH = 17;

function artifactBytes(value) {
  if (value instanceof Uint8Array) {
    return new Uint8Array(value);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value.slice(0));
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(
      value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength),
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
 * `codeHashHex` matches ledger/Core hashing of bytes after the fixed IVM header;
 * `artifactSha256Hex` commits to every header and body byte.
 */
export function computeIvmArtifactHashes(artifact) {
  const bytes = artifactBytes(artifact);
  if (bytes.length < IVM_PROGRAM_HEADER_LENGTH) {
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
  const codeHash = new Uint8Array(
    blake2b256(bytes.subarray(IVM_PROGRAM_HEADER_LENGTH)),
  );
  codeHash[codeHash.length - 1] |= 1;
  return {
    codeHashHex: bytesToHex(codeHash),
    artifactSha256Hex: bytesToHex(sha256(bytes)),
  };
}
