import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  SUPPORTED_CRYPTO_ALGORITHMS,
  generateKeyPair,
  loadKeyPair,
  normalizeCryptoAlgorithm,
  privateKeyMultihash,
  publicKeyFromPrivate,
  publicKeyMultihash,
  normalizeRecoveryPhrase,
  validateRecoveryPhrase,
  generateRecoveryPhrase,
  entropyToRecoveryPhrase,
  recoveryPhraseToEntropy,
  deriveEd25519SeedFromRecoveryPhrase,
  ed25519SeedToRecoveryPhrase,
  sign,
  signEd25519,
  supportedCryptoAlgorithms,
  verify,
  verifyEd25519,
  deriveConfidentialKeyset,
  deriveConfidentialKeysetFromHex,
  deriveConfidentialNullifierV2,
  generateSm2KeyPair,
  deriveSm2KeyPairFromSeed,
  loadSm2KeyPair,
  sm2PublicKeyMultihash,
  signSm2,
  verifySm2,
  buildKaigiRosterJoinProof,
  SM2_PRIVATE_KEY_LENGTH,
  SM2_PUBLIC_KEY_LENGTH,
  SM2_SIGNATURE_LENGTH,
  SM2_DEFAULT_DISTINGUISHED_ID,
  sm2FixtureFromSeed,
} from "../src/crypto.js";
import { NetworkId } from "../src/networkId.js";
import { verifyEd25519 as verifyBrowserEd25519 } from "../src/crypto.browser.js";
import { ed25519 } from "@noble/curves/ed25519";
import { __resetNativeStateForTests } from "../src/native.js";
import {
  normalizeCryptoAlgorithm as normalizeDistCryptoAlgorithm,
} from "../dist/crypto.js";
import { hasSm2Binding, makeNativeTest, sm2RequiredMethods } from "./helpers/native.js";

const SM2_DISTID = SM2_DEFAULT_DISTINGUISHED_ID;
const SM2_SEED = Buffer.from("11".repeat(32), "hex");
const SM2_MESSAGE = Buffer.from("iroha sm sdk fixture", "utf8");
const SM2_FIXTURE = hasSm2Binding()
  ? sm2FixtureFromSeed(SM2_DISTID, SM2_SEED, SM2_MESSAGE)
  : null;

const MESSAGE = Buffer.from("hyperledger iroha");
const nativeTest = makeNativeTest(test, { require: sm2RequiredMethods });

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
}

function withNativeDirectory(directory, fn) {
  const previousDirectory = process.env.IROHA_JS_NATIVE_DIR;
  const previousBinding = globalThis.__IROHA_NATIVE_BINDING__;
  delete globalThis.__IROHA_NATIVE_BINDING__;
  process.env.IROHA_JS_NATIVE_DIR = directory;
  __resetNativeStateForTests();
  try {
    return fn();
  } finally {
    if (previousDirectory === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousDirectory;
    }
    if (previousBinding === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previousBinding;
    }
    __resetNativeStateForTests();
  }
}

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

test("generateKeyPair rejects non-32-byte seed material", () => {
  assert.throws(
    () => generateKeyPair({ seed: Buffer.from("short seed") }),
    /seed must be exactly 32 bytes/,
  );
});

test("Ed25519 key generation and derivation remain available without the native host", () => {
  const seed = Buffer.from(Array.from({ length: 32 }, (_, index) => index + 1));
  withNativeBinding(Object.create(null), () => {
    const generated = generateKeyPair({ seed });
    const expectedPublicKey = Buffer.from(ed25519.getPublicKey(seed));

    assert.deepEqual(supportedCryptoAlgorithms(), SUPPORTED_CRYPTO_ALGORITHMS);
    assert.equal(generated.algorithm, "ed25519");
    assert.deepEqual(generated.privateKey, seed);
    assert.deepEqual(generated.publicKey, expectedPublicKey);
    assert.deepEqual(publicKeyFromPrivate(seed), expectedPublicKey);
    assert.deepEqual(
      publicKeyFromPrivate(Buffer.concat([seed, expectedPublicKey])),
      expectedPublicKey,
    );

    const signature = signEd25519(MESSAGE, generated.privateKey);
    assert.equal(verifyEd25519(MESSAGE, signature, generated.publicKey), true);
    assert.throws(
      () =>
        publicKeyFromPrivate(
          Buffer.concat([seed, Buffer.alloc(32, 0xff)]),
        ),
      /mismatched public key/u,
    );
  });
});

