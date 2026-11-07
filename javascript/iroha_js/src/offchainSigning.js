// Off-chain signing envelope of Iroha account signing keys.
//
// Mirrors `iroha_data_model::account::signing_key::offchain_signing_message_v1`
// byte for byte (specs/account_signing_keys.md). An account's controller signs
// only transactions; everything the account signs off chain, such as an Iroha
// Connect raw request or a Torii canonical request, is signed by its separate
// signing key over this envelope. The envelope starts with a fixed prefix and
// is longer than any 32-byte hash, so an off-chain signature is never a
// transaction or query signature.

import { AccountAddress } from "./address.js";
import { networkIdBytes } from "./networkId.js";
import { verifyEd25519Strict } from "./ed25519Strict.js";

const encoder = new TextEncoder();
const OFFCHAIN_SIGNING_DOMAIN_V1 = encoder.encode("iroha:offchain:v1\0");
const ED25519_MULTIHASH_PATTERN = /^ed0120([0-9A-F]{64})$/u;
const UINT32_MAX = 0xffffffff;

/** Domain tag of Torii canonical-request authentication. */
export const TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1 = "iroha:torii:canonical-request:v1";

/** Purpose codes of the off-chain envelope. The signer chooses; a requester never does. */
export const OffchainSignaturePurposeV1 = Object.freeze({
  CONNECT_RAW: 1,
  TORII_CANONICAL_REQUEST: 2,
});

/** The bytes every off-chain signing message starts with. */
export function offchainSigningDomainV1() {
  return Uint8Array.from(OFFCHAIN_SIGNING_DOMAIN_V1);
}

/**
 * The purpose a wallet signs a Connect raw request under: a Torii canonical
 * request for its tag, otherwise opaque Connect bytes.
 */
export function offchainPurposeForConnectRawTag(domainTag) {
  return domainTag === TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1
    ? OffchainSignaturePurposeV1.TORII_CANONICAL_REQUEST
    : OffchainSignaturePurposeV1.CONNECT_RAW;
}

function requireUint32(value, name) {
  if (!Number.isInteger(value) || value < 0 || value > UINT32_MAX) {
    throw new RangeError(`${name} must be an unsigned 32-bit integer`);
  }
  return value;
}

