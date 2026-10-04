//! Golden vectors and unit tests for the Torii collection-query language.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  Filter,
  FilterSyntaxError,
  ListQuery,
  ListQueryError,
  SortKey,
  ToriiError,
  decodePage,
  field,
  parseSort,
  sortToString,
} from "../src/index.js";
import * as browserSdk from "../src/browser.js";
import { decodePageText } from "../src/query/page.js";

const VECTORS = JSON.parse(
  readFileSync(new URL("../../../fixtures/torii/list_query/vectors.json", import.meta.url), "utf8"),
);

function assertListQueryError(error, code, parameter) {
  assert(error instanceof ToriiError, `${error} is a ToriiError`);
  assert(error instanceof ListQueryError, `${error} is a ListQueryError`);
  assert.equal(error.code, code);
  if (parameter !== undefined) assert.equal(error.parameter, parameter);
  return true;
}

test("golden vectors: version 1", () => {
  assert.equal(VECTORS.version, 1);
});

test("golden vectors: every filter parses, renders canonically and round-trips its JSON form", () => {
  assert(VECTORS.filters.length > 0);
  for (const vector of VECTORS.filters) {
    const parsed = Filter.parse(vector.text);
    assert.deepEqual(parsed.toJSON(), vector.json, vector.text);
    assert.equal(parsed.toString(), vector.canonical, vector.text);
    const decoded = Filter.fromJSON(vector.json);
    assert.equal(decoded.toString(), vector.canonical, vector.text);
    assert.deepEqual(decoded.toJSON(), vector.json, vector.text);
    assert.deepEqual(Filter.parse(vector.canonical).toJSON(), vector.json, vector.canonical);
    assert.equal(String(decoded), vector.canonical);
  }
});

test("golden vectors: JSON-form filters decode to their normalized tree", () => {
  assert(VECTORS.json_filters.length > 0);
  for (const vector of VECTORS.json_filters) {
    const label = JSON.stringify(vector.json);
    const decoded = Filter.fromJSON(vector.json);
    assert.equal(decoded.toString(), vector.canonical, label);
    assert.deepEqual(decoded.toJSON(), vector.normalized, label);
    assert.deepEqual(Filter.parse(vector.canonical).toJSON(), vector.normalized, label);
    assert.deepEqual(ListQuery.fromJSON({ filter: vector.json }).toJSON(), { filter: vector.normalized }, label);
  }
});

test("golden vectors: filter syntax errors report the reason, line and column", () => {
  for (const vector of VECTORS.filter_errors) {
    assert.throws(
      () => Filter.parse(vector.text),
      (error) => {
        assert(error instanceof FilterSyntaxError, vector.text);
        assertListQueryError(error, "invalid_filter", "filter");
        assert.equal(error.reason, vector.message, vector.text);
        assert.equal(error.line, vector.line, vector.text);
        assert.equal(error.column, vector.column, vector.text);
        assert.match(error.message, /^invalid `filter`: /u);
        return true;
      },
    );
  }
});

test("golden vectors: sort specifications", () => {
  for (const vector of VECTORS.sorts) {
    const keys = parseSort(vector.text);
    assert.equal(sortToString(keys), vector.canonical);
    assert.deepEqual(keys.map(String), vector.json);
    assert.deepEqual(vector.json.map((text) => SortKey.parse(text).toString()), vector.json);
  }
  for (const vector of VECTORS.sort_errors) {
    assert.throws(
      () => parseSort(vector.text),
      (error) => {
        assert(error instanceof FilterSyntaxError, vector.text);
        assertListQueryError(error, "invalid_sort", "sort");
        assert.equal(error.reason, vector.message, vector.text);
        assert.equal(error.line, vector.line, vector.text);
        assert.equal(error.column, vector.column, vector.text);
        return true;
      },
    );
  }
});

