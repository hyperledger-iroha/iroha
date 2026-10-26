"""Test-only Python adapter for the existing iroha_measurement JSON view.

Requires Python 3.11+, macOS/Linux, and the repository's original scripts and
generated measurement descriptor. No SDK/native module is imported here.
The original harness supplies IROHA_MEASUREMENT_OUTPUT_DIR/context.json.
Only closed public labels and integer sizes/counts enter a record. Single-thread
serial phases are supported; CPU windows include all process work, and active
threads count only this adapter's calling thread. No allocation source is claimed.
Observation faults never replace the operation's exact return or exception.
The diagnostic must check delivery separately; missing/invalid records reject
measurement qualification, not SDK operations. Files are created exclusively.
"""
from __future__ import annotations

import os
from pathlib import Path
import resource
import stat
import sys
import threading
import time
from contextlib import contextmanager

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / "scripts"))
import zk_resource_harness as harness

DESCRIPTOR = harness.load_descriptor()
WORKLOADS = frozenset({"python.sdk.transaction.128", "python.sdk.signing_failure",
                      "python.sdk.exception", "python.sdk.retry"})
PHASES = frozenset({"sdk_flow", "sdk_operation", "sdk_calls", "prepare", "sign",
                   "canonical_frame", "decode_verify", "json_roundtrip", "exception_boundary"})
BYTE_LABELS = frozenset({"payload_json_utf8", "signed_transaction_versioned",
                        "envelope_json_utf8"})
WORK_LABELS = frozenset({"prepared_transactions", "signed_transactions", "canonical_frames",
                        "decoded_transactions", "json_roundtrips"})
STEMS = frozenset({"sdk-python-success", "sdk-python-signing-failure",
                   "sdk-python-exception", "sdk-python-retry"})


def _directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    info = os.fstat(fd)
    if info.st_uid != os.getuid() or info.st_mode & 0o077:
        os.close(fd)
        raise ValueError("diagnostic directory must be owner-only")
    return fd


def read_context(output_dir):
    """Read only the original harness's bounded, regular, owner-only handoff."""
    directory = _directory(output_dir)
    try:
        fd = os.open(DESCRIPTOR["context_file"], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                     dir_fd=directory)
        with os.fdopen(fd, "rb") as stream:
            info = os.fstat(stream.fileno())
            if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
                    or info.st_uid != os.getuid() or info.st_mode & 0o022
                    or info.st_size > 16384):
                raise ValueError("invalid diagnostic context custody")
            raw = stream.read(16385)
        if len(raw) > 16384:
            raise ValueError("diagnostic context too large")
        context = harness.strict_json(raw)
        harness.decode_view(DESCRIPTOR, "context", context, "context")
        if context["schema"] != DESCRIPTOR["context_schema"]:
            raise ValueError("diagnostic context schema mismatch")
        return context
    finally:
        os.close(directory)


class DirectorySink:
    """Exclusive JSON-view delivery; errors stay separate from the SDK result."""

    def __init__(self, output_dir, stem):
        if stem not in STEMS:
            raise ValueError("unknown public diagnostic stem")
        self.output_dir, self.stem = Path(output_dir), stem
        self.written = 0
        self.errors = 0

    def __call__(self, record):
        try:
            # Use the original harness codec/descriptor, not a schema twin.
            harness.decode_record(DESCRIPTOR, record)
            raw = harness.control.canonical(record)
            directory = _directory(self.output_dir)
            try:
                fd = os.open(self.stem + ".json", os.O_WRONLY | os.O_CREAT | os.O_EXCL
                             | os.O_NOFOLLOW, 0o600, dir_fd=directory)
                with os.fdopen(fd, "wb") as stream:
                    stream.write(raw)
                    stream.flush()
                    os.fsync(stream.fileno())
            finally:
                os.close(directory)
            self.written += 1
        except BaseException:
            # Never record exception text, paths, SDK payloads or key material.
            self.errors += 1

    def check(self):
        """Check diagnostic delivery only after the operation returned/raised."""
        if self.written != 1 or self.errors:
            raise RuntimeError("diagnostic record delivery failed")


def directory_sink(output_dir, stem):
    """Construct an exclusive sink; no output is created before delivery."""
    return DirectorySink(output_dir, stem)


def _load():
    return int(os.getloadavg()[0] * 1000)


