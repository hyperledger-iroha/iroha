from __future__ import annotations

import json
from pathlib import Path

import pytest

import iroha_python.sorafs as sorafs_module
from iroha_python import (
    ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
    SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1,
    SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS,
    SORAFS_GOVERNANCE_DAG_CID_BYTES_V1,
    SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1,
    SORAFS_ORDERBOOK_PAYLOAD_KINDS,
    SORAFS_PDP_PAYLOAD_KINDS,
    SORAFS_REFERENCE_MAX_LABEL_BYTES_V1,
    SorafsFixtureBundlePayloadInput,
    SorafsGovernanceDagBlockInput,
    build_signed_orderbook_order_cancel,
    build_signed_orderbook_order_request,
    build_signed_orderbook_settlement_receipt,
    derive_orderbook_order_id,
    sign_orderbook_payload,
    validate_fixture_bundle,
    validate_governance_dag_block,
    validate_governance_dag_head_chain,
    validate_governance_log_node,
    validate_orderbook_payload,
    validate_pdp_bundle,
    validate_pdp_challenge_proof,
    validate_pdp_commitment_challenge,
    validate_pdp_payload,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]
_ORDERBOOK_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "orderbook"
_PDP_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "pdp"
_SORAFS_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest"
_GOVERNANCE_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "governance"
_MODERATION_FIXTURES = _SORAFS_FIXTURES / "moderation"
_REFERENCE_SDK_FIXTURES = _SORAFS_FIXTURES / "reference_sdk"
_REFERENCE_SDK_GENERATED_AT = 1_700_001_234
_KINDS = SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS
_REFERENCE_SDK_BUNDLE_PROFILES = (
    (
        "bundle_heterogeneous_positive",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["PDP_COMMITMENT"], "pdp/commitment_v1.to"),
            (_KINDS["PDP_CHALLENGE"], "pdp/challenge_v1.to"),
            (_KINDS["PDP_PROOF"], "pdp/proof_v1.to"),
            (_KINDS["POR_CHALLENGE"], "por/challenge_v1.to"),
            (_KINDS["POR_PROOF"], "por/proof_v1.to"),
            (_KINDS["POTR_RECEIPT"], "potr/receipt_v1.to"),
            (_KINDS["REPAIR_TASK_RECORD"], "repair/task_v1.to"),
            (
                _KINDS["ORDERBOOK_ORDER_REQUEST"],
                "orderbook/order_request_v1.to",
            ),
            (
                _KINDS["ORDERBOOK_ORDER_CANCEL"],
                "orderbook/order_cancel_v1.to",
            ),
            (
                _KINDS["ORDERBOOK_TRADE_EVENT"],
                "orderbook/trade_event_v1.to",
            ),
            (
                _KINDS["ORDERBOOK_SETTLEMENT_CHANNEL"],
                "orderbook/settlement_channel_v1.to",
            ),
            (
                _KINDS["ORDERBOOK_SETTLEMENT_RECEIPT"],
                "orderbook/settlement_receipt_v1.to",
            ),
        ),
    ),
    (
        "bundle_orderbook_bad_signature_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["POR_CHALLENGE"], "por/challenge_v1.to"),
            (_KINDS["POR_PROOF"], "por/proof_v1.to"),
            (
                _KINDS["ORDERBOOK_ORDER_REQUEST"],
                "orderbook/negative/order_request_bad_signature_v1.to",
            ),
        ),
    ),
    (
        "bundle_orderbook_trailing_bytes_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["POR_CHALLENGE"], "por/challenge_v1.to"),
            (_KINDS["POR_PROOF"], "por/proof_v1.to"),
            (
                _KINDS["ORDERBOOK_ORDER_REQUEST"],
                "orderbook/negative/order_request_trailing_bytes_v1.to",
            ),
        ),
    ),
    (
        "bundle_pdp_duplicate_hot_leaf_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["PDP_COMMITMENT"], "pdp/commitment_v1.to"),
            (
                _KINDS["PDP_CHALLENGE"],
                "pdp/negative/duplicate_hot_leaf_challenge_v1.to",
            ),
        ),
    ),
    (
        "bundle_pdp_missing_signature_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["PDP_COMMITMENT"], "pdp/commitment_v1.to"),
            (_KINDS["PDP_CHALLENGE"], "pdp/challenge_v1.to"),
            (
                _KINDS["PDP_PROOF"],
                "pdp/negative/missing_signature_proof_v1.to",
            ),
        ),
    ),
    (
        "bundle_pdp_wrong_provider_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (_KINDS["PDP_COMMITMENT"], "pdp/commitment_v1.to"),
            (_KINDS["PDP_CHALLENGE"], "pdp/challenge_v1.to"),
            (
                _KINDS["PDP_PROOF"],
                "pdp/negative/wrong_provider_proof_v1.to",
            ),
        ),
    ),
    (
        "bundle_repair_manifest_mismatch_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (
                _KINDS["REPAIR_TASK_RECORD"],
                "repair/negative/task_manifest_mismatch_v1.to",
            ),
        ),
    ),
    (
        "bundle_repair_provider_unassigned_negative",
        1_700_000_001,
        (
            (_KINDS["REPLICATION_ORDER"], "replication_order/order_v1.to"),
            (
                _KINDS["REPAIR_TASK_RECORD"],
                "repair/negative/task_provider_unassigned_v1.to",
            ),
        ),
    ),
    (
        "bundle_routing_admission_positive",
        300,
        (
            (_KINDS["PROVIDER_ADVERT"], "provider_admission/advert_v1.to"),
            (
                _KINDS["PROVIDER_ADMISSION_ENVELOPE"],
                "provider_admission/envelope_v1.to",
            ),
        ),
    ),
)
_ORDERBOOK_PRIVATE_KEY = bytes([0xB7]) * 32
_ORDERBOOK_OWNER_ACCOUNT = b"merchant@paynet"
_MAX_SCALED_XOR = (
    "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824.503042047"
)


