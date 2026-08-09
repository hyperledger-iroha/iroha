import { Buffer } from "buffer";

import { blake2b256 } from "./blake2b.js";
import {
  NumericV1,
  NumericV1Error,
  parseStrictLosslessIntegerJson,
} from "./numericV1.js";
import {
  noritoDecodeBlockProofs,
  noritoEncodeMultisigContractCallApproveRequest,
  noritoEncodeMultisigContractCallProposeRequest,
  noritoEncodeMultisigProposeRequest,
} from "./norito.js";
import { browserSignedTransactionHashHex } from "./transactionCodec.js";
import { buildCanonicalJsonRequest } from "./canonicalRequest.js";
import {
  normalizeKagemushaOperationId,
  normalizeKagemushaOperationReference,
  normalizeKagemushaOperationStatus,
  normalizeKagemushaRedeemRequestV4,
  normalizeOfflineStatus,
  normalizeKagemushaTopUpRequestV4,
  requireKagemushaJsonContentType,
} from "./kagemushaOffline.js";
import { privacyCapabilityTransportV1 } from "./privacyCapabilityTransport.js";
import {
  SUMERAGI_DIAGNOSTICS_TYPED_JSON_MAX_BYTES,
  SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES,
  parseSumeragiDiagnosticsJson,
  parseSumeragiStatusJson,
} from "./sumeragiTyped.js";
import {
  AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
  AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1,
} from "./authenticatedBlockProofs.browser.js";

const DEFAULT_SUCCESS_STATUSES = [200];
const BOUNDED_RESPONSE_MAX_STREAM_CHUNKS = 16_384;
const PRIVACY_CAPABILITIES_JSON_MAX_BYTES = 256 * 1024;
const MAX_UINT64_BIGINT = (1n << 64n) - 1n;
const EXPLORER_CURSOR_DEFAULT_LIMIT = 25;
const EXPLORER_CURSOR_MAX_LIMIT = 100;
const EXPLORER_CURSOR_MAX_LENGTH = 1_424;
const EXPLORER_CURSOR_PATTERN = /^[A-Za-z0-9_-]+$/u;
const EXPLORER_CURSOR_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const PRIVACY_CAPABILITIES_REQUEST_OPTION_KEYS = new Set([
  "headers",
  "signal",
  "successStatuses",
]);
const PIPELINE_SUCCESS_STATUS = "Applied";
const PIPELINE_STATUS_VALUES = new Set([
  "Queued",
  "Approved",
  "Committed",
  PIPELINE_SUCCESS_STATUS,
  "Rejected",
  "Expired",
]);
const PIPELINE_FAILURE_STATUSES = new Set(["Rejected", "Expired"]);
const PIPELINE_STATUS_RESOLUTION_VALUES = new Set(["queue", "cache", "state"]);
const HASH_LITERAL_PATTERN = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u;
const MULTISIG_PROPOSAL_STATUS_VALUES = new Set([
  "COLLECTING_SIGNATURES",
  "FINALIZED",
  "CANCELED",
  "EXPIRED",
]);
const COUNTED_LIST_OPTION_KEYS = new Set([
  "limit",
  "offset",
  "countMode",
  "count_mode",
  "signal",
]);
const ACCOUNT_HISTORY_OPTION_KEYS = new Set([
  ...COUNTED_LIST_OPTION_KEYS,
  "assetId",
  "asset_id",
]);
const CONTRACT_ACTIVITY_OPTION_KEYS = new Set([
  ...COUNTED_LIST_OPTION_KEYS,
  "authority",
  "contractAddress",
  "contract_address",
  "contractAlias",
  "contract_alias",
  "contractEntrypoint",
  "contract_entrypoint",
  "sinceTimestampMs",
  "since_timestamp_ms",
  "untilTimestampMs",
  "until_timestamp_ms",
  "resultOk",
  "result_ok",
]);
const CONTRACT_EVENT_FILTER_OPTION_KEYS = new Set([
  "authority",
  "contractAddress",
  "contract_address",
  "contractAlias",
  "contract_alias",
  "module",
  "eventKind",
  "event_kind",
  "participant",
  "assetId",
  "asset_id",
  "provenance",
  "sinceTimestampMs",
  "since_timestamp_ms",
  "untilTimestampMs",
  "until_timestamp_ms",
  "resultOk",
  "result_ok",
]);
const CONTRACT_EVENT_LIST_OPTION_KEYS = new Set([
  ...COUNTED_LIST_OPTION_KEYS,
  ...CONTRACT_EVENT_FILTER_OPTION_KEYS,
]);
const CONTRACT_EVENT_STREAM_OPTION_KEYS = new Set([
  "signal",
  ...CONTRACT_EVENT_FILTER_OPTION_KEYS,
]);
const LEDGER_HEADERS_OPTION_KEYS = new Set(["from", "limit", "signal"]);
const LEDGER_READ_OPTION_KEYS = new Set(["signal"]);

function normalizeBaseUrl(baseUrl) {
  const raw = String(baseUrl ?? "").trim();
  if (!raw) {
    throw new TypeError("ToriiBrowserClient baseUrl must be a non-empty URL");
  }
  return raw.replace(/\/+$/, "").replace(/\/v1\/explorer$/i, "").replace(/\/v1$/i, "");
}

function appendSearchParams(url, params) {
  if (!params) return;
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue;
    if (typeof value === "string" && value.trim() === "") continue;
    url.searchParams.set(key, String(value));
  }
}

function requireObject(value, context) {
  if (value === undefined || value === null) return {};
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function normalizeTransactionStatusScope(value, context) {
  const scope = value === undefined ? "global" : value;
  if (scope !== "local" && scope !== "global") {
    throw new TypeError(`${context} must be local or global`);
  }
  return scope;
}

function rejectRemovedWaitScope(options, context) {
  if (Object.prototype.hasOwnProperty.call(options, "scope")) {
    throw new TypeError(`${context}.scope is not supported; finality waits are global`);
  }
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${context} must not be empty`);
  }
  return trimmed;
}

function normalizeContractDeploymentStateRequest(value) {
  if (!isPlainObject(value)) {
    throw new TypeError("contract deployment-state request must be a plain object");
  }
  const keys = Object.keys(value).sort();
  if (
    keys.length !== 2 ||
    keys[0] !== "authority" ||
    keys[1] !== "contract_alias"
  ) {
    throw new TypeError(
      "contract deployment-state request requires exactly authority and contract_alias",
    );
  }
  const authority = requireNonEmptyString(
    value.authority,
    "contract deployment-state authority",
  );
  const contractAlias = requireNonEmptyString(
    value.contract_alias,
    "contract deployment-state contract_alias",
  );
  if (authority !== value.authority || contractAlias !== value.contract_alias) {
    throw new TypeError("contract deployment-state identifiers must be exact strings");
  }
  return {
    authority,
    contract_alias: contractAlias,
  };
}

function requireCanonicalDecimalString(value, context, { positive = false } = {}) {
  if (typeof value !== "string" || !/^(?:0|[1-9]\d*)$/u.test(value)) {
    throw new TypeError(`${context} must be a canonical decimal string`);
  }
  if (positive && value === "0") {
    throw new TypeError(`${context} must be positive`);
  }
  return value;
}

function normalizeContractDeploymentStateResponse(value, request) {
  if (!isPlainObject(value)) {
    throw new TypeError("contract deployment-state response must be a plain object");
  }
  const fields = [
    "authority",
    "contract_alias",
    "deploy_nonce",
    "dataspace_alias",
    "dataspace_id",
    "previous_contract_address",
    "observed_block_height",
    "observed_block_hash",
    "ledger_time_ms",
    "chain_discriminant",
  ];
  const keys = Object.keys(value).sort();
  if (keys.length !== fields.length || keys.some((key) => !fields.includes(key))) {
    throw new TypeError(
      "contract deployment-state response has missing or unsupported fields",
    );
  }
  if (value.authority !== request.authority) {
    throw new Error("contract deployment-state response authority mismatch");
  }
  if (value.contract_alias !== request.contract_alias) {
    throw new Error("contract deployment-state response alias mismatch");
  }
  const dataspaceAlias = requireNonEmptyString(
    value.dataspace_alias,
    "contract deployment-state response dataspace_alias",
  );
  if (dataspaceAlias !== value.dataspace_alias) {
    throw new TypeError(
      "contract deployment-state response dataspace_alias must be exact",
    );
  }
  const observedBlockHash = requireNonEmptyString(
    value.observed_block_hash,
    "contract deployment-state response observed_block_hash",
  );
  const hashMatch = HASH_LITERAL_PATTERN.exec(observedBlockHash);
  if (hashMatch === null || hashLiteralCrc16(hashMatch[1]) !== hashMatch[2]) {
    throw new TypeError(
      "contract deployment-state response observed_block_hash must be canonical",
    );
  }
  const previous = value.previous_contract_address;
  if (previous !== null) {
    const exactPrevious = requireNonEmptyString(
      previous,
      "contract deployment-state response previous_contract_address",
    );
    if (exactPrevious !== previous) {
      throw new TypeError(
        "contract deployment-state response previous_contract_address must be exact",
      );
    }
  }
  const chainDiscriminant = requireCanonicalDecimalString(
    value.chain_discriminant,
    "contract deployment-state response chain_discriminant",
  );
  if (BigInt(chainDiscriminant) > 0xffffn) {
    throw new RangeError(
      "contract deployment-state response chain_discriminant exceeds u16",
    );
  }
  return Object.freeze({
    authority: value.authority,
    contract_alias: value.contract_alias,
    deploy_nonce: requireCanonicalDecimalString(
      value.deploy_nonce,
      "contract deployment-state response deploy_nonce",
    ),
    dataspace_alias: dataspaceAlias,
    dataspace_id: requireCanonicalDecimalString(
      value.dataspace_id,
      "contract deployment-state response dataspace_id",
    ),
    previous_contract_address: previous,
    observed_block_height: requireCanonicalDecimalString(
      value.observed_block_height,
      "contract deployment-state response observed_block_height",
      { positive: true },
    ),
    observed_block_hash: observedBlockHash,
    ledger_time_ms: requireCanonicalDecimalString(
      value.ledger_time_ms,
      "contract deployment-state response ledger_time_ms",
    ),
    chain_discriminant: chainDiscriminant,
  });
}

function requireExactHashHex(value, context) {
  if (typeof value !== "string" || !/^[0-9a-fA-F]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be an exact 32-byte hexadecimal string`);
  }
  return value.toLowerCase();
}

function hashLiteralCrc16(body) {
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "utf8")) {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}

