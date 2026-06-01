from __future__ import annotations

import json

from iroha_python import (
    ToriiClient,
    get_privacy_algorithm_descriptor,
    get_privacy_algorithm_descriptors,
    get_privacy_criteria,
    privacy_capabilities,
)


def test_privacy_catalog_exposes_boi_compatible_descriptors() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    by_id = {descriptor["id"]: descriptor for descriptor in descriptors}

    assert len(descriptors) == 21
    assert get_privacy_criteria() == [
        "hide_amount",
        "hide_sender",
        "hide_receiver",
        "hide_asset_type",
        "post_quantum",
    ]

    shield = by_id["shield"]
    assert shield["category"] == "payment"
    assert shield["maturity"] == "specification"
    assert shield["hidden_features"] == ["hide_receiver"]
    assert shield["covered_criteria"] == ["hide_receiver"]
    assert shield["requirements"] == ["zk::RegisterZkAsset", "zk::Shield"]
    assert shield["status"] == "cataloged"
    assert shield["unavailable_reason"] is None
    assert shield["sdk_entrypoints"] == [
        "buildShieldInstruction",
        "buildTransaction",
        "submitSignedTransaction",
    ]
    assert shield["verifier_key_metadata"] == {
        "verifier_key_id": "zk::Shield",
        "proof_family": "commitment-only",
        "public_inputs_schema": "asset,from,amount,note_commitment",
        "pq_layers": {
            "proof": False,
            "authorization": False,
            "note_encryption": False,
        },
    }


def test_privacy_catalog_returns_defensive_copies() -> None:
    descriptor = get_privacy_algorithm_descriptor("pq-masp-stark-v0")
    assert descriptor is not None
    descriptor["covered_criteria"].clear()
    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = False

    fresh = get_privacy_algorithm_descriptor("pq-masp-stark-v0")
    assert fresh is not None
    assert "post_quantum" in fresh["covered_criteria"]
    assert fresh["verifier_key_metadata"]["pq_layers"]["proof"] is True
    assert get_privacy_algorithm_descriptor("unknown") is None
    assert get_privacy_algorithm_descriptor("../../shield") is None
    assert get_privacy_algorithm_descriptor(None) is None  # type: ignore[arg-type]


def test_privacy_catalog_descriptors_are_json_safe_and_boi_stable() -> None:
    required_fields = {
        "id",
        "name",
        "category",
        "maturity",
        "hidden_features",
        "covered_criteria",
        "requirements",
        "limitations",
        "status",
        "unavailable_reason",
        "sdk_entrypoints",
        "planned_sdk_entrypoints",
        "verifier_key_metadata",
    }

    for descriptor in get_privacy_algorithm_descriptors():
        assert required_fields <= descriptor.keys()
        json.dumps(descriptor)
        assert descriptor["hidden_features"] == descriptor["covered_criteria"]
        assert descriptor["requirements"] == descriptor["chain_requirements"]
        assert descriptor["limitations"] == [
            *descriptor["security_notes"],
            *descriptor["failure_modes"],
        ]
        assert descriptor["status"] == "cataloged"
        assert descriptor["unavailable_reason"] is None
        verifier_key_metadata = descriptor["verifier_key_metadata"]
        assert verifier_key_metadata == {
            "verifier_key_id": descriptor["verifier_key_id"],
            "proof_family": descriptor["proof_family"],
            "public_inputs_schema": descriptor["public_inputs_schema"],
            "pq_layers": descriptor["pq_layers"],
        }
        assert set(verifier_key_metadata["pq_layers"]) == {
            "proof",
            "authorization",
            "note_encryption",
        }


def test_privacy_catalog_enforces_execution_and_metadata_invariants() -> None:
    allowed_maturities = {
        "peer_reviewed",
        "accepted_conference",
        "technical_report",
        "arxiv_preprint",
        "specification",
    }
    descriptors = get_privacy_algorithm_descriptors()
    by_id = {descriptor["id"]: descriptor for descriptor in descriptors}

    assert len(by_id) == len(descriptors)
    assert by_id["jindo-lattice-pcs-zk-v0"]["maturity"] == "technical_report"

    zk_ace = by_id["zk-ace-pq-authorization-v0"]
    assert zk_ace["implementation_stage"] == "chain-executable"
    assert zk_ace["sdk_entrypoints"] == [
        "buildRegisterZkAceIdentityCommitmentInstruction",
        "buildRotateZkAceIdentityCommitmentInstruction",
        "buildRevokeZkAceIdentityCommitmentInstruction",
        "buildZkAceAuthorizedTransferInstruction",
        "buildZkAceAuthorizationProofV1",
    ]
    assert "buildZkAceAuthorizationProofV0" not in zk_ace["planned_sdk_entrypoints"]
    assert zk_ace["verifier_key_metadata"]["pq_layers"] == {
        "proof": True,
        "authorization": True,
        "note_encryption": False,
    }
    assert "post_quantum" not in zk_ace["covered_criteria"]

    for descriptor in descriptors:
        assert descriptor["maturity"] in allowed_maturities
        if descriptor["implementation_stage"] == "catalog-as-of-2026-05":
            assert descriptor["sdk_entrypoints"] == []
            assert descriptor["planned_sdk_entrypoints"]
        pq_layers = descriptor["verifier_key_metadata"]["pq_layers"]
        fully_pq = (
            pq_layers["proof"]
            and pq_layers["authorization"]
            and pq_layers["note_encryption"]
        )
        assert ("post_quantum" in descriptor["covered_criteria"]) is fully_pq


