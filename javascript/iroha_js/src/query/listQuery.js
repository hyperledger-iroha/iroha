/**
 * Collection-query controls (`filter`, `sort`, `select`, `aggregate`,
 * `limit`, `cursor`, `include_total`) and their two wire spellings: the JSON
 * body of `POST /v1/<collection>/query` and the `GET` query parameters.
 *
 * Mirrors `iroha_torii_shared::list_query::ListQuery`.
 */
import { FilterSyntaxError, ListQueryError } from "../toriiErrors.js";
import {
  Filter,
  SORT_MAX_KEYS,
  SortKey,
  fieldPathProblem,
  parseSort,
  renderFieldPath,
  sortToString,
} from "./grammar.js";

/** Maximum number of fields in one projection. */
export const SELECT_MAX_FIELDS = 64;
/** Maximum encoded length of a pagination cursor. */
export const CURSOR_MAX_BYTES = 4096;
/** Largest accepted page size (`u32`). */
const LIMIT_MAX = 4_294_967_295;
/** Maximum number of `group_by` fields in one aggregate. */
const AGGREGATE_MAX_GROUP_BY = 8;
/** Maximum number of metrics in one aggregate. */
const AGGREGATE_MAX_METRICS = 16;
/** JSON members accepted in a list-query body, in canonical order. */
export const LIST_QUERY_MEMBERS = Object.freeze([
  "filter",
  "sort",
  "select",
  "aggregate",
  "limit",
  "cursor",
  "include_total",
]);
/** URL parameters accepted by `GET` collection endpoints. */
export const LIST_QUERY_PARAMETERS = Object.freeze([
  "filter",
  "sort",
  "select",
  "limit",
  "cursor",
  "include_total",
]);
/** Aggregate functions accepted in `aggregate.metrics[].fn`. */
export const AGGREGATE_FUNCTIONS = Object.freeze([
  "count",
  "sum",
  "min",
  "max",
  "avg",
  "distinct_count",
]);

const INPUT_MEMBERS = Object.freeze([
  "filter",
  "sort",
  "select",
  "aggregate",
  "limit",
  "cursor",
  "includeTotal",
]);
const AGGREGATE_INPUT_MEMBERS = Object.freeze(["groupBy", "metrics", "having"]);
const AGGREGATE_MEMBERS = Object.freeze(["group_by", "metrics", "having"]);
const METRIC_INPUT_MEMBERS = Object.freeze(["alias", "fn", "field"]);
const CURSOR_PATTERN = /^[A-Za-z0-9_-]+$/u;
const LIMIT_TEXT = /^\+?[0-9]+$/u;
const QUERY_TOKEN = Symbol("iroha.query.listQuery");

function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function compareCodePoints(left, right) {
  const a = [...left];
  const b = [...right];
  const length = Math.min(a.length, b.length);
  for (let index = 0; index < length; index += 1) {
    const difference = a[index].codePointAt(0) - b[index].codePointAt(0);
    if (difference !== 0) return difference;
  }
  return a.length - b.length;
}

function rejectUnknownMembers(record, allowed, parameter, noun) {
  const unknown = Object.keys(record).filter((key) => !allowed.includes(key));
  if (unknown.length > 0) {
    throw new ListQueryError(
      parameter,
      `unknown ${noun} \`${unknown[0]}\`; expected one of: ${allowed.join(", ")}`,
    );
  }
}

function fieldError(parameter, path) {
  const problem = fieldPathProblem(path);
  return problem === null
    ? null
    : new ListQueryError(parameter, `invalid field \`${path}\`: ${problem}`);
}

// --- normalization of JavaScript inputs ---------------------------------------

function normalizeFilterInput(filter, parameter = "filter") {
  if (filter === undefined || filter === null) return undefined;
  if (filter instanceof Filter) return filter;
  if (typeof filter === "string") return filter;
  if (isPlainRecord(filter)) {
    try {
      return Filter.fromJSON(filter);
    } catch (error) {
      if (parameter !== "filter" && error instanceof ListQueryError) {
        throw new ListQueryError(parameter, `having: ${error.reason}`);
      }
      throw error;
    }
  }
  throw new ListQueryError(
    parameter,
    "a filter must be a Filter, a text filter string or the JSON form {\"op\": ..., \"args\": [...]}",
  );
}

