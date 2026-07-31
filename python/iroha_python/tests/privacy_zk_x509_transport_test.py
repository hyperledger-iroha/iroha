"""Fail-closed Python SDK tests for the native ZK-X509 action transport."""

from __future__ import annotations

from copy import deepcopy
from types import SimpleNamespace
from typing import Any, Callable, cast

import pytest

import iroha_python
import iroha_python.client as client_module
import iroha_python.crypto as crypto_module
from iroha_python import SorafsAliasPolicy, TransactionConfig, TransactionDraft
from iroha_python.client import ToriiClient
from iroha_python.privacy_catalog import (
    PRIVACY_PROTOCOL_IDS_V1,
    PrivacyCapabilitySnapshotV1,
    parse_privacy_capability_snapshot_v1,
)

X509_PROTOCOL = "iroha-zk-x509-stark-p256-v0"
SIGNED_X509_WIRE = b"canonical-signed-x509-wire"
SIGNED_OTHER_WIRE = b"canonical-signed-other-protocol-wire"
CANONICAL_GENESIS_HASH = bytes([0xA5]) * 32


def _tagged_protocol(protocol: str) -> dict[str, object]:
    return {"protocol": protocol, "value": None}


def _consensus_limits() -> dict[str, int]:
    return {
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
    }


def _x509_profile() -> dict[str, object]:
    return {
        "protocol_id": _tagged_protocol(X509_PROTOCOL),
        "proof_system_id": {
            "proof_system": "stark-fri-sha256-goldilocks",
            "value": None,
        },
        "engine_id": {"engine": "native-goldilocks-stark-fri", "value": None},
        "parameter_id": [1] * 32,
        "parameter_digest": [2] * 32,
        "verifier_digest": [3] * 32,
        "statement_schema_digest": [4] * 32,
        "engine_manifest_digest": [5] * 32,
        "protocol_limits": {"protocol": X509_PROTOCOL, "limits": None},
    }


def _capability_snapshot(
    *,
    compiled: bool = True,
    activation: bool = True,
    lifecycle: str = "active",
) -> PrivacyCapabilitySnapshotV1:
    rows: list[dict[str, object]] = [
        {
            "protocol_id": _tagged_protocol(protocol),
            "compiled_profile": {
                "status": "unavailable",
                "value": {"reason": "engine-unavailable", "detail": None},
            },
            "activation": None,
        }
        for protocol in PRIVACY_PROTOCOL_IDS_V1
    ]
    x509_row = rows[PRIVACY_PROTOCOL_IDS_V1.index(X509_PROTOCOL)]
    if compiled:
        profile = _x509_profile()
        x509_row["compiled_profile"] = {"status": "available", "value": profile}
        if activation:
            if lifecycle == "proposed":
                lifecycle_value: dict[str, object] = {
                    "state": "proposed",
                    "record": {"proposed_at_height": 40, "activate_at_height": 50},
                }
            else:
                lifecycle_value = {
                    "state": lifecycle,
                    "record": {
                        "proposed_at_height": 1,
                        "activated_at_height": 2,
                        "state_since_height": 2 if lifecycle == "active" else 3,
                    },
                }
            x509_row["activation"] = {
                **deepcopy(profile),
                "lifecycle": lifecycle_value,
                "pending_protocol_limits_tightening": None,
                "assurance": {"assurance": "experimental", "value": None},
            }
    return parse_privacy_capability_snapshot_v1(
        {
            "version": 1,
            "committed_height": 42,
            "consensus_policy": {
                "current_limits": _consensus_limits(),
                "pending_tightening": None,
            },
            "protocols": rows,
        }
    )


def _duplicate_x509_row(snapshot: dict[str, Any]) -> None:
    row = snapshot["protocols"][PRIVACY_PROTOCOL_IDS_V1.index(X509_PROTOCOL)]
    snapshot["protocols"].append(deepcopy(row))


def _mismatch_x509_compiled_binding(snapshot: dict[str, Any]) -> None:
    row = snapshot["protocols"][PRIVACY_PROTOCOL_IDS_V1.index(X509_PROTOCOL)]
    row["compiled_profile"]["value"]["protocol_id"]["protocol"] = (
        "vega-existing-credential-zk-v0"
    )


