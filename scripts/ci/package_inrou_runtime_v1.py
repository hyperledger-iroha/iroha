#!/usr/bin/env python3
"""Build the fixed, authenticated Inrou V1 minimal runtime root.

The production CLI intentionally has one destination and no compatibility
options.  It copies the host-ISA QEMU binary, ``setpriv``, their complete
dynamic-loader closure, and the fixed bind-mount placeholders into an atomic,
root-owned tree consumed by ``iroha3d_taira``.
"""

from __future__ import annotations

import argparse
import ctypes
import dataclasses
import errno
import hashlib
import os
import platform
import re
import secrets
import shutil
import stat
import struct
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Iterable, Sequence, Union


DESTINATION = Path("/opt/iroha/inrou-runtime-v1")
RUNTIME_ROOT_NAME = "root"
MANIFEST_NAME = "manifest.sha256"
MANIFEST_HEADER = "iroha-inrou-runtime-v1 sha256"

MAX_MANIFEST_BYTES = 1024 * 1024
MAX_FILE_BYTES = 1024 * 1024 * 1024
MAX_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
MAX_ENTRIES = 512
COPY_CHUNK_BYTES = 1024 * 1024
LDD_OUTPUT_MAX_BYTES = 1024 * 1024
RENAME_NOREPLACE = 1

QEMU_TARGET = PurePosixPath("/inrou/bin/qemu")
SETPRIV_TARGET = PurePosixPath("/inrou/bin/setpriv")
FIXED_HOST_TOOLS = (
    Path("/usr/bin/bwrap"),
    Path("/usr/bin/nsenter"),
    Path("/usr/bin/socat"),
)
DEFAULT_SETPRIV = Path("/usr/bin/setpriv")
DEFAULT_LDD = Path("/usr/bin/ldd")

REQUIRED_DIRECTORIES = (
    PurePosixPath("/dev"),
    PurePosixPath("/proc"),
    PurePosixPath("/tmp"),
    PurePosixPath("/inrou"),
    PurePosixPath("/inrou/bin"),
    PurePosixPath("/inrou/input"),
    PurePosixPath("/inrou/input/cloud-init"),
    PurePosixPath("/inrou/disk"),
)
PLACEHOLDERS = (
    PurePosixPath("/inrou/input/kernel"),
    PurePosixPath("/inrou/input/initrd"),
    PurePosixPath("/inrou/input/bundle"),
    PurePosixPath("/inrou/input/cloud-init/meta-data"),
    PurePosixPath("/inrou/input/cloud-init/network-config"),
    PurePosixPath("/inrou/input/cloud-init/user-data"),
    PurePosixPath("/inrou/disk/root"),
    *(PurePosixPath(f"/inrou/disk/lease{index}") for index in range(32)),
)

_DYNAMIC_LIBRARY_PREFIXES = ("/lib/", "/lib64/", "/usr/lib/", "/usr/lib64/")
_ADDRESS = r"0x[0-9A-Fa-f]+"
_LDD_RESOLVED = re.compile(
    rf"^[ \t]*(?P<name>[^ \t]+)[ \t]+=>[ \t]+(?P<path>/[^ \t]+)"
    rf"[ \t]+\((?P<address>{_ADDRESS})\)[ \t]*$"
)
_LDD_DIRECT = re.compile(
    rf"^[ \t]*(?P<path>/[^ \t]+)[ \t]+\((?P<address>{_ADDRESS})\)[ \t]*$"
)
_LDD_VDSO = re.compile(
    rf"^[ \t]*(?:linux-vdso\.so\.1|linux-gate\.so\.1)[ \t]+\((?:{_ADDRESS})\)[ \t]*$"
)


class PackagingError(RuntimeError):
    """Raised when the host or closure violates the Inrou V1 contract."""


@dataclasses.dataclass(frozen=True)
class ElfIdentity:
    """Host ELF identity and its kernel-selected interpreter."""

    elf_class: int
    byte_order: str
    machine: int
    interpreter: PurePosixPath


@dataclasses.dataclass(frozen=True)
class RuntimeFile:
    """One source file copied to an exact sandbox path."""

    target: PurePosixPath
    source: Path
    mode: int


