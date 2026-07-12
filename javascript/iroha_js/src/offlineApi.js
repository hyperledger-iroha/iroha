/** First-release Torii Offline HTTP contract helpers. */

import {
  normalizeAssetAliasFqn,
  normalizeAssetDefinitionId,
} from "./normalizers.js";
import { blake2b256 } from "./blake2b.js";

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
const MAX_TOP_UP_ANCHORS_PER_BLOCK = 16n;
const MAX_TOP_UP_FINALITY_SIBLINGS = 4;
const MAX_TOP_UP_SHIELD_LEAVES = 65_536n;
const MAX_TOP_UP_SHIELD_PROOF_BYTES = 192 * 1024;
const MAX_FINALITY_VALIDATORS = 4_096;
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

function requireClosedFields(record, required, optional, context) {
  const allowed = new Set([...required, ...optional]);
  for (const key of Reflect.ownKeys(record)) {
    if (typeof key !== "string") {
      if (Object.prototype.propertyIsEnumerable.call(record, key)) {
        throw new TypeError(`${context} must not contain enumerable symbol keys`);
      }
      continue;
    }
    if (Object.prototype.propertyIsEnumerable.call(record, key) && !allowed.has(key)) {
      throw new TypeError(`${context}.${key} is not part of the first-release contract`);
    }
  }
  for (const field of required) requireOwn(record, field, context);
  return record;
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

function isPortableVerifierIdField(value) {
  if (typeof value !== "string" || value.length === 0) return false;
  if (new TextEncoder().encode(value).length > 256) return false;
  if (!/^[a-z0-9][a-z0-9_/:.-]*[a-z0-9]$/u.test(value) && !/^[a-z0-9]$/u.test(value)) {
    return false;
  }
  return !["..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:"]
    .some((separator) => value.includes(separator));
}

function markedBlake2b256(bytes) {
  const digest = [...blake2b256(Uint8Array.from(bytes))];
  digest[digest.length - 1] |= 1;
  return digest;
}

function validateTopUpShieldProofAttachment(value, context) {
  const attachment = requireClosedFields(
    requireObject(value, context),
    ["backend", "proof", "vk_ref"],
    ["vk_commitment", "envelope_hash", "lane_privacy"],
    context,
  );
  const backend = requireExactString(requireOwn(attachment, "backend", context), `${context}.backend`);
  if (backend !== "halo2/ipa") {
    throw new TypeError(`${context}.backend must be halo2/ipa`);
  }

  const proofContext = `${context}.proof`;
  const proof = requireClosedFields(
    requireObject(requireOwn(attachment, "proof", context), proofContext),
    ["backend", "bytes"],
    [],
    proofContext,
  );
  if (requireExactString(requireOwn(proof, "backend", proofContext), `${proofContext}.backend`) !== backend) {
    throw new TypeError(`${proofContext}.backend must match ${context}.backend`);
  }
  const proofBytes = requireByteArray(requireOwn(proof, "bytes", proofContext), `${proofContext}.bytes`);
  if (proofBytes.length === 0 || proofBytes.length > MAX_TOP_UP_SHIELD_PROOF_BYTES) {
    throw new RangeError(
      `${proofContext}.bytes must contain between 1 and ${MAX_TOP_UP_SHIELD_PROOF_BYTES} bytes`,
    );
  }

  const verifierContext = `${context}.vk_ref`;
  const verifier = requireClosedFields(
    requireObject(requireOwn(attachment, "vk_ref", context), verifierContext),
    ["backend", "name"],
    [],
    verifierContext,
  );
  const verifierBackend = requireExactString(
    requireOwn(verifier, "backend", verifierContext),
    `${verifierContext}.backend`,
  );
  const verifierName = requireExactString(
    requireOwn(verifier, "name", verifierContext),
    `${verifierContext}.name`,
  );
  if (verifierBackend !== backend) {
    throw new TypeError(`${verifierContext}.backend must match ${context}.backend`);
  }
  if (!isPortableVerifierIdField(verifierBackend) || !isPortableVerifierIdField(verifierName)) {
    throw new TypeError(`${verifierContext} must use portable registry syntax`);
  }

  if (!Object.prototype.hasOwnProperty.call(attachment, "vk_commitment")
      || attachment.vk_commitment === null) {
    throw new TypeError(`${context}.vk_commitment is required for top-up shield evidence`);
  }
  normalizeFixedBytes(attachment.vk_commitment, `${context}.vk_commitment`, { nonZero: true });

  if (Object.prototype.hasOwnProperty.call(attachment, "envelope_hash")
      && attachment.envelope_hash !== null) {
    const envelopeHash = normalizeFixedBytes(
      attachment.envelope_hash,
      `${context}.envelope_hash`,
      { nonZero: true },
    );
    if (!fixedBytesEqual(envelopeHash, markedBlake2b256(proofBytes))) {
      throw new TypeError(`${context}.envelope_hash must match proof bytes`);
    }
  }
  if (Object.prototype.hasOwnProperty.call(attachment, "lane_privacy")
      && attachment.lane_privacy !== null) {
    throw new TypeError(`${context}.lane_privacy is not valid for top-up shield evidence`);
  }
}

function validateTopUpShieldEvidence(value, context) {
  const evidence = requireClosedFields(
    requireObject(value, context),
    ["initial_root", "finalized_root", "leaf_index", "proof"],
    [],
    context,
  );
  const initialRoot = normalizeFixedBytes(
    requireOwn(evidence, "initial_root", context),
    `${context}.initial_root`,
    { nonZero: true },
  );
  const finalizedRoot = normalizeFixedBytes(
    requireOwn(evidence, "finalized_root", context),
    `${context}.finalized_root`,
    { nonZero: true },
  );
  if (fixedBytesEqual(initialRoot, finalizedRoot)) {
    throw new TypeError(`${context}.finalized_root must differ from initial_root`);
  }
  requireUnsignedInteger(
    requireOwn(evidence, "leaf_index", context),
    `${context}.leaf_index`,
    MAX_TOP_UP_SHIELD_LEAVES - 1n,
  );
  validateTopUpShieldProofAttachment(requireOwn(evidence, "proof", context), `${context}.proof`);
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
    requireClosedFields(
      request,
      [
        "asset",
        "amount",
        "current_note",
        "shield_evidence",
        "artifact_generation",
        "operation_id",
        "authorization",
      ],
      [],
      context,
    );
    requireExactString(requireOwn(request, "asset", context), `${context}.asset`);
    const amount = normalizeScaledAmount(requireOwn(request, "amount", context), `${context}.amount`);
    const currentNote = normalizeSpendableNote(
      requireOwn(request, "current_note", context),
      `${context}.current_note`,
    );
    if (BigInt(currentNote.amount.atomic_units) !== BigInt(amount.atomic_units)
        || BigInt(currentNote.amount.scale) !== BigInt(amount.scale)) {
      throw new TypeError(`${context}.current_note.amount must equal amount`);
    }
    validateTopUpShieldEvidence(
      requireOwn(request, "shield_evidence", context),
      `${context}.shield_evidence`,
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

function normalizeActiveTransferVerifier(value, evaluatedBlockHeight, context) {
  const record = requireObject(value, context);
  const id = normalizeVerifierKeyId(requireOwn(record, "id", context), `${context}.id`);
  if (id.name.includes(":")) {
    throw new TypeError(`${context}.id.name must not contain ':' characters`);
  }
  const version = Number(requireUnsignedInteger(
    requireOwn(record, "version", context),
    `${context}.version`,
    MAX_U32,
  ));
  const circuitId = requireHumanMessage(
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
  if (
    withdrawalHeight !== null
    && BigInt(withdrawalHeight) <= BigInt(activationHeight)
  ) {
    throw new TypeError(
      `${context}.withdrawal_height must be greater than activation_height`,
    );
  }
  const evaluatedHeight = BigInt(evaluatedBlockHeight);
  if (
    BigInt(activationHeight) > evaluatedHeight
    || (withdrawalHeight !== null && evaluatedHeight >= BigInt(withdrawalHeight))
  ) {
    throw new TypeError(`${context} must be active at evaluated_block_height`);
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

  const shieldLeafIndex = requireUnsignedInteger(
    requireOwn(record, "shield_leaf_index", context),
    `${context}.shield_leaf_index`,
    MAX_TOP_UP_SHIELD_LEAVES - 1n,
  );

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
    shield_leaf_index: shieldLeafIndex,
    current_note: currentNote,
    topup_operation_id: topupOperationId,
    shield_verifier_id: normalizeVerifierKeyId(
      requireOwn(record, "shield_verifier_id", context),
      `${context}.shield_verifier_id`,
    ),
    shield_verifier_commitment: normalizeFixedBytes(
      requireOwn(record, "shield_verifier_commitment", context),
      `${context}.shield_verifier_commitment`,
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

function crc16CcittAscii(value) {
  let crc = 0xffff;
  for (let index = 0; index < value.length; index += 1) {
    crc ^= value.charCodeAt(index) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 0x8000) === 0
        ? (crc << 1) & 0xffff
        : ((crc << 1) ^ 0x1021) & 0xffff;
    }
  }
  return crc;
}

function normalizeHashLiteral(value, context) {
  const literal = requireExactString(value, context);
  const match = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(literal);
  if (match === null) {
    throw new TypeError(`${context} must be a canonical uppercase Norito hash literal`);
  }
  const [, body, checksum] = match;
  if ((Number.parseInt(body.slice(-2), 16) & 1) !== 1) {
    throw new TypeError(`${context} must carry the Iroha hash marker bit`);
  }
  const expectedChecksum = crc16CcittAscii(`hash:${body}`)
    .toString(16)
    .toUpperCase()
    .padStart(4, "0");
  if (checksum !== expectedChecksum) {
    throw new TypeError(`${context} has an invalid hash checksum`);
  }
  return literal;
}

function normalizeHeightContextId(value, context) {
  if (!Array.isArray(value) || value.length !== 1) {
    throw new TypeError(`${context} must be a single-field tuple`);
  }
  return [normalizeHashLiteral(value[0], `${context}[0]`)];
}

function normalizeFinalityUnitTag(value, context, tag, allowed) {
  const record = requireClosedFields(
    requireObject(value, context),
    [tag, "details"],
    [],
    context,
  );
  const selected = requireExactString(requireOwn(record, tag, context), `${context}.${tag}`);
  if (!allowed.includes(selected)) {
    throw new TypeError(`${context}.${tag} must be one of ${allowed.join(", ")}`);
  }
  if (requireOwn(record, "details", context) !== null) {
    throw new TypeError(`${context}.details must be null`);
  }
  return { [tag]: selected, details: null };
}

function normalizeFinalityDaLayout(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    [
      "encoding",
      "chunk_size_bytes",
      "data_shards",
      "parity_shards",
      "max_payload_size_bytes",
      "max_chunk_count",
    ],
    [],
    context,
  );
  const encoding = normalizeFinalityUnitTag(
    requireOwn(record, "encoding", context),
    `${context}.encoding`,
    "encoding",
    ["plain", "reed_solomon16"],
  );
  const chunkSizeBytes = requireUnsignedInteger(
    requireOwn(record, "chunk_size_bytes", context),
    `${context}.chunk_size_bytes`,
    MAX_U32,
    { positive: true },
  );
  const dataShards = requireUnsignedInteger(
    requireOwn(record, "data_shards", context),
    `${context}.data_shards`,
    0xffffn,
  );
  const parityShards = requireUnsignedInteger(
    requireOwn(record, "parity_shards", context),
    `${context}.parity_shards`,
    0xffffn,
  );
  const maxPayloadSizeBytes = requireUnsignedResponseInteger(
    requireOwn(record, "max_payload_size_bytes", context),
    `${context}.max_payload_size_bytes`,
    { positive: true },
  );
  const maxChunkCount = requireUnsignedInteger(
    requireOwn(record, "max_chunk_count", context),
    `${context}.max_chunk_count`,
    MAX_U32,
    { positive: true },
  );
  if (encoding.encoding === "plain") {
    if (BigInt(dataShards) !== 0n || BigInt(parityShards) !== 0n) {
      throw new TypeError(`${context} plain encoding must use zero data and parity shards`);
    }
  } else if (BigInt(dataShards) === 0n || BigInt(parityShards) === 0n) {
    throw new TypeError(`${context} reed_solomon16 encoding requires data and parity shards`);
  }
  return {
    encoding,
    chunk_size_bytes: chunkSizeBytes,
    data_shards: dataShards,
    parity_shards: parityShards,
    max_payload_size_bytes: maxPayloadSizeBytes,
    max_chunk_count: maxChunkCount,
  };
}

function normalizeFinalityValidatorPower(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["validator", "power"],
    [],
    context,
  );
  const validator = requireExactString(
    requireOwn(record, "validator", context),
    `${context}.validator`,
  );
  if (!/^ea0130[0-9A-F]{96}$/u.test(validator)) {
    throw new TypeError(`${context}.validator must be a canonical BLS-normal peer id`);
  }
  return {
    validator,
    power: requireUnsignedResponseInteger(
      requireOwn(record, "power", context),
      `${context}.power`,
      { positive: true },
    ),
  };
}

function normalizeFinalityQuorum(value, context, roster) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["min_signers", "total_power"],
    [],
    context,
  );
  const minSigners = requireUnsignedInteger(
    requireOwn(record, "min_signers", context),
    `${context}.min_signers`,
    BigInt(MAX_FINALITY_VALIDATORS),
    { positive: true },
  );
  const totalPower = requireUnsignedResponseInteger(
    requireOwn(record, "total_power", context),
    `${context}.total_power`,
    { positive: true },
  );
  const expectedMinSigners = BigInt(Math.floor(roster.length * 2 / 3) + 1);
  let expectedTotalPower = 0n;
  for (const entry of roster) expectedTotalPower += BigInt(entry.power);
  if (BigInt(minSigners) !== expectedMinSigners || BigInt(totalPower) !== expectedTotalPower) {
    throw new TypeError(`${context} must equal the canonical quorum derived from roster`);
  }
  return { min_signers: minSigners, total_power: totalPower };
}

function normalizeFinalityNextEpochSnapshot(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    [
      "epoch",
      "epoch_end_height",
      "mode",
      "roster",
      "validator_set_pops",
      "quorum",
      "leader_seed",
    ],
    [],
    context,
  );
  const rawRoster = requireOwn(record, "roster", context);
  if (!Array.isArray(rawRoster)
      || rawRoster.length === 0
      || rawRoster.length > MAX_FINALITY_VALIDATORS) {
    throw new RangeError(`${context}.roster must contain 1 to ${MAX_FINALITY_VALIDATORS} validators`);
  }
  const roster = rawRoster.map((entry, index) =>
    normalizeFinalityValidatorPower(entry, `${context}.roster[${index}]`));
  for (let index = 1; index < roster.length; index += 1) {
    if (roster[index - 1].validator >= roster[index].validator) {
      throw new TypeError(`${context}.roster must be strictly ordered and unique`);
    }
  }
  const rawPops = requireOwn(record, "validator_set_pops", context);
  if (!Array.isArray(rawPops) || rawPops.length !== roster.length) {
    throw new TypeError(`${context}.validator_set_pops must align one-for-one with roster`);
  }
  const validatorSetPops = rawPops.map((proof, index) => [
    ...requireByteArray(proof, `${context}.validator_set_pops[${index}]`, 96),
  ]);
  return {
    epoch: requireUnsignedResponseInteger(
      requireOwn(record, "epoch", context),
      `${context}.epoch`,
      { positive: true },
    ),
    epoch_end_height: requireUnsignedResponseInteger(
      requireOwn(record, "epoch_end_height", context),
      `${context}.epoch_end_height`,
      { positive: true },
    ),
    mode: normalizeFinalityUnitTag(
      requireOwn(record, "mode", context),
      `${context}.mode`,
      "mode",
      ["permissioned", "npos"],
    ),
    roster,
    validator_set_pops: validatorSetPops,
    quorum: normalizeFinalityQuorum(requireOwn(record, "quorum", context), `${context}.quorum`, roster),
    leader_seed: [...requireByteArray(
      requireOwn(record, "leader_seed", context),
      `${context}.leader_seed`,
      32,
    )],
  };
}

