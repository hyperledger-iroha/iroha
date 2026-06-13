import { Buffer } from "node:buffer";
import {
  createPrivateKey,
  createPublicKey,
  createHash,
  sign as signRaw,
  verify as verifyRaw,
} from "node:crypto";
import { AccountAddress } from "./address.js";
import { getNativeBinding } from "./native.js";

const ED25519_SEED_LENGTH = 32;
const ED25519_PUBLIC_KEY_LENGTH = 32;
const ED25519_PRIVATE_KEY_LENGTH = 64;

export const SM2_PRIVATE_KEY_LENGTH = 32;
export const SM2_PUBLIC_KEY_LENGTH = 65;
export const SM2_SIGNATURE_LENGTH = 64;
export const PRIVACY_FFI_VERSION_V1 = 1;
export const PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 7;
export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
export const PRIVACY_FFI_STATUS_ERROR = 1;
export const PRIVACY_FFI_ERROR_NULL_POINTER = 1;
export const PRIVACY_FFI_ERROR_MALFORMED_NORITO = 2;
export const PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM = 3;
export const PRIVACY_FFI_ERROR_PRODUCTION_DISABLED = 4;
export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;
const PRIVACY_MAX_BRIDGE_ABI_VERSION = 0xffff_ffff;
const KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION = 0xffff_ffff;
const ZK_ACE_ALGORITHM_ID = "zk-ace-pq-authorization-v0";
const ZK_ACE_PRODUCTION_ENTRYPOINT = "buildZkAceAuthorizationProofV1";
const ZK_ACE_PRODUCTION_VK_REF = "stark-fri:zk_ace_pq_authorization_v0";
const ZK_ACE_PRODUCTION_DISABLED_MESSAGE =
  "native ZK-ACE prover returned PRIVACY_FFI_ERROR_PRODUCTION_DISABLED for " +
  `${ZK_ACE_ALGORITHM_ID} ${ZK_ACE_PRODUCTION_ENTRYPOINT} ` +
  `${ZK_ACE_PRODUCTION_VK_REF}: ` +
  "Iroha production allowlist is not enabled for this audited row";
const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
const PRIVACY_NORITO_HEADER_BYTES = 40;
const PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES = 64;
const PRIVACY_NORITO_SUPPORTED_FLAGS_MASK = 0x27;
const PRIVACY_NORITO_FIELD_BITSET_FLAG = 0x20;
const PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06;
const PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE = 0x50;
const PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE = 0x42;
const PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE = 0x56;
const PRIVACY_REQUEST_SCHEMA_BYTE = 0x52;
const PRIVACY_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const PRIVACY_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE = (() => {
  const archive = Buffer.alloc(PRIVACY_NORITO_HEADER_BYTES);
  archive.write("NRT0", 0, "ascii");
  archive.fill(PRIVACY_REQUEST_SCHEMA_BYTE, 6, 22);
  return archive;
})();
const PRIVACY_NORITO_MAGIC = Buffer.from("NRT0", "ascii");
const KAGEMUSHA_ZK1_MAGIC = Buffer.from([0x5a, 0x4b, 0x31, 0x00]);
const KAGEMUSHA_ZK1_TLV_CID1 = Buffer.from("CID1", "ascii");
const KAGEMUSHA_ZK1_TLV_IPAK = Buffer.from("IPAK", "ascii");
const KAGEMUSHA_ZK1_TLV_H2VK = Buffer.from("H2VK", "ascii");
const KAGEMUSHA_NORITO_COMPACT_LEN_FLAG = 0x02;
const KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG = 0x04;
const KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 = 1;
const KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = Buffer.from(
  "c88489618a012c283ff3bb2ebabc7775",
  "hex",
);
const PRIVACY_CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ PRIVACY_CRC64_REFLECTED_POLY
          : crc >> 1n;
    }
    table[index] = crc;
  }
  return table;
})();

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
  const raw = String(value);
  const trimmed = raw.trim();
  if (!/^[\x20-\x7e]+$/.test(trimmed)) {
    throw new Error(`unsupported crypto algorithm: ${raw}`);
  }
  return trimmed.toLowerCase().replace(/[^a-z0-9]/g, "");
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
 * Build a native STARK/FRI-backed ZK-ACE transparent-transfer authorization.
 * @param {{fromAccountId?: string, from?: string, toAccountId?: string, to?: string, assetDefinitionId?: string, asset?: string, amount: string | number | bigint, chainId?: string, chain_id?: string, identityRoot?: ArrayBufferView | ArrayBuffer | Buffer | string, identity_root?: ArrayBufferView | ArrayBuffer | Buffer | string, identityBlinding?: ArrayBufferView | ArrayBuffer | Buffer | string, identity_blinding?: ArrayBufferView | ArrayBuffer | Buffer | string, replaySecret?: ArrayBufferView | ArrayBuffer | Buffer | string, replay_secret?: ArrayBufferView | ArrayBuffer | Buffer | string, policyHash?: ArrayBufferView | ArrayBuffer | Buffer | string, policy_hash?: ArrayBufferView | ArrayBuffer | Buffer | string, verifierKeyId?: string, verifier_key_id?: string, verifyingKeyCommitment?: ArrayBufferView | ArrayBuffer | Buffer | string, vkCommitment?: ArrayBufferView | ArrayBuffer | Buffer | string}} options
 * @returns {object}
 */
export function buildZkAceTransferAuthorizationV1(options) {
  if (!options || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("buildZkAceTransferAuthorizationV1 options must be an object");
  }
  const native = ensureGenericCryptoNative(
    resolveNativeBinding(),
    "zkAceBuildTransferAuthorizationV1",
  );
  const nativeArgs = zkAceTransferAuthorizationNativeArgs(options);
  let resultJson;
  let nativeError;
  try {
    resultJson = native.zkAceBuildTransferAuthorizationV1(...nativeArgs);
  } catch (error) {
    nativeError = error;
  }
  if (nativeError !== undefined) {
    throw sanitizeZkAceNativeProverError(nativeError);
  }
  let result;
  try {
    result = JSON.parse(resultJson);
  } catch (error) {
    throw new Error(`native ZK-ACE prover returned invalid JSON: ${error.message}`);
  }
  return normalizeZkAceAuthorizationResult(result);
}

function sanitizeZkAceNativeProverError(error) {
  const message = error?.message ?? String(error ?? "");
  if (
    /PRIVACY_FFI_ERROR_PRODUCTION_DISABLED|production[- ]disabled|Iroha production allowlist/i.test(
      message,
    )
  ) {
    return new Error(ZK_ACE_PRODUCTION_DISABLED_MESSAGE);
  }
  return new Error("native ZK-ACE prover failed");
}

function zkAceTransferAuthorizationNativeArgs(options) {
  return [
    requiredString(options.fromAccountId ?? options.from, "fromAccountId"),
    requiredString(options.toAccountId ?? options.to, "toAccountId"),
    requiredString(
      options.assetDefinitionId ?? options.asset_definition_id ?? options.asset,
      "assetDefinitionId",
    ),
    normalizePositiveU128Literal(options.amount, "amount"),
    requiredString(options.chainId ?? options.chain_id, "chainId"),
    fixed32Buffer(options.identityRoot ?? options.identity_root, "identityRoot"),
    fixed32Buffer(options.identityBlinding ?? options.identity_blinding, "identityBlinding"),
    fixed32Buffer(options.replaySecret ?? options.replay_secret, "replaySecret"),
    fixed32Buffer(options.policyHash ?? options.policy_hash, "policyHash"),
    options.verifierKeyId ?? options.verifier_key_id ?? null,
    optionalFixed32Buffer(
      options.verifyingKeyCommitment ??
        options.verifying_key_commitment ??
        options.vkCommitment ??
        options.vk_commitment,
      "verifyingKeyCommitment",
    ),
  ];
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

function hasKagemushaRecursiveSpendNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native.kagemushaRecursiveSpendInit !== "function" ||
    typeof native.kagemushaRecursiveSpendAppend !== "function" ||
    typeof native.kagemushaRecursiveSpendTransitionProfileInit !== "function" ||
    typeof native.kagemushaRecursiveSpendTransitionProfileAppend !== "function" ||
    typeof native.kagemushaRecursiveSpendLineageAppendBoundary !== "function" ||
    typeof native.kagemushaRecursiveSpendLineageWitnessFromInitResult !== "function" ||
    typeof native.kagemushaRecursiveSpendLineageWitnessAppendResult !== "function" ||
    typeof native.kagemushaRecursiveSpendVerify !== "function" ||
    typeof native.kagemushaRecursiveSpendRedeem !== "function"
  ) {
    return false;
  }
  return probeKagemushaRecursiveSpendNative(native);
}

function hasKagemushaRecursiveCompactPaymentTokenNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native
      .kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes !==
      "function"
  ) {
    return false;
  }
  return (
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
      ),
    ) && hasKagemushaRecursiveCompactPaymentTokenVerifierNative(native)
  );
}

function hasKagemushaRecursiveCompactPaymentTokenVerifierNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"
  ) {
    return false;
  }
  return expectKagemushaNativeProbeRejection(() =>
    native.kagemushaVerifyRecursiveCompactPaymentToken(
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
    ),
  );
}

function hasKagemushaRecursiveSpendCompactPaymentTokenProjectionNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle !== "function"
  ) {
    return false;
  }
  return expectKagemushaNativeProbeRejection(() =>
    native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
    ),
  );
}

function hasKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection !== "function" ||
    typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight !== "function"
  ) {
    return false;
  }
  return (
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
      ),
    ) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
        KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
        1,
      ),
    )
  );
}

function hasKagemushaCompactPaymentTokenNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native.kagemushaProveVerifiedCompactPaymentTokenWithRecords !== "function"
  ) {
    return false;
  }
  return expectKagemushaNativeProbeRejection(() =>
    native.kagemushaProveVerifiedCompactPaymentTokenWithRecords(
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
    ),
  );
}

function hasKagemushaRecursiveAggregationProofBundleNative(native) {
  const abiVersion = kagemushaRecursiveSpendNativeBridgeAbiVersion(native);
  if (
    !native ||
    !Number.isInteger(abiVersion) ||
    abiVersion < KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION ||
    typeof native
      .kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes !==
      "function"
  ) {
    return false;
  }
  return expectKagemushaNativeProbeRejection(() =>
    native.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
      KAGEMUSHA_NATIVE_PROBE_ARCHIVE,
    ),
  );
}

function kagemushaRecursiveSpendNativeBridgeAbiVersion(native) {
  if (typeof native?.connectNoritoBridgeAbiVersion !== "function") {
    return 0;
  }
  try {
    const version = native.connectNoritoBridgeAbiVersion();
    return typeof version === "number" &&
      Number.isSafeInteger(version) &&
      version >= 0 &&
      version <= KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION
      ? version
      : 0;
  } catch {
    return 0;
  }
}

const KAGEMUSHA_NATIVE_PROBE_ARCHIVE = Buffer.from([0]);

function isExpectedKagemushaNativeProbeRejection(error) {
  return (
    error instanceof Error &&
    /Kagemusha/i.test(error.message) &&
    /\b(?:archive|Norito|probe)\b/i.test(error.message)
  );
}

function expectKagemushaNativeProbeRejection(probe) {
  try {
    probe();
    return false;
  } catch (error) {
    return isExpectedKagemushaNativeProbeRejection(error);
  }
}

function probeKagemushaRecursiveSpendNative(native) {
  const probeArchive = KAGEMUSHA_NATIVE_PROBE_ARCHIVE;
  return (
    expectKagemushaNativeProbeRejection(() => native.kagemushaRecursiveSpendInit(probeArchive)) &&
    expectKagemushaNativeProbeRejection(() => native.kagemushaRecursiveSpendAppend(probeArchive)) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaRecursiveSpendTransitionProfileInit(probeArchive),
    ) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaRecursiveSpendTransitionProfileAppend(probeArchive),
    ) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaRecursiveSpendLineageAppendBoundary(probeArchive),
    ) &&
    expectKagemushaNativeProbeRejection(() => native.kagemushaRecursiveSpendVerify(probeArchive)) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaRecursiveSpendLineageWitnessFromInitResult(probeArchive, probeArchive),
    ) &&
    expectKagemushaNativeProbeRejection(() =>
      native.kagemushaRecursiveSpendLineageWitnessAppendResult(
        probeArchive,
        probeArchive,
        probeArchive,
      ),
    ) &&
    expectKagemushaNativeProbeRejection(() => native.kagemushaRecursiveSpendRedeem(probeArchive))
  );
}

