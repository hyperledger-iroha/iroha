"""Retired JSON privacy-capability inspection tests."""

from __future__ import annotations

import copy
import hashlib
import importlib
import json
from pathlib import Path

import pytest

import iroha_python
from iroha_python.privacy_catalog import (
    LEGACY_PRIVACY_CAPABILITY_INSPECTION_MAX_JSON_BYTES_V1,
    PRIVACY_PROTOCOL_IDS_V1,
    LegacyPrivacyCapabilityInspectionError,
    parse_legacy_privacy_capability_inspection_json_v1,
    parse_legacy_privacy_capability_inspection_v1,
)

MATRIX_PATH = Path(__file__).resolve().parents[3] / "fixtures" / "privacy" / "exact12_v1.tsv"
MATRIX_BYTES = MATRIX_PATH.read_bytes()
MATRIX_TEXT = MATRIX_BYTES.decode("utf-8", errors="strict")
MATRIX_ROWS = tuple(
    tuple(line.split("\t"))
    for line in MATRIX_TEXT.splitlines()
    if line and not line.startswith("#")
)


def _matrix_rows(kind: str) -> tuple[tuple[str, ...], ...]:
    return tuple(row for row in MATRIX_ROWS if row[0] == kind)


PROTOCOL_ROWS = _matrix_rows("protocol")
TYPED_ENVELOPE_ROWS = _matrix_rows("typed-envelope")
RETIRED_PROTOCOL_IDS = tuple(row[1] for row in _matrix_rows("retired"))
EXPECTED_PROTOCOL_IDS = tuple(row[2] for row in PROTOCOL_ROWS)
MATRIX_ROW_KINDS = frozenset(
    {
        "matrix-version",
        "registry-sha256",
        "protocol",
        "typed-envelope",
        "retired",
    }
)

RETIRED_MODULES = (
    "anonymous_pgc",
    "jindo",
    "research_adapters",
    "silent_threshold",
    "sis_hints",
    "vega",
    "verange",
    "zk_ams",
    "zk_x509",
    "zkat",
)

RETIRED_EXPORTS = (
    "get_privacy_algorithm_descriptor",
    "get_privacy_algorithm_descriptors",
    "get_privacy_criteria",
    "privacy_capabilities",
    "build_privacy_proof_envelope",
    "buildPrivacyProofEnvelope",
    "buildZkAtPolicyProofV1",
    "buildZkAtDevProofFixture",
    "buildSilentThresholdCredentialShowingProofV0",
    "buildSisHintsAnonymousCredentialProofV0",
    "buildPenumbraSpendProofV1",
    "buildAztecPrivateKernelProofV1",
    "buildMidenStarkTransactionProofV1",
)


def _tagged(protocol: str) -> dict[str, object]:
    return {"protocol": protocol, "value": None}


def _legacy_inspection_payload() -> dict[str, object]:
    return {
        "version": 1,
        "committed_height": 42,
        "consensus_policy": {
            "current_limits": {
                "max_actions_per_transaction": 1,
                "max_actions_per_block": 2,
                "max_proof_bytes_per_action": 9 * 1024 * 1024,
                "max_action_bytes": 9 * 1024 * 1024,
                "max_privacy_bytes_per_transaction": 9 * 1024 * 1024,
                "max_privacy_bytes_per_block": 18 * 1024 * 1024,
                "max_statement_and_encrypted_output_bytes_per_transaction": 256 * 1024,
                "max_nullifiers_per_action": 8,
                "max_commitments_per_action": 8,
                "retained_root_count": 2048,
            },
            "pending_tightening": None,
        },
        "protocols": [
            {
                "protocol_id": _tagged(protocol),
                "compiled_profile": {
                    "status": "unavailable",
                    "value": {"reason": "engine-unavailable", "detail": None},
                },
                "activation": None,
            }
            for protocol in EXPECTED_PROTOCOL_IDS
        ],
    }


def _profile() -> dict[str, object]:
    return {
        "protocol_id": _tagged("anonymous-pgc-k-out-of-n-v1"),
        "proof_system_id": {
            "proof_system": "anonymous-pgc-p256",
            "value": None,
        },
        "engine_id": {"engine": "native-anonymous-pgc-p256", "value": None},
        "parameter_id": [1] * 32,
        "parameter_digest": [2] * 32,
        "verifier_digest": [3] * 32,
        "statement_schema_digest": [4] * 32,
        "engine_manifest_digest": [5] * 32,
        "protocol_limits": {
            "protocol": "anonymous-pgc-k-out-of-n-v1",
            "limits": {
                "max_anonymity_set_size": 64,
                "max_recipient_count": 8,
            },
        },
    }