function requireMatchingReceiptHashHeader(response, name, expectedHash) {
  const literal = response.headers.get(name);
  if (literal === null) return;
  const match = HASH_LITERAL_PATTERN.exec(literal);
  if (match === null || hashLiteralCrc16(match[1]) !== match[2]) {
    throw new Error(`${name} must be a canonical Iroha hash literal`);
  }
  if (match[1].toLowerCase() !== expectedHash) {
    throw new Error(`${name} does not match the locally signed transaction`);
  }
}

function innerSignedTransactionHashHex(versionedSignedTransaction) {
  // Torii's signed-transaction field hashes the unversioned inner wire body.
  // The public pipeline identity instead hashes its External entrypoint wrapper.
  const digest = Buffer.from(blake2b256(versionedSignedTransaction.subarray(1)));
  digest[digest.length - 1] |= 1;
  return digest.toString("hex");
}

function requireTransactionBytes(value, context) {
  let bytes;
  if (value instanceof Uint8Array) {
    bytes = new Uint8Array(value);
  } else if (ArrayBuffer.isView(value)) {
    bytes = new Uint8Array(
      value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength),
    );
  } else if (value instanceof ArrayBuffer) {
    bytes = new Uint8Array(value.slice(0));
  } else {
    throw new TypeError(`${context} must be transaction bytes`);
  }
  if (bytes.length < 2 || bytes[0] !== 1) {
    throw new TypeError(
      `${context} must be an exact version-1 signed transaction payload`,
    );
  }
  return bytes;
}

function classifyGlobalPipelineStatusEnvelope(value, requestedHash, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be a pipeline status object`);
  }
  const hash = requireExactHashHex(value.hash, `${context}.hash`);
  if (hash !== requestedHash) {
    throw new Error(`${context}.hash does not match the requested transaction`);
  }
  if (value.scope !== "global") {
    throw new Error(`${context}.scope must be global`);
  }
  if (typeof value.summary !== "string") {
    throw new TypeError(`${context}.summary must be a string`);
  }
  if (!isPlainObject(value.status) || typeof value.status.kind !== "string") {
    throw new TypeError(`${context}.status.kind must be a string`);
  }
  if (!PIPELINE_STATUS_VALUES.has(value.status.kind)) {
    throw new Error(`${context}.status.kind is not a current pipeline status`);
  }
  if (!PIPELINE_STATUS_RESOLUTION_VALUES.has(value.resolved_from)) {
    throw new Error(`${context}.resolved_from is not a current status source`);
  }
  const kind = value.status.kind;
  if (kind === PIPELINE_SUCCESS_STATUS) {
    const blockHeight = value.status.block_height;
    if (!Number.isSafeInteger(blockHeight) || blockHeight < 1) {
      throw new Error(
        `${context}.status.block_height must be a positive safe integer`,
      );
    }
    if (value.resolved_from !== "cache" && value.resolved_from !== "state") {
      throw new Error(
        `${context} Applied status must be cache- or state-resolved`,
      );
    }
  } else if (PIPELINE_FAILURE_STATUSES.has(kind)) {
    if (value.resolved_from !== "cache" && value.resolved_from !== "state") {
      throw new Error(
        `${context} terminal failure must be cache- or state-resolved`,
      );
    }
  }
  return {
    kind,
    authoritative: value.resolved_from === "state",
  };
}

function abortError() {
  if (typeof DOMException === "function") {
    return new DOMException("The operation was aborted", "AbortError");
  }
  const error = new Error("The operation was aborted");
  error.name = "AbortError";
  return error;
}

function throwIfAborted(signal) {
  if (signal?.aborted) throw signal.reason ?? abortError();
}

function delayWithSignal(milliseconds, signal) {
  throwIfAborted(signal);
  if (milliseconds === 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    const onAbort = () => {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", onAbort);
      reject(signal.reason ?? abortError());
    };
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}

function requireCanonicalQuantity(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical Kotodama V1 quantity string`);
  }
  try {
    return NumericV1.decodeQuantityJson(value).toString();
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new TypeError(
      `${context} must be a canonical non-negative Kotodama V1 quantity (${error.code})`,
    );
  }
}

function normalizeQuantityRecord(value, context, fields, { optional = false } = {}) {
  const record = requireObject(value, context);
  const normalized = { ...record };
  for (const field of fields) {
    if (normalized[field] === undefined || normalized[field] === null) {
      if (!optional) {
        throw new TypeError(`${context}.${field} must be a canonical Kotodama V1 quantity string`);
      }
    } else {
      normalized[field] = requireCanonicalQuantity(
        normalized[field],
        `${context}.${field}`,
      );
    }
  }
  return normalized;
}

function normalizeQuantityPage(value, context, fields, options) {
  const page = requireObject(value, context);
  if (!Array.isArray(page.items)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  return {
    ...page,
    items: page.items.map((item, index) =>
      normalizeQuantityRecord(item, `${context}.items[${index}]`, fields, options),
    ),
  };
}

function normalizeExplorerAssetDefinitionRecord(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const fields = [
    "id",
    "owning_domain",
    "mintable",
    "logo",
    "metadata",
    "owned_by",
    "assets",
    "total_quantity",
    "locked_quantity",
    "circulating_quantity",
  ];
  const keys = Object.keys(value);
  if (keys.length !== fields.length || keys.some((key) => !fields.includes(key))) {
    throw new TypeError(`${context} has missing or unsupported fields`);
  }
  for (const field of ["id", "mintable", "owned_by"]) {
    const normalized = requireNonEmptyString(value[field], `${context}.${field}`);
    if (normalized !== value[field]) {
      throw new TypeError(`${context}.${field} must be an exact string`);
    }
  }
  if (value.owning_domain !== null) {
    const owningDomain = requireNonEmptyString(
      value.owning_domain,
      `${context}.owning_domain`,
    );
    if (owningDomain !== value.owning_domain) {
      throw new TypeError(`${context}.owning_domain must be an exact string or null`);
    }
  }
  for (const field of ["logo", "locked_quantity", "circulating_quantity"]) {
    if (value[field] !== null && typeof value[field] !== "string") {
      throw new TypeError(`${context}.${field} must be a string or null`);
    }
  }
  if (!Number.isInteger(value.assets) || value.assets < 0 || value.assets > 0xffff_ffff) {
    throw new TypeError(`${context}.assets must be a uint32`);
  }
  if (!isPlainObject(value.metadata)) {
    throw new TypeError(`${context}.metadata must be an object`);
  }
  const normalized = normalizeQuantityRecord(value, context, ["total_quantity"]);
  for (const field of ["locked_quantity", "circulating_quantity"]) {
    if (normalized[field] !== null) {
      normalized[field] = requireCanonicalQuantity(
        normalized[field],
        `${context}.${field}`,
      );
    }
  }
  return normalized;
}

function normalizeExplorerCursor(value, context, { nullable = false } = {}) {
  if (value === null && nullable) return null;
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty base64url string`);
  }
  const remainder = value.length % 4;
  const trailingSextet = EXPLORER_CURSOR_ALPHABET.indexOf(value[value.length - 1]);
  const hasNonCanonicalTrailingBits =
    (remainder === 2 && (trailingSextet & 0x0f) !== 0) ||
    (remainder === 3 && (trailingSextet & 0x03) !== 0);
  if (
    value.length > EXPLORER_CURSOR_MAX_LENGTH ||
    remainder === 1 ||
    !EXPLORER_CURSOR_PATTERN.test(value) ||
    hasNonCanonicalTrailingBits
  ) {
    throw new TypeError(
      `${context} must be canonical base64url without padding and at most ${EXPLORER_CURSOR_MAX_LENGTH} characters`,
    );
  }
  return value;
}

function requireExactExplorerCursorFields(record, expectedFields, context) {
  const expected = new Set(expectedFields);
  const unknown = Object.keys(record).find((field) => !expected.has(field));
  if (unknown !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknown}`);
  }
  const missing = expectedFields.find(
    (field) => !Object.prototype.hasOwnProperty.call(record, field),
  );
  if (missing !== undefined) {
    throw new TypeError(`${context} is missing required field ${missing}`);
  }
  return record;
}

function normalizeExplorerCursorMeta(value, context) {
  const meta = requireObject(value, context);
  requireExactExplorerCursorFields(meta, ["limit", "next_cursor", "has_more"], context);
  const limit = normalizePositiveInteger(meta.limit, `${context}.limit`, undefined);
  if (limit === undefined || limit > EXPLORER_CURSOR_MAX_LIMIT) {
    throw new TypeError(`${context}.limit must be between 1 and ${EXPLORER_CURSOR_MAX_LIMIT}`);
  }
  if (typeof meta.has_more !== "boolean") {
    throw new TypeError(`${context}.has_more must be a boolean`);
  }
  if (meta.next_cursor === undefined) {
    throw new TypeError(`${context}.next_cursor must be a string or null`);
  }
  const nextCursor = normalizeExplorerCursor(meta.next_cursor, `${context}.next_cursor`, {
    nullable: true,
  });
  if (meta.has_more !== (nextCursor !== null)) {
    throw new TypeError(`${context}.has_more must match next_cursor availability`);
  }
  return { limit, next_cursor: nextCursor, has_more: meta.has_more };
}

function normalizeExplorerCursorPage(value, context, normalizeItem = (item) => item) {
  const page = requireObject(value, context);
  requireExactExplorerCursorFields(page, ["pagination", "items"], context);
  if (!Array.isArray(page.items)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  const pagination = normalizeExplorerCursorMeta(page.pagination, `${context}.pagination`);
  if (page.items.length > pagination.limit) {
    throw new TypeError(`${context}.items must not exceed pagination.limit`);
  }
  return {
    pagination,
    items: page.items.map((item, index) => normalizeItem(item, index)),
  };
}

function normalizePositiveInteger(value, context, fallback) {
  if (value === undefined || value === null) return fallback;
  const numeric = Number(value);
  if (!Number.isSafeInteger(numeric) || numeric < 1) {
    throw new TypeError(`${context} must be a positive safe integer`);
  }
  return numeric;
}

function normalizeOffset(value, context, fallback = 0) {
  if (value === undefined || value === null) return fallback;
  const numeric = Number(value);
  if (!Number.isSafeInteger(numeric) || numeric < 0) {
    throw new TypeError(`${context} must be a non-negative safe integer`);
  }
  return numeric;
}

function normalizeBoolean(value, context) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return value;
}

