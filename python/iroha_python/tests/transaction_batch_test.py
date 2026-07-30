"""Focused tests for ordered mixed transaction batch authoring."""

from __future__ import annotations

from typing import Any

import pytest

import iroha_python.crypto as crypto_module
import iroha_python.tx as tx_module
from iroha_python import ContractCall, TransactionConfig, TransactionDraft

VALID_HASH = "00" * 31 + "01"
VALID_ADDRESS = "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"


def config(*, gas_limit: int | None = 1000) -> TransactionConfig:
    return TransactionConfig(
        chain_id="test-chain",
        authority="ed0120" + "11" * 32,
        fee_payment={
            "payer": "authority",
            "value": {"charge_limits": [], "gas_limit": gas_limit},
        },
        creation_time_ms=42,
    )


def test_contract_call_defensively_copies_and_bounds_arguments() -> None:
    source = bytearray(b"args")
    call = ContractCall(VALID_ADDRESS, VALID_HASH, "run", source)
    source[:] = b"nope"

    assert call.arguments == b"args"
    with pytest.raises(ValueError, match="1048576-byte limit"):
        ContractCall(VALID_ADDRESS, VALID_HASH, "run", b"x" * (1024 * 1024 + 1))
    with pytest.raises(ValueError, match="least significant bit"):
        ContractCall(VALID_ADDRESS, "00" * 32, "run")
    with pytest.raises(ValueError, match="entrypoint"):
        ContractCall(VALID_ADDRESS, VALID_HASH, " ")


def test_transaction_draft_keeps_instruction_call_instruction_order(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, Any] = {}

    def fake_build_signed_transaction(*args: Any, **kwargs: Any) -> str:
        captured["args"] = args
        captured["kwargs"] = kwargs
        return "signed"

    monkeypatch.setattr(tx_module, "build_signed_transaction", fake_build_signed_transaction)
    first = object()
    last = object()
    draft = TransactionDraft(config())
    draft.add_instruction(first)  # type: ignore[arg-type]
    call = draft.add_contract_call(VALID_ADDRESS, VALID_HASH, "run", bytearray(b"abc"))
    draft.add_instruction(last)  # type: ignore[arg-type]

    assert tuple(draft.entries) == (first, call, last)
    assert tuple(draft.instructions) == (first, last)
    assert draft.sign(b"private") == "signed"
    assert captured["kwargs"]["instructions"] is None
    assert captured["kwargs"]["entries"] == [first, call, last]


def test_instruction_only_draft_stays_legacy_unless_batch_is_explicit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[dict[str, Any]] = []

    def fake_build_signed_transaction(*_args: Any, **kwargs: Any) -> str:
        calls.append(kwargs)
        return "signed"

    monkeypatch.setattr(tx_module, "build_signed_transaction", fake_build_signed_transaction)
    instruction = object()
    legacy = TransactionDraft(config())
    legacy.add_instruction(instruction)  # type: ignore[arg-type]
    legacy.sign(b"private")

    explicit = TransactionDraft(config()).use_executable_batch()
    explicit.add_instruction(instruction)  # type: ignore[arg-type]
    explicit.sign(b"private")

    assert calls[0]["instructions"] == [instruction]
    assert calls[0]["entries"] is None
    assert calls[1]["instructions"] is None
    assert calls[1]["entries"] == [instruction]


def test_wallet_transfer_controls_chain_into_one_atomic_draft(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[Any, ...]] = []

    class FakeInstruction:
        @staticmethod
        def set_asset_transfer_availability(*args: Any, **kwargs: Any) -> tuple[Any, ...]:
            instruction = ("availability", *args, kwargs)
            calls.append(instruction)
            return instruction

        @staticmethod
        def set_asset_transfer_blacklist(*args: Any) -> tuple[Any, ...]:
            instruction = ("blacklist", *args)
            calls.append(instruction)
            return instruction

        @staticmethod
        def set_asset_transfer_control(*args: Any) -> tuple[Any, ...]:
            instruction = ("transfer_control", *args)
            calls.append(instruction)
            return instruction

        @staticmethod
        def set_asset_holding_limit(*args: Any) -> tuple[Any, ...]:
            instruction = ("holding_limit", *args)
            calls.append(instruction)
            return instruction

    monkeypatch.setattr(tx_module, "Instruction", FakeInstruction)
    draft = TransactionDraft(config())

    returned = (
        draft.set_asset_transfer_availability(
            "wallet",
            "digital_shekel",
            0,
            tx_module.AssetTransferAvailability.DISABLED,
            tx_module.AssetTransferAvailability.DISABLED,
            reason="operator close",
        )
        .set_asset_transfer_blacklist(
            "wallet",
            "digital_shekel",
            False,
        )
        .set_asset_transfer_control(
            "wallet",
            "digital_shekel",
            [{"window": "day", "cap_amount": 50}],
        )
        .set_asset_holding_limit("wallet", "digital_shekel", 0)
    )

    assert returned is draft
    assert tuple(draft.entries) == tuple(calls)
    assert calls == [
        (
            "availability",
            "wallet",
            "digital_shekel",
            0,
            "Disabled",
            "Disabled",
            {"reason": "operator close"},
        ),
        ("blacklist", "wallet", "digital_shekel", False),
        (
            "transfer_control",
            "wallet",
            "digital_shekel",
            [{"window": "DAY", "cap_amount": "50"}],
        ),
        ("holding_limit", "wallet", "digital_shekel", "0"),
    ]


