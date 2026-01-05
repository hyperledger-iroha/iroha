import { test } from "node:test";
import assert from "node:assert/strict";
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
  deriveConfidentialKeyset,
  deriveConfidentialKeysetFromHex,
  generateSm2KeyPair,
  deriveSm2KeyPairFromSeed,
  loadSm2KeyPair,
  sm2PublicKeyMultihash,
  signSm2,
  verifySm2,
  SM2_PRIVATE_KEY_LENGTH,
  SM2_PUBLIC_KEY_LENGTH,
  SM2_SIGNATURE_LENGTH,
  SM2_DEFAULT_DISTINGUISHED_ID,
  sm2FixtureFromSeed,
} from "../src/crypto.js";
import { hasNativeBinding, makeNativeTest } from "./helpers/native.js";

const SM2_DISTID = SM2_DEFAULT_DISTINGUISHED_ID;
const SM2_SEED = Buffer.from("11".repeat(32), "hex");
const SM2_MESSAGE = Buffer.from("iroha sm sdk fixture", "utf8");
const SM2_FIXTURE = hasNativeBinding ? sm2FixtureFromSeed(SM2_DISTID, SM2_SEED, SM2_MESSAGE) : null;

const MESSAGE = Buffer.from("hyperledger iroha");
const nativeTest = makeNativeTest(test);

test("generateKeyPair produces unique keys and valid lengths", () => {
  const kp1 = generateKeyPair();
  const kp2 = generateKeyPair();

  assert.equal(kp1.algorithm, "ed25519");
  assert.equal(kp1.publicKey.length, 32);
  assert.equal(kp1.privateKey.length, 32);

  assert.equal(kp2.publicKey.length, 32);
  assert.equal(kp2.privateKey.length, 32);

  assert.notDeepEqual(kp1.privateKey, kp2.privateKey);
  assert.notDeepEqual(kp1.publicKey, kp2.publicKey);
});

test("generateKeyPair is deterministic with a seed", () => {
  const seed = Buffer.alloc(32, 0x11);
  const kp1 = generateKeyPair({ seed });
  const kp2 = generateKeyPair({ seed });

  assert.deepEqual(kp1.privateKey, kp2.privateKey);
  assert.deepEqual(kp1.publicKey, kp2.publicKey);
  assert.deepEqual(kp1.publicKey, publicKeyFromPrivate(kp1.privateKey));
});

test("signEd25519 and verifyEd25519 round-trip", () => {
  const seed = Buffer.from(Array.from({ length: 32 }, (_, i) => i));
  const { privateKey, publicKey } = generateKeyPair({ seed });
  const signature = signEd25519(MESSAGE, privateKey);

  assert.equal(signature.length, 64);
  assert.equal(verifyEd25519(MESSAGE, signature, publicKey), true);
  assert.equal(verifyEd25519(Buffer.from("other"), signature, publicKey), false);
});

test("publicKeyFromPrivate round-trips generated keys", () => {
  const seed = Buffer.alloc(32, 0x22);
  const { publicKey, privateKey } = generateKeyPair({ seed });

  const derivedFromPrivate = publicKeyFromPrivate(privateKey);

  assert.deepEqual(derivedFromPrivate, publicKey);
});

test("invalid key lengths throw helpful errors", () => {
  assert.throws(() => generateKeyPair({ seed: Buffer.alloc(16) }), /seed must be 32 bytes/);
  assert.throws(
    () => publicKeyFromPrivate(Buffer.alloc(10)),
    /(payload size is incorrect|private key must be 32-byte seed)/,
  );
  assert.throws(() => verifyEd25519(MESSAGE, Buffer.alloc(64), Buffer.alloc(10)), /public key must be 32 bytes/);
});

nativeTest("generateSm2KeyPair produces valid keys and signatures", () => {
  const pair = generateSm2KeyPair();
  const message = Buffer.from("node sm2 smoke test");

  assert.equal(pair.algorithm, "sm2");
  assert.equal(pair.distid, SM2_DEFAULT_DISTINGUISHED_ID);
  assert.equal(pair.publicKey.length, SM2_PUBLIC_KEY_LENGTH);
  assert.equal(pair.privateKey.length, SM2_PRIVATE_KEY_LENGTH);

  const signature = signSm2(message, pair.privateKey, pair.distid);
  assert.equal(signature.length, SM2_SIGNATURE_LENGTH);
  assert.equal(verifySm2(message, signature, pair.publicKey, pair.distid), true);
  assert.equal(verifySm2(Buffer.from("tampered"), signature, pair.publicKey, pair.distid), false);
  assert.match(sm2PublicKeyMultihash(pair.publicKey, pair.distid), /^8626/i);
});