function normalizeSortInput(sort) {
  if (sort === undefined || sort === null) return Object.freeze([]);
  if (typeof sort === "string") return Object.freeze(parseSort(sort));
  if (sort instanceof SortKey) return Object.freeze([sort]);
  if (!Array.isArray(sort)) {
    throw new ListQueryError(
      "sort",
      "`sort` must be a specification such as \"-quantity,id\" or an array of keys",
    );
  }
  return Object.freeze(
    sort.map((key) => {
      if (key instanceof SortKey) return key;
      if (typeof key === "string") return SortKey.parse(key);
      throw new ListQueryError("sort", "sort keys are strings such as \"-quantity\" or SortKey values");
    }),
  );
}

function normalizeSelectInput(select) {
  if (select === undefined || select === null) return undefined;
  if (!Array.isArray(select)) {
    throw new ListQueryError(
      "select",
      "`select` must be an array of field names such as [\"id\", \"quantity\"]",
    );
  }
  return Object.freeze(
    select.map((path) => {
      if (typeof path !== "string") {
        throw new ListQueryError("select", "`select` must be an array of field names");
      }
      return path;
    }),
  );
}

function normalizeMetricInput(metric, index) {
  if (!isPlainRecord(metric)) {
    throw new ListQueryError("aggregate", `metrics[${index}] must be an object such as {alias: "n", fn: "count"}`);
  }
  rejectUnknownMembers(metric, METRIC_INPUT_MEMBERS, "aggregate", `metrics[${index}] member`);
  if (typeof metric.alias !== "string" || metric.alias.length === 0) {
    throw new ListQueryError("aggregate", `metrics[${index}].alias must be a non-empty string`);
  }
  if (!AGGREGATE_FUNCTIONS.includes(metric.fn)) {
    throw new ListQueryError(
      "aggregate",
      `unknown aggregate function \`${String(metric.fn)}\`; expected one of: ${AGGREGATE_FUNCTIONS.join(", ")}`,
    );
  }
  if (metric.field !== undefined && metric.field !== null) {
    if (typeof metric.field !== "string") {
      throw new ListQueryError("aggregate", `metrics[${index}].field must be a field name`);
    }
    const error = fieldError("aggregate", metric.field);
    if (error) throw error;
    return Object.freeze({ alias: metric.alias, fn: metric.fn, field: metric.field });
  }
  return Object.freeze({ alias: metric.alias, fn: metric.fn });
}

function normalizeAggregateInput(aggregate) {
  if (aggregate === undefined || aggregate === null) return undefined;
  if (!isPlainRecord(aggregate)) {
    throw new ListQueryError("aggregate", "`aggregate` must be an object such as {groupBy: [...], metrics: [...]}");
  }
  rejectUnknownMembers(aggregate, AGGREGATE_INPUT_MEMBERS, "aggregate", "member");
  let groupBy = Object.freeze([]);
  if (aggregate.groupBy !== undefined && aggregate.groupBy !== null) {
    if (!Array.isArray(aggregate.groupBy)) {
      throw new ListQueryError("aggregate", "`groupBy` must be an array of field names");
    }
    groupBy = Object.freeze(
      aggregate.groupBy.map((path) => {
        if (typeof path !== "string") {
          throw new ListQueryError("aggregate", "`groupBy` must be an array of field names");
        }
        const error = fieldError("aggregate", path);
        if (error) throw error;
        return path;
      }),
    );
  }
  if (aggregate.metrics !== undefined && aggregate.metrics !== null && !Array.isArray(aggregate.metrics)) {
    throw new ListQueryError("aggregate", "`metrics` must be an array of metrics");
  }
  const metrics = Object.freeze(
    (aggregate.metrics ?? []).map((metric, index) => normalizeMetricInput(metric, index)),
  );
  const having = normalizeFilterInput(aggregate.having, "aggregate");
  return Object.freeze({ groupBy, metrics, having });
}

function normalizeLimitInput(limit) {
  if (limit === undefined || limit === null) return undefined;
  let value = limit;
  if (typeof value === "bigint") {
    if (value < 0n || value > BigInt(LIMIT_MAX)) {
      throw new ListQueryError("limit", value === 0n ? "`limit` must be at least 1" : "`limit` is too large");
    }
    value = Number(value);
  }
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new ListQueryError("limit", "`limit` must be a positive integer");
  }
  if (value > LIMIT_MAX) {
    throw new ListQueryError("limit", "`limit` is too large");
  }
  return value;
}

