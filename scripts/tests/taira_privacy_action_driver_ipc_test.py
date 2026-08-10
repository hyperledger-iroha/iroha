from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts import taira_privacy_action_driver_ipc as ipc


ROOT = Path(__file__).resolve().parents[2]


def _request() -> bytes:
    return ipc.build_verange_request(
        asset_definition_id="verange_value#privacy",
        candidate_binding_sha256="11" * 32,
        creation_time_millis=1_900_000_000_000,
        network_id_hex="23" * 32,
        nonce=17,
        ttl_millis=7_200_000,
    )


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def _verange_public_artifacts(request: dict[str, object]) -> dict[str, object]:
    profile = b"canonical-native-verange-compiled-profile"
    activation = b"canonical-native-verange-relative-activation-template"
    candidate = str(request["candidate_binding_sha256"])
    setup_account = f"i105-qualification-setup-{candidate[:16]}"
    setup_public_key = hashlib.sha256(
        b"test-only-verange-setup-public-key\0" + bytes.fromhex(candidate)
    ).digest()
    setup_requirements: dict[str, object] = {
        "action_authority_account_id": "i105-qualified-authority",
        "action_authority_public_key_hex": "88" * 32,
        "activation_height_rule": ipc.VERANGE_ACTIVATION_HEIGHT_RULE,
        "activation_instruction": ipc.VERANGE_ACTIVATION_INSTRUCTION,
        "activation_lifecycle": ipc.VERANGE_ACTIVATION_LIFECYCLE,
        "activation_minimum_delay_blocks": (
            ipc.VERANGE_ACTIVATION_MINIMUM_DELAY_BLOCKS
        ),
        "activation_template_activate_at_height": (
            ipc.VERANGE_ACTIVATION_TEMPLATE_ACTIVATE_AT_HEIGHT
        ),
        "activation_template_norito_hex": activation.hex(),
        "activation_template_proposed_at_height": (
            ipc.VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT
        ),
        "activation_template_sha256": hashlib.sha256(activation).hexdigest(),
        "asset_definition_id": request["asset_definition_id"],
        "candidate_binding_sha256": candidate,
        "compiled_profile_sha256": hashlib.sha256(profile).hexdigest(),
        "domain_id": ipc.VERANGE_QUALIFICATION_DOMAIN_ID,
        "governance_permission": ipc.VERANGE_GOVERNANCE_PERMISSION,
        "protocol_id": ipc.VERANGE_PROTOCOL,
        "schema": ipc.VERANGE_SETUP_REQUIREMENTS_SCHEMA,
        "schema_version": ipc.VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION,
        "setup_authority_account_id": setup_account,
        "setup_authority_public_key_hex": setup_public_key.hex(),
        "setup_identity_binding_sha256": ipc._verange_setup_identity_binding(
            candidate, setup_account, setup_public_key
        ),
    }
    return {
        "action_authority_account_id": "i105-qualified-authority",
        "action_authority_public_key_hex": "88" * 32,
        "compiled_profile_norito_hex": profile.hex(),
        "compiled_profile_sha256": hashlib.sha256(profile).hexdigest(),
        "engine_id": "native-verange-p256",
        "engine_manifest_digest_hex": "55" * 32,
        "max_aggregation_count": 8,
        "parameter_digest_hex": "44" * 32,
        "parameter_id_hex": "33" * 32,
        "policy_id_hex": "22" * 32,
        "proof_system_id": "iroha-verange-p256",
        "protocol_id": ipc.VERANGE_PROTOCOL,
        "schema": ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA,
        "schema_version": ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION,
        "setup_requirements": setup_requirements,
        "setup_requirements_sha256": hashlib.sha256(
            _canonical(setup_requirements)[:-1]
        ).hexdigest(),
        "statement_schema_digest_hex": "66" * 32,
        "verifier_digest_hex": "77" * 32,
    }


