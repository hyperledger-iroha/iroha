#!/usr/bin/env python3
"""Verify that the privacy SDK imports one authenticated private wheel.

The wheel is read, sealed, and structurally validated before any
``iroha_python`` module is imported.  Installed destinations are derived from
the validated wheel members and the private environment's site-packages
directories; module-controlled ``__file__`` attributes are never used to
discover trusted files.
"""

import base64
import csv
import hashlib
import importlib.machinery
import importlib.metadata
import importlib.util
import io
import json
import os
import re
import site
import stat
import struct
import subprocess
import sys
import sysconfig
import unicodedata
import zipimport
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Iterable, NoReturn, Sequence

PACKAGE_NAME = "iroha_python"
PACKAGE_DISTRIBUTION_NAME = "iroha-python"
NATIVE_MODULE_NAME = "iroha_python._crypto"
PACKAGE_INITIALIZER_MEMBER = "iroha_python/__init__.py"
NATIVE_STEM = "_crypto"
NATIVE_FILE_ENDINGS = (".so", ".dylib", ".pyd")
DIST_INFO_PREFIX = "iroha_python-"
DIST_INFO_SUFFIX = ".dist-info"
DIST_INFO_REQUIRED_FILES = ("METADATA", "WHEEL", "RECORD")
PIP_GENERATED_DIST_INFO_FILES = ("INSTALLER", "REQUESTED", "direct_url.json")

MAX_WHEEL_BYTES = 512 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 4_096
MAX_MEMBER_NAME_BYTES = 4_096
MAX_PATH_COMPONENTS = 64
MAX_MEMBER_BYTES = 256 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_BYTES = 512 * 1024 * 1024
MAX_COMPRESSION_RATIO = 200
READ_CHUNK_BYTES = 1024 * 1024
ALLOWED_COMPRESSION = frozenset((zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED))
_SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
_EOCD = struct.Struct("<4s4H2LH")
_LOCAL_FILE_HEADER = struct.Struct("<4s5H3L2H")
_DATA_DESCRIPTOR = struct.Struct("<4s3L")
_DATA_DESCRIPTOR_WITHOUT_SIGNATURE = struct.Struct("<3L")


class VerificationError(RuntimeError):
    """The installed privacy wheel does not satisfy the release gate."""


def _fail(message: str) -> NoReturn:
    raise VerificationError(message)


@dataclass(frozen=True)
class FileSeal:
    """Stable identity, metadata, and SHA-256 for one regular file."""

    sha256: str
    device: int
    inode: int
    size: int
    mtime_ns: int
    ctime_ns: int
    mode: int

    @classmethod
    def parse(cls, value: str) -> "FileSeal":
        """Parse the seal emitted by ``privacy_python_sdk_file_seal``."""

        fields = value.split(":")
        if len(fields) != 7 or _SHA256_RE.fullmatch(fields[0]) is None:
            _fail("expected wheel seal has an invalid format")
        try:
            numeric = tuple(int(field, 10) for field in fields[1:6])
            mode = int(fields[6], 8)
        except ValueError:
            _fail("expected wheel seal has an invalid numeric field")
        if any(number < 0 for number in numeric):
            _fail("expected wheel seal has a negative numeric field")
        if mode < 0 or mode > 0o7777:
            _fail("expected wheel seal has an invalid file mode")
        parsed = cls(fields[0], *numeric, mode)
        if parsed.render() != value:
            _fail("expected wheel seal is not canonically encoded")
        return parsed

    @classmethod
    def from_stat(cls, digest: str, metadata: os.stat_result) -> "FileSeal":
        """Create a seal from a digest and an open descriptor's metadata."""

        return cls(
            digest,
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
            stat.S_IMODE(metadata.st_mode),
        )

    def render(self) -> str:
        """Return the shell helper's canonical seal representation."""

        return ":".join(
            (
                self.sha256,
                str(self.device),
                str(self.inode),
                str(self.size),
                str(self.mtime_ns),
                str(self.ctime_ns),
                oct(self.mode),
            )
        )


@dataclass(frozen=True)
class WheelMember:
    """One authenticated regular-file member from the private wheel."""

    name: str
    sha256: str
    size: int


@dataclass(frozen=True)
class WheelPreflight:
    """Authenticated complete package/dist-info layout for a private wheel."""

    path: Path
    seal: FileSeal
    package_member: str
    native_member: str
    dist_info_root: str
    metadata_version: str
    package_members: tuple[WheelMember, ...]
    package_directories: tuple[str, ...]
    dist_info_members: tuple[WheelMember, ...]
    dist_info_directories: tuple[str, ...]


@dataclass(frozen=True)
class InstalledLayout:
    """Trusted installed destinations derived from a validated wheel."""

    site_root: Path
    package_root: Path
    package_path: Path
    native_path: Path
    dist_info_root: Path


@dataclass(frozen=True)
class StableFile:
    """A stable read of an installed regular file."""

    path: Path
    seal: FileSeal


@dataclass(frozen=True)
class InstalledFileSet:
    """Stable seals for every installed wheel-owned or modeled file."""

    files: tuple[StableFile, ...]
    package: StableFile
    native: StableFile


@dataclass(frozen=True)
class AuthenticatedDependencyRoot:
    """One explicit repository source root used only for package dependencies."""

    path: Path
    module_name: str
    initializer_path: Path
    initializer_seal: FileSeal
    tree_state: tuple[str, ...]


def _stable_stat_tuple(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must be an absolute normalized path")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        _fail(f"{label} is unavailable: {error}")
    if resolved != path or path.is_symlink() or not resolved.is_dir():
        _fail(f"{label} must be a canonical real directory")
    return resolved


def _read_stable_regular_file(
    path: Path,
    *,
    label: str,
    max_bytes: int,
    expected_seal: FileSeal | None = None,
    allow_empty: bool = False,
) -> tuple[bytes, FileSeal]:
    """Read and seal a canonical, singly linked regular file."""

    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must have an absolute normalized path")
    if path.is_symlink():
        _fail(f"{label} must not be a symbolic link")
    try:
        canonical_path = path.resolve(strict=True)
    except OSError as error:
        _fail(f"{label} is unavailable: {error}")
    if canonical_path != path:
        _fail(f"{label} must have no symbolic-link path components")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(canonical_path, flags)
    except OSError as error:
        _fail(f"unable to open {label}: {error}")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            _fail(f"{label} must be a singly linked regular file")
        if (
            (before.st_size == 0 and not allow_empty)
            or before.st_size < 0
            or before.st_size > max_bytes
        ):
            _fail(f"{label} violates its {max_bytes}-byte size bound")

        digest = hashlib.sha256()
        chunks: list[bytes] = []
        observed = 0
        while chunk := os.read(descriptor, READ_CHUNK_BYTES):
            observed += len(chunk)
            if observed > max_bytes:
                _fail(f"{label} exceeds its {max_bytes}-byte size bound")
            digest.update(chunk)
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    try:
        path_state = os.stat(canonical_path, follow_symlinks=False)
    except OSError as error:
        _fail(f"{label} became unavailable after it was read: {error}")
    if (
        _stable_stat_tuple(before) != _stable_stat_tuple(after)
        or observed != before.st_size
        or (path_state.st_dev, path_state.st_ino)
        != (before.st_dev, before.st_ino)
    ):
        _fail(f"{label} changed while it was sealed")

    observed_seal = FileSeal.from_stat(digest.hexdigest(), before)
    if expected_seal is not None and observed_seal != expected_seal:
        _fail(f"{label} does not match the expected shell-provided seal")
    return b"".join(chunks), observed_seal


def assert_expected_file_seal(
    path: Path, expected_seal: FileSeal, *, label: str, max_bytes: int
) -> None:
    """Require a file to retain an earlier authenticated seal."""

    _read_stable_regular_file(
        path,
        label=label,
        max_bytes=max_bytes,
        expected_seal=expected_seal,
    )


def _canonical_zip_member_name(info: zipfile.ZipInfo) -> str:
    """Validate one archive name and return its canonical POSIX spelling."""

    original = info.orig_filename
    name = info.filename
    if original != name or "\x00" in original:
        _fail("wheel member name contains a NUL alias")
    if not name:
        _fail("wheel contains an empty member name")
    if "\\" in name:
        _fail(f"wheel member is not a POSIX path: {name!r}")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in name):
        _fail(f"wheel member contains a control character: {name!r}")
    try:
        encoded_name = name.encode("utf-8", errors="strict")
    except UnicodeError:
        _fail(f"wheel member is not valid UTF-8 text: {name!r}")
    if len(encoded_name) > MAX_MEMBER_NAME_BYTES:
        _fail("wheel member name exceeds the path-length bound")
    if unicodedata.normalize("NFC", name) != name:
        _fail(f"wheel member name is not NFC-normalized: {name!r}")

    is_directory = name.endswith("/")
    path_text = name[:-1] if is_directory else name
    if not path_text or path_text.startswith("/"):
        _fail(f"wheel member must be a relative POSIX path: {name!r}")
    if PureWindowsPath(path_text).drive:
        _fail(f"wheel member must not contain a drive path: {name!r}")
    components = path_text.split("/")
    if len(components) > MAX_PATH_COMPONENTS:
        _fail("wheel member exceeds the path-depth bound")
    if any(component in ("", ".", "..") for component in components):
        _fail(f"wheel member contains a dot or empty path alias: {name!r}")
    if any(
        ":" in component or component.endswith((".", " "))
        for component in components
    ):
        _fail(f"wheel member contains a platform path alias: {name!r}")

    pure_path = PurePosixPath(path_text)
    if pure_path.is_absolute() or pure_path.as_posix() != path_text:
        _fail(f"wheel member path is not canonical: {name!r}")
    canonical = path_text + ("/" if is_directory else "")
    if canonical != name or info.is_dir() != is_directory:
        _fail(f"wheel member directory spelling is not canonical: {name!r}")
    return canonical


