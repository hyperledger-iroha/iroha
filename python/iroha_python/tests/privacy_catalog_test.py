from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

import pytest

import iroha_python
from iroha_python import (
    ToriiClient,
    crypto,
    get_privacy_algorithm_descriptor,
    get_privacy_algorithm_descriptors,
    get_privacy_criteria,
    privacy_capabilities,
)
from iroha_python import privacy_catalog

EXPECTED_PRIVACY_CAPABILITY_KEYS = frozenset(
    {
        "anonymous_pgc_dev_fixture_v1",
        "anonymous_pgc_local_verifier_v1",
        "anonymous_pgc_receiver_set_builder_v1",
        "anonymous_pgc_sdk_exports_v1",
        "asset_hidden_pool_registration_instruction",
        "asset_hidden_transfer_instruction",
        "asset_hidden_transfer_proof_v1",
        "aztec_private_rollup_sdk_exports_v1",
        "bridge_available",
        "confidential_transfer_proof_v2",
        "confidential_unshield_proof_v3",
        "miden_stark_note_sdk_exports_v1",
        "monero_fcmp_plus_plus_sdk_exports_v1",
        "jindo_lattice_proof_builder_v0",
        "jindo_lattice_proof_envelope_builder_v0",
        "jindo_lattice_proof_verifier_v0",
        "jindo_lattice_public_inputs_builder_v0",
        "jindo_lattice_sdk_exports_v0",
        "ml_dsa_authorization",
        "ml_kem_note_encryption",
        "orchard_halo2_actions_sdk_exports_v1",
        "penumbra_masp_sdk_exports_v1",
        "pq_masp_stark_sdk_exports_v0",
        "privacy_algorithms",
        "privacy_criteria",
        "python_sdk_available",
        "shield_instruction",
        "silent_threshold_commitments_builder_v0",
        "silent_threshold_credential_proof_builder_v0",
        "silent_threshold_credential_proof_verifier_v0",
        "silent_threshold_envelope_builder_v0",
        "silent_threshold_sdk_exports_v0",
        "sis_hints_credential_commitments_builder_v0",
        "sis_hints_credential_envelope_builder_v0",
        "sis_hints_credential_proof_builder_v0",
        "sis_hints_credential_proof_verifier_v0",
        "sis_hints_credential_sdk_exports_v0",
        "stark_proof_family",
        "transfer_asset_instruction",
        "unshield_instruction",
        "vega_credential_predicate_proof_builder_v0",
        "vega_credential_predicate_proof_verifier_v0",
        "vega_predicate_commitment_builder_v0",
        "vega_proof_envelope_builder_v0",
        "vega_sdk_exports_v0",
        "verify_proof_instruction",
        "verange_commitment_builder_v1",
        "verange_proof_envelope_builder_v1",
        "verange_proof_builder_v1",
        "verange_proof_verifier_v1",
        "verange_dev_fixture_v1",
        "verange_local_verifier_v1",
        "verange_sdk_exports_v1",
        "zk_ace_air_opening_privacy_v1",
        "zk_ace_authorization_proof_v1",
        "zk_ace_authorized_transfer_instruction",
        "zk_ace_identity_lifecycle_instruction",
        "zk_ace_native_air_prover_v1",
        "zk_ace_register_identity_instruction",
        "zk_ace_revoke_identity_instruction",
        "zk_ace_rotate_identity_instruction",
        "zk_ace_sdk_exports_v1",
        "zk_ace_validator_support_v1",
        "zk_ams_admission_batch_builder_v0",
        "zk_ams_admission_batch_proof_builder_v0",
        "zk_ams_admission_batch_proof_verifier_v0",
        "zk_ams_proof_envelope_builder_v0",
        "zk_ams_sdk_exports_v0",
        "zk_transfer_instruction",
        "zk_x509_identity_commitments_builder_v0",
        "zk_x509_identity_dev_fixture_v0",
        "zk_x509_identity_envelope_builder_v0",
        "zk_x509_identity_local_verifier_v0",
        "zk_x509_identity_proof_builder_v0",
        "zk_x509_identity_proof_verifier_v0",
        "zk_x509_identity_sdk_exports_v0",
        "zkat_authenticator_envelope_builder_v1",
        "zkat_policy_commitment_builder_v1",
        "zkat_policy_proof_builder_v1",
        "zkat_policy_proof_verifier_v1",
        "zkat_sdk_exports_v1",
    }
)


def _expected_production_gate_items() -> list[tuple[str, bool]]:
    return [(key, False) for key, _label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS]


def _expected_required_production_gate_keys(algorithm_id: object) -> list[str]:
    waived = (
        set(privacy_catalog.TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS)
        if algorithm_id == "transparent-transfer"
        else set()
    )
    return [
        key
        for key, _label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS
        if key not in waived
    ]


def _expected_production_gate_missing(gate: dict[str, object]) -> list[str]:
    missing = gate["missing"]
    assert isinstance(missing, list)
    required_gates = gate["required_gates"]
    assert isinstance(required_gates, list)
    required_gate_set = set(required_gates)
    return [
        *(
            label
            for key, label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS
            if key in required_gate_set
        ),
        *(
            reason
            for reason in privacy_catalog.PRODUCTION_GATE_SUPPLEMENTAL_MISSING_REASONS
            if reason in missing
        ),
    ]


def _privacy_production_test_artifact(label: str) -> dict[str, str]:
    digest = hashlib.sha256(label.encode("utf-8")).hexdigest()
    return {"label": label, "uri": f"sha256:{digest}"}


def test_privacy_catalog_internal_review_evidence_test_artifact_helper_uses_sha256() -> None:
    label = "kagemusha-test-artifact"
    assert _privacy_production_test_artifact(label) == {
        "label": label,
        "uri": f"sha256:{hashlib.sha256(label.encode('utf-8')).hexdigest()}",
    }
    assert _privacy_production_test_artifact(
        "localnet-lifecycle-recursive-init-transparent-transfer"
    )["uri"] != _privacy_production_test_artifact(
        "localnet-lifecycle-recursive-append-transparent-transfer"
    )["uri"]


PRIVACY_PRODUCTION_TEST_REVIEW_SIGNATURE = (
    "ed25519:"
    + hashlib.sha512(b"privacy-production-review-artifact-signature").hexdigest()
)


def _privacy_production_test_entrypoints(
    descriptor: dict[str, object],
) -> list[str]:
    return privacy_catalog._privacy_descriptor_production_sdk_entrypoints(descriptor)


def _privacy_production_test_sdk_parity_artifacts(
    algorithm_id: str,
) -> dict[str, dict[str, dict[str, str]]]:
    return {
        kind: {
            surface: _privacy_production_test_artifact(
                f"{algorithm_id}-{surface}-{kind}-sdk-parity"
            )
            for surface in privacy_catalog.PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES
        }
        for kind in privacy_catalog.PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS
    }


def _privacy_production_test_sdk_exports(
    entrypoints: list[str],
) -> dict[str, list[str]]:
    return {
        surface: list(entrypoints)
        for surface in privacy_catalog.PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES
    }


def _privacy_production_test_row(
    descriptor: dict[str, object],
    *,
    chain_id: str,
    localnet_run_id: str,
) -> dict[str, object]:
    algorithm_id = str(descriptor["id"])
    entrypoints = _privacy_production_test_entrypoints(descriptor)
    fuzz_artifact = _privacy_production_test_artifact(f"{algorithm_id}-fuzz")
    performance_artifact = _privacy_production_test_artifact(f"{algorithm_id}-perf")
    gate_evidence = {
        key: [_privacy_production_test_artifact(f"{algorithm_id}-{key}")]
        for key in _expected_required_production_gate_keys(algorithm_id)
    }
    return {
        "version": privacy_catalog.PRODUCTION_GATE_VERSION,
        "covered_algorithm_id": algorithm_id,
        "chain_id": chain_id,
        "reviewer_identity": "crypto-reviewer@internal.example",
        "review_artifact": {
            **_privacy_production_test_artifact(f"{algorithm_id}-review"),
            "signature": PRIVACY_PRODUCTION_TEST_REVIEW_SIGNATURE,
        },
        "verifier_key_id": descriptor["verifier_key_id"],
        "proof_family": descriptor["proof_family"],
        "public_inputs_schema": descriptor["public_inputs_schema"],
        "sdk_entrypoints": list(entrypoints),
        "sdk_exports": _privacy_production_test_sdk_exports(entrypoints),
        "sdk_parity_artifacts": _privacy_production_test_sdk_parity_artifacts(
            algorithm_id
        ),
        "required_state": list(descriptor["required_state"]),
        "review_scope": {
            "version": privacy_catalog.PRIVACY_PRODUCTION_REVIEW_SCOPE_VERSION,
            "algorithm_id": algorithm_id,
            "chain_id": chain_id,
            "verifier_key_id": descriptor["verifier_key_id"],
            "proof_family": descriptor["proof_family"],
            "public_inputs_schema": descriptor["public_inputs_schema"],
            "sdk_entrypoints": list(entrypoints),
            "required_state": list(descriptor["required_state"]),
            "fuzz_artifact_hash": fuzz_artifact["uri"],
            "performance_artifact_hash": performance_artifact["uri"],
            "localnet_run_id": localnet_run_id,
        },
        "fuzz_results": {
            "passed": True,
            "artifact": fuzz_artifact,
        },
        "performance_results": {
            "passed": True,
            "artifact": performance_artifact,
        },
        "localnet_run_id": localnet_run_id,
        "localnet_acceptance": {
            "run_id": localnet_run_id,
            "target": "localnet",
            "peer_count": 4,
            "peer_ids": [
                "boi-privacy-peer-1@localnet",
                "boi-privacy-peer-2@localnet",
                "boi-privacy-peer-3@localnet",
                "boi-privacy-peer-4@localnet",
            ],
            "chain_id": chain_id,
            "smoke_passed": True,
            "smoke_tx_hash": _privacy_production_test_artifact(
                f"localnet-smoke-{algorithm_id}"
            )["uri"],
            "replay_rejected": True,
            "replay_rejection_hash": _privacy_production_test_artifact(
                f"localnet-replay-{algorithm_id}"
            )["uri"],
            "restart_persistence_checked": True,
            "restart_replay_rejected": True,
            "restart_replay_rejection_hash": _privacy_production_test_artifact(
                f"localnet-restart-replay-{algorithm_id}"
            )["uri"],
            "state_recovery_passed": True,
            "state_recovery_hash": _privacy_production_test_artifact(
                f"localnet-state-recovery-{algorithm_id}"
            )["uri"],
            "lifecycle_passed": True,
            "lifecycle_shield_tx_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-shield-{algorithm_id}"
            )["uri"],
            "lifecycle_hop_proof_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-hop-{algorithm_id}"
            )["uri"],
            "lifecycle_recursive_init_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-recursive-init-{algorithm_id}"
            )["uri"],
            "lifecycle_recursive_init_verify_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-recursive-init-verify-{algorithm_id}"
            )["uri"],
            "lifecycle_recursive_append_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-recursive-append-{algorithm_id}"
            )["uri"],
            "lifecycle_recursive_append_verify_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-recursive-append-verify-{algorithm_id}"
            )["uri"],
            "lifecycle_unshield_proof_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-unshield-{algorithm_id}"
            )["uri"],
            "lifecycle_redeem_tx_hash": _privacy_production_test_artifact(
                f"localnet-lifecycle-redeem-{algorithm_id}"
            )["uri"],
        },
        "gate_evidence": gate_evidence,
    }


def _privacy_production_test_manifest(
    descriptors: list[dict[str, object]],
    *,
    chain_id: str = "boi-localnet-4p",
    localnet_run_id: str = "boi-localnet-4peer-run-2026-06-09",
) -> dict[str, object]:
    return {
        "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
        "rows": [
            _privacy_production_test_row(
                descriptor,
                chain_id=chain_id,
                localnet_run_id=localnet_run_id,
            )
            for descriptor in descriptors
        ],
    }


def _raw_descriptor(**patch: object) -> dict[str, object]:
    descriptor: dict[str, object] = {
        "id": "shield",
        "name": "Shape check",
        "shortName": "Shape",
        "summary": "Descriptor used to test hostile catalog input validation.",
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
            "public_inputs_schema": (
                "asset,from,amount,note_commitment,chain_id,domain_separator"
            ),
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
        json.dumps([_raw_descriptor(), ["not", "an", "object"]]),
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


def test_privacy_catalog_loader_rejects_duplicate_verifier_key_ids(monkeypatch) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
                [
                _raw_descriptor(
                    id="shield",
                    publicInputsSchema="root",
                    verifierKeyId="shared_verifier_key",
                ),
                _raw_descriptor(
                    id="unshield",
                    publicInputsSchema="root",
                    verifierKeyId="shared_verifier_key",
                ),
            ]
        ),
    )

    with pytest.raises(RuntimeError, match="duplicate verifier_key_id 'shared_verifier_key'"):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "backend_family_map",
    [
        {},
        {"shield": "commitment-only", "stale-row": "stale-backend"},
    ],
)
def test_privacy_catalog_rejects_backend_family_registration_drift(
    monkeypatch,
    backend_family_map,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "BACKEND_FAMILY_BY_ALGORITHM_ID",
        backend_family_map,
    )

    with pytest.raises(
        RuntimeError,
        match="backend-family registration must exactly match catalog ids",
    ):
        privacy_catalog._validate_backend_family_registration(({"id": "shield"},))


def test_privacy_catalog_enforces_required_production_privacy_plan_rows() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    by_id = {descriptor["id"]: descriptor for descriptor in descriptors}

    for algorithm_id, implementation_stage, backend_family in (
        privacy_catalog.REQUIRED_PRIVACY_PLAN_ROWS
    ):
        descriptor = by_id[algorithm_id]
        assert descriptor["implementation_stage"] == implementation_stage
        assert descriptor["backend_family"] == backend_family
        if implementation_stage == "production-hardened":
            assert descriptor["planned_sdk_entrypoints"] == []
            assert any(
                privacy_catalog._entrypoint_is_production_proof_builder(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
        elif implementation_stage in (
            "component",
            "catalog-as-of-2026-05",
            "research-target-as-of-2026-05",
        ):
            assert descriptor["planned_sdk_entrypoints"] or descriptor["sdk_entrypoints"]
            assert descriptor["planned_sdk_entrypoints"] or any(
                privacy_catalog._entrypoint_is_production_proof_builder(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            ) or implementation_stage == "catalog-as-of-2026-05"
        else:
            assert descriptor["sdk_entrypoints"]
        assert descriptor["production_ready"] is False


def test_privacy_catalog_required_plan_raw_source_matches_required_inventory() -> None:
    raw_rows = {
        row["id"]: privacy_catalog._canonicalize_value(row)
        for row in json.loads(privacy_catalog._RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON)
    }

    for algorithm_id, implementation_stage, _backend_family in (
        privacy_catalog.REQUIRED_PRIVACY_PLAN_ROWS
    ):
        descriptor = raw_rows[algorithm_id]
        assert (
            descriptor["name"],
            descriptor["short_name"],
            descriptor["summary"],
        ) == privacy_catalog.REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[
            algorithm_id
        ]
        assert descriptor["implementation_stage"] == implementation_stage
        assert tuple(descriptor["security_notes"]) == (
            privacy_catalog.REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[
                algorithm_id
            ]
        )
        assert tuple(descriptor["sdk_entrypoints"]) == (
            privacy_catalog.REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
                algorithm_id
            ]
        )
        assert tuple(descriptor["planned_sdk_entrypoints"]) == (
            privacy_catalog.REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
                algorithm_id
            ]
        )


@pytest.mark.parametrize(
    ("raw_key", "tampered_value", "expected_field"),
    [
        ("implementationStage", "production-hardened", "implementation_stage"),
        ("securityNotes", ["tampered production note"], "security_notes"),
        ("sdkEntrypoints", ["buildAnonymousPgcDevProofFixture"], "sdk_entrypoints"),
        (
            "plannedSdkEntrypoints",
            ["buildAnonymousPgcKOutOfNProofV1"],
            "planned_sdk_entrypoints",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_required_plan_raw_source_overlay_drift(
    monkeypatch,
    raw_key,
    tampered_value,
    expected_field,
) -> None:
    rows = json.loads(privacy_catalog._RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON)
    for row in rows:
        if row["id"] == "anonymous-pgc-k-out-of-n-v1":
            row[raw_key] = tampered_value
            break

    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(rows),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required privacy plan raw source row "
            "'anonymous-pgc-k-out-of-n-v1'.*"
            f"field {expected_field!r}"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_pending_chain_backends_stay_pre_production() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    pending_backend_descriptors = [
        descriptor
        for descriptor in descriptors
        if descriptor["backend_family"]
        in privacy_catalog.PENDING_PRODUCTION_BACKEND_FAMILIES
    ]

    assert pending_backend_descriptors
    for descriptor in pending_backend_descriptors:
        assert descriptor["implementation_stage"] != "production-hardened"
        assert (
            "implementation stage is not production-hardened"
            in descriptor["production_gate"]["missing"]
        )


def test_privacy_catalog_rejects_pending_chain_backend_marked_production() -> None:
    descriptor = privacy_catalog._canonicalize_value(
        _raw_descriptor(
            id="orchard-halo2-actions-v1",
            implementationStage="production-hardened",
            proofFamily="halo2-pasta-action-bundle",
            publicInputsSchema="anchor,nullifiers,cmx,value_commitments,binding_signature",
            verifierKeyId="orchard_halo2_action_bundle_v1",
            sdkEntrypoints=["buildOrchardActionBundleProofV1"],
            plannedSdkEntrypoints=[],
        )
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "backend family 'halo2-ipa-orchard' is still pending "
            "production chain admission"
        ),
    ):
        privacy_catalog._validate_descriptor_shape(descriptor, 0)


def test_privacy_catalog_rejects_missing_required_production_privacy_plan_row(
) -> None:
    descriptors = [
        descriptor
        for descriptor in get_privacy_algorithm_descriptors()
        if descriptor["id"] != "anonymous-pgc-k-out-of-n-v1"
    ]

    with pytest.raises(
        RuntimeError,
        match=(
            "missing required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_stage_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["implementation_stage"] = "component"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep implementation_stage "
            "'sdk-builder'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_backend_drift(
    monkeypatch,
) -> None:
    backend_family_map = dict(privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID)
    backend_family_map["anonymous-pgc-k-out-of-n-v1"] = "forged-backend"
    monkeypatch.setattr(
        privacy_catalog,
        "BACKEND_FAMILY_BY_ALGORITHM_ID",
        backend_family_map,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep backend family "
            "'anonymous-pgc'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(
            get_privacy_algorithm_descriptors()
        )


def test_privacy_catalog_rejects_required_production_privacy_plan_display_text_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["summary"] = "Account-based private payment pilot."
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep display text"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_category_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["category"] = "authorization"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep category 'payment'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_maturity_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["maturity"] = "specification"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep maturity "
            "'accepted_conference'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_recommended_for_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["recommended_for"][0] = "claimed production rollout"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep recommendedFor"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_covered_criteria_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["covered_criteria"].append("hide_asset_type")
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep covered criteria"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_proof_family_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["proof_family"] = "forged-proof-family"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep proof family "
            "'anonymous-pgc-k-out-of-n'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_public_input_schema_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["public_inputs_schema"] = "forged_public_input"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep public inputs schema "
            "'anonymity_set_root,tx_digest,balance_commitments,"
            "receiver_set_commitment,receiver_ciphertext_commitments,"
            "receiver_threshold,receiver_count,link_tag,range_commitments,"
            "chain_id,domain_separator'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_verifier_key_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["verifier_key_id"] = "forged_verifier_key"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep verifier key id "
            "'anonymous_pgc_k_out_of_n_v1'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_pq_layer_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["pq_layers"]["proof"] = True
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep PQ layer 'proof'=False"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_chain_requirement_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["chain_requirements"][5] = (
                "typed zk::SubmitAnonymousPgcProofOnly instruction"
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep chain requirements"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_required_state_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["required_state"][4] = "forged wallet recovery placeholder"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep required state"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_setup_step_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["setup_steps"][1] = "Register forged Anonymous PGC verifier setup."
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep setup steps"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_execution_step_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["execution_steps"][2] = (
                "Submit forged Anonymous PGC proof-only envelope."
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep execution steps"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_state_token_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    required_state_tokens = dict(
        privacy_catalog.REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID
    )
    required_state_tokens["anonymous-pgc-k-out-of-n-v1"] = (
        "forged state placeholder",
    )
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID",
        required_state_tokens,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required state token "
            "'forged state placeholder'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(
            get_privacy_algorithm_descriptors()
        )


def test_privacy_catalog_rejects_required_production_privacy_plan_state_token_concatenated_false_positive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    required_state_by_id = {
        key: tuple(value)
        for key, value in privacy_catalog.REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID.items()
    }
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["required_state"][-1] = (
                "notwallet account blinding and receiver recovery metadata"
            )
            required_state_by_id[descriptor["id"]] = tuple(
                descriptor["required_state"]
            )
            break
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        required_state_by_id,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required state token "
            "'wallet account blinding'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_state_token_negated_bounded_false_positive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    required_state_by_id = {
        key: tuple(value)
        for key, value in privacy_catalog.REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID.items()
    }
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["required_state"][-1] = (
                "not wallet account blinding and receiver recovery metadata"
            )
            required_state_by_id[descriptor["id"]] = tuple(
                descriptor["required_state"]
            )
            break
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        required_state_by_id,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required state token "
            "'wallet account blinding'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_failure_modes_drift() -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["failure_modes"][1] = "accept forged replay tag"
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep failure modes"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_security_note_drift() -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["security_notes"][-1] = (
                "Production hardening requires parser fuzzing, latency gates, "
                "and internal cryptographic review."
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep security notes"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    required_failure_tokens = dict(
        privacy_catalog.REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID
    )
    required_failure_tokens["anonymous-pgc-k-out-of-n-v1"] = (
        "forged failure placeholder",
    )
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID",
        required_failure_tokens,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required "
            "failure-mode token 'forged failure placeholder'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(
            get_privacy_algorithm_descriptors()
        )


