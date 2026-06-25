from __future__ import annotations

from pathlib import Path

import pytest

from iroha_python import (
    SORAFS_ORDERBOOK_PAYLOAD_KINDS,
    SORAFS_PDP_PAYLOAD_KINDS,
    build_signed_orderbook_order_cancel,
    build_signed_orderbook_order_request,
    build_signed_orderbook_settlement_receipt,
    sign_orderbook_payload,
    validate_orderbook_payload,
    validate_pdp_bundle,
    validate_pdp_challenge_proof,
    validate_pdp_commitment_challenge,
    validate_pdp_payload,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]
_ORDERBOOK_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "orderbook"
_PDP_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "pdp"
_ORDERBOOK_PRIVATE_KEY = bytes([0xB7]) * 32
_ORDERBOOK_OWNER_ACCOUNT = b"merchant@paynet"


def _fixture(path: Path) -> bytes:
    return path.read_bytes()


def _pdp_fixtures() -> tuple[bytes, bytes, bytes]:
    return (
        _fixture(_PDP_FIXTURES / "commitment_v1.to"),
        _fixture(_PDP_FIXTURES / "challenge_v1.to"),
        _fixture(_PDP_FIXTURES / "proof_v1.to"),
    )


def _fixed32(value: int) -> bytes:
    return bytes([value]) * 32


def test_validate_orderbook_payload_accepts_canonical_order_request() -> None:
    outcome = validate_orderbook_payload(
        "order",
        _fixture(_ORDERBOOK_FIXTURES / "order_request_v1.to"),
        label="fixtures/sorafs_manifest/orderbook/order_request_v1.to",
        generated_at_unix=1_700_000_123,
    )

    assert outcome["status"] == "Ok"
    assert outcome["code"] == "SFS-OK-000"
    assert outcome["category"] == "validation"
    assert outcome["generated_at"] == 1_700_000_123
    assert outcome["inputs"][0]["kind"] == "orderbook_order_request"
    assert outcome["inputs"][0]["path"] == "fixtures/sorafs_manifest/orderbook/order_request_v1.to"


def test_validate_orderbook_payload_accepts_runtime_snapshot_alias() -> None:
    outcome = validate_orderbook_payload(
        SORAFS_ORDERBOOK_PAYLOAD_KINDS["RUNTIME_SNAPSHOT"],
        memoryview(_fixture(_ORDERBOOK_FIXTURES / "runtime_snapshot_v1.to")),
        generated_at_unix=1_700_000_456,
    )

    assert outcome["status"] == "Ok"
    assert outcome["code"] == "SFS-OK-000"
    assert outcome["inputs"][0]["kind"] == "orderbook_runtime_snapshot"


def test_validate_orderbook_payload_reports_malformed_norito() -> None:
    outcome = validate_orderbook_payload(
        "settlement_receipt",
        b"\x00" * 8,
        generated_at_unix=1_700_000_789,
    )

    assert outcome["status"] == "Error"
    assert outcome["category"] == "norito"
    assert outcome["code"].startswith("SFS-")
    assert outcome["inputs"][0]["kind"] == "settlement_receipt"


def test_sign_orderbook_payload_signs_mutable_fixture_payloads() -> None:
    private_key = bytes([0xB7]) * 32
    cases = (
        ("order", "order_request_v1.to", "orderbook_order_request"),
        ("order-cancel", "order_cancel_v1.to", "orderbook_order_cancel"),
        ("settlement-receipt", "settlement_receipt_v1.to", "settlement_receipt"),
    )

    for kind, filename, input_kind in cases:
        unsigned = _fixture(_ORDERBOOK_FIXTURES / filename)
        signed = sign_orderbook_payload(kind, memoryview(unsigned), private_key)
        assert isinstance(signed, bytes)
        assert signed != unsigned

        outcome = validate_orderbook_payload(kind, signed, generated_at_unix=1_700_000_999)
        assert outcome["status"] == "Ok"
        assert outcome["inputs"][0]["kind"] == input_kind


def test_sign_orderbook_payload_rejects_non_signable_and_bad_keys() -> None:
    snapshot = _fixture(_ORDERBOOK_FIXTURES / "runtime_snapshot_v1.to")
    order = _fixture(_ORDERBOOK_FIXTURES / "order_request_v1.to")

    with pytest.raises(ValueError, match="cannot be signed"):
        sign_orderbook_payload("runtime-snapshot", snapshot, bytes([0xB7]) * 32)
    with pytest.raises(ValueError, match="32 bytes"):
        sign_orderbook_payload("order-request", order, bytes([0xB7]) * 31)


