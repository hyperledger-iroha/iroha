#!/usr/bin/env python3
"""Summarise partner SLA discovery sessions for AND8."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Sequence

DEFAULT_SESSIONS_ROOT = Path("docs/source/sdk/android/partner_sla_sessions")
DEFAULT_RELEASE_CALENDAR = os.environ.get(
    "ANDROID_RELEASE_CALENDAR_PATH",
    "docs/source/sdk/android/readiness/archive/ga-2027-10/release_calendar_ga-2027-10.md",
)
DEFAULT_RELEASE_ACK_LOG = os.environ.get(
    "ANDROID_RELEASE_ACK_LOG_PATH",
    "docs/source/sdk/android/readiness/archive/ga-2027-10/ack_log_ga-2027-10.md",
)


def _env_flag(name: str, default: str) -> str:
    return os.environ.get(name, default)


class SLAOverride:
    """Description of a negotiated SLA override."""

    __slots__ = ("title", "commitment", "notes")

    def __init__(self, title: str, commitment: str, notes: Optional[str] = None) -> None:
        self.title = title
        self.commitment = commitment
        self.notes = notes

    @classmethod
    def from_json(cls, value: Any) -> "SLAOverride":
        if isinstance(value, str):
            return cls(title=value, commitment="", notes=None)
        if isinstance(value, dict):
            return cls(
                title=str(value.get("class") or value.get("title") or "Override"),
                commitment=str(value.get("commitment") or value.get("value") or ""),
                notes=(value.get("notes") or value.get("description")),
            )
        raise ValueError(f"Unsupported SLA override entry: {value!r}")

    def as_text(self) -> str:
        if self.notes:
            return f"{self.title}: {self.commitment} ({self.notes})".strip()
        return f"{self.title}: {self.commitment}".strip(": ")

    def as_dict(self) -> Dict[str, Any]:
        return {
            "title": self.title,
            "commitment": self.commitment,
            "notes": self.notes,
        }


class ActionItem:
    """Follow-up item captured during a session."""

    __slots__ = ("description", "owner", "due")

    def __init__(
        self,
        description: str,
        owner: Optional[str] = None,
        due: Optional[str] = None,
    ) -> None:
        self.description = description
        self.owner = owner
        self.due = due

    @classmethod
    def from_json(cls, value: Any) -> "ActionItem":
        if isinstance(value, str):
            return cls(description=value)
        if isinstance(value, dict):
            return cls(
                description=str(value.get("description") or value.get("text") or ""),
                owner=value.get("owner"),
                due=value.get("due"),
            )
        raise ValueError(f"Unsupported action item entry: {value!r}")

    def as_dict(self) -> Dict[str, Any]:
        return {
            "description": self.description,
            "owner": self.owner,
            "due": self.due,
        }


class SessionRecord:
    """Flattened session metadata used for reporting."""

    __slots__ = (
        "partner",
        "session_date",
        "acknowledged",
        "acknowledgement_ref",
        "blackout_windows",
        "sla_overrides",
        "action_items",
        "notes",
        "source",
    )

    def __init__(
        self,
        *,
        partner: str,
        session_date: str,
        acknowledged: bool,
        acknowledgement_ref: Optional[str],
        blackout_windows: List[str],
        sla_overrides: List[SLAOverride],
        action_items: List[ActionItem],
        notes: Optional[str],
        source: Path,
    ) -> None:
        self.partner = partner
        self.session_date = session_date
        self.acknowledged = acknowledged
        self.acknowledgement_ref = acknowledgement_ref
        self.blackout_windows = blackout_windows
        self.sla_overrides = sla_overrides
        self.action_items = action_items
        self.notes = notes
        self.source = source

    def to_dict(self) -> Dict[str, Any]:
        return {
            "partner": self.partner,
            "session_date": self.session_date,
            "acknowledged": self.acknowledged,
            "acknowledgement_ref": self.acknowledgement_ref,
            "blackout_windows": self.blackout_windows,
            "sla_overrides": [entry.as_dict() for entry in self.sla_overrides],
            "action_items": [entry.as_dict() for entry in self.action_items],
            "notes": self.notes,
            "source": str(self.source),
        }


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarise partner SLA discovery sessions for AND8."
    )
    parser.add_argument(
        "--sessions-root",
        default=_env_flag("ANDROID_SLA_SESSIONS_ROOT", str(DEFAULT_SESSIONS_ROOT)),
        help="Path containing partner_sla_sessions/<partner>/<date>/session.json",
    )
    parser.add_argument(
        "--partner",
        action="append",
        help="Filter by partner name (may be repeated).",
    )
    parser.add_argument(
        "--format",
        choices=("markdown", "json", "summary"),
        default="markdown",
        help="Output format (default: markdown table).",
    )
    parser.add_argument(
        "--release-calendar",
        default=DEFAULT_RELEASE_CALENDAR,
        help="Path to the release calendar entry for summary output (default: env ANDROID_RELEASE_CALENDAR_PATH).",
    )
    parser.add_argument(
        "--release-ack-log",
        default=DEFAULT_RELEASE_ACK_LOG,
        help="Path to the acknowledgement log mirrored in summary output (default: env ANDROID_RELEASE_ACK_LOG_PATH).",
    )
    parser.add_argument(
        "--out",
        help="Write output to PATH instead of stdout.",
    )
    return parser.parse_args(argv)


def collect_sessions(root: Path, partners: Optional[Iterable[str]] = None) -> List[SessionRecord]:
    partner_filter = {p.lower() for p in partners} if partners else None
    records: List[SessionRecord] = []
    if not root.exists():
        return records
    for partner_dir in sorted(p for p in root.iterdir() if p.is_dir()):
        partner_name = partner_dir.name
        if partner_filter and partner_name.lower() not in partner_filter:
            continue
        for session_dir in sorted(p for p in partner_dir.iterdir() if p.is_dir()):
            meta_path = session_dir / "session.json"
            if not meta_path.exists():
                print(
                    f"[warn] missing session.json for {partner_name} at {session_dir}",
                    file=sys.stderr,
                )
                continue
            with meta_path.open("r", encoding="utf-8") as handle:
                data = json.load(handle)
            try:
                records.append(_record_from_json(data, partner_name, session_dir))
            except Exception as exc:  # pylint: disable=broad-except
                print(
                    f"[warn] failed to parse {meta_path}: {exc}",
                    file=sys.stderr,
                )
    records.sort(key=lambda item: (item.partner.lower(), item.session_date))
    return records


def _record_from_json(data: Dict[str, Any], partner: str, session_dir: Path) -> SessionRecord:
    blackout_windows = [str(value) for value in data.get("blackout_windows", [])]
    overrides = [SLAOverride.from_json(entry) for entry in data.get("sla_overrides", [])]
    actions = [ActionItem.from_json(entry) for entry in data.get("action_items", [])]
    return SessionRecord(
        partner=str(data.get("partner") or partner),
        session_date=str(data.get("date") or session_dir.name),
        acknowledged=bool(data.get("acknowledged", False)),
        acknowledgement_ref=data.get("acknowledgement_ref"),
        blackout_windows=blackout_windows,
        sla_overrides=overrides,
        action_items=actions,
        notes=data.get("notes"),
        source=session_dir,
    )


def render_markdown_table(records: Sequence[SessionRecord]) -> str:
    if not records:
        return "| Partner | Session Date | Ack | Blackouts | SLA Overrides | Action Items | Notes |\n|--------|--------------|-----|-----------|---------------|--------------|-------|\n"
    lines = [
        "| Partner | Session Date | Ack | Blackouts | SLA Overrides | Action Items | Notes |",
        "|--------|--------------|-----|-----------|---------------|--------------|-------|",
    ]
    for record in records:
        ack = "✅" if record.acknowledged else "❌"
        if record.acknowledgement_ref:
            ack = f"{ack} ({record.acknowledgement_ref})"
        blackouts = ", ".join(record.blackout_windows) or "—"
        overrides = "; ".join(filter(None, (entry.as_text() for entry in record.sla_overrides))) or "—"
        actions = "; ".join(
            filter(None, (format_action(entry) for entry in record.action_items))
        ) or "—"
        notes = record.notes.strip() if record.notes else "—"
        lines.append(
            f"| {record.partner} | {record.session_date} | {ack} | {blackouts} | {overrides} | {actions} | {notes} |"
        )
    lines.append("")
    return "\n".join(lines)


def format_action(action: ActionItem) -> str:
    parts: List[str] = [action.description]
    if action.owner:
        parts.append(f"owner: {action.owner}")
    if action.due:
        parts.append(f"due: {action.due}")
    if len(parts) == 1:
        return parts[0]
    return f"{parts[0]} ({', '.join(parts[1:])})"


def render_json(records: Sequence[SessionRecord]) -> str:
    payload = [record.to_dict() for record in records]
    return json.dumps(payload, indent=2) + "\n"


def render_summary(
    records: Sequence[SessionRecord],
    *,
    release_calendar: Optional[str] = None,
    release_ack_log: Optional[str] = None,
) -> str:
    if not records:
        return "No partner SLA sessions recorded yet.\n"

    total_sessions = len(records)
    partners = sorted({record.partner for record in records})
    acknowledged = sorted({record.partner for record in records if record.acknowledged})
    pending = sorted({record.partner for record in records if not record.acknowledged})
    blackout_windows = sorted(
        {window for record in records for window in record.blackout_windows if window}
    )
    open_actions: List[str] = []
    for record in records:
        for action in record.action_items:
            text = format_action(action)
            if text:
                open_actions.append(f"{record.partner}: {text}")

    def format_list(values: Sequence[str]) -> str:
        return ", ".join(values) if values else "—"

    lines = [
        "Partner SLA Summary",
        "===================",
    ]
    if release_calendar:
        lines.append(f"Release calendar packet: {release_calendar}")
    if release_ack_log:
        lines.append(f"Acknowledgement log: {release_ack_log}")
    lines.extend(
        [
            f"Total partners: {len(partners)} ({format_list(partners)})",
            f"Sessions captured: {total_sessions}",
            f"Acknowledged partners: {len(acknowledged)} ({format_list(acknowledged)})",
            f"Pending acknowledgements: {len(pending)} ({format_list(pending)})",
            f"Known blackout windows: {format_list(blackout_windows)}",
        ]
    )
    if open_actions:
        lines.append("Open action items:")
        for entry in open_actions:
            lines.append(f"  - {entry}")
    else:
        lines.append("Open action items: none recorded.")
    lines.append("")
    return "\n".join(lines)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    records = collect_sessions(Path(args.sessions_root), args.partner)
    if args.format == "json":
        output = render_json(records)
    elif args.format == "summary":
        release_calendar = args.release_calendar or None
        release_ack_log = args.release_ack_log or None
        output = render_summary(
            records,
            release_calendar=release_calendar,
            release_ack_log=release_ack_log,
        )
    else:
        output = render_markdown_table(records)
    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
