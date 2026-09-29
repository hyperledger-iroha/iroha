#!/usr/bin/env python3
"""Fault injection of the Sumeragi release-gate soak (``scripts/sumeragi_soak.py``).

* Network loss and delay spikes on every P2P link:

  - Linux (``netem`` mode): ``tc netem`` on the loopback device of a network namespace that
    holds the whole soak network (``unshare --user --map-root-user --net --mount``). A ``prio``
    root qdisc sends only packets whose source or destination port is a peer's P2P port through
    a ``netem`` child, so Torii traffic of the load generator is untouched. Loss is packet loss:
    TCP retransmits, so the node sees the stream stall for a retransmission timeout (and a
    connection may time out and reconnect), never a corrupt or missing byte.
  - macOS (``proxy`` mode, also usable on Linux): a userspace TCP proxy listens in front of each
    peer's P2P port and every peer dials the proxies. A proxy cannot drop bytes of a TCP stream
    without corrupting the encrypted P2P framing, so it models what loss does to a TCP stream:
    each forwarded chunk is "lost" with the loss probability and then delivered only after a
    retransmission timeout (200 ms, doubling on repeated loss, at most five times), and with a
    small probability per chunk the connection is reset (RST both ways), which drops the
    in-flight messages and forces the peers to reconnect. Delay, jitter and spikes delay every
    chunk, preserving order. Differences to netem: the proxy never reorders or duplicates, its
    loss adds latency per chunk instead of per packet (a chunk is one ``recv`` of up to 64 KiB),
    and resets (which netem produces only through timeouts) are explicit.

* Disk full for one peer: its state directory (Kura store, Sumeragi records, installation log)
  lives on a size-limited volume — an HFS+ disk image attached with ``hdiutil`` on macOS, a
  ``tmpfs`` with a size limit inside the soak's mount namespace on Linux — and the fault fills
  the volume with a filler file until ``ENOSPC``; healing deletes it.

* ``kill -9`` and restart are done by the orchestrator (``os.kill(pid, SIGKILL)``).

The fault plan is a pure function of the seed (``plan_faults``).
"""

from __future__ import annotations

import asyncio
import errno
import os
import random
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Callable, Optional, Sequence

FAULT_KINDS = ("kill", "disk", "net")
FILLER_NAME = ".sumeragi-soak-disk-filler"
RTO_MS = 200.0
MAX_RETRANSMISSIONS = 5
CHUNK_BYTES = 64 * 1024


# ---------------------------------------------------------------------------------------------
# Plan
# ---------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class Fault:
    """One fault window of the plan, relative to the start of the load (seconds)."""

    kind: str
    start_s: float
    duration_s: float
    nodes: tuple[int, ...] = ()
    loss: float = 0.0

    @property
    def end_s(self) -> float:
        """End of the window (the heal time)."""
        return self.start_s + self.duration_s


@dataclass(frozen=True)
class PlanOptions:
    """Inputs of ``plan_faults``."""

    duration_s: float
    warmup_s: float
    final_quiet_s: float
    kinds: tuple[str, ...]
    validators: int
    disk_node: Optional[int]
    gap_s: tuple[float, float] = (60.0, 180.0)
    window_s: tuple[float, float] = (15.0, 60.0)
    loss: tuple[float, float] = (0.10, 0.30)
    max_kill: int = 1


def max_faulty(validators: int) -> int:
    """``f`` of an exact ``3f + 1`` committee."""
    return (validators - 1) // 3