function normalizeCursorInput(cursor) {
  if (cursor === undefined || cursor === null) return undefined;
  if (typeof cursor !== "string") {
    throw new ListQueryError("cursor", "`cursor` must be the string returned as `next_cursor`");
  }
  return cursor;
}

function normalizeIncludeTotalInput(includeTotal) {
  if (includeTotal === undefined || includeTotal === null) return false;
  if (typeof includeTotal !== "boolean") {
    throw new ListQueryError("include_total", "`include_total` must be true or false");
  }
  return includeTotal;
}

/**
 * The text spelling of a filter for a `?filter=` parameter, as used by event
 * streams: a `Filter` renders canonically (after its limits are checked), a
 * string is sent as-is and the JSON form is decoded first.
 *
 * @param {Filter | string | object | undefined | null} filter
 * @returns {string | undefined}
 */
export function filterQueryText(filter) {
  const normalized = normalizeFilterInput(filter);
  if (normalized === undefined) return undefined;
  if (typeof normalized === "string") return normalized;
  normalized.validate();
  if (hasStructuredLiteral(normalized)) {
    throw new ListQueryError(
      "filter",
      "object and list literals have no text form; compare metadata values in the JSON form through POST /query",
    );
  }
  return normalized.toString();
}

function hasStructuredLiteral(filter) {
  switch (filter.op) {
    case "and":
    case "or":
    case "not":
      return filter.args.some(hasStructuredLiteral);
    case "in":
    case "nin":
      return filter.args[1].some((value) => value !== null && typeof value === "object");
    case "exists":
    case "is_null":
      return false;
    default:
      return filter.args[1] !== null && typeof filter.args[1] === "object";
  }
}

// --- validation (Rust `ListQuery::validate`) ------------------------------------

function validateQuery(query) {
  if (query.filter instanceof Filter) query.filter.validate();
  if (query.sort.length > SORT_MAX_KEYS) {
    throw new ListQueryError("sort", `at most ${SORT_MAX_KEYS} sort keys are allowed`);
  }
  query.sort.forEach((key, index) => {
    if (query.sort.slice(0, index).some((earlier) => earlier.field === key.field)) {
      throw new ListQueryError("sort", `sort key \`${renderFieldPath(key.field)}\` appears more than once`);
    }
  });
  if (query.select !== undefined) {
    if (query.select.length === 0) {
      throw new ListQueryError("select", "`select` must list at least one field");
    }
    if (query.select.length > SELECT_MAX_FIELDS) {
      throw new ListQueryError("select", `at most ${SELECT_MAX_FIELDS} fields can be selected`);
    }
    query.select.forEach((path, index) => {
      const error = fieldError("select", path);
      if (error) throw error;
      if (query.select.slice(0, index).includes(path)) {
        throw new ListQueryError("select", `field \`${renderFieldPath(path)}\` is selected more than once`);
      }
    });
  }
  if (query.select !== undefined && query.aggregate !== undefined) {
    throw new ListQueryError(
      "select",
      "`select` and `aggregate` cannot be combined; aggregates define their own columns",
    );
  }
  if (query.aggregate !== undefined) {
    if (query.aggregate.metrics.length === 0) {
      throw new ListQueryError("aggregate", "`metrics` must list at least one metric");
    }
    if (query.aggregate.groupBy.length > AGGREGATE_MAX_GROUP_BY) {
      throw new ListQueryError(
        "aggregate",
        `\`group_by\` lists at most ${AGGREGATE_MAX_GROUP_BY} fields`,
      );
    }
    if (query.aggregate.metrics.length > AGGREGATE_MAX_METRICS) {
      throw new ListQueryError(
        "aggregate",
        `\`metrics\` lists at most ${AGGREGATE_MAX_METRICS} metrics`,
      );
    }
    const paths = [
      ...query.aggregate.groupBy,
      ...query.aggregate.metrics
        .filter((metric) => metric.field !== undefined)
        .map((metric) => metric.field),
    ];
    for (const path of paths) {
      const error = fieldError("aggregate", path);
      if (error) throw error;
    }
    if (query.aggregate.having instanceof Filter) {
      try {
        query.aggregate.having.validate();
      } catch (error) {
        if (error instanceof ListQueryError) {
          throw new ListQueryError("aggregate", `having: ${error.reason}`);
        }
        throw error;
      }
    }
  }
  if (query.limit === 0) {
    throw new ListQueryError("limit", "`limit` must be at least 1");
  }
  if (query.cursor !== undefined) {
    if (
      query.cursor.length === 0 ||
      query.cursor.length > CURSOR_MAX_BYTES ||
      !CURSOR_PATTERN.test(query.cursor)
    ) {
      throw new ListQueryError(
        "cursor",
        "`cursor` must be a `next_cursor` value returned by a previous page",
      );
    }
  }
}

