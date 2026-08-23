"""Common fail-closed transport tests for all thirteen Exact12 operations."""

from __future__ import annotations

from types import SimpleNamespace

import pytest

import iroha_python
import iroha_python.client as client_module
from iroha_python import NetworkId, SorafsAliasPolicy
from iroha_python.client import (
    PRIVACY_EXACT12_ACTION_OPERATIONS_V1,
    LocalSigningContext,
    PrivacyActionOperationViewV1,
    PrivacyExact12ActionOperationV1,
    PrivacyExact12ActionRequestV1,
    ToriiClient,
)

NETWORK_ID = NetworkId.from_bytes(bytes([0x91]) * 32)
MANIFEST_DIGEST = bytes([0xA2]) * 32
TRANSACTION_HASH = bytes([0x31]) * 32
TRANSACTION_INTENT_DIGEST = bytes([0x32]) * 32
STATEMENT_DIGEST = bytes([0x33]) * 32
PROOF_ENVELOPE_HASH = bytes([0x34]) * 32


class _FakeCrypto:
    def __init__(self, events: list[str]) -> None:
        self.events = events
        self.current_wire = b""

    def __getattr__(self, name: str):
        by_inspector = {
            spec.inspector: (operation, spec)
            for operation, spec in client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1.items()
        }
        if name not in by_inspector:
            raise AttributeError(name)
        operation, spec = by_inspector[name]

        def inspect(*args: object) -> dict[str, object]:
            expected_args = 2 if spec.inspector_requires_network_id else 1
            assert len(args) == expected_args
            assert args[0] == self.current_wire
            if spec.inspector_requires_network_id:
                assert args[1] == NETWORK_ID
            self.events.append(f"inspect:{operation}")
            return {
                "protocol_id": spec.protocol_id,
                "execution_classification": (
                    spec.inspection_execution_classification or spec.execution_mode
                ),
                "ledger_effect": (
                    None
                    if spec.ledger_effect_kind == "verification_only"
                    else spec.ledger_effect_kind
                ),
                "transaction_hash": TRANSACTION_HASH,
                "transaction_intent_digest": TRANSACTION_INTENT_DIGEST,
                "statement_digest": STATEMENT_DIGEST,
                "proof_envelope_hash": PROOF_ENVELOPE_HASH,
                **({"action_kind": spec.action_kind} if spec.action_kind is not None else {}),
            }

        return inspect

    def signed_transaction_envelope_from_versioned_v1(
        self,
        wire: object,
        network_id: object,
    ) -> SimpleNamespace:
        assert wire == self.current_wire
        assert network_id == NETWORK_ID
        self.events.append("reconstruct")
        return SimpleNamespace(
            signed_transaction_versioned=self.current_wire,
            hash=TRANSACTION_HASH,
        )


class _FakeManifest:
    manifest_digest = MANIFEST_DIGEST

    def __init__(
        self,
        operation: PrivacyExact12ActionOperationV1,
        events: list[str],
    ) -> None:
        self.operation = operation
        self.events = events

    def require_network_capability(self, protocol_id: str) -> dict[str, object]:
        spec = client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[self.operation]
        assert protocol_id == spec.protocol_id
        self.events.append("gate")
        return {
            "protocol_id": spec.protocol_id,
            "operation_schema": spec.manifest_operation_schema,
            "execution_mode": spec.execution_mode,
            "manifest_digest": MANIFEST_DIGEST,
            "committed_height": 411,
        }