def test_field_level_orderbook_builders_emit_valid_signed_payloads() -> None:
    order = build_signed_orderbook_order_request(
        {
            "orderId": _fixed32(0x11),
            "side": "bid",
            "tier": "hot",
            "pricePerGibMicroXor": "1000000",
            "quantityGib": "12",
            "ownerAccount": _ORDERBOOK_OWNER_ACCOUNT,
            "expiryUnix": "1700010000",
            "nonce": "7",
            "makerFeeBps": "25",
            "takerFeeBps": "30",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "order-request",
        order,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"

    cancel = build_signed_orderbook_order_cancel(
        {
            "order_id": _fixed32(0x11),
            "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
            "reason": "owner_requested",
            "nonce": 8,
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "order-cancel",
        cancel,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"

    receipt = build_signed_orderbook_settlement_receipt(
        {
            "receiptId": _fixed32(0x21),
            "channelId": _fixed32(0x22),
            "tradeId": _fixed32(0x23),
            "rangeStart": "0",
            "rangeEnd": "4096",
            "chunkHash": _fixed32(0x24),
            "bytesDelivered": "4096",
            "xorDebitedMicroXor": "100",
            "providerCreditMicroXor": "90",
            "feeAmountMicroXor": "10",
            "issuedAtUnix": "1700000999",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "settlement-receipt",
        receipt,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"


def test_field_level_settlement_receipt_builder_rejects_imbalanced_amounts() -> None:
    with pytest.raises(ValueError, match="settlement imbalance"):
        build_signed_orderbook_settlement_receipt(
            {
                "receiptId": _fixed32(0x31),
                "channelId": _fixed32(0x32),
                "tradeId": _fixed32(0x33),
                "rangeStart": "0",
                "rangeEnd": "4096",
                "chunkHash": _fixed32(0x34),
                "bytesDelivered": "4096",
                "xorDebitedMicroXor": "100",
                "providerCreditMicroXor": "91",
                "feeAmountMicroXor": "10",
                "issuedAtUnix": "1700000999",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_validate_pdp_payload_accepts_canonical_commitment() -> None:
    commitment, _challenge, _proof = _pdp_fixtures()
    outcome = validate_pdp_payload(
        SORAFS_PDP_PAYLOAD_KINDS["COMMITMENT"],
        commitment,
        label="fixtures/sorafs_manifest/pdp/commitment_v1.to",
        generated_at_unix=1_700_001_001,
    )

    assert outcome["status"] == "Ok"
    assert outcome["code"] == "SFS-OK-000"
    assert outcome["inputs"][0]["kind"] == "pdp_commitment"
    assert outcome["inputs"][0]["path"] == "fixtures/sorafs_manifest/pdp/commitment_v1.to"
    assert outcome["generated_at"] == 1_700_001_001


def test_validate_pdp_pair_and_bundle_helpers_accept_bound_fixtures() -> None:
    commitment, challenge, proof = _pdp_fixtures()

    commitment_challenge = validate_pdp_commitment_challenge(
        commitment,
        challenge,
        commitment_label="commitment.to",
        challenge_label="challenge.to",
        generated_at_unix=1_700_001_002,
    )
    challenge_proof = validate_pdp_challenge_proof(
        challenge,
        proof,
        challenge_label="challenge.to",
        proof_label="proof.to",
        generated_at_unix=1_700_001_003,
    )
    bundle = validate_pdp_bundle(
        commitment,
        challenge,
        proof,
        commitment_label="commitment.to",
        challenge_label="challenge.to",
        proof_label="proof.to",
        generated_at_unix=1_700_001_004,
    )

    assert commitment_challenge["status"] == "Ok"
    assert [entry["kind"] for entry in commitment_challenge["inputs"]] == [
        "pdp_commitment",
        "pdp_challenge",
    ]
    assert challenge_proof["status"] == "Ok"
    assert [entry["kind"] for entry in challenge_proof["inputs"]] == [
        "pdp_challenge",
        "pdp_proof",
    ]
    assert bundle["status"] == "Ok"
    assert [entry["kind"] for entry in bundle["inputs"]] == [
        "pdp_commitment",
        "pdp_challenge",
        "pdp_proof",
    ]
    assert bundle["generated_at"] == 1_700_001_004


def test_validate_pdp_payload_reports_malformed_payloads() -> None:
    outcome = validate_pdp_payload("proof", bytearray(8), generated_at_unix=1_700_001_005)

    assert outcome["status"] == "Error"
    assert outcome["category"] == "norito"
    assert outcome["code"] == "SFS-NORITO-001"
    assert outcome["inputs"][0]["kind"] == "pdp_proof"


def test_validate_pdp_challenge_proof_reports_signature_failure() -> None:
    _commitment, challenge, _proof = _pdp_fixtures()
    outcome = validate_pdp_challenge_proof(
        challenge,
        _fixture(_PDP_FIXTURES / "negative" / "missing_signature_proof_v1.to"),
        generated_at_unix=1_700_001_006,
    )

    assert outcome["status"] == "Error"
    assert outcome["category"] == "signature"
    assert outcome["code"] == "SFS-SIG-008"


def test_reference_validation_rejects_bad_arguments_before_native_validation() -> None:
    with pytest.raises(ValueError, match="unsupported SoraFS PDP payload kind"):
        validate_pdp_payload("bad-kind", b"\x00" * 8)
    with pytest.raises(ValueError, match="generated_at_unix"):
        validate_orderbook_payload("order-request", b"\x00" * 8, generated_at_unix=-1)
    with pytest.raises(TypeError, match="bytes-like"):
        validate_pdp_payload("proof", "not-bytes")  # type: ignore[arg-type]
