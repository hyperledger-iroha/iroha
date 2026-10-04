/**
 * The Torii collection-query language: filters and sort keys.
 *
 * This module is the JavaScript counterpart of
 * `iroha_torii_shared::list_query` (`text.rs`, `filter.rs`, `sort.rs`,
 * `builder.rs`) and is checked against the shared golden vectors in
 * `fixtures/torii/list_query/vectors.json`.
 *
 * A filter is an immutable `Filter` tree. Build one fluently with `field()`,
 * parse the text form with `Filter.parse()`, or decode the JSON form with
 * `Filter.fromJSON()`. `filter.toString()` renders the canonical text that
 * Torii's `Display` produces and `filter.toJSON()` the canonical JSON form
 * (`{"op": ..., "args": [...]}`).
 *
 * Literals follow the text rules: integers that fit `u64`/`i64` are JSON
 * numbers (`bigint` when outside the safe `number` range), decimals and wider
 * integers are exact decimal strings. JavaScript floating-point numbers are
 * never accepted as literals because they are not exact.
 */
import { FilterSyntaxError, ListQueryError } from "../toriiErrors.js";

/** Maximum nesting depth of a filter expression (root is depth 0). */
export const FILTER_MAX_DEPTH = 10;
/** Maximum operator nodes in one filter expression. */
export const FILTER_MAX_NODES = 1_024;
/** Maximum literals accepted by one `in` / `not in` operator. */
export const FILTER_MAX_MEMBERSHIP_VALUES = 1_024;
/** Maximum membership literals across one filter expression. */
export const FILTER_MAX_TOTAL_MEMBERSHIP_VALUES = 4_096;
/** Maximum UTF-8 length of one field path. */
export const FIELD_PATH_MAX_BYTES = 256;
/** Maximum accepted UTF-8 length of a text filter. */
export const FILTER_TEXT_MAX_BYTES = 32 * 1024;
/** Maximum number of keys in one sort specification. */
export const SORT_MAX_KEYS = 8;

const PARSE_MAX_NESTING = 64;
const KEYWORDS = Object.freeze([
  "and",
  "or",
  "not",
  "in",
  "is",
  "null",
  "true",
  "false",
  "exists",
]);
const OPERATOR_NAMES = "and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null";
const COMPARISON_SYMBOLS = Object.freeze({
  eq: "=",
  ne: "!=",
  lt: "<",
  lte: "<=",
  gt: ">",
  gte: ">=",
});
const U64_MAX = 18_446_744_073_709_551_615n;
const I64_MIN = -9_223_372_036_854_775_808n;
const DECIMAL_TEXT = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u;
const BARE_SEGMENT = /^[A-Za-z_][A-Za-z0-9_]*$/u;
const WHITESPACE_OR_CONTROL = /[\p{White_Space}\p{Cc}]/u;
const ONLY_WHITESPACE = /^\p{White_Space}*$/u;

/** A structural problem in a filter tree (Rust `FilterError`). */
class FilterTreeError extends Error {}

function malformed(location, reason) {
  return new FilterTreeError(location ? `${reason} (at \`${location}\`)` : reason);
}

function invalidField(field, reason) {
  return new FilterTreeError(`invalid field \`${field}\`: ${reason}`);
}

function invalidOperand(field, reason) {
  return new FilterTreeError(`invalid operand for \`${field}\`: ${reason}`);
}

function limitExceeded(limit, max) {
  return new FilterTreeError(`filter exceeds the ${limit} limit of ${max}`);
}

function asFilterError(error) {
  if (error instanceof FilterTreeError) {
    return new ListQueryError("filter", error.message);
  }
  return error;
}