def plan_faults(options: PlanOptions, rng: random.Random) -> list[Fault]:
    """A deterministic sequence of non-overlapping fault windows.

    Windows are separated by fault-free gaps; the first starts after ``warmup_s`` and the last
    ends ``final_quiet_s`` before the end, so every heal is followed by a fault-free interval
    in which O-LIVE and O-PERF are observed. A ``kill`` window kills between one and
    ``min(max_kill, f)`` validators, so at least ``q = n - f`` stay up.
    """
    for kind in options.kinds:
        if kind not in FAULT_KINDS:
            raise ValueError(f"unknown fault kind {kind!r}")
    if "disk" in options.kinds and options.disk_node is None:
        raise ValueError("a disk fault needs a disk node")
    faults: list[Fault] = []
    if not options.kinds:
        return faults
    f = max_faulty(options.validators)
    if f < 1:
        raise ValueError("fault injection needs at least four validators")
    max_kill = max(1, min(options.max_kill, f))
    cursor = options.warmup_s
    order: list[str] = []
    while True:
        duration = rng.uniform(*options.window_s)
        if cursor + duration + options.final_quiet_s > options.duration_s:
            break
        if not order:
            order = list(options.kinds)
            rng.shuffle(order)
        kind = order.pop()
        nodes: tuple[int, ...] = ()
        loss = 0.0
        if kind == "kill":
            count = rng.randint(1, max_kill)
            nodes = tuple(sorted(rng.sample(range(options.validators), count)))
        elif kind == "disk":
            nodes = (int(options.disk_node),)
        else:
            loss = round(rng.uniform(*options.loss), 4)
        faults.append(Fault(kind=kind, start_s=round(cursor, 3), duration_s=round(duration, 3), nodes=nodes, loss=loss))
        cursor += duration + rng.uniform(*options.gap_s)
    return faults


# ---------------------------------------------------------------------------------------------
# Link conditions
# ---------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class LinkCondition:
    """What every P2P link suffers: loss, delay, jitter, a delay spike and resets."""

    loss: float = 0.0
    delay_ms: float = 0.0
    jitter_ms: float = 0.0
    spike_ms: float = 0.0
    reset: float = 0.0

    def with_spike(self, spike_ms: float) -> "LinkCondition":
        """The same condition with a delay spike of ``spike_ms``."""
        return replace(self, spike_ms=spike_ms)


CLEAR = LinkCondition()


def net_condition(loss: float, rng: random.Random, reset_ratio: float) -> LinkCondition:
    """The condition of a ``net`` window: ``loss`` plus a small random base delay and jitter."""
    return LinkCondition(
        loss=loss,
        delay_ms=round(rng.uniform(5.0, 25.0), 1),
        jitter_ms=round(rng.uniform(2.0, 10.0), 1),
        reset=loss * reset_ratio,
    )


def spike_plan(rng: random.Random, duration_s: float) -> list[tuple[float, float, float]]:
    """Delay spikes inside a ``net`` window: ``(offset s, length s, extra delay ms)``."""
    spikes = []
    cursor = rng.uniform(2.0, 8.0)
    while cursor < duration_s:
        length = rng.uniform(1.0, 3.0)
        spikes.append((round(cursor, 3), round(min(length, duration_s - cursor), 3), round(rng.uniform(300.0, 1500.0), 1)))
        cursor += length + rng.uniform(5.0, 15.0)
    return spikes


