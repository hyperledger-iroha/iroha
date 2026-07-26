"""Focused parity tests for native SoraFS replication-order instructions."""

from __future__ import annotations

import base64
from pathlib import Path

import pytest

from iroha_python import (
    CompleteReplicationOrderInstruction,
    ExpireReplicationOrderInstruction,
    IssueReplicationOrderInstruction,
    SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1,
    decode_replication_order_instruction,
)

_ORDER_ID = "ab" * 32
_PROVIDER_ID = "10" * 32
_FIXTURE = (
    Path(__file__).resolve().parents[3]
    / "fixtures"
    / "sorafs_manifest"
    / "replication_order"
    / "order_v1.to"
).read_bytes()
_CRC64_POLY = 0xC96C5795D7870F42
_U64_MASK = (1 << 64) - 1


def _crc64(payload: bytes) -> int:
    crc = _U64_MASK
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = ((crc >> 1) ^ _CRC64_POLY) if crc & 1 else crc >> 1
    return (crc ^ _U64_MASK) & _U64_MASK


def _mutated_payload(needle: bytes, replacement: bytes) -> str:
    assert len(needle) == len(replacement)
    payload = bytearray(_FIXTURE)
    offset = payload.find(needle, 40)
    assert offset >= 40
    assert payload.find(needle, offset + 1) == -1
    payload[offset : offset + len(needle)] = replacement
    payload[31:39] = _crc64(bytes(payload[40:])).to_bytes(8, "little")
    return base64.b64encode(payload).decode("ascii")


def test_replication_instruction_payloads_use_exact_rust_fields() -> None:
    issue = IssueReplicationOrderInstruction(
        _ORDER_ID,
        base64.b64encode(_FIXTURE).decode("ascii"),
        20,
        28,
    )
    assert issue.to_payload() == {
        "IssueReplicationOrder": {
            "order_id": _ORDER_ID,
            "order_payload": base64.b64encode(_FIXTURE).decode("ascii"),
            "issued_epoch": 20,
            "deadline_epoch": 28,
        }
    }
    assert IssueReplicationOrderInstruction.from_payload(issue.to_payload()) == issue

    complete = CompleteReplicationOrderInstruction(_ORDER_ID, _PROVIDER_ID, 27)
    assert complete.to_payload() == {
        "CompleteReplicationOrder": {
            "order_id": _ORDER_ID,
            "provider_id": _PROVIDER_ID,
            "completion_epoch": 27,
        }
    }
    assert decode_replication_order_instruction(complete.to_payload()) == complete

    expire = ExpireReplicationOrderInstruction(_ORDER_ID, 29)
    assert decode_replication_order_instruction(expire.to_payload()) == expire


def test_replication_instruction_decoders_are_schema_closed() -> None:
    with pytest.raises(ValueError, match="provider_id"):
        CompleteReplicationOrderInstruction.from_payload(
            {
                "CompleteReplicationOrder": {
                    "order_id": _ORDER_ID,
                    "completion_epoch": 27,
                }
            }
        )
    with pytest.raises(ValueError, match="relayer"):
        CompleteReplicationOrderInstruction.from_payload(
            {
                "CompleteReplicationOrder": {
                    "order_id": _ORDER_ID,
                    "provider_id": _PROVIDER_ID,
                    "completion_epoch": 27,
                    "relayer": "confused-deputy",
                }
            }
        )
    with pytest.raises(ValueError, match="zero identifier"):
        CompleteReplicationOrderInstruction(_ORDER_ID, "00" * 32, 27)
    with pytest.raises(ValueError, match="lowercase hexadecimal"):
        CompleteReplicationOrderInstruction(_ORDER_ID.upper(), _PROVIDER_ID, 27)
    with pytest.raises(ValueError, match="non-negative u64"):
        ExpireReplicationOrderInstruction(_ORDER_ID, -1)
    with pytest.raises(ValueError, match="greater than issued_epoch"):
        IssueReplicationOrderInstruction(
            _ORDER_ID,
            base64.b64encode(_FIXTURE).decode("ascii"),
            20,
            20,
        )


def test_issue_rejects_invalid_embedded_replication_order_policy() -> None:
    canonical = base64.b64encode(_FIXTURE).decode("ascii")
    with pytest.raises(ValueError, match="canonical standard base64"):
        IssueReplicationOrderInstruction(_ORDER_ID, canonical + "\n", 1, 2)
    with pytest.raises(ValueError, match="must match"):
        IssueReplicationOrderInstruction("ac" * 32, canonical, 1, 2)
    with pytest.raises(ValueError, match="decoded limit"):
        IssueReplicationOrderInstruction(
            _ORDER_ID,
            base64.b64encode(
                bytes(SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 + 1)
            ).decode("ascii"),
            1,
            2,
        )

    duplicate_provider = _mutated_payload(bytes([0x11]) * 32, bytes([0x10]) * 32)
    with pytest.raises(ValueError):
        IssueReplicationOrderInstruction(_ORDER_ID, duplicate_provider, 1, 2)

    zero_target = _mutated_payload(b"\x02\x02\x00", b"\x02\x00\x00")
    with pytest.raises(ValueError):
        IssueReplicationOrderInstruction(_ORDER_ID, zero_target, 1, 2)

    invalid_deadline = _mutated_payload(
        (1_700_086_400).to_bytes(8, "little"),
        (1_700_000_000).to_bytes(8, "little"),
    )
    with pytest.raises(ValueError):
        IssueReplicationOrderInstruction(_ORDER_ID, invalid_deadline, 1, 2)


def test_replication_payloads_convert_to_native_instructions() -> None:
    issue = IssueReplicationOrderInstruction(
        _ORDER_ID,
        base64.b64encode(_FIXTURE).decode("ascii"),
        20,
        28,
    )
    complete = CompleteReplicationOrderInstruction(_ORDER_ID, _PROVIDER_ID, 27)
    expire = ExpireReplicationOrderInstruction(_ORDER_ID, 29)
    try:
        encoded = (
            issue.to_instruction().to_json(),
            complete.to_instruction().to_json(),
            expire.to_instruction().to_json(),
        )
    except RuntimeError as error:
        if "rebuild the extension" in str(error):
            pytest.skip("checked-in Python native extension is stale")
        raise
    assert "IssueReplicationOrder" in encoded[0]
    assert "provider_id" in encoded[1]
    assert "ExpireReplicationOrder" in encoded[2]