function normalizeExplorerPagination(options, context) {
  const page = normalizePositiveInteger(options.page, `${context}.page`, 1);
  const perPage = normalizePositiveInteger(
    options.perPage ?? options.per_page,
    `${context}.perPage`,
    25,
  );
  return { page, per_page: perPage };
}

function normalizeExplorerCursorPagination(options, context) {
  for (const removed of ["page", "perPage", "per_page", "offset", "pageSize"]) {
    if (Object.prototype.hasOwnProperty.call(options, removed)) {
      throw new TypeError(
        `${context}.${removed} is not supported; use cursor and limit`,
      );
    }
  }
  const limit = normalizePositiveInteger(
    options.limit,
    `${context}.limit`,
    EXPLORER_CURSOR_DEFAULT_LIMIT,
  );
  if (limit > EXPLORER_CURSOR_MAX_LIMIT) {
    throw new TypeError(`${context}.limit must be between 1 and ${EXPLORER_CURSOR_MAX_LIMIT}`);
  }
  const params = { limit };
  if (options.cursor !== undefined && options.cursor !== null) {
    params.cursor = normalizeExplorerCursor(options.cursor, `${context}.cursor`);
  }
  return params;
}

function normalizeIterablePagination(options, context) {
  const params = {};
  if (options.limit !== undefined && options.limit !== null) {
    params.limit = normalizePositiveInteger(options.limit, `${context}.limit`, undefined);
  }
  if (options.offset !== undefined && options.offset !== null) {
    params.offset = normalizeOffset(options.offset, `${context}.offset`);
  }
  return params;
}

function normalizeTransactionQuerySort(sort) {
  if (sort === undefined || sort === null) {
    return [];
  }
  if (typeof sort === "string") {
    const normalized = sort.trim().toLowerCase();
    if (normalized === "newest") {
      return [
        { key: "timestamp_ms", order: "desc" },
        { key: "entrypoint_hash", order: "desc" },
      ];
    }
    if (normalized === "oldest") {
      return [
        { key: "timestamp_ms", order: "asc" },
        { key: "entrypoint_hash", order: "asc" },
      ];
    }
    return normalized
      .split(",")
      .map((token) => token.trim())
      .filter(Boolean)
      .map((token) => {
        const parts = token.split(":");
        if (parts.length > 2) {
          throw new TypeError("sort entries must use key or key:asc/key:desc form");
        }
        const [key, order = "asc"] = parts;
        return {
          key: normalizeQueryFieldName(requireNonEmptyString(key, "sort key"), "sort key"),
          order: normalizeSortOrder(order, "sort order"),
        };
      });
  }
  if (Array.isArray(sort)) {
    return sort.map((entry, index) => {
      const item = requireObject(entry, `sort[${index}]`);
      return {
        key: normalizeQueryFieldName(requireNonEmptyString(item.key, `sort[${index}].key`), `sort[${index}].key`),
        order: normalizeSortOrder(item.order ?? "asc", `sort[${index}].order`),
      };
    });
  }
  throw new TypeError("sort must be a string or array");
}

function normalizeQueryFieldName(value, context) {
  const field = requireNonEmptyString(value, context);
  if (!/^[A-Za-z_][A-Za-z0-9_.-]*$/.test(field)) {
    throw new TypeError(`${context} must be an ASCII field name`);
  }
  return field;
}

function normalizeSortOrder(value, context) {
  const order = requireNonEmptyString(String(value ?? ""), context).toLowerCase();
  if (order !== "asc" && order !== "desc") {
    throw new TypeError(`${context} must be asc or desc`);
  }
  return order;
}

function normalizeCountMode(value, context) {
  if (value === undefined || value === null) {
    return undefined;
  }
  const mode = requireNonEmptyString(String(value), context).toLowerCase();
  if (mode !== "bounded" && mode !== "exact") {
    throw new TypeError(`${context} must be bounded or exact`);
  }
  return mode;
}

function requireSupportedOptions(value, context, supportedKeys) {
  const options = requireObject(value, context);
  const unsupported = Object.keys(options).find((key) => !supportedKeys.has(key));
  if (unsupported !== undefined) {
    throw new TypeError(`${context} contains unsupported option ${unsupported}`);
  }
  return options;
}

function optionAlias(options, camelCase, snakeCase) {
  return options[camelCase] ?? options[snakeCase];
}

function normalizeCountedListParams(options, context) {
  return {
    ...normalizeIterablePagination(options, context),
    count_mode: normalizeCountMode(
      optionAlias(options, "countMode", "count_mode"),
      `${context}.countMode`,
    ),
  };
}

function normalizeOptionalString(value, context) {
  if (value === undefined || value === null) return undefined;
  return requireNonEmptyString(value, context);
}

function normalizeOptionalUnsignedInteger(value, context) {
  if (value === undefined || value === null) return undefined;
  return normalizeOffset(value, context);
}

function normalizeOptionalBoolean(value, context) {
  if (value === undefined || value === null) return undefined;
  return normalizeBoolean(value, context);
}

function normalizeLedgerHeight(value, context) {
  let integer;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(
        `${context} must be a positive safe integer number or an exact decimal string/bigint`,
      );
    }
    integer = BigInt(value);
  } else if (typeof value === "bigint") {
    integer = value;
  } else if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^[0-9]+$/u.test(trimmed)) {
      throw new TypeError(`${context} must be a positive decimal integer`);
    }
    integer = BigInt(trimmed);
  } else {
    throw new TypeError(`${context} must be a positive decimal integer`);
  }
  if (integer <= 0n) {
    throw new TypeError(`${context} must be a positive decimal integer`);
  }
  if (integer > MAX_UINT64_BIGINT) {
    throw new RangeError(
      `${context} must not exceed ${MAX_UINT64_BIGINT.toString(10)}`,
    );
  }
  return integer.toString(10);
}

function normalizeLedgerEntryHash(value, context) {
  const literal = requireNonEmptyString(String(value), context);
  const normalized = literal.startsWith("0x") ? literal.slice(2) : literal;
  if (!/^[0-9a-fA-F]{64}$/u.test(normalized)) {
    throw new TypeError(`${context} must be exactly 32 bytes of hexadecimal`);
  }
  return normalized.toLowerCase();
}

function normalizeContractEventFilterParams(options, context) {
  const provenance = normalizeOptionalString(options.provenance, `${context}.provenance`);
  if (provenance !== undefined && provenance !== "emitted" && provenance !== "derived") {
    throw new TypeError(`${context}.provenance must be emitted or derived`);
  }
  return {
    authority: normalizeOptionalString(options.authority, `${context}.authority`),
    contract_address: normalizeOptionalString(
      optionAlias(options, "contractAddress", "contract_address"),
      `${context}.contractAddress`,
    ),
    contract_alias: normalizeOptionalString(
      optionAlias(options, "contractAlias", "contract_alias"),
      `${context}.contractAlias`,
    ),
    module: normalizeOptionalString(options.module, `${context}.module`),
    event_kind: normalizeOptionalString(
      optionAlias(options, "eventKind", "event_kind"),
      `${context}.eventKind`,
    ),
    participant: normalizeOptionalString(options.participant, `${context}.participant`),
    asset_id: normalizeOptionalString(
      optionAlias(options, "assetId", "asset_id"),
      `${context}.assetId`,
    ),
    provenance,
    since_timestamp_ms: normalizeOptionalUnsignedInteger(
      optionAlias(options, "sinceTimestampMs", "since_timestamp_ms"),
      `${context}.sinceTimestampMs`,
    ),
    until_timestamp_ms: normalizeOptionalUnsignedInteger(
      optionAlias(options, "untilTimestampMs", "until_timestamp_ms"),
      `${context}.untilTimestampMs`,
    ),
    result_ok: normalizeOptionalBoolean(
      optionAlias(options, "resultOk", "result_ok"),
      `${context}.resultOk`,
    ),
  };
}

function normalizeSelectEntry(entry, context) {
  if (typeof entry === "string") {
    const fieldPath = entry.trim();
    if (!fieldPath) {
      throw new TypeError(`${context} must be a non-empty field path`);
    }
    return fieldPath;
  }
  if (isPlainObject(entry)) {
    return entry;
  }
  throw new TypeError(`${context} must be a field-path string or plain object`);
}

function transactionFilter(op, field, value) {
  return { op, args: [field, value] };
}

function normalizeTransactionQueryEnvelope(options, context) {
  const opts = requireObject(options, `${context} options`);
  const pagination = normalizeIterablePagination(opts, `${context} options`);
  const filters = [];
  if (opts.filter !== undefined && opts.filter !== null) {
    filters.push(requireObject(opts.filter, `${context}.filter`));
  }
  if (opts.assetId !== undefined && opts.assetId !== null) {
    filters.push(transactionFilter("eq", "asset_id", requireNonEmptyString(opts.assetId, "assetId")));
  }
  if (opts.authority !== undefined && opts.authority !== null) {
    filters.push(transactionFilter("eq", "authority", requireNonEmptyString(opts.authority, "authority")));
  }
  if (opts.resultOk !== undefined && opts.resultOk !== null) {
    filters.push(transactionFilter("eq", "result_ok", normalizeBoolean(opts.resultOk, "resultOk")));
  }
  if (opts.sinceTimestampMs !== undefined && opts.sinceTimestampMs !== null) {
    filters.push(transactionFilter("gte", "timestamp_ms", normalizeOffset(opts.sinceTimestampMs, "sinceTimestampMs")));
  }
  if (opts.untilTimestampMs !== undefined && opts.untilTimestampMs !== null) {
    filters.push(transactionFilter("lte", "timestamp_ms", normalizeOffset(opts.untilTimestampMs, "untilTimestampMs")));
  }
  const envelope = {
    pagination,
    sort: normalizeTransactionQuerySort(opts.sort),
  };
  if (filters.length === 1) {
    envelope.filter = filters[0];
  } else if (filters.length > 1) {
    envelope.filter = { op: "and", args: filters };
  }
  if (opts.fetch_size !== undefined && opts.fetch_size !== null) {
    envelope.fetch_size = normalizePositiveInteger(opts.fetch_size, "fetch_size", undefined);
  }
  const countMode = normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode");
  if (countMode !== undefined) {
    envelope.count_mode = countMode;
  }
  const queryName = opts.queryName ?? opts.query_name;
  if (queryName !== undefined && queryName !== null) {
    envelope.query = requireNonEmptyString(queryName, "queryName");
  }
  if (opts.select !== undefined && opts.select !== null) {
    if (!Array.isArray(opts.select)) {
      throw new TypeError("select must be an array");
    }
    envelope.select = opts.select.map((entry, index) =>
      normalizeSelectEntry(entry, `select[${index}]`),
    );
  }
  return envelope;
}