nativeTest("deriveSm2KeyPairFromSeed matches fixture data", () => {
  const seed = Buffer.from(SM2_FIXTURE.seedHex, "hex");
  const message = Buffer.from(SM2_FIXTURE.messageHex, "hex");
  const pair = deriveSm2KeyPairFromSeed(seed, SM2_FIXTURE.distid);

  assert.equal(pair.distid, SM2_FIXTURE.distid);
  assert.equal(pair.privateKey.toString("hex").toUpperCase(), SM2_FIXTURE.privateKeyHex);
  assert.equal(pair.publicKey.toString("hex").toUpperCase(), SM2_FIXTURE.publicKeySec1Hex);
  assert.equal(
    sm2PublicKeyMultihash(pair.publicKey, pair.distid),
    SM2_FIXTURE.publicKeyMultihash,
  );

  const signature = signSm2(message, pair.privateKey, pair.distid);
  assert.equal(signature.toString("hex").toUpperCase(), SM2_FIXTURE.signature);
  assert.equal(verifySm2(message, signature, pair.publicKey, pair.distid), true);
});

nativeTest("loadSm2KeyPair round-trips private key material", () => {
  const seed = Buffer.from(SM2_FIXTURE.seedHex, "hex");
  const derived = deriveSm2KeyPairFromSeed(seed, SM2_FIXTURE.distid);
  const loaded = loadSm2KeyPair(derived.privateKey, derived.distid);
  assert.equal(loaded.distid, derived.distid);
  assert.deepEqual(loaded.publicKey, derived.publicKey);
  assert.deepEqual(loaded.privateKey, derived.privateKey);
});

nativeTest("sm2 helpers validate input lengths", () => {
  const message = Buffer.alloc(16, 0xab);
  assert.throws(() => loadSm2KeyPair(Buffer.alloc(10)), /sm2 private key must be 32 bytes/);
  assert.throws(() => sm2PublicKeyMultihash(Buffer.alloc(10)), /sm2 public key must be 65 bytes/);
  assert.throws(() => signSm2(message, Buffer.alloc(10)), /sm2 private key must be 32 bytes/);
  assert.throws(
    () => verifySm2(message, Buffer.alloc(10), Buffer.alloc(SM2_PUBLIC_KEY_LENGTH)),
    /sm2 signature must be 64 bytes/,
  );
  assert.throws(
    () => verifySm2(message, Buffer.alloc(SM2_SIGNATURE_LENGTH), Buffer.alloc(10)),
    /sm2 public key must be 65 bytes/,
  );
});

test("deriveConfidentialKeyset matches canonical vectors", () => {
  const seed = Buffer.alloc(32, 0x42);
  const keyset = deriveConfidentialKeyset(seed);
  assert.equal(keyset.skSpendHex, "42".repeat(32));
  assert.equal(
    keyset.nkHex,
    "cb7149cc545b97fe5ab1ffe85550f9b0146f3dbff7cf9d2921b9432b641bf0dc",
  );
  assert.equal(
    keyset.ivkHex,
    "fc0f3bf333d454923522f723ef589e0ca31ac1206724b1cd607e41ef0d4230f7",
  );
  assert.equal(
    keyset.ovkHex,
    "5dc50806af739fa5577484268fd77c4e2345c70dae5b55a132b4f9b1a3e00c4c",
  );
  assert.equal(
    keyset.fvkHex,
    "9a0fe79f768aeb440e07751dbddfa17ac97cbf21f3e79c2e0206e56b3c2629af",
  );
  assert.deepEqual(keyset.asHex(), {
    skSpend: keyset.skSpendHex,
    nk: keyset.nkHex,
    ivk: keyset.ivkHex,
    ovk: keyset.ovkHex,
    fvk: keyset.fvkHex,
  });

  const fromHex = deriveConfidentialKeysetFromHex("42".repeat(32));
  assert.deepEqual(fromHex.skSpend, keyset.skSpend);
  assert.equal(fromHex.nkHex, keyset.nkHex);
});

test("deriveConfidentialKeyset validates input", () => {
  assert.throws(() => deriveConfidentialKeyset(Buffer.alloc(2)), /32 bytes/);
  assert.throws(() => deriveConfidentialKeysetFromHex("ab"), /64 hex characters/);
  assert.throws(() => deriveConfidentialKeysetFromHex("zz".repeat(32)), /valid hex/);
});

test("deriveConfidentialKeyset delegates to native binding when available", () => {
  const seed = Buffer.alloc(32, 0x01);
  const expected = {
    sk_spend: Buffer.from(seed),
    nk: Buffer.alloc(32, 0x02),
    ivk: Buffer.alloc(32, 0x03),
    ovk: Buffer.alloc(32, 0x04),
    fvk: Buffer.alloc(32, 0x05),
  };

  withNativeBinding({ deriveConfidentialKeyset: () => expected }, () => {
    const keyset = deriveConfidentialKeyset(seed);
    assert.deepEqual(keyset.skSpend, expected.sk_spend);
    assert.equal(keyset.nkHex, expected.nk.toString("hex"));
  });
});

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}
