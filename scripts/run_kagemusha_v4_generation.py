#!/usr/bin/env python3
"""Run Kagemusha V4 candidate generation under a bounded polling process-group guard."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import time
from typing import Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.formal import run_sumeragi_v2_tlapm_guard as resource_guard


ABSOLUTE_MAX_MEMORY_BYTES = 256 * 1024 * 1024
SAMPLE_INTERVAL_SECONDS = 0.05
BYTES_PER_GIB = 1024 * 1024 * 1024
LOCK_PATH = Path("/tmp") / f"iroha-kagemusha-v4-{os.getuid()}.lock"
STAGING_ID_OPTION = "--staging-id"
STAGING_NAME_OPTION = "--staging-name"
OUTPUT_PARENT_FD_OPTION = "--output-parent-fd"
STAGING_ID_HEX_LENGTH = 32
STAGING_PREFIX = ".kagemusha-v4-staging-"
BUNDLE_EXECUTABLE = "kagemusha_recursive_spend_v4_bundle"
JOURNAL_PREFIX = ".kagemusha-v4-guard-"
JOURNAL_SUFFIX = ".json"
JOURNAL_SCHEMA = "iroha.kagemusha.candidate_guard_journal.v1"
MAX_JOURNAL_BYTES = 4096
MAX_RECOVERABLE_JOURNALS = 64
CANDIDATE_SESSION_WRAPPER_FLAG = "--kagemusha-candidate-session-wrapper"
MINIMUM_OUTPUT_FREE_BYTES = 512 * 1024 * 1024
DISK_BACKED_OUTPUT_FILESYSTEM_TYPES = frozenset(
    {
        "apfs",
        "btrfs",
        "ext2",
        "ext2/ext3",
        "ext3",
        "ext4",
        "f2fs",
        "fuseblk",
        "hfs",
        "hfsplus",
        "jfs",
        "reiserfs",
        "ufs",
        "xfs",
        "zfs",
    }
)


@dataclass
class ExecutionCopy:
    """A Darwin-safe private copy materialized only from the admitted fd."""

    directory_name: str
    file_name: str
    path: Path
    directory_descriptor: int
    file_descriptor: int
    directory_device: int
    directory_inode: int
    file_device: int
    file_inode: int
    mode: int
    size_bytes: int
    sha256: str

    def report_context(self) -> dict[str, object]:
        """Return the identity of the exact private execution copy."""

        return {
            "canonical_path": str(self.path),
            "directory_device": self.directory_device,
            "directory_inode": self.directory_inode,
            "directory_name": self.directory_name,
            "file_device": self.file_device,
            "file_inode": self.file_inode,
            "file_name": self.file_name,
            "method": "darwin_private_fd_copy",
            "mode": self.mode,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
        }


@dataclass
class ExecutableSnapshot:
    """Cryptographic and filesystem identity of one admitted executable."""

    path: Path
    sha256: str
    size_bytes: int
    device: int
    inode: int
    mode: int
    link_count: int
    owner_uid: int
    modified_ns: int
    changed_ns: int
    descriptor: int
    execution_copy: ExecutionCopy | None = None

    def report_context(self) -> dict[str, object]:
        """Return stable JSON evidence for this exact executable snapshot."""

        context: dict[str, object] = {
            "canonical_path": str(self.path),
            "build_profile": self.path.parent.name,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
            "stat_identity": {
                "changed_ns": self.changed_ns,
                "device": self.device,
                "inode": self.inode,
                "link_count": self.link_count,
                "mode": self.mode,
                "modified_ns": self.modified_ns,
                "owner_uid": self.owner_uid,
            },
        }
        if self.execution_copy is None:
            context["execution"] = {
                "descriptor_path": self.execution_path(),
                "method": "pinned_fd",
            }
        else:
            context["execution"] = self.execution_copy.report_context()
        return context

    def execution_path(self) -> str:
        """Return the inherited descriptor path used for both executions."""

        execution_descriptor = self.execution_descriptor()
        if execution_descriptor < 3:
            raise resource_guard.GuardError(
                "Kagemusha executable descriptor is unavailable"
            )
        if self.execution_copy is not None:
            return str(self.execution_copy.path)
        return f"/proc/self/fd/{execution_descriptor}"

    def execution_descriptor(self) -> int:
        """Return the fd which pins the bytes used by the next exec."""

        if self.execution_copy is not None:
            return self.execution_copy.file_descriptor
        return self.descriptor

    def close(self) -> None:
        """Close the executable descriptor retained for the full lifecycle."""

        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1
        if self.execution_copy is not None:
            for descriptor_name in ("file_descriptor", "directory_descriptor"):
                descriptor = getattr(self.execution_copy, descriptor_name)
                if descriptor >= 0:
                    os.close(descriptor)
                    setattr(self.execution_copy, descriptor_name, -1)


@dataclass
class PinnedOutputParent:
    """An output parent held open by identity for cleanup and journaling."""

    path: Path
    descriptor: int
    device: int
    inode: int
    output_name: str
    filesystem_type: str
    free_bytes_at_admission: int

    def validate(self, *, require_path: bool = True) -> None:
        """Require the descriptor identity and, when requested, its original path."""

        try:
            opened = os.fstat(self.descriptor)
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha output parent identity is unavailable"
            ) from error
        if (
            not stat.S_ISDIR(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (self.device, self.inode)
        ):
            raise resource_guard.GuardError("Kagemusha output parent identity changed")
        if require_path:
            try:
                current = os.stat(self.path, follow_symlinks=False)
            except OSError as error:
                raise resource_guard.GuardError(
                    "Kagemusha output parent path is unavailable"
                ) from error
            if (
                not stat.S_ISDIR(current.st_mode)
                or (current.st_dev, current.st_ino) != (self.device, self.inode)
            ):
                raise resource_guard.GuardError(
                    "Kagemusha output parent path identity changed"
                )

    def report_context(self) -> dict[str, object]:
        """Return stable JSON evidence for the pinned output parent."""

        return {
            "canonical_path": str(self.path),
            "device": self.device,
            "filesystem_type": self.filesystem_type,
            "free_bytes_at_admission": self.free_bytes_at_admission,
            "inode": self.inode,
            "minimum_free_bytes": MINIMUM_OUTPUT_FREE_BYTES,
            "output_name": self.output_name,
        }

    def close(self) -> None:
        """Close the owned directory descriptor."""

        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


def _executable_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return every mutable stat field bound by an executable snapshot."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _validate_executable_metadata(metadata: os.stat_result) -> None:
    """Require one owner-controlled, single-link, non-empty executable file."""

    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
        or metadata.st_size <= 0
        or metadata.st_mode & stat.S_IXUSR == 0
        or metadata.st_mode & 0o022 != 0
    ):
        raise resource_guard.GuardError(
            "Kagemusha executable has unsafe ownership, links, mode, or size"
        )


def _hash_executable_descriptor(
    descriptor: int,
) -> tuple[os.stat_result, str]:
    """Hash a pinned executable without changing its shared file offset."""

    before = os.fstat(descriptor)
    _validate_executable_metadata(before)
    digest = hashlib.sha256()
    offset = 0
    while offset < before.st_size:
        if hasattr(os, "pread"):
            chunk = os.pread(descriptor, min(1024 * 1024, before.st_size - offset), offset)
        else:  # pragma: no cover - every supported Unix platform exposes pread
            os.lseek(descriptor, offset, os.SEEK_SET)
            chunk = os.read(descriptor, min(1024 * 1024, before.st_size - offset))
        if not chunk:
            raise resource_guard.GuardError(
                "Kagemusha executable ended while it was being hashed"
            )
        digest.update(chunk)
        offset += len(chunk)
    after = os.fstat(descriptor)
    if _executable_stat_identity(before) != _executable_stat_identity(after):
        raise resource_guard.GuardError(
            "Kagemusha executable changed while it was being hashed"
        )
    return after, digest.hexdigest()


def _open_executable_identity(path: Path) -> tuple[int, os.stat_result, str]:
    """Open and hash one safe regular executable, retaining its descriptor."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha executable cannot be opened safely: {error}"
        ) from error
    try:
        after, digest = _hash_executable_descriptor(descriptor)
        path_after = os.stat(path, follow_symlinks=False)
        if _executable_stat_identity(after) != _executable_stat_identity(path_after):
            raise resource_guard.GuardError(
                "Kagemusha executable changed while it was being admitted"
            )
        return descriptor, after, digest
    except BaseException:
        os.close(descriptor)
        raise