function signalFrom(options) {
  return options.signal === undefined ? undefined : options.signal;
}

function signalOnlyOptions(options, context) {
  const item = requireObject(options, context);
  const unknown = Object.keys(item).filter((key) => key !== "signal");
  if (unknown.length > 0) {
    throw new TypeError(`${context} contains unsupported option ${unknown[0]}`);
  }
  return item;
}

function kagemushaOptions(options, context) {
  return signalOnlyOptions(options, context);
}

function copyRequestFields(source) {
  const body = { ...source };
  delete body.signal;
  delete body.headers;
  delete body.successStatuses;
  return body;
}

function normalizeMultisigSelectorBody(value, context) {
  const source = requireObject(value, context);
  const body = copyRequestFields(source);
  if (
    source.multisigAccountId !== undefined &&
    body.multisig_account_id !== undefined
  ) {
    throw new TypeError(`${context} must not duplicate multisigAccountId`);
  }
  if (
    source.multisigAccountAlias !== undefined &&
    body.multisig_account_alias !== undefined
  ) {
    throw new TypeError(`${context} must not duplicate multisigAccountAlias`);
  }
  if (source.multisigAccountId !== undefined && body.multisig_account_id === undefined) {
    body.multisig_account_id = requireNonEmptyString(
      source.multisigAccountId,
      `${context}.multisigAccountId`,
    );
  }
  if (source.multisigAccountAlias !== undefined && body.multisig_account_alias === undefined) {
    body.multisig_account_alias = requireNonEmptyString(
      source.multisigAccountAlias,
      `${context}.multisigAccountAlias`,
    );
  }
  delete body.multisigAccountId;
  delete body.multisigAccountAlias;
  if (body.multisig_account_id !== undefined) {
    body.multisig_account_id = requireNonEmptyString(
      body.multisig_account_id,
      `${context}.multisig_account_id`,
    );
  }
  if (body.multisig_account_alias !== undefined) {
    body.multisig_account_alias = requireNonEmptyString(
      body.multisig_account_alias,
      `${context}.multisig_account_alias`,
    );
  }
  const hasAccountId = body.multisig_account_id !== undefined;
  const hasAccountAlias = body.multisig_account_alias !== undefined;
  if (hasAccountId === hasAccountAlias) {
    throw new TypeError(
      `${context} requires exactly one of multisigAccountId or multisigAccountAlias`,
    );
  }
  return body;
}

function normalizeMultisigProposalsQueryBody(value, context) {
  const source = requireObject(value, context);
  const body = normalizeMultisigSelectorBody(source, context);
  if (source.status !== undefined) {
    if (!Array.isArray(source.status)) {
      throw new TypeError(`${context}.status must be an array`);
    }
    body.status = source.status.map((value, index) => {
      const status = requireNonEmptyString(value, `${context}.status[${index}]`).toUpperCase();
      if (!MULTISIG_PROPOSAL_STATUS_VALUES.has(status)) {
        throw new TypeError(
          `${context}.status[${index}] must be one of ${[
            ...MULTISIG_PROPOSAL_STATUS_VALUES,
          ].join(", ")}`,
        );
      }
      return status;
    });
  }
  if (body.cursor !== undefined && body.cursor !== null) {
    body.cursor = requireNonEmptyString(body.cursor, `${context}.cursor`);
  }
  if (body.limit !== undefined && body.limit !== null) {
    body.limit = normalizePositiveInteger(body.limit, `${context}.limit`, undefined);
  }
  return body;
}

function normalizeMultisigProposalsResolveBody(value, context) {
  const source = requireObject(value, context);
  const body = normalizeMultisigSelectorBody(source, context);
  if (source.proposalId !== undefined && body.proposal_id === undefined) {
    body.proposal_id = source.proposalId;
  }
  if (source.instructionsHash !== undefined && body.instructions_hash === undefined) {
    body.instructions_hash = source.instructionsHash;
  }
  delete body.proposalId;
  delete body.instructionsHash;
  if (body.proposal_id !== undefined) {
    body.proposal_id = requireNonEmptyString(body.proposal_id, `${context}.proposal_id`);
  }
  if (body.instructions_hash !== undefined) {
    body.instructions_hash = requireNonEmptyString(
      body.instructions_hash,
      `${context}.instructions_hash`,
    );
  }
  const hasProposalId = body.proposal_id !== undefined;
  const hasInstructionsHash = body.instructions_hash !== undefined;
  if (hasProposalId === hasInstructionsHash) {
    throw new TypeError(
      `${context} requires exactly one of proposalId or instructionsHash`,
    );
  }
  return body;
}

function responseStatus(response) {
  if (typeof response?.status === "number") return response.status;
  return response?.ok === true ? 200 : 0;
}

async function responseText(response) {
  if (typeof response?.text === "function") {
    return response.text().catch(() => "");
  }
  if (typeof response?.json === "function") {
    try {
      return JSON.stringify(await response.json());
    } catch {
      return "";
    }
  }
  return "";
}

function requireExactJsonContentType(contentType, context) {
  const mediaType = typeof contentType === "string"
    ? contentType.split(";", 1)[0].trim()
    : "";
  if (mediaType.toLowerCase() !== "application/json") {
    throw new TypeError(`${context} must use the application/json media type`);
  }
}

async function readBoundedResponseBytes(response, maximumBodyBytes, context) {
  if (!Number.isSafeInteger(maximumBodyBytes) || maximumBodyBytes < 0) {
    throw new TypeError(`${context} response byte-size bound is invalid`);
  }
  const rawContentLength = response?.headers?.get?.("content-length");
  let declaredLength = null;
  if (rawContentLength !== null && rawContentLength !== undefined) {
    if (
      typeof rawContentLength !== "string"
      || !/^(?:0|[1-9][0-9]*)$/u.test(rawContentLength)
    ) {
      throw new TypeError(
        `${context} Content-Length must be a canonical unsigned decimal integer`,
      );
    }
    declaredLength = Number(rawContentLength);
    if (
      !Number.isSafeInteger(declaredLength)
      || declaredLength > maximumBodyBytes
    ) {
      throw new RangeError(
        `${context} exceeds its ${maximumBodyBytes}-byte response limit`,
      );
    }
  }
  if (typeof response?.body?.getReader !== "function") {
    throw new TypeError(
      `${context} requires a byte-stream response body so its size can be bounded`,
    );
  }

  const reader = response.body.getReader();
  const chunks = [];
  let totalBytes = 0;
  let complete = false;
  try {
    while (true) {
      const result = await reader.read();
      if (!result || typeof result.done !== "boolean") {
        throw new TypeError(`${context} returned an invalid response stream result`);
      }
      if (result.done) {
        complete = true;
        break;
      }
      if (!(result.value instanceof Uint8Array)) {
        throw new TypeError(`${context} returned a non-byte response stream chunk`);
      }
      if (chunks.length >= BOUNDED_RESPONSE_MAX_STREAM_CHUNKS) {
        throw new RangeError(`${context} returned too many fragmented response chunks`);
      }
      if (result.value.byteLength > maximumBodyBytes - totalBytes) {
        throw new RangeError(
          `${context} exceeds its ${maximumBodyBytes}-byte response limit`,
        );
      }
      totalBytes += result.value.byteLength;
      chunks.push(new Uint8Array(result.value));
    }
  } catch (error) {
    if (!complete && typeof reader.cancel === "function") {
      try {
        await reader.cancel(error);
      } catch {
        // Preserve the deterministic validation failure.
      }
    }
    throw error;
  } finally {
    if (typeof reader.releaseLock === "function") reader.releaseLock();
  }

  const bytes = new Uint8Array(totalBytes);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  if (declaredLength !== null && declaredLength !== totalBytes) {
    throw new TypeError(`${context} Content-Length does not match the response body`);
  }
  return bytes;
}

async function readBoundedResponseText(response, maximumBodyBytes, context) {
  const bytes = await readBoundedResponseBytes(response, maximumBodyBytes, context);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    throw new TypeError(`${context} must be valid UTF-8`, { cause: error });
  }
}

export class ToriiBrowserHttpError extends Error {
  constructor(response, bodyText, status = responseStatus(response)) {
    super(`Torii request failed with status ${status}`);
    this.name = "ToriiBrowserHttpError";
    this.response = response;
    this.status = status;
    this.bodyText = bodyText;
  }
}

export class ToriiBrowserStreamGapError extends Error {
  constructor(message, options = {}) {
    super(message);
    this.name = "ToriiBrowserStreamGapError";
    this.code = options.code ?? "stream_gap";
    this.droppedMessages = options.droppedMessages ?? null;
    this.replayAvailable = options.replayAvailable === true;
    this.payload = options.payload ?? null;
  }
}

function streamRequestHeaders(defaultHeaders) {
  const headers = {};
  for (const [name, value] of Object.entries(defaultHeaders)) {
    const normalizedName = name.toLowerCase();
    if (normalizedName === "accept" || normalizedName === "last-event-id") continue;
    headers[name] = value;
  }
  headers.Accept = "text/event-stream";
  return headers;
}

function parseSseEventFrame(rawFrame) {
  let event = null;
  let id = null;
  let retry = null;
  const dataLines = [];
  for (const line of rawFrame.split(/\r\n|\r|\n/u)) {
    if (line === "" || line.startsWith(":")) continue;
    const separator = line.indexOf(":");
    const field = separator === -1 ? line : line.slice(0, separator);
    let value = separator === -1 ? "" : line.slice(separator + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "event") {
      event = value || null;
    } else if (field === "data") {
      dataLines.push(value);
    } else if (field === "id" && !value.includes("\0")) {
      id = value || null;
    } else if (field === "retry" && /^\d+$/u.test(value)) {
      const parsed = Number(value);
      if (Number.isSafeInteger(parsed)) retry = parsed;
    }
  }
  if (dataLines.length === 0 && event === null && id === null) return null;
  const raw = dataLines.length > 0 ? dataLines.join("\n") : null;
  let data = raw ?? "";
  if (raw !== null && raw.trim() !== "") {
    try {
      data = JSON.parse(raw);
    } catch {
      data = raw;
    }
  }
  return { event, data, id, retry, raw };
}

function extractSseFrames(buffer) {
  const frames = [];
  let remainder = buffer;
  while (true) {
    const boundary = /\r\n\r\n|\r\r|\n\n/u.exec(remainder);
    if (boundary === null) break;
    const parsed = parseSseEventFrame(remainder.slice(0, boundary.index));
    if (parsed !== null) frames.push(parsed);
    remainder = remainder.slice(boundary.index + boundary[0].length);
  }
  return { frames, remainder };
}

