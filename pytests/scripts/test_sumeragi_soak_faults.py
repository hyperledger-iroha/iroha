"""Unit tests of the soak's fault injection (``scripts/sumeragi_soak_faults.py``).

The fault plan, the link-condition arithmetic, the ``tc netem`` and volume commands are checked
as data; the userspace TCP proxy is exercised over real loopback sockets (forwarding, delay,
resets).
"""

from __future__ import annotations

import asyncio
import errno
import importlib.util
import random
import socket
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "sumeragi_soak_faults.py"
SPEC = importlib.util.spec_from_file_location("sumeragi_soak_faults_under_test", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
faults = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = faults
SPEC.loader.exec_module(faults)


def options(**overrides):
    values = dict(
        duration_s=3_600.0,
        warmup_s=60.0,
        final_quiet_s=300.0,
        kinds=("kill", "disk", "net"),
        validators=4,
        disk_node=3,
        gap_s=(30.0, 90.0),
        window_s=(10.0, 40.0),
        loss=(0.10, 0.30),
        max_kill=1,
    )
    values.update(overrides)
    return faults.PlanOptions(**values)


class PlanTests(unittest.TestCase):
    def test_the_plan_is_a_function_of_the_seed(self) -> None:
        first = faults.plan_faults(options(), random.Random(7))
        again = faults.plan_faults(options(), random.Random(7))
        other = faults.plan_faults(options(), random.Random(8))
        self.assertEqual(first, again)
        self.assertNotEqual(first, other)

    def test_windows_are_ordered_disjoint_and_leave_warmup_and_final_quiet(self) -> None:
        plan = faults.plan_faults(options(), random.Random(1))
        self.assertGreater(len(plan), 10)
        self.assertGreaterEqual(plan[0].start_s, 60.0)
        for earlier, later in zip(plan, plan[1:]):
            self.assertGreaterEqual(later.start_s - earlier.end_s, 30.0 - 1e-6)
        self.assertLessEqual(plan[-1].end_s + 300.0, 3_600.0 + 1e-6)
        self.assertEqual({fault.kind for fault in plan}, {"kill", "disk", "net"})

    def test_faults_respect_the_committee_bound_and_the_loss_range(self) -> None:
        plan = faults.plan_faults(options(validators=22, max_kill=9), random.Random(3))
        for fault in plan:
            if fault.kind == "kill":
                self.assertTrue(1 <= len(fault.nodes) <= 7)  # f = 7 at n = 22
                self.assertTrue(all(0 <= node < 22 for node in fault.nodes))
                self.assertEqual(len(set(fault.nodes)), len(fault.nodes))
            elif fault.kind == "disk":
                self.assertEqual(fault.nodes, (3,))
            else:
                self.assertTrue(0.10 <= fault.loss <= 0.30)
                self.assertEqual(fault.nodes, ())
        self.assertTrue(any(len(fault.nodes) > 1 for fault in plan if fault.kind == "kill"))

    def test_invalid_plans_are_refused(self) -> None:
        self.assertEqual(faults.plan_faults(options(kinds=()), random.Random(1)), [])
        with self.assertRaises(ValueError):
            faults.plan_faults(options(kinds=("flood",)), random.Random(1))
        with self.assertRaises(ValueError):
            faults.plan_faults(options(disk_node=None), random.Random(1))
        with self.assertRaises(ValueError):
            faults.plan_faults(options(validators=1), random.Random(1))
        self.assertEqual(faults.plan_faults(options(duration_s=100.0), random.Random(1)), [])

    def test_spikes_stay_inside_the_window(self) -> None:
        spikes = faults.spike_plan(random.Random(5), 120.0)
        self.assertTrue(spikes)
        for at, length, extra in spikes:
            self.assertTrue(0 <= at < 120.0 and at + length <= 120.0 + 1e-6)
            self.assertTrue(300.0 <= extra <= 1_500.0)


class ConditionTests(unittest.TestCase):
    def test_a_clear_link_adds_nothing(self) -> None:
        self.assertEqual(faults.chunk_delay_ms(faults.CLEAR, random.Random(1)), (0.0, 0))

    def test_loss_costs_doubling_retransmission_timeouts(self) -> None:
        delay, losses = faults.chunk_delay_ms(faults.LinkCondition(loss=0.999999), random.Random(1))
        self.assertEqual(losses, faults.MAX_RETRANSMISSIONS)
        self.assertEqual(delay, sum(faults.RTO_MS * 2**k for k in range(faults.MAX_RETRANSMISSIONS)))

    def test_delay_jitter_and_spike_add_up(self) -> None:
        condition = faults.LinkCondition(delay_ms=10.0, jitter_ms=5.0).with_spike(500.0)
        for seed in range(20):
            delay, losses = faults.chunk_delay_ms(condition, random.Random(seed))
            self.assertEqual(losses, 0)
            self.assertTrue(510.0 <= delay <= 515.0)

    def test_a_net_window_condition(self) -> None:
        condition = faults.net_condition(0.2, random.Random(2), reset_ratio=0.01)
        self.assertEqual(condition.loss, 0.2)
        self.assertAlmostEqual(condition.reset, 0.002)
        self.assertTrue(5.0 <= condition.delay_ms <= 25.0)


class NetemTests(unittest.TestCase):
    def test_setup_filters_only_the_p2p_ports_into_the_netem_band(self) -> None:
        commands = faults.netem_setup_commands([1337, 1338])
        self.assertEqual(commands[0], ["tc", "qdisc", "replace", "dev", "lo", "root", "handle", "1:", "prio"])
        self.assertIn("netem", commands[1])
        filters = [" ".join(command) for command in commands[2:]]
        self.assertEqual(len(filters), 4)
        for port in (1337, 1338):
            for direction in ("sport", "dport"):
                self.assertIn(f"match ip {direction} {port} 0xffff flowid 1:3", " ".join(filters))

    def test_change_renders_loss_delay_jitter_and_spike(self) -> None:
        command = faults.netem_change_command(faults.LinkCondition(loss=0.25, delay_ms=20.0, jitter_ms=5.0, spike_ms=800.0))
        self.assertEqual(command[-6:], ["netem", "delay", "820ms", "5ms", "loss", "25%"])
        clear = faults.netem_change_command(faults.CLEAR)
        self.assertEqual(clear[-5:], ["netem", "delay", "0ms", "loss", "0%"])

    def test_the_controller_runs_the_commands(self) -> None:
        calls = []
        netem = faults.Netem([9000], run=lambda command, check: calls.append((command, check)))
        netem.setup()
        netem.apply(faults.LinkCondition(loss=0.1))
        netem.teardown()
        self.assertEqual(calls[0][0], ["ip", "link", "set", "lo", "up"])
        self.assertEqual(calls[-1], (faults.netem_teardown_command(), False))
        self.assertTrue(all(check for _, check in calls[:-1]))


class DiskTests(unittest.TestCase):
    def test_volume_commands_per_platform(self) -> None:
        mount, unmount = faults.disk_mount_commands("darwin", Path("/r/disk.dmg"), Path("/r/state/peer3"), 256)
        self.assertEqual(mount[0][:2], ["hdiutil", "create"])
        self.assertIn("256m", mount[0])
        self.assertEqual(mount[1][:2], ["hdiutil", "attach"])
        self.assertEqual(unmount[:2], ["hdiutil", "detach"])
        mount, unmount = faults.disk_mount_commands("linux", Path("/r/disk.dmg"), Path("/r/state/peer3"), 64)
        self.assertEqual(mount, [["mount", "-t", "tmpfs", "-o", "size=64m,mode=0700", "tmpfs", "/r/state/peer3"]])
        self.assertEqual(unmount, ["umount", "-l", "/r/state/peer3"])
        with self.assertRaises(ValueError):
            faults.disk_mount_commands("win32", Path("a"), Path("b"), 1)

    def test_mount_moves_the_state_onto_the_volume_and_fill_stops_at_enospc(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            state = root / "state" / "peer3"
            (state / "storage").mkdir(parents=True)
            (state / "storage" / "blocks").write_text("kura")
            (state / "installation.log").write_text("log")
            calls = []
            volume = faults.DiskVolume(state, root, 16, platform="linux", run=lambda command, check: calls.append(command))
            volume.mount()
            self.assertEqual(calls[0][:3], ["mount", "-t", "tmpfs"])
            self.assertEqual((state / "storage" / "blocks").read_text(), "kura")
            self.assertEqual((state / "installation.log").read_text(), "log")
            self.assertFalse((root / "state" / "peer3.soak-staging").exists())

            budget = {"left": 3 * 1024 * 1024 + 100}
            real_open = open

            class Full:
                def __init__(self, handle):
                    self.handle = handle

                def write(self, data):
                    if budget["left"] <= 0:
                        raise OSError(errno.ENOSPC, "No space left on device")
                    count = min(len(data), budget["left"])
                    budget["left"] -= count
                    return self.handle.write(data[:count])

                def fileno(self):
                    return self.handle.fileno()

                def __enter__(self):
                    return self

                def __exit__(self, *exc):
                    self.handle.close()

            with mock.patch("builtins.open", lambda path, mode, buffering=-1: Full(real_open(path, mode, buffering=buffering))):
                written = volume.fill()
            self.assertEqual(written, 3 * 1024 * 1024 + 100)
            self.assertEqual(volume.filler.stat().st_size, written)
            volume.free()
            self.assertFalse(volume.filler.exists())
            volume.free()  # idempotent
            volume.unmount()
            self.assertEqual(calls[-1], ["umount", "-l", str(state)])


class ProxyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.server = socket.socket()
        self.server.bind(("127.0.0.1", 0))
        self.server.listen(8)
        self.target_port = self.server.getsockname()[1]
        self.accepted: list[socket.socket] = []
        self.echo = threading.Thread(target=self._echo, daemon=True)
        self.echo.start()
        probe = socket.socket()
        probe.bind(("127.0.0.1", 0))
        self.proxy_port = probe.getsockname()[1]
        probe.close()
        self.conditions = faults.ConditionBox()
        self.proxy = faults.ProxyNetwork([("127.0.0.1", self.proxy_port, "127.0.0.1", self.target_port)], self.conditions, seed=1)
        self.proxy.start()

    def tearDown(self) -> None:
        self.proxy.stop()
        self.server.close()
        for connection in self.accepted:
            connection.close()

    def _echo(self) -> None:
        while True:
            try:
                connection, _ = self.server.accept()
            except OSError:
                return
            self.accepted.append(connection)
            threading.Thread(target=self._serve, args=(connection,), daemon=True).start()

    @staticmethod
    def _serve(connection: socket.socket) -> None:
        try:
            while True:
                data = connection.recv(65536)
                if not data:
                    return
                connection.sendall(data)
        except OSError:
            return

    def roundtrip(self, payload: bytes) -> tuple[bytes, float]:
        with socket.create_connection(("127.0.0.1", self.proxy_port), timeout=10) as client:
            started = time.monotonic()
            client.sendall(payload)
            received = b""
            while len(received) < len(payload):
                chunk = client.recv(65536)
                if not chunk:
                    break
                received += chunk
            return received, time.monotonic() - started

    def test_a_clear_link_forwards_every_byte_in_order(self) -> None:
        payload = bytes(range(256)) * 1024
        received, _ = self.roundtrip(payload)
        self.assertEqual(received, payload)
        stats = self.proxy.stats.to_json()
        self.assertEqual(stats["connections"], 1)
        self.assertGreaterEqual(stats["bytes"], 2 * len(payload))

    def test_delay_applies_to_both_directions(self) -> None:
        self.conditions.set(faults.LinkCondition(delay_ms=150.0))
        received, elapsed = self.roundtrip(b"ping")
        self.assertEqual(received, b"ping")
        self.assertGreaterEqual(elapsed, 0.29)

    def test_resets_drop_the_connection(self) -> None:
        self.conditions.set(faults.LinkCondition(reset=1.0))
        with socket.create_connection(("127.0.0.1", self.proxy_port), timeout=10) as client:
            client.sendall(b"doomed")
            try:
                data = client.recv(16)
            except ConnectionResetError:
                data = b""
            self.assertEqual(data, b"")
        self.assertGreaterEqual(self.proxy.stats.to_json()["resets"], 1)
        self.conditions.set(faults.CLEAR)
        received, _ = self.roundtrip(b"after")
        self.assertEqual(received, b"after")

    def test_reset_all_cuts_open_connections(self) -> None:
        client = socket.create_connection(("127.0.0.1", self.proxy_port), timeout=10)
        try:
            client.sendall(b"x")
            self.assertEqual(client.recv(1), b"x")
            self.assertEqual(self.proxy.reset_all(), 1)
            try:
                data = client.recv(1)
            except ConnectionResetError:
                data = b""
            self.assertEqual(data, b"")
        finally:
            client.close()


class PumpBackpressureTests(unittest.IsolatedAsyncioTestCase):
    """Delayed forwarding retains bounded data and releases its two tasks."""

    class Reader:
        def __init__(self) -> None:
            self.reads = 0

        async def read(self, size: int) -> bytes:
            if self.reads == 64:
                return b""
            self.reads += 1
            return bytes([self.reads - 1]) * size

    class Writer:
        def __init__(self) -> None:
            self.waiting = asyncio.Event()
            self.resume = asyncio.Event()
            self.data: list[bytes] = []
            self.eof = False

        def write(self, data: bytes) -> None:
            self.data.append(data)

        async def drain(self) -> None:
            self.waiting.set()
            await self.resume.wait()

        def is_closing(self) -> bool:
            return False

        def write_eof(self) -> None:
            self.eof = True

    async def exercise_stalled_pump(self, operation: str) -> None:
        proxy = faults.ProxyNetwork([], faults.ConditionBox(), seed=1)
        reader = self.Reader()
        writer = self.Writer()
        pump = asyncio.create_task(proxy._pump(reader, writer, (writer, writer)))
        try:
            await asyncio.wait_for(writer.waiting.wait(), timeout=5)
            # Finish callbacks already ready when delivery entered its drain wait.
            boundary = asyncio.get_running_loop().create_future()
            asyncio.get_running_loop().call_soon(boundary.set_result, None)
            await boundary
            self.assertLessEqual(
                reader.reads,
                18,
                "delayed forwarding must retain at most 16 queued and two owned chunks",
            )
            if operation == "stop":
                self.assertEqual(reader.reads, 18)
                pump.cancel()
                with self.assertRaises(asyncio.CancelledError):
                    await asyncio.wait_for(pump, timeout=5)
            elif operation == "failure":
                self.assertEqual(reader.reads, 18)

                def fail(_data: bytes) -> None:
                    raise RuntimeError("original delivery failure")

                writer.write = fail
                writer.resume.set()
                with self.assertRaisesRegex(RuntimeError, "^original delivery failure$"):
                    await asyncio.wait_for(pump, timeout=5)
            else:
                writer.resume.set()
                await asyncio.wait_for(pump, timeout=5)
                self.assertEqual(reader.reads, 64)
                self.assertEqual(
                    writer.data,
                    [bytes([index]) * faults.CHUNK_BYTES for index in range(64)],
                )
                self.assertTrue(writer.eof)
                self.assertEqual(proxy.stats.to_json()["bytes"], 64 * faults.CHUNK_BYTES)
        finally:
            if not pump.done():
                writer.resume.set()
                await asyncio.wait_for(pump, timeout=5)
        self.assertEqual(
            [
                task
                for task in asyncio.all_tasks()
                if task is not asyncio.current_task() and not task.done()
            ],
            [],
            "stopped forwarding must not retain a reader or delivery task",
        )

    async def test_delayed_pump_backpressures_and_preserves_all_original_bytes(self) -> None:
        await self.exercise_stalled_pump("deliver")

    async def test_stopping_full_delayed_queue_reaps_reader_and_delivery(self) -> None:
        await self.exercise_stalled_pump("stop")

    async def test_delivery_failure_reaps_reader_and_preserves_original_error(self) -> None:
        await self.exercise_stalled_pump("failure")


if __name__ == "__main__":
    unittest.main()
