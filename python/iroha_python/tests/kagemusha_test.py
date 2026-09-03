"""Focused source-only tests for the canonical three-message KAGEMUSHA surface."""

from __future__ import annotations

import hashlib
import importlib
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

_MODULE = importlib.import_module(f"{_PURE_PACKAGE}.kagemusha")
Kagemusha = _MODULE.Kagemusha
NetworkId = sys.modules[f"{_PURE_PACKAGE}.crypto"].NetworkId

_PUBLIC_KEY = bytes.fromhex(
    "04"
    "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
)
_CREDENTIAL_ID = bytes.fromhex(
    "5fdf1ee6473b05a5a028aba63823662c49941ce2dec32fc2405a555ae15f429b"
)
_CREDENTIAL_SIGNATURE = bytes.fromhex(
    "cd6f105b119b14bc0604780765cc15b7e516446390b2fd166e299447924fd327"
    "04f7ca3d360dde119f8181124bf0f36407a32b20230e0b9d4771b76aaae22b05"
)
_REQUEST_SIGNATURE = bytes.fromhex(
    "6d88630e0afc0080786605389464dfdc451820464b13de4917142fa9bab63d4c"
    "39b06118015b2120b9ed81ee5bad0af05d2ad2f3d94d172f8deba562dc6842bb"
)
_ACKNOWLEDGEMENT_SIGNATURE = bytes.fromhex(
    "09605b382626027e316d21215ffb2dcf34756fb5f06bc30263783fefbb05a3a1"
    "44fc3a5569c544ef9b31266cc3d7fb246e96867f2f649cc557e34a29afa90e3d"
)


def _bytes(value: int, length: int = 32) -> bytes:
    return bytes((value,)) * length


def _base_context() -> dict[str, object]:
    network_id = NetworkId.from_bytes(bytes(range(1, 32)) + b"\x01")
    asset = Kagemusha.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    incarnation = Kagemusha.AssetIncarnation(_bytes(1))
    account = Kagemusha.AccountId(
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    )
    public_key = Kagemusha.DevicePublicKey(_PUBLIC_KEY)
    credential = Kagemusha.HardwareCredential(
        version=1,
        credential_id=_CREDENTIAL_ID,
        network_id=network_id,
        hardware_profile_id=_bytes(3),
        suite_id=_bytes(4),
        firmware_policy_digest=_bytes(5),
        policy_epoch=1,
        lane_commitment=_bytes(6),
        hardware_epoch_id=_bytes(7),
        hardware_epoch_generation=1,
        device_public_key=public_key,
        device_key_reference=Kagemusha.device_key_reference(public_key),
        issued_at_ms=1,
        expires_at_ms=10_000,
        governance_signature=Kagemusha.DeviceSignature(_CREDENTIAL_SIGNATURE),
    )
    return {
        "network_id": network_id,
        "asset": asset,
        "incarnation": incarnation,
        "account": account,
        "credential": credential,
    }


def _request(amount: int = 5) -> object:
    context = _base_context()
    return Kagemusha.PaymentRequest(
        version=1,
        release_id=_bytes(8),
        network_id=context["network_id"],
        asset=context["asset"],
        asset_incarnation=context["incarnation"],
        scale=2,
        liability_pool_id=Kagemusha.liability_pool_id(
            context["network_id"], context["asset"], context["incarnation"]
        ),
        recipient=context["account"],
        amount=amount,
        recipient_encryption_key=_bytes(20),
        hardware_credential=context["credential"],
        request_id=_bytes(9),
        issued_at_ms=100,
        expires_at_ms=200,
        signature=Kagemusha.DeviceSignature(_REQUEST_SIGNATURE),
    )


def _paired_proof(semantic_digest: bytes, parity_proof_bytes: int = 8) -> object:
    return Kagemusha.PairedProof(
        version=1,
        eq_protocol_digest=_bytes(10),
        ep_protocol_digest=_bytes(11),
        semantic_digest=semantic_digest,
        guard_eq_credential_audit=_bytes(12),
        guard_ep_credential_audit=_bytes(13),
        eq_deferred_audit=_bytes(14),
        ep_deferred_audit=_bytes(15),
        eq_proof=_bytes(16, parity_proof_bytes),
        ep_proof=_bytes(17, parity_proof_bytes),
        eq_history=_bytes(18, 544),
        ep_history=_bytes(19, 544),
    )


