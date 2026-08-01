import { Buffer } from "buffer";

/** Maximum complete encoded ProofBox budget used by the Rust data model. */
export const PROOF_BOX_MAX_ENCODED_BYTES = 64 * 1024 * 1024;
/** Maximum UTF-8 length of each portable verifier-key id component. */
export const VERIFYING_KEY_ID_MAX_FIELD_BYTES = 256;
/** Maximum lane-privacy Merkle path depth representable by the runtime. */
export const LANE_PRIVACY_MERKLE_MAX_DEPTH = 255;

const PORTABLE_ID_FORBIDDEN_SEPARATORS = Object.freeze([
  "..",
  "//",
  ":::",
  "/:",
  ":/",
  "/.",
  "./",
  ":.",
  ".:",
]);
const BASE64_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** Return whether one verifier-key id component matches the Rust grammar. */
export function isPortableVerifyingKeyIdField(value) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > VERIFYING_KEY_ID_MAX_FIELD_BYTES
  ) {
    return false;
  }
  const first = value.charCodeAt(0);
  const last = value.charCodeAt(value.length - 1);
  if (!isAsciiLowerOrDigit(first) || !isAsciiLowerOrDigit(last)) {
    return false;
  }
  if (
    PORTABLE_ID_FORBIDDEN_SEPARATORS.some((separator) =>
      value.includes(separator),
    )
  ) {
    return false;
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (
      !isAsciiLowerOrDigit(code) &&
      code !== 0x2d &&
      code !== 0x5f &&
      code !== 0x2f &&
      code !== 0x3a &&
      code !== 0x2e
    ) {
      return false;
    }
  }
  return true;
}

/** Compute the complete ProofBox size without allocating proof storage. */
export function proofBoxEncodedLength(backend, proofLength) {
  if (typeof backend !== "string") {
    throw new TypeError("ProofBox backend must be a string");
  }
  if (!Number.isSafeInteger(proofLength) || proofLength < 0) {
    throw new TypeError("ProofBox proof length must be a non-negative safe integer");
  }
  const backendLength = Buffer.byteLength(backend, "utf8");
  const backendValueLength = checkedProofBoxLengthSum(
    compactLengthPrefixBytes(backendLength),
    backendLength,
  );
  const proofValueLength = checkedProofBoxLengthSum(8, proofLength);
  return checkedProofBoxLengthSum(
    compactLengthPrefixBytes(backendValueLength),
    backendValueLength,
    compactLengthPrefixBytes(proofValueLength),
    proofValueLength,
  );
}

/** Return the maximum proof payload allowed for a specific backend label. */
export function proofBoxMaxProofBytes(backend) {
  if (proofBoxEncodedLength(backend, 0) > PROOF_BOX_MAX_ENCODED_BYTES) {
    return 0;
  }
  let lower = 0;
  let upper = PROOF_BOX_MAX_ENCODED_BYTES;
  while (lower < upper) {
    const candidate = lower + Math.ceil((upper - lower) / 2);
    if (proofBoxEncodedLength(backend, candidate) <= PROOF_BOX_MAX_ENCODED_BYTES) {
      lower = candidate;
    } else {
      upper = candidate - 1;
    }
  }
  return lower;
}

/** Return whether the complete ProofBox fits the first-release budget. */
export function proofBoxFitsEncodedBudget(backend, proofLength) {
  return proofBoxEncodedLength(backend, proofLength) <= PROOF_BOX_MAX_ENCODED_BYTES;
}

function compactLengthPrefixBytes(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError("Norito compact length must be a non-negative safe integer");
  }
  let remaining = value;
  let bytes = 1;
  while (remaining >= 0x80) {
    remaining = Math.floor(remaining / 0x80);
    bytes += 1;
  }
  return bytes;
}

function checkedProofBoxLengthSum(...values) {
  let total = 0;
  for (const value of values) {
    if (!Number.isSafeInteger(value) || value < 0 || !Number.isSafeInteger(total + value)) {
      throw new RangeError("encoded ProofBox length exceeds the safe integer range");
    }
    total += value;
  }
  return total;
}

/**
 * Validate canonical standard base64 and return its decoded length.
 *
 * Length and terminal-bit checks happen before any decoder allocation.
 */
export function canonicalBase64DecodedLength(value, context = "base64") {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value)
  ) {
    throw new TypeError(`${context} must be canonical standard base64`);
  }
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  if (padding > 0 && value.length < 4) {
    throw new TypeError(`${context} must be canonical standard base64`);
  }
  const terminalIndex = value.length - padding - 1;
  const terminalSextet = BASE64_ALPHABET.indexOf(value[terminalIndex]);
  if (
    terminalSextet < 0 ||
    (padding === 2 && (terminalSextet & 0x0f) !== 0) ||
    (padding === 1 && (terminalSextet & 0x03) !== 0)
  ) {
    throw new TypeError(`${context} must be canonical standard base64`);
  }
  const decodedLength = (value.length / 4) * 3 - padding;
  if (decodedLength === 0) {
    throw new TypeError(`${context} must decode to a non-empty proof`);
  }
  return decodedLength;
}

/** Copy a raw digest and set the canonical `Hash::prehashed` marker bit. */
export function canonicalizePrehashedBytes(bytes) {
  if (!Array.isArray(bytes) || bytes.length !== 32) {
    throw new TypeError("prehashed digest must contain exactly 32 bytes");
  }
  const canonical = bytes.slice();
  canonical[31] |= 1;
  return canonical;
}

/** Return whether a 32-byte digest carries the typed prehashed marker. */
export function hasCanonicalPrehashedMarker(bytes) {
  return Array.isArray(bytes) && bytes.length === 32 && (bytes[31] & 1) === 1;
}

/** Return whether a u32 leaf index can exist at the supplied Merkle depth. */
export function laneMerkleLeafIndexFitsDepth(leafIndex, depth) {
  if (
    !Number.isInteger(leafIndex) ||
    leafIndex < 0 ||
    leafIndex > 0xffff_ffff ||
    !Number.isInteger(depth) ||
    depth < 1 ||
    depth > LANE_PRIVACY_MERKLE_MAX_DEPTH
  ) {
    return false;
  }
  return depth >= 32 || leafIndex < 2 ** depth;
}

function isAsciiLowerOrDigit(code) {
  return (code >= 0x61 && code <= 0x7a) || (code >= 0x30 && code <= 0x39);
}
