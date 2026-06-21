import { Buffer } from "buffer";
import { ed25519 } from "@noble/curves/ed25519";
import { sha256 } from "@noble/hashes/sha256";

const ED25519_SEED_LENGTH = 32;
const ED25519_PUBLIC_KEY_LENGTH = 32;
const ED25519_PRIVATE_KEY_LENGTH = 64;

export const SM2_PRIVATE_KEY_LENGTH = 32;
export const SM2_PUBLIC_KEY_LENGTH = 65;
export const SM2_SIGNATURE_LENGTH = 64;
export const SM2_DEFAULT_DISTINGUISHED_ID = "1234567812345678";
export const PRIVACY_FFI_VERSION_V1 = 1;
export const PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 7;
export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
export const PRIVACY_FFI_STATUS_ERROR = 1;
export const PRIVACY_FFI_ERROR_NULL_POINTER = 1;
export const PRIVACY_FFI_ERROR_MALFORMED_NORITO = 2;
export const PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM = 3;
export const PRIVACY_FFI_ERROR_PRODUCTION_DISABLED = 4;
export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;
const PRIVACY_NORITO_HEADER_BYTES = 40;
const PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES = 64;
const PRIVACY_NORITO_SUPPORTED_FLAGS_MASK = 0x27;
const PRIVACY_NORITO_FIELD_BITSET_FLAG = 0x20;
const PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06;
const PRIVACY_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const PRIVACY_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
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

function normalizeSeed(seed) {
  const buffer = toBuffer(seed, "seed");
  if (buffer.length === ED25519_SEED_LENGTH) {
    return Buffer.from(buffer);
  }
  return Buffer.from(sha256(buffer));
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
    const publicKey = Buffer.from(buffer.subarray(ED25519_SEED_LENGTH));
    const derivedPublic = Buffer.from(ed25519.getPublicKey(seed));
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

function unsupported(operation) {
  throw new Error(`${operation} is unavailable in browser-only crypto builds.`);
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

/**
 * Generate an Ed25519 key pair. Seed material is hashed to 32 bytes when needed.
 * @param {{seed?: ArrayBufferView | ArrayBuffer | Buffer, algorithm?: string}} [options]
 * @returns {{algorithm: "ed25519", publicKey: Buffer, privateKey: Buffer}}
 */
export function generateKeyPair(options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    return unsupported(`generateKeyPair(${algorithm})`);
  }
  const seed = options.seed ? normalizeSeed(options.seed) : Buffer.from(ed25519.utils.randomPrivateKey());
  return {
    algorithm: "ed25519",
    publicKey: Buffer.from(ed25519.getPublicKey(seed)),
    privateKey: Buffer.from(seed),
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
    return unsupported(`publicKeyFromPrivate(${algorithm})`);
  }
  const seed = extractSeed(privateKey);
  return Buffer.from(ed25519.getPublicKey(seed));
}

export function loadKeyPair(privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    return unsupported(`loadKeyPair(${algorithm})`);
  }
  const privateKeyBuffer = toBuffer(privateKey, "privateKey");
  return {
    algorithm,
    publicKey: publicKeyFromPrivate(privateKeyBuffer),
    privateKey: extractSeed(privateKeyBuffer),
    distid: null,
  };
}