def _encrypted_credit(seed: int) -> bytes:
    envelope = Kagemusha.EncryptedCreditEnvelope(
        version=1,
        ephemeral_x25519_public_key=_bytes(seed),
        nonce=_bytes(seed + 1, 24),
        ciphertext_and_tag=_bytes(seed + 2, 216),
    )
    return Kagemusha.encode_encrypted_credit_envelope(envelope)


def _certificate(
    lifecycle: object, evidence: object, nullifier: bytes, seed: int = 70
) -> object:
    fields = {
        "version": 1,
        "certificate_id": _bytes(seed),
        "candidate_envelope_digest": _bytes(seed + 1),
        "lifecycle_binding_digest": Kagemusha.lifecycle_binding_digest(lifecycle),
        "transition_nullifier": nullifier,
        "outbox_reservation_commitment": _bytes(seed + 2),
        "commit_evidence": evidence,
        "hardware_profile_id": lifecycle.hardware_profile_id,
        "policy_epoch": lifecycle.policy_epoch,
        "hardware_terminal_commitment": _bytes(seed + 3),
    }
    provisional = Kagemusha.CommitCertificate(**fields)
    fields["certificate_id"] = Kagemusha.commit_certificate_id(provisional)
    return Kagemusha.CommitCertificate(**fields)


def _redemption_proof(
    semantic_digest: bytes,
    certificate: object,
    lifecycle: object,
    evidence: object,
    nullifier: bytes,
) -> object:
    return Kagemusha.RedemptionProof(
        version=1,
        eq_protocol_digest=_bytes(80),
        ep_protocol_digest=_bytes(81),
        semantic_digest=semantic_digest,
        candidate_envelope_digest=certificate.candidate_envelope_digest,
        commit_certificate_digest=Kagemusha.commit_certificate_digest(
            certificate, lifecycle, evidence, nullifier
        ),
        eq_deferred_audit=_bytes(82),
        ep_deferred_audit=_bytes(83),
        eq_proof=_bytes(84, 8),
        ep_proof=_bytes(85, 8),
        eq_history=_bytes(86, 544),
        ep_history=_bytes(87, 544),
    )


def _replace(value: object, **updates: object) -> object:
    fields = {name: getattr(value, name) for name in value.__slots__}
    fields.update(updates)
    return type(value)(**fields)


def _payment(request: object, seed: int = 50, parity_proof_bytes: int = 8) -> object:
    """Create codec shape fixtures, never a substitute for genuine proof qualification."""
    encrypted_credit = _encrypted_credit(seed)
    nullifier = _bytes(seed + 3)
    request_digest = Kagemusha.payment_request_digest(request)
    output = Kagemusha.PaymentOutput(
        version=1, request_digest=request_digest, amount=request.amount,
        sender_before_commitment=_bytes(seed + 1),
        sender_after_commitment=_bytes(seed + 2),
        transition_nullifier=nullifier, credit_id=Kagemusha.credit_id(nullifier, request_digest),
        ciphertext_commitment=_bytes(seed + 4),
        commit_evidence=Kagemusha.TrustedCommitTime(time_evidence_commitment=_bytes(seed + 6)),
        committed_at_ms=150,
    )
    provisional = Kagemusha.CommitCertificate(
        version=1, certificate_id=_bytes(70), candidate_envelope_digest=_bytes(71),
        lifecycle_binding_digest=_bytes(72), transition_nullifier=nullifier,
        outbox_reservation_commitment=_bytes(73), commit_evidence=output.commit_evidence,
        hardware_profile_id=_bytes(74), policy_epoch=1, hardware_terminal_commitment=_bytes(75),
    )
    certificate = _replace(provisional, certificate_id=Kagemusha.commit_certificate_id(provisional))
    proof = Kagemusha.PaymentProof(
        version=1, eq_protocol_digest=_bytes(80), ep_protocol_digest=_bytes(81),
        semantic_digest=Kagemusha.payment_body_digest(output, encrypted_credit),
        candidate_envelope_digest=certificate.candidate_envelope_digest,
        commit_certificate_digest=Kagemusha.commit_certificate_digest(certificate),
        eq_deferred_audit=_bytes(82), ep_deferred_audit=_bytes(83),
        eq_proof=_bytes(84, parity_proof_bytes), ep_proof=_bytes(85, parity_proof_bytes),
        eq_history=_bytes(86, 544), ep_history=_bytes(87, 544),
    )
    return Kagemusha.Payment(version=1, output=output, encrypted_credit=encrypted_credit, commit_certificate=certificate, proof=proof)

