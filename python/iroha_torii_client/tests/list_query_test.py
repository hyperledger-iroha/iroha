"""Golden-vector and builder tests for the shared collection-query core."""

from __future__ import annotations

import copy
import json
import pickle
import sys
from decimal import Decimal
from pathlib import Path

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client.list_query import (  # noqa: E402  (import depends on sys.path mutation)
    AGGREGATE_MAX_GROUP_BY,
    AGGREGATE_MAX_METRICS,
    FILTER_MAX_DEPTH,
    FILTER_MAX_MEMBERSHIP_VALUES,
    FILTER_MAX_NODES,
    AggregateFn,
    AggregateMetric,
    AggregateSpec,
    And,
    Comparison,
    Exists,
    F,
    Filter,
    FilterError,
    FilterSyntaxError,
    IsNull,
    ListQuery,
    ListQueryError,
    Membership,
    Not,
    Or,
    Page,
    SortKey,
    field,
    filter_text,
    iter_items,
    iter_pages,
    parse_filter,
    parse_sort,
)

VECTORS_PATH = Path(__file__).resolve().parents[3] / "fixtures" / "torii" / "list_query" / "vectors.json"
VECTORS = json.loads(VECTORS_PATH.read_text(encoding="utf-8"), parse_float=Decimal)


def _ids(cases: list, key: str) -> list:
    return [repr(case[key])[:60] for case in cases]


# ---------------------------------------------------------------------------
# Golden vectors (fixtures/torii/list_query/vectors.json)
# ---------------------------------------------------------------------------


def test_vector_file_version_is_supported() -> None:
    assert VECTORS["version"] == 1


@pytest.mark.parametrize("case", VECTORS["filters"], ids=_ids(VECTORS["filters"], "text"))
def test_filter_vectors_render_canonical_text_and_json(case: dict) -> None:
    from_json = Filter.from_json(case["json"])
    assert str(from_json) == case["canonical"]
    assert from_json.to_json() == case["json"]

    parsed = parse_filter(case["text"])
    assert parsed == from_json
    assert str(parsed) == case["canonical"]
    assert parsed.to_json() == case["json"]
    assert parse_filter(case["canonical"]) == parsed


@pytest.mark.parametrize(
    "case", VECTORS["json_filters"], ids=_ids(VECTORS["json_filters"], "canonical")
)
def test_json_filter_vectors_decode_to_the_normalized_tree(case: dict) -> None:
    decoded = Filter.from_json(case["json"])
    assert str(decoded) == case["canonical"]
    assert decoded.to_json() == case["normalized"]
    assert parse_filter(case["canonical"]) == decoded
    assert Filter.from_json(case["normalized"]) == decoded
    assert ListQuery.from_json({"filter": case["json"]}).to_json() == {"filter": case["normalized"]}


@pytest.mark.parametrize("case", VECTORS["filter_errors"], ids=_ids(VECTORS["filter_errors"], "text"))
def test_filter_error_vectors_report_message_line_and_column(case: dict) -> None:
    with pytest.raises(FilterSyntaxError) as raised:
        parse_filter(case["text"])
    error = raised.value
    assert (error.message, error.line, error.column) == (
        case["message"],
        case["line"],
        case["column"],
    )
    suffix = (
        f"(line {case['line']}, column {case['column']})"
        if "\n" in case["text"]
        else f"(column {case['column']})"
    )
    assert str(error) == f"{case['message']} {suffix}"


@pytest.mark.parametrize("case", VECTORS["sorts"], ids=_ids(VECTORS["sorts"], "text"))
def test_sort_vectors_render_canonical_text_and_json(case: dict) -> None:
    keys = parse_sort(case["text"])
    assert ",".join(str(key) for key in keys) == case["canonical"]
    assert [str(key) for key in keys] == case["json"]
    assert tuple(SortKey.parse(text) for text in case["json"]) == keys


