#!/usr/bin/env python3
"""Validates that all red-team reports have placeholders replaced."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Iterable, Sequence


DEFAULT_TEMPLATE = Path("docs/source/ministry/reports/moderation_red_team_template.md")
DEFAULT_REPORTS_DIR = Path("docs/source/ministry/reports")
PLACEHOLDER_PATTERN = re.compile(r"<[^>]+>")


def collect_placeholders(template_path: Path) -> Sequence[str]:
    """Return placeholder tokens from the template."""

    text = template_path.read_text(encoding="utf-8")
    tokens = sorted(set(PLACEHOLDER_PATTERN.findall(text)))
    return tokens


def iter_report_files(reports_dir: Path) -> Iterable[Path]:
    """Yield red-team report files (excluding the template)."""

    if not reports_dir.exists():
        return []
    for path in sorted(reports_dir.glob("*mod-red-team*.md")):
        if path.name == DEFAULT_TEMPLATE.name:
            continue
        yield path


def validate_reports(
    template_path: Path = DEFAULT_TEMPLATE,
    reports_dir: Path = DEFAULT_REPORTS_DIR,
) -> list[str]:
    """Validate all report files and return error strings."""

    placeholders = collect_placeholders(template_path)
    errors: list[str] = []
    for report_path in iter_report_files(reports_dir):
        content = report_path.read_text(encoding="utf-8")
        missing = [token for token in placeholders if token in content]
        if missing:
            formatted = ", ".join(missing)
            errors.append(
                f"{report_path}: template placeholders still present ({formatted})"
            )
    return errors


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--template",
        type=Path,
        default=DEFAULT_TEMPLATE,
        help=f"Template path (default: {DEFAULT_TEMPLATE}).",
    )
    parser.add_argument(
        "--reports-dir",
        type=Path,
        default=DEFAULT_REPORTS_DIR,
        help=f"Reports directory (default: {DEFAULT_REPORTS_DIR}).",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    errors = validate_reports(
        template_path=args.template,
        reports_dir=args.reports_dir,
    )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("Red-team report check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
