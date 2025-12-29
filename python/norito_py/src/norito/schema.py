# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Helpers for computing Norito schema hashes."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

FNV64_OFFSET = 0xCBF29CE484222325
FNV64_PRIME = 0x100000001B3


@dataclass(frozen=True)
class SchemaDescriptor:
    """Encapsulates information required to compute a Norito schema hash."""

    canonical_path: str | None = None
    structural_schema: Any | None = None

    def __post_init__(self) -> None:
        if self.canonical_path is None and self.structural_schema is None:
            raise ValueError("SchemaDescriptor requires a canonical path or structural schema")
        if self.canonical_path is not None and self.structural_schema is not None:
            raise ValueError("SchemaDescriptor cannot be constructed with both canonical path and structural schema")

    @classmethod
    def type_name(cls, canonical_path: str) -> "SchemaDescriptor":
        """Create a descriptor backed by a canonical dotted type path."""

        return cls(canonical_path=canonical_path)

    @classmethod
    def structural(cls, schema: Any) -> "SchemaDescriptor":
        """Create a descriptor from a structural schema description.

        The *schema* argument must be composed of dictionaries (with string keys),
        lists/tuples, strings, booleans, integers, or ``None``. Floating-point values
        are serialized using the same rules as the Rust Norito JSON writer. This
        mirrors ``iroha_schema`` output so hashes computed here match Rust goldens.
        """

        return cls(structural_schema=schema)

    def hash_bytes(self) -> bytes:
        """Return the 16-byte schema hash used in Norito headers."""

        if self.structural_schema is not None:
            payload = _canonical_json(self.structural_schema).encode("utf-8")
        else:
            assert self.canonical_path is not None
            payload = self.canonical_path.encode("utf-8")
        value = fnv1a_64(payload)
        return value.to_bytes(8, "little") * 2


def fnv1a_64(data: bytes) -> int:
    """Compute the FNV-1a 64-bit hash for *data*."""

    hash_value = FNV64_OFFSET
    for byte in data:
        hash_value ^= byte
        hash_value = (hash_value * FNV64_PRIME) & 0xFFFFFFFFFFFFFFFF
    return hash_value


def _canonical_json(value: Any) -> str:
    builder: list[str] = []

    def append(text: str) -> None:
        builder.append(text)

    def encode(obj: Any) -> None:
        if obj is None:
            append("null")
        elif isinstance(obj, bool):
            append("true" if obj else "false")
        elif isinstance(obj, int) and not isinstance(obj, bool):
            append(str(obj))
        elif isinstance(obj, float):
            append(_encode_float(obj))
        elif isinstance(obj, str):
            append(_encode_json_string(obj))
        elif isinstance(obj, Mapping):
            append("{")
            first = True
            for key in sorted(obj.keys()):
                if not isinstance(key, str):
                    raise TypeError("Structural schema keys must be strings")
                if not first:
                    append(",")
                append(_encode_json_string(key))
                append(":")
                encode(obj[key])
                first = False
            append("}")
        elif isinstance(obj, Sequence) and not isinstance(obj, (str, bytes, bytearray)):
            append("[")
            for idx, item in enumerate(obj):
                if idx:
                    append(",")
                encode(item)
            append("]")
        else:
            raise TypeError(f"Unsupported value in structural schema: {type(obj)!r}")

    encode(value)
    return "".join(builder)


def _encode_float(value: float) -> str:
    import math

    if math.isnan(value):
        return "NaN"
    if math.isinf(value):
        return "inf" if value > 0 else "-inf"
    if value.is_integer() and abs(value) <= 9_007_199_254_740_992.0:
        return f"{value:.1f}"
    # Match Rust's debug representation for other cases
    return format(value, ".17g")


def _encode_json_string(value: str) -> str:
    # Match Rust's escaping rules (see `write_json_string_charwise` in Rust impl)
    out: list[str] = ['"']
    for ch in value:
        code = ord(ch)
        if ch == '"':
            out.append("\\\"")
        elif ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\b":
            out.append("\\b")
        elif ch == "\f":
            out.append("\\f")
        elif ch == "\u2028":
            out.append("\\u2028")
        elif ch == "\u2029":
            out.append("\\u2029")
        elif code < 0x20:
            out.append(f"\\u00{code:02X}")
        elif code >= 0x10000:
            # Encode as surrogate pair to mirror Rust implementation
            code -= 0x10000
            hi = 0xD800 + (code >> 10)
            lo = 0xDC00 + (code & 0x3FF)
            out.append(f"\\u{hi:04X}")
            out.append(f"\\u{lo:04X}")
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


__all__ = ["SchemaDescriptor", "fnv1a_64"]
