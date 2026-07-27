"""Canonical first-release privacy capability snapshot tests."""

from __future__ import annotations

import copy
import importlib
import json

import pytest

import iroha_python
from iroha_python.privacy_catalog import (
    PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1,
    PRIVACY_PROTOCOL_IDS_V1,
    PrivacyCapabilitySnapshotError,
    parse_privacy_capability_snapshot_json_v1,
    parse_privacy_capability_snapshot_v1,
)

EXPECTED_PROTOCOL_IDS = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
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


def _snapshot() -> dict[str, object]:
    return {
        "version": 1,
        "committed_height": 42,
        "consensus_policy": {
            "current_limits": {
                "max_actions_per_transaction": 1,
                "max_actions_per_block": 2,
                "max_proof_bytes_per_action": 8 * 1024 * 1024,
                "max_action_bytes": 8 * 1024 * 1024,
                "max_privacy_bytes_per_transaction": 8 * 1024 * 1024,
                "max_privacy_bytes_per_block": 16 * 1024 * 1024,
                "max_statement_and_encrypted_output_bytes_per_transaction": 256
                * 1024,
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


def test_snapshot_parser_defensively_copies_the_closed_shape() -> None:
    source = _snapshot()
    parsed = parse_privacy_capability_snapshot_v1(source)
    assert tuple(
        row["protocol_id"]["protocol"] for row in parsed["protocols"]
    ) == EXPECTED_PROTOCOL_IDS
    source["protocols"][0]["protocol_id"]["protocol"] = "tampered"
    assert (
        parsed["protocols"][0]["protocol_id"]["protocol"]
        == "zk-ace-pq-authorization-v0"
    )


@pytest.mark.parametrize(
    "mutate",
    (
        lambda value: value.update({"unknown": True}),
        lambda value: value.update({"version": 2}),
        lambda value: value["protocols"].pop(),
        lambda value: value["protocols"].reverse(),
        lambda value: value["protocols"][0]["protocol_id"].update(
            {"protocol": "ZK-ACE"}
        ),
        lambda value: value["protocols"][3]["protocol_id"].update(
            {"protocol": "zk-ams-recursive-admission-v0"}
        ),
        lambda value: value["protocols"][5]["protocol_id"].update(
            {"protocol": "zk-x509-onchain-identity-v0"}
        ),
        lambda value: value["protocols"][0].update({"unknown": True}),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"unknown": 1}
        ),
        lambda value: value.update({"committed_height": True}),
        lambda value: value["consensus_policy"]["current_limits"].update(
            {"max_actions_per_transaction": 1.5}
        ),
    ),
)
def test_snapshot_parser_rejects_aliases_unknown_fields_and_malformed_rows(
    mutate,
) -> None:
    hostile = _snapshot()
    mutate(hostile)
    with pytest.raises(PrivacyCapabilitySnapshotError):
        parse_privacy_capability_snapshot_v1(hostile)


def test_snapshot_parser_accepts_only_matching_compiled_and_governed_bindings() -> None:
    valid = _snapshot()
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
    parsed = parse_privacy_capability_snapshot_v1(valid)
    assert parsed["protocols"][1]["compiled_profile"]["status"] == "available"

    hostile = copy.deepcopy(valid)
    hostile["protocols"][1]["activation"]["parameter_digest"] = [9] * 32
    with pytest.raises(
        PrivacyCapabilitySnapshotError, match="does not match compiled profile"
    ):
        parse_privacy_capability_snapshot_v1(hostile)


def test_json_parser_rejects_duplicates_non_finite_numbers_utf8_and_oversize() -> None:
    encoded = json.dumps(_snapshot(), separators=(",", ":")).encode()
    assert parse_privacy_capability_snapshot_json_v1(encoded)["version"] == 1

    with pytest.raises(PrivacyCapabilitySnapshotError, match="duplicate JSON key"):
        parse_privacy_capability_snapshot_json_v1(
            encoded.replace(b'{"version":1', b'{"version":1,"version":1', 1)
        )
    with pytest.raises(PrivacyCapabilitySnapshotError, match="not permitted"):
        parse_privacy_capability_snapshot_json_v1(
            encoded.replace(b'"committed_height":42', b'"committed_height":NaN', 1)
        )
    with pytest.raises(PrivacyCapabilitySnapshotError, match="strict UTF-8"):
        parse_privacy_capability_snapshot_json_v1(b"\xff")
    with pytest.raises(PrivacyCapabilitySnapshotError, match="262144-byte"):
        parse_privacy_capability_snapshot_json_v1(
            b" " * (PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1 + 1)
        )


def test_package_root_exports_only_the_typed_snapshot_contract() -> None:
    for name in (
        "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
        "PRIVACY_PROTOCOL_IDS_V1",
        "PrivacyCapabilitySnapshotError",
        "parse_privacy_capability_snapshot_json_v1",
        "parse_privacy_capability_snapshot_v1",
    ):
        assert hasattr(iroha_python, name)
        assert name in iroha_python.__all__
    for name in RETIRED_EXPORTS:
        assert not hasattr(iroha_python, name)
        assert name not in iroha_python.__all__


def test_retired_free_form_helper_modules_are_deleted() -> None:
    for module in RETIRED_MODULES:
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(f"iroha_python.{module}")