@pytest.mark.parametrize(
    "reason",
    [
        "line\nbreached",
        "ר" * 257,
    ],
)
def test_asset_transfer_availability_rejects_noncanonical_reason_before_native_call(
    monkeypatch: pytest.MonkeyPatch,
    reason: str,
) -> None:
    class FakeInstruction:
        @staticmethod
        def set_asset_transfer_availability(*args: Any, **kwargs: Any) -> tuple[Any, ...]:
            raise AssertionError(
                f"native constructor must not be called: {args!r} {kwargs!r}"
            )

    monkeypatch.setattr(tx_module, "Instruction", FakeInstruction)
    with pytest.raises(ValueError):
        TransactionDraft(config()).set_asset_transfer_availability(
            "wallet",
            "digital_shekel",
            0,
            tx_module.AssetTransferAvailability.DISABLED,
            tx_module.AssetTransferAvailability.DISABLED,
            reason=reason,
        )


def test_asset_transfer_caps_reject_duplicate_windows_before_native_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class FakeInstruction:
        @staticmethod
        def set_asset_transfer_control(*args: Any) -> tuple[Any, ...]:
            raise AssertionError(f"native constructor must not be called: {args!r}")

    monkeypatch.setattr(tx_module, "Instruction", FakeInstruction)
    draft = TransactionDraft(config())

    with pytest.raises(ValueError, match="duplicates"):
        draft.set_asset_transfer_control(
            "wallet",
            "digital_shekel",
            [
                {"window": "DAY", "cap_amount": 50},
                {"window": "day", "cap_amount": 25},
            ],
        )

    assert tuple(draft.entries) == ()


class RecordingBuilder:
    """Minimal native-builder stand-in used to inspect authoring calls."""

    def __init__(self, *_args: Any) -> None:
        self.operations: list[tuple[Any, ...]] = []

    def use_executable_batch(self) -> None:
        self.operations.append(("batch",))

    def add_instruction(self, instruction: Any) -> None:
        self.operations.append(("instruction", instruction))

    def add_contract_call(self, *call: Any) -> None:
        self.operations.append(("call", *call))

    def set_creation_time_ms(self, value: int) -> None:
        self.operations.append(("time", value))

    def sign(self, private_key: bytes) -> str:
        self.operations.append(("sign", private_key))
        return "signed"


def test_build_signed_transaction_entries_select_batch_and_reject_dual_inputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    builders: list[RecordingBuilder] = []

    def make_builder(*args: Any) -> RecordingBuilder:
        builder = RecordingBuilder(*args)
        builders.append(builder)
        return builder

    monkeypatch.setattr(crypto_module, "TransactionBuilder", make_builder)
    instruction = object()
    call = ContractCall(VALID_ADDRESS, VALID_HASH, "run", b"abc")

    result = crypto_module.build_signed_transaction(
        "test-chain",
        "authority",
        b"private",
        fee_payment={"payer": "authority", "value": {}},
        entries=[instruction, call],  # type: ignore[list-item]
        creation_time_ms=42,
    )

    assert result == "signed"
    assert builders[0].operations == [
        ("time", 42),
        ("batch",),
        ("instruction", instruction),
        ("call", VALID_ADDRESS, VALID_HASH, "run", b"abc"),
        ("sign", b"private"),
    ]
    with pytest.raises(ValueError, match="mutually exclusive"):
        crypto_module.build_signed_transaction(
            "test-chain",
            "authority",
            b"private",
            fee_payment={"payer": "authority", "value": {}},
            instructions=[],
            entries=[],
        )