export const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1 = "recursive_compact_v1";
export const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1 = "recursive_spend_v1";
export const KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1";
export const KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 6;
export const KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 7;
export const KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1 = "kagemusha-recursive-compact-v1";
export const KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT =
  "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch";
export const KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT =
  "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch";
export const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND = "halo2/ipa";
export const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
  "kagemusha-recursive-aggregation-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
  "kagemusha-recursive-spend-lineage-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 =
  "kagemusha-recursive-spend-lineage-onehop-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 =
  "kagemusha-recursive-spend-lineage-append-v1";
export const KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS = 64;
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64;
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = true;
export const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1;
export const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024;
export const KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128;
export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN =
  "iroha:kagemusha:v1:recursive-spend-transition-profile";
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN =
  "iroha:kagemusha:v1:recursive-spend-transition-profile-digest";
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN =
  "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1 =
  "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1 =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1 =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1 =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1";
export const KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1";
export const KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyRequestV1";
export const KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1";
export const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1";
export const KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1";
export const KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1";
export const KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME =
  "iroha_data_model::proof::ProofAttachment";
export const KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME =
  "iroha_data_model::proof::VerifyingKeyRecord";

export function isKagemushaRecursiveCompactUnavailable(error) {
  const message =
    typeof error === "string"
      ? error
      : error && typeof error.message === "string"
        ? error.message
        : "";
  return (
    message.includes(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT) ||
    message.includes(KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT)
  );
}

export function preferredKagemushaOfflineSpendMode(
  recursiveSpendAvailable,
  recursiveCompactAvailable,
) {
  if (arguments.length === 0) {
    return preferredKagemushaOfflineSpendModeForCapabilities(
      isKagemushaRecursiveCompactPaymentTokenNativeAvailable(),
      isKagemushaRecursiveSpendNativeAvailable(),
    );
  }
  return preferredKagemushaOfflineSpendModeForCapabilities(
    arguments.length >= 2 ? recursiveCompactAvailable : false,
    recursiveSpendAvailable,
  );
}

export function preferredKagemushaOfflineSpendModeForCapabilities(
  recursiveCompactAvailable,
  recursiveSpendAvailable,
) {
  void recursiveCompactAvailable;
  if (recursiveSpendAvailable) {
    return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1;
  }
  return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1;
}

export function canRedeemKagemushaRecursiveSpendWitnessless(proofCircuitId, hopCount) {
  return (
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
    isKagemushaRecursiveSpendLineageProofCircuitId(proofCircuitId) &&
    Number.isInteger(hopCount) &&
    hopCount >= 1 &&
    hopCount <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
  );
}

export function isKagemushaRecursiveSpendLineageProofCircuitId(proofCircuitId) {
  return (
    proofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
    proofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
    proofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
  );
}

export function isKagemushaRecursiveSpendLineageAppendOutputCircuitId(outputProofCircuitId) {
  return (
    outputProofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
    outputProofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
  );
}

export function isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(
  verifierOpeningLen,
) {
  return [2, 4, 8, 16, 32, 64, 128].includes(verifierOpeningLen);
}

export function kagemushaRecursiveSpendLineageKeyArtifactsForInit(
  verifierOpeningLen,
  lineageVerifierKeyBackend,
  lineageVerifierKey,
  lineageProvingKeyArchive,
) {
  return kagemushaRecursiveSpendLineageKeyArtifacts(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    verifierOpeningLen,
    lineageVerifierKeyBackend,
    lineageVerifierKey,
    lineageProvingKeyArchive,
  );
}

export function kagemushaRecursiveSpendLineageKeyArtifactsForAppend(
  verifierOpeningLen,
  lineageVerifierKeyBackend,
  lineageVerifierKey,
  lineageProvingKeyArchive,
) {
  return kagemushaRecursiveSpendLineageKeyArtifacts(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    verifierOpeningLen,
    lineageVerifierKeyBackend,
    lineageVerifierKey,
    lineageProvingKeyArchive,
  );
}

export function kagemushaRecursiveSpendLineageKeyArtifacts(
  proofCircuitId,
  verifierOpeningLen,
  lineageVerifierKeyBackend,
  lineageVerifierKey,
  lineageProvingKeyArchive,
) {
  return validateKagemushaRecursiveSpendLineageKeyArtifacts({
    proofCircuitId,
    verifierOpeningLen,
    lineageVerifierKeyBackend,
    lineageVerifierKey: kagemushaLineageKeyArtifactBytes(
      lineageVerifierKey,
      "lineage_verifier_key",
    ),
    lineageProvingKeyArchive: kagemushaLineageKeyArtifactBytes(
      lineageProvingKeyArchive,
      "lineage_proving_key_archive",
    ),
  });
}

export function validateKagemushaRecursiveSpendLineageKeyArtifacts(artifacts) {
  if (artifacts === undefined || artifacts === null || typeof artifacts !== "object") {
    throw new TypeError("lineage_key_artifacts");
  }
  if (
    artifacts.proofCircuitId !==
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 &&
    artifacts.proofCircuitId !==
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
  ) {
    throw new TypeError("proof_circuit_id");
  }
  if (!isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(
    artifacts.verifierOpeningLen,
  )) {
    throw new TypeError("verifier_opening_len");
  }
  const lineageVerifierKey = kagemushaLineageKeyArtifactBytes(
    artifacts.lineageVerifierKey,
    "lineage_verifier_key",
  );
  const lineageProvingKeyArchive = kagemushaLineageKeyArtifactBytes(
    artifacts.lineageProvingKeyArchive,
    "lineage_proving_key_archive",
  );
  if (
    artifacts.lineageVerifierKeyBackend !== KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND ||
    lineageVerifierKey.length === 0
  ) {
    throw new TypeError("lineage_verifier_key");
  }
  if (lineageProvingKeyArchive.length === 0) {
    throw new TypeError("lineage_proving_key_archive");
  }
  validateKagemushaRecursiveSpendLineageKeyArtifactPackageBinding(
    artifacts.proofCircuitId,
    artifacts.lineageVerifierKeyBackend,
    lineageVerifierKey,
    lineageProvingKeyArchive,
  );
  const storedLineageVerifierKey = Buffer.from(lineageVerifierKey);
  const storedLineageProvingKeyArchive = Buffer.from(lineageProvingKeyArchive);
  return Object.freeze({
    proofCircuitId: artifacts.proofCircuitId,
    verifierOpeningLen: artifacts.verifierOpeningLen,
    lineageVerifierKeyBackend: artifacts.lineageVerifierKeyBackend,
    get lineageVerifierKey() {
      return Buffer.from(storedLineageVerifierKey);
    },
    get lineageProvingKeyArchive() {
      return Buffer.from(storedLineageProvingKeyArchive);
    },
    isInitArtifact:
      artifacts.proofCircuitId ===
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    isAppendArtifact:
      artifacts.proofCircuitId ===
      KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
  });
}

function kagemushaLineageKeyArtifactBytes(value, name) {
  if (value === undefined || value === null) {
    return Buffer.alloc(0);
  }
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(new Uint8Array(value));
  }
  throw new TypeError(name);
}

function validateKagemushaRecursiveSpendLineageKeyArtifactPackageBinding(
  proofCircuitId,
  lineageVerifierKeyBackend,
  lineageVerifierKey,
  lineageProvingKeyArchive,
) {
  const verifierCircuitId = kagemushaLineageVerifierKeyEnvelopeCircuitId(
    lineageVerifierKey,
  );
  if (verifierCircuitId !== proofCircuitId) {
    throw new TypeError("lineage_verifier_key");
  }
  const archivePayload = kagemushaLineageProvingKeyArchivePayload(
    lineageProvingKeyArchive,
  );
  const circuitIdBytes = Buffer.from(proofCircuitId, "utf8");
  const verifierKeyCommitment = kagemushaVerifyingKeyCommitment(
    lineageVerifierKeyBackend,
    lineageVerifierKey,
  );
  if (
    !archivePayload.includes(circuitIdBytes) ||
    !archivePayload.includes(verifierKeyCommitment)
  ) {
    throw new TypeError("lineage_proving_key_archive");
  }
  const archive = kagemushaLineageProvingKeyArchive(lineageProvingKeyArchive);
  if (
    archive.version !== KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 ||
    archive.circuitFamily !== proofCircuitId ||
    !archive.vkCommitment.equals(verifierKeyCommitment) ||
    archive.provingKey.length === 0
  ) {
    throw new TypeError("lineage_proving_key_archive");
  }
}

function kagemushaLineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey) {
  if (
    lineageVerifierKey.length < KAGEMUSHA_ZK1_MAGIC.length ||
    !lineageVerifierKey.subarray(0, KAGEMUSHA_ZK1_MAGIC.length).equals(KAGEMUSHA_ZK1_MAGIC)
  ) {
    throw new TypeError("lineage_verifier_key");
  }
  let offset = KAGEMUSHA_ZK1_MAGIC.length;
  let circuitId = null;
  let sawIpaK = false;
  let sawH2Vk = false;
  while (offset < lineageVerifierKey.length) {
    if (offset + 8 > lineageVerifierKey.length) {
      throw new TypeError("lineage_verifier_key");
    }
    const tag = lineageVerifierKey.subarray(offset, offset + 4);
    const payloadLength = lineageVerifierKey.readUInt32LE(offset + 4);
    const payloadStart = offset + 8;
    const payloadEnd = payloadStart + payloadLength;
    if (payloadEnd > lineageVerifierKey.length) {
      throw new TypeError("lineage_verifier_key");
    }
    const payload = lineageVerifierKey.subarray(payloadStart, payloadEnd);
    if (tag.equals(KAGEMUSHA_ZK1_TLV_CID1)) {
      if (circuitId !== null || payload.some((byte) => byte < 0x20 || byte > 0x7e)) {
        throw new TypeError("lineage_verifier_key");
      }
      circuitId = payload.toString("utf8");
      if (circuitId.length === 0) {
        throw new TypeError("lineage_verifier_key");
      }
    } else if (tag.equals(KAGEMUSHA_ZK1_TLV_IPAK)) {
      if (sawIpaK || payload.length !== 4) {
        throw new TypeError("lineage_verifier_key");
      }
      sawIpaK = true;
    } else if (tag.equals(KAGEMUSHA_ZK1_TLV_H2VK)) {
      if (sawH2Vk || payload.length === 0) {
        throw new TypeError("lineage_verifier_key");
      }
      sawH2Vk = true;
    } else {
      throw new TypeError("lineage_verifier_key");
    }
    offset = payloadEnd;
  }
  if (circuitId === null || !sawIpaK || !sawH2Vk) {
    throw new TypeError("lineage_verifier_key");
  }
  return circuitId;
}

function kagemushaLineageProvingKeyArchive(lineageProvingKeyArchive) {
  try {
    const archivePayload = kagemushaLineageProvingKeyArchivePayload(
      lineageProvingKeyArchive,
    );
    const flags = lineageProvingKeyArchive[39];
    return kagemushaDecodeLineageProvingKeyArchivePayload(archivePayload, flags);
  } catch {
    throw new TypeError("lineage_proving_key_archive");
  }
}

function kagemushaLineageProvingKeyArchivePayload(lineageProvingKeyArchive) {
  try {
    const archivePayload = assertKagemushaNoritoArchive(
      lineageProvingKeyArchive,
      "lineage_proving_key_archive",
    );
    const schemaHash = lineageProvingKeyArchive.subarray(6, 22);
    const flags = lineageProvingKeyArchive[39];
    if (
      !schemaHash.equals(KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH) ||
      (flags & KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG) !== 0 ||
      (flags & PRIVACY_NORITO_FIELD_BITSET_FLAG) !== 0
    ) {
      throw new Error("lineage proving-key archive schema");
    }
    return Buffer.from(archivePayload);
  } catch {
    throw new TypeError("lineage_proving_key_archive");
  }
}

function kagemushaDecodeLineageProvingKeyArchivePayload(payload, flags) {
  let offset = 0;
  let field = kagemushaReadNoritoField(payload, offset, flags, "version");
  const versionPayload = field.payload;
  offset = field.offset;
  if (versionPayload.length !== 2) {
    throw new Error("lineage archive version");
  }
  const version = versionPayload.readUInt16LE(0);

  field = kagemushaReadNoritoField(payload, offset, flags, "circuit_family");
  const circuitFamily = kagemushaDecodeNoritoString(
    field.payload,
    flags,
    "circuit_family",
  );
  offset = field.offset;

  field = kagemushaReadNoritoField(payload, offset, flags, "vk_commitment");
  const vkCommitment = field.payload;
  offset = field.offset;
  if (vkCommitment.length !== 32) {
    throw new Error("lineage archive vk_commitment");
  }

  field = kagemushaReadNoritoField(payload, offset, flags, "proving_key");
  const provingKey = kagemushaDecodeNoritoByteVec(field.payload, "proving_key");
  offset = field.offset;
  if (offset !== payload.length) {
    throw new Error("lineage archive trailing bytes");
  }

  return {
    version,
    circuitFamily,
    vkCommitment: Buffer.from(vkCommitment),
    provingKey,
  };
}

