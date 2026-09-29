#!/usr/bin/env python3
"""Node-log parsing and oracles of the Sumeragi release-gate soak.

The soak (``scripts/sumeragi_soak.py``, ``specs/sumeragi.md`` §13.5 and §14 item 5) runs a real
multi-process network under faults and judges it only from what the nodes logged. The driver
emits two structured audit events (``crates/iroha_core/src/sumeragi/driver/audit.rs``, target
``iroha_core::sumeragi::driver::audit``):

* ``sumeragi block applied`` (INFO): the executor worker made a block the applied state, for every
  instance (the global chain and every lane). Fields: ``instance``, ``height``, ``view``,
  ``origin_view``, ``block``, ``result``, ``proposer``, ``payload_bytes``, ``attest``.
* ``sumeragi record durable`` (DEBUG): the persistence worker made a safety record durable
  (§7.4). Fields: ``instance``, ``key``, ``height``, ``epoch`` and ``signed``, the record's
  latest signatures at ``height``: ``proposal:<view>:<block>``,
  ``prepare:<view>:<block>:<result>:<attest 0|1>``, ``lock:<view>:<block>:<result>:<attest>``
  and ``timeout:<view>:<view of the carried PrepareQC or ->``, comma-separated, or ``-``.

Both the JSON and the text (full/compact) formats of the node logger are understood. From these
lines this module computes the §13.2 oracles on real nodes:

* O-AGR: every committed ``(instance, height)`` has one ``(block, result)`` on every node and
  across restarts, and each node's chain is contiguous (within a process lifetime exactly; across
  a restart at most the one height applied but not yet logged when the process was killed).
* O-SIGN: per consensus key, across restarts, no two different preimages of one kind at
  ``(instance, height, view)``; no new Prepare at a view at or below a timeout view the key had
  already made durable at that height; no new timeout carrying a PrepareQC below a lock the key
  had already made durable at that height (the timeout carries ``high_pqc``, which never falls
  below the lock, Lemma 2); and no durable record that goes backwards (a rolled-back record,
  §7.4 record provenance).
* O-LIVE: after every heal (end of a fault window) every node commits a new height of the
  observed instance within the bound ``B_live`` (§8.2) and again within every later window of
  ``B_live`` until the next fault; no node exits unless the soak killed it or a tolerated fault
  (disk full) stopped it.
* O-PERF: commit-gap percentiles in fault-free intervals (after a warm-up of heights, Appendix
  E8), committed throughput and client-observed commit latency against thresholds.

Everything here is pure: the orchestration lives in ``scripts/sumeragi_soak.py`` and writes the
run's timeline (``timeline.json``) and node logs, from which ``build_verdict`` recomputes the
verdict offline (``sumeragi_soak.py --analyze <run dir>``).
"""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterable, Mapping, Optional, Sequence

APPLIED_MESSAGE = "sumeragi block applied"
DURABLE_MESSAGE = "sumeragi record durable"
AUDIT_TARGET = "iroha_core::sumeragi::driver::audit"
# Log filter that enables both audit events on a node (the durable-record line is DEBUG).
AUDIT_LOG_FILTER = f"info,{AUDIT_TARGET}=debug"

# Other node lines the verdict reports as observations (not oracle inputs).
OBSERVED_MESSAGES = {
    "local_fault": "sumeragi: local fault",
    "evidence": "sumeragi: evidence of misbehaviour",
    "persistence_retry": "sumeragi persistence failed; retrying",
    "panic": "panicked at",
}

VERDICT_SCHEMA = "iroha.sumeragi.soak.verdict"
VERDICT_VERSION = 1

_ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
_TIMESTAMP_RE = re.compile(
    r"(?P<date>\d{4}-\d{2}-\d{2})T(?P<time>\d{2}:\d{2}:\d{2})(?:\.(?P<frac>\d+))?"
    r"(?P<tz>Z|[+-]\d{2}:?\d{2})"
)
_FIELD_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_.]*)=(\"(?:[^\"\\]|\\.)*\"|\S+)")
_HEX_RE = re.compile(r"^[0-9a-f]+$")


class AuditParseError(ValueError):
    """An audit line that names an audit event but cannot be decoded."""


@dataclass(frozen=True)
class Applied:
    """One ``sumeragi block applied`` line."""

    node: str
    boot: int
    line: int
    ts_ms: float
    instance: str
    height: int
    view: int
    origin_view: int
    block: str
    result: str
    proposer: int
    payload_bytes: int
    attest: bool


@dataclass(frozen=True)
class Signed:
    """The ``signed`` field of a durable record: the key's latest signatures at the height."""

    proposal: Optional[tuple[int, str]] = None
    prepare: Optional[tuple[int, str, str, bool]] = None
    lock: Optional[tuple[int, str, str, bool]] = None
    timeout: Optional[tuple[int, Optional[int]]] = None


@dataclass(frozen=True)
class Record:
    """One ``sumeragi record durable`` line."""

    node: str
    boot: int
    line: int
    ts_ms: float
    instance: str
    key: str
    height: int
    epoch: int
    signed: Signed


