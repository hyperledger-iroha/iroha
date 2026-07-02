#!/usr/bin/env python3
"""Resource guards for long-running Kagemusha staged subprocesses."""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import os
from pathlib import Path
import signal
import shlex
import subprocess
import time
from typing import Callable, Iterator, Protocol

try:  # pragma: no cover - exercised implicitly on POSIX hosts.
    import fcntl
except ImportError:  # pragma: no cover - Windows fallback for importability.
    fcntl = None  # type: ignore[assignment]


DEFAULT_MAX_RSS_GB = 8.0
DEFAULT_RSS_SAMPLE_INTERVAL_SECONDS = 5.0
DEFAULT_RESOURCE_LOCK_FILE = Path("/tmp").resolve() / "iroha-codex-kagemusha-heavy-job.lock"
RSS_LIMIT_EXIT_CODE = 137
BYTES_PER_GIB = 1024 * 1024 * 1024
PS_COMMAND = next(
    (
        str(candidate)
        for candidate in (Path("/bin/ps"), Path("/usr/bin/ps"))
        if candidate.exists()
    ),
    "ps",
)
HEAVY_JOB_COMMAND_MARKERS = (
    "iroha app zk kagemusha recursive-compact-key-artifacts",
    "iroha app zk kagemusha lineage-key-artifacts",
    "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output",
    "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
    "scripts/kagemusha_run_lineage_proof_staged.py",
)
HEAVY_JOB_SCRIPT_NAMES = (
    "kagemusha_run_recursive_compact_keygen_staged.py",
    "kagemusha_run_lineage_proof_staged.py",
)
HEAVY_IROHA_SUBCOMMANDS = (
    ("app", "zk", "kagemusha", "recursive-compact-key-artifacts"),
    ("app", "zk", "kagemusha", "lineage-key-artifacts"),
)
HEAVY_CARGO_TEST_MARKER = (
    "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output"
)


class WaitableProcess(Protocol):
    """Small process protocol used by the staged resource guard."""

    pid: int

    def wait(self, timeout: float | None = None) -> int:
        """Wait for process completion."""

    def terminate(self) -> None:
        """Ask the process to terminate."""

    def kill(self) -> None:
        """Forcefully kill the process."""


@dataclass(frozen=True)
class ResourceSummary:
    """Resource metrics captured while a staged child command ran."""

    max_rss_bytes: int
    rss_limit_bytes: int
    terminated_for_rss_limit: bool

    @classmethod
    def empty(cls, rss_limit_bytes: int) -> "ResourceSummary":
        """Return an empty summary for mocked command runners."""

        return cls(
            max_rss_bytes=0,
            rss_limit_bytes=max(0, int(rss_limit_bytes)),
            terminated_for_rss_limit=False,
        )

    @classmethod
    def combine(
        cls,
        summaries: list["ResourceSummary"],
        *,
        rss_limit_bytes: int,
    ) -> "ResourceSummary":
        """Combine per-command summaries into one run-level summary."""

        return cls(
            max_rss_bytes=max((summary.max_rss_bytes for summary in summaries), default=0),
            rss_limit_bytes=max(0, int(rss_limit_bytes)),
            terminated_for_rss_limit=any(
                summary.terminated_for_rss_limit for summary in summaries
            ),
        )

    def report_fields(self) -> dict[str, int | bool]:
        """Return the JSON report fields for this summary."""

        return {
            "max_rss_bytes": self.max_rss_bytes,
            "rss_limit_bytes": self.rss_limit_bytes,
            "terminated_for_rss_limit": self.terminated_for_rss_limit,
        }


@dataclass(frozen=True)
class GuardedCommandResult:
    """Exit status and resource metrics for one guarded command."""

    exit_code: int
    resource_summary: ResourceSummary


class HeavyJobLockUnavailable(RuntimeError):
    """Raised when another staged heavy job already holds the lock."""


