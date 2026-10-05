#!/usr/bin/env python3
"""Authenticate a genuine NoritoBridge ZIP and optionally install it without replacement."""

from __future__ import annotations

import argparse
import ctypes
import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile
from typing import NoReturn
import zipfile


ARCHIVE_ROOT = "NoritoBridge.xcframework"
EMBEDDED_MANIFEST = f"{ARCHIVE_ROOT}/NoritoBridge.artifacts.json"
SEMVER = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\Z")
CHUNK_SIZE = 1024 * 1024
MAX_ARCHIVE_BYTES = 1024 * 1024 * 1024
MAX_ARCHIVE_ENTRIES = 4096
MAX_ENTRY_BYTES = 512 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_BYTES = 1024 * 1024 * 1024
MAX_MANIFEST_BYTES = 64 * 1024


class ArchiveValidationError(RuntimeError):
    """The archive or installation destination violates the native release contract."""


def fail(message: str) -> NoReturn:
    raise ArchiveValidationError(message)


def canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        fail(f"{label} must be an absolute canonical directory")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"unable to inspect {label}: {error}")
    if resolved != path or stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        fail(f"{label} must be a non-symbolic canonical directory")
    return path


def canonical_regular_file(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        fail(f"{label} must be an absolute canonical file")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"unable to inspect {label}: {error}")
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
    ):
        fail(f"{label} must be a non-symbolic canonical single-link regular file")
    return path


def read_regular(path: Path, label: str, *, max_bytes: int) -> bytes:
    path = canonical_regular_file(path, label)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"unable to open {label}: {error}")
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.geteuid()
            or stat.S_IMODE(before.st_mode) & 0o022
        ):
            fail(
                f"{label} must remain a current-UID-owned, non-writable-by-others, "
                "single-link regular file"
            )
        if before.st_size > max_bytes:
            fail(f"{label} exceeds the {max_bytes}-byte limit")
        chunks: list[bytes] = []
        total_bytes = 0
        while chunk := os.read(descriptor, CHUNK_SIZE):
            total_bytes += len(chunk)
            if total_bytes > max_bytes:
                fail(f"{label} exceeds the {max_bytes}-byte limit")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        visible = path.lstat()
    finally:
        os.close(descriptor)
    identity = (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_uid,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    payload = b"".join(chunks)
    if identity != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_uid,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ) or identity != (
        visible.st_dev,
        visible.st_ino,
        visible.st_mode,
        visible.st_nlink,
        visible.st_uid,
        visible.st_size,
        visible.st_mtime_ns,
        visible.st_ctime_ns,
    ) or len(payload) != before.st_size:
        fail(f"{label} changed while it was being read")
    return payload


def validate_archive(path: Path, expected_version: str) -> bytes:
    payload = read_regular(
        path,
        "NoritoBridge archive",
        max_bytes=MAX_ARCHIVE_BYTES,
    )
    try:
        with zipfile.ZipFile(io.BytesIO(payload)) as archive:
            entries = archive.infolist()
            if not entries:
                fail("NoritoBridge archive is empty")
            if len(entries) > MAX_ARCHIVE_ENTRIES:
                fail(
                    "NoritoBridge archive exceeds the "
                    f"{MAX_ARCHIVE_ENTRIES}-entry limit"
                )
            names: set[str] = set()
            casefolded_names: set[str] = set()
            total_uncompressed = 0
            for entry in entries:
                name = entry.filename
                components = name.rstrip("/").split("/")
                casefolded = name.rstrip("/").casefold()
                if (
                    not name
                    or "\x00" in name
                    or "\\" in name
                    or name.startswith("/")
                    or not components
                    or components[0] != ARCHIVE_ROOT
                    or any(part in {"", ".", ".."} for part in components)
                    or name in names
                    or casefolded in casefolded_names
                ):
                    fail(
                        "unsafe, duplicate, or case-colliding NoritoBridge "
                        f"archive entry: {name!r}"
                    )
                names.add(name)
                casefolded_names.add(casefolded)
                if entry.flag_bits & 0x1:
                    fail(f"encrypted NoritoBridge archive entries are forbidden: {name}")
                if entry.compress_type != zipfile.ZIP_STORED:
                    fail(f"NoritoBridge archive entry is not deterministically stored: {name}")
                unix_mode = entry.external_attr >> 16
                file_type = stat.S_IFMT(unix_mode)
                if file_type == stat.S_IFLNK:
                    fail(f"symbolic links are forbidden in NoritoBridge archives: {name}")
                if file_type not in {0, stat.S_IFREG, stat.S_IFDIR}:
                    fail(f"unsupported NoritoBridge archive entry type: {name}")
                if entry.file_size > MAX_ENTRY_BYTES:
                    fail(
                        f"NoritoBridge archive entry exceeds the {MAX_ENTRY_BYTES}-byte "
                        f"limit: {name}"
                    )
                total_uncompressed += entry.file_size
                if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_BYTES:
                    fail(
                        "NoritoBridge archive exceeds the "
                        f"{MAX_TOTAL_UNCOMPRESSED_BYTES}-byte uncompressed limit"
                    )
            if EMBEDDED_MANIFEST not in names:
                fail(f"NoritoBridge archive is missing {EMBEDDED_MANIFEST}")
            manifest = archive.getinfo(EMBEDDED_MANIFEST)
            if not 2 <= manifest.file_size <= MAX_MANIFEST_BYTES:
                fail("embedded NoritoBridge manifest must contain 2..65536 bytes")
            bad_crc = archive.testzip()
            if bad_crc is not None:
                fail(f"NoritoBridge archive has a corrupt entry: {bad_crc}")
            try:
                manifest_document = json.loads(archive.read(manifest))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                fail(f"embedded NoritoBridge manifest is not JSON: {error}")
            if (
                not isinstance(manifest_document, dict)
                or manifest_document.get("version") != expected_version
            ):
                fail(
                    "embedded NoritoBridge manifest version must equal "
                    f"IrohaSwift/VERSION ({expected_version})"
                )
    except (
        OSError,
        RuntimeError,
        NotImplementedError,
        zipfile.BadZipFile,
        zipfile.LargeZipFile,
    ) as error:
        fail(f"unable to authenticate NoritoBridge archive: {error}")
    return payload


