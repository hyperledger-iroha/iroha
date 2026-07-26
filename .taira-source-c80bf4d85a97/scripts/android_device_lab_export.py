#!/usr/bin/env python3
"""Export Android device-lab reservations into weekly ICS + JSON artefacts."""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
from pathlib import Path
import sys
import urllib.request
from typing import Iterable, Sequence

UTC = dt.timezone.utc


@dataclasses.dataclass
class CalendarEvent:
    """Minimal representation of a VEVENT entry."""

    start: dt.datetime
    end: dt.datetime
    summary: str | None
    uid: str | None
    location: str | None
    description: str | None
    raw_lines: list[str]

    def overlaps(self, start: dt.datetime, end: dt.datetime) -> bool:
        """Return ``True`` when the event intersects the supplied interval."""

        return self.start < end and self.end > start


@dataclasses.dataclass
class CalendarExportResult:
    """Holds details about an export run for testing/reporting."""

    iso_year: int
    iso_week: int
    calendar_name: str
    ics_path: Path
    json_path: Path | None
    events: list[CalendarEvent]


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fetch the Android device lab calendar and emit audited artefacts."
    )
    parser.add_argument(
        "--ics-path",
        type=str,
        help="Local .ics file to use as the calendar source (use '-' for stdin).",
    )
    parser.add_argument(
        "--ics-url",
        type=str,
        help="Remote .ics URL to download when no local path is supplied.",
    )
    parser.add_argument(
        "--week",
        type=str,
        help="ISO week in the form YYYY-Www (defaults to current week).",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("artifacts/android/device_lab"),
        help="Directory for the exported .ics/.json files.",
    )
    parser.add_argument(
        "--json-summary",
        type=Path,
        help="Optional path for the JSON summary (defaults beside the .ics file).",
    )
    parser.add_argument(
        "--calendar-name",
        default="Android Device Lab – Reservations",
        help="Human readable calendar label stored in the summary JSON.",
    )
    parser.add_argument(
        "--pretty-json",
        action="store_true",
        help="Indent the JSON summary for easier manual review.",
    )
    return parser.parse_args(argv)


def load_ics_text(path_arg: str | None, url: str | None) -> str:
    if path_arg and url:
        raise ValueError("Specify only one of --ics-path or --ics-url")
    if path_arg:
        if path_arg == "-":
            return sys.stdin.read()
        return Path(path_arg).read_text(encoding="utf-8")
    if url:
        with urllib.request.urlopen(url) as response:
            data = response.read()
            encoding = response.headers.get_content_charset("utf-8")
        return data.decode(encoding)
    raise ValueError("One of --ics-path or --ics-url is required")


def unfold_ics_lines(text: str) -> list[str]:
    """Collapse folded ICS lines per RFC 5545."""

    cleaned = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = cleaned.split("\n")
    unfolded: list[str] = []
    for raw in lines:
        if raw == "":
            unfolded.append("")
            continue
        if raw.startswith(" ") or raw.startswith("\t"):
            if not unfolded:
                raise ValueError("Encountered folded line without a parent line")
            unfolded[-1] = unfolded[-1] + raw[1:]
        else:
            unfolded.append(raw)
    return unfolded


def split_calendar_sections(lines: Iterable[str]) -> tuple[list[str], list[list[str]], list[str]]:
    header: list[str] = []
    footer: list[str] = []
    events: list[list[str]] = []
    current: list[str] | None = None
    for line in lines:
        if line == "BEGIN:VEVENT":
            current = [line]
            continue
        if line == "END:VEVENT" and current is not None:
            current.append(line)
            events.append(current)
            current = None
            continue
        if current is not None:
            current.append(line)
            continue
        if events:
            footer.append(line)
        else:
            header.append(line)
    return header, events, footer


