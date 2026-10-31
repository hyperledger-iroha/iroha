"""Deterministic deadline boundaries for the actual harness observation loop.

Clock and original-child transitions are modelled. The actual journal and strict
window validator run over retained in-memory bytes; no child or native module is
started by these controls.
"""
import hashlib
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import zk_resource_harness as harness


class MemoryRecords:
    """Retain exactly published bytes for the genuine journal/reducer path."""

    def __init__(self):
        self.values = {}

    def publish(self, name, raw):
        if name in self.values:
            raise AssertionError("duplicate publication")
        self.values[name] = bytes(raw)
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def read(self, reference):
        raw = self.values[reference["path"]]
        if len(raw) != reference["bytes"] or hashlib.sha256(raw).hexdigest() != reference["sha256"]:
            raise AssertionError("changed original record")
        return raw


class DeadlineTests(unittest.TestCase):
    """Drive actual loop boundaries without subprocess or real-clock timing."""

    def scenario(self, kind):
        clock = SimpleNamespace(now=980_000_000 if kind in ("bounded_sleep", "wake_cross", "late_exit_during_wake") else 200_000_000,
                                sleeps=[], woke=False, reap_after_wake=False)
        calls, kills, retained = [], [], []
        first = SimpleNamespace(error=None)
        records = MemoryRecords()
        child = SimpleNamespace(pid=321, returncode=None)
        usage = SimpleNamespace(ru_utime=0.0, ru_stime=0.0, ru_maxrss=1)
        deliberate = harness.observer.ProcessObservationError("deliberate sample refusal")
        identity = {"pid": 321, "ppid": 123, "pgid": 456,
                    "birth": {"kind": "model", "counter": 1},
                    "executable_path": "/model/command", "executable_sha256": "a" * 64}

        def observation(start, finish):
            return {"started_monotonic_ns": start, "finished_monotonic_ns": finish,
                    "processes": [{"label": "measured_command", "identity": identity,
                                   "cpu_time_ns": 0, "cpu_counter_unit_ns": 1000, "rss_bytes": 4096}],
                    "cpu_time_ns": 0, "rss_bytes": 4096}

        def sleep(seconds):
            clock.sleeps.append(seconds)
            clock.now += round(seconds * 1_000_000_000)
            if kind in ("wake_cross", "late_exit_during_wake"):
                clock.now += 100_000_000
            clock.woke = True

        def reap():
            if clock.woke and not clock.reap_after_wake:
                clock.reap_after_wake = True
                if kind == "reap_cross":
                    clock.now = 1_100_000_000
                elif kind in ("exit_during_wake", "late_exit_during_wake"):
                    return (0, usage)
            if kills:
                return (9, usage)
            if calls:
                return (0, usage)
            return None

        def sample():
            calls.append(clock.now)
            if kind == "sample_error":
                raise deliberate
            start = clock.now
            clock.now = 1_100_000_000 if kind == "captured_overrun" else clock.now + 1_000_000
            return observation(start, clock.now)

        def retain(stage, error):
            if first.error is None:
                first.error = error
            if not any(prior is error for _, prior in retained):
                retained.append((stage, error))
            owner.first = first.error

        def kill(pid, *, reap):
            self.assertEqual((pid, reap), (321, False))
            kills.append(pid)
            return None

        owner = SimpleNamespace(started_ns=0, scope_reader=object(), reader=SimpleNamespace(cleanup_error=None),
                                retention_error=None, retain=retain, first=None,
                                try_reap=reap, diagnostics=lambda: list(retained))
        scope = SimpleNamespace(initial=observation(0, 1_000_000), observe=sample)
        reasons = []
        with mock.patch.object(harness.time, "monotonic_ns", lambda: clock.now), \
                mock.patch.object(harness.time, "sleep", sleep), \
                mock.patch.object(harness.os, "getpgid", lambda pid: 456), \
                mock.patch.object(harness, "_owned_child_monitor", lambda *args: None), \
                mock.patch.object(harness.observer, "ProcessScope", lambda *args: scope), \
                mock.patch.object(harness, "_kill_own_child", kill), \
                mock.patch.object(harness, "_publish_linux_owned", lambda *args: {}), \
                mock.patch.object(harness.subprocess, "Popen", side_effect=AssertionError("no child permitted")):
            result = harness.observe_child(child, object(), records, 50, 1000,
                                           exception_reasons=reasons, owned=owner)
        return SimpleNamespace(result=result, clock=clock, samples=calls, kills=kills,
                               first=first.error, retained=retained, records=records,
                               deliberate=deliberate, reasons=reasons, child=child)

    def test_wake_after_deadline_never_requests_live_sample(self):
        value = self.scenario("wake_cross")
        self.assertEqual(value.samples, [])
        self.assertEqual(value.kills, [321])
        self.assertTrue(value.result["timed_out"])
        self.assertIsNone(value.first)

    def test_sleep_is_bounded_by_unchanged_remaining_attempt_budget(self):
        value = self.scenario("bounded_sleep")
        self.assertEqual(value.clock.sleeps, [0.02])
        self.assertEqual(value.samples, [])
        self.assertEqual(value.result["exit"], {"kind": "signaled", "signal": 9})
        self.assertIsNone(value.first)

    def test_original_reap_crossing_deadline_precedes_any_sample(self):
        value = self.scenario("reap_cross")
        self.assertTrue(value.clock.reap_after_wake)
        self.assertEqual(value.samples, [])
        self.assertTrue(value.result["timed_out"])
        self.assertEqual(value.kills, [321])
        self.assertIsNone(value.first)

    def test_exit_during_wake_is_reaped_before_sampling_without_signal(self):
        value = self.scenario("exit_during_wake")
        self.assertEqual(value.samples, [])
        self.assertEqual(value.kills, [])
        self.assertEqual(value.result["exit"], {"kind": "exited", "code": 0})
        self.assertFalse(value.result["timed_out"])
        self.assertIsNone(value.first)

    def test_exit_already_terminal_after_late_wake_is_never_signalled(self):
        value = self.scenario("late_exit_during_wake")
        self.assertEqual(value.samples, [])
        self.assertEqual(value.kills, [])
        self.assertTrue(value.result["timed_out"])
        self.assertEqual(value.result["exit"], {"kind": "exited", "code": 0})
        self.assertEqual(value.child.returncode, 0)
        self.assertIsNone(value.first)

    def test_before_deadline_sample_uses_real_journal_and_validator(self):
        value = self.scenario("before_deadline")
        self.assertEqual(value.samples, [250_000_000])
        self.assertEqual(value.result["sampled"]["state"], "validated")
        self.assertEqual(value.result["sampled"]["observations"], 2)
        self.assertIn("observations/window.json", value.records.values)
        self.assertEqual(value.kills, [])
        self.assertIsNone(value.first)

    def test_attempted_sample_error_remains_exact_first_through_reap(self):
        value = self.scenario("sample_error")
        self.assertIs(value.first, value.deliberate)
        self.assertEqual(value.samples, [250_000_000])
        self.assertEqual(value.child.returncode, 0)
        self.assertEqual(value.result["sampled"]["state"], "failed")
        self.assertEqual(value.result["sampled"]["reason"], "linux_owned_observation_failed")
        self.assertEqual(value.retained[0], ("sample", value.deliberate))
        self.assertEqual(value.kills, [])

    def test_already_captured_overrun_is_retained_and_strictly_refused(self):
        value = self.scenario("captured_overrun")
        self.assertEqual(value.samples, [250_000_000])
        self.assertIsInstance(value.first, harness.observer.ProcessObservationError)
        self.assertEqual(str(value.first), "resource window exceeds the registered attempt budget")
        self.assertEqual(value.result["sampled"]["state"], "failed")
        self.assertNotIn("observations/window.json", value.records.values)
        chunk = harness.control.decode(value.records.values["observations/chunk-00000000000000000000.json"])
        self.assertEqual(len(chunk["observations"]), 2)
        self.assertEqual(chunk["observations"][-1]["finished_monotonic_ns"], 1_100_000_000)
        self.assertEqual(value.child.returncode, 0)


if __name__ == "__main__":
    unittest.main()
