#!/usr/bin/env python3
"""
Summarise Sumeragi pacemaker backpressure deferrals and correlate them with
DA availability warning log entries.

The script scans one or more Iroha log files (plain text or JSON lines), finds
every occurrence of the debug message
``Pacemaker deferred proposal assembly due to saturated transaction queue`` and
reports nearby DA/RBC maintenance events such as:
* ``DA availability still missing (advisory)``
* ``DA availability evidence observed``
* ``failed to purge persisted RBC session``

Typical usage::

    python3 scripts/sumeragi_backpressure_log_scraper.py \
        /var/log/irohad.log --window-before 30 --window-after 5

    python3 scripts/sumeragi_backpressure_log_scraper.py - --status status.json

Use ``--json`` for machine-readable output or ``--status`` to include counters
from a `/v2/sumeragi/status` snapshot. Passing ``-`` reads log lines from
standard input.
"""

from __future__ import annotations

import argparse
import json
import sys
import datetime as _dt
import re
from bisect import bisect_left, bisect_right
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

PACEMAKER_MESSAGE = (
    "Pacemaker deferred proposal assembly due to saturated transaction queue"
)
DA_AVAILABILITY_MESSAGE = "DA availability still missing (advisory)"
DA_AVAILABILITY_SATISFIED_MESSAGE = "DA availability evidence observed"
RBC_PURGE_FAIL_MESSAGE = "failed to purge persisted RBC session"

TIMESTAMP_KEYS = ("timestamp", "time", "ts", "@timestamp")


@dataclass
class LogEvent:
    """Structured representation of a single log line."""

    timestamp: _dt.datetime
    message: str
    level: Optional[str]
    fields: Dict[str, str]
    raw: str
    event_type: Optional[str] = None


def parse_timestamp(raw: str) -> Optional[_dt.datetime]:
    """Parse an ISO-8601 timestamp, tolerating `Z` or space separators."""
    value = raw.strip()
    if not value:
        return None
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    if " " in value and "T" not in value:
        value = value.replace(" ", "T", 1)
    try:
        return _dt.datetime.fromisoformat(value)
    except ValueError:
        return None


def _extract_fields_from_json(record: Dict) -> Tuple[Optional[str], Dict[str, str]]:
    message = None
    fields: Dict[str, str] = {}
    if "fields" in record and isinstance(record["fields"], dict):
        for key, value in record["fields"].items():
            if key == "message":
                message = str(value)
            else:
                fields[key] = str(value)
    if message is None and "message" in record:
        message = str(record["message"])
    return message, fields


def _extract_text_fields(text: str) -> Dict[str, str]:
    pattern = re.compile(r"([A-Za-z0-9_./]+)=([^,\s]+)")
    return {match.group(1): match.group(2).rstrip(",") for match in pattern.finditer(text)}


def parse_log_line(line: str) -> Optional[LogEvent]:
    """Parse a log line (JSON or text)."""
    stripped = line.strip()
    if not stripped:
        return None

    timestamp: Optional[_dt.datetime] = None
    level: Optional[str] = None
    message: Optional[str] = None
    fields: Dict[str, str] = {}

    if stripped.startswith("{") and stripped.endswith("}"):
        try:
            record = json.loads(stripped)
        except json.JSONDecodeError:
            record = None
        if isinstance(record, dict):
            for key in TIMESTAMP_KEYS:
                if key in record:
                    timestamp = parse_timestamp(str(record[key]))
                    if timestamp:
                        break
            level = record.get("level") or record.get("lvl")
            message, fields = _extract_fields_from_json(record)
            if message is None:
                message = stripped
        else:
            record = None
    else:
        record = None

    if record is None:
        # Fallback to textual parsing.
        # Expect format: "<ts> <level> <target>: message"
        match = re.match(
            r"^(?P<ts>\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(?P<rest>.*)$",
            stripped,
        )
        if not match:
            return None
        timestamp = parse_timestamp(match.group("ts"))
        if not timestamp:
            return None
        rest = match.group("rest").strip()
        parts = rest.split(None, 2)
        if parts and parts[0].isupper():
            level = parts[0]
            parts = parts[1:]
        if parts:
            # If there is a colon, separate target and message.
            if ": " in parts[-1]:
                target_and_msg = parts[-1].split(": ", 1)
                if len(target_and_msg) == 2:
                    message = target_and_msg[1]
                else:
                    message = target_and_msg[0]
            else:
                message = parts[-1]
        else:
            message = rest
        fields = _extract_text_fields(rest)

    if message is None or timestamp is None:
        return None

    # Merge any leftover key=value pairs from message body for JSON logs.
    if record is not None:
        merged_msg_fields = _extract_text_fields(message)
        for key, value in merged_msg_fields.items():
            fields.setdefault(key, value)

    event = LogEvent(timestamp=timestamp, message=message, level=level, fields=fields, raw=line.rstrip("\n"))
    event.event_type = classify_event(event)
    return event