def _fixture(path: Path) -> bytes:
    return path.read_bytes()


def _assert_exact_outcome(
    outcome: dict[str, object],
    fixture_root: Path,
    fixture_name: str,
    *,
    ensure_ascii: bool = True,
) -> None:
    expected_text = (fixture_root / fixture_name).read_text(encoding="utf-8")
    assert outcome == json.loads(expected_text)
    assert json.dumps(outcome, indent=2, ensure_ascii=ensure_ascii) + "\n" == expected_text


def _assert_governance_outcome(
    outcome: dict[str, object],
    fixture_name: str,
) -> None:
    _assert_exact_outcome(outcome, _GOVERNANCE_FIXTURES, fixture_name)


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
        SORAFS_ORDERBOOK_PAYLOAD_KINDS["ORDER_REQUEST"],
        _fixture(_ORDERBOOK_FIXTURES / "order_request_v1.to"),
        label="order_request_v1.to",
        generated_at_unix=123,
    )

    _assert_exact_outcome(
        outcome,
        _ORDERBOOK_FIXTURES,
        "order_request_validation_outcome_v1.json",
    )


def test_orderbook_signature_and_noncanonical_outcomes_match_exactly() -> None:
    for name in ("order_request_bad_signature", "order_request_trailing_bytes"):
        outcome = validate_orderbook_payload(
            "order-request",
            _fixture(_ORDERBOOK_FIXTURES / "negative" / f"{name}_v1.to"),
            label=f"{name}_v1.to",
            generated_at_unix=123,
        )
        _assert_exact_outcome(
            outcome,
            _ORDERBOOK_FIXTURES,
            f"negative/{name}_validation_outcome_v1.json",
        )


def test_validate_orderbook_payload_reports_malformed_norito() -> None:
    outcome = validate_orderbook_payload(
        "settlement-receipt",
        b"\x00" * 8,
        generated_at_unix=1_700_000_789,
    )

    assert outcome["status"] == "Error"
    assert outcome["category"] == "norito"
    assert outcome["code"].startswith("SFS-")
    assert outcome["inputs"][0]["kind"] == "settlement_receipt"


def test_sign_orderbook_payload_deterministically_reproduces_signed_fixtures() -> None:
    private_key = bytes([0xB7]) * 32
    cases = (
        ("order-request", "order_request_v1.to", "orderbook_order_request"),
        ("order-cancel", "order_cancel_v1.to", "orderbook_order_cancel"),
        ("settlement-receipt", "settlement_receipt_v1.to", "settlement_receipt"),
    )

    for kind, filename, input_kind in cases:
        unsigned = _fixture(_ORDERBOOK_FIXTURES / filename)
        signed = sign_orderbook_payload(kind, memoryview(unsigned), private_key)
        assert isinstance(signed, bytes)
        assert signed == unsigned

        outcome = validate_orderbook_payload(kind, signed, generated_at_unix=1_700_000_999)
        assert outcome["status"] == "Ok"
        assert outcome["inputs"][0]["kind"] == input_kind


