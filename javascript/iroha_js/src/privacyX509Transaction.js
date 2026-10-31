import { Buffer } from "buffer";
import { getNativeBinding } from "./native.js";
import { networkIdBytes } from "./networkId.js";
import { feePaymentIntentToNoritoJson } from "./transaction.js";
import { privacyExact12AuthenticatedConstructionArchiveV1 } from "./privacyCapabilities.js";
import { preparePrivacyX509DirectBytesV1 } from "./privacyX509Wire.js";

const TOKEN = Symbol("original X509 direct transaction");
const preparedStates = new WeakMap();
const PROOF_LIMIT = 9 * 1024 * 1024;
const PAYLOAD_LIMIT = 10 * 1024 * 1024;
const INPUT_KEYS = new Set(["manifest", "networkId", "requestNorito", "authority", "feePayment",
  "creationTimeMs", "ttlMs", "nonce", "metadata"]);

function bytes(value, maximum, label) {
  if (!(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) {
    throw new TypeError(`${label} must be bytes`);
  }
  if (value.byteLength === 0 || value.byteLength > maximum) {
    throw new RangeError(`${label} is outside its byte bound`);
  }
  const view = value instanceof ArrayBuffer
    ? new Uint8Array(value) : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  return Buffer.from(view);
}

function safeInteger(value, label, positive = false) {
  if (!Number.isSafeInteger(value) || value < (positive ? 1 : 0)) {
    throw new TypeError(`${label} must be an exact ${positive ? "positive" : "nonnegative"} safe integer`);
  }
  return value;
}

function metadataJson(value) {
  if (value === undefined || value === null) return null;
  if (typeof value === "string") return value;
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("metadata must be an object or Norito JSON string");
  }
  return JSON.stringify(value);
}

/** Immutable original direct X509 plan. Construct with preparePrivacyX509DirectTransactionV1. */
export class PrivacyX509DirectTransactionV1 {
  constructor(token, state) {
    if (token !== TOKEN) throw new TypeError("X509 prepared transactions have no public constructor");
    preparedStates.set(this, state);
    Object.freeze(this);
  }

  /** Copy of the exact statement to prove. */
  get materializedStatementNorito() { return stateFor(this).plan.materializedStatementNorito; }
  /** Copy of the complete original ordered intent digest. */
  get orderedIntentDigest() { return stateFor(this).plan.orderedIntentDigest; }
  /** Copy of the original ActionStatement frame. */
  get actionStatementNorito() { return stateFor(this).plan.actionStatementNorito; }
  /** Copy of the original ActionStatement digest. */
  get actionStatementDigest() { return stateFor(this).plan.actionStatementDigest; }

  /**
   * Complete the one original Proof/ActionProof occurrence and sign it natively.
   * The exact 32-byte Ed25519 key must control the retained authority. The native owner
   * rechecks tuple, network, committed limits and the exact X5S1 container/header binding;
   * proof/State verification remains consensus work.
   */
  sign(rawProof, privateKey) {
    const state = stateFor(this);
    privacyExact12AuthenticatedConstructionArchiveV1(state.manifest, state.networkId);
    const proof = bytes(rawProof, PROOF_LIMIT, "rawProof");
    const key = bytes(privateKey, 32, "privateKey");
    try {
      if (key.length !== 32) throw new RangeError("privateKey must be exactly 32 bytes");
      const result = state.sign(proof, key);
      const signedTransaction = bytes(result?.signedTransaction, PAYLOAD_LIMIT + 4096, "native signed transaction");
      const hash = bytes(result?.hash, 32, "native transaction hash");
      if (hash.length !== 32) throw new Error("native X509 signer returned an invalid hash");
      return { signedTransaction, hash };
    } finally {
      key.fill(0);
    }
  }
}

function stateFor(owner) {
  const state = preparedStates.get(owner);
  if (!state) throw new TypeError("invalid X509 prepared transaction receiver");
  return state;
}

/**
 * Freeze one direct X509 request before proof generation. The request must carry ZERO
 * derived intent and action/slot zero. `manifest` must come from authenticated Torii.
 * Mixed actions and contract invocations have separate ownership and are not accepted here.
 */
export function preparePrivacyX509DirectTransactionV1(input) {
  if (input === null || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("X509 input must be an object");
  }
  for (const key of Object.keys(input)) {
    if (!INPUT_KEYS.has(key)) throw new TypeError(`unsupported X509 input field ${key}`);
  }
  const creationTimeMs = safeInteger(input.creationTimeMs, "creationTimeMs");
  const ttlMs = safeInteger(input.ttlMs, "ttlMs", true);
  // This checks the private authenticated origin; no caller callback or admission-shaped
  // object can replace it. Final selected-profile admission is the native owner's job.
  const manifest = input.manifest;
  const networkId = input.networkId;
  const archive = privacyExact12AuthenticatedConstructionArchiveV1(manifest, networkId);
  const requestNorito = bytes(input.requestNorito, 256 * 1024 + 32, "requestNorito");
  const authority = input.authority;
  if (typeof authority !== "string" || !authority || authority.trim() !== authority) {
    throw new TypeError("authority must be the exact original account literal");
  }
  const nonce = input.nonce ?? null;
  if (nonce !== null && (!Number.isInteger(nonce) || nonce < 1 || nonce > 0xffff_ffff)) {
    throw new TypeError("nonce must be a nonzero u32");
  }
  const nativeInput = {
    manifest: archive,
    networkId: Buffer.from(networkIdBytes(networkId, "networkId")),
    requestNorito,
    authority,
    feePaymentJson: feePaymentIntentToNoritoJson(input.feePayment),
    metadataJson: metadataJson(input.metadata),
    creationTimeMs,
    ttlMs,
    nonce,
  };
  const native = getNativeBinding();
  if (typeof native?.privacyPrepareX509DirectTransactionV1 !== "function") {
    throw new Error("authenticated native X509 producer is unavailable");
  }
  const owner = native.privacyPrepareX509DirectTransactionV1(nativeInput);
  if (typeof owner?.sign !== "function") throw new Error("native X509 producer returned no signing owner");
  const plan = preparePrivacyX509DirectBytesV1(requestNorito,
    bytes(owner.originalPayloadBytes, PAYLOAD_LIMIT, "native original payload"));
  for (const field of ["materializedStatementNorito", "orderedIntentDigest", "actionStatementNorito", "actionStatementDigest"]) {
    if (!bytes(owner[field], 256 * 1024 + 1024, `native ${field}`).equals(plan[field])) {
      throw new Error(`native X509 ${field} differs from the original byte plan`);
    }
  }
  return new PrivacyX509DirectTransactionV1(TOKEN, {
    manifest, networkId, plan, sign: owner.sign.bind(owner),
  });
}

/** Prepare, ask the supplied holder prover for the exact statement's proof, then sign. */
export async function buildPrivacyX509DirectTransactionV1(input, { prove, privateKey } = {}) {
  if (typeof prove !== "function") throw new TypeError("prove must be a holder prover callback");
  const prepared = preparePrivacyX509DirectTransactionV1(input);
  const proof = await prove(prepared.materializedStatementNorito);
  return prepared.sign(proof, privateKey);
}