def _client(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[ToriiClient, _FakeCrypto, list[str]]:
    events: list[str] = []
    crypto = _FakeCrypto(events)
    monkeypatch.setattr(client_module, "_CRYPTO_MODULE", crypto)
    alias_policy = SorafsAliasPolicy(
        positive_ttl_secs=1,
        refresh_window_secs=1,
        hard_expiry_secs=1,
        negative_ttl_secs=1,
        revocation_ttl_secs=1,
        rotation_max_age_secs=1,
        successor_grace_secs=0,
        governance_grace_secs=0,
    )
    return (
        ToriiClient(
            "http://torii.invalid",
            sorafs_alias_policy=alias_policy,
            local_signing_context=LocalSigningContext(NETWORK_ID),
        ),
        crypto,
        events,
    )


def test_exact12_request_union_is_closed_and_exported() -> None:
    assert len(PRIVACY_EXACT12_ACTION_OPERATIONS_V1) == 13
    assert len(set(PRIVACY_EXACT12_ACTION_OPERATIONS_V1)) == 13
    assert tuple(client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1) == (
        PRIVACY_EXACT12_ACTION_OPERATIONS_V1
    )
    assert PRIVACY_EXACT12_ACTION_OPERATIONS_V1 is iroha_python.PRIVACY_EXACT12_ACTION_OPERATIONS_V1
    assert iroha_python.PrivacyExact12ActionRequestV1 is PrivacyExact12ActionRequestV1

    with pytest.raises(ValueError, match="one exact"):
        PrivacyExact12ActionRequestV1(
            "zk_ams_admission_and_provisioning_v1",  # type: ignore[arg-type]
            b"wire",
        )
    with pytest.raises(ValueError, match="32 non-zero"):
        PrivacyExact12ActionRequestV1(
            "zk_ace_authorization_action_v1",
            b"wire",
            expected_manifest_digest=bytes(32),
        )


def test_every_exact12_operation_authenticates_gates_and_submits_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    submitted: list[object] = []
    active_operation: PrivacyExact12ActionOperationV1 = PRIVACY_EXACT12_ACTION_OPERATIONS_V1[0]

    def capabilities(*, canonical_auth: object) -> _FakeManifest:
        assert canonical_auth == "auth"
        events.append("capabilities")
        return _FakeManifest(active_operation, events)

    monkeypatch.setattr(client, "privacy_capabilities_v1", capabilities)
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda envelope: submitted.append(envelope),
    )

    for index, operation in enumerate(PRIVACY_EXACT12_ACTION_OPERATIONS_V1, start=1):
        active_operation = operation
        wire = f"signed-wire-{index}".encode()
        crypto.current_wire = wire
        request = PrivacyExact12ActionRequestV1(
            operation,
            wire,
            expected_manifest_digest=MANIFEST_DIGEST,
        )
        event_offset = len(events)

        envelope, view = client.submit_signed_privacy_action_v1(
            request,
            canonical_auth="auth",  # type: ignore[arg-type]
            network_id=NETWORK_ID,
            wait=False,
        )

        assert envelope is submitted[-1]
        assert isinstance(view, PrivacyActionOperationViewV1)
        assert view.operation_schema == operation
        assert view.protocol_id == (
            client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[operation].protocol_id
        )
        assert view.local_state == "submitted"
        assert view.terminal_chain_state is None
        assert view.transaction_hash == TRANSACTION_HASH.hex()
        assert view.capability_manifest_digest == MANIFEST_DIGEST.hex()
        assert view.capability_committed_height == 411
        assert events[event_offset:] == [
            f"inspect:{operation}",
            "reconstruct",
            "capabilities",
            "gate",
        ]

    assert len(submitted) == 13


def test_wait_path_requires_committed_semantics_and_returns_typed_view(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "anonymous_pgc_payment_action_v1"
    crypto.current_wire = b"pgc-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    waits: list[dict[str, object]] = []

    def wait(_envelope: object, **kwargs: object) -> dict[str, object]:
        waits.append(kwargs)
        return {
            "status": {"kind": "Committed", "block_height": 512},
        }

    monkeypatch.setattr(client, "submit_transaction_envelope_and_wait", wait)
    envelope, view = client.submit_signed_privacy_action_v1(
        PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
        canonical_auth="auth",  # type: ignore[arg-type]
        network_id=NETWORK_ID,
    )

    assert envelope.signed_transaction_versioned == crypto.current_wire
    assert view.local_state == "terminal"
    assert view.terminal_chain_state == "Committed"
    assert view.committed_height == 512
    assert view.ledger_effect_kind == "anonymous_pgc_account_state_transition"
    assert waits[0]["success_statuses"] == ("Committed", "Applied")
    assert waits[0]["failure_statuses"] == ("Rejected", "Expired")


def test_manifest_rotation_fails_before_submission(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "zk_ams_batch_admission_action_v1"
    crypto.current_wire = b"zk-ams-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda _envelope: pytest.fail("rotated manifest permitted submission"),
    )

    with pytest.raises(RuntimeError, match="does not match the requested digest"):
        client.submit_signed_privacy_action_v1(
            PrivacyExact12ActionRequestV1(
                operation,
                crypto.current_wire,
                expected_manifest_digest=bytes([0xFF]) * 32,
            ),
            canonical_auth="auth",  # type: ignore[arg-type]
            network_id=NETWORK_ID,
            wait=False,
        )
    assert events == [f"inspect:{operation}", "reconstruct"]