function streamGapFromEvent(event) {
  const payload = isPlainObject(event.data) ? event.data : null;
  const code =
    typeof payload?.code === "string" && payload.code.trim() !== ""
      ? payload.code
      : "stream_error";
  const message =
    typeof payload?.message === "string" && payload.message.trim() !== ""
      ? payload.message
      : "The contract event stream reported a non-replayable gap.";
  const droppedMessages =
    Number.isSafeInteger(payload?.dropped_messages) && payload.dropped_messages >= 0
      ? payload.dropped_messages
      : null;
  const replayAvailable = payload?.replay_available === true;
  return new ToriiBrowserStreamGapError(message, {
    code,
    droppedMessages,
    replayAvailable,
    payload: payload === null
      ? null
      : {
          code,
          message,
          dropped_messages: droppedMessages,
          replay_available: replayAvailable,
        },
  });
}

export class ToriiBrowserClient {
  constructor(baseUrl, options = {}) {
    const normalizedOptions = requireObject(options, "ToriiBrowserClient options");
    this.baseUrl = normalizeBaseUrl(baseUrl);
    this.fetchImpl = normalizedOptions.fetchImpl ?? globalThis.fetch?.bind(globalThis);
    if (typeof this.fetchImpl !== "function") {
      throw new TypeError("ToriiBrowserClient requires a fetch implementation");
    }
    this.defaultHeaders = {
      ...(normalizedOptions.config?.toriiClient?.defaultHeaders ?? {}),
      ...(normalizedOptions.defaultHeaders ?? {}),
    };
    this.timeoutMs =
      normalizedOptions.config?.toriiClient?.timeoutMs ?? normalizedOptions.timeoutMs ?? null;
  }

  getOfflineCapability(options = {}) {
    const opts = kagemushaOptions(options, "getOfflineCapability options");
    return this._json("GET", "/v1/offline/readiness", {
      signal: opts.signal,
      responseObserver: (response) => requireKagemushaJsonContentType(
        response.headers.get("content-type"),
        "Offline capability response",
      ),
    }).then((payload) => normalizeOfflineStatus(payload));
  }

  submitKagemushaTopUpV4(request, options = {}) {
    return this._submitKagemushaCommandV4(
      "/v1/offline/top-up",
      "top_up",
      request,
      options,
      "submitKagemushaTopUpV4",
    );
  }

  submitKagemushaRedeemV4(request, options = {}) {
    return this._submitKagemushaCommandV4(
      "/v1/offline/redeem",
      "redeem",
      request,
      options,
      "submitKagemushaRedeemV4",
    );
  }

  getKagemushaOperationStatus(operationId, options = {}) {
    const canonicalId = normalizeKagemushaOperationId(operationId);
    const opts = kagemushaOptions(options, "getKagemushaOperationStatus options");
    return this._json("GET", `/v1/offline/operations/${canonicalId}`, {
      signal: opts.signal,
      responseObserver: (response) => requireKagemushaJsonContentType(
        response.headers.get("content-type"),
        "Kagemusha operation status response",
      ),
    }).then((payload) => normalizeKagemushaOperationStatus(payload, canonicalId));
  }

  _submitKagemushaCommandV4(path, kind, request, options, context) {
    const normalizeRequest = kind === "top_up"
      ? normalizeKagemushaTopUpRequestV4
      : normalizeKagemushaRedeemRequestV4;
    const normalized = normalizeRequest(request, `${context} request`);
    const opts = kagemushaOptions(options, `${context} options`);
    let location = null;
    return this._json("POST", path, {
      rawBody: normalized.norito,
      contentType: "application/x-norito",
      headers: {
        Accept: "application/json",
        "Idempotency-Key": normalized.operationId,
      },
      signal: opts.signal,
      successStatuses: [202],
      responseObserver: (response) => {
        requireKagemushaJsonContentType(
          response.headers.get("content-type"),
          "Kagemusha operation reference response",
        );
        location = response.headers.get("location");
      },
    }).then((payload) => normalizeKagemushaOperationReference(payload, {
      expectedOperationId: normalized.operationId,
      expectedKind: kind,
      location,
    }));
  }

  _url(path, params) {
    const normalizedPath = requireNonEmptyString(path, "path").replace(/^\/+/, "");
    const base = new URL(`${this.baseUrl}/`);
    const url = new URL(normalizedPath, base);
    appendSearchParams(url, params);
    return url;
  }

  async _json(method, path, options = {}) {
    const normalizedOptions = requireObject(options, `${method} ${path} options`);
    const headers = {
      Accept: "application/json",
      ...this.defaultHeaders,
      ...(normalizedOptions.headers ?? {}),
    };
    let timeoutId;
    let signal = normalizedOptions.signal;
    if (
      signal === undefined &&
      this.timeoutMs !== null &&
      this.timeoutMs !== undefined &&
      Number(this.timeoutMs) > 0
    ) {
      const controller = new AbortController();
      timeoutId = setTimeout(() => controller.abort(), Number(this.timeoutMs));
      signal = controller.signal;
    }
    const init = {
      method,
      cache: "no-store",
      headers,
      signal,
    };
    if (normalizedOptions.rawBody !== undefined) {
      init.body = normalizedOptions.rawBody;
      init.headers = {
        ...headers,
        ...(normalizedOptions.contentType ? { "Content-Type": normalizedOptions.contentType } : {}),
      };
    } else if (normalizedOptions.body !== undefined) {
      init.body = JSON.stringify(normalizedOptions.body);
      init.headers = {
        ...headers,
        "Content-Type": "application/json",
      };
    }
    let response;
    try {
      response = await this.fetchImpl(this._url(path, normalizedOptions.params), init);
    } finally {
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    }
    const status = responseStatus(response);
    const successStatuses = normalizedOptions.successStatuses ?? DEFAULT_SUCCESS_STATUSES;
    if (!successStatuses.includes(status)) {
      const errorResponse = typeof response?.clone === "function" ? response.clone() : response;
      const bodyText = await responseText(response);
      throw new ToriiBrowserHttpError(errorResponse, bodyText, status);
    }
    if (normalizedOptions.responseObserver !== undefined) {
      if (typeof normalizedOptions.responseObserver !== "function") {
        throw new TypeError(`${method} ${path} responseObserver must be a function`);
      }
      normalizedOptions.responseObserver(response);
    }
    if (status === 204) return null;
    const jsonParser = normalizedOptions.jsonParser ?? JSON.parse;
    if (typeof jsonParser !== "function") {
      throw new TypeError(`${method} ${path} jsonParser must be a function`);
    }
    if (typeof response.text !== "function" && typeof response.json === "function") {
      if (normalizedOptions.jsonParser !== undefined) {
        throw new TypeError(`${method} ${path} requires a text-capable response`);
      }
      return response.json();
    }
    const text = normalizedOptions.maximumBodyBytes === undefined
      ? await response.text()
      : await readBoundedResponseText(
        response,
        normalizedOptions.maximumBodyBytes,
        `${method} ${path}`,
      );
    if (text === "" && normalizedOptions.jsonParser === undefined) return null;
    return jsonParser(text);
  }

  async _bytes(method, path, options = {}) {
    const normalizedOptions = requireObject(options, `${method} ${path} options`);
    const headers = {
      ...this.defaultHeaders,
      ...(normalizedOptions.headers ?? {}),
      Accept: "application/x-norito",
    };
    let timeoutId;
    let signal = normalizedOptions.signal;
    if (
      signal === undefined &&
      this.timeoutMs !== null &&
      this.timeoutMs !== undefined &&
      Number(this.timeoutMs) > 0
    ) {
      const controller = new AbortController();
      timeoutId = setTimeout(() => controller.abort(), Number(this.timeoutMs));
      signal = controller.signal;
    }
    let response;
    try {
      response = await this.fetchImpl(this._url(path, normalizedOptions.params), {
        method,
        cache: "no-store",
        headers,
        signal,
      });
    } finally {
      if (timeoutId !== undefined) clearTimeout(timeoutId);
    }
    const status = responseStatus(response);
    const successStatuses = normalizedOptions.successStatuses ?? DEFAULT_SUCCESS_STATUSES;
    if (!successStatuses.includes(status)) {
      const errorResponse = typeof response?.clone === "function" ? response.clone() : response;
      const bodyText = await responseText(response);
      throw new ToriiBrowserHttpError(errorResponse, bodyText, status);
    }
    const contentType = response.headers?.get?.("content-type") ?? "";
    if (!/^application\/x-norito(?:\s*;|$)/iu.test(contentType)) {
      throw new TypeError(`${method} ${path} must return application/x-norito`);
    }
    if (normalizedOptions.maximumBodyBytes !== undefined) {
      return Buffer.from(
        await readBoundedResponseBytes(
          response,
          normalizedOptions.maximumBodyBytes,
          `${method} ${path}`,
        ),
      );
    }
    if (typeof response.arrayBuffer !== "function") {
      throw new TypeError(`${method} ${path} requires an arrayBuffer-capable response`);
    }
    return Buffer.from(await response.arrayBuffer());
  }

  async _canonicalJson(method, path, body, options, successStatuses = [200]) {
    const opts = requireObject(options, `${method} ${path} canonical options`);
    if (typeof opts.sign !== "function") {
      throw new TypeError(`${method} ${path} options.sign is required`);
    }
    const signed = await buildCanonicalJsonRequest({
      accountId: requireNonEmptyString(opts.authAccountId, `${method} ${path} options.authAccountId`),
      method,
      path,
      baseUrl: this.baseUrl,
      body,
      headers: opts.headers,
      sign: opts.sign,
      timestampMs: opts.timestampMs,
      nonce: opts.nonce,
    });
    return this._json(method, path, {
      rawBody: signed.body || undefined,
      contentType: body === undefined ? undefined : "application/json",
      headers: signed.headers,
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? successStatuses,
    });
  }

