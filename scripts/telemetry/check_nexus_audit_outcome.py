#!/usr/bin/env python3
"""Validate routed-trace audit telemetry and archive the outcome payload.

The script scans a telemetry JSON log (as produced by `iroha_logger` in JSON
mode), extracts the most recent `nexus.audit.outcome` event for the requested
trace, and writes the payload to `docs/examples/nexus_audit_outcomes/` for
record keeping.  It fails with a non-zero exit code when:

* no matching event is found;
* the most recent event falls outside the expected time window; or
* the event reports a disallowed status (defaults to `fail`).

Typical usage during a TRACE rehearsal or audit window:

```
scripts/telemetry/check_nexus_audit_outcome.py \
  --log /var/log/iroha/telemetry.json \
  --trace-id TRACE-TELEMETRY-BRIDGE \
  --window-start 2026-02-24T10:00Z \
  --window-minutes 30
```

The script prints a short summary on success and records a JSON artefact that
includes the parsed payload, the associated log entry, and helper metadata.
Integrate it with the routed-trace pipeline or CI to gate sign-off on the audit
window.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, Optional, Tuple

DEFAULT_OK_STATUSES = {"pass", "mitigated"}
DEFAULT_FAIL_STATUSES = {"fail"}
DEFAULT_OUTPUT_DIR = Path("docs/examples/nexus_audit_outcomes")


class AuditError(RuntimeError):
    """Raised when the audit outcome validation fails."""


def _parse_iso8601(value: str) -> dt.datetime:
    value = value.strip()
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        return dt.datetime.fromisoformat(value)
    except ValueError as exc:  # fall back to parsing without timezone
        raise AuditError(f"unable to parse timestamp '{value}': {exc}") from exc


def _parse_timestamp(raw: Optional[str]) -> Optional[dt.datetime]:
    if not raw or not isinstance(raw, str):
        return None
    try:
        return _parse_iso8601(raw)
    except AuditError:
        return None


def _extract_timestamp(entry: Dict[str, Any]) -> Optional[dt.datetime]:
    for key in ("timestamp", "ts", "time", "when"):
        ts = _parse_timestamp(entry.get(key))
        if ts is not None:
            return ts
    return None


def _iter_audit_events(log_path: Path) -> Iterable[Tuple[Dict[str, Any], Dict[str, Any]]]:
    with log_path.open("r", encoding="utf-8") as handle:
        for raw_line in handle:
            line = raw_line.strip()
            if not line or not line.startswith("{"):
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            fields = record.get("fields")
            if not isinstance(fields, dict):
                continue
            if fields.get("msg") != "nexus.audit.outcome":
                continue
            event_payload = fields.get("event")
            if isinstance(event_payload, str):
                try:
                    event_payload = json.loads(event_payload)
                except json.JSONDecodeError:
                    continue
            if not isinstance(event_payload, dict):
                continue
            record_ts = _extract_timestamp(record)
            if record_ts is None:
                record_ts = dt.datetime.utcnow().replace(tzinfo=dt.timezone.utc)
            yield event_payload, {"log": record, "timestamp": record_ts.isoformat()}


def _select_event(
    events: Iterable[Tuple[Dict[str, Any], Dict[str, Any]]],
    trace_id: str,
    min_slot: int,
) -> Optional[Tuple[Dict[str, Any], Dict[str, Any]]]:
    selected: Optional[Tuple[Dict[str, Any], Dict[str, Any]]] = None
    for payload, meta in events:
        if payload.get("trace_id") != trace_id:
            continue
        slot = int(payload.get("slot_height", 0))
        if slot < min_slot:
            continue
        if selected is None or meta["timestamp"] > selected[1]["timestamp"]:
            selected = (payload, meta)
    return selected


def _within_window(timestamp: dt.datetime, start: dt.datetime, minutes: int) -> bool:
    end = start + dt.timedelta(minutes=minutes)
    return start <= timestamp <= end


def _write_archive(
    output_dir: Path,
    trace_id: str,
    payload: Dict[str, Any],
    meta: Dict[str, Any],
) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    ts = meta["timestamp"].replace(":", "").replace("-", "")
    status = payload.get("status", "unknown")
    filename = f"{trace_id}-{ts}-{status}.json"
    archive_path = output_dir / filename
    content = {
        "trace_id": trace_id,
        "captured_at": meta["timestamp"],
        "payload": payload,
        "log": meta["log"],
    }
    archive_path.write_text(json.dumps(content, indent=2), encoding="utf-8")
    return archive_path


def main(argv: Optional[Iterable[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", required=True, type=Path, help="Telemetry JSON log file")
    parser.add_argument("--trace-id", required=True, help="Audit trace identifier to match")
    parser.add_argument(
        "--window-start",
        help="Expected audit window start time (ISO-8601, e.g., 2026-02-24T10:00Z)",
    )
    parser.add_argument(
        "--window-minutes",
        type=int,
        default=30,
        help="Grace window after --window-start before considering the audit missing",
    )
    parser.add_argument(
        "--min-slot-height",
        type=int,
        default=0,
        help="Ignore audit outcomes recorded before this slot height",
    )
    parser.add_argument(
        "--ok-status",
        action="append",
        help="Statuses treated as successful (defaults to pass and mitigated)",
    )
    parser.add_argument(
        "--fail-status",
        action="append",
        help="Statuses treated as fatal (defaults to fail)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="Directory where audit payload snapshots are stored",
    )

    args = parser.parse_args(argv)
    if not args.log.exists():
        raise AuditError(f"telemetry log not found: {args.log}")

    ok_statuses = set(args.ok_status or DEFAULT_OK_STATUSES)
    fail_statuses = set(args.fail_status or DEFAULT_FAIL_STATUSES)
    if ok_statuses & fail_statuses:
        raise AuditError("overlapping ok/fail status sets are not allowed")

    selected = _select_event(_iter_audit_events(args.log), args.trace_id, args.min_slot_height)
    if selected is None:
        raise AuditError(
            f"no nexus.audit.outcome entry found for trace {args.trace_id} (slot >= {args.min_slot_height})"
        )

    payload, meta = selected
    event_ts = _parse_iso8601(meta["timestamp"])

    if args.window_start:
        window_start = _parse_iso8601(args.window_start)
        if not _within_window(event_ts, window_start, args.window_minutes):
            raise AuditError(
                f"latest audit outcome ({event_ts.isoformat()}) is outside the expected window"
            )

    status = str(payload.get("status", "unknown"))
    if status in fail_statuses:
        raise AuditError(
            f"audit trace {args.trace_id} reported failing status '{status}'"
        )
    if ok_statuses and status not in ok_statuses:
        raise AuditError(
            f"audit trace {args.trace_id} reported unrecognised status '{status}'"
        )

    archive_path = _write_archive(args.output_dir, args.trace_id, payload, meta)
    reviewer = payload.get("reviewer", "unknown")
    mitigation = payload.get("mitigation_url") or "(none)"
    print(
        f"Trace {args.trace_id} status {status} at {event_ts.isoformat()} (reviewer {reviewer}); archive: {archive_path}"
    )
    if mitigation and mitigation != "(none)":
        print(f"Mitigation reference: {mitigation}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except AuditError as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(2)
    except Exception as exc:  # pragma: no cover - defensive guard
        print(f"unexpected failure: {exc}", file=sys.stderr)
        sys.exit(3)