/** UTF-8 encoded length of a JavaScript string. */
export function utf8ByteLength(text) {
  let bytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit < 0x80) {
      bytes += 1;
    } else if (unit < 0x800) {
      bytes += 2;
    } else if (
      unit >= 0xd800 &&
      unit <= 0xdbff &&
      index + 1 < text.length &&
      (text.charCodeAt(index + 1) & 0xfc00) === 0xdc00
    ) {
      bytes += 4;
      index += 1;
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

function isWellFormedText(text) {
  if (typeof text.isWellFormed === "function") return text.isWellFormed();
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = text.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

/** Compare two strings by Unicode scalar value (the UTF-8 byte order). */
function compareCodePoints(left, right) {
  const leftIterator = left[Symbol.iterator]();
  const rightIterator = right[Symbol.iterator]();
  for (;;) {
    const a = leftIterator.next();
    const b = rightIterator.next();
    if (a.done || b.done) {
      if (a.done && b.done) return 0;
      return a.done ? -1 : 1;
    }
    const difference = a.value.codePointAt(0) - b.value.codePointAt(0);
    if (difference !== 0) return difference;
  }
}

// ---------------------------------------------------------------------------
// Field paths
// ---------------------------------------------------------------------------

/**
 * The problem with a field path's syntax, or `null` when it is valid
 * (non-empty segments, bounded length, no whitespace or control characters).
 * Whether a collection exposes the field is decided by Torii.
 */
export function fieldPathProblem(path) {
  if (typeof path !== "string") return "field paths must be strings";
  if (path.length === 0) return "field paths must not be empty";
  if (utf8ByteLength(path) > FIELD_PATH_MAX_BYTES) {
    return `field paths must not exceed ${FIELD_PATH_MAX_BYTES} bytes`;
  }
  if (WHITESPACE_OR_CONTROL.test(path)) {
    return "field paths must not contain whitespace or control characters";
  }
  if (!isWellFormedText(path)) {
    return "field paths must contain only Unicode scalar values";
  }
  if (path.split(".").some((segment) => segment.length === 0)) {
    return "field path segments must not be empty";
  }
  return null;
}

/** Validate a field path for `parameter` and return it. */
export function checkFieldPath(path, parameter) {
  if (typeof path !== "string") {
    throw new ListQueryError(parameter, "field paths must be strings");
  }
  const problem = fieldPathProblem(path);
  if (problem !== null) {
    throw new ListQueryError(parameter, `invalid field \`${path}\`: ${problem}`);
  }
  return path;
}

function isKeywordSegment(segment) {
  const lower = segment.toLowerCase();
  return KEYWORDS.includes(lower);
}

function isBareSegment(segment, first) {
  return BARE_SEGMENT.test(segment) && !(first && isKeywordSegment(segment));
}

/**
 * Render a dotted field path in the text grammar: segments that are not
 * identifiers, or a keyword in the first segment, are quoted with backticks.
 */
export function renderFieldPath(path) {
  return path
    .split(".")
    .map((segment, index) =>
      isBareSegment(segment, index === 0) ? segment : `\`${segment}\``,
    )
    .join(".");
}

// ---------------------------------------------------------------------------
// Literals
// ---------------------------------------------------------------------------

/** Whether `text` is a canonical decimal literal: `-?(0|[1-9][0-9]*)(\.[0-9]+)?`. */
export function isDecimalText(text) {
  return typeof text === "string" && DECIMAL_TEXT.test(text);
}

function integerLiteral(value) {
  if (value >= I64_MIN && value <= U64_MAX) {
    const number = Number(value);
    return Number.isSafeInteger(number) ? number : value;
  }
  return value.toString();
}

/** The literal for a number token of the text grammar (Rust `number_value`). */
function numberTokenLiteral(raw) {
  if (!raw.includes(".")) {
    return integerLiteral(BigInt(raw));
  }
  return raw;
}

function scaledDecimalText(mantissa, scale) {
  if (scale === 0) return mantissa.toString();
  const negative = mantissa < 0n;
  let digits = (negative ? -mantissa : mantissa).toString();
  if (digits.length <= scale) digits = `${"0".repeat(scale + 1 - digits.length)}${digits}`;
  const split = digits.length - scale;
  return `${negative ? "-" : ""}${digits.slice(0, split)}.${digits.slice(split)}`;
}

function isDecimalLike(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    typeof value.mantissa === "bigint" &&
    Number.isSafeInteger(value.scale) &&
    value.scale >= 0
  );
}

function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function literalProblemNumber(value) {
  if (!Number.isFinite(value)) {
    return "non-finite numbers are not literals";
  }
  if (!Number.isInteger(value)) {
    return `decimal literals must be exact decimal strings such as "${String(value)}"; JavaScript numbers are binary floating point`;
  }
  return "integers outside the safe JavaScript range must be passed as bigint";
}

function normalizeScalarLiteral(value, field) {
  if (value === null) return null;
  switch (typeof value) {
    case "boolean":
      return value;
    case "string":
      if (!isWellFormedText(value)) {
        throw invalidOperand(field, "string literals must contain only Unicode scalar values");
      }
      return value;
    case "number":
      if (Number.isSafeInteger(value)) return value === 0 ? 0 : value;
      throw invalidOperand(field, literalProblemNumber(value));
    case "bigint":
      return integerLiteral(value);
    default:
      break;
  }
  if (isDecimalLike(value)) {
    return numberTokenLiteral(scaledDecimalText(value.mantissa, value.scale));
  }
  return undefined;
}

function normalizeStructuredValue(value, field, depth) {
  if (depth > 64) {
    throw invalidOperand(field, "structured literals nest too deeply");
  }
  if (Array.isArray(value)) {
    return Object.freeze(
      Array.from(value, (entry) => normalizeStructuredValue(entry, field, depth + 1)),
    );
  }
  if (isPlainRecord(value)) {
    const keys = Object.keys(value).sort(compareCodePoints);
    const record = {};
    for (const key of keys) {
      if (!isWellFormedText(key)) {
        throw invalidOperand(field, "object keys must contain only Unicode scalar values");
      }
      record[key] = normalizeStructuredValue(value[key], field, depth + 1);
    }
    return Object.freeze(record);
  }
  const scalar = normalizeScalarLiteral(value, field);
  if (scalar === undefined) {
    throw invalidOperand(
      field,
      "structured literals may only contain strings, integers, exact decimals, booleans, null, arrays and objects",
    );
  }
  return scalar;
}

/** Normalize a JavaScript value into a filter literal for `field`. */
function normalizeLiteral(value, field) {
  const scalar = normalizeScalarLiteral(value, field);
  if (scalar !== undefined) return scalar;
  if (Array.isArray(value) || isPlainRecord(value)) {
    return normalizeStructuredValue(value, field, 0);
  }
  throw invalidOperand(
    field,
    "literals must be strings, integers (number or bigint), exact decimals, booleans or null",
  );
}

function isStructuredLiteral(value) {
  return value !== null && typeof value === "object";
}

function isNumericLiteral(value) {
  return (
    (typeof value === "number" && Number.isFinite(value)) ||
    typeof value === "bigint" ||
    isDecimalText(value)
  );
}

/** Canonical JSON text of a literal (compact, object keys in scalar order). */
function renderLiteral(value) {
  if (value === null) return "null";
  switch (typeof value) {
    case "string":
      return JSON.stringify(value);
    case "number":
    case "bigint":
      return value.toString();
    case "boolean":
      return value ? "true" : "false";
    default:
      break;
  }
  if (Array.isArray(value)) {
    return `[${value.map(renderLiteral).join(",")}]`;
  }
  const keys = Object.keys(value).sort(compareCodePoints);
  return `{${keys.map((key) => `${JSON.stringify(key)}:${renderLiteral(value[key])}`).join(",")}}`;
}

function literalToJson(value) {
  if (Array.isArray(value)) return value.map(literalToJson);
  if (isStructuredLiteral(value)) {
    const record = {};
    for (const key of Object.keys(value)) record[key] = literalToJson(value[key]);
    return record;
  }
  return value;
}

// ---------------------------------------------------------------------------
// Filter tree
// ---------------------------------------------------------------------------

const FILTER_TOKEN = Symbol("iroha.query.filter");

function makeFilter(op, args) {
  return new Filter(FILTER_TOKEN, op, Object.freeze(args));
}

function requireFilter(value, context) {
  if (value instanceof Filter) return value;
  throw new TypeError(`${context} must be a Filter built with field(), Filter.parse() or Filter.fromJSON()`);
}

function joinFilters(op, left, right) {
  if (left.op === op && right.op === op) return makeFilter(op, [...left.args, ...right.args]);
  if (left.op === op) return makeFilter(op, [...left.args, right]);
  if (right.op === op) return makeFilter(op, [left, ...right.args]);
  return makeFilter(op, [left, right]);
}

/**
 * An immutable filter expression. `op` is the JSON-form operator and `args`
 * its operands: nested filters for `and`/`or`/`not`, `[field, literal]` for
 * comparisons, `[field, [literals...]]` for `in`/`nin` and `[field]` for
 * `exists`/`is_null`.
 */
export class Filter {
  constructor(token, op, args) {
    if (token !== FILTER_TOKEN) {
      throw new TypeError(
        "Filter instances are created with field(), Filter.parse() or Filter.fromJSON()",
      );
    }
    this.op = op;
    this.args = args;
    Object.freeze(this);
  }

  /** `this and other and ...`, flattening chains of `and`. */
  and(...others) {
    return others.reduce(
      (left, right, index) => joinFilters("and", left, requireFilter(right, `and() operand ${index}`)),
      this,
    );
  }

  /** `this or other or ...`, flattening chains of `or`. */
  or(...others) {
    return others.reduce(
      (left, right, index) => joinFilters("or", left, requireFilter(right, `or() operand ${index}`)),
      this,
    );
  }

  /** `not this` */
  not() {
    return makeFilter("not", [this]);
  }

  /** Canonical JSON form: `{"op": ..., "args": [...]}`. */
  toJSON() {
    return filterToJson(this);
  }

  /** Canonical text form; `Filter.parse` reads it back to the same tree. */
  toString() {
    const out = [];
    writeExpression(this, ROOT, out);
    return out.join("");
  }

  /**
   * Check the structural limits (depth, node count, membership totals) and
   * return this filter.
   *
   * @throws {ListQueryError} with code `invalid_filter`
   */
  validate() {
    try {
      validateTree(this);
    } catch (error) {
      throw asFilterError(error);
    }
    return this;
  }

  /** Conjunction of one or more filters. */
  static and(first, ...rest) {
    return requireFilter(first, "Filter.and() operand 0").and(...rest);
  }

  /** Disjunction of one or more filters. */
  static or(first, ...rest) {
    return requireFilter(first, "Filter.or() operand 0").or(...rest);
  }

  /** Negation of a filter. */
  static not(filter) {
    return requireFilter(filter, "Filter.not() operand").not();
  }

  /**
   * Parse the text form, e.g. `owned_by = "alice" and quantity >= 10`.
   *
   * @throws {FilterSyntaxError} with code `invalid_filter`, `line` and `column`
   */
  static parse(text) {
    return parseFilterText(text);
  }

  /**
   * Decode the JSON form (`{"op": ..., "args": [...]}`).
   *
   * @throws {ListQueryError} with code `invalid_filter`
   */
  static fromJSON(value) {
    try {
      const budget = { nodes: 0, membership: 0 };
      return filterFromJson(value, 0, budget, []);
    } catch (error) {
      throw asFilterError(error);
    }
  }
}

function filterToJson(filter) {
  switch (filter.op) {
    case "and":
    case "or":
    case "not":
      return { op: filter.op, args: filter.args.map(filterToJson) };
    case "in":
    case "nin":
      return { op: filter.op, args: [filter.args[0], filter.args[1].map(literalToJson)] };
    case "exists":
    case "is_null":
      return { op: filter.op, args: [filter.args[0]] };
    default:
      return { op: filter.op, args: [filter.args[0], literalToJson(filter.args[1])] };
  }
}

// --- validation (Rust `FilterExpr::validate`) ---------------------------------

function enterBudget(budget, depth) {
  if (depth > FILTER_MAX_DEPTH) {
    throw limitExceeded("nesting depth", FILTER_MAX_DEPTH);
  }
  budget.nodes += 1;
  if (budget.nodes > FILTER_MAX_NODES) {
    throw limitExceeded("node count", FILTER_MAX_NODES);
  }
}

function checkMembership(budget, field, values) {
  if (values.length === 0) {
    throw invalidOperand(field, "membership lists must not be empty");
  }
  if (values.length > FILTER_MAX_MEMBERSHIP_VALUES) {
    throw limitExceeded("membership list size", FILTER_MAX_MEMBERSHIP_VALUES);
  }
  budget.membership += values.length;
  if (budget.membership > FILTER_MAX_TOTAL_MEMBERSHIP_VALUES) {
    throw limitExceeded("total membership values", FILTER_MAX_TOTAL_MEMBERSHIP_VALUES);
  }
  const seen = new Set();
  for (const value of values) {
    const key = renderLiteral(value);
    if (seen.has(key)) {
      throw invalidOperand(field, "membership list values must be unique");
    }
    seen.add(key);
  }
  const homogeneous =
    values.every((value) => typeof value === "string") ||
    values.every(isNumericLiteral) ||
    values.every((value) => typeof value === "boolean");
  if (!homogeneous && !field.startsWith("metadata.")) {
    throw invalidOperand(field, "membership list values must all be strings, numbers or booleans");
  }
}

function checkFieldInTree(field) {
  const problem = fieldPathProblem(field);
  if (problem !== null) throw invalidField(field, problem);
}

function checkScalarOperand(field, value) {
  if (isStructuredLiteral(value) && !field.startsWith("metadata.")) {
    throw invalidOperand(field, "comparison literals must be strings, numbers, booleans or null");
  }
}

function checkRangeOperand(field, value) {
  if (!(isNumericLiteral(value) || typeof value === "string")) {
    throw invalidOperand(field, "range comparisons need a number, decimal or string literal");
  }
}

function validateNode(filter, depth, budget) {
  enterBudget(budget, depth);
  switch (filter.op) {
    case "and":
    case "or":
      if (filter.args.length === 0) {
        throw malformed("", `\`${filter.op}\` needs at least one operand`);
      }
      for (const nested of filter.args) validateNode(nested, depth + 1, budget);
      return;
    case "not":
      validateNode(filter.args[0], depth + 1, budget);
      return;
    case "eq":
    case "ne":
      checkFieldInTree(filter.args[0]);
      checkScalarOperand(filter.args[0], filter.args[1]);
      return;
    case "lt":
    case "lte":
    case "gt":
    case "gte":
      checkFieldInTree(filter.args[0]);
      checkRangeOperand(filter.args[0], filter.args[1]);
      return;
    case "in":
    case "nin":
      checkFieldInTree(filter.args[0]);
      checkMembership(budget, filter.args[0], filter.args[1]);
      return;
    default:
      checkFieldInTree(filter.args[0]);
  }
}

function validateTree(filter) {
  validateNode(filter, 0, { nodes: 0, membership: 0 });
}

function validateLeaf(filter) {
  try {
    validateNode(filter, 0, { nodes: 0, membership: 0 });
  } catch (error) {
    throw asFilterError(error);
  }
  return filter;
}

// --- JSON form decoding (Rust `FilterExpr::from_json_value`) -----------------

function renderLocation(location) {
  let out = "";
  for (const segment of location) {
    if (typeof segment === "number") {
      out += `[${segment}]`;
    } else {
      if (out) out += ".";
      out += segment;
    }
  }
  return out;
}

function jsonLiteral(value, field) {
  if (value === undefined) {
    throw invalidOperand(field, "literals must be JSON values");
  }
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw invalidOperand(field, literalProblemNumber(value));
  }
  return normalizeLiteral(value, field);
}