test("missing native files use the portable Ed25519 path", () => {
  const directory = mkdtempSync(join(tmpdir(), "iroha-js-missing-native-"));
  try {
    withNativeDirectory(directory, () => {
      const seed = Buffer.alloc(32, 0x5a);
      const generated = generateKeyPair({ seed });
      assert.deepEqual(generated.publicKey, Buffer.from(ed25519.getPublicKey(seed)));
      assert.deepEqual(publicKeyFromPrivate(seed), generated.publicKey);
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a present native file with a bad checksum never falls back", () => {
  const directory = mkdtempSync(join(tmpdir(), "iroha-js-invalid-native-"));
  try {
    writeFileSync(join(directory, "iroha_js_host.node"), "not-a-native-addon");
    writeFileSync(
      join(directory, "iroha_js_host.checksums.json"),
      `${JSON.stringify({
        entries: {
          [`${process.platform}-${process.arch}`]: {
            sha256: "0".repeat(64),
            build_execution_policy: "trusted-local-cargo-v1",
            build_provenance_version: 3,
            source_git_revision: "a".repeat(40),
            source_tree_clean: true,
            source_tree_sha256: "b".repeat(64),
          },
        },
      })}\n`,
    );
    withNativeDirectory(directory, () => {
      assert.throws(
        () => generateKeyPair({ seed: Buffer.alloc(32, 0x5b) }),
        /checksum mismatch/u,
      );
      assert.throws(
        () => publicKeyFromPrivate(Buffer.alloc(32, 0x5b)),
        /checksum mismatch/u,
      );
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("signEd25519 and verifyEd25519 round-trip", () => {
  const seed = Buffer.from(Array.from({ length: 32 }, (_, i) => i));
  const { privateKey, publicKey } = generateKeyPair({ seed });
  const signature = signEd25519(MESSAGE, privateKey);

  assert.equal(signature.length, 64);
  assert.equal(verifyEd25519(MESSAGE, signature, publicKey), true);
  assert.equal(verifyEd25519(Buffer.from("other"), signature, publicKey), false);
  assert.equal(verifyBrowserEd25519(MESSAGE, signature, publicKey), true);
});

test("Ed25519 verification matches Rust strict rejection semantics", () => {
  const message = Buffer.from(
    "e249bef6c1b5202881c8996347ee0e7c5a65aa8078c5d7848d004781b0cf79e3",
    "hex",
  );
  const publicKey = Buffer.from(
    "48075a597e721a156e2e0799de5cc0c5324dc6e7eaf1cdd46250868ec53215dd",
    "hex",
  );
  const mixedTorsion = Buffer.from(
    "88fc2ecb6b72920cf6476056977d8dde846c8fc3b180ea9dc3973a1d0f2d0fb3eda13e150fc47692e90dd4a773d83dfaf454c7d0de9af8e68c5fbbd503f6a10c",
    "hex",
  );
  assert.equal(ed25519.verify(mixedTorsion, message, publicKey, { zip215: false }), true);
  assert.equal(verifyEd25519(message, mixedTorsion, publicKey), false);
  assert.equal(verifyBrowserEd25519(message, mixedTorsion, publicKey), false);

  const seed = Buffer.alloc(32, 0x31);
  const ordinaryPublicKey = Buffer.from(ed25519.getPublicKey(seed));
  const ordinary = Buffer.from(ed25519.sign(message, seed));
  assert.equal(verifyEd25519(message, ordinary, ordinaryPublicKey), true);
  assert.equal(verifyBrowserEd25519(message, ordinary, ordinaryPublicKey), true);

  const scalarAtOrder = Buffer.alloc(32);
  let order = ed25519.CURVE.n;
  for (let index = 0; index < scalarAtOrder.length; index += 1) {
    scalarAtOrder[index] = Number(order & 0xffn);
    order >>= 8n;
  }
  const scalarOverflow = Buffer.concat([ordinary.subarray(0, 32), scalarAtOrder]);
  const smallOrderR = Buffer.concat([
    Buffer.from("01" + "00".repeat(31), "hex"),
    ordinary.subarray(32),
  ]);
  const nonCanonicalR = Buffer.concat([
    Buffer.from("ed" + "ff".repeat(30) + "7f", "hex"),
    ordinary.subarray(32),
  ]);
  for (const invalid of [scalarOverflow, smallOrderR, nonCanonicalR]) {
    assert.equal(verifyEd25519(message, invalid, ordinaryPublicKey), false);
    assert.equal(verifyBrowserEd25519(message, invalid, ordinaryPublicKey), false);
  }
  assert.equal(verifyEd25519(message, ordinary, Buffer.alloc(32, 0x02)), false);
  assert.equal(verifyBrowserEd25519(message, ordinary, Buffer.alloc(32, 0x02)), false);
});

test("publicKeyFromPrivate round-trips generated keys", () => {
  const seed = Buffer.alloc(32, 0x22);
  const { publicKey, privateKey } = generateKeyPair({ seed });

  const derivedFromPrivate = publicKeyFromPrivate(privateKey);
  const derivedFromKeypair = publicKeyFromPrivate(Buffer.concat([privateKey, publicKey]));

  assert.deepEqual(derivedFromPrivate, publicKey);
  assert.deepEqual(derivedFromKeypair, publicKey);
});

test("invalid key lengths throw helpful errors", () => {
  assert.throws(
    () => publicKeyFromPrivate(Buffer.alloc(10)),
    /(payload size is incorrect|private key must be 32-byte seed)/,
  );
  const seed = Buffer.alloc(32, 0x33);
  const { privateKey } = generateKeyPair({ seed });
  const mismatched = Buffer.concat([privateKey, Buffer.alloc(32, 0x00)]);
  assert.throws(() => publicKeyFromPrivate(mismatched), /mismatched public key/);
  assert.throws(() => verifyEd25519(MESSAGE, Buffer.alloc(64), Buffer.alloc(10)), /public key must be 32 bytes/);
});

test("recovery phrase helpers export Ed25519 seeds as reversible 24-word phrases", () => {
  const seed = Buffer.from(Array.from({ length: 32 }, (_, index) => index));
  const publicKey = Buffer.from(ed25519.getPublicKey(seed));
  const recovery = ed25519SeedToRecoveryPhrase(Buffer.concat([seed, publicKey]));

  assert.equal(recovery.wordCount, 24);
  assert.equal(recovery.words.length, 24);
  assert.equal(recovery.phrase, recovery.words.join(" "));
  assert.equal(validateRecoveryPhrase(recovery.phrase), true);
  assert.deepEqual(recoveryPhraseToEntropy(recovery.phrase), seed);
  assert.deepEqual(deriveEd25519SeedFromRecoveryPhrase(recovery.phrase), seed);
  assert.deepEqual(normalizeRecoveryPhrase(`  ${recovery.phrase.toUpperCase().replaceAll(" ", "  ")}  `), recovery);
});

test("recovery phrase helpers generate and derive 12-word phrases", () => {
  const recovery = entropyToRecoveryPhrase(Buffer.alloc(16, 7));
  const entropy = recoveryPhraseToEntropy(recovery.phrase);
  const seed = deriveEd25519SeedFromRecoveryPhrase(recovery.phrase);

  assert.equal(recovery.wordCount, 12);
  assert.equal(recovery.words.length, 12);
  assert.equal(entropy.length, 16);
  assert.equal(seed.length, 32);
  assert.equal(
    seed.toString("hex"),
    "d761d406af2a4a5a15f67c924378ed88d1f85c13f1a37fc7366f59789b3bcd65",
  );
  assert.notDeepEqual(seed, entropy);
  assert.equal(validateRecoveryPhrase(recovery.phrase), true);
});

test("Node recovery phrase generation does not require global Web Crypto", () => {
  const cryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, "crypto");
  try {
    Object.defineProperty(globalThis, "crypto", {
      configurable: true,
      value: undefined,
    });
    const recovery = generateRecoveryPhrase(12);
    assert.equal(recovery.wordCount, 12);
    assert.equal(validateRecoveryPhrase(recovery.phrase), true);
  } finally {
    if (cryptoDescriptor) {
      Object.defineProperty(globalThis, "crypto", cryptoDescriptor);
    } else {
      delete globalThis.crypto;
    }
  }
});

test("recovery phrase helpers reject malformed phrases and entropy", () => {
  const entropy = Buffer.alloc(16, 7);
  const recovery = entropyToRecoveryPhrase(entropy);
  const tamperedWords = [...recovery.words];
  tamperedWords[0] = "abandon";
  const tampered = tamperedWords.join(" ");

  assert.equal(recovery.wordCount, 12);
  assert.deepEqual(recoveryPhraseToEntropy(recovery.phrase), entropy);
  assert.equal(validateRecoveryPhrase(tampered), false);
  assert.throws(() => normalizeRecoveryPhrase(tampered), /checksum or word list/);
  assert.throws(() => normalizeRecoveryPhrase("abandon ".repeat(11)), /12 or 24 words/);
  assert.throws(() => entropyToRecoveryPhrase(Buffer.alloc(20)), /16 or 32 bytes/);
  assert.throws(() => generateRecoveryPhrase(15), /12 or 24/);
  assert.throws(() => ed25519SeedToRecoveryPhrase(Buffer.alloc(31)), /32-byte seed or 64-byte seed\+public/);
});

test("crypto algorithm labels cover Rust signing algorithms", () => {
  assert.deepEqual(SUPPORTED_CRYPTO_ALGORITHMS, [
    "ed25519",
    "secp256k1",
    "bls_normal",
    "bls_small",
    "ml-dsa",
    "gost3410-2012-256-paramset-a",
    "gost3410-2012-256-paramset-b",
    "gost3410-2012-256-paramset-c",
    "gost3410-2012-512-paramset-a",
    "gost3410-2012-512-paramset-b",
    "sm2",
  ]);
  assert.equal(normalizeCryptoAlgorithm("ML_DSA-65"), "ml-dsa");
  assert.equal(
    normalizeCryptoAlgorithm("GOST3410-2012-512-PARAMSET-B"),
    "gost3410-2012-512-paramset-b",
  );
  assert.equal(normalizeCryptoAlgorithm("bls-small"), "bls_small");
});

test("crypto algorithm labels reject unsupported and Unicode-confusable aliases", () => {
  for (const [label, normalize] of [
    ["src", normalizeCryptoAlgorithm],
    ["dist", normalizeDistCryptoAlgorithm],
  ]) {
    assert.equal(normalize("ed-25519"), "ed25519", `${label} keeps ASCII aliases`);
    for (const algorithm of [
      "unknown",
      "ed\t25519",
      "ed\u200B25519",
      "\u0435d25519",
      "ml\uFF0Ddsa",
      "mldsa44",
      "ML-DSA-44",
      "ML_DSA_87",
      "Ml.DsA/44",
      "ML-DSA-4-4",
      "ML-DSA-４４",
      "ML-DSA-８７",
      "gost3410-2012-512-paramset-\u0432",
    ]) {
      assert.throws(
        () => normalize(algorithm),
        /unsupported crypto algorithm/,
        `${label} must reject ${algorithm}`,
      );
    }
  }
});

test("generic crypto helpers delegate non-Ed25519 algorithms to native binding", () => {
  const privateKey = Buffer.from("native-private");
  const publicKey = Buffer.from("native-public");
  const signature = Buffer.from("native-signature");
  const binding = {
    supportedCryptoAlgorithms: () => ["ed25519", "gost3410-2012-256-paramset-a"],
    cryptoKeypair: (algorithm, seed) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(seed), Buffer.alloc(32, 0x73));
      return { algorithm, privateKey, publicKey };
    },
    cryptoKeypairFromPrivate: (algorithm, rawPrivateKey) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPrivateKey), privateKey);
      return { algorithm, privateKey, publicKey };
    },
    cryptoPublicKeyFromPrivate: (algorithm, rawPrivateKey) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPrivateKey), privateKey);
      return publicKey;
    },
    cryptoSign: (algorithm, rawPrivateKey, message) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPrivateKey), privateKey);
      assert.deepEqual(Buffer.from(message), MESSAGE);
      return signature;
    },
    cryptoVerify: (algorithm, rawPublicKey, message, rawSignature) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPublicKey), publicKey);
      assert.deepEqual(Buffer.from(message), MESSAGE);
      assert.deepEqual(Buffer.from(rawSignature), signature);
      return true;
    },
    cryptoPublicKeyMultihash: (algorithm, rawPublicKey) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPublicKey), publicKey);
      return "gost-pub-mh";
    },
    cryptoPrivateKeyMultihash: (algorithm, rawPrivateKey) => {
      assert.equal(algorithm, "gost3410-2012-256-paramset-a");
      assert.deepEqual(Buffer.from(rawPrivateKey), privateKey);
      return "gost-priv-mh";
    },
  };

  withNativeBinding(binding, () => {
    assert.deepEqual(supportedCryptoAlgorithms(), [
      "ed25519",
      "gost3410-2012-256-paramset-a",
    ]);
    const keyPair = generateKeyPair({ algorithm: "gost256a", seed: Buffer.alloc(32, 0x73) });
    assert.equal(keyPair.algorithm, "gost3410-2012-256-paramset-a");
    assert.deepEqual(keyPair.privateKey, privateKey);
    assert.deepEqual(keyPair.publicKey, publicKey);
    assert.equal(keyPair.distid, null);
    assert.deepEqual(loadKeyPair(privateKey, { algorithm: "gost256a" }).publicKey, publicKey);
    assert.deepEqual(publicKeyFromPrivate(privateKey, { algorithm: "gost256a" }), publicKey);
    assert.deepEqual(sign(MESSAGE, privateKey, { algorithm: "gost256a" }), signature);
    assert.equal(verify(MESSAGE, signature, publicKey, { algorithm: "gost256a" }), true);
    assert.equal(publicKeyMultihash(publicKey, { algorithm: "gost256a" }), "gost-pub-mh");
    assert.equal(privateKeyMultihash(privateKey, { algorithm: "gost256a" }), "gost-priv-mh");
  });
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
  assert.throws(
    () => deriveConfidentialKeysetFromHex(` ${"42".repeat(32)}`),
    /spendKeyHex must not contain surrounding whitespace/,
  );
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

