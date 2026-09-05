#!/usr/bin/env python3
"""Run the dedicated real Kagemusha proof binary under a hard macOS memory guard."""

from __future__ import annotations

import argparse
from contextlib import ExitStack, contextmanager
import ctypes
from dataclasses import dataclass
import errno
import fcntl
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import time
from typing import Iterator, NoReturn, Sequence


TEST_FUNCTION = (
    "real_mint_authority_bootstrap_and_positive_finalized_mint_use_reusable_keys"
)
TEST_NAME = (
    "zk::kagemusha_v1_recursion::real_handoff_qualification_tests::"
    f"real_payment_corridor::{TEST_FUNCTION}"
)
PROOF_BINARY = "kagemusha_real_proof"
PROOF_FEATURE = "kagemusha-real-proof-harness"
DEFAULT_MEMORY_LIMIT_GIB = 24.0
ABSOLUTE_MEMORY_LIMIT_GIB = 32.0
DEFAULT_TIMEOUT_SECONDS = 45 * 60.0
DEFAULT_SAMPLE_INTERVAL_SECONDS = 0.05
DEFAULT_PROGRESS_INTERVAL_SECONDS = 30.0
MEMORY_LIMIT_EXIT_CODE = 75
LOCK_UNAVAILABLE_EXIT_CODE = 73
DUPLICATE_RUN_EXIT_CODE = 74
ACCOUNTING_FAILURE_EXIT_CODE = 76
TIMEOUT_EXIT_CODE = 124
INTERRUPTED_EXIT_CODE = 130
DARWIN_RUSAGE_INFO_V4 = 4
DARWIN_PROC_PIDTBSDINFO = 3
PROCESS_GROUP_PID_CAPACITY = 512
STABLE_SNAPSHOT_ATTEMPTS = 32
HARD_KILL_MAX_ATTEMPTS = 100
TERMINATION_POLL_INTERVAL_SECONDS = 0.05
HARD_KILL_TIMEOUT_SECONDS = 5.0
CANONICAL_RUSTC_WRAPPER_ENVIRONMENT = (
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
)
BUILD_WRAPPER_ENVIRONMENT = frozenset(
    {
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        *CANONICAL_RUSTC_WRAPPER_ENVIRONMENT,
    }
)


class GuardError(RuntimeError):
    """The proof could not be supervised safely."""


class LockUnavailable(GuardError):
    """Another guarded memory-heavy process owns the host lock."""


class ProcessRaced(GuardError):
    """A process exited while a scoped accounting snapshot was captured."""


@dataclass(frozen=True)
class MemorySample:
    """One stable aggregate sample for the isolated Cargo process group."""

    rss_bytes: int
    physical_footprint_bytes: int
    process_count: int

    @property
    def enforced_bytes(self) -> int:
        """Use the larger observable accounting value for the hard ceiling."""

        return max(self.rss_bytes, self.physical_footprint_bytes)


class _DarwinRusageInfoV4(ctypes.Structure):
    _fields_ = [
        ("ri_uuid", ctypes.c_uint8 * 16),
        *[
            (name, ctypes.c_uint64)
            for name in (
                "ri_user_time",
                "ri_system_time",
                "ri_pkg_idle_wkups",
                "ri_interrupt_wkups",
                "ri_pageins",
                "ri_wired_size",
                "ri_resident_size",
                "ri_phys_footprint",
                "ri_proc_start_abstime",
                "ri_proc_exit_abstime",
                "ri_child_user_time",
                "ri_child_system_time",
                "ri_child_pkg_idle_wkups",
                "ri_child_interrupt_wkups",
                "ri_child_pageins",
                "ri_child_elapsed_abstime",
                "ri_diskio_bytesread",
                "ri_diskio_byteswritten",
                "ri_cpu_time_qos_default",
                "ri_cpu_time_qos_maintenance",
                "ri_cpu_time_qos_background",
                "ri_cpu_time_qos_utility",
                "ri_cpu_time_qos_legacy",
                "ri_cpu_time_qos_user_initiated",
                "ri_cpu_time_qos_user_interactive",
                "ri_billed_system_time",
                "ri_serviced_system_time",
                "ri_logical_writes",
                "ri_lifetime_max_phys_footprint",
                "ri_instructions",
                "ri_cycles",
                "ri_billed_energy",
                "ri_serviced_energy",
                "ri_interval_max_phys_footprint",
                "ri_runnable_time",
            )
        ],
    ]


