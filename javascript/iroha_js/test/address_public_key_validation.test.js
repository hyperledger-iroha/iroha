"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  DEFAULT_DOMAIN_NAME,
} from "../src/address.js";

const VALID_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
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

test("fromAccount enforces curve-specific public key length", () => {
  const shortKey = VALID_KEY.subarray(1);
  assert.equal(shortKey.length, 31);

  assert.throws(
    () => AccountAddress.fromAccount({ domain: DEFAULT_DOMAIN_NAME, publicKey: shortKey }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY,
  );
});

test("fromCanonicalBytes rejects controller payloads with mismatched key lengths", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
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
    () => AccountAddress.fromAccount({ domain: DEFAULT_DOMAIN_NAME, publicKey: SMALL_ORDER_KEY }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_PUBLIC_KEY &&
      /small-order/i.test(error.message),
  );
});

test("fromCanonicalBytes rejects non-canonical ed25519 encodings", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
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
