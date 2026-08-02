"""Focused tests for the Sumeragi TLAPM resource supervisor."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import textwrap
import time

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
GUARD_PATH = ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlapm_guard.py"
RUNNER_PATH = ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlaps.sh"
SPEC = importlib.util.spec_from_file_location("sumeragi_v2_tlapm_guard", GUARD_PATH)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = guard
SPEC.loader.exec_module(guard)


def _events(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _wait_for(path: Path, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.02)
    raise AssertionError(f"timed out waiting for {path}")


def _wait_for_process_group_exit(process_group_id: int, timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group_id, 0)
        except (ProcessLookupError, PermissionError):
            return
        time.sleep(0.02)
    raise AssertionError(f"process group {process_group_id} is still present")


def _wait_for_process_exit(process_id: int, timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            os.kill(process_id, 0)
        except (ProcessLookupError, PermissionError):
            return
        time.sleep(0.02)
    raise AssertionError(f"process {process_id} is still present")


def test_host_lock_is_exclusive_and_rejects_unsafe_metadata(tmp_path: Path) -> None:
    lock = tmp_path / "guard.lock"
    with guard._host_lock(lock):
        with pytest.raises(guard.LockUnavailable):
            with guard._host_lock(lock):
                pass

    lock.chmod(0o644)
    with pytest.raises(guard.GuardError, match="unsafe metadata"):
        with guard._host_lock(lock):
            pass


def test_foreign_job_detection_is_same_user_and_name_exact() -> None:
    uid = os.getuid()
    rows = [
        guard.ProcessRow(11, 1, 11, uid, 1, "/tools/tlapm"),
        guard.ProcessRow(12, 1, 12, uid, 1, "/tools/Poly"),
        guard.ProcessRow(13, 1, 13, uid + 1, 1, "/tools/isabelle"),
        guard.ProcessRow(14, 1, 14, uid, 1, "/tools/tlapm-helper.py"),
        guard.ProcessRow(
            15, 1, 15, uid, 1, "/tools/kagemusha_recursive_spend_v4_bundle"
        ),
    ]

    assert [row.pid for row in guard._foreign_heavy_jobs(rows)] == [11, 12, 15]


def test_foreign_job_detection_excludes_owned_process_group() -> None:
    uid = os.getuid()
    rows = [
        guard.ProcessRow(11, 1, 55, uid, 1, "/tools/tlapm"),
        guard.ProcessRow(12, 1, 66, uid, 1, "/tools/Poly"),
        guard.ProcessRow(13, 1, 55, uid, 1, "/tools/isabelle"),
    ]

    assert [
        row.pid
        for row in guard._foreign_heavy_jobs(rows, owned_process_group_id=55)
    ] == [12]


def test_process_inspection_is_bounded_and_timeout_fails_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert guard.CONTROL_RECORD_TIMEOUT_SECONDS == 0.2
    assert guard.PROCESS_INSPECTION_TIMEOUT_SECONDS == 10.0
    assert guard.PHYSICAL_FOOTPRINT_INTERVAL_SECONDS == 5.0

    def timeout(*_args: object, **_kwargs: object) -> None:
        raise subprocess.TimeoutExpired([guard.PS], guard.PROCESS_INSPECTION_TIMEOUT_SECONDS)

    monkeypatch.setattr(guard.subprocess, "run", timeout)
    with pytest.raises(guard.GuardError, match="exceeded 10 s"):
        guard._process_rows()


def test_process_inspection_tolerates_legacy_two_second_overrun(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    observed_timeouts: list[float] = []

    def completes_after_legacy_deadline(
        command: list[str], **options: object
    ) -> subprocess.CompletedProcess[str]:
        timeout = options.get("timeout")
        assert isinstance(timeout, float)
        assert options.get("env") == guard.PROCESS_INSPECTION_ENVIRONMENT
        observed_timeouts.append(timeout)
        if timeout <= 2.0:
            raise subprocess.TimeoutExpired(command, timeout)
        return subprocess.CompletedProcess(
            command,
            0,
            stdout="11 1 11 501 64 /bin/example\n",
            stderr="",
        )

    monkeypatch.setattr(guard.subprocess, "run", completes_after_legacy_deadline)

    assert guard._process_rows() == [
        guard.ProcessRow(11, 1, 11, 501, 64 * 1024, "/bin/example")
    ]
    assert observed_timeouts == [10.0]


def _darwin_identity(
    process_id: int,
    *,
    process_group_id: int = 321,
    parent_id: int = 900,
    real_uid: int | None = None,
    start_time: int | None = None,
) -> guard.DarwinProcessIdentity:
    uid = os.getuid() if real_uid is None else real_uid
    return guard.DarwinProcessIdentity(
        pid=process_id,
        parent_pid=parent_id,
        process_group_id=process_group_id,
        effective_uid=os.geteuid(),
        real_uid=uid,
        start_time_seconds=process_id if start_time is None else start_time,
        start_time_microseconds=17,
        command=f"worker-{process_id}",
    )


def test_darwin_group_inspection_is_libproc_scoped_and_excludes_contaminant(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    enumerations: list[int] = []
    identity_reads: list[int] = []
    identities = {
        321: _darwin_identity(321),
        322: _darwin_identity(322, parent_id=321),
        999: _darwin_identity(999, process_group_id=999),
    }

    def scoped_pids(process_group_id: int) -> tuple[int, ...]:
        enumerations.append(process_group_id)
        return (321, 322)

    def identity(process_id: int) -> guard.DarwinProcessIdentity:
        identity_reads.append(process_id)
        return identities[process_id]

    monkeypatch.setattr(guard, "_darwin_list_process_group_pids", scoped_pids)
    monkeypatch.setattr(guard, "_darwin_process_identity", identity)
    monkeypatch.setattr(
        guard,
        "_darwin_process_memory",
        lambda pid: guard.DarwinProcessMemory(pid * 1024, pid * 2048),
    )
    monkeypatch.setattr(
        guard.subprocess,
        "run",
        lambda *_args, **_options: (_ for _ in ()).throw(
            AssertionError("Darwin scoped accounting invoked ps")
        ),
    )

    rows = guard._darwin_process_group_rows(
        321, expected_body_identity=identities[321]
    )

    assert enumerations == [321, 321]
    assert [row.pid for row in rows] == [321, 322]
    assert 999 not in identity_reads
    assert sum(row.rss_bytes for row in rows) == (321 + 322) * 1024
    assert sum(int(row.physical_footprint_bytes or 0) for row in rows) == (
        321 + 322
    ) * 2048


@pytest.mark.parametrize(
    ("identity", "message"),
    [
        (_darwin_identity(322, process_group_id=999), "foreign process group"),
        (_darwin_identity(322, real_uid=os.getuid() + 1), "foreign real UID"),
    ],
)
def test_darwin_group_inspection_rejects_contaminant_identity(
    monkeypatch: pytest.MonkeyPatch,
    identity: guard.DarwinProcessIdentity,
    message: str,
) -> None:
    identities = {321: _darwin_identity(321), 322: identity}
    monkeypatch.setattr(
        guard, "_darwin_list_process_group_pids", lambda _pgid: (321, 322)
    )
    monkeypatch.setattr(
        guard, "_darwin_process_identity", lambda pid: identities[pid]
    )
    monkeypatch.setattr(
        guard,
        "_darwin_process_memory",
        lambda _pid: guard.DarwinProcessMemory(1, 1),
    )

    with pytest.raises(guard.GuardError, match=message):
        guard._darwin_process_group_rows(321)


def test_darwin_group_inspection_requires_body_and_kernel_confirmed_exit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        guard, "_darwin_list_process_group_pids", lambda _pgid: (322,)
    )
    with pytest.raises(guard.GuardError, match="omitted the guarded body"):
        guard._darwin_process_group_rows(321)

    monkeypatch.setattr(
        guard, "_darwin_list_process_group_pids", lambda _pgid: ()
    )
    monkeypatch.setattr(guard, "_process_group_exists", lambda _pgid: False)
    assert guard._darwin_process_group_rows(321) == []

    monkeypatch.setattr(guard, "_process_group_exists", lambda _pgid: True)
    with pytest.raises(guard.GuardError, match="omitted an existing"):
        guard._darwin_process_group_rows(321)


@pytest.mark.parametrize("result", [-1, guard.DARWIN_PROCESS_GROUP_PID_CAPACITY])
def test_darwin_group_pid_api_error_or_saturation_fails_closed(
    monkeypatch: pytest.MonkeyPatch, result: int
) -> None:
    class FakeLibproc:
        @staticmethod
        def proc_listpgrppids(
            _pgid: int, _buffer: object, _buffer_size: int
        ) -> int:
            if result < 0:
                guard.ctypes.set_errno(5)
            return result

    monkeypatch.setattr(guard, "_darwin_libproc_handle", lambda: FakeLibproc())
    message = "could not enumerate" if result < 0 else "buffer saturated"
    with pytest.raises(guard.GuardError, match=message):
        guard._darwin_list_process_group_pids(321)


def test_darwin_pid_reuse_during_sample_fails_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    identity_reads = 0

    def identity(_process_id: int) -> guard.DarwinProcessIdentity:
        nonlocal identity_reads
        identity_reads += 1
        return _darwin_identity(321, start_time=1 if identity_reads == 1 else 2)

    monkeypatch.setattr(
        guard, "_darwin_list_process_group_pids", lambda _pgid: (321,)
    )
    monkeypatch.setattr(guard, "_darwin_process_identity", identity)
    monkeypatch.setattr(
        guard,
        "_darwin_process_memory",
        lambda _pid: guard.DarwinProcessMemory(1, 1),
    )

    with pytest.raises(guard.GuardError, match="changed identity"):
        guard._darwin_process_group_rows(321)


def test_macos_libproc_load_failure_fails_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def unavailable(*_args: object, **_kwargs: object) -> None:
        raise OSError("libproc unavailable")

    monkeypatch.setattr(guard.sys, "platform", "darwin")
    monkeypatch.setattr(guard, "_darwin_libproc", None)
    monkeypatch.setattr(guard.ctypes, "CDLL", unavailable)

    with pytest.raises(guard.GuardError, match="could not load Darwin"):
        guard._physical_footprint_bytes([123])


@pytest.mark.skipif(sys.platform != "darwin", reason="Darwin libproc is required")
def test_macos_libproc_reports_current_process_footprint() -> None:
    footprint = guard._darwin_process_physical_footprint_bytes(os.getpid())

    assert footprint > 0


def test_sampler_uses_larger_rss_or_physical_footprint(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    uid = os.getuid()
    monkeypatch.setattr(
        guard,
        "_group_rows",
        lambda _pgid, _rows=None: [
            guard.ProcessRow(11, 1, 11, uid, 9, "/bin/tool")
        ],
    )
    monkeypatch.setattr(guard, "_physical_footprint_bytes", lambda _pids: 7)

    sample = guard._sample_group(11)

    assert sample.memory_bytes == 9
    assert sample.accounting_method == "max_rss_physical_footprint"


def test_darwin_scoped_sample_enforces_already_available_footprint() -> None:
    row = guard.ProcessRow(
        11,
        1,
        11,
        os.getuid(),
        9,
        "/bin/tool",
        physical_footprint_bytes=72,
    )

    sample = guard._sample_group(
        11,
        [row],
        include_physical_footprint=False,
    )

    assert sample.memory_bytes == 72
    assert sample.rss_bytes == 9
    assert sample.physical_footprint_bytes == 72
    assert sample.accounting_method == guard.MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT


def test_sampler_can_enforce_rss_while_retaining_footprint_diagnostics(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    uid = os.getuid()
    monkeypatch.setattr(
        guard,
        "_group_rows",
        lambda _pgid, _rows=None: [
            guard.ProcessRow(11, 1, 11, uid, 9, "/bin/tool")
        ],
    )
    monkeypatch.setattr(guard, "_physical_footprint_bytes", lambda _pids: 72)

    sample = guard._sample_group(
        11,
        memory_enforcement_mode=guard.MEMORY_ENFORCEMENT_PROCESS_TREE_RSS,
    )

    assert sample.memory_bytes == 9
    assert sample.rss_bytes == 9
    assert sample.physical_footprint_bytes == 72
    assert sample.accounting_method == guard.MEMORY_ENFORCEMENT_PROCESS_TREE_RSS


def test_unknown_memory_enforcement_mode_is_rejected() -> None:
    with pytest.raises(guard.GuardError, match="unknown memory enforcement mode"):
        guard._sample_group(11, [], memory_enforcement_mode="unreviewed")


def test_rss_enforcement_does_not_stop_on_diagnostic_footprint(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    diagnostic_footprint = 2 * 1024 * 1024 * 1024
    monkeypatch.setattr(guard, "_foreign_heavy_jobs", lambda *_args, **_kwargs: [])
    if sys.platform == "darwin":
        process_memory = guard._darwin_process_memory

        def diagnostic_process_memory(pid: int) -> guard.DarwinProcessMemory:
            measured = process_memory(pid)
            return guard.DarwinProcessMemory(
                measured.rss_bytes, diagnostic_footprint
            )

        monkeypatch.setattr(
            guard, "_darwin_process_memory", diagnostic_process_memory
        )
    else:
        monkeypatch.setattr(
            guard,
            "_physical_footprint_bytes",
            lambda _pids: diagnostic_footprint,
        )

    status = guard._run_guarded(
        [sys.executable, "-c", "import time; time.sleep(0.1)"],
        report_path=jsonl,
        summary_path=summary,
        memory_limit_bytes=1024 * 1024 * 1024,
        sample_interval_seconds=0.01,
        physical_footprint_interval_seconds=0.01,
        memory_enforcement_mode=guard.MEMORY_ENFORCEMENT_PROCESS_TREE_RSS,
    )

    document = json.loads(summary.read_text(encoding="utf-8"))
    footprint_samples = [
        event
        for event in _events(jsonl)
        if event.get("physical_footprint_bytes") == diagnostic_footprint
    ]
    assert status == 0
    assert document["exit_reason"] == "completed"
    assert document["memory_enforcement_mode"] == (
        guard.MEMORY_ENFORCEMENT_PROCESS_TREE_RSS
    )
    assert document["peak_physical_footprint_bytes"] == diagnostic_footprint
    assert footprint_samples
    assert all(
        int(sample["memory_bytes"]) == int(sample["rss_bytes"])
        for sample in footprint_samples
    )


def test_sample_group_reuses_supplied_process_snapshot(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    uid = os.getuid()
    rows = [guard.ProcessRow(11, 1, 55, uid, 9, "/bin/tool")]

    def unexpected_inspection() -> list[guard.ProcessRow]:
        raise AssertionError("sample performed a second process inspection")

    monkeypatch.setattr(guard, "_process_rows", unexpected_inspection)
    monkeypatch.setattr(guard, "_physical_footprint_bytes", lambda _pids: 0)

    sample = guard._sample_group(55, rows)

    assert sample.memory_bytes == 9
    assert sample.process_count == 1


def test_rss_sample_can_defer_physical_footprint(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A loader-startup sample must not invoke macOS footprint eagerly."""

    uid = os.getuid()
    rows = [guard.ProcessRow(11, 1, 55, uid, 9, "/bin/tool")]

    def unexpected_footprint(_pids: object) -> int:
        raise AssertionError("deferred sample invoked physical footprint")

    monkeypatch.setattr(guard, "_physical_footprint_bytes", unexpected_footprint)

    sample = guard._sample_group(
        55,
        rows,
        include_physical_footprint=False,
    )

    assert sample.memory_bytes == 9
    assert sample.physical_footprint_bytes == 0
    assert sample.accounting_method == "rss"