test("deriveConfidentialNullifierV2 binds the exact NetworkId bytes", () => {
  const firstNetworkId = NetworkId.fromBytes(Buffer.alloc(32, 0x11));
  const secondNetworkId = NetworkId.fromBytes(Buffer.alloc(32, 0x13));
  const calls = [];
  const binding = {
    deriveConfidentialNullifierV2: (...args) => {
      calls.push(args);
      return Buffer.alloc(32, 0x61);
    },
  };
  const input = {
    assetDefinitionId: "xor#sora",
    spendKey: Buffer.alloc(32, 0x21),
    rhoHex: "31".repeat(32),
  };

  withNativeBinding(binding, () => {
    deriveConfidentialNullifierV2({ ...input, networkId: firstNetworkId });
    deriveConfidentialNullifierV2({ ...input, networkId: secondNetworkId });
    assert.throws(
      () => deriveConfidentialNullifierV2({ ...input, networkId: "sora" }),
      /networkId must be a NetworkId/u,
    );
  });

  assert.deepEqual(calls[0][0], Buffer.from(firstNetworkId.toBytes()));
  assert.deepEqual(calls[1][0], Buffer.from(secondNetworkId.toBytes()));
  assert.notDeepEqual(calls[0][0], calls[1][0]);
});

test("buildKaigiRosterJoinProof fails closed before native dispatch", () => {
  let called = false;
  withNativeBinding({ buildKaigiRosterJoinProof: () => { called = true; } }, () => {
    assert.throws(
      () => buildKaigiRosterJoinProof({
        seed: Buffer.from("seed"),
        rosterRootHex: "44".repeat(32),
      }),
      /binds the signed participant authority/u,
    );
  });
  assert.equal(called, false);
});

test("buildKaigiRosterJoinProof remains unavailable for well-formed inputs", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({
      seed: Buffer.alloc(32, 0x44),
      rosterRootHex: "00".repeat(32),
    }),
    /binds the signed participant authority/u,
  );
});