def classify_event(event: LogEvent) -> Optional[str]:
    """Identify Sumeragi-specific events we care about."""
    lower_message = event.message.lower()
    if PACEMAKER_MESSAGE.lower() in lower_message:
        return "pacemaker-deferral"
    if DA_AVAILABILITY_MESSAGE.lower() in lower_message:
        return "da-availability-missing"
    if DA_AVAILABILITY_SATISFIED_MESSAGE.lower() in lower_message:
        return "da-availability-satisfied"
    if RBC_PURGE_FAIL_MESSAGE.lower() in lower_message:
        return "rbc-purge-failed"
    return None


def load_events(paths: Sequence[str]) -> List[LogEvent]:
    """Parse all provided paths (files or ``-``) into a sorted list of events."""
    events: List[LogEvent] = []
    stdin_consumed = False
    for raw in paths:
        if raw == "-":
            if stdin_consumed:
                print("[warn] stdin already consumed; skipping duplicate '-'", file=sys.stderr)
                continue
            stdin_consumed = True
            for line in sys.stdin:
                event = parse_log_line(line)
                if event is not None:
                    events.append(event)
            continue

        path = Path(raw)
        try:
            with path.open("r", encoding="utf-8") as handle:
                for line in handle:
                    event = parse_log_line(line)
                    if event is not None:
                        events.append(event)
        except FileNotFoundError:
            print(f"[warn] log file not found: {path}", file=sys.stderr)
    events.sort(key=lambda evt: evt.timestamp)
    return events


def correlate_events(
    pacemaker_events: Sequence[LogEvent],
    related_events: Sequence[LogEvent],
    window_before: _dt.timedelta,
    window_after: _dt.timedelta,
) -> List[Tuple[LogEvent, List[LogEvent]]]:
    """Associate each pacemaker deferral with nearby DA/RBC activity."""
    related_times = [evt.timestamp for evt in related_events]
    correlated: List[Tuple[LogEvent, List[LogEvent]]] = []
    for pacemaker in pacemaker_events:
        start_time = pacemaker.timestamp - window_before
        end_time = pacemaker.timestamp + window_after
        left = bisect_left(related_times, start_time)
        right = bisect_right(related_times, end_time)
        correlated.append((pacemaker, list(related_events[left:right])))
    return correlated


def summarise_fields(fields: Dict[str, str], keys: Sequence[str]) -> str:
    parts = []
    for key in keys:
        if key in fields:
            parts.append(f"{key}={fields[key]}")
    return ", ".join(parts)


