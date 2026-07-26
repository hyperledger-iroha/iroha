"""Columnar helpers for Norito adaptive layouts (subset of Rust implementation)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Sequence, Tuple, Union

from . import aos
from .heuristics import (
    AOS_NCB_SMALL_N,
    COMBO_DICT_AVG_LEN_MIN,
    COMBO_DICT_RATIO_MAX,
    COMBO_ENABLE_ID_DELTA,
    COMBO_ENABLE_NAME_DICT,
    COMBO_ID_DELTA_MIN_ROWS,
    COMBO_NO_DELTA_SMALL_N_IF_EMPTY,
)
from .varint import decode_varint, encode_varint

DESC_U64_STR_BOOL = 0x13
DESC_U64_DELTA_STR_BOOL = 0x53
# Optional column descriptors.
DESC_U64_OPTSTR_BOOL = 0x1B
DESC_U64_DELTA_OPTSTR_BOOL = 0x5B
DESC_U64_OPTU32_BOOL = 0x1C
DESC_U64_DELTA_OPTU32_BOOL = 0x5C
# Raw bytes descriptors.
DESC_U64_BYTES_BOOL = 0x14
DESC_U64_DELTA_BYTES_BOOL = 0x54
# Enum descriptors.
DESC_U64_ENUM_BOOL = 0x61
DESC_U64_DELTA_ENUM_BOOL = 0x63
DESC_U64_ENUM_BOOL_CODEDELTA = 0x65
DESC_U64_DELTA_ENUM_BOOL_CODEDELTA = 0x67
DESC_U64_ENUM_BOOL_DICT = 0xE1
DESC_U64_DELTA_ENUM_BOOL_DICT = 0xE3
DESC_U64_ENUM_BOOL_DICT_CODEDELTA = 0xE5
DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA = 0xE7

TAG_ENUM_NAME = 0
TAG_ENUM_CODE = 1


@dataclass(frozen=True)
class EnumName:
    """Enumeration value represented by a human-readable name."""

    value: str


@dataclass(frozen=True)
class EnumCode:
    """Enumeration value represented by an explicit numeric code."""

    value: int


EnumValue = Union[EnumName, EnumCode, str, int, Tuple[str, Union[str, int]]]
DESC_U64_DICT_STR_BOOL = 0x93

ADAPTIVE_TAG_AOS = 0x00
ADAPTIVE_TAG_NCB = 0x01


def encode_ncb_u64_str_bool(rows: Sequence[Tuple[int, str, bool]]) -> bytes:
    use_dict, dict_map, dict_vec = _build_dict(rows)
    if use_dict:
        assert dict_map is not None and dict_vec is not None
        return _encode_ncb_u64_dict_str_bool(rows, dict_map, dict_vec)
    if _should_use_id_delta(rows):
        return _encode_ncb_u64_str_bool_delta(rows)
    return _encode_ncb_u64_str_bool_offsets(rows)


def encode_rows_u64_str_bool_adaptive(rows: Sequence[Tuple[int, str, bool]]) -> bytes:
    if len(rows) <= AOS_NCB_SMALL_N:
        aos_body = aos.encode_rows_u64_str_bool(rows)
        columnar_body = encode_ncb_u64_str_bool(rows)
        if len(columnar_body) < len(aos_body):
            return bytes([ADAPTIVE_TAG_NCB]) + columnar_body
        return bytes([ADAPTIVE_TAG_AOS]) + aos_body
    if _should_use_columnar(len(rows)):
        return bytes([ADAPTIVE_TAG_NCB]) + encode_ncb_u64_str_bool(rows)
    return bytes([ADAPTIVE_TAG_AOS]) + aos.encode_rows_u64_str_bool(rows)


def decode_rows_u64_str_bool_adaptive(payload: bytes) -> List[Tuple[int, str, bool]]:
    if not payload:
        raise ValueError("adaptive payload is empty")
    tag = payload[0]
    body = payload[1:]
    if tag == ADAPTIVE_TAG_AOS:
        return aos.decode_rows_u64_str_bool(body)
    if tag == ADAPTIVE_TAG_NCB:
        return decode_ncb_u64_str_bool(body)
    raise ValueError(f"unknown adaptive tag {tag}")


def encode_ncb_u64_optstr_bool(
    rows: Sequence[Tuple[int, Optional[str], bool]],
) -> bytes:
    use_delta = _should_use_id_delta_opt(rows)
    values = [maybe for _, maybe, _ in rows]
    column_bytes, _present = _encode_opt_str_column(values)
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_DELTA_OPTSTR_BOOL if use_delta else DESC_U64_OPTSTR_BOOL)
    _pad_to(buf, 8)
    if use_delta and rows:
        first_id = int(rows[0][0])
        _write_u64(buf, first_id)
        prev = first_id
        for row_id, _, _ in rows[1:]:
            delta = int(row_id) - int(prev)
            buf.extend(encode_varint(_zigzag_encode(delta)))
            prev = int(row_id)
    else:
        for row_id, _, _ in rows:
            _write_u64(buf, row_id)
    buf.extend(column_bytes)
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def encode_rows_u64_optstr_bool_adaptive(
    rows: Sequence[Tuple[int, Optional[str], bool]],
) -> bytes:
    if len(rows) <= AOS_NCB_SMALL_N:
        aos_body = aos.encode_rows_u64_optstr_bool(rows)
        columnar_body = encode_ncb_u64_optstr_bool(rows)
        if len(columnar_body) < len(aos_body):
            return bytes([ADAPTIVE_TAG_NCB]) + columnar_body
        return bytes([ADAPTIVE_TAG_AOS]) + aos_body
    if _should_use_columnar(len(rows)):
        return bytes([ADAPTIVE_TAG_NCB]) + encode_ncb_u64_optstr_bool(rows)
    return bytes([ADAPTIVE_TAG_AOS]) + aos.encode_rows_u64_optstr_bool(rows)


def decode_rows_u64_optstr_bool_adaptive(
    payload: bytes,
) -> List[Tuple[int, Optional[str], bool]]:
    if not payload:
        raise ValueError("adaptive payload is empty")
    tag = payload[0]
    body = payload[1:]
    if tag == ADAPTIVE_TAG_AOS:
        return aos.decode_rows_u64_optstr_bool(body)
    if tag == ADAPTIVE_TAG_NCB:
        return decode_ncb_u64_optstr_bool(body)
    raise ValueError(f"unknown adaptive tag {tag}")


def decode_ncb_u64_optstr_bool(
    data: bytes,
) -> List[Tuple[int, Optional[str], bool]]:
    if len(data) < 5:
        raise ValueError("NCB optstr payload too short")
    offset = 0
    n = int.from_bytes(data[offset : offset + 4], "little", signed=False)
    offset += 4
    desc = data[offset]
    offset += 1
    if desc not in (DESC_U64_OPTSTR_BOOL, DESC_U64_DELTA_OPTSTR_BOOL):
        raise ValueError(f"Unsupported optstr descriptor 0x{desc:02x}")
    ids, offset = _decode_ids(data, offset, n, desc == DESC_U64_DELTA_OPTSTR_BOOL)
    values, offset = _decode_opt_str_column(data, offset, n)
    flags, offset = _decode_flags(data, offset, n)
    if offset != len(data):
        raise ValueError("Trailing bytes after optstr decode")
    rows: List[Tuple[int, Optional[str], bool]] = []
    for idx in range(n):
        rows.append((ids[idx], values[idx], flags[idx]))
    return rows


def encode_ncb_u64_optu32_bool(
    rows: Sequence[Tuple[int, Optional[int], bool]],
) -> bytes:
    use_delta = _should_use_id_delta_optu32(rows)
    values = [maybe for _, maybe, _ in rows]
    column_bytes = _encode_opt_u32_column(values)
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_DELTA_OPTU32_BOOL if use_delta else DESC_U64_OPTU32_BOOL)
    _pad_to(buf, 8)
    if use_delta and rows:
        first_id = int(rows[0][0])
        _write_u64(buf, first_id)
        prev = first_id
        for row_id, _, _ in rows[1:]:
            delta = int(row_id) - int(prev)
            buf.extend(encode_varint(_zigzag_encode(delta)))
            prev = int(row_id)
    else:
        for row_id, _, _ in rows:
            _write_u64(buf, row_id)
    buf.extend(column_bytes)
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def encode_rows_u64_optu32_bool_adaptive(
    rows: Sequence[Tuple[int, Optional[int], bool]],
) -> bytes:
    if len(rows) <= AOS_NCB_SMALL_N:
        aos_body = aos.encode_rows_u64_optu32_bool(rows)
        columnar_body = encode_ncb_u64_optu32_bool(rows)
        if len(columnar_body) < len(aos_body):
            return bytes([ADAPTIVE_TAG_NCB]) + columnar_body
        return bytes([ADAPTIVE_TAG_AOS]) + aos_body
    if _should_use_columnar(len(rows)):
        return bytes([ADAPTIVE_TAG_NCB]) + encode_ncb_u64_optu32_bool(rows)
    return bytes([ADAPTIVE_TAG_AOS]) + aos.encode_rows_u64_optu32_bool(rows)


def decode_rows_u64_optu32_bool_adaptive(
    payload: bytes,
) -> List[Tuple[int, Optional[int], bool]]:
    if not payload:
        raise ValueError("adaptive payload is empty")
    tag = payload[0]
    body = payload[1:]
    if tag == ADAPTIVE_TAG_AOS:
        return aos.decode_rows_u64_optu32_bool(body)
    if tag == ADAPTIVE_TAG_NCB:
        return decode_ncb_u64_optu32_bool(body)
    raise ValueError(f"unknown adaptive tag {tag}")


def decode_ncb_u64_optu32_bool(
    data: bytes,
) -> List[Tuple[int, Optional[int], bool]]:
    if len(data) < 5:
        raise ValueError("NCB optu32 payload too short")
    offset = 0
    n = int.from_bytes(data[offset : offset + 4], "little", signed=False)
    offset += 4
    desc = data[offset]
    offset += 1
    if desc not in (DESC_U64_OPTU32_BOOL, DESC_U64_DELTA_OPTU32_BOOL):
        raise ValueError(f"Unsupported optu32 descriptor 0x{desc:02x}")
    ids, offset = _decode_ids(data, offset, n, desc == DESC_U64_DELTA_OPTU32_BOOL)
    values, offset = _decode_opt_u32_column(data, offset, n)
    flags, offset = _decode_flags(data, offset, n)
    if offset != len(data):
        raise ValueError("Trailing bytes after optu32 decode")
    rows: List[Tuple[int, Optional[int], bool]] = []
    for idx in range(n):
        rows.append((ids[idx], values[idx], flags[idx]))
    return rows


def encode_ncb_u64_bytes_bool(rows: Sequence[Tuple[int, bytes, bool]]) -> bytes:
    use_delta = _should_use_id_delta_bytes(rows)
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_DELTA_BYTES_BOOL if use_delta else DESC_U64_BYTES_BOOL)
    _pad_to(buf, 8)
    if use_delta and rows:
        first_id = int(rows[0][0])
        _write_u64(buf, first_id)
        prev = first_id
        for row_id, _, _ in rows[1:]:
            delta = int(row_id) - int(prev)
            buf.extend(encode_varint(_zigzag_encode(delta)))
            prev = int(row_id)
    else:
        for row_id, _, _ in rows:
            _write_u64(buf, row_id)
    _pad_to(buf, 4)
    offsets: List[int] = [0]
    blob = bytearray()
    acc = 0
    for _, payload, _ in rows:
        acc += len(payload)
        offsets.append(acc)
        blob.extend(payload)
    for value in offsets:
        _write_u32(buf, value)
    buf.extend(blob)
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def encode_rows_u64_bytes_bool_adaptive(
    rows: Sequence[Tuple[int, bytes, bool]],
) -> bytes:
    if len(rows) <= AOS_NCB_SMALL_N:
        aos_body = aos.encode_rows_u64_bytes_bool(rows)
        columnar_body = encode_ncb_u64_bytes_bool(rows)
        if len(columnar_body) < len(aos_body):
            return bytes([ADAPTIVE_TAG_NCB]) + columnar_body
        return bytes([ADAPTIVE_TAG_AOS]) + aos_body
    if _should_use_columnar(len(rows)):
        return bytes([ADAPTIVE_TAG_NCB]) + encode_ncb_u64_bytes_bool(rows)
    return bytes([ADAPTIVE_TAG_AOS]) + aos.encode_rows_u64_bytes_bool(rows)


def decode_rows_u64_bytes_bool_adaptive(
    payload: bytes,
) -> List[Tuple[int, bytes, bool]]:
    if not payload:
        raise ValueError("adaptive payload is empty")
    tag = payload[0]
    body = payload[1:]
    if tag == ADAPTIVE_TAG_AOS:
        return aos.decode_rows_u64_bytes_bool(body)
    if tag == ADAPTIVE_TAG_NCB:
        return decode_ncb_u64_bytes_bool(body)
    raise ValueError(f"unknown adaptive tag {tag}")


def decode_ncb_u64_bytes_bool(data: bytes) -> List[Tuple[int, bytes, bool]]:
    if len(data) < 5:
        raise ValueError("NCB bytes payload too short")
    offset = 0
    n = int.from_bytes(data[offset : offset + 4], "little", signed=False)
    offset += 4
    desc = data[offset]
    offset += 1
    if desc not in (DESC_U64_BYTES_BOOL, DESC_U64_DELTA_BYTES_BOOL):
        raise ValueError(f"Unsupported bytes descriptor 0x{desc:02x}")
    ids, offset = _decode_ids(data, offset, n, desc == DESC_U64_DELTA_BYTES_BOOL)
    offset = _align(offset, 4)
    offsets = []
    for _ in range(n + 1):
        offsets.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
        offset += 4
    total_blob = offsets[-1] if offsets else 0
    blob = data[offset : offset + total_blob]
    if len(blob) != total_blob:
        raise ValueError("NCB bytes payload truncated (blob)")
    offset += total_blob
    flags, offset = _decode_flags(data, offset, n)
    if offset != len(data):
        raise ValueError("Trailing bytes after bytes decode")
    rows: List[Tuple[int, bytes, bool]] = []
    for idx in range(n):
        start = offsets[idx]
        end = offsets[idx + 1]
        rows.append((ids[idx], bytes(blob[start:end]), flags[idx]))
    return rows


def encode_ncb_u64_enum_bool(
    rows: Sequence[Tuple[int, EnumValue, bool]],
    *,
    use_delta_ids: Optional[bool] = None,
    use_name_dict: Optional[bool] = None,
    use_code_delta: Optional[bool] = None,
) -> bytes:
    normalised = [_normalise_enum_row(row) for row in rows]
    if use_delta_ids is None:
        use_delta_ids = _should_use_id_delta_enum(normalised)
    if use_name_dict is None:
        use_name_dict = _should_use_name_dict_enum(normalised)
    if use_code_delta is None:
        use_code_delta = _should_use_code_delta_enum(normalised)
    return _encode_ncb_u64_enum_bool(normalised, use_delta_ids, use_name_dict, use_code_delta)


def encode_rows_u64_enum_bool_adaptive(
    rows: Sequence[Tuple[int, EnumValue, bool]],
) -> bytes:
    normalised = [_normalise_enum_row(row) for row in rows]
    if len(rows) <= AOS_NCB_SMALL_N:
        ncb_body = _encode_ncb_u64_enum_bool(
            normalised,
            _should_use_id_delta_enum(normalised),
            _should_use_name_dict_enum(normalised),
            _should_use_code_delta_enum(normalised),
        )
        aos_body = aos.encode_rows_u64_enum_bool(normalised)
        if len(ncb_body) < len(aos_body):
            return bytes([ADAPTIVE_TAG_NCB]) + ncb_body
        return bytes([ADAPTIVE_TAG_AOS]) + aos_body
    if _should_use_columnar(len(rows)):
        body = _encode_ncb_u64_enum_bool(
            normalised,
            _should_use_id_delta_enum(normalised),
            _should_use_name_dict_enum(normalised),
            _should_use_code_delta_enum(normalised),
        )
        return bytes([ADAPTIVE_TAG_NCB]) + body
    return bytes([ADAPTIVE_TAG_AOS]) + aos.encode_rows_u64_enum_bool(normalised)


def decode_rows_u64_enum_bool_adaptive(
    payload: bytes,
) -> List[Tuple[int, Union[EnumName, EnumCode], bool]]:
    if not payload:
        raise ValueError("adaptive payload is empty")
    tag = payload[0]
    body = payload[1:]
    if tag == ADAPTIVE_TAG_AOS:
        decoded = aos.decode_rows_u64_enum_bool(body)
        return [
            (row_id, _marker_to_enum_value(marker), flag) for row_id, marker, flag in decoded
        ]
    if tag == ADAPTIVE_TAG_NCB:
        return decode_ncb_u64_enum_bool(body)
    raise ValueError(f"unknown adaptive tag {tag}")


def decode_ncb_u64_enum_bool(
    data: bytes,
) -> List[Tuple[int, Union[EnumName, EnumCode], bool]]:
    if len(data) < 5:
        raise ValueError("NCB enum payload too short")
    offset = 0
    n = int.from_bytes(data[offset : offset + 4], "little", signed=False)
    offset += 4
    desc = data[offset]
    offset += 1
    use_delta_ids, use_name_dict, use_code_delta = _parse_enum_descriptor(desc)
    ids, offset = _decode_ids(data, offset, n, use_delta_ids)
    tags = data[offset : offset + n]
    if len(tags) != n:
        raise ValueError("NCB enum payload truncated (tags)")
    offset += n
    name_count = sum(1 for tag in tags if tag == TAG_ENUM_NAME)
    code_count = n - name_count
    names: List[str]
    if use_name_dict:
        offset = _align(offset, 4)
        if offset + 4 > len(data):
            raise ValueError("NCB enum payload truncated (dict len)")
        dict_len = int.from_bytes(data[offset : offset + 4], "little", signed=False)
        offset += 4
        dict_offsets = []
        for _ in range(dict_len + 1):
            if offset + 4 > len(data):
                raise ValueError("NCB enum payload truncated (dict offsets)")
            dict_offsets.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        total = dict_offsets[-1] if dict_offsets else 0
        blob = data[offset : offset + total]
        if len(blob) != total:
            raise ValueError("NCB enum payload truncated (dict blob)")
        offset += total
        dictionary = []
        for idx in range(dict_len):
            start = dict_offsets[idx]
            end = dict_offsets[idx + 1]
            dictionary.append(blob[start:end].decode("utf-8"))
        offset = _align(offset, 4)
        indices = []
        for _ in range(name_count):
            if offset + 4 > len(data):
                raise ValueError("NCB enum payload truncated (dict codes)")
            indices.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        names = [dictionary[idx] for idx in indices]
    else:
        offset = _align(offset, 4)
        offsets = []
        for _ in range(name_count + 1):
            if offset + 4 > len(data):
                raise ValueError("NCB enum payload truncated (name offsets)")
            offsets.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        total = offsets[-1] if offsets else 0
        blob = data[offset : offset + total]
        if len(blob) != total:
            raise ValueError("NCB enum payload truncated (name blob)")
        offset += total
        names = []
        for idx in range(name_count):
            start = offsets[idx]
            end = offsets[idx + 1]
            names.append(blob[start:end].decode("utf-8"))
    offset = _align(offset, 4)
    codes: List[int] = []
    if code_count:
        if use_code_delta:
            if offset + 4 > len(data):
                raise ValueError("NCB enum payload truncated (code base)")
            base = int.from_bytes(data[offset : offset + 4], "little", signed=False)
            offset += 4
            codes.append(base)
            prev = base
            while len(codes) < code_count:
                value, offset = decode_varint(data, offset)
                delta = _zigzag_decode(value)
                prev = (prev + delta) & 0xFFFFFFFF
                codes.append(prev)
        else:
            for _ in range(code_count):
                if offset + 4 > len(data):
                    raise ValueError("NCB enum payload truncated (codes)")
                codes.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
                offset += 4
    flags, offset = _decode_flags(data, offset, n)
    if offset != len(data):
        raise ValueError("Trailing bytes after enum decode")
    rows: List[Tuple[int, Union[EnumName, EnumCode], bool]] = []
    name_idx = 0
    code_idx = 0
    for idx in range(n):
        tag = tags[idx]
        if tag == TAG_ENUM_NAME:
            value = EnumName(names[name_idx])
            name_idx += 1
        elif tag == TAG_ENUM_CODE:
            value = EnumCode(codes[code_idx])
            code_idx += 1
        else:
            raise ValueError(f"invalid enum tag {tag}")
        rows.append((ids[idx], value, flags[idx]))
    return rows


def _should_use_columnar(row_count: int) -> bool:
    """Mirror Rust's `should_use_columnar` heuristic for adaptive rows."""
    if row_count == 0:
        return False
    if row_count <= AOS_NCB_SMALL_N:
        # Small inputs use the two-pass probe above.
        return False
    return True


