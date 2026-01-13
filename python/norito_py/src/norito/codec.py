# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Core Norito encoding/decoding logic."""

from __future__ import annotations

from dataclasses import dataclass
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
from .result import Err, Ok
from .schema import SchemaDescriptor
from .varint import decode_varint, encode_varint

T = TypeVar("T")
E = TypeVar("E")

_decode_flags_stack: List[int] = []
_decode_flags_hint_stack: List[int] = []
_last_encode_flags: Optional[int] = None
_decode_root_stack: List[bytes] = []
_decode_context_stack: List[Tuple[int, int]] = []

_DYNAMIC_FLAGS_MASK = 0


class TypeAdapter(Protocol[T]):
    """Protocol implemented by all type adapters."""

    def encode(self, encoder: "NoritoEncoder", value: T) -> None:
        ...

    def decode(self, decoder: "NoritoDecoder") -> T:
        ...

    def fixed_size(self) -> Optional[int]:
        """Return the size in bytes if the encoding is fixed width."""
        ...

    def is_self_delimiting(self) -> bool:
        """Return True if the adapter carries its own length markers."""
        ...


class _RootGuard:
    __slots__ = ("_owned", "_context_active", "_flags", "_hint")

    def __init__(self, payload: bytes, flags: Optional[int] = None, hint: Optional[int] = None) -> None:
        self._owned = False
        self._context_active = False
        self._flags = flags
        self._hint = header.MINOR_VERSION if hint is None else hint & 0xFF
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
            _decode_context_stack.append((self._flags & 0xFF, self._hint))
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

    def __init__(self, flags: int) -> None:
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
        if compact:
            self.buffer.extend(encode_varint(value))
        else:
            self.buffer.extend(value.to_bytes(8, "little"))

    def child_encoder(self) -> "NoritoEncoder":
        return NoritoEncoder(self.flags)

    def extend(self, data: bytes) -> None:
        self.buffer.extend(data)

    def finish(self) -> bytes:
        return bytes(self.buffer)


@dataclass
class NoritoDecoder:
    """Decodes bytes using Norito layouts."""

    data: bytes
    flags: int
    flags_hint: int = header.MINOR_VERSION
    offset: int = 0

    def compact_len_active(self) -> bool:
        return bool(self.flags & header.COMPACT_LEN)

    def compact_seq_len_active(self) -> bool:
        return bool(self.flags & header.COMPACT_SEQ_LEN)

    def remaining(self) -> int:
        return len(self.data) - self.offset

    def read_exact(self, count: int) -> bytes:
        if self.offset + count > len(self.data):
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
        if compact:
            value, self.offset = decode_varint(self.data, self.offset)
            return value
        return self.read_uint(64)


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
        return decoder.read_u8() != 0

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

    def encode(self, encoder: NoritoEncoder, value: bytes | bytearray) -> None:
        data = bytes(value)
        if len(data) != self.length:
            raise ValueError(f"expected {self.length} bytes, found {len(data)}")
        encoder.write_bytes(data)

    def decode(self, decoder: NoritoDecoder) -> bytes:
        return decoder.read_bytes(self.length)

    def fixed_size(self) -> Optional[int]:
        return self.length

    def is_self_delimiting(self) -> bool:
        return False


# ---------------------------------------------------------------------------
# Composite adapters
# ---------------------------------------------------------------------------