def load_status_metrics(path: Optional[Path]) -> Dict[str, Optional[int]]:
    if path is None:
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError):
        return {}
    keys = [
        "pacemaker_backpressure_deferrals_total",
        "da_reschedule_total",
    ]
    metrics: Dict[str, Optional[int]] = {}

    def search(obj: object, key: str) -> Optional[int]:
        if isinstance(obj, dict):
            if key in obj:
                try:
                    return int(obj[key])
                except (ValueError, TypeError):
                    return None
            for child in obj.values():
                result = search(child, key)
                if result is not None:
                    return result
        elif isinstance(obj, list):
            for child in obj:
                result = search(child, key)
                if result is not None:
                    return result
        return None

    for key in keys:
        metrics[key] = search(data, key)
    return metrics


def human_summary(
    correlated: Sequence[Tuple[LogEvent, List[LogEvent]]],
    metrics: Dict[str, Optional[int]],
    window_before: _dt.timedelta,
    window_after: _dt.timedelta,
) -> None:
    total_pacemakers = len(correlated)
    total_related = sum(len(group) for _, group in correlated)
    print(
        f"Detected {total_pacemakers} pacemaker backpressure deferral(s) "
        f"and {total_related} correlated DA/RBC event(s) "
        f"(window -{window_before.total_seconds():.0f}s/+{window_after.total_seconds():.0f}s)."
    )
    if metrics:
        metric_parts = [
            f"{key}={value}" for key, value in metrics.items() if value is not None
        ]
        if metric_parts:
            print("Status counters: " + ", ".join(metric_parts))
    for pacemaker, rbc_group in correlated:
        depth_info = summarise_fields(pacemaker.fields, ["tx_queue_depth", "tx_queue_capacity"])
        print(f"- {pacemaker.timestamp.isoformat()} — pacemaker deferral ({depth_info or 'no depth info'})")
        if rbc_group:
            for rbc in rbc_group:
                details = summarise_fields(rbc.fields, ["height", "view", "attempt"])
                block_hash = rbc.fields.get("block_hash")
                if block_hash:
                    if len(block_hash) > 32:
                        block_hash = block_hash[:32] + "…"
                    details = f"{details}, block_hash={block_hash}" if details else f"block_hash={block_hash}"
                print(f"    • {rbc.timestamp.isoformat()} — {rbc.event_type} ({details or 'no context'})")
        else:
            print("    • No DA availability warning logs within correlation window.")


def json_summary(
    correlated: Sequence[Tuple[LogEvent, List[LogEvent]]],
    metrics: Dict[str, Optional[int]],
) -> None:
    payload = {
        "pacemaker_deferrals": [
            {
                "timestamp": pacemaker.timestamp.isoformat(),
                "message": pacemaker.message,
                "fields": pacemaker.fields,
                "rbc_events": [
                    {
                        "timestamp": rbc.timestamp.isoformat(),
                        "type": rbc.event_type,
                        "message": rbc.message,
                        "fields": rbc.fields,
                    }
                    for rbc in rbc_group
                ],
            }
            for pacemaker, rbc_group in correlated
        ],
        "status_counters": metrics,
    }
    json.dump(payload, sys.stdout, indent=2)
    print()


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Correlate pacemaker backpressure deferrals with DA availability warning and RBC retry/abort logs."
    )
    parser.add_argument("logs", nargs="*", help="Path(s) to log files to analyse.")
    parser.add_argument("--window-before", type=float, default=30.0, help="Seconds before a pacemaker deferral to consider DA/RBC events (default: 30).")
    parser.add_argument("--window-after", type=float, default=10.0, help="Seconds after a pacemaker deferral to consider DA/RBC events (default: 10).")
    parser.add_argument("--status", type=Path, help="Optional JSON snapshot from /v2/sumeragi/status to include counters.")
    parser.add_argument("--json", action="store_true", help="Emit JSON summary instead of human-readable text.")
    parser.add_argument("--self-test", action="store_true", help="Run built-in self tests and exit.")
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        run_self_tests()
        return 0
    if not args.logs:
        parser.error("at least one log file must be provided")

    events = load_events(args.logs)
    pacemaker_events = [evt for evt in events if evt.event_type == "pacemaker-deferral"]
    related_events = [
        evt
        for evt in events
        if evt.event_type
        and (evt.event_type.startswith("rbc-") or evt.event_type.startswith("da-"))
    ]
    window_before = _dt.timedelta(seconds=max(args.window_before, 0.0))
    window_after = _dt.timedelta(seconds=max(args.window_after, 0.0))
    correlated = correlate_events(pacemaker_events, related_events, window_before, window_after)
    metrics = load_status_metrics(args.status)

    if args.json:
        json_summary(correlated, metrics)
    else:
        human_summary(correlated, metrics, window_before, window_after)
    return 0


