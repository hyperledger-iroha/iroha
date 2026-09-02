"""Focused source-only tests for the canonical Offline Cash V1 Python surface."""

from __future__ import annotations

import base64
import hashlib
import importlib
import json
import sys
import types
from pathlib import Path

import pytest

# Exercise the pure Python codec independently of a previously built local
# extension. CI separately builds and tests the native extension itself.
_PACKAGE_ROOT = Path(__file__).resolve().parents[1] / "src" / "iroha_python"
_PURE_PACKAGE = "_iroha_python_offline_cash_source_test"
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

_MODULE = importlib.import_module(f"{_PURE_PACKAGE}.offline_cash_v1")
OfflineCashV1 = _MODULE.OfflineCashV1
NetworkId = sys.modules[f"{_PURE_PACKAGE}.crypto"].NetworkId

_FIXTURE = json.loads(
    (Path(__file__).resolve().parents[3] / "fixtures/offline/offline_cash_v1.json").read_text()
)
_CANONICAL_FIXTURE_KEYS = (
    "payment_request",
    "acceptance_intent_authorization",
    "acceptance_ticket",
    "no_commit_closure",
    "payment",
    "acknowledgement",
    "mint_authorization",
    "mint_credit",
    "redemption_voucher",
    "encrypted_credit_envelope",
    "encrypted_credit_aad",
    "credit_opening",
    "pre_ticket_exchange",
    "terminal_trio",
    "complete_five_message",
)
_HAS_CANONICAL_FIXTURE = all(key in _FIXTURE for key in _CANONICAL_FIXTURE_KEYS)
_FIXTURE_TRANSPORT_KINDS = {
    "payment_request": "payment_request",
    "acceptance_intent_authorization": "acceptance_intent_authorization",
    "acceptance_ticket": "acceptance_ticket",
    "payment": "payment",
    "acknowledgement": "acknowledgement",
    "mint_authorization": "mint_authorization",
    "mint_credit": "mint_credit",
    "redemption_voucher": "redemption_voucher",
}
_FIXTURE_SUMMARIES = {
    "pre_ticket_exchange": (8960, 9984, 13326),
    "terminal_trio": (8960, 9211, 12288),
    "complete_five_message": (16384, 18171, 24244),
}

_PUBLIC_KEY = bytes.fromhex(
    "04"
    "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
)


def _bytes(value: int, length: int = 32) -> bytes:
    return bytes((value,)) * length


def _semantic_digest(domain: str, transcript: bytes) -> bytes:
    return hashlib.sha256(
        domain.encode("ascii")
        + b"\0"
        + len(transcript).to_bytes(8, "little")
        + transcript
    ).digest()


def _intent_semantic_transcript(intent: object) -> bytes:
    return b"".join(
        (
            intent.version.to_bytes(2, "little"),
            intent.request_digest,
            intent.intent_id,
            intent.exact_amount.to_bytes(16, "little"),
            intent.sender_one_time_commitment,
        )
    )


def _authorization_statement_semantic_transcript(statement: object) -> bytes:
    return b"".join(
        (
            statement.version.to_bytes(2, "little"),
            _intent_semantic_transcript(statement.intent),
            statement.release_id,
            statement.suite_id,
            statement.vk_digest,
            statement.artifact_manifest_digest,
        )
    )


def _no_commit_statement_semantic_transcript(statement: object) -> bytes:
    return b"".join(
        (
            statement.version.to_bytes(2, "little"),
            statement.release_id,
            statement.suite_id,
            statement.vk_digest,
            statement.artifact_manifest_digest,
            statement.sender_hardware_binding_commitment,
            statement.request_id,
            statement.request_digest,
            statement.acceptance_ticket_id,
            statement.ticket_digest,
            statement.intent_authorization_digest,
            statement.intent_digest,
            statement.exact_amount.to_bytes(16, "little"),
            statement.sender_one_time_commitment,
            statement.recovery_id,
            statement.cancellation_nullifier,
            statement.equivalent_delivery_slot_commitment,
        )
    )


def _outbox_reservation_semantic_transcript(reservation: object) -> bytes:
    return b"".join(
        (
            reservation.reservation_id,
            (2 if reservation.operation_kind == "send_split" else 4).to_bytes(
                4, "little"
            ),
            reservation.reserved_outbox_bytes.to_bytes(4, "little"),
            reservation.issued_at_ms.to_bytes(8, "little"),
            reservation.expires_at_ms.to_bytes(8, "little"),
        )
    )