@dataclass
class OptionAdapter(Generic[T], TypeAdapter[Optional[T]]):
    inner: TypeAdapter[T]

    def encode(self, encoder: NoritoEncoder, value: Optional[T]) -> None:
        if value is None:
            encoder.write_u8(0)
            return
        encoder.write_u8(1)
        self.inner.encode(encoder, value)

    def decode(self, decoder: NoritoDecoder) -> Optional[T]:
        tag = decoder.read_u8()
        if tag == 0:
            return None
        if tag != 1:
            raise DecodeError(f"invalid Option tag {tag}")
        return self.inner.decode(decoder)

    def fixed_size(self) -> Optional[int]:
        inner = self.inner.fixed_size()
        if inner is not None:
            return 1 + inner
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class ResultAdapter(Generic[T, E], TypeAdapter[object]):
    ok: TypeAdapter[T]
    err: TypeAdapter[E]

    def encode(self, encoder: NoritoEncoder, value: Ok[T] | Err[E]) -> None:
        if isinstance(value, Ok):
            encoder.write_u8(0)
            self.ok.encode(encoder, value.value)
        elif isinstance(value, Err):
            encoder.write_u8(1)
            self.err.encode(encoder, value.error)
        else:
            raise ValueError("ResultAdapter expects Ok or Err instances")

    def decode(self, decoder: NoritoDecoder) -> Ok[T] | Err[E]:
        tag = decoder.read_u8()
        if tag == 0:
            return Ok(self.ok.decode(decoder))
        if tag == 1:
            return Err(self.err.decode(decoder))
        raise DecodeError(f"invalid Result tag {tag}")

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class SequenceAdapter(Generic[T], TypeAdapter[List[T]]):
    element: TypeAdapter[T]

    def encode(self, encoder: NoritoEncoder, value: Iterable[T]) -> None:
        elements = list(value)
        length = len(elements)
        compact_len = bool(encoder.flags & header.COMPACT_SEQ_LEN)
        encoder.write_length(length, compact=compact_len)

        if encoder.flags & header.PACKED_SEQ:
            self._encode_packed(encoder, elements)
        else:
            for element in elements:
                self.element.encode(encoder, element)

    def _encode_packed(self, encoder: NoritoEncoder, elements: List[T]) -> None:
        chunks = [encoder.child_encoder() for _ in elements]
        encoded_parts: List[bytes] = []
        for child, element in zip(chunks, elements):
            self.element.encode(child, element)
            encoded_parts.append(child.finish())

        varint_offsets = bool(encoder.flags & header.VARINT_OFFSETS)
        if varint_offsets:
            for chunk in encoded_parts:
                encoder.extend(encode_varint(len(chunk)))
        else:
            offsets = [0]
            total = 0
            for chunk in encoded_parts:
                total += len(chunk)
                offsets.append(total)
            for offset in offsets:
                encoder.extend(offset.to_bytes(8, "little"))
        for chunk in encoded_parts:
            encoder.extend(chunk)

    def decode(self, decoder: NoritoDecoder) -> List[T]:
        length = decoder.read_length(compact=decoder.compact_seq_len_active())

        if decoder.flags & header.PACKED_SEQ:
            return self._decode_packed(decoder, length)

        return [self.element.decode(decoder) for _ in range(length)]

    def _decode_packed(self, decoder: NoritoDecoder, length: int) -> List[T]:
        varint_offsets = bool(decoder.flags & header.VARINT_OFFSETS)
        if length == 0:
            # Compat Rust encoders appended a single zero offset tail even when the
            # sequence contained no elements. Accept and skip those bytes so older
            # payloads continue to decode.
            tail = decoder.data[decoder.offset :]
            if not tail:
                return []
            if len(tail) >= 8 and all(b == 0 for b in tail[:8]):
                decoder.offset += 8
                return []
            raise DecodeError("packed sequence declared zero length but carried trailing data")

        element_sizes: List[int] = []
        if varint_offsets:
            for _ in range(length):
                size, decoder.offset = decode_varint(decoder.data, decoder.offset)
                element_sizes.append(size)
        else:
            offsets = [decoder.read_uint(64) for _ in range(length + 1)]
            element_sizes = [b - a for a, b in zip(offsets, offsets[1:])]

        outputs: List[T] = []
        for size in element_sizes:
            chunk = decoder.read_bytes(size)
            child = NoritoDecoder(chunk, decoder.flags, decoder.flags_hint)
            outputs.append(self.element.decode(child))
            if child.remaining() != 0:
                raise DecodeError("packed element decode did not consume all bytes")
        return outputs

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class MapAdapter(Generic[T], TypeAdapter[Dict[Any, Any]]):
    key: TypeAdapter[Any]
    value: TypeAdapter[Any]

    def encode(self, encoder: NoritoEncoder, mapping: Dict[Any, Any]) -> None:
        sequence = SequenceAdapter(TupleAdapter(self.key, self.value))
        sequence.encode(encoder, list(mapping.items()))

    def decode(self, decoder: NoritoDecoder) -> Dict[Any, Any]:
        sequence = SequenceAdapter(TupleAdapter(self.key, self.value))
        return dict(sequence.decode(decoder))

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


