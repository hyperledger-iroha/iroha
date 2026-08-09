import { Buffer } from "buffer";
// Keep the canonical UTF-16 ordering primitive inside the audited Nexus graph.
import { compareUtf16 } from "./ordering.js";
import { AccountAddress } from "./address.js";
import {
  createConnectAppSession,
  createConnectSessionPreview,
  registerConnectSession,
} from "./connect.browser.js";
import { blake2b256 } from "./blake2b.js";
import { verifyEd25519 } from "./crypto.browser.js";
import {
  BrowserTransactionCodecError,
  browserTransactionCodec,
  browserSignedTransactionHashHex,
  finalizeBrowserSignedTransaction,
  validateBrowserTransferSignable,
} from "./transactionCodec.js";
import {
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "./numericV1.js";

void compareUtf16;

const ALGORITHM_ED25519 = "ed25519";
const ALGORITHM_ED25519_TAG = 0;
const MAX_PAYLOAD_BYTES = 1024 * 1024;
const MAX_SIGNED_TRANSACTION_BYTES = MAX_PAYLOAD_BYTES + 4096;
const MAX_TORII_RESPONSE_BYTES = 64 * 1024;
const DEFAULT_TORII_REQUEST_TIMEOUT_MS = 15_000;
const DEFAULT_STATUS_POLL_INTERVAL_MS = 1_000;
const DEFAULT_STATUS_POLL_TIMEOUT_MS = 30_000;
const DEFAULT_FAILURE_STATUSES = Object.freeze(["Rejected", "Expired"]);
const PIPELINE_STATUS_KINDS = new Set([
  "Queued",
  "Approved",
  "Committed",
  "Applied",
  "Rejected",
  "Expired",
]);
const PIPELINE_STATUS_SOURCES = new Set(["queue", "cache", "state"]);
const abortSignalAbortedGetter =
  typeof AbortSignal === "undefined"
    ? null
    : (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "aborted")?.get ??
      null);
const abortSignalReasonGetter =
  typeof AbortSignal === "undefined"
    ? null
    : (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "reason")?.get ??
      null);
const abortSignalEventTargetPrototype =
  typeof AbortSignal === "undefined"
    ? null
    : Object.getPrototypeOf(AbortSignal.prototype);
const abortSignalAddEventListener =
  abortSignalEventTargetPrototype === null
    ? null
    : (Object.getOwnPropertyDescriptor(
        abortSignalEventTargetPrototype,
        "addEventListener",
      )?.value ?? null);
const abortSignalRemoveEventListener =
  abortSignalEventTargetPrototype === null
    ? null
    : (Object.getOwnPropertyDescriptor(
        abortSignalEventTargetPrototype,
        "removeEventListener",
      )?.value ?? null);
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedArrayBufferGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "buffer",
)?.get;
const typedArrayByteLengthGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteLength",
)?.get;

function assertByteLength(length, maxBytes, context) {
  if (maxBytes !== undefined && length > maxBytes) {
    throw new TypeError(`${context} exceeds ${maxBytes} bytes`);
  }
}

function toBuffer(value, context, { maxBytes } = {}) {
  try {
    if (Buffer.isBuffer(value)) {
      assertByteLength(value.length, maxBytes, context);
      return Buffer.from(value);
    }
    if (value instanceof Uint8Array) {
      assertByteLength(value.byteLength, maxBytes, context);
      return Buffer.from(value);
    }
    if (ArrayBuffer.isView(value)) {
      assertByteLength(value.byteLength, maxBytes, context);
      return Buffer.from(
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      );
    }
    if (value instanceof ArrayBuffer) {
      assertByteLength(value.byteLength, maxBytes, context);
      return Buffer.from(new Uint8Array(value));
    }
    if (Array.isArray(value)) {
      if (Object.getPrototypeOf(value) !== Array.prototype) {
        throw new TypeError(`${context} byte arrays must use Array.prototype`);
      }
      const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
      const length = lengthDescriptor?.value;
      if (!Number.isSafeInteger(length) || length < 0) {
        throw new TypeError(`${context} byte array has an invalid length`);
      }
      assertByteLength(length, maxBytes, context);
      const ownKeys = Reflect.ownKeys(value);
      if (ownKeys.length !== length + 1) {
        throw new TypeError(`${context} byte arrays must be dense without custom fields`);
      }
      const bytes = new Uint8Array(length);
      for (let index = 0; index < length; index += 1) {
        const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
        if (
          !descriptor ||
          !descriptor.enumerable ||
          !Object.prototype.hasOwnProperty.call(descriptor, "value") ||
          !Number.isInteger(descriptor.value) ||
          descriptor.value < 0 ||
          descriptor.value > 255
        ) {
          throw new TypeError(
            `${context}[${index}] must be an integer byte from 0 through 255`,
          );
        }
        bytes[index] = descriptor.value;
      }
      return Buffer.from(bytes);
    }
  } catch (error) {
    if (error instanceof TypeError && error.message.startsWith(context)) {
      throw error;
    }
    throw new TypeError(`${context} must reference readable bytes`, { cause: error });
  }
  if (typeof value === "string") {
    const trimmed = value.startsWith("0x") ? value.slice(2) : value;
    if (maxBytes !== undefined && trimmed.length > maxBytes * 2) {
      throw new TypeError(`${context} exceeds ${maxBytes} bytes`);
    }
    if (/^[0-9a-fA-F]*$/.test(trimmed) && trimmed.length % 2 === 0) {
      return Buffer.from(trimmed, "hex");
    }
  }
  throw new TypeError(`${context} must be bytes or a hex string`);
}

function isBytesInput(value) {
  return (
    Buffer.isBuffer(value) ||
    value instanceof Uint8Array ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer ||
    Array.isArray(value) ||
    typeof value === "string"
  );
}

function exactHashHex(value, context, code) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new NexusAppError(
      code,
      `${context} must be exactly 64 lowercase hexadecimal characters`,
    );
  }
  return value;
}

function normalizeHashValue(value, context, code, { hexOnly = false } = {}) {
  if (typeof value === "string") {
    return exactHashHex(value, context, code);
  }
  if (hexOnly) {
    throw new NexusAppError(
      code,
      `${context} must be exactly 64 lowercase hexadecimal characters`,
    );
  }
  let bytes;
  try {
    bytes = toBuffer(value, context, { maxBytes: 32 });
  } catch (error) {
    throw new NexusAppError(code, `${context} must be an exact 32-byte hash`, error);
  }
  if (bytes.length !== 32) {
    throw new NexusAppError(code, `${context} must be an exact 32-byte hash`);
  }
  return bytes.toString("hex");
}

function ownDataDescriptor(value, key, context, code) {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (!descriptor) return null;
  if (!Object.prototype.hasOwnProperty.call(descriptor, "value")) {
    throw new NexusAppError(code, `${context}.${key} must be a data field`);
  }
  return descriptor;
}