def _assert_safe_zip_mode(info: zipfile.ZipInfo, name: str) -> None:
    """Reject symlinks, devices, sockets, and file/directory mode mismatches."""

    unix_mode = (info.external_attr >> 16) & 0xFFFF
    file_type = stat.S_IFMT(unix_mode)
    expected_type = stat.S_IFDIR if info.is_dir() else stat.S_IFREG
    if file_type not in (0, expected_type):
        _fail(f"wheel member has a symbolic-link or special-file mode: {name!r}")
    dos_directory = bool(info.external_attr & 0x10)
    if dos_directory != info.is_dir() and info.create_system != 3:
        _fail(f"wheel member has inconsistent directory attributes: {name!r}")
    if info.is_dir() and (info.file_size != 0 or info.compress_size != 0):
        _fail(f"wheel directory member contains data: {name!r}")


def _member_is_native_like(name: str) -> bool:
    if name.endswith("/"):
        return False
    basename = PurePosixPath(name).name
    return basename.casefold().endswith(NATIVE_FILE_ENDINGS)


def _stream_member_digest(
    wheel: zipfile.ZipFile, info: zipfile.ZipInfo
) -> str:
    digest = hashlib.sha256()
    observed = 0
    with wheel.open(info, "r") as source:
        while chunk := source.read(READ_CHUNK_BYTES):
            observed += len(chunk)
            if observed > info.file_size or observed > MAX_MEMBER_BYTES:
                _fail(f"wheel member exceeded its declared bound: {info.filename!r}")
            digest.update(chunk)
    if observed != info.file_size:
        _fail(f"wheel member size changed while read: {info.filename!r}")
    return digest.hexdigest()


def _read_member_bytes(
    wheel: zipfile.ZipFile, info: zipfile.ZipInfo, *, label: str
) -> bytes:
    """Read one already-bounded archive member exactly."""

    chunks: list[bytes] = []
    observed = 0
    with wheel.open(info, "r") as source:
        while chunk := source.read(READ_CHUNK_BYTES):
            observed += len(chunk)
            if observed > info.file_size or observed > MAX_MEMBER_BYTES:
                _fail(f"{label} exceeded its declared bound")
            chunks.append(chunk)
    if observed != info.file_size:
        _fail(f"{label} size changed while read")
    return b"".join(chunks)


def _record_hash(digest: str) -> str:
    encoded = base64.urlsafe_b64encode(bytes.fromhex(digest)).rstrip(b"=")
    return f"sha256={encoded.decode('ascii')}"


def _assert_record_payload(
    payload: bytes,
    *,
    expected_files: dict[str, tuple[str, int]],
    record_name: str,
    label: str,
) -> None:
    """Require exact, canonical SHA-256 RECORD coverage."""

    try:
        text = payload.decode("utf-8", errors="strict")
        rows = tuple(csv.reader(io.StringIO(text, newline=""), strict=True))
    except (UnicodeError, csv.Error) as error:
        _fail(f"{label} is not strict UTF-8 CSV: {error}")
    if not rows:
        _fail(f"{label} must not be empty")

    observed: set[str] = set()
    expected_names = set(expected_files)
    expected_names.add(record_name)
    for row in rows:
        if len(row) != 3:
            _fail(f"{label} row must contain exactly three columns")
        name, hash_field, size_field = row
        if name in observed:
            _fail(f"{label} contains a duplicate member row")
        if name not in expected_names:
            _fail(f"{label} contains an unexpected member row: {name!r}")
        observed.add(name)
        if name == record_name:
            if hash_field or size_field:
                _fail(f"{label} must leave its own hash and size empty")
            continue
        digest, size = expected_files[name]
        if hash_field != _record_hash(digest) or size_field != str(size):
            _fail(f"{label} hash or size does not match {name!r}")
    if observed != expected_names:
        _fail(f"{label} does not cover every installed file exactly once")


def _metadata_identity(payload: bytes) -> tuple[str, str]:
    """Extract one exact distribution name/version pair from METADATA."""

    try:
        text = payload.decode("utf-8", errors="strict")
    except UnicodeError as error:
        _fail(f"wheel METADATA is not strict UTF-8: {error}")
    names: list[str] = []
    versions: list[str] = []
    for line in text.splitlines():
        if not line:
            break
        if line.startswith((" ", "\t")):
            continue
        key, separator, value = line.partition(":")
        if not separator:
            _fail("wheel METADATA contains a malformed header")
        if key.casefold() == "name":
            names.append(value.strip())
        elif key.casefold() == "version":
            versions.append(value.strip())
    if len(names) != 1 or len(versions) != 1:
        _fail("wheel METADATA must contain exactly one Name and Version")
    normalized_name = re.sub(r"[-_.]+", "-", names[0]).casefold()
    if normalized_name != PACKAGE_DISTRIBUTION_NAME:
        _fail("wheel METADATA names a different distribution")
    version = versions[0]
    if (
        not version
        or version != version.strip()
        or any(ord(character) < 0x21 or ord(character) == 0x7F for character in version)
    ):
        _fail("wheel METADATA contains an invalid Version")
    return names[0], version


def _assert_canonical_zip_envelope(
    payload: bytes, wheel: zipfile.ZipFile, infos: Sequence[zipfile.ZipInfo]
) -> None:
    """Require one non-spanned ZIP with no prepended/appended/comment payload."""

    if len(payload) < _EOCD.size:
        _fail("fresh wheel is too short to contain a canonical ZIP end record")
    (
        signature,
        disk_number,
        central_disk,
        disk_members,
        total_members,
        central_size,
        central_offset,
        comment_size,
    ) = _EOCD.unpack(payload[-_EOCD.size :])
    if signature != b"PK\x05\x06":
        _fail("fresh wheel must not contain data outside its canonical ZIP records")
    if comment_size != 0 or wheel.comment:
        _fail("fresh wheel must have one canonical un-commented ZIP end record")
    if disk_number != 0 or central_disk != 0 or disk_members != total_members:
        _fail("fresh wheel must not use split or spanned ZIP records")
    if total_members != len(infos):
        _fail("fresh wheel central-directory member count is inconsistent")
    if central_offset != wheel.start_dir:
        _fail("fresh wheel must not contain a prepended payload or ZIP64 alias")
    if central_offset + central_size != len(payload) - _EOCD.size:
        _fail("fresh wheel must not contain data outside its canonical ZIP records")
    _assert_contiguous_local_records(payload, central_offset, infos)


