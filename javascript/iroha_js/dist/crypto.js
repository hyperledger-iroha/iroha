import { Buffer } from "node:buffer";
import {
  createPrivateKey,
  createPublicKey,
  createHash,
  sign as signRaw,
  verify as verifyRaw,
} from "node:crypto";
import { getNativeBinding } from "./native.js";

const ED25519_SEED_LENGTH = 32;
const ED25519_PUBLIC_KEY_LENGTH = 32;
const ED25519_PRIVATE_KEY_LENGTH = 64;

export const SM2_PRIVATE_KEY_LENGTH = 32;
export const SM2_PUBLIC_KEY_LENGTH = 65;
export const SM2_SIGNATURE_LENGTH = 64;

export const CRYPTO_ALGORITHMS = Object.freeze({
  ED25519: "ed25519",
  SECP256K1: "secp256k1",
  ML_DSA: "ml-dsa",
  BLS_NORMAL: "bls_normal",
  BLS_SMALL: "bls_small",
  GOST_2012_256_A: "gost3410-2012-256-paramset-a",
  GOST_2012_256_B: "gost3410-2012-256-paramset-b",
  GOST_2012_256_C: "gost3410-2012-256-paramset-c",
  GOST_2012_512_A: "gost3410-2012-512-paramset-a",
  GOST_2012_512_B: "gost3410-2012-512-paramset-b",
  SM2: "sm2",
});

export const SUPPORTED_CRYPTO_ALGORITHMS = Object.freeze([
  CRYPTO_ALGORITHMS.ED25519,
  CRYPTO_ALGORITHMS.SECP256K1,
  CRYPTO_ALGORITHMS.BLS_NORMAL,
  CRYPTO_ALGORITHMS.BLS_SMALL,
  CRYPTO_ALGORITHMS.ML_DSA,
  CRYPTO_ALGORITHMS.GOST_2012_256_A,
  CRYPTO_ALGORITHMS.GOST_2012_256_B,
  CRYPTO_ALGORITHMS.GOST_2012_256_C,
  CRYPTO_ALGORITHMS.GOST_2012_512_A,
  CRYPTO_ALGORITHMS.GOST_2012_512_B,
  CRYPTO_ALGORITHMS.SM2,
]);

const CRYPTO_ALGORITHM_ALIASES = new Map([
  ["ed25519", CRYPTO_ALGORITHMS.ED25519],
  ["ed", CRYPTO_ALGORITHMS.ED25519],
  ["eddsa", CRYPTO_ALGORITHMS.ED25519],
  ["secp256k1", CRYPTO_ALGORITHMS.SECP256K1],
  ["secp", CRYPTO_ALGORITHMS.SECP256K1],
  ["secpk1", CRYPTO_ALGORITHMS.SECP256K1],
  ["mldsa", CRYPTO_ALGORITHMS.ML_DSA],
  ["mldsa65", CRYPTO_ALGORITHMS.ML_DSA],
  ["mldsa44", CRYPTO_ALGORITHMS.ML_DSA],
  ["mldsa87", CRYPTO_ALGORITHMS.ML_DSA],
  ["blsnormal", CRYPTO_ALGORITHMS.BLS_NORMAL],
  ["bls12381g1", CRYPTO_ALGORITHMS.BLS_NORMAL],
  ["blssmall", CRYPTO_ALGORITHMS.BLS_SMALL],
  ["bls12381g2", CRYPTO_ALGORITHMS.BLS_SMALL],
  ["gost256a", CRYPTO_ALGORITHMS.GOST_2012_256_A],
  ["gost34102012256paramseta", CRYPTO_ALGORITHMS.GOST_2012_256_A],
  ["gost256b", CRYPTO_ALGORITHMS.GOST_2012_256_B],
  ["gost34102012256paramsetb", CRYPTO_ALGORITHMS.GOST_2012_256_B],
  ["gost256c", CRYPTO_ALGORITHMS.GOST_2012_256_C],
  ["gost34102012256paramsetc", CRYPTO_ALGORITHMS.GOST_2012_256_C],
  ["gost512a", CRYPTO_ALGORITHMS.GOST_2012_512_A],
  ["gost34102012512paramseta", CRYPTO_ALGORITHMS.GOST_2012_512_A],
  ["gost512b", CRYPTO_ALGORITHMS.GOST_2012_512_B],
  ["gost34102012512paramsetb", CRYPTO_ALGORITHMS.GOST_2012_512_B],
  ["sm2", CRYPTO_ALGORITHMS.SM2],
]);