def _acknowledgement(request: object, payment: object) -> object:
    return Kagemusha.Acknowledgement(
        version=1,
        request_digest=Kagemusha.payment_request_digest(request),
        payment_digest=Kagemusha.payment_digest(payment, request),
        inbox_receipt=Kagemusha.InboxReceipt(
            version=1,
            credit_id=payment.output.credit_id,
            receipt_commitment=_bytes(100),
        ),
        signature=Kagemusha.DeviceSignature(_ACKNOWLEDGEMENT_SIGNATURE),
    )


def _top_up_request(mint_parity_proof_bytes: int = 8) -> object:
    request = _request(40)
    encrypted_credit = _encrypted_credit(30)
    recipient_key = _bytes(20)
    context = Kagemusha.MintAuthorizationContext(
        version=1,
        operation_id=_bytes(0x77),
        release_id=request.release_id,
        suite_id=request.hardware_credential.suite_id,
        vk_digest=_bytes(0x78),
        artifact_manifest_digest=_bytes(0x79),
        network_id=request.network_id,
        asset=request.asset,
        asset_incarnation=request.asset_incarnation,
        scale=request.scale,
        liability_pool_id=request.liability_pool_id,
        amount=40,
        payer=request.recipient,
        recipient=request.recipient,
        hardware_credential_id=request.hardware_credential.credential_id,
        hardware_profile_id=request.hardware_credential.hardware_profile_id,
        policy_epoch=request.hardware_credential.policy_epoch,
        recipient_credential_commitment=_bytes(0x7A),
        credit_commitment=_bytes(0x7B),
        recipient_one_time_key=recipient_key,
    )
    statement = Kagemusha.MintAuthorizationStatement(
        version=1,
        context=context,
        issuance_commitment=_bytes(0x7D),
        credit_id=_bytes(0x7E),
        ciphertext_digest=Kagemusha.ciphertext_digest(encrypted_credit),
    )
    authorization = Kagemusha.MintAuthorization(
        version=1,
        statement=statement,
        proof=_paired_proof(
            Kagemusha.mint_authorization_statement_digest(statement),
            mint_parity_proof_bytes,
        ),
    )
    return Kagemusha.TopUpRequest(
        version=1,
        operation_id=context.operation_id,
        issuance_commitment=statement.issuance_commitment,
        credit_id=statement.credit_id,
        release_id=context.release_id,
        suite_id=context.suite_id,
        vk_digest=context.vk_digest,
        network_id=context.network_id,
        asset=context.asset,
        asset_incarnation=context.asset_incarnation,
        scale=context.scale,
        amount=context.amount,
        liability_pool_id=context.liability_pool_id,
        payer=context.payer,
        recipient=context.recipient,
        hardware_credential=request.hardware_credential,
        recipient_credential_commitment=context.recipient_credential_commitment,
        credit_commitment=context.credit_commitment,
        recipient_one_time_key=context.recipient_one_time_key,
        encrypted_credit=encrypted_credit,
        artifact_manifest_digest=context.artifact_manifest_digest,
        mint_authorization=authorization,
    )


