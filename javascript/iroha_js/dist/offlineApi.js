/** First-release Torii Offline HTTP contract helpers. */

import {
  normalizeAssetAliasFqn,
  normalizeAssetDefinitionId,
} from "./normalizers.js";

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
const MAX_OFFLINE_ASSET_SCALE = 28n;
const MAX_JSON_DEPTH = 128;
const JSON_INTEGER_TOKEN_PATTERN = /^-?(?:0|[1-9][0-9]*)$/u;
const PARSED_NUMBER_LEXEMES = new WeakMap();

class ParsedOfflineJsonNumber {
  constructor(value, token) {
    this.value = value;
    this.token = token;
  }
}

function materializeParsedJsonValue(container, key, parsed) {
  if (!(parsed instanceof ParsedOfflineJsonNumber)) return parsed;
  let lexemes = PARSED_NUMBER_LEXEMES.get(container);
  if (lexemes === undefined) {
    lexemes = new Map();
    PARSED_NUMBER_LEXEMES.set(container, lexemes);
  }
  lexemes.set(key, parsed.token);
  return parsed.value;
}

function requireIntegerJsonToken(container, key, context) {
  const token = PARSED_NUMBER_LEXEMES.get(container)?.get(key);
  if (token !== undefined && !JSON_INTEGER_TOKEN_PATTERN.test(token)) {
    throw new TypeError(
      `${context} must use a JSON integer token without a fraction or exponent`,
    );
  }
}

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
  requireIntegerJsonToken(record, field, `${context}.${field}`);
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
  if (Array.from(message).length > 1024) {
    throw new RangeError(`${context} must not exceed 1024 Unicode characters`);
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

/**
 * Parse a Torii Offline JSON body without rounding wide integer tokens.
 *
 * Number lexemes are retained out-of-band so typed integer fields can reject
 * fractional or exponent-form tokens even when JavaScript would coerce them
 * to an integral Number (for example `1.0` or `1e3`). Unknown members remain
 * ordinary JSON values and cannot affect the typed canonical value.
 */
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
    if (JSON_INTEGER_TOKEN_PATTERN.test(token)) {
      const integer = BigInt(token);
      if (
        integer >= BigInt(Number.MIN_SAFE_INTEGER)
        && integer <= BigInt(Number.MAX_SAFE_INTEGER)
      ) {
        return new ParsedOfflineJsonNumber(Number(token), token);
      }
      return new ParsedOfflineJsonNumber(integer, token);
    }
    const number = Number(token);
    if (!Number.isFinite(number)) syntax("non-finite JSON number");
    return new ParsedOfflineJsonNumber(number, token);
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
        const parsed = parseValue(depth + 1);
        const value = materializeParsedJsonValue(result, key, parsed);
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
        const parsed = parseValue(depth + 1);
        result.push(materializeParsedJsonValue(result, result.length, parsed));
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

  const parsed = parseValue(0);
  const value = parsed instanceof ParsedOfflineJsonNumber ? parsed.value : parsed;
  skipWhitespace();
  if (index !== text.length) syntax("trailing JSON data");
  return value;
}

export function requireOfflineAssetDefinitionId(value, context = "assetDefinitionId") {
  const exact = requireExactString(value, context);
  const normalized = exact.includes("#")
    ? normalizeAssetAliasFqn(exact, context)
    : normalizeAssetDefinitionId(exact, context);
  if (normalized !== exact) {
    throw new TypeError(`${context} must use an exact canonical asset selector`);
  }
  return normalized;
}

function requireUnsignedInteger(value, context, maximum, { positive = false } = {}) {
  let integer;
  if (typeof value === "bigint") {
    integer = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || Object.is(value, -0)) {
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
    if (value < 0n || (positive && value === 0n) || value > MAX_U64) {
      const lower = positive ? 1 : 0;
      throw new RangeError(`${context} must be between ${lower} and ${MAX_U64}`);
    }
    return value;
  }
  if (
    Number.isSafeInteger(value)
    && !Object.is(value, -0)
    && value >= (positive ? 1 : 0)
  ) return value;
  const requirement = positive ? "a positive lossless integer" : "a non-negative lossless integer";
  throw new TypeError(`${context} must be ${requirement}`);
}

