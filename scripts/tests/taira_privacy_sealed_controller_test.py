from __future__ import annotations

import hashlib
import json
import os
from dataclasses import replace
from pathlib import Path
import sys
import time
import types

import pytest

from scripts import deploy_taira_v21_reset as deploy
from scripts import taira_privacy_sealed_controller as controller


ROOT = Path(__file__).resolve().parents[2]
DRIVER_SHA256 = "99" * 32


def _json_response(value: object, status: int = 200) -> controller.HttpObservation:
    return controller.HttpObservation(
        status,
        {"content-type": "application/json"},
        json.dumps(value, separators=(",", ":")).encode("ascii"),
    )


def _request(candidate: str, nonce: int) -> controller.VeRangeActionRequest:
    return controller.VeRangeActionRequest(
        asset_definition_id="verange_value#privacy",
        candidate_binding_sha256=candidate,
        creation_time_millis=1_800_000_000_000 + nonce,
        network_id_hex="23" * 32,
        nonce=nonce,
        ttl_millis=120_000,
    )


def _public_artifacts(
    request: controller.VeRangeActionRequest,
) -> dict[str, object]:
    profile = b"canonical-native-verange-compiled-profile"
    activation = b"canonical-native-verange-relative-activation-template"
    setup_account = f"i105-qualification-setup-{request.candidate_binding_sha256[:16]}"
    setup_public_key = hashlib.sha256(
        b"test-only-verange-setup-public-key\0"
        + bytes.fromhex(request.candidate_binding_sha256)
    ).digest()
    setup_requirements: dict[str, object] = {
        "action_authority_account_id": "i105-qualified-authority",
        "action_authority_public_key_hex": "88" * 32,
        "activation_height_rule": controller.action_ipc.VERANGE_ACTIVATION_HEIGHT_RULE,
        "activation_instruction": controller.action_ipc.VERANGE_ACTIVATION_INSTRUCTION,
        "activation_lifecycle": controller.action_ipc.VERANGE_ACTIVATION_LIFECYCLE,
        "activation_minimum_delay_blocks": (
            controller.action_ipc.VERANGE_ACTIVATION_MINIMUM_DELAY_BLOCKS
        ),
        "activation_template_activate_at_height": (
            controller.action_ipc.VERANGE_ACTIVATION_TEMPLATE_ACTIVATE_AT_HEIGHT
        ),
        "activation_template_norito_hex": activation.hex(),
        "activation_template_proposed_at_height": (
            controller.action_ipc.VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT
        ),
        "activation_template_sha256": hashlib.sha256(activation).hexdigest(),
        "asset_definition_id": request.asset_definition_id,
        "candidate_binding_sha256": request.candidate_binding_sha256,
        "compiled_profile_sha256": hashlib.sha256(profile).hexdigest(),
        "domain_id": controller.action_ipc.VERANGE_QUALIFICATION_DOMAIN_ID,
        "governance_permission": controller.action_ipc.VERANGE_GOVERNANCE_PERMISSION,
        "protocol_id": controller.VERANGE_PROTOCOL,
        "schema": controller.action_ipc.VERANGE_SETUP_REQUIREMENTS_SCHEMA,
        "schema_version": (
            controller.action_ipc.VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION
        ),
        "setup_authority_account_id": setup_account,
        "setup_authority_public_key_hex": setup_public_key.hex(),
        "setup_identity_binding_sha256": (
            controller.action_ipc._verange_setup_identity_binding(
                request.candidate_binding_sha256, setup_account, setup_public_key
            )
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
        "policy_id_hex": controller._driver_seed_v1(request, 2).hex(),
        "proof_system_id": "iroha-verange-p256",
        "protocol_id": controller.VERANGE_PROTOCOL,
        "schema": controller.action_ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA,
        "schema_version": (
            controller.action_ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION
        ),
        "setup_requirements": setup_requirements,
        "setup_requirements_sha256": hashlib.sha256(
            controller._driver_json_bytes(setup_requirements)[:-1]
        ).hexdigest(),
        "statement_schema_digest_hex": "66" * 32,
        "verifier_digest_hex": "77" * 32,
    }


def _artifact(request: controller.VeRangeActionRequest, marker: int) -> controller.ActionArtifact:
    request_bytes = request.canonical_bytes()
    request_body = json.loads(request_bytes)
    transaction = b"NRT0" + bytes([marker]) * 64
    transaction_sha = hashlib.sha256(transaction).hexdigest()
    transaction_hash = f"{marker:02x}" * 32
    public_artifacts = _public_artifacts(request)
    response = {
        "availability": controller.action_ipc.CONSTRUCTION_ONLY_STATUS,
        "candidate_binding_sha256": request.candidate_binding_sha256,
        "limitations": list(
            controller.action_ipc.protocol_limitations(controller.VERANGE_PROTOCOL)
        ),
        "network_outcome_authoritative": False,
        "operation": controller.VERANGE_OPERATION,
        "protocol": controller.VERANGE_PROTOCOL,
        "public_admission_artifacts": public_artifacts,
        "qualification_scope": controller.action_ipc.QUALIFICATION_SCOPE,
        "request_id": request_body["request_id"],
        "schema": controller.DRIVER_RESPONSE_SCHEMA,
        "schema_version": controller.DRIVER_SCHEMA_VERSION,
        "transaction_hash_hex": transaction_hash,
        "transaction_norito_hex": transaction.hex(),
        "transaction_sha256": transaction_sha,
    }
    return controller.ActionArtifact(
        availability=controller.action_ipc.CONSTRUCTION_ONLY_STATUS,
        limitations=controller.action_ipc.protocol_limitations(
            controller.VERANGE_PROTOCOL
        ),
        network_outcome_authoritative=False,
        qualification_scope=controller.action_ipc.QUALIFICATION_SCOPE,
        request_id=request_body["request_id"],
        request_bytes=request_bytes,
        response_bytes=controller._driver_json_bytes(response),
        transaction=transaction,
        transaction_hash_hex=transaction_hash,
        transaction_sha256=transaction_sha,
        action_driver_sha256=DRIVER_SHA256,
        operation=controller.VERANGE_OPERATION,
        protocol=controller.VERANGE_PROTOCOL,
        public_admission_artifacts=controller.VeRangePublicAdmissionArtifactsV1.from_ipc(
            controller.action_ipc._verange_public_admission_artifacts(
                public_artifacts, expected_request=request_body
            )
        ),
    )


def _peers() -> tuple[controller.PeerEndpoint, ...]:
    return tuple(
        controller.PeerEndpoint(f"peer-{index}", f"http://127.0.0.1:{8080 + index}")
        for index in range(1, 5)
    )


def test_release_issuance_barrier_requires_real_controller_cases_not_constructors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("IROHA_PRIVACY_ALLOW_SELF_REPORTED_RECEIPT", "1")
    monkeypatch.setenv("IROHA_PRIVACY_VERANGE_SETUP_AUTHORITY_ADMITTED", "1")
    monkeypatch.setenv("IROHA_PRIVACY_VERANGE_SETUP_BUNDLE", "claimed")
    monkeypatch.setenv("IROHA_PRIVACY_VERANGE_CAPABILITY_QUERIES", "claimed")
    assert controller.missing_constructible_operations() == (
        "iroha-zk-ams-v1",
        "iroha-zk-x509-stark-p256-v0",
    )
    missing = controller.missing_release_operations()
    assert missing == controller.RETAINED_PROTOCOLS
    with pytest.raises(
        controller.SealedPrivacyControllerError,
        match="release issuance is closed",
    ) as raised:
        controller.require_complete_release_operation_surface()
    assert controller.VERANGE_PROTOCOL in str(raised.value)
    assert "iroha-zk-ams-v1" in str(raised.value)
    for residual in controller.action_ipc.VERANGE_CONTROLLER_BLOCKERS:
        assert residual in str(raised.value)
    with pytest.raises(TypeError):
        controller.CONTROLLER_CASE_RUNNERS[controller.VERANGE_PROTOCOL] = "fake"  # type: ignore[index]
    with pytest.raises(TypeError):
        controller.CONTROLLER_CASE_BLOCKERS[controller.VERANGE_PROTOCOL] = ()  # type: ignore[index]

    monkeypatch.setattr(
        controller,
        "CONSTRUCTIBLE_OPERATIONS",
        types.MappingProxyType(
            {
                **dict(controller.CONSTRUCTIBLE_OPERATIONS),
                "iroha-zk-ams-v1": "build-zk-ams-action-v1",
                "iroha-zk-x509-stark-p256-v0": "build-zk-x509-action-v1",
            }
        ),
    )
    assert controller.missing_constructible_operations() == ()
    assert controller.missing_release_operations() == controller.RETAINED_PROTOCOLS
    with pytest.raises(controller.SealedPrivacyControllerError, match="issuance is closed"):
        controller.require_complete_release_operation_surface()


def test_caller_and_environment_claims_cannot_remove_verange_blockers(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for key in (
        "IROHA_PRIVACY_VERANGE_SETUP_AUTHORITY_ADMITTED",
        "IROHA_PRIVACY_VERANGE_SETUP_BUNDLE",
        "IROHA_PRIVACY_VERANGE_CAPABILITY_QUERIES",
    ):
        monkeypatch.setenv(key, "true")
    assert controller.controller_case_blockers(controller.VERANGE_PROTOCOL) == (
        controller.action_ipc.VERANGE_CONTROLLER_BLOCKERS
    )
    with pytest.raises(TypeError):
        controller.require_complete_release_operation_surface(  # type: ignore[call-arg]
            setup_authority_admitted=True
        )
    response = json.loads(_artifact(_request("11" * 32, 7), 0x33).response_bytes)
    response["setup_authority_admitted"] = True
    request_bytes = _request("11" * 32, 7).canonical_bytes()
    with pytest.raises(
        controller.SealedPrivacyControllerError,
        match="canonical closed JSON object|fields are not exact",
    ):
        controller._parse_action_response(
            controller._driver_json_bytes(response), request_bytes
        )


def test_constructible_table_is_byte_equal_to_ipc_and_case_table_is_empty() -> None:
    assert dict(controller.CONSTRUCTIBLE_OPERATIONS) == dict(
        controller.action_ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL
    )
    assert len(controller.CONSTRUCTIBLE_OPERATIONS) == 10
    assert set(controller.CONSTRUCTIBLE_OPERATIONS).isdisjoint(
        {"iroha-zk-ams-v1", "iroha-zk-x509-stark-p256-v0"}
    )
    assert dict(controller.CONTROLLER_CASE_RUNNERS) == {}
    assert set(controller.CONTROLLER_CASE_BLOCKERS) == set(
        controller.RETAINED_PROTOCOLS
    )
    assert controller.controller_case_blockers(controller.VERANGE_PROTOCOL) == (
        controller.action_ipc.VERANGE_CONTROLLER_BLOCKERS
    )
    assert controller.VERANGE_PLANNED_CONTROLLER_CASE.blockers == (
        controller.action_ipc.VERANGE_CONTROLLER_BLOCKERS
    )
    assert controller.VERANGE_PLANNED_CONTROLLER_CASE.state_query_path == (
        "/v1/privacy/capabilities"
    )


def test_driver_request_is_canonical_bounded_and_contains_no_network_authority() -> None:
    payload = _request("11" * 32, 7).canonical_bytes()
    assert payload.endswith(b"\n")
    assert len(payload) <= controller.MAX_DRIVER_REQUEST_BYTES
    assert b"endpoint" not in payload
    assert b"credential" not in payload
    assert b"password" not in payload
    assert b"values" not in payload
    assert b"witness" not in payload
    assert b"chain_id" not in payload
    assert b"genesis_hash_hex" not in payload
    assert b"network_id_hex" in payload
    parsed = json.loads(payload)
    request_id = parsed.pop("request_id")
    assert request_id == hashlib.sha256(
        controller.REQUEST_ID_DOMAIN + controller._driver_json_bytes(parsed)[:-1]
    ).hexdigest()


def test_driver_response_rejects_digest_shell_suffix_and_duplicate_fields() -> None:
    request = _request("11" * 32, 7)
    artifact = _artifact(request, 0x33)
    parsed = controller._parse_action_response(
        artifact.response_bytes, artifact.request_bytes
    )
    assert parsed.transaction == artifact.transaction

    response = json.loads(artifact.response_bytes)
    response["transaction_norito_hex"] = "44"
    with pytest.raises(controller.SealedPrivacyControllerError, match="digest differs"):
        controller._parse_action_response(
            controller._driver_json_bytes(response), artifact.request_bytes
        )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not JSON"):
        controller._parse_action_response(
            artifact.response_bytes + b"{}\n", artifact.request_bytes
        )
    duplicate = artifact.response_bytes.replace(
        b'{"availability":',
        b'{"operation":"build-verange-action-v1","availability":',
        1,
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not JSON"):
        controller._parse_action_response(duplicate, artifact.request_bytes)


def test_operation_selected_native_inspection_binds_exact_statement_identity(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    request = controller.Exact12ActionRequest(
        asset_definition_id="verange_value#privacy",
        candidate_binding_sha256="11" * 32,
        creation_time_millis=1_800_000_000_007,
        network_id_hex="23" * 32,
        nonce=7,
        protocol=controller.VERANGE_PROTOCOL,
        ttl_millis=120_000,
    )
    legacy = _artifact(_request("11" * 32, 7), 0x33)
    action = replace(
        legacy,
        request_bytes=request.canonical_bytes(),
        request_id=json.loads(request.canonical_bytes())["request_id"],
    )
    inspected_value = {
        "adaptive_signed_transaction_bytes": len(action.transaction),
        "aggregation_count": 4,
        "asset_definition_id": request.asset_definition_id,
        "bit_length": 32,
        "encoded_proof_envelope_bytes": 256,
        "execution_classification": "action_verification_and_finality_only",
        "ledger_effect": None,
        "policy_id": controller._driver_seed_v1(request, 2),
        "proof_bytes": 128,
        "proof_envelope_hash": bytes.fromhex("77" * 32),
        "protocol_id": controller.VERANGE_PROTOCOL,
        "statement_bytes": 64,
        "statement_digest": bytes.fromhex("66" * 32),
        "submitted_versioned_transaction_bytes": len(action.transaction),
        "transaction_hash": bytes.fromhex(action.transaction_hash_hex),
        "transaction_intent_digest": bytes.fromhex("55" * 32),
        "value_commitments": [b"commitment"] * 4,
    }

    context_value = {
        "authority": "i105-qualified-authority",
        "authority_public_key": bytes.fromhex("88" * 32),
        "creation_time_millis": request.creation_time_millis,
        "fee_payment": "authority-empty-v1",
        "metadata": "empty-v1",
        "network_id": bytes.fromhex(request.network_id_hex),
        "nonce": request.nonce,
        "statement_action_index": 0,
        "statement_network_id": bytes.fromhex(request.network_id_hex),
        "transaction_hash": bytes.fromhex(action.transaction_hash_hex),
        "ttl_millis": request.ttl_millis,
    }

    class FakeNetworkId:
        @staticmethod
        def from_bytes(value: bytes) -> bytes:
            return value

    fake_crypto = types.SimpleNamespace(
        NetworkId=FakeNetworkId,
        inspect_signed_privacy_verange_action_v1=lambda _transaction: dict(
            inspected_value
        ),
        inspect_privacy_exact12_action_driver_transaction_context_v1=(
            lambda *_args: dict(context_value)
        ),
    )
    fake_package = types.ModuleType("iroha_python")
    fake_package.crypto = fake_crypto  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "iroha_python", fake_package)
    inspection = controller._inspect_native_action(action, request)
    assert inspection.protocol == controller.VERANGE_PROTOCOL
    assert inspection.operation == controller.VERANGE_OPERATION
    assert inspection.transaction_hash_hex == action.transaction_hash_hex
    assert inspection.identity_sha256 != "0" * 64
    assert inspection.availability == "constructible"
    assert inspection.limitations == controller.action_ipc.protocol_limitations(
        controller.VERANGE_PROTOCOL
    )
    assert inspection.network_outcome_authoritative is False

    inspected_value["asset_definition_id"] = "substituted#privacy"
    with pytest.raises(controller.SealedPrivacyControllerError, match="asset differs"):
        controller._inspect_native_action(action, request)

    inspected_value["asset_definition_id"] = request.asset_definition_id
    context_value["creation_time_millis"] = request.creation_time_millis + 1
    with pytest.raises(controller.SealedPrivacyControllerError, match="signed request context"):
        controller._inspect_native_action(action, request)


@pytest.mark.parametrize(
    "label,root",
    (
        ("peer-1", "https://127.0.0.1:8081"),
        ("peer-1", "http://localhost:8081"),
        ("peer-1", "http://user@127.0.0.1:8081"),
        ("peer-0", "http://127.0.0.1:8081"),
        ("peer-1", "http://127.0.0.1:8081/path"),
    ),
)
def test_direct_peer_roots_reject_redirectable_or_credential_bearing_aliases(
    label: str, root: str
) -> None:
    with pytest.raises(controller.SealedPrivacyControllerError, match="loopback"):
        controller.PeerEndpoint(label, root)


def test_peer_set_rejects_missing_duplicate_and_reordered_rows() -> None:
    peers = _peers()
    for hostile in (peers[:3], peers[::-1], (*peers[:3], peers[2])):
        with pytest.raises(controller.SealedPrivacyControllerError):
            controller.require_exact_peer_set(hostile)


def test_four_peer_verange_capability_query_is_native_bounded_and_still_blocked(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    action = _artifact(_request("11" * 32, 7), 0x33)
    public_artifacts = action.public_admission_artifacts
    assert public_artifacts is not None
    archive = b"NRT0-canonical-exact12-capability-manifest"
    manifest_digest_bytes = bytes.fromhex("aa" * 32)
    row = {
        "activation_state": "active",
        "committed_height": 42,
        "compiled_profile_status": "available",
        "engine_id": public_artifacts.engine_id,
        "engine_manifest_digest": bytes.fromhex(
            public_artifacts.engine_manifest_digest_hex
        ),
        "execution_mode": "component",
        "limitation": None,
        "manifest_digest": manifest_digest_bytes,
        "network_available": True,
        "operation_schema": "verange_range_proof_v1",
        "parameter_digest": bytes.fromhex(public_artifacts.parameter_digest_hex),
        "parameter_id": bytes.fromhex(public_artifacts.parameter_id_hex),
        "privacy_feature_mask": 1,
        "proof_system_id": public_artifacts.proof_system_id,
        "protocol_id": controller.VERANGE_PROTOCOL,
        "readiness": "available",
        "statement_schema_digest": bytes.fromhex(
            public_artifacts.statement_schema_digest_hex
        ),
        "unavailable_reason": None,
        "verifier_digest": bytes.fromhex(public_artifacts.verifier_digest_hex),
    }

    class FakeManifest:
        canonical_archive = archive
        committed_height = 42
        manifest_digest = manifest_digest_bytes
        version = 1

        @staticmethod
        def require_network_capability(protocol: str) -> dict[str, object]:
            assert protocol == controller.VERANGE_PROTOCOL
            return dict(row)

    fake_crypto = types.SimpleNamespace(
        privacy_exact12_capability_manifest_v1=lambda value: (
            FakeManifest() if value == archive else (_ for _ in ()).throw(ValueError())
        )
    )
    fake_package = types.ModuleType("iroha_python")
    fake_package.crypto = fake_crypto  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "iroha_python", fake_package)

    calls: list[tuple[str, str, str]] = []

    def exchange(
        peer: controller.PeerEndpoint,
        method: str,
        path: str,
        *,
        body: bytes | None,
        timeout_seconds: float,
        accept: str,
    ) -> controller.HttpObservation:
        assert body is None
        assert timeout_seconds > 0
        calls.append((peer.label, path, accept))
        return controller.HttpObservation(
            200,
            {"content-type": controller.PRIVACY_CAPABILITIES_MEDIA_TYPE},
            archive,
        )

    monkeypatch.setattr(controller, "_direct_exchange", exchange)
    transcript = controller.TranscriptBuilder(
        "planned-verange-capability-query", "11" * 32, DRIVER_SHA256
    )
    state = controller.query_four_peer_verange_capability_state(
        transcript, _peers(), public_artifacts
    )
    assert state.committed_height == 42
    assert state.manifest_digest_hex == "aa" * 32
    assert calls == [
        (
            f"peer-{index}",
            controller.PRIVACY_CAPABILITIES_PATH,
            controller.PRIVACY_CAPABILITIES_MEDIA_TYPE,
        )
        for index in range(1, 5)
    ]
    events = json.loads(transcript.finish()[0])["events"]
    assert len([event for event in events if event["kind"] == "direct-peer-http"]) == 4
    assert events[-1]["kind"] == "four-peer-verange-capability-state"
    assert controller.VERANGE_PROTOCOL not in controller.CONTROLLER_CASE_RUNNERS
    assert controller.controller_case_blockers(controller.VERANGE_PROTOCOL) == (
        controller.action_ipc.VERANGE_CONTROLLER_BLOCKERS
    )

    controller.require_verange_capability_state_preserved(
        state, replace(state, committed_height=43, manifest_digest_hex="bb" * 32)
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="binding changed"):
        controller.require_verange_capability_state_preserved(
            state, replace(state, capability_binding_sha256="cc" * 32)
        )


def test_verange_capability_query_rejects_inactive_or_substituted_profile(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    action = _artifact(_request("11" * 32, 7), 0x33)
    public_artifacts = action.public_admission_artifacts
    assert public_artifacts is not None
    peer = _peers()[0]
    archive = b"NRT0-hostile-capability-manifest"
    row = {
        "activation_state": "not-registered",
        "committed_height": 42,
        "compiled_profile_status": "available",
        "engine_id": public_artifacts.engine_id,
        "engine_manifest_digest": bytes.fromhex(
            public_artifacts.engine_manifest_digest_hex
        ),
        "execution_mode": "component",
        "limitation": None,
        "manifest_digest": bytes.fromhex("aa" * 32),
        "network_available": False,
        "operation_schema": "verange_range_proof_v1",
        "parameter_digest": bytes.fromhex(public_artifacts.parameter_digest_hex),
        "parameter_id": bytes.fromhex(public_artifacts.parameter_id_hex),
        "privacy_feature_mask": 1,
        "proof_system_id": public_artifacts.proof_system_id,
        "protocol_id": controller.VERANGE_PROTOCOL,
        "readiness": "available",
        "statement_schema_digest": bytes.fromhex(
            public_artifacts.statement_schema_digest_hex
        ),
        "unavailable_reason": None,
        "verifier_digest": bytes.fromhex(public_artifacts.verifier_digest_hex),
    }

    class FakeManifest:
        canonical_archive = archive
        committed_height = 42
        manifest_digest = bytes.fromhex("aa" * 32)
        version = 1

        @staticmethod
        def require_network_capability(_protocol: str) -> dict[str, object]:
            return dict(row)

    fake_package = types.ModuleType("iroha_python")
    fake_package.crypto = types.SimpleNamespace(  # type: ignore[attr-defined]
        privacy_exact12_capability_manifest_v1=lambda _value: FakeManifest()
    )
    monkeypatch.setitem(sys.modules, "iroha_python", fake_package)
    response = controller.HttpObservation(
        200,
        {"content-type": controller.PRIVACY_CAPABILITIES_MEDIA_TYPE},
        archive,
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not actively admitted"):
        controller._parse_verange_capability_state(response, peer, public_artifacts)

    row["activation_state"] = "active"
    row["network_available"] = True
    row["parameter_id"] = bytes.fromhex("99" * 32)
    with pytest.raises(controller.SealedPrivacyControllerError, match="differs from the driver"):
        controller._parse_verange_capability_state(response, peer, public_artifacts)


def _write_driver(path: Path, source: str) -> tuple[Path, str]:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o555)
    resolved = path.resolve(strict=True)
    return resolved, hashlib.sha256(resolved.read_bytes()).hexdigest()


def test_driver_admission_requires_exact_nonzero_digest_and_pins_bytes(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    driver, digest = _write_driver(tmp_path / "driver", "#!/bin/sh\nexit 0\n")
    with controller._pinned_action_driver(driver, digest, tmp_path) as pinned:
        assert pinned.sha256 == digest
        assert pinned.execution_method in {
            "darwin-private-fd-copy",
            "linux-pinned-fd",
        }
        assert os.path.samestat(os.fstat(pinned.source_descriptor), driver.stat())
        if sys.platform == "linux":
            assert pinned.execution_path.startswith("/proc/self/fd/")
            assert pinned.inherited_descriptors == (pinned.source_descriptor,)
        else:
            assert pinned.private_directory is not None
            assert Path(pinned.execution_path).parent == pinned.private_directory
    assert not tuple(tmp_path.glob(".privacy-action-driver-*"))

    with pytest.raises(controller.SealedPrivacyControllerError, match="nonzero"):
        controller._admit_action_driver(driver, "0" * 64, tmp_path)
    with pytest.raises(controller.SealedPrivacyControllerError, match="differ"):
        controller._admit_action_driver(driver, "aa" * 32, tmp_path)


def test_driver_admission_rejects_writable_hardlinked_and_nonregular_inputs(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    writable = tmp_path / "writable"
    writable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    writable.chmod(0o755)
    digest = hashlib.sha256(writable.read_bytes()).hexdigest()
    with pytest.raises(controller.SealedPrivacyControllerError, match="non-writable"):
        controller._admit_action_driver(writable.resolve(), digest, tmp_path)

    driver, digest = _write_driver(tmp_path / "linked", "#!/bin/sh\nexit 0\n")
    os.link(driver, tmp_path / "second-link")
    with pytest.raises(controller.SealedPrivacyControllerError, match="singly linked"):
        controller._admit_action_driver(driver, digest, tmp_path)

    directory = tmp_path / "not-a-file"
    directory.mkdir(mode=0o500)
    with pytest.raises(controller.SealedPrivacyControllerError, match="regular file"):
        controller._admit_action_driver(directory, "aa" * 32, tmp_path)


def test_pinned_driver_detects_source_path_substitution(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    driver, digest = _write_driver(tmp_path / "driver", "#!/bin/sh\nexit 0\n")
    replacement, _ = _write_driver(
        tmp_path / "replacement", "#!/bin/sh\nexit 1\n"
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="identity changed"):
        with controller._pinned_action_driver(driver, digest, tmp_path):
            driver.rename(tmp_path / "displaced")
            replacement.rename(driver)


def test_driver_invocation_kills_retained_descendants_and_reports_digest(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    source = f"""#!{sys.executable}
import hashlib
import json
import subprocess
import sys

request = json.load(sys.stdin)
with open("descendant.pid", "w", encoding="ascii") as output:
    child = subprocess.Popen(
        ["/bin/sleep", "30"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    output.write(str(child.pid) + "\\n")
transaction = b"proof-bearing-test-action"
profile = b"canonical-native-verange-compiled-profile"
activation = b"canonical-native-verange-relative-activation-template"
setup_account = "i105-qualification-setup-" + request["candidate_binding_sha256"][:16]
setup_public_key = hashlib.sha256(
    b"test-only-verange-setup-public-key\\0"
    + bytes.fromhex(request["candidate_binding_sha256"])
).digest()
setup_identity = hashlib.sha256()
setup_identity.update(b"iroha.taira.verange_qualification_setup_identity.v1\\0")
setup_identity.update(bytes.fromhex(request["candidate_binding_sha256"]))
setup_identity.update(len(setup_account.encode("ascii")).to_bytes(8, "little"))
setup_identity.update(setup_account.encode("ascii"))
setup_identity.update(setup_public_key)
setup_requirements = {{
    "action_authority_account_id": "i105-qualified-authority",
    "action_authority_public_key_hex": "88" * 32,
    "activation_height_rule": "activate_at_height=proposed_at_height+minimum_delay_blocks",
    "activation_instruction": "register-privacy-protocol-activation-v1",
    "activation_lifecycle": "proposed-relative-height-template-v1",
    "activation_minimum_delay_blocks": 300,
    "activation_template_activate_at_height": 301,
    "activation_template_norito_hex": activation.hex(),
    "activation_template_proposed_at_height": 1,
    "activation_template_sha256": hashlib.sha256(activation).hexdigest(),
    "asset_definition_id": request["asset_definition_id"],
    "candidate_binding_sha256": request["candidate_binding_sha256"],
    "compiled_profile_sha256": hashlib.sha256(profile).hexdigest(),
    "domain_id": "privacy.universal",
    "governance_permission": "CanEnactGovernance",
    "protocol_id": "verange-transparent-range-v1",
    "schema": "iroha.taira.verange_qualification_setup_requirements",
    "schema_version": 1,
    "setup_authority_account_id": setup_account,
    "setup_authority_public_key_hex": setup_public_key.hex(),
    "setup_identity_binding_sha256": setup_identity.hexdigest(),
}}
setup_requirements_sha256 = hashlib.sha256(
    json.dumps(setup_requirements, sort_keys=True, separators=(",", ":")).encode("ascii")
).hexdigest()
policy_id = hashlib.sha256(
    b"iroha.taira.privacy_action_driver_seed.v1\\0"
    + bytes.fromhex(request["candidate_binding_sha256"])
    + bytes.fromhex(request["request_id"])
    + b"\\x02"
).hexdigest()
response = {{
    "availability": "constructible",
    "candidate_binding_sha256": request["candidate_binding_sha256"],
    "limitations": [
        "MissingSealedControllerProtocolCaseEvidence",
        "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity",
        "MissingNativePublicOnlyVeRangePolicyActivationTransactionBundle",
        "MissingFourPeerCanonicalVeRangeCapabilityRowStateQueriesBeforeAfterRestart",
    ],
    "network_outcome_authoritative": False,
    "operation": "build-verange-action-v1",
    "protocol": "verange-transparent-range-v1",
    "public_admission_artifacts": {{
        "action_authority_account_id": "i105-qualified-authority",
        "action_authority_public_key_hex": "88" * 32,
        "compiled_profile_norito_hex": profile.hex(),
        "compiled_profile_sha256": hashlib.sha256(profile).hexdigest(),
        "engine_id": "native-verange-p256",
        "engine_manifest_digest_hex": "55" * 32,
        "max_aggregation_count": 8,
        "parameter_digest_hex": "44" * 32,
        "parameter_id_hex": "33" * 32,
        "policy_id_hex": policy_id,
        "proof_system_id": "iroha-verange-p256",
        "protocol_id": "verange-transparent-range-v1",
        "schema": "iroha.taira.verange_public_admission_artifacts",
        "schema_version": 1,
        "setup_requirements": setup_requirements,
        "setup_requirements_sha256": setup_requirements_sha256,
        "statement_schema_digest_hex": "66" * 32,
        "verifier_digest_hex": "77" * 32,
    }},
    "qualification_scope": "native-action-construction-only",
    "request_id": request["request_id"],
    "schema": "iroha.taira.privacy_action_driver_response",
    "schema_version": 1,
    "transaction_hash_hex": "33" * 32,
    "transaction_norito_hex": transaction.hex(),
    "transaction_sha256": hashlib.sha256(transaction).hexdigest(),
}}
sys.stdout.write(json.dumps(response, sort_keys=True, separators=(",", ":")) + "\\n")
"""
    driver, digest = _write_driver(tmp_path / "driver", source)
    artifact = controller.invoke_action_driver(
        driver,
        _request("11" * 32, 7),
        expected_sha256=digest,
        work_directory=tmp_path,
        timeout_seconds=5,
    )
    assert artifact.action_driver_sha256 == digest
    child_pid = int((tmp_path / "descendant.pid").read_text(encoding="ascii"))
    for _ in range(100):
        try:
            os.kill(child_pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.01)
    else:
        pytest.fail("action-driver descendant survived controller cleanup")


def test_case_rejects_a_substituted_driver_digest_before_network_use(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidate = "11" * 32
    primary_request = _request(candidate, 7)
    successor_request = _request(candidate, 8)
    primary = replace(
        _artifact(primary_request, 0x33), action_driver_sha256="aa" * 32
    )
    successor = _artifact(successor_request, 0x44)
    actions = iter((primary, successor))
    monkeypatch.setattr(
        controller,
        "invoke_action_driver",
        lambda *_args, **_kwargs: next(actions),
    )

    class Supervisor:
        peer = _peers()[-1]

    with pytest.raises(controller.SealedPrivacyControllerError, match="wrong driver"):
        controller.run_verange_diagnostic_case(
            case="driver-substitution",
            candidate_binding_sha256=candidate,
            action_driver=tmp_path / "unused-by-mock",
            expected_action_driver_sha256=DRIVER_SHA256,
            work_directory=tmp_path,
            peers=_peers(),
            restarted_supervisor=Supervisor(),  # type: ignore[arg-type]
            primary_request=primary_request,
            successor_request=successor_request,
            timeout_seconds=5,
        )


def test_generic_verange_finality_diagnostic_emits_non_authoritative_records(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidate = "11" * 32
    primary_request = _request(candidate, 7)
    successor_request = _request(candidate, 8)
    primary = _artifact(primary_request, 0x33)
    successor = _artifact(successor_request, 0x44)
    actions = iter((primary, successor))
    monkeypatch.setattr(
        controller,
        "invoke_action_driver",
        lambda *_args, **_kwargs: next(actions),
    )

    def inspect(
        action: controller.ActionArtifact,
        request: controller.VeRangeActionRequest,
    ) -> controller.NativeVeRangeInspection:
        request_id = bytes.fromhex(action.request_id)
        candidate_bytes = bytes.fromhex(request.candidate_binding_sha256)
        policy_id = hashlib.sha256(
            controller.DRIVER_SEED_DOMAIN
            + candidate_bytes
            + request_id
            + b"\x02"
        ).hexdigest()
        return controller.NativeVeRangeInspection(
            action.transaction_hash_hex,
            "55" * 32,
            "66" * 32,
            "77" * 32,
            policy_id,
            1024,
            512,
        )

    monkeypatch.setattr(controller, "_inspect_native_verange_action", inspect)

    peers = _peers()
    fleet_rounds = (
        (10, "aa" * 32),
        (11, "bb" * 32),
        (11, "bb" * 32),
        (12, "cc" * 32),
    )
    status_calls = 0
    peer_round: dict[str, int] = {}
    post_statuses = iter((202, 409, 400, 202))

    def exchange(
        peer: controller.PeerEndpoint,
        method: str,
        path: str,
        *,
        body: bytes | None,
        timeout_seconds: float,
    ) -> controller.HttpObservation:
        nonlocal status_calls
        assert timeout_seconds > 0
        if method == "POST":
            return controller.HttpObservation(next(post_statuses), {}, b"")
        if path == "/status":
            round_index = status_calls // 4
            peer_round[peer.label] = round_index
            status_calls += 1
            return _json_response({"blocks": fleet_rounds[round_index][0]})
        if path == "/v1/sumeragi/status":
            height, block_hash = fleet_rounds[peer_round[peer.label]]
            return _json_response(
                {
                    "last_committed_height": height,
                    "last_committed_subject": {"block_hash": block_hash},
                }
            )
        if path.startswith("/v1/pipeline/transactions/status?"):
            query = urllib_parse(path)
            return _json_response(
                {
                    "hash": query["hash"],
                    "resolved_from": peer.label,
                    "scope": "global",
                    "status": {"kind": "Applied"},
                }
            )
        raise AssertionError((peer, method, path, body))

    monkeypatch.setattr(controller, "_direct_exchange", exchange)

    class Supervisor:
        peer = peers[-1]

        @staticmethod
        def restart(deadline: float) -> tuple[int, int, int]:
            assert deadline > 0
            return 101, 202, 25

    records = controller.run_verange_diagnostic_case(
        case="verange-controller-owned-diagnostic",
        candidate_binding_sha256=candidate,
        action_driver=tmp_path / "unused-by-mock",
        expected_action_driver_sha256=DRIVER_SHA256,
        work_directory=tmp_path,
        peers=peers,
        restarted_supervisor=Supervisor(),  # type: ignore[arg-type]
        primary_request=primary_request,
        successor_request=successor_request,
        timeout_seconds=5,
    )
    transcript = json.loads(records.transcript)
    result = json.loads(records.result)
    assert controller._canonical_json_bytes(transcript) == records.transcript
    assert controller._canonical_json_bytes(result) == records.result
    assert result["diagnostic_only"] is True
    assert result["operation_surface_complete"] is False
    assert result["availability"] == "constructible"
    assert result["limitations"] == list(
        controller.action_ipc.protocol_limitations(controller.VERANGE_PROTOCOL)
    )
    assert result["network_outcome_authoritative"] is False
    assert result["qualification_scope"] == "native-action-construction-only"
    assert result["action_driver_sha256"] == DRIVER_SHA256
    assert transcript["action_driver_sha256"] == DRIVER_SHA256
    assert result["sentinel_height"] == result["recovered_height"] == 11
    assert result["sentinel_hash"] == result["recovered_hash"] == "bb" * 32
    assert result["successor_height"] == 12
    assert len([row for row in transcript["events"] if row["kind"] == "direct-peer-http"]) == 48
    assert any(row["kind"] == "controller-owned-restart" for row in transcript["events"])
    native_rows = [row for row in transcript["events"] if row["kind"] == "native-action"]
    assert len(native_rows) == 2
    assert all("response_base64" not in row for row in native_rows)
    assert all("transaction_base64" in row for row in native_rows)
    assert all(row["action_driver_sha256"] == DRIVER_SHA256 for row in native_rows)
    assert all(row["network_outcome_authoritative"] is False for row in native_rows)


def urllib_parse(path: str) -> dict[str, str]:
    from urllib.parse import parse_qs, urlsplit

    values = parse_qs(urlsplit(path).query, strict_parsing=True)
    return {key: rows[0] for key, rows in values.items()}


def test_pipeline_status_cannot_lie_about_transaction_identity() -> None:
    peer = _peers()[0]
    response = _json_response(
        {
            "hash": "22" * 32,
            "resolved_from": peer.label,
            "scope": "global",
            "status": {"kind": "Applied"},
        }
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not bound"):
        controller._pipeline_status_kind(response, "11" * 32, peer)


def test_replay_or_adversary_success_status_fails_closed() -> None:
    transcript = controller.TranscriptBuilder("case", "11" * 32, DRIVER_SHA256)
    peer = _peers()[0]
    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setattr(
            controller,
            "_direct_exchange",
            lambda *_args, **_kwargs: controller.HttpObservation(202, {}, b""),
        )
        with pytest.raises(controller.SealedPrivacyControllerError, match="accepted"):
            controller._submit(
                transcript, peer, b"proof-bearing", expected="rejected"
            )


def test_exact_restart_sentinel_rejects_a_higher_or_changed_common_sample(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    transcript = controller.TranscriptBuilder("case", "11" * 32, DRIVER_SHA256)
    monkeypatch.setattr(
        controller,
        "_fleet_sample",
        lambda *_args, **_kwargs: controller.FleetSample(12, "cc" * 32),
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="exact sentinel"):
        controller._wait_for_exact_sentinel(
            transcript,
            _peers(),
            controller.FleetSample(11, "bb" * 32),
            float("inf"),
        )


def test_controller_owned_pid_file_rejects_world_readable_or_symlink(
    tmp_path: Path,
) -> None:
    peer = _peers()[-1]

    class Process:
        @staticmethod
        def poll() -> None:
            return None

    pid_file = tmp_path / "child.pid"
    pid_file.write_text("123\n", encoding="ascii")
    pid_file.chmod(0o644)
    supervisor = controller.ControllerOwnedSupervisor(
        peer,
        Process(),  # type: ignore[arg-type]
        pid_file,
        pid_file.stat().st_uid,
        pid_file.stat().st_gid,
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="unsafe"):
        supervisor._child_pid()
    pid_file.chmod(0o600)
    link = tmp_path / "child-link.pid"
    link.symlink_to(pid_file)
    supervisor.child_pid_file = link
    with pytest.raises(controller.SealedPrivacyControllerError, match="unsafe"):
        supervisor._child_pid()


def _qualification_case_plan_fixture(
    tmp_path: Path,
) -> tuple[
    deploy.BundlePlan,
    controller.VeRangePublicAdmissionArtifactsV1,
    tuple[object, ...],
    dict[str, object],
]:
    candidate = "11" * 32
    cargo_lock = "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
    workspace_source = "22" * 32
    irohad = "33" * 32
    supervisor_digest = "44" * 32
    restart_generation = "55" * 32
    request = _request(candidate, 7)
    artifact = _artifact(request, 0x33).public_admission_artifacts
    assert artifact is not None

    peers: list[deploy.PeerPlan] = []
    configs: dict[str, str] = {}
    for number in range(1, 5):
        slug = f"validator-{number}"
        config_sha = f"{number + 5:02x}" * 32
        configs[slug] = config_sha
        workdir = tmp_path / "bundle" / "rendered" / slug
        peers.append(
            deploy.PeerPlan(
                number=number,
                label=f"peer-{number}",
                slug=slug,
                torii_port=8080 + number,
                p2p_port=1336 + number,
                workdir=workdir,
                storage=workdir / "storage",
                config=workdir / "config.toml",
                config_sha256=config_sha,
                config_identity=(number, 10, 11),
                workdir_identity=(number, 20, 21),
                storage_identity=(number, 30, 31),
                workdir_device=number,
                workdir_inode=20 + number,
                storage_device=number,
                storage_inode=30 + number,
            )
        )
    setup = artifact.setup_requirements
    manifest: dict[str, object] = {
        "candidate_binding_sha256": candidate,
        "cargo_lock_sha256": cargo_lock,
        "configs": configs,
        "dpn_validator_release_commit": "b" * 40,
        "genesis_expected_hash": "66" * 32,
        "genesis_public_key": "ed0120" + "77" * 32,
        "irohad_sha256": irohad,
        "privacy_qualification_setup": {
            "candidate_binding_sha256": candidate,
            "schema": "iroha.taira.verange_qualification_genesis_plan",
            "schema_version": 1,
            "setup_authority_account_id": setup.setup_authority_account_id,
            "setup_authority_public_key_hex": setup.setup_authority_public_key_hex,
            "setup_requirements_sha256": artifact.setup_requirements_sha256,
        },
        "signed_genesis_sha256": "88" * 32,
        "source_commit": "a" * 40,
        "unsigned_genesis_sha256": "99" * 32,
        "workspace_source_manifest_sha256": workspace_source,
    }
    bundle = deploy.BundlePlan(
        root=tmp_path / "bundle",
        owner_uid=501,
        owner_gid=20,
        runtime_user="validator",
        runtime_group="staff",
        manifest=manifest,
        manifest_sha256="aa" * 32,
        manifest_identity=(1, 2, 3),
        signed_genesis_identity=(4, 5, 6),
        peers=tuple(peers),
        bundle_bytes=1,
        free_bytes=1,
        free_bytes_by_device=((1, 1),),
        fsync_latency_ms=1.0,
    )
    binary_path = tmp_path / "installed" / "iroha3d"
    supervisor_path = tmp_path / "installed" / "taira_peer_supervisor.py"
    supervisors: list[object] = []
    for peer in peers:
        runtime = tmp_path / "runtime" / peer.slug
        pid_file = tmp_path / "runtime" / "pids" / f"{peer.slug}.pid"
        terminal_file = tmp_path / "runtime" / "terminal" / f"{peer.slug}.json"
        storage = runtime / "storage"
        argv = (
            "/usr/bin/python3",
            "-I",
            "-S",
            str(supervisor_path),
            "--binary",
            str(binary_path),
            "--binary-sha256",
            irohad,
            "--config",
            str(peer.config),
            "--config-sha256",
            peer.config_sha256,
            "--workdir",
            str(runtime),
            "--storage-dir",
            str(storage),
            "--pid-file",
            str(pid_file),
            "--terminal-unhealthy-file",
            str(terminal_file),
            "--restart-generation",
            restart_generation,
        )
        supervisors.append(
            types.SimpleNamespace(
                peer=peer,
                child_argv=argv,
                pid_file=pid_file,
                terminal_file=terminal_file,
                workdir=runtime,
                storage=storage,
            )
        )
    arguments: dict[str, object] = {
        "bundle": bundle,
        "candidate_binding_sha256": candidate,
        "cargo_lock_sha256": cargo_lock,
        "workspace_source_manifest_sha256": workspace_source,
        "public_artifacts": artifact,
        "supervisors": tuple(supervisors),
        "supervisor_sha256": supervisor_digest,
        "restart_generation": restart_generation,
    }
    return bundle, artifact, tuple(supervisors), arguments


def test_verange_case_plan_binds_reset_peers_genesis_source_and_supervisors(
    tmp_path: Path,
) -> None:
    _bundle, _artifact_value, _supervisors, arguments = (
        _qualification_case_plan_fixture(tmp_path)
    )
    plan = controller.build_verange_qualification_case_plan_v1(**arguments)
    assert tuple(peer.direct_torii_root for peer in plan.peers) == tuple(
        f"http://127.0.0.1:{8080 + number}" for number in range(1, 5)
    )
    assert plan.plan_binding_sha256 != "0" * 64
    assert len({row.child_argv_sha256 for row in plan.supervisors}) == 4
    transcript = controller.TranscriptBuilder(
        "planned-verange-case", plan.candidate_binding_sha256, DRIVER_SHA256
    )
    controller.bind_verange_qualification_case_plan_v1(transcript, plan)
    event = json.loads(transcript.finish()[0])["events"][0]
    assert event["kind"] == "verange-qualification-case-plan"
    assert event["plan_binding_sha256"] == plan.plan_binding_sha256
    assert dict(controller.CONTROLLER_CASE_RUNNERS) == {}
    with pytest.raises(TypeError):
        controller.build_verange_qualification_case_plan_v1(  # type: ignore[call-arg]
            **arguments, peer_roots=("http://attacker.invalid",)
        )


def test_verange_case_plan_rejects_source_genesis_peer_and_supervisor_substitution(
    tmp_path: Path,
) -> None:
    bundle, artifact, supervisors, arguments = _qualification_case_plan_fixture(tmp_path)

    for field, value in (
        ("candidate_binding_sha256", "ab" * 32),
        ("workspace_source_manifest_sha256", "bc" * 32),
    ):
        hostile = dict(arguments)
        hostile[field] = value
        with pytest.raises(controller.VeRangeQualificationPlanError):
            controller.build_verange_qualification_case_plan_v1(**hostile)

    missing_genesis_plan = replace(
        bundle,
        manifest={
            key: value
            for key, value in bundle.manifest.items()
            if key != "privacy_qualification_setup"
        },
    )
    with pytest.raises(
        controller.VeRangeQualificationPlanError, match="does not admit"
    ):
        controller.build_verange_qualification_case_plan_v1(
            **{**arguments, "bundle": missing_genesis_plan}
        )

    substituted_configs = dict(bundle.manifest)
    substituted_configs["configs"] = {
        **dict(bundle.manifest["configs"]),  # type: ignore[arg-type]
        bundle.peers[0].slug: "cc" * 32,
    }
    with pytest.raises(controller.VeRangeQualificationPlanError, match="config identity"):
        controller.build_verange_qualification_case_plan_v1(
            **{**arguments, "bundle": replace(bundle, manifest=substituted_configs)}
        )

    with pytest.raises(controller.VeRangeQualificationPlanError, match="supervisor identity"):
        controller.build_verange_qualification_case_plan_v1(
            **{**arguments, "supervisors": supervisors[::-1]}
        )

    substituted_setup = replace(
        artifact.setup_requirements, candidate_binding_sha256="dd" * 32
    )
    with pytest.raises(controller.VeRangeQualificationPlanError, match="internally"):
        controller.build_verange_qualification_case_plan_v1(
            **{
                **arguments,
                "public_artifacts": replace(
                    artifact, setup_requirements=substituted_setup
                ),
            }
        )


def test_source_boundary_has_no_driver_network_or_self_attestation_surface() -> None:
    driver = (
        ROOT / "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
    ).read_text(encoding="utf-8")
    sealed = (
        ROOT / "scripts/taira_privacy_sealed_controller.py"
    ).read_text(encoding="utf-8")
    case_plan = (
        ROOT / "scripts/taira_privacy_verange_case_plan.py"
    ).read_text(encoding="utf-8")
    capture = (
        ROOT / "scripts/capture_taira_privacy_protocol_four_peer_receipt.py"
    ).read_text(encoding="utf-8")
    python_native = (
        ROOT / "python/iroha_python/iroha_python_rs/src/lib.rs"
    ).read_text(encoding="utf-8")
    sealer = (ROOT / "scripts/seal_taira_release_controllers.py").read_text(
        encoding="utf-8"
    )
    assert "reqwest" not in driver
    assert "ureq" not in driver
    request_fields = driver.split("struct BuildActionRequestV1 {", 1)[1].split("}", 1)[0]
    assert "endpoint" not in request_fields
    assert "credential" not in request_fields
    assert "values" not in request_fields
    assert "witness" not in request_fields
    assert "proof" not in request_fields
    assert "chain_id" not in request_fields
    assert "genesis_hash_hex" not in request_fields
    assert "network_id_hex" in request_fields
    assert "std::net" not in driver
    assert "test result: ok" not in driver
    assert 'transaction_norito_hex: String' in driver
    assert 'transaction_hash_hex: String' in driver
    assert "VeRangePublicAdmissionArtifactsV1" in driver
    assert "public_admission_artifacts" in driver
    assert "compiled_profile_norito_hex" in driver
    assert (
        "const CONSTRUCTIBLE_OPERATION_SPECS_V1: "
        "[ConstructibleOperationSpecV1; 10]" in driver
    )
    assert 'protocol: "iroha-zk-ams-v1"' not in driver
    assert 'protocol: "iroha-zk-x509-stark-p256-v0"' not in driver
    assert "_NATIVE_INSPECTOR" in sealed
    assert "_NATIVE_EXTRA_FIELDS" in sealed
    assert "_NATIVE_CONTEXT_FIELDS" in sealed
    assert "CONSTRUCTIBLE_OPERATIONS" in sealed
    assert "CONTROLLER_CASE_RUNNERS" in sealed
    assert "CONTROLLER_CASE_BLOCKERS" in sealed
    assert "VERANGE_PLANNED_CONTROLLER_CASE" in sealed
    assert "query_four_peer_verange_capability_state" in sealed
    assert "build_verange_qualification_case_plan_v1" in sealed
    assert "bind_verange_qualification_case_plan_v1" in sealed
    assert "urllib" not in case_plan
    assert "subprocess" not in case_plan
    assert "CONTROLLER_CASE_RUNNERS" not in case_plan
    assert 'PRIVACY_CAPABILITIES_PATH = "/v1/privacy/capabilities"' in sealed
    assert "privacy_exact12_capability_manifest_v1" in sealed
    assert "_NATIVE_EFFECTS" not in sealed
    assert (
        "inspect_privacy_exact12_action_driver_transaction_context_v1" in sealed
    )
    assert "privacy_exact12_action_driver_signing_seed_v1" in python_native
    assert "signed.fee_payment_intent()" in python_native
    assert "signed.metadata().is_empty()" in python_native
    assert "Zeroizing<[u8; 32]>" in python_native
    context_inspector = python_native.split(
        "fn inspect_privacy_exact12_action_driver_transaction_context_v1_py(", 1
    )[1].split("#[pyfunction]", 1)[0]
    assert "counterparty" not in context_inspector
    assert 'result.set_item("availability", "available-experimental")' in python_native
    assert "MissingDistributionWideKnowledgeSoundnessEvidence" in python_native
    assert '"/v1/pipeline/transactions"' in sealed
    assert '"/v1/sumeragi/status"' in sealed
    assert "signal.SIGUSR1" in sealed
    assert "require_complete_release_operation_surface" in sealed
    assert "privacy protocol v2 issuance is closed" in capture
    assert capture.index("require_complete_release_operation_surface()") < capture.index(
        "case_rows = ["
    )
    assert '"scripts/taira_privacy_sealed_controller.py"' in sealer
    assert '"scripts/taira_privacy_verange_case_plan.py"' in sealer