def test_privacy_capabilities_uses_client_entrypoints() -> None:
    client = ToriiClient("http://torii.example", max_retries=0)

    capabilities = client.privacy_capabilities()

    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is False
    assert capabilities["transfer_asset_instruction"] is True
    assert capabilities["shield_instruction"] is True
    assert capabilities["zk_transfer_instruction"] is True
    assert capabilities["unshield_instruction"] is True
    assert capabilities["zk_ace_register_identity_instruction"] is True
    assert capabilities["zk_ace_rotate_identity_instruction"] is True
    assert capabilities["zk_ace_revoke_identity_instruction"] is True
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True
    assert capabilities["zk_ace_native_air_prover_v1"] is True
    assert capabilities["zk_ace_validator_support_v1"] is True
    assert capabilities["zk_ace_sdk_exports_v1"] is True
    assert capabilities["asset_hidden_transfer_instruction"] is False
    assert capabilities["ml_kem_note_encryption"] is False
    assert capabilities["privacy_algorithms"][0]["id"] == "transparent-transfer"


def test_module_privacy_capabilities_defaults_to_static_sdk_surface() -> None:
    capabilities = privacy_capabilities()

    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is False
    assert capabilities["transfer_asset_instruction"] is True
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True
    assert capabilities["zk_ace_native_air_prover_v1"] is True
    assert capabilities["zk_ace_validator_support_v1"] is True
    assert capabilities["zk_ace_sdk_exports_v1"] is True
    assert capabilities["privacy_criteria"] == get_privacy_criteria()


def test_privacy_capabilities_returns_defensive_copies() -> None:
    capabilities = privacy_capabilities()
    capabilities["privacy_algorithms"][0]["id"] = "tampered"
    capabilities["privacy_algorithms"][0]["verifier_key_metadata"]["pq_layers"][
        "proof"
    ] = "tampered"
    capabilities["privacy_criteria"].append("tampered")

    fresh = privacy_capabilities()

    assert fresh["privacy_algorithms"][0]["id"] == "transparent-transfer"
    assert fresh["privacy_algorithms"][0]["verifier_key_metadata"]["pq_layers"][
        "proof"
    ] is False
    assert fresh["privacy_criteria"] == get_privacy_criteria()


def test_torii_client_privacy_descriptors_return_defensive_copies() -> None:
    client = ToriiClient("http://torii.example", max_retries=0)
    descriptors = client.privacy_algorithm_descriptors()
    descriptors[0]["id"] = "tampered"

    assert client.privacy_algorithm_descriptors()[0]["id"] == "transparent-transfer"


def test_privacy_capabilities_tolerates_hostile_client_attribute_access() -> None:
    class HostileClient:
        def __getattribute__(self, name: str):
            if name.endswith("_and_wait"):
                raise RuntimeError("attribute trap")
            return super().__getattribute__(name)

    capabilities = privacy_capabilities(HostileClient())

    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is False
    assert capabilities["transfer_asset_instruction"] is False
    assert capabilities["shield_instruction"] is False
    assert capabilities["zk_transfer_instruction"] is False
    assert capabilities["unshield_instruction"] is False
    assert capabilities["zk_ace_authorized_transfer_instruction"] is False


def test_privacy_capabilities_does_not_treat_non_callable_client_fields_as_support() -> None:
    class ShadowedClient:
        transfer_asset_and_wait = None
        shield_asset_and_wait = "present but not callable"
        zk_transfer_prepared_and_wait = 1
        unshield_prepared_and_wait = False

    capabilities = privacy_capabilities(ShadowedClient())

    assert capabilities["transfer_asset_instruction"] is False
    assert capabilities["shield_instruction"] is False
    assert capabilities["zk_transfer_instruction"] is False
    assert capabilities["unshield_instruction"] is False
