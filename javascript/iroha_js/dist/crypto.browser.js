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

export function generateKeyPair(options = {}) {
  const seed = options.seed ? normalizeSeed(options.seed) : Buffer.from(ed25519.utils.randomPrivateKey());
  return {
    algorithm: "ed25519",
    publicKey: Buffer.from(ed25519.getPublicKey(seed)),
    privateKey: Buffer.from(seed),
  };
}

export function publicKeyFromPrivate(privateKey) {
  const seed = extractSeed(privateKey);
  return Buffer.from(ed25519.getPublicKey(seed));
}

export function signEd25519(message, privateKey) {
  const messageBuffer = toBuffer(message, "message");
  const seed = extractSeed(privateKey);
  return Buffer.from(ed25519.sign(messageBuffer, seed));
}

export function verifyEd25519(message, signature, publicKey) {
  const messageBuffer = toBuffer(message, "message");
  const signatureBuffer = toBuffer(signature, "signature");
  const publicKeyBuffer = normalizePublicKey(publicKey);
  return ed25519.verify(signatureBuffer, messageBuffer, publicKeyBuffer);
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

export function deriveConfidentialKeyset() {
  return unsupported("deriveConfidentialKeyset");
}

export function deriveConfidentialKeysetFromHex() {
  return unsupported("deriveConfidentialKeysetFromHex");
}

export function sm2FixtureFromSeed() {
  return unsupported("sm2FixtureFromSeed");
}
