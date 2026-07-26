"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { bls12_381 } from "@noble/curves/bls12-381";

import {
  assertCanonicalBls12381G1Compressed,
} from "../src/bls12381G1.js";
import {
  assertCanonicalBls12381G1Compressed as assertDistCanonicalBls12381G1Compressed,
} from "../dist/bls12381G1.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const BLS_NORMAL_FIXTURE_PATH = path.resolve(
  __dirname,
  "../../../fixtures/account/bls_normal_public_key.hex",
);
const BLS12381_FIELD_MODULUS =
  0x1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaabn;

function acceptedBy(assertion, compressed) {
  try {
    assertion(compressed);
    return true;
  } catch {
    return false;
  }
}

function acceptedByNoble(compressed) {
  try {
    const point = bls12_381.G1.Point.fromHex(compressed);
    point.assertValidity();
    return !point.is0();
  } catch {
    return false;
  }
}

function bytesFromBigInt(value) {
  const bytes = new Uint8Array(48);
  let remaining = value;
  for (let index = bytes.length - 1; index >= 0; index -= 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  bytes[0] |= 0x80;
  return bytes;
}

function hexToBytes(hex) {
  return Uint8Array.from(Buffer.from(hex, "hex"));
}

function assertAcceptanceMatchesNoble(vector) {
  const expected = acceptedByNoble(vector);
  assert.equal(
    acceptedBy(assertCanonicalBls12381G1Compressed, vector),
    expected,
  );
  assert.equal(
    acceptedBy(assertDistCanonicalBls12381G1Compressed, vector),
    expected,
  );
  return expected;
}

test("canonical BLS12-381 G1 validation agrees with noble on valid public keys", () => {
  const fixture = hexToBytes(
    fs.readFileSync(BLS_NORMAL_FIXTURE_PATH, "utf8").trim(),
  );
  const oppositeFixtureY = Uint8Array.from(fixture);
  oppositeFixtureY[0] ^= 0x20;
  const vectors = [
    fixture,
    oppositeFixtureY,
    ...Array.from({ length: 64 }, (_, index) =>
      bls12_381.G1.Point.BASE.multiply(BigInt(index + 1)).toBytes(true),
    ),
  ];

  for (const vector of vectors) {
    assert.equal(assertAcceptanceMatchesNoble(vector), true);
  }
});

test("canonical BLS12-381 G1 validation rejects hostile encodings like noble", () => {
  const noCompressionFlag = new Uint8Array(48);
  const identity = new Uint8Array(48);
  identity[0] = 0xc0;
  const identityWithSort = new Uint8Array(48);
  identityWithSort[0] = 0xe0;
  const onCurveOutsidePrimeOrderSubgroup = new Uint8Array(48);
  onCurveOutsidePrimeOrderSubgroup[0] = 0x80;
  const nonResidue = bytesFromBigInt(1n);
  const nonCanonicalFieldElement = bytesFromBigInt(BLS12381_FIELD_MODULUS);
  const hostile = [
    new Uint8Array(47),
    new Uint8Array(49),
    noCompressionFlag,
    identity,
    identityWithSort,
    onCurveOutsidePrimeOrderSubgroup,
    nonResidue,
    nonCanonicalFieldElement,
  ];

  for (const vector of hostile) {
    assert.equal(assertAcceptanceMatchesNoble(vector), false);
  }
});

test("canonical BLS12-381 G1 validation is stricter than reducing field encodings", () => {
  const twiceGeneratorX = bls12_381.G1.Point.BASE.multiply(2n).toAffine().x;
  const nonCanonicalEquivalent = bytesFromBigInt(
    BLS12381_FIELD_MODULUS + twiceGeneratorX,
  );
  assert.ok(
    BLS12381_FIELD_MODULUS + twiceGeneratorX < (1n << 381n),
    "the noncanonical field element must fit the compressed x-coordinate",
  );
  assert.equal(
    acceptedByNoble(nonCanonicalEquivalent),
    true,
    "the differential oracle reduces this field element modulo p",
  );
  assert.equal(
    acceptedBy(assertCanonicalBls12381G1Compressed, nonCanonicalEquivalent),
    false,
  );
  assert.equal(
    acceptedBy(assertDistCanonicalBls12381G1Compressed, nonCanonicalEquivalent),
    false,
  );
});

test("canonical BLS12-381 G1 validation differentially rejects deterministic hostile keys", () => {
  const vectors = [];
  for (let x = 0n; x < 64n; x += 1n) {
    const compressed = bytesFromBigInt(x);
    vectors.push(compressed);
    const oppositeY = Uint8Array.from(compressed);
    oppositeY[0] ^= 0x20;
    vectors.push(oppositeY);
  }
  for (let seed = 0; seed < 64; seed += 1) {
    const digest = createHash("sha512")
      .update(`iroha-bls12-381-validator-differential-${seed}`)
      .digest()
      .subarray(0, 48);
    for (let highFlags = 0; highFlags < 8; highFlags += 1) {
      const vector = Uint8Array.from(digest);
      vector[0] = (vector[0] & 0x1f) | (highFlags << 5);
      vectors.push(vector);
    }
  }

  for (const vector of vectors) {
    assert.equal(assertAcceptanceMatchesNoble(vector), false);
  }
});
