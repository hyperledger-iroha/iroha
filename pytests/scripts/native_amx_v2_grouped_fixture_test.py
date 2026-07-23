"""OpenAPI parity for the Rust-owned grouped Native AMX v2 fixture."""

from __future__ import annotations

import json
from pathlib import Path
import re
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = (
    REPO_ROOT / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json"
)
OPENAPI_PATHS = (
    REPO_ROOT / "docs" / "portal" / "static" / "openapi" / "torii.json",
    REPO_ROOT
    / "docs"
    / "portal"
    / "static"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)


def _validate_schema(
    value: Any,
    schema: dict[str, Any],
    components: dict[str, Any],
    path: str,
) -> None:
    reference = schema.get("$ref")
    if reference is not None:
        prefix = "#/components/schemas/"
        assert reference.startswith(prefix), f"{path}: unsupported reference {reference}"
        _validate_schema(value, components[reference[len(prefix) :]], components, path)
        return

    alternatives = schema.get("oneOf") or schema.get("anyOf")
    if alternatives is not None:
        failures: list[str] = []
        for alternative in alternatives:
            try:
                _validate_schema(value, alternative, components, path)
                return
            except AssertionError as error:
                failures.append(str(error))
        raise AssertionError(f"{path}: no schema alternative accepted value: {failures}")

    expected_type = schema.get("type")
    if expected_type == "object":
        assert isinstance(value, dict), f"{path}: expected object"
        properties = schema.get("properties", {})
        missing = set(schema.get("required", ())) - value.keys()
        assert not missing, f"{path}: missing required fields {sorted(missing)}"
        if schema.get("additionalProperties") is False:
            unknown = value.keys() - properties.keys()
            assert not unknown, f"{path}: unknown fields {sorted(unknown)}"
        for key, child in value.items():
            if key in properties:
                _validate_schema(child, properties[key], components, f"{path}.{key}")
    elif expected_type == "array":
        assert isinstance(value, list), f"{path}: expected array"
        assert len(value) >= schema.get("minItems", 0), f"{path}: below minItems"
        maximum = schema.get("maxItems")
        assert maximum is None or len(value) <= maximum, f"{path}: above maxItems"
        if schema.get("uniqueItems"):
            canonical = [json.dumps(item, sort_keys=True) for item in value]
            assert len(canonical) == len(set(canonical)), f"{path}: items are not unique"
        item_schema = schema.get("items")
        if item_schema is not None:
            for index, item in enumerate(value):
                _validate_schema(item, item_schema, components, f"{path}[{index}]")
    elif expected_type == "integer":
        assert isinstance(value, int) and not isinstance(value, bool), (
            f"{path}: expected integer"
        )
        assert value >= schema.get("minimum", value), f"{path}: below minimum"
        assert value <= schema.get("maximum", value), f"{path}: above maximum"
    elif expected_type == "boolean":
        assert isinstance(value, bool), f"{path}: expected boolean"
    elif expected_type == "string":
        assert isinstance(value, str), f"{path}: expected string"
        assert len(value) >= schema.get("minLength", 0), f"{path}: below minLength"
        pattern = schema.get("pattern")
        assert pattern is None or re.fullmatch(pattern, value), (
            f"{path}: does not match {pattern}"
        )
    elif expected_type == "null":
        assert value is None, f"{path}: expected null"

    if "enum" in schema:
        assert value in schema["enum"], f"{path}: value is not in enum"


def test_grouped_native_amx_v2_fixture_matches_current_openapi() -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    diagnostics = fixture["golden"]["expected_diagnostics"]
    group = fixture["golden"]["receipt_group"]
    assert diagnostics["lane_settlement_commitments"] == [group]
    assert len(group["native_amx_receipts"]) == 2
    assert fixture["golden"]["ordered_source_ids"] == [
        receipt["source_id"] for receipt in group["native_amx_receipts"]
    ]

    schemas_by_path = []
    for openapi_path in OPENAPI_PATHS:
        openapi = json.loads(openapi_path.read_text(encoding="utf-8"))
        components = openapi["components"]["schemas"]
        _validate_schema(
            diagnostics,
            components["SumeragiDiagnosticsResponse"],
            components,
            "SumeragiDiagnosticsResponse",
        )
        schemas_by_path.append(
            {
                name: components[name]
                for name in (
                    "NativeAmxPhase",
                    "NativeAmxAttestationBody",
                    "NativeAmxAttestationQc",
                    "NativeAmxLegRecord",
                    "NativeAmxParticipantLaneBlockDescriptor",
                    "NativeAmxParticipantLaneBlockProposal",
                    "NativeAmxReceipt",
                    "SumeragiNativeAmxParticipantApplication",
                )
            }
        )

    assert schemas_by_path[0] == schemas_by_path[1]


def test_grouped_native_amx_v2_negative_control_contract_is_bounded() -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    controls = fixture["negative_controls"]
    assert 12 <= len(controls) <= 64
    assert len({control["id"] for control in controls}) == len(controls)
    assert all(control["expectation"] == "reject" for control in controls)
    assert all(1 <= len(control["mutations"]) <= 4 for control in controls)
    assert {
        mutation["op"]
        for control in controls
        for mutation in control["mutations"]
    } <= {"replace", "remove", "copy", "swap", "repeat"}
    assert all(
        mutation["path"].startswith("/golden/receipt_group/")
        for control in controls
        for mutation in control["mutations"]
    )