def _read_executable_identity(path: Path) -> tuple[os.stat_result, str]:
    """Hash one safe executable through a short-lived descriptor."""

    descriptor, metadata, digest = _open_executable_identity(path)
    os.close(descriptor)
    return metadata, digest


def _snapshot_executable(path_text: str, expected_name: str) -> ExecutableSnapshot:
    """Admit and cryptographically snapshot one exact prebuilt executable."""

    supplied = Path(path_text)
    try:
        supplied_metadata = supplied.lstat()
        resolved = supplied.resolve(strict=True)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha executable is unavailable: {error}"
        ) from error
    if stat.S_ISLNK(supplied_metadata.st_mode):
        raise resource_guard.GuardError("Kagemusha executable must not be a symlink")
    admitted_name = (
        resolved.name[:-4] if resolved.name.endswith(".exe") else resolved.name
    )
    if admitted_name != expected_name:
        raise resource_guard.GuardError(
            f"Kagemusha resource guard requires the prebuilt {expected_name} executable"
        )
    descriptor, metadata, sha256 = _open_executable_identity(resolved)
    return ExecutableSnapshot(
        path=resolved,
        sha256=sha256,
        size_bytes=metadata.st_size,
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=metadata.st_mode,
        link_count=metadata.st_nlink,
        owner_uid=metadata.st_uid,
        modified_ns=metadata.st_mtime_ns,
        changed_ns=metadata.st_ctime_ns,
        descriptor=descriptor,
    )


def _validate_execution_copy(snapshot: ExecutableSnapshot) -> None:
    """Validate the private Darwin copy without rereading the source path."""

    if snapshot.execution_copy is not None:
        copy = snapshot.execution_copy
        copied_metadata, copied_sha256 = _hash_executable_descriptor(
            copy.file_descriptor
        )
        try:
            path_metadata = os.stat(copy.path, follow_symlinks=False)
            directory_metadata = os.fstat(copy.directory_descriptor)
            directory_path_metadata = os.stat(
                copy.path.parent, follow_symlinks=False
            )
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha private execution copy is unavailable"
            ) from error
        if (
            (copied_metadata.st_dev, copied_metadata.st_ino)
            != (copy.file_device, copy.file_inode)
            or (path_metadata.st_dev, path_metadata.st_ino)
            != (copy.file_device, copy.file_inode)
            or stat.S_IMODE(copied_metadata.st_mode) != 0o500
            or copied_metadata.st_size != snapshot.size_bytes
            or copied_sha256 != snapshot.sha256
            or (directory_metadata.st_dev, directory_metadata.st_ino)
            != (copy.directory_device, copy.directory_inode)
            or (directory_path_metadata.st_dev, directory_path_metadata.st_ino)
            != (copy.directory_device, copy.directory_inode)
            or stat.S_IMODE(directory_metadata.st_mode) != 0o500
        ):
            raise resource_guard.GuardError(
                "Kagemusha private execution copy changed after admission"
            )


def _validate_executable_unchanged(snapshot: ExecutableSnapshot) -> None:
    """Fail if the executable path, metadata, or bytes changed during the run."""

    pinned_metadata, pinned_sha256 = _hash_executable_descriptor(snapshot.descriptor)
    metadata, sha256 = _read_executable_identity(snapshot.path)
    expected_identity = (
        snapshot.device,
        snapshot.inode,
        snapshot.mode,
        snapshot.link_count,
        snapshot.owner_uid,
        snapshot.size_bytes,
        snapshot.modified_ns,
        snapshot.changed_ns,
    )
    if (
        _executable_stat_identity(pinned_metadata) != expected_identity
        or pinned_sha256 != snapshot.sha256
        or _executable_stat_identity(metadata) != expected_identity
        or sha256 != snapshot.sha256
    ):
        raise resource_guard.GuardError(
            "Kagemusha executable changed after admission"
        )
    _validate_execution_copy(snapshot)


