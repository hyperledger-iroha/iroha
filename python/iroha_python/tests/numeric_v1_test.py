"""Cross-SDK and adversarial tests for the exact Kotodama Numeric V1 codec."""

from __future__ import annotations

import json
from decimal import Decimal
from functools import partial
from pathlib import Path
from typing import Any, Callable, cast

import pytest

from iroha_python import (
    INT_MAX,
    INT_MIN,
    KotodamaDecimal,
    KotodamaInt,
    KotodamaQuantity,
    NumericV1Codec,
    NumericV1Error,
)


def _fixture() -> dict[str, Any]:
    root = Path(__file__).resolve().parents[3]
    return json.loads((root / "fixtures" / "numeric_v1_golden.json").read_text("utf-8"))


def _decoder(kind: str, input_kind: str) -> Callable[[object], object]:
    suffix = "frame" if input_kind == "frame" else "envelope"
    return getattr(NumericV1Codec, f"decode_{kind}_{suffix}")


def _decode_json(kind: str, value: str) -> object:
    return getattr(NumericV1Codec, f"decode_{kind}_json")(value)


def _encode(kind: str, output_kind: str, value: object) -> bytes:
    return getattr(NumericV1Codec, f"encode_{kind}_{output_kind}")(value)


def _assert_code(code: str, operation: Callable[[], object]) -> None:
    with pytest.raises(NumericV1Error) as raised:
        operation()
    assert raised.value.code == code


def test_exact_values_canonicalize_without_lossy_host_numbers() -> None:
    assert str(KotodamaInt("-129")) == "-129"
    assert str(KotodamaDecimal("1.2300")) == "1.23"
    assert str(KotodamaDecimal("0.000")) == "0"
    assert str(KotodamaQuantity("12.50")) == "12.5"
    _assert_code("negative_quantity", lambda: KotodamaQuantity("-0.1"))
    _assert_code("mantissa_overflow", lambda: KotodamaQuantity("-" + "9" * 154))
    assert str(KotodamaDecimal("1." + "0" * 10_000)) == "1"
    assert str(KotodamaDecimal(INT_MAX * 10, 1)) == str(INT_MAX)
    _assert_code("mantissa_overflow", lambda: KotodamaDecimal(f"{INT_MAX}.1"))
    _assert_code("invalid_scale", lambda: KotodamaDecimal("0.00000000000000000000000000001"))
    _assert_code("invalid_scale", lambda: KotodamaDecimal("1" * 10_000, 29))

    for lossy in (1.0, Decimal("1"), True):
        with pytest.raises(TypeError):
            KotodamaInt(cast(Any, lossy))
        with pytest.raises(TypeError):
            KotodamaDecimal(cast(Any, lossy))
        with pytest.raises(TypeError):
            KotodamaQuantity(cast(Any, lossy))


def test_json_boundary_accepts_only_canonical_strings() -> None:
    assert str(NumericV1Codec.decode_decimal_json("1.23")) == "1.23"
    assert str(NumericV1Codec.decode_quantity_json("0")) == "0"
    for alternate in ("+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0"):
        _assert_code(
            "invalid_text",
            partial(NumericV1Codec.decode_decimal_json, alternate),
        )
    for alternate in ("+1", "01", "-0", "1.0", "1e0"):
        _assert_code(
            "invalid_text",
            partial(NumericV1Codec.decode_int_json, alternate),
        )
    _assert_code("invalid_text", lambda: NumericV1Codec.decode_quantity_json("1.0"))
    _assert_code("negative_quantity", lambda: NumericV1Codec.decode_quantity_json("-1"))
    for decoder in (
        NumericV1Codec.decode_int_json,
        NumericV1Codec.decode_decimal_json,
        NumericV1Codec.decode_quantity_json,
    ):
        for non_string in (0, 1.0, Decimal("1"), True, None):
            with pytest.raises(TypeError):
                decoder(non_string)


def test_hostile_builtin_subclasses_are_rejected_before_user_hooks() -> None:
    class HostileStr(str):
        def encode(self, *_args: object, **_kwargs: object) -> bytes:
            raise AssertionError("hostile str.encode executed")

        def __str__(self) -> str:
            raise AssertionError("hostile str.__str__ executed")

    class HostileInt(int):
        def __lt__(self, _other: object) -> bool:
            raise AssertionError("hostile int comparison executed")

        def __int__(self) -> int:
            raise AssertionError("hostile int conversion executed")

    class HostileBytes(bytes):
        def __bytes__(self) -> bytes:
            raise AssertionError("hostile bytes conversion executed")

    for value in (HostileStr("1"), HostileInt(1)):
        for constructor in (KotodamaInt, KotodamaDecimal, KotodamaQuantity):
            with pytest.raises(TypeError):
                constructor(cast(Any, value))

    for decoder in (
        NumericV1Codec.decode_int_json,
        NumericV1Codec.decode_decimal_json,
        NumericV1Codec.decode_quantity_json,
    ):
        with pytest.raises(TypeError):
            decoder(HostileStr("1"))

    envelope = NumericV1Codec.encode_int_envelope(1)
    with pytest.raises(TypeError):
        NumericV1Codec.decode_int_envelope(HostileBytes(envelope))

    class HostileKotodamaInt(KotodamaInt):
        def __str__(self) -> str:
            raise AssertionError("hostile numeric __str__ executed")

    with pytest.raises(TypeError):
        NumericV1Codec.encode_int_frame(HostileKotodamaInt(1))