function filterFromJson(value, depth, budget, location) {
  enterBudget(budget, depth);
  if (!isPlainRecord(value)) {
    throw malformed(
      renderLocation(location),
      "a filter node must be an object such as {\"op\": \"eq\", \"args\": [\"field\", value]}",
    );
  }
  if (!Object.prototype.hasOwnProperty.call(value, "op")) {
    throw malformed(renderLocation(location), "a filter node needs an `op` member");
  }
  const op = value.op;
  if (typeof op !== "string") {
    throw malformed(renderLocation(location), "`op` must be a string");
  }
  const unknown = Object.keys(value)
    .filter((key) => key !== "op" && key !== "args")
    .sort(compareCodePoints);
  if (unknown.length > 0) {
    throw malformed(
      renderLocation(location),
      `unknown member \`${unknown[0]}\`; a filter node has only \`op\` and \`args\``,
    );
  }
  const args = Object.prototype.hasOwnProperty.call(value, "args") ? value.args : null;
  const here = [...location, "args"];
  switch (op) {
    case "and":
    case "or": {
      if (!Array.isArray(args)) {
        throw malformed(renderLocation(here), `\`${op}\` takes an array of filter nodes`);
      }
      if (args.length === 0) {
        throw malformed(renderLocation(here), `\`${op}\` needs at least one operand`);
      }
      if (args.length > FILTER_MAX_NODES - budget.nodes) {
        throw limitExceeded("node count", FILTER_MAX_NODES);
      }
      const operands = args.map((nested, index) =>
        filterFromJson(nested, depth + 1, budget, [...here, index]),
      );
      return makeFilter(op, operands);
    }
    case "not": {
      if (!Array.isArray(args) || args.length !== 1) {
        throw malformed(renderLocation(here), "`not` takes an array with exactly one filter node");
      }
      return makeFilter("not", [filterFromJson(args[0], depth + 1, budget, [...here, 0])]);
    }
    case "eq":
    case "ne":
    case "lt":
    case "lte":
    case "gt":
    case "gte": {
      const [field, operand] = binaryArgs(args, op, here);
      const leaf = makeFilter(op, [field, jsonLiteral(operand, field)]);
      validateNode(leaf, 0, { nodes: 0, membership: 0 });
      return leaf;
    }
    case "in":
    case "nin": {
      const [field, operand] = binaryArgs(args, op, here);
      if (!Array.isArray(operand)) {
        throw malformed(renderLocation(here), `\`${op}\` takes ["field", [value, ...]]`);
      }
      checkFieldInTree(field);
      const values = Object.freeze(operand.map((entry) => jsonLiteral(entry, field)));
      checkMembership(budget, field, values);
      return makeFilter(op, [field, values]);
    }
    case "exists":
    case "is_null": {
      if (!Array.isArray(args) || args.length !== 1) {
        throw malformed(renderLocation(here), `\`${op}\` takes ["field"]`);
      }
      if (typeof args[0] !== "string") {
        throw malformed(renderLocation(here), "the field must be a string");
      }
      checkFieldInTree(args[0]);
      return makeFilter(op, [args[0]]);
    }
    default:
      throw malformed(
        renderLocation(location),
        `unknown operator \`${op}\`; expected one of: ${OPERATOR_NAMES}`,
      );
  }
}