def test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_concatenated_false_positive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    required_failure_modes_by_id = {
        key: tuple(value)
        for key, value in privacy_catalog.REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID.items()
    }
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["failure_modes"][2] = "notreceiver-set substitution"
            required_failure_modes_by_id[descriptor["id"]] = tuple(
                descriptor["failure_modes"]
            )
            break
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        required_failure_modes_by_id,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required "
            "failure-mode token 'receiver-set substitution'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_negated_bounded_false_positive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    required_failure_modes_by_id = {
        key: tuple(value)
        for key, value in privacy_catalog.REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID.items()
    }
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["failure_modes"][2] = "not receiver-set substitution"
            required_failure_modes_by_id[descriptor["id"]] = tuple(
                descriptor["failure_modes"]
            )
            break
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        required_failure_modes_by_id,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain required "
            "failure-mode token 'receiver-set substitution'"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["source_references"][0]["url"] = (
                "https://example.com/forged-source"
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain source reference "
            "'Anonymous PGC with k-out-of-n Proofs' "
            "<https://eprint.iacr.org/2025/884>"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_extra(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["source_references"].append(
                {
                    "label": "Forged extra source",
                    "url": "https://example.com/forged-extra-source",
                }
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep source references"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_sdk_entrypoint_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["sdk_entrypoints"][1] = (
                "buildForgedAnonymousPgcProductionProof"
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep SDK entrypoints"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_rejects_required_production_privacy_plan_planned_sdk_entrypoint_drift(
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["planned_sdk_entrypoints"].append(
                "buildForgedAnonymousPgcProofV1"
            )
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must keep planned SDK entrypoints"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


@pytest.mark.parametrize(
    "sdk_entrypoints",
    [
        [],
        ["deriveOrchardWitness"],
        ["buildAnonymousPgcProductionInstruction"],
        ["buildAnonymousPgcProofTransaction"],
        ["buildSubmitAnonymousPgcProof"],
        ["buildAnonymousPgcProofEnvelope"],
        ["buildAnonymousPgcProofWitness"],
        ["buildAnonymousPgcProofPublicInputs"],
        ["buildAnonymousPgcProofRequest"],
        ["buildAnonymousPgcProofCommitment"],
        ["buildAnonymousPgcDevProofFixture"],
        ["buildAnonymousPgcNoProofBuilder"],
        ["buildAnonymousPgcNotProofBuilder"],
    ],
)
def test_privacy_catalog_rejects_required_production_privacy_plan_without_proof_builder(
    monkeypatch: pytest.MonkeyPatch,
    sdk_entrypoints,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["sdk_entrypoints"] = sdk_entrypoints
            break
    sdk_entrypoints_by_id = dict(
        privacy_catalog.REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID
    )
    sdk_entrypoints_by_id["anonymous-pgc-k-out-of-n-v1"] = tuple(sdk_entrypoints)
    monkeypatch.setattr(
        privacy_catalog,
        "REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        sdk_entrypoints_by_id,
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain or export a production "
            "proof builder"
        ),
    ):
        privacy_catalog._validate_required_privacy_plan_rows(tuple(descriptors))


def test_privacy_catalog_research_targets_keep_executable_entrypoints_planned_only() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    assert any(
        descriptor["sdk_entrypoints"]
        for descriptor in descriptors
        if descriptor["implementation_stage"] != "research-target-as-of-2026-05"
    )
    for descriptor in descriptors:
        if descriptor["implementation_stage"] != "research-target-as-of-2026-05":
            continue
        assert descriptor["sdk_entrypoints"] == []
        assert descriptor["planned_sdk_entrypoints"]


def test_privacy_catalog_research_targets_keep_exact_protocol_source_references() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    protocol_source_ids = {
        descriptor["id"]
        for descriptor in descriptors
        if descriptor["id"] in privacy_catalog._RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID
    }
    assert protocol_source_ids == set(
        privacy_catalog._RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID
    )

    for descriptor in descriptors:
        if descriptor["id"] not in privacy_catalog._RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID:
            continue
        source_urls = {
            source_reference["url"]
            for source_reference in descriptor["source_references"]
        }
        required_urls = privacy_catalog._RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID[
            descriptor["id"]
        ]
        assert required_urls <= source_urls


def test_privacy_catalog_research_targets_keep_production_readiness_notes() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    for descriptor in descriptors:
        if descriptor["implementation_stage"] != "research-target-as-of-2026-05":
            continue
        security_notes_text = " ".join(descriptor["security_notes"]).lower()
        assert all(
            privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                security_notes_text,
                token,
            )
            for token in privacy_catalog._RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS
        )
        assert any(
            privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                security_notes_text,
                token,
            )
            for token in privacy_catalog._RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS
        )


def test_privacy_catalog_research_protocol_rows_export_production_sdk_entrypoints() -> None:
    descriptors = {
        descriptor["id"]: descriptor
        for descriptor in get_privacy_algorithm_descriptors()
    }
    orchard = descriptors["orchard-halo2-actions-v1"]

    assert orchard["implementation_stage"] == "sdk-builder"
    assert orchard["sdk_entrypoints"] == [
        "buildOrchardActionBundleProofV1",
        "buildOrchardActionBundleInstruction",
    ]
    assert orchard["planned_sdk_entrypoints"] == []
    privacy_catalog._validate_research_target_sdk_entrypoints(
        tuple(descriptors.values())
    )


def test_privacy_catalog_loader_rejects_research_target_without_exact_protocol_source(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="orchard-halo2-actions-v1",
                    implementationStage="research-target-as-of-2026-05",
                    proofFamily="halo2-pasta-action-bundle",
                    publicInputsSchema="anchor,nullifiers,cmx,value_commitments,binding_signature",
                    verifierKeyId="orchard_halo2_action_bundle_v1",
                    recommendedFor=["shape validation"],
                    chainRequirements=["Orchard shielded state tree"],
                    securityNotes=["Use only for hostile shape validation."],
                    requiredState=[
                        "Orchard note commitment tree",
                        "wallet Orchard witness store",
                    ],
                    failureModes=["missing exact source reference"],
                    setupSteps=["Register Orchard verifier metadata."],
                    executionSteps=["Build an Orchard action-bundle proof."],
                    sourceReferences=[
                        {
                            "label": "Zcash Protocol Specification",
                            "url": "https://zips.z.cash/protocol/protocol.pdf",
                        }
                    ],
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildOrchardActionBundleProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="source_references' must include exact research target source URLs",
    ) as error:
        privacy_catalog._load_descriptors()

    assert "https://zips.z.cash/zip-0224" in str(error.value)


def test_privacy_catalog_loader_rejects_research_target_without_production_readiness_note(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="orchard-halo2-actions-v1",
                    implementationStage="research-target-as-of-2026-05",
                    sourceReferences=[
                        {
                            "label": "ZIP 224 Orchard Shielded Protocol",
                            "url": "https://zips.z.cash/zip-0224",
                        }
                    ],
                    securityNotes=[
                        "Orchard note semantics must remain domain-separated.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
                    ],
                    recommendedFor=["shape research"],
                    chainRequirements=["Orchard shielded state tree"],
                    requiredState=[
                        "Orchard shielded state tree",
                        "Orchard action-bundle verifier key registry",
                        "wallet Orchard witness store",
                    ],
                    failureModes=[
                        "stale anchor",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register Orchard verifier key parameters."],
                    executionSteps=["Build Orchard proof."],
                    proofFamily="halo2-pasta-action-bundle",
                    publicInputsSchema="anchor,nullifiers,cmx",
                    verifierKeyId="orchard_halo2_action_bundle_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildOrchardActionBundleProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include production readiness audit or "
            "review gating for research targets"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_research_readiness_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="orchard-halo2-actions-v1",
                    implementationStage="research-target-as-of-2026-05",
                    sourceReferences=[
                        {
                            "label": "ZIP 224 Orchard Shielded Protocol",
                            "url": "https://zips.z.cash/zip-0224",
                        }
                    ],
                    securityNotes=[
                        "Orchard note semantics must remain domain-separated.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
                        "notproduction readiness planning remains gated.",
                    ],
                    recommendedFor=["shape research"],
                    chainRequirements=["Orchard note commitment tree"],
                    requiredState=[
                        "Orchard note commitment tree",
                        "wallet Orchard witness store",
                        "Orchard action-bundle verifier key registry",
                    ],
                    failureModes=[
                        "stale anchor",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register Orchard verifier key parameters."],
                    executionSteps=["Build Orchard proof."],
                    proofFamily="halo2-pasta-action-bundle",
                    publicInputsSchema="anchor,nullifiers,cmx",
                    verifierKeyId="orchard_halo2_action_bundle_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildOrchardActionBundleProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include production readiness audit or "
            "review gating for research targets"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_research_readiness_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="orchard-halo2-actions-v1",
                    implementationStage="research-target-as-of-2026-05",
                    sourceReferences=[
                        {
                            "label": "ZIP 224 Orchard Shielded Protocol",
                            "url": "https://zips.z.cash/zip-0224",
                        }
                    ],
                    securityNotes=[
                        "Orchard note semantics must remain domain-separated.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
                        "not production readiness planning remains gated.",
                    ],
                    recommendedFor=["shape research"],
                    chainRequirements=["Orchard note commitment tree"],
                    requiredState=[
                        "Orchard note commitment tree",
                        "wallet Orchard witness store",
                        "Orchard action-bundle verifier key registry",
                    ],
                    failureModes=[
                        "stale anchor",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register Orchard verifier key parameters."],
                    executionSteps=["Build Orchard proof."],
                    proofFamily="halo2-pasta-action-bundle",
                    publicInputsSchema="anchor,nullifiers,cmx",
                    verifierKeyId="orchard_halo2_action_bundle_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildOrchardActionBundleProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include production readiness audit or "
            "review gating for research targets"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "patch",
    [
        {"publicInputsSchema": None, "verifierKeyId": "orphan_verifier_key"},
        {"publicInputsSchema": "root", "verifierKeyId": None},
    ],
)
def test_privacy_catalog_loader_rejects_unpaired_verifier_metadata(
    monkeypatch,
    patch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(**patch)]),
    )

    with pytest.raises(
        RuntimeError,
        match="public_inputs_schema' and 'verifier_key_id' must be supplied together",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "verifier_key_id",
    [
        "VerifierKey",
        "verifier_key_",
        "verifier__key",
        "verifier.key",
        "zk:Shield",
        "zk_::Shield",
        "zk::",
        "zk::Shield_",
        "zk::Shield__Key",
        "zk::Shield/../../admin",
    ],
)
def test_privacy_catalog_loader_rejects_bad_verifier_key_ids(
    monkeypatch,
    verifier_key_id,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    publicInputsSchema="root",
                    verifierKeyId=verifier_key_id,
                )
            ]
        ),
    )

    with pytest.raises(RuntimeError, match="field 'verifier_key_id' must be a verifier key id"):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "bad_id",
    [
        "Shield",
        "shield/../../admin",
        "shield space",
        "shield\nsecond",
        "shield.example",
        "_shield",
        "-shield",
        "shield_",
        "shield-",
    ],
)
def test_privacy_catalog_loader_rejects_additional_unsafe_ids(
    monkeypatch,
    bad_id,
) -> None:
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
        ("category", "rootkit", "field 'category' must be one of"),
        ("maturity", "production", "field 'maturity' must be one of"),
        (
            "implementationStage",
            "Production Hardened",
            "implementation_stage' must be a lowercase",
        ),
        (
            "implementationStage",
            "audited-production",
            "implementation_stage' must be a known implementation stage",
        ),
        (
            "implementationStage",
            "production-ready",
            "implementation_stage' must be a known implementation stage",
        ),
        ("coveredCriteria", ["hide_amount", "hide_amount"], "duplicates 'hide_amount'"),
        ("coveredCriteria", ["hide_identity"], "must be one of"),
        ("sdkEntrypoints", ["buildProof", "buildProof"], "duplicates 'buildProof'"),
        (
            "auditReferences",
            [{"label": "forged", "url": "https://audit.example/forged"}],
            "field 'auditReferences' is not a supported privacy catalog field",
        ),
        (
            "sourceReferences",
            [{"label": "paper\u200b", "url": "https://zips.z.cash/zip-0224"}],
            "clean bounded label",
        ),
        (
            "sourceReferences",
            [{"label": "paper\u007f", "url": "https://zips.z.cash/zip-0224"}],
            "clean bounded label",
        ),
        (
            "sourceReferences",
            [{"label": "Internal cryptographic review signoff", "url": "https://zips.z.cash/zip-0224"}],
            "label must describe protocol source material, not audit/signoff evidence",
        ),
        (
            "sourceReferences",
            [{"label": "Protocol s.e.c.u.r.i.t.y review", "url": "https://zips.z.cash/zip-0224"}],
            "label must describe protocol source material, not audit/signoff evidence",
        ),
        (
            "sourceReferences",
            [{"label": "Protocol security rev\u0456ew", "url": "https://zips.z.cash/zip-0224"}],
            "clean bounded label",
        ),
        (
            "sourceReferences",
            [{"label": "External.review report", "url": "https://zips.z.cash/zip-0224"}],
            "label must describe protocol source material, not audit/signoff evidence",
        ),
        (
            "sourceReferences",
            [{"label": "\u0391ssurance.report", "url": "https://zips.z.cash/zip-0224"}],
            "clean bounded label",
        ),
        (
            "sourceReferences",
            [{"label": "paper", "url": "https://audit.example/forged-signoff"}],
            "url must not be a placeholder, local, or private-network URL",
        ),
        (
            "extraProductionClaim",
            True,
            "field 'extraProductionClaim' is not a supported privacy catalog field",
        ),
        (
            "summary",
            "Mainnet-ready audited production proof.",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "summary",
            "M\u0430innet-re\u0430dy proof.",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "summary",
            "Claimed production proof.",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "name",
            "Claimed mainnet transfer",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "shortName",
            "Audit claim",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFuture", "buildFuture"],
            "duplicates 'buildFuture'",
        ),
        (
            "recommendedFor",
            [" audit evidence"],
            "must be clean and already trimmed",
        ),
        (
            "recommendedFor",
            ["Production-ready bank deployment"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "recommendedFor",
            ["claimed audit rollout"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "chainRequirements",
            ["production-ready verifier"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "requiredState",
            ["claimed mainnet root"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "setupSteps",
            ["Install audit claim verifier"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "executionSteps",
            ["Submit claimed production proof"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "id",
            "mainnet-ready-shield",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "id",
            "claimed-mainnet-shield",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "proofFamily",
            "halo2/mainnet-ready",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "proofFamily",
            "halo2/production-claim",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "publicInputsSchema",
            "root,production_gate_passed",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "publicInputsSchema",
            "root,audit_claim",
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        ("securityNotes", ["valid", ""], "must be a non-empty string"),
        (
            "securityNotes",
            ["line\nbreak"],
            "must be clean and already trimmed",
        ),
        (
            "securityNotes",
            ["line\u200bbreak"],
            "must be clean and already trimmed",
        ),
        (
            "securityNotes",
            ["Internal cryptographic review completed and production sign-off received."],
            (
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "securityNotes",
            ["A.u.d.i.t passed; s.e.c.u.r.i.t.y review approved."],
            (
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "securityNotes",
            ["External \u0430udit p\u0430ssed."],
            (
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "securityNotes",
            ["Claimed audit coverage is present."],
            (
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "securityNotes",
            ["Mainnet claim accepted by reviewer."],
            (
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "failureModes",
            ["Internal cryptographic review completed."],
            (
                "describe concrete failure modes, not completed audit "
                "or signoff claims"
            ),
        ),
        (
            "failureModes",
            ["Mainnet claim accepted by reviewer."],
            (
                "describe concrete failure modes, not completed audit "
                "or signoff claims"
            ),
        ),
        ("failureModes", ["valid", {"bad": "shape"}], "must be a string"),
        (
            "sdkEntrypoints",
            ["buildMainnetReadyProof"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "plannedSdkEntrypoints",
            ["buildAuditSignoffProof"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "plannedSdkEntrypoints",
            ["buildClaimedAuditProof"],
            (
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            ),
        ),
        (
            "recommendedFor",
            ["audit evidence", "audit evidence"],
            "field 'recommended_for' item 1 duplicates 'audit evidence'",
        ),
        ("chainRequirements", "not-a-list", "chain_requirements' must be a list"),
        (
            "chainRequirements",
            ["registry\x7f"],
            "must be clean and already trimmed",
        ),
        (
            "chainRequirements",
            ["registry\u200b"],
            "must be clean and already trimmed",
        ),
        (
            "sdkEntrypoints",
            [" buildProof"],
            "must be clean and already trimmed",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFuture\t"],
            "must be clean and already trimmed",
        ),
        (
            "chainRequirements",
            ["verifier registry", "verifier registry"],
            "field 'chain_requirements' item 1 duplicates 'verifier registry'",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_invalid_descriptor_fields(
    monkeypatch,
    field,
    value,
    message,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(**{field: value})]),
    )

    with pytest.raises(RuntimeError, match=message):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_verifier_key_production_claims(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    publicInputsSchema="root",
                    verifierKeyId="audited_production_vk",
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "verifier_key_id' must not claim production/mainnet/audit "
            "readiness before production gates pass"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "backend_family",
    [
        "stark-fri-mainnet-ready",
        "stark-fri-release-approved",
        "halo2-ipa-certified-mainnet",
        "stark-fri-boi-audited",
        "stark-fri-external-security-review",
    ],
)
def test_privacy_catalog_loader_rejects_backend_family_production_claims(
    monkeypatch, backend_family
) -> None:
    patched_backend_families = dict(privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID)
    patched_backend_families["shield"] = backend_family
    monkeypatch.setattr(
        privacy_catalog,
        "BACKEND_FAMILY_BY_ALGORITHM_ID",
        patched_backend_families,
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="shield")]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "backend family metadata must not claim production/mainnet/audit "
            "readiness before production gates pass"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "backend_family",
    [
        "",
        "halo2/ipa/pasta",
        "halo2:ipa:pasta",
        "halo2 ipa pasta",
        "halo2.ipa.pasta",
        "halo2_ipa_pasta",
        "halo2--ipa-pasta",
        "Halo2-ipa-pasta",
        ".halo2-ipa-pasta",
        "-halo2-ipa-pasta",
        "_halo2-ipa-pasta",
        "halo2-ipa-pasta.",
        "halo2-ipa-pasta-",
        "halo2-ipa-pasta_",
    ],
)
def test_privacy_catalog_loader_rejects_unportable_backend_families(
    monkeypatch,
    backend_family,
) -> None:
    patched_backend_families = dict(privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID)
    patched_backend_families["shield"] = backend_family
    monkeypatch.setattr(
        privacy_catalog,
        "BACKEND_FAMILY_BY_ALGORITHM_ID",
        patched_backend_families,
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="shield")]),
    )

    with pytest.raises(
        RuntimeError,
        match="request-portable verifier-key backend characters",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("source_references", "message"),
    [
        ({"label": "paper", "url": "https://example.test"}, "must be a list"),
        ([["paper", "https://example.test"]], "must be an object"),
        ([{"label": "", "url": "https://example.test"}], "must include non-empty"),
        ([{"label": " paper", "url": "https://example.test"}], "clean bounded label"),
        ([{"label": "paper\nnext", "url": "https://example.test"}], "clean bounded label"),
        ([{"label": "paper\x7f", "url": "https://example.test"}], "clean bounded label"),
        ([{"label": "p" * 161, "url": "https://example.test"}], "clean bounded label"),
        (
            [{"label": "Production-ready protocol source", "url": "https://example.test"}],
            (
                "label must not claim production/mainnet/audit readiness "
                "before production gates pass"
            ),
        ),
        ([{"label": "paper", "url": "http://example.test"}], "must use an https URL"),
        ([{"label": "paper", "url": "HTTPS://example.test"}], "must use an https URL"),
        ([{"label": "paper", "url": " https://example.test"}], "must use an https URL"),
        ([{"label": "paper", "url": "https://example.test/path\nnext"}], "must use an https URL"),
        ([{"label": "paper", "url": "https://user:pass@example.test"}], "must use an https URL"),
        ([{"label": "paper", "url": "https://"}], "must use an https URL"),
        ([{"label": "paper", "url": "https://example.test\\evil"}], "must use an https URL"),
        (
            [{"label": "paper", "url": "https://zips.z.ca\u0455h/zip-0224"}],
            "must use an https URL",
        ),
        (
            [{"label": "paper", "url": "https://xn--cah-ghd.org/source"}],
            "must use an https URL",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash/prot\u03bfcol/protocol.pdf"}],
            "must use an https URL",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash/zip-0224?claim=m\u0430innet"}],
            "must use an https URL",
        ),
        (
            [{"label": "paper", "url": "https://ZIPS.z.cash/zip-0224"}],
            "url must be canonical",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash:443/zip-0224"}],
            "url must be canonical",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash:8443/zip-0224"}],
            "url must be canonical",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash./zip-0224"}],
            "url must be canonical",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash/protocol/../zip-0224"}],
            "url must be canonical",
        ),
        (
            [{"label": "paper", "url": "https://zips.z.cash/protocol/%2e%2e/zip-0224"}],
            "url must be canonical",
        ),
        ([{"label": "paper"}], "must include string label and url"),
        (
            [
                {
                    "label": "paper",
                    "url": "https://example.test",
                    "productionGate": {"ready": True},
                }
            ],
            "contains unsupported keys",
        ),
        (
            [
                {"label": "paper", "url": "https://zips.z.cash/zip-0224"},
                {"label": "paper", "url": "https://zips.z.cash/zip-0225"},
            ],
            "duplicates label 'paper'",
        ),
        (
            [
                {"label": "paper A", "url": "https://zips.z.cash/zip-0224"},
                {"label": "paper B", "url": "https://zips.z.cash/zip-0224"},
            ],
            "duplicates url 'https://zips.z.cash/zip-0224'",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_bad_source_references(
    monkeypatch,
    source_references,
    message,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(sourceReferences=source_references)]),
    )

    with pytest.raises(RuntimeError, match=message):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "implementation_stage",
    [
        "sdk-builder",
        "component",
        "research-target-as-of-2026-05",
        "production-hardened",
    ],
)
@pytest.mark.parametrize("source_references", ["__missing__", []])
def test_privacy_catalog_loader_rejects_source_referenced_stages_without_sources(
    monkeypatch,
    implementation_stage,
    source_references,
) -> None:
    descriptor = _raw_descriptor(implementationStage=implementation_stage)
    if source_references != "__missing__":
        descriptor["sourceReferences"] = source_references
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match="source_references' is required for source-referenced implementation stages",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_allows_scaffold_stage_without_source_references(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="validator-scaffold-as-of-2026-05",
                )
            ]
        ),
    )

    [descriptor] = privacy_catalog._load_descriptors()

    assert descriptor["implementation_stage"] == "validator-scaffold-as-of-2026-05"


@pytest.mark.parametrize(
    "source_url",
    [
        "https://example.invalid/shape-source",
        "https://example.test/shape-source",
        "https://example.com/shape-source",
        "https://localhost/shape-source",
        "https://127.0.0.1/shape-source",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_with_placeholder_sources(
    monkeypatch,
    source_url,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage="sdk-builder",
        proofFamily="shape-proof",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": source_url,
            }
        ],
        sdkEntrypoints=["buildShapeProof"],
        plannedSdkEntrypoints=["buildFutureShapeProof"],
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match="url must not be a placeholder, local, or private-network URL",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "source_url",
    [
        "https://10.0.0.1/shape-source",
        "https://172.16.0.1/shape-source",
        "https://192.168.1.10/shape-source",
        "https://169.254.1.1/shape-source",
        "https://100.64.0.1/shape-source",
        "https://192.0.2.1/shape-source",
        "https://198.51.100.10/shape-source",
        "https://203.0.113.5/shape-source",
        "https://0177.0.0.1/shape-source",
        "https://0x7f.0.0.1/shape-source",
        "https://127.1/shape-source",
        "https://2130706433/shape-source",
        "https://0x7f000001/shape-source",
        "https://017700000001/shape-source",
        "https://192.168.257/shape-source",
        "https://[::1]/shape-source",
        "https://[::ffff:127.0.0.1]/shape-source",
        "https://[::ffff:c0a8:101]/shape-source",
        "https://[::7f00:1]/shape-source",
        "https://[64:ff9b::7f00:1]/shape-source",
        "https://[fe80::1]/shape-source",
        "https://[fec0::1]/shape-source",
        "https://[fc00::1]/shape-source",
        "https://[100::]/shape-source",
        "https://[2001:0000:4136:e378:8000:63bf:3fff:fdd2]/shape-source",
        "https://[2001:20::1]/shape-source",
        "https://[2001:db8::1]/shape-source",
        "https://[2002:7f00:1::]/shape-source",
        "https://[2002:c0a8:101::]/shape-source",
        "https://source.local/shape-source",
        "https://source.internal/shape-source",
        "https://127.0.0.1.nip.io/shape-source",
        "https://10.0.0.1.sslip.io/shape-source",
        "https://localhost.localtest.me/shape-source",
        "https://lvh.me/shape-source",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_with_private_sources(
    monkeypatch,
    source_url,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage="sdk-builder",
        proofFamily="shape-proof",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": source_url,
            }
        ],
        sdkEntrypoints=["buildShapeProof"],
        plannedSdkEntrypoints=["buildFutureShapeProof"],
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match="url must not be a placeholder, local, or private-network URL",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        (
            "recommendedFor",
            "__missing__",
            "field 'recommended_for' must be non-empty",
        ),
        ("recommendedFor", [], "field 'recommended_for' must be non-empty"),
        ("chainRequirements", [], "field 'chain_requirements' must be non-empty"),
        ("securityNotes", [], "field 'security_notes' must be non-empty"),
        ("requiredState", [], "field 'required_state' must be non-empty"),
        ("failureModes", [], "field 'failure_modes' must be non-empty"),
        ("setupSteps", [], "field 'setup_steps' must be non-empty"),
        ("executionSteps", [], "field 'execution_steps' must be non-empty"),
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_without_required_metadata(
    monkeypatch,
    field,
    value,
    message,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage="sdk-builder",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
    )
    if value == "__missing__":
        descriptor.pop(field)
    else:
        descriptor[field] = value
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=f"{message} for source-referenced implementation stages",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_stages_without_sdk_surface(
    monkeypatch,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage="production-hardened",
        proofFamily="shape-proof",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
    )
    descriptor["sdkEntrypoints"] = []
    descriptor["plannedSdkEntrypoints"] = []
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "source-referenced implementation stages must expose at least one "
            "executable or planned SDK entrypoint"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "implementation_stage",
    [
        "sdk-builder",
        "component",
        "research-target-as-of-2026-05",
        "production-hardened",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_without_verifier_binding(
    monkeypatch,
    implementation_stage,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage=implementation_stage,
        proofFamily="shape-proof",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
        sdkEntrypoints=(
            []
            if implementation_stage == "research-target-as-of-2026-05"
            else ["buildShapeProof"]
        ),
        plannedSdkEntrypoints=(
            []
            if implementation_stage == "production-hardened"
            else ["buildFutureShapeProof"]
        ),
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'public_inputs_schema' must be non-empty for "
            "source-referenced implementation stages"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "implementation_stage",
    [
        "sdk-builder",
        "component",
        "research-target-as-of-2026-05",
        "production-hardened",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_without_concrete_proof_family(
    monkeypatch,
    implementation_stage,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage=implementation_stage,
        proofFamily="none",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
        sdkEntrypoints=(
            []
            if implementation_stage == "research-target-as-of-2026-05"
            else ["buildShapeProof"]
        ),
        plannedSdkEntrypoints=(
            []
            if implementation_stage == "production-hardened"
            else ["buildFutureShapeProof"]
        ),
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'proof_family' must be a concrete proof family for "
            "source-referenced implementation stages"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "implementation_stage",
    [
        "chain-executable",
        "sdk-builder",
        "component",
        "research-target-as-of-2026-05",
        "production-hardened",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_stages_without_non_none_backend_family(
    monkeypatch,
    implementation_stage,
) -> None:
    descriptor = _raw_descriptor(
        id="shape-proof-v0",
        implementationStage=implementation_stage,
        proofFamily="shape-proof",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry", "wallet witness state metadata"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
        sdkEntrypoints=(
            []
            if implementation_stage == "research-target-as-of-2026-05"
            else ["buildShapeProof"]
        ),
        plannedSdkEntrypoints=(
            []
            if implementation_stage == "production-hardened"
            else ["buildFutureShapeProof"]
        ),
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must have a registered non-none backend family for "
            "source-referenced implementation stages"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "implementation_stage",
    [
        "component",
        "research-target-as-of-2026-05",
    ],
)
def test_privacy_catalog_loader_rejects_pre_production_stages_without_planned_sdk_surface(
    monkeypatch,
    implementation_stage,
) -> None:
    descriptor = _raw_descriptor(
        implementationStage=implementation_stage,
        proofFamily="shape-proof",
        publicInputsSchema="root,domain_separator",
        verifierKeyId="shape_verifier_v0",
        recommendedFor=["shape validation"],
        chainRequirements=["shape verifier registry"],
        securityNotes=["Review shape proof constraints"],
        requiredState=["shape verifier registry"],
        failureModes=["shape proof rejected"],
        setupSteps=["Register shape verifier"],
        executionSteps=["Build shape proof"],
        sourceReferences=[
            {
                "label": "Shape source",
                "url": "https://zips.z.cash/zip-0224",
            }
        ],
        sdkEntrypoints=(
            []
            if implementation_stage == "research-target-as-of-2026-05"
            else ["buildShapeProofEnvelope"]
        ),
        plannedSdkEntrypoints=[],
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned_sdk_entrypoints' must be non-empty or a production "
            "proof builder must be exported for pre-production "
            "source-referenced implementation stages"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("pq_layers", "message"),
    [
        (["proof"], "pq_layers' must be an object"),
        ({"proof": False, "authorization": False}, "pq_layers.note_encryption' is required"),
        (
            {
                "proof": False,
                "authorization": False,
                "noteEncryption": False,
                "audit": True,
            },
            "contains unsupported keys",
        ),
        (
            {"proof": False, "authorization": False, "noteEncryption": "no"},
            "pq_layers.note_encryption' must be a boolean",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_bad_pq_layers(
    monkeypatch,
    pq_layers,
    message,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(pqLayers=pq_layers)]),
    )

    with pytest.raises(RuntimeError, match=message):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "pq_layers",
    [
        {"proof": False, "authorization": True, "noteEncryption": True},
        {"proof": True, "authorization": False, "noteEncryption": True},
        {"proof": True, "authorization": True, "noteEncryption": False},
        {"proof": False, "authorization": False, "noteEncryption": False},
    ],
)
def test_privacy_catalog_loader_rejects_partial_post_quantum_claims(
    monkeypatch,
    pq_layers,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    coveredCriteria=["post_quantum"],
                    pqLayers=pq_layers,
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'covered_criteria' item 'post_quantum' "
            "requires all pq_layers to be true"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "covered_criteria",
    [
        [],
        ["hide_amount"],
        ["hide_amount", "hide_sender"],
    ],
)
def test_privacy_catalog_loader_rejects_fully_pq_layers_without_post_quantum(
    monkeypatch,
    covered_criteria,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    coveredCriteria=covered_criteria,
                    pqLayers={
                        "proof": True,
                        "authorization": True,
                        "noteEncryption": True,
                    },
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'pq_layers' with all layers true requires "
            "covered_criteria item 'post_quantum'"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "missing_url",
    [
        "https://csrc.nist.gov/pubs/fips/203/final",
        "https://csrc.nist.gov/pubs/fips/204/final",
        "https://csrc.nist.gov/pubs/fips/205/final",
    ],
)
def test_privacy_catalog_loader_rejects_post_quantum_rows_without_nist_fips_sources(
    monkeypatch,
    missing_url,
) -> None:
    source_references = [
        {"label": f"FIPS {index}", "url": url}
        for index, url in enumerate(
            sorted(privacy_catalog._POST_QUANTUM_REQUIRED_SOURCE_URLS),
            start=1,
        )
        if url != missing_url
    ]
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    coveredCriteria=["post_quantum"],
                    pqLayers={
                        "proof": True,
                        "authorization": True,
                        "noteEncryption": True,
                    },
                    sourceReferences=source_references,
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "source_references' must include NIST FIPS 203, "
            "FIPS 204, and FIPS 205"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("planned_sdk_entrypoints", "missing_fragment"),
    [
        (
            ["buildPqMaspStarkTransferProofV0", "encapsulateMlKem"],
            "MlDsa",
        ),
        (
            [
                "buildPqMaspStarkTransferProofV0",
                "generateNotMlDsaKeyPair",
                "encapsulateMlKem",
            ],
            "MlDsa",
        ),
        (
            ["buildPqMaspStarkTransferProofV0", "generateMlDsaKeyPair"],
            "MlKem",
        ),
        (
            [
                "buildPqMaspStarkTransferProofV0",
                "generateMlDsaKeyPair",
                "encapsulateNotMlKem",
            ],
            "MlKem",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_post_quantum_rows_without_planned_pq_primitive_entrypoints(
    monkeypatch,
    planned_sdk_entrypoints,
    missing_fragment,
) -> None:
    source_references = [
        {"label": f"FIPS {index}", "url": url}
        for index, url in enumerate(
            sorted(privacy_catalog._POST_QUANTUM_REQUIRED_SOURCE_URLS),
            start=1,
        )
    ]
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    coveredCriteria=["post_quantum"],
                    pqLayers={
                        "proof": True,
                        "authorization": True,
                        "noteEncryption": True,
                    },
                    sourceReferences=source_references,
                    plannedSdkEntrypoints=planned_sdk_entrypoints,
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "sdk_entrypoints' or 'planned_sdk_entrypoints' must include "
            "ML-DSA authorization and ML-KEM note-encryption SDK entrypoints"
        ),
    ) as error:
        privacy_catalog._load_descriptors()

    assert missing_fragment in str(error.value)


@pytest.mark.parametrize(
    ("patch", "field", "missing_token"),
    [
        (
            {"securityNotes": ["ML-DSA domains require audit"]},
            "security_notes",
            "ML-KEM",
        ),
        (
            {
                "securityNotes": [
                    "notML-DSA and notML-KEM primitive domains require audit",
                ]
            },
            "security_notes",
            "ML-DSA",
        ),
        (
            {
                "securityNotes": [
                    "not ML-DSA and not ML-KEM primitive domains require audit",
                ]
            },
            "security_notes",
            "ML-DSA",
        ),
        (
            {"failureModes": ["ML-KEM domain mismatch"]},
            "failure_modes",
            "ML-DSA",
        ),
        (
            {"failureModes": ["notML-DSA or notML-KEM domain mismatch"]},
            "failure_modes",
            "ML-DSA",
        ),
        (
            {"failureModes": ["not ML-DSA or not ML-KEM domain mismatch"]},
            "failure_modes",
            "ML-DSA",
        ),
        (
            {"requiredState": ["PQ nullifier set"]},
            "required_state",
            "ML-KEM",
        ),
        (
            {"requiredState": ["notML-KEM encrypted note payload store"]},
            "required_state",
            "ML-KEM",
        ),
        (
            {"requiredState": ["not ML-KEM encrypted note payload store"]},
            "required_state",
            "ML-KEM",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_post_quantum_rows_without_primitive_metadata(
    monkeypatch,
    patch,
    field,
    missing_token,
) -> None:
    source_references = [
        {"label": f"FIPS {index}", "url": url}
        for index, url in enumerate(
            sorted(privacy_catalog._POST_QUANTUM_REQUIRED_SOURCE_URLS),
            start=1,
        )
    ]
    descriptor = _raw_descriptor(
        coveredCriteria=["post_quantum"],
        pqLayers={
            "proof": True,
            "authorization": True,
            "noteEncryption": True,
        },
        sourceReferences=source_references,
        plannedSdkEntrypoints=[
            "buildPqMaspStarkTransferProofV0",
            "generateMlDsaKeyPair",
            "encapsulateMlKem",
        ],
        securityNotes=["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes=["ML-DSA or ML-KEM domain mismatch"],
        requiredState=["ML-KEM encrypted note payload store"],
    )
    descriptor.update(patch)
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=rf"field '{field}' must include post-quantum",
    ) as error:
        privacy_catalog._load_descriptors()

    assert missing_token in str(error.value)


@pytest.mark.parametrize(
    ("field", "value", "canonical_field"),
    [
        ("status", "available", "status"),
        ("unavailable_reason", None, "unavailable_reason"),
        ("unavailableReason", None, "unavailable_reason"),
        ("hidden_features", ["hide_sender"], "hidden_features"),
        ("hiddenFeatures", ["hide_sender"], "hidden_features"),
        ("requirements", ["chain verifier"], "requirements"),
        ("limitations", ["none"], "limitations"),
        (
            "verifier_key_metadata",
            {"proof_family": "fake"},
            "verifier_key_metadata",
        ),
        (
            "verifierKeyMetadata",
            {"proofFamily": "fake"},
            "verifier_key_metadata",
        ),
        ("backend_family", "fake-backend", "backend_family"),
        ("backendFamily", "fake-backend", "backend_family"),
        ("production_ready", True, "production_ready"),
        ("productionReady", True, "production_ready"),
        (
            "production_gate",
            {"ready": True},
            "production_gate",
        ),
        (
            "productionGate",
            {"ready": True},
            "production_gate",
        ),
    ],
)
def test_privacy_catalog_loader_rejects_derived_boi_availability_fields(
    monkeypatch,
    field,
    value,
    canonical_field,
) -> None:
    descriptor = _raw_descriptor(**{field: value})
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([descriptor]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            rf"field {canonical_field!r} is derived and must not be supplied"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_production_gate_remains_fail_closed_for_catalog_claims(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="production-hardened",
                    proofFamily="shape-proof",
                    publicInputsSchema="root,domain_separator",
                    verifierKeyId="shape_verifier_v0",
                    sdkEntrypoints=["buildProductionProof"],
                    plannedSdkEntrypoints=[],
                    recommendedFor=["production proof validation"],
                    chainRequirements=["production verifier key registry"],
                    securityNotes=[
                        "Review production proof constraints.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "production verifier key registry",
                        "wallet production witness store",
                    ],
                    failureModes=[
                        "production proof rejected",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register production verifier key"],
                    executionSteps=["Build production proof"],
                    sourceReferences=[
                        {
                            "label": "Production proof protocol source",
                            "url": "https://zips.z.cash/protocol/protocol.pdf",
                        }
                    ],
                )
            ]
        ),
    )

    [descriptor] = privacy_catalog._load_descriptors()

    gate = descriptor["production_gate"]
    assert descriptor["production_ready"] is False
    assert gate["ready"] is False
    assert list(gate["gates"].items()) == _expected_production_gate_items()
    assert gate["required_gates"] == _expected_required_production_gate_keys(
        descriptor["id"]
    )
    assert "implementation stage is not production-hardened" not in gate["missing"]
    assert "dev fixture entrypoints are not production entrypoints" not in gate[
        "missing"
    ]
    assert gate["missing"] == _expected_production_gate_missing(gate)
    assert "Iroha production allowlist is not enabled for this audited row" in gate[
        "missing"
    ]


def test_privacy_catalog_accepts_internal_review_evidence_for_all_rows() -> None:
    chain_id = "boi-localnet-4p"
    source_descriptors = get_privacy_algorithm_descriptors()
    manifest = _privacy_production_test_manifest(
        source_descriptors,
        chain_id=chain_id,
    )

    descriptors = get_privacy_algorithm_descriptors(manifest, chain_id=chain_id)
    by_id = {descriptor["id"]: descriptor for descriptor in descriptors}

    assert len(descriptors) == 21
    assert all(descriptor["production_ready"] is True for descriptor in descriptors)
    assert all(
        descriptor["production_gate"]["ready"] is True
        for descriptor in descriptors
    )
    for source_descriptor in source_descriptors:
        algorithm_id = source_descriptor["id"]
        descriptor = by_id[algorithm_id]
        expected_entrypoints = _privacy_production_test_entrypoints(source_descriptor)
        assert descriptor["implementation_stage"] == "production-hardened"
        assert descriptor["status"] == "production-ready"
        assert descriptor["planned_sdk_entrypoints"] == []
        assert descriptor["sdk_entrypoints"] == expected_entrypoints
        assert descriptor["sdk_exports"] == _privacy_production_test_sdk_exports(
            expected_entrypoints
        )
        assert all(
            not privacy_catalog._entrypoint_is_dev_fixture(entrypoint)
            and not privacy_catalog._entrypoint_is_local_verifier(entrypoint)
            for entrypoint in descriptor["sdk_entrypoints"]
        )
        gate = descriptor["production_gate"]
        assert gate["missing"] == []
        assert gate["required_gates"] == _expected_required_production_gate_keys(
            algorithm_id
        )
        for key in gate["required_gates"]:
            assert gate["gates"][key] is True
        if algorithm_id == "transparent-transfer":
            for key in privacy_catalog.TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS:
                assert gate["gates"][key] is False
        assert gate["chain_id"] == chain_id
        assert gate["reviewer_identity"] == "crypto-reviewer@internal.example"
        assert gate["localnet_acceptance"]["peer_count"] == 4
        assert gate["localnet_acceptance"]["peer_ids"] == [
            "boi-privacy-peer-1@localnet",
            "boi-privacy-peer-2@localnet",
            "boi-privacy-peer-3@localnet",
            "boi-privacy-peer-4@localnet",
        ]
        assert gate["localnet_acceptance"]["chain_id"] == chain_id
        assert gate["localnet_acceptance"]["smoke_tx_hash"].startswith("sha256:")
        assert gate["localnet_acceptance"]["replay_rejected"] is True
        assert gate["localnet_acceptance"]["replay_rejection_hash"].startswith(
            "sha256:"
        )
        assert gate["localnet_acceptance"]["restart_replay_rejected"] is True
        assert gate["localnet_acceptance"]["lifecycle_passed"] is True
        for lifecycle_key in (
            "lifecycle_shield_tx_hash",
            "lifecycle_hop_proof_hash",
            "lifecycle_recursive_init_hash",
            "lifecycle_recursive_init_verify_hash",
            "lifecycle_recursive_append_hash",
            "lifecycle_recursive_append_verify_hash",
            "lifecycle_unshield_proof_hash",
            "lifecycle_redeem_tx_hash",
        ):
            assert gate["localnet_acceptance"][lifecycle_key].startswith("sha256:")
        assert gate["audit_references"][0]["uri"].startswith("sha256:")
        assert gate["sdk_exports"] == descriptor["sdk_exports"]
        assert gate["review_scope"]["algorithm_id"] == algorithm_id
        assert gate["review_scope"]["sdk_entrypoints"] == expected_entrypoints
        assert gate["review_scope"]["fuzz_artifact_hash"] == gate["fuzz_results"][
            "artifact"
        ]["uri"]
        assert gate["review_scope"]["performance_artifact_hash"] == gate[
            "performance_results"
        ]["artifact"]["uri"]

    zk_ace = get_privacy_algorithm_descriptor(
        "zk-ace-pq-authorization-v0",
        manifest,
        chain_id=chain_id,
    )
    assert zk_ace is not None
    assert zk_ace["production_ready"] is True
    capabilities = privacy_capabilities(
        production_evidence=manifest,
        chain_id=chain_id,
    )
    assert all(
        descriptor["production_ready"] is True
        for descriptor in capabilities["privacy_algorithms"]
    )


@pytest.mark.parametrize(
    "mutator",
    [
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].pop("signature"),
            id="unsigned-review-artifact",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"signature": "minisign:reviewer-placeholder"}
            ),
            id="malformed-review-artifact-signature",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"signature": f"ed25519:{'0' * 128}"}
            ),
            id="zero-review-artifact-signature",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"signature": f"ed25519:{'a' * 128}"}
            ),
            id="repeated-review-artifact-signature",
        ),
        pytest.param(
            lambda row, _descriptor: row.update(
                {"reviewer_identity": "reviewer-placeholder@internal.example"}
            ),
            id="placeholder-reviewer-identity",
        ),
        pytest.param(
            lambda row, _descriptor: row.update(
                {"reviewer_identity": "mock-reviewer@internal.example"}
            ),
            id="mock-reviewer-identity",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"label": "review artifact placeholder"}
            ),
            id="placeholder-review-artifact-label",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"label": "Mock review artifact"}
            ),
            id="mock-review-artifact-label",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": "https://audit.example/review.pdf"}
            ),
            id="non-hash-addressed-review-artifact",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"sha256:{'A' * 64}"}
            ),
            id="uppercase-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"sha256:{'0' * 64}"}
            ),
            id="zero-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"urn:sha256:{'0' * 64}"}
            ),
            id="zero-urn-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"hash://sha256/{'0' * 64}"}
            ),
            id="zero-hash-url-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"sha256:{'a' * 64}"}
            ),
            id="repeated-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"urn:sha256:{'b' * 64}"}
            ),
            id="repeated-urn-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_artifact"].update(
                {"uri": f"hash://sha256/{'c' * 64}"}
            ),
            id="repeated-hash-url-review-artifact-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_entrypoints"].append(
                "buildShadowDevProofFixture"
            ),
            id="dev-fixture-sdk-entrypoint",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_entrypoints"].append(
                "verifyShadowProofLocally"
            ),
            id="local-only-verifier-entrypoint",
        ),
        pytest.param(
            lambda row, _descriptor: row.pop("sdk_exports"),
            id="missing-sdk-exports",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_exports"]["python"].append(
                "buildShadowDevProofFixture"
            ),
            id="dev-fixture-sdk-export",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_exports"].pop("ffi"),
            id="missing-ffi-sdk-export",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_exports"]["swift"].__setitem__(
                0,
                "buildDifferentAuditedProof",
            )
            if row["sdk_exports"]["swift"]
            else row["sdk_exports"]["swift"].append("buildDifferentAuditedProof"),
            id="stale-swift-sdk-export",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_parity_artifacts"]["golden_vectors"].pop(
                "ffi"
            ),
            id="missing-ffi-sdk-parity-artifact",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_parity_artifacts"]["types"]["swift"].update(
                {"label": "Mock Swift types SDK parity artifact"}
            ),
            id="mock-sdk-parity-artifact",
        ),
        pytest.param(
            lambda row, _descriptor: row["sdk_parity_artifacts"]["types"]["swift"].update(
                {"label": "Swift types SDK parity artifact placeholder"}
            ),
            id="placeholder-sdk-parity-artifact",
        ),
        pytest.param(
            lambda row, _descriptor: row.update({"verifier_key_id": "wrong_verifier_key"}),
            id="wrong-verifier-key",
        ),
        pytest.param(
            lambda row, _descriptor: row.update({"public_inputs_schema": "mutated_schema"}),
            id="wrong-public-input-schema",
        ),
        pytest.param(
            lambda row, _descriptor: row.pop("review_scope"),
            id="missing-review-scope",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_scope"].update(
                {"algorithm_id": "other-algorithm"}
            ),
            id="stale-review-scope-algorithm",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_scope"]["sdk_entrypoints"].append(
                "buildShadowProof"
            ),
            id="stale-review-scope-sdk-entrypoints",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_scope"].update(
                {"fuzz_artifact_hash": f"sha256:{'b' * 64}"}
            ),
            id="stale-review-scope-fuzz-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["review_scope"].update(
                {"localnet_run_id": "boi-localnet-4peer-run-other"}
            ),
            id="stale-review-scope-localnet",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update({"peer_count": 3}),
            id="weak-localnet-peer-count",
        ),
        pytest.param(
            lambda row, _descriptor: row.update(
                {"localnet_run_id": "mock-boi-localnet-4peer-run-2026-06-09"}
            ),
            id="mock-localnet-run-id",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"]["peer_ids"].__setitem__(
                3,
                row["localnet_acceptance"]["peer_ids"][0],
            ),
            id="duplicate-localnet-peer-id",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"chain_id": "boi-localnet-other-4p"}
            ),
            id="wrong-localnet-chain",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"smoke_passed": False}
            ),
            id="missing-localnet-smoke",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"smoke_tx_hash": "sha256:not-a-hex-digest"}
            ),
            id="bad-localnet-smoke-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"replay_rejected": False}
            ),
            id="missing-replay-rejection",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {
                    "replay_rejection_hash": row["localnet_acceptance"][
                        "smoke_tx_hash"
                    ]
                }
            ),
            id="reused-localnet-replay-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"restart_persistence_checked": False}
            ),
            id="missing-restart-persistence",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"restart_replay_rejected": False}
            ),
            id="missing-restart-replay-rejection",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"state_recovery_passed": False}
            ),
            id="missing-state-recovery",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].pop(
                "lifecycle_redeem_tx_hash"
            ),
            id="missing-localnet-lifecycle-redeem",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_passed": False}
            ),
            id="missing-localnet-lifecycle",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_append_hash": "sha256:not-a-hex-digest"}
            ),
            id="bad-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"sha256:{'0' * 64}"}
            ),
            id="zero-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"urn:sha256:{'0' * 64}"}
            ),
            id="zero-urn-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"hash://sha256/{'0' * 64}"}
            ),
            id="zero-hash-url-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"sha256:{'a' * 64}"}
            ),
            id="repeated-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"urn:sha256:{'b' * 64}"}
            ),
            id="repeated-urn-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {"lifecycle_recursive_init_hash": f"hash://sha256/{'c' * 64}"}
            ),
            id="repeated-hash-url-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, _descriptor: row["localnet_acceptance"].update(
                {
                    "lifecycle_redeem_tx_hash": row["localnet_acceptance"][
                        "lifecycle_unshield_proof_hash"
                    ]
                }
            ),
            id="reused-localnet-lifecycle-hash",
        ),
        pytest.param(
            lambda row, descriptor: row["gate_evidence"].pop(
                _expected_required_production_gate_keys(descriptor["id"])[0]
            ),
            id="missing-gate-evidence",
        ),
        pytest.param(
            lambda row, descriptor: row["gate_evidence"][
                _expected_required_production_gate_keys(descriptor["id"])[0]
            ][0].update({"label": "production gate artifact placeholder"}),
            id="placeholder-production-gate-artifact-label",
        ),
    ],
)
def test_privacy_catalog_rejects_invalid_internal_review_evidence(mutator) -> None:
    chain_id = "boi-localnet-4p"
    for target in get_privacy_algorithm_descriptors():
        row = _privacy_production_test_row(
            target,
            chain_id=chain_id,
            localnet_run_id="boi-localnet-4peer-run-2026-06-09",
        )
        mutator(row, target)
        manifest = {
            "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
            "rows": [row],
        }

        descriptor = get_privacy_algorithm_descriptor(
            str(target["id"]),
            manifest,
            chain_id=chain_id,
        )

        assert descriptor is not None
        assert descriptor["production_ready"] is False
        assert descriptor["production_gate"]["ready"] is False
        assert (
            "Iroha production allowlist is not enabled for this audited row"
            in descriptor["production_gate"]["missing"]
        )


