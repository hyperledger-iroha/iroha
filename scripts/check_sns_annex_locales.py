#!/usr/bin/env python3
"""
Validate that every SNS annex job has the full set of localized report files.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Sequence

DEFAULT_JOBS = "docs/source/sns/regulatory/annex_jobs.json"
DEFAULT_REPORT_ROOT = "docs/source/sns/reports"
DEFAULT_LOCALES = ("en", "ar", "es", "fr", "pt", "ru", "ur")


def load_jobs(path: Path) -> List[dict]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"[error] failed to parse annex jobs JSON: {exc}") from exc
    if not isinstance(raw, list):
        raise SystemExit(f"[error] annex jobs file must contain a JSON array: {path}")
    return raw


def available_locales(raw: str | None) -> List[str]:
    if not raw:
        return list(DEFAULT_LOCALES)
    locales = []
    for token in raw.split(","):
        value = token.strip()
        if value:
            locales.append(value)
    return locales or list(DEFAULT_LOCALES)


def sanitize_suffix(value: str) -> str:
    if not value:
        raise ValueError("suffix is required")
    return value if value.startswith(".") else f".{value}"


def expected_file(report_root: Path, suffix: str, cycle: str, locale: str) -> Path:
    base = report_root / sanitize_suffix(suffix)
    filename = f"{cycle}.md" if locale == "en" else f"{cycle}.{locale}.md"
    return base / filename


@dataclass
class JobCheckResult:
    suffix: str
    cycle: str
    missing: List[Path]

    @property
    def ok(self) -> bool:
        return not self.missing


def check_jobs(
    *,
    report_root: Path,
    jobs: Iterable[dict],
    locales: Sequence[str],
) -> List[JobCheckResult]:
    results: List[JobCheckResult] = []
    for job in jobs:
        suffix = job.get("suffix")
        cycle = job.get("cycle")
        if not isinstance(suffix, str) or not isinstance(cycle, str):
            raise SystemExit(f"[error] job entries must include string suffix/cycle: {job}")
        missing: List[Path] = []
        for locale in locales:
            path = expected_file(report_root, suffix, cycle, locale)
            if not path.exists():
                missing.append(path)
        results.append(JobCheckResult(suffix=suffix, cycle=cycle, missing=missing))
    return results


def summarize_results(results: Sequence[JobCheckResult]) -> dict:
    missing = [
        {"suffix": result.suffix, "cycle": result.cycle, "path": str(path)}
        for result in results
        for path in result.missing
    ]
    return {
        "jobs_checked": len(results),
        "jobs_with_full_coverage": sum(1 for result in results if result.ok),
        "missing_files": missing,
    }


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Ensure SNS annex jobs have the required localized reports."
    )
    parser.add_argument(
        "--jobs",
        default=DEFAULT_JOBS,
        help=f"Path to annex_jobs.json (default: {DEFAULT_JOBS})",
    )
    parser.add_argument(
        "--report-root",
        default=DEFAULT_REPORT_ROOT,
        help=f"Root directory containing localized reports (default: {DEFAULT_REPORT_ROOT})",
    )
    parser.add_argument(
        "--locales",
        default=",".join(DEFAULT_LOCALES),
        help="Comma-separated locales to enforce (default: en,ar,es,fr,pt,ru,ur)",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path to write a machine-readable summary.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> None:
    args = parse_args(argv)
    repo_root = Path(".").resolve()
    jobs_path = (repo_root / args.jobs).resolve()
    report_root = (repo_root / args.report_root).resolve()

    jobs = load_jobs(jobs_path)
    locales = available_locales(args.locales)
    results = check_jobs(report_root=report_root, jobs=jobs, locales=locales)
    summary = summarize_results(results)

    if args.json_out:
        Path(args.json_out).write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    missing = summary["missing_files"]
    if missing:
        for entry in missing:
            print(
                f"[sns] missing locale file for suffix={entry['suffix']} cycle={entry['cycle']}: {entry['path']}"
            )
        raise SystemExit(1)

    print(
        f"[sns] annex localization check passed ({summary['jobs_checked']} jobs, "
        f"{summary['jobs_with_full_coverage']} with all locales)"
    )


if __name__ == "__main__":  # pragma: no cover
    main()