def _response(request: dict[str, object]) -> bytes:
    transaction = b"norito-proof-bearing-transaction"
    operation = str(request["operation"])
    protocol = ipc.PROTOCOL_BY_CONSTRUCTIBLE_OPERATION[operation]
    return _canonical(
        {
            "availability": ipc.protocol_status(protocol),
            "candidate_binding_sha256": request["candidate_binding_sha256"],
            "limitations": list(ipc.protocol_limitations(protocol)),
            "network_outcome_authoritative": False,
            "operation": operation,
            "protocol": protocol,
            "public_admission_artifacts": (
                _verange_public_artifacts(request)
                if protocol == ipc.VERANGE_PROTOCOL
                else None
            ),
            "qualification_scope": ipc.QUALIFICATION_SCOPE,
            "request_id": request["request_id"],
            "schema": ipc.RESPONSE_SCHEMA,
            "schema_version": ipc.SCHEMA_VERSION,
            "transaction_hash_hex": "33" * 32,
            "transaction_norito_hex": transaction.hex(),
            "transaction_sha256": hashlib.sha256(transaction).hexdigest(),
        }
    )


def test_canonical_request_and_typed_response_round_trip() -> None:
    request = ipc.validate_request(_request())
    response = ipc.validate_response(_response(request), expected_request=request)
    assert response["transaction_norito"] == b"norito-proof-bearing-transaction"
    assert response["transaction_hash_hex"] == "33" * 32
    assert response["network_outcome_authoritative"] is False
    assert response["qualification_scope"] == "native-action-construction-only"
    assert response["public_admission_artifacts"]["protocol_id"] == ipc.VERANGE_PROTOCOL


@pytest.mark.parametrize("protocol", tuple(ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL))
def test_every_admitted_operation_has_one_strict_request_response_binding(
    protocol: str,
) -> None:
    payload = ipc.build_action_request(
        asset_definition_id=(
            "pq_note#privacy" if protocol == "pq-masp-stark-v0" else "value#privacy"
        ),
        candidate_binding_sha256="11" * 32,
        creation_time_millis=1_900_000_000_000,
        network_id_hex="23" * 32,
        nonce=17,
        protocol=protocol,
        ttl_millis=7_200_000,
    )
    request = ipc.validate_request(payload)
    assert request["operation"] == ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL[protocol]
    response = ipc.validate_response(_response(request), expected_request=request)
    assert response["operation"] == ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL[protocol]
    assert response["protocol"] == protocol
    assert response["availability"] == ipc.protocol_status(protocol)
    assert response["limitations"] == ipc.protocol_limitations(protocol)
    assert (response["public_admission_artifacts"] is not None) == (
        protocol == ipc.VERANGE_PROTOCOL
    )

    substituted = json.loads(_response(request))
    substituted["protocol"] = next(
        candidate
        for candidate in ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL
        if candidate != protocol
    )
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="context differs"):
        ipc.validate_response(_canonical(substituted), expected_request=request)


def test_python_and_rust_share_one_request_id_golden() -> None:
    path = ROOT / "fixtures/privacy_exact12_action_driver_request_id_v1.json"
    golden = json.loads(path.read_bytes())
    assert set(golden) == {
        "canonical_request",
        "canonical_request_id_body",
        "request",
        "request_id",
        "schema",
        "schema_version",
    }
    assert golden["schema"] == "iroha.taira.privacy_action_driver_request_id_golden"
    assert golden["schema_version"] == 1
    request = dict(golden["request"])
    request_id = request.pop("request_id")
    body = ipc._canonical(request)[:-1]
    assert body.decode("ascii") == golden["canonical_request_id_body"]
    assert request_id == golden["request_id"]
    assert request_id == hashlib.sha256(ipc.REQUEST_ID_DOMAIN + body).hexdigest()
    rebuilt = ipc.build_verange_request(
        asset_definition_id=request["asset_definition_id"],
        candidate_binding_sha256=request["candidate_binding_sha256"],
        creation_time_millis=request["creation_time_millis"],
        network_id_hex=request["network_id_hex"],
        nonce=request["nonce"],
        ttl_millis=request["ttl_millis"],
    )
    assert rebuilt.decode("ascii") == golden["canonical_request"]
    assert ipc.validate_request(rebuilt)["request_id"] == request_id