def decode_ics_text(value: str | None) -> str | None:
    if value is None:
        return None
    return (
        value.replace("\\n", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .strip()
    )


def parse_dt(token: str) -> dt.datetime:
    token = token.strip()
    if token.endswith("Z"):
        naive = dt.datetime.strptime(token[:-1], "%Y%m%dT%H%M%S")
        return naive.replace(tzinfo=UTC)
    if token.isdigit() and len(token) == 8:
        date_only = dt.datetime.strptime(token, "%Y%m%d")
        return date_only.replace(tzinfo=UTC)
    try:
        naive = dt.datetime.strptime(token, "%Y%m%dT%H%M%S")
    except ValueError as exc:  # pragma: no cover - defensive path for millis
        raise ValueError(f"Unsupported datetime token: {token}") from exc
    return naive.replace(tzinfo=UTC)


def parse_event(lines: list[str]) -> CalendarEvent:
    props: dict[str, str] = {}
    for line in lines:
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        field = key.split(";", 1)[0].upper()
        props[field] = value
    start = parse_dt(props.get("DTSTART", "19700101T000000"))
    end = parse_dt(props.get("DTEND", props.get("DTSTART", "19700101T000000")))
    return CalendarEvent(
        start=start,
        end=end,
        summary=decode_ics_text(props.get("SUMMARY")),
        uid=decode_ics_text(props.get("UID")),
        location=decode_ics_text(props.get("LOCATION")),
        description=decode_ics_text(props.get("DESCRIPTION")),
        raw_lines=lines,
    )


def iso_week_bounds(iso_year: int, iso_week: int) -> tuple[dt.datetime, dt.datetime]:
    week_start = dt.datetime.fromisocalendar(iso_year, iso_week, 1).replace(
        tzinfo=UTC, hour=0, minute=0, second=0, microsecond=0
    )
    return week_start, week_start + dt.timedelta(days=7)


def export_calendar_week(
    ics_text: str,
    iso_year: int,
    iso_week: int,
    out_dir: Path,
    json_summary_path: Path | None,
    calendar_name: str,
    pretty_json: bool = False,
) -> CalendarExportResult:
    lines = unfold_ics_lines(ics_text)
    header, event_blocks, footer = split_calendar_sections(lines)
    events = [parse_event(block) for block in event_blocks]
    week_start, week_end = iso_week_bounds(iso_year, iso_week)
    filtered_blocks: list[list[str]] = []
    filtered_events: list[CalendarEvent] = []
    for event, block in zip(events, event_blocks):
        if event.overlaps(week_start, week_end):
            filtered_blocks.append(block)
            filtered_events.append(event)

    out_dir.mkdir(parents=True, exist_ok=True)
    week_label = f"{iso_year}-W{iso_week:02d}"
    ics_path = out_dir / f"{week_label}-calendar.ics"
    json_path = json_summary_path or out_dir / f"{week_label}-calendar.json"

    ics_lines = header + [line for block in filtered_blocks for line in block] + footer
    ics_content = "\r\n".join(line for line in ics_lines if line is not None) + "\r\n"
    ics_path.write_text(ics_content, encoding="utf-8")

    summary = {
        "calendar": calendar_name,
        "iso_year": iso_year,
        "iso_week": iso_week,
        "generated_at": dt.datetime.now(tz=UTC).isoformat(),
        "event_count": len(filtered_events),
        "events": [
            {
                "start": event.start.isoformat(),
                "end": event.end.isoformat(),
                "summary": event.summary,
                "location": event.location,
                "uid": event.uid,
                "description": event.description,
            }
            for event in filtered_events
        ],
    }
    json_kwargs = (
        {"indent": 2, "ensure_ascii": False}
        if pretty_json
        else {"separators": (",", ":")}
    )
    json_path.write_text(json.dumps(summary, **json_kwargs) + "\n", encoding="utf-8")

    return CalendarExportResult(
        iso_year=iso_year,
        iso_week=iso_week,
        calendar_name=calendar_name,
        ics_path=ics_path,
        json_path=json_path,
        events=filtered_events,
    )


def determine_iso_week(week_arg: str | None) -> tuple[int, int]:
    if week_arg:
        try:
            year_part, week_part = week_arg.split("-W")
            year = int(year_part)
            week = int(week_part)
        except ValueError as exc:
            raise SystemExit("--week must look like YYYY-Www") from exc
        return year, week
    now = dt.datetime.now(tz=UTC)
    iso_year, iso_week, _ = now.isocalendar()
    return int(iso_year), int(iso_week)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    iso_year, iso_week = determine_iso_week(args.week)
    try:
        ics_text = load_ics_text(args.ics_path, args.ics_url)
    except Exception as exc:  # pragma: no cover - plumbing guard
        print(f"Failed to load calendar: {exc}", file=sys.stderr)
        return 1
    result = export_calendar_week(
        ics_text=ics_text,
        iso_year=iso_year,
        iso_week=iso_week,
        out_dir=args.out_dir,
        json_summary_path=args.json_summary,
        calendar_name=args.calendar_name,
        pretty_json=args.pretty_json,
    )
    print(
        f"Exported {len(result.events)} events to {result.ics_path} "
        f"(week {result.iso_year}-W{result.iso_week:02d})."
    )
    if result.json_path:
        print(f"Wrote summary JSON to {result.json_path}.")
    for event in result.events:
        summary = event.summary or event.uid or "(no summary)"
        print(f"- {event.start.isoformat()} → {event.end.isoformat()} :: {summary}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
