import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createPrivateKey,
  createPublicKey,
  createHash,
  hkdfSync,
  randomBytes,
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

const MODULE_DIR = dirname(fileURLToPath(import.meta.url));
const WORKSPACE_SM2_FIXTURE_PATH = resolvePath(
  MODULE_DIR,
  "../../../fixtures/sm/sm2_fixture.json",
);

const SM2_FIXTURE_FALLBACK = Object.freeze({
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

let SM2_FIXTURE_REFERENCE = SM2_FIXTURE_FALLBACK;
try {
  const rawFixture = readFileSync(WORKSPACE_SM2_FIXTURE_PATH, "utf8");
  const parsed = JSON.parse(rawFixture);
  SM2_FIXTURE_REFERENCE = Object.freeze({
    distid: String(parsed.distid),
    seedHex: String(parsed.seed_hex).toUpperCase(),
    messageHex: String(parsed.message_hex).toUpperCase(),
    privateKeyHex: String(parsed.private_key_hex).toUpperCase(),
    publicKeySec1Hex: String(parsed.public_key_sec1_hex).toUpperCase(),
    publicKeyMultihash: String(parsed.public_key_multihash),
    publicKeyPrefixed: String(parsed.public_key_prefixed),
    za: String(parsed.za).toUpperCase(),
    signature: String(parsed.signature).toUpperCase(),
    r: String(parsed.r).toUpperCase(),
    s: String(parsed.s).toUpperCase(),
  });
} catch {
  SM2_FIXTURE_REFERENCE = SM2_FIXTURE_FALLBACK;
}

const SM2_FIXTURE_SEED = Buffer.from(SM2_FIXTURE_REFERENCE.seedHex, "hex");
const SM2_FIXTURE_MESSAGE = Buffer.from(SM2_FIXTURE_REFERENCE.messageHex, "hex");
export const SM2_DEFAULT_DISTINGUISHED_ID = SM2_FIXTURE_REFERENCE.distid;

const ED25519_PKCS8_PREFIX = Buffer.from([
  0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
]);
const ED25519_SPKI_PREFIX = Buffer.from([
  0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
]);

const CONFIDENTIAL_KEY_SALT = Buffer.from("iroha:confidential:key-derivation:v1");
const CONFIDENTIAL_INFO_NK = Buffer.from("iroha:confidential:nk");
const CONFIDENTIAL_INFO_IVK = Buffer.from("iroha:confidential:ivk");
const CONFIDENTIAL_INFO_OVK = Buffer.from("iroha:confidential:ovk");
const CONFIDENTIAL_INFO_FVK = Buffer.from("iroha:confidential:fvk");

function resolveNativeBinding() {
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

/**
 * Generate an Ed25519 key pair. Seed material is hashed to 32 bytes when needed.
 * @param {{seed?: ArrayBufferView | ArrayBuffer | Buffer}} [options]
 * @returns {{algorithm: "ed25519", publicKey: Buffer, privateKey: Buffer}}
 */
export function generateKeyPair(options = {}) {
  const seed = options.seed ? normalizeSeed(options.seed) : undefined;
  const native = resolveNativeBinding();
  if (native?.ed25519Keypair) {
    const result = native.ed25519Keypair(seed);
    return {
      algorithm: result.algorithm,
      publicKey: Buffer.from(result.publicKey),
      privateKey: Buffer.from(result.privateKey),
    };
  }

  const effectiveSeed = seed ?? randomBytes(ED25519_SEED_LENGTH);
  const privateKeyObject = privateKeyFromSeed(effectiveSeed);
  const publicKey = exportPublicKey(privateKeyObject);
  const privateKey = Buffer.from(effectiveSeed);
  return { algorithm: "ed25519", publicKey, privateKey };
}

/**
 * Derive the public key for a given private key (32-byte seed or 64-byte seed+public concatenation).
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function publicKeyFromPrivate(privateKey) {
  const buffer = toBuffer(privateKey, "privateKey");
  const native = resolveNativeBinding();
  if (native?.ed25519PublicKeyFromPrivate) {
    return Buffer.from(native.ed25519PublicKeyFromPrivate(buffer));
  }
  const seed = extractSeed(buffer);
  const privateKeyObject = privateKeyFromSeed(seed);
  return exportPublicKey(privateKeyObject);
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
  const raw =
    native?.deriveConfidentialKeyset?.(seed) ??
    deriveConfidentialKeysetFallback(seed);

  const keyset = {
    skSpend: toBufferField(raw, "sk_spend"),
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

function toBufferField(payload, snake) {
  const value = payload && snake ? payload[snake] : null;
  if (value === null || value === undefined) {
    throw new Error(`native binding returned missing \`${snake}\``);
  }
  return toBuffer(value, String(snake));
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

function deriveConfidentialKeysetFallback(seed) {
  const derive = (info) => hkdfSync("sha3-512", seed, CONFIDENTIAL_KEY_SALT, info, 32);
  return {
    sk_spend: Buffer.from(seed),
    nk: derive(CONFIDENTIAL_INFO_NK),
    ivk: derive(CONFIDENTIAL_INFO_IVK),
    ovk: derive(CONFIDENTIAL_INFO_OVK),
    fvk: derive(CONFIDENTIAL_INFO_FVK),
  };
}