  /** Submit exact locally signed version-1 transaction bytes to the pipeline. */
  submitTransaction(signedTransaction, options = {}) {
    const opts = requireObject(options, "submitTransaction options");
    const body = requireTransactionBytes(
      signedTransaction,
      "submitTransaction signedTransaction",
    );
    const expectedEntrypointHash = browserSignedTransactionHashHex(body);
    const expectedSignedTransactionHash = innerSignedTransactionHashHex(body);
    return this._json("POST", "/v1/pipeline/transactions", {
      rawBody: body,
      contentType: "application/x-norito",
      headers: {
        Accept: "application/json",
        ...(opts.headers ?? {}),
      },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 201, 202, 204],
      responseObserver: (response) => {
        for (const name of [
          "x-iroha-entrypoint-hash",
          "x-iroha-transaction-hash",
        ]) {
          requireMatchingReceiptHashHeader(
            response,
            name,
            expectedEntrypointHash,
          );
        }
        requireMatchingReceiptHashHeader(
          response,
          "x-iroha-signed-transaction-hash",
          expectedSignedTransactionHash,
        );
      },
    });
  }

  /** Fetch one exact pipeline status by transaction hash. */
  async getTransactionStatus(hashHex, options = {}) {
    const opts = requireObject(options, "getTransactionStatus options");
    const hash = requireExactHashHex(hashHex, "getTransactionStatus hashHex");
    const scope = normalizeTransactionStatusScope(
      opts.scope,
      "getTransactionStatus options.scope",
    );
    try {
      return await this._json("GET", "/v1/pipeline/transactions/status", {
        params: {
          hash,
          scope,
        },
        headers: opts.headers,
        signal: signalFrom(opts),
        successStatuses: [200],
      });
    } catch (error) {
      if (error instanceof ToriiBrowserHttpError && error.status === 404) {
        return null;
      }
      throw error;
    }
  }

  /** Poll until global chain state proves the exact transaction was applied. */
  async waitForTransactionStatus(hashHex, options = {}) {
    const opts = requireObject(options, "waitForTransactionStatus options");
    const hash = requireExactHashHex(hashHex, "waitForTransactionStatus hashHex");
    rejectRemovedWaitScope(opts, "waitForTransactionStatus options");
    const intervalMs = normalizeOffset(
      opts.intervalMs,
      "waitForTransactionStatus.intervalMs",
      250,
    );
    const timeoutMs = normalizePositiveInteger(
      opts.timeoutMs,
      "waitForTransactionStatus.timeoutMs",
      60_000,
    );
    const maxAttempts = normalizePositiveInteger(
      opts.maxAttempts,
      "waitForTransactionStatus.maxAttempts",
      Math.max(1, Math.ceil(timeoutMs / Math.max(1, intervalMs))),
    );
    const signal = signalFrom(opts);
    const deadline = Date.now() + timeoutMs;
    let lastStatus = null;
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      throwIfAborted(signal);
      lastStatus = await this.getTransactionStatus(hash, {
        signal,
        scope: "global",
        headers: opts.headers,
      });
      if (lastStatus === null) {
        if (attempt >= maxAttempts || Date.now() >= deadline) break;
        await delayWithSignal(
          Math.min(intervalMs, Math.max(0, deadline - Date.now())),
          signal,
        );
        continue;
      }
      const classified = classifyGlobalPipelineStatusEnvelope(
        lastStatus,
        hash,
        "pipeline transaction status",
      );
      if (
        classified.kind === PIPELINE_SUCCESS_STATUS &&
        classified.authoritative
      ) {
        return lastStatus;
      }
      if (
        classified.authoritative &&
        PIPELINE_FAILURE_STATUSES.has(classified.kind)
      ) {
        const error = new Error(
          `Transaction ${hash} reached terminal ${classified.kind} status`,
        );
        error.name = "ToriiBrowserTransactionStatusError";
        error.hashHex = hash;
        error.status = classified.kind;
        error.payload = lastStatus;
        throw error;
      }
      if (attempt >= maxAttempts || Date.now() >= deadline) break;
      await delayWithSignal(Math.min(intervalMs, Math.max(0, deadline - Date.now())), signal);
    }
    const error = new Error(
      `Transaction ${hash} did not reach persisted Applied status within ${timeoutMs}ms`,
    );
    error.name = "ToriiBrowserTransactionTimeoutError";
    error.hashHex = hash;
    error.payload = lastStatus;
    throw error;
  }

  /** Submit exact locally signed bytes and wait for persisted Applied finality. */
  async submitTransactionAndWait(signedTransaction, options = {}) {
    const opts = requireObject(options, "submitTransactionAndWait options");
    rejectRemovedWaitScope(opts, "submitTransactionAndWait options");
    const body = requireTransactionBytes(
      signedTransaction,
      "submitTransactionAndWait signedTransaction",
    );
    const hashHex = browserSignedTransactionHashHex(body);
    if (opts.hashHex !== undefined) {
      const assertedHash = requireExactHashHex(
        opts.hashHex,
        "submitTransactionAndWait options.hashHex",
      );
      if (assertedHash !== hashHex) {
        throw new Error(
          "submitTransactionAndWait options.hashHex does not match signedTransaction",
        );
      }
    }
    await this.submitTransaction(body, {
      signal: signalFrom(opts),
      headers: opts.headers,
    });
    return this.waitForTransactionStatus(hashHex, opts);
  }

  /** Read the node compatibility advert before constructing deployment bytes. */
  getNodeCapabilities(options = {}) {
    const opts = requireObject(options, "getNodeCapabilities options");
    return this._json("GET", "/v1/node/capabilities", {
      headers: opts.headers,
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200],
    });
  }

  /** @internal Raw bounded transport for the optional privacy-capabilities API. */
  async [privacyCapabilityTransportV1](options = {}) {
    const opts = requireSupportedOptions(
      options,
      "getPrivacyCapabilitiesV1 options",
      PRIVACY_CAPABILITIES_REQUEST_OPTION_KEYS,
    );
    return this._json("GET", "/v1/privacy/capabilities", {
      headers: { ...(opts.headers ?? {}), Accept: "application/json" },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200],
      maximumBodyBytes: PRIVACY_CAPABILITIES_JSON_MAX_BYTES,
      jsonParser: (text) => parseStrictLosslessIntegerJson(
        text,
        "privacy capabilities response",
      ),
      responseObserver: (response) => requireExactJsonContentType(
        response.headers?.get?.("content-type"),
        "privacy capabilities response",
      ),
    });
  }

  /** Resolve a contract alias; caller-supplied canonical signing headers are preserved. */
  resolveContractAlias(contractAlias, options) {
    return this._canonicalJson("POST", "/v1/contracts/aliases/resolve", {
        contract_alias: requireNonEmptyString(
          contractAlias,
          "resolveContractAlias contractAlias",
        ),
      }, options);
  }

  /** Read the exact one-view deployment CAS state through canonical app auth. */
  async getContractDeploymentState(request, options = {}) {
    const body = normalizeContractDeploymentStateRequest(request);
    const opts = requireObject(options, "getContractDeploymentState options");
    if (opts.sign !== undefined) {
      if (typeof opts.sign !== "function") {
        throw new TypeError("getContractDeploymentState options.sign must be a function");
      }
      const signed = await buildCanonicalJsonRequest({
        accountId: requireNonEmptyString(
          opts.authAccountId,
          "getContractDeploymentState options.authAccountId",
        ),
        method: "POST",
        path: "/v1/contracts/deployment-state",
        baseUrl: this.baseUrl,
        body,
        headers: opts.headers,
        sign: opts.sign,
        timestampMs: opts.timestampMs,
        nonce: opts.nonce,
      });
      const response = await this._json("POST", "/v1/contracts/deployment-state", {
        rawBody: signed.body,
        contentType: "application/json",
        headers: signed.headers,
        signal: signalFrom(opts),
        successStatuses: opts.successStatuses ?? [200],
      });
      return normalizeContractDeploymentStateResponse(response, body);
    }
    const response = await this._json("POST", "/v1/contracts/deployment-state", {
      body,
      headers: opts.headers,
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200],
    });
    return normalizeContractDeploymentStateResponse(response, body);
  }

  /** Read exact account state; caller-supplied canonical signing headers are preserved. */
  getAccount(accountId, options = {}) {
    const opts = requireObject(options, "getAccount options");
    return this._json(
      "GET",
      `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}`,
      {
        headers: opts.headers,
        signal: signalFrom(opts),
        successStatuses: opts.successStatuses ?? [200],
      },
    );
  }

  listExplorerAccounts(options = {}) {
    const opts = requireObject(options, "listExplorerAccounts options");
    return this._json("GET", "/v1/explorer/accounts", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerAccounts options"),
        domain: opts.domain,
        with_asset: opts.withAsset ?? opts.with_asset,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeExplorerCursorPage(payload, "explorer accounts response"),
    );
  }

  getExplorerAccount(accountId, options = {}) {
    const opts = requireObject(options, "getExplorerAccount options");
    return this._json("GET", `/v1/explorer/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  listExplorerDomains(options = {}) {
    const opts = requireObject(options, "listExplorerDomains options");
    return this._json("GET", "/v1/explorer/domains", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerDomains options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeExplorerCursorPage(payload, "explorer domains response"),
    );
  }

  getExplorerDomain(domainId, options = {}) {
    const opts = requireObject(options, "getExplorerDomain options");
    return this._json("GET", `/v1/explorer/domains/${encodeURIComponent(requireNonEmptyString(domainId, "domainId"))}`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerAssets(options = {}) {
    const opts = requireObject(options, "listExplorerAssets options");
    return this._json("GET", "/v1/explorer/assets", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerAssets options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        definition: opts.definition,
        asset_id: opts.assetId ?? opts.asset_id,
      },
      signal: signalFrom(opts),
    }).then((payload) => {
      const context = "explorer assets response";
      return normalizeExplorerCursorPage(payload, context, (item, index) =>
        normalizeQuantityRecord(item, `${context}.items[${index}]`, ["value"]),
      );
    });
  }

  getExplorerAsset(assetId, options = {}) {
    const opts = requireObject(options, "getExplorerAsset options");
    return this._json("GET", `/v1/explorer/assets/${encodeURIComponent(requireNonEmptyString(assetId, "assetId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(payload, "explorer asset response", ["quantity"]),
    );
  }

  listAccountAssets(accountId, options = {}) {
    const opts = requireObject(options, "listAccountAssets options");
    return this._json("GET", `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/assets`, {
      params: {
        ...normalizeIterablePagination(opts, "listAccountAssets options"),
        asset: opts.asset ?? opts.assetId,
        scope: opts.scope,
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "account assets response", ["quantity"]),
    );
  }

  /** List effective direct and role-inherited permissions for an account. */
  listAccountPermissions(accountId, options = {}) {
    const context = "listAccountPermissions options";
    const opts = requireSupportedOptions(options, context, COUNTED_LIST_OPTION_KEYS);
    return this._json(
      "GET",
      `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/permissions`,
      {
        params: normalizeCountedListParams(opts, context),
        signal: signalFrom(opts),
      },
    );
  }

  /** List indexed value movement and affected-transaction history for an account. */
  listAccountHistory(accountId, options = {}) {
    const context = "listAccountHistory options";
    const opts = requireSupportedOptions(options, context, ACCOUNT_HISTORY_OPTION_KEYS);
    return this._json(
      "GET",
      `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/history`,
      {
        params: {
          ...normalizeCountedListParams(opts, context),
          asset_id: normalizeOptionalString(
            optionAlias(opts, "assetId", "asset_id"),
            `${context}.assetId`,
          ),
        },
        signal: signalFrom(opts),
      },
    );
  }

  queryAccountTransactions(accountId, options = {}) {
    const opts = requireObject(options, "queryAccountTransactions options");
    return this._json("POST", `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/transactions/query`, {
      body: normalizeTransactionQueryEnvelope(opts, "queryAccountTransactions"),
      signal: signalFrom(opts),
    });
  }

  queryTransactions(options = {}) {
    const opts = requireObject(options, "queryTransactions options");
    return this._json("POST", "/v1/transactions/query", {
      body: normalizeTransactionQueryEnvelope(opts, "queryTransactions"),
      signal: signalFrom(opts),
    });
  }

  queryVisibleTransactions(options = {}) {
    const opts = requireObject(options, "queryVisibleTransactions options");
    return this._json("POST", "/v1/transactions/visible/query", {
      body: normalizeTransactionQueryEnvelope(opts, "queryVisibleTransactions"),
      signal: signalFrom(opts),
    });
  }

  /** List committed contract-call activity using Torii's route-specific filters. */
  listContractActivity(options = {}) {
    const context = "listContractActivity options";
    const opts = requireSupportedOptions(options, context, CONTRACT_ACTIVITY_OPTION_KEYS);
    return this._json("GET", "/v1/contracts/activity", {
      params: {
        ...normalizeCountedListParams(opts, context),
        authority: normalizeOptionalString(opts.authority, `${context}.authority`),
        contract_address: normalizeOptionalString(
          optionAlias(opts, "contractAddress", "contract_address"),
          `${context}.contractAddress`,
        ),
        contract_alias: normalizeOptionalString(
          optionAlias(opts, "contractAlias", "contract_alias"),
          `${context}.contractAlias`,
        ),
        contract_entrypoint: normalizeOptionalString(
          optionAlias(opts, "contractEntrypoint", "contract_entrypoint"),
          `${context}.contractEntrypoint`,
        ),
        since_timestamp_ms: normalizeOptionalUnsignedInteger(
          optionAlias(opts, "sinceTimestampMs", "since_timestamp_ms"),
          `${context}.sinceTimestampMs`,
        ),
        until_timestamp_ms: normalizeOptionalUnsignedInteger(
          optionAlias(opts, "untilTimestampMs", "until_timestamp_ms"),
          `${context}.untilTimestampMs`,
        ),
        result_ok: normalizeOptionalBoolean(
          optionAlias(opts, "resultOk", "result_ok"),
          `${context}.resultOk`,
        ),
      },
      signal: signalFrom(opts),
    });
  }

  /** List indexed generic contract events using Torii's route-specific filters. */
  listContractEvents(options = {}) {
    const context = "listContractEvents options";
    const opts = requireSupportedOptions(options, context, CONTRACT_EVENT_LIST_OPTION_KEYS);
    return this._json("GET", "/v1/contracts/events", {
      params: {
        ...normalizeCountedListParams(opts, context),
        ...normalizeContractEventFilterParams(opts, context),
      },
      signal: signalFrom(opts),
    });
  }

  /**
   * Open one non-replayable fetch stream for generic contract events.
   * Stream gaps and an unrequested EOF are terminal; callers must explicitly resubscribe.
   */
  streamContractEvents(options = {}) {
    const context = "streamContractEvents options";
    const opts = requireSupportedOptions(options, context, CONTRACT_EVENT_STREAM_OPTION_KEYS);
    const params = normalizeContractEventFilterParams(opts, context);
    const client = this;
    return (async function* contractEventIterator() {
      const response = await client.fetchImpl(client._url("/v1/contracts/events/sse", params), {
        method: "GET",
        cache: "no-store",
        headers: streamRequestHeaders(client.defaultHeaders),
        signal: signalFrom(opts),
      });
      const status = responseStatus(response);
      if (status !== 200) {
        const errorResponse = typeof response?.clone === "function" ? response.clone() : response;
        const bodyText = await responseText(response);
        throw new ToriiBrowserHttpError(errorResponse, bodyText, status);
      }
      if (typeof response?.body?.getReader !== "function") {
        throw new ToriiBrowserStreamGapError(
          "The contract event stream ended without a readable response body.",
          { code: "stream_unexpected_eof" },
        );
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      let ended = false;
      try {
        while (!ended) {
          const chunk = await reader.read();
          ended = chunk.done === true;
          if (chunk.value !== undefined) {
            buffer += decoder.decode(chunk.value, { stream: !ended });
          }
          if (ended) buffer += decoder.decode();
          const parsed = extractSseFrames(buffer);
          buffer = parsed.remainder;
          for (const event of parsed.frames) {
            if (event.event === "stream_error") throw streamGapFromEvent(event);
            yield event;
          }
        }
        if (opts.signal?.aborted === true) return;
        throw new ToriiBrowserStreamGapError(
          "The contract event stream ended unexpectedly and cannot be resumed.",
          { code: "stream_unexpected_eof" },
        );
      } finally {
        if (!ended && typeof reader.cancel === "function") {
          try {
            await reader.cancel();
          } catch {
            // Preserve the stream error or consumer cancellation that entered this block.
          }
        }
        if (typeof reader.releaseLock === "function") reader.releaseLock();
      }
    })();
  }

  listAssetHolders(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "listAssetHolders options");
    return this._json("GET", `/v1/assets/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/holders`, {
      params: {
        ...normalizeIterablePagination(opts, "listAssetHolders options"),
        account_id: opts.accountId ?? opts.account_id,
        scope: opts.scope,
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "asset holders response", ["quantity"]),
    );
  }

  listAssetDefinitions(options = {}) {
    const opts = requireObject(options, "listAssetDefinitions options");
    return this._json("GET", "/v1/assets/definitions", {
      params: {
        ...normalizeIterablePagination(opts, "listAssetDefinitions options"),
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(
        payload,
        "asset definitions response",
        ["total_quantity"],
        { optional: true },
      ),
    );
  }

  getAssetDefinition(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getAssetDefinition options");
    return this._json("GET", `/v1/assets/definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(
        payload,
        "asset definition response",
        ["total_quantity"],
        { optional: true },
      ),
    );
  }

  resolveAlias(aliasOrRequest, options = {}) {
    const opts = requireObject(options, "resolveAlias options");
    const body =
      typeof aliasOrRequest === "string"
        ? { alias: requireNonEmptyString(aliasOrRequest, "alias") }
        : requireObject(aliasOrRequest, "resolveAlias request");
    return this._json("POST", "/v1/aliases/resolve", {
      body,
      signal: signalFrom(opts),
    });
  }

  resolveAssetAlias(aliasOrRequest, options = {}) {
    const opts = requireObject(options, "resolveAssetAlias options");
    const body =
      typeof aliasOrRequest === "string"
        ? { alias: requireNonEmptyString(aliasOrRequest, "alias") }
        : requireObject(aliasOrRequest, "resolveAssetAlias request");
    return this._json("POST", "/v1/assets/aliases/resolve", {
      body,
      signal: signalFrom(opts),
    });
  }

  listExplorerAssetDefinitions(options = {}) {
    const opts = requireObject(options, "listExplorerAssetDefinitions options");
    return this._json("GET", "/v1/explorer/asset-definitions", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerAssetDefinitions options"),
        owning_domain: opts.owningDomain ?? opts.owning_domain,
        owned_by: opts.ownedBy ?? opts.owned_by,
      },
      signal: signalFrom(opts),
    }).then((payload) => {
      const context = "explorer asset definitions response";
      return normalizeExplorerCursorPage(payload, context, (item, index) =>
        normalizeExplorerAssetDefinitionRecord(item, `${context}.items[${index}]`),
      );
    });
  }

  getExplorerAssetDefinitionEconometrics(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getExplorerAssetDefinitionEconometrics options");
    return this._json("GET", `/v1/explorer/asset-definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/econometrics`, {
      signal: signalFrom(opts),
    });
  }

  getExplorerAssetDefinitionSnapshot(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getExplorerAssetDefinitionSnapshot options");
    return this._json("GET", `/v1/explorer/asset-definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/snapshot`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerNfts(options = {}) {
    const opts = requireObject(options, "listExplorerNfts options");
    return this._json("GET", "/v1/explorer/nfts", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerNfts options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        domain: opts.domain,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeExplorerCursorPage(payload, "explorer nfts response"),
    );
  }

  getExplorerNft(nftId, options = {}) {
    const opts = requireObject(options, "getExplorerNft options");
    return this._json("GET", `/v1/explorer/nfts/${encodeURIComponent(requireNonEmptyString(nftId, "nftId"))}`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerRwas(options = {}) {
    const opts = requireObject(options, "listExplorerRwas options");
    return this._json("GET", "/v1/explorer/rwas", {
      params: {
        ...normalizeExplorerCursorPagination(opts, "listExplorerRwas options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        domain: opts.domain,
      },
      signal: signalFrom(opts),
    }).then((payload) => {
      const context = "explorer rwas response";
      return normalizeExplorerCursorPage(payload, context, (item, index) =>
        normalizeQuantityRecord(
          item,
          `${context}.items[${index}]`,
          ["quantity", "held_quantity"],
        ),
      );
    });
  }

  getExplorerRwa(rwaId, options = {}) {
    const opts = requireObject(options, "getExplorerRwa options");
    return this._json("GET", `/v1/explorer/rwas/${encodeURIComponent(requireNonEmptyString(rwaId, "rwaId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(payload, "explorer rwa response", ["quantity", "held_quantity"]),
    );
  }

  listExplorerBlocks(options = {}) {
    const opts = requireObject(options, "listExplorerBlocks options");
    return this._json("GET", "/v1/explorer/blocks", {
      params: normalizeExplorerPagination(opts, "listExplorerBlocks options"),
      signal: signalFrom(opts),
    });
  }

  getExplorerBlock(identifier, options = {}) {
    const opts = requireObject(options, "getExplorerBlock options");
    return this._json("GET", `/v1/explorer/blocks/${encodeURIComponent(String(identifier))}`, {
      signal: signalFrom(opts),
    });
  }

  /** List newest-first canonical ledger headers with Torii's bounded window. */
  listLedgerHeaders(options = {}) {
    const context = "listLedgerHeaders options";
    const opts = requireSupportedOptions(options, context, LEDGER_HEADERS_OPTION_KEYS);
    return this._json("GET", "/v1/ledger/headers", {
      params: {
        from: opts.from === undefined
          ? undefined
          : normalizeLedgerHeight(opts.from, `${context}.from`),
        limit: opts.limit === undefined
          ? undefined
          : normalizePositiveInteger(opts.limit, `${context}.limit`, undefined),
      },
      signal: signalFrom(opts),
    });
  }

  /** Fetch the node-provided execution state root recorded at a block height. */
  getLedgerStateRoot(height, options = {}) {
    const context = "getLedgerStateRoot options";
    const opts = requireSupportedOptions(options, context, LEDGER_READ_OPTION_KEYS);
    const normalizedHeight = normalizeLedgerHeight(height, "getLedgerStateRoot height");
    return this._json("GET", `/v1/ledger/state/${normalizedHeight}`, {
      signal: signalFrom(opts),
    });
  }

  /** Fetch the node-provided execution QC at a block height. */
  getLedgerStateProof(height, options = {}) {
    const context = "getLedgerStateProof options";
    const opts = requireSupportedOptions(options, context, LEDGER_READ_OPTION_KEYS);
    const normalizedHeight = normalizeLedgerHeight(height, "getLedgerStateProof height");
    return this._json("GET", `/v1/ledger/state-proof/${normalizedHeight}`, {
      signal: signalFrom(opts),
    });
  }

  /** Fetch the exact canonical result-bearing SignedBlockWire at a finalized height. */
  async getLedgerExecutedBlockWire(height, options = {}) {
    const context = "getLedgerExecutedBlockWire options";
    const opts = requireSupportedOptions(options, context, LEDGER_READ_OPTION_KEYS);
    const normalizedHeight = normalizeLedgerHeight(
      height,
      "getLedgerExecutedBlockWire height",
    );
    const bytes = await this._bytes(
      "GET",
      `/v1/ledger/block/${normalizedHeight}`,
      {
        signal: signalFrom(opts),
        maximumBodyBytes: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
      },
    );
    if (bytes.byteLength === 0) {
      throw new TypeError("executed block wire response must not be empty");
    }
    return bytes;
  }

  /** Fetch and decode the canonical Norito block inclusion/execution proof. */
  async getLedgerBlockProof(height, entryHash, options = {}) {
    const context = "getLedgerBlockProof options";
    const opts = requireSupportedOptions(options, context, LEDGER_READ_OPTION_KEYS);
    const normalizedHeight = normalizeLedgerHeight(height, "getLedgerBlockProof height");
    const normalizedHash = normalizeLedgerEntryHash(
      entryHash,
      "getLedgerBlockProof entryHash",
    );
    const bytes = await this._bytes(
      "GET",
      `/v1/ledger/block/${normalizedHeight}/proof/${normalizedHash}`,
      {
        signal: signalFrom(opts),
        maximumBodyBytes: AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1,
      },
    );
    return noritoDecodeBlockProofs(bytes);
  }

  getExplorerMetrics(options = {}) {
    const opts = requireObject(options, "getExplorerMetrics options");
    return this._json("GET", "/v1/explorer/metrics", { signal: signalFrom(opts) });
  }

  getExplorerHealth(options = {}) {
    const opts = requireObject(options, "getExplorerHealth options");
    return this._json("GET", "/v1/explorer/health", { signal: signalFrom(opts) });
  }

  listExplorerTransactions(options = {}) {
    const opts = requireObject(options, "listExplorerTransactions options");
    return this._json("GET", "/v1/explorer/transactions", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerTransactions options"),
        authority: opts.authority,
        block: opts.block,
        status: opts.status,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  listLatestExplorerTransactions(options = {}) {
    const opts = requireObject(options, "listLatestExplorerTransactions options");
    return this._json("GET", "/v1/explorer/transactions/latest", {
      params: {
        per_page: opts.perPage ?? opts.per_page,
        authority: opts.authority,
        block: opts.block,
        status: opts.status,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerTransaction(hash, options = {}) {
    const opts = requireObject(options, "getExplorerTransaction options");
    return this._json("GET", `/v1/explorer/transactions/${encodeURIComponent(requireNonEmptyString(hash, "hash"))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  listExplorerInstructions(options = {}) {
    const opts = requireObject(options, "listExplorerInstructions options");
    return this._json("GET", "/v1/explorer/instructions", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerInstructions options"),
        account: opts.account,
        authority: opts.authority,
        kind: opts.kind,
        transaction_hash: opts.transactionHash ?? opts.transaction_hash,
        transaction_status: opts.transactionStatus ?? opts.transaction_status,
        block: opts.block,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  listLatestExplorerInstructions(options = {}) {
    const opts = requireObject(options, "listLatestExplorerInstructions options");
    return this._json("GET", "/v1/explorer/instructions/latest", {
      params: {
        per_page: opts.perPage ?? opts.per_page,
        account: opts.account,
        authority: opts.authority,
        kind: opts.kind,
        transaction_hash: opts.transactionHash ?? opts.transaction_hash,
        transaction_status: opts.transactionStatus ?? opts.transaction_status,
        block: opts.block,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerInstruction(transactionHash, index, options = {}) {
    const opts = requireObject(options, "getExplorerInstruction options");
    return this._json("GET", `/v1/explorer/instructions/${encodeURIComponent(requireNonEmptyString(transactionHash, "transactionHash"))}/${encodeURIComponent(String(index))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  getExplorerInstructionContractView(transactionHash, index, options = {}) {
    const opts = requireObject(options, "getExplorerInstructionContractView options");
    return this._json("GET", `/v1/explorer/instructions/${encodeURIComponent(requireNonEmptyString(transactionHash, "transactionHash"))}/${encodeURIComponent(String(index))}/contract-view`, {
      signal: signalFrom(opts),
    });
  }

  getMultisigSpec(selector, options) {
    return this._canonicalJson("POST", "/v1/multisig/spec",
      normalizeMultisigSelectorBody(selector, "getMultisigSpec selector"), options);
  }

  queryMultisigProposals(selector, options) {
    return this._canonicalJson("POST", "/v1/multisig/proposals/query",
      normalizeMultisigProposalsQueryBody(
        selector,
        "queryMultisigProposals selector",
      ), options);
  }

  resolveMultisigProposal(request, options) {
    const normalizedRequest = normalizeMultisigProposalsResolveBody(
      request,
      "resolveMultisigProposal request",
    );
    return this._canonicalJson(
      "POST", "/v1/multisig/proposals/resolve", normalizedRequest, options,
    );
  }

  async submitMultisigPropose(request, options = {}) {
    const opts = requireObject(options, "submitMultisigPropose options");
    return this._json("POST", "/v1/multisig/propose", {
      rawBody: noritoEncodeMultisigProposeRequest(requireObject(request, "submitMultisigPropose request")),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  async submitMultisigContractCallPropose(request, options = {}) {
    const opts = requireObject(options, "submitMultisigContractCallPropose options");
    return this._json("POST", "/v1/contracts/call/multisig/propose", {
      rawBody: noritoEncodeMultisigContractCallProposeRequest(
        requireObject(request, "submitMultisigContractCallPropose request"),
      ),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  async submitMultisigContractCallApprove(request, options = {}) {
    const opts = requireObject(options, "submitMultisigContractCallApprove options");
    return this._json("POST", "/v1/contracts/call/multisig/approve", {
      rawBody: noritoEncodeMultisigContractCallApproveRequest(
        requireObject(request, "submitMultisigContractCallApprove request"),
      ),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  getSumeragiStatus(options = {}) {
    const opts = requireObject(options, "getSumeragiStatus options");
    return this._json("GET", "/v1/sumeragi/status", { signal: signalFrom(opts) });
  }

  getSumeragiStatusTyped(options = {}) {
    const opts = signalOnlyOptions(options, "getSumeragiStatusTyped options");
    return this._json("GET", "/v1/sumeragi/status", {
      headers: { Accept: "application/json" },
      signal: signalFrom(opts),
      maximumBodyBytes: SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES,
      responseObserver: (response) => {
        requireExactJsonContentType(
          response.headers.get("content-type"),
          "Sumeragi typed status response",
        );
      },
      jsonParser: (text) => parseSumeragiStatusJson(
        text,
        "Sumeragi typed status",
      ),
    });
  }

  getSumeragiDiagnostics(options = {}) {
    const opts = requireObject(options, "getSumeragiDiagnostics options");
    return this._json("GET", "/v1/sumeragi/diagnostics", { signal: signalFrom(opts) });
  }

  getSumeragiDiagnosticsTyped(options = {}) {
    const opts = signalOnlyOptions(options, "getSumeragiDiagnosticsTyped options");
    return this._json("GET", "/v1/sumeragi/diagnostics", {
      headers: { Accept: "application/json" },
      signal: signalFrom(opts),
      maximumBodyBytes: SUMERAGI_DIAGNOSTICS_TYPED_JSON_MAX_BYTES,
      responseObserver: (response) => {
        requireExactJsonContentType(
          response.headers.get("content-type"),
          "Sumeragi typed diagnostics response",
        );
      },
      jsonParser: (text) => parseSumeragiDiagnosticsJson(
        text,
        "Sumeragi typed diagnostics",
      ),
    });
  }

  getSumeragiTelemetry(options = {}) {
    const opts = requireObject(options, "getSumeragiTelemetry options");
    return this._json("GET", "/v1/sumeragi/telemetry", { signal: signalFrom(opts) });
  }

  listKaigiRelays(options = {}) {
    const opts = requireObject(options, "listKaigiRelays options");
    return this._json("GET", "/v1/kaigi/relays", { signal: signalFrom(opts) });
  }

  getKaigiRelay(relayId, options = {}) {
    const opts = requireObject(options, "getKaigiRelay options");
    return this._json("GET", `/v1/kaigi/relays/${encodeURIComponent(requireNonEmptyString(relayId, "relayId"))}`, {
      signal: signalFrom(opts),
    });
  }

  getKaigiRelaysHealth(options = {}) {
    const opts = requireObject(options, "getKaigiRelaysHealth options");
    return this._json("GET", "/v1/kaigi/relays/health", { signal: signalFrom(opts) });
  }

}

export { ToriiBrowserClient as ToriiClient, ToriiBrowserHttpError as ToriiHttpError };