function requireBytes(value, name) {
  if (value instanceof Uint8Array) {
    return Uint8Array.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  if (value instanceof ArrayBuffer) {
    return Uint8Array.from(new Uint8Array(value));
  }
  throw new TypeError(`${name} must be a Uint8Array, ArrayBuffer view or ArrayBuffer`);
}

function writeUint32LE(out, offset, value) {
  new DataView(out.buffer, out.byteOffset, out.byteLength).setUint32(offset, value, true);
}

/**
 * Build the message an account's signing key signs off chain.
 *
 * Layout: `"iroha:offchain:v1\0" || purpose u8 || network_id [u8; 32] ||
 * u32 LE account length || canonical account-address bytes || generation u32 LE ||
 * u32 LE tag length || UTF-8 tag || u64 LE bytes length || bytes`.
 *
 * The account is named by its canonical address bytes when the caller already
 * holds them from a trusted source (`accountCanonicalBytes`, for example a
 * Torii native identity in a browser); otherwise its I105 `accountId` is
 * parsed by the native codec.
 *
 * @param {{purpose: number, networkId: import("./networkId.js").NetworkId, accountId?: string,
 *   accountCanonicalBytes?: ArrayBuffer | ArrayBufferView, generation: number,
 *   domainTag: string, bytes: ArrayBuffer | ArrayBufferView}} input
 * @returns {Uint8Array}
 */
export function buildOffchainSigningMessageV1(input) {
  if (!input || typeof input !== "object") {
    throw new TypeError("off-chain signing input must be an object");
  }
  const { purpose, networkId, accountId, accountCanonicalBytes, generation, domainTag, bytes } = input;
  if (
    purpose !== OffchainSignaturePurposeV1.CONNECT_RAW &&
    purpose !== OffchainSignaturePurposeV1.TORII_CANONICAL_REQUEST
  ) {
    throw new RangeError("purpose must be an OffchainSignaturePurposeV1 code");
  }
  requireUint32(generation, "generation");
  if (typeof domainTag !== "string" || domainTag.length === 0) {
    throw new TypeError("domainTag must be a non-empty string");
  }
  const network = networkIdBytes(networkId, "networkId");
  const account = accountCanonicalBytes === undefined
    ? AccountAddress.fromAccountId(accountId).canonicalBytes()
    : requireBytes(accountCanonicalBytes, "accountCanonicalBytes");
  if (account.length === 0) {
    throw new TypeError("account canonical address bytes must not be empty");
  }
  const tag = encoder.encode(domainTag);
  const payload = requireBytes(bytes, "bytes");
  requireUint32(account.length, "account address length");
  requireUint32(tag.length, "domainTag length");
  const length = OFFCHAIN_SIGNING_DOMAIN_V1.length + 1 + network.length + 4 + account.length +
    4 + 4 + tag.length + 8 + payload.length;
  const out = new Uint8Array(length);
  let offset = 0;
  out.set(OFFCHAIN_SIGNING_DOMAIN_V1, offset);
  offset += OFFCHAIN_SIGNING_DOMAIN_V1.length;
  out[offset] = purpose;
  offset += 1;
  out.set(network, offset);
  offset += network.length;
  writeUint32LE(out, offset, account.length);
  offset += 4;
  out.set(account, offset);
  offset += account.length;
  writeUint32LE(out, offset, generation);
  offset += 4;
  writeUint32LE(out, offset, tag.length);
  offset += 4;
  out.set(tag, offset);
  offset += tag.length;
  new DataView(out.buffer).setBigUint64(offset, BigInt(payload.length), true);
  offset += 8;
  out.set(payload, offset);
  return out;
}

/**
 * Normalize an account signing key read from committed state.
 *
 * Accepts the JSON body of Torii `GET /v1/accounts/{account_id}/signing-key`
 * (`{ public_key: "ed0120…", generation }`) or `{ publicKey: Uint8Array, generation }`.
 *
 * @returns {{readonly publicKey: Uint8Array, readonly generation: number}}
 */
export function normalizeAccountSigningKey(value) {
  if (!value || typeof value !== "object") {
    throw new TypeError("account signing key must be an object");
  }
  const generation = requireUint32(value.generation, "account signing key generation");
  let publicKey;
  if (typeof value.public_key === "string") {
    const match = ED25519_MULTIHASH_PATTERN.exec(value.public_key);
    if (match === null) {
      throw new TypeError("account signing key must be a canonical Ed25519 multihash (ed0120…)");
    }
    publicKey = Uint8Array.from(match[1].match(/../gu), (pair) => Number.parseInt(pair, 16));
  } else {
    publicKey = requireBytes(value.publicKey, "account signing key publicKey");
  }
  if (publicKey.length !== 32) {
    throw new TypeError("account signing key must be a 32-byte Ed25519 public key");
  }
  return Object.freeze({ publicKey, generation });
}

/**
 * Verify an Ed25519 off-chain signature against an account's registered
 * signing key. The key must come from committed state, never from the signer.
 *
 * @returns {boolean}
 */
export function verifyOffchainSignatureV1({
  signingKey,
  purpose,
  networkId,
  accountId,
  accountCanonicalBytes,
  domainTag,
  bytes,
  signature,
}) {
  const key = normalizeAccountSigningKey(signingKey);
  const message = buildOffchainSigningMessageV1({
    purpose,
    networkId,
    accountId,
    accountCanonicalBytes,
    generation: key.generation,
    domainTag,
    bytes,
  });
  try {
    return verifyEd25519Strict(message, requireBytes(signature, "signature"), key.publicKey);
  } catch {
    return false;
  }
}