function normalizeFinalityRound(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["context_id", "height", "view"],
    [],
    context,
  );
  return {
    context_id: normalizeHeightContextId(requireOwn(record, "context_id", context), `${context}.context_id`),
    height: requireUnsignedResponseInteger(
      requireOwn(record, "height", context),
      `${context}.height`,
      { positive: true },
    ),
    view: requireUnsignedResponseInteger(
      requireOwn(record, "view", context),
      `${context}.view`,
    ),
  };
}

function normalizeFinalityBlockSubject(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["block_hash", "payload_hash"],
    ["parent_block_hash"],
    context,
  );
  const result = {
    block_hash: normalizeHashLiteral(requireOwn(record, "block_hash", context), `${context}.block_hash`),
    payload_hash: normalizeHashLiteral(requireOwn(record, "payload_hash", context), `${context}.payload_hash`),
  };
  if (Object.prototype.hasOwnProperty.call(record, "parent_block_hash")) {
    result.parent_block_hash = normalizeHashLiteral(record.parent_block_hash, `${context}.parent_block_hash`);
  }
  return result;
}

function normalizeFinalityExecutionCommitment(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["parent_state_root", "post_state_root", "ordinary_writes_root", "topup_anchor_count"],
    ["topup_anchor_root"],
    context,
  );
  const topupAnchorCount = requireUnsignedInteger(
    requireOwn(record, "topup_anchor_count", context),
    `${context}.topup_anchor_count`,
    MAX_TOP_UP_ANCHORS_PER_BLOCK,
  );
  const hasTopUpRoot = Object.prototype.hasOwnProperty.call(record, "topup_anchor_root");
  if ((BigInt(topupAnchorCount) === 0n) === hasTopUpRoot) {
    throw new TypeError(`${context}.topup_anchor_root must be present exactly when count is non-zero`);
  }
  const result = {
    parent_state_root: normalizeHashLiteral(
      requireOwn(record, "parent_state_root", context),
      `${context}.parent_state_root`,
    ),
    post_state_root: normalizeHashLiteral(
      requireOwn(record, "post_state_root", context),
      `${context}.post_state_root`,
    ),
    ordinary_writes_root: normalizeHashLiteral(
      requireOwn(record, "ordinary_writes_root", context),
      `${context}.ordinary_writes_root`,
    ),
    topup_anchor_count: topupAnchorCount,
  };
  if (hasTopUpRoot) {
    result.topup_anchor_root = normalizeHashLiteral(
      record.topup_anchor_root,
      `${context}.topup_anchor_root`,
    );
  }
  return result;
}