/**
 * Sign a message using an Ed25519 private key.
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function signEd25519(message, privateKey) {
  const messageBuffer = toBuffer(message, "message");
  const seed = extractSeed(privateKey);
  return Buffer.from(ed25519.sign(messageBuffer, seed));
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
  return ed25519.verify(signatureBuffer, messageBuffer, publicKeyBuffer);
}

export function sign(message, privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    return unsupported(`sign(${algorithm})`);
  }
  return signEd25519(message, privateKey);
}

export function verify(message, signature, publicKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  if (algorithm !== CRYPTO_ALGORITHMS.ED25519) {
    return unsupported(`verify(${algorithm})`);
  }
  return verifyEd25519(message, signature, publicKey);
}

export function publicKeyMultihash(_publicKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  return unsupported(`publicKeyMultihash(${algorithm})`);
}

export function privateKeyMultihash(_privateKey, options = {}) {
  const algorithm = normalizeCryptoAlgorithm(options.algorithm);
  return unsupported(`privateKeyMultihash(${algorithm})`);
}

export function generateSm2KeyPair() {
  return unsupported("generateSm2KeyPair");
}

export function deriveSm2KeyPairFromSeed() {
  return unsupported("deriveSm2KeyPairFromSeed");
}

export function loadSm2KeyPair() {
  return unsupported("loadSm2KeyPair");
}

export function sm2PublicKeyMultihash() {
  return unsupported("sm2PublicKeyMultihash");
}

export function signSm2() {
  return unsupported("signSm2");
}

export function verifySm2() {
  return unsupported("verifySm2");
}

export function buildKaigiRosterJoinProof() {
  return unsupported("buildKaigiRosterJoinProof");
}

export function buildZkAceTransferAuthorizationV1() {
  return unsupported("buildZkAceTransferAuthorizationV1");
}

export function deriveConfidentialKeyset() {
  return unsupported("deriveConfidentialKeyset");
}

export function deriveConfidentialKeysetFromHex() {
  return unsupported("deriveConfidentialKeysetFromHex");
}

export function deriveConfidentialOwnerTagV2() {
  return unsupported("deriveConfidentialOwnerTagV2");
}

export function deriveConfidentialDiversifierV2() {
  return unsupported("deriveConfidentialDiversifierV2");
}

export function deriveConfidentialReceiveAddressV2() {
  return unsupported("deriveConfidentialReceiveAddressV2");
}

export function deriveConfidentialNoteV2() {
  return unsupported("deriveConfidentialNoteV2");
}

export function deriveConfidentialNullifierV2() {
  return unsupported("deriveConfidentialNullifierV2");
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
export const KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN =
  "iroha:kagemusha:v1:recursive-spend-accumulator";
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

export class KagemushaRecursiveSpendRequestCodecError extends Error {
  constructor(kind, field, message) {
    super(message ?? `invalid Kagemusha recursive spend ${kind}: ${field}`);
    this.name = "KagemushaRecursiveSpendRequestCodecError";
    this.kind = kind;
    this.field = field;
  }
}

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
  return Buffer.from(
    sha256(
      Buffer.concat([
        Buffer.from("iroha:zk:v1:vk", "utf8"),
        backendLength,
        backend,
        verifierKeyLength,
        lineageVerifierKey,
      ]),
    ),
  );
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

function privacyCrc64(payload) {
  let crc = PRIVACY_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = PRIVACY_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ PRIVACY_CRC64_MASK);
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

export function isKagemushaRecursiveSpendNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveCompactPaymentTokenNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable() {
  return false;
}

export function isKagemushaCompactPaymentTokenNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveAggregationProofBundleNativeAvailable() {
  return false;
}

export function isKagemushaPallasOpenEnvelopeBuilderNativeAvailable() {
  return false;
}

export function kagemushaProveVerifiedCompactPaymentTokenWithRecords() {
  return unsupported("kagemushaProveVerifiedCompactPaymentTokenWithRecords");
}

export function kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes() {
  return unsupported(
    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
  );
}

export function kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes() {
  return unsupported(
    "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
  );
}

export function kagemushaBuildPallasOpenEnvelopesArchive() {
  return unsupported("kagemushaBuildPallasOpenEnvelopesArchive");
}

export function kagemushaBuildPreviousProofOpenEnvelopesArchive() {
  return unsupported("kagemushaBuildPreviousProofOpenEnvelopesArchive");
}

export function kagemushaVerifyRecursiveCompactPaymentToken() {
  return unsupported("kagemushaVerifyRecursiveCompactPaymentToken");
}

export function kagemushaRecursiveSpendCompactPaymentTokenFromBundle() {
  return unsupported("kagemushaRecursiveSpendCompactPaymentTokenFromBundle");
}

export function kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection() {
  return unsupported("kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection");
}

export function kagemushaRecursiveSpendInit() {
  return unsupported("kagemushaRecursiveSpendInit");
}

export function kagemushaRecursiveSpendAppend() {
  return unsupported("kagemushaRecursiveSpendAppend");
}

export function kagemushaRecursiveSpendTransitionProfileInit() {
  return unsupported("kagemushaRecursiveSpendTransitionProfileInit");
}

export function kagemushaRecursiveSpendTransitionProfileAppend() {
  return unsupported("kagemushaRecursiveSpendTransitionProfileAppend");
}

export function kagemushaRecursiveSpendLineageAppendBoundary() {
  return unsupported("kagemushaRecursiveSpendLineageAppendBoundary");
}

export function kagemushaRecursiveSpendLineageWitnessFromInitResult() {
  return unsupported("kagemushaRecursiveSpendLineageWitnessFromInitResult");
}

export function kagemushaRecursiveSpendLineageWitnessAppendResult() {
  return unsupported("kagemushaRecursiveSpendLineageWitnessAppendResult");
}

export function kagemushaRecursiveSpendVerify() {
  return unsupported("kagemushaRecursiveSpendVerify");
}

export function kagemushaRecursiveSpendRedeem() {
  return unsupported("kagemushaRecursiveSpendRedeem");
}

export function buildKagemushaRecursiveSpendableNoteDescriptor() {
  return unsupported("buildKagemushaRecursiveSpendableNoteDescriptor");
}

export function buildKagemushaRecursiveSpendVerifierRecordRef() {
  return unsupported("buildKagemushaRecursiveSpendVerifierRecordRef");
}

export function encodeKagemushaRecursiveSpendInitRequest() {
  return unsupported("encodeKagemushaRecursiveSpendInitRequest");
}

export function encodeKagemushaRecursiveSpendAppendRequest() {
  return unsupported("encodeKagemushaRecursiveSpendAppendRequest");
}

export function encodeKagemushaRecursiveSpendVerifyRequest() {
  return unsupported("encodeKagemushaRecursiveSpendVerifyRequest");
}

export function encodeKagemushaRecursiveSpendRedeemRequest() {
  return unsupported("encodeKagemushaRecursiveSpendRedeemRequest");
}

export function decodeKagemushaRecursiveSpendVerifyResult() {
  return unsupported("decodeKagemushaRecursiveSpendVerifyResult");
}

export function decodeKagemushaRecursiveSpendBundle() {
  return unsupported("decodeKagemushaRecursiveSpendBundle");
}

export function kagemushaRecursiveSpendInitTyped() {
  return unsupported("kagemushaRecursiveSpendInitTyped");
}

export function kagemushaRecursiveSpendAppendTyped() {
  return unsupported("kagemushaRecursiveSpendAppendTyped");
}

export function kagemushaRecursiveSpendVerifyTyped() {
  return unsupported("kagemushaRecursiveSpendVerifyTyped");
}

export function kagemushaRecursiveSpendRedeemTyped() {
  return unsupported("kagemushaRecursiveSpendRedeemTyped");
}

export function isPrivacyNativeAvailable() {
  return false;
}

export function privacyCapabilitiesV1() {
  return unsupported("privacyCapabilitiesV1");
}

export function privacyProofRequestV1() {
  return unsupported("privacyProofRequestV1");
}

export function privacyBuildProofV1() {
  return unsupported("privacyBuildProofV1");
}

export function privacyVerifyProofV1() {
  return unsupported("privacyVerifyProofV1");
}

export function sm2FixtureFromSeed() {
  return unsupported("sm2FixtureFromSeed");
}