test("golden vectors: query bodies and GET pairs", () => {
  for (const vector of VECTORS.queries) {
    const query = ListQuery.fromJSON(vector.body);
    assert.deepEqual(query.toJSON(), vector.body);
    assert.deepEqual(query.toQueryPairs(), vector.query_pairs);
    assert.deepEqual(ListQuery.fromQueryPairs(vector.query_pairs).toJSON(), vector.body);
  }
  const built = ListQuery.from({
    filter: field("owned_by").eq("alice").and(field("quantity").gt(1)),
    sort: [field("quantity").desc(), "id"],
    select: ["id", "quantity"],
    limit: 25,
    includeTotal: true,
  });
  assert.deepEqual(built.toJSON(), VECTORS.queries[1].body);
  assert.deepEqual(built.toQueryPairs(), VECTORS.queries[1].query_pairs);
  assert.deepEqual(
    Object.keys(built.toJSON()),
    ["filter", "sort", "select", "limit", "include_total"],
    "members are emitted in canonical order",
  );
  const continued = ListQuery.from({ limit: 10, cursor: "q1_abc-DEF" });
  assert.deepEqual(continued.toJSON(), VECTORS.queries[2].body);
  assert.deepEqual(continued.toQueryPairs(), VECTORS.queries[2].query_pairs);
  assert.deepEqual(ListQuery.from().toJSON(), VECTORS.queries[0].body);
});

test("golden vectors: page envelopes", () => {
  for (const vector of VECTORS.pages) {
    const page = decodePage(vector.json);
    assert.deepEqual(page.items, vector.json.items);
    assert.equal(page.nextCursor !== null, vector.has_more);
    assert.equal(page.nextCursor, vector.json.next_cursor ?? null);
    assert.equal(page.total, vector.json.total);
  }
});

test("golden vectors: rejected bodies and GET pairs carry the server error code", () => {
  for (const vector of VECTORS.query_body_errors) {
    assert.throws(
      () => ListQuery.fromJSON(vector.body),
      (error) => assertListQueryError(error, vector.code, vector.parameter),
      JSON.stringify(vector.body),
    );
  }
  for (const vector of VECTORS.query_pair_errors) {
    assert.throws(
      () => ListQuery.fromQueryPairs(vector.query_pairs),
      (error) => assertListQueryError(error, vector.code, vector.parameter),
      JSON.stringify(vector.query_pairs),
    );
  }
});

test("builder matches the parser and renders canonical text", () => {
  const built = field("owned_by")
    .eq("alice")
    .and(field("quantity").gte("10.5"))
    .and(field("status").in(["A", "B"]).or(field("tier").lt(-1)))
    .and(field("metadata.frozen").exists().not())
    .and(field("note").isNotNull());
  const parsed = Filter.parse(
    `owned_by = "alice" and quantity >= 10.5 and (status in ["A", "B"] or tier < -1)
       and not exists(metadata.frozen) and note is not null`,
  );
  assert.deepEqual(built.toJSON(), parsed.toJSON());
  assert.equal(
    built.toString(),
    'owned_by = "alice" and quantity >= "10.5" and (status in ["A", "B"] or tier < -1) and not exists(metadata.frozen) and note is not null',
  );
  const tier = field("tier");
  assert.equal(tier.gt(1).and(tier.lt(5)).toString(), "tier > 1 and tier < 5");
  assert.equal(field("id").desc().toString(), "-id");
  assert.equal(field("id").asc().toString(), "id");
  assert.equal(field("a").notIn([1, 2]).toString(), "a not in [1, 2]");
  assert.equal(field("a").isNull().toString(), "a is null");
  assert.equal(Filter.not(field("a").eq(1).and(field("b").eq(2))).toString(), "not (a = 1 and b = 2)");
  assert.equal(
    Filter.or(field("a").eq(1), field("b").eq(2)).and(field("c").eq(3)).toString(),
    "(a = 1 or b = 2) and c = 3",
  );
  assert.equal(field("metadata.display-name").eq("x").toString(), 'metadata.`display-name` = "x"');
  assert.equal(field("and").eq(1).toString(), "`and` = 1");
});

test("and/or flatten chains like the Rust builder", () => {
  const a = field("a").eq(1);
  const b = field("b").eq(2);
  const c = field("c").eq(3);
  assert.equal(a.and(b).and(c).args.length, 3);
  assert.equal(a.and(b.and(c)).args.length, 3);
  assert.equal(Filter.and(a, b, c).args.length, 3);
  assert.equal(Filter.or(a, b).or(Filter.or(c, a)).args.length, 4);
  assert.throws(() => a.and("b = 2"), TypeError);
});