class ConditionBox:
    """The current link condition, shared between the orchestrator and the proxies."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._condition = CLEAR

    def get(self) -> LinkCondition:
        """The current condition."""
        with self._lock:
            return self._condition

    def set(self, condition: LinkCondition) -> None:
        """Replace the condition."""
        with self._lock:
            self._condition = condition


def chunk_delay_ms(condition: LinkCondition, rng: random.Random) -> tuple[float, int]:
    """Delay of one forwarded chunk and how many times it was "lost" (each costs an RTO)."""
    delay = condition.delay_ms + condition.spike_ms
    if condition.jitter_ms > 0:
        delay += rng.uniform(0.0, condition.jitter_ms)
    losses = 0
    rto = RTO_MS
    while losses < MAX_RETRANSMISSIONS and condition.loss > 0 and rng.random() < condition.loss:
        delay += rto
        rto *= 2
        losses += 1
    return delay, losses


@dataclass
class ProxyStats:
    """Counters of all proxies."""

    connections: int = 0
    chunks: int = 0
    bytes: int = 0
    lost_chunks: int = 0
    resets: int = 0
    lock: threading.Lock = field(default_factory=threading.Lock, repr=False)

    def to_json(self) -> dict[str, int]:
        """Counters for the run record."""
        with self.lock:
            return {
                "connections": self.connections,
                "chunks": self.chunks,
                "bytes": self.bytes,
                "lost_chunks": self.lost_chunks,
                "resets": self.resets,
            }


class ProxyNetwork:
    """Userspace TCP proxies in front of the peers' P2P ports (one asyncio loop in a thread)."""

    def __init__(
        self,
        routes: Sequence[tuple[str, int, str, int]],
        conditions: ConditionBox,
        seed: int,
    ) -> None:
        """``routes``: ``(listen host, listen port, target host, target port)`` per peer."""
        self.routes = list(routes)
        self.conditions = conditions
        self.rng = random.Random(seed)
        self.stats = ProxyStats()
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._thread: Optional[threading.Thread] = None
        self._servers: list[asyncio.base_events.Server] = []
        self._writers: set[asyncio.StreamWriter] = set()
        self._ready = threading.Event()
        self._error: Optional[BaseException] = None

    def start(self) -> None:
        """Bind every proxy and start forwarding."""
        self._thread = threading.Thread(target=self._run, name="sumeragi-soak-proxy", daemon=True)
        self._thread.start()
        self._ready.wait(timeout=30)
        if self._error is not None:
            raise RuntimeError(f"proxy start failed: {self._error}") from self._error
        if not self._ready.is_set():
            raise RuntimeError("proxy start timed out")

    def stop(self) -> None:
        """Close every proxy and connection."""
        loop = self._loop
        if loop is None:
            return
        future = asyncio.run_coroutine_threadsafe(self._close(), loop)
        try:
            future.result(timeout=10)
        finally:
            loop.call_soon_threadsafe(loop.stop)
            if self._thread is not None:
                self._thread.join(timeout=10)
            self._loop = None

    def reset_all(self) -> int:
        """Reset every open connection now (a network partition flap); returns how many."""
        loop = self._loop
        if loop is None:
            return 0
        future = asyncio.run_coroutine_threadsafe(self._reset_all(), loop)
        return future.result(timeout=10)

    def _run(self) -> None:
        loop = asyncio.new_event_loop()
        self._loop = loop
        asyncio.set_event_loop(loop)
        try:
            loop.run_until_complete(self._bind())
        except BaseException as error:  # reported to start()
            self._error = error
            self._ready.set()
            loop.close()
            return
        self._ready.set()
        try:
            loop.run_forever()
        finally:
            loop.close()

    async def _bind(self) -> None:
        for listen_host, listen_port, target_host, target_port in self.routes:
            server = await asyncio.start_server(
                lambda reader, writer, host=target_host, port=target_port: self._handle(
                    reader, writer, host, port
                ),
                host=listen_host,
                port=listen_port,
                reuse_address=True,
            )
            self._servers.append(server)

    async def _close(self) -> None:
        for server in self._servers:
            server.close()
        for writer in list(self._writers):
            _abort(writer)
        for server in self._servers:
            await server.wait_closed()

    async def _reset_all(self) -> int:
        writers = list(self._writers)
        for writer in writers:
            _abort(writer)
        with self.stats.lock:
            self.stats.resets += len(writers) // 2
        return len(writers) // 2

    async def _handle(
        self,
        client_reader: asyncio.StreamReader,
        client_writer: asyncio.StreamWriter,
        target_host: str,
        target_port: int,
    ) -> None:
        try:
            server_reader, server_writer = await asyncio.open_connection(target_host, target_port)
        except OSError:
            _abort(client_writer)
            return
        with self.stats.lock:
            self.stats.connections += 1
        self._writers.update((client_writer, server_writer))
        pair = (client_writer, server_writer)
        try:
            await asyncio.gather(
                self._pump(client_reader, server_writer, pair),
                self._pump(server_reader, client_writer, pair),
                return_exceptions=True,
            )
        finally:
            for writer in pair:
                self._writers.discard(writer)
                if not writer.is_closing():
                    writer.close()

    async def _pump(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
        pair: tuple[asyncio.StreamWriter, asyncio.StreamWriter],
    ) -> None:
        loop = asyncio.get_running_loop()
        queue: asyncio.Queue[Optional[tuple[float, bytes]]] = asyncio.Queue()
        deliver = asyncio.ensure_future(self._deliver(queue, writer, pair))
        last_due = 0.0
        try:
            while True:
                data = await reader.read(CHUNK_BYTES)
                if not data:
                    break
                condition = self.conditions.get()
                if condition.reset > 0 and self.rng.random() < condition.reset:
                    with self.stats.lock:
                        self.stats.resets += 1
                    for side in pair:
                        _abort(side)
                    break
                delay_ms, losses = chunk_delay_ms(condition, self.rng)
                with self.stats.lock:
                    self.stats.chunks += 1
                    self.stats.bytes += len(data)
                    self.stats.lost_chunks += 1 if losses else 0
                # In order, like TCP: a chunk never overtakes an earlier one.
                last_due = max(last_due, loop.time() + delay_ms / 1000.0)
                await queue.put((last_due, data))
        except (ConnectionError, OSError):
            for side in pair:
                _abort(side)
        finally:
            await queue.put(None)
            await deliver

    @staticmethod
    async def _deliver(
        queue: "asyncio.Queue[Optional[tuple[float, bytes]]]",
        writer: asyncio.StreamWriter,
        pair: tuple[asyncio.StreamWriter, asyncio.StreamWriter],
    ) -> None:
        loop = asyncio.get_running_loop()
        while True:
            item = await queue.get()
            if item is None:
                if not writer.is_closing():
                    try:
                        writer.write_eof()
                    except (OSError, RuntimeError):
                        pass
                return
            due, data = item
            wait = due - loop.time()
            if wait > 0:
                await asyncio.sleep(wait)
            if writer.is_closing():
                continue
            try:
                writer.write(data)
                await writer.drain()
            except (ConnectionError, OSError):
                for side in pair:
                    _abort(side)


