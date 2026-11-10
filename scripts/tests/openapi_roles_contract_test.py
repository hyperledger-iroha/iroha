"""Stored-role query responses keep projections and metrics separate from ledger JSON."""

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
MIRRORS = (
    "artifacts/openapi/torii.json",
    "artifacts/openapi/versions/current/torii.json",
    "crates/iroha_torii/assets/openapi/torii.json",
)


@pytest.fixture(params=MIRRORS)
def document(request):
    return json.loads((ROOT / request.param).read_text())


def validator(document, name):
    schema = {"$ref": f"#/components/schemas/{name}", "components": document["components"]}
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_role_ledger_shape_remains_closed_while_query_rows_admit_projection(document):
    ledger = validator(document, "NativeRoleV1")
    role_page = validator(document, "RolePage")
    full = {"id": "role_0", "permissions": [{"name": "fixture_permission", "payload": {"value": 1}}]}
    ledger.validate(full)
    for row in (full, {"id": full["id"]}, {"permissions": full["permissions"]}):
        role_page.validate({"items": [row], "next_cursor": None})
    for row in ({"id": full["id"]}, {"permissions": []}, {**full, "permission_epochs": []}):
        assert not ledger.is_valid(row)
    for row in ({}, {"permissions": "omitted"}, {"id": "role_0", "unknown": []}):
        assert not role_page.is_valid({"items": [row], "next_cursor": None})


def test_aggregate_rows_admit_only_actual_metric_types_and_optional_group_id(document):
    for name in ("RoleAggregateRow", "AccountRoleAggregateRow"):
        schema = validator(document, name)
        for row in ({"n": 7, "id": 7}, {"id": "role_3", "n": 1}, {"n": 0}):
            schema.validate(row)
        for row in ({}, {"id": "role_3"}, {"n": -1}, {"n": True}, {"n": 1.5}, {"n": "7"}, {"n": []}, {"n": {}}, {"n": 2**64}):
            assert not schema.is_valid(row), (name, row)
    validator(document, "RoleAggregateRow").validate({"sets": 1, "sum": None, "min": None, "max": None, "avg": None})
    assert not validator(document, "AccountRoleAggregateRow").is_valid({"sum": None})


def test_aggregate_aliases_and_metric_count_are_bounded(document):
    for name in ("RoleAggregateRow", "AccountRoleAggregateRow"):
        schema = validator(document, name)
        sixteen = {f"metric_{i}": i for i in range(16)}
        schema.validate(sixteen)
        schema.validate({"id": "role_3", **sixteen})
        for row in ({**sixteen, "overflow": 17}, {"id": "role_3", **sixteen, "overflow": 17}):
            assert not schema.is_valid(row)
        for alias in ("", "0count", "n.value", "n-value", "n\n", "n\r", "é"):
            assert not schema.is_valid({alias: 1}), (name, alias)
        schema.validate({"_n9": 1})


def test_role_page_envelopes_preserve_group_totals_and_empty_continuations(document):
    for name in ("RolePage", "AccountRolePage"):
        schema = validator(document, name)
        for page in (
            {"items": [{"n": 7}], "next_cursor": None, "total": 1},
            {"items": [{"id": "role_2", "n": 1}], "next_cursor": "opaque", "total": 2},
            {"items": [], "next_cursor": None, "total": 0},
            {"items": [], "next_cursor": "opaque"},
        ):
            schema.validate(page)
        for page in ({"items": []}, {"items": [], "next_cursor": None, "total": -1}, {"items": [], "next_cursor": None, "offset": 1}):
            assert not schema.is_valid(page)
        total = document["components"]["schemas"][name]["properties"]["total"]["description"]
        assert "group count after having" in total


def test_role_route_examples_only_use_exposed_fields(document):
    for base in ("/v1/roles", "/v1/accounts/{account_id}/roles"):
        operation = document["paths"][base]["get"]
        params = {parameter["name"]: parameter for parameter in operation["parameters"]}
        assert 'id in ["auditor", "operator"]' in params["filter"]["description"]
        assert "-id" in params["sort"]["description"]
        assert "quantity" not in json.dumps(params)
        post = document["paths"][base + "/query"]["post"]
        assert "having precedes pagination" in post["description"]
        assert "cannot be combined with select" in post["description"]
        if "account_id" in base:
            assert "only assignments of that canonical account" in post["description"]