@dataclass
class TupleAdapter(TypeAdapter[Tuple[Any, ...]]):
    elements: Tuple[TypeAdapter[Any], ...]

    def encode(self, encoder: NoritoEncoder, value: Tuple[Any, ...]) -> None:
        if len(value) != len(self.elements):
            raise ValueError("tuple length mismatch")
        for adapter, item in zip(self.elements, value):
            adapter.encode(encoder, item)

    def decode(self, decoder: NoritoDecoder) -> Tuple[Any, ...]:
        return tuple(adapter.decode(decoder) for adapter in self.elements)

    def fixed_size(self) -> Optional[int]:
        sizes = [adapter.fixed_size() for adapter in self.elements]
        if all(size is not None for size in sizes):
            return sum(size or 0 for size in sizes)
        return None

    def is_self_delimiting(self) -> bool:
        return all(adapter.is_self_delimiting() for adapter in self.elements)


# ---------------------------------------------------------------------------
# Struct support (hybrid bitset layout)
# ---------------------------------------------------------------------------


@dataclass
class StructField:
    name: str
    adapter: TypeAdapter[Any]


class StructAdapter(TypeAdapter[Any]):
    """Adapter for struct-like layouts honoring PACKED_STRUCT semantics."""

    def __init__(
        self,
        fields: List[StructField],
        *,
        factory: Optional[Callable[..., Any]] = None,
    ) -> None:
        self.fields = fields
        self.factory = factory

    def encode(self, encoder: NoritoEncoder, value: Any) -> None:
        if encoder.flags & header.PACKED_STRUCT and encoder.flags & header.FIELD_BITSET:
            self._encode_packed(encoder, value)
            return
        for field in self.fields:
            field_value = self._extract_field_value(value, field.name)
            field.adapter.encode(encoder, field_value)

    def _encode_packed(self, encoder: NoritoEncoder, value: Any) -> None:
        field_payloads: List[bytes] = []
        needs_size: List[bool] = []
        bitset = 0

        for idx, field in enumerate(self.fields):
            field_value = self._extract_field_value(value, field.name)
            child = encoder.child_encoder()
            field.adapter.encode(child, field_value)
            data = child.finish()
            field_payloads.append(data)
            size_required = self._needs_size(field.adapter)
            needs_size.append(size_required)
            if size_required:
                bitset |= 1 << idx

        bitset_bytes = (len(self.fields) + 7) // 8
        encoder.extend(bitset.to_bytes(bitset_bytes, "little"))

        for idx, data in enumerate(field_payloads):
            if needs_size[idx]:
                encoder.extend(encode_varint(len(data)))

        for data in field_payloads:
            encoder.extend(data)

    def decode(self, decoder: NoritoDecoder) -> Any:
        if decoder.flags & header.PACKED_STRUCT and decoder.flags & header.FIELD_BITSET:
            values = self._decode_packed(decoder)
        else:
            values: Dict[str, Any] = {}
            for field in self.fields:
                values[field.name] = field.adapter.decode(decoder)
        return self._build_instance(values)

    def _decode_packed(self, decoder: NoritoDecoder) -> Dict[str, Any]:
        bitset_bytes = (len(self.fields) + 7) // 8
        bitset_data = decoder.read_exact(bitset_bytes)
        bitset = int.from_bytes(bitset_data, "little")

        needs_size = [bool((bitset >> idx) & 1) for idx in range(len(self.fields))]
        dynamic_sizes: List[int] = []
        for need in needs_size:
            if need:
                size, decoder.offset = decode_varint(decoder.data, decoder.offset)
                dynamic_sizes.append(size)

        size_iter = iter(dynamic_sizes)
        values: Dict[str, Any] = {}
        for idx, field in enumerate(self.fields):
            if needs_size[idx]:
                size = next(size_iter)
                chunk = decoder.read_bytes(size)
                child = NoritoDecoder(chunk, decoder.flags, decoder.flags_hint)
                value = field.adapter.decode(child)
                if child.remaining() != 0:
                    raise DecodeError("packed struct field did not fully decode")
            else:
                value = field.adapter.decode(decoder)
            values[field.name] = value
        return values

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True

    def _build_instance(self, values: Dict[str, Any]) -> Any:
        if self.factory is None:
            return values
        return self.factory(**values)

    @staticmethod
    def _needs_size(adapter: TypeAdapter[Any]) -> bool:
        if adapter.fixed_size() is not None:
            return False
        if adapter.is_self_delimiting():
            return False
        return True

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


