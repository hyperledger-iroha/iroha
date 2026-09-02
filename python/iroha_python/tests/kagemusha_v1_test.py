"""Focused source-only tests for the canonical three-message Kagemusha V1 surface."""

from __future__ import annotations

import hashlib
import importlib
import json
import sys
import types
from pathlib import Path

import pytest

_PACKAGE_ROOT = Path(__file__).resolve().parents[1] / "src" / "iroha_python"
_PURE_PACKAGE = "_iroha_python_kagemusha_source_test"
if _PURE_PACKAGE not in sys.modules:
    package = types.ModuleType(_PURE_PACKAGE)
    package.__path__ = [str(_PACKAGE_ROOT)]
    package.__package__ = _PURE_PACKAGE
    sys.modules[_PURE_PACKAGE] = package

    crypto = types.ModuleType(f"{_PURE_PACKAGE}.crypto")

    class NetworkId:
        """Minimal typed network identity used only by this source-codec test."""

        def __init__(self, value: bytes) -> None:
            raw = bytes(value)
            if len(raw) != 32:
                raise ValueError("NetworkId must contain 32 bytes")
            self._raw = raw

        @classmethod
        def from_bytes(cls, value: bytes) -> NetworkId:
            return cls(value)

        def to_bytes(self) -> bytes:
            return self._raw

        def __eq__(self, other: object) -> bool:
            return isinstance(other, NetworkId) and self._raw == other._raw

    def require_network_id(value: object, context: str = "network_id") -> NetworkId:
        if not isinstance(value, NetworkId):
            raise TypeError(f"{context} must be a NetworkId")
        return value

    crypto.NetworkId = NetworkId
    crypto._require_network_id = require_network_id
    sys.modules[f"{_PURE_PACKAGE}.crypto"] = crypto

_MODULE = importlib.import_module(f"{_PURE_PACKAGE}.kagemusha_v1")
KagemushaV1 = _MODULE.KagemushaV1
NetworkId = sys.modules[f"{_PURE_PACKAGE}.crypto"].NetworkId

_PUBLIC_KEY = bytes.fromhex(
    "04"
    "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
)


def _bytes(value: int, length: int = 32) -> bytes:
    return bytes((value,)) * length


def _signature() -> bytes:
    return bytes(31) + b"\x01" + bytes(31) + b"\x01"


def _base_context() -> dict[str, object]:
    network_id = NetworkId.from_bytes(bytes(range(1, 32)) + b"\x01")
    asset = KagemushaV1.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    incarnation = KagemushaV1.AssetIncarnation(_bytes(1))
    account = KagemushaV1.AccountId(
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    )
    public_key = KagemushaV1.DevicePublicKey(_PUBLIC_KEY)
    signature = KagemushaV1.DeviceSignature(_signature())
    credential = KagemushaV1.HardwareCredential(
        version=1,
        credential_id=_bytes(2),
        network_id=network_id,
        hardware_profile_id=_bytes(3),
        suite_id=_bytes(4),
        firmware_policy_digest=_bytes(5),
        policy_epoch=1,
        lane_commitment=_bytes(6),
        hardware_epoch_id=_bytes(7),
        hardware_epoch_generation=1,
        device_public_key=public_key,
        device_key_reference=KagemushaV1.device_key_reference(public_key),
        issued_at_ms=1,
        expires_at_ms=10_000,
        governance_signature=signature,
    )
    return {
        "network_id": network_id,
        "asset": asset,
        "incarnation": incarnation,
        "account": account,
        "signature": signature,
        "credential": credential,
    }


def _request(amount: int = 5) -> object:
    context = _base_context()
    return KagemushaV1.PaymentRequest(
        version=1,
        release_id=_bytes(8),
        network_id=context["network_id"],
        asset=context["asset"],
        asset_incarnation=context["incarnation"],
        scale=2,
        liability_pool_id=KagemushaV1.liability_pool_id(
            context["network_id"], context["asset"], context["incarnation"]
        ),
        recipient=context["account"],
        recipient_lane_id=context["credential"].lane_commitment,
        recipient_encryption_key=_bytes(20),
        amount=amount,
        hardware_credential=context["credential"],
        request_id=_bytes(9),
        issued_at_ms=100,
        expires_at_ms=200,
        signature=context["signature"],
    )