function normalizeFinalityQuorumCertificate(value, context, expected = {}) {
  const record = requireClosedFields(
    requireObject(value, context),
    ["round", "phase", "subject", "execution_commitment", "signers", "aggregate_signature"],
    [],
    context,
  );
  const round = normalizeFinalityRound(requireOwn(record, "round", context), `${context}.round`);
  const phase = normalizeFinalityUnitTag(
    requireOwn(record, "phase", context),
    `${context}.phase`,
    "phase",
    ["commit"],
  );
  const executionCommitment = normalizeFinalityExecutionCommitment(
    requireOwn(record, "execution_commitment", context),
    `${context}.execution_commitment`,
  );
  if (expected.height !== undefined && BigInt(round.height) !== BigInt(expected.height)) {
    throw new TypeError(`${context}.round.height does not match the height context`);
  }
  if (expected.contextId !== undefined && round.context_id[0] !== expected.contextId[0]) {
    throw new TypeError(`${context}.round.context_id does not match the height context`);
  }
  if (expected.requireTopUps === true && BigInt(executionCommitment.topup_anchor_count) === 0n) {
    throw new TypeError(`${context}.execution_commitment must authenticate at least one top-up`);
  }
  const rawSigners = requireOwn(record, "signers", context);
  if (!Array.isArray(rawSigners)
      || rawSigners.length === 0
      || rawSigners.length > MAX_FINALITY_VALIDATORS) {
    throw new RangeError(`${context}.signers must contain 1 to ${MAX_FINALITY_VALIDATORS} entries`);
  }
  const signers = rawSigners.map((signer, index) =>
    requireUnsignedInteger(signer, `${context}.signers[${index}]`, MAX_U32));
  for (let index = 1; index < signers.length; index += 1) {
    if (BigInt(signers[index - 1]) >= BigInt(signers[index])) {
      throw new TypeError(`${context}.signers must be strictly increasing and unique`);
    }
  }
  return {
    round,
    phase,
    subject: normalizeFinalityBlockSubject(requireOwn(record, "subject", context), `${context}.subject`),
    execution_commitment: executionCommitment,
    signers,
    aggregate_signature: [...requireByteArray(
      requireOwn(record, "aggregate_signature", context),
      `${context}.aggregate_signature`,
      96,
    )],
  };
}