@dataclasses.dataclass(frozen=True)
class ManifestFile:
    """Attested output metadata for one regular file."""

    target: PurePosixPath
    sha256: str
    exact_bytes: int
    mode: int


def _default_qemu() -> Path:
    machine = platform.machine().lower()
    if machine in {"x86_64", "amd64"}:
        return Path("/usr/bin/qemu-system-x86_64")
    if machine in {"aarch64", "arm64"}:
        return Path("/usr/bin/qemu-system-aarch64")
    raise PackagingError(
        f"Inrou V1 supports only x86_64 and aarch64 Linux hosts, not {machine!r}"
    )


def _canonical_absolute_host_path(path: Union[Path, str], label: str) -> Path:
    text = os.fspath(path)
    if not text.startswith("/") or "//" in text or "\\" in text:
        raise PackagingError(f"{label} must be one canonical absolute path: {text!r}")
    if any(ord(character) < 0x21 or ord(character) > 0x7E for character in text):
        raise PackagingError(f"{label} must use visible ASCII: {text!r}")
    if os.path.normpath(text) != text:
        raise PackagingError(f"{label} must not contain path traversal: {text!r}")
    return Path(text)


def _canonical_sandbox_path(path: PurePosixPath, label: str) -> PurePosixPath:
    text = path.as_posix()
    if not text.startswith("/") or text == "/" or "//" in text or "\\" in text:
        raise PackagingError(f"{label} must be a canonical non-root absolute path")
    if any(ord(character) < 0x21 or ord(character) > 0x7E for character in text):
        raise PackagingError(f"{label} must use visible ASCII")
    if PurePosixPath(text).as_posix() != text or any(part in {".", ".."} for part in path.parts):
        raise PackagingError(f"{label} contains path traversal")
    if text == "/run" or text.startswith("/run/") or text == "/sys" or text.startswith("/sys/"):
        raise PackagingError(f"{label} overlaps a forbidden host-state path")
    return path


def _canonical_dynamic_library_path(text: str) -> PurePosixPath:
    path = _canonical_sandbox_path(PurePosixPath(text), "dynamic dependency path")
    if path.as_posix() != text:
        raise PackagingError(f"dynamic dependency path is not canonical: {text!r}")
    if not any(text.startswith(prefix) for prefix in _DYNAMIC_LIBRARY_PREFIXES):
        raise PackagingError(f"dynamic dependency escapes fixed system-library roots: {text}")
    return path


def parse_ldd_output(output: str) -> tuple[PurePosixPath, ...]:
    """Parse only the canonical successful GNU ``ldd`` output surface."""

    if "\x00" in output or "\r" in output:
        raise PackagingError("ldd output contains a forbidden control record")
    dependencies: set[PurePosixPath] = set()
    saw_record = False
    for line in output.split("\n"):
        if not line:
            continue
        saw_record = True
        if _LDD_VDSO.fullmatch(line):
            continue
        resolved = _LDD_RESOLVED.fullmatch(line)
        if resolved is not None:
            dependency = _canonical_dynamic_library_path(resolved.group("path"))
            dependencies.add(dependency)
            continue
        direct = _LDD_DIRECT.fullmatch(line)
        if direct is not None:
            dependency = _canonical_dynamic_library_path(direct.group("path"))
            dependencies.add(dependency)
            continue
        raise PackagingError(f"ldd emitted an unsupported dependency record: {line!r}")
    if not saw_record:
        raise PackagingError("ldd emitted no dependency records")
    return tuple(sorted(dependencies, key=lambda item: item.as_posix().encode("ascii")))