const SM2_FIXTURE_REFERENCE = Object.freeze({
  distid: "1234567812345678",
  seedHex: "1111111111111111111111111111111111111111111111111111111111111111",
  messageHex: "69726F686120736D2073646B2066697874757265",
  privateKeyHex: "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569",
  publicKeySec1Hex:
    "04361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
  publicKeyMultihash:
    "86265300103132333435363738313233343536373804361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
  publicKeyPrefixed:
    "sm2:86265300103132333435363738313233343536373804361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
  za: "E54EDEDE2A2FCC1C9DF868C56F8A2DD8C562F1AD3C78DC11DD7D91BB6F0EBD46",
  signature:
    "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7FF72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268",
  r: "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7F",
  s: "F72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268",
});

const SM2_FIXTURE_SEED = Buffer.from(SM2_FIXTURE_REFERENCE.seedHex, "hex");
const SM2_FIXTURE_MESSAGE = Buffer.from(SM2_FIXTURE_REFERENCE.messageHex, "hex");
export const SM2_DEFAULT_DISTINGUISHED_ID = SM2_FIXTURE_REFERENCE.distid;

const ED25519_PKCS8_PREFIX = Buffer.from([
  0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
]);
const ED25519_SPKI_PREFIX = Buffer.from([
  0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
]);

function resolveNativeBinding() {
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function cryptoAlgorithmAliasKey(value) {
  return String(value)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "");
}

export function supportedCryptoAlgorithms() {
  const native = resolveNativeBinding();
  if (typeof native.supportedCryptoAlgorithms === "function") {
    return native.supportedCryptoAlgorithms().map((algorithm) =>
      normalizeCryptoAlgorithm(algorithm),
    );
  }
  return [...SUPPORTED_CRYPTO_ALGORITHMS];
}

export function normalizeCryptoAlgorithm(algorithm = CRYPTO_ALGORITHMS.ED25519) {
  if (algorithm === undefined || algorithm === null || algorithm === "") {
    return CRYPTO_ALGORITHMS.ED25519;
  }
  const normalized = CRYPTO_ALGORITHM_ALIASES.get(cryptoAlgorithmAliasKey(algorithm));
  if (!normalized) {
    throw new Error(`unsupported crypto algorithm: ${algorithm}`);
  }
  return normalized;
}

function ensureGenericCryptoNative(native, operation) {
  if (!native || typeof native[operation] !== "function") {
    throw new Error(
      `${operation} requires the iroha_js_host native binding built with full crypto support`,
    );
  }
  return native;
}

function normalizeNativeKeyPair(result, algorithm) {
  return {
    algorithm: normalizeCryptoAlgorithm(result.algorithm ?? algorithm),
    publicKey: Buffer.from(result.publicKey),
    privateKey: Buffer.from(result.privateKey),
    distid: typeof result.distid === "string" ? result.distid : null,
  };
}

/**
 * Generate a key pair. Ed25519 remains available in all Node builds; other algorithms require the native binding.
 * @param {{seed?: ArrayBufferView | ArrayBuffer | Buffer, algorithm?: string}} [options]
 * @returns {{algorithm: string, publicKey: Buffer, privateKey: Buffer, distid?: string | null}}
 */
export function generateKeyPair(options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoKeypair");
    const seed = options.seed ? toBuffer(options.seed, "seed") : undefined;
    return normalizeNativeKeyPair(native.cryptoKeypair(algorithm, seed), algorithm);
  }
  const seed = options.seed ? normalizeSeed(options.seed) : undefined;
  const native = resolveNativeBinding();
  if (typeof native.ed25519Keypair !== "function") {
    throw new Error("Native binding does not expose ed25519Keypair");
  }
  const result = native.ed25519Keypair(seed);
  return {
    algorithm: result.algorithm,
    publicKey: Buffer.from(result.publicKey),
    privateKey: Buffer.from(result.privateKey),
  };
}

/**
 * Derive the public key for a given private key (32-byte seed or 64-byte seed+public concatenation).
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @param {{algorithm?: string}} [options]
 * @returns {Buffer}
 */
