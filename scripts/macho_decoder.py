#!/usr/bin/env python3
"""Strict, bounded Mach-O header and load-command decoder.

This is the repository's sole Mach-O decoder. It parses thin and fat (32- and
64-bit) images without executing or loading them and fails closed on any
out-of-bounds, overlapping, duplicated or non-canonical structure. Callers such
as ``sorafs_javascript_runtime_graph.py`` layer their own admission profile on
top of the decoded slices.
"""

from __future__ import annotations

import struct


class MachOError(RuntimeError):
    """A Mach-O input is malformed or unsafe."""


_MACHO_MAGIC = {
    b"\xce\xfa\xed\xfe": ("<", False),
    b"\xfe\xed\xfa\xce": (">", False),
    b"\xcf\xfa\xed\xfe": ("<", True),
    b"\xfe\xed\xfa\xcf": (">", True),
}
_MACHO_FAT_MAGIC = {
    b"\xca\xfe\xba\xbe": (">", False),
    b"\xbe\xba\xfe\xca": ("<", False),
    b"\xca\xfe\xba\xbf": (">", True),
    b"\xbf\xba\xfe\xca": ("<", True),
}
_MACHO_DEPENDENCY_COMMANDS = frozenset(
    {
        0x0C,  # LC_LOAD_DYLIB
        0x18 | 0x80000000,  # LC_LOAD_WEAK_DYLIB
        0x1F | 0x80000000,  # LC_REEXPORT_DYLIB
        0x20,  # LC_LAZY_LOAD_DYLIB
        0x23 | 0x80000000,  # LC_LOAD_UPWARD_DYLIB
    }
)
_MACHO_ID_DYLIB = 0x0D
_MACHO_RPATH = 0x1C | 0x80000000
_MACHO_CODE_SIGNATURE = 0x1D
_MACHO_SEGMENT = 0x01
_MACHO_SEGMENT_64 = 0x19
_MACHO_MAX_SLICES = 64
_MACHO_MAX_COMMANDS = 4096


def macho_uint(
    data: bytes, offset: int, size: int, endian: str, limit: int, label: str,
) -> int:
    """Read one bounded unsigned 32- or 64-bit field."""

    if offset < 0 or size not in {4, 8} or offset + size > limit:
        raise MachOError(f"Mach-O {label} is out of bounds")
    code = "I" if size == 4 else "Q"
    return int(struct.unpack_from(endian + code, data, offset)[0])


def macho_c_string(
    data: bytes, start: int, limit: int, label: str,
) -> str:
    """Read one NUL-terminated UTF-8 load-command string with zero padding."""

    if start < 0 or start >= limit:
        raise MachOError(f"Mach-O {label} offset is out of bounds")
    end = data.find(b"\0", start, limit)
    if end < 0:
        raise MachOError(f"Mach-O {label} is not NUL terminated")
    if any(data[end + 1:limit]):
        raise MachOError(f"Mach-O {label} has nonzero command padding")
    try:
        value = data[start:end].decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise MachOError(f"Mach-O {label} is not UTF-8") from error
    if not value or "\0" in value or "\n" in value or "\r" in value:
        raise MachOError(f"Mach-O {label} is unsafe")
    return value


