#!/usr/bin/env python3
"""Verify Swift Norito fixture parity and cadence metadata."""

from __future__ import annotations

import argparse
import json
import hashlib
import sys
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Iterable, List, Optional, Sequence, Tuple

DEFAULT_SOURCE = Path("java/iroha_android/src/test/resources")
DEFAULT_TARGET = Path("IrohaSwift/Fixtures")
DEFAULT_STATE = Path("artifacts/swift_fixture_regen_state.json")
DEFAULT_ODD_OWNER = "android-foundations"
DEFAULT_EVEN_OWNER = "swift-lead"
DEFAULT_CADENCE_LABEL = "weekly-wed-1700utc"


def collect_files(root: Path) -> List[Path]:
    if not root.exists():
        raise FileNotFoundError(f"missing directory: {root}")
    files = [p for p in root.rglob("*") if p.is_file()]
    files.sort()
    return files


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def compare(source: Path, target: Path) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    source_map = {p.relative_to(source): p for p in collect_files(source)}
    target_map = {p.relative_to(target): p for p in collect_files(target)}

    missing = sorted(rel for rel in source_map if rel not in target_map)
    extra = sorted(rel for rel in target_map if rel not in source_map)

    diffs: List[Tuple[Path, Path]] = []
    for rel, src_path in source_map.items():
        tgt_path = target_map.get(rel)
        if tgt_path is None:
            continue
        if fingerprint(src_path) != fingerprint(tgt_path):
            diffs.append((src_path, tgt_path))
    return missing, extra, diffs


@dataclass(frozen=True)
class RotationRoster:
    odd_weeks: str = DEFAULT_ODD_OWNER
    even_weeks: str = DEFAULT_EVEN_OWNER

    def expected_owner(self, iso_week: int) -> str:
        return self.odd_weeks if iso_week % 2 else self.even_weeks


@dataclass(frozen=True)
class StateInfo:
    rotation_owner: str
    auto_owner: str
    owner_source: str
    trigger: str
    cadence_label: str
    slot_start: datetime
    next_slot: datetime
    generated_at: datetime
    age_hours: float


