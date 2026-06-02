from __future__ import annotations

import json

import pytest

from iroha_python import (
    ToriiClient,
    get_privacy_algorithm_descriptor,
    get_privacy_algorithm_descriptors,
    get_privacy_criteria,
    privacy_capabilities,
)
from iroha_python import privacy_catalog


def _raw_descriptor(**patch: object) -> dict[str, object]:
    descriptor: dict[str, object] = {
        "id": "shape-check",
        "name": "Shape check",
        "category": "payment",
        "maturity": "specification",
        "coveredCriteria": [],
        "chainRequirements": [],
        "securityNotes": [],
        "failureModes": [],
        "sdkEntrypoints": [],
        "plannedSdkEntrypoints": [],
        "proofFamily": "none",
        "publicInputsSchema": None,
        "verifierKeyId": None,
        "pqLayers": {
            "proof": False,
            "authorization": False,
            "noteEncryption": False,
        },
    }
    descriptor.update(patch)
    return descriptor


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


def test_privacy_catalog_loader_rejects_non_list_payload(monkeypatch) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps({"id": "not-a-list"}),
    )

    with pytest.raises(RuntimeError, match="must decode to a list"):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_non_object_entries(monkeypatch) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="valid"), ["not", "an", "object"]]),
    )

    with pytest.raises(RuntimeError, match="entry 1 must decode to an object"):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize("bad_id", ["", "   ", None, 7])
def test_privacy_catalog_loader_rejects_missing_or_invalid_ids(
    monkeypatch,
    bad_id,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([{"id": bad_id}]),
    )

    with pytest.raises(RuntimeError, match="non-empty id"):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_duplicate_ids(monkeypatch) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="shield"), _raw_descriptor(id="shield")]),
    )

    with pytest.raises(RuntimeError, match="duplicate id 'shield'"):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize("bad_id", ["Shield", "shield/../../admin", "shield.v1"])
def test_privacy_catalog_loader_rejects_unsafe_ids(monkeypatch, bad_id) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id=bad_id)]),
    )

    with pytest.raises(RuntimeError, match="lowercase and URL-safe"):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("name", "", "field 'name' must be a non-empty string"),
        ("category", None, "field 'category' must be a non-empty string"),
        ("maturity", 7, "field 'maturity' must be a non-empty string"),
        ("proofFamily", "__missing__", "field 'proof_family' is required"),
        ("coveredCriteria", "hide_receiver", "field 'covered_criteria' must be a list"),
        ("coveredCriteria", "__missing__", "field 'covered_criteria' is required"),
        ("chainRequirements", {"bad": "shape"}, "field 'chain_requirements' must be a list"),
        ("sdkEntrypoints", [7], "field 'sdk_entrypoints' item 0 must be a string"),
        (
            "sourceReferences",
            [{"label": "bad", "url": 7}],
            "source_references' item 0 must include string label and url",
        ),
        ("pqLayers", ["bad"], "field 'pq_layers' must be an object"),
        (
            "pqLayers",
            {"proof": True, "authorization": False},
            "field 'pq_layers.note_encryption' is required",
        ),
        ("pqLayers", {"proof": "yes"}, "field 'pq_layers.proof' must be a boolean"),
    ],
)
def test_privacy_catalog_loader_rejects_malformed_metadata_shapes(
    monkeypatch,
    field,
    value,
    message,
) -> None:
    descriptor = _raw_descriptor()
    if value == "__missing__":
        descriptor.pop(field)
    else:
        descriptor[field] = value
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(RuntimeError, match=message):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_does_not_alias_pq_layer_metadata(monkeypatch) -> None:
    raw_pq_layers = {
        "proof": True,
        "authorization": False,
        "noteEncryption": True,
    }
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="alias-check", pqLayers=raw_pq_layers)]),
    )

    [descriptor] = privacy_catalog._load_descriptors()

    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = False
    assert descriptor["pq_layers"]["proof"] is True


def test_privacy_catalog_returns_defensive_copies() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    descriptors[0]["id"] = "tampered"
    descriptors[0]["verifier_key_metadata"]["pq_layers"]["proof"] = "tampered"

    fresh_descriptors = get_privacy_algorithm_descriptors()
    assert fresh_descriptors[0]["id"] == "transparent-transfer"
    assert fresh_descriptors[0]["verifier_key_metadata"]["pq_layers"]["proof"] is False

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
    assert get_privacy_algorithm_descriptor(b"shield") is None  # type: ignore[arg-type]