function normalizeTopUpFinalityHeightContext(value, context) {
  const record = requireClosedFields(
    requireObject(value, context),
    [
      "context_id",
      "chain_id",
      "protocol_version",
      "height",
      "epoch",
      "epoch_end_height",
      "mode",
      "nexus_amx_context_hash",
      "da_layout",
      "leader_seed",
    ],
    ["next_epoch_snapshot", "parent_commit_qc"],
    context,
  );
  const contextId = normalizeHeightContextId(
    requireOwn(record, "context_id", context),
    `${context}.context_id`,
  );
  const chainId = requireHumanMessage(requireOwn(record, "chain_id", context), `${context}.chain_id`);
  if (new TextEncoder().encode(chainId).length > 128) {
    throw new RangeError(`${context}.chain_id must contain at most 128 UTF-8 bytes`);
  }
  const protocolVersion = requireUnsignedInteger(
    requireOwn(record, "protocol_version", context),
    `${context}.protocol_version`,
    0xffffn,
  );
  if (BigInt(protocolVersion) !== 2n) {
    throw new TypeError(`${context}.protocol_version must be 2`);
  }
  const height = requireUnsignedResponseInteger(
    requireOwn(record, "height", context),
    `${context}.height`,
    { positive: true },
  );
  const epoch = requireUnsignedResponseInteger(
    requireOwn(record, "epoch", context),
    `${context}.epoch`,
  );
  const epochEndHeight = requireUnsignedResponseInteger(
    requireOwn(record, "epoch_end_height", context),
    `${context}.epoch_end_height`,
    { positive: true },
  );
  if (BigInt(epochEndHeight) < BigInt(height)) {
    throw new TypeError(`${context}.epoch_end_height must not precede height`);
  }
  const result = {
    context_id: contextId,
    chain_id: chainId,
    protocol_version: protocolVersion,
    height,
    epoch,
    epoch_end_height: epochEndHeight,
    mode: normalizeFinalityUnitTag(
      requireOwn(record, "mode", context),
      `${context}.mode`,
      "mode",
      ["permissioned", "npos"],
    ),
    nexus_amx_context_hash: normalizeHashLiteral(
      requireOwn(record, "nexus_amx_context_hash", context),
      `${context}.nexus_amx_context_hash`,
    ),
    da_layout: normalizeFinalityDaLayout(
      requireOwn(record, "da_layout", context),
      `${context}.da_layout`,
    ),
    leader_seed: [...requireByteArray(
      requireOwn(record, "leader_seed", context),
      `${context}.leader_seed`,
      32,
    )],
  };
  if (Object.prototype.hasOwnProperty.call(record, "next_epoch_snapshot")) {
    result.next_epoch_snapshot = normalizeFinalityNextEpochSnapshot(
      record.next_epoch_snapshot,
      `${context}.next_epoch_snapshot`,
    );
  }
  if (Object.prototype.hasOwnProperty.call(record, "parent_commit_qc")) {
    result.parent_commit_qc = normalizeFinalityQuorumCertificate(
      record.parent_commit_qc,
      `${context}.parent_commit_qc`,
    );
  }
  return result;
}