def _assert_contiguous_local_records(
    payload: bytes,
    central_offset: int,
    infos: Sequence[zipfile.ZipInfo],
) -> None:
    """Require local records and explicit data descriptors to cover every byte."""

    ordered_infos = sorted(infos, key=lambda info: info.header_offset)
    expected_offset = 0
    for index, info in enumerate(ordered_infos):
        if info.header_offset != expected_offset:
            _fail(
                "fresh wheel local records do not contiguously cover "
                "the pre-central payload"
            )
        fixed_header_end = expected_offset + _LOCAL_FILE_HEADER.size
        if fixed_header_end > central_offset:
            _fail("fresh wheel local record overlaps its central directory")
        (
            signature,
            extract_version,
            local_flags,
            compression,
            _modified_time,
            _modified_date,
            local_crc,
            local_compressed_size,
            local_file_size,
            name_size,
            extra_size,
        ) = _LOCAL_FILE_HEADER.unpack(payload[expected_offset:fixed_header_end])
        if signature != b"PK\x03\x04":
            _fail("fresh wheel contains a non-canonical local record signature")
        if (
            extract_version != info.extract_version
            or local_flags != info.flag_bits
            or compression != info.compress_type
        ):
            _fail("fresh wheel local and central record policies disagree")

        name_end = fixed_header_end + name_size
        extra_end = name_end + extra_size
        data_end = extra_end + info.compress_size
        if data_end > central_offset:
            _fail("fresh wheel local record exceeds its pre-central payload")
        try:
            expected_name = info.orig_filename.encode("utf-8", errors="strict")
        except UnicodeError:
            _fail("fresh wheel local member name is not canonical UTF-8")
        if payload[fixed_header_end:name_end] != expected_name:
            _fail("fresh wheel local and central member names disagree")
        if local_compressed_size == 0xFFFFFFFF or local_file_size == 0xFFFFFFFF:
            _fail("fresh wheel must not use ZIP64 local-size aliases")

        next_offset = (
            ordered_infos[index + 1].header_offset
            if index + 1 < len(ordered_infos)
            else central_offset
        )
        if info.flag_bits & 0x08:
            if local_crc != 0 or local_compressed_size != 0 or local_file_size != 0:
                _fail("fresh wheel data-descriptor local header is not canonical")
            descriptor = payload[data_end:next_offset]
            if len(descriptor) == _DATA_DESCRIPTOR.size:
                (
                    descriptor_signature,
                    descriptor_crc,
                    descriptor_compressed_size,
                    descriptor_file_size,
                ) = _DATA_DESCRIPTOR.unpack(descriptor)
                if descriptor_signature != b"PK\x07\x08":
                    _fail("fresh wheel data descriptor has an invalid signature")
            elif len(descriptor) == _DATA_DESCRIPTOR_WITHOUT_SIGNATURE.size:
                (
                    descriptor_crc,
                    descriptor_compressed_size,
                    descriptor_file_size,
                ) = _DATA_DESCRIPTOR_WITHOUT_SIGNATURE.unpack(descriptor)
            else:
                _fail(
                    "fresh wheel data descriptor does not exactly cover "
                    "its local record"
                )
            if (
                descriptor_crc != info.CRC
                or descriptor_compressed_size != info.compress_size
                or descriptor_file_size != info.file_size
            ):
                _fail("fresh wheel data descriptor disagrees with its central record")
        else:
            if (
                local_crc != info.CRC
                or local_compressed_size != info.compress_size
                or local_file_size != info.file_size
            ):
                _fail("fresh wheel local and central sizes or CRC disagree")
            if data_end != next_offset:
                _fail(
                    "fresh wheel local records do not contiguously cover "
                    "the pre-central payload"
                )
        expected_offset = next_offset
    if expected_offset != central_offset:
        _fail(
            "fresh wheel local records do not contiguously cover "
            "the pre-central payload"
        )


def seal_wheel(wheel_path: Path) -> FileSeal:
    """Return a stable seal for one canonical private wheel candidate."""

    _payload, seal = _read_stable_regular_file(
        wheel_path,
        label="fresh private wheel",
        max_bytes=MAX_WHEEL_BYTES,
    )
    return seal


