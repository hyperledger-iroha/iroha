from __future__ import annotations

import json
from pathlib import Path

import pytest
from iroha_python import (
    ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
    SORAFS_GOVERNANCE_DAG_CID_BYTES_V1,
    SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1,
    SORAFS_ORDERBOOK_PAYLOAD_KINDS,
    SORAFS_PDP_PAYLOAD_KINDS,
    SORAFS_REFERENCE_MAX_LABEL_BYTES_V1,
    SorafsGovernanceDagBlockInput,
    build_signed_orderbook_order_cancel,
    build_signed_orderbook_order_request,
    build_signed_orderbook_settlement_receipt,
    derive_orderbook_order_id,
    sign_orderbook_payload,
    validate_governance_dag_block,
    validate_governance_dag_head_chain,
    validate_orderbook_payload,
    validate_pdp_bundle,
    validate_pdp_challenge_proof,
    validate_pdp_commitment_challenge,
    validate_pdp_payload,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]
_ORDERBOOK_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "orderbook"
_PDP_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "pdp"
_GOVERNANCE_FIXTURES = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "governance"
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
) -> None:
    expected_text = (fixture_root / fixture_name).read_text(encoding="utf-8")
    assert outcome == json.loads(expected_text)
    assert json.dumps(outcome, indent=2, ensure_ascii=True) + "\n" == expected_text


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
        "order",
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
    order_id = derive_orderbook_order_id(_ORDERBOOK_OWNER_ACCOUNT, 7)
    assert len(order_id) == 32
    order = build_signed_orderbook_order_request(
        {
            "side": "bid",
            "tier": "hot",
            "pricePerGib": _MAX_SCALED_XOR,
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
            "receiptId": _fixed32(0x21),
            "channelId": _fixed32(0x22),
            "tradeId": _fixed32(0x23),
            "rangeStart": "0",
            "rangeEnd": "4096",
            "chunkHash": _fixed32(0x24),
            "bytesDelivered": "4096",
            "xorDebited": "340282366920938463463374607431768211456.000000001",
            "providerCredit": "340282366920938463463374607431768211456",
            "feeAmount": "0.000000001",
            "issuedAtUnix": "1700000999",
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
                "xorDebited": "100",
                "providerCredit": "91",
                "feeAmount": "10",
                "issuedAtUnix": "1700000999",
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


def test_field_level_orderbook_builders_reject_duplicate_exact_aliases() -> None:
    with pytest.raises(TypeError, match="exactly once"):
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

    with pytest.raises(TypeError, match="exactly once"):
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
    with pytest.raises(ValueError, match="unsupported SoraFS PDP payload kind"):
        validate_pdp_payload("bad-kind", b"\x00" * 8)
    with pytest.raises(ValueError, match="generated_at_unix"):
        validate_orderbook_payload("order-request", b"\x00" * 8, generated_at_unix=-1)
    with pytest.raises(TypeError, match="bytes-like"):
        validate_pdp_payload("proof", "not-bytes")  # type: ignore[arg-type]


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
