import { getNativeBinding } from "./native.js";

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

function requireSorafsNativeFunction(functionName, capability) {
  const binding = getNativeBinding();
  if (!binding || typeof binding[functionName] !== "function") {
    throw new Error(
      `SoraFS ${capability} requires the native iroha_js_host module. Run \`npm run build:native\` before using these helpers.`,
    );
  }
  return binding;
}

function requireSorafsNativeBinding() {
  return requireSorafsNativeFunction(
    "sorafsDecodeReplicationOrder",
    "decoding",
  );
}

export const SORAFS_ORDERBOOK_PAYLOAD_KINDS = Object.freeze({
  ORDER_REQUEST: "order-request",
  ORDER_CANCEL: "order-cancel",
  TRADE_EVENT: "trade-event",
  SETTLEMENT_CHANNEL: "settlement-channel",
  SETTLEMENT_RECEIPT: "settlement-receipt",
  RUNTIME_SNAPSHOT: "runtime-snapshot",
});

const ORDERBOOK_KIND_ALIASES = Object.freeze({
  order: SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
  "order-request": SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
  "orderbook-order-request": SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
  request: SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
  cancel: SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_CANCEL,
  "order-cancel": SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_CANCEL,
  "orderbook-order-cancel": SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_CANCEL,
  trade: SORAFS_ORDERBOOK_PAYLOAD_KINDS.TRADE_EVENT,
  "trade-event": SORAFS_ORDERBOOK_PAYLOAD_KINDS.TRADE_EVENT,
  "orderbook-trade-event": SORAFS_ORDERBOOK_PAYLOAD_KINDS.TRADE_EVENT,
  channel: SORAFS_ORDERBOOK_PAYLOAD_KINDS.SETTLEMENT_CHANNEL,
  "settlement-channel": SORAFS_ORDERBOOK_PAYLOAD_KINDS.SETTLEMENT_CHANNEL,
  receipt: SORAFS_ORDERBOOK_PAYLOAD_KINDS.SETTLEMENT_RECEIPT,
  "settlement-receipt": SORAFS_ORDERBOOK_PAYLOAD_KINDS.SETTLEMENT_RECEIPT,
  snapshot: SORAFS_ORDERBOOK_PAYLOAD_KINDS.RUNTIME_SNAPSHOT,
  "runtime-snapshot": SORAFS_ORDERBOOK_PAYLOAD_KINDS.RUNTIME_SNAPSHOT,
  "orderbook-runtime-snapshot": SORAFS_ORDERBOOK_PAYLOAD_KINDS.RUNTIME_SNAPSHOT,
});

function normalizeOrderbookPayloadKind(kind) {
  if (typeof kind !== "string") {
    throw new TypeError("kind must be a string");
  }
  const normalized = kind.trim().toLowerCase().replace(/_/g, "-");
  const canonical = ORDERBOOK_KIND_ALIASES[normalized];
  if (!canonical) {
    throw new TypeError(`unsupported SoraFS orderbook payload kind: ${kind}`);
  }
  return canonical;
}

export const SORAFS_PDP_PAYLOAD_KINDS = Object.freeze({
  COMMITMENT: "commitment",
  CHALLENGE: "challenge",
  PROOF: "proof",
});

const PDP_KIND_ALIASES = Object.freeze({
  commitment: SORAFS_PDP_PAYLOAD_KINDS.COMMITMENT,
  "pdp-commitment": SORAFS_PDP_PAYLOAD_KINDS.COMMITMENT,
  challenge: SORAFS_PDP_PAYLOAD_KINDS.CHALLENGE,
  "pdp-challenge": SORAFS_PDP_PAYLOAD_KINDS.CHALLENGE,
  proof: SORAFS_PDP_PAYLOAD_KINDS.PROOF,
  "pdp-proof": SORAFS_PDP_PAYLOAD_KINDS.PROOF,
});

function normalizePdpPayloadKind(kind) {
  if (typeof kind !== "string") {
    throw new TypeError("kind must be a string");
  }
  const normalized = kind.trim().toLowerCase().replace(/_/g, "-");
  const canonical = PDP_KIND_ALIASES[normalized];
  if (!canonical) {
    throw new TypeError(`unsupported SoraFS PDP payload kind: ${kind}`);
  }
  return canonical;
}

function readPayloadField(object, ...names) {
  for (const name of names) {
    if (Object.prototype.hasOwnProperty.call(object, name)) {
      return object[name];
    }
  }
  return undefined;
}

