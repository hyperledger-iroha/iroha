from __future__ import annotations

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
        "bridge_available",
        "confidential_transfer_proof_v2",
        "confidential_unshield_proof_v3",
        "jindo_lattice_dev_fixture_v0",
        "jindo_lattice_local_verifier_v0",
        "jindo_lattice_proof_envelope_builder_v0",
        "jindo_lattice_public_inputs_builder_v0",
        "jindo_lattice_sdk_exports_v0",
        "ml_dsa_authorization",
        "ml_kem_note_encryption",
        "privacy_algorithms",
        "privacy_criteria",
        "python_sdk_available",
        "shield_instruction",
        "silent_threshold_commitments_builder_v0",
        "silent_threshold_dev_fixture_v0",
        "silent_threshold_envelope_builder_v0",
        "silent_threshold_local_verifier_v0",
        "silent_threshold_sdk_exports_v0",
        "sis_hints_credential_commitments_builder_v0",
        "sis_hints_credential_dev_fixture_v0",
        "sis_hints_credential_envelope_builder_v0",
        "sis_hints_credential_local_verifier_v0",
        "sis_hints_credential_sdk_exports_v0",
        "stark_proof_family",
        "transfer_asset_instruction",
        "unshield_instruction",
        "vega_dev_fixture_v0",
        "vega_local_verifier_v0",
        "vega_predicate_commitment_builder_v0",
        "vega_proof_envelope_builder_v0",
        "vega_sdk_exports_v0",
        "verange_commitment_builder_v1",
        "verange_dev_fixture_v1",
        "verange_local_verifier_v1",
        "verange_proof_envelope_builder_v1",
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
        "zk_ams_dev_fixture_v0",
        "zk_ams_local_verifier_v0",
        "zk_ams_proof_envelope_builder_v0",
        "zk_ams_sdk_exports_v0",
        "zk_transfer_instruction",
        "zk_x509_identity_commitments_builder_v0",
        "zk_x509_identity_dev_fixture_v0",
        "zk_x509_identity_envelope_builder_v0",
        "zk_x509_identity_local_verifier_v0",
        "zk_x509_identity_sdk_exports_v0",
        "zkat_authenticator_envelope_builder_v1",
        "zkat_dev_fixture_v1",
        "zkat_local_verifier_v1",
        "zkat_policy_commitment_builder_v1",
        "zkat_sdk_exports_v1",
    }
)


def _expected_production_gate_items() -> list[tuple[str, bool]]:
    return [(key, False) for key, _label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS]


def _expected_production_gate_missing(gate: dict[str, object]) -> list[str]:
    missing = gate["missing"]
    assert isinstance(missing, list)
    return [
        *(label for _key, label in privacy_catalog.PRODUCTION_GATE_REQUIREMENTS),
        *(
            reason
            for reason in privacy_catalog.PRODUCTION_GATE_SUPPLEMENTAL_MISSING_REASONS
            if reason in missing
        ),
    ]


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
        assert descriptor["planned_sdk_entrypoints"]
        assert descriptor["production_ready"] is False


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


@pytest.mark.parametrize(
    "planned_entrypoints",
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
    ],
)
def test_privacy_catalog_rejects_required_production_privacy_plan_without_proof_builder(
    planned_entrypoints,
) -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "anonymous-pgc-k-out-of-n-v1":
            descriptor["planned_sdk_entrypoints"] = planned_entrypoints
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "required production privacy plan row "
            "'anonymous-pgc-k-out-of-n-v1' must retain a planned production "
            "proof builder until production gates pass"
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
    research_target_ids = {
        descriptor["id"]
        for descriptor in descriptors
        if descriptor["implementation_stage"] == "research-target-as-of-2026-05"
    }
    assert research_target_ids == set(
        privacy_catalog._RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID
    )

    for descriptor in descriptors:
        if descriptor["implementation_stage"] != "research-target-as-of-2026-05":
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
            token in security_notes_text
            for token in privacy_catalog._RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS
        )
        assert any(
            token in security_notes_text
            for token in privacy_catalog._RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS
        )


