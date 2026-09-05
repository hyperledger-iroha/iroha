"""Canonical Rust-generated KAGEMUSHA three-message fixture parity."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from kagemusha_test import Kagemusha


_FIXTURE_PATH = (
    Path(__file__).resolve().parents[3] / "fixtures" / "offline" / "kagemusha_v1.json"
)


def _raw(section: dict[str, object]) -> bytes:
    return bytes.fromhex(str(section["norito_hex"]))


def _assert_canonical_section(
    section: dict[str, object],
    kind: str,
    value: object,
    encoder: object,
    *bindings: object,
) -> None:
    expected = _raw(section)
    assert callable(encoder)
    assert encoder(value, *bindings) == expected
    assert hashlib.sha256(expected).hexdigest() == section["sha256"]
    assert Kagemusha.encode_text(kind, expected) == section["kgm1"]
    assert Kagemusha.decode_text(kind, str(section["kgm1"])) == expected
    assert len(expected) == section["raw_bytes"]


def test_rust_generated_three_message_fixture_is_byte_identical() -> None:
    fixture = json.loads(_FIXTURE_PATH.read_text(encoding="utf-8"))
    assert fixture["fixture_version"] == 1
    assert fixture["protocol"] == "KAGEMUSHA"
    assert fixture["text_prefix"] == "kgm1:"
    assert fixture["ipm1_message_order"] == [
        {"kind": "request", "tag": 1},
        {"kind": "payment", "tag": 2},
        {"kind": "acknowledgement", "tag": 3},
    ]
    assert "acceptance_intent" not in fixture
    assert "acceptance_ticket" not in fixture

    request_raw = _raw(fixture["payment_request"])
    request = Kagemusha.decode_payment_request(request_raw)
    payment_raw = _raw(fixture["payment"])
    payment = Kagemusha.decode_payment(payment_raw, request)
    acknowledgement_raw = _raw(fixture["acknowledgement"])
    acknowledgement = Kagemusha.decode_acknowledgement(
        acknowledgement_raw, request, payment
    )

    _assert_canonical_section(
        fixture["payment_request"],
        "request",
        request,
        Kagemusha.encode_payment_request,
    )
    _assert_canonical_section(
        fixture["payment"],
        "payment",
        payment,
        Kagemusha.encode_payment,
        request,
    )
    _assert_canonical_section(
        fixture["acknowledgement"],
        "acknowledgement",
        acknowledgement,
        Kagemusha.encode_acknowledgement,
        request,
        payment,
    )

    expected_raw = len(request_raw) + len(payment_raw) + len(acknowledgement_raw)
    assert Kagemusha.validate_complete_exchange(
        request, payment, acknowledgement
    ) == expected_raw
    if "complete_three_message" in fixture:
        assert fixture["complete_three_message"]["raw_bytes"] == expected_raw