@pytest.mark.parametrize("case", VECTORS["sort_errors"], ids=_ids(VECTORS["sort_errors"], "text"))
def test_sort_error_vectors_report_message_line_and_column(case: dict) -> None:
    with pytest.raises(FilterSyntaxError) as raised:
        parse_sort(case["text"])
    error = raised.value
    assert (error.message, error.line, error.column) == (
        case["message"],
        case["line"],
        case["column"],
    )


@pytest.mark.parametrize("case", VECTORS["queries"], ids=_ids(VECTORS["queries"], "body"))
def test_query_vectors_render_body_and_ordered_get_pairs(case: dict) -> None:
    query = ListQuery.from_json(case["body"])
    assert query.to_json() == case["body"]  # JSON members compare structurally
    pairs = [list(pair) for pair in query.to_query_pairs()]
    assert pairs == case["query_pairs"]
    assert ListQuery.from_query_pairs([tuple(pair) for pair in case["query_pairs"]]) == query


def test_query_vector_builds_from_the_fluent_builder() -> None:
    built = ListQuery(
        filter=(F.owned_by == "alice") & (F.quantity > 1),
        sort=[-F.quantity, F.id],
        select=["id", "quantity"],
        limit=25,
        include_total=True,
    )
    assert built.to_json() == VECTORS["queries"][1]["body"]
    assert [list(pair) for pair in built.to_query_pairs()] == VECTORS["queries"][1]["query_pairs"]


@pytest.mark.parametrize(
    "case", VECTORS["query_body_errors"], ids=_ids(VECTORS["query_body_errors"], "body")
)
def test_query_body_error_vectors_name_the_parameter_and_code(case: dict) -> None:
    with pytest.raises(ListQueryError) as raised:
        ListQuery.from_json(case["body"])
    assert (raised.value.parameter, raised.value.code) == (case["parameter"], case["code"])


@pytest.mark.parametrize(
    "case", VECTORS["query_pair_errors"], ids=_ids(VECTORS["query_pair_errors"], "query_pairs")
)
def test_query_pair_error_vectors_name_the_parameter_and_code(case: dict) -> None:
    with pytest.raises(ListQueryError) as raised:
        ListQuery.from_query_pairs([tuple(pair) for pair in case["query_pairs"]])
    assert (raised.value.parameter, raised.value.code) == (case["parameter"], case["code"])


@pytest.mark.parametrize("case", VECTORS["pages"], ids=_ids(VECTORS["pages"], "json"))
def test_page_vectors_decode_and_reencode(case: dict) -> None:
    page = Page.from_json(case["json"])
    assert page.has_more is case["has_more"]
    assert page.to_json() == case["json"]


# ---------------------------------------------------------------------------
# Builder
# ---------------------------------------------------------------------------


def test_builder_matches_the_parser() -> None:
    built = (
        (F.owned_by == "alice")
        & (F.quantity >= Decimal("10.5"))
        & (F.status.in_("A", "B") | (F.tier < -1))
        & ~F.metadata.frozen.exists()
        & F.note.is_not_null()
    )
    parsed = parse_filter(
        'owned_by = "alice" and quantity >= 10.5 and (status in ["A", "B"] or tier < -1)\n'
        " and not exists(metadata.frozen) and note is not null"
    )
    assert built == parsed
    built.validate()


def test_method_spellings_match_operators() -> None:
    tier = field("tier")
    assert (tier.gt(1) & tier.lt(5)) == ((F.tier > 1) & (F.tier < 5))
    assert str(tier.gt(1) & tier.lt(5)) == "tier > 1 and tier < 5"
    assert tier.eq(1) == (F.tier == 1)
    assert tier.ne(1) == (F.tier != 1)
    assert tier.lte(1) == (F.tier <= 1)
    assert tier.gte(1) == (F.tier >= 1)
    assert F.tier.not_in([1, 2]) == Membership("nin", "tier", [1, 2])
    assert F.tier.in_([1, 2]) == F.tier.in_(1, 2)