@pytest.mark.parametrize("mutation", ["suffix", "truncated", "unknown", "request-id"])
def test_request_framing_fails_closed(mutation: str) -> None:
    payload = _request()
    if mutation == "suffix":
        payload += b"\n"
    elif mutation == "truncated":
        payload = payload[:-1]
    else:
        value = json.loads(payload)
        if mutation == "unknown":
            value["endpoint"] = "http://peer.invalid"
        else:
            value["request_id"] = "ff" * 32
        payload = _canonical(value)
    with pytest.raises(ipc.PrivacyActionDriverIpcError):
        ipc.validate_request(payload)


def test_response_context_and_payload_digests_fail_closed() -> None:
    request = ipc.validate_request(_request())
    value = json.loads(_response(request))
    value["transaction_norito_hex"] = "00" + value["transaction_norito_hex"][2:]
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="digest differs"):
        ipc.validate_response(_canonical(value), expected_request=request)


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("availability", "available"),
        ("limitations", []),
        ("network_outcome_authoritative", True),
        ("qualification_scope", "end-to-end-qualified"),
    ),
)
def test_response_cannot_upgrade_construction_to_outcome_evidence(
    field: str, value: object
) -> None:
    request = ipc.validate_request(_request())
    response = json.loads(_response(request))
    response[field] = value
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="context differs"):
        ipc.validate_response(_canonical(response), expected_request=request)


def test_response_rejects_duplicate_fields_and_driver_outcome_claims() -> None:
    request = ipc.validate_request(_request())
    response = _response(request)
    duplicate = response.replace(
        b'{"availability":',
        b'{"operation":"build-verange-action-v1","availability":',
        1,
    )
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="not JSON"):
        ipc.validate_response(duplicate, expected_request=request)
    value = json.loads(response)
    value["status"] = "passed"
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="fields are not exact"):
        ipc.validate_response(_canonical(value), expected_request=request)


def test_response_rejects_an_incomplete_caller_claimed_request_context() -> None:
    request = ipc.validate_request(_request())
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="fields are not exact"):
        ipc.validate_response(
            _response(request),
            expected_request={
                "candidate_binding_sha256": request["candidate_binding_sha256"],
                "request_id": request["request_id"],
            },
        )


@pytest.mark.parametrize(
    ("field", "value", "match"),
    (
        ("setup_authority_admitted", True, "fields are not exact"),
        ("controller_credential", "claimed", "fields are not exact"),
        ("compiled_profile_sha256", "99" * 32, "digest differs"),
        ("max_aggregation_count", 7, "different native profile"),
    ),
)
def test_verange_public_artifacts_reject_caller_setup_and_profile_claims(
    field: str, value: object, match: str
) -> None:
    request = ipc.validate_request(_request())
    response = json.loads(_response(request))
    artifacts = response["public_admission_artifacts"]
    assert isinstance(artifacts, dict)
    artifacts[field] = value
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match=match):
        ipc.validate_response(_canonical(response), expected_request=request)


