#!/usr/bin/env python3
"""
Ensure SNS KPI annex jobs stay aligned with regulatory memos and report stubs.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, MutableMapping, Sequence, Tuple

DEFAULT_JOBS = "docs/source/sns/regulatory/annex_jobs.json"
DEFAULT_REGULATORY_ROOT = "docs/source/sns/regulatory"
DEFAULT_REPORT_ROOT = "docs/source/sns/reports"

MARKER_RE = re.compile(
    r"<!--\s*sns-annex:([a-z0-9\.\-]+?)-(\d{4}-\d{2}):start\s*-->",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class AnnexJob:
    suffix: str
    cycle: str
    jurisdiction: str
    regulatory_entry: Path


def normalize_suffix(value: str) -> str:
    text = value.strip()
    if not text:
        raise ValueError("suffix cannot be empty")
    if not text.startswith("."):
        text = f".{text}"
    return text


def load_jobs(path: Path, regulatory_root: Path) -> List[AnnexJob]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - serialization error path
        raise SystemExit(f"[error] failed to parse annex jobs JSON: {exc}") from exc
    if not isinstance(raw, list):
        raise SystemExit(f"[error] annex jobs file must contain a JSON array: {path}")

    jobs: List[AnnexJob] = []
    for entry in raw:
        if not isinstance(entry, Mapping):
            raise SystemExit(f"[error] invalid annex job entry (expected object): {entry!r}")
        suffix = entry.get("suffix")
        cycle = entry.get("cycle")
        jurisdiction = entry.get("jurisdiction")
        if not isinstance(suffix, str) or not isinstance(cycle, str) or not isinstance(jurisdiction, str):
            raise SystemExit(f"[error] annex job missing required fields: {entry!r}")
        normalized_suffix = normalize_suffix(suffix)
        regulatory_entry = entry.get("regulatory_entry")
        regulatory_path = (
            Path(regulatory_entry)
            if isinstance(regulatory_entry, str)
            else regulatory_root / jurisdiction / f"{cycle}.md"
        )
        jobs.append(
            AnnexJob(
                suffix=normalized_suffix,
                cycle=cycle,
                jurisdiction=jurisdiction,
                regulatory_entry=regulatory_path,
            )
        )
    return jobs


def annex_report_path(report_root: Path, suffix: str, cycle: str) -> Path:
    return report_root / suffix / f"{cycle}.md"


def is_translation_stub(path: Path) -> bool:
    return bool(re.fullmatch(r"\d{4}-\d{2}\.[a-z]{2}\.md", path.name))


def collect_annex_markers(regulatory_root: Path) -> Dict[Tuple[str, str], List[Path]]:
    markers: Dict[Tuple[str, str], List[Path]] = {}
    for path in regulatory_root.rglob("*.md"):
        if not re.fullmatch(r"\d{4}-\d{2}\.md", path.name):
            continue
        if is_translation_stub(path):
            continue
        text = path.read_text(encoding="utf-8")
        for suffix_raw, cycle in MARKER_RE.findall(text):
            suffix = normalize_suffix(suffix_raw.lower())
            markers.setdefault((suffix, cycle), []).append(path)
    return markers


def summarize(
    *,
    jobs: Sequence[AnnexJob],
    markers: Mapping[Tuple[str, str], Sequence[Path]],
    report_root: Path,
) -> MutableMapping[str, object]:
    summary: MutableMapping[str, object] = {
        "jobs_total": len(jobs),
        "duplicates": [],
        "unsorted": False,
        "missing_memos": [],
        "missing_reports": [],
        "jobs_without_markers": [],
        "markers_without_jobs": [],
    }

    seen_keys: set[Tuple[str, str, str]] = set()
    job_index: set[Tuple[str, str]] = set()
    duplicates: List[dict] = []

    sorted_jobs = sorted(jobs, key=lambda job: (job.cycle, job.suffix))
    if list(jobs) != sorted_jobs:
        summary["unsorted"] = True

    for job in jobs:
        key = (job.suffix, job.cycle, job.jurisdiction)
        if key in seen_keys:
            duplicates.append(
                {"suffix": job.suffix, "cycle": job.cycle, "jurisdiction": job.jurisdiction}
            )
        else:
            seen_keys.add(key)
        job_index.add((job.suffix, job.cycle))

        if not job.regulatory_entry.exists():
            summary["missing_memos"].append(
                {
                    "suffix": job.suffix,
                    "cycle": job.cycle,
                    "path": str(job.regulatory_entry),
                }
            )

        report_path = annex_report_path(report_root, job.suffix, job.cycle)
        if not report_path.exists():
            summary["missing_reports"].append(
                {"suffix": job.suffix, "cycle": job.cycle, "path": str(report_path)}
            )

        marker_sources = markers.get((job.suffix, job.cycle))
        if not marker_sources:
            summary["jobs_without_markers"].append(
                {
                    "suffix": job.suffix,
                    "cycle": job.cycle,
                    "expected_memo": str(job.regulatory_entry),
                }
            )

    summary["duplicates"] = duplicates

    markers_without_jobs: List[dict] = []
    for (suffix, cycle), sources in markers.items():
        if (suffix, cycle) not in job_index:
            markers_without_jobs.append(
                {
                    "suffix": suffix,
                    "cycle": cycle,
                    "sources": [str(path) for path in sources],
                }
            )
    summary["markers_without_jobs"] = markers_without_jobs
    return summary


def has_issues(summary: Mapping[str, object]) -> bool:
    return bool(
        summary.get("unsorted")
        or summary.get("duplicates")
        or summary.get("missing_memos")
        or summary.get("missing_reports")
        or summary.get("jobs_without_markers")
        or summary.get("markers_without_jobs")
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate SNS annex job scheduling and localization coverage."
    )
    parser.add_argument(
        "--jobs",
        default=DEFAULT_JOBS,
        help=f"Path to annex_jobs.json (default: {DEFAULT_JOBS})",
    )
    parser.add_argument(
        "--regulatory-root",
        default=DEFAULT_REGULATORY_ROOT,
        help="Root directory containing regulatory memos with sns-annex markers.",
    )
    parser.add_argument(
        "--report-root",
        default=DEFAULT_REPORT_ROOT,
        help="Root directory containing annex report stubs.",
    )
    parser.add_argument(
        "--base-dir",
        default=".",
        help="Repository root; defaults to current working directory.",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path to write a machine-readable summary.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    base_dir = Path(args.base_dir).resolve()
    jobs_path = (base_dir / args.jobs).resolve()
    regulatory_root = (base_dir / args.regulatory_root).resolve()
    report_root = (base_dir / args.report_root).resolve()

    jobs = load_jobs(jobs_path, regulatory_root=regulatory_root)
    markers = collect_annex_markers(regulatory_root)
    summary = summarize(jobs=jobs, markers=markers, report_root=report_root)

    if args.json_out:
        Path(args.json_out).write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    if has_issues(summary):
        for dup in summary["duplicates"]:
            print(
                "[sns-annex] duplicate annex job entry detected: "
                f"suffix={dup['suffix']} cycle={dup['cycle']} jurisdiction={dup['jurisdiction']}",
                file=sys.stderr,
            )
        if summary["unsorted"]:
            print("[sns-annex] annex_jobs.json entries are not sorted by cycle/suffix", file=sys.stderr)
        for entry in summary["missing_memos"]:
            print(
                f"[sns-annex] missing regulatory memo for suffix={entry['suffix']} "
                f"cycle={entry['cycle']}: {entry['path']}",
                file=sys.stderr,
            )
        for entry in summary["missing_reports"]:
            print(
                f"[sns-annex] missing annex report for suffix={entry['suffix']} "
                f"cycle={entry['cycle']}: {entry['path']}",
                file=sys.stderr,
            )
        for entry in summary["jobs_without_markers"]:
            print(
                f"[sns-annex] regulatory memo missing sns-annex marker for "
                f"suffix={entry['suffix']} cycle={entry['cycle']}: {entry['expected_memo']}",
                file=sys.stderr,
            )
        for entry in summary["markers_without_jobs"]:
            print(
                "[sns-annex] annex marker without matching job: "
                f"suffix={entry['suffix']} cycle={entry['cycle']} sources={entry['sources']}",
                file=sys.stderr,
            )
        return 1

    print(
        "[sns-annex] schedule check passed "
        f"({summary['jobs_total']} jobs, {len(markers)} annex markers)"
    )
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
