"""CLI dispatch for the authenticated Sumeragi v2 release cache helper."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import selectors
import stat
import struct
import subprocess
import sys
import time


_MACHO_TOOL_PATHS = {
    "codesign": Path("/usr/bin/codesign"),
    "install_name_tool": Path("/usr/bin/install_name_tool"),
    "otool": Path("/usr/bin/otool"),
}
_MACHO_OUTPUT_BYTES = 1024 * 1024
_MACHO_ARTIFACT_BYTES = 256 * 1024 * 1024
_MACHO_TOOL_BYTES = 64 * 1024 * 1024
_MACHO_ARTIFACT_PATHS = {
    "launcher": "bin/python3",
    "trampoline": "Resources/Python.app/Contents/MacOS/Python",
}
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_MODE_RE = re.compile(r"[0-7]{4}")
_FRAMEWORK_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")


def _macho_rewrites(framework: str) -> dict[str, tuple[str, str]]:
    """Return canonical archive-local loader paths for one framework."""

    if _FRAMEWORK_RE.fullmatch(framework) is None:
        raise _MachOError("framework Python name is unsafe")
    return {
        "launcher": (
            _MACHO_ARTIFACT_PATHS["launcher"],
            f"@executable_path/../{framework}",
        ),
        "trampoline": (
            _MACHO_ARTIFACT_PATHS["trampoline"],
            f"@executable_path/../../../../{framework}",
        ),
    }


class _MachOError(RuntimeError):
    """A Mach-O input, closure, or transformation is unsafe."""


_MACHO_MAXIMUM_FILE_BYTES = _MACHO_ARTIFACT_BYTES
_MACHO_MAX_IMAGES = 4096
_MACHO_MAX_CANDIDATES = 100_000
_MACHO_IDENTITY_FIELDS = (
    "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
    "st_size", "st_mtime_ns", "st_ctime_ns",
)


def _macho_unchanged(before: os.stat_result, after: os.stat_result) -> bool:
    return all(
        getattr(before, field) == getattr(after, field)
        for field in _MACHO_IDENTITY_FIELDS
    )


def _macho_bounded_relative(value: str) -> None:
    encoded = os.fsencode(value)
    parts = PurePosixPath(value).parts
    if (
        len(encoded) > 4096
        or len(parts) > 128
        or any(len(os.fsencode(part)) > 255 for part in parts)
    ):
        raise _MachOError("Mach-O archive path exceeds its bound")


def _macho_path_is_omitted(
    path: str, omitted_paths: frozenset[str],
) -> bool:
    return any(
        path == omitted or path.startswith(f"{omitted}/")
        for omitted in omitted_paths
    )


def _canonical_macho_payload(document: dict[str, object]) -> bytes:
    return (
        json.dumps(
            document, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
        ).encode("utf-8")
        + b"\n"
    )


def _macho_read_regular(
    path: Path,
    label: str,
    *,
    maximum_bytes: int = _MACHO_MAXIMUM_FILE_BYTES,
    require_single_link: bool = True,
) -> tuple[bytes, os.stat_result]:
    """Read one bounded, stable, no-follow regular Mach-O input."""

    try:
        before = path.lstat()
    except OSError as error:
        raise _MachOError(f"{label} is unavailable") from error
    if (
        maximum_bytes <= 0
        or stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(before.st_mode) & 0o022
        or before.st_nlink < 1
        or (require_single_link and before.st_nlink != 1)
        or not 0 <= before.st_size <= maximum_bytes
    ):
        raise _MachOError(f"{label} metadata is unsafe")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise _MachOError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if not _macho_unchanged(before, opened):
            raise _MachOError(f"{label} changed while opened")
        payload = bytearray()
        while block := os.read(descriptor, 1024 * 1024):
            payload.extend(block)
            if len(payload) > maximum_bytes:
                raise _MachOError(f"{label} exceeds its bound")
        after = os.fstat(descriptor)
        if len(payload) != opened.st_size or not _macho_unchanged(opened, after):
            raise _MachOError(f"{label} changed while read")
    finally:
        os.close(descriptor)
    try:
        current = path.lstat()
    except OSError as error:
        raise _MachOError(f"{label} disappeared after reading") from error
    if not _macho_unchanged(after, current):
        raise _MachOError(f"{label} changed after reading")
    return bytes(payload), after


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


def _macho_uint(
    data: bytes, offset: int, size: int, endian: str, limit: int, label: str,
) -> int:
    if offset < 0 or size not in {4, 8} or offset + size > limit:
        raise _MachOError(f"Mach-O {label} is out of bounds")
    code = "I" if size == 4 else "Q"
    return int(struct.unpack_from(endian + code, data, offset)[0])


def _macho_c_string(
    data: bytes, start: int, limit: int, label: str,
) -> str:
    if start < 0 or start >= limit:
        raise _MachOError(f"Mach-O {label} offset is out of bounds")
    end = data.find(b"\0", start, limit)
    if end < 0:
        raise _MachOError(f"Mach-O {label} is not NUL terminated")
    if any(data[end + 1:limit]):
        raise _MachOError(f"Mach-O {label} has nonzero command padding")
    try:
        value = data[start:end].decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise _MachOError(f"Mach-O {label} is not UTF-8") from error
    if not value or "\0" in value or "\n" in value or "\r" in value:
        raise _MachOError(f"Mach-O {label} is unsafe")
    return value


def _parse_macho_thin(
    data: bytes, offset: int, size: int, label: str,
) -> dict[str, object]:
    """Strictly parse one bounded thin Mach-O slice."""

    limit = offset + size
    if offset < 0 or size < 4 or limit > len(data):
        raise _MachOError(f"Mach-O slice is out of bounds: {label}")
    format_value = _MACHO_MAGIC.get(data[offset:offset + 4])
    if format_value is None:
        raise _MachOError(f"Mach-O slice has unsupported magic: {label}")
    endian, is_64 = format_value
    header_size = 32 if is_64 else 28
    if size < header_size:
        raise _MachOError(f"Mach-O header is truncated: {label}")
    cpu_type = _macho_uint(data, offset + 4, 4, endian, limit, "CPU type")
    cpu_subtype = _macho_uint(
        data, offset + 8, 4, endian, limit, "CPU subtype",
    )
    file_type = _macho_uint(data, offset + 12, 4, endian, limit, "file type")
    command_count = _macho_uint(
        data, offset + 16, 4, endian, limit, "load-command count",
    )
    command_bytes = _macho_uint(
        data, offset + 20, 4, endian, limit, "load-command byte length",
    )
    if command_count == 0 or command_count > _MACHO_MAX_COMMANDS:
        raise _MachOError(f"Mach-O load-command count is invalid: {label}")
    command_start = offset + header_size
    command_end = command_start + command_bytes
    if command_end < command_start or command_end > limit:
        raise _MachOError(f"Mach-O load-command table is out of bounds: {label}")

    commands: list[dict[str, object]] = []
    cursor = command_start
    code_signature: dict[str, int] | None = None
    linkedit: dict[str, int] | None = None
    for index in range(command_count):
        if cursor + 8 > command_end:
            raise _MachOError(f"Mach-O load command is truncated: {label}")
        command = _macho_uint(data, cursor, 4, endian, command_end, "command")
        command_size = _macho_uint(
            data, cursor + 4, 4, endian, command_end, "command size",
        )
        if (
            command_size < 8
            or command_size % (8 if is_64 else 4) != 0
            or cursor + command_size > command_end
        ):
            raise _MachOError(f"Mach-O load-command size is invalid: {label}")
        record: dict[str, object] = {
            "index": index,
            "command": command,
            "offset": cursor,
            "size": command_size,
            "raw": data[cursor:cursor + command_size],
        }
        if command in _MACHO_DEPENDENCY_COMMANDS | {_MACHO_ID_DYLIB}:
            if command_size < 24:
                raise _MachOError(f"Mach-O dylib command is truncated: {label}")
            name_offset = _macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size,
                "install-name",
            )
            if name_offset < 24:
                raise _MachOError(f"Mach-O install-name offset is invalid: {label}")
            record.update(
                {
                    "name": _macho_c_string(
                        data, cursor + name_offset, cursor + command_size,
                        "install-name",
                    ),
                    "name_offset": name_offset,
                    "timestamp": _macho_uint(
                        data, cursor + 12, 4, endian, cursor + command_size,
                        "dylib timestamp",
                    ),
                    "current_version": _macho_uint(
                        data, cursor + 16, 4, endian, cursor + command_size,
                        "dylib current version",
                    ),
                    "compatibility_version": _macho_uint(
                        data, cursor + 20, 4, endian, cursor + command_size,
                        "dylib compatibility version",
                    ),
                }
            )
        elif command == _MACHO_RPATH:
            if command_size < 12:
                raise _MachOError(f"Mach-O rpath command is truncated: {label}")
            path_offset = _macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size, "rpath",
            )
            if path_offset < 12:
                raise _MachOError(f"Mach-O rpath offset is invalid: {label}")
            record.update(
                {
                    "name": _macho_c_string(
                        data, cursor + path_offset, cursor + command_size,
                        "rpath",
                    ),
                    "name_offset": path_offset,
                }
            )
        elif command == _MACHO_CODE_SIGNATURE:
            if command_size != 16 or code_signature is not None:
                raise _MachOError(
                    f"Mach-O code-signature command is invalid: {label}"
                )
            data_offset = _macho_uint(
                data, cursor + 8, 4, endian, cursor + command_size,
                "code-signature offset",
            )
            data_size = _macho_uint(
                data, cursor + 12, 4, endian, cursor + command_size,
                "code-signature size",
            )
            if (
                data_size == 0
                or data_offset < command_end - offset
                or data_offset + data_size > size
            ):
                raise _MachOError(f"Mach-O code signature is out of bounds: {label}")
            code_signature = {
                "command_offset": cursor,
                "data_offset": offset + data_offset,
                "data_size": data_size,
            }
        elif command in {_MACHO_SEGMENT, _MACHO_SEGMENT_64}:
            segment_64 = command == _MACHO_SEGMENT_64
            minimum = 72 if segment_64 else 56
            if command_size < minimum:
                raise _MachOError(f"Mach-O segment command is truncated: {label}")
            segment_name = data[cursor + 8:cursor + 24]
            zero = segment_name.find(b"\0")
            rendered_segment = segment_name if zero < 0 else segment_name[:zero]
            if rendered_segment == b"__LINKEDIT":
                if linkedit is not None:
                    raise _MachOError(f"Mach-O __LINKEDIT is duplicated: {label}")
                word_size = 8 if segment_64 else 4
                file_offset_field = cursor + (40 if segment_64 else 32)
                file_size_field = cursor + (48 if segment_64 else 36)
                linkedit = {
                    "command_offset": cursor,
                    "vm_size_offset": cursor + (32 if segment_64 else 28),
                    "file_offset": _macho_uint(
                        data, file_offset_field, word_size, endian,
                        cursor + command_size, "__LINKEDIT offset",
                    ),
                    "file_size_offset": file_size_field,
                    "file_size": _macho_uint(
                        data, file_size_field, word_size, endian,
                        cursor + command_size, "__LINKEDIT size",
                    ),
                    "word_size": word_size,
                }
        commands.append(record)
        cursor += command_size
    if cursor != command_end:
        raise _MachOError(f"Mach-O load-command byte length disagrees: {label}")
    if code_signature is not None:
        signature_end = code_signature["data_offset"] + code_signature["data_size"]
        if signature_end != limit:
            raise _MachOError(f"Mach-O code signature is not final: {label}")
        if linkedit is None:
            raise _MachOError(f"Mach-O signed image lacks __LINKEDIT: {label}")
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
            raise _MachOError(
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


def _parse_macho(data: bytes, label: str) -> list[dict[str, object]] | None:
    """Return strict slice metadata, or ``None`` for a non-Mach-O file."""

    if len(data) < 4:
        return None
    if data[:4] in _MACHO_MAGIC:
        return [_parse_macho_thin(data, 0, len(data), label)]
    fat_format = _MACHO_FAT_MAGIC.get(data[:4])
    if fat_format is None:
        return None
    endian, fat_64 = fat_format
    if len(data) < 8:
        raise _MachOError(f"Mach-O fat header is truncated: {label}")
    count = _macho_uint(data, 4, 4, endian, len(data), "fat slice count")
    if count == 0 or count > _MACHO_MAX_SLICES:
        raise _MachOError(f"Mach-O fat slice count is invalid: {label}")
    entry_size = 32 if fat_64 else 20
    table_end = 8 + count * entry_size
    if table_end > len(data):
        raise _MachOError(f"Mach-O fat slice table is truncated: {label}")
    ranges: list[tuple[int, int]] = []
    slices: list[dict[str, object]] = []
    seen_architectures: set[tuple[int, int]] = set()
    for index in range(count):
        cursor = 8 + index * entry_size
        cpu_type = _macho_uint(
            data, cursor, 4, endian, table_end, "fat CPU type",
        )
        cpu_subtype = _macho_uint(
            data, cursor + 4, 4, endian, table_end, "fat CPU subtype",
        )
        word_size = 8 if fat_64 else 4
        slice_offset = _macho_uint(
            data, cursor + 8, word_size, endian, table_end, "fat slice offset",
        )
        slice_size = _macho_uint(
            data, cursor + 8 + word_size, word_size, endian, table_end,
            "fat slice size",
        )
        align_offset = cursor + (24 if fat_64 else 16)
        alignment = _macho_uint(
            data, align_offset, 4, endian, table_end, "fat slice alignment",
        )
        if fat_64 and _macho_uint(
            data, cursor + 28, 4, endian, table_end, "fat reserved field",
        ) != 0:
            raise _MachOError(f"Mach-O fat reserved field is nonzero: {label}")
        if (
            slice_size == 0
            or slice_offset < table_end
            or slice_offset + slice_size > len(data)
            or alignment > 63
            or slice_offset % (1 << alignment) != 0
        ):
            raise _MachOError(f"Mach-O fat slice is out of bounds: {label}")
        architecture = (cpu_type, cpu_subtype)
        if architecture in seen_architectures:
            raise _MachOError(f"Mach-O fat architecture is duplicated: {label}")
        seen_architectures.add(architecture)
        ranges.append((slice_offset, slice_offset + slice_size))
        parsed = _parse_macho_thin(data, slice_offset, slice_size, label)
        if (
            parsed["cpu_type"] != cpu_type
            or parsed["cpu_subtype"] != cpu_subtype
        ):
            raise _MachOError(f"Mach-O fat architecture disagrees: {label}")
        slices.append(parsed)
    ordered = sorted(ranges)
    if any(left[1] > right[0] for left, right in zip(ordered, ordered[1:])):
        raise _MachOError(f"Mach-O fat slices overlap: {label}")
    return slices


_MACHO_SYSTEM_PREFIXES = ("/System/Library/", "/usr/lib/")
_MACHO_DEPENDENCY_DIRECTORY = "iroha-loader-deps"
_MACHO_SAFE_BASENAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")
_MACHO_TRANSCRIPT_FORMAT = "iroha-sumeragi-v2-framework-python-mach-o-transcript"
_MACHO_TOOL_ARCHIVES = {
    "install_name_tool": (
        "release-bootstrap.install-name-tool.v1",
        "mach-o-tools/bin/install_name_tool",
    ),
    "install_name_tool_library": (
        "release-bootstrap.install-name-tool-libcodedirectory.v1",
        "mach-o-tools/lib/libcodedirectory.dylib",
    ),
    "codesign": (
        "release-bootstrap.codesign.v1",
        "mach-o-tools/bin/codesign",
    ),
}


def _macho_digest_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8", "strict")).hexdigest()


def _macho_path_parts(value: str, label: str) -> tuple[str, ...]:
    path = PurePosixPath(value)
    if (
        not value
        or path.is_absolute()
        or path.as_posix() != value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise _MachOError(f"Mach-O archive path is unsafe: {label}")
    _macho_bounded_relative(value)
    return path.parts


def _macho_archive_join(parent: str, value: str, label: str) -> str:
    rendered = posixpath.normpath(posixpath.join(parent, value))
    if rendered in {"", ".", ".."} or rendered.startswith("../"):
        raise _MachOError(f"Mach-O install-name escapes its archive: {label}")
    _macho_path_parts(rendered, label)
    return rendered


def _macho_source_file(
    path: Path, archive_path: str, label: str,
) -> dict[str, object] | None:
    """Read one stable source and return it only when it is Mach-O."""

    _macho_path_parts(archive_path, label)
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise _MachOError(f"Mach-O dependency is unavailable: {label}") from error
    if resolved != path:
        path = resolved
    data, metadata = _macho_read_regular(
        path, label, require_single_link=False,
    )
    slices = _parse_macho(data, label)
    if slices is None:
        return None
    if (
        metadata.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or metadata.st_nlink != 1
    ):
        raise _MachOError(f"Mach-O source metadata is unsafe: {label}")
    return {
        "source": path,
        "archive_path": archive_path,
        "data": data,
        "metadata": metadata,
        "sha256": hashlib.sha256(data).hexdigest(),
        "slices": slices,
    }


def _framework_python_internal_macho_images(
    version_root: Path,
    source_python: Path,
    framework: str,
    stdlib_name: str,
) -> dict[Path, dict[str, object]]:
    """Read every Mach-O image in the isolated framework source closure."""

    omitted = frozenset({f"lib/{stdlib_name}/site-packages"})
    sources: list[tuple[Path, str]] = [
        (source_python, "bin/python3"),
        (version_root / framework, framework),
    ]
    for root_name in ("Resources", "lib"):
        root = version_root / root_name
        pending = [(root, root_name)]
        while pending:
            directory, relative = pending.pop()
            try:
                entries = tuple(sorted(os.scandir(directory), key=lambda item: item.name))
            except OSError as error:
                raise _MachOError(
                    f"could not discover framework Mach-O sources: {relative}"
                ) from error
            for entry in entries:
                child_relative = f"{relative}/{entry.name}"
                if _macho_path_is_omitted(child_relative, omitted):
                    continue
                try:
                    metadata = entry.stat(follow_symlinks=False)
                except OSError as error:
                    raise _MachOError(
                        f"framework Mach-O source is unavailable: {child_relative}"
                    ) from error
                if stat.S_ISDIR(metadata.st_mode):
                    pending.append((Path(entry.path), child_relative))
                elif stat.S_ISREG(metadata.st_mode):
                    sources.append((Path(entry.path), child_relative))
                elif not stat.S_ISLNK(metadata.st_mode):
                    raise _MachOError(
                        f"framework Mach-O source is special: {child_relative}"
                    )
                if len(sources) + len(pending) > _MACHO_MAX_CANDIDATES:
                    raise _MachOError("framework Mach-O source traversal is unbounded")

    images: dict[Path, dict[str, object]] = {}
    archive_paths: set[str] = set()
    for source, archive_path in sorted(sources, key=lambda item: item[1]):
        image = _macho_source_file(
            source, archive_path, f"framework Mach-O {archive_path}",
        )
        if image is None:
            continue
        resolved = source.resolve(strict=True)
        if resolved in images or archive_path in archive_paths:
            raise _MachOError("framework Mach-O image mapping is not unique")
        images[resolved] = image
        archive_paths.add(archive_path)
    return images


def _macho_rpaths(slices: list[dict[str, object]], label: str) -> tuple[str, ...]:
    observed: tuple[str, ...] | None = None
    for slice_value in slices:
        commands = slice_value["commands"]
        assert isinstance(commands, list)
        current = tuple(
            str(command["name"])
            for command in commands
            if command["command"] == _MACHO_RPATH
        )
        if observed is None:
            observed = current
        elif current != observed:
            raise _MachOError(f"Mach-O slice rpaths disagree: {label}")
    return observed or ()


def _resolve_macho_source_name(
    name: str,
    image: dict[str, object],
    source_python: Path,
    *,
    rpaths: tuple[str, ...],
) -> Path | None:
    """Resolve one non-system source dependency without loader fallbacks."""

    if name.startswith(_MACHO_SYSTEM_PREFIXES):
        return None
    source = image["source"]
    slices = image["slices"]
    assert isinstance(source, Path) and isinstance(slices, list)
    file_types = {int(slice_value["file_type"]) for slice_value in slices}
    if len(file_types) != 1:
        raise _MachOError("Mach-O slice file types disagree")
    executable_parent = source.parent if file_types == {2} else source_python.parent

    def expand(value: str) -> Path | None:
        if value.startswith("@loader_path/"):
            return source.parent / value.removeprefix("@loader_path/")
        if value.startswith("@executable_path/"):
            return executable_parent / value.removeprefix("@executable_path/")
        if value.startswith("/"):
            return Path(value)
        return None

    candidates: list[Path] = []
    if name.startswith("@rpath/"):
        suffix = name.removeprefix("@rpath/")
        for rpath in rpaths:
            expanded = expand(rpath)
            if expanded is not None:
                candidates.append(expanded / suffix)
    else:
        expanded = expand(name)
        if expanded is not None:
            candidates.append(expanded)
    resolved: list[Path] = []
    for candidate in candidates:
        try:
            value = candidate.resolve(strict=True)
        except OSError:
            continue
        if value not in resolved:
            resolved.append(value)
    if len(resolved) != 1:
        raise _MachOError(
            "Mach-O dependency does not resolve to exactly one source: "
            f"{image['archive_path']}"
        )
    return resolved[0]


def _framework_python_macho_closure(
    version_root: Path,
    source_python: Path,
    framework: str,
    stdlib_name: str,
) -> tuple[dict[Path, dict[str, object]], dict[str, Path]]:
    """Discover the exact recursive non-system framework loader closure."""

    images = _framework_python_internal_macho_images(
        version_root, source_python, framework, stdlib_name,
    )
    by_archive = {
        str(image["archive_path"]): source for source, image in images.items()
    }
    pending = list(sorted(images))
    external_sources: dict[str, Path] = {}
    while pending:
        source = pending.pop(0)
        image = images[source]
        slices = image["slices"]
        assert isinstance(slices, list)
        rpaths = _macho_rpaths(slices, str(image["archive_path"]))
        dependency_names: set[str] = set()
        for slice_value in slices:
            commands = slice_value["commands"]
            assert isinstance(commands, list)
            for command in commands:
                if command["command"] in _MACHO_DEPENDENCY_COMMANDS:
                    dependency_names.add(str(command["name"]))
        for name in sorted(dependency_names):
            target = _resolve_macho_source_name(
                name, image, source_python, rpaths=rpaths,
            )
            if target is None or target in images:
                continue
            basename = target.name
            if _MACHO_SAFE_BASENAME_RE.fullmatch(basename) is None:
                raise _MachOError("Mach-O dependency basename is unsafe")
            target_data, target_metadata = _macho_read_regular(
                target, "external framework Mach-O dependency",
            )
            target_digest = hashlib.sha256(target_data).hexdigest()
            archive_path = (
                f"lib/{stdlib_name}/{_MACHO_DEPENDENCY_DIRECTORY}/"
                f"{target_digest}-{basename}"
            )
            if archive_path in by_archive and by_archive[archive_path] != target:
                raise _MachOError("Mach-O dependency archive name collides")
            target_image = _macho_source_file(
                target, archive_path, "external framework Mach-O dependency",
            )
            if target_image is None:
                raise _MachOError("external framework dependency is not Mach-O")
            if (
                target_image["metadata"].st_dev != target_metadata.st_dev
                or target_image["metadata"].st_ino != target_metadata.st_ino
                or target_image["sha256"] != target_digest
            ):
                raise _MachOError("external framework dependency changed")
            images[target] = target_image
            if len(images) > _MACHO_MAX_IMAGES:
                raise _MachOError("framework Mach-O closure contains too many images")
            by_archive[archive_path] = target
            external_sources[archive_path] = target
            pending.append(target)
            pending.sort()
    return images, external_sources


def _macho_relative_install_name(source_archive: str, target_archive: str) -> str:
    relative = posixpath.relpath(
        target_archive, posixpath.dirname(source_archive) or ".",
    )
    if relative in {"", ".", ".."}:
        raise _MachOError("Mach-O relative install-name is unsafe")
    return "@loader_path/" + relative


def _framework_python_macho_plan(
    images: dict[Path, dict[str, object]], source_python: Path,
    *, force_sign_paths: frozenset[str] = frozenset(),
) -> list[dict[str, object]]:
    """Return the deterministic path-free rewrite plan for the closure."""

    source_to_archive = {
        source: str(image["archive_path"]) for source, image in images.items()
    }
    plan: list[dict[str, object]] = []
    for source, image in sorted(
        images.items(), key=lambda item: str(item[1]["archive_path"]),
    ):
        archive_path = str(image["archive_path"])
        slices = image["slices"]
        assert isinstance(slices, list)
        rpaths = _macho_rpaths(slices, archive_path)
        slice_operations: list[tuple[tuple[object, ...], ...]] = []
        for slice_value in slices:
            commands = slice_value["commands"]
            assert isinstance(commands, list)
            operations: list[tuple[object, ...]] = []
            for command in commands:
                command_value = int(command["command"])
                if command_value in _MACHO_DEPENDENCY_COMMANDS:
                    old = str(command["name"])
                    target = _resolve_macho_source_name(
                        old, image, source_python, rpaths=rpaths,
                    )
                    if target is None:
                        continue
                    target_archive = source_to_archive.get(target)
                    if target_archive is None:
                        raise _MachOError("Mach-O dependency escaped its closure")
                    if archive_path == "bin/python3":
                        replacement = "@executable_path/../" + target_archive
                    elif archive_path == "Resources/Python.app/Contents/MacOS/Python":
                        replacement = "@executable_path/../../../../" + target_archive
                    else:
                        replacement = _macho_relative_install_name(
                            archive_path, target_archive,
                        )
                    if old != replacement:
                        operations.append(("change", old, replacement))
            slice_operations.append(tuple(sorted(set(operations))))
        if not slice_operations:
            continue
        if any(value != slice_operations[0] for value in slice_operations[1:]):
            raise _MachOError(
                f"Mach-O slice rewrite plans disagree: {archive_path}"
            )
        operations = slice_operations[0]
        if operations or archive_path in force_sign_paths or archive_path in _MACHO_ARTIFACT_PATHS.values():
            plan.append(
                {
                    "path": archive_path,
                    "source": source,
                    "source_mode": format(
                        stat.S_IMODE(image["metadata"].st_mode), "04o",
                    ),
                    "source_sha256": image["sha256"],
                    "source_size": len(image["data"]),
                    "operations": operations,
                }
            )
    return plan


def _validate_framework_python_macho_plan(
    plan: list[dict[str, object]], version_root: Path, framework: str,
) -> None:
    """Require signed launchers and unique deterministic dependent rewrites."""

    paths = [item.get("path") for item in plan if isinstance(item, dict)]
    if len(paths) != len(plan) or paths != sorted(paths) or len(set(paths)) != len(paths):
        raise _MachOError("framework Mach-O rewrite plan is not deterministic")
    source_name = str(version_root / framework)
    launcher_operations = {
        "bin/python3": (
            ("change", source_name, "@executable_path/../" + framework),
        ),
        "Resources/Python.app/Contents/MacOS/Python": (
            (
                "change",
                source_name,
                "@executable_path/../../../../" + framework,
            ),
        ),
    }
    for path, expected in launcher_operations.items():
        matches = [item for item in plan if item.get("path") == path]
        if len(matches) != 1 or matches[0].get("operations") not in {(), expected}:
            raise _MachOError(
                f"framework Mach-O launcher rewrite is not exact: {path}"
            )
    for item in plan:
        path = item.get("path")
        operations = item.get("operations")
        if (
            not isinstance(path, str)
            or not isinstance(item.get("source"), Path)
            or not isinstance(item.get("source_mode"), str)
            or _MODE_RE.fullmatch(item["source_mode"]) is None
            or int(item["source_mode"], 8) & 0o022
            or not int(item["source_mode"], 8) & 0o444
            or not isinstance(item.get("source_sha256"), str)
            or re.fullmatch(r"[0-9a-f]{64}", item["source_sha256"]) is None
            or type(item.get("source_size")) is not int
            or not 0 <= item["source_size"] <= _MACHO_MAXIMUM_FILE_BYTES
            or not isinstance(operations, tuple)
            or any(
                not isinstance(operation, tuple)
                or len(operation) != 3
                or operation[0] != "change"
                or not isinstance(operation[1], str)
                or not isinstance(operation[2], str)
                or not operation[2].startswith(("@loader_path/", "@executable_path/"))
                for operation in operations
            )
        ):
            raise _MachOError("framework Mach-O rewrite plan is malformed")
        _macho_path_parts(path, "framework Mach-O rewrite plan")


def _macho_tool_snapshot(
    path: Path, expected_sha256: str, label: str, *, executable: bool,
) -> dict[str, object]:
    if (
        not path.is_absolute()
        or Path(os.path.abspath(path)) != path
        or path.resolve(strict=True) != path
        or re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
    ):
        raise _MachOError(f"{label} path or digest is not exact")
    data, metadata = _macho_read_regular(
        path, label, maximum_bytes=_MACHO_TOOL_BYTES,
        require_single_link=False,
    )
    digest = hashlib.sha256(data).hexdigest()
    if (
        digest != expected_sha256
        or metadata.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or metadata.st_nlink < 1
        or (executable and metadata.st_mode & 0o111 == 0)
    ):
        raise _MachOError(f"{label} is not one authenticated tool input")
    slices = _parse_macho(data, label)
    if slices is None:
        raise _MachOError(f"{label} is not Mach-O")
    return {
        "path": path,
        "data": data,
        "metadata": metadata,
        "sha256": digest,
        "slices": slices,
    }


def _macho_tool_dependency_names(
    snapshot: dict[str, object], label: str,
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    slices = snapshot["slices"]
    assert isinstance(slices, list)
    dependencies: tuple[str, ...] | None = None
    rpaths: tuple[str, ...] | None = None
    for slice_value in slices:
        commands = slice_value["commands"]
        assert isinstance(commands, list)
        current_dependencies = tuple(
            str(command["name"])
            for command in commands
            if command["command"] in _MACHO_DEPENDENCY_COMMANDS
        )
        current_rpaths = tuple(
            str(command["name"])
            for command in commands
            if command["command"] == _MACHO_RPATH
        )
        if dependencies is None:
            dependencies = current_dependencies
            rpaths = current_rpaths
        elif dependencies != current_dependencies or rpaths != current_rpaths:
            raise _MachOError(f"{label} Mach-O slices disagree")
    return dependencies or (), rpaths or ()


def _validate_framework_macho_tools(
    install_name_tool: Path,
    expected_install_name_tool_sha256: str,
    install_name_tool_library: Path,
    expected_install_name_tool_library_sha256: str,
    codesign: Path,
    expected_codesign_sha256: str,
) -> dict[str, dict[str, object]]:
    """Authenticate the exact closed Mach-O rewrite/sign tool inputs."""

    snapshots = {
        "install_name_tool": _macho_tool_snapshot(
            install_name_tool, expected_install_name_tool_sha256,
            "install_name_tool", executable=True,
        ),
        "install_name_tool_library": _macho_tool_snapshot(
            install_name_tool_library,
            expected_install_name_tool_library_sha256,
            "install_name_tool libcodedirectory", executable=False,
        ),
        "codesign": _macho_tool_snapshot(
            codesign, expected_codesign_sha256, "codesign", executable=True,
        ),
    }
    expected_library = (
        install_name_tool.parent.parent / "lib" / "libcodedirectory.dylib"
    )
    if install_name_tool_library != expected_library:
        raise _MachOError(
            "install_name_tool libcodedirectory is not its exact adjacent closure"
        )
    install_dependencies, install_rpaths = _macho_tool_dependency_names(
        snapshots["install_name_tool"], "install_name_tool",
    )
    if (
        "@rpath/libcodedirectory.dylib" not in install_dependencies
        or install_rpaths != ("@executable_path/../lib",)
        or any(
            not name.startswith(_MACHO_SYSTEM_PREFIXES)
            and name != "@rpath/libcodedirectory.dylib"
            for name in install_dependencies
        )
    ):
        raise _MachOError("install_name_tool loader closure is not exact")
    library_dependencies, _ = _macho_tool_dependency_names(
        snapshots["install_name_tool_library"],
        "install_name_tool libcodedirectory",
    )
    codesign_dependencies, _ = _macho_tool_dependency_names(
        snapshots["codesign"], "codesign",
    )
    if any(
        not name.startswith(_MACHO_SYSTEM_PREFIXES)
        for name in (*library_dependencies, *codesign_dependencies)
    ):
        raise _MachOError("Mach-O signing tool closure leaves system libraries")
    return snapshots


def _path_free_macho_tool_records(
    tools: dict[str, dict[str, object]],
) -> dict[str, dict[str, object]]:
    if set(tools) != set(_MACHO_TOOL_ARCHIVES):
        raise _MachOError("framework Mach-O tool set is not exact")
    records: dict[str, dict[str, object]] = {}
    for name in sorted(tools):
        metadata = tools[name]["metadata"]
        data = tools[name]["data"]
        assert isinstance(metadata, os.stat_result) and isinstance(data, bytes)
        archive_id, archive_name = _MACHO_TOOL_ARCHIVES[name]
        records[name] = {
            "archive_id": archive_id,
            "archive_name": archive_name,
            "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
            "sha256": tools[name]["sha256"],
            "size_bytes": len(data),
        }
    return records


def _run_framework_macho_tool(
    executable: Path,
    arguments: list[str],
    sanitized_arguments: list[str],
    *,
    runtime_root: Path,
    operation: str,
    required_stderr_line: bytes | None = None,
) -> dict[str, object]:
    """Run one authenticated tool under a closed environment and attest it."""

    argv = [str(executable), *arguments]
    result = subprocess.run(
        argv,
        cwd=runtime_root,
        env={
            "HOME": str(runtime_root.parent),
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": str(executable.parent),
            "TMPDIR": str(runtime_root.parent),
        },
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=30,
    )
    if (
        result.returncode != 0
        or result.stdout != b""
        or len(result.stderr) > 64 * 1024
        or (
            required_stderr_line is not None
            and result.stderr.splitlines().count(required_stderr_line) != 1
        )
    ):
        raise _MachOError(f"framework Mach-O {operation} failed")
    actual_argv = _canonical_macho_payload({"argv": argv})
    return {
        "operation": operation,
        "argv": [operation, *sanitized_arguments],
        "argv_sha256": hashlib.sha256(actual_argv).hexdigest(),
        "exit_status": result.returncode,
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "stdout_size_bytes": len(result.stdout),
        "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
        "stderr_size_bytes": len(result.stderr),
    }


def _runtime_macho_target(
    name: str,
    archive_path: str,
    file_type: int,
    rpaths: tuple[str, ...],
    image_paths: set[str],
) -> str | None:
    if name.startswith(_MACHO_SYSTEM_PREFIXES):
        return None
    parent = posixpath.dirname(archive_path)
    executable_parent = parent if file_type == 2 else "bin"

    def expand(value: str) -> str | None:
        if value.startswith("@loader_path/"):
            return _macho_archive_join(
                parent, value.removeprefix("@loader_path/"), archive_path,
            )
        if value.startswith("@executable_path/"):
            return _macho_archive_join(
                executable_parent,
                value.removeprefix("@executable_path/"), archive_path,
            )
        return None

    candidates: list[str] = []
    if name.startswith("@rpath/"):
        suffix = name.removeprefix("@rpath/")
        for rpath in rpaths:
            expanded = expand(rpath)
            if expanded is not None:
                candidate = _macho_archive_join(expanded, suffix, archive_path)
                if candidate in image_paths and candidate not in candidates:
                    candidates.append(candidate)
    else:
        expanded = expand(name)
        if expanded is not None and expanded in image_paths:
            candidates.append(expanded)
    if len(candidates) != 1:
        raise _MachOError(
            f"archived Mach-O dependency is not internally exact: {archive_path}"
        )
    return candidates[0]


def _framework_runtime_macho_projection(
    runtime_root: Path,
    framework: str,
    expected_paths: set[str] | None = None,
) -> dict[str, object]:
    """Independently parse and close every Mach-O dependency in the archive."""

    image_values: dict[str, tuple[bytes, list[dict[str, object]]]] = {}
    if expected_paths is not None:
        if (
            not expected_paths
            or len(expected_paths) > _MACHO_MAX_IMAGES
            or any(not isinstance(path, str) for path in expected_paths)
        ):
            raise _MachOError("archived Mach-O path set is not exact")
        for archive_path in sorted(expected_paths):
            _macho_path_parts(archive_path, "archived Mach-O image")
            data, _ = _macho_read_regular(
                runtime_root / archive_path,
                f"archived Mach-O candidate {archive_path}",
            )
            slices = _parse_macho(data, archive_path)
            if slices is None:
                raise _MachOError(
                    f"archived closure member is not Mach-O: {archive_path}"
                )
            image_values[archive_path] = (data, slices)
    else:
        pending = [(runtime_root, "")]
        while pending:
            directory, relative = pending.pop()
            try:
                entries = tuple(
                    sorted(os.scandir(directory), key=lambda item: item.name)
                )
            except OSError as error:
                raise _MachOError("archived Mach-O closure is unreadable") from error
            for entry in entries:
                archive_path = (
                    entry.name if not relative else f"{relative}/{entry.name}"
                )
                metadata = entry.stat(follow_symlinks=False)
                if stat.S_ISDIR(metadata.st_mode):
                    pending.append((Path(entry.path), archive_path))
                elif stat.S_ISREG(metadata.st_mode):
                    data, after = _macho_read_regular(
                        Path(entry.path),
                        f"archived Mach-O candidate {archive_path}",
                    )
                    if (metadata.st_dev, metadata.st_ino) != (
                        after.st_dev, after.st_ino,
                    ):
                        raise _MachOError("archived Mach-O candidate changed")
                    slices = _parse_macho(data, archive_path)
                    if slices is not None:
                        image_values[archive_path] = (data, slices)
                elif not stat.S_ISLNK(metadata.st_mode):
                    raise _MachOError(
                        "archived Mach-O closure contains a special entry"
                    )
                if len(image_values) + len(pending) > _MACHO_MAX_IMAGES:
                    raise _MachOError("archived Mach-O traversal is unbounded")
    image_paths = set(image_values)
    required = {
        "bin/python3", framework,
        "Resources/Python.app/Contents/MacOS/Python",
    }
    if not required <= image_paths:
        raise _MachOError("archived Mach-O indispensable closure is incomplete")
    images: list[dict[str, object]] = []
    for archive_path in sorted(image_values):
        data, slices = image_values[archive_path]
        slice_records: list[dict[str, object]] = []
        for slice_value in slices:
            commands = slice_value["commands"]
            assert isinstance(commands, list)
            rpaths = tuple(
                str(command["name"])
                for command in commands
                if command["command"] == _MACHO_RPATH
            )
            dependencies: list[dict[str, object]] = []
            for command in commands:
                if command["command"] not in _MACHO_DEPENDENCY_COMMANDS:
                    continue
                install_name = str(command["name"])
                target = _runtime_macho_target(
                    install_name, archive_path, int(slice_value["file_type"]),
                    rpaths, image_paths,
                )
                if target is None:
                    dependency = {
                        "command": command["command"],
                        "binding": "system",
                        "install_name_sha256": _macho_digest_text(install_name),
                    }
                else:
                    dependency = {
                        "command": command["command"],
                        "binding": "archive",
                        "install_name": install_name,
                        "target": target,
                    }
                dependencies.append(dependency)
            if slice_value["code_signature"] is None:
                raise _MachOError(
                    f"archived Mach-O image is unsigned: {archive_path}"
                )
            slice_records.append(
                {
                    "cpu_type": slice_value["cpu_type"],
                    "cpu_subtype": slice_value["cpu_subtype"],
                    "file_type": slice_value["file_type"],
                    "dependencies": dependencies,
                    "id_dylib_sha256": [
                        _macho_digest_text(str(command["name"]))
                        for command in commands
                        if command["command"] == _MACHO_ID_DYLIB
                    ],
                    "rpath_sha256": [
                        _macho_digest_text(value) for value in rpaths
                    ],
                    "code_signature": "embedded",
                }
            )
        images.append(
            {
                "path": archive_path,
                "size_bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "slices": slice_records,
            }
        )
    by_path = {str(image["path"]): image for image in images}
    launch_expectations = {
        "bin/python3": "@executable_path/../" + framework,
        "Resources/Python.app/Contents/MacOS/Python": (
            "@executable_path/../../../../" + framework
        ),
    }
    for path, expected_name in launch_expectations.items():
        image = by_path[path]
        slices = image["slices"]
        assert isinstance(slices, list)
        for slice_value in slices:
            dependencies = slice_value["dependencies"]
            assert isinstance(dependencies, list)
            matches = [
                dependency for dependency in dependencies
                if dependency.get("binding") == "archive"
                and dependency.get("target") == framework
            ]
            if len(matches) != 1 or matches[0].get("install_name") != expected_name:
                raise _MachOError(
                    f"archived framework launcher binding is wrong: {path}"
                )
    return {
        "format": "iroha-sumeragi-v2-framework-python-mach-o",
        "schema_version": 1,
        "image_count": len(images),
        "images": images,
    }


def _apply_framework_macho_plan(
    runtime_root: Path,
    plan: list[dict[str, object]],
    error_type: type[Exception],
) -> list[dict[str, object]]:
    """Apply, sign, and attest the exact deterministic closure rewrite plan."""

    transforms: list[dict[str, object]] = []
    install_tool = _MACHO_TOOL_PATHS["install_name_tool"]
    codesign = _MACHO_TOOL_PATHS["codesign"]
    for item in plan:
        archive_path = str(item["path"])
        destination = runtime_root / archive_path
        before, _ = _macho_read_regular(destination, f"Mach-O transform {archive_path}")
        if (
            len(before) != item["source_size"]
            or hashlib.sha256(before).hexdigest() != item["source_sha256"]
        ):
            raise _MachOError("Mach-O transform source copy is not exact")
        operations = item["operations"]
        assert isinstance(operations, tuple)
        arguments: list[str] = []
        operation_records: list[dict[str, object]] = []
        for operation in operations:
            if operation[0] != "change" or len(operation) != 3:
                raise _MachOError("Mach-O rewrite operation is unsupported")
            old, replacement = str(operation[1]), str(operation[2])
            arguments.extend(["-change", old, replacement])
            operation_records.append(
                {
                    "operation": "change",
                    "source_install_name_sha256": _macho_digest_text(old),
                    "replacement": replacement,
                }
            )
        if operations:
            arguments.append(str(destination))
            rewrite = _macho_run(
                [str(install_tool), *arguments],
                runtime_root,
                error_type,
                label="install-name rewrite",
            )
            if rewrite.returncode != 0 or rewrite.stdout:
                raise _MachOError("framework Mach-O install-name rewrite failed")
            pre_sign, _ = _macho_read_regular(
                destination, f"rewritten Mach-O {archive_path}",
            )
            if pre_sign == before:
                raise _MachOError("Mach-O rewrite did not change its image")
        else:
            pre_sign = before
        signing = _macho_run(
            [
                str(codesign), "--force", "--sign", "-", "--timestamp=none",
                str(destination),
            ],
            runtime_root,
            error_type,
            label="ad-hoc signing",
        )
        if signing.returncode != 0 or signing.stdout:
            raise _MachOError("framework Mach-O ad-hoc signing failed")
        post_sign, _ = _macho_read_regular(
            destination, f"signed Mach-O {archive_path}",
        )
        if operations and post_sign == pre_sign:
            raise _MachOError("Mach-O ad-hoc signature did not change its image")
        _require_adhoc_signature(destination, codesign, error_type)
        parsed = _parse_macho(post_sign, archive_path)
        if parsed is None or any(
            slice_value["code_signature"] is None for slice_value in parsed
        ):
            raise _MachOError("signed Mach-O image has no embedded signature")
        _seal_relocated_macho(destination, error_type)
        transforms.append(
            {
                "path": archive_path,
                "source_mode": item["source_mode"],
                "source_sha256": item["source_sha256"],
                "source_size_bytes": item["source_size"],
                "derived_mode": "0500",
                "derived_sha256": hashlib.sha256(post_sign).hexdigest(),
                "derived_size_bytes": len(post_sign),
                "operations": operation_records,
                "codesign": "adhoc",
            }
        )
    return transforms


def _stop_macho_process(process: subprocess.Popen[bytes]) -> None:
    """Stop and reap only the authenticated Mach-O child owned by this call."""

    if process.poll() is not None:
        process.wait()
        return
    try:
        process.terminate()
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    except ProcessLookupError:
        process.wait()


def _macho_run(
    argv: list[str],
    cwd: Path,
    error_type: type[Exception],
    *,
    environment: dict[str, str] | None = None,
    maximum_output_bytes: int = _MACHO_OUTPUT_BYTES,
    label: str = "Mach-O tool",
):
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    streams: dict[str, bytearray] = {
        "stdout": bytearray(),
        "stderr": bytearray(),
    }
    overflow = False
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment
            or {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        assert process.stdout is not None and process.stderr is not None
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        deadline = time.monotonic() + 30
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(argv, 30)
            events = selector.select(remaining)
            if not events:
                raise subprocess.TimeoutExpired(argv, 30)
            for key, _ in events:
                stream_name = key.data
                retained = streams[stream_name]
                allowance = maximum_output_bytes - len(retained)
                chunk = os.read(key.fd, min(64 * 1024, allowance + 1))
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                if len(chunk) > allowance:
                    retained.extend(chunk[:allowance])
                    overflow = True
                    break
                retained.extend(chunk)
            if overflow:
                break
        if overflow:
            for key in list(selector.get_map().values()):
                selector.unregister(key.fileobj)
                key.fileobj.close()
            _stop_macho_process(process)
            returncode = process.returncode
        else:
            returncode = process.wait(
                timeout=max(deadline - time.monotonic(), 0.001)
            )
    except (OSError, subprocess.TimeoutExpired) as error:
        if process is not None:
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()
            _stop_macho_process(process)
        raise error_type(f"framework Python {label} could not run") from error
    finally:
        selector.close()
    if overflow:
        raise error_type(f"framework Python {label} output exceeds its bound")
    return subprocess.CompletedProcess(
        argv, returncode, bytes(streams["stdout"]), bytes(streams["stderr"])
    )


def _macho_tool_record(
    name: str, digest_regular, error_type: type[Exception]
) -> dict[str, object]:
    path = _MACHO_TOOL_PATHS[name]
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type(f"framework Python {name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_nlink < 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not metadata.st_mode & 0o111
        or not 0 < metadata.st_size <= _MACHO_TOOL_BYTES
    ):
        raise error_type(f"framework Python {name} metadata is unsafe")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        payload = bytearray()
        while block := os.read(descriptor, 1024 * 1024):
            payload.extend(block)
            if len(payload) > _MACHO_TOOL_BYTES:
                raise error_type(f"framework Python {name} exceeds its bound")
        after = os.fstat(descriptor)
    except OSError as error:
        raise error_type(f"framework Python {name} could not be read") from error
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    current = path.lstat()
    stable = ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
              "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(
        getattr(metadata, field) != getattr(observed, field)
        for observed in (opened, after, current)
        for field in stable
    ) or len(payload) != opened.st_size:
        raise error_type(f"framework Python {name} metadata is unsafe")
    return {
        "path": str(path),
        "mode": f"{stat.S_IMODE(opened.st_mode):04o}",
        "sha256": hashlib.sha256(payload).hexdigest(),
        "size_bytes": len(payload),
    }


def _preflight_macho(
    path: Path, error_type: type[Exception], *, source: bool
) -> None:
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type("framework Python Mach-O input is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid not in ({0, os.geteuid()} if source else {os.geteuid()})
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not metadata.st_mode & 0o111
        or not 0 < metadata.st_size <= _MACHO_ARTIFACT_BYTES
    ):
        raise error_type("framework Python Mach-O input metadata is unsafe")


def _macho_dependencies(
    path: Path, otool: Path, error_type: type[Exception]
) -> list[str]:
    result = _macho_run([str(otool), "-L", str(path)], path.parent, error_type)
    if result.returncode != 0 or result.stderr:
        raise error_type("framework Python dependency inspection failed")
    try:
        lines = result.stdout.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise error_type("framework Python dependency output is not UTF-8") from error
    if not lines:
        raise error_type("framework Python dependency output is malformed")
    thin_header = f"{path}:"
    architecture_header = re.compile(
        rf"{re.escape(str(path))} \(architecture ([A-Za-z0-9][A-Za-z0-9._+-]*)\):"
    )
    sections: list[tuple[str, list[str]]] = []
    current_architecture: str | None = None
    current_dependencies: list[str] | None = None
    for line in lines:
        match = architecture_header.fullmatch(line)
        if line == thin_header or match is not None:
            architecture = "thin" if match is None else match.group(1)
            if current_dependencies is not None:
                sections.append((current_architecture or "", current_dependencies))
            if sections and (
                architecture == "thin" or sections[0][0] == "thin"
            ):
                raise error_type("framework Python dependency output is malformed")
            if any(observed == architecture for observed, _ in sections):
                raise error_type("framework Python dependency output is malformed")
            current_architecture = architecture
            current_dependencies = []
            continue
        if current_dependencies is None:
            raise error_type("framework Python dependency output is malformed")
        marker = line.find(" (")
        dependency = line[1:marker] if line.startswith("\t") and marker > 1 else ""
        if (
            not dependency
            or not line.endswith(")")
            or "\0" in dependency
            or any(ord(character) < 0x20 for character in dependency)
        ):
            raise error_type("framework Python dependency output is malformed")
        current_dependencies.append(dependency)
    if current_dependencies is None:
        raise error_type("framework Python dependency output is malformed")
    sections.append((current_architecture or "", current_dependencies))
    if (
        any(not dependencies for _, dependencies in sections)
        or any(
            dependencies != sections[0][1]
            for _, dependencies in sections[1:]
        )
    ):
        raise error_type("framework Python dependency slices disagree")
    dependencies = sections[0][1]
    if len(set(dependencies)) != len(dependencies):
        raise error_type("framework Python dependencies are not unique")
    return dependencies


def _dependency_vector_sha256(dependencies: list[str]) -> str:
    payload = json.dumps(
        dependencies, ensure_ascii=False, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def probe_framework_python_runtime(
    *,
    runtime_root: Path,
    stdlib_name: str,
    probe_code: str,
    error_type: type[Exception],
) -> bytes:
    """Run the relocated interpreter with hard-bounded output and child reaping."""

    executable = runtime_root / "bin/python3"
    _preflight_macho(executable, error_type, source=False)
    metadata = executable.lstat()
    if stat.S_IMODE(metadata.st_mode) != 0o500:
        raise error_type("archived framework Python executable metadata is unsafe")
    expected_zip = runtime_root / "lib" / (
        f"python{sys.version_info.major}{sys.version_info.minor}.zip"
    )
    expected_stdlib = runtime_root / "lib" / stdlib_name
    expected_dynload = expected_stdlib / "lib-dynload"
    result = _macho_run(
        [
            str(executable), "-I", "-S", "-c", probe_code,
            str(executable), str(runtime_root), str(expected_zip),
            str(expected_stdlib), str(expected_dynload),
        ],
        runtime_root,
        error_type,
        environment={
            "LANG": "C", "LC_ALL": "C", "PATH": str(runtime_root / "bin"),
        },
        maximum_output_bytes=4096,
        label="archived interpreter probe",
    )
    expected_stdout = os.fsencode(str(executable)) + b"\n"
    if (
        result.returncode != 0
        or result.stdout != expected_stdout
        or result.stderr
    ):
        raise error_type(
            "archived framework Python isolated probe did not report its executable"
        )
    return expected_stdout


def _require_adhoc_signature(
    path: Path, codesign: Path, error_type: type[Exception]
) -> None:
    verification = _macho_run(
        [str(codesign), "--verify", "--strict", "--verbose=0", str(path)],
        path.parent,
        error_type,
    )
    details = _macho_run(
        [str(codesign), "-d", "--verbose=4", str(path)],
        path.parent,
        error_type,
    )
    try:
        detail_lines = details.stderr.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise error_type("framework Python signature output is not UTF-8") from error
    if (
        verification.returncode != 0
        or verification.stdout
        or verification.stderr
        or details.returncode != 0
        or details.stdout
        or detail_lines.count("Signature=adhoc") != 1
    ):
        raise error_type("framework Python relocated output is not ad-hoc signed")


def _artifact_record(
    path: Path,
    dependencies: list[str],
    framework_dependency: str,
    digest_regular,
    error_type: type[Exception],
    *,
    source: bool,
) -> dict[str, object]:
    _preflight_macho(path, error_type, source=source)
    if source:
        try:
            payload, metadata = _macho_read_regular(
                path,
                "framework Python Mach-O input",
                maximum_bytes=_MACHO_ARTIFACT_BYTES,
            )
        except _MachOError as error:
            raise error_type(str(error)) from error
        digest = hashlib.sha256(payload).hexdigest()
        size = len(payload)
    else:
        digest, size, metadata = digest_regular(
            path,
            "framework Python Mach-O input",
            maximum_bytes=_MACHO_ARTIFACT_BYTES,
        )
    try:
        current = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type("framework Python Mach-O input changed") from error
    mode = f"{stat.S_IMODE(metadata.st_mode):04o}"
    if (
        resolved != path
        or
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or not metadata.st_mode & 0o111
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or metadata.st_uid not in ({0, os.geteuid()} if source else {os.geteuid()})
        or (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mode,
            metadata.st_uid,
            metadata.st_nlink,
            metadata.st_size,
        )
        != (
            current.st_dev,
            current.st_ino,
            current.st_mode,
            current.st_uid,
            current.st_nlink,
            current.st_size,
        )
    ):
        raise error_type("framework Python Mach-O input metadata is unsafe")
    record: dict[str, object] = {
        "mode": mode,
        "sha256": digest,
        "size_bytes": size,
        "dependency_vector_sha256": _dependency_vector_sha256(dependencies),
    }
    if source:
        record["framework_dependency_sha256"] = hashlib.sha256(
            framework_dependency.encode("utf-8")
        ).hexdigest()
    else:
        record["framework_dependency"] = framework_dependency
        record["codesign"] = "adhoc"
    return record


def _seal_relocated_macho(path: Path, error_type: type[Exception]) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        before = path.lstat()
        descriptor = os.open(path, flags)
    except OSError as error:
        raise error_type("framework Python relocated output could not be sealed") from error
    try:
        opened = os.fstat(descriptor)
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or before.st_uid != os.geteuid()
            or before.st_nlink != 1
            or (before.st_dev, before.st_ino, before.st_mode, before.st_size)
            != (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size)
        ):
            raise error_type("framework Python relocated output metadata is unsafe")
        os.fchmod(descriptor, 0o500)
        os.fsync(descriptor)
        sealed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    if (
        (sealed.st_dev, sealed.st_ino, sealed.st_uid, sealed.st_nlink, sealed.st_size)
        != (after.st_dev, after.st_ino, after.st_uid, after.st_nlink, after.st_size)
        or stat.S_IMODE(sealed.st_mode) != 0o500
        or stat.S_IMODE(after.st_mode) != 0o500
    ):
        raise error_type("framework Python relocated output sealing changed identity")


def _macho_contract_mode(
    value: object, label: str, *, executable: bool = False,
) -> str:
    if (
        not isinstance(value, str)
        or _MODE_RE.fullmatch(value) is None
        or int(value, 8) & 0o022
        or not int(value, 8) & 0o444
        or (executable and not int(value, 8) & 0o111)
    ):
        raise _MachOError(f"{label} mode is unsafe")
    return value


def _macho_contract_digest(value: object, label: str) -> str:
    if not isinstance(value, str) or _DIGEST_RE.fullmatch(value) is None:
        raise _MachOError(f"{label} digest is malformed")
    return value


def _macho_contract_size(value: object, label: str) -> int:
    if type(value) is not int or not 0 < value <= _MACHO_MAXIMUM_FILE_BYTES:
        raise _MachOError(f"{label} size is outside its bound")
    return value


def _validate_macho_runtime_projection(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {
        "format", "schema_version", "image_count", "images",
    }:
        raise _MachOError("framework Mach-O runtime projection is malformed")
    images = value["images"]
    if (
        value["format"] != "iroha-sumeragi-v2-framework-python-mach-o"
        or value["schema_version"] != 1
        or type(value["schema_version"]) is not int
        or not isinstance(images, list)
        or type(value["image_count"]) is not int
        or value["image_count"] != len(images)
        or not 3 <= len(images) <= _MACHO_MAX_IMAGES
    ):
        raise _MachOError("framework Mach-O runtime projection is not exact")
    paths: list[str] = []
    for image in images:
        if not isinstance(image, dict) or set(image) != {
            "path", "size_bytes", "sha256", "slices",
        }:
            raise _MachOError("framework Mach-O runtime image is malformed")
        path = image["path"]
        if not isinstance(path, str):
            raise _MachOError("framework Mach-O runtime path is malformed")
        _macho_path_parts(path, "framework Mach-O runtime image")
        _macho_contract_digest(image["sha256"], "framework Mach-O runtime image")
        _macho_contract_size(image["size_bytes"], "framework Mach-O runtime image")
        slices = image["slices"]
        if not isinstance(slices, list) or not 1 <= len(slices) <= _MACHO_MAX_SLICES:
            raise _MachOError("framework Mach-O runtime slices are malformed")
        architectures: set[tuple[int, int]] = set()
        for slice_value in slices:
            if not isinstance(slice_value, dict) or set(slice_value) != {
                "cpu_type", "cpu_subtype", "file_type", "dependencies",
                "id_dylib_sha256", "rpath_sha256", "code_signature",
            }:
                raise _MachOError("framework Mach-O runtime slice is malformed")
            for field in ("cpu_type", "cpu_subtype", "file_type"):
                if type(slice_value[field]) is not int or slice_value[field] < 0:
                    raise _MachOError("framework Mach-O runtime integer is malformed")
            architecture = (
                slice_value["cpu_type"], slice_value["cpu_subtype"],
            )
            if architecture in architectures:
                raise _MachOError("framework Mach-O runtime architecture repeats")
            architectures.add(architecture)
            if slice_value["code_signature"] != "embedded":
                raise _MachOError("framework Mach-O runtime image is unsigned")
            for field in ("id_dylib_sha256", "rpath_sha256"):
                digests = slice_value[field]
                if (
                    not isinstance(digests, list)
                    or len(digests) > _MACHO_MAX_COMMANDS
                    or any(
                        not isinstance(digest, str)
                        or _DIGEST_RE.fullmatch(digest) is None
                        for digest in digests
                    )
                ):
                    raise _MachOError("framework Mach-O command digests are malformed")
            dependencies = slice_value["dependencies"]
            if (
                not isinstance(dependencies, list)
                or len(dependencies) > _MACHO_MAX_COMMANDS
            ):
                raise _MachOError("framework Mach-O dependencies are malformed")
            for dependency in dependencies:
                if not isinstance(dependency, dict):
                    raise _MachOError("framework Mach-O dependency is malformed")
                binding = dependency.get("binding")
                keys = (
                    {"command", "binding", "install_name_sha256"}
                    if binding == "system"
                    else {"command", "binding", "install_name", "target"}
                    if binding == "archive"
                    else set()
                )
                if (
                    set(dependency) != keys
                    or type(dependency.get("command")) is not int
                    or dependency["command"] not in _MACHO_DEPENDENCY_COMMANDS
                ):
                    raise _MachOError("framework Mach-O dependency binding is malformed")
                if binding == "system":
                    _macho_contract_digest(
                        dependency["install_name_sha256"],
                        "framework Mach-O system install name",
                    )
                else:
                    install_name = dependency["install_name"]
                    target = dependency["target"]
                    if (
                        not isinstance(install_name, str)
                        or not install_name.startswith(
                            ("@loader_path/", "@executable_path/")
                        )
                        or not isinstance(target, str)
                    ):
                        raise _MachOError(
                            "framework Mach-O archive dependency is unsafe"
                        )
                    _macho_path_parts(target, "framework Mach-O dependency target")
        paths.append(path)
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        raise _MachOError("framework Mach-O runtime image order is not exact")
    path_set = set(paths)
    for image in images:
        for slice_value in image["slices"]:
            for dependency in slice_value["dependencies"]:
                if (
                    dependency["binding"] == "archive"
                    and dependency["target"] not in path_set
                ):
                    raise _MachOError("framework Mach-O dependency target is absent")
    return value


def _validate_macho_closure_contract(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {
        "format", "schema_version", "source_image_count",
        "external_sources", "transforms", "runtime",
    }:
        raise _MachOError("framework Mach-O closure contract is malformed")
    external = value["external_sources"]
    transforms = value["transforms"]
    if (
        value["format"] != _MACHO_TRANSCRIPT_FORMAT
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or type(value["source_image_count"]) is not int
        or not 3 <= value["source_image_count"] <= _MACHO_MAX_IMAGES
        or not isinstance(external, list)
        or not isinstance(transforms, list)
    ):
        raise _MachOError("framework Mach-O closure contract is not exact")
    projection = _validate_macho_runtime_projection(value["runtime"])
    if value["source_image_count"] != projection["image_count"]:
        raise _MachOError("framework Mach-O source/runtime counts disagree")
    external_paths: list[str] = []
    external_inputs: list[str] = []
    for source in external:
        if not isinstance(source, dict) or set(source) != {
            "input_path", "path", "mode", "sha256", "size_bytes",
        }:
            raise _MachOError("external Mach-O source binding is malformed")
        input_path = source["input_path"]
        path = source["path"]
        if (
            not isinstance(input_path, str)
            or not input_path.startswith("mach-o-dependency-sources/")
            or not isinstance(path, str)
            or f"/{_MACHO_DEPENDENCY_DIRECTORY}/" not in f"/{path}"
        ):
            raise _MachOError("external Mach-O source path is unsafe")
        _macho_path_parts(input_path, "external Mach-O input")
        _macho_path_parts(path, "external Mach-O archive member")
        _macho_contract_mode(source["mode"], "external Mach-O source")
        _macho_contract_digest(source["sha256"], "external Mach-O source")
        _macho_contract_size(source["size_bytes"], "external Mach-O source")
        external_inputs.append(input_path)
        external_paths.append(path)
    if (
        external_paths != sorted(external_paths)
        or len(external_paths) != len(set(external_paths))
        or len(external_inputs) != len(set(external_inputs))
    ):
        raise _MachOError("external Mach-O source order is not exact")
    transform_paths: list[str] = []
    transform_inputs: list[str] = []
    for transform in transforms:
        if not isinstance(transform, dict) or set(transform) != {
            "input_path", "path", "source_mode", "source_sha256",
            "source_size_bytes", "derived_mode", "derived_sha256",
            "derived_size_bytes", "operations", "codesign",
        }:
            raise _MachOError("framework Mach-O transform is malformed")
        input_path = transform["input_path"]
        path = transform["path"]
        if not isinstance(input_path, str) or not isinstance(path, str):
            raise _MachOError("framework Mach-O transform path is malformed")
        _macho_path_parts(input_path, "framework Mach-O transform input")
        _macho_path_parts(path, "framework Mach-O transform output")
        _macho_contract_mode(transform["source_mode"], "Mach-O transform source")
        _macho_contract_digest(
            transform["source_sha256"], "Mach-O transform source",
        )
        _macho_contract_size(
            transform["source_size_bytes"], "Mach-O transform source",
        )
        if transform["derived_mode"] != "0500" or transform["codesign"] != "adhoc":
            raise _MachOError("framework Mach-O transform output is not sealed")
        _macho_contract_digest(
            transform["derived_sha256"], "Mach-O transform output",
        )
        _macho_contract_size(
            transform["derived_size_bytes"], "Mach-O transform output",
        )
        operations = transform["operations"]
        if not isinstance(operations, list):
            raise _MachOError("framework Mach-O operations are malformed")
        replacements: set[str] = set()
        for operation in operations:
            if not isinstance(operation, dict) or set(operation) != {
                "operation", "source_install_name_sha256", "replacement",
            }:
                raise _MachOError("framework Mach-O operation is malformed")
            replacement = operation["replacement"]
            if (
                operation["operation"] != "change"
                or not isinstance(replacement, str)
                or not replacement.startswith(
                    ("@loader_path/", "@executable_path/")
                )
                or replacement in replacements
            ):
                raise _MachOError("framework Mach-O operation is unsafe")
            _macho_contract_digest(
                operation["source_install_name_sha256"],
                "framework Mach-O source install name",
            )
            replacements.add(replacement)
        transform_inputs.append(input_path)
        transform_paths.append(path)
    runtime_paths = {
        str(image["path"]) for image in projection["images"]
    }
    if (
        transform_paths != sorted(transform_paths)
        or len(transform_paths) != len(set(transform_paths))
        or len(transform_inputs) != len(set(transform_inputs))
        or not set(transform_paths) <= runtime_paths
        or not set(external_paths) <= set(transform_paths)
    ):
        raise _MachOError("framework Mach-O transform order is not exact")
    transform_by_path = {
        str(item["path"]): item for item in transforms
    }
    for source in external:
        transform = transform_by_path.get(str(source["path"]))
        if (
            transform is None
            or transform["input_path"] != source["input_path"]
            or transform["source_mode"] != source["mode"]
            or transform["source_sha256"] != source["sha256"]
            or transform["source_size_bytes"] != source["size_bytes"]
        ):
            raise _MachOError("external Mach-O copy/sign binding changed")
    return value


def _validate_framework_python_relocation_contract(
    value: object, error_type: type[Exception],
) -> dict[str, object]:
    try:
        if not isinstance(value, dict) or set(value) != {
            "format", "schema_version", "framework", "tools", "artifacts",
            "closure",
        }:
            raise _MachOError("framework Python relocation contract is malformed")
        if (
            value["format"] != "iroha-sumeragi-v2-framework-python-relocation"
            or type(value["schema_version"]) is not int
            or value["schema_version"] != 2
            or not isinstance(value["framework"], str)
            or _FRAMEWORK_RE.fullmatch(value["framework"]) is None
            or not isinstance(value["tools"], dict)
            or set(value["tools"]) != set(_MACHO_TOOL_PATHS)
            or not isinstance(value["artifacts"], dict)
            or set(value["artifacts"]) != set(_MACHO_ARTIFACT_PATHS)
        ):
            raise _MachOError("framework Python relocation contract is not exact")
        for name, expected_path in _MACHO_TOOL_PATHS.items():
            record = value["tools"][name]
            if (
                not isinstance(record, dict)
                or set(record) != {"path", "mode", "sha256", "size_bytes"}
                or record["path"] != str(expected_path)
            ):
                raise _MachOError(
                    "framework Python relocation tool binding is not exact"
                )
            _macho_contract_mode(
                record["mode"], "framework Python relocation tool",
                executable=True,
            )
            _macho_contract_digest(
                record["sha256"], "framework Python relocation tool",
            )
            _macho_contract_size(
                record["size_bytes"], "framework Python relocation tool",
            )
        closure = _validate_macho_closure_contract(value["closure"])
        rewrites = _macho_rewrites(value["framework"])
        transform_by_path = {
            item["path"]: item for item in closure["transforms"]
        }
        for name, (path, replacement) in rewrites.items():
            transform = transform_by_path.get(path)
            artifact = value["artifacts"][name]
            if (
                not isinstance(artifact, dict)
                or set(artifact) != {"path", "source", "derived"}
                or artifact["path"] != path
                or not isinstance(artifact["source"], dict)
                or set(artifact["source"]) != {
                    "mode", "sha256", "size_bytes",
                    "framework_dependency_sha256",
                    "dependency_vector_sha256",
                }
                or not isinstance(artifact["derived"], dict)
                or set(artifact["derived"]) != {
                    "mode", "sha256", "size_bytes", "framework_dependency",
                    "dependency_vector_sha256", "codesign",
                }
            ):
                raise _MachOError(
                    "framework Python relocation artifact binding is not exact"
                )
            source = artifact["source"]
            derived = artifact["derived"]
            _macho_contract_mode(
                source["mode"], "framework Python relocation source",
                executable=True,
            )
            _macho_contract_digest(
                source["sha256"], "framework Python relocation source",
            )
            _macho_contract_size(
                source["size_bytes"], "framework Python relocation source",
            )
            _macho_contract_digest(
                source["framework_dependency_sha256"],
                "framework Python source dependency",
            )
            _macho_contract_digest(
                source["dependency_vector_sha256"],
                "framework Python source dependency vector",
            )
            _macho_contract_digest(
                derived["sha256"], "framework Python relocated output",
            )
            _macho_contract_size(
                derived["size_bytes"], "framework Python relocated output",
            )
            _macho_contract_digest(
                derived["dependency_vector_sha256"],
                "framework Python relocated dependency vector",
            )
            operations = transform["operations"] if transform is not None else []
            operation_replacements = [item["replacement"] for item in operations]
            already_local = source["framework_dependency_sha256"] == _macho_digest_text(replacement)
            if (
                transform is None
                or transform["input_path"]
                != ("python3" if name == "launcher" else path)
                or transform["source_mode"] != source["mode"]
                or transform["source_sha256"] != source["sha256"]
                or transform["source_size_bytes"]
                != source["size_bytes"]
                or transform["derived_mode"] != derived["mode"]
                or transform["derived_sha256"] != derived["sha256"]
                or transform["derived_size_bytes"]
                != derived["size_bytes"]
                or operation_replacements not in ([], [replacement])
                or already_local != (not operations)
                or (
                    operations and operations[0]["source_install_name_sha256"]
                    != source["framework_dependency_sha256"]
                )
                or derived["mode"] != "0500"
                or derived["framework_dependency"] != replacement
                or derived["codesign"] != "adhoc"
                or (operations and source["sha256"] == derived["sha256"])
                or (
                    operations and source["dependency_vector_sha256"]
                    == derived["dependency_vector_sha256"]
                )
            ):
                raise _MachOError(
                    "framework Python launcher closure binding changed"
                )
        return value
    except _MachOError as error:
        raise error_type(str(error)) from error


def _macho_external_input_path(
    archive_path: str, source_sha256: str,
) -> str:
    archive_name = PurePosixPath(archive_path).name
    prefix = f"{source_sha256}-"
    basename = archive_name.removeprefix(prefix)
    if (
        _MACHO_SAFE_BASENAME_RE.fullmatch(basename) is None
        or not archive_name.startswith(prefix)
    ):
        raise _MachOError("external Mach-O archive identity is malformed")
    value = f"mach-o-dependency-sources/{archive_name}"
    _macho_path_parts(value, "external Mach-O input")
    return value


def _macho_input_path(
    archive_path: str, external: dict[str, Path],
    source_sha256: str,
) -> str:
    if archive_path in external:
        return _macho_external_input_path(archive_path, source_sha256)
    return "python3" if archive_path == "bin/python3" else archive_path


def _macho_operation_records(
    operations: tuple[tuple[object, ...], ...],
) -> list[dict[str, object]]:
    return [
        {
            "operation": "change",
            "source_install_name_sha256": _macho_digest_text(str(operation[1])),
            "replacement": str(operation[2]),
        }
        for operation in operations
    ]


def _framework_launcher_artifacts(
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    digest_regular,
    error_type: type[Exception],
) -> dict[str, object]:
    rewrites = _macho_rewrites(framework)
    old_dependency = str(version_root / framework)
    sources = {
        "launcher": source_python,
        "trampoline": version_root
        / "Resources/Python.app/Contents/MacOS/Python",
    }
    artifacts: dict[str, object] = {}
    for name, (relative, rewritten) in rewrites.items():
        source_path = sources[name]
        destination = runtime_root / relative
        source_dependencies = _macho_dependencies(
            source_path, _MACHO_TOOL_PATHS["otool"], error_type,
        )
        derived_dependencies = _macho_dependencies(
            destination, _MACHO_TOOL_PATHS["otool"], error_type,
        )
        source_framework_dependencies = [
            value for value in source_dependencies if value in {old_dependency, rewritten}
        ]
        if len(source_framework_dependencies) != 1:
            raise _MachOError(
                f"framework Mach-O launcher dependency is not exact: {relative}"
            )
        source_framework_dependency = source_framework_dependencies[0]
        expected = [
            rewritten if value == source_framework_dependency else value for value in source_dependencies
        ]
        if (
            derived_dependencies != expected
            or derived_dependencies.count(rewritten) != 1
            or old_dependency in derived_dependencies
        ):
            raise _MachOError(
                f"framework Mach-O launcher rewrite is not exact: {relative}"
            )
        artifacts[name] = {
            "path": relative,
            "source": _artifact_record(
                source_path, source_dependencies, source_framework_dependency,
                digest_regular, error_type, source=True,
            ),
            "derived": _artifact_record(
                destination, derived_dependencies, rewritten,
                digest_regular, error_type, source=False,
            ),
        }
    return artifacts


def _external_macho_records(
    images: dict[Path, dict[str, object]],
    external: dict[str, Path],
) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    for archive_path, source in sorted(external.items()):
        image = images.get(source)
        if image is None or image["archive_path"] != archive_path:
            raise _MachOError("external Mach-O source escaped its closure")
        digest = str(image["sha256"])
        records.append(
            {
                "input_path": _macho_external_input_path(archive_path, digest),
                "path": archive_path,
                "mode": format(
                    stat.S_IMODE(image["metadata"].st_mode), "04o",
                ),
                "sha256": digest,
                "size_bytes": len(image["data"]),
            }
        )
    return records


def _observe_macho_transforms(
    runtime_root: Path,
    plan: list[dict[str, object]],
    external: dict[str, Path],
    error_type: type[Exception],
) -> list[dict[str, object]]:
    transforms: list[dict[str, object]] = []
    for item in plan:
        path = str(item["path"])
        destination = runtime_root / path
        payload, metadata = _macho_read_regular(
            destination, f"relocated Mach-O {path}",
        )
        if (
            metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o500
        ):
            raise _MachOError("relocated Mach-O output is not sealed")
        _require_adhoc_signature(
            destination, _MACHO_TOOL_PATHS["codesign"], error_type,
        )
        slices = _parse_macho(payload, path)
        if slices is None or any(
            slice_value["code_signature"] is None for slice_value in slices
        ):
            raise _MachOError("relocated Mach-O output is unsigned")
        operations = item["operations"]
        assert isinstance(operations, tuple)
        transforms.append(
            {
                "input_path": _macho_input_path(
                    path, external, str(item["source_sha256"]),
                ),
                "path": path,
                "source_mode": item["source_mode"],
                "source_sha256": item["source_sha256"],
                "source_size_bytes": item["source_size"],
                "derived_mode": "0500",
                "derived_sha256": hashlib.sha256(payload).hexdigest(),
                "derived_size_bytes": len(payload),
                "operations": _macho_operation_records(operations),
                "codesign": "adhoc",
            }
        )
    return transforms


def _validate_projection_against_sources(
    images: dict[Path, dict[str, object]],
    source_python: Path,
    plan: list[dict[str, object]],
    transforms: list[dict[str, object]],
    projection: dict[str, object],
) -> None:
    source_by_archive = {
        str(image["archive_path"]): (source, image)
        for source, image in images.items()
    }
    plan_by_path = {str(item["path"]): item for item in plan}
    transform_by_path = {
        str(item["path"]): item for item in transforms
    }
    runtime_by_path = {
        str(image["path"]): image for image in projection["images"]
    }
    if set(source_by_archive) != set(runtime_by_path):
        raise _MachOError("framework Mach-O runtime closure changed membership")
    source_to_archive = {
        source: archive for archive, (source, _) in source_by_archive.items()
    }
    for archive_path, (source, image) in source_by_archive.items():
        runtime = runtime_by_path[archive_path]
        transform = transform_by_path.get(archive_path)
        expected_digest = (
            str(transform["derived_sha256"])
            if transform is not None
            else str(image["sha256"])
        )
        expected_size = (
            int(transform["derived_size_bytes"])
            if transform is not None
            else len(image["data"])
        )
        if (
            runtime["sha256"] != expected_digest
            or runtime["size_bytes"] != expected_size
        ):
            raise _MachOError("framework Mach-O runtime bytes changed")
        plan_item = plan_by_path.get(archive_path)
        operations = (
            {
                str(operation[1]): str(operation[2])
                for operation in plan_item["operations"]
            }
            if plan_item is not None
            else {}
        )
        source_slices = image["slices"]
        runtime_slices = runtime["slices"]
        if len(source_slices) != len(runtime_slices):
            raise _MachOError("framework Mach-O runtime slices changed")
        for source_slice, runtime_slice in zip(source_slices, runtime_slices):
            source_commands = source_slice["commands"]
            rpaths = tuple(
                str(command["name"])
                for command in source_commands
                if command["command"] == _MACHO_RPATH
            )
            expected_dependencies: list[dict[str, object]] = []
            for command in source_commands:
                command_value = int(command["command"])
                if command_value not in _MACHO_DEPENDENCY_COMMANDS:
                    continue
                name = str(command["name"])
                target = _resolve_macho_source_name(
                    name, image, source_python, rpaths=rpaths,
                )
                if target is None:
                    expected_dependencies.append(
                        {
                            "command": command_value,
                            "binding": "system",
                            "install_name_sha256": _macho_digest_text(name),
                        }
                    )
                else:
                    target_archive = source_to_archive.get(target)
                    if target_archive is None:
                        raise _MachOError(
                            "framework Mach-O dependency escaped source closure"
                        )
                    expected_dependencies.append(
                        {
                            "command": command_value,
                            "binding": "archive",
                            "install_name": operations.get(name, name),
                            "target": target_archive,
                        }
                    )
            expected_slice = {
                "cpu_type": source_slice["cpu_type"],
                "cpu_subtype": source_slice["cpu_subtype"],
                "file_type": source_slice["file_type"],
                "dependencies": expected_dependencies,
                "id_dylib_sha256": [
                    _macho_digest_text(str(command["name"]))
                    for command in source_commands
                    if command["command"] == _MACHO_ID_DYLIB
                ],
                "rpath_sha256": [
                    _macho_digest_text(value) for value in rpaths
                ],
                "code_signature": "embedded",
            }
            if runtime_slice != expected_slice:
                raise _MachOError(
                    f"framework Mach-O loader semantics changed: {archive_path}"
                )


def _observe_framework_python_relocation(
    *,
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    digest_regular,
    error_type: type[Exception],
) -> tuple[dict[str, object], dict[str, Path]]:
    stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
    images, external = _framework_python_macho_closure(
        version_root, source_python, framework, stdlib_name,
    )
    plan = _framework_python_macho_plan(
        images, source_python, force_sign_paths=frozenset(external),
    )
    _validate_framework_python_macho_plan(plan, version_root, framework)
    transforms = _observe_macho_transforms(
        runtime_root, plan, external, error_type,
    )
    projection = _framework_runtime_macho_projection(
        runtime_root,
        framework,
        {str(image["archive_path"]) for image in images.values()},
    )
    _validate_projection_against_sources(
        images, source_python, plan, transforms, projection,
    )
    document = {
        "format": "iroha-sumeragi-v2-framework-python-relocation",
        "schema_version": 2,
        "framework": framework,
        "tools": {
            name: _macho_tool_record(name, digest_regular, error_type)
            for name in sorted(_MACHO_TOOL_PATHS)
        },
        "artifacts": _framework_launcher_artifacts(
            version_root, framework, source_python, runtime_root,
            digest_regular, error_type,
        ),
        "closure": {
            "format": _MACHO_TRANSCRIPT_FORMAT,
            "schema_version": 1,
            "source_image_count": len(images),
            "external_sources": _external_macho_records(images, external),
            "transforms": transforms,
            "runtime": projection,
        },
    }
    roots = {
        _macho_external_input_path(
            archive_path, str(images[source]["sha256"]),
        ): source
        for archive_path, source in external.items()
    }
    return document, roots


def relocate_framework_python_runtime(
    *,
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    digest_regular,
    error_type: type[Exception],
    copy_external=None,
) -> dict[str, object]:
    """Copy, rewrite, sign, and attest the recursive non-system loader closure."""

    try:
        stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
        tools_before = {
            name: _macho_tool_record(name, digest_regular, error_type)
            for name in sorted(_MACHO_TOOL_PATHS)
        }
        images, external = _framework_python_macho_closure(
            version_root, source_python, framework, stdlib_name,
        )
        if external and not callable(copy_external):
            raise _MachOError(
                "framework Mach-O external-copy callback is unavailable"
            )
        for archive_path, source in sorted(external.items()):
            image = images[source]
            copy_external(
                source,
                archive_path,
                _macho_external_input_path(
                    archive_path, str(image["sha256"]),
                ),
            )
        plan = _framework_python_macho_plan(
            images, source_python, force_sign_paths=frozenset(external),
        )
        _validate_framework_python_macho_plan(plan, version_root, framework)
        _apply_framework_macho_plan(runtime_root, plan, error_type)
        observed, _ = _observe_framework_python_relocation(
            version_root=version_root,
            framework=framework,
            source_python=source_python,
            runtime_root=runtime_root,
            digest_regular=digest_regular,
            error_type=error_type,
        )
        if observed["tools"] != tools_before:
            raise _MachOError(
                "framework Python Mach-O tools changed during relocation"
            )
        return _validate_framework_python_relocation_contract(
            observed, error_type,
        )
    except _MachOError as error:
        raise error_type(str(error)) from error


def verify_framework_python_relocation(
    *,
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    contract: object,
    digest_regular,
    error_type: type[Exception],
) -> dict[str, Path]:
    """Reauthenticate the complete source, rewrite, signature, and loader graph."""

    expected = _validate_framework_python_relocation_contract(
        contract, error_type,
    )
    try:
        observed, roots = _observe_framework_python_relocation(
            version_root=version_root,
            framework=framework,
            source_python=source_python,
            runtime_root=runtime_root,
            digest_regular=digest_regular,
            error_type=error_type,
        )
    except _MachOError as error:
        raise error_type(str(error)) from error
    if observed != expected:
        raise error_type("framework Python relocation provenance changed")
    return roots


def bind_framework_python_relocation(
    inputs: list[dict[str, object]],
    outputs: list[dict[str, object]],
    contract: object,
    error_type: type[Exception],
    *,
    update: bool,
) -> set[str]:
    """Bind every transformed source record to its exact archived derivation."""

    document = _validate_framework_python_relocation_contract(
        contract, error_type,
    )
    input_by_path = {
        record.get("path"): record
        for record in inputs if isinstance(record, dict)
    }
    output_by_path = {
        record.get("path"): record
        for record in outputs if isinstance(record, dict)
    }
    if len(input_by_path) != len(inputs) or len(output_by_path) != len(outputs):
        raise error_type("framework Python relocation inventories are not unique")
    relocated: set[str] = set()
    for transform in document["closure"]["transforms"]:
        input_path = transform["input_path"]
        output_path = transform["path"]
        source = input_by_path.get(input_path)
        output = output_by_path.get(output_path)
        if (
            not isinstance(source, dict)
            or not isinstance(output, dict)
            or source.get("kind") != "file"
            or output.get("kind") != "file"
            or (
                source.get("source_mode"),
                source.get("sha256"),
                source.get("size"),
            )
            != (
                transform["source_mode"],
                transform["source_sha256"],
                transform["source_size_bytes"],
            )
            or (
                output.get("mode"), output.get("sha256"), output.get("size"),
            )
            != (
                transform["derived_mode"],
                transform["derived_sha256"],
                transform["derived_size_bytes"],
            )
        ):
            raise error_type("framework Python source/derived binding changed")
        destination = {
            "destination_device": output.get("device"),
            "destination_inode": output.get("inode"),
            "destination_mode": output.get("mode"),
        }
        if update:
            source.update(destination)
        elif any(
            source.get(key) != value for key, value in destination.items()
        ):
            raise error_type("framework Python destination provenance changed")
        relocated.add(input_path)
    return relocated


def run(
    *,
    error_type: type[Exception],
    cleanup_invocation,
    cleanup_sdk_command_work,
    copy_cache,
    copy_framework_python_runtime,
    copy_private_bundle,
    copy_runtime,
    copy_sdk_dependencies,
    create_sdk_command_work,
    publish_validation_failure,
    seal_release_result,
    snapshot_cache,
    verify_cache_sources,
    verify_framework_python_runtime,
    verify_private_bundle,
    verify_runtime_sources,
    verify_sdk_dependencies,
    argv: list[str] | None = None,
) -> int:
    """Parse ``argv`` and dispatch through the explicitly supplied operations."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--source-cargo-home", type=Path)
    parser.add_argument("--cargo-home", type=Path)
    parser.add_argument("--inventory", type=Path)
    parser.add_argument("--final", action="store_true")
    parser.add_argument("--publish-validation-failure", action="store_true")
    parser.add_argument("--seal-release-result", action="store_true")
    parser.add_argument("--copy-runtime", action="store_true")
    parser.add_argument("--copy-framework-python", action="store_true")
    parser.add_argument("--verify-framework-python", action="store_true")
    parser.add_argument("--verify-runtime-sources", action="store_true")
    parser.add_argument("--verify-cache-sources", action="store_true")
    parser.add_argument("--copy-private-bundle", action="store_true")
    parser.add_argument("--verify-private-bundle", action="store_true")
    parser.add_argument("--copy-sdk-dependencies", action="store_true")
    parser.add_argument("--verify-sdk-dependencies", action="store_true")
    parser.add_argument("--create-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-invocation", action="store_true")
    parser.add_argument("--runtime-root", type=Path)
    parser.add_argument("--runtime-inventory", type=Path)
    parser.add_argument("--runtime-source", type=Path, action="append", default=[])
    parser.add_argument("--bundle-source", type=Path)
    parser.add_argument("--bundle-root", type=Path)
    parser.add_argument("--sdk-dependency-bundle-manifest", type=Path)
    parser.add_argument("--expected-sdk-dependency-bundle-manifest-sha256")
    parser.add_argument("--repository-root", type=Path)
    parser.add_argument("--sdk-input-root", type=Path)
    parser.add_argument("--sdk-work-root", type=Path)
    parser.add_argument("--sdk-archive", type=Path)
    parser.add_argument("--sdk-dependency-inventory", type=Path)
    parser.add_argument("--sdk-work-final-inventory", type=Path)
    parser.add_argument("--invocation-root", type=Path)
    parser.add_argument("--bootstrap-evidence", type=Path)
    parser.add_argument("--source-manifest-sha256")
    parser.add_argument("--candidate-root", type=Path)
    parser.add_argument("--scaling-evidence-manifest", type=Path)
    parser.add_argument("--expected-signer-fingerprint")
    parser.add_argument("--expected-scaling-trial-harness-sha256")
    parser.add_argument("--expected-scaling-configuration-sha256")
    parser.add_argument("--expected-scaling-irohad-sha256")
    parser.add_argument("--expected-scaling-iroha-cli-sha256")
    parser.add_argument("--validator-exit-status", type=int)
    parser.add_argument("--cleanup-base", type=Path)
    parser.add_argument("--cleanup-prefix")
    args = parser.parse_args(argv)
    try:
        if args.create_sdk_command_work or args.cleanup_sdk_command_work:
            if (
                args.create_sdk_command_work == args.cleanup_sdk_command_work
                or args.sdk_input_root is None
                or args.sdk_work_root is None
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.copy_sdk_dependencies, args.verify_sdk_dependencies,
                    args.cleanup_invocation,
                ))
                or args.runtime_source
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_archive,
                    args.sdk_dependency_inventory,
                    args.sdk_work_final_inventory, args.invocation_root,
                    args.bootstrap_evidence, args.source_manifest_sha256,
                    args.candidate_root, args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
            ):
                raise error_type("SDK command work inputs are not exact")
            if args.create_sdk_command_work:
                create_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            else:
                cleanup_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            return 0
        if args.copy_sdk_dependencies or args.verify_sdk_dependencies:
            if (
                args.copy_sdk_dependencies == args.verify_sdk_dependencies
                or any(value is None for value in (
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_input_root, args.sdk_work_root,
                    args.sdk_archive, args.sdk_dependency_inventory,
                ))
                or (args.copy_sdk_dependencies and args.sdk_work_final_inventory is not None)
                or (args.verify_sdk_dependencies and args.sdk_work_final_inventory is None)
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.create_sdk_command_work,
                    args.cleanup_sdk_command_work,
                    args.cleanup_invocation,
                ))
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.invocation_root, args.bootstrap_evidence,
                    args.source_manifest_sha256, args.candidate_root,
                    args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
                or args.runtime_source
            ):
                raise error_type("SDK dependency bundle inputs are not exact")
            sdk_arguments = (
                args.sdk_dependency_bundle_manifest,
                args.expected_sdk_dependency_bundle_manifest_sha256,
                args.repository_root, args.sdk_input_root, args.sdk_work_root,
                args.sdk_archive, args.sdk_dependency_inventory,
            )
            if args.copy_sdk_dependencies:
                copy_sdk_dependencies(*sdk_arguments)
            else:
                verify_sdk_dependencies(
                    *sdk_arguments,
                    final_work_inventory=args.sdk_work_final_inventory,
                )
            return 0
        if args.copy_framework_python or args.verify_framework_python:
            if (
                args.copy_framework_python == args.verify_framework_python
                or args.runtime_root is None
                or args.runtime_inventory is None
                or args.runtime_source
                or args.source_cargo_home is not None
                or args.cargo_home is not None
                or args.inventory is not None
                or args.final
                or args.publish_validation_failure
                or args.seal_release_result
                or args.copy_runtime
                or args.verify_runtime_sources
                or args.verify_cache_sources
                or args.copy_private_bundle
                or args.verify_private_bundle
                or args.copy_sdk_dependencies
                or args.verify_sdk_dependencies
                or args.create_sdk_command_work
                or args.cleanup_sdk_command_work
                or args.cleanup_invocation
            ):
                raise error_type("framework Python runtime inputs are not exact")
            if args.copy_framework_python:
                copy_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            else:
                verify_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            return 0
        if args.verify_cache_sources:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise error_type("caller cache verification lacks required inputs")
            verify_cache_sources(args.source_cargo_home, args.cargo_home, args.inventory)
        elif args.verify_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise error_type("private bundle verification lacks required inputs")
            verify_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.copy_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise error_type("private bundle copy lacks required inputs")
            copy_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.verify_runtime_sources:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise error_type("runtime source verification lacks its inventory")
            verify_runtime_sources(args.runtime_source, args.runtime_root, args.runtime_inventory)
        elif args.cleanup_invocation:
            if args.cleanup_base is None or args.invocation_root is None or args.cleanup_prefix is None:
                raise error_type("private invocation cleanup lacks required inputs")
            cleanup_invocation(args.cleanup_base, args.invocation_root, args.cleanup_prefix)
        elif args.publish_validation_failure:
            if any(value is None for value in (
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )):
                raise error_type("receipt validation failure lacks required inputs")
            publish_validation_failure(
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )
        elif args.copy_runtime:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise error_type("private child runtime lacks its root")
            copy_runtime(args.runtime_root, args.runtime_source, args.runtime_inventory)
        elif args.seal_release_result:
            if any(value is None for value in (
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )):
                raise error_type("retained release publication lacks required inputs")
            seal_release_result(
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )
        elif args.final:
            if args.cargo_home is None or args.inventory is None:
                raise error_type("final cache snapshot lacks required paths")
            if args.source_cargo_home is not None:
                raise error_type("final cache snapshot does not accept a source home")
            snapshot_cache(args.cargo_home, args.inventory)
        else:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise error_type("cache copy requires a source home")
            copy_cache(args.source_cargo_home, args.cargo_home, args.inventory)
    except (error_type, OSError) as error:
        print(f"release Cargo cache isolation failed: {error}", file=sys.stderr)
        return 1
    return 0