def _mismatch_x509_activation_binding(snapshot: dict[str, Any]) -> None:
    row = snapshot["protocols"][PRIVACY_PROTOCOL_IDS_V1.index(X509_PROTOCOL)]
    row["activation"]["protocol_id"]["protocol"] = "vega-existing-credential-zk-v0"


class _FakeCrypto:
    def __init__(self, events: list[str]) -> None:
        self.events = events
        self.envelope = SimpleNamespace(
            signed_transaction_versioned=SIGNED_X509_WIRE,
            hash=bytes([7]) * 32,
        )

    def inspect_signed_privacy_zk_x509_identity_presentation_action_v1(
        self,
        wire: object,
        genesis_hash: object,
    ) -> dict[str, object]:
        self.events.append("inspect")
        if wire != SIGNED_X509_WIRE:
            raise ValueError("wrong privacy protocol")
        if genesis_hash != CANONICAL_GENESIS_HASH:
            raise ValueError("wrong genesis")
        return {"protocol_id": X509_PROTOCOL}

    def signed_transaction_envelope_from_versioned_v1(
        self,
        wire: object,
    ) -> SimpleNamespace:
        self.events.append("reconstruct")
        assert wire == SIGNED_X509_WIRE
        return self.envelope


def _client_with_crypto(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[ToriiClient, _FakeCrypto, list[str]]:
    events: list[str] = []
    fake_crypto = _FakeCrypto(events)
    monkeypatch.setattr(client_module, "_CRYPTO_MODULE", fake_crypto)
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
        ToriiClient("http://torii.invalid", sorafs_alias_policy=alias_policy),
        fake_crypto,
        events,
    )


def test_crypto_x509_inspector_is_public_and_forwards_exact_genesis(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[bytes, bytes]] = []

    def inspect(wire: bytes, genesis_hash: bytes) -> dict[str, object]:
        calls.append((wire, genesis_hash))
        return {"protocol_id": X509_PROTOCOL}

    monkeypatch.setattr(
        crypto_module,
        "_crypto",
        SimpleNamespace(
            inspect_signed_privacy_zk_x509_identity_presentation_action_v1=inspect
        ),
    )
    result = crypto_module.inspect_signed_privacy_zk_x509_identity_presentation_action_v1(
        bytearray(SIGNED_X509_WIRE),
        memoryview(CANONICAL_GENESIS_HASH),
    )

    assert result == {"protocol_id": X509_PROTOCOL}
    assert calls == [(SIGNED_X509_WIRE, CANONICAL_GENESIS_HASH)]
    assert (
        iroha_python.inspect_signed_privacy_zk_x509_identity_presentation_action_v1
        is crypto_module.inspect_signed_privacy_zk_x509_identity_presentation_action_v1
    )
    assert "inspect_signed_privacy_zk_x509_identity_presentation_action_v1" in (
        iroha_python.__all__
    )


def test_crypto_x509_inspector_rejects_bad_inputs_and_native_contract_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    inspect = crypto_module.inspect_signed_privacy_zk_x509_identity_presentation_action_v1
    with pytest.raises(TypeError, match="signed_transaction_versioned"):
        inspect("not-bytes", CANONICAL_GENESIS_HASH)  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="canonical_genesis_hash"):
        inspect(SIGNED_X509_WIRE, "not-bytes")  # type: ignore[arg-type]

    monkeypatch.setattr(crypto_module, "_crypto", SimpleNamespace())
    with pytest.raises(RuntimeError, match="rebuild the extension"):
        inspect(SIGNED_X509_WIRE, CANONICAL_GENESIS_HASH)

    monkeypatch.setattr(
        crypto_module,
        "_crypto",
        SimpleNamespace(
            inspect_signed_privacy_zk_x509_identity_presentation_action_v1=lambda *_: []
        ),
    )
    with pytest.raises(RuntimeError, match="invalid result"):
        inspect(SIGNED_X509_WIRE, CANONICAL_GENESIS_HASH)

    def reject(*_: object) -> None:
        raise RuntimeError("native detail must not escape")

    monkeypatch.setattr(
        crypto_module,
        "_crypto",
        SimpleNamespace(
            inspect_signed_privacy_zk_x509_identity_presentation_action_v1=reject
        ),
    )
    with pytest.raises(ValueError, match="invalid canonical signed ZK-X509"):
        inspect(SIGNED_X509_WIRE, CANONICAL_GENESIS_HASH)


