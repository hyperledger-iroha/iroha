/** First-release Torii Offline HTTP contract helpers. */

export const OFFLINE_READINESS_PATH = "/v1/offline/readiness";
export const OFFLINE_TOP_UP_PATH = "/v1/offline/top-up";
export const OFFLINE_REDEEM_PATH = "/v1/offline/redeem";
export const OFFLINE_OPERATIONS_PATH = "/v1/offline/operations";

const OPERATION_ID_PATTERN = /^(?!0{64}$)[0-9a-f]{64}$/u;
const TRANSACTION_HASH_PATTERN = /^[0-9a-f]{64}$/u;
const ERROR_CODE_PATTERN = /^[a-z0-9][a-z0-9_]{0,63}$/u;
const MAX_U32 = 0xffff_ffffn;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U128 = (1n << 128n) - 1n;
const MAX_JSON_DEPTH = 128;

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireObject(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be a plain object`);
  }
  return value;
}

function requireOwn(record, field, context) {
  if (!Object.prototype.hasOwnProperty.call(record, field)) {
    throw new TypeError(`${context}.${field} is required`);
  }
  return record[field];
}

function requireExactString(value, context, { nonEmpty = true } = {}) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  if (nonEmpty && value.length === 0) {
    throw new TypeError(`${context} must not be empty`);
  }
  if (value.trim() !== value) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  assertWellFormedUnicode(value, context);
  return value;
}

function requireHumanMessage(value, context) {
  const message = requireExactString(value, context);
  if (/[\u0000-\u001f\u007f-\u009f]/u.test(message)) {
    throw new TypeError(`${context} must not contain control characters`);
  }
  return message;
}

function assertWellFormedUnicode(value, context) {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new TypeError(`${context} must not contain an unpaired UTF-16 surrogate`);
      }
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      throw new TypeError(`${context} must not contain an unpaired UTF-16 surrogate`);
    }
  }
}

/** Parse a Torii Offline JSON body without rounding wide integer tokens. */
export function parseOfflineJson(text, context = "Offline JSON response") {
  if (typeof text !== "string") {
    throw new TypeError(`${context} must be JSON text`);
  }
  let index = 0;

  const syntax = (message) => {
    throw new SyntaxError(`${context}: ${message} at UTF-16 offset ${index}`);
  };
  const skipWhitespace = () => {
    while (index < text.length && /[\t\n\r ]/u.test(text[index])) {
      index += 1;
    }
  };
  const parseString = (stringContext) => {
    if (text[index] !== '"') syntax("expected a JSON string");
    const start = index;
    index += 1;
    let escaped = false;
    while (index < text.length) {
      const code = text.charCodeAt(index);
      if (!escaped && code === 0x22) {
        index += 1;
        let value;
        try {
          value = JSON.parse(text.slice(start, index));
        } catch {
          syntax("invalid JSON string");
        }
        assertWellFormedUnicode(value, stringContext);
        return value;
      }
      if (!escaped && code < 0x20) syntax("unescaped control character in string");
      if (!escaped && code === 0x5c) {
        escaped = true;
      } else {
        escaped = false;
      }
      index += 1;
    }
    syntax("unterminated JSON string");
  };
  const parseNumber = () => {
    const remainder = text.slice(index);
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/u.exec(remainder);
    if (!match) syntax("invalid JSON number");
    const token = match[0];
    index += token.length;
    if (/^-?(?:0|[1-9][0-9]*)$/u.test(token)) {
      const integer = BigInt(token);
      if (
        integer >= BigInt(Number.MIN_SAFE_INTEGER)
        && integer <= BigInt(Number.MAX_SAFE_INTEGER)
      ) {
        return Number(token);
      }
      return integer;
    }
    const number = Number(token);
    if (!Number.isFinite(number)) syntax("non-finite JSON number");
    return number;
  };
  const parseLiteral = (literal, value) => {
    if (!text.startsWith(literal, index)) syntax(`expected ${literal}`);
    index += literal.length;
    return value;
  };
  const parseValue = (depth) => {
    if (depth > MAX_JSON_DEPTH) {
      throw new RangeError(`${context} exceeds the maximum JSON nesting depth`);
    }
    skipWhitespace();
    const next = text[index];
    if (next === '"') return parseString(context);
    if (next === "{") {
      index += 1;
      const result = {};
      const keys = new Set();
      skipWhitespace();
      if (text[index] === "}") {
        index += 1;
        return result;
      }
      while (index < text.length) {
        const key = parseString(`${context} object key`);
        if (keys.has(key)) {
          throw new TypeError(`${context} contains duplicate JSON key: ${key}`);
        }
        keys.add(key);
        skipWhitespace();
        if (text[index] !== ":") syntax("expected ':' after object key");
        index += 1;
        const value = parseValue(depth + 1);
        Object.defineProperty(result, key, {
          value,
          enumerable: true,
          configurable: true,
          writable: true,
        });
        skipWhitespace();
        if (text[index] === "}") {
          index += 1;
          return result;
        }
        if (text[index] !== ",") syntax("expected ',' or '}'");
        index += 1;
        skipWhitespace();
      }
      syntax("unterminated JSON object");
    }
    if (next === "[") {
      index += 1;
      const result = [];
      skipWhitespace();
      if (text[index] === "]") {
        index += 1;
        return result;
      }
      while (index < text.length) {
        result.push(parseValue(depth + 1));
        skipWhitespace();
        if (text[index] === "]") {
          index += 1;
          return result;
        }
        if (text[index] !== ",") syntax("expected ',' or ']'");
        index += 1;
        skipWhitespace();
      }
      syntax("unterminated JSON array");
    }
    if (next === "t") return parseLiteral("true", true);
    if (next === "f") return parseLiteral("false", false);
    if (next === "n") return parseLiteral("null", null);
    return parseNumber();
  };

  const value = parseValue(0);
  skipWhitespace();
  if (index !== text.length) syntax("trailing JSON data");
  return value;
}

export function requireOfflineAssetDefinitionId(value, context = "assetDefinitionId") {
  return requireExactString(value, context);
}

function requireUnsignedInteger(value, context, maximum, { positive = false } = {}) {
  let integer;
  if (typeof value === "bigint") {
    integer = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a safe integer or bigint`);
    }
    integer = BigInt(value);
  } else {
    throw new TypeError(`${context} must be a safe integer or bigint`);
  }
  if (integer < 0n || (positive && integer === 0n) || integer > maximum) {
    const range = positive ? `between 1 and ${maximum}` : `between 0 and ${maximum}`;
    throw new RangeError(`${context} must be ${range}`);
  }
  return value;
}