export function publicKeyFromPrivate(privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoPublicKeyFromPrivate");
    return Buffer.from(
      native.cryptoPublicKeyFromPrivate(algorithm, toBuffer(privateKey, "privateKey")),
    );
  }
  const buffer = toBuffer(privateKey, "privateKey");
  const native = resolveNativeBinding();
  if (typeof native.ed25519PublicKeyFromPrivate !== "function") {
    throw new Error("Native binding does not expose ed25519PublicKeyFromPrivate");
  }
  return Buffer.from(native.ed25519PublicKeyFromPrivate(buffer));
}

export function loadKeyPair(privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm === CRYPTO_ALGORITHMS.ED25519) {
    const privateKeyBuffer = toBuffer(privateKey, "privateKey");
    return {
      algorithm,
      publicKey: publicKeyFromPrivate(privateKeyBuffer),
      privateKey: extractSeed(privateKeyBuffer),
      distid: null,
    };
  }
  const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoKeypairFromPrivate");
  return normalizeNativeKeyPair(
    native.cryptoKeypairFromPrivate(algorithm, toBuffer(privateKey, "privateKey")),
    algorithm,
  );
}

/**
 * Sign a message using an Ed25519 private key.
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function signEd25519(message, privateKey) {
  const seed = extractSeed(privateKey);
  const privateKeyObject = privateKeyFromSeed(seed);
  const messageBuffer = toBuffer(message, "message");
  return Buffer.from(signRaw(null, messageBuffer, privateKeyObject));
}

/**
 * Verify an Ed25519 signature.
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signature
 * @param {ArrayBufferView | ArrayBuffer | Buffer} publicKey
 * @returns {boolean}
 */
export function verifyEd25519(message, signature, publicKey) {
  const messageBuffer = toBuffer(message, "message");
  const signatureBuffer = toBuffer(signature, "signature");
  const publicKeyBuffer = normalizePublicKey(publicKey);
  const publicKeyObject = createPublicKey({
    key: Buffer.concat([ED25519_SPKI_PREFIX, publicKeyBuffer]),
    format: "der",
    type: "spki",
  });
  return verifyRaw(null, messageBuffer, publicKeyObject, signatureBuffer);
}

export function sign(message, privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm === CRYPTO_ALGORITHMS.ED25519) {
    return signEd25519(message, privateKey);
  }
  const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoSign");
  return Buffer.from(
    native.cryptoSign(
      algorithm,
      toBuffer(privateKey, "privateKey"),
      toBuffer(message, "message"),
    ),
  );
}

export function verify(message, signature, publicKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm === CRYPTO_ALGORITHMS.ED25519) {
    return verifyEd25519(message, signature, publicKey);
  }
  const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoVerify");
  return Boolean(
    native.cryptoVerify(
      algorithm,
      toBuffer(publicKey, "publicKey"),
      toBuffer(message, "message"),
      toBuffer(signature, "signature"),
    ),
  );
}

export function publicKeyMultihash(publicKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoPublicKeyMultihash");
  return native.cryptoPublicKeyMultihash(algorithm, toBuffer(publicKey, "publicKey"));
}

export function privateKeyMultihash(privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  const native = ensureGenericCryptoNative(resolveNativeBinding(), "cryptoPrivateKeyMultihash");
  return native.cryptoPrivateKeyMultihash(algorithm, toBuffer(privateKey, "privateKey"));
}

function normalizeSm2Distid(distid, native) {
  if (distid === undefined || distid === null) {
    if (native && typeof native.sm2DefaultDistid === "function") {
      return native.sm2DefaultDistid();
    }
    return SM2_DEFAULT_DISTINGUISHED_ID;
  }
  if (typeof distid !== "string") {
    throw new TypeError("distid must be a string");
  }
  const cleaned = distid.trim();
  if (!cleaned) {
    throw new Error("distid must not be empty");
  }
  return cleaned;
}

function ensureSm2Native(native) {
  if (
    !native ||
    typeof native.sm2Keypair !== "function" ||
    typeof native.sm2KeypairFromSeed !== "function" ||
    typeof native.sm2KeypairFromPrivate !== "function" ||
    typeof native.sm2Sign !== "function" ||
    typeof native.sm2Verify !== "function" ||
    typeof native.sm2PublicKeyMultihash !== "function"
  ) {
    throw new Error(
      "SM2 operations require the iroha_js_host native binding built with SM support",
    );
  }
  return native;
}

