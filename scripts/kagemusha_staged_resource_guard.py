#!/usr/bin/env python3
"""Resource guard primitives for Kagemusha V4 release generation.

The guarded command is always placed in a new process group.  Only that group
is signalled when the memory or host-headroom guard trips.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
from typing import Iterator, Sequence

try:
    import fcntl
except ImportError:  # pragma: no cover - the release workflow is POSIX-only.
    fcntl = None  # type: ignore[assignment]

try:
    import resource
except ImportError:  # pragma: no cover - keeps the refusal path importable.
    resource = None  # type: ignore[assignment]


BYTES_PER_GIB = 1024 * 1024 * 1024
DEFAULT_MAX_MEMORY_GIB = 64.0
MAXIMUM_MEMORY_GIB = 64.0
DEFAULT_SAMPLE_INTERVAL_SECONDS = 0.1
DEFAULT_FOOTPRINT_INTERVAL_SECONDS = 5.0
MAXIMUM_SAMPLE_INTERVAL_SECONDS = 5.0
MAXIMUM_FOOTPRINT_INTERVAL_SECONDS = 60.0
DEFAULT_MINIMUM_HEADROOM_GIB = 4.0
DEFAULT_MINIMUM_HEADROOM_FRACTION = 0.10
GUARD_EXIT_CODE = 137
GUARD_FD_ENV = "IROHA_KAGEMUSHA_V4_GUARD_FD"
RESOURCE_LOCK = Path("/tmp/iroha-kagemusha-v4-generation.lock")
MAX_STAGE_LINE_BYTES = 512
MAX_STAGE_DRAIN_BYTES = 64 * 1024
MAX_RECORDED_STAGE_EVENTS = 1024
PROCESS_TERMINATION_TIMEOUT_SECONDS = 10.0
MEMORY_BOUNDED_RELEASE_CODEGEN_UNITS = "256"
MEMORY_ACCOUNTING_MODE = "process_tree_rss"
PS = next(
    (candidate for candidate in ("/bin/ps", "/usr/bin/ps") if Path(candidate).exists()),
    "ps",
)
FOOTPRINT = next(
    (
        candidate
        for candidate in ("/usr/bin/footprint", "/bin/footprint")
        if Path(candidate).exists()
    ),
    None,
)
MEMORY_PRESSURE = "/usr/bin/memory_pressure"
SYSCTL = next(
    (
        candidate
        for candidate in ("/usr/sbin/sysctl", "/usr/bin/sysctl")
        if Path(candidate).exists()
    ),
    None,
)
FOOTPRINT_VALUE = re.compile(
    r"(?P<value>[0-9][0-9,]*(?:\.[0-9]+)?)\s*"
    r"(?P<unit>bytes?|[kmgt]i?b|[kmgt]b|[kmgt])\b",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class MemorySample:
    """One process-tree and host-memory sample."""

    monotonic_seconds: float
    process_tree_rss_bytes: int
    process_tree_footprint_bytes: int
    available_memory_bytes: int

    @property
    def guarded_memory_bytes(self) -> int:
        """Return process-tree RSS used for Kagemusha memory enforcement."""

        return self.process_tree_rss_bytes


@dataclass(frozen=True)
class GuardResult:
    """Final outcome of one guarded command."""

    exit_code: int
    report: dict[str, object]


class HeavyJobLockUnavailable(RuntimeError):
    """Another guarded Kagemusha V4 generation owns the shared lock."""


def gib_to_bytes(value: float) -> int:
    """Convert a GiB value to bytes."""

    numeric = float(value)
    if not math.isfinite(numeric):
        raise ValueError("GiB value must be finite")
    try:
        return int(numeric * BYTES_PER_GIB)
    except OverflowError as error:
        raise ValueError("GiB value is too large") from error


def validate_memory_limit_gib(value: float) -> None:
    """Validate the public memory limit without allowing a raised ceiling."""

    numeric = float(value)
    if not math.isfinite(numeric):
        raise ValueError("memory limit must be finite")
    if numeric <= 0:
        raise ValueError("memory limit must be greater than zero")
    if numeric > MAXIMUM_MEMORY_GIB:
        raise ValueError(
            "memory limit must not exceed the reviewed "
            f"{MAXIMUM_MEMORY_GIB:g} GiB ceiling"
        )


def validate_minimum_headroom_gib(value: float) -> None:
    """Validate the fixed host-memory reserve."""

    numeric = float(value)
    if not math.isfinite(numeric):
        raise ValueError("minimum headroom must be finite")
    if numeric < 0:
        raise ValueError("minimum headroom must not be negative")


@contextmanager
def acquire_heavy_job_lock(path: Path = RESOURCE_LOCK) -> Iterator[None]:
    """Acquire the single-heavy-job lock for this process lifetime."""

    if fcntl is None:  # pragma: no cover - unsupported Windows workflow.
        raise RuntimeError(
            "the Kagemusha V4 resource guard requires POSIX file locking"
        )
    flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags, 0o600)
    try:
        metadata = os.fstat(fd)
        if not stat.S_ISREG(metadata.st_mode):
            raise RuntimeError(f"Kagemusha V4 resource lock is not a file: {path}")
        if hasattr(os, "getuid") and metadata.st_uid != os.getuid():
            raise RuntimeError(
                f"Kagemusha V4 resource lock is not owned by this user: {path}"
            )
        os.fchmod(fd, 0o600)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (BlockingIOError, OSError) as error:
            raise HeavyJobLockUnavailable(
                f"another guarded Kagemusha V4 generation owns {path}"
            ) from error
        os.ftruncate(fd, 0)
        os.write(fd, f"pid={os.getpid()}\n".encode("ascii"))
        os.fsync(fd)
        yield
    finally:
        try:
            fcntl.flock(fd, fcntl.LOCK_UN)
        finally:
            os.close(fd)


def parse_ps_rows(output: str) -> list[tuple[int, int, int, int]]:
    """Parse ``pid ppid pgid rss_kib`` rows."""

    rows: list[tuple[int, int, int, int]] = []
    for line in output.splitlines():
        fields = line.split()
        if len(fields) < 4:
            continue
        try:
            pid, parent, group, rss_kib = (int(field) for field in fields[:4])
        except ValueError:
            continue
        rows.append((pid, parent, group, max(0, rss_kib) * 1024))
    return rows


def owned_process_ids(
    root_pid: int, rows: Sequence[tuple[int, int, int, int]]
) -> list[int]:
    """Return descendants and members of the root's process group."""

    children: dict[int, list[int]] = {}
    group_by_pid: dict[int, int] = {}
    for pid, parent, group, _ in rows:
        children.setdefault(parent, []).append(pid)
        group_by_pid[pid] = group
    descendants: set[int] = set()
    stack = [root_pid]
    while stack:
        pid = stack.pop()
        if pid in descendants:
            continue
        descendants.add(pid)
        stack.extend(children.get(pid, ()))
    root_group = group_by_pid.get(root_pid, root_pid)
    descendants.update(
        pid for pid, group in group_by_pid.items() if group == root_group
    )
    return sorted(descendants)