def _state(eq: int, ep: int) -> object:
    return KagemushaV1.PastaStateCommitment(eq=_bytes(eq), ep=_bytes(ep))


def _proof(semantic_digest: bytes) -> object:
    return KagemushaV1.PairedProof(
        version=1,
        eq_protocol_digest=_bytes(10),
        ep_protocol_digest=_bytes(11),
        semantic_digest=semantic_digest,
        guard_eq_credential_audit=_bytes(12),
        guard_ep_credential_audit=_bytes(13),
        eq_deferred_audit=_bytes(14),
        ep_deferred_audit=_bytes(15),
        eq_proof=_bytes(16, 8),
        ep_proof=_bytes(17, 8),
        eq_history=_bytes(18, 544),
        ep_history=_bytes(19, 544),
    )


def _encrypted_credit(seed: int) -> bytes:
    envelope = KagemushaV1.EncryptedCreditEnvelope(
        version=1,
        ephemeral_x25519_public_key=_bytes(seed),
        nonce=_bytes(seed + 1, 24),
        ciphertext_and_tag=_bytes(seed + 2, 216),
    )
    return KagemushaV1.encode_encrypted_credit_envelope(envelope)


def _payment(request: object, seed: int = 30) -> object:
    encrypted_credit = _encrypted_credit(seed)
    before = _state(seed + 4, seed + 5)
    after = _state(seed + 6, seed + 7)
    request_digest = KagemushaV1.payment_request_digest(request)
    transition_nullifier = _bytes(seed + 3)
    ciphertext_commitment = _bytes(seed + 8)
    credit_id = KagemushaV1.credit_id(
        transition_nullifier,
        request_digest,
        before,
        after,
        request.recipient_lane_id,
        request.recipient_encryption_key,
        request.amount,
        ciphertext_commitment,
    )
    lifecycle = KagemushaV1.LifecycleBinding(
        version=1,
        network_id=request.network_id,
        protocol_version=1,
        suite_id=request.hardware_credential.suite_id,
        vk_digest=_bytes(seed + 9),
        release_id=request.release_id,
        asset=request.asset,
        asset_incarnation=request.asset_incarnation,
        scale=request.scale,
        liability_pool_id=request.liability_pool_id,
        hardware_profile_id=request.hardware_credential.hardware_profile_id,
        policy_epoch=request.hardware_credential.policy_epoch,
        operation_kind="send_split",
        request_id=request.request_id,
        credit_id=credit_id,
        ciphertext_digest=KagemushaV1.ciphertext_digest(encrypted_credit),
    )
    statement = KagemushaV1.TransferStatement(
        version=1,
        lifecycle=lifecycle,
        amount=request.amount,
        transition_nullifier=transition_nullifier,
        sender_before_commitment=before,
        sender_after_commitment=after,
        request_digest=request_digest,
        recipient_lane_id=request.recipient_lane_id,
        recipient_encryption_key=request.recipient_encryption_key,
        ciphertext_commitment=ciphertext_commitment,
        committed_at_ms=150,
        hardware_transition_commitment=_bytes(seed + 10),
    )
    return KagemushaV1.Payment(
        version=1,
        statement=statement,
        proof=_proof(KagemushaV1.transfer_statement_digest(statement)),
        encrypted_credit=encrypted_credit,
    )


def _acknowledgement(request: object, payment: object) -> object:
    return KagemushaV1.Acknowledgement(
        version=1,
        request_digest=KagemushaV1.payment_request_digest(request),
        payment_digest=KagemushaV1.payment_digest(payment, request),
        inbox_receipt=KagemushaV1.InboxReceipt(
            version=1,
            credit_id=payment.statement.lifecycle.credit_id,
            receipt_commitment=_bytes(60),
        ),
        signature=_base_context()["signature"],
    )


def test_three_message_hard_cut_has_no_retired_public_api() -> None:
    for name in (
        "AcceptanceIntent",
        "AcceptanceTicket",
        "NoCommitClosure",
        "CommitCertificate",
        "CommitWrapperProof",
        "validate_pre_ticket_exchange",
        "validate_complete_exchange",
    ):
        assert not hasattr(KagemushaV1, name)
    assert set(KagemushaV1.payload_kinds).issuperset(
        {"payment_request", "payment", "acknowledgement"}
    )
    assert not any("acceptance" in kind for kind in KagemushaV1.payload_kinds)