def _mint_stage_pair() -> tuple[object, object]:
    top_up = _top_up_request()
    original_authorization = top_up.mint_authorization
    assert original_authorization is not None
    context = original_authorization.statement.context
    lifecycle = Kagemusha.LifecycleBinding(
        version=1,
        network_id=context.network_id,
        protocol_version=1,
        suite_id=context.suite_id,
        vk_digest=context.vk_digest,
        release_id=context.release_id,
        asset=context.asset,
        asset_incarnation=context.asset_incarnation,
        scale=context.scale,
        liability_pool_id=context.liability_pool_id,
        hardware_profile_id=context.hardware_profile_id,
        policy_epoch=context.policy_epoch,
        operation_kind="mint_fold",
        request_id=bytes(32),
        receiver_lane_commitment=bytes(32),
        credit_id=original_authorization.statement.credit_id,
        ciphertext_digest=original_authorization.statement.ciphertext_digest,
    )
    provisional = Kagemusha.MintCreditStatement(
        version=1,
        lifecycle=lifecycle,
        recipient_credential_commitment=context.recipient_credential_commitment,
        authorization_context_digest=Kagemusha.mint_authorization_context_digest(context),
        mint_authorization_digest=_bytes(0x90),
        amount=context.amount,
        issuance_commitment=original_authorization.statement.issuance_commitment,
        recipient=context.recipient,
        credit_commitment=context.credit_commitment,
        minted_at_ms=123,
    )
    credit_id = Kagemusha.mint_credit_id(provisional)
    lifecycle = _replace(lifecycle, credit_id=credit_id)
    authorization_statement = Kagemusha.MintAuthorizationStatement(
        version=1,
        context=context,
        issuance_commitment=provisional.issuance_commitment,
        credit_id=credit_id,
        ciphertext_digest=provisional.lifecycle.ciphertext_digest,
    )
    authorization = Kagemusha.MintAuthorization(
        version=1,
        statement=authorization_statement,
        proof=_paired_proof(
            Kagemusha.mint_authorization_statement_digest(authorization_statement), 8
        ),
    )
    statement = _replace(
        provisional,
        lifecycle=lifecycle,
        mint_authorization_digest=Kagemusha.mint_authorization_digest(authorization),
    )
    credit = Kagemusha.MintCredit(
        version=1,
        statement=statement,
        proof=_paired_proof(Kagemusha.mint_credit_statement_digest(statement), 8),
        finality_certificate_binding=_bytes(0x91),
        finality_authority_head=_bytes(0x92),
        finality_genesis_roster_id=_bytes(0x93),
        finality_proof_binding_digest=_bytes(0x94),
        encrypted_credit=top_up.encrypted_credit,
        artifact_manifest_digest=context.artifact_manifest_digest,
    )
    return authorization, credit


def _redemption_voucher(request: object) -> object:
    zero = bytes(32)
    lifecycle = Kagemusha.LifecycleBinding(
        version=1,
        network_id=request.network_id,
        protocol_version=1,
        suite_id=request.hardware_credential.suite_id,
        vk_digest=_bytes(0xA3),
        release_id=request.release_id,
        asset=request.asset,
        asset_incarnation=request.asset_incarnation,
        scale=request.scale,
        liability_pool_id=request.liability_pool_id,
        hardware_profile_id=request.hardware_credential.hardware_profile_id,
        policy_epoch=request.hardware_credential.policy_epoch,
        operation_kind="redeem_split",
        request_id=zero,
        receiver_lane_commitment=zero,
        credit_id=zero,
        ciphertext_digest=zero,
    )
    evidence = Kagemusha.MonotonicLease(lease_evidence_commitment=_bytes(0xA4))
    values = {
        "version": 1,
        "lifecycle": lifecycle,
        "amount": 12,
        "beneficiary": request.recipient,
        "terminal_nullifier": _bytes(0xA2),
        "redemption_commitment": _bytes(0xA8),
        "redemption_id": _bytes(0xA9),
        "commit_evidence": evidence,
    }
    provisional = Kagemusha.RedemptionStatement(**values)
    values["redemption_id"] = Kagemusha.redemption_id(provisional)
    statement = Kagemusha.RedemptionStatement(**values)
    certificate = _certificate(lifecycle, evidence, statement.terminal_nullifier, 0xB0)
    return Kagemusha.RedemptionVoucher(
        version=1,
        statement=statement,
        commit_certificate=certificate,
        proof=_redemption_proof(
            Kagemusha.redemption_statement_digest(statement),
            certificate,
            lifecycle,
            evidence,
            statement.terminal_nullifier,
        ),
        artifact_manifest_digest=_bytes(0xB9),
    )


def _exchange(parity_proof_bytes: int = 8) -> tuple[object, ...]:
    request = _request()
    payment = _payment(request, parity_proof_bytes=parity_proof_bytes)
    return request, payment, _acknowledgement(request, payment)


def _digest(domain: bytes, transcript: bytes) -> bytes:
    return hashlib.sha256(domain + b"\0" + len(transcript).to_bytes(8, "little") + transcript).digest()