function binaryArgs(args, op, location) {
  if (Array.isArray(args) && args.length === 2) {
    if (typeof args[0] !== "string") {
      throw malformed(renderLocation(location), "the first argument must be the field name");
    }
    return args;
  }
  throw malformed(renderLocation(location), `\`${op}\` takes ["field", value]`);
}

// --- canonical text rendering (Rust `Display for FilterExpr`) -----------------

const ROOT = 0;
const OR = 1;
const AND = 2;
const NOT = 3;

function writeExpression(filter, parent, out) {
  switch (filter.op) {
    case "and":
    case "or": {
      const keyword = filter.op;
      const own = keyword === "or" ? OR : AND;
      const parenthesize = keyword === "or" ? parent !== ROOT : parent === AND || parent === NOT;
      if (parenthesize) out.push("(");
      filter.args.forEach((operand, index) => {
        if (index > 0) out.push(` ${keyword} `);
        writeExpression(operand, own, out);
      });
      if (parenthesize) out.push(")");
      return;
    }
    case "not": {
      const inner = filter.args[0];
      if (inner.op === "is_null") {
        out.push(`${renderFieldPath(inner.args[0])} is not null`);
        return;
      }
      out.push("not ");
      writeExpression(inner, NOT, out);
      return;
    }
    case "in":
    case "nin":
      out.push(
        `${renderFieldPath(filter.args[0])} ${filter.op === "in" ? "in" : "not in"} [${filter.args[1]
          .map(renderLiteral)
          .join(", ")}]`,
      );
      return;
    case "exists":
      out.push(`exists(${renderFieldPath(filter.args[0])})`);
      return;
    case "is_null":
      out.push(`${renderFieldPath(filter.args[0])} is null`);
      return;
    default:
      out.push(
        `${renderFieldPath(filter.args[0])} ${COMPARISON_SYMBOLS[filter.op]} ${renderLiteral(filter.args[1])}`,
      );
  }
}

// ---------------------------------------------------------------------------
// Fluent builder
// ---------------------------------------------------------------------------