def preflight_wheel(
    wheel_path: Path,
    expected_wheel_seal: str | FileSeal,
    *,
    extension_suffixes: Sequence[str] | None = None,
) -> WheelPreflight:
    """Authenticate and structurally validate a wheel without importing it."""

    expected = (
        FileSeal.parse(expected_wheel_seal)
        if isinstance(expected_wheel_seal, str)
        else expected_wheel_seal
    )
    if not isinstance(expected, FileSeal):
        _fail("expected wheel seal must be a FileSeal or its canonical text")
    payload, observed_seal = _read_stable_regular_file(
        wheel_path,
        label="fresh private wheel",
        max_bytes=MAX_WHEEL_BYTES,
        expected_seal=expected,
    )

    suffixes = tuple(
        importlib.machinery.EXTENSION_SUFFIXES
        if extension_suffixes is None
        else extension_suffixes
    )
    if (
        not suffixes
        or any(
            not suffix
            or "/" in suffix
            or "\\" in suffix
            or not suffix.endswith((".so", ".pyd"))
            for suffix in suffixes
        )
    ):
        _fail("current interpreter reported invalid extension suffixes")
    expected_native_names = {
        f"iroha_python/{NATIVE_STEM}{suffix}" for suffix in suffixes
    }

    try:
        with zipfile.ZipFile(io.BytesIO(payload), "r") as wheel:
            infos = wheel.infolist()
            if not infos or len(infos) > MAX_ARCHIVE_MEMBERS:
                _fail(
                    "fresh wheel must contain between one and "
                    f"{MAX_ARCHIVE_MEMBERS} members"
                )
            _assert_canonical_zip_envelope(payload, wheel, infos)
            if min(info.header_offset for info in infos) != 0:
                _fail("fresh wheel must not contain a prepended payload")

            normalized_names: dict[str, str] = {}
            infos_by_name: dict[str, zipfile.ZipInfo] = {}
            header_offsets: set[int] = set()
            total_uncompressed = 0
            for info in infos:
                name = _canonical_zip_member_name(info)
                normalized_key = unicodedata.normalize(
                    "NFC", name.rstrip("/")
                ).casefold()
                prior = normalized_names.get(normalized_key)
                if prior is not None:
                    _fail(
                        "wheel members are not unique after path normalization: "
                        f"{prior!r} and {name!r}"
                    )
                normalized_names[normalized_key] = name
                infos_by_name[name] = info

                _assert_safe_zip_mode(info, name)
                if info.flag_bits & 0x1:
                    _fail(f"wheel member must not be encrypted: {name!r}")
                if info.compress_type not in ALLOWED_COMPRESSION:
                    _fail(f"wheel member uses unsupported compression: {name!r}")
                if (
                    info.file_size < 0
                    or info.compress_size < 0
                    or info.file_size > MAX_MEMBER_BYTES
                    or info.compress_size > MAX_WHEEL_BYTES
                ):
                    _fail(f"wheel member violates an archive size bound: {name!r}")
                if (
                    info.file_size > 0
                    and info.compress_size == 0
                    and not info.is_dir()
                ):
                    _fail(f"wheel member has an invalid zero compressed size: {name!r}")
                if (
                    info.compress_size > 0
                    and info.file_size
                    > info.compress_size * MAX_COMPRESSION_RATIO
                ):
                    _fail(f"wheel member exceeds the compression-ratio bound: {name!r}")
                total_uncompressed += info.file_size
                if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_BYTES:
                    _fail("wheel exceeds the total uncompressed-size bound")
                if (
                    info.header_offset < 0
                    or info.header_offset >= wheel.start_dir
                    or info.header_offset in header_offsets
                ):
                    _fail(f"wheel member has an invalid local-header offset: {name!r}")
                header_offsets.add(info.header_offset)

            package_infos = [
                info
                for name, info in infos_by_name.items()
                if name == PACKAGE_INITIALIZER_MEMBER
            ]
            package_tree_infos = [
                info
                for name, info in infos_by_name.items()
                if name.startswith(f"{PACKAGE_NAME}/")
            ]
            current_native_infos = [
                info
                for name, info in infos_by_name.items()
                if name in expected_native_names
            ]
            native_like_names = {
                name for name in infos_by_name if _member_is_native_like(name)
            }
            if len(package_infos) != 1:
                _fail("fresh wheel must contain exactly one package initializer")
            if len(current_native_infos) != 1:
                _fail(
                    "fresh wheel must contain exactly one "
                    "current-platform native module"
                )
            selected_native_name = current_native_infos[0].filename
            if native_like_names != {selected_native_name}:
                _fail(
                    "fresh wheel contains a loose, nested, or non-current native module"
                )
            top_level_roots = {
                PurePosixPath(name.rstrip("/")).parts[0]
                for name in infos_by_name
            }
            dist_info_roots = {
                root
                for root in top_level_roots
                if root.startswith(DIST_INFO_PREFIX)
                and root.endswith(DIST_INFO_SUFFIX)
            }
            if len(dist_info_roots) != 1:
                _fail("fresh wheel must contain exactly one iroha_python dist-info root")
            dist_info_root = next(iter(dist_info_roots))
            if top_level_roots != {PACKAGE_NAME, dist_info_root}:
                _fail(
                    "fresh wheel must not contain scripts, data roots, or other packages"
                )
            required_dist_info_members = {
                f"{dist_info_root}/{filename}"
                for filename in DIST_INFO_REQUIRED_FILES
            }
            if not required_dist_info_members.issubset(infos_by_name):
                _fail(
                    "fresh wheel dist-info must contain METADATA, WHEEL, and RECORD"
                )
            generated_dist_info_members = {
                f"{dist_info_root}/{filename}"
                for filename in PIP_GENERATED_DIST_INFO_FILES
            }
            if generated_dist_info_members.intersection(infos_by_name):
                _fail(
                    "fresh wheel must not preseed pip-generated dist-info files"
                )
            forbidden_package_members = {
                info.filename
                for info in package_tree_infos
                if not info.is_dir()
                and PurePosixPath(info.filename).name.casefold().endswith(
                    (".pyc", ".pyo")
                )
            }
            if forbidden_package_members:
                _fail("fresh wheel package must not contain bytecode")

            member_digests: dict[str, str] = {}
            for info in infos:
                if not info.is_dir():
                    member_digests[info.filename] = _stream_member_digest(
                        wheel, info
                    )
            metadata_name = f"{dist_info_root}/METADATA"
            record_name = f"{dist_info_root}/RECORD"
            metadata_payload = _read_member_bytes(
                wheel,
                infos_by_name[metadata_name],
                label="wheel METADATA",
            )
            _metadata_distribution_name, metadata_version = _metadata_identity(
                metadata_payload
            )
            wheel_record_payload = _read_member_bytes(
                wheel,
                infos_by_name[record_name],
                label="wheel RECORD",
            )
            _assert_record_payload(
                wheel_record_payload,
                expected_files={
                    name: (member_digests[name], info.file_size)
                    for name, info in infos_by_name.items()
                    if not info.is_dir() and name != record_name
                },
                record_name=record_name,
                label="wheel RECORD",
            )
    except VerificationError:
        raise
    except (
        EOFError,
        NotImplementedError,
        RuntimeError,
        zipfile.BadZipFile,
        zipfile.LargeZipFile,
    ) as error:
        _fail(f"fresh private wheel is not a valid bounded ZIP archive: {error}")

    return WheelPreflight(
        path=wheel_path,
        seal=observed_seal,
        package_member=PACKAGE_INITIALIZER_MEMBER,
        native_member=selected_native_name,
        dist_info_root=dist_info_root,
        metadata_version=metadata_version,
        package_members=tuple(
            WheelMember(
                name=info.filename,
                sha256=member_digests[info.filename],
                size=info.file_size,
            )
            for info in sorted(
                package_tree_infos,
                key=lambda candidate: candidate.filename,
            )
            if not info.is_dir()
        ),
        package_directories=tuple(
            info.filename
            for info in sorted(
                package_tree_infos,
                key=lambda candidate: candidate.filename,
            )
            if info.is_dir()
        ),
        dist_info_members=tuple(
            WheelMember(
                name=info.filename,
                sha256=member_digests[info.filename],
                size=info.file_size,
            )
            for info in sorted(
                infos,
                key=lambda candidate: candidate.filename,
            )
            if not info.is_dir()
            and info.filename.startswith(f"{dist_info_root}/")
        ),
        dist_info_directories=tuple(
            info.filename
            for info in sorted(
                infos,
                key=lambda candidate: candidate.filename,
            )
            if info.is_dir()
            and info.filename.startswith(f"{dist_info_root}/")
        ),
    )


def canonical_site_roots(
    environment_root: Path, site_roots: Iterable[Path]
) -> tuple[Path, ...]:
    """Validate and normalize private-environment site-packages roots."""

    environment_root = _canonical_directory(environment_root, "private venv")
    roots: set[Path] = set()
    for path in site_roots:
        root = _canonical_directory(path, "private venv site-packages")
        if not root.is_relative_to(environment_root):
            _fail(f"site-packages escaped the private venv: {root}")
        roots.add(root)
    if not roots:
        _fail("private venv has no site-packages directory")
    return tuple(sorted(roots, key=os.fspath))


def _path_lexists(path: Path) -> bool:
    return os.path.lexists(path)


def _tree_expectations(
    *,
    root_name: str,
    members: Sequence[WheelMember],
    explicit_directories: Sequence[str],
) -> tuple[set[str], set[str]]:
    files: set[str] = set()
    directories: set[str] = set()
    prefix = f"{root_name}/"
    for member in members:
        if not member.name.startswith(prefix):
            _fail("authenticated wheel member escaped its recorded root")
        relative = member.name.removeprefix(prefix)
        if not relative:
            _fail("authenticated wheel file has an empty relative path")
        files.add(relative)
        parts = PurePosixPath(relative).parts
        directories.update(
            PurePosixPath(*parts[:index]).as_posix()
            for index in range(1, len(parts))
        )
    for name in explicit_directories:
        if not name.startswith(prefix):
            _fail("authenticated wheel directory escaped its recorded root")
        relative = name.removeprefix(prefix).removesuffix("/")
        if relative:
            directories.add(relative)
            parts = PurePosixPath(relative).parts
            directories.update(
                PurePosixPath(*parts[:index]).as_posix()
                for index in range(1, len(parts))
            )
    if files.intersection(directories):
        _fail("authenticated wheel layout aliases a file and directory")
    return files, directories


def _inspect_exact_installed_tree(
    root: Path,
    *,
    expected_files: set[str],
    expected_directories: set[str],
    label: str,
) -> dict[str, Path]:
    """Reject every missing, extra, aliased, linked, or special tree entry."""

    _canonical_directory(root, label)
    observed_files: dict[str, Path] = {}
    observed_directories: set[str] = set()
    pending = [root]
    observed_entries = 0
    while pending:
        directory = pending.pop()
        try:
            entries = sorted(directory.iterdir(), key=lambda path: path.name)
        except OSError as error:
            _fail(f"unable to inspect {label}: {error}")
        for entry in entries:
            observed_entries += 1
            if observed_entries > MAX_ARCHIVE_MEMBERS:
                _fail(f"{label} exceeds the installed entry-count bound")
            try:
                relative = entry.relative_to(root).as_posix()
                metadata = entry.lstat()
            except OSError as error:
                _fail(f"unable to stat {label} entry: {error}")
            if entry.is_symlink():
                _fail(f"{label} contains a symbolic link")
            if stat.S_ISDIR(metadata.st_mode):
                if entry.name.casefold() == "__pycache__":
                    _fail(f"{label} contains a bytecode cache directory")
                try:
                    if entry.resolve(strict=True) != entry:
                        _fail(f"{label} contains a non-canonical directory")
                except OSError as error:
                    _fail(f"unable to resolve {label} directory: {error}")
                observed_directories.add(relative)
                pending.append(entry)
                continue
            if not stat.S_ISREG(metadata.st_mode):
                _fail(f"{label} contains a non-regular file")
            if metadata.st_nlink != 1:
                _fail(f"{label} contains a multiply linked file")
            folded_name = entry.name.casefold()
            if folded_name.endswith((".pyc", ".pyo")):
                _fail(f"{label} contains bytecode")
            observed_files[relative] = entry
    if set(observed_files) != expected_files:
        _fail(f"{label} file layout does not exactly match the fresh wheel")
    if observed_directories != expected_directories:
        _fail(f"{label} directory layout does not exactly match the fresh wheel")
    return observed_files