function requireUnsignedResponseInteger(value, context, { positive = false } = {}) {
  if (typeof value === "bigint") {
    if (value < 0n || (positive && value === 0n) || value > MAX_U128) {
      const lower = positive ? 1 : 0;
      throw new RangeError(`${context} must be between ${lower} and ${MAX_U128}`);
    }
    return value;
  }
  if (Number.isSafeInteger(value) && value >= (positive ? 1 : 0)) return value;
  const requirement = positive ? "a positive lossless integer" : "a non-negative lossless integer";
  throw new TypeError(`${context} must be ${requirement}`);
}

function requireByteArray(value, context, exactLength = null) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be a JSON byte array`);
  }
  if (exactLength !== null && value.length !== exactLength) {
    throw new RangeError(`${context} must contain exactly ${exactLength} bytes`);
  }
  for (let index = 0; index < value.length; index += 1) {
    if (!Object.prototype.hasOwnProperty.call(value, index)) {
      throw new TypeError(`${context} must not be sparse`);
    }
    const byte = value[index];
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
      throw new RangeError(`${context}[${index}] must be an integer byte`);
    }
  }
  return value;
}

function operationIdFromBytes(value, context) {
  const bytes = requireByteArray(value, context, 32);
  if (bytes.every((byte) => byte === 0)) {
    throw new RangeError(`${context} must not be all zero`);
  }
  return bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function requireOfflineOperationId(value, context = "operationId") {
  if (typeof value !== "string" || !OPERATION_ID_PATTERN.test(value)) {
    throw new TypeError(`${context} must be a non-zero lowercase 64-character hexadecimal string`);
  }
  return value;
}

export function requireOfflineJsonContentType(value, context = "Offline response") {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must use Content-Type application/json`);
  }
  const mediaType = value.split(";", 1)[0].trim().toLowerCase();
  if (mediaType !== "application/json") {
    throw new TypeError(`${context} must use Content-Type application/json`);
  }
  return value;
}

function requireTransactionHash(value, context) {
  if (typeof value !== "string" || !TRANSACTION_HASH_PATTERN.test(value)) {
    throw new TypeError(`${context} must be a lowercase 64-character hexadecimal string`);
  }
  return value;
}