@dataclass
class NodeLog:
    """Everything the soak read from one node's logs (all boots, in order)."""

    node: str
    applied: list[Applied] = field(default_factory=list)
    records: list[Record] = field(default_factory=list)
    observations: dict[str, int] = field(default_factory=dict)
    examples: dict[str, str] = field(default_factory=dict)
    parse_errors: list[str] = field(default_factory=list)
    boots: int = 0
    lines: int = 0


@dataclass(frozen=True)
class Violation:
    """One oracle violation."""

    oracle: str
    kind: str
    detail: Mapping[str, Any]

    def to_json(self) -> dict[str, Any]:
        """Render the violation for the verdict."""
        return {"oracle": self.oracle, "kind": self.kind, **dict(self.detail)}


@dataclass(frozen=True)
class Window:
    """A fault window ``[start_ms, end_ms)`` of the timeline."""

    start_ms: float
    end_ms: float
    kind: str
    nodes: tuple[str, ...] = ()


@dataclass(frozen=True)
class Boot:
    """One process lifetime of a node: ``[start_ms, end_ms)`` and how it ended."""

    node: str
    index: int
    start_ms: float
    end_ms: Optional[float]
    # "killed" (kill -9 by the soak), "stopped" (orderly stop at the end), "exited:<code>" (the
    # process ended by itself) or "running" (the timeline ended first).
    ended: str


@dataclass(frozen=True)
class Timeline:
    """The run as the orchestrator saw it (``timeline.json``)."""

    start_ms: float
    end_ms: float
    nodes: tuple[str, ...]
    windows: tuple[Window, ...]
    boots: tuple[Boot, ...]
    # Fault windows during which an exit of the listed nodes is tolerated (e.g. disk full).
    tolerated_exit_kinds: tuple[str, ...] = ("disk",)

    @staticmethod
    def from_json(value: Mapping[str, Any]) -> "Timeline":
        """Decode ``timeline.json``."""
        return Timeline(
            start_ms=float(value["start_ms"]),
            end_ms=float(value["end_ms"]),
            nodes=tuple(str(node) for node in value["nodes"]),
            windows=tuple(
                Window(
                    start_ms=float(window["start_ms"]),
                    end_ms=float(window["end_ms"]),
                    kind=str(window["kind"]),
                    nodes=tuple(str(node) for node in window.get("nodes", ())),
                )
                for window in value.get("windows", ())
            ),
            boots=tuple(
                Boot(
                    node=str(boot["node"]),
                    index=int(boot["index"]),
                    start_ms=float(boot["start_ms"]),
                    end_ms=None if boot.get("end_ms") is None else float(boot["end_ms"]),
                    ended=str(boot.get("ended", "running")),
                )
                for boot in value.get("boots", ())
            ),
            tolerated_exit_kinds=tuple(value.get("tolerated_exit_kinds", ("disk",))),
        )

    def to_json(self) -> dict[str, Any]:
        """Encode as ``timeline.json``."""
        return {
            "start_ms": self.start_ms,
            "end_ms": self.end_ms,
            "nodes": list(self.nodes),
            "windows": [
                {
                    "start_ms": window.start_ms,
                    "end_ms": window.end_ms,
                    "kind": window.kind,
                    "nodes": list(window.nodes),
                }
                for window in self.windows
            ],
            "boots": [
                {
                    "node": boot.node,
                    "index": boot.index,
                    "start_ms": boot.start_ms,
                    "end_ms": boot.end_ms,
                    "ended": boot.ended,
                }
                for boot in self.boots
            ],
            "tolerated_exit_kinds": list(self.tolerated_exit_kinds),
        }

    def steady_intervals(self) -> list[tuple[float, float]]:
        """The fault-free intervals ``[heal, next fault)`` inside the run, in order."""
        intervals: list[tuple[float, float]] = []
        cursor = self.start_ms
        for start, end in _merged_windows(self.windows):
            if start > cursor:
                intervals.append((cursor, min(start, self.end_ms)))
            cursor = max(cursor, end)
        if cursor < self.end_ms:
            intervals.append((cursor, self.end_ms))
        return [(start, end) for start, end in intervals if end > start]


@dataclass(frozen=True)
class Thresholds:
    """O-PERF thresholds and the O-LIVE bound (milliseconds unless named otherwise)."""

    live_bound_ms: float
    max_gap_p99_ms: float
    max_gap_ms: float
    max_latency_p99_ms: float
    min_tps: float
    warmup_heights: int = 20


@dataclass(frozen=True)
class LoadSample:
    """A committed-transaction counter sample of one node (Torii ``/status``)."""

    ts_ms: float
    node: str
    boot: int
    txs_approved: int


@dataclass(frozen=True)
class Probe:
    """A client-observed commit latency probe (submit and wait for the commit)."""

    start_ms: float
    end_ms: float
    ok: bool