def inspect_elf(path: Path) -> ElfIdentity:
    """Read the ELF header and the single mandatory PT_INTERP record."""

    try:
        with path.open("rb") as source:
            header = source.read(64)
            if len(header) < 52 or header[:4] != b"\x7fELF":
                raise PackagingError(f"runtime executable is not an ELF file: {path}")
            elf_class = header[4]
            data_encoding = header[5]
            if elf_class not in {1, 2} or data_encoding not in {1, 2}:
                raise PackagingError(f"runtime executable has an unsupported ELF encoding: {path}")
            byte_order = "<" if data_encoding == 1 else ">"
            if elf_class == 1:
                machine = struct.unpack_from(f"{byte_order}H", header, 18)[0]
                program_offset = struct.unpack_from(f"{byte_order}I", header, 28)[0]
                program_entry_bytes = struct.unpack_from(f"{byte_order}H", header, 42)[0]
                program_count = struct.unpack_from(f"{byte_order}H", header, 44)[0]
                minimum_entry_bytes = 32
                elf_header_bytes = 52
            else:
                machine = struct.unpack_from(f"{byte_order}H", header, 18)[0]
                program_offset = struct.unpack_from(f"{byte_order}Q", header, 32)[0]
                program_entry_bytes = struct.unpack_from(f"{byte_order}H", header, 54)[0]
                program_count = struct.unpack_from(f"{byte_order}H", header, 56)[0]
                minimum_entry_bytes = 56
                elf_header_bytes = 64
            if not (1 <= program_count <= 1024) or program_entry_bytes < minimum_entry_bytes:
                raise PackagingError(
                    f"runtime executable has a malformed program-header table: {path}"
                )
            file_bytes = os.fstat(source.fileno()).st_size
            table_end = program_offset + program_entry_bytes * program_count
            if program_offset < elf_header_bytes or table_end > file_bytes:
                raise PackagingError(
                    f"runtime executable has an out-of-bounds program-header table: {path}"
                )
            interpreters: list[bytes] = []
            for index in range(program_count):
                source.seek(program_offset + index * program_entry_bytes)
                program_header = source.read(program_entry_bytes)
                if len(program_header) != program_entry_bytes:
                    raise PackagingError(f"runtime executable program header was truncated: {path}")
                program_type = struct.unpack_from(f"{byte_order}I", program_header, 0)[0]
                if program_type != 3:  # PT_INTERP
                    continue
                if elf_class == 1:
                    segment_offset = struct.unpack_from(f"{byte_order}I", program_header, 4)[0]
                    segment_bytes = struct.unpack_from(f"{byte_order}I", program_header, 16)[0]
                else:
                    segment_offset = struct.unpack_from(f"{byte_order}Q", program_header, 8)[0]
                    segment_bytes = struct.unpack_from(f"{byte_order}Q", program_header, 32)[0]
                if not (2 <= segment_bytes <= 4096) or segment_offset + segment_bytes > file_bytes:
                    raise PackagingError(
                        f"runtime executable has an invalid PT_INTERP segment: {path}"
                    )
                source.seek(segment_offset)
                interpreters.append(source.read(segment_bytes))
    except OSError as error:
        raise PackagingError(f"failed to inspect runtime ELF {path}: {error}") from error
    if len(interpreters) != 1:
        raise PackagingError(f"runtime executable must carry exactly one PT_INTERP segment: {path}")
    raw_interpreter = interpreters[0]
    if not raw_interpreter.endswith(b"\x00") or b"\x00" in raw_interpreter[:-1]:
        raise PackagingError(f"runtime executable has a non-canonical PT_INTERP string: {path}")
    try:
        interpreter_text = raw_interpreter[:-1].decode("ascii")
    except UnicodeDecodeError as error:
        raise PackagingError(f"runtime executable PT_INTERP is not ASCII: {path}") from error
    interpreter = _canonical_dynamic_library_path(interpreter_text)
    return ElfIdentity(elf_class, byte_order, machine, interpreter)


def _validate_path_chain(
    path: Path,
    *,
    owner_uid: int,
    owner_gid: int,
    allow_final_symlink: bool,
    label: str,
) -> Path:
    path = _canonical_absolute_host_path(path, label)
    prefixes = [Path("/")]
    current = Path("/")
    for part in path.parts[1:]:
        current /= part
        prefixes.append(current)
    for prefix in prefixes:
        try:
            metadata = os.lstat(prefix)
        except OSError as error:
            raise PackagingError(f"cannot inspect {label} component {prefix}: {error}") from error
        final = prefix == path
        is_link = stat.S_ISLNK(metadata.st_mode)
        if is_link:
            if not final or not allow_final_symlink:
                raise PackagingError(f"{label} must not be a symbolic link: {path}")
            if metadata.st_uid != owner_uid or metadata.st_gid != owner_gid:
                raise PackagingError(f"{label} symlink is not owned by the fixed owner: {prefix}")
            continue
        if final:
            continue
        if not stat.S_ISDIR(metadata.st_mode):
            raise PackagingError(f"{label} ancestor is not a directory: {prefix}")
        owner_is_trusted = (metadata.st_uid, metadata.st_gid) in {
            (0, 0),
            (owner_uid, owner_gid),
        }
        if not owner_is_trusted or metadata.st_mode & 0o022:
            raise PackagingError(f"{label} ancestor is not owner-custodied: {prefix}")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise PackagingError(f"cannot resolve {label} {path}: {error}") from error
    _canonical_absolute_host_path(resolved, f"resolved {label}")
    return resolved