def test_and_or_flatten_chains_like_the_rust_builder() -> None:
    a, b, c = (F.a == 1), (F.b == 2), (F.c == 3)
    assert (a & b) & c == And([a, b, c])
    assert a & (b & c) == And([a, b, c])
    assert (a | b) | c == Or([a, b, c])
    assert str((a | b) & c) == "(a = 1 or b = 2) and c = 3"
    assert str(~(a & b)) == "not (a = 1 and b = 2)"
    assert str(And([And([a, b]), c])) == "(a = 1 and b = 2) and c = 3"
    assert Filter.all([]) is None
    assert Filter.all([a, "b = 2", c]) == And([a, b, c])
    assert Filter.any([a, b]) == Or([a, b])


def test_paths_render_backticks_only_where_required() -> None:
    assert str(F.metadata["display-name"] == "x") == 'metadata.`display-name` = "x"'
    assert str(F["and"] == 1) == "`and` = 1"
    assert str(F["metadata.null"] == 1) == "metadata.null = 1"
    assert str(F.alias_binding.bound_at_ms.desc()) == "-alias_binding.bound_at_ms"
    assert (F.metadata["display-name"] == "x").to_json() == {
        "op": "eq",
        "args": ["metadata.display-name", "x"],
    }
    with pytest.raises(ValueError, match="segment"):
        F.metadata["a.b"]


@pytest.mark.parametrize(
    ("value", "literal", "text"),
    [
        (1, 1, "1"),
        (-2, -2, "-2"),
        ((1 << 64) - 1, (1 << 64) - 1, "18446744073709551615"),
        (-(1 << 63), -(1 << 63), "-9223372036854775808"),
        (1 << 64, "18446744073709551616", '"18446744073709551616"'),
        (Decimal("10.5"), "10.5", '"10.5"'),
        (Decimal("25"), 25, "25"),
        (Decimal("1E+2"), 100, "100"),
        (Decimal("10.50"), "10.50", '"10.50"'),
        (True, True, "true"),
        (None, None, "null"),
        ("é", "é", '"é"'),
    ],
)
def test_literals_are_exact(value: object, literal: object, text: str) -> None:
    expr = F.a == value
    assert expr.to_json() == {"op": "eq", "args": ["a", literal]}
    assert str(expr) == f"a = {text}"


@pytest.mark.parametrize("value", [1.5, float("nan"), Decimal("NaN"), Decimal("Infinity"), object()])
def test_inexact_or_unknown_literals_are_rejected(value: object) -> None:
    with pytest.raises(TypeError):
        F.a == value  # noqa: B015


def test_bool_and_number_literals_stay_distinct() -> None:
    assert (F.a == True) != (F.a == 1)  # noqa: E712
    assert F.a.in_(True, 1).values == (True, 1)
    with pytest.raises(FilterError, match="strings, numbers or booleans"):
        F.a.in_(True, 1).validate()
    F.metadata.tags.in_(True, 1).validate()


def test_python_boolean_operators_fail_loudly() -> None:
    with pytest.raises(TypeError, match="truth value"):
        bool(F.a == 1)
    with pytest.raises(TypeError, match="parenthesize"):
        F.a == 1 & F.b  # noqa: B015
    with pytest.raises(TypeError, match="truth value"):
        if F.a:
            pass
    with pytest.raises(TypeError):
        0 < F.a < 5  # noqa: B015


def test_structured_literals_only_for_metadata_fields() -> None:
    expr = F.metadata.tags == ["a", {"k": Decimal("2.5")}]
    assert expr.to_json() == {"op": "eq", "args": ["metadata.tags", ["a", {"k": "2.5"}]]}
    assert str(expr) == 'metadata.tags = ["a",{"k":"2.5"}]'
    expr.validate()
    with pytest.raises(FilterError, match="comparison literals"):
        (F.tags == ["a"]).validate()