/**
 * A validated collection query.
 *
 * Construct one with `ListQuery.from({filter, sort, select, aggregate, limit,
 * cursor, includeTotal})`; every collection method also accepts that plain
 * object directly. `filter` is a `Filter`, a text filter (sent as-is) or the
 * JSON form; `sort` is `"-quantity,id"`, an array of keys or `SortKey`s.
 */
export class ListQuery {
  constructor(token, fields) {
    if (token !== QUERY_TOKEN) {
      throw new TypeError("use ListQuery.from(), ListQuery.fromJSON() or ListQuery.fromQueryPairs()");
    }
    this.filter = fields.filter;
    this.sort = fields.sort;
    this.select = fields.select;
    this.aggregate = fields.aggregate;
    this.limit = fields.limit;
    this.cursor = fields.cursor;
    this.includeTotal = fields.includeTotal;
    Object.freeze(this);
  }

  /**
   * Normalize and validate a query given as a plain object (or return an
   * existing `ListQuery` unchanged).
   *
   * @throws {ListQueryError} naming the offending control
   */
  static from(input = {}) {
    if (input instanceof ListQuery) return input;
    if (input === undefined || input === null) return EMPTY_QUERY;
    if (!isPlainRecord(input)) {
      throw new ListQueryError("query", "a collection query must be an object such as {filter, sort, limit}");
    }
    rejectUnknownMembers(input, INPUT_MEMBERS, "query", "member");
    const query = new ListQuery(QUERY_TOKEN, {
      filter: normalizeFilterInput(input.filter),
      sort: normalizeSortInput(input.sort),
      select: normalizeSelectInput(input.select),
      aggregate: normalizeAggregateInput(input.aggregate),
      limit: normalizeLimitInput(input.limit),
      cursor: normalizeCursorInput(input.cursor),
      includeTotal: normalizeIncludeTotalInput(input.includeTotal),
    });
    validateQuery(query);
    return query;
  }

  /**
   * Decode a `POST /v1/<collection>/query` body exactly as Torii does.
   *
   * @throws {ListQueryError} naming the offending member
   */
  static fromJSON(body) {
    if (!isPlainRecord(body)) {
      throw new ListQueryError(
        "query",
        "the request body must be a JSON object such as {\"filter\": \"...\", \"limit\": 50}",
      );
    }
    const fields = {
      filter: undefined,
      sort: Object.freeze([]),
      select: undefined,
      aggregate: undefined,
      limit: undefined,
      cursor: undefined,
      includeTotal: false,
    };
    for (const key of Object.keys(body).sort(compareCodePoints)) {
      const value = body[key];
      switch (key) {
        case "filter":
          fields.filter = value === null ? undefined : decodeFilterMember(value, "filter");
          break;
        case "sort":
          fields.sort = decodeSortMember(value);
          break;
        case "select":
          fields.select = decodeSelectMember(value);
          break;
        case "aggregate":
          fields.aggregate = value === null ? undefined : decodeAggregateMember(value);
          break;
        case "limit":
          fields.limit = value === null ? undefined : decodeLimitMember(value);
          break;
        case "cursor":
          if (value !== null && typeof value !== "string") {
            throw new ListQueryError("cursor", "`cursor` must be the string returned as `next_cursor`");
          }
          fields.cursor = value === null ? undefined : value;
          break;
        case "include_total":
          if (value !== null && typeof value !== "boolean") {
            throw new ListQueryError("include_total", "`include_total` must be true or false");
          }
          fields.includeTotal = value === true;
          break;
        default:
          throw new ListQueryError(
            "query",
            `unknown member \`${key}\`; expected one of: ${LIST_QUERY_MEMBERS.join(", ")}`,
          );
      }
    }
    const query = new ListQuery(QUERY_TOKEN, fields);
    validateQuery(query);
    return query;
  }

