import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import { NumericV1, NumericV1Error } from "./numericV1.js";

const SORAFS_XOR_QUANTITY_MAX_TEXT_LENGTH = 155;

function isPlainObject(value) {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError("bytes must be a Buffer, ArrayBuffer, or typed array");
}

function requireSorafsNativeFunction(
  functionName,
  capability,
  nativeRuntime = defaultNativeRuntime,
) {
  const binding = resolveNativeRuntimeBinding(nativeRuntime);
  if (!binding || typeof binding[functionName] !== "function") {
    throw new Error(
      `SoraFS ${capability} requires the native iroha_js_host module. Run \`npm run build:native\` before using these helpers.`,
    );
  }
  return binding;
}

function requireSorafsNativeBinding(nativeRuntime = defaultNativeRuntime) {
  return requireSorafsNativeFunction(
    "sorafsDecodeReplicationOrder",
    "decoding",
    nativeRuntime,
  );
}

export const SORAFS_ORDERBOOK_PAYLOAD_KINDS = Object.freeze({
  ORDER_REQUEST: "order-request",
  ORDER_CANCEL: "order-cancel",
  TRADE_EVENT: "trade-event",
  SETTLEMENT_CHANNEL: "settlement-channel",
  SETTLEMENT_RECEIPT: "settlement-receipt",
});

/** Canonical maximum byte length for a V1 orderbook owner account. */
export const ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 = 256;

const ORDERBOOK_PAYLOAD_KIND_SET = new Set(
  Object.values(SORAFS_ORDERBOOK_PAYLOAD_KINDS),
);
const ORDERBOOK_SIDE_SET = new Set(["bid", "ask"]);
const ORDERBOOK_TIER_SET = new Set(["hot", "warm", "archive"]);
const ORDERBOOK_CANCEL_REASON_SET = new Set([
  "owner-requested",
  "expired",
  "governance",
  "replaced",
]);

function requireCanonicalOrderbookSelector(value, allowed, label) {
  if (typeof value !== "string" || !allowed.has(value)) {
    throw new TypeError(`${label} is not a canonical V1 selector`);
  }
  return value;
}

function normalizeOrderbookPayloadKind(kind) {
  if (typeof kind !== "string") {
    throw new TypeError("kind must be a string");
  }
  if (!ORDERBOOK_PAYLOAD_KIND_SET.has(kind)) {
    throw new TypeError(`unsupported SoraFS orderbook payload kind: ${kind}`);
  }
  return kind;
}

export const SORAFS_PDP_PAYLOAD_KINDS = Object.freeze({
  COMMITMENT: "commitment",
  CHALLENGE: "challenge",
  PROOF: "proof",
});

/** Canonical heterogeneous fixture-bundle payload selectors. */
export const SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS = Object.freeze({
  PROVIDER_ADVERT: "provider-advert",
  PROVIDER_ADMISSION_ENVELOPE: "provider-admission-envelope",
  REPLICATION_ORDER: "replication-order",
  POR_CHALLENGE: "por-challenge",
  POR_PROOF: "por-proof",
  POTR_RECEIPT: "potr-receipt",
  REPAIR_EVIDENCE: "repair-evidence",
  REPAIR_REPORT: "repair-report",
  REPAIR_TASK_RECORD: "repair-task-record",
  REPAIR_SLASH_PROPOSAL: "repair-slash-proposal",
  REPAIR_TASK_EVENT: "repair-task-event",
  ORDERBOOK_ORDER_REQUEST: "orderbook-order-request",
  ORDERBOOK_ORDER_CANCEL: "orderbook-order-cancel",
  ORDERBOOK_TRADE_EVENT: "orderbook-trade-event",
  ORDERBOOK_SETTLEMENT_CHANNEL: "orderbook-settlement-channel",
  ORDERBOOK_SETTLEMENT_RECEIPT: "orderbook-settlement-receipt",
  PDP_COMMITMENT: "pdp-commitment",
  PDP_CHALLENGE: "pdp-challenge",
  PDP_PROOF: "pdp-proof",
});

/** Maximum payload count accepted by one fixture-bundle validation call. */
export const SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1 = 64;

/** Maximum ordered block count accepted by governance DAG head validation. */
export const SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1 = 64;
/** Exact byte length of a canonical Governance DAG block CID. */
export const SORAFS_GOVERNANCE_DAG_CID_BYTES_V1 = 32;
/** Maximum aggregate bytes accepted by one governance DAG reference call. */
export const SORAFS_REFERENCE_MAX_INPUT_BYTES_V1 = 67_108_864;
/** Maximum UTF-8 bytes accepted by one governance DAG diagnostic label. */
export const SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 = 1_024;

const PDP_PAYLOAD_KIND_SET = new Set(Object.values(SORAFS_PDP_PAYLOAD_KINDS));
const FIXTURE_BUNDLE_PAYLOAD_KIND_SET = new Set(
  Object.values(SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS),
);

function normalizePdpPayloadKind(kind) {
  if (typeof kind !== "string") {
    throw new TypeError("kind must be a string");
  }
  if (!PDP_PAYLOAD_KIND_SET.has(kind)) {
    throw new TypeError(`unsupported SoraFS PDP payload kind: ${kind}`);
  }
  return kind;
}

function normalizeFixtureBundlePayloadKind(kind) {
  if (typeof kind !== "string") {
    throw new TypeError("payload kind must be a string");
  }
  if (!FIXTURE_BUNDLE_PAYLOAD_KIND_SET.has(kind)) {
    throw new TypeError(`unsupported SoraFS fixture-bundle payload kind: ${kind}`);
  }
  return kind;
}

function readPayloadField(object, ...names) {
  for (const name of names) {
    if (Object.prototype.hasOwnProperty.call(object, name)) {
      return object[name];
    }
  }
  return undefined;
}

function rejectUnexpectedFields(object, allowedFields, context) {
  const allowed = new Set(allowedFields);
  const unexpected = Object.keys(object).filter((field) => !allowed.has(field));
  if (unexpected.length > 0) {
    throw new TypeError(
      `${context} contains unsupported fields: ${unexpected.join(", ")}`,
    );
  }
}

function formatAssignment(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      providerIdHex: "",
      sliceGiB: 0,
      lane: null,
    };
  }
  const providerValue = raw.providerIdHex;
  const providerIdHex =
    typeof providerValue === "string" ? providerValue : "";
  const sliceValue = raw.sliceGib;
  const laneValue = raw.lane;
  return {
    providerIdHex,
    sliceGiB: typeof sliceValue === "number" ? sliceValue : Number(sliceValue ?? 0),
    lane:
      typeof laneValue === "string"
        ? laneValue
        : laneValue === null || laneValue === undefined
        ? null
        : String(laneValue),
  };
}

function formatMetadata(raw) {
  if (!raw || typeof raw !== "object") {
    return { key: "", value: "" };
  }
  return {
    key: typeof raw.key === "string" ? raw.key : String(raw.key ?? ""),
    value: typeof raw.value === "string" ? raw.value : String(raw.value ?? ""),
  };
}

function formatSla(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      ingestDeadlineSecs: 0,
      minAvailabilityPercentMilli: 0,
      minPorSuccessPercentMilli: 0,
    };
  }
  const ingest = raw.ingestDeadlineSecs;
  const availability = raw.minAvailabilityPercentMilli;
  const por = raw.minPorSuccessPercentMilli;
  return {
    ingestDeadlineSecs: Number(ingest ?? 0),
    minAvailabilityPercentMilli: Number(availability ?? 0),
    minPorSuccessPercentMilli: Number(por ?? 0),
  };
}

export class SorafsGatewayFetchError extends Error {
  /**
   * @param {Record<string, any>} [payload]
   * @param {Error | null} [original]
   */
  constructor(payload = {}, original = null) {
    const message =
      (payload && typeof payload.message === "string" && payload.message) ||
      original?.message ||
      "SoraFS gateway fetch failed";
    super(message);
    this.name = "SorafsGatewayFetchError";
    this.kind = typeof payload.kind === "string" ? payload.kind : "multi_source";
    this.code = typeof payload.code === "string" ? payload.code : "unknown";
    this.retryable = Boolean(payload.retryable);
    this.chunkIndex =
      typeof payload.chunkIndex === "number" ? payload.chunkIndex : null;
    this.attempts =
      typeof payload.attempts === "number" ? payload.attempts : null;
    this.lastError =
      payload && typeof payload.lastError === "object" ? payload.lastError : null;
    this.providers = Array.isArray(payload.providers) ? payload.providers : null;
    this.details =
      payload && typeof payload.details === "object" ? payload.details : null;
    this.observerError =
      payload && typeof payload.observerError === "string"
        ? payload.observerError
        : null;
    this.original = original ?? null;
    this.payload = payload || {};
  }
}