def _execution_copy_name(staging_id: str) -> str:
    """Return the journal-bound private execution directory name."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{STAGING_PREFIX}{staging_id}-exec"


def _prepare_execution_copy(
    parent: PinnedOutputParent,
    snapshot: ExecutableSnapshot,
    staging_id: str,
    executable_name: str = BUNDLE_EXECUTABLE,
) -> None:
    """On Darwin, copy admitted fd bytes into a private disk-backed path."""

    if sys.platform != "darwin":
        return
    if snapshot.execution_copy is not None:
        raise resource_guard.GuardError("Kagemusha execution copy already exists")
    if (
        not executable_name
        or Path(executable_name).name != executable_name
        or executable_name in {".", ".."}
    ):
        raise resource_guard.GuardError("Kagemusha execution-copy name is invalid")
    parent.validate()
    directory_name = _execution_copy_name(staging_id)
    file_name = executable_name
    relative_file = f"{directory_name}/{file_name}"
    directory_descriptor = -1
    file_descriptor = -1
    created = False
    try:
        os.mkdir(directory_name, mode=0o700, dir_fd=parent.descriptor)
        created = True
        directory_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_DIRECTORY"):
            directory_flags |= os.O_DIRECTORY
        if hasattr(os, "O_NOFOLLOW"):
            directory_flags |= os.O_NOFOLLOW
        directory_descriptor = os.open(
            directory_name, directory_flags, dir_fd=parent.descriptor
        )
        directory_metadata = os.fstat(directory_descriptor)
        if (
            not stat.S_ISDIR(directory_metadata.st_mode)
            or directory_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(directory_metadata.st_mode) != 0o700
        ):
            raise resource_guard.GuardError(
                "Kagemusha execution-copy directory is unsafe"
            )

        write_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            write_flags |= os.O_NOFOLLOW
        writer = os.open(relative_file, write_flags, 0o600, dir_fd=parent.descriptor)
        try:
            offset = 0
            while offset < snapshot.size_bytes:
                chunk = os.pread(
                    snapshot.descriptor,
                    min(1024 * 1024, snapshot.size_bytes - offset),
                    offset,
                )
                if not chunk:
                    raise resource_guard.GuardError(
                        "admitted Kagemusha executable ended during private copy"
                    )
                resource_guard._write_all(writer, chunk)
                offset += len(chunk)
            os.fsync(writer)
            os.fchmod(writer, 0o500)
            os.fsync(writer)
        finally:
            os.close(writer)

        read_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            read_flags |= os.O_NOFOLLOW
        file_descriptor = os.open(
            relative_file, read_flags, dir_fd=parent.descriptor
        )
        copied_metadata, copied_sha256 = _hash_executable_descriptor(file_descriptor)
        path_metadata = os.stat(
            relative_file, dir_fd=parent.descriptor, follow_symlinks=False
        )
        if (
            _executable_stat_identity(copied_metadata)
            != _executable_stat_identity(path_metadata)
            or copied_metadata.st_size != snapshot.size_bytes
            or copied_sha256 != snapshot.sha256
            or stat.S_IMODE(copied_metadata.st_mode) != 0o500
        ):
            raise resource_guard.GuardError(
                "Kagemusha private execution copy does not match admission"
            )
        os.fchmod(directory_descriptor, 0o500)
        os.fsync(directory_descriptor)
        os.fsync(parent.descriptor)
        parent.validate()
        snapshot.execution_copy = ExecutionCopy(
            directory_name=directory_name,
            file_name=file_name,
            path=parent.path / directory_name / file_name,
            directory_descriptor=directory_descriptor,
            file_descriptor=file_descriptor,
            directory_device=directory_metadata.st_dev,
            directory_inode=directory_metadata.st_ino,
            file_device=copied_metadata.st_dev,
            file_inode=copied_metadata.st_ino,
            mode=copied_metadata.st_mode,
            size_bytes=copied_metadata.st_size,
            sha256=copied_sha256,
        )
        directory_descriptor = -1
        file_descriptor = -1
    except BaseException:
        for descriptor in (file_descriptor, directory_descriptor):
            if descriptor >= 0:
                os.close(descriptor)
        if created:
            try:
                os.chmod(directory_name, 0o700, dir_fd=parent.descriptor)
                shutil.rmtree(directory_name, dir_fd=parent.descriptor)
                os.fsync(parent.descriptor)
            except BaseException:
                pass
        raise


def _release_execution_copy(
    parent: PinnedOutputParent, snapshot: ExecutableSnapshot
) -> None:
    """Validate and unlock a private copy so journal cleanup can remove it."""

    copy = snapshot.execution_copy
    if copy is None:
        return
    _validate_execution_copy(snapshot)
    parent.validate(require_path=False)
    os.fchmod(copy.directory_descriptor, 0o700)
    os.fsync(copy.directory_descriptor)
    for descriptor_name in ("file_descriptor", "directory_descriptor"):
        descriptor = getattr(copy, descriptor_name)
        if descriptor >= 0:
            os.close(descriptor)
            setattr(copy, descriptor_name, -1)
    snapshot.execution_copy = None


def _physical_memory_bytes() -> int:
    """Return installed physical memory, or the absolute guard ceiling."""

    if sys.platform == "darwin":
        try:
            completed = subprocess.run(
                ["/usr/sbin/sysctl", "-n", "hw.memsize"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="ascii",
                timeout=5,
            )
            if completed.returncode == 0:
                value = int(completed.stdout.strip())
                if value > 0:
                    return value
        except (OSError, ValueError, subprocess.TimeoutExpired):
            pass
    try:
        pages = int(os.sysconf("SC_PHYS_PAGES"))
        page_size = int(os.sysconf("SC_PAGE_SIZE"))
        value = pages * page_size
        if value > 0:
            return value
    except (OSError, ValueError, TypeError):
        pass
    return ABSOLUTE_MAX_MEMORY_BYTES * 2


def _effective_memory_limit_bytes(requested_gib: float | None) -> int:
    """Apply the non-bypassable 256 MiB / half-physical-RAM ceiling."""

    physical_half = max(1, _physical_memory_bytes() // 2)
    ceiling = min(ABSOLUTE_MAX_MEMORY_BYTES, physical_half)
    if requested_gib is None:
        return ceiling
    if not math.isfinite(requested_gib) or requested_gib <= 0:
        raise resource_guard.GuardError("--max-memory-gib must be greater than zero")
    if requested_gib > ceiling / BYTES_PER_GIB:
        raise resource_guard.GuardError(
            "--max-memory-gib may lower but cannot raise the Kagemusha safety ceiling"
        )
    requested = int(requested_gib * BYTES_PER_GIB)
    if requested == 0:
        raise resource_guard.GuardError(
            "--max-memory-gib is too small to represent a positive byte limit"
        )
    return requested


def _is_kagemusha_heavy_process(row: resource_guard.ProcessRow) -> bool:
    """Identify a V4 generator that is not owned by this supervisor."""

    if row.uid != os.getuid() or row.pid == os.getpid():
        return False
    name = Path(row.command).name.lower()
    return name == "kagemusha_recursive_spend_v4_bundle" or name.startswith(
        "kagemusha_recu"
    )


def _reject_foreign_kagemusha_jobs() -> None:
    """Fail closed instead of racing an unowned candidate generator."""

    jobs = [
        row for row in resource_guard._process_rows() if _is_kagemusha_heavy_process(row)
    ]
    if jobs:
        first = min(jobs, key=lambda row: row.pid)
        raise resource_guard.GuardError(
            "pre-existing Kagemusha V4 generator is outside this guard "
            f"(pid={first.pid}, pgid={first.process_group_id})"
        )


def _prepare_report_directory(path: Path) -> tuple[Path, Path]:
    """Create one new owner-private resource evidence directory."""

    path.mkdir(parents=True, mode=0o700, exist_ok=False)
    os.chmod(path, 0o700)
    return path / "kagemusha_resource.jsonl", path / "kagemusha_resource_summary.json"


def _validate_generation_command(command: Sequence[str]) -> None:
    """Require a prebuilt bundle generator, never a compiler or shell wrapper."""

    executable = Path(command[0]).name
    if executable.endswith(".exe"):
        executable = executable[:-4]
    if executable != BUNDLE_EXECUTABLE:
        raise resource_guard.GuardError(
            "Kagemusha resource guard requires the prebuilt "
            "kagemusha_recursive_spend_v4_bundle executable; build it before "
            "entering the 256 MiB generation guard"
        )
    if len(command) < 2 or command[1] != "generate-candidate":
        raise resource_guard.GuardError(
            "Kagemusha resource guard supervises only generate-candidate"
        )
    if any(
        option in command
        for option in (STAGING_ID_OPTION, STAGING_NAME_OPTION, OUTPUT_PARENT_FD_OPTION)
    ):
        raise resource_guard.GuardError(
            "Kagemusha staging and output-parent options are reserved for the resource guard"
        )


def _required_option(command: Sequence[str], option: str) -> str:
    """Return one exact two-argument option from the bundle command."""

    positions = [index for index, value in enumerate(command) if value == option]
    if len(positions) != 1:
        raise resource_guard.GuardError(
            f"Kagemusha generation command requires exactly one {option}"
        )
    position = positions[0]
    if position + 1 >= len(command) or command[position + 1].startswith("--"):
        raise resource_guard.GuardError(
            f"Kagemusha generation command has no value for {option}"
        )
    return command[position + 1]


def _run_text_command(command: Sequence[str], description: str) -> str:
    """Run one fixed filesystem-inspection command with a short timeout."""

    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=5,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise resource_guard.GuardError(f"could not inspect {description}") from error
    if completed.returncode != 0:
        detail = completed.stderr.strip() or f"exit status {completed.returncode}"
        raise resource_guard.GuardError(
            f"could not inspect {description}: {detail}"
        )
    return completed.stdout


def _filesystem_type(path: Path) -> str:
    """Return the normalized filesystem type containing an output parent."""

    if sys.platform.startswith("linux"):
        stat_command = next(
            (
                candidate
                for candidate in ("/usr/bin/stat", "/bin/stat")
                if Path(candidate).is_file()
            ),
            None,
        )
        if stat_command is None:
            raise resource_guard.GuardError("filesystem stat utility is unavailable")
        output = _run_text_command(
            [stat_command, "--file-system", "--format=%T", "--", str(path)],
            "Kagemusha output filesystem type",
        )
        filesystem_type = output.strip().lower()
    elif sys.platform == "darwin":
        df_output = _run_text_command(
            ["/bin/df", "-P", str(path)], "Kagemusha output filesystem device"
        )
        rows = [line.split() for line in df_output.splitlines() if line.strip()]
        if len(rows) < 2 or not rows[-1]:
            raise resource_guard.GuardError(
                "Kagemusha output filesystem device is malformed"
            )
        device = rows[-1][0]
        mount_output = _run_text_command(
            ["/sbin/mount"], "Kagemusha output filesystem mount table"
        )
        prefix = f"{device} on "
        matching = [line for line in mount_output.splitlines() if line.startswith(prefix)]
        if len(matching) != 1 or " (" not in matching[0]:
            raise resource_guard.GuardError(
                "Kagemusha output filesystem has no unique mount-table entry"
            )
        filesystem_type = matching[0].split(" (", 1)[1].split(",", 1)[0]
        filesystem_type = filesystem_type.rstrip(")").strip().lower()
    else:
        raise resource_guard.GuardError(
            "Kagemusha output filesystem validation is unsupported on this platform"
        )
    if not filesystem_type:
        raise resource_guard.GuardError("Kagemusha output filesystem type is empty")
    return filesystem_type


def _valid_output_leaf(output_name: str) -> bool:
    """Return whether an output name is one safe, non-reserved path leaf."""

    return (
        bool(output_name)
        and output_name not in {".", ".."}
        and Path(output_name).name == output_name
        and "/" not in output_name
        and (os.altsep is None or os.altsep not in output_name)
        and not output_name.startswith((STAGING_PREFIX, JOURNAL_PREFIX))
    )


def _prepare_guarded_command(
    command: Sequence[str],
) -> tuple[list[str], PinnedOutputParent, str]:
    """Bind one unguessable staging prefix to this supervised invocation."""

    out_dir = Path(_required_option(command, "--out-dir"))
    output_name = out_dir.name
    if not _valid_output_leaf(output_name):
        raise resource_guard.GuardError(
            "Kagemusha output path must end in one directory name"
        )
    parent = out_dir.parent if out_dir.parent != Path("") else Path(".")
    try:
        parent = parent.resolve(strict=True)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha output parent is unavailable: {error}"
        ) from error
    if not parent.is_dir():
        raise resource_guard.GuardError("Kagemusha output parent is not a directory")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(parent, flags)
        opened = os.fstat(descriptor)
        current = os.stat(parent, follow_symlinks=False)
    except OSError as error:
        if "descriptor" in locals():
            os.close(descriptor)
        raise resource_guard.GuardError(
            f"Kagemusha output parent cannot be pinned: {error}"
        ) from error
    if (
        not stat.S_ISDIR(opened.st_mode)
        or not stat.S_ISDIR(current.st_mode)
        or (opened.st_dev, opened.st_ino) != (current.st_dev, current.st_ino)
    ):
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent changed while it was pinned"
        )
    try:
        filesystem_type = _filesystem_type(parent)
    except BaseException:
        os.close(descriptor)
        raise
    if filesystem_type not in DISK_BACKED_OUTPUT_FILESYSTEM_TYPES:
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent is not on an admitted disk-backed filesystem "
            f"({filesystem_type})"
        )
    try:
        filesystem = os.fstatvfs(descriptor)
    except OSError as error:
        os.close(descriptor)
        raise resource_guard.GuardError(
            f"Kagemusha output free space is unavailable: {error}"
        ) from error
    free_bytes = filesystem.f_bavail * filesystem.f_frsize
    if free_bytes < MINIMUM_OUTPUT_FREE_BYTES:
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent has less than 512 MiB available"
        )
    pinned = PinnedOutputParent(
        path=parent,
        descriptor=descriptor,
        device=opened.st_dev,
        inode=opened.st_ino,
        output_name=output_name,
        filesystem_type=filesystem_type,
        free_bytes_at_admission=free_bytes,
    )
    staging_id = secrets.token_hex(STAGING_ID_HEX_LENGTH // 2)
    staging_name = f"{STAGING_PREFIX}{staging_id}-work"
    return [
        *command,
        STAGING_ID_OPTION,
        staging_id,
        STAGING_NAME_OPTION,
        staging_name,
        OUTPUT_PARENT_FD_OPTION,
        str(descriptor),
    ], pinned, staging_id


def _cleanup_staging(parent: PinnedOutputParent, staging_id: str) -> int:
    """Remove only residue carrying this guard's unguessable staging id."""

    if (
        len(staging_id) != STAGING_ID_HEX_LENGTH
        or not staging_id.isascii()
        or any(byte not in "0123456789abcdef" for byte in staging_id)
    ):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    if not shutil.rmtree.avoids_symlink_attacks:
        raise resource_guard.GuardError(
            "this Python runtime cannot safely remove Kagemusha staging residue"
        )

    parent.validate(require_path=False)
    prefix = f"{STAGING_PREFIX}{staging_id}-"
    removed = 0
    with os.scandir(parent.descriptor) as entries:
        for entry in entries:
            if not entry.name.startswith(prefix):
                continue
            metadata = entry.stat(follow_symlinks=False)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != os.geteuid()
                or metadata.st_mode & 0o077 != 0
            ):
                raise resource_guard.GuardError(
                    "refusing to remove untrusted Kagemusha staging residue "
                    f"{entry.name}"
                )
            os.chmod(entry.name, 0o700, dir_fd=parent.descriptor)
            shutil.rmtree(entry.name, dir_fd=parent.descriptor)
            removed += 1

    parent.validate(require_path=False)
    with os.scandir(parent.descriptor) as entries:
        if any(entry.name.startswith(prefix) for entry in entries):
            raise resource_guard.GuardError(
                "Kagemusha staging residue remains after guarded cleanup"
            )
    return removed


