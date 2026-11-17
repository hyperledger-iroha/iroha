# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Core Norito encoding/decoding logic."""

from __future__ import annotations

from dataclasses import dataclass, field
from struct import pack, unpack
from typing import (
    Any,
    Callable,
    Dict,
    Generic,
    Iterable,
    List,
    Mapping,
    Optional,
    Protocol,
    Tuple,
    TypeVar,
)

from . import header
from .compression import (
    compress_zstd,
    decompress_zstd,
    resolve_zstd_level,
)
from .crc64 import crc64
from .errors import DecodeError, NoritoError, UnsupportedCompressionError
from .header import WireLayout
from .result import Err, Ok
from .schema import SchemaDescriptor
from .varint import VarintError, decode_varint, encode_varint

T = TypeVar("T")
E = TypeVar("E")

_decode_flags_stack: List[int] = []
_decode_root_stack: List[bytes] = []
_decode_context_stack: List[int] = []

_DYNAMIC_FLAGS_MASK = 0


class TypeAdapter(Protocol[T]):
    """Protocol implemented by all type adapters."""

    def encode(self, encoder: "NoritoEncoder", value: T) -> None:
        ...

    def decode(self, decoder: "NoritoDecoder") -> T:
        ...

    def fixed_size(self) -> Optional[int]:
        """Return current-layout sizing metadata, not a dense field length."""
        ...

    def is_self_delimiting(self) -> bool:
        """Return True if the adapter carries its own length markers."""
        ...


class _RootGuard:
    __slots__ = ("_owned", "_context_active", "_flags")

    def __init__(self, payload: bytes, flags: Optional[int] = None) -> None:
        self._owned = False
        self._context_active = False
        self._flags = flags
        if not _decode_root_stack:
            _decode_root_stack.append(bytes(payload))
            self._owned = True

    def release(self) -> None:
        if self._owned:
            _decode_root_stack.pop()
            self._owned = False
        if self._context_active:
            _decode_context_stack.pop()
            self._context_active = False

    def __enter__(self) -> "_RootGuard":
        if self._flags is not None:
            _decode_context_stack.append(self._flags & 0xFF)
            self._context_active = True
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.release()


def payload_root_bytes() -> Optional[bytes]:
    """Return the currently installed root payload, if present."""

    if not _decode_root_stack:
        return None
    return _decode_root_stack[-1]