/**
 * Decode a Norito-encoded replication order into a structured object.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {{
 *   schemaVersion: number,
 *   orderIdHex: string,
 *   manifestCidHex: string,
 *   manifestCidBase64: string,
 *   manifestDigestHex: string,
 *   chunkingProfile: string,
 *   targetReplicas: number,
 *   assignments: Array<{ providerIdHex: string, sliceGiB: number, lane: string | null }>,
 *   issuedAtUnix: number,
 *   deadlineAtUnix: number,
 *   sla: { ingestDeadlineSecs: number, minAvailabilityPercentMilli: number, minPorSuccessPercentMilli: number },
 *   metadata: Array<{ key: string, value: string }>
 * }}
 */
export function decodeReplicationOrder(bytes) {
  const buffer = toBuffer(bytes);
  const binding = requireSorafsNativeBinding();
  if (typeof binding.sorafsDecodeReplicationOrder !== "function") {
    throw new Error("Native binding does not expose sorafsDecodeReplicationOrder");
  }
  const payload = binding.sorafsDecodeReplicationOrder(buffer);
  const assignments = Array.isArray(payload.assignments)
    ? payload.assignments.map((entry) => formatAssignment(entry))
    : [];
  const metadata = Array.isArray(payload.metadata)
    ? payload.metadata.map((entry) => formatMetadata(entry))
    : [];
  const schemaVersion = Number(payload.schemaVersion ?? 0);
  const orderIdValue = payload.orderIdHex;
  const orderIdHex = typeof orderIdValue === "string" ? orderIdValue : "";
  const manifestCidHexValue = payload.manifestCidHex;
  if (
    typeof manifestCidHexValue !== "string" ||
    !/^[0-9a-f]{72}$/.test(manifestCidHexValue)
  ) {
    throw new Error(
      "Native replication order must expose a canonical 36-byte manifestCidHex",
    );
  }
  const manifestCidHex = manifestCidHexValue;
  const manifestCidBase64Value = payload.manifestCidBase64;
  const manifestCidBase64 =
    typeof manifestCidBase64Value === "string" ? manifestCidBase64Value : "";
  const manifestDigestValue = payload.manifestDigestHex;
  const manifestDigestHex =
    typeof manifestDigestValue === "string" ? manifestDigestValue : "";
  const chunkingProfileValue = payload.chunkingProfile;
  const chunkingProfile =
    typeof chunkingProfileValue === "string" ? chunkingProfileValue : "";
  const targetReplicas = payload.targetReplicas ?? 0;
  const issuedAtUnix = payload.issuedAtUnix ?? 0;
  const deadlineAtUnix = payload.deadlineAtUnix ?? 0;
  const sla = formatSla(payload.sla);
  return {
    schemaVersion,
    orderIdHex,
    manifestCidHex,
    manifestCidBase64,
    manifestDigestHex,
    chunkingProfile,
    targetReplicas: Number(targetReplicas ?? 0),
    assignments,
    issuedAtUnix: Number(issuedAtUnix ?? 0),
    deadlineAtUnix: Number(deadlineAtUnix ?? 0),
    sla,
    metadata,
  };
}

/**
 * Validate a Norito-encoded orderbook payload with the Rust reference validator.
 * @param {string} kind
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateOrderbookPayload(kind, bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(options, ["label", "generatedAtUnix"], "options");
  const canonicalKind = normalizeOrderbookPayloadKind(kind);
  const buffer = toBuffer(bytes);
  const label =
    typeof options.label === "string" && options.label.trim() !== ""
      ? options.label.trim()
      : `sdk:sorafs.orderbook.${canonicalKind}`;
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidateOrderbookPayloadJson",
    "orderbook validation",
  );
  const payload = binding.sorafsValidateOrderbookPayloadJson(
    canonicalKind,
    buffer,
    label,
    generatedAtUnix,
  );
  if (typeof payload !== "string") {
    throw new Error("Native binding returned a non-string orderbook validation payload");
  }
  const outcome = JSON.parse(payload);
  if (!isPlainObject(outcome)) {
    throw new Error("Native binding returned an invalid orderbook validation outcome");
  }
  return outcome;
}

/**
 * Validate a bare appeal-finance `CancelAssetLock` V1 archive with the Rust
 * reference validator.
 *
 * A successful diagnostic outcome does not itself authorize settlement.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateAppealFinanceCancelAssetLock(bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(options, ["label", "generatedAtUnix"], "options");
  const buffer = toBuffer(bytes);
  const label =
    typeof options.label === "string" && options.label.trim() !== ""
      ? options.label.trim()
      : "sdk:sorafs.appeal_finance.cancel_asset_lock";
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidateAppealFinanceCancelAssetLockJson",
    "appeal-finance CancelAssetLock validation",
  );
  const payload = binding.sorafsValidateAppealFinanceCancelAssetLockJson(
    buffer,
    label,
    generatedAtUnix,
  );
  if (typeof payload !== "string") {
    throw new Error(
      "Native binding returned a non-string appeal-finance validation payload",
    );
  }
  const outcome = JSON.parse(payload);
  if (!isPlainObject(outcome)) {
    throw new Error(
      "Native binding returned an invalid appeal-finance validation outcome",
    );
  }
  return outcome;
}

/**
 * Sign a Norito-encoded mutable orderbook payload with an Ed25519 private key.
 * @param {string} kind
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function signOrderbookPayload(kind, bytes, privateKey) {
  const canonicalKind = normalizeOrderbookPayloadKind(kind);
  const buffer = toBuffer(bytes);
  const key = toBuffer(privateKey);
  const binding = requireSorafsNativeFunction(
    "sorafsSignOrderbookPayload",
    "orderbook signing",
  );
  const signed = binding.sorafsSignOrderbookPayload(canonicalKind, buffer, key);
  if (Buffer.isBuffer(signed)) {
    return signed;
  }
  if (ArrayBuffer.isView(signed) || signed instanceof ArrayBuffer) {
    return toBuffer(signed);
  }
  throw new Error("Native binding returned a non-buffer orderbook signing payload");
}

/**
 * Derive the canonical V1 orderbook order id from owner-account bytes and nonce.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} ownerAccount
 * @param {number | bigint | string} nonce
 * @returns {Buffer}
 */
export function deriveOrderbookOrderId(ownerAccount, nonce) {
  const owner = orderbookOwnerAccount(ownerAccount);
  const canonicalNonce = decimalIntegerString(nonce, "nonce", { positive: true });
  const binding = requireSorafsNativeFunction(
    "sorafsDeriveOrderbookOrderId",
    "orderbook order id derivation",
  );
  const orderId = requireSignedBuilderBuffer(
    binding.sorafsDeriveOrderbookOrderId(owner, canonicalNonce),
    "orderbook order id derivation",
  );
  if (orderId.length !== 32) {
    throw new Error("Native binding returned a non-32-byte orderbook order id");
  }
  return orderId;
}

function requireSignedBuilderBuffer(value, context) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer) {
    return toBuffer(value);
  }
  throw new Error(`Native binding returned a non-buffer ${context} payload`);
}