def _create_staging_directory(
    parent: PinnedOutputParent, staging_id: str
) -> str:
    """Create the exact hidden work directory relative to the pinned parent fd."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    parent.validate()
    name = f"{STAGING_PREFIX}{staging_id}-work"
    os.mkdir(name, mode=0o700, dir_fd=parent.descriptor)
    metadata = os.stat(name, dir_fd=parent.descriptor, follow_symlinks=False)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise resource_guard.GuardError(
            "Kagemusha staging directory has unsafe metadata"
        )
    os.fsync(parent.descriptor)
    parent.validate()
    return name


def _valid_staging_id(staging_id: str) -> bool:
    """Return whether a staging id is canonical guard-generated lower hex."""

    return (
        len(staging_id) == STAGING_ID_HEX_LENGTH
        and staging_id.isascii()
        and all(byte in "0123456789abcdef" for byte in staging_id)
    )


def _journal_name(staging_id: str) -> str:
    """Return the reserved journal leaf for one validated staging id."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{JOURNAL_PREFIX}{staging_id}{JOURNAL_SUFFIX}"


def _journal_document(
    parent: PinnedOutputParent,
    staging_id: str,
    *,
    output_name: str | None = None,
) -> dict[str, object]:
    """Build the exact durable recovery record for one guarded invocation."""

    stored_output_name = parent.output_name if output_name is None else output_name
    if not _valid_output_leaf(stored_output_name):
        raise resource_guard.GuardError("Kagemusha journal output leaf is invalid")
    return {
        "execution_copy_name": _execution_copy_name(staging_id),
        "output_name": stored_output_name,
        "parent_device": parent.device,
        "parent_inode": parent.inode,
        "recovery_scope": "same_output_parent",
        "schema": JOURNAL_SCHEMA,
        "staging_id": staging_id,
        "staging_prefix": f"{STAGING_PREFIX}{staging_id}-",
    }


