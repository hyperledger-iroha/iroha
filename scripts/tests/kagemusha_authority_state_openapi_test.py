"""Authored authority-state contract against native public synthetic fixture JSON.

The fixture is the exact owned JSON emitted by the shared DTO's genuine native
signature/World-root test. It is synthetic data, not installed release authority.
"""

import copy
import json
import re
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
MIRRORS = (
    ROOT / "artifacts/openapi/torii.json",
    ROOT / "crates/iroha_torii/assets/openapi/torii.json",
    ROOT / "artifacts/openapi/versions/current/torii.json",
)
ROUTE = "/v1/kagemusha/authority-state/{asset_definition_id}"
FRAME = "iroha.torii.v1.kagemusha.authority-state.response"
FIXTURE = Path(__file__).parent / "fixtures/kagemusha_authority_state_v1.json"


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate member {key}")
        result[key] = value
    return result


@pytest.fixture(scope="module")
def document():
    return json.loads(MIRRORS[0].read_bytes(), object_pairs_hook=reject_duplicates)


def validator(document, name):
    return Draft202012Validator(
        {
            "$ref": f"#/components/schemas/{name}",
            "components": document["components"],
        }
    )


def test_three_authored_mirrors_are_byte_identical(document):
    assert all(path.read_bytes() == MIRRORS[0].read_bytes() for path in MIRRORS)
    Draft202012Validator.check_schema(document["components"]["schemas"]["KagemushaAuthorityStateV1"])


def test_route_matches_native_admission_and_framing(document):
    operation = document["paths"][ROUTE]["get"]
    source = (ROOT / "crates/iroha_torii/src/kagemusha_state.rs").read_text()
    dto = (ROOT / "crates/iroha_torii_shared/src/kagemusha_state.rs").read_text()
    assert f'const ROUTE: &str = "{ROUTE}"' in source
    assert f'frame = "{FRAME}"' in dto
    assert source.index("validate_api_token(") < source.index("bridge_finality_challenge(")
    for gate in (
        "asset_id.to_string() != asset", "FINALITY_HEAVY_QUERY_RATE_COST",
        "acquire_query_admission", "try_acquire_parts", "AllocationBudget::new",
        "KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1", "height < 2",
        "restart_required()", "status.unanchored", "status.abstaining",
        "status.halted.is_some()", "status.signer.as_ref()",
        "with_native_world_state_snapshot_v1", "proof_response_with_exact_egress",
        "finalize_bridge_finality_attestation_response",
    ):
        assert gate in source
    assert operation["x-iroha-max-response-bytes"] == 128 * 1024 * 1024
    assert operation["security"] == [{}, {"IrohaApiToken": []}]
    assert operation["x-iroha-route-auth"] == {
        "admission": "public", "authentication": "torii_default",
        "schemaVersion": 1, "stableRouteId": "kagemusha.authority_state",
    }
    assert operation["x-iroha-tool-effect"] == "read"
    assert "403" in operation["responses"] and "401" not in operation["responses"]
    errors = (ROOT / "crates/iroha_torii/src/lib.rs").read_text()
    assert "NotPermitted(_) => StatusCode::FORBIDDEN" in errors
    for response in operation["responses"].values():
        assert response["headers"]["Cache-Control"]["schema"]["const"] == "no-store"
        assert response["headers"]["X-Content-Type-Options"]["schema"]["const"] == "nosniff"
    content = operation["responses"]["200"]["content"]
    assert set(content) == {"application/json", "application/x-norito"}
    assert content["application/json"]["schema"] == {"$ref": "#/components/schemas/KagemushaAuthorityStateV1"}
    assert content["application/x-norito"]["schema"]["x-iroha-norito-frame"] == FRAME
    assert content["application/x-norito"]["schema"]["maxLength"] == 128 * 1024 * 1024
    assert "kagemusha_authority_state_unavailable" in operation["responses"]["503"]["description"]