def _is_matching_distribution_entry(name: str) -> bool:
    folded = name.casefold()
    distribution_aliases = (
        PACKAGE_NAME,
        PACKAGE_DISTRIBUTION_NAME,
        PACKAGE_DISTRIBUTION_NAME.replace("-", "."),
    )
    return any(
        folded == f"{alias}{suffix}"
        or folded.startswith(f"{alias}-")
        and folded.endswith(suffix)
        for alias in distribution_aliases
        for suffix in (".dist-info", ".egg-info")
    )


def _assert_installed_layout(
    wheel: WheelPreflight,
    layout: InstalledLayout,
) -> tuple[dict[str, Path], dict[str, Path]]:
    package_files, package_directories = _tree_expectations(
        root_name=PACKAGE_NAME,
        members=wheel.package_members,
        explicit_directories=wheel.package_directories,
    )
    installed_package_files = _inspect_exact_installed_tree(
        layout.package_root,
        expected_files=package_files,
        expected_directories=package_directories,
        label="installed iroha_python package",
    )

    dist_info_files, dist_info_directories = _tree_expectations(
        root_name=wheel.dist_info_root,
        members=wheel.dist_info_members,
        explicit_directories=wheel.dist_info_directories,
    )
    dist_info_files.update(PIP_GENERATED_DIST_INFO_FILES)
    installed_dist_info_files = _inspect_exact_installed_tree(
        layout.dist_info_root,
        expected_files=dist_info_files,
        expected_directories=dist_info_directories,
        label="installed iroha_python dist-info",
    )
    return installed_package_files, installed_dist_info_files


def derive_installed_layout(
    *,
    environment_root: Path,
    site_roots: Iterable[Path],
    wheel: WheelPreflight,
) -> InstalledLayout:
    """Derive and authenticate the unique complete installed layout."""

    roots = canonical_site_roots(environment_root, site_roots)
    package_parts = PurePosixPath(wheel.package_member).parts
    native_parts = PurePosixPath(wheel.native_member).parts
    matches: list[InstalledLayout] = []
    matching_distribution_entries: list[Path] = []
    for root in roots:
        try:
            top_level_entries = tuple(root.iterdir())
        except OSError as error:
            _fail(f"unable to inspect private site-packages: {error}")
        matching_distribution_entries.extend(
            entry
            for entry in top_level_entries
            if _is_matching_distribution_entry(entry.name)
        )
        package_root = root / PACKAGE_NAME
        if _path_lexists(package_root):
            matches.append(
                InstalledLayout(
                    site_root=root,
                    package_root=package_root,
                    package_path=root.joinpath(*package_parts),
                    native_path=root.joinpath(*native_parts),
                    dist_info_root=root / wheel.dist_info_root,
                )
            )
    if len(matches) != 1:
        _fail(
            "fresh wheel must have exactly one complete installation in "
            "private site-packages"
        )
    layout = matches[0]
    _canonical_directory(layout.package_root, "installed iroha_python package")
    _canonical_directory(
        layout.dist_info_root,
        "installed iroha_python dist-info",
    )
    if matching_distribution_entries != [layout.dist_info_root]:
        _fail(
            "private venv must contain exactly one matching iroha-python "
            "distribution origin"
        )
    _assert_installed_layout(wheel, layout)
    return layout


def _read_installed_file(
    path: Path,
    *,
    label: str,
    site_root: Path,
    expected_digest: str | None = None,
    expected_size: int | None = None,
) -> StableFile:
    if not path.is_relative_to(site_root):
        _fail(f"{label} escaped private venv site-packages")
    payload, seal = _read_stable_regular_file(
        path,
        label=label,
        max_bytes=MAX_MEMBER_BYTES,
        allow_empty=True,
    )
    if expected_digest is not None and seal.sha256 != expected_digest:
        _fail(f"{label} does not match the fresh wheel")
    if expected_size is not None and seal.size != expected_size:
        _fail(f"{label} size does not match the fresh wheel")
    # Keep the payload live through the digest comparison so this helper
    # authenticates actual bytes rather than metadata alone.
    if len(payload) != seal.size:
        _fail(f"{label} changed while its installed bytes were verified")
    return StableFile(path=path, seal=seal)


def _assert_direct_url(payload: bytes, wheel: WheelPreflight) -> None:
    try:
        value = json.loads(payload.decode("utf-8", errors="strict"))
    except (UnicodeError, json.JSONDecodeError) as error:
        _fail(f"installed direct_url.json is invalid: {error}")
    if not isinstance(value, dict) or set(value) != {"archive_info", "url"}:
        _fail("installed direct_url.json has an unexpected top-level policy")
    if value["url"] != wheel.path.as_uri():
        _fail("installed direct_url.json does not name the authenticated wheel")
    archive_info = value["archive_info"]
    if not isinstance(archive_info, dict) or set(archive_info) not in (
        {"hashes"},
        {"hash", "hashes"},
    ):
        _fail("installed direct_url.json has an unexpected archive policy")
    if archive_info["hashes"] != {"sha256": wheel.seal.sha256}:
        _fail("installed direct_url.json has the wrong wheel digest")
    if (
        "hash" in archive_info
        and archive_info["hash"] != f"sha256={wheel.seal.sha256}"
    ):
        _fail("installed direct_url.json has the wrong legacy wheel digest")


def verify_installed_files(
    wheel: WheelPreflight,
    layout: InstalledLayout,
) -> InstalledFileSet:
    """Seal and authenticate every installed package and dist-info file."""

    package_paths, dist_info_paths = _assert_installed_layout(wheel, layout)
    stable_by_name: dict[str, StableFile] = {}
    for member in wheel.package_members:
        relative = member.name.removeprefix(f"{PACKAGE_NAME}/")
        stable_by_name[member.name] = _read_installed_file(
            package_paths[relative],
            label=f"installed {member.name}",
            site_root=layout.site_root,
            expected_digest=member.sha256,
            expected_size=member.size,
        )

    record_name = f"{wheel.dist_info_root}/RECORD"
    for member in wheel.dist_info_members:
        if member.name == record_name:
            continue
        relative = member.name.removeprefix(f"{wheel.dist_info_root}/")
        stable_by_name[member.name] = _read_installed_file(
            dist_info_paths[relative],
            label=f"installed {member.name}",
            site_root=layout.site_root,
            expected_digest=member.sha256,
            expected_size=member.size,
        )

    installer_name = f"{wheel.dist_info_root}/INSTALLER"
    requested_name = f"{wheel.dist_info_root}/REQUESTED"
    direct_url_name = f"{wheel.dist_info_root}/direct_url.json"
    for name, expected_payload in (
        (installer_name, b"pip\n"),
        (requested_name, b""),
    ):
        relative = name.removeprefix(f"{wheel.dist_info_root}/")
        payload, _seal = _read_stable_regular_file(
            dist_info_paths[relative],
            label=f"installed {name}",
            max_bytes=MAX_MEMBER_BYTES,
            allow_empty=True,
        )
        if payload != expected_payload:
            _fail(f"installed {name} does not match the modeled pip output")
        stable_by_name[name] = StableFile(
            path=dist_info_paths[relative],
            seal=_seal,
        )

    direct_url_relative = direct_url_name.removeprefix(
        f"{wheel.dist_info_root}/"
    )
    direct_url_payload, direct_url_seal = _read_stable_regular_file(
        dist_info_paths[direct_url_relative],
        label=f"installed {direct_url_name}",
        max_bytes=MAX_MEMBER_BYTES,
        allow_empty=False,
    )
    _assert_direct_url(direct_url_payload, wheel)
    stable_by_name[direct_url_name] = StableFile(
        path=dist_info_paths[direct_url_relative],
        seal=direct_url_seal,
    )

    record_relative = record_name.removeprefix(f"{wheel.dist_info_root}/")
    record_payload, record_seal = _read_stable_regular_file(
        dist_info_paths[record_relative],
        label="installed RECORD",
        max_bytes=MAX_MEMBER_BYTES,
        allow_empty=False,
    )
    _assert_record_payload(
        record_payload,
        expected_files={
            name: (installed.seal.sha256, installed.seal.size)
            for name, installed in stable_by_name.items()
        },
        record_name=record_name,
        label="installed RECORD",
    )
    stable_by_name[record_name] = StableFile(
        path=dist_info_paths[record_relative],
        seal=record_seal,
    )

    package = stable_by_name[wheel.package_member]
    native = stable_by_name[wheel.native_member]
    return InstalledFileSet(
        files=tuple(
            stable_by_name[name] for name in sorted(stable_by_name)
        ),
        package=package,
        native=native,
    )