@pytest.mark.parametrize(
    ("field", "value", "match"),
    (
        ("endpoint", "http://127.0.0.1:8080", "fields are not exact"),
        ("credential", "forbidden", "fields are not exact"),
        ("private_key", "forbidden", "fields are not exact"),
        ("candidate_binding_sha256", "99" * 32, "exact public contract"),
        ("activation_template_norito_hex", b"substituted".hex(), "digest differs"),
        ("setup_identity_binding_sha256", "99" * 32, "exact candidate"),
    ),
)
def test_verange_setup_requirements_reject_substitution_and_network_authority(
    field: str, value: object, match: str
) -> None:
    request = ipc.validate_request(_request())
    response = json.loads(_response(request))
    artifacts = response["public_admission_artifacts"]
    assert isinstance(artifacts, dict)
    requirements = artifacts["setup_requirements"]
    assert isinstance(requirements, dict)
    requirements[field] = value
    artifacts["setup_requirements_sha256"] = hashlib.sha256(
        _canonical(requirements)[:-1]
    ).hexdigest()
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match=match):
        ipc.validate_response(_canonical(response), expected_request=request)


def test_verange_setup_identity_is_network_independent_and_candidate_bound() -> None:
    def request(candidate: str, network: str) -> dict[str, object]:
        return ipc.validate_request(
            ipc.build_verange_request(
                asset_definition_id="verange_value#privacy",
                candidate_binding_sha256=candidate,
                creation_time_millis=1_900_000_000_000,
                network_id_hex=network,
                nonce=17,
                ttl_millis=7_200_000,
            )
        )

    first = _verange_public_artifacts(request("11" * 32, "23" * 32))[
        "setup_requirements"
    ]
    network_substitution = _verange_public_artifacts(
        request("11" * 32, "45" * 32)
    )["setup_requirements"]
    candidate_substitution = _verange_public_artifacts(
        request("33" * 32, "23" * 32)
    )["setup_requirements"]
    setup_fields = (
        "setup_authority_account_id",
        "setup_authority_public_key_hex",
        "setup_identity_binding_sha256",
    )
    assert all(first[field] == network_substitution[field] for field in setup_fields)
    assert all(first[field] != candidate_substitution[field] for field in setup_fields)


def test_verange_has_public_fragments_but_retains_three_completion_blockers() -> None:
    assert ipc.VERANGE_PROTOCOL not in (
        ipc.PROTOCOLS_REQUIRING_UNPRESERVED_ADMISSION_ARTIFACTS
    )
    assert ipc.protocol_limitations(ipc.VERANGE_PROTOCOL) == (
        ipc.MISSING_CONTROLLER_CASE_EVIDENCE,
        *ipc.VERANGE_CONTROLLER_BLOCKERS,
    )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("asset_definition_id", "privacy_☃#asset"),
        ("network_id_hex", "22" * 32),
        ("creation_time_millis", 2**63),
    ],
)
def test_cross_language_input_bounds_fail_closed(field: str, value: object) -> None:
    arguments = {
        "asset_definition_id": "verange_value#privacy",
        "candidate_binding_sha256": "11" * 32,
        "creation_time_millis": 1_900_000_000_000,
        "network_id_hex": "23" * 32,
        "nonce": 17,
        "ttl_millis": 7_200_000,
    }
    arguments[field] = value
    with pytest.raises(ipc.PrivacyActionDriverIpcError):
        ipc.build_verange_request(**arguments)


