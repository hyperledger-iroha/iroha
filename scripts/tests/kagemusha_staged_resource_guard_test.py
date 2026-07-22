#!/usr/bin/env python3
"""Tests for the Kagemusha V4 staged resource guard."""

from __future__ import annotations

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

import kagemusha_staged_resource_guard as guard  # noqa: E402


class KagemushaStagedResourceGuardTests(unittest.TestCase):
    """Exercise accounting, handshake, and owned-child termination."""

    def test_owned_process_ids_include_descendants_and_process_group(self) -> None:
        rows = [
            (100, 1, 100, 10),
            (101, 100, 100, 20),
            (102, 101, 100, 30),
            (103, 1, 100, 40),
            (200, 1, 200, 50),
        ]
        self.assertEqual(guard.owned_process_ids(100, rows), [100, 101, 102, 103])

    def test_effective_limit_preserves_headroom(self) -> None:
        gib = guard.BYTES_PER_GIB
        self.assertEqual(
            guard.effective_limit_bytes(16 * gib, 20 * gib, 4 * gib), 16 * gib
        )
        self.assertEqual(
            guard.effective_limit_bytes(16 * gib, 10 * gib, 4 * gib), 6 * gib
        )
        self.assertEqual(guard.effective_limit_bytes(16 * gib, 3 * gib, 4 * gib), 0)

    def test_supervisor_only_stop_leaves_four_gib_termination_margin(self) -> None:
        gib = guard.BYTES_PER_GIB
        self.assertEqual(
            guard.soft_stop_bytes(16 * gib, kernel_limit_enforced=False), 12 * gib
        )
        self.assertEqual(
            guard.soft_stop_bytes(16 * gib, kernel_limit_enforced=True), 15 * gib
        )

    def test_memory_limit_cannot_raise_reviewed_ceiling(self) -> None:
        guard.validate_memory_limit_gib(16)
        with self.assertRaisesRegex(ValueError, "must not exceed"):
            guard.validate_memory_limit_gib(16.01)
        for non_finite in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(non_finite=non_finite):
                with self.assertRaisesRegex(ValueError, "finite"):
                    guard.validate_memory_limit_gib(non_finite)
        with self.assertRaisesRegex(ValueError, "too large"):
            guard.gib_to_bytes(1e308)

    def test_minimum_headroom_must_be_finite_and_non_negative(self) -> None:
        guard.validate_minimum_headroom_gib(0)
        guard.validate_minimum_headroom_gib(4)
        with self.assertRaisesRegex(ValueError, "must not be negative"):
            guard.validate_minimum_headroom_gib(-0.01)
        with self.assertRaisesRegex(ValueError, "must be finite"):
            guard.validate_minimum_headroom_gib(float("nan"))

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW"), "requires no-follow open")
    def test_heavy_job_lock_does_not_follow_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_text("do-not-truncate\n", encoding="utf-8")
            lock = root / "lock"
            lock.symlink_to(target)
            with self.assertRaises(OSError):
                with guard.acquire_heavy_job_lock(lock):
                    self.fail("symlinked lock must not be acquired")
            self.assertEqual(target.read_text(encoding="utf-8"), "do-not-truncate\n")

    def test_address_space_limit_never_raises_an_inherited_limit(self) -> None:
        if guard.resource is None or not hasattr(guard.resource, "RLIMIT_AS"):
            self.skipTest("address-space rlimit is unavailable")
        inherited_soft = 2 * guard.BYTES_PER_GIB
        inherited_hard = 3 * guard.BYTES_PER_GIB
        with (
            mock.patch.object(
                guard.resource,
                "getrlimit",
                return_value=(inherited_soft, inherited_hard),
            ),
            mock.patch.object(guard.resource, "setrlimit") as setrlimit,
        ):
            guard._limit_address_space(16 * guard.BYTES_PER_GIB)
        setrlimit.assert_called_once_with(
            guard.resource.RLIMIT_AS, (inherited_soft, inherited_hard)
        )

    @unittest.skipUnless(sys.platform == "darwin", "Darwin-specific launch path")
    def test_default_guard_uses_supervisor_when_darwin_rejects_rlimit_as(self) -> None:
        program = (
            "import os; fd=int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD']); "
            "os.write(fd, b'stage=darwin-supervisor-only\\n')"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.report["memory_enforcement_mode"], "supervisor")
        self.assertEqual(
            result.report["kernel_address_space_limit_enforced"], False
        )

    def test_macos_physical_memory_falls_back_to_sysctl(self) -> None:
        expected = 32 * guard.BYTES_PER_GIB
        completed = subprocess.CompletedProcess(
            ["sysctl", "-n", "hw.memsize"], 0, f"{expected}\n", ""
        )
        with (
            mock.patch.object(guard.sys, "platform", "darwin"),
            mock.patch.object(guard.os, "sysconf", side_effect=ValueError),
            mock.patch.object(guard, "SYSCTL", "/usr/sbin/sysctl"),
            mock.patch.object(guard.subprocess, "run", return_value=completed) as run,
        ):
            self.assertEqual(guard.total_physical_memory_bytes(), expected)
        self.assertEqual(
            run.call_args.args[0], ["/usr/sbin/sysctl", "-n", "hw.memsize"]
        )

    def test_sampling_intervals_cannot_disable_supervision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "sample interval must not exceed"):
                guard.run_guarded_command(
                    [sys.executable, "-c", "pass"],
                    report_path=root / "report.json",
                    sample_interval_seconds=guard.MAXIMUM_SAMPLE_INTERVAL_SECONDS + 1,
                    lock_path=root / "lock",
                )
            with self.assertRaisesRegex(
                ValueError, "footprint interval must not exceed"
            ):
                guard.run_guarded_command(
                    [sys.executable, "-c", "pass"],
                    report_path=root / "report.json",
                    footprint_interval_seconds=(
                        guard.MAXIMUM_FOOTPRINT_INTERVAL_SECONDS + 1
                    ),
                    lock_path=root / "lock",
                )

    def test_termination_escalates_for_residual_owned_group_only(self) -> None:
        class ExitedLeader:
            pid = 424242

            def wait(self, timeout: float | None = None) -> int:
                del timeout
                return 0

        process = ExitedLeader()
        with mock.patch.object(guard.os, "killpg") as killpg:
            guard.terminate_owned_process_group(process)  # type: ignore[arg-type]
        self.assertEqual(
            killpg.call_args_list,
            [
                mock.call(process.pid, signal.SIGTERM),
                mock.call(process.pid, 0),
                mock.call(process.pid, signal.SIGKILL),
            ],
        )

    def test_guard_requires_live_child_handshake(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", "pass"],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
            self.assertEqual(result.exit_code, 2)
            self.assertEqual(
                result.report["termination_reason"], "missing_child_guard_handshake"
            )

    def test_guard_accepts_handshake_and_writes_private_report(self) -> None:
        program = (
            "import os; fd=int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD']); "
            "os.write(fd, b'stage=test-handshake\\n')"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "report.json"
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=report,
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
            self.assertEqual(result.exit_code, 0)
            self.assertEqual(result.report["last_stage"], "stage=test-handshake")
            self.assertEqual(result.report["child_guard_handshake_received"], True)
            self.assertEqual(json.loads(report.read_text())["completed"], True)
            self.assertEqual(report.stat().st_mode & 0o777, 0o600)

    def test_guard_rejects_unstructured_pipe_write_as_handshake(self) -> None:
        program = (
            "import os; fd=int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD']); "
            "os.write(fd, b'not-a-stage\\n')"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
        self.assertEqual(result.exit_code, 2)
        self.assertEqual(
            result.report["termination_reason"], "missing_child_guard_handshake"
        )

    def test_guard_bounds_recorded_stage_events(self) -> None:
        stage_count = guard.MAX_RECORDED_STAGE_EVENTS + 17
        program = (
            "import os; "
            "fd=int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD']); "
            "[os.write(fd, f'stage={index}\\n'.encode()) "
            f"for index in range({stage_count})]"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.report["stage_event_count"], stage_count)
        self.assertEqual(
            len(result.report["stage_events"]), guard.MAX_RECORDED_STAGE_EVENTS
        )
        self.assertEqual(result.report["stage_events_dropped"], 17)
        self.assertEqual(result.report["last_stage"], f"stage={stage_count - 1}")

    @unittest.skipUnless(
        sys.platform.startswith(("linux", "darwin")), "POSIX signal test"
    )
    def test_guard_normalizes_child_signal_exit_status(self) -> None:
        program = (
            "import os, signal; "
            "fd=int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD']); "
            "os.write(fd, b'stage=signal-exit\\n'); "
            "os.kill(os.getpid(), signal.SIGTERM)"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
        self.assertEqual(result.report["child_exit_code"], -signal.SIGTERM)
        self.assertEqual(result.exit_code, 128 + signal.SIGTERM)

    @unittest.skipUnless(
        sys.platform.startswith(("linux", "darwin")), "POSIX group test"
    )
    def test_guard_rejects_and_stops_residual_owned_process_group(self) -> None:
        program = """
import os
import subprocess
import sys
fd = int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD'])
worker = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(5)'])
os.write(fd, f'stage=residual-worker-{worker.pid}\\n'.encode())
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.25,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
        self.assertEqual(result.exit_code, guard.GUARD_EXIT_CODE)
        self.assertEqual(
            result.report["termination_reason"], "residual_owned_process_group"
        )

    def test_runner_reports_missing_executable_without_traceback(self) -> None:
        runner = SCRIPTS / "run_kagemusha_v4_generation.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(runner),
                    "--report",
                    str(root / "report.json"),
                    "--minimum-headroom-gib",
                    "0",
                    "--",
                    str(root / "missing-command"),
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
        self.assertEqual(completed.returncode, 2)
        self.assertIn("resource guard refused to start", completed.stderr)
        self.assertNotIn("Traceback", completed.stderr)

    @unittest.skipUnless(sys.platform.startswith(("linux", "darwin")), "POSIX RSS test")
    def test_guard_stops_owned_allocator_at_soft_limit(self) -> None:
        program = """
import os
import time
fd = int(os.environ['IROHA_KAGEMUSHA_V4_GUARD_FD'])
os.write(fd, b'stage=synthetic-allocation\\n')
chunks = []
while True:
    chunks.append(bytearray(4 * 1024 * 1024))
    time.sleep(0.01)
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = guard.run_guarded_command(
                [sys.executable, "-c", program],
                report_path=root / "report.json",
                max_memory_gib=0.125,
                minimum_headroom_gib=0,
                sample_interval_seconds=0.02,
                footprint_interval_seconds=60,
                enforce_address_space=False,
                minimum_effective_bytes=1,
                lock_path=root / "lock",
            )
            self.assertEqual(result.exit_code, guard.GUARD_EXIT_CODE)
            self.assertEqual(
                result.report["termination_reason"], "child_memory_soft_limit"
            )
            self.assertGreater(result.report["max_process_tree_rss_bytes"], 0)


if __name__ == "__main__":
    unittest.main()