def test_public_facade_and_three_message_caps() -> None:
    assert Kagemusha.__name__ == "Kagemusha"
    assert _MODULE.__all__ == ["Kagemusha"]
    assert Kagemusha.maximum_complete_exchange_raw_bytes == 9_211
    assert Kagemusha.maximum_complete_exchange_text_bytes == 12_288
    assert Kagemusha.maximum_payment_proof_bytes == 6_528
    assert Kagemusha.payment_outbox_minimum_bytes == 25_728
    for name in ("AmountPolicy", "SingleExact", "PartialUntilTotal", "BoundedMultiPayment",
                 "OpenReceive", "AcceptanceIntent", "AcceptanceTicket", "NoCommitClosure",
                 "encode_acceptance_intent", "decode_acceptance_ticket", "validate_pre_ticket_exchange"):
        assert not hasattr(Kagemusha, name)


def test_operation_21_mint_stage_bodies_are_canonical_bounded_and_credit_bound() -> None:
    authorization, credit = _mint_stage_pair()
    authorization_bytes = Kagemusha.encode_mint_authorization(authorization)
    credit_bytes = Kagemusha.encode_mint_credit(credit, authorization)
    command = Kagemusha.DeviceMintStageCommand(
        version=1,
        canonical_authorization=authorization_bytes,
        canonical_mint_credit=credit_bytes,
    )
    command_bytes = Kagemusha.encode_device_mint_stage_command_shape(command)
    assert len(command_bytes) <= Kagemusha.maximum_device_mint_stage_command_bytes
    decoded = Kagemusha.decode_device_mint_stage_command_shape_exact(command_bytes)
    assert decoded.canonical_authorization == authorization_bytes
    assert decoded.canonical_mint_credit == credit_bytes
    assert (
        Kagemusha.encode_device_mint_stage_command_shape(
            authorization_bytes, credit_bytes
        )
        == command_bytes
    )

    for disposition in (
        Kagemusha.device_mint_stage_disposition_staged,
        Kagemusha.device_mint_stage_disposition_exact_duplicate,
    ):
        result = Kagemusha.DeviceMintStageResult(
            version=1,
            disposition=disposition,
            credit_id=credit.statement.lifecycle.credit_id,
        )
        result_bytes = Kagemusha.encode_device_mint_stage_result_shape(result, command)
        assert len(result_bytes) <= Kagemusha.maximum_device_mint_stage_result_bytes
        result_roundtrip = Kagemusha.decode_device_mint_stage_result_shape_exact(
            result_bytes, decoded
        )
        assert (
            Kagemusha.encode_device_mint_stage_result_shape(result_roundtrip, decoded)
            == result_bytes
        )

    with pytest.raises(Kagemusha.Error):
        Kagemusha.decode_device_mint_stage_command_shape_exact(command_bytes + b"\0")
    with pytest.raises(Kagemusha.Error):
        Kagemusha.encode_device_mint_stage_command_shape(
            Kagemusha.DeviceMintStageCommand(
                version=1,
                canonical_authorization=authorization_bytes + b"\0",
                canonical_mint_credit=credit_bytes,
            )
        )
    with pytest.raises(Kagemusha.Error, match="credit ID"):
        Kagemusha.encode_device_mint_stage_result_shape(
            Kagemusha.DeviceMintStageResult(
                version=1, disposition=0, credit_id=_bytes(0xEE)
            ),
            command,
        )
    with pytest.raises(Kagemusha.Error, match="disposition"):
        Kagemusha.DeviceMintStageResult(version=1, disposition=2, credit_id=_bytes(1))
    with pytest.raises(Kagemusha.Error, match="7936"):
        Kagemusha.DeviceMintStageCommand(
            version=1,
            canonical_authorization=bytes(7937),
            canonical_mint_credit=credit_bytes,
        )

    bad_statement = _replace(
        credit.statement,
        lifecycle=_replace(credit.statement.lifecycle, credit_id=_bytes(0xEF)),
    )
    with pytest.raises(Kagemusha.Error, match="credit ID"):
        Kagemusha.mint_credit_statement_digest(bad_statement)
    with pytest.raises(Kagemusha.Error, match="credit ID"):
        Kagemusha.validate_mint_credit_against_authorization(
            _replace(credit, statement=bad_statement), authorization
        )
    substituted_authorization = _replace(
        authorization,
        proof=_replace(authorization.proof, eq_proof=_bytes(0xF0, 9)),
    )
    with pytest.raises(Kagemusha.Error, match="authorization digest"):
        Kagemusha.encode_device_mint_stage_command_shape(
            Kagemusha.encode_mint_authorization(substituted_authorization), credit_bytes
        )