function kagemushaReadNoritoField(buffer, offset, flags, name) {
  const length = kagemushaReadNoritoLength(buffer, offset, flags, `${name}.length`);
  const payloadStart = length.offset;
  const payloadEnd = payloadStart + length.value;
  if (payloadEnd > buffer.length) {
    throw new Error(`${name} payload is truncated`);
  }
  return {
    payload: buffer.subarray(payloadStart, payloadEnd),
    offset: payloadEnd,
  };
}

function kagemushaReadNoritoLength(buffer, offset, flags, name) {
  if ((flags & KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) === 0) {
    if (offset + 8 > buffer.length) {
      throw new Error(`${name} is truncated`);
    }
    const value = buffer.readBigUInt64LE(offset);
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error(`${name} exceeds safe length`);
    }
    return { value: Number(value), offset: offset + 8 };
  }
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (let index = 0; index < 10; index += 1) {
    if (cursor >= buffer.length) {
      throw new Error(`${name} varint is truncated`);
    }
    const byte = BigInt(buffer[cursor]);
    cursor += 1;
    const chunk = byte & 0x7fn;
    if (shift >= 63n && chunk > 1n) {
      throw new Error(`${name} varint exceeds u64 length space`);
    }
    value |= chunk << shift;
    if ((byte & 0x80n) === 0n) {
      const encodedLength = cursor - offset;
      if (
        encodedLength > 1 &&
        value < (1n << (7n * BigInt(encodedLength - 1)))
      ) {
        throw new Error(`${name} varint is non-canonical`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error(`${name} exceeds safe length`);
      }
      return { value: Number(value), offset: cursor };
    }
    shift += 7n;
  }
  throw new Error(`${name} varint is too long`);
}

function kagemushaDecodeNoritoString(payload, flags, name) {
  const length = kagemushaReadNoritoLength(payload, 0, flags, `${name}.value.length`);
  const start = length.offset;
  const end = start + length.value;
  if (end !== payload.length) {
    throw new Error(`${name} payload length mismatch`);
  }
  const bytes = payload.subarray(start, end);
  const value = bytes.toString("utf8");
  if (!Buffer.from(value, "utf8").equals(bytes)) {
    throw new Error(`${name} must be valid utf8`);
  }
  return value;
}