def test_transaction_draft_delegates_exact_x509_prepare_and_sign_inputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[object, ...]] = []
    signed_result = object()

    class Builder:
        def prepare_privacy_zk_x509_identity_presentation_action_v1(
            self, statement: bytes
        ) -> bytes:
            calls.append(("prepare", statement))
            return bytes([9]) * 32

        def sign_privacy_zk_x509_identity_presentation_action_v1(
            self,
            private_key: bytes,
            genesis_hash: bytes,
            statement: bytes,
            proof: bytes,
        ) -> object:
            calls.append(("sign", private_key, genesis_hash, statement, proof))
            return signed_result

    draft = TransactionDraft(
        TransactionConfig(
            chain_id="test-chain",
            authority="ed0120" + "11" * 32,
            fee_payment={
                "payer": "authority",
                "value": {"charge_limits": [], "gas_limit": 1000},
            },
            creation_time_ms=42,
        )
    )
    monkeypatch.setattr(draft, "to_builder", lambda: Builder())

    statement = b"canonical-typed-statement"
    proof = b"X5S1-proof"
    assert draft.prepare_privacy_zk_x509_identity_presentation_action_v1(
        canonical_statement_archive=statement
    ) == bytes([9]) * 32
    assert (
        draft.sign_privacy_zk_x509_identity_presentation_action_v1(
            b"private-key",
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            canonical_statement_archive=statement,
            credential_proof=proof,
        )
        is signed_result
    )
    assert calls == [
        ("prepare", statement),
        ("sign", b"private-key", CANONICAL_GENESIS_HASH, statement, proof),
    ]


def test_transaction_draft_rejects_x509_action_mixing_before_native_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    draft = TransactionDraft(
        TransactionConfig(
            chain_id="test-chain",
            authority="ed0120" + "11" * 32,
            fee_payment={
                "payer": "authority",
                "value": {"charge_limits": [], "gas_limit": 1000},
            },
            creation_time_ms=42,
        )
    )
    draft.add_instruction(object())  # type: ignore[arg-type]
    monkeypatch.setattr(
        draft,
        "to_builder",
        lambda: pytest.fail("mixed X509 draft reached native builder"),
    )

    with pytest.raises(ValueError, match="otherwise empty"):
        draft.prepare_privacy_zk_x509_identity_presentation_action_v1(
            canonical_statement_archive=b"statement"
        )
    with pytest.raises(ValueError, match="otherwise empty"):
        draft.sign_privacy_zk_x509_identity_presentation_action_v1(
            b"private-key",
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            canonical_statement_archive=b"statement",
            credential_proof=b"proof",
        )


def test_x509_transport_authenticates_live_gates_and_submits_exactly_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, fake_crypto, events = _client_with_crypto(monkeypatch)
    capability_calls = 0

    def capabilities() -> PrivacyCapabilitySnapshotV1:
        nonlocal capability_calls
        capability_calls += 1
        events.append("capabilities")
        return _capability_snapshot()

    submissions: list[object] = []

    def submit(envelope: object) -> dict[str, str]:
        events.append("submit")
        submissions.append(envelope)
        return {"status": "queued"}

    monkeypatch.setattr(client, "privacy_capabilities_v1", capabilities)
    monkeypatch.setattr(client, "submit_transaction_envelope", submit)

    envelope, result = (
        client.submit_signed_privacy_zk_x509_identity_presentation_action_v1(
            SIGNED_X509_WIRE,
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            wait=False,
        )
    )

    assert envelope is fake_crypto.envelope
    assert result == {"status": "queued"}
    assert submissions == [fake_crypto.envelope]
    assert capability_calls == 1
    assert events == ["inspect", "capabilities", "reconstruct", "submit"]


