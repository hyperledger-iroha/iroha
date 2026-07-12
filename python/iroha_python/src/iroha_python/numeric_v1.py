"""Exact Kotodama Numeric V1 values and canonical pointer-ABI codecs.

The module deliberately accepts neither :class:`float` nor
``decimal.Decimal``.  Kotodama numeric values cross JSON boundaries as
canonical base-10 strings and use Python's arbitrary-precision ``int`` only
while enforcing the consensus-visible signed 512-bit domain.
"""

from __future__ import annotations

import hashlib
import hmac
import re
from dataclasses import dataclass
from types import MappingProxyType
from typing import Mapping, NoReturn, Union, cast

INT_MIN = -(1 << 511)
"""Smallest canonical Kotodama ``int`` and numeric mantissa."""

INT_MAX = (1 << 511) - 1
"""Largest canonical Kotodama ``int`` and numeric mantissa."""

MAX_SCALE = 28
"""Largest canonical ``decimal`` and ``quantity`` scale."""

MAX_MANTISSA_BYTES = 64
"""Maximum encoded two's-complement mantissa width."""

_MAX_INT_TEXT_BYTES = 155
_MAX_SIGNIFICANT_DIGITS = 154
_FRAME_HEADER_BYTES = 40
_ENVELOPE_HEADER_BYTES = 7
_HASH_BYTES = 32
_U64_MASK = (1 << 64) - 1
_CRC64_POLY = 0xC96C5795D7870F42
_FRAME_PREFIX = b"NRT0\x00\x00"
_CANONICAL_INT_RE = re.compile(r"-?(?:0|[1-9][0-9]*)\Z", re.ASCII)
_SCALED_RE = re.compile(r"(-?)(0|[1-9][0-9]*)(?:\.([0-9]+))?\Z", re.ASCII)


@dataclass(frozen=True)
class NumericV1Schema:
    """One schema-bound Numeric V1 pointer payload."""

    name: str
    schema_hash: bytes
    pointer_type: int
    scaled: bool


NUMERIC_V1_SCHEMAS: Mapping[str, NumericV1Schema] = MappingProxyType(
    {
        "int": NumericV1Schema(
            "iroha.numeric.IntValueV1",
            bytes.fromhex("07c039457363b9e1d36bbd31d93dec4a"),
            0x0011,
            False,
        ),
        "decimal": NumericV1Schema(
            "iroha.numeric.DecimalValueV1",
            bytes.fromhex("ba2ffed52e4d8ee16f17efefe1828524"),
            0x0012,
            True,
        ),
        "quantity": NumericV1Schema(
            "iroha.numeric.QuantityValueV1",
            bytes.fromhex("e4769984c81ce0e8b678f2eb06274ee3"),
            0x0013,
            True,
        ),
    }
)
"""Canonical schema metadata keyed by source type name."""