@dataclass(frozen=True)
class RunningHeavyJob:
    """A running Kagemusha process that can consume staged-job memory."""

    pid: int
    parent_pid: int
    process_group_id: int
    rss_bytes: int


def rss_limit_bytes_from_gb(value: float) -> int:
    """Convert a GiB limit to bytes."""

    return int(float(value) * BYTES_PER_GIB)


def validate_resource_options(
    *,
    max_rss_gb: float,
    rss_sample_interval_seconds: float,
) -> list[str]:
    """Validate resource-guard CLI options."""

    errors: list[str] = []
    try:
        max_rss_value = float(max_rss_gb)
    except (TypeError, ValueError):
        max_rss_value = 0.0
    try:
        sample_value = float(rss_sample_interval_seconds)
    except (TypeError, ValueError):
        sample_value = 0.0
    if max_rss_value <= 0:
        errors.append("--max-rss-gb must be greater than zero")
    if sample_value <= 0:
        errors.append("--rss-sample-interval-seconds must be greater than zero")
    return errors


def validate_report_resource_fields(
    document: dict[object, object],
    label: str,
    *,
    require_not_terminated: bool,
) -> list[str]:
    """Validate resource fields embedded in staged JSON reports."""

    required = {
        "max_rss_bytes",
        "rss_limit_bytes",
        "terminated_for_rss_limit",
    }
    missing = sorted(required - set(document))
    if missing:
        return [f"{label} is missing {missing[0]}"]
    max_rss = document["max_rss_bytes"]
    if isinstance(max_rss, bool) or not isinstance(max_rss, int) or max_rss < 0:
        return [f"{label} max_rss_bytes must be a non-negative integer"]
    rss_limit = document["rss_limit_bytes"]
    if isinstance(rss_limit, bool) or not isinstance(rss_limit, int) or rss_limit <= 0:
        return [f"{label} rss_limit_bytes must be a positive integer"]
    terminated = document["terminated_for_rss_limit"]
    if not isinstance(terminated, bool):
        return [f"{label} terminated_for_rss_limit must be a boolean"]
    if require_not_terminated and terminated:
        return [f"{label} terminated_for_rss_limit must be false for publishable evidence"]
    if not terminated and max_rss > rss_limit:
        return [f"{label} max_rss_bytes must not exceed rss_limit_bytes unless terminated"]
    return []


