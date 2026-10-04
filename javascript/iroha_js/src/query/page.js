/**
 * The collection page envelope `{"items": [...], "next_cursor": ..., "total": N}`
 * and the JSON codecs used for collection requests and responses.
 */
import { ToriiError } from "../toriiErrors.js";
import {
  parseStrictLosslessJson,
  stringifyStrictLosslessIntegerJson,
} from "../strictLosslessJson.js";

const LONG_DIGIT_RUN = /\d{16}/u;
const U64_MAX = 18_446_744_073_709_551_615n;
const STRING_OR_LONG_INTEGER = /"(?:[^"\\]|\\.)*"|\d{16,}/gu;

function protocolError(message, cause) {
  return new ToriiError(message, { code: "invalid_response", cause });
}

function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

/** Whether `text` has an integer token of 16 or more digits outside strings. */
export function hasUnquotedLongInteger(text) {
  if (!LONG_DIGIT_RUN.test(text)) return false;
  for (const match of text.matchAll(STRING_OR_LONG_INTEGER)) {
    if (match[0].charCodeAt(0) !== 0x22) return true;
  }
  return false;
}

/** Copy a lossless-parser result into plain objects and arrays. */
export function toPlainJson(value) {
  if (Array.isArray(value)) return value.map(toPlainJson);
  if (value !== null && typeof value === "object") {
    const record = {};
    for (const key of Object.keys(value)) record[key] = toPlainJson(value[key]);
    return record;
  }
  return value;
}

/**
 * Parse a JSON response without losing integer precision: integers beyond
 * `Number.MAX_SAFE_INTEGER` outside strings become `bigint`. Responses without
 * such integers take the native `JSON.parse` fast path.
 *
 * @param {string} text
 * @param {string} context
 * @returns {unknown}
 */
export function parseJsonPreservingIntegers(text, context) {
  try {
    if (hasUnquotedLongInteger(text)) {
      return toPlainJson(
        parseStrictLosslessJson(text, context, { floatingPointPaths: [["items"]] }),
      );
    }
    return JSON.parse(text);
  } catch (error) {
    throw protocolError(`${context} is not valid JSON: ${error.message}`, error);
  }
}

/**
 * Serialize a request body; `bigint` values become exact integer tokens.
 *
 * @param {unknown} value
 * @param {string} context
 * @returns {string}
 */
export function stringifyRequestJson(value, context) {
  return stringifyStrictLosslessIntegerJson(value, context);
}

/**
 * Decode the exact page envelope, requiring an explicit `next_cursor`.
 * `total` (a `u64`) is a number,
 * or a `bigint` beyond `Number.MAX_SAFE_INTEGER`, and is present only when
 * the query asked for it.
 *
 * @template T
 * @param {unknown} value
 * @param {string} [context]
 * @returns {{items: T[], nextCursor: string | null, total: number | bigint | undefined}}
 */
export function decodePage(value, context = "collection page") {
  if (!isPlainRecord(value)) {
    throw protocolError(`${context} must be a JSON object`);
  }
  for (const field of Object.keys(value)) {
    if (!["items", "next_cursor", "total"].includes(field)) {
      throw protocolError(`${context} contains unknown field \`${field}\``);
    }
  }
  if (!Array.isArray(value.items)) {
    throw protocolError(`${context} must contain an \`items\` array`);
  }
  let nextCursor = null;
  if (!Object.hasOwn(value, "next_cursor")) {
    throw protocolError(`${context} must contain \`next_cursor\``);
  }
  if (value.next_cursor !== null) {
    if (typeof value.next_cursor !== "string" || value.next_cursor.length === 0) {
      throw protocolError(`${context} \`next_cursor\` must be a non-empty string or null`);
    }
    nextCursor = value.next_cursor;
  }
  let total;
  if (Object.hasOwn(value, "total")) {
    const valid = typeof value.total === "bigint"
      ? value.total > BigInt(Number.MAX_SAFE_INTEGER) && value.total <= U64_MAX
      : Number.isSafeInteger(value.total) && value.total >= 0;
    if (!valid) {
      throw protocolError(`${context} \`total\` must be a non-negative 64-bit integer`);
    }
    if (value.total < value.items.length) {
      throw protocolError(`${context} \`total\` cannot be smaller than the page`);
    }
    total = value.total;
  }
  return { items: value.items, nextCursor, total };
}

/** Parse and decode a page from response text. */
export function decodePageText(text, context = "collection page") {
  if (typeof text !== "string" || text.length === 0) {
    throw protocolError(`${context} is empty`);
  }
  return decodePage(parseJsonPreservingIntegers(text, context), context);
}