@pytest.mark.parametrize("value", ["0" * 64, "A" * 64, "ab" * 31, "ab" * 33, "ab" * 32 + " ", "0x" + "ab" * 32, "g" * 64])
def test_challenge_rejects_noncanonical_or_zero_values(document, value):
    header = next(p for p in document["paths"][ROUTE]["get"]["parameters"] if p["in"] == "header")
    assert header["required"] and header["x-iroha-header-count"] == 1
    assert not Draft202012Validator(header["schema"]).is_valid(value)
    assert Draft202012Validator(header["schema"]).is_valid("ab" * 32)


def test_response_schema_uses_only_the_exact_native_root_fields(document):
    schema = document["components"]["schemas"]["KagemushaAuthorityStateV1"]
    fields = {"attestation", "world_snapshot", "asset_definition", "asset_incarnation", "verifier_registry"}
    assert set(schema["properties"]) == set(schema["required"]) == fields
    assert schema["additionalProperties"] is False
    assert schema["x-iroha-norito-frame"] == FRAME
    assert schema["properties"]["verifier_registry"] == {"$ref": "#/components/schemas/GovernanceKagemushaGovernedVerifierRegistryV1"}


def test_every_reachable_struct_is_closed_except_native_metadata(document):
    schemas = document["components"]["schemas"]
    seen = set()

    def walk(value):
        if isinstance(value, list):
            for item in value:
                walk(item)
        elif isinstance(value, dict):
            if "$ref" in value:
                name = value["$ref"].rsplit("/", 1)[1]
                if name not in seen:
                    seen.add(name)
                    walk(schemas[name])
            if value.get("type") == "object":
                if "properties" in value:
                    assert value.get("additionalProperties") is False
                else:
                    assert value.get("additionalProperties") == {"$ref": "#/components/schemas/JsonValue"}
            for key, item in value.items():
                if key != "$ref":
                    # JsonValue is the actual native arbitrary metadata payload.
                    if key == "additionalProperties" and item == {"$ref": "#/components/schemas/JsonValue"}:
                        continue
                    walk(item)

    walk(schemas["KagemushaAuthorityStateV1"])
    assert {"SumeragiFinalityAttestation", "WorldStateSnapshotEntryV1", "AssetConfidentialPolicy", "GovernanceKagemushaGovernedVerifierReleaseV1"} <= seen


@pytest.mark.parametrize("kind", ["Table", "Cell"])
def test_world_element_exact_tag_and_explicit_key_absence(document, kind):
    hash_literal = "hash:" + "11" * 32 + "#ABCD"
    value = {"field_id": "world.fixture", "kind": {"kind": kind, "value": None}, "key_hash": hash_literal if kind == "Table" else None, "value_hash": hash_literal}
    check = validator(document, "WorldStateSnapshotEntryV1")
    check.validate(value)
    for field in value:
        missing = copy.deepcopy(value)
        del missing[field]
        assert not check.is_valid(missing)
    changed = copy.deepcopy(value)
    changed["key_hash"] = None if kind == "Table" else hash_literal
    assert not check.is_valid(changed)
    for invalid_kind in ({"kind": kind}, {"kind": kind, "value": 1}, {"kind": "Other", "value": None}, {"kind": kind, "value": None, "unknown": 0}):
        changed = copy.deepcopy(value)
        changed["kind"] = invalid_kind
        assert not check.is_valid(changed)


@pytest.mark.parametrize("scale", [None, 0, 1, 28])
def test_numeric_spec_actual_object_shape(document, scale):
    check = validator(document, "NumericSpec")
    check.validate({"scale": scale})
    for value in ({}, {"scale": scale, "precision": 512}, {"scale": -1}, {"scale": 29}, {"scale": "2"}, scale):
        assert not check.is_valid(value)


@pytest.mark.parametrize("value", ["Infinitely", "Once", "Not", "Limited(1)", "Limited(4294967295)"])
def test_mintable_matches_native_string_codec(document, value):
    check = validator(document, "AssetMintable")
    check.validate(value)
    for malformed in ("Limited(0)", "Limited(01)", "Limited(-1)", "Limited(1.0)", "Limited(4294967296)", "Limited(9999999999)", "Unknown", {"mintable": "Once"}, 1):
        assert not check.is_valid(malformed)