def test_signal_race_is_benign_after_group_absence_is_verified(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls = 0

    def raced_killpg(_process_group_id: int, _signum: int) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            raise OSError("process exited during signal")
        raise ProcessLookupError

    monkeypatch.setattr(guard.os, "killpg", raced_killpg)

    guard._signal_process_group(12345, signal.SIGTERM)
    assert calls == 2


def test_signal_error_remains_fatal_while_group_exists(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls = 0

    def failed_killpg(_process_group_id: int, _signum: int) -> None:
        nonlocal calls
        calls += 1
        if calls == 1:
            raise OSError("signal failed")

    monkeypatch.setattr(guard.os, "killpg", failed_killpg)

    with pytest.raises(guard.GuardError, match="could not signal owned process group"):
        guard._signal_process_group(12345, signal.SIGTERM)
    assert calls == 2


def test_delayed_distinct_wrapper_gets_long_reap_bound_without_extra_signal(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    waits: list[float] = []
    signals: list[tuple[int, int]] = []

    class DelayedWrapper:
        pid = 111

        def wait(self, *, timeout: float) -> int:
            waits.append(timeout)
            if timeout <= guard.TERM_GRACE_SECONDS:
                raise subprocess.TimeoutExpired(self.pid, timeout)
            return 0

    monkeypatch.setattr(guard, "_process_group_exists", lambda _pgid: False)
    monkeypatch.setattr(
        guard,
        "_signal_process_group",
        lambda pgid, signum: signals.append((pgid, signum)),
    )

    guard._kill_owned_group_immediately(DelayedWrapper(), 222)

    assert waits == [guard.WRAPPER_REAP_TIMEOUT_SECONDS]
    assert signals == [(222, signal.SIGKILL)]


def test_wedged_distinct_wrapper_is_killed_and_timeout_becomes_guard_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    waits: list[float] = []
    signals: list[tuple[int, int]] = []

    class WedgedWrapper:
        pid = 111

        def wait(self, *, timeout: float) -> int:
            waits.append(timeout)
            raise subprocess.TimeoutExpired(self.pid, timeout)

    monkeypatch.setattr(guard, "_process_group_exists", lambda _pgid: False)
    monkeypatch.setattr(
        guard,
        "_signal_process_group",
        lambda pgid, signum: signals.append((pgid, signum)),
    )

    with pytest.raises(guard.GuardError, match="wrapper 111 could not be reaped"):
        guard._kill_owned_group_immediately(WedgedWrapper(), 222)

    assert waits == [
        guard.WRAPPER_REAP_TIMEOUT_SECONDS,
        guard.TERM_GRACE_SECONDS,
    ]
    assert signals == [(222, signal.SIGKILL), (111, signal.SIGKILL)]


def test_delayed_process_inspection_schedules_from_probe_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    sample_interval_seconds = 0.04
    probe_delay_seconds = 0.06
    probe_windows: list[tuple[float, float]] = []

    def delayed_process_rows() -> list[guard.ProcessRow]:
        started = time.monotonic()
        time.sleep(probe_delay_seconds)
        probe_windows.append((started, time.monotonic()))
        return []

    monkeypatch.setattr(guard, "_process_rows", delayed_process_rows)
    monkeypatch.setattr(guard.sys, "platform", "linux")
    monkeypatch.setattr(guard.os, "fsync", lambda _descriptor: None)

    def unexpected_footprint(_pids: object) -> int:
        raise AssertionError("short-lived child was footprint-probed during startup")

    monkeypatch.setattr(guard, "_physical_footprint_bytes", unexpected_footprint)

    status = guard._run_guarded(
        [sys.executable, "-c", "import time; time.sleep(0.4)"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=sample_interval_seconds,
    )

    document = json.loads(summary.read_text(encoding="utf-8"))
    sample_count = int(document["sample_count"])
    runtime_probes = probe_windows[1 : 1 + sample_count]
    assert status == 0
    assert len(runtime_probes) >= 2
    gaps_after_probe = [
        current_started - previous_finished
        for (_, previous_finished), (current_started, _) in zip(
            runtime_probes, runtime_probes[1:]
        )
    ]
    assert min(gaps_after_probe) >= sample_interval_seconds * 0.9


def test_darwin_runtime_samples_never_request_a_global_inventory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    global_inspections = 0
    scoped_inspections: list[int] = []
    expected_identities: list[guard.DarwinProcessIdentity | None] = []

    def global_rows() -> list[guard.ProcessRow]:
        nonlocal global_inspections
        global_inspections += 1
        if global_inspections > 2:
            raise AssertionError("Darwin hot loop requested a global inventory")
        return []

    def capture_body(
        process_group_id: int, wrapper_process_id: int
    ) -> guard.DarwinProcessIdentity:
        return _darwin_identity(
            process_group_id,
            process_group_id=process_group_id,
            parent_id=wrapper_process_id,
        )

    def scoped_rows(
        process_group_id: int,
        *,
        expected_body_identity: guard.DarwinProcessIdentity | None = None,
    ) -> list[guard.ProcessRow]:
        scoped_inspections.append(process_group_id)
        expected_identities.append(expected_body_identity)
        return [
            guard.ProcessRow(
                process_group_id,
                1,
                process_group_id,
                os.getuid(),
                1024,
                "/bin/worker",
                2048,
            )
        ]

    monkeypatch.setattr(guard.sys, "platform", "darwin")
    monkeypatch.setattr(guard, "_process_rows", global_rows)
    monkeypatch.setattr(guard, "_capture_darwin_body_identity", capture_body)
    monkeypatch.setattr(guard, "_darwin_process_group_rows", scoped_rows)
    monkeypatch.setattr(
        guard.subprocess,
        "run",
        lambda *_args, **_options: (_ for _ in ()).throw(
            AssertionError("Darwin runtime invoked /bin/ps")
        ),
    )

    status = guard._run_guarded(
        [sys.executable, "-c", "import time; time.sleep(0.15)"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=0.01,
        physical_footprint_interval_seconds=60,
    )

    assert status == 0
    assert global_inspections == 2
    assert len(scoped_inspections) >= 2
    assert len(set(scoped_inspections)) == 1
    assert all(identity is not None for identity in expected_identities)


def test_sampling_failure_terminates_known_group_and_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"

    def fail_sample(
        _pgid: int, _rows: object | None = None, **_options: object
    ) -> guard.MemorySample:
        raise guard.GuardError("inspection timeout")

    monkeypatch.setattr(guard, "_sample_group", fail_sample)
    result = guard._run_guarded(
        ["/bin/sleep", "30"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=0.01,
    )

    assert result == 1
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "guard_error"
    spawn = next(event for event in _events(jsonl) if event["event"] == "spawn")
    _wait_for_process_group_exit(int(spawn["process_group_id"]))


def test_foreign_job_started_after_spawn_kills_only_owned_group_and_exits_74(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    uid = os.getuid()
    foreign = guard.ProcessRow(
        2_000_000_000,
        1,
        2_000_000_000,
        uid,
        1,
        "/tools/Poly",
    )
    inspections = 0

    def process_rows() -> list[guard.ProcessRow]:
        nonlocal inspections
        inspections += 1
        return [] if inspections == 1 else [foreign]

    killed_groups: list[int] = []
    kill_owned_group = guard._kill_owned_group_immediately

    def record_kill(
        process: subprocess.Popen[bytes], process_group_id: int
    ) -> None:
        killed_groups.append(process_group_id)
        kill_owned_group(process, process_group_id)

    monkeypatch.setattr(guard, "_process_rows", process_rows)
    monkeypatch.setattr(guard.sys, "platform", "linux")
    monkeypatch.setattr(guard, "_kill_owned_group_immediately", record_kill)

    status = guard._run_guarded(
        ["/bin/sleep", "30"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=0.01,
    )

    document = json.loads(summary.read_text(encoding="utf-8"))
    spawn = next(event for event in _events(jsonl) if event["event"] == "spawn")
    conflict = next(
        event for event in _events(jsonl) if event["event"] == "foreign_heavy_job"
    )
    owned_process_group_id = int(spawn["process_group_id"])
    assert status == guard.FOREIGN_JOB_EXIT_CODE
    assert document["exit_reason"] == "foreign_heavy_job"
    assert document["exit_status"] == guard.FOREIGN_JOB_EXIT_CODE
    assert killed_groups == [owned_process_group_id]
    assert owned_process_group_id != foreign.process_group_id
    assert conflict["phase"] == "runtime"
    assert conflict["foreign_process_group_id"] == foreign.process_group_id
    assert conflict["owned_process_group_id"] == owned_process_group_id
    _wait_for_process_group_exit(owned_process_group_id)


def test_runtime_foreign_inspection_timeout_terminates_owned_group_and_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    inspections = 0

    def process_rows() -> list[guard.ProcessRow]:
        nonlocal inspections
        inspections += 1
        if inspections == 1:
            return []
        raise guard.GuardError("runtime process inspection timed out")

    signalled_groups: list[int] = []
    signal_process_group = guard._signal_process_group

    def record_signal(process_group_id: int, signum: int) -> None:
        signalled_groups.append(process_group_id)
        signal_process_group(process_group_id, signum)

    monkeypatch.setattr(guard, "_process_rows", process_rows)
    monkeypatch.setattr(guard.sys, "platform", "linux")
    monkeypatch.setattr(guard, "_signal_process_group", record_signal)

    status = guard._run_guarded(
        ["/bin/sleep", "30"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=0.01,
    )

    document = json.loads(summary.read_text(encoding="utf-8"))
    spawn = next(event for event in _events(jsonl) if event["event"] == "spawn")
    owned_process_group_id = int(spawn["process_group_id"])
    assert status == 1
    assert document["exit_reason"] == "guard_error"
    assert signalled_groups
    assert set(signalled_groups) == {owned_process_group_id}
    _wait_for_process_group_exit(owned_process_group_id)


def test_foreign_job_at_final_success_gate_skips_finalize(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    uid = os.getuid()
    foreign = guard.ProcessRow(
        2_000_000_000,
        1,
        2_000_000_000,
        uid,
        1,
        "/tools/tlapm",
    )
    inspections = 0

    def process_rows() -> list[guard.ProcessRow]:
        nonlocal inspections
        inspections += 1
        return [foreign] if inspections >= 3 else []

    finalize_calls = 0

    def finalize() -> int:
        nonlocal finalize_calls
        finalize_calls += 1
        return 1

    monkeypatch.setattr(guard, "_process_rows", process_rows)
    monkeypatch.setattr(guard.sys, "platform", "linux")

    status = guard._run_guarded(
        ["/usr/bin/true"],
        report_path=jsonl,
        summary_path=summary,
        sample_interval_seconds=60,
        post_success_finalize=finalize,
        post_run_cleanup=lambda: 0,
    )

    document = json.loads(summary.read_text(encoding="utf-8"))
    conflict = next(
        event for event in _events(jsonl) if event["event"] == "foreign_heavy_job"
    )
    assert status == guard.FOREIGN_JOB_EXIT_CODE
    assert document["exit_reason"] == "foreign_heavy_job"
    assert document["exit_status"] == guard.FOREIGN_JOB_EXIT_CODE
    assert document["post_success_finalize"] == "skipped"
    assert document["post_run_cleanup"] == "completed"
    assert finalize_calls == 0
    assert conflict["phase"] == "final_success_gate"
    assert conflict["foreign_process_group_id"] == foreign.process_group_id
    assert conflict["owned_process_group_id"] is None


def test_memory_limit_path_stops_remeasures_then_kills(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    actions: list[object] = []

    class FakeProcess:
        pid = 111

    monkeypatch.setattr(
        guard,
        "_signal_process_group",
        lambda pgid, signum: actions.append(("signal", pgid, signum)),
    )
    monkeypatch.setattr(
        guard,
        "_kill_owned_group_immediately",
        lambda _process, pgid: actions.append(("kill", pgid)),
    )

    def remeasure() -> guard.MemorySample:
        actions.append("remeasure")
        return guard.MemorySample(3, 2, 3, 1, "kernel")

    sample = guard._stop_remeasure_then_kill_owned_group(
        FakeProcess(), 222, remeasure
    )

    assert sample.memory_bytes == 3
    assert actions == [
        ("signal", 222, signal.SIGSTOP),
        "remeasure",
        ("kill", 222),
    ]


def test_memory_limit_terminates_owned_group_and_exits_75(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert guard.MAX_MEMORY_BYTES == 2 * 1024 * 1024 * 1024
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    monkeypatch.setattr(
        guard,
        "_sample_group",
        lambda _pgid, _rows=None, **_options: guard.MemorySample(
            2, 2, 0, 1, "rss"
        ),
    )
    immediate_kills = 0
    kill_immediately = guard._kill_owned_group_immediately

    def record_immediate_kill(
        process: subprocess.Popen[bytes], process_group_id: int
    ) -> None:
        nonlocal immediate_kills
        immediate_kills += 1
        kill_immediately(process, process_group_id)

    monkeypatch.setattr(guard, "_kill_owned_group_immediately", record_immediate_kill)

    status = guard._run_guarded(
        ["/bin/sleep", "30"],
        report_path=jsonl,
        summary_path=summary,
        memory_limit_bytes=1,
        sample_interval_seconds=0.01,
    )

    assert status == guard.MEMORY_LIMIT_EXIT_CODE
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "memory_limit"
    assert document["exit_status"] == 75
    assert document["peak_memory_bytes"] == 2
    assert document["kernel_peak_rss_bytes"] > 0
    assert immediate_kills == 1
    assert any(
        event["event"] == "memory_limit_remeasure" for event in _events(jsonl)
    )
    spawn = next(event for event in _events(jsonl) if event["event"] == "spawn")
    _wait_for_process_group_exit(int(spawn["process_group_id"]))


def test_wait4_rss_units_are_normalized_to_bytes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(guard.sys, "platform", "darwin")
    assert guard._normalized_wait4_max_rss_bytes(123) == 123

    monkeypatch.setattr(guard.sys, "platform", "linux")
    assert guard._normalized_wait4_max_rss_bytes(123) == 123 * 1024


def test_kernel_high_water_mark_catches_a_spike_between_polls(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    monkeypatch.setattr(
        guard,
        "_sample_group",
        lambda _pgid, _rows=None, **_options: guard.MemorySample(
            0, 0, 0, 1, "rss"
        ),
    )

    status = guard._run_guarded(
        ["/usr/bin/true"],
        report_path=jsonl,
        summary_path=summary,
        memory_limit_bytes=1,
        sample_interval_seconds=10,
    )

    assert status == guard.MEMORY_LIMIT_EXIT_CODE
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "kernel_memory_limit"
    assert document["kernel_peak_rss_bytes"] > 1
    assert document["kernel_peak_rss_method"] == "wait4_ru_maxrss"
    assert document["evidence_peak_rss_bytes"] == document["kernel_peak_rss_bytes"]


def test_report_context_child_environment_and_validation_are_bound_to_summary(
    tmp_path: Path,
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    observed = tmp_path / "environment"
    environment = os.environ.copy()
    environment["IROHA_GUARD_TEST_VALUE"] = "sealed-value"

    status = guard._run_guarded(
        [
            sys.executable,
            "-c",
            "import os,pathlib,sys; pathlib.Path(sys.argv[1]).write_text("
            "os.environ['IROHA_GUARD_TEST_VALUE'])",
            str(observed),
        ],
        report_path=jsonl,
        summary_path=summary,
        report_context={"fixture": {"sha256": "ab" * 32}},
        child_environment=environment,
        post_run_validation=lambda: None,
    )

    assert status == 0
    assert observed.read_text(encoding="utf-8") == "sealed-value"
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["report_context"] == {"fixture": {"sha256": "ab" * 32}}
    assert document["post_run_validation"] == "completed"
    start = next(event for event in _events(jsonl) if event["event"] == "start")
    assert start["report_context"] == document["report_context"]


def test_post_run_validation_failure_fails_closed(tmp_path: Path) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"

    def validate() -> None:
        raise guard.GuardError("fixture identity changed")

    status = guard._run_guarded(
        ["/usr/bin/true"],
        report_path=jsonl,
        summary_path=summary,
        post_run_validation=validate,
    )

    assert status == 1
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "post_run_validation_error"
    assert document["post_run_validation"] == "failed"


def test_failed_run_preserves_reason_and_skips_finalize_before_cleanup(
    tmp_path: Path,
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    order: list[str] = []

    def validate() -> None:
        order.append("validate")
        raise guard.GuardError("fixture validation failure")

    def finalize() -> int:
        order.append("finalize")
        return 1

    def cleanup() -> int:
        order.append("cleanup")
        return 0

    status = guard._run_guarded(
        ["/usr/bin/false"],
        report_path=jsonl,
        summary_path=summary,
        post_run_validation=validate,
        post_success_finalize=finalize,
        post_run_cleanup=cleanup,
    )

    assert status == 1
    assert order == ["validate", "cleanup"]
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "child_exit"
    assert document["post_run_validation"] == "failed"
    assert document["post_success_finalize"] == "skipped"
    assert document["post_run_cleanup"] == "completed"


def test_post_run_cleanup_is_bound_into_final_status_and_summary(tmp_path: Path) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    calls = 0

    def cleanup() -> int:
        nonlocal calls
        calls += 1
        return 2

    status = guard._run_guarded(
        ["/usr/bin/true"],
        report_path=jsonl,
        summary_path=summary,
        post_run_cleanup=cleanup,
    )

    assert status == 0
    assert calls == 1
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["post_run_cleanup"] == "completed"
    assert document["post_run_cleanup_removed"] == 2


def test_post_run_cleanup_failure_fails_closed(tmp_path: Path) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"

    def cleanup() -> int:
        raise guard.GuardError("fixture cleanup failure")

    status = guard._run_guarded(
        ["/usr/bin/true"],
        report_path=jsonl,
        summary_path=summary,
        post_run_cleanup=cleanup,
    )

    assert status == 1
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "post_run_cleanup_error"
    assert document["post_run_cleanup"] == "failed"
    assert document["post_run_cleanup_removed"] is None


def test_signal_terminates_child_and_grandchild_and_emits_summary(tmp_path: Path) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    pids = tmp_path / "pids"
    child_source = (
        "import os, pathlib, subprocess, sys, time; "
        "child=subprocess.Popen(['/bin/sleep','60']); "
        "pathlib.Path(sys.argv[1]).write_text(f'{os.getpid()} {child.pid}\\n'); "
        "time.sleep(60)"
    )
    process = subprocess.Popen(
        [
            sys.executable,
            str(GUARD_PATH),
            "--jsonl",
            str(jsonl),
            "--summary",
            str(summary),
            "--",
            sys.executable,
            "-c",
            child_source,
            str(pids),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    _wait_for(pids)
    child_pid, grandchild_pid = map(
        int, pids.read_text(encoding="utf-8").split()
    )

    process.send_signal(signal.SIGTERM)
    stdout, stderr = process.communicate(timeout=10)

    assert stdout == ""
    assert process.returncode == 128 + signal.SIGTERM, stderr
    document = json.loads(summary.read_text(encoding="utf-8"))
    assert document["exit_reason"] == "signal"
    assert document["exit_status"] == 128 + signal.SIGTERM
    _wait_for_process_group_exit(child_pid)
    for pid in (child_pid, grandchild_pid):
        with pytest.raises(ProcessLookupError):
            os.kill(pid, 0)


def test_supervisor_sigkill_triggers_lifeline_cleanup_and_holds_lock(
    tmp_path: Path,
) -> None:
    jsonl = tmp_path / "resource.jsonl"
    summary = tmp_path / "summary.json"
    pids = tmp_path / "pids"
    grandchild_source = (
        "import signal,time; "
        "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
        "time.sleep(60)"
    )
    child_source = (
        "import os,pathlib,signal,subprocess,sys,time; "
        "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
        f"child=subprocess.Popen([sys.executable,'-c',{grandchild_source!r}]); "
        "pathlib.Path(sys.argv[1]).write_text(f'{os.getpid()} {child.pid}\\n'); "
        "time.sleep(60)"
    )
    process = subprocess.Popen(
        [
            sys.executable,
            str(GUARD_PATH),
            "--jsonl",
            str(jsonl),
            "--summary",
            str(summary),
            "--",
            sys.executable,
            "-c",
            child_source,
            str(pids),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    _wait_for(pids)
    child_pid, grandchild_pid = map(
        int, pids.read_text(encoding="utf-8").split()
    )
    spawn = next(event for event in _events(jsonl) if event["event"] == "spawn")
    wrapper_pid = int(spawn["wrapper_pid"])
    assert child_pid == int(spawn["process_group_id"])
    assert wrapper_pid != child_pid

    try:
        process.kill()
        process.wait(timeout=5)
        with pytest.raises(guard.LockUnavailable):
            with guard._host_lock(
                guard.HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
            ):
                pass
        _wait_for_process_group_exit(child_pid, timeout=10)
        _wait_for_process_exit(wrapper_pid, timeout=10)
        for pid in (child_pid, grandchild_pid):
            with pytest.raises(ProcessLookupError):
                os.kill(pid, 0)
        with guard._host_lock(
            guard.HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
        ):
            pass
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        try:
            os.killpg(child_pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def test_normal_completion_and_lingering_descendant_are_distinguished(
    tmp_path: Path,
) -> None:
    completed_jsonl = tmp_path / "completed.jsonl"
    completed_summary = tmp_path / "completed-summary.json"
    assert (
        guard._run_guarded(
            ["/usr/bin/true"],
            report_path=completed_jsonl,
            summary_path=completed_summary,
            sample_interval_seconds=0.01,
        )
        == 0
    )
    assert json.loads(completed_summary.read_text(encoding="utf-8"))[
        "exit_reason"
    ] == "completed"

    lingering_jsonl = tmp_path / "lingering.jsonl"
    lingering_summary = tmp_path / "lingering-summary.json"
    status = guard._run_guarded(
        ["/bin/sh", "-c", "/bin/sleep 30 &"],
        report_path=lingering_jsonl,
        summary_path=lingering_summary,
        sample_interval_seconds=0.01,
    )
    assert status == 1
    assert json.loads(lingering_summary.read_text(encoding="utf-8"))[
        "exit_reason"
    ] == "lingering_process_group"


def test_runner_self_wraps_and_defaults_to_one_thread() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")
    assert 'exec python3 "$RESOURCE_GUARD"' in source
    assert "unset SUMERAGI_TLAPS_SUPERVISOR_PID" in source
    assert "IROHA_RESOURCE_GUARD_AUTH_FD" in source
    assert "IROHA_RESOURCE_GUARD_AUTH_TOKEN" in source
    assert "RESOURCE_AUTH_MAGIC" in source
    assert "readonly TLAPM_THREADS=1" in source
    assert '"${SUMERAGI_TLAPS_THREADS:-1}" != 1' in source
    assert "SUMERAGI_TLAPS_THREADS must equal 1" in source


@pytest.mark.parametrize(
    ("auth_fd", "auth_token"),
    [
        ("", "0" * 64),
        ("0", "0" * 64),
        ("9", "not-a-token"),
    ],
)
def test_runner_rejects_partial_or_forged_authorization_environment(
    tmp_path: Path, auth_fd: str, auth_token: str
) -> None:
    runner = tmp_path / RUNNER_PATH.name
    shutil.copy2(RUNNER_PATH, runner)
    environment = os.environ.copy()
    environment["IROHA_RESOURCE_GUARD_AUTH_FD"] = auth_fd
    environment["IROHA_RESOURCE_GUARD_AUTH_TOKEN"] = auth_token

    result = subprocess.run(
        ["/bin/bash", str(runner)],
        cwd=tmp_path,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=5,
    )

    assert result.returncode == 1
    assert "resource-guard authorization" in result.stderr


def test_runner_executes_its_body_through_guard_with_one_thread(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    formal_scripts = repo / "scripts" / "formal"
    formal_sources = repo / "formal" / "sumeragi_v2"
    formal_scripts.mkdir(parents=True)
    formal_sources.mkdir(parents=True)
    shutil.copy2(RUNNER_PATH, formal_scripts / RUNNER_PATH.name)
    shutil.copy2(GUARD_PATH, formal_scripts / GUARD_PATH.name)

    checker = formal_scripts / "check_sumeragi_v2_proof_ledger.py"
    checker.write_text(
        textwrap.dedent(
            """\
            from pathlib import Path
            import sys

            args = sys.argv[1:]
            if args == ["--print-source-manifest-sha256"]:
                print("a" * 64)
            elif args == ["--print-proof-modules"]:
                print("FixtureFirstProof")
                print("FixtureMiddleProof")
                print("FixtureFinalProof")
            elif "--write-evidence" in args:
                output = Path(args[args.index("--write-evidence") + 1])
                output.write_text('{"backend_verification":true}\\n', encoding="utf-8")
            elif args:
                raise SystemExit(64)
            """
        ),
        encoding="utf-8",
    )
    tlapm_args = tmp_path / "tlapm-args"
    tlapm = tmp_path / "tlapm"
    tlapm.write_text(
        textwrap.dedent(
            f"""\
            #!/bin/bash
            set -eu
            [[ -z "${{IROHA_RESOURCE_GUARD_AUTH_FD:-}}" ]]
            [[ -z "${{IROHA_RESOURCE_GUARD_AUTH_TOKEN:-}}" ]]
            [[ -z "${{SUMERAGI_TLAPS_SUPERVISOR_PID:-}}" ]]
            if [[ "${{1:-}}" == --version ]]; then
              printf '%s\\n' 3ab43c7
              exit 0
            fi
            printf '%s\\n' "$*" >> {tlapm_args}
            printf '%s\\n' '[INFO]: All 1 obligation proved.'
            """
        ),
        encoding="utf-8",
    )
    tlapm.chmod(0o755)

    environment = os.environ.copy()
    environment["TLAPM_BIN"] = str(tlapm)
    environment.pop("SUMERAGI_TLAPS_THREADS", None)
    # The retired PID marker exactly matches the direct shell's parent, but it
    # is no longer accepted as authorization and therefore cannot skip wrapping.
    environment["SUMERAGI_TLAPS_SUPERVISOR_PID"] = str(os.getpid())
    result = subprocess.run(
        ["/bin/bash", str(formal_scripts / RUNNER_PATH.name)],
        cwd=repo,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=20,
    )

    assert result.returncode == 0, result.stderr
    invocations = tlapm_args.read_text(encoding="utf-8").splitlines()
    assert len(invocations) == 6
    assert all(
        "--summary -N --strict --threads 1" in invocation
        for invocation in invocations[:3]
    )
    assert all(
        "--strict --nofp --threads 1" in invocation
        for invocation in invocations[3:]
    )
    assert all("--cache-dir" in invocation for invocation in invocations)
    assert "FixtureFirstProof.tla" in invocations[0]
    assert "FixtureMiddleProof.tla" in invocations[1]
    assert "FixtureFinalProof.tla" in invocations[2]
    assert "FixtureFirstProof.tla" in invocations[3]
    assert "FixtureMiddleProof.tla" in invocations[4]
    assert "FixtureFinalProof.tla" in invocations[5]
    evidence = repo / "target" / "formal" / "sumeragi_v2"
    assert (evidence / "tlaps" / "FixtureFirstProof.preflight.log").is_file()
    assert (evidence / "tlaps" / "FixtureMiddleProof.preflight.log").is_file()
    assert (evidence / "tlaps" / "FixtureFinalProof.preflight.log").is_file()
    assert not (evidence / "tlaps-cache").exists()
    summary = json.loads(
        (evidence / "tlaps_resource_summary.json").read_text(encoding="utf-8")
    )
    assert summary["exit_reason"] == "completed"
    assert summary["exit_status"] == 0
    assert any(
        event["event"] == "sample"
        for event in _events(evidence / "tlaps_resource.jsonl")
    )
