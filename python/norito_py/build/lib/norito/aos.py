"""Array-of-Struct helpers for Norito adaptive columnar layouts."""

from __future__ import annotations

from typing import List, Optional, Sequence, Tuple, Union

from .varint import decode_varint, encode_varint

AOS_VERSION: int = 0x1


def _write_len(buf: bytearray, length: int) -> None:
    buf.extend(encode_varint(length))


def _write_len_and_version(buf: bytearray, length: int) -> None:
    _write_len(buf, length)
    buf.append(AOS_VERSION)


def _read_len_and_version(data: bytes, offset: int = 0) -> Tuple[int, int]:
    length, offset = decode_varint(data, offset)
    if offset >= len(data):
        raise ValueError("missing AoS version byte")
    version = data[offset]
    offset += 1
    if version != AOS_VERSION:
        raise ValueError(f"Unsupported AoS version byte {version:#04x}")
    return length, offset


def encode_rows_u64_str_bool(rows: Sequence[Tuple[int, str, bool]]) -> bytes:
    buf = bytearray()
    _write_len_and_version(buf, len(rows))
    for row_id, name, flag in rows:
        buf.extend(int(row_id).to_bytes(8, "little", signed=False))
        data = name.encode("utf-8")
        _write_len(buf, len(data))
        buf.extend(data)
        buf.append(1 if flag else 0)
    return bytes(buf)


def encode_rows_u64_bytes_bool(rows: Sequence[Tuple[int, bytes, bool]]) -> bytes:
    buf = bytearray()
    _write_len_and_version(buf, len(rows))
    for row_id, payload, flag in rows:
        buf.extend(int(row_id).to_bytes(8, "little", signed=False))
        _write_len(buf, len(payload))
        buf.extend(payload)
        buf.append(1 if flag else 0)
    return bytes(buf)


def decode_rows_u64_bytes_bool(data: bytes) -> List[Tuple[int, bytes, bool]]:
    offset = 0
    length, offset = _read_len_and_version(data, offset)
    rows: List[Tuple[int, bytes, bool]] = []
    for _ in range(length):
        row_id = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        blob_len, offset = decode_varint(data, offset)
        blob = data[offset : offset + blob_len]
        offset += blob_len
        if offset >= len(data):
            raise ValueError("AoS payload truncated while reading boolean column")
        flag = data[offset] != 0
        offset += 1
        rows.append((row_id, bytes(blob), flag))
    if offset != len(data):
        raise ValueError("Trailing bytes after AoS decode")
    return rows


def decode_rows_u64_str_bool(data: bytes) -> List[Tuple[int, str, bool]]:
    decoded = decode_rows_u64_bytes_bool(data)
    rows: List[Tuple[int, str, bool]] = []
    for row_id, blob, flag in decoded:
        rows.append((row_id, blob.decode("utf-8"), flag))
    return rows


def encode_rows_u64_optstr_bool(rows: Sequence[Tuple[int, Optional[str], bool]]) -> bytes:
    buf = bytearray()
    _write_len_and_version(buf, len(rows))
    for row_id, maybe_name, flag in rows:
        buf.extend(int(row_id).to_bytes(8, "little", signed=False))
        if maybe_name is None:
            buf.append(0)
        else:
            buf.append(1)
            encoded = maybe_name.encode("utf-8")
            _write_len(buf, len(encoded))
            buf.extend(encoded)
        buf.append(1 if flag else 0)
    return bytes(buf)


def decode_rows_u64_optstr_bool(
    data: bytes,
) -> List[Tuple[int, Optional[str], bool]]:
    offset = 0
    length, offset = _read_len_and_version(data, offset)
    rows: List[Tuple[int, Optional[str], bool]] = []
    for _ in range(length):
        row_id = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        if offset >= len(data):
            raise ValueError("AoS optstr payload truncated (tag)")
        tag = data[offset]
        offset += 1
        if tag not in (0, 1):
            raise ValueError(f"invalid optional string tag {tag}")
        if tag == 0:
            maybe_name = None
        else:
            size, offset = decode_varint(data, offset)
            name_bytes = data[offset : offset + size]
            offset += size
            maybe_name = name_bytes.decode("utf-8")
        if offset >= len(data):
            raise ValueError("AoS optstr payload truncated (flag)")
        flag = data[offset] != 0
        offset += 1
        rows.append((row_id, maybe_name, flag))
    if offset != len(data):
        raise ValueError("Trailing bytes after AoS optstr decode")
    return rows