def decode_ncb_u64_str_bool(data: bytes) -> List[Tuple[int, str, bool]]:
    offset = 0
    if len(data) < 5:
        raise ValueError("NCB payload too short")
    n = int.from_bytes(data[offset : offset + 4], "little", signed=False)
    offset += 4
    desc = data[offset]
    offset += 1
    if desc not in (DESC_U64_STR_BOOL, DESC_U64_DELTA_STR_BOOL, DESC_U64_DICT_STR_BOOL):
        raise ValueError(f"Unsupported str/bool descriptor 0x{desc:02x}")
    ids: List[int]
    if desc == DESC_U64_DELTA_STR_BOOL:
        offset = _align(offset, 8)
        base = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        ids = [base]
        while len(ids) < n:
            value, offset = decode_varint(data, offset)
            delta = _zigzag_decode(value)
            ids.append((ids[-1] + delta) & 0xFFFFFFFFFFFFFFFF)
    else:
        offset = _align(offset, 8)
        ids = []
        for _ in range(n):
            ids.append(int.from_bytes(data[offset : offset + 8], "little", signed=False))
            offset += 8
    offset = _align(offset, 4)
    if desc == DESC_U64_DICT_STR_BOOL:
        dict_len = int.from_bytes(data[offset : offset + 4], "little", signed=False)
        offset += 4
        dict_offsets: List[int] = []
        for _ in range(dict_len + 1):
            dict_offsets.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        dict_blob = data[offset : offset + dict_offsets[-1]]
        offset += dict_offsets[-1]
        dictionary = []
        for i in range(dict_len):
            start = dict_offsets[i]
            end = dict_offsets[i + 1]
            dictionary.append(dict_blob[start:end].decode("utf-8"))
        offset = _align(offset, 4)
        codes: List[int] = []
        for _ in range(n):
            codes.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        names = [dictionary[code] for code in codes]
    else:
        offsets: List[int] = []
        for _ in range(n + 1):
            offsets.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
            offset += 4
        blob = data[offset : offset + offsets[-1]]
        offset += offsets[-1]
        names = []
        for i in range(n):
            start = offsets[i]
            end = offsets[i + 1]
            names.append(blob[start:end].decode("utf-8"))
    bit_bytes = (n + 7) // 8
    flags = data[offset : offset + bit_bytes]
    if len(flags) != bit_bytes:
        raise ValueError("NCB payload truncated (flags)")
    out: List[Tuple[int, str, bool]] = []
    for idx in range(n):
        byte = flags[idx // 8]
        bit = (byte >> (idx % 8)) & 1
        out.append((ids[idx], names[idx], bool(bit)))
    return out


def _encode_ncb_u64_str_bool_offsets(rows: Sequence[Tuple[int, str, bool]]) -> bytes:
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_STR_BOOL)
    _pad_to(buf, 8)
    for row_id, _, _ in rows:
        _write_u64(buf, row_id)
    _pad_to(buf, 4)
    offsets: List[int] = [0]
    blob = bytearray()
    acc = 0
    for _, name, _ in rows:
        encoded = name.encode("utf-8")
        acc += len(encoded)
        offsets.append(acc)
        blob.extend(encoded)
    for value in offsets:
        _write_u32(buf, value)
    buf.extend(blob)
    flags = _build_flag_bytes(rows)
    buf.extend(flags)
    return bytes(buf)