function normalizeTopUpFinalityProof(value, context, expected) {
  const record = requireClosedFields(
    requireObject(snapshotJson(value, context), context),
    ["version", "anchor", "commit_qc", "anchor_path"],
    [],
    context,
  );
  const version = requireUnsignedInteger(
    requireOwn(record, "version", context),
    `${context}.version`,
    0xffffn,
  );
  if (BigInt(version) !== 1n) {
    throw new TypeError(`${context}.version must be 1`);
  }

  const anchorContext = `${context}.anchor`;
  const proofAnchor = requireClosedFields(
    requireObject(requireOwn(record, "anchor", context), anchorContext),
    ["topup_operation_id", "anchor_digest"],
    [],
    anchorContext,
  );
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
  const rawCommitQc = requireClosedFields(
    requireObject(requireOwn(record, "commit_qc", context), commitQcContext),
    ["height_context", "certificate"],
    [],
    commitQcContext,
  );
  const heightContext = normalizeTopUpFinalityHeightContext(
    requireOwn(rawCommitQc, "height_context", commitQcContext),
    `${commitQcContext}.height_context`,
  );
  if (BigInt(heightContext.height) !== BigInt(expected.finalizedBlockHeight)) {
    throw new TypeError(`${commitQcContext}.height_context.height does not match finalized_block_height`);
  }
  const certificate = normalizeFinalityQuorumCertificate(
    requireOwn(rawCommitQc, "certificate", commitQcContext),
    `${commitQcContext}.certificate`,
    { height: heightContext.height, contextId: heightContext.context_id, requireTopUps: true },
  );

  const anchorPathContext = `${context}.anchor_path`;
  const rawAnchorPath = requireClosedFields(
    requireObject(requireOwn(record, "anchor_path", context), anchorPathContext),
    ["leaf_index", "leaf_count", "siblings"],
    [],
    anchorPathContext,
  );
  const leafIndex = requireUnsignedInteger(
    requireOwn(rawAnchorPath, "leaf_index", anchorPathContext),
    `${anchorPathContext}.leaf_index`,
    MAX_TOP_UP_ANCHORS_PER_BLOCK - 1n,
  );
  const leafCount = requireUnsignedInteger(
    requireOwn(rawAnchorPath, "leaf_count", anchorPathContext),
    `${anchorPathContext}.leaf_count`,
    MAX_TOP_UP_ANCHORS_PER_BLOCK,
    { positive: true },
  );
  if (BigInt(leafIndex) >= BigInt(leafCount)) {
    throw new TypeError(`${anchorPathContext}.leaf_index must be less than leaf_count`);
  }
  if (BigInt(leafCount) !== BigInt(certificate.execution_commitment.topup_anchor_count)) {
    throw new TypeError(`${anchorPathContext}.leaf_count must match the certified top-up count`);
  }
  const rawSiblings = requireOwn(rawAnchorPath, "siblings", anchorPathContext);
  const expectedSiblingCount = Math.ceil(Math.log2(Number(leafCount)));
  if (!Array.isArray(rawSiblings)
      || rawSiblings.length !== expectedSiblingCount
      || rawSiblings.length > MAX_TOP_UP_FINALITY_SIBLINGS) {
    throw new TypeError(`${anchorPathContext}.siblings has a non-canonical Merkle depth`);
  }
  const siblings = rawSiblings.map((sibling, index) =>
    normalizeFixedBytes(sibling, `${anchorPathContext}.siblings[${index}]`, { nonZero: true }));

  return {
    version,
    anchor: { topup_operation_id: topupOperationId, anchor_digest: anchorDigest },
    commit_qc: { height_context: heightContext, certificate },
    anchor_path: { leaf_index: leafIndex, leaf_count: leafCount, siblings },
  };
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
