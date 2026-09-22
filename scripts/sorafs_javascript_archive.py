"""Bounded original npm archive bytes for the SoraFS JavaScript consumer.

This dependency-free, pure parser accepts one gzip stream containing regular
POSIX ustar members rooted at package/. It never extracts, installs, imports or
executes package content. Live producers and original-byte consumers must share
this owner; package/source, dependency, runtime and execution joins are separate.
No environment variables or filesystem paths are consumed.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
import unicodedata
import zlib

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_TAR_BYTES = 256 * 1024 * 1024
MAX_MEMBER_BYTES = 32 * 1024 * 1024
MAX_MEMBERS = 20000
MAX_NAME_BYTES = 1024
MAX_PAX_BYTES = 16 * 1024
MAX_PATH_NODES = 80000
MAX_PATH_BYTES = 16 * 1024 * 1024
_CHUNK = 64 * 1024
_BLOCK = 512


class ArchiveError(ValueError):
    """Original npm bytes do not satisfy the bounded archive contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ArchiveError(message)


@dataclass(frozen=True)
class NpmMember:
    """One inert, regular member with its exact bytes and admitted mode."""
    name: str
    content: bytes
    mode: int


@dataclass(frozen=True)
class NpmArchive:
    """Original compressed bytes and their unambiguous package-relative members."""
    raw: bytes
    members: tuple[NpmMember, ...]
    tar_size: int

    def files(self) -> dict[str, bytes]:
        """Project captured content without reading historical install paths."""
        return {member.name: member.content for member in self.members}


class _GzipReader:
    """Inflate at most one bounded chunk at a time, including tar padding."""
    def __init__(self, raw: bytes, limit: int):
        self.raw = raw
        self.limit = limit
        self.offset = 0
        self.total = 0
        self.pending = b""
        self.decoder = zlib.decompressobj(16 + zlib.MAX_WBITS)

    def read(self, size: int) -> bytes:
        """Return up to size bytes, validating gzip integrity before EOF."""
        parts, remaining = [], size
        while remaining and not self.decoder.eof:
            if self.pending:
                compressed = self.pending
            else:
                compressed = self.raw[self.offset:self.offset + _CHUNK]
                self.offset += len(compressed)
            limit = min(remaining, _CHUNK, self.limit - self.total + 1)
            try:
                output = self.decoder.decompress(compressed, limit)
            except zlib.error as error:
                raise ArchiveError("npm gzip stream is corrupt") from error
            self.pending = self.decoder.unconsumed_tail
            self.total += len(output)
            _require(self.total <= self.limit, "npm inflated tar byte bound")
            if output:
                parts.append(output)
                remaining -= len(output)
            if self.decoder.eof:
                _require(not self.decoder.unused_data and self.offset == len(self.raw),
                         "npm gzip has concatenated streams or trailing bytes")
            else:
                _require(bool(compressed) or bool(output), "npm gzip stream is truncated")
        return b"".join(parts)

    def exact(self, size: int) -> bytes:
        result = self.read(size)
        _require(len(result) == size, "npm tar member or header is truncated")
        return result


def _text(raw: bytes) -> str:
    prefix, separator, padding = raw.partition(b"\0")
    _require(not separator or not padding.strip(b"\0"), "npm tar text padding is not zero")
    try:
        return prefix.decode("utf-8", "strict")
    except UnicodeError as error:
        raise ArchiveError("npm tar text is not UTF-8") from error


def _octal(raw: bytes, *, empty: bool = False) -> int:
    digits = raw.rstrip(b"\0 ").lstrip(b" ")
    if empty and not digits:
        return 0
    _require(re.fullmatch(b"[0-7]+", digits) is not None, "npm tar number is not octal")
    return int(digits, 8)


def _header(block: bytes) -> tuple[str, int, int, int, bytes]:
    _require(block[257:265] == b"ustar\x0000" and not any(block[500:]),
             "npm tar header is not the admitted POSIX ustar layout")
    checksum = sum(block[:148]) + 8 * ord(" ") + sum(block[156:])
    _require(_octal(block[148:156]) == checksum, "npm tar checksum differs")
    name, prefix = _text(block[:100]), _text(block[345:500])
    if prefix:
        name = prefix + "/" + name
    _require(not any(block[157:257]), "npm tar links are forbidden")
    for field in (block[108:116], block[116:124]):
        _octal(field, empty=True)
    for field in (block[329:337], block[337:345]):
        _require(_octal(field, empty=True) == 0, "npm tar device metadata is forbidden")
    _text(block[265:297])
    _text(block[297:329])
    mode, size = _octal(block[100:108]), _octal(block[124:136])
    _require(mode in (0o644, 0o755) or (block[156:157] == b"x" and mode == 0),
             "npm tar member mode is outside the fixed profile")
    return name, mode, size, _octal(block[136:148]), block[156:157]