function membershipLeaf(op, path, values) {
  if (values === null || typeof values !== "object" || typeof values[Symbol.iterator] !== "function") {
    throw new ListQueryError("filter", `\`${op}\` takes an iterable of literals`);
  }
  let literals;
  try {
    literals = Object.freeze(Array.from(values, (value) => normalizeLiteral(value, path)));
  } catch (error) {
    throw asFilterError(error);
  }
  return validateLeaf(makeFilter(op, [path, literals]));
}

function comparisonLeaf(op, path, value) {
  let literal;
  try {
    literal = normalizeLiteral(value, path);
  } catch (error) {
    throw asFilterError(error);
  }
  return validateLeaf(makeFilter(op, [path, literal]));
}

/** A field awaiting an operator; see `field()`. Reusable and immutable. */
export class FieldRef {
  constructor(path) {
    this.path = checkFieldPath(path, "filter");
    Object.freeze(this);
  }

  /** `field = value` */
  eq(value) {
    return comparisonLeaf("eq", this.path, value);
  }

  /** `field != value` (also matches rows where the field is absent). */
  ne(value) {
    return comparisonLeaf("ne", this.path, value);
  }

  /** `field < value` */
  lt(value) {
    return comparisonLeaf("lt", this.path, value);
  }

  /** `field <= value` */
  lte(value) {
    return comparisonLeaf("lte", this.path, value);
  }

  /** `field > value` */
  gt(value) {
    return comparisonLeaf("gt", this.path, value);
  }

  /** `field >= value` */
  gte(value) {
    return comparisonLeaf("gte", this.path, value);
  }

  /** `field in [values...]` */
  in(values) {
    return membershipLeaf("in", this.path, values);
  }

  /** `field not in [values...]` (also matches rows where the field is absent). */
  notIn(values) {
    return membershipLeaf("nin", this.path, values);
  }

  /** `exists(field)` */
  exists() {
    return makeFilter("exists", [this.path]);
  }

  /** `field is null` (absent or null). */
  isNull() {
    return makeFilter("is_null", [this.path]);
  }

  /** `field is not null` */
  isNotNull() {
    return makeFilter("not", [makeFilter("is_null", [this.path])]);
  }

  /** Ascending sort key on this field. */
  asc() {
    return new SortKey(this.path, false);
  }

  /** Descending sort key on this field. */
  desc() {
    return new SortKey(this.path, true);
  }
}

/** Start a predicate or sort key on a field path such as `metadata.tier`. */
export function field(path) {
  return new FieldRef(path);
}

// ---------------------------------------------------------------------------
// Sort keys
// ---------------------------------------------------------------------------

/** One sort key: `field` sorts ascending and `-field` descending. */
export class SortKey {
  constructor(path, descending = false) {
    this.field = checkFieldPath(path, "sort");
    if (typeof descending !== "boolean") {
      throw new TypeError("SortKey descending flag must be a boolean");
    }
    this.descending = descending;
    Object.freeze(this);
  }

  /** Ascending key. */
  static asc(path) {
    return new SortKey(path, false);
  }

  /** Descending key. */
  static desc(path) {
    return new SortKey(path, true);
  }

  /**
   * Parse exactly one key such as `-quantity`.
   *
   * @throws {FilterSyntaxError} with code `invalid_sort`
   */
  static parse(text) {
    const keys = parseSortText(text);
    if (keys.length !== 1) {
      throw syntaxError(
        "sort",
        text,
        0,
        "expected exactly one sort key; pass each key as its own array element",
      );
    }
    return keys[0];
  }

  /** `"asc"` or `"desc"`. */
  get order() {
    return this.descending ? "desc" : "asc";
  }

  /** Canonical spelling such as `-quantity`. */
  toString() {
    return `${this.descending ? "-" : ""}${renderFieldPath(this.field)}`;
  }

  toJSON() {
    return this.toString();
  }
}

/** Render keys as a comma-separated specification such as `-quantity,id`. */
export function sortToString(keys) {
  return keys.map((key) => key.toString()).join(",");
}

/**
 * Parse a sort specification such as `-quantity,id`.
 *
 * @throws {FilterSyntaxError} with code `invalid_sort`
 */
export function parseSort(text) {
  return parseSortText(text);
}

// ---------------------------------------------------------------------------
// Text grammar (Rust `text.rs`)
// ---------------------------------------------------------------------------

function positionOf(input, index) {
  const before = input.slice(0, index);
  let line = 1;
  let lineStart = 0;
  for (let cursor = 0; cursor < before.length; cursor += 1) {
    if (before.charCodeAt(cursor) === 0x0a) {
      line += 1;
      lineStart = cursor + 1;
    }
  }
  let column = 1;
  for (const _ of input.slice(lineStart, index)) column += 1;
  return { offset: utf8ByteLength(before), line, column };
}

function syntaxError(parameter, input, index, reason, multiline = input.includes("\n")) {
  const { offset, line, column } = positionOf(input, Math.min(index, input.length));
  return new FilterSyntaxError(parameter, reason, { offset, line, column, multiline });
}

function isDigit(code) {
  return code >= 0x30 && code <= 0x39;
}

function isAlpha(code) {
  return (code >= 0x41 && code <= 0x5a) || (code >= 0x61 && code <= 0x7a);
}

function isWordChar(code) {
  return isAlpha(code) || isDigit(code) || code === 0x5f;
}

function isControlCodePoint(codePoint) {
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}

function hexValue(code) {
  if (code >= 0x30 && code <= 0x39) return code - 0x30;
  if (code >= 0x41 && code <= 0x46) return code - 0x41 + 10;
  if (code >= 0x61 && code <= 0x66) return code - 0x61 + 10;
  return -1;
}

class Lexer {
  constructor(input, allowMinus) {
    this.input = input;
    this.position = 0;
    this.allowMinus = allowMinus;
    this.parameter = allowMinus ? "sort" : "filter";
  }

  error(index, reason) {
    return syntaxError(this.parameter, this.input, index, reason);
  }

  code(index) {
    return index < this.input.length ? this.input.charCodeAt(index) : -1;
  }

  tokens() {
    const out = [];
    for (;;) {
      const token = this.nextToken();
      out.push(token);
      if (token.kind === "end") return out;
    }
  }