function snapshotDataFields(value, allowed, context, code) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new NexusAppError(code, `${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new NexusAppError(code, `${context} must be a plain object`);
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== "string" ||
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      throw new NexusAppError(code, `${context} must contain enumerable data fields only`);
    }
    if (!allowed.has(key)) {
      throw new NexusAppError(code, `${context}.${key} is not supported`);
    }
    Object.defineProperty(snapshot, key, {
      value: descriptor.value,
      enumerable: true,
      configurable: false,
      writable: false,
    });
  }
  return Object.freeze(snapshot);
}

function normalizeAliasFamily(
  value,
  aliases,
  context,
  code,
  { normalize = (candidate) => candidate, equals = Object.is } = {},
) {
  let selected = null;
  let selectedKey = null;
  let found = false;
  for (const key of aliases) {
    const descriptor = ownDataDescriptor(value, key, context, code);
    if (!descriptor || descriptor.value === undefined || descriptor.value === null) continue;
    const normalized = normalize(descriptor.value, `${context}.${key}`);
    if (found && !equals(selected, normalized)) {
      throw new NexusAppError(
        code,
        `${context}.${selectedKey} conflicts with ${context}.${key}`,
      );
    }
    selected = normalized;
    selectedKey = key;
    found = true;
  }
  return found ? selected : null;
}

function normalizeByteAliases(value, aliases, context, code, { maxBytes } = {}) {
  let selected = null;
  let selectedKey = null;
  for (const key of aliases) {
    const descriptor = ownDataDescriptor(value, key, context, code);
    if (!descriptor || descriptor.value === undefined || descriptor.value === null) continue;
    let normalized;
    try {
      normalized = toBuffer(descriptor.value, `${context}.${key}`, { maxBytes });
    } catch (error) {
      throw new NexusAppError(code, `${context}.${key} must be bytes`, error);
    }
    if (selected !== null && !selected.equals(normalized)) {
      throw new NexusAppError(
        code,
        `${context}.${selectedKey} conflicts with ${context}.${key}`,
      );
    }
    selected = normalized;
    selectedKey = key;
  }
  return selected;
}

function normalizeConsistentByteSources(sources, code, { maxBytes } = {}) {
  let selected = null;
  let selectedContext = null;
  for (const { value, context } of sources) {
    if (value === undefined || value === null) continue;
    let normalized;
    try {
      normalized = toBuffer(value, context, { maxBytes });
    } catch (error) {
      throw new NexusAppError(code, `${context} must be bytes`, error);
    }
    if (selected !== null && !selected.equals(normalized)) {
      throw new NexusAppError(
        code,
        `${selectedContext} conflicts with ${context}`,
      );
    }
    selected = normalized;
    selectedContext = context;
  }
  return selected;
}

function normalizeHashAliases(value, aliases, context, invalidCode, conflictCode) {
  let selected = null;
  let selectedKey = null;
  for (const { key, hexOnly = false } of aliases) {
    const descriptor = ownDataDescriptor(value, key, context, invalidCode);
    if (!descriptor || descriptor.value === undefined || descriptor.value === null) continue;
    const normalized = normalizeHashValue(
      descriptor.value,
      `${context}.${key}`,
      invalidCode,
      { hexOnly },
    );
    if (selected !== null && selected !== normalized) {
      throw new NexusAppError(
        conflictCode,
        `${context}.${selectedKey} conflicts with ${context}.${key}`,
      );
    }
    selected = normalized;
    selectedKey = key;
  }
  return selected;
}

const PAYLOAD_BYTE_ALIASES = Object.freeze(["payloadBytes", "payload_bytes", "bytes"]);
const SIGNED_BYTE_ALIASES = Object.freeze([
  "signedTransaction",
  "signed_transaction",
  "bytes",
]);
const PAYLOAD_HASH_ALIASES = Object.freeze([
  Object.freeze({ key: "payloadHashHex", hexOnly: true }),
  Object.freeze({ key: "payload_hash_hex", hexOnly: true }),
  Object.freeze({ key: "hashHex", hexOnly: true }),
  Object.freeze({ key: "hash_hex", hexOnly: true }),
  Object.freeze({ key: "hash" }),
]);
const TRANSACTION_HASH_ALIASES = Object.freeze([
  Object.freeze({ key: "hashHex", hexOnly: true }),
  Object.freeze({ key: "hash_hex", hexOnly: true }),
  Object.freeze({ key: "transactionHashHex", hexOnly: true }),
  Object.freeze({ key: "transaction_hash_hex", hexOnly: true }),
  Object.freeze({ key: "signedTransactionHashHex", hexOnly: true }),
  Object.freeze({ key: "signed_transaction_hash_hex", hexOnly: true }),
  Object.freeze({ key: "signedTransactionHash" }),
  Object.freeze({ key: "signed_transaction_hash" }),
  Object.freeze({ key: "hash" }),
]);
const SUBMISSION_HASH_ALIASES = Object.freeze([
  // `signed_transaction_hash` is a distinct inner-wire identity in Torii receipts.
  Object.freeze({ key: "hashHex", hexOnly: true }),
  Object.freeze({ key: "hash_hex", hexOnly: true }),
  Object.freeze({ key: "transactionHashHex", hexOnly: true }),
  Object.freeze({ key: "transaction_hash_hex", hexOnly: true }),
  Object.freeze({ key: "entrypointHashHex", hexOnly: true }),
  Object.freeze({ key: "entrypoint_hash_hex", hexOnly: true }),
  Object.freeze({ key: "transactionHash" }),
  Object.freeze({ key: "transaction_hash" }),
  Object.freeze({ key: "entrypointHash" }),
  Object.freeze({ key: "entrypoint_hash" }),
  Object.freeze({ key: "hash" }),
  Object.freeze({ key: "txHash" }),
  Object.freeze({ key: "tx_hash" }),
]);
const SIGNATURE_FIELDS = new Set([
  "algorithm",
  "alg",
  "signature",
  "bytes",
  "payload",
]);
const SIGNABLE_FIELDS = new Set([
  "payloadBytes",
  "payloadHashHex",
  "authority",
  "signingPublicKey",
  "signatureAlgorithm",
]);
const CONFIG_FIELDS = new Set([
  "chainId",
  "baseUrl",
  "toriiBaseUrl",
  "connectBaseUrl",
  "node",
  "authority",
  "accountId",
  "signingPublicKey",
  "fetchImpl",
  "webSocketImpl",
  "allowInsecure",
  "appMeta",
  "appMetadata",
  "permissions",
  "connectTransport",
  "connect",
  "transactionCodec",
  "toriiClient",
]);
const TRANSFER_DRAFT_FIELDS = new Set([
  "chainId",
  "authority",
  "accountId",
  "sourceAccountId",
  "sourceAssetHoldingId",
  "sourceAssetId",
  "assetId",
  "quantity",
  "destinationAccountId",
  "destination",
  "to",
  "metadata",
  "creationTimeMs",
  "ttlMs",
  "nonce",
  "feePayment",
  "signingPublicKey",
]);
const FINALIZE_OPTION_FIELDS = new Set([
  "wait",
  "intervalMs",
  "timeoutMs",
  "maxAttempts",
  "failureStatuses",
  "onStatus",
  "signal",
  "signingPublicKey",
  "toriiClient",
]);
const STATUS_WAIT_OPTION_FIELDS = Object.freeze([
  "intervalMs",
  "timeoutMs",
  "maxAttempts",
  "failureStatuses",
  "onStatus",
  "signal",
]);
const CONNECT_OPTION_FIELDS = new Set([
  "sid",
  "chainId",
  "node",
  "appKeyPair",
  "nonce",
  "protocol",
]);
const CONNECT_SESSION_FIELDS = new Set([
  "sid",
  "walletLaunchUri",
  "wallet_launch_uri",
  "wallet_uri",
  "appLaunchUri",
  "app_launch_uri",
  "app_uri",
  "tokenApp",
  "token_app",
  "tokenWallet",
  "token_wallet",
  "tokenManagement",
  "token_management",
  "tokenRelay",
  "token_relay",
  "approvedAccountId",
  "approvedAccount",
  "approved_account",
  "signingPublicKey",
  "signing_public_key",
  "appSession",
  "preview",
]);
const APPROVAL_FIELDS = new Set([
  "accountId",
  "account_id",
  "signingPublicKey",
  "signing_public_key",
  "session",
]);
const BROWSER_CONNECT_APPROVAL_FIELDS = new Set([
  "accountId",
  "walletPublicKey",
  "signature",
]);

function projectBrowserConnectApproval(value) {
  // Browser Connect verifies the approval proof; the facade keeps only the
  // account identity and never treats the X25519 wallet key as a signing key.
  let approval;
  try {
    approval = snapshotDataFields(
      value,
      BROWSER_CONNECT_APPROVAL_FIELDS,
      "browser Connect approval",
      "invalid_wallet_approval",
    );
  } catch (error) {
    throw new NexusAppError(
      "invalid_wallet_approval",
      "browser Connect approval must be an exact data-only proof envelope",
      error,
    );
  }
  for (const [field, byteLength] of [
    ["walletPublicKey", 32],
    ["signature", 64],
  ]) {
    try {
      const bytes = approval[field];
      const buffer = Reflect.apply(typedArrayBufferGetter, bytes, []);
      const actualByteLength = Reflect.apply(
        typedArrayByteLengthGetter,
        bytes,
        [],
      );
      if (
        !(bytes instanceof Uint8Array) ||
        Object.getPrototypeOf(bytes) !== Uint8Array.prototype ||
        (typeof SharedArrayBuffer !== "undefined" &&
          buffer instanceof SharedArrayBuffer) ||
        actualByteLength !== byteLength
      ) {
        throw new TypeError("invalid browser Connect proof bytes");
      }
    } catch (error) {
      throw new NexusAppError(
        "invalid_wallet_approval",
        `browser Connect approval.${field} must be exactly ${byteLength} bytes`,
        error,
      );
    }
  }
  return Object.freeze({ accountId: approval.accountId });
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

function normalizeTransferQuantity(value) {
  try {
    if (value instanceof KotodamaQuantity) {
      return NumericV1.encodeQuantityJson(value);
    }
    if (typeof value === "string") {
      return NumericV1.decodeQuantityJson(value).toString();
    }
    if (typeof value === "bigint") {
      return new KotodamaQuantity(value, 0).toString();
    }
    throw new NexusAppError(
      "invalid_transfer_input",
      "transfer quantity must be a KotodamaQuantity, canonical quantity string, or bigint; JavaScript numbers are rejected",
    );
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new NexusAppError(
      "invalid_transfer_input",
      `transfer quantity must be canonical and non-negative (${error.code})`,
      error,
    );
  }
}

function irohaPrehash(payloadBytes) {
  const digest = Buffer.from(blake2b256(payloadBytes));
  digest[digest.length - 1] |= 1;
  return digest;
}

function irohaPrehashHex(payloadBytes) {
  const digest = irohaPrehash(payloadBytes);
  return digest.toString("hex");
}

function accountEd25519PublicKey(accountId) {
  if (
    typeof accountId !== "string" ||
    accountId.length === 0 ||
    accountId.length > 512 ||
    accountId.trim() !== accountId ||
    Buffer.byteLength(accountId, "utf8") > 1536
  ) {
    throw new NexusAppError(
      "missing_signing_public_key",
      "approved account must be an exact bounded canonical I105 account",
    );
  }
  let address;
  try {
    address = AccountAddress.fromI105(accountId);
  } catch (error) {
    throw new NexusAppError(
      "missing_signing_public_key",
      "approved account must be a canonical single-key Ed25519 I105 account",
      error,
    );
  }
  const controller = address._controller;
  if (
    !controller ||
    controller.tag !== 0 ||
    controller.curve !== 1 ||
    controller.publicKey.length !== 32
  ) {
    throw new NexusAppError(
      "missing_signing_public_key",
      "approved account must be a canonical single-key Ed25519 I105 account",
    );
  }
  return Buffer.from(controller.publicKey);
}

function validateEd25519PublicKey(publicKey, context) {
  if (publicKey.length !== 32) {
    throw new NexusAppError(
      "invalid_signing_public_key",
      `${context} must be a 32-byte Ed25519 public key`,
    );
  }
  return publicKey;
}

function validateEd25519SignatureForPayload(publicKey, payloadBytes, signature) {
  let verified = false;
  try {
    verified = verifyEd25519(irohaPrehash(payloadBytes), signature, publicKey);
  } catch {
    verified = false;
  }
  if (!verified) {
    throw new NexusAppError(
      "invalid_signature",
      "Ed25519 signature does not verify for the signable payload",
    );
  }
}

function normalizeAlgorithm(algorithm) {
  if (algorithm === undefined || algorithm === null) {
    return ALGORITHM_ED25519;
  }
  if (typeof algorithm === "number") {
    if (Number.isInteger(algorithm) && algorithm === ALGORITHM_ED25519_TAG) {
      return ALGORITHM_ED25519;
    }
    throw new NexusAppError(
      "unsupported_signature_algorithm",
      `unsupported signature algorithm ${String(algorithm)}`,
    );
  }
  if (typeof algorithm !== "string") {
    throw new NexusAppError(
      "unsupported_signature_algorithm",
      `unsupported signature algorithm ${String(algorithm)}`,
    );
  }
  if (!algorithm || !/^[\x20-\x7e]+$/.test(algorithm)) {
    throw new NexusAppError(
      "unsupported_signature_algorithm",
      `unsupported signature algorithm ${algorithm}`,
    );
  }
  if (algorithm !== algorithm.trim()) {
    throw new NexusAppError(
      "unsupported_signature_algorithm",
      `unsupported signature algorithm ${algorithm}`,
    );
  }
  if (algorithm === ALGORITHM_ED25519 || algorithm === "0") {
    return ALGORITHM_ED25519;
  }
  throw new NexusAppError(
    "unsupported_signature_algorithm",
    `unsupported signature algorithm ${String(algorithm)}`,
  );
}

function nexusSignableErrorCode(error) {
  if (!(error instanceof BrowserTransactionCodecError)) return "invalid_payload";
  if (error.code === "payload_hash_mismatch") return "payload_hash_mismatch";
  if (error.code === "authority_mismatch") return "authority_mismatch";
  if (error.code === "invalid_hash") return "invalid_payload_hash";
  if (
    error.code === "invalid_public_key" ||
    (error.code === "invalid_bytes" && error.message.includes("signingPublicKey"))
  ) {
    return "invalid_signing_public_key";
  }
  if (
    error.code === "unsupported_algorithm" &&
    error.message.includes("signatureAlgorithm")
  ) {
    return "unsupported_signature_algorithm";
  }
  return "invalid_payload";
}

function validateNexusTransferSignable(signable, constraints = {}) {
  try {
    return validateBrowserTransferSignable(signable, constraints);
  } catch (error) {
    if (error instanceof NexusAppError) throw error;
    throw new NexusAppError(
      nexusSignableErrorCode(error),
      `invalid canonical Transfer::Asset signable: ${error?.message ?? String(error)}`,
      error,
    );
  }
}

function copyValidatedSignable(signable) {
  return Object.freeze({
    payloadBytes: Buffer.from(signable.payloadBytes),
    payloadHashHex: signable.payloadHashHex,
    authority: signable.authority,
    signingPublicKey: Buffer.from(signable.signingPublicKey),
    signatureAlgorithm: ALGORITHM_ED25519,
  });
}

function normalizeConnectSession(session) {
  session = snapshotDataFields(
    session,
    CONNECT_SESSION_FIELDS,
    "connect session",
    "invalid_connect_session",
  );
  const walletLaunchUri = normalizeAliasFamily(
    session,
    ["walletLaunchUri", "wallet_launch_uri", "wallet_uri"],
    "connect session",
    "invalid_connect_session",
  );
  const appLaunchUri = normalizeAliasFamily(
    session,
    ["appLaunchUri", "app_launch_uri", "app_uri"],
    "connect session",
    "invalid_connect_session",
  );
  const tokenApp = normalizeAliasFamily(
    session,
    ["tokenApp", "token_app"],
    "connect session",
    "invalid_connect_session",
  );
  const tokenWallet = normalizeAliasFamily(
    session,
    ["tokenWallet", "token_wallet"],
    "connect session",
    "invalid_connect_session",
  );
  const tokenManagement = normalizeAliasFamily(
    session,
    ["tokenManagement", "token_management"],
    "connect session",
    "invalid_connect_session",
  );
  const tokenRelay = normalizeAliasFamily(
    session,
    ["tokenRelay", "token_relay"],
    "connect session",
    "invalid_connect_session",
  );
  const approvedAccount = normalizeAliasFamily(
    session,
    ["approvedAccountId", "approvedAccount", "approved_account"],
    "connect session",
    "invalid_connect_session",
  );
  const signingPublicKey = normalizeByteAliases(
    session,
    ["signingPublicKey", "signing_public_key"],
    "connect session",
    "invalid_connect_session",
    { maxBytes: 32 },
  );
  return {
    sid: requireNonEmptyString(session.sid, "session.sid"),
    walletLaunchUri,
    appLaunchUri,
    tokenApp,
    tokenWallet,
    tokenManagement,
    tokenRelay,
    approvedAccountId: approvedAccount,
    approvedAccount,
    signingPublicKey,
    appSession: session.appSession ?? null,
    preview: session.preview ?? null,
  };
}

function isRawByteSignature(value) {
  return (
    Buffer.isBuffer(value) ||
    value instanceof Uint8Array ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer ||
    Array.isArray(value)
  );
}

function normalizeSignature(signature) {
  if (isRawByteSignature(signature) || !signature || typeof signature !== "object") {
    let bytes;
    try {
      bytes = toBuffer(signature, "signature", { maxBytes: 64 });
    } catch (error) {
      throw new NexusAppError("invalid_signature", error.message, error);
    }
    if (bytes.length !== 64) {
      throw new NexusAppError(
        "invalid_signature",
        `Ed25519 signature must be 64 bytes, got ${bytes.length}`,
      );
    }
    return { algorithm: ALGORITHM_ED25519, signature: bytes };
  }
  signature = snapshotDataFields(
    signature,
    SIGNATURE_FIELDS,
    "signature",
    "invalid_signature",
  );
  const algorithmDescriptor = Object.getOwnPropertyDescriptor(signature, "algorithm");
  const algDescriptor = Object.getOwnPropertyDescriptor(signature, "alg");
  const algorithm = normalizeAlgorithm(algorithmDescriptor?.value);
  const aliasAlgorithm = normalizeAlgorithm(algDescriptor?.value);
  if (algorithmDescriptor && algDescriptor && algorithm !== aliasAlgorithm) {
    throw new NexusAppError(
      "unsupported_signature_algorithm",
      "signature.algorithm conflicts with signature.alg",
    );
  }
  const bytes = normalizeByteAliases(
    signature,
    ["signature", "bytes", "payload"],
    "signature",
    "invalid_signature",
    { maxBytes: 64 },
  );
  if (bytes === null) {
    throw new NexusAppError(
      "invalid_signature",
      "signature must include signature, bytes, or payload",
    );
  }
  if (bytes.length !== 64) {
    throw new NexusAppError(
      "invalid_signature",
      `Ed25519 signature must be 64 bytes, got ${bytes.length}`,
    );
  }
  return { algorithm, signature: bytes };
}

function defaultBuildTransferPayload(input) {
  return browserTransactionCodec.buildTransferPayload(input);
}

function defaultFinalizeSignedTransaction(signable, signature, publicKey) {
  return finalizeBrowserSignedTransaction(signable, signature, publicKey);
}

function normalizePayloadBuildResult(result) {
  if (isBytesInput(result)) {
    return {
      payloadBytes: toBuffer(result, "payloadBytes", { maxBytes: MAX_PAYLOAD_BYTES }),
      assertedHashHex: null,
    };
  }
  if (result === null || typeof result !== "object" || Array.isArray(result)) {
    throw new NexusAppError(
      "invalid_payload",
      "transaction codec must return payload bytes or a payload result object",
    );
  }
  const payloadBytes = normalizeByteAliases(
    result,
    PAYLOAD_BYTE_ALIASES,
    "transaction codec result",
    "invalid_payload",
    { maxBytes: MAX_PAYLOAD_BYTES },
  );
  if (payloadBytes === null) {
    throw new NexusAppError(
      "invalid_payload",
      "transaction codec result must include payload bytes",
    );
  }
  const assertedHashHex = normalizeHashAliases(
    result,
    PAYLOAD_HASH_ALIASES,
    "transaction codec result",
    "invalid_payload_hash",
    "payload_hash_mismatch",
  );
  return { payloadBytes, assertedHashHex };
}

function canonicalSignedTransactionHashHex(signedTransaction) {
  try {
    return exactHashHex(
      browserSignedTransactionHashHex(signedTransaction),
      "canonical signed transaction hash",
      "invalid_transaction_hash",
    );
  } catch (error) {
    if (error instanceof NexusAppError) throw error;
    throw new NexusAppError(
      "invalid_signed_transaction",
      "signed transaction must be canonical version-1 single-signature Transfer::Asset bytes",
      error,
    );
  }
}

function normalizeFinalizedTransaction(result) {
  if (
    result === null ||
    typeof result !== "object" ||
    Array.isArray(result) ||
    isBytesInput(result)
  ) {
    throw new NexusAppError(
      "invalid_transaction_hash",
      "transaction finalizer must return signed bytes and an exact canonical hash",
    );
  }
  const signedTransaction = normalizeByteAliases(
    result,
    SIGNED_BYTE_ALIASES,
    "transaction finalizer result",
    "invalid_signed_transaction",
    { maxBytes: MAX_SIGNED_TRANSACTION_BYTES },
  );
  if (signedTransaction === null) {
    throw new NexusAppError(
      "invalid_signed_transaction",
      "transaction finalizer result must include signed transaction bytes",
    );
  }
  const assertedHashHex = normalizeHashAliases(
    result,
    TRANSACTION_HASH_ALIASES,
    "transaction finalizer result",
    "invalid_transaction_hash",
    "transaction_hash_mismatch",
  );
  if (assertedHashHex === null) {
    throw new NexusAppError(
      "invalid_transaction_hash",
      "transaction finalizer result must include an exact canonical transaction hash",
    );
  }
  const computedHashHex = canonicalSignedTransactionHashHex(signedTransaction);
  if (assertedHashHex !== computedHashHex) {
    throw new NexusAppError(
      "transaction_hash_mismatch",
      `transaction finalizer hash ${assertedHashHex} does not match canonical hash ${computedHashHex}`,
    );
  }
  return { signedTransaction, hashHex: computedHashHex };
}

function submissionHashHex(submission) {
  if (submission === null || typeof submission !== "object") return null;
  const direct = normalizeHashAliases(
    submission,
    SUBMISSION_HASH_ALIASES,
    "Torii submission",
    "invalid_transaction_hash",
    "transaction_hash_mismatch",
  );
  const payloadDescriptor = ownDataDescriptor(
    submission,
    "payload",
    "Torii submission",
    "invalid_transaction_hash",
  );
  const payload = payloadDescriptor?.value;
  const nested =
    payload !== null && typeof payload === "object" && !Array.isArray(payload)
      ? normalizeHashAliases(
          payload,
          SUBMISSION_HASH_ALIASES,
          "Torii submission.payload",
          "invalid_transaction_hash",
          "transaction_hash_mismatch",
        )
      : null;
  if (direct !== null && nested !== null && direct !== nested) {
    throw new NexusAppError(
      "transaction_hash_mismatch",
      "Torii submission hash aliases conflict with submission.payload hash aliases",
    );
  }
  return direct ?? nested;
}

function maybeInvoke(method, receiver, ...args) {
  if (typeof method === "function") {
    return Reflect.apply(method, receiver, args);
  }
  return undefined;
}

function normalizeToriiBaseUrl(value) {
  const literal = requireNonEmptyString(value, "config.toriiBaseUrl");
  let url;
  try {
    url = new URL(literal);
  } catch (error) {
    throw new TypeError("config.toriiBaseUrl must be an absolute HTTP(S) URL", {
      cause: error,
    });
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new TypeError("config.toriiBaseUrl must use HTTP or HTTPS");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new TypeError(
      "config.toriiBaseUrl must not contain credentials, a query, or a fragment",
    );
  }
  url.pathname = url.pathname
    .replace(/\/+$/u, "")
    .replace(/\/v1(?:\/explorer)?$/iu, "");
  return url.toString().replace(/\/$/u, "");
}

function normalizeNonNegativeInteger(value, fallback, context) {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${context} must be a non-negative safe integer`);
  }
  return value;
}