def test_privacy_catalog_rejects_research_target_executable_sdk_entrypoint() -> None:
    descriptors = json.loads(json.dumps(get_privacy_algorithm_descriptors()))
    for descriptor in descriptors:
        if descriptor["id"] == "orchard-halo2-actions-v1":
            descriptor["sdk_entrypoints"] = ["verifySharedOrchardProof"]
            break

    with pytest.raises(
        RuntimeError,
        match=(
            "research target 'orchard-halo2-actions-v1' cannot advertise "
            "executable SDK entrypoints"
        ),
    ):
        privacy_catalog._validate_research_target_sdk_entrypoints(tuple(descriptors))


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
                    chainRequirements=["Orchard note commitment tree"],
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
                        "Hardening gates require parser fuzzing, performance review, and external audit.",
                    ],
                    recommendedFor=["shape research"],
                    chainRequirements=["Orchard note commitment tree"],
                    requiredState=[
                        "Orchard note commitment tree",
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
            [{"label": "External audit signoff", "url": "https://zips.z.cash/zip-0224"}],
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
            ["External audit completed and production sign-off received."],
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
            ["External audit completed."],
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


def test_privacy_catalog_loader_rejects_backend_family_production_claims(
    monkeypatch,
) -> None:
    patched_backend_families = dict(privacy_catalog.BACKEND_FAMILY_BY_ALGORITHM_ID)
    patched_backend_families["shield"] = "stark-fri-mainnet-ready"
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
        "chain-executable",
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
        "chain-executable",
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
        "chain-executable",
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
        id="transparent-transfer",
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
        "chain-executable",
        "sdk-builder",
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
            else ["buildShapeProof"]
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
            "planned_sdk_entrypoints' must be non-empty for pre-production "
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
            ["buildPqMaspStarkTransferProofV0", "generateMlDsaKeyPair"],
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
            "planned_sdk_entrypoints' must include planned ML-DSA "
            "authorization and ML-KEM note-encryption SDK entrypoints"
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
            {"failureModes": ["ML-KEM domain mismatch"]},
            "failure_modes",
            "ML-DSA",
        ),
        (
            {"requiredState": ["PQ nullifier set"]},
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
                        "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
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
    assert "implementation stage is not production-hardened" not in gate["missing"]
    assert "dev fixture entrypoints are not production entrypoints" not in gate[
        "missing"
    ]
    assert gate["missing"] == _expected_production_gate_missing(gate)
    assert "Iroha production allowlist is not enabled for this audited row" in gate[
        "missing"
    ]


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
                        "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
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
                        "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
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
            "security_notes' must include audit/review, fuzzing, and "
            "performance hardening gates for source-referenced entries"
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
                        "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
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
            "production SDK entrypoints until production gates pass"
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
    descriptors[0]["production_gate"]["missing"].reverse()
    descriptors[0]["production_gate"]["missing"].clear()
    descriptors[0]["production_gate"]["audit_references"].append(
        {"label": "forged audit", "url": "https://audit.example/forged"}
    )
    planned = next(
        descriptor for descriptor in descriptors if descriptor["planned_sdk_entrypoints"]
    )
    planned_id = planned["id"]
    planned["planned_sdk_entrypoints"].clear()
    source_descriptor = next(
        descriptor for descriptor in descriptors if descriptor["source_references"]
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
    assert fresh_descriptors[0]["production_gate"]["missing"] == (
        _expected_production_gate_missing(fresh_descriptors[0]["production_gate"])
    )
    assert "external audit signoff is missing" in fresh_descriptors[0][
        "production_gate"
    ]["missing"]
    assert fresh_descriptors[0]["production_gate"]["audit_references"] == []
    fresh_planned = next(
        descriptor for descriptor in fresh_descriptors if descriptor["id"] == planned_id
    )
    assert fresh_planned["planned_sdk_entrypoints"]
    assert "planned SDK entrypoints remain" in fresh_planned["production_gate"]["missing"]
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
    descriptor["planned_sdk_entrypoints"].clear()
    descriptor["source_references"][0]["label"] = "forged source"
    descriptor["verifier_key_metadata"]["pq_layers"]["proof"] = False
    descriptor["production_ready"] = True
    descriptor["production_gate"]["ready"] = True
    descriptor["production_gate"]["missing"].reverse()
    descriptor["production_gate"]["audit_references"].append(
        {"label": "forged audit", "url": "https://audit.example/forged"}
    )

    fresh = get_privacy_algorithm_descriptor("pq-masp-stark-v0")
    assert fresh is not None
    assert "post_quantum" in fresh["covered_criteria"]
    assert fresh["pq_layers"]["proof"] is True
    assert fresh["planned_sdk_entrypoints"]
    assert fresh["source_references"][0]["label"] != "forged source"
    assert fresh["verifier_key_metadata"]["pq_layers"]["proof"] is True
    assert fresh["production_ready"] is False
    assert fresh["production_gate"]["ready"] is False
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
        assert all(
            ready is False
            for ready in descriptor["production_gate"]["gates"].values()
        )
        assert descriptor["production_gate"]["audit_references"] == []
        assert descriptor["production_gate"]["missing"] == (
            _expected_production_gate_missing(descriptor["production_gate"])
        )
        assert "external audit signoff is missing" in descriptor[
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

    zk_ace = by_id["zk-ace-pq-authorization-v0"]
    assert zk_ace["implementation_stage"] == "chain-executable"
    assert zk_ace["sdk_entrypoints"] == [
        "buildRegisterZkAceIdentityCommitmentInstruction",
        "buildRotateZkAceIdentityCommitmentInstruction",
        "buildRevokeZkAceIdentityCommitmentInstruction",
        "buildZkAceAuthorizedTransferInstruction",
    ]
    assert zk_ace["planned_sdk_entrypoints"] == [
        "buildZkAceAuthorizationProofV1",
        "buildShieldedZkAceAuthorizedTransferInstruction",
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
    assert "dev fixture entrypoints are not production entrypoints" in anonymous_pgc[
        "production_gate"
    ]["missing"]
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
        "buildZkAtDevProofFixture",
        "verifyZkAtAuthenticatorLocally",
    ]
    assert zkat["planned_sdk_entrypoints"] == [
        "buildZkAtPolicyCommitmentInstruction",
        "buildZkAtPolicyProofV1",
        "buildZkAtAuthorizedTransaction",
    ]
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
                token in value
                for token in privacy_catalog._LEDGER_MUTATION_PROTECTION_METADATA_TOKENS
                for value in protection_values
            )
            typed_admission_text = " ".join(
                value.lower()
                for field in privacy_catalog._TYPED_CHAIN_ADMISSION_METADATA_FIELDS
                for value in descriptor[field]
            )
            assert any(
                token in typed_admission_text
                for token in privacy_catalog._TYPED_CHAIN_ADMISSION_TYPE_TOKENS
            )
            assert any(
                token in typed_admission_text
                for token in privacy_catalog._TYPED_CHAIN_ADMISSION_MUTATION_TOKENS
            )
            required_state_text = " ".join(descriptor["required_state"]).lower()
            if any(
                token in required_state_text
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
                    assert any(token in persistence_text for token in tokens)
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES
            and descriptor["category"]
            not in privacy_catalog._WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES
        ):
            required_state_text = " ".join(descriptor["required_state"]).lower()
            assert any(
                token in required_state_text
                for token in privacy_catalog._WALLET_STATE_METADATA_TOKENS
            )
            security_notes_text = " ".join(descriptor["security_notes"]).lower()
            for tokens in privacy_catalog._WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS:
                assert any(token in security_notes_text for token in tokens)
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
            and descriptor["category"]
            in privacy_catalog._CREDENTIAL_STATE_REQUIRED_CATEGORIES
        ):
            required_state_text = " ".join(descriptor["required_state"]).lower()
            assert any(
                token in required_state_text
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
                assert any(token in failure_modes_text for token in tokens)
            verifier_key_record_text = " ".join(
                value.lower()
                for field in privacy_catalog._VERIFIER_KEY_RECORD_METADATA_FIELDS
                for value in descriptor[field]
            )
            assert any(
                token in verifier_key_record_text
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
                token in chain_domain_binding_text
                for token in privacy_catalog._CHAIN_DOMAIN_BINDING_METADATA_TOKENS
            )
        if (
            descriptor["implementation_stage"]
            in privacy_catalog._SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        ):
            security_notes_text = " ".join(descriptor["security_notes"]).lower()
            for tokens in privacy_catalog._SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS:
                assert any(token in security_notes_text for token in tokens)
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
                token in security_notes_text
                for token in (
                    privacy_catalog._RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS
                )
            )
            assert any(
                token in security_notes_text
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
            planned_entrypoint_names = [
                entrypoint.rsplit(".", 1)[-1]
                for entrypoint in descriptor["planned_sdk_entrypoints"]
            ]
            for fragment in (
                privacy_catalog._POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS
            ):
                assert any(fragment in name for name in planned_entrypoint_names)
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
                    assert any(token in value for value in values)


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

    assert planned_entrypoints
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

    assert planned_entrypoints
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
        "asset_hidden_transfer_proof_v1",
        "shielded_zk_ace_authorized_transfer_instruction",
        "anonymous_pgc_account_commitment_instruction",
        "anonymous_pgc_k_out_of_n_proof_v1",
        "anonymous_pgc_transfer_instruction",
        "verange_proof_v1",
        "zkat_policy_commitment_instruction",
        "zkat_policy_proof_v1",
        "zkat_authorized_transaction",
        "zk_ams_admission_batch_proof_v0",
        "submit_zk_ams_admission_batch_instruction",
        "vega_credential_predicate_proof_v0",
        "submit_vega_credential_proof_instruction",
        "silent_threshold_credential_showing_proof_v0",
        "submit_silent_threshold_credential_proof_instruction",
        "zk_x509_identity_proof_v0",
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

    assert planned_entrypoints
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
    assert capabilities["verange_commitment_builder_v1"] is True
    assert capabilities["verange_proof_envelope_builder_v1"] is True
    assert capabilities["verange_dev_fixture_v1"] is True
    assert capabilities["verange_local_verifier_v1"] is True
    assert capabilities["verange_sdk_exports_v1"] is True
    assert capabilities["anonymous_pgc_receiver_set_builder_v1"] is True
    assert capabilities["anonymous_pgc_dev_fixture_v1"] is True
    assert capabilities["anonymous_pgc_local_verifier_v1"] is True
    assert capabilities["anonymous_pgc_sdk_exports_v1"] is True
    assert capabilities["zkat_policy_commitment_builder_v1"] is True
    assert capabilities["zkat_authenticator_envelope_builder_v1"] is True
    assert capabilities["zkat_dev_fixture_v1"] is True
    assert capabilities["zkat_local_verifier_v1"] is True
    assert capabilities["zkat_sdk_exports_v1"] is True
    assert capabilities["zk_ams_admission_batch_builder_v0"] is True
    assert capabilities["zk_ams_proof_envelope_builder_v0"] is True
    assert capabilities["zk_ams_dev_fixture_v0"] is True
    assert capabilities["zk_ams_local_verifier_v0"] is True
    assert capabilities["zk_ams_sdk_exports_v0"] is True
    assert capabilities["vega_predicate_commitment_builder_v0"] is True
    assert capabilities["vega_proof_envelope_builder_v0"] is True
    assert capabilities["vega_dev_fixture_v0"] is True
    assert capabilities["vega_local_verifier_v0"] is True
    assert capabilities["vega_sdk_exports_v0"] is True
    assert capabilities["silent_threshold_commitments_builder_v0"] is True
    assert capabilities["silent_threshold_envelope_builder_v0"] is True
    assert capabilities["silent_threshold_dev_fixture_v0"] is True
    assert capabilities["silent_threshold_local_verifier_v0"] is True
    assert capabilities["silent_threshold_sdk_exports_v0"] is True
    assert capabilities["zk_x509_identity_commitments_builder_v0"] is True
    assert capabilities["zk_x509_identity_envelope_builder_v0"] is True
    assert capabilities["zk_x509_identity_dev_fixture_v0"] is True
    assert capabilities["zk_x509_identity_local_verifier_v0"] is True
    assert capabilities["zk_x509_identity_sdk_exports_v0"] is True
    assert capabilities["jindo_lattice_public_inputs_builder_v0"] is True
    assert capabilities["jindo_lattice_proof_envelope_builder_v0"] is True
    assert capabilities["jindo_lattice_dev_fixture_v0"] is True
    assert capabilities["jindo_lattice_local_verifier_v0"] is True
    assert capabilities["jindo_lattice_sdk_exports_v0"] is True
    assert capabilities["sis_hints_credential_commitments_builder_v0"] is True
    assert capabilities["sis_hints_credential_envelope_builder_v0"] is True
    assert capabilities["sis_hints_credential_dev_fixture_v0"] is True
    assert capabilities["sis_hints_credential_local_verifier_v0"] is True
    assert capabilities["sis_hints_credential_sdk_exports_v0"] is True
    assert capabilities["asset_hidden_transfer_instruction"] is False
    assert capabilities["ml_kem_note_encryption"] is False
    assert capabilities["privacy_algorithms"][0]["id"] == "transparent-transfer"


def test_module_privacy_capabilities_defaults_to_static_sdk_surface() -> None:
    capabilities = privacy_capabilities()

    assert set(capabilities) == EXPECTED_PRIVACY_CAPABILITY_KEYS
    assert capabilities["python_sdk_available"] is True
    assert capabilities["bridge_available"] is False
    assert capabilities["transfer_asset_instruction"] is True
    assert capabilities["zk_ace_identity_lifecycle_instruction"] is True
    assert capabilities["zk_ace_authorized_transfer_instruction"] is True
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
        assert descriptor["production_gate"]["missing"] == (
            _expected_production_gate_missing(descriptor["production_gate"])
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
    planned = next(
        descriptor
        for descriptor in capabilities["privacy_algorithms"]
        if descriptor["planned_sdk_entrypoints"]
    )
    planned_id = planned["id"]
    planned["planned_sdk_entrypoints"].clear()
    source_descriptor = next(
        descriptor
        for descriptor in capabilities["privacy_algorithms"]
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
    assert fresh["privacy_algorithms"][0]["production_gate"]["missing"] == (
        _expected_production_gate_missing(
            fresh["privacy_algorithms"][0]["production_gate"]
        )
    )
    assert "external audit signoff is missing" in fresh["privacy_algorithms"][0][
        "production_gate"
    ]["missing"]
    assert (
        fresh["privacy_algorithms"][0]["production_gate"]["audit_references"]
        == []
    )
    fresh_planned = next(
        descriptor
        for descriptor in fresh["privacy_algorithms"]
        if descriptor["id"] == planned_id
    )
    assert fresh_planned["planned_sdk_entrypoints"]
    assert "planned SDK entrypoints remain" in fresh_planned["production_gate"]["missing"]
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