def test_signed_512_bit_endpoints_and_neighbors() -> None:
    for value in (INT_MIN, INT_MAX):
        integer = KotodamaInt(value)
        frame = NumericV1Codec.encode_int_frame(integer)
        assert len(frame) == 108
        assert NumericV1Codec.decode_int_frame(frame) == integer
        assert (
            NumericV1Codec.decode_int_envelope(NumericV1Codec.encode_int_envelope(integer))
            == integer
        )
    _assert_code("mantissa_overflow", lambda: KotodamaInt(INT_MIN - 1))
    _assert_code("mantissa_overflow", lambda: KotodamaInt(INT_MAX + 1))
    _assert_code("mantissa_overflow", lambda: KotodamaInt("1" * 10_000))
    _assert_code("invalid_text", lambda: KotodamaInt("x" * 10_000))
    _assert_code("mantissa_overflow", lambda: KotodamaDecimal("1" * 10_000))


def test_authenticated_inputs_reject_truncation_and_tampering() -> None:
    frame = NumericV1Codec.encode_int_frame(KotodamaInt("128"))
    assert NumericV1Codec.decode_int_frame(memoryview(frame)) == KotodamaInt("128")
    for length in range(len(frame)):
        with pytest.raises(NumericV1Error):
            NumericV1Codec.decode_int_frame(frame[:length])

    bad_checksum = bytearray(frame)
    bad_checksum[-1] ^= 1
    _assert_code("checksum_mismatch", lambda: NumericV1Codec.decode_int_frame(bad_checksum))

    bad_hash = bytearray(NumericV1Codec.encode_int_envelope(KotodamaInt("1")))
    bad_hash[-1] ^= 1
    _assert_code("payload_hash_mismatch", lambda: NumericV1Codec.decode_int_envelope(bad_hash))

    retired = bytearray(NumericV1Codec.encode_int_envelope(KotodamaInt("1")))
    retired[:3] = b"\x00\x10\x02"
    _assert_code("type_not_allowed", lambda: NumericV1Codec.decode_int_envelope(retired))

    known_wrong = bytearray(NumericV1Codec.encode_int_envelope(KotodamaInt("1")))
    known_wrong[:3] = b"\x00\x01\x02"
    _assert_code("wrong_type", lambda: NumericV1Codec.decode_int_envelope(known_wrong))

    unknown = bytearray(NumericV1Codec.encode_int_envelope(KotodamaInt("1")))
    unknown[:3] = b"\x00\x14\x02"
    _assert_code("unknown_type", lambda: NumericV1Codec.decode_int_envelope(unknown))


def test_consumes_every_rust_authored_shared_golden_vector() -> None:
    fixture = _fixture()
    assert fixture["format"] == "iroha.numeric.v1"
    assert fixture["signed_bits"] == 512
    assert fixture["maximum_scale"] == 28

    for vector in fixture["text"]:
        constructor = KotodamaDecimal if vector["kind"] == "decimal" else KotodamaQuantity
        assert str(constructor(vector["input"])) == vector["canonical"], vector["id"]

    for vector in fixture["valid"]:
        kind = vector["kind"]
        value = _decode_json(kind, vector["canonical"])
        frame = _encode(kind, "frame", value)
        envelope = _encode(kind, "envelope", value)
        assert frame[40:].hex() == vector["body_hex"], vector["id"]
        assert frame.hex() == vector["frame_hex"], vector["id"]
        assert envelope.hex() == vector["envelope_hex"], vector["id"]
        assert (
            str(_decoder(kind, "frame")(bytes.fromhex(vector["frame_hex"]))) == vector["canonical"]
        )
        assert (
            str(_decoder(kind, "envelope")(bytes.fromhex(vector["envelope_hex"])))
            == vector["canonical"]
        )

    for vector in fixture["invalid"]:
        decode = _decoder(vector["decode_as"], vector["input"])
        _assert_code(
            vector["expected"],
            partial(decode, bytes.fromhex(vector["hex"])),
        )