def test_operation_21_rejects_malformed_results_and_copies_command_inputs() -> None:
    raw = Kagemusha.encode_device_mint_stage_result_shape(
        Kagemusha.DeviceMintStageResult(version=1, disposition=0, credit_id=_bytes(1))
    )
    assert len(raw) == 78
    assert raw[40:46] == bytes((2, 1, 0, 1, 0, 32))

    for offset, replacement in ((41, 2), (44, 2), (43, 2), (39, 0)):
        mutated = bytearray(raw)
        mutated[offset] = replacement
        payload = mutated[40:]
        mutated[31:39] = _MODULE._crc64_xz(payload).to_bytes(8, "little")
        with pytest.raises(Kagemusha.Error):
            Kagemusha.decode_device_mint_stage_result_shape_exact(mutated)
    zero_credit_id = bytearray(raw)
    zero_credit_id[46:] = bytes(32)
    zero_credit_id[31:39] = _MODULE._crc64_xz(zero_credit_id[40:]).to_bytes(
        8, "little"
    )
    with pytest.raises(Kagemusha.Error):
        Kagemusha.decode_device_mint_stage_result_shape_exact(zero_credit_id)
    for invalid in (raw[:-1], raw + b"\0", bytes(129)):
        with pytest.raises(Kagemusha.Error):
            Kagemusha.decode_device_mint_stage_result_shape_exact(invalid)
    with pytest.raises(Kagemusha.Error, match="65536"):
        Kagemusha.decode_device_mint_stage_command_shape_exact(bytes(65537))

    authorization, credit = _mint_stage_pair()
    authorization_bytes = bytearray(Kagemusha.encode_mint_authorization(authorization))
    credit_bytes = bytearray(Kagemusha.encode_mint_credit(credit, authorization))
    command = Kagemusha.DeviceMintStageCommand(
        version=1,
        canonical_authorization=authorization_bytes,
        canonical_mint_credit=credit_bytes,
    )
    before = Kagemusha.encode_device_mint_stage_command_shape(command)
    authorization_bytes[:] = bytes(len(authorization_bytes))
    credit_bytes[:] = bytes(len(credit_bytes))
    assert Kagemusha.encode_device_mint_stage_command_shape(command) == before


def test_three_message_shapes_and_canonical_norito_alignments() -> None:
    request, payment, acknowledgement = _exchange()
    assert request.amount == 5
    assert request.recipient_encryption_key == _bytes(20)
    assert payment.__slots__ == ("version", "output", "encrypted_credit", "commit_certificate", "proof")
    context = Kagemusha.peer_credit_context(payment.output, request)
    assert _MODULE._model_alignment(Kagemusha.PeerCreditContext) == 16
    assert _MODULE._model_alignment(Kagemusha.Acknowledgement) == 2
    assert _MODULE._model_alignment(Kagemusha.EncryptedCreditEnvelope) == 8
    assert _MODULE._model_alignment(Kagemusha.Payment) == 16
    assert Kagemusha.encode_payment_request(request)[40:48] == bytes(8)
    assert Kagemusha.encode_peer_credit_context(context)[40:48] == bytes(8)
    assert Kagemusha.encode_peer_credit_context(context)[48] != 0
    assert Kagemusha.encode_acknowledgement(acknowledgement, request, payment)[40] != 0
    with pytest.raises(TypeError):
        _replace(request, request_mode=object())


def test_request_exact_amount_and_encryption_key_are_mandatory() -> None:
    request = _request(7)
    assert Kagemusha.decode_payment_request(Kagemusha.encode_payment_request(request)) == request
    with pytest.raises(Kagemusha.Error):
        _replace(request, amount=0)
    with pytest.raises(Kagemusha.Error):
        _replace(request, recipient_encryption_key=bytes(32))