@pytest.mark.parametrize(
    "expr",
    [
        F.metadata.tags == ["a"],
        F.metadata.owner != {"kind": "dao"},
        F.metadata.tags.in_(["a"], ["b"]),
        (F.id == "x") & ~((F.owned_by == "y") | (F.metadata.tags == ["a"])),
    ],
)
def test_structured_literals_exist_only_in_the_json_form(expr: Filter) -> None:
    message = (
        "object and array literals have no text form; "
        "they exist only in the JSON form of a POST /query body"
    )
    with pytest.raises(FilterError, match=message) as raised:
        expr.to_text()
    assert raised.value.field and raised.value.field.startswith("metadata.")
    with pytest.raises(FilterError, match=message):
        filter_text(expr)
    with pytest.raises(ListQueryError) as query_error:
        ListQuery(filter=expr).to_query_pairs()
    assert query_error.value.code == "invalid_filter"
    body = ListQuery(filter=expr).to_json()
    assert body == {"filter": expr.to_json()}
    assert ListQuery.from_json(json.loads(json.dumps(body), parse_float=Decimal)).filter == expr


def test_scalar_filters_have_a_checked_text_form() -> None:
    expr = (F.owned_by == "alice") & F.status.in_("active", "paused")
    assert expr.to_text() == str(expr) == filter_text(expr)
    assert parse_filter(expr.to_text()) == expr
    assert filter_text('status = "active"') == 'status = "active"'


def test_membership_requires_ordered_values() -> None:
    with pytest.raises(TypeError, match="ordered"):
        F.a.in_({1, 2})
    with pytest.raises(TypeError, match="list or tuple"):
        Membership("in", "a", "ab")


def test_validation_enforces_structural_limits() -> None:
    deep: Filter = F.a == True  # noqa: E712
    for _ in range(FILTER_MAX_DEPTH + 1):
        deep = Not(deep)
    with pytest.raises(FilterError, match="nesting depth limit of 10"):
        deep.validate()
    with pytest.raises(FilterError, match="node count limit"):
        And([F.a == True] * FILTER_MAX_NODES).validate()  # noqa: E712
    with pytest.raises(FilterError, match="membership list size limit"):
        F.a.in_(list(range(FILTER_MAX_MEMBERSHIP_VALUES + 1))).validate()
    with pytest.raises(FilterError, match="must not be empty"):
        F.a.in_().validate()
    with pytest.raises(FilterError, match="unique"):
        F.a.in_(1, 1).validate()
    with pytest.raises(FilterError, match="range comparisons"):
        (F.a < None).validate()
    with pytest.raises(FilterError, match="whitespace"):
        (F["a b"] == 1).validate()


@pytest.mark.parametrize(
    ("value", "needle"),
    [
        ({"op": "eq", "args": ["a", True], "extra": 1}, "unknown member `extra`"),
        ({"op": "and", "args": []}, "at least one operand"),
        ({"op": "not", "args": []}, "exactly one filter node"),
        ({"op": "in", "args": ["a", []]}, "must not be empty"),
        ({"op": "nin", "args": ["a", [True, True]]}, "must be unique"),
        ({"op": "exists", "args": "a"}, 'takes ["field"]'),
        ({"args": []}, "needs an `op`"),
        (["eq"], "must be an object"),
        ({"op": "eq", "args": ["a b", 1]}, "whitespace"),
        ({"op": "lt", "args": ["a", True]}, "range comparisons"),
        ({"op": "eq", "args": ["a", [1]]}, "comparison literals"),
        (
            {"op": "and", "args": [{"op": "eq", "args": ["a", 1]}, {"op": "between", "args": ["b", 1, 2]}]},
            "unknown operator `between`; expected one of: and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null (at `args[1]`)",
        ),
    ],
)
def test_json_form_rejects_malformed_nodes(value: object, needle: str) -> None:
    with pytest.raises(FilterError) as raised:
        Filter.from_json(value)
    assert needle in str(raised.value)


_FRACTIONAL = (
    'invalid operand for `{field}`: fractional JSON numbers are not exact; '
    'write decimals as strings such as "1.5"'
)


