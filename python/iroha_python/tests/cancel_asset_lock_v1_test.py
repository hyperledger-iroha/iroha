"""Strict bare ``CancelAssetLock`` V1 codec and appeal-finance profiles."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast

import pytest

import iroha_python.sorafs as sorafs_module
from iroha_python import (
    CANCEL_ASSET_LOCK_WIRE_ID_V1,
    CancelAssetLockV1,
    decode_cancel_asset_lock_v1,
    encode_cancel_asset_lock_v1,
    validate_appeal_finance_cancel_asset_lock,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]
_FIXTURE_ROOT = _REPO_ROOT / "fixtures" / "sorafs_manifest" / "appeal_finance"
_PROFILE_ROOT = _FIXTURE_ROOT.parent / "reference_sdk"
_REQUIRED_FIXTURE_NAMES = (
    "cancel_asset_lock_v1.json",
    "cancel_asset_lock_v1.to",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
    "negative/cancel_asset_lock_nested_escrow_id_v1.to",
    "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.to",
)
_FIXTURES = {name: (_FIXTURE_ROOT / name).read_bytes() for name in _REQUIRED_FIXTURE_NAMES}
_ESCROW_ID = "hash:73CCD4E0DD69AD434DB75056B600AA4F74C8FC5556B11BDC799DFDB7EA29851F#434B"
_CANONICAL_ARCHIVE_HEX = (
    "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000"
    "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc"
    "799dfdb7ea29851f0b0501000000140400000000"
)
_CRC64_MASK = (1 << 64) - 1
_CRC64_REFLECTED_POLYNOMIAL = 0xC96C_5795_D787_0F42


def _strict_json_object(payload: bytes) -> dict[str, Any]:
    decoded = json.loads(payload.decode("utf-8", errors="strict"))
    if type(decoded) is not dict:
        raise TypeError("fixture JSON must be an object")
    return decoded


def _crc64(payload: bytes) -> int:
    crc = _CRC64_MASK
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = (
                (crc >> 1) ^ _CRC64_REFLECTED_POLYNOMIAL
                if crc & 1
                else crc >> 1
            )
    return (crc ^ _CRC64_MASK) & _CRC64_MASK


def _archive_with_true_trailing_payload_byte(archive: bytes) -> bytes:
    malformed = bytearray(archive + b"\x00")
    payload = bytes(malformed[40:])
    malformed[23:31] = len(payload).to_bytes(8, "little")
    malformed[31:39] = _crc64(payload).to_bytes(8, "little")
    return bytes(malformed)


def _assert_outcome(
    outcome: dict[str, Any],
    *,
    status: str,
    code: str,
    category: str,
    label: str,
    generated_at: int,
) -> None:
    assert outcome["status"] == status
    assert outcome["code"] == code
    assert outcome["category"] == category
    assert outcome["version"] == 1
    assert outcome["generated_at"] == generated_at
    assert outcome["inputs"] == [{"kind": "cancel_asset_lock", "path": label}]
    assert f"sorafs.reference.code.{code}" in outcome["telemetry_tags"]


def test_all_eight_appeal_finance_cancel_asset_lock_fixtures_are_mandatory() -> None:
    assert tuple(_FIXTURES) == _REQUIRED_FIXTURE_NAMES
    assert len(_FIXTURES) == 8
    assert all(_FIXTURES.values())


def test_bare_cancel_asset_lock_v1_matches_the_exact_canonical_archive() -> None:
    bare = _strict_json_object(_FIXTURES["cancel_asset_lock_v1.json"])
    value = CancelAssetLockV1(**bare)
    archive = _FIXTURES["cancel_asset_lock_v1.to"]

    assert CANCEL_ASSET_LOCK_WIRE_ID_V1 == ("iroha_data_model::isi::escrow::CancelAssetLock")
    assert value.to_mapping() == {
        "escrow_id": _ESCROW_ID,
        "expected_remaining_amount": "20",
    }
    assert len(archive) == 85
    assert archive.hex() == _CANONICAL_ARCHIVE_HEX
    assert value.encode() == archive
    assert (
        encode_cancel_asset_lock_v1(
            value.escrow_id,
            value.expected_remaining_amount,
        )
        == archive
    )
    assert decode_cancel_asset_lock_v1(archive) == value


def test_bare_cancel_asset_lock_v1_rejects_all_shared_negative_fixtures() -> None:
    for name in (
        "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
        "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
        "negative/cancel_asset_lock_zero_expected_v1.json",
    ):
        with pytest.raises((TypeError, ValueError)):
            CancelAssetLockV1(**_strict_json_object(_FIXTURES[name]))

    for name in (
        "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
        "negative/cancel_asset_lock_nested_escrow_id_v1.to",
        "negative/cancel_asset_lock_zero_expected_v1.to",
    ):
        with pytest.raises(ValueError):
            decode_cancel_asset_lock_v1(_FIXTURES[name])


@pytest.mark.parametrize(
    "fields",
    [
        {"escrow_id": _ESCROW_ID},
        {"expected_remaining_amount": "20"},
        {
            "escrow_id": _ESCROW_ID,
            "expected_remaining_amount": "20",
            "legacy": {},
        },
    ],
)
def test_bare_cancel_asset_lock_v1_rejects_missing_and_extra_fields(
    fields: dict[str, Any],
) -> None:
    with pytest.raises(TypeError):
        cast(Any, CancelAssetLockV1)(**fields)


def test_bare_cancel_asset_lock_v1_encoder_requires_exactly_two_arguments() -> None:
    encode = cast(Any, encode_cancel_asset_lock_v1)
    with pytest.raises(TypeError):
        encode(_ESCROW_ID)
    with pytest.raises(TypeError):
        encode(_ESCROW_ID, "20", {})


@pytest.mark.parametrize(
    "escrow_id",
    [
        _ESCROW_ID[5:69],
        bytes.fromhex(_ESCROW_ID[5:69]),
        [_ESCROW_ID],
        {"Hash": _ESCROW_ID},
        _ESCROW_ID.lower(),
        f"{_ESCROW_ID[:-1]}0",
        "\ud800",
        "\udc00",
    ],
)
def test_bare_cancel_asset_lock_v1_rejects_escrow_aliases(
    escrow_id: Any,
) -> None:
    with pytest.raises((TypeError, ValueError)):
        encode_cancel_asset_lock_v1(escrow_id, "20")


@pytest.mark.parametrize(
    "quantity",
    [
        20,
        b"20",
        ["20"],
        {"Quantity": "20"},
        "",
        "0",
        "-1",
        "020",
        "+20",
        "20.0",
        "2e1",
        "\ud800",
        "\udc00",
    ],
)
def test_bare_cancel_asset_lock_v1_rejects_quantity_aliases(
    quantity: Any,
) -> None:
    with pytest.raises((TypeError, ValueError)):
        encode_cancel_asset_lock_v1(_ESCROW_ID, quantity)


def test_bare_cancel_asset_lock_v1_decoder_rejects_aliases_and_frame_substitution() -> None:
    canonical = _FIXTURES["cancel_asset_lock_v1.to"]
    for alias in (
        canonical.hex(),
        bytearray(canonical),
        memoryview(canonical),
        list(canonical),
        {"bytes": canonical},
    ):
        with pytest.raises(TypeError):
            decode_cancel_asset_lock_v1(cast(Any, alias))

    with pytest.raises(ValueError):
        decode_cancel_asset_lock_v1(canonical.hex().encode("ascii"))

    wrong_version = bytearray(canonical)
    wrong_version[4] = 1
    with pytest.raises(ValueError, match="magic or version"):
        decode_cancel_asset_lock_v1(bytes(wrong_version))

    wrong_schema = bytearray(canonical)
    wrong_schema[6] ^= 1
    with pytest.raises(ValueError, match="schema"):
        decode_cancel_asset_lock_v1(bytes(wrong_schema))

    compressed = bytearray(canonical)
    compressed[22] = 1
    with pytest.raises(ValueError, match="uncompressed"):
        decode_cancel_asset_lock_v1(bytes(compressed))

    wrong_flags = bytearray(canonical)
    wrong_flags[39] = 0
    with pytest.raises(ValueError, match="compact-length"):
        decode_cancel_asset_lock_v1(bytes(wrong_flags))

    padded = canonical[:40] + b"\x00" + canonical[40:]
    with pytest.raises(ValueError, match="unpadded"):
        decode_cancel_asset_lock_v1(padded)


def test_nested_escrow_id_and_true_trailing_bytes_are_independent_failures() -> None:
    nested = _FIXTURES["negative/cancel_asset_lock_nested_escrow_id_v1.to"]
    assert len(nested) == 86
    assert nested[40:42] == b"\x21\x20"
    with pytest.raises(ValueError):
        decode_cancel_asset_lock_v1(nested)

    trailing = _archive_with_true_trailing_payload_byte(
        _FIXTURES["cancel_asset_lock_v1.to"]
    )
    assert len(trailing) == 86
    assert int.from_bytes(trailing[23:31], "little") == 46
    with pytest.raises(ValueError, match="trailing bytes"):
        decode_cancel_asset_lock_v1(trailing)


def test_fixture_json_decoder_rejects_invalid_utf8() -> None:
    with pytest.raises(UnicodeDecodeError):
        _strict_json_object(b'{"\x80":1}')


def test_appeal_finance_validation_profiles_are_stable() -> None:
    profiles = (
        (
            "cancel_asset_lock_v1.to",
            "cancel_asset_lock_v1.to",
            41,
            "Ok",
            "SFS-OK-000",
            "validation",
        ),
        (
            "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "cancel_asset_lock_legacy_missing_expected_v1.to",
            42,
            "Error",
            "SFS-NORITO-001",
            "norito",
        ),
        (
            "negative/cancel_asset_lock_zero_expected_v1.to",
            "cancel_asset_lock_zero_expected_v1.to",
            43,
            "Error",
            "SFS-VAL-001",
            "validation",
        ),
    )
    for fixture_name, label, generated_at, status, code, category in profiles:
        outcome = validate_appeal_finance_cancel_asset_lock(
            _FIXTURES[fixture_name],
            label=label,
            generated_at_unix=generated_at,
        )
        _assert_outcome(
            outcome,
            status=status,
            code=code,
            category=category,
            label=label,
            generated_at=generated_at,
        )


def test_appeal_finance_profiles_match_the_signed_inventory_fixtures() -> None:
    profiles = (
        (
            "cancel_asset_lock_v1.to",
            "cancel_asset_lock_v1.to",
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json",
        ),
        (
            "negative/cancel_asset_lock_zero_expected_v1.to",
            "cancel_asset_lock_zero_expected_v1.to",
            "appeal_finance_cancel_asset_lock_zero_expected_negative_validation_outcome_v1.json",
        ),
    )
    for fixture_name, label, expected_name in profiles:
        expected_text = (_PROFILE_ROOT / expected_name).read_text(encoding="utf-8")
        expected = json.loads(expected_text)
        outcome = validate_appeal_finance_cancel_asset_lock(
            _FIXTURES[fixture_name],
            label=label,
            generated_at_unix=123,
        )
        assert outcome == expected
        assert json.dumps(outcome, indent=2) + "\n" == expected_text


def test_appeal_finance_validation_rejects_text_archive_aliases() -> None:
    canonical = _FIXTURES["cancel_asset_lock_v1.to"]
    for alias in (canonical.hex(), list(canonical)):
        with pytest.raises(TypeError):
            validate_appeal_finance_cancel_asset_lock(cast(Any, alias))


def test_appeal_finance_validation_fails_closed_without_native_symbol(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sorafs_module, "_crypto", object())

    with pytest.raises(
        RuntimeError,
        match=(
            r"requires native function "
            r"`sorafs_validate_appeal_finance_cancel_asset_lock_json`"
        ),
    ):
        sorafs_module.validate_appeal_finance_cancel_asset_lock(
            _FIXTURES["cancel_asset_lock_v1.to"],
            label="cancel_asset_lock_v1.to",
            generated_at_unix=123,
        )
