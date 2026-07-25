"""Native-extension-free normalization for exact Numeric V1 quantities."""

from __future__ import annotations

from decimal import Decimal
from typing import Union

from .numeric_v1 import KotodamaQuantity, NumericV1Codec

QuantityLike = Union[KotodamaQuantity, str, int, Decimal]

_U128_MAX = (1 << 128) - 1
_MAX_CANONICAL_QUANTITY_TEXT_LENGTH = 155


def _quantity_from_decimal(value: Decimal) -> KotodamaQuantity:
    """Convert a finite ``Decimal`` without context rounding or exponent expansion."""

    if not value.is_finite():
        raise ValueError("quantity must be a finite decimal value")
    if value.is_zero():
        return KotodamaQuantity(0, 0)

    parts = value.as_tuple()
    digits = list(parts.digits)
    exponent = int(parts.exponent)
    while exponent < 0 and digits[-1] == 0:
        digits.pop()
        exponent += 1

    significant_digits = len(digits) + max(exponent, 0)
    if significant_digits > 154:
        raise ValueError("quantity mantissa is outside the signed 512-bit domain")
    if exponent < -28:
        raise ValueError("canonical quantity scale exceeds 28")

    mantissa = "".join(str(digit) for digit in digits)
    if exponent > 0:
        mantissa += "0" * exponent
    if parts.sign:
        mantissa = f"-{mantissa}"
    return KotodamaQuantity(mantissa, max(-exponent, 0))


def _normalize_quantity(quantity: QuantityLike) -> str:
    """Return one exact, canonical, non-negative V1 asset quantity.

    Strings are already a wire-facing representation and therefore must be
    canonical. Python ``int`` and ``Decimal`` inputs are lossless host values;
    unlike ``float``, they can be checked without first rounding through a
    host floating-point representation.
    """

    if type(quantity) is KotodamaQuantity:
        return str(quantity)
    if type(quantity) is str:
        if len(quantity) > _MAX_CANONICAL_QUANTITY_TEXT_LENGTH:
            raise ValueError("quantity text exceeds the canonical V1 bound")
        return str(NumericV1Codec.decode_quantity_json(quantity))
    if type(quantity) is int:
        return str(KotodamaQuantity(quantity, 0))
    if type(quantity) is Decimal:
        return str(_quantity_from_decimal(quantity))
    raise TypeError("quantity must be KotodamaQuantity, canonical string, int, or Decimal")


def _normalize_u128_quantity(quantity: QuantityLike, context: str) -> str:
    try:
        value = Decimal(_normalize_quantity(quantity))
    except TypeError as exc:
        raise TypeError(
            f"{context} must be a non-negative whole number within u128: {exc}"
        ) from exc
    except ValueError as exc:
        raise ValueError(
            f"{context} must be a non-negative whole number within u128: {exc}"
        ) from exc
    if value < 0 or value != value.to_integral_value() or value > _U128_MAX:
        raise ValueError(f"{context} must be a non-negative whole number within u128")
    return str(int(value))


def _normalize_positive_quantity(quantity: QuantityLike, context: str) -> str:
    try:
        normalized = _normalize_quantity(quantity)
    except TypeError as exc:
        raise TypeError(
            f"{context} must be positive and use a finite canonical quantity: {exc}"
        ) from exc
    except ValueError as exc:
        raise ValueError(
            f"{context} must be positive and use a finite canonical quantity: {exc}"
        ) from exc
    if Decimal(normalized) <= 0:
        raise ValueError(f"{context} must be positive")
    return normalized