@dataclass(frozen=True)
class LoadRecord:
    """What the load generator did (``load.json``)."""

    submitted: int
    attempted: int
    samples: tuple[LoadSample, ...]
    probes: tuple[Probe, ...]

    @staticmethod
    def from_json(value: Mapping[str, Any]) -> "LoadRecord":
        """Decode ``load.json``."""
        return LoadRecord(
            submitted=int(value.get("submitted", 0)),
            attempted=int(value.get("attempted", 0)),
            samples=tuple(
                LoadSample(
                    ts_ms=float(sample["ts_ms"]),
                    node=str(sample["node"]),
                    boot=int(sample["boot"]),
                    txs_approved=int(sample["txs_approved"]),
                )
                for sample in value.get("samples", ())
            ),
            probes=tuple(
                Probe(
                    start_ms=float(probe["start_ms"]),
                    end_ms=float(probe["end_ms"]),
                    ok=bool(probe["ok"]),
                )
                for probe in value.get("probes", ())
            ),
        )


# ---------------------------------------------------------------------------------------------
# Parsing
# ---------------------------------------------------------------------------------------------


def parse_timestamp_ms(text: str) -> float:
    """Milliseconds since the Unix epoch of an RFC 3339 timestamp (any fraction precision)."""
    match = _TIMESTAMP_RE.search(text)
    if match is None:
        raise AuditParseError(f"no RFC 3339 timestamp in {text!r}")
    base = datetime.strptime(f"{match['date']}T{match['time']}", "%Y-%m-%dT%H:%M:%S")
    tz = match["tz"]
    if tz == "Z":
        offset = timedelta(0)
    else:
        sign = 1 if tz[0] == "+" else -1
        digits = tz[1:].replace(":", "")
        offset = sign * timedelta(hours=int(digits[:2]), minutes=int(digits[2:]))
    moment = base.replace(tzinfo=timezone(offset))
    frac = match["frac"] or ""
    micros = int((frac + "000000")[:6]) if frac else 0
    return moment.timestamp() * 1000.0 + micros / 1000.0


def parse_signed(value: str) -> Signed:
    """Decode the ``signed`` field of a durable record (grammar in the module documentation)."""
    value = value.strip().strip('"')
    if value == "-":
        return Signed()
    proposal = prepare = lock = timeout = None
    for entry in value.split(","):
        parts = entry.split(":")
        kind = parts[0]
        try:
            if kind == "proposal" and len(parts) == 3:
                _require(proposal is None, entry)
                proposal = (int(parts[1]), _hex(parts[2]))
            elif kind in ("prepare", "lock") and len(parts) == 5:
                vote = (int(parts[1]), _hex(parts[2]), _hex(parts[3]), _flag(parts[4]))
                if kind == "prepare":
                    _require(prepare is None, entry)
                    prepare = vote
                else:
                    _require(lock is None, entry)
                    lock = vote
            elif kind == "timeout" and len(parts) == 3:
                _require(timeout is None, entry)
                timeout = (int(parts[1]), None if parts[2] == "-" else int(parts[2]))
            else:
                raise AuditParseError(f"unknown signed entry {entry!r}")
        except ValueError as error:
            raise AuditParseError(f"malformed signed entry {entry!r}: {error}") from error
    return Signed(proposal=proposal, prepare=prepare, lock=lock, timeout=timeout)


def _require(condition: bool, entry: str) -> None:
    if not condition:
        raise AuditParseError(f"repeated signed entry {entry!r}")


def _hex(value: str) -> str:
    value = value.strip().lower()
    if not _HEX_RE.match(value):
        raise AuditParseError(f"not lowercase hex: {value!r}")
    return value


def _flag(value: str) -> bool:
    if value in ("0", "false"):
        return False
    if value in ("1", "true"):
        return True
    raise AuditParseError(f"not a flag: {value!r}")


def _int(fields: Mapping[str, Any], name: str) -> int:
    if name not in fields:
        raise AuditParseError(f"missing field {name!r}")
    value = fields[name]
    if isinstance(value, bool):
        raise AuditParseError(f"field {name!r} is not an integer")
    try:
        return int(str(value).strip('"'))
    except ValueError as error:
        raise AuditParseError(f"field {name!r} is not an integer: {value!r}") from error


def _text(fields: Mapping[str, Any], name: str) -> str:
    if name not in fields:
        raise AuditParseError(f"missing field {name!r}")
    return str(fields[name]).strip('"')


def _bool(fields: Mapping[str, Any], name: str) -> bool:
    if name not in fields:
        raise AuditParseError(f"missing field {name!r}")
    value = fields[name]
    if isinstance(value, bool):
        return value
    return _flag(str(value).strip('"'))


def split_event(raw: str) -> Optional[tuple[str, Optional[str], Mapping[str, Any]]]:
    """``(message, timestamp text, fields)`` of a node log line, or ``None`` for other lines.

    JSON lines (the logger's ``json`` format) are decoded whole; a text line (``full`` or
    ``compact``) is decoded only when it carries one of the audit or observed messages, and its
    fields are the ``key=value`` pairs after the message (the event's own fields come first; a
    later repetition, e.g. a span field, is ignored). The timestamp is ``None`` when the line has
    none (e.g. a panic message written to stderr).
    """
    line = _ANSI_RE.sub("", raw).strip()
    if not line:
        return None
    if line.startswith("{"):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            value = None
        if isinstance(value, dict):
            fields = value.get("fields")
            if not isinstance(fields, dict):
                fields = value
            message = fields.get("message")
            if not isinstance(message, str):
                return None
            timestamp = value.get("timestamp")
            return message, timestamp if isinstance(timestamp, str) else None, fields
    for message in (APPLIED_MESSAGE, DURABLE_MESSAGE, *OBSERVED_MESSAGES.values()):
        index = line.find(message)
        if index < 0:
            continue
        timestamp_match = _TIMESTAMP_RE.match(line)
        fields: dict[str, Any] = {}
        for key, value in _FIELD_RE.findall(line[index + len(message) :]):
            fields.setdefault(key, value)
        return message, None if timestamp_match is None else timestamp_match.group(0), fields
    return None