@dataclass
class NoritoEncoder:
    """Encodes values using Norito layouts."""

    flags: int
    buffer: bytearray
    layout: WireLayout

    def __init__(self, flags: int, *, layout: WireLayout = WireLayout.CURRENT) -> None:
        self.layout = WireLayout(layout)
        header.validate_flags(flags, layout=self.layout)
        self.flags = flags
        self.buffer = bytearray()

    # Primitive writers -------------------------------------------------
    def write_u8(self, value: int) -> None:
        self.buffer.append(value & 0xFF)

    def write_i8(self, value: int) -> None:
        self.buffer.append(value & 0xFF)

    def write_uint(self, value: int, bits: int) -> None:
        max_value = (1 << bits) - 1
        if value < 0 or value > max_value:
            raise ValueError(f"value {value} does not fit in u{bits}")
        self.buffer.extend(value.to_bytes(bits // 8, "little", signed=False))

    def write_int(self, value: int, bits: int) -> None:
        min_value = -(1 << (bits - 1))
        max_value = (1 << (bits - 1)) - 1
        if value < min_value or value > max_value:
            raise ValueError(f"value {value} does not fit in i{bits}")
        self.buffer.extend(value.to_bytes(bits // 8, "little", signed=True))

    def write_f32(self, value: float) -> None:
        self.buffer.extend(pack("<f", value))

    def write_f64(self, value: float) -> None:
        self.buffer.extend(pack("<d", value))

    def write_bytes(self, data: bytes) -> None:
        self.buffer.extend(data)

    # Length helpers ----------------------------------------------------
    def write_length(self, value: int, *, compact: bool) -> None:
        if not 0 <= value <= 0xFFFF_FFFF_FFFF_FFFF:
            raise ValueError("length must fit in u64")
        if compact or self.layout is WireLayout.DENSE:
            self.buffer.extend(encode_varint(value))
        else:
            self.buffer.extend(value.to_bytes(8, "little"))

    def write_count(self, value: int) -> None:
        """Write a sequence/map count in the selected layout."""
        self.write_length(value, compact=False)

    def write_variant_index(self, value: int) -> None:
        """Write a u32 enum tag (a shortest varint in dense)."""
        if not 0 <= value <= 0xFFFF_FFFF:
            raise ValueError("enum tag must fit in u32")
        if self.layout is WireLayout.DENSE:
            self.buffer.extend(encode_varint(value))
        else:
            self.write_uint(value, 32)

    def write_field(self, adapter: TypeAdapter[Any], value: Any) -> None:
        """Write one length-delimited child inheriting this layout."""
        child = NoritoEncoder(self.flags, layout=self.layout)
        adapter.encode(child, value)
        payload = child.finish()
        self.write_length(len(payload), compact=bool(self.flags & header.COMPACT_LEN))
        self.write_bytes(payload)

    def extend(self, data: bytes) -> None:
        self.buffer.extend(data)

    def finish(self) -> bytes:
        return bytes(self.buffer)


@dataclass
class NoritoDecoder:
    """Decodes bytes using Norito layouts."""

    data: bytes
    flags: int
    offset: int = 0
    layout: WireLayout = field(default=WireLayout.CURRENT, kw_only=True)

    def __post_init__(self) -> None:
        self.layout = WireLayout(self.layout)
        header.validate_flags(self.flags, layout=self.layout)
        if not 0 <= self.offset <= len(self.data):
            raise DecodeError("invalid decoder offset")

    def compact_len_active(self) -> bool:
        return self.layout is WireLayout.DENSE or bool(self.flags & header.COMPACT_LEN)

    def remaining(self) -> int:
        return len(self.data) - self.offset

    def read_exact(self, count: int) -> bytes:
        if count < 0 or self.offset + count > len(self.data):
            raise DecodeError("unexpected end of data")
        chunk = self.data[self.offset : self.offset + count]
        self.offset += count
        return chunk

    def read_u8(self) -> int:
        return self._read_int(1, signed=False)

    def read_i8(self) -> int:
        return self._read_int(1, signed=True)

    def read_uint(self, bits: int) -> int:
        return self._read_int(bits // 8, signed=False)

    def read_int(self, bits: int) -> int:
        return self._read_int(bits // 8, signed=True)

    def _read_int(self, size: int, *, signed: bool) -> int:
        raw = self.read_exact(size)
        return int.from_bytes(raw, "little", signed=signed)

    def read_f32(self) -> float:
        return unpack("<f", self.read_exact(4))[0]

    def read_f64(self) -> float:
        return unpack("<d", self.read_exact(8))[0]

    def read_bytes(self, length: int) -> bytes:
        return self.read_exact(length)

    def read_length(self, *, compact: bool) -> int:
        if compact or self.layout is WireLayout.DENSE:
            start = self.offset
            try:
                value, end = decode_varint(self.data, start)
            except VarintError as exc:
                raise DecodeError(str(exc), cause=exc) from exc
            if self.layout is WireLayout.DENSE and (
                value > 0xFFFF_FFFF_FFFF_FFFF
                or self.data[start:end] != encode_varint(value)
            ):
                raise DecodeError("non-canonical dense varint")
            self.offset = end
            return value
        return self.read_uint(64)

    def read_count(self) -> int:
        """Read a sequence/map count in the advertised layout."""
        return self.read_length(compact=False)

    def read_variant_index(self) -> int:
        """Read a canonical u32 enum tag."""
        if self.layout is WireLayout.CURRENT:
            return self.read_uint(32)
        value = self.read_length(compact=True)
        if value > 0xFFFF_FFFF:
            raise DecodeError("enum tag exceeds u32")
        return value

    def read_field(self, adapter: TypeAdapter[T]) -> T:
        """Decode an exact child span with the same flags and layout."""
        length = self.read_length(compact=self.compact_len_active())
        child = NoritoDecoder(self.read_exact(length), self.flags, layout=self.layout)
        value = adapter.decode(child)
        if child.remaining():
            raise DecodeError("trailing bytes in Norito field")
        return value


# ---------------------------------------------------------------------------
# Primitive adapters
# ---------------------------------------------------------------------------


@dataclass
class UIntAdapter(TypeAdapter[int]):
    bits: int

    def encode(self, encoder: NoritoEncoder, value: int) -> None:
        encoder.write_uint(value, self.bits)

    def decode(self, decoder: NoritoDecoder) -> int:
        return decoder.read_uint(self.bits)

    def fixed_size(self) -> Optional[int]:
        return self.bits // 8

    def is_self_delimiting(self) -> bool:
        return False


@dataclass
class IntAdapter(TypeAdapter[int]):
    bits: int

    def encode(self, encoder: NoritoEncoder, value: int) -> None:
        encoder.write_int(value, self.bits)

    def decode(self, decoder: NoritoDecoder) -> int:
        return decoder.read_int(self.bits)

    def fixed_size(self) -> Optional[int]:
        return self.bits // 8

    def is_self_delimiting(self) -> bool:
        return False


class BoolAdapter(TypeAdapter[bool]):
    def encode(self, encoder: NoritoEncoder, value: bool) -> None:
        encoder.write_u8(1 if value else 0)

    def decode(self, decoder: NoritoDecoder) -> bool:
        tag = decoder.read_u8()
        if decoder.layout is WireLayout.DENSE and tag not in (0, 1):
            raise DecodeError(f"invalid bool tag {tag}")
        return tag != 0

    def fixed_size(self) -> Optional[int]:
        return 1

    def is_self_delimiting(self) -> bool:
        return False


class BytesAdapter(TypeAdapter[bytes]):
    def encode(self, encoder: NoritoEncoder, value: bytes) -> None:
        compact = bool(encoder.flags & header.COMPACT_LEN)
        encoder.write_length(len(value), compact=compact)
        encoder.write_bytes(value)

    def decode(self, decoder: NoritoDecoder) -> bytes:
        length = decoder.read_length(compact=decoder.compact_len_active())
        return decoder.read_bytes(length)

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


class StringAdapter(TypeAdapter[str]):
    def encode(self, encoder: NoritoEncoder, value: str) -> None:
        data = value.encode("utf-8")
        BytesAdapter().encode(encoder, data)

    def decode(self, decoder: NoritoDecoder) -> str:
        data = BytesAdapter().decode(decoder)
        return data.decode("utf-8")

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class FixedBytesAdapter(TypeAdapter[bytes]):
    length: int
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, value: bytes | bytearray) -> None:
        data = bytes(value)
        if len(data) != self.length:
            raise ValueError(f"expected {self.length} bytes, found {len(data)}")
        if self.framed and encoder.layout is WireLayout.CURRENT:
            for byte in data:
                encoder.write_field(UIntAdapter(8), byte)
        else:
            encoder.write_bytes(data)

    def decode(self, decoder: NoritoDecoder) -> bytes:
        if self.framed and decoder.layout is WireLayout.CURRENT:
            return bytes(decoder.read_field(UIntAdapter(8)) for _ in range(self.length))
        return decoder.read_bytes(self.length)

    def fixed_size(self) -> Optional[int]:
        return None if self.framed else self.length

    def is_self_delimiting(self) -> bool:
        return False


# ---------------------------------------------------------------------------
# Composite adapters
# ---------------------------------------------------------------------------


@dataclass
class OptionAdapter(Generic[T], TypeAdapter[Optional[T]]):
    inner: TypeAdapter[T]
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, value: Optional[T]) -> None:
        if value is None:
            encoder.write_u8(0)
            return
        encoder.write_u8(1)
        if self.framed or encoder.layout is WireLayout.DENSE:
            encoder.write_field(self.inner, value)
        else:
            self.inner.encode(encoder, value)

    def decode(self, decoder: NoritoDecoder) -> Optional[T]:
        tag = decoder.read_u8()
        if tag == 0:
            return None
        if tag != 1:
            raise DecodeError(f"invalid Option tag {tag}")
        if self.framed or decoder.layout is WireLayout.DENSE:
            return decoder.read_field(self.inner)
        return self.inner.decode(decoder)

    def fixed_size(self) -> Optional[int]:
        inner = self.inner.fixed_size()
        if inner is not None and not self.framed:
            return 1 + inner
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class ResultAdapter(Generic[T, E], TypeAdapter[object]):
    ok: TypeAdapter[T]
    err: TypeAdapter[E]
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, value: Ok[T] | Err[E]) -> None:
        if isinstance(value, Ok):
            encoder.write_u8(0)
            adapter, child = self.ok, value.value
        elif isinstance(value, Err):
            encoder.write_u8(1)
            adapter, child = self.err, value.error
        else:
            raise ValueError("ResultAdapter expects Ok or Err instances")
        if self.framed or encoder.layout is WireLayout.DENSE:
            encoder.write_field(adapter, child)
        else:
            adapter.encode(encoder, child)

    def decode(self, decoder: NoritoDecoder) -> Ok[T] | Err[E]:
        tag = decoder.read_u8()
        framed = self.framed or decoder.layout is WireLayout.DENSE
        if tag == 0:
            return Ok(decoder.read_field(self.ok) if framed else self.ok.decode(decoder))
        if tag == 1:
            return Err(decoder.read_field(self.err) if framed else self.err.decode(decoder))
        raise DecodeError(f"invalid Result tag {tag}")

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class SequenceAdapter(Generic[T], TypeAdapter[List[T]]):
    element: TypeAdapter[T]
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, value: Iterable[T]) -> None:
        elements = list(value)
        length = len(elements)
        encoder.write_count(length)
        framed = (self.framed or encoder.layout is WireLayout.DENSE) and not (
            isinstance(self.element, UIntAdapter) and self.element.bits == 8
        )
        for element in elements:
            if framed:
                encoder.write_field(self.element, element)
            else:
                self.element.encode(encoder, element)

    def decode(self, decoder: NoritoDecoder) -> List[T]:
        length = decoder.read_count()
        framed = (self.framed or decoder.layout is WireLayout.DENSE) and not (
            isinstance(self.element, UIntAdapter) and self.element.bits == 8
        )
        if framed:
            if length > decoder.remaining():
                raise DecodeError("sequence count exceeds available element prefixes")
            return [decoder.read_field(self.element) for _ in range(length)]
        return [self.element.decode(decoder) for _ in range(length)]

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class MapAdapter(Generic[T], TypeAdapter[Dict[Any, Any]]):
    key: TypeAdapter[Any]
    value: TypeAdapter[Any]
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, mapping: Dict[Any, Any]) -> None:
        if self.framed or encoder.layout is WireLayout.DENSE:
            encoder.write_count(len(mapping))
            for key, value in sorted(mapping.items()):
                encoder.write_field(self.key, key)
                encoder.write_field(self.value, value)
            return
        sequence = SequenceAdapter(TupleAdapter((self.key, self.value)))
        sequence.encode(encoder, list(mapping.items()))

    def decode(self, decoder: NoritoDecoder) -> Dict[Any, Any]:
        if self.framed or decoder.layout is WireLayout.DENSE:
            length = decoder.read_count()
            if length > decoder.remaining() // 2:
                raise DecodeError("map count exceeds available field prefixes")
            mapping = {}
            previous = None
            for index in range(length):
                key = decoder.read_field(self.key)
                if index and key <= previous:
                    raise DecodeError("map keys must be strictly increasing")
                mapping[key] = decoder.read_field(self.value)
                previous = key
            return mapping
        sequence = SequenceAdapter(TupleAdapter((self.key, self.value)))
        return dict(sequence.decode(decoder))

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class TupleAdapter(TypeAdapter[Tuple[Any, ...]]):
    elements: Tuple[TypeAdapter[Any], ...]
    framed: bool = False

    def encode(self, encoder: NoritoEncoder, value: Tuple[Any, ...]) -> None:
        if len(value) != len(self.elements):
            raise ValueError("tuple length mismatch")
        for adapter, item in zip(self.elements, value):
            if self.framed or encoder.layout is WireLayout.DENSE:
                encoder.write_field(adapter, item)
            else:
                adapter.encode(encoder, item)

    def decode(self, decoder: NoritoDecoder) -> Tuple[Any, ...]:
        if self.framed or decoder.layout is WireLayout.DENSE:
            return tuple(decoder.read_field(adapter) for adapter in self.elements)
        return tuple(adapter.decode(decoder) for adapter in self.elements)

    def fixed_size(self) -> Optional[int]:
        sizes = [adapter.fixed_size() for adapter in self.elements]
        if not self.framed and all(size is not None for size in sizes):
            return sum(size or 0 for size in sizes)
        return None

    def is_self_delimiting(self) -> bool:
        return all(adapter.is_self_delimiting() for adapter in self.elements)


# ---------------------------------------------------------------------------
# Struct support
# ---------------------------------------------------------------------------


@dataclass
class StructField:
    name: str
    adapter: TypeAdapter[Any]


class StructAdapter(TypeAdapter[Any]):
    """Record fields, length-prefixed in dense or when ``framed=True``."""

    def __init__(
        self,
        fields: List[StructField],
        *,
        factory: Optional[Callable[..., Any]] = None,
        framed: bool = False,
    ) -> None:
        self.fields = fields
        self.factory = factory
        self.framed = framed

    def encode(self, encoder: NoritoEncoder, value: Any) -> None:
        for field in self.fields:
            field_value = self._extract_field_value(value, field.name)
            if self.framed or encoder.layout is WireLayout.DENSE:
                encoder.write_field(field.adapter, field_value)
            else:
                field.adapter.encode(encoder, field_value)

    def decode(self, decoder: NoritoDecoder) -> Any:
        values: Dict[str, Any] = {}
        for field in self.fields:
            values[field.name] = (
                decoder.read_field(field.adapter)
                if self.framed or decoder.layout is WireLayout.DENSE
                else field.adapter.decode(decoder)
            )
        return self._build_instance(values)

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True

    def _build_instance(self, values: Dict[str, Any]) -> Any:
        if self.factory is None:
            return values
        return self.factory(**values)

    @staticmethod
    def _extract_field_value(value: Any, name: str) -> Any:
        if isinstance(value, Mapping):
            return value[name]
        if hasattr(value, name):
            return getattr(value, name)
        raise AttributeError(f"Struct value is missing field '{name}'")


# ---------------------------------------------------------------------------
# High level encode/decode
# ---------------------------------------------------------------------------


_BASE_FLAGS = header.MINOR_VERSION
DEFAULT_FLAGS = _BASE_FLAGS | _DYNAMIC_FLAGS_MASK


def _apply_adaptive_flags(flags: int, payload_len: int) -> int:
    _ = payload_len
    return flags


def encode(
    value: Any,
    schema: SchemaDescriptor,
    adapter: TypeAdapter[Any],
    *,
    flags: int = DEFAULT_FLAGS,
    layout: WireLayout = WireLayout.CURRENT,
    compression: bool | int = False,
    compression_level: int | None = None,
    compression_profile: str | None = None,
) -> bytes:
    encoder = NoritoEncoder(flags, layout=layout)
    adapter.encode(encoder, value)
    payload = encoder.finish()
    checksum = crc64(payload)
    payload_bytes = payload
    compression_byte = header.COMPRESSION_NONE
    if compression:
        if compression is True:
            chosen = header.COMPRESSION_ZSTD
        elif isinstance(compression, int):
            if compression in (header.COMPRESSION_NONE, header.COMPRESSION_ZSTD):
                chosen = compression
            else:
                raise UnsupportedCompressionError(compression)
        else:
            raise TypeError("compression must be bool or compression constant")

        if chosen == header.COMPRESSION_ZSTD:
            if compression_profile is not None and compression_level is not None:
                raise ValueError(
                    "compression_level and compression_profile are mutually exclusive"
                )
            compression_byte = header.COMPRESSION_ZSTD
            level = resolve_zstd_level(
                len(payload),
                level=compression_level,
                profile=compression_profile,
            )
            payload_bytes = compress_zstd(payload, level=level)
        else:
            compression_byte = header.COMPRESSION_NONE
    norito_header = header.NoritoHeader(
        schema_hash=schema.hash_bytes(),
        payload_length=len(payload),
        checksum=checksum,
        flags=flags,
        compression=compression_byte,
        minor=encoder.layout.value,
    )
    encoded = norito_header.encode() + payload_bytes
    return encoded


def encode_adaptive(
    value: Any,
    adapter: TypeAdapter[Any],
    *,
    flags: int = DEFAULT_FLAGS,
    layout: WireLayout = WireLayout.CURRENT,
) -> bytes:
    """Encode bare bytes; their protocol owner must retain the selected layout."""
    encoder = NoritoEncoder(flags, layout=layout)
    adapter.encode(encoder, value)
    payload = encoder.finish()
    final_flags = _apply_adaptive_flags(flags, len(payload))
    if final_flags != flags:
        encoder = NoritoEncoder(final_flags, layout=layout)
        adapter.encode(encoder, value)
        payload = encoder.finish()
    return payload


def encode_with_header_flags(
    value: Any,
    adapter: TypeAdapter[Any],
    *,
    flags: int = DEFAULT_FLAGS,
    layout: WireLayout = WireLayout.CURRENT,
) -> Tuple[bytes, int]:
    payload = encode_adaptive(value, adapter, flags=flags, layout=layout)
    final_flags = _apply_adaptive_flags(flags, len(payload))
    return payload, final_flags


class DecodeFlagsGuard:
    __slots__ = ("_active", "_flags")

    def __init__(self, flags: int) -> None:
        self._flags = flags & 0xFF
        self._active = False

    def __enter__(self) -> "DecodeFlagsGuard":
        _decode_flags_stack.append(self._flags)
        self._active = True
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._active:
            _decode_flags_stack.pop()
            self._active = False

    @staticmethod
    def enter(flags: int) -> "DecodeFlagsGuard":
        return DecodeFlagsGuard(flags)


def reset_decode_state() -> None:
    _decode_flags_stack.clear()
    _decode_root_stack.clear()
    _decode_context_stack.clear()


def effective_decode_flags() -> Optional[int]:
    if _decode_context_stack:
        return _decode_context_stack[-1]
    if _decode_flags_stack:
        return _decode_flags_stack[-1]
    return None


@dataclass(frozen=True)
class ArchiveView:
    """Validated view over a payload retaining its advertised layout and flags."""

    _payload: bytes
    _flags: int
    layout: WireLayout = WireLayout.CURRENT

    def as_bytes(self) -> bytes:
        return self._payload

    @property
    def flags(self) -> int:
        return self._flags

    def decode(self, adapter: TypeAdapter[Any]) -> Any:
        with _RootGuard(self._payload, self._flags):
            decoder = NoritoDecoder(self._payload, self._flags, layout=self.layout)
            value = adapter.decode(decoder)
            if decoder.remaining() != 0:
                raise DecodeError("trailing bytes after Norito decode")
            return value


def from_bytes_view(
    data: bytes,
    *,
    schema: SchemaDescriptor | None = None,
    layout: WireLayout | None = None,
) -> ArchiveView:
    """Validate a framed archive and return a view over the payload."""

    reset_decode_state()
    expected_hash = schema.hash_bytes() if schema is not None else None
    norito_header, payload = header.NoritoHeader.decode(
        data,
        expected_schema_hash=expected_hash,
        expected_layout=layout,
    )
    if norito_header.compression != header.COMPRESSION_NONE:
        raise UnsupportedCompressionError(norito_header.compression)
    norito_header.validate_checksum(payload)
    return ArchiveView(payload, norito_header.flags, norito_header.layout)


def decode_adaptive(
    data: bytes,
    adapter: TypeAdapter[Any],
    *,
    layout: WireLayout = WireLayout.CURRENT,
    flags: int | None = None,
) -> Any:
    """Decode exact bare bytes with an owner-supplied layout, without inference."""
    layout = WireLayout(layout)
    if flags is None:
        if layout is WireLayout.DENSE:
            flags = 0
        elif _decode_flags_stack:
            flags = _decode_flags_stack[-1]
        else:
            flags = _apply_adaptive_flags(DEFAULT_FLAGS, len(data))
    with _RootGuard(data, flags):
        decoder = NoritoDecoder(data, flags, layout=layout)
        value = adapter.decode(decoder)
        if decoder.remaining() != 0:
            raise DecodeError("trailing bytes after decode")
        return value


def decode(
    data: bytes,
    adapter: TypeAdapter[Any],
    *,
    schema: SchemaDescriptor | None = None,
    layout: WireLayout | None = None,
) -> Any:
    expected_hash = schema.hash_bytes() if schema is not None else None
    norito_header, payload = header.NoritoHeader.decode(
        data,
        expected_schema_hash=expected_hash,
        expected_layout=layout,
    )
    if norito_header.compression == header.COMPRESSION_ZSTD:
        payload_bytes = decompress_zstd(
            payload, target_size=norito_header.payload_length
        )
    else:
        payload_bytes = payload
    norito_header.validate_checksum(payload_bytes)
    with _RootGuard(payload_bytes, norito_header.flags):
        decoder = NoritoDecoder(payload_bytes, norito_header.flags, layout=norito_header.layout)
        value = adapter.decode(decoder)
        if decoder.remaining() != 0:
            raise DecodeError("trailing bytes after decode")
        return value


def try_decode(
    data: bytes,
    adapter: TypeAdapter[Any],
    *,
    schema: SchemaDescriptor | None = None,
    layout: WireLayout | None = None,
) -> Ok[Any] | Err[NoritoError]:
    """Decode a payload and surface Norito errors without raising.

    Mirrors the new fallible `NoritoDeserialize::try_deserialize` helper on the
    Rust side.  The returned value is either an Ok(wrapper) containing the
    decoded value or an Err with the encountered :class:`NoritoError`.
    """

    try:
        return Ok(decode(data, adapter, schema=schema, layout=layout))
    except NoritoError as exc:
        return Err(exc)


# Convenience adapter constructors


def u8() -> TypeAdapter[int]:
    return UIntAdapter(8)


def u16() -> TypeAdapter[int]:
    return UIntAdapter(16)


def u32() -> TypeAdapter[int]:
    return UIntAdapter(32)


def u64() -> TypeAdapter[int]:
    return UIntAdapter(64)


def u128() -> TypeAdapter[int]:
    return UIntAdapter(128)


def i8() -> TypeAdapter[int]:
    return IntAdapter(8)


def i16() -> TypeAdapter[int]:
    return IntAdapter(16)


def i32() -> TypeAdapter[int]:
    return IntAdapter(32)


def i64() -> TypeAdapter[int]:
    return IntAdapter(64)


def bool_() -> TypeAdapter[bool]:
    return BoolAdapter()


def bytes_() -> TypeAdapter[bytes]:
    return BytesAdapter()


def fixed_bytes(length: int, *, framed: bool = False) -> TypeAdapter[bytes]:
    """Describe raw bytes, or generic current-layout arrays with framed elements."""
    if length <= 0:
        raise ValueError("length must be positive")
    return FixedBytesAdapter(length, framed=framed)


def string() -> TypeAdapter[str]:
    return StringAdapter()


def option(inner: TypeAdapter[T], *, framed: bool = False) -> TypeAdapter[Optional[T]]:
    """Describe an option; ``framed`` enables current-layout child prefixes."""
    return OptionAdapter(inner, framed=framed)


def result(
    ok: TypeAdapter[T], err: TypeAdapter[E], *, framed: bool = False,
) -> TypeAdapter[Ok[T] | Err[E]]:
    """Describe a result; ``framed`` enables current-layout child prefixes."""
    return ResultAdapter(ok, err, framed=framed)


def seq(inner: TypeAdapter[T], *, framed: bool = False) -> TypeAdapter[List[T]]:
    """Describe a sequence; ``framed`` prefixes current-layout non-byte elements."""
    return SequenceAdapter(inner, framed=framed)


def map_adapter(
    key: TypeAdapter[Any], value: TypeAdapter[Any], *, framed: bool = False,
) -> TypeAdapter[Dict[Any, Any]]:
    """Describe a map; dense/framed maps have ordered keys and child prefixes."""
    return MapAdapter(key, value, framed=framed)


def tuple_adapter(*inners: TypeAdapter[Any], framed: bool = False) -> TypeAdapter[Tuple[Any, ...]]:
    """Describe a tuple; ``framed`` enables current-layout member prefixes."""
    return TupleAdapter(tuple(inners), framed=framed)


__all__ = [
    "NoritoEncoder",
    "NoritoDecoder",
    "TypeAdapter",
    "encode",
    "encode_adaptive",
    "encode_with_header_flags",
    "decode",
    "try_decode",
    "decode_adaptive",
    "u8",
    "u16",
    "u32",
    "u64",
    "i8",
    "i16",
    "i32",
    "i64",
    "bool_",
    "bytes_",
    "fixed_bytes",
    "string",
    "option",
    "result",
    "seq",
    "map_adapter",
    "tuple_adapter",
    "StructAdapter",
    "StructField",
    "DecodeFlagsGuard",
    "reset_decode_state",
    "DEFAULT_FLAGS",
]