def test_privacy_catalog_rejects_duplicate_internal_review_evidence_rows() -> None:
    chain_id = "boi-localnet-4p"
    target = get_privacy_algorithm_descriptor("zk-ace-pq-authorization-v0")
    assert target is not None
    row = _privacy_production_test_row(
        target,
        chain_id=chain_id,
        localnet_run_id="boi-localnet-4peer-run-2026-06-09",
    )
    valid_manifest = {
        "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
        "rows": [row],
    }
    valid_descriptor = get_privacy_algorithm_descriptor(
        str(target["id"]),
        valid_manifest,
        chain_id=chain_id,
    )
    assert valid_descriptor is not None
    assert valid_descriptor["production_gate"]["audit_references"][0][
        "uri"
    ].startswith("sha256:")

    duplicate_descriptor = get_privacy_algorithm_descriptor(
        str(target["id"]),
        {
            "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
            "rows": [row, dict(row)],
        },
        chain_id=chain_id,
    )
    assert duplicate_descriptor is not None
    assert duplicate_descriptor["production_ready"] is False
    assert duplicate_descriptor["production_gate"]["audit_references"] == []


def test_privacy_catalog_rejects_chain_mismatched_internal_review_evidence() -> None:
    for target in get_privacy_algorithm_descriptors():
        row = _privacy_production_test_row(
            target,
            chain_id="boi-localnet-4p",
            localnet_run_id="boi-localnet-4peer-run-2026-06-09",
        )
        manifest = {
            "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
            "rows": [row],
        }

        descriptor = get_privacy_algorithm_descriptor(
            str(target["id"]),
            manifest,
            chain_id="wrong-chain",
        )

        assert descriptor is not None
        assert descriptor["production_ready"] is False
        assert descriptor["production_gate"]["ready"] is False