def _audit_timestamp_ms(timestamp: Optional[str]) -> float:
    if timestamp is None:
        raise AuditParseError("audit line without a timestamp")
    return parse_timestamp_ms(timestamp)


def parse_log_lines(node: str, boot: int, lines: Iterable[str], log: NodeLog) -> None:
    """Append the audit events and observations of one boot's log lines to ``log``.

    An audit line that cannot be decoded is kept in ``log.parse_errors``: the verdict fails on
    it, since an oracle that cannot read its input would pass vacuously.
    """
    for number, raw in enumerate(lines, start=1):
        log.lines += 1
        try:
            event = split_event(raw)
            if event is None:
                continue
            message, timestamp, fields = event
            if message == APPLIED_MESSAGE:
                ts_ms = _audit_timestamp_ms(timestamp)
                log.applied.append(
                    Applied(
                        node=node,
                        boot=boot,
                        line=number,
                        ts_ms=ts_ms,
                        instance=_hex(_text(fields, "instance")),
                        height=_int(fields, "height"),
                        view=_int(fields, "view"),
                        origin_view=_int(fields, "origin_view"),
                        block=_hex(_text(fields, "block")),
                        result=_hex(_text(fields, "result")),
                        proposer=_int(fields, "proposer"),
                        payload_bytes=_int(fields, "payload_bytes"),
                        attest=_bool(fields, "attest"),
                    )
                )
            elif message == DURABLE_MESSAGE:
                ts_ms = _audit_timestamp_ms(timestamp)
                log.records.append(
                    Record(
                        node=node,
                        boot=boot,
                        line=number,
                        ts_ms=ts_ms,
                        instance=_hex(_text(fields, "instance")),
                        key=_hex(_text(fields, "key")),
                        height=_int(fields, "height"),
                        epoch=_int(fields, "epoch"),
                        signed=parse_signed(_text(fields, "signed")),
                    )
                )
            else:
                for name, observed in OBSERVED_MESSAGES.items():
                    if message.startswith(observed) or observed in message:
                        log.observations[name] = log.observations.get(name, 0) + 1
                        log.examples.setdefault(name, _ANSI_RE.sub("", raw).strip()[:400])
                        break
        except AuditParseError as error:
            log.parse_errors.append(f"{node} boot {boot} line {number}: {error}")


def boot_logs(node_dir: Path) -> list[tuple[int, Path]]:
    """The boot logs of one node directory (``boot<k>.log``), in boot order."""
    boots = []
    for path in node_dir.glob("boot*.log"):
        suffix = path.stem[len("boot") :]
        if suffix.isdigit():
            boots.append((int(suffix), path))
    return sorted(boots)


def load_node_logs(log_root: Path) -> dict[str, NodeLog]:
    """Parse every node's boot logs under ``log_root/<node>/boot<k>.log``."""
    logs: dict[str, NodeLog] = {}
    for node_dir in sorted(path for path in log_root.iterdir() if path.is_dir()):
        log = NodeLog(node=node_dir.name)
        for boot, path in boot_logs(node_dir):
            log.boots += 1
            with path.open("r", encoding="utf-8", errors="replace") as handle:
                parse_log_lines(node_dir.name, boot, handle, log)
        logs[node_dir.name] = log
    return logs


# ---------------------------------------------------------------------------------------------
# Oracles
# ---------------------------------------------------------------------------------------------


def check_agreement(applied: Sequence[Applied]) -> tuple[list[Violation], dict[str, Any]]:
    """O-AGR over every applied line of every node."""
    violations: list[Violation] = []
    commits: dict[tuple[str, int], dict[tuple[str, str], list[str]]] = {}
    for event in applied:
        seen = commits.setdefault((event.instance, event.height), {})
        seen.setdefault((event.block, event.result), [])
        where = f"{event.node}#{event.boot}"
        if where not in seen[(event.block, event.result)]:
            seen[(event.block, event.result)].append(where)
    for (instance, height), outcomes in sorted(commits.items()):
        if len(outcomes) > 1:
            violations.append(
                Violation(
                    "O-AGR",
                    "conflicting-commit",
                    {
                        "instance": instance,
                        "height": height,
                        "commits": [
                            {"block": block, "result": result, "nodes": nodes}
                            for (block, result), nodes in sorted(outcomes.items())
                        ],
                    },
                )
            )
    chains: dict[tuple[str, str], list[Applied]] = {}
    for event in applied:
        chains.setdefault((event.node, event.instance), []).append(event)
    for (node, instance), events in sorted(chains.items()):
        events.sort(key=lambda event: (event.boot, event.line))
        previous: Optional[Applied] = None
        for event in events:
            if previous is not None:
                if event.boot == previous.boot:
                    if event.height != previous.height + 1:
                        violations.append(
                            Violation(
                                "O-AGR",
                                "chain-gap" if event.height > previous.height else "height-regression",
                                {
                                    "node": node,
                                    "instance": instance,
                                    "boot": event.boot,
                                    "after": previous.height,
                                    "height": event.height,
                                },
                            )
                        )
                elif event.height > previous.height + 2:
                    # At most one height can be applied but not yet logged when the process died.
                    violations.append(
                        Violation(
                            "O-AGR",
                            "chain-gap-across-restart",
                            {
                                "node": node,
                                "instance": instance,
                                "boot": event.boot,
                                "after": previous.height,
                                "height": event.height,
                            },
                        )
                    )
            previous = event if previous is None or event.height >= previous.height else previous
    checked = {
        "applied_lines": len(applied),
        "instances": len({event.instance for event in applied}),
        "committed_heights": len(commits),
    }
    return violations, checked