def _abort(writer: asyncio.StreamWriter) -> None:
    """Close with RST (``SO_LINGER`` 0), dropping whatever is in flight."""
    sock = writer.get_extra_info("socket")
    if sock is not None:
        try:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        except OSError:
            pass
    writer.transport.abort()


# ---------------------------------------------------------------------------------------------
# netem (Linux)
# ---------------------------------------------------------------------------------------------

NETEM_PARENT = "1:3"
NETEM_HANDLE = "30:"


def netem_setup_commands(ports: Sequence[int], device: str = "lo") -> list[list[str]]:
    """``tc`` commands that route the P2P ports' packets through a ``netem`` child qdisc.

    The ``prio`` root's default priomap sends ordinary traffic to bands 1 and 2; only the u32
    filters (source or destination port equal to a P2P port) select band 3, the ``netem`` child.
    """
    commands = [
        ["tc", "qdisc", "replace", "dev", device, "root", "handle", "1:", "prio"],
        [
            "tc", "qdisc", "replace", "dev", device, "parent", NETEM_PARENT, "handle", NETEM_HANDLE,
            "netem", "delay", "0ms", "loss", "0%",
        ],
    ]
    for port in ports:
        for direction in ("sport", "dport"):
            commands.append(
                [
                    "tc", "filter", "add", "dev", device, "parent", "1:0", "protocol", "ip",
                    "prio", "3", "u32", "match", "ip", direction, str(port), "0xffff",
                    "flowid", NETEM_PARENT,
                ]
            )
    return commands


def netem_change_command(condition: LinkCondition, device: str = "lo") -> list[str]:
    """The ``tc qdisc change`` that applies ``condition`` to the P2P band."""
    delay = condition.delay_ms + condition.spike_ms
    command = [
        "tc", "qdisc", "change", "dev", device, "parent", NETEM_PARENT, "handle", NETEM_HANDLE,
        "netem", "delay", f"{delay:g}ms",
    ]
    if condition.jitter_ms > 0:
        command.append(f"{condition.jitter_ms:g}ms")
    command.extend(["loss", f"{condition.loss * 100:g}%"])
    return command


def netem_teardown_command(device: str = "lo") -> list[str]:
    """Remove the soak's qdiscs."""
    return ["tc", "qdisc", "del", "dev", device, "root"]


class Netem:
    """Applies link conditions with ``tc netem`` inside the soak's network namespace."""

    def __init__(self, ports: Sequence[int], device: str = "lo", run: Callable[..., object] = subprocess.run) -> None:
        self.ports = list(ports)
        self.device = device
        self._run = run

    def setup(self) -> None:
        """Bring the device up and install the qdiscs."""
        self._run(["ip", "link", "set", self.device, "up"], check=True)
        for command in netem_setup_commands(self.ports, self.device):
            self._run(command, check=True)

    def apply(self, condition: LinkCondition) -> None:
        """Apply ``condition``."""
        self._run(netem_change_command(condition, self.device), check=True)

    def teardown(self) -> None:
        """Remove the qdiscs (the namespace disappears with the soak anyway)."""
        self._run(netem_teardown_command(self.device), check=False)