def validate_regular_source(
    path: Path,
    *,
    owner_uid: int,
    owner_gid: int,
    executable: bool,
    allow_symlink: bool,
    label: str,
) -> Path:
    """Return one securely resolved, immutable, singly-linked source file."""

    resolved = _validate_path_chain(
        path,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        allow_final_symlink=allow_symlink,
        label=label,
    )
    _validate_path_chain(
        resolved,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        allow_final_symlink=False,
        label=f"resolved {label}",
    )
    try:
        metadata = os.lstat(resolved)
    except OSError as error:
        raise PackagingError(f"cannot inspect resolved {label} {resolved}: {error}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise PackagingError(f"{label} must resolve to a regular file: {path}")
    if metadata.st_uid != owner_uid or metadata.st_gid != owner_gid:
        raise PackagingError(f"{label} must resolve to the fixed owner: {resolved}")
    if metadata.st_nlink != 1:
        raise PackagingError(f"{label} must resolve to a singly-linked file: {resolved}")
    if metadata.st_mode & 0o7022:
        raise PackagingError(f"{label} must not be privileged or group/other writable: {resolved}")
    if executable and metadata.st_mode & 0o111 == 0:
        raise PackagingError(f"{label} must be executable: {resolved}")
    if metadata.st_size > MAX_FILE_BYTES:
        raise PackagingError(f"{label} exceeds the fixed per-file limit: {resolved}")
    return resolved


def _run_ldd(binary: Path, ldd: Path) -> tuple[PurePosixPath, ...]:
    try:
        result = subprocess.run(
            [os.fspath(ldd), os.fspath(binary)],
            cwd="/",
            env={"LC_ALL": "C", "LANG": "C", "PATH": "/usr/bin:/bin"},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PackagingError(f"failed to resolve ELF dependencies for {binary}: {error}") from error
    if result.returncode != 0:
        raise PackagingError(
            f"ldd rejected {binary} with status {result.returncode}: "
            f"{result.stderr[:4096].decode('utf-8', 'replace')!r}"
        )
    if result.stderr:
        raise PackagingError(f"ldd emitted unexpected stderr for {binary}")
    if len(result.stdout) > LDD_OUTPUT_MAX_BYTES:
        raise PackagingError(f"ldd output for {binary} exceeds its fixed bound")
    try:
        output = result.stdout.decode("ascii")
    except UnicodeDecodeError as error:
        raise PackagingError(f"ldd output for {binary} is not ASCII") from error
    return parse_ldd_output(output)


def collect_runtime_files(
    qemu: Path,
    setpriv: Path,
    ldd: Path,
    *,
    owner_uid: int = 0,
    owner_gid: int = 0,
) -> tuple[RuntimeFile, ...]:
    """Resolve QEMU, setpriv, their interpreters, and their full ldd closure."""

    ldd_resolved = validate_regular_source(
        ldd,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        executable=True,
        allow_symlink=False,
        label="fixed ldd",
    )
    executable_sources = (
        (QEMU_TARGET, qemu, "QEMU"),
        (SETPRIV_TARGET, setpriv, "setpriv"),
    )
    files: dict[PurePosixPath, RuntimeFile] = {}
    elf_identities: list[ElfIdentity] = []
    executable_records: list[tuple[Path, ElfIdentity]] = []
    for target, source, label in executable_sources:
        resolved = validate_regular_source(
            source,
            owner_uid=owner_uid,
            owner_gid=owner_gid,
            executable=True,
            allow_symlink=False,
            label=label,
        )
        identity = inspect_elf(resolved)
        elf_identities.append(identity)
        executable_records.append((resolved, identity))
        files[target] = RuntimeFile(target, resolved, 0o555)
    if len({(item.elf_class, item.byte_order, item.machine) for item in elf_identities}) != 1:
        raise PackagingError("QEMU and setpriv do not share one host ELF identity")

    interpreter_targets = {identity.interpreter for identity in elf_identities}
    for executable, identity in executable_records:
        dependencies = set(_run_ldd(executable, ldd_resolved))
        dependencies.add(identity.interpreter)
        for target in sorted(dependencies, key=lambda item: item.as_posix().encode("ascii")):
            source = validate_regular_source(
                Path(target.as_posix()),
                owner_uid=owner_uid,
                owner_gid=owner_gid,
                executable=target in interpreter_targets,
                allow_symlink=True,
                label=f"dynamic dependency {target}",
            )
            record = RuntimeFile(
                target=target,
                source=source,
                mode=0o555 if target in interpreter_targets else 0o444,
            )
            previous = files.get(target)
            if previous is not None and (
                previous.source != record.source or previous.mode != record.mode
            ):
                raise PackagingError(f"dynamic dependency target has conflicting sources: {target}")
            files[target] = record
    return tuple(sorted(files.values(), key=lambda item: item.target.as_posix().encode("ascii")))


def _set_fd_owner(descriptor: int, owner_uid: int, owner_gid: int) -> None:
    metadata = os.fstat(descriptor)
    if metadata.st_uid != owner_uid or metadata.st_gid != owner_gid:
        os.fchown(descriptor, owner_uid, owner_gid)


def _validate_attested_source_metadata(
    metadata: os.stat_result,
    source: Path,
    *,
    owner_uid: int,
    owner_gid: int,
    executable: bool,
    phase: str,
) -> None:
    """Fail unless an opened source still satisfies every custody invariant."""

    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != owner_uid
        or metadata.st_gid != owner_gid
        or metadata.st_nlink != 1
        or metadata.st_mode & 0o7022
        or (executable and metadata.st_mode & 0o111 == 0)
    ):
        raise PackagingError(f"runtime source changed identity {phase}: {source}")


def _copy_attested_file(
    source: Path,
    destination: Path,
    *,
    mode: int,
    owner_uid: int,
    owner_gid: int,
) -> tuple[str, int]:
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    destination_flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        source_descriptor = os.open(source, source_flags)
    except OSError as error:
        raise PackagingError(f"runtime source is not one direct regular file: {source}") from error
    try:
        source_before = os.fstat(source_descriptor)
        _validate_attested_source_metadata(
            source_before,
            source,
            owner_uid=owner_uid,
            owner_gid=owner_gid,
            executable=mode == 0o555,
            phase="before copy",
        )
        destination_descriptor = os.open(destination, destination_flags, 0o600)
        try:
            digest = hashlib.sha256()
            exact_bytes = 0
            while True:
                chunk = os.read(source_descriptor, COPY_CHUNK_BYTES)
                if not chunk:
                    break
                exact_bytes += len(chunk)
                if exact_bytes > MAX_FILE_BYTES:
                    raise PackagingError(
                        f"runtime source exceeds the fixed per-file limit: {source}"
                    )
                digest.update(chunk)
                view = memoryview(chunk)
                while view:
                    written = os.write(destination_descriptor, view)
                    if written <= 0:
                        raise PackagingError(f"short write while packaging {source}")
                    view = view[written:]
            source_after = os.fstat(source_descriptor)
            _validate_attested_source_metadata(
                source_after,
                source,
                owner_uid=owner_uid,
                owner_gid=owner_gid,
                executable=mode == 0o555,
                phase="after copy",
            )
            if (
                source_after.st_dev != source_before.st_dev
                or source_after.st_ino != source_before.st_ino
                or source_after.st_mode != source_before.st_mode
                or source_after.st_uid != source_before.st_uid
                or source_after.st_gid != source_before.st_gid
                or source_after.st_nlink != source_before.st_nlink
                or source_after.st_size != source_before.st_size
                or source_after.st_mtime_ns != source_before.st_mtime_ns
                or source_after.st_ctime_ns != source_before.st_ctime_ns
            ):
                raise PackagingError(f"runtime source changed while copied: {source}")
            if exact_bytes != source_before.st_size:
                raise PackagingError(f"runtime source changed length while copied: {source}")
            _set_fd_owner(destination_descriptor, owner_uid, owner_gid)
            os.fchmod(destination_descriptor, mode)
            os.fsync(destination_descriptor)
            return digest.hexdigest(), exact_bytes
        finally:
            os.close(destination_descriptor)
    finally:
        os.close(source_descriptor)


def _target_host_path(root: Path, target: PurePosixPath) -> Path:
    _canonical_sandbox_path(target, "runtime target")
    return root.joinpath(*target.parts[1:])


def _all_directories(files: Iterable[PurePosixPath]) -> set[PurePosixPath]:
    directories = set(REQUIRED_DIRECTORIES)
    for file_path in files:
        parent = file_path.parent
        while parent != PurePosixPath("/"):
            directories.add(parent)
            parent = parent.parent
    return directories


def _manifest_bytes(
    directories: Iterable[PurePosixPath], files: Iterable[ManifestFile]
) -> bytes:
    records: dict[str, str] = {"/": "d - 0 0555 /"}
    for directory in directories:
        text = _canonical_sandbox_path(directory, "runtime directory").as_posix()
        records[text] = f"d - 0 0555 {text}"
    for file in files:
        text = _canonical_sandbox_path(file.target, "runtime file").as_posix()
        if text in records:
            raise PackagingError(f"runtime manifest repeats target {text}")
        records[text] = f"f {file.sha256} {file.exact_bytes} {file.mode:04o} {text}"
    ordered_paths = sorted(records, key=lambda item: item.encode("ascii"))
    if len(ordered_paths) > MAX_ENTRIES:
        raise PackagingError("Inrou runtime closure exceeds the fixed 512-entry limit")
    records_payload = "\n".join(records[path] for path in ordered_paths)
    payload = f"{MANIFEST_HEADER}\n{records_payload}\n".encode("ascii")
    if len(payload) > MAX_MANIFEST_BYTES:
        raise PackagingError("Inrou runtime manifest exceeds the fixed byte limit")
    return payload


def _write_manifest(
    path: Path, payload: bytes, *, owner_uid: int, owner_gid: int
) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags, 0o600)
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise PackagingError("short write while creating the Inrou runtime manifest")
            view = view[written:]
        _set_fd_owner(descriptor, owner_uid, owner_gid)
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    """Persist one already-materialized directory without following its leaf."""

    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    descriptor = os.open(path, flags)
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISDIR(metadata.st_mode):
            raise PackagingError(f"runtime materialization entry is not a directory: {path}")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def materialize_runtime(
    staging_directory: Path,
    runtime_files: Sequence[RuntimeFile],
    *,
    owner_uid: int,
    owner_gid: int,
) -> None:
    """Materialize one complete runtime inside an unpublished staging directory."""

    targets: dict[PurePosixPath, RuntimeFile] = {}
    for runtime_file in runtime_files:
        target = _canonical_sandbox_path(runtime_file.target, "runtime file target")
        if runtime_file.mode not in {0o444, 0o555}:
            raise PackagingError(f"runtime file {target} has a forbidden mode")
        if target in targets or target in PLACEHOLDERS:
            raise PackagingError(f"runtime file target is repeated or reserved: {target}")
        targets[target] = runtime_file
    if QEMU_TARGET not in targets or targets[QEMU_TARGET].mode != 0o555:
        raise PackagingError("runtime closure omitted executable /inrou/bin/qemu")
    if SETPRIV_TARGET not in targets or targets[SETPRIV_TARGET].mode != 0o555:
        raise PackagingError("runtime closure omitted executable /inrou/bin/setpriv")

    all_file_targets = tuple(targets) + PLACEHOLDERS
    directories = _all_directories(all_file_targets)
    conflicts = set(all_file_targets).intersection(directories)
    if conflicts:
        rendered = ", ".join(sorted(path.as_posix() for path in conflicts))
        raise PackagingError(f"runtime file conflicts with a required directory: {rendered}")
    root = staging_directory / RUNTIME_ROOT_NAME
    root.mkdir(mode=0o700)
    os.chown(root, owner_uid, owner_gid)
    ordered_directories = sorted(
        directories,
        key=lambda item: (len(item.parts), item.as_posix().encode("ascii")),
    )
    for directory in ordered_directories:
        host_path = _target_host_path(root, directory)
        host_path.mkdir(mode=0o700)
        os.chown(host_path, owner_uid, owner_gid)

    manifest_files: list[ManifestFile] = []
    total_bytes = 0
    for target, runtime_file in sorted(
        targets.items(), key=lambda item: item[0].as_posix().encode("ascii")
    ):
        destination = _target_host_path(root, target)
        digest, exact_bytes = _copy_attested_file(
            runtime_file.source,
            destination,
            mode=runtime_file.mode,
            owner_uid=owner_uid,
            owner_gid=owner_gid,
        )
        total_bytes += exact_bytes
        if total_bytes > MAX_TOTAL_BYTES:
            raise PackagingError("Inrou runtime closure exceeds the fixed aggregate byte limit")
        manifest_files.append(ManifestFile(target, digest, exact_bytes, runtime_file.mode))
    empty_digest = hashlib.sha256(b"").hexdigest()
    for target in PLACEHOLDERS:
        destination = _target_host_path(root, target)
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(destination, flags, 0o600)
        try:
            _set_fd_owner(descriptor, owner_uid, owner_gid)
            os.fchmod(descriptor, 0o444)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        manifest_files.append(ManifestFile(target, empty_digest, 0, 0o444))

    manifest_payload = _manifest_bytes(directories, manifest_files)
    _write_manifest(
        staging_directory / MANIFEST_NAME,
        manifest_payload,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
    )
    for directory in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        os.chmod(_target_host_path(root, directory), 0o555)
    os.chmod(root, 0o555)
    os.chown(staging_directory, owner_uid, owner_gid)
    os.chmod(staging_directory, 0o555)
    # Files are individually fsync'd above. Persist every containing directory
    # from leaves to the unpublished staging root before renameat2 makes the
    # tree visible; a successful publication must never name an incomplete
    # runtime after a crash.
    for directory in sorted(
        directories,
        key=lambda item: (-len(item.parts), item.as_posix().encode("ascii")),
    ):
        _fsync_directory(_target_host_path(root, directory))
    _fsync_directory(root)
    _fsync_directory(staging_directory)