function normalizePositiveInteger(value, context) {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`${context} must be a positive safe integer`);
  }
  return value;
}

function normalizeStatusSet(value, fallback, context) {
  const source = value === undefined || value === null ? fallback : value;
  if (typeof source === "string") {
    throw new TypeError(`${context} must be an iterable of status strings`);
  }
  const result = new Set();
  let rawCount = 0;
  for (const status of source) {
    rawCount += 1;
    if (rawCount > 32) {
      throw new TypeError(`${context} must not contain more than 32 statuses`);
    }
    if (
      typeof status !== "string" ||
      status.length === 0 ||
      status.length > 64 ||
      status.trim() !== status ||
      !/^[\x20-\x7e]+$/u.test(status)
    ) {
      throw new TypeError(`${context} must contain exact printable status strings`);
    }
    result.add(status);
  }
  if (result.size === 0) {
    throw new TypeError(`${context} must not be empty`);
  }
  return result;
}

function normalizeStatusScope(value) {
  const scope = value === undefined ? "global" : value;
  if (scope !== "local" && scope !== "global") {
    throw new TypeError("transaction status scope must be local or global");
  }
  return scope;
}

function abortSignalState(signal) {
  if (abortSignalAbortedGetter === null) {
    throw new TypeError("transaction status signal must be an AbortSignal");
  }
  try {
    return {
      aborted: Reflect.apply(abortSignalAbortedGetter, signal, []),
      reason:
        abortSignalReasonGetter === null
          ? undefined
          : Reflect.apply(abortSignalReasonGetter, signal, []),
    };
  } catch (error) {
    throw new TypeError("transaction status signal must be an AbortSignal", {
      cause: error,
    });
  }
}