def _encode_ncb_u64_str_bool_delta(rows: Sequence[Tuple[int, str, bool]]) -> bytes:
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_DELTA_STR_BOOL)
    _pad_to(buf, 8)
    first_id = rows[0][0] if rows else 0
    _write_u64(buf, first_id)
    prev = first_id
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - int(prev)
        zz = _zigzag_encode(delta)
        buf.extend(encode_varint(zz))
        prev = row_id
    _pad_to(buf, 4)
    offsets: List[int] = [0]
    blob = bytearray()
    acc = 0
    for _, name, _ in rows:
        encoded = name.encode("utf-8")
        acc += len(encoded)
        offsets.append(acc)
        blob.extend(encoded)
    for value in offsets:
        _write_u32(buf, value)
    buf.extend(blob)
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def _encode_ncb_u64_dict_str_bool(
    rows: Sequence[Tuple[int, str, bool]],
    mapping: Dict[str, int],
    dictionary: List[str],
) -> bytes:
    buf = bytearray()
    _write_u32(buf, len(rows))
    buf.append(DESC_U64_DICT_STR_BOOL)
    _pad_to(buf, 8)
    for row_id, _, _ in rows:
        _write_u64(buf, row_id)
    _pad_to(buf, 4)
    _write_u32(buf, len(dictionary))
    offsets: List[int] = [0]
    blob = bytearray()
    acc = 0
    for item in dictionary:
        encoded = item.encode("utf-8")
        acc += len(encoded)
        offsets.append(acc)
        blob.extend(encoded)
    for value in offsets:
        _write_u32(buf, value)
    buf.extend(blob)
    _pad_to(buf, 4)
    for _, name, _ in rows:
        _write_u32(buf, mapping[name])
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def _build_flag_bytes(rows: Sequence[Tuple[object, ...]]) -> bytes:
    bit_bytes = (len(rows) + 7) // 8
    flags = bytearray(bit_bytes)
    for idx, row in enumerate(rows):
        if row[-1]:
            flags[idx // 8] |= 1 << (idx % 8)
    return bytes(flags)


def _build_dict(rows: Sequence[Tuple[int, str, bool]]) -> Tuple[bool, Dict[str, int] | None, List[str] | None]:
    if not COMBO_ENABLE_NAME_DICT or not rows:
        return False, None, None
    distinct: Dict[str, int] = {}
    total_len = 0
    for _, name, _ in rows:
        total_len += len(name)
        distinct.setdefault(name, len(distinct))
    distinct_count = len(distinct)
    ratio = distinct_count / len(rows)
    avg_len = total_len / len(rows)
    if ratio <= COMBO_DICT_RATIO_MAX and avg_len >= COMBO_DICT_AVG_LEN_MIN:
        dictionary = ["" for _ in range(distinct_count)]
        for name, idx in distinct.items():
            dictionary[idx] = name
        mapping = distinct
        return True, mapping, dictionary
    return False, None, None


def _should_use_id_delta(rows: Sequence[Tuple[int, str, bool]]) -> bool:
    if not COMBO_ENABLE_ID_DELTA or len(rows) < COMBO_ID_DELTA_MIN_ROWS:
        return False
    if len(rows) <= COMBO_NO_DELTA_SMALL_N_IF_EMPTY and any(not name for _, name, _ in rows):
        return False
    if not rows:
        return False
    prev = rows[0][0]
    varint_bytes = 0
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - int(prev)
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 8 * (len(rows) - 1):
            return False
        prev = row_id
    return True


def _should_use_id_delta_opt(rows: Sequence[Tuple[int, Optional[str], bool]]) -> bool:
    if len(rows) < 2:
        return False
    prev = int(rows[0][0])
    varint_bytes = 0
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - prev
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 8 * (len(rows) - 1):
            return False
        prev = int(row_id)
    return True


def _should_use_id_delta_optu32(rows: Sequence[Tuple[int, Optional[int], bool]]) -> bool:
    if len(rows) < 2:
        return False
    prev = int(rows[0][0])
    varint_bytes = 0
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - prev
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 8 * (len(rows) - 1):
            return False
        prev = int(row_id)
    return True


def _should_use_id_delta_bytes(rows: Sequence[Tuple[int, bytes, bool]]) -> bool:
    if not COMBO_ENABLE_ID_DELTA or len(rows) < COMBO_ID_DELTA_MIN_ROWS:
        return False
    if len(rows) <= COMBO_NO_DELTA_SMALL_N_IF_EMPTY and any(len(blob) == 0 for _, blob, _ in rows):
        return False
    prev = int(rows[0][0])
    varint_bytes = 0
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - prev
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 8 * (len(rows) - 1):
            return False
        prev = int(row_id)
    return True


def _should_use_id_delta_enum(rows: Sequence[Tuple[int, aos.EnumMarker, bool]]) -> bool:
    if len(rows) < 2:
        return False
    prev = int(rows[0][0])
    varint_bytes = 0
    for row_id, _, _ in rows[1:]:
        delta = int(row_id) - prev
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 8 * (len(rows) - 1):
            return False
        prev = int(row_id)
    return True


def _should_use_name_dict_enum(rows: Sequence[Tuple[int, aos.EnumMarker, bool]]) -> bool:
    total_len = 0
    name_count = 0
    distinct: Dict[str, int] = {}
    for _, marker, _ in rows:
        tag, value = marker
        if tag == "name":
            name = str(value)
            total_len += len(name)
            name_count += 1
            distinct.setdefault(name, len(distinct))
    if name_count == 0:
        return False
    ratio = len(distinct) / name_count
    avg_len = total_len / name_count
    return ratio <= COMBO_DICT_RATIO_MAX and avg_len >= COMBO_DICT_AVG_LEN_MIN


def _should_use_code_delta_enum(rows: Sequence[Tuple[int, aos.EnumMarker, bool]]) -> bool:
    codes: List[int] = []
    for _, marker, _ in rows:
        tag, value = marker
        if tag == "code":
            codes.append(int(value))
    if len(codes) < 2:
        return False
    prev = codes[0]
    varint_bytes = 0
    for code in codes[1:]:
        delta = int(code) - int(prev)
        if delta < -(1 << 63) or delta > (1 << 63) - 1:
            return False
        zz = _zigzag_encode(delta)
        varint_bytes += _varint_len(zz)
        if varint_bytes >= 4 * (len(codes) - 1):
            return False
        prev = int(code)
    return True


def _encode_opt_str_column(values: Sequence[Optional[str]]) -> Tuple[bytes, int]:
    n = len(values)
    bit_bytes = (n + 7) // 8
    buf = bytearray(bit_bytes)
    present = 0
    for idx, value in enumerate(values):
        if value is not None:
            buf[idx // 8] |= 1 << (idx % 8)
            present += 1
    _pad_to(buf, 4)
    offsets_pos = len(buf)
    buf.extend(b"\x00" * 4 * (present + 1))
    offsets: List[int] = [0]
    acc = 0
    for value in values:
        if value is not None:
            encoded = value.encode("utf-8")
            acc += len(encoded)
            offsets.append(acc)
            buf.extend(encoded)
    for idx, off in enumerate(offsets):
        start = offsets_pos + idx * 4
        buf[start : start + 4] = int(off).to_bytes(4, "little", signed=False)
    return bytes(buf), present


def _decode_opt_str_column(
    data: bytes, offset: int, n: int
) -> Tuple[List[Optional[str]], int]:
    bit_bytes = (n + 7) // 8
    start = offset
    bits = data[offset : offset + bit_bytes]
    if len(bits) != bit_bytes:
        raise ValueError("NCB optstr payload truncated (presence bits)")
    offset += bit_bytes
    present = _count_bits(bits, n)
    mis = (offset - start) % 4
    if mis:
        offset += 4 - mis
    offsets_bytes = data[offset : offset + 4 * (present + 1)]
    if len(offsets_bytes) != 4 * (present + 1):
        raise ValueError("NCB optstr payload truncated (offsets)")
    offset += 4 * (present + 1)
    last = int.from_bytes(offsets_bytes[-4:], "little", signed=False) if present else 0
    blob = data[offset : offset + last]
    if len(blob) != last:
        raise ValueError("NCB optstr payload truncated (blob)")
    offset += last
    present_values: List[str] = []
    for idx in range(present):
        start = int.from_bytes(offsets_bytes[idx * 4 : idx * 4 + 4], "little", signed=False)
        end = int.from_bytes(
            offsets_bytes[(idx + 1) * 4 : (idx + 1) * 4 + 4], "little", signed=False
        )
        present_values.append(blob[start:end].decode("utf-8"))
    expanded: List[Optional[str]] = []
    name_iter = iter(present_values)
    for bit in _iter_bits(bits, n):
        expanded.append(next(name_iter) if bit else None)
    return expanded, offset


def _encode_opt_u32_column(values: Sequence[Optional[int]]) -> bytes:
    n = len(values)
    bit_bytes = (n + 7) // 8
    buf = bytearray(bit_bytes)
    present_values: List[int] = []
    for idx, value in enumerate(values):
        if value is not None:
            buf[idx // 8] |= 1 << (idx % 8)
            present_values.append(int(value))
    _pad_to(buf, 4)
    for value in present_values:
        buf.extend(int(value).to_bytes(4, "little", signed=False))
    return bytes(buf)


def _decode_opt_u32_column(data: bytes, offset: int, n: int) -> Tuple[List[Optional[int]], int]:
    bit_bytes = (n + 7) // 8
    start = offset
    bits = data[offset : offset + bit_bytes]
    if len(bits) != bit_bytes:
        raise ValueError("NCB optu32 payload truncated (presence bits)")
    offset += bit_bytes
    present = _count_bits(bits, n)
    mis = (offset - start) % 4
    if mis:
        offset += 4 - mis
    values: List[int] = []
    for _ in range(present):
        if offset + 4 > len(data):
            raise ValueError("NCB optu32 payload truncated (values)")
        values.append(int.from_bytes(data[offset : offset + 4], "little", signed=False))
        offset += 4
    expanded: List[Optional[int]] = []
    val_iter = iter(values)
    for bit in _iter_bits(bits, n):
        expanded.append(next(val_iter) if bit else None)
    return expanded, offset


def _decode_ids(
    data: bytes, offset: int, n: int, use_delta: bool
) -> Tuple[List[int], int]:
    offset = _align(offset, 8)
    ids: List[int] = []
    if use_delta and n:
        base = int.from_bytes(data[offset : offset + 8], "little", signed=False)
        offset += 8
        ids.append(base)
        prev = base
        while len(ids) < n:
            value, offset = decode_varint(data, offset)
            delta = _zigzag_decode(value)
            prev = (prev + delta) & 0xFFFFFFFFFFFFFFFF
            ids.append(prev)
    else:
        for _ in range(n):
            ids.append(int.from_bytes(data[offset : offset + 8], "little", signed=False))
            offset += 8
    return ids, offset


def _decode_flags(data: bytes, offset: int, n: int) -> Tuple[List[bool], int]:
    bit_bytes = (n + 7) // 8
    bits = data[offset : offset + bit_bytes]
    if len(bits) != bit_bytes:
        raise ValueError("NCB payload truncated (flags)")
    offset += bit_bytes
    flags = [bit for bit in _iter_bits(bits, n)]
    return flags, offset


def _count_bits(bits: bytes, total: int) -> int:
    full_bytes = total // 8
    count = 0
    for idx in range(full_bytes):
        count += bin(bits[idx]).count("1")
    remaining = total % 8
    if remaining:
        mask = (1 << remaining) - 1
        count += bin(bits[full_bytes] & mask).count("1")
    return count


def _iter_bits(bits: bytes, total: int) -> Iterable[bool]:
    for idx in range(total):
        byte = bits[idx // 8]
        yield ((byte >> (idx % 8)) & 1) == 1


def _normalise_enum_row(
    row: Tuple[int, EnumValue, bool]
) -> Tuple[int, aos.EnumMarker, bool]:
    row_id, value, flag = row
    marker = _enum_value_to_marker(value)
    return int(row_id), marker, bool(flag)


def _enum_value_to_marker(value: EnumValue) -> aos.EnumMarker:
    if isinstance(value, EnumName):
        return ("name", value.value)
    if isinstance(value, EnumCode):
        return ("code", int(value.value))
    if isinstance(value, str):
        return ("name", value)
    if isinstance(value, int):
        return ("code", value)
    if isinstance(value, tuple) and len(value) == 2:
        tag, inner = value
        if tag == "name":
            return ("name", str(inner))
        if tag == "code":
            return ("code", int(inner))
    raise TypeError(f"Unsupported enum value representation: {value!r}")


def _marker_to_enum_value(marker: aos.EnumMarker) -> Union[EnumName, EnumCode]:
    tag, value = marker
    if tag == "name":
        return EnumName(str(value))
    if tag == "code":
        return EnumCode(int(value))
    raise ValueError(f"Invalid enum marker tag {tag!r}")


def _encode_ncb_u64_enum_bool(
    rows: Sequence[Tuple[int, aos.EnumMarker, bool]],
    use_delta_ids: bool,
    use_name_dict: bool,
    use_code_delta: bool,
) -> bytes:
    n = len(rows)
    buf = bytearray()
    _write_u32(buf, n)
    desc = (
        (DESC_U64_ENUM_BOOL if not use_name_dict else DESC_U64_ENUM_BOOL_DICT)
        | (0x02 if use_delta_ids else 0)
        | (0x04 if use_code_delta else 0)
    )
    buf.append(desc)
    _pad_to(buf, 8)
    if use_delta_ids and rows:
        first_id = int(rows[0][0])
        _write_u64(buf, first_id)
        prev = first_id
        for row_id, _, _ in rows[1:]:
            delta = int(row_id) - prev
            buf.extend(encode_varint(_zigzag_encode(delta)))
            prev = int(row_id)
    else:
        for row_id, _, _ in rows:
            _write_u64(buf, row_id)
    tags = bytearray()
    names: List[str] = []
    codes: List[int] = []
    for _, marker, _ in rows:
        tag, value = marker
        if tag == "name":
            tags.append(TAG_ENUM_NAME)
            names.append(str(value))
        elif tag == "code":
            tags.append(TAG_ENUM_CODE)
            codes.append(int(value))
        else:
            raise ValueError(f"Invalid enum marker tag {tag}")
    buf.extend(tags)
    if use_name_dict:
        dictionary: Dict[str, int] = {}
        dict_vec: List[str] = []
        for name in names:
            if name not in dictionary:
                dictionary[name] = len(dict_vec)
                dict_vec.append(name)
        _pad_to(buf, 4)
        _write_u32(buf, len(dict_vec))
        offsets: List[int] = [0]
        blob = bytearray()
        acc = 0
        for name in dict_vec:
            encoded = name.encode("utf-8")
            acc += len(encoded)
            offsets.append(acc)
            blob.extend(encoded)
        for off in offsets:
            _write_u32(buf, off)
        buf.extend(blob)
        _pad_to(buf, 4)
        for name in names:
            _write_u32(buf, dictionary[name])
    else:
        _pad_to(buf, 4)
        offsets: List[int] = [0]
        blob = bytearray()
        acc = 0
        for name in names:
            encoded = name.encode("utf-8")
            acc += len(encoded)
            offsets.append(acc)
            blob.extend(encoded)
        for off in offsets:
            _write_u32(buf, off)
        buf.extend(blob)
    _pad_to(buf, 4)
    if use_code_delta and codes:
        _write_u32(buf, codes[0])
        prev = codes[0]
        for code in codes[1:]:
            delta = int(code) - int(prev)
            buf.extend(encode_varint(_zigzag_encode(delta)))
            prev = int(code)
    else:
        for code in codes:
            _write_u32(buf, code)
    buf.extend(_build_flag_bytes(rows))
    return bytes(buf)


def _parse_enum_descriptor(desc: int) -> Tuple[bool, bool, bool]:
    mapping = {
        DESC_U64_ENUM_BOOL: (False, False, False),
        DESC_U64_DELTA_ENUM_BOOL: (True, False, False),
        DESC_U64_ENUM_BOOL_CODEDELTA: (False, False, True),
        DESC_U64_DELTA_ENUM_BOOL_CODEDELTA: (True, False, True),
        DESC_U64_ENUM_BOOL_DICT: (False, True, False),
        DESC_U64_DELTA_ENUM_BOOL_DICT: (True, True, False),
        DESC_U64_ENUM_BOOL_DICT_CODEDELTA: (False, True, True),
        DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA: (True, True, True),
    }
    try:
        return mapping[desc]
    except KeyError as exc:
        raise ValueError(f"Unsupported enum descriptor 0x{desc:02x}") from exc


def _zigzag_encode(value: int) -> int:
    return ((value << 1) ^ (value >> 63)) & 0xFFFFFFFFFFFFFFFF


def _zigzag_decode(value: int) -> int:
    return (value >> 1) ^ -(value & 1)


def _varint_len(value: int) -> int:
    length = 1
    while value >= 0x80:
        value >>= 7
        length += 1
    return length


def _write_u32(buf: bytearray, value: int) -> None:
    buf.extend(int(value).to_bytes(4, "little", signed=False))


def _write_u64(buf: bytearray, value: int) -> None:
    buf.extend(int(value).to_bytes(8, "little", signed=False))


def _pad_to(buf: bytearray, align: int) -> None:
    mis = len(buf) % align
    if mis:
        buf.extend(b"\x00" * (align - mis))


def _align(offset: int, align: int) -> int:
    mis = offset % align
    if mis:
        return offset + (align - mis)
    return offset


__all__ = [
    "EnumName",
    "EnumCode",
    "encode_ncb_u64_str_bool",
    "encode_rows_u64_str_bool_adaptive",
    "decode_rows_u64_str_bool_adaptive",
    "decode_ncb_u64_str_bool",
    "encode_ncb_u64_optstr_bool",
    "encode_rows_u64_optstr_bool_adaptive",
    "decode_rows_u64_optstr_bool_adaptive",
    "decode_ncb_u64_optstr_bool",
    "encode_ncb_u64_optu32_bool",
    "encode_rows_u64_optu32_bool_adaptive",
    "decode_rows_u64_optu32_bool_adaptive",
    "decode_ncb_u64_optu32_bool",
    "encode_ncb_u64_bytes_bool",
    "encode_rows_u64_bytes_bool_adaptive",
    "decode_rows_u64_bytes_bool_adaptive",
    "decode_ncb_u64_bytes_bool",
    "encode_ncb_u64_enum_bool",
    "encode_rows_u64_enum_bool_adaptive",
    "decode_rows_u64_enum_bool_adaptive",
    "decode_ncb_u64_enum_bool",
]