def parse_version(root: Path) -> str:
    raw = read_regular(
        root / "IrohaSwift/VERSION",
        "IrohaSwift VERSION",
        max_bytes=64,
    )
    try:
        version = raw.decode("ascii").strip()
    except UnicodeDecodeError as error:
        fail(f"IrohaSwift VERSION must be ASCII: {error}")
    if raw != f"{version}\n".encode("ascii") or SEMVER.fullmatch(version) is None:
        fail("IrohaSwift VERSION must contain one canonical SemVer and a newline")
    return version


def _stage_identity(path: Path) -> tuple[int, int, int, int]:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or path.resolve(strict=True) != path
    ):
        fail("archive stage must remain a canonical, owned mode-0700 directory")
    return metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_uid


def _assert_stage_inventory(stage: Path) -> None:
    if {entry.name for entry in stage.iterdir()} != {ARCHIVE_ROOT, "NoritoBridge.artifacts.json"}:
        fail("archive staging inventory changed during authentication")


def _extract_archive(payload: bytes, stage: Path) -> None:
    """Extract already-bounded immutable bytes using exclusive regular files."""
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        for entry in archive.infolist():
            destination = stage.joinpath(*entry.filename.rstrip("/").split("/"))
            file_type = stat.S_IFMT(entry.external_attr >> 16)
            if entry.is_dir() or file_type == stat.S_IFDIR:
                destination.mkdir(mode=0o755, parents=True, exist_ok=True)
                continue
            destination.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
            flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
            descriptor = os.open(destination, flags, 0o600)
            with os.fdopen(descriptor, "wb") as output, archive.open(entry) as source:
                copied = 0
                while chunk := source.read(CHUNK_SIZE):
                    copied += len(chunk)
                    if copied > entry.file_size:
                        fail("archive member changed its declared size during extraction")
                    output.write(chunk)
                if copied != entry.file_size:
                    fail("archive member has incomplete bytes")
                output.flush()
                os.fchmod(output.fileno(), 0o644)
                os.fsync(output.fileno())
    os.symlink(
        EMBEDDED_MANIFEST,
        stage / "NoritoBridge.artifacts.json",
    )