function snapshotJson(value, context, ancestors = new Set(), depth = 0) {
  if (depth > MAX_JSON_DEPTH) {
    throw new RangeError(`${context} exceeds the maximum JSON nesting depth`);
  }
  if (value === null || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    assertWellFormedUnicode(value, context);
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError(`${context} numbers must be non-negative safe integers`);
    }
    return value;
  }
  if (typeof value === "bigint") {
    if (value < 0n || value > MAX_U128) {
      throw new RangeError(`${context} bigint must fit in unsigned 128-bit range`);
    }
    return value;
  }
  if (typeof value !== "object") {
    throw new TypeError(`${context} contains unsupported ${typeof value} value`);
  }
  if (ancestors.has(value)) {
    throw new TypeError(`${context} must not contain a cycle`);
  }
  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const result = [];
      for (let index = 0; index < value.length; index += 1) {
        if (!Object.prototype.hasOwnProperty.call(value, index)) {
          throw new TypeError(`${context} must not contain sparse arrays`);
        }
        result.push(snapshotJson(value[index], `${context}[${index}]`, ancestors, depth + 1));
      }
      return result;
    }
    requireObject(value, context);
    const result = {};
    for (const key of Reflect.ownKeys(value)) {
      if (typeof key !== "string") {
        if (Object.prototype.propertyIsEnumerable.call(value, key)) {
          throw new TypeError(`${context} must not contain enumerable symbol keys`);
        }
        continue;
      }
      if (!Object.prototype.propertyIsEnumerable.call(value, key)) {
        continue;
      }
      assertWellFormedUnicode(key, `${context} key`);
      Object.defineProperty(result, key, {
        value: snapshotJson(value[key], `${context}.${key}`, ancestors, depth + 1),
        enumerable: true,
        configurable: true,
        writable: true,
      });
    }
    return result;
  } finally {
    ancestors.delete(value);
  }
}