  nextToken() {
    const { input } = this;
    while (this.position < input.length) {
      const code = input.charCodeAt(this.position);
      if (code === 0x20 || code === 0x09 || code === 0x0d || code === 0x0a) {
        this.position += 1;
      } else {
        break;
      }
    }
    const start = this.position;
    if (start >= input.length) return { kind: "end", start };
    const code = input.charCodeAt(start);
    const single = (kind) => {
      this.position += 1;
      return { kind, start };
    };
    const compare = (value, width) => {
      this.position += width;
      return { kind: "compare", value, start };
    };
    switch (code) {
      case 0x28: return single("lparen");
      case 0x29: return single("rparen");
      case 0x5b: return single("lbracket");
      case 0x5d: return single("rbracket");
      case 0x2c: return single("comma");
      case 0x2e:
        if (isDigit(this.code(start + 1))) {
          throw this.error(start, "decimal literals need a leading digit, e.g. `0.5`");
        }
        return single("dot");
      case 0x3d:
        return compare("eq", this.code(start + 1) === 0x3d ? 2 : 1);
      case 0x21:
        if (this.code(start + 1) === 0x3d) return compare("ne", 2);
        throw this.error(start, "use the keyword `not` instead of `!`");
      case 0x3c: {
        const next = this.code(start + 1);
        if (next === 0x3d) return compare("lte", 2);
        if (next === 0x3e) return compare("ne", 2);
        return compare("lt", 1);
      }
      case 0x3e:
        return this.code(start + 1) === 0x3d ? compare("gte", 2) : compare("gt", 1);
      case 0x26:
        throw this.error(start, "use the keyword `and` instead of `&` or `&&`");
      case 0x7c:
        throw this.error(start, "use the keyword `or` instead of `|` or `||`");
      case 0x22:
      case 0x27:
        return this.string(code);
      case 0x60:
        return this.quotedSegment();
      case 0x2d:
        if (isDigit(this.code(start + 1))) return this.number();
        if (this.allowMinus) return single("minus");
        throw this.error(
          start,
          "unexpected `-`; quote field names that contain `-` with backticks, e.g. `display-name`",
        );
      case 0x3a:
        throw this.error(
          start,
          this.allowMinus
            ? "unexpected `:`; write `field` for ascending and `-field` for descending order"
            : "unexpected `:`; compare values with `=`, e.g. `status = \"active\"`",
        );
      default:
        break;
    }
    if (isDigit(code)) return this.number();
    if (isAlpha(code) || code === 0x5f) {
      let end = start + 1;
      while (isWordChar(this.code(end))) end += 1;
      this.position = end;
      if (this.code(end) === 0x2d && isAlpha(this.code(end + 1))) {
        let wordEnd = end;
        while (isWordChar(this.code(wordEnd)) || this.code(wordEnd) === 0x2d) wordEnd += 1;
        throw this.error(
          start,
          `wrap field names containing \`-\` in backticks, e.g. \`${input.slice(start, wordEnd)}\``,
        );
      }
      return { kind: "word", value: input.slice(start, end), start };
    }
    const character = String.fromCodePoint(input.codePointAt(start));
    throw this.error(start, `unexpected character \`${character}\``);
  }

  number() {
    const start = this.position;
    let end = start;
    if (this.code(end) === 0x2d) end += 1;
    const integerStart = end;
    while (isDigit(this.code(end))) end += 1;
    if (end - integerStart > 1 && this.code(integerStart) === 0x30) {
      throw this.error(start, "numbers must not have leading zeros");
    }
    if (this.code(end) === 0x2e) {
      end += 1;
      const fractionStart = end;
      while (isDigit(this.code(end))) end += 1;
      if (end === fractionStart) {
        throw this.error(start, "decimal literals need digits after `.`");
      }
    }
    const next = this.code(end);
    if (next === 0x65 || next === 0x45) {
      throw this.error(start, "exponent notation is not supported; write the full decimal value");
    }
    if (isAlpha(next) || next === 0x5f) {
      throw this.error(start, "a number cannot be followed directly by letters; quote text values");
    }
    this.position = end;
    return { kind: "number", value: this.input.slice(start, end), start };
  }

  string(quote) {
    const { input } = this;
    const start = this.position;
    let out = "";
    let index = start + 1;
    for (;;) {
      if (index >= input.length) {
        throw this.error(start, "unterminated string literal");
      }
      const codePoint = input.codePointAt(index);
      const width = codePoint > 0xffff ? 2 : 1;
      if (codePoint === quote) {
        this.position = index + 1;
        return { kind: "str", value: out, start };
      }
      if (codePoint === 0x5c) {
        if (index + 1 >= input.length) {
          throw this.error(index, "unterminated escape sequence");
        }
        const escaped = input.codePointAt(index + 1);
        let consumed = 2;
        switch (escaped) {
          case 0x22: out += "\""; break;
          case 0x27: out += "'"; break;
          case 0x5c: out += "\\"; break;
          case 0x2f: out += "/"; break;
          case 0x62: out += "\b"; break;
          case 0x66: out += "\f"; break;
          case 0x6e: out += "\n"; break;
          case 0x72: out += "\r"; break;
          case 0x74: out += "\t"; break;
          case 0x75: {
            const decoded = this.unicodeEscape(index);
            out += decoded.text;
            consumed = decoded.consumed;
            break;
          }
          default:
            throw this.error(index, `unknown escape sequence \`\\${String.fromCodePoint(escaped)}\``);
        }
        index += consumed;
        continue;
      }
      if (isControlCodePoint(codePoint)) {
        throw this.error(index, "control characters must be escaped inside string literals");
      }
      out += String.fromCodePoint(codePoint);
      index += width;
    }
  }