@pytest.mark.parametrize(
    ("text", "field"),
    [
        ('{"op": "gte", "args": ["quantity", 10.5]}', "quantity"),
        ('{"op": "in", "args": ["a", [1, 2.5]]}', "a"),
        ('{"op": "eq", "args": ["metadata.x", {"a": [1, {"b": 1.5}]}]}', "metadata.x"),
        ('{"op": "in", "args": ["metadata.x", [{"b": [0.5]}]]}', "metadata.x"),
    ],
)
def test_json_form_rejects_fractional_numbers_like_torii(text: str, field: str) -> None:
    expected = _FRACTIONAL.format(field=field)
    for parsed in (json.loads(text, parse_float=Decimal), json.loads(text)):
        with pytest.raises(FilterError) as raised:
            Filter.from_json(parsed)
        assert str(raised.value) == expected
        assert raised.value.field == field
    with pytest.raises(ListQueryError) as query_error:
        ListQuery.from_json({"filter": json.loads(text, parse_float=Decimal)})
    assert query_error.value.code == "invalid_filter"
    assert str(query_error.value) == f"invalid `filter`: {expected}"


@pytest.mark.parametrize("op", ["and", "or"])
def test_json_form_single_operand_connectives_decode_to_their_operand(op: str) -> None:
    leaf = {"op": "eq", "args": ["a", 1]}
    decoded = Filter.from_json({"op": op, "args": [leaf]})
    assert decoded == (F.a == 1)
    assert decoded.to_json() == leaf
    assert parse_filter(str(decoded)) == decoded


def test_json_form_collapses_nested_single_operand_connectives() -> None:
    leaf = {"op": "eq", "args": ["a", 1]}
    is_null = {"op": "is_null", "args": ["b"]}
    decoded = Filter.from_json(
        {
            "op": "and",
            "args": [
                {"op": "or", "args": [{"op": "and", "args": [leaf, is_null]}]},
                {"op": "or", "args": [leaf]},
            ],
        }
    )
    assert decoded == And([And([F.a == 1, F.b.is_null()]), F.a == 1])
    assert str(decoded) == "(a = 1 and b is null) and a = 1"
    # The collapsed connective still counts toward the depth limit.
    deep: dict = leaf
    for _ in range(FILTER_MAX_DEPTH + 1):
        deep = {"op": "or", "args": [deep]}
    with pytest.raises(FilterError, match="nesting depth limit of 10"):
        Filter.from_json(deep)


def test_field_paths_must_not_contain_backticks() -> None:
    message = "invalid field `metadata.a`b`: field paths must not contain backticks"
    with pytest.raises(FilterError) as raised:
        (F["metadata.a`b"] == 1).validate()
    assert str(raised.value) == message
    for value in (
        {"op": "eq", "args": ["metadata.a`b", 1]},
        {"op": "in", "args": ["metadata.a`b", [1]]},
        {"op": "exists", "args": ["metadata.a`b"]},
    ):
        with pytest.raises(FilterError) as raised:
            Filter.from_json(value)
        assert str(raised.value) == message
    aggregate = AggregateSpec(metrics=[AggregateMetric("n", "count")], group_by=["a`b"])
    for query, parameter in (
        (ListQuery(select=["id", "a`b"]), "select"),
        (ListQuery(sort=[SortKey("a`b")]), "sort"),
        (ListQuery(aggregate=aggregate), "aggregate"),
    ):
        with pytest.raises(ListQueryError) as query_error:
            query.validate()
        assert query_error.value.parameter == parameter
        assert "must not contain backticks" in query_error.value.message
    # A backtick in the text form always opens or closes a quoted segment.
    assert parse_filter("`a-b`.c = 1") == (F["a-b.c"] == 1)


def test_string_literals_keep_del_and_c1_but_reject_c0_controls() -> None:
    raw = "x\x7fy\x80\x85\x9fz"
    parsed = parse_filter(f'a = "{raw}"')
    assert parsed == (F.a == raw)
    assert str(parsed) == f'a = "{raw}"'
    assert parse_filter(f"a = '{raw}'") == parsed
    for control in ("\x00", "\x01", "\t", "\n", "\x1f"):
        with pytest.raises(FilterSyntaxError) as raised:
            parse_filter(f'a = "x{control}y"')
        assert raised.value.message == "control characters must be escaped inside string literals"
        assert raised.value.column == 7
    # Only `"`, `\` and U+0000..U+001F are escaped, with JSON's short forms.
    value = 'q"b\\s\b\f\n\r\t\x00\x1f\x7f\x85/\''
    rendered = str(F.a == value)
    assert rendered == 'a = "q\\"b\\\\s\\b\\f\\n\\r\\t\\u0000\\u001f\x7f\x85/\'"'
    assert parse_filter(rendered) == (F.a == value)