/**
 * Build and sign canonical Norito `OrderRequestV1` bytes from field values.
 * @param {Record<string, any>} fields
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function buildSignedOrderbookOrderRequest(fields, privateKey) {
  if (!isPlainObject(fields)) {
    throw new TypeError("fields must be an object");
  }
  rejectRetiredOrderbookFields(fields, [
    "order_id",
    "price_per_gib",
    "quantity_gib",
    "remaining_gib",
    "owner_account",
    "provider_id",
    "expiry_unix",
    "maker_fee_bps",
    "taker_fee_bps",
    "pricePerGibMicroXor",
    "price_per_gib_micro_xor",
    "pricePerGibMicro",
    "price_per_gib_micro",
  ]);
  const side = requireCanonicalOrderbookSelector(
    requiredField(fields, "side", "side"),
    ORDERBOOK_SIDE_SET,
    "side",
  );
  const tier = requireCanonicalOrderbookSelector(
    requiredField(fields, "tier", "tier"),
    ORDERBOOK_TIER_SET,
    "tier",
  );
  const quantityGib = decimalIntegerString(
    requiredField(fields, "quantityGib", "quantityGib"),
    "quantityGib",
    { positive: true },
  );
  const remainingValue = optionalField(fields, "remainingGib");
  const ownerAccount = orderbookOwnerAccountField(
    fields,
    "ownerAccount",
    "ownerAccount",
  );
  const providerValue = optionalField(fields, "providerId");
  const providerId =
    providerValue === undefined ? Buffer.alloc(0) : toBuffer(providerValue);
  if (side === "bid") {
    if (providerId.length !== 0) {
      throw new RangeError("providerId must be absent or empty for bid orders");
    }
  } else {
    if (providerId.length !== 32) {
      throw new RangeError("providerId must be exactly 32 bytes for ask orders");
    }
    if (providerId.equals(Buffer.alloc(32))) {
      throw new RangeError("providerId must not be all zero");
    }
  }
  const nonce = decimalIntegerString(
    requiredField(fields, "nonce", "nonce"),
    "nonce",
    { positive: true },
  );
  const pricePerGib = canonicalXorQuantityString(
    requiredField(fields, "pricePerGib", "pricePerGib"),
    "pricePerGib",
    { positive: true },
  );
  const orderId = deriveOrderbookOrderId(ownerAccount, nonce);
  const suppliedOrderId = optionalField(fields, "orderId");
  if (suppliedOrderId !== undefined) {
    const supplied = toBuffer(suppliedOrderId);
    if (supplied.length !== 32 || !supplied.equals(orderId)) {
      throw new RangeError(
        `orderId must equal the canonical owner-and-nonce derivation ${orderId.toString("hex")}`,
      );
    }
  }
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookOrderRequest",
    "orderbook order request builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookOrderRequest(
      orderId,
      side,
      tier,
      pricePerGib,
      quantityGib,
      remainingValue === undefined
        ? undefined
        : decimalIntegerString(remainingValue, "remainingGib", { positive: true }),
      ownerAccount,
      providerId,
      decimalIntegerString(
        requiredField(fields, "expiryUnix", "expiryUnix"),
        "expiryUnix",
        { positive: true },
      ),
      nonce,
      normalizeOrderbookFeeBps(
        requiredField(fields, "makerFeeBps", "makerFeeBps"),
        "makerFeeBps",
      ),
      normalizeOrderbookFeeBps(
        requiredField(fields, "takerFeeBps", "takerFeeBps"),
        "takerFeeBps",
      ),
      toBuffer(privateKey),
    ),
    "orderbook order request builder",
  );
}

/**
 * Build and sign canonical Norito `OrderCancelV1` bytes from field values.
 * @param {Record<string, any>} fields
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function buildSignedOrderbookOrderCancel(fields, privateKey) {
  if (!isPlainObject(fields)) {
    throw new TypeError("fields must be an object");
  }
  rejectRetiredOrderbookFields(fields, ["order_id", "owner_account"]);
  const reason = requireCanonicalOrderbookSelector(
    requiredField(fields, "reason", "reason"),
    ORDERBOOK_CANCEL_REASON_SET,
    "reason",
  );
  const ownerAccount = orderbookOwnerAccountField(
    fields,
    "ownerAccount",
    "ownerAccount",
  );
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookOrderCancel",
    "orderbook cancel builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookOrderCancel(
      fixedBytesField(fields, "orderId", "orderId"),
      ownerAccount,
      reason,
      decimalIntegerString(
        requiredField(fields, "nonce", "nonce"),
        "nonce",
        { positive: true },
      ),
      toBuffer(privateKey),
    ),
    "orderbook cancel builder",
  );
}

/**
 * Build and sign canonical Norito `SettlementReceiptV1` bytes from field values.
 * @param {Record<string, any>} fields
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey
 * @returns {Buffer}
 */
export function buildSignedOrderbookSettlementReceipt(fields, privateKey) {
  if (!isPlainObject(fields)) {
    throw new TypeError("fields must be an object");
  }
  rejectRetiredOrderbookFields(fields, [
    "receipt_id",
    "channel_id",
    "trade_id",
    "range_start",
    "range_end",
    "chunk_hash",
    "bytes_delivered",
    "xor_debited",
    "provider_credit",
    "fee_amount",
    "issued_at_unix",
    "xorDebitedMicroXor",
    "xor_debited_micro_xor",
    "xorDebitedMicro",
    "xor_debited_micro",
    "providerCreditMicroXor",
    "provider_credit_micro_xor",
    "providerCreditMicro",
    "provider_credit_micro",
    "feeAmountMicroXor",
    "fee_amount_micro_xor",
    "feeAmountMicro",
    "fee_amount_micro",
  ]);
  const receiptId = fixedBytesField(fields, "receiptId", "receiptId");
  const channelId = fixedBytesField(fields, "channelId", "channelId");
  const tradeId = fixedBytesField(fields, "tradeId", "tradeId");
  const rangeStart = decimalIntegerString(
    requiredField(fields, "rangeStart", "rangeStart"),
    "rangeStart",
  );
  const rangeEnd = decimalIntegerString(
    requiredField(fields, "rangeEnd", "rangeEnd"),
    "rangeEnd",
    { positive: true },
  );
  const chunkHash = fixedBytesField(fields, "chunkHash", "chunkHash");
  const bytesDelivered = decimalIntegerString(
    requiredField(fields, "bytesDelivered", "bytesDelivered"),
    "bytesDelivered",
    { positive: true },
  );
  const xorDebited = canonicalXorQuantityString(
    requiredField(fields, "xorDebited", "xorDebited"),
    "xorDebited",
    { positive: true },
  );
  const providerCredit = canonicalXorQuantityString(
    requiredField(fields, "providerCredit", "providerCredit"),
    "providerCredit",
  );
  const feeAmount = canonicalXorQuantityString(
    requiredField(fields, "feeAmount", "feeAmount"),
    "feeAmount",
  );
  const issuedAtUnix = decimalIntegerString(
    requiredField(fields, "issuedAtUnix", "issuedAtUnix"),
    "issuedAtUnix",
    { positive: true },
  );
  const privateKeyBytes = toBuffer(privateKey);
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookSettlementReceipt",
    "orderbook settlement receipt builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookSettlementReceipt(
      receiptId,
      channelId,
      tradeId,
      rangeStart,
      rangeEnd,
      chunkHash,
      bytesDelivered,
      xorDebited,
      providerCredit,
      feeAmount,
      issuedAtUnix,
      privateKeyBytes,
    ),
    "orderbook settlement receipt builder",
  );
}

function parseReferenceOutcomePayload(payload, capability) {
  if (typeof payload !== "string") {
    throw new Error(`Native binding returned a non-string ${capability} payload`);
  }
  const outcome = JSON.parse(payload);
  if (!isPlainObject(outcome)) {
    throw new Error(`Native binding returned an invalid ${capability} outcome`);
  }
  return outcome;
}

function referenceLabel(options, name, fallback) {
  const value = options[name];
  if (typeof value === "string" && value.trim() !== "") {
    return value.trim();
  }
  return fallback;
}

function governanceReferenceLabel(value, fallback, field) {
  const label = value === undefined || value === null ? fallback : value;
  if (typeof label !== "string") {
    throw new TypeError(`${field} must be a string`);
  }
  for (let index = 0; index < label.length; index += 1) {
    const codeUnit = label.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = label.charCodeAt(index + 1);
      if (
        index + 1 >= label.length ||
        next < 0xdc00 ||
        next > 0xdfff
      ) {
        throw new TypeError(`${field} must be valid Unicode text`);
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new TypeError(`${field} must be valid Unicode text`);
    }
  }
  if (label.length === 0 || label.trim().length === 0) {
    throw new TypeError(`${field} must not be blank`);
  }
  if (label.trim() !== label) {
    throw new TypeError(`${field} must not contain surrounding whitespace`);
  }
  if (/[\u0000-\u001f\u007f-\u009f]/u.test(label)) {
    throw new TypeError(`${field} must not contain control characters`);
  }
  if (Buffer.byteLength(label, "utf8") > SORAFS_REFERENCE_MAX_LABEL_BYTES_V1) {
    throw new TypeError(
      `${field} must be at most ${SORAFS_REFERENCE_MAX_LABEL_BYTES_V1} UTF-8 bytes`,
    );
  }
  return label;
}

function governanceReferenceAggregateBytes(context, ...sizes) {
  let total = 0;
  for (const size of sizes) {
    total += size;
    if (total > SORAFS_REFERENCE_MAX_INPUT_BYTES_V1) {
      throw new TypeError(
        `${context} inputs exceed ${SORAFS_REFERENCE_MAX_INPUT_BYTES_V1} aggregate bytes`,
      );
    }
  }
}

function normalizeGovernanceDagBlockInput(value, index) {
  if (!isPlainObject(value)) {
    throw new TypeError(`blocks[${index}] must be an object`);
  }
  rejectUnexpectedFields(value, ["bytes", "label"], `blocks[${index}]`);
  if (value.bytes === undefined) {
    throw new TypeError(`blocks[${index}].bytes is required`);
  }
  return {
    bytes: toBuffer(value.bytes),
    label: governanceReferenceLabel(
      value.label,
      `governance-dag-block-${index}.to`,
      `blocks[${index}].label`,
    ),
  };
}

