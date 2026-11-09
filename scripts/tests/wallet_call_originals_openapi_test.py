"""Check the native Kaigi and Kagemusha wallet-read OpenAPI contracts.

Requires Python 3.11+, the pinned Rust toolchain and pytest/jsonschema dependencies.
Compiles the real std-only route catalog and reads authored artifacts; performs no
network reads, signing, ledger writes, proof verification, or deployment qualification.
"""

import copy
import csv
import json
import random
import re
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from scripts import sdk_operation_inventory

ROOT = Path(__file__).resolve().parents[2]
MIRRORS = (
    ROOT / "artifacts/openapi/torii.json",
    ROOT / "crates/iroha_torii/assets/openapi/torii.json",
    ROOT / "artifacts/openapi/versions/current/torii.json",
)
BASE = "/v1/kagemusha/{scheme}/wallets/{wallet}"
OFFLINE = (
    (BASE + "/loads/{request}", "kagemusha_load_issuance_get", 512),
    (BASE + "/loads/{request}/finality", "kagemusha_load_finality_get", 262144),
    (BASE + "/loads/{request}/epochs/{boundary}", "kagemusha_load_epoch_get", 262144),
    (BASE + "/ledger/{entrypoint}/finality", "kagemusha_ledger_finality_get", 1327104),
    (BASE + "/ledger/{entrypoint}/epochs/{boundary}", "kagemusha_ledger_epoch_get", 262144),
)
CALL = "/v1/kaigi/calls/{call_id}"
READS = ((CALL, "application.kaigi_calls_by_call_id_get"),
         (CALL + "/signals", "application.kaigi_calls_by_call_id_signals_get")) + tuple(
    (path, "contracts." + stable) for path, stable, _ in OFFLINE
)
SECURITY = [
    {"IrohaCanonicalAccount": [], "IrohaCanonicalNonce": [],
     "IrohaCanonicalSignature": [], "IrohaCanonicalTimestampMs": []},
    {"IrohaCanonicalWitness": []},
]


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate member " + key)
        result[key] = value
    return result


@pytest.fixture(scope="module")
def document():
    return json.loads(MIRRORS[0].read_bytes(), object_pairs_hook=reject_duplicates)


def validator(document, name):
    return Draft202012Validator({"$ref": "#/components/schemas/" + name,
                                "components": document["components"]})


def native_fields(name):
    source = (ROOT / "crates/iroha_torii/src/routing.rs").read_text()
    body = source.split("pub struct " + name + " {", 1)[1].split("\n}", 1)[0]
    return dict(re.findall(r"pub (\w+): ([^,]+),", body))


def test_artifact_mirrors_match_without_losing_existing_pool_cut(document):
    assert all(path.read_bytes() == MIRRORS[0].read_bytes() for path in MIRRORS)
    operation = document["paths"]["/v1/privacy/ironwood/pool-cut/{height}"]["post"]
    assert operation["x-iroha-route-auth"]["stableRouteId"] == "privacy.ironwood.pool_cut"
    assert operation["x-iroha-max-request-bytes"] == 4096


def test_real_rust_catalog_compiles_and_projects_the_seven_wallet_reads():
    generated = sdk_operation_inventory.generate(ROOT)
    assert generated == (ROOT / sdk_operation_inventory.INVENTORY).read_bytes()
    routes = {row["route_id"]: row for row in csv.DictReader(
        generated.decode().splitlines()[1:], delimiter="\t")}
    for path, stable in READS:
        route = routes[stable]
        assert route["method"] == "GET" and route["path"] == path
        assert route["sdk"] == route["openapi"] == "true"
        assert route["authentication"] == (
            "torii_default" if path == CALL else "canonical_account_signature")
        assert route["admission"] == (
            "public" if path == CALL else "authenticated_account")


@pytest.mark.parametrize("path,stable", READS)
def test_wallet_reads_are_in_both_catalog_projections(document, path, stable):
    catalog = (ROOT / "crates/iroha_torii_shared/src/route_catalog.rs").read_text()
    line = next(line for line in catalog.splitlines() if '"' + stable + '"' in line)
    assert '"' + path + '"' in line
    assert ".with_projections(RouteProjections::OPENAPI_AND_SDK)" in line
    operation = document["paths"][path]["get"]
    private = path != CALL
    assert operation["x-iroha-route-auth"] == {
        "schemaVersion": 1, "stableRouteId": stable,
        "authentication": "canonical_account_signature" if private else "torii_default",
        "admission": "authenticated_account" if private else "public",
    }
    assert operation["security"] == (SECURITY if private else [{}, {"IrohaApiToken": []}])
    assert operation["x-iroha-tool-effect"] == "read"
    assert "requestBody" not in operation
    if private:
        for response in operation["responses"].values():
            assert response["headers"]["Cache-Control"]["schema"]["const"] == "private, no-store"


@pytest.mark.parametrize("path,stable,bound", OFFLINE)
def test_offline_success_is_only_bounded_original_bytes(document, path, stable, bound):
    operation = document["paths"][path]["get"]
    assert operation["x-iroha-max-response-bytes"] == bound
    content = operation["responses"]["200"]["content"]
    assert content == {"application/x-norito": {"schema": {
        "type": "string", "format": "binary", "minLength": 1, "maxLength": bound,
    }}}
    parameters = operation["parameters"]
    assert not any(parameter["in"] == "query" for parameter in parameters)
    assert {p["name"] for p in parameters if p["in"] == "path"} == set(re.findall(r"\{([^}]+)\}", path))
    accept = next(p for p in parameters if p["name"] == "Accept")
    assert accept["required"] is True
    assert accept["schema"] == {"type": "string", "const": "application/x-norito"}
    assert operation["responses"]["406"]["content"]["text/plain"]["schema"]["const"] == "Accept application/x-norito"