def test_aggregates_are_bounded_and_their_paths_validated() -> None:
    count = AggregateMetric("n", AggregateFn.COUNT)

    def aggregate(group_by: list, metrics: list) -> ListQuery:
        return ListQuery(aggregate=AggregateSpec(metrics=metrics, group_by=group_by))

    def groups(size: int) -> list:
        return [f"metadata.k{index}" for index in range(size)]

    def metrics(size: int) -> list:
        return [AggregateMetric(f"m{index}", "count") for index in range(size)]

    widest = aggregate(groups(AGGREGATE_MAX_GROUP_BY), metrics(AGGREGATE_MAX_METRICS))
    widest.validate()
    assert ListQuery.from_json(widest.to_json()) == widest
    rejected = [
        (
            aggregate(groups(AGGREGATE_MAX_GROUP_BY + 1), [count]),
            "`group_by` lists at most 8 fields",
        ),
        (
            aggregate([], metrics(AGGREGATE_MAX_METRICS + 1)),
            "`metrics` lists at most 16 metrics",
        ),
        (
            aggregate(["a..b"], [count]),
            "invalid field `a..b`: field path segments must not be empty",
        ),
        (
            aggregate([], [AggregateMetric("s", "sum", "a b")]),
            "invalid field `a b`: field paths must not contain whitespace or control characters",
        ),
    ]
    for query, message in rejected:
        with pytest.raises(ListQueryError) as raised:
            query.validate()
        assert (raised.value.parameter, raised.value.code) == ("aggregate", "invalid_aggregate")
        assert raised.value.message == message
        with pytest.raises(ListQueryError) as decoded:
            ListQuery.from_json(query.to_json())
        assert decoded.value.message == message
    for body, needle in (
        (
            {"groupby": ["a"], "metrics": [{"alias": "n", "fn": "count"}]},
            "unknown aggregate member `groupby`",
        ),
        (
            {"metrics": [{"alias": "n", "fn": "count", "feild": "a"}]},
            "unknown metric member `feild`",
        ),
    ):
        with pytest.raises(ListQueryError) as raised:
            ListQuery.from_json({"aggregate": body})
        assert raised.value.code == "invalid_aggregate"
        assert needle in raised.value.message


def test_json_form_checks_the_field_before_the_operand() -> None:
    with pytest.raises(FilterError, match="invalid field `a b`"):
        Filter.from_json({"op": "eq", "args": ["a b", Decimal("1.5")]})
    with pytest.raises(FilterError, match="invalid field `a b`"):
        Filter.from_json({"op": "in", "args": ["a b", [Decimal("1.5")]]})


def test_json_form_accepts_exact_and_wide_numbers() -> None:
    assert Filter.from_json({"op": "gte", "args": ["quantity", "10.5"]}) == (
        F.quantity >= Decimal("10.5")
    )
    assert Filter.from_json({"op": "lt", "args": ["a", 1 << 70]}).to_json() == {
        "op": "lt",
        "args": ["a", str(1 << 70)],
    }


def test_filters_are_immutable_hashable_copyable_and_picklable() -> None:
    expr = (F.a == {"k": [1]}) | F.b.in_(1, 2) | F.c.is_not_null()
    with pytest.raises(AttributeError):
        expr.operands = ()  # type: ignore[misc]
    assert hash(expr) == hash(copy.deepcopy(expr))
    assert pickle.loads(pickle.dumps(expr)) == expr
    assert pickle.loads(pickle.dumps(F.x.y)).path == "x.y"
    assert repr(F.a == 1) == "Filter('a = 1')"
    assert isinstance(F.a == 1, Comparison)
    assert isinstance(F.a.exists(), Exists) and isinstance(F.a.is_null(), IsNull)