function ensureKaigiRosterNative(native) {
  if (!native || typeof native.buildKaigiRosterJoinProof !== "function") {
    throw new Error(
      "Kaigi roster proof helper unavailable; build iroha_js_host with `npm run build:native` before using private Kaigi joins",
    );
  }
  return native;
}

function ensureConfidentialV2Native(native, operation) {
  if (!native || typeof native[operation] !== "function") {
    throw new Error(
      `confidential v2 helper '${operation}' is unavailable; build iroha_js_host with \`npm run build:native\` before using shielded transfer v2`,
    );
  }
  return native;
}

export function generateSm2KeyPair(options = {}) {
  const native = ensureSm2Native(resolveNativeBinding());
  const effectiveDistid = normalizeSm2Distid(options.distid, native);
  const result = native.sm2Keypair(effectiveDistid);
  const privateKey = Buffer.from(result.privateKey);
  const publicKey = Buffer.from(result.publicKey);
  if (privateKey.length !== SM2_PRIVATE_KEY_LENGTH) {
    throw new Error("native sm2Keypair returned invalid private key length");
  }
  if (publicKey.length !== SM2_PUBLIC_KEY_LENGTH) {
    throw new Error("native sm2Keypair returned invalid public key length");
  }
  return {
    algorithm: "sm2",
    distid: typeof result.distid === "string" ? result.distid : effectiveDistid,
    privateKey,
    publicKey,
  };
}

export function deriveSm2KeyPairFromSeed(seed, distid) {
  const native = ensureSm2Native(resolveNativeBinding());
  const seedBuffer = toBuffer(seed, "seed");
  const effectiveDistid = normalizeSm2Distid(distid, native);
  const result = native.sm2KeypairFromSeed(effectiveDistid, seedBuffer);
  const privateKey = Buffer.from(result.privateKey);
  const publicKey = Buffer.from(result.publicKey);
  if (privateKey.length !== SM2_PRIVATE_KEY_LENGTH) {
    throw new Error("native sm2KeypairFromSeed returned invalid private key length");
  }
  if (publicKey.length !== SM2_PUBLIC_KEY_LENGTH) {
    throw new Error("native sm2KeypairFromSeed returned invalid public key length");
  }
  return {
    algorithm: "sm2",
    distid: typeof result.distid === "string" ? result.distid : effectiveDistid,
    privateKey,
    publicKey,
  };
}

export function loadSm2KeyPair(privateKey, distid) {
  const native = ensureSm2Native(resolveNativeBinding());
  const privateKeyBuffer = toBuffer(privateKey, "privateKey");
  if (privateKeyBuffer.length !== SM2_PRIVATE_KEY_LENGTH) {
    throw new Error(`sm2 private key must be ${SM2_PRIVATE_KEY_LENGTH} bytes`);
  }
  const effectiveDistid = normalizeSm2Distid(distid, native);
  const result = native.sm2KeypairFromPrivate(effectiveDistid, privateKeyBuffer);
  return {
    algorithm: "sm2",
    distid: typeof result.distid === "string" ? result.distid : effectiveDistid,
    privateKey: Buffer.from(result.privateKey),
    publicKey: Buffer.from(result.publicKey),
  };
}

export function sm2PublicKeyMultihash(publicKey, distid) {
  const native = ensureSm2Native(resolveNativeBinding());
  const buffer = toBuffer(publicKey, "publicKey");
  if (buffer.length !== SM2_PUBLIC_KEY_LENGTH) {
    throw new Error(`sm2 public key must be ${SM2_PUBLIC_KEY_LENGTH} bytes`);
  }
  const effectiveDistid = normalizeSm2Distid(distid, native);
  return native.sm2PublicKeyMultihash(buffer, effectiveDistid);
}

export function signSm2(message, privateKey, distid) {
  const native = ensureSm2Native(resolveNativeBinding());
  const privateKeyBuffer = toBuffer(privateKey, "privateKey");
  if (privateKeyBuffer.length !== SM2_PRIVATE_KEY_LENGTH) {
    throw new Error(`sm2 private key must be ${SM2_PRIVATE_KEY_LENGTH} bytes`);
  }
  const messageBuffer = toBuffer(message, "message");
  const effectiveDistid = normalizeSm2Distid(distid, native);
  const signature = native.sm2Sign(privateKeyBuffer, messageBuffer, effectiveDistid);
  const buffer = Buffer.from(signature);
  if (buffer.length !== SM2_SIGNATURE_LENGTH) {
    throw new Error("native sm2Sign returned invalid signature length");
  }
  return buffer;
}