def test_privacy_catalog_rejects_mock_chain_internal_review_evidence() -> None:
    chain_id = "mock-privacy-4peer-chain"
    for target in get_privacy_algorithm_descriptors():
        row = _privacy_production_test_row(
            target,
            chain_id=chain_id,
            localnet_run_id="boi-localnet-4peer-run-2026-06-09",
        )
        manifest = {
            "version": privacy_catalog.PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION,
            "rows": [row],
        }

        descriptor = get_privacy_algorithm_descriptor(
            str(target["id"]),
            manifest,
            chain_id=chain_id,
        )

        assert descriptor is not None
        assert descriptor["production_ready"] is False
        assert descriptor["production_gate"]["ready"] is False


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildShapeDevProofFixture",
        "buildFutureMockProof",
        "buildFutureMockProofV2",
        "buildFutureDev.Proof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_production_stage_fixture_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="production-hardened",
                    sdkEntrypoints=[entrypoint],
                    plannedSdkEntrypoints=[],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "production-hardened targets cannot advertise "
            "fixture/mock SDK entrypoints"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildProofFixture",
        "buildMockProof",
        "buildMockProofV2",
        "buildProof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_non_explicit_sdk_fixture_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(sdkEntrypoints=[entrypoint])]),
    )

    with pytest.raises(
        RuntimeError,
        match="fixture/mock SDK entrypoints must use explicit DevFixture names",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_accepts_explicit_dev_fixture_sdk_entrypoints(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[
                        "buildShapeDevProofFixture",
                        "verifyShapeProofLocally",
                    ],
                    securityNotes=[
                        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
                    ],
                    plannedSdkEntrypoints=["buildShapeProductionProof"],
                )
            ]
        ),
    )

    [descriptor] = privacy_catalog._load_descriptors()

    assert descriptor["sdk_entrypoints"] == [
        "buildShapeDevProofFixture",
        "verifyShapeProofLocally",
    ]
    assert descriptor["planned_sdk_entrypoints"] == ["buildShapeProductionProof"]


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildShapeDevProofFixture",
        "buildShapeDevFixture",
        "buildShapeDev.Proof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_dev_fixture_entrypoints_without_local_verifier(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(sdkEntrypoints=[entrypoint])]),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable DevFixture SDK entrypoints must be paired "
            "with a local verifier entrypoint"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProofNotLocalVerifier",
        "verifyShapeProofNoLocal",
        "verifyShapeProofNonLocalOnly",
        "verifyShapeProofNotLocally",
    ],
)
def test_privacy_catalog_loader_rejects_dev_fixture_local_verifier_near_misses(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[
                        "buildShapeDevProofFixture",
                        entrypoint,
                    ],
                    securityNotes=[
                        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
                    ],
                    plannedSdkEntrypoints=["buildShapeProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable DevFixture SDK entrypoints must be paired "
            "with a local verifier entrypoint"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProofLocally",
        "verifyShapeProofLocal",
        "verifyShapeProofLocalVerifier",
        "Iroha.Privacy.verifyShapeProofLocally",
        "Iroha.Privacy.verifyShapeProofLocalVerifier",
    ],
)
def test_privacy_catalog_loader_rejects_local_verifier_entrypoints_without_dev_fixture(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="sdk-builder",
                    proofFamily="shape-proof",
                    publicInputsSchema="root,domain_separator",
                    verifierKeyId="shape_verifier_v0",
                    recommendedFor=["shape validation"],
                    chainRequirements=["shape verifier registry"],
                    securityNotes=["Review shape proof constraints"],
                    requiredState=["shape verifier registry"],
                    failureModes=["shape proof rejected"],
                    setupSteps=["Register shape verifier"],
                    executionSteps=["Build shape proof"],
                    sourceReferences=[
                        {
                            "label": "Shape source",
                            "url": "https://zips.z.cash/zip-0224",
                        }
                    ],
                    sdkEntrypoints=[entrypoint],
                    plannedSdkEntrypoints=["buildShapeProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable local-only verifier SDK entrypoints must be paired "
            "with an explicit DevFixture entrypoint"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildShapeNotDevFixture",
        "buildShapeNoDevFixture",
        "buildShapeNonDevFixture",
        "buildShapeWithoutDevFixture",
        "buildShapeNotDevProofFixture",
        "buildShapeNoDevProofFixture",
        "buildShapeNonDevProofFixture",
        "buildShapeWithoutDevProofFixture",
    ],
)
def test_privacy_catalog_loader_rejects_local_verifier_with_negated_dev_fixture_alias(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="validator-scaffold-as-of-2026-05",
                    sdkEntrypoints=[entrypoint, "verifyShapeProofLocally"],
                    securityNotes=[
                        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
                    ],
                    plannedSdkEntrypoints=["buildShapeProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="fixture/mock SDK entrypoints must use explicit DevFixture names",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("entrypoint", "expected"),
    [
        ("buildShapeDevFixture", True),
        ("buildShapeDevFixtureV1", True),
        ("buildShapeDevProofFixture", True),
        ("buildShapeDevProofFixtureV1", True),
        ("Iroha.Privacy.buildShapeDevProofFixture", True),
        ("buildShapeDev.Proof.Fixture", True),
        ("Iroha.Privacy.buildShapeDev.Proof.Fixture", True),
        ("buildShapeNotDevFixture", False),
        ("buildShapeNoDevFixture", False),
        ("buildShapeNonDevFixture", False),
        ("buildShapeWithoutDevFixture", False),
        ("buildShapeNotDev.Proof.Fixture", False),
        ("buildShapeNoDev.Proof.Fixture", False),
        ("buildShapeNonDev.Proof.Fixture", False),
        ("buildShapeWithoutDev.Proof.Fixture", False),
        ("buildShapeNotDevProofFixture", False),
        ("buildShapeNoDevProofFixture", False),
        ("buildShapeNonDevProofFixture", False),
        ("buildShapeWithoutDevProofFixture", False),
        ("buildShapeDevFixtureFactory", False),
        ("buildShapeDevelopmentFixture", False),
    ],
)
def test_privacy_catalog_explicit_dev_fixture_classifier_requires_non_negated_terminal_evidence(
    entrypoint,
    expected,
) -> None:
    assert privacy_catalog._entrypoint_is_explicit_dev_fixture(entrypoint) is expected


@pytest.mark.parametrize(
    ("entrypoint", "expected"),
    [
        ("buildShapeTransferInstruction", True),
        ("Iroha.Privacy.buildShapeTransferInstruction", True),
        ("buildShapeInstructionV1", True),
        ("buildShapeAuthorizedTransaction", True),
        ("buildShapeTransactionV1", True),
        ("buildSubmitShapeProof", True),
        ("buildSubmitShapeProofV1", True),
        ("buildShapeNoInstruction", False),
        ("buildShapeNotInstruction", False),
        ("buildShapeNonInstruction", False),
        ("buildShapeWithoutInstruction", False),
        ("buildShapeNoTransaction", False),
        ("buildShapeNotTransaction", False),
        ("buildMidenStarkTransactionProofV1", False),
        ("buildNoSubmitShapeProof", False),
        ("buildNotSubmitShapeProof", False),
        ("buildNonSubmitShapeProof", False),
        ("buildWithoutSubmitShapeProof", False),
        ("buildShapeInstructionalProof", False),
        ("buildShapeTransactionalProof", False),
        ("buildShapeSubmitterProof", False),
    ],
)
def test_privacy_catalog_planned_ledger_mutation_classifier_requires_non_negated_evidence(
    entrypoint,
    expected,
) -> None:
    assert privacy_catalog._entrypoint_is_planned_ledger_mutation(entrypoint) is expected


@pytest.mark.parametrize(
    ("entrypoint", "expected"),
    [
        ("verifyShapeProofLocally", True),
        ("verifyShapeProofLocal", True),
        ("verifyShapeProofLocalVerifier", True),
        ("Iroha.Privacy.verifyShapeProofLocally", True),
        ("Iroha.Privacy.verifyShapeProofLocalVerifier", True),
        ("verifyShapeProofNotLocalVerifier", False),
        ("verifyShapeProofNoLocal", False),
        ("verifyShapeProofNonLocalOnly", False),
        ("verifyShapeProofNotLocally", False),
    ],
)
def test_privacy_catalog_local_verifier_classifier_rejects_negated_near_misses(
    entrypoint,
    expected,
) -> None:
    assert privacy_catalog._entrypoint_is_local_verifier(entrypoint) is expected


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildShapeDevProofFixture",
        "buildShapeDev.Proof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_chain_executable_dev_fixture_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="chain-executable",
                    sdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="chain-executable targets cannot advertise fixture/mock SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProofLocally",
        "verifyShapeProofLocal",
        "Iroha.Privacy.verifyShapeProofLocally",
        "Iroha.Privacy.verifyShapeProofLocalVerifier",
    ],
)
def test_privacy_catalog_loader_rejects_chain_executable_local_verifier_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="chain-executable",
                    sdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="chain-executable targets cannot advertise local-only verifier SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("field", "entrypoints"),
    [
        ("sdkEntrypoints", ["buildShapeInstruction"]),
        ("plannedSdkEntrypoints", ["buildShapeInstruction"]),
        ("sdkEntrypoints", ["Iroha.Privacy.buildShapeInstruction"]),
        ("plannedSdkEntrypoints", ["Iroha.Privacy.buildShapeInstruction"]),
    ],
)
def test_privacy_catalog_loader_rejects_component_instruction_entrypoints(
    monkeypatch,
    field,
    entrypoints,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="component",
                    **{field: entrypoints},
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="component targets cannot advertise instruction SDK entrypoint",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_without_protection_metadata(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=[
                        "buildShapeTransferInstruction",
                        "buildShapeAuthorizedTransaction",
                    ],
                    requiredState=["shape verifier registry"],
                    failureModes=["shape verifier mismatch"],
                    chainRequirements=["shape verifier registry"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require replay, "
            "nullifier, revocation, or link-tag protection metadata"
        ),
    ) as error:
        privacy_catalog._load_descriptors()

    assert "buildShapeTransferInstruction" in str(error.value)
    assert "buildShapeAuthorizedTransaction" in str(error.value)


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_with_concatenated_replay_token(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-negative-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization notreplay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "stale notreplay state",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require replay, "
            "nullifier, revocation, or link-tag protection metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_with_negated_bounded_replay_token(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization not replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "not nullifier replay state",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require replay, "
            "nullifier, revocation, or link-tag protection metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_without_typed_admission_metadata(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage=None,
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=[
                        "buildShapeTransferInstruction",
                        "buildShapeAuthorizedTransaction",
                    ],
                    requiredState=["shape replay guard"],
                    failureModes=["shape replay"],
                    chainRequirements=["shape verifier registry"],
                    setupSteps=["Register shape verifier."],
                    executionSteps=["Submit shape proof."],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require explicit typed "
            "chain admission metadata"
        ),
    ) as error:
        privacy_catalog._load_descriptors()

    assert "buildShapeTransferInstruction" in str(error.value)
    assert "buildShapeAuthorizedTransaction" in str(error.value)


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_with_concatenated_typed_admission_tokens(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage=None,
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=[
                        "buildShapeTransferInstruction",
                        "buildShapeAuthorizedTransaction",
                    ],
                    requiredState=["shape replay guard"],
                    failureModes=["shape replay"],
                    chainRequirements=["shape verifier registry"],
                    setupSteps=["Register shape verifier."],
                    executionSteps=["Submit untyped shape noninstruction admission."],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require explicit typed "
            "chain admission metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_with_concatenated_zk_namespace(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage=None,
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=[
                        "buildShapeTransferInstruction",
                        "buildShapeAuthorizedTransaction",
                    ],
                    requiredState=["shape replay guard"],
                    failureModes=["shape replay"],
                    chainRequirements=["shape verifier registry"],
                    setupSteps=["Register shape verifier."],
                    executionSteps=["Submit notzk::ShapeTransfer admission."],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require explicit typed "
            "chain admission metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_planned_ledger_mutation_with_negated_bounded_typed_admission_tokens(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage=None,
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=[
                        "buildShapeTransferInstruction",
                        "buildShapeAuthorizedTransaction",
                    ],
                    requiredState=["shape replay guard"],
                    failureModes=["shape replay"],
                    chainRequirements=["shape verifier registry"],
                    setupSteps=["Register shape verifier."],
                    executionSteps=["Submit not typed shape no instruction admission."],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require explicit typed "
            "chain admission metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ("typed zk::SubmitShapeTransfer instruction", True),
        ("zk::SubmitShapeTransfer", True),
        ("typed notzk::SubmitShapeTransfer instruction", False),
        ("typed not_zk::SubmitShapeTransfer instruction", False),
    ],
)
def test_privacy_catalog_zk_namespace_metadata_token_uses_prefix_boundary(
    value,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_metadata_token(value.lower(), "zk::")
        is expected
    )


@pytest.mark.parametrize(
    ("value", "token", "expected"),
    [
        ("typed zk::SubmitShapeTransfer instruction", "typed", True),
        ("typed zk::SubmitShapeTransfer instruction", "instruction", True),
        ("zk::SubmitShapeTransfer", "zk::", True),
        ("not typed shape instruction", "typed", False),
        ("typed shape no instruction admission", "instruction", False),
        ("not transaction admission", "transaction", False),
        ("not zk::SubmitShapeTransfer", "zk::", False),
        ("without zk::SubmitShapeTransfer", "zk::", False),
        ("typed notzk::SubmitShapeTransfer instruction", "zk::", False),
        ("typed not_zk::SubmitShapeTransfer instruction", "zk::", False),
    ],
)
def test_privacy_catalog_typed_admission_metadata_rejects_negated_bounded_tokens(
    value,
    token,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_typed_admission_token(
            value.lower(),
            token,
        )
        is expected
    )