def parse_macho_thin(
    data: bytes, offset: int, size: int, label: str,
) -> dict[str, object]:
    """Strictly parse one bounded thin Mach-O slice."""

    limit = offset + size
    if offset < 0 or size < 4 or limit > len(data):
        raise MachOError(f"Mach-O slice is out of bounds: {label}")
    format_value = _MACHO_MAGIC.get(data[offset:offset + 4])
    if format_value is None:
        raise MachOError(f"Mach-O slice has unsupported magic: {label}")
    endian, is_64 = format_value
    header_size = 32 if is_64 else 28
    if size < header_size:
        raise MachOError(f"Mach-O header is truncated: {label}")
    cpu_type = macho_uint(data, offset + 4, 4, endian, limit, "CPU type")
    cpu_subtype = macho_uint(
        data, offset + 8, 4, endian, limit, "CPU subtype",
    )
    file_type = macho_uint(data, offset + 12, 4, endian, limit, "file type")
    command_count = macho_uint(
        data, offset + 16, 4, endian, limit, "load-command count",
    )
    command_bytes = macho_uint(
        data, offset + 20, 4, endian, limit, "load-command byte length",
    )
    if command_count == 0 or command_count > _MACHO_MAX_COMMANDS:
        raise MachOError(f"Mach-O load-command count is invalid: {label}")
    command_start = offset + header_size
    command_end = command_start + command_bytes
    if command_end < command_start or command_end > limit:
        raise MachOError(f"Mach-O load-command table is out of bounds: {label}")

    commands: list[dict[str, object]] = []
    cursor = command_start
    code_signature: dict[str, int] | None = None
    linkedit: dict[str, int] | None = None
    for index in range(command_count):
        if cursor + 8 > command_end:
            raise MachOError(f"Mach-O load command is truncated: {label}")
        command = macho_uint(data, cursor, 4, endian, command_end, "command")
        command_size = macho_uint(
            data, cursor + 4, 4, endian, command_end, "command size",
        )
        if (
            command_size < 8
            or command_size % (8 if is_64 else 4) != 0
            or cursor + command_size > command_end
        ):
            raise MachOError(f"Mach-O load-command size is invalid: {label}")
        record: dict[str, object] = {
            "index": index,
            "command": command,
            "offset": cursor,
            "size": command_size,
            "raw": data[cursor:cursor + command_size],
        }
        if command in _MACHO_DEPENDENCY_COMMANDS | {_MACHO_ID_DYLIB}:
            if command_size < 24:
                raise MachOError(f"Mach-O dylib command is truncated: {label}")
            name_offset = macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size,
                "install-name",
            )
            if name_offset < 24:
                raise MachOError(f"Mach-O install-name offset is invalid: {label}")
            record.update(
                {
                    "name": macho_c_string(
                        data, cursor + name_offset, cursor + command_size,
                        "install-name",
                    ),
                    "name_offset": name_offset,
                    "timestamp": macho_uint(
                        data, cursor + 12, 4, endian, cursor + command_size,
                        "dylib timestamp",
                    ),
                    "current_version": macho_uint(
                        data, cursor + 16, 4, endian, cursor + command_size,
                        "dylib current version",
                    ),
                    "compatibility_version": macho_uint(
                        data, cursor + 20, 4, endian, cursor + command_size,
                        "dylib compatibility version",
                    ),
                }
            )
        elif command == _MACHO_RPATH:
            if command_size < 12:
                raise MachOError(f"Mach-O rpath command is truncated: {label}")
            path_offset = macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size, "rpath",
            )
            if path_offset < 12:
                raise MachOError(f"Mach-O rpath offset is invalid: {label}")
            record.update(
                {
                    "name": macho_c_string(
                        data, cursor + path_offset, cursor + command_size,
                        "rpath",
                    ),
                    "name_offset": path_offset,
                }
            )
        elif command == _MACHO_CODE_SIGNATURE:
            if command_size != 16 or code_signature is not None:
                raise MachOError(
                    f"Mach-O code-signature command is invalid: {label}"
                )
            data_offset = macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size,
                "code-signature offset",
            )
            data_size = macho_uint(
                data, cursor + 12, 4, endian, cursor + command_size,
                "code-signature size",
            )
            if (
                data_size == 0
                or data_offset < command_end - offset
                or data_offset + data_size > size
            ):
                raise MachOError(f"Mach-O code signature is out of bounds: {label}")
            code_signature = {
                "command_offset": cursor,
                "data_offset": offset + data_offset,
                "data_size": data_size,
            }
        elif command in {_MACHO_SEGMENT, _MACHO_SEGMENT_64}:
            segment_64 = command == _MACHO_SEGMENT_64
            minimum = 72 if segment_64 else 56
            if command_size < minimum:
                raise MachOError(f"Mach-O segment command is truncated: {label}")
            segment_name = data[cursor + 8:cursor + 24]
            zero = segment_name.find(b"\0")
            rendered_segment = segment_name if zero < 0 else segment_name[:zero]
            if rendered_segment == b"__LINKEDIT":
                if linkedit is not None:
                    raise MachOError(f"Mach-O __LINKEDIT is duplicated: {label}")
                word_size = 8 if segment_64 else 4
                file_offset_field = cursor + (40 if segment_64 else 32)
                file_size_field = cursor + (48 if segment_64 else 36)
                linkedit = {
                    "command_offset": cursor,
                    "vm_size_offset": cursor + (32 if segment_64 else 28),
                    "file_offset": macho_uint(
                        data, file_offset_field, word_size, endian,
                        cursor + command_size, "__LINKEDIT offset",
                    ),
                    "file_size_offset": file_size_field,
                    "file_size": macho_uint(
                        data, file_size_field, word_size, endian,
                        cursor + command_size, "__LINKEDIT size",
                    ),
                    "word_size": word_size,
                }
        commands.append(record)
        cursor += command_size
    if cursor != command_end:
        raise MachOError(f"Mach-O load-command byte length disagrees: {label}")
    if code_signature is not None:
        signature_end = code_signature["data_offset"] + code_signature["data_size"]
        if signature_end != limit:
            raise MachOError(f"Mach-O code signature is not final: {label}")
        if linkedit is None:
            raise MachOError(f"Mach-O signed image lacks __LINKEDIT: {label}")
        linkedit_end = (
            offset + linkedit["file_offset"] + linkedit["file_size"]
        )
        linkedit_start = offset + linkedit["file_offset"]
        if (
            linkedit_start < command_end
            or code_signature["data_offset"] < linkedit_start
            or signature_end > linkedit_end
            or linkedit_end != limit
        ):
            raise MachOError(
                f"Mach-O __LINKEDIT does not contain its signature: {label}"
            )
    return {
        "offset": offset,
        "size": size,
        "endian": endian,
        "is_64": is_64,
        "cpu_type": cpu_type,
        "cpu_subtype": cpu_subtype,
        "file_type": file_type,
        "header_size": header_size,
        "command_start": command_start,
        "command_end": command_end,
        "command_count": command_count,
        "command_bytes": command_bytes,
        "commands": commands,
        "code_signature": code_signature,
        "linkedit": linkedit,
    }


