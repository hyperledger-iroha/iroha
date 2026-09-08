"""Shared nominal error, Unit, and cursor schema fixtures for the Python SDK."""

from copy import deepcopy
import json
from pathlib import Path

import pytest

from iroha_python import (
    ContractErrorTypeDescriptor,
    ContractManifest,
    EntrypointValueTypeV1,
)


FIXTURE = Path(__file__).resolve().parents[3] / "fixtures/kotodama/nominal_errors_v1.json"


def payload():
    return json.loads(FIXTURE.read_text(encoding="utf-8"))["manifest"]


def test_shared_nominal_error_unit_cursor_and_page_schemas():
    value = payload()
    manifest = ContractManifest.from_payload(value)
    assert manifest.error_types[0].identity == "example/vault@1.0.0::金庫::拒否"
    assert manifest.error_types[0].variants[0].name == "不足"
    assert manifest.error_types[0].variants[0].code == manifest.error_types[1].variants[0].code
    assert [entry.return_schema.canonical_type_name for entry in manifest.entrypoints] == [
        entry["return_type"] for entry in value["entrypoints"]
    ]
    assert [entry.return_schema.word_count for entry in manifest.entrypoints] == [1, 1, 2]


@pytest.mark.parametrize("mutation", [
    lambda value: value.pop("error_types"),
    lambda value: value["error_types"].append(deepcopy(value["error_types"][0])),
    lambda value: value["entrypoints"][0]["return_schema"]["nodes"][2]["value"]["variants"][0].update(name="Other"),
    lambda value: value["entrypoints"][0]["return_schema"]["nodes"][2]["value"]["variants"][0].update(code=3),
])
def test_boundary_nominal_errors_require_the_exact_catalog(mutation):
    value = payload()
    mutation(value)
    with pytest.raises(TypeError):
        ContractManifest.from_payload(value)


@pytest.mark.parametrize("identity", ["", "bad identity", "bad<identity>", "x" * 1025, "__kotodama_link_hidden", "\ud800"])
def test_error_identity_rejects_noncanonical_paths(identity):
    value = payload()["error_types"][0]
    value["identity"] = identity
    with pytest.raises(TypeError):
        ContractErrorTypeDescriptor.from_payload(value)


@pytest.mark.parametrize("code", [0, True, -1, 0x1_0000_0000, "1"])
def test_error_discriminant_is_a_nonzero_u32(code):
    value = payload()["error_types"][0]
    value["variants"][0]["code"] = code
    with pytest.raises(TypeError):
        ContractErrorTypeDescriptor.from_payload(value)


@pytest.mark.parametrize("mutation", [
    lambda value: value["variants"].reverse(),
    lambda value: value["variants"][1].update(code=1),
    lambda value: value["variants"][1].update(name="不足"),
    lambda value: value.update(namespace="OldError"),
    lambda value: value["variants"][0].update(namespace="OldError"),
])
def test_error_variant_schema_is_exact_and_ordered(mutation):
    value = payload()["error_types"][0]
    mutation(value)
    with pytest.raises(TypeError):
        ContractErrorTypeDescriptor.from_payload(value)


@pytest.mark.parametrize("kind,value", [("Unit", 0), ("StateCursor", {"kind": "Json", "value": None})])
def test_unit_and_cursor_payload_kinds_are_exact(kind, value):
    with pytest.raises(TypeError):
        EntrypointValueTypeV1.from_payload({"nodes": [{"kind": kind, "value": value}]})


def test_state_page_cannot_forge_its_key_type():
    schema = payload()["entrypoints"][2]["return_schema"]
    schema["nodes"][-1]["value"]["kind"] = "Bool"
    with pytest.raises(TypeError, match="forged"):
        EntrypointValueTypeV1.from_payload(schema)


@pytest.mark.parametrize("type_name", [
    "missing/package@1::Vault::Failure",
    "Result<(), missing/package@1::Vault::Failure>",
    "StateMap<int, List<Option<missing/package@1::Vault::Failure>, 8>>",
    "Record{status: missing/package@1::Vault::Failure}",
])
def test_state_only_nominal_errors_require_catalog_membership(type_name):
    value = payload()
    value["entrypoints"] = []
    ContractManifest.from_payload(value)
    value["states"] = [{"name": "status", "type_name": type_name}]
    with pytest.raises(TypeError, match="error_types catalog"):
        ContractManifest.from_payload(value)


def test_state_only_nominal_errors_cannot_omit_the_catalog():
    value = payload()
    value["entrypoints"] = []
    value.pop("error_types")
    with pytest.raises(TypeError, match="error_types catalog"):
        ContractManifest.from_payload(value)


@pytest.mark.parametrize("type_name", [
    "StatePage{anything: int}",
    "StatePage{items: List<(int, bool), 8>, next: Option<StateCursor<bool>>}",
    "StatePage{items: List<(Json, bool), 8>, next: Option<StateCursor<Json>>}",
])
def test_state_page_reserved_shape_cannot_be_forged(type_name):
    value = payload()
    value["states"][2]["type_name"] = type_name
    with pytest.raises(TypeError):
        ContractManifest.from_payload(value)


def test_public_unit_requires_an_explicit_exact_return_descriptor():
    value = payload()
    entry = value["entrypoints"][0]
    entry["return_type"] = "()"
    entry["return_schema"] = {"nodes": [{"kind": "Unit", "value": None}]}
    parsed = ContractManifest.from_payload(value)
    assert parsed.entrypoints[0].return_schema.word_count == 1
    for fields in [("return_type",), ("return_schema",), ("return_type", "return_schema")]:
        for omitted in [False, True]:
            invalid = deepcopy(value)
            for field in fields:
                if omitted:
                    invalid["entrypoints"][0].pop(field)
                else:
                    invalid["entrypoints"][0][field] = None
            with pytest.raises(TypeError, match="canonical exact V1 interface"):
                ContractManifest.from_payload(invalid)


def test_exported_structs_retain_locked_identity_in_public_and_durable_schemas():
    fixture = FIXTURE.with_name("exported_structs_v1.json").read_text(encoding="utf-8")
    vectors = json.loads(FIXTURE.with_name("exported_struct_names_v1.json").read_text(encoding="utf-8"))
    original = "std/math@1.0.0::Math::Receipt"
    for name in vectors["valid"]:
        manifest = ContractManifest.from_payload(json.loads(fixture.replace(original, name))["manifest"])
        entrypoint = manifest.entrypoints[0]
        assert entrypoint.return_schema.canonical_type_name == f"struct {name}"
        assert entrypoint.argument_schema.fields[0].type.word_count == 3
    for name in vectors["invalid"]:
        with pytest.raises(TypeError):
            ContractManifest.from_payload(json.loads(fixture.replace(original, name))["manifest"])
    value = json.loads(fixture)["manifest"]
    value["entrypoints"] = []
    value["error_types"] = []
    with pytest.raises(TypeError, match="error_types catalog"):
        ContractManifest.from_payload(value)