def test_x509_transport_wait_path_submits_through_wait_helper_exactly_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, fake_crypto, events = _client_with_crypto(monkeypatch)

    def capabilities() -> PrivacyCapabilitySnapshotV1:
        events.append("capabilities")
        return _capability_snapshot()

    waits: list[tuple[object, dict[str, object]]] = []

    def submit_and_wait(envelope: object, **kwargs: object) -> dict[str, str]:
        events.append("wait")
        waits.append((envelope, kwargs))
        return {"status": "Committed"}

    monkeypatch.setattr(client, "privacy_capabilities_v1", capabilities)
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda _: pytest.fail("wait path performed a separate direct submission"),
    )
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope_and_wait",
        submit_and_wait,
    )

    envelope, result = (
        client.submit_signed_privacy_zk_x509_identity_presentation_action_v1(
            SIGNED_X509_WIRE,
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            interval=0.25,
            timeout=5.0,
            max_attempts=3,
            scope="local",
        )
    )

    assert envelope is fake_crypto.envelope
    assert result == {"status": "Committed"}
    assert len(waits) == 1
    assert waits[0][0] is fake_crypto.envelope
    assert waits[0][1]["interval"] == 0.25
    assert waits[0][1]["timeout"] == 5.0
    assert waits[0][1]["max_attempts"] == 3
    assert waits[0][1]["scope"] == "local"
    assert events == ["inspect", "capabilities", "reconstruct", "wait"]


def test_x509_transport_rejects_wrong_protocol_before_capability_fetch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _, events = _client_with_crypto(monkeypatch)
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda: pytest.fail("wrong protocol reached capability fetch"),
    )
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda _: pytest.fail("wrong protocol was submitted"),
    )

    with pytest.raises(ValueError, match="wrong privacy protocol"):
        client.submit_signed_privacy_zk_x509_identity_presentation_action_v1(
            SIGNED_OTHER_WIRE,
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            wait=False,
        )
    assert events == ["inspect"]


@pytest.mark.parametrize(
    ("mutate", "message"),
    (
        (_duplicate_x509_row, "exactly one"),
        (_mismatch_x509_compiled_binding, "compiled profile has a mismatched binding"),
        (_mismatch_x509_activation_binding, "activation has a mismatched binding"),
    ),
    ids=("duplicate-row", "compiled-binding", "activation-binding"),
)
def test_x509_transport_rejects_malformed_capability_bindings_before_submission(
    monkeypatch: pytest.MonkeyPatch,
    mutate: Callable[[dict[str, Any]], None],
    message: str,
) -> None:
    client, _, events = _client_with_crypto(monkeypatch)
    snapshot = cast(dict[str, Any], deepcopy(_capability_snapshot()))
    mutate(snapshot)
    monkeypatch.setattr(client, "privacy_capabilities_v1", lambda: snapshot)
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda _: pytest.fail("malformed capability row permitted submission"),
    )

    with pytest.raises(RuntimeError, match=message):
        client.submit_signed_privacy_zk_x509_identity_presentation_action_v1(
            SIGNED_X509_WIRE,
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            wait=False,
        )
    assert events == ["inspect"]


@pytest.mark.parametrize(
    ("snapshot", "message"),
    (
        (_capability_snapshot(compiled=False, activation=False), "compiled profile"),
        (_capability_snapshot(activation=False), "no governed activation"),
        (_capability_snapshot(lifecycle="proposed"), "not active"),
        (_capability_snapshot(lifecycle="suspended"), "not active"),
        (_capability_snapshot(lifecycle="retired"), "not active"),
    ),
    ids=("unavailable", "no-activation", "proposed", "suspended", "retired"),
)
def test_x509_transport_fails_closed_for_unavailable_or_inactive_capability(
    monkeypatch: pytest.MonkeyPatch,
    snapshot: PrivacyCapabilitySnapshotV1,
    message: str,
) -> None:
    client, _, events = _client_with_crypto(monkeypatch)
    monkeypatch.setattr(client, "privacy_capabilities_v1", lambda: snapshot)
    monkeypatch.setattr(
        client,
        "submit_transaction_envelope",
        lambda _: pytest.fail("inactive X509 protocol was submitted"),
    )

    with pytest.raises(RuntimeError, match=message):
        client.submit_signed_privacy_zk_x509_identity_presentation_action_v1(
            SIGNED_X509_WIRE,
            canonical_genesis_hash=CANONICAL_GENESIS_HASH,
            wait=False,
        )
    assert events == ["inspect"]
