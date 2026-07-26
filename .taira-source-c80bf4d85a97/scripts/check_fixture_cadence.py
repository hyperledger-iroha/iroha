#!/usr/bin/env python3
"""Validate cross-SDK Norito fixture regeneration cadence."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional


DEFAULT_STATE_PATHS: Dict[str, Path] = {
    "android": Path("artifacts/android_fixture_regen_state.json"),
    "python": Path("artifacts/python_fixture_regen_state.json"),
    "swift": Path("artifacts/swift_fixture_regen_state.json"),
    "js": Path("artifacts/js_fixture_regen_state.json"),
}

DEFAULT_PLATFORMS: List[str] = ["android", "python"]


@dataclass(frozen=True)
class PlatformState:
    platform: str
    path: Path
    generated_at: datetime
    rotation_owner: Optional[str]
    cadence: Optional[str]


@dataclass(frozen=True)
class PlatformReport:
    data: PlatformState
    age_hours: float

    @property
    def platform(self) -> str:
        return self.data.platform

    @property
    def generated_at(self) -> datetime:
        return self.data.generated_at


def parse_timestamp(value: str) -> datetime:
    """Parse ISO-8601 timestamp strings (supports trailing 'Z')."""
    if not value:
        raise ValueError("missing timestamp")
    if value.endswith("Z"):
        value = f"{value[:-1]}+00:00"
    dt = datetime.fromisoformat(value)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def isoformat_utc(value: datetime) -> str:
    return value.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def load_platform_state(platform: str, path: Path) -> PlatformState:
    if not path.exists():
        raise RuntimeError(f"[{platform}] cadence state file missing: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - json lib already tested
        raise RuntimeError(f"[{platform}] failed to decode cadence state {path}: {exc}") from exc
    generated_value = payload.get("generated_at")
    if not isinstance(generated_value, str):
        raise RuntimeError(f"[{platform}] cadence state missing generated_at: {path}")
    generated_at = parse_timestamp(generated_value)
    rotation_owner = payload.get("rotation_owner")
    cadence = payload.get("cadence")
    return PlatformState(
        platform=platform,
        path=path,
        generated_at=generated_at,
        rotation_owner=rotation_owner,
        cadence=cadence,
    )


def build_reports(
    states: Iterable[PlatformState],
    *,
    now: datetime,
    max_age_hours: float,
) -> List[PlatformReport]:
    reports: List[PlatformReport] = []
    for state in states:
        age = (now - state.generated_at).total_seconds() / 3600.0
        reports.append(PlatformReport(state, age))
        if age > max_age_hours:
            raise RuntimeError(
                f"[{state.platform}] fixtures are {age:.1f}h old (limit {max_age_hours}h)"
            )
    return reports


def compute_skew_hours(reports: List[PlatformReport]) -> Optional[float]:
    if len(reports) < 2:
        return None
    oldest = min(report.generated_at for report in reports)
    newest = max(report.generated_at for report in reports)
    return (newest - oldest).total_seconds() / 3600.0


def render_summary(
    reports: List[PlatformReport],
    *,
    max_age_hours: float,
    max_skew_hours: float,
    skew_hours: Optional[float],
) -> List[str]:
    lines: List[str] = []
    for report in reports:
        owner = report.data.rotation_owner or "n/a"
        cadence = report.data.cadence or "n/a"
        lines.append(
            "[ok] {platform} fixtures generated_at={generated} age={age:.2f}h "
            "cadence={cadence} owner={owner}".format(
                platform=report.platform,
                generated=isoformat_utc(report.generated_at),
                age=report.age_hours,
                cadence=cadence,
                owner=owner,
            )
        )
    if skew_hours is not None:
        lines.append(
            f"[ok] cross-SDK skew={skew_hours:.2f}h (limit {max_skew_hours}h, "
            f"max age {max_age_hours}h)"
        )
    else:
        lines.append(f"[info] only one platform checked (max age {max_age_hours}h enforced)")
    return lines


def build_json_report(
    reports: List[PlatformReport],
    *,
    checked_at: datetime,
    max_age_hours: float,
    max_skew_hours: float,
    skew_hours: Optional[float],
    status: str,
    errors: List[str],
) -> Dict[str, object]:
    return {
        "checked_at": isoformat_utc(checked_at),
        "status": status,
        "max_age_hours": max_age_hours,
        "max_skew_hours": max_skew_hours,
        "skew_hours": skew_hours,
        "platforms": [
            {
                "platform": report.platform,
                "path": str(report.data.path),
                "generated_at": isoformat_utc(report.generated_at),
                "age_hours": report.age_hours,
                "rotation_owner": report.data.rotation_owner,
                "cadence": report.data.cadence,
            }
            for report in reports
        ],
        "errors": errors,
    }


def render_markdown_report(
    reports: List[PlatformReport],
    *,
    checked_at: datetime,
    max_age_hours: float,
    max_skew_hours: float,
    skew_hours: Optional[float],
    status: str,
    errors: List[str],
) -> str:
    lines = [
        "# Norito Fixture Cadence Summary",
        "",
        f"- Checked at: {isoformat_utc(checked_at)}",
        f"- Status: {status}",
        f"- Maximum fixture age: {max_age_hours:.1f}h",
        f"- Maximum cross-SDK skew: {max_skew_hours:.1f}h",
    ]
    if skew_hours is not None:
        lines.append(f"- Observed skew: {skew_hours:.2f}h")
    else:
        lines.append("- Observed skew: n/a (single platform sample)")
    lines.append("")

    if reports:
        lines.append(
            "| Platform | Generated At (UTC) | Age (h) | Cadence | Rotation Owner | State File |"
        )
        lines.append("| --- | --- | ---: | --- | --- | --- |")
        for report in reports:
            lines.append(
                "| {platform} | {generated} | {age:.2f} | {cadence} | {owner} | `{path}` |".format(
                    platform=report.platform,
                    generated=isoformat_utc(report.generated_at),
                    age=report.age_hours,
                    cadence=report.data.cadence or "n/a",
                    owner=report.data.rotation_owner or "n/a",
                    path=report.data.path,
                )
            )
    else:
        lines.append("_No platform cadence state files were evaluated._")

    if errors:
        lines.append("")
        lines.append("## Errors")
        lines.extend(f"- {err}" for err in errors)

    return "\n".join(lines) + "\n"


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Norito fixture regeneration cadence across SDKs."
    )
    parser.add_argument(
        "--platform",
        action="append",
        choices=sorted(DEFAULT_STATE_PATHS.keys()),
        help=(
            "Platform to check (default: android+python). "
            "Repeat to include multiple entries."
        ),
    )
    parser.add_argument(
        "--android-state",
        type=Path,
        default=DEFAULT_STATE_PATHS["android"],
        help="Path to Android cadence state JSON.",
    )
    parser.add_argument(
        "--python-state",
        type=Path,
        default=DEFAULT_STATE_PATHS["python"],
        help="Path to Python cadence state JSON.",
    )
    parser.add_argument(
        "--swift-state",
        type=Path,
        default=DEFAULT_STATE_PATHS["swift"],
        help="Path to Swift cadence state JSON.",
    )
    parser.add_argument(
        "--js-state",
        type=Path,
        default=DEFAULT_STATE_PATHS["js"],
        help="Path to JavaScript cadence state JSON.",
    )
    parser.add_argument(
        "--max-age-hours",
        type=float,
        default=72.0,
        help="Maximum allowed age for any fixture set (hours).",
    )
    parser.add_argument(
        "--max-skew-hours",
        type=float,
        default=6.0,
        help="Maximum allowed difference between freshest and stalest fixture sets.",
    )
    parser.add_argument(
        "--now",
        type=str,
        help="Override the timestamp used for calculations (ISO-8601).",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path to write a JSON report for dashboards/governance packets.",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Optional path to write a Markdown summary for governance/pre-read packets.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    platforms = args.platform or list(DEFAULT_PLATFORMS)
    now = parse_timestamp(args.now) if args.now else datetime.now(tz=timezone.utc)

    platform_paths = {
        "android": args.android_state,
        "python": args.python_state,
        "swift": args.swift_state,
        "js": args.js_state,
    }

    try:
        states = [
            load_platform_state(platform, platform_paths[platform])
            for platform in platforms
        ]
        reports = build_reports(states, now=now, max_age_hours=args.max_age_hours)
        skew_hours = compute_skew_hours(reports)
        if skew_hours is not None and skew_hours > args.max_skew_hours:
            raise RuntimeError(
                f"cross-SDK skew {skew_hours:.2f}h exceeds {args.max_skew_hours}h limit"
            )
        status = "ok"
        errors: List[str] = []
        lines = render_summary(
            reports,
            max_age_hours=args.max_age_hours,
            max_skew_hours=args.max_skew_hours,
            skew_hours=skew_hours,
        )
    except RuntimeError as exc:
        status = "error"
        errors = [str(exc)]
        reports = []
        skew_hours = None
        lines = []

    for line in lines:
        print(line)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(
            json.dumps(
                build_json_report(
                    reports,
                    checked_at=now,
                    max_age_hours=args.max_age_hours,
                    max_skew_hours=args.max_skew_hours,
                    skew_hours=skew_hours,
                    status=status,
                    errors=errors,
                ),
                indent=2,
                sort_keys=True,
            ),
            encoding="utf-8",
        )

    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(
            render_markdown_report(
                reports,
                checked_at=now,
                max_age_hours=args.max_age_hours,
                max_skew_hours=args.max_skew_hours,
                skew_hours=skew_hours,
                status=status,
                errors=errors,
            ),
            encoding="utf-8",
        )

    if errors:
        for err in errors:
            print(f"[error] {err}")
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