def _path(name: str) -> str:
    _require(0 < len(name.encode("utf-8")) <= MAX_NAME_BYTES
             and name.startswith("package/"), "npm member must have its bounded package/ root")
    _require(unicodedata.normalize("NFC", name) == name
             and not any(ord(char) < 32 or ord(char) == 127 for char in name)
             and "\\" not in name and ":" not in name,
             "npm member path has a noncanonical spelling")
    parts = name.split("/")
    _require(all(part not in ("", ".", "..") and not part.endswith((" ", "."))
                 and len(part.encode("utf-8")) <= 255 for part in parts),
             "npm member path has an unsafe component")
    return "/".join(parts[1:])


def _pax(raw: bytes) -> dict[str, str]:
    """Admit npm's path and redundant size/mtime, never numeric overrides."""
    fields, offset = {}, 0
    while offset < len(raw):
        length, separator, _ = raw[offset:].partition(b" ")
        _require(bool(separator) and re.fullmatch(b"[1-9][0-9]{0,5}", length) is not None,
                 "npm PAX record length is malformed")
        end = offset + int(length)
        _require(offset + len(length) + 3 < end <= len(raw), "npm PAX record extent differs")
        record = raw[offset + len(length) + 1:end]
        key, separator, value = record[:-1].partition(b"=")
        _require(record.endswith(b"\n") and record.count(b"\n") == 1 and bool(separator)
                 and key in (b"path", b"size", b"mtime") and key.decode() not in fields
                 and b"\0" not in value, "npm PAX fields are duplicate or unsupported")
        fields[key.decode()] = _text(value)
        offset = end
    _require("path" in fields, "npm PAX omits its sole path authority")
    for key in ("size", "mtime"):
        _require(key not in fields or re.fullmatch(r"0|[1-9][0-9]{0,11}", fields[key]) is not None,
                 "npm PAX numeric metadata is not a bounded canonical integer")
    fields["path"] = _path(fields["path"])
    return fields


def _body(reader: _GzipReader, size: int) -> bytes:
    padding = (-size) % _BLOCK
    _require(size + padding <= reader.limit - reader.total,
             "npm declared member exceeds the remaining tar byte bound")
    result = reader.exact(size)
    _require(not any(reader.exact(padding)), "npm tar member padding is not zero")
    return result


def parse_npm_archive(raw: bytes, *, tar_byte_limit: int | None = None) -> NpmArchive:
    """Validate original gzip/tar bytes before retaining bounded regular content.

    Per-member sizes are checked before payload reads; inflated bytes, including
    headers, metadata and trailing zero records, share one fixed budget. This is
    an archive observation, not source provenance or executed SDK qualification.
    """
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_ARCHIVE_BYTES,
             "npm compressed archive byte bound")
    limit = MAX_TAR_BYTES if tar_byte_limit is None else tar_byte_limit
    _require(type(limit) is int and 0 < limit <= MAX_TAR_BYTES,
             "npm shared tar budget must be positive and cannot raise the fixed limit")
    reader = _GzipReader(raw, limit)
    members, spellings, files = [], {}, set()
    path_bytes, pending = 0, None
    while True:
        block = reader.exact(_BLOCK)
        if not any(block):
            _require(pending is None and not any(reader.exact(_BLOCK)),
                     "npm tar requires two zero end blocks and no pending PAX record")
            while tail := reader.read(_BLOCK):
                _require(len(tail) == _BLOCK and not any(tail),
                         "npm tar has trailing content or incomplete record padding")
            _require(bool(members), "npm archive has no regular members")
            return NpmArchive(raw, tuple(members), reader.total)
        name, mode, size, mtime, kind = _header(block)
        if kind == b"x":
            _require(pending is None and 0 < size <= MAX_PAX_BYTES
                     and len(members) < MAX_MEMBERS,
                     "npm PAX count or byte bound")
            pending = _pax(_body(reader, size))
            continue
        _require(kind == b"0", "npm tar contains a non-regular or unsupported member")
        _require(len(members) < MAX_MEMBERS and size <= MAX_MEMBER_BYTES,
                 "npm member count or byte bound")
        # PAX supplies the effective name, but cannot hide an unsafe header path.
        original_name = _path(name)
        name = original_name
        if pending is not None:
            _require(all(key not in pending or int(pending[key]) == value
                         for key, value in (("size", size), ("mtime", mtime))),
                     "npm PAX numeric metadata differs from its following header")
            name, pending = pending["path"], None
        _require(name.casefold() not in files, "npm archive duplicates or aliases a member")
        parts = name.split("/")
        for length in range(1, len(parts) + 1):
            ancestor = "/".join(parts[:length])
            key = ancestor.casefold()
            _require(key not in spellings or spellings[key] == ancestor,
                     "npm archive aliases a path ancestor")
            _require(length == len(parts) or key not in files,
                     "npm archive file is also a path ancestor")
            if length == len(parts):
                _require(key not in spellings, "npm archive replaces a path ancestor with a file")
            if key not in spellings:
                path_bytes += len(ancestor.encode("utf-8"))
                _require(len(spellings) < MAX_PATH_NODES and path_bytes <= MAX_PATH_BYTES,
                         "npm path ownership count or byte bound")
                spellings[key] = ancestor
        files.add(name.casefold())
        members.append(NpmMember(name, _body(reader, size), mode))