def check_sign_once(records: Sequence[Record]) -> tuple[list[Violation], dict[str, Any]]:
    """O-SIGN over every durable record of every key (all nodes, all boots)."""
    violations: list[Violation] = []
    by_key: dict[str, list[Record]] = {}
    for record in records:
        by_key.setdefault(record.key, []).append(record)
    for key, key_records in sorted(by_key.items()):
        nodes = sorted({record.node for record in key_records})
        if len(nodes) > 1:
            violations.append(
                Violation("O-SIGN", "shared-key", {"key": key, "nodes": nodes})
            )
            key_records.sort(key=lambda record: (record.ts_ms, record.node, record.boot, record.line))
        else:
            key_records.sort(key=lambda record: (record.boot, record.line))
        violations.extend(_sign_once_of_key(key, key_records))
    checked = {"records": len(records), "keys": len(by_key)}
    return violations, checked


def _sign_once_of_key(key: str, records: Sequence[Record]) -> list[Violation]:
    violations: list[Violation] = []
    # Per instance: the last height and, per height, what the key made durable so far.
    heights: dict[str, int] = {}
    state: dict[tuple[str, int], dict[str, Any]] = {}

    def violation(kind: str, record: Record, **detail: Any) -> None:
        violations.append(
            Violation(
                "O-SIGN",
                kind,
                {
                    "key": key,
                    "instance": record.instance,
                    "height": record.height,
                    "node": record.node,
                    "boot": record.boot,
                    "line": record.line,
                    **detail,
                },
            )
        )

    for record in records:
        last_height = heights.get(record.instance)
        if last_height is not None and record.height < last_height:
            violation("record-regression", record, previous_height=last_height)
        heights[record.instance] = max(record.height, last_height or record.height)
        slot = state.setdefault(
            (record.instance, record.height),
            {
                "proposals": {},
                "prepares": {},
                "locks": {},
                "timeouts": {},
                "max_timeout_view": None,
                "max_lock_view": None,
                "last": Signed(),
            },
        )
        signed = record.signed
        last: Signed = slot["last"]
        # A newer record never loses an entry nor lowers its view (§7.4 record provenance).
        for kind in ("proposal", "prepare", "lock", "timeout"):
            before = getattr(last, kind)
            now = getattr(signed, kind)
            if before is not None and (now is None or now[0] < before[0]):
                violation(
                    "record-regression",
                    record,
                    entry=kind,
                    before=list(before),
                    now=None if now is None else list(now),
                )
        if signed.proposal is not None:
            view, block = signed.proposal
            seen = slot["proposals"].setdefault(view, block)
            if seen != block:
                violation("double-proposal", record, view=view, blocks=[seen, block])
        if signed.prepare is not None:
            view = signed.prepare[0]
            preimage = signed.prepare[1:]
            if view not in slot["prepares"]:
                max_timeout = slot["max_timeout_view"]
                if max_timeout is not None and view <= max_timeout:
                    violation(
                        "prepare-after-timeout", record, view=view, timeout_view=max_timeout
                    )
            seen = slot["prepares"].setdefault(view, preimage)
            if seen != preimage:
                violation(
                    "double-prepare", record, view=view, preimages=[list(seen), list(preimage)]
                )
        if signed.lock is not None:
            view = signed.lock[0]
            preimage = signed.lock[1:]
            seen = slot["locks"].setdefault(view, preimage)
            if seen != preimage:
                violation(
                    "conflicting-lock", record, view=view, preimages=[list(seen), list(preimage)]
                )
        if signed.timeout is not None:
            view, carried = signed.timeout
            if view not in slot["timeouts"]:
                max_lock = slot["max_lock_view"]
                if max_lock is not None and (carried is None or carried < max_lock):
                    violation(
                        "timeout-below-lock", record, view=view, carried=carried, lock_view=max_lock
                    )
            seen = slot["timeouts"].setdefault(view, carried)
            if seen != carried:
                violation("double-timeout", record, view=view, carried=[seen, carried])
        # What this record made durable bounds the next records (strictly later signatures).
        if signed.timeout is not None:
            slot["max_timeout_view"] = max(
                signed.timeout[0],
                slot["max_timeout_view"] if slot["max_timeout_view"] is not None else -1,
            )
        if signed.lock is not None:
            slot["max_lock_view"] = max(
                signed.lock[0],
                slot["max_lock_view"] if slot["max_lock_view"] is not None else -1,
            )
        slot["last"] = signed
    return violations