class _DarwinProcBsdInfo(ctypes.Structure):
    _fields_ = [
        ("pbi_flags", ctypes.c_uint32),
        ("pbi_status", ctypes.c_uint32),
        ("pbi_xstatus", ctypes.c_uint32),
        ("pbi_pid", ctypes.c_uint32),
        ("pbi_ppid", ctypes.c_uint32),
        ("pbi_uid", ctypes.c_uint32),
        ("pbi_gid", ctypes.c_uint32),
        ("pbi_ruid", ctypes.c_uint32),
        ("pbi_rgid", ctypes.c_uint32),
        ("pbi_svuid", ctypes.c_uint32),
        ("pbi_svgid", ctypes.c_uint32),
        ("rfu_1", ctypes.c_uint32),
        ("pbi_comm", ctypes.c_char * 16),
        ("pbi_name", ctypes.c_char * 32),
        ("pbi_nfiles", ctypes.c_uint32),
        ("pbi_pgid", ctypes.c_uint32),
        ("pbi_pjobc", ctypes.c_uint32),
        ("e_tdev", ctypes.c_uint32),
        ("e_tpgid", ctypes.c_uint32),
        ("pbi_nice", ctypes.c_int32),
        ("pbi_start_tvsec", ctypes.c_uint64),
        ("pbi_start_tvusec", ctypes.c_uint64),
    ]