def _combine_flags(flags: int, hint: int) -> int:
    return flags & 0xFF


def _apply_adaptive_flags(flags: int, payload_len: int) -> int:
    _ = payload_len
    return flags


def _record_encode_flags(flags: Optional[int]) -> None:
    global _last_encode_flags
    _last_encode_flags = flags if flags is None else flags & 0xFF


def take_last_encode_flags() -> Optional[int]:
    global _last_encode_flags
    value = _last_encode_flags
    _last_encode_flags = None
    return value


def encode(
    value: Any,
    schema: SchemaDescriptor,
    adapter: TypeAdapter[Any],
    *,
    flags: int = DEFAULT_FLAGS,
    compression: bool | int = False,
    compression_level: int | None = None,
    compression_profile: str | None = None,
) -> bytes:
    encoder = NoritoEncoder(flags)
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
    )
    encoded = norito_header.encode() + payload_bytes
    _record_encode_flags(flags)
    return encoded


def encode_adaptive(value: Any, adapter: TypeAdapter[Any], *, flags: int = DEFAULT_FLAGS) -> bytes:
    encoder = NoritoEncoder(flags)
    adapter.encode(encoder, value)
    payload = encoder.finish()
    final_flags = _apply_adaptive_flags(flags, len(payload))
    if final_flags != flags:
        encoder = NoritoEncoder(final_flags)
        adapter.encode(encoder, value)
        payload = encoder.finish()
    _record_encode_flags(final_flags)
    return payload


def encode_with_header_flags(
    value: Any, adapter: TypeAdapter[Any], *, flags: int = DEFAULT_FLAGS
) -> Tuple[bytes, int]:
    payload = encode_adaptive(value, adapter, flags=flags)
    final_flags = take_last_encode_flags()
    if final_flags is None:
        raise RuntimeError("encode_adaptive did not record layout flags")
    return payload, final_flags


class DecodeFlagsGuard:
    __slots__ = ("_active", "_flags", "_hint")

    def __init__(self, flags: int, hint: Optional[int] = None) -> None:
        self._flags = flags & 0xFF
        self._hint = header.MINOR_VERSION if hint is None else hint & 0xFF
        self._active = False

    def __enter__(self) -> "DecodeFlagsGuard":
        _decode_flags_stack.append(self._flags)
        _decode_flags_hint_stack.append(self._hint)
        self._active = True
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._active:
            _decode_flags_stack.pop()
            _decode_flags_hint_stack.pop()
            self._active = False

    @staticmethod
    def enter(flags: int) -> "DecodeFlagsGuard":
        return DecodeFlagsGuard(flags)

    @staticmethod
    def enter_with_hint(flags: int, _hint: int) -> "DecodeFlagsGuard":
        return DecodeFlagsGuard(flags, _hint)


def reset_decode_state() -> None:
    _decode_flags_stack.clear()
    _decode_flags_hint_stack.clear()
    _record_encode_flags(None)
    _decode_root_stack.clear()
    _decode_context_stack.clear()


def effective_decode_flags() -> Optional[int]:
    if _decode_context_stack:
        flags, hint = _decode_context_stack[-1]
        return _combine_flags(flags, hint)
    if _decode_flags_stack:
        hint = _decode_flags_hint_stack[-1] if _decode_flags_hint_stack else header.MINOR_VERSION
        return _combine_flags(_decode_flags_stack[-1], hint)
    return None