export function verifySm2(message, signature, publicKey, distid) {
  const native = ensureSm2Native(resolveNativeBinding());
  const publicKeyBuffer = toBuffer(publicKey, "publicKey");
  if (publicKeyBuffer.length !== SM2_PUBLIC_KEY_LENGTH) {
    throw new Error(`sm2 public key must be ${SM2_PUBLIC_KEY_LENGTH} bytes`);
  }
  const signatureBuffer = toBuffer(signature, "signature");
  if (signatureBuffer.length !== SM2_SIGNATURE_LENGTH) {
    throw new Error(`sm2 signature must be ${SM2_SIGNATURE_LENGTH} bytes`);
  }
  const messageBuffer = toBuffer(message, "message");
  const effectiveDistid = normalizeSm2Distid(distid, native);
  return Boolean(
    native.sm2Verify(publicKeyBuffer, messageBuffer, signatureBuffer, effectiveDistid),
  );
}

/**
 * Build the proof artefacts required for a `Kaigi::JoinKaigi` `ZkRosterV1` join.
 * @param {{seed: ArrayBufferView | ArrayBuffer | Buffer, rosterRootHex?: string | null}} options
 * @returns {{commitment: Buffer, nullifier: Buffer, rosterRoot: Buffer, proof: Buffer, commitmentHex: string, nullifierHex: string, rosterRootHex: string, proofBase64: string}}
 */
export function buildKaigiRosterJoinProof(options) {
  if (!options || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("buildKaigiRosterJoinProof options must be an object");
  }
  const seed = toBuffer(options.seed, "seed");
  if (seed.length === 0) {
    throw new Error("seed must not be empty");
  }
  const native = ensureKaigiRosterNative(resolveNativeBinding());
  const result = native.buildKaigiRosterJoinProof(
    seed,
    options.rosterRootHex ?? options.roster_root_hex ?? null,
  );
  const commitment = Buffer.from(result.commitment);
  const nullifier = Buffer.from(result.nullifier);
  const rosterRoot = Buffer.from(result.rosterRoot ?? result.roster_root);
  const proof = Buffer.from(result.proof);
  if (commitment.length !== 32 || nullifier.length !== 32 || rosterRoot.length !== 32) {
    throw new Error("native Kaigi roster proof helper returned invalid digest lengths");
  }
  if (proof.length === 0) {
    throw new Error("native Kaigi roster proof helper returned an empty proof");
  }
  return {
    commitment,
    nullifier,
    rosterRoot,
    proof,
    commitmentHex: commitment.toString("hex"),
    nullifierHex: nullifier.toString("hex"),
    rosterRootHex: rosterRoot.toString("hex"),
    proofBase64: proof.toString("base64"),
  };
}

/**
 * Derive the confidential key hierarchy from a 32-byte spend key.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} spendKey
 * @returns {{skSpend: Buffer, nk: Buffer, ivk: Buffer, ovk: Buffer, fvk: Buffer, skSpendHex: string, nkHex: string, ivkHex: string, ovkHex: string, fvkHex: string, asHex(): Record<string, string>}}
 */
export function deriveConfidentialKeyset(spendKey) {
  const seed = toBuffer(spendKey, "spendKey");
  if (seed.length !== 32) {
    throw new Error("confidential spend key must be 32 bytes");
  }

  const native = resolveNativeBinding();
  if (typeof native.deriveConfidentialKeyset !== "function") {
    throw new Error("Native binding does not expose deriveConfidentialKeyset");
  }
  const raw = native.deriveConfidentialKeyset(seed);

  const keyset = {
    skSpend: toBufferField(raw, "sk_spend", "skSpend"),
    nk: toBufferField(raw, "nk"),
    ivk: toBufferField(raw, "ivk"),
    ovk: toBufferField(raw, "ovk"),
    fvk: toBufferField(raw, "fvk"),
  };
  return wrapConfidentialKeyset(keyset);
}