@contextmanager
def acquire_heavy_job_lock(lock_file: Path) -> Iterator[None]:
    """Acquire the shared staged-heavy-job lock for one process lifetime."""

    lock_fd = os.open(lock_file, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        os.fchmod(lock_fd, 0o600)
        if fcntl is not None:
            try:
                fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except (BlockingIOError, OSError) as exc:
                raise HeavyJobLockUnavailable from exc
        else:  # pragma: no cover - POSIX hosts use fcntl.
            try:
                os.lockf(lock_fd, os.F_TLOCK, 0)
            except OSError as exc:
                raise HeavyJobLockUnavailable from exc
        os.ftruncate(lock_fd, 0)
        os.write(lock_fd, f"pid={os.getpid()}\n".encode("utf-8"))
        os.fsync(lock_fd)
        yield
    finally:
        try:
            if fcntl is not None:
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
            else:  # pragma: no cover - POSIX hosts use fcntl.
                os.lockf(lock_fd, os.F_ULOCK, 0)
        finally:
            os.close(lock_fd)


def _rss_bytes_for_pid_direct(pid: int) -> int:
    """Return RSS bytes for a single ``pid`` using platform ``ps`` output."""

    completed = subprocess.run(
        [PS_COMMAND, "-o", "rss=", "-p", str(pid)],
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if completed.returncode != 0:
        return 0
    raw = completed.stdout.strip()
    if not raw:
        return 0
    try:
        return int(raw.splitlines()[-1].strip()) * 1024
    except ValueError:
        return 0


def _rss_bytes_from_owned_ps(root_pid: int, output: str) -> int:
    """Return RSS bytes for ``root_pid`` descendants and owned process group."""

    rss_by_pid: dict[int, int] = {}
    pgid_by_pid: dict[int, int] = {}
    children_by_parent: dict[int, list[int]] = {}
    for line in output.splitlines():
        fields = line.split()
        if len(fields) < 4:
            continue
        try:
            pid = int(fields[0])
            parent_pid = int(fields[1])
            process_group_id = int(fields[2])
            rss_kib = int(fields[3])
        except ValueError:
            continue
        rss_by_pid[pid] = max(0, rss_kib) * 1024
        pgid_by_pid[pid] = process_group_id
        children_by_parent.setdefault(parent_pid, []).append(pid)

    tree_total = 0
    seen: set[int] = set()
    stack = [root_pid]
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        tree_total += rss_by_pid.get(pid, 0)
        stack.extend(children_by_parent.get(pid, ()))

    owned_process_group_id = pgid_by_pid.get(root_pid, root_pid)
    group_total = sum(
        rss_bytes
        for pid, rss_bytes in rss_by_pid.items()
        if pgid_by_pid.get(pid) == owned_process_group_id
    )
    return max(tree_total, group_total)


def _parse_ps_process_rows(output: str) -> list[tuple[int, int, int, int, str]]:
    """Parse ``ps`` process rows into pid, ppid, pgid, RSS bytes, and command."""

    rows: list[tuple[int, int, int, int, str]] = []
    for line in output.splitlines():
        fields = line.split(None, 4)
        if len(fields) < 5:
            continue
        try:
            pid = int(fields[0])
            parent_pid = int(fields[1])
            process_group_id = int(fields[2])
            rss_kib = int(fields[3])
        except ValueError:
            continue
        rows.append(
            (
                pid,
                parent_pid,
                process_group_id,
                max(0, rss_kib) * 1024,
                fields[4],
            )
        )
    return rows


def _command_tokens(command: str) -> list[str]:
    """Split a platform process command line into best-effort tokens."""

    try:
        tokens = shlex.split(command)
    except ValueError:
        tokens = command.split()
    return [token for token in tokens if token]


def _token_basename(token: str) -> str:
    """Return a command token basename without requiring the path to exist."""

    return Path(token).name


def _is_python_executable(token: str) -> bool:
    """Return whether a token looks like a Python interpreter executable."""

    name = _token_basename(token).lower()
    return name == "python" or name.startswith("python")


def _command_is_heavy_job(command: str) -> bool:
    """Return whether a process command is an actual Kagemusha heavy job."""

    tokens = _command_tokens(command)
    if not tokens:
        return False

    executable_name = _token_basename(tokens[0])
    if executable_name in HEAVY_JOB_SCRIPT_NAMES:
        return True
    if len(tokens) >= 2 and _is_python_executable(tokens[0]):
        if _token_basename(tokens[1]) in HEAVY_JOB_SCRIPT_NAMES:
            return True
    if (
        len(tokens) >= 3
        and executable_name == "env"
        and _is_python_executable(tokens[1])
        and _token_basename(tokens[2]) in HEAVY_JOB_SCRIPT_NAMES
    ):
        return True

    if HEAVY_CARGO_TEST_MARKER in command:
        if executable_name == "cargo" or executable_name.startswith("cargo-"):
            return True
        if HEAVY_CARGO_TEST_MARKER in executable_name:
            return True

    iroha_candidate_indices = [0]
    if executable_name == "env" and len(tokens) >= 2:
        iroha_candidate_indices.append(1)
    for index in iroha_candidate_indices:
        if index >= len(tokens) or _token_basename(tokens[index]) != "iroha":
            continue
        for subcommand in HEAVY_IROHA_SUBCOMMANDS:
            end = index + 1 + len(subcommand)
            if tuple(tokens[index + 1 : end]) == subcommand:
                return True
    return False


def find_running_heavy_jobs(*, exclude_pids: set[int] | None = None) -> list[RunningHeavyJob]:
    """Return running Kagemusha heavy jobs outside the supplied pid set."""

    excluded = set(exclude_pids or set())
    completed = subprocess.run(
        [PS_COMMAND, "-axo", "pid=,ppid=,pgid=,rss=,command="],
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if completed.returncode != 0:
        return []
    jobs: list[RunningHeavyJob] = []
    for pid, parent_pid, process_group_id, rss_bytes, command in _parse_ps_process_rows(
        completed.stdout
    ):
        if pid in excluded:
            continue
        if _command_is_heavy_job(command):
            jobs.append(
                RunningHeavyJob(
                    pid=pid,
                    parent_pid=parent_pid,
                    process_group_id=process_group_id,
                    rss_bytes=rss_bytes,
                )
            )
    return jobs


def validate_no_conflicting_heavy_jobs() -> list[str]:
    """Reject starting a staged job while another Kagemusha heavy job is live."""

    jobs = find_running_heavy_jobs(exclude_pids={os.getpid()})
    if not jobs:
        return []
    first = jobs[0]
    return [
        (
            "another Kagemusha staged heavy job is already running outside this "
            "guard; wait for it to finish or stop it before starting another "
            f"job (pid={first.pid}, pgid={first.process_group_id}, "
            f"rss_bytes={first.rss_bytes})"
        )
    ]


def rss_bytes_for_pid(pid: int) -> int:
    """Return total RSS bytes for ``pid`` and its process descendants."""

    if pid <= 0:
        return 0
    completed = subprocess.run(
        [PS_COMMAND, "-axo", "pid=,ppid=,pgid=,rss="],
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if completed.returncode == 0:
        total = _rss_bytes_from_owned_ps(pid, completed.stdout)
        if total > 0:
            return total
    return _rss_bytes_for_pid_direct(pid)


def _process_group_exists(process_group_id: int) -> bool:
    """Return whether a process group still has at least one member."""

    try:
        os.killpg(process_group_id, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def terminate_owned_process(process: WaitableProcess) -> None:
    """Terminate a child process launched by the staged runner."""

    pid = int(getattr(process, "pid", 0) or 0)
    process_group_signal_sent = False
    if pid > 0:
        try:
            os.killpg(pid, signal.SIGTERM)
            process_group_signal_sent = True
        except ProcessLookupError:
            process_group_signal_sent = False

    if not process_group_signal_sent:
        process.terminate()
    wait_completed = False
    try:
        process.wait(timeout=10.0)
        wait_completed = True
    except subprocess.TimeoutExpired:
        pass
    if process_group_signal_sent:
        if not _process_group_exists(pid):
            return
        try:
            os.killpg(pid, signal.SIGKILL)
        except ProcessLookupError:
            return
        process.wait(timeout=10.0)
    else:
        if wait_completed:
            return
        process.kill()
        process.wait(timeout=10.0)


def _write_guard_line(log_handle: object, line: str) -> None:
    log_handle.write(line.encode("utf-8"))  # type: ignore[attr-defined]
    log_handle.flush()  # type: ignore[attr-defined]
    os.fsync(log_handle.fileno())  # type: ignore[attr-defined]


def run_with_resource_guard(
    *,
    process: WaitableProcess,
    log_handle: object,
    heartbeat_label: str,
    started_monotonic: float,
    heartbeat_interval_seconds: float,
    max_rss_bytes: int,
    rss_sample_interval_seconds: float,
    rss_sampler: Callable[[int], int] = rss_bytes_for_pid,
    process_terminator: Callable[[WaitableProcess], None] = terminate_owned_process,
) -> GuardedCommandResult:
    """Wait for a child command while sampling RSS and writing guarded heartbeats."""

    last_rss_bytes = 0
    peak_rss_bytes = 0
    pid = int(getattr(process, "pid", 0) or 0)
    next_heartbeat = (
        started_monotonic + heartbeat_interval_seconds
        if heartbeat_interval_seconds > 0
        else None
    )
    next_sample = started_monotonic + rss_sample_interval_seconds

    while True:
        now = time.monotonic()
        wakeups: list[float] = []
        if next_heartbeat is not None:
            wakeups.append(max(next_heartbeat - now, 0.0))
        wakeups.append(max(next_sample - now, 0.0))
        timeout = min(wakeups) if wakeups else None
        try:
            exit_code = process.wait(timeout=timeout)
            residual_rss_bytes = max(0, int(rss_sampler(pid)))
            peak_rss_bytes = max(peak_rss_bytes, residual_rss_bytes)
            if residual_rss_bytes > 0:
                now = time.monotonic()
                elapsed_seconds = max(now - started_monotonic, 0.0)
                _write_guard_line(
                    log_handle,
                    (
                        f"[kagemusha-staged-runner] {heartbeat_label} "
                        "process-group-residual "
                        f"elapsed_seconds={elapsed_seconds:.6f} "
                        f"rss_bytes={residual_rss_bytes} "
                        f"max_rss_bytes={peak_rss_bytes} "
                        f"rss_limit_bytes={max_rss_bytes}\n"
                    ),
                )
                process_terminator(process)
                return GuardedCommandResult(
                    exit_code=RSS_LIMIT_EXIT_CODE,
                    resource_summary=ResourceSummary(
                        max_rss_bytes=peak_rss_bytes,
                        rss_limit_bytes=max_rss_bytes,
                        terminated_for_rss_limit=True,
                    ),
                )
            return GuardedCommandResult(
                exit_code=exit_code,
                resource_summary=ResourceSummary(
                    max_rss_bytes=peak_rss_bytes,
                    rss_limit_bytes=max_rss_bytes,
                    terminated_for_rss_limit=False,
                ),
            )
        except subprocess.TimeoutExpired:
            now = time.monotonic()

        sampled = False
        if now >= next_sample:
            last_rss_bytes = max(0, int(rss_sampler(pid)))
            peak_rss_bytes = max(peak_rss_bytes, last_rss_bytes)
            sampled = True
            next_sample = now + rss_sample_interval_seconds
            if last_rss_bytes > max_rss_bytes:
                elapsed_seconds = max(now - started_monotonic, 0.0)
                _write_guard_line(
                    log_handle,
                    (
                        f"[kagemusha-staged-runner] {heartbeat_label} rss-limit "
                        f"elapsed_seconds={elapsed_seconds:.6f} "
                        f"rss_bytes={last_rss_bytes} "
                        f"max_rss_bytes={peak_rss_bytes} "
                        f"rss_limit_bytes={max_rss_bytes}\n"
                    ),
                )
                process_terminator(process)
                return GuardedCommandResult(
                    exit_code=RSS_LIMIT_EXIT_CODE,
                    resource_summary=ResourceSummary(
                        max_rss_bytes=peak_rss_bytes,
                        rss_limit_bytes=max_rss_bytes,
                        terminated_for_rss_limit=True,
                    ),
                )

        if next_heartbeat is not None and now >= next_heartbeat:
            if not sampled:
                last_rss_bytes = max(0, int(rss_sampler(pid)))
                peak_rss_bytes = max(peak_rss_bytes, last_rss_bytes)
            elapsed_seconds = max(now - started_monotonic, 0.0)
            _write_guard_line(
                log_handle,
                (
                    f"[kagemusha-staged-runner] {heartbeat_label} heartbeat "
                    f"elapsed_seconds={elapsed_seconds:.6f} "
                    f"rss_bytes={last_rss_bytes} "
                    f"max_rss_bytes={peak_rss_bytes} "
                    f"rss_limit_bytes={max_rss_bytes}\n"
                ),
            )
            next_heartbeat = now + heartbeat_interval_seconds