  unicodeEscape(at) {
    const { input } = this;
    const invalid = () =>
      this.error(at, "invalid `\\u` escape; expected four hexadecimal digits");
    const unpaired = () => this.error(at, "unpaired UTF-16 surrogate in `\\u` escape");
    const readUnit = (from) => {
      let value = 0;
      for (let offset = 0; offset < 4; offset += 1) {
        const digit = hexValue(this.code(from + offset));
        if (digit < 0) return -1;
        value = value * 16 + digit;
      }
      return value;
    };
    const first = readUnit(at + 2);
    if (first < 0) throw invalid();
    if (first >= 0xd800 && first < 0xdc00) {
      if (input.charCodeAt(at + 6) !== 0x5c || input.charCodeAt(at + 7) !== 0x75) {
        throw unpaired();
      }
      const second = readUnit(at + 8);
      if (second < 0) throw invalid();
      if (!(second >= 0xdc00 && second < 0xe000)) throw unpaired();
      const combined = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
      return { text: String.fromCodePoint(combined), consumed: 12 };
    }
    if (first >= 0xdc00 && first < 0xe000) throw unpaired();
    return { text: String.fromCodePoint(first), consumed: 6 };
  }

  quotedSegment() {
    const { input } = this;
    const start = this.position;
    const close = input.indexOf("`", start + 1);
    if (close === -1) {
      throw this.error(start, "unterminated backtick-quoted field name");
    }
    const segment = input.slice(start + 1, close);
    if (segment.length === 0) {
      throw this.error(start, "backtick-quoted field names must not be empty");
    }
    if (segment.includes(".")) {
      throw this.error(
        start,
        "a backtick-quoted segment must not contain `.`; quote each segment separately",
      );
    }
    this.position = close + 1;
    return { kind: "quoted", value: segment, start };
  }
}

function describe(token) {
  switch (token.kind) {
    case "word":
    case "quoted":
      return `\`${token.value}\``;
    case "str":
      return "a string literal";
    case "number":
      return `the number \`${token.value}\``;
    case "compare":
      return "a comparison operator";
    case "minus":
      return "`-`";
    case "lparen":
      return "`(`";
    case "rparen":
      return "`)`";
    case "lbracket":
      return "`[`";
    case "rbracket":
      return "`]`";
    case "comma":
      return "`,`";
    case "dot":
      return "`.`";
    default:
      return "the end of the input";
  }
}

function isKeyword(token, keyword) {
  return token.kind === "word" && token.value.toLowerCase() === keyword;
}

function keywordOf(token) {
  if (token.kind !== "word") return null;
  const lower = token.value.toLowerCase();
  return KEYWORDS.includes(lower) ? lower : null;
}

class Parser {
  constructor(input, allowMinus) {
    this.input = input;
    this.parameter = allowMinus ? "sort" : "filter";
    this.tokens = new Lexer(input, allowMinus).tokens();
    this.position = 0;
    this.nesting = 0;
  }

  peek() {
    return this.tokens[Math.min(this.position, this.tokens.length - 1)];
  }

  peekKindAt(ahead) {
    return this.tokens[Math.min(this.position + ahead, this.tokens.length - 1)].kind;
  }

  advance() {
    const token = this.peek();
    if (token.kind !== "end") this.position += 1;
    return token;
  }

  errorAt(token, reason) {
    return syntaxError(this.parameter, this.input, token.start, reason);
  }

  enter(token) {
    this.nesting += 1;
    if (this.nesting > PARSE_MAX_NESTING) {
      throw this.errorAt(token, "filter nests too deeply");
    }
  }

  filter() {
    const first = this.and();
    if (!isKeyword(this.peek(), "or")) return first;
    const operands = [first];
    while (isKeyword(this.peek(), "or")) {
      this.advance();
      operands.push(this.and());
    }
    return makeFilter("or", operands);
  }

  and() {
    const first = this.unary();
    if (!isKeyword(this.peek(), "and")) return first;
    const operands = [first];
    while (isKeyword(this.peek(), "and")) {
      this.advance();
      operands.push(this.unary());
    }
    return makeFilter("and", operands);
  }

  unary() {
    if (isKeyword(this.peek(), "not")) {
      const token = this.advance();
      this.enter(token);
      const inner = this.unary();
      this.nesting -= 1;
      return makeFilter("not", [inner]);
    }
    return this.primary();
  }

  primary() {
    const token = this.peek();
    switch (token.kind) {
      case "lparen": {
        this.advance();
        this.enter(token);
        const inner = this.filter();
        this.nesting -= 1;
        this.expectClose("rparen", token);
        return inner;
      }
      case "word":
        if (isKeyword(token, "exists") && this.peekKindAt(1) === "lparen") {
          this.advance();
          const open = this.advance();
          const path = this.path();
          this.expectClose("rparen", open);
          return makeFilter("exists", [path]);
        }
        return this.predicate(this.path());
      case "quoted":
        return this.predicate(this.path());
      case "str":
      case "number":
        throw this.errorAt(
          token,
          "expected a field name on the left-hand side, e.g. `quantity > 5`",
        );
      case "end":
        throw this.errorAt(token, "expected a filter expression");
      default:
        throw this.errorAt(token, `expected a field name, found ${describe(token)}`);
    }
  }

  expectClose(close, open) {
    const token = this.advance();
    if (token.kind === close) return;
    const [symbol, opened] = close === "rparen" ? ["`)`", "`(`"] : ["`]`", "`[`"];
    const { column } = positionOf(this.input, open.start);
    throw this.errorAt(
      token,
      `expected ${symbol} to close the ${opened} at column ${column}, found ${describe(token)}`,
    );
  }

  path() {
    const first = this.advance();
    let path;
    if (first.kind === "word") {
      const keyword = keywordOf(first);
      if (keyword !== null) {
        throw this.errorAt(
          first,
          `expected a field name, found the keyword \`${keyword}\`; quote a field with this name as \`${first.value}\` in backticks`,
        );
      }
      path = first.value;
    } else if (first.kind === "quoted") {
      path = first.value;
    } else {
      throw this.errorAt(first, `expected a field name, found ${describe(first)}`);
    }
    while (this.peek().kind === "dot") {
      this.advance();
      const segment = this.advance();
      if (segment.kind === "word" || segment.kind === "quoted") {
        path += `.${segment.value}`;
      } else {
        throw this.errorAt(segment, `expected a field name after \`.\`, found ${describe(segment)}`);
      }
    }
    const problem = fieldPathProblem(path);
    if (problem !== null) {
      throw this.errorAt(first, `invalid field \`${path}\`: ${problem}`);
    }
    return path;
  }

