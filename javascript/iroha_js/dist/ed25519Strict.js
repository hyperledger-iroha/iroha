import { ed25519 } from "@noble/curves/ed25519";
import { sha512 } from "@noble/hashes/sha512";

const ED25519_POINT_BYTES = 32;
const ED25519_SIGNATURE_BYTES = 64;
// ExtendedPoint/fromHex/toRawBytes/multiplyUnsafe are available throughout the
// package's declared @noble/curves ^1.4.0 range. Newer noble releases also
// expose the same constructor as `Point`.
const Ed25519Point = ed25519.ExtendedPoint;

export class StrictEd25519Error extends Error {
  constructor(code, message) {
    super(message);
    this.name = "StrictEd25519Error";
    this.code = code;
  }
}

function snapshotBytes(value, expectedLength, context) {
  if (!(value instanceof Uint8Array) || value.byteLength !== expectedLength) {
    throw new StrictEd25519Error(
      "invalid_length",
      `${context} must contain exactly ${expectedLength} bytes`,
    );
  }
  return Uint8Array.from(value);
}

function equalBytes(left, right) {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= left[index] ^ right[index];
  }
  return difference === 0;
}

function numberFromLittleEndian(bytes) {
  let value = 0n;
  for (let index = bytes.length - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[index]);
  }
  return value;
}

function parseStrictPoint(bytes, context) {
  let point;
  try {
    point = Ed25519Point.fromHex(bytes, false);
  } catch (error) {
    throw new StrictEd25519Error(
      "invalid_encoding",
      `${context} is not a canonical compressed Ed25519 point: ${error.message}`,
    );
  }
  if (!equalBytes(point.toRawBytes(), bytes)) {
    throw new StrictEd25519Error(
      "invalid_encoding",
      `${context} is not the canonical compressed Ed25519 encoding`,
    );
  }
  if (point.isSmallOrder()) {
    throw new StrictEd25519Error(
      "small_order",
      `${context} is a small-order Ed25519 point`,
    );
  }
  if (!point.isTorsionFree()) {
    throw new StrictEd25519Error(
      "mixed_torsion",
      `${context} is not in the prime-order Ed25519 subgroup`,
    );
  }
  return point;
}

/** Validate a canonical compressed point in the prime-order Ed25519 subgroup. */
export function assertValidEd25519PublicKey(publicKey) {
  const snapshot = snapshotBytes(publicKey, ED25519_POINT_BYTES, "public key");
  parseStrictPoint(snapshot, "public key");
  return snapshot;
}

/**
 * Verify with Iroha/ed25519-dalek strict semantics.
 *
 * Noble's built-in verifier clears the cofactor even with `zip215: false`, which
 * accepts some mixed-torsion signatures rejected by Rust `verify_strict`. This
 * implementation requires the exact, uncofactored Ed25519 equation.
 */
export function verifyEd25519Strict(message, signature, publicKey) {
  if (!(message instanceof Uint8Array)) return false;
  let signatureBytes;
  let publicKeyBytes;
  try {
    signatureBytes = snapshotBytes(signature, ED25519_SIGNATURE_BYTES, "signature");
    publicKeyBytes = snapshotBytes(publicKey, ED25519_POINT_BYTES, "public key");
    const publicPoint = parseStrictPoint(publicKeyBytes, "public key");
    const encodedR = signatureBytes.subarray(0, ED25519_POINT_BYTES);
    const rPoint = parseStrictPoint(encodedR, "signature R");
    const scalarS = numberFromLittleEndian(
      signatureBytes.subarray(ED25519_POINT_BYTES),
    );
    if (scalarS >= ed25519.CURVE.n) return false;

    const messageSnapshot = Uint8Array.from(message);
    const challenge =
      numberFromLittleEndian(
        sha512
          .create()
          .update(encodedR)
          .update(publicKeyBytes)
          .update(messageSnapshot)
          .digest(),
      ) % ed25519.CURVE.n;
    const left = Ed25519Point.BASE.multiplyUnsafe(scalarS);
    const right = rPoint.add(publicPoint.multiplyUnsafe(challenge));
    return left.equals(right);
  } catch {
    return false;
  }
}