def test_privacy_catalog_loader_rejects_stateful_ledger_mutation_without_restart_persistence(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=["authorization replay"],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require "
            "restart/persistence metadata for root, nullifier, revocation, "
            "or replay state"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_stateful_ledger_mutation_with_negated_bounded_restart_persistence(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Replay guard must not persist across restart.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "stale replay state",
                        "duplicate replay rejection",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "planned ledger-mutating SDK entrypoints require "
            "restart/persistence metadata for root, nullifier, revocation, "
            "or replay state"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_stateful_ledger_mutation_without_replay_failure_modes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=[
                        "Register zkAt verifier key and persist replay state.",
                    ],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction and update replay guard.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'failure_modes' must include stale-state and "
            "duplicate/replay rejection"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_stateful_ledger_mutation_with_negated_bounded_replay_failure_modes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=[
                        "zkAt verifier key registry",
                        "typed zk::ZkAtPolicyCommitment instruction admission",
                    ],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Replay guard must persist across restart.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "not stale replay state",
                        "no duplicate replay rejection",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=[
                        "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
                    ],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyCommitmentInstruction"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'failure_modes' must include stale-state and "
            "duplicate/replay rejection"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_flow_without_wallet_state(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier"],
                    securityNotes=["Policy proof review required."],
                    requiredState=["policy commitment registry"],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register policy verifier."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include wallet or witness state metadata "
            "for source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_wallet_state_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier"],
                    securityNotes=["Policy proof review required."],
                    requiredState=[
                        "policy commitment registry",
                        "notwallet policy notwitness store",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register policy verifier."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include wallet or witness state metadata "
            "for source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_wallet_state_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier"],
                    securityNotes=["Policy proof review required."],
                    requiredState=[
                        "policy commitment registry",
                        "not wallet policy store",
                        "no witness state",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register policy verifier."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include wallet or witness state metadata "
            "for source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_witness_privacy_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs stay notlocal and notexposed through SDK APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include wallet/witness privacy notes "
            "for source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_witness_privacy_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs are not local and no private input remains protected.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include wallet/witness privacy notes "
            "for source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_credential_flow_without_commitment_state(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="vega-existing-credential-zk-v0",
                    category="credential",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "Vega source",
                            "url": "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
                        }
                    ],
                    recommendedFor=["credential predicate proofs"],
                    chainRequirements=["credential predicate verifier"],
                    securityNotes=[
                        "Credential proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "credential issuer registry",
                        "wallet credential witness store",
                        "revocation policy",
                    ],
                    failureModes=[
                        "credential replay",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register credential verifier."],
                    executionSteps=["Build credential proof."],
                    proofFamily="existing-credential-zk",
                    publicInputsSchema="issuer_commitment,credential_schema",
                    verifierKeyId="vega_existing_credential_zk_v0",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildVegaCredentialPredicateProofV0"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include credential, identity, or admission "
            "commitment/accumulator state metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_credential_state_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="vega-existing-credential-zk-v0",
                    category="credential",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "Vega source",
                            "url": "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
                        }
                    ],
                    recommendedFor=["credential predicate proofs"],
                    chainRequirements=["credential predicate verifier"],
                    securityNotes=[
                        "Credential proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "credential issuer registry",
                        "wallet credential witness store",
                        "notcommitment predicate store",
                        "notaccumulator admission state",
                    ],
                    failureModes=[
                        "credential replay",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register credential verifier."],
                    executionSteps=["Build credential proof."],
                    proofFamily="existing-credential-zk",
                    publicInputsSchema="issuer_commitment,credential_schema",
                    verifierKeyId="vega_existing_credential_zk_v0",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildVegaCredentialPredicateProofV0"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include credential, identity, or admission "
            "commitment/accumulator state metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_credential_state_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="vega-existing-credential-zk-v0",
                    category="credential",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "Vega source",
                            "url": "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
                        }
                    ],
                    recommendedFor=["credential predicate proofs"],
                    chainRequirements=["credential predicate verifier"],
                    securityNotes=[
                        "Credential proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "credential issuer registry",
                        "wallet credential witness store",
                        "not commitment predicate store",
                        "without accumulator admission state",
                    ],
                    failureModes=[
                        "credential replay",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register credential verifier."],
                    executionSteps=["Build credential proof."],
                    proofFamily="existing-credential-zk",
                    publicInputsSchema="issuer_commitment,credential_schema",
                    verifierKeyId="vega_existing_credential_zk_v0",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildVegaCredentialPredicateProofV0"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "required_state' must include credential, identity, or admission "
            "commitment/accumulator state metadata"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_verifier_flow_without_key_record_metadata(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier"],
                    securityNotes=[
                        "Policy proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include verifier-key record metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_verifier_key_record_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["notverifier key registry"],
                    securityNotes=[
                        "Policy proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "notverifier-key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register notverifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include verifier-key record metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_verifier_key_record_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["not verifier key registry"],
                    securityNotes=[
                        "Policy proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "without verifier-key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register no verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include verifier-key record metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_verifier_flow_without_chain_domain_binding(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Policy proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,policy_hash",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include chain/domain binding metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_chain_domain_binding_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "notdomain-separation planning remains under review.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,notanchor",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include chain/domain binding metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_chain_domain_binding_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Policy proof is not domain separation evidence.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "without tx_digest binding",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key without anchor evidence."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,not_tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "must include chain/domain binding metadata for source-referenced "
            "verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "public_inputs_schema",
    [
        "policy_commitment,policy_hash",
        "policy_commitment,notanchor",
        "policy_commitment,not_tx_digest",
        "policy_commitment,no_chain_id",
        "policy_commitment,non_chain_tag",
        "policy_commitment,without_reference_block",
        "policy_commitment,policy_not_tx_digest",
        "policy_commitment,not_policy_tx_digest",
        "policy_commitment,no_policy_domain_separator",
        "policy_commitment,policy_without_reference_block",
        "policy_commitment,non_policy_rollup_state",
    ],
)
def test_privacy_catalog_loader_rejects_source_referenced_verifier_without_public_input_binding(
    monkeypatch,
    public_inputs_schema,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "domain separator mismatch",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema=public_inputs_schema,
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "field 'public_inputs_schema' must include chain/domain binding "
            "public input for source-referenced verifier entries"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("public_inputs_schema", "expected"),
    [
        ("policy_commitment,tx_digest", True),
        ("policy_commitment,policy_tx_digest", True),
        ("policy_commitment,tx_digest_v1", True),
        ("policy_commitment,domain_separator", True),
        ("policy_commitment,policy_domain_separator_hash", True),
        ("policy_commitment,anchor", True),
        ("policy_commitment,reference_block_height", True),
        ("policy_commitment,rollup_state_root", True),
        ("policy_commitment,not_tx_digest", False),
        ("policy_commitment,no_chain_id", False),
        ("policy_commitment,non_chain_tag", False),
        ("policy_commitment,without_reference_block", False),
        ("policy_commitment,not_anchor", False),
        ("policy_commitment,policy_not_tx_digest", False),
        ("policy_commitment,not_policy_tx_digest", False),
        ("policy_commitment,no_policy_domain_separator", False),
        ("policy_commitment,policy_without_reference_block", False),
        ("policy_commitment,non_policy_rollup_state", False),
        ("policy_commitment,anchorless", False),
    ],
)
def test_privacy_catalog_public_input_schema_chain_domain_binding_rejects_negated_fragments(
    public_inputs_schema,
    expected,
) -> None:
    assert (
        privacy_catalog._public_inputs_schema_has_chain_domain_binding(
            public_inputs_schema
        )
        is expected
    )


@pytest.mark.parametrize(
    ("value", "token", "expected"),
    [
        ("domain separation binds the verifier inputs", "domain separation", True),
        ("policy tx_digest binding is explicit", "tx_digest", True),
        ("reference-block finality is pinned", "reference-block", True),
        ("not domain separation evidence", "domain separation", False),
        ("without tx_digest binding", "tx_digest", False),
        ("no policy domain separator", "domain separator", False),
        ("non-domain-separated placeholder", "domain-separated", False),
        ("not_anchor placeholder", "anchor", False),
        ("not a domain separator", "domain separator", False),
    ],
)
def test_privacy_catalog_chain_domain_binding_metadata_rejects_negated_bounded_tokens(
    value,
    token,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_chain_domain_binding_token(value, token)
        is expected
    )


@pytest.mark.parametrize(
    ("value", "token", "expected"),
    [
        ("deterministic vectors are required", "deterministic vectors", True),
        ("negative/adversarial test cases are required", "negative/adversarial", True),
        ("replay/nullifier rejection tests are required", "replay/nullifier", True),
        ("parser/verifier fuzzing is required", "parser/verifier fuzzing", True),
        ("performance gates are required", "performance", True),
        ("internal cryptographic review is required", "review", True),
        ("not deterministic vectors", "deterministic vectors", False),
        ("no negative/adversarial test cases", "negative/adversarial", False),
        ("without replay/nullifier rejection tests", "replay/nullifier", False),
        ("not parser/verifier fuzzing", "parser/verifier fuzzing", False),
        ("no verifier fuzzing", "verifier fuzzing", False),
        ("not performance gates", "performance", False),
        ("without audit review", "audit", False),
    ],
)
def test_privacy_catalog_source_hardening_metadata_rejects_negated_bounded_tokens(
    value,
    token,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_source_hardening_token(value, token)
        is expected
    )


@pytest.mark.parametrize(
    ("value", "token", "expected"),
    [
        ("wallet witness store", "wallet", True),
        ("credential commitment registry", "commitment", True),
        ("accumulator state registry", "accumulator", True),
        ("verifier key registry", "verifier key", True),
        ("malformed proof bytes", "malformed proof", True),
        ("wrong verifier key", "wrong verifier key", True),
        ("public input mismatch", "public input mismatch", True),
        ("authorization replay guard", "replay", True),
        ("nullifier set must persist across restart", "nullifier", True),
        ("nullifier set must persist across restart", "persist", True),
        ("restart persistence metadata", "persist", True),
        ("persistent replay state", "persist", True),
        ("stale replay state", "stale", True),
        ("duplicate nullifier rejection", "duplicate", True),
        ("production readiness audit", "production", True),
        ("production readiness audit", "audit", True),
        ("ML-DSA and ML-KEM domains", "ML-DSA", True),
        ("ML-DSA and ML-KEM domains", "ML-KEM", True),
        ("not wallet state", "wallet", False),
        ("no witness store", "witness", False),
        ("non-wallet placeholder", "wallet", False),
        ("without commitment registry", "commitment", False),
        ("not accumulator state", "accumulator", False),
        ("not verifier key registry", "verifier key", False),
        ("without verifier-key registration", "verifier-key", False),
        ("not malformed proof bytes", "malformed proof", False),
        ("not wrong verifier key", "wrong verifier key", False),
        ("no public input mismatch", "public input mismatch", False),
        ("not replay guard", "replay", False),
        ("without nullifier persistence", "nullifier", False),
        ("without nullifier persistence", "persist", False),
        ("non-persistent replay state", "persist", False),
        ("not persist across restart", "persist", False),
        ("not stale replay state", "stale", False),
        ("no duplicate nullifier rejection", "duplicate", False),
        ("not production readiness audit", "production", False),
        ("no audit review", "audit", False),
        ("not ML-DSA domain", "ML-DSA", False),
        ("without ML-KEM state", "ML-KEM", False),
    ],
)
def test_privacy_catalog_affirmed_metadata_rejects_negated_bounded_state_tokens(
    value,
    token,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_affirmed_metadata_token(value, token)
        is expected
    )


@pytest.mark.parametrize(
    ("value", "token", "expected"),
    [
        ("wallet witness material stays local", "wallet", True),
        ("wallet witness material stays local", "local", True),
        ("private inputs must not be exposed", "not be exposed", True),
        ("plaintext must not leak", "must not leak", True),
        ("secrets never leave the wallet", "never leave", True),
        ("must not leak wallet note ownership", "wallet", True),
        ("must not expose wallet witness data", "wallet", True),
        ("never leave the wallet", "wallet", True),
        ("wallet witness material is not local", "local", False),
        ("no private input remains protected", "private input", False),
        ("without wallet witness custody", "wallet", False),
        ("not secret material", "secret", False),
    ],
)
def test_privacy_catalog_wallet_witness_privacy_preserves_exposure_negation(
    value,
    token,
    expected,
) -> None:
    assert (
        privacy_catalog._catalog_text_contains_wallet_witness_privacy_token(
            value,
            token,
        )
        is expected
    )


def test_privacy_catalog_loader_rejects_source_referenced_verifier_without_negative_failure_modes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=["authorization replay"],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "failure_modes' must include malformed-proof, wrong-verifier-key, "
            "and wrong-public-input rejection for source-referenced verifier "
            "entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_verifier_with_concatenated_negative_failure_modes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "notmalformed proof bytes",
                        "notwrong verifier key",
                        "notpublic input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "failure_modes' must include malformed-proof, wrong-verifier-key, "
            "and wrong-public-input rejection for source-referenced verifier "
            "entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_verifier_with_negated_bounded_failure_modes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "authorization replay guard",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "not malformed proof bytes",
                        "not wrong verifier key",
                        "no public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "failure_modes' must include malformed-proof, wrong-verifier-key, "
            "and wrong-public-input rejection for source-referenced verifier "
            "entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_flow_without_hardening_notes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Policy proof review required.",
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include deterministic vectors, "
            "negative/adversarial cases, replay/nullifier rejection tests, "
            "parser/verifier fuzzing, performance, and audit/review "
            "hardening gates for source-referenced entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_hardening_note_concatenated_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires notdeterministic vectors, notnegative/adversarial test cases, notreplay/nullifier rejection tests, notparser/verifier fuzzing, notperformance gates, and notaudit queue.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include deterministic vectors, "
            "negative/adversarial cases, replay/nullifier rejection tests, "
            "parser/verifier fuzzing, performance, and audit/review "
            "hardening gates for source-referenced entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_hardening_note_negated_bounded_false_positive(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
                        "Production hardening requires not deterministic vectors, no negative/adversarial test cases, without replay/nullifier rejection tests, not parser/verifier fuzzing, no verifier fuzzing, not performance gates, and without audit review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                        "zkAt verifier key registry",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include deterministic vectors, "
            "negative/adversarial cases, replay/nullifier rejection tests, "
            "parser/verifier fuzzing, performance, and audit/review "
            "hardening gates for source-referenced entries"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_source_referenced_flow_without_witness_privacy_notes(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    id="zkat-policy-private-auth-v1",
                    category="authorization",
                    implementationStage="sdk-builder",
                    sourceReferences=[
                        {
                            "label": "zkAt source",
                            "url": "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
                        }
                    ],
                    recommendedFor=["policy privacy"],
                    chainRequirements=["zkAt verifier key registry"],
                    securityNotes=[
                        "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                    ],
                    requiredState=[
                        "policy commitment registry",
                        "wallet policy witness store",
                    ],
                    failureModes=[
                        "policy-root substitution",
                        "malformed proof bytes",
                        "wrong verifier key",
                        "public input mismatch",
                    ],
                    setupSteps=["Register zkAt verifier key."],
                    executionSteps=["Build policy proof."],
                    proofFamily="zkat-policy-private-authenticator",
                    publicInputsSchema="policy_commitment,tx_digest",
                    verifierKeyId="zkat_policy_private_auth_v1",
                    sdkEntrypoints=[],
                    plannedSdkEntrypoints=["buildZkAtPolicyProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "security_notes' must include wallet/witness privacy notes for "
            "source-referenced privacy flows"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildShapeDevProofFixture",
        "buildShapeDev.Proof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_research_target_dev_fixture_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="research-target-as-of-2026-05",
                    sdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="research targets cannot advertise fixture/mock SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProofLocally",
        "verifyShapeProofLocal",
        "verifyShapeProofLocalVerifier",
        "Iroha.Privacy.verifyShapeProofLocally",
        "Iroha.Privacy.verifyShapeProofLocalVerifier",
    ],
)
def test_privacy_catalog_loader_rejects_research_target_local_verifier_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="research-target-as-of-2026-05",
                    sdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="research targets cannot advertise local-only verifier SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProof",
        "buildShapeProductionProof",
        "buildShapeProofEnvelope",
        "buildShapeProductionInstruction",
    ],
)
def test_privacy_catalog_loader_rejects_research_target_executable_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="research-target-as-of-2026-05",
                    sdkEntrypoints=[entrypoint],
                    plannedSdkEntrypoints=["buildShapeProductionProofV1"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="research targets cannot advertise executable SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "security_notes",
    [
        [],
        ["Review shape proof constraints"],
        ["The SDK dev fixture is deterministic only."],
        ["Production Shape proofs remain unavailable."],
        [
            "The SDK notdev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        [
            "The SDK dev fixture is deterministic only; notproduction Shape proofs remain notunavailable.",
        ],
        [
            "The SDK not dev fixture is deterministic only; not production Shape proofs remain not unavailable.",
        ],
    ],
)
def test_privacy_catalog_loader_rejects_dev_fixture_entrypoints_without_non_production_warning(
    monkeypatch,
    security_notes,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[
                        "buildShapeDevProofFixture",
                        "verifyShapeProofLocally",
                    ],
                    securityNotes=security_notes,
                    plannedSdkEntrypoints=["buildShapeProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable DevFixture SDK entrypoints must include a security note "
            "that marks dev fixtures as non-production"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_dev_fixture_entrypoints_without_planned_production_surface(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[
                        "buildShapeDevProofFixture",
                        "verifyShapeProofLocally",
                    ],
                    securityNotes=[
                        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
                    ],
                    plannedSdkEntrypoints=[],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable DevFixture SDK entrypoints must retain planned "
            "production SDK entrypoints or export a production proof builder "
            "until production gates pass"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "planned_entrypoints",
    [
        [
            "buildShapeProductionInstruction",
            "buildShapeProofInstruction",
        ],
        ["buildShapeProofTransaction"],
        ["buildSubmitShapeProof"],
        ["buildShapeProofEnvelope"],
        ["buildShapeProofWitness"],
        ["buildShapeProofPublicInputs"],
        ["buildShapeProofRequest"],
        ["buildShapeProofCommitment"],
    ],
)
def test_privacy_catalog_loader_rejects_dev_fixture_entrypoints_without_planned_production_proof_builder(
    monkeypatch,
    planned_entrypoints,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=[
                        "buildShapeDevProofFixture",
                        "verifyShapeProofLocally",
                    ],
                    securityNotes=[
                        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
                    ],
                    plannedSdkEntrypoints=planned_entrypoints,
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "executable DevFixture SDK entrypoints must retain a planned "
            "production proof builder until production gates pass"
        ),
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_overlapping_sdk_entrypoints(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=["buildProof"],
                    plannedSdkEntrypoints=["buildProof"],
                )
            ]
        ),
    )

    with pytest.raises(RuntimeError, match="entry 'buildProof' is already executable"):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_accepts_namespaced_sdk_entrypoints(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    sdkEntrypoints=["Iroha.Privacy.buildProof"],
                    plannedSdkEntrypoints=["Iroha.Privacy.buildFutureProof"],
                )
            ]
        ),
    )

    descriptor = privacy_catalog._load_descriptors()[0]

    assert descriptor["sdk_entrypoints"] == ["Iroha.Privacy.buildProof"]
    assert descriptor["planned_sdk_entrypoints"] == [
        "Iroha.Privacy.buildFutureProof"
    ]


@pytest.mark.parametrize(
    "entrypoint",
    [
        "buildFutureDevProofFixture",
        "buildFutureProofFixture",
        "buildFutureMockProof",
        "buildFutureM-o-c-kProof",
        "buildFutureDev.Proof.Fixture",
    ],
)
def test_privacy_catalog_loader_rejects_planned_fixture_or_mock_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    plannedSdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=f"entry '{entrypoint}' is a fixture/mock entrypoint",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyFutureShapeProofLocally",
        "verifyFutureShapeProofLocal",
        "verifyFutureShapeProofLocalVerifier",
        "Iroha.Privacy.verifyFutureShapeProofLocally",
        "Iroha.Privacy.verifyFutureShapeProofLocalVerifier",
    ],
)
def test_privacy_catalog_loader_rejects_planned_local_verifier_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    plannedSdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=f"entry '{re.escape(entrypoint)}' is a local-only verifier entrypoint",
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    "entrypoint",
    [
        "verifyShapeProofLocally",
        "verifyShapeProofLocal",
        "verifyShapeProofLocalVerifier",
        "Iroha.Privacy.verifyShapeProofLocally",
        "Iroha.Privacy.verifyShapeProofLocalVerifier",
    ],
)
def test_privacy_catalog_loader_rejects_production_hardened_local_verifier_entrypoints(
    monkeypatch,
    entrypoint,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="production-hardened",
                    sdkEntrypoints=[entrypoint],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match=(
            "production-hardened targets cannot advertise "
            "local-only verifier SDK entrypoints"
        ),
    ):
        privacy_catalog._load_descriptors()


@pytest.mark.parametrize(
    ("entrypoint", "expected"),
    [
        ("buildShapeProductionProof", True),
        ("Iroha.Privacy.buildShapeProductionProof", True),
        ("buildAnonymousPgcKOutOfNProofV1", True),
        ("buildShapeNoProofBuilder", False),
        ("buildShapeNotProofBuilder", False),
        ("buildShapeProofInstruction", False),
        ("buildShapeProofTransaction", False),
        ("buildSubmitShapeProof", False),
        ("buildShapeProofEnvelope", False),
        ("buildShapeProofWitness", False),
        ("buildShapeProofPublicInputs", False),
        ("buildShapeProofRequest", False),
        ("buildShapeProofCommitment", False),
        ("buildShapeDevProofFixture", False),
    ],
)
def test_privacy_catalog_production_proof_builder_rejects_ledger_mutation_aliases(
    entrypoint,
    expected,
) -> None:
    assert privacy_catalog._entrypoint_is_production_proof_builder(entrypoint) is expected