def test_sign_orderbook_payload_rejects_non_signable_and_bad_keys() -> None:
    trade = _fixture(_ORDERBOOK_FIXTURES / "trade_event_v1.to")
    order = _fixture(_ORDERBOOK_FIXTURES / "order_request_v1.to")

    with pytest.raises(ValueError, match="cannot be signed"):
        sign_orderbook_payload("trade-event", trade, bytes([0xB7]) * 32)
    with pytest.raises(ValueError, match="32 bytes"):
        sign_orderbook_payload("order-request", order, bytes([0xB7]) * 31)


def test_field_level_orderbook_builders_emit_valid_signed_payloads() -> None:
    order_id = derive_orderbook_order_id(_ORDERBOOK_OWNER_ACCOUNT, 7)
    assert len(order_id) == 32
    order = build_signed_orderbook_order_request(
        {
            "side": "bid",
            "tier": "hot",
            "price_per_gib": _MAX_SCALED_XOR,
            "quantity_gib": "12",
            "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
            "expiry_unix": "1700010000",
            "nonce": "7",
            "maker_fee_bps": "25",
            "taker_fee_bps": "30",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "order-request",
        order,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"

    ask = build_signed_orderbook_order_request(
        {
            "side": "ask",
            "tier": "hot",
            "price_per_gib": "1.25",
            "quantity_gib": "4",
            "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
            "provider_id": _fixed32(0x72),
            "expiry_unix": "1700010000",
            "nonce": "8",
            "maker_fee_bps": "25",
            "taker_fee_bps": "30",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert ask != order
    assert validate_orderbook_payload(
        "order-request",
        ask,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"
    ask_other_provider = build_signed_orderbook_order_request(
        {
            "side": "ask",
            "tier": "hot",
            "price_per_gib": "1.25",
            "quantity_gib": "4",
            "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
            "provider_id": _fixed32(0x73),
            "expiry_unix": "1700010000",
            "nonce": "8",
            "maker_fee_bps": "25",
            "taker_fee_bps": "30",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert ask_other_provider != ask

    cancel = build_signed_orderbook_order_cancel(
        {
            "order_id": order_id,
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
            "receipt_id": _fixed32(0x21),
            "channel_id": _fixed32(0x22),
            "trade_id": _fixed32(0x23),
            "range_start": "0",
            "range_end": "4096",
            "chunk_hash": _fixed32(0x24),
            "bytes_delivered": "4096",
            "xor_debited": "340282366920938463463374607431768211456.000000001",
            "provider_credit": "340282366920938463463374607431768211456",
            "fee_amount": "0.000000001",
            "issued_at_unix": "1700000999",
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "settlement-receipt",
        receipt,
        generated_at_unix=1_700_000_999,
    )["status"] == "Ok"


def test_order_id_derivation_matches_cross_sdk_golden_vector() -> None:
    assert derive_orderbook_order_id(b"buyer@sora", 7).hex() == (
        "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69"
    )
    with pytest.raises(ValueError, match="must not be empty"):
        derive_orderbook_order_id(b"", 7)
    with pytest.raises(ValueError, match="greater than zero"):
        derive_orderbook_order_id(b"buyer@sora", 0)


def test_orderbook_builders_accept_owner_account_at_v1_byte_ceiling() -> None:
    owner_account = bytes([0x45]) * ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1
    order_id = derive_orderbook_order_id(owner_account, 9)
    order = build_signed_orderbook_order_request(
        {
            "side": "bid",
            "tier": "hot",
            "price_per_gib": "1",
            "quantity_gib": "1",
            "owner_account": owner_account,
            "expiry_unix": "1700010000",
            "nonce": "9",
            "maker_fee_bps": 0,
            "taker_fee_bps": 0,
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "order-request", order, generated_at_unix=1
    )["status"] == "Ok"

    cancel = build_signed_orderbook_order_cancel(
        {
            "order_id": order_id,
            "owner_account": owner_account,
            "reason": "owner_requested",
            "nonce": 10,
        },
        _ORDERBOOK_PRIVATE_KEY,
    )
    assert validate_orderbook_payload(
        "order-cancel", cancel, generated_at_unix=1
    )["status"] == "Ok"


def test_orderbook_owner_account_byte_ceiling_rejects_adversarial_inputs() -> None:
    owner_account = bytes([0x45]) * (ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1)
    expected = "owner_account must be at most 256 bytes"
    with pytest.raises(ValueError, match=expected):
        derive_orderbook_order_id(owner_account, 9)
    with pytest.raises(ValueError, match=expected):
        build_signed_orderbook_order_request(
            {
                "side": "bid",
                "tier": "hot",
                "price_per_gib": "1",
                "quantity_gib": "1",
                "owner_account": owner_account,
                "expiry_unix": "1700010000",
                "nonce": "9",
                "maker_fee_bps": 0,
                "taker_fee_bps": 0,
            },
            _ORDERBOOK_PRIVATE_KEY,
        )
    with pytest.raises(ValueError, match=expected):
        build_signed_orderbook_order_cancel(
            {
                "order_id": _fixed32(0x45),
                "owner_account": owner_account,
                "reason": "owner_requested",
                "nonce": 10,
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_field_level_orderbook_builder_rejects_noncanonical_order_id() -> None:
    with pytest.raises(ValueError, match="canonical owner-and-nonce derivation"):
        build_signed_orderbook_order_request(
            {
                "order_id": _fixed32(0x11),
                "side": "bid",
                "tier": "hot",
                "price_per_gib": "1",
                "quantity_gib": "12",
                "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
                "expiry_unix": "1700010000",
                "nonce": "7",
                "maker_fee_bps": "25",
                "taker_fee_bps": "30",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_field_level_orderbook_builder_enforces_exact_provider_binding() -> None:
    common = {
        "tier": "hot",
        "price_per_gib": "1",
        "quantity_gib": "1",
        "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
        "expiry_unix": "1700010000",
        "nonce": "17",
        "maker_fee_bps": 0,
        "taker_fee_bps": 0,
    }
    with pytest.raises(ValueError, match="absent or empty for bid"):
        build_signed_orderbook_order_request(
            {**common, "side": "bid", "provider_id": _fixed32(0x72)},
            _ORDERBOOK_PRIVATE_KEY,
        )
    with pytest.raises(ValueError, match="exactly 32 bytes for ask"):
        build_signed_orderbook_order_request(
            {**common, "side": "ask"},
            _ORDERBOOK_PRIVATE_KEY,
        )
    with pytest.raises(ValueError, match="must not be all zero"):
        build_signed_orderbook_order_request(
            {**common, "side": "ask", "provider_id": bytes(32)},
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_field_level_settlement_receipt_builder_rejects_imbalanced_amounts() -> None:
    with pytest.raises(ValueError, match="settlement imbalance"):
        build_signed_orderbook_settlement_receipt(
            {
                "receipt_id": _fixed32(0x31),
                "channel_id": _fixed32(0x32),
                "trade_id": _fixed32(0x33),
                "range_start": "0",
                "range_end": "4096",
                "chunk_hash": _fixed32(0x34),
                "bytes_delivered": "4096",
                "xor_debited": "100",
                "provider_credit": "91",
                "fee_amount": "10",
                "issued_at_unix": "1700000999",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


@pytest.mark.parametrize(
    "retired_field",
    [
        "price_per_gib_micro_xor",
        "pricePerGibMicroXor",
        "price_per_gib_micro",
        "pricePerGibMicro",
    ],
)
def test_field_level_orderbook_builder_rejects_retired_price_fields(
    retired_field: str,
) -> None:
    with pytest.raises(TypeError, match="retired"):
        build_signed_orderbook_order_request(
            {retired_field: "1000000"},
            _ORDERBOOK_PRIVATE_KEY,
        )


@pytest.mark.parametrize(
    "retired_field",
    [
        "xor_debited_micro_xor",
        "xorDebitedMicroXor",
        "xor_debited_micro",
        "xorDebitedMicro",
        "provider_credit_micro_xor",
        "providerCreditMicroXor",
        "provider_credit_micro",
        "providerCreditMicro",
        "fee_amount_micro_xor",
        "feeAmountMicroXor",
        "fee_amount_micro",
        "feeAmountMicro",
    ],
)
def test_field_level_receipt_builder_rejects_retired_amount_fields(
    retired_field: str,
) -> None:
    with pytest.raises(TypeError, match="retired"):
        build_signed_orderbook_settlement_receipt(
            {retired_field: "100"},
            _ORDERBOOK_PRIVATE_KEY,
        )


@pytest.mark.parametrize(
    "price",
    [
        1,
        1.0,
        True,
        None,
        "",
        "+1",
        "-1",
        " 1",
        "1 ",
        "01",
        "1.",
        ".1",
        "1.0",
        "1.000000000",
        "1e0",
        "0.0000000001",
        str(1 << 511),
        "1" * 156,
        "1" * 10_000,
    ],
)
def test_field_level_orderbook_builder_rejects_noncanonical_xor_quantities(
    price: object,
) -> None:
    with pytest.raises((TypeError, ValueError)):
        build_signed_orderbook_order_request(
            {
                "side": "bid",
                "tier": "hot",
                "price_per_gib": price,
                "quantity_gib": "12",
                "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
                "expiry_unix": "1700010000",
                "nonce": "7",
                "maker_fee_bps": "25",
                "taker_fee_bps": "30",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_max_scaled_xor_quantity_uses_the_155_character_boundary() -> None:
    assert len(_MAX_SCALED_XOR) == 155


def test_field_level_orderbook_builders_reject_retired_field_aliases() -> None:
    with pytest.raises(TypeError, match="retired"):
        build_signed_orderbook_order_request(
            {
                "side": "bid",
                "tier": "hot",
                "price_per_gib": "1",
                "pricePerGib": "2",
                "quantity_gib": "12",
                "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
                "expiry_unix": "1700010000",
                "nonce": "7",
                "maker_fee_bps": "25",
                "taker_fee_bps": "30",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )

    with pytest.raises(TypeError, match="retired"):
        build_signed_orderbook_settlement_receipt(
            {
                "receipt_id": _fixed32(0x41),
                "channel_id": _fixed32(0x42),
                "trade_id": _fixed32(0x43),
                "range_start": "0",
                "range_end": "4096",
                "chunk_hash": _fixed32(0x44),
                "bytes_delivered": "4096",
                "xor_debited": "100",
                "provider_credit": "90",
                "fee_amount": "10",
                "feeAmount": "9",
                "issued_at_unix": "1700000999",
            },
            _ORDERBOOK_PRIVATE_KEY,
        )


def test_field_level_orderbook_builders_reject_noncanonical_selectors() -> None:
    common = {
        "tier": "hot",
        "price_per_gib": "1",
        "quantity_gib": "12",
        "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
        "expiry_unix": "1700010000",
        "nonce": "7",
        "maker_fee_bps": "25",
        "taker_fee_bps": "30",
    }
    for side in ("Bid", " bid", "BID"):
        with pytest.raises(ValueError, match="canonical V1 selector"):
            build_signed_orderbook_order_request(
                {**common, "side": side},
                _ORDERBOOK_PRIVATE_KEY,
            )
    with pytest.raises(ValueError, match="canonical V1 selector"):
        build_signed_orderbook_order_cancel(
            {
                "order_id": _fixed32(0x45),
                "owner_account": _ORDERBOOK_OWNER_ACCOUNT,
                "reason": "owner-requested",
                "nonce": 10,
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
    assert outcome["code"] == "SFS-PDP-DIAG-000"
    assert {entry["key"]: entry["value"] for entry in outcome["context"]}[
        "production_acceptance"
    ] == "false"
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
        commitment_label="commitment_v1.to",
        challenge_label="challenge_v1.to",
        proof_label="proof_v1.to",
        generated_at_unix=123,
    )

    assert commitment_challenge["status"] == "Ok"
    assert commitment_challenge["code"] == "SFS-PDP-DIAG-000"
    assert [entry["kind"] for entry in commitment_challenge["inputs"]] == [
        "pdp_commitment",
        "pdp_challenge",
    ]
    assert challenge_proof["status"] == "Ok"
    assert challenge_proof["code"] == "SFS-PDP-DIAG-000"
    assert [entry["kind"] for entry in challenge_proof["inputs"]] == [
        "pdp_challenge",
        "pdp_proof",
    ]
    _assert_exact_outcome(bundle, _PDP_FIXTURES, "bundle_validation_outcome_v1.json")


def test_validate_fixture_bundle_accepts_linked_replication_and_por() -> None:
    outcome = validate_fixture_bundle(
        (
            SorafsFixtureBundlePayloadInput(
                SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS["REPLICATION_ORDER"],
                _fixture(_SORAFS_FIXTURES / "replication_order" / "order_v1.to"),
                "replication-order.to",
            ),
            SorafsFixtureBundlePayloadInput(
                SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS["POR_PROOF"],
                _fixture(_SORAFS_FIXTURES / "por" / "proof_v1.to"),
                "por-proof.to",
            ),
        ),
        now_unix=1_700_000_001,
        generated_at_unix=1_700_001_238,
    )

    assert outcome["status"] == "Ok"
    assert outcome["code"] == "SFS-OK-000"
    assert outcome["generated_at"] == 1_700_001_238
    assert [entry["kind"] for entry in outcome["inputs"]] == [
        "replication_order",
        "por_proof",
    ]


@pytest.mark.parametrize(
    ("profile_name", "now_unix", "payload_specs"),
    _REFERENCE_SDK_BUNDLE_PROFILES,
    ids=[profile[0] for profile in _REFERENCE_SDK_BUNDLE_PROFILES],
)
def test_fixture_bundle_matches_release_wide_outcomes_byte_for_byte(
    profile_name: str,
    now_unix: int,
    payload_specs: tuple[tuple[str, str], ...],
) -> None:
    assert len(_REFERENCE_SDK_BUNDLE_PROFILES) == 9
    outcome = validate_fixture_bundle(
        tuple(
            SorafsFixtureBundlePayloadInput(
                kind,
                _fixture(_SORAFS_FIXTURES / path),
                path,
            )
            for kind, path in payload_specs
        ),
        now_unix=now_unix,
        generated_at_unix=_REFERENCE_SDK_GENERATED_AT,
    )

    _assert_exact_outcome(
        outcome,
        _REFERENCE_SDK_FIXTURES,
        f"{profile_name}_validation_outcome_v1.json",
    )


def test_fixture_bundle_selectors_and_input_snapshots_are_exact() -> None:
    assert tuple(SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS.values()) == (
        "provider-advert",
        "provider-admission-envelope",
        "replication-order",
        "por-challenge",
        "por-proof",
        "potr-receipt",
        "repair-evidence",
        "repair-report",
        "repair-task-record",
        "repair-slash-proposal",
        "repair-task-event",
        "orderbook-order-request",
        "orderbook-order-cancel",
        "orderbook-trade-event",
        "orderbook-settlement-channel",
        "orderbook-settlement-receipt",
        "pdp-commitment",
        "pdp-challenge",
        "pdp-proof",
    )
    source = bytearray((1, 2, 3))
    payload = SorafsFixtureBundlePayloadInput("por-proof", source)
    source[0] = 9
    assert payload.payload == bytes((1, 2, 3))
    assert (
        SorafsFixtureBundlePayloadInput("por-proof", b"\x00", "proof\u200b.to").label
        == "proof\u200b.to"
    )
    with pytest.raises(ValueError, match="valid Unicode"):
        SorafsFixtureBundlePayloadInput("por-proof", b"\x00", "\ud800")


def test_validate_fixture_bundle_rejects_aliases_and_unbounded_input() -> None:
    with pytest.raises(ValueError, match="fixture-bundle payload kind"):
        SorafsFixtureBundlePayloadInput("por_proof", b"\x00")
    with pytest.raises(ValueError, match=r"1\.\.=64"):
        validate_fixture_bundle(())
    item = SorafsFixtureBundlePayloadInput("por-proof", b"\x00")
    with pytest.raises(ValueError, match=r"1\.\.=64"):
        validate_fixture_bundle(
            (item,) * (SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1 + 1)
        )


def test_all_pdp_negative_outcomes_match_exactly() -> None:
    commitment, challenge, _proof = _pdp_fixtures()

    single_cases = (
        (
            "duplicate_hot_leaf_challenge",
            lambda payload: validate_pdp_payload(
                "challenge",
                payload,
                label="duplicate_hot_leaf_challenge_v1.to",
                generated_at_unix=123,
            ),
        ),
        (
            "missing_signature_proof",
            lambda payload: validate_pdp_payload(
                "proof",
                payload,
                label="missing_signature_proof_v1.to",
                generated_at_unix=123,
            ),
        ),
    )
    pair_names = ("late_proof", "wrong_manifest_proof", "wrong_provider_proof")
    bundle_names = (
        "missing_hot_leaf_path_proof",
        "missing_segment_path_proof",
        "wrong_path_proof",
    )

    for name, validate in single_cases:
        outcome = validate(_fixture(_PDP_FIXTURES / "negative" / f"{name}_v1.to"))
        _assert_exact_outcome(
            outcome,
            _PDP_FIXTURES,
            f"negative/{name}_validation_outcome_v1.json",
        )

    for name in pair_names:
        outcome = validate_pdp_challenge_proof(
            challenge,
            _fixture(_PDP_FIXTURES / "negative" / f"{name}_v1.to"),
            challenge_label="challenge_v1.to",
            proof_label=f"{name}_v1.to",
            generated_at_unix=123,
        )
        _assert_exact_outcome(
            outcome,
            _PDP_FIXTURES,
            f"negative/{name}_validation_outcome_v1.json",
        )

    for name in bundle_names:
        outcome = validate_pdp_bundle(
            commitment,
            challenge,
            _fixture(_PDP_FIXTURES / "negative" / f"{name}_v1.to"),
            commitment_label="commitment_v1.to",
            challenge_label="challenge_v1.to",
            proof_label=f"{name}_v1.to",
            generated_at_unix=123,
        )
        _assert_exact_outcome(
            outcome,
            _PDP_FIXTURES,
            f"negative/{name}_validation_outcome_v1.json",
        )


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
    for kind in (
        "bad-kind",
        "pdp-proof",
        "pdp_proof",
        "PROOF",
        "Proof",
        " proof ",
    ):
        with pytest.raises(ValueError, match="unsupported SoraFS PDP payload kind"):
            validate_pdp_payload(kind, b"\x00" * 8)
    for kind in (
        "bad-kind",
        "order",
        "request",
        "order_request",
        "orderbook-order-request",
        "ORDER-REQUEST",
        " order-request ",
        "runtime-snapshot",
    ):
        with pytest.raises(ValueError, match="unsupported SoraFS orderbook payload kind"):
            validate_orderbook_payload(kind, b"\x00" * 8)
    with pytest.raises(ValueError, match="generated_at_unix"):
        validate_orderbook_payload("order-request", b"\x00" * 8, generated_at_unix=-1)
    with pytest.raises(TypeError, match="bytes-like"):
        validate_pdp_payload("proof", "not-bytes")  # type: ignore[arg-type]


def test_validate_governance_log_node_matches_moderation_outcome_byte_for_byte() -> None:
    node_metadata = json.loads(
        (_MODERATION_FIXTURES / "governance_node_v1.json").read_text(encoding="utf-8")
    )
    outcome = validate_governance_log_node(
        _fixture(_MODERATION_FIXTURES / "governance_node_v1.to"),
        expected_node_cid=bytes.fromhex(node_metadata["node_cid_hex"]),
        label="moderation/governance_node_v1.to",
        generated_at_unix=_REFERENCE_SDK_GENERATED_AT,
    )

    _assert_exact_outcome(
        outcome,
        _MODERATION_FIXTURES,
        "governance_node_validation_outcome_v1.json",
        ensure_ascii=False,
    )


def test_validate_governance_log_node_rejects_bad_cids_before_native_dispatch() -> None:
    for invalid_length in (0, 31, 33):
        with pytest.raises(
            ValueError,
            match=rf"exactly {SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes",
        ):
            validate_governance_log_node(
                b"\x00",
                expected_node_cid=bytes(invalid_length),
                generated_at_unix=1,
            )


def test_validate_governance_log_node_fails_closed_without_native_function(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sorafs_module, "_crypto", object())
    with pytest.raises(
        RuntimeError,
        match="sorafs_validate_governance_log_node_json",
    ):
        validate_governance_log_node(
            b"\x00",
            expected_node_cid=bytes(SORAFS_GOVERNANCE_DAG_CID_BYTES_V1),
            generated_at_unix=1,
        )


def test_validate_governance_dag_block_accepts_canonical_fixture() -> None:
    outcome = validate_governance_dag_block(
        _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to"),
        label="dag_block_0_v1.to",
        generated_at_unix=123,
    )

    _assert_governance_outcome(
        outcome,
        "dag_block_validation_outcome_v1.json",
    )


def test_validate_governance_dag_block_rejects_expected_cid_mismatch() -> None:
    outcome = validate_governance_dag_block(
        _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to"),
        expected_block_cid=bytes([0x7F]) * 32,
        generated_at_unix=123,
    )

    _assert_governance_outcome(
        outcome,
        "dag_block_cid_mismatch_validation_outcome_v1.json",
    )


def test_validate_governance_dag_head_chain_accepts_root_to_head_fixture() -> None:
    blocks = [
        SorafsGovernanceDagBlockInput(
            _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to"),
            "dag_block_0_v1.to",
        ),
        SorafsGovernanceDagBlockInput(
            _fixture(_GOVERNANCE_FIXTURES / "dag_block_1_v1.to"),
            "dag_block_1_v1.to",
        ),
    ]
    outcome = validate_governance_dag_head_chain(
        _fixture(_GOVERNANCE_FIXTURES / "dag_head_v1.to"),
        blocks,
        head_label="dag_head_v1.to",
        generated_at_unix=123,
    )

    _assert_governance_outcome(
        outcome,
        "dag_head_validation_outcome_v1.json",
    )


def test_validate_governance_dag_head_chain_rejects_reordered_blocks() -> None:
    blocks = [
        SorafsGovernanceDagBlockInput(
            _fixture(_GOVERNANCE_FIXTURES / "dag_block_1_v1.to"),
        ),
        SorafsGovernanceDagBlockInput(
            _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to"),
        ),
    ]
    outcome = validate_governance_dag_head_chain(
        _fixture(_GOVERNANCE_FIXTURES / "dag_head_v1.to"),
        blocks,
        generated_at_unix=123,
    )

    _assert_governance_outcome(
        outcome,
        "dag_head_reordered_validation_outcome_v1.json",
    )


def test_governance_dag_negative_vectors_match_reference_outcomes() -> None:
    root = _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to")
    child = _fixture(_GOVERNANCE_FIXTURES / "dag_block_1_v1.to")

    block_signature_outcome = validate_governance_dag_block(
        _fixture(_GOVERNANCE_FIXTURES / "dag_block_bad_signature_v1.to"),
        label="dag_block_bad_signature_v1.to",
        generated_at_unix=123,
    )
    _assert_governance_outcome(
        block_signature_outcome,
        "dag_block_bad_signature_validation_outcome_v1.json",
    )

    trailing_bytes_outcome = validate_governance_dag_block(
        _fixture(_GOVERNANCE_FIXTURES / "dag_block_trailing_bytes_v1.to"),
        label="dag_block_trailing_bytes_v1.to",
        generated_at_unix=123,
    )
    _assert_governance_outcome(
        trailing_bytes_outcome,
        "dag_block_trailing_bytes_validation_outcome_v1.json",
    )

    head_signature_outcome = validate_governance_dag_head_chain(
        _fixture(_GOVERNANCE_FIXTURES / "dag_head_bad_signature_v1.to"),
        [
            SorafsGovernanceDagBlockInput(root, "dag_block_0_v1.to"),
            SorafsGovernanceDagBlockInput(child, "dag_block_1_v1.to"),
        ],
        head_label="dag_head_bad_signature_v1.to",
        generated_at_unix=123,
    )
    _assert_governance_outcome(
        head_signature_outcome,
        "dag_head_bad_signature_validation_outcome_v1.json",
    )

    predecessor_outcome = validate_governance_dag_head_chain(
        _fixture(_GOVERNANCE_FIXTURES / "dag_head_bad_predecessor_v1.to"),
        [
            SorafsGovernanceDagBlockInput(root, "dag_block_0_v1.to"),
            SorafsGovernanceDagBlockInput(
                _fixture(
                    _GOVERNANCE_FIXTURES
                    / "dag_block_1_bad_predecessor_v1.to"
                ),
                "dag_block_1_bad_predecessor_v1.to",
            ),
        ],
        head_label="dag_head_bad_predecessor_v1.to",
        generated_at_unix=123,
    )
    _assert_governance_outcome(
        predecessor_outcome,
        "dag_head_bad_predecessor_validation_outcome_v1.json",
    )


def test_governance_dag_wrappers_enforce_labels_and_block_count() -> None:
    root = _fixture(_GOVERNANCE_FIXTURES / "dag_block_0_v1.to")
    head = _fixture(_GOVERNANCE_FIXTURES / "dag_head_v1.to")
    with pytest.raises(ValueError, match="UTF-8 bytes"):
        validate_governance_dag_block(
            root,
            label="x" * (SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 + 1),
        )
    with pytest.raises(ValueError, match="control characters"):
        validate_governance_dag_block(root, label="bad\u0001label")
    for invalid_length in (0, 31, 33):
        with pytest.raises(
            ValueError,
            match=rf"exactly {SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes",
        ):
            validate_governance_dag_block(
                root,
                expected_block_cid=bytes(invalid_length),
            )
    with pytest.raises(ValueError, match=r"1\.\.="):
        validate_governance_dag_head_chain(head, [])
    with pytest.raises(ValueError, match=r"1\.\.="):
        validate_governance_dag_head_chain(
            head,
            [
                SorafsGovernanceDagBlockInput(root)
                for _ in range(SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1 + 1)
            ],
        )