def test_rust_driver_is_narrow_non_networked_and_builds_a_native_proof_action() -> None:
    source = (
        ROOT / "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
    ).read_text(encoding="utf-8")
    manifest = (ROOT / "crates/iroha_core/Cargo.toml").read_text(encoding="utf-8")
    for builder in (
        "build_privacy_release_zk_ace_network_action_v1",
        "build_privacy_release_anonymous_pgc_network_action_v1",
        "build_privacy_release_verange_network_action_v1",
        "build_privacy_release_vega_network_action_v1",
        "build_privacy_release_jindo_network_action_v1",
        "build_privacy_release_bootle_lantern_network_action_v1",
        "build_privacy_release_orchard_network_action_v1",
        "build_privacy_release_fcmp_network_action_v1",
        "build_privacy_release_ivm_private_note_network_action_v1",
        "build_privacy_release_pq_masp_network_actions_v1",
    ):
        assert builder in source
    assert 'protocol: "iroha-zk-ams-v1"' not in source
    assert 'protocol: "iroha-zk-x509-stark-p256-v0"' not in source
    assert "transaction.encode_versioned()" in source
    assert "use iroha_version::codec::EncodeVersioned;" in source
    request_fields = source.split("struct BuildActionRequestV1 {", 1)[1].split("}", 1)[0]
    assert "values" not in request_fields
    assert "proof" not in request_fields
    assert "witness" not in request_fields
    assert "chain_id" not in request_fields
    assert "genesis_hash_hex" not in request_fields
    assert "network_id_hex" in request_fields
    assert "VERANGE_RELEASE_VALUES.to_vec()" in source
    assert "Zeroizing::new" in source
    assert 'const MAX_ASSET_DEFINITION_ID_BYTES: usize = 1024;' in source
    assert "MAX_CHAIN_ID_BYTES" not in source
    assert (
        "const MAX_CREATION_TIME_MILLIS: u64 = 9_223_372_036_854_775_807;"
        in source
    )
    assert "reqwest" not in source
    assert "iroha::client" not in source
    assert "privacy_exact12_action_driver" in manifest
    assert 'required-features = ["privacy-release-evidence"]' in manifest
    assert "python_and_rust_share_one_request_id_golden" in source
    assert "privacy_exact12_action_driver_request_id_v1.json" in source
    assert "CONSTRUCTIBLE_OPERATION_SPECS_V1" in source
    assert "native-action-construction-only" in source
    assert "MissingSealedControllerProtocolCaseEvidence" in source
    assert "MissingCanonicalAdmissionArtifactBundle" in source
    assert "VeRangePublicAdmissionArtifactsV1" in source
    assert "VeRangeQualificationSetupRequirementsV1" in source
    assert "compiled_profile_norito_hex" in source
    assert "activation_template_norito_hex" in source
    assert "public_admission_artifacts" in source
    setup_seed_body = source.split(
        "fn derive_nonzero_verange_setup_seed(candidate: &[u8; 32])", 1
    )[1].split("\n}", 1)[0]
    assert "request_id" not in setup_seed_body
    assert "network_id" not in setup_seed_body
    setup_fields = source.split(
        "struct VeRangeQualificationSetupRequirementsV1 {", 1
    )[1].split("}", 1)[0]
    assert "private_key" not in setup_fields
    assert "credential" not in setup_fields
    assert "endpoint" not in setup_fields
    assert "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity" in source
    assert "MissingNativePublicOnlyVeRangePolicyActivationTransactionBundle" in source
    assert (
        "MissingFourPeerCanonicalVeRangeCapabilityRowStateQueriesBeforeAfterRestart"
        in source
    )
    assert "available-experimental" in source
    assert "MissingDistributionWideKnowledgeSoundnessEvidence" in source


def test_jindo_is_machine_readable_experimental_without_false_certification() -> None:
    payload = ipc.build_action_request(
        asset_definition_id="value#privacy",
        candidate_binding_sha256="11" * 32,
        creation_time_millis=1_900_000_000_000,
        network_id_hex="23" * 32,
        nonce=17,
        protocol="iroha-jindo-polynomial-commitment-v0",
        ttl_millis=7_200_000,
    )
    request = ipc.validate_request(payload)
    response = ipc.validate_response(_response(request), expected_request=request)
    assert response["availability"] == "available-experimental"
    assert response["limitations"] == (
        "MissingSealedControllerProtocolCaseEvidence",
        "MissingDistributionWideKnowledgeSoundnessEvidence",
    )
    assert response["network_outcome_authoritative"] is False


def test_clean_v1_request_has_only_network_identity() -> None:
    request = ipc.validate_request(_request())
    assert request["network_id_hex"] == "23" * 32
    assert {"chain_id", "genesis_hash_hex"}.isdisjoint(request)