def reject_preseeded_modules(modules: dict[str, object] | None = None) -> None:
    """Reject an interpreter that already contains the package or descendants."""

    module_table = sys.modules if modules is None else modules
    seeded = sorted(
        name
        for name in module_table
        if name == PACKAGE_NAME or name.startswith(f"{PACKAGE_NAME}.")
    )
    if seeded:
        _fail(
            "privacy wheel verification rejects preseeded package modules: "
            + ", ".join(seeded)
        )


def _spec_origin(spec: importlib.machinery.ModuleSpec, label: str) -> Path:
    if not spec.has_location or not isinstance(spec.origin, str):
        _fail(f"{label} import spec must have a concrete filesystem origin")
    origin = Path(spec.origin)
    if not origin.is_absolute():
        _fail(f"{label} import spec origin must be absolute")
    try:
        canonical = origin.resolve(strict=True)
    except OSError as error:
        _fail(f"{label} import spec origin is unavailable: {error}")
    if canonical != origin:
        _fail(f"{label} import spec origin must be canonical")
    return canonical


def _capture_dependency_tree(package_root: Path, module_name: str) -> tuple[str, ...]:
    """Seal every importable source and reject bytecode/native loader aliases."""

    state: list[str] = []
    pending = [package_root]
    while pending:
        directory = pending.pop()
        try:
            entries = sorted(directory.iterdir(), key=lambda path: path.name)
        except OSError as error:
            _fail(f"unable to inspect authenticated {module_name} sources: {error}")
        for entry in entries:
            if len(state) >= MAX_ARCHIVE_MEMBERS:
                _fail(
                    f"authenticated {module_name} source tree exceeds "
                    "the entry-count bound"
                )
            try:
                relative = entry.relative_to(package_root).as_posix()
                metadata = entry.lstat()
            except OSError as error:
                _fail(
                    f"unable to stat authenticated {module_name} source: {error}"
                )
            if entry.is_symlink():
                _fail(
                    f"authenticated {module_name} source tree contains a symbolic link"
                )
            if stat.S_ISDIR(metadata.st_mode):
                if entry.name.casefold() == "__pycache__":
                    _fail(
                        f"authenticated {module_name} source tree contains bytecode"
                    )
                try:
                    if entry.resolve(strict=True) != entry:
                        _fail(
                            f"authenticated {module_name} source directory "
                            "is not canonical"
                        )
                except OSError as error:
                    _fail(
                        f"authenticated {module_name} source directory "
                        f"is unavailable: {error}"
                    )
                state.append(
                    f"d:{relative}:{metadata.st_dev}:{metadata.st_ino}:"
                    f"{stat.S_IMODE(metadata.st_mode):o}"
                )
                pending.append(entry)
                continue
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                _fail(
                    f"authenticated {module_name} source tree contains "
                    "a special or multiply linked file"
                )
            folded_name = entry.name.casefold()
            if folded_name.endswith(
                (".pyc", ".pyo", *NATIVE_FILE_ENDINGS)
            ):
                _fail(
                    f"authenticated {module_name} source tree contains "
                    "a bytecode or native loader alias"
                )
            if folded_name.endswith(".py"):
                _, seal = _read_stable_regular_file(
                    entry,
                    label=f"authenticated {module_name} source",
                    max_bytes=MAX_MEMBER_BYTES,
                )
                state.append(f"p:{relative}:{seal.render()}")
            else:
                state.append(
                    f"f:{relative}:{metadata.st_dev}:{metadata.st_ino}:"
                    f"{metadata.st_size}:{metadata.st_mtime_ns}:"
                    f"{metadata.st_ctime_ns}:{stat.S_IMODE(metadata.st_mode):o}"
                )
    return tuple(sorted(state))


def authenticate_dependency_roots(
    *,
    environment_root: Path,
    norito_root: Path,
    torii_root: Path,
) -> tuple[AuthenticatedDependencyRoot, AuthenticatedDependencyRoot]:
    """Authenticate the two fixed repository roots needed by ``__init__.py``."""

    canonical_norito = _canonical_directory(
        norito_root, "authenticated Norito source root"
    )
    canonical_torii = _canonical_directory(
        torii_root, "authenticated Torii client source root"
    )
    if (
        canonical_norito.name != "src"
        or canonical_norito.parent.name != "norito_py"
        or canonical_torii.name != "iroha_torii_client"
        or canonical_norito.parent.parent != canonical_torii.parent
    ):
        _fail("authenticated dependency roots do not have the expected repository layout")
    if canonical_norito == canonical_torii:
        _fail("authenticated dependency roots must be distinct")
    for root in (canonical_norito, canonical_torii):
        if root.is_relative_to(environment_root) or environment_root.is_relative_to(
            root
        ):
            _fail("authenticated dependency roots must remain outside the private venv")
        try:
            entries = tuple(root.iterdir())
        except OSError as error:
            _fail(f"unable to inspect authenticated dependency root: {error}")
        if len(entries) > MAX_ARCHIVE_MEMBERS:
            _fail("authenticated dependency root exceeds the entry-count bound")
        for entry in entries:
            normalized = unicodedata.normalize("NFC", entry.name).casefold()
            if normalized in {"iroha_python", "iroha_python.py"}:
                _fail(
                    "authenticated dependency root contains an iroha_python shadow"
                )
            if _is_matching_distribution_entry(entry.name):
                _fail(
                    "authenticated dependency root contains an iroha-python "
                    "distribution shadow"
                )

    authenticated: list[AuthenticatedDependencyRoot] = []
    for root, module_name, initializer in (
        (canonical_norito, "norito", canonical_norito / "norito/__init__.py"),
        (
            canonical_torii,
            "iroha_torii_client",
            canonical_torii / "__init__.py",
        ),
    ):
        _, initializer_seal = _read_stable_regular_file(
            initializer,
            label=f"authenticated {module_name} initializer",
            max_bytes=MAX_MEMBER_BYTES,
        )
        authenticated.append(
            AuthenticatedDependencyRoot(
                path=root,
                module_name=module_name,
                initializer_path=initializer,
                initializer_seal=initializer_seal,
                tree_state=_capture_dependency_tree(
                    root,
                    module_name,
                ),
            )
        )
    return authenticated[0], authenticated[1]


def _trusted_dependency_spec(
    dependency: AuthenticatedDependencyRoot,
) -> importlib.machinery.ModuleSpec:
    loader = importlib.machinery.SourceFileLoader(
        dependency.module_name,
        str(dependency.initializer_path),
    )
    spec = importlib.util.spec_from_file_location(
        dependency.module_name,
        dependency.initializer_path,
        loader=loader,
        submodule_search_locations=[str(dependency.initializer_path.parent)],
    )
    if (
        spec is None
        or type(spec.loader) is not importlib.machinery.SourceFileLoader
        or spec.loader_state is not None
        or spec.submodule_search_locations is None
        or tuple(spec.submodule_search_locations)
        != (str(dependency.initializer_path.parent),)
        or _spec_origin(spec, dependency.module_name)
        != dependency.initializer_path
    ):
        _fail(
            f"authenticated dependency {dependency.module_name} "
            "does not have one fixed source-package spec"
        )
    if (
        spec.loader.name != dependency.module_name
        or Path(spec.loader.path) != dependency.initializer_path
    ):
        _fail(
            f"authenticated dependency {dependency.module_name} "
            "loader does not match its fixed origin"
        )
    return spec