/**
 * Diagnose one Norito-encoded PDP payload with the Rust reference validator.
 * A successful result is structural-only and never authorizes production acceptance.
 * @param {string} kind
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validatePdpPayload(kind, bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(options, ["label", "generatedAtUnix"], "options");
  const canonicalKind = normalizePdpPayloadKind(kind);
  const buffer = toBuffer(bytes);
  const label = referenceLabel(
    options,
    "label",
    `sdk:sorafs.pdp.${canonicalKind}`,
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpPayloadJson",
    "PDP validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpPayloadJson(
      canonicalKind,
      buffer,
      label,
      generatedAtUnix,
    ),
    "PDP validation",
  );
}

/**
 * Diagnose PDP commitment/challenge binding with the Rust reference validator.
 * A successful result does not evaluate provider admission or Merkle witnesses.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} commitmentBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {{ commitmentLabel?: string, challengeLabel?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validatePdpCommitmentChallenge(
  commitmentBytes,
  challengeBytes,
  options = {},
) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    ["commitmentLabel", "challengeLabel", "generatedAtUnix"],
    "options",
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpCommitmentChallengeJson",
    "PDP commitment/challenge validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpCommitmentChallengeJson(
      toBuffer(commitmentBytes),
      referenceLabel(
        options,
        "commitmentLabel",
        "sdk:sorafs.pdp.commitment",
      ),
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        "challengeLabel",
        "sdk:sorafs.pdp.challenge",
      ),
      generatedAtUnix,
    ),
    "PDP commitment/challenge validation",
  );
}

/**
 * Diagnose PDP challenge/proof binding with the Rust reference validator.
 * A successful result does not evaluate provider admission or commitment roots.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} proofBytes
 * @param {{ challengeLabel?: string, proofLabel?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validatePdpChallengeProof(
  challengeBytes,
  proofBytes,
  options = {},
) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    ["challengeLabel", "proofLabel", "generatedAtUnix"],
    "options",
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpChallengeProofJson",
    "PDP challenge/proof validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpChallengeProofJson(
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        "challengeLabel",
        "sdk:sorafs.pdp.challenge",
      ),
      toBuffer(proofBytes),
      referenceLabel(
        options,
        "proofLabel",
        "sdk:sorafs.pdp.proof",
      ),
      generatedAtUnix,
    ),
    "PDP challenge/proof validation",
  );
}

/**
 * Exhaustively diagnose PDP bytes, signature, coverage, and both Merkle roots.
 * A successful result still does not evaluate governed provider admission and therefore
 * returns `SFS-PDP-DIAG-000` with `production_acceptance=false`.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} commitmentBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} proofBytes
 * @param {{ commitmentLabel?: string, challengeLabel?: string, proofLabel?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validatePdpBundle(
  commitmentBytes,
  challengeBytes,
  proofBytes,
  options = {},
) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    ["commitmentLabel", "challengeLabel", "proofLabel", "generatedAtUnix"],
    "options",
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpBundleJson",
    "PDP bundle validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpBundleJson(
      toBuffer(commitmentBytes),
      referenceLabel(
        options,
        "commitmentLabel",
        "sdk:sorafs.pdp.commitment",
      ),
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        "challengeLabel",
        "sdk:sorafs.pdp.challenge",
      ),
      toBuffer(proofBytes),
      referenceLabel(
        options,
        "proofLabel",
        "sdk:sorafs.pdp.proof",
      ),
      generatedAtUnix,
    ),
    "PDP bundle validation",
  );
}

/**
 * Validate a bounded heterogeneous fixture bundle and its canonical cross-links.
 * @param {Array<{ kind: string, bytes: ArrayBufferView | ArrayBuffer | Buffer, label?: string }>} payloads
 * @param {{ nowUnix?: number | bigint, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateFixtureBundle(payloads, options = {}) {
  if (!Array.isArray(payloads)) {
    throw new TypeError("payloads must be an array");
  }
  if (
    payloads.length === 0 ||
    payloads.length > SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1
  ) {
    throw new TypeError(
      `payloads must contain 1..=${SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1} entries`,
    );
  }
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(options, ["nowUnix", "generatedAtUnix"], "options");
  let aggregateBytes = 0;
  const normalizedPayloads = payloads.map((payload, index) => {
    if (!isPlainObject(payload)) {
      throw new TypeError(`payloads[${index}] must be an object`);
    }
    rejectUnexpectedFields(
      payload,
      ["kind", "bytes", "label"],
      `payloads[${index}]`,
    );
    const kind = normalizeFixtureBundlePayloadKind(payload.kind);
    if (payload.bytes === undefined) {
      throw new TypeError(`payloads[${index}].bytes is required`);
    }
    const bytes = Buffer.from(toBuffer(payload.bytes));
    const label = governanceReferenceLabel(
      payload.label,
      `${kind}.to`,
      `payloads[${index}].label`,
    );
    aggregateBytes += bytes.length + Buffer.byteLength(label, "utf8");
    if (aggregateBytes > SORAFS_REFERENCE_MAX_INPUT_BYTES_V1) {
      throw new TypeError(
        `fixture-bundle inputs exceed ${SORAFS_REFERENCE_MAX_INPUT_BYTES_V1} aggregate bytes`,
      );
    }
    return { kind, bytes, label };
  });
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const nowUnix = normalizeReferenceUnix(
    options.nowUnix ?? generatedAtUnix,
    "options.nowUnix",
  );
  const binding = requireSorafsNativeFunction(
    "sorafsValidateFixtureBundleJson",
    "fixture-bundle validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidateFixtureBundleJson(
      normalizedPayloads,
      nowUnix,
      generatedAtUnix,
    ),
    "fixture-bundle validation",
  );
}

/**
 * Validate one canonical GovernanceLogNodeV1 and bind it to its expected CID.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, expectedNodeCid: ArrayBufferView | ArrayBuffer | Buffer, generatedAtUnix?: number | bigint }} options
 * @returns {Record<string, any>}
 */
export function validateGovernanceLogNode(bytes, options) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    ["label", "expectedNodeCid", "generatedAtUnix"],
    "options",
  );
  const payload = Buffer.from(toBuffer(bytes));
  const label = governanceReferenceLabel(
    options.label,
    "governance.to",
    "options.label",
  );
  const expectedValue = options.expectedNodeCid;
  if (expectedValue === undefined || expectedValue === null) {
    throw new TypeError("options.expectedNodeCid is required");
  }
  const expectedNodeCid = Buffer.from(toBuffer(expectedValue));
  if (expectedNodeCid.length !== SORAFS_GOVERNANCE_DAG_CID_BYTES_V1) {
    throw new TypeError(
      `options.expectedNodeCid must contain exactly ${SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes`,
    );
  }
  if (
    payload.length +
      Buffer.byteLength(label, "utf8") +
      expectedNodeCid.length >
    SORAFS_REFERENCE_MAX_INPUT_BYTES_V1
  ) {
    throw new TypeError(
      `governance log-node validation inputs exceed ${SORAFS_REFERENCE_MAX_INPUT_BYTES_V1} aggregate bytes`,
    );
  }
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidateGovernanceLogNodeJson",
    "governance log-node validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidateGovernanceLogNodeJson(
      payload,
      label,
      expectedNodeCid,
      generatedAtUnix,
    ),
    "governance log-node validation",
  );
}