class NumericV1Error(ValueError):
    """Stable validation failure raised by the Numeric V1 codec."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _fail(code: str, message: str) -> NoReturn:
    raise NumericV1Error(code, message)


def _as_bytes(value: object, context: str) -> bytes:
    if type(value) is bytes:
        return cast(bytes, value)
    if type(value) is bytearray:
        return bytes(cast(bytearray, value))
    if type(value) is memoryview:
        return cast(memoryview, value).tobytes()
    raise TypeError(f"{context} must be bytes-like")


def _check_int_range(value: int) -> int:
    if value < INT_MIN or value > INT_MAX:
        _fail("mantissa_overflow", "numeric mantissa is outside the signed 512-bit domain")
    return value


def _checked_int(value: Union[int, str], context: str) -> int:
    if type(value) is int:
        return value
    if type(value) is str:
        if _CANONICAL_INT_RE.fullmatch(value) is None or value == "-0":
            _fail("invalid_text", f"{context} must use canonical base-10 syntax")
        if len(value.encode("ascii")) > _MAX_INT_TEXT_BYTES:
            _fail("mantissa_overflow", "integer text exceeds the signed 512-bit input bound")
        return int(value)
    raise TypeError(f"{context} must be an int or canonical integer string")


def _normalize_scaled(mantissa: Union[int, str], scale: int, *, quantity: bool) -> tuple[int, int]:
    if type(scale) is not int or scale < 0:
        _fail("invalid_scale", "numeric scale must be a non-negative integer")

    normalized_scale = scale
    if type(mantissa) is str:
        if _CANONICAL_INT_RE.fullmatch(mantissa) is None or mantissa == "-0":
            _fail("invalid_text", "mantissa must use canonical base-10 syntax")
        negative = mantissa.startswith("-")
        magnitude = mantissa[1:] if negative else mantissa
        if magnitude == "0":
            normalized_mantissa = 0
            normalized_scale = 0
        else:
            while normalized_scale > 0 and magnitude.endswith("0"):
                magnitude = magnitude[:-1]
                normalized_scale -= 1
            normalized_text = f"{'-' if negative else ''}{magnitude}"
            if normalized_scale > MAX_SCALE:
                _fail("invalid_scale", "canonical numeric scale exceeds 28")
            if len(normalized_text.encode("ascii")) > _MAX_INT_TEXT_BYTES:
                _fail(
                    "mantissa_overflow",
                    "numeric mantissa is outside the signed 512-bit domain",
                )
            normalized_mantissa = int(normalized_text)
    else:
        normalized_mantissa = _checked_int(mantissa, "mantissa")

    if normalized_mantissa == 0:
        normalized_scale = 0
    else:
        while normalized_scale > 0 and normalized_mantissa % 10 == 0:
            normalized_mantissa //= 10
            normalized_scale -= 1

    if normalized_scale > MAX_SCALE:
        _fail("invalid_scale", "canonical numeric scale exceeds 28")
    _check_int_range(normalized_mantissa)
    if quantity and normalized_mantissa < 0:
        _fail("negative_quantity", "quantity cannot be negative")
    return normalized_mantissa, normalized_scale


def _parse_scaled(value: object, *, quantity: bool) -> tuple[int, int]:
    if type(value) is not str:
        raise TypeError("decimal and quantity values must be strings")
    matched = _SCALED_RE.fullmatch(value)
    if matched is None or value == "-0":
        _fail("invalid_text", "numeric text is not canonical decimal syntax")

    sign, whole, fraction_value = matched.groups()
    fraction = fraction_value or ""
    raw_digits = f"{whole}{fraction}"
    first = 0
    while first < len(raw_digits) and raw_digits[first] == "0":
        first += 1
    if first == len(raw_digits):
        return _normalize_scaled(0, 0, quantity=quantity)

    end = len(raw_digits)
    scale = len(fraction)
    while scale > 0 and raw_digits[end - 1] == "0":
        end -= 1
        scale -= 1
    if scale > MAX_SCALE:
        _fail("invalid_scale", "canonical numeric scale exceeds 28")
    if end - first > _MAX_SIGNIFICANT_DIGITS:
        _fail("mantissa_overflow", "decimal mantissa exceeds the signed 512-bit input bound")
    mantissa = int(f"{sign}{raw_digits[first:end]}")
    return _normalize_scaled(mantissa, scale, quantity=quantity)


def _scaled_text(mantissa: int, scale: int) -> str:
    if scale == 0:
        return str(mantissa)
    negative = mantissa < 0
    digits = str(-mantissa if negative else mantissa)
    if len(digits) <= scale:
        digits = f"{'0' * (scale + 1 - len(digits))}{digits}"
    split = len(digits) - scale
    return f"{'-' if negative else ''}{digits[:split]}.{digits[split:]}"


@dataclass(frozen=True, init=False)
class KotodamaInt:
    """Lossless Kotodama V1 signed integer."""

    value: int

    def __init__(self, value: Union[int, str]) -> None:
        object.__setattr__(self, "value", _check_int_range(_checked_int(value, "int")))

    def __str__(self) -> str:
        return str(self.value)


@dataclass(frozen=True, init=False)
class KotodamaDecimal:
    """Lossless exact Kotodama V1 base-10 value."""

    mantissa: int
    scale: int

    def __init__(self, value: Union[int, str], scale: int | None = None) -> None:
        normalized = (
            _parse_scaled(value, quantity=False)
            if scale is None
            else _normalize_scaled(value, scale, quantity=False)
        )
        object.__setattr__(self, "mantissa", normalized[0])
        object.__setattr__(self, "scale", normalized[1])

    def __str__(self) -> str:
        return _scaled_text(self.mantissa, self.scale)


@dataclass(frozen=True, init=False)
class KotodamaQuantity:
    """Lossless nominal non-negative Kotodama V1 asset quantity."""

    mantissa: int
    scale: int

    def __init__(self, value: Union[int, str], scale: int | None = None) -> None:
        normalized = (
            _parse_scaled(value, quantity=True)
            if scale is None
            else _normalize_scaled(value, scale, quantity=True)
        )
        object.__setattr__(self, "mantissa", normalized[0])
        object.__setattr__(self, "scale", normalized[1])

    def __str__(self) -> str:
        return _scaled_text(self.mantissa, self.scale)


NumericValue = Union[KotodamaInt, KotodamaDecimal, KotodamaQuantity]


def _canonical_int_input(value: Union[KotodamaInt, int, str]) -> KotodamaInt:
    if type(value) is KotodamaInt:
        return cast(KotodamaInt, value)
    return KotodamaInt(cast(Union[int, str], value))


def _canonical_decimal_input(value: Union[KotodamaDecimal, str]) -> KotodamaDecimal:
    if type(value) is KotodamaDecimal:
        return cast(KotodamaDecimal, value)
    return KotodamaDecimal(cast(str, value))


def _canonical_quantity_input(value: Union[KotodamaQuantity, str]) -> KotodamaQuantity:
    if type(value) is KotodamaQuantity:
        return cast(KotodamaQuantity, value)
    return KotodamaQuantity(cast(str, value))


def _encode_twos_complement(value: int) -> bytes:
    _check_int_range(value)
    if value == 0:
        return b""
    if value > 0:
        encoded = value.to_bytes((value.bit_length() + 7) // 8, "little")
        if encoded[-1] & 0x80:
            encoded += b"\x00"
    else:
        width = 1
        while value < -(1 << (width * 8 - 1)):
            width += 1
        encoded = value.to_bytes(width, "little", signed=True)
    if len(encoded) > MAX_MANTISSA_BYTES:
        _fail("mantissa_overflow", "mantissa is too wide")
    return encoded


def _decode_twos_complement(encoded: bytes) -> int:
    if len(encoded) > MAX_MANTISSA_BYTES:
        _fail("mantissa_overflow", "mantissa is too wide")
    if not encoded:
        return 0
    last = encoded[-1]
    if len(encoded) == 1 and last == 0:
        _fail("noncanonical_mantissa", "zero must use an empty mantissa")
    if len(encoded) > 1:
        previous = encoded[-2]
        if (last == 0 and previous & 0x80 == 0) or (last == 0xFF and previous & 0x80 != 0):
            _fail("noncanonical_mantissa", "mantissa has redundant sign extension")
    return _check_int_range(int.from_bytes(encoded, "little", signed=True))


def _crc64_xz(data: bytes) -> int:
    crc = _U64_MASK
    for byte in data:
        index = (crc ^ byte) & 0xFF
        table_value = index
        for _ in range(8):
            table_value = (
                table_value >> 1 if table_value & 1 == 0 else (table_value >> 1) ^ _CRC64_POLY
            )
        crc = table_value ^ (crc >> 8)
    return (crc ^ _U64_MASK) & _U64_MASK


def _payload_hash(frame: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(frame, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def _body_for(kind: str, value: NumericValue) -> bytes:
    if kind == "int":
        assert isinstance(value, KotodamaInt)
        mantissa = value.value
        scale = None
    else:
        assert isinstance(value, (KotodamaDecimal, KotodamaQuantity))
        mantissa = value.mantissa
        scale = value.scale
    encoded = _encode_twos_complement(mantissa)
    body = len(encoded).to_bytes(4, "little") + encoded
    return body if scale is None else body + bytes((scale,))


def _frame_for(kind: str, value: NumericValue) -> bytes:
    schema = NUMERIC_V1_SCHEMAS[kind]
    body = _body_for(kind, value)
    return b"".join(
        (
            _FRAME_PREFIX,
            schema.schema_hash,
            b"\x00",
            len(body).to_bytes(8, "little"),
            _crc64_xz(body).to_bytes(8, "little"),
            b"\x00",
            body,
        )
    )


def _decode_frame(kind: str, input_value: object) -> NumericValue:
    schema = NUMERIC_V1_SCHEMAS[kind]
    frame = _as_bytes(input_value, "numeric frame")
    maximum = _FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (1 if schema.scaled else 0)
    if len(frame) < _FRAME_HEADER_BYTES:
        _fail("frame_too_short", "numeric frame is truncated")
    if len(frame) > maximum:
        _fail("frame_too_large", "numeric frame is oversized")
    if frame[:6] != _FRAME_PREFIX:
        _fail("invalid_header", "numeric frame has the wrong Norito magic or version")
    if not hmac.compare_digest(frame[6:22], schema.schema_hash):
        _fail("schema_mismatch", "numeric frame schema does not match its type")
    if frame[22] != 0:
        _fail("compression_not_allowed", "numeric frames cannot be compressed")
    if frame[39] != 0:
        _fail("layout_flags_not_allowed", "numeric frame flags must be zero")
    body_length = int.from_bytes(frame[23:31], "little")
    if body_length != len(frame) - _FRAME_HEADER_BYTES:
        _fail("length_mismatch", "numeric frame length is inconsistent")
    body = frame[_FRAME_HEADER_BYTES:]
    if int.from_bytes(frame[31:39], "little") != _crc64_xz(body):
        _fail("checksum_mismatch", "numeric frame checksum failed")
    if len(body) < 4:
        _fail("length_mismatch", "numeric body has no mantissa length")
    mantissa_length = int.from_bytes(body[:4], "little")
    expected = 4 + mantissa_length + (1 if schema.scaled else 0)
    if mantissa_length > MAX_MANTISSA_BYTES:
        _fail("mantissa_overflow", "numeric mantissa is too wide")
    if expected != len(body):
        _fail("length_mismatch", "numeric body length is inconsistent")
    mantissa = _decode_twos_complement(body[4 : 4 + mantissa_length])
    if not schema.scaled:
        return KotodamaInt(mantissa)
    scale = body[-1]
    if scale > MAX_SCALE:
        _fail("invalid_scale", "numeric scale exceeds 28")
    if (mantissa == 0 and scale != 0) or (scale > 0 and mantissa % 10 == 0):
        _fail("noncanonical_decimal", "numeric value has a noncanonical scale")
    if kind == "quantity":
        return KotodamaQuantity(mantissa, scale)
    return KotodamaDecimal(mantissa, scale)


def _envelope_for(kind: str, value: NumericValue) -> bytes:
    schema = NUMERIC_V1_SCHEMAS[kind]
    frame = _frame_for(kind, value)
    return b"".join(
        (
            schema.pointer_type.to_bytes(2, "big"),
            b"\x01",
            len(frame).to_bytes(4, "big"),
            frame,
            _payload_hash(frame),
        )
    )


def _decode_envelope(kind: str, input_value: object) -> NumericValue:
    schema = NUMERIC_V1_SCHEMAS[kind]
    envelope = _as_bytes(input_value, "numeric pointer envelope")
    if len(envelope) < _ENVELOPE_HEADER_BYTES:
        _fail("truncated_envelope", "numeric envelope is truncated")
    pointer_type = int.from_bytes(envelope[:2], "big")
    if pointer_type == 0x0010:
        _fail("type_not_allowed", "retired Amount pointer type is permanently reserved")
    known_allowed = 0x0001 <= pointer_type <= 0x000F or 0x0011 <= pointer_type <= 0x0013
    if not known_allowed:
        _fail("unknown_type", "numeric envelope has an unknown pointer type")
    if pointer_type != schema.pointer_type:
        _fail("wrong_type", "numeric envelope type does not match")
    if envelope[2] != 1:
        _fail("invalid_envelope_version", "numeric envelope version must be 1")
    frame_length = int.from_bytes(envelope[3:7], "big")
    maximum = _FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (1 if schema.scaled else 0)
    if frame_length > maximum:
        _fail("oversized_length", "numeric envelope declares an oversized frame")
    if _ENVELOPE_HEADER_BYTES + frame_length + _HASH_BYTES != len(envelope):
        _fail("truncated_envelope", "numeric envelope length is inconsistent")
    frame_end = _ENVELOPE_HEADER_BYTES + frame_length
    frame = envelope[_ENVELOPE_HEADER_BYTES:frame_end]
    if not hmac.compare_digest(_payload_hash(frame), envelope[frame_end:]):
        _fail("payload_hash_mismatch", "numeric envelope payload hash failed")
    return _decode_frame(kind, frame)


class NumericV1Codec:
    """Canonical JSON, Norito-frame, and pointer-envelope Numeric V1 codec."""

    int_min = INT_MIN
    int_max = INT_MAX
    max_scale = MAX_SCALE
    max_mantissa_bytes = MAX_MANTISSA_BYTES
    schemas = NUMERIC_V1_SCHEMAS

    @staticmethod
    def encode_int_json(value: Union[KotodamaInt, int, str]) -> str:
        return str(_canonical_int_input(value))

    @staticmethod
    def encode_decimal_json(value: Union[KotodamaDecimal, str]) -> str:
        return str(_canonical_decimal_input(value))

    @staticmethod
    def encode_quantity_json(value: Union[KotodamaQuantity, str]) -> str:
        return str(_canonical_quantity_input(value))

    @staticmethod
    def decode_int_json(value: object) -> KotodamaInt:
        if type(value) is not str:
            raise TypeError("int JSON must be a string")
        return KotodamaInt(value)

    @staticmethod
    def decode_decimal_json(value: object) -> KotodamaDecimal:
        if type(value) is not str:
            raise TypeError("decimal JSON must be a string")
        decoded = KotodamaDecimal(value)
        if str(decoded) != value:
            _fail("invalid_text", "decimal JSON must use canonical spelling")
        return decoded

    @staticmethod
    def decode_quantity_json(value: object) -> KotodamaQuantity:
        if type(value) is not str:
            raise TypeError("quantity JSON must be a string")
        decoded = KotodamaQuantity(value)
        if str(decoded) != value:
            _fail("invalid_text", "quantity JSON must use canonical spelling")
        return decoded

    @staticmethod
    def encode_int_frame(value: Union[KotodamaInt, int, str]) -> bytes:
        return _frame_for("int", _canonical_int_input(value))

    @staticmethod
    def encode_decimal_frame(value: Union[KotodamaDecimal, str]) -> bytes:
        return _frame_for("decimal", _canonical_decimal_input(value))

    @staticmethod
    def encode_quantity_frame(value: Union[KotodamaQuantity, str]) -> bytes:
        return _frame_for("quantity", _canonical_quantity_input(value))

    @staticmethod
    def decode_int_frame(value: object) -> KotodamaInt:
        decoded = _decode_frame("int", value)
        assert isinstance(decoded, KotodamaInt)
        return decoded

    @staticmethod
    def decode_decimal_frame(value: object) -> KotodamaDecimal:
        decoded = _decode_frame("decimal", value)
        assert isinstance(decoded, KotodamaDecimal)
        return decoded

    @staticmethod
    def decode_quantity_frame(value: object) -> KotodamaQuantity:
        decoded = _decode_frame("quantity", value)
        assert isinstance(decoded, KotodamaQuantity)
        return decoded

    @staticmethod
    def encode_int_envelope(value: Union[KotodamaInt, int, str]) -> bytes:
        return _envelope_for("int", _canonical_int_input(value))

    @staticmethod
    def encode_decimal_envelope(value: Union[KotodamaDecimal, str]) -> bytes:
        return _envelope_for("decimal", _canonical_decimal_input(value))

    @staticmethod
    def encode_quantity_envelope(value: Union[KotodamaQuantity, str]) -> bytes:
        return _envelope_for("quantity", _canonical_quantity_input(value))

    @staticmethod
    def decode_int_envelope(value: object) -> KotodamaInt:
        decoded = _decode_envelope("int", value)
        assert isinstance(decoded, KotodamaInt)
        return decoded

    @staticmethod
    def decode_decimal_envelope(value: object) -> KotodamaDecimal:
        decoded = _decode_envelope("decimal", value)
        assert isinstance(decoded, KotodamaDecimal)
        return decoded

    @staticmethod
    def decode_quantity_envelope(value: object) -> KotodamaQuantity:
        decoded = _decode_envelope("quantity", value)
        assert isinstance(decoded, KotodamaQuantity)
        return decoded


__all__ = [
    "INT_MAX",
    "INT_MIN",
    "MAX_MANTISSA_BYTES",
    "MAX_SCALE",
    "NUMERIC_V1_SCHEMAS",
    "KotodamaDecimal",
    "KotodamaInt",
    "KotodamaQuantity",
    "NumericV1Codec",
    "NumericV1Error",
    "NumericV1Schema",
]