def trusted_import_specs(
    layout: InstalledLayout,
) -> tuple[importlib.machinery.ModuleSpec, importlib.machinery.ModuleSpec]:
    """Resolve specs with fixed file finders and authenticate their origins."""

    package_finder = importlib.machinery.FileFinder(
        str(layout.site_root),
        (
            importlib.machinery.SourceFileLoader,
            importlib.machinery.SOURCE_SUFFIXES,
        ),
    )
    package_spec = package_finder.find_spec(PACKAGE_NAME)
    if package_spec is None:
        _fail("trusted FileFinder could not resolve installed iroha_python")
    if type(package_spec.loader) is not importlib.machinery.SourceFileLoader:
        _fail("iroha_python must use SourceFileLoader")
    if package_spec.loader_state is not None:
        _fail("iroha_python import spec has unexpected loader_state")
    if _spec_origin(package_spec, PACKAGE_NAME) != layout.package_path:
        _fail("iroha_python import spec resolved outside the validated wheel path")
    package_locations = package_spec.submodule_search_locations
    if package_locations is None or tuple(package_locations) != (
        str(layout.package_root),
    ):
        _fail("iroha_python import spec has an unexpected package search path")
    if (
        package_spec.loader.name != PACKAGE_NAME
        or Path(package_spec.loader.path) != layout.package_path
    ):
        _fail("iroha_python source loader does not match the trusted origin")

    native_finder = importlib.machinery.FileFinder(
        str(layout.package_root),
        (
            importlib.machinery.ExtensionFileLoader,
            importlib.machinery.EXTENSION_SUFFIXES,
        ),
    )
    native_spec = native_finder.find_spec(NATIVE_MODULE_NAME)
    if native_spec is None:
        _fail("trusted FileFinder could not resolve installed iroha_python._crypto")
    if type(native_spec.loader) is not importlib.machinery.ExtensionFileLoader:
        _fail("iroha_python._crypto must use ExtensionFileLoader")
    if native_spec.loader_state is not None:
        _fail("iroha_python._crypto import spec has unexpected loader_state")
    if _spec_origin(native_spec, NATIVE_MODULE_NAME) != layout.native_path:
        _fail(
            "iroha_python._crypto import spec resolved outside the validated wheel path"
        )
    if native_spec.submodule_search_locations is not None:
        _fail("iroha_python._crypto import spec must not describe a package")
    if (
        native_spec.loader.name != NATIVE_MODULE_NAME
        or Path(native_spec.loader.path) != layout.native_path
    ):
        _fail("iroha_python._crypto loader does not match the trusted origin")
    return package_spec, native_spec


def _assert_unique_distribution_origin(
    wheel: WheelPreflight,
    layout: InstalledLayout,
) -> None:
    """Bind importlib.metadata lookups to the authenticated dist-info tree."""

    try:
        distributions = tuple(
            importlib.metadata.distributions(name=PACKAGE_DISTRIBUTION_NAME)
        )
    except (OSError, ValueError) as error:
        _fail(f"unable to resolve authenticated distribution metadata: {error}")
    if len(distributions) != 1:
        _fail(
            "package import must resolve exactly one iroha-python distribution"
        )
    distribution = distributions[0]
    if type(distribution) is not importlib.metadata.PathDistribution:
        _fail("iroha-python distribution must use the standard path loader")
    distribution_path = getattr(distribution, "_path", None)
    if not isinstance(distribution_path, Path):
        _fail("iroha-python distribution has no concrete metadata path")
    try:
        canonical_path = distribution_path.resolve(strict=True)
    except OSError as error:
        _fail(f"iroha-python distribution origin is unavailable: {error}")
    if canonical_path != layout.dist_info_root:
        _fail("iroha-python distribution resolved outside authenticated dist-info")
    try:
        names = distribution.metadata.get_all("Name")
        version = distribution.version
    except (OSError, UnicodeError, ValueError) as error:
        _fail(f"unable to read authenticated distribution metadata: {error}")
    if (
        names is None
        or len(names) != 1
        or re.sub(r"[-_.]+", "-", names[0]).casefold()
        != PACKAGE_DISTRIBUTION_NAME
        or version != wheel.metadata_version
    ):
        _fail("iroha-python distribution identity does not match the fresh wheel")


def _assert_loaded_module(
    *,
    module: object,
    spec: importlib.machinery.ModuleSpec,
    expected_name: str,
    expected_path: Path,
) -> None:
    if sys.modules.get(expected_name) is not module:
        _fail(f"{expected_name} replaced its authenticated sys.modules entry")
    if getattr(module, "__spec__", None) is not spec:
        _fail(f"{expected_name} replaced its authenticated import spec")
    if getattr(module, "__loader__", None) is not spec.loader:
        _fail(f"{expected_name} replaced its authenticated loader")
    expected_package = (
        expected_name
        if spec.submodule_search_locations is not None
        else expected_name.rpartition(".")[0]
    )
    if getattr(module, "__package__", None) != expected_package:
        _fail(f"{expected_name} has an invalid package identity")
    if getattr(module, "__name__", None) != expected_name:
        _fail(f"{expected_name} has an invalid module identity")
    module_file = getattr(module, "__file__", None)
    if not isinstance(module_file, str) or Path(module_file) != expected_path:
        _fail(f"{expected_name} replaced its authenticated filesystem origin")
    if spec.loader_state is not None:
        _fail(f"{expected_name} import spec changed loader_state")
    if _spec_origin(spec, expected_name) != expected_path:
        _fail(f"{expected_name} import spec origin changed during import")


def _authenticated_source_directories(
    wheel: WheelPreflight,
    layout: InstalledLayout,
    dependencies: Sequence[AuthenticatedDependencyRoot],
) -> tuple[Path, ...]:
    """Enumerate roots that must use source-only path importers."""

    _package_files, package_directories = _tree_expectations(
        root_name=PACKAGE_NAME,
        members=wheel.package_members,
        explicit_directories=wheel.package_directories,
    )
    directories: set[Path] = {
        layout.package_root,
        *(
            layout.package_root.joinpath(*PurePosixPath(relative).parts)
            for relative in package_directories
        ),
    }
    for dependency in dependencies:
        pending = [dependency.path]
        while pending:
            directory = pending.pop()
            directories.add(directory)
            try:
                entries = tuple(directory.iterdir())
            except OSError as error:
                _fail(
                    "unable to enumerate authenticated source directories: "
                    f"{error}"
                )
            for entry in entries:
                try:
                    metadata = entry.lstat()
                except OSError as error:
                    _fail(
                        "unable to stat authenticated source directory entry: "
                        f"{error}"
                    )
                if stat.S_ISDIR(metadata.st_mode) and not entry.is_symlink():
                    pending.append(entry)
            if len(directories) > MAX_ARCHIVE_MEMBERS:
                _fail("authenticated source directories exceed the path bound")
    return tuple(sorted(directories, key=os.fspath))