def test_privacy_catalog_descriptor_lookup_does_not_stringify_hostile_ids() -> None:
    class HostileId:
        def __str__(self) -> str:
            raise AssertionError("hostile __str__ should not be called")

    assert get_privacy_algorithm_descriptor(HostileId()) is None  # type: ignore[arg-type]


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

    anonymous_pgc = by_id["anonymous-pgc-k-out-of-n-v1"]
    assert anonymous_pgc["implementation_stage"] == "sdk-builder"
    assert anonymous_pgc["sdk_entrypoints"] == [
        "buildAnonymousPgcReceiverSet",
        "buildAnonymousPgcDevProofFixture",
        "verifyAnonymousPgcDevProofLocally",
    ]
    assert anonymous_pgc["planned_sdk_entrypoints"] == [
        "buildAnonymousPgcAccountCommitmentInstruction",
        "buildAnonymousPgcKOutOfNProofV1",
        "buildAnonymousPgcTransferInstruction",
    ]

    zkat = by_id["zkat-policy-private-auth-v1"]
    assert zkat["implementation_stage"] == "sdk-builder"
    assert zkat["sdk_entrypoints"] == [
        "buildZkAtPolicyCommitment",
        "buildZkAtAuthenticatorEnvelope",
        "buildZkAtDevProofFixture",
        "verifyZkAtAuthenticatorLocally",
    ]
    assert zkat["planned_sdk_entrypoints"] == [
        "buildZkAtPolicyCommitmentInstruction",
        "buildZkAtPolicyProofV1",
        "buildZkAtAuthorizedTransaction",
    ]

    zk_ams = by_id["zk-ams-recursive-admission-v0"]
    assert zk_ams["implementation_stage"] == "sdk-builder"
    assert zk_ams["public_inputs_schema"] == (
        "issuer_root,admission_batch_root,admission_nullifiers,"
        "anonymous_account_commitments,recursive_proof_digest,domain_separator"
    )
    assert zk_ams["sdk_entrypoints"] == [
        "buildZkAmsAdmissionBatch",
        "buildZkAmsAdmissionProofEnvelope",
        "buildZkAmsAdmissionDevProofFixture",
        "verifyZkAmsAdmissionProofLocally",
    ]
    assert zk_ams["planned_sdk_entrypoints"] == [
        "buildZkAmsAdmissionBatchProofV0",
        "buildSubmitZkAmsAdmissionBatchInstruction",
    ]

    vega = by_id["vega-existing-credential-zk-v0"]
    assert vega["implementation_stage"] == "sdk-builder"
    assert vega["sdk_entrypoints"] == [
        "buildVegaCredentialPredicateCommitment",
        "buildVegaCredentialProofEnvelope",
        "buildVegaCredentialDevProofFixture",
        "verifyVegaCredentialProofLocally",
    ]
    assert vega["planned_sdk_entrypoints"] == [
        "buildVegaCredentialPredicateProofV0",
        "buildSubmitVegaCredentialProofInstruction",
    ]

    silent_threshold = by_id["silent-threshold-anoncred-v0"]
    assert silent_threshold["implementation_stage"] == "sdk-builder"
    assert silent_threshold["public_inputs_schema"] == (
        "issuer_set_commitment,threshold_policy_hash,"
        "credential_showing_commitment,showing_nullifier,"
        "verifier_policy_hash,domain_separator"
    )
    assert silent_threshold["sdk_entrypoints"] == [
        "buildSilentThresholdCredentialCommitments",
        "buildSilentThresholdCredentialEnvelope",
        "buildSilentThresholdCredentialDevProofFixture",
        "verifySilentThresholdCredentialProofLocally",
    ]
    assert silent_threshold["planned_sdk_entrypoints"] == [
        "buildSilentThresholdCredentialShowingProofV0",
        "buildSubmitSilentThresholdCredentialProofInstruction",
    ]

    zk_x509 = by_id["zk-x509-onchain-identity-v0"]
    assert zk_x509["implementation_stage"] == "sdk-builder"
    assert zk_x509["public_inputs_schema"] == (
        "ca_root_commitment,certificate_policy_hash,revocation_root,"
        "subject_commitment,address_binding,domain_separator"
    )
    assert zk_x509["sdk_entrypoints"] == [
        "buildZkX509IdentityCommitments",
        "buildZkX509IdentityEnvelope",
        "buildZkX509IdentityDevProofFixture",
        "verifyZkX509IdentityProofLocally",
    ]
    assert zk_x509["planned_sdk_entrypoints"] == [
        "buildZkX509IdentityProofV0",
        "buildSubmitZkX509IdentityProofInstruction",
    ]

    jindo = by_id["jindo-lattice-pcs-zk-v0"]
    assert jindo["implementation_stage"] == "sdk-builder"
    assert jindo["public_inputs_schema"] == (
        "commitment,opening_claim,query_set,parameter_hash,domain_separator"
    )
    assert jindo["sdk_entrypoints"] == [
        "buildJindoLatticePublicInputs",
        "buildJindoLatticeProofEnvelope",
        "buildJindoLatticeDevProofFixture",
        "verifyJindoLatticeProofLocally",
    ]
    assert jindo["planned_sdk_entrypoints"] == [
        "buildJindoLatticeProofV0",
        "verifyJindoPolynomialCommitmentV0",
    ]

    sis_hints = by_id["sis-hints-anoncred-pq-v0"]
    assert sis_hints["implementation_stage"] == "sdk-builder"
    assert sis_hints["public_inputs_schema"] == (
        "issuer_commitment,credential_commitment,"
        "showing_policy_hash,parameter_hash,domain_separator"
    )
    assert sis_hints["sdk_entrypoints"] == [
        "buildSisHintsCredentialCommitments",
        "buildSisHintsCredentialEnvelope",
        "buildSisHintsCredentialDevProofFixture",
        "verifySisHintsCredentialProofLocally",
    ]
    assert sis_hints["planned_sdk_entrypoints"] == [
        "buildSisHintsAnonymousCredentialProofV0",
        "buildSubmitSisHintsCredentialProofInstruction",
    ]

    verange = by_id["verange-transparent-range-v1"]
    assert verange["implementation_stage"] == "component"
    assert verange["public_inputs_schema"] == (
        "commitments,range_parameters,aggregation_count,domain_separator,payload_digest"
    )
    assert verange["sdk_entrypoints"] == [
        "buildRangeCommitment",
        "buildVeRangeDevProofFixture",
        "buildVeRangeProofEnvelope",
        "verifyVeRangeProofLocally",
    ]
    assert verange["planned_sdk_entrypoints"] == [
        "buildVeRangeProofV1",
    ]

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
    assert capabilities["zk_ace_air_opening_privacy_v1"] is True
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
    assert capabilities["zk_ace_air_opening_privacy_v1"] is True
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
    assert capabilities["zk_ace_validator_support_v1"] is False


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
