"""Canonical Rust-generated operation-16 public-body parity, not hardware qualification."""

from __future__ import annotations

import json
from pathlib import Path

from kagemusha_test import Kagemusha

_FIXTURE_PATH = (
    Path(__file__).resolve().parents[3]
    / "fixtures"
    / "offline"
    / "kagemusha_device_mint_stage_v1.json"
)


def _wire(section: dict[str, object]) -> bytes:
    raw = bytes.fromhex(str(section["hex"]))
    assert len(raw) == section["raw_bytes"]
    assert raw.hex() == section["hex"]
    return raw


def test_operation_16_rust_fixture_is_byte_identical() -> None:
    fixture = json.loads(_FIXTURE_PATH.read_text(encoding="utf-8"))
    assert fixture["fixture_version"] == 1
    assert fixture["protocol"] == "KAGEMUSHA"
    assert fixture["operation"] == 16
    assert fixture["structural_only"] is True
    device_model = "iroha_data_model::kagemusha::kagemusha_device_v1::"
    assert fixture["command"]["schema"] == f"{device_model}KagemushaDeviceMintStageCommandV1"
    assert fixture["command"]["alignment"] == 8

    authorization_bytes = _wire(fixture["authorization"])
    authorization = Kagemusha.decode_mint_authorization(authorization_bytes)
    assert Kagemusha.encode_mint_authorization(authorization) == authorization_bytes
    credit_bytes = _wire(fixture["mint_credit"])
    credit = Kagemusha.decode_mint_credit(credit_bytes, authorization)
    assert Kagemusha.encode_mint_credit(credit, authorization) == credit_bytes
    assert credit.statement.lifecycle.credit_id.hex() == fixture["credit_id_hex"]

    command_bytes = _wire(fixture["command"])
    command = Kagemusha.decode_device_mint_stage_command_shape_exact(command_bytes)
    assert command.canonical_authorization == authorization_bytes
    assert command.canonical_mint_credit == credit_bytes
    assert Kagemusha.encode_device_mint_stage_command_shape(command) == command_bytes
    assert (
        Kagemusha.encode_device_mint_stage_command_shape(authorization_bytes, credit_bytes)
        == command_bytes
    )

    for section_name, disposition in (("staged_result", 0), ("exact_duplicate_result", 1)):
        section = fixture[section_name]
        assert section["schema"] == f"{device_model}KagemushaDeviceMintStageResultV1"
        assert section["alignment"] == 2
        result_bytes = _wire(section)
        result = Kagemusha.decode_device_mint_stage_result_shape_exact(result_bytes, command)
        assert result.disposition == disposition
        assert result.credit_id.hex() == fixture["credit_id_hex"]
        assert Kagemusha.encode_device_mint_stage_result_shape(result, command) == result_bytes
