# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""IEEE-754 helpers that reproduce the Rust reference's float semantics."""

from __future__ import annotations

import math
import struct

_F64 = struct.Struct("<d")
_I64 = struct.Struct("<q")

NAN = float("nan")
#: ``f64::MAX``.
F64_MAX = 1.7976931348623157e308


def total_key(value: float) -> int:
    """Integer key that orders floats exactly like Rust's ``f64::total_cmp``."""
    bits = _I64.unpack(_F64.pack(value))[0]
    if bits < 0:
        bits ^= 0x7FFFFFFFFFFFFFFF
    return bits


def ieee_div(numerator: float, denominator: float) -> float:
    """``numerator / denominator`` with IEEE semantics for a zero denominator.

    Python raises on float division by zero; Rust yields an infinity or NaN.
    """
    if denominator != 0.0:
        return numerator / denominator
    if numerator != numerator or numerator == 0.0:
        return NAN
    sign = math.copysign(1.0, numerator) * math.copysign(1.0, denominator)
    return math.copysign(math.inf, sign)


def round_half_away(value: float) -> float:
    """Rust's ``f64::round`` for finite non-negative values (ties away from zero)."""
    floor = math.floor(value)
    return floor + 1.0 if value - floor >= 0.5 else float(floor)