  /**
   * Decode `GET` parameters given as already percent-decoded `[key, value]`
   * pairs, exactly as Torii does.
   *
   * @throws {ListQueryError} naming the offending parameter
   */
  static fromQueryPairs(pairs) {
    const fields = {
      filter: undefined,
      sort: Object.freeze([]),
      select: undefined,
      aggregate: undefined,
      limit: undefined,
      cursor: undefined,
      includeTotal: false,
    };
    const seen = new Set();
    for (const pair of pairs) {
      const [key, value] = pair;
      if (!LIST_QUERY_PARAMETERS.includes(key)) {
        const hint = key === "aggregate" ? "; aggregates are only available through POST /query" : "";
        throw new ListQueryError(
          "query",
          `unknown parameter \`${key}\`; expected one of: ${LIST_QUERY_PARAMETERS.join(", ")}${hint}`,
        );
      }
      if (seen.has(key)) {
        throw new ListQueryError(key, `\`${key}\` must appear at most once`);
      }
      seen.add(key);
      switch (key) {
        case "filter":
          fields.filter = Filter.parse(value);
          break;
        case "sort":
          fields.sort = Object.freeze(parseSort(value));
          break;
        case "select":
          fields.select = Object.freeze(value.split(",").map((path) => path.trim()));
          break;
        case "limit": {
          if (!LIMIT_TEXT.test(value) || BigInt(value) > 18_446_744_073_709_551_615n) {
            throw new ListQueryError("limit", `\`limit\` must be a positive integer, got \`${value}\``);
          }
          fields.limit = decodeLimitMember(BigInt(value));
          break;
        }
        case "cursor":
          fields.cursor = value;
          break;
        default:
          if (value !== "true" && value !== "false") {
            throw new ListQueryError(
              "include_total",
              `\`include_total\` must be \`true\` or \`false\`, got \`${value}\``,
            );
          }
          fields.includeTotal = value === "true";
      }
    }
    const query = new ListQuery(QUERY_TOKEN, fields);
    validateQuery(query);
    return query;
  }

  /** The same query continued after a page's `nextCursor`. */
  withCursor(cursor) {
    const next = new ListQuery(QUERY_TOKEN, { ...this, cursor: normalizeCursorInput(cursor) });
    validateQuery(next);
    return next;
  }

  /**
   * Canonical JSON body for `POST /v1/<collection>/query` (members in the
   * order filter, sort, select, aggregate, limit, cursor, include_total;
   * absent members are omitted). Integers outside the safe range are `bigint`.
   */
  toJSON() {
    const body = {};
    if (this.filter !== undefined) {
      body.filter = typeof this.filter === "string" ? this.filter : this.filter.toJSON();
    }
    if (this.sort.length > 0) body.sort = this.sort.map((key) => key.toString());
    if (this.select !== undefined) body.select = [...this.select];
    if (this.aggregate !== undefined) body.aggregate = aggregateToJson(this.aggregate);
    if (this.limit !== undefined) body.limit = this.limit;
    if (this.cursor !== undefined) body.cursor = this.cursor;
    if (this.includeTotal) body.include_total = true;
    return body;
  }

  /**
   * `GET` parameters (not yet percent-encoded) in canonical order.
   *
   * @throws {ListQueryError} for aggregates, which have no URL form
   */
  toQueryPairs() {
    if (this.aggregate !== undefined) {
      throw new ListQueryError("aggregate", "aggregates are only available through POST /query");
    }
    const pairs = [];
    if (this.filter !== undefined) {
      pairs.push(["filter", typeof this.filter === "string" ? this.filter : this.filter.toString()]);
    }
    if (this.sort.length > 0) pairs.push(["sort", sortToString(this.sort)]);
    if (this.select !== undefined) pairs.push(["select", this.select.join(",")]);
    if (this.limit !== undefined) pairs.push(["limit", String(this.limit)]);
    if (this.cursor !== undefined) pairs.push(["cursor", this.cursor]);
    if (this.includeTotal) pairs.push(["include_total", "true"]);
    return pairs;
  }
}

const EMPTY_QUERY = new ListQuery(QUERY_TOKEN, {
  filter: undefined,
  sort: Object.freeze([]),
  select: undefined,
  aggregate: undefined,
  limit: undefined,
  cursor: undefined,
  includeTotal: false,
});

function aggregateToJson(aggregate) {
  const out = {};
  if (aggregate.groupBy.length > 0) out.group_by = [...aggregate.groupBy];
  out.metrics = aggregate.metrics.map((metric) =>
    metric.field === undefined
      ? { alias: metric.alias, fn: metric.fn }
      : { alias: metric.alias, fn: metric.fn, field: metric.field },
  );
  if (aggregate.having !== undefined) {
    out.having = typeof aggregate.having === "string" ? aggregate.having : aggregate.having.toJSON();
  }
  return out;
}