def test_privacy_catalog_loader_rejects_catalog_stage_sdk_entrypoints(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="catalog-as-of-2026-05",
                    sdkEntrypoints=["buildForgedProductionProof"],
                    plannedSdkEntrypoints=["buildRealProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="catalog-only targets cannot advertise SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_production_stage_planned_entrypoints(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps(
            [
                _raw_descriptor(
                    implementationStage="production-hardened",
                    plannedSdkEntrypoints=["buildFutureProductionProof"],
                )
            ]
        ),
    )

    with pytest.raises(
        RuntimeError,
        match="production-hardened targets cannot retain planned SDK entrypoints",
    ):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_loader_rejects_missing_backend_family_metadata(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        privacy_catalog,
        "_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        json.dumps([_raw_descriptor(id="unmapped-backend-family")]),
    )

    with pytest.raises(RuntimeError, match="missing backend family metadata"):
        privacy_catalog._load_descriptors()


def test_privacy_catalog_getters_return_defensive_copies() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    descriptors[0]["id"] = "tampered"
    descriptors[1]["verifier_key_metadata"]["pq_layers"]["proof"] = True

    fresh = get_privacy_algorithm_descriptors()

    assert fresh[0]["id"] != "tampered"
    assert fresh[1]["verifier_key_metadata"]["pq_layers"]["proof"] is False


def test_privacy_catalog_single_descriptor_lookup_is_defensive() -> None:
    descriptor = get_privacy_algorithm_descriptor("shield")
    assert descriptor is not None
    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = True

    fresh = get_privacy_algorithm_descriptor("shield")

    assert fresh is not None
    assert fresh["verifier_key_metadata"]["pq_layers"]["proof"] is False
    assert get_privacy_algorithm_descriptor("../../shield") is None
    assert get_privacy_algorithm_descriptor(7) is None


def test_privacy_capabilities_treats_hostile_client_attributes_as_unavailable() -> None:
    class HostileClient:
        def __getattr__(self, _name):
            raise RuntimeError("attribute trap")

    capabilities = privacy_capabilities(HostileClient())

    assert capabilities["transfer_asset_instruction"] is False
    assert capabilities["shield_instruction"] is False
    assert capabilities["zk_transfer_instruction"] is False
    assert capabilities["unshield_instruction"] is False
    assert capabilities["privacy_algorithms"][0]["id"] == "transparent-transfer"


@pytest.mark.parametrize(
    "bad_id",
    ["Shield", "shield/../../admin", "shield.v1", "_shield", "-shield", "shield_", "shield-"],
)
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
        ("name", " Shape check", "field 'name' must be clean and already trimmed"),
        ("shortName", "", "field 'short_name' must be a non-empty string"),
        (
            "shortName",
            "Shape\nnext",
            "field 'short_name' must be clean and already trimmed",
        ),
        ("summary", "   ", "field 'summary' must be a non-empty string"),
        (
            "summary",
            "Descriptor\x7fsummary",
            "field 'summary' must be clean and already trimmed",
        ),
        (
            "summary",
            "Descriptor\u200bsummary",
            "field 'summary' must be clean and already trimmed",
        ),
        ("category", None, "field 'category' must be a non-empty string"),
        ("category", "payments", "field 'category' must be one of"),
        ("maturity", 7, "field 'maturity' must be a non-empty string"),
        ("maturity", "blog_post", "field 'maturity' must be one of"),
        (
            "implementationStage",
            "Chain-Executable",
            "field 'implementation_stage' must be a lowercase hyphenated identifier",
        ),
        (
            "implementationStage",
            "chain--executable",
            "field 'implementation_stage' must be a lowercase hyphenated identifier",
        ),
        ("proofFamily", "__missing__", "field 'proof_family' is required"),
        ("proofFamily", "", "field 'proof_family' must be a non-empty string"),
        (
            "proofFamily",
            " halo2-ipa",
            "field 'proof_family' must be clean and already trimmed",
        ),
        ("proofFamily", "Halo2", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "halo2..ipa", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "halo2/../ipa", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "halo2--ipa", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "/halo2", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "-halo2", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "halo2/", "field 'proof_family' must be a proof family name"),
        ("proofFamily", "halo2-", "field 'proof_family' must be a proof family name"),
        (
            "publicInputsSchema",
            "",
            "field 'public_inputs_schema' must be a non-empty string or null",
        ),
        (
            "publicInputsSchema",
            "root,\nproof",
            "field 'public_inputs_schema' must be clean and already trimmed",
        ),
        (
            "publicInputsSchema",
            "root,",
            "field 'public_inputs_schema' token 1 must be a non-empty public input name",
        ),
        (
            "publicInputsSchema",
            "root, proof",
            "field 'public_inputs_schema' token 1 must be clean and already trimmed",
        ),
        (
            "publicInputsSchema",
            "root,Proof",
            "field 'public_inputs_schema' token 1 must be a lowercase public input name",
        ),
        (
            "publicInputsSchema",
            "root,1proof",
            "field 'public_inputs_schema' token 1 must be a lowercase public input name",
        ),
        (
            "publicInputsSchema",
            "root,field_",
            "field 'public_inputs_schema' token 1 must be a lowercase public input name",
        ),
        (
            "publicInputsSchema",
            "root,field__digest",
            "field 'public_inputs_schema' token 1 must be a lowercase public input name",
        ),
        (
            "publicInputsSchema",
            "root,proof",
            "field 'public_inputs_schema' token 1 must not include proof or witness payload metadata",
        ),
        (
            "publicInputsSchema",
            "root,recursive_proof_digest",
            "field 'public_inputs_schema' token 1 must not include proof or witness payload metadata",
        ),
        (
            "publicInputsSchema",
            "root,wallet_witness_digest",
            "field 'public_inputs_schema' token 1 must not include proof or witness payload metadata",
        ),
        (
            "publicInputsSchema",
            "root,root",
            "field 'public_inputs_schema' token 1 duplicates 'root'",
        ),
        (
            "verifierKeyId",
            "   ",
            "field 'verifier_key_id' must be a non-empty string or null",
        ),
        (
            "verifierKeyId",
            7,
            "field 'verifier_key_id' must be a non-empty string or null",
        ),
        (
            "verifierKeyId",
            "zk::Shield\t",
            "field 'verifier_key_id' must be clean and already trimmed",
        ),
        ("coveredCriteria", "hide_receiver", "field 'covered_criteria' must be a list"),
        ("coveredCriteria", "__missing__", "field 'covered_criteria' is required"),
        (
            "coveredCriteria",
            ["hide_sender", "forged_availability"],
            "field 'covered_criteria' item 1 must be one of",
        ),
        (
            "coveredCriteria",
            ["hide_sender", "hide_sender"],
            "field 'covered_criteria' item 1 duplicates 'hide_sender'",
        ),
        ("chainRequirements", {"bad": "shape"}, "field 'chain_requirements' must be a list"),
        ("sdkEntrypoints", [7], "field 'sdk_entrypoints' item 0 must be a string"),
        (
            "sdkEntrypoints",
            [""],
            "field 'sdk_entrypoints' item 0 must be a non-empty string",
        ),
        (
            "sdkEntrypoints",
            ["buildProof-withSuffix"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["build$Proof"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["_buildProof"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["buildProof_"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["build_Proof"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["Iroha._Privacy.buildProof"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["Iroha.Privacy_.buildProof"],
            "field 'sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFuture$Proof"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["_buildFutureProof"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFutureProof_"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFuture_Proof"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["Iroha._Privacy.buildFutureProof"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["Iroha.Privacy_.buildFutureProof"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFutureProof;rm"],
            "field 'planned_sdk_entrypoints' item 0 must be an SDK entrypoint name",
        ),
        (
            "sdkEntrypoints",
            ["buildProof", "buildProof"],
            "field 'sdk_entrypoints' item 1 duplicates 'buildProof'",
        ),
        (
            "plannedSdkEntrypoints",
            ["buildFutureProof", "buildFutureProof"],
            "field 'planned_sdk_entrypoints' item 1 duplicates 'buildFutureProof'",
        ),
        (
            "sourceReferences",
            [{"label": "bad", "url": 7}],
            "source_references' item 0 must include string label and url",
        ),
        (
            "sourceReferences",
            [{"label": "", "url": "https://example.invalid"}],
            "source_references' item 0 must include non-empty label and url",
        ),
        (
            "sourceReferences",
            [{"label": "bad", "url": "http://example.invalid"}],
            "source_references' item 0 must use an https URL",
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://127%2e0%2e0%2e1/source",
                }
            ],
            "source_references' item 0 must use an https URL",
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://localhost%2elocaltest%2eme/source",
                }
            ],
            "source_references' item 0 must use an https URL",
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://256.256.256.256/source",
                }
            ],
            "source_references' item 0 must use an https URL",
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?section=notes%ZZappendix",
                }
            ],
            "source_references' item 0 must use an https URL",
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224#external-audit-complete",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?production=ready",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?evidence=audit%3Dcomplete",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?evidence=production%253Dready",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?evidence=mainnet%2520claim",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224#external-%2561udit-complete",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
        ),
        (
            "sourceReferences",
            [
                {
                    "label": "paper",
                    "url": "https://zips.z.cash/zip-0224?evidence=production%2525253Dready",
                }
            ],
            (
                "source_references' item 0 url must describe protocol "
                "source material, not audit/signoff or readiness evidence"
            ),
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
        json.dumps([_raw_descriptor(pqLayers=raw_pq_layers)]),
    )

    [descriptor] = privacy_catalog._load_descriptors()

    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = False
    assert descriptor["pq_layers"]["proof"] is True


def test_privacy_catalog_returns_defensive_copies() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    descriptors[0]["id"] = "tampered"
    descriptors[0]["pq_layers"]["proof"] = "tampered"
    descriptors[0]["verifier_key_metadata"]["pq_layers"]["proof"] = "tampered"
    descriptors[0]["production_ready"] = True
    descriptors[0]["production_gate"]["ready"] = True
    descriptors[0]["production_gate"]["gates"]["external_audit"] = True
    real_proving = descriptors[0]["production_gate"]["gates"].pop("real_proving")
    descriptors[0]["production_gate"]["gates"]["real_proving"] = real_proving
    descriptors[0]["production_gate"]["required_gates"].append("real_proving")
    descriptors[0]["production_gate"]["missing"].reverse()
    descriptors[0]["production_gate"]["missing"].clear()
    descriptors[0]["production_gate"]["audit_references"].append(
        {"label": "forged audit", "url": "https://audit.example/forged"}
    )
    sdk_descriptor = next(
        descriptor for descriptor in descriptors[1:] if descriptor["sdk_entrypoints"]
    )
    sdk_descriptor_id = sdk_descriptor["id"]
    sdk_descriptor["sdk_entrypoints"].clear()
    source_descriptor = next(
        descriptor for descriptor in descriptors[1:] if descriptor["source_references"]
    )
    source_descriptor_id = source_descriptor["id"]
    source_descriptor["source_references"][0]["url"] = "https://audit.example/forged"
    source_descriptor["source_references"].append(
        {"label": "forged source", "url": "https://audit.example/forged"}
    )

    fresh_descriptors = get_privacy_algorithm_descriptors()
    assert fresh_descriptors[0]["id"] == "transparent-transfer"
    assert fresh_descriptors[0]["pq_layers"]["proof"] is False
    assert fresh_descriptors[0]["verifier_key_metadata"]["pq_layers"]["proof"] is False
    assert fresh_descriptors[0]["production_ready"] is False
    assert fresh_descriptors[0]["production_gate"]["ready"] is False
    assert fresh_descriptors[0]["production_gate"]["gates"]["external_audit"] is False
    assert list(fresh_descriptors[0]["production_gate"]["gates"].items()) == (
        _expected_production_gate_items()
    )
    assert fresh_descriptors[0]["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys(fresh_descriptors[0]["id"])
    )
    assert fresh_descriptors[0]["production_gate"]["missing"] == (
        _expected_production_gate_missing(fresh_descriptors[0]["production_gate"])
    )
    assert "internal cryptographic review signoff is missing" in fresh_descriptors[0][
        "production_gate"
    ]["missing"]
    assert fresh_descriptors[0]["production_gate"]["audit_references"] == []
    fresh_sdk_descriptor = next(
        descriptor
        for descriptor in fresh_descriptors
        if descriptor["id"] == sdk_descriptor_id
    )
    assert fresh_sdk_descriptor["sdk_entrypoints"]
    assert (
        "planned SDK entrypoints remain"
        not in fresh_sdk_descriptor["production_gate"]["missing"]
    )
    fresh_source_descriptor = next(
        descriptor
        for descriptor in fresh_descriptors
        if descriptor["id"] == source_descriptor_id
    )
    assert all(
        source["url"] != "https://audit.example/forged"
        for source in fresh_source_descriptor["source_references"]
    )

    descriptor = get_privacy_algorithm_descriptor("pq-masp-stark-v0")
    assert descriptor is not None
    descriptor["covered_criteria"].clear()
    descriptor["pq_layers"]["proof"] = False
    descriptor["sdk_entrypoints"].clear()
    descriptor["planned_sdk_entrypoints"].clear()
    descriptor["source_references"][0]["label"] = "forged source"
    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = False
    descriptor["production_ready"] = True
    descriptor["production_gate"]["ready"] = True
    descriptor["production_gate"]["required_gates"].clear()
    descriptor["production_gate"]["missing"].reverse()
    descriptor["production_gate"]["audit_references"].append(
        {"label": "forged audit", "url": "https://audit.example/forged"}
    )

    fresh = get_privacy_algorithm_descriptor("pq-masp-stark-v0")
    assert fresh is not None
    assert "post_quantum" in fresh["covered_criteria"]
    assert fresh["pq_layers"]["proof"] is True
    assert fresh["sdk_entrypoints"]
    assert fresh["planned_sdk_entrypoints"] == []
    assert fresh["source_references"][0]["label"] != "forged source"
    assert fresh["verifier_key_metadata"]["pq_layers"]["proof"] is True
    assert fresh["production_ready"] is False
    assert fresh["production_gate"]["ready"] is False
    assert fresh["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys(fresh["id"])
    )
    assert fresh["production_gate"]["missing"] == _expected_production_gate_missing(
        fresh["production_gate"]
    )
    assert fresh["production_gate"]["audit_references"] == []
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
    descriptors = get_privacy_algorithm_descriptors()
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
        "backend_family",
        "verifier_key_metadata",
        "production_ready",
        "production_gate",
    }

    assert list(privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID) == [
        descriptor["id"] for descriptor in descriptors
    ]
    for descriptor in descriptors:
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
        assert (
            descriptor["backend_family"]
            == privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID[descriptor["id"]]
        )
        assert descriptor["production_ready"] is False
        assert descriptor["production_gate"]["version"] == (
            privacy_catalog.PRODUCTION_GATE_VERSION
        )
        assert descriptor["production_gate"]["ready"] is False
        assert list(descriptor["production_gate"]["gates"].items()) == (
            _expected_production_gate_items()
        )
        assert descriptor["production_gate"]["required_gates"] == (
            _expected_required_production_gate_keys(descriptor["id"])
        )
        assert all(
            ready is False
            for ready in descriptor["production_gate"]["gates"].values()
        )
        assert descriptor["production_gate"]["audit_references"] == []
        assert descriptor["production_gate"]["missing"] == (
            _expected_production_gate_missing(descriptor["production_gate"])
        )
        assert "internal cryptographic review signoff is missing" in descriptor[
            "production_gate"
        ]["missing"]
        assert "Iroha production allowlist is not enabled for this audited row" in descriptor[
            "production_gate"
        ]["missing"]
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

    transparent = by_id["transparent-transfer"]
    assert transparent["proof_family"] == "none"
    assert list(transparent["production_gate"]["gates"].items()) == (
        _expected_production_gate_items()
    )
    assert transparent["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys("transparent-transfer")
    )
    assert not any(
        key in transparent["production_gate"]["required_gates"]
        for key in privacy_catalog.TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS
    )
    assert not any(
        missing
        in {
            "real proving engine is not registered",
            "real verifier is not registered",
            "witness privacy checks are incomplete",
            "verifier fuzzing gate is incomplete",
        }
        for missing in transparent["production_gate"]["missing"]
    )

    zk_ace = by_id["zk-ace-pq-authorization-v0"]
    assert zk_ace["implementation_stage"] == "chain-executable"
    assert zk_ace["sdk_entrypoints"] == [
        "buildRegisterZkAceIdentityCommitmentInstruction",
        "buildRotateZkAceIdentityCommitmentInstruction",
        "buildRevokeZkAceIdentityCommitmentInstruction",
        "buildZkAceAuthorizedTransferInstruction",
        "buildZkAceAuthorizationProofV1",
    ]
    assert zk_ace["planned_sdk_entrypoints"] == []
    assert "buildZkAceAuthorizationProofV0" not in zk_ace["planned_sdk_entrypoints"]
    assert zk_ace["required_state"] == [
        "registered ZK-ACE identity commitment",
        "source-account allowlist",
        "authorization policy hash registry",
        "active ZK-ACE verifier key",
        "chain/domain binding state",
        "transfer digest binding",
        "replay nullifier uniqueness set",
        "identity rotation/revocation registry",
        "STARK/FRI verifier parameter floors",
        "wallet identity witness and replay-secret store",
    ]
    assert zk_ace["verifier_key_metadata"]["pq_layers"] == {
        "proof": True,
        "authorization": True,
        "note_encryption": False,
    }
    assert "post_quantum" not in zk_ace["covered_criteria"]
    assert zk_ace["proof_family"] == "stark/fri/sha256-goldilocks"
    assert zk_ace["backend_family"] == "stark-fri"
    assert zk_ace["production_ready"] is False
    assert zk_ace["production_gate"]["ready"] is False
    assert zk_ace["production_gate"]["audit_references"] == []
    assert list(zk_ace["production_gate"]["gates"].items()) == (
        _expected_production_gate_items()
    )
    assert zk_ace["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys(zk_ace["id"])
    )
    assert all(ready is False for ready in zk_ace["production_gate"]["gates"].values())
    assert zk_ace["production_gate"]["missing"] == [
        *(label for _key, label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS),
        "implementation stage is not production-hardened",
        "Iroha production allowlist is not enabled for this audited row",
    ]

    anonymous_pgc = by_id["anonymous-pgc-k-out-of-n-v1"]
    assert anonymous_pgc["implementation_stage"] == "sdk-builder"
    assert "dev fixture entrypoints are not production entrypoints" not in anonymous_pgc[
        "production_gate"
    ]["missing"]
    assert anonymous_pgc["sdk_entrypoints"] == [
        "buildAnonymousPgcReceiverSet",
        "buildAnonymousPgcAccountCommitmentInstruction",
        "buildAnonymousPgcKOutOfNProofV1",
        "buildAnonymousPgcTransferInstruction",
    ]
    assert anonymous_pgc["planned_sdk_entrypoints"] == []
    assert [
        requirement
        for requirement in anonymous_pgc["chain_requirements"]
        if "zk::" in requirement and "instruction" in requirement
    ] == [
        "typed zk::RegisterAnonymousPgcAccountCommitment instruction",
        "typed zk::SubmitAnonymousPgcTransfer instruction",
    ]

    zkat = by_id["zkat-policy-private-auth-v1"]
    assert zkat["implementation_stage"] == "sdk-builder"
    assert zkat["sdk_entrypoints"] == [
        "buildZkAtPolicyCommitment",
        "buildZkAtAuthenticatorEnvelope",
        "buildZkAtPolicyProofV1",
        "verifyZkAtPolicyProofV1",
    ]
    assert zkat["planned_sdk_entrypoints"] == []
    assert [
        requirement
        for requirement in zkat["chain_requirements"]
        if "zk::" in requirement
    ] == [
        "typed zk::RegisterZkAtPolicyCommitment instruction",
        "typed zk::SubmitZkAtAuthorizedTransaction admission",
    ]

    zk_ams = by_id["zk-ams-recursive-admission-v0"]
    assert zk_ams["implementation_stage"] == "sdk-builder"
    assert zk_ams["public_inputs_schema"] == (
        "issuer_root,admission_batch_root,admission_nullifiers,"
        "anonymous_account_commitments,recursive_admission_digest,domain_separator"
    )
    assert zk_ams["sdk_entrypoints"] == [
        "buildZkAmsAdmissionBatch",
        "buildZkAmsAdmissionProofEnvelope",
        "buildZkAmsAdmissionBatchProofV0",
        "verifyZkAmsAdmissionBatchProofV0",
    ]
    assert zk_ams["planned_sdk_entrypoints"] == []

    vega = by_id["vega-existing-credential-zk-v0"]
    assert vega["implementation_stage"] == "sdk-builder"
    assert vega["sdk_entrypoints"] == [
        "buildVegaCredentialPredicateCommitment",
        "buildVegaCredentialProofEnvelope",
        "buildVegaCredentialPredicateProofV0",
        "verifyVegaCredentialPredicateProofV0",
    ]
    assert vega["planned_sdk_entrypoints"] == []

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
        "buildSilentThresholdCredentialShowingProofV0",
        "verifySilentThresholdCredentialShowingProofV0",
    ]
    assert silent_threshold["planned_sdk_entrypoints"] == []

    zk_x509 = by_id["zk-x509-onchain-identity-v0"]
    assert zk_x509["implementation_stage"] == "sdk-builder"
    assert zk_x509["public_inputs_schema"] == (
        "ca_root_commitment,certificate_policy_hash,revocation_root,"
        "subject_commitment,address_binding,domain_separator"
    )
    assert zk_x509["sdk_entrypoints"] == [
        "buildZkX509IdentityCommitments",
        "buildZkX509IdentityEnvelope",
        "buildZkX509IdentityProofV0",
        "verifyZkX509IdentityProofV0",
    ]
    assert zk_x509["planned_sdk_entrypoints"] == []

    jindo = by_id["jindo-lattice-pcs-zk-v0"]
    assert jindo["implementation_stage"] == "sdk-builder"
    assert jindo["public_inputs_schema"] == (
        "commitment,opening_claim,query_set,parameter_hash,domain_separator"
    )
    assert jindo["sdk_entrypoints"] == [
        "buildJindoLatticePublicInputs",
        "buildJindoLatticeProofEnvelope",
        "buildJindoLatticeProofV0",
        "verifyJindoPolynomialCommitmentV0",
    ]
    assert jindo["planned_sdk_entrypoints"] == []

    sis_hints = by_id["sis-hints-anoncred-pq-v0"]
    assert sis_hints["implementation_stage"] == "sdk-builder"
    assert sis_hints["public_inputs_schema"] == (
        "issuer_commitment,credential_commitment,"
        "showing_policy_hash,parameter_hash,domain_separator"
    )
    assert sis_hints["sdk_entrypoints"] == [
        "buildSisHintsCredentialCommitments",
        "buildSisHintsCredentialEnvelope",
        "buildSisHintsAnonymousCredentialProofV0",
        "verifySisHintsAnonymousCredentialProofV0",
    ]
    assert sis_hints["planned_sdk_entrypoints"] == []

    verange = by_id["verange-transparent-range-v1"]
    assert verange["implementation_stage"] == "component"
    assert verange["public_inputs_schema"] == (
        "commitments,range_parameters,aggregation_count,domain_separator,payload_digest"
    )
    assert verange["sdk_entrypoints"] == [
        "buildRangeCommitment",
        "buildVeRangeDevProofFixture",
        "buildVeRangeProofEnvelope",
        "buildVeRangeProofV1",
        "verifyVeRangeProofLocally",
        "verifyVeRangeProofV1",
    ]
    assert verange["planned_sdk_entrypoints"] == []

    for descriptor in descriptors:
        assert descriptor["maturity"] in allowed_maturities
        if any(
            privacy_catalog._entrypoint_is_local_verifier(entrypoint)
            for entrypoint in descriptor["sdk_entrypoints"]
        ):
            assert any(
                privacy_catalog._entrypoint_is_explicit_dev_fixture(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
        if any(
            privacy_catalog._entrypoint_is_explicit_dev_fixture(entrypoint)
            for entrypoint in descriptor["sdk_entrypoints"]
        ):
            assert any(
                privacy_catalog._entrypoint_is_production_proof_builder(entrypoint)
                for entrypoint in descriptor["planned_sdk_entrypoints"]
            ) or any(
                privacy_catalog._entrypoint_is_production_proof_builder(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
        if descriptor["implementation_stage"] == "production-hardened":
            assert not any(
                privacy_catalog._entrypoint_is_local_verifier(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
        if descriptor["implementation_stage"] == "chain-executable":
            assert not any(
                privacy_catalog._entrypoint_is_dev_fixture(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
            assert not any(
                privacy_catalog._entrypoint_is_local_verifier(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
        if descriptor["implementation_stage"] == "component":
            assert not any(
                privacy_catalog._entrypoint_is_instruction_builder(entrypoint)
                for entrypoint in (
                    *descriptor["sdk_entrypoints"],
                    *descriptor["planned_sdk_entrypoints"],
                )
            )
        planned_ledger_mutations = [
            entrypoint
            for entrypoint in descriptor["planned_sdk_entrypoints"]
            if privacy_catalog._entrypoint_is_planned_ledger_mutation(entrypoint)
        ]
        if planned_ledger_mutations:
            protection_values = [
                value.lower()
                for field in ("required_state", "failure_modes", "chain_requirements")
                for value in descriptor[field]
            ]
            assert any(
                privacy_catalog._catalog_text_values_contain_affirmed_metadata_token(
                    protection_values,
                    token,
                )
                for token in privacy_catalog._LEDGER_MUTATION_PROTECTION_METADATA_TOKENS
            )
            typed_admission_text = " ".join(
                value.lower()
                for field in privacy_catalog._TYPED_CHAIN_ADMISSION_METADATA_FIELDS
                for value in descriptor[field]
            )
            assert any(
                privacy_catalog._catalog_text_contains_typed_admission_token(
                    typed_admission_text,
                    token,
                )
                for token in privacy_catalog._TYPED_CHAIN_ADMISSION_TYPE_TOKENS
            )
            assert any(
                privacy_catalog._catalog_text_contains_typed_admission_token(
                    typed_admission_text,
                    token,
                )
                for token in privacy_catalog._TYPED_CHAIN_ADMISSION_MUTATION_TOKENS
            )
            required_state_text = " ".join(descriptor["required_state"]).lower()
            if any(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    required_state_text,
                    token,
                )
                for token in privacy_catalog._STATEFUL_LEDGER_STATE_TOKENS
            ):
                persistence_text = " ".join(
                    value.lower()
                    for field in (
                        privacy_catalog._STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS
                    )
                    for value in descriptor[field]
                )
                for tokens in (
                    privacy_catalog._STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS
                ):
                    assert any(
                        privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                            persistence_text,
                            token,
                        )
                        for token in tokens
                    )
                failure_modes_text = " ".join(descriptor["failure_modes"]).lower()
                for tokens in (
                    privacy_catalog._STATEFUL_LEDGER_FAILURE_MODE_TOKEN_GROUPS
                ):
                    assert any(
                        privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                            failure_modes_text,
                            token,
                        )
                        for token in tokens
                    )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES
            and descriptor["category"]
            not in privacy_catalog._WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES
        ):
            required_state_text = " ".join(descriptor["required_state"]).lower()
            assert any(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    required_state_text,
                    token,
                )
                for token in privacy_catalog._WALLET_STATE_METADATA_TOKENS
            )
            security_notes_text = " ".join(descriptor["security_notes"]).lower()
            for tokens in privacy_catalog._WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS:
                assert any(
                    privacy_catalog._catalog_text_contains_wallet_witness_privacy_token(
                        security_notes_text,
                        token,
                    )
                    for token in tokens
                )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
            and descriptor["category"]
            in privacy_catalog._CREDENTIAL_STATE_REQUIRED_CATEGORIES
        ):
            required_state_text = " ".join(descriptor["required_state"]).lower()
            assert any(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    required_state_text,
                    token,
                )
                for token in privacy_catalog._CREDENTIAL_STATE_METADATA_TOKENS
            )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
            and descriptor["verifier_key_id"] is not None
        ):
            public_input_tokens = (
                descriptor["public_inputs_schema"].split(",")
                if descriptor["public_inputs_schema"] is not None
                else []
            )
            for token in public_input_tokens:
                assert not any(
                    segment
                    in privacy_catalog._PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS
                    for segment in token.split("_")
                )
            failure_modes_text = " ".join(descriptor["failure_modes"]).lower()
            for tokens in (
                privacy_catalog._VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS
            ):
                assert any(
                    privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                        failure_modes_text,
                        token,
                    )
                    for token in tokens
                )
            verifier_key_record_text = " ".join(
                value.lower()
                for field in privacy_catalog._VERIFIER_KEY_RECORD_METADATA_FIELDS
                for value in descriptor[field]
            )
            assert any(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    verifier_key_record_text,
                    token,
                )
                for token in privacy_catalog._VERIFIER_KEY_RECORD_METADATA_TOKENS
            )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
            and descriptor["verifier_key_id"] is not None
        ):
            chain_domain_binding_text = " ".join(
                str(value).lower()
                for field in privacy_catalog._CHAIN_DOMAIN_BINDING_METADATA_FIELDS
                for value in (
                    [descriptor[field]]
                    if isinstance(descriptor[field], str)
                    else descriptor[field]
                )
            )
            assert any(
                privacy_catalog._catalog_text_contains_chain_domain_binding_token(
                    chain_domain_binding_text,
                    token,
                )
                for token in privacy_catalog._CHAIN_DOMAIN_BINDING_METADATA_TOKENS
            )
            assert privacy_catalog._public_inputs_schema_has_chain_domain_binding(
                descriptor["public_inputs_schema"]
            )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        ):
            security_notes_text = " ".join(descriptor["security_notes"]).lower()
            for tokens in privacy_catalog._SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS:
                assert any(
                    privacy_catalog._catalog_text_contains_source_hardening_token(
                        security_notes_text,
                        token,
                    )
                    for token in tokens
                )
        if descriptor["implementation_stage"] == "research-target-as-of-2026-05":
            assert not any(
                privacy_catalog._entrypoint_is_dev_fixture(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
            assert not any(
                privacy_catalog._entrypoint_is_local_verifier(entrypoint)
                for entrypoint in descriptor["sdk_entrypoints"]
            )
            assert descriptor["sdk_entrypoints"] == []
            assert descriptor["planned_sdk_entrypoints"]
            security_notes_text = " ".join(descriptor["security_notes"]).lower()
            assert all(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    security_notes_text,
                    token,
                )
                for token in (
                    privacy_catalog._RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS
                )
            )
            assert any(
                privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                    security_notes_text,
                    token,
                )
                for token in privacy_catalog._RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS
            )
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
        if "post_quantum" in descriptor["covered_criteria"]:
            source_urls = {
                source_reference["url"]
                for source_reference in descriptor["source_references"]
            }
            assert privacy_catalog._POST_QUANTUM_REQUIRED_SOURCE_URLS <= source_urls
            post_quantum_entrypoints = (
                descriptor["sdk_entrypoints"]
                if descriptor["implementation_stage"]
                in privacy_catalog._EXECUTABLE_SDK_IMPLEMENTATION_STAGES
                else descriptor["planned_sdk_entrypoints"]
            )
            planned_entrypoint_names = [
                entrypoint.rsplit(".", 1)[-1]
                for entrypoint in post_quantum_entrypoints
            ]
            for fragment in (
                privacy_catalog._POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS
            ):
                assert any(
                    privacy_catalog._planned_entrypoint_name_has_primitive_fragment(
                        name,
                        fragment,
                    )
                    for name in planned_entrypoint_names
                )
            post_quantum_token_fields = (
                (
                    "security_notes",
                    privacy_catalog._POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS,
                ),
                (
                    "failure_modes",
                    privacy_catalog._POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS,
                ),
                (
                    "required_state",
                    privacy_catalog._POST_QUANTUM_REQUIRED_STATE_TOKENS,
                ),
            )
            for field, required_tokens in post_quantum_token_fields:
                values = descriptor[field]
                for token in required_tokens:
                    assert any(
                        privacy_catalog._catalog_text_contains_affirmed_metadata_token(
                            value,
                            token,
                        )
                        for value in values
                    )