def encode_rows_u64_optu32_bool(rows: Sequence[Tuple[int, Optional[int], bool]]) -> bytes:
    buf = bytearray()
    _write_len_and_version(buf, len(rows))
    for row_id, value, flag in rows:
        buf.extend(int(row_id).to_bytes(8, "little", signed=False))
        if value is None:
            buf.append(0)
        else:
            buf.append(1)
            buf.extend(int(value).to_bytes(4, "little", signed=False))
        buf.append(1 if flag else 0)
    return bytes(buf)


def decode_rows_u64_optu32_bool(
    data: bytes,
) -> List[Tuple[int, Optional[int], bool]]:
    offset = 0
    length, offset = _read_len_and_version(data, offset)
    rows: List[Tuple[int, Optional[int], bool]] = []
    for _ in range(length):
        row_id = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        if offset >= len(data):
            raise ValueError("AoS optu32 payload truncated (tag)")
        tag = data[offset]
        offset += 1
        if tag not in (0, 1):
            raise ValueError(f"invalid optional u32 tag {tag}")
        if tag == 0:
            value = None
        else:
            value = int.from_bytes(data[offset : offset + 4], "little", signed=False)
            offset += 4
        if offset >= len(data):
            raise ValueError("AoS optu32 payload truncated (flag)")
        flag = data[offset] != 0
        offset += 1
        rows.append((row_id, value, flag))
    if offset != len(data):
        raise ValueError("Trailing bytes after AoS optu32 decode")
    return rows


EnumMarker = Tuple[str, Union[int, str]]


def encode_rows_u64_enum_bool(rows: Sequence[Tuple[int, EnumMarker, bool]]) -> bytes:
    buf = bytearray()
    # Historical format omits the version byte to preserve goldens.
    _write_len(buf, len(rows))
    for row_id, enum_value, flag in rows:
        tag, payload = enum_value
        buf.extend(int(row_id).to_bytes(8, "little", signed=False))
        if tag == "name":
            buf.append(0)
            encoded = str(payload).encode("utf-8")
            _write_len(buf, len(encoded))
            buf.extend(encoded)
        elif tag == "code":
            buf.append(1)
            buf.extend(int(payload).to_bytes(4, "little", signed=False))
        else:
            raise ValueError(f"unknown enum marker tag {tag!r}")
        buf.append(1 if flag else 0)
    return bytes(buf)


def decode_rows_u64_enum_bool(data: bytes) -> List[Tuple[int, EnumMarker, bool]]:
    offset = 0
    length, offset = decode_varint(data, offset)
    rows: List[Tuple[int, EnumMarker, bool]] = []
    for _ in range(length):
        row_id = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        if offset >= len(data):
            raise ValueError("AoS enum payload truncated (tag)")
        tag = data[offset]
        offset += 1
        if tag == 0:
            name_len, offset = decode_varint(data, offset)
            name_bytes = data[offset : offset + name_len]
            offset += name_len
            marker: EnumMarker = ("name", name_bytes.decode("utf-8"))
        elif tag == 1:
            marker = (
                "code",
                int.from_bytes(data[offset : offset + 4], "little", signed=False),
            )
            offset += 4
        else:
            raise ValueError(f"invalid enum tag {tag}")
        if offset >= len(data):
            raise ValueError("AoS enum payload truncated (flag)")
        flag = data[offset] != 0
        offset += 1
        rows.append((row_id, marker, flag))
    if offset != len(data):
        raise ValueError("Trailing bytes after AoS enum decode")
    return rows


__all__ = [
    "encode_rows_u64_str_bool",
    "decode_rows_u64_str_bool",
    "encode_rows_u64_bytes_bool",
    "decode_rows_u64_bytes_bool",
    "encode_rows_u64_optstr_bool",
    "decode_rows_u64_optstr_bool",
    "encode_rows_u64_optu32_bool",
    "decode_rows_u64_optu32_bool",
    "encode_rows_u64_enum_bool",
    "decode_rows_u64_enum_bool",
]