function kagemushaDecodeNoritoByteVec(payload, name) {
  if (payload.length < 8) {
    throw new Error(`${name} sequence length is truncated`);
  }
  const length = payload.readBigUInt64LE(0);
  if (length > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${name} exceeds safe length`);
  }
  const end = 8 + Number(length);
  if (end !== payload.length) {
    throw new Error(`${name} payload length mismatch`);
  }
  return Buffer.from(payload.subarray(8));
}

function kagemushaVerifyingKeyCommitment(lineageVerifierKeyBackend, lineageVerifierKey) {
  const backend = Buffer.from(lineageVerifierKeyBackend, "utf8");
  const backendLength = Buffer.alloc(8);
  backendLength.writeBigUInt64BE(BigInt(backend.length));
  const verifierKeyLength = Buffer.alloc(8);
  verifierKeyLength.writeBigUInt64BE(BigInt(lineageVerifierKey.length));
  return createHash("sha256")
    .update("iroha:zk:v1:vk")
    .update(backendLength)
    .update(backend)
    .update(verifierKeyLength)
    .update(lineageVerifierKey)
    .digest();
}

export function requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit() {
  return true;
}

export function requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
  proofCircuitId,
  hopCount,
) {
  return !canRedeemKagemushaRecursiveSpendWitnessless(proofCircuitId, hopCount);
}

export function canAppendKagemushaRecursiveSpendWitnesslessLineage(previousHopCount) {
  return (
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
    Number.isInteger(previousHopCount) &&
    previousHopCount >= 1 &&
    previousHopCount < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
  );
}

export function normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId) {
  if (outputProofCircuitId === undefined || outputProofCircuitId === null || outputProofCircuitId === "") {
    return KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1;
  }
  if (outputProofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1) {
    return KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1;
  }
  return outputProofCircuitId;
}

export function isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId) {
  const normalized = normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId);
  return (
    normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
    normalized === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
  );
}

export function requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
  outputProofCircuitId,
) {
  return isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId),
  );
}

export function preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(previousHopCount) {
  return canAppendKagemushaRecursiveSpendWitnesslessLineage(previousHopCount)
    ? KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    : KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1;
}

export function canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
  outputProofCircuitId,
  previousHopCount,
) {
  if (!Number.isInteger(previousHopCount) || previousHopCount < 1) {
    return false;
  }
  const normalized = normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId);
  if (normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1) {
    return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;
  }
  if (normalized === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1) {
    return canAppendKagemushaRecursiveSpendWitnesslessLineage(previousHopCount);
  }
  return false;
}

export function isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(previousProofCircuitId) {
  return (
    previousProofCircuitId === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
    isKagemushaRecursiveSpendLineageProofCircuitId(previousProofCircuitId)
  );
}

export function requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
  previousProofCircuitId,
) {
  return isKagemushaRecursiveSpendLineageProofCircuitId(previousProofCircuitId);
}

export function isSupportedKagemushaRecursiveSpendAppendProofTransition(
  previousProofCircuitId,
  outputProofCircuitId,
) {
  const normalizedOutput =
    normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId);
  return (
    (previousProofCircuitId === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 &&
      normalizedOutput === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1) ||
    (isKagemushaRecursiveSpendLineageProofCircuitId(previousProofCircuitId) &&
      (normalizedOutput === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
        normalizedOutput === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1))
  );
}

export function canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
  previousProofCircuitId,
  outputProofCircuitId,
  previousHopCount,
) {
  if (!canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId, previousHopCount)) {
    return false;
  }
  if (!isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(previousProofCircuitId)) {
    return false;
  }
  return isSupportedKagemushaRecursiveSpendAppendProofTransition(
    previousProofCircuitId,
    outputProofCircuitId,
  );
}

export function requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
  outputProofCircuitId,
  previousHopCount,
) {
  return (
    isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
      normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId),
    ) &&
    Number.isInteger(previousHopCount) &&
    previousHopCount >= 1
  );
}

function ensureKagemushaRecursiveSpendNative(native, operation) {
  if (!hasKagemushaRecursiveSpendNative(native)) {
    throw new Error(
      `Kagemusha recursive spend helper '${operation}' is unavailable; build iroha_js_host with recursive Kagemusha support`,
    );
  }
  return native;
}

function callKagemushaRecursiveSpendNative(operation, requestArchive, archiveName = "requestArchive") {
  const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName);
  const native = ensureKagemushaRecursiveSpendNative(resolveNativeBinding(), operation);
  const result = native[operation](request);
  return kagemushaRecursiveSpendOutputToBuffer(result, operation);
}

function kagemushaRecursiveSpendOutputToBuffer(result, operation) {
  if (result === undefined || result === null) {
    throw new Error(`native ${operation} returned no output`);
  }
  if (typeof result === "string") {
    throw new Error(`native ${operation} returned text instead of Norito bytes`);
  }
  const outputView = toBuffer(result, `native ${operation} output`);
  if (outputView.length === 0) {
    throw new Error(`native ${operation} returned empty output`);
  }
  if (outputView.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES) {
    throw new Error(`native ${operation} returned oversized output`);
  }
  const output = Buffer.from(outputView);
  assertKagemushaNoritoArchive(
    output,
    operation,
    `native ${operation} returned invalid Norito archive`,
    `native ${operation} returned empty Norito payload`,
  );
  return output;
}

function assertKagemushaNoritoArchive(
  output,
  archiveName,
  invalidMessage = `${archiveName} must be a valid Norito archive`,
  emptyPayloadMessage = `${archiveName} must contain a non-empty Norito payload`,
) {
  const fail = () => {
    throw new Error(invalidMessage);
  };
  if (output.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES) {
    throw new Error(
      `${archiveName} must not exceed ${KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes`,
    );
  }
  if (output.length < PRIVACY_NORITO_HEADER_BYTES) {
    fail();
  }
  if (!output.subarray(0, 4).equals(PRIVACY_NORITO_MAGIC)) {
    fail();
  }
  if (output[4] !== 0 || output[5] !== 0 || output[22] !== 0) {
    fail();
  }
  const flags = output[39];
  if (
    (flags & ~PRIVACY_NORITO_SUPPORTED_FLAGS_MASK) !== 0 ||
    ((flags & PRIVACY_NORITO_FIELD_BITSET_FLAG) !== 0 &&
      (flags & PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS) !==
        PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS)
  ) {
    fail();
  }
  const payloadLengthBig = output.readBigUInt64LE(23);
  if (payloadLengthBig > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail();
  }
  const payloadLength = Number(payloadLengthBig);
  if (payloadLength === 0) {
    throw new Error(emptyPayloadMessage);
  }
  const minimumLength = PRIVACY_NORITO_HEADER_BYTES + payloadLength;
  if (output.length < minimumLength) {
    fail();
  }
  const paddingLength = output.length - minimumLength;
  if (paddingLength > PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES) {
    fail();
  }
  const padding = output.subarray(
    PRIVACY_NORITO_HEADER_BYTES,
    PRIVACY_NORITO_HEADER_BYTES + paddingLength,
  );
  if (padding.some((byte) => byte !== 0)) {
    fail();
  }
  const payload = output.subarray(PRIVACY_NORITO_HEADER_BYTES + paddingLength);
  if (privacyCrc64(payload) !== output.readBigUInt64LE(31)) {
    fail();
  }
  return payload;
}

export function isKagemushaRecursiveSpendNativeAvailable() {
  try {
    return hasKagemushaRecursiveSpendNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function isKagemushaRecursiveCompactPaymentTokenNativeAvailable() {
  try {
    return hasKagemushaRecursiveCompactPaymentTokenNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable() {
  try {
    return hasKagemushaRecursiveCompactPaymentTokenVerifierNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable() {
  try {
    return hasKagemushaRecursiveSpendCompactPaymentTokenProjectionNative(
      resolveNativeBinding(),
    );
  } catch {
    return false;
  }
}

export function isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable() {
  try {
    return hasKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNative(
      resolveNativeBinding(),
    );
  } catch {
    return false;
  }
}

export function isKagemushaCompactPaymentTokenNativeAvailable() {
  try {
    return hasKagemushaCompactPaymentTokenNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function isKagemushaRecursiveAggregationProofBundleNativeAvailable() {
  try {
    return hasKagemushaRecursiveAggregationProofBundleNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function kagemushaRecursiveSpendInit(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendInit",
    requestArchive,
  );
}

export function kagemushaRecursiveSpendAppend(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendAppend",
    requestArchive,
  );
}

export function kagemushaRecursiveSpendTransitionProfileInit(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendTransitionProfileInit",
    requestArchive,
  );
}

export function kagemushaRecursiveSpendTransitionProfileAppend(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendTransitionProfileAppend",
    requestArchive,
  );
}

export function kagemushaRecursiveSpendLineageAppendBoundary(profileArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendLineageAppendBoundary",
    profileArchive,
    "profileArchive",
  );
}

export function kagemushaRecursiveSpendLineageWitnessFromInitResult(
  requestArchive,
  bundleArchive,
) {
  const request = toOwnedKagemushaArchiveBuffer(requestArchive, "requestArchive");
  const bundle = toOwnedKagemushaArchiveBuffer(bundleArchive, "bundleArchive");
  const native = ensureKagemushaRecursiveSpendNative(
    resolveNativeBinding(),
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
  );
  const result = native.kagemushaRecursiveSpendLineageWitnessFromInitResult(
    request,
    bundle,
  );
  if (result === undefined || result === null) {
    throw new Error(
      "native kagemushaRecursiveSpendLineageWitnessFromInitResult returned no output",
    );
  }
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
  );
}

export function kagemushaRecursiveSpendLineageWitnessAppendResult(
  previousWitnessArchive,
  requestArchive,
  bundleArchive,
) {
  const previousWitness = toOwnedKagemushaArchiveBuffer(
    previousWitnessArchive,
    "previousWitnessArchive",
  );
  const request = toOwnedKagemushaArchiveBuffer(requestArchive, "requestArchive");
  const bundle = toOwnedKagemushaArchiveBuffer(bundleArchive, "bundleArchive");
  const native = ensureKagemushaRecursiveSpendNative(
    resolveNativeBinding(),
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
  );
  const result = native.kagemushaRecursiveSpendLineageWitnessAppendResult(
    previousWitness,
    request,
    bundle,
  );
  if (result === undefined || result === null) {
    throw new Error(
      "native kagemushaRecursiveSpendLineageWitnessAppendResult returned no output",
    );
  }
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
  );
}

export function kagemushaRecursiveSpendVerify(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendVerify",
    requestArchive,
  );
}

export function kagemushaRecursiveSpendRedeem(requestArchive) {
  return callKagemushaRecursiveSpendNative(
    "kagemushaRecursiveSpendRedeem",
    requestArchive,
  );
}

export class KagemushaRecursiveSpendRequestCodecError extends Error {
  constructor(kind, field, message) {
    super(message ?? `invalid Kagemusha recursive spend ${kind}: ${field}`);
    this.name = "KagemushaRecursiveSpendRequestCodecError";
    this.kind = kind;
    this.field = field;
  }
}

export function buildKagemushaRecursiveSpendableNoteDescriptor(input) {
  return kagemushaNormalizeSpendableNote(input);
}

export function buildKagemushaRecursiveSpendVerifierRecordRef(input) {
  return kagemushaNormalizeVerifierRecordRef(input, "verifierRecord");
}

export function encodeKagemushaRecursiveSpendInitRequest(request) {
  const normalized = kagemushaNormalizeInitRequest(request);
  const payload = Buffer.concat([
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.recordBundle,
        KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
        "recordBundle",
      ),
    ),
    kagemushaField(kagemushaBytesVec(normalized.pallasOpenEnvelopes)),
    kagemushaField(kagemushaSpendableNotePayload(normalized.currentNote)),
    kagemushaField(
      kagemushaOptionRaw(
        kagemushaVerifyingKeyBoxPayload(normalized.lineageVerifierKey),
      ),
    ),
    kagemushaField(kagemushaOptionBytesVec(normalized.lineageProvingKeyArchive)),
    kagemushaField(kagemushaOptionU64(normalized.blockHeight)),
  ]);
  return kagemushaNoritoArchiveForType(
    KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
    payload,
  );
}

export function encodeKagemushaRecursiveSpendAppendRequest(request) {
  const normalized = kagemushaNormalizeAppendRequest(request);
  const normalizedOutput = normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
    normalized.outputProofCircuitId,
  );
  const outputWire =
    normalizedOutput === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
      ? ""
      : normalizedOutput;
  const previousRecordPayload =
    normalized.previousLineageVerifierRecord === null
      ? null
      : kagemushaCompactPayloadForRequest(
          normalized.previousLineageVerifierRecord.recordBytes,
          KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
          "previousLineageVerifierRecord",
        );
  const lineageVerifierKeyPayload =
    normalized.lineageVerifierKey === null
      ? null
      : kagemushaVerifyingKeyBoxPayload(normalized.lineageVerifierKey);
  const payload = Buffer.concat([
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.previousBundle,
        KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
        "previousBundle",
      ),
    ),
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.recordBundle,
        KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
        "recordBundle",
      ),
    ),
    kagemushaField(kagemushaBytesVec(normalized.pallasOpenEnvelopes)),
    kagemushaField(kagemushaSpendableNotePayload(normalized.currentNote)),
    kagemushaField(kagemushaString(outputWire)),
    kagemushaField(kagemushaOptionRaw(previousRecordPayload)),
    kagemushaField(
      kagemushaBytesVec(normalized.previousProofOpenEnvelopes ?? Buffer.alloc(0)),
    ),
    kagemushaField(kagemushaOptionRaw(lineageVerifierKeyPayload)),
    kagemushaField(kagemushaOptionBytesVec(normalized.lineageProvingKeyArchive)),
    kagemushaField(kagemushaOptionU64(normalized.blockHeight)),
  ]);
  return kagemushaNoritoArchiveForType(
    KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
    payload,
  );
}

export function encodeKagemushaRecursiveSpendVerifyRequest(request) {
  const normalized = kagemushaNormalizeVerifyRequest(request);
  const lineageRecordPayload =
    normalized.lineageVerifierRecord === null
      ? null
      : kagemushaCompactPayloadForRequest(
          normalized.lineageVerifierRecord.recordBytes,
          KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
          "lineageVerifierRecord",
        );
  const payload = Buffer.concat([
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.bundle,
        KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
        "bundle",
      ),
    ),
    kagemushaField(kagemushaOptionRaw(lineageRecordPayload)),
    kagemushaField(kagemushaOptionU64(normalized.blockHeight)),
  ]);
  return kagemushaNoritoArchiveForType(
    KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
    payload,
  );
}

export function encodeKagemushaRecursiveSpendRedeemRequest(request) {
  const normalized = kagemushaNormalizeRedeemRequest(request);
  const lineageWitnessPayload =
    normalized.lineageWitness === null
      ? null
      : kagemushaCompactPayloadForRequest(
          normalized.lineageWitness,
          KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
          "lineageWitness",
        );
  const lineageRecordPayload =
    normalized.lineageVerifierRecord === null
      ? null
      : kagemushaCompactPayloadForRequest(
          normalized.lineageVerifierRecord.recordBytes,
          KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
          "lineageVerifierRecord",
        );
  const payload = Buffer.concat([
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.bundle,
        KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
        "bundle",
      ),
    ),
    kagemushaField(kagemushaAccountIdPayload(normalized.recipient)),
    kagemushaField(kagemushaU128(normalized.publicAmount, "publicAmount")),
    kagemushaRawField(
      kagemushaCompactPayloadForRequest(
        normalized.redeemProof,
        KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
        "redeemProof",
      ),
    ),
    kagemushaField(kagemushaOptionRaw(lineageWitnessPayload)),
    kagemushaField(kagemushaOptionRaw(lineageRecordPayload)),
    kagemushaField(kagemushaOptionU64(normalized.blockHeight)),
  ]);
  return kagemushaNoritoArchiveForType(
    KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_WIRE_NAME,
    payload,
  );
}

export function decodeKagemushaRecursiveSpendVerifyResult(archive) {
  const { payload, flags } = kagemushaTypedArchivePayload(
    archive,
    KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
    "verifyResult",
  );
  if (flags !== KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) {
    throw kagemushaArchiveCodecError("verifyResult", "verifyResult must use compact Norito layout");
  }
  let offset = 0;
  let result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.valid",
    kagemushaReadBoolPayload,
  );
  const valid = result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.hopCount",
    kagemushaReadU32Payload,
  );
  const hopCount = result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.encodedBytes",
    kagemushaReadU32Payload,
  );
  const encodedBytes = result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.reason",
    (fieldPayload) => kagemushaReadStringPayload(fieldPayload, flags, "verifyResult.reason"),
  );
  const reason = result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.chainAdmissible",
    kagemushaReadBoolPayload,
  );
  const chainAdmissible = result;
  [result, offset] = kagemushaReadFieldValue(
    payload,
    offset,
    flags,
    "verifyResult.chainAdmissionReason",
    (fieldPayload) =>
      kagemushaReadStringPayload(fieldPayload, flags, "verifyResult.chainAdmissionReason"),
  );
  const chainAdmissionReason = result;
  let witnesslessRedeemSupported = false;
  let lineageWitnessRequired = false;
  if (offset < payload.length) {
    [witnesslessRedeemSupported, offset] = kagemushaReadFieldValue(
      payload,
      offset,
      flags,
      "verifyResult.witnesslessRedeemSupported",
      kagemushaReadBoolPayload,
    );
  }
  if (offset < payload.length) {
    [lineageWitnessRequired, offset] = kagemushaReadFieldValue(
      payload,
      offset,
      flags,
      "verifyResult.lineageWitnessRequired",
      kagemushaReadBoolPayload,
    );
  }
  if (offset !== payload.length) {
    throw kagemushaArchiveCodecError("verifyResult", "verifyResult has trailing bytes");
  }
  return Object.freeze({
    valid,
    hopCount,
    hop_count: hopCount,
    encodedBytes,
    encoded_bytes: encodedBytes,
    reason,
    chainAdmissible,
    chain_admissible: chainAdmissible,
    chainAdmissionReason,
    chain_admission_reason: chainAdmissionReason,
    witnesslessRedeemSupported,
    witnessless_redeem_supported: witnesslessRedeemSupported,
    lineageWitnessRequired,
    lineage_witness_required: lineageWitnessRequired,
  });
}

export function decodeKagemushaRecursiveSpendBundle(archive) {
  const { payload, flags } = kagemushaTypedArchivePayload(
    archive,
    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    "bundle",
  );
  if (flags !== KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) {
    throw kagemushaArchiveCodecError("bundle", "bundle must use compact Norito layout");
  }
  let offset = 0;
  let field = kagemushaReadNoritoField(payload, offset, flags, "bundle.accumulator");
  const accumulatorPayload = field.payload;
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "bundle.proof");
  const proofPayload = field.payload;
  offset = field.offset;
  if (offset !== payload.length) {
    throw kagemushaArchiveCodecError("bundle", "bundle has trailing bytes");
  }
  const accumulator = kagemushaReadAccumulatorSummary(accumulatorPayload, flags);
  const proofCircuitId = kagemushaReadRecursiveProofCircuitId(proofPayload, flags);
  const initialRoot = Buffer.from(accumulator.initialRoot);
  const finalRoot = Buffer.from(accumulator.finalRoot);
  return Object.freeze({
    hopCount: accumulator.hopCount,
    hop_count: accumulator.hopCount,
    proofCircuitId,
    proof_circuit_id: proofCircuitId,
    asset: accumulator.asset,
    chainId: accumulator.chainId,
    chain_id: accumulator.chainId,
    get initialRoot() {
      return Buffer.from(initialRoot);
    },
    get initial_root() {
      return Buffer.from(initialRoot);
    },
    get finalRoot() {
      return Buffer.from(finalRoot);
    },
    get final_root() {
      return Buffer.from(finalRoot);
    },
    currentNote: accumulator.currentNote,
    current_note: accumulator.currentNote,
  });
}

export function kagemushaRecursiveSpendInitTyped(request) {
  return kagemushaRecursiveSpendInit(
    encodeKagemushaRecursiveSpendInitRequest(request),
  );
}

export function kagemushaRecursiveSpendAppendTyped(request) {
  return kagemushaRecursiveSpendAppend(
    encodeKagemushaRecursiveSpendAppendRequest(request),
  );
}

export function kagemushaRecursiveSpendVerifyTyped(request) {
  return decodeKagemushaRecursiveSpendVerifyResult(
    kagemushaRecursiveSpendVerify(
      encodeKagemushaRecursiveSpendVerifyRequest(request),
    ),
  );
}

export function kagemushaRecursiveSpendRedeemTyped(request) {
  return kagemushaRecursiveSpendRedeem(
    encodeKagemushaRecursiveSpendRedeemRequest(request),
  );
}

export function kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive) {
  const recordBundle = toOwnedKagemushaArchiveBuffer(
    recordBundleArchive,
    "recordBundleArchive",
  );
  const native = resolveNativeBinding();
  if (!hasKagemushaCompactPaymentTokenNative(native)) {
    throw new Error(
      "Kagemusha compact payment-token prover requires native bridge ABI 6 with compact-token prover symbol",
    );
  }
  const result = native.kagemushaProveVerifiedCompactPaymentTokenWithRecords(recordBundle);
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
  );
}

export function kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
  recordBundleArchive,
  pallasOpenEnvelopesArchive,
) {
  const recordBundle = toOwnedKagemushaArchiveBuffer(
    recordBundleArchive,
    "recordBundleArchive",
  );
  const pallasOpenEnvelopes = toOwnedKagemushaArchiveBuffer(
    pallasOpenEnvelopesArchive,
    "pallasOpenEnvelopesArchive",
  );
  const native = resolveNativeBinding();
  if (!hasKagemushaRecursiveAggregationProofBundleNative(native)) {
    throw new Error(
      "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6 with recursive aggregation prover symbol",
    );
  }
  const result =
    native.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
      recordBundle,
      pallasOpenEnvelopes,
    );
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
  );
}

export function kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
  recordBundleArchive,
  pallasOpenEnvelopesArchive,
  recursiveCompactKeyArtifactsArchive,
) {
  const recordBundle = toOwnedKagemushaArchiveBuffer(
    recordBundleArchive,
    "recordBundleArchive",
  );
  const pallasOpenEnvelopes = toOwnedKagemushaArchiveBuffer(
    pallasOpenEnvelopesArchive,
    "pallasOpenEnvelopesArchive",
  );
  const recursiveCompactKeyArtifacts = toOwnedKagemushaArchiveBuffer(
    recursiveCompactKeyArtifactsArchive,
    "recursiveCompactKeyArtifactsArchive",
  );
  const native = resolveNativeBinding();
  if (!hasKagemushaRecursiveCompactPaymentTokenNative(native)) {
    throw new Error(
      "recursive compact Kagemusha payment-token prover requires native bridge ABI 7 with compact prover and verifier symbols",
    );
  }
  const result =
    native.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
      recordBundle,
      pallasOpenEnvelopes,
      recursiveCompactKeyArtifacts,
    );
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
  );
}

export function kagemushaVerifyRecursiveCompactPaymentToken(
  compactTokenArchive,
  recursiveCompactVerifierKeysArchive,
) {
  const compactToken = toOwnedKagemushaArchiveBuffer(
    compactTokenArchive,
    "compactTokenArchive",
  );
  const recursiveCompactVerifierKeys = toOwnedKagemushaArchiveBuffer(
    recursiveCompactVerifierKeysArchive,
    "recursiveCompactVerifierKeysArchive",
  );
  const native = resolveNativeBinding();
  if (!hasKagemushaRecursiveCompactPaymentTokenVerifierNative(native)) {
    throw new Error(
      "recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol",
    );
  }
  const result = native.kagemushaVerifyRecursiveCompactPaymentToken(
    compactToken,
    recursiveCompactVerifierKeys,
  );
  if (typeof result !== "boolean") {
    throw new Error(
      "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
    );
  }
  return result;
}

export function kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundleArchive) {
  const bundle = toOwnedKagemushaArchiveBuffer(bundleArchive, "bundleArchive");
  const native = resolveNativeBinding();
  if (!hasKagemushaRecursiveSpendCompactPaymentTokenProjectionNative(native)) {
    throw new Error(
      "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7 with the compact projection symbol",
    );
  }
  const result = native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(bundle);
  return kagemushaRecursiveSpendOutputToBuffer(
    result,
    "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
  );
}

export function kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
  compactTokenArchive,
  verifierRecordArchive,
  blockHeight = undefined,
) {
  const compactToken = toOwnedKagemushaArchiveBuffer(
    compactTokenArchive,
    "compactTokenArchive",
  );
  const verifierRecord = toOwnedKagemushaArchiveBuffer(
    verifierRecordArchive,
    "verifierRecordArchive",
  );
  const checkedBlockHeight = normalizeKagemushaBlockHeight(blockHeight);
  const native = resolveNativeBinding();
  if (!hasKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNative(native)) {
    throw new Error(
      "recursive spend compact Kagemusha payment-token projection verifier requires native bridge ABI 7 with the compact projection verifier symbols",
    );
  }
  const result =
    checkedBlockHeight === null
      ? native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(
          compactToken,
          verifierRecord,
        )
      : native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
          compactToken,
          verifierRecord,
          checkedBlockHeight,
        );
  if (typeof result !== "boolean") {
    throw new Error(
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result",
    );
  }
  return result;
}

function normalizeKagemushaBlockHeight(blockHeight) {
  if (blockHeight === undefined || blockHeight === null) {
    return null;
  }
  if (typeof blockHeight === "number") {
    if (!Number.isFinite(blockHeight) || !Number.isInteger(blockHeight)) {
      throw new TypeError("blockHeight must be an integer");
    }
    if (blockHeight < 0 || Object.is(blockHeight, -0)) {
      throw new RangeError("blockHeight must be non-negative");
    }
    if (!Number.isSafeInteger(blockHeight)) {
      throw new RangeError(
        "blockHeight number must be a safe integer; use bigint for larger u64 values",
      );
    }
    return blockHeight;
  }
  if (typeof blockHeight === "bigint") {
    if (blockHeight < 0n) {
      throw new RangeError("blockHeight must be non-negative");
    }
    if (blockHeight > U64_MAX) {
      throw new RangeError("blockHeight must fit in u64");
    }
    return blockHeight;
  }
  throw new TypeError("blockHeight must be a number or bigint");
}

function hasPrivacyNativeSurface(native) {
  const abiVersion = privacyBridgeAbiVersion(native);
  return (
    native &&
    Number.isInteger(abiVersion) &&
    abiVersion >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION &&
    typeof native.privacyCapabilitiesV1 === "function" &&
    typeof native.privacyProofRequestV1 === "function" &&
    typeof native.privacyBuildProofV1 === "function" &&
    typeof native.privacyVerifyProofV1 === "function"
  );
}

function privacyNativeProbeReturnsBytes(native, operation, requestArchive = undefined) {
  let request;
  try {
    const result =
      requestArchive === undefined
        ? native[operation]()
        : native[operation]((request = Buffer.from(requestArchive)));
    privacyNativeOutputToBuffer(result, operation, { clearSource: true });
    return true;
  } catch {
    return false;
  } finally {
    if (request) {
      request.fill(0);
    }
  }
}

function privacyNativeProofRequestProbeReturnsBytes(native) {
  let publicInputs;
  try {
    publicInputs = Buffer.from("privacy-native-availability-public-input-v1", "utf8");
    const result = native.privacyProofRequestV1(
      ZK_ACE_ALGORITHM_ID,
      ZK_ACE_PRODUCTION_ENTRYPOINT,
      ZK_ACE_PRODUCTION_VK_REF,
      publicInputs,
      Buffer.alloc(0),
      Buffer.alloc(0),
    );
    privacyRequestNativeOutputToBuffer(result, "privacyProofRequestV1", {
      clearSource: true,
    });
    return true;
  } catch {
    return false;
  } finally {
    if (publicInputs) {
      publicInputs.fill(0);
    }
  }
}

function hasPrivacyNative(native) {
  return (
    hasPrivacyNativeSurface(native) &&
    privacyNativeProbeReturnsBytes(native, "privacyCapabilitiesV1") &&
    privacyNativeProofRequestProbeReturnsBytes(native) &&
    privacyNativeProbeReturnsBytes(
      native,
      "privacyBuildProofV1",
      PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE,
    ) &&
    privacyNativeProbeReturnsBytes(
      native,
      "privacyVerifyProofV1",
      PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE,
    )
  );
}

function privacyBridgeAbiVersion(native) {
  if (typeof native?.connectNoritoBridgeAbiVersion !== "function") {
    return 0;
  }
  try {
    const version = native.connectNoritoBridgeAbiVersion();
    return typeof version === "number" &&
      Number.isSafeInteger(version) &&
      version >= 0 &&
      version <= PRIVACY_MAX_BRIDGE_ABI_VERSION
      ? version
      : 0;
  } catch {
    return 0;
  }
}

function ensurePrivacyNative(native, operation) {
  if (!hasPrivacyNativeSurface(native)) {
    throw new Error(
      `${operation} requires the iroha_js_host native binding built with privacy FFI support`,
    );
  }
  return native;
}

function toPrivacyArchiveBuffer(value, name) {
  if (typeof value === "string") {
    throw new TypeError(`${name} must be Norito V1 bytes, not a string`);
  }
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array || value instanceof DataView) {
    if (!(value.buffer instanceof ArrayBuffer)) {
      throw new TypeError(`${name} must not use shared memory`);
    }
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError(
    `${name} must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer`,
  );
}

function toPrivacyRequestArchiveBuffer(value, name) {
  const request = toPrivacyArchiveBuffer(value, name);
  if (request.length === 0) {
    throw new Error(`${name} must not be empty`);
  }
  if (request.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
    throw new Error(`${name} must not exceed ${PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes`);
  }
  assertPrivacyNoritoArchive(request, name, "request", PRIVACY_REQUEST_SCHEMA_BYTE);
  return Buffer.from(request);
}

function toPrivacyRequestComponentBuffer(value, name, { allowEmpty = true } = {}) {
  if (typeof value === "string") {
    throw new TypeError(`${name} must be bytes-like, not a string`);
  }
  let bytes;
  if (Buffer.isBuffer(value)) {
    bytes = value;
  } else if (value instanceof Uint8Array || value instanceof DataView) {
    if (!(value.buffer instanceof ArrayBuffer)) {
      throw new TypeError(`${name} must not use shared memory`);
    }
    bytes = Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  } else if (value instanceof ArrayBuffer) {
    bytes = Buffer.from(value);
  } else {
    throw new TypeError(
      `${name} must be bytes-like as a Buffer, Uint8Array, DataView, or ArrayBuffer`,
    );
  }
  if (!allowEmpty && bytes.length === 0) {
    throw new Error(`${name} must not be empty`);
  }
  if (bytes.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
    throw new Error(`${name} must not exceed ${PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes`);
  }
  return Buffer.from(bytes);
}

function requirePrivacyRequestText(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  return value;
}

function privacyNativeOutputToBuffer(result, operation, options = {}) {
  let output;
  if (result === undefined || result === null) {
    throw new Error(`native ${operation} returned no output`);
  }
  if (typeof result === "string") {
    throw new Error(`native ${operation} returned text instead of Norito V1 bytes`);
  }
  try {
    output = toPrivacyArchiveBuffer(result, `native ${operation} output`);
    if (output.length === 0) {
      throw new Error(`native ${operation} returned empty output`);
    }
    if (output.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
      throw new Error(`native ${operation} returned oversized output`);
    }
    assertPrivacyNoritoArchive(
      output,
      operation,
      "native",
      privacyExpectedResultSchemaByte(operation),
    );
    return Buffer.from(output);
  } finally {
    if (options.clearSource === true && output) {
      output.fill(0);
    }
  }
}

function privacyRequestNativeOutputToBuffer(result, operation, options = {}) {
  let output;
  if (result === undefined || result === null) {
    throw new Error(`native ${operation} returned no output`);
  }
  if (typeof result === "string") {
    throw new Error(`native ${operation} returned text instead of Norito V1 bytes`);
  }
  try {
    output = toPrivacyArchiveBuffer(result, `native ${operation} output`);
    if (output.length === 0) {
      throw new Error(`native ${operation} returned empty output`);
    }
    if (output.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
      throw new Error(`native ${operation} returned oversized output`);
    }
    assertPrivacyNoritoArchive(
      output,
      operation,
      "request",
      PRIVACY_REQUEST_SCHEMA_BYTE,
    );
    return Buffer.from(output);
  } finally {
    if (options.clearSource === true && output) {
      output.fill(0);
    }
  }
}

function privacyCrc64(payload) {
  let crc = PRIVACY_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = PRIVACY_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ PRIVACY_CRC64_MASK);
}

function assertPrivacyNoritoArchive(
  output,
  operation,
  context = "native",
  expectedSchemaByte,
) {
  const fail = () => {
    if (context === "request") {
      throw new Error(`${operation} must be a valid Norito V1 archive`);
    }
    throw new Error(`native ${operation} returned invalid Norito V1 archive`);
  };
  if (
    !Number.isInteger(expectedSchemaByte) ||
    expectedSchemaByte < 0 ||
    expectedSchemaByte > 0xff
  ) {
    if (context === "request") {
      throw new Error(`${operation} must use the privacy request schema`);
    }
    throw new Error(`native ${operation} returned unexpected privacy result schema`);
  }
  if (output.length < PRIVACY_NORITO_HEADER_BYTES) {
    fail();
  }
  if (!output.subarray(0, 4).equals(PRIVACY_NORITO_MAGIC)) {
    fail();
  }
  if (output[4] !== 0 || output[5] !== 0) {
    fail();
  }
  if (output[22] !== 0) {
    fail();
  }
  const flags = output[39];
  if (
    (flags & ~PRIVACY_NORITO_SUPPORTED_FLAGS_MASK) !== 0 ||
    ((flags & PRIVACY_NORITO_FIELD_BITSET_FLAG) !== 0 &&
      (flags & PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS) !==
        PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS)
  ) {
    fail();
  }
  const payloadLengthBig = output.readBigUInt64LE(23);
  if (payloadLengthBig > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail();
  }
  const payloadLength = Number(payloadLengthBig);
  if (payloadLength === 0) {
    if (context === "request") {
      throw new Error(`${operation} must contain a non-empty privacy request payload`);
    }
    throw new Error(`native ${operation} returned empty privacy result payload`);
  }
  const minimumLength = PRIVACY_NORITO_HEADER_BYTES + payloadLength;
  if (output.length < minimumLength) {
    fail();
  }
  const paddingLength = output.length - minimumLength;
  if (paddingLength > PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES) {
    fail();
  }
  const padding = output.subarray(
    PRIVACY_NORITO_HEADER_BYTES,
    PRIVACY_NORITO_HEADER_BYTES + paddingLength,
  );
  if (padding.some((byte) => byte !== 0)) {
    fail();
  }
  const payload = output.subarray(PRIVACY_NORITO_HEADER_BYTES + paddingLength);
  if (privacyCrc64(payload) !== output.readBigUInt64LE(31)) {
    fail();
  }
  if (output.subarray(6, 22).some((byte) => byte !== expectedSchemaByte)) {
    if (context === "request") {
      throw new Error(`${operation} must use the privacy request schema`);
    }
    throw new Error(`native ${operation} returned unexpected privacy result schema`);
  }
}

function privacyExpectedResultSchemaByte(operation) {
  switch (operation) {
    case "privacyCapabilitiesV1":
      return PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE;
    case "privacyBuildProofV1":
      return PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE;
    case "privacyVerifyProofV1":
      return PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE;
    default:
      throw new Error(`native ${operation} is not a supported privacy native operation`);
  }
}

function invokePrivacyNative(native, operation, ...args) {
  try {
    return native[operation](...args);
  } catch {
    throw new Error(`native ${operation} failed`);
  }
}

function callPrivacyNative(operation, requestArchive) {
  const request = toPrivacyRequestArchiveBuffer(requestArchive, "requestArchive");
  try {
    const native = ensurePrivacyNative(resolveNativeBinding(), operation);
    const result = invokePrivacyNative(native, operation, request);
    return privacyNativeOutputToBuffer(result, operation);
  } finally {
    request.fill(0);
  }
}

export function isPrivacyNativeAvailable() {
  try {
    return hasPrivacyNative(resolveNativeBinding());
  } catch {
    return false;
  }
}

export function privacyCapabilitiesV1() {
  const native = ensurePrivacyNative(resolveNativeBinding(), "privacyCapabilitiesV1");
  const result = invokePrivacyNative(native, "privacyCapabilitiesV1");
  return privacyNativeOutputToBuffer(result, "privacyCapabilitiesV1");
}

export function privacyProofRequestV1(input) {
  if (!input || typeof input !== "object") {
    throw new TypeError("privacyProofRequestV1 input must be an object");
  }
  const {
    algorithmId,
    entrypoint,
    vkRef,
    publicInputs,
    witness = Buffer.alloc(0),
    proof = Buffer.alloc(0),
  } = input;
  const algorithmIdText = requirePrivacyRequestText(algorithmId, "algorithmId");
  const entrypointText = requirePrivacyRequestText(entrypoint, "entrypoint");
  const vkRefText = requirePrivacyRequestText(vkRef, "vkRef");
  const publicInputsBuffer = toPrivacyRequestComponentBuffer(publicInputs, "publicInputs", {
    allowEmpty: false,
  });
  const witnessBuffer = toPrivacyRequestComponentBuffer(witness, "witness");
  const proofBuffer = toPrivacyRequestComponentBuffer(proof, "proof");
  try {
    const native = ensurePrivacyNative(resolveNativeBinding(), "privacyProofRequestV1");
    const result = invokePrivacyNative(
      native,
      "privacyProofRequestV1",
      algorithmIdText,
      entrypointText,
      vkRefText,
      publicInputsBuffer,
      witnessBuffer,
      proofBuffer,
    );
    return privacyRequestNativeOutputToBuffer(result, "privacyProofRequestV1");
  } finally {
    publicInputsBuffer.fill(0);
    witnessBuffer.fill(0);
    proofBuffer.fill(0);
  }
}

export function privacyBuildProofV1(requestArchive) {
  return callPrivacyNative("privacyBuildProofV1", requestArchive);
}

export function privacyVerifyProofV1(requestArchive) {
  return callPrivacyNative("privacyVerifyProofV1", requestArchive);
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

function kagemushaNormalizeInitRequest(request) {
  kagemushaRequirePlainObject(request, "request");
  const lineageVerifierKey = kagemushaRequiredOwnedBuffer(
    request,
    ["lineageVerifierKey", "lineage_verifier_key"],
    "lineageVerifierKey",
  );
  if (lineageVerifierKey.length === 0) {
    throw kagemushaFieldCodecError("lineageVerifierKey");
  }
  const lineageProvingKeyArchive = kagemushaRequireNestedArchive(
    kagemushaObjectValue(
      request,
      ["lineageProvingKeyArchive", "lineage_proving_key_archive"],
    ),
    "lineageProvingKeyArchive",
  );
  return Object.freeze({
    recordBundle: kagemushaRequiredOwnedBuffer(
      request,
      ["recordBundle", "record_bundle"],
      "recordBundle",
    ),
    pallasOpenEnvelopes: kagemushaRequireNestedArchive(
      kagemushaObjectValue(
        request,
        ["pallasOpenEnvelopes", "pallas_open_envelopes", "pallasOpenEnvelopesArchive"],
      ),
      "pallasOpenEnvelopes",
    ),
    currentNote: kagemushaNormalizeSpendableNote(
      kagemushaObjectValue(request, ["currentNote", "current_note"]),
    ),
    lineageVerifierKey,
    lineageProvingKeyArchive,
    blockHeight: kagemushaNormalizeBlockHeight(
      kagemushaObjectValue(request, ["blockHeight", "block_height"]),
      "blockHeight",
    ),
  });
}

function kagemushaNormalizeAppendRequest(request) {
  kagemushaRequirePlainObject(request, "request");
  const previousBundle = kagemushaRequiredOwnedBuffer(
    request,
    ["previousBundle", "previous_bundle"],
    "previousBundle",
  );
  const previousSummary = decodeKagemushaRecursiveSpendBundle(previousBundle);
  const outputProofCircuitId = kagemushaObjectValue(request, [
    "outputProofCircuitId",
    "output_proof_circuit_id",
  ]);
  const normalizedOutput = normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
    outputProofCircuitId,
  );
  if (
    !canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
      previousSummary.proofCircuitId,
      normalizedOutput,
      previousSummary.hopCount,
    )
  ) {
    throw kagemushaFieldCodecError("outputProofCircuitId");
  }
  const previousLineageVerifierRecordValue = kagemushaObjectValue(request, [
    "previousLineageVerifierRecord",
    "previous_lineage_verifier_record",
  ]);
  const previousLineageVerifierRecord =
    previousLineageVerifierRecordValue === undefined ||
    previousLineageVerifierRecordValue === null
      ? null
      : kagemushaNormalizeVerifierRecordRef(
          previousLineageVerifierRecordValue,
          "previousLineageVerifierRecord",
        );
  if (
    requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
      previousSummary.proofCircuitId,
    ) &&
    previousLineageVerifierRecord === null
  ) {
    throw kagemushaFieldCodecError("previousLineageVerifierRecord");
  }
  const previousProofOpenEnvelopesValue = kagemushaObjectValue(request, [
    "previousProofOpenEnvelopes",
    "previous_proof_open_envelopes",
    "previousRecursiveProofOpenEnvelopesArchive",
  ]);
  const previousProofOpenEnvelopes =
    previousProofOpenEnvelopesValue === undefined ||
    previousProofOpenEnvelopesValue === null
      ? null
      : kagemushaRequireNestedArchive(
          previousProofOpenEnvelopesValue,
          "previousProofOpenEnvelopes",
        );
  if (
    previousProofOpenEnvelopes !== null &&
    previousProofOpenEnvelopes.length >
      KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
  ) {
    throw kagemushaFieldCodecError("previousProofOpenEnvelopes");
  }
  if (
    requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
      normalizedOutput,
      previousSummary.hopCount,
    ) &&
    previousProofOpenEnvelopes === null
  ) {
    throw kagemushaFieldCodecError("previousProofOpenEnvelopes");
  }
  const lineageVerifierKeyValue = kagemushaObjectValue(request, [
    "lineageVerifierKey",
    "lineage_verifier_key",
  ]);
  const lineageVerifierKey =
    lineageVerifierKeyValue === undefined || lineageVerifierKeyValue === null
      ? null
      : kagemushaBytesValue(lineageVerifierKeyValue, "lineageVerifierKey");
  const lineageProvingKeyArchiveValue = kagemushaObjectValue(request, [
    "lineageProvingKeyArchive",
    "lineage_proving_key_archive",
  ]);
  const lineageProvingKeyArchive =
    lineageProvingKeyArchiveValue === undefined ||
    lineageProvingKeyArchiveValue === null
      ? null
      : kagemushaRequireNestedArchive(
          lineageProvingKeyArchiveValue,
          "lineageProvingKeyArchive",
        );
  if (requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(normalizedOutput)) {
    if (lineageVerifierKey === null || lineageVerifierKey.length === 0) {
      throw kagemushaFieldCodecError("lineageVerifierKey");
    }
    if (lineageProvingKeyArchive === null || lineageProvingKeyArchive.length === 0) {
      throw kagemushaFieldCodecError("lineageProvingKeyArchive");
    }
  }
  return Object.freeze({
    previousBundle,
    recordBundle: kagemushaRequiredOwnedBuffer(
      request,
      ["recordBundle", "record_bundle"],
      "recordBundle",
    ),
    pallasOpenEnvelopes: kagemushaRequireNestedArchive(
      kagemushaObjectValue(
        request,
        ["pallasOpenEnvelopes", "pallas_open_envelopes", "pallasOpenEnvelopesArchive"],
      ),
      "pallasOpenEnvelopes",
    ),
    currentNote: kagemushaNormalizeSpendableNote(
      kagemushaObjectValue(request, ["currentNote", "current_note"]),
    ),
    outputProofCircuitId,
    previousLineageVerifierRecord,
    previousProofOpenEnvelopes,
    lineageVerifierKey,
    lineageProvingKeyArchive,
    blockHeight: kagemushaNormalizeBlockHeight(
      kagemushaObjectValue(request, ["blockHeight", "block_height"]),
      "blockHeight",
    ),
  });
}

function kagemushaNormalizeVerifyRequest(request) {
  kagemushaRequirePlainObject(request, "request");
  const lineageVerifierRecordValue = kagemushaObjectValue(request, [
    "lineageVerifierRecord",
    "lineage_verifier_record",
  ]);
  return Object.freeze({
    bundle: kagemushaRequiredOwnedBuffer(request, ["bundle"], "bundle"),
    lineageVerifierRecord:
      lineageVerifierRecordValue === undefined || lineageVerifierRecordValue === null
        ? null
        : kagemushaNormalizeVerifierRecordRef(
            lineageVerifierRecordValue,
            "lineageVerifierRecord",
          ),
    blockHeight: kagemushaNormalizeBlockHeight(
      kagemushaObjectValue(request, ["blockHeight", "block_height"]),
      "blockHeight",
    ),
  });
}

function kagemushaNormalizeRedeemRequest(request) {
  kagemushaRequirePlainObject(request, "request");
  const recipient = kagemushaObjectValue(request, ["recipient"]);
  kagemushaRequireNonBlankUnpadded(recipient, "recipient");
  const lineageWitnessValue = kagemushaObjectValue(request, [
    "lineageWitness",
    "lineage_witness",
  ]);
  const lineageVerifierRecordValue = kagemushaObjectValue(request, [
    "lineageVerifierRecord",
    "lineage_verifier_record",
  ]);
  return Object.freeze({
    bundle: kagemushaRequiredOwnedBuffer(request, ["bundle"], "bundle"),
    recipient,
    publicAmount: kagemushaCanonicalU128Decimal(
      kagemushaObjectValue(request, ["publicAmount", "public_amount"]),
      "publicAmount",
    ),
    redeemProof: kagemushaRequiredOwnedBuffer(
      request,
      ["redeemProof", "redeem_proof"],
      "redeemProof",
    ),
    lineageWitness:
      lineageWitnessValue === undefined || lineageWitnessValue === null
        ? null
        : kagemushaRequireNestedArchive(lineageWitnessValue, "lineageWitness"),
    lineageVerifierRecord:
      lineageVerifierRecordValue === undefined || lineageVerifierRecordValue === null
        ? null
        : kagemushaNormalizeVerifierRecordRef(
            lineageVerifierRecordValue,
            "lineageVerifierRecord",
          ),
    blockHeight: kagemushaNormalizeBlockHeight(
      kagemushaObjectValue(request, ["blockHeight", "block_height"]),
      "blockHeight",
    ),
  });
}

function kagemushaNormalizeSpendableNote(input) {
  kagemushaRequirePlainObject(input, "currentNote");
  const noteCommitment = kagemushaRequiredOwnedBuffer(
    input,
    ["noteCommitment", "note_commitment", "commitment"],
    "noteCommitment",
  );
  const spendNullifier = kagemushaRequiredOwnedBuffer(
    input,
    ["spendNullifier", "spend_nullifier", "nullifier"],
    "spendNullifier",
  );
  if (noteCommitment.length !== 32) {
    throw kagemushaFieldCodecError("noteCommitment", "noteCommitment must be exactly 32 bytes");
  }
  if (spendNullifier.length !== 32) {
    throw kagemushaFieldCodecError("spendNullifier", "spendNullifier must be exactly 32 bytes");
  }
  if (noteCommitment.every((byte) => byte === 0)) {
    throw kagemushaFieldCodecError("noteCommitment");
  }
  if (spendNullifier.every((byte) => byte === 0) || spendNullifier.equals(noteCommitment)) {
    throw kagemushaFieldCodecError("spendNullifier");
  }
  const amount = kagemushaCanonicalU128Decimal(
    kagemushaObjectValue(input, ["amount"]),
    "amount",
  );
  const commitment = Buffer.from(noteCommitment);
  const nullifier = Buffer.from(spendNullifier);
  return Object.freeze({
    get noteCommitment() {
      return Buffer.from(commitment);
    },
    get note_commitment() {
      return Buffer.from(commitment);
    },
    get spendNullifier() {
      return Buffer.from(nullifier);
    },
    get spend_nullifier() {
      return Buffer.from(nullifier);
    },
    amount,
  });
}

function kagemushaNormalizeVerifierRecordRef(input, field) {
  kagemushaRequirePlainObject(input, field);
  const verifierKeyId = kagemushaObjectValue(input, [
    "verifierKeyId",
    "verifier_key_id",
  ]);
  kagemushaRequirePortableId(verifierKeyId, `${field}.verifierKeyId`);
  const recordBytes = kagemushaRequiredOwnedBuffer(
    input,
    ["recordBytes", "record_bytes"],
    `${field}.recordBytes`,
  );
  kagemushaCompactPayloadForRequest(
    recordBytes,
    KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
    field,
  );
  const storedRecord = Buffer.from(recordBytes);
  return Object.freeze({
    verifierKeyId,
    verifier_key_id: verifierKeyId,
    get recordBytes() {
      return Buffer.from(storedRecord);
    },
    get record_bytes() {
      return Buffer.from(storedRecord);
    },
  });
}

function kagemushaNoritoArchiveForType(typeName, payload) {
  const payloadBuffer = Buffer.from(payload);
  if (payloadBuffer.length === 0) {
    throw kagemushaArchiveCodecError(typeName, "payload must not be empty");
  }
  const header = Buffer.alloc(PRIVACY_NORITO_HEADER_BYTES);
  PRIVACY_NORITO_MAGIC.copy(header, 0);
  kagemushaSchemaHashForTypeName(typeName).copy(header, 6);
  header.writeBigUInt64LE(BigInt(payloadBuffer.length), 23);
  header.writeBigUInt64LE(privacyCrc64(payloadBuffer), 31);
  header[39] = KAGEMUSHA_NORITO_COMPACT_LEN_FLAG;
  return Buffer.concat([header, payloadBuffer]);
}

function kagemushaSchemaHashForTypeName(typeName) {
  return createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(typeName, "utf8"))
    .digest()
    .subarray(0, 16);
}

function kagemushaTypedArchivePayload(archive, schema, field) {
  const buffer = toKagemushaArchiveView(archive, field);
  const payload = assertKagemushaNoritoArchive(buffer, field);
  if (!buffer.subarray(6, 22).equals(kagemushaSchemaHashForTypeName(schema))) {
    throw kagemushaArchiveCodecError(field, `${field} must use ${schema}`);
  }
  if (buffer[22] !== 0) {
    throw kagemushaArchiveCodecError(field, `${field} must not be compressed`);
  }
  return { payload: Buffer.from(payload), flags: buffer[39] };
}

function kagemushaCompactPayloadForRequest(archive, schema, field) {
  const { payload, flags } = kagemushaTypedArchivePayload(archive, schema, field);
  if (flags !== KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) {
    throw kagemushaArchiveCodecError(field, `${field} must use compact Norito layout`);
  }
  return payload;
}

function kagemushaRequireNestedArchive(value, field) {
  if (value === undefined || value === null) {
    throw kagemushaArchiveCodecError(field);
  }
  try {
    return toOwnedKagemushaArchiveBuffer(value, field);
  } catch (error) {
    throw kagemushaArchiveCodecError(field, error.message);
  }
}

function kagemushaSpendableNotePayload(note) {
  const normalized = kagemushaNormalizeSpendableNote(note);
  return Buffer.concat([
    kagemushaField(kagemushaFixedBytesPayload(normalized.noteCommitment)),
    kagemushaField(kagemushaFixedBytesPayload(normalized.spendNullifier)),
    kagemushaField(kagemushaNumeric(normalized.amount)),
  ]);
}

function kagemushaFixedBytesPayload(bytes) {
  const out = [];
  for (const byte of bytes) {
    out.push(kagemushaField(Buffer.from([byte])));
  }
  return Buffer.concat(out);
}

function kagemushaNumeric(value) {
  const integer = kagemushaCanonicalU128BigInt(value, "amount");
  const mantissa = kagemushaPositiveTwosComplementLittleEndian(integer);
  const mantissaHeader = Buffer.alloc(4);
  mantissaHeader.writeUInt32LE(mantissa.length, 0);
  const scale = Buffer.alloc(4);
  return Buffer.concat([
    kagemushaField(Buffer.concat([mantissaHeader, mantissa])),
    kagemushaField(scale),
  ]);
}

function kagemushaU128(value, field) {
  const integer = kagemushaCanonicalU128BigInt(value, field);
  const out = Buffer.alloc(16);
  let remaining = integer;
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  if (remaining !== 0n) {
    throw kagemushaFieldCodecError(field, `${field} must fit in u128`);
  }
  return out;
}

function kagemushaPositiveTwosComplementLittleEndian(integer) {
  if (integer === 0n) {
    return Buffer.alloc(0);
  }
  const bytes = [];
  let remaining = integer;
  while (remaining > 0n) {
    bytes.push(Number(remaining & 0xffn));
    remaining >>= 8n;
  }
  if ((bytes[bytes.length - 1] & 0x80) !== 0) {
    bytes.push(0);
  }
  return Buffer.from(bytes);
}

function kagemushaVerifyingKeyBoxPayload(lineageVerifierKey) {
  const key = Buffer.from(lineageVerifierKey ?? Buffer.alloc(0));
  if (key.length === 0) {
    throw kagemushaFieldCodecError("lineageVerifierKey");
  }
  return Buffer.concat([
    kagemushaField(kagemushaString(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND)),
    kagemushaField(kagemushaBytesVec(key)),
  ]);
}

function kagemushaAccountIdPayload(recipient) {
  try {
    const { address } = AccountAddress.parseEncoded(recipient);
    const canonical = Buffer.from(address.canonicalBytes());
    if (canonical.length < 4 || canonical[1] !== 0) {
      throw new Error("recipient controller");
    }
    const curveId = canonical[2];
    const keyLength = canonical[3];
    if (4 + keyLength !== canonical.length) {
      throw new Error("recipient controller length");
    }
    const publicKey = canonical.subarray(4);
    const controllerTag = Buffer.alloc(4);
    controllerTag.writeUInt32LE(0, 0);
    return Buffer.concat([
      controllerTag,
      kagemushaField(kagemushaPublicKeyPayload(curveId, publicKey)),
    ]);
  } catch {
    throw kagemushaFieldCodecError(
      "recipient",
      "recipient must use canonical I105 account form",
    );
  }
}

function kagemushaPublicKeyPayload(curveId, publicKey) {
  const tags = new Map([
    [0x01, 0],
    [0x04, 1],
    [0x03, 2],
    [0x05, 3],
    [0x02, 4],
    [0x0a, 5],
    [0x0b, 6],
    [0x0c, 7],
    [0x0d, 8],
    [0x0e, 9],
    [0x0f, 10],
  ]);
  const tag = tags.get(curveId);
  if (tag === undefined) {
    throw kagemushaFieldCodecError("recipient", `unsupported recipient curve id: ${curveId}`);
  }
  return kagemushaFixedBytesPayload(Buffer.concat([Buffer.from([tag]), publicKey]));
}

function kagemushaReadAccumulatorSummary(payload, flags) {
  let offset = 0;
  let field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.kind");
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.chainId");
  const chainId = kagemushaReadChainIdPayload(field.payload, flags);
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.asset");
  const assetBytes = kagemushaReadFixedBytesFlexible(field.payload, flags, 16, "asset");
  const asset = `hex:${assetBytes.toString("hex")}`;
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.initialRoot");
  const initialRoot = kagemushaReadFixedBytesFlexible(field.payload, flags, 32, "initialRoot");
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.finalRoot");
  const finalRoot = kagemushaReadFixedBytesFlexible(field.payload, flags, 32, "finalRoot");
  offset = field.offset;
  offset = kagemushaSkipFields(payload, offset, flags, 1, "accumulator");
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.hopCount");
  const hopCount = kagemushaReadU32Payload(field.payload);
  offset = field.offset;
  offset = kagemushaSkipFields(payload, offset, flags, 15, "accumulator");
  field = kagemushaReadNoritoField(payload, offset, flags, "accumulator.currentNote");
  const currentNote = kagemushaReadSpendableNotePayload(field.payload, flags);
  offset = field.offset;
  if (offset !== payload.length) {
    throw kagemushaArchiveCodecError("bundle", "accumulator has trailing bytes");
  }
  return { chainId, asset, initialRoot, finalRoot, hopCount, currentNote };
}

function kagemushaReadChainIdPayload(payload, flags) {
  try {
    return kagemushaReadStringPayload(payload, flags, "chainId");
  } catch {
    const field = kagemushaReadNoritoField(payload, 0, flags, "chainId");
    if (field.offset !== payload.length) {
      throw kagemushaArchiveCodecError("bundle", "chainId has trailing bytes");
    }
    return kagemushaReadStringPayload(field.payload, flags, "chainId");
  }
}

function kagemushaReadRecursiveProofCircuitId(payload, flags) {
  const first = kagemushaReadNoritoField(payload, 0, flags, "recursiveProof.verifierKeyId");
  const verifierPayload = first.payload;
  let offset = 0;
  let field = kagemushaReadNoritoField(verifierPayload, offset, flags, "verifierKeyId.backend");
  offset = field.offset;
  field = kagemushaReadNoritoField(verifierPayload, offset, flags, "verifierKeyId.name");
  const name = kagemushaReadStringPayload(field.payload, flags, "verifierKeyId.name");
  offset = field.offset;
  if (offset !== verifierPayload.length) {
    throw kagemushaArchiveCodecError("bundle", "verifierKeyId has trailing bytes");
  }
  kagemushaRequirePortableId(name, "verifierKeyId");
  return name;
}

function kagemushaReadSpendableNotePayload(payload, flags) {
  let offset = 0;
  let field = kagemushaReadNoritoField(payload, offset, flags, "currentNote.noteCommitment");
  const noteCommitment = kagemushaReadFixedBytesFlexible(
    field.payload,
    flags,
    32,
    "noteCommitment",
  );
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "currentNote.spendNullifier");
  const spendNullifier = kagemushaReadFixedBytesFlexible(
    field.payload,
    flags,
    32,
    "spendNullifier",
  );
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "currentNote.amount");
  const amount = kagemushaReadNumericPayload(field.payload, flags);
  offset = field.offset;
  if (offset !== payload.length) {
    throw kagemushaArchiveCodecError("bundle", "currentNote has trailing bytes");
  }
  return kagemushaNormalizeSpendableNote({
    noteCommitment,
    spendNullifier,
    amount,
  });
}

function kagemushaReadFixedBytesFlexible(payload, flags, expectedSize, field) {
  if (payload.length === expectedSize) {
    return Buffer.from(payload);
  }
  try {
    return kagemushaReadConstVecPayload(payload, flags, 0, expectedSize, field);
  } catch {
    if (payload.length < 8) {
      throw kagemushaArchiveCodecError(field);
    }
    const count = payload.readBigUInt64LE(0);
    if (count !== BigInt(expectedSize)) {
      throw kagemushaArchiveCodecError(field);
    }
    return kagemushaReadConstVecPayload(payload, flags, 8, expectedSize, field);
  }
}

function kagemushaReadConstVecPayload(payload, flags, start, expectedSize, field) {
  let offset = start;
  const bytes = [];
  while (offset < payload.length) {
    const fieldPayload = kagemushaReadNoritoField(payload, offset, flags, field);
    if (fieldPayload.payload.length !== 1) {
      throw kagemushaArchiveCodecError(field);
    }
    bytes.push(fieldPayload.payload[0]);
    offset = fieldPayload.offset;
  }
  if (bytes.length !== expectedSize) {
    throw kagemushaArchiveCodecError(field);
  }
  return Buffer.from(bytes);
}

function kagemushaReadNumericPayload(payload, flags) {
  let offset = 0;
  let field = kagemushaReadNoritoField(payload, offset, flags, "amount.mantissa");
  const mantissaPayload = field.payload;
  offset = field.offset;
  field = kagemushaReadNoritoField(payload, offset, flags, "amount.scale");
  const scalePayload = field.payload;
  offset = field.offset;
  if (offset !== payload.length || mantissaPayload.length < 4) {
    throw kagemushaArchiveCodecError("amount");
  }
  const mantissaLength = mantissaPayload.readUInt32LE(0);
  if (4 + mantissaLength !== mantissaPayload.length) {
    throw kagemushaArchiveCodecError("amount");
  }
  if (scalePayload.length !== 4 || scalePayload.readUInt32LE(0) !== 0) {
    throw kagemushaFieldCodecError("amount", "numeric scale must be zero");
  }
  const integer = kagemushaBigIntFromLittleTwosComplement(mantissaPayload.subarray(4));
  return kagemushaCanonicalU128Decimal(integer, "amount");
}

function kagemushaBigIntFromLittleTwosComplement(payload) {
  if (payload.length === 0) {
    return 0n;
  }
  let value = 0n;
  for (let index = payload.length - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(payload[index]);
  }
  if ((payload[payload.length - 1] & 0x80) !== 0) {
    value -= 1n << (8n * BigInt(payload.length));
  }
  return value;
}

function kagemushaReadFieldValue(buffer, offset, flags, context, decode) {
  const field = kagemushaReadNoritoField(buffer, offset, flags, context);
  return [decode(field.payload), field.offset];
}

function kagemushaReadBoolPayload(payload) {
  if (payload.length === 1 && payload[0] === 0) {
    return false;
  }
  if (payload.length === 1 && payload[0] === 1) {
    return true;
  }
  throw kagemushaFieldCodecError("bool");
}

function kagemushaReadU32Payload(payload) {
  if (payload.length !== 4) {
    throw kagemushaArchiveCodecError("u32");
  }
  return payload.readUInt32LE(0);
}

function kagemushaReadStringPayload(payload, flags, context) {
  try {
    return kagemushaDecodeNoritoString(payload, flags, context);
  } catch (error) {
    throw kagemushaArchiveCodecError(context, error.message);
  }
}

function kagemushaSkipFields(payload, offset, flags, count, context) {
  let cursor = offset;
  for (let index = 0; index < count; index += 1) {
    cursor = kagemushaReadNoritoField(payload, cursor, flags, context).offset;
  }
  return cursor;
}

function kagemushaField(payload) {
  return kagemushaRawField(payload);
}

function kagemushaRawField(payload) {
  const bytes = Buffer.from(payload);
  return Buffer.concat([kagemushaCompactLength(bytes.length), bytes]);
}

function kagemushaString(value) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([kagemushaCompactLength(bytes.length), bytes]);
}

function kagemushaBytesVec(value) {
  const bytes = value === undefined || value === null ? Buffer.alloc(0) : Buffer.from(value);
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(bytes.length), 0);
  return Buffer.concat([length, bytes]);
}

function kagemushaOptionRaw(payload) {
  if (payload === undefined || payload === null) {
    return Buffer.from([0]);
  }
  return Buffer.concat([Buffer.from([1]), kagemushaRawField(payload)]);
}

function kagemushaOptionBytesVec(value) {
  if (value === undefined || value === null) {
    return Buffer.from([0]);
  }
  return Buffer.concat([Buffer.from([1]), kagemushaField(kagemushaBytesVec(value))]);
}

function kagemushaOptionU64(value) {
  if (value === undefined || value === null) {
    return Buffer.from([0]);
  }
  const checked = kagemushaNormalizeBlockHeight(value, "blockHeight");
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64LE(checked, 0);
  return Buffer.concat([Buffer.from([1]), kagemushaField(bytes)]);
}

function kagemushaCompactLength(value) {
  let remaining = typeof value === "bigint" ? value : BigInt(value);
  if (remaining < 0n || remaining > U64_MAX) {
    throw kagemushaArchiveCodecError("length");
  }
  const bytes = [];
  while (remaining >= 0x80n) {
    bytes.push(Number((remaining & 0x7fn) | 0x80n));
    remaining >>= 7n;
  }
  bytes.push(Number(remaining));
  return Buffer.from(bytes);
}

function kagemushaCanonicalU128Decimal(value, field) {
  return kagemushaCanonicalU128BigInt(value, field).toString(10);
}

function kagemushaCanonicalU128BigInt(value, field) {
  let integer;
  if (typeof value === "bigint") {
    integer = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw kagemushaFieldCodecError(field, `${field} must be a canonical decimal u128`);
    }
    integer = BigInt(value);
  } else if (typeof value === "string") {
    if (!/^\d+$/.test(value) || (value.length > 1 && value.startsWith("0"))) {
      throw kagemushaFieldCodecError(field, `${field} must be a canonical decimal u128`);
    }
    integer = BigInt(value);
  } else {
    throw kagemushaFieldCodecError(field, `${field} must be a canonical decimal u128`);
  }
  if (integer <= 0n || integer > U128_MAX) {
    throw kagemushaFieldCodecError(field, `${field} must fit in u128`);
  }
  return integer;
}

function kagemushaNormalizeBlockHeight(value, field) {
  if (value === undefined || value === null) {
    return null;
  }
  let height;
  if (typeof value === "bigint") {
    height = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw kagemushaFieldCodecError(field, `${field} must be a non-negative u64`);
    }
    if (value < 0 || Object.is(value, -0)) {
      throw kagemushaFieldCodecError(field, `${field} must be non-negative`);
    }
    height = BigInt(value);
  } else if (typeof value === "string") {
    if (!/^(0|[1-9]\d*)$/.test(value)) {
      throw kagemushaFieldCodecError(
        field,
        `${field} must be a canonical unsigned decimal u64`,
      );
    }
    height = BigInt(value);
  } else {
    throw kagemushaFieldCodecError(field, `${field} must be a non-negative u64`);
  }
  if (height < 0n || height > U64_MAX) {
    throw kagemushaFieldCodecError(field, `${field} must fit in u64`);
  }
  return height;
}

function kagemushaRequirePortableId(value, field) {
  kagemushaRequireNonBlankUnpadded(value, field);
  if (value.length > 256 || !/^[A-Za-z0-9._\-/:@+=]+$/u.test(value)) {
    throw kagemushaFieldCodecError(field, `${field} must use portable registry syntax`);
  }
}

function kagemushaRequireNonBlankUnpadded(value, field) {
  if (typeof value !== "string" || value.trim().length === 0 || value.trim() !== value) {
    throw kagemushaFieldCodecError(field, `${field} must be a non-empty unpadded string`);
  }
}

function kagemushaRequiredOwnedBuffer(input, names, field) {
  const value = kagemushaObjectValue(input, names);
  if (value === undefined || value === null) {
    throw kagemushaFieldCodecError(field);
  }
  return kagemushaBytesValue(value, field);
}

function kagemushaBytesValue(value, field) {
  try {
    return toOwnedBuffer(value, field);
  } catch (error) {
    throw kagemushaFieldCodecError(field, error.message);
  }
}

function kagemushaObjectValue(input, names) {
  for (const name of names) {
    if (Object.prototype.hasOwnProperty.call(input, name)) {
      return input[name];
    }
  }
  return undefined;
}

function kagemushaRequirePlainObject(value, field) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw kagemushaFieldCodecError(field, `${field} must be an object`);
  }
}

function kagemushaFieldCodecError(field, message) {
  return new KagemushaRecursiveSpendRequestCodecError("field", field, message);
}

function kagemushaArchiveCodecError(field, message) {
  return new KagemushaRecursiveSpendRequestCodecError("archive", field, message);
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

function toOwnedBuffer(value, name) {
  return Buffer.from(toBuffer(value, name));
}

function toKagemushaArchiveView(value, name) {
  if (typeof value === "string") {
    const byteLength = Buffer.byteLength(value, "utf8");
    if (byteLength > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES) {
      throw new Error(
        `${name} must not exceed ${KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes`,
      );
    }
    return Buffer.from(value, "utf8");
  }
  return toBuffer(value, name);
}

function toOwnedKagemushaArchiveBuffer(value, name) {
  const view = toKagemushaArchiveView(value, name);
  if (view.length === 0) {
    throw new Error(`${name} must not be empty`);
  }
  if (view.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES) {
    throw new Error(
      `${name} must not exceed ${KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes`,
    );
  }
  assertKagemushaNoritoArchive(view, name);
  return Buffer.from(view);
}

function requiredString(value, name) {
  const text = typeof value === "string" ? value.trim() : String(value ?? "").trim();
  if (!text) {
    throw new Error(`${name} must be a non-empty string`);
  }
  return text;
}

function fixed32Buffer(value, name) {
  const hexValue = normalizeFixed32HexInput(value, name);
  return Buffer.from(hexValue, "hex");
}

function optionalFixed32Buffer(value, name) {
  if (value === undefined || value === null) {
    return undefined;
  }
  return fixed32Buffer(value, name);
}

function normalizeProofAttachmentFromNative(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("native ZK-ACE prover returned invalid proof attachment");
  }
  const proof = { ...value };
  if (typeof proof.proof_b64 !== "string" || proof.proof_b64.length === 0) {
    throw new Error("native ZK-ACE prover returned missing proof_b64");
  }
  if (typeof proof.backend !== "string" || proof.backend.length === 0) {
    throw new Error("native ZK-ACE prover returned missing proof backend");
  }
  const verifyingKeyRef = proof.vk_ref ?? proof.verifying_key_ref ?? proof.verifyingKeyRef;
  if (!verifyingKeyRef || typeof verifyingKeyRef !== "object" || Array.isArray(verifyingKeyRef)) {
    throw new Error("native ZK-ACE prover returned missing verifying key reference");
  }
  const normalized = {
    backend: proof.backend,
    proof_b64: proof.proof_b64,
    vk_ref: verifyingKeyRef,
  };
  const commitment =
    proof.vk_commitment ?? proof.verifying_key_commitment ?? proof.verifyingKeyCommitment;
  if (commitment !== undefined && commitment !== null) {
    normalized.vk_commitment = commitment;
  }
  const envelopeHash = proof.envelope_hash ?? proof.envelopeHash;
  if (envelopeHash !== undefined && envelopeHash !== null) {
    normalized.envelope_hash = envelopeHash;
  }
  return normalized;
}

function normalizeZkAceAuthorizationResult(result) {
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    throw new Error("native ZK-ACE prover returned invalid authorization payload");
  }
  return {
    publicInputs: result.public_inputs ?? result.publicInputs,
    public_inputs: result.public_inputs ?? result.publicInputs,
    proof: normalizeProofAttachmentFromNative(result.proof),
    identityCommitment: result.identity_commitment,
    identity_commitment: result.identity_commitment,
    txDigest: result.tx_digest,
    tx_digest: result.tx_digest,
    replayNullifier: result.replay_nullifier,
    replay_nullifier: result.replay_nullifier,
    policyHash: result.policy_hash,
    policy_hash: result.policy_hash,
    verifierKeyId: result.verifier_key_id,
    verifier_key_id: result.verifier_key_id,
    authorizationProofBytes: Number(result.authorization_proof_bytes ?? 0),
    authorization_proof_bytes: Number(result.authorization_proof_bytes ?? 0),
    authorizationPublicInputBytes: Number(result.authorization_public_input_bytes ?? 0),
    authorization_public_input_bytes: Number(result.authorization_public_input_bytes ?? 0),
    replayNullifierBytes: Number(result.replay_nullifier_bytes ?? 32),
    replay_nullifier_bytes: Number(result.replay_nullifier_bytes ?? 32),
  };
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

function normalizePositiveU128Literal(value, name) {
  let amount;
  if (typeof value === "bigint") {
    amount = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new Error(`${name} must be a positive decimal u128 string`);
    }
    amount = BigInt(value);
  } else if (typeof value === "string") {
    const normalized = value.trim();
    if (!/^\d+$/.test(normalized)) {
      throw new Error(`${name} must be a positive decimal u128 string`);
    }
    amount = BigInt(normalized);
  } else {
    throw new Error(`${name} must be a positive decimal u128 string`);
  }
  if (amount <= 0n || amount > U128_MAX) {
    throw new Error(`${name} must be a positive decimal u128 string`);
  }
  return amount.toString(10);
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