def process_tree_rss_bytes(root_pid: int) -> tuple[int, list[int]]:
    """Return aggregate RSS and owned process identifiers."""

    try:
        completed = subprocess.run(
            [PS, "-axo", "pid=,ppid=,pgid=,rss="],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError("failed to sample the guarded process group") from error
    if completed.returncode != 0:
        raise RuntimeError(
            f"failed to sample the guarded process group: {PS} exited "
            f"with status {completed.returncode}"
        )
    rows = parse_ps_rows(completed.stdout)
    rss_by_pid = {pid: rss for pid, _, _, rss in rows}
    # `owned_process_ids` deliberately includes the requested root so callers
    # can reason about a live leader.  Do not retain that synthetic identifier
    # after the leader has exited: probing a recycled process-group identifier
    # can target an unrelated group or fail with EPERM on macOS.
    pids = [pid for pid in owned_process_ids(root_pid, rows) if pid in rss_by_pid]
    return sum(rss_by_pid.get(pid, 0) for pid in pids), pids


def _unit_multiplier(unit: str) -> int:
    normalized = unit.lower()
    if normalized in {"byte", "bytes", "b"}:
        return 1
    if normalized in {"k", "kb", "kib"}:
        return 1024
    if normalized in {"m", "mb", "mib"}:
        return 1024 * 1024
    if normalized in {"g", "gb", "gib"}:
        return BYTES_PER_GIB
    if normalized in {"t", "tb", "tib"}:
        return 1024 * BYTES_PER_GIB
    return 0


def parse_footprint_bytes(output: str) -> int:
    """Parse the largest TOTAL/footprint value from macOS ``footprint``."""

    values: list[int] = []
    for line in output.splitlines():
        lowered = line.lower()
        if "total" not in lowered and "footprint" not in lowered:
            continue
        for match in FOOTPRINT_VALUE.finditer(line):
            multiplier = _unit_multiplier(match.group("unit"))
            if multiplier:
                values.append(
                    int(float(match.group("value").replace(",", "")) * multiplier)
                )
    return max(values, default=0)


def process_tree_footprint_bytes(pids: Sequence[int]) -> int:
    """Return summed macOS physical footprints, or zero when unavailable."""

    if FOOTPRINT is None or sys.platform != "darwin":
        return 0
    total = 0
    for pid in sorted(set(pids)):
        try:
            completed = subprocess.run(
                [FOOTPRINT, str(pid)],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS,
            )
        except (OSError, subprocess.SubprocessError):
            continue
        if completed.returncode == 0:
            total += parse_footprint_bytes(completed.stdout)
    return total


def total_physical_memory_bytes() -> int:
    """Return installed physical memory using portable POSIX counters."""

    try:
        pages = int(os.sysconf("SC_PHYS_PAGES"))
        page_size = int(os.sysconf("SC_PAGE_SIZE"))
        total = max(0, pages * page_size)
        if total > 0:
            return total
    except (OSError, ValueError, KeyError):
        pass
    if sys.platform == "darwin" and SYSCTL is not None:
        try:
            completed = subprocess.run(
                [SYSCTL, "-n", "hw.memsize"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS,
            )
            if completed.returncode == 0:
                return max(0, int(completed.stdout.strip()))
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
    return 0


def available_memory_bytes(total_memory_bytes: int | None = None) -> int | None:
    """Return available host memory, or ``None`` when it cannot be sampled."""

    if sys.platform.startswith("linux"):
        try:
            for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
                if line.startswith("MemAvailable:"):
                    fields = line.split()
                    if len(fields) >= 2:
                        return max(0, int(fields[1]) * 1024)
        except (OSError, ValueError, IndexError):
            return None
        return None
    if sys.platform == "darwin" and Path(MEMORY_PRESSURE).exists():
        environment = os.environ.copy()
        environment["LC_ALL"] = "C"
        try:
            completed = subprocess.run(
                [MEMORY_PRESSURE, "-Q"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="utf-8",
                errors="replace",
                env=environment,
                timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS,
            )
        except (OSError, subprocess.SubprocessError):
            return None
        match = re.search(
            r"free percentage:\s*([0-9]+(?:\.[0-9]+)?)%",
            completed.stdout,
            re.IGNORECASE,
        )
        total = total_memory_bytes or total_physical_memory_bytes()
        if completed.returncode == 0 and match and total > 0:
            percentage = min(100.0, max(0.0, float(match.group(1))))
            return int(total * percentage / 100.0)
    return None


def minimum_headroom_bytes(total_memory_bytes: int, configured_gib: float) -> int:
    """Return the larger fixed or proportional host reserve."""

    validate_minimum_headroom_gib(configured_gib)
    return max(
        gib_to_bytes(configured_gib),
        int(total_memory_bytes * DEFAULT_MINIMUM_HEADROOM_FRACTION),
    )


def effective_limit_bytes(
    requested_limit_bytes: int,
    available_bytes: int | None,
    reserve_bytes: int,
) -> int:
    """Lower the child limit when current host availability requires it."""

    if available_bytes is None:
        return requested_limit_bytes
    return min(requested_limit_bytes, max(0, available_bytes - reserve_bytes))


def physical_memory_capped_limit_bytes(
    requested_limit_bytes: int, total_memory_bytes: int
) -> int:
    """Cap one reviewed request at half of installed physical memory."""

    if requested_limit_bytes <= 0:
        raise ValueError("requested memory limit must be greater than zero")
    if total_memory_bytes <= 0:
        raise ValueError("total physical memory must be greater than zero")
    return min(
        requested_limit_bytes,
        gib_to_bytes(MAXIMUM_MEMORY_GIB),
        max(1, total_memory_bytes // 2),
    )


def soft_stop_bytes(
    hard_limit_bytes: int, *, kernel_limit_enforced: bool = True
) -> int:
    """Leave bounded shutdown room below the configured maximum."""

    if kernel_limit_enforced:
        margin = min(BYTES_PER_GIB, max(16 * 1024 * 1024, hard_limit_bytes // 16))
    else:
        # Darwin exposes RLIMIT_AS as an alias of its unenforceable RLIMIT_RSS
        # and rejects finite values with EINVAL. Leave a larger sampling and
        # process-group termination margin when supervision is the only limit.
        # Three GiB at the reviewed 64-GiB ceiling is still three times the
        # kernel-backed margin and sits on top of the independent host-memory
        # reserve. A four-GiB margin incorrectly rejected the partitioned
        # release data-model compiler at just over 12 GiB even with more than
        # 100 GiB of host memory available, before Kagemusha generation began.
        margin = min(
            3 * BYTES_PER_GIB,
            max(64 * 1024 * 1024, hard_limit_bytes // 4),
        )
    return max(1, hard_limit_bytes - margin)


def address_space_limit_supported() -> bool:
    """Return whether this host can install a finite child address-space limit."""

    return (
        resource is not None
        and hasattr(resource, "RLIMIT_AS")
        and sys.platform != "darwin"
    )


def _limit_address_space(limit_bytes: int) -> None:
    """Install the child address-space ceiling before exec."""

    if resource is None or not hasattr(resource, "RLIMIT_AS"):
        return
    current_soft, current_hard = resource.getrlimit(resource.RLIMIT_AS)

    def bounded(current: int) -> int:
        if current == resource.RLIM_INFINITY:
            return limit_bytes
        return min(current, limit_bytes)

    hard_limit = bounded(current_hard)
    soft_limit = min(bounded(current_soft), hard_limit)
    resource.setrlimit(resource.RLIMIT_AS, (soft_limit, hard_limit))


def _process_group_exists(process_group_id: int) -> bool:
    """Return whether the owned process group still has a member."""

    try:
        os.killpg(process_group_id, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _signal_owned_process_group(process_group_id: int, signal_number: int) -> None:
    """Signal the owned group, falling back to its exact sampled members.

    Some macOS sandbox profiles permit signalling child PIDs but reject a
    process-group signal when one Cargo descendant has a more restrictive
    execution profile.  The fallback remains scoped to PIDs observed in the
    freshly sampled descendant/group set; it never scans or signals by name.
    """

    try:
        os.killpg(process_group_id, signal_number)
        return
    except ProcessLookupError:
        return
    except PermissionError:
        pass

    try:
        _, pids = process_tree_rss_bytes(process_group_id)
    except RuntimeError:
        pids = []
    for pid in pids:
        try:
            os.kill(pid, signal_number)
        except ProcessLookupError:
            continue
        except PermissionError:
            # A later liveness sample and the SIGKILL phase still fail closed
            # if any exact owned member survives.
            continue


def terminate_owned_process_group(process: subprocess.Popen[bytes]) -> None:
    """Terminate only the process group created for ``process``."""

    process_group_id = int(process.pid)
    if process_group_id <= 0:
        raise RuntimeError("guarded process has no valid owned process group")
    _signal_owned_process_group(process_group_id, signal.SIGTERM)
    try:
        process.wait(timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        pass
    try:
        _, residual_pids = process_tree_rss_bytes(process_group_id)
    except RuntimeError:
        if not _process_group_exists(process_group_id):
            return
    else:
        if not residual_pids:
            return
    _signal_owned_process_group(process_group_id, signal.SIGKILL)
    try:
        process.wait(timeout=PROCESS_TERMINATION_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:  # pragma: no cover - unkillable OS process.
        raise RuntimeError(
            f"owned process group {process_group_id} did not terminate after SIGKILL"
        )


def _drain_stage_pipe(fd: int, pending: bytearray) -> list[str]:
    """Drain a bounded amount of child progress without starving supervision."""

    stages: list[str] = []
    drained_bytes = 0
    while drained_bytes < MAX_STAGE_DRAIN_BYTES:
        try:
            chunk = os.read(fd, min(4096, MAX_STAGE_DRAIN_BYTES - drained_bytes))
        except BlockingIOError:
            break
        if not chunk:
            break
        pending.extend(chunk)
        drained_bytes += len(chunk)
    while b"\n" in pending:
        line, _, remainder = pending.partition(b"\n")
        pending[:] = remainder
        decoded = line.decode("utf-8", errors="replace").strip()
        if decoded:
            stages.append(decoded[:MAX_STAGE_LINE_BYTES])
    if len(pending) > MAX_STAGE_LINE_BYTES:
        del pending[MAX_STAGE_LINE_BYTES:]
    return stages


def atomic_write_report(path: Path, document: dict[str, object]) -> None:
    """Write a private JSON receipt without exposing a partial document."""

    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            json.dump(document, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def run_guarded_command(
    command: Sequence[str],
    *,
    report_path: Path,
    max_memory_gib: float = DEFAULT_MAX_MEMORY_GIB,
    minimum_headroom_gib: float = DEFAULT_MINIMUM_HEADROOM_GIB,
    sample_interval_seconds: float = DEFAULT_SAMPLE_INTERVAL_SECONDS,
    footprint_interval_seconds: float = DEFAULT_FOOTPRINT_INTERVAL_SECONDS,
    enforce_address_space: bool = True,
    minimum_effective_bytes: int = BYTES_PER_GIB,
    lock_path: Path = RESOURCE_LOCK,
) -> GuardResult:
    """Run under reviewed, half-physical-RAM, headroom, lock, and stage guards."""

    validate_memory_limit_gib(max_memory_gib)
    validate_minimum_headroom_gib(minimum_headroom_gib)
    if isinstance(command, (str, bytes)) or not command:
        raise ValueError("guarded command must not be empty")
    command_parts = list(command)
    if any(not isinstance(part, str) for part in command_parts):
        raise ValueError("guarded command arguments must be strings")
    if not command_parts[0]:
        raise ValueError("guarded command executable must not be empty")
    if (
        not math.isfinite(sample_interval_seconds)
        or not math.isfinite(footprint_interval_seconds)
        or sample_interval_seconds <= 0
        or footprint_interval_seconds <= 0
    ):
        raise ValueError("sampling intervals must be greater than zero")
    if sample_interval_seconds > MAXIMUM_SAMPLE_INTERVAL_SECONDS:
        raise ValueError(
            "sample interval must not exceed "
            f"{MAXIMUM_SAMPLE_INTERVAL_SECONDS:g} seconds"
        )
    if footprint_interval_seconds > MAXIMUM_FOOTPRINT_INTERVAL_SECONDS:
        raise ValueError(
            "footprint interval must not exceed "
            f"{MAXIMUM_FOOTPRINT_INTERVAL_SECONDS:g} seconds"
        )
    if minimum_effective_bytes <= 0:
        raise ValueError("minimum effective byte limit must be greater than zero")

    requested_limit = gib_to_bytes(max_memory_gib)
    total_memory = total_physical_memory_bytes()
    if total_memory <= 0:
        raise RuntimeError("could not determine total physical memory")
    physical_half_limit = max(1, total_memory // 2)
    reviewed_limit = physical_memory_capped_limit_bytes(
        requested_limit, total_memory
    )
    reserve = minimum_headroom_bytes(total_memory, minimum_headroom_gib)
    initial_available = available_memory_bytes(total_memory)
    if initial_available is None:
        raise RuntimeError("could not determine available host memory")
    hard_limit = effective_limit_bytes(reviewed_limit, initial_available, reserve)
    if hard_limit < minimum_effective_bytes:
        raise RuntimeError(
            "insufficient memory headroom for guarded Kagemusha V4 generation: "
            f"effective_limit_bytes={hard_limit}, reserve_bytes={reserve}"
        )
    kernel_limit_enforced = enforce_address_space and address_space_limit_supported()
    soft_limit = soft_stop_bytes(
        hard_limit, kernel_limit_enforced=kernel_limit_enforced
    )

    started_at_unix = time.time()
    started_at_monotonic = time.monotonic()
    max_rss = 0
    max_footprint = 0
    minimum_available = initial_available
    termination_reason: str | None = None
    stage_events: list[str] = []
    stage_event_count = 0
    handshake_received = False
    pending = bytearray()

    def record_stage_events(events: Sequence[str]) -> None:
        nonlocal handshake_received, stage_event_count
        for event in events:
            stage_event_count += 1
            if event.startswith("stage=") and event[len("stage=") :].strip():
                handshake_received = True
            if len(stage_events) < MAX_RECORDED_STAGE_EVENTS:
                stage_events.append(event)
            else:
                stage_events[-1] = event

    with acquire_heavy_job_lock(lock_path):
        read_fd, write_fd = os.pipe()
        os.set_blocking(read_fd, False)
        process: subprocess.Popen[bytes] | None = None
        try:
            environment = os.environ.copy()
            environment[GUARD_FD_ENV] = str(write_fd)
            environment.setdefault("RAYON_NUM_THREADS", "1")
            environment.setdefault("CARGO_BUILD_JOBS", "1")
            # Release rustc can otherwise cross the Darwin supervisor's
            # sampled soft stop while optimizing the broad Iroha data model,
            # before Kagemusha generation even begins. More, smaller codegen
            # units preserve release optimization semantics while bounding
            # the compiler's resident working set.
            environment.setdefault(
                "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
                MEMORY_BOUNDED_RELEASE_CODEGEN_UNITS,
            )
            environment.setdefault(
                "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS",
                MEMORY_BOUNDED_RELEASE_CODEGEN_UNITS,
            )
            preexec = (
                (lambda: _limit_address_space(hard_limit))
                if kernel_limit_enforced
                else None
            )
            try:
                process = subprocess.Popen(
                    command_parts,
                    env=environment,
                    pass_fds=(write_fd,),
                    start_new_session=True,
                    preexec_fn=preexec,
                )
            finally:
                os.close(write_fd)

            last_footprint_at = 0.0
            last_footprint_bytes = 0
            while process.poll() is None:
                now = time.monotonic()
                record_stage_events(_drain_stage_pipe(read_fd, pending))
                try:
                    rss, pids = process_tree_rss_bytes(process.pid)
                except RuntimeError:
                    termination_reason = "process_memory_sample_unavailable"
                    terminate_owned_process_group(process)
                    break
                if now - last_footprint_at >= footprint_interval_seconds:
                    last_footprint_bytes = process_tree_footprint_bytes(pids)
                    last_footprint_at = now
                footprint = last_footprint_bytes
                max_rss = max(max_rss, rss)
                max_footprint = max(max_footprint, footprint)
                available = available_memory_bytes(total_memory)
                if available is None:
                    termination_reason = "host_memory_sample_unavailable"
                else:
                    minimum_available = min(minimum_available, available)
                sample = MemorySample(
                    monotonic_seconds=now,
                    process_tree_rss_bytes=rss,
                    process_tree_footprint_bytes=footprint,
                    available_memory_bytes=available or 0,
                )
                if sample.guarded_memory_bytes >= soft_limit:
                    termination_reason = "child_memory_soft_limit"
                elif available is not None and available <= reserve:
                    termination_reason = "host_headroom_floor"
                if termination_reason is not None:
                    terminate_owned_process_group(process)
                    break
                time.sleep(sample_interval_seconds)
            record_stage_events(_drain_stage_pipe(read_fd, pending))
            if pending:
                partial_stage = pending.decode("utf-8", errors="replace").strip()
                if partial_stage:
                    record_stage_events([partial_stage[:MAX_STAGE_LINE_BYTES]])
                pending.clear()
            child_exit = process.wait()
            if termination_reason is None:
                try:
                    final_rss, final_pids = process_tree_rss_bytes(process.pid)
                except RuntimeError:
                    termination_reason = "process_memory_sample_unavailable"
                    terminate_owned_process_group(process)
                else:
                    final_footprint = process_tree_footprint_bytes(final_pids)
                    max_rss = max(max_rss, final_rss)
                    max_footprint = max(max_footprint, final_footprint)
                    if final_pids:
                        termination_reason = "residual_owned_process_group"
                        terminate_owned_process_group(process)
        except BaseException:
            if process is not None:
                try:
                    terminate_owned_process_group(process)
                except (OSError, RuntimeError, subprocess.SubprocessError):
                    pass
            raise
        finally:
            os.close(read_fd)

    if termination_reason is not None:
        exit_code = GUARD_EXIT_CODE
    elif child_exit == 0 and not handshake_received:
        termination_reason = "missing_child_guard_handshake"
        exit_code = 2
    else:
        exit_code = child_exit if child_exit >= 0 else 128 - child_exit
    completed_at_unix = time.time()
    report: dict[str, object] = {
        "schema": "iroha.kagemusha-v4-resource-guard.v1",
        "command": command_parts,
        "started_at_unix_seconds": started_at_unix,
        "completed_at_unix_seconds": completed_at_unix,
        "elapsed_seconds": time.monotonic() - started_at_monotonic,
        "absolute_memory_ceiling_bytes": gib_to_bytes(MAXIMUM_MEMORY_GIB),
        "total_physical_memory_bytes": total_memory,
        "physical_half_limit_bytes": physical_half_limit,
        "requested_limit_bytes": requested_limit,
        "reviewed_limit_bytes": reviewed_limit,
        "effective_hard_limit_bytes": hard_limit,
        "soft_stop_bytes": soft_limit,
        "kernel_address_space_limit_enforced": kernel_limit_enforced,
        "memory_accounting_mode": MEMORY_ACCOUNTING_MODE,
        "memory_enforcement_mode": (
            "kernel_and_supervisor" if kernel_limit_enforced else "supervisor"
        ),
        "minimum_headroom_bytes": reserve,
        "initial_available_memory_bytes": initial_available,
        "minimum_available_memory_bytes": minimum_available,
        "max_process_tree_rss_bytes": max_rss,
        "max_process_tree_footprint_bytes": max_footprint,
        "guarded_peak_bytes": max_rss,
        "stage_events": stage_events,
        "stage_event_count": stage_event_count,
        "stage_events_dropped": max(0, stage_event_count - len(stage_events)),
        "last_stage": stage_events[-1] if stage_events else None,
        "child_guard_handshake_received": handshake_received,
        "termination_reason": termination_reason,
        "child_exit_code": child_exit,
        "exit_code": exit_code,
        "completed": exit_code == 0,
    }
    atomic_write_report(report_path, report)
    return GuardResult(exit_code=exit_code, report=report)
