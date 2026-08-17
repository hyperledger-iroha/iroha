"""Focused tests for the fixed native Taira authority client contract."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

import pytest

from scripts import taira_authority_client as client


def _digest(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def _native_result(request: dict[str, object], status: str) -> dict[str, object]:
    return {
        "authority_envelope": {"schema": "test-authority-envelope-v1"},
        "durable_receipt": {"schema": "test-durable-receipt-v1"},
        "operation_id": request["operation_id"],
        "role": request["role"],
        "schema": client.CLIENT_RESULT_SCHEMA,
        "status": status,
    }


def _decoded(payload: bytes | None) -> dict[str, object]:
    assert payload is not None
    value = json.loads(payload)
    assert isinstance(value, dict)
    return value


def test_registry_and_installation_roots_are_closed() -> None:
    assert tuple(client.ROLE_REGISTRY) == client.ROLE_LABELS
    assert len(client.ROLE_REGISTRY) == 8
    roles = tuple(client.ROLE_REGISTRY.values())
    assert len({role.service_id for role in roles}) == 8
    assert len({role.administrator_id for role in roles}) == 8
    assert len({role.binding_path for role in roles}) == 8
    assert len({role.request_socket for role in roles}) == 8
    assert len({role.state_directory for role in roles}) == 8
    assert all(role.role in role.binding_path.parts for role in roles)
    assert all(role.role in role.request_socket.parts for role in roles)
    assert all(role.role in role.state_directory.parts for role in roles)


def test_run_and_operation_ids_use_the_pinned_length_framed_contract() -> None:
    role = "native-evidence"
    subject = {"b": 2, "a": 1}
    manifest = [
        {"name": "evidence/one", "ordinal": 0, "sha256": _digest("one"), "size": 3}
    ]
    subject_hash = hashlib.sha256(client.canonical_json_bytes(subject)[:-1]).digest()
    expected_run = hashlib.sha256(
        client.RUN_ID_DOMAIN
        + len(role).to_bytes(8, "big")
        + role.encode("ascii")
        + len(subject_hash).to_bytes(8, "big")
        + subject_hash
    ).hexdigest()
    assert client.derive_run_id(role, subject) == expected_run
    manifest_hash = hashlib.sha256(client.canonical_json_bytes(manifest)[:-1]).digest()
    expected_operation = hashlib.sha256(
        client.OPERATION_ID_DOMAIN
        + len(role).to_bytes(8, "big")
        + role.encode("ascii")
        + (32).to_bytes(8, "big")
        + bytes.fromhex(expected_run)
        + (32).to_bytes(8, "big")
        + subject_hash
        + (32).to_bytes(8, "big")
        + manifest_hash
    ).hexdigest()
    assert client._operation_id(role, expected_run, subject, manifest) == expected_operation


def test_preflight_requires_the_exact_authenticated_status(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    status = {
        "administrator_id": (
            "taira-authority-native-evidence-administrator-v1"
        ),
        "audit_head": _digest("audit"),
        "audit_sequence": 1,
        "binding_sha256": _digest("binding"),
        "key_revision": 1,
        "policy_revision": 1,
        "revoked": False,
        "role": "native-evidence",
        "schema": client.CLIENT_STATUS_SCHEMA,
        "service_id": "taira-authority-native-evidence-v1",
        "status": "ready",
    }
    monkeypatch.setattr(client, "_invoke_native_client", lambda *_args, **_kwargs: status)
    assert client.preflight("native-evidence") == status
    status["revoked"] = True
    with pytest.raises(client.TairaAuthorityClientError, match="not ready"):
        client.preflight("native-evidence")


def test_authorize_preserves_descriptor_order_and_rechecks_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    first.write_bytes(b"first")
    second.write_bytes(b"second")
    captured: dict[str, object] = {}

    def invoke(command: str, role: str, payload: bytes | None, opened=()):
        request = _decoded(payload)
        captured.update(request)
        assert command == "authorize"
        assert role == "native-evidence"
        assert [os.pread(item.descriptor, item.identity[6], 0) for item in opened] == [
            b"first",
            b"second",
        ]
        return _native_result(request, "authorized")

    monkeypatch.setattr(client, "_invoke_native_client", invoke)
    result = client.authorize(
        "native-evidence",
        {"subject": "exact"},
        artifacts=(
            client.Artifact("evidence/first", first),
            client.Artifact("evidence/second", second),
        ),
    )
    manifest = captured["artifact_manifest"]
    assert isinstance(manifest, list)
    assert [row["name"] for row in manifest] == ["evidence/first", "evidence/second"]
    assert [row["ordinal"] for row in manifest] == [0, 1]
    assert result.artifact_manifest == tuple(manifest)


@pytest.mark.parametrize("attack", ("symlink", "hardlink", "alias"))
def test_artifact_aliases_and_links_are_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, attack: str,
) -> None:
    source = tmp_path / "source"
    source.write_bytes(b"immutable")
    if attack == "symlink":
        target = tmp_path / "target"
        target.symlink_to(source)
        artifacts = (client.Artifact("artifact", target),)
    elif attack == "hardlink":
        target = tmp_path / "target"
        os.link(source, target)
        artifacts = (client.Artifact("artifact", target),)
    else:
        artifacts = (
            client.Artifact("artifact/one", source),
            client.Artifact("artifact/two", source),
        )
    monkeypatch.setattr(
        client,
        "_invoke_native_client",
        lambda *_args, **_kwargs: pytest.fail("unsafe artifact reached native client"),
    )
    with pytest.raises(client.TairaAuthorityClientError, match="unsafe|alias"):
        client.authorize("native-evidence", {"subject": "exact"}, artifacts=artifacts)


def test_post_call_artifact_mutation_refuses_the_result(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"before")

    def invoke(_command: str, _role: str, payload: bytes | None, _opened=()):
        request = _decoded(payload)
        artifact.write_bytes(b"after!")
        return _native_result(request, "authorized")

    monkeypatch.setattr(client, "_invoke_native_client", invoke)
    with pytest.raises(client.TairaAuthorityClientError, match="mutated"):
        client.authorize(
            "native-evidence",
            {"subject": "exact"},
            artifacts=(client.Artifact("artifact", artifact),),
        )


def test_historical_verification_uses_the_original_operation_without_resigning(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    subject = {"subject": "exact"}
    run_id = client.derive_run_id("rollout-observation", subject)
    operation_id = client._operation_id("rollout-observation", run_id, subject, [])
    calls: list[str] = []

    def invoke(command: str, role: str, payload: bytes | None, _opened=()):
        request = _decoded(payload)
        calls.append(command)
        assert role == "rollout-observation"
        assert request["operation_id"] == operation_id
        assert request["schema"] == client.CLIENT_VERIFICATION_SCHEMA
        return _native_result(request, "valid")

    monkeypatch.setattr(client, "_invoke_native_client", invoke)
    result = client.verify_receipt(
        "rollout-observation",
        subject,
        authority_envelope={"schema": "envelope"},
        durable_receipt={"schema": "receipt"},
        run_id=run_id,
        operation_id=operation_id,
    )
    assert result.status == "valid"
    assert calls == ["verify-receipt"]


def test_deploy_dry_run_apply_and_finalize_share_one_lease_identity(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    subject = {"deployment": _digest("deployment")}
    payload = tmp_path / "deployment-artifact"
    payload.write_bytes(b"deployment")
    artifacts = (client.Artifact("deployment/artifact", payload),)
    calls: list[dict[str, object]] = []

    def invoke(_command: str, role: str, payload: bytes | None, _opened=()):
        request = _decoded(payload)
        calls.append(request)
        disposition = request.get("disposition")
        status = {"dry-run": "verified", "apply": "authorized", "finalize": "finalized"}[
            disposition
        ]
        return _native_result(request, status)

    monkeypatch.setattr(client, "_invoke_native_client", invoke)
    dry_run = client.authorize(
        "deploy-issuance", subject, artifacts=artifacts, disposition="dry-run"
    )
    applied = client.authorize(
        "deploy-issuance", subject, artifacts=artifacts, disposition="apply"
    )
    finalized = client.finalize_deployment(
        subject,
        lease=applied,
        outcome="success",
        result_sha256=_digest("result"),
    )
    assert dry_run.operation_id == applied.operation_id == finalized.operation_id
    assert dry_run.run_id == applied.run_id == finalized.run_id
    assert [request["disposition"] for request in calls] == [
        "dry-run",
        "apply",
        "finalize",
    ]
    assert "deployment_result" not in calls[0]
    assert "deployment_result" not in calls[1]
    assert calls[2]["deployment_result"] == {
        "outcome": "success",
        "result_sha256": _digest("result"),
    }