test("literals follow the text rules and reject inexact numbers", () => {
  assert.deepEqual(field("a").eq(7).toJSON(), { op: "eq", args: ["a", 7] });
  assert.deepEqual(field("a").eq(-7n).toJSON(), { op: "eq", args: ["a", -7] });
  assert.deepEqual(field("a").eq(2n ** 64n - 1n).toJSON(), {
    op: "eq",
    args: ["a", 18_446_744_073_709_551_615n],
  });
  assert.equal(field("a").eq(2n ** 64n - 1n).toString(), "a = 18446744073709551615");
  assert.deepEqual(field("a").eq(2n ** 64n).toJSON(), {
    op: "eq",
    args: ["a", "18446744073709551616"],
  });
  assert.deepEqual(field("a").eq({ mantissa: 105n, scale: 1 }).toJSON(), {
    op: "eq",
    args: ["a", "10.5"],
  });
  assert.deepEqual(field("a").eq({ mantissa: 25n, scale: 0 }).toJSON(), {
    op: "eq",
    args: ["a", 25],
  });
  assert.deepEqual(field("a").eq(-0).toJSON(), { op: "eq", args: ["a", 0] });
  for (const value of [10.5, Number.MAX_SAFE_INTEGER + 2, Number.NaN, Infinity, undefined, () => 1, Symbol("x")]) {
    assert.throws(() => field("a").eq(value), (error) => assertListQueryError(error, "invalid_filter"));
  }
  assert.throws(() => field("a").eq("\ud800"), (error) => assertListQueryError(error, "invalid_filter"));
});

test("structured literals are limited to metadata fields and render with sorted keys", () => {
  const filter = field("metadata.tags").eq({ b: 1, a: [true, "x"] });
  // Rendered exactly like Torii's Display; the text grammar has no object or
  // list literal, so such filters travel in the JSON form (POST /query) only.
  assert.equal(filter.toString(), 'metadata.tags = {"a":[true,"x"],"b":1}');
  assert.deepEqual(filter.toJSON(), { op: "eq", args: ["metadata.tags", { a: [true, "x"], b: 1 }] });
  assert.deepEqual(Filter.fromJSON(filter.toJSON()).toJSON(), filter.toJSON());
  assert.deepEqual(ListQuery.from({ filter }).toJSON(), { filter: filter.toJSON() });
  assert.throws(() => field("tags").eq(["a"]), (error) => assertListQueryError(error, "invalid_filter"));
});

test("membership operands are validated", () => {
  assert.throws(() => field("a").in([]), /membership lists must not be empty/u);
  assert.throws(() => field("a").in([1, 1]), /must be unique/u);
  assert.throws(() => field("a").in(["x", 1]), /strings, numbers or booleans/u);
  assert.equal(field("metadata.x").in(["x", 1]).toString(), 'metadata.x in ["x", 1]');
  assert.equal(field("a").in(["1", 2]).toString(), 'a in ["1", 2]');
  assert.throws(() => field("a").in("abc"), (error) => assertListQueryError(error, "invalid_filter"));
  assert.equal(field("a").in(new Set(["x", "y"])).toString(), 'a in ["x", "y"]');
});

test("range comparisons need numbers, decimals or strings", () => {
  assert.throws(() => field("a").lt(true), /range comparisons need a number/u);
  assert.throws(() => field("a").gte(null), /range comparisons need a number/u);
  assert.equal(field("a").gte("2024-01-01").toString(), 'a >= "2024-01-01"');
});

test("field paths are validated", () => {
  for (const path of ["", "a b", "a..b", ".a", "a\u0007", "a\u007f", "a`b"]) {
    assert.throws(() => field(path), (error) => assertListQueryError(error, "invalid_filter"), path);
  }
  assert.throws(() => field(42), (error) => assertListQueryError(error, "invalid_filter"));
  assert.throws(() => new SortKey("a b"), (error) => assertListQueryError(error, "invalid_sort"));
});