def observed_instance(applied: Sequence[Applied]) -> Optional[str]:
    """The instance O-LIVE and O-PERF observe: the one with the most applied lines.

    The load drives the global chain; lanes idle without routed work (idle chains create no
    blocks), so they are reported but not held to the liveness bound.
    """
    counts: dict[str, int] = {}
    for event in applied:
        counts[event.instance] = counts.get(event.instance, 0) + 1
    if not counts:
        return None
    return sorted(counts.items(), key=lambda item: (-item[1], item[0]))[0][0]


def check_liveness(
    applied: Sequence[Applied],
    timeline: Timeline,
    instance: Optional[str],
    bound_ms: float,
) -> tuple[list[Violation], dict[str, Any]]:
    """O-LIVE: commits resume within ``bound_ms`` after every heal and never stall after it."""
    violations: list[Violation] = []
    intervals = timeline.steady_intervals()
    commits: dict[str, list[float]] = {node: [] for node in timeline.nodes}
    for event in applied:
        if event.instance == instance and event.node in commits:
            commits[event.node].append(event.ts_ms)
    for times in commits.values():
        times.sort()
    windows_checked = 0
    for start, end in intervals:
        for node in timeline.nodes:
            windows_checked += 1
            cursor = start
            for ts in (ts for ts in commits[node] if start <= ts < end):
                if ts - cursor > bound_ms:
                    violations.append(
                        Violation(
                            "O-LIVE",
                            "no-commit-within-bound",
                            {
                                "node": node,
                                "instance": instance,
                                "from_ms": cursor,
                                "next_commit_ms": ts,
                                "waited_ms": ts - cursor,
                                "bound_ms": bound_ms,
                            },
                        )
                    )
                cursor = ts
            if end - cursor > bound_ms:
                violations.append(
                    Violation(
                        "O-LIVE",
                        "no-commit-within-bound",
                        {
                            "node": node,
                            "instance": instance,
                            "from_ms": cursor,
                            "next_commit_ms": None,
                            "waited_ms": end - cursor,
                            "bound_ms": bound_ms,
                        },
                    )
                )
    if instance is None and intervals:
        violations.append(
            Violation("O-LIVE", "no-commit-at-all", {"nodes": list(timeline.nodes)})
        )
    tolerated = [
        window
        for window in timeline.windows
        if window.kind in timeline.tolerated_exit_kinds
    ]
    for boot in timeline.boots:
        if not boot.ended.startswith("exited"):
            continue
        end = boot.end_ms if boot.end_ms is not None else timeline.end_ms
        if any(
            boot.node in window.nodes and window.start_ms <= end <= window.end_ms
            for window in tolerated
        ):
            continue
        violations.append(
            Violation(
                "O-LIVE",
                "unexpected-exit",
                {"node": boot.node, "boot": boot.index, "at_ms": end, "ended": boot.ended},
            )
        )
    checked = {
        "instance": instance,
        "bound_ms": bound_ms,
        "steady_intervals": len(intervals),
        "node_windows": windows_checked,
    }
    return violations, checked


def percentile(values: Sequence[float], fraction: float) -> Optional[float]:
    """Nearest-rank percentile of ``values`` (``None`` when empty)."""
    if not values:
        return None
    ordered = sorted(values)
    rank = max(1, math.ceil(fraction * len(ordered)))
    return ordered[min(rank, len(ordered)) - 1]


def committed_transactions(samples: Sequence[LoadSample]) -> Optional[int]:
    """Committed transactions seen by the best-observed node (counters restart with a boot)."""
    if not samples:
        return None
    per_node: dict[str, int] = {}
    segments: dict[tuple[str, int], list[int]] = {}
    for sample in samples:
        segments.setdefault((sample.node, sample.boot), []).append(sample.txs_approved)
    for (node, _boot), values in segments.items():
        per_node[node] = per_node.get(node, 0) + max(values) - min(values)
    return max(per_node.values())