// --- body member decoding (Rust `ListQuery::from_json_value`) -------------------

function decodeFilterMember(value, parameter) {
  try {
    return typeof value === "string" ? Filter.parse(value) : Filter.fromJSON(value);
  } catch (error) {
    if (parameter !== "filter" && error instanceof ListQueryError) {
      const reason = error instanceof FilterSyntaxError
        ? `${error.reason} (column ${error.column})`
        : error.reason;
      throw new ListQueryError(parameter, `having: ${reason}`);
    }
    throw error;
  }
}

function decodeSortMember(value) {
  if (value === null) return Object.freeze([]);
  if (!Array.isArray(value)) {
    throw new ListQueryError("sort", "`sort` must be an array of keys such as [\"-quantity\", \"id\"]");
  }
  return Object.freeze(
    value.map((key) => {
      if (typeof key !== "string") {
        throw new ListQueryError("sort", "sort keys are strings such as \"-quantity\" or \"id\"");
      }
      return SortKey.parse(key);
    }),
  );
}

function decodeSelectMember(value) {
  if (value === null) return undefined;
  if (!Array.isArray(value)) {
    throw new ListQueryError(
      "select",
      "`select` must be an array of field names such as [\"id\", \"quantity\"]",
    );
  }
  return Object.freeze(
    value.map((path) => {
      if (typeof path !== "string") {
        throw new ListQueryError("select", "`select` must be an array of field names");
      }
      return path;
    }),
  );
}

function decodeLimitMember(value) {
  const integer =
    typeof value === "bigint"
      ? value
      : typeof value === "number" && Number.isSafeInteger(value) && value >= 0
        ? BigInt(value)
        : null;
  if (integer === null || integer < 0n) {
    throw new ListQueryError("limit", "`limit` must be a positive integer");
  }
  if (integer === 0n) {
    throw new ListQueryError("limit", "`limit` must be at least 1");
  }
  if (integer > BigInt(LIMIT_MAX)) {
    throw new ListQueryError("limit", "`limit` is too large");
  }
  return Number(integer);
}

function decodeAggregateMember(value) {
  if (!isPlainRecord(value)) {
    throw new ListQueryError("aggregate", "`aggregate` must be an object");
  }
  rejectUnknownMembers(value, AGGREGATE_MEMBERS, "aggregate", "member");
  const groupByValue = value.group_by ?? [];
  if (!Array.isArray(groupByValue) || groupByValue.some((path) => typeof path !== "string")) {
    throw new ListQueryError("aggregate", "`group_by` must be an array of field names");
  }
  const metricsValue = value.metrics ?? [];
  if (!Array.isArray(metricsValue)) {
    throw new ListQueryError("aggregate", "`metrics` must be an array of metrics");
  }
  const metrics = metricsValue.map((metric, index) => {
    if (!isPlainRecord(metric)) {
      throw new ListQueryError("aggregate", `metrics[${index}] must be an object`);
    }
    rejectUnknownMembers(metric, METRIC_INPUT_MEMBERS, "aggregate", `metrics[${index}] member`);
    if (typeof metric.alias !== "string") {
      throw new ListQueryError("aggregate", `metrics[${index}].alias must be a string`);
    }
    if (!AGGREGATE_FUNCTIONS.includes(metric.fn)) {
      throw new ListQueryError(
        "aggregate",
        `unknown aggregate function \`${String(metric.fn)}\`; expected one of: ${AGGREGATE_FUNCTIONS.join(", ")}`,
      );
    }
    if (metric.field !== undefined && metric.field !== null && typeof metric.field !== "string") {
      throw new ListQueryError("aggregate", `metrics[${index}].field must be a field name`);
    }
    return metric.field === undefined || metric.field === null
      ? Object.freeze({ alias: metric.alias, fn: metric.fn })
      : Object.freeze({ alias: metric.alias, fn: metric.fn, field: metric.field });
  });
  const having =
    value.having === undefined || value.having === null
      ? undefined
      : decodeFilterMember(value.having, "aggregate");
  return Object.freeze({
    groupBy: Object.freeze([...groupByValue]),
    metrics: Object.freeze(metrics),
    having,
  });
}