def isoformat_utc(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_timestamp(value: str) -> datetime:
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as exc:  # pragma: no cover - defensive
        raise RuntimeError(f"invalid timestamp '{value}': {exc}") from exc


def scheduled_slot(now: datetime, *, label: str = DEFAULT_CADENCE_LABEL, interval_hours: float = 48.0) -> datetime:
    """Return the most recent cadence slot at or before `now`."""
    normalized = (label or DEFAULT_CADENCE_LABEL).strip().lower() or DEFAULT_CADENCE_LABEL
    if normalized == "rolling-48h":
        return rolling_slot(now, interval_hours=interval_hours or 48.0)
    if normalized == "fallback-mon-thu-utc":
        return fallback_slot(now)
    return weekly_slot(now, weekday=3, hour=17)


def weekly_slot(now: datetime, *, weekday: int, hour: int, minute: int = 0) -> datetime:
    now_utc = now.astimezone(timezone.utc)
    iso_year, iso_week, _ = now_utc.isocalendar()
    monday = datetime.strptime(f"{iso_year} {iso_week} 1", "%G %V %u").replace(tzinfo=timezone.utc)
    slot = monday + timedelta(days=weekday - 1, hours=hour, minutes=minute)
    while slot > now_utc:
        slot -= timedelta(days=7)
    return slot


def fallback_slot(now: datetime) -> datetime:
    monday = weekly_slot(now, weekday=1, hour=17)
    thursday = weekly_slot(now, weekday=4, hour=17)
    return max(monday, thursday)


def rolling_slot(now: datetime, *, interval_hours: float) -> datetime:
    anchor = datetime(2026, 1, 1, tzinfo=timezone.utc)
    seconds = interval_hours * 3600.0
    if seconds <= 0:
        return now
    offset = (now - anchor).total_seconds()
    steps = int(offset // seconds)
    return anchor + timedelta(seconds=steps * seconds)


def next_slot(slot: datetime, *, label: str, interval_hours: float) -> datetime:
    if label == "rolling-48h":
        return slot + timedelta(hours=interval_hours or 48.0)
    if label == "fallback-mon-thu-utc":
        if slot.weekday() == 0:
            return slot + timedelta(days=3)
        return slot + timedelta(days=4)
    return slot + timedelta(days=7)


def load_state(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"cadence state file missing at {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse cadence state JSON: {exc}") from exc
    return data


def validate_state(
    state_path: Path,
    roster: RotationRoster,
    *,
    max_age_hours: float,
    schedule_tolerance_hours: float,
    allowed_cadence_labels: Optional[Sequence[str]] = None,
    now: Optional[datetime] = None,
) -> StateInfo:
    state = load_state(state_path)

    generated_at_raw = state.get("generated_at")
    if not isinstance(generated_at_raw, str):
        raise RuntimeError("cadence state missing 'generated_at' field")
    generated_at = parse_timestamp(generated_at_raw)

    current = now or datetime.now(timezone.utc)
    age_hours = (current - generated_at).total_seconds() / 3600.0
    if age_hours > max_age_hours:
        raise RuntimeError(f"fixtures are {age_hours:.1f}h old (limit {max_age_hours:.1f}h)")

    trigger = str(state.get("trigger", "scheduled")).lower()
    if trigger not in {"scheduled", "event"}:
        raise RuntimeError(f"unexpected trigger value '{trigger}' in cadence state")

    raw_label = state.get("cadence")
    cadence_label = (raw_label or DEFAULT_CADENCE_LABEL).strip().lower() or DEFAULT_CADENCE_LABEL
    allowed = [lbl.strip().lower() for lbl in allowed_cadence_labels or [] if lbl.strip()]
    if allowed and cadence_label not in allowed:
        raise RuntimeError(
            f"cadence label '{cadence_label}' is not in the allowed set {allowed}"
        )

    interval_hours = float(state.get("cadence_interval_hours", 48.0))

    window = state.get("window", {}) or {}
    slot_start_raw = window.get("slot_start")
    slot_start = (
        parse_timestamp(slot_start_raw)
        if isinstance(slot_start_raw, str)
        else scheduled_slot(generated_at, label=cadence_label, interval_hours=interval_hours)
    )
    next_slot_raw = window.get("next_slot")
    next_slot_value = (
        parse_timestamp(next_slot_raw)
        if isinstance(next_slot_raw, str)
        else next_slot(slot_start, label=cadence_label, interval_hours=interval_hours)
    )
    window_hours = float(state.get("cadence_window_hours", schedule_tolerance_hours))
    slot_end = (
        parse_timestamp(window.get("slot_end"))
        if isinstance(window.get("slot_end"), str)
        else slot_start + timedelta(hours=window_hours)
    )

    iso_week = int(window.get("iso_week", slot_start.isocalendar()[1]))
    expected_owner = roster.expected_owner(iso_week)

    rotation_owner = str(state.get("rotation_owner", "")).strip()
    if not rotation_owner:
        raise RuntimeError("cadence state missing 'rotation_owner'")
    auto_owner = str(state.get("rotation_owner_auto", expected_owner)).strip() or expected_owner
    owner_source = str(state.get("rotation_owner_source", "auto")).strip().lower() or "auto"

    if auto_owner.lower() != expected_owner.lower():
        raise RuntimeError(
            f"auto rotation owner mismatch (expected '{expected_owner}', got '{auto_owner}')"
        )

    if owner_source == "auto" and rotation_owner.lower() != expected_owner.lower():
        raise RuntimeError(
            f"rotation owner mismatch (expected '{expected_owner}', got '{rotation_owner}')"
        )

    tolerance = timedelta(hours=schedule_tolerance_hours)
    if trigger == "scheduled":
        if generated_at < slot_start - tolerance:
            raise RuntimeError(
                f"fixtures regenerated before scheduled window (slot {isoformat_utc(slot_start)}, generated {isoformat_utc(generated_at)})"
            )
        if generated_at > slot_end + tolerance:
            raise RuntimeError(
                f"fixtures regenerated after scheduled window (slot {isoformat_utc(slot_start)}, generated {isoformat_utc(generated_at)})"
        )

    return StateInfo(
        rotation_owner=rotation_owner,
        auto_owner=auto_owner,
        owner_source=owner_source,
        trigger=trigger,
        cadence_label=cadence_label,
        slot_start=slot_start,
        next_slot=next_slot_value,
        generated_at=generated_at,
        age_hours=age_hours,
    )


def format_state_summary(info: StateInfo, max_age_hours: float) -> str:
    return (
        "[swift-fixtures] cadence ok: "
        f"age={info.age_hours:.1f}h (limit {max_age_hours:.1f}h) "
        f"owner={info.rotation_owner} (auto={info.auto_owner}, source={info.owner_source}) "
        f"trigger={info.trigger} cadence={info.cadence_label} "
        f"slot_start={isoformat_utc(info.slot_start)} "
        f"next_slot={isoformat_utc(info.next_slot)}"
    )


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check Swift Norito fixture parity and cadence metadata")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE,
                        help=f"Canonical fixture directory (default: {DEFAULT_SOURCE})")
    parser.add_argument("--target", type=Path, default=DEFAULT_TARGET,
                        help=f"Swift fixture directory (default: {DEFAULT_TARGET})")
    parser.add_argument("--quiet", action="store_true", help="Suppress success output")
    parser.add_argument("--state", action="store_true",
                        help=f"Validate cadence state metadata using {DEFAULT_STATE}")
    parser.add_argument("--state-file", type=Path,
                        help="Path to cadence state JSON (implies --state)")
    parser.add_argument("--max-age-hours", type=float, default=48.0,
                        help="Maximum allowed fixture age when validating cadence state (default: 48)")
    parser.add_argument("--odd-week-owner", default=DEFAULT_ODD_OWNER,
                        help=f"Expected rotation owner for odd ISO weeks (default: {DEFAULT_ODD_OWNER})")
    parser.add_argument("--even-week-owner", default=DEFAULT_EVEN_OWNER,
                        help=f"Expected rotation owner for even ISO weeks (default: {DEFAULT_EVEN_OWNER})")
    parser.add_argument("--schedule-tolerance-hours", type=float, default=6.0,
                        help="Allowed lead/lag (hours) around the scheduled slot when --state is used (default: 6)")
    parser.add_argument(
        "--cadence-label",
        action="append",
        dest="cadence_labels",
        help="Allowed cadence label (default: weekly-wed-1700utc). Pass multiple times to allow more than one label.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        missing, extra, diffs = compare(args.source, args.target)
    except FileNotFoundError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    has_error = False
    if missing:
        has_error = True
        print("[error] missing files in target:")
        for rel in missing:
            print(f"    {rel}")
    if extra:
        has_error = True
        print("[error] unexpected files in target:")
        for rel in extra:
            print(f"    {rel}")
    if diffs:
        has_error = True
        print("[error] content mismatches:")
        for src, tgt in diffs:
            rel = tgt.relative_to(args.target)
            print(f"    {rel} (source={src}, target={tgt})")

    if has_error:
        return 1

    if not args.quiet:
        print(f"[ok] Fixtures match between {args.source} and {args.target}")

    state_path: Optional[Path] = args.state_file
    if args.state:
        state_path = state_path or DEFAULT_STATE

    if state_path is not None:
        roster = RotationRoster(args.odd_week_owner, args.even_week_owner)
        allowed_labels = args.cadence_labels or [DEFAULT_CADENCE_LABEL]
        try:
            info = validate_state(
                state_path,
                roster,
                max_age_hours=args.max_age_hours,
                schedule_tolerance_hours=args.schedule_tolerance_hours,
                allowed_cadence_labels=allowed_labels,
            )
        except RuntimeError as exc:
            print(f"[error] {exc}", file=sys.stderr)
            return 1
        print(format_state_summary(info, args.max_age_hours))
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