def check_performance(
    applied: Sequence[Applied],
    timeline: Timeline,
    instance: Optional[str],
    thresholds: Thresholds,
    load: Optional[LoadRecord],
) -> tuple[list[Violation], dict[str, Any]]:
    """O-PERF: gap and latency percentiles in fault-free intervals, and committed throughput."""
    violations: list[Violation] = []
    intervals = timeline.steady_intervals()
    gaps: list[float] = []
    blocks_best = 0
    payload_best = 0
    by_node: dict[str, list[Applied]] = {}
    for event in applied:
        if event.instance == instance:
            by_node.setdefault(event.node, []).append(event)
    for events in by_node.values():
        events.sort(key=lambda event: event.ts_ms)
        node_blocks = 0
        node_payload = 0
        for start, end in intervals:
            inside = [event for event in events if start <= event.ts_ms < end]
            node_blocks += len(inside)
            node_payload += sum(event.payload_bytes for event in inside)
            steady = inside[thresholds.warmup_heights :]
            gaps.extend(
                later.ts_ms - earlier.ts_ms for earlier, later in zip(steady, steady[1:])
            )
        blocks_best = max(blocks_best, node_blocks)
        payload_best = max(payload_best, node_payload)
    steady_ms = sum(end - start for start, end in intervals)
    run_ms = max(timeline.end_ms - timeline.start_ms, 1.0)
    metrics: dict[str, Any] = {
        "instance": instance,
        "steady_seconds": round(steady_ms / 1000.0, 3),
        "gap_samples": len(gaps),
        "gap_p50_ms": _round(percentile(gaps, 0.50)),
        "gap_p90_ms": _round(percentile(gaps, 0.90)),
        "gap_p99_ms": _round(percentile(gaps, 0.99)),
        "gap_max_ms": _round(max(gaps) if gaps else None),
        "blocks_per_second": round(blocks_best * 1000.0 / steady_ms, 4) if steady_ms else None,
        "payload_bytes_per_second": round(payload_best * 1000.0 / steady_ms, 1)
        if steady_ms
        else None,
    }
    p99 = metrics["gap_p99_ms"]
    if p99 is None:
        violations.append(
            Violation(
                "O-PERF",
                "no-gap-samples",
                {"warmup_heights": thresholds.warmup_heights, "steady_seconds": metrics["steady_seconds"]},
            )
        )
    else:
        if p99 > thresholds.max_gap_p99_ms:
            violations.append(
                Violation(
                    "O-PERF",
                    "gap-p99-above-threshold",
                    {"gap_p99_ms": p99, "threshold_ms": thresholds.max_gap_p99_ms},
                )
            )
        if metrics["gap_max_ms"] > thresholds.max_gap_ms:
            violations.append(
                Violation(
                    "O-PERF",
                    "gap-max-above-threshold",
                    {"gap_max_ms": metrics["gap_max_ms"], "threshold_ms": thresholds.max_gap_ms},
                )
            )
    committed = committed_transactions(load.samples) if load is not None else None
    tps = None if committed is None else committed * 1000.0 / run_ms
    metrics["committed_transactions"] = committed
    metrics["committed_tps"] = None if tps is None else round(tps, 4)
    metrics["submitted_transactions"] = None if load is None else load.submitted
    metrics["attempted_transactions"] = None if load is None else load.attempted
    if thresholds.min_tps > 0:
        if tps is None:
            violations.append(
                Violation("O-PERF", "throughput-unmeasured", {"min_tps": thresholds.min_tps})
            )
        elif tps < thresholds.min_tps:
            violations.append(
                Violation(
                    "O-PERF",
                    "throughput-below-threshold",
                    {"committed_tps": round(tps, 4), "min_tps": thresholds.min_tps},
                )
            )
    latencies = []
    failed_probes = 0
    if load is not None:
        for probe in load.probes:
            if not any(start <= probe.start_ms and probe.end_ms <= end for start, end in intervals):
                continue
            if probe.ok:
                latencies.append(probe.end_ms - probe.start_ms)
            else:
                failed_probes += 1
    metrics["latency_samples"] = len(latencies)
    metrics["latency_failed_probes"] = failed_probes
    metrics["latency_p50_ms"] = _round(percentile(latencies, 0.50))
    metrics["latency_p90_ms"] = _round(percentile(latencies, 0.90))
    metrics["latency_p99_ms"] = _round(percentile(latencies, 0.99))
    latency_p99 = metrics["latency_p99_ms"]
    if latency_p99 is not None and latency_p99 > thresholds.max_latency_p99_ms:
        violations.append(
            Violation(
                "O-PERF",
                "latency-p99-above-threshold",
                {"latency_p99_ms": latency_p99, "threshold_ms": thresholds.max_latency_p99_ms},
            )
        )
    if load is not None and load.probes and not latencies:
        violations.append(
            Violation(
                "O-PERF",
                "no-successful-latency-probe",
                {"probes": len(load.probes), "failed_in_steady_intervals": failed_probes},
            )
        )
    metrics["thresholds"] = {
        "max_gap_p99_ms": thresholds.max_gap_p99_ms,
        "max_gap_ms": thresholds.max_gap_ms,
        "max_latency_p99_ms": thresholds.max_latency_p99_ms,
        "min_tps": thresholds.min_tps,
        "warmup_heights": thresholds.warmup_heights,
    }
    return violations, metrics


def _round(value: Optional[float]) -> Optional[float]:
    return None if value is None else round(value, 3)


def _merged_windows(windows: Iterable[Window]) -> list[tuple[float, float]]:
    merged: list[tuple[float, float]] = []
    for start, end in sorted((window.start_ms, window.end_ms) for window in windows):
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


# ---------------------------------------------------------------------------------------------
# Liveness bound (§8.2) and verdict
# ---------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class LiveBoundParams:
    """Inputs of ``B_live`` (§8.2) with the §9.3 defaults and the §9.4 nominal delays."""

    n: int
    t_max_eff_ms: float = 30_000.0
    payload_retry_interval_ms: float = 5_000.0
    build_timeout_ms: float = 200.0
    delta_ms: float = 500.0
    e_max_ms: float = 4_000.0
    a_max_ms: float = 1_000.0
    sync_batch: int = 64
    sync_retry_ms: float = 1_000.0
    lag_heights: int = 64
    t_base_ms: Optional[float] = None
    rebroadcast_interval_ms: Optional[float] = None
    fetch_retry_ms: Optional[float] = None
    growth: float = 1.5