def test_filter_text_passes_raw_text_through() -> None:
    assert filter_text("a == 1") == "a == 1"
    assert filter_text(F.a == 1) == "a = 1"
    with pytest.raises(TypeError):
        filter_text({"op": "eq", "args": ["a", 1]})  # type: ignore[arg-type]


def test_text_errors_point_at_the_problem_with_fix_hints() -> None:
    cases = {
        "a = 1 or": "expected a filter expression",
        "exists(a": "expected `)` to close the `(` at column 7, found the end of the input",
        "a not 1": "expected `in` after `not`",
        "a is not 1": "expected `null` after `is not`",
        "a": "expected an operator after `a` (=, !=, <, <=, >, >=, in, not in, is null)",
        "a ! 1": "use the keyword `not` instead of `!`",
        "a = 1.": "decimal literals need digits after `.`",
        "a = 1x": "a number cannot be followed directly by letters",
        'a = "\\q"': "unknown escape sequence `\\q`",
        'a = "\\ud800"': "unpaired UTF-16 surrogate",
        'a = "\\u12"': "invalid `\\u` escape",
        "`` = 1": "backtick-quoted field names must not be empty",
        "`a.b` = 1": "must not contain `.`",
        "a.`b": "unterminated backtick-quoted field name",
        "(" * 70 + "a = 1" + ")" * 70: "filter nests too deeply",
        "x" * 40_000: "filters must not exceed 32768 bytes",
    }
    for text, needle in cases.items():
        with pytest.raises(FilterSyntaxError) as raised:
            parse_filter(text)
        assert needle in raised.value.message, (text[:40], raised.value.message)


def test_text_strings_decode_escapes_and_single_quotes() -> None:
    assert parse_filter('a = "q\\"\\\\\\né\\ud83d\\ude00"') == (F.a == 'q"\\\né\U0001f600')
    assert parse_filter("a = 'it\\'s'") == (F.a == "it's")


def test_sort_keys_accept_fields_strings_and_reject_legacy_spellings() -> None:
    query = ListQuery(sort=["-quantity", F.id, F.metadata["ui-order"].desc(), +F.name])
    assert [str(key) for key in query.sort] == ["-quantity", "id", "-metadata.`ui-order`", "name"]
    assert ListQuery(sort="-quantity,id").sort == (SortKey("quantity", True), SortKey("id"))
    for legacy in ("id:desc", "id desc", ["id:asc"]):
        with pytest.raises(ListQueryError) as raised:
            ListQuery(sort=legacy)
        assert raised.value.code == "invalid_sort"
    with pytest.raises(FilterSyntaxError, match="at most 8 keys"):
        parse_sort(",".join(f"k{index}" for index in range(9)))


def test_list_query_validation_names_the_control() -> None:
    cases = [
        (ListQuery(limit=0), "limit", "invalid_limit"),
        (ListQuery(limit=1 << 32), "limit", "invalid_limit"),
        (ListQuery(cursor=""), "cursor", "invalid_cursor"),
        (ListQuery(select=()), "select", "invalid_select"),
        (ListQuery(select=["id", "id"]), "select", "invalid_select"),
        (ListQuery(sort=["id", "-id"]), "sort", "invalid_sort"),
        (
            ListQuery(
                select=["id"],
                aggregate=AggregateSpec(metrics=[AggregateMetric("n", AggregateFn.COUNT)]),
            ),
            "select",
            "invalid_select",
        ),
        (ListQuery(aggregate=AggregateSpec(metrics=[])), "aggregate", "invalid_aggregate"),
        (ListQuery(filter=F.a.in_()), "filter", "invalid_filter"),
    ]
    for query, parameter, code in cases:
        with pytest.raises(ListQueryError) as raised:
            query.validate()
        assert (raised.value.parameter, raised.value.code) == (parameter, code), query
    ListQuery(filter="raw text is not parsed client-side ==").validate()