function abortReasonOrDefault(reason) {
  return reason === undefined ? new Error("operation aborted") : reason;
}

function addAbortSignalListener(signal, listener) {
  abortSignalState(signal);
  if (typeof abortSignalAddEventListener !== "function") {
    throw new TypeError("transaction status signal must be an AbortSignal");
  }
  Reflect.apply(abortSignalAddEventListener, signal, [
    "abort",
    listener,
    { once: true },
  ]);
}

function removeAbortSignalListener(signal, listener) {
  abortSignalState(signal);
  if (typeof abortSignalRemoveEventListener !== "function") {
    throw new TypeError("transaction status signal must be an AbortSignal");
  }
  Reflect.apply(abortSignalRemoveEventListener, signal, ["abort", listener]);
}

function throwIfAbortSignalAborted(signal) {
  if (signal === null) return;
  const { aborted, reason } = abortSignalState(signal);
  if (aborted) throw abortReasonOrDefault(reason);
}

function abortControllerWithReason(controller, reason) {
  if (reason === undefined) {
    controller.abort();
  } else {
    controller.abort(reason);
  }
}

function normalizeTransactionStatusOptions(options = {}) {
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("transaction status options must be an object");
  }
  if (Object.getOwnPropertyDescriptor(options, "successStatuses")) {
    throw new TypeError(
      "transaction status options contains unsupported field successStatuses",
    );
  }
  if (Object.getOwnPropertyDescriptor(options, "scope")) {
    throw new TypeError(
      "transaction status options contains unsupported field scope; finality waits are global",
    );
  }
  const intervalMs = normalizeNonNegativeInteger(
    options.intervalMs,
    DEFAULT_STATUS_POLL_INTERVAL_MS,
    "transaction status intervalMs",
  );
  const timeoutMs =
    options.timeoutMs === null
      ? null
      : normalizeNonNegativeInteger(
          options.timeoutMs,
          DEFAULT_STATUS_POLL_TIMEOUT_MS,
          "transaction status timeoutMs",
        );
  const maxAttempts =
    options.maxAttempts === undefined || options.maxAttempts === null
      ? null
      : normalizePositiveInteger(
          options.maxAttempts,
          "transaction status maxAttempts",
        );
  if (
    options.onStatus !== undefined &&
    options.onStatus !== null &&
    typeof options.onStatus !== "function"
  ) {
    throw new TypeError("transaction status onStatus must be a function");
  }
  const signal = options.signal ?? null;
  if (signal !== null) {
    if (typeof signal !== "object") {
      throw new TypeError("transaction status signal must be an AbortSignal");
    }
    abortSignalState(signal);
  }
  const failureStatuses = normalizeStatusSet(
    options.failureStatuses,
    DEFAULT_FAILURE_STATUSES,
    "transaction status failureStatuses",
  );
  if (failureStatuses.has("Applied")) {
    throw new TypeError("transaction status Applied cannot be configured as failure");
  }
  return Object.freeze({
    intervalMs,
    timeoutMs,
    maxAttempts,
    failureStatuses: Object.freeze([...failureStatuses]),
    onStatus: options.onStatus ?? null,
    signal,
  });
}

function throwIfStatusWaitAborted(statusOptions, shouldWait, context = {}) {
  if (!shouldWait || statusOptions.signal === null) return;
  const { aborted, reason } = abortSignalState(statusOptions.signal);
  if (!aborted) return;
  const cause = abortReasonOrDefault(reason);
  const submitted = context.submissionState === "submitted";
  throw new NexusAppError(
    submitted ? "status_wait_aborted" : "operation_aborted",
    submitted
      ? "transaction status wait was aborted after submission"
      : "transaction submission was aborted before dispatch",
    cause,
    {
      phase: submitted ? "status_wait" : "submission",
      submissionState: submitted ? "submitted" : "not_submitted",
      ...context,
    },
  );
}

function responseHeader(response, name) {
  return typeof response?.headers?.get === "function"
    ? response.headers.get(name)
    : null;
}

async function cancelResponseBody(response, signal = null) {
  try {
    const body = response?.body;
    const cancel = body?.cancel;
    if (typeof cancel === "function") {
      await awaitStatusWithGuards(Reflect.apply(cancel, body, []), {
        signal,
        timeoutMs: null,
      });
    }
  } catch {
    // Cancellation is best-effort; preserve the validation or transport result.
  }
}

async function readBoundedResponseText(response, context, signal = null) {
  const declaredLength = responseHeader(response, "content-length");
  if (declaredLength !== null && declaredLength !== undefined) {
    if (!/^(?:0|[1-9]\d*)$/u.test(declaredLength)) {
      await cancelResponseBody(response, signal);
      throw new Error(`${context} returned an invalid Content-Length`);
    }
    if (BigInt(declaredLength) > BigInt(MAX_TORII_RESPONSE_BYTES)) {
      await cancelResponseBody(response, signal);
      throw new Error(`${context} exceeds ${MAX_TORII_RESPONSE_BYTES} response bytes`);
    }
  }
  if (response?.body && typeof response.body.getReader === "function") {
    const reader = response.body.getReader();
    const chunks = [];
    let total = 0;
    try {
      while (true) {
        const { done, value } = await awaitStatusWithGuards(reader.read(), {
          signal,
          timeoutMs: null,
        });
        if (done) break;
        if (!(value instanceof Uint8Array)) {
          throw new Error(`${context} returned an invalid response stream`);
        }
        total += value.byteLength;
        if (total > MAX_TORII_RESPONSE_BYTES) {
          throw new Error(`${context} exceeds ${MAX_TORII_RESPONSE_BYTES} response bytes`);
        }
        chunks.push(value);
      }
    } catch (error) {
      try {
        await awaitStatusWithGuards(reader.cancel(error), {
          signal,
          timeoutMs: null,
        });
      } catch {
        // Preserve the original bounded-read failure.
      }
      throw error;
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch (error) {
      throw new Error(`${context} must return valid UTF-8`, { cause: error });
    }
  }
  if (typeof response?.arrayBuffer === "function") {
    const bytes = new Uint8Array(
      await awaitStatusWithGuards(response.arrayBuffer(), {
        signal,
        timeoutMs: null,
      }),
    );
    if (bytes.byteLength > MAX_TORII_RESPONSE_BYTES) {
      throw new Error(`${context} exceeds ${MAX_TORII_RESPONSE_BYTES} response bytes`);
    }
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch (error) {
      throw new Error(`${context} must return valid UTF-8`, { cause: error });
    }
  }
  if (typeof response?.text === "function") {
    const text = await awaitStatusWithGuards(response.text(), {
      signal,
      timeoutMs: null,
    });
    if (Buffer.byteLength(text, "utf8") > MAX_TORII_RESPONSE_BYTES) {
      throw new Error(`${context} exceeds ${MAX_TORII_RESPONSE_BYTES} response bytes`);
    }
    return text;
  }
  throw new Error(`${context} returned an unreadable response body`);
}

function parseJsonResponse(text, context) {
  if (text === "") return null;
  let payload;
  try {
    payload = JSON.parse(text);
  } catch (error) {
    throw new Error(`${context} must return JSON`, { cause: error });
  }
  if (payload !== null && (typeof payload !== "object" || Array.isArray(payload))) {
    throw new Error(`${context} must return a JSON object or null`);
  }
  return payload;
}

function classifyPipelineStatus(payload, expectedHash, context) {
  if (payload === null || typeof payload !== "object" || Array.isArray(payload)) {
    throw new TypeError(`${context} must be an object`);
  }
  if (payload.hash !== expectedHash) {
    throw new TypeError(`${context}.hash must match the requested transaction`);
  }
  if (payload.scope !== "global") {
    throw new TypeError(`${context}.scope must be global`);
  }
  if (typeof payload.summary !== "string") {
    throw new TypeError(`${context}.summary must be a string`);
  }
  if (
    payload.status === null ||
    typeof payload.status !== "object" ||
    Array.isArray(payload.status) ||
    typeof payload.status.kind !== "string" ||
    !PIPELINE_STATUS_KINDS.has(payload.status.kind)
  ) {
    throw new TypeError(`${context}.status.kind is missing or unsupported`);
  }
  const kind = payload.status.kind;
  const resolvedFrom = payload.resolved_from;
  if (!PIPELINE_STATUS_SOURCES.has(resolvedFrom)) {
    throw new TypeError(`${context}.resolved_from is unsupported`);
  }
  if (kind === "Applied") {
    if (
      !Number.isSafeInteger(payload.status.block_height) ||
      payload.status.block_height <= 0
    ) {
      throw new TypeError(
        `${context} Applied status must have a positive block height`,
      );
    }
    if (resolvedFrom !== "cache" && resolvedFrom !== "state") {
      throw new TypeError(
        `${context} Applied status must be cache- or state-resolved`,
      );
    }
  } else if (kind === "Rejected" || kind === "Expired") {
    if (resolvedFrom !== "cache" && resolvedFrom !== "state") {
      throw new TypeError(
        `${context} terminal failure must be cache- or state-resolved`,
      );
    }
  }
  return { kind, resolvedFrom };
}

function requireAuthoritativeAppliedStatus(payload, expectedHash, context) {
  const { kind, resolvedFrom } = classifyPipelineStatus(
    payload,
    expectedHash,
    context,
  );
  if (kind !== "Applied" || resolvedFrom !== "state") {
    throw new TypeError(
      `${context} must be state-resolved Applied finality`,
    );
  }
  return payload;
}

function delayWithSignal(milliseconds, signal) {
  if (signal === null) {
    return milliseconds === 0
      ? Promise.resolve()
      : new Promise((resolve) => setTimeout(resolve, milliseconds));
  }
  try {
    throwIfAbortSignalAborted(signal);
  } catch (error) {
    return Promise.reject(error);
  }
  if (milliseconds === 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    let settled = false;
    let timer = null;
    const cleanup = () => {
      if (timer !== null) clearTimeout(timer);
      try {
        removeAbortSignalListener(signal, onAbort);
      } catch {
        // Listener cleanup is best-effort after the delay has settled.
      }
    };
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      cleanup();
      callback(value);
    };
    const onAbort = () => {
      try {
        const { reason } = abortSignalState(signal);
        finish(reject, abortReasonOrDefault(reason));
      } catch (error) {
        finish(reject, error);
      }
    };
    try {
      addAbortSignalListener(signal, onAbort);
      throwIfAbortSignalAborted(signal);
    } catch (error) {
      finish(reject, error);
      return;
    }
    timer = setTimeout(() => finish(resolve), milliseconds);
  });
}