def live_bound_ms(params: LiveBoundParams) -> float:
    """``B_live = (f + 2 + level_cap)·B_view + ⌈lag/sync_batch⌉·(sync_retry + 2Δ + sync_batch·(E_max + A_max))``."""
    n = params.n
    f = (n - 1) // 3
    small = n <= 4
    t_base = params.t_base_ms if params.t_base_ms is not None else (2_000.0 if small else 3_000.0)
    rebroadcast = (
        params.rebroadcast_interval_ms
        if params.rebroadcast_interval_ms is not None
        else (500.0 if small else 1_000.0)
    )
    fetch_retry = (
        params.fetch_retry_ms if params.fetch_retry_ms is not None else (250.0 if small else 500.0)
    )
    level_cap = 0
    timeout = t_base
    while timeout < params.t_max_eff_ms:
        level_cap += 1
        timeout *= params.growth
    fetch = math.ceil(math.log2(f + 1)) * fetch_retry
    b_view = (
        params.t_max_eff_ms
        + params.payload_retry_interval_ms
        + 2 * params.build_timeout_ms
        + 2 * rebroadcast
        + 4 * params.delta_ms
        + fetch
    )
    sync = math.ceil(params.lag_heights / params.sync_batch) * (
        params.sync_retry_ms
        + 2 * params.delta_ms
        + params.sync_batch * (params.e_max_ms + params.a_max_ms)
    )
    return (f + 2 + level_cap) * b_view + sync


def build_verdict(
    logs: Mapping[str, NodeLog],
    timeline: Timeline,
    thresholds: Thresholds,
    load: Optional[LoadRecord],
    run: Optional[Mapping[str, Any]] = None,
) -> dict[str, Any]:
    """Compute every oracle and the overall verdict (``ok`` is false on any violation)."""
    applied = [event for log in logs.values() for event in log.applied]
    records = [record for log in logs.values() for record in log.records]
    instance = observed_instance(applied)
    agr, agr_checked = check_agreement(applied)
    sign, sign_checked = check_sign_once(records)
    live, live_checked = check_liveness(applied, timeline, instance, thresholds.live_bound_ms)
    perf, perf_metrics = check_performance(applied, timeline, instance, thresholds, load)
    harness: list[Violation] = []
    parse_errors = [error for log in logs.values() for error in log.parse_errors]
    if parse_errors:
        harness.append(
            Violation(
                "HARNESS", "unparsed-audit-lines", {"count": len(parse_errors), "first": parse_errors[:5]}
            )
        )
    missing = [node for node in timeline.nodes if node not in logs or not logs[node].applied]
    if missing:
        harness.append(Violation("HARNESS", "node-without-applied-lines", {"nodes": missing}))
    silent = [node for node in timeline.nodes if node not in logs or not logs[node].records]
    if silent:
        # Without durable-record lines O-SIGN is blind (the audit DEBUG filter is missing).
        harness.append(Violation("HARNESS", "node-without-record-lines", {"nodes": silent}))

    def oracle(violations: list[Violation], checked: Mapping[str, Any]) -> dict[str, Any]:
        return {
            "ok": not violations,
            "violations": [violation.to_json() for violation in violations[:200]],
            "violation_count": len(violations),
            "checked": dict(checked),
        }

    instances: dict[str, dict[str, Any]] = {}
    for event in applied:
        entry = instances.setdefault(event.instance, {"applied_lines": 0, "max_height": 0})
        entry["applied_lines"] += 1
        entry["max_height"] = max(entry["max_height"], event.height)
    oracles = {
        "O-AGR": oracle(agr, agr_checked),
        "O-SIGN": oracle(sign, sign_checked),
        "O-LIVE": oracle(live, live_checked),
        "O-PERF": oracle(perf, perf_metrics),
    }
    verdict = {
        "schema": VERDICT_SCHEMA,
        "version": VERDICT_VERSION,
        "ok": all(entry["ok"] for entry in oracles.values()) and not harness,
        "oracles": oracles,
        "harness": [violation.to_json() for violation in harness],
        "instances": instances,
        "nodes": {
            name: {
                "boots": log.boots,
                "lines": log.lines,
                "applied_lines": len(log.applied),
                "record_lines": len(log.records),
                "max_height": max(
                    (event.height for event in log.applied if event.instance == instance),
                    default=None,
                ),
                "observations": dict(sorted(log.observations.items())),
                "examples": dict(sorted(log.examples.items())),
            }
            for name, log in sorted(logs.items())
        },
        "timeline": {
            "duration_seconds": round((timeline.end_ms - timeline.start_ms) / 1000.0, 3),
            "fault_windows": len(timeline.windows),
            "fault_kinds": sorted({window.kind for window in timeline.windows}),
            "boots": len(timeline.boots),
            "kills": sum(1 for boot in timeline.boots if boot.ended == "killed"),
        },
    }
    if run is not None:
        verdict["run"] = dict(run)
    return verdict