def _clocks():
    return (time.perf_counter_ns(), time.thread_time_ns(), time.process_time_ns())


def _usage():
    usage = resource.getrusage(resource.RUSAGE_SELF)
    scale = 1 if sys.platform == "darwin" else 1024
    return (int(usage.ru_utime * 1_000_000_000), int(usage.ru_stime * 1_000_000_000),
            int(usage.ru_maxrss) * scale)


class _UnavailableSession:
    """No observation evidence after setup fails; the SDK still runs once."""

    @contextmanager
    def phase(self, label):
        yield

    def bytes(self, label, length):
        pass

    def work(self, label, units):
        pass

    def _finish(self, error):
        raise RuntimeError("diagnostic observation unavailable")


class Session:
    """One diagnostic operation; recording methods never return decisions."""

    def __init__(self, context, workload, expected_error):
        harness.decode_view(DESCRIPTOR, "context", context, "context")
        if context["schema"] != DESCRIPTOR["context_schema"]:
            raise ValueError("diagnostic context schema mismatch")
        if workload not in WORKLOADS:
            raise ValueError("unknown public diagnostic workload")
        if sys.platform != "darwin" and not sys.platform.startswith("linux"):
            raise ValueError("unsupported diagnostic host")
        self.identity = {k: v for k, v in context.items() if k != "schema"}
        self.identity.update(workload=workload, emitter="python.iroha.sdk_transaction", flow="sdk")
        self.expected_error = expected_error
        self.thread = threading.get_ident()
        self.nodes, self.stack, self.byte_entries, self.work_entries = [], [], [], []
        self.faults = 0
        self.clock_anomalies = 0
        self.begin_usage = _usage()
        self.begin_load = _load()
        self._open("sdk_flow")
        self._open("sdk_operation")

    def _open(self, label):
        if label not in PHASES or threading.get_ident() != self.thread:
            raise ValueError("unsupported diagnostic phase")
        parent = self.stack[-1][0] if self.stack else None
        index = next((i for i, node in enumerate(self.nodes)
                      if node["label"] == label and node["parent"] == parent), None)
        load = _load()
        if index is None:
            node = {key: 0 for key, kind in DESCRIPTOR["fields"]["phase_node"].items()
                    if kind in ("u64", "u32")}
            node.update(label=label, parent=parent, active_threads_max=1,
                        load_milli_first_enter=load)
            self.nodes.append(node)
            index = len(self.nodes) - 1
        node = self.nodes[index]
        node["calls"] += 1
        node["load_milli_max"] = max(node["load_milli_max"], load)
        if parent is not None:
            self.nodes[parent]["peak_concurrent_children"] = 1
        self.stack.append([index, _clocks(), [0, 0, 0]])
        return index

    def _close(self, error=None):
        end = _clocks()
        index, start, children = self.stack.pop()
        elapsed = [b - a for a, b in zip(start, end)]
        if any(v < 0 or v < child for v, child in zip(elapsed, children)):
            self.clock_anomalies += 1
        elapsed = [max(v, child, 0) for v, child in zip(elapsed, children)]
        node = self.nodes[index]
        node["completed" if error is None else "interrupted"] += 1
        if error is not None and not isinstance(error, self.expected_error):
            node["unwound"] += 1
        for position, name in enumerate(("wall", "thread_cpu", "process_cpu_window")):
            node[name + "_inclusive_ns"] += elapsed[position]
            node[name + "_exclusive_ns"] += elapsed[position] - children[position]
        load = _load()
        node["load_milli_last_exit"] = load
        node["load_milli_max"] = max(node["load_milli_max"], load)
        if self.stack:
            for position, value in enumerate(elapsed):
                self.stack[-1][2][position] += value

    @contextmanager
    def phase(self, label):
        """Measure a serial public API boundary without replacing its exception."""
        opened = False
        try:
            self._open(label)
            opened = True
        except BaseException:
            self.faults += 1
        error = None
        try:
            yield
        except BaseException as caught:
            error = caught
            raise
        finally:
            if opened:
                try:
                    self._close(error)
                except BaseException:
                    self.faults += 1

    def _counter(self, entries, label, units, allowed, suffix):
        try:
            if (label not in allowed or type(units) is not int or not 0 <= units <= harness.MAX_U64
                    or threading.get_ident() != self.thread):
                raise ValueError("invalid public diagnostic counter")
            phase = self.stack[-1][0]
            entry = next((e for e in entries if e["label"] == label and e["phase"] == phase), None)
            if entry is None:
                entry = {"label": label, "phase": phase, "count": 0,
                         "min_" + suffix: units, "max_" + suffix: units, "total_" + suffix: 0}
                if suffix == "bytes":
                    entry["kind"] = "transaction"
                entries.append(entry)
            if entry["total_" + suffix] + units > harness.MAX_U64:
                raise ValueError("diagnostic counter overflow")
            entry["count"] += 1
            entry["min_" + suffix] = min(entry["min_" + suffix], units)
            entry["max_" + suffix] = max(entry["max_" + suffix], units)
            entry["total_" + suffix] += units
        except BaseException:
            self.faults += 1

    def bytes(self, label, length):
        """Count actual public byte lengths, never inferred allocation sizes."""
        self._counter(self.byte_entries, label, length, BYTE_LABELS, "bytes")

    def work(self, label, units):
        """Count completed public operations in the current phase."""
        self._counter(self.work_entries, label, units, WORK_LABELS, "units")

    def _finish(self, error):
        expected = error is not None and isinstance(error, self.expected_error)
        self._close(error)
        # A handled Result-style SDK error completes the root but interrupts its
        # failed operation. A diagnostic exception unwinds both, as in Rust.
        self._close(None if expected else error)
        end_usage, end_load = _usage(), _load()
        soft, hard = resource.getrlimit(resource.RLIMIT_AS)
        soft, hard = (None if n == resource.RLIM_INFINITY else n for n in (soft, hard))
        failures = []
        if error is not None:
            failures.append({"phase": 1, "stage": "sdk_operation",
                             "code": "sdk_error" if expected else "diagnostic_exception"})
        if self.faults:
            failures.append({"phase": 1, "stage": "sdk_operation", "code": "observation_error"})
        outcome = ("failed" if expected or (self.faults and error is None) else
                   "unwound" if error is not None else "succeeded")
        health = {key: 0 for key in DESCRIPTOR["fields"]["recorder"] if key != "classification"}
        health.update(classification="measured", clock_anomaly_events=self.clock_anomalies)
        record = {
            "schema": DESCRIPTOR["record_schema"], "identity": self.identity, "outcome": outcome,
            "phase_tree": {"classification": "measured", "nodes": self.nodes},
            "byte_counters": {"classification": "measured", "entries": self.byte_entries},
            "work_counters": {"classification": "measured", "entries": self.work_entries},
            "allocations": {"classification": "measured", "sources": []},
            "declared": [], "failures": {"classification": "measured", "dropped": 0, "entries": failures},
            "process": {"classification": "measured", "pid": os.getpid(),
                        "cpu_user_ns": end_usage[0] - self.begin_usage[0],
                        "cpu_system_ns": end_usage[1] - self.begin_usage[1],
                        "peak_rss_bytes": end_usage[2], "peak_rss_bytes_at_begin": self.begin_usage[2],
                        "peak_rss_source": "kernel_lifetime_high_water", "logical_cpus": os.cpu_count() or 1,
                        "load_milli_begin": self.begin_load, "load_milli_finish": end_load,
                        "thermal_begin": "unavailable", "thermal_finish": "unavailable",
                        "thermal_source": "python.diagnostic.unavailable"},
            "address_space": {"classification": "local_scheduling", "soft_limit_bytes": soft,
                              "hard_limit_bytes": hard, "enforced": soft is not None,
                              "source": "getrlimit.RLIMIT_AS"},
            "scheduling": {"classification": "local_scheduling", "workers": 1,
                           "workers_provenance": "diagnostic.calling_thread", "phase_workers": []},
            "recorder": health,
        }
        return record


def observe(context, workload, operation, sink, *, expected_error=()):
    """Return the exact SDK object or re-raise its exact exception after recording.

The adapter does not inspect the operation's result. Sink faults never replace it;
the harness/diagnostic separately rejects missing or invalid observation evidence.
"""
    try:
        session = Session(context, workload, expected_error)
    except BaseException:
        session = _UnavailableSession()
    error = None
    try:
        return operation(session)
    except BaseException as caught:
        error = caught
        raise
    finally:
        try:
            sink(session._finish(error))
        except BaseException:
            # Missing evidence is a diagnostic failure, never an SDK result.
            pass