/**
 * Validate one canonical GovernanceDagBlockV1 with the Rust reference validator.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, expectedBlockCid?: ArrayBufferView | ArrayBuffer | Buffer, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateGovernanceDagBlock(bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    ["label", "expectedBlockCid", "generatedAtUnix"],
    "options",
  );
  const payload = toBuffer(bytes);
  const label = governanceReferenceLabel(
    options.label,
    "governance-dag-block.to",
    "options.label",
  );
  const expectedValue = options.expectedBlockCid;
  const expectedBlockCid =
    expectedValue === undefined || expectedValue === null
      ? undefined
      : toBuffer(expectedValue);
  if (
    expectedBlockCid !== undefined &&
    expectedBlockCid.length !== SORAFS_GOVERNANCE_DAG_CID_BYTES_V1
  ) {
    throw new TypeError(
      `options.expectedBlockCid must contain exactly ${SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes`,
    );
  }
  governanceReferenceAggregateBytes(
    "governance DAG block validation",
    payload.length,
    Buffer.byteLength(label, "utf8"),
    expectedBlockCid?.length ?? 0,
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidateGovernanceDagBlockJson",
    "governance DAG block validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidateGovernanceDagBlockJson(
      payload,
      label,
      expectedBlockCid,
      generatedAtUnix,
    ),
    "governance DAG block validation",
  );
}

/**
 * Validate a signed GovernanceDagHeadV1 against an ordered contiguous block tail.
 * Histories up to 64 blocks use the full root-to-head sequence; longer histories
 * use the newest checkpoint-anchored tail.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} headBytes
 * @param {Array<{ bytes: ArrayBufferView | ArrayBuffer | Buffer, label?: string }>} blocks
 * @param {{ headLabel?: string, generatedAtUnix?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateGovernanceDagHeadChain(
  headBytes,
  blocks,
  options = {},
) {
  if (!Array.isArray(blocks)) {
    throw new TypeError("blocks must be an array");
  }
  if (
    blocks.length === 0 ||
    blocks.length > SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1
  ) {
    throw new TypeError(
      `blocks must contain 1..=${SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1} entries`,
    );
  }
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(options, ["headLabel", "generatedAtUnix"], "options");
  const head = toBuffer(headBytes);
  const headLabel = governanceReferenceLabel(
    options.headLabel,
    "governance-dag-head.to",
    "options.headLabel",
  );
  const normalizedBlocks = blocks.map(normalizeGovernanceDagBlockInput);
  governanceReferenceAggregateBytes(
    "governance DAG head-chain validation",
    head.length,
    Buffer.byteLength(headLabel, "utf8"),
    ...normalizedBlocks.flatMap((block) => [
      block.bytes.length,
      Buffer.byteLength(block.label, "utf8"),
    ]),
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(options.generatedAtUnix);
  const binding = requireSorafsNativeFunction(
    "sorafsValidateGovernanceDagHeadChainJson",
    "governance DAG head-chain validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidateGovernanceDagHeadChainJson(
      head,
      headLabel,
      normalizedBlocks,
      generatedAtUnix,
    ),
    "governance DAG head-chain validation",
  );
}

function assertNonEmptyString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value.trim();
}

function normalizeHex32(value, label) {
  const trimmed = assertNonEmptyString(value, label);
  const body = trimmed.startsWith("0x") || trimmed.startsWith("0X") ? trimmed.slice(2) : trimmed;
  if (body.length !== 64 || !/^[0-9a-fA-F]+$/.test(body)) {
    throw new TypeError(`${label} must be a 32-byte hex string`);
  }
  return body.toLowerCase();
}

function normalizeBase64Payload(value, label) {
  const compact = assertNonEmptyString(value, label).replace(/\s+/g, "");
  if (compact.length === 0) {
    throw new TypeError(`${label} must be a non-empty base64 string`);
  }
  let padded = compact;
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      throw new TypeError(`${label} must be a valid base64 string`);
    }
    if (compact.length % 4 !== 0) {
      throw new TypeError(`${label} must be a valid base64 string`);
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(compact) || compact.length % 4 === 1) {
      throw new TypeError(`${label} must be a valid base64 string`);
    }
    const padLength = (4 - (compact.length % 4)) % 4;
    padded = compact + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.length === 0 || decoded.toString("base64") !== padded) {
    throw new TypeError(`${label} must be a valid base64 string`);
  }
  return decoded.toString("base64");
}

function requireCanonicalGatewayHex32(value, label) {
  if (
    typeof value !== "string" ||
    value.length !== 64 ||
    !/^[0-9a-f]{64}$/.test(value) ||
    /^0{64}$/.test(value)
  ) {
    throw new TypeError(`${label} must be non-zero canonical lowercase 32-byte hex`);
  }
  return value;
}

function requireCanonicalGatewayToken(value, label) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 4 * 1024
  ) {
    throw new TypeError(`${label} must be exact canonical standard base64`);
  }
  let canonical;
  try {
    canonical = normalizeBase64Payload(value, label);
  } catch (error) {
    throw new TypeError(`${label} must be exact canonical standard base64`, {
      cause: error,
    });
  }
  if (canonical !== value) {
    throw new TypeError(`${label} must be exact canonical standard base64`);
  }
  if (Buffer.from(value, "base64").length > 2 * 1024) {
    throw new TypeError(`${label} must be exact canonical standard base64`);
  }
  return value;
}

function parseCanonicalIpv4(host) {
  if (!/^[0-9.]+$/.test(host)) {
    return null;
  }
  const parts = host.split(".");
  if (
    parts.length !== 4 ||
    parts.some(
      (part) =>
        part.length === 0 ||
        (part.length > 1 && part.startsWith("0")) ||
        !/^\d+$/.test(part) ||
        Number(part) > 255,
    )
  ) {
    return [];
  }
  return parts.map(Number);
}

function parseCanonicalIpv6(host) {
  const literal = host.startsWith("[") && host.endsWith("]") ? host.slice(1, -1) : host;
  if (!literal.includes(":") || literal.includes("%") || literal.includes(".")) {
    return null;
  }
  const halves = literal.split("::");
  if (halves.length > 2) {
    return [];
  }
  const parseHalf = (half) => {
    if (half === "") return [];
    const parts = half.split(":");
    if (parts.some((part) => !/^[0-9a-fA-F]{1,4}$/.test(part))) return null;
    return parts.map((part) => Number.parseInt(part, 16));
  };
  const left = parseHalf(halves[0]);
  const right = parseHalf(halves.length === 2 ? halves[1] : "");
  if (left === null || right === null) return [];
  if (halves.length === 1) return left.length === 8 ? left : [];
  const missing = 8 - left.length - right.length;
  if (missing < 1) return [];
  return [...left, ...Array(missing).fill(0), ...right];
}

function isPublicGatewayLiteral(host) {
  const ipv4 = parseCanonicalIpv4(host);
  if (ipv4 !== null) {
    if (ipv4.length !== 4) return false;
    const [first, second, third, fourth] = ipv4;
    return !(
      first === 0 ||
      first === 10 ||
      first === 127 ||
      first >= 224 ||
      (first === 100 && second >= 64 && second <= 127) ||
      (first === 169 && second === 254) ||
      (first === 172 && second >= 16 && second <= 31) ||
      (first === 192 && second === 0 && third === 0) ||
      (first === 192 && second === 0 && third === 2) ||
      (first === 192 && second === 88 && third === 99) ||
      (first === 192 && second === 168) ||
      (first === 198 && (second === 18 || second === 19)) ||
      (first === 198 && second === 51 && third === 100) ||
      (first === 203 && second === 0 && third === 113) ||
      (first === 255 && second === 255 && third === 255 && fourth === 255)
    );
  }
  const ipv6 = parseCanonicalIpv6(host);
  if (ipv6 === null) return true;
  if (ipv6.length !== 8) return false;
  const [first, second] = ipv6;
  const globalUnicast = (first & 0xe000) === 0x2000;
  const documentation =
    (first === 0x2001 && second === 0x0db8) ||
    (first === 0x3fff && (second & 0xf000) === 0);
  const specialPurpose = first === 0x2001 && second <= 0x01ff;
  return globalUnicast && !documentation && !specialPurpose && first !== 0x2002;
}

function requireCanonicalGatewayUrl(value, label, expectedPath) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 2_048 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value)
  ) {
    throw new TypeError(`${label} must be an exact canonical HTTPS URL`);
  }
  let parsed;
  try {
    parsed = new URL(value);
  } catch (error) {
    throw new TypeError(`${label} must be an exact canonical HTTPS URL`, { cause: error });
  }
  const expected =
    expectedPath === "/"
      ? value === parsed.origin || value === `${parsed.origin}/`
      : value === `${parsed.origin}${expectedPath}`;
  const hostname = parsed.hostname.toLowerCase();
  const privateDnsName =
    hostname === "localhost" ||
    hostname.endsWith(".localhost") ||
    hostname.endsWith(".local") ||
    hostname.endsWith(".internal") ||
    hostname.endsWith(".lan") ||
    hostname.endsWith(".") ||
    hostname.length > 253;
  if (
    parsed.protocol !== "https:" ||
    parsed.username !== "" ||
    parsed.password !== "" ||
    parsed.port !== "" ||
    parsed.search !== "" ||
    parsed.hash !== "" ||
    !expected ||
    privateDnsName ||
    !isPublicGatewayLiteral(parsed.hostname)
  ) {
    throw new TypeError(`${label} must be an exact public HTTPS origin${expectedPath}`);
  }
  return value;
}

function assertPositiveIntegerLike(value, label) {
  if (typeof value === "bigint") {
    if (value <= 0n) {
      throw new TypeError(`${label} must be greater than zero`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Number.isNaN(value)) {
      throw new TypeError(`${label} must be a finite number`);
    }
    const coerced = Math.trunc(value);
    if (coerced <= 0) {
      throw new TypeError(`${label} must be greater than zero`);
    }
    if (coerced !== value) {
      throw new TypeError(`${label} must be an integer`);
    }
    if (!Number.isSafeInteger(coerced)) {
      throw new TypeError(`${label} must be a safe integer`);
    }
    return coerced;
  }
  throw new TypeError(`${label} must be a positive integer`);
}

function assertNonNegativeIntegerLike(value, label) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Number.isNaN(value)) {
      throw new TypeError(`${label} must be a finite number`);
    }
    const coerced = Math.trunc(value);
    if (coerced < 0) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    if (coerced !== value) {
      throw new TypeError(`${label} must be an integer`);
    }
    if (!Number.isSafeInteger(coerced)) {
      throw new TypeError(`${label} must be a safe integer`);
    }
    return coerced;
  }
  throw new TypeError(`${label} must be a non-negative integer`);
}

function decimalIntegerString(value, label, { positive = false } = {}) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) {
      throw new TypeError(`${label} must be an unsigned decimal integer`);
    }
    if (positive && BigInt(trimmed) <= 0n) {
      throw new TypeError(`${label} must be greater than zero`);
    }
    return trimmed;
  }
  const normalized = positive
    ? assertPositiveIntegerLike(value, label)
    : assertNonNegativeIntegerLike(value, label);
  return normalized.toString();
}

function canonicalXorQuantityString(value, label, { positive = false } = {}) {
  if (typeof value !== "string") {
    throw new TypeError(`${label} must be a canonical XOR quantity string`);
  }
  if (value.length > SORAFS_XOR_QUANTITY_MAX_TEXT_LENGTH) {
    throw new RangeError(`${label} exceeds the canonical XOR quantity text bound`);
  }
  let quantity;
  try {
    quantity = NumericV1.decodeQuantityJson(value);
  } catch (error) {
    if (!(error instanceof NumericV1Error)) {
      throw error;
    }
    throw new TypeError(
      `${label} must be a canonical non-negative XOR quantity (${error.code})`,
    );
  }
  if (quantity.scale > 9) {
    throw new RangeError(`${label} must have at most 9 fractional decimal places`);
  }
  if (positive && quantity.mantissa <= 0n) {
    throw new RangeError(`${label} must be greater than zero`);
  }
  return quantity.toString();
}

function rejectRetiredOrderbookFields(fields, names) {
  for (const name of names) {
    if (Object.prototype.hasOwnProperty.call(fields, name)) {
      throw new TypeError(`${name} is retired from the canonical V1 SDK surface`);
    }
  }
}

function requiredField(object, label, ...names) {
  const value = readPayloadField(object, ...names);
  if (value === undefined || value === null) {
    throw new TypeError(`${label} is required`);
  }
  return value;
}

function optionalField(object, ...names) {
  const value = readPayloadField(object, ...names);
  return value === undefined || value === null ? undefined : value;
}

function fixedBytesField(object, label, ...names) {
  const bytes = toBuffer(requiredField(object, label, ...names));
  if (bytes.length !== 32) {
    throw new TypeError(`${label} must be exactly 32 bytes`);
  }
  return bytes;
}

function orderbookOwnerAccount(value, label = "ownerAccount") {
  const bytes = toBuffer(value);
  if (bytes.length === 0) {
    throw new TypeError(`${label} must not be empty`);
  }
  if (bytes.length > ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1) {
    throw new RangeError(
      `${label} must be at most ${ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1} bytes`,
    );
  }
  return bytes;
}

function orderbookOwnerAccountField(object, label, ...names) {
  return orderbookOwnerAccount(requiredField(object, label, ...names), label);
}

function normalizeOrderbookFeeBps(value, label) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) {
      throw new TypeError(`${label} must be an unsigned decimal integer`);
    }
    const parsed = BigInt(trimmed);
    if (parsed > 65535n) {
      throw new TypeError(`${label} must fit within a 16-bit unsigned integer`);
    }
    return Number(parsed);
  }
  const normalized = assertNonNegativeIntegerLike(value, label);
  const numeric = typeof normalized === "bigint" ? normalized : BigInt(normalized);
  if (numeric > 65535n) {
    throw new TypeError(`${label} must fit within a 16-bit unsigned integer`);
  }
  return Number(numeric);
}

function normalizeReferenceUnix(value, label) {
  const normalized = assertNonNegativeIntegerLike(value, label);
  if (typeof normalized === "bigint") {
    if (normalized > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError(`${label} must be a safe integer`);
    }
    return Number(normalized);
  }
  return normalized;
}

function normalizeGeneratedAtUnix(value) {
  return normalizeReferenceUnix(
    value ?? Math.floor(Date.now() / 1000),
    "options.generatedAtUnix",
  );
}

function toSafeNumber(value) {
  if (typeof value === "bigint") {
    if (value <= BigInt(Number.MAX_SAFE_INTEGER) && value >= BigInt(Number.MIN_SAFE_INTEGER)) {
      return Number(value);
    }
    return value;
  }
  return value;
}

export function normaliseGatewayProvider(spec) {
  if (spec == null || typeof spec !== "object") {
    throw new TypeError("provider specification must be an object");
  }
  const name = spec.name;
  if (
    typeof name !== "string" ||
    Buffer.byteLength(name, "utf8") > 128 ||
    !/^[0-9A-Za-z._:-]+$/.test(name)
  ) {
    throw new TypeError("provider.name must be canonical ASCII and at most 128 bytes");
  }
  const providerIdHex = requireCanonicalGatewayHex32(
    spec.providerIdHex,
    "provider.providerIdHex",
  );
  const gatewayPublicKeyHex = requireCanonicalGatewayHex32(
    spec.gatewayPublicKeyHex,
    "provider.gatewayPublicKeyHex",
  );
  const baseUrl = requireCanonicalGatewayUrl(spec.baseUrl, "provider.baseUrl", "/");
  const streamTokenB64 = requireCanonicalGatewayToken(
    spec.streamTokenB64,
    "provider.streamTokenB64",
  );
  const native = {
    name,
    provider_id_hex: providerIdHex,
    gateway_public_key_hex: gatewayPublicKeyHex,
    base_url: baseUrl,
    stream_token_b64: streamTokenB64,
  };
  if (spec.privacyEventsUrl !== undefined && spec.privacyEventsUrl !== null) {
    native.privacy_events_url = requireCanonicalGatewayUrl(
      spec.privacyEventsUrl,
      "provider.privacyEventsUrl",
      "/privacy/events",
    );
  }
  return native;
}

function normaliseLocalProxyOptions(options) {
  if (options == null) {
    return undefined;
  }
  if (typeof options !== "object") {
    throw new TypeError("localProxy options must be an object");
  }
  const result = {};
  if (typeof options.bindAddr === "string" && options.bindAddr.trim() !== "") {
    result.bind_addr = options.bindAddr.trim();
  }
  if (typeof options.telemetryLabel === "string" && options.telemetryLabel.trim() !== "") {
    result.telemetry_label = options.telemetryLabel.trim();
  }
  if (typeof options.guardCacheKeyHex === "string" && options.guardCacheKeyHex.trim() !== "") {
    result.guard_cache_key_hex = options.guardCacheKeyHex.trim();
  }
  if (typeof options.emitBrowserManifest === "boolean") {
    result.emit_browser_manifest = options.emitBrowserManifest;
  }
  if (typeof options.proxyMode === "string" && options.proxyMode.trim() !== "") {
    result.proxy_mode = options.proxyMode.trim();
  }
  if (typeof options.prewarmCircuits === "boolean") {
    result.prewarm_circuits = options.prewarmCircuits;
  }
  if (typeof options.maxStreamsPerCircuit === "number") {
    result.max_streams_per_circuit = options.maxStreamsPerCircuit;
  }
  if (typeof options.circuitTtlHintSecs === "number") {
    result.circuit_ttl_hint_secs = options.circuitTtlHintSecs;
  }
  if (options.noritoBridge) {
    const bridge = options.noritoBridge;
    const spoolDir = assertNonEmptyString(bridge.spoolDir, "localProxy.noritoBridge.spoolDir");
    result.norito_bridge = { spool_dir: spoolDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.norito_bridge.extension = bridge.extension.trim();
    }
  }
  if (options.carBridge) {
    const bridge = options.carBridge;
    const cacheDir = assertNonEmptyString(bridge.cacheDir, "localProxy.carBridge.cacheDir");
    result.car_bridge = { cache_dir: cacheDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.car_bridge.extension = bridge.extension.trim();
    }
    if (typeof bridge.allowZst === "boolean") {
      result.car_bridge.allow_zst = bridge.allowZst;
    }
  }
  if (options.kaigiBridge) {
    const bridge = options.kaigiBridge;
    const spoolDir = assertNonEmptyString(bridge.spoolDir, "localProxy.kaigiBridge.spoolDir");
    result.kaigi_bridge = { spool_dir: spoolDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.kaigi_bridge.extension = bridge.extension.trim();
    }
    if (typeof bridge.roomPolicy === "string" && bridge.roomPolicy.trim() !== "") {
      const normalized = bridge.roomPolicy.trim().toLowerCase();
      if (normalized !== "public" && normalized !== "authenticated") {
        throw new TypeError(
          "localProxy.kaigiBridge.roomPolicy must be `public` or `authenticated`",
        );
      }
      result.kaigi_bridge.room_policy = normalized;
    }
  }
  return result;
}

function deriveScoreboardTelemetryLabel(options) {
  if (
    typeof options.scoreboardTelemetryLabel === "string" &&
    options.scoreboardTelemetryLabel.trim() !== ""
  ) {
    return options.scoreboardTelemetryLabel.trim();
  }
  if (typeof options.telemetryRegion === "string" && options.telemetryRegion.trim() !== "") {
    return `region:${options.telemetryRegion.trim()}`;
  }
  if (typeof options.clientId === "string" && options.clientId.trim() !== "") {
    return `client:${options.clientId.trim()}`;
  }
  return "sdk:js";
}

const GATEWAY_ROLLOUT_PHASE_LABELS_V1 = Object.freeze([
  "canary",
  "ramp",
  "default",
]);
const GATEWAY_TRANSPORT_POLICY_LABELS_V1 = Object.freeze([
  "soranet-first",
  "soranet-strict",
  "direct-only",
]);
const GATEWAY_ANONYMITY_POLICY_LABELS_V1 = Object.freeze([
  "anon-guard-pq",
  "anon-majority-pq",
  "anon-strict-pq",
]);
const GATEWAY_WRITE_MODE_LABELS_V1 = Object.freeze([
  "read-only",
  "upload-pq-only",
]);

function optionalExactGatewayLabel(value, field, labels) {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string" || !labels.includes(value)) {
    throw new TypeError(`${field} must be one of ${labels.join("|")}`);
  }
  return value;
}

function normaliseGatewayOptions(options = {}) {
  if (options == null) {
    return undefined;
  }
  if (typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  rejectUnexpectedFields(
    options,
    [
      "manifestEnvelopeB64",
      "manifestCidHex",
      "clientId",
      "telemetryRegion",
      "rolloutPhase",
      "maxPeers",
      "retryBudget",
      "transportPolicy",
      "anonymityPolicy",
      "writeMode",
      "policyOverride",
      "localProxy",
      "scoreboardOutPath",
      "scoreboardNowUnixSecs",
      "scoreboardTelemetryLabel",
      "scoreboardAllowImplicitMetadata",
    ],
    "sorafsGatewayFetch options",
  );
  const native = {};
  if (typeof options.manifestEnvelopeB64 === "string" && options.manifestEnvelopeB64.trim() !== "") {
    native.manifest_envelope_b64 = normalizeBase64Payload(
      options.manifestEnvelopeB64,
      "manifestEnvelopeB64",
    );
  }
  if (typeof options.manifestCidHex === "string" && options.manifestCidHex.trim() !== "") {
    native.manifest_cid_hex = normalizeHex32(options.manifestCidHex, "manifestCidHex");
  }
  if (typeof options.clientId === "string" && options.clientId.trim() !== "") {
    native.client_id = options.clientId.trim();
  }
  if (typeof options.telemetryRegion === "string" && options.telemetryRegion.trim() !== "") {
    native.telemetry_region = options.telemetryRegion.trim();
  }
  const rolloutPhase = optionalExactGatewayLabel(
    options.rolloutPhase,
    "rolloutPhase",
    GATEWAY_ROLLOUT_PHASE_LABELS_V1,
  );
  if (rolloutPhase !== undefined) {
    native.rollout_phase = rolloutPhase;
  }
  if (options.maxPeers !== undefined && options.maxPeers !== null) {
    if (
      typeof options.maxPeers !== "number" ||
      !Number.isFinite(options.maxPeers) ||
      !Number.isInteger(options.maxPeers) ||
      !Number.isSafeInteger(options.maxPeers) ||
      options.maxPeers < 1
    ) {
      throw new TypeError("maxPeers must be a positive safe integer");
    }
    native.max_peers = options.maxPeers;
  }
  if (options.retryBudget !== undefined && options.retryBudget !== null) {
    if (
      typeof options.retryBudget !== "number" ||
      !Number.isFinite(options.retryBudget) ||
      !Number.isInteger(options.retryBudget) ||
      !Number.isSafeInteger(options.retryBudget) ||
      options.retryBudget < 0
    ) {
      throw new TypeError("retryBudget must be a non-negative safe integer");
    }
    native.retry_budget = options.retryBudget;
  }
  const transportPolicy = optionalExactGatewayLabel(
    options.transportPolicy,
    "transportPolicy",
    GATEWAY_TRANSPORT_POLICY_LABELS_V1,
  );
  if (transportPolicy !== undefined) {
    native.transport_policy = transportPolicy;
  }
  const anonymityPolicy = optionalExactGatewayLabel(
    options.anonymityPolicy,
    "anonymityPolicy",
    GATEWAY_ANONYMITY_POLICY_LABELS_V1,
  );
  if (anonymityPolicy !== undefined) {
    native.anonymity_policy = anonymityPolicy;
  }
  const writeMode = optionalExactGatewayLabel(
    options.writeMode,
    "writeMode",
    GATEWAY_WRITE_MODE_LABELS_V1,
  );
  if (writeMode !== undefined) {
    native.write_mode = writeMode;
  }
  if (
    options.policyOverride != null &&
    typeof options.policyOverride === "object"
  ) {
    const override = {};
    const overrideTransportPolicy = optionalExactGatewayLabel(
      options.policyOverride.transportPolicy,
      "policyOverride.transportPolicy",
      GATEWAY_TRANSPORT_POLICY_LABELS_V1,
    );
    if (overrideTransportPolicy !== undefined) {
      override.transport_policy = overrideTransportPolicy;
    }
    const overrideAnonymityPolicy = optionalExactGatewayLabel(
      options.policyOverride.anonymityPolicy,
      "policyOverride.anonymityPolicy",
      GATEWAY_ANONYMITY_POLICY_LABELS_V1,
    );
    if (overrideAnonymityPolicy !== undefined) {
      override.anonymity_policy = overrideAnonymityPolicy;
    }
    if (Object.keys(override).length > 0) {
      native.policy_override = override;
    }
  }
  const proxyOptions = normaliseLocalProxyOptions(options.localProxy);
  if (proxyOptions) {
    native.local_proxy = proxyOptions;
  }
  let scoreboardOutRequested = false;
  if (typeof options.scoreboardOutPath === "string" && options.scoreboardOutPath.trim() !== "") {
    native.scoreboard_out_path = options.scoreboardOutPath.trim();
    scoreboardOutRequested = true;
  }
  if (
    options.scoreboardNowUnixSecs !== undefined &&
    options.scoreboardNowUnixSecs !== null
  ) {
    native.scoreboard_now_unix_secs = assertNonNegativeIntegerLike(
      options.scoreboardNowUnixSecs,
      "scoreboardNowUnixSecs",
    );
  }
  if (
    typeof options.scoreboardTelemetryLabel === "string" &&
    options.scoreboardTelemetryLabel.trim() !== ""
  ) {
    native.scoreboard_telemetry_label = options.scoreboardTelemetryLabel.trim();
  } else if (scoreboardOutRequested) {
    native.scoreboard_telemetry_label = deriveScoreboardTelemetryLabel(options);
  }
  if (typeof options.scoreboardAllowImplicitMetadata === "boolean") {
    native.scoreboard_allow_implicit_metadata = options.scoreboardAllowImplicitMetadata;
  }
  return native;
}

function transformProviderReport(report) {
  return {
    provider: report.provider,
    successes: report.successes,
    failures: report.failures,
    disabled: report.disabled,
  };
}

function transformChunkReceipt(receipt) {
  return {
    chunkIndex: receipt.chunk_index,
    provider: receipt.provider,
    attempts: receipt.attempts,
    latencyMs: receipt.latency_ms,
    bytes: receipt.bytes,
  };
}

function transformCarVerification(raw) {
  if (!raw) {
    return null;
  }
  return {
    manifestDigestHex: raw.manifest_digest_hex,
    manifestPayloadDigestHex: raw.manifest_payload_digest_hex,
    manifestCarDigestHex: raw.manifest_car_digest_hex,
    manifestContentLength: toSafeNumber(raw.manifest_content_length),
    manifestChunkCount: toSafeNumber(raw.manifest_chunk_count),
    manifestChunkProfileHandle: raw.manifest_chunk_profile_handle,
    manifestGovernance: {
      councilSignatures: Array.isArray(raw.manifest_governance?.council_signatures)
        ? raw.manifest_governance.council_signatures.map((entry) => ({
            signerHex: entry.signer_hex,
            signatureHex: entry.signature_hex,
          }))
        : [],
    },
    carArchive: {
      size: toSafeNumber(raw.car_archive.size),
      payloadDigestHex: raw.car_archive.payload_digest_hex,
      archiveDigestHex: raw.car_archive.archive_digest_hex,
      cidHex: raw.car_archive.cid_hex,
      rootCidsHex: Array.isArray(raw.car_archive.root_cids_hex)
        ? raw.car_archive.root_cids_hex.slice()
        : [],
      verified: Boolean(raw.car_archive.verified),
      porLeafCount: toSafeNumber(raw.car_archive.por_leaf_count),
    },
  };
}

function transformGatewayResult(raw) {
  const localManifest = raw.local_proxy_manifest_json
    ? JSON.parse(raw.local_proxy_manifest_json)
    : null;
  return {
    manifestIdHex: raw.manifest_id_hex,
    chunkerHandle: raw.chunker_handle,
    chunkCount: raw.chunk_count,
    assembledBytes: toSafeNumber(raw.assembled_bytes),
    payload: raw.payload,
    telemetryRegion: raw.telemetry_region ?? null,
    anonymity: {
      policy: raw.anonymity_policy,
      status: raw.anonymity_status,
      reason: raw.anonymity_reason,
      soranetSelected: raw.anonymity_soranet_selected,
      pqSelected: raw.anonymity_pq_selected,
      classicalSelected: raw.anonymity_classical_selected,
      classicalRatio: raw.anonymity_classical_ratio,
      pqRatio: raw.anonymity_pq_ratio,
      candidateRatio: raw.anonymity_candidate_ratio,
      deficitRatio: raw.anonymity_deficit_ratio,
      supplyDelta: raw.anonymity_supply_delta,
      brownout: Boolean(raw.anonymity_brownout),
      brownoutEffective: Boolean(raw.anonymity_brownout_effective),
      usesClassical: Boolean(raw.anonymity_uses_classical),
    },
    providerReports: Array.isArray(raw.provider_reports)
      ? raw.provider_reports.map(transformProviderReport)
      : [],
    chunkReceipts: Array.isArray(raw.chunk_receipts)
      ? raw.chunk_receipts.map(transformChunkReceipt)
      : [],
    localProxyManifest: localManifest,
    carVerification: transformCarVerification(raw.car_verification),
    metadata: transformGatewayMetadata(raw.metadata),
    scoreboard: transformGatewayScoreboard(raw.scoreboard),
  };
}

function transformGatewayScoreboard(raw) {
  if (raw === undefined) {
    return undefined;
  }
  if (raw === null) {
    return null;
  }
  if (!Array.isArray(raw)) {
    return undefined;
  }
  return raw.map((entry) => ({
    provider_id:
      typeof entry.provider_id === "string" ? entry.provider_id : "",
    alias: entry.alias ?? null,
    raw_score: entry.raw_score ?? 0,
    normalized_weight: entry.normalized_weight ?? 0,
    eligibility: entry.eligibility ?? null,
  }));
}

function transformGatewayMetadata(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      providerCount: 0,
      gatewayProviderCount: 0,
      providerMix: "none",
      transportPolicy: "",
      transportPolicyOverride: false,
      transportPolicyOverrideLabel: null,
      anonymityPolicy: "",
      anonymityPolicyOverride: false,
      anonymityPolicyOverrideLabel: null,
      maxParallel: null,
      maxPeers: null,
      retryBudget: null,
      providerFailureThreshold: 0,
      assumeNowUnix: 0,
      telemetrySourceLabel: null,
      telemetryRegion: null,
      gatewayManifestProvided: false,
      gatewayManifestId: null,
      gatewayManifestCid: null,
      writeMode: "read-only",
      writeModeEnforcesPq: false,
      allowImplicitMetadata: false,
    };
  }
  const coerceOptionalNumber = (value) =>
    value === undefined || value === null ? null : toSafeNumber(value);
  return {
    providerCount: toSafeNumber(raw.provider_count ?? 0),
    gatewayProviderCount: toSafeNumber(raw.gateway_provider_count ?? 0),
    providerMix: typeof raw.provider_mix === "string" ? raw.provider_mix : "none",
    transportPolicy: typeof raw.transport_policy === "string" ? raw.transport_policy : "",
    transportPolicyOverride: Boolean(raw.transport_policy_override),
    transportPolicyOverrideLabel:
      typeof raw.transport_policy_override_label === "string"
        ? raw.transport_policy_override_label
        : null,
    anonymityPolicy: typeof raw.anonymity_policy === "string" ? raw.anonymity_policy : "",
    anonymityPolicyOverride: Boolean(raw.anonymity_policy_override),
    anonymityPolicyOverrideLabel:
      typeof raw.anonymity_policy_override_label === "string"
        ? raw.anonymity_policy_override_label
        : null,
    writeMode: typeof raw.write_mode === "string" ? raw.write_mode : "read-only",
    writeModeEnforcesPq: Boolean(raw.write_mode_enforces_pq),
    maxParallel: coerceOptionalNumber(raw.max_parallel),
    maxPeers: coerceOptionalNumber(raw.max_peers),
    retryBudget: coerceOptionalNumber(raw.retry_budget),
    providerFailureThreshold: toSafeNumber(raw.provider_failure_threshold ?? 0),
    assumeNowUnix: toSafeNumber(raw.assume_now_unix ?? 0),
    telemetrySourceLabel:
      typeof raw.telemetry_source_label === "string" ? raw.telemetry_source_label : null,
    telemetryRegion: typeof raw.telemetry_region === "string" ? raw.telemetry_region : null,
    gatewayManifestProvided: Boolean(raw.gateway_manifest_provided),
    gatewayManifestId:
      typeof raw.gateway_manifest_id === "string" ? raw.gateway_manifest_id : null,
    gatewayManifestCid:
      typeof raw.gateway_manifest_cid === "string" ? raw.gateway_manifest_cid : null,
    allowImplicitMetadata: Boolean(raw.allow_implicit_metadata),
  };
}

export function sorafsGatewayFetch(
  manifestIdHex,
  chunkerHandle,
  planJson,
  providers,
  options = {},
) {
  return sorafsGatewayFetchWithRuntime(
    defaultNativeRuntime,
    manifestIdHex,
    chunkerHandle,
    planJson,
    providers,
    options,
  );
}

function sorafsGatewayFetchWithRuntime(
  nativeRuntime,
  manifestIdHex,
  chunkerHandle,
  planJson,
  providers,
  options = {},
) {
  const baseOptions = options ?? {};
  if (!isPlainObject(baseOptions)) {
    throw new TypeError("sorafsGatewayFetch options must be a plain object");
  }
  const normalizedManifestId = normalizeHex32(manifestIdHex, "manifestIdHex");
  const normalizedChunkerHandle = assertNonEmptyString(chunkerHandle, "chunkerHandle");
  const normalizedPlanJson = assertNonEmptyString(planJson, "planJson");
  if (!Array.isArray(providers) || providers.length === 0) {
    throw new TypeError("providers must be a non-empty array");
  }
  if (providers.length > 256) {
    throw new TypeError("providers must contain at most 256 entries");
  }
  const nativeProviders = providers.map(normaliseGatewayProvider);
  const uniqueProviderCount = new Set(
    nativeProviders.map((spec) => spec.provider_id_hex),
  ).size;
  if (uniqueProviderCount < 2) {
    throw new Error(
      "sorafsGatewayFetch requires at least two gateway providers.",
    );
  }
  const nativeOptions = normaliseGatewayOptions(baseOptions);
  const binding = requireSorafsNativeFunction(
    "sorafsGatewayFetch",
    "gateway fetch",
    nativeRuntime,
  );
  try {
    const raw = binding.sorafsGatewayFetch(
      normalizedManifestId,
      normalizedChunkerHandle,
      normalizedPlanJson,
      nativeProviders,
      nativeOptions,
    );
    return transformGatewayResult(raw);
  } catch (error) {
    throw convertSorafsGatewayError(error);
  }
}

/** @internal Source-level gateway facade bound to one immutable native runtime. */
export function _createSorafsGatewayApi(nativeRuntime) {
  return Object.freeze({
    sorafsGatewayFetch: (
      manifestIdHex,
      chunkerHandle,
      planJson,
      providers,
      options = {},
    ) =>
      sorafsGatewayFetchWithRuntime(
        nativeRuntime,
        manifestIdHex,
        chunkerHandle,
        planJson,
        providers,
        options,
      ),
  });
}

function convertSorafsGatewayError(error) {
  const payload = parseGatewayErrorPayload(error?.message);
  if (payload && payload.kind === "multi_source") {
    return new SorafsGatewayFetchError(payload, error);
  }
  return error;
}

function parseGatewayErrorPayload(text) {
  if (typeof text !== "string") {
    return null;
  }
  const trimmed = text.trim();
  if (!trimmed.startsWith("{")) {
    return null;
  }
  try {
    const parsed = JSON.parse(trimmed);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}
