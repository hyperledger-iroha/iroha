"""Exercise collector exception cleanup with simulated children and signals only."""

from __future__ import annotations

import contextlib
import importlib.util
import signal
import sys
import types
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT / "scripts" / "run_kagemusha_v1_release_evidence.py"
SPEC = importlib.util.spec_from_file_location("kagemusha_cleanup_runner", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUNNER
SPEC.loader.exec_module(RUNNER)


class _Child:
    """Model one group leader and a descendant without starting any process."""

    def __init__(self, stage: str, error: BaseException) -> None:
        self.stage = stage
        self.error = error
        self.injected = False
        self.process = types.SimpleNamespace(pid=74123, returncode=None)
        self.usage = types.SimpleNamespace(ru_utime=0.01, ru_stime=0.01, ru_maxrss=4096)
        self.reaped = False
        self.group_alive = True
        self.events: list[tuple[object, ...]] = []

    def fault(self, stage: str) -> None:
        if self.stage == stage and not self.injected:
            self.injected = True
            raise self.error

    def wait4(self, pid: int, flags: int) -> tuple[object, ...]:
        self.events.append(("wait", pid, flags))
        self.fault("wait")
        if self.reaped:
            raise AssertionError("already reaped leader was waited a second time")
        if self.stage in {"fstat", "wait"} and self.group_alive:
            return 0, 0, None
        self.reaped = True
        return pid, 0, self.usage

    def killpg(self, pid: int, sig: int) -> None:
        self.events.append(("signal", pid, sig))
        if pid != self.process.pid:
            raise AssertionError("signal escaped the owned child group")
        if not self.group_alive:
            raise ProcessLookupError("simulated group is gone")
        if sig in {signal.SIGTERM, signal.SIGKILL}:
            self.group_alive = False

    def clock(self) -> float:
        if self.reaped:
            self.fault("after_reap")
        return 0.0

    def fstat(self, descriptor: int) -> object:
        self.events.append(("fstat", descriptor))
        self.fault("fstat")
        return types.SimpleNamespace(st_size=1)

    def fsync(self, descriptor: int) -> None:
        self.events.append(("fsync", descriptor))
        self.fault(f"fsync_{descriptor}")

    def capture(self, descriptor: int, *_args: object, **_kwargs: object) -> object:
        self.events.append(("capture", descriptor))
        self.fault(f"capture_{descriptor}")
        return types.SimpleNamespace(size=1)

    def close(self, descriptor: int) -> None:
        self.events.append(("close", descriptor))

    @contextlib.contextmanager
    def patched(self):
        # No bootstrap is required: transcript capture is simulated, so this
        # exercises the runner's real control flow without importing verifiers.
        with (
            mock.patch.object(RUNNER, "_mkdir_private"),
            mock.patch.object(RUNNER.Path, "exists", return_value=False),
            mock.patch.object(RUNNER.os, "open", side_effect=[101, 102]),
            mock.patch.object(RUNNER.os, "close", side_effect=self.close),
            mock.patch.object(RUNNER.os, "fstat", side_effect=self.fstat),
            mock.patch.object(RUNNER.os, "fsync", side_effect=self.fsync),
            mock.patch.object(RUNNER.os, "wait4", side_effect=self.wait4),
            mock.patch.object(RUNNER.os, "killpg", side_effect=self.killpg),
            mock.patch.object(RUNNER.os, "kill", side_effect=AssertionError("no individual signals")),
            mock.patch.object(RUNNER.subprocess, "Popen", return_value=self.process) as launch,
            mock.patch.object(RUNNER.time, "monotonic", side_effect=self.clock),
            mock.patch.object(RUNNER.time, "sleep", side_effect=AssertionError("unexpected polling")),
            mock.patch.object(RUNNER, "_stable_transcript_from_fd", side_effect=self.capture),
            mock.patch.object(RUNNER, "_terminate_process_group", wraps=RUNNER._terminate_process_group) as stop,
            mock.patch.object(RUNNER, "_quiesce_process_group", wraps=RUNNER._quiesce_process_group) as quiesce,
        ):
            yield launch, stop, quiesce

    def run(self) -> object:
        return RUNNER._run_process(
            Path("/simulated/administrator-tool"), [], cwd=ROOT,
            stdout_path=Path("/simulated/stdout"), stderr_path=Path("/simulated/stderr"),
            timeout_ms=1000, transcript_limit=1024, require_nonempty_streams=True,
        )


class ProcessExceptionCleanupTest(unittest.TestCase):
    """A failing observation must not leave the command or its descendants live."""

    def assert_closed(self, child: _Child) -> None:
        self.assertEqual([event for event in child.events if event[0] == "close"],
                         [("close", 101), ("close", 102)])

    def test_live_observation_failure_terminates_reaps_and_preserves_original(self) -> None:
        for stage in ("wait", "fstat"):
            for exception in (OSError, KeyboardInterrupt):
                with self.subTest(stage=stage, exception=exception.__name__):
                    original = exception("observation failed")
                    child = _Child(stage, original)
                    with child.patched() as (launch, stop, quiesce):
                        with self.assertRaises(exception) as raised:
                            child.run()
                        self.assertIs(raised.exception, original)
                        stop.assert_called_once_with(child.process)
                        quiesce.assert_called_once_with(child.process.pid, leader=child.process)
                        self.assertTrue(launch.call_args.kwargs["start_new_session"])
                    self.assertTrue(child.reaped)
                    self.assertFalse(child.group_alive)
                    self.assert_closed(child)
                    final_signal = max(index for index, event in enumerate(child.events)
                                       if event[0] == "signal")
                    first_close = next(index for index, event in enumerate(child.events)
                                       if event[0] == "close")
                    self.assertLess(final_signal, first_close)

    def test_interrupt_immediately_after_reap_only_quiesces_descendants(self) -> None:
        original = KeyboardInterrupt("clock interrupted after wait4 reaped")
        child = _Child("after_reap", original)
        with child.patched() as (_launch, stop, quiesce):
            with self.assertRaises(KeyboardInterrupt) as raised:
                child.run()
            self.assertIs(raised.exception, original)
            stop.assert_not_called()
            quiesce.assert_called_once_with(child.process.pid, leader=child.process)
        self.assertEqual(child.process.returncode, 0)
        self.assertEqual(len([event for event in child.events if event[0] == "wait"]), 1)
        self.assertFalse(child.group_alive)
        self.assert_closed(child)

    def test_transcript_failure_never_waits_or_terminates_reaped_leader(self) -> None:
        for stage in ("fsync_101", "fsync_102", "capture_101", "capture_102"):
            with self.subTest(stage=stage):
                original = OSError("transcript capture failed")
                child = _Child(stage, original)
                with child.patched() as (_launch, stop, quiesce):
                    with self.assertRaises(OSError) as raised:
                        child.run()
                    self.assertIs(raised.exception, original)
                    stop.assert_not_called()
                    self.assertGreaterEqual(quiesce.call_count, 1)
                    quiesce.assert_any_call(child.process.pid, leader=child.process)
                self.assertEqual(len([event for event in child.events if event[0] == "wait"]), 1)
                self.assertFalse(child.group_alive)
                self.assert_closed(child)

    def test_termination_failure_still_attempts_quiescence_and_closes_fds(self) -> None:
        original = OSError("monitor failed")
        cleanup = PermissionError("termination denied")
        child = _Child("wait", original)
        with child.patched() as (_launch, _stop, quiesce):
            with mock.patch.object(RUNNER, "_terminate_process_group", side_effect=cleanup):
                with self.assertRaises(PermissionError) as raised:
                    child.run()
                self.assertIs(raised.exception, cleanup)
                self.assertIs(cleanup.__context__, original)
                quiesce.assert_called_once_with(child.process.pid, leader=child.process)
        self.assertFalse(child.group_alive)
        self.assertTrue(child.reaped, "the group leader must be reaped after descendant quiescence")
        self.assert_closed(child)

    def test_quiescence_failure_surfaces_with_original_exception_context(self) -> None:
        original = OSError("wait observation failed")
        cleanup = RuntimeError("descendant survived cleanup")
        child = _Child("wait", original)
        with child.patched():
            with mock.patch.object(RUNNER, "_quiesce_process_group", side_effect=cleanup):
                with self.assertRaises(RuntimeError) as raised:
                    child.run()
                self.assertIs(raised.exception, cleanup)
                self.assertIs(cleanup.__context__, original)
        self.assertTrue(child.reaped)
        self.assert_closed(child)

    def test_stdout_close_failure_still_attempts_stderr_close(self) -> None:
        for observation_failed in (False, True):
            with self.subTest(observation_failed=observation_failed):
                original = OSError("observation failed before descriptor cleanup")
                close_error = OSError("stdout descriptor close failed")
                child = _Child("wait" if observation_failed else "unused", original)

                def close(descriptor: int) -> None:
                    child.close(descriptor)
                    if descriptor == 101:
                        raise close_error

                with child.patched():
                    with mock.patch.object(RUNNER.os, "close", side_effect=close):
                        with self.assertRaises(OSError) as raised:
                            child.run()
                        self.assertIs(raised.exception, close_error)
                        self.assertIs(close_error.__context__, original if observation_failed else None)
                self.assertTrue(child.reaped)
                self.assertFalse(child.group_alive)
                self.assert_closed(child)

    def test_spawn_failure_closes_fds_without_targeting_any_process_group(self) -> None:
        original = OSError("executable could not start")
        child = _Child("unused", original)
        with child.patched() as (launch, stop, quiesce):
            launch.side_effect = original
            with self.assertRaises(OSError) as raised:
                child.run()
            self.assertIs(raised.exception, original)
            stop.assert_not_called()
            quiesce.assert_not_called()
        self.assertFalse(any(event[0] in {"wait", "signal"} for event in child.events))
        self.assert_closed(child)

    def test_quiescence_reaps_zombie_leader_before_waiting_for_group_disappearance(self) -> None:
        for exit_stage in ("already_exited", "term", "kill"):
            with self.subTest(exit_stage=exit_stage):
                process = types.SimpleNamespace(pid=74123, returncode=None)
                usage = types.SimpleNamespace(ru_utime=0.01, ru_stime=0.01, ru_maxrss=4096)
                state = {"exited": exit_stage == "already_exited", "reaped": False, "clock": 0.0}
                events: list[tuple[object, ...]] = []

                def wait4(pid: int, flags: int) -> tuple[object, ...]:
                    self.assertEqual((pid, flags), (process.pid, RUNNER.os.WNOHANG))
                    self.assertFalse(state["reaped"], "quiescence waited for an already reaped leader")
                    events.append(("wait", pid, flags))
                    if state["exited"]:
                        state["reaped"] = True
                        return pid, 7 << 8, usage
                    return 0, 0, None

                def killpg(pid: int, sig: int) -> None:
                    self.assertEqual(pid, process.pid)
                    events.append(("signal", pid, sig))
                    # Unlike the simpler observation fixture, this group stays
                    # visible after exit until its zombie leader is collected.
                    if state["reaped"]:
                        raise ProcessLookupError("the reaped zombie no longer keeps the group visible")
                    if sig == signal.SIGKILL or (sig == signal.SIGTERM and exit_stage == "term"):
                        state["exited"] = True

                def clock() -> float:
                    state["clock"] += 0.25
                    return state["clock"]

                with (
                    mock.patch.object(RUNNER.os, "wait4", side_effect=wait4),
                    mock.patch.object(RUNNER.os, "killpg", side_effect=killpg),
                    mock.patch.object(RUNNER.os, "kill", side_effect=AssertionError("no individual signals")),
                    mock.patch.object(RUNNER.subprocess, "Popen", side_effect=AssertionError("no real child")),
                    mock.patch.object(RUNNER.time, "monotonic", side_effect=clock),
                    mock.patch.object(RUNNER.time, "sleep"),
                ):
                    RUNNER._quiesce_process_group(process.pid, leader=process)
                    waits = len([event for event in events if event[0] == "wait"])
                    RUNNER._quiesce_process_group(process.pid, leader=process)
                    self.assertEqual(len([event for event in events if event[0] == "wait"]), waits)
                self.assertTrue(state["reaped"])
                self.assertEqual(process.returncode, 7)
                self.assertIs(process._kagemusha_usage, usage)
                sent = [event[2] for event in events if event[0] == "signal" and event[2] != 0]
                expected = {"already_exited": [], "term": [signal.SIGTERM],
                            "kill": [signal.SIGTERM, signal.SIGKILL]}[exit_stage]
                self.assertEqual(sent, expected)

    def test_quiescence_rejects_mismatched_leader_without_waiting_or_signaling(self) -> None:
        process = types.SimpleNamespace(pid=74123, returncode=None)
        with (
            mock.patch.object(RUNNER.os, "wait4", side_effect=AssertionError("must not wait")) as wait,
            mock.patch.object(RUNNER.os, "killpg", side_effect=AssertionError("must not signal")) as signal_group,
            mock.patch.object(RUNNER.time, "monotonic", side_effect=AssertionError("must not start cleanup")) as clock,
        ):
            with self.assertRaisesRegex(RUNNER.KagemushaRunnerError, "does not belong"):
                RUNNER._quiesce_process_group(process.pid + 1, leader=process)
            wait.assert_not_called()
            signal_group.assert_not_called()
            clock.assert_not_called()
        self.assertIsNone(process.returncode)


if __name__ == "__main__":
    unittest.main()
