#!/usr/bin/env python3
"""Run the non-shipping Kagemusha compact-generation memory benchmark safely."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import secrets
import stat
import subprocess
import sys
from typing import Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import run_kagemusha_v4_generation as candidate_guard


resource_guard = candidate_guard.resource_guard
BENCHMARK_EXECUTABLE = "kagemusha_recursive_spend_v4_memory_benchmark"
BENCHMARK_SUBCOMMAND = "measure-compact-k17"
K17_SHAPE_PROBE_SUBCOMMAND = "probe-compact-k17-shape"
BENCHMARK_SUBCOMMANDS = frozenset(
    {BENCHMARK_SUBCOMMAND, K17_SHAPE_PROBE_SUBCOMMAND}
)
SCRATCH_PREFIX = ".kagemusha-v4-benchmark-scratch-"
MINIMUM_SCRATCH_FREE_BYTES = candidate_guard.MINIMUM_OUTPUT_FREE_BYTES
DISK_BACKED_FILESYSTEM_TYPES = frozenset(
    {
        "apfs",
        "btrfs",
        "ext2/ext3",
        "ext4",
        "fuseblk",
        "hfs",
        "hfsplus",
        "ufs",
        "xfs",
        "zfs",
    }
)


def _benchmark_memory_enforcement_mode(platform: str | None = None) -> str:
    """Match production footprint enforcement on Darwin and retain RSS elsewhere."""

    selected_platform = sys.platform if platform is None else platform
    if selected_platform == "darwin":
        return resource_guard.MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT
    return resource_guard.MEMORY_ENFORCEMENT_PROCESS_TREE_RSS


@dataclass(frozen=True)
class ScratchDirectory:
    """One owner-private, disk-backed directory used as the child TMPDIR."""

    path: Path
    name: str
    parent_path: Path
    parent_descriptor: int
    parent_device: int
    parent_inode: int
    run_descriptor: int
    run_device: int
    run_inode: int
    filesystem_type: str
    free_bytes_at_admission: int

    def report_context(self) -> dict[str, object]:
        """Return stable JSON evidence for the admitted scratch location."""

        return {
            "ambient_temp_environment_ignored": True,
            "canonical_parent": str(self.parent_path),
            "canonical_run_directory": str(self.path),
            "filesystem_type": self.filesystem_type,
            "free_bytes_at_admission": self.free_bytes_at_admission,
            "minimum_free_bytes": MINIMUM_SCRATCH_FREE_BYTES,
            "parent_device": self.parent_device,
            "parent_inode": self.parent_inode,
            "run_device": self.run_device,
            "run_inode": self.run_inode,
        }


def _run_text_command(command: Sequence[str], description: str) -> str:
    """Run one fixed system inspection command and return its text output."""

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
    """Return a normalized filesystem type for a scratch-parent path."""

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
            "scratch filesystem type",
        )
        filesystem_type = output.strip().lower()
    elif sys.platform == "darwin":
        df_output = _run_text_command(
            ["/bin/df", "-P", str(path)], "scratch filesystem device"
        )
        rows = [line.split() for line in df_output.splitlines() if line.strip()]
        if len(rows) < 2 or not rows[-1]:
            raise resource_guard.GuardError("scratch filesystem device is malformed")
        device = rows[-1][0]
        mount_output = _run_text_command(
            ["/sbin/mount"], "scratch filesystem mount table"
        )
        prefix = f"{device} on "
        matching = [line for line in mount_output.splitlines() if line.startswith(prefix)]
        if len(matching) != 1 or " (" not in matching[0]:
            raise resource_guard.GuardError(
                "scratch filesystem device has no unique mount-table entry"
            )
        filesystem_type = matching[0].split(" (", 1)[1].split(",", 1)[0].rstrip(")")
        filesystem_type = filesystem_type.strip().lower()
    else:
        raise resource_guard.GuardError(
            "scratch filesystem validation is unsupported on this platform"
        )
    if not filesystem_type:
        raise resource_guard.GuardError("scratch filesystem type is empty")
    return filesystem_type


def _prepare_scratch_directory(parent: Path) -> ScratchDirectory:
    """Create an owner-private run directory on an explicitly disk-backed parent."""

    parent_descriptor = -1
    run_descriptor = -1
    run_name = ""
    try:
        resolved = parent.resolve(strict=True)
        metadata = resolved.stat()
    except OSError as error:
        raise resource_guard.GuardError(
            f"benchmark scratch parent is unavailable: {error}"
        ) from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077 != 0
    ):
        raise resource_guard.GuardError(
            "benchmark scratch parent must be an owner-private directory"
        )
    open_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        parent_descriptor = os.open(resolved, open_flags)
        opened_parent = os.fstat(parent_descriptor)
    except OSError as error:
        if parent_descriptor >= 0:
            os.close(parent_descriptor)
        raise resource_guard.GuardError(
            f"could not pin benchmark scratch parent: {error}"
        ) from error
    if (
        not stat.S_ISDIR(opened_parent.st_mode)
        or opened_parent.st_uid != os.geteuid()
        or opened_parent.st_mode & 0o077 != 0
        or (opened_parent.st_dev, opened_parent.st_ino)
        != (metadata.st_dev, metadata.st_ino)
    ):
        os.close(parent_descriptor)
        raise resource_guard.GuardError(
            "benchmark scratch parent changed while it was opened"
        )
    filesystem_type = _filesystem_type(resolved)
    if filesystem_type not in DISK_BACKED_FILESYSTEM_TYPES:
        os.close(parent_descriptor)
        raise resource_guard.GuardError(
            "benchmark scratch parent is not on an admitted disk-backed filesystem "
            f"({filesystem_type})"
        )
    filesystem = os.fstatvfs(parent_descriptor)
    free_bytes = filesystem.f_bavail * filesystem.f_frsize
    if free_bytes < MINIMUM_SCRATCH_FREE_BYTES:
        os.close(parent_descriptor)
        raise resource_guard.GuardError(
            "benchmark scratch parent has less than 16 GiB available"
        )
    run_name = f"{SCRATCH_PREFIX}{secrets.token_hex(16)}"
    run_path = resolved / run_name
    try:
        os.mkdir(run_name, mode=0o700, dir_fd=parent_descriptor)
        run_descriptor = os.open(run_name, open_flags, dir_fd=parent_descriptor)
        run_metadata = os.fstat(run_descriptor)
        current = os.stat(run_name, dir_fd=parent_descriptor, follow_symlinks=False)
    except OSError as error:
        if run_descriptor >= 0:
            os.close(run_descriptor)
        try:
            if run_name:
                os.rmdir(run_name, dir_fd=parent_descriptor)
        except OSError:
            pass
        os.close(parent_descriptor)
        raise resource_guard.GuardError(
            f"could not create benchmark scratch directory: {error}"
        ) from error
    if (
        not stat.S_ISDIR(run_metadata.st_mode)
        or run_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(run_metadata.st_mode) != 0o700
        or (current.st_dev, current.st_ino)
        != (run_metadata.st_dev, run_metadata.st_ino)
        or (os.fstat(parent_descriptor).st_dev, os.fstat(parent_descriptor).st_ino)
        != (opened_parent.st_dev, opened_parent.st_ino)
    ):
        os.close(run_descriptor)
        try:
            if (current.st_dev, current.st_ino) == (
                run_metadata.st_dev,
                run_metadata.st_ino,
            ):
                os.rmdir(run_name, dir_fd=parent_descriptor)
        finally:
            os.close(parent_descriptor)
        raise resource_guard.GuardError(
            "benchmark scratch identity changed while it was being prepared"
        )
    return ScratchDirectory(
        path=run_path,
        name=run_name,
        parent_path=resolved,
        parent_descriptor=parent_descriptor,
        parent_device=opened_parent.st_dev,
        parent_inode=opened_parent.st_ino,
        run_descriptor=run_descriptor,
        run_device=run_metadata.st_dev,
        run_inode=run_metadata.st_ino,
        filesystem_type=filesystem_type,
        free_bytes_at_admission=free_bytes,
    )


def _cleanup_scratch_directory(scratch: ScratchDirectory) -> int:
    """Remove the exact empty scratch directory after validating its identity."""

    try:
        parent = os.fstat(scratch.parent_descriptor)
        run = os.fstat(scratch.run_descriptor)
        current = os.stat(
            scratch.name,
            dir_fd=scratch.parent_descriptor,
            follow_symlinks=False,
        )
        if (parent.st_dev, parent.st_ino) != (
            scratch.parent_device,
            scratch.parent_inode,
        ):
            raise resource_guard.GuardError(
                "benchmark scratch parent identity changed"
            )
        if (
            not stat.S_ISDIR(run.st_mode)
            or run.st_uid != os.geteuid()
            or stat.S_IMODE(run.st_mode) != 0o700
            or (run.st_dev, run.st_ino) != (scratch.run_device, scratch.run_inode)
            or (current.st_dev, current.st_ino)
            != (scratch.run_device, scratch.run_inode)
        ):
            raise resource_guard.GuardError(
                "benchmark scratch directory identity changed"
            )
        if os.listdir(scratch.run_descriptor):
            raise resource_guard.GuardError(
                "benchmark scratch directory is not empty after the child exited"
            )
        os.rmdir(scratch.name, dir_fd=scratch.parent_descriptor)
        os.fsync(scratch.parent_descriptor)
        return 1
    finally:
        for descriptor in (scratch.run_descriptor, scratch.parent_descriptor):
            try:
                os.close(descriptor)
            except OSError:
                pass


def _validate_benchmark_command(command: Sequence[str]) -> None:
    """Admit only the prebuilt benchmark's two exact non-shipping operations."""

    executable = Path(command[0]).name
    if executable.endswith(".exe"):
        executable = executable[:-4]
    if executable != BENCHMARK_EXECUTABLE:
        raise resource_guard.GuardError(
            "Kagemusha benchmark guard requires the prebuilt "
            f"{BENCHMARK_EXECUTABLE} executable"
        )
    if len(command) != 2 or command[1] not in BENCHMARK_SUBCOMMANDS:
        admitted = "|".join(sorted(BENCHMARK_SUBCOMMANDS))
        raise resource_guard.GuardError(
            "Kagemusha benchmark guard supervises only the exact "
            f"<{admitted}> operations with no extra arguments"
        )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resource-report", required=True, type=Path)
    parser.add_argument("--scratch-parent", required=True, type=Path)
    parser.add_argument("--max-memory-gib", type=float)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Acquire the shared locks and supervise one anonymous-file benchmark."""

    args = _parser().parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        print("Kagemusha benchmark guard requires a command after --", file=sys.stderr)
        return 2
    executable_snapshot: candidate_guard.ExecutableSnapshot | None = None
    try:
        _validate_benchmark_command(command)
        executable_snapshot = candidate_guard._snapshot_executable(
            command[0], BENCHMARK_EXECUTABLE
        )
        memory_limit = candidate_guard._effective_memory_limit_bytes(
            args.max_memory_gib
        )
        jsonl_path, summary_path = candidate_guard._prepare_report_directory(
            args.resource_report
        )
        with resource_guard._host_lock(
            resource_guard.HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
        ) as heavy_lock:
            with resource_guard._host_lock(
                candidate_guard.LOCK_PATH, description="Kagemusha generator"
            ) as kagemusha_lock:
                candidate_guard._reject_foreign_kagemusha_jobs()
                scratch = _prepare_scratch_directory(args.scratch_parent)
                cleanup_started = False
                execution_parent: candidate_guard.PinnedOutputParent | None = None
                execution_staging_id = secrets.token_hex(
                    candidate_guard.STAGING_ID_HEX_LENGTH // 2
                )

                def cleanup_scratch() -> int:
                    nonlocal cleanup_started
                    if cleanup_started:
                        return 0
                    cleanup_started = True
                    removed = 0
                    if executable_snapshot.execution_copy is not None:
                        if execution_parent is None:
                            raise resource_guard.GuardError(
                                "benchmark execution-copy parent is unavailable"
                            )
                        candidate_guard._release_execution_copy(
                            execution_parent, executable_snapshot
                        )
                        removed += candidate_guard._cleanup_staging(
                            execution_parent, execution_staging_id
                        )
                    if execution_parent is not None:
                        execution_parent.close()
                    removed += _cleanup_scratch_directory(scratch)
                    return removed

                try:
                    if sys.platform == "darwin":
                        execution_parent = candidate_guard.PinnedOutputParent(
                            path=scratch.path,
                            descriptor=os.dup(scratch.run_descriptor),
                            device=scratch.run_device,
                            inode=scratch.run_inode,
                            output_name="benchmark-execution",
                            filesystem_type=scratch.filesystem_type,
                            free_bytes_at_admission=scratch.free_bytes_at_admission,
                        )
                        candidate_guard._prepare_execution_copy(
                            execution_parent,
                            executable_snapshot,
                            execution_staging_id,
                            BENCHMARK_EXECUTABLE,
                        )
                    command[0] = executable_snapshot.execution_path()
                    child_environment = os.environ.copy()
                    for variable in ("TMPDIR", "TMP", "TEMP"):
                        child_environment.pop(variable, None)
                    child_environment["TMPDIR"] = str(scratch.path)
                    return candidate_guard._run_guarded_with_pinned_executable(
                        command,
                        executable_snapshot,
                        report_path=jsonl_path,
                        summary_path=summary_path,
                        memory_limit_bytes=memory_limit,
                        maximum_memory_bytes=(
                            candidate_guard.ABSOLUTE_MAX_MEMORY_BYTES
                        ),
                        absolute_memory_ceiling_bytes=(
                            candidate_guard.ABSOLUTE_MAX_MEMORY_BYTES
                        ),
                        memory_enforcement_mode=_benchmark_memory_enforcement_mode(),
                        held_lock_descriptors=(heavy_lock, kagemusha_lock),
                        child_directory_descriptors=(scratch.run_descriptor,),
                        sample_interval_seconds=(
                            candidate_guard.SAMPLE_INTERVAL_SECONDS
                        ),
                        physical_footprint_interval_seconds=(
                            candidate_guard.SAMPLE_INTERVAL_SECONDS
                        ),
                        post_run_cleanup=cleanup_scratch,
                        post_run_validation=(
                            lambda: candidate_guard._validate_executable_unchanged(
                                executable_snapshot
                            )
                        ),
                        report_context={
                            "executable_identity": (
                                executable_snapshot.report_context()
                            ),
                            "scratch": scratch.report_context(),
                        },
                        child_environment=child_environment,
                    )
                except BaseException:
                    if not cleanup_started:
                        cleanup_scratch()
                    raise
    except resource_guard.LockUnavailable as error:
        print(f"Kagemusha benchmark guard refused to start: {error}", file=sys.stderr)
        return resource_guard.LOCK_UNAVAILABLE_EXIT_CODE
    except (resource_guard.GuardError, OSError) as error:
        print(f"Kagemusha benchmark guard failed closed: {error}", file=sys.stderr)
        return 1
    finally:
        if executable_snapshot is not None:
            executable_snapshot.close()


if __name__ == "__main__":
    raise SystemExit(main())