def test_planned_privacy_sdk_entrypoints_remain_unexported_and_fail_closed() -> None:
    descriptors = get_privacy_algorithm_descriptors()
    planned_entrypoints = {
        entrypoint
        for descriptor in descriptors
        for entrypoint in descriptor["planned_sdk_entrypoints"]
    }
    sdk_entrypoints = {
        entrypoint
        for descriptor in descriptors
        for entrypoint in descriptor["sdk_entrypoints"]
    }
    planned_name_variants = set().union(
        *(_planned_entrypoint_name_variants(entrypoint) for entrypoint in planned_entrypoints)
    )
    package_exports = set(getattr(iroha_python, "__all__", ()))
    crypto_exports = set(getattr(crypto, "__all__", ()))

    assert planned_entrypoints == set()
    assert planned_entrypoints.isdisjoint(sdk_entrypoints)
    assert all(
        not privacy_catalog._entrypoint_is_local_verifier(entrypoint)
        for entrypoint in planned_entrypoints
    )
    assert planned_name_variants.isdisjoint(package_exports)
    assert planned_name_variants.isdisjoint(crypto_exports)

    for module_name, module in (
        ("iroha_python", iroha_python),
        ("iroha_python.crypto", crypto),
    ):
        for entrypoint in planned_name_variants:
            assert not hasattr(module, entrypoint), (
                f"{entrypoint} is still a planned production privacy entrypoint "
                f"and must not be exported from {module_name} until the "
                "production gate passes"
            )

    capabilities = privacy_capabilities()
    for descriptor in capabilities["privacy_algorithms"]:
        if descriptor["planned_sdk_entrypoints"]:
            assert descriptor["production_ready"] is False
            assert descriptor["production_gate"]["ready"] is False
            assert "planned SDK entrypoints remain" in descriptor[
                "production_gate"
            ]["missing"]
            assert descriptor["planned_sdk_entrypoints"]
            assert set(descriptor["planned_sdk_entrypoints"]).isdisjoint(
                descriptor["sdk_entrypoints"]
            )


def test_planned_privacy_sdk_entrypoints_have_no_public_python_definitions() -> None:
    planned_entrypoints = {
        entrypoint
        for descriptor in get_privacy_algorithm_descriptors()
        for entrypoint in descriptor["planned_sdk_entrypoints"]
    }
    planned_name_variants = set().union(
        *(_planned_entrypoint_name_variants(entrypoint) for entrypoint in planned_entrypoints)
    )
    source_root = Path(privacy_catalog.__file__).resolve().parent

    assert planned_entrypoints == set()
    for source_path in sorted(source_root.rglob("*.py")):
        text = source_path.read_text(encoding="utf8")
        for entrypoint in planned_name_variants:
            pattern = re.compile(
                rf"^(?:def\s+{re.escape(entrypoint)}\s*\(|{re.escape(entrypoint)}\s*=)",
                re.MULTILINE,
            )
            assert not pattern.search(text), (
                f"{entrypoint} is still a planned production privacy entrypoint "
                f"and must not be publicly defined in {source_path.relative_to(source_root)} "
                "until the production gate passes"
            )


def _snake_entrypoint_name(entrypoint: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", entrypoint).lower()


def _planned_entrypoint_name_variants(entrypoint: str) -> set[str]:
    snake = _snake_entrypoint_name(entrypoint)
    return {
        entrypoint,
        snake,
        snake.replace("ve_range", "verange"),
        snake.replace("zk_at", "zkat"),
    }


def test_privacy_capabilities_do_not_advertise_planned_production_entrypoints() -> None:
    capabilities = privacy_capabilities()
    capability_keys = {
        key
        for key in capabilities
        if key not in {"privacy_algorithms", "privacy_criteria"}
    }
    planned_entrypoints = {
        entrypoint
        for descriptor in get_privacy_algorithm_descriptors()
        for entrypoint in descriptor["planned_sdk_entrypoints"]
    }
    forbidden_status_keys = {
        "shielded_zk_ace_authorized_transfer_instruction",
        "zkat_policy_commitment_instruction",
        "zkat_policy_proof_v1",
        "zkat_authorized_transaction",
        "zk_ams_admission_batch_proof_v0",
        "submit_zk_ams_admission_batch_instruction",
        "vega_credential_predicate_proof_v0",
        "submit_vega_credential_proof_instruction",
        "silent_threshold_credential_showing_proof_v0",
        "submit_silent_threshold_credential_proof_instruction",
        "submit_zk_x509_identity_proof_instruction",
        "jindo_lattice_proof_v0",
        "jindo_polynomial_commitment_v0",
        "sis_hints_anonymous_credential_proof_v0",
        "submit_sis_hints_credential_proof_instruction",
        "orchard_action_bundle_proof_v1",
        "orchard_action_bundle_instruction",
        "penumbra_spend_proof_v1",
        "penumbra_output_proof_v1",
        "penumbra_shielded_pool_transaction",
        "fcmp_plus_plus_membership_proof_v1",
        "fcmp_plus_plus_transfer_instruction",
        "miden_stark_transaction_proof_v1",
        "miden_note_transaction_instruction",
        "aztec_private_kernel_proof_v1",
        "aztec_private_rollup_transaction_instruction",
        "pq_masp_stark_transfer_proof_v0",
        "ml_kem_note_encryption",
    }

    assert planned_entrypoints == set()
    for entrypoint in planned_entrypoints:
        for variant in _planned_entrypoint_name_variants(entrypoint):
            exact_key = _snake_entrypoint_name(variant)
            assert exact_key not in capability_keys, (
                f"{entrypoint} is planned and must not have an executable "
                f"privacy capability key {exact_key}"
            )

    for key in forbidden_status_keys:
        assert capabilities.get(key, False) is False, (
            f"{key} must stay false or absent until the audited production "
            "privacy row is enabled"
        )


def test_privacy_capabilities_uses_client_entrypoints() -> None:
    client = ToriiClient("http://torii.example", max_retries=0)

    capabilities = client.privacy_capabilities()

    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is crypto.is_privacy_native_available()
    assert capabilities["transfer_asset_instruction"] is True
    assert capabilities["shield_instruction"] is True
    assert capabilities["zk_transfer_instruction"] is True
    assert capabilities["unshield_instruction"] is True
    assert capabilities["verify_proof_instruction"] is True
    assert capabilities["zk_ace_register_identity_instruction"] is True
    assert capabilities["zk_ace_rotate_identity_instruction"] is True
    assert capabilities["zk_ace_revoke_identity_instruction"] is True
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True
    assert capabilities["zk_ace_authorization_proof_v1"] is True
    assert capabilities["zk_ace_native_air_prover_v1"] is True
    assert capabilities["zk_ace_validator_support_v1"] is True
    assert capabilities["zk_ace_air_opening_privacy_v1"] is True
    assert capabilities["zk_ace_sdk_exports_v1"] is True
    assert capabilities["verange_commitment_builder_v1"] is True
    assert capabilities["verange_proof_envelope_builder_v1"] is True
    assert capabilities["verange_proof_builder_v1"] is True
    assert capabilities["verange_proof_verifier_v1"] is True
    assert capabilities["verange_dev_fixture_v1"] is True
    assert capabilities["verange_local_verifier_v1"] is True
    assert capabilities["verange_sdk_exports_v1"] is True
    assert capabilities["anonymous_pgc_receiver_set_builder_v1"] is True
    assert capabilities["anonymous_pgc_dev_fixture_v1"] is True
    assert capabilities["anonymous_pgc_local_verifier_v1"] is True
    assert capabilities["anonymous_pgc_sdk_exports_v1"] is True
    assert capabilities["zkat_policy_commitment_builder_v1"] is True
    assert capabilities["zkat_authenticator_envelope_builder_v1"] is True
    assert capabilities["zkat_policy_proof_builder_v1"] is True
    assert capabilities["zkat_policy_proof_verifier_v1"] is True
    assert capabilities["zkat_sdk_exports_v1"] is True
    assert capabilities["zk_ams_admission_batch_builder_v0"] is True
    assert capabilities["zk_ams_proof_envelope_builder_v0"] is True
    assert capabilities["zk_ams_admission_batch_proof_builder_v0"] is True
    assert capabilities["zk_ams_admission_batch_proof_verifier_v0"] is True
    assert capabilities["zk_ams_sdk_exports_v0"] is True
    assert capabilities["vega_predicate_commitment_builder_v0"] is True
    assert capabilities["vega_proof_envelope_builder_v0"] is True
    assert capabilities["vega_credential_predicate_proof_builder_v0"] is True
    assert capabilities["vega_credential_predicate_proof_verifier_v0"] is True
    assert capabilities["vega_sdk_exports_v0"] is True
    assert capabilities["silent_threshold_commitments_builder_v0"] is True
    assert capabilities["silent_threshold_envelope_builder_v0"] is True
    assert capabilities["silent_threshold_credential_proof_builder_v0"] is True
    assert capabilities["silent_threshold_credential_proof_verifier_v0"] is True
    assert capabilities["silent_threshold_sdk_exports_v0"] is True
    assert capabilities["zk_x509_identity_commitments_builder_v0"] is True
    assert capabilities["zk_x509_identity_envelope_builder_v0"] is True
    assert capabilities["zk_x509_identity_proof_builder_v0"] is True
    assert capabilities["zk_x509_identity_proof_verifier_v0"] is True
    assert capabilities["zk_x509_identity_dev_fixture_v0"] is True
    assert capabilities["zk_x509_identity_local_verifier_v0"] is True
    assert capabilities["zk_x509_identity_sdk_exports_v0"] is True
    assert capabilities["jindo_lattice_public_inputs_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_envelope_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_verifier_v0"] is True
    assert capabilities["jindo_lattice_sdk_exports_v0"] is True
    assert capabilities["sis_hints_credential_commitments_builder_v0"] is True
    assert capabilities["sis_hints_credential_envelope_builder_v0"] is True
    assert capabilities["sis_hints_credential_proof_builder_v0"] is True
    assert capabilities["sis_hints_credential_proof_verifier_v0"] is True
    assert capabilities["sis_hints_credential_sdk_exports_v0"] is True
    assert capabilities["asset_hidden_pool_registration_instruction"] is True
    assert capabilities["asset_hidden_transfer_instruction"] is True
    assert capabilities["asset_hidden_transfer_proof_v1"] is True
    assert capabilities["ml_kem_note_encryption"] is False
    assert capabilities["privacy_algorithms"][0]["id"] == "transparent-transfer"


def test_module_privacy_capabilities_defaults_to_static_sdk_surface() -> None:
    capabilities = privacy_capabilities()

    assert set(capabilities) == EXPECTED_PRIVACY_CAPABILITY_KEYS
    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is crypto.is_privacy_native_available()
    assert capabilities["transfer_asset_instruction"] is True
    assert capabilities["verify_proof_instruction"] is True
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True
    assert capabilities["zk_ace_authorization_proof_v1"] is True
    assert capabilities["zk_ace_native_air_prover_v1"] is True
    assert capabilities["zk_ace_validator_support_v1"] is True
    assert capabilities["zk_ace_air_opening_privacy_v1"] is True
    assert capabilities["zk_ace_sdk_exports_v1"] is True
    assert capabilities["verange_sdk_exports_v1"] is True
    assert capabilities["anonymous_pgc_sdk_exports_v1"] is True
    assert capabilities["zkat_sdk_exports_v1"] is True
    assert capabilities["zk_ams_sdk_exports_v0"] is True
    assert capabilities["vega_sdk_exports_v0"] is True
    assert capabilities["silent_threshold_sdk_exports_v0"] is True
    assert capabilities["zk_x509_identity_sdk_exports_v0"] is True
    assert capabilities["jindo_lattice_sdk_exports_v0"] is True
    assert capabilities["sis_hints_credential_sdk_exports_v0"] is True
    assert capabilities["privacy_criteria"] == get_privacy_criteria()


def test_privacy_capabilities_do_not_promote_dev_fixture_or_local_verifier_exports() -> None:
    capabilities = privacy_capabilities()
    component_export_groups = (
        (
            "anonymous_pgc_dev_fixture_v1",
            "anonymous_pgc_local_verifier_v1",
        ),
    )

    for dev_fixture_key, local_verifier_key in component_export_groups:
        assert capabilities[dev_fixture_key] is True
        assert capabilities[local_verifier_key] is True
    assert capabilities["anonymous_pgc_sdk_exports_v1"] is True

    assert capabilities["jindo_lattice_proof_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_verifier_v0"] is True
    assert capabilities["jindo_lattice_sdk_exports_v0"] is True
    assert capabilities["sis_hints_credential_proof_builder_v0"] is True
    assert capabilities["sis_hints_credential_proof_verifier_v0"] is True
    assert capabilities["sis_hints_credential_sdk_exports_v0"] is True


def test_privacy_capabilities_jindo_exports_require_production_entrypoints(
    monkeypatch,
) -> None:
    def callable_on_jindo(name: str) -> bool:
        return name != "buildJindoLatticeProofV0"

    monkeypatch.setattr(privacy_catalog, "_callable_on_jindo", callable_on_jindo)

    capabilities = privacy_capabilities()

    assert capabilities["jindo_lattice_public_inputs_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_envelope_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_builder_v0"] is False
    assert capabilities["jindo_lattice_proof_verifier_v0"] is True
    assert capabilities["jindo_lattice_sdk_exports_v0"] is False


@pytest.mark.parametrize(
    ("capability_key", "public_names", "native_name"),
    [
        (
            "confidential_transfer_proof_v2",
            (
                "buildConfidentialTransferProofV2",
                "build_confidential_transfer_proof_v2",
            ),
            "build_confidential_transfer_proof_v2",
        ),
        (
            "confidential_unshield_proof_v3",
            (
                "buildConfidentialUnshieldProofV3",
                "build_confidential_unshield_proof_v3",
            ),
            "build_confidential_unshield_proof_v3",
        ),
    ],
)
def test_confidential_python_capabilities_require_public_and_native_builders(
    monkeypatch: pytest.MonkeyPatch,
    capability_key: str,
    public_names: tuple[str, str],
    native_name: str,
) -> None:
    def callable_on_crypto(name: str) -> bool:
        return name in public_names

    monkeypatch.setattr(
        privacy_catalog,
        "_callable_on_crypto",
        callable_on_crypto,
    )
    monkeypatch.setattr(
        privacy_catalog,
        "_callable_on_native_crypto",
        lambda name: name == native_name,
    )

    capabilities = privacy_capabilities()

    assert capabilities[capability_key] is True

    monkeypatch.setattr(
        privacy_catalog,
        "_callable_on_native_crypto",
        lambda _name: False,
    )

    disabled = privacy_capabilities()

    assert disabled[capability_key] is False


def test_confidential_python_exports_catalog_named_proof_builders() -> None:
    assert "buildConfidentialTransferProofV2" in crypto.__all__
    assert "build_confidential_transfer_proof_v2" in crypto.__all__
    assert "buildConfidentialUnshieldProofV3" in crypto.__all__
    assert "build_confidential_unshield_proof_v3" in crypto.__all__
    assert "buildConfidentialTransferProofV2" in iroha_python.__all__
    assert "buildConfidentialUnshieldProofV3" in iroha_python.__all__
    assert (
        crypto.buildConfidentialTransferProofV2
        is crypto.build_confidential_transfer_proof_v2
    )
    assert (
        crypto.buildConfidentialUnshieldProofV3
        is crypto.build_confidential_unshield_proof_v3
    )
    assert (
        iroha_python.buildConfidentialTransferProofV2
        is crypto.buildConfidentialTransferProofV2
    )
    assert (
        iroha_python.buildConfidentialUnshieldProofV3
        is crypto.buildConfidentialUnshieldProofV3
    )


def test_confidential_transfer_python_builder_delegates_to_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, tuple[object, ...]] = {}

    class Native:
        @staticmethod
        def build_confidential_transfer_proof_v2(*args: object) -> dict[str, object]:
            captured["args"] = args
            return {
                "nullifiers": [b"n" * 32],
                "output_commitments": [b"o" * 32],
                "root": b"r" * 32,
                "proof": b"proof",
            }

    monkeypatch.setattr(crypto, "_crypto", Native())

    result = crypto.buildConfidentialTransferProofV2(
        chain_id="wonderland",
        asset_definition_id="xor#wonderland",
        spend_key=b"s" * 32,
        tree_commitments=[b"t" * 32],
        inputs=[{"amount": "7", "rho": b"i" * 32, "leaf_index": 0}],
        outputs=[
            {
                "amount": "7",
                "rho": b"u" * 32,
                "owner_tag": b"w" * 32,
            }
        ],
        root_hint=b"r" * 32,
        verifying_key={
            "backend": "halo2/ipa",
            "circuit_id": "confidential_transfer_v2",
            "bytes": b"vk",
        },
    )

    assert result["proof"] == b"proof"
    assert captured["args"] == (
        "wonderland",
        "xor#wonderland",
        b"s" * 32,
        [b"t" * 32],
        [{"amount": "7", "rho": b"i" * 32, "leaf_index": 0}],
        [{"amount": "7", "rho": b"u" * 32, "owner_tag": b"w" * 32}],
        b"r" * 32,
        "halo2/ipa",
        "confidential_transfer_v2",
        b"vk",
    )