@dataclass(frozen=True)
class ArchiveView:
    """Validated view over an archived payload with header flags."""

    _payload: bytes
    _flags: int
    _flags_hint: int

    def as_bytes(self) -> bytes:
        return self._payload

    @property
    def flags(self) -> int:
        return self._flags

    @property
    def flags_hint(self) -> int:
        return self._flags_hint

    def decode(self, adapter: TypeAdapter[Any]) -> Any:
        with _RootGuard(self._payload, self._flags, self._flags_hint):
            decoder = NoritoDecoder(self._payload, self._flags, self._flags_hint)
            value = adapter.decode(decoder)
            if decoder.remaining() != 0:
                raise DecodeError("trailing bytes after Norito decode")
            return value


def from_bytes_view(
    data: bytes, *, schema: SchemaDescriptor | None = None
) -> ArchiveView:
    """Validate a framed archive and return a view over the payload."""

    reset_decode_state()
    expected_hash = schema.hash_bytes() if schema is not None else None
    norito_header, payload = header.NoritoHeader.decode(
        data,
        expected_schema_hash=expected_hash,
    )
    if norito_header.compression != header.COMPRESSION_NONE:
        raise UnsupportedCompressionError(norito_header.compression)
    norito_header.validate_checksum(payload)
    return ArchiveView(payload, norito_header.flags, norito_header.minor)


def decode_adaptive(data: bytes, adapter: TypeAdapter[Any]) -> Any:
    if _decode_flags_stack:
        flags = _decode_flags_stack[-1]
    else:
        flags = _apply_adaptive_flags(DEFAULT_FLAGS, len(data))
    with _RootGuard(data, flags, header.MINOR_VERSION):
        decoder = NoritoDecoder(data, flags, header.MINOR_VERSION)
        value = adapter.decode(decoder)
        if decoder.remaining() != 0:
            raise DecodeError("trailing bytes after decode")
        return value


def decode(
    data: bytes,
    adapter: TypeAdapter[Any],
    *,
    schema: SchemaDescriptor | None = None,
) -> Any:
    expected_hash = schema.hash_bytes() if schema is not None else None
    norito_header, payload = header.NoritoHeader.decode(
        data,
        expected_schema_hash=expected_hash,
    )
    if norito_header.compression == header.COMPRESSION_ZSTD:
        payload_bytes = decompress_zstd(
            payload, target_size=norito_header.payload_length
        )
    else:
        payload_bytes = payload
    norito_header.validate_checksum(payload_bytes)
    with _RootGuard(payload_bytes, norito_header.flags, norito_header.minor):
        decoder = NoritoDecoder(payload_bytes, norito_header.flags, norito_header.minor)
        value = adapter.decode(decoder)
        if decoder.remaining() != 0:
            raise DecodeError("trailing bytes after decode")
        return value


def try_decode(
    data: bytes,
    adapter: TypeAdapter[Any],
    *,
    schema: SchemaDescriptor | None = None,
) -> Ok[Any] | Err[NoritoError]:
    """Decode a payload and surface Norito errors without raising.

    Mirrors the new fallible `NoritoDeserialize::try_deserialize` helper on the
    Rust side.  The returned value is either an Ok(wrapper) containing the
    decoded value or an Err with the encountered :class:`NoritoError`.
    """

    try:
        return Ok(decode(data, adapter, schema=schema))
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


def fixed_bytes(length: int) -> TypeAdapter[bytes]:
    if length <= 0:
        raise ValueError("length must be positive")
    return FixedBytesAdapter(length)


def string() -> TypeAdapter[str]:
    return StringAdapter()


def option(inner: TypeAdapter[T]) -> TypeAdapter[Optional[T]]:
    return OptionAdapter(inner)


def result(ok: TypeAdapter[T], err: TypeAdapter[E]) -> TypeAdapter[Ok[T] | Err[E]]:
    return ResultAdapter(ok, err)


def seq(inner: TypeAdapter[T]) -> TypeAdapter[List[T]]:
    return SequenceAdapter(inner)


def map_adapter(key: TypeAdapter[Any], value: TypeAdapter[Any]) -> TypeAdapter[Dict[Any, Any]]:
    return MapAdapter(key, value)


def tuple_adapter(*inners: TypeAdapter[Any]) -> TypeAdapter[Tuple[Any, ...]]:
    return TupleAdapter(tuple(inners))


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
    "take_last_encode_flags",
    "DEFAULT_FLAGS",
]