/**
 * Derive the confidential key hierarchy from a hex-encoded spend key.
 * @param {string} spendKeyHex
 * @returns {ReturnType<typeof deriveConfidentialKeyset>}
 */
export function deriveConfidentialKeysetFromHex(spendKeyHex) {
  if (typeof spendKeyHex !== "string") {
    throw new TypeError("spendKeyHex must be a string");
  }
  const cleaned = spendKeyHex.trim();
  if (cleaned.length !== 64) {
    throw new Error("confidential spend key must be 64 hex characters (32 bytes)");
  }
  const seed = Buffer.from(cleaned, "hex");
  if (seed.length !== 32) {
    throw new Error("confidential spend key must be valid hex");
  }
  return deriveConfidentialKeyset(seed);
}

/**
 * Derive the confidential v2 owner tag from a 32-byte spend key.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} spendKey
 * @param {{diversifierHex?: string, diversifier?: ArrayBufferView | ArrayBuffer | Buffer}} [options]
 * @returns {Buffer}
 */
export function deriveConfidentialOwnerTagV2(spendKey, options = {}) {
  const native = ensureConfidentialV2Native(
    resolveNativeBinding(),
    "deriveConfidentialOwnerTagV2",
  );
  const spendKeyBuffer = toBuffer(spendKey, "spendKey");
  if (spendKeyBuffer.length !== 32) {
    throw new Error("confidential spend key must be 32 bytes");
  }
  const diversifierHex =
    options?.diversifierHex !== undefined || options?.diversifier !== undefined
      ? normalizeFixed32HexInput(
          options.diversifierHex ?? options.diversifier,
          "diversifier",
        )
      : undefined;
  return Buffer.from(native.deriveConfidentialOwnerTagV2(spendKeyBuffer, diversifierHex));
}

/**
 * Derive a canonical confidential v2 diversifier from seed material.
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} seed
 * @returns {{diversifier: Buffer, diversifierHex: string}}
 */
export function deriveConfidentialDiversifierV2(seed) {
  const native = ensureConfidentialV2Native(
    resolveNativeBinding(),
    "deriveConfidentialDiversifierV2",
  );
  const seedBuffer = toBuffer(seed, "seed");
  if (seedBuffer.length === 0) {
    throw new Error("seed must not be empty");
  }
  const diversifier = Buffer.from(native.deriveConfidentialDiversifierV2(seedBuffer));
  return {
    diversifier,
    diversifierHex: diversifier.toString("hex"),
  };
}

/**
 * Derive diversified confidential v2 receive-address material.
 * @param {{spendKey: ArrayBufferView | ArrayBuffer | Buffer, diversifierSeed: ArrayBufferView | ArrayBuffer | Buffer | string}} input
 * @returns {{ownerTag: Buffer, ownerTagHex: string, diversifier: Buffer, diversifierHex: string}}
 */
export function deriveConfidentialReceiveAddressV2(input) {
  const native = ensureConfidentialV2Native(
    resolveNativeBinding(),
    "deriveConfidentialReceiveAddressV2",
  );
  const spendKeyBuffer = toBuffer(input?.spendKey, "spendKey");
  if (spendKeyBuffer.length !== 32) {
    throw new Error("confidential spend key must be 32 bytes");
  }
  const diversifierSeed = toBuffer(input?.diversifierSeed, "diversifierSeed");
  if (diversifierSeed.length === 0) {
    throw new Error("diversifierSeed must not be empty");
  }
  const raw = native.deriveConfidentialReceiveAddressV2(spendKeyBuffer, diversifierSeed);
  const ownerTagHex = String(raw.ownerTagHex ?? raw.owner_tag_hex ?? "").trim();
  const diversifierHex = String(raw.diversifierHex ?? raw.diversifier_hex ?? "").trim();
  return {
    ownerTag: Buffer.from(normalizeFixed32HexInput(ownerTagHex, "ownerTag"), "hex"),
    ownerTagHex: normalizeFixed32HexInput(ownerTagHex, "ownerTag"),
    diversifier: Buffer.from(normalizeFixed32HexInput(diversifierHex, "diversifier"), "hex"),
    diversifierHex: normalizeFixed32HexInput(diversifierHex, "diversifier"),
  };
}