class DarwinProcessAccounting:
    """Read stable process-group RSS and physical footprint through libproc."""

    def __init__(self) -> None:
        try:
            self._libproc = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        except OSError as error:
            raise GuardError("could not load macOS process accounting") from error
        self._libproc.proc_listpgrppids.argtypes = (
            ctypes.c_int,
            ctypes.c_void_p,
            ctypes.c_int,
        )
        self._libproc.proc_listpgrppids.restype = ctypes.c_int
        self._libproc.proc_pidinfo.argtypes = (
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_uint64,
            ctypes.c_void_p,
            ctypes.c_int,
        )
        self._libproc.proc_pidinfo.restype = ctypes.c_int
        self._libproc.proc_pid_rusage.argtypes = (
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_void_p,
        )
        self._libproc.proc_pid_rusage.restype = ctypes.c_int

    def _process_ids(self, process_group_id: int) -> tuple[int, ...]:
        if process_group_id <= 1:
            raise GuardError("refusing to inspect an invalid process group")
        buffer = (ctypes.c_int * PROCESS_GROUP_PID_CAPACITY)()
        ctypes.set_errno(0)
        count = self._libproc.proc_listpgrppids(
            process_group_id, buffer, ctypes.sizeof(buffer)
        )
        if count < 0:
            error_number = ctypes.get_errno()
            raise GuardError(
                "could not enumerate the guarded process group: "
                f"{os.strerror(error_number) if error_number else 'unknown error'}"
            )
        if count >= PROCESS_GROUP_PID_CAPACITY:
            raise GuardError("guarded process-group accounting buffer saturated")
        process_ids = tuple(sorted(int(buffer[index]) for index in range(count)))
        if any(process_id <= 1 for process_id in process_ids):
            raise GuardError("guarded process group contained an invalid PID")
        if len(process_ids) != len(set(process_ids)):
            raise GuardError("guarded process group contained duplicate PIDs")
        return process_ids

    def _identity(self, process_id: int, process_group_id: int) -> None:
        info = _DarwinProcBsdInfo()
        ctypes.set_errno(0)
        result = self._libproc.proc_pidinfo(
            process_id,
            DARWIN_PROC_PIDTBSDINFO,
            0,
            ctypes.byref(info),
            ctypes.sizeof(info),
        )
        if result != ctypes.sizeof(info):
            error_number = ctypes.get_errno()
            if error_number == errno.ESRCH:
                raise ProcessRaced(f"pid {process_id} exited during accounting")
            raise GuardError(f"could not authenticate guarded pid {process_id}")
        if (
            int(info.pbi_pid) != process_id
            or int(info.pbi_pgid) != process_group_id
            or int(info.pbi_ruid) != os.getuid()
            or int(info.pbi_uid) != os.geteuid()
            or int(info.pbi_start_tvsec) <= 0
        ):
            raise GuardError("guarded process identity changed during accounting")

    def _memory(self, process_id: int) -> tuple[int, int]:
        usage = _DarwinRusageInfoV4()
        ctypes.set_errno(0)
        result = self._libproc.proc_pid_rusage(
            process_id,
            DARWIN_RUSAGE_INFO_V4,
            ctypes.byref(usage),
        )
        if result != 0:
            error_number = ctypes.get_errno()
            if error_number == errno.ESRCH:
                raise ProcessRaced(f"pid {process_id} exited during accounting")
            raise GuardError(f"could not read memory for guarded pid {process_id}")
        return max(0, int(usage.ri_resident_size)), max(
            0, int(usage.ri_phys_footprint)
        )

    def sample(self, process_group_id: int) -> MemorySample:
        last_race: ProcessRaced | None = None
        last_unaccounted: tuple[int, ...] = ()
        for _ in range(STABLE_SNAPSHOT_ATTEMPTS):
            before = self._process_ids(process_group_id)
            rss_bytes = 0
            footprint_bytes = 0
            accounted: set[int] = set()
            for process_id in before:
                try:
                    self._identity(process_id, process_group_id)
                    rss, footprint = self._memory(process_id)
                except ProcessRaced as error:
                    # A compiler that exits between enumeration and rusage no longer
                    # contributes to the live group. Keep accounting the remaining
                    # members, then authenticate that every member still present was
                    # included before accepting this snapshot.
                    last_race = error
                    continue
                accounted.add(process_id)
                rss_bytes += rss
                footprint_bytes += footprint
            after = self._process_ids(process_group_id)
            # Successfully measured processes that exited before `after` only
            # overcount memory, which is fail-safe. A newly appeared or raced
            # process is retried and may never be silently omitted.
            unaccounted = set(after).difference(accounted)
            if not unaccounted:
                return MemorySample(rss_bytes, footprint_bytes, len(after))
            last_unaccounted = tuple(sorted(unaccounted))
        raise GuardError(
            "could not obtain a complete process-group memory snapshot after "
            f"{STABLE_SNAPSHOT_ATTEMPTS} attempts; current unaccounted PIDs: "
            + ", ".join(str(process_id) for process_id in last_unaccounted)
        ) from last_race

    def authenticated_process_ids(self, process_group_id: int) -> tuple[int, ...]:
        """Return current group members only after rechecking ownership and group identity."""

        last_race: ProcessRaced | None = None
        for _ in range(STABLE_SNAPSHOT_ATTEMPTS):
            process_ids = self._process_ids(process_group_id)
            authenticated: list[int] = []
            for process_id in process_ids:
                try:
                    self._identity(process_id, process_group_id)
                except ProcessRaced as error:
                    last_race = error
                    continue
                authenticated.append(process_id)
            # Exact membership stability is neither necessary nor desirable during termination:
            # signal every member whose identity was just authenticated, then rescan for stragglers.
            if authenticated or not process_ids:
                return tuple(authenticated)
        raise GuardError(
            "could not authenticate any process-group member for termination"
        ) from last_race

    def group_exists(self, process_group_id: int) -> bool:
        return bool(self._process_ids(process_group_id))


def _fail(message: str) -> NoReturn:
    raise GuardError(message)