function awaitStatusWithGuards(value, statusOptions) {
  const { signal, timeoutMs } = statusOptions;
  if (signal === null && timeoutMs === null) return Promise.resolve(value);
  return new Promise((resolve, reject) => {
    let settled = false;
    let timeout = null;
    const deadline = timeoutMs === null ? null : Date.now() + timeoutMs;
    const cleanup = () => {
      if (timeout !== null) clearTimeout(timeout);
      if (signal !== null) {
        try {
          removeAbortSignalListener(signal, onAbort);
        } catch {
          // Listener cleanup is best-effort after the wait has settled.
        }
      }
    };
    const finish = (callback, result) => {
      if (settled) return;
      settled = true;
      cleanup();
      callback(result);
    };
    const onAbort = () => {
      try {
        const { reason } = abortSignalState(signal);
        finish(reject, abortReasonOrDefault(reason));
      } catch (error) {
        finish(reject, error);
      }
    };
    // Observe the underlying promise before any synchronous abort path can
    // settle this guard. A late rejection must never become unhandled merely
    // because cancellation won the race.
    Promise.resolve(value).then(
      (result) => finish(resolve, result),
      (error) => finish(reject, error),
    );
    if (signal !== null) {
      try {
        addAbortSignalListener(signal, onAbort);
        throwIfAbortSignalAborted(signal);
      } catch (error) {
        finish(reject, error);
        return;
      }
    }
    const scheduleTimeout = () => {
      if (deadline === null || settled) return;
      const remaining = Math.max(0, deadline - Date.now());
      timeout = setTimeout(() => {
        if (Date.now() < deadline) {
          scheduleTimeout();
          return;
        }
        finish(
          reject,
          new BrowserTransactionStatusTimeoutError(
            `transaction status did not settle within ${timeoutMs}ms`,
            null,
            null,
          ),
        );
      }, Math.min(remaining, 2_147_483_647));
    };
    if (timeoutMs === 0) {
      finish(
        reject,
        new BrowserTransactionStatusTimeoutError(
          "transaction status did not settle within 0ms",
          null,
          null,
        ),
      );
    } else if (deadline !== null) {
      scheduleTimeout();
    }
  });
}

class BrowserTransactionRejectedError extends Error {
  constructor(status, payload) {
    super(`transaction reached failure status ${status}`);
    this.name = "BrowserTransactionRejectedError";
    this.status = status;
    this.payload = payload;
  }
}

class BrowserTransactionStatusTimeoutError extends Error {
  constructor(message, attempts, payload) {
    super(message);
    this.name = "BrowserTransactionStatusTimeoutError";
    this.attempts = attempts;
    this.payload = payload;
  }
}

class BrowserToriiPipelineClient {
  constructor(baseUrl, fetchImpl) {
    this.baseUrl = normalizeToriiBaseUrl(baseUrl);
    this.fetchImpl = fetchImpl ?? globalThis.fetch?.bind(globalThis);
    if (typeof this.fetchImpl !== "function") {
      throw new TypeError("browser Nexus Torii submission requires fetch");
    }
  }

  async _open(path, init, externalSignal) {
    const controller = new AbortController();
    let externalListenerAttached = false;
    const detachExternalListener = () => {
      if (!externalListenerAttached) return;
      externalListenerAttached = false;
      try {
        removeAbortSignalListener(externalSignal, onAbort);
      } catch {
        // Listener cleanup is best-effort once the request is closed.
      }
    };
    const onAbort = () => {
      try {
        const { reason } = abortSignalState(externalSignal);
        abortControllerWithReason(controller, reason);
      } catch (error) {
        controller.abort(error);
      }
    };
    if (externalSignal !== undefined && externalSignal !== null) {
      const initial = abortSignalState(externalSignal);
      if (initial.aborted) {
        abortControllerWithReason(controller, initial.reason);
      } else {
        externalListenerAttached = true;
        try {
          addAbortSignalListener(externalSignal, onAbort);
          const registered = abortSignalState(externalSignal);
          if (registered.aborted) onAbort();
        } catch (error) {
          detachExternalListener();
          throw error;
        }
      }
    }
    const timeout = setTimeout(
      () => controller.abort(new Error("Torii request timed out")),
      DEFAULT_TORII_REQUEST_TIMEOUT_MS,
    );
    try {
      throwIfAbortSignalAborted(controller.signal);
      const response = await awaitStatusWithGuards(
        this.fetchImpl(new URL(`${this.baseUrl}${path}`).toString(), {
          credentials: "omit",
          redirect: "error",
          referrerPolicy: "no-referrer",
          ...init,
          signal: controller.signal,
        }),
        { signal: controller.signal, timeoutMs: null },
      );
      if (!response || !Number.isInteger(response.status)) {
        throw new Error("Torii fetch returned an invalid response");
      }
      return {
        response,
        signal: controller.signal,
        close: () => {
          clearTimeout(timeout);
          detachExternalListener();
          controller.abort(new Error("Torii response closed"));
        },
      };
    } catch (error) {
      clearTimeout(timeout);
      detachExternalListener();
      throw error;
    }
  }

  async submitTransaction(payload) {
    const signedTransaction = toBuffer(payload, "signed transaction", {
      maxBytes: MAX_SIGNED_TRANSACTION_BYTES,
    });
    if (signedTransaction.length < 2) {
      throw new TypeError("signed transaction must not be empty");
    }
    const request = await this._open(
      "/v1/pipeline/transactions",
      {
        method: "POST",
        cache: "no-store",
        headers: {
          Accept: "application/json, application/x-norito",
          "Content-Type": "application/x-norito",
        },
        body: Buffer.from(signedTransaction),
      },
    );
    try {
      const { response, signal } = request;
      if (![200, 201, 202, 204].includes(response.status)) {
        await cancelResponseBody(response, signal);
        throwIfAbortSignalAborted(signal);
        throw new Error(`Torii transaction submission returned HTTP ${response.status}`);
      }
      if (response.status === 204) {
        await cancelResponseBody(response, signal);
        throwIfAbortSignalAborted(signal);
        return null;
      }
      const contentType = responseHeader(response, "content-type")?.toLowerCase() ?? "";
      if (contentType.includes("application/x-norito")) {
        await cancelResponseBody(response, signal);
        throwIfAbortSignalAborted(signal);
        return null;
      }
      const text = await readBoundedResponseText(
        response,
        "Torii submission",
        signal,
      );
      if (text === "") return null;
      return parseJsonResponse(text, "Torii submission");
    } finally {
      request.close();
    }
  }

  async getTransactionStatus(hashHex, options = {}) {
    const hash = exactHashHex(
      hashHex,
      "transaction status hash",
      "invalid_transaction_hash",
    );
    if (options === null || typeof options !== "object" || Array.isArray(options)) {
      throw new TypeError("transaction status options must be an object");
    }
    const scope = normalizeStatusScope(options.scope);
    const query = new URLSearchParams({ hash, scope });
    const request = await this._open(
      `/v1/pipeline/transactions/status?${query.toString()}`,
      {
        method: "GET",
        cache: "no-store",
        headers: { Accept: "application/json" },
      },
      options.signal,
    );
    try {
      const { response, signal } = request;
      if (response.status === 404 || response.status === 204) {
        await cancelResponseBody(response, signal);
        throwIfAbortSignalAborted(signal);
        return null;
      }
      if (response.status !== 200 && response.status !== 202) {
        await cancelResponseBody(response, signal);
        throwIfAbortSignalAborted(signal);
        throw new Error(`Torii transaction status returned HTTP ${response.status}`);
      }
      const text = await readBoundedResponseText(
        response,
        "Torii transaction status",
        signal,
      );
      return parseJsonResponse(text, "Torii transaction status");
    } finally {
      request.close();
    }
  }