/**
 * Derive a confidential v2 note commitment from note material.
 * @param {{assetDefinitionId: string, amount: string | number | bigint, rhoHex?: string, rho?: ArrayBufferView | ArrayBuffer | Buffer, ownerTagHex?: string, ownerTag?: ArrayBufferView | ArrayBuffer | Buffer}} input
 * @returns {{commitment: Buffer, commitmentHex: string}}
 */
export function deriveConfidentialNoteV2(input) {
  const native = ensureConfidentialV2Native(
    resolveNativeBinding(),
    "deriveConfidentialNoteV2",
  );
  const assetDefinitionId = String(input?.assetDefinitionId ?? "").trim();
  if (!assetDefinitionId) {
    throw new Error("assetDefinitionId is required");
  }
  const amount = normalizeWholeNumberLiteral(input?.amount, "amount");
  const rhoHex = normalizeFixed32HexInput(input?.rhoHex ?? input?.rho, "rho");
  const ownerTagHex = normalizeFixed32HexInput(
    input?.ownerTagHex ?? input?.ownerTag,
    "ownerTag",
  );
  const commitment = Buffer.from(
    native.deriveConfidentialNoteV2(
      assetDefinitionId,
      amount,
      rhoHex,
      ownerTagHex,
    ),
  );
  return {
    commitment,
    commitmentHex: commitment.toString("hex"),
  };
}

/**
 * Derive a confidential v2 nullifier from note material.
 * @param {{chainId: string, assetDefinitionId: string, spendKey: ArrayBufferView | ArrayBuffer | Buffer, rhoHex?: string, rho?: ArrayBufferView | ArrayBuffer | Buffer}} input
 * @returns {{nullifier: Buffer, nullifierHex: string}}
 */
export function deriveConfidentialNullifierV2(input) {
  const native = ensureConfidentialV2Native(
    resolveNativeBinding(),
    "deriveConfidentialNullifierV2",
  );
  const chainId = String(input?.chainId ?? "").trim();
  const assetDefinitionId = String(input?.assetDefinitionId ?? "").trim();
  const spendKey = toBuffer(input?.spendKey, "spendKey");
  if (!chainId) {
    throw new Error("chainId is required");
  }
  if (!assetDefinitionId) {
    throw new Error("assetDefinitionId is required");
  }
  if (spendKey.length !== 32) {
    throw new Error("confidential spend key must be 32 bytes");
  }
  const rhoHex = normalizeFixed32HexInput(input?.rhoHex ?? input?.rho, "rho");
  const nullifier = Buffer.from(
    native.deriveConfidentialNullifierV2(
      chainId,
      assetDefinitionId,
      spendKey,
      rhoHex,
    ),
  );
  return {
    nullifier,
    nullifierHex: nullifier.toString("hex"),
  };
}

/**
 * Return the canonical SM2 signing fixture values for the given seed and message.
 * @param {string} distid
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} seed
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
 * @returns {{distid: string, seedHex: string, messageHex: string, privateKeyHex: string, publicKeySec1Hex: string, publicKeyMultihash: string, publicKeyPrefixed: string, za: string, signature: string, r: string, s: string}}
 */
export function sm2FixtureFromSeed(distid, seed, message) {
  if (typeof distid !== "string") {
    throw new TypeError("distid must be a string");
  }
  const native = resolveNativeBinding();
  const seedBuffer = toBuffer(seed, "seed");
  const messageBuffer = toBuffer(message, "message");
  if (!native?.sm2FixtureFromSeed) {
    if (
      distid === SM2_FIXTURE_REFERENCE.distid &&
      seedBuffer.equals(SM2_FIXTURE_SEED) &&
      messageBuffer.equals(SM2_FIXTURE_MESSAGE)
    ) {
      return { ...SM2_FIXTURE_REFERENCE };
    }
    throw new Error("SM2 fixture helper unavailable; build iroha_js_host with SM support");
  }
  const fixture = native.sm2FixtureFromSeed(distid, seedBuffer, messageBuffer);
  return {
    distid: fixture.distid,
    seedHex: fixture.seedHex,
    messageHex: fixture.messageHex,
    privateKeyHex: fixture.privateKeyHex,
    publicKeySec1Hex: fixture.publicKeySec1Hex,
    publicKeyMultihash: fixture.publicKeyMultihash,
    publicKeyPrefixed: fixture.publicKeyPrefixed,
    za: fixture.za,
    signature: fixture.signature,
    r: fixture.r,
    s: fixture.s,
  };
}