# ---------------------------------------------------------------------------
# Self tests
# ---------------------------------------------------------------------------

def run_self_tests() -> None:
    import io
    import unittest

    class ParserTests(unittest.TestCase):
        def test_parse_json_pacemaker(self) -> None:
            line = json.dumps(
                {
                    "timestamp": "2025-01-10T12:00:00.123Z",
                    "level": "DEBUG",
                    "target": "iroha_core::sumeragi::main_loop",
                    "fields": {
                        "tx_queue_depth": 16,
                        "tx_queue_capacity": 16,
                        "message": PACEMAKER_MESSAGE,
                    },
                }
            )
            event = parse_log_line(line)
            self.assertIsNotNone(event)
            assert event
            self.assertEqual(event.event_type, "pacemaker-deferral")
            self.assertEqual(event.fields.get("tx_queue_depth"), "16")

        def test_parse_text_da_availability(self) -> None:
            line = (
                "2025-01-10T12:00:05.000Z INFO iroha_core::sumeragi::main_loop "
                "height=125 view=0 attempts=2 block_hash=abc123 "
                f"{DA_AVAILABILITY_MESSAGE}"
            )
            event = parse_log_line(line)
            self.assertIsNotNone(event)
            assert event
            self.assertEqual(event.event_type, "da-availability-missing")
            self.assertEqual(event.fields.get("height"), "125")

        def test_correlation(self) -> None:
            base = _dt.datetime(2025, 1, 10, 12, 0, 0, tzinfo=_dt.timezone.utc)
            pacemaker = LogEvent(
                timestamp=base,
                message=PACEMAKER_MESSAGE,
                level="DEBUG",
                fields={},
                raw="",
                event_type="pacemaker-deferral",
            )
            da = LogEvent(
                timestamp=base + _dt.timedelta(seconds=5),
                message=DA_AVAILABILITY_MESSAGE,
                level="INFO",
                fields={"height": "123"},
                raw="",
                event_type="da-availability-missing",
            )
            correlated = correlate_events(
                [pacemaker],
                [da],
                window_before=_dt.timedelta(seconds=10),
                window_after=_dt.timedelta(seconds=10),
            )
            self.assertEqual(len(correlated), 1)
            _, group = correlated[0]
            self.assertEqual(len(group), 1)
            self.assertEqual(group[0].fields["height"], "123")

        def test_load_events_from_stdin(self) -> None:
            payload = (
                '{"timestamp":"2025-01-10T12:00:00Z","level":"DEBUG","fields":{"message":"'
                + PACEMAKER_MESSAGE
                + '","tx_queue_depth":8,"tx_queue_capacity":16}}\n'
                "2025-01-10T12:00:05Z DEBUG iroha_core::sumeragi::main_loop "
                f"height=42 {DA_AVAILABILITY_MESSAGE}\n"
            )
            buffer = io.StringIO(payload)
            original_stdin = sys.stdin
            try:
                sys.stdin = buffer
                events = load_events(["-"])
            finally:
                sys.stdin = original_stdin
            kinds = {evt.event_type for evt in events if evt.event_type}
            self.assertIn("pacemaker-deferral", kinds)
            self.assertIn("da-availability-missing", kinds)

    suite = unittest.TestSuite()
    suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(ParserTests))
    runner = unittest.TextTestRunner()
    runner.run(suite)


if __name__ == "__main__":
    sys.exit(main())