def load_from_trusted_specs(
    *,
    wheel: WheelPreflight,
    layout: InstalledLayout,
    package_spec: importlib.machinery.ModuleSpec,
    native_spec: importlib.machinery.ModuleSpec,
    dependencies: Sequence[AuthenticatedDependencyRoot],
) -> tuple[object, object]:
    """Load fixed dependencies, extension, and package without meta-path hooks."""

    reject_preseeded_modules()
    if len(dependencies) != 2 or tuple(
        dependency.module_name for dependency in dependencies
    ) != ("norito", "iroha_torii_client"):
        _fail("package initializer requires exactly two authenticated dependencies")
    dependency_specs = tuple(
        _trusted_dependency_spec(dependency) for dependency in dependencies
    )
    dependency_names = tuple(
        name
        for dependency in dependencies
        for name in tuple(sys.modules)
        if name == dependency.module_name
        or name.startswith(f"{dependency.module_name}.")
    )
    if dependency_names:
        _fail(
            "privacy wheel verification rejects preseeded dependency modules: "
            + ", ".join(sorted(dependency_names))
        )
    package_loader = package_spec.loader
    native_loader = native_spec.loader
    if not isinstance(package_loader, importlib.machinery.SourceFileLoader):
        _fail("trusted package spec lost SourceFileLoader")
    if not isinstance(native_loader, importlib.machinery.ExtensionFileLoader):
        _fail("trusted native spec lost ExtensionFileLoader")

    original_path = tuple(sys.path)
    original_meta_path = tuple(sys.meta_path)
    original_path_hooks = tuple(sys.path_hooks)
    original_importer_cache = dict(sys.path_importer_cache)
    original_dont_write_bytecode = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        authenticated_paths = [
            *(str(dependency.path) for dependency in dependencies),
            str(layout.site_root),
        ]
        sys.path[:] = authenticated_paths + [
            path for path in original_path if path not in authenticated_paths
        ]
        sys.meta_path[:] = [
            importlib.machinery.BuiltinImporter,
            importlib.machinery.FrozenImporter,
            importlib.machinery.PathFinder,
        ]
        source_loader_details = (
            (
                importlib.machinery.SourceFileLoader,
                importlib.machinery.SOURCE_SUFFIXES,
            ),
        )
        general_loader_details = (
            (
                importlib.machinery.ExtensionFileLoader,
                importlib.machinery.EXTENSION_SUFFIXES,
            ),
            (
                importlib.machinery.SourceFileLoader,
                importlib.machinery.SOURCE_SUFFIXES,
            ),
            (
                importlib.machinery.SourcelessFileLoader,
                importlib.machinery.BYTECODE_SUFFIXES,
            ),
        )
        sys.path_hooks[:] = [
            zipimport.zipimporter,
            importlib.machinery.FileFinder.path_hook(*general_loader_details),
        ]
        sys.path_importer_cache.clear()
        sys.path_importer_cache[str(layout.site_root)] = (
            importlib.machinery.FileFinder(
                str(layout.site_root),
                *general_loader_details,
            )
        )
        for source_directory in _authenticated_source_directories(
            wheel,
            layout,
            dependencies,
        ):
            sys.path_importer_cache[str(source_directory)] = (
                importlib.machinery.FileFinder(
                    str(source_directory),
                    *source_loader_details,
                )
            )
        _assert_unique_distribution_origin(wheel, layout)
        for dependency, dependency_spec in zip(
            dependencies, dependency_specs, strict=True
        ):
            dependency_loader = dependency_spec.loader
            if not isinstance(
                dependency_loader, importlib.machinery.SourceFileLoader
            ):
                _fail("authenticated dependency spec lost SourceFileLoader")
            module = importlib.util.module_from_spec(dependency_spec)
            sys.modules[dependency.module_name] = module
            dependency_loader.exec_module(module)
            _assert_loaded_module(
                module=module,
                spec=dependency_spec,
                expected_name=dependency.module_name,
                expected_path=dependency.initializer_path,
            )

        package = importlib.util.module_from_spec(package_spec)
        sys.modules[PACKAGE_NAME] = package

        native = importlib.util.module_from_spec(native_spec)
        sys.modules[NATIVE_MODULE_NAME] = native
        native_loader.exec_module(native)
        package_loader.exec_module(package)

        _assert_loaded_module(
            module=package,
            spec=package_spec,
            expected_name=PACKAGE_NAME,
            expected_path=layout.package_path,
        )
        _assert_loaded_module(
            module=native,
            spec=native_spec,
            expected_name=NATIVE_MODULE_NAME,
            expected_path=layout.native_path,
        )
        if getattr(package, "__version__", None) != wheel.metadata_version:
            _fail(
                "iroha_python observed a version outside authenticated METADATA"
            )
        _assert_unique_distribution_origin(wheel, layout)
        for dependency in dependencies:
            assert_expected_file_seal(
                dependency.initializer_path,
                dependency.initializer_seal,
                label=f"authenticated {dependency.module_name} initializer",
                max_bytes=MAX_MEMBER_BYTES,
            )
            if (
                _capture_dependency_tree(
                    dependency.path,
                    dependency.module_name,
                )
                != dependency.tree_state
            ):
                _fail(
                    f"authenticated {dependency.module_name} source tree "
                    "changed during package import"
                )
        return package, native
    except BaseException:
        for name in tuple(sys.modules):
            if (
                name == PACKAGE_NAME
                or name.startswith(f"{PACKAGE_NAME}.")
                or any(
                    name == dependency.module_name
                    or name.startswith(f"{dependency.module_name}.")
                    for dependency in dependencies
                )
            ):
                sys.modules.pop(name, None)
        raise
    finally:
        sys.dont_write_bytecode = original_dont_write_bytecode
        sys.path[:] = original_path
        sys.meta_path[:] = original_meta_path
        sys.path_hooks[:] = original_path_hooks
        sys.path_importer_cache.clear()
        sys.path_importer_cache.update(original_importer_cache)


def assert_no_python_runtime_dependency(dependencies: str) -> None:
    """Reject explicit CPython runtime linkage in a Darwin extension."""

    lowered = dependencies.lower()
    if "python.framework" in lowered or "libpython" in lowered:
        _fail("Darwin native wheel must not depend on Python.framework or libpython")


def _darwin_dependency_output(native_path: Path) -> str:
    otool = Path("/usr/bin/otool")
    if not otool.is_file() or not os.access(otool, os.X_OK):
        _fail("Darwin installed-wheel verification requires otool")
    try:
        return subprocess.run(
            [str(otool), "-L", str(native_path)],
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout
    except (OSError, subprocess.SubprocessError) as error:
        _fail(f"otool failed while inspecting the installed native module: {error}")


def verify_current_environment(
    environment_root: Path,
    wheel_path: Path,
    expected_wheel_seal: str | FileSeal,
    norito_root: Path,
    torii_root: Path,
    *,
    site_roots: Iterable[Path] | None = None,
    platform_name: str | None = None,
    dependency_output: str | None = None,
    extension_suffixes: Sequence[str] | None = None,
) -> Path:
    """Preflight the wheel, load its extension, and verify post-load state."""

    environment_root = _canonical_directory(environment_root, "private venv")
    if Path(sys.prefix).resolve(strict=True) != environment_root:
        _fail("installed-wheel verifier is not running in the private venv")
    dependencies = authenticate_dependency_roots(
        environment_root=environment_root,
        norito_root=norito_root,
        torii_root=torii_root,
    )

    # This rejection and every wheel check intentionally precede package import.
    reject_preseeded_modules()
    wheel = preflight_wheel(
        wheel_path,
        expected_wheel_seal,
        extension_suffixes=extension_suffixes,
    )
    discovered_site_roots = (
        {
            Path(path)
            for path in (
                *site.getsitepackages(),
                sysconfig.get_paths()["purelib"],
                sysconfig.get_paths()["platlib"],
            )
        }
        if site_roots is None
        else set(site_roots)
    )
    layout = derive_installed_layout(
        environment_root=environment_root,
        site_roots=discovered_site_roots,
        wheel=wheel,
    )
    installed_before = verify_installed_files(wheel, layout)
    package_spec, native_spec = trusted_import_specs(layout)
    load_from_trusted_specs(
        wheel=wheel,
        layout=layout,
        package_spec=package_spec,
        native_spec=native_spec,
        dependencies=dependencies,
    )
    installed_after = verify_installed_files(wheel, layout)
    if installed_after.files != installed_before.files:
        _fail("iroha_python package or dist-info changed while it was imported")

    selected_platform = sys.platform if platform_name is None else platform_name
    if selected_platform == "darwin":
        output = (
            _darwin_dependency_output(layout.native_path)
            if dependency_output is None
            else dependency_output
        )
        assert_no_python_runtime_dependency(output)
        assert_expected_file_seal(
            layout.native_path,
            installed_after.native.seal,
            label=NATIVE_MODULE_NAME,
            max_bytes=MAX_MEMBER_BYTES,
        )

    # Re-authenticate the path after all archive-derived and loader operations.
    assert_expected_file_seal(
        wheel.path,
        wheel.seal,
        label="fresh private wheel",
        max_bytes=MAX_WHEEL_BYTES,
    )
    return layout.native_path


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "--seal":
        try:
            seal = seal_wheel(Path(sys.argv[2]))
        except (OSError, VerificationError) as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        print(seal.render())
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == "--preflight":
        try:
            wheel = preflight_wheel(Path(sys.argv[2]), sys.argv[3])
        except (OSError, VerificationError) as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        print(wheel.path)
        return 0
    if len(sys.argv) != 6:
        raise SystemExit(
            "usage: verify_privacy_python_wheel.py "
            "[--seal|--preflight] PRIVATE_VENV_OR_WHEEL PRIVATE_WHEEL_OR_SEAL "
            "EXPECTED_WHEEL_SEAL [NORITO_ROOT TORII_ROOT]"
        )
    try:
        native_path = verify_current_environment(
            Path(sys.argv[1]),
            Path(sys.argv[2]),
            sys.argv[3],
            Path(sys.argv[4]),
            Path(sys.argv[5]),
        )
    except (OSError, VerificationError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(native_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
