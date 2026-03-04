#!/usr/bin/env python3
"""
Validate `android.telemetry.redaction.override` NDJSON feeds and ledger parity.

Roadmap item AND7 requires override apply/revoke actions to emit sanitised
events (hashed tokens, masked actor roles) alongside the Markdown audit log.
This helper checks the NDJSON stream for policy violations, cross-references
override ids against the Markdown ledger, and emits optional JSON/Markdown
summaries for governance packets.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List, Sequence

EXPECTED_SIGNAL = "android.telemetry.redaction.override"
ALLOWED_ACTOR_ROLES = {
    "support",
    "sre",
    "docs",
    "compliance",
    "program",
    "other",
}
AUDIT_HEADER_PREFIX = "| Ticket ID"


class ValidationError(RuntimeError):
    """Domain error for invalid override events or ledgers."""


@dataclasses.dataclass(frozen=True)
class Event:
    """Normalised override event entry."""

    action: str
    override_id: str
    actor_role: str
    ticket_id: str
    issued_at: str | None
    expires_at: str | None
    revoked_at: str | None
    logged_at: str | None
    approver: str | None
    reason: str | None


@dataclasses.dataclass(frozen=True)
class Summary:
    """Aggregate results for an override event file."""

    events: List[Event]
    invalid: List[str]
    missing_in_ledger: List[str]
    unknown_actions: List[str]
    ledger_rows: int
    active_overrides: int


def _resolve_path_from_manifest(
    manifest: dict,
    label: str,
    manifest_dir: Path,
    default: Path,
    required: bool = True,
) -> Path:
    """Resolve an asset path from the bundle manifest."""
    assets = manifest.get("assets") or []
    for asset in assets:
        if asset.get("label") != label:
            continue
        path_str = asset.get("bundled_path") or asset.get("source")
        if not path_str:
            break
        candidate = Path(path_str)
        if not candidate.is_absolute():
            candidate = (manifest_dir / candidate).resolve()
        return candidate
    if required:
        raise ValidationError(f"bundle manifest missing label {label!r}")
    return default


def _parse_timestamp(value: str) -> str:
    """Return a canonicalised ISO-8601 timestamp (preserves timezone)."""
    raw = value.strip()
    if raw.endswith("Z"):
        raw = raw[:-1] + "+00:00"
    try:
        ts = datetime.fromisoformat(raw)
    except ValueError as exc:
        raise ValidationError(f"invalid timestamp: {value!r}") from exc
    if ts.tzinfo is None:
        raise ValidationError("timestamp must include a timezone offset")
    return ts.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _is_hex_digest(value: str) -> bool:
    return len(value) in (32, 64) and all(ch in "0123456789abcdef" for ch in value.lower())


def load_events(path: Path) -> List[dict]:
    """Load NDJSON events from disk."""
    if not path.is_file():
        raise ValidationError(f"event log not found: {path}")
    events: List[dict] = []
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            events.append(json.loads(stripped))
        except json.JSONDecodeError as exc:
            raise ValidationError(f"invalid JSON on line {lineno}: {exc}") from exc
    return events


def _parse_ledger(path: Path) -> tuple[set[str], int]:
    """Parse the Markdown audit log and return (token_hashes, row_count)."""
    if not path.is_file():
        raise ValidationError(f"audit log not found: {path}")
    lines = path.read_text(encoding="utf-8").splitlines()

    start = -1
    for idx, line in enumerate(lines):
        if line.strip().startswith(AUDIT_HEADER_PREFIX):
            start = idx
            break
    if start == -1:
        raise ValidationError("audit log is missing the override table header")

    token_hashes: set[str] = set()
    row_count = 0
    for line in lines[start + 2 :]:
        stripped = line.strip()
        if not stripped.startswith("|"):
            break
        if stripped.startswith("|-----------"):
            continue
        segments = [segment.strip().replace("\\|", "|") for segment in stripped.strip("|").split("|")]
        if len(segments) < 4:
            continue
        token_hashes.add(segments[3])
        row_count += 1
    return token_hashes, row_count


def _normalise_event(raw: dict, line_no: int) -> Event:
    def expect(field: str) -> str:
        if field not in raw or not str(raw[field]).strip():
            raise ValidationError(f"line {line_no}: missing required field {field!r}")
        return str(raw[field]).strip()

    if raw.get("signal") not in {None, EXPECTED_SIGNAL}:
        raise ValidationError(
            f"line {line_no}: unexpected signal id {raw.get('signal')!r} (expected {EXPECTED_SIGNAL})"
        )

    action = expect("action").lower()
    override_id = expect("override_id")

    for forbidden in ("token", "raw_token"):
        if forbidden in raw:
            raise ValidationError(f"line {line_no}: found forbidden field {forbidden!r}")

    if not _is_hex_digest(override_id):
        raise ValidationError(f"line {line_no}: override_id must be a 32 or 64 char hex digest")

    actor_role = expect("actor_role_masked").lower()
    if actor_role not in ALLOWED_ACTOR_ROLES:
        raise ValidationError(
            f"line {line_no}: actor_role_masked must be one of {sorted(ALLOWED_ACTOR_ROLES)}"
        )

    ticket_id = expect("ticket_id")

    issued_at = expires_at = revoked_at = logged_at = approver = reason = None

    if "logged_at" in raw and str(raw["logged_at"]).strip():
        logged_at = _parse_timestamp(str(raw["logged_at"]))

    if action == "apply":
        issued_at = _parse_timestamp(expect("issued_at"))
        expires_at = _parse_timestamp(expect("expires_at"))
        approver = expect("approver")
        reason = expect("reason")
    elif action == "revoke":
        revoked_at = _parse_timestamp(expect("revoked_at"))
        expires_at = _parse_timestamp(expect("expires_at"))
    else:
        raise ValidationError(f"line {line_no}: unsupported action {action!r}")

    return Event(
        action=action,
        override_id=override_id,
        actor_role=actor_role,
        ticket_id=ticket_id,
        issued_at=issued_at,
        expires_at=expires_at,
        revoked_at=revoked_at,
        logged_at=logged_at,
        approver=approver,
        reason=reason,
    )


def build_summary(
    raw_events: Iterable[dict],
    ledger_path: Path | None,
    skip_ledger: bool,
) -> Summary:
    events: List[Event] = []
    invalid: List[str] = []
    for idx, raw in enumerate(raw_events, start=1):
        try:
            events.append(_normalise_event(raw, idx))
        except ValidationError as exc:
            invalid.append(str(exc))

    ledger_hashes: set[str] = set()
    ledger_rows = 0
    if not skip_ledger:
        ledger_hashes, ledger_rows = _parse_ledger(ledger_path or Path())

    missing_in_ledger = [event.override_id for event in events if ledger_hashes and event.override_id not in ledger_hashes]
    unknown_actions: List[str] = []
    active: set[str] = set()
    for event in events:
        if event.action == "apply":
            active.add(event.override_id)
        elif event.action == "revoke":
            active.discard(event.override_id)
        else:
            unknown_actions.append(event.action)

    return Summary(
        events=events,
        invalid=invalid,
        missing_in_ledger=missing_in_ledger,
        unknown_actions=unknown_actions,
        ledger_rows=ledger_rows,
        active_overrides=len(active),
    )


def summary_to_json(summary: Summary) -> dict:
    return {
        "event_count": len(summary.events),
        "apply_events": sum(1 for event in summary.events if event.action == "apply"),
        "revoke_events": sum(1 for event in summary.events if event.action == "revoke"),
        "active_overrides": summary.active_overrides,
        "ledger_rows": summary.ledger_rows,
        "missing_in_ledger": summary.missing_in_ledger,
        "invalid_events": summary.invalid,
    }


def render_markdown(summary: Summary) -> str:
    lines = [
        "# Override Event Validation",
        "",
        f"- Total events: **{len(summary.events)}**",
        f"- Apply events: **{sum(1 for event in summary.events if event.action == 'apply')}**",
        f"- Revoke events: **{sum(1 for event in summary.events if event.action == 'revoke')}**",
        f"- Active overrides (from events): **{summary.active_overrides}**",
        f"- Ledger rows: **{summary.ledger_rows}**",
    ]
    if summary.missing_in_ledger:
        lines.append(f"- Missing in ledger: **{len(summary.missing_in_ledger)}**")
    if summary.invalid:
        lines.append(f"- Invalid events: **{len(summary.invalid)}**")
    lines.append("")
    if summary.missing_in_ledger:
        lines.append("## Missing override ids")
        lines.append("")
        for oid in summary.missing_in_ledger:
            lines.append(f"- `{oid}`")
        lines.append("")
    if summary.invalid:
        lines.append("## Invalid events")
        lines.append("")
        for message in summary.invalid:
            lines.append(f"- {message}")
        lines.append("")
    return "\n".join(lines)


def _print_summary(summary: Summary) -> int:
    print(f"Events: {len(summary.events)} (apply={sum(1 for e in summary.events if e.action == 'apply')}, revoke={sum(1 for e in summary.events if e.action == 'revoke')})")
    print(f"Active overrides (events): {summary.active_overrides}")
    print(f"Ledger rows: {summary.ledger_rows}")
    if summary.missing_in_ledger:
        print(f"ERROR missing in ledger ({len(summary.missing_in_ledger)}): {', '.join(summary.missing_in_ledger)}")
    if summary.invalid:
        print(f"ERROR invalid events ({len(summary.invalid)}):")
        for message in summary.invalid:
            print(f"  - {message}")

    return 2 if summary.invalid or summary.missing_in_ledger else 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate android.telemetry.redaction.override NDJSON feeds"
    )
    default_events = Path("docs/source/sdk/android/readiness/override_logs/override_events.ndjson")
    default_ledger = Path("docs/source/sdk/android/telemetry_override_log.md")
    parser.add_argument(
        "--events",
        type=Path,
        default=default_events,
        help="Path to the NDJSON log (default: %(default)s)",
    )
    parser.add_argument(
        "--ledger",
        type=Path,
        default=default_ledger,
        help="Path to the Markdown audit log (default: %(default)s)",
    )
    parser.add_argument(
        "--skip-ledger",
        action="store_true",
        help="Skip ledger cross-checks (structural validation only)",
    )
    parser.add_argument(
        "--bundle-manifest",
        type=Path,
        help="Optional AND7 enablement bundle manifest to resolve events/ledger paths",
    )
    parser.add_argument(
        "--events-label",
        default="override-events",
        help="Label to use when resolving events from the bundle manifest (default: %(default)s)",
    )
    parser.add_argument(
        "--ledger-label",
        default="override-ledger",
        help="Label to use when resolving ledger from the bundle manifest (default: %(default)s)",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path to write a JSON summary",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Optional path to write a Markdown summary",
    )
    args = parser.parse_args(argv)

    events_path = args.events
    ledger_path = args.ledger
    if args.bundle_manifest:
        if not args.bundle_manifest.is_file():
            print(f"error: bundle manifest not found: {args.bundle_manifest}")
            return 2
        manifest = json.loads(args.bundle_manifest.read_text(encoding="utf-8"))
        manifest_dir = args.bundle_manifest.parent
        events_path = _resolve_path_from_manifest(
            manifest,
            label=args.events_label,
            manifest_dir=manifest_dir,
            default=events_path,
        )
        if not args.skip_ledger:
            ledger_path = _resolve_path_from_manifest(
                manifest,
                label=args.ledger_label,
                manifest_dir=manifest_dir,
                default=ledger_path,
            )

    try:
        raw_events = load_events(events_path)
        summary = build_summary(raw_events, ledger_path=ledger_path, skip_ledger=args.skip_ledger)
    except ValidationError as exc:
        print(f"error: {exc}")
        return 2

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(summary_to_json(summary), indent=2), encoding="utf-8")

    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(render_markdown(summary), encoding="utf-8")

    return _print_summary(summary)


if __name__ == "__main__":
    raise SystemExit(main())