@pytest.mark.parametrize("path,stable,bound", OFFLINE)
def test_original_selectors_reject_noncanonical_and_zero_spelling(document, path, stable, bound):
    for parameter in document["paths"][path]["get"]["parameters"]:
        if parameter["in"] != "path" or parameter["name"] == "boundary":
            continue
        check = Draft202012Validator(parameter["schema"])
        check.validate("ab" * 32)
        for invalid in ("0" * 64, "AB" * 32, "ab" * 31, "ab" * 33,
                        "0x" + "ab" * 32, "ab" * 32 + " ", [171] * 32):
            assert not check.is_valid(invalid)
        if parameter["name"] == "entrypoint":
            assert not check.is_valid("ab" * 31 + "aa")


@pytest.mark.parametrize("path,stable,bound", [read for read in OFFLINE if "{boundary}" in read[0]])
def test_epoch_selector_matches_exact_native_u64_range(document, path, stable, bound):
    parameter = next(p for p in document["paths"][path]["get"]["parameters"] if p["name"] == "boundary")
    check = Draft202012Validator(parameter["schema"])
    values = [0, 1, 2, 9, 10, 10**19 - 1, 10**19, 2**64 - 1, 2**64, 10**20 - 1]
    rng = random.Random(20261108)
    values.extend(rng.randrange(10**20) for _ in range(500))
    for value in values:
        assert check.is_valid(str(value)) is (2 <= value <= 2**64 - 1)
    for invalid in ("", "02", "+2", "-2", "2.0", "2 ", 2, None):
        assert not check.is_valid(invalid)


@pytest.mark.parametrize("schema_name,native_name", (
    ("KaigiCallView", "KaigiCallViewDto"),
    ("KaigiCallSignal", "KaigiCallSignalDto"),
    ("KaigiCallSignalList", "KaigiCallSignalListDto"),
))
def test_kaigi_fields_and_option_omission_match_native_dtos(document, schema_name, native_name):
    schema = document["components"]["schemas"][schema_name]
    Draft202012Validator.check_schema(schema)
    fields = native_fields(native_name)
    assert schema["additionalProperties"] is False
    assert set(schema["properties"]) == set(fields)
    assert set(schema["required"]) == {key for key, ty in fields.items() if not ty.startswith("Option<")}
    for name, ty in fields.items():
        prop = schema["properties"][name]
        if ty in ("u64", "Option<u64>"):
            assert prop["minimum"] == 0 and prop["maximum"] == 2**64 - 1
        if ty in ("u32", "Option<u32>") and name != "max_participants":
            assert prop["minimum"] == 0 and prop["maximum"] == 2**32 - 1


def test_kaigi_call_shape_refuses_nulls_retired_fields_and_number_coercion(document):
    call = {
        "call_id": "universal.universal:demo", "domain": "universal.universal", "call_name": "demo",
        "gas_rate_per_minute": 0, "metadata": {}, "privacy_mode": "transparent", "room_policy": "public",
        "roster_root_hex": "ab" * 32, "commitment_count": 0, "nullifier_count": 0,
        "usage_commitment_count": 0, "status": "active", "created_at_ms": 2**64 - 1,
        "total_duration_ms": 0, "total_billed_gas": 0, "segments_recorded": 0,
    }
    check = validator(document, "KaigiCallView")
    check.validate(call)
    for key, value in (("title", None), ("participant_count", None), ("max_participants", 0),
                       ("max_participants", 4097), ("created_at_ms", "42"), ("created_at_ms", True),
                       ("created_at_ms", 2**64), ("roster_root_hex", "AB" * 32), ("gas_asset_id", "xor")):
        candidate = copy.deepcopy(call)
        candidate[key] = value
        assert not check.is_valid(candidate), (key, value)


def test_signal_page_keeps_anchored_cursor_and_exact_query_controls(document):
    source = (ROOT / "crates/iroha_torii/src/routing.rs").read_text()
    assert native_fields("KaigiCallSignalDto")["entrypoint_hash"] == "String"
    assert "entrypoint_hash: tx.entrypoint_hash().to_string()," in source
    operation = document["paths"][CALL + "/signals"]["get"]
    query = {p["name"]: p for p in operation["parameters"] if p["in"] == "query"}
    assert set(query) == set(native_fields("KaigiCallSignalsParams")) == {"limit", "cursor", "after_timestamp_ms"}
    assert query["limit"]["schema"]["default"] == 50
    assert query["limit"]["schema"]["maximum"] == 10000
    check = validator(document, "KaigiCallSignalList")
    check.validate({"has_more": False, "items": []})
    check.validate({"has_more": True, "items": [], "next_cursor": "AQ"})
    signal = {"entrypoint_hash": "ab" * 32, "call_id": "universal.universal:demo",
              "signal_kind": "signal", "created_at_ms": 0, "metadata": {"nested": [1, None]}}
    check.validate({"has_more": False, "items": [signal]})
    for cursor in (None, "", "AQ=", "AR", " A Q", "A" * 1028):
        assert not check.is_valid({"has_more": True, "items": [], "next_cursor": cursor})
    for invalid in ("hash:" + "AB" * 32 + "#ABCD", "AB" * 32, "ab" * 31 + "aa", "0" * 64):
        assert not validator(document, "KaigiCallSignal").is_valid({**signal, "entrypoint_hash": invalid})
    for field in ("authority", "timestamp_ms", "host_account_id", "participant_account_id"):
        assert not validator(document, "KaigiCallSignal").is_valid({**signal, field: None})