def test_protocol_registry_is_exactly_the_canonical_twelve() -> None:
    assert PRIVACY_PROTOCOL_IDS_V1 == EXPECTED_PROTOCOL_IDS
    assert len(PRIVACY_PROTOCOL_IDS_V1) == 12
    assert len(set(PRIVACY_PROTOCOL_IDS_V1)) == 12


def test_shared_exact12_matrix_binds_routes_typed_envelopes_and_retired_ids() -> None:
    assert MATRIX_TEXT.endswith("\n")
    assert "\r" not in MATRIX_TEXT
    assert all(MATRIX_TEXT.split("\n")[:-1])
    assert all(row[0] in MATRIX_ROW_KINDS for row in MATRIX_ROWS)
    assert _matrix_rows("matrix-version") == (("matrix-version", "1"),)
    assert len(PROTOCOL_ROWS) == 12
    assert tuple(row[1] for row in PROTOCOL_ROWS) == tuple(map(str, range(12)))
    assert all(len(row) == 5 for row in PROTOCOL_ROWS)
    registry_preimage = "".join(f"{protocol}\n" for protocol in EXPECTED_PROTOCOL_IDS)
    assert _matrix_rows("registry-sha256") == (
        ("registry-sha256", hashlib.sha256(registry_preimage.encode()).hexdigest()),
    )
    assert len(TYPED_ENVELOPE_ROWS) == 12
    assert tuple(row[1:4] for row in TYPED_ENVELOPE_ROWS) == tuple(
        row[2:5] for row in PROTOCOL_ROWS
    )
    assert all(
        len(row) == 6
        and all(
            len(digest) == 64
            and digest == digest.lower()
            and set(digest) <= set("0123456789abcdef")
            and digest != "0" * 64
            for digest in row[4:]
        )
        for row in TYPED_ENVELOPE_ROWS
    )
    assert len(RETIRED_PROTOCOL_IDS) == len(set(RETIRED_PROTOCOL_IDS))
    assert set(RETIRED_PROTOCOL_IDS).isdisjoint(EXPECTED_PROTOCOL_IDS)


def test_legacy_inspection_parser_defensively_copies_the_closed_shape() -> None:
    source = _legacy_inspection_payload()
    parsed = parse_legacy_privacy_capability_inspection_v1(source)
    assert (
        parsed["consensus_policy"]["current_limits"]["max_proof_bytes_per_action"]
        == 9 * 1024 * 1024
    )
    assert (
        parsed["consensus_policy"]["current_limits"]["max_privacy_bytes_per_block"]
        == 18 * 1024 * 1024
    )
    assert (
        tuple(row["protocol_id"]["protocol"] for row in parsed["protocols"])
        == EXPECTED_PROTOCOL_IDS
    )
    source["protocols"][0]["protocol_id"]["protocol"] = "tampered"
    assert parsed["protocols"][0]["protocol_id"]["protocol"] == "zk-ace-pq-authorization-v0"


@pytest.mark.parametrize(
    "mutate",
    (
        lambda value: value.update({"unknown": True}),
        lambda value: value.update({"version": 2}),
        lambda value: value["protocols"].pop(),
        lambda value: value["protocols"].reverse(),
        lambda value: value["protocols"][0]["protocol_id"].update({"protocol": "ZK-ACE"}),
        lambda value: value["protocols"][3]["protocol_id"].update(
            {"protocol": "zk-ams-recursive-admission-v0"}
        ),
        lambda value: value["protocols"][5]["protocol_id"].update(
            {"protocol": "zk-x509-onchain-identity-v0"}
        ),
        lambda value: value["protocols"][0].update({"unknown": True}),
        lambda value: value["consensus_policy"]["current_limits"].update({"unknown": 1}),
        lambda value: value.update({"committed_height": True}),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_actions_per_transaction": 1.5}
        ),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_proof_bytes_per_action": 9 * 1024 * 1024 + 1}
        ),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_action_bytes": 9 * 1024 * 1024 + 1}
        ),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_privacy_bytes_per_transaction": 9 * 1024 * 1024 + 1}
        ),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_privacy_bytes_per_block": 18 * 1024 * 1024 + 1}
        ),
    ),
)
def test_legacy_inspection_rejects_aliases_unknown_fields_and_malformed_rows(
    mutate,
) -> None:
    hostile = _legacy_inspection_payload()
    mutate(hostile)
    with pytest.raises(LegacyPrivacyCapabilityInspectionError):
        parse_legacy_privacy_capability_inspection_v1(hostile)