def _load_native_owner(root: Path, name: str):
    path = canonical_regular_file(root / "scripts" / name, "native artifact owner")
    specification = importlib.util.spec_from_file_location("norito_archive_" + path.stem, path)
    if specification is None or specification.loader is None:
        fail("unable to load the repository native artifact validator")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _validate_native_contents(root: Path, lockfile: Path, artifact_directory: Path) -> dict[str, object]:
    """Require the real source/tool/ABI/slice/export/pin owner before admission."""
    validator = _load_native_owner(root, "validate_norito_bridge_xcframework.py")
    framework = artifact_directory / ARCHIVE_ROOT
    try:
        manifest = validator.validate(
            root=root,
            lockfile_path=lockfile,
            xcframework=framework,
            manifest_path=framework / "NoritoBridge.artifacts.json",
            manifest_link=artifact_directory / "NoritoBridge.artifacts.json",
            expected_link_target=EMBEDDED_MANIFEST,
            swift_loader=root / "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
            verify_repository_provenance=True,
        )
        archive_owner = _load_native_owner(root, "archive_norito_xcframework.py")
        archive_owner._validate_native_binaries(framework, validator)
        return manifest
    except (OSError, RuntimeError, ValueError) as error:
        fail(f"archived native artifact authentication failed: {error}")


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _rename_no_replace(source: Path, destination: Path) -> None:
    """Publish an existing directory or symlink without replacing any caller path."""
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin":
        function = getattr(library, "renameatx_np", None)
        at_fdcwd, flag = -2, 0x4
    elif sys.platform.startswith("linux"):
        function = getattr(library, "renameat2", None)
        at_fdcwd, flag = -100, 0x1
    else:
        function = None
        at_fdcwd, flag = 0, 0
    if function is None:
        fail("host does not provide atomic no-replace artifact installation")
    function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    function.restype = ctypes.c_int
    if function(at_fdcwd, os.fsencode(source), at_fdcwd, os.fsencode(destination), flag) != 0:
        error_number = ctypes.get_errno()
        raise OSError(error_number, os.strerror(error_number), os.fspath(destination))


def _installation_directory(root: Path, destination: Path) -> Path:
    destination = canonical_directory(destination, "artifact installation directory")
    metadata = destination.lstat()
    if (
        metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not os.access(destination, os.W_OK | os.X_OK)
    ):
        fail("artifact installation directory must be owned, writable and not writable by others")
    if root == destination or root in destination.parents:
        if destination != root / "dist":
            fail("checkout-local installation is restricted to the default dist directory")
    for name in (ARCHIVE_ROOT, "NoritoBridge.artifacts.json"):
        if os.path.lexists(destination / name):
            fail(f"artifact installation refuses an existing path: {destination / name}")
    return destination


def authenticate_archive(
    root: Path,
    archive: Path,
    lockfile: Path,
    *,
    expected_sha256: str | None = None,
    install_directory: Path | None = None,
    scratch_directory: Path | None = None,
) -> dict[str, object]:
    """Authenticate original ZIP bytes and optionally install the exact generation."""
    root = canonical_directory(root, "repository root")
    payload = validate_archive(archive, parse_version(root))
    digest = hashlib.sha256(payload).hexdigest()
    if expected_sha256 is not None:
        if re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None or digest != expected_sha256:
            fail("NoritoBridge archive does not match its expected SHA-256")
    destination = (
        _installation_directory(root, install_directory)
        if install_directory is not None else None
    )
    parent = destination if destination is not None else scratch_directory
    if parent is None:
        parent = Path(tempfile.gettempdir()).resolve(strict=True)
    parent = canonical_directory(parent, "archive staging parent")
    stage = Path(tempfile.mkdtemp(prefix=".NoritoBridge.archive-check.", dir=parent))
    identity = _stage_identity(stage)
    try:
        _extract_archive(payload, stage)
        manifest = _validate_native_contents(root, lockfile, stage)
        if _stage_identity(stage) != identity:
            fail("archive staging directory changed during authentication")
        _assert_stage_inventory(stage)
        if destination is not None:
            # Both publications are exclusive. If a competing path appears,
            # leave the authenticated generation/stage for inspection and never
            # delete or overwrite that caller-owned path.
            _installation_directory(root, destination)
            _rename_no_replace(stage / ARCHIVE_ROOT, destination / ARCHIVE_ROOT)
            _rename_no_replace(stage / "NoritoBridge.artifacts.json", destination / "NoritoBridge.artifacts.json")
            _fsync_directory(destination)
            manifest = _validate_native_contents(root, lockfile, destination)
            if _stage_identity(stage) != identity:
                fail("archive staging directory changed during installation")
            stage.rmdir()
        else:
            # This private directory contains only the bounded authenticated
            # archive tree and its generated link. The fd-based implementation
            # does not follow symlinks during removal.
            shutil.rmtree(stage)
    except BaseException as error:
        raise ArchiveValidationError(f"{error}; retained archive stage: {stage}") from error
    return {"archive_sha256": digest, "version": manifest["version"], "installed": str(destination) if destination else None}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--lockfile-path", type=Path, required=True)
    parser.add_argument("--expected-sha256")
    parser.add_argument("--scratch-dir", type=Path)
    parser.add_argument("--install-dir", type=Path)
    arguments = parser.parse_args()
    result = authenticate_archive(
        arguments.root, arguments.archive, arguments.lockfile_path,
        expected_sha256=arguments.expected_sha256,
        install_directory=arguments.install_dir,
        scratch_directory=arguments.scratch_dir,
    )
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveValidationError, OSError) as error:
        print(f"validate_norito_bridge_archive.py: error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