test("field paths must not contain backticks, which the text form cannot quote", () => {
  assert.throws(
    () => field("metadata.a`b"),
    (error) => {
      assertListQueryError(error, "invalid_filter", "filter");
      assert.equal(error.reason, "invalid field `metadata.a`b`: field paths must not contain backticks");
      return true;
    },
  );
  assert.throws(
    () => Filter.fromJSON({ op: "exists", args: ["a`b"] }),
    (error) => assertListQueryError(error, "invalid_filter", "filter") && /must not contain backticks/u.test(error.reason),
  );
  assert.throws(() => Filter.fromJSON({ op: "in", args: ["a`b", [1]] }), /must not contain backticks/u);
  assert.throws(() => new SortKey("a`b"), (error) => assertListQueryError(error, "invalid_sort", "sort"));
  assert.throws(() => ListQuery.fromJSON({ select: ["a`b"] }), (error) => assertListQueryError(error, "invalid_select", "select"));
  assert.throws(() => ListQuery.from({ select: ["id", "a`b"] }), (error) => assertListQueryError(error, "invalid_select", "select"));
  // A backtick in the text form always opens or closes a quoted segment.
  assert.equal(Filter.parse("`a-b`.c = 1").toJSON().args[0], "a-b.c");
});

test("string literals accept raw DEL and C1 characters but not U+0000..U+001F", () => {
  const raw = "x\u007fy\u0080\u0085\u009fz";
  const parsed = Filter.parse(`a = "${raw}"`);
  assert.deepEqual(parsed.toJSON(), { op: "eq", args: ["a", raw] });
  assert.equal(parsed.toString(), `a = "${raw}"`, "DEL and C1 render raw");
  assert.deepEqual(Filter.parse(`a = '${raw}'`).toJSON(), parsed.toJSON());
  for (const control of ["\u0000", "\u0001", "\t", "\n", "\u001f"]) {
    assert.throws(
      () => Filter.parse(`a = "x${control}y"`),
      (error) => {
        assert(error instanceof FilterSyntaxError);
        assert.equal(error.reason, "control characters must be escaped inside string literals");
        return true;
      },
      JSON.stringify(control),
    );
  }
  // Only `"`, `\` and U+0000..U+001F are escaped, with JSON's short forms.
  const value = "q\"b\\s\b\f\n\r\t\u0000\u001f\u007f\u0085/'";
  const rendered = field("a").eq(value).toString();
  assert.equal(rendered, 'a = "q\\"b\\\\s\\b\\f\\n\\r\\t\\u0000\\u001f\u007f\u0085/\'"');
  assert.deepEqual(Filter.parse(rendered).toJSON(), { op: "eq", args: ["a", value] });
});

test("a one-operand and/or in the JSON form decodes to its operand", () => {
  const leaf = { op: "eq", args: ["a", 1] };
  for (const op of ["and", "or"]) {
    const decoded = Filter.fromJSON({ op, args: [leaf] });
    assert.equal(decoded.op, "eq");
    assert.deepEqual(decoded.toJSON(), leaf);
    assert.equal(decoded.toString(), "a = 1");
  }
  const nested = Filter.fromJSON({
    op: "and",
    args: [{ op: "or", args: [{ op: "and", args: [leaf, { op: "is_null", args: ["b"] }] }] }, { op: "or", args: [leaf] }],
  });
  assert.equal(nested.toString(), "(a = 1 and b is null) and a = 1");
  assert.deepEqual(nested.toJSON(), {
    op: "and",
    args: [{ op: "and", args: [leaf, { op: "is_null", args: ["b"] }] }, leaf],
  });
  assert.throws(() => Filter.fromJSON({ op: "and", args: [] }), /needs at least one operand/u);
  // The collapsed connective still counts toward the depth limit.
  let deep = leaf;
  for (let index = 0; index <= 10; index += 1) deep = { op: "and", args: [deep] };
  assert.throws(() => Filter.fromJSON(deep), /nesting depth limit of 10/u);
});

