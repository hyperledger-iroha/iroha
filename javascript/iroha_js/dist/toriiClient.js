import { createHash, randomBytes } from "node:crypto";

import {
  resolveToriiClientConfig,
  extractConfidentialGasConfig,
} from "./config.js";
import { getNativeBinding } from "./native.js";
import {
  canonicalizeMultihashHex,
  ensureCanonicalAccountId,
  normalizeAccountId,
  normalizeAssetId,
  normalizeAssetHoldingId,
  normalizeIdentifierInput,
  normalizeOpaqueLiteral,
  normalizeRwaId,
} from "./normalizers.js";
import { AccountAddressError } from "./address.js";
import {
  buildDaIngestRequest,
  deriveDaChunkerHandle,
  generateDaProofSummary,
  emitDaProofSummaryArtifact,
} from "./dataAvailability.js";
import { sorafsGatewayFetch } from "./sorafs.js";
import { buildPacs008Message, buildPacs009Message } from "./isoBridge.js";
import { looksLikeIban, normalizeIban } from "./identifiers.js";
import {
  createValidationError,
  ValidationErrorCode,
  ValidationError,
} from "./validationError.js";
import { buildCanonicalRequestHeaders } from "./canonicalRequest.js";
import { blake2b256 } from "./blake2b.js";
import { SM2_DEFAULT_DISTINGUISHED_ID, verifyEd25519, verifySm2 } from "./crypto.js";
import { getCurveEntryByPublicKeyMulticodec } from "./curveRegistry.js";

const DEFAULT_PAGE_SIZE = 100;

const DEFAULT_SUCCESS_STATUSES = ["Approved", "Committed", "Applied"];
const DEFAULT_FAILURE_STATUSES = ["Rejected", "Expired"];
const DEFAULT_TX_STATUS_POLL_INTERVAL_MS = 1_000;
const DEFAULT_TX_STATUS_TIMEOUT_MS = 30_000;
const DEFAULT_ISO_POLL_INTERVAL_MS = 2_000;
const DEFAULT_ISO_POLL_ATTEMPTS = 12;
const EXPECTED_DATA_MODEL_VERSION = 1;
const MIN_ISO_POLL_INTERVAL_MS = 10;
const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_INTEGER_BIGINT = BigInt(MAX_SAFE_INTEGER);
const MAX_NUMERIC_SCALE = 28;
const MAX_NUMERIC_BITS = 512;
const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const RAW_UTF8_HEADERS_INIT_KEY = "__irohaRawUtf8Headers";
const RAW_UTF8_HEADER_SUPPORT_FLAG = "__irohaSupportsRawUtf8Headers";
const BFV_IDENTIFIER_SCHEMA_NAME =
  "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext";
const BFV_IDENTIFIER_SEED_BYTES = 32;
const BFV_IDENTIFIER_SHA512_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.prg.v1",
  "utf8",
);
const BFV_IDENTIFIER_SLOT_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.slot.v1",
  "utf8",
);
const BFV_IDENTIFIER_U_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.u.v1",
  "utf8",
);
const BFV_IDENTIFIER_E1_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.e1.v1",
  "utf8",
);
const BFV_IDENTIFIER_E2_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.e2.v1",
  "utf8",
);
const CRC64_REFLECTED_POLY = 0xc96c5795d7870f42n;
const DA_FETCH_ARTIFACT_PREFIX = "artifacts/da/fetch_";
const DA_PROVE_ARTIFACT_PREFIX = "artifacts/da/prove_availability_";
const TX_STATUS_POLL_OPTION_KEYS = new Set([
  "signal",
  "intervalMs",
  "timeoutMs",
  "maxAttempts",
  "successStatuses",
  "failureStatuses",
  "onStatus",
]);
const OFFLINE_SETTLEMENT_AND_WAIT_OPTION_KEYS = new Set([
  "signal",
  ...TX_STATUS_POLL_OPTION_KEYS,
]);
const GET_METRICS_OPTION_KEYS = new Set(["asText", "signal"]);
const CONNECT_APP_LIST_OPTION_KEYS = new Set(["limit", "cursor", "signal"]);
const GET_TX_STATUS_OPTION_KEYS = new Set([
  "allowShortHash",
  "signal",
  "scope",
  "endpoints",
]);

const ISO_NON_TERMINAL_STATUS_VALUES = new Set(["pending", "accepted"]);
const ISO_STATUS_VALUES = new Map([
  ["pending", "Pending"],
  ["accepted", "Accepted"],
  ["rejected", "Rejected"],
  ["committed", "Committed"],
]);
const PACS002_STATUS_CODES = new Set(["ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"]);

function resolveNativeBinding() {
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function decodeTransactionReceiptPayload(payload) {
  const native = resolveNativeBinding();
  if (!native || typeof native.decodeTransactionReceiptJson !== "function") {
    return null;
  }
  try {
    const json = native.decodeTransactionReceiptJson(payload);
    return JSON.parse(json);
  } catch {
    return null;
  }
}

const HEADER_SORA_PROOF = "sora-proof";
const HEADER_SORA_NAME = "sora-name";
const HEADER_SORA_PROOF_STATUS = "sora-proof-status";
const HEADER_SORA_PDP_COMMITMENT = "sora-pdp-commitment";

const EVIDENCE_KIND_VALUES = new Set([
  "DoublePrepare",
  "DoubleCommit",
  "InvalidQc",
  "InvalidProposal",
  "Censorship",
]);

const EVIDENCE_PHASE_VALUES = new Set(["Prepare", "Commit", "NewView"]);

const KAIGI_HEALTH_STATUS_VALUES = new Set(["healthy", "degraded", "unavailable"]);
const KAIGI_EVENT_KIND_VALUES = new Set(["registration", "health"]);
const KAIGI_CALL_EVENT_KIND_VALUES = new Set(["roster_updated", "ended"]);
const SORAFS_REPLICATION_STATUS_VALUES = new Set(["pending", "completed", "expired"]);
const SORAFS_PIN_STATUS_VALUES = new Set(["pending", "approved", "retired"]);
const UAID_MANIFEST_STATUS_VALUES = new Set(["Pending", "Active", "Expired", "Revoked"]);
const VERIFYING_KEY_STATUS_VALUES = new Set([
  "Proposed",
  "Active",
  "Withdrawn",
]);
const VERIFYING_KEY_STATUS_ALIASES = new Map(
  [...VERIFYING_KEY_STATUS_VALUES].map((value) => [value.toLowerCase(), value]),
);
const SCCP_FINALITY_MODEL_VALUES = new Set([
  "EthereumBeaconExecution",
  "BscValidatorSet",
  "SolanaFinalizedSlot",
  "TonMasterchain",
  "TronDpos",
  "SubstrateGrandpa",
]);
const SCCP_VERIFIER_TARGET_VALUES = new Set([
  "EvmContract",
  "SolanaProgram",
  "TonContract",
  "TronContract",
  "SubstrateRuntime",
]);
const SCCP_HUB_MESSAGE_KIND_VALUES = new Set([
  "Burn",
  "TokenAdd",
  "TokenPause",
  "TokenResume",
  "AssetRegister",
  "RouteActivate",
  "Transfer",
]);
const SCCP_CHAIN_FAMILY_VALUES = new Set(["Evm", "Solana", "Ton", "Tron", "Substrate"]);
const SCCP_MESSAGE_PAYLOAD_KIND_VALUES = new Set(["AssetRegister", "RouteActivate", "Transfer"]);
const SUBSCRIPTION_STATUS_VALUES = new Set([
  "active",
  "paused",
  "past_due",
  "canceled",
  "suspended",
]);
const SUBSCRIPTION_PLAN_LIST_OPTION_KEYS = new Set([
  "provider",
  "limit",
  "offset",
  "signal",
]);
const SUBSCRIPTION_LIST_OPTION_KEYS = new Set([
  "owned_by",
  "ownedBy",
  "provider",
  "status",
  "limit",
  "offset",
  "signal",
]);

const CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      if ((crc & 1n) !== 0n) {
        crc = (crc >> 1n) ^ CRC64_REFLECTED_POLY;
      } else {
        crc >>= 1n;
      }
    }
    table[index] = crc;
  }
  return table;
})();

function isSecureProtocol(protocol) {
  return protocol === "https:" || protocol === "wss:";
}

function isAbsoluteUrl(candidate) {
  if (!candidate) {
    return false;
  }
  if (candidate instanceof URL) {
    return true;
  }
  if (typeof candidate !== "string") {
    return false;
  }
  return /^[a-z][a-z0-9+.-]*:\/\//iu.test(candidate);
}

const OFFLINE_ALLOWANCE_STRING_FIELDS = new Set([
  "certificate_id_hex",
  "controller_id",
  "asset_id",
  "verdict_id_hex",
  "attestation_nonce_hex",
]);
const OFFLINE_ALLOWANCE_NUMERIC_FIELDS = new Set([
  "registered_at_ms",
  "certificate_expires_at_ms",
  "policy_expires_at_ms",
  "refresh_at_ms",
]);
const OFFLINE_ALLOWANCE_EXISTS_FIELDS = new Set([
  ...OFFLINE_ALLOWANCE_STRING_FIELDS,
  ...OFFLINE_ALLOWANCE_NUMERIC_FIELDS,
]);
const OFFLINE_TRANSFER_STRING_FIELDS = new Set([
  "bundle_id_hex",
  "controller_id",
  "receiver_id",
  "deposit_account_id",
  "asset_id",
  "status",
  "platform_policy",
  "certificate_id_hex",
  "verdict_id_hex",
  "attestation_nonce_hex",
]);
const OFFLINE_TRANSFER_NUMERIC_FIELDS = new Set([
  "receipt_count",
  "recorded_at_ms",
  "recorded_at_height",
  "archived_at_height",
  "certificate_expires_at_ms",
  "policy_expires_at_ms",
  "refresh_at_ms",
]);
const OFFLINE_TRANSFER_EXISTS_FIELDS = new Set([
  ...OFFLINE_TRANSFER_STRING_FIELDS,
  ...OFFLINE_TRANSFER_NUMERIC_FIELDS,
]);
const OFFLINE_REJECTION_STATS_OPTION_KEYS = new Set(["telemetryProfile", "signal"]);
const OFFLINE_REVOCATION_STRING_FIELDS = new Set([
  "verdict_id_hex",
  "issuer_id",
  "reason",
  "note",
]);
const OFFLINE_REVOCATION_NUMERIC_FIELDS = new Set(["revoked_at_ms"]);
const OFFLINE_REVOCATION_EXISTS_FIELDS = new Set([
  ...OFFLINE_REVOCATION_STRING_FIELDS,
  ...OFFLINE_REVOCATION_NUMERIC_FIELDS,
]);
const EMPTY_FILTER_FIELD_SET = new Set();
const SNS_CONTROLLER_TYPES = new Set(["Account", "Multisig", "ResolverTemplate", "ExternalLink"]);
const SNS_NAME_STATUS_VALUES = new Set(["Active", "GracePeriod", "Redemption", "Frozen", "Tombstoned"]);
const SNS_SUFFIX_STATUS_VALUES = new Set(["Active", "Paused", "Revoked"]);
const SNS_AUCTION_KIND_VALUES = new Set(["VickreyCommitReveal", "DutchReopen"]);
const SNS_GOV_CASE_DISPUTE_TYPES = new Set([
  "ownership",
  "policy_violation",
  "abuse",
  "billing",
  "other",
]);
const SNS_GOV_CASE_PRIORITY_VALUES = new Set(["urgent", "high", "standard", "info"]);
const SNS_GOV_CASE_STATUS_VALUES = new Set([
  "open",
  "triage",
  "hearing",
  "decision",
  "remediation",
  "closed",
  "suspended",
]);
const SNS_GOV_CASE_REPORTER_ROLES = new Set([
  "registrar",
  "steward",
  "guardian",
  "public",
  "support",
]);
const SNS_GOV_CASE_RESPONDENT_ROLES = new Set([
  "registrant",
  "controller",
  "registrar",
  "steward",
  "other",
]);
const SNS_GOV_CASE_EVIDENCE_KIND_VALUES = new Set([
  "document",
  "screenshot",
  "log",
  "governance",
  "other",
]);
const SNS_GOV_CASE_DECISION_FINDINGS = new Set(["upheld", "rejected", "partial", "withdrawn"]);
const ITERABLE_LIST_OPTION_KEYS = new Set([
  "limit",
  "offset",
  "filter",
  "sort",
  "signal",
  "canonicalAuth",
]);
const ASSET_ID_LIST_OPTION_KEYS = new Set([
  ...ITERABLE_LIST_OPTION_KEYS,
  "assetId",
]);
const OFFLINE_ITERABLE_OPTION_KEYS = new Set([
  ...ITERABLE_LIST_OPTION_KEYS,
  "controllerId",
  "receiverId",
  "depositAccountId",
  "assetId",
  "certificateExpiresBeforeMs",
  "certificateExpiresAfterMs",
  "policyExpiresBeforeMs",
  "policyExpiresAfterMs",
  "refreshBeforeMs",
  "refreshAfterMs",
  "verdictIdHex",
  "attestationNonceHex",
  "certificateIdHex",
  "requireVerdict",
  "onlyMissingVerdict",
  "platformPolicy",
  "includeExpired",
]);
const ITERABLE_OPTION_KEYS = new Set([
  "limit",
  "offset",
  "filter",
  "sort",
  "signal",
  "fetchSize",
  "fetch_size",
  "queryName",
  "query_name",
  "select",
  "pageSize",
  "maxItems",
  "contains",
  "hashPrefix",
  "hash_prefix",
  "namespace",
  "controllerId",
  "receiverId",
  "depositAccountId",
  "assetId",
  "certificateExpiresBeforeMs",
  "certificateExpiresAfterMs",
  "policyExpiresBeforeMs",
  "policyExpiresAfterMs",
  "refreshBeforeMs",
  "refreshAfterMs",
  "verdictIdHex",
  "attestationNonceHex",
  "certificateIdHex",
  "requireVerdict",
  "onlyMissingVerdict",
  "platformPolicy",
  "includeExpired",
  "canonicalAuth",
]);
const SORAFS_ALIAS_ITERATOR_OPTION_KEYS = new Set([
  "namespace",
  "manifestDigestHex",
  "limit",
  "offset",
  "pageSize",
  "maxItems",
  "signal",
]);
const SORAFS_PIN_ITERATOR_OPTION_KEYS = new Set([
  "status",
  "limit",
  "offset",
  "pageSize",
  "maxItems",
  "signal",
]);
const SORAFS_REPLICATION_ITERATOR_OPTION_KEYS = new Set([
  "status",
  "manifestDigestHex",
  "limit",
  "offset",
  "pageSize",
  "maxItems",
  "signal",
]);
const ITERABLE_QUERY_OPTION_KEYS = new Set([
  "limit",
  "offset",
  "filter",
  "sort",
  "fetchSize",
  "queryName",
  "select",
  "signal",
  "canonicalAuth",
]);
const EXPLORER_NFT_LIST_OPTION_KEYS = new Set([
  "page",
  "perPage",
  "limit",
  "offset",
  "ownedBy",
  "owned_by",
  "domainId",
  "domain_id",
  "domain",
  "pageSize",
  "maxItems",
  "signal",
]);
const EXPLORER_NFT_ITERATOR_OPTION_KEYS = new Set([
  ...EXPLORER_NFT_LIST_OPTION_KEYS,
  "pageSize",
  "maxItems",
]);
const EXPLORER_RWA_LIST_OPTION_KEYS = new Set([
  "page",
  "perPage",
  "limit",
  "offset",
  "ownedBy",
  "owned_by",
  "domainId",
  "domain_id",
  "domain",
  "pageSize",
  "maxItems",
  "signal",
]);
const EXPLORER_RWA_ITERATOR_OPTION_KEYS = new Set([
  ...EXPLORER_RWA_LIST_OPTION_KEYS,
  "pageSize",
  "maxItems",
]);
const UPLOAD_ATTACHMENT_OPTION_KEYS = new Set(["contentType", "content_type"]);
const SNS_GOV_CASE_PUBLICATION_STATES = new Set(["public", "redacted", "sealed"]);

const OFFLINE_SUMMARY_STRING_FIELDS = new Set(["certificate_id_hex", "controller_id"]);
const OFFLINE_SUMMARY_EXISTS_FIELDS = new Set([...OFFLINE_SUMMARY_STRING_FIELDS]);

const OFFLINE_ALLOWANCE_FILTER_RULES = Object.freeze({
  stringFields: OFFLINE_ALLOWANCE_STRING_FIELDS,
  numericFields: OFFLINE_ALLOWANCE_NUMERIC_FIELDS,
  rangeFields: OFFLINE_ALLOWANCE_NUMERIC_FIELDS,
  existsFields: OFFLINE_ALLOWANCE_EXISTS_FIELDS,
});

const OFFLINE_TRANSFER_FILTER_RULES = Object.freeze({
  stringFields: OFFLINE_TRANSFER_STRING_FIELDS,
  numericFields: OFFLINE_TRANSFER_NUMERIC_FIELDS,
  rangeFields: OFFLINE_TRANSFER_NUMERIC_FIELDS,
  existsFields: OFFLINE_TRANSFER_EXISTS_FIELDS,
});

const OFFLINE_REVOCATION_FILTER_RULES = Object.freeze({
  stringFields: OFFLINE_REVOCATION_STRING_FIELDS,
  numericFields: OFFLINE_REVOCATION_NUMERIC_FIELDS,
  rangeFields: OFFLINE_REVOCATION_NUMERIC_FIELDS,
  existsFields: OFFLINE_REVOCATION_EXISTS_FIELDS,
});
const OFFLINE_SUMMARY_FILTER_RULES = Object.freeze({
  stringFields: OFFLINE_SUMMARY_STRING_FIELDS,
  numericFields: EMPTY_FILTER_FIELD_SET,
  rangeFields: EMPTY_FILTER_FIELD_SET,
  existsFields: OFFLINE_SUMMARY_EXISTS_FIELDS,
});

export class TransactionStatusError extends Error {
  constructor(hashHex, status, payload) {
    const statusLabel = status == null ? "unknown" : String(status);
    const rejectionReason = extractPipelineRejectionReason(payload);
    const reasonSuffix = rejectionReason ? ` (reason=${rejectionReason})` : "";
    super(`Transaction ${hashHex} reported failure status ${statusLabel}${reasonSuffix}`);
    this.name = "TransactionStatusError";
    this.hashHex = hashHex;
    this.status = status;
    this.payload = payload;
    this.rejectionReason = rejectionReason;
  }
}

export class TransactionTimeoutError extends Error {
  constructor(message, hashHex, attempts, payload) {
    super(message);
    this.name = "TransactionTimeoutError";
    this.hashHex = hashHex;
    this.attempts = attempts;
    this.payload = payload;
  }
}

export class IsoMessageTimeoutError extends Error {
  constructor(messageId, attempts, lastStatus) {
    super(`ISO message ${messageId} did not reach a terminal status after ${attempts} attempts`);
    this.name = "IsoMessageTimeoutError";
    this.messageId = messageId;
    this.attempts = attempts;
    this.lastStatus = lastStatus ?? null;
  }
}

export class ToriiDataModelCompatibilityError extends Error {
  constructor(expected, actual, cause) {
    const actualLabel = actual == null ? "missing" : String(actual);
    super(`Torii data model version mismatch (expected ${expected}, got ${actualLabel}).`);
    this.name = "ToriiDataModelCompatibilityError";
    this.expected = expected;
    this.actual = actual ?? null;
    if (cause !== undefined) {
      this.cause = cause;
    }
  }
}

export class ToriiHttpError extends Error {
  constructor({
    status,
    expected,
    statusText,
    code,
    rejectCode,
    errorMessage,
    bodyText,
    bodyJson,
  }) {
    const expectedLabel =
      Array.isArray(expected) && expected.length > 0
        ? expected.slice().sort((a, b) => a - b).join(", ")
        : "none";
    const statusLabel = statusText ? `${status} ${statusText}` : String(status);
    const detailParts = [];
    if (rejectCode && rejectCode !== code) {
      detailParts.push(`reject=${rejectCode}`);
    }
    if (code) {
      detailParts.push(code);
    }
    if (errorMessage && (!code || errorMessage !== code)) {
      detailParts.push(errorMessage);
    }
    if (detailParts.length === 0 && bodyText) {
      detailParts.push(bodyText);
    }
    const suffix = detailParts.length > 0 ? `: ${detailParts.join(" — ")}` : "";
    super(`Torii responded with HTTP ${statusLabel} (expected ${expectedLabel})${suffix}`);
    this.name = "ToriiHttpError";
    this.status = status;
    this.statusText = statusText ?? null;
    this.expected = Array.isArray(expected) ? [...expected] : [];
    this.code = code ?? null;
    this.rejectCode = rejectCode ?? null;
    this.errorMessage = errorMessage ?? null;
    this.bodyText = bodyText ?? null;
    this.bodyJson = bodyJson ?? null;
  }
}

/**
 * Extract the pipeline status kind from a Torii pipeline payload.
 * @param {unknown} payload
 * @returns {string | null}
 */
export function extractPipelineStatusKind(payload) {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const direct = coerceStatusKind(payload.status);
  if (direct) {
    return direct;
  }
  const content = payload.content;
  if (content && typeof content === "object") {
    return coerceStatusKind(content.status);
  }
  return null;
}

/**
 * Extract a rejection reason string from a Torii pipeline payload when available.
 * @param {unknown} payload
 * @returns {string | null}
 */
export function extractPipelineRejectionReason(payload) {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const direct = coerceRejectionReason(
    payload.rejection_reason
      ?? payload.rejectionReason
      ?? payload.reason
      ?? payload.reject_code
      ?? payload.rejectCode,
  );
  if (direct) {
    return direct;
  }
  const content = payload.content;
  if (!content || typeof content !== "object") {
    return null;
  }
  const nested = coerceRejectionReason(
    content.rejection_reason
      ?? content.rejectionReason
      ?? content.reason
      ?? content.reject_code
      ?? content.rejectCode,
  );
  if (nested) {
    return nested;
  }
  const status = content.status;
  if (!status || typeof status !== "object") {
    return null;
  }
  const fromStatus = coerceRejectionReason(
    status.rejection_reason
      ?? status.rejectionReason
      ?? status.reason
      ?? status.reject_code
      ?? status.rejectCode,
  );
  if (fromStatus) {
    return fromStatus;
  }
  const statusKind = status.kind == null ? null : String(status.kind);
  if (
    statusKind &&
    statusKind.toLowerCase() === "rejected" &&
    typeof status.content === "string"
  ) {
    return coerceRejectionReason(status.content);
  }
  return null;
}

export function decodePdpCommitmentHeader(headers) {
  const raw = readHeaderValue(headers, HEADER_SORA_PDP_COMMITMENT);
  if (raw == null) {
    return null;
  }
  const value = String(raw).trim();
  if (!value) {
    throw new Error("Failed to decode Sora-PDP-Commitment header: payload is empty");
  }
  try {
    return strictDecodeBase64(value);
  } catch (err) {
    const message = err && typeof err.message === "string" ? err.message : String(err);
    throw new Error(`Failed to decode Sora-PDP-Commitment header: ${message}`);
  }
}

function coerceStatusKind(value) {
  if (!value) {
    return null;
  }
  if (typeof value === "object" && "kind" in value) {
    const kind = value.kind;
    return kind == null ? null : String(kind);
  }
  return String(value);
}

function coerceRejectionReason(value) {
  if (value == null) {
    return null;
  }
  const text = String(value).trim();
  return text ? text : null;
}

function normalizeStatusSet(input, fallback) {
  if (!input) {
    return new Set(fallback.map((status) => String(status)));
  }
  const result = new Set();
  for (const value of input) {
    result.add(String(value));
  }
  return result;
}

function normalizeTransactionStatusScope(value, context, fallback = "auto") {
  const raw = value === undefined || value === null ? fallback : value;
  const normalized = String(raw).trim().toLowerCase();
  if (!normalized) {
    return fallback;
  }
  if (normalized === "local" || normalized === "auto" || normalized === "global") {
    return normalized;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${context} must be one of: local, auto, global`,
    context,
  );
}

function normalizeStatusEndpointCandidates(primaryBaseUrl, endpoints, context) {
  const normalized = [];
  const seen = new Set();
  const pushEndpoint = (value, pathLabel) => {
    const text = String(value ?? "").trim();
    if (!text) {
      return;
    }
    let parsed;
    try {
      parsed = new URL(text);
    } catch {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${pathLabel} must be a valid absolute URL`,
        pathLabel,
      );
    }
    parsed.pathname = "";
    parsed.search = "";
    parsed.hash = "";
    const canonical = parsed.toString().replace(/\/+$/u, "");
    if (!seen.has(canonical)) {
      seen.add(canonical);
      normalized.push(canonical);
    }
  };

  pushEndpoint(primaryBaseUrl, `${context}.primary`);
  if (endpoints === undefined || endpoints === null) {
    return normalized;
  }

  let values;
  if (typeof endpoints === "string") {
    values = endpoints.split(",").map((entry) => entry.trim());
  } else if (Array.isArray(endpoints)) {
    values = endpoints;
  } else if (typeof endpoints[Symbol.iterator] === "function") {
    values = [...endpoints];
  } else {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must be a string or iterable of URLs`,
      context,
    );
  }

  values.forEach((value, index) => {
    pushEndpoint(value, `${context}[${index}]`);
  });
  return normalized;
}

function readHeaderValue(headers, name) {
  if (!headers) {
    return null;
  }
  if (typeof headers.get === "function") {
    return headers.get(name) ?? headers.get(name.toLowerCase());
  }
  const lower = name.toLowerCase();
  if (headers instanceof Map) {
    return headers.get(name) ?? headers.get(lower);
  }
  if (typeof headers === "object") {
    const direct = headers[name];
    if (typeof direct === "string") {
      return direct;
    }
    const fallback = headers[lower];
    if (typeof fallback === "string") {
      return fallback;
    }
  }
  return null;
}

function strictDecodeBase64(value) {
  const compact = value.replace(/\s+/gu, "");
  validateBase64Alphabet(compact);
  if (typeof Buffer !== "undefined") {
    const decoded = Buffer.from(compact, "base64");
    ensureBase64RoundTrip(decoded, compact);
    return Uint8Array.from(decoded);
  }
  if (typeof atob === "function") {
    let binary;
    try {
      binary = atob(compact);
    } catch (error) {
      throw error instanceof Error ? error : new Error(String(error));
    }
    const reencoded = typeof btoa === "function" ? btoa(binary) : null;
    if (reencoded && !base64StringsEquivalent(reencoded, compact)) {
      throw new Error("invalid base64 payload");
    }
    const decoded = new Uint8Array(binary.length);
    for (let idx = 0; idx < binary.length; idx += 1) {
      decoded[idx] = binary.charCodeAt(idx);
    }
    return decoded;
  }
  throw new Error("no base64 decoder available");
}

const BASE64_ALPHABET_PATTERN = /^[A-Za-z0-9+/]*={0,2}$/u;
const BASE64_DATA_PATTERN = /[A-Za-z0-9+/]/u;

function validateBase64Alphabet(value) {
  if (!value) {
    throw new Error("payload is empty");
  }
  if (!BASE64_ALPHABET_PATTERN.test(value) || !BASE64_DATA_PATTERN.test(value)) {
    throw new Error("invalid base64 payload");
  }
}

function ensureBase64RoundTrip(buffer, original) {
  const canonical =
    buffer.length === 0 ? "" : Buffer.from(buffer).toString("base64");
  if (!base64StringsEquivalent(canonical, original)) {
    throw new Error("invalid base64 payload");
  }
}

function base64StringsEquivalent(left, right) {
  return stripBase64Padding(left) === stripBase64Padding(right);
}

function stripBase64Padding(value) {
  return value.replace(/=+$/u, "");
}

function sortJsonForErrorMessage(value) {
  if (Array.isArray(value)) {
    return value.map((item) => sortJsonForErrorMessage(item));
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const sorted = {};
  for (const key of Object.keys(value).sort()) {
    sorted[key] = sortJsonForErrorMessage(value[key]);
  }
  return sorted;
}

/**
 * Minimal Torii HTTP client mirroring the Python helper.
 *
 * Provides attachment and prover-report utilities needed by Python
 * developers; more endpoints will be added as the broader JS SDK grows.
 */

/**
 * @typedef {Object} BlockListOptions
 * @property {number} [offsetHeight]
 * @property {number} [limit]
 *
 * @typedef {Object} EventStreamOptions
 * @property {string | Record<string, unknown>} [filter]
 * @property {string} [lastEventId]
 * @property {AbortSignal} [signal]
 *
 * @typedef {Object} IterableListOptions
 * @property {number} [limit]
 * @property {number} [offset]
 * @property {string | Record<string, unknown>} [filter]
 * @property {string | ReadonlyArray<{key: string, order?: "asc" | "desc"}>} [sort]
 * @property {AbortSignal} [signal]
 * @property {string} [assetId]
 * @property {number} [certificateExpiresBeforeMs]
 * @property {number} [certificateExpiresAfterMs]
 * @property {number} [policyExpiresBeforeMs]
 * @property {number} [policyExpiresAfterMs]
 * @property {string} [verdictIdHex]
 * @property {boolean} [requireVerdict]
 * @property {boolean} [onlyMissingVerdict]
 *
 * @typedef {IterableListOptions & {
 *   fetchSize?: number;
 *   queryName?: string;
 *   select?: ReadonlyArray<Record<string, unknown>>;
 * }} IterableQueryOptions
 *
 * @typedef {IterableListOptions & {
 *   pageSize?: number;
 *   maxItems?: number;
 * }} PaginationIteratorOptions
 *
 * @typedef {IterableListOptions & { assetId?: string; }} AccountAssetListOptions
 * @typedef {IterableListOptions & { assetId?: string; }} AccountTransactionListOptions
 * @typedef {IterableListOptions & { assetId?: string; }} AssetHolderListOptions
 *
 * @typedef {PaginationIteratorOptions & { assetId?: string; }} AccountAssetIteratorOptions
 * @typedef {PaginationIteratorOptions & { assetId?: string; }} AccountTransactionIteratorOptions
 * @typedef {PaginationIteratorOptions & { assetId?: string; }} AssetHolderIteratorOptions
 *
 * @template [T=unknown]
 * @typedef {Object} SseEvent
 * @property {string | null} event
 * @property {T | string} data
 * @property {string | null} id
 * @property {number | null | undefined} [retry]
 * @property {string | null} raw
 */
export class ToriiClient {
  /**
   * @param {string} baseUrl Base Torii URL (e.g. http://localhost:8080).
 * @param {object} [options]
 * @param {typeof fetch} [options.fetchImpl] Custom fetch implementation.
 * @param {Record<string, unknown>} [options.config] Optional iroha_config payload.
 * @param {number} [options.timeoutMs]
 * @param {number} [options.maxRetries]
 * @param {number} [options.backoffInitialMs]
 * @param {number} [options.backoffMultiplier]
 * @param {number} [options.maxBackoffMs]
 * @param {ReadonlyArray<number>} [options.retryStatuses]
 * @param {ReadonlyArray<string>} [options.retryMethods]
 * @param {Record<string, string>} [options.defaultHeaders]
 * @param {string} [options.authToken]
 * @param {string} [options.apiToken]
 * @param {"local"|"auto"|"global"} [options.transactionStatusScope]
 * @param {ReadonlyArray<string> | string} [options.statusEndpoints]
 * @param {typeof sorafsGatewayFetch} [options.sorafsGatewayFetch] Custom gateway fetch hook (tests).
 * @param {object} [options.sorafsAliasPolicy] Override SoraFS alias cache TTLs (seconds).
 * @param {(warning: {alias: string | null, evaluation: {state: string | null, statusLabel: string | null, rotationDue: boolean, ageSeconds: number | null, generatedAtUnix: number | null, expiresAtUnix: number | null, expiresInSeconds: number | null, servable: boolean}}) => void} [options.onSorafsAliasWarning]
 * @param {(manifest: Buffer, payload: Buffer, options: Record<string, unknown>) => unknown} [options.generateDaProofSummary] Custom proof summary generator (tests).
 */
  constructor(baseUrl, options = {}) {
    if (!baseUrl) {
      throw new Error("baseUrl is required");
    }
    const opts =
      options === undefined || options === null
        ? {}
        : requirePlainObjectOption(options, "ToriiClient options");
    this._baseUrl = baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
    this._fetch = opts.fetchImpl ?? opts.fetch ?? globalThis.fetch;
    if (typeof this._fetch !== "function") {
      throw new Error("fetch implementation is required");
    }
    if (
      opts.sorafsGatewayFetch !== undefined &&
      typeof opts.sorafsGatewayFetch !== "function"
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "ToriiClient options.sorafsGatewayFetch must be a function",
        "ToriiClient.options.sorafsGatewayFetch",
      );
    }
    this._sorafsGatewayFetch =
      typeof opts.sorafsGatewayFetch === "function" ? opts.sorafsGatewayFetch : sorafsGatewayFetch;
    if (
      opts.generateDaProofSummary !== undefined &&
      typeof opts.generateDaProofSummary !== "function"
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "ToriiClient options.generateDaProofSummary must be a function when provided",
        "ToriiClient.options.generateDaProofSummary",
      );
    }
    this._generateDaProofSummary =
      typeof opts.generateDaProofSummary === "function"
        ? opts.generateDaProofSummary
        : generateDaProofSummary;
    this._allowInsecure = Boolean(
      opts.allowInsecure ??
        (opts.config &&
          typeof opts.config === "object" &&
          opts.config.allowInsecure),
    );
    const overrides = { ...opts };
    delete overrides.fetchImpl;
    delete overrides.fetch;
    delete overrides.config;
    delete overrides.allowInsecure;
    delete overrides.sorafsAliasPolicy;
    delete overrides.onSorafsAliasWarning;
    delete overrides.sorafsGatewayFetch;
    delete overrides.generateDaProofSummary;
    this._config = resolveToriiClientConfig({
      config: opts.config,
      overrides,
    });
    if (
      opts.sorafsAliasPolicy !== undefined &&
      opts.sorafsAliasPolicy !== null &&
      !isPlainObject(opts.sorafsAliasPolicy)
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "sorafsAliasPolicy must be a plain object when provided",
        "ToriiClient.options.sorafsAliasPolicy",
      );
    }
    this._sorafsPolicyOverrides = opts.sorafsAliasPolicy ? { ...opts.sorafsAliasPolicy } : null;
    this._sorafsResolvedPolicy = null;
    if (
      opts.onSorafsAliasWarning !== undefined &&
      opts.onSorafsAliasWarning !== null &&
      typeof opts.onSorafsAliasWarning !== "function"
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "onSorafsAliasWarning must be a function when provided",
        "ToriiClient.options.onSorafsAliasWarning",
      );
    }
    this._sorafsAliasWarningHook =
      typeof opts.onSorafsAliasWarning === "function" ? opts.onSorafsAliasWarning : null;
    this._statusState = createStatusSnapshotState();
    this._dataModelCompatibility = { status: "unknown", actual: null };
    this._dataModelCompatibilityPromise = null;
    const parsedBase = new URL(this._baseUrl.endsWith("/") ? this._baseUrl : `${this._baseUrl}/`);
    this._baseOrigin = `${parsedBase.protocol}//${parsedBase.host}`;
    this._baseHost = parsedBase.host;
    this._baseProtocol = parsedBase.protocol.toLowerCase();
    const hasCredentials =
      Boolean(this._config.authToken || this._config.apiToken) ||
      headersContainCredentials(this._config.defaultHeaders);
    if (hasCredentials && !this._allowInsecure && !isSecureProtocol(this._baseProtocol)) {
      throw new Error(
        "ToriiClient: auth/api tokens require an https base URL; pass allowInsecure: true for local/dev use only.",
      );
    }
  }

  /**
   * List accounts (`GET /v1/accounts`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async listAccounts(options = {}) {
    return this._listIterable("/v1/accounts", options, normalizeAccountListResponse);
  }

  /**
   * Query accounts (`POST /v1/accounts/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async queryAccounts(options = {}) {
    return this._queryIterable("/v1/accounts/query", options, normalizeAccountListResponse);
  }

  /**
   * Iterate over accounts using automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateAccounts(options = {}) {
    return this._iterateIterable(this.listAccounts, options);
  }

  /**
   * Iterate accounts via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateAccountsQuery(options = {}) {
    return this._iterateIterable(this.queryAccounts, options);
  }

  /**
   * List domains (`GET /v1/domains`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async listDomains(options = {}) {
    return this._listIterable("/v1/domains", options, normalizeDomainListResponse);
  }

  /**
   * Query domains (`POST /v1/domains/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async queryDomains(options = {}) {
    return this._queryIterable("/v1/domains/query", options, normalizeDomainListResponse);
  }

  /**
   * Iterate domains with automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateDomains(options = {}) {
    return this._iterateIterable(this.listDomains, options);
  }

  /**
   * Iterate domains via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateDomainsQuery(options = {}) {
    return this._iterateIterable(this.queryDomains, options);
  }

  /**
   * List asset definitions (`GET /v1/assets/definitions`).
   * Returned items expose the full asset-definition record and may include
   * `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`.
   * Structured filters/sorts accept `alias_binding.status`,
   * `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms`, and
   * `alias_binding.bound_at_ms`.
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: Array<object>, total: number}>}
   */
  async listAssetDefinitions(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "listAssetDefinitions",
    );
    this._assertPermissionRequirement(requirePermissions, "listAssetDefinitions");
    return this._listIterable(
      "/v1/assets/definitions",
      rest,
      normalizeAssetDefinitionListResponse,
    );
  }

  /**
   * Query asset definitions (`POST /v1/assets/definitions/query`).
   * Returned items expose the full asset-definition record and may include
   * `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`.
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<object>, total: number}>}
   */
  async queryAssetDefinitions(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "queryAssetDefinitions",
    );
    this._assertPermissionRequirement(requirePermissions, "queryAssetDefinitions");
    return this._queryIterable(
      "/v1/assets/definitions/query",
      rest,
      normalizeAssetDefinitionListResponse,
    );
  }

  /**
   * Iterate over asset definitions using automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateAssetDefinitions(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateAssetDefinitions",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateAssetDefinitions");
    return this._iterateIterable(this.listAssetDefinitions, rest);
  }

  /**
   * Iterate asset definitions via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateAssetDefinitionsQuery(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateAssetDefinitionsQuery",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateAssetDefinitionsQuery");
    return this._iterateIterable(this.queryAssetDefinitions, rest);
  }

  /**
   * List repo agreements (`GET /v1/repo/agreements`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: ReadonlyArray<ToriiRepoAgreement>, total: number}>}
   */
  async listRepoAgreements(options = {}) {
    return this._listIterable(
      "/v1/repo/agreements",
      options,
      normalizeRepoAgreementListResponse,
    );
  }

  /**
   * Query repo agreements (`POST /v1/repo/agreements/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: ReadonlyArray<ToriiRepoAgreement>, total: number}>}
   */
  async queryRepoAgreements(options = {}) {
    return this._queryIterable(
      "/v1/repo/agreements/query",
      options,
      normalizeRepoAgreementListResponse,
    );
  }

  /**
   * Iterate repo agreements with automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiRepoAgreement, void, unknown>}
   */
  iterateRepoAgreements(options = {}) {
    return this._iterateIterable(this.listRepoAgreements, options);
  }

  /**
   * Iterate repo agreements using the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiRepoAgreement, void, unknown>}
   */
  iterateRepoAgreementsQuery(options = {}) {
    return this._iterateIterable(this.queryRepoAgreements, options);
  }

  /**
   * List NFTs (`GET /v1/nfts`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async listNfts(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "listNfts",
    );
    this._assertPermissionRequirement(requirePermissions, "listNfts");
    return this._listIterable("/v1/nfts", rest, normalizeNftListResponse);
  }

  /**
   * Query NFTs (`POST /v1/nfts/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async queryNfts(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "queryNfts",
    );
    this._assertPermissionRequirement(requirePermissions, "queryNfts");
    return this._queryIterable("/v1/nfts/query", rest, normalizeNftListResponse);
  }

  /**
   * Iterate over NFTs using automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateNfts(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateNfts",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateNfts");
    return this._iterateIterable(this.listNfts, rest);
  }

  /**
   * Iterate NFTs via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateNftsQuery(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateNftsQuery",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateNftsQuery");
    return this._iterateIterable(this.queryNfts, rest);
  }

  /**
   * List explorer NFTs with optional owner/domain filters (`GET /v1/explorer/nfts`).
   * @param {ExplorerNftListOptions} [options]
   * @returns {Promise<{pagination: ToriiExplorerPaginationMeta, items: Array<{id: string, ownedBy: string, metadata: unknown}>}>}
   */
  async listExplorerNfts(options = {}) {
    const normalized = ToriiClient._normalizeExplorerNftListOptions(
      options,
      "listExplorerNfts options",
    );
    const params = {
      page: normalized.page,
      per_page: normalized.perPage,
    };
    if (normalized.ownedBy !== undefined) {
      params.owned_by = normalized.ownedBy;
    }
    if (normalized.domain !== undefined) {
      params.domain = normalized.domain;
    }
    const response = await this._request("GET", "/v1/explorer/nfts", {
      params,
      headers: { Accept: "application/json" },
      signal: normalized.signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer nfts endpoint returned no payload");
    }
    return normalizeExplorerNftPage(payload);
  }

  /**
   * Iterate explorer NFTs with optional owner/domain filters.
   * @param {ExplorerNftIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string, ownedBy: string, metadata: unknown}, void, unknown>}
   */
  iterateExplorerNfts(options = {}) {
    const normalized = ToriiClient._normalizeExplorerNftIteratorOptions(
      options,
      "iterateExplorerNfts options",
    );
    const { maxItems, ...listOptions } = normalized;
    const self = this;
    return (async function* iterator() {
      let page = normalized.page;
      let remaining = maxItems;
      while (true) {
        const pageResult = await self.listExplorerNfts({
          ...listOptions,
          page,
          perPage: normalized.perPage,
        });
        const items = Array.isArray(pageResult?.items) ? pageResult.items : [];
        if (items.length === 0) {
          return;
        }
        for (const item of items) {
          yield item;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        const { pagination } = pageResult;
        if (
          (pagination && pagination.totalPages && page >= pagination.totalPages) ||
          items.length < normalized.perPage
        ) {
          return;
        }
        page += 1;
      }
    })();
  }

  /**
   * List NFTs owned by an account (`GET /v1/explorer/nfts?owned_by=...`).
   * @param {string} accountId
   * @param {ExplorerNftListOptions} [options]
   * @returns {Promise<{pagination: ToriiExplorerPaginationMeta, items: Array<{id: string, ownedBy: string, metadata: unknown}>}>}
   */
  async listAccountNfts(accountId, options = {}) {
    const normalizedId = ToriiClient._normalizeAccountId(accountId, "accountId");
    return this.listExplorerNfts({ ...options, ownedBy: normalizedId });
  }

  /**
   * Iterate NFTs owned by an account.
   * @param {string} accountId
   * @param {ExplorerNftIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string, ownedBy: string, metadata: unknown}, void, unknown>}
   */
  iterateAccountNfts(accountId, options = {}) {
    const normalizedId = ToriiClient._normalizeAccountId(accountId, "accountId");
    return this.iterateExplorerNfts({ ...options, ownedBy: normalizedId });
  }

  /**
   * List RWAs (`GET /v1/rwas`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async listRwas(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "listRwas",
    );
    this._assertPermissionRequirement(requirePermissions, "listRwas");
    return this._listIterable("/v1/rwas", rest, normalizeRwaListResponse);
  }

  /**
   * Query RWAs (`POST /v1/rwas/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{id: string}>, total: number}>}
   */
  async queryRwas(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "queryRwas",
    );
    this._assertPermissionRequirement(requirePermissions, "queryRwas");
    return this._queryIterable("/v1/rwas/query", rest, normalizeRwaListResponse);
  }

  /**
   * Iterate over RWAs using automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateRwas(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateRwas",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateRwas");
    return this._iterateIterable(this.listRwas, rest);
  }

  /**
   * Iterate RWAs via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{id: string}, void, unknown>}
   */
  iterateRwasQuery(options = {}) {
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateRwasQuery",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateRwasQuery");
    return this._iterateIterable(this.queryRwas, rest);
  }

  /**
   * List explorer RWAs with optional owner/domain filters (`GET /v1/explorer/rwas`).
   * @param {ExplorerRwaListOptions} [options]
   * @returns {Promise<{pagination: ToriiExplorerPaginationMeta, items: Array<object>}>}
   */
  async listExplorerRwas(options = {}) {
    const normalized = ToriiClient._normalizeExplorerRwaListOptions(
      options,
      "listExplorerRwas options",
    );
    const params = {
      page: normalized.page,
      per_page: normalized.perPage,
    };
    if (normalized.ownedBy !== undefined) {
      params.owned_by = normalized.ownedBy;
    }
    if (normalized.domain !== undefined) {
      params.domain = normalized.domain;
    }
    const response = await this._request("GET", "/v1/explorer/rwas", {
      params,
      headers: { Accept: "application/json" },
      signal: normalized.signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer rwas endpoint returned no payload");
    }
    return normalizeExplorerRwaPage(payload);
  }

  /**
   * Get explorer detail for a single RWA (`GET /v1/explorer/rwas/{rwaId}`).
   * @param {string} rwaId
   * @param {{ signal?: AbortSignalLike }} [options]
   * @returns {Promise<object>}
   */
  async getExplorerRwaDetail(rwaId, options = {}) {
    const normalizedId = normalizeRwaId(rwaId, "rwaId");
    const { signal } = ToriiClient._normalizeOptionsWithSignal(options, "getExplorerRwaDetail");
    const response = await this._request(
      "GET",
      `/v1/explorer/rwas/${encodeURIComponent(normalizedId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer rwa detail endpoint returned no payload");
    }
    return normalizeExplorerRwaRecord(payload, "explorer rwa detail response");
  }

  /**
   * Iterate explorer RWAs with optional owner/domain filters.
   * @param {ExplorerRwaIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateExplorerRwas(options = {}) {
    const normalized = ToriiClient._normalizeExplorerRwaIteratorOptions(
      options,
      "iterateExplorerRwas options",
    );
    const { maxItems, ...listOptions } = normalized;
    const self = this;
    return (async function* iterator() {
      let page = normalized.page;
      let remaining = maxItems;
      while (true) {
        const pageResult = await self.listExplorerRwas({
          ...listOptions,
          page,
          perPage: normalized.perPage,
        });
        const items = Array.isArray(pageResult?.items) ? pageResult.items : [];
        if (items.length === 0) {
          return;
        }
        for (const item of items) {
          yield item;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        const { pagination } = pageResult;
        if (
          (pagination && pagination.totalPages && page >= pagination.totalPages) ||
          items.length < normalized.perPage
        ) {
          return;
        }
        page += 1;
      }
    })();
  }

  /**
   * List explorer RWAs owned by an account (`GET /v1/explorer/rwas?owned_by=...`).
   * @param {string} accountId
   * @param {ExplorerRwaListOptions} [options]
   * @returns {Promise<{pagination: ToriiExplorerPaginationMeta, items: Array<object>}>}
   */
  async listAccountRwas(accountId, options = {}) {
    const normalizedId = ToriiClient._normalizeAccountId(accountId, "accountId");
    return this.listExplorerRwas({ ...options, ownedBy: normalizedId });
  }

  /**
   * Iterate explorer RWAs owned by an account.
   * @param {string} accountId
   * @param {ExplorerRwaIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateAccountRwas(accountId, options = {}) {
    const normalizedId = ToriiClient._normalizeAccountId(accountId, "accountId");
    return this.iterateExplorerRwas({ ...options, ownedBy: normalizedId });
  }

  /**
   * List asset holdings belonging to an account (`GET /v1/accounts/{id}/assets`).
   * @param {string} accountId
   * @param {AccountAssetListOptions} [options]
   * @returns {Promise<{items: Array<{asset_id: string, quantity: string}>, total: number}>}
   */
  async listAccountAssets(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const encodedId = encodeURIComponent(normalizedId);
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "listAccountAssets",
    );
    this._assertPermissionRequirement(requirePermissions, "listAccountAssets");
    return this._listIterable(
      `/v1/accounts/${encodedId}/assets`,
      rest,
      normalizeAccountAssetListResponse,
      ASSET_ID_LIST_OPTION_KEYS,
    );
  }

  /**
   * Query asset holdings belonging to an account (`POST /v1/accounts/{id}/assets/query`).
   * @param {string} accountId
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{asset_id: string, quantity: string}>, total: number}>}
   */
  async queryAccountAssets(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const encodedId = encodeURIComponent(normalizedId);
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "queryAccountAssets",
    );
    this._assertPermissionRequirement(requirePermissions, "queryAccountAssets");
    return this._queryIterable(
      `/v1/accounts/${encodedId}/assets/query`,
      rest,
      normalizeAccountAssetListResponse,
    );
  }

  /**
   * Iterate over an account's asset holdings.
   * @param {string} accountId
   * @param {AccountAssetIteratorOptions} [options]
   * @returns {AsyncGenerator<{asset_id: string, quantity: string}, void, unknown>}
   */
  iterateAccountAssets(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateAccountAssets",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateAccountAssets");
    return this._iterateIterable(this.listAccountAssets.bind(this, normalizedId), rest);
  }

  /**
   * Iterate per-account asset balances via the query endpoint.
   * @param {string} accountId
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{asset_id: string, quantity: string}, void, unknown>}
   */
  iterateAccountAssetsQuery(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const { requirePermissions, options: rest } = ToriiClient._splitPermissionedIterableOptions(
      options,
      "iterateAccountAssetsQuery",
    );
    this._assertPermissionRequirement(requirePermissions, "iterateAccountAssetsQuery");
    return this._iterateIterable(this.queryAccountAssets.bind(this, normalizedId), rest);
  }

  /**
   * List transactions involving an account (`GET /v1/accounts/{id}/transactions`).
   * @param {string} accountId
   * @param {AccountTransactionListOptions} [options]
   * @returns {Promise<{items: Array<object>, total: number}>}
   */
  async listAccountTransactions(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const encodedId = encodeURIComponent(normalizedId);
    return this._listIterable(
      `/v1/accounts/${encodedId}/transactions`,
      options,
      normalizeAccountTransactionListResponse,
      ASSET_ID_LIST_OPTION_KEYS,
    );
  }

  /**
   * Query transactions involving an account (`POST /v1/accounts/{id}/transactions/query`).
   * @param {string} accountId
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<object>, total: number}>}
   */
  async queryAccountTransactions(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const encodedId = encodeURIComponent(normalizedId);
    return this._queryIterable(
      `/v1/accounts/${encodedId}/transactions/query`,
      options,
      normalizeAccountTransactionListResponse,
    );
  }

  /**
   * Iterate over transactions involving an account.
   * @param {string} accountId
   * @param {AccountTransactionIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateAccountTransactions(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    return this._iterateIterable(
      this.listAccountTransactions.bind(this, normalizedId),
      options,
    );
  }

  /**
   * Iterate per-account transactions via the structured query endpoint.
   * @param {string} accountId
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<object, void, unknown>}
   */
  iterateAccountTransactionsQuery(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    return this._iterateIterable(
      this.queryAccountTransactions.bind(this, normalizedId),
      options,
    );
  }

  /**
   * List holders for an asset definition (`GET /v1/assets/{definitionId}/holders`).
   * @param {string} assetDefinitionId
   * @param {AssetHolderListOptions} [options]
   * @returns {Promise<{items: Array<{account_id: string, quantity: string}>, total: number}>}
   */
  async listAssetHolders(assetDefinitionId, options = {}) {
    const normalizedId = ToriiClient._requireAssetDefinitionId(assetDefinitionId);
    const encodedId = encodeURIComponent(normalizedId);
    return this._listIterable(
      `/v1/assets/${encodedId}/holders`,
      options,
      normalizeAssetHolderListResponse,
      ASSET_ID_LIST_OPTION_KEYS,
    );
  }

  /**
   * Query holders for an asset definition (`POST /v1/assets/{definitionId}/holders/query`).
   * @param {string} assetDefinitionId
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<{items: Array<{account_id: string, quantity: string}>, total: number}>}
   */
  async queryAssetHolders(assetDefinitionId, options = {}) {
    const normalizedId = ToriiClient._requireAssetDefinitionId(assetDefinitionId);
    const encodedId = encodeURIComponent(normalizedId);
    return this._queryIterable(
      `/v1/assets/${encodedId}/holders/query`,
      options,
      normalizeAssetHolderListResponse,
    );
  }

  /**
   * Iterate over holders for an asset definition.
   * @param {string} assetDefinitionId
   * @param {AssetHolderIteratorOptions} [options]
   * @returns {AsyncGenerator<{account_id: string, quantity: string}, void, unknown>}
   */
  iterateAssetHolders(assetDefinitionId, options = {}) {
    const normalizedId = ToriiClient._requireAssetDefinitionId(assetDefinitionId);
    return this._iterateIterable(this.listAssetHolders.bind(this, normalizedId), options);
  }

  /**
   * Iterate asset-definition holders via the query endpoint.
   * @param {string} assetDefinitionId
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{account_id: string, quantity: string}, void, unknown>}
   */
  iterateAssetHoldersQuery(assetDefinitionId, options = {}) {
    const normalizedId = ToriiClient._requireAssetDefinitionId(assetDefinitionId);
    return this._iterateIterable(this.queryAssetHolders.bind(this, normalizedId), options);
  }

  /**
   * List permission tokens granted directly to an account (`GET /v1/accounts/{accountId}/permissions`).
   * @param {string} accountId
   * @param {{limit?: number, offset?: number, signal?: AbortSignal}} [options]
   * @returns {Promise<{items: Array<{name: string, payload: unknown}>, total: number}>}
   */
  async listAccountPermissions(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const encodedId = encodeURIComponent(normalizedId);
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listAccountPermissions",
    );
    const params = ToriiClient._encodePaginationParams(rest);
    const response = await this._request(
      "GET",
      `/v1/accounts/${encodedId}/permissions`,
      {
        headers: { Accept: "application/json" },
        params: params ?? undefined,
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    const base = ToriiClient._validateIterablePayload(payload);
    return normalizeAccountPermissionListResponse(base);
  }

  /**
   * Iterate permission tokens granted directly to an account.
   * @param {string} accountId
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<{name: string, payload: unknown}, void, unknown>}
   */
  iterateAccountPermissions(accountId, options = {}) {
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    return this._iterateIterable(
      this.listAccountPermissions.bind(this, normalizedId),
      options,
    );
  }

  /**
   * Upload an attachment to Torii.
   * @param {ArrayBufferView | ArrayBuffer | string} data Attachment bytes.
   * @param {{contentType: string}} options Attachment metadata.
   * @returns {Promise<ToriiAttachmentMetadata>} Attachment metadata returned by Torii.
   */
  async uploadAttachment(data, options = {}) {
    const normalizedOptions =
      options === undefined || options === null
        ? {}
        : requirePlainObjectOption(options, "uploadAttachment options");
    assertSupportedOptionKeys(
      normalizedOptions,
      UPLOAD_ATTACHMENT_OPTION_KEYS,
      "uploadAttachment options",
    );
    const normalizedContentType = requireNonEmptyString(
      pickOverride(normalizedOptions, "content_type", "contentType"),
      "uploadAttachment options.contentType",
    );
    const body = normalizeAttachmentUploadPayload(data, "uploadAttachment data");
    const response = await this._request("POST", "/v1/zk/attachments", {
      headers: { "Content-Type": normalizedContentType },
      body,
    });
    await this._expectStatus(response, [201]);
    const payload = await this._maybeJson(response);
    return normalizeAttachmentMetadataRecord(payload, "upload attachment response");
  }

  /**
   * List stored attachments.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ReadonlyArray<ToriiAttachmentMetadata>>}
   */
  async listAttachments(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listAttachments");
    const response = await this._request("GET", "/v1/zk/attachments", {
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeAttachmentMetadataList(payload);
  }

  /**
   * Fetch an attachment and its optional content type.
   * @param {string} attachmentId Attachment identifier.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<{data: Buffer, contentType: string | null}>}
   */
  async getAttachment(attachmentId, options = {}) {
    const normalizedId = requireNonEmptyString(attachmentId, "attachmentId");
    const { signal } = normalizeSignalOnlyOption(options, "getAttachment");
    const response = await this._request("GET", `/v1/zk/attachments/${normalizedId}`, {
      signal,
    });
    await this._expectStatus(response, [200]);
    const arrayBuffer = await response.arrayBuffer();
    const contentType = this._getHeader(response, "content-type");
    return { data: Buffer.from(arrayBuffer), contentType: contentType ?? null };
  }

  /**
   * Delete an attachment by id.
   * @param {string} attachmentId
   */
  async deleteAttachment(attachmentId) {
    const normalizedId = requireNonEmptyString(attachmentId, "attachmentId");
    const response = await this._request("DELETE", `/v1/zk/attachments/${normalizedId}`);
    await this._expectStatus(response, [200, 202, 204, 404]);
  }

  /**
   * List verifying keys stored in the registry (`GET /v1/zk/vk`).
   * @param {ToriiVerifyingKeyListOptions} [options]
   * @returns {Promise<unknown>}
   */
  async listVerifyingKeys(options = {}) {
    const { signal, params } = buildVerifyingKeyListQuery(options);
    const response = await this._request("GET", "/v1/zk/vk", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    return this._maybeJson(response);
  }

  /**
   * List verifying keys and normalise the payload.
   * @param {ToriiVerifyingKeyListOptions} [options]
   * @returns {Promise<ReadonlyArray<ToriiVerifyingKeyListItem>>}
   */
  async listVerifyingKeysTyped(options = {}) {
    const payload = await this.listVerifyingKeys(options);
    return normalizeVerifyingKeyListPayload(payload);
  }

  /**
   * Iterate verifying-key registry entries with offset-based pagination.
   * @param {{pageSize?: number, maxItems?: number, signal?: AbortSignal} & ToriiVerifyingKeyListOptions} [options]
   * @returns {AsyncGenerator<ToriiVerifyingKeyListItem, void, unknown>}
   */
  iterateVerifyingKeys(options = {}) {
    const fetchPage = async (pageOptions = {}) => {
      const items = await this.listVerifyingKeysTyped(pageOptions);
      return { items: items ?? [] };
    };
    return this._iterateOffsetIterable(
      fetchPage,
      options,
      VERIFYING_KEY_ITERATOR_OPTION_KEYS,
    );
  }

  /**
   * Fetch a verifying key record (`GET /v1/zk/vk/{backend}/{name}`).
   * @param {string} backend
   * @param {string} name
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<unknown>}
   */
  async getVerifyingKey(backend, name, options = {}) {
    const normalizedBackend = encodeURIComponent(
      requireNonEmptyString(backend, "getVerifyingKey backend"),
    );
    const normalizedName = encodeURIComponent(
      requireNonEmptyString(name, "getVerifyingKey name"),
    );
    const { signal } = normalizeSignalOnlyOption(options, "getVerifyingKey");
    const response = await this._request(
      "GET",
      `/v1/zk/vk/${normalizedBackend}/${normalizedName}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    return this._maybeJson(response);
  }

  /**
   * Fetch a verifying key record and normalise the payload.
   * @param {string} backend
   * @param {string} name
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiVerifyingKeyDetail>}
   */
  async getVerifyingKeyTyped(backend, name, options = {}) {
    const payload = await this.getVerifyingKey(backend, name, options);
    return normalizeVerifyingKeyDetail(payload);
  }

  /**
   * Submit a verifying key registration request (`POST /v1/zk/vk/register`).
   * @param {ToriiVerifyingKeyRegisterPayload} payload
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<void>}
   */
  async registerVerifyingKey(payload, options = {}) {
    const body = JSON.stringify(normalizeVerifyingKeyRegisterPayload(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "registerVerifyingKey",
    );
    const response = await this._request("POST", "/v1/zk/vk/register", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [202]);
  }

  /**
   * Submit a verifying key update request (`POST /v1/zk/vk/update`).
   * @param {ToriiVerifyingKeyUpdatePayload} payload
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<void>}
   */
  async updateVerifyingKey(payload, options = {}) {
    const body = JSON.stringify(normalizeVerifyingKeyUpdatePayload(payload));
    const { signal } = normalizeSignalOnlyOption(options, "updateVerifyingKey");
    const response = await this._request("POST", "/v1/zk/vk/update", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [202]);
  }

  /**
   * Evaluate the mock alias VOPRF helper (`POST /v1/aliases/voprf/evaluate`).
   * @param {string} blindedElementHex hex-encoded blinded element.
   * @returns {Promise<import("./index").AliasVoprfEvaluateResponse>}
   */
  async evaluateAliasVoprf(blindedElementHex) {
    const payload = {
      blinded_element_hex: requireHexString(blindedElementHex, "blindedElementHex"),
    };
    const response = await this._request("POST", "/v1/aliases/voprf/evaluate", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (
      !json ||
      typeof json.evaluated_element_hex !== "string" ||
      typeof json.backend !== "string"
    ) {
      throw new Error("Unexpected alias VOPRF response payload");
    }
    return {
      evaluated_element_hex: json.evaluated_element_hex,
      backend: json.backend,
    };
  }

  /**
   * Resolve an ISO bridge alias (`POST /v1/aliases/resolve`).
   * Returns null when the alias is missing (404). Throws when the runtime is disabled (503).
  * @param {string} alias
  * @returns {Promise<Record<string, unknown> | null>}
  */
  async resolveAlias(alias) {
    const aliasInput = requireNonEmptyString(alias, "alias");
    const normalizedAlias = looksLikeIban(aliasInput)
      ? normalizeIban(aliasInput, "alias")
      : aliasInput;
    const payload = { alias: normalizedAlias };
    const response = await this._request("POST", "/v1/aliases/resolve", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    if (response.status === 404) {
      return null;
    }
    if (response.status === 503) {
      throw new Error("ISO bridge runtime is disabled on the target node");
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("alias resolve endpoint returned no payload");
    }
    return normalizeAliasResolutionResponse(body, "alias resolve response");
  }

  /**
   * Resolve an ISO bridge alias by deterministic index (`POST /v1/aliases/resolve_index`).
   * Returns null when the index is unknown (404). Throws when the runtime is disabled (503).
   * @param {number | string | bigint} index
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async resolveAliasByIndex(index) {
    const payload = {
      index: ToriiClient._normalizeUnsignedInteger(index, "index", { allowZero: true }),
    };
    const response = await this._request("POST", "/v1/aliases/resolve_index", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    if (response.status === 404) {
      return null;
    }
    if (response.status === 503) {
      throw new Error("ISO bridge runtime is disabled on the target node");
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("alias resolve_index endpoint returned no payload");
    }
    return normalizeAliasResolutionResponse(body, "alias resolve_index response");
  }

  /**
   * List aliases bound to a canonical account id (`POST /v1/aliases/by_account`).
   * Returns null when the account is unknown (404).
   * @param {string} accountId
   * @param {{dataspace?: string, domain?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<{account_id: string, total: number, items: Array<{alias: string, dataspace: string, domain: string | null, is_primary: boolean}>} | null>}
   */
  async lookupAliasesByAccount(accountId, options = {}) {
    const normalizedAccountId = ToriiClient._requireAccountId(accountId, "accountId");
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "lookupAliasesByAccount",
    );
    assertSupportedOptionKeys(
      rest,
      new Set(["dataspace", "domain"]),
      "lookupAliasesByAccount options",
    );
    const dataspace =
      rest.dataspace === undefined
        ? undefined
        : requireNonEmptyString(rest.dataspace, "lookupAliasesByAccount.options.dataspace");
    const domain =
      rest.domain === undefined
        ? undefined
        : requireNonEmptyString(rest.domain, "lookupAliasesByAccount.options.domain");
    const response = await this._request("POST", "/v1/aliases/by_account", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({
        account_id: normalizedAccountId,
        ...(dataspace ? { dataspace } : {}),
        ...(domain ? { domain } : {}),
      }),
      signal,
    });
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("alias by-account endpoint returned no payload");
    }
    return normalizeAliasLookupByAccountResponse(body, "alias by-account response");
  }

  /**
   * List globally registered identifier policies (`GET /v1/identifier-policies`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<{total: number, items: Array<Record<string, unknown>>}>}
   */
  async listIdentifierPolicies(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listIdentifierPolicies",
    );
    assertSupportedOptionKeys(rest, new Set([]), "listIdentifierPolicies options");
    const response = await this._request("GET", "/v1/identifier-policies", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("identifier policy list endpoint returned no payload");
    }
    return normalizeIdentifierPolicyListResponse(payload, "identifier policy list response");
  }

  /**
   * List globally registered RAM-LFE program policies (`GET /v1/ram-lfe/program-policies`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<{total: number, items: Array<Record<string, unknown>>}>}
   */
  async listRamLfeProgramPolicies(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listRamLfeProgramPolicies",
    );
    assertSupportedOptionKeys(rest, new Set([]), "listRamLfeProgramPolicies options");
    const response = await this._request("GET", "/v1/ram-lfe/program-policies", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("ram-lfe program policy list endpoint returned no payload");
    }
    return normalizeRamLfeProgramPolicyListResponse(
      payload,
      "ram-lfe program policy list response",
    );
  }

  /**
   * Resolve an identifier through a hidden-function policy (`POST /v1/identifiers/resolve`).
   * Returns null when the policy or identifier binding is missing (404).
   * @param {{policyId: string, input?: string, encryptedInput?: string, signal?: AbortSignal}} options
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async resolveIdentifier(options) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "resolveIdentifier",
    );
    const payload = buildIdentifierResolveRequest(rest, "resolveIdentifier");
    const response = await this._request("POST", "/v1/identifiers/resolve", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    if (response.status === 404) {
      return null;
    }
    if (response.status === 409) {
      throw new Error("Identifier policy is inactive or the target binding is unavailable");
    }
    if (response.status === 503) {
      throw new Error("Identifier resolver runtime is disabled on the target node");
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("identifier resolve endpoint returned no payload");
    }
    return normalizeIdentifierResolveResponse(body, "identifier resolve response");
  }

  /**
   * Execute one RAM-LFE program from plaintext or BFV-encrypted input
   * (`POST /v1/ram-lfe/programs/{program_id}/execute`).
   * Returns null when the program policy is missing (404).
   * @param {string} programId
   * @param {{inputHex?: string, encryptedInput?: string, signal?: AbortSignal}} options
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async executeRamLfeProgram(programId, options) {
    const normalizedProgramId = requireNonEmptyString(
      programId,
      "executeRamLfeProgram.programId",
    );
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "executeRamLfeProgram",
    );
    const payload = buildRamLfeExecuteRequest(rest, "executeRamLfeProgram");
    const response = await this._request(
      "POST",
      `/v1/ram-lfe/programs/${encodeURIComponent(normalizedProgramId)}/execute`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    if (response.status === 409) {
      throw new Error("RAM-LFE program policy is inactive");
    }
    if (response.status === 503) {
      throw new Error("RAM-LFE runtime is disabled on the target node");
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("ram-lfe execute endpoint returned no payload");
    }
    return normalizeRamLfeExecuteResponse(body, "ram-lfe execute response");
  }

  /**
   * Look up a persisted identifier claim by its receipt hash (`GET /v1/identifiers/receipts/{receipt_hash}`).
   * Returns null when the receipt hash is unknown (404).
   * @param {string} receiptHash
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getIdentifierClaimByReceiptHash(receiptHash, options = {}) {
    const normalizedReceiptHash = normalizeHex32String(
      receiptHash,
      "getIdentifierClaimByReceiptHash.receiptHash",
    );
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "getIdentifierClaimByReceiptHash",
    );
    assertSupportedOptionKeys(
      rest,
      new Set([]),
      "getIdentifierClaimByReceiptHash options",
    );
    const response = await this._request(
      "GET",
      `/v1/identifiers/receipts/${encodeURIComponent(normalizedReceiptHash)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("identifier receipt lookup endpoint returned no payload");
    }
    return normalizeIdentifierClaimLookupResponse(
      body,
      "identifier claim lookup response",
    );
  }

  /**
   * Issue a signed on-chain claim receipt for an account identifier binding.
   * Returns null when the policy or account is missing (404).
   * @param {string} accountId
   * @param {{policyId: string, input?: string, encryptedInput?: string, signal?: AbortSignal}} options
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async issueIdentifierClaimReceipt(accountId, options) {
    const normalizedAccountId = normalizeAccountPathLiteral(accountId, "accountId");
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "issueIdentifierClaimReceipt",
    );
    const payload = buildIdentifierResolveRequest(rest, "issueIdentifierClaimReceipt");
    const response = await this._request(
      "POST",
      `/v1/accounts/${encodeURIComponent(normalizedAccountId)}/identifiers/claim-receipt`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    if (response.status === 409) {
      throw new Error("Identifier claim receipt cannot be issued for this account or policy");
    }
    if (response.status === 503) {
      throw new Error("Identifier resolver runtime is disabled on the target node");
    }
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("identifier claim-receipt endpoint returned no payload");
    }
    return normalizeIdentifierResolveResponse(body, "identifier claim receipt response");
  }

  /**
   * Verify a RAM-LFE execution receipt against the node's registered program policy
   * (`POST /v1/ram-lfe/receipts/verify`).
   * @param {{receipt: Record<string, unknown>, outputHex?: string, signal?: AbortSignal}} options
   * @returns {Promise<Record<string, unknown>>}
   */
  async verifyRamLfeReceipt(options) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "verifyRamLfeReceipt",
    );
    const payload = buildRamLfeReceiptVerifyRequest(rest, "verifyRamLfeReceipt");
    const response = await this._request("POST", "/v1/ram-lfe/receipts/verify", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("ram-lfe receipt verify endpoint returned no payload");
    }
    return normalizeRamLfeReceiptVerifyResponse(
      body,
      "ram-lfe receipt verify response",
    );
  }

  /**
   * List SoraFS alias bindings with attestation metadata (`GET /v1/sorafs/aliases`).
   * @param {{namespace?: string, manifestDigestHex?: string, limit?: number, offset?: number, signal?: AbortSignal}} [options]
   * @returns {Promise<SorafsAliasListResponse>}
   */
  async listSorafsAliases(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listSorafsAliases",
    );
    const params = buildSorafsAliasListParams(rest);
    const response = await this._request("GET", "/v1/sorafs/aliases", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs alias list endpoint returned no payload");
    }
    return normalizeSorafsAliasListResponse(payload);
  }

  /**
   * Iterate SoraFS alias bindings with offset-based pagination.
   * @param {{pageSize?: number, maxItems?: number, signal?: AbortSignal} & SorafsAliasListOptions} [options]
   * @returns {AsyncGenerator<SorafsAliasRecord, void, unknown>}
   */
  iterateSorafsAliases(options = {}) {
    return this._iterateOffsetIterable(
      this.listSorafsAliases,
      options,
      SORAFS_ALIAS_ITERATOR_OPTION_KEYS,
      ["aliases"],
    );
  }

  /**
   * List SoraFS manifests registered in the pin registry (`GET /v1/sorafs/pin`).
   * @param {{status?: "pending"|"approved"|"retired", limit?: number, offset?: number, signal?: AbortSignal}} [options]
   * @returns {Promise<SorafsPinListResponse>}
   */
  async listSorafsPinManifests(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listSorafsPinManifests",
    );
    const params = buildSorafsPinListParams(rest);
    const response = await this._request("GET", "/v1/sorafs/pin", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs pin list endpoint returned no payload");
    }
    return normalizeSorafsPinListResponse(payload);
  }

  /**
   * Iterate SoraFS pin manifests via the list endpoint.
   * @param {{pageSize?: number, maxItems?: number, signal?: AbortSignal} & SorafsPinListOptions} [options]
   * @returns {AsyncGenerator<SorafsPinManifest, void, unknown>}
   */
  iterateSorafsPinManifests(options = {}) {
    return this._iterateOffsetIterable(
      this.listSorafsPinManifests,
      options,
      SORAFS_PIN_ITERATOR_OPTION_KEYS,
      ["manifests"],
    );
  }

  /**
   * List SoraFS replication orders with attestation metadata (`GET /v1/sorafs/replication`).
   * @param {{status?: "pending"|"completed"|"expired", manifestDigestHex?: string, limit?: number, offset?: number, signal?: AbortSignal}} [options]
   * @returns {Promise<SorafsReplicationListResponse>}
   */
  async listSorafsReplicationOrders(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "listSorafsReplicationOrders",
    );
    const params = buildSorafsReplicationListParams(rest);
    const response = await this._request("GET", "/v1/sorafs/replication", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs replication list endpoint returned no payload");
    }
    return normalizeSorafsReplicationListResponse(payload);
  }

  /**
   * Iterate SoraFS replication orders via the list endpoint.
   * @param {{pageSize?: number, maxItems?: number, signal?: AbortSignal} & SorafsReplicationListOptions} [options]
   * @returns {AsyncGenerator<SorafsReplicationOrderRecord, void, unknown>}
   */
  iterateSorafsReplicationOrders(options = {}) {
    return this._iterateOffsetIterable(
      this.listSorafsReplicationOrders,
      options,
      SORAFS_REPLICATION_ITERATOR_OPTION_KEYS,
      ["replication_orders"],
    );
  }

  /**
   * Fetch a SoraFS pin manifest (`GET /v1/sorafs/pin/{digest}`) with alias proof enforcement.
   * @param {string} digestHex Manifest digest (hex string).
   * @param {{ headers?: Record<string, string>, signal?: AbortSignal }} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getSorafsPinManifest(digestHex, options = {}) {
    const normalized = requireHexString(digestHex, "digestHex");
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "getSorafsPinManifest",
    );
    const headers = {
      Accept: "application/json",
      ...(rest.headers ?? {}),
    };
    const response = await this._request(
      "GET",
      `/v1/sorafs/pin/${normalized}`,
      {
        headers,
        signal,
      },
    );
    await this._expectStatus(response, [200, 404]);
    if (response.status === 404) {
      return null;
    }
    this._enforceSorafsAliasPolicy(response);
    return this._maybeJson(response);
  }

  /**
   * Fetch a SoraFS pin manifest with typed output.
   * @param {string} digestHex
   * @param {{ headers?: Record<string, string>, signal?: AbortSignal }} [options]
   * @returns {Promise<SorafsPinManifestResponse>}
   */
  async getSorafsPinManifestTyped(digestHex, options = {}) {
    const payload = await this.getSorafsPinManifest(digestHex, options);
    if (!payload) {
      throw new Error("sorafs pin manifest endpoint returned 404");
    }
    return normalizeSorafsPinManifestResponse(payload);
  }

  /**
   * Register a SoraFS manifest inside the pin registry (`POST /v1/sorafs/pin/register`).
   * Mirrors `iroha sorafs pin register` so SDK callers can avoid shelling out to the CLI.
   * @param {SorafsPinRegisterRequest} input
   * @returns {Promise<Record<string, unknown>>}
   */
  async registerSorafsPinManifest(input = {}) {
    const record = ensureRecord(input ?? {}, "registerSorafsPinManifest input");
    const { signal } = normalizeSignalOption(record, "registerSorafsPinManifest");
    const body = JSON.stringify(
      buildSorafsPinRegisterPayload(record, "registerSorafsPinManifest"),
    );
    const response = await this._request("POST", "/v1/sorafs/pin/register", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs pin register endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Register a SoraFS manifest and return a typed response.
   * @param {SorafsPinRegisterRequest} input
   * @returns {Promise<SorafsPinRegisterResponse>}
   */
  async registerSorafsPinManifestTyped(input = {}) {
    const payload = await this.registerSorafsPinManifest(input);
    return normalizeSorafsPinRegisterResponse(payload);
  }

  /**
   * Pin a SoraFS manifest + payload (`POST /v1/sorafs/storage/pin`).
   * @param {{manifest: ArrayBufferView | ArrayBuffer | Buffer | string, payload: ArrayBufferView | ArrayBuffer | Buffer | string, signal?: AbortSignal}} input
   * @returns {Promise<SorafsPinResponse>}
   */
  async pinSorafsManifest(input) {
    const record = ensureRecord(input ?? {}, "pinSorafsManifest input");
    const manifestValue =
      record.manifest ?? record.manifest_b64 ?? record.manifestB64;
    const payloadBytes =
      record.payload ?? record.payload_b64 ?? record.payloadB64;
    const manifestB64 = normalizeRequiredBase64Payload(
      manifestValue,
      "pinSorafsManifest.manifest",
    );
    const payloadB64 = normalizeRequiredBase64Payload(
      payloadBytes,
      "pinSorafsManifest.payload",
    );
    const body = JSON.stringify({
      manifest_b64: manifestB64,
      payload_b64: payloadB64,
    });
    const response = await this._request("POST", "/v1/sorafs/storage/pin", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal: record.signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs storage pin endpoint returned no payload");
    }
    return normalizeSorafsPinResponse(payload);
  }

  /**
   * Fetch a payload range from SoraFS storage (`POST /v1/sorafs/storage/fetch`).
   * @param {{
   *   manifestIdHex: string;
   *   offset: number | string | bigint;
   *   length: number | string | bigint;
   *   providerIdHex?: string | Buffer | ArrayBuffer | ArrayBufferView | null;
   *   signal?: AbortSignal;
   * }} input
   * @returns {Promise<SorafsFetchResponse>}
   */
  async fetchSorafsPayloadRange(input) {
    const record = ensureRecord(input ?? {}, "fetchSorafsPayloadRange input");
    const manifestId =
      record.manifest_id_hex ??
      record.manifestIdHex ??
      record.manifestId ??
      record.manifest;
    const manifestIdHex = normalizeHex32String(
      manifestId,
      "fetchSorafsPayloadRange.manifestIdHex",
    );
    const offset = ToriiClient._normalizeUnsignedInteger(
      record.offset,
      "fetchSorafsPayloadRange.offset",
      { allowZero: true },
    );
    const length = ToriiClient._normalizeUnsignedInteger(
      record.length,
      "fetchSorafsPayloadRange.length",
      { allowZero: false },
    );
    const providerId =
      record.provider_id_hex ??
      record.providerIdHex ??
      record.providerId ??
      null;
    const body = {
      manifest_id_hex: manifestIdHex,
      offset,
      length,
    };
    if (providerId !== undefined && providerId !== null) {
      body.provider_id_hex = normalizeHex32String(
        providerId,
        "fetchSorafsPayloadRange.providerIdHex",
      );
    }
    const response = await this._request("POST", "/v1/sorafs/storage/fetch", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(body),
      signal: record.signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs storage fetch endpoint returned no payload");
    }
    return normalizeSorafsFetchResponse(payload);
  }

  /**
   * Fetch the latest storage state snapshot (`GET /v1/sorafs/storage/state`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SorafsStorageStateResponse>}
   */
  async getSorafsStorageState(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSorafsStorageState");
    const response = await this._request("GET", "/v1/sorafs/storage/state", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs storage state endpoint returned no payload");
    }
    return normalizeSorafsStorageStateResponse(payload);
  }

  /**
   * Fetch a stored manifest (`GET /v1/sorafs/storage/manifest/{manifest_id_hex}`).
   * @param {string} manifestIdHex
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SorafsManifestResponse>}
   */
  async getSorafsManifest(manifestIdHex, options = {}) {
    const normalized = requireHexString(manifestIdHex, "manifestIdHex");
    const { signal } = normalizeSignalOnlyOption(options, "getSorafsManifest");
    const response = await this._request(
      "GET",
      `/v1/sorafs/storage/manifest/${normalized}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sorafs manifest endpoint returned no payload");
    }
    return normalizeSorafsManifestResponse(payload);
  }

  /**
   * Fetch the canonical DA manifest + chunk plan for a storage ticket (`GET /v1/da/manifests/{ticket}`).
   * @param {string} storageTicketHex
   * @param {{signal?: AbortSignal, blockHashHex?: string}} [options]
   * @returns {Promise<DaManifestFetchResponse>}
   */
  async getDaManifest(storageTicketHex, options = {}) {
    const normalizedTicket = normalizeStorageTicketHex(
      storageTicketHex,
      "storageTicketHex",
    );
    const opts = ensureRecord(options ?? {}, "getDaManifest options");
    const blockHashHex = opts.blockHashHex ?? opts.block_hash_hex ?? null;
    const { blockHashHex: _ignoredBlockHash, block_hash_hex: _ignoredBlockHashSnake, ...rest } =
      opts;
    const { signal } = normalizeSignalOnlyOption(rest, "getDaManifest");
    let path = `/v1/da/manifests/${normalizedTicket}`;
    if (blockHashHex !== null && blockHashHex !== undefined) {
      const normalizedHash = normalizeHex32String(
        blockHashHex,
        "getDaManifest.blockHashHex",
      );
      path = `${path}?block_hash=${normalizedHash}`;
    }
    const response = await this._request(
      "GET",
      path,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("da manifest endpoint returned no payload");
    }
    return normalizeDaManifestFetchResponse(payload);
  }

  /**
   * Fetch a DA manifest bundle and persist the CLI-style artefacts to disk.
   * Writes `manifest_<ticket>.norito`, `manifest_<ticket>.json`, and `chunk_plan_<ticket>.json`
   * into `options.outputDir` (defaults to `artifacts/da/fetch_<timestamp>`).
   * @param {string} storageTicketHex
   * @param {{outputDir?: string, signal?: AbortSignal, label?: string}} [options]
   * @returns {Promise<{manifest: DaManifestFetchResponse, paths: {manifestPath: string, manifestJsonPath: string, chunkPlanPath: string, label: string}, outputDir: string}>}
   */
  async getDaManifestToDir(storageTicketHex, options = {}) {
    const opts = ensureRecord(options ?? {}, "getDaManifestToDir options");
    const outputDir = await resolveDaOutputDir(
      opts.outputDir ?? opts.output_dir,
      DA_FETCH_ARTIFACT_PREFIX,
    );
    const manifest = await this.getDaManifest(storageTicketHex, {
      signal: opts.signal,
      blockHashHex: opts.blockHashHex ?? opts.block_hash_hex,
    });
    const label =
      opts.label ??
      opts.ticketLabel ??
      opts.ticket_label ??
      manifest.manifest_hash_hex ??
      manifest.storage_ticket_hex;
    const paths = await persistDaManifestBundle(manifest, outputDir, label);
    return { manifest, paths, outputDir };
  }

  /**
   * Submit a DA ingest request (`POST /v1/da/ingest`).
   * Mirrors the `iroha da submit` flow so callers can push blobs without shelling out to the CLI.
   * @param {DaIngestRequestInput & {signal?: AbortSignal}} options
   * @returns {Promise<DaIngestSubmitResponse>}
   */
  async submitDaBlob(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "submitDaBlob",
    );
    const { request, artifacts } = buildDaIngestRequest(rest);
    const artifactDir = normalizeOptionalPathInput(
      rest.artifactDir ?? rest.artifact_dir,
      "submitDaBlob.artifactDir",
    );
    const noSubmit =
      rest.noSubmit === true || rest.no_submit === true || rest.dryRun === true;
    let artifactPaths = null;
    if (artifactDir) {
      artifactPaths = await persistDaRequestArtifacts(artifactDir, request);
    }
    if (noSubmit) {
      return {
        status: "prepared",
        duplicate: false,
        receipt: null,
        artifacts,
        pdpCommitmentHeader: null,
        artifactPaths,
      };
    }
    const response = await this._request("POST", "/v1/da/ingest", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(request),
      signal,
    });
    await this._expectStatus(response, [202]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("da ingest endpoint returned no payload");
    }
    const normalized = normalizeDaIngestResponse(payload);
    const pdpHeader =
      response.headers && typeof response.headers.get === "function"
        ? response.headers.get(HEADER_SORA_PDP_COMMITMENT)
        : null;
    if (artifactDir) {
      artifactPaths = await persistDaReceiptArtifacts(
        artifactDir,
        normalized,
        pdpHeader,
        artifactPaths,
      );
    }
    return {
      ...normalized,
      artifacts,
      pdpCommitmentHeader: pdpHeader ?? null,
      artifactPaths,
    };
  }

  /**
   * Fetch a DA payload via the multi-source orchestrator and return the gateway summary.
   * Mirrors the `iroha da prove-availability` manifest+fetch portion.
   * @param {{
   *   storageTicketHex?: string;
   *   manifestBundle?: DaManifestFetchResponse;
   *   chunkPlan?: unknown;
   *   planJson?: string;
   *   chunkerHandle?: string;
   *   gatewayProviders: ReadonlyArray<SorafsGatewayProviderSpec>;
   *   fetchOptions?: SorafsGatewayFetchOptions;
   *   proofSummary?: boolean | { sampleCount?: number; sampleSeed?: number | bigint; leafIndexes?: ReadonlyArray<number | bigint>; };
   *   signal?: AbortSignal;
   * }} [options]
   * @returns {Promise<DaGatewayFetchSession>}
   */
  async fetchDaPayloadViaGateway(options = {}) {
    const record = ensureRecord(options ?? {}, "fetchDaPayloadViaGateway options");
    const { signal } = normalizeSignalOption(record, "fetchDaPayloadViaGateway");
    const providers = normalizeDaGatewayProviders(
      record.gatewayProviders ?? record.providers ?? record.gateway_providers,
      "fetchDaPayloadViaGateway.gatewayProviders",
    );
    let manifestBundle = record.manifestBundle ?? null;
    if (!manifestBundle) {
      const ticket =
        record.storageTicketHex ??
        record.storageTicket ??
        record.ticketHex ??
        record.ticket;
      if (!ticket) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          "fetchDaPayloadViaGateway.storageTicketHex is required when manifestBundle is not provided",
          "fetchDaPayloadViaGateway.storageTicketHex",
        );
      }
      manifestBundle = await this.getDaManifest(ticket, { signal });
  }
  const manifestIdSource =
    record.manifestIdHex ??
    record.manifest_id_hex ??
    manifestBundle.manifest_hash_hex;
    const manifestIdHex = normalizeHex32String(
      manifestIdSource,
      "fetchDaPayloadViaGateway.manifestIdHex",
    );

    const chunkPlanInput =
      record.planJson ??
      record.chunkPlan ??
      record.chunk_plan ??
      manifestBundle.chunk_plan;
    const { planJson, planObject } = normaliseChunkPlanPayload(
      chunkPlanInput,
      "fetchDaPayloadViaGateway.chunkPlan",
    );

    const chunkerHandle = normaliseChunkerHandle(
      record.chunkerHandle,
      manifestBundle,
      "fetchDaPayloadViaGateway",
    );

    const proofSummaryOptions = normalizeProofSummaryOption(
      record.proofSummary,
      "fetchDaPayloadViaGateway.proofSummary",
    );

    const fetchOptions = record.fetchOptions ?? {};
    const gatewayResult = await this._sorafsGatewayFetch(
      manifestIdHex,
      chunkerHandle,
      planJson,
      providers,
      fetchOptions,
    );

    let proofSummary = null;
    if (proofSummaryOptions) {
      const manifestBytes = extractManifestBytesForProof(
        manifestBundle,
        "fetchDaPayloadViaGateway.proofSummary",
      );
      if (!manifestBytes) {
        throw new Error(
          "fetchDaPayloadViaGateway.proofSummary requires manifestBundle.manifest_bytes or manifestBundle.manifest_b64",
        );
      }
      const payloadBuffer = gatewayResult?.payload;
      if (!Buffer.isBuffer(payloadBuffer)) {
        throw new Error(
          "fetchDaPayloadViaGateway.proofSummary requires the gateway payload buffer; ensure sorafsGatewayFetch returned binary payloads",
        );
      }
      proofSummary = await this._generateDaProofSummary(
        manifestBytes,
        payloadBuffer,
        proofSummaryOptions,
      );
    }

    return {
      manifest: manifestBundle,
      manifestIdHex,
      chunkerHandle,
      chunkPlan: planObject,
      chunkPlanJson: planJson,
      gatewayResult,
      proofSummary,
    };
  }

  /**
   * End-to-end DA availability check mirroring `iroha da prove-availability`.
   * Downloads the manifest bundle (unless provided), fetches the payload via the gateway,
   * and writes the artefacts (`manifest_*`, `chunk_plan_*`, `payload_*.car`, `scoreboard.json`,
   * `proof_summary_*.json`) to `options.outputDir` (defaults to `artifacts/da/prove_availability_<timestamp>`).
   * @param {{
   *   storageTicketHex?: string;
   *   manifestBundle?: DaManifestFetchResponse;
   *   gatewayProviders: ReadonlyArray<SorafsGatewayProviderSpec>;
   *   fetchOptions?: SorafsGatewayFetchOptions;
   *   proofSummary?: boolean | { sampleCount?: number; sampleSeed?: number | bigint; leafIndexes?: ReadonlyArray<number | bigint>; } | Record<string, unknown>;
   *   outputDir?: string;
   *   chunkerHandle?: string;
   *   signal?: AbortSignal;
   *   scoreboardPath?: string;
   * }} [options]
   * @returns {Promise<{
   *   manifest: DaManifestFetchResponse;
   *   manifestPaths: {manifestPath: string, manifestJsonPath: string, chunkPlanPath: string, label: string};
   *   payloadPath: string;
   *   scoreboardPath: string | null;
   *   proofSummaryPath: string;
   *   proofSummaryArtifact: unknown;
   *   proofSummary: unknown;
   *   gatewayResult: DaGatewayFetchResult;
   *   outputDir: string;
   * }>}
   */
  async proveDaAvailabilityToDir(options = {}) {
    const record = ensureRecord(options ?? {}, "proveDaAvailabilityToDir options");
    const outputDir = await resolveDaOutputDir(
      record.outputDir,
      DA_PROVE_ARTIFACT_PREFIX,
    );
    const providers = normalizeDaGatewayProviders(
      record.gatewayProviders,
      "proveDaAvailabilityToDir.gatewayProviders",
    );

    let manifest = record.manifestBundle ?? null;
    if (!manifest) {
      const ticket =
        record.storageTicketHex ??
        record.storageTicket ??
        record.ticketHex ??
        record.ticket;
      if (!ticket) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          "proveDaAvailabilityToDir.storageTicketHex is required when manifestBundle is not provided",
          "proveDaAvailabilityToDir.storageTicketHex",
        );
      }
      manifest = await this.getDaManifest(ticket, { signal: record.signal });
    }

    const label =
      record.label ??
      record.ticketLabel ??
      record.ticket_label ??
      manifest.storage_ticket_hex;
    const manifestPaths = await persistDaManifestBundle(manifest, outputDir, label);

    const fetchOptions =
      record.fetchOptions ?? {};
    const scoreboardPath =
      normalizeOptionalPathInput(
        record.scoreboardPath ??
          record.scoreboard_path ??
          fetchOptions.scoreboardOutPath ??
          fetchOptions.scoreboard_out_path,
        "proveDaAvailabilityToDir.scoreboardPath",
      ) ?? (await makeDefaultScoreboardPath(outputDir));
    const mergedFetchOptions = {
      ...fetchOptions,
      scoreboardOutPath: scoreboardPath,
    };

    const session = await this.fetchDaPayloadViaGateway({
      manifestBundle: manifest,
      chunkerHandle: record.chunkerHandle,
      gatewayProviders: providers,
      fetchOptions: mergedFetchOptions,
      proofSummary: record.proofSummary ?? true,
      signal: record.signal,
    });

    const pathModule = await getPathModule();
    const payloadPath = pathModule.join(
      outputDir,
      `payload_${manifestPaths.label}.car`,
    );
    await writeBufferFile(payloadPath, session.gatewayResult.payload);

    if (scoreboardPath) {
      await writeJsonFile(
        scoreboardPath,
        session.gatewayResult.scoreboard ?? null,
      );
    }

    const proofSummaryPath = pathModule.join(
      outputDir,
      `proof_summary_${manifestPaths.label}.json`,
    );
    const proofResult = await emitDaProofSummaryArtifact({
      summary: session.proofSummary ?? undefined,
      manifestBytes: manifest.manifest_bytes,
      payloadBytes: session.gatewayResult.payload,
      proofOptions:
        record.proofSummary && typeof record.proofSummary === "object"
          ? record.proofSummary
          : undefined,
      manifestPath: manifestPaths.manifestPath,
      payloadPath,
      outputPath: proofSummaryPath,
    });

    return {
      manifest,
      manifestPaths,
      payloadPath,
      scoreboardPath,
      proofSummaryPath,
      proofSummaryArtifact: proofResult.artifact,
      proofSummary: proofResult.summary,
      gatewayResult: session.gatewayResult,
      outputDir,
    };
  }

  /**
   * Submit an uptime probe sample (`POST /v1/sorafs/capacity/uptime`).
   * @param {{uptimeSecs: number, observedSecs: number, signal?: AbortSignal}} [input]
   * @returns {Promise<SorafsUptimeObservationResponse>}
   */
  async submitSorafsUptimeObservation(input = {}) {
    const normalizedInput = ensureRecord(
      input ?? {},
      "submitSorafsUptimeObservation input",
    );
    const { signal } = normalizeSignalOption(
      normalizedInput,
      "submitSorafsUptimeObservation",
    );
    const { signal: _ignored, ...record } = normalizedInput;
    assertSupportedOptionKeys(
      record,
      new Set(["uptime_secs", "uptimeSecs", "observed_secs", "observedSecs"]),
      "submitSorafsUptimeObservation input",
    );
    const payload = {
      uptime_secs: ToriiClient._normalizeUnsignedInteger(
        record.uptime_secs ?? record.uptimeSecs,
        "submitSorafsUptimeObservation.uptimeSecs",
        { allowZero: false },
      ),
      observed_secs: ToriiClient._normalizeUnsignedInteger(
        record.observed_secs ?? record.observedSecs,
        "submitSorafsUptimeObservation.observedSecs",
        { allowZero: false },
      ),
    };
    const response = await this._request("POST", "/v1/sorafs/capacity/uptime", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (!json) {
      throw new Error("sorafs capacity uptime endpoint returned no payload");
    }
    return normalizeSorafsUptimeObservationResponse(json);
  }

  /**
   * Record a PoR challenge issued by governance (`POST /v1/sorafs/capacity/por-challenge`).
   * @param {{challenge?: string | ArrayBuffer | ArrayBufferView | Buffer, challengeB64?: string, signal?: AbortSignal}} [input]
   * @returns {Promise<SorafsPorSubmissionResponse>}
   */
  async recordSorafsPorChallenge(input = {}) {
    const normalizedInput = ensureRecord(
      input ?? {},
      "recordSorafsPorChallenge input",
    );
    const { signal } = normalizeSignalOption(
      normalizedInput,
      "recordSorafsPorChallenge",
    );
    const { signal: _ignored, ...record } = normalizedInput;
    assertSupportedOptionKeys(
      record,
      new Set(["challenge", "challenge_b64", "challengeB64"]),
      "recordSorafsPorChallenge input",
    );
    const payload = {
      challenge_b64: normalizeRequiredBase64Payload(
        record.challenge ?? record.challenge_b64 ?? record.challengeB64,
        "recordSorafsPorChallenge.challenge",
      ),
    };
    const response = await this._request(
      "POST",
      "/v1/sorafs/capacity/por-challenge",
      {
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (!json) {
      throw new Error("sorafs capacity por-challenge endpoint returned no payload");
    }
    return normalizeSorafsPorSubmissionResponse(json, "sorafs por challenge response");
  }

  /**
   * Record a PoR proof submitted by a provider (`POST /v1/sorafs/capacity/por-proof`).
   * @param {{proof?: string | ArrayBuffer | ArrayBufferView | Buffer, proofB64?: string, signal?: AbortSignal}} [input]
   * @returns {Promise<SorafsPorSubmissionResponse>}
   */
  async recordSorafsPorProof(input = {}) {
    const normalizedInput = ensureRecord(
      input ?? {},
      "recordSorafsPorProof input",
    );
    const { signal } = normalizeSignalOption(
      normalizedInput,
      "recordSorafsPorProof",
    );
    const { signal: _ignored, ...record } = normalizedInput;
    assertSupportedOptionKeys(
      record,
      new Set(["proof", "proof_b64", "proofB64"]),
      "recordSorafsPorProof input",
    );
    const payload = {
      proof_b64: normalizeRequiredBase64Payload(
        record.proof ?? record.proof_b64 ?? record.proofB64,
        "recordSorafsPorProof.proof",
      ),
    };
    const response = await this._request("POST", "/v1/sorafs/capacity/por-proof", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (!json) {
      throw new Error("sorafs capacity por-proof endpoint returned no payload");
    }
    return normalizeSorafsPorSubmissionResponse(json, "sorafs por proof response");
  }

  /**
   * Record an audit verdict for a PoR challenge (`POST /v1/sorafs/capacity/por-verdict`).
   * @param {{verdict?: string | ArrayBuffer | ArrayBufferView | Buffer, verdictB64?: string, signal?: AbortSignal}} [input]
   * @returns {Promise<SorafsPorVerdictResponse>}
   */
  async recordSorafsPorVerdict(input = {}) {
    const normalizedInput = ensureRecord(
      input ?? {},
      "recordSorafsPorVerdict input",
    );
    const { signal } = normalizeSignalOption(
      normalizedInput,
      "recordSorafsPorVerdict",
    );
    const { signal: _ignored, ...record } = normalizedInput;
    assertSupportedOptionKeys(
      record,
      new Set(["verdict", "verdict_b64", "verdictB64"]),
      "recordSorafsPorVerdict input",
    );
    const payload = {
      verdict_b64: normalizeRequiredBase64Payload(
        record.verdict ?? record.verdict_b64 ?? record.verdictB64,
        "recordSorafsPorVerdict.verdict",
      ),
    };
    const response = await this._request("POST", "/v1/sorafs/capacity/por-verdict", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (!json) {
      throw new Error("sorafs capacity por-verdict endpoint returned no payload");
    }
    return normalizeSorafsPorVerdictResponse(json);
  }

  /**
   * Record a PoR probe observation (`POST /v1/sorafs/capacity/por`).
   * @param {{success: boolean, signal?: AbortSignal}} [input]
   * @returns {Promise<SorafsPorObservationResponse>}
   */
  async submitSorafsPorObservation(input = {}) {
    const normalizedInput = ensureRecord(
      input ?? {},
      "submitSorafsPorObservation input",
    );
    const { signal } = normalizeSignalOption(
      normalizedInput,
      "submitSorafsPorObservation",
    );
    const { signal: _ignored, ...record } = normalizedInput;
    assertSupportedOptionKeys(
      record,
      new Set(["success"]),
      "submitSorafsPorObservation input",
    );
    const payload = {
      success: requireBooleanLike(
        record.success,
        "submitSorafsPorObservation.success",
      ),
    };
    const response = await this._request("POST", "/v1/sorafs/capacity/por", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    if (!json) {
      throw new Error("sorafs capacity por endpoint returned no payload");
    }
    return normalizeSorafsPorObservationResponse(json);
  }

  /**
   * Fetch PoR challenge status snapshots (`GET /v1/sorafs/por/status`).
   * Returns the raw Norito bytes for downstream decoding.
   * @param {SorafsPorStatusOptions} [options]
   * @returns {Promise<Buffer>}
   */
  async getSorafsPorStatus(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "getSorafsPorStatus",
    );
    const params = buildSorafsPorStatusParams(rest);
    const response = await this._request("GET", "/v1/sorafs/por/status", {
      headers: { Accept: "application/x-norito" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    return Buffer.from(await response.arrayBuffer());
  }

  /**
   * Export PoR challenge history (`GET /v1/sorafs/por/export`).
   * Returns Norito bytes covering the requested epoch range.
   * @param {SorafsPorExportOptions} [options]
   * @returns {Promise<Buffer>}
   */
  async exportSorafsPorStatus(options = {}) {
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "exportSorafsPorStatus",
    );
    const params = buildSorafsPorExportParams(rest);
    const response = await this._request("GET", "/v1/sorafs/por/export", {
      headers: { Accept: "application/x-norito" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    return Buffer.from(await response.arrayBuffer());
  }

  /**
   * Fetch the weekly PoR report (`GET /v1/sorafs/por/report/{iso_week}`).
   * Returns Norito bytes for the requested ISO week (e.g., `2026-W05`).
   * @param {SorafsIsoWeekInput} isoWeek
  * @param {{signal?: AbortSignal}} [options]
  * @returns {Promise<Buffer>}
  */
  async getSorafsPorWeeklyReport(isoWeek, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSorafsPorWeeklyReport",
    );
    const label = normalizeIsoWeekLabel(isoWeek, "getSorafsPorWeeklyReport.isoWeek");
    const response = await this._request(
      "GET",
      `/v1/sorafs/por/report/${encodeURIComponent(label)}`,
      {
        headers: { Accept: "application/x-norito" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    return Buffer.from(await response.arrayBuffer());
  }

  /**
   * Fetch aggregated holdings for a UAID (`GET /v1/accounts/{uaid}/portfolio`).
   * @param {string} uaid
   * @param {{assetId?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<UaidPortfolioResponse>}
   */
  async getUaidPortfolio(uaid, options = {}) {
    const canonicalUaid = normalizeUaidLiteral(uaid, "getUaidPortfolio.uaid");
    const { signal, assetId } = normalizeUaidPortfolioOptions(
      options,
      "getUaidPortfolio",
    );
    const params = {};
    if (assetId !== undefined) {
      params.asset_id = assetId;
    }
    const response = await this._request(
      "GET",
      `/v1/accounts/${encodeURIComponent(canonicalUaid)}/portfolio`,
      {
        headers: { Accept: "application/json" },
        signal,
        params: Object.keys(params).length === 0 ? undefined : params,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("uaid portfolio endpoint returned no payload");
    }
    return normalizeUaidPortfolioResponse(payload);
  }

  /**
   * Fetch UAID dataspace bindings (`GET /v1/space-directory/uaids/{uaid}`).
   * @param {string} uaid
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<UaidBindingsResponse>}
   */
  async getUaidBindings(uaid, options = {}) {
    const canonicalUaid = normalizeUaidLiteral(uaid, "getUaidBindings.uaid");
    const { signal } = normalizeSignalOnlyOption(options, "getUaidBindings");
    const response = await this._request(
      "GET",
      `/v1/space-directory/uaids/${encodeURIComponent(canonicalUaid)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("uaid bindings endpoint returned no payload");
    }
    return normalizeUaidBindingsResponse(payload);
  }

  /**
   * Fetch Space Directory manifests for a UAID (`GET /v1/space-directory/uaids/{uaid}/manifests`).
   * @param {string} uaid
   * @param {{dataspaceId?: number, signal?: AbortSignal}} [options]
   * @returns {Promise<UaidManifestsResponse>}
   */
  async getUaidManifests(uaid, options = {}) {
    const canonicalUaid = normalizeUaidLiteral(uaid, "getUaidManifests.uaid");
    const { signal, rest } = ToriiClient._normalizeOptionsWithSignal(
      options,
      "getUaidManifests",
    );
    assertSupportedOptionKeys(rest, new Set(["dataspaceId"]), "getUaidManifests options");
    const params = {};
    if (rest.dataspaceId !== undefined && rest.dataspaceId !== null) {
      params.dataspace = ToriiClient._normalizeUnsignedInteger(
        rest.dataspaceId,
        "getUaidManifests.dataspaceId",
        { allowZero: true },
      );
    }
    const response = await this._request(
      "GET",
      `/v1/space-directory/uaids/${encodeURIComponent(canonicalUaid)}/manifests`,
      {
        headers: { Accept: "application/json" },
        params,
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("uaid manifests endpoint returned no payload");
    }
    return normalizeUaidManifestsResponse(payload);
  }

  /**
   * Publish (or rotate) a Space Directory manifest (`POST /v1/space-directory/manifests`).
   * @param {PublishSpaceDirectoryManifestRequest} request
   * @returns {Promise<unknown | null>}
   */
  async publishSpaceDirectoryManifest(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "publishSpaceDirectoryManifest",
    );
    const payload = normalizePublishSpaceDirectoryManifestRequest(request);
    const response = await this._request("POST", "/v1/space-directory/manifests", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [202]);
    return this._maybeJson(response);
  }

  /**
   * Revoke an active Space Directory manifest (`POST /v1/space-directory/manifests/revoke`).
   * @param {RevokeSpaceDirectoryManifestRequest} request
   * @returns {Promise<unknown | null>}
   */
  async revokeSpaceDirectoryManifest(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "revokeSpaceDirectoryManifest",
    );
    const payload = normalizeRevokeSpaceDirectoryManifestRequest(request);
    const response = await this._request(
      "POST",
      "/v1/space-directory/manifests/revoke",
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [202]);
    return this._maybeJson(response);
  }

  /**
   * Submit a Norito-encoded transaction payload.
   * Throws ToriiDataModelCompatibilityError when the node data model version mismatches.
   * @param {ArrayBufferView | ArrayBuffer | Buffer} payload
   * @returns {Promise<any>} Submission receipt (decoded from Norito) or JSON when present; otherwise null.
   */
  async submitTransaction(payload) {
    await this._ensureDataModelCompatibility();
    const response = await this._request("POST", "/v1/pipeline/transactions", {
      headers: {
        "Content-Type": "application/x-norito",
        Accept: "application/x-norito, application/json",
      },
      body: toBuffer(payload),
      retryProfile: "pipeline",
    });
    await this._expectStatus(response, [200, 201, 202, 204]);
    const route = this._extractSubmissionRoute(response);
    const contentType = this._getHeader(response, "content-type");
    const enrichSubmission = (value) => {
      if (!route) {
        return value;
      }
      if (value === null || value === undefined) {
        return { route };
      }
      if (typeof value === "object" && !Array.isArray(value)) {
        return { ...value, route };
      }
      return { value, route };
    };
    if (contentType && contentType.toLowerCase().includes("application/x-norito")) {
      const body = Buffer.from(await response.arrayBuffer());
      if (body.length === 0) {
        return enrichSubmission(null);
      }
      return enrichSubmission(decodeTransactionReceiptPayload(body));
    }
    return enrichSubmission(await this._maybeJson(response));
  }

  _extractSubmissionRoute(response) {
    const laneRaw = this._getHeader(response, "x-iroha-route-lane-id");
    const dataspaceRaw = this._getHeader(response, "x-iroha-route-dataspace-id");
    let laneId = null;
    if (laneRaw != null && String(laneRaw).trim() !== "") {
      try {
        laneId = ToriiClient._normalizeUnsignedInteger(
          laneRaw,
          "submitTransaction.route.laneId",
        );
      } catch {
        laneId = null;
      }
    }
    let dataspaceId = null;
    if (dataspaceRaw != null && String(dataspaceRaw).trim() !== "") {
      try {
        dataspaceId = ToriiClient._normalizeUnsignedInteger(
          dataspaceRaw,
          "submitTransaction.route.dataspaceId",
        );
      } catch {
        dataspaceId = null;
      }
    }
    if (laneId === null && dataspaceId === null) {
      return null;
    }
    return {
      acceptedBy: this._baseUrl,
      laneId,
      dataspaceId,
    };
  }

  async _ensureDataModelCompatibility() {
    const expected = EXPECTED_DATA_MODEL_VERSION;
    if (this._dataModelCompatibility.status === "compatible") {
      return;
    }
    if (this._dataModelCompatibility.status === "incompatible") {
      throw new ToriiDataModelCompatibilityError(expected, this._dataModelCompatibility.actual);
    }
    if (this._dataModelCompatibilityPromise) {
      return this._dataModelCompatibilityPromise;
    }
    const promise = (async () => {
      let capabilities;
      try {
        capabilities = await this.getNodeCapabilities();
      } catch (error) {
        if (error instanceof ValidationError) {
          this._dataModelCompatibility = { status: "incompatible", actual: null };
          throw new ToriiDataModelCompatibilityError(expected, null, error);
        }
        if (error instanceof ToriiHttpError && error.status === 404) {
          // Some deployments do not expose the capability advert yet. In that
          // case, proceed with the client-side expected model version.
          this._dataModelCompatibility = { status: "compatible", actual: expected };
          return;
        }
        throw error;
      }
      const actual = capabilities.dataModelVersion;
      if (actual !== expected) {
        this._dataModelCompatibility = { status: "incompatible", actual };
        throw new ToriiDataModelCompatibilityError(expected, actual);
      }
      this._dataModelCompatibility = { status: "compatible", actual };
    })();
    this._dataModelCompatibilityPromise = promise;
    promise
      .finally(() => {
        if (this._dataModelCompatibilityPromise === promise) {
          this._dataModelCompatibilityPromise = null;
        }
      })
      .catch(() => {
        // Avoid unhandled rejections from the cleanup chain.
      });
    return promise;
  }

  /**
   * Query pipeline status for a transaction hash (hex encoded).
   * @param {string} hashHex
   * @param {{
   *   allowShortHash?: boolean,
   *   signal?: AbortSignal,
   *   scope?: "local" | "auto" | "global",
   *   endpoints?: ReadonlyArray<string> | string,
   * }} [options]
   * @returns {Promise<any>} Parsed JSON if present; otherwise null.
  */
  async getTransactionStatus(hashHex, options = {}) {
    const optionRecord =
      options === undefined || options === null
        ? {}
        : requirePlainObjectOption(options, "getTransactionStatus options");
    assertSupportedOptionKeys(
      optionRecord,
      GET_TX_STATUS_OPTION_KEYS,
      "getTransactionStatus options",
    );
    if (
      optionRecord.allowShortHash !== undefined &&
      optionRecord.allowShortHash !== null &&
      typeof optionRecord.allowShortHash !== "boolean"
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "getTransactionStatus options.allowShortHash must be a boolean when provided",
        "getTransactionStatus.options.allowShortHash",
      );
    }
    const allowShortHash = optionRecord.allowShortHash === true;
    const scope = normalizeTransactionStatusScope(
      optionRecord.scope,
      "getTransactionStatus options.scope",
      this._config.transactionStatusScope || "auto",
    );
    const endpointCandidates = normalizeStatusEndpointCandidates(
      this._baseUrl,
      optionRecord.endpoints ?? this._config.statusEndpoints,
      "getTransactionStatus options.endpoints",
    );
    const { signal } = normalizeSignalOption(
      optionRecord,
      "getTransactionStatus",
    );
    const normalizedHash = normalizeHashLike32(
      hashHex,
      "getTransactionStatus.hashHex",
      { allowShort: allowShortHash },
    );
    const candidates =
      scope === "local" ? endpointCandidates.slice(0, 1) : endpointCandidates;
    let firstError = null;
    for (const endpointBaseUrl of candidates) {
      try {
        const payload = await this._getTransactionStatusFromEndpoint(
          endpointBaseUrl,
          normalizedHash,
          { signal, scope },
        );
        if (!payload) {
          continue;
        }
        if (
          endpointBaseUrl !== this._baseUrl &&
          payload &&
          typeof payload === "object" &&
          !Array.isArray(payload)
        ) {
          return {
            ...payload,
            resolved_from: endpointBaseUrl,
          };
        }
        return payload;
      } catch (error) {
        if (this._isAbortError(error)) {
          throw error;
        }
        if (!firstError) {
          firstError = error;
        }
      }
    }
    if (firstError) {
      throw firstError;
    }
    return null;
  }

  async _getTransactionStatusFromEndpoint(baseUrl, normalizedHash, options = {}) {
    const { signal, scope } = options;
    const response = await this._request(
      "GET",
      `${baseUrl}/v1/pipeline/transactions/status`,
      {
        params: { hash: normalizedHash, scope },
        allowAbsoluteUrl: true,
        retryProfile: "pipeline",
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200, 202, 204]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    if (
      typeof payload === "object" &&
      payload !== null &&
      Object.keys(payload).length === 0
    ) {
      return null;
    }
    return normalizePipelineTransactionStatus(payload);
  }

  /**
   * Fetch transaction pipeline status and normalise the payload.
   * @param {string} hashHex
   * @returns {Promise<ToriiPipelineStatus | null>}
   */
  async getTransactionStatusTyped(hashHex, options = {}) {
    const payload = await this.getTransactionStatus(hashHex, options);
    if (!payload) {
      return null;
    }
    return normalizePipelineStatusPayload(payload);
  }

  /**
   * Poll pipeline status until the transaction reaches a terminal state.
   * @param {string} hashHex
   * @param {{
   *   signal?: AbortSignal,
   *   intervalMs?: number,
   *   timeoutMs?: number | null,
   *   maxAttempts?: number | null,
   *   successStatuses?: Iterable<string>,
   *   failureStatuses?: Iterable<string>,
   *   onStatus?: (status: string | null, payload: any, attempt: number) => (void | Promise<void>)
   * }} [options]
  * @returns {Promise<any>}
  * @throws {TransactionStatusError} when the transaction reports a failure status.
  * @throws {TransactionTimeoutError} when timeout or max attempts elapse.
  */
  async waitForTransactionStatus(hashHex, options = {}) {
    const normalizedHash = normalizeHashLike32(
      hashHex,
      "waitForTransactionStatus.hashHex",
    );
    const {
      intervalMs,
      timeoutMs,
      maxAttempts,
      signal,
      successSet,
      failureSet,
      onStatus,
    } = ToriiClient._normalizeTransactionStatusPollOptions(
      options,
      "waitForTransactionStatus options",
    );

    const hasTimeout = timeoutMs !== null;
    const timeoutBudgetMs = hasTimeout ? timeoutMs : Number.POSITIVE_INFINITY;
    const deadline = hasTimeout ? Date.now() + timeoutBudgetMs : Number.POSITIVE_INFINITY;

    let attempts = 0;
    let lastPayload = null;
    // eslint-disable-next-line no-constant-condition
    while (true) {
      throwIfAborted(signal);
      attempts += 1;
      lastPayload = await this.getTransactionStatus(normalizedHash, { signal });
      const status = extractPipelineStatusKind(lastPayload);
      if (onStatus) {
        await onStatus(status, lastPayload, attempts);
      }
      throwIfAborted(signal);
      if (status !== null) {
        if (successSet.has(status)) {
          return lastPayload;
        }
        if (failureSet.has(status)) {
          throw new TransactionStatusError(normalizedHash, status, lastPayload);
        }
      }

      if (maxAttempts !== null && attempts >= maxAttempts) {
        throw new TransactionTimeoutError(
          `Transaction ${normalizedHash} did not reach a terminal status after ${attempts} attempts`,
          normalizedHash,
          attempts,
          lastPayload,
        );
      }

      if (hasTimeout && Date.now() >= deadline) {
        throw new TransactionTimeoutError(
          `Transaction ${normalizedHash} did not reach a terminal status within ${timeoutBudgetMs}ms`,
          normalizedHash,
          attempts,
          lastPayload,
        );
      }

      if (intervalMs > 0) {
        await delay(intervalMs, signal);
      }
    }
  }

  /**
   * Poll transaction pipeline status until it reaches a terminal state and normalise the payload.
   * @param {string} hashHex
   * @param {TransactionStatusPollOptions} [options]
   * @returns {Promise<ToriiPipelineStatus | null>}
   */
  async waitForTransactionStatusTyped(hashHex, options = {}) {
    const payload = await this.waitForTransactionStatus(hashHex, options);
    if (!payload) {
      return null;
    }
    return normalizePipelineStatusPayload(payload);
  }

  /**
   * Submit a transaction payload and await its terminal pipeline status.
   * @param {ArrayBufferView | ArrayBuffer | Buffer} payload
   * @param {{
   *   hashHex: string,
   *   intervalMs?: number,
   *   timeoutMs?: number | null,
   *   maxAttempts?: number | null,
   *   successStatuses?: Iterable<string>,
   *   failureStatuses?: Iterable<string>,
   *   onStatus?: (status: string | null, payload: any, attempt: number) => (void | Promise<void>)
   * }} options
   * @returns {Promise<any>}
   */
  async submitTransactionAndWait(payload, options) {
    const record = ToriiClient._requirePlainObject(
      options,
      "submitTransactionAndWait options",
    );
    const { hashHex, ...pollOptions } = record;
    const normalizedHash = requireHexString(hashHex, "options.hashHex");
    await this.submitTransaction(payload);
    return this.waitForTransactionStatus(normalizedHash, pollOptions);
  }

  /**
   * Submit a transaction payload and await its terminal pipeline status (normalised structure).
   * @param {ArrayBufferView | ArrayBuffer | Buffer} payload
   * @param {SubmitTransactionAndWaitOptions} options
   * @returns {Promise<ToriiPipelineStatus | null>}
   */
  async submitTransactionAndWaitTyped(payload, options) {
    const status = await this.submitTransactionAndWait(payload, options);
    if (!status) {
      return null;
    }
    return normalizePipelineStatusPayload(status);
  }

  /**
   * Fetch the pipeline recovery sidecar for a block height (`GET /v1/pipeline/recovery/{height}`).
   * Returns null when the node has no recovery artefacts for the requested height.
   * @param {number | string | bigint} height
   * @returns {Promise<any | null>}
   */
  async getPipelineRecovery(height) {
    const normalizedHeight = ToriiClient._normalizeUnsignedInteger(height, "height", {
      allowZero: true,
    });
    const response = await this._request(
      "GET",
      `/v1/pipeline/recovery/${normalizedHeight}`,
      { headers: { Accept: "application/json" } },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("pipeline recovery endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch a pipeline recovery sidecar and normalise it into typed fields.
   * @param {number | string | bigint} height
   * @returns {Promise<ToriiPipelineRecoverySidecar | null>}
   */
  async getPipelineRecoveryTyped(height) {
    const payload = await this.getPipelineRecovery(height);
    if (!payload) {
      return null;
    }
    return normalizePipelineRecoverySidecar(payload);
  }

  /**
   * Fetch Torii health snapshot (`GET /v1/health`).
   * @param {{ signal?: AbortSignal }} [options]
   * @returns {Promise<Record<string, unknown> | {status: string} | null>}
   */
  async getHealth(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getHealth");
    const response = await this._request("GET", "/v1/health", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (payload === null || payload === undefined) {
      return null;
    }
    return normalizeHealthSnapshot(payload, "health response");
  }

  /**
   * Fetch Torii configuration snapshot (`GET /v1/configuration`).
   * @returns {Promise<any | null>} Parsed JSON when available, otherwise null.
   */
  async getConfiguration() {
    const response = await this._request("GET", "/v1/configuration", {
      headers: { Accept: "application/json" },
    });
    if (response.status === 404 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    return this._maybeJson(response);
  }

  /**
   * Fetch Torii configuration snapshot (`GET /v1/configuration`) with typed fields.
   * @returns {Promise<ToriiConfigurationSnapshot | null>}
   */
  async getConfigurationTyped() {
    const payload = await this.getConfiguration();
    if (payload === null) {
      return null;
    }
    return normalizeConfigurationSnapshot(payload);
  }

  /**
   * Convenience helper returning the confidential gas schedule if present.
   * @returns {Promise<{
   *   proofBase: number;
   *   perPublicInput: number;
   *   perProofByte: number;
   *   perNullifier: number;
   *   perCommitment: number;
   * } | null>}
   */
  async getConfidentialGasSchedule() {
    const config = await this.getConfiguration();
    if (!config) {
      return null;
    }
    return extractConfidentialGasConfig({ config });
  }

  /**
   * Fetch Torii status snapshot (`GET /v1/status`) with typed governance breakdown.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiStatusSnapshot>}
   */
  async getStatusSnapshot(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getStatusSnapshot");
    const response = await this._request("GET", "/v1/status", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeStatusSnapshot(payload, this._statusState);
  }

  /**
   * Fetch the Network Time Service now snapshot (`GET /v1/time/now`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiNetworkTimeNow>}
   */
  async getNetworkTimeNow(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getNetworkTimeNow");
    const response = await this._request("GET", "/v1/time/now", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeTimeNowResponse(payload);
  }

  /**
   * Fetch Network Time Service diagnostics (`GET /v1/time/status`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiNetworkTimeStatus>}
   */
  async getNetworkTimeStatus(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getNetworkTimeStatus");
    const response = await this._request("GET", "/v1/time/status", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeTimeStatusResponse(payload);
  }

  /**
   * Fetch node capability advert (`GET /v1/node/capabilities`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiNodeCapabilities>}
   */
  async getNodeCapabilities(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getNodeCapabilities");
    const response = await this._request("GET", "/v1/node/capabilities", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeNodeCapabilitiesResponse(payload);
  }

  /**
   * Fetch SCCP capability discovery (`GET /v1/sccp/capabilities`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getSccpCapabilities(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSccpCapabilities");
    const response = await this._request("GET", "/v1/sccp/capabilities", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSccpCapabilitiesResponse(payload);
  }

  /**
   * Fetch typed SCCP proof manifests (`GET /v1/sccp/manifests`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getSccpProofManifests(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSccpProofManifests");
    const response = await this._request("GET", "/v1/sccp/manifests", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSccpProofManifestSetResponse(payload);
  }

  /**
   * Fetch a typed SCCP transparent message-proof artifact (`GET /v1/sccp/artifacts/message/{message_id}`).
   * @param {string|Buffer|Uint8Array|ArrayBuffer|ArrayBufferView} messageIdHex
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getSccpMessageProofArtifact(messageIdHex, options = {}) {
    const normalizedMessageId = normalizeHex32String(
      messageIdHex,
      "getSccpMessageProofArtifact.messageIdHex",
    );
    const { signal } = normalizeSignalOnlyOption(options, "getSccpMessageProofArtifact");
    const response = await this._request(
      "GET",
      `/v1/sccp/artifacts/message/${normalizedMessageId}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSccpMessageTransparentProofArtifact(payload);
  }

  /**
   * Fetch a normalized SCCP counterparty proof job (`GET /v1/sccp/jobs/message/{message_id}`).
   * @param {string|Buffer|Uint8Array|ArrayBuffer|ArrayBufferView} messageIdHex
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getSccpMessageProofJob(messageIdHex, options = {}) {
    const normalizedMessageId = normalizeHex32String(
      messageIdHex,
      "getSccpMessageProofJob.messageIdHex",
    );
    const { signal } = normalizeSignalOnlyOption(options, "getSccpMessageProofJob");
    const response = await this._request(
      "GET",
      `/v1/sccp/jobs/message/${normalizedMessageId}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSccpCounterpartyProofJob(payload);
  }

  /**
   * Fetch the active ABI version (`GET /v1/runtime/abi/active`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeAbiActiveResponse>}
   */
  async getRuntimeAbiActive(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getRuntimeAbiActive");
    const response = await this._request("GET", "/v1/runtime/abi/active", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeAbiActiveResponse(payload);
  }

  /**
   * Fetch the canonical ABI hash for the active policy (`GET /v1/runtime/abi/hash`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeAbiHashResponse>}
   */
  async getRuntimeAbiHash(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getRuntimeAbiHash");
    const response = await this._request("GET", "/v1/runtime/abi/hash", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeAbiHashResponse(payload);
  }

  /**
   * Fetch runtime metrics summary (`GET /v1/runtime/metrics`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeMetrics>}
   */
  async getRuntimeMetrics(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getRuntimeMetrics");
    const response = await this._request("GET", "/v1/runtime/metrics", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeMetricsResponse(payload);
  }

  /**
   * List recorded runtime upgrades (`GET /v1/runtime/upgrades`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ReadonlyArray<ToriiRuntimeUpgradeListItem>>}
   */
  async listRuntimeUpgrades(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listRuntimeUpgrades");
    const response = await this._request("GET", "/v1/runtime/upgrades", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeUpgradesListResponse(payload);
  }

  /**
   * Propose a runtime upgrade manifest (`POST /v1/runtime/upgrades/propose`).
   * @param {ToriiRuntimeUpgradeManifestInput} manifest
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeUpgradeTxResponse>}
   */
  async proposeRuntimeUpgrade(manifest, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "proposeRuntimeUpgrade",
    );
    const payload = normalizeRuntimeUpgradeManifestPayload(
      manifest,
      "proposeRuntimeUpgrade.manifest",
    );
    const response = await this._request("POST", "/v1/runtime/upgrades/propose", {
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const json = await this._maybeJson(response);
    return normalizeRuntimeUpgradeTxResponse(json, "runtime upgrade propose response");
  }

  /**
   * Build activation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/activate/{id}`).
   * @param {string | BinaryLike} idHex
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeUpgradeTxResponse>}
   */
  async activateRuntimeUpgrade(idHex, options = {}) {
    const hash = normalizeHex32String(idHex, "activateRuntimeUpgrade.idHex");
    const { signal } = normalizeSignalOnlyOption(options, "activateRuntimeUpgrade");
    const response = await this._request(
      "POST",
      `/v1/runtime/upgrades/activate/0x${hash}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeUpgradeTxResponse(payload, "runtime upgrade activate response");
  }

  /**
   * Build cancellation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/cancel/{id}`).
   * @param {string | BinaryLike} idHex
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiRuntimeUpgradeTxResponse>}
   */
  async cancelRuntimeUpgrade(idHex, options = {}) {
    const hash = normalizeHex32String(idHex, "cancelRuntimeUpgrade.idHex");
    const { signal } = normalizeSignalOnlyOption(options, "cancelRuntimeUpgrade");
    const response = await this._request(
      "POST",
      `/v1/runtime/upgrades/cancel/0x${hash}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeRuntimeUpgradeTxResponse(payload, "runtime upgrade cancel response");
  }

  /**
   * List currently online peers (`GET /v1/peers`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ReadonlyArray<Record<string, unknown>>>}
   */
  async listPeers(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listPeers");
    const response = await this._request("GET", "/v1/peers", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    return this._maybeJson(response);
  }

  /**
   * List online peers and normalise their metadata.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ReadonlyArray<{address: string, public_key_hex: string}>>}
   */
  async listPeersTyped(options = {}) {
    const payload = await this.listPeers(options);
    return normalizePeerListResponse(payload);
  }

  /**
   * List telemetry peer metadata (`GET /v1/telemetry/peers-info`).
  * @param {{signal?: AbortSignal}} [options]
  * @returns {Promise<ReadonlyArray<ToriiTelemetryPeerInfo>>}
  */
  async listTelemetryPeersInfo(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listTelemetryPeersInfo", {
      allowExtras: false,
    });
    const response = await this._request("GET", "/v1/telemetry/peers-info", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("telemetry peers endpoint returned no payload");
    }
    return normalizeTelemetryPeersInfoList(payload);
  }

  /**
   * Fetch Torii explorer network metrics (`GET /v1/explorer/metrics`).
   * Returns null when telemetry outputs are disabled or gated.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiExplorerMetricsSnapshot | null>}
   */
  async getExplorerMetrics(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getExplorerMetrics");
    const response = await this._request("GET", "/v1/explorer/metrics", {
      headers: { Accept: "application/json" },
      signal,
    });
    if (response.status === 403 || response.status === 404 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer metrics endpoint returned no payload");
    }
    return normalizeExplorerMetricsResponse(payload);
  }

  /**
   * Fetch the share-ready Explorer QR payload for an account (`GET /v1/explorer/accounts/{account_id}/qr`).
   * @param {string} accountId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiExplorerAccountQrSnapshot>}
   */
  async getExplorerAccountQr(accountId, options = {}) {
    const { signal } = normalizeExplorerRequestOptions(options);
    const normalizedId = normalizeAccountPathLiteral(accountId, "accountId");
    const response = await this._request(
      "GET",
      `/v1/explorer/accounts/${encodeURIComponent(normalizedId)}/qr`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer account QR endpoint returned no payload");
    }
    return normalizeExplorerAccountQrResponse(payload, "explorer account qr response");
  }

  /**
   * Fetch the public Sora VPN profile (`GET /v1/vpn/profile`).
   * Returns null when the control plane is unavailable.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiVpnProfile | null>}
   */
  async getVpnProfile(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getVpnProfile");
    const response = await this._request("GET", "/v1/vpn/profile", {
      headers: { Accept: "application/json" },
      signal,
    });
    if (response.status === 404 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("vpn profile endpoint returned no payload");
    }
    return normalizeVpnProfileResponse(payload);
  }

  /**
   * Create a signed Sora VPN session (`POST /v1/vpn/sessions`).
   * @param {{exitClass?: string}} request
   * @param {{signal?: AbortSignal, canonicalAuth: CanonicalRequestAuth}} options
   * @returns {Promise<ToriiVpnSession>}
   */
  async createVpnSession(request = {}, options) {
    const record = ensureRecord(request, "createVpnSession request");
    const { signal, canonicalAuth } = normalizeVpnSessionOptions(
      options,
      "createVpnSession",
    );
    const exitClass =
      record.exitClass === undefined || record.exitClass === null
        ? ""
        : requireNonEmptyString(record.exitClass, "createVpnSession request.exitClass");
    const response = await this._request("POST", "/v1/vpn/sessions", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({ exit_class: exitClass }),
      signal,
      canonicalAuth,
    });
    await this._expectStatus(response, [201]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("vpn session endpoint returned no payload");
    }
    return normalizeVpnSessionResponse(payload);
  }

  /**
   * Fetch an authenticated active Sora VPN session (`GET /v1/vpn/sessions/{session_id}`).
   * Returns null when the session is already absent.
   * @param {string} sessionId
   * @param {{signal?: AbortSignal, canonicalAuth: CanonicalRequestAuth}} options
   * @returns {Promise<ToriiVpnSession | null>}
   */
  async getVpnSession(sessionId, options) {
    const normalizedSessionId = requireNonEmptyString(sessionId, "sessionId");
    const { signal, canonicalAuth } = normalizeVpnSessionOptions(
      options,
      "getVpnSession",
    );
    const response = await this._request(
      "GET",
      `/v1/vpn/sessions/${encodeURIComponent(normalizedSessionId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
        canonicalAuth,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("vpn session status endpoint returned no payload");
    }
    return normalizeVpnSessionResponse(payload, "vpn session status response");
  }

  /**
   * Delete a signed Sora VPN session (`DELETE /v1/vpn/sessions/{session_id}`).
   * Returns null when the session is already absent.
   * @param {string} sessionId
   * @param {{signal?: AbortSignal, canonicalAuth: CanonicalRequestAuth}} options
   * @returns {Promise<ToriiVpnReceipt | null>}
   */
  async deleteVpnSession(sessionId, options) {
    const normalizedSessionId = requireNonEmptyString(sessionId, "sessionId");
    const { signal, canonicalAuth } = normalizeVpnSessionOptions(
      options,
      "deleteVpnSession",
    );
    const response = await this._request(
      "DELETE",
      `/v1/vpn/sessions/${encodeURIComponent(normalizedSessionId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
        canonicalAuth,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("vpn delete endpoint returned no payload");
    }
    return normalizeVpnReceiptResponse(payload, "vpn delete response");
  }

  /**
   * Fetch canonical Sora VPN receipt history for the authenticated account (`GET /v1/vpn/receipts`).
   * @param {{signal?: AbortSignal, canonicalAuth: CanonicalRequestAuth}} options
   * @returns {Promise<ReadonlyArray<ToriiVpnReceipt>>}
   */
  async listVpnReceipts(options) {
    const { signal, canonicalAuth } = normalizeVpnSessionOptions(
      options,
      "listVpnReceipts",
    );
    const response = await this._request("GET", "/v1/vpn/receipts", {
      headers: { Accept: "application/json" },
      signal,
      canonicalAuth,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("vpn receipts endpoint returned no payload");
    }
    return normalizeVpnReceiptListResponse(payload);
  }

  /**
   * Fetch the suffix policy for a Sora Name Service suffix (`GET /v1/sns/policies/{suffix_id}`).
   * @param {number} suffixId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsSuffixPolicy>}
   */
  async getSnsPolicy(suffixId, options = {}) {
    const resolvedId = ToriiClient._normalizeUnsignedInteger(suffixId, "suffixId", { allowZero: true });
    if (resolvedId > 0xffff) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        "suffixId must fit within a u16",
        "suffixId",
      );
    }
    const { signal } = normalizeSignalOnlyOption(options, "getSnsPolicy");
    const response = await this._request("GET", `/v1/sns/policies/${resolvedId}`, {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sns policy endpoint returned no payload");
    }
    return normalizeSnsSuffixPolicy(payload);
  }

  _normalizeLegacySnsDomainSelector(selector, context) {
    const normalizedSelector = requireNonEmptyString(selector, "selector").trim();
    const dotIndex = normalizedSelector.indexOf(".");
    if (dotIndex <= 0 || dotIndex === normalizedSelector.length - 1) {
      throw createValidationError(
        ValidationErrorCode.INVALID_FIELD,
        `${context} selector must be of the form label.suffix`,
        "selector",
      );
    }
    return normalizedSelector.slice(0, dotIndex);
  }

  /**
   * Fetch a Sora Name Service registration (`GET /v1/sns/names/domain/{literal}`).
   * @param {string} selector
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsNameRecord>}
   */
  async getSnsRegistration(selector, options = {}) {
    const literal = this._normalizeLegacySnsDomainSelector(selector, "getSnsRegistration");
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSnsRegistration",
    );
    const response = await this._request(
      "GET",
      `/v1/sns/names/domain/${encodeURIComponent(literal)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sns registration endpoint returned no payload");
    }
    return normalizeSnsNameRecord(payload, "sns registration response");
  }

  /**
   * Register a Sora Name Service name (`POST /v1/sns/names`).
   * @param {SnsRegisterNameRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsRegisterNameResponse>}
  */
  async registerSnsName(request, options = {}) {
    const payload = normalizeSnsRegisterRequest(request, "registerSnsName");
    const { signal } = normalizeSignalOnlyOption(options, "registerSnsName");
    const response = await this._request("POST", "/v1/sns/names", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 201]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("sns registrations endpoint returned no payload");
    }
    return normalizeSnsRegisterResponse(body);
  }

  /**
   * Renew a Sora Name Service registration (`POST /v1/sns/names/domain/{literal}/renew`).
   * @param {string} selector
   * @param {SnsRenewNameRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsNameRecord>}
   */
  async renewSnsRegistration(selector, request, options = {}) {
    const literal = this._normalizeLegacySnsDomainSelector(selector, "renewSnsRegistration");
    const payload = normalizeSnsRenewRequest(request, "renewSnsRegistration");
    const { signal } = normalizeSignalOnlyOption(options, "renewSnsRegistration");
    const response = await this._request(
      "POST",
      `/v1/sns/names/domain/${encodeURIComponent(literal)}/renew`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("sns renewal endpoint returned no payload");
    }
    return normalizeSnsNameRecord(body, "sns renewal response");
  }

  /**
   * Transfer ownership of a Sora Name Service registration (`POST /v1/sns/names/domain/{literal}/transfer`).
   * @param {string} selector
   * @param {SnsTransferNameRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsNameRecord>}
   */
  async transferSnsRegistration(selector, request, options = {}) {
    const literal = this._normalizeLegacySnsDomainSelector(selector, "transferSnsRegistration");
    const payload = normalizeSnsTransferRequest(request, "transferSnsRegistration");
    const { signal } = normalizeSignalOnlyOption(options, "transferSnsRegistration");
    const response = await this._request(
      "POST",
      `/v1/sns/names/domain/${encodeURIComponent(literal)}/transfer`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("sns transfer endpoint returned no payload");
    }
    return normalizeSnsNameRecord(body, "sns transfer response");
  }

  /**
   * Freeze a Sora Name Service registration (`POST /v1/sns/names/domain/{literal}/freeze`).
   * @param {string} selector
   * @param {SnsFreezeNameRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsNameRecord>}
  */
  async freezeSnsRegistration(selector, request, options = {}) {
    const literal = this._normalizeLegacySnsDomainSelector(selector, "freezeSnsRegistration");
    const payload = normalizeSnsFreezeRequest(request, "freezeSnsRegistration");
    const { signal } = normalizeSignalOnlyOption(options, "freezeSnsRegistration");
    const response = await this._request(
      "POST",
      `/v1/sns/names/domain/${encodeURIComponent(literal)}/freeze`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("sns freeze endpoint returned no payload");
    }
    return normalizeSnsNameRecord(body, "sns freeze response");
  }

  /**
   * Remove a freeze from a Sora Name Service registration (`DELETE /v1/sns/names/domain/{literal}/freeze`).
   * @param {string} selector
   * @param {SnsGovernanceHook} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SnsNameRecord>}
  */
  async unfreezeSnsRegistration(selector, request, options = {}) {
    const literal = this._normalizeLegacySnsDomainSelector(selector, "unfreezeSnsRegistration");
    const payload = normalizeSnsGovernanceHook(request, "unfreezeSnsRegistration");
    const { signal } = normalizeSignalOnlyOption(options, "unfreezeSnsRegistration");
    const response = await this._request(
      "DELETE",
      `/v1/sns/names/domain/${encodeURIComponent(literal)}/freeze`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("sns unfreeze endpoint returned no payload");
    }
    return normalizeSnsNameRecord(body, "sns unfreeze response");
  }

  /**
   * SNS governance case routes were removed from Torii.
   * @param {Record<string, unknown>} payload
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<never>}
   */
  async createSnsGovernanceCase(payload, options = {}) {
    normalizeSnsGovernanceCaseCreatePayload(payload);
    normalizeSignalOnlyOption(options, "createSnsGovernanceCase");
    throw new Error("Torii removed /v1/sns/governance/cases; use /v1/sns/names operations instead");
  }

  /**
   * SNS governance case routes were removed from Torii.
   * @param {SnsCaseExportOptions} [options]
   * @returns {Promise<never>}
   */
  async exportSnsGovernanceCases(options = {}) {
    normalizeSnsCaseExportOptions(options);
    throw new Error("Torii removed /v1/sns/governance/cases; use /v1/sns/names operations instead");
  }

  /**
   * Iterate SNS arbitration cases using the export feed.
   * @param {SnsCaseExportOptions} [options]
   * @returns {AsyncGenerator<SnsGovernanceCase, void, unknown>}
   */
  iterateSnsGovernanceCases(options = {}) {
    const { signal, params } = normalizeSnsCaseExportOptions(options);
    const baseOptions = {};
    if (Object.prototype.hasOwnProperty.call(params, "status")) {
      baseOptions.status = params.status;
    }
    if (Object.prototype.hasOwnProperty.call(params, "limit")) {
      baseOptions.limit = params.limit;
    }
    const initialSince = Object.prototype.hasOwnProperty.call(params, "since")
      ? params.since
      : undefined;
    const self = this;
    return (async function* iterator() {
      let sinceCursor = initialSince;
      while (true) {
        const exportOptions = { ...baseOptions, signal };
        if (sinceCursor !== undefined && sinceCursor !== null) {
          exportOptions.since = sinceCursor;
        }
        const page = await self.exportSnsGovernanceCases(exportOptions);
        for (const entry of page.cases) {
          yield entry;
        }
        if (!page.nextSince) {
          return;
        }
        sinceCursor = page.nextSince;
      }
    })();
  }

  /**
   * Fetch a governance proposal (`GET /v1/gov/proposals/{id}`).
   * @param {string} proposalId
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getGovernanceProposal(proposalId, options) {
    const normalized = requireNonEmptyString(proposalId, "proposalId");
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceProposal");
    const response = await this._request(
      "GET",
      `/v1/gov/proposals/${encodeURIComponent(normalized)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200, 404]);
    if (response.status === 404) {
      return null;
    }
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("governance proposal endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch a governance proposal and normalise the payload.
   * @param {string} proposalId
   * @returns {Promise<ToriiGovernanceProposalResult>}
   */
  async getGovernanceProposalTyped(proposalId, options) {
    const payload = await this.getGovernanceProposal(proposalId, options);
    if (!payload) {
      return { found: false, proposal: null };
    }
    return parseGovernanceProposalResult(payload);
  }

  /**
   * Fetch a governance referendum (`GET /v1/gov/referenda/{id}`).
   * @param {string} referendumId
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getGovernanceReferendum(referendumId, options) {
    const normalized = requireNonEmptyString(referendumId, "referendumId");
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceReferendum");
    const response = await this._request(
      "GET",
      `/v1/gov/referenda/${encodeURIComponent(normalized)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("governance referendum endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch a referendum and normalise the payload.
   * @param {string} referendumId
   * @returns {Promise<ToriiGovernanceReferendumResult>}
   */
  async getGovernanceReferendumTyped(referendumId, options) {
    const payload = await this.getGovernanceReferendum(referendumId, options);
    if (!payload) {
      return { found: false, referendum: null };
    }
    return parseGovernanceReferendumResult(payload);
  }

  /**
   * Fetch referendum tally (`GET /v1/gov/tally/{id}`).
   * @param {string} referendumId
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getGovernanceTally(referendumId, options) {
    const normalized = requireNonEmptyString(referendumId, "referendumId");
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceTally");
    const response = await this._request(
      "GET",
      `/v1/gov/tally/${encodeURIComponent(normalized)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200, 404]);
    if (response.status === 404) {
      return null;
    }
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("governance tally endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch referendum tally (`GET /v1/gov/tally/{id}`) with typed output.
   * @param {string} referendumId
   * @returns {Promise<ToriiGovernanceTallyResult>}
   */
  async getGovernanceTallyTyped(referendumId, options) {
    const normalizedId = requireNonEmptyString(referendumId, "referendumId");
    const payload = await this.getGovernanceTally(normalizedId, options);
    if (!payload) {
      return createEmptyGovernanceTallyResult(normalizedId);
    }
    const tally = parseGovernanceTally(payload);
    return { found: true, referendum_id: tally.referendum_id, tally };
  }

  /**
   * Fetch referendum locks (`GET /v1/gov/locks/{id}`).
   * @param {string} referendumId
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getGovernanceLocks(referendumId, options) {
    const normalized = requireNonEmptyString(referendumId, "referendumId");
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceLocks");
    const response = await this._request(
      "GET",
      `/v1/gov/locks/${encodeURIComponent(normalized)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200, 404]);
    if (response.status === 404) {
      return null;
    }
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("governance locks endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch referendum locks with typed output.
   * @param {string} referendumId
   * @returns {Promise<ToriiGovernanceLocksResult>}
   */
  async getGovernanceLocksTyped(referendumId, options) {
    const normalizedId = requireNonEmptyString(referendumId, "referendumId");
    const payload = await this.getGovernanceLocks(normalizedId, options);
    if (!payload) {
      return createEmptyGovernanceLocksResult(normalizedId);
    }
    return parseGovernanceLocksResult(payload);
  }

  /**
   * Fetch unlock sweep statistics (`GET /v1/gov/unlocks/stats`).
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getGovernanceUnlockStats(options) {
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceUnlockStats");
    const response = await this._request("GET", "/v1/gov/unlocks/stats", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("governance unlock stats endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch unlock sweep statistics with typed output.
   * @returns {Promise<ToriiGovernanceUnlockStats>}
   */
  async getGovernanceUnlockStatsTyped(options) {
    const payload = await this.getGovernanceUnlockStats(options);
    if (!payload) {
      throw new Error("governance unlock stats endpoint returned no payload");
    }
    return parseGovernanceUnlockStats(payload);
  }

  /**
   * Fetch the current council roster (`GET /v1/gov/council/current`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiGovernanceCouncilCurrentResponse>}
   */
  async getGovernanceCouncilCurrent(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getGovernanceCouncilCurrent",
    );
    const response = await this._request("GET", "/v1/gov/council/current", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeGovernanceCouncilCurrentResponse(payload);
  }

  /**
   * Derive a council roster from VRF candidates (`POST /v1/gov/council/derive-vrf`).
   * @param {ToriiGovernanceCouncilDeriveRequest} payload
   * @returns {Promise<ToriiGovernanceCouncilDeriveResponse>}
   */
  async governanceDeriveCouncilVrf(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceCouncilDeriveRequest(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceDeriveCouncilVrf",
    );
    const response = await this._request("POST", "/v1/gov/council/derive-vrf", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    return normalizeGovernanceCouncilDeriveResponse(draft);
  }

  /**
   * Persist a council roster for an epoch (`POST /v1/gov/council/persist`).
   * @param {ToriiGovernanceCouncilPersistRequest} payload
   * @returns {Promise<ToriiGovernanceCouncilPersistResponse>}
   */
  async governancePersistCouncil(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceCouncilPersistRequest(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governancePersistCouncil",
    );
    const response = await this._request("POST", "/v1/gov/council/persist", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    return normalizeGovernanceCouncilPersistResponse(draft);
  }

  /**
   * Replace a council member using the next available alternate (`POST /v1/gov/council/replace`).
   * @param {ToriiGovernanceCouncilReplaceRequest} payload
   * @returns {Promise<ToriiGovernanceCouncilReplaceResponse>}
   */
  async governanceReplaceCouncil(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceCouncilReplaceRequest(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceReplaceCouncil",
    );
    const response = await this._request("POST", "/v1/gov/council/replace", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    return normalizeGovernanceCouncilReplaceResponse(draft);
  }

  /**
   * Inspect council derivation metadata (`GET /v1/gov/council/audit`).
   * @param {ToriiGovernanceCouncilAuditOptions} [options]
   * @returns {Promise<ToriiGovernanceCouncilAuditResponse>}
   */
  async getGovernanceCouncilAudit(options = {}) {
    const { signal, params } = buildGovernanceCouncilAuditQuery(options);
    const response = await this._request("GET", "/v1/gov/council/audit", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeGovernanceCouncilAuditResponse(payload);
  }

  /**
   * Finalise a referendum (`POST /v1/gov/finalize`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async governanceFinalizeReferendum(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceFinalizePayload(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceFinalizeReferendum",
    );
    const response = await this._request("POST", "/v1/gov/finalize", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200, 202, 204]);
    const draft = await this._maybeJson(response);
    return draft == null ? null : normalizeGovernanceDraftResponse(draft);
  }

  /**
   * Finalise a referendum and always return a typed draft.
   * @param {ToriiGovernanceFinalizeRequest} payload
   * @returns {Promise<ToriiGovernanceDraftResponse>}
   */
  async governanceFinalizeReferendumTyped(payload, options = {}) {
    const result = await this.governanceFinalizeReferendum(payload, options);
    return result ?? createEmptyGovernanceDraftResponse("governance finalize response");
  }

  /**
   * Enact a governance proposal (`POST /v1/gov/enact`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async governanceEnactProposal(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceEnactPayload(payload));
    const { signal } = normalizeSignalOnlyOption(options, "governanceEnactProposal");
    const response = await this._request("POST", "/v1/gov/enact", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200, 202, 204]);
    const draft = await this._maybeJson(response);
    return draft == null ? null : normalizeGovernanceDraftResponse(draft);
  }

  /**
   * Enact a governance proposal and always return a typed draft.
   * @param {ToriiGovernanceEnactRequest} payload
   * @returns {Promise<ToriiGovernanceDraftResponse>}
   */
  async governanceEnactProposalTyped(payload, options = {}) {
    const result = await this.governanceEnactProposal(payload, options);
    return result ?? createEmptyGovernanceDraftResponse("governance enact response");
  }

  /**
   * Draft a governance deployment proposal (`POST /v1/gov/proposals/deploy-contract`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<ToriiGovernanceDraftResponse>}
   */
  async governanceProposeDeployContract(payload, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceProposeDeployContract",
    );
    const body = JSON.stringify(normalizeGovernanceDeployContractProposalPayload(payload));
    const response = await this._request("POST", "/v1/gov/proposals/deploy-contract", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    if (!draft) {
      throw new Error("governance deploy-contract endpoint returned no payload");
    }
    return normalizeGovernanceDraftResponse(draft, "governance deploy-contract response");
  }

  /**
   * Submit a plain governance ballot (`POST /v1/gov/ballots/plain`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<ToriiGovernanceBallotResponse>}
   */
  async governanceSubmitPlainBallot(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernancePlainBallotPayload(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceSubmitPlainBallot",
    );
    const response = await this._request("POST", "/v1/gov/ballots/plain", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    if (!draft) {
      throw new Error("governance plain ballot endpoint returned no payload");
    }
    return normalizeGovernanceBallotResponse(draft, "governance plain ballot response");
  }

  /**
   * Submit a ZK governance ballot (`POST /v1/gov/ballots/zk`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<ToriiGovernanceBallotResponse>}
   */
  async governanceSubmitZkBallot(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceZkBallotPayload(payload));
    const { signal } = normalizeSignalOnlyOption(options, "governanceSubmitZkBallot");
    const response = await this._request("POST", "/v1/gov/ballots/zk", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    if (!draft) {
      throw new Error("governance zk ballot endpoint returned no payload");
    }
    return normalizeGovernanceBallotResponse(draft, "governance zk ballot response");
  }

  /**
   * Submit a ZK ballot using the v1 envelope DTO (`POST /v1/gov/ballots/zk-v1`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<ToriiGovernanceBallotResponse>}
   */
  async governanceSubmitZkBallotV1(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceZkBallotV1Payload(payload));
    const { signal } = normalizeSignalOnlyOption(options, "governanceSubmitZkBallotV1");
    const response = await this._request("POST", "/v1/gov/ballots/zk-v1", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    if (!draft) {
      throw new Error("governance zk ballot v1 endpoint returned no payload");
    }
    return normalizeGovernanceBallotResponse(draft, "governance zk ballot v1 response");
  }

  /**
   * Submit a BallotProof payload (`POST /v1/gov/ballots/zk-v1/ballot-proof`).
   * @param {Record<string, unknown>} payload
   * @returns {Promise<ToriiGovernanceBallotResponse>}
   */
  async governanceSubmitZkBallotProofV1(payload, options = {}) {
    const body = JSON.stringify(normalizeGovernanceZkBallotProofPayload(payload));
    const { signal } = normalizeSignalOnlyOption(
      options,
      "governanceSubmitZkBallotProofV1",
    );
    const response = await this._request("POST", "/v1/gov/ballots/zk-v1/ballot-proof", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body,
      signal,
    });
    await this._expectStatus(response, [200]);
    const draft = await this._maybeJson(response);
    if (!draft) {
      throw new Error("governance zk ballot proof endpoint returned no payload");
    }
    return normalizeGovernanceBallotResponse(
      draft,
      "governance zk ballot proof response",
    );
  }

  /**
   * Apply the `gov_protected_namespaces` parameter (`POST /v1/gov/protected-namespaces`).
   * @param {string | string[]} namespaces
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ProtectedNamespacesApplyResponse>}
   */
  async setProtectedNamespaces(namespaces, options = {}) {
    const values = normalizeProtectedNamespaceList(namespaces);
    const { signal } = normalizeSignalOnlyOption(options, "setProtectedNamespaces");
    const response = await this._request("POST", "/v1/gov/protected-namespaces", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({ namespaces: values }),
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("protected namespaces apply endpoint returned no payload");
    }
    return normalizeProtectedNamespacesApplyResponse(payload);
  }

  /**
   * Fetch the current `gov_protected_namespaces` setting (`GET /v1/gov/protected-namespaces`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ProtectedNamespacesGetResponse>}
   */
  async getProtectedNamespaces(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getProtectedNamespaces");
    const response = await this._request("GET", "/v1/gov/protected-namespaces", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("protected namespaces endpoint returned no payload");
    }
    return normalizeProtectedNamespacesGetResponse(payload);
  }

  /**
   * Fetch Sumeragi consensus status (`GET /v1/sumeragi/status`).
   * Includes view-change proof counters (`view_change_proof_{accepted,stale,rejected}_total`) alongside QC, epoch, membership hash (`membership.{height,view,epoch,view_hash}`), RBC, and queue fields.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<any>}
   */

  /**
   * Fetch recent commit certificates (newest first) via `/v1/sumeragi/commit-certificates`.
   */
  async listSumeragiCommitCertificates() {
    const response = await this._request("GET", "/v1/sumeragi/commit-certificates", {
      headers: { Accept: this._acceptHeader() },
    });
    if (!response) {
      throw new Error("sumeragi commit certificates endpoint returned no payload");
    }
    return response;
  }

  /**
   * Fetch consensus key lifecycle records (newest first) via `/v1/sumeragi/key-lifecycle`.
   */
  async listSumeragiKeyLifecycle() {
    const response = await this._request("GET", "/v1/sumeragi/key-lifecycle", {
      headers: { Accept: this._acceptHeader() },
    });
    if (!response) {
      throw new Error("sumeragi key lifecycle endpoint returned no payload");
    }
    return response;
  }

  async getSumeragiStatus(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSumeragiStatus");
    const response = await this._request("GET", "/v1/sumeragi/status", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    return this._maybeJson(response);
  }

  /**
   * Fetch Sumeragi consensus status and normalize Nexus fields.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiStatus>}
   */
  async getSumeragiStatusTyped(options = {}) {
    const payload = await this.getSumeragiStatus(options);
    return parseSumeragiStatusPayload(payload);
  }

  /**
   * Fetch Sumeragi pacemaker timers (`GET /v1/sumeragi/pacemaker`).
   * Returns null when telemetry outputs are disabled.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiPacemakerResponse | null>}
   */
  async getSumeragiPacemaker(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiPacemaker",
    );
    const response = await this._request("GET", "/v1/sumeragi/pacemaker", {
      headers: { Accept: "application/json" },
      signal,
    });
    if (response.status === 403 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi pacemaker endpoint returned no payload");
    }
    return normalizeSumeragiPacemakerSnapshot(payload);
  }

  /**
   * Fetch the latest Highest/Locked QC snapshot (`GET /v1/sumeragi/qc`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiQcSnapshot>}
   */
  async getSumeragiQc(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSumeragiQc");
    const response = await this._request("GET", "/v1/sumeragi/qc", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi qc endpoint returned no payload");
    }
    return normalizeSumeragiQcSnapshot(payload);
  }

  /**
   * Fetch a commit QC record for a block hash (`GET /v1/sumeragi/commit_qc/{hash}`).
   * @param {string} blockHashHex 32-byte block hash (hex; `0x`/`blake2b32:` prefixes accepted).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiCommitQcRecord>}
   */
  async getSumeragiCommitQc(blockHashHex, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiCommitQc",
    );
    const normalizedHash = normalizeHex32String(
      blockHashHex,
      "getSumeragiCommitQc.blockHashHex",
      { allowScheme: true },
    );
    const response = await this._request(
      "GET",
      `/v1/sumeragi/commit_qc/${normalizedHash}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi commit_qc endpoint returned no payload");
    }
    return normalizeSumeragiCommitQcRecord(payload, "sumeragi commit_qc response");
  }

  /**
   * Fetch per-phase latency telemetry (`GET /v1/sumeragi/phases`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiPhasesSnapshot>}
   */
  async getSumeragiPhases(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSumeragiPhases");
    const response = await this._request("GET", "/v1/sumeragi/phases", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi phases endpoint returned no payload");
    }
    return normalizeSumeragiPhasesSnapshot(payload);
  }

  /**
   * Fetch network→BLS key mapping (`GET /v1/sumeragi/bls_keys`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, string | null>>}
   */
  async getSumeragiBlsKeys(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiBlsKeys",
    );
    const response = await this._request("GET", "/v1/sumeragi/bls_keys", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi BLS keys endpoint returned no payload");
    }
    return normalizeSumeragiBlsKeysMap(payload);
  }

  /**
   * Fetch leader/PRF context snapshot (`GET /v1/sumeragi/leader`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiLeaderSnapshot>}
   */
  async getSumeragiLeader(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSumeragiLeader");
    const response = await this._request("GET", "/v1/sumeragi/leader", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi leader endpoint returned no payload");
    }
    return normalizeSumeragiLeaderSnapshot(payload);
  }

  /**
   * Fetch the collector plan snapshot (`GET /v1/sumeragi/collectors`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiCollectorsPlan>}
   */
  async getSumeragiCollectors(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiCollectors",
    );
    const response = await this._request("GET", "/v1/sumeragi/collectors", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi collectors endpoint returned no payload");
    }
    return normalizeSumeragiCollectorsPlan(payload);
  }

  /**
   * Fetch the on-chain Sumeragi parameter snapshot (`GET /v1/sumeragi/params`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiSumeragiParamsSnapshot>}
   */
  async getSumeragiParams(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiParams",
    );
    const response = await this._request("GET", "/v1/sumeragi/params", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi params endpoint returned no payload");
    }
   return normalizeSumeragiParamsSnapshot(payload);
  }

  /**
   * Fetch aggregated Sumeragi telemetry (`GET /v1/sumeragi/telemetry`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown>>}
   */
  async getSumeragiTelemetry(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiTelemetry",
    );
    const response = await this._request("GET", "/v1/sumeragi/telemetry", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi telemetry endpoint returned no payload");
    }
    return payload;
  }

  /**
   * Fetch aggregated Sumeragi telemetry with typed output.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SumeragiTelemetrySnapshot>}
   */
  async getSumeragiTelemetryTyped(options = {}) {
    const payload = await this.getSumeragiTelemetry(options);
    return normalizeSumeragiTelemetrySnapshot(payload, "sumeragi telemetry");
  }

  /**
   * Fetch RBC throughput metrics (`GET /v1/sumeragi/rbc`).
   * Returns null when telemetry is disabled or forbidden.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<any | null>}
   */
  async getSumeragiRbc(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getSumeragiRbc");
    const response = await this._request("GET", "/v1/sumeragi/rbc", {
      headers: { Accept: "application/json" },
      signal,
    });
    if (response.status === 403 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi rbc response missing JSON body");
    }
    return normalizeSumeragiRbcSnapshot(
      payload,
      "sumeragi rbc response",
    );
  }

  /**
   * Fetch active RBC sessions (`GET /v1/sumeragi/rbc/sessions`).
   * Returns null when telemetry outputs are disabled.
   * @returns {Promise<any | null>}
   */
  async getSumeragiRbcSessions(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiRbcSessions",
    );
    const response = await this._request("GET", "/v1/sumeragi/rbc/sessions", {
      headers: { Accept: "application/json" },
      signal,
    });
    if (response.status === 403 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi rbc sessions response missing JSON body");
    }
    return normalizeSumeragiRbcSessionsSnapshot(
      payload,
      "sumeragi rbc sessions response",
    );
  }

  /**
   * Fetch delivery status for a specific RBC session.
   * Returns null when the session has not been observed.
   * @param {number | string | bigint} height
   * @param {number | string | bigint} view
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<any | null>}
   */
  async getSumeragiRbcDelivered(height, view, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getSumeragiRbcDelivered",
    );
    const normalizedHeight = ToriiClient._normalizeUnsignedInteger(height, "height", {
      allowZero: true,
    });
    const normalizedView = ToriiClient._normalizeUnsignedInteger(view, "view", {
      allowZero: true,
    });
    const response = await this._request(
      "GET",
      `/v1/sumeragi/rbc/delivered/${normalizedHeight}/${normalizedView}`,
      { headers: { Accept: "application/json" }, signal },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("sumeragi rbc delivered response missing JSON body");
    }
    return normalizeSumeragiRbcDeliveryStatus(
      payload,
      "sumeragi rbc delivered response",
    );
  }

  /**
   * Attempt to auto-detect a delivered RBC session for sampling.
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SumeragiRbcSession | null>}
   */
  async findRbcSamplingCandidate(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "findRbcSamplingCandidate",
    );
    const snapshot = await this.getSumeragiRbcSessions({ signal });
    if (!snapshot || !Array.isArray(snapshot.items)) {
      return null;
    }
    const candidate = snapshot.items.find(
      (session) =>
        session &&
        session.delivered &&
        typeof session.blockHash === "string" &&
        session.blockHash.length > 0,
    );
    return candidate ?? null;
  }

  /**
   * Request RBC chunk samples (`POST /v1/sumeragi/rbc/sample`).
   * Returns null when the session payload is unavailable.
   * @param {{
   *   blockHash: string;
   *   height: number | string | bigint;
   *   view: number | string | bigint;
   *   count?: number | string | bigint;
   *   seed?: number | string | bigint;
   *   apiToken?: string;
   *   signal?: AbortSignal;
   * }} options
   * @returns {Promise<any | null>}
   */
  async sampleRbcChunks(options = {}) {
    const record = requirePlainObjectOption(options, "sampleRbcChunks options");
    assertSupportedOptionKeys(
      record,
      new Set(["blockHash", "height", "view", "count", "seed", "apiToken", "signal"]),
      "sampleRbcChunks options",
    );
    const { signal } = normalizeSignalOption(record, "sampleRbcChunks");
    const payload = {
      block_hash: requireHexString(record.blockHash, "blockHash"),
      height: ToriiClient._normalizeUnsignedInteger(record.height, "height", {
        allowZero: true,
      }),
      view: ToriiClient._normalizeUnsignedInteger(record.view, "view", {
        allowZero: true,
      }),
    };
    if (record.count !== undefined && record.count !== null) {
      payload.count = ToriiClient._normalizeUnsignedInteger(record.count, "count", {
        allowZero: false,
      });
    }
    if (record.seed !== undefined && record.seed !== null) {
      payload.seed = ToriiClient._normalizeUnsignedInteger(record.seed, "seed", {
        allowZero: true,
      });
    }
    const headers = {
      "Content-Type": "application/json",
      Accept: "application/json",
    };
    if (record.apiToken) {
      headers["X-API-Token"] = String(record.apiToken);
    }
    const response = await this._request("POST", "/v1/sumeragi/rbc/sample", {
      headers,
      body: JSON.stringify(payload),
      signal,
    });
    if (response.status === 404) {
      return null;
    }
    if (response.status === 401) {
      throw new Error("RBC sampling requires a valid X-API-Token");
    }
    await this._expectStatus(response, [200]);
    const sample = await this._maybeJson(response);
    if (!sample) {
      throw new Error("sumeragi rbc sample response missing JSON body");
    }
    return normalizeRbcSample(sample, "sumeragi rbc sample response");
  }

  /**
   * List recorded consensus evidence (`GET /v1/sumeragi/evidence`).
   * @param {SumeragiEvidenceListOptions} [options]
   * @returns {Promise<SumeragiEvidenceListResponse>}
    */
  async listSumeragiEvidence(options = {}) {
    const resolvedOptions =
      options === undefined || options === null
        ? {}
        : requirePlainObjectOption(options, "listSumeragiEvidence options");
    assertSupportedOptionKeys(
      resolvedOptions,
      new Set(["limit", "offset", "kind", "signal"]),
      "listSumeragiEvidence options",
    );
    const { signal } = normalizeSignalOption(resolvedOptions, "listSumeragiEvidence");
    const params = {};
    if (resolvedOptions.limit !== undefined && resolvedOptions.limit !== null) {
      const limit = ToriiClient._normalizeUnsignedInteger(resolvedOptions.limit, "limit", {
        allowZero: false,
      });
      if (limit > 1000) {
        throw new RangeError("limit must be <= 1000");
      }
      params.limit = limit;
    }
    if (resolvedOptions.offset !== undefined && resolvedOptions.offset !== null) {
      const offset = ToriiClient._normalizeUnsignedInteger(resolvedOptions.offset, "offset", {
        allowZero: true,
      });
      params.offset = offset;
    }
    if (resolvedOptions.kind !== undefined && resolvedOptions.kind !== null) {
      const kind = String(resolvedOptions.kind);
      if (!EVIDENCE_KIND_VALUES.has(kind)) {
        throw new RangeError(
          `kind must be one of ${Array.from(EVIDENCE_KIND_VALUES).join(", ")}`,
        );
      }
      params.kind = kind;
    }
    const response = await this._request("GET", "/v1/sumeragi/evidence", {
      headers: { Accept: "application/json" },
      params: Object.keys(params).length > 0 ? params : undefined,
      signal,
    });
    await this._expectStatus(response, [200]);
    return normalizeSumeragiEvidenceListResponse(await this._maybeJson(response));
  }

  /**
   * Retrieve the total number of evidence entries (`GET /v1/sumeragi/evidence/count`).
   * @returns {Promise<SumeragiEvidenceCountResponse>}
   */
  async getSumeragiEvidenceCount() {
    const response = await this._request("GET", "/v1/sumeragi/evidence/count", {
      headers: { Accept: "application/json" },
    });
    await this._expectStatus(response, [200]);
    const payload = ensureRecord(
      await this._maybeJson(response),
      "sumeragi evidence count response",
    );
    const count = Number(payload.count ?? 0);
    if (!Number.isFinite(count) || count < 0) {
      throw new TypeError("sumeragi evidence count response.count must be a non-negative number");
    }
    return { count };
  }

  /**
   * Submit consensus evidence (`POST /v1/sumeragi/evidence/submit`).
   * @param {SumeragiEvidenceSubmitRequest} request
   * @returns {Promise<SumeragiEvidenceSubmitResponse>}
   */
  async submitSumeragiEvidence(request) {
    const record = ensureRecord(request, "request");
    const evidenceHex = record.evidence_hex;
    if (typeof evidenceHex !== "string" || evidenceHex.trim().length === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        "request.evidence_hex must be a non-empty hex string",
        "submitSumeragiEvidence.request.evidence_hex",
      );
    }
    const headers = {
      "Content-Type": "application/json",
      Accept: "application/json",
    };
    if (record.apiToken) {
      headers["X-API-Token"] = String(record.apiToken);
    }
    const response = await this._request("POST", "/v1/sumeragi/evidence/submit", {
      headers,
      body: JSON.stringify({ evidence_hex: evidenceHex }),
    });
    await this._expectStatus(response, [202]);
    const payload = ensureRecord(
      await this._maybeJson(response),
      "sumeragi evidence submit response",
    );
    return {
      status: String(payload.status ?? ""),
      kind: String(payload.kind ?? ""),
    };
  }

  /**
   * Fetch metrics (`GET /v1/metrics`). When `asText` is true the raw text payload is returned.
   * @param {{asText?: boolean, signal?: AbortSignal}} [options]
   * @returns {Promise<unknown>}
   */
  async getMetrics(options = {}) {
    const normalizedOptions =
      options === undefined ? {} : ensureRecord(options, "getMetrics options");
    assertSupportedOptionKeys(
      normalizedOptions,
      GET_METRICS_OPTION_KEYS,
      "getMetrics options",
    );
    const { signal } = normalizeSignalOption(normalizedOptions, "getMetrics");
    let asText = false;
    if ("asText" in normalizedOptions) {
      if (typeof normalizedOptions.asText !== "boolean") {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          "getMetrics options.asText must be boolean",
          "getMetrics.options.asText",
        );
      }
      asText = normalizedOptions.asText;
    }
    const response = await this._request("GET", "/v1/metrics", {
      headers: { Accept: asText ? "text/plain" : "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    if (asText) {
      if (typeof response.text === "function") {
        return response.text();
      }
      const buffer = Buffer.from(await response.arrayBuffer());
      return buffer.toString("utf8");
    }
    return this._maybeJson(response);
  }

  /**
   * Fetch a block by height (`GET /v1/blocks/{height}`).
   * @param {number | string | bigint} height
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<any>}
   */
  async getBlock(height, options = {}) {
    const normalized = ToriiClient._normalizeUnsignedInteger(
      height,
      "getBlock.height",
      { allowZero: true },
    );
    const { signal } = normalizeSignalOnlyOption(options, "getBlock");
    const response = await this._request("GET", `/v1/blocks/${normalized}`, {
      signal,
    });
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer block endpoint returned no payload");
    }
    return normalizeExplorerBlockRecord(payload, "explorer block response");
  }

  /**
   * List blocks with optional pagination (`GET /v1/blocks`).
   * @param {BlockListOptions} [options]
   * @returns {Promise<any>}
   */
  async listBlocks(options) {
    const normalizedOptions = ToriiClient._normalizeBlockListOptions(options);
    const params = {};
    if (normalizedOptions.offsetHeight !== undefined) {
      params.offset_height = normalizedOptions.offsetHeight;
    }
    if (normalizedOptions.limit !== undefined) {
      params.limit = normalizedOptions.limit;
    }
    const response = await this._request("GET", "/v1/blocks", {
      params: Object.keys(params).length > 0 ? params : undefined,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("explorer blocks endpoint returned no payload");
    }
    return normalizeExplorerBlocksPage(payload);
  }

  /**
   * Stream JSON events from `/v1/events/sse`.
   * @template [T=unknown]
   * @param {EventStreamOptions} [options]
   * @returns {AsyncGenerator<SseEvent<T>, void, unknown>}
   */
  streamEvents(options) {
    const { signal, lastEventId } = normalizeEventStreamOptions(
      options,
      "streamEvents",
      ["filter"],
    );
    const params = {};
    const filterValue =
      options && typeof options === "object" ? options.filter : undefined;
    const filterPayload = ToriiClient._normalizeEventFilter(filterValue);
    if (filterPayload) {
      params.filter = filterPayload;
    }
    return this._streamSse("/v1/events/sse", {
      params: Object.keys(params).length > 0 ? params : undefined,
      lastEventId,
      signal,
    });
  }

  /**
   * Stream Sumeragi status events (`/v1/sumeragi/status/sse`).
   * @template [T=unknown]
   * @param {Omit<EventStreamOptions, "filter">} [options]
   * @returns {AsyncGenerator<SseEvent<T>, void, unknown>}
   */
  streamSumeragiStatus(options) {
    const { signal, lastEventId } = normalizeEventStreamOptions(
      options,
      "streamSumeragiStatus",
    );
    return this._streamSse("/v1/sumeragi/status/sse", { lastEventId, signal });
  }

  /**
   * Fetch the current Kaigi call view (`GET /v1/kaigi/calls/{call_id}`).
   * @param {string} callId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<KaigiCallView | null>}
   */
  async getKaigiCall(callId, options = {}) {
    const normalizedCallId = requireNonEmptyString(callId, "callId");
    const { signal } = normalizeSignalOnlyOption(options, "getKaigiCall");
    const response = await this._request(
      "GET",
      `/v1/kaigi/calls/${encodeURIComponent(normalizedCallId)}`,
      { headers: { Accept: "application/json" }, signal },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("kaigi call endpoint returned no payload");
    }
    return normalizeKaigiCallView(payload);
  }

  /**
   * Fetch committed Kaigi signal metadata for a call (`GET /v1/kaigi/calls/{call_id}/signals`).
   * @param {string} callId
   * @param {KaigiCallSignalsOptions} [options]
   * @returns {Promise<KaigiCallSignalsList>}
   */
  async listKaigiCallSignals(callId, options = {}) {
    const normalizedCallId = requireNonEmptyString(callId, "callId");
    const { signal, params } = buildKaigiCallSignalsQuery(options);
    const response = await this._request(
      "GET",
      `/v1/kaigi/calls/${encodeURIComponent(normalizedCallId)}/signals`,
      {
        headers: { Accept: "application/json" },
        params,
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeKaigiCallSignalsList(payload);
  }

  /**
   * Stream Kaigi call lifecycle updates (`/v1/kaigi/calls/{call_id}/events`).
   * @param {string} callId
   * @param {KaigiCallEventsOptions} [options]
   * @returns {AsyncGenerator<SseEvent<KaigiCallEventPayload>, void, unknown>}
   */
  streamKaigiCallEvents(callId, options) {
    const normalizedCallId = requireNonEmptyString(callId, "callId");
    const { signal, lastEventId } = normalizeEventStreamOptions(
      options,
      "streamKaigiCallEvents",
      ["kind"],
    );
    const params = buildKaigiCallEventParams(options);
    const iterator = this._streamSse(
      `/v1/kaigi/calls/${encodeURIComponent(normalizedCallId)}/events`,
      {
        params,
        lastEventId,
        signal,
      },
    );
    return (async function* mapEvents() {
      for await (const event of iterator) {
        let data = event.data;
        if (data && typeof data === "object") {
          data = normalizeKaigiCallEventData(data);
        }
        yield {
          ...event,
          data,
        };
      }
    })();
  }

  /**
   * Fetch Kaigi relay summaries (`GET /v1/kaigi/relays`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<KaigiRelaySummaryList>}
   */
  async listKaigiRelays(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listKaigiRelays");
    const response = await this._request("GET", "/v1/kaigi/relays", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeKaigiRelaySummaryList(payload);
  }

  /**
   * Fetch detailed metadata for a Kaigi relay (`GET /v1/kaigi/relays/{relay_id}`).
   * @param {string} relayId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<KaigiRelayDetail | null>}
   */
  async getKaigiRelay(relayId, options = {}) {
    const normalizedRelay = ToriiClient._requireAccountId(relayId, "relayId");
    const { signal } = normalizeSignalOnlyOption(options, "getKaigiRelay");
    const response = await this._request(
      "GET",
      `/v1/kaigi/relays/${encodeURIComponent(normalizedRelay)}`,
      { headers: { Accept: "application/json" }, signal },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("kaigi relay detail endpoint returned no payload");
    }
    return normalizeKaigiRelayDetail(payload);
  }

  /**
   * Fetch aggregated Kaigi relay health metrics (`GET /v1/kaigi/relays/health`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<KaigiRelayHealthSnapshot>}
   */
  async getKaigiRelaysHealth(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getKaigiRelaysHealth");
    const response = await this._request("GET", "/v1/kaigi/relays/health", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeKaigiRelayHealthSnapshot(payload);
  }

  /**
   * Stream Kaigi relay registration/health updates (`/v1/kaigi/relays/events`).
   * @param {KaigiRelayEventsOptions} [options]
   * @returns {AsyncGenerator<SseEvent<KaigiRelayEventPayload>, void, unknown>}
   */
  streamKaigiRelayEvents(options) {
    const { signal, lastEventId } = normalizeEventStreamOptions(
      options,
      "streamKaigiRelayEvents",
      ["domain", "relay", "kind"],
    );
    const params = buildKaigiRelayEventParams(options);
    const iterator = this._streamSse("/v1/kaigi/relays/events", {
      params,
      lastEventId,
      signal,
    });
    return (async function* mapEvents() {
      for await (const event of iterator) {
        let data = event.data;
        if (data && typeof data === "object") {
          data = normalizeKaigiRelayEventData(data);
        }
        yield {
          ...event,
          data,
        };
      }
    })();
  }

  /**
   * List prover reports with optional filters.
   * @param {Record<string, unknown>} filters
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ReadonlyArray<ToriiProverReport>>}
   * @throws When the server projection omits report fields (e.g. `ids_only=true`).
   */
  async listProverReports(filters = {}, options = {}) {
    const normalizedFilters = ToriiClient._encodeProverFilters(filters);
    const { signal } = normalizeSignalOnlyOption(options, "listProverReports");
    const response = await this._request("GET", "/v1/zk/prover/reports", {
      params: normalizedFilters,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeProverReportList(payload, normalizedFilters, "prover reports response");
  }

  /**
   * Iterate prover reports with offset-based pagination and typed projections.
   * @param {ToriiProverReportFilters} [filters]
   * @param {PaginationIteratorOptions & {signal?: AbortSignal}} [options]
   * @returns {AsyncGenerator<ToriiProverReport | string | ToriiProverReportMessageSummary, void, unknown>}
   */
  iterateProverReports(filters = {}, options = {}) {
    const fetchPage = async (pageOptions = {}) => {
      const { signal, ...rest } = pageOptions ?? {};
      const pageFilters = { ...filters, ...rest };
      const result = await this.listProverReports(pageFilters, { signal });
      if (result.kind === "ids") {
        return { items: result.ids ?? [] };
      }
      if (result.kind === "messages") {
        return { items: result.messages ?? [] };
      }
      return { items: result.reports ?? [] };
    };
    return this._iterateOffsetIterable(
      fetchPage,
      options,
      PROVER_REPORT_ITERATOR_OPTION_KEYS,
    );
  }

  /**
   * Fetch Connect overlay status (`GET /v1/connect/status`).
   * Returns null when Connect is disabled on the node.
   * @returns {Promise<any | null>}
   */
  async getConnectStatus() {
    const response = await this._request("GET", "/v1/connect/status", {
      headers: { Accept: "application/json" },
    });
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("connect status response missing JSON body");
    }
    return normalizeConnectStatusSnapshot(payload, "connect status response");
  }

  /**
   * Create a Connect session and retrieve deeplink URIs (`POST /v1/connect/session`).
   * @param {{ sid: string; node?: string | null }} input
   * @returns {Promise<ConnectSessionResponse>}
   */
  async createConnectSession(input = {}) {
    if (!input || typeof input !== "object") {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "createConnectSession input must be an object",
        "createConnectSession.input",
      );
    }
    const payload = {
      sid: normalizeConnectSid(input.sid, "sid"),
    };
    if (input.node !== undefined && input.node !== null) {
      payload.node = requireNonEmptyString(input.node, "node");
    }
    const response = await this._request("POST", "/v1/connect/session", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("connect session response missing JSON body");
    }
    return normalizeConnectSessionResponse(body, "connect session response");
  }

  /**
   * Delete a Connect session (`DELETE /v1/connect/session/{sid}`).
   * @param {string} sid
   * @returns {Promise<boolean>} True when the session existed.
   */
  async deleteConnectSession(sid) {
    const normalizedSid = normalizeConnectSid(sid, "sid");
    const response = await this._request(
      "DELETE",
      `/v1/connect/session/${encodeURIComponent(normalizedSid)}`,
    );
    if (response.status === 404) {
      return false;
    }
    await this._expectStatus(response, [204]);
    return true;
  }

  /**
   * List registered Connect applications (`GET /v1/connect/app/apps`).
   * @param {{limit?: number, cursor?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAppRegistryPage>}
   */
  async listConnectApps(options = {}) {
    const resolvedOptions =
      options === undefined || options === null
        ? {}
        : requirePlainObjectOption(options, "connect app list options");
    assertSupportedOptionKeys(
      resolvedOptions,
      CONNECT_APP_LIST_OPTION_KEYS,
      "listConnectApps options",
    );
    const { signal } = normalizeSignalOption(resolvedOptions, "listConnectApps");
    const params = {};
    if (resolvedOptions.limit !== undefined && resolvedOptions.limit !== null) {
      params.limit = ToriiClient._normalizeUnsignedInteger(
        resolvedOptions.limit,
        "connectApps.limit",
        { allowZero: false },
      );
    }
    if (resolvedOptions.cursor !== undefined && resolvedOptions.cursor !== null) {
      params.cursor = requireNonEmptyString(resolvedOptions.cursor, "connectApps.cursor");
    }
    const response = await this._request("GET", "/v1/connect/app/apps", {
      headers: { Accept: "application/json" },
      params: Object.keys(params).length === 0 ? undefined : params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeConnectAppRegistryPage(payload);
  }

  /**
   * Iterate Connect applications with cursor-based pagination.
   * @param {ConnectAppIteratorOptions} [options]
   * @returns {AsyncGenerator<ConnectAppRecord, void, unknown>}
   */
  iterateConnectApps(options = {}) {
    return this._iterateCursorIterable(this.listConnectApps, options);
  }

  /**
   * Fetch a single Connect application (`GET /v1/connect/app/apps/{app_id}`).
   * @param {string} appId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAppRecord>}
  */
  async getConnectApp(appId, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getConnectApp");
    const normalizedId = requireNonEmptyString(appId, "appId");
    const response = await this._request(
      "GET",
      `/v1/connect/app/apps/${encodeURIComponent(normalizedId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeConnectAppRecord(payload, "connect app response");
  }

  /**
   * Register or update a Connect application (`POST /v1/connect/app/apps`).
   * @param {ConnectAppUpsertInput} record
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAppRecord | null>}
   */
  async registerConnectApp(record, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "registerConnectApp");
    const payload = toConnectAppWritePayload(record, "registerConnectApp.record");
    const response = await this._request("POST", "/v1/connect/app/apps", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 201, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      return null;
    }
    return normalizeConnectAppRecord(body, "connect app response");
  }

  /**
   * Delete a Connect application (`DELETE /v1/connect/app/apps/{app_id}`).
   * @param {string} appId
   * @returns {Promise<boolean>} True when the record existed.
   */
  async deleteConnectApp(appId) {
    const normalizedId = requireNonEmptyString(appId, "appId");
    const response = await this._request(
      "DELETE",
      `/v1/connect/app/apps/${encodeURIComponent(normalizedId)}`,
    );
    if (response.status === 404) {
      return false;
    }
    await this._expectStatus(response, [200, 202, 204]);
    return true;
  }

  /**
   * Fetch mutable Connect policy toggles (`GET /v1/connect/app/policy`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAppPolicyControls>}
   */
  async getConnectAppPolicy(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getConnectAppPolicy");
    const response = await this._request("GET", "/v1/connect/app/policy", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeConnectAppPolicyControls(payload, "connect app policy response");
  }

  /**
   * Update Connect policy toggles (`POST /v1/connect/app/policy`).
   * @param {ConnectAppPolicyUpdate} updates
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAppPolicyControls>}
   */
  async updateConnectAppPolicy(updates, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "updateConnectAppPolicy");
    const payload = toConnectAppPolicyPayload(updates, "updateConnectAppPolicy.updates");
    const response = await this._request("POST", "/v1/connect/app/policy", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    return normalizeConnectAppPolicyControls(body, "connect app policy response");
  }

  /**
   * Fetch the Connect admission manifest (`GET /v1/connect/app/manifest`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAdmissionManifest>}
   */
  async getConnectAdmissionManifest(options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "getConnectAdmissionManifest",
    );
    const response = await this._request("GET", "/v1/connect/app/manifest", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeConnectAdmissionManifest(payload, "connect admission manifest");
  }

  /**
   * Replace the Connect admission manifest (`PUT /v1/connect/app/manifest`).
   * @param {ConnectAdmissionManifestInput} manifest
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ConnectAdmissionManifest>}
   */
  async setConnectAdmissionManifest(manifest, options = {}) {
    const { signal } = normalizeSignalOnlyOption(
      options,
      "setConnectAdmissionManifest",
    );
    const payload = toConnectAdmissionManifestPayload(
      manifest,
      "setConnectAdmissionManifest.manifest",
    );
    const response = await this._request("PUT", "/v1/connect/app/manifest", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    return normalizeConnectAdmissionManifest(body, "connect admission manifest");
  }

  /**
   * Build a Connect WebSocket URL using the client's base Torii URL.
   * @param {{sid: string, role: string, token: string, endpointPath?: string}} options
   * @returns {string}
   */
  buildConnectWebSocketUrl(options = {}) {
    return buildConnectWebSocketUrlInternal(
      this._baseUrl,
      options,
      "ToriiClient.buildConnectWebSocketUrl",
    );
  }

  /**
   * Open a Connect WebSocket (`/v1/connect/ws`) using the provided implementation.
   * @param {{
   *   sid: string,
   *   role: string,
   *   token: string,
   *   endpointPath?: string,
   *   protocols?: string | ReadonlyArray<string>,
   *   websocketOptions?: unknown,
   *   WebSocketImpl?: typeof WebSocket
   * }} options
   * @returns {unknown}
   */
  openConnectWebSocket(options = {}) {
    const normalizedOptions =
      options === undefined
        ? {}
        : requirePlainObjectOption(options, "ToriiClient.openConnectWebSocket options", {
            message: "must be an object",
          });
    const {
      insecureTransportTelemetryHook,
      ...rest
    } = normalizedOptions;
    const allowInsecure = rest.allowInsecure ?? this._allowInsecure;
    const telemetryHook =
      insecureTransportTelemetryHook ?? this._config.insecureTransportTelemetryHook;
    return openConnectWebSocketInternal(
      {
        ...rest,
        allowInsecure,
        baseUrl: this._baseUrl,
        insecureTransportTelemetryHook: telemetryHook,
      },
      "ToriiClient.openConnectWebSocket",
    );
  }

  /**
   * Build a Connect WebSocket URL for an arbitrary Torii base.
   * @param {string} baseUrl
   * @param {{sid: string, role: string, token: string, endpointPath?: string}} options
   * @returns {string}
   */
  static buildConnectWebSocketUrl(baseUrl, options = {}) {
    return buildConnectWebSocketUrlInternal(
      baseUrl,
      options,
      "ToriiClient.buildConnectWebSocketUrl",
    );
  }

  static buildRbcSampleRequest(session, overrides = {}) {
    return buildRbcSampleRequestInternal(
      session,
      overrides,
      "ToriiClient.buildRbcSampleRequest",
    );
  }

  /**
   * Register a contract manifest via Torii (`POST /v1/contracts/code`).
   * Wraps `RegisterSmartContractCode` into a signed transaction.
   * @param {RegisterContractCodeRequest} request
   * @returns {Promise<unknown | null>}
   */
  async registerContractCode(request = {}) {
    const payload = normalizeRegisterContractCodeRequest(request);
    const response = await this._request("POST", "/v1/contracts/code", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    await this._expectStatus(response, [200, 202]);
    return this._maybeJson(response);
  }

  /**
   * Deploy contract bytecode and register the manifest (`POST /v1/contracts/deploy`).
   * @param {DeployContractRequest} request
   * @returns {Promise<DeployContractResponse | null>}
   */
  async deployContract(request = {}) {
    const payload = normalizeDeployContractRequest(request);
    const response = await this._request("POST", "/v1/contracts/deploy", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    return body ? normalizeDeployContractResponse(body) : null;
  }

  /**
   * Invoke a deployed contract instance (`POST /v1/contracts/call`).
   * @param {ContractCallRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ContractCallResponse>}
  */
  async callContract(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "callContract");
    const payload = normalizeContractCallRequest(request);
    const response = await this._request("POST", "/v1/contracts/call", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("contract call endpoint returned no payload");
    }
    return normalizeContractCallResponse(body);
  }

  /**
   * Propose a contract call through a multisig authority (`POST /v1/contracts/call/multisig/propose`).
   * @param {object} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async proposeMultisigContractCall(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "proposeMultisigContractCall");
    const payload = normalizeMultisigContractCallProposeRequest(request);
    const response = await this._request("POST", "/v1/contracts/call/multisig/propose", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("multisig contract propose endpoint returned no payload");
    }
    return normalizeMultisigContractCallResponse(
      body,
      "multisig contract propose response",
    );
  }

  /**
   * Approve a multisig contract call proposal (`POST /v1/contracts/call/multisig/approve`).
   * @param {object} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async approveMultisigContractCall(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "approveMultisigContractCall");
    const payload = normalizeMultisigContractCallApproveRequest(request);
    const response = await this._request("POST", "/v1/contracts/call/multisig/approve", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("multisig contract approve endpoint returned no payload");
    }
    return normalizeMultisigContractCallResponse(
      body,
      "multisig contract approve response",
    );
  }

  /**
   * Resolve the current multisig spec through an alias-aware selector (`POST /v1/multisig/spec`).
   * @param {object} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getMultisigSpec(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getMultisigSpec");
    const payload = normalizeMultisigSelectorOnlyRequest(request, "getMultisigSpec request");
    const response = await this._request("POST", "/v1/multisig/spec", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("multisig spec endpoint returned no payload");
    }
    return normalizeMultisigSpecResponse(body);
  }

  /**
   * List nonterminal multisig proposals for a selector (`POST /v1/multisig/proposals/list`).
   * @param {object} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async listMultisigProposals(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "listMultisigProposals");
    const payload = normalizeMultisigSelectorOnlyRequest(
      request,
      "listMultisigProposals request",
    );
    const response = await this._request("POST", "/v1/multisig/proposals/list", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("multisig proposals list endpoint returned no payload");
    }
    return normalizeMultisigProposalsListResponse(body);
  }

  /**
   * Fetch one multisig proposal by proposal id or instructions hash (`POST /v1/multisig/proposals/get`).
   * @param {object} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<object>}
   */
  async getMultisigProposal(request = {}, options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getMultisigProposal");
    const payload = normalizeMultisigProposalLookupRequest(request);
    const response = await this._request("POST", "/v1/multisig/proposals/get", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("multisig proposal get endpoint returned no payload");
    }
    return normalizeMultisigProposalGetResponse(body);
  }

  /**
   * Fetch on-chain contract manifest by code hash (`GET /v1/contracts/code/{hash}`).
   * @param {string} codeHashHex
   * @returns {Promise<ContractManifestRecord | null>}
   */
  async getContractManifest(codeHashHex) {
    const normalizedHash = normalizeHex32String(codeHashHex, "codeHashHex");
    const response = await this._request("GET", `/v1/contracts/code/${normalizedHash}`, {
      headers: { Accept: "application/json" },
    });
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    return normalizeContractManifestResponse(payload);
  }

  /**
   * Fetch stored contract code bytes (`GET /v1/contracts/code-bytes/{hash}`).
   * @param {string} codeHashHex
   * @returns {Promise<ContractCodeBytesRecord | null>}
   */
  async getContractCodeBytes(codeHashHex) {
    const normalizedHash = normalizeHex32String(codeHashHex, "codeHashHex");
    const response = await this._request(
      "GET",
      `/v1/contracts/code-bytes/${normalizedHash}`,
      { headers: { Accept: "application/json" } },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    return normalizeContractCodeBytesResponse(payload);
  }

  /**
   * Read the governance binding for one contract address (`GET /v1/gov/contracts/{contract_address}`).
   * @param {string} contractAddress
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiGovernanceContractResponse>}
   */
  async getGovernanceContract(contractAddress, options = {}) {
    const normalizedAddress = requireNonEmptyString(contractAddress, "contractAddress");
    const { signal } = normalizeSignalOnlyOption(options, "getGovernanceContract");
    const response = await this._request(
      "GET",
      `/v1/gov/contracts/${encodeURIComponent(normalizedAddress)}`,
      {
        headers: { Accept: "application/json" },
        signal: signal ?? undefined,
      },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeGovernanceContractResponse(payload);
  }

  /**
   * List registered triggers (`GET /v1/triggers`).
   * @param {TriggerListOptions} [options]
   * @returns {Promise<ToriiTriggerListPage>}
   */
  async listTriggers(options = {}) {
    const { signal, params } = buildTriggerListQuery(options);
    const response = await this._request("GET", "/v1/triggers", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeTriggerListResponse(payload, "trigger list response");
  }

  /**
   * Fetch a trigger definition (`GET /v1/triggers/{trigger_id}`).
   * @param {string} triggerId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiTriggerRecord | null>}
  */
  async getTrigger(triggerId, options = {}) {
    const normalizedId = requireNonEmptyString(triggerId, "triggerId");
    const { signal } = normalizeSignalOnlyOption(options, "getTrigger");
    const response = await this._request(
      "GET",
      `/v1/triggers/${encodeURIComponent(normalizedId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    return normalizeTriggerRecord(payload, "trigger response");
  }

  /**
   * Register or update a trigger (`POST /v1/triggers`).
   * @param {ToriiTriggerUpsertRequest} trigger
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async registerTrigger(trigger, options = {}) {
    const payload = normalizeTriggerUpsertPayload(trigger);
    const { signal } = normalizeSignalOnlyOption(options, "registerTrigger");
    const response = await this._request("POST", "/v1/triggers", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 201, 202]);
    return this._maybeJson(response);
  }

  /**
   * Register or update a trigger and normalise the response payload.
   * @param {ToriiTriggerUpsertRequest} trigger
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiTriggerMutationResponse | null>}
   */
  async registerTriggerTyped(trigger, options = {}) {
    const payload = await this.registerTrigger(trigger, options);
    if (!payload) {
      return null;
    }
    return normalizeTriggerMutationResponse(payload, "registerTrigger response");
  }

  /**
   * Delete a trigger (`DELETE /v1/triggers/{trigger_id}`).
   * @param {string} triggerId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async deleteTrigger(triggerId, options = {}) {
    const normalizedId = requireNonEmptyString(triggerId, "triggerId");
    const { signal } = normalizeSignalOnlyOption(options, "deleteTrigger");
    const response = await this._request(
      "DELETE",
      `/v1/triggers/${encodeURIComponent(normalizedId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    await this._expectStatus(response, [200, 202, 204, 404]);
    return this._maybeJson(response);
  }

  /**
   * Delete a trigger and normalise the response payload when available.
   * @param {string} triggerId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiTriggerMutationResponse | null>}
   */
  async deleteTriggerTyped(triggerId, options = {}) {
    const payload = await this.deleteTrigger(triggerId, options);
    if (!payload) {
      return null;
    }
    return normalizeTriggerMutationResponse(payload, "deleteTrigger response");
  }

  /**
   * Query triggers with structured filters (`POST /v1/triggers/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<ToriiTriggerListPage>}
   */
  async queryTriggers(options = {}) {
    const normalizedOptions = normalizeIterableQueryOptions(
      options,
      "trigger query options",
    );
    const { signal, ...rest } = normalizedOptions;
    const envelope = ToriiClient._buildIterableQueryEnvelope(rest);
    const response = await this._request("POST", "/v1/triggers/query", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(envelope),
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeTriggerListResponse(payload, "trigger query response");
  }

  /**
   * Iterate triggers using the `GET /v1/triggers` endpoint.
   * @param {TriggerIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiTriggerRecord, void, unknown>}
   */
  iterateTriggers(options = {}) {
    return this._iterateIterable(this.listTriggers, options);
  }

  /**
   * Iterate triggers using the structured query endpoint.
   * @param {TriggerQueryIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiTriggerRecord, void, unknown>}
   */
  iterateTriggersQuery(options = {}) {
    return this._iterateIterable(this.queryTriggers, options);
  }

  /**
   * List subscription plans (`GET /v1/subscriptions/plans`).
   * @param {SubscriptionPlanListOptions} [options]
   * @returns {Promise<SubscriptionPlanListResponse>}
   */
  async listSubscriptionPlans(options = {}) {
    const { signal, params } = buildSubscriptionPlanListQuery(options);
    const response = await this._request("GET", "/v1/subscriptions/plans", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSubscriptionPlanListResponse(payload);
  }

  /**
   * Iterate subscription plans using the list endpoint.
   * @param {SubscriptionPlanIteratorOptions} [options]
   * @returns {AsyncGenerator<SubscriptionPlanListItem, void, unknown>}
   */
  iterateSubscriptionPlans(options = {}) {
    return this._iterateIterable(this.listSubscriptionPlans, options);
  }

  /**
   * Create a subscription plan (`POST /v1/subscriptions/plans`).
   * @param {SubscriptionPlanCreateRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionPlanCreateResponse>}
   */
  async createSubscriptionPlan(request, options = {}) {
    const payload = normalizeSubscriptionPlanCreateRequest(request);
    const { signal } = normalizeSignalOnlyOption(options, "createSubscriptionPlan");
    const response = await this._request("POST", "/v1/subscriptions/plans", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription plan create endpoint returned no payload");
    }
    return normalizeSubscriptionPlanCreateResponse(body);
  }

  /**
   * List subscriptions (`GET /v1/subscriptions`).
   * @param {SubscriptionListOptions} [options]
   * @returns {Promise<SubscriptionListResponse>}
   */
  async listSubscriptions(options = {}) {
    const { signal, params } = buildSubscriptionListQuery(options);
    const response = await this._request("GET", "/v1/subscriptions", {
      headers: { Accept: "application/json" },
      params,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    return normalizeSubscriptionListResponse(payload);
  }

  /**
   * Iterate subscriptions using the list endpoint.
   * @param {SubscriptionIteratorOptions} [options]
   * @returns {AsyncGenerator<SubscriptionListItem, void, unknown>}
   */
  iterateSubscriptions(options = {}) {
    return this._iterateIterable(this.listSubscriptions, options);
  }

  /**
   * Create a subscription (`POST /v1/subscriptions`).
   * @param {SubscriptionCreateRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionCreateResponse>}
   */
  async createSubscription(request, options = {}) {
    const payload = normalizeSubscriptionCreateRequest(request);
    const { signal } = normalizeSignalOnlyOption(options, "createSubscription");
    const response = await this._request("POST", "/v1/subscriptions", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription create endpoint returned no payload");
    }
    return normalizeSubscriptionCreateResponse(body);
  }

  /**
   * Fetch a subscription (`GET /v1/subscriptions/{subscription_id}`).
   * @param {string} subscriptionId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionGetResponse | null>}
   */
  async getSubscription(subscriptionId, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const { signal } = normalizeSignalOnlyOption(options, "getSubscription");
    const response = await this._request(
      "GET",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}`,
      {
        headers: { Accept: "application/json" },
        signal,
      },
    );
    if (response.status === 404) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    return normalizeSubscriptionGetResponse(payload);
  }

  /**
   * Pause a subscription (`POST /v1/subscriptions/{subscription_id}/pause`).
   * @param {string} subscriptionId
   * @param {SubscriptionActionRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async pauseSubscription(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionActionRequest(request, "pauseSubscription");
    const { signal } = normalizeSignalOnlyOption(options, "pauseSubscription");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/pause`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription pause endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "pauseSubscription response");
  }

  /**
   * Resume a subscription (`POST /v1/subscriptions/{subscription_id}/resume`).
   * @param {string} subscriptionId
   * @param {SubscriptionActionRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async resumeSubscription(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionActionRequest(request, "resumeSubscription");
    const { signal } = normalizeSignalOnlyOption(options, "resumeSubscription");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/resume`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription resume endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "resumeSubscription response");
  }

  /**
   * Cancel a subscription (`POST /v1/subscriptions/{subscription_id}/cancel`).
   * @param {string} subscriptionId
   * @param {SubscriptionActionRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async cancelSubscription(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionActionRequest(request, "cancelSubscription");
    const { signal } = normalizeSignalOnlyOption(options, "cancelSubscription");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/cancel`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription cancel endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "cancelSubscription response");
  }

  /**
   * Keep a subscription (`POST /v1/subscriptions/{subscription_id}/keep`).
   * @param {string} subscriptionId
   * @param {SubscriptionActionRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async keepSubscription(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionActionRequest(request, "keepSubscription");
    const { signal } = normalizeSignalOnlyOption(options, "keepSubscription");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/keep`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription keep endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "keepSubscription response");
  }

  /**
   * Charge a subscription immediately (`POST /v1/subscriptions/{subscription_id}/charge-now`).
   * @param {string} subscriptionId
   * @param {SubscriptionActionRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async chargeSubscriptionNow(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionActionRequest(
      request,
      "chargeSubscriptionNow",
    );
    const { signal } = normalizeSignalOnlyOption(options, "chargeSubscriptionNow");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/charge-now`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription charge-now endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "chargeSubscriptionNow response");
  }

  /**
   * Record subscription usage (`POST /v1/subscriptions/{subscription_id}/usage`).
   * @param {string} subscriptionId
   * @param {SubscriptionUsageRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<SubscriptionActionResponse>}
   */
  async recordSubscriptionUsage(subscriptionId, request, options = {}) {
    const normalizedId = requireNonEmptyString(subscriptionId, "subscriptionId");
    const payload = normalizeSubscriptionUsageRequest(request, "recordSubscriptionUsage");
    const { signal } = normalizeSignalOnlyOption(options, "recordSubscriptionUsage");
    const response = await this._request(
      "POST",
      `/v1/subscriptions/${encodeURIComponent(normalizedId)}/usage`,
      {
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify(payload),
        signal,
      },
    );
    await this._expectStatus(response, [200, 202]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("subscription usage endpoint returned no payload");
    }
    return normalizeSubscriptionActionResponse(body, "recordSubscriptionUsage response");
  }

  /**
   * Fetch offline cash feature readiness (`GET /v1/offline/cash/readiness`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashReadinessResponse>}
   */
  async getOfflineCashReadiness(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getOfflineCashReadiness");
    const response = await this._request("GET", "/v1/offline/cash/readiness", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("offline cash readiness response missing JSON body");
    }
    return normalizeOfflineCashReadinessResponse(
      body,
      "offline cash readiness response",
    );
  }

  /**
   * Submit an offline cash setup request (`POST /v1/offline/cash/setup`).
   * @param {ToriiOfflineCashRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashEnvelope>}
   */
  async setupOfflineCash(request, options = {}) {
    return this._postOfflineCashRequest(
      "/v1/offline/cash/setup",
      request,
      "setupOfflineCash",
      options,
    );
  }

  /**
   * Submit an offline cash load request (`POST /v1/offline/cash/load`).
   * @param {ToriiOfflineCashRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashEnvelope>}
   */
  async loadOfflineCash(request, options = {}) {
    return this._postOfflineCashRequest(
      "/v1/offline/cash/load",
      request,
      "loadOfflineCash",
      options,
    );
  }

  /**
   * Submit an offline cash refresh request (`POST /v1/offline/cash/refresh`).
   * @param {ToriiOfflineCashRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashEnvelope>}
   */
  async refreshOfflineCash(request, options = {}) {
    return this._postOfflineCashRequest(
      "/v1/offline/cash/refresh",
      request,
      "refreshOfflineCash",
      options,
    );
  }

  /**
   * Submit an offline cash sync request (`POST /v1/offline/cash/sync`).
   * @param {ToriiOfflineCashRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashEnvelope>}
   */
  async syncOfflineCash(request, options = {}) {
    return this._postOfflineCashRequest(
      "/v1/offline/cash/sync",
      request,
      "syncOfflineCash",
      options,
    );
  }

  /**
   * Submit an offline cash redeem request (`POST /v1/offline/cash/redeem`).
   * @param {ToriiOfflineCashRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineCashEnvelope>}
   */
  async redeemOfflineCash(request, options = {}) {
    return this._postOfflineCashRequest(
      "/v1/offline/cash/redeem",
      request,
      "redeemOfflineCash",
      options,
    );
  }

  /**
   * Fetch the signed offline revocation bundle (`GET /v1/offline/revocations/bundle`).
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineRevocationBundle>}
   */
  async getOfflineRevocationBundle(options = {}) {
    const { signal } = normalizeSignalOnlyOption(options, "getOfflineRevocationBundle");
    const response = await this._request("GET", "/v1/offline/revocations/bundle", {
      headers: { Accept: "application/json" },
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("offline revocation bundle response missing JSON body");
    }
    return normalizeOfflineRevocationBundleResponse(
      body,
      "offline revocation bundle response",
    );
  }

  async _postOfflineCashRequest(path, request, context, options = {}) {
    const payload = normalizeOfflineCashRequest(request, context);
    const { signal } = normalizeSignalOnlyOption(options, context);
    const response = await this._request("POST", path, {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error(`${context} response missing JSON body`);
    }
    return normalizeOfflineCashEnvelope(body, `${context} response`);
  }

  /**
   * List offline transfers (`GET /v1/offline/transfers`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<ToriiOfflineTransferListResponse>}
   */
  async listOfflineTransfers(options = {}) {
    if (options && isPlainObject(options.filter)) {
      ToriiClient._validateOfflineTransferFilter(options.filter, "options.filter");
    }
    return this._listIterable(
      "/v1/offline/transfers",
      options,
      (payload) =>
        normalizeOfflineTransferListResponse(payload, "offline transfers response"),
      OFFLINE_ITERABLE_OPTION_KEYS,
    );
  }

  /**
   * Query offline transfers (`POST /v1/offline/transfers/query`).
   * @param {IterableQueryOptions} [options]
   * @returns {Promise<ToriiOfflineTransferListResponse>}
   */
  async queryOfflineTransfers(options = {}) {
    return this._queryIterable(
      "/v1/offline/transfers/query",
      options,
      (payload) =>
        normalizeOfflineTransferListResponse(payload, "offline transfers query response"),
      (envelope) => {
        if (envelope && envelope.filter && isPlainObject(envelope.filter)) {
          ToriiClient._validateOfflineTransferFilter(envelope.filter, "filter");
        }
      },
      OFFLINE_ITERABLE_OPTION_KEYS,
      true,
    );
  }

  /**
   * Iterate offline transfers with automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiOfflineTransferItem, void, unknown>}
   */
  iterateOfflineTransfers(options = {}) {
    return this._iterateIterable(this.listOfflineTransfers, options);
  }

  /**
   * Iterate offline transfers via the structured query endpoint.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiOfflineTransferItem, void, unknown>}
   */
  iterateOfflineTransfersQuery(options = {}) {
    return this._iterateIterable(this.queryOfflineTransfers, options);
  }

  /**
   * List offline verdict revocations (`GET /v1/offline/revocations`).
   * @param {IterableListOptions} [options]
   * @returns {Promise<ToriiOfflineRevocationListResponse>}
   */
  async listOfflineRevocations(options = {}) {
    if (options && isPlainObject(options.filter)) {
      ToriiClient._validateOfflineRevocationFilter(options.filter, "options.filter");
    }
    return this._listIterable(
      "/v1/offline/revocations",
      options,
      (payload) =>
        normalizeOfflineRevocationListResponse(payload, "offline revocations response"),
      OFFLINE_ITERABLE_OPTION_KEYS,
    );
  }

  /**
   * Iterate offline verdict revocations with automatic pagination.
   * @param {PaginationIteratorOptions} [options]
   * @returns {AsyncGenerator<ToriiOfflineRevocationItem, void, unknown>}
   */
  iterateOfflineRevocations(options = {}) {
    return this._iterateIterable(this.listOfflineRevocations, options);
  }

  /**
   * Issue an operator-signed build claim (`POST /v1/offline/build-claims/issue`).
   * @param {ToriiOfflineBuildClaimIssueRequest} request
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineBuildClaimIssueResponse>}
   */
  async issueOfflineBuildClaim(request, options = {}) {
    const payload = normalizeOfflineBuildClaimIssueRequest(
      request,
      "issueOfflineBuildClaim",
    );
    const { signal } = normalizeSignalOnlyOption(options, "issueOfflineBuildClaim");
    const response = await this._request("POST", "/v1/offline/build-claims/issue", {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
      signal,
    });
    await this._expectStatus(response, [200]);
    const body = await this._maybeJson(response);
    if (!body) {
      throw new Error("offline build claim issue response missing JSON body");
    }
    return normalizeOfflineBuildClaimIssueResponse(
      body,
      "offline build claim issue response",
    );
  }

  /**
   * Fetch aggregated offline rejection counters (`GET /v1/offline/rejections`).
   * Returns null when telemetry outputs are disabled for the active profile.
   * @param {{telemetryProfile?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiOfflineRejectionStatsResponse | null>}
   */
  async getOfflineRejectionStats(options = {}) {
    const normalizedOptions =
      options === undefined
        ? {}
        : ensureRecord(options, "getOfflineRejectionStats options");
    assertSupportedOptionKeys(
      normalizedOptions,
      OFFLINE_REJECTION_STATS_OPTION_KEYS,
      "getOfflineRejectionStats options",
    );
    const { signal } = normalizeSignalOption(
      normalizedOptions,
      "getOfflineRejectionStats",
    );
    let telemetryProfile;
    if (normalizedOptions.telemetryProfile !== undefined) {
      if (normalizedOptions.telemetryProfile === null) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          "getOfflineRejectionStats options.telemetryProfile must be a non-empty string",
          "getOfflineRejectionStats.options.telemetryProfile",
        );
      }
      telemetryProfile = requireNonEmptyString(
        normalizedOptions.telemetryProfile,
        "options.telemetryProfile",
      );
    }
    const headers = { Accept: "application/json" };
    if (telemetryProfile) {
      headers["X-Torii-Telemetry-Profile"] = telemetryProfile;
    }
    const response = await this._request("GET", "/v1/offline/rejections", {
      headers,
      signal,
    });
    if (response.status === 403 || response.status === 404 || response.status === 503) {
      return null;
    }
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("offline rejections endpoint returned no payload");
    }
    return normalizeOfflineRejectionStatsResponse(payload, "offline rejections response");
  }

  /**
   * Fetch a single prover report.
   * @param {string} reportId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<ToriiProverReport>}
   */
  async getProverReport(reportId, options = {}) {
    const normalizedId = requireNonEmptyString(reportId, "reportId");
    const { signal } = normalizeSignalOnlyOption(options, "getProverReport");
    const response = await this._request(
      "GET",
      `/v1/zk/prover/reports/${encodeURIComponent(normalizedId)}`,
      { signal },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      throw new Error("prover report response missing JSON body");
    }
    return normalizeProverReportRecord(payload, "prover report response");
  }

  /**
   * Delete a prover report by id.
   * @param {string} reportId
   */
  async deleteProverReport(reportId, options = {}) {
    const normalizedId = requireNonEmptyString(reportId, "reportId");
    const { signal } = normalizeSignalOnlyOption(options, "deleteProverReport");
    const response = await this._request(
      "DELETE",
      `/v1/zk/prover/reports/${encodeURIComponent(normalizedId)}`,
      { signal },
    );
    await this._expectStatus(response, [204, 404]);
  }

  /**
   * Submit an ISO 20022 pacs.008 message (`POST /v1/iso20022/pacs008`).
   * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message XML payload
   * @param {{contentType?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async submitIsoPacs008(message, options = {}) {
    const { signal, contentType, retryProfile } = normalizeIsoSubmissionOptions(
      options,
      "submitIsoPacs008",
    );
    const body = normalizeIsoPayload(message, "submitIsoPacs008.message");
    const response = await this._request("POST", "/v1/iso20022/pacs008", {
      headers: {
        "Content-Type": contentType ?? "application/xml",
        Accept: "application/json",
      },
      body,
      signal,
      retryProfile,
    });
    await this._expectStatus(response, [202]);
    const payload = await this._maybeJson(response);
    return payload == null
      ? null
      : normalizeIsoSubmissionResponse(payload, "ISO pacs008 submission");
  }

  /**
   * Submit an ISO 20022 pacs.009 message (`POST /v1/iso20022/pacs009`).
   * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message XML payload
   * @param {{contentType?: string, signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async submitIsoPacs009(message, options = {}) {
    const { signal, contentType, retryProfile } = normalizeIsoSubmissionOptions(
      options,
      "submitIsoPacs009",
    );
    const body = normalizeIsoPayload(message, "submitIsoPacs009.message");
    const response = await this._request("POST", "/v1/iso20022/pacs009", {
      headers: {
        "Content-Type": contentType ?? "application/xml",
        Accept: "application/json",
      },
      body,
      signal,
      retryProfile,
    });
    await this._expectStatus(response, [202]);
    const payload = await this._maybeJson(response);
    return payload == null
      ? null
      : normalizeIsoSubmissionResponse(payload, "ISO pacs009 submission");
  }

  /**
   * Fetch ISO 20022 message status (`GET /v1/iso20022/status/{msg_id}`).
   * @param {string} messageId
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async getIsoMessageStatus(messageId, options = {}) {
    const normalizedId = requireNonEmptyString(messageId, "messageId");
    const { signal, retryProfile } = normalizeIsoStatusOptions(options, "getIsoMessageStatus");
    const response = await this._request(
      "GET",
      `/v1/iso20022/status/${encodeURIComponent(normalizedId)}`,
      { headers: { Accept: "application/json" }, signal, retryProfile },
    );
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload) {
      return null;
    }
    return normalizeIsoStatusResponse(payload, "ISO status response");
  }

  /**
   * Wait for an ISO bridge message to reach a terminal state.
   * @param {string} messageId
   * @param {IsoMessageWaitOptions} [options]
   * @returns {Promise<Record<string, unknown>>}
   */
  async waitForIsoMessageStatus(messageId, options = {}) {
    const normalizedId = requireNonEmptyString(messageId, "messageId");
    const optionPath = "waitForIsoMessageStatus.options";
    const resolvedOptions =
      options === undefined || options === null
        ? {}
        : requireIsoPlainObject(options, optionPath);
    const allowedKeys = new Set([
      "pollIntervalMs",
      "maxAttempts",
      "resolveOnAcceptedWithoutTransaction",
      "resolveOnAccepted",
      "onPoll",
      "signal",
      "retryProfile",
    ]);
    const unsupportedKeys = Object.keys(resolvedOptions).filter((key) => !allowedKeys.has(key));
    if (unsupportedKeys.length > 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${optionPath} contains unsupported fields: ${unsupportedKeys.join(", ")}`,
        optionPath,
      );
    }
    const pollIntervalMs =
      resolvedOptions.pollIntervalMs === undefined
        ? DEFAULT_ISO_POLL_INTERVAL_MS
        : ToriiClient._normalizeUnsignedInteger(
            resolvedOptions.pollIntervalMs,
            "wait.pollIntervalMs",
            {
              allowZero: true,
              min: MIN_ISO_POLL_INTERVAL_MS,
            },
          );
    const maxAttempts =
      resolvedOptions.maxAttempts === undefined
        ? DEFAULT_ISO_POLL_ATTEMPTS
        : ToriiClient._normalizeUnsignedInteger(resolvedOptions.maxAttempts, "wait.maxAttempts", {
            allowZero: false,
          });
    const resolveAlias = resolvedOptions.resolveOnAccepted;
    const resolveCanonical = resolvedOptions.resolveOnAcceptedWithoutTransaction;
    if (resolveCanonical !== undefined && typeof resolveCanonical !== "boolean") {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "wait.resolveOnAcceptedWithoutTransaction must be a boolean",
        `${optionPath}.resolveOnAcceptedWithoutTransaction`,
      );
    }
    if (resolveAlias !== undefined && typeof resolveAlias !== "boolean") {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "wait.resolveOnAccepted must be a boolean",
        `${optionPath}.resolveOnAccepted`,
      );
    }
    if (
      resolveCanonical !== undefined &&
      resolveAlias !== undefined &&
      resolveCanonical !== resolveAlias
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "wait.resolveOnAccepted and wait.resolveOnAcceptedWithoutTransaction must match when both are provided",
        `${optionPath}.resolveOnAccepted`,
      );
    }
    const resolveOnAccepted =
      resolveCanonical !== undefined ? resolveCanonical : resolveAlias ?? false;
    let retryProfile;
    if (resolvedOptions.retryProfile !== undefined && resolvedOptions.retryProfile !== null) {
      retryProfile = requireNonEmptyString(
        resolvedOptions.retryProfile,
        `${optionPath}.retryProfile`,
      );
    }
    const onPoll =
      resolvedOptions.onPoll === undefined || resolvedOptions.onPoll === null
        ? undefined
        : resolvedOptions.onPoll;
    if (onPoll !== undefined && typeof onPoll !== "function") {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "wait.onPoll must be a function",
        `${optionPath}.onPoll`,
      );
    }
    let signal;
    try {
      ({ signal } = normalizeSignalOption(
        resolvedOptions,
        "waitForIsoMessageStatus",
      ));
    } catch (error) {
      if (error instanceof TypeError) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          error.message,
          `${optionPath}.signal`,
          error,
        );
      }
      throw error;
    }
    let lastStatus = null;
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      throwIfAborted(signal);
      const status = await this.getIsoMessageStatus(normalizedId, { signal, retryProfile });
      lastStatus = status ?? null;
      if (onPoll) {
        // Allow async callbacks for telemetry/logging hooks.
        // eslint-disable-next-line no-await-in-loop
        await onPoll({ attempt, status: lastStatus });
      }
      if (ToriiClient._isIsoStatusTerminal(lastStatus, resolveOnAccepted)) {
        return lastStatus;
      }
      if (attempt < maxAttempts) {
        throwIfAborted(signal);
        // eslint-disable-next-line no-await-in-loop
        await delay(pollIntervalMs);
      }
    }
    throw new IsoMessageTimeoutError(normalizedId, maxAttempts, lastStatus);
  }

  /**
   * Submit an ISO 20022 pacs.008 message and wait for a terminal status.
   * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
   * @param {{contentType?: string, signal?: AbortSignal, wait?: IsoMessageWaitOptions}} [options]
   * @returns {Promise<Record<string, unknown>>}
   */
  async submitIsoPacs008AndWait(message, options = {}) {
    const record =
      options === undefined || options === null
        ? {}
        : requireIsoPlainObject(options, "submitIsoPacs008AndWait.options");
    const { wait: waitOptions, ...submitOptions } = record;
    let normalizedWait =
      waitOptions === undefined || waitOptions === null
        ? undefined
        : requireIsoPlainObject(waitOptions, "submitIsoPacs008AndWait.options.wait");
    if (normalizedWait) {
      const needsSignal = normalizedWait.signal === undefined && submitOptions.signal !== undefined;
      const needsRetryProfile =
        normalizedWait.retryProfile === undefined && submitOptions.retryProfile !== undefined;
      if (needsSignal || needsRetryProfile) {
        normalizedWait = { ...normalizedWait };
        if (needsSignal) {
          normalizedWait.signal = submitOptions.signal;
        }
        if (needsRetryProfile) {
          normalizedWait.retryProfile = submitOptions.retryProfile;
        }
      }
    }
    const submission = await this.submitIsoPacs008(message, submitOptions);
    const messageId = submission?.message_id;
    if (!messageId) {
      throw new Error("ISO pacs.008 submission did not return a message_id");
    }
    return this.waitForIsoMessageStatus(messageId, normalizedWait);
  }

  /**
   * Submit an ISO 20022 pacs.009 message and wait for a terminal status.
   * @param {ArrayBufferView | ArrayBuffer | Buffer | string} message
   * @param {{contentType?: string, signal?: AbortSignal, wait?: IsoMessageWaitOptions}} [options]
   * @returns {Promise<Record<string, unknown>>}
   */
  async submitIsoPacs009AndWait(message, options = {}) {
    const record =
      options === undefined || options === null
        ? {}
        : requireIsoPlainObject(options, "submitIsoPacs009AndWait.options");
    const { wait: waitOptions, ...submitOptions } = record;
    let normalizedWait =
      waitOptions === undefined || waitOptions === null
        ? undefined
        : requireIsoPlainObject(waitOptions, "submitIsoPacs009AndWait.options.wait");
    if (normalizedWait) {
      const needsSignal = normalizedWait.signal === undefined && submitOptions.signal !== undefined;
      const needsRetryProfile =
        normalizedWait.retryProfile === undefined && submitOptions.retryProfile !== undefined;
      if (needsSignal || needsRetryProfile) {
        normalizedWait = { ...normalizedWait };
        if (needsSignal) {
          normalizedWait.signal = submitOptions.signal;
        }
        if (needsRetryProfile) {
          normalizedWait.retryProfile = submitOptions.retryProfile;
        }
      }
    }
    const submission = await this.submitIsoPacs009(message, submitOptions);
    const messageId = submission?.message_id;
    if (!messageId) {
      throw new Error("ISO pacs.009 submission did not return a message_id");
    }
    return this.waitForIsoMessageStatus(messageId, normalizedWait);
  }

  /**
   * Build and submit an ISO 20022 message from structured fields.
   * @param {import("./index").BuildPacs008Options | import("./index").BuildPacs009Options} fields
   * @param {{kind?: string, contentType?: string, signal?: AbortSignal, wait?: IsoMessageWaitOptions}} [options]
   * @returns {Promise<Record<string, unknown> | null>}
   */
  async submitIsoMessage(fields, options = {}) {
    const resolvedOptions =
      options === undefined || options === null
        ? {}
        : requireIsoPlainObject(options, "submitIsoMessage.options");
    const normalizedKind =
      resolvedOptions.kind === undefined || resolvedOptions.kind === null
        ? null
        : normalizeIsoMessageKind(
            resolvedOptions.kind,
            "submitIsoMessage options.kind",
          );
    const normalizedMessageKind =
      resolvedOptions.messageKind === undefined || resolvedOptions.messageKind === null
        ? null
        : normalizeIsoMessageKind(
            resolvedOptions.messageKind,
            "submitIsoMessage options.messageKind",
          );
    if (
      normalizedKind !== null &&
      normalizedMessageKind !== null &&
      normalizedKind !== normalizedMessageKind
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "submitIsoMessage options.kind and options.messageKind must match when both are provided",
        "submitIsoMessage.options.kind",
      );
    }
    const kind = normalizedKind ?? normalizedMessageKind ?? "pacs.008";
    const { signal, contentType, retryProfile } = normalizeIsoSubmissionOptions(
      resolvedOptions,
      "submitIsoMessage",
      ["kind", "messageKind", "wait", "retryProfile"],
    );
    const normalizedFields =
      fields === undefined || fields === null ? {} : { ...fields };
    if (
      normalizedFields.creationDateTime === undefined ||
      normalizedFields.creationDateTime === null
    ) {
      normalizedFields.creationDateTime = new Date().toISOString();
    }
    const xml =
      kind === "pacs.009"
        ? buildPacs009Message(normalizedFields)
        : buildPacs008Message(normalizedFields);
    const submissionOptions = {
      signal,
      contentType:
        contentType ??
        (kind === "pacs.009" ? "application/pacs009+xml" : "application/pacs008+xml"),
      retryProfile,
    };
    let waitOptions = resolvedOptions.wait;
    if (waitOptions !== undefined && waitOptions !== null) {
      waitOptions = requireIsoPlainObject(waitOptions, "submitIsoMessage.options.wait");
      const needsSignal = waitOptions.signal === undefined && submissionOptions.signal;
      const needsRetryProfile =
        waitOptions.retryProfile === undefined && submissionOptions.retryProfile;
      if (needsSignal || needsRetryProfile) {
        waitOptions = { ...waitOptions };
        if (needsSignal) {
          waitOptions.signal = submissionOptions.signal;
        }
        if (needsRetryProfile) {
          waitOptions.retryProfile = submissionOptions.retryProfile;
        }
      }
      return kind === "pacs.009"
        ? this.submitIsoPacs009AndWait(xml, { ...submissionOptions, wait: waitOptions })
        : this.submitIsoPacs008AndWait(xml, { ...submissionOptions, wait: waitOptions });
    }
    return kind === "pacs.009"
      ? this.submitIsoPacs009(xml, submissionOptions)
      : this.submitIsoPacs008(xml, submissionOptions);
  }

  /**
   * Count prover reports with optional filters.
   * @param {Record<string, unknown>} filters
   * @param {{signal?: AbortSignal}} [options]
   * @returns {Promise<number>}
   */
  async countProverReports(filters = {}, options = {}) {
    const normalizedFilters = ToriiClient._encodeProverFilters(filters);
    const { signal } = normalizeSignalOnlyOption(options, "countProverReports");
    const response = await this._request("GET", "/v1/zk/prover/reports/count", {
      params: normalizedFilters,
      signal,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    if (!payload || typeof payload.count === "undefined") {
      throw new Error("invalid prover count payload");
    }
    return ToriiClient._normalizeUnsignedInteger(payload.count, "prover count", {
      allowZero: true,
    });
  }

  async _request(method, path, options = {}) {
    const pathIsAbsolute = isAbsoluteUrl(path);
    const url = pathIsAbsolute ? new URL(path) : new URL(path, this._baseUrl + "/");
    const protocol = url.protocol.toLowerCase();
    const originMatches =
      url.host === this._baseHost && protocol === this._baseProtocol;
    const initHeaders = this._createHeaders(options.headers);
    const hasCredentials = headersContainCredentials(initHeaders);
    const canonicalAuth = options.canonicalAuth
      ? ToriiClient._normalizeCanonicalAuth(options.canonicalAuth)
      : null;
    const hasCanonicalAuth = canonicalAuth !== null;
    const hasSensitiveBody = bodyContainsSensitiveKeyMaterial(options.body, initHeaders);
    const hasSensitiveTransport = hasCredentials || hasCanonicalAuth || hasSensitiveBody;
    const allowAbsoluteUrl = options.allowAbsoluteUrl === true;
    const methodUpper = String(method).toUpperCase();
    if (hasSensitiveTransport) {
      if (protocol !== this._baseProtocol) {
        throw new Error(
          `ToriiClient: refusing to send sensitive request material over mismatched scheme ${url.protocol}; use ${this._baseProtocol.replace(":", "")} URLs derived from the client base URL.`,
        );
      }
      if (pathIsAbsolute && url.host !== this._baseHost) {
        throw new Error(
          `ToriiClient: refusing to send sensitive request material to mismatched host ${url.host} (expected ${this._baseHost}); use relative paths on the configured base URL.`,
        );
      }
      if (!this._allowInsecure && !isSecureProtocol(protocol)) {
        throw new Error(
          `ToriiClient: refusing to send sensitive request material over insecure protocol ${url.protocol}; use https or allowInsecure: true.`,
        );
      }
    } else if (pathIsAbsolute && !originMatches && !allowAbsoluteUrl) {
      throw new Error(
        "ToriiClient: absolute URLs are blocked when no credentials are attached; pass allowAbsoluteUrl: true to override.",
      );
    }
    if (hasSensitiveTransport && this._allowInsecure && !isSecureProtocol(protocol)) {
      this._emitInsecureTransportTelemetry({
        client: "torii",
        method: methodUpper,
        hasCredentials,
        hasSensitiveBody,
        hasCanonicalAuth,
        allowInsecure: true,
        url: url.toString(),
        baseUrl: this._baseUrl,
        host: url.host,
        protocol,
        pathIsAbsolute,
        originMatches,
      });
    }
    if (options.params && Object.keys(options.params).length > 0) {
      const search = new URLSearchParams();
      for (const [key, value] of Object.entries(options.params)) {
        if (Array.isArray(value)) {
          for (const item of value) {
            search.append(key, String(item));
          }
        } else if (typeof value !== "undefined") {
          search.append(key, String(value));
        }
      }
      url.search = search.toString();
    }
    const init = {
      method: methodUpper,
      headers: initHeaders,
      body: options.body,
    };
    if (canonicalAuth) {
      const bodyForSigning =
        init.body === undefined || init.body === null
          ? Buffer.alloc(0)
          : Buffer.isBuffer(init.body)
            ? init.body
            : Buffer.from(init.body);
      const canonicalHeaders = buildCanonicalRequestHeaders({
        accountId: canonicalAuth.accountId,
        method: methodUpper,
        path: url.pathname,
        query: url.search.startsWith("?") ? url.search.slice(1) : url.search,
        body: bodyForSigning,
        privateKey: canonicalAuth.privateKey,
      });
      for (const [key, value] of Object.entries(canonicalHeaders)) {
        setHeader(initHeaders, key, value);
      }
      if (headerValueRequiresRawUtf8Transport(canonicalHeaders["X-Iroha-Account"])) {
        init[RAW_UTF8_HEADERS_INIT_KEY] = {
          "X-Iroha-Account": canonicalHeaders["X-Iroha-Account"],
        };
      }
    }
    const rawUtf8Headers = cloneRawUtf8Headers(init[RAW_UTF8_HEADERS_INIT_KEY]);
    const shouldUseNodeRawUtf8Transport =
      rawUtf8Headers &&
      !fetchSupportsRawUtf8Headers(this._fetch) &&
      canUseNodeRawUtf8Transport(url);
    if (
      rawUtf8Headers &&
      !fetchSupportsRawUtf8Headers(this._fetch) &&
      !shouldUseNodeRawUtf8Transport
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "ToriiClient: canonicalAuth.accountId contains UTF-8 characters that require a fetch implementation with raw UTF-8 header support.",
        "canonicalAuth.accountId",
      );
    }
    const retryProfileName =
      typeof options.retryProfile === "string" && options.retryProfile
        ? options.retryProfile
        : "default";
    const retryPolicy =
      (this._config.retryProfiles && this._config.retryProfiles[retryProfileName]) ||
      this._config.retryProfiles?.default ||
      null;
    const policyMaxRetries =
      retryPolicy && typeof retryPolicy.maxRetries === "number"
        ? retryPolicy.maxRetries
        : this._config.maxRetries;
    const policyBackoffInitial =
      retryPolicy && typeof retryPolicy.backoffInitialMs === "number"
        ? retryPolicy.backoffInitialMs
        : this._config.backoffInitialMs;
    const policyBackoffMultiplier =
      retryPolicy && typeof retryPolicy.backoffMultiplier === "number"
        ? retryPolicy.backoffMultiplier
        : this._config.backoffMultiplier;
    const policyMaxBackoffMs =
      retryPolicy && typeof retryPolicy.maxBackoffMs === "number"
        ? retryPolicy.maxBackoffMs
        : this._config.maxBackoffMs;
    const maxRetries = Math.max(0, Number(policyMaxRetries) || 0);
    let attempt = 0;
    let backoffMs = Math.max(0, policyBackoffInitial || 0);
    let lastError;

    // eslint-disable-next-line no-constant-condition
    while (true) {
      attempt += 1;
      const attemptStartedAt = Date.now();
      const callerSignal = options.signal ?? null;
      const shouldInstallTimeoutController =
        !callerSignal &&
        typeof AbortController === "function" &&
        this._config.timeoutMs > 0;
      const controller = shouldInstallTimeoutController ? new AbortController() : null;
      let timeoutId;
      let timedOut = false;
      const signal = callerSignal ?? controller?.signal ?? null;
      if (controller) {
        timeoutId = setTimeout(() => {
          timedOut = true;
          controller.abort();
        }, this._config.timeoutMs);
      }
      try {
        const response = shouldUseNodeRawUtf8Transport
          ? await performNodeRawUtf8Request({
              url,
              method: methodUpper,
              headers: cloneHeadersForFetch(initHeaders),
              rawUtf8Headers,
              body: init.body,
              signal: signal ?? undefined,
            })
          : await this._fetch(url.toString(), {
              ...init,
              // Give fetch a fresh header bag for each retry attempt. Reusing the
              // same object across retries can break native fetch implementations.
              headers: cloneHeadersForFetch(initHeaders),
              [RAW_UTF8_HEADERS_INIT_KEY]: cloneRawUtf8Headers(
                init[RAW_UTF8_HEADERS_INIT_KEY],
              ),
              signal: signal ?? undefined,
            });
        if (timeoutId) {
          clearTimeout(timeoutId);
        }
        if (
          !this._shouldRetryResponse(methodUpper, response.status, retryPolicy) ||
          attempt > maxRetries
        ) {
          return response;
        }
        const durationMs = Math.max(0, Date.now() - attemptStartedAt);
        lastError = new Error(`retryable status ${response.status}`);
        this._emitRetryTelemetry({
          phase: "response",
          attempt,
          nextAttempt: attempt + 1,
          maxRetries,
          method: methodUpper,
          url: url.toString(),
          status: response.status,
          backoffMs,
          profile: retryProfileName,
          durationMs,
        });
      } catch (error) {
        if (timeoutId) {
          clearTimeout(timeoutId);
        }
        if (options.signal && options.signal.aborted) {
          throw error;
        }
        if (
          !this._shouldRetryError(error, methodUpper, timedOut, retryPolicy) ||
          attempt > maxRetries
        ) {
          throw error;
        }
        const durationMs = Math.max(0, Date.now() - attemptStartedAt);
        lastError = error;
        this._emitRetryTelemetry({
          phase: timedOut ? "timeout" : "network",
          attempt,
          nextAttempt: attempt + 1,
          maxRetries,
          method: methodUpper,
          url: url.toString(),
          errorName: error?.name ?? null,
          errorMessage: error?.message ?? null,
          timedOut,
          backoffMs,
          profile: retryProfileName,
          durationMs,
        });
      }

      if (attempt > maxRetries) {
        throw lastError ?? new Error("request failed after maximum retries");
      }

      if (backoffMs > 0) {
        await delay(backoffMs);
        const multiplier = policyBackoffMultiplier || 1;
        backoffMs = Math.min(
          Math.max(backoffMs * multiplier, policyBackoffInitial || 0),
          policyMaxBackoffMs || backoffMs,
        );
      }
    }
  }

  _emitRetryTelemetry(event) {
    const hook = this._config.retryTelemetryHook;
    if (typeof hook !== "function") {
      return;
    }
    const payload = {
      ...event,
      timestampMs: Date.now(),
    };
    try {
      hook(payload);
    } catch {
      // No-op: telemetry hooks must never break retries.
    }
  }

  _emitInsecureTransportTelemetry(event) {
    const hook = this._config.insecureTransportTelemetryHook;
    if (typeof hook !== "function") {
      return;
    }
    try {
      hook({ ...event, timestampMs: Date.now() });
    } catch {
      // No-op: telemetry hooks must never break requests.
    }
  }

  _resolveSorafsPolicy() {
    if (this._sorafsResolvedPolicy) {
      return this._sorafsResolvedPolicy;
    }
    const native = requireSorafsNativeBinding();
    const defaults = native.sorafsAliasPolicyDefaults();
    const policy = { ...defaults };
    const overrides = this._sorafsPolicyOverrides;
    if (overrides && typeof overrides === "object") {
      const positive = pickOverride(overrides, "positive_ttl_secs", "positiveTtlSecs");
      if (positive !== undefined && positive !== null) {
        policy.positive_ttl_secs = ToriiClient._normalizeUnsignedInteger(
          positive,
          "sorafsAliasPolicy.positiveTtlSecs",
          { allowZero: false },
        );
      }
      const refresh = pickOverride(overrides, "refresh_window_secs", "refreshWindowSecs");
      if (refresh !== undefined && refresh !== null) {
        policy.refresh_window_secs = ToriiClient._normalizeUnsignedInteger(
          refresh,
          "sorafsAliasPolicy.refreshWindowSecs",
          { allowZero: false },
        );
      }
      const hard = pickOverride(overrides, "hard_expiry_secs", "hardExpirySecs");
      if (hard !== undefined && hard !== null) {
        policy.hard_expiry_secs = ToriiClient._normalizeUnsignedInteger(
          hard,
          "sorafsAliasPolicy.hardExpirySecs",
          { allowZero: false },
        );
      }
      const negative = pickOverride(overrides, "negative_ttl_secs", "negativeTtlSecs");
      if (negative !== undefined && negative !== null) {
        policy.negative_ttl_secs = ToriiClient._normalizeUnsignedInteger(
          negative,
          "sorafsAliasPolicy.negativeTtlSecs",
          { allowZero: false },
        );
      }
      const revocation = pickOverride(overrides, "revocation_ttl_secs", "revocationTtlSecs");
      if (revocation !== undefined && revocation !== null) {
        policy.revocation_ttl_secs = ToriiClient._normalizeUnsignedInteger(
          revocation,
          "sorafsAliasPolicy.revocationTtlSecs",
          { allowZero: false },
        );
      }
      const rotation = pickOverride(overrides, "rotation_max_age_secs", "rotationMaxAgeSecs");
      if (rotation !== undefined && rotation !== null) {
        policy.rotation_max_age_secs = ToriiClient._normalizeUnsignedInteger(
          rotation,
          "sorafsAliasPolicy.rotationMaxAgeSecs",
          { allowZero: false },
        );
      }
    }

    if (policy.refresh_window_secs > policy.positive_ttl_secs) {
      throw new Error(
        "sorafsAliasPolicy.refreshWindowSecs must not exceed positiveTtlSecs",
      );
    }
    if (policy.hard_expiry_secs < policy.positive_ttl_secs) {
      throw new Error(
        "sorafsAliasPolicy.hardExpirySecs must be greater than or equal to positiveTtlSecs",
      );
    }

    this._sorafsResolvedPolicy = policy;
    return policy;
  }

  _enforceSorafsAliasPolicy(response) {
    if (!response || response.status !== 200) {
      return;
    }
    const proofB64 = this._getHeader(response, HEADER_SORA_PROOF);
    if (!proofB64) {
      return;
    }
    const native = requireSorafsNativeBinding();
    const policy = this._resolveSorafsPolicy();

    let evaluation;
    try {
      evaluation = native.sorafsEvaluateAliasProof(proofB64, policy);
    } catch (error) {
      const message =
        error && typeof error.message === "string" ? error.message : String(error);
      throw new Error(`failed to validate SoraFS alias proof: ${message}`);
    }

    if (!evaluation || evaluation.servable !== true) {
      const aliasLabel = this._getHeader(response, HEADER_SORA_NAME) ?? "<unknown>";
      const statusHeader = this._getHeader(response, HEADER_SORA_PROOF_STATUS);
      const statusHint = statusHeader ? `; header reported ${statusHeader}` : "";
      const statusLabel =
        evaluation && typeof evaluation.status_label === "string"
          ? evaluation.status_label
          : "unknown";
      const ageSeconds =
        evaluation && typeof evaluation.age_seconds === "number"
          ? evaluation.age_seconds
          : NaN;
      throw new Error(
        `alias proof for '${aliasLabel}' rejected: state ${statusLabel}${statusHint} (age ${ageSeconds} seconds)`,
      );
    }

    if (
      (evaluation.state === "refresh_window" || evaluation.rotation_due === true) &&
      typeof this._sorafsAliasWarningHook === "function"
    ) {
      this._sorafsAliasWarningHook({
        alias: this._getHeader(response, HEADER_SORA_NAME) ?? null,
        evaluation: formatSorafsEvaluation(evaluation),
      });
    }
  }

  _createHeaders(provided = {}) {
    const headers = {};
    const applyEntries = (source) => {
      if (!source) {
        return;
      }
      if (typeof Headers === "function" && source instanceof Headers) {
        source.forEach((value, key) => {
          setHeader(headers, key, value);
        });
        return;
      }
      if (
        typeof source[Symbol.iterator] === "function" &&
        !isPlainObject(source)
      ) {
        for (const entry of source) {
          if (!entry) {
            continue;
          }
          const [key, value] = entry;
          if (key === undefined) {
            continue;
          }
          if (value === null) {
            deleteHeader(headers, key);
          } else if (value !== undefined) {
            setHeader(headers, key, value);
          }
        }
        return;
      }
      if (typeof source === "object") {
        for (const [key, value] of Object.entries(source)) {
          if (value === null) {
            deleteHeader(headers, key);
            continue;
          }
          if (value !== undefined) {
            setHeader(headers, key, value);
          }
        }
      }
    };
    applyEntries(this._config.defaultHeaders);
    applyEntries(provided);
    if (this._config.apiToken) {
      if (!hasHeader(headers, "x-api-token")) {
        headers["X-API-Token"] = this._config.apiToken;
      }
    }
    if (this._config.authToken && !hasHeader(headers, "authorization")) {
      headers.Authorization = `Bearer ${this._config.authToken}`;
    }
    attachHeaderAccessors(headers);
    return headers;
  }

  _hasClientCredentials() {
    return (
      Boolean(this._config?.authToken) ||
      Boolean(this._config?.apiToken) ||
      headersContainCredentials(this._config?.defaultHeaders)
    );
  }

  _assertPermissionRequirement(requirePermissions, contextPath) {
    if (!requirePermissions) {
      return;
    }
    if (this._hasClientCredentials()) {
      return;
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${contextPath} requires authToken or apiToken when requirePermissions is true`,
      `${contextPath}.requirePermissions`,
    );
  }

  _shouldRetryResponse(method, status, policy) {
    const activePolicy =
      policy ||
      this._config.retryProfiles?.default || {
        retryMethods: this._config.retryMethods,
        retryStatuses: this._config.retryStatuses,
      };
    const methods = activePolicy.retryMethods ?? this._config.retryMethods;
    const statuses = activePolicy.retryStatuses ?? this._config.retryStatuses;
    return methods.has(method.toUpperCase()) && statuses.has(Number(status));
  }

  _shouldRetryError(error, method, timedOut, policy) {
    const activePolicy =
      policy ||
      this._config.retryProfiles?.default || {
        retryMethods: this._config.retryMethods,
      };
    const methods = activePolicy.retryMethods ?? this._config.retryMethods;
    if (!methods.has(method.toUpperCase())) {
      return false;
    }
    if (timedOut) {
      return true;
    }
    if (this._isAbortError(error)) {
      return timedOut;
    }
    if (error && (error.name === "TypeError" || error.code === "ECONNRESET")) {
      return true;
    }
    return false;
  }

  _isAbortError(error) {
    if (!error) {
      return false;
    }
    if (typeof DOMException !== "undefined" && error instanceof DOMException) {
      return error.name === "AbortError";
    }
    return error.name === "AbortError";
  }

  async _expectStatus(response, expected) {
    if (expected.includes(response.status)) {
      return;
    }
    throw await this._buildHttpError(response, expected);
  }

  async _buildHttpError(response, expected) {
    const { bodyText, bodyJson } = await this._readErrorBody(response);
    const rejectCode = this._extractRejectCode(response);
    const code =
      rejectCode ??
      this._extractErrorCode(bodyJson) ??
      this._extractCodeFromText(bodyText) ??
      null;
    const errorMessage = this._extractErrorMessage(bodyJson, bodyText);
    return new ToriiHttpError({
      status: response.status,
      statusText: response.statusText ?? null,
      expected,
      code,
      rejectCode,
      errorMessage,
      bodyText,
      bodyJson,
    });
  }

  async _readErrorBody(response) {
    const contentType = this._getHeader(response, "content-type");
    const looksLikeJson =
      typeof contentType === "string" &&
      contentType.toLowerCase().includes("application/json");
    if (looksLikeJson && typeof response.json === "function") {
      try {
        const bodyJson = await response.json();
        const bodyText =
          bodyJson == null
            ? null
            : typeof bodyJson === "string"
              ? bodyJson
              : JSON.stringify(bodyJson);
        return { bodyText, bodyJson };
      } catch {
        // fall through to text parsing
      }
    }
    if (typeof response.text !== "function") {
      return { bodyText: null, bodyJson: null };
    }
    let text = null;
    try {
      text = await response.text();
    } catch {
      text = null;
    }
    if (!text) {
      return { bodyText: null, bodyJson: null };
    }
    const trimmed = text.trim();
    if (!trimmed) {
      return { bodyText: "", bodyJson: null };
    }
    try {
      return { bodyText: text, bodyJson: JSON.parse(trimmed) };
    } catch {
      return { bodyText: text, bodyJson: null };
    }
  }

  _extractErrorCode(payload) {
    if (!payload || typeof payload !== "object") {
      return null;
    }
    if (typeof payload.code === "string" && payload.code) {
      return payload.code;
    }
    if (typeof payload.reason === "string" && payload.reason) {
      return payload.reason;
    }
    if (typeof payload.error === "string" && payload.error.startsWith("ERR_")) {
      return payload.error;
    }
    return null;
  }

  _extractCodeFromText(text) {
    if (typeof text !== "string" || !text) {
      return null;
    }
    const match = text.match(/ERR_[A-Z0-9_]+/u);
    return match ? match[0] : null;
  }

  _trimErrorBodyText(text, maxLength = 512) {
    if (typeof text !== "string") {
      return null;
    }
    const trimmed = text.trim();
    if (!trimmed) {
      return null;
    }
    if (trimmed.length <= maxLength) {
      return trimmed;
    }
    return `${trimmed.slice(0, maxLength)}...`;
  }

  _extractErrorMessageValue(value) {
    if (typeof value === "string") {
      return this._trimErrorBodyText(value);
    }
    if (Array.isArray(value)) {
      for (const item of value) {
        const nested = this._extractErrorMessageValue(item);
        if (nested) {
          return nested;
        }
      }
      return null;
    }
    if (!value || typeof value !== "object") {
      return null;
    }
    const candidateKeys = [
      "message",
      "error",
      "errors",
      "detail",
      "details",
      "reason",
      "rejection_reason",
      "description",
    ];
    const caseInsensitiveValues = new Map();
    for (const [key, entryValue] of Object.entries(value)) {
      const normalizedKey = String(key).toLowerCase();
      if (!caseInsensitiveValues.has(normalizedKey)) {
        caseInsensitiveValues.set(normalizedKey, entryValue);
      }
    }
    for (const key of candidateKeys) {
      if (!caseInsensitiveValues.has(key)) {
        continue;
      }
      const nested = this._extractErrorMessageValue(caseInsensitiveValues.get(key));
      if (nested) {
        return nested;
      }
    }
    return null;
  }

  _compactErrorJson(value) {
    if (value === null || value === undefined) {
      return null;
    }
    try {
      return this._trimErrorBodyText(JSON.stringify(sortJsonForErrorMessage(value)));
    } catch {
      return null;
    }
  }

  _extractErrorMessage(payload, fallbackText) {
    const nested = this._extractErrorMessageValue(payload);
    if (nested) {
      return nested;
    }
    const compact = this._compactErrorJson(payload);
    if (compact) {
      return compact;
    }
    if (typeof fallbackText === "string" && fallbackText.trim()) {
      return this._trimErrorBodyText(fallbackText);
    }
    return null;
  }

  _extractRejectCode(response) {
    const raw = this._getHeader(response, "x-iroha-reject-code");
    if (typeof raw !== "string") {
      return null;
    }
    const trimmed = raw.trim();
    return trimmed ? trimmed : null;
  }

  static _encodeProverFilters(filters) {
    if (filters === undefined || filters === null) {
      return {};
    }
    const record = ToriiClient._requirePlainObject(filters, "prover filters");
    const params = {};
    for (const [rawKey, rawValue] of Object.entries(record)) {
      if (rawValue === undefined || rawValue === null) {
        continue;
      }
      const entry = PROVER_FILTER_ALIAS_MAP.get(rawKey);
      if (!entry) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `unknown prover filter '${rawKey}'`,
          `proverFilters.${rawKey}`,
        );
      }
      const { key, spec } = entry;
      if (spec.type === "boolean") {
        const flag = normalizeProverFilterBoolean(rawValue, `proverFilters.${rawKey}`);
        if (flag) {
          params[key] = true;
        }
        continue;
      }
      if (spec.type === "string") {
        params[key] = normalizeProverFilterString(rawValue, `proverFilters.${rawKey}`);
        continue;
      }
      if (spec.type === "integer") {
        params[key] = ToriiClient._normalizeUnsignedInteger(
          rawValue,
          `proverFilters.${rawKey}`,
          { allowZero: Boolean(spec.allowZero) },
        );
        continue;
      }
      if (spec.type === "enum") {
        params[key] = normalizeProverFilterEnum(
          rawValue,
          `proverFilters.${rawKey}`,
          spec.values,
        );
        continue;
      }
    }
    return params;
  }

  /**
   * @template [T=unknown]
   * @param {string} path
   * @param {EventStreamOptions & { params?: Record<string, unknown> }} [options]
   * @returns {AsyncGenerator<SseEvent<T>, void, unknown>}
   */
  _streamSse(path, options = {}) {
    const { signal } = normalizeSignalOption(options, "_streamSse");
    const params = options.params;
    const headers = this._createHeaders({ Accept: "text/event-stream" });
    if (options.lastEventId) {
      headers["Last-Event-ID"] = options.lastEventId;
    }
    const requestOptions = {
      params,
      headers,
      signal,
      retryProfile: "streaming",
    };
    const self = this;
    return (async function* iterator() {
      const response = await self._request("GET", path, requestOptions);
      await self._expectStatus(response, [200]);
      const decoder = new TextDecoder();
      let buffer = "";
      for await (const chunk of readBodyChunks(response.body)) {
        buffer += decoder.decode(chunk, { stream: true });
        const { events, remainder } = flushSseBuffer(buffer);
        buffer = remainder;
        for (const event of events) {
          yield event;
        }
      }
      if (buffer.length > 0) {
        const { events } = flushSseBuffer(`${buffer}\n\n`);
        for (const event of events) {
          yield event;
        }
      }
    })();
  }

  static _normalizeEventFilter(filter) {
    if (filter === undefined || filter === null) {
      return undefined;
    }
    if (typeof filter === "string") {
      return filter;
    }
    if (typeof filter === "object") {
      try {
        return JSON.stringify(filter);
      } catch (error) {
        throw createValidationError(
          ValidationErrorCode.INVALID_JSON_VALUE,
          `failed to serialise filter: ${
            error instanceof Error ? error.message : String(error)
          }`,
          "eventFilter",
          error instanceof Error ? error : undefined,
        );
      }
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "filter must be a string or plain object",
      "eventFilter",
    );
  }

  /**
   * @template [T=unknown]
   * @param {string} raw
   * @returns {SseEvent<T> | null}
   */
  static _parseSseEvent(raw) {
    const lines = raw.split(/\r?\n/);
    let event = null;
    let id = null;
    let retry = null;
    const dataLines = [];
    for (const line of lines) {
      if (!line || line.startsWith(":")) {
        continue;
      }
      const colonIndex = line.indexOf(":");
      const field = colonIndex === -1 ? line : line.slice(0, colonIndex);
      let value = colonIndex === -1 ? "" : line.slice(colonIndex + 1);
      if (value.startsWith(" ")) {
        value = value.slice(1);
      }
      switch (field) {
        case "event":
          event = value || null;
          break;
        case "data":
          dataLines.push(value);
          break;
        case "id":
          id = value || null;
          break;
        case "retry": {
          const parsed = Number(value);
          if (Number.isFinite(parsed)) {
            retry = parsed;
          }
          break;
        }
        default:
          break;
      }
    }
    if (dataLines.length === 0 && !event && !id) {
      return null;
    }
    const rawData = dataLines.join("\n");
    let data = rawData;
    const trimmed = rawData.trim();
    if (trimmed.length > 0) {
      try {
        data = JSON.parse(trimmed);
      } catch {
        data = rawData;
      }
    }
    return {
      event,
      data,
      id,
      retry,
      raw: rawData.length > 0 ? rawData : null,
    };
  }

  _getHeader(response, name) {
    if (!response.headers || typeof response.headers.get !== "function") {
      return null;
    }
    const value = response.headers.get(name);
    return value === undefined ? null : value;
  }

  _acceptHeader() {
    const headers = this._config?.defaultHeaders ?? {};
    for (const [key, value] of Object.entries(headers)) {
      if (value !== undefined && value !== null && key.toLowerCase() === "accept") {
        return String(value);
      }
    }
    return "application/json";
  }

  async _maybeJson(response) {
    const contentType = this._getHeader(response, "content-type");
    if (!contentType || !contentType.toLowerCase().includes("application/json")) {
      return null;
    }
    try {
      return await response.json();
    } catch {
      return null;
    }
  }

  async _listIterable(
    path,
    options = {},
    normalizePage,
    allowedKeys = ITERABLE_LIST_OPTION_KEYS,
  ) {
    const optionContext = `options for ${path}`;
    const normalizedOptions = normalizeIterableListOptions(
      options,
      optionContext,
      allowedKeys,
    );
    const canonicalAuth = ToriiClient._normalizeCanonicalAuth(normalizedOptions.canonicalAuth);
    const { signal, canonicalAuth: _ignoredCanonical, ...rest } = normalizedOptions;
    const params = ToriiClient._encodeIterableListParams(rest, optionContext);
    const response = await this._request("GET", path, {
      params: params ?? undefined,
      headers: { Accept: "application/json" },
      signal,
      canonicalAuth,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    const base = ToriiClient._validateIterablePayload(payload);
    return typeof normalizePage === "function" ? normalizePage(base) : base;
  }

  async _queryIterable(
    path,
    options = {},
    normalizePage,
    envelopeHook,
    extraAllowedKeys = [],
    includeListParams = false,
  ) {
    const optionContext = `options for ${path}`;
    const normalizedOptions = normalizeIterableQueryOptions(
      options,
      optionContext,
      extraAllowedKeys,
    );
    const canonicalAuth = ToriiClient._normalizeCanonicalAuth(normalizedOptions.canonicalAuth);
    const { signal, canonicalAuth: _ignoredCanonical, ...rest } = normalizedOptions;
    const envelope = ToriiClient._buildIterableQueryEnvelope(rest);
    if (typeof envelopeHook === "function") {
      envelopeHook(envelope, rest);
    }
    const params = includeListParams
      ? ToriiClient._encodeIterableListParams(rest, optionContext)
      : undefined;
    const response = await this._request("POST", path, {
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(envelope),
      params: params ?? undefined,
      signal,
      canonicalAuth,
    });
    await this._expectStatus(response, [200]);
    const payload = await this._maybeJson(response);
    const base = ToriiClient._validateIterablePayload(payload);
    return typeof normalizePage === "function" ? normalizePage(base) : base;
  }

  _iterateIterable(fetchPage, options = {}) {
    const iteratorLabel =
      typeof fetchPage === "function" && fetchPage.name
        ? `${fetchPage.name} iterator options`
        : "iterator options";
    const normalizedOptions = ToriiClient._normalizeIterableOptions(
      options,
      iteratorLabel,
    );
    const {
      pageSize: rawPageSize,
      maxItems,
      signal,
      ...rest
    } = normalizedOptions;
    const normalizedOffset = ToriiClient._normalizeOffset(rest.offset);
    const baseOptions = { ...rest };
    if (baseOptions.offset !== undefined) {
      delete baseOptions.offset;
    }
    const overallLimit =
      maxItems === undefined || maxItems === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(maxItems, "maxItems", {
            allowZero: false,
          });
    const preferredPageSize =
      rawPageSize === undefined || rawPageSize === null
        ? baseOptions.limit
        : rawPageSize;
    const defaultPageSize = ToriiClient._normalizeUnsignedInteger(
      preferredPageSize ?? DEFAULT_PAGE_SIZE,
      preferredPageSize === undefined ? "pageSize" : "limit",
      { allowZero: false },
    );
    if (baseOptions.limit !== undefined) {
      delete baseOptions.limit;
    }
    const self = this;
    return (async function* iterator() {
      let offset = normalizedOffset;
      let remaining = overallLimit;
      let produced = 0;
      let knownTotal = null;
      while (true) {
        let pageLimit = defaultPageSize;
        if (remaining !== null) {
          pageLimit = Math.min(pageLimit, remaining);
          if (pageLimit <= 0) {
            return;
          }
        }
        const pageOptions = {
          ...baseOptions,
          limit: pageLimit,
          offset,
          signal,
        };
        const page = await fetchPage.call(self, pageOptions);
        if (page && page.total !== undefined && page.total !== null) {
          const parsedTotal = Number(page.total);
          if (Number.isFinite(parsedTotal)) {
            knownTotal = Math.max(0, parsedTotal);
          }
        }
        const items = Array.isArray(page?.items) ? page.items : [];
        if (items.length === 0) {
          return;
        }
        for (const item of items) {
          yield item;
          produced += 1;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        if ((knownTotal !== null && produced >= knownTotal) || items.length < pageLimit) {
          return;
        }
        offset += items.length;
      }
    })();
  }

  _iterateOffsetIterable(
    fetchPage,
    options = {},
    allowedKeys = ITERABLE_OPTION_KEYS,
    itemKeys = ["items"],
  ) {
    const iteratorLabel =
      typeof fetchPage === "function" && fetchPage.name
        ? `${fetchPage.name} iterator options`
        : "iterator options";
    const mergedAllowed = new Set([
      ...allowedKeys,
      "pageSize",
      "maxItems",
      "limit",
      "offset",
      "signal",
    ]);
    const normalizedOptions = ToriiClient._normalizeIterableOptions(
      options,
      iteratorLabel,
      mergedAllowed,
    );
    const normalizedItemKeys = normalizeItemKeyList(itemKeys, iteratorLabel);
    const {
      pageSize: rawPageSize,
      maxItems,
      signal,
      ...rest
    } = normalizedOptions;
    const normalizedOffset = ToriiClient._normalizeOffset(rest.offset);
    const baseOptions = { ...rest };
    if (baseOptions.offset !== undefined) {
      delete baseOptions.offset;
    }
    const overallLimit =
      maxItems === undefined || maxItems === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(maxItems, "maxItems", {
            allowZero: false,
          });
    const preferredPageSize =
      rawPageSize === undefined || rawPageSize === null ? baseOptions.limit : rawPageSize;
    const defaultPageSize = ToriiClient._normalizeUnsignedInteger(
      preferredPageSize ?? DEFAULT_PAGE_SIZE,
      preferredPageSize === undefined ? "pageSize" : "limit",
      { allowZero: false },
    );
    if (baseOptions.limit !== undefined) {
      delete baseOptions.limit;
    }
    const self = this;
    return (async function* iterator() {
      let offset = normalizedOffset;
      let remaining = overallLimit;
      while (true) {
        let pageLimit = defaultPageSize;
        if (remaining !== null) {
          pageLimit = Math.min(pageLimit, remaining);
          if (pageLimit <= 0) {
            return;
          }
        }
        const pageOptions = {
          ...baseOptions,
          limit: pageLimit,
          offset,
          signal,
        };
        const page = await fetchPage.call(self, pageOptions);
        const items = extractOffsetItems(page);
        if (items.length === 0) {
          return;
        }
        for (const item of items) {
          yield item;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        if (items.length < pageLimit) {
          return;
        }
        offset += items.length;
      }
    })();

    function extractOffsetItems(page) {
      if (Array.isArray(page)) {
        return page;
      }
      if (!page || typeof page !== "object") {
        return [];
      }
      for (const key of normalizedItemKeys) {
        if (Array.isArray(page[key])) {
          return page[key];
        }
      }
      const expectedLabel =
        normalizedItemKeys.length === 0
          ? ""
          : ` (expected: ${normalizedItemKeys.join(", ")})`;
      throw new Error(`offset iterator response is missing iterable items${expectedLabel}`);
    }

    function normalizeItemKeyList(rawKeys, contextLabel) {
      if (rawKeys === undefined || rawKeys === null) {
        return ["items"];
      }
      if (typeof rawKeys === "string") {
        const trimmed = rawKeys.trim();
        if (!trimmed) {
          throw new Error(`${contextLabel} item key must be a non-empty string`);
        }
        return [trimmed];
      }
      if (!Array.isArray(rawKeys)) {
        throw new Error(`${contextLabel} item keys must be a string or array of strings`);
      }
      const keys = [];
      for (const entry of rawKeys) {
        if (typeof entry !== "string") {
          throw new Error(`${contextLabel} item keys must be strings`);
        }
        const trimmed = entry.trim();
        if (!trimmed) {
          throw new Error(`${contextLabel} item keys must be non-empty strings`);
        }
        if (!keys.includes(trimmed)) {
          keys.push(trimmed);
        }
      }
      return keys.length > 0 ? keys : ["items"];
    }
  }

  _iterateCursorIterable(fetchPage, options = {}) {
    const iteratorLabel =
      typeof fetchPage === "function" && fetchPage.name
        ? `${fetchPage.name} iterator options`
        : "iterator options";
    const normalizedOptions = ToriiClient._normalizeIterableOptions(
      options,
      iteratorLabel,
    );
    const {
      pageSize: rawPageSize,
      maxItems,
      signal,
      cursor: initialCursor,
      ...rest
    } = normalizedOptions;
    const baseOptions = { ...rest };
    if (baseOptions.limit !== undefined) {
      delete baseOptions.limit;
    }
    if (baseOptions.cursor !== undefined) {
      delete baseOptions.cursor;
    }
    const overallLimit =
      maxItems === undefined || maxItems === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(maxItems, "maxItems", {
            allowZero: false,
          });
    const preferredPageSize =
      rawPageSize === undefined || rawPageSize === null ? rest.limit : rawPageSize;
    const defaultPageSize = ToriiClient._normalizeUnsignedInteger(
      preferredPageSize ?? DEFAULT_PAGE_SIZE,
      preferredPageSize === undefined ? "pageSize" : "limit",
      { allowZero: false },
    );
    let startingCursor = null;
    if (initialCursor !== undefined && initialCursor !== null) {
      startingCursor = requireNonEmptyString(initialCursor, "cursor");
    }
    const self = this;
    return (async function* iterator() {
      let cursor = startingCursor;
      let remaining = overallLimit;
      while (true) {
        let pageLimit = defaultPageSize;
        if (remaining !== null) {
          pageLimit = Math.min(pageLimit, remaining);
          if (pageLimit <= 0) {
            return;
          }
        }
        const pageOptions = {
          ...baseOptions,
          limit: pageLimit,
          cursor: cursor ?? undefined,
          signal,
        };
        const page = await fetchPage.call(self, pageOptions);
        const items = Array.isArray(page?.items) ? page.items : [];
        if (items.length === 0) {
          return;
        }
        for (const item of items) {
          yield item;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        if (!page || page.nextCursor === null || page.nextCursor === undefined) {
          if (items.length < pageLimit) {
            return;
          }
          return;
        }
        cursor = page.nextCursor;
      }
    })();
  }

  _iterateContractInstancePages(fetchPage, options = {}) {
    const iteratorLabel =
      typeof fetchPage === "function" && fetchPage.name
        ? `${fetchPage.name} iterator options`
        : "iterator options";
    const normalizedOptions = ToriiClient._normalizeIterableOptions(
      options,
      iteratorLabel,
    );
    const {
      pageSize: rawPageSize,
      maxItems,
      signal,
      ...rest
    } = normalizedOptions;
    const normalizedOffset = ToriiClient._normalizeOffset(rest.offset);
    const baseOptions = { ...rest };
    if (baseOptions.offset !== undefined) {
      delete baseOptions.offset;
    }
    const overallLimit =
      maxItems === undefined || maxItems === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(maxItems, "maxItems", {
            allowZero: false,
          });
    const preferredPageSize =
      rawPageSize === undefined || rawPageSize === null ? baseOptions.limit : rawPageSize;
    const defaultPageSize = ToriiClient._normalizeUnsignedInteger(
      preferredPageSize ?? DEFAULT_PAGE_SIZE,
      preferredPageSize === undefined ? "pageSize" : "limit",
      { allowZero: false },
    );
    if (baseOptions.limit !== undefined) {
      delete baseOptions.limit;
    }
    const self = this;
    return (async function* iterator() {
      let offset = normalizedOffset;
      let remaining = overallLimit;
      while (true) {
        let pageLimit = defaultPageSize;
        if (remaining !== null) {
          pageLimit = Math.min(pageLimit, remaining);
          if (pageLimit <= 0) {
            return;
          }
        }
        const pageOptions = {
          ...baseOptions,
          offset,
          limit: pageLimit,
          signal,
        };
        const page = await fetchPage.call(self, pageOptions);
        const items = Array.isArray(page?.instances) ? page.instances : [];
        if (items.length === 0) {
          return;
        }
        for (const instance of items) {
          yield instance;
          if (remaining !== null) {
            remaining -= 1;
            if (remaining <= 0) {
              return;
            }
          }
        }
        if (items.length < pageLimit) {
          return;
        }
        offset += items.length;
      }
    })();
  }

  static _validateIterablePayload(payload) {
    if (!payload || typeof payload !== "object" || !Array.isArray(payload.items)) {
      throw new Error("iterable endpoint returned unexpected payload");
    }
    const totalValue =
      payload.total === undefined || payload.total === null
        ? payload.items.length
        : Number(payload.total);
    if (!Number.isFinite(totalValue) || totalValue < 0) {
      throw new Error("iterable endpoint returned invalid total count");
    }
    return {
      items: payload.items,
      total: totalValue,
    };
  }

  static _requireNonEmptyString(value, name) {
    if (typeof value !== "string" || value.trim().length === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a non-empty string`,
        name,
      );
    }
    return value;
  }

  static _requirePlainObject(value, name) {
    if (!isPlainObject(value)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must be a plain object`,
        name,
      );
    }
    return value;
  }

  static _requireAccountId(accountId, name = "accountId") {
    return normalizeAccountId(accountId, name);
  }

  static _normalizeAccountId(accountId, name = "accountId") {
    try {
      return ToriiClient._requireAccountId(accountId, name);
    } catch (error) {
      if (
        error &&
        !(error instanceof AccountAddressError) &&
        error.cause instanceof AccountAddressError
      ) {
        throw error.cause;
      }
      throw error;
    }
  }

  static _requireAssetId(assetId, name = "assetId") {
    return normalizeAssetId(assetId, name);
  }

  static _normalizeAssetId(assetId, name = "assetId") {
    return ToriiClient._requireAssetId(assetId, name);
  }

  static _requireAssetHoldingId(assetHoldingId, name = "assetHoldingId") {
    return normalizeAssetHoldingId(assetHoldingId, name);
  }

  static _normalizeAssetHoldingId(assetHoldingId, name = "assetHoldingId") {
    return ToriiClient._requireAssetHoldingId(assetHoldingId, name);
  }

  static _requireAssetDefinitionId(assetDefinitionId) {
    return ToriiClient._requireNonEmptyString(assetDefinitionId, "assetDefinitionId");
  }

  static _requireDomainId(domainId, context = "domainId") {
    return ToriiClient._requireNonEmptyString(domainId, context);
  }

  static _normalizeIterableOptions(options, context = "options", allowedKeys = ITERABLE_OPTION_KEYS) {
    if (options === undefined || options === null) {
      return {};
    }
    const record = requirePlainObjectOption(options, context);
    assertSupportedOptionKeys(record, allowedKeys, context);
    const normalized = { ...record };
    const { signal } = normalizeSignalOption(record, context);
    if (signal !== undefined) {
      normalized.signal = signal;
    } else {
      delete normalized.signal;
    }
    return normalized;
  }

  static _normalizePrivateKey(value, context = "canonicalAuth.privateKey") {
    const path = typeof context === "string" ? context.replace(/\s+/g, ".") : "canonicalAuth";
    if (value === undefined || value === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${path} must be provided`,
        path,
      );
    }
    let buffer;
    if (Buffer.isBuffer(value)) {
      buffer = value;
    } else if (typeof value === "string") {
      const trimmed = value.trim();
      const hex = trimmed.startsWith("0x") || trimmed.startsWith("0X")
        ? trimmed.slice(2)
        : trimmed;
      if (/^[0-9a-fA-F]+$/u.test(hex) && hex.length % 2 === 0) {
        buffer = Buffer.from(hex, "hex");
      } else {
        buffer = Buffer.from(trimmed, "utf8");
      }
    } else if (ArrayBuffer.isView(value)) {
      buffer = Buffer.from(value.buffer, value.byteOffset, value.byteLength);
    } else if (value instanceof ArrayBuffer) {
      buffer = Buffer.from(value);
    } else if (Array.isArray(value)) {
      buffer = normalizeByteArray(value, path);
    } else {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${path} must be a Buffer, byte array, ArrayBuffer view, ArrayBuffer, or hex string`,
        path,
      );
    }
    if (buffer.length !== 32 && buffer.length !== 64) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${path} must contain a 32- or 64-byte ed25519 private key`,
        path,
      );
    }
    return buffer;
  }

  static _normalizeCanonicalAuth(auth, context = "canonicalAuth") {
    if (auth === undefined || auth === null) {
      return null;
    }
    const record = requirePlainObjectOption(auth, `${context} options`);
    const rawAccountId = record.accountId;
    if (rawAccountId === undefined || rawAccountId === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.accountId is required`,
        `${context}.accountId`,
      );
    }
    const accountId = ToriiClient._normalizeAccountId(rawAccountId, `${context}.accountId`);
    const privateKey = ToriiClient._normalizePrivateKey(
      record.privateKey,
      `${context}.privateKey`,
    );
    return { accountId, privateKey };
  }

  static _splitPermissionedIterableOptions(options, context = "iterable options") {
    if (options === undefined || options === null) {
      return { requirePermissions: false, options: {} };
    }
    const record = requirePlainObjectOption(options, `${context} options`);
    const { requirePermissions, ...rest } = record;
    if (requirePermissions !== undefined && requirePermissions !== null) {
      if (typeof requirePermissions !== "boolean") {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.requirePermissions must be a boolean when provided`,
          `${context}.requirePermissions`,
        );
      }
    }
    return {
      requirePermissions: requirePermissions === true,
      options: rest,
    };
  }

  static _normalizeOptionsWithSignal(options, context) {
    if (options === undefined) {
      return { signal: undefined, rest: {} };
    }
    const record = ensureRecord(options, `${context} options`);
    const { signal } = normalizeSignalOption(record, context);
    const { signal: _ignored, ...rest } = record;
    return { signal, rest };
  }

  static _normalizeBlockListOptions(options) {
    if (options === undefined || options === null) {
      return {};
    }
    const record = requirePlainObjectOption(options, "block list options");
    assertSupportedOptionKeys(record, new Set(["offsetHeight", "limit"]), "block list options");
    const normalized = {};
    if (record.offsetHeight !== undefined && record.offsetHeight !== null) {
      normalized.offsetHeight = ToriiClient._normalizeUnsignedInteger(
        record.offsetHeight,
        "offsetHeight",
        { allowZero: true },
      );
    }
    if (record.limit !== undefined && record.limit !== null) {
      normalized.limit = ToriiClient._normalizeUnsignedInteger(record.limit, "limit");
    }
    return normalized;
  }

  static _normalizeExplorerNftListOptions(options = {}, context = "explorer nft options") {
    const normalized = ToriiClient._normalizeIterableOptions(
      options,
      context,
      EXPLORER_NFT_LIST_OPTION_KEYS,
    );
    const { signal } = normalizeSignalOption(normalized, context);
    const perPageSource =
      normalized.perPage ?? normalized.limit ?? normalized.per_page ?? normalized.limit;
    const perPage = ToriiClient._normalizeUnsignedInteger(
      perPageSource ?? DEFAULT_PAGE_SIZE,
      `${context}.perPage`,
      { allowZero: false },
    );
    let pageValue = normalized.page ?? normalized.page_number ?? null;
    if (pageValue !== null && pageValue !== undefined) {
      pageValue = ToriiClient._normalizeUnsignedInteger(
        pageValue,
        `${context}.page`,
        { allowZero: false },
      );
    } else {
      const offset = ToriiClient._normalizeOffset(normalized.offset);
      pageValue = Math.floor(offset / perPage) + 1;
    }
    const ownedByRaw = normalized.ownedBy ?? normalized.owned_by;
    const domainRaw = normalized.domainId ?? normalized.domain_id ?? normalized.domain;
    const base = {
      page: pageValue,
      perPage,
      signal,
    };
    if (ownedByRaw !== undefined && ownedByRaw !== null) {
      base.ownedBy = ToriiClient._normalizeAccountId(ownedByRaw, `${context}.ownedBy`);
    }
    if (domainRaw !== undefined && domainRaw !== null) {
      base.domain = ToriiClient._requireDomainId(domainRaw, `${context}.domainId`);
    }
    return base;
  }

  static _normalizeExplorerNftIteratorOptions(
    options = {},
    context = "explorer nft iterator options",
  ) {
    const normalized = ToriiClient._normalizeIterableOptions(
      options,
      context,
      EXPLORER_NFT_ITERATOR_OPTION_KEYS,
    );
    const { pageSize, maxItems, ...listOptions } = normalized;
    const base = ToriiClient._normalizeExplorerNftListOptions(listOptions, context);
    const iterator = { ...base };
    if (pageSize !== undefined && pageSize !== null) {
      iterator.perPage = ToriiClient._normalizeUnsignedInteger(
        pageSize,
        `${context}.pageSize`,
        { allowZero: false },
      );
    }
    if (maxItems !== undefined && maxItems !== null) {
      iterator.maxItems = ToriiClient._normalizeUnsignedInteger(
        maxItems,
        `${context}.maxItems`,
        { allowZero: false },
      );
    } else {
      iterator.maxItems = null;
    }
    return iterator;
  }

  static _normalizeExplorerRwaListOptions(options = {}, context = "explorer rwa options") {
    const normalized = ToriiClient._normalizeIterableOptions(
      options,
      context,
      EXPLORER_RWA_LIST_OPTION_KEYS,
    );
    const { signal } = normalizeSignalOption(normalized, context);
    const perPageSource =
      normalized.perPage ?? normalized.limit ?? normalized.per_page ?? normalized.limit;
    const perPage = ToriiClient._normalizeUnsignedInteger(
      perPageSource ?? DEFAULT_PAGE_SIZE,
      `${context}.perPage`,
      { allowZero: false },
    );
    let pageValue = normalized.page ?? normalized.page_number ?? null;
    if (pageValue !== null && pageValue !== undefined) {
      pageValue = ToriiClient._normalizeUnsignedInteger(
        pageValue,
        `${context}.page`,
        { allowZero: false },
      );
    } else {
      const offset = ToriiClient._normalizeOffset(normalized.offset);
      pageValue = Math.floor(offset / perPage) + 1;
    }
    const ownedByRaw = normalized.ownedBy ?? normalized.owned_by;
    const domainRaw = normalized.domainId ?? normalized.domain_id ?? normalized.domain;
    const base = {
      page: pageValue,
      perPage,
      signal,
    };
    if (ownedByRaw !== undefined && ownedByRaw !== null) {
      base.ownedBy = ToriiClient._normalizeAccountId(ownedByRaw, `${context}.ownedBy`);
    }
    if (domainRaw !== undefined && domainRaw !== null) {
      base.domain = ToriiClient._requireDomainId(domainRaw, `${context}.domainId`);
    }
    return base;
  }

  static _normalizeExplorerRwaIteratorOptions(
    options = {},
    context = "explorer rwa iterator options",
  ) {
    const normalized = ToriiClient._normalizeIterableOptions(
      options,
      context,
      EXPLORER_RWA_ITERATOR_OPTION_KEYS,
    );
    const { pageSize, maxItems, ...listOptions } = normalized;
    const base = ToriiClient._normalizeExplorerRwaListOptions(listOptions, context);
    const iterator = { ...base };
    if (pageSize !== undefined && pageSize !== null) {
      iterator.perPage = ToriiClient._normalizeUnsignedInteger(
        pageSize,
        `${context}.pageSize`,
        { allowZero: false },
      );
    }
    if (maxItems !== undefined && maxItems !== null) {
      iterator.maxItems = ToriiClient._normalizeUnsignedInteger(
        maxItems,
        `${context}.maxItems`,
        { allowZero: false },
      );
    } else {
      iterator.maxItems = null;
    }
    return iterator;
  }

  static _normalizeTransactionStatusPollOptions(options, context = "transaction status options") {
    if (options === undefined || options === null) {
      return {
        signal: undefined,
        intervalMs: DEFAULT_TX_STATUS_POLL_INTERVAL_MS,
        timeoutMs: DEFAULT_TX_STATUS_TIMEOUT_MS,
        maxAttempts: null,
        successSet: normalizeStatusSet(undefined, DEFAULT_SUCCESS_STATUSES),
        failureSet: normalizeStatusSet(undefined, DEFAULT_FAILURE_STATUSES),
        onStatus: null,
      };
    }
    const record = requirePlainObjectOption(options, context);
    assertSupportedOptionKeys(record, TX_STATUS_POLL_OPTION_KEYS, context);
    const signalContext =
      typeof context === "string" && context.endsWith(" options")
        ? context.slice(0, -8)
        : context;
    const { signal } = normalizeSignalOption(record, signalContext);
    let intervalMs = DEFAULT_TX_STATUS_POLL_INTERVAL_MS;
    if (record.intervalMs !== undefined && record.intervalMs !== null) {
      intervalMs = ToriiClient._normalizeUnsignedInteger(
        record.intervalMs,
        `${context}.intervalMs`,
        { allowZero: true },
      );
    }
    let timeoutMs = DEFAULT_TX_STATUS_TIMEOUT_MS;
    if (record.timeoutMs === null) {
      timeoutMs = null;
    } else if (record.timeoutMs !== undefined) {
      timeoutMs = ToriiClient._normalizeUnsignedInteger(
        record.timeoutMs,
        `${context}.timeoutMs`,
        { allowZero: true },
      );
    }
    let maxAttempts = null;
    if (record.maxAttempts === null) {
      maxAttempts = null;
    } else if (record.maxAttempts !== undefined) {
      maxAttempts = ToriiClient._normalizeUnsignedInteger(
        record.maxAttempts,
        `${context}.maxAttempts`,
      );
    }
    let onStatus = null;
    if (record.onStatus !== undefined && record.onStatus !== null) {
      if (typeof record.onStatus !== "function") {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.onStatus must be a function`,
          `${context}.onStatus`,
        );
      }
      onStatus = record.onStatus;
    }
    return {
      signal,
      intervalMs,
      timeoutMs,
      maxAttempts,
      successSet: normalizeStatusSet(record.successStatuses, DEFAULT_SUCCESS_STATUSES),
      failureSet: normalizeStatusSet(record.failureStatuses, DEFAULT_FAILURE_STATUSES),
      onStatus,
    };
  }

  static _encodeIterableListParams(options = {}, context = "iterable list options") {
    const optionPath = context ?? "options";
    const normalizedOptions = ToriiClient._normalizeIterableOptions(options, context);
    const params = {};
    if (normalizedOptions.limit !== undefined && normalizedOptions.limit !== null) {
      params.limit = ToriiClient._normalizeUnsignedInteger(normalizedOptions.limit, "limit", {
        allowZero: true,
      });
    }
    if (normalizedOptions.offset !== undefined && normalizedOptions.offset !== null) {
      params.offset = ToriiClient._normalizeOffset(normalizedOptions.offset);
    }
    const filterParam = ToriiClient._normalizeFilterParam(normalizedOptions.filter);
    if (filterParam !== undefined) {
      params.filter = filterParam;
    }
    const sortParam = ToriiClient._encodeSortQueryParam(normalizedOptions.sort);
    if (sortParam !== undefined) {
      params.sort = sortParam;
    }
    if (normalizedOptions.controllerId !== undefined && normalizedOptions.controllerId !== null) {
      params.controller_id = ToriiClient._normalizeAccountId(
        normalizedOptions.controllerId,
        "controllerId",
      );
    }
    if (normalizedOptions.receiverId !== undefined && normalizedOptions.receiverId !== null) {
      params.receiver_id = ToriiClient._normalizeAccountId(
        normalizedOptions.receiverId,
        "receiverId",
      );
    }
    if (
      normalizedOptions.depositAccountId !== undefined &&
      normalizedOptions.depositAccountId !== null
    ) {
      params.deposit_account_id = ToriiClient._normalizeAccountId(
        normalizedOptions.depositAccountId,
        "depositAccountId",
      );
    }
    if (normalizedOptions.assetId !== undefined && normalizedOptions.assetId !== null) {
      params.asset_id = ToriiClient._normalizeAssetId(normalizedOptions.assetId, "assetId");
    }
    if (
      normalizedOptions.certificateExpiresBeforeMs !== undefined &&
      normalizedOptions.certificateExpiresBeforeMs !== null
    ) {
      params.certificate_expires_before_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.certificateExpiresBeforeMs,
        "certificateExpiresBeforeMs",
        { allowZero: true },
      );
    }
    if (
      normalizedOptions.certificateExpiresAfterMs !== undefined &&
      normalizedOptions.certificateExpiresAfterMs !== null
    ) {
      params.certificate_expires_after_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.certificateExpiresAfterMs,
        "certificateExpiresAfterMs",
        { allowZero: true },
      );
    }
    if (
      normalizedOptions.policyExpiresBeforeMs !== undefined &&
      normalizedOptions.policyExpiresBeforeMs !== null
    ) {
      params.policy_expires_before_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.policyExpiresBeforeMs,
        "policyExpiresBeforeMs",
        { allowZero: true },
      );
    }
    if (
      normalizedOptions.policyExpiresAfterMs !== undefined &&
      normalizedOptions.policyExpiresAfterMs !== null
    ) {
      params.policy_expires_after_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.policyExpiresAfterMs,
        "policyExpiresAfterMs",
        { allowZero: true },
      );
    }
    if (
      normalizedOptions.refreshBeforeMs !== undefined &&
      normalizedOptions.refreshBeforeMs !== null
    ) {
      params.refresh_before_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.refreshBeforeMs,
        "refreshBeforeMs",
        { allowZero: true },
      );
    }
    if (
      normalizedOptions.refreshAfterMs !== undefined &&
      normalizedOptions.refreshAfterMs !== null
    ) {
      params.refresh_after_ms = ToriiClient._normalizeUnsignedInteger(
        normalizedOptions.refreshAfterMs,
        "refreshAfterMs",
        { allowZero: true },
      );
    }
    const verdictIdHex = ToriiClient._normalizeOptionalString(
      normalizedOptions.verdictIdHex,
      `${optionPath}.verdictIdHex`,
    );
    const attestationNonceHex = ToriiClient._normalizeOptionalString(
      normalizedOptions.attestationNonceHex,
      `${optionPath}.attestationNonceHex`,
    );
    const certificateIdHex = ToriiClient._normalizeOptionalString(
      normalizedOptions.certificateIdHex,
      `${optionPath}.certificateIdHex`,
    );
    const requireVerdict = ToriiClient._normalizeBooleanOption(
      normalizedOptions.requireVerdict,
      `${optionPath}.requireVerdict`,
    );
    const onlyMissingVerdict = ToriiClient._normalizeBooleanOption(
      normalizedOptions.onlyMissingVerdict,
      `${optionPath}.onlyMissingVerdict`,
    );
    if (requireVerdict && onlyMissingVerdict) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${optionPath}.onlyMissingVerdict cannot be combined with ${optionPath}.requireVerdict`,
        `${optionPath}.onlyMissingVerdict`,
      );
    }
    if (onlyMissingVerdict && verdictIdHex !== undefined) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${optionPath}.verdictIdHex cannot be combined with ${optionPath}.onlyMissingVerdict`,
        `${optionPath}.verdictIdHex`,
      );
    }
    if (verdictIdHex !== undefined) {
      params.verdict_id_hex = verdictIdHex.toLowerCase();
    }
    if (attestationNonceHex !== undefined) {
      params.attestation_nonce_hex = attestationNonceHex.toLowerCase();
    }
    if (certificateIdHex !== undefined) {
      params.certificate_id_hex = certificateIdHex.toLowerCase();
    }
    if (requireVerdict) {
      params.require_verdict = "true";
    }
    if (onlyMissingVerdict) {
      params.only_missing_verdict = "true";
    }
    const platformPolicy = ToriiClient._normalizeOptionalString(
      normalizedOptions.platformPolicy,
      `${optionPath}.platformPolicy`,
    );
    if (platformPolicy !== undefined) {
      params.platform_policy = platformPolicy.toLowerCase();
    }
    if (
      ToriiClient._normalizeBooleanOption(
        normalizedOptions.includeExpired,
        `${optionPath}.includeExpired`,
      )
    ) {
      params.include_expired = "true";
    }
    return Object.keys(params).length === 0 ? undefined : params;
  }

  static _normalizeOptionalString(value, context) {
    if (value === undefined || value === null) {
      return undefined;
    }
    if (typeof value !== "string") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a string`,
        context,
      );
    }
    const trimmed = value.trim();
    return trimmed.length === 0 ? undefined : trimmed;
  }

  static _normalizeBooleanOption(value, context) {
    if (value === undefined || value === null) {
      return false;
    }
    if (typeof value !== "boolean") {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must be a boolean`,
        context,
      );
    }
    return value;
  }

  static _normalizeFilterParam(filter) {
    if (filter === undefined || filter === null) {
      return undefined;
    }
    if (typeof filter === "string") {
      const trimmed = filter.trim();
      if (!trimmed) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          "filter must not be an empty string",
          "filter",
        );
      }
      return trimmed;
    }
    if (typeof filter === "object") {
      const plain = ToriiClient._requirePlainObject(filter, "filter");
      try {
        return JSON.stringify(plain);
      } catch (error) {
        throw createValidationError(
          ValidationErrorCode.INVALID_JSON_VALUE,
          `failed to serialise filter object: ${
            error instanceof Error ? error.message : String(error)
          }`,
          "filter",
          error instanceof Error ? error : undefined,
        );
      }
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "filter must be a string or plain object",
      "filter",
    );
  }

  static _encodeSortQueryParam(sort) {
    if (sort === undefined || sort === null) {
      return undefined;
    }
    if (typeof sort === "string") {
      const trimmed = sort.trim();
      if (!trimmed) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          "sort string must be non-empty",
          "sort",
        );
      }
      const tokens = trimmed
        .split(",")
        .map((token) => token.trim())
        .filter((token) => token.length > 0);
      if (tokens.length === 0) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          "sort string must include at least one key",
          "sort",
        );
      }
      tokens.forEach((token) => {
        const [rawKey, rawOrder] = token.split(":");
        if (!rawKey || !rawKey.trim()) {
          throw createValidationError(
            ValidationErrorCode.INVALID_OBJECT,
            `sort token "${token}" must include a key`,
            "sort",
          );
        }
        if (rawOrder !== undefined) {
          ToriiClient._normalizeSortOrder(
            rawOrder,
            `sort token "${token}" order`,
          );
        }
      });
      return trimmed;
    }
    if (Array.isArray(sort)) {
      if (sort.length === 0) {
        return undefined;
      }
      const parts = sort.map((entry, index) => {
        if (!entry || typeof entry !== "object" || typeof entry.key !== "string") {
          throw createValidationError(
            ValidationErrorCode.INVALID_OBJECT,
            `sort[${index}] must provide a key`,
            `sort[${index}]`,
          );
        }
        const key = entry.key.trim();
        if (!key) {
          throw createValidationError(
            ValidationErrorCode.INVALID_STRING,
            `sort[${index}] key must be non-empty`,
            `sort[${index}].key`,
          );
        }
        const order = ToriiClient._normalizeSortOrder(
          entry.order,
          `sort[${index}].order`,
        );
        return `${key}:${order}`;
      });
      return parts.join(",");
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "sort must be a string or array of {key, order}",
      "sort",
    );
  }

  static _buildIterableQueryEnvelope(options = {}) {
    const pagination = {
      offset: ToriiClient._normalizeOffset(options.offset),
    };
    if (options.limit !== undefined && options.limit !== null) {
      pagination.limit = ToriiClient._normalizeUnsignedInteger(options.limit, "limit", {
        allowZero: true,
      });
    }
    const envelope = {
      pagination,
      sort: ToriiClient._encodeSortArray(options.sort),
    };
    const filterObject = ToriiClient._normalizeFilterObject(options.filter);
    if (filterObject !== undefined) {
      envelope.filter = filterObject;
    }
    if (options.fetchSize !== undefined && options.fetchSize !== null) {
      envelope.fetch_size = ToriiClient._normalizeUnsignedInteger(
        options.fetchSize,
        "fetchSize",
        { allowZero: false },
      );
    }
    if (options.queryName !== undefined && options.queryName !== null) {
      const name = String(options.queryName).trim();
      if (!name) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          "queryName must be a non-empty string",
          "queryName",
        );
      }
      envelope.query = name;
    }
    if (options.select !== undefined && options.select !== null) {
      if (!Array.isArray(options.select)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          "select must be an array of projection objects",
          "select",
        );
      }
      envelope.select = options.select.map((entry, index) => {
        const plain = ToriiClient._requirePlainObject(entry, `select[${index}]`);
        return plain;
      });
    }
    return envelope;
  }

  static _normalizeFilterObject(filter) {
    if (filter === undefined || filter === null) {
      return undefined;
    }
    if (typeof filter === "string") {
      const trimmed = filter.trim();
      if (!trimmed) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          "filter string must be non-empty",
          "filter",
        );
      }
      let parsed;
      try {
        parsed = JSON.parse(trimmed);
      } catch (error) {
        throw createValidationError(
          ValidationErrorCode.INVALID_JSON_VALUE,
          `failed to parse filter JSON: ${
            error instanceof Error ? error.message : String(error)
          }`,
          "filter",
          error instanceof Error ? error : undefined,
        );
      }
      return ToriiClient._requirePlainObject(parsed, "filter");
    }
    if (typeof filter === "object") {
      return ToriiClient._requirePlainObject(filter, "filter");
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "filter must be a string or plain object",
      "filter",
    );
  }

  static _validateOfflineAllowanceFilter(filter, context = "filter") {
    ToriiClient._validateFilterExpression(filter, context, OFFLINE_ALLOWANCE_FILTER_RULES);
  }

  static _validateOfflineTransferFilter(filter, context = "filter") {
    ToriiClient._validateFilterExpression(filter, context, OFFLINE_TRANSFER_FILTER_RULES);
  }

  static _validateOfflineRevocationFilter(filter, context = "filter") {
    ToriiClient._validateFilterExpression(filter, context, OFFLINE_REVOCATION_FILTER_RULES);
  }

  static _validateOfflineSummaryFilter(filter, context = "filter") {
    ToriiClient._validateFilterExpression(filter, context, OFFLINE_SUMMARY_FILTER_RULES);
  }

  static _validateFilterExpression(node, context, rules) {
    const expr = ToriiClient._requirePlainObject(node, context);
    const operators = Object.keys(expr);
    if (operators.length !== 1) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must contain exactly one filter operator`,
        normalizeErrorPath(context),
      );
    }
    const operator = operators[0];
    const operand = expr[operator];
    switch (operator) {
      case "And":
      case "Or": {
        const list = ToriiClient._requireArray(
          operand,
          `${context}.${operator}`,
          { allowEmpty: false },
        );
        list.forEach((entry, index) =>
          ToriiClient._validateFilterExpression(
            entry,
            `${context}.${operator}[${index}]`,
            rules,
          ),
        );
        return;
      }
      case "Not": {
        if (Array.isArray(operand)) {
          if (operand.length !== 1) {
            throw createValidationError(
              ValidationErrorCode.INVALID_OBJECT,
              `${context}.Not must include exactly one expression`,
              normalizeErrorPath(`${context}.Not`),
            );
          }
          ToriiClient._validateFilterExpression(
            operand[0],
            `${context}.Not[0]`,
            rules,
          );
          return;
        }
        ToriiClient._validateFilterExpression(
          operand,
          `${context}.Not`,
          rules,
        );
        return;
      }
      case "Eq":
      case "Ne":
        ToriiClient._validateFilterEquality(operator, operand, rules, context);
        return;
      case "In":
      case "Nin":
        ToriiClient._validateFilterMembership(operator, operand, rules, context);
        return;
      case "Lt":
      case "Lte":
      case "Gt":
      case "Gte":
        ToriiClient._validateFilterRange(operator, operand, rules, context);
        return;
      case "Exists":
        ToriiClient._validateFilterExists(operator, operand, rules, context);
        return;
      case "IsNull":
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.IsNull is not supported`,
          normalizeErrorPath(`${context}.IsNull`),
        );
      default:
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}: unsupported filter operator "${operator}"`,
          normalizeErrorPath(context),
        );
    }
  }

  static _validateFilterEquality(operator, operand, rules, context) {
    const { field, value } = ToriiClient._extractFilterFieldValuePair(
      operand,
      operator,
      context,
    );
    const stringFields = rules?.stringFields ?? EMPTY_FILTER_FIELD_SET;
    const numericFields = rules?.numericFields ?? EMPTY_FILTER_FIELD_SET;
    if (stringFields.has(field)) {
      ToriiClient._assertFilterStringValue(value, `${context}.${operator}[1]`);
      return;
    }
    if (numericFields.has(field)) {
      ToriiClient._assertFilterNumberValue(value, `${context}.${operator}[1]`);
      return;
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${operator}: field "${field}" is not allowed for offline filters`,
      normalizeErrorPath(`${context}.${operator}`),
    );
  }

  static _validateFilterMembership(operator, operand, rules, context) {
    const { field, valueList } = ToriiClient._extractFilterFieldListPair(
      operand,
      operator,
      context,
    );
    const stringFields = rules?.stringFields ?? EMPTY_FILTER_FIELD_SET;
    if (!stringFields.has(field)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${operator}: field "${field}" is not allowed for offline filters`,
        normalizeErrorPath(`${context}.${operator}`),
      );
    }
    valueList.forEach((entry, index) =>
      ToriiClient._assertFilterStringValue(
        entry,
        `${context}.${operator}[1][${index}]`,
      ),
    );
  }

  static _validateFilterRange(operator, operand, rules, context) {
    const { field, value } = ToriiClient._extractFilterFieldValuePair(
      operand,
      operator,
      context,
    );
    const rangeFields = rules?.rangeFields ?? EMPTY_FILTER_FIELD_SET;
    if (!rangeFields.has(field)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${operator}: field "${field}" does not support range filters`,
        normalizeErrorPath(`${context}.${operator}`),
      );
    }
    ToriiClient._assertFilterNumberValue(value, `${context}.${operator}[1]`);
  }

  static _validateFilterExists(operator, operand, rules, context) {
    const field = ToriiClient._extractFilterFieldName(
      operand,
      operator,
      context,
    );
    const existsFields = rules?.existsFields ?? EMPTY_FILTER_FIELD_SET;
    if (!existsFields.has(field)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${operator}: field "${field}" does not support existence checks`,
        normalizeErrorPath(`${context}.${operator}`),
      );
    }
  }

  static _extractFilterFieldValuePair(operand, operator, context) {
    const pair = ToriiClient._requireArray(
      operand,
      `${context}.${operator}`,
      { expectedLength: 2 },
    );
    const field = ToriiClient._normalizeFilterField(pair[0], `${context}.${operator}[0]`);
    return { field, value: pair[1] };
  }

  static _extractFilterFieldListPair(operand, operator, context) {
    const pair = ToriiClient._requireArray(
      operand,
      `${context}.${operator}`,
      { expectedLength: 2 },
    );
    const field = ToriiClient._normalizeFilterField(pair[0], `${context}.${operator}[0]`);
    const values = ToriiClient._requireArray(
      pair[1],
      `${context}.${operator}[1]`,
      { allowEmpty: true },
    );
    return { field, valueList: values };
  }

  static _extractFilterFieldName(operand, operator, context) {
    if (Array.isArray(operand)) {
      if (operand.length === 0) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.${operator} must include a field name`,
          normalizeErrorPath(`${context}.${operator}`),
        );
      }
      return ToriiClient._normalizeFilterField(operand[0], `${context}.${operator}[0]`);
    }
    return ToriiClient._normalizeFilterField(operand, `${context}.${operator}`);
  }

  static _normalizeFilterField(value, context) {
    const raw = ToriiClient._requireNonEmptyString(value, context);
    const trimmed = raw.trim();
    if (!trimmed) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a non-empty string`,
        normalizeErrorPath(context),
      );
    }
    return trimmed;
  }

  static _assertFilterStringValue(value, context) {
    if (typeof value !== "string") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a string`,
        normalizeErrorPath(context),
      );
    }
    return value;
  }

  static _assertFilterNumberValue(value, context) {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${context} must be numeric`,
        normalizeErrorPath(context),
      );
    }
    return value;
  }

  static _requireArray(value, context, { expectedLength = null, allowEmpty = true } = {}) {
    if (!Array.isArray(value)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must be an array`,
        normalizeErrorPath(context),
      );
    }
    if (expectedLength !== null && value.length !== expectedLength) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must contain exactly ${expectedLength} entr${
          expectedLength === 1 ? "y" : "ies"
        }`,
        normalizeErrorPath(context),
      );
    }
    if (!allowEmpty && value.length === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must not be empty`,
        normalizeErrorPath(context),
      );
    }
    return value;
  }

  static _encodeSortArray(sort) {
    if (sort === undefined || sort === null) {
      return [];
    }
    if (Array.isArray(sort)) {
      return sort.map((entry, index) => {
        if (!entry || typeof entry !== "object" || typeof entry.key !== "string") {
          throw createValidationError(
            ValidationErrorCode.INVALID_OBJECT,
            `sort[${index}] must provide a key`,
            `sort[${index}]`,
          );
        }
        const key = entry.key.trim();
        if (!key) {
          throw createValidationError(
            ValidationErrorCode.INVALID_STRING,
            `sort[${index}] key must be non-empty`,
            `sort[${index}].key`,
          );
        }
        const order = ToriiClient._normalizeSortOrder(
          entry.order,
          `sort[${index}].order`,
        );
        return { key, order };
      });
    }
    if (typeof sort === "string") {
      return sort
        .split(",")
        .map((token) => token.trim())
        .filter((token) => token.length > 0)
        .map((token, index) => {
          const [rawKey, rawOrder] = token.split(":");
          if (!rawKey) {
            throw createValidationError(
              ValidationErrorCode.INVALID_OBJECT,
              `sort token at index ${index} must include a key`,
              `sort[${index}]`,
            );
          }
          const key = rawKey.trim();
          if (!key) {
            throw createValidationError(
              ValidationErrorCode.INVALID_STRING,
              `sort token at index ${index} must include a key`,
              `sort[${index}]`,
            );
          }
          const order =
            rawOrder === undefined
              ? "asc"
              : ToriiClient._normalizeSortOrder(
                  rawOrder,
                  `sort token at index ${index} order`,
                );
          return { key, order };
        });
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "sort must be a string or array of {key, order}",
      "sort",
    );
  }

  static _normalizeSortOrder(value, context = "sort order") {
    if (value === undefined || value === null) {
      return "asc";
    }
    const normalized = String(value).trim().toLowerCase();
    if (!normalized) {
      return "asc";
    }
    if (normalized === "asc" || normalized === "desc") {
      return normalized;
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context} must be "asc" or "desc"`,
      normalizeErrorPath(context),
    );
  }

  static _isIsoStatusTerminal(status, resolveOnAcceptedWithoutTransaction) {
    if (!status) {
      return false;
    }
    if (status.transaction_hash) {
      return true;
    }
    const label = status.status ? String(status.status).toLowerCase() : "";
    if (!label) {
      return false;
    }
    if (ISO_NON_TERMINAL_STATUS_VALUES.has(label)) {
      if (label === "accepted" && resolveOnAcceptedWithoutTransaction) {
        return true;
      }
      return false;
    }
    return true;
  }

  static _encodePaginationParams(options = {}) {
    const params = {};
    if (options.limit !== undefined && options.limit !== null) {
      params.limit = ToriiClient._normalizeUnsignedInteger(options.limit, "limit", {
        allowZero: true,
      });
    }
    if (options.offset !== undefined && options.offset !== null) {
      params.offset = ToriiClient._normalizeOffset(options.offset);
    }
    return Object.keys(params).length === 0 ? undefined : params;
  }

  static _normalizeOffset(value) {
    if (value === undefined || value === null) {
      return 0;
    }
    return ToriiClient._normalizeUnsignedInteger(value, "offset", { allowZero: true });
  }

  static _normalizeUnsignedInteger(value, name, options = {}) {
    const allowZero = Boolean(options.allowZero);
    const min = options.min;
    const max = options.max;
    let numeric;
    if (typeof value === "number") {
      if (!Number.isFinite(value) || !Number.isInteger(value)) {
        const qualifier = allowZero ? "non-negative integer" : "positive integer";
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name} must be a ${qualifier}`,
          name,
        );
      }
      if (value < 0 || (!allowZero && value === 0)) {
        const qualifier = allowZero ? "non-negative integer" : "positive integer";
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name} must be a ${qualifier}`,
          name,
        );
      }
      if (!Number.isSafeInteger(value)) {
        throw createValidationError(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name} must be at most ${MAX_SAFE_INTEGER}`,
          name,
        );
      }
      numeric = value;
    } else if (typeof value === "bigint") {
      if (value < 0n || (!allowZero && value === 0n)) {
        const qualifier = allowZero ? "non-negative integer" : "positive integer";
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name} must be a ${qualifier}`,
          name,
        );
      }
      if (value > MAX_SAFE_INTEGER_BIGINT) {
        throw createValidationError(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name} must be at most ${MAX_SAFE_INTEGER}`,
          name,
        );
      }
      numeric = Number(value);
    } else if (typeof value === "string") {
      const trimmed = value.trim();
      if (!/^[0-9]+$/.test(trimmed)) {
        const qualifier = allowZero ? "non-negative integer" : "positive integer";
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name} must be a ${qualifier}`,
          name,
        );
      }
      const bigint = BigInt(trimmed);
      if (bigint < 0n || (!allowZero && bigint === 0n)) {
        const qualifier = allowZero ? "non-negative integer" : "positive integer";
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name} must be a ${qualifier}`,
          name,
        );
      }
      if (bigint > MAX_SAFE_INTEGER_BIGINT) {
        throw createValidationError(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name} must be at most ${MAX_SAFE_INTEGER}`,
          name,
        );
      }
      numeric = Number(bigint);
    } else {
      const qualifier = allowZero ? "non-negative" : "positive";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier} integer`,
        name,
      );
    }
    if (numeric === 0) {
      return 0;
    }
    if (min !== undefined && numeric < min) {
      const qualifier = allowZero ? `at least ${min} or zero` : `at least ${min}`;
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be ${qualifier}`,
        name,
      );
    }
    if (max !== undefined && numeric > max) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be at most ${max}`,
        name,
      );
    }
    return Math.floor(numeric);
  }
}

function normalizeIsoSubmissionResponse(payload, context, options = {}) {
  const record = ToriiClient._requirePlainObject(payload, context);
  const rawMessageId = record.message_id;
  if (typeof rawMessageId !== "string" || rawMessageId.trim().length === 0) {
    throw new Error(`${context} did not return a message_id`);
  }
  const messageId = rawMessageId.trim();
  const status = normalizeIsoStatus(record.status, `${context}.status`);
  let pacs002Code = null;
  if (record.pacs002_code !== undefined && record.pacs002_code !== null) {
    pacs002Code = normalizePacs002Code(record.pacs002_code, `${context}.pacs002_code`);
  }

  const base = {
    message_id: messageId,
    status,
    pacs002_code: pacs002Code,
    transaction_hash: normalizeIsoOptionalString(record.transaction_hash, `${context}.transaction_hash`),
    hold_reason_code: normalizeIsoOptionalString(record.hold_reason_code, `${context}.hold_reason_code`),
    change_reason_codes: normalizeIsoStringArray(
      record.change_reason_codes,
      `${context}.change_reason_codes`,
    ),
    rejection_reason_code: normalizeIsoOptionalString(
      record.rejection_reason_code,
      `${context}.rejection_reason_code`,
    ),
    ledger_id: normalizeIsoOptionalString(record.ledger_id, `${context}.ledger_id`),
    source_account_id: normalizeIsoOptionalString(
      record.source_account_id,
      `${context}.source_account_id`,
    ),
    source_account_address: normalizeIsoOptionalString(
      record.source_account_address,
      `${context}.source_account_address`,
    ),
    target_account_id: normalizeIsoOptionalString(
      record.target_account_id,
      `${context}.target_account_id`,
    ),
    target_account_address: normalizeIsoOptionalString(
      record.target_account_address,
      `${context}.target_account_address`,
    ),
    asset_definition_id: normalizeIsoOptionalString(
      record.asset_definition_id,
      `${context}.asset_definition_id`,
    ),
    asset_id: normalizeIsoOptionalString(record.asset_id, `${context}.asset_id`),
  };

  if (options.includeStatusFields) {
    base.detail = normalizeIsoOptionalString(record.detail, `${context}.detail`, {
      allowEmpty: true,
    });
    if (record.updated_at_ms === undefined || record.updated_at_ms === null) {
      base.updated_at_ms = null;
    } else {
      base.updated_at_ms = ToriiClient._normalizeUnsignedInteger(
        record.updated_at_ms,
        `${context}.updated_at_ms`,
        { allowZero: true },
      );
    }
  }

  return base;
}

function normalizeIsoStatusResponse(payload, context) {
  return normalizeIsoSubmissionResponse(payload, context, { includeStatusFields: true });
}

function normalizeIsoStatus(value, context) {
  const status = ToriiClient._requireNonEmptyString(value, context).trim();
  const normalized = ISO_STATUS_VALUES.get(status.toLowerCase());
  if (!normalized) {
    throw new TypeError(
      `${context} must be one of ${[...ISO_STATUS_VALUES.values()].join(", ")}`,
    );
  }
  return normalized;
}

function normalizePacs002Code(value, context) {
  const code = ToriiClient._requireNonEmptyString(value, context).trim().toUpperCase();
  if (!PACS002_STATUS_CODES.has(code)) {
    throw new TypeError(
      `${context} must be one of ${[...PACS002_STATUS_CODES.values()].join(", ")}`,
    );
  }
  return code;
}

function normalizeIsoMessageKind(value, context) {
  const normalizedPath =
    typeof context === "string" ? context.replace(/\s+/g, ".") : context;
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context} must be 'pacs.008' or 'pacs.009'`,
      normalizedPath,
    );
  }
  const normalized = value.trim().toLowerCase();
  if (normalized === "pacs.008" || normalized === "pacs.009") {
    return normalized;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_STRING,
    `${context} must be 'pacs.008' or 'pacs.009'`,
    normalizedPath,
  );
}

function normalizeIsoSubmissionOptions(options, context, extraAllowedKeys = []) {
  if (options === undefined) {
    return { signal: undefined, contentType: undefined, retryProfile: undefined };
  }
  const optionPath = `${context}.options`;
  const unsupportedLabel = context === "submitIsoMessage" ? `${context} options` : optionPath;
  if (!isPlainObject(options)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${optionPath} must be a plain object`,
      optionPath,
    );
  }
  const allowedKeys = new Set(["signal", "contentType", "retryProfile", ...extraAllowedKeys]);
  const extras = Object.keys(options).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${unsupportedLabel} contains unsupported fields: ${extras.join(", ")}`,
      optionPath,
    );
  }
  const rawSignal = options.signal;
  let signal;
  if (rawSignal !== undefined && rawSignal !== null) {
    if (!isAbortSignalLike(rawSignal)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${optionPath}.signal must be an AbortSignal`,
        `${optionPath}.signal`,
      );
    }
    signal = rawSignal;
  }
  let contentType;
  if (options.contentType !== undefined && options.contentType !== null) {
    if (typeof options.contentType !== "string") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${optionPath}.contentType must be a string`,
        `${optionPath}.contentType`,
      );
    }
    const trimmed = options.contentType.trim();
    if (!trimmed) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${optionPath}.contentType must not be empty`,
        `${optionPath}.contentType`,
      );
    }
    contentType = trimmed;
  }
  let retryProfile;
  if (options.retryProfile !== undefined && options.retryProfile !== null) {
    retryProfile = requireNonEmptyString(options.retryProfile, `${optionPath}.retryProfile`);
  }
  return { signal, contentType, retryProfile };
}

function normalizeIsoStatusOptions(options, context) {
  if (options === undefined) {
    return { signal: undefined, retryProfile: undefined };
  }
  const optionPath = `${context}.options`;
  if (!isPlainObject(options)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${optionPath} must be a plain object`,
      optionPath,
    );
  }
  const extras = Object.keys(options).filter(
    (key) => key !== "signal" && key !== "retryProfile",
  );
  if (extras.length > 0) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${optionPath} contains unsupported fields: ${extras.join(", ")}`,
      optionPath,
    );
  }
  const { signal } = normalizeSignalOption(options, context);
  let retryProfile;
  if (options.retryProfile !== undefined && options.retryProfile !== null) {
    retryProfile = requireNonEmptyString(options.retryProfile, `${optionPath}.retryProfile`);
  }
  return { signal, retryProfile };
}

function requireIsoPlainObject(value, context) {
  if (!isPlainObject(value)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must be a plain object`,
      context,
    );
  }
  return value;
}

function normalizeIsoPayload(message, context) {
  if (message === undefined || message === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} is required`,
      context,
    );
  }
  try {
    return toXmlBuffer(message, context);
  } catch (error) {
    if (error instanceof TypeError) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        error.message,
        context,
        error,
      );
    }
    throw error;
  }
}

function normalizeIsoOptionalString(value, name, options = {}) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string or null`);
  }
  const trimmed = value.trim();
  if (!trimmed && !options.allowEmpty) {
    return null;
  }
  return trimmed;
}

function normalizeIsoStringArray(value, name) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${name} must be an array of strings`);
  }
  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`${name}[${index}] must be a string`);
    }
    const normalized = entry.trim();
    if (!normalized) {
      throw new TypeError(`${name}[${index}] must be a non-empty string`);
    }
    return normalized;
  });
}

function normalizeHealthSnapshot(payload, context) {
  if (payload === null || payload === undefined) {
    throw new TypeError(`${context} must not be empty`);
  }
  if (typeof payload === "string") {
    return { status: requireNonEmptyString(payload, context) };
  }
  const record = ensureRecord(payload, context);
  const source = record.status;
  const status = requireNonEmptyString(
    source == null ? "" : String(source),
    `${context}.status`,
  );
  if (typeof record.status === "string" && record.status === status) {
    return record;
  }
  return { ...record, status };
}

function normalizeStatusSnapshot(payload, state) {
  if (!isPlainObject(payload)) {
    throw new TypeError("status response must be a JSON object");
  }
  const statusPayload = parseStatusPayload(payload);
  const metrics =
    state && typeof state.record === "function"
      ? state.record(statusPayload)
      : computeStatusMetrics(null, statusPayload);
  return {
    timestamp: monotonicTimestamp(),
    status: statusPayload,
    metrics,
  };
}

function createStatusSnapshotState() {
  let previous = null;
  return {
    record(current) {
      const metrics = computeStatusMetrics(previous, current);
      previous = current;
      return metrics;
    },
  };
}

function normalizeTimeNowResponse(payload) {
  const record = ensureRecord(payload ?? {}, "time now response");
  return {
    timestampMs: ToriiClient._normalizeUnsignedInteger(
      record.now,
      "time now response.now",
      { allowZero: true },
    ),
    offsetMs: coerceInteger(record.offset_ms, "time now response.offset_ms"),
    confidenceMs: coerceInteger(record.confidence_ms, "time now response.confidence_ms"),
  };
}

function normalizeTimeStatusResponse(payload) {
  const record = ensureRecord(payload ?? {}, "time status response");
  const peers = ToriiClient._normalizeUnsignedInteger(
    record.peers ?? 0,
    "time status response.peers",
    { allowZero: true },
  );
  const rawSamples = record.samples === undefined ? [] : record.samples;
  if (!Array.isArray(rawSamples)) {
    throw new TypeError("time status response.samples must be an array");
  }
  const samples = rawSamples.map((entry, index) => {
    const sample = ensureRecord(entry, `time status response.samples[${index}]`);
    return {
      peer: requireNonEmptyString(sample.peer, `time status response.samples[${index}].peer`),
      lastOffsetMs: coerceInteger(
        sample.last_offset_ms,
        `time status response.samples[${index}].last_offset_ms`,
      ),
      lastRttMs: coerceInteger(
        sample.last_rtt_ms,
        `time status response.samples[${index}].last_rtt_ms`,
      ),
      count: ToriiClient._normalizeUnsignedInteger(
        sample.count ?? 0,
        `time status response.samples[${index}].count`,
        { allowZero: true },
      ),
    };
  });
  const rttRecord = ensureRecord(record.rtt ?? {}, "time status response.rtt");
  const rawBuckets =
    rttRecord.buckets === undefined ? [] : rttRecord.buckets;
  if (!Array.isArray(rawBuckets)) {
    throw new TypeError("time status response.rtt.buckets must be an array");
  }
  const buckets = rawBuckets.map((entry, index) => {
    const bucket = ensureRecord(entry, `time status response.rtt.buckets[${index}]`);
    return {
      le: coerceInteger(bucket.le, `time status response.rtt.buckets[${index}].le`),
      count: ToriiClient._normalizeUnsignedInteger(
        bucket.count ?? 0,
        `time status response.rtt.buckets[${index}].count`,
        { allowZero: true },
      ),
    };
  });
  const rtt = {
    buckets,
    sumMs: ToriiClient._normalizeUnsignedInteger(
      rttRecord.sum_ms ?? 0,
      "time status response.rtt.sum_ms",
      { allowZero: true },
    ),
    count: ToriiClient._normalizeUnsignedInteger(
      rttRecord.count ?? 0,
      "time status response.rtt.count",
      { allowZero: true },
    ),
  };
  return {
    peers,
    samples,
    rtt,
    note: optionalString(record.note, "time status response.note"),
  };
}

function parseStatusPayload(payload) {
  return {
    peers: coerceStatusInt(payload.peers, "status.peers"),
    queue_size: coerceStatusInt(payload.queue_size, "status.queue_size"),
    commit_time_ms: coerceStatusInt(payload.commit_time_ms, "status.commit_time_ms"),
    da_reschedule_total: coerceStatusInt(
      payload.da_reschedule_total,
      "status.da_reschedule_total",
    ),
    txs_approved: coerceStatusInt(payload.txs_approved, "status.txs_approved"),
    txs_rejected: coerceStatusInt(payload.txs_rejected, "status.txs_rejected"),
    view_changes: coerceStatusInt(payload.view_changes, "status.view_changes"),
    governance: parseGovernanceSnapshot(payload.governance),
    lane_commitments: parseLaneCommitments(payload.lane_commitments),
    dataspace_commitments: parseDataspaceCommitments(payload.dataspace_commitments),
    lane_governance: parseLaneGovernance(payload.lane_governance),
    lane_governance_sealed_total: coerceStatusInt(
      payload.lane_governance_sealed_total,
      "status.lane_governance_sealed_total",
    ),
    lane_governance_sealed_aliases: parseStringArray(
      payload.lane_governance_sealed_aliases,
      "status.lane_governance_sealed_aliases",
    ),
    raw: Object.freeze({ ...payload }),
  };
}

function parseGovernanceSnapshot(payload) {
  if (payload == null) {
    return null;
  }
  const record = ensureRecord(payload, "governance payload");
  const proposals = ensureRecord(record.proposals, "governance.proposals");
  const protectedNamespace = ensureRecord(
    record.protected_namespace,
    "governance.protected_namespace",
  );
  const manifestAdmission = ensureRecord(
    record.manifest_admission,
    "governance.manifest_admission",
  );
  const manifestQuorum = ensureRecord(
    record.manifest_quorum,
    "governance.manifest_quorum",
  );
  const recentActivations = parseGovernanceActivations(
    record.recent_manifest_activations,
  );
  return {
    proposals: {
      proposed: coerceNestedInt(proposals, "proposed", "governance.proposals"),
      approved: coerceNestedInt(proposals, "approved", "governance.proposals"),
      rejected: coerceNestedInt(proposals, "rejected", "governance.proposals"),
      enacted: coerceNestedInt(proposals, "enacted", "governance.proposals"),
    },
    protected_namespace: {
      total_checks: coerceNestedInt(
        protectedNamespace,
        "total_checks",
        "governance.protected_namespace",
      ),
      allowed: coerceNestedInt(
        protectedNamespace,
        "allowed",
        "governance.protected_namespace",
      ),
      rejected: coerceNestedInt(
        protectedNamespace,
        "rejected",
        "governance.protected_namespace",
      ),
    },
    manifest_admission: {
      total_checks: coerceNestedInt(
        manifestAdmission,
        "total_checks",
        "governance.manifest_admission",
      ),
      allowed: coerceNestedInt(
        manifestAdmission,
        "allowed",
        "governance.manifest_admission",
      ),
      missing_manifest: coerceNestedInt(
        manifestAdmission,
        "missing_manifest",
        "governance.manifest_admission",
      ),
      non_validator_authority: coerceNestedInt(
        manifestAdmission,
        "non_validator_authority",
        "governance.manifest_admission",
      ),
      quorum_rejected: coerceNestedInt(
        manifestAdmission,
        "quorum_rejected",
        "governance.manifest_admission",
      ),
      protected_namespace_rejected: coerceNestedInt(
        manifestAdmission,
        "protected_namespace_rejected",
        "governance.manifest_admission",
      ),
      runtime_hook_rejected: coerceNestedInt(
        manifestAdmission,
        "runtime_hook_rejected",
        "governance.manifest_admission",
      ),
    },
    manifest_quorum: {
      total_checks: coerceNestedInt(
        manifestQuorum,
        "total_checks",
        "governance.manifest_quorum",
      ),
      satisfied: coerceNestedInt(
        manifestQuorum,
        "satisfied",
        "governance.manifest_quorum",
      ),
      rejected: coerceNestedInt(
        manifestQuorum,
        "rejected",
        "governance.manifest_quorum",
      ),
    },
    recent_manifest_activations: recentActivations,
  };
}

function parseGovernanceActivations(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(
      "governance.recent_manifest_activations must be an array",
    );
  }
  return payload.map((entry, index) => {
    if (!isPlainObject(entry)) {
      throw new TypeError(
        `governance.recent_manifest_activations[${index}] must be an object`,
      );
    }
    const contractAddress =
      entry.contract_address === undefined || entry.contract_address === null
        ? ""
        : String(entry.contract_address);
    const codeHash =
      entry.code_hash_hex === undefined || entry.code_hash_hex === null
        ? ""
        : String(entry.code_hash_hex);
    const abiHash =
      entry.abi_hash_hex === undefined || entry.abi_hash_hex === null
        ? null
        : String(entry.abi_hash_hex);
    return {
      contract_address: contractAddress,
      code_hash_hex: codeHash,
      abi_hash_hex: abiHash,
      height: coerceNestedInt(
        entry,
        "height",
        `governance.recent_manifest_activations[${index}]`,
      ),
      activated_at_ms: coerceNestedInt(
        entry,
        "activated_at_ms",
        `governance.recent_manifest_activations[${index}]`,
      ),
    };
  });
}

function normalizePipelineStatusPayload(payload) {
  const record = ensureRecord(payload ?? {}, "pipeline status payload");
  const kindValue = record.kind;
  const kind = kindValue == null ? "Unknown" : String(kindValue);
  const contentRecord = isPlainObject(record.content)
    ? ensureRecord(record.content, "pipeline status payload.content")
    : null;
  const hashHex =
    contentRecord && typeof contentRecord.hash === "string"
      ? contentRecord.hash
      : null;
  const authority =
    contentRecord && typeof contentRecord.authority === "string"
      ? contentRecord.authority
      : null;
  const statusValue = contentRecord && "status" in contentRecord
    ? contentRecord.status
    : record.status;
  const status = normalizePipelineStatusStatus(statusValue);
  const normalizedContent =
    contentRecord === null ? null : Object.freeze({ ...contentRecord });
  return Object.freeze({
    kind,
    hashHex,
    authority,
    status,
    content: normalizedContent,
    raw: Object.freeze({ ...record }),
  });
}

function normalizePipelineStatusStatus(value) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === "string") {
    return Object.freeze({ kind: value, content: null, raw: value });
  }
  if (isPlainObject(value)) {
    const record = ensureRecord(value, "pipeline status payload.status");
    return Object.freeze({
      kind: record.kind == null ? null : String(record.kind),
      content: record.content ?? null,
      raw: Object.freeze({ ...record }),
    });
  }
  return Object.freeze({
    kind: String(value),
    content: null,
    raw: value,
  });
}

function parseLaneCommitments(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError("status.lane_commitments must be an array");
  }
  return payload.map((entry, index) => {
    const record = ensureRecord(entry, `status.lane_commitments[${index}]`);
    return {
      block_height: coerceNestedInt(
        record,
        "block_height",
        `status.lane_commitments[${index}]`,
      ),
      lane_id: coerceNestedInt(record, "lane_id", `status.lane_commitments[${index}]`),
      tx_count: coerceNestedInt(record, "tx_count", `status.lane_commitments[${index}]`),
      total_chunks: coerceNestedInt(
        record,
        "total_chunks",
        `status.lane_commitments[${index}]`,
      ),
      rbc_bytes_total: coerceNestedInt(
        record,
        "rbc_bytes_total",
        `status.lane_commitments[${index}]`,
      ),
      teu_total: coerceNestedInt(record, "teu_total", `status.lane_commitments[${index}]`),
      block_hash:
        record.block_hash === undefined || record.block_hash === null
          ? ""
          : String(record.block_hash),
    };
  });
}

function parseDataspaceCommitments(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError("status.dataspace_commitments must be an array");
  }
  return payload.map((entry, index) => {
    const record = ensureRecord(entry, `status.dataspace_commitments[${index}]`);
    return {
      block_height: coerceNestedInt(
        record,
        "block_height",
        `status.dataspace_commitments[${index}]`,
      ),
      lane_id: coerceNestedInt(record, "lane_id", `status.dataspace_commitments[${index}]`),
      dataspace_id: coerceNestedInt(
        record,
        "dataspace_id",
        `status.dataspace_commitments[${index}]`,
      ),
      tx_count: coerceNestedInt(record, "tx_count", `status.dataspace_commitments[${index}]`),
      total_chunks: coerceNestedInt(
        record,
        "total_chunks",
        `status.dataspace_commitments[${index}]`,
      ),
      rbc_bytes_total: coerceNestedInt(
        record,
        "rbc_bytes_total",
        `status.dataspace_commitments[${index}]`,
      ),
      teu_total: coerceNestedInt(record, "teu_total", `status.dataspace_commitments[${index}]`),
      block_hash:
        record.block_hash === undefined || record.block_hash === null
          ? ""
          : String(record.block_hash),
    };
  });
}

function parseLanePrivacyCommitments(payload, context) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    parseLanePrivacyCommitment(entry, `${context}[${index}]`),
  );
}

function parseLanePrivacyCommitment(entry, context) {
  const record = ensureRecord(entry, context);
  const id = coerceNestedInt(record, "id", context);
  const scheme =
    record.scheme === undefined || record.scheme === null ? "" : String(record.scheme);
  if (scheme !== "merkle" && scheme !== "snark") {
    throw new TypeError(`${context}.scheme must be "merkle" or "snark"`);
  }
  let merkle = null;
  if (scheme === "merkle") {
    const merkleRecord = ensureRecord(record.merkle, `${context}.merkle`);
    merkle = {
      root:
        merkleRecord.root === undefined || merkleRecord.root === null
          ? ""
          : String(merkleRecord.root),
      max_depth: coerceNestedInt(merkleRecord, "max_depth", `${context}.merkle`),
    };
  }
  let snark = null;
  if (scheme === "snark") {
    const snarkRecord = ensureRecord(record.snark, `${context}.snark`);
    snark = {
      circuit_id: coerceNestedInt(snarkRecord, "circuit_id", `${context}.snark`),
      verifying_key_digest:
        snarkRecord.verifying_key_digest === undefined ||
        snarkRecord.verifying_key_digest === null
          ? ""
          : String(snarkRecord.verifying_key_digest),
      statement_hash:
        snarkRecord.statement_hash === undefined || snarkRecord.statement_hash === null
          ? ""
          : String(snarkRecord.statement_hash),
      proof_hash:
        snarkRecord.proof_hash === undefined || snarkRecord.proof_hash === null
          ? ""
          : String(snarkRecord.proof_hash),
    };
  }
  return { id, scheme, merkle, snark };
}

function parseLaneSettlementCommitments(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError("status.lane_settlement_commitments must be an array");
  }
  return payload.map((entry, index) => {
    const record = ensureRecord(entry, `status.lane_settlement_commitments[${index}]`);
    const swapMetadataRecord = record.swap_metadata;
    let swapMetadata = null;
    if (swapMetadataRecord != null) {
      const metadata = ensureRecord(
        swapMetadataRecord,
        `status.lane_settlement_commitments[${index}].swap_metadata`,
      );
      swapMetadata = {
        epsilon_bps: coerceNestedInt(
          metadata,
          "epsilon_bps",
          `status.lane_settlement_commitments[${index}].swap_metadata`,
        ),
        twap_window_seconds: coerceNestedInt(
          metadata,
          "twap_window_seconds",
          `status.lane_settlement_commitments[${index}].swap_metadata`,
        ),
        liquidity_profile:
          metadata.liquidity_profile === undefined || metadata.liquidity_profile === null
            ? ""
            : String(metadata.liquidity_profile),
        twap_local_per_xor:
          metadata.twap_local_per_xor === undefined || metadata.twap_local_per_xor === null
            ? ""
            : String(metadata.twap_local_per_xor),
        volatility_class:
          metadata.volatility_class === undefined || metadata.volatility_class === null
            ? ""
            : String(metadata.volatility_class),
      };
    }
    const receiptsRecord = record.receipts;
    if (!Array.isArray(receiptsRecord)) {
      throw new TypeError(
        `status.lane_settlement_commitments[${index}].receipts must be an array`,
      );
    }
    const receipts = receiptsRecord.map((receipt, receiptIndex) => {
      const receiptRecord = ensureRecord(
        receipt,
        `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
      );
      return {
        source_id:
          receiptRecord.source_id === undefined || receiptRecord.source_id === null
            ? ""
            : String(receiptRecord.source_id),
        local_amount_micro: coerceNestedInt(
          receiptRecord,
          "local_amount_micro",
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
        ),
        xor_due_micro: coerceNestedInt(
          receiptRecord,
          "xor_due_micro",
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
        ),
        xor_after_haircut_micro: coerceNestedInt(
          receiptRecord,
          "xor_after_haircut_micro",
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
        ),
        xor_variance_micro: coerceNestedInt(
          receiptRecord,
          "xor_variance_micro",
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
        ),
        timestamp_ms: coerceNestedInt(
          receiptRecord,
          "timestamp_ms",
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
        ),
      };
    });
    return {
      block_height: coerceNestedInt(
        record,
        "block_height",
        `status.lane_settlement_commitments[${index}]`,
      ),
      lane_id: coerceNestedInt(record, "lane_id", `status.lane_settlement_commitments[${index}]`),
      dataspace_id: coerceNestedInt(
        record,
        "dataspace_id",
        `status.lane_settlement_commitments[${index}]`,
      ),
      tx_count: coerceNestedInt(record, "tx_count", `status.lane_settlement_commitments[${index}]`),
      total_local_micro: coerceNestedInt(
        record,
        "total_local_micro",
        `status.lane_settlement_commitments[${index}]`,
      ),
      total_xor_due_micro: coerceNestedInt(
        record,
        "total_xor_due_micro",
        `status.lane_settlement_commitments[${index}]`,
      ),
      total_xor_after_haircut_micro: coerceNestedInt(
        record,
        "total_xor_after_haircut_micro",
        `status.lane_settlement_commitments[${index}]`,
      ),
      total_xor_variance_micro: coerceNestedInt(
        record,
        "total_xor_variance_micro",
        `status.lane_settlement_commitments[${index}]`,
      ),
      swap_metadata: swapMetadata,
      receipts,
    };
  });
}

function parseLaneRelayEnvelopes(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError("status.lane_relay_envelopes must be an array");
  }
  return payload.map((entry, index) => {
    const context = `status.lane_relay_envelopes[${index}]`;
    const record = ensureRecord(entry, context);
    const blockHeader = ensureRecord(record.block_header, `${context}.block_header`);
    const qc =
      record.qc === undefined || record.qc === null ? null : ensureRecord(record.qc, `${context}.qc`);
    const settlementCommitments = parseLaneSettlementCommitments([
      record.settlement_commitment,
    ]);
    if (settlementCommitments.length !== 1) {
      throw new TypeError(`${context}.settlement_commitment must be an object`);
    }
    const settlementHash = requireNonEmptyString(
      record.settlement_hash,
      `${context}.settlement_hash`,
    );
    const manifestRoot =
      record.manifest_root === undefined || record.manifest_root === null
        ? null
        : requireNonEmptyString(record.manifest_root, `${context}.manifest_root`);
    let fastpqProof = null;
    if (record.fastpq_proof !== undefined && record.fastpq_proof !== null) {
      const proof = ensureRecord(record.fastpq_proof, `${context}.fastpq_proof`);
      fastpqProof = {
        proof_digest: requireNonEmptyString(proof.proof_digest, `${context}.fastpq_proof.proof_digest`),
        verified_at_height:
          proof.verified_at_height === undefined || proof.verified_at_height === null
            ? null
            : coerceNestedInt(proof, "verified_at_height", `${context}.fastpq_proof`),
      };
    }
    return {
      lane_id: coerceNestedInt(record, "lane_id", context),
      dataspace_id: coerceNestedInt(record, "dataspace_id", context),
      block_height: coerceNestedInt(record, "block_height", context),
      block_header: blockHeader,
      qc,
      da_commitment_hash:
        record.da_commitment_hash === undefined || record.da_commitment_hash === null
          ? null
          : String(record.da_commitment_hash),
      settlement_commitment: settlementCommitments[0],
      settlement_hash: settlementHash,
      rbc_bytes_total: coerceNestedInt(record, "rbc_bytes_total", context),
      manifest_root: manifestRoot,
      fastpq_proof: fastpqProof,
    };
  });
}

function parseLaneGovernance(payload) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError("status.lane_governance must be an array");
  }
  return payload.map((entry, index) => {
    const record = ensureRecord(entry, `status.lane_governance[${index}]`);
    const runtimePayload = record.runtime_upgrade;
    let runtimeUpgrade = null;
    if (runtimePayload != null) {
      const runtimeRecord = ensureRecord(
        runtimePayload,
        `status.lane_governance[${index}].runtime_upgrade`,
      );
      runtimeUpgrade = {
        allow: coerceBoolean(
          runtimeRecord.allow,
          `status.lane_governance[${index}].runtime_upgrade.allow`,
        ),
        require_metadata: coerceBoolean(
          runtimeRecord.require_metadata,
          `status.lane_governance[${index}].runtime_upgrade.require_metadata`,
        ),
        metadata_key:
          runtimeRecord.metadata_key === undefined || runtimeRecord.metadata_key === null
            ? null
            : String(runtimeRecord.metadata_key),
        allowed_ids: parseStringArray(
          runtimeRecord.allowed_ids,
          `status.lane_governance[${index}].runtime_upgrade.allowed_ids`,
        ),
      };
    }
    return {
      lane_id: coerceNestedInt(record, "lane_id", `status.lane_governance[${index}]`),
      alias:
        record.alias === undefined || record.alias === null ? "" : String(record.alias),
      dataspace_id: coerceNestedInt(
        record,
        "dataspace_id",
        `status.lane_governance[${index}]`,
      ),
      visibility:
        record.visibility === undefined || record.visibility === null
          ? ""
          : String(record.visibility),
      storage_profile:
        record.storage_profile === undefined || record.storage_profile === null
          ? ""
          : String(record.storage_profile),
      governance:
        record.governance === undefined || record.governance === null
          ? null
          : String(record.governance),
      manifest_required: coerceBoolean(
        record.manifest_required,
        `status.lane_governance[${index}].manifest_required`,
      ),
      manifest_ready: coerceBoolean(
        record.manifest_ready,
        `status.lane_governance[${index}].manifest_ready`,
      ),
      manifest_path:
        record.manifest_path === undefined || record.manifest_path === null
          ? null
          : String(record.manifest_path),
      validator_ids: parseStringArray(
        record.validator_ids,
        `status.lane_governance[${index}].validator_ids`,
      ),
      quorum: coerceOptionalInt(
        record.quorum,
        `status.lane_governance[${index}].quorum`,
      ),
      protected_namespaces: parseStringArray(
        record.protected_namespaces,
        `status.lane_governance[${index}].protected_namespaces`,
      ),
      runtime_upgrade: runtimeUpgrade,
      privacy_commitments: parseLanePrivacyCommitments(
        record.privacy_commitments,
        `status.lane_governance[${index}].privacy_commitments`,
      ),
    };
  });
}

function parseSumeragiStatusPayload(payload) {
  const record = ensureRecord(payload, "sumeragi status payload");
  const normalized = { ...record };
  normalized.mode_tag = requireNonEmptyString(
    record.mode_tag ?? "",
    "sumeragi.mode_tag",
  );
  normalized.staged_mode_tag =
    record.staged_mode_tag == null
      ? null
      : requireNonEmptyString(
          record.staged_mode_tag,
          "sumeragi.staged_mode_tag",
        );
  normalized.staged_mode_activation_height =
    record.staged_mode_activation_height == null
      ? null
      : coerceInteger(
          record.staged_mode_activation_height,
          "sumeragi.staged_mode_activation_height",
        );
  normalized.mode_activation_lag_blocks =
    record.mode_activation_lag_blocks == null
      ? null
      : coerceInteger(
          record.mode_activation_lag_blocks,
          "sumeragi.mode_activation_lag_blocks",
        );
  normalized.consensus_caps = record.consensus_caps
    ? parseConsensusCaps(record.consensus_caps)
    : null;
  normalized.commit_qc =
    record.commit_qc == null
      ? null
      : parseCommitQc(record.commit_qc, "sumeragi.commit_qc");
  normalized.commit_quorum =
    record.commit_quorum == null
      ? null
      : parseCommitQuorum(record.commit_quorum, "sumeragi.commit_quorum");
  normalized.lane_commitments = parseLaneCommitments(record.lane_commitments);
  normalized.dataspace_commitments = parseDataspaceCommitments(
    record.dataspace_commitments,
  );
  normalized.lane_settlement_commitments = parseLaneSettlementCommitments(
    record.lane_settlement_commitments,
  );
  normalized.lane_relay_envelopes = parseLaneRelayEnvelopes(record.lane_relay_envelopes);
  normalized.lane_governance = parseLaneGovernance(record.lane_governance);
  normalized.lane_governance_sealed_total = coerceStatusInt(
    record.lane_governance_sealed_total,
    "sumeragi.lane_governance_sealed_total",
  );
  normalized.lane_governance_sealed_aliases = parseStringArray(
    record.lane_governance_sealed_aliases,
    "sumeragi.lane_governance_sealed_aliases",
  );
  return Object.freeze(normalized);
}

function parseConsensusCaps(value) {
  const record = ensureRecord(value, "sumeragi.consensus_caps");
  return Object.freeze({
    collectors_k: coerceInteger(record.collectors_k, "sumeragi.consensus_caps.collectors_k"),
    redundant_send_r: coerceInteger(
      record.redundant_send_r,
      "sumeragi.consensus_caps.redundant_send_r",
    ),
    da_enabled: coerceBoolean(record.da_enabled, "sumeragi.consensus_caps.da_enabled"),
    rbc_chunk_max_bytes: coerceInteger(
      record.rbc_chunk_max_bytes,
      "sumeragi.consensus_caps.rbc_chunk_max_bytes",
    ),
    rbc_session_ttl_ms: coerceInteger(
      record.rbc_session_ttl_ms,
      "sumeragi.consensus_caps.rbc_session_ttl_ms",
    ),
    rbc_store_max_sessions: coerceInteger(
      record.rbc_store_max_sessions,
      "sumeragi.consensus_caps.rbc_store_max_sessions",
    ),
    rbc_store_soft_sessions: coerceInteger(
      record.rbc_store_soft_sessions,
      "sumeragi.consensus_caps.rbc_store_soft_sessions",
    ),
    rbc_store_max_bytes: coerceInteger(
      record.rbc_store_max_bytes,
      "sumeragi.consensus_caps.rbc_store_max_bytes",
    ),
    rbc_store_soft_bytes: coerceInteger(
      record.rbc_store_soft_bytes,
      "sumeragi.consensus_caps.rbc_store_soft_bytes",
    ),
  });
}

function parseCommitQc(value, context) {
  const record = ensureRecord(value, context);
  return Object.freeze({
    height: coerceInteger(record.height, `${context}.height`),
    view: coerceInteger(record.view, `${context}.view`),
    epoch: coerceInteger(record.epoch, `${context}.epoch`),
    block_hash:
      record.block_hash == null
        ? null
        : requireNonEmptyString(record.block_hash, `${context}.block_hash`),
    validator_set_hash:
      record.validator_set_hash == null
        ? null
        : requireNonEmptyString(
            record.validator_set_hash,
            `${context}.validator_set_hash`,
          ),
    validator_set_len: coerceInteger(
      record.validator_set_len,
      `${context}.validator_set_len`,
    ),
    signatures_total: coerceInteger(
      record.signatures_total,
      `${context}.signatures_total`,
    ),
  });
}

function parseCommitQuorum(value, context) {
  const record = ensureRecord(value, context);
  return Object.freeze({
    height: coerceInteger(record.height, `${context}.height`),
    view: coerceInteger(record.view, `${context}.view`),
    block_hash:
      record.block_hash == null
        ? null
        : requireNonEmptyString(record.block_hash, `${context}.block_hash`),
    signatures_present: coerceInteger(
      record.signatures_present,
      `${context}.signatures_present`,
    ),
    signatures_counted: coerceInteger(
      record.signatures_counted,
      `${context}.signatures_counted`,
    ),
    signatures_set_b: coerceInteger(
      record.signatures_set_b,
      `${context}.signatures_set_b`,
    ),
    signatures_required: coerceInteger(
      record.signatures_required,
      `${context}.signatures_required`,
    ),
    last_updated_ms: coerceInteger(
      record.last_updated_ms,
      `${context}.last_updated_ms`,
    ),
  });
}

function normalizeSumeragiCommitQcRecord(payload, context) {
  const record = ensureRecord(payload, context);
  const subject_block_hash = normalizeHex32String(
    record.subject_block_hash,
    `${context}.subject_block_hash`,
    { allowScheme: true },
  );
  if (record.commit_qc == null) {
    return Object.freeze({ subject_block_hash, commit_qc: null });
  }
  return Object.freeze({
    subject_block_hash,
    commit_qc: normalizeSumeragiCommitQcPayload(record.commit_qc, `${context}.commit_qc`),
  });
}

function normalizeSumeragiCommitQcPayload(payload, context) {
  const record = ensureRecord(payload, context);
  return Object.freeze({
    phase: requireNonEmptyString(record.phase, `${context}.phase`),
    parent_state_root: normalizeHex32String(
      record.parent_state_root,
      `${context}.parent_state_root`,
      { allowScheme: true },
    ),
    post_state_root: normalizeHex32String(
      record.post_state_root,
      `${context}.post_state_root`,
      { allowScheme: true },
    ),
    height: coerceInteger(record.height, `${context}.height`),
    view: coerceInteger(record.view, `${context}.view`),
    epoch: coerceInteger(record.epoch, `${context}.epoch`),
    mode_tag: requireNonEmptyString(record.mode_tag, `${context}.mode_tag`),
    validator_set_hash: normalizeHex32String(
      record.validator_set_hash,
      `${context}.validator_set_hash`,
      { allowScheme: true },
    ),
    validator_set_hash_version: coerceInteger(
      record.validator_set_hash_version,
      `${context}.validator_set_hash_version`,
    ),
    validator_set: normalizeStringArray(
      record.validator_set,
      `${context}.validator_set`,
    ),
    signers_bitmap: normalizeArbitraryHex(
      record.signers_bitmap,
      `${context}.signers_bitmap`,
    ),
    bls_aggregate_signature: normalizeArbitraryHex(
      record.bls_aggregate_signature,
      `${context}.bls_aggregate_signature`,
    ),
  });
}

function normalizeStringArray(payload, context) {
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((value, index) =>
    requireNonEmptyString(value, `${context}[${index}]`),
  );
}

function normalizeSumeragiPacemakerSnapshot(payload) {
  const record = ensureRecord(payload, "sumeragi pacemaker response");
  return {
    backoff_ms: coerceInteger(record.backoff_ms, "sumeragi pacemaker.backoff_ms"),
    rtt_floor_ms: coerceInteger(record.rtt_floor_ms, "sumeragi pacemaker.rtt_floor_ms"),
    jitter_ms: coerceInteger(record.jitter_ms, "sumeragi pacemaker.jitter_ms"),
    backoff_multiplier: coerceInteger(
      record.backoff_multiplier,
      "sumeragi pacemaker.backoff_multiplier",
    ),
    rtt_floor_multiplier: coerceInteger(
      record.rtt_floor_multiplier,
      "sumeragi pacemaker.rtt_floor_multiplier",
    ),
    max_backoff_ms: coerceInteger(record.max_backoff_ms, "sumeragi pacemaker.max_backoff_ms"),
    jitter_frac_permille: coerceInteger(
      record.jitter_frac_permille,
      "sumeragi pacemaker.jitter_frac_permille",
    ),
    round_elapsed_ms: coerceInteger(
      record.round_elapsed_ms,
      "sumeragi pacemaker.round_elapsed_ms",
    ),
    view_timeout_target_ms: coerceInteger(
      record.view_timeout_target_ms,
      "sumeragi pacemaker.view_timeout_target_ms",
    ),
    view_timeout_remaining_ms: coerceInteger(
      record.view_timeout_remaining_ms,
      "sumeragi pacemaker.view_timeout_remaining_ms",
    ),
  };
}

function normalizeSumeragiQcSnapshot(payload) {
  const record = ensureRecord(payload, "sumeragi qc response");
  return {
    highest_qc: normalizeSumeragiQcEntry(record.highest_qc, "sumeragi qc.highest_qc"),
    locked_qc: normalizeSumeragiQcEntry(record.locked_qc, "sumeragi qc.locked_qc"),
  };
}

function normalizeSumeragiQcEntry(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    height: coerceInteger(record.height, `${context}.height`),
    view: coerceInteger(record.view, `${context}.view`),
    subject_block_hash:
      record.subject_block_hash == null
        ? null
        : requireNonEmptyString(record.subject_block_hash, `${context}.subject_block_hash`),
  };
}

function normalizeSumeragiPhasesSnapshot(payload) {
  const record = ensureRecord(payload, "sumeragi phases response");
  return {
    propose_ms: coerceInteger(record.propose_ms, "sumeragi phases.propose_ms"),
    collect_da_ms: coerceInteger(record.collect_da_ms, "sumeragi phases.collect_da_ms"),
    collect_prevote_ms: coerceInteger(record.collect_prevote_ms, "sumeragi phases.collect_prevote_ms"),
    collect_precommit_ms: coerceInteger(
      record.collect_precommit_ms,
      "sumeragi phases.collect_precommit_ms",
    ),
    collect_aggregator_ms: coerceInteger(
      record.collect_aggregator_ms,
      "sumeragi phases.collect_aggregator_ms",
    ),
    commit_ms: coerceInteger(record.commit_ms, "sumeragi phases.commit_ms"),
    pipeline_total_ms: coerceInteger(
      record.pipeline_total_ms,
      "sumeragi phases.pipeline_total_ms",
    ),
    collect_aggregator_gossip_total: coerceInteger(
      record.collect_aggregator_gossip_total,
      "sumeragi phases.collect_aggregator_gossip_total",
    ),
    block_created_dropped_by_lock_total: coerceInteger(
      record.block_created_dropped_by_lock_total,
      "sumeragi phases.block_created_dropped_by_lock_total",
    ),
    block_created_hint_mismatch_total: coerceInteger(
      record.block_created_hint_mismatch_total,
      "sumeragi phases.block_created_hint_mismatch_total",
    ),
    block_created_proposal_mismatch_total: coerceInteger(
      record.block_created_proposal_mismatch_total,
      "sumeragi phases.block_created_proposal_mismatch_total",
    ),
    ema_ms: normalizeSumeragiPhasesEma(record.ema_ms, "sumeragi phases.ema_ms"),
  };
}

function normalizeSumeragiPhasesEma(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    propose_ms: coerceInteger(record.propose_ms, `${context}.propose_ms`),
    collect_da_ms: coerceInteger(record.collect_da_ms, `${context}.collect_da_ms`),
    collect_prevote_ms: coerceInteger(record.collect_prevote_ms, `${context}.collect_prevote_ms`),
    collect_precommit_ms: coerceInteger(
      record.collect_precommit_ms,
      `${context}.collect_precommit_ms`,
    ),
    collect_aggregator_ms: coerceInteger(
      record.collect_aggregator_ms,
      `${context}.collect_aggregator_ms`,
    ),
    commit_ms: coerceInteger(record.commit_ms, `${context}.commit_ms`),
    pipeline_total_ms: coerceInteger(record.pipeline_total_ms, `${context}.pipeline_total_ms`),
  };
}

function normalizeSumeragiBlsKeysMap(payload) {
  const record = ensureRecord(payload, "sumeragi BLS keys payload");
  const normalized = {};
  for (const [key, value] of Object.entries(record)) {
    const peerId = requireNonEmptyString(key, "sumeragi BLS key peer id");
    if (value == null) {
      normalized[peerId] = null;
    } else {
      normalized[peerId] = requireNonEmptyString(value, `sumeragi BLS key["${peerId}"]`);
    }
  }
  return normalized;
}

function normalizeSumeragiLeaderSnapshot(payload) {
  const record = ensureRecord(payload, "sumeragi leader response");
  return {
    leader_index: coerceInteger(record.leader_index, "sumeragi leader.leader_index"),
    prf: normalizeSumeragiPrfContext(record.prf, "sumeragi leader.prf"),
  };
}

function normalizeSumeragiPrfContext(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    height: coerceInteger(record.height, `${context}.height`),
    view: coerceInteger(record.view, `${context}.view`),
    epoch_seed:
      record.epoch_seed == null
        ? null
        : requireNonEmptyString(record.epoch_seed, `${context}.epoch_seed`),
  };
}

function normalizeSumeragiCollectorsPlan(payload) {
  const record = ensureRecord(payload, "sumeragi collectors response");
  return {
    consensus_mode: requireNonEmptyString(
      record.consensus_mode,
      "sumeragi collectors.consensus_mode",
    ),
    mode: requireNonEmptyString(record.mode, "sumeragi collectors.mode"),
    topology_len: coerceInteger(record.topology_len, "sumeragi collectors.topology_len"),
    min_votes_for_commit: coerceInteger(
      record.min_votes_for_commit,
      "sumeragi collectors.min_votes_for_commit",
    ),
    proxy_tail_index: coerceInteger(record.proxy_tail_index, "sumeragi collectors.proxy_tail_index"),
    height: coerceInteger(record.height, "sumeragi collectors.height"),
    view: coerceInteger(record.view, "sumeragi collectors.view"),
    collectors_k: coerceInteger(record.collectors_k, "sumeragi collectors.collectors_k"),
    redundant_send_r: coerceInteger(record.redundant_send_r, "sumeragi collectors.redundant_send_r"),
    epoch_seed:
      record.epoch_seed == null
        ? null
        : requireNonEmptyString(record.epoch_seed, "sumeragi collectors.epoch_seed"),
    collectors: normalizeSumeragiCollectorList(record.collectors, "sumeragi collectors.collectors"),
    prf: normalizeSumeragiPrfContext(record.prf, "sumeragi collectors.prf"),
  };
}

function normalizeSumeragiCollectorEntry(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    index: coerceInteger(record.index, `${context}.index`),
    peer_id: requireNonEmptyString(record.peer_id, `${context}.peer_id`),
  };
}

function normalizeSumeragiCollectorList(payload, context) {
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    normalizeSumeragiCollectorEntry(entry, `${context}[${index}]`),
  );
}

function normalizeSumeragiParamsSnapshot(payload) {
  const record = ensureRecord(payload, "sumeragi params response");
  return {
    block_time_ms: coerceInteger(record.block_time_ms, "sumeragi params.block_time_ms"),
    commit_time_ms: coerceInteger(record.commit_time_ms, "sumeragi params.commit_time_ms"),
    max_clock_drift_ms: coerceInteger(
      record.max_clock_drift_ms,
      "sumeragi params.max_clock_drift_ms",
    ),
    collectors_k: coerceInteger(record.collectors_k, "sumeragi params.collectors_k"),
    redundant_send_r: coerceInteger(record.redundant_send_r, "sumeragi params.redundant_send_r"),
    da_enabled: coerceBoolean(record.da_enabled, "sumeragi params.da_enabled"),
    next_mode:
      record.next_mode == null
        ? null
        : requireNonEmptyString(record.next_mode, "sumeragi params.next_mode"),
    mode_activation_height:
      record.mode_activation_height == null
        ? null
        : coerceInteger(
            record.mode_activation_height,
            "sumeragi params.mode_activation_height",
          ),
    chain_height: coerceInteger(record.chain_height, "sumeragi params.chain_height"),
  };
}

function normalizeSumeragiTelemetrySnapshot(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    availability: normalizeSumeragiTelemetryAvailability(
      record.availability,
      `${context}.availability`,
    ),
    qc_latency_ms: normalizeSumeragiTelemetryQcLatencyList(
      record.qc_latency_ms,
      `${context}.qc_latency_ms`,
    ),
    rbc_backlog: normalizeSumeragiTelemetryRbcBacklog(
      record.rbc_backlog,
      `${context}.rbc_backlog`,
    ),
    vrf: normalizeSumeragiTelemetryVrfSummary(record.vrf, `${context}.vrf`),
  };
}

function normalizeSumeragiTelemetryAvailability(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    total_votes_ingested: coerceStatusInt(
      record.total_votes_ingested,
      `${context}.total_votes_ingested`,
    ),
    collectors: normalizeSumeragiTelemetryAvailabilityCollectors(
      record.collectors,
      `${context}.collectors`,
    ),
  };
}

function normalizeSumeragiTelemetryAvailabilityCollectors(payload, context) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    normalizeSumeragiTelemetryAvailabilityCollector(entry, `${context}[${index}]`),
  );
}

function normalizeSumeragiTelemetryAvailabilityCollector(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    collector_idx: coerceStatusInt(record.collector_idx, `${context}.collector_idx`),
    peer_id: requireNonEmptyString(record.peer_id, `${context}.peer_id`),
    votes_ingested: coerceStatusInt(record.votes_ingested, `${context}.votes_ingested`),
  };
}

function normalizeSumeragiTelemetryQcLatencyList(payload, context) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    normalizeSumeragiTelemetryQcLatencyEntry(entry, `${context}[${index}]`),
  );
}

function normalizeSumeragiTelemetryQcLatencyEntry(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    kind: requireNonEmptyString(record.kind, `${context}.kind`),
    last_ms: coerceInteger(record.last_ms, `${context}.last_ms`),
  };
}

function normalizeSumeragiTelemetryRbcBacklog(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    pending_sessions: coerceStatusInt(record.pending_sessions, `${context}.pending_sessions`),
    total_missing_chunks: coerceStatusInt(
      record.total_missing_chunks,
      `${context}.total_missing_chunks`,
    ),
    max_missing_chunks: coerceStatusInt(
      record.max_missing_chunks,
      `${context}.max_missing_chunks`,
    ),
  };
}

function normalizeSumeragiTelemetryVrfSummary(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    found: requireBooleanLike(record.found, `${context}.found`),
    epoch: coerceStatusInt(record.epoch, `${context}.epoch`),
    finalized: requireBooleanLike(record.finalized, `${context}.finalized`),
    seed_hex: optionalString(record.seed_hex, `${context}.seed_hex`),
    epoch_length: coerceStatusInt(record.epoch_length, `${context}.epoch_length`),
    commit_deadline_offset: coerceStatusInt(
      record.commit_deadline_offset,
      `${context}.commit_deadline_offset`,
    ),
    reveal_deadline_offset: coerceStatusInt(
      record.reveal_deadline_offset,
      `${context}.reveal_deadline_offset`,
    ),
    roster_len: coerceStatusInt(record.roster_len, `${context}.roster_len`),
    updated_at_height: coerceStatusInt(
      record.updated_at_height,
      `${context}.updated_at_height`,
    ),
    participants_total: coerceStatusInt(
      record.participants_total,
      `${context}.participants_total`,
    ),
    commitments_total: coerceStatusInt(
      record.commitments_total,
      `${context}.commitments_total`,
    ),
    reveals_total: coerceStatusInt(record.reveals_total, `${context}.reveals_total`),
    late_reveals_total: coerceStatusInt(
      record.late_reveals_total,
      `${context}.late_reveals_total`,
    ),
    committed_no_reveal: normalizeSumeragiTelemetryNumericArray(
      record.committed_no_reveal,
      `${context}.committed_no_reveal`,
    ),
    no_participation: normalizeSumeragiTelemetryNumericArray(
      record.no_participation,
      `${context}.no_participation`,
    ),
    late_reveals: normalizeSumeragiTelemetryVrfLateRevealList(
      record.late_reveals,
      `${context}.late_reveals`,
    ),
  };
}

function normalizeSumeragiTelemetryNumericArray(payload, context) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((value, index) => coerceInteger(value, `${context}[${index}]`));
}

function normalizeSumeragiTelemetryVrfLateRevealList(payload, context) {
  if (payload == null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    normalizeSumeragiTelemetryVrfLateReveal(entry, `${context}[${index}]`),
  );
}

function normalizeSumeragiTelemetryVrfLateReveal(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    signer: requireNonEmptyString(record.signer, `${context}.signer`),
    noted_at_height: coerceStatusInt(record.noted_at_height, `${context}.noted_at_height`),
  };
}

const GOVERNANCE_PROPOSAL_STATUSES = new Set([
  "Proposed",
  "Approved",
  "Rejected",
  "Enacted",
]);

function parseGovernanceProposalResult(payload) {
  const record = ensureRecord(payload, "governance proposal payload");
  if (typeof record.found !== "boolean") {
    throw new TypeError("governance proposal payload missing bool `found` field");
  }
  if (record.proposal == null) {
    return { found: record.found, proposal: null };
  }
  const proposalRecord = ensureRecord(record.proposal, "governance proposal payload.proposal");
  return {
    found: record.found,
    proposal: parseGovernanceProposalRecord(proposalRecord),
  };
}

function parseGovernanceProposalRecord(payload) {
  const record = ensureRecord(payload, "governance proposal record");
  const proposer = requireNonEmptyString(record.proposer, "governance.proposal.proposer");
  const createdHeight = coerceInteger(record.created_height, "governance.proposal.created_height");
  const statusValue = record.status;
  if (typeof statusValue !== "string" || !GOVERNANCE_PROPOSAL_STATUSES.has(statusValue)) {
    throw new TypeError(
      "governance proposal status must be one of Proposed, Approved, Rejected, Enacted",
    );
  }
  const kindPayload = ensureRecord(record.kind, "governance.proposal.kind");
  return {
    proposer,
    created_height: createdHeight,
    status: statusValue,
    kind: parseGovernanceProposalKind(kindPayload, "governance.proposal.kind"),
  };
}

function parseGovernanceProposalKind(payload, context) {
  const entries = Object.entries(payload);
  if (entries.length !== 1) {
    throw new TypeError(`${context} must contain exactly one variant entry`);
  }
  const [variantRaw, details] = entries[0];
  const variant = requireNonEmptyString(variantRaw, `${context}.variant`);
  let deployContract = null;
  if (variant === "DeployContract") {
    if (!isPlainObject(details)) {
      throw new TypeError("DeployContract proposal kind expects an object payload");
    }
    deployContract = parseGovernanceDeployContract(details, `${context}.DeployContract`);
  }
  return {
    variant,
    deploy_contract: deployContract,
    raw: cloneGovernanceKindRaw(details),
  };
}

function parseGovernanceDeployContract(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    contract_address: requireNonEmptyString(
      record.contract_address,
      `${context}.contract_address`,
    ),
    code_hash_hex: requireNonEmptyString(record.code_hash_hex, `${context}.code_hash_hex`),
    abi_hash_hex: requireNonEmptyString(record.abi_hash_hex, `${context}.abi_hash_hex`),
    abi_version: requireNonEmptyString(record.abi_version, `${context}.abi_version`),
  };
}

function cloneGovernanceKindRaw(details) {
  if (isPlainObject(details)) {
    return { ...details };
  }
  return { value: details };
}

function parseGovernanceReferendumResult(payload) {
  const record = ensureRecord(payload, "governance referendum payload");
  if (typeof record.found !== "boolean") {
    throw new TypeError("governance referendum payload missing bool `found` field");
  }
  let referendum = null;
  if (record.referendum != null) {
    if (!isPlainObject(record.referendum)) {
      throw new TypeError("governance referendum payload `referendum` must be an object");
    }
    referendum = { ...record.referendum };
  }
  return { found: record.found, referendum };
}

function parseGovernanceTally(payload) {
  const record = ensureRecord(payload, "governance tally payload");
  const referendumId = requireNonEmptyString(
    record.referendum_id,
    "governance.tally.referendum_id",
  );
  return {
    referendum_id: referendumId,
    approve: coerceInteger(record.approve ?? 0, "governance.tally.approve"),
    reject: coerceInteger(record.reject ?? 0, "governance.tally.reject"),
    abstain: coerceInteger(record.abstain ?? 0, "governance.tally.abstain"),
  };
}

function createEmptyGovernanceTallyResult(referendumId) {
  return {
    found: false,
    referendum_id: requireNonEmptyString(
      referendumId,
      "governance tally referendum_id",
    ),
    tally: null,
  };
}

function createEmptyGovernanceLocksResult(referendumId) {
  return {
    found: false,
    referendum_id: requireNonEmptyString(
      referendumId,
      "governance locks referendum_id",
    ),
    locks: {},
  };
}

function parseGovernanceLocksResult(payload) {
  const record = ensureRecord(payload, "governance locks payload");
  if (typeof record.found !== "boolean") {
    throw new TypeError("governance locks payload missing bool `found` field");
  }
  const referendumId = requireNonEmptyString(
    record.referendum_id,
    "governance.locks.referendum_id",
  );
  const locksPayload = record.locks ?? {};
  if (!isPlainObject(locksPayload)) {
    throw new TypeError("governance locks payload `locks` must be an object");
  }
  const locks = {};
  for (const [accountId, entry] of Object.entries(locksPayload)) {
    if (typeof accountId !== "string" || !accountId) {
      throw new TypeError("governance locks keys must be non-empty account identifiers");
    }
    locks[accountId] = parseGovernanceLockRecord(
      ensureRecord(entry, `governance.locks["${accountId}"]`),
      `governance.locks["${accountId}"]`,
    );
  }
  return {
    found: record.found,
    referendum_id: referendumId,
    locks,
  };
}

function parseGovernanceLockRecord(payload, context) {
  const owner = requireNonEmptyString(payload.owner, `${context}.owner`);
  const amount = coerceInteger(payload.amount, `${context}.amount`);
  const expiryHeight = coerceInteger(payload.expiry_height, `${context}.expiry_height`);
  const direction = coerceInteger(payload.direction, `${context}.direction`);
  if (direction < 0 || direction > 255) {
    throw new RangeError(`${context}.direction must be within 0-255`);
  }
  const durationBlocks = coerceInteger(payload.duration_blocks ?? 0, `${context}.duration_blocks`);
  return {
    owner,
    amount,
    expiry_height: expiryHeight,
    direction,
    duration_blocks: durationBlocks,
  };
}

function parseGovernanceUnlockStats(payload) {
  const record = ensureRecord(payload, "governance unlock stats payload");
  return {
    height_current: coerceInteger(record.height_current, "governance.unlock_stats.height_current"),
    expired_locks_now: coerceInteger(
      record.expired_locks_now,
      "governance.unlock_stats.expired_locks_now",
    ),
    referenda_with_expired: coerceInteger(
      record.referenda_with_expired,
      "governance.unlock_stats.referenda_with_expired",
    ),
    last_sweep_height: coerceInteger(
      record.last_sweep_height,
      "governance.unlock_stats.last_sweep_height",
    ),
  };
}

function normalizeGovernanceCouncilCurrentResponse(payload) {
  const record = ensureRecord(payload, "governance council current response");
  const derivedRaw = record.derived_by ?? "Vrf";
  const derivedBy =
    String(derivedRaw).trim().toLowerCase() === "fallback" ? "Fallback" : "Vrf";
  return {
    epoch: ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      "governance council current response.epoch",
      { allowZero: true },
    ),
    members: normalizeGovernanceCouncilMembers(
      record.members,
      "governance council current response.members",
    ),
    alternates: normalizeGovernanceCouncilMembers(
      record.alternates ?? [],
      "governance council current response.alternates",
    ),
    candidate_count: ToriiClient._normalizeUnsignedInteger(
      record.candidate_count ?? 0,
      "governance council current response.candidate_count",
      { allowZero: true },
    ),
    verified: ToriiClient._normalizeUnsignedInteger(
      record.verified ?? 0,
      "governance council current response.verified",
      { allowZero: true },
    ),
    derived_by: derivedBy,
  };
}

function normalizeErrorPath(context) {
  return typeof context === "string" ? context.replace(/\s+/g, ".") : context;
}

function normalizeGovernanceCouncilMembers(value, context) {
  if (!Array.isArray(value)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must be an array`,
      normalizeErrorPath(context),
    );
  }
  return value.map((entry, index) => {
    const record = ensureRecord(entry, `${context}[${index}]`);
    return {
      account_id: ToriiClient._normalizeAccountId(
        record.account_id,
        `${context}[${index}].account_id`,
      ),
    };
  });
}

function normalizeGovernanceCouncilDeriveRequest(input, context = "governanceDeriveCouncilVrf payload") {
  const record = ensureRecord(input, context);
  const payload = {
    candidates: normalizeGovernanceCouncilCandidateList(
      record.candidates,
      `${context}.candidates`,
    ),
  };
  if (record.committee_size !== undefined || record.committeeSize !== undefined) {
    const committeeValue = record.committee_size ?? record.committeeSize;
    payload.committee_size = ToriiClient._normalizeUnsignedInteger(
      committeeValue,
      `${context}.committee_size`,
      { allowZero: false },
    );
  }
  if (record.alternate_size !== undefined || record.alternateSize !== undefined) {
    const alternateValue = record.alternate_size ?? record.alternateSize;
    payload.alternate_size = ToriiClient._normalizeUnsignedInteger(
      alternateValue,
      `${context}.alternate_size`,
      { allowZero: false },
    );
  }
  const epochValue = record.epoch;
  if (epochValue !== undefined && epochValue !== null) {
    payload.epoch = ToriiClient._normalizeUnsignedInteger(
      epochValue,
      `${context}.epoch`,
      { allowZero: true },
    );
  }
  return payload;
}

function normalizeGovernanceCouncilCandidateList(value, context) {
  if (!Array.isArray(value) || value.length === 0) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must be a non-empty array`,
      normalizeErrorPath(context),
    );
  }
  return value.map((entry, index) =>
    normalizeGovernanceCouncilCandidate(entry, `${context}[${index}]`),
  );
}

function normalizeGovernanceCouncilCandidate(input, context) {
  const record = ensureRecord(input, context);
  const candidate = {
    account_id: ToriiClient._normalizeAccountId(
      record.account_id ?? record.accountId,
      `${context}.account_id`,
    ),
    variant: normalizeGovernanceCouncilVariant(record.variant, `${context}.variant`),
  };
  const pkValue =
    record.pk ??
    record.pk_b64 ??
    record.pkB64 ??
    record.publicKey ??
    record.public_key ??
    record.publicKeyB64 ??
    record.public_key_b64;
  if (pkValue === undefined || pkValue === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context}.pk is required`,
      `${normalizeErrorPath(context)}.pk`,
    );
  }
  candidate.pk_b64 = normalizeRequiredBase64Payload(pkValue, `${context}.pk`);
  const proofValue =
    record.proof ??
    record.proof_b64 ??
    record.proofB64 ??
    record.signature ??
    record.signature_b64 ??
    record.signatureB64;
  if (proofValue === undefined || proofValue === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context}.proof is required`,
      `${normalizeErrorPath(context)}.proof`,
    );
  }
  candidate.proof_b64 = normalizeRequiredBase64Payload(proofValue, `${context}.proof`);
  return candidate;
}

function normalizeGovernanceCouncilVariant(value, name) {
  const normalized = requireNonEmptyString(value, name);
  const lowered = normalized.toLowerCase();
  if (lowered === "normal") {
    return "Normal";
  }
  if (lowered === "small") {
    return "Small";
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_STRING,
    `${name} must be "Normal" or "Small"`,
    normalizeErrorPath(name),
  );
}

function normalizeGovernanceCouncilDeriveResponse(
  payload,
  context = "governance council derive response",
) {
  const record = ensureRecord(payload, context);
  const derivedRaw = record.derived_by ?? "Vrf";
  const derivedBy =
    String(derivedRaw).trim().toLowerCase() === "fallback" ? "Fallback" : "Vrf";
  return {
    epoch: ToriiClient._normalizeUnsignedInteger(record.epoch, `${context}.epoch`, {
      allowZero: true,
    }),
    members: normalizeGovernanceCouncilMembers(record.members, `${context}.members`),
    alternates: normalizeGovernanceCouncilMembers(
      record.alternates ?? [],
      `${context}.alternates`,
    ),
    total_candidates: ToriiClient._normalizeUnsignedInteger(
      record.total_candidates,
      `${context}.total_candidates`,
      { allowZero: true },
    ),
    verified: ToriiClient._normalizeUnsignedInteger(
      record.verified,
      `${context}.verified`,
      { allowZero: true },
    ),
    derived_by: derivedBy,
  };
}

function normalizeGovernanceCouncilPersistRequest(input) {
  const base = normalizeGovernanceCouncilDeriveRequest(
    input,
    "governancePersistCouncil payload",
  );
  const record = ensureRecord(input, "governancePersistCouncil payload");
  const authorityValue = record.authority;
  const hasPrivateKey =
    pickOverride(record, "private_key", "privateKey") !== undefined ||
    pickOverride(record, "private_key_hex", "privateKeyHex") !== undefined ||
    pickOverride(record, "private_key_bytes", "privateKeyBytes") !== undefined ||
    pickOverride(record, "private_key_multihash", "privateKeyMultihash") !== undefined;
  if (authorityValue === undefined && !hasPrivateKey) {
    return base;
  }
  if (!authorityValue || !hasPrivateKey) {
    throw new TypeError(
      "governancePersistCouncil payload requires both authority and privateKey when credentials are provided",
    );
  }
  return {
    ...base,
    authority: ToriiClient._normalizeAccountId(
      authorityValue,
      "governancePersistCouncil.authority",
    ),
    private_key: resolveAuthorityPrivateKey(
      record,
      "governancePersistCouncil",
    ),
  };
}

function normalizeGovernanceCouncilPersistResponse(payload) {
  return normalizeGovernanceCouncilDeriveResponse(
    payload,
    "governance council persist response",
  );
}

function normalizeGovernanceCouncilReplaceRequest(input) {
  const record = ensureRecord(input ?? {}, "governanceReplaceCouncil payload");
  const body = {
    missing: ToriiClient._normalizeAccountId(
      record.missing,
      "governanceReplaceCouncil.missing",
    ),
  };
  if (record.epoch !== undefined && record.epoch !== null) {
    body.epoch = ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      "governanceReplaceCouncil.epoch",
      { allowZero: true },
    );
  }
  if (record.authority !== undefined && record.authority !== null) {
    body.authority = ToriiClient._normalizeAccountId(
      record.authority,
      "governanceReplaceCouncil.authority",
    );
  }
  if (record.private_key !== undefined || record.privateKey !== undefined) {
    body.private_key = requireNonEmptyString(
      record.private_key ?? record.privateKey,
      "governanceReplaceCouncil.privateKey",
    );
  }
  return body;
}

function normalizeGovernanceCouncilReplaceResponse(payload) {
  const record = ensureRecord(payload, "governance council replace response");
  return {
    epoch: ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      "governance council replace response.epoch",
      { allowZero: true },
    ),
    members: normalizeGovernanceCouncilMembers(
      record.members ?? [],
      "governance council replace response.members",
    ),
    alternates: normalizeGovernanceCouncilMembers(
      record.alternates ?? [],
      "governance council replace response.alternates",
    ),
    replaced: coerceBoolean(
      record.replaced ?? false,
      "governance council replace response.replaced",
    ),
  };
}

const GOVERNANCE_COUNCIL_AUDIT_OPTION_KEYS = new Set(["epoch", "signal"]);

function buildGovernanceCouncilAuditQuery(options = {}) {
  const normalizedOptions =
    options === undefined
      ? {}
      : ensureRecord(options, "getGovernanceCouncilAudit options");
  const { signal } = normalizeSignalOption(
    normalizedOptions,
    "getGovernanceCouncilAudit",
  );
  assertSupportedOptionKeys(
    normalizedOptions,
    GOVERNANCE_COUNCIL_AUDIT_OPTION_KEYS,
    "getGovernanceCouncilAudit options",
  );
  const params = {};
  if (normalizedOptions.epoch !== undefined && normalizedOptions.epoch !== null) {
    params.epoch = ToriiClient._normalizeUnsignedInteger(
      normalizedOptions.epoch,
      "governanceCouncilAudit.epoch",
      { allowZero: true },
    );
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function normalizeGovernanceCouncilAuditResponse(payload) {
  const record = ensureRecord(payload, "governance council audit response");
  const derivedRaw = record.derived_by ?? "Vrf";
  const derivedBy =
    String(derivedRaw).trim().toLowerCase() === "fallback" ? "Fallback" : "Vrf";
  return {
    epoch: ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      "governance council audit response.epoch",
      { allowZero: true },
    ),
    seed_hex: normalizeArbitraryHex(
      record.seed_hex,
      "governance council audit response.seed_hex",
    ),
    beacon_hex: normalizeHex32String(
      record.beacon_hex,
      "governance council audit response.beacon_hex",
    ),
    members_count: ToriiClient._normalizeUnsignedInteger(
      record.members_count ?? 0,
      "governance council audit response.members_count",
      { allowZero: true },
    ),
    candidate_count: ToriiClient._normalizeUnsignedInteger(
      record.candidate_count ?? 0,
      "governance council audit response.candidate_count",
      { allowZero: true },
    ),
    alternates_count: ToriiClient._normalizeUnsignedInteger(
      record.alternates_count ?? 0,
      "governance council audit response.alternates_count",
      { allowZero: true },
    ),
    verified: ToriiClient._normalizeUnsignedInteger(
      record.verified ?? 0,
      "governance council audit response.verified",
      { allowZero: true },
    ),
    derived_by: derivedBy,
    chain_id: requireNonEmptyString(
      record.chain_id,
      "governance council audit response.chain_id",
    ),
  };
}

function normalizeProtectedNamespaceList(input) {
  if (input === undefined || input === null) {
    throw new TypeError("protected namespaces input is required");
  }
  const values = Array.isArray(input) ? input : [input];
  if (values.length === 0) {
    throw new TypeError("protected namespaces list must not be empty");
  }
  return values.map((entry, index) =>
    requireNonEmptyString(entry, `namespaces[${index}]`),
  );
}

function normalizeProtectedNamespacesApplyResponse(payload) {
  const record = ensureRecord(payload, "protected namespaces apply response");
  return {
    ok: coerceBoolean(record.ok, "protected namespaces apply response.ok"),
    applied: coerceInteger(record.applied ?? 0, "protected namespaces apply response.applied"),
  };
}

function normalizeProtectedNamespacesGetResponse(payload) {
  const record = ensureRecord(payload, "protected namespaces response");
  return {
    found: coerceBoolean(record.found, "protected namespaces response.found"),
    namespaces: parseStringArray(
      record.namespaces,
      "protected namespaces response.namespaces",
    ).map((value, index) =>
      requireNonEmptyString(value, `protected namespaces response.namespaces[${index}]`),
    ),
  };
}

function normalizeNodeCapabilitiesResponse(payload) {
  const record = ensureRecord(payload, "node capabilities response");
  const cryptoRecord = ensureRecord(record.crypto ?? {}, "node capabilities response.crypto");
  const curvesRecord = cryptoRecord.curves ?? {};
  return {
    abiVersion: ToriiClient._normalizeUnsignedInteger(
      record.abi_version,
      "node capabilities response.abi_version",
      { allowZero: false },
    ),
    dataModelVersion: ToriiClient._normalizeUnsignedInteger(
      record.data_model_version,
      "node capabilities response.data_model_version",
      { allowZero: false },
    ),
    crypto: {
      sm: normalizeNodeSmCapabilities(cryptoRecord.sm, "node capabilities response.crypto.sm"),
      curves: normalizeNodeCurveCapabilities(
        curvesRecord,
        "node capabilities response.crypto.curves",
      ),
    },
  };
}

function normalizeSccpCapabilitiesResponse(payload) {
  const record = ensureRecord(payload, "sccp capabilities response");
  return {
    localDomain: ToriiClient._normalizeUnsignedInteger(
      record.local_domain,
      "sccp capabilities response.local_domain",
      { allowZero: true },
    ),
    localChain: requireNonEmptyString(
      record.local_chain,
      "sccp capabilities response.local_chain",
    ),
    proofFamily: requireNonEmptyString(
      record.proof_family,
      "sccp capabilities response.proof_family",
    ),
    burnBundlePath: requireNonEmptyString(
      record.burn_bundle_path,
      "sccp capabilities response.burn_bundle_path",
    ),
    governanceBundlePath: requireNonEmptyString(
      record.governance_bundle_path,
      "sccp capabilities response.governance_bundle_path",
    ),
    messageBundlePath: requireNonEmptyString(
      record.message_bundle_path,
      "sccp capabilities response.message_bundle_path",
    ),
    messageProofPath: requireNonEmptyString(
      record.message_proof_path,
      "sccp capabilities response.message_proof_path",
    ),
    messageJobPath: requireNonEmptyString(
      record.message_job_path,
      "sccp capabilities response.message_job_path",
    ),
    proofManifestPath: requireNonEmptyString(
      record.proof_manifest_path,
      "sccp capabilities response.proof_manifest_path",
    ),
    legacyBurnRegistryBackend: requireNonEmptyString(
      record.legacy_burn_registry_backend,
      "sccp capabilities response.legacy_burn_registry_backend",
    ),
    legacyGovernanceRegistryBackend: requireNonEmptyString(
      record.legacy_governance_registry_backend,
      "sccp capabilities response.legacy_governance_registry_backend",
    ),
    proofSubmitPath: optionalString(
      record.proof_submit_path ?? null,
      "sccp capabilities response.proof_submit_path",
    ),
    messageSubmitPath: optionalString(
      record.message_submit_path ?? null,
      "sccp capabilities response.message_submit_path",
    ),
    messagePayloadKinds: parseStringArray(
      record.message_payload_kinds,
      "sccp capabilities response.message_payload_kinds",
    ).map((value, index) =>
      requireNonEmptyString(value, `sccp capabilities response.message_payload_kinds[${index}]`),
    ),
    codecs: parseRecordArray(
      record.codecs,
      "sccp capabilities response.codecs",
    ).map((entry, index) =>
      normalizeSccpCodecCapability(entry, `sccp capabilities response.codecs[${index}]`),
    ),
    counterparties: parseRecordArray(
      record.counterparties,
      "sccp capabilities response.counterparties",
    ).map((entry, index) =>
      normalizeSccpCounterpartyCapability(
        entry,
        `sccp capabilities response.counterparties[${index}]`,
      ),
    ),
  };
}

function normalizeSccpCodecCapability(value, context) {
  const record = ensureRecord(value, context);
  return {
    id: ToriiClient._normalizeUnsignedInteger(record.id, `${context}.id`, { allowZero: false }),
    key: requireNonEmptyString(record.key, `${context}.key`),
    description: requireNonEmptyString(record.description, `${context}.description`),
  };
}

function normalizeSccpCounterpartyCapability(value, context) {
  const record = ensureRecord(value, context);
  return {
    domain: ToriiClient._normalizeUnsignedInteger(record.domain, `${context}.domain`, {
      allowZero: true,
    }),
    chain: requireNonEmptyString(record.chain, `${context}.chain`),
    messageBackend: requireNonEmptyString(record.message_backend, `${context}.message_backend`),
    registryBackend: requireNonEmptyString(
      record.registry_backend,
      `${context}.registry_backend`,
    ),
    counterpartyAccountCodec: ToriiClient._normalizeUnsignedInteger(
      record.counterparty_account_codec,
      `${context}.counterparty_account_codec`,
      { allowZero: false },
    ),
    counterpartyAccountCodecKey: requireNonEmptyString(
      record.counterparty_account_codec_key,
      `${context}.counterparty_account_codec_key`,
    ),
  };
}

function normalizeSccpProofManifestSetResponse(payload) {
  const record = ensureRecord(payload, "sccp proof manifests response");
  return {
    localDomain: ToriiClient._normalizeUnsignedInteger(
      record.local_domain,
      "sccp proof manifests response.local_domain",
      { allowZero: true },
    ),
    localChain: requireNonEmptyString(
      record.local_chain,
      "sccp proof manifests response.local_chain",
    ),
    proofFamily: requireNonEmptyString(
      record.proof_family,
      "sccp proof manifests response.proof_family",
    ),
    manifests: parseRecordArray(
      record.manifests,
      "sccp proof manifests response.manifests",
    ).map((entry, index) =>
      normalizeSccpProofManifest(entry, `sccp proof manifests response.manifests[${index}]`),
    ),
  };
}

function normalizeSccpProofManifest(value, context) {
  const record = ensureRecord(value, context);
  const finalityModel = requireNonEmptyString(record.finality_model, `${context}.finality_model`);
  const verifierTarget = requireNonEmptyString(
    record.verifier_target,
    `${context}.verifier_target`,
  );
  if (!SCCP_FINALITY_MODEL_VALUES.has(finalityModel)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.finality_model must be a supported SCCP finality model`,
      `${context}.finality_model`,
    );
  }
  if (!SCCP_VERIFIER_TARGET_VALUES.has(verifierTarget)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.verifier_target must be a supported SCCP verifier target`,
      `${context}.verifier_target`,
    );
  }
  return {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    localDomain: ToriiClient._normalizeUnsignedInteger(record.local_domain, `${context}.local_domain`, {
      allowZero: true,
    }),
    localChain: requireNonEmptyString(record.local_chain, `${context}.local_chain`),
    counterpartyDomain: ToriiClient._normalizeUnsignedInteger(
      record.counterparty_domain,
      `${context}.counterparty_domain`,
      { allowZero: true },
    ),
    chain: requireNonEmptyString(record.chain, `${context}.chain`),
    proofFamily: requireNonEmptyString(record.proof_family, `${context}.proof_family`),
    messageBackend: requireNonEmptyString(record.message_backend, `${context}.message_backend`),
    registryBackend: requireNonEmptyString(record.registry_backend, `${context}.registry_backend`),
    counterpartyAccountCodec: ToriiClient._normalizeUnsignedInteger(
      record.counterparty_account_codec,
      `${context}.counterparty_account_codec`,
      { allowZero: false },
    ),
    counterpartyAccountCodecKey: requireNonEmptyString(
      record.counterparty_account_codec_key,
      `${context}.counterparty_account_codec_key`,
    ),
    finalityModel,
    verifierTarget,
    manifestSeed: requireNonEmptyString(record.manifest_seed, `${context}.manifest_seed`),
    requiredPublicInputs: parseStringArray(
      record.required_public_inputs,
      `${context}.required_public_inputs`,
    ).map((entry, index) =>
      requireNonEmptyString(entry, `${context}.required_public_inputs[${index}]`),
    ),
    messagePayloadKinds: parseStringArray(
      record.message_payload_kinds,
      `${context}.message_payload_kinds`,
    ).map((entry, index) =>
      requireNonEmptyString(entry, `${context}.message_payload_kinds[${index}]`),
    ),
  };
}

function normalizeSccpMessageTransparentProofArtifact(payload) {
  const context = "sccp message proof artifact response";
  const record = ensureRecord(payload, context);
  const finalityModel = requireNonEmptyString(record.finality_model, `${context}.finality_model`);
  const verifierTarget = requireNonEmptyString(
    record.verifier_target,
    `${context}.verifier_target`,
  );
  if (!SCCP_FINALITY_MODEL_VALUES.has(finalityModel)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.finality_model must be a supported SCCP finality model`,
      `${context}.finality_model`,
    );
  }
  if (!SCCP_VERIFIER_TARGET_VALUES.has(verifierTarget)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.verifier_target must be a supported SCCP verifier target`,
      `${context}.verifier_target`,
    );
  }
  const artifact = {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    localDomain: ToriiClient._normalizeUnsignedInteger(record.local_domain, `${context}.local_domain`, {
      allowZero: true,
    }),
    counterpartyDomain: ToriiClient._normalizeUnsignedInteger(
      record.counterparty_domain,
      `${context}.counterparty_domain`,
      { allowZero: true },
    ),
    proofFamily: requireNonEmptyString(record.proof_family, `${context}.proof_family`),
    messageBackend: requireNonEmptyString(record.message_backend, `${context}.message_backend`),
    registryBackend: requireNonEmptyString(record.registry_backend, `${context}.registry_backend`),
    manifestSeed: requireNonEmptyString(record.manifest_seed, `${context}.manifest_seed`),
    finalityModel,
    verifierTarget,
    publicInputs: normalizeSccpMessageTransparentPublicInputs(
      record.public_inputs,
      `${context}.public_inputs`,
    ),
    proofBytes: normalizeArbitraryHex(record.proof_bytes, `${context}.proof_bytes`),
    bundle: normalizeSccpMessageProofBundle(record.bundle, `${context}.bundle`),
  };
  if (artifact.bundle.commitment.messageId !== artifact.publicInputs.messageId) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment.message_id must match public_inputs.message_id`,
      `${context}.bundle.commitment.message_id`,
    );
  }
  if (artifact.bundle.commitment.payloadHash !== artifact.publicInputs.payloadHash) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment.payload_hash must match public_inputs.payload_hash`,
      `${context}.bundle.commitment.payload_hash`,
    );
  }
  if (artifact.bundle.commitmentRoot !== artifact.publicInputs.commitmentRoot) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment_root must match public_inputs.commitment_root`,
      `${context}.bundle.commitment_root`,
    );
  }
  return artifact;
}

function normalizeSccpCounterpartyProofJob(payload) {
  const context = "sccp message proof job response";
  const record = ensureRecord(payload, context);
  const chainFamily = requireNonEmptyString(record.chain_family, `${context}.chain_family`);
  const finalityModel = requireNonEmptyString(record.finality_model, `${context}.finality_model`);
  const verifierTarget = requireNonEmptyString(
    record.verifier_target,
    `${context}.verifier_target`,
  );
  if (!SCCP_CHAIN_FAMILY_VALUES.has(chainFamily)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.chain_family must be a supported SCCP chain family`,
      `${context}.chain_family`,
    );
  }
  if (!SCCP_FINALITY_MODEL_VALUES.has(finalityModel)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.finality_model must be a supported SCCP finality model`,
      `${context}.finality_model`,
    );
  }
  if (!SCCP_VERIFIER_TARGET_VALUES.has(verifierTarget)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.verifier_target must be a supported SCCP verifier target`,
      `${context}.verifier_target`,
    );
  }
  const job = {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    chainFamily,
    chain: requireNonEmptyString(record.chain, `${context}.chain`),
    localDomain: ToriiClient._normalizeUnsignedInteger(record.local_domain, `${context}.local_domain`, {
      allowZero: true,
    }),
    counterpartyDomain: ToriiClient._normalizeUnsignedInteger(
      record.counterparty_domain,
      `${context}.counterparty_domain`,
      { allowZero: true },
    ),
    proofFamily: requireNonEmptyString(record.proof_family, `${context}.proof_family`),
    messageBackend: requireNonEmptyString(record.message_backend, `${context}.message_backend`),
    registryBackend: requireNonEmptyString(record.registry_backend, `${context}.registry_backend`),
    manifestSeed: requireNonEmptyString(record.manifest_seed, `${context}.manifest_seed`),
    finalityModel,
    verifierTarget,
    publicInputs: normalizeSccpMessageTransparentPublicInputs(
      record.public_inputs,
      `${context}.public_inputs`,
    ),
    payloadKind: requireNonEmptyString(record.payload_kind, `${context}.payload_kind`),
    payloadProjection: normalizeSccpPayloadProjection(
      record.payload_projection,
      `${context}.payload_projection`,
    ),
    bundle: normalizeSccpMessageProofBundle(record.bundle, `${context}.bundle`),
  };
  if (job.bundle.commitment.messageId !== job.publicInputs.messageId) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment.message_id must match public_inputs.message_id`,
      `${context}.bundle.commitment.message_id`,
    );
  }
  if (job.bundle.commitment.payloadHash !== job.publicInputs.payloadHash) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment.payload_hash must match public_inputs.payload_hash`,
      `${context}.bundle.commitment.payload_hash`,
    );
  }
  if (job.bundle.commitmentRoot !== job.publicInputs.commitmentRoot) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bundle.commitment_root must match public_inputs.commitment_root`,
      `${context}.bundle.commitment_root`,
    );
  }
  return job;
}

function normalizeSccpPayloadProjection(value, context) {
  const record = ensureRecord(value, context);
  const entries = Object.entries(record);
  if (entries.length !== 1) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must contain exactly one SCCP payload projection variant`,
      context,
    );
  }
  const [[kind, body]] = entries;
  const variant = ensureRecord(body, `${context}.${kind}`);
  if (!SCCP_MESSAGE_PAYLOAD_KIND_VALUES.has(kind)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} has unsupported SCCP payload projection variant`,
      context,
    );
  }
  const payload = { ...variant };
  const numericFields = {
    AssetRegister: ["version", "target_domain", "home_domain", "nonce", "decimals"],
    RouteActivate: ["version", "source_domain", "target_domain", "nonce"],
    Transfer: [
      "version",
      "source_domain",
      "dest_domain",
      "nonce",
      "asset_home_domain",
      "amount",
    ],
  }[kind];
  for (const field of numericFields) {
    payload[field] = ToriiClient._normalizeUnsignedInteger(
      payload[field],
      `${context}.${kind}.${field}`,
      { allowZero: true },
    );
  }
  if (kind === "AssetRegister" || kind === "RouteActivate" || kind === "Transfer") {
    payload.asset_id = normalizeSccpNormalizedCodecValue(
      payload.asset_id,
      `${context}.${kind}.asset_id`,
    );
  }
  if (kind === "RouteActivate" || kind === "Transfer") {
    payload.route_id = normalizeSccpNormalizedCodecValue(
      payload.route_id,
      `${context}.${kind}.route_id`,
    );
  }
  if (kind === "Transfer") {
    payload.sender = normalizeSccpNormalizedCodecValue(
      payload.sender,
      `${context}.${kind}.sender`,
    );
    payload.recipient = normalizeSccpNormalizedCodecValue(
      payload.recipient,
      `${context}.${kind}.recipient`,
    );
  }
  return { kind, value: payload };
}

function normalizeSccpNormalizedCodecValue(value, context) {
  const record = ensureRecord(value, context);
  const entries = Object.entries(record);
  if (entries.length !== 1) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must contain exactly one SCCP codec variant`,
      context,
    );
  }
  const [[kind, body]] = entries;
  const variant = ensureRecord(body, `${context}.${kind}`);
  switch (kind) {
    case "TextUtf8":
      return {
        kind,
        value: requireNonEmptyString(variant.value, `${context}.${kind}.value`),
      };
    case "EvmHex":
    case "SolanaBase58":
      return {
        kind,
        bytes: normalizeSccpCodecScalarValue(variant.bytes, `${context}.${kind}.bytes`),
      };
    case "TonRaw":
      return {
        kind,
        workchain: coerceInteger(variant.workchain, `${context}.${kind}.workchain`),
        account: normalizeSccpCodecScalarValue(variant.account, `${context}.${kind}.account`),
      };
    case "TronBase58Check":
      return {
        kind,
        payload: normalizeSccpCodecScalarValue(variant.payload, `${context}.${kind}.payload`),
      };
    default:
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} has unsupported SCCP codec variant`,
        context,
      );
  }
}

function normalizeSccpCodecScalarValue(value, context) {
  if (Buffer.isBuffer(value)) {
    return value.toString("hex");
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength).toString("hex");
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value).toString("hex");
  }
  if (Array.isArray(value)) {
    return normalizeByteArray(value, context).toString("hex");
  }
  const text = requireNonEmptyString(value, context).trim();
  if (/^(0x)?[0-9a-fA-F]+$/.test(text)) {
    return normalizeArbitraryHex(text, context);
  }
  return text;
}

function normalizeSccpMessageTransparentPublicInputs(value, context) {
  const record = ensureRecord(value, context);
  return {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    messageId: normalizeHex32String(record.message_id, `${context}.message_id`),
    payloadHash: normalizeHex32String(record.payload_hash, `${context}.payload_hash`),
    targetDomain: ToriiClient._normalizeUnsignedInteger(record.target_domain, `${context}.target_domain`, {
      allowZero: true,
    }),
    commitmentRoot: normalizeHex32String(record.commitment_root, `${context}.commitment_root`),
    finalityHeight: ToriiClient._normalizeUnsignedInteger(
      record.finality_height,
      `${context}.finality_height`,
      { allowZero: true },
    ),
    finalityBlockHash: normalizeHex32String(
      record.finality_block_hash,
      `${context}.finality_block_hash`,
    ),
  };
}

function normalizeSccpMessageProofBundle(value, context) {
  const record = ensureRecord(value, context);
  return {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    commitmentRoot: normalizeHex32String(record.commitment_root, `${context}.commitment_root`),
    commitment: normalizeSccpHubCommitment(record.commitment, `${context}.commitment`),
    merkleProof: normalizeSccpMerkleProof(record.merkle_proof, `${context}.merkle_proof`),
    payload: normalizeSccpPayloadEnvelope(record.payload, `${context}.payload`),
    finalityProof: normalizeArbitraryHex(record.finality_proof, `${context}.finality_proof`),
  };
}

function normalizeSccpHubCommitment(value, context) {
  const record = ensureRecord(value, context);
  const kind = requireNonEmptyString(record.kind, `${context}.kind`);
  if (!SCCP_HUB_MESSAGE_KIND_VALUES.has(kind)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.kind must be a supported SCCP message kind`,
      `${context}.kind`,
    );
  }
  return {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    kind,
    targetDomain: ToriiClient._normalizeUnsignedInteger(record.target_domain, `${context}.target_domain`, {
      allowZero: true,
    }),
    messageId: normalizeHex32String(record.message_id, `${context}.message_id`),
    payloadHash: normalizeHex32String(record.payload_hash, `${context}.payload_hash`),
    parliamentCertificateHash:
      record.parliament_certificate_hash === undefined || record.parliament_certificate_hash === null
        ? null
        : normalizeHex32String(
            record.parliament_certificate_hash,
            `${context}.parliament_certificate_hash`,
          ),
  };
}

function normalizeSccpMerkleProof(value, context) {
  const record = ensureRecord(value, context);
  return {
    steps: parseRecordArray(record.steps, `${context}.steps`).map((entry, index) =>
      normalizeSccpMerkleStep(entry, `${context}.steps[${index}]`),
    ),
  };
}

function normalizeSccpMerkleStep(value, context) {
  const record = ensureRecord(value, context);
  if (typeof record.sibling_is_left !== "boolean") {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.sibling_is_left must be boolean`,
      `${context}.sibling_is_left`,
    );
  }
  return {
    siblingHash: normalizeHex32String(record.sibling_hash, `${context}.sibling_hash`),
    siblingIsLeft: record.sibling_is_left,
  };
}

function normalizeSccpPayloadEnvelope(value, context) {
  const record = ensureRecord(value, context);
  const keys = Object.keys(record);
  if (keys.length !== 1) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must contain exactly one SCCP payload variant`,
      context,
    );
  }
  const [kind] = keys;
  if (!SCCP_MESSAGE_PAYLOAD_KIND_VALUES.has(kind)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must use a supported SCCP payload variant`,
      context,
    );
  }
  return {
    kind,
    value: ensureRecord(record[kind], `${context}.${kind}`),
  };
}

function normalizeNodeSmCapabilities(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    enabled: coerceBoolean(record.enabled, `${context}.enabled`),
    defaultHash: optionalString(record.default_hash, `${context}.default_hash`),
    allowedSigning: parseStringArray(
      record.allowed_signing,
      `${context}.allowed_signing`,
    ),
    sm2DistIdDefault: optionalString(
      record.sm2_distid_default,
      `${context}.sm2_distid_default`,
    ),
    opensslPreview: coerceBoolean(record.openssl_preview ?? false, `${context}.openssl_preview`),
    acceleration: normalizeNodeSmAcceleration(record.acceleration, `${context}.acceleration`),
  };
}

function normalizeNodeSmAcceleration(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    scalar: coerceBoolean(record.scalar ?? true, `${context}.scalar`),
    neonSm3: coerceBoolean(record.neon_sm3 ?? false, `${context}.neon_sm3`),
    neonSm4: coerceBoolean(record.neon_sm4 ?? false, `${context}.neon_sm4`),
    policy: requireNonEmptyString(record.policy ?? "", `${context}.policy`),
  };
}

function normalizeNodeCurveCapabilities(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const rawVersion = record.registry_version;
  const registryVersion =
    rawVersion === undefined || rawVersion === null
      ? 1
      : ToriiClient._normalizeUnsignedInteger(rawVersion, `${context}.registry_version`, {
          allowZero: false,
        });
  const allowedRaw = record.allowed_curve_ids ?? [];
  const bitmapRaw = record.allowed_curve_bitmap ?? [];
  return {
    registryVersion,
    allowedCurveIds: parseIntegerArray(allowedRaw, `${context}.allowed_curve_ids`),
    allowedCurveBitmap: parseIntegerArray(bitmapRaw, `${context}.allowed_curve_bitmap`),
  };
}

function normalizeConfigurationSnapshot(payload) {
  const record = ensureRecord(payload, "configuration response");
  const publicKeyHex = requireNonEmptyString(
    record.public_key,
    "configuration response.public_key",
  );
  const logger = normalizeConfigurationLogger(record.logger, "configuration response.logger");
  const network = normalizeConfigurationNetwork(record.network, "configuration response.network");
  const queue = normalizeConfigurationQueue(record.queue, "configuration response.queue");
  const confidentialGas = normalizeConfigurationConfidentialGas(
    record.confidential_gas,
    "configuration response.confidential_gas",
  );
  const transport = normalizeConfigurationTransport(
    record.transport,
    "configuration response.transport",
  );
  const nexus = normalizeConfigurationNexus(record.nexus, "configuration response.nexus");
  return {
    publicKeyHex,
    logger,
    network,
    queue,
    confidentialGas,
    transport,
    nexus,
  };
}

function normalizeConfigurationLogger(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    level: requireNonEmptyString(record.level ?? "", `${context}.level`),
    filter: optionalString(record.filter ?? null, `${context}.filter`),
  };
}

function normalizeConfigurationNetwork(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    blockGossipSize: ToriiClient._normalizeUnsignedInteger(
      record.block_gossip_size,
      `${context}.block_gossip_size`,
    ),
    blockGossipPeriodMs: ToriiClient._normalizeUnsignedInteger(
      record.block_gossip_period_ms,
      `${context}.block_gossip_period_ms`,
    ),
    transactionGossipSize: ToriiClient._normalizeUnsignedInteger(
      record.transaction_gossip_size,
      `${context}.transaction_gossip_size`,
    ),
    transactionGossipPeriodMs: ToriiClient._normalizeUnsignedInteger(
      record.transaction_gossip_period_ms,
      `${context}.transaction_gossip_period_ms`,
    ),
  };
}

function normalizeConfigurationQueue(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  return {
    capacity: ToriiClient._normalizeUnsignedInteger(record.capacity, `${context}.capacity`, {
      allowZero: false,
    }),
  };
}

function normalizeConfigurationConfidentialGas(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  return {
    proofBase: ToriiClient._normalizeUnsignedInteger(
      record.proof_base,
      `${context}.proof_base`,
      { allowZero: true },
    ),
    perPublicInput: ToriiClient._normalizeUnsignedInteger(
      record.per_public_input,
      `${context}.per_public_input`,
      { allowZero: true },
    ),
    perProofByte: ToriiClient._normalizeUnsignedInteger(
      record.per_proof_byte,
      `${context}.per_proof_byte`,
      { allowZero: true },
    ),
    perNullifier: ToriiClient._normalizeUnsignedInteger(
      record.per_nullifier,
      `${context}.per_nullifier`,
      { allowZero: true },
    ),
    perCommitment: ToriiClient._normalizeUnsignedInteger(
      record.per_commitment,
      `${context}.per_commitment`,
      { allowZero: true },
    ),
  };
}

function normalizeConfigurationTransport(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  const noritoRpc = normalizeConfigurationTransportNoritoRpc(
    record.norito_rpc,
    `${context}.norito_rpc`,
  );
  const streaming = normalizeConfigurationTransportStreaming(
    record.streaming,
    `${context}.streaming`,
  );
  return { noritoRpc, streaming };
}

function normalizeConfigurationTransportNoritoRpc(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  return {
    enabled: coerceBoolean(record.enabled ?? false, `${context}.enabled`),
    stage: requireNonEmptyString(record.stage ?? "", `${context}.stage`),
    requireMtls: coerceBoolean(
      record.require_mtls ?? false,
      `${context}.require_mtls`,
    ),
    canaryAllowlistSize: ToriiClient._normalizeUnsignedInteger(
      record.canary_allowlist_size ?? 0,
      `${context}.canary_allowlist_size`,
      { allowZero: true },
    ),
  };
}

function normalizeConfigurationTransportStreaming(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  const soranet = normalizeConfigurationStreamingSoranet(
    record.soranet,
    `${context}.soranet`,
  );
  return { soranet };
}

function normalizeConfigurationStreamingSoranet(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const paddingRaw = record.padding_budget_ms ?? null;
  const paddingBudgetMs =
    paddingRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          paddingRaw,
          `${context}.padding_budget_ms`,
          { allowZero: true },
        );
  return {
    enabled: coerceBoolean(record.enabled ?? false, `${context}.enabled`),
    streamTag: requireNonEmptyString(
      record.stream_tag ?? "",
      `${context}.stream_tag`,
    ),
    exitMultiaddr: requireNonEmptyString(
      record.exit_multiaddr ?? "",
      `${context}.exit_multiaddr`,
    ),
    paddingBudgetMs,
    accessKind: requireNonEmptyString(
      record.access_kind ?? "",
      `${context}.access_kind`,
    ),
    garCategory: requireNonEmptyString(
      record.gar_category ?? "",
      `${context}.gar_category`,
    ),
    channelSalt: requireNonEmptyString(
      record.channel_salt ?? "",
      `${context}.channel_salt`,
    ),
    provisionSpoolDir: requireNonEmptyString(
      record.provision_spool_dir ?? "",
      `${context}.provision_spool_dir`,
    ),
    provisionWindowSegments: ToriiClient._normalizeUnsignedInteger(
      record.provision_window_segments,
      `${context}.provision_window_segments`,
      { allowZero: false },
    ),
    provisionQueueCapacity: ToriiClient._normalizeUnsignedInteger(
      record.provision_queue_capacity,
      `${context}.provision_queue_capacity`,
      { allowZero: false },
    ),
  };
}

function normalizeConfigurationNexus(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  const axt = normalizeConfigurationNexusAxt(record.axt, `${context}.axt`);
  return { axt };
}

function normalizeConfigurationNexusAxt(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const slotLengthMs = ToriiClient._normalizeUnsignedInteger(
    record.slot_length_ms,
    `${context}.slot_length_ms`,
    { allowZero: false },
  );
  const maxClockSkewMs = ToriiClient._normalizeUnsignedInteger(
    record.max_clock_skew_ms ?? 0,
    `${context}.max_clock_skew_ms`,
    { allowZero: true },
  );
  const proofCacheTtlSlots = ToriiClient._normalizeUnsignedInteger(
    record.proof_cache_ttl_slots,
    `${context}.proof_cache_ttl_slots`,
    { allowZero: false },
  );
  const replayRetentionSlots = ToriiClient._normalizeUnsignedInteger(
    record.replay_retention_slots,
    `${context}.replay_retention_slots`,
    { allowZero: false },
  );
  return {
    slotLengthMs,
    maxClockSkewMs,
    proofCacheTtlSlots,
    replayRetentionSlots,
  };
}

function normalizeRuntimeAbiActiveResponse(payload) {
  const record = ensureRecord(payload, "runtime abi active response");
  return {
    abiVersion: ToriiClient._normalizeUnsignedInteger(
      record.abi_version,
      "runtime abi active response.abi_version",
      { allowZero: false },
    ),
  };
}

function normalizeRuntimeAbiHashResponse(payload) {
  const record = ensureRecord(payload, "runtime abi hash response");
  return {
    policy: requireNonEmptyString(record.policy, "runtime abi hash response.policy"),
    abiHashHex: requireHexString(record.abi_hash_hex ?? "", "runtime abi hash response.abi_hash_hex"),
  };
}

function normalizeRuntimeMetricsResponse(payload) {
  const record = ensureRecord(payload, "runtime metrics response");
  const counters = ensureRecord(
    record.upgrade_events_total ?? {},
    "runtime metrics response.upgrade_events_total",
  );
  return {
    abiVersion: ToriiClient._normalizeUnsignedInteger(
      record.abi_version,
      "runtime metrics response.abi_version",
      { allowZero: false },
    ),
    upgradeEventsTotal: {
      proposed: ToriiClient._normalizeUnsignedInteger(
        counters.proposed ?? 0,
        "runtime metrics response.upgrade_events_total.proposed",
        { allowZero: true },
      ),
      activated: ToriiClient._normalizeUnsignedInteger(
        counters.activated ?? 0,
        "runtime metrics response.upgrade_events_total.activated",
        { allowZero: true },
      ),
      canceled: ToriiClient._normalizeUnsignedInteger(
        counters.canceled ?? 0,
        "runtime metrics response.upgrade_events_total.canceled",
        { allowZero: true },
      ),
    },
  };
}

function normalizeExplorerMetricsResponse(payload) {
  const record = ensureRecord(payload ?? {}, "explorer metrics response");
  const avgCommit = normalizeExplorerDurationMs(
    record.avg_commit_time,
    "explorer metrics response.avg_commit_time",
  );
  const avgBlock = normalizeExplorerDurationMs(
    record.avg_block_time,
    "explorer metrics response.avg_block_time",
  );
  return {
    peers: ToriiClient._normalizeUnsignedInteger(record.peers ?? 0, "explorer metrics response.peers", {
      allowZero: true,
    }),
    domains: ToriiClient._normalizeUnsignedInteger(
      record.domains ?? 0,
      "explorer metrics response.domains",
      { allowZero: true },
    ),
    accounts: ToriiClient._normalizeUnsignedInteger(
      record.accounts ?? 0,
      "explorer metrics response.accounts",
      { allowZero: true },
    ),
    assets: ToriiClient._normalizeUnsignedInteger(
      record.assets ?? 0,
      "explorer metrics response.assets",
      { allowZero: true },
    ),
    transactionsAccepted: ToriiClient._normalizeUnsignedInteger(
      record.transactions_accepted ?? 0,
      "explorer metrics response.transactions_accepted",
      { allowZero: true },
    ),
    transactionsRejected: ToriiClient._normalizeUnsignedInteger(
      record.transactions_rejected ?? 0,
      "explorer metrics response.transactions_rejected",
      { allowZero: true },
    ),
    blockHeight: ToriiClient._normalizeUnsignedInteger(
      record.block ?? 0,
      "explorer metrics response.block",
      { allowZero: true },
    ),
    blockCreatedAt: optionalString(
      record.block_created_at ?? null,
      "explorer metrics response.block_created_at",
    ),
    finalizedBlockHeight: ToriiClient._normalizeUnsignedInteger(
      record.finalized_block ?? 0,
      "explorer metrics response.finalized_block",
      { allowZero: true },
    ),
    averageCommitTimeMs: avgCommit ?? null,
    averageBlockTimeMs: avgBlock ?? null,
  };
}

const EXPLORER_ACCOUNT_QR_OPTION_KEYS = new Set(["signal"]);
const VPN_SESSION_OPTION_KEYS = new Set(["signal", "canonicalAuth"]);

function normalizeExplorerRequestOptions(options) {
  if (options === undefined) {
    return { signal: undefined };
  }
  const record = ensureRecord(options, "getExplorerAccountQr options");
  assertSupportedOptionKeys(
    record,
    EXPLORER_ACCOUNT_QR_OPTION_KEYS,
    "getExplorerAccountQr options",
  );
  const { signal } = normalizeSignalOption(record, "getExplorerAccountQr");
  return { signal };
}

function normalizeVpnProfileResponse(payload) {
  const record = ensureRecord(payload ?? {}, "vpn profile response");
  return {
    available: coerceBoolean(record.available ?? false, "vpn profile response.available"),
    relayEndpoint: requireNonEmptyString(
      record.relay_endpoint ?? "",
      "vpn profile response.relay_endpoint",
    ),
    supportedExitClasses: normalizeStringArray(
      record.supported_exit_classes ?? [],
      "vpn profile response.supported_exit_classes",
    ),
    defaultExitClass: requireNonEmptyString(
      record.default_exit_class ?? "",
      "vpn profile response.default_exit_class",
    ),
    leaseSecs: ToriiClient._normalizeUnsignedInteger(
      record.lease_secs ?? 0,
      "vpn profile response.lease_secs",
      { allowZero: true },
    ),
    dnsPushIntervalSecs: ToriiClient._normalizeUnsignedInteger(
      record.dns_push_interval_secs ?? 0,
      "vpn profile response.dns_push_interval_secs",
      { allowZero: true },
    ),
    meterFamily: requireNonEmptyString(
      record.meter_family ?? "",
      "vpn profile response.meter_family",
    ),
    routePushes: requireStringArray(
      record.route_pushes ?? [],
      "vpn profile response.route_pushes",
    ),
    excludedRoutes: requireStringArray(
      record.excluded_routes ?? [],
      "vpn profile response.excluded_routes",
    ),
    dnsServers: requireStringArray(
      record.dns_servers ?? [],
      "vpn profile response.dns_servers",
    ),
    tunnelAddresses: requireStringArray(
      record.tunnel_addresses ?? [],
      "vpn profile response.tunnel_addresses",
    ),
    mtuBytes: ToriiClient._normalizeUnsignedInteger(
      record.mtu_bytes ?? 0,
      "vpn profile response.mtu_bytes",
      { allowZero: true },
    ),
    displayBillingLabel: requireNonEmptyString(
      record.display_billing_label ?? "",
      "vpn profile response.display_billing_label",
    ),
  };
}

function normalizeVpnSessionResponse(payload, context = "vpn session response") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    sessionId: requireNonEmptyString(record.session_id ?? "", `${context}.session_id`),
    accountId: requireNonEmptyString(record.account_id ?? "", `${context}.account_id`),
    exitClass: requireNonEmptyString(record.exit_class ?? "", `${context}.exit_class`),
    relayEndpoint: requireNonEmptyString(
      record.relay_endpoint ?? "",
      `${context}.relay_endpoint`,
    ),
    leaseSecs: ToriiClient._normalizeUnsignedInteger(
      record.lease_secs ?? 0,
      `${context}.lease_secs`,
      { allowZero: true },
    ),
    expiresAtMs: ToriiClient._normalizeUnsignedInteger(
      record.expires_at_ms ?? 0,
      `${context}.expires_at_ms`,
      { allowZero: true },
    ),
    connectedAtMs: ToriiClient._normalizeUnsignedInteger(
      record.connected_at_ms ?? 0,
      `${context}.connected_at_ms`,
      { allowZero: true },
    ),
    meterFamily: requireNonEmptyString(record.meter_family ?? "", `${context}.meter_family`),
    routePushes: requireStringArray(record.route_pushes ?? [], `${context}.route_pushes`),
    excludedRoutes: requireStringArray(
      record.excluded_routes ?? [],
      `${context}.excluded_routes`,
    ),
    dnsServers: requireStringArray(record.dns_servers ?? [], `${context}.dns_servers`),
    tunnelAddresses: requireStringArray(
      record.tunnel_addresses ?? [],
      `${context}.tunnel_addresses`,
    ),
    mtuBytes: ToriiClient._normalizeUnsignedInteger(
      record.mtu_bytes ?? 0,
      `${context}.mtu_bytes`,
      { allowZero: true },
    ),
    helperTicketHex: requireHexString(
      record.helper_ticket_hex ?? "",
      `${context}.helper_ticket_hex`,
    ),
    bytesIn: ToriiClient._normalizeUnsignedInteger(
      record.bytes_in ?? 0,
      `${context}.bytes_in`,
      { allowZero: true },
    ),
    bytesOut: ToriiClient._normalizeUnsignedInteger(
      record.bytes_out ?? 0,
      `${context}.bytes_out`,
      { allowZero: true },
    ),
    status: requireNonEmptyString(record.status ?? "", `${context}.status`),
  };
}

function normalizeVpnReceiptResponse(payload, context = "vpn receipt response") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    sessionId: requireNonEmptyString(record.session_id ?? "", `${context}.session_id`),
    accountId: requireNonEmptyString(record.account_id ?? "", `${context}.account_id`),
    exitClass: requireNonEmptyString(record.exit_class ?? "", `${context}.exit_class`),
    relayEndpoint: requireNonEmptyString(
      record.relay_endpoint ?? "",
      `${context}.relay_endpoint`,
    ),
    meterFamily: requireNonEmptyString(record.meter_family ?? "", `${context}.meter_family`),
    connectedAtMs: ToriiClient._normalizeUnsignedInteger(
      record.connected_at_ms ?? 0,
      `${context}.connected_at_ms`,
      { allowZero: true },
    ),
    disconnectedAtMs: ToriiClient._normalizeUnsignedInteger(
      record.disconnected_at_ms ?? 0,
      `${context}.disconnected_at_ms`,
      { allowZero: true },
    ),
    durationMs: ToriiClient._normalizeUnsignedInteger(
      record.duration_ms ?? 0,
      `${context}.duration_ms`,
      { allowZero: true },
    ),
    bytesIn: ToriiClient._normalizeUnsignedInteger(
      record.bytes_in ?? 0,
      `${context}.bytes_in`,
      { allowZero: true },
    ),
    bytesOut: ToriiClient._normalizeUnsignedInteger(
      record.bytes_out ?? 0,
      `${context}.bytes_out`,
      { allowZero: true },
    ),
    status: requireNonEmptyString(record.status ?? "", `${context}.status`),
    receiptSource: requireNonEmptyString(
      record.receipt_source ?? "",
      `${context}.receipt_source`,
    ),
  };
}

function normalizeVpnReceiptListResponse(payload) {
  const record = ensureRecord(payload ?? {}, "vpn receipts response");
  if (!Array.isArray(record.items)) {
    throw new TypeError("vpn receipts response.items must be an array");
  }
  const items = record.items.map((item, index) =>
    normalizeVpnReceiptResponse(item, `vpn receipts response.items[${index}]`),
  );
  return items;
}

function normalizeVpnSessionOptions(options, context) {
  if (options === undefined) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options.canonicalAuth is required`,
      `${context}.canonicalAuth`,
    );
  }
  const record = ensureRecord(options, `${context} options`);
  assertSupportedOptionKeys(record, VPN_SESSION_OPTION_KEYS, `${context} options`);
  const { signal } = normalizeSignalOption(record, context);
  const canonicalAuth = ToriiClient._normalizeCanonicalAuth(
    record.canonicalAuth,
    `${context}.canonicalAuth`,
  );
  if (!canonicalAuth) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options.canonicalAuth is required`,
      `${context}.canonicalAuth`,
    );
  }
  return { signal, canonicalAuth };
}

function normalizeSnsSuffixPolicy(payload) {
  const record = ensureRecord(payload ?? {}, "sns suffix policy");
  const suffixId = ToriiClient._normalizeUnsignedInteger(record.suffix_id, "sns suffix policy.suffix_id", {
    allowZero: true,
  });
  const suffix = requireNonEmptyString(record.suffix, "sns suffix policy.suffix").toLowerCase();
  const steward = ToriiClient._normalizeAccountId(record.steward, "sns suffix policy.steward");
  const statusRaw = requireNonEmptyString(record.status, "sns suffix policy.status");
  if (!SNS_SUFFIX_STATUS_VALUES.has(statusRaw)) {
    throw new TypeError(`sns suffix policy.status must be one of ${Array.from(SNS_SUFFIX_STATUS_VALUES).join(", ")}`);
  }
  const minTermYears = ToriiClient._normalizeUnsignedInteger(record.min_term_years, "sns suffix policy.min_term_years");
  const maxTermYears = ToriiClient._normalizeUnsignedInteger(record.max_term_years, "sns suffix policy.max_term_years");
  const graceDays = ToriiClient._normalizeUnsignedInteger(record.grace_period_days, "sns suffix policy.grace_period_days", {
    allowZero: true,
  });
  const redemptionDays = ToriiClient._normalizeUnsignedInteger(
    record.redemption_period_days,
    "sns suffix policy.redemption_period_days",
    { allowZero: true },
  );
  const referralCap = ToriiClient._normalizeUnsignedInteger(
    record.referral_cap_bps,
    "sns suffix policy.referral_cap_bps",
    { allowZero: true },
  );
  const reservedLabels = Array.isArray(record.reserved_labels)
    ? record.reserved_labels.map((entry, index) => normalizeReservedLabel(entry, `sns suffix policy.reserved_labels[${index}]`))
    : [];
  const paymentAssetId = requireNonEmptyString(record.payment_asset_id, "sns suffix policy.payment_asset_id");
  const pricing = Array.isArray(record.pricing)
    ? record.pricing.map((entry, index) => normalizeSnsPricingTier(entry, `sns suffix policy.pricing[${index}]`))
    : [];
  const feeSplit = normalizeSnsFeeSplit(record.fee_split, "sns suffix policy.fee_split");
  const fundSplitter = ToriiClient._normalizeAccountId(
    record.fund_splitter_account,
    "sns suffix policy.fund_splitter_account",
  );
  const policyVersion = ToriiClient._normalizeUnsignedInteger(
    record.policy_version,
    "sns suffix policy.policy_version",
    { allowZero: true },
  );
  const metadata = normalizeOptionalMetadata(record.metadata, "sns suffix policy.metadata");
  return {
    suffixId,
    suffix,
    steward,
    status: statusRaw,
    minTermYears,
    maxTermYears,
    gracePeriodDays: graceDays,
    redemptionPeriodDays: redemptionDays,
    referralCapBps: referralCap,
    reservedLabels,
    paymentAssetId,
    pricing,
    feeSplit,
    fundSplitterAccount: fundSplitter,
    policyVersion,
    metadata,
  };
}

function normalizeReservedLabel(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const normalizedLabel = requireNonEmptyString(record.normalized_label, `${context}.normalized_label`).toLowerCase();
  const assignedTo =
    record.assigned_to === undefined || record.assigned_to === null
      ? null
      : ToriiClient._normalizeAccountId(record.assigned_to, `${context}.assigned_to`);
  const releaseAtMs =
    record.release_at_ms === undefined || record.release_at_ms === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(record.release_at_ms, `${context}.release_at_ms`, { allowZero: true });
  const note = requireNonEmptyString(record.note ?? "", `${context}.note`);
  return {
    normalizedLabel,
    assignedTo,
    releaseAtMs,
    note,
  };
}

function normalizeSnsTokenValue(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const assetId = ToriiClient._normalizeAssetId(record.asset_id, `${context}.asset_id`);
  const amount = normalizeNumericString(record.amount, `${context}.amount`);
  return { assetId, amount };
}

function normalizeSnsPricingTier(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const tierId = ToriiClient._normalizeUnsignedInteger(record.tier_id, `${context}.tier_id`, { allowZero: true });
  const regex = requireNonEmptyString(record.label_regex, `${context}.label_regex`);
  const basePrice = normalizeSnsTokenValue(record.base_price, `${context}.base_price`);
  const auctionKindRaw = requireNonEmptyString(record.auction_kind, `${context}.auction_kind`);
  if (!SNS_AUCTION_KIND_VALUES.has(auctionKindRaw)) {
    throw new TypeError(`${context}.auction_kind must be one of ${Array.from(SNS_AUCTION_KIND_VALUES).join(", ")}`);
  }
  const dutchFloor =
    record.dutch_floor === undefined || record.dutch_floor === null
      ? null
      : normalizeSnsTokenValue(record.dutch_floor, `${context}.dutch_floor`);
  const minDurationYears = ToriiClient._normalizeUnsignedInteger(
    record.min_duration_years,
    `${context}.min_duration_years`,
    { allowZero: true },
  );
  const maxDurationYears = ToriiClient._normalizeUnsignedInteger(
    record.max_duration_years,
    `${context}.max_duration_years`,
    { allowZero: true },
  );
  return {
    tierId,
    labelRegex: regex,
    basePrice,
    auctionKind: auctionKindRaw,
    dutchFloor,
    minDurationYears,
    maxDurationYears,
  };
}

function normalizeSnsFeeSplit(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    treasuryBps: ToriiClient._normalizeUnsignedInteger(record.treasury_bps, `${context}.treasury_bps`, {
      allowZero: true,
    }),
    stewardBps: ToriiClient._normalizeUnsignedInteger(record.steward_bps, `${context}.steward_bps`, {
      allowZero: true,
    }),
    referralMaxBps: ToriiClient._normalizeUnsignedInteger(
      record.referral_max_bps,
      `${context}.referral_max_bps`,
      { allowZero: true },
    ),
    escrowBps: ToriiClient._normalizeUnsignedInteger(record.escrow_bps, `${context}.escrow_bps`, { allowZero: true }),
  };
}

function normalizeSnsRegisterRequest(payload, context) {
  const record = ensureRecord(payload ?? {}, `${context} payload`);
  const selector = normalizeSnsSelector(record.selector, `${context}.selector`);
  const controllers = normalizeSnsControllers(record.controllers, `${context}.controllers`);
  const owner = ToriiClient._normalizeAccountId(record.owner, `${context}.owner`);
  const termYears =
    record.term_years === undefined
      ? ToriiClient._normalizeUnsignedInteger(1, `${context}.term_years`)
      : ToriiClient._normalizeUnsignedInteger(record.term_years, `${context}.term_years`);
  const pricingClassHint =
    record.pricing_class_hint === undefined || record.pricing_class_hint === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(record.pricing_class_hint, `${context}.pricing_class_hint`, {
          allowZero: true,
        });
  const payment = normalizeSnsPayment(record.payment, `${context}.payment`);
  const governance =
    record.governance === undefined || record.governance === null
      ? null
      : normalizeSnsGovernanceHook(record.governance, `${context}.governance`);
  const metadata = normalizeOptionalMetadata(record.metadata, `${context}.metadata`);
  return {
    selector,
    owner,
    controllers,
    term_years: termYears,
    pricing_class_hint: pricingClassHint ?? undefined,
    payment,
    governance: governance ?? undefined,
    metadata,
  };
}

function normalizeSnsRenewRequest(payload, context) {
  const record = ensureRecord(payload ?? {}, `${context} payload`);
  const termYears = ToriiClient._normalizeUnsignedInteger(record.term_years, `${context}.term_years`);
  const payment = normalizeSnsPayment(record.payment, `${context}.payment`);
  return {
    term_years: termYears,
    payment,
  };
}

function normalizeSnsTransferRequest(payload, context) {
  const record = ensureRecord(payload ?? {}, `${context} payload`);
  const newOwner = ToriiClient._normalizeAccountId(record.new_owner, `${context}.new_owner`);
  const governance = normalizeSnsGovernanceHook(record.governance, `${context}.governance`);
  return {
    new_owner: newOwner,
    governance,
  };
}

function normalizeSnsFreezeRequest(payload, context) {
  const record = ensureRecord(payload ?? {}, `${context} payload`);
  const reason = requireNonEmptyString(record.reason, `${context}.reason`);
  const untilMs = ToriiClient._normalizeUnsignedInteger(record.until_ms, `${context}.until_ms`, { allowZero: true });
  const guardianTicket = record.guardian_ticket;
  if (guardianTicket === undefined || guardianTicket === null) {
    throw new TypeError(`${context}.guardian_ticket must be provided`);
  }
  return {
    reason,
    until_ms: untilMs,
    guardian_ticket: guardianTicket,
  };
}

function normalizeSnsPayment(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const assetId = ToriiClient._normalizeAssetId(
    record.asset_id ?? record.assetId,
    `${context}.asset_id`,
  );
  const gross = ToriiClient._normalizeUnsignedInteger(
    record.gross_amount ?? record.grossAmount,
    `${context}.gross_amount`,
  );
  const netRaw = record.net_amount ?? record.netAmount;
  const settlementTx = record.settlement_tx ?? record.settlementTx;
  if (settlementTx === undefined || settlementTx === null) {
    throw new TypeError(`${context}.settlement_tx must be provided`);
  }
  const payer = ToriiClient._normalizeAccountId(
    record.payer,
    `${context}.payer`,
  );
  const signature = record.signature;
  if (signature === undefined || signature === null) {
    throw new TypeError(`${context}.signature must be provided`);
  }
  const payment = {
    asset_id: assetId,
    gross_amount: gross,
    settlement_tx: settlementTx,
    payer,
    signature,
  };
  if (netRaw !== undefined && netRaw !== null) {
    payment.net_amount = ToriiClient._normalizeUnsignedInteger(
      netRaw,
      `${context}.net_amount`,
      { allowZero: true },
    );
  }
  return payment;
}

function normalizeSnsGovernanceHook(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const proposalId = requireNonEmptyString(record.proposal_id, `${context}.proposal_id`);
  const councilVoteHash = requireNonEmptyString(record.council_vote_hash, `${context}.council_vote_hash`);
  const daoVoteHash = requireNonEmptyString(record.dao_vote_hash, `${context}.dao_vote_hash`);
  const stewardAck = requireNonEmptyString(record.steward_ack, `${context}.steward_ack`);
  const guardianClearance =
    record.guardian_clearance === undefined || record.guardian_clearance === null
      ? null
      : requireNonEmptyString(record.guardian_clearance, `${context}.guardian_clearance`);
  return {
    proposal_id: proposalId,
    council_vote_hash: councilVoteHash,
    dao_vote_hash: daoVoteHash,
    steward_ack: stewardAck,
    guardian_clearance: guardianClearance ?? undefined,
  };
}

function normalizeSnsSelector(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const version = ToriiClient._normalizeUnsignedInteger(record.version ?? 1, `${context}.version`, { allowZero: true });
  const suffixId = ToriiClient._normalizeUnsignedInteger(record.suffix_id, `${context}.suffix_id`, { allowZero: true });
  const label = requireNonEmptyString(record.label, `${context}.label`);
  return {
    version,
    suffix_id: suffixId,
    label,
  };
}

function normalizeSnsControllers(value, context) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of controllers`);
  }
  return value.map((entry, index) => normalizeSnsController(entry, `${context}[${index}]`));
}

function normalizeSnsController(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const kind = requireNonEmptyString(record.controller_type, `${context}.controller_type`);
  if (!SNS_CONTROLLER_TYPES.has(kind)) {
    throw new TypeError(`${context}.controller_type must be one of ${Array.from(SNS_CONTROLLER_TYPES).join(", ")}`);
  }
  const accountAddress =
    record.account_address === undefined || record.account_address === null
      ? null
      : requireNonEmptyString(record.account_address, `${context}.account_address`);
  const resolverTemplateId =
    record.resolver_template_id === undefined || record.resolver_template_id === null
      ? null
      : requireNonEmptyString(record.resolver_template_id, `${context}.resolver_template_id`);
  const payloadMetadata = normalizeOptionalMetadata(record.payload, `${context}.payload`);
  return {
    controller_type: kind,
    account_address: accountAddress ?? undefined,
    resolver_template_id: resolverTemplateId ?? undefined,
    payload: payloadMetadata,
  };
}

function normalizeSnsNameRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const selector = normalizeSnsSelector(record.selector, `${context}.selector`);
  const nameHash = normalizeHex32String(record.name_hash, `${context}.name_hash`);
  const owner = ToriiClient._normalizeAccountId(record.owner, `${context}.owner`);
  const controllers = normalizeSnsControllers(record.controllers, `${context}.controllers`);
  const status = normalizeSnsNameStatus(record.status, `${context}.status`);
  const pricingClass = ToriiClient._normalizeUnsignedInteger(
    record.pricing_class,
    `${context}.pricing_class`,
    { allowZero: true },
  );
  const registeredAtMs = ToriiClient._normalizeUnsignedInteger(record.registered_at_ms, `${context}.registered_at_ms`, {
    allowZero: true,
  });
  const expiresAtMs = ToriiClient._normalizeUnsignedInteger(record.expires_at_ms, `${context}.expires_at_ms`, {
    allowZero: true,
  });
  const graceExpiresAtMs = ToriiClient._normalizeUnsignedInteger(
    record.grace_expires_at_ms,
    `${context}.grace_expires_at_ms`,
    { allowZero: true },
  );
  const redemptionExpiresAtMs = ToriiClient._normalizeUnsignedInteger(
    record.redemption_expires_at_ms,
    `${context}.redemption_expires_at_ms`,
    { allowZero: true },
  );
  const metadata = normalizeOptionalMetadata(record.metadata, `${context}.metadata`);
  const auction =
    record.auction === undefined || record.auction === null
      ? null
      : normalizeSnsAuction(record.auction, `${context}.auction`);
  return {
    selector,
    nameHash,
    owner,
    controllers,
    status,
    pricingClass,
    registeredAtMs,
    expiresAtMs,
    graceExpiresAtMs,
    redemptionExpiresAtMs,
    metadata,
    auction,
  };
}

function normalizeSnsAuction(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const kindRaw = requireNonEmptyString(record.kind, `${context}.kind`);
  if (!SNS_AUCTION_KIND_VALUES.has(kindRaw)) {
    throw new TypeError(`${context}.kind must be one of ${Array.from(SNS_AUCTION_KIND_VALUES).join(", ")}`);
  }
  const openedAtMs = ToriiClient._normalizeUnsignedInteger(record.opened_at_ms, `${context}.opened_at_ms`, {
    allowZero: true,
  });
  const closesAtMs = ToriiClient._normalizeUnsignedInteger(record.closes_at_ms, `${context}.closes_at_ms`, {
    allowZero: true,
  });
  const floorPrice = normalizeSnsTokenValue(record.floor_price, `${context}.floor_price`);
  const highestCommitment =
    record.highest_commitment === undefined || record.highest_commitment === null
      ? null
      : normalizeHex32String(record.highest_commitment, `${context}.highest_commitment`);
  const settlementTx =
    record.settlement_tx === undefined || record.settlement_tx === null
      ? null
      : record.settlement_tx;
  return {
    kind: kindRaw,
    openedAtMs,
    closesAtMs,
    floorPrice,
    highestCommitment,
    settlementTx,
  };
}

function normalizeSnsNameStatus(payload, context) {
  if (typeof payload === "string") {
    if (!SNS_NAME_STATUS_VALUES.has(payload)) {
      throw new TypeError(`${context} must be one of ${Array.from(SNS_NAME_STATUS_VALUES).join(", ")}`);
    }
    return { status: payload };
  }
  const record = ensureRecord(payload ?? {}, context);
  const status = requireNonEmptyString(record.status ?? "", `${context}.status`);
  if (!SNS_NAME_STATUS_VALUES.has(status)) {
    throw new TypeError(`${context}.status must be one of ${Array.from(SNS_NAME_STATUS_VALUES).join(", ")}`);
  }
  if (status === "Frozen") {
    const frozen = ensureRecord(record.detail ?? record, `${context}.frozen`);
    return {
      status,
      reason: requireNonEmptyString(
        frozen.reason,
        `${context}.frozen.reason`,
      ),
      untilMs: ToriiClient._normalizeUnsignedInteger(
        frozen.until_ms ?? frozen.untilMs,
        `${context}.frozen.until_ms`,
        {
        allowZero: true,
      },
      ),
    };
  }
  if (status === "Tombstoned") {
    const tombstone = ensureRecord(record.detail ?? record, `${context}.tombstone`);
    return {
      status,
      reason: requireNonEmptyString(tombstone.reason, `${context}.tombstone.reason`),
    };
  }
  return { status };
}

function normalizeSnsRegisterResponse(payload) {
  const record = ensureRecord(payload ?? {}, "sns register response");
  return {
    nameRecord: normalizeSnsNameRecord(record.name_record, "sns register response.name_record"),
  };
}

function normalizeSnsGovernanceCaseSelectorInput(selector, context) {
  if (selector === undefined || selector === null) {
    return undefined;
  }
  if (typeof selector === "string") {
    return requireNonEmptyString(selector, context);
  }
  const normalized = normalizeSnsGovernanceCaseSelector(selector, context);
  return {
    suffix_id: normalized.suffixId,
    label: normalized.label,
    global_form: normalized.globalForm,
  };
}

function normalizeSnsGovernanceCaseCreatePayload(payload) {
  const record = ensureRecord(payload ?? {}, "createSnsGovernanceCase payload");
  const normalized = {};
  const handledKeys = new Set();

  const selector = normalizeSnsGovernanceCaseSelectorInput(
    record.selector,
    "createSnsGovernanceCase payload.selector",
  );
  if (selector !== undefined) {
    handledKeys.add("selector");
    normalized.selector = selector;
  }

  const disputeType = record.dispute_type ?? record.disputeType;
  if (disputeType !== undefined && disputeType !== null) {
    handledKeys.add("dispute_type");
    handledKeys.add("disputeType");
    normalized.dispute_type = normalizeSnsGovernanceCaseDisputeType(
      disputeType,
      "createSnsGovernanceCase payload.dispute_type",
    );
  }

  const priority = record.priority;
  if (priority !== undefined && priority !== null) {
    handledKeys.add("priority");
    normalized.priority = normalizeSnsGovernanceCasePriority(
      priority,
      "createSnsGovernanceCase payload.priority",
    );
  }

  const status = record.status;
  if (status !== undefined && status !== null) {
    handledKeys.add("status");
    normalized.status = normalizeSnsGovernanceCaseStatus(
      status,
      "createSnsGovernanceCase payload.status",
    );
  }

  if (record.reason !== undefined && record.reason !== null) {
    handledKeys.add("reason");
    normalized.reason = requireNonEmptyString(
      record.reason,
      "createSnsGovernanceCase payload.reason",
    );
  }

  const reporterRaw = record.reporter;
  if (reporterRaw !== undefined && reporterRaw !== null) {
    handledKeys.add("reporter");
    const reporterRecord = ensureRecord(
      reporterRaw,
      "createSnsGovernanceCase payload.reporter",
    );
    const reporterWithAliases = { ...reporterRecord };
    if (reporterRecord.referenceTicket !== undefined && reporterRecord.referenceTicket !== null) {
      reporterWithAliases.reference_ticket = reporterRecord.referenceTicket;
    }
    const reporter = normalizeSnsGovernanceCaseReporter(
      reporterWithAliases,
      "createSnsGovernanceCase payload.reporter",
    );
    normalized.reporter = {
      role: reporter.role,
      contact: reporter.contact,
    };
    if (reporter.referenceTicket !== undefined && reporter.referenceTicket !== null) {
      normalized.reporter.reference_ticket = reporter.referenceTicket;
    }
  }

  const respondentsRaw = record.respondents;
  if (respondentsRaw !== undefined && respondentsRaw !== null) {
    handledKeys.add("respondents");
    if (!Array.isArray(respondentsRaw)) {
      throw new TypeError("createSnsGovernanceCase payload.respondents must be an array");
    }
    const respondentsWithAliases = respondentsRaw.map((entry, index) => {
      const recordEntry = ensureRecord(
        entry ?? {},
        `createSnsGovernanceCase payload.respondents[${index}]`,
      );
      return {
        ...recordEntry,
        account_id: recordEntry.account_id ?? recordEntry.accountId,
      };
    });
    const respondents = normalizeSnsGovernanceCaseRespondents(
      respondentsWithAliases,
      "createSnsGovernanceCase payload.respondents",
    );
    normalized.respondents = respondents.map(({ role, accountId, contact }) => {
      const out = { role, account_id: accountId };
      if (contact !== undefined && contact !== null) {
        out.contact = contact;
      }
      return out;
    });
  }

  const allegationsRaw = record.allegations;
  if (allegationsRaw !== undefined && allegationsRaw !== null) {
    handledKeys.add("allegations");
    if (!Array.isArray(allegationsRaw)) {
      throw new TypeError("createSnsGovernanceCase payload.allegations must be an array");
    }
    const allegationsWithAliases = allegationsRaw.map((entry, index) => {
      const recordEntry = ensureRecord(
        entry ?? {},
        `createSnsGovernanceCase payload.allegations[${index}]`,
      );
      return {
        ...recordEntry,
        policy_reference: recordEntry.policy_reference ?? recordEntry.policyReference,
      };
    });
    const allegations = normalizeSnsGovernanceCaseAllegations(
      allegationsWithAliases,
      "createSnsGovernanceCase payload.allegations",
    );
    normalized.allegations = allegations.map(({ code, summary, policyReference }) => {
      const out = { code };
      if (summary !== undefined && summary !== null) {
        out.summary = summary;
      }
      if (policyReference !== undefined && policyReference !== null) {
        out.policy_reference = policyReference;
      }
      return out;
    });
  }

  const evidenceRaw = record.evidence;
  if (evidenceRaw !== undefined && evidenceRaw !== null) {
    handledKeys.add("evidence");
    if (!Array.isArray(evidenceRaw)) {
      throw new TypeError("createSnsGovernanceCase payload.evidence must be an array");
    }
    const evidence = normalizeSnsGovernanceCaseEvidenceList(
      evidenceRaw.map((entry, index) => {
        const recordEntry = ensureRecord(
          entry ?? {},
          `createSnsGovernanceCase payload.evidence[${index}]`,
        );
        return {
          ...recordEntry,
          hash: recordEntry.hash ?? recordEntry.hashHex,
        };
      }),
      "createSnsGovernanceCase payload.evidence",
    );
    normalized.evidence = evidence.map(({ id, kind, uri, hashHex, description, sealed }) => {
      const out = { id, kind, hash: hashHex, sealed };
      if (uri !== undefined && uri !== null) {
        out.uri = uri;
      }
      if (description !== undefined && description !== null) {
        out.description = description;
      }
      return out;
    });
  }

  const slaRaw = record.sla;
  if (slaRaw !== undefined && slaRaw !== null) {
    handledKeys.add("sla");
    const slaRecord = ensureRecord(slaRaw, "createSnsGovernanceCase payload.sla");
    const slaWithAliases = { ...slaRecord };
    slaWithAliases.acknowledge_by = slaRecord.acknowledge_by ?? slaRecord.acknowledgeBy;
    slaWithAliases.resolution_by = slaRecord.resolution_by ?? slaRecord.resolutionBy;
    if (Array.isArray(slaRecord.extensions)) {
      slaWithAliases.extensions = slaRecord.extensions.map((entry, index) => {
        const recordEntry = ensureRecord(
          entry ?? {},
          `createSnsGovernanceCase payload.sla.extensions[${index}]`,
        );
        return {
          ...recordEntry,
          approved_by: recordEntry.approved_by ?? recordEntry.approvedBy,
          new_resolution_by: recordEntry.new_resolution_by ?? recordEntry.newResolutionBy,
        };
      });
    }
    const sla = normalizeSnsGovernanceCaseSla(
      slaWithAliases,
      "createSnsGovernanceCase payload.sla",
    );
    normalized.sla = {
      acknowledge_by: sla.acknowledgeBy,
      resolution_by: sla.resolutionBy,
    };
    if (sla.extensions.length > 0) {
      normalized.sla.extensions = sla.extensions.map((entry) => ({
        approved_by: entry.approvedBy,
        reason: entry.reason,
        new_resolution_by: entry.newResolutionBy,
      }));
    }
  }

  const actionsRaw = record.actions;
  if (actionsRaw !== undefined && actionsRaw !== null) {
    handledKeys.add("actions");
    const actions = normalizeSnsGovernanceCaseActions(
      actionsRaw,
      "createSnsGovernanceCase payload.actions",
    );
    normalized.actions = actions.map(({ timestamp, actor, action, notes }) => {
      const out = { timestamp, actor, action };
      if (notes !== undefined && notes !== null) {
        out.notes = notes;
      }
      return out;
    });
  }

  const decisionRaw = record.decision;
  if (decisionRaw !== undefined && decisionRaw !== null) {
    handledKeys.add("decision");
    const decisionRecord = ensureRecord(
      decisionRaw,
      "createSnsGovernanceCase payload.decision",
    );
    const decisionWithAliases = { ...decisionRecord };
    decisionWithAliases.effective_at = decisionRecord.effective_at ?? decisionRecord.effectiveAt;
    decisionWithAliases.publication_state =
      decisionRecord.publication_state ?? decisionRecord.publicationState;
    const decision = normalizeSnsGovernanceCaseDecision(
      decisionWithAliases,
      "createSnsGovernanceCase payload.decision",
    );
    const outgoingDecision = {};
    if (decision.finding !== null && decision.finding !== undefined) {
      outgoingDecision.finding = decision.finding;
    }
    if (decision.remedies && decision.remedies.length > 0) {
      outgoingDecision.remedies = decision.remedies;
    }
    if (decision.effectiveAt !== undefined && decision.effectiveAt !== null) {
      outgoingDecision.effective_at = decision.effectiveAt;
    }
    if (decision.publicationState !== null && decision.publicationState !== undefined) {
      outgoingDecision.publication_state = decision.publicationState;
    }
    if (Object.keys(outgoingDecision).length > 0) {
      normalized.decision = outgoingDecision;
    }
  }

  const timestampFields = [
    ["reported_at", "reportedAt"],
    ["acknowledged_at", "acknowledgedAt"],
    ["triage_started_at", "triageStartedAt"],
    ["hearing_scheduled_at", "hearingScheduledAt"],
    ["resolution_issued_at", "resolutionIssuedAt"],
  ];

  for (const [snake, camel] of timestampFields) {
    const raw = record[snake] ?? record[camel];
    if (raw !== undefined && raw !== null) {
      handledKeys.add(snake);
      handledKeys.add(camel);
      normalized[snake] = requireNonEmptyString(
        raw,
        `createSnsGovernanceCase payload.${snake}`,
      );
    }
  }

  for (const [key, value] of Object.entries(record)) {
    if (!handledKeys.has(key) && value !== undefined) {
      normalized[key] = value;
    }
  }

  return normalized;
}

const SNS_CASE_EXPORT_OPTION_KEYS = new Set(["since", "status", "limit", "signal"]);

function normalizeSnsCaseExportOptions(options) {
  if (options === undefined || options === null) {
    return { signal: undefined, params: {} };
  }
  const record = ensureRecord(options, "exportSnsGovernanceCases options");
  assertSupportedOptionKeys(
    record,
    SNS_CASE_EXPORT_OPTION_KEYS,
    "exportSnsGovernanceCases options",
  );
  const params = {};
  if (record.since !== undefined && record.since !== null) {
    params.since = requireNonEmptyString(record.since, "exportSnsGovernanceCases.since");
  }
  if (record.status !== undefined && record.status !== null) {
    params.status = requireNonEmptyString(record.status, "exportSnsGovernanceCases.status");
  }
  if (record.limit !== undefined && record.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(record.limit, "exportSnsGovernanceCases.limit", {
      allowZero: true,
    });
  }
  const { signal } = normalizeSignalOption(record, "exportSnsGovernanceCases");
  return { signal, params };
}

function normalizeSnsGovernanceCaseExportResponse(payload) {
  const record = ensureRecord(payload ?? {}, "sns governance case export response");
  const cases = normalizeSnsGovernanceCaseList(record.cases, "sns governance case export response.cases");
  const nextSinceRaw = record.next_since;
  const nextCursorRaw = record.next_cursor;
  const totalCountRaw = record.total_count;
  const nextSince =
    nextSinceRaw === undefined || nextSinceRaw === null
      ? null
      : requireNonEmptyString(nextSinceRaw, "sns governance case export response.next_since");
  const nextCursor =
    nextCursorRaw === undefined || nextCursorRaw === null
      ? null
      : requireNonEmptyString(nextCursorRaw, "sns governance case export response.next_cursor");
  const totalCount =
    totalCountRaw === undefined || totalCountRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          totalCountRaw,
          "sns governance case export response.total_count",
          { allowZero: true },
        );
  return { cases, nextSince, nextCursor, totalCount };
}

function normalizeSnsGovernanceCaseList(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCase(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCase(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const caseId = requireNonEmptyString(record.case_id, `${context}.case_id`);
  const selector = normalizeSnsGovernanceCaseSelector(record.selector, `${context}.selector`);
  const disputeType = normalizeSnsGovernanceCaseDisputeType(record.dispute_type, `${context}.dispute_type`);
  const priority = normalizeSnsGovernanceCasePriority(record.priority, `${context}.priority`);
  const reportedAt = requireNonEmptyString(record.reported_at, `${context}.reported_at`);
  const acknowledgedAt = optionalString(record.acknowledged_at, `${context}.acknowledged_at`);
  const triageStartedAt = optionalString(record.triage_started_at, `${context}.triage_started_at`);
  const hearingScheduledAt = optionalString(record.hearing_scheduled_at, `${context}.hearing_scheduled_at`);
  const resolutionIssuedAt = optionalString(record.resolution_issued_at, `${context}.resolution_issued_at`);
  const status = normalizeSnsGovernanceCaseStatus(record.status, `${context}.status`);
  const reporter = normalizeSnsGovernanceCaseReporter(record.reporter, `${context}.reporter`);
  const respondents = normalizeSnsGovernanceCaseRespondents(record.respondents, `${context}.respondents`);
  const allegations = normalizeSnsGovernanceCaseAllegations(record.allegations, `${context}.allegations`);
  const evidence = normalizeSnsGovernanceCaseEvidenceList(record.evidence, `${context}.evidence`);
  const sla = normalizeSnsGovernanceCaseSla(record.sla, `${context}.sla`);
  const actions = normalizeSnsGovernanceCaseActions(record.actions, `${context}.actions`);
  const decision = normalizeSnsGovernanceCaseDecision(record.decision, `${context}.decision`);
  return {
    caseId,
    selector,
    disputeType,
    priority,
    reportedAt,
    acknowledgedAt,
    triageStartedAt,
    hearingScheduledAt,
    resolutionIssuedAt,
    status,
    reporter,
    respondents,
    allegations,
    evidence,
    sla,
    actions,
    decision,
  };
}

function normalizeSnsGovernanceCaseSelector(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const suffixRaw = record.suffix_id ?? record.suffixId;
  const labelRaw = record.label;
  const globalFormRaw = record.global_form ?? record.globalForm;
  const suffixId = ToriiClient._normalizeUnsignedInteger(
    suffixRaw,
    `${context}.suffix_id`,
    {
      allowZero: true,
    },
  );
  const label = requireNonEmptyString(labelRaw, `${context}.label`);
  const globalForm = requireNonEmptyString(globalFormRaw, `${context}.global_form`);
  return { suffixId, label, globalForm };
}

function normalizeSnsGovernanceCaseDisputeType(value, context) {
  const normalized = requireNonEmptyString(value, context);
  if (!SNS_GOV_CASE_DISPUTE_TYPES.has(normalized)) {
    throw new TypeError(
      `${context} must be one of ${Array.from(SNS_GOV_CASE_DISPUTE_TYPES).join(", ")}`,
    );
  }
  return normalized;
}

function normalizeSnsGovernanceCasePriority(value, context) {
  const normalized = requireNonEmptyString(value, context);
  if (!SNS_GOV_CASE_PRIORITY_VALUES.has(normalized)) {
    throw new TypeError(
      `${context} must be one of ${Array.from(SNS_GOV_CASE_PRIORITY_VALUES).join(", ")}`,
    );
  }
  return normalized;
}

function normalizeSnsGovernanceCaseStatus(value, context) {
  const normalized = requireNonEmptyString(value, context);
  if (!SNS_GOV_CASE_STATUS_VALUES.has(normalized)) {
    throw new TypeError(
      `${context} must be one of ${Array.from(SNS_GOV_CASE_STATUS_VALUES).join(", ")}`,
    );
  }
  return normalized;
}

function normalizeSnsGovernanceCaseReporter(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const role = requireNonEmptyString(record.role, `${context}.role`);
  if (!SNS_GOV_CASE_REPORTER_ROLES.has(role)) {
    throw new TypeError(
      `${context}.role must be one of ${Array.from(SNS_GOV_CASE_REPORTER_ROLES).join(", ")}`,
    );
  }
  const contact = requireNonEmptyString(record.contact, `${context}.contact`);
  const referenceTicket = optionalString(record.reference_ticket, `${context}.reference_ticket`);
  return { role, contact, referenceTicket };
}

function normalizeSnsGovernanceCaseRespondents(value, context) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCaseRespondent(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCaseRespondent(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const role = requireNonEmptyString(record.role, `${context}.role`);
  if (!SNS_GOV_CASE_RESPONDENT_ROLES.has(role)) {
    throw new TypeError(
      `${context}.role must be one of ${Array.from(SNS_GOV_CASE_RESPONDENT_ROLES).join(", ")}`,
    );
  }
  const accountId = requireNonEmptyString(record.account_id, `${context}.account_id`);
  const contact = optionalString(record.contact, `${context}.contact`);
  return { role, accountId, contact };
}

function normalizeSnsGovernanceCaseAllegations(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCaseAllegation(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCaseAllegation(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const code = requireNonEmptyString(record.code, `${context}.code`);
  const summary = optionalString(record.summary, `${context}.summary`);
  const policyReference = optionalString(record.policy_reference, `${context}.policy_reference`);
  return { code, summary, policyReference };
}

function normalizeSnsGovernanceCaseEvidenceList(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCaseEvidence(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCaseEvidence(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  const kind = requireNonEmptyString(record.kind, `${context}.kind`);
  if (!SNS_GOV_CASE_EVIDENCE_KIND_VALUES.has(kind)) {
    throw new TypeError(
      `${context}.kind must be one of ${Array.from(SNS_GOV_CASE_EVIDENCE_KIND_VALUES).join(", ")}`,
    );
  }
  const uri = optionalString(record.uri, `${context}.uri`);
  const hashHex = normalizeHex32String(record.hash, `${context}.hash`);
  const description = optionalString(record.description, `${context}.description`);
  const sealed =
    record.sealed === undefined || record.sealed === null ? false : coerceBoolean(record.sealed, `${context}.sealed`);
  return { id, kind, uri, hashHex, description, sealed };
}

function normalizeSnsGovernanceCaseSla(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const acknowledgeBy = requireNonEmptyString(record.acknowledge_by, `${context}.acknowledge_by`);
  const resolutionBy = requireNonEmptyString(record.resolution_by, `${context}.resolution_by`);
  const extensions =
    record.extensions === undefined || record.extensions === null
      ? []
      : normalizeSnsGovernanceCaseSlaExtensions(record.extensions, `${context}.extensions`);
  return { acknowledgeBy, resolutionBy, extensions };
}

function normalizeSnsGovernanceCaseSlaExtensions(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCaseSlaExtension(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCaseSlaExtension(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const approvedBy = requireNonEmptyString(record.approved_by, `${context}.approved_by`);
  const reason = requireNonEmptyString(record.reason, `${context}.reason`);
  const newResolutionBy = requireNonEmptyString(record.new_resolution_by, `${context}.new_resolution_by`);
  return { approvedBy, reason, newResolutionBy };
}

function normalizeSnsGovernanceCaseActions(value, context) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty array`);
  }
  return value.map((entry, index) => normalizeSnsGovernanceCaseAction(entry, `${context}[${index}]`));
}

function normalizeSnsGovernanceCaseAction(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const timestamp = requireNonEmptyString(record.timestamp, `${context}.timestamp`);
  const actor = requireNonEmptyString(record.actor, `${context}.actor`);
  const action = requireNonEmptyString(record.action, `${context}.action`);
  const notes = optionalString(record.notes, `${context}.notes`);
  return { timestamp, actor, action, notes };
}

function normalizeSnsGovernanceCaseDecision(payload, context) {
  if (payload === undefined || payload === null) {
    return null;
  }
  const record = ensureRecord(payload ?? {}, context);
  let finding = null;
  if (record.finding !== undefined && record.finding !== null) {
    const normalized = requireNonEmptyString(record.finding, `${context}.finding`);
    if (!SNS_GOV_CASE_DECISION_FINDINGS.has(normalized)) {
      throw new TypeError(
        `${context}.finding must be one of ${Array.from(SNS_GOV_CASE_DECISION_FINDINGS).join(", ")}`,
      );
    }
    finding = normalized;
  }
  const remedies =
    record.remedies === undefined || record.remedies === null
      ? []
      : normalizeStringList(record.remedies, `${context}.remedies`);
  const effectiveAt = optionalString(record.effective_at, `${context}.effective_at`);
  let publicationState = null;
  if (record.publication_state !== undefined && record.publication_state !== null) {
    const normalized = requireNonEmptyString(record.publication_state, `${context}.publication_state`);
    if (!SNS_GOV_CASE_PUBLICATION_STATES.has(normalized)) {
      throw new TypeError(
        `${context}.publication_state must be one of ${Array.from(SNS_GOV_CASE_PUBLICATION_STATES).join(", ")}`,
      );
    }
    publicationState = normalized;
  }
  return { finding, remedies, effectiveAt, publicationState };
}

function normalizeStringList(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => requireNonEmptyString(entry, `${context}[${index}]`));
}

function normalizeExplorerNftRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const id = requireNonEmptyString(record.id ?? "", `${context}.id`);
  const ownedBy = requireNonEmptyString(
    record.owned_by ?? "",
    `${context}.owned_by`,
  );
  const metadata =
    record.metadata === undefined || record.metadata === null
      ? {}
      : cloneJsonValue(record.metadata, `${context}.metadata`);
  return { id, ownedBy, metadata };
}

function normalizeExplorerNftPage(payload) {
  const record = ensureRecord(payload ?? {}, "explorer nfts response");
  const items = record.items;
  if (!Array.isArray(items)) {
    throw new TypeError("explorer nfts response.items must be an array");
  }
  return {
    pagination: normalizeExplorerPaginationMeta(
      record.pagination ?? {},
      "explorer nfts response.pagination",
    ),
    items: items.map((item, index) =>
      normalizeExplorerNftRecord(item, `explorer nfts response.items[${index}]`),
    ),
  };
}

function normalizeExplorerRwaRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const id = requireNonEmptyString(record.id ?? "", `${context}.id`);
  const ownedBy = requireNonEmptyString(record.owned_by ?? "", `${context}.owned_by`);
  const quantity = requireNonEmptyString(record.quantity ?? "", `${context}.quantity`);
  const heldQuantity = requireNonEmptyString(
    record.held_quantity ?? "",
    `${context}.held_quantity`,
  );
  const primaryReference = requireNonEmptyString(
    record.primary_reference ?? "",
    `${context}.primary_reference`,
  );
  if (typeof record.is_frozen !== "boolean") {
    throw new TypeError(`${context}.is_frozen must be a boolean`);
  }
  let status = null;
  if (record.status !== undefined && record.status !== null) {
    status = requireNonEmptyString(record.status, `${context}.status`);
  }
  const metadata =
    record.metadata === undefined || record.metadata === null
      ? {}
      : cloneJsonValue(record.metadata, `${context}.metadata`);
  return {
    id,
    ownedBy,
    quantity,
    heldQuantity,
    primaryReference,
    status,
    isFrozen: record.is_frozen,
    metadata,
    raw: cloneJsonValue(record, `${context}.raw`),
  };
}

function normalizeExplorerRwaPage(payload) {
  const record = ensureRecord(payload ?? {}, "explorer rwas response");
  const items = record.items;
  if (!Array.isArray(items)) {
    throw new TypeError("explorer rwas response.items must be an array");
  }
  return {
    pagination: normalizeExplorerPaginationMeta(
      record.pagination ?? {},
      "explorer rwas response.pagination",
    ),
    items: items.map((item, index) =>
      normalizeExplorerRwaRecord(item, `explorer rwas response.items[${index}]`),
    ),
  };
}

function normalizeExplorerBlocksPage(payload) {
  const record = ensureRecord(payload ?? {}, "explorer blocks response");
  const items = record.items;
  if (!Array.isArray(items)) {
    throw new TypeError("explorer blocks response.items must be an array");
  }
  return {
    pagination: normalizeExplorerPaginationMeta(
      record.pagination ?? {},
      "explorer blocks response.pagination",
    ),
    items: items.map((item, index) =>
      normalizeExplorerBlockRecord(item, `explorer blocks response.items[${index}]`),
    ),
  };
}

function normalizeExplorerAccountQrResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const canonicalId = requireNonEmptyString(
    record.canonical_id ?? "",
    `${context}.canonical_id`,
  );
  const literal = requireNonEmptyString(record.literal ?? "", `${context}.literal`);
  const networkPrefix = ToriiClient._normalizeUnsignedInteger(
    record.network_prefix ?? 0,
    `${context}.network_prefix`,
    { allowZero: false },
  );
  const errorCorrection = requireNonEmptyString(
    record.error_correction ?? "",
    `${context}.error_correction`,
  );
  const modules = ToriiClient._normalizeUnsignedInteger(
    record.modules ?? 0,
    `${context}.modules`,
    { allowZero: false },
  );
  const qrVersion = ToriiClient._normalizeUnsignedInteger(
    record.qr_version ?? 0,
    `${context}.qr_version`,
    { allowZero: false },
  );
  const svg = requireNonEmptyString(record.svg ?? "", `${context}.svg`);
  return {
    canonicalId,
    literal,
    networkPrefix,
    errorCorrection,
    modules,
    qrVersion,
    svg,
  };
}

function normalizeExplorerPaginationMeta(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    page: ToriiClient._normalizeUnsignedInteger(record.page ?? 1, `${context}.page`, {
      allowZero: false,
    }),
    perPage: ToriiClient._normalizeUnsignedInteger(
      record.per_page ?? 1,
      `${context}.per_page`,
      { allowZero: false },
    ),
    totalPages: ToriiClient._normalizeUnsignedInteger(
      record.total_pages ?? 0,
      `${context}.total_pages`,
      { allowZero: true },
    ),
    totalItems: ToriiClient._normalizeUnsignedInteger(
      record.total_items ?? 0,
      `${context}.total_items`,
      { allowZero: true },
    ),
  };
}

function normalizeExplorerBlockRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const prevBlockHashRaw = record.prev_block_hash ?? null;
  const transactionsHashRaw = record.transactions_hash ?? null;
  return {
    hash: requireHexString(record.hash ?? "", `${context}.hash`),
    height: ToriiClient._normalizeUnsignedInteger(record.height ?? 1, `${context}.height`, {
      allowZero: false,
    }),
    createdAt: requireNonEmptyString(
      record.created_at ?? "",
      `${context}.created_at`,
    ),
    prevBlockHash:
      prevBlockHashRaw === null || prevBlockHashRaw === undefined
        ? null
        : requireHexString(prevBlockHashRaw, `${context}.prev_block_hash`),
    transactionsHash:
      transactionsHashRaw === null || transactionsHashRaw === undefined
        ? null
        : requireHexString(transactionsHashRaw, `${context}.transactions_hash`),
    transactionsRejected: ToriiClient._normalizeUnsignedInteger(
      record.transactions_rejected ?? 0,
      `${context}.transactions_rejected`,
      { allowZero: true },
    ),
    transactionsTotal: ToriiClient._normalizeUnsignedInteger(
      record.transactions_total ?? 0,
      `${context}.transactions_total`,
      { allowZero: true },
    ),
  };
}

function normalizeRuntimeUpgradesListResponse(payload) {
  const record = ensureRecord(payload, "runtime upgrades list response");
  const items = record.items ?? [];
  if (!Array.isArray(items)) {
    throw new TypeError("runtime upgrades list response.items must be an array");
  }
  return items.map((item, index) =>
    normalizeRuntimeUpgradeListItem(item, index, "runtime upgrades list response.items"),
  );
}

function normalizeRuntimeUpgradeListItem(value, index, context) {
  const record = ensureRecord(value, `${context}[${index}]`);
  const idHex = requireHexString(record.id_hex ?? "", `${context}[${index}].id_hex`);
  const normalizedRecord = normalizeRuntimeUpgradeRecord(
    record.record,
    `${context}[${index}].record`,
  );
  return { idHex, record: normalizedRecord };
}

function normalizeRuntimeUpgradeRecord(value, context) {
  const record = ensureRecord(value, context);
  return {
    manifest: normalizeRuntimeUpgradeManifest(record.manifest, `${context}.manifest`),
    status: normalizeRuntimeUpgradeStatus(record.status, `${context}.status`),
    proposer: requireNonEmptyString(record.proposer, `${context}.proposer`),
    createdHeight: ToriiClient._normalizeUnsignedInteger(
      record.created_height,
      `${context}.created_height`,
      { allowZero: true },
    ),
  };
}

function normalizeRuntimeUpgradeManifest(value, context) {
  const record = ensureRecord(value, context);
  const abiVersion = ToriiClient._normalizeUnsignedInteger(
    record.abi_version,
    `${context}.abi_version`,
    { allowZero: false },
  );
  const addedSyscalls = parseIntegerArray(
    record.added_syscalls,
    `${context}.added_syscalls`,
  );
  const addedPointerTypes = parseIntegerArray(
    record.added_pointer_types,
    `${context}.added_pointer_types`,
  );
  assertFirstReleaseRuntimeUpgradeManifest({
    abiVersion,
    addedSyscalls,
    addedPointerTypes,
    context,
  });
  return {
    name: requireNonEmptyString(record.name, `${context}.name`),
    description: requireNonEmptyString(record.description, `${context}.description`),
    abiVersion,
    abiHashHex: requireHexString(
      record.abi_hash ?? "",
      `${context}.abi_hash`,
    ),
    addedSyscalls,
    addedPointerTypes,
    startHeight: ToriiClient._normalizeUnsignedInteger(
      record.start_height,
      `${context}.start_height`,
      { allowZero: true },
    ),
    endHeight: ToriiClient._normalizeUnsignedInteger(
      record.end_height,
      `${context}.end_height`,
      { allowZero: true },
    ),
  };
}

function normalizeRuntimeUpgradeStatus(value, context) {
  const record = ensureRecord(value, context);
  const keys = Object.keys(record);
  if (keys.length !== 1) {
    throw new TypeError(`${context} must contain exactly one status key`);
  }
  const kind = keys[0];
  const payload = record[kind];
  if (kind === "Proposed" || kind === "Canceled") {
    return { kind };
  }
  if (kind === "ActivatedAt") {
    const height = ToriiClient._normalizeUnsignedInteger(
      payload,
      `${context}.ActivatedAt`,
      { allowZero: true },
    );
    return { kind: "ActivatedAt", activatedHeight: height };
  }
  throw new TypeError(`${context} contains unsupported variant ${kind}`);
}

function normalizeRuntimeUpgradeManifestPayload(value, context) {
  const record = ensureRecord(value, context);
  const abiVersion = record.abi_version ?? record.abiVersion;
  const abiHashValue =
    record.abi_hash ?? record.abiHash ?? record.abiHashHex;
  const startValue = record.start_height ?? record.startHeight;
  const endValue = record.end_height ?? record.endHeight;
  if (abiVersion === undefined || abiVersion === null) {
    throw new TypeError(`${context}.abi_version is required`);
  }
  if (abiHashValue === undefined || abiHashValue === null) {
    throw new TypeError(`${context}.abi_hash is required`);
  }
  const startHeight = ToriiClient._normalizeUnsignedInteger(
    startValue,
    `${context}.start_height`,
    { allowZero: true },
  );
  const endHeight = ToriiClient._normalizeUnsignedInteger(
    endValue,
    `${context}.end_height`,
    { allowZero: true },
  );
  if (endHeight <= startHeight) {
    throw new TypeError(`${context}.end_height must be greater than start_height`);
  }
  const normalizedAbiVersion = ToriiClient._normalizeUnsignedInteger(
    abiVersion,
    `${context}.abi_version`,
    { allowZero: false },
  );
  const addedSyscalls = parseIntegerArray(
    record.added_syscalls ?? record.addedSyscalls ?? [],
    `${context}.added_syscalls`,
  );
  const addedPointerTypes = parseIntegerArray(
    record.added_pointer_types ?? record.addedPointerTypes ?? [],
    `${context}.added_pointer_types`,
  );
  assertFirstReleaseRuntimeUpgradeManifest({
    abiVersion: normalizedAbiVersion,
    addedSyscalls,
    addedPointerTypes,
    context,
  });
  return {
    name: requireNonEmptyString(record.name, `${context}.name`),
    description: requireNonEmptyString(
      record.description,
      `${context}.description`,
    ),
    abi_version: normalizedAbiVersion,
    abi_hash: normalizeHex32String(abiHashValue, `${context}.abi_hash`),
    added_syscalls: addedSyscalls,
    added_pointer_types: addedPointerTypes,
    start_height: startHeight,
    end_height: endHeight,
  };
}

function assertFirstReleaseRuntimeUpgradeManifest({
  abiVersion,
  addedSyscalls,
  addedPointerTypes,
  context,
}) {
  if (abiVersion !== 1) {
    throw new TypeError(`${context}.abi_version must be 1 in the first release`);
  }
  if (addedSyscalls.length > 0) {
    throw new TypeError(`${context}.added_syscalls must be empty in the first release`);
  }
  if (addedPointerTypes.length > 0) {
    throw new TypeError(`${context}.added_pointer_types must be empty in the first release`);
  }
}

function normalizeRuntimeUpgradeTxResponse(payload, context) {
  const base = normalizeGovernanceDraftResponse(payload, context);
  return {
    ok: base.ok,
    tx_instructions: base.tx_instructions,
  };
}

function parseIntegerArray(value, context) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => {
    const numeric = coerceIntegerLike(entry, `${context}[${index}]`);
    if (numeric < 0) {
      throw new RangeError(`${context}[${index}] must be non-negative`);
    }
    return numeric;
  });
}

function coerceStatusInt(value, context) {
  if (value === undefined || value === null) {
    return 0;
  }
  const numeric = coerceIntegerLike(value, context);
  if (numeric < 0) {
    throw new RangeError(`${context} must be non-negative`);
  }
  return numeric;
}

function coerceNestedInt(mapping, key, context) {
  if (!isPlainObject(mapping)) {
    throw new TypeError(`${context} must be an object`);
  }
  const value = mapping[key];
  if (value === undefined || value === null) {
    return 0;
  }
  const numeric = coerceIntegerLike(value, `${context}.${key}`);
  if (numeric < 0) {
    throw new RangeError(`${context}.${key} must be non-negative`);
  }
  return numeric;
}

function ensureRecord(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function normalizeOptionalMetadata(value, context) {
  if (value === undefined || value === null) {
    return {};
  }
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be a plain object when provided`);
  }
  return { ...value };
}

function coerceInteger(value, context) {
  if (value === undefined || value === null || value === "") {
    throw new TypeError(`${context} must be numeric`);
  }
  return coerceIntegerLike(value, context);
}

function coerceBoolean(value, context) {
  if (value === undefined || value === null || value === "") {
    return false;
  }
  if (typeof value === "boolean") {
    return value;
  }
  if (value === 1 || value === "1") {
    return true;
  }
  if (value === 0 || value === "0") {
    return false;
  }
  if (typeof value === "string") {
    const lower = value.toLowerCase();
    if (lower === "true") {
      return true;
    }
    if (lower === "false") {
      return false;
    }
  }
  throw new TypeError(`${context} must be boolean`);
}

function requireBooleanLike(value, context) {
  if (value === undefined || value === null || value === "") {
    throw new TypeError(`${context} must be boolean`);
  }
  return coerceBoolean(value, context);
}

function coerceOptionalInt(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  return coerceIntegerLike(value, context);
}

function parseStringArray(value, context) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of strings`);
  }
  return value.map((entry, index) => {
    if (entry === undefined || entry === null || typeof entry !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    return entry;
  });
}

function parseRecordArray(value, context) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of objects`);
  }
  return value.map((entry, index) => ensureRecord(entry, `${context}[${index}]`));
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function optionalString(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === "string") {
    return value;
  }
  throw new TypeError(`${context} must be a string when present`);
}

function optionalBoolean(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  return coerceBoolean(value, context);
}

function optionalNumber(value, context, { allowNegative = false } = {}) {
  if (value === undefined || value === null) {
    return null;
  }
  const numeric = coerceIntegerLike(value, context);
  if (!allowNegative && numeric < 0) {
    throw new RangeError(`${context} must be non-negative`);
  }
  return numeric;
}

function optionalRecord(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object when present`);
  }
  return value;
}

function optionalStringArray(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array when present`);
  }
  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    return entry;
  });
}

function requireStringArray(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    return entry;
  });
}

function requireUnsignedIntegerArray(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) =>
    ToriiClient._normalizeUnsignedInteger(entry, `${context}[${index}]`, {
      allowZero: true,
    }),
  );
}

function normalizeUaidLiteral(value, context = "uaid") {
  const literal = requireNonEmptyString(value, context);
  const trimmed = literal.trim();
  if (!trimmed) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  let hexPortion;
  if (trimmed.slice(0, 5).toLowerCase() === "uaid:") {
    hexPortion = trimmed.slice(5).trim();
  } else {
    hexPortion = trimmed;
  }
  if (hexPortion.length !== 64 || !/^[0-9a-fA-F]+$/.test(hexPortion)) {
    throw new TypeError(`${context} must contain 64 hex characters`);
  }
  if (!/[13579bdf]$/i.test(hexPortion)) {
    throw new TypeError(`${context} must have least significant bit set to 1`);
  }
  return `uaid:${hexPortion.toLowerCase()}`;
}

function normalizeUaidPortfolioOptions(options, context = "getUaidPortfolio") {
  if (options === undefined) {
    return { signal: undefined, assetId: undefined };
  }
  const record = ensureRecord(options, `${context} options`);
  assertSupportedOptionKeys(record, new Set(["signal", "assetId"]), `${context} options`);
  const { signal } = normalizeSignalOption(record, context);
  let assetId;
  if (record.assetId !== undefined && record.assetId !== null) {
    assetId = ToriiClient._normalizeAssetId(record.assetId, `${context}.assetId`);
  }
  return { signal, assetId };
}

function normalizeUaidPortfolioResponse(payload) {
  const record = ensureRecord(payload, "uaid portfolio response");
  const uaid = normalizeUaidLiteral(record.uaid, "uaid portfolio response.uaid");
  const totalsRecord = ensureRecord(
    record.totals ?? {},
    "uaid portfolio response.totals",
  );
  const totals = {
    accounts: ToriiClient._normalizeUnsignedInteger(
      totalsRecord.accounts ?? 0,
      "uaid portfolio response.totals.accounts",
      { allowZero: true },
    ),
    positions: ToriiClient._normalizeUnsignedInteger(
      totalsRecord.positions ?? 0,
      "uaid portfolio response.totals.positions",
      { allowZero: true },
    ),
  };
  const dataspacesValue = record.dataspaces ?? [];
  if (!Array.isArray(dataspacesValue)) {
    throw new TypeError("uaid portfolio response.dataspaces must be an array");
  }
  const dataspaces = dataspacesValue.map((entry, index) =>
    normalizeUaidPortfolioDataspace(
      entry,
      `uaid portfolio response.dataspaces[${index}]`,
    ),
  );
  return { uaid, totals, dataspaces };
}

function normalizeUaidPortfolioDataspace(value, context) {
  const record = ensureRecord(value, context);
  const accountsValue = record.accounts ?? [];
  if (!Array.isArray(accountsValue)) {
    throw new TypeError(`${context}.accounts must be an array`);
  }
  return {
    dataspace_id: ToriiClient._normalizeUnsignedInteger(
      record.dataspace_id,
      `${context}.dataspace_id`,
      { allowZero: true },
    ),
    dataspace_alias: optionalString(record.dataspace_alias, `${context}.dataspace_alias`),
    accounts: accountsValue.map((entry, index) =>
      normalizeUaidPortfolioAccount(entry, `${context}.accounts[${index}]`),
    ),
  };
}

function normalizeUaidPortfolioAccount(value, context) {
  const record = ensureRecord(value, context);
  const assetsValue = record.assets ?? [];
  if (!Array.isArray(assetsValue)) {
    throw new TypeError(`${context}.assets must be an array`);
  }
  return {
    account_id: ToriiClient._normalizeAccountId(record.account_id, `${context}.account_id`),
    label: optionalString(record.label, `${context}.label`),
    assets: assetsValue.map((entry, index) =>
      normalizeUaidPortfolioAsset(entry, `${context}.assets[${index}]`),
    ),
  };
}

function normalizeUaidPortfolioAsset(value, context) {
  const record = ensureRecord(value, context);
  return {
    asset_id: ToriiClient._normalizeAssetId(record.asset_id, `${context}.asset_id`),
    asset_definition_id: requireNonEmptyString(
      record.asset_definition_id,
      `${context}.asset_definition_id`,
    ),
    quantity: requireNonEmptyString(record.quantity, `${context}.quantity`),
  };
}

function normalizeUaidBindingsResponse(payload) {
  const record = ensureRecord(payload, "uaid bindings response");
  const uaid = normalizeUaidLiteral(record.uaid, "uaid bindings response.uaid");
  const dataspacesValue = record.dataspaces ?? [];
  if (!Array.isArray(dataspacesValue)) {
    throw new TypeError("uaid bindings response.dataspaces must be an array");
  }
  const dataspaces = dataspacesValue.map((entry, index) =>
    normalizeUaidBindingsDataspace(
      entry,
      `uaid bindings response.dataspaces[${index}]`,
    ),
  );
  return { uaid, dataspaces };
}

function normalizeUaidBindingsDataspace(value, context) {
  const record = ensureRecord(value, context);
  return {
    dataspace_id: ToriiClient._normalizeUnsignedInteger(
      record.dataspace_id,
      `${context}.dataspace_id`,
      { allowZero: true },
    ),
    dataspace_alias: optionalString(record.dataspace_alias, `${context}.dataspace_alias`),
    accounts: requireStringArray(record.accounts ?? [], `${context}.accounts`),
  };
}

function normalizeUaidManifestsResponse(payload) {
  const record = ensureRecord(payload, "uaid manifests response");
  const uaid = normalizeUaidLiteral(record.uaid, "uaid manifests response.uaid");
  const manifestsValue = record.manifests ?? [];
  if (!Array.isArray(manifestsValue)) {
    throw new TypeError("uaid manifests response.manifests must be an array");
  }
  const manifests = manifestsValue.map((entry, index) =>
    normalizeUaidManifestRecord(
      entry,
      `uaid manifests response.manifests[${index}]`,
    ),
  );
  return { uaid, manifests };
}

function normalizeUaidManifestRecord(value, context) {
  const record = ensureRecord(value, context);
  const status = requireNonEmptyString(record.status, `${context}.status`);
  if (!UAID_MANIFEST_STATUS_VALUES.has(status)) {
    throw new TypeError(
      `${context}.status must be one of ${Array.from(UAID_MANIFEST_STATUS_VALUES).join(", ")}`,
    );
  }
  return {
    dataspace_id: ToriiClient._normalizeUnsignedInteger(
      record.dataspace_id,
      `${context}.dataspace_id`,
      { allowZero: true },
    ),
    dataspace_alias: optionalString(record.dataspace_alias, `${context}.dataspace_alias`),
    manifest_hash: normalizeHex32String(
      record.manifest_hash,
      `${context}.manifest_hash`,
    ),
    status,
    lifecycle: normalizeUaidManifestLifecycle(record.lifecycle, `${context}.lifecycle`),
    accounts: requireStringArray(record.accounts ?? [], `${context}.accounts`),
    manifest: normalizeUaidManifest(record.manifest, `${context}.manifest`),
  };
}

function normalizeUaidManifestLifecycle(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const activated = coerceOptionalInt(
    record.activated_epoch,
    `${context}.activated_epoch`,
  );
  const expired = coerceOptionalInt(record.expired_epoch, `${context}.expired_epoch`);
  const revocationValue = record.revocation ?? null;
  let revocation = null;
  if (revocationValue !== null && revocationValue !== undefined) {
    const rev = ensureRecord(revocationValue, `${context}.revocation`);
    revocation = {
      epoch: ToriiClient._normalizeUnsignedInteger(
        rev.epoch,
        `${context}.revocation.epoch`,
        { allowZero: true },
      ),
      reason: optionalString(rev.reason, `${context}.revocation.reason`),
    };
  }
  return {
    activated_epoch: activated,
    expired_epoch: expired,
    revocation,
  };
}

function normalizeUaidManifest(value, context) {
  const record = ensureRecord(value, context);
  const entriesValue = record.entries ?? [];
  if (!Array.isArray(entriesValue)) {
    throw new TypeError(`${context}.entries must be an array`);
  }
  return {
    version: requireNonEmptyString(record.version, `${context}.version`),
    uaid: normalizeUaidLiteral(record.uaid, `${context}.uaid`),
    dataspace: ToriiClient._normalizeUnsignedInteger(
      record.dataspace,
      `${context}.dataspace`,
      { allowZero: true },
    ),
    issued_ms: ToriiClient._normalizeUnsignedInteger(
      record.issued_ms,
      `${context}.issued_ms`,
      { allowZero: true },
    ),
    activation_epoch: ToriiClient._normalizeUnsignedInteger(
      record.activation_epoch,
      `${context}.activation_epoch`,
      { allowZero: true },
    ),
    expiry_epoch: coerceOptionalInt(record.expiry_epoch, `${context}.expiry_epoch`),
    entries: entriesValue.map((entry, index) =>
      normalizeUaidManifestEntry(entry, `${context}.entries[${index}]`),
    ),
  };
}

function normalizeUaidManifestEntry(value, context) {
  const record = ensureRecord(value, context);
  if (!isPlainObject(record.scope)) {
    throw new TypeError(`${context}.scope must be an object`);
  }
  if (!isPlainObject(record.effect)) {
    throw new TypeError(`${context}.effect must be an object`);
  }
  return {
    scope: record.scope,
    effect: record.effect,
    notes: optionalString(record.notes, `${context}.notes`),
  };
}

function normalizeProverFilterBoolean(value, name) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    if (value === 1) {
      return true;
    }
    if (value === 0) {
      return false;
    }
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true" || normalized === "1" || normalized === "yes") {
      return true;
    }
    if (normalized === "false" || normalized === "0" || normalized === "no") {
      return false;
    }
  }
  throw new TypeError(`${name} must be a boolean`);
}

function normalizeProverFilterString(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${name} must be a non-empty string`);
  }
  return trimmed;
}

function normalizeProverFilterEnum(value, name, allowed) {
  const normalized = normalizeProverFilterString(value, name).toLowerCase();
  if (!allowed.includes(normalized)) {
    throw new TypeError(`${name} must be one of: ${allowed.join(", ")}`);
  }
  return normalized;
}

function isTruthyFilter(filters, key) {
  if (!filters || typeof filters !== "object") {
    return false;
  }
  if (key in filters) {
    return Boolean(filters[key]);
  }
  const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
  if (camelKey in filters) {
    return Boolean(filters[camelKey]);
  }
  return false;
}

function extractExtraFields(record, recognizedKeys) {
  const extra = {};
  for (const [key, value] of Object.entries(record)) {
    if (!recognizedKeys.has(key)) {
      extra[key] = value;
    }
  }
  return extra;
}

function computeStatusMetrics(previous, current) {
  const metrics = {
    commit_latency_ms: current.commit_time_ms,
    queue_size: current.queue_size,
    queue_delta: previous ? current.queue_size - previous.queue_size : 0,
    da_reschedule_delta: previous
      ? Math.max(0, current.da_reschedule_total - previous.da_reschedule_total)
      : 0,
    tx_approved_delta: previous
      ? Math.max(0, current.txs_approved - previous.txs_approved)
      : 0,
    tx_rejected_delta: previous
      ? Math.max(0, current.txs_rejected - previous.txs_rejected)
      : 0,
    view_change_delta: previous
      ? Math.max(0, current.view_changes - previous.view_changes)
      : 0,
  };
  metrics.has_activity = Boolean(
    metrics.queue_delta ||
      metrics.da_reschedule_delta ||
      metrics.tx_approved_delta ||
      metrics.tx_rejected_delta ||
      metrics.view_change_delta,
  );
  return metrics;
}

function monotonicTimestamp() {
  if (
    typeof performance === "object" &&
    performance &&
    typeof performance.now === "function"
  ) {
    return performance.now();
  }
  if (
    typeof process === "object" &&
    process &&
    typeof process.hrtime === "function"
  ) {
    const [seconds, nanoseconds] = process.hrtime();
    return seconds * 1_000 + nanoseconds / 1_000_000;
  }
  return Date.now();
}

const PROVER_FILTER_DEFINITIONS = {
  ok_only: { type: "boolean", aliases: ["okOnly"] },
  failed_only: { type: "boolean", aliases: ["failedOnly"] },
  errors_only: { type: "boolean", aliases: ["errorsOnly"] },
  ids_only: { type: "boolean", aliases: ["idsOnly"] },
  messages_only: { type: "boolean", aliases: ["messagesOnly"] },
  latest: { type: "boolean", aliases: ["latest"] },
  content_type: { type: "string", aliases: ["contentType"] },
  has_tag: { type: "string", aliases: ["hasTag"] },
  id: { type: "string", aliases: ["id"] },
  limit: { type: "integer", aliases: ["limit"], allowZero: false },
  offset: { type: "integer", aliases: ["offset"], allowZero: true },
  since_ms: { type: "integer", aliases: ["sinceMs"], allowZero: true },
  before_ms: { type: "integer", aliases: ["beforeMs"], allowZero: true },
  order: { type: "enum", aliases: ["order"], values: ["asc", "desc"] },
};

const PROVER_FILTER_ALIAS_MAP = (() => {
  const map = new Map();
  for (const [key, spec] of Object.entries(PROVER_FILTER_DEFINITIONS)) {
    map.set(key, { key, spec });
    const aliases = Array.isArray(spec.aliases) ? spec.aliases : [];
    for (const alias of aliases) {
      map.set(alias, { key, spec });
    }
  }
  return map;
})();
const PROVER_REPORT_ITERATOR_OPTION_KEYS = (() => {
  const keys = new Set(["pageSize", "maxItems", "limit", "offset", "signal"]);
  for (const key of PROVER_FILTER_ALIAS_MAP.keys()) {
    keys.add(key);
  }
  return keys;
})();

async function* readBodyChunks(body) {
  if (!body) {
    return;
  }
  if (typeof body.getReader === "function") {
    const reader = body.getReader();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          break;
        }
        if (value) {
          yield value;
        }
      }
    } finally {
      if (typeof reader.releaseLock === "function") {
        reader.releaseLock();
      }
    }
    return;
  }
  if (typeof body[Symbol.asyncIterator] === "function") {
    for await (const chunk of body) {
      yield chunk;
    }
    return;
  }
  throw new Error("SSE response body is not async iterable");
}

function flushSseBuffer(buffer) {
  /** @type {Array<SseEvent>} */
  const events = [];
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const index = buffer.indexOf("\n\n");
    if (index === -1) {
      break;
    }
    const raw = buffer.slice(0, index);
    buffer = buffer.slice(index + 2);
    const parsed = ToriiClient._parseSseEvent(raw);
    if (parsed) {
      events.push(parsed);
    }
  }
  return { events, remainder: buffer };
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
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return Buffer.alloc(0);
    }
    return normalizeByteArray(value, "payload");
  }
  throw new TypeError("payload must be a Buffer or ArrayBuffer view");
}

function normalizeByteArray(value, context) {
  const bytes = value.map((entry, index) => {
    const numeric = Number(entry);
    if (!Number.isInteger(numeric) || numeric < 0 || numeric > 255) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${context}[${index}] must be an integer between 0 and 255`,
        `${context}[${index}]`,
      );
    }
    return numeric;
  });
  return Buffer.from(bytes);
}

function toXmlBuffer(value, name) {
  if (typeof value === "string") {
    if (!value.trim()) {
      throw new TypeError(`${name} must be a non-empty string or binary payload`);
    }
    return Buffer.from(value, "utf8");
  }
  return toBuffer(value);
}

function attachHeaderAccessors(headers) {
  if (!headers || typeof headers !== "object") {
    return;
  }
  if (typeof headers.get !== "function") {
    Object.defineProperty(headers, "get", {
      value(name) {
        const key = findHeaderKey(headers, name);
        return key ? headers[key] : undefined;
      },
      enumerable: false,
    });
  }
  if (typeof headers.has !== "function") {
    Object.defineProperty(headers, "has", {
      value(name) {
        return findHeaderKey(headers, name) !== null;
      },
      enumerable: false,
    });
  }
}

function findHeaderKey(headers, name) {
  if (!headers || typeof headers !== "object" || name == null) {
    return null;
  }
  const target = String(name).toLowerCase();
  for (const key of Object.keys(headers)) {
    if (key.toLowerCase() === target) {
      return key;
    }
  }
  return null;
}

function setHeader(headers, name, value) {
  if (name == null) {
    return;
  }
  const normalized = String(value);
  const existing = findHeaderKey(headers, name);
  const key = existing ?? String(name);
  headers[key] = normalized;
}

function deleteHeader(headers, name) {
  const key = findHeaderKey(headers, name);
  if (key) {
    delete headers[key];
  }
}

function hasHeader(headers, name) {
  return findHeaderKey(headers, name) !== null;
}

function cloneHeadersForFetch(headers) {
  const clone = {};
  if (headers && typeof headers === "object") {
    for (const [key, value] of Object.entries(headers)) {
      clone[key] = value;
    }
  }
  return clone;
}

function cloneRawUtf8Headers(headers) {
  if (!headers || typeof headers !== "object") {
    return undefined;
  }
  const clone = {};
  for (const [key, value] of Object.entries(headers)) {
    clone[key] = String(value);
  }
  return Object.keys(clone).length > 0 ? clone : undefined;
}

function headerValueRequiresRawUtf8Transport(value) {
  if (value == null) {
    return false;
  }
  const rendered = String(value);
  for (let index = 0; index < rendered.length; index += 1) {
    if (rendered.charCodeAt(index) > 0xff) {
      return true;
    }
  }
  return false;
}

function fetchSupportsRawUtf8Headers(fetchImpl) {
  return Boolean(fetchImpl && fetchImpl[RAW_UTF8_HEADER_SUPPORT_FLAG] === true);
}

function canUseNodeRawUtf8Transport(url) {
  const parsed = url instanceof URL ? url : new URL(String(url));
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return false;
  }
  return typeof process !== "undefined" && Boolean(process?.versions?.node);
}

async function performNodeRawUtf8Request({
  url,
  method,
  headers,
  rawUtf8Headers,
  body,
  signal,
}) {
  const parsed = url instanceof URL ? url : new URL(String(url));
  const requestBuffer = buildRawUtf8HttpRequestBuffer({
    url: parsed,
    method,
    headers,
    rawUtf8Headers,
    body,
  });
  throwIfAborted(signal);
  const socket = await openNodeRawUtf8Socket(parsed);
  if (typeof socket.setNoDelay === "function") {
    socket.setNoDelay(true);
  }
  return new Promise((resolve, reject) => {
    let settled = false;
    const chunks = [];
    const finishWithError = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      reject(error instanceof Error ? error : new Error(String(error)));
    };
    const finishWithResponse = () => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      try {
        resolve(createNodeRawUtf8Response(Buffer.concat(chunks)));
      } catch (error) {
        reject(error);
      }
    };
    const onAbort = () => {
      const reason = signal?.reason ?? createAbortError();
      try {
        socket.destroy(reason instanceof Error ? reason : undefined);
      } catch {
        // Ignore socket teardown failures during abort.
      }
      finishWithError(reason);
    };
    const cleanup = () => {
      socket.removeListener("error", onError);
      socket.removeListener("data", onData);
      socket.removeListener("end", onEnd);
      socket.removeListener("close", onClose);
      if (signal) {
        signal.removeEventListener("abort", onAbort);
      }
    };
    const onError = (error) => {
      finishWithError(error);
    };
    const onData = (chunk) => {
      chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    };
    const onEnd = () => {
      finishWithResponse();
    };
    const onClose = (hadError) => {
      if (settled || hadError) {
        return;
      }
      if (chunks.length > 0) {
        finishWithResponse();
        return;
      }
      finishWithError(new Error("raw UTF-8 request socket closed before a response arrived"));
    };
    socket.once("error", onError);
    socket.on("data", onData);
    socket.once("end", onEnd);
    socket.once("close", onClose);
    if (signal) {
      signal.addEventListener("abort", onAbort, { once: true });
    }
    try {
      socket.write(requestBuffer);
    } catch (error) {
      finishWithError(error);
    }
  });
}

async function openNodeRawUtf8Socket(url) {
  const port =
    url.port !== "" ? Number(url.port) : url.protocol === "https:" ? 443 : 80;
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`invalid port for raw UTF-8 request: ${url.port}`);
  }
  if (url.protocol === "https:") {
    const tls = await import("node:tls");
    return tls.connect({
      host: url.hostname,
      port,
      ALPNProtocols: ["http/1.1"],
      servername: url.hostname.includes(":") ? undefined : url.hostname,
    });
  }
  const net = await import("node:net");
  return net.createConnection({
    host: url.hostname,
    port,
  });
}

function buildRawUtf8HttpRequestBuffer({
  url,
  method,
  headers,
  rawUtf8Headers,
  body,
}) {
  const renderedHeaders = cloneHeadersForFetch(headers);
  const rawHeaders = cloneRawUtf8Headers(rawUtf8Headers) ?? {};
  for (const name of Object.keys(rawHeaders)) {
    deleteHeader(renderedHeaders, name);
  }
  if (!hasHeader(renderedHeaders, "Host")) {
    renderedHeaders.Host = renderRawHttpHostHeader(url);
  }
  if (!hasHeader(renderedHeaders, "Connection")) {
    renderedHeaders.Connection = "close";
  }
  const bodyBuffer = encodeRawUtf8RequestBody(body);
  if (
    bodyBuffer &&
    !hasHeader(renderedHeaders, "Content-Length") &&
    !hasHeader(renderedHeaders, "Transfer-Encoding")
  ) {
    renderedHeaders["Content-Length"] = String(bodyBuffer.length);
  }
  const target = `${url.pathname || "/"}${url.search || ""}` || "/";
  const chunks = [Buffer.from(`${method} ${target} HTTP/1.1\r\n`, "ascii")];
  for (const [name, value] of Object.entries(renderedHeaders)) {
    chunks.push(encodeRawHttpHeaderLine(name, value));
  }
  for (const [name, value] of Object.entries(rawHeaders)) {
    chunks.push(encodeRawHttpHeaderLine(name, value));
  }
  chunks.push(Buffer.from("\r\n", "ascii"));
  if (bodyBuffer) {
    chunks.push(bodyBuffer);
  }
  return Buffer.concat(chunks);
}

function encodeRawUtf8RequestBody(body) {
  if (body === undefined || body === null) {
    return null;
  }
  if (typeof body === "string") {
    return Buffer.from(body, "utf8");
  }
  if (body instanceof URLSearchParams) {
    return Buffer.from(body.toString(), "utf8");
  }
  return toBuffer(body);
}

function renderRawHttpHostHeader(url) {
  const defaultPort = url.protocol === "https:" ? "443" : "80";
  const hostname = url.hostname.includes(":") ? `[${url.hostname}]` : url.hostname;
  if (url.port && url.port !== defaultPort) {
    return `${hostname}:${url.port}`;
  }
  return hostname;
}

function encodeRawHttpHeaderLine(name, value) {
  const renderedName = String(name);
  if (!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/u.test(renderedName)) {
    throw new TypeError(`invalid raw HTTP header name: ${renderedName}`);
  }
  const renderedValue = value == null ? "" : String(value);
  if (/[\r\n]/u.test(renderedValue)) {
    throw new TypeError(`invalid raw HTTP header value for ${renderedName}`);
  }
  return Buffer.concat([
    Buffer.from(`${renderedName}: `, "ascii"),
    Buffer.from(renderedValue, "utf8"),
    Buffer.from("\r\n", "ascii"),
  ]);
}

function createNodeRawUtf8Response(buffer) {
  const parsed = parseRawHttpResponse(buffer);
  if (typeof Response === "function") {
    return new Response(parsed.body, {
      status: parsed.status,
      statusText: parsed.statusText,
      headers: parsed.headers,
    });
  }
  attachHeaderAccessors(parsed.headers);
  return {
    status: parsed.status,
    statusText: parsed.statusText,
    headers: parsed.headers,
    body: null,
    json: async () => JSON.parse(parsed.body.toString("utf8")),
    text: async () => parsed.body.toString("utf8"),
    arrayBuffer: async () =>
      parsed.body.buffer.slice(
        parsed.body.byteOffset,
        parsed.body.byteOffset + parsed.body.byteLength,
      ),
  };
}

function parseRawHttpResponse(buffer) {
  const headerSeparator = buffer.indexOf("\r\n\r\n");
  if (headerSeparator === -1) {
    throw new Error("invalid raw HTTP response: missing header terminator");
  }
  const headerBlock = buffer.subarray(0, headerSeparator).toString("latin1");
  const lines = headerBlock.split("\r\n");
  const statusLine = lines.shift();
  const match =
    typeof statusLine === "string"
      ? /^HTTP\/1\.[01]\s+(\d{3})(?:\s+(.*))?$/u.exec(statusLine)
      : null;
  if (!match) {
    throw new Error("invalid raw HTTP response: malformed status line");
  }
  const headers = {};
  for (const line of lines) {
    const delimiter = line.indexOf(":");
    if (delimiter === -1) {
      continue;
    }
    const name = line.slice(0, delimiter).trim();
    const value = line.slice(delimiter + 1).trim();
    const existing = findHeaderKey(headers, name);
    if (existing) {
      headers[existing] = `${headers[existing]}, ${value}`;
    } else {
      headers[name] = value;
    }
  }
  let body = buffer.subarray(headerSeparator + 4);
  const status = Number.parseInt(match[1], 10);
  if (status < 200 || status === 204 || status === 304) {
    body = Buffer.alloc(0);
  } else if (responseUsesChunkedTransferEncoding(headers)) {
    body = decodeChunkedHttpBody(body);
  } else {
    const contentLength = parseContentLengthHeader(headers);
    if (contentLength !== null) {
      if (body.length < contentLength) {
        throw new Error("invalid raw HTTP response: body shorter than content-length");
      }
      body = body.subarray(0, contentLength);
    }
  }
  return {
    status,
    statusText: match[2] ?? "",
    headers,
    body,
  };
}

function responseUsesChunkedTransferEncoding(headers) {
  const key = findHeaderKey(headers, "transfer-encoding");
  if (!key) {
    return false;
  }
  return String(headers[key])
    .split(",")
    .some((entry) => entry.trim().toLowerCase() === "chunked");
}

function parseContentLengthHeader(headers) {
  const key = findHeaderKey(headers, "content-length");
  if (!key) {
    return null;
  }
  const parsed = Number.parseInt(String(headers[key]).trim(), 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error("invalid raw HTTP response: malformed content-length");
  }
  return parsed;
}

function decodeChunkedHttpBody(buffer) {
  const chunks = [];
  let offset = 0;
  while (offset < buffer.length) {
    const lineEnd = buffer.indexOf("\r\n", offset);
    if (lineEnd === -1) {
      throw new Error("invalid raw HTTP response: unterminated chunk size");
    }
    const sizeLine = buffer
      .subarray(offset, lineEnd)
      .toString("latin1")
      .split(";", 1)[0]
      .trim();
    const size = Number.parseInt(sizeLine, 16);
    if (!Number.isInteger(size) || size < 0) {
      throw new Error("invalid raw HTTP response: malformed chunk size");
    }
    offset = lineEnd + 2;
    if (size === 0) {
      if (offset + 2 > buffer.length) {
        throw new Error("invalid raw HTTP response: missing chunk terminator");
      }
      return Buffer.concat(chunks);
    }
    if (offset + size + 2 > buffer.length) {
      throw new Error("invalid raw HTTP response: truncated chunk body");
    }
    chunks.push(buffer.subarray(offset, offset + size));
    offset += size;
    if (buffer[offset] !== 13 || buffer[offset + 1] !== 10) {
      throw new Error("invalid raw HTTP response: malformed chunk delimiter");
    }
    offset += 2;
  }
  throw new Error("invalid raw HTTP response: missing terminating chunk");
}

function headersContainCredentials(headers) {
  if (!headers || typeof headers !== "object") {
    return false;
  }
  return (
    hasHeader(headers, "authorization") ||
    hasHeader(headers, "x-api-token")
  );
}

function bodyContainsSensitiveKeyMaterial(body, headers) {
  if (body == null) {
    return false;
  }
  const contentTypeKey = findHeaderKey(headers, "content-type");
  const contentType =
    contentTypeKey && headers[contentTypeKey] != null
      ? String(headers[contentTypeKey]).toLowerCase()
      : null;
  const shouldInspectAsText =
    typeof body === "string" || Boolean(contentType && contentType.includes("json"));
  if (!shouldInspectAsText) {
    return false;
  }
  let text;
  if (typeof body === "string") {
    text = body;
  } else if (Buffer.isBuffer(body)) {
    text = body.toString("utf8");
  } else if (body instanceof Uint8Array) {
    text = Buffer.from(body).toString("utf8");
  } else if (ArrayBuffer.isView(body)) {
    text = Buffer.from(body.buffer, body.byteOffset, body.byteLength).toString("utf8");
  } else if (body instanceof ArrayBuffer) {
    text = Buffer.from(body).toString("utf8");
  } else {
    return false;
  }
  return /"(?:private_key|privateKey|private_key_hex|privateKeyHex|private_key_bytes|privateKeyBytes|private_key_multihash|privateKeyMultihash)"\s*:/u.test(
    text,
  );
}

function delay(ms, signal) {
  if (!signal) {
    return new Promise((resolve) => {
      setTimeout(resolve, ms);
    });
  }
  throwIfAborted(signal);
  return new Promise((resolve, reject) => {
    const onAbort = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      reject(signal.reason ?? new Error("The operation was aborted"));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function createAbortError() {
  if (typeof DOMException === "function") {
    return new DOMException("The operation was aborted", "AbortError");
  }
  const error = new Error("The operation was aborted");
  error.name = "AbortError";
  return error;
}

function throwIfAborted(signal) {
  if (!signal) {
    return;
  }
  if (typeof signal.throwIfAborted === "function") {
    signal.throwIfAborted();
    return;
  }
  if (signal.aborted) {
    throw (
      signal.reason ??
      new Error("The operation was aborted")
    );
  }
}

const CONNECT_SID_BYTES = 32;
const BASE64URL_BODY_PATTERN = /^[A-Za-z0-9_-]+$/;

function normalizeConnectSid(value, name) {
  const trimmed = requireNonEmptyString(value, name);
  const hexBody =
    trimmed.startsWith("0x") || trimmed.startsWith("0X")
      ? trimmed.slice(2)
      : trimmed;
  if (
    hexBody.length === CONNECT_SID_BYTES * 2 &&
    /^[0-9a-fA-F]+$/.test(hexBody)
  ) {
    return hexBody.toLowerCase();
  }
  const { body, padding } = splitBase64Url(trimmed);
  if (!body || !BASE64URL_BODY_PATTERN.test(body)) {
    throw connectSidTypeError(name);
  }
  if (padding) {
    throw connectSidTypeError(name);
  }
  let decoded;
  try {
    decoded = decodeBase64UrlBody(body);
  } catch {
    throw connectSidTypeError(name);
  }
  if (decoded.length !== CONNECT_SID_BYTES) {
    throw connectSidTypeError(name);
  }
  return trimmed;
}

function connectSidTypeError(name) {
  return new TypeError(
    `${name} must be a 32-byte base64url string (no padding) or hex string`,
  );
}

function splitBase64Url(value) {
  const match = value.match(/=+$/u);
  if (!match) {
    return { body: value, padding: "" };
  }
  const padding = match[0];
  return {
    body: value.slice(0, -padding.length),
    padding,
  };
}

function decodeBase64UrlBody(body) {
  let padded = body.replace(/-/g, "+").replace(/_/g, "/");
  const remainder = padded.length % 4;
  if (remainder === 1) {
    throw new TypeError("invalid base64url length");
  }
  if (remainder === 2) {
    padded += "==";
  } else if (remainder === 3) {
    padded += "=";
  }
  return Buffer.from(padded, "base64");
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a string`,
      name,
    );
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must not be empty`,
      name,
    );
  }
  return trimmed;
}

function requireHexString(value, name) {
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  const normalized = value.trim();
  if (!normalized) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  const hasPrefix =
    normalized.startsWith("0x") || normalized.startsWith("0X");
  const hex = hasPrefix ? normalized.slice(2) : normalized;
  if (hex.length === 0 || hex.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(hex)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  return normalized;
}

function normalizeStorageTicketHex(value, name) {
  const normalized = requireHexString(value, name);
  const hex = normalized.startsWith("0x") || normalized.startsWith("0X")
    ? normalized.slice(2)
    : normalized;
  if (hex.length !== 64) {
    throw new TypeError(`${name} must contain 64 hex characters`);
  }
  return hex.toLowerCase();
}

function coerceIntegerLike(value, context) {
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isSafeInteger(value)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return value;
  }
  if (typeof value === "bigint") {
    const numeric = Number(value);
    if (!Number.isSafeInteger(numeric)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return numeric;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw new TypeError(`${context} must be an integer`);
    }
    const numeric = Number(trimmed);
    if (!Number.isFinite(numeric) || !Number.isSafeInteger(numeric)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return numeric;
  }
  throw new TypeError(`${context} must be an integer`);
}

function requireNonNegativeIntegerLike(value, context) {
  if (value === undefined || value === null) {
    throw new TypeError(`${context} is required`);
  }
  const numeric = coerceIntegerLike(value, context);
  if (numeric < 0) {
    throw new RangeError(`${context} must be >= 0`);
  }
  return numeric;
}

function requireEvidencePhase(value, context) {
  const phase = requireNonEmptyString(value, context);
  if (!EVIDENCE_PHASE_VALUES.has(phase)) {
    throw new RangeError(
      `${context} must be one of ${Array.from(EVIDENCE_PHASE_VALUES).join(", ")}`,
    );
  }
  return phase;
}

let cachedSorafsNative;

function getSorafsNativeBinding() {
  if (cachedSorafsNative === undefined) {
    cachedSorafsNative = getNativeBinding();
  }
  return cachedSorafsNative;
}

function requireSorafsNativeBinding() {
  const binding = getSorafsNativeBinding();
  if (
    !binding ||
    typeof binding.sorafsEvaluateAliasProof !== "function" ||
    typeof binding.sorafsAliasPolicyDefaults !== "function" ||
    typeof binding.sorafsDecodeReplicationOrder !== "function"
  ) {
    throw new Error(
      "SoraFS helpers require the native iroha_js_host module. Run `npm run build:native` before using alias validation or replication order decoding.",
    );
  }
  return binding;
}

function pickOverride(source, snakeName, camelName) {
  if (!source || typeof source !== "object") {
    return undefined;
  }
  if (Object.prototype.hasOwnProperty.call(source, snakeName)) {
    return source[snakeName];
  }
  if (Object.prototype.hasOwnProperty.call(source, camelName)) {
    return source[camelName];
  }
  return undefined;
}

function formatSorafsEvaluation(evaluation) {
  if (!evaluation || typeof evaluation !== "object") {
    return {
      state: null,
      statusLabel: null,
      rotationDue: false,
      ageSeconds: null,
      generatedAtUnix: null,
      expiresAtUnix: null,
      expiresInSeconds: null,
    };
  }
  return {
    state: typeof evaluation.state === "string" ? evaluation.state : null,
    statusLabel:
      typeof evaluation.status_label === "string" ? evaluation.status_label : null,
    rotationDue: Boolean(evaluation.rotation_due),
    ageSeconds:
      typeof evaluation.age_seconds === "number" ? evaluation.age_seconds : null,
    generatedAtUnix:
      typeof evaluation.generated_at_unix === "number"
        ? evaluation.generated_at_unix
        : null,
    expiresAtUnix:
      typeof evaluation.expires_at_unix === "number" ? evaluation.expires_at_unix : null,
    expiresInSeconds:
      typeof evaluation.expires_in_seconds === "number"
        ? evaluation.expires_in_seconds
        : null,
    servable: evaluation.servable === true,
  };
}

function normalizeAuthorityCredentials(source, context) {
  const record = ensureRecord(source, context);
  const authority = ToriiClient._normalizeAccountId(
    record.authority,
    `${context}.authority`,
  );
  const privateKey = resolveAuthorityPrivateKey(record, context);
  return {
    authority,
    private_key: privateKey,
  };
}

function resolveAuthorityPrivateKey(record, context) {
  const direct = pickOverride(record, "private_key", "privateKey");
  if (direct !== undefined && direct !== null) {
    if (typeof direct === "string") {
      return requireNonEmptyString(direct, `${context}.privateKey`);
    }
    return formatAuthorityPrivateKeyBytes(direct, record, context);
  }
  const multihash = pickOverride(
    record,
    "private_key_multihash",
    "privateKeyMultihash",
  );
  if (multihash !== undefined && multihash !== null) {
    return requireNonEmptyString(multihash, `${context}.privateKeyMultihash`);
  }
  const hexInput = pickOverride(record, "private_key_hex", "privateKeyHex");
  if (hexInput !== undefined && hexInput !== null) {
    return formatAuthorityPrivateKeyHex(hexInput, record, context, "privateKeyHex");
  }
  const bytesInput = pickOverride(
    record,
    "private_key_bytes",
    "privateKeyBytes",
  );
  if (bytesInput !== undefined && bytesInput !== null) {
    return formatAuthorityPrivateKeyBytes(bytesInput, record, context);
  }
  throw new TypeError(`${context}.privateKey is required`);
}

function formatAuthorityPrivateKeyBytes(value, record, context) {
  const hex = normalizeHex32String(value, `${context}.privateKeyBytes`);
  return formatAuthorityPrivateKeyHex(hex, record, context, "privateKeyBytes");
}

function formatAuthorityPrivateKeyHex(value, record, context, label) {
  const hex = normalizeHex32String(value, `${context}.${label}`);
  const algorithm =
    record.private_key_algorithm ?? record.privateKeyAlgorithm ?? "ed25519";
  const normalizedAlgorithm = requireNonEmptyString(
    algorithm,
    `${context}.private_key_algorithm`,
  );
  return `${normalizedAlgorithm}:${hex}`;
}

function normalizeManifestPayload(manifest, context) {
  if (!isPlainObject(manifest)) {
    throw new TypeError(`${context} must be an object`);
  }
  const hasField = (...keys) =>
    keys.some((key) => Object.prototype.hasOwnProperty.call(manifest, key));
  const getField = (...keys) => {
    for (const key of keys) {
      if (Object.prototype.hasOwnProperty.call(manifest, key)) {
        return manifest[key];
      }
    }
    return undefined;
  };
  const normalized = {
    code_hash: null,
    abi_hash: null,
    compiler_fingerprint: null,
    features_bitmap: null,
    access_set_hints: null,
    entrypoints: null,
    kotoba: null,
    provenance: null,
  };
  if (hasField("code_hash", "codeHash")) {
    normalized.code_hash = normalizeOptionalHex32(
      getField("code_hash", "codeHash"),
      `${context}.code_hash`,
    );
  }
  if (hasField("abi_hash", "abiHash")) {
    normalized.abi_hash = normalizeOptionalHex32(
      getField("abi_hash", "abiHash"),
      `${context}.abi_hash`,
    );
  }
  if (hasField("compiler_fingerprint", "compilerFingerprint")) {
    const fingerprint = getField("compiler_fingerprint", "compilerFingerprint") ?? null;
    normalized.compiler_fingerprint =
      fingerprint === null
        ? null
        : requireNonEmptyString(fingerprint, `${context}.compiler_fingerprint`);
  }
  if (hasField("features_bitmap", "featuresBitmap")) {
    const features = getField("features_bitmap", "featuresBitmap") ?? null;
    normalized.features_bitmap =
      features === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            features,
            `${context}.features_bitmap`,
            { allowZero: true },
          );
  }
  if (hasField("access_set_hints", "accessSetHints")) {
    const hints = getField("access_set_hints", "accessSetHints");
    normalized.access_set_hints =
      hints === null
        ? null
        : normalizeAccessSetHintsPayload(hints, `${context}.access_set_hints`);
  }
  if (hasField("entrypoints", "entryPoints")) {
    const entries = getField("entrypoints", "entryPoints");
    if (entries === null) {
      normalized.entrypoints = null;
    } else if (!Array.isArray(entries)) {
      throw new TypeError(`${context}.entrypoints must be an array`);
    } else {
      normalized.entrypoints = entries.map((entry, index) => {
        if (!isPlainObject(entry)) {
          throw new TypeError(`${context}.entrypoints[${index}] must be an object`);
        }
        return entry;
      });
    }
  }
  if (hasField("kotoba")) {
    const kotoba = getField("kotoba");
    normalized.kotoba =
      kotoba === null ? null : normalizeManifestKotobaPayload(kotoba, `${context}.kotoba`);
  }
  if (hasField("provenance")) {
    const provenance = getField("provenance");
    normalized.provenance =
      provenance === null
        ? null
        : normalizeManifestProvenancePayload(provenance, `${context}.provenance`);
  }
  return normalized;
}

function normalizeManifestKotobaPayload(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => {
    const record = ensureRecord(entry, `${context}[${index}]`);
    return {
      msg_id: requireNonEmptyString(
        record.msg_id ?? record.msgId,
        `${context}[${index}].msg_id`,
      ),
      translations: cloneJsonValue(
        record.translations,
        `${context}[${index}].translations`,
      ),
    };
  });
}

function normalizeManifestPublicKeyPayload(value, context) {
  const literal = requireNonEmptyString(value, context).trim();
  let prefixedAlgorithm = null;
  let multihashLiteral = literal;
  const separator = literal.indexOf(":");
  if (separator > 0) {
    prefixedAlgorithm = literal.slice(0, separator).trim().toLowerCase();
    multihashLiteral = literal.slice(separator + 1);
  }
  const canonical = canonicalizeMultihashHex(multihashLiteral, context);
  const bytes = Buffer.from(canonical, "hex");
  const functionCode = decodeVarintBuffer(bytes, 0, context);
  const digestLength = decodeVarintBuffer(bytes, functionCode.nextIndex, context);
  const payload = bytes.subarray(digestLength.nextIndex);
  if (payload.length !== digestLength.value) {
    throw new Error(`${context} multihash payload length does not match its digest header`);
  }
  const entry = getCurveEntryByPublicKeyMulticodec(functionCode.value);
  if (!entry) {
    throw new Error(
      `${context} uses unsupported multihash code 0x${functionCode.value.toString(16)}`,
    );
  }
  if (
    prefixedAlgorithm &&
    prefixedAlgorithm !== entry.algorithm &&
    !(prefixedAlgorithm === "mldsa" && entry.algorithm === "ml-dsa")
  ) {
    throw new Error(`${context} algorithm prefix does not match the multihash payload`);
  }
  const fnHex = bytes.subarray(0, functionCode.nextIndex).toString("hex");
  const lenHex = bytes.subarray(functionCode.nextIndex, digestLength.nextIndex).toString("hex");
  const payloadHex = payload.toString("hex").toUpperCase();
  return `${fnHex}${lenHex}${payloadHex}`;
}

function normalizeManifestSignaturePayload(value, context) {
  if (Buffer.isBuffer(value) || value instanceof Uint8Array) {
    return Buffer.from(value).toString("hex").toUpperCase();
  }
  if (Array.isArray(value)) {
    return normalizeByteArray(value, context).toString("hex").toUpperCase();
  }
  const literal = requireNonEmptyString(value, context).trim();
  const body =
    literal.includes(":") && literal.indexOf(":") > 0
      ? literal.slice(literal.indexOf(":") + 1)
      : literal;
  if (body.length === 0 || body.length % 2 !== 0 || !/^[0-9A-Fa-f]+$/u.test(body)) {
    throw new TypeError(`${context} must be an even-length hexadecimal string`);
  }
  return body.toUpperCase();
}

function normalizeManifestProvenancePayload(value, context) {
  const record = ensureRecord(value, context);
  return {
    signer: normalizeManifestPublicKeyPayload(record.signer, `${context}.signer`),
    signature: normalizeManifestSignaturePayload(record.signature, `${context}.signature`),
  };
}

function normalizeAccessSetHintsPayload(payload, context) {
  const record = ensureRecord(payload, context);
  const normalizeKeys = (value, name) => {
    if (value === undefined || value === null) {
      return [];
    }
    if (!Array.isArray(value)) {
      throw new TypeError(`${name} must be an array of strings`);
    }
    return value.map((entry, index) =>
      requireNonEmptyString(entry, `${name}[${index}]`),
    );
  };
  return {
    read_keys: normalizeKeys(
      record.read_keys ?? record.readKeys,
      `${context}.read_keys`,
    ),
    write_keys: normalizeKeys(
      record.write_keys ?? record.writeKeys,
      `${context}.write_keys`,
    ),
  };
}

function normalizeOptionalBase64Payload(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeRequiredBase64Payload(value, name);
}

function normalizeRequiredBase64Payload(value, name) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a non-empty base64 string`,
        name,
      );
    }
    try {
      const decoded = strictDecodeBase64(trimmed);
      return Buffer.from(decoded).toString("base64");
    } catch (error) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a valid base64 string`,
        name,
        error instanceof Error ? error : undefined,
      );
    }
  }
  const buffer = toBuffer(value);
  if (buffer.length === 0) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a non-empty base64 string`,
      name,
    );
  }
  return Buffer.from(buffer).toString("base64");
}

function normalizeBase64Token(value, name) {
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a string`,
      name,
    );
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a non-empty base64 string`,
      name,
    );
  }
  const normalized = tryNormalizeBase64String(trimmed);
  if (normalized) {
    return normalized;
  }
  const base64UrlCandidate = trimmed.replace(/-/g, "+").replace(/_/g, "/");
  if (base64UrlCandidate !== trimmed) {
    const urlNormalized = tryNormalizeBase64String(base64UrlCandidate);
    if (urlNormalized) {
      return urlNormalized;
    }
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_STRING,
    `${name} must be a valid base64 or base64url string`,
    name,
  );
}

function tryNormalizeBase64String(value) {
  try {
    const decoded = strictDecodeBase64(value);
    return Buffer.from(decoded).toString("base64");
  } catch {
    return null;
  }
}

function normalizeOptionalHex32(value, name) {
  if (value === undefined) {
    return undefined;
  }
  if (value === null) {
    return null;
  }
  return normalizeHex32String(value, name);
}

function normalizeHex32String(value, name, options = {}) {
  const allowShort = options.allowShort === true;
  const allowScheme = options.allowScheme === true;
  const schemeName =
    typeof options.scheme === "string" && options.scheme.trim()
      ? options.scheme.trim().toLowerCase()
      : "blake2b32";
  if (Buffer.isBuffer(value)) {
    return normalizeHex32String(value.toString("hex"), name, options);
  }
  if (ArrayBuffer.isView(value)) {
    return normalizeHex32String(
      Buffer.from(value.buffer, value.byteOffset, value.byteLength).toString("hex"),
      name,
      options,
    );
  }
  if (value instanceof ArrayBuffer) {
    return normalizeHex32String(Buffer.from(value).toString("hex"), name, options);
  }
  if (Array.isArray(value)) {
    return normalizeHex32String(normalizeByteArray(value, name).toString("hex"), name, options);
  }
  let normalized = requireNonEmptyString(value, name);
  if (allowScheme && normalized.includes(":")) {
    const [scheme, rest] = normalized.split(":", 2);
    if (scheme && scheme.toLowerCase() !== schemeName) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${name} must be a 32-byte hex string`,
        name,
      );
    }
    normalized = rest.trim();
  }
  const hex =
    normalized.startsWith("0x") || normalized.startsWith("0X")
      ? normalized.slice(2)
      : normalized;
  const isValidLength =
    allowShort && hex.length > 0 && hex.length % 2 === 0 ? hex.length <= 64 : hex.length === 64;
  if (!isValidLength || !/^[0-9a-fA-F]+$/.test(hex)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a 32-byte hex string`,
      name,
    );
  }
  return hex.toLowerCase();
}

function normalizeHashLike32(value, name, options = {}) {
  if (Buffer.isBuffer(value)) {
    return normalizeHex32String(value.toString("hex"), name, options);
  }
  if (ArrayBuffer.isView(value)) {
    return normalizeHex32String(
      Buffer.from(value.buffer, value.byteOffset, value.byteLength).toString("hex"),
      name,
      options,
    );
  }
  if (value instanceof ArrayBuffer) {
    return normalizeHex32String(Buffer.from(value).toString("hex"), name, options);
  }
  if (Array.isArray(value)) {
    return normalizeHex32String(normalizeByteArray(value, name).toString("hex"), name, options);
  }
  const normalized = requireNonEmptyString(value, name);
  const trimmed = normalized.trim();
  if (/^hash:[0-9a-fA-F]{64}#[0-9a-fA-F]{4}$/.test(trimmed)) {
    return parseHashLiteralToHex(trimmed, name);
  }
  return normalizeHex32String(trimmed, name, options);
}

function parseHashLiteralToHex(literal, name) {
  const match = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/.exec(literal);
  if (!match) {
    throw new TypeError(
      `${name} must be a canonical "hash:<HEX>#<CRC>" literal or hex string`,
    );
  }
  const [, body, checksum] = match;
  const expected = computeHashLiteralCrc("hash", body.toUpperCase());
  if (expected !== checksum.toUpperCase()) {
    throw new TypeError(`${name} has invalid checksum; expected ${expected}`);
  }
  return body.toLowerCase();
}

function formatHashLiteral(bodyHex) {
  const upper = bodyHex.toUpperCase();
  const checksum = computeHashLiteralCrc("hash", upper);
  return `hash:${upper}#${checksum}`;
}

function normalizeOptionalHashLiteral(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  const hex = normalizeHashLike32(value, name);
  return formatHashLiteral(hex);
}

function computeHashLiteralCrc(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };
  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return (crc & 0xffff).toString(16).toUpperCase().padStart(4, "0");
}

function normalizeNumericLiteral(value, name, { allowNegative = false } = {}) {
  let raw;
  if (typeof value === "string") {
    raw = value.trim();
  } else if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${name} must be a finite number`);
    }
    raw = value.toString();
  } else if (typeof value === "bigint") {
    raw = value.toString();
  } else {
    throw new TypeError(`${name} must be a string, number, or bigint`);
  }

  if (!raw) {
    throw new TypeError(`${name} must be a valid Numeric literal`);
  }

  let digits = raw;
  const sign = digits[0];
  if (sign === "-" || sign === "+") {
    if (sign === "-" && !allowNegative) {
      throw new TypeError(`${name} must be non-negative`);
    }
    digits = digits.slice(1);
  }
  if (!digits) {
    throw new TypeError(`${name} must be a valid Numeric literal`);
  }

  let seenDot = false;
  let scale = 0;
  let mantissa = "";
  for (const ch of digits) {
    if (ch === ".") {
      if (seenDot) {
        throw new TypeError(`${name} must be a valid Numeric literal`);
      }
      seenDot = true;
      continue;
    }
    if (ch < "0" || ch > "9") {
      throw new TypeError(`${name} must be a valid Numeric literal`);
    }
    mantissa += ch;
    if (seenDot) {
      scale += 1;
    }
  }
  if (!mantissa) {
    throw new TypeError(`${name} must be a valid Numeric literal`);
  }
  if (scale > MAX_NUMERIC_SCALE) {
    throw new TypeError(`${name} scale exceeds ${MAX_NUMERIC_SCALE} decimal places`);
  }

  let mantissaValue = BigInt(mantissa);
  if (sign === "-") {
    mantissaValue = -mantissaValue;
  }
  const absValue = mantissaValue < 0n ? -mantissaValue : mantissaValue;
  if (absValue !== 0n && absValue.toString(2).length > MAX_NUMERIC_BITS) {
    throw new TypeError(`${name} mantissa exceeds ${MAX_NUMERIC_BITS} bits`);
  }

  return raw;
}

function normalizeNumericString(value, name) {
  return normalizeNumericLiteral(value, name, { allowNegative: false });
}

function normalizeAmountLike(value, name) {
  if (value === undefined || value === null) {
    throw new TypeError(`${name} is required`);
  }
  return normalizeNumericLiteral(value, name, { allowNegative: false });
}

function normalizeOptionalHexString(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return requireHexString(value, name);
}

function cloneJsonValue(value, path) {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${path} must not contain non-finite numbers`);
    }
    return value;
  }
  if (typeof value === "bigint") {
    return value.toString(10);
  }
  if (Array.isArray(value)) {
    return value.map((entry, index) => cloneJsonValue(entry, `${path}[${index}]`));
  }
  if (typeof value === "object") {
    const result = {};
    for (const [key, nested] of Object.entries(value)) {
      if (typeof key !== "string" || key.length === 0) {
        throw new TypeError(`${path} keys must be non-empty strings`);
      }
      result[key] = cloneJsonValue(nested, `${path}.${key}`);
    }
    return result;
  }
  throw new TypeError(`${path} contains unsupported value type: ${typeof value}`);
}

function normalizeSpaceDirectoryManifestPayload(input, context) {
  const manifest = cloneJsonValue(input, context);
  const normalized = {};
  const versionRaw = manifest.version ?? manifest.Version;
  const version = requireNonEmptyString(
    versionRaw,
    `${context}.version`,
  ).trim();
  if (!version) {
    throw new TypeError(`${context}.version must be a non-empty string`);
  }
  normalized.version = version;
  const uaidLiteral =
    manifest.uaid ?? manifest.uaid_literal ?? manifest.uaidLiteral;
  normalized.uaid = normalizeUaidLiteral(uaidLiteral, `${context}.uaid`);
  const dataspaceRaw =
    manifest.dataspace ?? manifest.dataspace_id ?? manifest.dataspaceId;
  normalized.dataspace = ToriiClient._normalizeUnsignedInteger(
    dataspaceRaw,
    `${context}.dataspace`,
  );
  const issuedMs = normalizeOptionalUnsignedInteger(
    manifest.issued_ms ?? manifest.issuedMs,
    `${context}.issued_ms`,
  );
  if (issuedMs !== undefined) {
    normalized.issued_ms = issuedMs;
  }
  const activationEpoch = normalizeOptionalUnsignedInteger(
    manifest.activation_epoch ?? manifest.activationEpoch,
    `${context}.activation_epoch`,
  );
  if (activationEpoch !== undefined) {
    normalized.activation_epoch = activationEpoch;
  }
  const expiryEpoch = normalizeOptionalUnsignedInteger(
    manifest.expiry_epoch ?? manifest.expiryEpoch,
    `${context}.expiry_epoch`,
  );
  if (expiryEpoch !== undefined) {
    normalized.expiry_epoch = expiryEpoch;
  }
  if (manifest.accounts !== undefined && manifest.accounts !== null) {
    normalized.accounts = requireStringArray(
      manifest.accounts,
      `${context}.accounts`,
    ).map((account, index) =>
      ToriiClient._normalizeAccountId(account, `${context}.accounts[${index}]`),
    );
  }
  const entriesRaw = manifest.entries ?? manifest.Entries;
  if (!Array.isArray(entriesRaw) || entriesRaw.length === 0) {
    throw new TypeError(`${context}.entries must be a non-empty array`);
  }
  normalized.entries = entriesRaw.map((entry, index) => {
    const record = ensureRecord(entry, `${context}.entries[${index}]`);
    if (
      record.effect === undefined ||
      record.effect === null ||
      typeof record.effect !== "object"
    ) {
      throw new TypeError(
        `${context}.entries[${index}].effect must be an object`,
      );
    }
    if (
      record.scope !== undefined &&
      record.scope !== null &&
      !isPlainObject(record.scope)
    ) {
      throw new TypeError(
        `${context}.entries[${index}].scope must be an object when present`,
      );
    }
    if (
      record.notes !== undefined &&
      record.notes !== null &&
      typeof record.notes !== "string"
    ) {
      throw new TypeError(
        `${context}.entries[${index}].notes must be a string when present`,
      );
    }
    return record;
  });
  return normalized;
}

function normalizeOptionalUnsignedInteger(value, context) {
  if (value === undefined || value === null) {
    return undefined;
  }
  return ToriiClient._normalizeUnsignedInteger(value, context, {
    allowZero: true,
  });
}

function normalizeRegisterContractCodeRequest(input) {
  const record = ensureRecord(input, "registerContractCode request");
  const credentials = normalizeAuthorityCredentials(record, "registerContractCode");
  const manifest = normalizeManifestPayload(record.manifest, "registerContractCode.manifest");
  const codeBytes = record.codeBytes ?? record.code_bytes;
  const payload = {
    ...credentials,
    manifest,
  };
  if (codeBytes !== undefined) {
    payload.code_bytes = normalizeOptionalBase64Payload(
      codeBytes,
      "registerContractCode.codeBytes",
    );
  }
  return payload;
}

function normalizePublishSpaceDirectoryManifestRequest(input) {
  const record = ensureRecord(input, "publishSpaceDirectoryManifest request");
  if (record.manifest === undefined || record.manifest === null) {
    throw new TypeError("publishSpaceDirectoryManifest.manifest is required");
  }
  const credentials = normalizeAuthorityCredentials(
    record,
    "publishSpaceDirectoryManifest",
  );
  const manifest = normalizeSpaceDirectoryManifestPayload(
    record.manifest,
    "publishSpaceDirectoryManifest.manifest",
  );
  const payload = {
    ...credentials,
    manifest,
  };
  const reason = optionalString(
    record.reason,
    "publishSpaceDirectoryManifest.reason",
  );
  if (reason !== null) {
    payload.reason = reason;
  }
  return payload;
}

function normalizeRevokeSpaceDirectoryManifestRequest(input) {
  const record = ensureRecord(input, "revokeSpaceDirectoryManifest request");
  const credentials = normalizeAuthorityCredentials(
    record,
    "revokeSpaceDirectoryManifest",
  );
  const uaid = normalizeUaidLiteral(
    record.uaid,
    "revokeSpaceDirectoryManifest.uaid",
  );
  const dataspace = ToriiClient._normalizeUnsignedInteger(
    record.dataspace ?? record.dataspaceId,
    "revokeSpaceDirectoryManifest.dataspace",
  );
  const revokedEpoch = ToriiClient._normalizeUnsignedInteger(
    record.revoked_epoch ?? record.revokedEpoch,
    "revokeSpaceDirectoryManifest.revokedEpoch",
    { allowZero: true },
  );
  const payload = {
    ...credentials,
    uaid,
    dataspace,
    revoked_epoch: revokedEpoch,
  };
  const reason = optionalString(
    record.reason,
    "revokeSpaceDirectoryManifest.reason",
  );
  if (reason !== null) {
    payload.reason = reason;
  }
  return payload;
}

function normalizeDeployContractRequest(input) {
  const record = ensureRecord(input, "deployContract request");
  const credentials = normalizeAuthorityCredentials(record, "deployContract");
  const codeB64 = record.code_b64 ?? record.codeB64;
  const contractAlias = record.contract_alias ?? record.contractAlias;
  const payload = {
    ...credentials,
    code_b64: normalizeRequiredBase64Payload(
      codeB64,
      "deployContract.codeB64",
    ),
    contract_alias: requireNonEmptyString(
      contractAlias,
      "deployContract.contract_alias",
    ),
  };
  if (record.dataspace !== undefined && record.dataspace !== null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "deployContract.dataspace is not accepted by /v1/contracts/deploy; use contractAlias to select the dataspace",
      "deployContract.dataspace",
    );
  }
  if (record.manifest !== undefined && record.manifest !== null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "deployContract.manifest is not accepted by /v1/contracts/deploy",
      "deployContract.manifest",
    );
  }
  const leaseExpiryValue = record.lease_expiry_ms ?? record.leaseExpiryMs;
  if (leaseExpiryValue !== undefined && leaseExpiryValue !== null) {
    const leaseExpiryMs = coerceInteger(
      leaseExpiryValue,
      "deployContract.lease_expiry_ms",
    );
    if (leaseExpiryMs < 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        "deployContract.lease_expiry_ms must be a non-negative integer",
        "deployContract.lease_expiry_ms",
      );
    }
    payload.lease_expiry_ms = leaseExpiryMs;
  }
  return payload;
}

function normalizeDeployContractResponse(payload) {
  const record = ensureRecord(payload, "deployContract response");
  const contractAlias =
    record.contract_alias === undefined || record.contract_alias === null
      ? null
      : requireNonEmptyString(
          record.contract_alias,
          "deployContract.response.contract_alias",
        );
  const contractAddress =
    record.contract_address === undefined || record.contract_address === null
      ? null
      : requireNonEmptyString(
          record.contract_address,
          "deployContract.response.contract_address",
        );
  const previousContractAddress =
    record.previous_contract_address === undefined
      || record.previous_contract_address === null
      ? null
      : requireNonEmptyString(
          record.previous_contract_address,
          "deployContract.response.previous_contract_address",
        );
  const dataspace =
    record.dataspace === undefined || record.dataspace === null
      ? null
      : requireNonEmptyString(record.dataspace, "deployContract.response.dataspace");
  const deployNonce =
    record.deploy_nonce === undefined || record.deploy_nonce === null
      ? null
      : coerceInteger(record.deploy_nonce, "deployContract.response.deploy_nonce");
  const txHashHex =
    record.tx_hash_hex === undefined || record.tx_hash_hex === null
      ? null
      : normalizeHex32String(record.tx_hash_hex, "deployContract.response.tx_hash_hex");
  return {
    ok: Boolean(record.ok),
    contract_alias: contractAlias,
    contract_address: contractAddress,
    previous_contract_address: previousContractAddress,
    upgraded: Boolean(record.upgraded),
    dataspace,
    deploy_nonce: deployNonce,
    tx_hash_hex: txHashHex,
    code_hash_hex: normalizeHex32String(
      record.code_hash_hex,
      "deployContract.response.code_hash_hex",
    ),
    abi_hash_hex: normalizeHex32String(
      record.abi_hash_hex,
      "deployContract.response.abi_hash_hex",
    ),
  };
}

function normalizeGovernanceFinalizePayload(input) {
  const record = ensureRecord(input, "governanceFinalizeReferendum payload");
  const referendumId = record.referendum_id ?? record.referendumId;
  if (referendumId === undefined || referendumId === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      "governanceFinalizeReferendum.referendum_id is required",
      "governanceFinalizeReferendum.referendum_id",
    );
  }
  const proposalId = record.proposal_id ?? record.proposalId;
  if (proposalId === undefined || proposalId === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      "governanceFinalizeReferendum.proposal_id is required",
      "governanceFinalizeReferendum.proposal_id",
    );
  }
  return {
    referendum_id: requireNonEmptyString(
      referendumId,
      "governanceFinalizeReferendum.referendum_id",
    ),
    proposal_id: normalizeHex32String(
      proposalId,
      "governanceFinalizeReferendum.proposal_id",
    ),
  };
}

function normalizeGovernanceEnactPayload(input) {
  const record = ensureRecord(input, "governanceEnactProposal payload");
  const payload = {
    proposal_id: normalizeHex32String(
      record.proposal_id ?? record.proposalId,
      "governanceEnactProposal.proposal_id",
    ),
  };
  const preimageValue = record.preimage_hash ?? record.preimageHash;
  if (preimageValue !== undefined && preimageValue !== null) {
    payload.preimage_hash = normalizeHex32String(
      preimageValue,
      "governanceEnactProposal.preimage_hash",
    );
  }
  const windowValue =
    record.window;
  if (windowValue !== undefined && windowValue !== null) {
    payload.window = normalizeGovernanceWindow(
      windowValue,
      "governanceEnactProposal.window",
    );
  }
  return payload;
}

function normalizeGovernanceWindow(value, name) {
  const record = ensureRecord(value, name);
  const lowerValue = record.lower;
  const upperValue = record.upper;
  if (lowerValue === undefined || upperValue === undefined) {
    const basePath = normalizeErrorPath(name);
    if (lowerValue === undefined) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name}.lower is required`,
        `${basePath}.lower`,
      );
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name}.upper is required`,
      `${basePath}.upper`,
    );
  }
  const lower = ToriiClient._normalizeUnsignedInteger(lowerValue, `${name}.lower`, {
    allowZero: true,
  });
  const upper = ToriiClient._normalizeUnsignedInteger(upperValue, `${name}.upper`, {
    allowZero: true,
  });
  if (upper < lower) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name}.upper must be greater than or equal to lower`,
      `${normalizeErrorPath(name)}.upper`,
    );
  }
  return { lower, upper };
}

function normalizeGovernanceDraftResponse(
  payload,
  context = "governance draft response",
) {
  const record = ensureRecord(payload, context);
  const instructionsValue = record.tx_instructions ?? [];
  if (!Array.isArray(instructionsValue)) {
    throw new TypeError(`${context}.tx_instructions must be an array`);
  }
  const txInstructions = instructionsValue.map((entry, index) => {
    const item = ensureRecord(entry, `${context}.tx_instructions[${index}]`);
    const wireId = requireNonEmptyString(
      item.wire_id,
      `${context}.tx_instructions[${index}].wire_id`,
    );
    const payloadHexValue = item.payload_hex;
    const normalizedPayload =
      payloadHexValue === undefined || payloadHexValue === null
        ? null
        : normalizeArbitraryHex(
            payloadHexValue,
            `${context}.tx_instructions[${index}].payload_hex`,
          );
    return normalizedPayload === null
      ? { wire_id: wireId }
      : { wire_id: wireId, payload_hex: normalizedPayload };
  });
  let proposalId = record.proposal_id ?? null;
  if (proposalId !== null && proposalId !== undefined) {
    proposalId = normalizeHex32String(proposalId, `${context}.proposal_id`);
  } else {
    proposalId = null;
  }
  const normalized = {
    ok: Boolean(record.ok),
    proposal_id: proposalId,
    tx_instructions: txInstructions,
  };
  if (record.accepted !== undefined) {
    normalized.accepted = Boolean(record.accepted);
  }
  if (record.reason !== undefined) {
    normalized.reason =
      record.reason === null || record.reason === undefined
        ? null
        : requireNonEmptyString(record.reason, `${context}.reason`);
  }
  return normalized;
}

function createEmptyGovernanceDraftResponse(context) {
  return normalizeGovernanceDraftResponse(
    {
      ok: true,
      tx_instructions: [],
    },
    context,
  );
}

function normalizeTriggerMutationResponse(
  payload,
  context = "trigger mutation response",
) {
  const record = ensureRecord(payload, context);
  const base = normalizeGovernanceDraftResponse(record, context);
  let triggerId = record.trigger_id ?? null;
  if (triggerId !== null && triggerId !== undefined) {
    triggerId = requireNonEmptyString(triggerId, `${context}.trigger_id`);
  } else {
    triggerId = null;
  }
  const messageValue = record.message ?? null;
  const message =
    messageValue === null || messageValue === undefined
      ? null
      : String(messageValue);
  const normalized = {
    ok: base.ok,
    trigger_id: triggerId,
    tx_instructions: base.tx_instructions,
  };
  if (base.accepted !== undefined) {
    normalized.accepted = base.accepted;
  }
  if (message !== null && message.length > 0) {
    normalized.message = message;
  }
  return normalized;
}

function normalizeGovernanceBallotResponse(payload, context) {
  const record = ensureRecord(payload, context);
  if (record.accepted === undefined) {
    throw new TypeError(`${context}.accepted is required`);
  }
  const base = normalizeGovernanceDraftResponse(record, context);
  const reason =
    record.reason === undefined || record.reason === null
      ? null
      : requireNonEmptyString(record.reason, `${context}.reason`);
  return {
    ok: base.ok,
    proposal_id: base.proposal_id,
    tx_instructions: base.tx_instructions,
    accepted: Boolean(record.accepted),
    reason,
  };
}

function normalizeGovernanceDeployContractProposalPayload(input) {
  const record = ensureRecord(input, "governanceProposeDeployContract payload");
  const contractAddressValue = record.contract_address ?? record.contractAddress ?? null;
  const contractAliasValue = record.contract_alias ?? record.contractAlias ?? null;
  if ((contractAddressValue == null) === (contractAliasValue == null)) {
    throw new TypeError(
      "governanceProposeDeployContract requires exactly one of contract_address or contract_alias",
    );
  }
  const abiVersion = requireNonEmptyString(
    record.abi_version ?? record.abiVersion ?? "1",
    "governanceProposeDeployContract.abiVersion",
  );
  const codeHashValue =
    record.code_hash ?? record.codeHash;
  if (codeHashValue === undefined || codeHashValue === null) {
    throw new TypeError("governanceProposeDeployContract.code_hash is required");
  }
  const abiHashValue =
    record.abi_hash ?? record.abiHash;
  if (abiHashValue === undefined || abiHashValue === null) {
    throw new TypeError("governanceProposeDeployContract.abi_hash is required");
  }
  const payload = {
    abi_version: abiVersion,
    code_hash: normalizeHashLike32(codeHashValue, "governanceProposeDeployContract.code_hash"),
    abi_hash: normalizeHashLike32(abiHashValue, "governanceProposeDeployContract.abi_hash"),
  };
  if (contractAddressValue != null) {
    payload.contract_address = requireNonEmptyString(
      contractAddressValue,
      "governanceProposeDeployContract.contract_address",
    );
  } else {
    payload.contract_alias = requireNonEmptyString(
      contractAliasValue,
      "governanceProposeDeployContract.contract_alias",
    );
  }
  const windowValue =
    record.window;
  if (windowValue !== undefined && windowValue !== null) {
    payload.window = normalizeGovernanceWindow(
      windowValue,
      "governanceProposeDeployContract.window",
    );
  }
  const modeValue = record.mode;
  if (modeValue !== undefined && modeValue !== null) {
    payload.mode = normalizeGovernanceVotingMode(
      modeValue,
      "governanceProposeDeployContract.mode",
    );
  }
  if (record.limits !== undefined) {
    payload.limits = cloneJsonValue(
      record.limits,
      "governanceProposeDeployContract.limits",
    );
  }
  return payload;
}

function normalizeGovernanceVotingMode(value, name) {
  const normalized = requireNonEmptyString(value, name);
  const lower = normalized.toLowerCase();
  if (lower === "zk" || lower === "zkballot" || lower === "zk_vote") {
    return "Zk";
  }
  if (lower === "plain" || lower === "plainballot") {
    return "Plain";
  }
  throw new TypeError(`${name} must be either "Zk" or "Plain"`);
}

function normalizeGovernancePlainBallotPayload(input) {
  const record = ensureRecord(input, "governanceSubmitPlainBallot payload");
  const direction = record.direction;
  const payload = {
    authority: ToriiClient._normalizeAccountId(
      record.authority,
      "governanceSubmitPlainBallot.authority",
    ),
    chain_id: requireNonEmptyString(
      record.chain_id ?? record.chainId,
      "governanceSubmitPlainBallot.chainId",
    ),
    referendum_id: requireNonEmptyString(
      record.referendum_id ?? record.referendumId,
      "governanceSubmitPlainBallot.referendumId",
    ),
    owner: ToriiClient._normalizeAccountId(
      record.owner,
      "governanceSubmitPlainBallot.owner",
    ),
    amount: normalizeNumericString(record.amount, "governanceSubmitPlainBallot.amount"),
    duration_blocks: ToriiClient._normalizeUnsignedInteger(
      record.duration_blocks ?? record.durationBlocks,
      "governanceSubmitPlainBallot.durationBlocks",
      { allowZero: false },
    ),
    direction: normalizeGovernanceBallotDirection(
      direction,
      "governanceSubmitPlainBallot.direction",
    ),
  };
  return payload;
}

function normalizeGovernanceBallotDirection(value, name) {
  const normalized = requireNonEmptyString(value, name).toLowerCase();
  if (normalized === "aye") {
    return "Aye";
  }
  if (normalized === "nay") {
    return "Nay";
  }
  if (normalized === "abstain") {
    return "Abstain";
  }
  throw new TypeError(`${name} must be one of Aye, Nay, or Abstain`);
}

function normalizeGovernancePublicInputs(value, name) {
  const cloned = cloneJsonValue(value, name);
  if (!isPlainObject(cloned)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be an object`,
      name,
    );
  }
  const normalized = { ...cloned };
  rejectGovernancePublicInputKey(
    normalized,
    "durationBlocks",
    "duration_blocks",
    name,
  );
  rejectGovernancePublicInputKey(normalized, "root_hint_hex", "root_hint", name);
  rejectGovernancePublicInputKey(normalized, "rootHintHex", "root_hint", name);
  rejectGovernancePublicInputKey(normalized, "rootHint", "root_hint", name);
  rejectGovernancePublicInputKey(normalized, "nullifier_hex", "nullifier", name);
  rejectGovernancePublicInputKey(normalized, "nullifierHex", "nullifier", name);
  normalizeGovernancePublicInputHex(normalized, "root_hint", name);
  normalizeGovernancePublicInputHex(normalized, "nullifier", name);
  ensureGovernanceLockHintsComplete(normalized, name);
  if (
    Object.prototype.hasOwnProperty.call(normalized, "owner") &&
    normalized.owner !== null
  ) {
    normalized.owner = ensureCanonicalAccountId(normalized.owner, `${name}.owner`);
  }
  return normalized;
}

function normalizeGovernancePublicInputHex(target, key, name) {
  if (!Object.prototype.hasOwnProperty.call(target, key)) {
    return;
  }
  const value = target[key];
  if (value === null) {
    return;
  }
  const context = `${name}.${key}`;
  const raw = requireNonEmptyString(value, context).trim();
  let body = raw;
  if (raw.includes(":")) {
    const [scheme, rest] = raw.split(":", 2);
    if (scheme && scheme.toLowerCase() !== "blake2b32") {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${context} must be a 32-byte hex string`,
        context,
      );
    }
    body = rest.trim();
  }
  if (body.startsWith("0x") || body.startsWith("0X")) {
    body = body.slice(2);
  }
  if (!/^[0-9a-fA-F]{64}$/.test(body)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${context} must be a 32-byte hex string`,
      context,
    );
  }
  target[key] = body.toLowerCase();
}

function rejectGovernancePublicInputKey(target, key, canonicalKey, name) {
  if (!Object.prototype.hasOwnProperty.call(target, key)) {
    return;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must use ${canonicalKey} (unsupported key ${key})`,
    name,
  );
}

function ensureGovernanceLockHintsComplete(source, name) {
  const hasOwner = source.owner !== undefined && source.owner !== null;
  const hasAmount = source.amount !== undefined && source.amount !== null;
  const hasDuration =
    source.duration_blocks !== undefined && source.duration_blocks !== null;
  const hasAnyLockHint = hasOwner || hasAmount || hasDuration;
  if (hasAnyLockHint && !(hasOwner && hasAmount && hasDuration)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must include owner, amount, and duration_blocks when providing lock hints`,
      name,
    );
  }
}

function normalizeGovernanceZkBallotPayload(input) {
  const record = ensureRecord(input, "governanceSubmitZkBallot payload");
  const payload = {
    authority: ToriiClient._normalizeAccountId(
      record.authority,
      "governanceSubmitZkBallot.authority",
    ),
    chain_id: requireNonEmptyString(
      record.chain_id ?? record.chainId,
      "governanceSubmitZkBallot.chainId",
    ),
    election_id: requireNonEmptyString(
      record.election_id ?? record.electionId,
      "governanceSubmitZkBallot.electionId",
    ),
    proof_b64: normalizeRequiredBase64Payload(
      record.proof ?? record.proof_b64 ?? record.proofB64,
      "governanceSubmitZkBallot.proofB64",
    ),
  };
  if (record.public !== undefined && record.public !== null) {
    payload.public = normalizeGovernancePublicInputs(
      record.public,
      "governanceSubmitZkBallot.public",
    );
  }
  return payload;
}

function normalizeGovernanceZkBallotV1Payload(input) {
  const record = ensureRecord(input, "governanceSubmitZkBallotV1 payload");
  const payload = {
    authority: ToriiClient._normalizeAccountId(
      record.authority,
      "governanceSubmitZkBallotV1.authority",
    ),
    chain_id: requireNonEmptyString(
      record.chain_id ?? record.chainId,
      "governanceSubmitZkBallotV1.chainId",
    ),
    election_id: requireNonEmptyString(
      record.election_id ?? record.electionId,
      "governanceSubmitZkBallotV1.electionId",
    ),
    backend: requireNonEmptyString(
      record.backend,
      "governanceSubmitZkBallotV1.backend",
    ),
    envelope_b64: normalizeRequiredBase64Payload(
      record.envelope ?? record.envelope_b64 ?? record.envelopeB64,
      "governanceSubmitZkBallotV1.envelopeB64",
    ),
  };
  rejectGovernancePublicInputKey(
    record,
    "root_hint_hex",
    "root_hint",
    "governanceSubmitZkBallotV1",
  );
  rejectGovernancePublicInputKey(
    record,
    "rootHintHex",
    "root_hint",
    "governanceSubmitZkBallotV1",
  );
  rejectGovernancePublicInputKey(
    record,
    "rootHint",
    "root_hint",
    "governanceSubmitZkBallotV1",
  );
  rejectGovernancePublicInputKey(
    record,
    "nullifier_hex",
    "nullifier",
    "governanceSubmitZkBallotV1",
  );
  rejectGovernancePublicInputKey(
    record,
    "nullifierHex",
    "nullifier",
    "governanceSubmitZkBallotV1",
  );
  const rootHint = record.root_hint;
  if (rootHint !== undefined && rootHint !== null) {
    payload.root_hint = normalizeHex32String(
      rootHint,
      "governanceSubmitZkBallotV1.root_hint",
      { allowScheme: true, scheme: "blake2b32" },
    );
  }
  if (record.owner !== undefined && record.owner !== null) {
    payload.owner = requireNonEmptyString(
      record.owner,
      "governanceSubmitZkBallotV1.owner",
    );
  }
  if (record.amount !== undefined && record.amount !== null) {
    payload.amount = normalizeNumericString(
      record.amount,
      "governanceSubmitZkBallotV1.amount",
    );
  }
  const durationBlocks = record.duration_blocks ?? record.durationBlocks;
  if (durationBlocks !== undefined && durationBlocks !== null) {
    payload.duration_blocks = ToriiClient._normalizeUnsignedInteger(
      durationBlocks,
      "governanceSubmitZkBallotV1.durationBlocks",
      { allowZero: false },
    );
  }
  if (record.direction !== undefined && record.direction !== null) {
    payload.direction = normalizeGovernanceBallotDirection(
      record.direction,
      "governanceSubmitZkBallotV1.direction",
    );
  }
  const nullifier = record.nullifier;
  if (nullifier !== undefined && nullifier !== null) {
    payload.nullifier = normalizeHex32String(
      nullifier,
      "governanceSubmitZkBallotV1.nullifier",
      { allowScheme: true, scheme: "blake2b32" },
    );
  }
  ensureGovernanceLockHintsComplete(payload, "governanceSubmitZkBallotV1");
  if (payload.owner !== undefined && payload.owner !== null) {
    payload.owner = ensureCanonicalAccountId(
      payload.owner,
      "governanceSubmitZkBallotV1.owner",
    );
  }
  return payload;
}

function normalizeGovernanceZkBallotProofPayload(input) {
  const record = ensureRecord(input, "governanceSubmitZkBallotProofV1 payload");
  const ballot = ensureRecord(
    record.ballot,
    "governanceSubmitZkBallotProofV1.ballot",
  );
  const ballotContext = "governanceSubmitZkBallotProofV1.ballot";
  const normalizedBallot = { ...ballot };
  rejectGovernancePublicInputKey(
    normalizedBallot,
    "rootHintHex",
    "root_hint",
    ballotContext,
  );
  rejectGovernancePublicInputKey(
    normalizedBallot,
    "root_hint_hex",
    "root_hint",
    ballotContext,
  );
  rejectGovernancePublicInputKey(
    normalizedBallot,
    "rootHint",
    "root_hint",
    ballotContext,
  );
  rejectGovernancePublicInputKey(
    normalizedBallot,
    "nullifierHex",
    "nullifier",
    ballotContext,
  );
  rejectGovernancePublicInputKey(
    normalizedBallot,
    "nullifier_hex",
    "nullifier",
    ballotContext,
  );
  if (Object.prototype.hasOwnProperty.call(normalizedBallot, "root_hint")) {
    const rootHint = normalizedBallot.root_hint;
    if (rootHint !== null) {
      normalizedBallot.root_hint = normalizeHex32String(
        rootHint,
        `${ballotContext}.root_hint`,
        { allowScheme: true, scheme: "blake2b32" },
      );
    }
  }
  if (Object.prototype.hasOwnProperty.call(normalizedBallot, "nullifier")) {
    const nullifier = normalizedBallot.nullifier;
    if (nullifier !== null) {
      normalizedBallot.nullifier = normalizeHex32String(
        nullifier,
        `${ballotContext}.nullifier`,
        { allowScheme: true, scheme: "blake2b32" },
      );
    }
  }
  ensureGovernanceLockHintsComplete(normalizedBallot, ballotContext);
  if (
    Object.prototype.hasOwnProperty.call(normalizedBallot, "owner") &&
    normalizedBallot.owner !== null
  ) {
    normalizedBallot.owner = ensureCanonicalAccountId(
      normalizedBallot.owner,
      `${ballotContext}.owner`,
    );
  }
  const payload = {
    authority: requireNonEmptyString(
      record.authority,
      "governanceSubmitZkBallotProofV1.authority",
    ),
    chain_id: requireNonEmptyString(
      record.chain_id ?? record.chainId,
      "governanceSubmitZkBallotProofV1.chainId",
    ),
    election_id: requireNonEmptyString(
      record.election_id ?? record.electionId,
      "governanceSubmitZkBallotProofV1.electionId",
    ),
    ballot: normalizedBallot,
  };
  return payload;
}

function normalizeArbitraryHex(value, name) {
  const normalized = requireHexString(value, name);
  const hex =
    normalized.startsWith("0x") || normalized.startsWith("0X")
      ? normalized.slice(2)
      : normalized;
  return hex.toLowerCase();
}

function normalizeContractTargetSelector(record, context) {
  const contractAddress = pickOverride(record, "contract_address", "contractAddress");
  const contractAlias = pickOverride(record, "contract_alias", "contractAlias");
  const hasContractAddress = contractAddress !== undefined && contractAddress !== null;
  const hasContractAlias = contractAlias !== undefined && contractAlias !== null;
  if (hasContractAddress === hasContractAlias) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} requires exactly one of contract_address or contract_alias`,
      normalizeErrorPath(context),
    );
  }
  if (hasContractAddress) {
    return {
      contract_address: requireNonEmptyString(
        contractAddress,
        `${context}.contract_address`,
      ),
    };
  }
  return {
    contract_alias: requireNonEmptyString(
      contractAlias,
      `${context}.contract_alias`,
    ),
  };
}

function normalizeContractCallRequest(input) {
  const record = ensureRecord(input, "contractCall request");
  const credentials = normalizeAuthorityCredentials(record, "contractCall");
  const normalized = {
    ...credentials,
    ...normalizeContractTargetSelector(record, "contractCall"),
  };
  if (record.entrypoint !== undefined && record.entrypoint !== null) {
    normalized.entrypoint = requireNonEmptyString(
      record.entrypoint,
      "contractCall.entrypoint",
    );
  }
  if (record.payload !== undefined) {
    normalized.payload = cloneJsonValue(record.payload, "contractCall.payload");
  }
  const gasAsset = record.gas_asset_id ?? record.gasAssetId;
  if (gasAsset !== undefined && gasAsset !== null) {
    normalized.gas_asset_id = ToriiClient._normalizeAssetId(
      gasAsset,
      "contractCall.gasAssetId",
    );
  }
  const gasLimit = record.gas_limit ?? record.gasLimit;
  normalized.gas_limit = ToriiClient._normalizeUnsignedInteger(
    gasLimit,
    "contractCall.gasLimit",
    { allowZero: true },
  );
  return normalized;
}

function normalizeContractCallResponse(payload) {
  const record = ensureRecord(payload, "contractCall response");
  const normalized = {
    ok: Boolean(record.ok),
    submitted: Boolean(record.submitted),
    dataspace: requireNonEmptyString(
      record.dataspace,
      "contractCall response.dataspace",
    ),
    code_hash_hex: normalizeHex32String(
      record.code_hash_hex,
      "contractCall response.code_hash_hex",
    ),
    abi_hash_hex: normalizeHex32String(
      record.abi_hash_hex,
      "contractCall response.abi_hash_hex",
    ),
    creation_time_ms: ToriiClient._normalizeUnsignedInteger(
      record.creation_time_ms,
      "contractCall response.creation_time_ms",
      { allowZero: true },
    ),
  };
  if (record.contract_address !== undefined && record.contract_address !== null) {
    normalized.contract_address = requireNonEmptyString(
      record.contract_address,
      "contractCall response.contract_address",
    );
  }
  if (record.tx_hash_hex !== undefined && record.tx_hash_hex !== null) {
    normalized.tx_hash_hex = normalizeHex32String(
      record.tx_hash_hex,
      "contractCall response.tx_hash_hex",
    );
  } else {
    normalized.tx_hash_hex = null;
  }
  normalized.entrypoint =
    record.entrypoint === undefined || record.entrypoint === null
      ? null
      : requireNonEmptyString(
          record.entrypoint,
          "contractCall response.entrypoint",
        );
  normalized.transaction_scaffold_b64 =
    record.transaction_scaffold_b64 === undefined || record.transaction_scaffold_b64 === null
      ? null
      : normalizeRequiredBase64Payload(
          record.transaction_scaffold_b64,
          "contractCall response.transaction_scaffold_b64",
        );
  normalized.signed_transaction_b64 =
    record.signed_transaction_b64 === undefined || record.signed_transaction_b64 === null
      ? null
      : normalizeRequiredBase64Payload(
          record.signed_transaction_b64,
          "contractCall response.signed_transaction_b64",
        );
  normalized.signing_message_b64 =
    record.signing_message_b64 === undefined || record.signing_message_b64 === null
      ? null
      : normalizeRequiredBase64Payload(
          record.signing_message_b64,
          "contractCall response.signing_message_b64",
        );
  return normalized;
}

function normalizeMultisigAccountSelector(input, context) {
  const record = ensureRecord(input, context);
  const multisigAccountId = pickOverride(
    record,
    "multisig_account_id",
    "multisigAccountId",
  );
  const multisigAccountAlias = pickOverride(
    record,
    "multisig_account_alias",
    "multisigAccountAlias",
  );
  const hasAccountId = multisigAccountId !== undefined && multisigAccountId !== null;
  const hasAlias = multisigAccountAlias !== undefined && multisigAccountAlias !== null;
  if (hasAccountId === hasAlias) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} requires exactly one of multisig_account_id or multisig_account_alias`,
      normalizeErrorPath(context),
    );
  }
  if (hasAccountId) {
    return {
      multisig_account_id: ToriiClient._normalizeAccountId(
        multisigAccountId,
        `${context}.multisig_account_id`,
      ),
    };
  }
  return {
    multisig_account_alias: normalizeMultisigAccountAliasLiteral(
      multisigAccountAlias,
      `${context}.multisig_account_alias`,
    ),
  };
}

function normalizeMultisigAccountAliasLiteral(value, context) {
  const alias = requireNonEmptyString(value, context).trim();
  const parts = alias.split("@");
  const scopeParts = parts[1]?.split(".") ?? [];
  if (
    parts.length !== 2 ||
    !parts[0] ||
    !parts[1] ||
    scopeParts.length < 1 ||
    scopeParts.length > 2 ||
    scopeParts.some((part) => !part)
  ) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context} must use name@dataspace or name@domain.dataspace form`,
      normalizeErrorPath(context),
    );
  }
  if (/\s/.test(alias)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context} must not contain whitespace`,
      normalizeErrorPath(context),
    );
  }
  return alias;
}

function normalizeAccountPathLiteral(value, context) {
  const literal = requireNonEmptyString(value, context).trim();
  if (literal.includes("@")) {
    return normalizeMultisigAccountAliasLiteral(literal, context);
  }
  return normalizeAccountId(literal, context);
}

function hasDetachedPrivateKeyInput(record) {
  return (
    pickOverride(record, "private_key", "privateKey") !== undefined ||
    pickOverride(record, "private_key_multihash", "privateKeyMultihash") !== undefined ||
    pickOverride(record, "private_key_hex", "privateKeyHex") !== undefined ||
    pickOverride(record, "private_key_bytes", "privateKeyBytes") !== undefined
  );
}

function normalizeOptionalDetachedPrivateKey(record, context) {
  if (!hasDetachedPrivateKeyInput(record)) {
    return null;
  }
  return resolveAuthorityPrivateKey(record, context);
}

function normalizeMultisigSelectorOnlyRequest(input, context) {
  return normalizeMultisigAccountSelector(input, context);
}

function normalizeMultisigContractCallProposeRequest(input) {
  const record = ensureRecord(input, "proposeMultisigContractCall request");
  const selector = normalizeMultisigAccountSelector(
    record,
    "proposeMultisigContractCall request",
  );
  const payload = {
    ...selector,
    signer_account_id: ToriiClient._normalizeAccountId(
      record.signer_account_id ?? record.signerAccountId,
      "proposeMultisigContractCall request.signer_account_id",
    ),
    ...normalizeContractTargetSelector(record, "proposeMultisigContractCall request"),
    entrypoint: requireNonEmptyString(
      record.entrypoint,
      "proposeMultisigContractCall request.entrypoint",
    ),
  };
  const privateKey = normalizeOptionalDetachedPrivateKey(
    record,
    "proposeMultisigContractCall request",
  );
  if (privateKey !== null) {
    payload.private_key = privateKey;
  }
  const publicKeyHex = pickOverride(record, "public_key_hex", "publicKeyHex");
  if (publicKeyHex !== undefined && publicKeyHex !== null) {
    payload.public_key_hex = normalizeHex32String(
      publicKeyHex,
      "proposeMultisigContractCall request.public_key_hex",
    );
  }
  const signatureB64 = pickOverride(record, "signature_b64", "signatureB64");
  if (signatureB64 !== undefined && signatureB64 !== null) {
    payload.signature_b64 = normalizeRequiredBase64Payload(
      signatureB64,
      "proposeMultisigContractCall request.signature_b64",
    );
  }
  const creationTimeMs = pickOverride(record, "creation_time_ms", "creationTimeMs");
  if (creationTimeMs !== undefined && creationTimeMs !== null) {
    payload.creation_time_ms = ToriiClient._normalizeUnsignedInteger(
      creationTimeMs,
      "proposeMultisigContractCall request.creation_time_ms",
      { allowZero: true },
    );
  }
  if (record.payload !== undefined) {
    payload.payload = cloneJsonValue(
      record.payload,
      "proposeMultisigContractCall request.payload",
    );
  }
  const gasAssetId = pickOverride(record, "gas_asset_id", "gasAssetId");
  if (gasAssetId !== undefined && gasAssetId !== null) {
    payload.gas_asset_id = ToriiClient._normalizeAssetId(
      gasAssetId,
      "proposeMultisigContractCall request.gas_asset_id",
    );
  }
  const feeSponsor = pickOverride(record, "fee_sponsor", "feeSponsor");
  if (feeSponsor !== undefined && feeSponsor !== null) {
    payload.fee_sponsor = ToriiClient._normalizeAccountId(
      feeSponsor,
      "proposeMultisigContractCall request.fee_sponsor",
    );
  }
  const gasLimit = pickOverride(record, "gas_limit", "gasLimit");
  if (gasLimit !== undefined && gasLimit !== null) {
    payload.gas_limit = ToriiClient._normalizeUnsignedInteger(
      gasLimit,
      "proposeMultisigContractCall request.gas_limit",
      { allowZero: false },
    );
  }
  return payload;
}

function normalizeMultisigContractCallApproveRequest(input) {
  const record = ensureRecord(input, "approveMultisigContractCall request");
  const selector = normalizeMultisigAccountSelector(
    record,
    "approveMultisigContractCall request",
  );
  const payload = {
    ...selector,
    signer_account_id: ToriiClient._normalizeAccountId(
      record.signer_account_id ?? record.signerAccountId,
      "approveMultisigContractCall request.signer_account_id",
    ),
  };
  const privateKey = normalizeOptionalDetachedPrivateKey(
    record,
    "approveMultisigContractCall request",
  );
  if (privateKey !== null) {
    payload.private_key = privateKey;
  }
  const publicKeyHex = pickOverride(record, "public_key_hex", "publicKeyHex");
  if (publicKeyHex !== undefined && publicKeyHex !== null) {
    payload.public_key_hex = normalizeHex32String(
      publicKeyHex,
      "approveMultisigContractCall request.public_key_hex",
    );
  }
  const signatureB64 = pickOverride(record, "signature_b64", "signatureB64");
  if (signatureB64 !== undefined && signatureB64 !== null) {
    payload.signature_b64 = normalizeRequiredBase64Payload(
      signatureB64,
      "approveMultisigContractCall request.signature_b64",
    );
  }
  const creationTimeMs = pickOverride(record, "creation_time_ms", "creationTimeMs");
  if (creationTimeMs !== undefined && creationTimeMs !== null) {
    payload.creation_time_ms = ToriiClient._normalizeUnsignedInteger(
      creationTimeMs,
      "approveMultisigContractCall request.creation_time_ms",
      { allowZero: true },
    );
  }
  const proposalId = pickOverride(record, "proposal_id", "proposalId");
  if (proposalId !== undefined && proposalId !== null) {
    payload.proposal_id = requireNonEmptyString(
      proposalId,
      "approveMultisigContractCall request.proposal_id",
    );
  }
  const instructionsHash = pickOverride(
    record,
    "instructions_hash",
    "instructionsHash",
  );
  if (instructionsHash !== undefined && instructionsHash !== null) {
    payload.instructions_hash = normalizeHex32String(
      instructionsHash,
      "approveMultisigContractCall request.instructions_hash",
    );
  }
  if (!payload.proposal_id && !payload.instructions_hash) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "approveMultisigContractCall request requires proposal_id or instructions_hash",
      "approveMultisigContractCall.request",
    );
  }
  return payload;
}

function normalizeMultisigContractCallResponse(
  payload,
  context = "multisig contract call response",
) {
  const record = ensureRecord(payload, context);
  return {
    ok: Boolean(record.ok),
    resolved_multisig_account_id: ToriiClient._normalizeAccountId(
      record.resolved_multisig_account_id,
      `${context}.resolved_multisig_account_id`,
    ),
    submitted: optionalBoolean(record.submitted, `${context}.submitted`),
    proposal_id: optionalString(record.proposal_id, `${context}.proposal_id`),
    instructions_hash:
      record.instructions_hash === undefined || record.instructions_hash === null
        ? null
        : normalizeHex32String(record.instructions_hash, `${context}.instructions_hash`),
    executed_tx_hash_hex:
      record.executed_tx_hash_hex === undefined || record.executed_tx_hash_hex === null
        ? null
        : normalizeHex32String(
            record.executed_tx_hash_hex,
            `${context}.executed_tx_hash_hex`,
          ),
    creation_time_ms:
      record.creation_time_ms === undefined || record.creation_time_ms === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            record.creation_time_ms,
            `${context}.creation_time_ms`,
            { allowZero: true },
          ),
    signing_message_b64: normalizeOptionalBase64Payload(
      record.signing_message_b64,
      `${context}.signing_message_b64`,
    ),
  };
}

function normalizeMultisigSpecResponse(payload, context = "multisig spec response") {
  const record = ensureRecord(payload, context);
  return {
    resolved_multisig_account_id: ToriiClient._normalizeAccountId(
      record.resolved_multisig_account_id,
      `${context}.resolved_multisig_account_id`,
    ),
    spec: cloneJsonValue(record.spec, `${context}.spec`),
  };
}

function normalizeMultisigProposalEntry(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    proposal_id: requireNonEmptyString(record.proposal_id, `${context}.proposal_id`),
    instructions_hash: normalizeHex32String(
      record.instructions_hash,
      `${context}.instructions_hash`,
    ),
    proposal: cloneJsonValue(record.proposal, `${context}.proposal`),
  };
}

function normalizeMultisigProposalsListResponse(
  payload,
  context = "multisig proposals list response",
) {
  const record = ensureRecord(payload, context);
  const proposalsValue = record.proposals;
  if (!Array.isArray(proposalsValue)) {
    throw new TypeError(`${context}.proposals must be an array`);
  }
  return {
    resolved_multisig_account_id: ToriiClient._normalizeAccountId(
      record.resolved_multisig_account_id,
      `${context}.resolved_multisig_account_id`,
    ),
    proposals: proposalsValue.map((entry, index) =>
      normalizeMultisigProposalEntry(entry, `${context}.proposals[${index}]`),
    ),
  };
}

function normalizeMultisigProposalLookupRequest(input) {
  const record = ensureRecord(input, "getMultisigProposal request");
  const payload = normalizeMultisigAccountSelector(record, "getMultisigProposal request");
  const proposalId = pickOverride(record, "proposal_id", "proposalId");
  if (proposalId !== undefined && proposalId !== null) {
    payload.proposal_id = requireNonEmptyString(
      proposalId,
      "getMultisigProposal request.proposal_id",
    );
  }
  const instructionsHash = pickOverride(
    record,
    "instructions_hash",
    "instructionsHash",
  );
  if (instructionsHash !== undefined && instructionsHash !== null) {
    payload.instructions_hash = normalizeHex32String(
      instructionsHash,
      "getMultisigProposal request.instructions_hash",
    );
  }
  if (!payload.proposal_id && !payload.instructions_hash) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "getMultisigProposal request requires proposal_id or instructions_hash",
      "getMultisigProposal.request",
    );
  }
  return payload;
}

function normalizeMultisigProposalGetResponse(
  payload,
  context = "multisig proposal get response",
) {
  const record = ensureRecord(payload, context);
  return {
    resolved_multisig_account_id: ToriiClient._normalizeAccountId(
      record.resolved_multisig_account_id,
      `${context}.resolved_multisig_account_id`,
    ),
    proposal_id: requireNonEmptyString(record.proposal_id, `${context}.proposal_id`),
    instructions_hash: normalizeHex32String(
      record.instructions_hash,
      `${context}.instructions_hash`,
    ),
    proposal: cloneJsonValue(record.proposal, `${context}.proposal`),
  };
}

function normalizeContractManifestResponse(payload) {
  const record = ensureRecord(payload, "contract manifest response");
  const manifestRecord = ensureRecord(
    record.manifest ?? {},
    "contract manifest response.manifest",
  );
  const accessHints =
    manifestRecord.access_set_hints ?? null;
  const entrypointsValue =
    manifestRecord.entrypoints ?? null;
  const kotobaValue = manifestRecord.kotoba ?? null;
  const provenanceValue = manifestRecord.provenance ?? null;
  return {
    manifest: {
      code_hash: normalizeOptionalHex32(
        manifestRecord.code_hash,
        "manifest.code_hash",
      ),
      abi_hash: normalizeOptionalHex32(
        manifestRecord.abi_hash,
        "manifest.abi_hash",
      ),
      compiler_fingerprint:
        manifestRecord.compiler_fingerprint ?? null,
      features_bitmap: manifestRecord.features_bitmap ?? null,
      access_set_hints:
        accessHints === null
          ? null
          : normalizeAccessSetHintsPayload(
              accessHints,
              "manifest.access_set_hints",
            ),
      entrypoints: normalizeManifestEntrypointsPayload(
        entrypointsValue,
        "manifest.entrypoints",
      ),
      kotoba:
        kotobaValue === null
          ? null
          : cloneJsonValue(kotobaValue, "manifest.kotoba"),
      provenance:
        provenanceValue === null
          ? null
          : cloneJsonValue(provenanceValue, "manifest.provenance"),
    },
    code_bytes:
      record.code_bytes === undefined || record.code_bytes === null
        ? null
        : normalizeOptionalBase64Payload(record.code_bytes, "contractManifest.code_bytes"),
  };
}

function normalizeContractCodeBytesResponse(payload) {
  const record = ensureRecord(payload, "contract code bytes response");
  return {
    code_b64: normalizeRequiredBase64Payload(
      record.code_b64,
      "contractCodeBytes.code_b64",
    ),
  };
}

function normalizeManifestEntrypointsPayload(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${name} must be an array of entrypoints`);
  }
  if (value.length === 0) {
    return [];
  }
  return value.map((entry, index) => {
    const record = ensureRecord(entry, `${name}[${index}]`);
    const entryName = requireNonEmptyString(
      record.name,
      `${name}[${index}].name`,
    );
    const permission =
      record.permission === undefined || record.permission === null
        ? null
        : requireNonEmptyString(
            record.permission,
            `${name}[${index}].permission`,
          );
    const kind = normalizeManifestEntrypointKind(
      record.kind,
      `${name}[${index}].kind`,
    );
    return {
      name: entryName,
      kind,
      permission,
    };
  });
}

function normalizeManifestEntrypointKind(value, name) {
  if (value === undefined || value === null) {
    return { kind: "Public" };
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw new TypeError(`${name} must be a non-empty string`);
    }
    return { kind: trimmed };
  }
  const record = ensureRecord(value, name);
  const kind = requireNonEmptyString(record.kind, `${name}.kind`);
  return "value" in record ? { kind, value: record.value } : { kind };
}

function normalizeGovernanceContractResponse(payload) {
  const record = ensureRecord(payload ?? {}, "governance contract response");
  return {
    found: Boolean(record.found),
    contract_address: requireNonEmptyString(
      record.contract_address,
      "governanceContract.contract_address",
    ),
    dataspace:
      record.dataspace == null
        ? null
        : requireNonEmptyString(record.dataspace, "governanceContract.dataspace"),
    code_hash_hex:
      record.code_hash_hex == null
        ? null
        : normalizeHex32String(
            record.code_hash_hex,
            "governanceContract.code_hash_hex",
          ),
  };
}

function normalizeAliasResolutionResponse(
  payload,
  context = "alias resolution response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const alias = requireNonEmptyString(record.alias, `${context}.alias`);
  const normalizedAlias = looksLikeIban(alias)
    ? normalizeIban(alias, `${context}.alias`)
    : alias;
  const accountId = ToriiClient._requireAccountId(
    record.account_id,
    `${context}.account_id`,
  );
  const result = {
    alias: normalizedAlias,
    account_id: accountId,
  };
  if (record.index !== undefined && record.index !== null) {
    result.index = ToriiClient._normalizeUnsignedInteger(record.index, `${context}.index`, {
      allowZero: true,
    });
  }
  const sourceValue = record.source;
  if (sourceValue !== undefined && sourceValue !== null) {
    result.source = requireNonEmptyString(sourceValue, `${context}.source`);
  }
  return result;
}

function normalizeAliasLookupByAccountResponse(
  payload,
  context = "alias by-account response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const accountId = ToriiClient._requireAccountId(record.account_id, `${context}.account_id`);
  const rawItems = Array.isArray(record.items) ? record.items : [];
  const items = rawItems.map((item, index) => {
    const aliasRecord = ensureRecord(item, `${context}.items[${index}]`);
    const alias = requireNonEmptyString(aliasRecord.alias, `${context}.items[${index}].alias`);
    const dataspace = requireNonEmptyString(
      aliasRecord.dataspace,
      `${context}.items[${index}].dataspace`,
    );
    const rawDomain = aliasRecord.domain;
    const domain =
      rawDomain == null
        ? null
        : requireNonEmptyString(rawDomain, `${context}.items[${index}].domain`);
    const rawIsPrimary = aliasRecord.is_primary ?? aliasRecord.isPrimary;
    if (typeof rawIsPrimary !== "boolean") {
      throw new TypeError(`${context}.items[${index}].is_primary must be a boolean`);
    }
    return {
      alias,
      dataspace,
      domain,
      is_primary: rawIsPrimary,
    };
  });
  const total =
    record.total === undefined
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return {
    account_id: accountId,
    total,
    items,
  };
}

function buildIdentifierResolveRequest(options, context) {
  const record = ensureRecord(options, `${context} options`);
  assertSupportedOptionKeys(
    record,
    new Set(["policyId", "input", "encryptedInput"]),
    `${context} options`,
  );
  const policyId = requireNonEmptyString(record.policyId, `${context}.policyId`);
  const hasInput = record.input !== undefined && record.input !== null;
  const hasEncryptedInput = record.encryptedInput !== undefined && record.encryptedInput !== null;
  if (hasInput === hasEncryptedInput) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options must supply exactly one of input or encryptedInput`,
      `${context}.input`,
    );
  }
  const payload = { policy_id: policyId };
  if (hasInput) {
    payload.input = requireNonEmptyString(record.input, `${context}.input`);
  } else {
    payload.encrypted_input = requireHexString(
      record.encryptedInput,
      `${context}.encryptedInput`,
    );
  }
  return payload;
}

function buildRamLfeExecuteRequest(options, context) {
  const record = ensureRecord(options, `${context} options`);
  assertSupportedOptionKeys(
    record,
    new Set(["inputHex", "encryptedInput"]),
    `${context} options`,
  );
  const hasInputHex = record.inputHex !== undefined && record.inputHex !== null;
  const hasEncryptedInput =
    record.encryptedInput !== undefined && record.encryptedInput !== null;
  if (hasInputHex === hasEncryptedInput) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options must supply exactly one of inputHex or encryptedInput`,
      `${context}.inputHex`,
    );
  }
  if (hasInputHex) {
    return {
      input_hex: requireHexString(record.inputHex, `${context}.inputHex`),
    };
  }
  return {
    encrypted_input: requireHexString(
      record.encryptedInput,
      `${context}.encryptedInput`,
    ),
  };
}

function buildRamLfeReceiptVerifyRequest(options, context) {
  const record = ensureRecord(options, `${context} options`);
  assertSupportedOptionKeys(
    record,
    new Set(["receipt", "outputHex"]),
    `${context} options`,
  );
  if (record.receipt === undefined || record.receipt === null) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options must supply a receipt object`,
      `${context}.receipt`,
    );
  }
  const payload = {
    receipt: ensureRecord(record.receipt, `${context}.receipt`),
  };
  if (record.outputHex !== undefined && record.outputHex !== null) {
    payload.output_hex = requireHexString(record.outputHex, `${context}.outputHex`);
  }
  return payload;
}

function normalizeIdentifierBfvParameters(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    polynomial_degree: ToriiClient._normalizeUnsignedInteger(
      record.polynomial_degree,
      `${context}.polynomial_degree`,
      { allowZero: false },
    ),
    plaintext_modulus: ToriiClient._normalizeUnsignedInteger(
      record.plaintext_modulus,
      `${context}.plaintext_modulus`,
      { allowZero: false },
    ),
    ciphertext_modulus: ToriiClient._normalizeUnsignedInteger(
      record.ciphertext_modulus,
      `${context}.ciphertext_modulus`,
      { allowZero: false },
    ),
    decomposition_base_log: ToriiClient._normalizeUnsignedInteger(
      record.decomposition_base_log,
      `${context}.decomposition_base_log`,
      { allowZero: false },
    ),
  };
}

function normalizeIdentifierBfvPublicKey(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    b: requireUnsignedIntegerArray(record.b, `${context}.b`),
    a: requireUnsignedIntegerArray(record.a, `${context}.a`),
  };
}

function normalizeIdentifierBfvPublicParameters(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    parameters: normalizeIdentifierBfvParameters(
      record.parameters,
      `${context}.parameters`,
    ),
    public_key: normalizeIdentifierBfvPublicKey(
      record.public_key,
      `${context}.public_key`,
    ),
    max_input_bytes: ToriiClient._normalizeUnsignedInteger(
      record.max_input_bytes,
      `${context}.max_input_bytes`,
      { allowZero: false },
    ),
  };
}

function u64ToLittleEndianBuffer(value) {
  const normalized = BigInt.asUintN(64, BigInt(value));
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(normalized);
  return buffer;
}

function createSha512Digest(parts) {
  const hash = createHash("sha512");
  for (const part of parts) {
    hash.update(part);
  }
  return hash.digest();
}

function deriveIdentifierBfvSeed(record, context) {
  const hasSeedHex = record.seedHex !== undefined && record.seedHex !== null;
  const hasSeedBytes = record.seed !== undefined && record.seed !== null;
  if (hasSeedHex && hasSeedBytes) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must not supply both seed and seedHex`,
      `${context}.seed`,
    );
  }
  if (hasSeedHex) {
    return Buffer.from(requireHexString(record.seedHex, `${context}.seedHex`), "hex");
  }
  if (hasSeedBytes) {
    return toBuffer(record.seed);
  }
  return randomBytes(BFV_IDENTIFIER_SEED_BYTES);
}

class IdentifierBfvDeterministicStream {
  constructor(seed, domain) {
    this.seed = Buffer.from(seed);
    this.domain = Buffer.from(domain);
    this.counter = 0n;
    this.buffer = Buffer.alloc(0);
    this.offset = 0;
  }

  nextBytes(length) {
    let remaining = length;
    const chunks = [];
    while (remaining > 0) {
      if (this.offset >= this.buffer.length) {
        this.buffer = createSha512Digest([
          BFV_IDENTIFIER_SHA512_DOMAIN,
          this.domain,
          this.seed,
          u64ToLittleEndianBuffer(this.counter),
        ]);
        this.counter += 1n;
        this.offset = 0;
      }
      const available = Math.min(remaining, this.buffer.length - this.offset);
      chunks.push(this.buffer.subarray(this.offset, this.offset + available));
      this.offset += available;
      remaining -= available;
    }
    return Buffer.concat(chunks, length);
  }

  nextU64() {
    return this.nextBytes(8).readBigUInt64LE(0);
  }
}

function crc64Ecma(payload) {
  let crc = UINT64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ UINT64_MASK);
}

function noritoSchemaHash(typeName) {
  let hash = 0xcbf29ce484222325n;
  for (const byte of Buffer.from(typeName, "utf8")) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  const part = u64ToLittleEndianBuffer(hash);
  return Buffer.concat([part, part]);
}

function frameNoritoPayload(typeName, payload, flags = 0) {
  const header = Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    noritoSchemaHash(typeName),
    Buffer.from([0]),
    u64ToLittleEndianBuffer(payload.length),
    u64ToLittleEndianBuffer(crc64Ecma(payload)),
    Buffer.from([flags & 0xff]),
  ]);
  return Buffer.concat([header, payload]);
}

function encodeNoritoField(payload) {
  return Buffer.concat([u64ToLittleEndianBuffer(payload.length), payload]);
}

function encodeNoritoU64(value) {
  return u64ToLittleEndianBuffer(value);
}

function encodeNoritoVec(values, encode) {
  const parts = [u64ToLittleEndianBuffer(values.length)];
  for (const value of values) {
    const payload = encode(value);
    parts.push(u64ToLittleEndianBuffer(payload.length), payload);
  }
  return Buffer.concat(parts);
}

function encodeNoritoBfvCiphertext(ciphertext) {
  return Buffer.concat([
    encodeNoritoField(encodeNoritoVec(ciphertext.c0, encodeNoritoU64)),
    encodeNoritoField(encodeNoritoVec(ciphertext.c1, encodeNoritoU64)),
  ]);
}

function encodeNoritoBfvIdentifierCiphertext(ciphertext) {
  return frameNoritoPayload(
    BFV_IDENTIFIER_SCHEMA_NAME,
    encodeNoritoField(encodeNoritoVec(ciphertext.slots, encodeNoritoBfvCiphertext)),
  );
}

function requireSafeBfvUint(value, name) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be a non-negative safe integer`,
      name,
    );
  }
  return BigInt(value);
}

function normalizeIdentifierBfvEncryptionInputs(policySummary, input, options = {}) {
  const normalizedPolicy = normalizeIdentifierPolicySummary(
    policySummary,
    "encryptIdentifierInputForPolicy.policy",
  );
  if (normalizedPolicy.input_encryption !== "bfv-v1") {
    throw new Error(
      `encryptIdentifierInputForPolicy: policy ${normalizedPolicy.policy_id} does not publish BFV encrypted-input support`,
    );
  }
  const publicParameters = getIdentifierBfvPublicParameters(normalizedPolicy);
  if (!publicParameters) {
    throw new Error(
      `encryptIdentifierInputForPolicy: policy ${normalizedPolicy.policy_id} is missing decoded BFV public parameters`,
    );
  }
  const normalizedInput = normalizeIdentifierInput(
    input,
    normalizedPolicy.normalization,
    "encryptIdentifierInputForPolicy.input",
  );
  const record = ensureRecord(options, "encryptIdentifierInputForPolicy options");
  assertSupportedOptionKeys(
    record,
    new Set(["seed", "seedHex"]),
    "encryptIdentifierInputForPolicy options",
  );
  return {
    policy: normalizedPolicy,
    publicParameters,
    inputBytes: Buffer.from(normalizedInput, "utf8"),
    seed: deriveIdentifierBfvSeed(record, "encryptIdentifierInputForPolicy options"),
  };
}

function validateIdentifierBfvPublicParameters(publicParameters, context) {
  const params = publicParameters.parameters;
  const polynomialDegree = Number(
    requireSafeBfvUint(params.polynomial_degree, `${context}.parameters.polynomial_degree`),
  );
  const plaintextModulus = requireSafeBfvUint(
    params.plaintext_modulus,
    `${context}.parameters.plaintext_modulus`,
  );
  const ciphertextModulus = requireSafeBfvUint(
    params.ciphertext_modulus,
    `${context}.parameters.ciphertext_modulus`,
  );
  const decompositionBaseLog = Number(
    requireSafeBfvUint(
      params.decomposition_base_log,
      `${context}.parameters.decomposition_base_log`,
    ),
  );
  const maxInputBytes = Number(
    requireSafeBfvUint(publicParameters.max_input_bytes, `${context}.max_input_bytes`),
  );
  if (polynomialDegree < 2 || (polynomialDegree & (polynomialDegree - 1)) !== 0) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.polynomial_degree must be a power of two and at least 2`,
      `${context}.parameters.polynomial_degree`,
    );
  }
  if (decompositionBaseLog < 1 || decompositionBaseLog > 16) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.decomposition_base_log must be within 1..=16`,
      `${context}.parameters.decomposition_base_log`,
    );
  }
  if (plaintextModulus < 2n) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.plaintext_modulus must be at least 2`,
      `${context}.parameters.plaintext_modulus`,
    );
  }
  if (ciphertextModulus <= plaintextModulus) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.ciphertext_modulus must be greater than plaintext_modulus`,
      `${context}.parameters.ciphertext_modulus`,
    );
  }
  if (ciphertextModulus % plaintextModulus !== 0n) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.ciphertext_modulus must be divisible by plaintext_modulus`,
      `${context}.parameters.ciphertext_modulus`,
    );
  }
  if (maxInputBytes < 1) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.max_input_bytes must be at least 1`,
      `${context}.max_input_bytes`,
    );
  }
  if (BigInt(maxInputBytes) >= plaintextModulus) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.max_input_bytes must fit into one plaintext slot`,
      `${context}.max_input_bytes`,
    );
  }
  const publicKey = publicParameters.public_key;
  const b = publicKey.b.map((value, index) =>
    requireSafeBfvUint(value, `${context}.public_key.b[${index}]`),
  );
  const a = publicKey.a.map((value, index) =>
    requireSafeBfvUint(value, `${context}.public_key.a[${index}]`),
  );
  if (a.length !== polynomialDegree || b.length !== polynomialDegree) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.public_key polynomials must match polynomial_degree`,
      `${context}.public_key`,
    );
  }
  for (const [arrayName, coefficients] of [
    ["a", a],
    ["b", b],
  ]) {
    for (let index = 0; index < coefficients.length; index += 1) {
      if (coefficients[index] >= ciphertextModulus) {
        throw createValidationError(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${context}.public_key.${arrayName}[${index}] exceeds ciphertext_modulus`,
          `${context}.public_key.${arrayName}[${index}]`,
        );
      }
    }
  }
  return {
    polynomialDegree,
    plaintextModulus,
    ciphertextModulus,
    maxInputBytes,
    delta: ciphertextModulus / plaintextModulus,
    publicKey: { a, b },
  };
}

function addModBigInt(lhs, rhs, modulus) {
  return (lhs + rhs) % modulus;
}

function subModBigInt(lhs, rhs, modulus) {
  return lhs >= rhs ? lhs - rhs : modulus - ((rhs - lhs) % modulus);
}

function mulModBigInt(lhs, rhs, modulus) {
  return (lhs * rhs) % modulus;
}

function polyAddMod(params, lhs, rhs) {
  return lhs.map((value, index) =>
    addModBigInt(value, rhs[index], params.ciphertextModulus),
  );
}

function polySubMod(params, lhs, rhs) {
  return lhs.map((value, index) =>
    subModBigInt(value, rhs[index], params.ciphertextModulus),
  );
}

function polyMulMod(params, lhs, rhs) {
  const out = Array.from({ length: params.polynomialDegree }, () => 0n);
  for (let i = 0; i < params.polynomialDegree; i += 1) {
    for (let j = 0; j < params.polynomialDegree; j += 1) {
      const term = mulModBigInt(lhs[i], rhs[j], params.ciphertextModulus);
      const target = i + j;
      if (target < params.polynomialDegree) {
        out[target] = addModBigInt(out[target], term, params.ciphertextModulus);
      } else {
        out[target - params.polynomialDegree] = subModBigInt(
          out[target - params.polynomialDegree],
          term,
          params.ciphertextModulus,
        );
      }
    }
  }
  return out;
}

function encodeIdentifierSlots(params, inputBytes) {
  if (inputBytes.length > params.maxInputBytes) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `encryptIdentifierInputForPolicy.input exceeds max_input_bytes ${params.maxInputBytes}`,
      "encryptIdentifierInputForPolicy.input",
    );
  }
  const slots = Array.from({ length: params.maxInputBytes + 1 }, () => 0n);
  slots[0] = BigInt(inputBytes.length);
  for (let index = 0; index < inputBytes.length; index += 1) {
    slots[index + 1] = BigInt(inputBytes[index]);
  }
  return slots;
}

function sampleSmallPoly(params, stream) {
  return Array.from({ length: params.polynomialDegree }, () => {
    const sample = Number(stream.nextBytes(1)[0] % 3);
    if (sample === 0) {
      return 0n;
    }
    if (sample === 1) {
      return 1n;
    }
    return params.ciphertextModulus - 1n;
  });
}

function sampleUniformPoly(params, stream) {
  const bound = 1n << 64n;
  const threshold = bound - (bound % params.ciphertextModulus);
  return Array.from({ length: params.polynomialDegree }, () => {
    let candidate = stream.nextU64();
    while (candidate >= threshold) {
      candidate = stream.nextU64();
    }
    return candidate % params.ciphertextModulus;
  });
}

function encryptIdentifierScalar(params, scalar, seed) {
  const u = sampleSmallPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_U_DOMAIN),
  );
  const e1 = sampleSmallPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_E1_DOMAIN),
  );
  const e2 = sampleSmallPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_E2_DOMAIN),
  );
  const encoded = Array.from({ length: params.polynomialDegree }, () => 0n);
  encoded[0] = mulModBigInt(scalar, params.delta, params.ciphertextModulus);
  return {
    c0: polyAddMod(
      params,
      polyAddMod(params, polyMulMod(params, params.publicKey.b, u), e1),
      encoded,
    ),
    c1: polyAddMod(params, polyMulMod(params, params.publicKey.a, u), e2),
  };
}

function normalizeIdentifierPolicyListResponse(
  payload,
  context = "identifier policy list response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const itemsValue = record.items;
  if (!Array.isArray(itemsValue)) {
    throw new Error(`${context}.items must be an array`);
  }
  return {
    total: ToriiClient._normalizeUnsignedInteger(record.total ?? itemsValue.length, `${context}.total`, {
      allowZero: true,
    }),
    items: itemsValue.map((item, index) =>
      normalizeIdentifierPolicySummary(item, `${context}.items[${index}]`),
    ),
  };
}

function normalizeRamLfeProgramPolicyListResponse(
  payload,
  context = "ram-lfe program policy list response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const itemsValue = record.items;
  if (!Array.isArray(itemsValue)) {
    throw new Error(`${context}.items must be an array`);
  }
  return {
    total: ToriiClient._normalizeUnsignedInteger(record.total ?? itemsValue.length, `${context}.total`, {
      allowZero: true,
    }),
    items: itemsValue.map((item, index) =>
      normalizeRamLfeProgramPolicySummary(item, `${context}.items[${index}]`),
    ),
  };
}

function normalizeRamLfeProgramPolicySummary(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const result = {
    program_id: requireNonEmptyString(record.program_id, `${context}.program_id`),
    owner: ToriiClient._requireAccountId(record.owner, `${context}.owner`),
    active: record.active === true,
    resolver_public_key: requireNonEmptyString(
      record.resolver_public_key,
      `${context}.resolver_public_key`,
    ),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    verification_mode: requireNonEmptyString(
      record.verification_mode,
      `${context}.verification_mode`,
    ).toLowerCase(),
  };
  if (record.input_encryption !== undefined && record.input_encryption !== null) {
    result.input_encryption = requireNonEmptyString(
      record.input_encryption,
      `${context}.input_encryption`,
    );
  }
  if (
    record.input_encryption_public_parameters !== undefined &&
    record.input_encryption_public_parameters !== null
  ) {
    result.input_encryption_public_parameters = requireHexString(
      record.input_encryption_public_parameters,
      `${context}.input_encryption_public_parameters`,
    );
  }
  if (
    record.input_encryption_public_parameters_decoded !== undefined &&
    record.input_encryption_public_parameters_decoded !== null
  ) {
    result.input_encryption_public_parameters_decoded =
      normalizeIdentifierBfvPublicParameters(
        record.input_encryption_public_parameters_decoded,
        `${context}.input_encryption_public_parameters_decoded`,
      );
  }
  if (record.note !== undefined && record.note !== null) {
    result.note = requireNonEmptyString(record.note, `${context}.note`);
  }
  return result;
}

function normalizeIdentifierPolicySummary(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const result = {
    policy_id: requireNonEmptyString(record.policy_id, `${context}.policy_id`),
    owner: ToriiClient._requireAccountId(record.owner, `${context}.owner`),
    active: record.active === true,
    normalization: requireNonEmptyString(record.normalization, `${context}.normalization`),
    resolver_public_key: requireNonEmptyString(
      record.resolver_public_key,
      `${context}.resolver_public_key`,
    ),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
  };
  if (record.input_encryption !== undefined && record.input_encryption !== null) {
    result.input_encryption = requireNonEmptyString(
      record.input_encryption,
      `${context}.input_encryption`,
    );
  }
  if (
    record.input_encryption_public_parameters !== undefined &&
    record.input_encryption_public_parameters !== null
  ) {
    result.input_encryption_public_parameters = requireHexString(
      record.input_encryption_public_parameters,
      `${context}.input_encryption_public_parameters`,
    );
  }
  if (
    record.input_encryption_public_parameters_decoded !== undefined &&
    record.input_encryption_public_parameters_decoded !== null
  ) {
    result.input_encryption_public_parameters_decoded =
      normalizeIdentifierBfvPublicParameters(
        record.input_encryption_public_parameters_decoded,
        `${context}.input_encryption_public_parameters_decoded`,
      );
  }
  if (record.note !== undefined && record.note !== null) {
    result.note = requireNonEmptyString(record.note, `${context}.note`);
  }
  return result;
}

function normalizeIdentifierResolutionPayload(
  payload,
  context = "identifier resolution payload",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    policy_id: requireNonEmptyString(record.policy_id, `${context}.policy_id`),
    opaque_id: normalizeOpaqueLiteral(record.opaque_id, `${context}.opaque_id`),
    receipt_hash: normalizeHex32String(record.receipt_hash, `${context}.receipt_hash`),
    uaid: normalizeUaidLiteral(record.uaid, `${context}.uaid`),
    account_id: ToriiClient._requireAccountId(record.account_id, `${context}.account_id`),
    resolved_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.resolved_at_ms,
      `${context}.resolved_at_ms`,
      { allowZero: true },
    ),
    expires_at_ms:
      record.expires_at_ms === undefined || record.expires_at_ms === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
          record.expires_at_ms,
          `${context}.expires_at_ms`,
          { allowZero: true },
        ),
  };
}

function normalizeIdentifierResolveResponse(
  payload,
  context = "identifier resolve response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const result = {
    policy_id: requireNonEmptyString(record.policy_id, `${context}.policy_id`),
    opaque_id: normalizeOpaqueLiteral(record.opaque_id, `${context}.opaque_id`),
    receipt_hash: normalizeHex32String(record.receipt_hash, `${context}.receipt_hash`),
    uaid: normalizeUaidLiteral(record.uaid, `${context}.uaid`),
    account_id: ToriiClient._requireAccountId(record.account_id, `${context}.account_id`),
    resolved_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.resolved_at_ms,
      `${context}.resolved_at_ms`,
      { allowZero: true },
    ),
    expires_at_ms:
      record.expires_at_ms === undefined || record.expires_at_ms === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
          record.expires_at_ms,
          `${context}.expires_at_ms`,
          { allowZero: true },
        ),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    signature: requireHexString(record.signature, `${context}.signature`).toUpperCase(),
    signature_payload_hex: requireHexString(
      record.signature_payload_hex,
      `${context}.signature_payload_hex`,
    ).toUpperCase(),
    signature_payload: normalizeIdentifierResolutionPayload(
      record.signature_payload,
      `${context}.signature_payload`,
    ),
  };
  return result;
}

function normalizeIdentifierClaimLookupResponse(
  payload,
  context = "identifier claim lookup response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    policy_id: requireNonEmptyString(record.policy_id, `${context}.policy_id`),
    opaque_id: normalizeOpaqueLiteral(record.opaque_id, `${context}.opaque_id`),
    receipt_hash: normalizeHex32String(record.receipt_hash, `${context}.receipt_hash`),
    uaid: normalizeUaidLiteral(record.uaid, `${context}.uaid`),
    account_id: ToriiClient._requireAccountId(record.account_id, `${context}.account_id`),
    verified_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.verified_at_ms,
      `${context}.verified_at_ms`,
      { allowZero: true },
    ),
    expires_at_ms:
      record.expires_at_ms === undefined || record.expires_at_ms === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
          record.expires_at_ms,
          `${context}.expires_at_ms`,
          { allowZero: true },
    ),
  };
}

function normalizeRamLfeExecuteResponse(
  payload,
  context = "ram-lfe execute response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    program_id: requireNonEmptyString(record.program_id, `${context}.program_id`),
    opaque_hash: requireNonEmptyString(record.opaque_hash, `${context}.opaque_hash`),
    receipt_hash: requireNonEmptyString(record.receipt_hash, `${context}.receipt_hash`),
    output_hex: requireHexString(record.output_hex, `${context}.output_hex`),
    output_hash: requireNonEmptyString(record.output_hash, `${context}.output_hash`),
    associated_data_hash: requireNonEmptyString(
      record.associated_data_hash,
      `${context}.associated_data_hash`,
    ),
    executed_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.executed_at_ms,
      `${context}.executed_at_ms`,
      { allowZero: true },
    ),
    expires_at_ms:
      record.expires_at_ms === undefined || record.expires_at_ms === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
          record.expires_at_ms,
          `${context}.expires_at_ms`,
          { allowZero: true },
        ),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    verification_mode: requireNonEmptyString(
      record.verification_mode,
      `${context}.verification_mode`,
    ).toLowerCase(),
    receipt: ensureRecord(record.receipt ?? {}, `${context}.receipt`),
  };
}

function normalizeRamLfeReceiptVerifyResponse(
  payload,
  context = "ram-lfe receipt verify response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const result = {
    valid: record.valid === true,
    program_id: requireNonEmptyString(record.program_id, `${context}.program_id`),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    verification_mode: requireNonEmptyString(
      record.verification_mode,
      `${context}.verification_mode`,
    ).toLowerCase(),
    output_hash: requireNonEmptyString(record.output_hash, `${context}.output_hash`),
    associated_data_hash: requireNonEmptyString(
      record.associated_data_hash,
      `${context}.associated_data_hash`,
    ),
  };
  if (record.output_hash_matches !== undefined && record.output_hash_matches !== null) {
    result.output_hash_matches = record.output_hash_matches === true;
  }
  if (record.error !== undefined && record.error !== null) {
    result.error = requireNonEmptyString(record.error, `${context}.error`);
  }
  return result;
}

function buildSorafsAliasListParams(options = {}) {
  const record = ensureRecord(options, "listSorafsAliases options");
  assertSupportedOptionKeys(
    record,
    new Set(["namespace", "manifestDigestHex", "limit", "offset"]),
    "listSorafsAliases options",
  );
  const params = {};
  if (record.namespace !== undefined && record.namespace !== null) {
    params.namespace = requireNonEmptyString(
      record.namespace,
      "sorafsAliasList.namespace",
    );
  }
  if (record.manifestDigestHex !== undefined && record.manifestDigestHex !== null) {
    params.manifest_digest = normalizeHex32String(
      record.manifestDigestHex,
      "sorafsAliasList.manifestDigestHex",
    );
  }
  if (record.limit !== undefined && record.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      record.limit,
      "sorafsAliasList.limit",
      { allowZero: false },
    );
  }
  if (record.offset !== undefined && record.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      record.offset,
      "sorafsAliasList.offset",
      { allowZero: true },
    );
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildSorafsPinListParams(options = {}) {
  const record = ensureRecord(options, "listSorafsPinManifests options");
  assertSupportedOptionKeys(
    record,
    new Set(["status", "limit", "offset"]),
    "listSorafsPinManifests options",
  );
  const params = {};
  if (record.status !== undefined && record.status !== null) {
    const normalized = requireNonEmptyString(
      record.status,
      "sorafsPinList.status",
    ).toLowerCase();
    if (!SORAFS_PIN_STATUS_VALUES.has(normalized)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "sorafsPinList.status must be pending, approved, or retired",
        "sorafsPinList.status",
      );
    }
    params.status = normalized;
  }
  if (record.limit !== undefined && record.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      record.limit,
      "sorafsPinList.limit",
      { allowZero: false },
    );
  }
  if (record.offset !== undefined && record.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      record.offset,
      "sorafsPinList.offset",
      { allowZero: true },
    );
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildSorafsReplicationListParams(options = {}) {
  const record = ensureRecord(options, "listSorafsReplicationOrders options");
  assertSupportedOptionKeys(
    record,
    new Set(["status", "manifestDigestHex", "limit", "offset"]),
    "listSorafsReplicationOrders options",
  );
  const params = {};
  if (record.status !== undefined && record.status !== null) {
    const normalized = requireNonEmptyString(
      record.status,
      "sorafsReplicationList.status",
    ).toLowerCase();
    if (!SORAFS_REPLICATION_STATUS_VALUES.has(normalized)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "sorafsReplicationList.status must be pending, completed, or expired",
        "sorafsReplicationList.status",
      );
    }
    params.status = normalized;
  }
  if (record.manifestDigestHex !== undefined && record.manifestDigestHex !== null) {
    params.manifest_digest = normalizeHex32String(
      record.manifestDigestHex,
      "sorafsReplicationList.manifestDigestHex",
    );
  }
  if (record.limit !== undefined && record.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      record.limit,
      "sorafsReplicationList.limit",
      { allowZero: false },
    );
  }
  if (record.offset !== undefined && record.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      record.offset,
      "sorafsReplicationList.offset",
      { allowZero: true },
    );
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildSorafsPorStatusParams(options = {}) {
  const record = ensureRecord(options, "getSorafsPorStatus options");
  assertSupportedOptionKeys(
    record,
    new Set([
      "manifest",
      "manifestHex",
      "provider",
      "providerHex",
      "epoch",
      "status",
      "limit",
      "page_token",
      "pageToken",
      "pageTokenHex",
    ]),
    "getSorafsPorStatus options",
  );
  const params = {};
  const manifest = record.manifest;
  if (manifest !== undefined && manifest !== null) {
    params.manifest = requireHexString(
      manifest,
      "sorafsPorStatus.manifestHex",
    );
  }
  const provider = record.provider;
  if (provider !== undefined && provider !== null) {
    params.provider = requireHexString(
      provider,
      "sorafsPorStatus.providerHex",
    );
  }
  if (record.epoch !== undefined && record.epoch !== null) {
    params.epoch = ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      "sorafsPorStatus.epoch",
      { allowZero: false },
    );
  }
  if (record.status !== undefined && record.status !== null) {
    params.status = requireNonEmptyString(
      record.status,
      "sorafsPorStatus.status",
    ).trim();
  }
  if (record.limit !== undefined && record.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      record.limit,
      "sorafsPorStatus.limit",
      { allowZero: false },
    );
  }
  const pageToken = record.page_token;
  if (pageToken !== undefined && pageToken !== null) {
    params.page_token = requireHexString(
      pageToken,
      "sorafsPorStatus.pageTokenHex",
    );
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildSorafsPorExportParams(options = {}) {
  const record = ensureRecord(options, "exportSorafsPorStatus options");
  assertSupportedOptionKeys(
    record,
    new Set(["start_epoch", "startEpoch", "end_epoch", "endEpoch"]),
    "exportSorafsPorStatus options",
  );
  const params = {};
  if (record.start_epoch !== undefined || record.startEpoch !== undefined) {
    params.start_epoch = ToriiClient._normalizeUnsignedInteger(
      record.start_epoch,
      "sorafsPorExport.startEpoch",
      { allowZero: false },
    );
  }
  if (record.end_epoch !== undefined || record.endEpoch !== undefined) {
    params.end_epoch = ToriiClient._normalizeUnsignedInteger(
      record.end_epoch,
      "sorafsPorExport.endEpoch",
      { allowZero: false },
    );
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function normalizeSorafsPinListResponse(payload, context = "sorafs pin list response") {
  const record = ensureRecord(payload ?? {}, context);
  const manifestsValue = record.manifests;
  if (!Array.isArray(manifestsValue)) {
    throw new TypeError(`${context}.manifests must be an array`);
  }
  return {
    attestation: optionalRecord(record.attestation, `${context}.attestation`),
    total_count: ToriiClient._normalizeUnsignedInteger(
      record.total_count,
      `${context}.total_count`,
      { allowZero: true },
    ),
    returned_count: ToriiClient._normalizeUnsignedInteger(
      record.returned_count,
      `${context}.returned_count`,
      { allowZero: true },
    ),
    offset: ToriiClient._normalizeUnsignedInteger(record.offset, `${context}.offset`, {
      allowZero: true,
    }),
    limit: ToriiClient._normalizeUnsignedInteger(record.limit, `${context}.limit`, {
      allowZero: false,
    }),
    manifests: manifestsValue.map((entry, index) =>
      normalizeSorafsManifestRecord(entry, `${context}.manifests[${index}]`),
    ),
  };
}

function normalizeSorafsPinManifestResponse(
  payload,
  context = "sorafs pin manifest response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const aliasesValue = record.aliases;
  if (!Array.isArray(aliasesValue)) {
    throw new TypeError(`${context}.aliases must be an array`);
  }
  const ordersValue = record.replication_orders;
  if (!Array.isArray(ordersValue)) {
    throw new TypeError(`${context}.replication_orders must be an array`);
  }
  return {
    attestation: optionalRecord(record.attestation, `${context}.attestation`),
    manifest: normalizeSorafsManifestRecord(record.manifest, `${context}.manifest`),
    aliases: aliasesValue.map((entry, index) =>
      normalizeSorafsAliasRecord(entry, `${context}.aliases[${index}]`),
    ),
    replication_orders: ordersValue.map((entry, index) =>
      normalizeSorafsReplicationOrderRecord(
        entry,
        `${context}.replication_orders[${index}]`,
      ),
    ),
  };
}

function normalizeSorafsManifestRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    digest_hex: normalizeHex32String(record.digest_hex, `${context}.digest_hex`),
    chunker: normalizeSorafsChunkerHandle(record.chunker, `${context}.chunker`),
    chunk_digest_sha3_256_hex: normalizeHex32String(
      record.chunk_digest_sha3_256_hex,
      `${context}.chunk_digest_sha3_256_hex`,
    ),
    pin_policy: optionalRecord(record.pin_policy, `${context}.pin_policy`) ?? {},
    submitted_by: ToriiClient._requireAccountId(record.submitted_by, `${context}.submitted_by`),
    submitted_epoch: ToriiClient._normalizeUnsignedInteger(
      record.submitted_epoch,
      `${context}.submitted_epoch`,
      { allowZero: true },
    ),
    status: normalizeSorafsManifestStatus(record.status, `${context}.status`),
    metadata: optionalRecord(record.metadata, `${context}.metadata`) ?? {},
    alias:
      record.alias === undefined || record.alias === null
        ? null
        : normalizeSorafsManifestAlias(record.alias, `${context}.alias`),
    successor_of_hex:
      record.successor_of_hex === undefined || record.successor_of_hex === null
        ? null
        : normalizeHex32String(record.successor_of_hex, `${context}.successor_of_hex`),
    status_timestamp_unix: optionalNumber(
      record.status_timestamp_unix,
      `${context}.status_timestamp_unix`,
    ),
    governance_refs: Array.isArray(record.governance_refs)
      ? record.governance_refs.map((entry, index) =>
          normalizeSorafsGovernanceReference(entry, `${context}.governance_refs[${index}]`),
        )
      : [],
    council_envelope_digest_hex:
      record.council_envelope_digest_hex === undefined || record.council_envelope_digest_hex === null
        ? null
        : normalizeHex32String(
            record.council_envelope_digest_hex,
            `${context}.council_envelope_digest_hex`,
          ),
    lineage:
      record.lineage === undefined || record.lineage === null
        ? null
        : normalizeSorafsLineage(record.lineage, `${context}.lineage`),
  };
}

function normalizeSorafsChunkerHandle(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    profile_id: ToriiClient._normalizeUnsignedInteger(record.profile_id, `${context}.profile_id`, {
      allowZero: false,
    }),
    namespace: requireNonEmptyString(record.namespace, `${context}.namespace`),
    name: requireNonEmptyString(record.name, `${context}.name`),
    semver: requireNonEmptyString(record.semver, `${context}.semver`),
    multihash_code: ToriiClient._normalizeUnsignedInteger(
      record.multihash_code,
      `${context}.multihash_code`,
      { allowZero: true },
    ),
  };
}

function normalizeSorafsManifestStatus(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawState = requireNonEmptyString(record.state, `${context}.state`);
  const state = rawState.toLowerCase();
  if (!SORAFS_PIN_STATUS_VALUES.has(state)) {
    throw new TypeError(`${context}.state must be pending, approved, or retired`);
  }
  return {
    state,
    epoch:
      record.epoch === undefined || record.epoch === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(record.epoch, `${context}.epoch`, {
            allowZero: true,
          }),
  };
}

function normalizeSorafsManifestAlias(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    namespace: requireNonEmptyString(record.namespace, `${context}.namespace`),
    name: requireNonEmptyString(record.name, `${context}.name`),
    proof_b64: requireNonEmptyString(record.proof_b64, `${context}.proof_b64`),
  };
}

function normalizeSorafsGovernanceReference(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const targetsValue = optionalRecord(record.targets, `${context}.targets`) ?? {};
  return {
    cid: optionalString(record.cid, `${context}.cid`),
    kind: requireNonEmptyString(record.kind, `${context}.kind`),
    effective_at: optionalString(record.effective_at, `${context}.effective_at`),
    effective_at_unix: optionalNumber(record.effective_at_unix, `${context}.effective_at_unix`),
    targets: {
      alias: optionalString(targetsValue.alias, `${context}.targets.alias`),
      pin_digest_hex:
        targetsValue.pin_digest_hex === undefined || targetsValue.pin_digest_hex === null
          ? null
          : normalizeHex32String(targetsValue.pin_digest_hex, `${context}.targets.pin_digest_hex`),
    },
    signers: optionalStringArray(record.signers, `${context}.signers`) ?? [],
  };
}

function normalizeSorafsLineage(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  if (record.is_head === undefined || typeof record.is_head !== "boolean") {
    throw new TypeError(`${context}.is_head must be a boolean`);
  }
  return {
    successor_of_hex:
      record.successor_of_hex === undefined || record.successor_of_hex === null
        ? null
        : normalizeHex32String(record.successor_of_hex, `${context}.successor_of_hex`),
    head_hex: normalizeHex32String(record.head_hex, `${context}.head_hex`),
    depth_to_head: ToriiClient._normalizeUnsignedInteger(
      record.depth_to_head,
      `${context}.depth_to_head`,
      { allowZero: true },
    ),
    is_head: record.is_head,
    superseded_by:
      record.superseded_by === undefined || record.superseded_by === null
        ? null
        : normalizeSorafsLineageSuccessor(record.superseded_by, `${context}.superseded_by`),
    immediate_successor:
      record.immediate_successor === undefined || record.immediate_successor === null
        ? null
        : normalizeSorafsLineageSuccessor(record.immediate_successor, `${context}.immediate_successor`),
    anomalies: optionalStringArray(record.anomalies, `${context}.anomalies`) ?? [],
  };
}

function normalizeSorafsLineageSuccessor(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    digest_hex: normalizeHex32String(record.digest_hex, `${context}.digest_hex`),
    status: normalizeSorafsManifestStatus(record.status, `${context}.status`),
    approved_epoch:
      record.approved_epoch === undefined || record.approved_epoch === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(record.approved_epoch, `${context}.approved_epoch`, {
            allowZero: true,
          }),
    approved_at: optionalString(record.approved_at, `${context}.approved_at`),
    status_timestamp_unix: optionalNumber(
      record.status_timestamp_unix,
      `${context}.status_timestamp_unix`,
    ),
  };
}

function normalizeSorafsAliasListResponse(
  payload,
  context = "sorafs alias list response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const aliasesValue = record.aliases;
  if (!Array.isArray(aliasesValue)) {
    throw new TypeError(`${context}.aliases must be an array`);
  }
  return {
    attestation: optionalRecord(record.attestation, `${context}.attestation`),
    total_count: ToriiClient._normalizeUnsignedInteger(
      record.total_count,
      `${context}.total_count`,
      { allowZero: true },
    ),
    returned_count: ToriiClient._normalizeUnsignedInteger(
      record.returned_count,
      `${context}.returned_count`,
      { allowZero: true },
    ),
    offset: ToriiClient._normalizeUnsignedInteger(record.offset, `${context}.offset`, {
      allowZero: true,
    }),
    limit: ToriiClient._normalizeUnsignedInteger(record.limit, `${context}.limit`, {
      allowZero: false,
    }),
    aliases: aliasesValue.map((entry, index) =>
      normalizeSorafsAliasRecord(entry, `${context}.aliases[${index}]`),
    ),
  };
}

function normalizeSorafsAliasRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    alias: requireNonEmptyString(record.alias, `${context}.alias`),
    namespace: requireNonEmptyString(record.namespace, `${context}.namespace`),
    name: requireNonEmptyString(record.name, `${context}.name`),
    manifest_digest_hex: normalizeHex32String(
      record.manifest_digest_hex,
      `${context}.manifest_digest_hex`,
    ),
    bound_by: ToriiClient._requireAccountId(record.bound_by, `${context}.bound_by`),
    bound_epoch: ToriiClient._normalizeUnsignedInteger(
      record.bound_epoch,
      `${context}.bound_epoch`,
      { allowZero: true },
    ),
    expiry_epoch: ToriiClient._normalizeUnsignedInteger(
      record.expiry_epoch,
      `${context}.expiry_epoch`,
      { allowZero: true },
    ),
    proof_b64: requireNonEmptyString(record.proof_b64, `${context}.proof_b64`),
    cache_state: optionalString(record.cache_state, `${context}.cache_state`),
    status_label: optionalString(record.status_label, `${context}.status_label`),
    cache_rotation_due: optionalBoolean(
      record.cache_rotation_due,
      `${context}.cache_rotation_due`,
    ),
    cache_age_seconds: optionalNumber(record.cache_age_seconds, `${context}.cache_age_seconds`),
    proof_generated_at_unix: optionalNumber(
      record.proof_generated_at_unix,
      `${context}.proof_generated_at_unix`,
    ),
    proof_expires_at_unix: optionalNumber(
      record.proof_expires_at_unix,
      `${context}.proof_expires_at_unix`,
    ),
    proof_expires_in_seconds: optionalNumber(
      record.proof_expires_in_seconds,
      `${context}.proof_expires_in_seconds`,
    ),
    policy_positive_ttl_secs: optionalNumber(
      record.policy_positive_ttl_secs,
      `${context}.policy_positive_ttl_secs`,
    ),
    policy_refresh_window_secs: optionalNumber(
      record.policy_refresh_window_secs,
      `${context}.policy_refresh_window_secs`,
    ),
    policy_hard_expiry_secs: optionalNumber(
      record.policy_hard_expiry_secs,
      `${context}.policy_hard_expiry_secs`,
    ),
    policy_rotation_max_age_secs: optionalNumber(
      record.policy_rotation_max_age_secs,
      `${context}.policy_rotation_max_age_secs`,
    ),
    policy_successor_grace_secs: optionalNumber(
      record.policy_successor_grace_secs,
      `${context}.policy_successor_grace_secs`,
    ),
    policy_governance_grace_secs: optionalNumber(
      record.policy_governance_grace_secs,
      `${context}.policy_governance_grace_secs`,
    ),
    cache_evaluation: optionalRecord(record.cache_evaluation, `${context}.cache_evaluation`),
    cache_decision: optionalString(record.cache_decision, `${context}.cache_decision`),
    cache_reasons: optionalStringArray(record.cache_reasons, `${context}.cache_reasons`),
    lineage: optionalRecord(record.lineage, `${context}.lineage`),
  };
}

function normalizeSorafsReplicationListResponse(
  payload,
  context = "sorafs replication list response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const ordersValue = record.replication_orders;
  if (!Array.isArray(ordersValue)) {
    throw new TypeError(`${context}.replication_orders must be an array`);
  }
  return {
    attestation: optionalRecord(record.attestation, `${context}.attestation`),
    total_count: ToriiClient._normalizeUnsignedInteger(
      record.total_count,
      `${context}.total_count`,
      { allowZero: true },
    ),
    returned_count: ToriiClient._normalizeUnsignedInteger(
      record.returned_count,
      `${context}.returned_count`,
      { allowZero: true },
    ),
    offset: ToriiClient._normalizeUnsignedInteger(record.offset, `${context}.offset`, {
      allowZero: true,
    }),
    limit: ToriiClient._normalizeUnsignedInteger(record.limit, `${context}.limit`, {
      allowZero: false,
    }),
    replication_orders: ordersValue.map((entry, index) =>
      normalizeSorafsReplicationOrderRecord(entry, `${context}.replication_orders[${index}]`),
    ),
  };
}

function normalizeSorafsReplicationOrderRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const receiptsValue = record.receipts ?? [];
  if (!Array.isArray(receiptsValue)) {
    throw new TypeError(`${context}.receipts must be an array`);
  }
  return {
    order_id_hex: normalizeHex32String(record.order_id_hex, `${context}.order_id_hex`),
    manifest_digest_hex: normalizeHex32String(
      record.manifest_digest_hex,
      `${context}.manifest_digest_hex`,
    ),
    issued_by: ToriiClient._requireAccountId(record.issued_by, `${context}.issued_by`),
    issued_epoch: ToriiClient._normalizeUnsignedInteger(
      record.issued_epoch,
      `${context}.issued_epoch`,
      { allowZero: true },
    ),
    deadline_epoch: ToriiClient._normalizeUnsignedInteger(
      record.deadline_epoch,
      `${context}.deadline_epoch`,
      { allowZero: true },
    ),
    status: normalizeSorafsReplicationStatus(record.status, `${context}.status`),
    canonical_order_b64: requireNonEmptyString(
      record.canonical_order_b64,
      `${context}.canonical_order_b64`,
    ),
    order: optionalRecord(record.order, `${context}.order`) ?? {},
    receipts: receiptsValue.map((entry, index) =>
      normalizeSorafsReplicationReceipt(entry, `${context}.receipts[${index}]`),
    ),
    providers: requireStringArray(record.providers ?? [], `${context}.providers`),
  };
}

function normalizeSorafsReplicationStatus(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    state: requireNonEmptyString(record.state, `${context}.state`),
    epoch:
      record.epoch === undefined || record.epoch === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(record.epoch, `${context}.epoch`, {
            allowZero: true,
          }),
  };
}

function normalizeSorafsReplicationReceipt(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    provider_hex: normalizeHex32String(record.provider_hex, `${context}.provider_hex`),
    status: requireNonEmptyString(record.status, `${context}.status`),
    timestamp: ToriiClient._normalizeUnsignedInteger(record.timestamp, `${context}.timestamp`, {
      allowZero: true,
    }),
    por_sample_digest_hex:
      record.por_sample_digest_hex === undefined || record.por_sample_digest_hex === null
        ? null
        : normalizeHex32String(
            record.por_sample_digest_hex,
            `${context}.por_sample_digest_hex`,
          ),
  };
}

function normalizeSorafsPinResponse(payload, context = "sorafs pin response") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    manifest_id_hex: normalizeHex32String(
      record.manifest_id_hex ?? "",
      `${context}.manifest_id_hex`,
    ),
    payload_digest_hex: normalizeHex32String(
      record.payload_digest_hex ?? "",
      `${context}.payload_digest_hex`,
    ),
    content_length: ToriiClient._normalizeUnsignedInteger(
      record.content_length,
      `${context}.content_length`,
      { allowZero: true },
    ),
  };
}

function normalizeSorafsPinRegisterResponse(
  payload,
  context = "sorafs pin register response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const aliasValue =
    record.alias === undefined || record.alias === null
      ? null
      : normalizeSorafsPinAliasRequest(record.alias, `${context}.alias`);
  const successorValue = record.successor_of_hex ?? record.successorOfHex ?? null;
  return {
    manifest_digest_hex: normalizeHex32String(
      record.manifest_digest_hex ?? record.manifestDigestHex,
      `${context}.manifest_digest_hex`,
    ),
    chunker_handle: requireNonEmptyString(
      record.chunker_handle ?? record.chunkerHandle,
      `${context}.chunker_handle`,
    ),
    submitted_epoch: ToriiClient._normalizeUnsignedInteger(
      record.submitted_epoch ?? record.submittedEpoch,
      `${context}.submitted_epoch`,
      { allowZero: true },
    ),
    alias: aliasValue,
    successor_of_hex:
      successorValue === null
        ? null
        : normalizeHex32String(successorValue, `${context}.successor_of_hex`),
  };
}

function normalizePipelineTransactionStatus(
  payload,
  context = "pipeline transaction status response",
) {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new TypeError(`${context} must be an object`);
  }
  if (!("content" in payload) && typeof payload.status === "string") {
    return { status: String(payload.status) };
  }
  const record = ensureRecord(payload ?? {}, context);
  const kindValue = record.kind;
  const kind = kindValue == null ? "Unknown" : String(kindValue);
  const contentRecord = ensureRecord(record.content, `${context}.content`);
  const hash = normalizeHex32String(
    contentRecord.hash ?? "",
    `${context}.content.hash`,
  );
  const statusRecord = ensureRecord(
    contentRecord.status,
    `${context}.content.status`,
  );
  const statusKindValue = statusRecord.kind;
  const statusKind =
    statusKindValue == null ? "Unknown" : String(statusKindValue);
  const statusContent =
    statusRecord.content === undefined ? null : statusRecord.content;
  const normalizedStatus = {
    ...statusRecord,
    kind: statusKind,
    content: statusContent,
  };
  const normalizedContent = {
    ...contentRecord,
    hash,
    status: normalizedStatus,
  };
  return {
    ...record,
    kind,
    content: normalizedContent,
  };
}

function normalizePipelineRecoverySidecar(payload, context = "pipeline recovery response") {
  const record = ensureRecord(payload ?? {}, context);
  const format = requireNonEmptyString(
    record.format ?? "",
    `${context}.format`,
  );
  const height = ToriiClient._normalizeUnsignedInteger(record.height, `${context}.height`, {
    allowZero: true,
  });
  const dag = normalizePipelineDagSnapshot(record.dag, `${context}.dag`);
  const rawTxs = record.txs ?? [];
  if (!Array.isArray(rawTxs)) {
    throw new TypeError(`${context}.txs must be an array`);
  }
  return {
    format,
    height,
    dag,
    txs: rawTxs.map((entry, index) =>
      normalizePipelineTxSnapshot(entry, `${context}.txs[${index}]`),
    ),
  };
}

function normalizePipelineDagSnapshot(payload, context = "pipeline recovery dag") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    fingerprintHex: normalizeHex32String(
      record.fingerprint ?? "",
      `${context}.fingerprint`,
    ),
    keyCount: ToriiClient._normalizeUnsignedInteger(
      record.key_count,
      `${context}.key_count`,
      { allowZero: true },
    ),
  };
}

function normalizePipelineTxSnapshot(payload, context = "pipeline recovery tx") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    hashHex: normalizeHex32String(
      record.hash ?? "",
      `${context}.hash`,
    ),
    reads: normalizeStringArray(record.reads ?? [], `${context}.reads`),
    writes: normalizeStringArray(record.writes ?? [], `${context}.writes`),
  };
}

function normalizeSorafsFetchResponse(payload, context = "sorafs fetch response") {
  const record = ensureRecord(payload ?? {}, context);
  return {
    manifest_id_hex: normalizeHex32String(
      record.manifest_id_hex ?? "",
      `${context}.manifest_id_hex`,
    ),
    offset: ToriiClient._normalizeUnsignedInteger(
      record.offset,
      `${context}.offset`,
      { allowZero: true },
    ),
    length: ToriiClient._normalizeUnsignedInteger(
      record.length,
      `${context}.length`,
      { allowZero: true },
    ),
    data_b64: normalizeRequiredBase64Payload(
      record.data_b64,
      `${context}.data_b64`,
    ),
  };
}

function normalizeSorafsStorageStateResponse(
  payload,
  context = "sorafs storage state response",
) {
  const record = ensureRecord(payload ?? {}, context);
  const normalize = (field, allowZero = true) =>
    ToriiClient._normalizeUnsignedInteger(record[field], `${context}.${field}`, {
      allowZero,
    });
  return {
    bytes_used: normalize("bytes_used"),
    bytes_capacity: normalize("bytes_capacity"),
    pin_queue_depth: normalize("pin_queue_depth"),
    fetch_inflight: normalize("fetch_inflight"),
    fetch_bytes_per_sec: normalize("fetch_bytes_per_sec"),
    por_inflight: normalize("por_inflight"),
    por_samples_success_total: normalize("por_samples_success_total"),
    por_samples_failed_total: normalize("por_samples_failed_total", true),
    fetch_utilisation_bps: normalize("fetch_utilisation_bps"),
    pin_queue_utilisation_bps: normalize("pin_queue_utilisation_bps"),
    por_utilisation_bps: normalize("por_utilisation_bps"),
  };
}

function normalizeSorafsManifestResponse(
  payload,
  context = "sorafs manifest response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    manifest_id_hex: normalizeHex32String(
      record.manifest_id_hex ?? "",
      `${context}.manifest_id_hex`,
    ),
    manifest_b64: normalizeRequiredBase64Payload(
      record.manifest_b64,
      `${context}.manifest_b64`,
    ),
    manifest_digest_hex: normalizeHex32String(
      record.manifest_digest_hex ?? "",
      `${context}.manifest_digest_hex`,
    ),
    payload_digest_hex: normalizeHex32String(
      record.payload_digest_hex ?? "",
      `${context}.payload_digest_hex`,
    ),
    content_length: ToriiClient._normalizeUnsignedInteger(
      record.content_length,
      `${context}.content_length`,
      { allowZero: true },
    ),
    chunk_count: ToriiClient._normalizeUnsignedInteger(
      record.chunk_count,
      `${context}.chunk_count`,
      { allowZero: true },
    ),
    chunk_profile_handle: requireNonEmptyString(
      record.chunk_profile_handle ?? "",
      `${context}.chunk_profile_handle`,
    ),
    stored_at_unix_secs: ToriiClient._normalizeUnsignedInteger(
      record.stored_at_unix_secs,
      `${context}.stored_at_unix_secs`,
      { allowZero: true },
    ),
  };
}

function buildSorafsPinRegisterPayload(record, context) {
  const credentials = normalizeAuthorityCredentials(record, context);
  const chunkerInput = record.chunker;
  if (!chunkerInput || typeof chunkerInput !== "object") {
    throw new TypeError(`${context}.chunker is required`);
  }
  const chunker = ensureRecord(chunkerInput, `${context}.chunker`);
  const chunkerProfileId = ToriiClient._normalizeUnsignedInteger(
    chunker.profileId ?? chunker.profile_id,
    `${context}.chunker.profileId`,
    { allowZero: false },
  );
  const chunkerNamespace = requireNonEmptyString(
    chunker.namespace ?? chunker.ns ?? chunker.profile_namespace,
    `${context}.chunker.namespace`,
  );
  const chunkerName = requireNonEmptyString(
    chunker.name ?? chunker.handle ?? chunker.profile ?? chunker.id,
    `${context}.chunker.name`,
  );
  const chunkerSemver = requireNonEmptyString(
    chunker.semver ?? chunker.version ?? chunker.rev,
    `${context}.chunker.semver`,
  );
  const multihashCode = ToriiClient._normalizeUnsignedInteger(
    chunker.multihashCode ?? chunker.multihash_code ?? chunker.multihash ?? 0,
    `${context}.chunker.multihashCode`,
    { allowZero: true },
  );
  const pinPolicySource = record.pinPolicy;
  if (!pinPolicySource || typeof pinPolicySource !== "object") {
    throw new TypeError(`${context}.pinPolicy is required`);
  }
  const pinPolicy = normalizeSorafsPinPolicyRequest(pinPolicySource, `${context}.pinPolicy`);
  const manifestDigestHex = normalizeHex32String(
    record.manifestDigestHex,
    `${context}.manifestDigestHex`,
  );
  const chunkDigestHex = normalizeHex32String(
    record.chunkDigestSha3_256Hex ??
      record.chunk_digest_sha3_256_hex ??
      record.chunkDigest ??
      record.chunk_digest,
    `${context}.chunkDigestSha3_256Hex`,
  );
  const submittedEpoch = ToriiClient._normalizeUnsignedInteger(
    record.submittedEpoch,
    `${context}.submittedEpoch`,
    { allowZero: true },
  );
  const payload = {
    authority: credentials.authority,
    private_key: credentials.private_key,
    chunker_profile_id: chunkerProfileId,
    chunker_namespace: chunkerNamespace,
    chunker_name: chunkerName,
    chunker_semver: chunkerSemver,
    chunker_multihash_code: multihashCode,
    pin_policy: pinPolicy,
    manifest_digest_hex: manifestDigestHex,
    chunk_digest_sha3_256_hex: chunkDigestHex,
    submitted_epoch: submittedEpoch,
  };
  const aliasInput =
    record.alias ??
    (record.aliasNamespace || record.aliasName || record.aliasProof
      ? {
          namespace: record.aliasNamespace,
          name: record.aliasName,
          proof:
            record.aliasProof ??
            record.alias_proof ??
            record.aliasProofB64 ??
            record.alias_proof_b64,
        }
      : null);
  if (aliasInput) {
    payload.alias = normalizeSorafsPinAliasRequest(aliasInput, `${context}.alias`);
  }
  const successorHex =
    record.successorOfHex ?? null;
  if (successorHex !== undefined && successorHex !== null) {
    payload.successor_of_hex = normalizeHex32String(
      successorHex,
      `${context}.successorOfHex`,
    );
  }
  return payload;
}

function normalizeSorafsPinPolicyRequest(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const minReplicas = ToriiClient._normalizeUnsignedInteger(
    record.minReplicas,
    `${context}.minReplicas`,
  );
  const retentionEpoch = ToriiClient._normalizeUnsignedInteger(
    record.retentionEpoch ?? 0,
    `${context}.retentionEpoch`,
    { allowZero: true },
  );
  const storageClass = normalizeSorafsStorageClass(
    record.storageClass,
    `${context}.storageClass`,
  );
  return {
    min_replicas: minReplicas,
    storage_class: { type: storageClass },
    retention_epoch: retentionEpoch,
  };
}

function normalizeSorafsStorageClass(value, context) {
  if (!value) {
    throw new TypeError(`${context} is required`);
  }
  let label = value;
  if (typeof value === "object") {
    label = value.type ?? value.name ?? value.label ?? null;
  }
  const normalized = requireNonEmptyString(label, context).toLowerCase();
  switch (normalized) {
    case "hot":
      return "Hot";
    case "warm":
      return "Warm";
    case "cold":
      return "Cold";
    default:
      throw new TypeError(`${context} must be Hot, Warm, or Cold`);
  }
}

function normalizeSorafsPinAliasRequest(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const namespace = requireNonEmptyString(record.namespace, `${context}.namespace`);
  const name = requireNonEmptyString(record.name, `${context}.name`);
  const proofValue =
    record.proof ??
    record.proof_b64 ??
    record.proofB64 ??
    record.proof_base64 ??
    record.proofBase64;
  if (proofValue === undefined || proofValue === null) {
    throw new TypeError(`${context}.proof is required`);
  }
  return {
    namespace,
    name,
    proof_base64: normalizeRequiredBase64Payload(proofValue, `${context}.proof`),
  };
}

function normalizeDaSamplingPlan(payload, context = "da manifest response.sampling_plan") {
  if (payload === null || payload === undefined) {
    return null;
  }
  const record = ensureRecord(payload, context);
  const assignmentHash = normalizeHex32String(
    record.assignment_hash ?? "",
    `${context}.assignment_hash`,
  );
  const sampleWindow = ToriiClient._normalizeUnsignedInteger(
    record.sample_window,
    `${context}.sample_window`,
    { allowZero: true },
  );
  const samples = [];
  const sampleArray = record.samples ?? [];
  if (!Array.isArray(sampleArray)) {
    throw new TypeError(`${context}.samples must be an array`);
  }
  for (let idx = 0; idx < sampleArray.length; idx += 1) {
    const entry = ensureRecord(sampleArray[idx], `${context}.samples[${idx}]`);
    samples.push({
      index: ToriiClient._normalizeUnsignedInteger(
        entry.index,
        `${context}.samples[${idx}].index`,
        { allowZero: true },
      ),
      role: requireNonEmptyString(
        entry.role ?? "",
        `${context}.samples[${idx}].role`,
      ),
      group: ToriiClient._normalizeUnsignedInteger(
        entry.group ?? 0,
        `${context}.samples[${idx}].group`,
        { allowZero: true },
      ),
    });
  }
  return {
    assignment_hash_hex: assignmentHash,
    sample_window: sampleWindow,
    samples,
  };
}

function normalizeDaManifestFetchResponse(payload, context = "da manifest response") {
  const record = ensureRecord(payload ?? {}, context);
  const storageTicketHex = normalizeHex32String(
    record.storage_ticket ?? "",
    `${context}.storage_ticket`,
  );
  const manifestHashHex = normalizeHex32String(
    record.manifest_hash ?? "",
    `${context}.manifest_hash`,
  );
  const manifestB64 = normalizeRequiredBase64Payload(
    record.manifest_norito,
    `${context}.manifest_norito`,
  );
  return {
    storage_ticket_hex: storageTicketHex,
    client_blob_id_hex: normalizeHex32String(
      record.client_blob_id ?? "",
      `${context}.client_blob_id`,
    ),
    manifest_hash_hex: manifestHashHex,
    blob_hash_hex: normalizeHex32String(
      record.blob_hash ?? "",
      `${context}.blob_hash`,
    ),
    chunk_root_hex: normalizeHex32String(
      record.chunk_root ?? "",
      `${context}.chunk_root`,
    ),
    lane_id: ToriiClient._normalizeUnsignedInteger(
      record.lane_id,
      `${context}.lane_id`,
      { allowZero: true },
    ),
    epoch: ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      `${context}.epoch`,
      { allowZero: true },
    ),
    manifest_len: ToriiClient._normalizeUnsignedInteger(
      record.manifest_len ?? 0,
      `${context}.manifest_len`,
      { allowZero: true },
    ),
    manifest_b64: manifestB64,
    manifest_bytes: Buffer.from(manifestB64, "base64"),
    manifest_json:
      record.manifest ?? null,
    chunk_plan: record.chunk_plan ?? null,
    sampling_plan: normalizeDaSamplingPlan(
      record.sampling_plan ?? null,
      `${context}.sampling_plan`,
    ),
  };
}

async function persistDaManifestBundle(manifestBundle, outputDir, labelInput) {
  if (!manifestBundle) {
    throw new Error("manifestBundle is required");
  }
  if (
    !manifestBundle.manifest_bytes ||
    !Buffer.isBuffer(manifestBundle.manifest_bytes)
  ) {
    throw new Error("manifestBundle.manifest_bytes must be a Buffer");
  }
  const pathModule = await getPathModule();
  const fs = await getFsModule();
  const label = sanitizeTicketLabel(
    labelInput ??
      manifestBundle.manifest_hash_hex ??
      manifestBundle.storage_ticket_hex,
    "manifest ticket label",
  );
  const manifestPath = pathModule.join(outputDir, `manifest_${label}.norito`);
  const manifestJsonPath = pathModule.join(outputDir, `manifest_${label}.json`);
  const chunkPlanPath = pathModule.join(outputDir, `chunk_plan_${label}.json`);
  const samplingPlanPath = manifestBundle.sampling_plan
    ? pathModule.join(outputDir, `sampling_plan_${label}.json`)
    : null;

  await fs.mkdir(outputDir, { recursive: true });
  await writeBufferFile(manifestPath, manifestBundle.manifest_bytes);
  await writeJsonFile(manifestJsonPath, manifestBundle.manifest_json ?? {});
  const chunkPlan = normalizeChunkPlanForPersistence(manifestBundle.chunk_plan);
  await writeJsonFile(chunkPlanPath, chunkPlan);
  if (samplingPlanPath) {
    await writeJsonFile(samplingPlanPath, manifestBundle.sampling_plan);
  }

  return {
    manifestPath,
    manifestJsonPath,
    chunkPlanPath,
    samplingPlanPath,
    label,
  };
}

async function persistDaRequestArtifacts(artifactDir, request) {
  const pathModule = await getPathModule();
  const requestJsonPath = pathModule.join(artifactDir, "da_request.json");
  await writeJsonFile(requestJsonPath, request);
  return {
    requestJsonPath,
    receiptJsonPath: null,
    responseHeadersPath: null,
  };
}

async function persistDaReceiptArtifacts(
  artifactDir,
  response,
  pdpHeader,
  existing,
) {
  const pathModule = await getPathModule();
  const next = existing ?? {
    requestJsonPath: null,
    receiptJsonPath: null,
    responseHeadersPath: null,
  };
  if (response.receipt) {
    const receiptPath = pathModule.join(artifactDir, "da_receipt.json");
    await writeJsonFile(receiptPath, response.receipt);
    next.receiptJsonPath = receiptPath;
  }
  if (pdpHeader) {
    const headersPath = pathModule.join(artifactDir, "da_response_headers.json");
    await writeJsonFile(headersPath, { [HEADER_SORA_PDP_COMMITMENT]: pdpHeader });
    next.responseHeadersPath = headersPath;
  }
  return next;
}

async function resolveDaOutputDir(inputDir, prefix) {
  const pathModule = await getPathModule();
  if (inputDir === undefined || inputDir === null) {
    return pathModule.resolve(`${prefix}${Date.now()}`);
  }
  const trimmed = String(inputDir).trim();
  if (!trimmed) {
    return pathModule.resolve(`${prefix}${Date.now()}`);
  }
  return pathModule.resolve(trimmed);
}

async function getPathModule() {
  if (!getPathModule.cache) {
    getPathModule.cache = await import("node:path");
  }
  return getPathModule.cache;
}
getPathModule.cache = null;

async function getFsModule() {
  if (!getFsModule.cache) {
    getFsModule.cache = await import("node:fs/promises");
  }
  return getFsModule.cache;
}
getFsModule.cache = null;

async function writeJsonFile(filePath, value) {
  const fs = await getFsModule();
  const pathModule = await getPathModule();
  const dir = pathModule.dirname(filePath);
  if (dir && dir !== ".") {
    await fs.mkdir(dir, { recursive: true });
  }
  const body = `${JSON.stringify(value, null, 2)}\n`;
  await fs.writeFile(filePath, body, "utf8");
}

async function writeBufferFile(filePath, buffer) {
  const fs = await getFsModule();
  const pathModule = await getPathModule();
  const dir = pathModule.dirname(filePath);
  if (dir && dir !== ".") {
    await fs.mkdir(dir, { recursive: true });
  }
  await fs.writeFile(filePath, buffer);
}

function normalizeChunkPlanForPersistence(plan) {
  if (plan === undefined || plan === null) {
    return {};
  }
  if (typeof plan === "string") {
    try {
      return JSON.parse(plan);
    } catch {
      return { chunk_plan: plan };
    }
  }
  if (Array.isArray(plan) || typeof plan === "object") {
    return plan;
  }
  return {};
}

function sanitizeTicketLabel(value, context = "ticket label") {
  const normalized = String(value ?? "")
    .trim()
    .replace(/^0x/i, "")
    .toLowerCase();
  if (!normalized) {
    throw new TypeError(`${context} must not be empty`);
  }
  if (!/^[a-z0-9_-]+$/.test(normalized)) {
    throw new TypeError(
      `${context} may only contain ASCII letters, numbers, underscores, or dashes`,
    );
  }
  return normalized;
}

function normalizeOptionalPathInput(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const trimmed = String(value).trim();
  if (!trimmed) {
    throw new TypeError(`${context} must not be an empty string`);
  }
  return trimmed;
}

async function makeDefaultScoreboardPath(outputDir) {
  const pathModule = await getPathModule();
  return pathModule.join(outputDir, "scoreboard.json");
}

function normaliseChunkPlanPayload(plan, context) {
  if (plan === undefined || plan === null) {
    throw new TypeError(`${context} must be provided`);
  }
  if (typeof plan === "string") {
    const trimmed = plan.trim();
    if (!trimmed) {
      throw new TypeError(`${context} must not be an empty string`);
    }
    let parsed;
    try {
      parsed = JSON.parse(trimmed);
    } catch (error) {
      throw new TypeError(`${context} string must contain valid JSON`);
    }
    return { planJson: trimmed, planObject: parsed };
  }
  if (Array.isArray(plan) || isPlainObject(plan)) {
    let rendered;
    try {
      rendered = JSON.stringify(plan);
    } catch (error) {
      throw new TypeError(`${context} could not be serialised to JSON: ${error?.message ?? error}`);
    }
    return { planJson: rendered, planObject: plan };
  }
  throw new TypeError(`${context} must be a JSON string, array, or object`);
}

function normaliseChunkerHandle(explicitHandle, manifestBundle, context) {
  if (explicitHandle !== undefined && explicitHandle !== null) {
    const handle = requireNonEmptyString(explicitHandle, "fetchDaPayloadViaGateway.chunkerHandle").trim();
    if (!handle) {
      throw new TypeError("fetchDaPayloadViaGateway.chunkerHandle must not be empty");
    }
    return handle;
  }
  const bundleHandle =
    manifestBundle && typeof manifestBundle === "object"
      ? manifestBundle.chunker_handle ??
        manifestBundle.chunkerHandle ??
        manifestBundle.chunk_profile_handle ??
        manifestBundle.chunkProfileHandle ??
        null
      : null;
  if (bundleHandle && typeof bundleHandle === "string" && bundleHandle.trim()) {
    return bundleHandle.trim();
  }
  const manifestJson =
    manifestBundle && typeof manifestBundle === "object"
      ? manifestBundle.manifest_json ?? manifestBundle.manifest ?? null
      : null;
  const inferred = inferChunkerHandleFromManifest(manifestJson);
  if (inferred) {
    return inferred;
  }
  const manifestBytes = extractManifestBytesForProof(
    manifestBundle,
    `${context}.chunkerHandle`,
  );
  if (manifestBytes) {
    try {
      return deriveDaChunkerHandle(manifestBytes);
    } catch {
      if (bundleHandle && typeof bundleHandle === "string" && bundleHandle.trim()) {
        return bundleHandle.trim();
      }
      // Fall back to the default Sorafs chunker identifier for mocked manifests.
      return "sorafs.sf1@1.0.0";
    }
  }
  throw new TypeError(
    `${context}.chunkerHandle is required when the manifest bundle does not expose chunker metadata or manifest bytes`,
  );
}

function normalizeProofSummaryOption(value, context) {
  if (value === undefined || value === null || value === false) {
    return null;
  }
  if (value === true) {
    return {};
  }
  const record = ensureRecord(value, context);
  const sampleCount =
    record.sampleCount ?? undefined;
  const sampleSeed =
    record.sampleSeed ?? undefined;
  const leafIndexes =
    record.leafIndexes ?? undefined;
  const normalized = {};
  if (sampleCount !== undefined) {
    normalized.sampleCount = sampleCount;
  }
  if (sampleSeed !== undefined) {
    normalized.sampleSeed = sampleSeed;
  }
  if (leafIndexes !== undefined) {
    if (!Array.isArray(leafIndexes)) {
      throw new TypeError(`${context}.leafIndexes must be an array`);
    }
    normalized.leafIndexes = leafIndexes.slice();
  }
  return normalized;
}

function extractManifestBytesForProof(bundle, context) {
  if (!bundle || typeof bundle !== "object") {
    throw new TypeError(`${context} requires a manifest bundle object`);
  }
  const direct =
    tryBufferFrom(bundle.manifest_bytes) ?? tryBufferFrom(bundle.manifestBytes);
  if (direct) {
    return direct;
  }
  let manifestB64 = null;
  let manifestField = null;
  if (typeof bundle.manifest_b64 === "string" && bundle.manifest_b64.trim()) {
    manifestB64 = bundle.manifest_b64;
    manifestField = "manifest_b64";
  } else if (typeof bundle.manifestB64 === "string" && bundle.manifestB64.trim()) {
    manifestB64 = bundle.manifestB64;
    manifestField = "manifestB64";
  }
  if (manifestB64) {
    const normalized = normalizeRequiredBase64Payload(
      manifestB64,
      `${context}.${manifestField}`,
    );
    return Buffer.from(normalized, "base64");
  }
  return null;
}

function tryBufferFrom(value) {
  if (!value) {
    return null;
  }
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView && ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (typeof ArrayBuffer !== "undefined" && value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  return null;
}

function inferChunkerHandleFromManifest(manifestJson) {
  if (!isPlainObject(manifestJson)) {
    return null;
  }
  if (
    typeof manifestJson.chunker_handle === "string" &&
    manifestJson.chunker_handle.trim()
  ) {
    return manifestJson.chunker_handle.trim();
  }
  if (
    typeof manifestJson.chunk_profile_handle === "string" &&
    manifestJson.chunk_profile_handle.trim()
  ) {
    return manifestJson.chunk_profile_handle.trim();
  }
  const chunking =
    manifestJson.chunking ??
    manifestJson.chunk_profile ??
    manifestJson.chunkProfile ??
    null;
  if (isPlainObject(chunking)) {
    const namespace =
      chunking.namespace ??
      chunking.ns ??
      chunking.profile_namespace ??
      chunking.profileNamespace;
    const name =
      chunking.name ??
      chunking.profile ??
      chunking.handle ??
      chunking.chunker ??
      chunking.id;
    const version =
      chunking.semver ??
      chunking.version ??
      chunking.profile_version ??
      chunking.rev ??
      chunking.release;
    if (
      typeof namespace === "string" &&
      namespace.trim() &&
      typeof name === "string" &&
      name.trim() &&
      typeof version === "string" &&
      version.trim()
    ) {
      return `${namespace.trim()}.${name.trim()}@${version.trim()}`;
    }
  }
  return null;
}

function normalizeDaGatewayProviders(value, context) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty array`);
  }
  return value.map((entry, index) => {
    const record = ensureRecord(entry, `${context}[${index}]`);
    const name = requireNonEmptyString(
      record.name,
      `${context}[${index}].name`,
    ).trim();
    const providerIdHex = normalizeHex32String(
      record.providerIdHex ??
        record.provider_id_hex ??
        record.providerId ??
        record.provider_id,
      `${context}[${index}].providerIdHex`,
    );
    const baseUrl = requireNonEmptyString(
      record.baseUrl,
      `${context}[${index}].baseUrl`,
    );
    const streamTokenB64 = normalizeBase64Token(
      record.streamTokenB64 ??
        record.stream_token_b64 ??
        record.streamToken ??
        record.stream_token,
      `${context}[${index}].streamTokenB64`,
    );
    const privacyEventsUrl =
      record.privacyEventsUrl ?? null;
    return {
      name,
      providerIdHex,
      baseUrl,
      streamTokenB64,
      privacyEventsUrl: privacyEventsUrl ?? null,
    };
  });
}

function normalizeDaIngestResponse(payload, context = "da ingest response") {
  const record = ensureRecord(payload ?? {}, context);
  const status = requireNonEmptyString(record.status, `${context}.status`);
  const duplicate = requireBooleanLike(record.duplicate, `${context}.duplicate`);
  const receipt = record.receipt
    ? normalizeDaIngestReceipt(record.receipt, `${context}.receipt`)
    : null;
  return { status, duplicate, receipt };
}

function normalizeDaIngestReceipt(payload, context = "da ingest receipt") {
  const record = ensureRecord(payload ?? {}, context);
  const clientBlobId = decodeDaDigestTuple(
    record.client_blob_id,
    32,
    `${context}.client_blob_id`,
  );
  const blobHash = decodeDaDigestTuple(
    record.blob_hash,
    32,
    `${context}.blob_hash`,
  );
  const chunkRoot = decodeDaDigestTuple(
    record.chunk_root,
    32,
    `${context}.chunk_root`,
  );
  const manifestHash = decodeDaDigestTuple(
    record.manifest_hash,
    32,
    `${context}.manifest_hash`,
  );
  const storageTicket = decodeDaDigestTuple(
    record.storage_ticket,
    32,
    `${context}.storage_ticket`,
  );
  const stripeLayoutRecord = record.stripe_layout;
  const stripeLayout = (() => {
    if (stripeLayoutRecord === null || stripeLayoutRecord === undefined) {
      return { total_stripes: 0, shards_per_stripe: 0, row_parity_stripes: 0 };
    }
    const layout = ensureRecord(stripeLayoutRecord, `${context}.stripe_layout`);
    const totalStripes = ToriiClient._normalizeUnsignedInteger(
      layout.total_stripes ?? 0,
      `${context}.stripe_layout.total_stripes`,
      { allowZero: true },
    );
    const shardsPerStripe = ToriiClient._normalizeUnsignedInteger(
      layout.shards_per_stripe ?? 0,
      `${context}.stripe_layout.shards_per_stripe`,
      { allowZero: true },
    );
    const rowParityStripes = ToriiClient._normalizeUnsignedInteger(
      layout.row_parity_stripes ?? 0,
      `${context}.stripe_layout.row_parity_stripes`,
      { allowZero: true },
    );
    return {
      total_stripes: totalStripes,
      shards_per_stripe: shardsPerStripe,
      row_parity_stripes: rowParityStripes,
    };
  })();
  const pdpCommitment = record.pdp_commitment ?? null;
  let pdpCommitmentBytes = null;
  let pdpCommitmentB64 = null;
  if (pdpCommitment !== null && pdpCommitment !== undefined) {
    if (typeof pdpCommitment !== "string") {
      throw new TypeError(`${context}.pdp_commitment must be a base64 string when present`);
    }
    const trimmed = pdpCommitment.trim();
    try {
      const decoded = strictDecodeBase64(trimmed);
      pdpCommitmentBytes = Buffer.from(decoded);
      pdpCommitmentB64 = Buffer.from(decoded).toString("base64");
    } catch {
      throw new TypeError(`${context}.pdp_commitment must be a valid base64 string`);
    }
  }
  return {
    client_blob_id_hex: bufferToUpperHex(clientBlobId),
    client_blob_id_bytes: clientBlobId,
    lane_id: ToriiClient._normalizeUnsignedInteger(
      record.lane_id,
      `${context}.lane_id`,
      { allowZero: true },
    ),
    epoch: ToriiClient._normalizeUnsignedInteger(
      record.epoch,
      `${context}.epoch`,
      { allowZero: true },
    ),
    blob_hash_hex: bufferToUpperHex(blobHash),
    blob_hash_bytes: blobHash,
    chunk_root_hex: bufferToUpperHex(chunkRoot),
    chunk_root_bytes: chunkRoot,
    manifest_hash_hex: bufferToUpperHex(manifestHash),
    manifest_hash_bytes: manifestHash,
    storage_ticket_hex: bufferToUpperHex(storageTicket),
    storage_ticket_bytes: storageTicket,
    stripe_layout: stripeLayout,
    pdp_commitment_b64: pdpCommitmentB64 ?? (pdpCommitment ?? null),
    pdp_commitment_bytes: pdpCommitmentBytes,
    queued_at_unix: ToriiClient._normalizeUnsignedInteger(
      record.queued_at_unix,
      `${context}.queued_at_unix`,
      { allowZero: true },
    ),
    operator_signature_hex: normalizeUpperHex(
      record.operator_signature ?? "",
      `${context}.operator_signature`,
    ),
    rent_quote: normalizeDaRentQuote(
      record.rent_quote ?? null,
      `${context}.rent_quote`,
    ),
  };
}

function normalizeDaRentQuote(payload, context = "da rent quote") {
  if (payload === null || payload === undefined) {
    return null;
  }
  const record = ensureRecord(payload, context);
  return {
    base_rent_micro: normalizeMicroAmount(
      record.base_rent,
      `${context}.base_rent`,
    ),
    protocol_reserve_micro: normalizeMicroAmount(
      record.protocol_reserve,
      `${context}.protocol_reserve`,
    ),
    provider_reward_micro: normalizeMicroAmount(
      record.provider_reward,
      `${context}.provider_reward`,
    ),
    pdp_bonus_micro: normalizeMicroAmount(
      record.pdp_bonus,
      `${context}.pdp_bonus`,
    ),
    potr_bonus_micro: normalizeMicroAmount(
      record.potr_bonus,
      `${context}.potr_bonus`,
    ),
    egress_credit_per_gib_micro: normalizeMicroAmount(
      record.egress_credit_per_gib,
      `${context}.egress_credit_per_gib`,
    ),
  };
}

function normalizeMicroAmount(value, context) {
  if (value === null || value === undefined) {
    throw new TypeError(`${context} must be provided`);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^(0|[1-9]\d*)$/.test(trimmed)) {
      throw new TypeError(`${context} must be a non-negative integer string`);
    }
    return trimmed;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || value < 0 || !Number.isInteger(value)) {
      throw new TypeError(`${context} must be a non-negative integer number when encoded numerically`);
    }
    return Math.trunc(value).toString();
  }
  if (typeof value === "bigint") {
    if (value < 0) {
      throw new TypeError(`${context} must be non-negative`);
    }
    return value.toString();
  }
  throw new TypeError(`${context} must be encoded as a string, number, or bigint`);
}

function decodeDaDigestTuple(value, expectedLength, name) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new TypeError(`${name} must be an array`);
  }
  const bytes = Array.isArray(value[0]) ? value[0] : value;
  if (bytes.length !== expectedLength) {
    throw new TypeError(`${name} must contain ${expectedLength} entries`);
  }
  const buffer = Buffer.from(bytes);
  if (buffer.length !== expectedLength) {
    throw new TypeError(`${name} must decode to ${expectedLength} bytes`);
  }
  return buffer;
}

function bufferToUpperHex(buffer) {
  return Buffer.from(buffer).toString("hex").toUpperCase();
}

function normalizeUpperHex(value, name) {
  const trimmed = requireNonEmptyString(String(value ?? ""), name).replace(/^0x/i, "");
  if (trimmed.length === 0 || trimmed.length % 2 !== 0) {
    throw new TypeError(`${name} must be an even-length hex string`);
  }
  if (!/^([0-9A-Fa-f]{2})+$/.test(trimmed)) {
    throw new TypeError(`${name} must be a hex string`);
  }
  return trimmed.toUpperCase();
}

function normalizeSorafsUptimeObservationResponse(
  payload,
  context = "sorafs uptime response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    status: requireNonEmptyString(record.status, `${context}.status`),
    uptime_secs: ToriiClient._normalizeUnsignedInteger(
      record.uptime_secs,
      `${context}.uptime_secs`,
      { allowZero: true },
    ),
    observed_secs: ToriiClient._normalizeUnsignedInteger(
      record.observed_secs,
      `${context}.observed_secs`,
      { allowZero: true },
    ),
  };
}

function normalizeSorafsPorSubmissionResponse(
  payload,
  context = "sorafs por submission response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    status: requireNonEmptyString(record.status, `${context}.status`),
  };
}

function normalizeSorafsPorVerdictResponse(
  payload,
  context = "sorafs por verdict response",
) {
  return normalizeSorafsPorSubmissionResponse(payload, context);
}

function normalizeSorafsPorObservationResponse(
  payload,
  context = "sorafs por observation response",
) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    status: requireNonEmptyString(record.status, `${context}.status`),
    success: requireBooleanLike(record.success, `${context}.success`),
  };
}

function normalizeIsoWeekLabel(input, name) {
  const path = normalizeErrorPath(name);
  if (typeof input === "string") {
    const trimmed = input.trim();
    if (!trimmed) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a non-empty ISO week string`,
        path,
      );
    }
    if (!/^\d{4}-W\d{2}$/u.test(trimmed)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must match YYYY-Www format`,
        path,
      );
    }
    return trimmed;
  }
  if (input && typeof input === "object") {
    const year = ToriiClient._normalizeUnsignedInteger(input.year, `${name}.year`, {
      allowZero: false,
    });
    const week = ToriiClient._normalizeUnsignedInteger(input.week, `${name}.week`, {
      allowZero: false,
    });
    if (week < 1 || week > 53) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name}.week must be between 1 and 53`,
        normalizeErrorPath(`${name}.week`),
      );
    }
    const weekLabel = week.toString().padStart(2, "0");
    const yearLabel = year.toString().padStart(4, "0");
    return `${yearLabel}-W${weekLabel}`;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must be an ISO week string or {year, week} object`,
    path,
  );
}

function normalizePeerListResponse(payload, context = "peer list response") {
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) => {
    const record = ensureRecord(entry, `${context}[${index}]`);
    const address = requireNonEmptyString(
      record.address,
      `${context}[${index}].address`,
    );
    const idRecord = ensureRecord(
      record.id,
      `${context}[${index}].id`,
    );
    const publicKeyHex = normalizeHex32String(
      idRecord.public_key,
      `${context}[${index}].id.public_key`,
    );
    return {
      address,
      public_key_hex: publicKeyHex,
    };
  });
}

function normalizeTelemetryPeersInfoList(payload) {
  if (!Array.isArray(payload)) {
    throw new TypeError("telemetry peers response must be an array");
  }
  return payload.map((entry, index) =>
    normalizeTelemetryPeerInfo(entry, `telemetry peers response[${index}]`),
  );
}

function normalizeTelemetryPeerInfo(value, context) {
  const record = ensureRecord(value, context);
  const url = requireNonEmptyString(record.url, `${context}.url`);
  const connected = requireBooleanLike(record.connected, `${context}.connected`);
  const telemetryUnsupported = requireBooleanLike(
    record.telemetry_unsupported ?? false,
    `${context}.telemetry_unsupported`,
  );
  const configValue = record.config ?? null;
  const locationValue = record.location ?? null;
  const result = {
    url,
    connected,
    telemetryUnsupported,
  };
  if (configValue !== null) {
    result.config = normalizeTelemetryPeerConfig(configValue, `${context}.config`);
  }
  if (locationValue !== null) {
    result.location = normalizeTelemetryPeerLocation(locationValue, `${context}.location`);
  }
  if (record.connected_peers !== undefined && record.connected_peers !== null) {
    if (!Array.isArray(record.connected_peers)) {
      throw new TypeError(`${context}.connected_peers must be an array`);
    }
    result.connectedPeers = record.connected_peers.map((entry, idx) =>
      requireNonEmptyString(entry, `${context}.connected_peers[${idx}]`),
    );
  }
  return result;
}

function normalizeTelemetryPeerConfig(value, context) {
  const record = ensureRecord(value, context);
  const config = {
    publicKey: requireNonEmptyString(
      record.public_key ?? "",
      `${context}.public_key`,
    ),
  };
  const queueCapacity = record.queue_capacity;
  if (queueCapacity !== undefined && queueCapacity !== null) {
    config.queueCapacity = ToriiClient._normalizeUnsignedInteger(
      queueCapacity,
      `${context}.queue_capacity`,
      { allowZero: true },
    );
  }
  const blockSize = record.network_block_gossip_size;
  if (blockSize !== undefined && blockSize !== null) {
    config.networkBlockGossipSize = ToriiClient._normalizeUnsignedInteger(
      blockSize,
      `${context}.network_block_gossip_size`,
      { allowZero: true },
    );
  }
  const txSize = record.network_tx_gossip_size;
  if (txSize !== undefined && txSize !== null) {
    config.networkTxGossipSize = ToriiClient._normalizeUnsignedInteger(
      txSize,
      `${context}.network_tx_gossip_size`,
      { allowZero: true },
    );
  }
  const blockPeriod = normalizeExplorerDurationMs(
    record.network_block_gossip_period,
    `${context}.network_block_gossip_period`,
  );
  if (blockPeriod !== undefined) {
    config.networkBlockGossipPeriodMs = blockPeriod;
  }
  const txPeriod = normalizeExplorerDurationMs(
    record.network_tx_gossip_period,
    `${context}.network_tx_gossip_period`,
  );
  if (txPeriod !== undefined) {
    config.networkTxGossipPeriodMs = txPeriod;
  }
  return config;
}

function normalizeTelemetryPeerLocation(value, context) {
  const record = ensureRecord(value, context);
  return {
    lat: requireFiniteNumber(record.lat, `${context}.lat`),
    lon: requireFiniteNumber(record.lon, `${context}.lon`),
    country: requireNonEmptyString(record.country, `${context}.country`),
    city: requireNonEmptyString(record.city, `${context}.city`),
  };
}

function normalizeExplorerDurationMs(value, context) {
  if (value === undefined || value === null) {
    return undefined;
  }
  const record = ensureRecord(value, context);
  return ToriiClient._normalizeUnsignedInteger(record.ms, `${context}.ms`, {
    allowZero: true,
  });
}

function requireFiniteNumber(value, context) {
  if (typeof value !== "number" || Number.isNaN(value) || !Number.isFinite(value)) {
    throw new TypeError(`${context} must be a finite number`);
  }
  return value;
}

function normalizeKaigiRelaySummaryList(payload) {
  const record = ensureRecord(payload ?? {}, "kaigi relay summary response");
  const rawItems = Array.isArray(record.items) ? record.items : [];
  const items = rawItems.map((entry, index) =>
    normalizeKaigiRelaySummary(entry, `kaigi relay summary response.items[${index}]`),
  );
  return {
    total: ToriiClient._normalizeUnsignedInteger(
      record.total ?? items.length ?? 0,
      "kaigiRelay.total",
      { allowZero: true },
    ),
    items,
  };
}

function normalizeKaigiRelaySummary(payload, context) {
  const record = ensureRecord(payload, context);
  const relayId = requireNonEmptyString(record.relay_id, `${context}.relay_id`);
  const domain = requireNonEmptyString(record.domain, `${context}.domain`);
  const bandwidthClass = ToriiClient._normalizeUnsignedInteger(
    record.bandwidth_class ?? 0,
    `${context}.bandwidth_class`,
    { allowZero: true },
  );
  const hpkeFingerprint = normalizeHex32String(
    record.hpke_fingerprint_hex,
    `${context}.hpke_fingerprint_hex`,
  );
  let status = null;
  if (record.status !== undefined && record.status !== null) {
    const value = String(record.status).toLowerCase();
    if (!KAIGI_HEALTH_STATUS_VALUES.has(value)) {
      throw new TypeError(`${context}.status must be healthy, degraded, or unavailable`);
    }
    status = value;
  }
  let reportedAtMs = null;
  if (record.reported_at_ms !== undefined && record.reported_at_ms !== null) {
    reportedAtMs = ToriiClient._normalizeUnsignedInteger(
      record.reported_at_ms,
      `${context}.reported_at_ms`,
      { allowZero: true },
    );
  }
  return {
    relay_id: relayId,
    domain,
    bandwidth_class: bandwidthClass,
    hpke_fingerprint_hex: hpkeFingerprint,
    status,
    reported_at_ms: reportedAtMs,
  };
}

function normalizeKaigiRelayDetail(payload) {
  const record = ensureRecord(payload ?? {}, "kaigi relay detail");
  const relaySummary = normalizeKaigiRelaySummary(record.relay, "kaigi relay detail.relay");
  const hpkePublicKey = requireNonEmptyString(
    record.hpke_public_key_b64,
    "kaigi relay detail.hpke_public_key_b64",
  );
  let reportedCall = null;
  if (record.reported_call !== undefined && record.reported_call !== null) {
    const callRecord = ensureRecord(record.reported_call, "kaigi relay detail.reported_call");
    reportedCall = {
      domain_id: requireNonEmptyString(
        callRecord.domain_id,
        "kaigi relay detail.reported_call.domain_id",
      ),
      call_name: requireNonEmptyString(
        callRecord.call_name,
        "kaigi relay detail.reported_call.call_name",
      ),
    };
  }
  let reportedBy = null;
  if (record.reported_by !== undefined && record.reported_by !== null) {
    reportedBy = requireNonEmptyString(
      record.reported_by,
      "kaigi relay detail.reported_by",
    );
  }
  let notes = null;
  if (record.notes !== undefined && record.notes !== null) {
    notes = String(record.notes);
  }
  let metrics = null;
  if (record.metrics !== undefined && record.metrics !== null) {
    metrics = normalizeKaigiRelayDomainMetrics(record.metrics, "kaigi relay detail.metrics");
  }
  return {
    relay: relaySummary,
    hpke_public_key_b64: hpkePublicKey,
    reported_call: reportedCall,
    reported_by: reportedBy,
    notes,
    metrics,
  };
}

function normalizeKaigiRelayDomainMetrics(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    domain: requireNonEmptyString(
      record.domain,
      `${context}.domain`,
    ),
    registrations_total: ToriiClient._normalizeUnsignedInteger(
      record.registrations_total ?? 0,
      `${context}.registrations_total`,
      { allowZero: true },
    ),
    manifest_updates_total: ToriiClient._normalizeUnsignedInteger(
      record.manifest_updates_total ?? 0,
      `${context}.manifest_updates_total`,
      { allowZero: true },
    ),
    failovers_total: ToriiClient._normalizeUnsignedInteger(
      record.failovers_total ?? 0,
      `${context}.failovers_total`,
      { allowZero: true },
    ),
    health_reports_total: ToriiClient._normalizeUnsignedInteger(
      record.health_reports_total ?? 0,
      `${context}.health_reports_total`,
      { allowZero: true },
    ),
  };
}

function normalizeKaigiRelayHealthSnapshot(payload) {
  const record = ensureRecord(payload ?? {}, "kaigi relay health snapshot");
  const rawDomains = Array.isArray(record.domains) ? record.domains : [];
  const domains = rawDomains.map((entry, index) =>
    normalizeKaigiRelayDomainMetrics(entry, `kaigi relay health snapshot.domains[${index}]`),
  );
  return {
    healthy_total: ToriiClient._normalizeUnsignedInteger(
      record.healthy_total ?? 0,
      "kaigi relay health snapshot.healthy_total",
      { allowZero: true },
    ),
    degraded_total: ToriiClient._normalizeUnsignedInteger(
      record.degraded_total ?? 0,
      "kaigi relay health snapshot.degraded_total",
      { allowZero: true },
    ),
    unavailable_total: ToriiClient._normalizeUnsignedInteger(
      record.unavailable_total ?? 0,
      "kaigi relay health snapshot.unavailable_total",
      { allowZero: true },
    ),
    reports_total: ToriiClient._normalizeUnsignedInteger(
      record.reports_total ?? 0,
      "kaigi relay health snapshot.reports_total",
      { allowZero: true },
    ),
    registrations_total: ToriiClient._normalizeUnsignedInteger(
      record.registrations_total ?? 0,
      "kaigi relay health snapshot.registrations_total",
      { allowZero: true },
    ),
    failovers_total: ToriiClient._normalizeUnsignedInteger(
      record.failovers_total ?? 0,
      "kaigi relay health snapshot.failovers_total",
      { allowZero: true },
    ),
    domains,
  };
}

function normalizeKaigiCallEventRef(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    call_id: requireNonEmptyString(record.call_id, `${context}.call_id`),
    domain: requireNonEmptyString(record.domain, `${context}.domain`),
    call_name: requireNonEmptyString(record.call_name, `${context}.call_name`),
  };
}

function normalizeKaigiCallView(payload) {
  const record = ensureRecord(payload ?? {}, "kaigi call");
  const metadata = ensureRecord(record.metadata ?? {}, "kaigi call.metadata");
  const relayManifest =
    record.relay_manifest === null || record.relay_manifest === undefined
      ? undefined
      : ensureRecord(record.relay_manifest, "kaigi call.relay_manifest");
  return {
    call_id: requireNonEmptyString(record.call_id, "kaigi call.call_id"),
    domain: requireNonEmptyString(record.domain, "kaigi call.domain"),
    call_name: requireNonEmptyString(record.call_name, "kaigi call.call_name"),
    host_account_id:
      record.host_account_id === null || record.host_account_id === undefined
        ? undefined
        : requireNonEmptyString(record.host_account_id, "kaigi call.host_account_id"),
    billing_account_id:
      record.billing_account_id === null || record.billing_account_id === undefined
        ? undefined
        : requireNonEmptyString(
            record.billing_account_id,
            "kaigi call.billing_account_id",
          ),
    title:
      record.title === null || record.title === undefined
        ? undefined
        : String(record.title),
    description:
      record.description === null || record.description === undefined
        ? undefined
        : String(record.description),
    max_participants:
      record.max_participants === null || record.max_participants === undefined
        ? undefined
        : ToriiClient._normalizeUnsignedInteger(
            record.max_participants,
            "kaigi call.max_participants",
            { allowZero: true },
          ),
    gas_rate_per_minute: ToriiClient._normalizeUnsignedInteger(
      record.gas_rate_per_minute ?? 0,
      "kaigi call.gas_rate_per_minute",
      { allowZero: true },
    ),
    metadata,
    scheduled_start_ms:
      record.scheduled_start_ms === null || record.scheduled_start_ms === undefined
        ? undefined
        : ToriiClient._normalizeUnsignedInteger(
            record.scheduled_start_ms,
            "kaigi call.scheduled_start_ms",
            { allowZero: true },
          ),
    privacy_mode: requireNonEmptyString(
      record.privacy_mode,
      "kaigi call.privacy_mode",
    ).toLowerCase(),
    room_policy: requireNonEmptyString(
      record.room_policy,
      "kaigi call.room_policy",
    ).toLowerCase(),
    relay_manifest: relayManifest,
    roster_root_hex: normalizeHex32String(
      record.roster_root_hex,
      "kaigi call.roster_root_hex",
    ),
    participant_count: normalizeOptionalUnsignedInteger(
      record.participant_count,
      "kaigi call.participant_count",
    ),
    commitment_count: ToriiClient._normalizeUnsignedInteger(
      record.commitment_count ?? 0,
      "kaigi call.commitment_count",
      { allowZero: true },
    ),
    nullifier_count: ToriiClient._normalizeUnsignedInteger(
      record.nullifier_count ?? 0,
      "kaigi call.nullifier_count",
      { allowZero: true },
    ),
    usage_commitment_count: ToriiClient._normalizeUnsignedInteger(
      record.usage_commitment_count ?? 0,
      "kaigi call.usage_commitment_count",
      { allowZero: true },
    ),
    status: requireNonEmptyString(record.status, "kaigi call.status").toLowerCase(),
    created_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.created_at_ms ?? 0,
      "kaigi call.created_at_ms",
      { allowZero: true },
    ),
    ended_at_ms:
      record.ended_at_ms === null || record.ended_at_ms === undefined
        ? undefined
        : ToriiClient._normalizeUnsignedInteger(
            record.ended_at_ms,
            "kaigi call.ended_at_ms",
            { allowZero: true },
          ),
    total_duration_ms: ToriiClient._normalizeUnsignedInteger(
      record.total_duration_ms ?? 0,
      "kaigi call.total_duration_ms",
      { allowZero: true },
    ),
    total_billed_gas: ToriiClient._normalizeUnsignedInteger(
      record.total_billed_gas ?? 0,
      "kaigi call.total_billed_gas",
      { allowZero: true },
    ),
    segments_recorded: ToriiClient._normalizeUnsignedInteger(
      record.segments_recorded ?? 0,
      "kaigi call.segments_recorded",
      { allowZero: true },
    ),
  };
}

function normalizeKaigiCallSignal(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    entrypoint_hash: requireNonEmptyString(
      record.entrypoint_hash,
      `${context}.entrypoint_hash`,
    ),
    authority:
      record.authority === null || record.authority === undefined
        ? undefined
        : requireNonEmptyString(record.authority, `${context}.authority`),
    timestamp_ms:
      record.timestamp_ms === null || record.timestamp_ms === undefined
        ? undefined
        : ToriiClient._normalizeUnsignedInteger(
            record.timestamp_ms,
            `${context}.timestamp_ms`,
            { allowZero: true },
          ),
    call_id: requireNonEmptyString(record.call_id, `${context}.call_id`),
    signal_kind: requireNonEmptyString(
      record.signal_kind,
      `${context}.signal_kind`,
    ).toLowerCase(),
    host_account_id:
      record.host_account_id === null || record.host_account_id === undefined
        ? undefined
        : requireNonEmptyString(record.host_account_id, `${context}.host_account_id`),
    participant_account_id:
      record.participant_account_id === null || record.participant_account_id === undefined
        ? undefined
        : requireNonEmptyString(
            record.participant_account_id,
            `${context}.participant_account_id`,
          ),
    created_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.created_at_ms ?? 0,
      `${context}.created_at_ms`,
      { allowZero: true },
    ),
    metadata: ensureRecord(record.metadata ?? {}, `${context}.metadata`),
  };
}

function normalizeKaigiCallSignalsList(payload) {
  const record = ensureRecord(payload ?? {}, "kaigi call signals");
  const rawItems = Array.isArray(record.items) ? record.items : [];
  return {
    total: ToriiClient._normalizeUnsignedInteger(
      record.total ?? rawItems.length,
      "kaigi call signals.total",
      { allowZero: true },
    ),
    items: rawItems.map((entry, index) =>
      normalizeKaigiCallSignal(entry, `kaigi call signals.items[${index}]`),
    ),
  };
}

function normalizeKaigiCallEventData(payload) {
  const record = ensureRecord(payload, "kaigi call event");
  const kind = requireNonEmptyString(record.kind, "kaigi call event.kind").toLowerCase();
  if (!KAIGI_CALL_EVENT_KIND_VALUES.has(kind)) {
    throw new TypeError("kaigi call event.kind must be roster_updated or ended");
  }
  const call = normalizeKaigiCallEventRef(record.call, "kaigi call event.call");
  if (kind === "ended") {
    return {
      kind,
      call,
      status: requireNonEmptyString(record.status, "kaigi call event.status").toLowerCase(),
      ended_at_ms: ToriiClient._normalizeUnsignedInteger(
        record.ended_at_ms ?? 0,
        "kaigi call event.ended_at_ms",
        { allowZero: true },
      ),
    };
  }
  return {
    kind,
    call,
    privacy_mode: requireNonEmptyString(
      record.privacy_mode,
      "kaigi call event.privacy_mode",
    ).toLowerCase(),
    participant_count: normalizeOptionalUnsignedInteger(
      record.participant_count,
      "kaigi call event.participant_count",
    ),
    commitment_count: ToriiClient._normalizeUnsignedInteger(
      record.commitment_count ?? 0,
      "kaigi call event.commitment_count",
      { allowZero: true },
    ),
    nullifier_count: ToriiClient._normalizeUnsignedInteger(
      record.nullifier_count ?? 0,
      "kaigi call event.nullifier_count",
      { allowZero: true },
    ),
    roster_root_hex:
      record.roster_root_hex === null || record.roster_root_hex === undefined
        ? undefined
        : normalizeHex32String(
            record.roster_root_hex,
            "kaigi call event.roster_root_hex",
          ),
  };
}

function normalizeKaigiRelayEventData(payload) {
  const record = ensureRecord(payload, "kaigi relay event");
  const kind = requireNonEmptyString(record.kind, "kaigi relay event.kind").toLowerCase();
  const domain = requireNonEmptyString(
    record.domain,
    "kaigi relay event.domain",
  );
  const relayId = requireNonEmptyString(
    record.relay_id,
    "kaigi relay event.relay_id",
  );
  if (!KAIGI_EVENT_KIND_VALUES.has(kind)) {
    throw new TypeError("kaigi relay event.kind must be registration or health");
  }
  if (kind === "registration") {
    return {
      kind,
      domain,
      relay_id: relayId,
      bandwidth_class: ToriiClient._normalizeUnsignedInteger(
        record.bandwidth_class ?? 0,
        "kaigi relay event.bandwidth_class",
        { allowZero: true },
      ),
      hpke_fingerprint_hex: normalizeHex32String(
        record.hpke_fingerprint_hex,
        "kaigi relay event.hpke_fingerprint_hex",
      ),
    };
  }
  const statusValue = requireNonEmptyString(
    record.status,
    "kaigi relay event.status",
  ).toLowerCase();
  if (!KAIGI_HEALTH_STATUS_VALUES.has(statusValue)) {
    throw new TypeError("kaigi relay event.status must be healthy, degraded, or unavailable");
  }
  const callRecord = ensureRecord(record.call, "kaigi relay event.call");
  const callDomain = requireNonEmptyString(
    callRecord.domain,
    "kaigi relay event.call.domain",
  );
  const callName = requireNonEmptyString(
    callRecord.name,
    "kaigi relay event.call.name",
  );
  return {
    kind,
    domain,
    relay_id: relayId,
    status: statusValue,
    reported_at_ms: ToriiClient._normalizeUnsignedInteger(
      record.reported_at_ms ?? 0,
      "kaigi relay event.reported_at_ms",
      { allowZero: true },
    ),
    call: {
      domain: callDomain,
      name: callName,
    },
  };
}

function buildKaigiRelayEventParams(options = {}) {
  const normalizedOptions =
    options === undefined
      ? {}
      : requirePlainObjectOption(options, "kaigi relay event options", {
          message: "must be an object",
        });
  const params = {};
  if (normalizedOptions.domain !== undefined && normalizedOptions.domain !== null) {
    params.domain = requireNonEmptyString(
      normalizedOptions.domain,
      "kaigiRelayEvents.domain",
    ).toLowerCase();
  }
  if (normalizedOptions.relay !== undefined && normalizedOptions.relay !== null) {
    params.relay = requireNonEmptyString(normalizedOptions.relay, "kaigiRelayEvents.relay");
  }
  if (normalizedOptions.kind !== undefined && normalizedOptions.kind !== null) {
    const values = Array.isArray(normalizedOptions.kind)
      ? normalizedOptions.kind
      : String(normalizedOptions.kind)
          .split(",")
          .map((entry) => entry.trim())
          .filter((entry) => entry.length > 0);
    const normalized = [];
    for (const entry of values) {
      const token = requireNonEmptyString(entry, "kaigiRelayEvents.kind").toLowerCase();
      if (!KAIGI_EVENT_KIND_VALUES.has(token)) {
        throw new TypeError("kaigiRelayEvents.kind must be registration or health");
      }
      normalized.push(token);
    }
    if (normalized.length > 0) {
      params.kind = normalized.join(",");
    }
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildKaigiCallSignalsQuery(options = {}) {
  const normalizedOptions =
    options === undefined || options === null
      ? undefined
      : requirePlainObjectOption(options, "kaigi call signals options", {
          message: "must be an object",
        });
  if (normalizedOptions) {
    assertSupportedOptionKeys(
      normalizedOptions,
      new Set(["afterTimestampMs", "after_timestamp_ms", "limit", "offset", "signal"]),
      "kaigi call signals options",
    );
  }
  const { signal } = normalizeSignalOption(normalizedOptions, "listKaigiCallSignals");
  const source = normalizedOptions ?? {};
  const params = {};
  const afterTimestampMs =
    source.afterTimestampMs !== undefined
      ? source.afterTimestampMs
      : source.after_timestamp_ms;
  if (afterTimestampMs !== undefined && afterTimestampMs !== null) {
    params.after_timestamp_ms = ToriiClient._normalizeUnsignedInteger(
      afterTimestampMs,
      "kaigiCallSignals.afterTimestampMs",
      { allowZero: true },
    );
  }
  if (source.limit !== undefined && source.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      source.limit,
      "kaigiCallSignals.limit",
      { allowZero: false },
    );
  }
  if (source.offset !== undefined && source.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      source.offset,
      "kaigiCallSignals.offset",
      { allowZero: true },
    );
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function buildKaigiCallEventParams(options = {}) {
  const normalizedOptions =
    options === undefined
      ? {}
      : requirePlainObjectOption(options, "kaigi call event options", {
          message: "must be an object",
        });
  const params = {};
  if (normalizedOptions.kind !== undefined && normalizedOptions.kind !== null) {
    const values = Array.isArray(normalizedOptions.kind)
      ? normalizedOptions.kind
      : String(normalizedOptions.kind)
          .split(",")
          .map((entry) => entry.trim())
          .filter((entry) => entry.length > 0);
    const normalized = [];
    for (const entry of values) {
      const token = requireNonEmptyString(entry, "kaigiCallEvents.kind").toLowerCase();
      if (!KAIGI_CALL_EVENT_KIND_VALUES.has(token)) {
        throw new TypeError("kaigiCallEvents.kind must be roster_updated or ended");
      }
      normalized.push(token);
    }
    if (normalized.length > 0) {
      params.kind = normalized.join(",");
    }
  }
  return Object.keys(params).length === 0 ? undefined : params;
}

function buildTriggerListQuery(options = {}) {
  const normalizedOptions =
    options === undefined || options === null
      ? undefined
      : requirePlainObjectOption(options, "trigger list options", {
          message: "must be an object",
        });
  const { signal } = normalizeSignalOption(normalizedOptions ?? undefined, "triggers");
  const params = {};
  const source = normalizedOptions ?? {};
  if (source.namespace !== undefined && source.namespace !== null) {
    params.namespace = requireNonEmptyString(source.namespace, "triggers.namespace");
  }
  if (source.authority !== undefined && source.authority !== null) {
    params.authority = normalizeAccountId(source.authority, "triggers.authority");
  }
  if (source.limit !== undefined && source.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(source.limit, "triggers.limit", {
      allowZero: false,
    });
  }
  if (source.offset !== undefined && source.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(source.offset, "triggers.offset", {
      allowZero: true,
    });
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function buildSubscriptionPlanListQuery(options = {}) {
  const normalizedOptions =
    options === undefined || options === null
      ? undefined
      : ensureRecord(options, "subscription plan list options");
  if (normalizedOptions) {
    assertSupportedOptionKeys(
      normalizedOptions,
      SUBSCRIPTION_PLAN_LIST_OPTION_KEYS,
      "subscription plan list options",
    );
  }
  const { signal } = normalizeSignalOption(
    normalizedOptions ?? undefined,
    "subscription plans",
  );
  const params = {};
  const source = normalizedOptions ?? {};
  if (source.provider !== undefined && source.provider !== null) {
    params.provider = normalizeAccountId(
      source.provider,
      "subscriptionPlans.provider",
    );
  }
  if (source.limit !== undefined && source.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      source.limit,
      "subscriptionPlans.limit",
      { allowZero: false },
    );
  }
  if (source.offset !== undefined && source.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      source.offset,
      "subscriptionPlans.offset",
      { allowZero: true },
    );
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function normalizeSubscriptionStatusFilter(value, context) {
  const normalized = requireNonEmptyString(value, context).toLowerCase();
  if (!SUBSCRIPTION_STATUS_VALUES.has(normalized)) {
    throw new TypeError(
      `${context} must be one of ${Array.from(SUBSCRIPTION_STATUS_VALUES).join(", ")}`,
    );
  }
  return normalized;
}

function buildSubscriptionListQuery(options = {}) {
  const normalizedOptions =
    options === undefined || options === null
      ? undefined
      : ensureRecord(options, "subscription list options");
  if (normalizedOptions) {
    assertSupportedOptionKeys(
      normalizedOptions,
      SUBSCRIPTION_LIST_OPTION_KEYS,
      "subscription list options",
    );
  }
  const { signal } = normalizeSignalOption(
    normalizedOptions ?? undefined,
    "subscriptions",
  );
  const params = {};
  const source = normalizedOptions ?? {};
  const ownedBy = pickOverride(source, "owned_by", "ownedBy");
  if (ownedBy !== undefined && ownedBy !== null) {
    params.owned_by = normalizeAccountId(ownedBy, "subscriptions.ownedBy");
  }
  if (source.provider !== undefined && source.provider !== null) {
    params.provider = normalizeAccountId(source.provider, "subscriptions.provider");
  }
  if (source.status !== undefined && source.status !== null) {
    params.status = normalizeSubscriptionStatusFilter(
      source.status,
      "subscriptions.status",
    );
  }
  if (source.limit !== undefined && source.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      source.limit,
      "subscriptions.limit",
      { allowZero: false },
    );
  }
  if (source.offset !== undefined && source.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      source.offset,
      "subscriptions.offset",
      { allowZero: true },
    );
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function normalizeAccountListResponse(payload) {
  return normalizeIdListResponse(payload, "account list response");
}

function normalizeDomainListResponse(payload) {
  return normalizeIdListResponse(payload, "domain list response");
}

function normalizeAssetDefinitionListResponse(payload) {
  return normalizeIdListResponse(payload, "asset definition list response");
}

function normalizeNftListResponse(payload) {
  return normalizeIdListResponse(payload, "nft list response");
}

function normalizeRwaListResponse(payload) {
  return normalizeIdListResponse(payload, "rwa list response");
}

function normalizeSignalOption(options, context) {
  if (options === undefined) {
    return { signal: undefined };
  }
  if (!isPlainObject(options)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options must be an object`,
      `${context}.options`,
    );
  }
  const { signal } = options;
  if (signal !== undefined && signal !== null && !isAbortSignalLike(signal)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} options.signal must be an AbortSignal`,
      `${context}.options.signal`,
    );
  }
  return { signal: signal ?? undefined };
}

function normalizeSignalOnlyOption(options, context) {
  const { signal } = normalizeSignalOption(options, context);
  if (options !== undefined) {
    const extras = Object.keys(options).filter((key) => key !== "signal");
    if (extras.length > 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} options contains unsupported fields: ${extras.join(", ")}`,
        `${context}.options`,
      );
    }
  }
  return { signal };
}

function requirePlainObjectOption(value, context, { message } = {}) {
  if (!isPlainObject(value)) {
    const normalizedPath =
      typeof context === "string" ? context.replace(/\s+/g, ".") : context;
    const normalizedMessage = message ?? "must be a plain object";
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} ${normalizedMessage}`,
      normalizedPath,
    );
  }
  return value;
}

function assertSupportedOptionKeys(record, allowedKeys, context) {
  const extras = Object.keys(record).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    const path = typeof context === "string" ? context.replace(/\s+/g, ".") : context;
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} contains unsupported fields: ${extras.join(", ")}`,
      path,
    );
  }
}

function normalizeIterableListOptions(options, context, allowedKeys = ITERABLE_LIST_OPTION_KEYS) {
  const normalized = ToriiClient._normalizeIterableOptions(options, context, allowedKeys);
  assertSupportedOptionKeys(normalized, allowedKeys, context);
  return normalized;
}

function normalizeIterableQueryOptions(options, context, extraAllowedKeys = []) {
  const allowedKeys = new Set([
    ...ITERABLE_QUERY_OPTION_KEYS,
    ...extraAllowedKeys,
    "fetch_size",
    "query_name",
  ]);
  const normalized = ToriiClient._normalizeIterableOptions(options, context, allowedKeys);
  if (normalized.fetch_size !== undefined && normalized.fetchSize === undefined) {
    normalized.fetchSize = normalized.fetch_size;
  }
  if (normalized.query_name !== undefined && normalized.queryName === undefined) {
    normalized.queryName = normalized.query_name;
  }
  delete normalized.fetch_size;
  delete normalized.query_name;
  const resolvedAllowedKeys = new Set([...ITERABLE_QUERY_OPTION_KEYS, ...extraAllowedKeys]);
  assertSupportedOptionKeys(normalized, resolvedAllowedKeys, context);
  return normalized;
}

function normalizeEventStreamOptions(options, context, allowedExtraKeys = []) {
  const { signal } = normalizeSignalOption(options, context);
  const normalized = options ?? {};
  const allowedKeys = new Set(["signal", "lastEventId", ...allowedExtraKeys]);
  assertSupportedOptionKeys(normalized, allowedKeys, `${context} options`);
  let lastEventId;
  if (normalized.lastEventId !== undefined) {
    if (normalized.lastEventId === null) {
      throw new TypeError(`${context}.lastEventId must be a non-empty string`);
    }
    lastEventId = requireNonEmptyString(
      normalized.lastEventId,
      `${context}.lastEventId`,
    ).trim();
  }
  return { signal, lastEventId };
}

function isAbortSignalLike(value) {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof value.aborted === "boolean" &&
    typeof value.addEventListener === "function" &&
    typeof value.removeEventListener === "function"
  );
}

function normalizeAccountAssetListResponse(payload) {
  return normalizeIterableItems(
    payload,
    "account asset list response",
    normalizeAccountAssetListItem,
  );
}

function normalizeAccountTransactionListResponse(payload) {
  return normalizeIterableItems(
    payload,
    "account transaction list response",
    normalizeAccountTransactionListItem,
  );
}

function normalizeAssetHolderListResponse(payload) {
  return normalizeIterableItems(
    payload,
    "asset holder list response",
    normalizeAssetHolderListItem,
  );
}

function normalizeAccountPermissionListResponse(payload) {
  return normalizeIterableItems(
    payload,
    "account permission list response",
    normalizeAccountPermissionItem,
  );
}

function normalizeRepoAgreementListResponse(payload) {
  return normalizeIterableItems(
    payload,
    "repo agreement list response",
    normalizeRepoAgreement,
  );
}

function normalizeRepoAgreement(entry, context) {
  const record = ensureRecord(entry, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  const initiator = requireNonEmptyString(record.initiator, `${context}.initiator`);
  const counterparty = requireNonEmptyString(
    record.counterparty,
    `${context}.counterparty`,
  );
  const custodian =
    record.custodian === undefined || record.custodian === null
      ? null
      : requireNonEmptyString(record.custodian, `${context}.custodian`);
  const rateBps = Number(record.rate_bps ?? 0);
  const maturityTimestampMs = ToriiClient._normalizeUnsignedInteger(
    record.maturity_timestamp_ms ?? 0,
    `${context}.maturity_timestamp_ms`,
    { allowZero: true },
  );
  const initiatedTimestampMs = ToriiClient._normalizeUnsignedInteger(
    record.initiated_timestamp_ms ?? 0,
    `${context}.initiated_timestamp_ms`,
    { allowZero: true },
  );
  const lastMarginCheckTimestampMs = ToriiClient._normalizeUnsignedInteger(
    record.last_margin_check_timestamp_ms ?? 0,
    `${context}.last_margin_check_timestamp_ms`,
    { allowZero: true },
  );
  return {
    id,
    initiator,
    counterparty,
    custodian,
    cashLeg: normalizeRepoLeg(record.cash_leg, `${context}.cash_leg`),
    collateralLeg: normalizeRepoLeg(record.collateral_leg, `${context}.collateral_leg`),
    rateBps,
    maturityTimestampMs,
    initiatedTimestampMs,
    lastMarginCheckTimestampMs,
    governance: normalizeRepoGovernance(record.governance, `${context}.governance`),
  };
}

function normalizeRepoLeg(value, context) {
  const record = ensureRecord(value, context);
  return {
    assetDefinitionId: requireNonEmptyString(
      record.asset_definition_id,
      `${context}.asset_definition_id`,
    ),
    quantity: requireNonEmptyString(record.quantity, `${context}.quantity`),
    metadata: cloneJsonValue(record.metadata ?? {}, `${context}.metadata`),
  };
}

function normalizeRepoGovernance(value, context) {
  const record = ensureRecord(value, context);
  return {
    haircutBps: Number(record.haircut_bps ?? 0),
    marginFrequencySecs: ToriiClient._normalizeUnsignedInteger(
      record.margin_frequency_secs ?? 0,
      `${context}.margin_frequency_secs`,
      { allowZero: true },
    ),
  };
}

function normalizeAttachmentMetadataList(payload, context = "attachment list response") {
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  return payload.map((entry, index) =>
    normalizeAttachmentMetadataRecord(entry, `${context}[${index}]`),
  );
}

function normalizeAttachmentUploadPayload(value, context) {
  if (typeof value === "string") {
    return Buffer.from(value, "utf8");
  }
  if (
    Buffer.isBuffer(value) ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer ||
    Array.isArray(value)
  ) {
    return toBuffer(value);
  }
  throw new TypeError(`${context} must be a string or binary payload`);
}

function normalizeAttachmentMetadataRecord(value, context) {
  const record = ensureRecord(value, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  const contentType = requireNonEmptyString(
    record.content_type,
    `${context}.content_type`,
  );
  const size = ToriiClient._normalizeUnsignedInteger(
    record.size,
    `${context}.size`,
    { allowZero: true },
  );
  const createdMs = ToriiClient._normalizeUnsignedInteger(
    record.created_ms,
    `${context}.created_ms`,
    { allowZero: true },
  );
  const tenant = optionalString(record.tenant, `${context}.tenant`);
  return {
    id,
    contentType,
    size,
    createdMs,
    tenant,
  };
}

const VERIFYING_KEY_LIST_OPTION_KEYS = new Set([
  "signal",
  "backend",
  "backend_filter",
  "status",
  "statusFilter",
  "verifyingKeyStatus",
  "nameContains",
  "name_contains",
  "limit",
  "offset",
  "order",
  "sort",
  "sortOrder",
  "idsOnly",
  "ids_only",
]);
const VERIFYING_KEY_ITERATOR_OPTION_KEYS = new Set([
  ...VERIFYING_KEY_LIST_OPTION_KEYS,
  "pageSize",
  "maxItems",
]);

function buildVerifyingKeyListQuery(options = {}) {
  const { signal } = normalizeSignalOption(options, "listVerifyingKeys");
  assertSupportedOptionKeys(
    options ?? {},
    VERIFYING_KEY_LIST_OPTION_KEYS,
    "listVerifyingKeys options",
  );
  const params = {};
  const backendValue = options.backend ?? options.backend_filter;
  if (backendValue !== undefined && backendValue !== null) {
    params.backend = requireNonEmptyString(backendValue, "listVerifyingKeys.backend");
  }
  const statusValue =
    options.status ?? options.statusFilter ?? options.verifyingKeyStatus;
  const normalizedStatus = normalizeVerifyingKeyStatusValue(
    statusValue,
    "listVerifyingKeys.status",
    { optional: true },
  );
  if (normalizedStatus) {
    params.status = normalizedStatus;
  }
  const nameContains = options.nameContains ?? options.name_contains;
  if (nameContains !== undefined && nameContains !== null) {
    params.name_contains = requireNonEmptyString(
      nameContains,
      "listVerifyingKeys.nameContains",
    );
  }
  if (options.limit !== undefined && options.limit !== null) {
    params.limit = ToriiClient._normalizeUnsignedInteger(
      options.limit,
      "listVerifyingKeys.limit",
      { allowZero: false },
    );
  }
  if (options.offset !== undefined && options.offset !== null) {
    params.offset = ToriiClient._normalizeUnsignedInteger(
      options.offset,
      "listVerifyingKeys.offset",
      { allowZero: true },
    );
  }
  const orderValue = options.order ?? options.sort ?? options.sortOrder;
  if (orderValue !== undefined && orderValue !== null) {
    const normalizedOrder = requireNonEmptyString(orderValue, "listVerifyingKeys.order");
    const lower = normalizedOrder.toLowerCase();
    if (lower !== "asc" && lower !== "desc") {
      throw new TypeError('listVerifyingKeys.order must be "asc" or "desc"');
    }
    params.order = lower;
  }
  const idsOnlyValue = options.idsOnly ?? options.ids_only;
  if (idsOnlyValue !== undefined && idsOnlyValue !== null) {
    params.ids_only = requireBooleanLike(
      idsOnlyValue,
      "listVerifyingKeys.idsOnly",
    );
  }
  return { signal, params: Object.keys(params).length === 0 ? undefined : params };
}

function normalizeVerifyingKeyListPayload(
  payload,
  context = "verifying key list response",
) {
  if (payload === undefined || payload === null) {
    return [];
  }
  if (Array.isArray(payload)) {
    return payload.map((entry, index) =>
      normalizeVerifyingKeyListItem(entry, `${context}[${index}]`),
    );
  }
  const record = ensureRecord(payload, context);
  if (!Array.isArray(record.items)) {
    throw new TypeError(`${context} must be an array or { items: [] } object`);
  }
  return record.items.map((entry, index) =>
    normalizeVerifyingKeyListItem(entry, `${context}.items[${index}]`),
  );
}

function normalizeVerifyingKeyListItem(payload, context) {
  const record = ensureRecord(payload, context);
  let idPayload = record.id;
  if (!idPayload && record.backend && record.name) {
    idPayload = { backend: record.backend, name: record.name };
  }
  if (!idPayload) {
    throw new TypeError(`${context} must include an id`);
  }
  const id = normalizeVerifyingKeyId(idPayload, `${context}.id`);
  let normalizedRecord = null;
  if (record.record !== undefined && record.record !== null) {
    normalizedRecord = normalizeVerifyingKeyRecord(record.record, `${context}.record`);
  }
  return { id, record: normalizedRecord };
}

function normalizeVerifyingKeyDetail(
  payload,
  context = "verifying key detail response",
) {
  const record = ensureRecord(payload, context);
  const id = normalizeVerifyingKeyId(record.id, `${context}.id`);
  return {
    id,
    record: normalizeVerifyingKeyRecord(record.record, `${context}.record`),
  };
}

function normalizeVerifyingKeyId(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    name: requireNonEmptyString(record.name, `${context}.name`),
  };
}

function normalizeVerifyingKeyRecord(payload, context) {
  const record = ensureRecord(payload, context);
  const gasSchedule = record.gas_schedule_id ?? null;
  const metadataCid = record.metadata_uri_cid ?? null;
  const vkBytesCid = record.vk_bytes_cid ?? null;
  const inlinePayload =
    record.key ?? null;
  return {
    version: ToriiClient._normalizeUnsignedInteger(record.version, `${context}.version`, {
      allowZero: false,
    }),
    circuit_id: requireNonEmptyString(
      record.circuit_id,
      `${context}.circuit_id`,
    ),
    backend: requireNonEmptyString(record.backend, `${context}.backend`),
    curve:
      record.curve === undefined || record.curve === null
        ? null
        : requireNonEmptyString(record.curve, `${context}.curve`),
    public_inputs_schema_hash: requireNonEmptyString(
      record.public_inputs_schema_hash,
      `${context}.public_inputs_schema_hash`,
    ),
    commitment_hex: requireHexString(
      record.commitment,
      `${context}.commitment_hex`,
    ),
    vk_len: ToriiClient._normalizeUnsignedInteger(
      record.vk_len,
      `${context}.vk_len`,
      { allowZero: false },
    ),
    max_proof_bytes:
      record.max_proof_bytes === undefined || record.max_proof_bytes === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            record.max_proof_bytes,
            `${context}.max_proof_bytes`,
            { allowZero: false },
          ),
    gas_schedule_id:
      gasSchedule === null ? null : requireNonEmptyString(gasSchedule, `${context}.gas_schedule_id`),
    metadata_uri_cid:
      metadataCid === null ? null : requireNonEmptyString(metadataCid, `${context}.metadata_uri_cid`),
    vk_bytes_cid:
      vkBytesCid === null ? null : requireNonEmptyString(vkBytesCid, `${context}.vk_bytes_cid`),
    activation_height:
      record.activation_height === undefined || record.activation_height === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            record.activation_height,
            `${context}.activation_height`,
            { allowZero: true },
          ),
    withdraw_height:
      record.withdraw_height === undefined || record.withdraw_height === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            record.withdraw_height,
            `${context}.withdraw_height`,
            { allowZero: true },
          ),
    status: normalizeVerifyingKeyStatusValue(record.status, `${context}.status`),
    inline_key: normalizeVerifyingKeyInline(inlinePayload, `${context}.inline_key`),
  };
}

function normalizeVerifyingKeyInline(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  const backend = requireNonEmptyString(record.backend, `${context}.backend`);
  const bytesValue = record.bytes_b64;
  return {
    backend,
    bytes_b64: normalizeRequiredBase64Payload(bytesValue, `${context}.bytes_b64`),
  };
}

function normalizeVerifyingKeyStatusValue(value, context, { optional = false } = {}) {
  if (value === undefined || value === null || value === "") {
    if (optional) {
      return undefined;
    }
    throw new TypeError(`${context} must be a verifying key status`);
  }
  const normalized = requireNonEmptyString(String(value), context).toLowerCase();
  const canonical = VERIFYING_KEY_STATUS_ALIASES.get(normalized);
  if (!canonical) {
    throw new TypeError(`${context} must be one of ${[...VERIFYING_KEY_STATUS_VALUES].join(", ")}`);
  }
  return canonical;
}

function normalizeVerifyingKeyRegisterPayload(input) {
  const record = ensureRecord(input, "registerVerifyingKey payload");
  const authorityValue =
    record.authority;
  const privateKeyValue = record.private_key;
  const payload = {
    authority: ToriiClient._normalizeAccountId(
      authorityValue,
      "registerVerifyingKey.authority",
    ),
    private_key: requireNonEmptyString(privateKeyValue, "registerVerifyingKey.privateKey"),
    backend: requireNonEmptyString(record.backend, "registerVerifyingKey.backend"),
    name: requireNonEmptyString(record.name, "registerVerifyingKey.name"),
    version: ToriiClient._normalizeUnsignedInteger(
      record.version,
      "registerVerifyingKey.version",
      { allowZero: false },
    ),
    circuit_id: requireNonEmptyString(
      record.circuit_id,
      "registerVerifyingKey.circuitId",
    ),
    public_inputs_schema_hash_hex: requireHexString(
      record.public_inputs_schema_hash_hex ??
        record.public_inputs_schema_hash ??
        record.publicInputsSchemaHashHex ??
        record.publicInputsSchemaHash,
      "registerVerifyingKey.publicInputsSchemaHashHex",
    ),
    gas_schedule_id: requireNonEmptyString(
      record.gas_schedule_id,
      "registerVerifyingKey.gasScheduleId",
    ),
  };
  assignVerifyingKeyOptionalFields(record, payload, "registerVerifyingKey");
  return payload;
}

function normalizeVerifyingKeyUpdatePayload(input) {
  const record = ensureRecord(input, "updateVerifyingKey payload");
  const authorityValue =
    record.authority;
  const privateKeyValue = record.private_key;
  const payload = {
    authority: ToriiClient._normalizeAccountId(
      authorityValue,
      "updateVerifyingKey.authority",
    ),
    private_key: requireNonEmptyString(privateKeyValue, "updateVerifyingKey.privateKey"),
    backend: requireNonEmptyString(record.backend, "updateVerifyingKey.backend"),
    name: requireNonEmptyString(record.name, "updateVerifyingKey.name"),
    version: ToriiClient._normalizeUnsignedInteger(
      record.version,
      "updateVerifyingKey.version",
      { allowZero: false },
    ),
    circuit_id: requireNonEmptyString(
      record.circuit_id,
      "updateVerifyingKey.circuitId",
    ),
    public_inputs_schema_hash_hex: requireHexString(
      record.public_inputs_schema_hash_hex ??
        record.public_inputs_schema_hash ??
        record.publicInputsSchemaHashHex ??
        record.publicInputsSchemaHash,
      "updateVerifyingKey.publicInputsSchemaHashHex",
    ),
  };
  const gasSchedule = record.gas_schedule_id;
  if (gasSchedule !== undefined && gasSchedule !== null) {
    payload.gas_schedule_id = requireNonEmptyString(
      gasSchedule,
      "updateVerifyingKey.gasScheduleId",
    );
  }
  assignVerifyingKeyOptionalFields(record, payload, "updateVerifyingKey");
  return payload;
}

function assignVerifyingKeyOptionalFields(record, payload, context) {
  const curveValue = record.curve;
  if (curveValue !== undefined && curveValue !== null) {
    payload.curve = requireNonEmptyString(curveValue, `${context}.curve`);
  }
  const maxProofBytes = record.max_proof_bytes;
  if (maxProofBytes !== undefined && maxProofBytes !== null) {
    payload.max_proof_bytes = ToriiClient._normalizeUnsignedInteger(
      maxProofBytes,
      `${context}.maxProofBytes`,
      { allowZero: false },
    );
  }
  const metadataCid = record.metadata_uri_cid;
  if (metadataCid !== undefined && metadataCid !== null) {
    payload.metadata_uri_cid = requireNonEmptyString(
      metadataCid,
      `${context}.metadataUriCid`,
    );
  }
  const vkBytesCid = record.vk_bytes_cid;
  if (vkBytesCid !== undefined && vkBytesCid !== null) {
    payload.vk_bytes_cid = requireNonEmptyString(
      vkBytesCid,
      `${context}.vkBytesCid`,
    );
  }
  const activationHeight = record.activation_height;
  if (activationHeight !== undefined && activationHeight !== null) {
    payload.activation_height = ToriiClient._normalizeUnsignedInteger(
      activationHeight,
      `${context}.activationHeight`,
      { allowZero: true },
    );
  }
  const withdrawHeight = record.withdraw_height;
  if (withdrawHeight !== undefined && withdrawHeight !== null) {
    payload.withdraw_height = ToriiClient._normalizeUnsignedInteger(
      withdrawHeight,
      `${context}.withdrawHeight`,
      { allowZero: true },
    );
  }
  const commitmentValue = record.commitment_hex;
  if (commitmentValue !== undefined && commitmentValue !== null) {
    payload.commitment_hex = requireHexString(
      commitmentValue,
      `${context}.commitmentHex`,
    );
  }
  const statusValue = normalizeVerifyingKeyStatusValue(
    record.status,
    `${context}.status`,
    { optional: true },
  );
  if (statusValue) {
    payload.status = statusValue;
  }
  const bytesValue =
    record.vk_bytes ??
    record.verifyingKeyBytes ??
    record.bytes ??
    record.inlineKeyBytes;
  const lenValue = record.vk_len;
  if (bytesValue !== undefined && bytesValue !== null) {
    const { base64, length } = normalizeVerifyingKeyBytesValue(
      bytesValue,
      `${context}.vk_bytes`,
    );
    payload.vk_bytes = base64;
    if (lenValue !== undefined && lenValue !== null) {
      const normalizedLen = ToriiClient._normalizeUnsignedInteger(
        lenValue,
        `${context}.vkLen`,
        { allowZero: false },
      );
      if (normalizedLen !== length) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.vk_len must match vk_bytes length (${length})`,
          `${context}.vkLen`,
        );
      }
      payload.vk_len = normalizedLen;
    } else {
      payload.vk_len = length;
    }
  } else if (lenValue !== undefined && lenValue !== null) {
    payload.vk_len = ToriiClient._normalizeUnsignedInteger(
      lenValue,
      `${context}.vkLen`,
      { allowZero: false },
    );
  }
}

function normalizeVerifyingKeyBytesValue(value, context) {
  const base64 = normalizeRequiredBase64Payload(value, context);
  const buffer = Buffer.from(base64, "base64");
  return { base64, length: buffer.length };
}

function normalizeProverReportList(payload, filters, context) {
  if (payload == null) {
    return { kind: "reports", reports: [] };
  }
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be an array`);
  }
  if (payload.length === 0) {
    return { kind: "reports", reports: [] };
  }
  const idsOnlyRequested = isTruthyFilter(filters, "ids_only");
  const messagesOnlyRequested = isTruthyFilter(filters, "messages_only");
  const first = payload[0];
  if (typeof first === "string") {
    if (!idsOnlyRequested) {
      throw new Error(
        "Torii returned id-only prover report projection; pass { ids_only: true } to listProverReports to consume this payload",
      );
    }
    payload.forEach((value, index) =>
      requireNonEmptyString(value, `${context}[${index}]`),
    );
    return { kind: "ids", ids: payload.slice() };
  }
  if (
    isPlainObject(first) &&
    Object.keys(first).every((key) => key === "id" || key === "error")
  ) {
    if (!messagesOnlyRequested) {
      throw new Error(
        "Torii returned message-only prover report projection; pass { messages_only: true } to listProverReports to consume this payload",
      );
    }
    const messages = payload.map((entry, index) => {
      const record = ensureRecord(entry, `${context}[${index}]`);
      return {
        id: requireNonEmptyString(record.id, `${context}[${index}].id`),
        error:
          record.error === undefined || record.error === null
            ? null
            : requireNonEmptyString(record.error, `${context}[${index}].error`),
      };
    });
    return { kind: "messages", messages };
  }
  return {
    kind: "reports",
    reports: payload.map((entry, index) =>
      normalizeProverReportRecord(entry, `${context}[${index}]`),
    ),
  };
}

function normalizeProverReportRecord(value, context) {
  const record = ensureRecord(value, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  const ok = requireBooleanLike(record.ok, `${context}.ok`);
  const error =
    record.error === undefined || record.error === null
      ? null
      : requireNonEmptyString(record.error, `${context}.error`);
  const contentType = requireNonEmptyString(
    record.content_type,
    `${context}.content_type`,
  );
  const size = ToriiClient._normalizeUnsignedInteger(record.size, `${context}.size`, {
    allowZero: true,
  });
  const createdMs = ToriiClient._normalizeUnsignedInteger(
    record.created_ms,
    `${context}.created_ms`,
    { allowZero: true },
  );
  const processedMs = ToriiClient._normalizeUnsignedInteger(
    record.processed_ms,
    `${context}.processed_ms`,
    { allowZero: true },
  );
  const latencyMs = ToriiClient._normalizeUnsignedInteger(
    record.latency_ms ?? 0,
    `${context}.latency_ms`,
    { allowZero: true },
  );
  const zkTags = record.zk1_tags;
  const normalizedTags =
    zkTags === undefined || zkTags === null
      ? null
      : parseStringArray(zkTags, `${context}.zk1_tags`);
  return {
    id,
    ok,
    error,
    content_type: contentType,
    size,
    created_ms: createdMs,
    processed_ms: processedMs,
    latency_ms: latencyMs,
    zk1_tags: normalizedTags,
  };
}

function normalizeSumeragiEvidenceListResponse(payload) {
  const record = ensureRecord(payload, "sumeragi evidence response");
  const rawItems = record.items;
  if (!Array.isArray(rawItems)) {
    throw new TypeError("sumeragi evidence response.items must be an array");
  }
  const items = rawItems.map((item, index) =>
    normalizeSumeragiEvidenceRecord(item, `sumeragi evidence response.items[${index}]`),
  );
  const totalValue = record.total;
  const total =
    totalValue === undefined || totalValue === null
      ? items.length
      : requireNonNegativeIntegerLike(
          totalValue,
          "sumeragi evidence response.total",
        );
  return { total, items };
}

function normalizeSumeragiEvidenceRecord(value, context) {
  const record = ensureRecord(value, context);
  const kind = requireNonEmptyString(record.kind, `${context}.kind`);
  const base = {
    kind,
    recorded_height: requireNonNegativeIntegerLike(
      record.recorded_height,
      `${context}.recorded_height`,
    ),
    recorded_view: requireNonNegativeIntegerLike(
      record.recorded_view,
      `${context}.recorded_view`,
    ),
    recorded_ms: requireNonNegativeIntegerLike(
      record.recorded_ms,
      `${context}.recorded_ms`,
    ),
  };
  if (
    kind === "DoublePrepare" ||
    kind === "DoubleCommit"
  ) {
    const phase = requireEvidencePhase(
      pickOverride(record, "phase", "phase"),
      `${context}.phase`,
    );
    return {
      ...base,
      phase,
      height: requireNonNegativeIntegerLike(
        record.height,
        `${context}.height`,
      ),
      view: requireNonNegativeIntegerLike(
        record.view,
        `${context}.view`,
      ),
      epoch: requireNonNegativeIntegerLike(
        record.epoch,
        `${context}.epoch`,
      ),
      signer: requireNonEmptyString(
        record.signer,
        `${context}.signer`,
      ),
      block_hash_1: requireHexString(
        record.block_hash_1,
        `${context}.block_hash_1`,
      ),
      block_hash_2: requireHexString(
        record.block_hash_2,
        `${context}.block_hash_2`,
      ),
    };
  }
  if (kind === "InvalidQc") {
    return {
      ...base,
      height: requireNonNegativeIntegerLike(
        record.height,
        `${context}.height`,
      ),
      view: requireNonNegativeIntegerLike(
        record.view,
        `${context}.view`,
      ),
      epoch: requireNonNegativeIntegerLike(
        record.epoch,
        `${context}.epoch`,
      ),
      subject_block_hash: requireHexString(
        record.subject_block_hash,
        `${context}.subject_block_hash`,
      ),
      phase: requireEvidencePhase(
        record.phase,
        `${context}.phase`,
      ),
      reason: requireNonEmptyString(
        record.reason,
        `${context}.reason`,
      ),
    };
  }
  if (kind === "InvalidProposal") {
    return {
      ...base,
      height: requireNonNegativeIntegerLike(
        record.height,
        `${context}.height`,
      ),
      view: requireNonNegativeIntegerLike(
        record.view,
        `${context}.view`,
      ),
      epoch: requireNonNegativeIntegerLike(
        record.epoch,
        `${context}.epoch`,
      ),
      subject_block_hash: requireHexString(
        record.subject_block_hash,
        `${context}.subject_block_hash`,
      ),
      payload_hash: requireHexString(
        record.payload_hash,
        `${context}.payload_hash`,
      ),
      reason: requireNonEmptyString(
        record.reason,
        `${context}.reason`,
      ),
    };
  }
  if (kind === "Censorship") {
    return {
      ...base,
      tx_hash: requireHexString(
        record.tx_hash,
        `${context}.tx_hash`,
      ),
      receipt_count: requireNonNegativeIntegerLike(
        record.receipt_count,
        `${context}.receipt_count`,
      ),
      min_height: requireNonNegativeIntegerLike(
        record.min_height,
        `${context}.min_height`,
      ),
      max_height: requireNonNegativeIntegerLike(
        record.max_height,
        `${context}.max_height`,
      ),
      signers: requireStringArray(
        record.signers,
        `${context}.signers`,
      ),
    };
  }
  const detailValue = record.detail;
  if (detailValue === undefined || detailValue === null) {
    return base;
  }
  return {
    ...base,
    detail: requireNonEmptyString(detailValue, `${context}.detail`),
  };
}

function normalizeIterableItems(payload, context, normalizeItem) {
  const normalizedItems = payload.items.map((entry, index) =>
    normalizeItem(entry, `${context}.items[${index}]`),
  );
  return {
    items: normalizedItems,
    total: payload.total,
  };
}

function normalizeIdListResponse(payload, context) {
  return normalizeIterableItems(payload, context, normalizeListItemWithId);
}

function normalizeListItemWithId(value, context) {
  const record = ensureRecord(value, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  return { ...record, id };
}

function rejectAliasField(record, context, aliasKey, canonicalKey) {
  if (Object.prototype.hasOwnProperty.call(record, aliasKey)) {
    throw new TypeError(
      `${context}.${aliasKey} is not supported; use ${canonicalKey}`,
    );
  }
}

function normalizeAccountAssetListItem(value, context) {
  const record = ensureRecord(value, context);
  rejectAliasField(record, context, "assetId", "asset_id");
  const assetId = requireNonEmptyString(
    record.asset_id,
    `${context}.asset_id`,
  );
  if (typeof record.quantity !== "string") {
    throw new TypeError(`${context}.quantity must be a string`);
  }
  const quantity = requireNonEmptyString(record.quantity, `${context}.quantity`);
  const normalized = {
    ...record,
    asset_id: assetId,
    quantity,
  };
  return normalized;
}

function normalizeAssetHolderListItem(value, context) {
  const record = ensureRecord(value, context);
  rejectAliasField(record, context, "accountId", "account_id");
  const accountId = requireNonEmptyString(
    record.account_id,
    `${context}.account_id`,
  );
  if (typeof record.quantity !== "string") {
    throw new TypeError(`${context}.quantity must be a string`);
  }
  const quantity = requireNonEmptyString(record.quantity, `${context}.quantity`);
  const normalized = {
    ...record,
    account_id: accountId,
    quantity,
  };
  return normalized;
}

function normalizeAccountTransactionListItem(value, context) {
  const record = ensureRecord(value, context);
  rejectAliasField(record, context, "entrypointHash", "entrypoint_hash");
  rejectAliasField(record, context, "resultOk", "result_ok");
  rejectAliasField(record, context, "timestampMs", "timestamp_ms");
  const entrypointHash = requireNonEmptyString(
    record.entrypoint_hash,
    `${context}.entrypoint_hash`,
  );
  const resultOk = requireBooleanLike(
    record.result_ok,
    `${context}.result_ok`,
  );
  let authorityValue = record.authority;
  if (authorityValue !== undefined && authorityValue !== null) {
    authorityValue = requireNonEmptyString(authorityValue, `${context}.authority`);
  }
  let timestampValue = record.timestamp_ms;
  if (timestampValue !== undefined && timestampValue !== null) {
    timestampValue = ToriiClient._normalizeUnsignedInteger(
      timestampValue,
      `${context}.timestamp_ms`,
      { allowZero: true },
    );
  } else {
    timestampValue = undefined;
  }
  const normalized = {
    ...record,
    entrypoint_hash: entrypointHash,
    result_ok: resultOk,
  };
  if (authorityValue !== undefined) {
    normalized.authority = authorityValue;
  }
  if (timestampValue !== undefined) {
    normalized.timestamp_ms = timestampValue;
  } else {
    delete normalized.timestamp_ms;
  }
  return normalized;
}

function normalizeAccountPermissionItem(value, context) {
  const record = ensureRecord(value, context);
  const name = requireNonEmptyString(record.name, `${context}.name`);
  return { ...record, name };
}

function normalizeTriggerUpsertPayload(input) {
  const record = ensureRecord(input, "registerTrigger payload");
  const payload = cloneJsonValue(record, "registerTrigger.payload");
  const id = requireNonEmptyString(
    record.id,
    "registerTrigger.id",
  );
  payload.id = id;
  delete payload.trigger_id;
  delete payload.triggerId;

  const actionValue =
    record.action ??
    record.payload ??
    record.definition ??
    record.trigger ??
    record.triggerDefinition ??
    payload.action;
  if (actionValue === undefined || actionValue === null) {
    throw new TypeError("registerTrigger.action is required");
  }
  if (typeof actionValue === "string") {
    const trimmed = actionValue.trim();
    if (trimmed.length === 0) {
      throw new TypeError(
        "registerTrigger.action must be a non-empty string when provided as base64",
      );
    }
    try {
      const decoded = strictDecodeBase64(trimmed);
      payload.action = Buffer.from(decoded).toString("base64");
    } catch {
      throw new TypeError("registerTrigger.action must be a valid base64 string");
    }
  } else if (!Array.isArray(actionValue) && typeof actionValue === "object") {
    payload.action = cloneJsonValue(actionValue, "registerTrigger.action");
  } else {
    throw new TypeError(
      "registerTrigger.action must be an object or base64 string produced by Norito serialization",
    );
  }
  delete payload.payload;
  delete payload.definition;
  delete payload.trigger;
  delete payload.triggerDefinition;

  if (payload.metadata === null || payload.metadata === undefined) {
    delete payload.metadata;
  } else if (
    typeof payload.metadata !== "object" ||
    Array.isArray(payload.metadata)
  ) {
    throw new TypeError("registerTrigger.metadata must be an object when provided");
  }
  return payload;
}

function normalizeSubscriptionPlanCreateRequest(input) {
  const record = ensureRecord(input, "createSubscriptionPlan request");
  const credentials = normalizeAuthorityCredentials(record, "createSubscriptionPlan");
  const planId = pickOverride(record, "plan_id", "planId");
  if (planId === undefined || planId === null) {
    throw new TypeError("createSubscriptionPlan.planId is required");
  }
  const planValue = record.plan;
  if (planValue === undefined || planValue === null) {
    throw new TypeError("createSubscriptionPlan.plan is required");
  }
  if (!isPlainObject(planValue)) {
    throw new TypeError("createSubscriptionPlan.plan must be an object");
  }
  return {
    ...credentials,
    plan_id: ToriiClient._requireAssetDefinitionId(planId),
    plan: cloneJsonValue(planValue, "createSubscriptionPlan.plan"),
  };
}

function normalizeSubscriptionCreateRequest(input) {
  const record = ensureRecord(input, "createSubscription request");
  const credentials = normalizeAuthorityCredentials(record, "createSubscription");
  const subscriptionId = pickOverride(record, "subscription_id", "subscriptionId");
  if (subscriptionId === undefined || subscriptionId === null) {
    throw new TypeError("createSubscription.subscriptionId is required");
  }
  const planId = pickOverride(record, "plan_id", "planId");
  if (planId === undefined || planId === null) {
    throw new TypeError("createSubscription.planId is required");
  }
  const payload = {
    ...credentials,
    subscription_id: requireNonEmptyString(subscriptionId, "createSubscription.subscriptionId"),
    plan_id: ToriiClient._requireAssetDefinitionId(planId),
  };
  const billingTriggerId = pickOverride(record, "billing_trigger_id", "billingTriggerId");
  if (billingTriggerId !== undefined && billingTriggerId !== null) {
    payload.billing_trigger_id = requireNonEmptyString(
      billingTriggerId,
      "createSubscription.billingTriggerId",
    );
  }
  const usageTriggerId = pickOverride(record, "usage_trigger_id", "usageTriggerId");
  if (usageTriggerId !== undefined && usageTriggerId !== null) {
    payload.usage_trigger_id = requireNonEmptyString(
      usageTriggerId,
      "createSubscription.usageTriggerId",
    );
  }
  const firstChargeMs = pickOverride(record, "first_charge_ms", "firstChargeMs");
  if (firstChargeMs !== undefined && firstChargeMs !== null) {
    payload.first_charge_ms = ToriiClient._normalizeUnsignedInteger(
      firstChargeMs,
      "createSubscription.firstChargeMs",
      { allowZero: true },
    );
  }
  const grantUsage = pickOverride(record, "grant_usage_to_provider", "grantUsageToProvider");
  if (grantUsage !== undefined && grantUsage !== null) {
    payload.grant_usage_to_provider = requireBooleanLike(
      grantUsage,
      "createSubscription.grantUsageToProvider",
    );
  }
  return payload;
}

function normalizeSubscriptionActionRequest(input, context) {
  const record = ensureRecord(input, `${context} request`);
  const credentials = normalizeAuthorityCredentials(record, context);
  const payload = {
    ...credentials,
  };
  const chargeAt = pickOverride(record, "charge_at_ms", "chargeAtMs");
  if (chargeAt !== undefined && chargeAt !== null) {
    payload.charge_at_ms = ToriiClient._normalizeUnsignedInteger(
      chargeAt,
      `${context}.chargeAtMs`,
      { allowZero: true },
    );
  }
  const cancelMode = pickOverride(record, "cancel_mode", "cancelMode");
  if (cancelMode !== undefined && cancelMode !== null) {
    const normalized = requireNonEmptyString(cancelMode, `${context}.cancelMode`).toLowerCase();
    if (normalized !== "immediate" && normalized !== "period_end") {
      throw new TypeError(`${context}.cancelMode must be immediate or period_end`);
    }
    payload.cancel_mode = normalized;
  }
  return payload;
}

function normalizeSubscriptionUsageRequest(input, context) {
  const record = ensureRecord(input, `${context} request`);
  const credentials = normalizeAuthorityCredentials(record, context);
  const unitKey = pickOverride(record, "unit_key", "unitKey");
  if (unitKey === undefined || unitKey === null) {
    throw new TypeError(`${context}.unitKey is required`);
  }
  const delta = record.delta;
  if (delta === undefined || delta === null) {
    throw new TypeError(`${context}.delta is required`);
  }
  const payload = {
    ...credentials,
    unit_key: requireNonEmptyString(unitKey, `${context}.unitKey`),
    delta: normalizeNumericLiteral(delta, `${context}.delta`),
  };
  const usageTriggerId = pickOverride(record, "usage_trigger_id", "usageTriggerId");
  if (usageTriggerId !== undefined && usageTriggerId !== null) {
    payload.usage_trigger_id = requireNonEmptyString(
      usageTriggerId,
      `${context}.usageTriggerId`,
    );
  }
  return payload;
}

function normalizeTriggerRecord(payload, context) {
  const record = ensureRecord(payload, context);
  const id = requireNonEmptyString(record.id, `${context}.id`);
  const action = ensureRecord(record.action ?? {}, `${context}.action`);
  let metadata = {};
  if (record.metadata !== undefined && record.metadata !== null) {
    metadata = ensureRecord(record.metadata, `${context}.metadata`);
  }
  return {
    id,
    action,
    metadata,
    raw: record,
  };
}

function normalizeTriggerListResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeTriggerRecord(entry, `${context}.items[${index}]`),
  );
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return {
    items,
    total: totalValue,
  };
}

function normalizeSubscriptionPlanCreateResponse(payload) {
  const record = ensureRecord(payload, "subscription plan create response");
  return {
    ok: Boolean(record.ok),
    plan_id: requireNonEmptyString(record.plan_id, "subscriptionPlanCreate.plan_id"),
    tx_hash_hex: requireNonEmptyString(
      record.tx_hash_hex,
      "subscriptionPlanCreate.tx_hash_hex",
    ),
  };
}

function normalizeSubscriptionCreateResponse(payload) {
  const record = ensureRecord(payload, "subscription create response");
  const normalized = {
    ok: Boolean(record.ok),
    subscription_id: requireNonEmptyString(
      record.subscription_id,
      "subscriptionCreate.subscription_id",
    ),
    billing_trigger_id: requireNonEmptyString(
      record.billing_trigger_id,
      "subscriptionCreate.billing_trigger_id",
    ),
    first_charge_ms: ToriiClient._normalizeUnsignedInteger(
      record.first_charge_ms,
      "subscriptionCreate.first_charge_ms",
      { allowZero: true },
    ),
    tx_hash_hex: requireNonEmptyString(
      record.tx_hash_hex,
      "subscriptionCreate.tx_hash_hex",
    ),
  };
  if (record.usage_trigger_id !== undefined && record.usage_trigger_id !== null) {
    normalized.usage_trigger_id = requireNonEmptyString(
      record.usage_trigger_id,
      "subscriptionCreate.usage_trigger_id",
    );
  }
  return normalized;
}

function normalizeSubscriptionActionResponse(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    ok: Boolean(record.ok),
    subscription_id: requireNonEmptyString(
      record.subscription_id,
      `${context}.subscription_id`,
    ),
    tx_hash_hex: requireNonEmptyString(
      record.tx_hash_hex,
      `${context}.tx_hash_hex`,
    ),
  };
}

function normalizeSubscriptionPlanListResponse(payload) {
  const record = ensureRecord(payload ?? {}, "subscription plan list response");
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError("subscription plan list response.items must be an array");
  }
  const items = rawItems.map((entry, index) => {
    const item = ensureRecord(entry, `subscription plan list response.items[${index}]`);
    const planValue = item.plan;
    if (planValue === undefined || planValue === null) {
      throw new TypeError(
        `subscription plan list response.items[${index}].plan is required`,
      );
    }
    if (!isPlainObject(planValue)) {
      throw new TypeError(
        `subscription plan list response.items[${index}].plan must be an object`,
      );
    }
    return {
      plan_id: requireNonEmptyString(
        item.plan_id,
        `subscription plan list response.items[${index}].plan_id`,
      ),
      plan: cloneJsonValue(
        planValue,
        `subscription plan list response.items[${index}].plan`,
      ),
    };
  });
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(
          record.total,
          "subscription plan list response.total",
          { allowZero: true },
        );
  return {
    items,
    total: totalValue,
  };
}

function normalizeSubscriptionListResponse(payload) {
  const record = ensureRecord(payload ?? {}, "subscription list response");
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError("subscription list response.items must be an array");
  }
  const items = rawItems.map((entry, index) =>
    normalizeSubscriptionListItem(entry, `subscription list response.items[${index}]`),
  );
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(
          record.total,
          "subscription list response.total",
          { allowZero: true },
        );
  return {
    items,
    total: totalValue,
  };
}

function normalizeSubscriptionListItem(value, context) {
  const record = ensureRecord(value, context);
  const subscriptionValue = record.subscription;
  if (subscriptionValue === undefined || subscriptionValue === null) {
    throw new TypeError(`${context}.subscription is required`);
  }
  if (!isPlainObject(subscriptionValue)) {
    throw new TypeError(`${context}.subscription must be an object`);
  }
  let invoiceValue = record.invoice;
  if (invoiceValue !== undefined && invoiceValue !== null && !isPlainObject(invoiceValue)) {
    throw new TypeError(`${context}.invoice must be an object`);
  }
  let planValue = record.plan;
  if (planValue !== undefined && planValue !== null && !isPlainObject(planValue)) {
    throw new TypeError(`${context}.plan must be an object`);
  }
  const normalized = {
    subscription_id: requireNonEmptyString(
      record.subscription_id,
      `${context}.subscription_id`,
    ),
    subscription: cloneJsonValue(subscriptionValue, `${context}.subscription`),
  };
  if (invoiceValue !== undefined) {
    normalized.invoice =
      invoiceValue === null ? null : cloneJsonValue(invoiceValue, `${context}.invoice`);
  }
  if (planValue !== undefined) {
    normalized.plan =
      planValue === null ? null : cloneJsonValue(planValue, `${context}.plan`);
  }
  return normalized;
}

function normalizeSubscriptionGetResponse(payload) {
  const record = ensureRecord(payload, "subscription get response");
  return normalizeSubscriptionListItem(record, "subscription get response");
}

function normalizeOfflineCashRequest(input, context) {
  return ensureRecord(input, `${context} request`);
}

function normalizeOfflineCashReadinessResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  return {
    ...record,
    offline_recursive_stark: coerceBoolean(
      record.offline_recursive_stark,
      `${context}.offline_recursive_stark`,
    ),
  };
}

function normalizeOfflineCashEnvelope(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    ...record,
    lineage_state: ensureRecord(record.lineage_state, `${context}.lineage_state`),
  };
}

function normalizeOfflineRevocationBundleResponse(payload, context) {
  return ensureRecord(payload, context);
}

function normalizeOfflineAllowanceListResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeOfflineAllowanceListItem(entry, `${context}.items[${index}]`),
  );
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return {
    items,
    total: totalValue,
  };
}

function normalizeOfflineAllowanceListItem(value, context) {
  const record = ensureRecord(value, context);
  const certificateId = requireNonEmptyString(
    record.certificate_id_hex,
    `${context}.certificate_id_hex`,
  );
  const controllerId = ToriiClient._requireAccountId(
    record.controller_id,
    `${context}.controller_id`,
  );
  const controllerDisplay = requireNonEmptyString(
    record.controller_display,
    `${context}.controller_display`,
  );
  const assetId = requireNonEmptyString(
    record.asset_id,
    `${context}.asset_id`,
  );
  const assetDefinitionId = requireNonEmptyString(
    record.asset_definition_id,
    `${context}.asset_definition_id`,
  );
  const assetDefinitionName = requireNonEmptyString(
    record.asset_definition_name,
    `${context}.asset_definition_name`,
  );
  if (!Object.prototype.hasOwnProperty.call(record, "asset_definition_alias")) {
    throw new TypeError(`${context}.asset_definition_alias is required`);
  }
  const assetDefinitionAlias =
    record.asset_definition_alias === null
      ? null
      : requireNonEmptyString(
          record.asset_definition_alias,
          `${context}.asset_definition_alias`,
        );
  const registeredAt = ToriiClient._normalizeUnsignedInteger(
    record.registered_at_ms,
    `${context}.registered_at_ms`,
    { allowZero: true },
  );
  const expiresAt = ToriiClient._normalizeUnsignedInteger(
    record.expires_at_ms,
    `${context}.expires_at_ms`,
    { allowZero: true },
  );
  const policyExpiresAt =
    record.policy_expires_at_ms === undefined || record.policy_expires_at_ms === null
      ? expiresAt
      : ToriiClient._normalizeUnsignedInteger(
          record.policy_expires_at_ms,
          `${context}.policy_expires_at_ms`,
          { allowZero: true },
        );
  const refreshAtRaw = record.refresh_at_ms;
  const refreshAt =
    refreshAtRaw === undefined || refreshAtRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          refreshAtRaw,
          `${context}.refresh_at_ms`,
          { allowZero: true },
  );
  const verdictHex = normalizeOptionalHexString(
    record.verdict_id_hex,
    `${context}.verdict_id_hex`,
  );
  const attestationNonceHex = normalizeOptionalHexString(
    record.attestation_nonce_hex,
    `${context}.attestation_nonce_hex`,
  );
  const deadlineKind =
    ToriiClient._normalizeOptionalString(
      record.deadline_kind,
      `${context}.deadline_kind`,
    ) ?? null;
  const deadlineState =
    ToriiClient._normalizeOptionalString(
      record.deadline_state,
      `${context}.deadline_state`,
    ) ?? null;
  const deadlineMs = coerceOptionalInt(
    record.deadline_ms,
    `${context}.deadline_ms`,
  );
  const deadlineMsRemaining = coerceOptionalInt(
    record.deadline_ms_remaining,
    `${context}.deadline_ms_remaining`,
  );
  const allowanceRecord = ensureRecord(record.record, `${context}.record`);
  const integrityMetadataSource =
    allowanceRecord.metadata ?? allowanceRecord.certificate?.metadata;
  const integrityMetadataContext =
    allowanceRecord.metadata === undefined || allowanceRecord.metadata === null
      ? `${context}.record.certificate.metadata`
      : `${context}.record.metadata`;
  const integrityMetadata = normalizeOfflineIntegrityMetadata(
    integrityMetadataSource,
    integrityMetadataContext,
  );
  const remainingAmount = normalizeAmountLike(
    record.remaining_amount ?? allowanceRecord.remaining_amount,
    `${context}.remaining_amount`,
  );
  return {
    certificate_id_hex: certificateId,
    controller_id: controllerId,
    controller_display: controllerDisplay,
    asset_id: assetId,
    asset_definition_id: assetDefinitionId,
    asset_definition_name: assetDefinitionName,
    asset_definition_alias: assetDefinitionAlias,
    registered_at_ms: registeredAt,
    expires_at_ms: expiresAt,
    policy_expires_at_ms: policyExpiresAt,
    refresh_at_ms: refreshAt,
    verdict_id_hex: verdictHex,
    attestation_nonce_hex: attestationNonceHex,
    remaining_amount: remainingAmount,
    deadline_kind: deadlineKind,
    deadline_state: deadlineState,
    deadline_ms: deadlineMs,
    deadline_ms_remaining: deadlineMsRemaining,
    record: allowanceRecord,
    integrity_metadata: integrityMetadata,
  };
}

function normalizeOfflineIntegrityMetadata(rawMetadata, context) {
  if (rawMetadata === undefined || rawMetadata === null) {
    return null;
  }
  const metadata = ToriiClient._requirePlainObject(rawMetadata, context);
  const policyRaw = metadata["android.integrity.policy"];
  if (policyRaw === undefined || policyRaw === null) {
    return null;
  }
  const normalizedPolicy = requireNonEmptyString(
    policyRaw,
    `${context}["android.integrity.policy"]`,
  )
    .trim()
    .replace(/[\s-]+/g, "_")
    .toLowerCase();
  if (!normalizedPolicy) {
    return null;
  }
  if (normalizedPolicy === "provisioned") {
    const provisioned = normalizeProvisionedIntegrityMetadata(metadata, context);
    return {
      policy: normalizedPolicy,
      provisioned,
    };
  }
  if (normalizedPolicy === "play_integrity") {
    const playIntegrity = normalizePlayIntegrityMetadata(metadata, context);
    return {
      policy: normalizedPolicy,
      play_integrity: playIntegrity,
    };
  }
  if (normalizedPolicy === "hms_safety_detect") {
    const safetyDetect = normalizeHmsSafetyDetectMetadata(metadata, context);
    return {
      policy: normalizedPolicy,
      hms_safety_detect: safetyDetect,
    };
  }
  return { policy: normalizedPolicy };
}

function normalizeProvisionedIntegrityMetadata(metadata, context) {
  const inspectorKey = requireNonEmptyString(
    metadata["android.provisioned.inspector_public_key"],
    `${context}["android.provisioned.inspector_public_key"]`,
  );
  const manifestSchema = requireNonEmptyString(
    metadata["android.provisioned.manifest_schema"],
    `${context}["android.provisioned.manifest_schema"]`,
  );
  const manifestVersionRaw = metadata["android.provisioned.manifest_version"];
  const manifestVersion =
    manifestVersionRaw === undefined || manifestVersionRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          manifestVersionRaw,
          `${context}["android.provisioned.manifest_version"]`,
          { allowZero: true },
        );
  const maxManifestAgeRaw = metadata["android.provisioned.max_manifest_age_ms"];
  const maxManifestAgeMs =
    maxManifestAgeRaw === undefined || maxManifestAgeRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          maxManifestAgeRaw,
          `${context}["android.provisioned.max_manifest_age_ms"]`,
          { allowZero: true },
        );
  const manifestDigestHex = normalizeOptionalHexString(
    metadata["android.provisioned.manifest_digest"],
    `${context}["android.provisioned.manifest_digest"]`,
  );
  return {
    inspector_public_key: inspectorKey,
    manifest_schema: manifestSchema,
    manifest_version: manifestVersion,
    max_manifest_age_ms: maxManifestAgeMs,
    manifest_digest_hex: manifestDigestHex,
  };
}

function normalizePlayIntegrityMetadata(metadata, context) {
  const projectNumber = ToriiClient._normalizeUnsignedInteger(
    metadata["android.play_integrity.cloud_project_number"],
    `${context}["android.play_integrity.cloud_project_number"]`,
    { allowZero: false },
  );
  const environment = requireNonEmptyString(
    metadata["android.play_integrity.environment"],
    `${context}["android.play_integrity.environment"]`,
  );
  const packageNames = normalizeStringArray(
    metadata["android.play_integrity.package_names"],
    `${context}["android.play_integrity.package_names"]`,
  );
  const signingDigests = normalizeStringArray(
    metadata["android.play_integrity.signing_digests_sha256"],
    `${context}["android.play_integrity.signing_digests_sha256"]`,
  );
  const allowedAppVerdicts = normalizeStringArray(
    metadata["android.play_integrity.allowed_app_verdicts"],
    `${context}["android.play_integrity.allowed_app_verdicts"]`,
  );
  const allowedDeviceVerdicts = normalizeStringArray(
    metadata["android.play_integrity.allowed_device_verdicts"],
    `${context}["android.play_integrity.allowed_device_verdicts"]`,
  );
  const maxTokenAgeRaw = metadata["android.play_integrity.max_token_age_ms"];
  const maxTokenAgeMs =
    maxTokenAgeRaw === undefined || maxTokenAgeRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          maxTokenAgeRaw,
          `${context}["android.play_integrity.max_token_age_ms"]`,
          { allowZero: true },
        );
  return {
    cloud_project_number: projectNumber,
    environment,
    package_names: packageNames,
    signing_digests_sha256: signingDigests,
    allowed_app_verdicts: allowedAppVerdicts,
    allowed_device_verdicts: allowedDeviceVerdicts,
    max_token_age_ms: maxTokenAgeMs,
  };
}

function normalizeHmsSafetyDetectMetadata(metadata, context) {
  const appId = requireNonEmptyString(
    metadata["android.hms_safety_detect.app_id"],
    `${context}["android.hms_safety_detect.app_id"]`,
  );
  const packageNames = normalizeStringArray(
    metadata["android.hms_safety_detect.package_names"],
    `${context}["android.hms_safety_detect.package_names"]`,
  );
  const signingDigests = normalizeStringArray(
    metadata["android.hms_safety_detect.signing_digests_sha256"],
    `${context}["android.hms_safety_detect.signing_digests_sha256"]`,
  );
  const requiredEvaluations = normalizeStringArray(
    metadata["android.hms_safety_detect.required_evaluations"] ?? [],
    `${context}["android.hms_safety_detect.required_evaluations"]`,
  );
  const maxTokenAgeRaw = metadata["android.hms_safety_detect.max_token_age_ms"];
  const maxTokenAgeMs =
    maxTokenAgeRaw === undefined || maxTokenAgeRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          maxTokenAgeRaw,
          `${context}["android.hms_safety_detect.max_token_age_ms"]`,
          { allowZero: true },
        );
  return {
    app_id: appId,
    package_names: packageNames,
    signing_digests_sha256: signingDigests,
    required_evaluations: requiredEvaluations,
    max_token_age_ms: maxTokenAgeMs,
  };
}

function normalizeOfflineCertificateIssueResponse(payload, context) {
  const record = ensureRecord(payload, context);
  const certificateIdHex = normalizeHex32String(
    record.certificate_id_hex,
    `${context}.certificate_id_hex`,
  );
  const certificate = ensureRecord(record.certificate, `${context}.certificate`);
  return { certificate_id_hex: certificateIdHex, certificate };
}

function normalizeOfflineAllowanceRegisterResponse(payload, context) {
  const record = ensureRecord(payload, context);
  const certificateIdHex = normalizeHex32String(
    record.certificate_id_hex,
    `${context}.certificate_id_hex`,
  );
  return { certificate_id_hex: certificateIdHex };
}

function normalizeOfflineSettlementSubmitResponse(payload, context) {
  const record = ensureRecord(payload, context);
  const bundleIdHex = requireNonEmptyString(
    record.bundle_id_hex,
    `${context}.bundle_id_hex`,
  );
  const txHashRaw = record.transaction_hash_hex;
  const transactionHashHex =
    txHashRaw === undefined || txHashRaw === null
      ? null
      : requireNonEmptyString(txHashRaw, `${context}.transaction_hash_hex`);
  return {
    bundle_id_hex: bundleIdHex,
    transaction_hash_hex: transactionHashHex,
  };
}

function normalizeOfflineBuildClaimIssueRequest(input, context) {
  const record = ensureRecord(input, context);
  const certificateIdHex = normalizeHashLike32(
    pickOverride(record, "certificate_id_hex", "certificateIdHex"),
    `${context}.certificate_id_hex`,
  );
  const txIdHex = normalizeHashLike32(
    pickOverride(record, "tx_id_hex", "txIdHex"),
    `${context}.tx_id_hex`,
  );
  const platform = requireNonEmptyString(
    record.platform,
    `${context}.platform`,
  ).toLowerCase();
  if (platform !== "apple" && platform !== "android") {
    throw new TypeError(`${context}.platform must be "apple" or "android"`);
  }
  const appIdRaw = pickOverride(record, "app_id", "appId");
  const appId =
    appIdRaw === undefined || appIdRaw === null
      ? undefined
      : requireNonEmptyString(appIdRaw, `${context}.app_id`);
  const buildNumberRaw = pickOverride(record, "build_number", "buildNumber");
  const buildNumber =
    buildNumberRaw === undefined || buildNumberRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(buildNumberRaw, `${context}.build_number`, {
          allowZero: true,
        });
  const issuedAtMsRaw = pickOverride(record, "issued_at_ms", "issuedAtMs");
  const issuedAtMs =
    issuedAtMsRaw === undefined || issuedAtMsRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(issuedAtMsRaw, `${context}.issued_at_ms`, {
          allowZero: true,
        });
  const expiresAtMsRaw = pickOverride(record, "expires_at_ms", "expiresAtMs");
  const expiresAtMs =
    expiresAtMsRaw === undefined || expiresAtMsRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(expiresAtMsRaw, `${context}.expires_at_ms`, {
          allowZero: true,
        });
  const normalized = {
    certificate_id_hex: certificateIdHex,
    tx_id_hex: txIdHex,
    platform,
  };
  if (appId !== undefined) {
    normalized.app_id = appId;
  }
  if (buildNumber !== undefined) {
    normalized.build_number = buildNumber;
  }
  if (issuedAtMs !== undefined) {
    normalized.issued_at_ms = issuedAtMs;
  }
  if (expiresAtMs !== undefined) {
    normalized.expires_at_ms = expiresAtMs;
  }
  return normalized;
}

function normalizeOfflineBuildClaimIssueResponse(payload, context) {
  const record = ensureRecord(payload, context);
  const claimIdHex = normalizeHashLike32(
    record.claim_id_hex,
    `${context}.claim_id_hex`,
  );
  const buildClaim = normalizeOfflineBuildClaim(
    ensureRecord(record.build_claim, `${context}.build_claim`),
    `${context}.build_claim`,
  );
  return {
    claim_id_hex: claimIdHex,
    build_claim: buildClaim,
  };
}

function normalizeOfflineBuildClaim(record, context) {
  const claimIdLiteral = normalizeOptionalHashLiteral(
    pickOverride(record, "claim_id", "claimId"),
    `${context}.claim_id`,
  );
  if (!claimIdLiteral) {
    throw new TypeError(`${context}.claim_id must be provided`);
  }
  const nonceLiteral = normalizeOptionalHashLiteral(
    pickOverride(record, "nonce", "nonce"),
    `${context}.nonce`,
  );
  if (!nonceLiteral) {
    throw new TypeError(`${context}.nonce must be provided`);
  }
  const platform = normalizeOfflineBuildClaimResponsePlatform(
    pickOverride(record, "platform", "platform"),
    `${context}.platform`,
  );
  const appId = requireNonEmptyString(
    pickOverride(record, "app_id", "appId"),
    `${context}.app_id`,
  );
  const buildNumber = ToriiClient._normalizeUnsignedInteger(
    pickOverride(record, "build_number", "buildNumber"),
    `${context}.build_number`,
    { allowZero: true },
  );
  const issuedAtMs = ToriiClient._normalizeUnsignedInteger(
    pickOverride(record, "issued_at_ms", "issuedAtMs"),
    `${context}.issued_at_ms`,
    { allowZero: true },
  );
  const expiresAtMs = ToriiClient._normalizeUnsignedInteger(
    pickOverride(record, "expires_at_ms", "expiresAtMs"),
    `${context}.expires_at_ms`,
    { allowZero: true },
  );
  const operatorSignature = requireNonEmptyString(
    pickOverride(record, "operator_signature", "operatorSignature"),
    `${context}.operator_signature`,
  );
  const lineageScopeRaw = pickOverride(record, "lineage_scope", "lineageScope");
  const lineageScope =
    lineageScopeRaw === undefined || lineageScopeRaw === null
      ? undefined
      : requireNonEmptyString(lineageScopeRaw, `${context}.lineage_scope`);
  const normalized = {
    claim_id: claimIdLiteral,
    nonce: nonceLiteral,
    platform,
    app_id: appId,
    build_number: buildNumber,
    issued_at_ms: issuedAtMs,
    expires_at_ms: expiresAtMs,
    operator_signature: operatorSignature,
  };
  if (lineageScope !== undefined) {
    normalized.lineage_scope = lineageScope;
  }
  return normalized;
}

function normalizeOfflineBuildClaimResponsePlatform(value, context) {
  const normalized = requireNonEmptyString(value, context).trim().toLowerCase();
  if (normalized === "apple" || normalized === "ios") {
    return "Apple";
  }
  if (normalized === "android") {
    return "Android";
  }
  throw new TypeError(`${context} must be either "Apple" or "Android"`);
}

function normalizeOfflineSettlementSubmitRequest(input, context) {
  const record = ensureRecord(input, context);
  const credentials = normalizeAuthorityCredentials(record, context);
  const transfer = ensureRecord(record.transfer, `${context}.transfer`);
  const repairRaw = pickOverride(
    record,
    "repair_existing_build_claims",
    "repairExistingBuildClaims",
  );
  let repairExistingBuildClaims = false;
  if (repairRaw !== undefined && repairRaw !== null) {
    if (typeof repairRaw !== "boolean") {
      throw new TypeError(
        `${context}.repairExistingBuildClaims must be a boolean when provided`,
      );
    }
    repairExistingBuildClaims = repairRaw;
  }
  const overrides = normalizeOfflineSettlementBuildClaimOverrides(
    pickOverride(record, "build_claim_overrides", "buildClaimOverrides"),
    `${context}.build_claim_overrides`,
  );
  const normalized = {
    authority: credentials.authority,
    private_key: credentials.private_key,
    transfer,
  };
  if (overrides.length > 0) {
    normalized.build_claim_overrides = overrides;
  }
  if (repairExistingBuildClaims) {
    normalized.repair_existing_build_claims = true;
  }
  return normalized;
}

function normalizeOfflineSettlementBuildClaimOverrides(value, context) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array when provided`);
  }
  return value.map((entry, index) =>
    normalizeOfflineSettlementBuildClaimOverride(entry, `${context}[${index}]`),
  );
}

function normalizeOfflineSettlementBuildClaimOverride(value, context) {
  const record = ensureRecord(value, context);
  const txIdHex = normalizeHashLike32(
    pickOverride(record, "tx_id_hex", "txIdHex"),
    `${context}.tx_id_hex`,
  );
  const appIdRaw = pickOverride(record, "app_id", "appId");
  const appId =
    appIdRaw === undefined || appIdRaw === null
      ? undefined
      : requireNonEmptyString(appIdRaw, `${context}.app_id`);
  const buildNumberRaw = pickOverride(record, "build_number", "buildNumber");
  const buildNumber =
    buildNumberRaw === undefined || buildNumberRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(buildNumberRaw, `${context}.build_number`, {
          allowZero: true,
        });
  const issuedAtMsRaw = pickOverride(record, "issued_at_ms", "issuedAtMs");
  const issuedAtMs =
    issuedAtMsRaw === undefined || issuedAtMsRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(issuedAtMsRaw, `${context}.issued_at_ms`, {
          allowZero: true,
        });
  const expiresAtMsRaw = pickOverride(record, "expires_at_ms", "expiresAtMs");
  const expiresAtMs =
    expiresAtMsRaw === undefined || expiresAtMsRaw === null
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(expiresAtMsRaw, `${context}.expires_at_ms`, {
          allowZero: true,
        });
  return {
    tx_id_hex: txIdHex,
    app_id: appId,
    build_number: buildNumber,
    issued_at_ms: issuedAtMs,
    expires_at_ms: expiresAtMs,
  };
}

function normalizeOfflineTopUpRequest(input, context) {
  const record = ensureRecord(input, context);
  const certificate = normalizeOfflineCertificateDraft(
    record.certificate,
    `${context}.certificate`,
  );
  const credentials = normalizeAuthorityCredentials(record, context);
  return {
    authority: credentials.authority,
    private_key: credentials.private_key,
    certificate,
  };
}

function normalizeOfflineAllowanceRegisterRequest(input, context) {
  const record = ensureRecord(input, context);
  const certificate = normalizeOfflineCertificate(
    record.certificate,
    `${context}.certificate`,
  );
  const credentials = normalizeAuthorityCredentials(record, context);
  return {
    authority: credentials.authority,
    private_key: credentials.private_key,
    certificate,
  };
}

function normalizeOfflineCertificate(certificate, context) {
  const record = ensureRecord(certificate, context);
  const normalized = normalizeOfflineCertificateDraft(record, context);
  const operator = ToriiClient._normalizeAccountId(
    record.operator,
    `${context}.operator`,
  );
  const operatorSignature = normalizeUpperHex(
    pickOverride(record, "operator_signature", "operatorSignature"),
    `${context}.operator_signature`,
  );
  return {
    ...normalized,
    operator,
    operator_signature: operatorSignature,
  };
}

function normalizeOfflineCertificateDraft(draft, context) {
  const record = ensureRecord(draft, context);
  const controller = ToriiClient._normalizeAccountId(
    record.controller,
    `${context}.controller`,
  );
  const allowance = ensureRecord(record.allowance, `${context}.allowance`);
  const asset = requireNonEmptyString(allowance.asset, `${context}.allowance.asset`);
  const amount = normalizeAmountLike(
    allowance.amount,
    `${context}.allowance.amount`,
  );
  const commitment = normalizeOfflineBytesArray(
    allowance.commitment,
    `${context}.allowance.commitment`,
  );
  const spendPublicKey = requireNonEmptyString(
    record.spend_public_key,
    `${context}.spend_public_key`,
  );
  const attestationReport = normalizeOfflineBytesArray(
    record.attestation_report,
    `${context}.attestation_report`,
  );
  const issuedAtMs = ToriiClient._normalizeUnsignedInteger(
    record.issued_at_ms,
    `${context}.issued_at_ms`,
    { allowZero: true },
  );
  const expiresAtMs = ToriiClient._normalizeUnsignedInteger(
    record.expires_at_ms,
    `${context}.expires_at_ms`,
    { allowZero: true },
  );
  const policy = ensureRecord(record.policy, `${context}.policy`);
  const maxBalance = normalizeAmountLike(
    policy.max_balance,
    `${context}.policy.max_balance`,
  );
  const maxTxValue = normalizeAmountLike(
    policy.max_tx_value,
    `${context}.policy.max_tx_value`,
  );
  const policyExpiresAtMs = ToriiClient._normalizeUnsignedInteger(
    policy.expires_at_ms,
    `${context}.policy.expires_at_ms`,
    { allowZero: true },
  );
  const metadataRaw = record.metadata ?? {};
  const metadata = isPlainObject(metadataRaw)
    ? metadataRaw
    : ensureRecord(metadataRaw, `${context}.metadata`);
  const verdictId = normalizeOptionalHashLiteral(
    record.verdict_id,
    `${context}.verdict_id`,
  );
  const attestationNonce = normalizeOptionalHashLiteral(
    record.attestation_nonce,
    `${context}.attestation_nonce`,
  );
  const refreshAtMs =
    record.refresh_at_ms ?? null;
  const normalizedRefreshAtMs =
    refreshAtMs === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          refreshAtMs,
          `${context}.refresh_at_ms`,
          { allowZero: true },
        );
  return {
    controller,
    allowance: {
      asset,
      amount,
      commitment,
    },
    spend_public_key: spendPublicKey,
    attestation_report: attestationReport,
    issued_at_ms: issuedAtMs,
    expires_at_ms: expiresAtMs,
    policy: {
      max_balance: maxBalance,
      max_tx_value: maxTxValue,
      expires_at_ms: policyExpiresAtMs,
    },
    metadata,
    verdict_id: verdictId,
    attestation_nonce: attestationNonce,
    refresh_at_ms: normalizedRefreshAtMs,
  };
}

function ensureTopUpCertificateIdsMatch(issuedId, registeredId, context) {
  if (!issuedId || !registeredId) {
    throw new Error(`${context} missing certificate id in top-up response`);
  }
  if (issuedId.toLowerCase() !== registeredId.toLowerCase()) {
    throw new Error(
      `${context} certificate mismatch (issued ${issuedId}, registered ${registeredId})`,
    );
  }
}

function normalizeOfflineBytesArray(value, context) {
  if (value === undefined || value === null) {
    throw new TypeError(`${context} must be provided`);
  }
  const buffer = toBuffer(value);
  return Array.from(buffer.values());
}

function normalizeOfflineSummaryListResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeOfflineSummaryListItem(entry, `${context}.items[${index}]`),
  );
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return {
    items,
    total: totalValue,
  };
}

function normalizeOfflineSummaryListItem(value, context) {
  const record = ensureRecord(value, context);
  const certificateId = requireNonEmptyString(
    record.certificate_id_hex,
    `${context}.certificate_id_hex`,
  );
  const controllerId = ToriiClient._requireAccountId(
    record.controller_id,
    `${context}.controller_id`,
  );
  const controllerDisplay = requireNonEmptyString(
    record.controller_display,
    `${context}.controller_display`,
  );
  const summaryHash = requireNonEmptyString(
    record.summary_hash_hex,
    `${context}.summary_hash_hex`,
  );
  const appleCounters = normalizeCounterMap(
    record.apple_key_counters,
    `${context}.apple_key_counters`,
  );
  const androidCounters = normalizeCounterMap(
    record.android_series_counters,
    `${context}.android_series_counters`,
  );
  const policyCounters = normalizeCounterMap(
    record.policy_key_counters,
    `${context}.policy_key_counters`,
  );
  const counterTotals = normalizeCounterTotals(
    record.counter_totals,
    `${context}.counter_totals`,
  );
  const metadataRecord =
    record.metadata === undefined || record.metadata === null
      ? null
      : ToriiClient._requirePlainObject(record.metadata, `${context}.metadata`);
  return {
    certificate_id_hex: certificateId,
    controller_id: controllerId,
    controller_display: controllerDisplay,
    summary_hash_hex: summaryHash,
    apple_key_counters: appleCounters,
    android_series_counters: androidCounters,
    policy_key_counters: policyCounters,
    counter_totals: counterTotals,
    metadata: metadataRecord,
  };
}

function normalizeCounterMap(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const result = {};
  for (const [key, raw] of Object.entries(record)) {
    result[key] = ToriiClient._normalizeUnsignedInteger(raw, `${context}.${key}`, {
      allowZero: true,
    });
  }
  return result;
}

function normalizeCounterTotals(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const record = ensureRecord(value, context);
  const normalizeField = (snake) =>
    ToriiClient._normalizeUnsignedInteger(record[snake], `${context}.${snake}`, {
      allowZero: true,
    });
  return {
    total_counters: normalizeField("total_counters"),
    total_weight: normalizeField("total_weight"),
    apple: normalizeField("apple"),
    android: normalizeField("android"),
    policy: normalizeField("policy"),
  };
}

function normalizeOfflineTransferListResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeOfflineTransferListItem(entry, `${context}.items[${index}]`),
  );
  const totalValue =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return {
    items,
    total: totalValue,
  };
}

function normalizeOfflineTransferListItem(value, context) {
  const record = ensureRecord(value, context);
  const bundleId = requireNonEmptyString(
    record.bundle_id_hex,
    `${context}.bundle_id_hex`,
  );
  const controllerId = ToriiClient._requireAccountId(
    record.controller_id,
    `${context}.controller_id`,
  );
  const controllerDisplay = requireNonEmptyString(
    record.controller_display,
    `${context}.controller_display`,
  );
  const receiverId = ToriiClient._requireAccountId(
    record.receiver_id,
    `${context}.receiver_id`,
  );
  const receiverDisplay = requireNonEmptyString(
    record.receiver_display,
    `${context}.receiver_display`,
  );
  const depositAccountId = ToriiClient._requireAccountId(
    record.deposit_account_id,
    `${context}.deposit_account_id`,
  );
  const depositDisplay = requireNonEmptyString(
    record.deposit_account_display,
    `${context}.deposit_account_display`,
  );
  let assetId = null;
  if (record.asset_id !== undefined && record.asset_id !== null) {
    assetId = requireNonEmptyString(record.asset_id, `${context}.asset_id`);
  }
  const receiptCount = ToriiClient._normalizeUnsignedInteger(
    record.receipt_count,
    `${context}.receipt_count`,
    { allowZero: true },
  );
  const totalAmount = requireNonEmptyString(
    record.total_amount,
    `${context}.total_amount`,
  );
  const claimedDelta = requireNonEmptyString(
    record.claimed_delta,
    `${context}.claimed_delta`,
  );
  const status = requireNonEmptyString(record.status, `${context}.status`);
  const recordedAtMs = ToriiClient._normalizeUnsignedInteger(
    record.recorded_at_ms,
    `${context}.recorded_at_ms`,
    { allowZero: true },
  );
  const recordedAtHeight = ToriiClient._normalizeUnsignedInteger(
    record.recorded_at_height,
    `${context}.recorded_at_height`,
    { allowZero: true },
  );
  let archivedAtHeight = null;
  const archivedRaw = record.archived_at_height;
  if (archivedRaw !== undefined && archivedRaw !== null) {
    archivedAtHeight = ToriiClient._normalizeUnsignedInteger(
      archivedRaw,
      `${context}.archived_at_height`,
      { allowZero: true },
    );
  }
  const certIdRaw = record.certificate_id_hex;
  const certificateId = certIdRaw ? ToriiClient._normalizeOptionalString(certIdRaw, `${context}.certificate_id_hex`) : undefined;
  const certExpiresRaw = record.certificate_expires_at_ms;
  const certificateExpiresAtMs =
    certExpiresRaw === undefined || certExpiresRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          certExpiresRaw,
          `${context}.certificate_expires_at_ms`,
          { allowZero: true },
        );
  const policyExpiresRaw = record.policy_expires_at_ms;
  const policyExpiresAtMs =
    policyExpiresRaw === undefined || policyExpiresRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          policyExpiresRaw,
          `${context}.policy_expires_at_ms`,
          { allowZero: true },
        );
  const refreshRaw = record.refresh_at_ms;
  const refreshAtMs =
    refreshRaw === undefined || refreshRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          refreshRaw,
          `${context}.refresh_at_ms`,
          { allowZero: true },
        );
  const verdictIdRaw = record.verdict_id_hex;
  const verdictId = verdictIdRaw
    ? ToriiClient._normalizeOptionalString(verdictIdRaw, `${context}.verdict_id_hex`)
    : undefined;
  const nonceRaw = record.attestation_nonce_hex;
  const attestationNonce = nonceRaw
    ? ToriiClient._normalizeOptionalString(nonceRaw, `${context}.attestation_nonce_hex`)
    : undefined;
  const platformPolicyRaw = record.platform_policy ?? null;
  const platformPolicy =
    platformPolicyRaw === undefined || platformPolicyRaw === null
      ? null
      : requireNonEmptyString(platformPolicyRaw, `${context}.platform_policy`);
  const tokenSnapshotRaw = record.platform_token_snapshot ?? null;
  const platformTokenSnapshot =
    tokenSnapshotRaw === undefined || tokenSnapshotRaw === null
      ? null
      : normalizeOfflinePlatformTokenSnapshot(
          tokenSnapshotRaw,
          `${context}.platform_token_snapshot`,
        );
  const verdictSnapshotRaw = record.verdict_snapshot ?? null;
  const verdictSnapshot =
    verdictSnapshotRaw === undefined || verdictSnapshotRaw === null
      ? null
      : normalizeOfflineVerdictSnapshot(
          verdictSnapshotRaw,
          `${context}.verdict_snapshot`,
        );
  const transitionsRaw = record.status_transitions ?? [];
  if (!Array.isArray(transitionsRaw)) {
    throw new TypeError(`${context}.status_transitions must be an array`);
  }
  const statusTransitions = transitionsRaw.map((entry, index) =>
    normalizeOfflineStatusTransition(entry, `${context}.status_transitions[${index}]`),
  );
  const transferPayload = ensureRecord(record.transfer, `${context}.transfer`);
  const integrityMetadata = normalizeOfflineIntegrityMetadata(
    transferPayload.metadata,
    `${context}.transfer.metadata`,
  );
  return {
    bundle_id_hex: bundleId,
    controller_id: controllerId,
    controller_display: controllerDisplay,
    receiver_id: receiverId,
    receiver_display: receiverDisplay,
    deposit_account_id: depositAccountId,
    deposit_account_display: depositDisplay,
    asset_id: assetId,
    receipt_count: receiptCount,
    total_amount: totalAmount,
    claimed_delta: claimedDelta,
    status,
    recorded_at_ms: recordedAtMs,
    recorded_at_height: recordedAtHeight,
    archived_at_height: archivedAtHeight,
    certificate_id_hex: certificateId ?? null,
    certificate_expires_at_ms: certificateExpiresAtMs,
    policy_expires_at_ms: policyExpiresAtMs,
    refresh_at_ms: refreshAtMs,
    verdict_id_hex: verdictId ?? null,
    attestation_nonce_hex: attestationNonce ?? null,
    platform_policy: platformPolicy,
    platform_token_snapshot: platformTokenSnapshot,
    verdict_snapshot: verdictSnapshot,
    status_transitions: statusTransitions,
    transfer: transferPayload,
    integrity_metadata: integrityMetadata,
  };
}

function normalizeOfflinePlatformTokenSnapshot(value, context) {
  const record = ensureRecord(value, context);
  const policy = requireNonEmptyString(
    record.policy,
    `${context}.policy`,
  );
  const attestationJwsB64 = requireNonEmptyString(
    record.attestation_jws_b64,
    `${context}.attestation_jws_b64`,
  );
  return {
    policy,
    attestation_jws_b64: attestationJwsB64,
  };
}

function normalizeOfflineVerdictSnapshot(value, context) {
  const record = ensureRecord(value, context);
  const certificateId = requireNonEmptyString(
    record.certificate_id,
    `${context}.certificate_id`,
  );
  let verdictId = null;
  if (record.verdict_id !== undefined && record.verdict_id !== null) {
    verdictId = requireNonEmptyString(record.verdict_id, `${context}.verdict_id`);
  }
  let attestationNonce = null;
  const nonceRaw = record.attestation_nonce;
  if (nonceRaw !== undefined && nonceRaw !== null) {
    attestationNonce = requireNonEmptyString(nonceRaw, `${context}.attestation_nonce`);
  }
  const refreshRaw = record.refresh_at_ms;
  const refreshAtMs =
    refreshRaw === undefined || refreshRaw === null
      ? null
      : ToriiClient._normalizeUnsignedInteger(refreshRaw, `${context}.refresh_at_ms`, {
          allowZero: true,
        });
  const certificateExpiresAtMs = ToriiClient._normalizeUnsignedInteger(
    record.certificate_expires_at_ms,
    `${context}.certificate_expires_at_ms`,
    { allowZero: true },
  );
  const policyExpiresAtMs = ToriiClient._normalizeUnsignedInteger(
    record.policy_expires_at_ms,
    `${context}.policy_expires_at_ms`,
    { allowZero: true },
  );
  return {
    certificate_id: certificateId,
    verdict_id: verdictId,
    attestation_nonce: attestationNonce,
    refresh_at_ms: refreshAtMs,
    certificate_expires_at_ms: certificateExpiresAtMs,
    policy_expires_at_ms: policyExpiresAtMs,
  };
}

function normalizeOfflineStatusTransition(value, context) {
  const record = ensureRecord(value, context);
  const status = requireNonEmptyString(record.status, `${context}.status`);
  const transitionedAtMs = ToriiClient._normalizeUnsignedInteger(
    record.transitioned_at_ms,
    `${context}.transitioned_at_ms`,
    { allowZero: true },
  );
  const snapshotRaw = record.verdict_snapshot ?? null;
  const verdictSnapshot =
    snapshotRaw === undefined || snapshotRaw === null
      ? null
      : normalizeOfflineVerdictSnapshot(snapshotRaw, `${context}.verdict_snapshot`);
  return {
    status,
    transitioned_at_ms: transitionedAtMs,
    verdict_snapshot: verdictSnapshot,
  };
}

function normalizeOfflineRevocationListResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeOfflineRevocationItem(entry, `${context}.items[${index}]`),
  );
  const total =
    record.total === undefined || record.total === null
      ? items.length
      : ToriiClient._normalizeUnsignedInteger(record.total, `${context}.total`, {
          allowZero: true,
        });
  return { items, total };
}

function normalizeOfflineRevocationItem(value, context) {
  const record = ensureRecord(value, context);
  const verdictId = requireNonEmptyString(
    record.verdict_id_hex,
    `${context}.verdict_id_hex`,
  ).toLowerCase();
  const issuerId = ToriiClient._requireAccountId(
    record.issuer_id,
    `${context}.issuer_id`,
  );
  const issuerDisplay = requireNonEmptyString(
    record.issuer_display,
    `${context}.issuer_display`,
  );
  const revokedAt = ToriiClient._normalizeUnsignedInteger(
    record.revoked_at_ms,
    `${context}.revoked_at_ms`,
    { allowZero: true },
  );
  const reason = requireNonEmptyString(record.reason, `${context}.reason`);
  const note =
    record.note === undefined || record.note === null
      ? null
      : requireNonEmptyString(record.note, `${context}.note`);
  let metadata = null;
  if (record.metadata !== undefined && record.metadata !== null) {
    metadata = ToriiClient._requirePlainObject(record.metadata, `${context}.metadata`);
  }
  const payload = ensureRecord(record.record, `${context}.record`);
  return {
    verdict_id_hex: verdictId,
    issuer_id: issuerId,
    issuer_display: issuerDisplay,
    revoked_at_ms: revokedAt,
    reason,
    note,
    metadata,
    record: payload,
  };
}

function normalizeOfflineRejectionStatsResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const items = rawItems.map((entry, index) =>
    normalizeOfflineRejectionStatsItem(entry, `${context}.items[${index}]`),
  );
  const total = ToriiClient._normalizeUnsignedInteger(
    record.total ?? items.reduce((acc, item) => acc + item.count, 0),
    `${context}.total`,
    { allowZero: true },
  );
  return { total, items };
}

function normalizeOfflineRejectionStatsItem(value, context) {
  const record = ensureRecord(value, context);
  const platform = requireNonEmptyString(record.platform, `${context}.platform`);
  const reason = requireNonEmptyString(record.reason, `${context}.reason`);
  const count = ToriiClient._normalizeUnsignedInteger(record.count, `${context}.count`, {
    allowZero: true,
  });
  return { platform, reason, count };
}

function normalizeConnectSessionResponse(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const sid = normalizeConnectSid(record.sid, `${context}.sid`);
  const walletUri = requireNonEmptyString(
    record.wallet_uri,
    `${context}.wallet_uri`,
  );
  const appUri = requireNonEmptyString(
    record.app_uri,
    `${context}.app_uri`,
  );
  const tokenApp = requireNonEmptyString(
    record.token_app,
    `${context}.token_app`,
  );
  const tokenWallet = requireNonEmptyString(
    record.token_wallet,
    `${context}.token_wallet`,
  );
  const recognized = new Set([
    "sid",
    "wallet_uri",
    "app_uri",
    "token_app",
    "token_wallet",
  ]);
  const extra = extractExtraFields(record, recognized);
  return {
    sid,
    wallet_uri: walletUri,
    app_uri: appUri,
    token_app: tokenApp,
    token_wallet: tokenWallet,
    extra,
    raw: record,
  };
}

function normalizeConnectAppRecord(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const normalizedId = requireNonEmptyString(
    record.app_id ?? record.appId,
    `${context}.app_id`,
  );
  const displayName = optionalString(
    record.display_name ?? record.displayName,
    `${context}.display_name`,
  );
  const description = optionalString(
    record.description,
    `${context}.description`,
  );
  const iconUrl = optionalString(
    record.icon_url ?? record.iconUrl,
    `${context}.icon_url`,
  );
  const namespaceSource = record.namespaces ?? [];
  const namespaces = parseStringArray(
    namespaceSource ?? [],
    `${context}.namespaces`,
  );
  const metadataRaw = record.metadata ?? {};
  const policyRaw = record.policy ?? {};
  const metadata = ToriiClient._requirePlainObject(
    metadataRaw,
    `${context}.metadata`,
  );
  const policy = ToriiClient._requirePlainObject(policyRaw, `${context}.policy`);
  const recognized = new Set([
    "app_id",
    "appId",
    "display_name",
    "displayName",
    "description",
    "icon_url",
    "iconUrl",
    "namespaces",
    "metadata",
    "policy",
  ]);
  const extra = extractExtraFields(record, recognized);
  return {
    appId: normalizedId,
    displayName,
    description,
    iconUrl,
    namespaces,
    metadata,
    policy,
    extra,
    raw: record,
  };
}

function normalizeConnectAppRegistryPage(payload) {
  const record = ensureRecord(payload ?? {}, "connect app registry response");
  const rawItems = record.items ?? [];
  if (!Array.isArray(rawItems)) {
    throw new TypeError("connect app registry response.items must be an array");
  }
  const items = rawItems.map((entry, index) =>
    normalizeConnectAppRecord(entry, `connect app registry response.items[${index}]`),
  );
  let total = null;
  if (record.total !== undefined && record.total !== null) {
    total = ToriiClient._normalizeUnsignedInteger(
      record.total,
      "connect app registry response.total",
      { allowZero: true },
    );
  }
  let cursor = record.next_cursor ?? null;
  if (cursor !== null && cursor !== undefined) {
    cursor = requireNonEmptyString(cursor, "connect app registry response.next_cursor");
  } else {
    cursor = null;
  }
  const recognized = new Set(["items", "total", "next_cursor"]);
  const extra = extractExtraFields(record, recognized);
  return {
    items,
    total,
    nextCursor: cursor,
    extra,
    raw: record,
  };
}

function toConnectAppWritePayload(record, context) {
  const normalized = normalizeConnectAppRecord(record, context);
  const payload = {
    app_id: normalized.appId,
    namespaces: [...normalized.namespaces],
    metadata: { ...normalized.metadata },
    policy: { ...normalized.policy },
    ...normalized.extra,
  };
  if (normalized.displayName !== null) {
    payload.display_name = normalized.displayName;
  }
  if (normalized.description !== null) {
    payload.description = normalized.description;
  }
  if (normalized.iconUrl !== null) {
    payload.icon_url = normalized.iconUrl;
  }
  return payload;
}

function normalizeConnectAppPolicyControls(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const policyPayload =
    record.policy && typeof record.policy === "object" ? record.policy : record;
  const relayEnabled = optionalBoolean(
    policyPayload.relay_enabled ?? policyPayload.relayEnabled,
    `${context}.relay_enabled`,
  );
  const wsMaxSessions = coerceOptionalInt(
    policyPayload.ws_max_sessions ?? policyPayload.wsMaxSessions,
    `${context}.ws_max_sessions`,
  );
  const wsPerIpMaxSessions = coerceOptionalInt(
    policyPayload.ws_per_ip_max_sessions ?? policyPayload.wsPerIpMaxSessions,
    `${context}.ws_per_ip_max_sessions`,
  );
  const wsRatePerIpPerMin = coerceOptionalInt(
    policyPayload.ws_rate_per_ip_per_min ?? policyPayload.wsRatePerIpPerMin,
    `${context}.ws_rate_per_ip_per_min`,
  );
  const sessionTtlMs = coerceOptionalInt(
    policyPayload.session_ttl_ms ?? policyPayload.sessionTtlMs,
    `${context}.session_ttl_ms`,
  );
  const frameMaxBytes = coerceOptionalInt(
    policyPayload.frame_max_bytes ?? policyPayload.frameMaxBytes,
    `${context}.frame_max_bytes`,
  );
  const sessionBufferMaxBytes = coerceOptionalInt(
    policyPayload.session_buffer_max_bytes ?? policyPayload.sessionBufferMaxBytes,
    `${context}.session_buffer_max_bytes`,
  );
  const pingIntervalMs = coerceOptionalInt(
    policyPayload.ping_interval_ms ?? policyPayload.pingIntervalMs,
    `${context}.ping_interval_ms`,
  );
  const pingMissTolerance = coerceOptionalInt(
    policyPayload.ping_miss_tolerance ?? policyPayload.pingMissTolerance,
    `${context}.ping_miss_tolerance`,
  );
  const pingMinIntervalMs = coerceOptionalInt(
    policyPayload.ping_min_interval_ms ?? policyPayload.pingMinIntervalMs,
    `${context}.ping_min_interval_ms`,
  );
  const recognized = new Set([
    "relay_enabled",
    "relayEnabled",
    "ws_max_sessions",
    "wsMaxSessions",
    "ws_per_ip_max_sessions",
    "wsPerIpMaxSessions",
    "ws_rate_per_ip_per_min",
    "wsRatePerIpPerMin",
    "session_ttl_ms",
    "sessionTtlMs",
    "frame_max_bytes",
    "frameMaxBytes",
    "session_buffer_max_bytes",
    "sessionBufferMaxBytes",
    "ping_interval_ms",
    "pingIntervalMs",
    "ping_miss_tolerance",
    "pingMissTolerance",
    "ping_min_interval_ms",
    "pingMinIntervalMs",
  ]);
  const extra = extractExtraFields(policyPayload, recognized);
  return {
    relayEnabled,
    wsMaxSessions,
    wsPerIpMaxSessions,
    wsRatePerIpPerMin,
    sessionTtlMs,
    frameMaxBytes,
    sessionBufferMaxBytes,
    pingIntervalMs,
    pingMissTolerance,
    pingMinIntervalMs,
    extra,
    raw: policyPayload,
  };
}

function toConnectAppPolicyPayload(updates, context) {
  const normalized = normalizeConnectAppPolicyControls(updates, context);
  const payload = { ...normalized.extra };
  if (normalized.relayEnabled !== null) {
    payload.relay_enabled = normalized.relayEnabled;
  }
  if (normalized.wsMaxSessions !== null) {
    payload.ws_max_sessions = normalized.wsMaxSessions;
  }
  if (normalized.wsPerIpMaxSessions !== null) {
    payload.ws_per_ip_max_sessions = normalized.wsPerIpMaxSessions;
  }
  if (normalized.wsRatePerIpPerMin !== null) {
    payload.ws_rate_per_ip_per_min = normalized.wsRatePerIpPerMin;
  }
  if (normalized.sessionTtlMs !== null) {
    payload.session_ttl_ms = normalized.sessionTtlMs;
  }
  if (normalized.frameMaxBytes !== null) {
    payload.frame_max_bytes = normalized.frameMaxBytes;
  }
  if (normalized.sessionBufferMaxBytes !== null) {
    payload.session_buffer_max_bytes = normalized.sessionBufferMaxBytes;
  }
  if (normalized.pingIntervalMs !== null) {
    payload.ping_interval_ms = normalized.pingIntervalMs;
  }
  if (normalized.pingMissTolerance !== null) {
    payload.ping_miss_tolerance = normalized.pingMissTolerance;
  }
  if (normalized.pingMinIntervalMs !== null) {
    payload.ping_min_interval_ms = normalized.pingMinIntervalMs;
  }
  return payload;
}

function normalizeConnectAdmissionManifest(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const manifestRecord = isPlainObject(record.manifest) ? record.manifest : record;
  const rawEntries = manifestRecord.entries ?? [];
  if (!Array.isArray(rawEntries)) {
    throw new TypeError(`${context}.entries must be an array`);
  }
  const entries = rawEntries.map((entry, index) =>
    normalizeConnectAdmissionManifestEntry(entry, `${context}.entries[${index}]`),
  );
  const version = coerceOptionalInt(
    manifestRecord.version,
    `${context}.version`,
  );
  const manifestHash = optionalString(
    manifestRecord.manifest_hash ?? manifestRecord.manifestHash,
    `${context}.manifest_hash`,
  );
  const updatedAt = optionalString(
    manifestRecord.updated_at ?? manifestRecord.updatedAt,
    `${context}.updated_at`,
  );
  const recognized = new Set([
    "entries",
    "version",
    "manifest_hash",
    "manifestHash",
    "updated_at",
    "updatedAt",
  ]);
  const extra = extractExtraFields(manifestRecord, recognized);
  return {
    version,
    entries,
    manifestHash,
    updatedAt,
    extra,
    raw: record,
  };
}

function normalizeConnectAdmissionManifestEntry(payload, context) {
  const record = ensureRecord(payload ?? {}, context);
  const appId = requireNonEmptyString(
    record.app_id ?? record.appId,
    `${context}.app_id`,
  );
  const namespaces = parseStringArray(record.namespaces ?? [], `${context}.namespaces`);
  const metadata = ToriiClient._requirePlainObject(
    record.metadata ?? {},
    `${context}.metadata`,
  );
  const policy = ToriiClient._requirePlainObject(record.policy ?? {}, `${context}.policy`);
  const recognized = new Set([
    "app_id",
    "appId",
    "namespaces",
    "metadata",
    "policy",
  ]);
  const extra = extractExtraFields(record, recognized);
  return {
    appId,
    namespaces,
    metadata,
    policy,
    extra,
    raw: record,
  };
}

function toConnectAdmissionManifestPayload(input, context) {
  const normalized = normalizeConnectAdmissionManifest(input, context);
  const payload = {
    entries: normalized.entries.map((entry) => ({
      app_id: entry.appId,
      namespaces: [...entry.namespaces],
      metadata: { ...entry.metadata },
      policy: { ...entry.policy },
      ...entry.extra,
    })),
    ...normalized.extra,
  };
  if (normalized.version !== null) {
    payload.version = normalized.version;
  }
  if (normalized.manifestHash !== null) {
    payload.manifest_hash = normalized.manifestHash;
  }
  if (normalized.updatedAt !== null) {
    payload.updated_at = normalized.updatedAt;
  }
  return payload;
}

function normalizeConnectStatusSnapshot(payload, context) {
  const record = ensureRecord(payload, context);
  const perIpRaw = Array.isArray(record.per_ip_sessions) ? record.per_ip_sessions : [];
  const perIpSessions = perIpRaw.map((entry, index) =>
    normalizeConnectPerIpSessions(entry, `${context}.per_ip_sessions[${index}]`),
  );
  let policy = null;
  if (record.policy !== undefined && record.policy !== null) {
    policy = normalizeConnectStatusPolicySnapshot(
      record.policy,
      `${context}.policy`,
    );
  }
  return {
    enabled: Boolean(record.enabled),
    sessionsTotal: coerceStatusInt(
      record.sessions_total,
      `${context}.sessions_total`,
    ),
    sessionsActive: coerceStatusInt(
      record.sessions_active,
      `${context}.sessions_active`,
    ),
    perIpSessions,
    bufferedSessions: coerceStatusInt(
      record.buffered_sessions,
      `${context}.buffered_sessions`,
    ),
    totalBufferBytes: coerceStatusInt(
      record.total_buffer_bytes,
      `${context}.total_buffer_bytes`,
    ),
    dedupeSize: coerceStatusInt(record.dedupe_size, `${context}.dedupe_size`),
    policy,
    framesInTotal: coerceStatusInt(
      record.frames_in_total,
      `${context}.frames_in_total`,
    ),
    framesOutTotal: coerceStatusInt(
      record.frames_out_total,
      `${context}.frames_out_total`,
    ),
    ciphertextTotal: coerceStatusInt(
      record.ciphertext_total,
      `${context}.ciphertext_total`,
    ),
    dedupeDropsTotal: coerceStatusInt(
      record.dedupe_drops_total,
      `${context}.dedupe_drops_total`,
    ),
    bufferDropsTotal: coerceStatusInt(
      record.buffer_drops_total,
      `${context}.buffer_drops_total`,
    ),
    plaintextControlDropsTotal: coerceStatusInt(
      record.plaintext_control_drops_total,
      `${context}.plaintext_control_drops_total`,
    ),
    monotonicDropsTotal: coerceStatusInt(
      record.monotonic_drops_total,
      `${context}.monotonic_drops_total`,
    ),
    sequenceViolationClosesTotal: coerceStatusInt(
      record.sequence_violation_closes_total,
      `${context}.sequence_violation_closes_total`,
    ),
    roleDirectionMismatchTotal: coerceStatusInt(
      record.role_direction_mismatch_total,
      `${context}.role_direction_mismatch_total`,
    ),
    pingMissTotal: coerceStatusInt(
      record.ping_miss_total,
      `${context}.ping_miss_total`,
    ),
    p2pRebroadcastsTotal: coerceStatusInt(
      record.p2p_rebroadcasts_total,
      `${context}.p2p_rebroadcasts_total`,
    ),
    p2pRebroadcastSkippedTotal: coerceStatusInt(
      record.p2p_rebroadcast_skipped_total,
      `${context}.p2p_rebroadcast_skipped_total`,
    ),
  };
}

function normalizeConnectPerIpSessions(payload, context) {
  const record = ensureRecord(payload, context);
  const ip = requireNonEmptyString(
    record.ip,
    `${context}.ip`,
  );
  const sessions = ToriiClient._normalizeUnsignedInteger(
    record.sessions,
    `${context}.sessions`,
    { allowZero: true },
  );
  return { ip, sessions };
}

function normalizeConnectStatusPolicySnapshot(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    wsMaxSessions: ToriiClient._normalizeUnsignedInteger(
      record.ws_max_sessions,
      `${context}.ws_max_sessions`,
      { allowZero: true },
    ),
    wsPerIpMaxSessions: ToriiClient._normalizeUnsignedInteger(
      record.ws_per_ip_max_sessions,
      `${context}.ws_per_ip_max_sessions`,
      { allowZero: true },
    ),
    wsRatePerIpPerMin: ToriiClient._normalizeUnsignedInteger(
      record.ws_rate_per_ip_per_min,
      `${context}.ws_rate_per_ip_per_min`,
      { allowZero: true },
    ),
    sessionTtlMs: ToriiClient._normalizeUnsignedInteger(
      record.session_ttl_ms,
      `${context}.session_ttl_ms`,
      { allowZero: true },
    ),
    frameMaxBytes: ToriiClient._normalizeUnsignedInteger(
      record.frame_max_bytes,
      `${context}.frame_max_bytes`,
      { allowZero: true },
    ),
    sessionBufferMaxBytes: ToriiClient._normalizeUnsignedInteger(
      record.session_buffer_max_bytes,
      `${context}.session_buffer_max_bytes`,
      { allowZero: true },
    ),
    relayEnabled: requireBooleanLike(
      record.relay_enabled,
      `${context}.relay_enabled`,
    ),
    relayStrategy: requireNonEmptyString(
      record.relay_strategy,
      `${context}.relay_strategy`,
    ),
    relayEffectiveStrategy: requireNonEmptyString(
      record.relay_effective_strategy,
      `${context}.relay_effective_strategy`,
    ),
    relayP2pAttached: requireBooleanLike(
      record.relay_p2p_attached,
      `${context}.relay_p2p_attached`,
    ),
    heartbeatIntervalMs: ToriiClient._normalizeUnsignedInteger(
      record.heartbeat_interval_ms,
      `${context}.heartbeat_interval_ms`,
      { allowZero: true },
    ),
    heartbeatMissTolerance: ToriiClient._normalizeUnsignedInteger(
      record.heartbeat_miss_tolerance,
      `${context}.heartbeat_miss_tolerance`,
      { allowZero: true },
    ),
    heartbeatMinIntervalMs: ToriiClient._normalizeUnsignedInteger(
      record.heartbeat_min_interval_ms,
      `${context}.heartbeat_min_interval_ms`,
      { allowZero: true },
    ),
  };
}

const DEFAULT_CONNECT_WS_PATH = "/v1/connect/ws";

function buildConnectWebSocketDescriptor(baseUrl, options, context) {
  const resolvedBase = requireNonEmptyString(baseUrl, `${context}.baseUrl`);
  const baseParsed = new URL(resolvedBase);
  const baseHost = baseParsed.host;
  const baseWebSocketProtocol = mapHttpProtocolToWebSocket(baseParsed.protocol, context);
  const baseHasTokenQuery = baseParsed.searchParams.has("token");
  const params =
    options === undefined
      ? {}
      : requirePlainObjectOption(options, `${context} options`, {
          message: "must be an object",
        });
  const normalizedSid = normalizeConnectSid(params.sid, `${context}.sid`);
  const normalizedRole = normalizeConnectRole(params.role, `${context}.role`);
  const token = requireNonEmptyString(params.token, `${context}.token`);
  const endpointPath = params.endpointPath ?? DEFAULT_CONNECT_WS_PATH;
  const baseWithSlash = resolvedBase.endsWith("/") ? resolvedBase : `${resolvedBase}/`;
  const endpoint = new URL(endpointPath, baseWithSlash);
  const endpointHost = endpoint.host;
  const endpointProtocol = mapHttpProtocolToWebSocket(endpoint.protocol, context);
  if (endpointHost !== baseHost) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: endpointPath host ${endpointHost} must match baseUrl host ${baseHost} when credentials are attached.`,
      `${context}.endpointPath`,
    );
  }
  if (endpointProtocol !== baseWebSocketProtocol) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: endpointPath protocol must match baseUrl protocol (${baseWebSocketProtocol.replace(":", "")}) when credentials are attached.`,
      `${context}.endpointPath`,
    );
  }
  const hadQueryToken = endpoint.searchParams.has("token");
  endpoint.search = "";
  endpoint.protocol = endpointProtocol;
  const allowInsecure =
    params.allowInsecure ??
    (params.config &&
      typeof params.config === "object" &&
      params.config.allowInsecure) ??
    false;
  if (!allowInsecure && !isSecureProtocol(endpoint.protocol)) {
    throw new Error(
      `${context}: refusing insecure WebSocket protocol ${endpoint.protocol}; pass allowInsecure: true for local/dev use only.`,
    );
  }
  if (hadQueryToken) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: query parameter token is forbidden; send the token via Authorization or Sec-WebSocket-Protocol instead.`,
      `${context}.token`,
    );
  }
  if (baseHasTokenQuery) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: baseUrl must not include token query parameters; send the token via Authorization or Sec-WebSocket-Protocol instead.`,
      `${context}.token`,
    );
  }
  endpoint.searchParams.set("sid", normalizedSid);
  endpoint.searchParams.set("role", normalizedRole);
  return {
    url: endpoint.toString(),
    token,
    role: normalizedRole,
    sid: normalizedSid,
    allowInsecure,
  };
}

function buildConnectWebSocketUrlInternal(baseUrl, options, context) {
  return buildConnectWebSocketDescriptor(baseUrl, options, context).url;
}

function openConnectWebSocketInternal(options, context) {
  const normalizedOptions = requirePlainObjectOption(options, `${context} options`, {
    message: "must be an object",
  });
  const { baseUrl, protocols, websocketOptions, WebSocketImpl } = normalizedOptions;
  const { url, token, allowInsecure } = buildConnectWebSocketDescriptor(
    baseUrl,
    normalizedOptions,
    context,
  );
  const parsedBaseUrl = new URL(baseUrl);
  const parsedUrl = new URL(url);
  const pathIsAbsolute = isAbsoluteUrl(normalizedOptions.endpointPath);
  const originMatches =
    parsedUrl.host === parsedBaseUrl.host &&
    parsedUrl.protocol === mapHttpProtocolToWebSocket(parsedBaseUrl.protocol, context);
  emitConnectInsecureTelemetry(
    normalizedOptions.insecureTransportTelemetryHook,
    {
      client: "connect-ws",
      method: "GET",
      url: parsedUrl.toString(),
      baseUrl: parsedBaseUrl.toString(),
      host: parsedUrl.host,
      protocol: parsedUrl.protocol,
      pathIsAbsolute,
      originMatches,
      allowInsecure: Boolean(allowInsecure),
      hasCredentials: Boolean(token),
    },
  );
  const WebSocketClass = resolveWebSocketImplementation(WebSocketImpl, context);
  const supportsHeaders = !isLikelyBrowserWebSocket(WebSocketClass);
  const resolvedOptions =
    websocketOptions === undefined || websocketOptions === null ? {} : { ...websocketOptions };
  const headerContainer =
    resolvedOptions.headers && typeof resolvedOptions.headers === "object"
      ? { ...resolvedOptions.headers }
      : {};
  if (token && supportsHeaders) {
    if (!hasHeader(headerContainer, "authorization")) {
      headerContainer.Authorization = `Bearer ${token}`;
    }
  }
  const finalProtocols = supportsHeaders
    ? protocols
    : attachProtocolToken(protocols, buildConnectProtocolToken(token), context);
  if (Object.keys(headerContainer).length > 0) {
    resolvedOptions.headers = headerContainer;
  } else if (!websocketOptions) {
    delete resolvedOptions.headers;
  }
  const shouldPassOptions =
    websocketOptions !== undefined || Object.keys(headerContainer).length > 0;
  if (shouldPassOptions) {
    if (finalProtocols !== undefined) {
      return new WebSocketClass(url, finalProtocols, resolvedOptions);
    }
    return new WebSocketClass(url, resolvedOptions);
  }
  if (finalProtocols !== undefined) {
    return new WebSocketClass(url, finalProtocols);
  }
  return new WebSocketClass(url);
}

function emitConnectInsecureTelemetry(hook, event) {
  if (typeof hook !== "function") {
    return;
  }
  if (!event.allowInsecure || !event.hasCredentials || isSecureProtocol(event.protocol)) {
    return;
  }
  try {
    hook({ ...event, timestampMs: Date.now() });
  } catch {
    // No-op: telemetry hooks must never break Connect websockets.
  }
}

function normalizeConnectRole(value, name) {
  const normalized = requireNonEmptyString(value, name).trim().toLowerCase();
  if (normalized === "app" || normalized === "wallet") {
    return normalized;
  }
  throw new TypeError(`${name} must be either "app" or "wallet"`);
}

function mapHttpProtocolToWebSocket(protocol, context) {
  switch (protocol) {
    case "http:":
      return "ws:";
    case "https:":
      return "wss:";
    case "ws:":
    case "wss:":
      return protocol;
    default:
      throw new Error(`${context}: unsupported protocol "${protocol}" for Connect WebSocket`);
  }
}

function buildConnectProtocolToken(token) {
  return `iroha-connect.token.v1.${encodeBase64Url(token)}`;
}

function attachProtocolToken(protocols, tokenProtocol, context) {
  if (protocols === undefined || protocols === null) {
    return [tokenProtocol];
  }
  if (typeof protocols === "string") {
    return protocols === tokenProtocol ? protocols : [tokenProtocol, protocols];
  }
  if (Array.isArray(protocols)) {
    if (protocols.includes(tokenProtocol)) {
      return [...protocols];
    }
    return [tokenProtocol, ...protocols];
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${context}.protocols must be a string or array when header authentication is unavailable`,
    `${context}.protocols`,
  );
}

function encodeBase64Url(value) {
  const raw = Buffer.from(String(value), "utf8").toString("base64");
  return raw.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/u, "");
}

function resolveWebSocketImplementation(candidate, context) {
  const impl = candidate ?? globalThis.WebSocket;
  if (typeof impl !== "function") {
    throw new Error(`${context}: WebSocket implementation is required`);
  }
  return impl;
}

function isLikelyBrowserWebSocket(impl) {
  const browserWebSocket =
    typeof globalThis !== "undefined" &&
    globalThis.window &&
    typeof globalThis.window.WebSocket !== "undefined"
      ? globalThis.window.WebSocket
      : null;
  if (!browserWebSocket) {
    return false;
  }
  return impl === browserWebSocket;
}

function normalizeSumeragiRbcSnapshot(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    sessionsActive: coerceStatusInt(
      record.sessions_active,
      `${context}.sessions_active`,
    ),
    sessionsPrunedTotal: coerceStatusInt(
      record.sessions_pruned_total,
      `${context}.sessions_pruned_total`,
    ),
    readyBroadcastsTotal: coerceStatusInt(
      record.ready_broadcasts_total,
      `${context}.ready_broadcasts_total`,
    ),
    deliverBroadcastsTotal: coerceStatusInt(
      record.deliver_broadcasts_total,
      `${context}.deliver_broadcasts_total`,
    ),
    payloadBytesDeliveredTotal: coerceStatusInt(
      record.payload_bytes_delivered_total,
      `${context}.payload_bytes_delivered_total`,
    ),
  };
}

function normalizeSumeragiRbcSessionsSnapshot(payload, context) {
  const record = ensureRecord(payload, context);
  const rawItems = Array.isArray(record.items) ? record.items : [];
  const items = rawItems.map((entry, index) =>
    normalizeSumeragiRbcSession(entry, `${context}.items[${index}]`),
  );
  return {
    sessionsActive: coerceStatusInt(
      record.sessions_active,
      `${context}.sessions_active`,
    ),
    items,
  };
}

function normalizeSumeragiRbcSession(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    blockHash: optionalString(record.block_hash, `${context}.block_hash`),
    height: coerceStatusInt(record.height, `${context}.height`),
    view: coerceStatusInt(record.view, `${context}.view`),
    totalChunks: coerceStatusInt(record.total_chunks, `${context}.total_chunks`),
    receivedChunks: coerceStatusInt(
      record.received_chunks,
      `${context}.received_chunks`,
    ),
    readyCount: coerceStatusInt(record.ready_count, `${context}.ready_count`),
    delivered: requireBooleanLike(record.delivered, `${context}.delivered`),
    invalid: requireBooleanLike(record.invalid, `${context}.invalid`),
    payloadHash: optionalString(
      record.payload_hash,
      `${context}.payload_hash`,
    ),
    recovered: requireBooleanLike(record.recovered, `${context}.recovered`),
  };
}

function normalizeSumeragiRbcDeliveryStatus(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    height: coerceStatusInt(record.height, `${context}.height`),
    view: coerceStatusInt(record.view, `${context}.view`),
    delivered: requireBooleanLike(record.delivered, `${context}.delivered`),
    present: requireBooleanLike(record.present, `${context}.present`),
    blockHash: optionalString(record.block_hash, `${context}.block_hash`),
    readyCount: coerceStatusInt(record.ready_count, `${context}.ready_count`),
    receivedChunks: coerceStatusInt(
      record.received_chunks,
      `${context}.received_chunks`,
    ),
    totalChunks: coerceStatusInt(record.total_chunks, `${context}.total_chunks`),
  };
}

function normalizeRbcSample(payload, context) {
  const record = ensureRecord(payload, context);
  const blockHash = requireHexString(record.block_hash, `${context}.block_hash`);
  const chunkRoot = requireHexString(record.chunk_root, `${context}.chunk_root`);
  const samplesRaw = Array.isArray(record.samples) ? record.samples : [];
  const samples = samplesRaw.map((entry, index) =>
    normalizeRbcChunkProof(entry, `${context}.samples[${index}]`),
  );
  let payloadHash = null;
  if (record.payload_hash != null) {
    payloadHash = requireHexString(record.payload_hash, `${context}.payload_hash`);
  }
  return {
    blockHash,
    height: coerceStatusInt(record.height, `${context}.height`),
    view: coerceStatusInt(record.view, `${context}.view`),
    totalChunks: coerceStatusInt(record.total_chunks, `${context}.total_chunks`),
    chunkRoot,
    payloadHash,
    samples,
  };
}

function normalizeRbcChunkProof(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    index: ToriiClient._normalizeUnsignedInteger(
      record.index,
      `${context}.index`,
      { allowZero: true },
    ),
    chunkHex: requireHexString(record.chunk_hex, `${context}.chunk_hex`),
    digestHex: requireHexString(record.digest_hex, `${context}.digest_hex`),
    proof: normalizeRbcMerkleProof(record.proof, `${context}.proof`),
  };
}

function normalizeRbcMerkleProof(payload, context) {
  const record = ensureRecord(payload, context);
  const auditPathRaw = Array.isArray(record.audit_path) ? record.audit_path : [];
  const auditPath = auditPathRaw.map((entry, index) => {
    if (entry === null || entry === undefined) {
      return null;
    }
    return requireHexString(entry, `${context}.audit_path[${index}]`);
  });
  let depth = null;
  if (record.depth !== undefined && record.depth !== null) {
    depth = ToriiClient._normalizeUnsignedInteger(
      record.depth,
      `${context}.depth`,
      { allowZero: true },
    );
  }
  return {
    leafIndex: ToriiClient._normalizeUnsignedInteger(
      record.leaf_index,
      `${context}.leaf_index`,
      { allowZero: true },
    ),
    depth,
    auditPath,
  };
}

function buildRbcSampleRequestInternal(session, overrides, context) {
  if (!session || typeof session !== "object") {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: session must be an object`,
      `${context}.session`,
    );
  }
  if (overrides === null || (overrides !== undefined && typeof overrides !== "object")) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}: overrides must be an object`,
      `${context}.overrides`,
    );
  }
  const opts = overrides ?? {};
  const blockHash = requireHexString(session.blockHash, `${context}.blockHash`);
  const height = ToriiClient._normalizeUnsignedInteger(
    session.height,
    `${context}.height`,
    { allowZero: true },
  );
  const view = ToriiClient._normalizeUnsignedInteger(
    session.view,
    `${context}.view`,
    { allowZero: true },
  );
  const request = {
    blockHash,
    height,
    view,
  };
  if (opts.count !== undefined && opts.count !== null) {
    request.count = ToriiClient._normalizeUnsignedInteger(
      opts.count,
      `${context}.count`,
      { allowZero: false },
    );
  }
  if (opts.seed !== undefined && opts.seed !== null) {
    request.seed = ToriiClient._normalizeUnsignedInteger(
      opts.seed,
      `${context}.seed`,
      { allowZero: true },
    );
  }
  if (opts.apiToken !== undefined && opts.apiToken !== null) {
    request.apiToken = String(opts.apiToken);
  }
  return request;
}

function decodeVarintBuffer(buffer, startIndex, context) {
  let value = 0n;
  let shift = 0n;
  let index = startIndex;
  while (index < buffer.length) {
    const byte = BigInt(buffer[index]);
    value |= (byte & 0x7fn) << shift;
    index += 1;
    if ((byte & 0x80n) === 0n) {
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error(`${context} contains an oversized multihash varint`);
      }
      return { value: Number(value), nextIndex: index };
    }
    shift += 7n;
    if (shift > 63n) {
      throw new Error(`${context} contains an invalid multihash varint`);
    }
  }
  throw new Error(`${context} contains a truncated multihash varint`);
}

function decodeIdentifierReceiptPublicKey(value, context) {
  const literal = requireNonEmptyString(value, context).trim();
  let prefixedAlgorithm = null;
  let multihashLiteral = literal;
  const separator = literal.indexOf(":");
  if (separator > 0) {
    prefixedAlgorithm = literal.slice(0, separator).trim().toLowerCase();
    multihashLiteral = literal.slice(separator + 1);
  }
  const canonical = canonicalizeMultihashHex(multihashLiteral, context);
  const bytes = Buffer.from(canonical, "hex");
  const functionCode = decodeVarintBuffer(bytes, 0, context);
  const digestLength = decodeVarintBuffer(bytes, functionCode.nextIndex, context);
  const payload = bytes.subarray(digestLength.nextIndex);
  if (payload.length !== digestLength.value) {
    throw new Error(`${context} multihash payload length does not match its digest header`);
  }
  const entry = getCurveEntryByPublicKeyMulticodec(functionCode.value);
  if (!entry) {
    throw new Error(
      `${context} uses unsupported multihash code 0x${functionCode.value.toString(16)}`,
    );
  }
  const algorithm = entry.algorithm;
  if (
    prefixedAlgorithm &&
    prefixedAlgorithm !== algorithm &&
    !(prefixedAlgorithm === "mldsa" && algorithm === "ml-dsa")
  ) {
    throw new Error(`${context} algorithm prefix does not match the multihash payload`);
  }
  return { algorithm, publicKey: payload };
}

function irohaPrehash(messageBytes) {
  const digest = Buffer.from(blake2b256(messageBytes));
  digest[digest.length - 1] |= 1;
  return digest;
}

function assertIdentifierReceiptPayloadMatchesTopLevel(receipt, context) {
  const payload = receipt.signature_payload;
  const mismatches = [
    ["policy_id", payload.policy_id, receipt.policy_id],
    ["opaque_id", payload.opaque_id, receipt.opaque_id],
    ["receipt_hash", payload.receipt_hash, receipt.receipt_hash],
    ["uaid", payload.uaid, receipt.uaid],
    ["account_id", payload.account_id, receipt.account_id],
    ["resolved_at_ms", payload.resolved_at_ms, receipt.resolved_at_ms],
    ["expires_at_ms", payload.expires_at_ms, receipt.expires_at_ms],
  ].filter(([, payloadValue, topLevelValue]) => payloadValue !== topLevelValue);
  if (mismatches.length > 0) {
    const [field] = mismatches[0];
    throw new Error(`${context} top-level receipt fields do not match signature_payload.${field}`);
  }
}

export function getIdentifierBfvPublicParameters(policySummary) {
  const normalizedPolicy = normalizeIdentifierPolicySummary(
    policySummary,
    "getIdentifierBfvPublicParameters.policy",
  );
  if (normalizedPolicy.input_encryption !== "bfv-v1") {
    return null;
  }
  return normalizedPolicy.input_encryption_public_parameters_decoded ?? null;
}

export function encryptIdentifierInputForPolicy(policySummary, input, options = {}) {
  const { publicParameters, inputBytes, seed } = normalizeIdentifierBfvEncryptionInputs(
    policySummary,
    input,
    options,
  );
  const params = validateIdentifierBfvPublicParameters(
    publicParameters,
    "encryptIdentifierInputForPolicy.policy.input_encryption_public_parameters_decoded",
  );
  const slots = encodeIdentifierSlots(params, inputBytes).map((scalar, index) => {
    const slotSeed = createSha512Digest([
      BFV_IDENTIFIER_SLOT_DOMAIN,
      seed,
      u64ToLittleEndianBuffer(index),
    ]);
    return encryptIdentifierScalar(params, scalar, slotSeed);
  });
  const ciphertext = {
    slots: slots.map((slot) => ({
      c0: slot.c0.map((value) => Number(value)),
      c1: slot.c1.map((value) => Number(value)),
    })),
  };
  return encodeNoritoBfvIdentifierCiphertext(ciphertext).toString("hex");
}

export function buildIdentifierRequestForPolicy(policySummary, options = {}) {
  const normalizedPolicy = normalizeIdentifierPolicySummary(
    policySummary,
    "buildIdentifierRequestForPolicy.policy",
  );
  const record = ensureRecord(options, "buildIdentifierRequestForPolicy options");
  assertSupportedOptionKeys(
    record,
    new Set(["input", "encryptedInput", "encrypt", "seed", "seedHex"]),
    "buildIdentifierRequestForPolicy options",
  );
  const hasInput = record.input !== undefined && record.input !== null;
  const hasEncryptedInput =
    record.encryptedInput !== undefined && record.encryptedInput !== null;
  const encrypt = record.encrypt === true;
  if (hasInput === hasEncryptedInput) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "buildIdentifierRequestForPolicy options must supply exactly one of input or encryptedInput",
      "buildIdentifierRequestForPolicy.input",
    );
  }
  if ((record.seed !== undefined && record.seed !== null) || (record.seedHex !== undefined && record.seedHex !== null)) {
    if (!hasInput || !encrypt) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        "buildIdentifierRequestForPolicy options may only supply seed/seedHex when encrypting plaintext input",
        "buildIdentifierRequestForPolicy.seed",
      );
    }
  }
  if (hasInput) {
    if (encrypt) {
      return {
        policyId: normalizedPolicy.policy_id,
        encryptedInput: encryptIdentifierInputForPolicy(normalizedPolicy, record.input, {
          seed: record.seed,
          seedHex: record.seedHex,
        }),
      };
    }
    return {
      policyId: normalizedPolicy.policy_id,
      input: normalizeIdentifierInput(
        record.input,
        normalizedPolicy.normalization,
        "buildIdentifierRequestForPolicy.input",
      ),
    };
  }
  if (normalizedPolicy.input_encryption !== "bfv-v1") {
    throw new Error(
      `buildIdentifierRequestForPolicy: policy ${normalizedPolicy.policy_id} does not publish BFV encrypted-input support`,
    );
  }
  return {
    policyId: normalizedPolicy.policy_id,
    encryptedInput: requireHexString(
      record.encryptedInput,
      "buildIdentifierRequestForPolicy.encryptedInput",
    ),
  };
}

export function verifyIdentifierResolutionReceipt(receipt, policySummary) {
  const normalizedReceipt = normalizeIdentifierResolveResponse(
    receipt,
    "verifyIdentifierResolutionReceipt.receipt",
  );
  const normalizedPolicy = normalizeIdentifierPolicySummary(
    policySummary,
    "verifyIdentifierResolutionReceipt.policy",
  );
  if (normalizedReceipt.policy_id !== normalizedPolicy.policy_id) {
    throw new Error(
      `verifyIdentifierResolutionReceipt: receipt policy ${normalizedReceipt.policy_id} does not match policy ${normalizedPolicy.policy_id}`,
    );
  }
  assertIdentifierReceiptPayloadMatchesTopLevel(
    normalizedReceipt,
    "verifyIdentifierResolutionReceipt.receipt",
  );
  const signedPayload = Buffer.from(normalizedReceipt.signature_payload_hex, "hex");
  const prehash = irohaPrehash(signedPayload);
  const signature = Buffer.from(normalizedReceipt.signature, "hex");
  const verifyingKey = decodeIdentifierReceiptPublicKey(
    normalizedPolicy.resolver_public_key,
    "verifyIdentifierResolutionReceipt.policy.resolver_public_key",
  );
  switch (verifyingKey.algorithm) {
    case "ed25519":
      return verifyEd25519(prehash, signature, verifyingKey.publicKey);
    case "sm2":
      return verifySm2(
        prehash,
        signature,
        verifyingKey.publicKey,
        SM2_DEFAULT_DISTINGUISHED_ID,
      );
    case "secp256k1":
    case "ml-dsa":
      throw new Error(
        `verifyIdentifierResolutionReceipt: ${verifyingKey.algorithm} verification is not available in the JS SDK without additional native support`,
      );
    default:
      throw new Error(
        `verifyIdentifierResolutionReceipt: unsupported algorithm ${verifyingKey.algorithm}`,
      );
  }
}

export function buildRbcSampleRequest(session, overrides = {}) {
  return buildRbcSampleRequestInternal(
    session,
    overrides,
    "buildRbcSampleRequest",
  );
}

export function buildConnectWebSocketUrl(baseUrl, options = {}) {
  return buildConnectWebSocketUrlInternal(
    baseUrl,
    options,
    "buildConnectWebSocketUrl",
  );
}

export function openConnectWebSocket(options = {}) {
  const normalizedOptions = requirePlainObjectOption(
    options,
    "openConnectWebSocket options",
    { message: "must be an object" },
  );
  return openConnectWebSocketInternal(normalizedOptions, "openConnectWebSocket");
}