def _journal_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return stable metadata used to bind one opened journal path."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _create_run_journal(parent: PinnedOutputParent, staging_id: str) -> None:
    """Durably create the recovery marker before the generator can spawn."""

    parent.validate()
    name = _journal_name(staging_id)
    payload = resource_guard._canonical_json(_journal_document(parent, staging_id))
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = -1
    created_identity: tuple[int, int] | None = None
    try:
        descriptor = os.open(name, flags, 0o600, dir_fd=parent.descriptor)
        metadata = os.fstat(descriptor)
        created_identity = (metadata.st_dev, metadata.st_ino)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise resource_guard.GuardError(
                "Kagemusha run journal has unsafe metadata"
            )
        resource_guard._write_all(descriptor, payload)
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        parent.validate(require_path=False)
        os.fsync(parent.descriptor)
    except BaseException as error:
        if descriptor >= 0:
            os.close(descriptor)
            descriptor = -1
        cleanup_error: BaseException | None = None
        if created_identity is not None:
            try:
                current = os.stat(
                    name, dir_fd=parent.descriptor, follow_symlinks=False
                )
                if (current.st_dev, current.st_ino) != created_identity:
                    raise resource_guard.GuardError(
                        "partial Kagemusha run journal identity changed"
                    )
                os.unlink(name, dir_fd=parent.descriptor)
                os.fsync(parent.descriptor)
            except FileNotFoundError:
                pass
            except BaseException as failure:
                cleanup_error = failure
        if cleanup_error is not None:
            raise resource_guard.GuardError(
                "could not remove a partial Kagemusha run journal"
            ) from cleanup_error
        raise error


def _read_run_journal(
    parent: PinnedOutputParent, name: str
) -> tuple[str, dict[str, object]]:
    """Read and strictly validate one marker relative to the pinned parent."""

    if not name.startswith(JOURNAL_PREFIX) or not name.endswith(JOURNAL_SUFFIX):
        raise resource_guard.GuardError("Kagemusha run journal name is malformed")
    staging_id = name[len(JOURNAL_PREFIX) : -len(JOURNAL_SUFFIX)]
    if name != _journal_name(staging_id):
        raise resource_guard.GuardError("Kagemusha run journal id is malformed")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent.descriptor)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or opened.st_nlink != 1
            or stat.S_IMODE(opened.st_mode) != 0o600
            or opened.st_size <= 0
            or opened.st_size > MAX_JOURNAL_BYTES
        ):
            raise resource_guard.GuardError(
                "Kagemusha run journal has unsafe metadata"
            )
        payload = bytearray()
        while len(payload) <= MAX_JOURNAL_BYTES:
            chunk = os.read(descriptor, MAX_JOURNAL_BYTES + 1 - len(payload))
            if not chunk:
                break
            payload.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    current = os.stat(name, dir_fd=parent.descriptor, follow_symlinks=False)
    if not (
        _journal_stat_identity(opened)
        == _journal_stat_identity(after)
        == _journal_stat_identity(current)
        and len(payload) == opened.st_size
    ):
        raise resource_guard.GuardError("Kagemusha run journal changed while read")
    try:
        document = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise resource_guard.GuardError("Kagemusha run journal is invalid JSON") from error
    if not isinstance(document, dict):
        raise resource_guard.GuardError("Kagemusha run journal is not an object")
    stored_output_name = document.get("output_name")
    if not isinstance(stored_output_name, str) or not _valid_output_leaf(
        stored_output_name
    ):
        raise resource_guard.GuardError(
            "Kagemusha run journal has an unsafe output leaf"
        )
    expected = _journal_document(
        parent, staging_id, output_name=stored_output_name
    )
    if document != expected or bytes(payload) != resource_guard._canonical_json(expected):
        raise resource_guard.GuardError("Kagemusha run journal is not canonical or bound")
    return staging_id, document


def _remove_run_journal(parent: PinnedOutputParent, staging_id: str) -> None:
    """Remove one already validated journal and durably sync its parent."""

    name = _journal_name(staging_id)
    recovered_id, _document = _read_run_journal(parent, name)
    if recovered_id != staging_id:
        raise resource_guard.GuardError("Kagemusha run journal id changed")
    os.unlink(name, dir_fd=parent.descriptor)
    os.fsync(parent.descriptor)
    parent.validate(require_path=False)