@pytest.mark.parametrize("parity_proof_bytes", (8, 1_283, 2_495))
def test_three_message_ipm1_roundtrip_and_cap_sized_payment_proof(parity_proof_bytes: int) -> None:
    request, payment, acknowledgement = _exchange(parity_proof_bytes)
    messages = (
        ("request", request, ()),
        ("payment", payment, (request,)),
        ("acknowledgement", acknowledgement, (request, payment)),
    )
    assert dict(Kagemusha.ipm1_payload_kinds) == {
        "request": 1,
        "payment": 2,
        "acknowledgement": 3,
    }
    raw_bytes = text_bytes = 0
    for kind, value, bindings in messages:
        tag = Kagemusha.encode_ipm1_payload_kind(kind)
        raw = Kagemusha.encode_ipm1_payload(kind, value, *bindings)
        assert Kagemusha.decode_ipm1_payload(tag, raw, *bindings) == value
        text = Kagemusha.encode_typed_text(kind, value, *bindings)
        assert Kagemusha.decode_typed_text(kind, text, *bindings) == value
        raw_bytes += len(raw)
        text_bytes += len(text)
    assert Kagemusha.validate_complete_exchange(request, payment, acknowledgement) == raw_bytes
    assert raw_bytes <= 9_211 and text_bytes <= 12_288
    proof_raw = Kagemusha.encode_payment_proof(payment.proof)
    assert len(proof_raw) <= 6_528
    assert Kagemusha.decode_payment_proof(proof_raw) == payment.proof
    with pytest.raises(Kagemusha.Error):
        Kagemusha.encode_ipm1_payload_kind("authorization")


def test_fixed_semantic_transcripts_and_payment_body_preimage() -> None:
    request, payment, _ = _exchange()
    request_transcript = Kagemusha.payment_request_transcript(request)
    output_transcript = Kagemusha.payment_output_transcript(payment.output)
    assert (len(request_transcript), len(output_transcript)) == (390, 254)
    assert Kagemusha.payment_request_digest(request) == _digest(b"iroha:kagemusha:v1:payment-request", request_transcript)
    assert Kagemusha.payment_request_signing_bytes(request)[-326:] == request_transcript[:326]
    assert payment.proof.semantic_digest == _digest(
        b"iroha:kagemusha:v1:payment-body",
        Kagemusha.payment_output_digest(payment.output) + Kagemusha.ciphertext_digest(payment.encrypted_credit),
    )


def test_operations_are_the_six_unbounded_history_transitions() -> None:
    assert Kagemusha.operation_kinds == (
        "bootstrap", "mint_fold", "send_split", "receive_fold",
        "redeem_split", "rotate",
    )
    for name in (
        "receive_fold_batch_minimum_occupancy",
        "receive_fold_batch_maximum_occupancy",
        "validate_receive_fold_batch_occupancy",
    ):
        assert not hasattr(Kagemusha, name)



def test_public_payment_exposes_only_committed_state_heads() -> None:
    request, payment, _ = _exchange()
    assert payment.output.sender_before_commitment == _bytes(51)
    assert payment.output.sender_after_commitment == _bytes(52)
    assert payment.output.committed_at_ms == 150
    forbidden = {
        "predecessor",
        "successor",
        "recipient_lane_id",
        "hardware_transition_commitment",
    }
    for model in (payment.output, Kagemusha.PeerCreditContext, request):
        assert forbidden.isdisjoint(model.__slots__)
    assert Kagemusha.decode_payment(Kagemusha.encode_payment(payment, request), request) == payment
    voucher = _redemption_voucher(request)
    assert forbidden.isdisjoint(voucher.statement.__slots__)
    assert Kagemusha.decode_redemption_voucher(Kagemusha.encode_redemption_voucher(voucher)) == voucher


def test_request_key_binds_prepared_transfer_credit_id_and_aad() -> None:
    request, payment, _ = _exchange()
    second_request = _replace(
        request,
        request_id=_bytes(110),
        recipient_encryption_key=_bytes(111),
    )
    second_payment = _payment(second_request)
    assert payment.output.credit_id != second_payment.output.credit_id
    context = Kagemusha.peer_credit_context(payment.output, request)
    assert context.request_digest == payment.output.request_digest
    assert context.recipient_encryption_key == request.recipient_encryption_key
    assert context.prepared_transfer_digest == Kagemusha.prepared_transfer_digest(
        request,
        payment.output.sender_before_commitment,
        payment.output.sender_after_commitment,
        payment.output.transition_nullifier,
        payment.output.ciphertext_commitment,
    )
    other_context = Kagemusha.peer_credit_context(second_payment.output, second_request)
    assert context.prepared_transfer_digest != other_context.prepared_transfer_digest
    assert Kagemusha.decode_peer_credit_context(Kagemusha.encode_peer_credit_context(context)) == context
    assert Kagemusha.encrypted_credit_aad_for_peer(payment.output, request).context_digest != (
        Kagemusha.encrypted_credit_aad_for_peer(second_payment.output, second_request).context_digest)
    with pytest.raises(Kagemusha.Error):
        Kagemusha.encode_payment(payment, second_request)