function formatAssignment(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      providerIdHex: "",
      sliceGiB: 0,
      lane: null,
    };
  }
  const providerValue = readPayloadField(raw, "provider_id_hex", "providerIdHex");
  const providerIdHex =
    typeof providerValue === "string" ? providerValue : "";
  const sliceValue = readPayloadField(raw, "slice_gib", "sliceGib");
  const laneValue = readPayloadField(raw, "lane");
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
  const ingest = readPayloadField(
    raw,
    "ingest_deadline_secs",
    "ingestDeadlineSecs",
  );
  const availability = readPayloadField(
    raw,
    "min_availability_percent_milli",
    "minAvailabilityPercentMilli",
  );
  const por = readPayloadField(
    raw,
    "min_por_success_percent_milli",
    "minPorSuccessPercentMilli",
  );
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
 *   manifestCidUtf8: string | null,
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
  const schemaVersion = Number(
    readPayloadField(payload, "schema_version", "schemaVersion") ?? 0,
  );
  const orderIdValue = readPayloadField(payload, "order_id_hex", "orderIdHex");
  const orderIdHex = typeof orderIdValue === "string" ? orderIdValue : "";
  const manifestCidUtf8Value = readPayloadField(
    payload,
    "manifest_cid_utf8",
    "manifestCidUtf8",
  );
  const manifestCidUtf8 =
    manifestCidUtf8Value === null
      ? null
      : typeof manifestCidUtf8Value === "string"
      ? manifestCidUtf8Value
      : null;
  const manifestCidBase64Value = readPayloadField(
    payload,
    "manifest_cid_base64",
    "manifestCidBase64",
  );
  const manifestCidBase64 =
    typeof manifestCidBase64Value === "string" ? manifestCidBase64Value : "";
  const manifestDigestValue = readPayloadField(
    payload,
    "manifest_digest_hex",
    "manifestDigestHex",
  );
  const manifestDigestHex =
    typeof manifestDigestValue === "string" ? manifestDigestValue : "";
  const chunkingProfileValue = readPayloadField(
    payload,
    "chunking_profile",
    "chunkingProfile",
  );
  const chunkingProfile =
    typeof chunkingProfileValue === "string" ? chunkingProfileValue : "";
  const targetReplicas = readPayloadField(
    payload,
    "target_replicas",
    "targetReplicas",
  ) ?? 0;
  const issuedAtUnix = readPayloadField(
    payload,
    "issued_at_unix",
    "issuedAtUnix",
  ) ?? 0;
  const deadlineAtUnix = readPayloadField(
    payload,
    "deadline_at_unix",
    "deadlineAtUnix",
  ) ?? 0;
  const sla = formatSla(readPayloadField(payload, "sla"));
  return {
    schemaVersion,
    orderIdHex,
    manifestCidUtf8,
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
 * @param {{ label?: string, generatedAtUnix?: number | bigint, generated_at?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validateOrderbookPayload(kind, bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  const canonicalKind = normalizeOrderbookPayloadKind(kind);
  const buffer = toBuffer(bytes);
  const label =
    typeof options.label === "string" && options.label.trim() !== ""
      ? options.label.trim()
      : `sdk:sorafs.orderbook.${canonicalKind}`;
  const generatedAtUnix = normalizeGeneratedAtUnix(
    readPayloadField(options, "generatedAtUnix", "generated_at"),
  );
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
  const quantityGib = decimalIntegerString(
    requiredField(fields, "quantityGib", "quantityGib", "quantity_gib"),
    "quantityGib",
    { positive: true },
  );
  const remainingValue = optionalField(fields, "remainingGib", "remaining_gib");
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookOrderRequest",
    "orderbook order request builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookOrderRequest(
      fixedBytesField(fields, "orderId", "orderId", "order_id"),
      String(requiredField(fields, "side", "side")),
      String(requiredField(fields, "tier", "tier")),
      decimalIntegerString(
        requiredField(
          fields,
          "pricePerGibMicroXor",
          "pricePerGibMicroXor",
          "price_per_gib_micro_xor",
        ),
        "pricePerGibMicroXor",
        { positive: true },
      ),
      quantityGib,
      remainingValue === undefined
        ? undefined
        : decimalIntegerString(remainingValue, "remainingGib", { positive: true }),
      bytesField(fields, "ownerAccount", "ownerAccount", "owner_account"),
      decimalIntegerString(
        requiredField(fields, "expiryUnix", "expiryUnix", "expiry_unix"),
        "expiryUnix",
        { positive: true },
      ),
      decimalIntegerString(
        requiredField(fields, "nonce", "nonce"),
        "nonce",
        { positive: true },
      ),
      normalizeOrderbookFeeBps(
        requiredField(fields, "makerFeeBps", "makerFeeBps", "maker_fee_bps"),
        "makerFeeBps",
      ),
      normalizeOrderbookFeeBps(
        requiredField(fields, "takerFeeBps", "takerFeeBps", "taker_fee_bps"),
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
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookOrderCancel",
    "orderbook cancel builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookOrderCancel(
      fixedBytesField(fields, "orderId", "orderId", "order_id"),
      bytesField(fields, "ownerAccount", "ownerAccount", "owner_account"),
      String(requiredField(fields, "reason", "reason")),
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
  const binding = requireSorafsNativeFunction(
    "sorafsBuildSignedOrderbookSettlementReceipt",
    "orderbook settlement receipt builder",
  );
  return requireSignedBuilderBuffer(
    binding.sorafsBuildSignedOrderbookSettlementReceipt(
      fixedBytesField(fields, "receiptId", "receiptId", "receipt_id"),
      fixedBytesField(fields, "channelId", "channelId", "channel_id"),
      fixedBytesField(fields, "tradeId", "tradeId", "trade_id"),
      decimalIntegerString(
        requiredField(fields, "rangeStart", "rangeStart", "range_start"),
        "rangeStart",
      ),
      decimalIntegerString(
        requiredField(fields, "rangeEnd", "rangeEnd", "range_end"),
        "rangeEnd",
        { positive: true },
      ),
      fixedBytesField(fields, "chunkHash", "chunkHash", "chunk_hash"),
      decimalIntegerString(
        requiredField(fields, "bytesDelivered", "bytesDelivered", "bytes_delivered"),
        "bytesDelivered",
        { positive: true },
      ),
      decimalIntegerString(
        requiredField(
          fields,
          "xorDebitedMicroXor",
          "xorDebitedMicroXor",
          "xor_debited_micro_xor",
        ),
        "xorDebitedMicroXor",
        { positive: true },
      ),
      decimalIntegerString(
        requiredField(
          fields,
          "providerCreditMicroXor",
          "providerCreditMicroXor",
          "provider_credit_micro_xor",
        ),
        "providerCreditMicroXor",
      ),
      decimalIntegerString(
        requiredField(fields, "feeAmountMicroXor", "feeAmountMicroXor", "fee_amount_micro_xor"),
        "feeAmountMicroXor",
      ),
      decimalIntegerString(
        requiredField(fields, "issuedAtUnix", "issuedAtUnix", "issued_at_unix"),
        "issuedAtUnix",
        { positive: true },
      ),
      toBuffer(privateKey),
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

function referenceLabel(options, names, fallback) {
  for (const name of names) {
    const value = readPayloadField(options, name);
    if (typeof value === "string" && value.trim() !== "") {
      return value.trim();
    }
  }
  return fallback;
}

/**
 * Validate one Norito-encoded PDP payload with the Rust reference validator.
 * @param {string} kind
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ label?: string, generatedAtUnix?: number | bigint, generated_at?: number | bigint }} [options]
 * @returns {Record<string, any>}
 */
export function validatePdpPayload(kind, bytes, options = {}) {
  if (!isPlainObject(options)) {
    throw new TypeError("options must be an object");
  }
  const canonicalKind = normalizePdpPayloadKind(kind);
  const buffer = toBuffer(bytes);
  const label = referenceLabel(
    options,
    ["label"],
    `sdk:sorafs.pdp.${canonicalKind}`,
  );
  const generatedAtUnix = normalizeGeneratedAtUnix(
    readPayloadField(options, "generatedAtUnix", "generated_at"),
  );
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
 * Validate PDP commitment/challenge binding with the Rust reference validator.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} commitmentBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {{ commitmentLabel?: string, commitment_label?: string, challengeLabel?: string, challenge_label?: string, generatedAtUnix?: number | bigint, generated_at?: number | bigint }} [options]
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
  const generatedAtUnix = normalizeGeneratedAtUnix(
    readPayloadField(options, "generatedAtUnix", "generated_at"),
  );
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpCommitmentChallengeJson",
    "PDP commitment/challenge validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpCommitmentChallengeJson(
      toBuffer(commitmentBytes),
      referenceLabel(
        options,
        ["commitmentLabel", "commitment_label"],
        "sdk:sorafs.pdp.commitment",
      ),
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        ["challengeLabel", "challenge_label"],
        "sdk:sorafs.pdp.challenge",
      ),
      generatedAtUnix,
    ),
    "PDP commitment/challenge validation",
  );
}

/**
 * Validate PDP challenge/proof binding with the Rust reference validator.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} proofBytes
 * @param {{ challengeLabel?: string, challenge_label?: string, proofLabel?: string, proof_label?: string, generatedAtUnix?: number | bigint, generated_at?: number | bigint }} [options]
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
  const generatedAtUnix = normalizeGeneratedAtUnix(
    readPayloadField(options, "generatedAtUnix", "generated_at"),
  );
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpChallengeProofJson",
    "PDP challenge/proof validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpChallengeProofJson(
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        ["challengeLabel", "challenge_label"],
        "sdk:sorafs.pdp.challenge",
      ),
      toBuffer(proofBytes),
      referenceLabel(
        options,
        ["proofLabel", "proof_label"],
        "sdk:sorafs.pdp.proof",
      ),
      generatedAtUnix,
    ),
    "PDP challenge/proof validation",
  );
}

/**
 * Validate PDP commitment/challenge/proof binding with the Rust reference validator.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} commitmentBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} challengeBytes
 * @param {ArrayBufferView | ArrayBuffer | Buffer} proofBytes
 * @param {{ commitmentLabel?: string, commitment_label?: string, challengeLabel?: string, challenge_label?: string, proofLabel?: string, proof_label?: string, generatedAtUnix?: number | bigint, generated_at?: number | bigint }} [options]
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
  const generatedAtUnix = normalizeGeneratedAtUnix(
    readPayloadField(options, "generatedAtUnix", "generated_at"),
  );
  const binding = requireSorafsNativeFunction(
    "sorafsValidatePdpBundleJson",
    "PDP bundle validation",
  );
  return parseReferenceOutcomePayload(
    binding.sorafsValidatePdpBundleJson(
      toBuffer(commitmentBytes),
      referenceLabel(
        options,
        ["commitmentLabel", "commitment_label"],
        "sdk:sorafs.pdp.commitment",
      ),
      toBuffer(challengeBytes),
      referenceLabel(
        options,
        ["challengeLabel", "challenge_label"],
        "sdk:sorafs.pdp.challenge",
      ),
      toBuffer(proofBytes),
      referenceLabel(
        options,
        ["proofLabel", "proof_label"],
        "sdk:sorafs.pdp.proof",
      ),
      generatedAtUnix,
    ),
    "PDP bundle validation",
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

function normalizeBase64PayloadMaybeUrl(value, label) {
  const trimmed = assertNonEmptyString(value, label);
  try {
    return normalizeBase64Payload(trimmed, label);
  } catch (error) {
    if (!/[-_]/.test(trimmed)) {
      throw error;
    }
  }
  const normalized = trimmed.replace(/-/g, "+").replace(/_/g, "/");
  try {
    return normalizeBase64Payload(normalized, label);
  } catch {
    throw new TypeError(`${label} must be a valid base64 or base64url string`);
  }
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

function bytesField(object, label, ...names) {
  const bytes = toBuffer(requiredField(object, label, ...names));
  if (bytes.length === 0) {
    throw new TypeError(`${label} must not be empty`);
  }
  return bytes;
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

function normalizeGeneratedAtUnix(value) {
  const raw = value ?? Math.floor(Date.now() / 1000);
  const normalized = assertNonNegativeIntegerLike(raw, "options.generatedAtUnix");
  if (typeof normalized === "bigint") {
    if (normalized > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError("options.generatedAtUnix must be a safe integer");
    }
    return Number(normalized);
  }
  return normalized;
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

function normaliseGatewayProvider(spec) {
  if (spec == null || typeof spec !== "object") {
    throw new TypeError("provider specification must be an object");
  }
  const name = assertNonEmptyString(spec.name, "provider.name");
  const providerIdHex = assertNonEmptyString(spec.providerIdHex, "provider.providerIdHex");
  const normalizedProviderId =
    providerIdHex.startsWith("0x") || providerIdHex.startsWith("0X")
      ? providerIdHex.slice(2)
      : providerIdHex;
  if (normalizedProviderId.length !== 64 || !/^[0-9a-fA-F]+$/.test(normalizedProviderId)) {
    throw new TypeError("provider.providerIdHex must be a 32-byte hex string");
  }
  const baseUrl = assertNonEmptyString(spec.baseUrl, "provider.baseUrl");
  const streamTokenB64 = normalizeBase64PayloadMaybeUrl(
    spec.streamTokenB64,
    "provider.streamTokenB64",
  );
  const native = {
    name,
    provider_id_hex: normalizedProviderId.toLowerCase(),
    base_url: baseUrl,
    stream_token_b64: streamTokenB64,
  };
  if (typeof spec.privacyEventsUrl === "string" && spec.privacyEventsUrl.trim() !== "") {
    native.privacy_events_url = spec.privacyEventsUrl.trim();
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

function normaliseGatewayOptions(options = {}) {
  if (options == null) {
    return undefined;
  }
  if (typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
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
  if (typeof options.rolloutPhase === "string" && options.rolloutPhase.trim() !== "") {
    native.rollout_phase = options.rolloutPhase.trim();
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
  if (typeof options.transportPolicy === "string" && options.transportPolicy.trim() !== "") {
    native.transport_policy = options.transportPolicy.trim();
  }
  if (typeof options.anonymityPolicy === "string" && options.anonymityPolicy.trim() !== "") {
    native.anonymity_policy = options.anonymityPolicy.trim();
  }
  if (typeof options.writeMode === "string" && options.writeMode.trim() !== "") {
    native.write_mode = options.writeMode.trim();
  }
  if (
    options.policyOverride != null &&
    typeof options.policyOverride === "object"
  ) {
    const override = {};
    const { transportPolicy, anonymityPolicy } = options.policyOverride;
    if (typeof transportPolicy === "string" && transportPolicy.trim() !== "") {
      override.transport_policy = transportPolicy.trim();
    }
    if (typeof anonymityPolicy === "string" && anonymityPolicy.trim() !== "") {
      override.anonymity_policy = anonymityPolicy.trim();
    }
    if (Object.keys(override).length > 0) {
      native.policy_override = override;
    }
  }
  const proxyOptions = normaliseLocalProxyOptions(options.localProxy);
  if (proxyOptions) {
    native.local_proxy = proxyOptions;
  }
  if (options.taikaiCache != null) {
    native.taikai_cache = normaliseTaikaiCacheOptions(options.taikaiCache);
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

function normaliseTaikaiCacheOptions(cache) {
  if (!cache || typeof cache !== "object") {
    throw new TypeError("taikaiCache must be an object");
  }
  if (!cache.qos || typeof cache.qos !== "object") {
    throw new TypeError("taikaiCache.qos must be an object with rate fields");
  }
  const qos = cache.qos;
  const burstValue = assertPositiveIntegerLike(qos.burstMultiplier, "taikaiCache.qos.burstMultiplier");
  const burst =
    typeof burstValue === "bigint"
      ? Number(burstValue)
      : burstValue;
  if (!Number.isSafeInteger(burst) || burst > 0xffffffff) {
    throw new TypeError("taikaiCache.qos.burstMultiplier must fit within a 32-bit unsigned integer");
  }

  const result = {
    hot_capacity_bytes: assertPositiveIntegerLike(
      cache.hotCapacityBytes,
      "taikaiCache.hotCapacityBytes",
    ),
    hot_retention_secs: assertPositiveIntegerLike(
      cache.hotRetentionSecs,
      "taikaiCache.hotRetentionSecs",
    ),
    warm_capacity_bytes: assertPositiveIntegerLike(
      cache.warmCapacityBytes,
      "taikaiCache.warmCapacityBytes",
    ),
    warm_retention_secs: assertPositiveIntegerLike(
      cache.warmRetentionSecs,
      "taikaiCache.warmRetentionSecs",
    ),
    cold_capacity_bytes: assertPositiveIntegerLike(
      cache.coldCapacityBytes,
      "taikaiCache.coldCapacityBytes",
    ),
    cold_retention_secs: assertPositiveIntegerLike(
      cache.coldRetentionSecs,
      "taikaiCache.coldRetentionSecs",
    ),
    qos: {
      priority_rate_bps: assertPositiveIntegerLike(
        qos.priorityRateBps,
        "taikaiCache.qos.priorityRateBps",
      ),
      standard_rate_bps: assertPositiveIntegerLike(
        qos.standardRateBps,
        "taikaiCache.qos.standardRateBps",
      ),
      bulk_rate_bps: assertPositiveIntegerLike(
        qos.bulkRateBps,
        "taikaiCache.qos.bulkRateBps",
      ),
      burst_multiplier: burst,
    },
  };

  if (cache.reliability != null) {
    const reliability = {};
    if (
      cache.reliability.failuresToTrip !== undefined &&
      cache.reliability.failuresToTrip !== null
    ) {
      reliability.failures_to_trip = assertPositiveIntegerLike(
        cache.reliability.failuresToTrip,
        "taikaiCache.reliability.failuresToTrip",
      );
    }
    if (
      cache.reliability.openSecs !== undefined &&
      cache.reliability.openSecs !== null
    ) {
      reliability.open_secs = assertPositiveIntegerLike(
        cache.reliability.openSecs,
        "taikaiCache.reliability.openSecs",
      );
    }
    result.reliability = reliability;
  }

  return result;
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

function normaliseQosCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { priority: 0, standard: 0, bulk: 0 };
  }
  return {
    priority: toSafeNumber(raw.priority ?? 0),
    standard: toSafeNumber(raw.standard ?? 0),
    bulk: toSafeNumber(raw.bulk ?? 0),
  };
}

function normaliseTierCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { hot: 0, warm: 0, cold: 0 };
  }
  return {
    hot: toSafeNumber(raw.hot ?? 0),
    warm: toSafeNumber(raw.warm ?? 0),
    cold: toSafeNumber(raw.cold ?? 0),
  };
}

function normaliseEvictionCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { expired: 0, capacity: 0 };
  }
  return {
    expired: toSafeNumber(raw.expired ?? 0),
    capacity: toSafeNumber(raw.capacity ?? 0),
  };
}

function transformTaikaiCacheSummary(raw) {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const evictions = raw.evictions || {};
  const promotions = raw.promotions || {};
  return {
    hits: normaliseTierCounts(raw.hits),
    misses: toSafeNumber(raw.misses ?? 0),
    inserts: normaliseTierCounts(raw.inserts),
    evictions: {
      hot: normaliseEvictionCounts(evictions.hot),
      warm: normaliseEvictionCounts(evictions.warm),
      cold: normaliseEvictionCounts(evictions.cold),
    },
    promotions: {
      warmToHot: toSafeNumber(promotions.warm_to_hot ?? 0),
      coldToWarm: toSafeNumber(promotions.cold_to_warm ?? 0),
      coldToHot: toSafeNumber(promotions.cold_to_hot ?? 0),
    },
    qosDenials: normaliseQosCounts(raw.qos_denials),
  };
}

function transformTaikaiCacheQueue(raw) {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  return {
    pendingSegments: toSafeNumber(raw.pending_segments ?? 0),
    pendingBytes: toSafeNumber(raw.pending_bytes ?? 0),
    pendingBatches: toSafeNumber(raw.pending_batches ?? 0),
    inFlightBatches: toSafeNumber(raw.in_flight_batches ?? 0),
    hedgedBatches: toSafeNumber(raw.hedged_batches ?? 0),
    shaperDenials: normaliseQosCounts(raw.shaper_denials),
    droppedSegments: toSafeNumber(raw.dropped_segments ?? 0),
    failovers: toSafeNumber(raw.failovers ?? 0),
    openCircuits: toSafeNumber(raw.open_circuits ?? 0),
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
    taikaiCacheSummary: transformTaikaiCacheSummary(raw.taikai_cache_summary),
    taikaiCacheQueue: transformTaikaiCacheQueue(raw.taikai_cache_queue),
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
  const baseOptions = options ?? {};
  if (!isPlainObject(baseOptions)) {
    throw new TypeError("sorafsGatewayFetch options must be a plain object");
  }
  const normalizedManifestId = normalizeHex32(manifestIdHex, "manifestIdHex");
  const normalizedChunkerHandle = assertNonEmptyString(chunkerHandle, "chunkerHandle");
  const normalizedPlanJson = assertNonEmptyString(planJson, "planJson");
  const { __nativeBinding: injectedBinding, ...restOptions } = baseOptions;
  const binding = injectedBinding ?? requireSorafsNativeBinding();
  if (!binding || typeof binding.sorafsGatewayFetch !== "function") {
    throw new Error(
      "sorafsGatewayFetch requires the native iroha_js_host module. Run `npm run build:native` before using this helper.",
    );
  }
  if (!Array.isArray(providers) || providers.length === 0) {
    throw new TypeError("providers must be a non-empty array");
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
  const nativeOptions = normaliseGatewayOptions(restOptions);
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