def test_snapshot_bounds_and_asset_field_set_match_current_sources(document):
    schemas = document["components"]["schemas"]
    snapshot = (ROOT / "crates/iroha_data_model/src/sumeragi_finality/world_state.rs").read_text()
    assert "MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1: usize = 131_072" in snapshot
    assert schemas["WorldStateSnapshotV1"]["properties"]["entries"]["maxItems"] == 131_072
    source = (ROOT / "crates/iroha_data_model/src/asset/definition.rs").read_text()
    struct = source.split("pub struct AssetDefinition {", 1)[1].split("\n    }", 1)[0]
    fields = set(re.findall(r"pub ([a-z_]+):", struct))
    schema = schemas["KagemushaAuthorityAssetDefinitionV1"]
    assert set(schema["properties"]) == set(schema["required"]) == fields
    assert schemas["AxtAssetIncarnationV1"]["minItems"] == schemas["AxtAssetIncarnationV1"]["maxItems"] == 1


def test_signed_clock_body_fields_match_current_model_and_uint64_bounds(document):
    source = (ROOT / "crates/iroha_data_model/src/sumeragi_finality.rs").read_text()
    body = source.split("pub struct SumeragiFinalityAttestationBody {", 1)[1].split("\n}", 1)[0]
    fields = set(re.findall(r"(?m)^    pub ([a-z_]+):", body))
    schema = document["components"]["schemas"]["SumeragiFinalityAttestationBody"]
    assert set(schema["properties"]) == set(schema["required"]) == fields
    assert schema["additionalProperties"] is False
    assert "self.observed_at_unix_ms != 0" in source
    clock = schema["properties"]["observed_at_unix_ms"]
    assert clock["type"] == "integer" and clock["format"] == "uint64"
    assert clock["minimum"] == 1 and clock["maximum"] == (1 << 64) - 1
    check = Draft202012Validator(clock)
    for value in (1, 1_000_000, (1 << 64) - 1):
        check.validate(value)
    for value in (0, -1, 1 << 64, "1000000", True, None):
        assert not check.is_valid(value), value


@pytest.fixture(scope="module")
def native_fixture(document):
    # This file is copied byte-for-byte only after the shared DTO native test runs.
    payload = json.loads(FIXTURE.read_bytes(), object_pairs_hook=reject_duplicates)
    validator(document, "KagemushaAuthorityStateV1").validate(payload)
    return payload


def test_actual_native_fixture_is_data_only_and_uses_exact_tuple_codec(native_fixture):
    assert native_fixture["verifier_registry"]["active_release_id"] is None
    assert native_fixture["verifier_registry"]["releases"] == []
    assert isinstance(native_fixture["asset_incarnation"], list)
    assert len(native_fixture["asset_incarnation"]) == 1
    assert native_fixture["attestation"]["body"]["status"]["protocol_version"] == 1
    assert native_fixture["attestation"]["body"]["status"]["applied_height"] == 2
    assert native_fixture["attestation"]["body"]["observed_at_unix_ms"] == 1_000_000


def test_native_fixture_rejects_unknown_nested_authority_and_missing_fields(document, native_fixture):
    check = validator(document, "KagemushaAuthorityStateV1")
    for challenge in ([7] * 32, "0" * 64, "ab" * 32, "AB" * 31, "AB" * 33):
        changed = copy.deepcopy(native_fixture)
        changed["attestation"]["body"]["challenge"] = challenge
        assert not check.is_valid(changed)
    paths = [(), ("attestation",), ("attestation", "body"), ("attestation", "body", "status"), ("world_snapshot",), ("asset_definition",), ("asset_definition", "spec"), ("asset_definition", "confidential_policy"), ("verifier_registry",)]
    for path in paths:
        changed = copy.deepcopy(native_fixture)
        value = changed
        for key in path:
            value = value[key]
        value["trusted_root"] = "caller-selected"
        assert not check.is_valid(changed), path
        for field in value:
            if field == "trusted_root":
                continue
            missing = copy.deepcopy(native_fixture)
            target = missing
            for key in path:
                target = target[key]
            del target[field]
            assert not check.is_valid(missing), (path, field)