def _run_journal_exists(parent: PinnedOutputParent, staging_id: str) -> bool:
    """Return whether the exact marker name still has a directory entry."""

    parent.validate(require_path=False)
    try:
        os.stat(
            _journal_name(staging_id),
            dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        return False
    return True


def _output_leaf_exists(parent: PinnedOutputParent, output_name: str) -> bool:
    """Return whether one validated output leaf is visible in the pinned parent."""

    if not _valid_output_leaf(output_name):
        raise resource_guard.GuardError("Kagemusha output leaf is invalid")
    try:
        os.stat(output_name, dir_fd=parent.descriptor, follow_symlinks=False)
    except FileNotFoundError:
        return False
    return True


def _cleanup_guarded_run(
    parent: PinnedOutputParent,
    staging_id: str,
    *,
    publication_confirmed: bool = False,
) -> int:
    """Clean staging, preserving recovery evidence for uncertain publication."""

    _recovered_id, document = _read_run_journal(parent, _journal_name(staging_id))
    output_name = document["output_name"]
    if not isinstance(output_name, str):  # guarded by _read_run_journal
        raise resource_guard.GuardError("Kagemusha journal output leaf is invalid")
    removed = _cleanup_staging(parent, staging_id)
    if _output_leaf_exists(parent, output_name) and not publication_confirmed:
        raise resource_guard.GuardError(
            "Kagemusha output became visible without confirmed publication; "
            "the recovery journal was retained for reconciliation"
        )
    _remove_run_journal(parent, staging_id)
    return removed


def _recover_stale_runs(parent: PinnedOutputParent) -> int:
    """Recover marker-bound residue in this same parent while holding the locks."""

    parent.validate()
    with os.scandir(parent.descriptor) as entries:
        names = sorted(
            entry.name for entry in entries if entry.name.startswith(JOURNAL_PREFIX)
        )
    if len(names) > MAX_RECOVERABLE_JOURNALS:
        raise resource_guard.GuardError("too many stale Kagemusha run journals")
    removed = 0
    for name in names:
        staging_id, document = _read_run_journal(parent, name)
        removed += _cleanup_staging(parent, staging_id)
        output_name = document["output_name"]
        if not isinstance(output_name, str):  # guarded by _read_run_journal
            raise resource_guard.GuardError(
                "Kagemusha journal output leaf is invalid"
            )
        if _output_leaf_exists(parent, output_name):
            raise resource_guard.GuardError(
                "stale Kagemusha journal records an already-visible output; "
                "manual reconciliation is required"
            )
        _remove_run_journal(parent, staging_id)
    with os.scandir(parent.descriptor) as entries:
        if any(entry.name.startswith(STAGING_PREFIX) for entry in entries):
            raise resource_guard.GuardError(
                "unjournaled Kagemusha staging residue exists in the output parent"
            )
    parent.validate()
    return removed


def _run_candidate_session_wrapper(argv: Sequence[str]) -> int:
    """Own a pinned-executable body and kill it when its supervisor disappears."""

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--lifeline-fd", required=True, type=int)
    parser.add_argument("--control-fd", required=True, type=int)
    parser.add_argument("--auth-fd", required=True, type=int)
    parser.add_argument("--executable-fd", required=True, type=int)
    parser.add_argument("--execution-path", required=True)
    parser.add_argument("--held-lock-fd", action="append", default=[], type=int)
    parser.add_argument("--child-directory-fd", action="append", default=[], type=int)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise resource_guard.GuardError("candidate session command is empty")
    descriptors = (
        args.lifeline_fd,
        args.control_fd,
        args.auth_fd,
        args.executable_fd,
        *args.held_lock_fd,
        *args.child_directory_fd,
    )
    if len(set(descriptors)) != len(descriptors):
        raise resource_guard.GuardError(
            "candidate session control descriptors overlap"
        )
    resource_guard._require_pipe_descriptor(args.lifeline_fd, "lifeline")
    resource_guard._require_pipe_descriptor(args.control_fd, "control")
    resource_guard._require_pipe_descriptor(args.auth_fd, "authorization")
    for descriptor in args.held_lock_fd:
        metadata = os.fstat(descriptor)
        if (
            descriptor < 3
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise resource_guard.GuardError("held lock descriptor is invalid")
    for descriptor in args.child_directory_fd:
        metadata = os.fstat(descriptor)
        if descriptor < 3 or not stat.S_ISDIR(metadata.st_mode):
            raise resource_guard.GuardError("child directory descriptor is invalid")
    executable_metadata = os.fstat(args.executable_fd)
    if args.executable_fd < 3:
        raise resource_guard.GuardError("pinned executable descriptor is invalid")
    _validate_executable_metadata(executable_metadata)
    if command[0] != args.execution_path:
        raise resource_guard.GuardError(
            "candidate session did not target its pinned executable descriptor"
        )
    execution_path_metadata = os.stat(args.execution_path, follow_symlinks=True)
    if _executable_stat_identity(execution_path_metadata) != _executable_stat_identity(
        executable_metadata
    ):
        raise resource_guard.GuardError(
            "candidate execution path does not identify its pinned bytes"
        )
    if os.environ.get(resource_guard.RESOURCE_GUARD_AUTH_FD_ENV) != str(
        args.auth_fd
    ):
        raise resource_guard.GuardError(
            "authorization descriptor environment is inconsistent"
        )

    received_signal = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, receive_signal)

    child: subprocess.Popen[bytes] | None = None
    try:
        if resource_guard._lifeline_closed(args.lifeline_fd, 0):
            return 1
        child = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(
                args.auth_fd,
                args.executable_fd,
                *args.child_directory_fd,
            ),
            start_new_session=True,
            env=os.environ.copy(),
        )
        process_group_id = child.pid
        resource_guard._close_descriptor(args.auth_fd)
        args.auth_fd = -1
        if process_group_id <= 1 or process_group_id == os.getpgrp():
            raise resource_guard.GuardError(
                "candidate body did not enter its own process group"
            )
        resource_guard._write_wrapper_control(
            args.control_fd, f"READY {process_group_id}"
        )

        completed: tuple[int, int] | None = None
        while completed is None:
            completed = resource_guard._wait4_nonblocking(child)
            if completed is not None:
                break
            if received_signal or resource_guard._lifeline_closed(
                args.lifeline_fd, 0.05
            ):
                resource_guard._terminate_owned_group(child, process_group_id)
                return 1
        if completed is None:
            raise resource_guard.GuardError(
                "candidate session lost the body return code"
            )
        returncode, kernel_peak_rss_bytes = completed
        lingering = resource_guard._process_group_exists(process_group_id)
        if lingering:
            resource_guard._terminate_owned_group(child, process_group_id)
        resource_guard._write_wrapper_control(
            args.control_fd,
            f"EXIT {returncode} {1 if lingering else 0} {kernel_peak_rss_bytes}",
        )
        return 1 if lingering else resource_guard._exit_status(returncode)
    except BaseException as error:
        if child is not None:
            try:
                resource_guard._terminate_owned_group(child, child.pid)
            except BaseException:
                pass
        try:
            resource_guard._write_wrapper_control(args.control_fd, "ERROR")
        except BaseException:
            pass
        print(f"candidate session wrapper failed: {error}", file=sys.stderr)
        return 1
    finally:
        for descriptor in descriptors:
            resource_guard._close_descriptor(descriptor)


