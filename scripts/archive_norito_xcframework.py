#!/usr/bin/env python3
"""Publish one authenticated, reproducible NoritoBridge XCFramework archive."""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import errno
import fcntl
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from typing import Iterator, NoReturn
import zipfile


ARCHIVE_ROOT = "NoritoBridge.xcframework"
MANIFEST_NAME = "NoritoBridge.artifacts.json"
PUBLISH_LOCK_NAME = ".NoritoBridge.publish.lockfile"
INHERITED_OUTPUT_LOCK_ENV = "NORITO_BRIDGE_OUTPUT_LOCK_FD"
CHUNK_SIZE = 1024 * 1024
ZIP_MIN_EPOCH = 315_532_800  # 1980-01-01T00:00:00Z
ZIP_MAX_EPOCH = 4_354_819_199  # 2107-12-31T23:59:59Z


class ArchiveError(RuntimeError):
    """The XCFramework cannot be authenticated or archived safely."""


class ArchiveInterrupted(ArchiveError):
    """The archive operation received an interrupting signal."""


@dataclass(frozen=True)
class SourceEntry:
    relative: str
    is_directory: bool
    device: int
    inode: int
    mode: int
    size: int
    mtime_ns: int


@dataclass
class TemporaryArchive:
    path: Path
    descriptor: int
    device: int
    inode: int
    mode: int
    owner: int
    size: int
    mtime_ns: int
    ctime_ns: int
    sha256: str

    def _descriptor_digest(self) -> tuple[os.stat_result, str, os.stat_result]:
        before = os.fstat(self.descriptor)
        os.lseek(self.descriptor, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        while True:
            chunk = os.read(self.descriptor, CHUNK_SIZE)
            if not chunk:
                break
            digest.update(chunk)
        return before, digest.hexdigest(), os.fstat(self.descriptor)

    def assert_at_path(self, visible_path: Path, *, allow_rename_ctime: bool) -> None:
        directory_fd = _open_directory(visible_path.parent)
        try:
            try:
                visible = os.stat(
                    visible_path.name,
                    dir_fd=directory_fd,
                    follow_symlinks=False,
                )
            except OSError as error:
                fail(f"archive temporary path changed: {visible_path}: {error}")
            before, digest, after = self._descriptor_digest()
            expected_identity = (self.device, self.inode)
            if (
                not stat.S_ISREG(before.st_mode)
                or before.st_nlink != 1
                or before.st_uid != os.geteuid()
                or (before.st_dev, before.st_ino) != expected_identity
                or (after.st_dev, after.st_ino) != expected_identity
                or (visible.st_dev, visible.st_ino) != expected_identity
                or before.st_mode != self.mode
                or after.st_mode != self.mode
                or visible.st_mode != self.mode
                or before.st_uid != self.owner
                or after.st_uid != self.owner
                or visible.st_uid != self.owner
                or before.st_size != self.size
                or after.st_size != self.size
                or visible.st_size != self.size
                or before.st_mtime_ns != self.mtime_ns
                or after.st_mtime_ns != self.mtime_ns
                or visible.st_mtime_ns != self.mtime_ns
                or digest != self.sha256
                or (
                    not allow_rename_ctime
                    and (
                        before.st_ctime_ns != self.ctime_ns
                        or after.st_ctime_ns != self.ctime_ns
                        or visible.st_ctime_ns != self.ctime_ns
                    )
                )
            ):
                fail(f"archive temporary path no longer names its authenticated inode: {visible_path}")
        finally:
            os.close(directory_fd)

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


@dataclass(frozen=True)
class PublishLock:
    path: Path
    descriptor: int

    def assert_authenticated(self) -> None:
        try:
            descriptor_metadata = os.fstat(self.descriptor)
            path_metadata = self.path.lstat()
        except OSError as error:
            fail(f"publish lock became unavailable: {self.path}: {error}")
        if (
            not stat.S_ISREG(descriptor_metadata.st_mode)
            or descriptor_metadata.st_nlink != 1
            or descriptor_metadata.st_uid != os.geteuid()
            or not stat.S_ISREG(path_metadata.st_mode)
            or path_metadata.st_nlink != 1
            or path_metadata.st_uid != os.geteuid()
            or (descriptor_metadata.st_dev, descriptor_metadata.st_ino)
            != (path_metadata.st_dev, path_metadata.st_ino)
        ):
            fail(f"publish lock is not authenticated: {self.path}")

    def assert_held(self) -> None:
        self.assert_authenticated()
        try:
            fcntl.flock(self.descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            fail(f"publish lock is not held: {self.path}: {error}")


def fail(message: str) -> NoReturn:
    raise ArchiveError(message)


def _canonical_existing_directory(raw: str, label: str) -> Path:
    candidate = Path(raw)
    if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
        fail(f"{label} must be an absolute canonical directory")
    try:
        metadata = candidate.lstat()
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        fail(f"unable to inspect {label} {candidate}: {error}")
    if (
        resolved != candidate
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
    ):
        fail(f"{label} must be a non-symbolic canonical directory")
    return candidate


def _repository_root() -> Path:
    return Path(__file__).resolve(strict=True).parent.parent


def _is_within(path: Path, parent: Path) -> bool:
    return path == parent or parent in path.parents


def _assert_destination_absent(output: Path) -> None:
    directory_fd = _open_directory(output.parent)
    try:
        try:
            os.stat(output.name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            return
        fail(f"archive output must not already exist: {output}")
    finally:
        os.close(directory_fd)


def _canonical_output(
    raw: str,
    source: Path,
    repository_root: Path,
) -> Path:
    output = Path(raw)
    if not output.is_absolute() or output != Path(os.path.abspath(output)):
        fail("archive output must be an absolute canonical filename")
    if output.suffix != ".zip" or output.name in {"", ".", ".."}:
        fail("archive output must be a .zip filename")
    parent = _canonical_existing_directory(str(output.parent), "archive output parent")
    output = parent / output.name
    if source == output or source in output.parents:
        fail("archive output must be outside NoritoBridge.xcframework")
    if _is_within(parent, repository_root):
        fail("archive output parent must be outside the Iroha source tree")
    parent_metadata = parent.lstat()
    if parent_metadata.st_uid != os.geteuid() or not os.access(
        parent,
        os.W_OK | os.X_OK,
    ):
        fail("archive output parent must be an owned, writable directory")
    _assert_destination_absent(output)
    return output


def _source_date_epoch() -> tuple[int, int, tuple[int, int, int, int, int, int]]:
    raw = os.environ.get("SOURCE_DATE_EPOCH")
    if raw is None or not re.fullmatch(r"[0-9]+", raw):
        fail("SOURCE_DATE_EPOCH must be an explicit canonical unsigned integer")
    if len(raw) > 1 and raw.startswith("0"):
        fail("SOURCE_DATE_EPOCH must not contain leading zeroes")
    epoch = int(raw, 10)
    if not ZIP_MIN_EPOCH <= epoch <= ZIP_MAX_EPOCH:
        fail("SOURCE_DATE_EPOCH is outside the ZIP timestamp range")
    normalized = epoch - (epoch % 2)
    stamp = time.gmtime(normalized)
    return epoch, normalized, (
        stamp.tm_year,
        stamp.tm_mon,
        stamp.tm_mday,
        stamp.tm_hour,
        stamp.tm_min,
        stamp.tm_sec,
    )


def _open_directory(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    return os.open(path, flags)


def _acquire_lock(lock_path: Path) -> PublishLock:
    directory_fd = _open_directory(lock_path.parent)
    try:
        flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_NOFOLLOW", 0)
        lock_fd = os.open(lock_path.name, flags, 0o600, dir_fd=directory_fd)
    except BaseException:
        os.close(directory_fd)
        raise
    os.close(directory_fd)
    try:
        guard = PublishLock(lock_path, lock_fd)
        guard.assert_authenticated()
        try:
            fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            fail(f"another publisher holds {lock_path}")
        except OSError as error:
            fail(f"unable to acquire publish lock {lock_path}: {error}")
        guard.assert_held()
        return guard
    except BaseException:
        os.close(lock_fd)
        raise


def _inherited_output_lock(lock_path: Path) -> PublishLock | None:
    if INHERITED_OUTPUT_LOCK_ENV not in os.environ:
        return None
    raw_descriptor = os.environ[INHERITED_OUTPUT_LOCK_ENV]
    if not re.fullmatch(r"0|[1-9][0-9]*", raw_descriptor):
        fail(f"{INHERITED_OUTPUT_LOCK_ENV} must be one canonical decimal descriptor")
    guard = PublishLock(lock_path, int(raw_descriptor, 10))
    guard.assert_held()
    return guard


@contextlib.contextmanager
def _archive_lock(artifact_root: Path) -> Iterator[PublishLock]:
    source_lock_path = artifact_root / PUBLISH_LOCK_NAME
    inherited = _inherited_output_lock(source_lock_path)
    guard: PublishLock | None = None
    try:
        if inherited is not None:
            guard = inherited
        else:
            guard = _acquire_lock(source_lock_path)
        yield guard
    finally:
        if guard is not None and inherited is None:
            os.close(guard.descriptor)


def _entry_from_stat(relative: str, metadata: os.stat_result) -> SourceEntry:
    return SourceEntry(
        relative=relative,
        is_directory=stat.S_ISDIR(metadata.st_mode),
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=metadata.st_mode,
        size=metadata.st_size,
        mtime_ns=metadata.st_mtime_ns,
    )


def _enumerate_source(source: Path) -> tuple[SourceEntry, ...]:
    entries: list[SourceEntry] = []

    def visit(directory: Path, prefix: str) -> None:
        try:
            children = sorted(os.scandir(directory), key=lambda item: os.fsencode(item.name))
        except OSError as error:
            fail(f"unable to enumerate {directory}: {error}")
        for child in children:
            relative = f"{prefix}/{child.name}" if prefix else child.name
            if "\x00" in child.name or child.name in {"", ".", ".."}:
                fail(f"invalid XCFramework entry name: {relative!r}")
            try:
                metadata = child.stat(follow_symlinks=False)
            except OSError as error:
                fail(f"unable to inspect XCFramework entry {relative}: {error}")
            if stat.S_ISLNK(metadata.st_mode):
                fail(f"symbolic links are forbidden in XCFramework archives: {relative}")
            if stat.S_ISDIR(metadata.st_mode):
                entries.append(_entry_from_stat(relative, metadata))
                visit(Path(child.path), relative)
            elif stat.S_ISREG(metadata.st_mode):
                if metadata.st_nlink != 1:
                    fail(f"hard-linked XCFramework files are forbidden: {relative}")
                entries.append(_entry_from_stat(relative, metadata))
            else:
                fail(f"unsupported XCFramework entry type: {relative}")

    visit(source, "")
    if not entries:
        fail("NoritoBridge.xcframework is empty")
    return tuple(entries)


def _same_source_entry(expected: SourceEntry, actual: os.stat_result) -> bool:
    return (
        expected.device == actual.st_dev
        and expected.inode == actual.st_ino
        and expected.mode == actual.st_mode
        and expected.size == actual.st_size
        and expected.mtime_ns == actual.st_mtime_ns
    )


def _copy_file(source: Path, destination: Path, expected: SourceEntry) -> str:
    read_flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    source_fd = os.open(source, read_flags)
    try:
        before = os.fstat(source_fd)
        if not stat.S_ISREG(before.st_mode) or not _same_source_entry(expected, before):
            fail(f"XCFramework source changed before snapshot: {expected.relative}")
        write_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        write_flags |= getattr(os, "O_NOFOLLOW", 0)
        destination_fd = os.open(destination, write_flags, 0o600)
        hasher = hashlib.sha256()
        try:
            while True:
                chunk = os.read(source_fd, CHUNK_SIZE)
                if not chunk:
                    break
                hasher.update(chunk)
                view = memoryview(chunk)
                while view:
                    written = os.write(destination_fd, view)
                    view = view[written:]
            after = os.fstat(source_fd)
            if not _same_source_entry(expected, after):
                fail(f"XCFramework source changed during snapshot: {expected.relative}")
            os.fchmod(destination_fd, 0o644)
            os.fsync(destination_fd)
        finally:
            os.close(destination_fd)
    finally:
        os.close(source_fd)
    return hasher.hexdigest()


def _fsync_directory(path: Path) -> None:
    directory_fd = _open_directory(path)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


def _snapshot_source(
    source: Path,
    scratch_root: Path,
) -> tuple[Path, Path, dict[str, str]]:
    baseline = _enumerate_source(source)
    container = Path(
        tempfile.mkdtemp(prefix=".NoritoBridge.archive-snapshot.", dir=scratch_root)
    )
    try:
        snapshot = container / ARCHIVE_ROOT
        snapshot.mkdir(mode=0o755)
        digests: dict[str, str] = {}
        for entry in baseline:
            destination = snapshot / entry.relative
            if entry.is_directory:
                destination.mkdir(mode=0o755)
            else:
                digests[entry.relative] = _copy_file(
                    source / entry.relative,
                    destination,
                    entry,
                )
        if _enumerate_source(source) != baseline:
            fail("XCFramework source changed while its immutable snapshot was created")
        directories = [snapshot]
        directories.extend(
            snapshot / entry.relative for entry in baseline if entry.is_directory
        )
        for directory in reversed(directories):
            os.chmod(directory, 0o755, follow_symlinks=False)
            _fsync_directory(directory)
        _fsync_directory(container)
        return container, snapshot, digests
    except BaseException:
        # The uniquely named cache snapshot is intentionally retained. Deleting
        # a pathname after a failed identity check could remove foreign data
        # installed by an uncooperative same-UID process.
        raise


def _load_generation_validator():
    # The archive owner is output-only. Dynamic validation imports must never
    # materialize bytecode beside authenticated repository sources.
    sys.dont_write_bytecode = True
    validator_path = Path(__file__).resolve(strict=True).with_name(
        "validate_norito_bridge_xcframework.py"
    )
    metadata = validator_path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        fail(f"XCFramework validator is unavailable: {validator_path}")
    spec = importlib.util.spec_from_file_location(
        "norito_bridge_xcframework_archive_validator",
        validator_path,
    )
    if spec is None or spec.loader is None:
        fail(f"unable to load XCFramework validator: {validator_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _validate_generation(snapshot: Path):
    validator = _load_generation_validator()
    manifest_path = snapshot / MANIFEST_NAME
    manifest_link = snapshot.parent / MANIFEST_NAME
    expected_link_target = f"{ARCHIVE_ROOT}/{MANIFEST_NAME}"
    try:
        os.symlink(expected_link_target, manifest_link)
        validator.validate(
            root=Path(__file__).resolve(strict=True).parent.parent,
            xcframework=snapshot,
            manifest_path=manifest_path,
            manifest_link=manifest_link,
            expected_link_target=expected_link_target,
            swift_loader=None,
            verify_repository_provenance=True,
        )
    except validator.ValidationError as error:
        fail(f"XCFramework generation is not canonical: {error}")
    finally:
        manifest_link.unlink(missing_ok=True)
    return validator


def _pinned_native_tool(name: str) -> Path:
    tool = Path("/usr/bin") / name
    try:
        metadata = tool.lstat()
        resolved = tool.resolve(strict=True)
    except OSError as error:
        fail(f"required Apple archive tool is unavailable: {tool}: {error}")
    if (
        resolved != tool
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or not os.access(tool, os.X_OK)
    ):
        fail(f"required Apple archive tool is not authenticated: {tool}")
    return tool


def _run_native_tool(tool: Path, arguments: list[str]) -> str:
    developer_dir = os.environ.get("NORITO_BRIDGE_SEAL_DEVELOPER_DIR")
    if developer_dir is None:
        fail("NORITO_BRIDGE_SEAL_DEVELOPER_DIR is required for native inspection")
    try:
        result = subprocess.run(
            [str(tool), *arguments],
            executable=str(tool),
            env={
                "HOME": "/tmp",
                "DEVELOPER_DIR": developer_dir,
                "LANG": "C.UTF-8",
                "LC_ALL": "C.UTF-8",
                "PATH": "/usr/bin:/bin",
                "TMPDIR": "/tmp",
            },
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as error:
        fail(f"unable to execute authenticated Apple archive tool {tool}: {error}")
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        fail(f"Apple archive tool rejected {' '.join(arguments)}: {detail}")
    return result.stdout


def _validate_native_binaries(snapshot: Path, validator: object) -> None:
    if sys.platform != "darwin":
        fail("native XCFramework authentication requires a Darwin host")
    lipo = _pinned_native_tool("lipo")
    nm = _pinned_native_tool("nm")
    required_symbols = set(validator.EXPECTED_REQUIRED_SYMBOLS)
    forbidden_symbols = set(validator.EXPECTED_FORBIDDEN_SYMBOLS)
    for identifier, expected in validator.EXPECTED_SLICES.items():
        binary = snapshot / identifier / validator.LIBRARY_NAME
        architectures = _run_native_tool(lipo, ["-archs", str(binary)]).split()
        if (
            len(architectures) != len(set(architectures))
            or set(architectures) != set(expected["architectures"])
        ):
            fail(
                f"XCFramework native architecture mismatch for {identifier}: "
                f"expected {expected['architectures']}, found {architectures}"
            )
        raw_symbols = _run_native_tool(nm, ["-gUj", str(binary)])
        symbols = {
            line.strip().removeprefix("_")
            for line in raw_symbols.splitlines()
            if line.strip()
        }
        missing = sorted(required_symbols - symbols)
        forbidden = sorted(forbidden_symbols & symbols)
        if missing:
            fail(
                f"XCFramework {identifier} is missing required native symbols: "
                + ", ".join(missing)
            )
        if forbidden:
            fail(
                f"XCFramework {identifier} exports forbidden native symbols: "
                + ", ".join(forbidden)
            )


def _zip_info(
    name: str,
    stamp: tuple[int, int, int, int, int, int],
    directory: bool,
) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=stamp)
    info.create_system = 3
    info.create_version = 20
    info.extract_version = 20
    info.flag_bits = 0x800
    info.extra = b""
    info.comment = b""
    if directory:
        info.compress_type = zipfile.ZIP_STORED
        info.external_attr = ((stat.S_IFDIR | 0o755) << 16) | 0x10
    else:
        # Stored entries avoid host-zlib output variance. Static Apple libraries
        # are release artifacts; byte reproducibility is the primary contract.
        info.compress_type = zipfile.ZIP_STORED
        info.external_attr = (stat.S_IFREG | 0o644) << 16
    return info


def _write_archive(
    snapshot: Path,
    output: Path,
    stamp: tuple[int, int, int, int, int, int],
    expected_digests: dict[str, str],
) -> TemporaryArchive:
    file_descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.",
        suffix=".tmp",
        dir=output.parent,
    )
    temporary = Path(temporary_name)
    try:
        entries = _enumerate_source(snapshot)
        archive_entries: list[tuple[bytes, str, Path, bool]] = [
            (ARCHIVE_ROOT.encode("utf-8"), f"{ARCHIVE_ROOT}/", snapshot, True)
        ]
        for entry in entries:
            archive_name = f"{ARCHIVE_ROOT}/{entry.relative}"
            if entry.is_directory:
                archive_name += "/"
            archive_entries.append(
                (
                    archive_name.encode("utf-8"),
                    archive_name,
                    snapshot / entry.relative,
                    entry.is_directory,
                )
            )
        archive_entries.sort(key=lambda entry: entry[0])
        with os.fdopen(os.dup(file_descriptor), "w+b") as raw:
            with zipfile.ZipFile(
                raw,
                mode="w",
                compression=zipfile.ZIP_STORED,
                allowZip64=True,
                strict_timestamps=True,
            ) as archive:
                archive.comment = b""
                for _, archive_name, source_path, is_directory in archive_entries:
                    info = _zip_info(archive_name, stamp, is_directory)
                    if is_directory:
                        archive.writestr(info, b"")
                        continue
                    relative = source_path.relative_to(snapshot).as_posix()
                    hasher = hashlib.sha256()
                    with source_path.open("rb") as source_handle, archive.open(
                        info,
                        mode="w",
                        force_zip64=source_path.stat().st_size >= (1 << 31),
                    ) as archive_handle:
                        while True:
                            chunk = source_handle.read(CHUNK_SIZE)
                            if not chunk:
                                break
                            hasher.update(chunk)
                            archive_handle.write(chunk)
                    if hasher.hexdigest() != expected_digests[relative]:
                        fail(f"immutable snapshot changed while archiving: {relative}")
            raw.flush()
            os.fchmod(raw.fileno(), 0o644)
            os.fsync(raw.fileno())
        os.lseek(file_descriptor, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(file_descriptor, CHUNK_SIZE)
            if not chunk:
                break
            size += len(chunk)
            digest.update(chunk)
        metadata = os.fstat(file_descriptor)
        visible = temporary.lstat()
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_uid != os.geteuid()
            or (metadata.st_dev, metadata.st_ino)
            != (visible.st_dev, visible.st_ino)
            or metadata.st_mode != visible.st_mode
            or metadata.st_size != size
        ):
            fail("archive temporary path is not an authenticated regular file")
        archive = TemporaryArchive(
            path=temporary,
            descriptor=file_descriptor,
            device=metadata.st_dev,
            inode=metadata.st_ino,
            mode=metadata.st_mode,
            owner=metadata.st_uid,
            size=size,
            mtime_ns=metadata.st_mtime_ns,
            ctime_ns=metadata.st_ctime_ns,
            sha256=digest.hexdigest(),
        )
        file_descriptor = -1
        return archive
    except BaseException:
        if file_descriptor >= 0:
            os.close(file_descriptor)
        # Never unlink this public name after failure: it may have been swapped
        # for a foreign inode. The uniquely named cache residue is fail-closed.
        raise


def _rename_no_replace(temporary: Path, output: Path) -> None:
    if temporary.parent != output.parent:
        fail("atomic archive publication requires one destination directory")
    directory_fd = _open_directory(output.parent)
    try:
        library = ctypes.CDLL(None, use_errno=True)
        if sys.platform == "darwin":
            try:
                rename = library.renameatx_np
            except AttributeError:
                fail("atomic no-replace archive publication is unavailable")
            rename.argtypes = (
                ctypes.c_int,
                ctypes.c_char_p,
                ctypes.c_int,
                ctypes.c_char_p,
                ctypes.c_uint,
            )
            rename.restype = ctypes.c_int
            result = rename(
                directory_fd,
                os.fsencode(temporary.name),
                directory_fd,
                os.fsencode(output.name),
                0x00000004,  # RENAME_EXCL
            )
        elif sys.platform.startswith("linux"):
            try:
                rename = library.renameat2
            except AttributeError:
                fail("atomic no-replace archive publication is unavailable")
            rename.argtypes = (
                ctypes.c_int,
                ctypes.c_char_p,
                ctypes.c_int,
                ctypes.c_char_p,
                ctypes.c_uint,
            )
            rename.restype = ctypes.c_int
            result = rename(
                directory_fd,
                os.fsencode(temporary.name),
                directory_fd,
                os.fsencode(output.name),
                0x00000001,  # RENAME_NOREPLACE
            )
        else:
            fail("atomic no-replace archive publication is unavailable")
        if result != 0:
            error_number = ctypes.get_errno()
            if error_number in {errno.EEXIST, errno.ENOTEMPTY}:
                fail(f"archive output appeared before atomic publication: {output}")
            raise OSError(
                error_number,
                os.strerror(error_number),
                os.fspath(output),
            )
    finally:
        os.close(directory_fd)


def _atomic_publish(
    temporary: TemporaryArchive,
    output: Path,
) -> None:
    temporary.assert_at_path(temporary.path, allow_rename_ctime=False)
    _assert_destination_absent(output)
    temporary.assert_at_path(temporary.path, allow_rename_ctime=False)
    _rename_no_replace(temporary.path, output)
    # A source-name swap racing the rename can never be accepted as a release:
    # the destination must still be the exact inode held open since creation.
    # On mismatch we deliberately retain every public pathname for inspection.
    temporary.assert_at_path(output, allow_rename_ctime=True)
    _fsync_directory(output.parent)


def archive_xcframework(
    source_raw: str,
    output_raw: str,
    scratch_raw: str,
) -> tuple[str, int]:
    repository_root = _repository_root()
    source = _canonical_existing_directory(source_raw, "NoritoBridge XCFramework")
    if source.name != ARCHIVE_ROOT:
        fail(f"XCFramework directory must be named exactly {ARCHIVE_ROOT}")
    artifact_root = _canonical_existing_directory(str(source.parent), "Apple artifact root")
    if _is_within(artifact_root, repository_root):
        fail("Apple artifact root must be outside the Iroha source tree")
    artifact_root_metadata = artifact_root.lstat()
    if artifact_root_metadata.st_uid != os.geteuid() or not os.access(
        artifact_root,
        os.R_OK | os.W_OK | os.X_OK,
    ):
        fail("Apple artifact root must be an owned, writable directory")
    output = _canonical_output(
        output_raw,
        source,
        repository_root,
    )
    scratch_root = _canonical_existing_directory(
        scratch_raw,
        "archive scratch root",
    )
    scratch_metadata = scratch_root.lstat()
    if (
        _is_within(scratch_root, repository_root)
        or _is_within(scratch_root, output.parent)
        or scratch_metadata.st_uid != os.geteuid()
        or not os.access(scratch_root, os.R_OK | os.W_OK | os.X_OK)
    ):
        fail(
            "archive scratch root must be an owned, writable external directory "
            "outside the archive output directory"
        )
    _, _, stamp = _source_date_epoch()

    temporary_archive: TemporaryArchive | None = None
    with _archive_lock(artifact_root) as archive_lock:
        try:
            snapshot_container, snapshot, digests = _snapshot_source(
                source,
                scratch_root,
            )
            print(
                f"[norito-bridge-archive] retained-snapshot={snapshot_container}",
                file=sys.stderr,
            )
            validator = _validate_generation(snapshot)
            _validate_native_binaries(snapshot, validator)
            temporary_archive = _write_archive(
                snapshot,
                output,
                stamp,
                digests,
            )
            archive_lock.assert_held()
            _atomic_publish(
                temporary_archive,
                output,
            )
            return temporary_archive.sha256, temporary_archive.size
        finally:
            if temporary_archive is not None:
                temporary_archive.close()


def _install_signal_handlers() -> None:
    def interrupted(signum: int, _frame: object) -> NoReturn:
        raise ArchiveInterrupted(f"archive interrupted by signal {signum}")

    for signal_number in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(signal_number, interrupted)


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create a sorted, SOURCE_DATE_EPOCH-normalized archive from one "
            "authenticated NoritoBridge.xcframework generation."
        )
    )
    parser.add_argument("--xcframework", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--scratch-dir", required=True)
    return parser.parse_args()


def main() -> None:
    if (
        sys.version_info[:2] != (3, 12)
        or not sys.flags.isolated
        or not sys.flags.no_site
    ):
        fail("archive owner requires isolated no-site Python 3.12 (-I -S)")
    args = _parse_args()
    _install_signal_handlers()
    digest, size = archive_xcframework(
        args.xcframework,
        args.output,
        args.scratch_dir,
    )
    print(f"[norito-bridge-archive] sha256={digest} bytes={size} path={args.output}")


if __name__ == "__main__":
    try:
        main()
    except (ArchiveError, OSError, zipfile.BadZipFile) as error:
        print(f"[norito-bridge-archive] ERROR: {error}", file=sys.stderr)
        raise SystemExit(1) from None