def _remove_staging_directory(path: Path) -> None:
    for directory, child_directories, files in os.walk(path, topdown=False):
        for file_name in files:
            os.chmod(Path(directory) / file_name, 0o600, follow_symlinks=False)
        for child_name in child_directories:
            os.chmod(Path(directory) / child_name, 0o700, follow_symlinks=False)
        os.chmod(directory, 0o700, follow_symlinks=False)
    shutil.rmtree(path)


def _linux_renameat2_noreplace(
    parent_descriptor: int, source_name: str, destination_name: str
) -> None:
    """Publish one sibling directory with the kernel's no-replace guarantee."""

    try:
        renameat2 = ctypes.CDLL(None, use_errno=True).renameat2
    except (AttributeError, OSError) as error:
        raise PackagingError("libc does not expose required renameat2") from error
    renameat2.argtypes = (
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    )
    renameat2.restype = ctypes.c_int
    ctypes.set_errno(0)
    result = renameat2(
        parent_descriptor,
        os.fsencode(source_name),
        parent_descriptor,
        os.fsencode(destination_name),
        RENAME_NOREPLACE,
    )
    if result == 0:
        return
    error_number = ctypes.get_errno()
    if error_number == errno.EEXIST:
        raise PackagingError(
            f"runtime destination already exists: {destination_name}"
        )
    raise PackagingError(
        "renameat2(RENAME_NOREPLACE) failed while publishing the Inrou runtime: "
        f"{os.strerror(error_number)}"
    )