test("aggregates reject unknown members, bound their size and validate their paths", () => {
  const count = { alias: "n", fn: "count" };
  const rejected = [
    [{ aggregate: { groupby: ["a"], metrics: [count] } }, /unknown member `groupby`/u],
    [{ aggregate: { metrics: [{ ...count, feild: "a" }] } }, /unknown metrics\[0\] member `feild`/u],
    [{ aggregate: { group_by: "abcdefghi".split(""), metrics: [count] } }, /`group_by` lists at most 8 fields/u],
    [
      { aggregate: { metrics: Array.from({ length: 17 }, (_, index) => ({ alias: `m${index}`, fn: "count" })) } },
      /`metrics` lists at most 16 metrics/u,
    ],
    [{ aggregate: { group_by: ["a..b"], metrics: [count] } }, /invalid field `a\.\.b`/u],
    [{ aggregate: { group_by: ["a`b"], metrics: [count] } }, /must not contain backticks/u],
    [{ aggregate: { metrics: [{ alias: "s", fn: "sum", field: "a b" }] } }, /invalid field `a b`/u],
  ];
  for (const [body, pattern] of rejected) {
    assert.throws(
      () => ListQuery.fromJSON(body),
      (error) => assertListQueryError(error, "invalid_aggregate", "aggregate") && pattern.test(error.reason),
      JSON.stringify(body),
    );
  }
  const widest = {
    group_by: "abcdefgh".split(""),
    metrics: Array.from({ length: 16 }, (_, index) => ({ alias: `m${index}`, fn: "count" })),
  };
  assert.deepEqual(ListQuery.fromJSON({ aggregate: widest }).toJSON(), { aggregate: widest });
  assert.throws(
    () => ListQuery.from({ aggregate: { groupBy: "abcdefghi".split(""), metrics: [count] } }),
    (error) => assertListQueryError(error, "invalid_aggregate", "aggregate"),
  );
  assert.throws(
    () => ListQuery.from({ aggregate: { metrics: [{ ...count, feild: "a" }] } }),
    (error) => assertListQueryError(error, "invalid_aggregate", "aggregate"),
  );
});

test("structural limits apply to built, parsed and decoded filters", () => {
  let deep = field("a").eq(true);
  for (let index = 0; index <= 10; index += 1) deep = deep.not();
  assert.throws(() => deep.validate(), /nesting depth limit of 10/u);
  assert.throws(() => ListQuery.from({ filter: deep }), (error) => assertListQueryError(error, "invalid_filter", "filter"));
  assert.throws(() => Filter.fromJSON(deep.toJSON()), /nesting depth limit of 10/u);
  const wide = { op: "and", args: Array.from({ length: 1_024 }, () => ({ op: "eq", args: ["a", true] })) };
  assert.throws(() => Filter.fromJSON(wide), /node count limit of 1024/u);
});

test("over-length filters report the limit position on a character boundary", () => {
  const cases = [
    // The 32768th byte is the second byte of a two-byte character: the
    // position moves back to that character, as in Rust.
    ['a = "' + "\u00e9".repeat(16_384) + '"', 32_767, 16_387],
    ['a = "' + "x".repeat(32_762) + "\u{1f600}" + '"', 32_767, 32_768],
    ['a = "' + "x".repeat(32_763) + "\u20ac" + '"', 32_768, 32_769],
  ];
  for (const [text, offset, column] of cases) {
    assert.throws(
      () => Filter.parse(text),
      (error) => {
        assert(error instanceof FilterSyntaxError);
        assert.equal(error.code, "invalid_filter");
        assert.equal(error.reason, "filters must not exceed 32768 bytes");
        assert.equal(error.offset, offset);
        assert.equal(error.line, 1);
        assert.equal(error.column, column);
        assert.equal(error.message, `invalid \`filter\`: filters must not exceed 32768 bytes (column ${column})`);
        return true;
      },
    );
  }
});

test("fractional JSON numbers are rejected in every literal position", () => {
  for (const json of [
    { op: "eq", args: ["quantity", 1.5] },
    { op: "in", args: ["quantity", [1, 2.5]] },
    { op: "eq", args: ["metadata.price", { amount: 0.1 }] },
  ]) {
    assert.throws(() => Filter.fromJSON(json), (error) => assertListQueryError(error, "invalid_filter"));
  }
  assert.deepEqual(Filter.parse("quantity >= 10.5").toJSON(), { op: "gte", args: ["quantity", "10.5"] });
  // Decimals are exact strings, so the canonical text quotes them.
  assert.equal(Filter.fromJSON({ op: "gte", args: ["quantity", "10.5"] }).toString(), 'quantity >= "10.5"');
});