  async waitForTransactionStatus(hashHex, options = {}) {
    const normalized = normalizeTransactionStatusOptions(options);
    const failureStatuses = new Set(normalized.failureStatuses);
    const controller = new AbortController();
    let externalListenerAttached = false;
    let timeout = null;
    const startedAt = Date.now();
    const deadline =
      normalized.timeoutMs === null
        ? null
        : startedAt + normalized.timeoutMs;
    let attempts = 0;
    let lastPayload = null;
    const timeoutError = () =>
      new BrowserTransactionStatusTimeoutError(
        `transaction status did not settle within ${normalized.timeoutMs}ms`,
        attempts,
        lastPayload,
      );
    const throwIfDeadlineReached = () => {
      if (deadline !== null && Date.now() >= deadline) {
        throw timeoutError();
      }
    };
    const onExternalAbort = () => {
      try {
        const { reason } = abortSignalState(normalized.signal);
        abortControllerWithReason(controller, reason);
      } catch (error) {
        controller.abort(error);
      }
    };
    const cleanup = () => {
      if (timeout !== null) clearTimeout(timeout);
      if (externalListenerAttached) {
        externalListenerAttached = false;
        try {
          removeAbortSignalListener(normalized.signal, onExternalAbort);
        } catch {
          // Listener cleanup is best-effort after polling has settled.
        }
      }
    };
    const scheduleTimeout = () => {
      if (deadline === null || abortSignalState(controller.signal).aborted) return;
      const remaining = Math.max(0, deadline - Date.now());
      timeout = setTimeout(() => {
        if (Date.now() < deadline) {
          scheduleTimeout();
          return;
        }
        controller.abort(timeoutError());
      }, Math.min(remaining, 2_147_483_647));
    };
    try {
      if (normalized.signal !== null) {
        const initial = abortSignalState(normalized.signal);
        if (initial.aborted) {
          abortControllerWithReason(controller, initial.reason);
        } else {
          externalListenerAttached = true;
          try {
            addAbortSignalListener(normalized.signal, onExternalAbort);
            const registered = abortSignalState(normalized.signal);
            if (registered.aborted) onExternalAbort();
          } catch (error) {
            cleanup();
            throw error;
          }
        }
      }
      if (
        normalized.timeoutMs === 0 &&
        !abortSignalState(controller.signal).aborted
      ) {
        controller.abort(timeoutError());
      } else {
        scheduleTimeout();
      }
      while (true) {
        throwIfAbortSignalAborted(controller.signal);
        throwIfDeadlineReached();
        attempts += 1;
        lastPayload = await this.getTransactionStatus(hashHex, {
          scope: "global",
          signal: controller.signal,
        });
        throwIfAbortSignalAborted(controller.signal);
        throwIfDeadlineReached();
        const resolution =
          lastPayload === null
            ? null
            : classifyPipelineStatus(
                lastPayload,
                hashHex,
                "transaction status response",
              );
        const status = resolution?.kind ?? null;
        if (normalized.onStatus) {
          const callback = Promise.resolve().then(() =>
            Reflect.apply(normalized.onStatus, undefined, [
              status,
              lastPayload,
              attempts,
            ]),
          );
          await awaitStatusWithGuards(callback, {
            signal: controller.signal,
            timeoutMs: null,
          });
        }
        throwIfAbortSignalAborted(controller.signal);
        throwIfDeadlineReached();
        if (status === "Applied" && resolution.resolvedFrom === "state") {
          return lastPayload;
        }
        const isCanonicalTerminalFailure =
          status === "Rejected" || status === "Expired";
        const isStateTerminalFailure =
          isCanonicalTerminalFailure &&
          resolution?.resolvedFrom === "state";
        const isConfiguredStateFailure =
          failureStatuses.has(status) &&
          resolution?.resolvedFrom === "state";
        if (
          status !== null &&
          (isStateTerminalFailure ||
            isConfiguredStateFailure)
        ) {
          throw new BrowserTransactionRejectedError(status, lastPayload);
        }
        if (normalized.maxAttempts !== null && attempts >= normalized.maxAttempts) {
          throw new BrowserTransactionStatusTimeoutError(
            `transaction status did not settle after ${attempts} attempts`,
            attempts,
            lastPayload,
          );
        }
        await delayWithSignal(normalized.intervalMs, controller.signal);
      }
    } finally {
      cleanup();
    }
  }
}

export class NexusAppError extends Error {
  constructor(code, message, cause, context = {}) {
    super(message);
    this.name = "NexusAppError";
    Object.defineProperties(this, {
      code: {
        value: code,
        enumerable: true,
      },
      ...(arguments.length >= 3
        ? {
            cause: {
              value: cause,
              enumerable: true,
            },
          }
        : {}),
      phase: {
        value: context.phase ?? "validation",
        enumerable: true,
      },
      submissionState: {
        value: context.submissionState ?? "not_submitted",
        enumerable: true,
      },
      ...(context.signedTransactionHashHex === undefined
        ? {}
        : {
            signedTransactionHashHex: {
              value: context.signedTransactionHashHex,
              enumerable: true,
            },
          }),
      ...(context.submission === undefined
        ? {}
        : {
            submission: {
              value: context.submission,
              enumerable: true,
            },
          }),
      ...(context.status === undefined
        ? {}
        : {
            status: {
              value: context.status,
              enumerable: true,
            },
          }),
    });
  }
}

export class NexusAppClient {
  constructor(config = {}) {
    config = snapshotDataFields(
      config,
      CONFIG_FIELDS,
      "config",
      "invalid_config",
    );
    normalizeAliasFamily(
      config,
      ["authority", "accountId"],
      "config",
      "invalid_config",
    );
    this.config = config;
    this.connect = config.connectTransport ?? config.connect ?? null;
    this.transactionCodec = config.transactionCodec ?? null;
    this.toriiClient =
      config.toriiClient ??
      (config.toriiBaseUrl || config.baseUrl
        ? new BrowserToriiPipelineClient(
            config.toriiBaseUrl ?? config.baseUrl,
            config.fetchImpl,
          )
        : null);
  }

  async startConnect(options = {}) {
    options = snapshotDataFields(
      options,
      CONNECT_OPTION_FIELDS,
      "connect options",
      "invalid_connect_options",
    );
    let injected;
    try {
      const connect = this.connect;
      const startConnect = connect?.startConnect;
      injected = await maybeInvoke(
        startConnect,
        connect,
        options,
        this.config,
      );
    } catch (error) {
      throw new NexusAppError(
        "connect_start_failed",
        "Connect session registration failed",
        error,
      );
    }
    if (injected !== undefined) {
      return normalizeConnectSession(injected);
    }
    const baseUrl = requireNonEmptyString(
      this.config.connectBaseUrl ?? this.config.toriiBaseUrl ?? this.config.baseUrl,
      "config.baseUrl",
    );
    const chainId = requireNonEmptyString(
      options.chainId ?? this.config.chainId,
      "chainId",
    );
    const node = options.node ?? this.config.node ?? null;
    const preview = createConnectSessionPreview({
      chainId,
      node,
      appKeyPair: options.appKeyPair,
      nonce: options.nonce,
      protocol: options.protocol,
    });
    const registered = await registerConnectSession(
      baseUrl,
      preview.sidBase64Url,
      { node, fetchImpl: this.config.fetchImpl },
    );
    const normalizedRegistered = normalizeConnectSession(registered);
    return normalizeConnectSession({
      ...normalizedRegistered,
      preview,
      walletLaunchUri: normalizedRegistered.walletLaunchUri ?? preview.walletUri,
      appLaunchUri: normalizedRegistered.appLaunchUri ?? preview.appUri,
    });
  }

  async awaitApproval(session) {
    const normalized = normalizeConnectSession(session);
    let injected;
    try {
      const connect = this.connect;
      const awaitApproval = connect?.awaitApproval;
      injected = await maybeInvoke(
        awaitApproval,
        connect,
        normalized,
        this.config,
      );
    } catch (error) {
      throw new NexusAppError(
        "connect_approval_failed",
        "Connect wallet approval failed",
        error,
      );
    }
    let approved = injected;
    if (approved === undefined) {
      const appSession =
        normalized.appSession ??
        createConnectAppSession({
          baseUrl: this.config.connectBaseUrl ?? this.config.toriiBaseUrl ?? this.config.baseUrl,
          preview: normalized.preview,
          session: {
            sid: normalized.sid,
            token_app: normalized.tokenApp,
            token_relay: normalized.tokenRelay,
          },
          appMeta: this.config.appMeta ?? this.config.appMetadata ?? null,
          permissions: this.config.permissions ?? null,
          webSocketImpl: this.config.webSocketImpl,
          allowInsecure: this.config.allowInsecure,
        });
      normalized.appSession = appSession;
      let waitForApproval;
      try {
        waitForApproval = appSession?.waitForApproval;
      } catch (error) {
        throw new NexusAppError(
          "connect_session_unapproved",
          "Connect approval capability could not be resolved",
          error,
        );
      }
      if (typeof waitForApproval !== "function") {
        throw new NexusAppError(
          "connect_session_unapproved",
          "Connect app session cannot await wallet approval",
        );
      }
      let browserApproval;
      try {
        browserApproval = await Reflect.apply(waitForApproval, appSession, []);
      } catch (error) {
        throw new NexusAppError(
          "connect_approval_failed",
          "Connect wallet approval failed",
          error,
        );
      }
      approved = projectBrowserConnectApproval(browserApproval);
    }
    approved = snapshotDataFields(
      approved,
      APPROVAL_FIELDS,
      "wallet approval",
      "invalid_wallet_approval",
    );
    const accountIdRaw = normalizeAliasFamily(
      approved,
      ["accountId", "account_id"],
      "wallet approval",
      "invalid_wallet_approval",
    );
    if (
      typeof accountIdRaw !== "string" ||
      accountIdRaw.length === 0 ||
      accountIdRaw.trim() !== accountIdRaw
    ) {
      throw new NexusAppError(
        "approval_missing_account",
        "wallet approval did not include an exact account",
      );
    }
    const accountId = accountIdRaw;
    const configuredAuthority = normalizeAliasFamily(
      this.config,
      ["authority", "accountId"],
      "config",
      "invalid_config",
    );
    for (const [context, assertedAccount] of [
      ["configured authority", configuredAuthority],
      ["Connect session approved account", normalized.approvedAccountId],
    ]) {
      if (assertedAccount !== null && assertedAccount !== accountId) {
        throw new NexusAppError(
          "approval_account_mismatch",
          `${context} does not match the wallet approval account`,
        );
      }
    }
    const approvedSigningPublicKey = normalizeByteAliases(
      approved,
      ["signingPublicKey", "signing_public_key"],
      "wallet approval",
      "invalid_wallet_approval",
      { maxBytes: 32 },
    );
    const signingPublicKey = validateEd25519PublicKey(
      normalizeConsistentByteSources(
        [
          {
            value: this.config.signingPublicKey,
            context: "config.signingPublicKey",
          },
          {
            value: normalized.signingPublicKey,
            context: "connect session.signingPublicKey",
          },
          {
            value: approvedSigningPublicKey,
            context: "wallet approval.signingPublicKey",
          },
          {
            value: accountEd25519PublicKey(accountId),
            context: "wallet approval account controller",
          },
        ],
        "approval_signing_key_mismatch",
        { maxBytes: 32 },
      ),
      "approved signingPublicKey",
    );
    normalized.approvedAccountId = accountId;
    normalized.approvedAccount = accountId;
    normalized.signingPublicKey = Buffer.from(signingPublicKey);
    return {
      accountId,
      signingPublicKey: Buffer.from(signingPublicKey),
      session: normalized,
    };
  }

