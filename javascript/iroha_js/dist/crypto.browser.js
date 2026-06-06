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
export const PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 6;
export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
export const PRIVACY_FFI_STATUS_ERROR = 1;
export const PRIVACY_FFI_ERROR_NULL_POINTER = 1;
export const PRIVACY_FFI_ERROR_MALFORMED_NORITO = 2;
export const PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM = 3;
export const PRIVACY_FFI_ERROR_PRODUCTION_DISABLED = 4;
export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;

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
export const KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION = 6;
export const KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7;
export const KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1 = "kagemusha-recursive-compact-v1";
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

export function isKagemushaCompactPaymentTokenNativeAvailable() {
  return false;
}

export function isKagemushaRecursiveAggregationProofBundleNativeAvailable() {
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

export function kagemushaVerifyRecursiveCompactPaymentToken() {
  return unsupported("kagemushaVerifyRecursiveCompactPaymentToken");
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

export function isPrivacyNativeAvailable() {
  return false;
}

export function privacyCapabilitiesV1() {
  return unsupported("privacyCapabilitiesV1");
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