@pytest.mark.parametrize(
    ("verifying_key", "message"),
    [
        (
            {
                "backend": " halo2/ipa",
                "circuit_id": "confidential_transfer_v2",
                "bytes": b"vk",
            },
            r"verifying_key\.backend must not contain surrounding whitespace",
        ),
        (
            {
                "backend": "halo2/ipa ",
                "circuit_id": "confidential_transfer_v2",
                "bytes": b"vk",
            },
            r"verifying_key\.backend must not contain surrounding whitespace",
        ),
        (
            {
                "backend": "halo2/ipa",
                "circuit_id": " confidential_transfer_v2",
                "bytes": b"vk",
            },
            r"verifying_key\.circuit_id must not contain surrounding whitespace",
        ),
        (
            {
                "backend": "halo2/ipa",
                "circuit_id": "confidential_transfer_v2 ",
                "bytes": b"vk",
            },
            r"verifying_key\.circuit_id must not contain surrounding whitespace",
        ),
    ],
)
def test_confidential_transfer_python_builder_rejects_padded_verifying_key_metadata(
    monkeypatch: pytest.MonkeyPatch,
    verifying_key: dict[str, object],
    message: str,
) -> None:
    captured: dict[str, bool] = {}

    class Native:
        @staticmethod
        def build_confidential_transfer_proof_v2(*args: object) -> dict[str, object]:
            captured["called"] = True
            raise AssertionError("native prover should not be called")

    monkeypatch.setattr(crypto, "_crypto", Native())

    with pytest.raises(ValueError, match=message):
        crypto.buildConfidentialTransferProofV2(
            chain_id="wonderland",
            asset_definition_id="xor#wonderland",
            spend_key=b"s" * 32,
            tree_commitments=[b"t" * 32],
            inputs=[{"amount": "7", "rho": b"i" * 32, "leaf_index": 0}],
            outputs=[
                {
                    "amount": "7",
                    "rho": b"u" * 32,
                    "owner_tag": b"w" * 32,
                }
            ],
            root_hint=b"r" * 32,
            verifying_key=verifying_key,
        )

    assert captured == {}


def test_confidential_unshield_python_builder_delegates_to_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, tuple[object, ...]] = {}

    class Native:
        @staticmethod
        def build_confidential_unshield_proof_v3(*args: object) -> dict[str, object]:
            captured["args"] = args
            return {
                "nullifiers": [b"n" * 32],
                "output_commitments": [b"o" * 32],
                "root": b"r" * 32,
                "proof": b"proof",
            }

    monkeypatch.setattr(crypto, "_crypto", Native())

    result = crypto.buildConfidentialUnshieldProofV3(
        chain_id="wonderland",
        asset_definition_id="xor#wonderland",
        spend_key=b"s" * 32,
        tree_commitments=[b"t" * 32],
        inputs=[{"amount": "9", "rho": b"i" * 32, "leaf_index": 0}],
        outputs=[{"amount": "2", "rho": b"u" * 32}],
        public_amount=7,
        root_hint=b"r" * 32,
        verifying_key={
            "backend": "halo2/ipa",
            "circuit_id": "confidential_unshield_v3",
            "bytes": b"vk",
        },
    )

    assert result["proof"] == b"proof"
    assert captured["args"] == (
        "wonderland",
        "xor#wonderland",
        b"s" * 32,
        [b"t" * 32],
        [{"amount": "9", "rho": b"i" * 32, "leaf_index": 0}],
        [{"amount": "2", "rho": b"u" * 32}],
        "7",
        b"r" * 32,
        "halo2/ipa",
        "confidential_unshield_v3",
        b"vk",
    )


@pytest.mark.parametrize(
    "missing_builder",
    [
        "build_zk_ace_authorization_proof_v1",
        "zk_ace_build_transfer_authorization_v1",
    ],
)
def test_zk_ace_python_capabilities_require_both_proof_builder_names(
    monkeypatch: pytest.MonkeyPatch,
    missing_builder: str,
) -> None:
    original_callable_on_crypto = privacy_catalog._callable_on_crypto

    def callable_on_crypto_without_one_builder(name: str) -> bool:
        if name == missing_builder:
            return False
        return original_callable_on_crypto(name)

    monkeypatch.setattr(
        privacy_catalog,
        "_callable_on_crypto",
        callable_on_crypto_without_one_builder,
    )

    capabilities = privacy_capabilities()

    assert capabilities["zk_ace_authorization_proof_v1"] is False
    assert capabilities["zk_ace_sdk_exports_v1"] is False
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True


def test_zk_ace_python_exports_catalog_named_proof_builder() -> None:
    assert "build_zk_ace_authorization_proof_v1" in crypto.__all__
    assert "build_zk_ace_authorization_proof_v1" in iroha_python.__all__
    assert "privacy_proof_request_v1" in crypto.__all__
    assert "privacy_proof_request_v1" in iroha_python.__all__
    assert (
        crypto.build_zk_ace_authorization_proof_v1
        is not crypto.zk_ace_build_transfer_authorization_v1
    )
    assert callable(crypto.build_zk_ace_authorization_proof_v1)
    assert callable(crypto.privacy_proof_request_v1)
    assert (
        iroha_python.build_zk_ace_authorization_proof_v1
        is crypto.build_zk_ace_authorization_proof_v1
    )
    assert iroha_python.privacy_proof_request_v1 is crypto.privacy_proof_request_v1


def test_zk_ace_python_catalog_named_proof_builder_delegates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    def fake_transfer_authorization_builder(**kwargs: object) -> dict[str, object]:
        captured.update(kwargs)
        return {"ok": True, "entrypoint": "zk_ace_authorization_proof_v1"}

    monkeypatch.setattr(
        crypto,
        "zk_ace_build_transfer_authorization_v1",
        fake_transfer_authorization_builder,
    )

    result = crypto.build_zk_ace_authorization_proof_v1(
        from_account_id="alice@wonderland",
        to_account_id="bob@wonderland",
        asset_definition_id="xor#wonderland",
        amount="1",
        chain_id="wonderland",
        identity_root=b"identity-root",
        identity_blinding=b"identity-blinding",
        replay_secret=b"replay-secret",
        policy_hash=b"policy-hash",
    )

    assert result == {"ok": True, "entrypoint": "zk_ace_authorization_proof_v1"}
    assert captured == {
        "from_account_id": "alice@wonderland",
        "to_account_id": "bob@wonderland",
        "asset_definition_id": "xor#wonderland",
        "amount": "1",
        "chain_id": "wonderland",
        "identity_root": b"identity-root",
        "identity_blinding": b"identity-blinding",
        "replay_secret": b"replay-secret",
        "policy_hash": b"policy-hash",
    }


def test_zk_ace_python_catalog_named_proof_builder_propagates_native_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def unavailable_transfer_authorization_builder(**_kwargs: object) -> dict[str, object]:
        raise RuntimeError(
            "iroha_python._crypto is missing ZK-ACE prover support; rebuild the extension"
        )

    monkeypatch.setattr(
        crypto,
        "zk_ace_build_transfer_authorization_v1",
        unavailable_transfer_authorization_builder,
    )

    with pytest.raises(RuntimeError, match="missing ZK-ACE prover support"):
        crypto.build_zk_ace_authorization_proof_v1()


def test_zk_ace_python_proof_builder_sanitizes_production_disabled_native_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    secret = b"py-zk-ace-private-secret-1234567"
    proof = "candidate-zk-ace-proof"

    class ProductionDisabledNative:
        def __init__(self) -> None:
            self.calls: list[tuple[object, ...]] = []

        def zk_ace_build_transfer_authorization_v1(self, *args: object) -> str:
            self.calls.append(args)
            raise RuntimeError(
                "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED "
                "zk-ace-pq-authorization-v0 "
                "buildZkAceAuthorizationProofV1 "
                "stark-fri:zk_ace_pq_authorization_v0 "
                f"Iroha production allowlist {secret.decode()} {proof}"
            )

    native = ProductionDisabledNative()
    monkeypatch.setattr(crypto, "_crypto", native)

    with pytest.raises(RuntimeError) as exc_info:
        crypto.build_zk_ace_authorization_proof_v1(
            from_account_id="alice@wonderland",
            to_account_id="bob@wonderland",
            asset_definition_id="xor#wonderland",
            amount="17",
            chain_id="wonderland",
            identity_root=bytes([0x31]) * 32,
            identity_blinding=bytes([0x32]) * 32,
            replay_secret=secret,
            policy_hash=bytes([0x34]) * 32,
            verifier_key_id="stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            vk_commitment=bytes([0x55]) * 32,
        )

    error = exc_info.value
    message = str(error)
    assert "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED" in message
    assert "zk-ace-pq-authorization-v0" in message
    assert "buildZkAceAuthorizationProofV1" in message
    assert "stark-fri:zk_ace_pq_authorization_v0" in message
    assert "Iroha production allowlist" in message
    assert secret.decode() not in message
    assert proof not in message
    assert error.__cause__ is None
    assert error.__context__ is None

    assert len(native.calls) == 1
    assert native.calls[0][0] == "alice@wonderland"
    assert native.calls[0][7] == secret
    assert native.calls[0][9] == "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0"
    assert native.calls[0][10] == bytes([0x55]) * 32


def test_zk_ace_python_transfer_authorization_rejects_malformed_amounts_before_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class CountingNative:
        def __init__(self) -> None:
            self.calls = 0

        def zk_ace_build_transfer_authorization_v1(self, *_args: object) -> str:
            self.calls += 1
            return "{}"

    class HostileAmount:
        stringified = False

        def __str__(self) -> str:
            self.stringified = True
            return "17"

    native = CountingNative()
    hostile_amount = HostileAmount()
    monkeypatch.setattr(crypto, "_crypto", native)

    invalid_amounts: list[object] = [
        None,
        True,
        "",
        " ",
        "0",
        "0000",
        "-1",
        "+1",
        "1.0",
        "1e3",
        0,
        -1,
        (1 << 128),
        b"17",
        hostile_amount,
    ]

    for amount in invalid_amounts:
        with pytest.raises(
            (TypeError, ValueError),
            match="amount must be a positive decimal u128 string",
        ):
            crypto.zk_ace_build_transfer_authorization_v1(
                from_account_id="alice@wonderland",
                to_account_id="bob@wonderland",
                asset_definition_id="xor#wonderland",
                amount=amount,  # type: ignore[arg-type]
                chain_id="wonderland",
                identity_root=bytes([0x31]) * 32,
                identity_blinding=bytes([0x32]) * 32,
                replay_secret=bytes([0x33]) * 32,
                policy_hash=bytes([0x34]) * 32,
            )

    assert native.calls == 0
    assert hostile_amount.stringified is False


def test_zk_ace_python_transfer_authorization_canonicalizes_positive_u128_amounts(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class CapturingNative:
        def __init__(self) -> None:
            self.amounts: list[str] = []

        def zk_ace_build_transfer_authorization_v1(self, *args: object) -> str:
            self.amounts.append(str(args[3]))
            return json.dumps({"ok": True})

    native = CapturingNative()
    monkeypatch.setattr(crypto, "_crypto", native)

    for amount in ("00017", 23, (1 << 128) - 1):
        result = crypto.zk_ace_build_transfer_authorization_v1(
            from_account_id="alice@wonderland",
            to_account_id="bob@wonderland",
            asset_definition_id="xor#wonderland",
            amount=amount,
            chain_id="wonderland",
            identity_root=bytes([0x31]) * 32,
            identity_blinding=bytes([0x32]) * 32,
            replay_secret=bytes([0x33]) * 32,
            policy_hash=bytes([0x34]) * 32,
        )
        assert result == {"ok": True}

    assert native.amounts == ["17", "23", str((1 << 128) - 1)]


def test_zk_ace_python_transfer_authorization_rejects_non_object_native_payload(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class NonObjectPayloadNative:
        @staticmethod
        def zk_ace_build_transfer_authorization_v1(*_args: object) -> str:
            return "[]"

    monkeypatch.setattr(crypto, "_crypto", NonObjectPayloadNative())

    with pytest.raises(RuntimeError, match="non-object payload"):
        crypto.zk_ace_build_transfer_authorization_v1(
            from_account_id="alice@wonderland",
            to_account_id="bob@wonderland",
            asset_definition_id="xor#wonderland",
            amount="1",
            chain_id="wonderland",
            identity_root=b"identity-root",
            identity_blinding=b"identity-blinding",
            replay_secret=b"replay-secret",
            policy_hash=b"policy-hash",
        )


def test_privacy_native_availability_probe_is_exact_bool_and_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto, "is_privacy_native_available", lambda: True)
    assert privacy_catalog._privacy_native_available() is True

    monkeypatch.setattr(crypto, "is_privacy_native_available", lambda: "true")
    assert privacy_catalog._privacy_native_available() is False

    def raise_probe_error() -> bool:
        raise RuntimeError("native privacy probe failed")

    monkeypatch.setattr(crypto, "is_privacy_native_available", raise_probe_error)
    assert privacy_catalog._privacy_native_available() is False


def test_privacy_capabilities_reports_native_bridge_without_production_claims(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(privacy_catalog, "_privacy_native_available", lambda: True)

    capabilities = privacy_capabilities()

    assert capabilities["bridge_available"] is True
    assert all(
        descriptor["production_ready"] is False
        for descriptor in capabilities["privacy_algorithms"]
    )
    assert all(
        descriptor["production_gate"]["ready"] is False
        for descriptor in capabilities["privacy_algorithms"]
    )
    assert all(
        descriptor["production_gate"]["gates"]["external_audit"] is False
        for descriptor in capabilities["privacy_algorithms"]
    )
    for descriptor in capabilities["privacy_algorithms"]:
        assert list(descriptor["production_gate"]["gates"].items()) == (
            _expected_production_gate_items()
        )
        assert descriptor["production_gate"]["required_gates"] == (
            _expected_required_production_gate_keys(descriptor["id"])
        )
        assert descriptor["production_gate"]["missing"] == (
            _expected_production_gate_missing(descriptor["production_gate"])
        )
    zk_ace_capability = next(
        descriptor
        for descriptor in capabilities["privacy_algorithms"]
        if descriptor["id"] == "zk-ace-pq-authorization-v0"
    )
    assert zk_ace_capability["proof_family"] == "stark/fri/sha256-goldilocks"
    assert zk_ace_capability["backend_family"] == "stark-fri"
    assert zk_ace_capability["production_ready"] is False
    assert zk_ace_capability["production_gate"]["ready"] is False
    assert zk_ace_capability["production_gate"]["audit_references"] == []
    assert list(zk_ace_capability["production_gate"]["gates"].items()) == (
        _expected_production_gate_items()
    )
    assert zk_ace_capability["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys(zk_ace_capability["id"])
    )
    assert all(
        ready is False
        for ready in zk_ace_capability["production_gate"]["gates"].values()
    )
    assert "Iroha production allowlist is not enabled for this audited row" in (
        zk_ace_capability["production_gate"]["missing"]
    )


def test_privacy_capabilities_returns_defensive_copies() -> None:
    capabilities = privacy_capabilities()
    capabilities["privacy_algorithms"][0]["id"] = "tampered"
    capabilities["privacy_algorithms"][0]["pq_layers"]["proof"] = "tampered"
    capabilities["privacy_algorithms"][0]["verifier_key_metadata"]["pq_layers"][
        "proof"
    ] = "tampered"
    capabilities["privacy_algorithms"][0]["production_ready"] = True
    capabilities["privacy_algorithms"][0]["production_gate"]["ready"] = True
    capabilities["privacy_algorithms"][0]["production_gate"]["gates"][
        "external_audit"
    ] = True
    real_proving = capabilities["privacy_algorithms"][0]["production_gate"][
        "gates"
    ].pop("real_proving")
    capabilities["privacy_algorithms"][0]["production_gate"]["gates"][
        "real_proving"
    ] = real_proving
    capabilities["privacy_algorithms"][0]["production_gate"][
        "missing"
    ].reverse()
    capabilities["privacy_algorithms"][0]["production_gate"][
        "missing"
    ].clear()
    capabilities["privacy_algorithms"][0]["production_gate"][
        "audit_references"
    ].append({"label": "forged audit", "url": "https://audit.example/forged"})
    sdk_descriptor = next(
        descriptor
        for descriptor in capabilities["privacy_algorithms"][1:]
        if descriptor["sdk_entrypoints"]
    )
    sdk_descriptor_id = sdk_descriptor["id"]
    sdk_descriptor["sdk_entrypoints"].clear()
    source_descriptor = next(
        descriptor
        for descriptor in capabilities["privacy_algorithms"][1:]
        if descriptor["source_references"]
    )
    source_descriptor_id = source_descriptor["id"]
    source_descriptor["source_references"][0]["url"] = "https://audit.example/forged"
    source_descriptor["source_references"].append(
        {"label": "forged source", "url": "https://audit.example/forged"}
    )
    capabilities["privacy_criteria"].append("tampered")

    fresh = privacy_capabilities()

    assert fresh["privacy_algorithms"][0]["id"] == "transparent-transfer"
    assert fresh["privacy_algorithms"][0]["pq_layers"]["proof"] is False
    assert fresh["privacy_algorithms"][0]["verifier_key_metadata"]["pq_layers"][
        "proof"
    ] is False
    assert fresh["privacy_algorithms"][0]["production_ready"] is False
    assert fresh["privacy_algorithms"][0]["production_gate"]["ready"] is False
    assert (
        fresh["privacy_algorithms"][0]["production_gate"]["gates"][
            "external_audit"
        ]
        is False
    )
    assert list(
        fresh["privacy_algorithms"][0]["production_gate"]["gates"].items()
    ) == _expected_production_gate_items()
    assert fresh["privacy_algorithms"][0]["production_gate"]["required_gates"] == (
        _expected_required_production_gate_keys(fresh["privacy_algorithms"][0]["id"])
    )
    assert fresh["privacy_algorithms"][0]["production_gate"]["missing"] == (
        _expected_production_gate_missing(
            fresh["privacy_algorithms"][0]["production_gate"]
        )
    )
    assert "internal cryptographic review signoff is missing" in fresh["privacy_algorithms"][0][
        "production_gate"
    ]["missing"]
    assert (
        fresh["privacy_algorithms"][0]["production_gate"]["audit_references"]
        == []
    )
    fresh_sdk_descriptor = next(
        descriptor
        for descriptor in fresh["privacy_algorithms"]
        if descriptor["id"] == sdk_descriptor_id
    )
    assert fresh_sdk_descriptor["sdk_entrypoints"]
    assert (
        "planned SDK entrypoints remain"
        not in fresh_sdk_descriptor["production_gate"]["missing"]
    )
    fresh_source_descriptor = next(
        descriptor
        for descriptor in fresh["privacy_algorithms"]
        if descriptor["id"] == source_descriptor_id
    )
    assert all(
        source["url"] != "https://audit.example/forged"
        for source in fresh_source_descriptor["source_references"]
    )
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
    assert capabilities["bridge_available"] is crypto.is_privacy_native_available()
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