function responseIntegerAsBigInt(value) {
  return typeof value === "bigint" ? value : BigInt(value);
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
    requireIntegerJsonToken(value, index, `${context}[${index}]`);
    const byte = value[index];
    if (!Number.isInteger(byte) || Object.is(byte, -0) || byte < 0 || byte > 255) {
      throw new RangeError(`${context}[${index}] must be an integer byte`);
    }
  }
  return value;
}

function normalizeFixedBytes(value, context, { nonZero = false } = {}) {
  const bytes = requireByteArray(value, context, 32);
  if (nonZero && bytes.every((byte) => byte === 0)) {
    throw new RangeError(`${context} must not be all zero`);
  }
  return [...bytes];
}

function fixedBytesEqual(left, right) {
  return left.every((byte, index) => byte === right[index]);
}

function compareFixedBytes(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
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
    if (!Number.isSafeInteger(value) || Object.is(value, -0) || value < 0) {
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
        requireIntegerJsonToken(value, index, `${context}[${index}]`);
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
      requireIntegerJsonToken(value, key, `${context}.${key}`);
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

function normalizeScaledAmount(value, context) {
  const amount = requireObject(value, context);
  const atomicUnits = requireUnsignedInteger(requireOwn(amount, "atomic_units", context), `${context}.atomic_units`, MAX_U128, {
    positive: true,
  });
  const scale = requireUnsignedInteger(
    requireOwn(amount, "scale", context),
    `${context}.scale`,
    MAX_OFFLINE_ASSET_SCALE,
  );
  return { atomic_units: atomicUnits, scale };
}

function validateScaledAmount(value, context) {
  normalizeScaledAmount(value, context);
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
    if (
      new TextEncoder().encode(generation).length > 128
      || /[\u0000-\u001f\u007f-\u009f]/u.test(generation)
    ) {
      throw new RangeError(
        `${context}.artifact_generation must be at most 128 non-control UTF-8 bytes`,
      );
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

function normalizeOfflineVerifierId(value, context) {
  const record = requireObject(value, context);
  const backend = requireExactString(requireOwn(record, "backend", context), `${context}.backend`);
  const name = requireExactString(requireOwn(record, "name", context), `${context}.name`);
  if (Array.from(backend).length > 256 || Array.from(name).length > 256) {
    throw new RangeError(`${context} backend and name must not exceed 256 Unicode characters`);
  }
  return { backend, name };
}

function normalizeActiveTransferVerifier(value, evaluatedBlockHeight, context) {
  const record = requireObject(value, context);
  const id = normalizeOfflineVerifierId(requireOwn(record, "id", context), `${context}.id`);
  const version = Number(requireUnsignedInteger(
    requireOwn(record, "version", context),
    `${context}.version`,
    MAX_U32,
  ));
  const circuitId = requireExactString(
    requireOwn(record, "circuit_id", context),
    `${context}.circuit_id`,
  );
  const commitment = requireTransactionHash(
    requireOwn(record, "commitment", context),
    `${context}.commitment`,
  );
  const publicInputsSchemaHash = requireTransactionHash(
    requireOwn(record, "public_inputs_schema_hash", context),
    `${context}.public_inputs_schema_hash`,
  );
  const maxProofBytes = Number(requireUnsignedInteger(
    requireOwn(record, "max_proof_bytes", context),
    `${context}.max_proof_bytes`,
    MAX_U32,
    { positive: true },
  ));
  const activationHeight = requireUnsignedInteger(
    requireOwn(record, "activation_height", context),
    `${context}.activation_height`,
    MAX_U64,
  );
  const rawWithdrawalHeight = requireOwn(record, "withdrawal_height", context);
  const withdrawalHeight = rawWithdrawalHeight === null
    ? null
    : requireUnsignedInteger(
      rawWithdrawalHeight,
      `${context}.withdrawal_height`,
      MAX_U64,
      { positive: true },
    );
  const evaluated = responseIntegerAsBigInt(evaluatedBlockHeight);
  if (responseIntegerAsBigInt(activationHeight) > evaluated) {
    throw new RangeError(`${context}.activation_height is after the evaluated block`);
  }
  if (withdrawalHeight !== null && responseIntegerAsBigInt(withdrawalHeight) <= evaluated) {
    throw new RangeError(`${context}.withdrawal_height is not after the evaluated block`);
  }
  return {
    id,
    version,
    circuit_id: circuitId,
    commitment,
    public_inputs_schema_hash: publicInputsSchemaHash,
    max_proof_bytes: maxProofBytes,
    activation_height: activationHeight,
    withdrawal_height: withdrawalHeight,
  };
}

export function normalizeOfflineReadinessResponse(payload, expectedAssetDefinitionId) {
  const context = "offline readiness response";
  const requestedSelector = requireOfflineAssetDefinitionId(
    expectedAssetDefinitionId,
    "requested asset selector",
  );
  const record = requireObject(payload, context);
  const assetDefinitionId = normalizeAssetDefinitionId(
    requireExactString(
      requireOwn(record, "asset_definition_id", context),
      `${context}.asset_definition_id`,
    ),
    `${context}.asset_definition_id`,
  );
  if (!requestedSelector.includes("#") && assetDefinitionId !== requestedSelector) {
    throw new TypeError(`${context}.asset_definition_id does not match the requested asset`);
  }
  const rawAssetScale = requireOwn(record, "asset_scale", context);
  const assetScale = rawAssetScale === null
    ? null
    : Number(requireUnsignedInteger(rawAssetScale, `${context}.asset_scale`, MAX_U32));
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
  const blockerCodes = new Set();
  const blockers = blockersValue.map((value, index) => {
    const blockerContext = `${context}.blockers[${index}]`;
    const blocker = requireObject(value, blockerContext);
    const code = requireExactString(requireOwn(blocker, "code", blockerContext), `${blockerContext}.code`);
    if (!ERROR_CODE_PATTERN.test(code)) {
      throw new TypeError(`${blockerContext}.code must be a stable lowercase code of 1 to 64 characters`);
    }
    if (blockerCodes.has(code)) {
      throw new TypeError(`${context}.blockers must not repeat blocker code ${code}`);
    }
    blockerCodes.add(code);
    const message = requireHumanMessage(
      requireOwn(blocker, "message", blockerContext),
      `${blockerContext}.message`,
    );
    return { code, message };
  });
  const rawActiveTransferVerifier = requireOwn(record, "active_transfer_verifier", context);
  const activeTransferVerifier = rawActiveTransferVerifier === null
    ? null
    : normalizeActiveTransferVerifier(
      rawActiveTransferVerifier,
      evaluatedBlockHeight,
      `${context}.active_transfer_verifier`,
    );
  const rawActiveTopUpShieldVerifier = requireOwn(
    record,
    "active_topup_shield_verifier",
    context,
  );
  const activeTopUpShieldVerifier = rawActiveTopUpShieldVerifier === null
    ? null
    : normalizeActiveTransferVerifier(
      rawActiveTopUpShieldVerifier,
      evaluatedBlockHeight,
      `${context}.active_topup_shield_verifier`,
    );
  if (ready !== (blockers.length === 0)) {
    throw new TypeError(`${context}.ready must be true exactly when blockers is empty`);
  }
  const scaleUnavailable = blockerCodes.has("asset_scale_unavailable");
  if ((assetScale === null) !== scaleUnavailable) {
    throw new TypeError(
      `${context}.asset_scale must be null exactly with asset_scale_unavailable`,
    );
  }
  const scaleUnsupported = blockerCodes.has("asset_scale_unsupported");
  if ((assetScale !== null && BigInt(assetScale) > MAX_OFFLINE_ASSET_SCALE) !== scaleUnsupported) {
    throw new TypeError(
      `${context}.asset_scale_unsupported must reflect whether asset_scale exceeds 28`,
    );
  }
  const verifierUnavailable = blockerCodes.has("transfer_verifier_unavailable");
  if ((activeTransferVerifier === null) !== verifierUnavailable) {
    throw new TypeError(
      `${context}.active_transfer_verifier must be null exactly with transfer_verifier_unavailable`,
    );
  }
  const topUpShieldVerifierUnavailable = blockerCodes.has(
    "topup_shield_verifier_unavailable",
  );
  if ((activeTopUpShieldVerifier === null) !== topUpShieldVerifierUnavailable) {
    throw new TypeError(
      `${context}.active_topup_shield_verifier must be null exactly with topup_shield_verifier_unavailable`,
    );
  }
  if (
    ready
    && (
      assetScale === null
      || BigInt(assetScale) > MAX_OFFLINE_ASSET_SCALE
      || activeTransferVerifier === null
      || activeTopUpShieldVerifier === null
    )
  ) {
    throw new TypeError(`${context}.ready requires an Offline-supported scale and active verifiers`);
  }
  return {
    asset_definition_id: assetDefinitionId,
    asset_scale: assetScale,
    evaluated_block_height: evaluatedBlockHeight,
    evaluated_block_hash: evaluatedBlockHash,
    active_transfer_verifier: activeTransferVerifier,
    active_topup_shield_verifier: activeTopUpShieldVerifier,
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
  return requireUnsignedInteger(
    requireOwn(record, field, context),
    `${context}.${field}`,
    maximum,
  );
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

function fixedBytesHex(bytes) {
  return bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function normalizeSpendableNote(value, context) {
  const record = requireObject(value, context);
  const noteCommitment = normalizeFixedBytes(
    requireOwn(record, "note_commitment", context),
    `${context}.note_commitment`,
    { nonZero: true },
  );
  const spendNullifier = normalizeFixedBytes(
    requireOwn(record, "spend_nullifier", context),
    `${context}.spend_nullifier`,
    { nonZero: true },
  );
  if (fixedBytesEqual(noteCommitment, spendNullifier)) {
    throw new TypeError(`${context}.spend_nullifier must differ from note_commitment`);
  }
  return {
    chain_id: requireHumanMessage(requireOwn(record, "chain_id", context), `${context}.chain_id`),
    asset: requireHumanMessage(requireOwn(record, "asset", context), `${context}.asset`),
    note_commitment: noteCommitment,
    spend_nullifier: spendNullifier,
    amount: normalizeScaledAmount(requireOwn(record, "amount", context), `${context}.amount`),
  };
}

function normalizeVerifierKeyId(value, context) {
  const record = requireObject(value, context);
  const backend = requireHumanMessage(
    requireOwn(record, "backend", context),
    `${context}.backend`,
  );
  const name = requireHumanMessage(requireOwn(record, "name", context), `${context}.name`);
  if (new TextEncoder().encode(backend).length > 256) {
    throw new RangeError(`${context}.backend must contain at most 256 UTF-8 bytes`);
  }
  if (new TextEncoder().encode(name).length > 256) {
    throw new RangeError(`${context}.name must contain at most 256 UTF-8 bytes`);
  }
  return {
    backend,
    name,
  };
}

function normalizeTopUpAnchor(value, context, expected) {
  const record = requireObject(value, context);
  const version = requireUnsignedInteger(
    requireOwn(record, "version", context),
    `${context}.version`,
    0xffffn,
  );
  if (BigInt(version) !== 2n) {
    throw new TypeError(`${context}.version must be 2`);
  }
  const amount = normalizeScaledAmount(
    requireOwn(record, "amount", context),
    `${context}.amount`,
  );
  const assetScale = requireUnsignedInteger(
    requireOwn(record, "asset_scale", context),
    `${context}.asset_scale`,
    MAX_OFFLINE_ASSET_SCALE,
  );
  if (BigInt(assetScale) !== BigInt(amount.scale)) {
    throw new TypeError(`${context}.asset_scale must equal amount.scale`);
  }
  const initialRoot = normalizeFixedBytes(
    requireOwn(record, "initial_root", context),
    `${context}.initial_root`,
    { nonZero: true },
  );
  const finalizedRoot = normalizeFixedBytes(
    requireOwn(record, "finalized_root", context),
    `${context}.finalized_root`,
    { nonZero: true },
  );
  if (fixedBytesEqual(initialRoot, finalizedRoot)) {
    throw new TypeError(`${context}.finalized_root must differ from initial_root`);
  }

  const rawNullifiers = requireOwn(record, "topup_anchor_nullifiers", context);
  if (!Array.isArray(rawNullifiers) || rawNullifiers.length < 1 || rawNullifiers.length > 2) {
    throw new RangeError(`${context}.topup_anchor_nullifiers must contain one or two entries`);
  }
  const topupAnchorNullifiers = rawNullifiers.map((raw, index) =>
    normalizeFixedBytes(
      raw,
      `${context}.topup_anchor_nullifiers[${index}]`,
      { nonZero: true },
    ));
  for (let index = 1; index < topupAnchorNullifiers.length; index += 1) {
    if (compareFixedBytes(topupAnchorNullifiers[index - 1], topupAnchorNullifiers[index]) >= 0) {
      throw new TypeError(
        `${context}.topup_anchor_nullifiers must be strictly sorted and unique`,
      );
    }
  }

  const currentNote = normalizeSpendableNote(
    requireOwn(record, "current_note", context),
    `${context}.current_note`,
  );
  const chainId = requireHumanMessage(
    requireOwn(record, "chain_id", context),
    `${context}.chain_id`,
  );
  if (currentNote.chain_id !== chainId) {
    throw new TypeError(`${context}.current_note.chain_id must equal chain_id`);
  }
  if (
    BigInt(currentNote.amount.atomic_units) !== BigInt(amount.atomic_units)
    || BigInt(currentNote.amount.scale) !== BigInt(amount.scale)
  ) {
    throw new TypeError(`${context}.current_note.amount must equal amount`);
  }
  if (topupAnchorNullifiers.some((nullifier) =>
    fixedBytesEqual(nullifier, currentNote.note_commitment)
    || fixedBytesEqual(nullifier, currentNote.spend_nullifier))) {
    throw new TypeError(`${context}.topup_anchor_nullifiers must not reuse current note material`);
  }

  const topupOperationId = normalizeFixedBytes(
    requireOwn(record, "topup_operation_id", context),
    `${context}.topup_operation_id`,
    { nonZero: true },
  );
  if (fixedBytesHex(topupOperationId) !== expected.operationId) {
    throw new TypeError(`${context}.topup_operation_id does not match the operation`);
  }
  const finalizedHeight = requireUnsignedInteger(
    requireOwn(record, "finalized_height", context),
    `${context}.finalized_height`,
    MAX_U64,
    { positive: true },
  );
  if (BigInt(finalizedHeight) !== BigInt(expected.finalizedBlockHeight)) {
    throw new TypeError(`${context}.finalized_height does not match finalized_block_height`);
  }
  const finalizedTxHash = normalizeFixedBytes(
    requireOwn(record, "finalized_tx_hash", context),
    `${context}.finalized_tx_hash`,
    { nonZero: true },
  );
  if (fixedBytesHex(finalizedTxHash) !== expected.transactionHash) {
    throw new TypeError(`${context}.finalized_tx_hash does not match transaction_hash`);
  }
  const artifactGeneration = requireHumanMessage(
    requireOwn(record, "artifact_generation", context),
    `${context}.artifact_generation`,
  );
  if (new TextEncoder().encode(artifactGeneration).length > 128) {
    throw new RangeError(`${context}.artifact_generation must contain at most 128 UTF-8 bytes`);
  }

  return {
    version,
    chain_id: chainId,
    payer: requireHumanMessage(requireOwn(record, "payer", context), `${context}.payer`),
    asset: requireHumanMessage(requireOwn(record, "asset", context), `${context}.asset`),
    asset_scale: assetScale,
    amount,
    initial_root: initialRoot,
    finalized_root: finalizedRoot,
    topup_anchor_nullifiers: topupAnchorNullifiers,
    current_note: currentNote,
    topup_operation_id: topupOperationId,
    transfer_verifier_id: normalizeVerifierKeyId(
      requireOwn(record, "transfer_verifier_id", context),
      `${context}.transfer_verifier_id`,
    ),
    transfer_verifier_commitment: normalizeFixedBytes(
      requireOwn(record, "transfer_verifier_commitment", context),
      `${context}.transfer_verifier_commitment`,
      { nonZero: true },
    ),
    artifact_generation: artifactGeneration,
    finalized_height: finalizedHeight,
    finalized_tx_hash: finalizedTxHash,
    anchor_digest: normalizeFixedBytes(
      requireOwn(record, "anchor_digest", context),
      `${context}.anchor_digest`,
      { nonZero: true },
    ),
  };
}

function normalizeTopUpFinalityProof(value, context, expected) {
  // Preserve the complete direct proof for the native verifier, while only
  // inspecting the small set of public bindings needed to reject response
  // substitution before cryptographic verification.
  const directProof = snapshotJson(value, context);
  const record = requireObject(directProof, context);
  const version = requireUnsignedInteger(
    requireOwn(record, "version", context),
    `${context}.version`,
    0xffffn,
  );
  if (BigInt(version) !== 1n) {
    throw new TypeError(`${context}.version must be 1`);
  }

  const anchorContext = `${context}.anchor`;
  const proofAnchor = requireObject(requireOwn(record, "anchor", context), anchorContext);
  const topupOperationId = normalizeFixedBytes(
    requireOwn(proofAnchor, "topup_operation_id", anchorContext),
    `${anchorContext}.topup_operation_id`,
    { nonZero: true },
  );
  if (fixedBytesHex(topupOperationId) !== expected.operationId) {
    throw new TypeError(`${anchorContext}.topup_operation_id does not match the operation`);
  }
  const anchorDigest = normalizeFixedBytes(
    requireOwn(proofAnchor, "anchor_digest", anchorContext),
    `${anchorContext}.anchor_digest`,
    { nonZero: true },
  );
  if (!fixedBytesEqual(anchorDigest, expected.anchor.anchor_digest)) {
    throw new TypeError(`${anchorContext}.anchor_digest does not match the finalized anchor`);
  }

  const commitQcContext = `${context}.commit_qc`;
  const commitQc = requireObject(requireOwn(record, "commit_qc", context), commitQcContext);
  const heightContextContext = `${commitQcContext}.height_context`;
  const heightContext = requireObject(
    requireOwn(commitQc, "height_context", commitQcContext),
    heightContextContext,
  );
  const contextHeight = requireUnsignedResponseInteger(
    requireOwn(heightContext, "height", heightContextContext),
    `${heightContextContext}.height`,
    { positive: true },
  );
  if (BigInt(contextHeight) !== BigInt(expected.finalizedBlockHeight)) {
    throw new TypeError(
      `${heightContextContext}.height does not match finalized_block_height`,
    );
  }

  const certificateContext = `${commitQcContext}.certificate`;
  const certificate = requireObject(
    requireOwn(commitQc, "certificate", commitQcContext),
    certificateContext,
  );
  const roundContext = `${certificateContext}.round`;
  const round = requireObject(requireOwn(certificate, "round", certificateContext), roundContext);
  const certificateHeight = requireUnsignedResponseInteger(
    requireOwn(round, "height", roundContext),
    `${roundContext}.height`,
    { positive: true },
  );
  if (BigInt(certificateHeight) !== BigInt(expected.finalizedBlockHeight)) {
    throw new TypeError(`${roundContext}.height does not match finalized_block_height`);
  }

  requireObject(requireOwn(record, "anchor_path", context), `${context}.anchor_path`);
  return directProof;
}

function normalizeOperationResult(value, context, operationId) {
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
    result.anchor = normalizeTopUpAnchor(
      requireOwn(rawResult, "anchor", resultContext),
      `${resultContext}.anchor`,
      {
        operationId,
        transactionHash,
        finalizedBlockHeight: result.finalized_block_height,
      },
    );
    result.finality_proof = normalizeTopUpFinalityProof(
      requireOwn(rawResult, "finality_proof", resultContext),
      `${resultContext}.finality_proof`,
      {
        operationId,
        anchor: result.anchor,
        finalizedBlockHeight: result.finalized_block_height,
      },
    );
  } else {
    for (const topUpOnlyField of ["anchor", "finality_proof"]) {
      if (Object.prototype.hasOwnProperty.call(rawResult, topUpOnlyField)) {
        throw new TypeError(
          `${resultContext}.${topUpOnlyField} is invalid for a redeem result`,
        );
      }
    }
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
        result: normalizeOperationResult(
          requireOwn(rawValue, "result", valueContext),
          `${valueContext}.result`,
          returnedOperationId,
        ),
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
