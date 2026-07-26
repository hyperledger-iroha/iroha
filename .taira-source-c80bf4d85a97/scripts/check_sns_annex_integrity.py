#!/usr/bin/env python3
"""
Verify that SNS KPI annex reports reference existing dashboard exports and that the
recorded SHA-256 digests match the artefacts on disk.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence

DEFAULT_JOBS = "docs/source/sns/regulatory/annex_jobs.json"
DEFAULT_REPORT_ROOT = "docs/source/sns/reports"


def load_jobs(path: Path) -> List[dict]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"[error] failed to parse annex jobs JSON: {exc}") from exc
    if not isinstance(data, list):
        raise SystemExit(f"[error] annex jobs file must contain a JSON array: {path}")
    return data


def extract_front_matter(report_path: Path) -> List[str]:
    lines = report_path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0].strip() != "---":
        raise SystemExit(f"[error] annex report missing front matter: {report_path}")
    fm_lines: List[str] = []
    for line in lines[1:]:
        if line.strip() == "---":
            break
        fm_lines.append(line.rstrip("\n"))
    else:
        raise SystemExit(f"[error] annex report front matter not terminated: {report_path}")
    return fm_lines


def _strip_quotes(value: str) -> str:
    value = value.strip()
    if value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    return value


def _parse_top_level(lines: Iterable[str], key: str) -> Optional[str]:
    needle = f"{key}:"
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith(needle):
            return _strip_quotes(stripped[len(needle) :])
    return None


def _parse_dashboard_export(lines: List[str]) -> Dict[str, str]:
    in_block = False
    indent_prefix = "  "
    path_value = None
    sha_value = None
    for line in lines:
        if not in_block:
            if line.strip().startswith("dashboard_export"):
                in_block = True
            continue
        if not line.startswith(indent_prefix):
            in_block = False
            continue
        stripped = line.strip()
        if stripped.startswith("path:"):
            path_value = _strip_quotes(stripped.split(":", 1)[1])
        elif stripped.startswith("sha256:"):
            sha_value = _strip_quotes(stripped.split(":", 1)[1])
    if not path_value or not sha_value:
        raise SystemExit("front matter missing dashboard_export.path/sha256 entries")
    return {"path": path_value, "sha256": sha_value}


def compute_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sanitize_suffix(value: str) -> str:
    return value if value.startswith(".") else f".{value}"


def report_path_for_job(report_root: Path, suffix: str, cycle: str) -> Path:
    return report_root / sanitize_suffix(suffix) / f"{cycle}.md"


@dataclass
class AnnexReportStatus:
    suffix: str
    cycle: str
    report_path: Path
    artifact_path: Path
    expected_sha: str
    actual_sha: str

    @property
    def ok(self) -> bool:
        return self.expected_sha == self.actual_sha


def audit_annex_reports(
    *,
    repo_root: Path,
    report_root: Path,
    jobs: Iterable[dict],
) -> List[AnnexReportStatus]:
    results: List[AnnexReportStatus] = []
    for job in jobs:
        suffix = job.get("suffix")
        cycle = job.get("cycle")
        if not suffix or not cycle:
            raise SystemExit(f"[error] job missing suffix/cycle: {job}")
        report_path = report_path_for_job(report_root, suffix, cycle)
        if not report_path.exists():
            raise SystemExit(f"[error] annex report not found: {report_path}")
        fm_lines = extract_front_matter(report_path)
        dashboard_meta = _parse_dashboard_export(fm_lines)
        artifact = (repo_root / dashboard_meta["path"]).resolve()
        if not artifact.exists():
            raise SystemExit(
                f"[error] dashboard export missing for {suffix} {cycle}: {artifact}"
            )
        expected_sha = dashboard_meta["sha256"]
        actual_sha = compute_sha256(artifact)
        results.append(
            AnnexReportStatus(
                suffix=suffix,
                cycle=cycle,
                report_path=report_path,
                artifact_path=artifact,
                expected_sha=expected_sha,
                actual_sha=actual_sha,
            )
        )
    return results


def summarize_results(results: Sequence[AnnexReportStatus]) -> dict:
    mismatches = [
        {
            "suffix": result.suffix,
            "cycle": result.cycle,
            "report": str(result.report_path),
            "artifact": str(result.artifact_path),
            "expected_sha": result.expected_sha,
            "actual_sha": result.actual_sha,
        }
        for result in results
        if not result.ok
    ]
    return {
        "reports_checked": len(results),
        "reports_with_valid_hash": len(results) - len(mismatches),
        "mismatched_reports": mismatches,
    }


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--jobs",
        default=DEFAULT_JOBS,
        help=f"Path to annex_jobs.json (default: {DEFAULT_JOBS})",
    )
    parser.add_argument(
        "--report-root",
        default=DEFAULT_REPORT_ROOT,
        help=f"Directory containing localized annex reports (default: {DEFAULT_REPORT_ROOT})",
    )
    parser.add_argument(
        "--base-dir",
        default=".",
        help="Repository root; defaults to current working directory.",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path to write a JSON summary.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> None:
    args = parse_args(argv)
    repo_root = Path(args.base_dir).resolve()
    jobs = load_jobs((repo_root / args.jobs).resolve())
    report_root = (repo_root / args.report_root).resolve()
    results = audit_annex_reports(
        repo_root=repo_root,
        report_root=report_root,
        jobs=jobs,
    )
    summary = summarize_results(results)
    if args.json_out:
        Path(args.json_out).write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    mismatches = summary["mismatched_reports"]
    if mismatches:
        for entry in mismatches:
            print(
                "[sns] annex hash mismatch for "
                f"suffix={entry['suffix']} cycle={entry['cycle']}: "
                f"{entry['artifact']} expected {entry['expected_sha']} "
                f"but computed {entry['actual_sha']}"
            )
        raise SystemExit(1)
    print(
        "[sns] annex integrity check passed "
        f"({summary['reports_checked']} reports audited)"
    )


if __name__ == "__main__":  # pragma: no cover
    main()