test("field paths are quoted where text is parsed and raw where a path is its own JSON value", () => {
  const query = ListQuery.from({
    filter: field("metadata.ui-order").gte(2),
    sort: [SortKey.desc("metadata.ui-order"), "id"],
    select: ["id", "metadata.ui-order"],
  });
  assert.deepEqual(query.toJSON(), {
    filter: { op: "gte", args: ["metadata.ui-order", 2] },
    sort: ["-metadata.`ui-order`", "id"],
    select: ["id", "metadata.ui-order"],
  });
  assert.deepEqual(query.toQueryPairs(), [
    ["filter", "metadata.`ui-order` >= 2"],
    ["sort", "-metadata.`ui-order`,id"],
    ["select", "id,metadata.ui-order"],
  ]);
  assert.equal(ListQuery.from({ sort: "-metadata.`ui-order`" }).sort[0].field, "metadata.ui-order");
  assert.deepEqual(
    ListQuery.from({ aggregate: { groupBy: ["metadata.ui-order"], metrics: [{ alias: "n", fn: "sum", field: "metadata.ui-order" }] } }).toJSON(),
    { aggregate: { group_by: ["metadata.ui-order"], metrics: [{ alias: "n", fn: "sum", field: "metadata.ui-order" }] } },
  );
});

test("JSON-form errors name the offending node", () => {
  assert.throws(
    () => Filter.fromJSON({ op: "and", args: [{ op: "eq", args: ["a", 1] }, { op: "between", args: ["b", 1, 2] }] }),
    (error) => {
      assertListQueryError(error, "invalid_filter", "filter");
      assert.match(error.message, /unknown operator `between`/u);
      assert.match(error.message, /args\[1\]/u);
      return true;
    },
  );
  assert.throws(() => Filter.fromJSON({ op: "eq", args: ["a", 1], extra: 1 }), /unknown member `extra`/u);
  assert.throws(() => Filter.fromJSON({ Eq: ["a", 1] }), /needs an `op` member/u);
  assert.throws(() => Filter.fromJSON({ Pipeline: { Block: {} } }), /needs an `op` member/u);
  assert.throws(() => Filter.fromJSON({ op: "gt", args: ["a", 1.5] }), /exact decimal strings/u);
});

test("Filter instances are immutable and cannot be forged", () => {
  const filter = field("a").eq(1);
  assert(Object.isFrozen(filter));
  assert(Object.isFrozen(filter.args));
  assert.throws(() => new Filter(Symbol("x"), "eq", ["a", 1]), TypeError);
  assert.throws(() => new ListQuery(Symbol("x"), {}), TypeError);
});

test("ListQuery.from validates every control client-side", () => {
  const cases = [
    [{ offset: 10 }, "invalid_query", "query"],
    [{ count_mode: "exact" }, "invalid_query", "query"],
    [{ limit: 0 }, "invalid_limit", "limit"],
    [{ limit: 1.5 }, "invalid_limit", "limit"],
    [{ limit: 2 ** 32 }, "invalid_limit", "limit"],
    [{ cursor: "has space" }, "invalid_cursor", "cursor"],
    [{ cursor: 7 }, "invalid_cursor", "cursor"],
    [{ includeTotal: "yes" }, "invalid_include_total", "include_total"],
    [{ select: [] }, "invalid_select", "select"],
    [{ select: ["id", "id"] }, "invalid_select", "select"],
    [{ select: "id" }, "invalid_select", "select"],
    [{ select: ["id"], aggregate: { metrics: [{ alias: "n", fn: "count" }] } }, "invalid_select", "select"],
    [{ aggregate: { metrics: [] } }, "invalid_aggregate", "aggregate"],
    [{ aggregate: { metrics: [{ alias: "n", fn: "median" }] } }, "invalid_aggregate", "aggregate"],
    [{ aggregate: { group_by: ["a"], metrics: [{ alias: "n", fn: "count" }] } }, "invalid_aggregate", "aggregate"],
    [{ sort: "id:desc" }, "invalid_sort", "sort"],
    [{ sort: ["id", "-id"] }, "invalid_sort", "sort"],
    [{ sort: { key: "id" } }, "invalid_sort", "sort"],
    [{ filter: 42 }, "invalid_filter", "filter"],
    [{ filter: { Eq: ["id", "x"] } }, "invalid_filter", "filter"],
  ];
  for (const [input, code, parameter] of cases) {
    assert.throws(
      () => ListQuery.from(input),
      (error) => assertListQueryError(error, code, parameter),
      JSON.stringify(input),
    );
  }
  assert.throws(() => ListQuery.from("limit=5"), (error) => assertListQueryError(error, "invalid_query"));
});