def parse_macho(data: bytes, label: str) -> list[dict[str, object]] | None:
    """Return strict slice metadata, or ``None`` for a non-Mach-O file."""

    if len(data) < 4:
        return None
    if data[:4] in _MACHO_MAGIC:
        return [parse_macho_thin(data, 0, len(data), label)]
    fat_format = _MACHO_FAT_MAGIC.get(data[:4])
    if fat_format is None:
        return None
    endian, fat_64 = fat_format
    if len(data) < 8:
        raise MachOError(f"Mach-O fat header is truncated: {label}")
    count = macho_uint(data, 4, 4, endian, len(data), "fat slice count")
    if count == 0 or count > _MACHO_MAX_SLICES:
        raise MachOError(f"Mach-O fat slice count is invalid: {label}")
    entry_size = 32 if fat_64 else 20
    table_end = 8 + count * entry_size
    if table_end > len(data):
        raise MachOError(f"Mach-O fat slice table is truncated: {label}")
    ranges: list[tuple[int, int]] = []
    slices: list[dict[str, object]] = []
    seen_architectures: set[tuple[int, int]] = set()
    for index in range(count):
        cursor = 8 + index * entry_size
        cpu_type = macho_uint(
            data, cursor, 4, endian, table_end, "fat CPU type",
        )
        cpu_subtype = macho_uint(
            data, cursor + 4, 4, endian, table_end, "fat CPU subtype",
        )
        word_size = 8 if fat_64 else 4
        slice_offset = macho_uint(
            data, cursor + 8, word_size, endian, table_end, "fat slice offset",
        )
        slice_size = macho_uint(
            data, cursor + 8 + word_size, word_size, endian, table_end,
            "fat slice size",
        )
        align_offset = cursor + (24 if fat_64 else 16)
        alignment = macho_uint(
            data, align_offset, 4, endian, table_end, "fat slice alignment",
        )
        if fat_64 and macho_uint(
            data, cursor + 28, 4, endian, table_end, "fat reserved field",
        ) != 0:
            raise MachOError(f"Mach-O fat reserved field is nonzero: {label}")
        if (
            slice_size == 0
            or slice_offset < table_end
            or slice_offset + slice_size > len(data)
            or alignment > 63
            or slice_offset % (1 << alignment) != 0
        ):
            raise MachOError(f"Mach-O fat slice is out of bounds: {label}")
        architecture = (cpu_type, cpu_subtype)
        if architecture in seen_architectures:
            raise MachOError(f"Mach-O fat architecture is duplicated: {label}")
        seen_architectures.add(architecture)
        ranges.append((slice_offset, slice_offset + slice_size))
        parsed = parse_macho_thin(data, slice_offset, slice_size, label)
        if (
            parsed["cpu_type"] != cpu_type
            or parsed["cpu_subtype"] != cpu_subtype
        ):
            raise MachOError(f"Mach-O fat architecture disagrees: {label}")
        slices.append(parsed)
    ordered = sorted(ranges)
    if any(left[1] > right[0] for left, right in zip(ordered, ordered[1:])):
        raise MachOError(f"Mach-O fat slices overlap: {label}")
    return slices