function stringifyJsonSnapshot(value) {
  if (value === null || typeof value === "boolean" || typeof value === "number") {
    return String(value);
  }
  if (typeof value === "bigint") {
    return value.toString(10);
  }
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stringifyJsonSnapshot).join(",")}]`;
  }
  return `{${Object.keys(value)
    .map((key) => `${JSON.stringify(key)}:${stringifyJsonSnapshot(value[key])}`)
    .join(",")}}`;
}

function validateScaledAmount(value, context) {
  const amount = requireObject(value, context);
  requireUnsignedInteger(requireOwn(amount, "atomic_units", context), `${context}.atomic_units`, MAX_U128, {
    positive: true,
  });
  requireUnsignedInteger(requireOwn(amount, "scale", context), `${context}.scale`, MAX_U32);
}

function validateAuthorizationOperationId(request, context, operationId) {
  const authorization = requireObject(requireOwn(request, "authorization", context), `${context}.authorization`);
  const authorizedOperationId = operationIdFromBytes(
    requireOwn(authorization, "operation_id", `${context}.authorization`),
    `${context}.authorization.operation_id`,
  );
  if (authorizedOperationId !== operationId) {
    throw new TypeError(
      `${context}.authorization.operation_id must match ${context}.operation_id`,
    );
  }
}

function snapshotAndValidateCommand(input, context, kind) {
  const request = snapshotJson(input, context);
  requireObject(request, context);
  const operationId = operationIdFromBytes(
    requireOwn(request, "operation_id", context),
    `${context}.operation_id`,
  );
  validateAuthorizationOperationId(request, context, operationId);

  if (kind === "top_up") {
    requireExactString(requireOwn(request, "asset", context), `${context}.asset`);
    validateScaledAmount(requireOwn(request, "amount", context), `${context}.amount`);
    requireObject(requireOwn(request, "current_note", context), `${context}.current_note`);
    requireObject(requireOwn(request, "record_bundle", context), `${context}.record_bundle`);
    requireByteArray(
      requireOwn(request, "pallas_open_envelopes_archive", context),
      `${context}.pallas_open_envelopes_archive`,
    );
    const generation = requireExactString(
      requireOwn(request, "artifact_generation", context),
      `${context}.artifact_generation`,
    );
    if (generation.length > 128 || /[\u0000-\u001f\u007f]/u.test(generation)) {
      throw new RangeError(`${context}.artifact_generation must be at most 128 non-control characters`);
    }
  } else {
    requireObject(requireOwn(request, "bundle", context), `${context}.bundle`);
    requireExactString(requireOwn(request, "recipient", context), `${context}.recipient`);
    validateScaledAmount(requireOwn(request, "amount", context), `${context}.amount`);
    requireObject(requireOwn(request, "redeem_proof", context), `${context}.redeem_proof`);
    requireObject(requireOwn(request, "redemption", context), `${context}.redemption`);
    requireObject(
      requireOwn(request, "lineage_verifier_record", context),
      `${context}.lineage_verifier_record`,
    );
    requireUnsignedInteger(
      requireOwn(request, "block_height", context),
      `${context}.block_height`,
      MAX_U64,
    );
    for (const field of ["lineage_witness", "offline_change"]) {
      if (Object.prototype.hasOwnProperty.call(request, field) && request[field] !== null) {
        requireObject(request[field], `${context}.${field}`);
      }
    }
  }
  return { body: stringifyJsonSnapshot(request), operationId };
}

export function normalizeOfflineTopUpRequest(input, context = "submitOfflineTopUp request") {
  return snapshotAndValidateCommand(input, context, "top_up");
}

export function normalizeOfflineRedeemRequest(input, context = "submitOfflineRedeem request") {
  return snapshotAndValidateCommand(input, context, "redeem");
}

function cloneResponseJson(value, context) {
  return snapshotJson(value, context);
}

export function normalizeOfflineReadinessResponse(payload, expectedAssetDefinitionId) {
  const context = "offline readiness response";
  const record = requireObject(payload, context);
  const assetDefinitionId = requireExactString(
    requireOwn(record, "asset_definition_id", context),
    `${context}.asset_definition_id`,
  );
  if (assetDefinitionId !== expectedAssetDefinitionId) {
    throw new TypeError(`${context}.asset_definition_id does not match the requested asset`);
  }
  const evaluatedBlockHeight = requireUnsignedResponseInteger(
    requireOwn(record, "evaluated_block_height", context),
    `${context}.evaluated_block_height`,
  );
  const evaluatedBlockHash = requireTransactionHash(
    requireOwn(record, "evaluated_block_hash", context),
    `${context}.evaluated_block_hash`,
  );
  const ready = requireOwn(record, "ready", context);
  if (typeof ready !== "boolean") {
    throw new TypeError(`${context}.ready must be boolean`);
  }
  const blockersValue = requireOwn(record, "blockers", context);
  if (!Array.isArray(blockersValue)) {
    throw new TypeError(`${context}.blockers must be an array`);
  }
  const blockers = blockersValue.map((value, index) => {
    const blockerContext = `${context}.blockers[${index}]`;
    const blocker = requireObject(value, blockerContext);
    const code = requireExactString(requireOwn(blocker, "code", blockerContext), `${blockerContext}.code`);
    if (!ERROR_CODE_PATTERN.test(code)) {
      throw new TypeError(`${blockerContext}.code must be a stable lowercase code of 1 to 64 characters`);
    }
    const message = requireHumanMessage(
      requireOwn(blocker, "message", blockerContext),
      `${blockerContext}.message`,
    );
    return { code, message };
  });
  if (ready !== (blockers.length === 0)) {
    throw new TypeError(`${context}.ready must be true exactly when blockers is empty`);
  }
  return {
    asset_definition_id: assetDefinitionId,
    evaluated_block_height: evaluatedBlockHeight,
    evaluated_block_hash: evaluatedBlockHash,
    ready,
    blockers,
  };
}

function normalizeTaggedUnit(value, tagName, allowed, context) {
  const record = requireObject(value, context);
  const tag = requireOwn(record, tagName, context);
  if (typeof tag !== "string" || !allowed.has(tag)) {
    throw new TypeError(`${context}.${tagName} must be one of ${[...allowed].join(", ")}`);
  }
  if (Object.prototype.hasOwnProperty.call(record, "value") && record.value !== null) {
    throw new TypeError(`${context}.value must be null when present`);
  }
  return { [tagName]: tag, value: null };
}

function normalizeOperationKind(value, context) {
  return normalizeTaggedUnit(value, "kind", new Set(["top_up", "redeem"]), context);
}

export function requireOfflineOperationLocation(value, operationId, context = "Location header") {
  const expected = `${OFFLINE_OPERATIONS_PATH}/${requireOfflineOperationId(operationId)}`;
  if (value !== expected) {
    throw new TypeError(`${context} must equal ${expected}`);
  }
  return value;
}

export function normalizeOfflineOperationReference(
  payload,
  { expectedOperationId, expectedKind, location } = {},
) {
  const context = "offline operation reference";
  const record = requireObject(payload, context);
  const operationId = requireOfflineOperationId(
    requireOwn(record, "operation_id", context),
    `${context}.operation_id`,
  );
  if (expectedOperationId !== undefined && operationId !== expectedOperationId) {
    throw new TypeError(`${context}.operation_id does not match the submitted request`);
  }
  const kind = normalizeOperationKind(requireOwn(record, "kind", context), `${context}.kind`);
  if (expectedKind !== undefined && kind.kind !== expectedKind) {
    throw new TypeError(`${context}.kind does not match the submitted command`);
  }
  const state = normalizeTaggedUnit(
    requireOwn(record, "state", context),
    "state",
    new Set(["pending"]),
    `${context}.state`,
  );
  const transactionHash = requireTransactionHash(
    requireOwn(record, "transaction_hash", context),
    `${context}.transaction_hash`,
  );
  const statusUri = requireOfflineOperationLocation(
    requireOwn(record, "status_uri", context),
    operationId,
    `${context}.status_uri`,
  );
  if (location !== undefined) {
    requireOfflineOperationLocation(location, operationId);
  }
  return {
    operation_id: operationId,
    kind,
    state,
    transaction_hash: transactionHash,
    status_uri: statusUri,
    submitted_at_ms: requireUnsignedResponseInteger(
      requireOwn(record, "submitted_at_ms", context),
      `${context}.submitted_at_ms`,
    ),
  };
}

function optionalErrorString(record, field, context) {
  if (!Object.prototype.hasOwnProperty.call(record, field) || record[field] === null) {
    return undefined;
  }
  const value = record[field];
  if (typeof value !== "string") {
    throw new TypeError(`${context}.${field} must be a string`);
  }
  assertWellFormedUnicode(value, `${context}.${field}`);
  return value;
}

function optionalErrorUnsigned(record, field, context, maximum) {
  if (!Object.prototype.hasOwnProperty.call(record, field) || record[field] === null) {
    return undefined;
  }
  return requireUnsignedInteger(record[field], `${context}.${field}`, maximum);
}

function normalizeQueueErrorDetails(value, context) {
  const record = requireObject(value, context);
  const state = requireOwn(record, "state", context);
  if (typeof state !== "string") {
    throw new TypeError(`${context}.state must be a string`);
  }
  assertWellFormedUnicode(state, `${context}.state`);
  const saturated = requireOwn(record, "saturated", context);
  if (typeof saturated !== "boolean") {
    throw new TypeError(`${context}.saturated must be boolean`);
  }
  return {
    state,
    queued: requireUnsignedInteger(
      requireOwn(record, "queued", context),
      `${context}.queued`,
      MAX_U64,
    ),
    capacity: requireUnsignedInteger(
      requireOwn(record, "capacity", context),
      `${context}.capacity`,
      MAX_U64,
    ),
    saturated,
  };
}

function normalizeAxtErrorDetails(value, context) {
  const record = requireObject(value, context);
  const result = {};
  for (const field of ["code", "reason"]) {
    const normalized = optionalErrorString(record, field, context);
    if (normalized !== undefined) result[field] = normalized;
  }
  for (const [field, maximum] of [
    ["snapshot_version", MAX_U64],
    ["dataspace", MAX_U64],
    ["lane", MAX_U32],
    ["next_min_handle_era", MAX_U64],
    ["next_min_sub_nonce", MAX_U64],
  ]) {
    const normalized = optionalErrorUnsigned(record, field, context, maximum);
    if (normalized !== undefined) result[field] = normalized;
  }
  return result;
}

function normalizeErrorDetails(value, context) {
  const record = requireObject(value, context);
  const result = {};
  for (const field of [
    "layer",
    "reject_code",
    "endpoint",
    "field",
    "expected",
    "actual",
    "profile",
    "tx_hash",
    "last_status",
    "hint",
  ]) {
    const normalized = optionalErrorString(record, field, context);
    if (normalized !== undefined) result[field] = normalized;
  }
  for (const [field, maximum] of [
    ["retry_after_seconds", MAX_U64],
    ["chain_discriminant", 0xffffn],
  ]) {
    const normalized = optionalErrorUnsigned(record, field, context, maximum);
    if (normalized !== undefined) result[field] = normalized;
  }
  if (Object.prototype.hasOwnProperty.call(record, "queue") && record.queue !== null) {
    result.queue = normalizeQueueErrorDetails(record.queue, `${context}.queue`);
  }
  if (Object.prototype.hasOwnProperty.call(record, "axt") && record.axt !== null) {
    result.axt = normalizeAxtErrorDetails(record.axt, `${context}.axt`);
  }
  return result;
}

function normalizeErrorEnvelope(value, context) {
    const record = requireObject(value, context);
    const code = requireExactString(requireOwn(record, "code", context), `${context}.code`);
  if (!ERROR_CODE_PATTERN.test(code)) {
    throw new TypeError(`${context}.code must be a stable lowercase code of 1 to 64 characters`);
  }
  const message = requireHumanMessage(
    requireOwn(record, "message", context),
    `${context}.message`,
  );
  const result = { code, message };
  if (record.details !== undefined && record.details !== null) {
    result.details = normalizeErrorDetails(record.details, `${context}.details`);
  }
  return result;
}

function normalizeOperationResult(value, context) {
  const record = requireObject(value, context);
  const kind = requireOwn(record, "kind", context);
  if (kind !== "top_up" && kind !== "redeem") {
    throw new TypeError(`${context}.kind must be top_up or redeem`);
  }
  const resultContext = `${context}.result`;
  const rawResult = requireObject(requireOwn(record, "result", context), resultContext);
  const transactionHash = requireTransactionHash(
    requireOwn(rawResult, "transaction_hash", resultContext),
    `${resultContext}.transaction_hash`,
  );
  const result = {
    transaction_hash: transactionHash,
    finalized_block_height: requireUnsignedResponseInteger(
      requireOwn(rawResult, "finalized_block_height", resultContext),
      `${resultContext}.finalized_block_height`,
      { positive: true },
    ),
    server_time_ms: requireUnsignedResponseInteger(
      requireOwn(rawResult, "server_time_ms", resultContext),
      `${resultContext}.server_time_ms`,
      { positive: true },
    ),
  };
  if (kind === "top_up") {
    result.anchor = cloneResponseJson(
      requireObject(requireOwn(rawResult, "anchor", resultContext), `${resultContext}.anchor`),
      `${resultContext}.anchor`,
    );
  } else if (Object.prototype.hasOwnProperty.call(rawResult, "anchor")) {
    throw new TypeError(`${resultContext}.anchor is invalid for a redeem result`);
  }
  return { kind, result };
}

export function normalizeOfflineOperationStatus(payload, expectedOperationId) {
  const context = "offline operation status";
  const operationId = requireOfflineOperationId(expectedOperationId);
  const record = requireObject(payload, context);
  const state = requireOwn(record, "state", context);
  if (state !== "pending" && state !== "applied" && state !== "rejected") {
    throw new TypeError(`${context}.state must be pending, applied, or rejected`);
  }
  const valueContext = `${context}.value`;
  const rawValue = requireObject(requireOwn(record, "value", context), valueContext);
  const returnedOperationId = requireOfflineOperationId(
    requireOwn(rawValue, "operation_id", valueContext),
    `${valueContext}.operation_id`,
  );
  if (returnedOperationId !== operationId) {
    throw new TypeError(`${valueContext}.operation_id does not match the requested operation`);
  }
  if (state === "pending") {
    return {
      state,
      value: {
        operation_id: returnedOperationId,
        kind: normalizeOperationKind(requireOwn(rawValue, "kind", valueContext), `${valueContext}.kind`),
        transaction_hash: requireTransactionHash(
          requireOwn(rawValue, "transaction_hash", valueContext),
          `${valueContext}.transaction_hash`,
        ),
        submitted_at_ms: requireUnsignedResponseInteger(
          requireOwn(rawValue, "submitted_at_ms", valueContext),
          `${valueContext}.submitted_at_ms`,
        ),
      },
    };
  }
  if (state === "applied") {
    return {
      state,
      value: {
        operation_id: returnedOperationId,
        result: normalizeOperationResult(requireOwn(rawValue, "result", valueContext), `${valueContext}.result`),
      },
    };
  }
  return {
    state,
    value: {
      operation_id: returnedOperationId,
      kind: normalizeOperationKind(requireOwn(rawValue, "kind", valueContext), `${valueContext}.kind`),
      transaction_hash: requireTransactionHash(
        requireOwn(rawValue, "transaction_hash", valueContext),
        `${valueContext}.transaction_hash`,
      ),
      error: normalizeErrorEnvelope(requireOwn(rawValue, "error", valueContext), `${valueContext}.error`),
    },
  };
}