test("ListQuery encodes text filters as-is, aggregates and cursors", () => {
  const text = 'owned_by = "alice" and quantity > 1';
  const query = ListQuery.from({ filter: text, sort: "-quantity,id", limit: 5 });
  assert.deepEqual(query.toJSON(), { filter: text, sort: ["-quantity", "id"], limit: 5 });
  assert.deepEqual(query.toQueryPairs(), [
    ["filter", text],
    ["sort", "-quantity,id"],
    ["limit", "5"],
  ]);
  const aggregate = ListQuery.from({
    filter: field("quantity").gt(0),
    aggregate: {
      groupBy: ["asset"],
      metrics: [
        { alias: "holders", fn: "count" },
        { alias: "supply", fn: "sum", field: "quantity" },
      ],
      having: field("holders").gte(10),
    },
    sort: ["-supply"],
    limit: 20,
  });
  assert.deepEqual(aggregate.toJSON(), {
    filter: { op: "gt", args: ["quantity", 0] },
    sort: ["-supply"],
    aggregate: {
      group_by: ["asset"],
      metrics: [
        { alias: "holders", fn: "count" },
        { alias: "supply", fn: "sum", field: "quantity" },
      ],
      having: { op: "gte", args: ["holders", 10] },
    },
    limit: 20,
  });
  assert.deepEqual(ListQuery.fromJSON(aggregate.toJSON()).toJSON(), aggregate.toJSON());
  assert.throws(() => aggregate.toQueryPairs(), (error) => assertListQueryError(error, "invalid_aggregate"));
  const next = query.withCursor("q1_next");
  assert.equal(next.cursor, "q1_next");
  assert.equal(next.limit, 5);
  assert.equal(query.cursor, undefined, "withCursor does not mutate the original query");
  assert.throws(() => query.withCursor("not a cursor"), (error) => assertListQueryError(error, "invalid_cursor"));
  assert.strictEqual(ListQuery.from(query), query);
});

test("page envelopes reject malformed members and keep unknown ones out", () => {
  assert.throws(() => decodePage([]), (error) => error instanceof ToriiError && error.code === "invalid_response");
  assert.throws(() => decodePage({ items: {} }), /`items` array/u);
  assert.throws(() => decodePage({ items: [], next_cursor: 7 }), /next_cursor/u);
  assert.throws(() => decodePage({ items: [], next_cursor: null, total: -1 }), /total/u);
  assert.throws(() => decodePage({ items: [], next_cursor: null, total: 2n ** 64n }), /total/u);
  assert.throws(() => decodePage({ items: [], next_cursor: null, total: 1.5 }), /total/u);
  assert.equal(decodePage({ items: [], next_cursor: null, total: 2n ** 60n }).total, 2n ** 60n);
  assert.equal(
    decodePageText('{"items":[],"next_cursor":null,"total":18446744073709551615}').total,
    18_446_744_073_709_551_615n,
    "a u64 total beyond 2^53 is a bigint",
  );
  assert.equal(decodePageText('{"items":[],"next_cursor":null,"total":12}').total, 12);
  for (const payload of [
    { items: [] },
    { items: [], next_cursor: "" },
    { items: [1], next_cursor: null, total: 0 },
    { items: [], next_cursor: null, total: null },
    { items: [], next_cursor: null, has_more: false },
    { items: [], next_cursor: null, count_mode: "exact" },
  ]) assert.throws(() => decodePage(payload), error => error.code === "invalid_response");
});

test("the browser aggregate exports the same query API", () => {
  for (const name of ["Filter", "ListQuery", "SortKey", "field", "parseSort", "ToriiError", "ToriiHttpError", "ListQueryError", "FilterSyntaxError", "ToriiStreamGapError", "ToriiCollection", "decodePage"]) {
    assert.equal(typeof browserSdk[name], "function", name);
  }
  assert.equal(browserSdk.Filter, Filter);
});