@pytest.mark.parametrize("retired_protocol", RETIRED_PROTOCOL_IDS)
def test_every_retired_matrix_protocol_is_rejected(retired_protocol: str) -> None:
    hostile = _legacy_inspection_payload()
    hostile["protocols"][0]["protocol_id"]["protocol"] = retired_protocol
    with pytest.raises(LegacyPrivacyCapabilityInspectionError):
        parse_legacy_privacy_capability_inspection_v1(hostile)


def test_legacy_inspection_accepts_only_matching_compiled_and_governed_bindings() -> None:
    valid = _legacy_inspection_payload()
    profile = _profile()
    valid["protocols"][1]["compiled_profile"] = {
        "status": "available",
        "value": profile,
    }
    valid["protocols"][1]["activation"] = {
        **copy.deepcopy(profile),
        "lifecycle": {
            "state": "active",
            "record": {
                "proposed_at_height": 1,
                "activated_at_height": 2,
                "state_since_height": 2,
            },
        },
        "pending_protocol_limits_tightening": None,
        "assurance": {"assurance": "experimental", "value": None},
    }
    parsed = parse_legacy_privacy_capability_inspection_v1(valid)
    assert parsed["protocols"][1]["compiled_profile"]["status"] == "available"

    hostile = copy.deepcopy(valid)
    hostile["protocols"][1]["activation"]["parameter_digest"] = [9] * 32
    with pytest.raises(
        LegacyPrivacyCapabilityInspectionError, match="does not match compiled profile"
    ):
        parse_legacy_privacy_capability_inspection_v1(hostile)


def test_json_parser_rejects_duplicates_non_finite_numbers_utf8_and_oversize() -> None:
    encoded = json.dumps(_legacy_inspection_payload(), separators=(",", ":")).encode()
    assert parse_legacy_privacy_capability_inspection_json_v1(encoded)["version"] == 1

    with pytest.raises(LegacyPrivacyCapabilityInspectionError, match="duplicate JSON key"):
        parse_legacy_privacy_capability_inspection_json_v1(
            encoded.replace(b'{"version":1', b'{"version":1,"version":1', 1)
        )
    with pytest.raises(LegacyPrivacyCapabilityInspectionError, match="not permitted"):
        parse_legacy_privacy_capability_inspection_json_v1(
            encoded.replace(b'"committed_height":42', b'"committed_height":NaN', 1)
        )
    with pytest.raises(LegacyPrivacyCapabilityInspectionError, match="strict UTF-8"):
        parse_legacy_privacy_capability_inspection_json_v1(b"\xff")
    with pytest.raises(LegacyPrivacyCapabilityInspectionError, match="262144-byte"):
        parse_legacy_privacy_capability_inspection_json_v1(
            b" " * (LEGACY_PRIVACY_CAPABILITY_INSPECTION_MAX_JSON_BYTES_V1 + 1)
        )


def test_package_root_exports_only_explicit_legacy_inspection_names() -> None:
    for name in (
        "LEGACY_PRIVACY_CAPABILITY_INSPECTION_VERSION_V1",
        "PRIVACY_PROTOCOL_IDS_V1",
        "LegacyPrivacyCapabilityInspectionError",
        "parse_legacy_privacy_capability_inspection_json_v1",
        "parse_legacy_privacy_capability_inspection_v1",
    ):
        assert hasattr(iroha_python, name)
        assert name in iroha_python.__all__
    for name in RETIRED_EXPORTS:
        assert not hasattr(iroha_python, name)
        assert name not in iroha_python.__all__
    for ambiguous in (
        "PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1",
        "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
        "PrivacyCapabilityRowV1",
        "PrivacyCapabilitySnapshotError",
        "PrivacyCapabilitySnapshotV1",
        "parse_privacy_capability_snapshot_json_v1",
        "parse_privacy_capability_snapshot_v1",
    ):
        assert not hasattr(iroha_python, ambiguous)
        assert ambiguous not in iroha_python.__all__


def test_retired_free_form_helper_modules_are_deleted() -> None:
    for module in RETIRED_MODULES:
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(f"iroha_python.{module}")