  buildTransferDraft(input = {}) {
    input = snapshotDataFields(
      input,
      TRANSFER_DRAFT_FIELDS,
      "transfer input",
      "invalid_transfer_input",
    );
    const inputAuthority = normalizeAliasFamily(
      input,
      ["authority", "accountId", "sourceAccountId"],
      "transfer input",
      "invalid_transfer_input",
    );
    const configuredAuthority = normalizeAliasFamily(
      this.config,
      ["authority", "accountId"],
      "config",
      "invalid_config",
    );
    const authority = inputAuthority ?? configuredAuthority;
    if (!authority) {
      throw new NexusAppError(
        "missing_authority",
        "transfer authority is required",
      );
    }
    const sourceAssetHoldingId = normalizeAliasFamily(
      input,
      ["sourceAssetHoldingId", "sourceAssetId", "assetId"],
      "transfer input",
      "invalid_transfer_input",
    );
    const destinationAccountId = normalizeAliasFamily(
      input,
      ["destinationAccountId", "destination", "to"],
      "transfer input",
      "invalid_transfer_input",
    );
    const chainId = requireNonEmptyString(
      input.chainId ?? this.config.chainId,
      "chainId",
    );
    const quantity = normalizeTransferQuantity(input.quantity);
    if (input.feePayment === undefined) {
      throw new NexusAppError(
        "missing_fee_payment",
        "transfer input requires a signature-bound feePayment intent",
      );
    }
    const payloadInput = {
      chainId,
      authority,
      sourceAssetHoldingId,
      quantity,
      destinationAccountId,
      metadata: input.metadata ?? null,
      creationTimeMs: input.creationTimeMs ?? null,
      ttlMs: input.ttlMs ?? null,
      nonce: input.nonce ?? null,
      feePayment: input.feePayment,
    };
    let transactionCodec;
    let payloadBuilder = null;
    let payloadHasher = null;
    try {
      transactionCodec = this.transactionCodec;
      if (transactionCodec !== null) {
        payloadBuilder = transactionCodec.buildTransferPayload ?? null;
        payloadHasher = transactionCodec.payloadHashHex ?? null;
      }
    } catch (error) {
      throw new NexusAppError(
        "invalid_transaction_codec",
        "transaction codec payload capabilities could not be resolved",
        error,
      );
    }
    if (transactionCodec !== null) {
      for (const [name, method] of [
        ["buildTransferPayload", payloadBuilder],
        ["payloadHashHex", payloadHasher],
      ]) {
        if (method !== null && typeof method !== "function") {
          throw new NexusAppError(
            "invalid_transaction_codec",
            `transaction codec ${name} must be a function`,
          );
        }
      }
    }
    let payloadResult;
    try {
      payloadResult =
        payloadBuilder === null
          ? defaultBuildTransferPayload(payloadInput)
          : Reflect.apply(payloadBuilder, transactionCodec, [payloadInput]);
    } catch (error) {
      throw new NexusAppError(
        "invalid_payload",
        "transaction payload construction failed",
        error,
      );
    }
    const { payloadBytes, assertedHashHex } = normalizePayloadBuildResult(payloadResult);
    if (payloadBytes.length === 0) {
      throw new NexusAppError("invalid_payload", "payloadBytes must not be empty");
    }
    const payloadHashHex = irohaPrehashHex(payloadBytes);
    if (assertedHashHex !== null && assertedHashHex !== payloadHashHex) {
      throw new NexusAppError(
        "payload_hash_mismatch",
        `transaction codec payload hash ${assertedHashHex} does not match canonical hash ${payloadHashHex}`,
      );
    }
    if (payloadHasher !== null) {
      let codecHash;
      try {
        codecHash = Reflect.apply(payloadHasher, transactionCodec, [
          Buffer.from(payloadBytes),
        ]);
      } catch (error) {
        throw new NexusAppError(
          "invalid_payload_hash",
          "transaction codec payload hasher failed",
          error,
        );
      }
      const codecHashHex = exactHashHex(
        codecHash,
        "transactionCodec.payloadHashHex result",
        "invalid_payload_hash",
      );
      if (codecHashHex !== payloadHashHex) {
        throw new NexusAppError(
          "payload_hash_mismatch",
          `transaction codec payloadHashHex ${codecHashHex} does not match canonical hash ${payloadHashHex}`,
        );
      }
    }
    const signingPublicKey =
      input.signingPublicKey ??
      this.config.signingPublicKey ??
      accountEd25519PublicKey(authority);
    return {
      input: { ...payloadInput, signingPublicKey },
      signable: {
        payloadBytes,
        payloadHashHex,
        authority,
        signingPublicKey: signingPublicKey
          ? validateEd25519PublicKey(
              toBuffer(signingPublicKey, "signingPublicKey", { maxBytes: 32 }),
              "signingPublicKey",
            )
          : null,
        signatureAlgorithm: ALGORITHM_ED25519,
      },
    };
  }

  async requestSignature(session, signable) {
    const normalizedSession = normalizeConnectSession(session);
    const configuredAuthority = normalizeAliasFamily(
      this.config,
      ["authority", "accountId"],
      "config",
      "invalid_config",
    );
    const approvedAccount = normalizedSession.approvedAccountId;
    if (
      approvedAccount !== null &&
      configuredAuthority !== null &&
      approvedAccount !== configuredAuthority
    ) {
      throw new NexusAppError(
        "approval_account_mismatch",
        "Connect session approved account conflicts with the configured authority",
      );
    }
    const expectedSigningPublicKey = normalizeConsistentByteSources(
      [
        {
          value: normalizedSession.signingPublicKey,
          context: "connect session.signingPublicKey",
        },
        {
          value: this.config.signingPublicKey,
          context: "config.signingPublicKey",
        },
        {
          value: approvedAccount === null ? null : accountEd25519PublicKey(approvedAccount),
          context: "Connect session approved account controller",
        },
      ],
      "approval_signing_key_mismatch",
      { maxBytes: 32 },
    );
    const canonicalSignable = validateNexusTransferSignable(signable, {
      authority: approvedAccount ?? configuredAuthority,
      signingPublicKey: expectedSigningPublicKey,
    });
    let injected;
    try {
      const connect = this.connect;
      const requestSignature = connect?.requestSignature;
      injected = await maybeInvoke(
        requestSignature,
        connect,
        normalizedSession,
        copyValidatedSignable(canonicalSignable),
        this.config,
      );
    } catch (error) {
      throw new NexusAppError(
        "wallet_signature_failed",
        "wallet signature request failed",
        error,
      );
    }
    if (injected !== undefined) {
      return normalizeSignature(injected);
    }
    const appSession = normalizedSession.appSession;
    let signTransaction;
    try {
      signTransaction = appSession?.signTransaction;
    } catch (error) {
      throw new NexusAppError(
        "connect_session_unapproved",
        "Connect signing capability could not be resolved",
        error,
      );
    }
    if (typeof signTransaction !== "function") {
      throw new NexusAppError(
        "connect_session_unapproved",
        "Connect app session is not approved or cannot sign transactions",
      );
    }
    let signature;
    try {
      signature = await Reflect.apply(signTransaction, appSession, [
        Buffer.from(canonicalSignable.payloadBytes),
      ]);
    } catch (error) {
      throw new NexusAppError(
        "wallet_signature_failed",
        "Connect wallet signature request failed",
        error,
      );
    }
    return normalizeSignature({ algorithm: ALGORITHM_ED25519, signature });
  }