def _parser(argv: Sequence[str] | None = None) -> argparse.Namespace:
    repository = Path(__file__).resolve().parents[1]
    default_target = (
        repository.parent / ".taira-testnet-build-targets" / "proof-memory-fix"
    )
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=repository)
    parser.add_argument("--target-dir", type=Path, default=default_target)
    parser.add_argument(
        "--memory-limit-gib", type=float, default=DEFAULT_MEMORY_LIMIT_GIB
    )
    parser.add_argument("--timeout-seconds", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument(
        "--sample-interval-seconds",
        type=float,
        default=DEFAULT_SAMPLE_INTERVAL_SECONDS,
    )
    parser.add_argument(
        "--progress-interval-seconds",
        type=float,
        default=DEFAULT_PROGRESS_INTERVAL_SECONDS,
    )
    parser.add_argument(
        "--summary",
        type=Path,
        default=Path("/tmp") / f"iroha-kagemusha-real-proof-{os.getuid()}.json",
    )
    args = parser.parse_args(argv)
    if not 0 < args.memory_limit_gib <= ABSOLUTE_MEMORY_LIMIT_GIB:
        parser.error(
            f"--memory-limit-gib must be positive and at most {ABSOLUTE_MEMORY_LIMIT_GIB:g}"
        )
    if args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be positive")
    if not 0.05 <= args.sample_interval_seconds <= 1.0:
        parser.error("--sample-interval-seconds must be between 0.05 and 1.0")
    if args.progress_interval_seconds <= 0:
        parser.error("--progress-interval-seconds must be positive")
    return args


def proof_command(target_dir: Path) -> list[str]:
    """Return the one permitted dedicated proof-binary command."""

    return [
        "cargo",
        "iroha-fast",
        "--zero-debug",
        "--no-sccache",
        "--target-dir",
        str(target_dir),
        "--",
        "run",
        "--locked",
        "-p",
        "iroha_core",
        "--bin",
        PROOF_BINARY,
        "--features",
        PROOF_FEATURE,
    ]


def _proof_environment(target_dir: Path) -> dict[str, str]:
    """Return an environment that cannot inherit an out-of-group compiler wrapper."""

    environment = os.environ.copy()
    for name in tuple(environment):
        if name in BUILD_WRAPPER_ENVIRONMENT or name.startswith("SCCACHE_"):
            environment.pop(name)
    # Cargo documents an empty canonical wrapper variable as the explicit
    # override that disables a wrapper configured in user/global Cargo config.
    # Leave the `CARGO_BUILD_*` aliases absent so these canonical overrides win.
    for name in CANONICAL_RUSTC_WRAPPER_ENVIRONMENT:
        environment[name] = ""
    environment["CARGO_TARGET_DIR"] = str(target_dir)
    environment["TAIRA_TESTNET_CARGO_TARGET_DIR"] = str(target_dir)
    return environment


def _host_memory_bytes() -> int:
    try:
        result = subprocess.run(
            ["/usr/sbin/sysctl", "-n", "hw.memsize"],
            check=True,
            capture_output=True,
            text=True,
            timeout=5,
        )
        value = int(result.stdout.strip())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise GuardError("could not determine physical host memory") from error
    if value <= 0:
        raise GuardError("physical host memory was invalid")
    return value


@contextmanager
def _exclusive_lock(path: Path, description: str) -> Iterator[None]:
    flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
        ):
            raise GuardError(f"unsafe {description} lock: {path}")
        os.fchmod(descriptor, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise LockUnavailable(f"another {description} is already running") from error
        os.ftruncate(descriptor, 0)
        os.write(descriptor, f"pid={os.getpid()}\n".encode("ascii"))
        os.fsync(descriptor)
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def _running_duplicate_commands() -> list[tuple[int, str]]:
    try:
        result = subprocess.run(
            ["/bin/ps", "-axo", "pid=,command="],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise GuardError("could not inspect existing proof processes") from error
    matches: list[tuple[int, str]] = []
    for raw_line in result.stdout.splitlines():
        fields = raw_line.strip().split(maxsplit=1)
        if len(fields) != 2:
            continue
        try:
            process_id = int(fields[0])
        except ValueError:
            continue
        if process_id == os.getpid():
            continue
        if TEST_FUNCTION in fields[1] or PROOF_BINARY in fields[1]:
            matches.append((process_id, fields[1]))
    return matches


def _format_gib(value: int) -> str:
    return f"{value / (1024**3):.2f} GiB"


def _terminate_process_group(
    process: subprocess.Popen[bytes],
    accounting: DarwinProcessAccounting,
    *,
    graceful: bool = True,
) -> None:
    process_group_id = process.pid

    def signal_authenticated_members(signal_number: signal.Signals) -> bool:
        # The child was created as a fresh session leader, so its live process-group ID cannot be
        # recycled while any descendant remains. Signal the group atomically to cover compiler
        # churn. Some macOS app sandboxes reject killpg even for an owned child; in that case,
        # authenticate each current member through libproc immediately before signaling it.
        process_ids = accounting.authenticated_process_ids(process_group_id)
        if not process_ids:
            return False
        try:
            os.killpg(process_group_id, signal_number)
            return True
        except ProcessLookupError:
            return True
        except PermissionError:
            pass
        for process_id in reversed(process_ids):
            try:
                os.kill(process_id, signal_number)
            except ProcessLookupError:
                pass
        return True

    def hard_kill_until_empty() -> None:
        deadline = time.monotonic() + HARD_KILL_TIMEOUT_SECONDS
        attempts = 0
        while attempts < HARD_KILL_MAX_ATTEMPTS and time.monotonic() < deadline:
            if not signal_authenticated_members(signal.SIGKILL):
                # A second authenticated enumeration closes the gap between the
                # empty observation and accepting termination as complete.
                if not accounting.authenticated_process_ids(process_group_id):
                    return
            attempts += 1
            time.sleep(TERMINATION_POLL_INTERVAL_SECONDS)
        remaining = accounting.authenticated_process_ids(process_group_id)
        if remaining:
            raise GuardError(
                "guarded process group survived repeated authenticated SIGKILL; "
                f"remaining members: {', '.join(str(pid) for pid in remaining)}"
            )

    if not graceful:
        hard_kill_until_empty()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        return

    signal_authenticated_members(signal.SIGTERM)
    deadline = time.monotonic() + 3.0
    while time.monotonic() < deadline:
        if not accounting.authenticated_process_ids(process_group_id):
            break
        time.sleep(TERMINATION_POLL_INTERVAL_SECONDS)
    if accounting.authenticated_process_ids(process_group_id):
        hard_kill_until_empty()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _write_summary(path: Path, payload: dict[str, object]) -> None:
    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.partial")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(temporary, flags, 0o600)
    try:
        encoded = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
        view = memoryview(encoded)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise GuardError("could not finish writing the proof-guard summary")
            view = view[written:]
        os.fsync(descriptor)
    except BaseException:
        os.close(descriptor)
        temporary.unlink(missing_ok=True)
        raise
    else:
        os.close(descriptor)
    os.replace(temporary, path)


def _validate_inputs(args: argparse.Namespace) -> tuple[Path, Path, Path, int]:
    if sys.platform != "darwin":
        _fail("the hard Kagemusha proof guard currently requires macOS libproc")
    repository = args.repository.resolve(strict=True)
    if not (repository / "Cargo.toml").is_file():
        _fail("--repository is not an Iroha Cargo checkout")
    target_dir = args.target_dir.resolve()
    if target_dir == repository / "target" or repository in target_dir.parents:
        _fail("--target-dir must be outside the repository")
    target_dir.mkdir(parents=True, exist_ok=True)
    summary_path = args.summary.resolve()
    limit_bytes = int(args.memory_limit_gib * 1024**3)
    host_memory = _host_memory_bytes()
    if limit_bytes > host_memory // 4:
        _fail("memory ceiling may not exceed one quarter of physical host memory")
    return repository, target_dir, summary_path, limit_bytes


def _run(args: argparse.Namespace) -> int:
    repository, target_dir, summary_path, limit_bytes = _validate_inputs(args)
    duplicate = _running_duplicate_commands()
    if duplicate:
        process_id, _ = duplicate[0]
        print(
            f"proof guard refused duplicate run: matching process pid={process_id}",
            file=sys.stderr,
        )
        return DUPLICATE_RUN_EXIT_CODE

    command = proof_command(target_dir)
    environment = _proof_environment(target_dir)
    accounting = DarwinProcessAccounting()
    started = time.monotonic()
    deadline = started + args.timeout_seconds
    peak = MemorySample(0, 0, 0)
    sample_count = 0
    child_exit_code: int | None = None
    exit_reason = "guard_error"
    exit_code = ACCOUNTING_FAILURE_EXIT_CODE
    process: subprocess.Popen[bytes] | None = None
    next_progress = started

    print(
        "proof guard starting the dedicated proof qualification; "
        f"limit={_format_gib(limit_bytes)}, timeout={args.timeout_seconds:g}s, "
        f"target={target_dir}",
        file=sys.stderr,
    )
    try:
        process = subprocess.Popen(
            command,
            cwd=repository,
            env=environment,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            start_new_session=True,
        )
        while True:
            now = time.monotonic()
            returncode = process.poll()
            sample = accounting.sample(process.pid)
            sample_count += 1
            peak = MemorySample(
                max(peak.rss_bytes, sample.rss_bytes),
                max(peak.physical_footprint_bytes, sample.physical_footprint_bytes),
                max(peak.process_count, sample.process_count),
            )
            if sample.enforced_bytes > limit_bytes:
                exit_reason = "memory_limit"
                exit_code = MEMORY_LIMIT_EXIT_CODE
                print(
                    "proof guard tripped: "
                    f"current RSS={_format_gib(sample.rss_bytes)}, "
                    f"physical footprint={_format_gib(sample.physical_footprint_bytes)}, "
                    f"processes={sample.process_count}",
                    file=sys.stderr,
                )
                _terminate_process_group(process, accounting, graceful=False)
                child_exit_code = process.returncode
                break
            if returncode is not None and sample.process_count == 0:
                child_exit_code = returncode
                exit_reason = "completed" if returncode == 0 else "child_exit"
                exit_code = returncode
                break
            if now >= deadline:
                exit_reason = "timeout"
                exit_code = TIMEOUT_EXIT_CODE
                _terminate_process_group(process, accounting)
                child_exit_code = process.returncode
                break
            if now >= next_progress:
                print(
                    "proof guard: "
                    f"elapsed={now - started:.1f}s, RSS={_format_gib(sample.rss_bytes)}, "
                    f"peak RSS={_format_gib(peak.rss_bytes)}, "
                    f"processes={sample.process_count}",
                    file=sys.stderr,
                )
                next_progress = now + args.progress_interval_seconds
            time.sleep(args.sample_interval_seconds)
    except KeyboardInterrupt:
        exit_reason = "interrupted"
        exit_code = INTERRUPTED_EXIT_CODE
        if process is not None:
            _terminate_process_group(process, accounting)
            child_exit_code = process.returncode
    except BaseException as error:
        print(f"proof guard failed closed: {error}", file=sys.stderr)
        if process is not None:
            _terminate_process_group(process, accounting)
            child_exit_code = process.returncode
    finally:
        ended = time.monotonic()
        summary = {
            "child_exit_code": child_exit_code,
            "command": command,
            "duration_seconds": round(ended - started, 3),
            "exit_code": exit_code,
            "exit_reason": exit_reason,
            "memory_limit_bytes": limit_bytes,
            "peak_enforced_bytes": peak.enforced_bytes,
            "peak_physical_footprint_bytes": peak.physical_footprint_bytes,
            "peak_process_count": peak.process_count,
            "peak_rss_bytes": peak.rss_bytes,
            "repository": str(repository),
            "sample_count": sample_count,
            "schema_version": 1,
            "target_dir": str(target_dir),
            "binary_target": PROOF_BINARY,
            "test_name": TEST_NAME,
        }
        _write_summary(summary_path, summary)
        print(
            f"proof guard result: {exit_reason}; peak RSS={_format_gib(peak.rss_bytes)}; "
            f"summary={summary_path}",
            file=sys.stderr,
        )
    return exit_code


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser(argv)
    runner_lock = Path("/tmp") / f"iroha-kagemusha-proof-runner-{os.getuid()}.lock"
    heavy_lock = Path("/tmp") / f"iroha-memory-heavy-{os.getuid()}.lock"
    try:
        with ExitStack() as stack:
            stack.enter_context(_exclusive_lock(heavy_lock, "memory-heavy job"))
            stack.enter_context(_exclusive_lock(runner_lock, "Kagemusha proof runner"))
            return _run(args)
    except LockUnavailable as error:
        print(f"proof guard refused to start: {error}", file=sys.stderr)
        return LOCK_UNAVAILABLE_EXIT_CODE
    except (GuardError, OSError) as error:
        print(f"proof guard failed closed: {error}", file=sys.stderr)
        return ACCOUNTING_FAILURE_EXIT_CODE


if __name__ == "__main__":
    raise SystemExit(main())
