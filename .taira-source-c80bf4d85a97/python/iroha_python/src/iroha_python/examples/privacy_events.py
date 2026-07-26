"""CLI helper for downloading relay `/privacy/events` feeds (PY6-P4/P5).

This example wraps :mod:`iroha_python.privacy` so operators can export
privacy-safe telemetry evidence without hand-rolling HTTP scripts.  It
supports both one-shot downloads (buffering the feed before writing it to a
JSON/NDJSON artefact) and streaming mode for long-running collectors.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import asdict
from pathlib import Path
from typing import IO, Iterable, Sequence

from iroha_python.privacy import (
    PrivacyEvent,
    PrivacyEventGarAbuseCategory,
    PrivacyEventThrottle,
    fetch_privacy_events,
    stream_privacy_events,
)


def _event_payload_to_dict(event: PrivacyEvent) -> dict | None:
    payload = event.payload
    if payload is None:
        return None
    if isinstance(payload, (PrivacyEventThrottle, PrivacyEventGarAbuseCategory)):
        # dataclasses.asdict handles nested optional ints for the other payloads.
        payload_dict = asdict(payload)
        if isinstance(payload, PrivacyEventThrottle):
            payload_dict["scope"] = payload.scope.value
        elif isinstance(payload, PrivacyEventGarAbuseCategory):
            payload_dict["label"] = payload.label
        return payload_dict
    payload_dict = asdict(payload)
    return payload_dict


def _event_to_dict(event: PrivacyEvent) -> dict:
    payload = _event_payload_to_dict(event)
    return {
        "timestamp_unix": event.timestamp_unix,
        "mode": event.mode.value,
        "kind": event.kind.value,
        "payload": payload,
    }


def _write_ndjson(events: Iterable[PrivacyEvent], sink: IO[str]) -> None:
    for event in events:
        sink.write(json.dumps(_event_to_dict(event), separators=(",", ":")))
        sink.write("\n")


def _write_json(events: Iterable[PrivacyEvent], sink: IO[str], indent: int | None) -> None:
    payload = [_event_to_dict(event) for event in events]
    json.dump(payload, sink, indent=indent)
    sink.write("\n")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Fetch privacy-safe telemetry from relay `/privacy/events` endpoints "
            "and emit JSON/NDJSON artefacts."
        )
    )
    parser.add_argument(
        "base_url",
        help="Relay admin base URL (e.g., http://relay:7070) or a full `/privacy/events` endpoint.",
    )
    parser.add_argument(
        "--path",
        default="/privacy/events",
        help="Endpoint appended to the base URL when it does not already end with `/privacy/events` (default: %(default)s).",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=10.0,
        help="HTTP timeout in seconds (default: %(default)s).",
    )
    parser.add_argument(
        "--output",
        default="-",
        help="File path for the exported events (default: stdout).",
    )
    parser.add_argument(
        "--format",
        choices=("ndjson", "json"),
        default="ndjson",
        help="Output format (default: %(default)s).",
    )
    parser.add_argument(
        "--indent",
        type=int,
        default=None,
        help="Indent level for JSON output (ignored for NDJSON).",
    )
    parser.add_argument(
        "--stream",
        action="store_true",
        help="Stream events continuously instead of downloading once (forces NDJSON output).",
    )
    parser.add_argument(
        "--max-events",
        type=int,
        default=None,
        help="Stop after writing the given number of events in streaming mode.",
    )
    return parser.parse_args(argv)


def run(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    output_path = args.output
    stream_mode = args.stream
    if stream_mode and args.format != "ndjson":
        print("error: streaming mode only supports --format ndjson", file=sys.stderr)
        return 1
    try:
        sink: IO[str]
        if output_path == "-":
            sink = sys.stdout
        else:
            sink = Path(output_path).open("w", encoding="utf-8")
        with sink:
            if stream_mode:
                iterator = stream_privacy_events(
                    args.base_url,
                    path=args.path,
                    timeout=args.timeout,
                )
                emitted = 0
                for event in iterator:
                    sink.write(json.dumps(_event_to_dict(event), separators=(",", ":")))
                    sink.write("\n")
                    emitted += 1
                    if args.max_events is not None and emitted >= args.max_events:
                        break
                sink.flush()
            else:
                events = fetch_privacy_events(
                    args.base_url,
                    path=args.path,
                    timeout=args.timeout,
                )
                if args.format == "json":
                    _write_json(events, sink, args.indent)
                else:
                    _write_ndjson(events, sink)
    except KeyboardInterrupt:
        return 130
    except Exception as exc:  # pragma: no cover - exercised via tests
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


def main() -> None:  # pragma: no cover - convenience entrypoint
    raise SystemExit(run())


__all__ = ["main", "parse_args", "run"]