def _spawn_pinned_guarded_session(
    command: Sequence[str],
    environment: dict[str, str],
    held_lock_descriptors: Sequence[int],
    child_directory_descriptors: Sequence[int],
    executable_snapshot: ExecutableSnapshot,
) -> resource_guard.GuardedSession:
    """Spawn a lifeline wrapper that inherits the admitted executable fd."""

    _validate_executable_unchanged(executable_snapshot)
    if not command or command[0] != executable_snapshot.execution_path():
        raise resource_guard.GuardError(
            "guarded command must execute the admitted descriptor path"
        )
    execution_descriptor = executable_snapshot.execution_descriptor()
    execution_path = executable_snapshot.execution_path()
    auth_reader, auth_writer = resource_guard._pipe()
    lifeline_reader, lifeline_writer = resource_guard._pipe()
    control_reader, control_writer = resource_guard._pipe()
    token = secrets.token_hex(32)
    child_environment = environment.copy()
    child_environment.pop("SUMERAGI_TLAPS_SUPERVISOR_PID", None)
    child_environment[resource_guard.RESOURCE_GUARD_AUTH_FD_ENV] = str(auth_reader)
    child_environment[resource_guard.RESOURCE_GUARD_AUTH_TOKEN_ENV] = token
    wrapper_command = [
        sys.executable,
        str(Path(__file__).resolve()),
        CANDIDATE_SESSION_WRAPPER_FLAG,
        "--lifeline-fd",
        str(lifeline_reader),
        "--control-fd",
        str(control_writer),
        "--auth-fd",
        str(auth_reader),
        "--executable-fd",
        str(execution_descriptor),
        "--execution-path",
        execution_path,
    ]
    for descriptor in held_lock_descriptors:
        wrapper_command.extend(("--held-lock-fd", str(descriptor)))
    for descriptor in child_directory_descriptors:
        wrapper_command.extend(("--child-directory-fd", str(descriptor)))
    wrapper_command.extend(("--", *command))
    wrapper: subprocess.Popen[bytes] | None = None
    control: resource_guard.SessionControl | None = None
    try:
        resource_guard._write_all(
            auth_writer,
            f"{resource_guard.RESOURCE_GUARD_AUTH_MAGIC}:{token}\n".encode("ascii"),
        )
        resource_guard._close_descriptor(auth_writer)
        auth_writer = -1
        wrapper = subprocess.Popen(
            wrapper_command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(
                auth_reader,
                lifeline_reader,
                control_writer,
                execution_descriptor,
                *held_lock_descriptors,
                *child_directory_descriptors,
            ),
            start_new_session=True,
            env=child_environment,
        )
        for descriptor in (auth_reader, lifeline_reader, control_writer):
            resource_guard._close_descriptor(descriptor)
        auth_reader = -1
        lifeline_reader = -1
        control_writer = -1
        control = resource_guard.SessionControl(control_reader)
        control_reader = -1
        ready = control.read_line(
            timeout=resource_guard.SESSION_READY_TIMEOUT_SECONDS,
            description="candidate lifeline wrapper readiness",
        )
        fields = ready.split()
        if len(fields) != 2 or fields[0] != "READY" or not fields[1].isdigit():
            raise resource_guard.GuardError(
                "candidate lifeline wrapper emitted invalid readiness"
            )
        process_group_id = int(fields[1])
        if process_group_id <= 1 or process_group_id == wrapper.pid:
            raise resource_guard.GuardError(
                "candidate lifeline wrapper reported an invalid body process group"
            )
        session = resource_guard.GuardedSession(
            wrapper, process_group_id, lifeline_writer, control
        )
        lifeline_writer = -1
        control = None
        return session
    except BaseException:
        resource_guard._close_descriptor(lifeline_writer)
        lifeline_writer = -1
        if wrapper is not None:
            try:
                wrapper.wait(timeout=resource_guard.TERM_GRACE_SECONDS * 2 + 1)
            except subprocess.TimeoutExpired:
                try:
                    resource_guard._terminate_owned_group(wrapper, wrapper.pid)
                except BaseException:
                    pass
        if control is not None:
            control.close()
        raise
    finally:
        for descriptor in (
            auth_reader,
            auth_writer,
            lifeline_reader,
            lifeline_writer,
            control_reader,
            control_writer,
        ):
            if descriptor >= 0:
                resource_guard._close_descriptor(descriptor)


def _run_guarded_with_pinned_executable(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    **guard_options: object,
) -> int:
    """Run the resource guard with a session spawner that inherits the exec fd.

    Callers must put ``executable_snapshot.execution_path()`` in ``command[0]``
    and retain/close the snapshot around this call.
    """

    original_spawner = resource_guard._spawn_guarded_session

    def spawn(
        child_command: Sequence[str],
        environment: dict[str, str],
        held_lock_descriptors: Sequence[int],
        child_directory_descriptors: Sequence[int],
    ) -> resource_guard.GuardedSession:
        return _spawn_pinned_guarded_session(
            child_command,
            environment,
            held_lock_descriptors,
            child_directory_descriptors,
            executable_snapshot,
        )

    resource_guard._spawn_guarded_session = spawn
    try:
        return resource_guard._run_guarded(command, **guard_options)
    finally:
        resource_guard._spawn_guarded_session = original_spawner