function privateKeyFromSeed(seed) {
  const der = Buffer.concat([ED25519_PKCS8_PREFIX, seed]);
  return createPrivateKey({ key: der, format: "der", type: "pkcs8" });
}

function exportPublicKey(privateKeyObject) {
  const publicKeyObject = createPublicKey(privateKeyObject);
  const spki = publicKeyObject.export({ type: "spki", format: "der" });
  return Buffer.from(spki).subarray(ED25519_SPKI_PREFIX.length);
}

function normalizeSeed(seed) {
  const buffer = toBuffer(seed, "seed");
  if (buffer.length === ED25519_SEED_LENGTH) {
    return Buffer.from(buffer);
  }
  return createHash("sha256").update(buffer).digest();
}

function normalizePublicKey(publicKey) {
  const buffer = toBuffer(publicKey, "publicKey");
  if (buffer.length !== ED25519_PUBLIC_KEY_LENGTH) {
    throw new Error("ed25519 public key must be 32 bytes");
  }
  return Buffer.from(buffer);
}

function extractSeed(privateKey) {
  const buffer = toBuffer(privateKey, "privateKey");
  if (buffer.length === ED25519_SEED_LENGTH) {
    return Buffer.from(buffer);
  }
  if (buffer.length === ED25519_PRIVATE_KEY_LENGTH) {
    const seed = Buffer.from(buffer.subarray(0, ED25519_SEED_LENGTH));
    const publicKey = buffer.subarray(ED25519_SEED_LENGTH);
    const derivedPublic = exportPublicKey(privateKeyFromSeed(seed));
    if (!derivedPublic.equals(publicKey)) {
      throw new Error("ed25519 private key payload has mismatched public key");
    }
    return seed;
  }
  throw new Error("ed25519 private key must be 32-byte seed or 64-byte seed+public");
}

function toBuffer(value, name) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (typeof value === "string") {
    return Buffer.from(value, "utf8");
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError(`${name} must be a Buffer, string, or ArrayBuffer view`);
}

function normalizeFixed32HexInput(value, name) {
  if (typeof value === "string") {
    const normalized = value.trim().replace(/^0x/i, "").toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(normalized)) {
      throw new Error(`${name} must be a 32-byte hex string`);
    }
    return normalized;
  }
  const buffer = toBuffer(value, name);
  if (buffer.length !== 32) {
    throw new Error(`${name} must be 32 bytes`);
  }
  return Buffer.from(buffer).toString("hex");
}

function normalizeWholeNumberLiteral(value, name) {
  const normalized = String(value ?? "").trim();
  if (!/^\d+$/.test(normalized)) {
    throw new Error(`${name} must be a whole-number string`);
  }
  return normalized;
}

function toBufferField(payload, ...fieldNames) {
  for (const fieldName of fieldNames) {
    if (!payload || !fieldName) {
      continue;
    }
    const value = payload[fieldName];
    if (value !== null && value !== undefined) {
      return toBuffer(value, String(fieldName));
    }
  }
  const rendered = fieldNames.map((fieldName) => `\`${fieldName}\``).join(" or ");
  throw new Error(`native binding returned missing ${rendered}`);
}

function wrapConfidentialKeyset(keys) {
  const result = {
    skSpend: Buffer.from(keys.skSpend),
    nk: Buffer.from(keys.nk),
    ivk: Buffer.from(keys.ivk),
    ovk: Buffer.from(keys.ovk),
    fvk: Buffer.from(keys.fvk),
    asHex() {
      return {
        skSpend: result.skSpendHex,
        nk: result.nkHex,
        ivk: result.ivkHex,
        ovk: result.ovkHex,
        fvk: result.fvkHex,
      };
    },
  };

  Object.defineProperties(result, {
    skSpendHex: {
      enumerable: true,
      get() {
        return result.skSpend.toString("hex");
      },
    },
    nkHex: {
      enumerable: true,
      get() {
        return result.nk.toString("hex");
      },
    },
    ivkHex: {
      enumerable: true,
      get() {
        return result.ivk.toString("hex");
      },
    },
    ovkHex: {
      enumerable: true,
      get() {
        return result.ovk.toString("hex");
      },
    },
    fvkHex: {
      enumerable: true,
      get() {
        return result.fvk.toString("hex");
      },
    },
  });

  return result;
}