def test_payment_rejects_output_ciphertext_certificate_and_proof_substitutions() -> None:
    request, payment, _ = _exchange()
    for field in (
        "request_digest",
        "sender_before_commitment",
        "sender_after_commitment",
        "transition_nullifier",
        "credit_id",
        "ciphertext_commitment",
    ):
        with pytest.raises(Kagemusha.Error):
            Kagemusha.encode_payment(
                _replace(payment, output=_replace(payment.output, **{field: _bytes(112)})),
                request,
            )
    for field in ("semantic_digest", "candidate_envelope_digest", "commit_certificate_digest"):
        with pytest.raises(Kagemusha.Error):
            Kagemusha.encode_payment(
                _replace(payment, proof=_replace(payment.proof, **{field: _bytes(113)})),
                request,
            )
    with pytest.raises(Kagemusha.Error):
        Kagemusha.encode_payment(_replace(payment, encrypted_credit=_encrypted_credit(120)), request)
    with pytest.raises(Kagemusha.Error):
        Kagemusha.encode_payment(_replace(payment, commit_certificate=_replace(
            payment.commit_certificate, hardware_terminal_commitment=_bytes(114))), request)
    rerandomized = _replace(payment, proof=_replace(payment.proof, eq_proof=_bytes(115, 17), ep_proof=_bytes(116, 17)))
    assert Kagemusha.payment_body_digest(rerandomized.output, rerandomized.encrypted_credit) == payment.proof.semantic_digest
    Kagemusha.encode_payment(rerandomized, request)


def test_request_bound_credit_id_and_peer_opening_preimages() -> None:
    transition, request_digest = _bytes(0x81), _bytes(0x82)
    assert Kagemusha.credit_id(transition, request_digest) == hashlib.sha256(
        b"iroha:kagemusha:v1:credit-id\0" + transition + request_digest).digest()
    opening = Kagemusha.peer_credit_opening_commitment(_bytes(0x84), _bytes(0x85), 5, _bytes(0x86), _bytes(0x87), _bytes(0x88))
    assert opening == hashlib.sha256(
        b"iroha:kagemusha:v1:peer-credit-opening-commitment\0" + (1).to_bytes(2, "little")
        + _bytes(0x84) + _bytes(0x85) + (5).to_bytes(16, "little")
        + _bytes(0x86) + _bytes(0x87) + _bytes(0x88)).digest()
    with pytest.raises(Kagemusha.Error):
        Kagemusha.peer_credit_opening_commitment(_bytes(0x84), _bytes(0x85), 0, _bytes(0x86), _bytes(0x87), _bytes(0x88))


def test_reservation_covers_post_commit_payment_and_certificate_policy_is_positive() -> None:
    _, payment, _ = _exchange()
    with pytest.raises(Kagemusha.Error, match="positive"):
        _replace(payment.commit_certificate, policy_epoch=0)
    fields = {"reservation_id": _bytes(90), "operation_kind": "send_split", "issued_at_ms": 1, "expires_at_ms": 2}
    Kagemusha.OutboxReservation(**fields, reserved_outbox_bytes=25_728)
    with pytest.raises(Kagemusha.Error):
        Kagemusha.OutboxReservation(**fields, reserved_outbox_bytes=25_727)


def test_payer_top_up_instruction_round_trips_unchanged_canonical_box() -> None:
    request = _top_up_request()
    instruction = Kagemusha.build_top_up_instruction(request)
    archive = Kagemusha.encode_top_up_instruction(instruction)
    assert Kagemusha.maximum_top_up_request_bytes == 16 * 1024
    assert Kagemusha.top_up_instruction_wire_id == "iroha.kagemusha.v1.top_up"
    assert Kagemusha.decode_top_up_instruction(archive) == instruction
    assert hashlib.sha256(archive).hexdigest() == (
        "97b5c65bb272242ddd90084d8ba95cc399cc1dfa92d91a8c46a1433c1769d086"
    )
    maximum = _top_up_request(2_495)
    assert len(Kagemusha.encode_top_up_request(maximum)) <= 16 * 1024