def _run_authenticated_bundle_command(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    *,
    held_lock_descriptors: Sequence[int] = (),
    child_directory_descriptors: Sequence[int] = (),
) -> None:
    """Run one short bundle operation under a supervisor-death lifeline."""

    if not command or command[0] != executable_snapshot.execution_path():
        raise resource_guard.GuardError(
            "bundle control command must execute the admitted descriptor path"
        )
    environment = os.environ.copy()
    session = _spawn_pinned_guarded_session(
        command,
        environment,
        held_lock_descriptors,
        child_directory_descriptors,
        executable_snapshot,
    )
    received_signal = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    watched_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
    previous_handlers = {
        signum: signal.getsignal(signum) for signum in watched_signals
    }
    for signum in watched_signals:
        signal.signal(signum, receive_signal)
    interrupted = 0
    try:
        deadline = time.monotonic() + 300
        while session.wrapper.poll() is None and time.monotonic() < deadline:
            if received_signal:
                interrupted = received_signal
                resource_guard._terminate_owned_group(
                    session.wrapper, session.process_group_id
                )
                break
            time.sleep(0.05)
        if interrupted:
            raise resource_guard.GuardError(
                f"Kagemusha publication interrupted by signal {interrupted}"
            )
        if session.wrapper.poll() is None:
            resource_guard._terminate_owned_group(
                session.wrapper, session.process_group_id
            )
            raise resource_guard.GuardError(
                "timed out publishing the validated Kagemusha candidate"
            )
        wrapper_exit = session.control.read_line(
            timeout=resource_guard.PROCESS_INSPECTION_TIMEOUT_SECONDS,
            description="candidate publisher exit status",
        )
        fields = wrapper_exit.split()
        if (
            len(fields) != 4
            or fields[0] != "EXIT"
            or fields[2] not in {"0", "1"}
            or not fields[3].isdigit()
        ):
            raise resource_guard.GuardError(
                "candidate publisher wrapper emitted invalid exit status"
            )
        try:
            returncode = int(fields[1])
        except ValueError as error:
            raise resource_guard.GuardError(
                "candidate publisher emitted a non-integer status"
            ) from error
        if fields[2] == "1":
            raise resource_guard.GuardError(
                "candidate publisher left a lingering process group"
            )
        if returncode != 0:
            raise resource_guard.GuardError(
                "validated Kagemusha candidate publication failed with status "
                f"{returncode}"
            )
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)
        session.close()
        if interrupted:
            prior = previous_handlers[interrupted]
            if callable(prior):
                prior(interrupted, None)


def _publish_staged_candidate(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    output_parent: PinnedOutputParent,
    staging_id: str,
    held_lock_descriptors: Sequence[int] = (),
) -> int:
    """Authenticate and atomically publish staging only after the guard verdict."""

    _validate_executable_unchanged(executable_snapshot)
    output_parent.validate()
    try:
        os.stat(
            output_parent.output_name,
            dir_fd=output_parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        pass
    else:
        raise resource_guard.GuardError(
            "Kagemusha candidate output appeared before guarded publication"
        )
    publish_command = [
        executable_snapshot.execution_path(),
        "publish-staged-candidate",
        "--out-dir",
        _required_option(command, "--out-dir"),
        STAGING_ID_OPTION,
        staging_id,
        STAGING_NAME_OPTION,
        f"{STAGING_PREFIX}{staging_id}-work",
        OUTPUT_PARENT_FD_OPTION,
        str(output_parent.descriptor),
        "--source-commit",
        _required_option(command, "--source-commit"),
        "--source-tree-sha256",
        _required_option(command, "--source-tree-sha256"),
    ]
    _run_authenticated_bundle_command(
        publish_command,
        executable_snapshot,
        held_lock_descriptors=held_lock_descriptors,
        child_directory_descriptors=(output_parent.descriptor,),
    )
    _validate_executable_unchanged(executable_snapshot)
    output_parent.validate()
    try:
        published = os.stat(
            output_parent.output_name,
            dir_fd=output_parent.descriptor,
            follow_symlinks=False,
        )
    except OSError as error:
        raise resource_guard.GuardError(
            "Kagemusha publisher did not create the requested candidate directory"
        ) from error
    if (
        not stat.S_ISDIR(published.st_mode)
        or published.st_uid != os.geteuid()
        or published.st_mode & 0o077 != 0
    ):
        raise resource_guard.GuardError(
            "published Kagemusha candidate directory is untrusted"
        )
    return 1


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resource-report", required=True, type=Path)
    parser.add_argument("--max-memory-gib", type=float)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Acquire the global lock and supervise exactly one generation command."""

    args = _parser().parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        print("Kagemusha resource guard requires a command after --", file=sys.stderr)
        return 2
    output_parent: PinnedOutputParent | None = None
    executable_snapshot: ExecutableSnapshot | None = None
    try:
        _validate_generation_command(command)
        executable_snapshot = _snapshot_executable(command[0], BUNDLE_EXECUTABLE)
        command[0] = str(executable_snapshot.path)
        guarded_command, output_parent, staging_id = _prepare_guarded_command(command)
        memory_limit = _effective_memory_limit_bytes(args.max_memory_gib)
        jsonl_path, summary_path = _prepare_report_directory(args.resource_report)
        with resource_guard._host_lock(
            resource_guard.HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
        ) as heavy_lock:
            with resource_guard._host_lock(
                LOCK_PATH, description="Kagemusha generator"
            ) as kagemusha_lock:
                _reject_foreign_kagemusha_jobs()
                recovered_staging = _recover_stale_runs(output_parent)
                _create_run_journal(output_parent, staging_id)
                publication_confirmed = False

                def publish_candidate() -> int:
                    nonlocal publication_confirmed
                    result = _publish_staged_candidate(
                        command,
                        executable_snapshot,
                        output_parent,
                        staging_id,
                        held_lock_descriptors=(heavy_lock, kagemusha_lock),
                    )
                    publication_confirmed = True
                    return result

                def cleanup_candidate() -> int:
                    _release_execution_copy(output_parent, executable_snapshot)
                    return _cleanup_guarded_run(
                        output_parent,
                        staging_id,
                        publication_confirmed=publication_confirmed,
                    )

                try:
                    _prepare_execution_copy(
                        output_parent, executable_snapshot, staging_id
                    )
                    guarded_command[0] = executable_snapshot.execution_path()
                    _create_staging_directory(output_parent, staging_id)
                    return _run_guarded_with_pinned_executable(
                        guarded_command,
                        executable_snapshot,
                        report_path=jsonl_path,
                        summary_path=summary_path,
                        memory_limit_bytes=memory_limit,
                        held_lock_descriptors=(heavy_lock, kagemusha_lock),
                        child_directory_descriptors=(output_parent.descriptor,),
                        sample_interval_seconds=SAMPLE_INTERVAL_SECONDS,
                        post_run_cleanup=cleanup_candidate,
                        post_run_validation=lambda: _validate_executable_unchanged(
                            executable_snapshot
                        ),
                        post_success_finalize=publish_candidate,
                        report_context={
                            "executable_identity": (
                                executable_snapshot.report_context()
                            ),
                            "output_parent": output_parent.report_context(),
                            "same_parent_recovered_staging_directories": (
                                recovered_staging
                            ),
                            "staging_id": staging_id,
                        },
                    )
                except BaseException:
                    if _run_journal_exists(output_parent, staging_id):
                        cleanup_candidate()
                    raise
    except resource_guard.LockUnavailable as error:
        print(f"Kagemusha resource guard refused to start: {error}", file=sys.stderr)
        return resource_guard.LOCK_UNAVAILABLE_EXIT_CODE
    except (resource_guard.GuardError, OSError) as error:
        print(f"Kagemusha resource guard failed closed: {error}", file=sys.stderr)
        return 1
    finally:
        if output_parent is not None:
            output_parent.close()
        if executable_snapshot is not None:
            executable_snapshot.close()


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == CANDIDATE_SESSION_WRAPPER_FLAG:
        raise SystemExit(_run_candidate_session_wrapper(sys.argv[2:]))
    raise SystemExit(main())