def _commit_evidence_semantic_transcript(evidence: object) -> bytes:
    if evidence.source == "trusted_time":
        tag = 0
        commitment = evidence.evidence.time_evidence_commitment
    else:
        tag = 1
        commitment = evidence.evidence.lease_evidence_commitment
    return tag.to_bytes(4, "little") + commitment


def _commit_certificate_semantic_transcript(
    certificate: object, *, include_id: bool
) -> bytes:
    values = [certificate.version.to_bytes(2, "little")]
    if include_id:
        values.append(certificate.certificate_id)
    values.extend(
        (
            certificate.candidate_envelope_digest,
            certificate.lifecycle_binding_digest,
            certificate.transition_nullifier,
            certificate.outbox_reservation_commitment,
            _commit_evidence_semantic_transcript(certificate.commit_evidence),
            certificate.hardware_profile_id,
            certificate.policy_epoch.to_bytes(8, "little"),
            certificate.hardware_terminal_commitment,
        )
    )
    return b"".join(values)


def _signature() -> bytes:
    return bytes(31) + b"\x01" + bytes(31) + b"\x01"


def _base_context() -> dict[str, object]:
    network_id = NetworkId.from_bytes(bytes(range(1, 32)) + b"\x01")
    asset = OfflineCashV1.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    incarnation = OfflineCashV1.AssetIncarnation(_bytes(1))
    account = OfflineCashV1.AccountId(
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    )
    public_key = OfflineCashV1.DevicePublicKey(_PUBLIC_KEY)
    signature = OfflineCashV1.DeviceSignature(_signature())
    credential = OfflineCashV1.HardwareCredential(
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
        device_key_reference=OfflineCashV1.device_key_reference(public_key),
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
    return OfflineCashV1.PaymentRequest(
        version=1,
        release_id=_bytes(8),
        network_id=context["network_id"],
        asset=context["asset"],
        asset_incarnation=context["incarnation"],
        scale=2,
        liability_pool_id=OfflineCashV1.liability_pool_id(
            context["network_id"], context["asset"], context["incarnation"]
        ),
        recipient=context["account"],
        amount=amount,
        hardware_credential=context["credential"],
        request_id=_bytes(9),
        issued_at_ms=100,
        expires_at_ms=200,
        signature=context["signature"],
    )


def _proof(semantic_digest: bytes, proof_length: int = 8) -> object:
    return OfflineCashV1.PairedProof(
        version=1,
        eq_protocol_digest=_bytes(10),
        ep_protocol_digest=_bytes(11),
        semantic_digest=semantic_digest,
        guard_eq_credential_audit=_bytes(12),
        guard_ep_credential_audit=_bytes(13),
        eq_deferred_audit=_bytes(14),
        ep_deferred_audit=_bytes(15),
        eq_proof=_bytes(16, proof_length),
        ep_proof=_bytes(17, proof_length),
        eq_history=_bytes(18, 544),
        ep_history=_bytes(19, 544),
    )


def _pre_ticket(
    request: object, exact_amount: int = 5, ticket_seed: int = 24
) -> tuple[object, object, object]:
    intent = OfflineCashV1.AcceptanceIntent(
        version=1,
        request_digest=OfflineCashV1.payment_request_digest(request),
        intent_id=_bytes(20),
        exact_amount=exact_amount,
        sender_one_time_commitment=_bytes(21),
    )
    statement = OfflineCashV1.AcceptanceIntentAuthorizationStatement(
        version=1,
        intent=intent,
        release_id=request.release_id,
        suite_id=request.hardware_credential.suite_id,
        vk_digest=_bytes(22),
        artifact_manifest_digest=_bytes(23),
    )
    authorization = OfflineCashV1.AcceptanceIntentAuthorization(
        version=1,
        statement=statement,
        proof=_proof(OfflineCashV1.acceptance_authorization_statement_digest(statement)),
    )
    ticket = OfflineCashV1.AcceptanceTicket(
        version=1,
        network_id=request.network_id,
        request_id=request.request_id,
        request_digest=OfflineCashV1.payment_request_digest(request),
        acceptance_ticket_id=_bytes(ticket_seed),
        asset=request.asset,
        asset_incarnation=request.asset_incarnation,
        scale=request.scale,
        intent_digest=OfflineCashV1.acceptance_intent_digest(intent),
        exact_amount=exact_amount,
        reserved_inbox_bytes=8960,
        recipient_one_time_key=_bytes(25),
        hardware_profile_id=request.hardware_credential.hardware_profile_id,
        policy_epoch=request.hardware_credential.policy_epoch,
        issued_at_ms=110,
        expires_at_ms=190,
        signature=request.signature,
    )
    return intent, authorization, ticket


def _no_commit_closure_statement(
    request: object, authorization: object, ticket: object
) -> object:
    intent = authorization.statement.intent
    return OfflineCashV1.NoCommitClosureStatement(
        version=1,
        release_id=authorization.statement.release_id,
        suite_id=authorization.statement.suite_id,
        vk_digest=authorization.statement.vk_digest,
        artifact_manifest_digest=authorization.statement.artifact_manifest_digest,
        sender_hardware_binding_commitment=_bytes(26),
        request_id=request.request_id,
        request_digest=OfflineCashV1.payment_request_digest(request),
        acceptance_ticket_id=ticket.acceptance_ticket_id,
        ticket_digest=OfflineCashV1.acceptance_ticket_digest(ticket),
        intent_authorization_digest=OfflineCashV1.acceptance_authorization_digest(
            authorization
        ),
        intent_digest=OfflineCashV1.acceptance_intent_digest(intent),
        exact_amount=intent.exact_amount,
        sender_one_time_commitment=intent.sender_one_time_commitment,
        recovery_id=_bytes(27),
        cancellation_nullifier=_bytes(28),
        equivalent_delivery_slot_commitment=_bytes(29),
    )


def _no_commit_closure(
    request: object, authorization: object, ticket: object, proof_length: int = 8
) -> object:
    statement = _no_commit_closure_statement(request, authorization, ticket)
    return OfflineCashV1.NoCommitClosure(
        version=1,
        statement=statement,
        request=request,
        intent_authorization=authorization,
        acceptance_ticket=ticket,
        proof=_proof(
            OfflineCashV1.no_commit_closure_statement_digest(statement), proof_length
        ),
    )


def _envelope(seed: int = 36) -> tuple[object, bytes]:
    value = OfflineCashV1.EncryptedCreditEnvelope(
        version=1,
        ephemeral_x25519_public_key=_bytes(seed),
        nonce=_bytes(seed + 1, 24),
        ciphertext_and_tag=_bytes(seed + 2, 216),
    )
    return value, OfflineCashV1.encode_encrypted_credit_envelope(value)


def test_strict_oc1_transport_enforces_current_caps() -> None:
    raw = bytes.fromhex("fbff0001")
    assert OfflineCashV1.encode_text("payment_request", raw) == "oc1:-_8AAQ"
    assert OfflineCashV1.decode_text("payment_request", "oc1:-_8AAQ") == raw
    assert OfflineCashV1.maximum_request_raw_bytes == 1024
    assert OfflineCashV1.maximum_request_text_bytes == 1370
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.encode_text("payment_request", _bytes(1, 1025))
    for invalid in (
        "OC1:-_8AAQ", "oc1:", "oc1:-_8AAQ==", "oc1:-_8A AQ", "oc1:+_8AAQ", "oc1:A"
    ):
        with pytest.raises(OfflineCashV1.Error):
            OfflineCashV1.decode_text("payment_request", invalid)


def test_exact_positive_request_amount_round_trips() -> None:
    request = _request(5)
    raw = OfflineCashV1.encode_payment_request(request)
    assert len(raw) <= 1024
    decoded = OfflineCashV1.decode_payment_request(raw)
    assert decoded.amount == 5
    assert OfflineCashV1.encode_payment_request(decoded) == raw
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.decode_payment_request(_bytes(1, 1025))
    canonical = OfflineCashV1.encode_payment_request(_request())
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.decode_payment_request(canonical + b"\0")


def test_proof_authorization_and_one_use_ticket_form_pre_ticket_exchange() -> None:
    request = _request()
    intent, authorization, ticket = _pre_ticket(request)
    authorization_raw = OfflineCashV1.encode_acceptance_intent_authorization(
        authorization, request
    )
    ticket_raw = OfflineCashV1.encode_acceptance_ticket(ticket, request, intent)
    assert OfflineCashV1.encode_acceptance_intent_authorization(
        OfflineCashV1.decode_acceptance_intent_authorization(authorization_raw, request),
        request,
    ) == authorization_raw
    assert OfflineCashV1.encode_acceptance_ticket(
        OfflineCashV1.decode_acceptance_ticket(ticket_raw, request, intent), request, intent
    ) == ticket_raw
    assert OfflineCashV1.validate_pre_ticket_exchange(request, authorization, ticket) <= 9984
    assert OfflineCashV1.maximum_pre_ticket_text_bytes == 13326


def test_no_commit_recovery_closure_is_canonical_cross_bound_and_bounded() -> None:
    request = _request()
    _intent, authorization, ticket = _pre_ticket(request)
    closure = _no_commit_closure(request, authorization, ticket)
    raw = OfflineCashV1.encode_no_commit_closure(closure)
    assert OfflineCashV1.maximum_no_commit_closure_bytes == 16384
    assert len(raw) <= OfflineCashV1.maximum_no_commit_closure_bytes
    decoded = OfflineCashV1.decode_no_commit_closure(raw)
    assert OfflineCashV1.encode_no_commit_closure(decoded) == raw
    assert len(OfflineCashV1.no_commit_closure_digest(decoded)) == 32
    assert not hasattr(decoded.statement, "predecessor_state")
    assert not hasattr(decoded.statement, "successor_state")

    _unused, _unused_authorization, substituted_ticket = _pre_ticket(
        request, ticket_seed=30
    )
    substituted = OfflineCashV1.NoCommitClosure(
        version=1,
        statement=closure.statement,
        request=request,
        intent_authorization=authorization,
        acceptance_ticket=substituted_ticket,
        proof=closure.proof,
    )
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.encode_no_commit_closure(substituted)
    substituted_authorization_statement = OfflineCashV1.AcceptanceIntentAuthorizationStatement(
        version=1,
        intent=authorization.statement.intent,
        release_id=authorization.statement.release_id,
        suite_id=authorization.statement.suite_id,
        vk_digest=_bytes(31),
        artifact_manifest_digest=authorization.statement.artifact_manifest_digest,
    )
    substituted_authorization = OfflineCashV1.AcceptanceIntentAuthorization(
        version=1,
        statement=substituted_authorization_statement,
        proof=_proof(
            OfflineCashV1.acceptance_authorization_statement_digest(
                substituted_authorization_statement
            )
        ),
    )
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.encode_no_commit_closure(
            OfflineCashV1.NoCommitClosure(
                version=1,
                statement=closure.statement,
                request=request,
                intent_authorization=substituted_authorization,
                acceptance_ticket=ticket,
                proof=closure.proof,
            )
        )
    with pytest.raises(TypeError):
        OfflineCashV1.NoCommitClosure(
            version=1,
            statement=closure.statement,
            request=request,
            intent_authorization=authorization,
            acceptance_ticket=ticket,
            proof=closure.proof,
            predecessor_state=_bytes(32),
        )
    with pytest.raises(OfflineCashV1.Error, match="length|16384"):
        OfflineCashV1.decode_no_commit_closure(_bytes(1, 16385))


def test_circuit_bound_semantic_hashes_use_exact_fixed_transcripts() -> None:
    request = _request()
    intent, authorization, ticket = _pre_ticket(request)
    closure = _no_commit_closure(request, authorization, ticket)
    intent_transcript = _intent_semantic_transcript(intent)
    authorization_transcript = _authorization_statement_semantic_transcript(
        authorization.statement
    )
    closure_transcript = _no_commit_statement_semantic_transcript(closure.statement)
    reservation = OfflineCashV1.OutboxReservation(
        reservation_id=_bytes(42),
        operation_kind="send_split",
        reserved_outbox_bytes=OfflineCashV1.payment_outbox_minimum_bytes,
        issued_at_ms=100,
        expires_at_ms=200,
    )
    reservation_transcript = _outbox_reservation_semantic_transcript(reservation)

    assert len(intent_transcript) == 114
    assert len(authorization_transcript) == 244
    assert len(closure_transcript) == 498
    assert len(reservation_transcript) == 56
    assert OfflineCashV1.acceptance_intent_digest(intent) == _semantic_digest(
        "iroha:offline-cash:v1:acceptance-intent", intent_transcript
    )
    assert OfflineCashV1.acceptance_authorization_statement_digest(
        authorization.statement
    ) == _semantic_digest(
        "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
        authorization_transcript,
    )
    assert OfflineCashV1.no_commit_closure_statement_digest(
        closure.statement
    ) == _semantic_digest(
        "iroha:offline-cash:v1:no-commit-closure-statement", closure_transcript
    )
    assert OfflineCashV1.outbox_reservation_commitment(reservation) == _semantic_digest(
        "iroha:offline-cash:v1:outbox-reservation", reservation_transcript
    )
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.OutboxReservation(
            reservation_id=_bytes(42),
            operation_kind="send_split",
            reserved_outbox_bytes=OfflineCashV1.payment_outbox_minimum_bytes - 1,
            issued_at_ms=100,
            expires_at_ms=200,
        )
    assert intent_transcript != OfflineCashV1.encode_acceptance_intent(intent)


def test_typed_credit_opening_aad_and_envelope_are_exact_and_bounded() -> None:
    opening = OfflineCashV1.CreditOpening(
        version=1,
        credit_id=_bytes(30),
        amount=7,
        credit_commitment_opening=_bytes(31),
        recipient_binding_opening=_bytes(32),
        recovery_nonce=_bytes(33),
    )
    opening_raw = OfflineCashV1.encode_credit_opening(opening)
    assert len(opening_raw) == 200
    assert OfflineCashV1.decode_credit_opening(opening_raw, _bytes(30), 7).amount == 7
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.decode_credit_opening(opening_raw, _bytes(29), 7)

    aad = OfflineCashV1.EncryptedCreditAad(
        version=1,
        purpose="peer",
        context_digest=_bytes(34),
        issuance_or_transition_commitment=_bytes(35),
        credit_id=_bytes(30),
        amount=7,
    )
    aad_raw = OfflineCashV1.encode_encrypted_credit_aad(aad)
    assert OfflineCashV1.encode_encrypted_credit_aad(
        OfflineCashV1.decode_encrypted_credit_aad(aad_raw)
    ) == aad_raw

    envelope, envelope_raw = _envelope()
    assert len(envelope_raw) <= 384
    assert OfflineCashV1.encode_encrypted_credit_envelope(
        OfflineCashV1.decode_encrypted_credit_envelope(envelope_raw)
    ) == envelope_raw
    assert envelope.ciphertext_and_tag == _bytes(38, 216)


def test_mint_authorization_and_credit_have_complete_reciprocal_binding() -> None:
    context_values = _base_context()
    _envelope_value, encrypted_credit = _envelope(40)
    ciphertext = OfflineCashV1.ciphertext_digest(encrypted_credit)
    context = OfflineCashV1.MintAuthorizationContext(
        version=1,
        operation_id=_bytes(41),
        release_id=_bytes(42),
        suite_id=_bytes(43),
        vk_digest=_bytes(44),
        artifact_manifest_digest=_bytes(45),
        network_id=context_values["network_id"],
        asset=context_values["asset"],
        asset_incarnation=context_values["incarnation"],
        scale=2,
        liability_pool_id=OfflineCashV1.liability_pool_id(
            context_values["network_id"],
            context_values["asset"],
            context_values["incarnation"],
        ),
        amount=7,
        payer=context_values["account"],
        recipient=context_values["account"],
        hardware_credential_id=context_values["credential"].credential_id,
        hardware_profile_id=context_values["credential"].hardware_profile_id,
        policy_epoch=1,
        recipient_credential_commitment=_bytes(46),
        credit_commitment=_bytes(47),
        recipient_one_time_key=_bytes(48),
    )
    authorization_statement = OfflineCashV1.MintAuthorizationStatement(
        version=1,
        context=context,
        issuance_commitment=_bytes(49),
        credit_id=_bytes(50),
        ciphertext_digest=ciphertext,
    )
    authorization = OfflineCashV1.MintAuthorization(
        version=1,
        statement=authorization_statement,
        proof=_proof(
            OfflineCashV1.mint_authorization_statement_digest(authorization_statement)
        ),
    )
    lifecycle = OfflineCashV1.LifecycleBinding(
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
        acceptance_ticket_id=bytes(32),
        credit_id=authorization_statement.credit_id,
        ciphertext_digest=ciphertext,
    )
    statement = OfflineCashV1.MintCreditStatement(
        version=1,
        lifecycle=lifecycle,
        recipient_credential_commitment=context.recipient_credential_commitment,
        authorization_context_digest=OfflineCashV1.mint_authorization_context_digest(context),
        mint_authorization_digest=OfflineCashV1.mint_authorization_digest(authorization),
        amount=context.amount,
        issuance_commitment=authorization_statement.issuance_commitment,
        recipient=context.recipient,
        credit_commitment=context.credit_commitment,
        minted_at_ms=1,
    )
    credit = OfflineCashV1.MintCredit(
        version=1,
        statement=statement,
        proof=_proof(_MODULE._digest_model(_MODULE._DOMAIN["mint_statement"], _MODULE._SCHEMA_MINT_STATEMENT, statement)),
        finality_certificate_binding=_bytes(51),
        finality_authority_head=_bytes(52),
        finality_genesis_roster_id=_bytes(53),
        finality_proof_binding_digest=_bytes(54),
        encrypted_credit=encrypted_credit,
        artifact_manifest_digest=context.artifact_manifest_digest,
    )
    authorization_raw = OfflineCashV1.encode_mint_authorization(authorization)
    assert OfflineCashV1.encode_mint_authorization(
        OfflineCashV1.decode_mint_authorization(authorization_raw)
    ) == authorization_raw
    credit_raw = OfflineCashV1.encode_mint_credit(credit, authorization)
    decoded = OfflineCashV1.decode_mint_credit(credit_raw, authorization)
    assert OfflineCashV1.validate_mint_credit_against_authorization(decoded, authorization)

    mismatched = OfflineCashV1.MintAuthorizationStatement(
        version=1,
        context=context,
        issuance_commitment=_bytes(55),
        credit_id=authorization_statement.credit_id,
        ciphertext_digest=ciphertext,
    )
    with pytest.raises(OfflineCashV1.Error):
        OfflineCashV1.validate_mint_credit_against_authorization(
            credit,
            OfflineCashV1.MintAuthorization(
                version=1,
                statement=mismatched,
                proof=_proof(OfflineCashV1.mint_authorization_statement_digest(mismatched)),
            ),
        )


def test_retired_links_and_software_money_crypto_are_absent() -> None:
    with pytest.raises(TypeError):
        OfflineCashV1.PairedProof(
            version=1,
            eq_protocol_digest=_bytes(1),
            ep_protocol_digest=_bytes(2),
            semantic_digest=_bytes(3),
            guard_eq_credential_audit=_bytes(4),
            guard_ep_credential_audit=_bytes(5),
            eq_deferred_audit=_bytes(6),
            ep_deferred_audit=_bytes(7),
            predecessor_state=OfflineCashV1.PastaStateCommitment(eq=_bytes(8), ep=_bytes(9)),
            successor_state=OfflineCashV1.PastaStateCommitment(eq=_bytes(10), ep=_bytes(11)),
            eq_proof=_bytes(12, 8),
            ep_proof=_bytes(13, 8),
            eq_history=_bytes(14, 544),
            ep_history=_bytes(15, 544),
        )
    for name in (
        "encrypt_credit", "decrypt_credit", "prove_payment", "sign_payment",
        "software_fallback", "drain_staged_credits",
    ):
        assert not hasattr(OfflineCashV1, name)
    assert OfflineCashV1.complete_exchange_target_bytes == 16384
    assert OfflineCashV1.maximum_complete_exchange_raw_bytes == 18171
    assert OfflineCashV1.maximum_session_raw_bytes == 9211
    assert OfflineCashV1.maximum_session_text_bytes == 12288


def test_native_generated_v1_fixture_round_trips_all_transported_values() -> None:
    assert _FIXTURE.get("fixture_version") == 1
    assert _HAS_CANONICAL_FIXTURE, "fixture_version 1 must contain the complete canonical key set"
    def raw(name: str) -> bytes:
        entry = _FIXTURE[name]
        assert entry["raw_bytes"] == len(bytes.fromhex(entry["norito_hex"]))
        return bytes.fromhex(entry["norito_hex"])

    for name, kind in _FIXTURE_TRANSPORT_KINDS.items():
        assert set(_FIXTURE[name]) == {"norito_hex", "oc1", "raw_bytes"}
        assert OfflineCashV1.encode_text(kind, raw(name)) == _FIXTURE[name]["oc1"]
    assert set(_FIXTURE["encrypted_credit_envelope"]) == {
        "norito_hex",
        "raw_bytes",
        "recipient_x25519_public_key_hex",
    }
    for name in ("encrypted_credit_aad", "credit_opening"):
        assert set(_FIXTURE[name]) == {"norito_hex", "raw_bytes"}
    for name, (target, raw_cap, text_cap) in _FIXTURE_SUMMARIES.items():
        summary = _FIXTURE[name]
        assert set(summary) == {
            "raw_bytes",
            "text_bytes",
            "raw_target_bytes",
            "raw_hard_cap_bytes",
            "text_hard_cap_bytes",
            "within_raw_target",
            "within_raw_hard_cap",
            "within_text_hard_cap",
        }
        assert summary["raw_target_bytes"] == target
        assert summary["raw_hard_cap_bytes"] == raw_cap
        assert summary["text_hard_cap_bytes"] == text_cap
        assert summary["within_raw_target"] == (summary["raw_bytes"] <= target)
        assert summary["within_raw_hard_cap"] == (summary["raw_bytes"] <= raw_cap)
        assert summary["within_text_hard_cap"] == (summary["text_bytes"] <= text_cap)

    request = OfflineCashV1.decode_payment_request(raw("payment_request"))
    authorization = OfflineCashV1.decode_acceptance_intent_authorization(
        raw("acceptance_intent_authorization"), request
    )
    ticket = OfflineCashV1.decode_acceptance_ticket(
        raw("acceptance_ticket"), request, authorization.statement.intent
    )
    intent_transcript = _intent_semantic_transcript(authorization.statement.intent)
    authorization_transcript = _authorization_statement_semantic_transcript(
        authorization.statement
    )
    assert len(intent_transcript) == 114
    assert len(authorization_transcript) == 244
    assert ticket.intent_digest == _semantic_digest(
        "iroha:offline-cash:v1:acceptance-intent", intent_transcript
    )
    assert authorization.proof.semantic_digest == _semantic_digest(
        "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
        authorization_transcript,
    )
    assert set(_FIXTURE["no_commit_closure"]) == {"norito_hex", "oc1", "raw_bytes"}
    expected_closure_text = "oc1:" + base64.urlsafe_b64encode(
        raw("no_commit_closure")
    ).decode("ascii").rstrip("=")
    assert _FIXTURE["no_commit_closure"]["oc1"] == expected_closure_text
    decoded_closure = OfflineCashV1.decode_no_commit_closure(raw("no_commit_closure"))
    assert OfflineCashV1.encode_no_commit_closure(decoded_closure) == raw(
        "no_commit_closure"
    )
    closure_transcript = _no_commit_statement_semantic_transcript(
        decoded_closure.statement
    )
    assert len(closure_transcript) == 498
    assert decoded_closure.proof.semantic_digest == _semantic_digest(
        "iroha:offline-cash:v1:no-commit-closure-statement", closure_transcript
    )
    payment = OfflineCashV1.decode_payment(raw("payment"), request)
    certificate_id_transcript = _commit_certificate_semantic_transcript(
        payment.commit_certificate, include_id=False
    )
    certificate_transcript = _commit_certificate_semantic_transcript(
        payment.commit_certificate, include_id=True
    )
    assert len(_commit_evidence_semantic_transcript(payment.commit_certificate.commit_evidence)) == 36
    assert len(certificate_id_transcript) == 238
    assert len(certificate_transcript) == 270
    assert payment.commit_certificate.certificate_id == _semantic_digest(
        "iroha:offline-cash:v1:commit-certificate-id", certificate_id_transcript
    )
    assert payment.proof.commit_certificate_digest == _semantic_digest(
        "iroha:offline-cash:v1:commit-certificate", certificate_transcript
    )
    acknowledgement = OfflineCashV1.decode_acknowledgement(
        raw("acknowledgement"), request, payment
    )
    mint_authorization = OfflineCashV1.decode_mint_authorization(raw("mint_authorization"))
    mint_credit = OfflineCashV1.decode_mint_credit(raw("mint_credit"), mint_authorization)
    redemption_voucher = OfflineCashV1.decode_redemption_voucher(
        raw("redemption_voucher")
    )
    assert OfflineCashV1.encode_redemption_voucher(
        redemption_voucher
    ) == raw("redemption_voucher")
    assert OfflineCashV1.encode_encrypted_credit_envelope(
        OfflineCashV1.decode_encrypted_credit_envelope(
            raw("encrypted_credit_envelope"),
            bytes.fromhex(
                _FIXTURE["encrypted_credit_envelope"]["recipient_x25519_public_key_hex"]
            ),
        )
    ) == raw("encrypted_credit_envelope")
    assert OfflineCashV1.encode_encrypted_credit_aad(
        OfflineCashV1.decode_encrypted_credit_aad(raw("encrypted_credit_aad"))
    ) == raw("encrypted_credit_aad")
    assert OfflineCashV1.encode_credit_opening(
        OfflineCashV1.decode_credit_opening(raw("credit_opening"))
    ) == raw("credit_opening")
    assert OfflineCashV1.validate_pre_ticket_exchange(
        request, authorization, ticket
    ) == _FIXTURE["pre_ticket_exchange"]["raw_bytes"]
    assert OfflineCashV1.validate_session(
        request, payment, acknowledgement
    ) == _FIXTURE["terminal_trio"]["raw_bytes"]
    assert OfflineCashV1.validate_complete_exchange(
        request, authorization, ticket, payment, acknowledgement
    ) == _FIXTURE["complete_five_message"]["raw_bytes"]
    assert OfflineCashV1.validate_mint_credit_against_authorization(
        mint_credit, mint_authorization
    )

    certificate = payment.commit_certificate
    mutated_lifecycle_digest = bytearray(certificate.lifecycle_binding_digest)
    mutated_lifecycle_digest[0] ^= 1
    substituted_certificate = OfflineCashV1.CommitCertificate(
        version=certificate.version,
        certificate_id=certificate.certificate_id,
        candidate_envelope_digest=certificate.candidate_envelope_digest,
        lifecycle_binding_digest=bytes(mutated_lifecycle_digest),
        transition_nullifier=certificate.transition_nullifier,
        outbox_reservation_commitment=certificate.outbox_reservation_commitment,
        commit_evidence=certificate.commit_evidence,
        hardware_profile_id=certificate.hardware_profile_id,
        policy_epoch=certificate.policy_epoch,
        hardware_terminal_commitment=certificate.hardware_terminal_commitment,
    )
    substituted_payment = OfflineCashV1.Payment(
        version=payment.version,
        statement=payment.statement,
        acceptance_intent=payment.acceptance_intent,
        acceptance_ticket=payment.acceptance_ticket,
        commit_certificate=substituted_certificate,
        proof=payment.proof,
        encrypted_credit=payment.encrypted_credit,
        artifact_manifest_digest=payment.artifact_manifest_digest,
    )
    with pytest.raises(OfflineCashV1.Error, match="commit certificate lifecycle digest"):
        OfflineCashV1.encode_payment(substituted_payment, request)

    mutated_certificate_id = bytearray(certificate.certificate_id)
    mutated_certificate_id[0] ^= 1
    certificate_id_substitution = OfflineCashV1.CommitCertificate(
        version=certificate.version,
        certificate_id=bytes(mutated_certificate_id),
        candidate_envelope_digest=certificate.candidate_envelope_digest,
        lifecycle_binding_digest=certificate.lifecycle_binding_digest,
        transition_nullifier=certificate.transition_nullifier,
        outbox_reservation_commitment=certificate.outbox_reservation_commitment,
        commit_evidence=certificate.commit_evidence,
        hardware_profile_id=certificate.hardware_profile_id,
        policy_epoch=certificate.policy_epoch,
        hardware_terminal_commitment=certificate.hardware_terminal_commitment,
    )
    with pytest.raises(OfflineCashV1.Error, match="commit certificate ID"):
        OfflineCashV1.encode_payment(
            OfflineCashV1.Payment(
                version=payment.version,
                statement=payment.statement,
                acceptance_intent=payment.acceptance_intent,
                acceptance_ticket=payment.acceptance_ticket,
                commit_certificate=certificate_id_substitution,
                proof=payment.proof,
                encrypted_credit=payment.encrypted_credit,
                artifact_manifest_digest=payment.artifact_manifest_digest,
            ),
            request,
        )

    mutated_certificate_digest = bytearray(payment.proof.commit_certificate_digest)
    mutated_certificate_digest[0] ^= 1
    proof_digest_substitution = OfflineCashV1.CommitWrapperProof(
        version=payment.proof.version,
        eq_protocol_digest=payment.proof.eq_protocol_digest,
        ep_protocol_digest=payment.proof.ep_protocol_digest,
        semantic_digest=payment.proof.semantic_digest,
        candidate_envelope_digest=payment.proof.candidate_envelope_digest,
        commit_certificate_digest=bytes(mutated_certificate_digest),
        eq_deferred_audit=payment.proof.eq_deferred_audit,
        ep_deferred_audit=payment.proof.ep_deferred_audit,
        eq_proof=payment.proof.eq_proof,
        ep_proof=payment.proof.ep_proof,
        eq_history=payment.proof.eq_history,
        ep_history=payment.proof.ep_history,
    )
    with pytest.raises(OfflineCashV1.Error, match="commit wrapper certificate digest"):
        OfflineCashV1.encode_payment(
            OfflineCashV1.Payment(
                version=payment.version,
                statement=payment.statement,
                acceptance_intent=payment.acceptance_intent,
                acceptance_ticket=payment.acceptance_ticket,
                commit_certificate=payment.commit_certificate,
                proof=proof_digest_substitution,
                encrypted_credit=payment.encrypted_credit,
                artifact_manifest_digest=payment.artifact_manifest_digest,
            ),
            request,
        )

    statement = redemption_voucher.statement
    mutated_redemption_id = bytearray(statement.redemption_id)
    mutated_redemption_id[0] ^= 1
    substituted_statement = OfflineCashV1.RedemptionStatement(
        version=statement.version,
        lifecycle=statement.lifecycle,
        amount=statement.amount,
        beneficiary=statement.beneficiary,
        terminal_nullifier=statement.terminal_nullifier,
        redemption_commitment=statement.redemption_commitment,
        redemption_id=bytes(mutated_redemption_id),
        commit_evidence=statement.commit_evidence,
    )
    substituted_voucher = OfflineCashV1.RedemptionVoucher(
        version=redemption_voucher.version,
        statement=substituted_statement,
        commit_certificate=redemption_voucher.commit_certificate,
        proof=redemption_voucher.proof,
        artifact_manifest_digest=redemption_voucher.artifact_manifest_digest,
    )
    with pytest.raises(OfflineCashV1.Error, match="redemption ID"):
        OfflineCashV1.encode_redemption_voucher(substituted_voucher)