def _publish_staging_noreplace(
    parent_descriptor: int, source_name: str, destination_name: str
) -> None:
    if any(not name or "/" in name or "\0" in name for name in (source_name, destination_name)):
        raise PackagingError("runtime publication names must be single path components")
    if sys.platform == "linux":
        _linux_renameat2_noreplace(parent_descriptor, source_name, destination_name)
        return
    # Unit tests exercise materialization on non-Linux hosts, but main() makes
    # this fallback unreachable in production. Keep its precondition strict.
    try:
        os.stat(destination_name, dir_fd=parent_descriptor, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        raise PackagingError(f"runtime destination already exists: {destination_name}")
    os.rename(
        source_name,
        destination_name,
        src_dir_fd=parent_descriptor,
        dst_dir_fd=parent_descriptor,
    )


def install_runtime(
    destination: Path,
    runtime_files: Sequence[RuntimeFile],
    *,
    owner_uid: int = 0,
    owner_gid: int = 0,
) -> None:
    """Build and atomically publish a previously absent runtime directory."""

    destination = _canonical_absolute_host_path(destination, "runtime destination")
    parent = destination.parent
    _validate_path_chain(
        parent,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        allow_final_symlink=False,
        label="runtime destination parent",
    )
    try:
        parent_metadata = os.lstat(parent)
    except OSError as error:
        raise PackagingError(
            f"cannot inspect runtime destination parent {parent}: {error}"
        ) from error
    if (
        not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != owner_uid
        or parent_metadata.st_gid != owner_gid
        or parent_metadata.st_mode & 0o022
    ):
        raise PackagingError("runtime destination parent must be an owner-custodied directory")
    parent_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    try:
        parent_descriptor = os.open(parent, parent_flags)
    except OSError as error:
        raise PackagingError(f"cannot pin runtime destination parent {parent}: {error}") from error
    pinned_parent = os.fstat(parent_descriptor)
    if (
        pinned_parent.st_dev != parent_metadata.st_dev
        or pinned_parent.st_ino != parent_metadata.st_ino
        or pinned_parent.st_uid != owner_uid
        or pinned_parent.st_gid != owner_gid
        or pinned_parent.st_mode & 0o022
    ):
        os.close(parent_descriptor)
        raise PackagingError("runtime destination parent changed while it was pinned")
    try:
        try:
            os.stat(destination.name, dir_fd=parent_descriptor, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise PackagingError(f"runtime destination already exists: {destination}")
        staging_name = ""
        for _ in range(128):
            candidate = f".{destination.name}.staging.{secrets.token_hex(12)}"
            try:
                os.mkdir(candidate, mode=0o700, dir_fd=parent_descriptor)
            except FileExistsError:
                continue
            staging_name = candidate
            break
        if not staging_name:
            raise PackagingError("exhausted exclusive Inrou runtime staging names")
        staging = (
            Path(f"/proc/self/fd/{parent_descriptor}") / staging_name
            if sys.platform == "linux"
            else parent / staging_name
        )
        try:
            os.chown(staging, owner_uid, owner_gid)
            os.chmod(staging, 0o700)
            materialize_runtime(
                staging,
                runtime_files,
                owner_uid=owner_uid,
                owner_gid=owner_gid,
            )
            _publish_staging_noreplace(
                parent_descriptor, staging_name, destination.name
            )
            os.fsync(parent_descriptor)
        except BaseException:
            if staging.exists():
                _remove_staging_directory(staging)
            raise
    finally:
        os.close(parent_descriptor)


def _validate_fixed_host_tools(*, owner_uid: int = 0, owner_gid: int = 0) -> None:
    for tool in FIXED_HOST_TOOLS:
        validate_regular_source(
            tool,
            owner_uid=owner_uid,
            owner_gid=owner_gid,
            executable=True,
            allow_symlink=False,
            label=f"fixed host tool {tool}",
        )


def _parser() -> argparse.ArgumentParser:
    def host_path(value: str) -> Path:
        try:
            return _canonical_absolute_host_path(value, "command-line host path")
        except PackagingError as error:
            raise argparse.ArgumentTypeError(str(error)) from error

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=host_path, help="fixed host-ISA QEMU executable")
    parser.add_argument("--setpriv", type=host_path, default=DEFAULT_SETPRIV)
    parser.add_argument("--ldd", type=host_path, default=DEFAULT_LDD)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if sys.platform != "linux":
        raise PackagingError("Inrou V1 runtime packaging requires Linux")
    if os.geteuid() != 0 or os.getegid() != 0:
        raise PackagingError("Inrou V1 runtime packaging requires root:root")
    qemu = args.qemu if args.qemu is not None else _default_qemu()
    _validate_fixed_host_tools()
    runtime_files = collect_runtime_files(qemu, args.setpriv, args.ldd)
    install_runtime(DESTINATION, runtime_files)
    print(f"packaged {len(runtime_files)} Inrou runtime closure files at {DESTINATION}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except PackagingError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
