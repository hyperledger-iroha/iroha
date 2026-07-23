"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  encodeI105AccountAddress,
} from "../src/address.js";
import { AccountAddress as DistAccountAddress } from "../dist/address.js";

const VALID_KEY = Buffer.from(
  "68F4B6017D0F876A55C80A82B8388A54AAD264D367269E2DE8BE079C935B5F96",
  "hex",
);
const SMALL_ORDER_KEY = Buffer.from(
  "0100000000000000000000000000000000000000000000000000000000000000",
  "hex",
);
const NON_CANONICAL_IDENTITY = Buffer.from(
  "EEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7F",
  "hex",
);
const INVALID_COMPRESSED_KEY = Buffer.alloc(32, 0x02);
const MIXED_TORSION_KEY = Buffer.from(
  "6AEBC0B955CE4A2F1344029986B775E6EA5C40F93F1112B86EC51678EB9DC0FB",
  "hex",
);

test("fromAccount enforces curve-specific public key length", () => {
  const shortKey = VALID_KEY.subarray(1);
  assert.equal(shortKey.length, 31);

  assert.throws(
    () => AccountAddress.fromAccount({ publicKey: shortKey }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY,
  );
});

test("fromCanonicalBytes rejects controller payloads with mismatched key lengths", () => {
  const address = AccountAddress.fromAccount({
    publicKey: VALID_KEY,
  });
  const canonical = Buffer.from(address.canonicalBytes());
  const tampered = Buffer.from(canonical.slice(0, canonical.length - 1));
  tampered[3] = 31;

  assert.throws(
    () => AccountAddress.fromCanonicalBytes(tampered),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY,
  );
});

test("fromAccount rejects small-order ed25519 public keys", () => {
  assert.throws(
    () => AccountAddress.fromAccount({ publicKey: SMALL_ORDER_KEY }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY &&
      /small-order/i.test(error.message),
  );
});

test("fromAccount rejects mixed-torsion ed25519 public keys in src and dist", () => {
  for (const Address of [AccountAddress, DistAccountAddress]) {
    assert.throws(
      () => Address.fromAccount({ publicKey: MIXED_TORSION_KEY }),
      (error) =>
        error?.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY &&
        /prime-order ed25519 subgroup/i.test(error.message),
    );
  }
});

test("fromCanonicalBytes rejects non-canonical ed25519 encodings", () => {
  const address = AccountAddress.fromAccount({
    publicKey: VALID_KEY,
  });
  const canonical = Buffer.from(address.canonicalBytes());
  const keyLength = canonical[3];
  const keyOffset = 4;
  const tampered = Buffer.from(canonical);
  assert.equal(keyLength, NON_CANONICAL_IDENTITY.length);
  tampered.set(NON_CANONICAL_IDENTITY, keyOffset);

  assert.throws(
    () => AccountAddress.fromCanonicalBytes(tampered),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY &&
      /non-canonical/i.test(error.message),
  );
});

test("encodeI105AccountAddress rejects invalid ed25519 controller keys", () => {
  const canonical = Buffer.from(
    AccountAddress.fromAccount({ publicKey: VALID_KEY }).canonicalBytes(),
  );
  const keyOffset = canonical.length - VALID_KEY.length;

  for (const publicKey of [
    SMALL_ORDER_KEY,
    NON_CANONICAL_IDENTITY,
    INVALID_COMPRESSED_KEY,
    MIXED_TORSION_KEY,
  ]) {
    const tampered = Buffer.from(canonical);
    tampered.set(publicKey, keyOffset);
    assert.throws(
      () => encodeI105AccountAddress(tampered),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY,
    );
  }
});

test("fromAccount rejects canonical-range bytes that do not decompress", () => {
  assert.throws(
    () => AccountAddress.fromAccount({ publicKey: INVALID_COMPRESSED_KEY }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY &&
      /compressed ed25519/i.test(error.message),
  );
});
