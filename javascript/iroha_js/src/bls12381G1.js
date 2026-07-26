// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// BLS12-381 G1 is y² = x³ + 4 over Fp. Public keys must be points in the
// prime-order r subgroup, encoded with the 48-byte compressed serialization.
const BLS12381_FIELD_MODULUS =
  0x1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaabn;
const BLS12381_G1_SUBGROUP_ORDER =
  0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001n;
const BLS12381_SQRT_EXPONENT = (BLS12381_FIELD_MODULUS + 1n) >> 2n;
const BLS12381_G1_SUBGROUP_TOP_BIT = 1n << 254n;
const COMPRESSED_FLAG = 0x80;
const INFINITY_FLAG = 0x40;
const SORT_FLAG = 0x20;
const X_HIGH_BITS_MASK = 0x1f;
const JACOBIAN_INFINITY = Object.freeze({ x: 0n, y: 1n, z: 0n });

function fieldMod(value) {
  const reduced = value % BLS12381_FIELD_MODULUS;
  return reduced < 0n ? reduced + BLS12381_FIELD_MODULUS : reduced;
}

function fieldPow(base, exponent) {
  let result = 1n;
  let factor = fieldMod(base);
  let remaining = exponent;
  while (remaining !== 0n) {
    if ((remaining & 1n) !== 0n) {
      result = (result * factor) % BLS12381_FIELD_MODULUS;
    }
    factor = (factor * factor) % BLS12381_FIELD_MODULUS;
    remaining >>= 1n;
  }
  return result;
}

function doubleJacobian(point) {
  if (point.z === 0n || point.y === 0n) {
    return JACOBIAN_INFINITY;
  }
  const yy = (point.y * point.y) % BLS12381_FIELD_MODULUS;
  const s = (((4n * point.x) % BLS12381_FIELD_MODULUS) * yy)
    % BLS12381_FIELD_MODULUS;
  const m = (((3n * point.x) % BLS12381_FIELD_MODULUS) * point.x)
    % BLS12381_FIELD_MODULUS;
  const x = fieldMod(m * m - 2n * s);
  const y = fieldMod(
    m * fieldMod(s - x) - ((8n * yy) % BLS12381_FIELD_MODULUS) * yy,
  );
  const z = (((2n * point.y) % BLS12381_FIELD_MODULUS) * point.z)
    % BLS12381_FIELD_MODULUS;
  return { x, y, z };
}

function addAffineToJacobian(point, affineX, affineY) {
  if (point.z === 0n) {
    return { x: affineX, y: affineY, z: 1n };
  }
  const zz = (point.z * point.z) % BLS12381_FIELD_MODULUS;
  const u = (affineX * zz) % BLS12381_FIELD_MODULUS;
  const s = (((affineY * zz) % BLS12381_FIELD_MODULUS) * point.z)
    % BLS12381_FIELD_MODULUS;
  const h = fieldMod(u - point.x);
  const r = fieldMod(s - point.y);
  if (h === 0n) {
    return r === 0n ? doubleJacobian(point) : JACOBIAN_INFINITY;
  }
  const hh = (h * h) % BLS12381_FIELD_MODULUS;
  const hhh = (h * hh) % BLS12381_FIELD_MODULUS;
  const v = (point.x * hh) % BLS12381_FIELD_MODULUS;
  const x = fieldMod(r * r - hhh - 2n * v);
  const y = fieldMod(r * fieldMod(v - x) - point.y * hhh);
  const z = (point.z * h) % BLS12381_FIELD_MODULUS;
  return { x, y, z };
}

function isInPrimeOrderSubgroup(affineX, affineY) {
  // The direct [r]P test is intentionally used here instead of a curve-specific
  // endomorphism shortcut: validation operates only on public data, and this
  // formulation makes the subgroup invariant straightforward to audit.
  let result = JACOBIAN_INFINITY;
  for (
    let bit = BLS12381_G1_SUBGROUP_TOP_BIT;
    bit !== 0n;
    bit >>= 1n
  ) {
    result = doubleJacobian(result);
    if ((BLS12381_G1_SUBGROUP_ORDER & bit) !== 0n) {
      result = addAffineToJacobian(result, affineX, affineY);
    }
  }
  return result.z === 0n;
}

function compressedXCoordinate(compressed) {
  let x = BigInt(compressed[0] & X_HIGH_BITS_MASK);
  for (let index = 1; index < compressed.length; index += 1) {
    x = (x << 8n) | BigInt(compressed[index]);
  }
  return x;
}

export function assertCanonicalBls12381G1Compressed(compressed) {
  if (!(compressed instanceof Uint8Array) || compressed.byteLength !== 48) {
    throw new TypeError("BLS12-381 G1 public key must be exactly 48 bytes");
  }
  if ((compressed[0] & COMPRESSED_FLAG) === 0) {
    throw new TypeError("BLS12-381 G1 public key must use compressed encoding");
  }
  if ((compressed[0] & INFINITY_FLAG) !== 0) {
    throw new TypeError("BLS12-381 G1 identity public keys are forbidden");
  }

  const x = compressedXCoordinate(compressed);
  if (x >= BLS12381_FIELD_MODULUS) {
    throw new TypeError("BLS12-381 G1 x-coordinate is not canonical");
  }
  const right = fieldMod(
    ((x * x) % BLS12381_FIELD_MODULUS) * x + 4n,
  );
  const y = fieldPow(right, BLS12381_SQRT_EXPONENT);
  if ((y * y) % BLS12381_FIELD_MODULUS !== right) {
    throw new TypeError("BLS12-381 G1 compressed point is not on the curve");
  }
  // Both sign bits select canonical opposite roots when y != 0. A zero root
  // has only one serialization and therefore cannot carry the sort flag.
  if (y === 0n && (compressed[0] & SORT_FLAG) !== 0) {
    throw new TypeError("BLS12-381 G1 zero y-coordinate has a noncanonical sign");
  }
  if (!isInPrimeOrderSubgroup(x, y)) {
    throw new TypeError("BLS12-381 G1 public key is not in the prime-order subgroup");
  }
}