# ---------------------------------------------------------------------------------------------
# Disk full
# ---------------------------------------------------------------------------------------------


def disk_mount_commands(platform: str, image: Path, mountpoint: Path, size_mb: int) -> tuple[list[list[str]], list[str]]:
    """``(commands that create and mount the volume, the unmount command)`` for ``platform``."""
    if platform == "darwin":
        return (
            [
                [
                    "hdiutil", "create", "-quiet", "-size", f"{size_mb}m", "-fs", "HFS+",
                    "-volname", "sumeragi-soak", "-layout", "NONE", "-ov", str(image),
                ],
                [
                    "hdiutil", "attach", "-quiet", "-nobrowse", "-noverify", "-noautoopen",
                    "-owners", "on", "-mountpoint", str(mountpoint), str(image),
                ],
            ],
            ["hdiutil", "detach", "-quiet", "-force", str(mountpoint)],
        )
    if platform.startswith("linux"):
        return (
            [["mount", "-t", "tmpfs", "-o", f"size={size_mb}m,mode=0700", "tmpfs", str(mountpoint)]],
            ["umount", "-l", str(mountpoint)],
        )
    raise ValueError(f"no disk-full volume for platform {platform!r}")


class DiskVolume:
    """A size-limited volume holding one peer's state directory."""

    def __init__(
        self,
        state_dir: Path,
        work_dir: Path,
        size_mb: int,
        platform: str = sys.platform,
        run: Callable[..., object] = subprocess.run,
    ) -> None:
        self.state_dir = state_dir
        self.image = work_dir / "sumeragi-soak-disk.dmg"
        self.size_mb = size_mb
        self.platform = platform
        self._run = run
        self._mounted = False

    @property
    def filler(self) -> Path:
        """The filler file."""
        return self.state_dir / FILLER_NAME

    def mount(self) -> None:
        """Move the state directory onto a fresh volume mounted in its place."""
        staging = self.state_dir.with_name(self.state_dir.name + ".soak-staging")
        if self.state_dir.exists():
            self.state_dir.rename(staging)
        self.state_dir.mkdir(mode=0o700, parents=True)
        commands, _ = disk_mount_commands(self.platform, self.image, self.state_dir, self.size_mb)
        for command in commands:
            self._run(command, check=True)
        self._mounted = True
        os.chmod(self.state_dir, 0o700)
        if staging.exists():
            for entry in staging.iterdir():
                target = self.state_dir / entry.name
                if entry.is_dir() and not entry.is_symlink():
                    shutil.copytree(entry, target, symlinks=True)
                else:
                    shutil.copy2(entry, target, follow_symlinks=False)
            shutil.rmtree(staging)

    def fill(self) -> int:
        """Fill the volume until ``ENOSPC``; returns the filler size."""
        written = 0
        with open(self.filler, "wb", buffering=0) as handle:
            for size in (4 * 1024 * 1024, 64 * 1024, 4 * 1024, 512):
                block = bytes(size)
                while True:
                    try:
                        count = handle.write(block)
                    except OSError as error:
                        if error.errno in (errno.ENOSPC, errno.EDQUOT):
                            break
                        raise
                    if not count:
                        break
                    written += count
            try:
                os.fsync(handle.fileno())
            except OSError as error:
                if error.errno not in (errno.ENOSPC, errno.EDQUOT):
                    raise
        return written

    def free(self) -> None:
        """Delete the filler."""
        try:
            self.filler.unlink()
        except FileNotFoundError:
            pass

    def unmount(self) -> None:
        """Detach the volume (its contents are lost with it on Linux)."""
        if not self._mounted:
            return
        _, command = disk_mount_commands(self.platform, self.image, self.state_dir, self.size_mb)
        for attempt in range(5):
            result = self._run(command, check=False)
            if getattr(result, "returncode", 0) == 0:
                break
            time.sleep(1.0 + attempt)
        self._mounted = False
        if self.platform == "darwin":
            try:
                self.image.unlink()
            except FileNotFoundError:
                pass