  predicate(path) {
    const token = this.advance();
    if (token.kind === "compare") {
      return makeFilter(token.value, [path, this.literal()]);
    }
    if (isKeyword(token, "in")) {
      return makeFilter("in", [path, this.list()]);
    }
    if (isKeyword(token, "not")) {
      const next = this.advance();
      if (isKeyword(next, "in")) return makeFilter("nin", [path, this.list()]);
      throw this.errorAt(next, "expected `in` after `not` (as in `field not in [...]`)");
    }
    if (isKeyword(token, "is")) {
      const next = this.advance();
      if (isKeyword(next, "null")) return makeFilter("is_null", [path]);
      if (isKeyword(next, "not")) {
        const nullToken = this.advance();
        if (isKeyword(nullToken, "null")) {
          return makeFilter("not", [makeFilter("is_null", [path])]);
        }
        throw this.errorAt(nullToken, "expected `null` after `is not`");
      }
      throw this.errorAt(
        next,
        "expected `null` or `not null` after `is`; compare values with `=`",
      );
    }
    const operators = "(=, !=, <, <=, >, >=, in, not in, is null)";
    if (token.kind === "end") {
      throw this.errorAt(
        token,
        `expected an operator after \`${renderFieldPath(path)}\` ${operators}`,
      );
    }
    throw this.errorAt(
      token,
      `expected an operator after \`${renderFieldPath(path)}\` ${operators}, found ${describe(token)}`,
    );
  }

  list() {
    const open = this.advance();
    let close;
    if (open.kind === "lbracket") {
      close = "rbracket";
    } else if (open.kind === "lparen") {
      close = "rparen";
    } else {
      throw this.errorAt(open, `expected \`[\` to start a value list, found ${describe(open)}`);
    }
    const values = [];
    for (;;) {
      if (this.peek().kind === close) {
        if (values.length === 0) {
          throw this.errorAt(this.peek(), "value lists must not be empty");
        }
        this.advance();
        return Object.freeze(values);
      }
      values.push(this.literal());
      const separator = this.peek();
      if (separator.kind === "comma") {
        this.advance();
      } else if (separator.kind !== close) {
        this.expectClose(close, open);
      }
    }
  }

  literal() {
    const token = this.advance();
    switch (token.kind) {
      case "str":
        return token.value;
      case "number":
        return numberTokenLiteral(token.value);
      case "word": {
        const lower = token.value.toLowerCase();
        if (lower === "true") return true;
        if (lower === "false") return false;
        if (lower === "null") return null;
        throw this.errorAt(
          token,
          `expected a literal value, found \`${token.value}\`; quote text values, e.g. "${token.value}"`,
        );
      }
      default:
        throw this.errorAt(token, `expected a literal value, found ${describe(token)}`);
    }
  }

  finish() {
    const token = this.peek();
    if (token.kind === "end") return;
    const hint =
      token.kind === "word" || token.kind === "quoted"
        ? "; combine conditions with `and` or `or`"
        : "";
    throw this.errorAt(token, `unexpected ${describe(token)} after a complete filter${hint}`);
  }
}

/**
 * UTF-16 index of the character containing UTF-8 byte `byteOffset`: an offset
 * inside a multi-byte character moves back to that character's first byte, as
 * Rust's `FilterSyntaxError::at` does.
 */
function indexAtUtf8Offset(text, byteOffset) {
  let bytes = 0;
  for (let index = 0; index < text.length;) {
    const unit = text.charCodeAt(index);
    const pair = unit >= 0xd800 && unit <= 0xdbff && index + 1 < text.length;
    const width = unit < 0x80 ? 1 : unit < 0x800 ? 2 : pair ? 4 : 3;
    if (bytes + width > byteOffset) return index;
    bytes += width;
    index += pair ? 2 : 1;
  }
  return text.length;
}

function firstLoneSurrogate(text) {
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = text.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        index += 1;
        continue;
      }
      return index;
    }
    if (unit >= 0xdc00 && unit <= 0xdfff) return index;
  }
  return -1;
}

function requireScalarText(parameter, text) {
  if (isWellFormedText(text)) return;
  throw syntaxError(
    parameter,
    text,
    firstLoneSurrogate(text),
    "text must contain only Unicode scalar values",
  );
}

function parseFilterText(text) {
  if (typeof text !== "string") {
    throw new TypeError("filter text must be a string");
  }
  requireScalarText("filter", text);
  if (utf8ByteLength(text) > FILTER_TEXT_MAX_BYTES) {
    throw syntaxError(
      "filter",
      text,
      indexAtUtf8Offset(text, FILTER_TEXT_MAX_BYTES),
      `filters must not exceed ${FILTER_TEXT_MAX_BYTES} bytes`,
    );
  }
  if (ONLY_WHITESPACE.test(text)) {
    throw syntaxError("filter", text, 0, "expected a filter expression");
  }
  const parser = new Parser(text, false);
  const expression = parser.filter();
  parser.finish();
  try {
    validateTree(expression);
  } catch (error) {
    if (error instanceof FilterTreeError) {
      throw syntaxError("filter", text, 0, error.message, false);
    }
    throw error;
  }
  return expression;
}

function parseSortText(text) {
  if (typeof text !== "string") {
    throw new TypeError("sort specification must be a string");
  }
  requireScalarText("sort", text);
  if (ONLY_WHITESPACE.test(text)) {
    throw syntaxError("sort", text, 0, "expected at least one sort key");
  }
  const parser = new Parser(text, true);
  const keys = [];
  for (;;) {
    const token = parser.peek();
    let descending = false;
    if (token.kind === "minus") {
      parser.advance();
      descending = true;
    }
    const path = parser.path();
    if (keys.some((existing) => existing.field === path)) {
      throw parser.errorAt(token, `sort key \`${renderFieldPath(path)}\` appears more than once`);
    }
    keys.push(new SortKey(path, descending));
    if (keys.length > SORT_MAX_KEYS) {
      throw parser.errorAt(token, `sort specifications accept at most ${SORT_MAX_KEYS} keys`);
    }
    const next = parser.advance();
    if (next.kind === "end") return keys;
    if (next.kind === "comma") continue;
    if (next.kind === "word" && /^(?:asc|desc)$/iu.test(next.value)) {
      throw parser.errorAt(next, "write `field` for ascending and `-field` for descending order");
    }
    throw parser.errorAt(next, `expected \`,\` between sort keys, found ${describe(next)}`);
  }
}