def test_request_payment_ack_round_trip_and_text_transport() -> None:
    request = _request()
    payment = _payment(request)
    acknowledgement = _acknowledgement(request, payment)
    values = (
        ("payment_request", request, KagemushaV1.encode_payment_request, KagemushaV1.decode_payment_request, ()),
        ("payment", payment, KagemushaV1.encode_payment, KagemushaV1.decode_payment, (request,)),
        (
            "acknowledgement",
            acknowledgement,
            KagemushaV1.encode_acknowledgement,
            KagemushaV1.decode_acknowledgement,
            (request, payment),
        ),
    )
    for kind, value, encoder, decoder, bindings in values:
        raw = encoder(value, *bindings)
        assert encoder(decoder(raw, *bindings), *bindings) == raw
        text = KagemushaV1.encode_text(kind, raw)
        assert text.startswith("kgm1:")
        assert KagemushaV1.decode_text(kind, text) == raw
    assert KagemushaV1.validate_session(request, payment, acknowledgement) <= 9_211


def test_shared_fixture_is_exact_three_message_protocol() -> None:
    fixture = json.loads(
        (Path(__file__).resolve().parents[3] / "fixtures/offline/kagemusha_v1.json").read_text()
    )
    assert fixture["protocol"] == "KAGEMUSHA V1"
    assert fixture["text_prefix"] == "kgm1:"
    assert not any(
        retired in fixture
        for retired in (
            "acceptance_intent_authorization",
            "acceptance_ticket",
            "no_commit_closure",
            "complete_five_message",
        )
    )
    request = _request()
    payment = _payment(request)
    acknowledgement = _acknowledgement(request, payment)
    entries = (
        ("payment_request", KagemushaV1.encode_payment_request(request)),
        ("payment", KagemushaV1.encode_payment(payment, request)),
        (
            "acknowledgement",
            KagemushaV1.encode_acknowledgement(acknowledgement, request, payment),
        ),
    )
    for name, raw in entries:
        entry = fixture[name]
        assert bytes.fromhex(entry["norito_hex"]) == raw
        assert entry["raw_bytes"] == len(raw)
        assert entry["sha256"] == hashlib.sha256(raw).hexdigest()
        assert entry["kgm1"] == KagemushaV1.encode_text(name, raw)


def test_peer_aad_binds_state_lane_key_time_and_hardware_transition() -> None:
    request = _request()
    payment = _payment(request)
    context = KagemushaV1.peer_credit_context(payment.statement, request)
    encoded = KagemushaV1.encode_peer_credit_context(context)
    assert KagemushaV1.encode_peer_credit_context(
        KagemushaV1.decode_peer_credit_context(encoded)
    ) == encoded
    aad = KagemushaV1.encrypted_credit_aad_for_peer(payment.statement, request)
    assert aad.purpose == "peer"
    assert aad.credit_id == payment.statement.lifecycle.credit_id
    assert aad.context_digest != payment.statement.request_digest


def test_distinct_valid_payments_against_one_request_are_accepted() -> None:
    request = _request()
    first = _payment(request, 30)
    second = _payment(request, 70)
    assert first.statement.lifecycle.credit_id != second.statement.lifecycle.credit_id
    assert KagemushaV1.decode_payment(KagemushaV1.encode_payment(first, request), request) == first
    assert KagemushaV1.decode_payment(KagemushaV1.encode_payment(second, request), request) == second


def test_old_payment_shape_and_single_component_state_are_rejected() -> None:
    request = _request()
    payment = _payment(request)
    with pytest.raises(TypeError):
        KagemushaV1.Payment(
            version=1,
            statement=payment.statement,
            proof=payment.proof,
            encrypted_credit=payment.encrypted_credit,
            commit_certificate=_bytes(1),
        )
    fields = {name: getattr(payment.statement, name) for name in payment.statement.__slots__}
    fields["sender_before_commitment"] = _bytes(1)
    with pytest.raises(TypeError):
        KagemushaV1.TransferStatement(**fields)