  async finalizeAndSubmit(signable, signature, options = {}) {
    options = snapshotDataFields(
      options,
      FINALIZE_OPTION_FIELDS,
      "finalize options",
      "invalid_finalize_options",
    );
    if (options.wait !== undefined && typeof options.wait !== "boolean") {
      throw new NexusAppError(
        "invalid_finalize_options",
        "finalize options.wait must be a boolean",
      );
    }
    const shouldWait = options.wait !== false;
    let statusOptions = null;
    if (shouldWait) {
      try {
        statusOptions = normalizeTransactionStatusOptions(options);
      } catch (error) {
        throw new NexusAppError(
          "invalid_finalize_options",
          "transaction status options are invalid",
          error,
        );
      }
    } else {
      for (const field of STATUS_WAIT_OPTION_FIELDS) {
        if (ownDataDescriptor(
          options,
          field,
          "finalize options",
          "invalid_finalize_options",
        )) {
          throw new NexusAppError(
            "invalid_finalize_options",
            `finalize options.${field} is not allowed when wait is false`,
          );
        }
      }
    }
    throwIfStatusWaitAborted(statusOptions, shouldWait);
    signable = snapshotDataFields(
      signable,
      SIGNABLE_FIELDS,
      "signable",
      "invalid_payload",
    );
    normalizeAlgorithm(signable.signatureAlgorithm);
    const normalizedSignature = normalizeSignature(signature);
    const signingPublicKey = normalizeConsistentByteSources(
      [
        {
          value: signable.signingPublicKey,
          context: "signable.signingPublicKey",
        },
        {
          value: options.signingPublicKey,
          context: "finalize options.signingPublicKey",
        },
        {
          value: this.config.signingPublicKey,
          context: "config.signingPublicKey",
        },
      ],
      "invalid_signing_public_key",
      { maxBytes: 32 },
    );
    if (!signingPublicKey) {
      throw new NexusAppError(
        "missing_signing_public_key",
        "signing public key is required to finalize a wallet-signed transaction",
      );
    }
    const publicKey = validateEd25519PublicKey(
      signingPublicKey,
      "signingPublicKey",
    );
    const canonicalSignable = validateNexusTransferSignable(
      {
        ...signable,
        signingPublicKey: publicKey,
      },
      { signingPublicKey: publicKey },
    );
    const { payloadBytes, payloadHashHex } = canonicalSignable;
    validateEd25519SignatureForPayload(
      publicKey,
      payloadBytes,
      normalizedSignature.signature,
    );
    const expectedFinalized = finalizeBrowserSignedTransaction(
      canonicalSignable,
      normalizedSignature,
      publicKey,
    );
    let transactionCodec;
    let transactionFinalizer = null;
    try {
      transactionCodec = this.transactionCodec;
      if (transactionCodec !== null) {
        transactionFinalizer =
          transactionCodec.finalizeSignedTransaction ?? null;
      }
    } catch (error) {
      throw new NexusAppError(
        "invalid_transaction_codec",
        "transaction codec finalizer could not be resolved",
        error,
        { phase: "finalization" },
      );
    }
    if (transactionCodec !== null) {
      throwIfStatusWaitAborted(statusOptions, shouldWait);
      if (
        transactionFinalizer !== null &&
        typeof transactionFinalizer !== "function"
      ) {
        throw new NexusAppError(
          "invalid_transaction_codec",
          "transaction codec finalizer must be a function",
          undefined,
          { phase: "finalization" },
        );
      }
    }
    let finalizedResult;
    try {
      finalizedResult =
        transactionFinalizer === null
          ? defaultFinalizeSignedTransaction(
              canonicalSignable,
              normalizedSignature,
              publicKey,
            )
          : Reflect.apply(transactionFinalizer, transactionCodec, [
              canonicalSignable,
              normalizedSignature,
              publicKey,
            ]);
    } catch (error) {
      throw new NexusAppError(
        "invalid_signed_transaction",
        "transaction finalization failed",
        error,
        { phase: "finalization" },
      );
    }
    throwIfStatusWaitAborted(statusOptions, shouldWait);
    let finalized;
    try {
      finalized = normalizeFinalizedTransaction(finalizedResult);
    } catch (error) {
      let code = "invalid_signed_transaction";
      try {
        if (error instanceof NexusAppError) code = error.code;
      } catch {
        // Hostile thrown values remain opaque causes.
      }
      throw new NexusAppError(
        code,
        "transaction finalizer returned an invalid result",
        error,
        { phase: "finalization" },
      );
    }
    if (!finalized.signedTransaction.equals(expectedFinalized.signedTransaction)) {
      throw new NexusAppError(
        "signed_transaction_mismatch",
        "transaction finalizer bytes do not match the independently finalized canonical transfer",
        undefined,
        { phase: "finalization" },
      );
    }
    if (finalized.hashHex !== expectedFinalized.hashHex) {
      throw new NexusAppError(
        "transaction_hash_mismatch",
        "transaction finalizer hash does not match the independently finalized canonical transfer",
        undefined,
        { phase: "finalization" },
      );
    }
    throwIfStatusWaitAborted(statusOptions, shouldWait);
    const toriiClient = options.toriiClient ?? this.toriiClient;
    let submitTransaction;
    let waitForTransactionStatus = null;
    try {
      submitTransaction = toriiClient?.submitTransaction;
    } catch (error) {
      throw new NexusAppError(
        "torii_client_unavailable",
        "Torii client capabilities could not be resolved",
        error,
        { phase: "submission" },
      );
    }
    if (typeof submitTransaction !== "function") {
      throw new NexusAppError(
        "torii_client_unavailable",
        "Torii client is required to submit the signed transaction",
        undefined,
        { phase: "submission" },
      );
    }
    throwIfStatusWaitAborted(statusOptions, shouldWait);
    if (shouldWait) {
      try {
        waitForTransactionStatus = toriiClient?.waitForTransactionStatus;
      } catch (error) {
        throw new NexusAppError(
          "status_wait_unavailable",
          "Torii status-wait capability could not be resolved",
          error,
          { phase: "submission" },
        );
      }
    }
    if (shouldWait && typeof waitForTransactionStatus !== "function") {
      throw new NexusAppError(
        "status_wait_unavailable",
        "wait-enabled submission requires a Torii status waiter",
        undefined,
        { phase: "submission" },
      );
    }
    throwIfStatusWaitAborted(statusOptions, shouldWait);
    let submission;
    try {
      submission = await Reflect.apply(submitTransaction, toriiClient, [
        finalized.signedTransaction,
      ]);
    } catch (error) {
      throw new NexusAppError(
        "submission_outcome_unknown",
        "Torii transaction submission did not return a confirmed outcome",
        error,
        {
          phase: "submission",
          submissionState: "unknown",
          signedTransactionHashHex: finalized.hashHex,
        },
      );
    }
    const submittedContext = {
      phase: "submission",
      submissionState: "submitted",
      signedTransactionHashHex: finalized.hashHex,
      submission,
    };
    let submittedHashHex;
    try {
      submittedHashHex = submissionHashHex(submission);
    } catch (error) {
      throw new NexusAppError(
        "invalid_submission_response",
        "Torii returned an invalid transaction submission receipt",
        error,
        submittedContext,
      );
    }
    if (submittedHashHex && submittedHashHex !== finalized.hashHex) {
      throw new NexusAppError(
        "invalid_submission_response",
        `Torii returned transaction hash ${submittedHashHex} but local hash is ${finalized.hashHex}`,
        undefined,
        submittedContext,
      );
    }
    let status = null;
    if (shouldWait) {
      const waitContext = {
        phase: "status_wait",
        submissionState: "submitted",
        signedTransactionHashHex: finalized.hashHex,
        submission,
      };
      throwIfStatusWaitAborted(statusOptions, true, waitContext);
      try {
        const pendingStatus = Reflect.apply(waitForTransactionStatus, toriiClient, [
          finalized.hashHex,
          statusOptions,
        ]);
        status = await awaitStatusWithGuards(
          pendingStatus,
          statusOptions,
        );
      } catch (error) {
        throwIfStatusWaitAborted(statusOptions, true, waitContext);
        let code = "status_wait_failed";
        let message = "failed while waiting for Torii pipeline status";
        let observedStatus;
        try {
          if (error instanceof BrowserTransactionRejectedError) {
            code = "transaction_rejected";
            message = "transaction reached a terminal failure status";
            observedStatus = error.status;
          } else if (error instanceof BrowserTransactionStatusTimeoutError) {
            code = "status_wait_timeout";
            message = "transaction status wait timed out";
            observedStatus = error.payload;
          }
        } catch {
          // Hostile rejection values remain opaque causes.
        }
        throw new NexusAppError(
          code,
          message,
          error,
          {
            ...waitContext,
            ...(observedStatus === undefined
              ? {}
              : { status: observedStatus }),
          },
        );
      }
      try {
        requireAuthoritativeAppliedStatus(
          status,
          finalized.hashHex,
          "Torii status waiter response",
        );
      } catch (error) {
        throw new NexusAppError(
          "status_wait_non_applied",
          "Torii status waiter returned before exact Applied execution finality",
          error,
          {
            ...waitContext,
            status,
          },
        );
      }
    }
    return {
      signedTransaction: finalized.signedTransaction,
      signedTransactionHashHex: finalized.hashHex,
      submission,
      status,
    };
  }

  async transferWithWallet(session, input, options = {}) {
    input = snapshotDataFields(
      input,
      TRANSFER_DRAFT_FIELDS,
      "transfer input",
      "invalid_transfer_input",
    );
    const normalizedSession = session ? normalizeConnectSession(session) : {};
    const approvedAccount =
      normalizedSession.approvedAccountId ??
      normalizedSession.approvedAccount ??
      null;
    const inputAuthority = normalizeAliasFamily(
      input,
      ["authority", "accountId", "sourceAccountId"],
      "transfer input",
      "invalid_transfer_input",
    );
    const configuredAuthority = normalizeAliasFamily(
      this.config,
      ["authority", "accountId"],
      "config",
      "invalid_config",
    );
    if (approvedAccount && inputAuthority && approvedAccount !== inputAuthority) {
      throw new NexusAppError(
        "approval_account_mismatch",
        "transfer authority does not match the approved wallet account",
      );
    }
    if (
      approvedAccount &&
      configuredAuthority &&
      approvedAccount !== configuredAuthority
    ) {
      throw new NexusAppError(
        "approval_account_mismatch",
        "configured authority does not match the approved wallet account",
      );
    }
    const authority = inputAuthority ?? approvedAccount ?? configuredAuthority;
    const signingPublicKey = normalizeConsistentByteSources(
      [
        {
          value: input.signingPublicKey,
          context: "transfer input.signingPublicKey",
        },
        {
          value: normalizedSession.signingPublicKey,
          context: "connect session.signingPublicKey",
        },
        {
          value: this.config.signingPublicKey,
          context: "config.signingPublicKey",
        },
      ],
      "approval_signing_key_mismatch",
      { maxBytes: 32 },
    );
    const draft = this.buildTransferDraft({
      ...input,
      authority,
      signingPublicKey,
    });
    const signature = await this.requestSignature(normalizedSession, draft.signable);
    return this.finalizeAndSubmit(draft.signable, signature, options);
  }
}

export {
  ALGORITHM_ED25519 as NexusSignatureAlgorithmEd25519,
  irohaPrehashHex as nexusPayloadHashHex,
  validateBrowserTransferSignable,
};
