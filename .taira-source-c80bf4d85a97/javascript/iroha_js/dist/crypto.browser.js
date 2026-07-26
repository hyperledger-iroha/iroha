import { Buffer } from "buffer";
import { ed25519 } from "@noble/curves/ed25519";
import { sha256 } from "@noble/hashes/sha256";
import { verifyEd25519Strict } from "./ed25519Strict.js";
import {
  entropyToMnemonic,
  generateMnemonic,
  mnemonicToEntropy,
  validateMnemonic,
} from "@scure/bip39";
import { wordlist as englishWordlist } from "@scure/bip39/wordlists/english.js";

const ED25519_SEED_LENGTH = 32;
const ED25519_PUBLIC_KEY_LENGTH = 32;
const ED25519_PRIVATE_KEY_LENGTH = 64;
const RECOVERY_PHRASE_WORD_COUNTS = new Set([12, 24]);
const RECOVERY_PHRASE_STRENGTH_BITS = new Map([
  [12, 128],
  [24, 256],
]);
const RECOVERY_ENTROPY_LENGTH_TO_WORD_COUNT = new Map([
  [16, 12],
  [32, 24],
]);

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
  return verifyEd25519Strict(messageBuffer, signatureBuffer, publicKeyBuffer);
}

function recoveryPhraseWords(phrase) {
  if (typeof phrase !== "string") {
    throw new TypeError("recovery phrase must be a string");
  }
  return phrase.normalize("NFKD").trim().toLowerCase().split(/\s+/).filter(Boolean);
}

export function normalizeRecoveryPhrase(phrase) {
  const words = recoveryPhraseWords(phrase);
  const wordCount = words.length;
  if (!RECOVERY_PHRASE_WORD_COUNTS.has(wordCount)) {
    throw new Error("recovery phrase must contain 12 or 24 words");
  }
  const normalizedPhrase = words.join(" ");
  if (!validateMnemonic(normalizedPhrase, englishWordlist)) {
    throw new Error("recovery phrase checksum or word list is invalid");
  }
  return { phrase: normalizedPhrase, words, wordCount };
}

export function validateRecoveryPhrase(phrase) {
  try {
    normalizeRecoveryPhrase(phrase);
    return true;
  } catch {
    return false;
  }
}

export function generateRecoveryPhrase(wordCount = 24) {
  const strength = RECOVERY_PHRASE_STRENGTH_BITS.get(wordCount);
  if (!strength) {
    throw new Error("recovery phrase word count must be 12 or 24");
  }
  return normalizeRecoveryPhrase(generateMnemonic(englishWordlist, strength));
}

export function entropyToRecoveryPhrase(entropy) {
  const buffer = toBuffer(entropy, "entropy");
  if (!RECOVERY_ENTROPY_LENGTH_TO_WORD_COUNT.has(buffer.length)) {
    throw new Error("recovery phrase entropy must be 16 or 32 bytes");
  }
  return normalizeRecoveryPhrase(entropyToMnemonic(buffer, englishWordlist));
}

export function recoveryPhraseToEntropy(phrase) {
  const recovery = normalizeRecoveryPhrase(phrase);
  return Buffer.from(mnemonicToEntropy(recovery.phrase, englishWordlist));
}

export function deriveEd25519SeedFromRecoveryPhrase(phrase) {
  return normalizeSeed(recoveryPhraseToEntropy(phrase));
}

export function ed25519SeedToRecoveryPhrase(privateKey) {
  return entropyToRecoveryPhrase(extractSeed(privateKey));
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

function privacyCrc64(payload) {
  let crc = PRIVACY_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = PRIVACY_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ PRIVACY_CRC64_MASK);
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