def test_aggregates_serialize_in_post_bodies_only() -> None:
    query = ListQuery(
        filter=F.quantity > 0,
        aggregate=AggregateSpec(
            group_by=["asset"],
            metrics=[
                AggregateMetric("holders", AggregateFn.COUNT),
                AggregateMetric("supply", "sum", F.quantity),
            ],
            having="holders >= 10",
        ),
        sort=["-supply"],
        limit=20,
    )
    assert query.to_json() == {
        "filter": {"op": "gt", "args": ["quantity", 0]},
        "sort": ["-supply"],
        "aggregate": {
            "group_by": ["asset"],
            "metrics": [
                {"alias": "holders", "fn": "count"},
                {"alias": "supply", "fn": "sum", "field": "quantity"},
            ],
            "having": "holders >= 10",
        },
        "limit": 20,
    }
    with pytest.raises(ListQueryError) as raised:
        query.to_query_pairs()
    assert raised.value.code == "invalid_aggregate"
    decoded = ListQuery.from_json(json.loads(json.dumps(query.to_json())))
    assert decoded.aggregate is not None and decoded.aggregate.having == parse_filter("holders >= 10")
    with pytest.raises(ValueError, match="unknown aggregate function `median`"):
        AggregateMetric("m", "median")  # type: ignore[arg-type]


def test_page_helpers_and_iteration_follow_cursors() -> None:
    pages = {
        None: Page(("a", "b"), "c1"),
        "c1": Page(("c",), "c2"),
        "c2": Page((), None, total=3),
    }
    seen: list = []

    def fetch(query: ListQuery) -> Page:
        seen.append(query.cursor)
        return pages[query.cursor]

    base = ListQuery(limit=2)
    assert list(iter_items(fetch, base)) == ["a", "b", "c"]
    assert seen == [None, "c1", "c2"]
    assert [len(page) for page in iter_pages(fetch, base)] == [2, 1, 0]
    assert base.next_page(pages[None]) == ListQuery(limit=2, cursor="c1")
    assert base.next_page(pages["c2"]) is None
    assert list(Page(("x",), None)) == ["x"]
    assert Page.from_json({"items": [1, 2], "next_cursor": None}, item=str).items == ("1", "2")


def test_iteration_is_lazy_and_rejects_a_cursor_that_does_not_advance() -> None:
    calls: list = []

    def looping(query: ListQuery) -> Page:
        calls.append(query.cursor)
        return Page((query.cursor,), "same")

    iterator = iter_items(looping, ListQuery())
    assert next(iterator) is None
    assert calls == [None]
    assert next(iterator) == "same"
    with pytest.raises(RuntimeError, match="answered cursor `same` with the same `next_cursor`"):
        next(iterator)
    assert calls == [None, "same"]


def test_iteration_follows_short_and_empty_pages_until_the_cursor_is_null() -> None:
    # History collections bound the scan behind each page, so a page can be
    # short or empty while `next_cursor` continues; only null ends the read.
    pages = {
        None: Page((), "c1"),
        "c1": Page(("a",), "c2"),
        "c2": Page((), "c3"),
        "c3": Page((), "c1"),
    }
    calls: list = []

    def fetch(query: ListQuery) -> Page:
        calls.append(query.cursor)
        if len(calls) > 5:
            return Page(("z",), None)
        return pages[query.cursor]

    assert list(iter_items(fetch, ListQuery(limit=10))) == ["a", "a", "z"]
    assert calls == [None, "c1", "c2", "c3", "c1", "c2"]


@pytest.mark.parametrize(
    ("payload", "needle"),
    [
        ([], "JSON object"),
        ({"next_cursor": None}, "`items` array"),
        ({"items": [], "next_cursor": 1}, "next_cursor"),
        ({"items": []}, "next_cursor"),
        ({"items": [], "next_cursor": ""}, "next_cursor"),
        ({"items": [], "next_cursor": None, "has_more": False}, "unknown envelope"),
        ({"items": [1], "next_cursor": None, "total": 0}, "current page"),
        ({"items": [], "next_cursor": None, "total": None}, "current page"),
        ({"items": [], "next_cursor": None, "total": -1}, "total"),
        ({"items": [], "next_cursor": None, "total": True}, "total"),
    ],
)
def test_page_decoding_is_strict(payload: object, needle: str) -> None:
    with pytest.raises(ValueError, match=needle):
        Page.from_json(payload)
