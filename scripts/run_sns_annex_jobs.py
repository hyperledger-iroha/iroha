#!/usr/bin/env python3
"""
Run `cargo xtask sns-annex` for every configured suffix/cycle pair and update
the associated regulatory memos with the new KPI annex block.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate SNS KPI annex reports and refresh regulatory memos."
    )
    parser.add_argument(
        "--jobs",
        default="docs/source/sns/regulatory/annex_jobs.json",
        help="Path to the JSON file describing annex jobs.",
    )
    parser.add_argument(
        "--runner",
        default="cargo xtask sns-annex",
        help="Command used to run each annex job (default: %(default)s).",
    )
    parser.add_argument(
        "--default-dashboard",
        dest="default_dashboard",
        default="dashboards/grafana/sns_suffix_analytics.json",
        help="Dashboard export used when a job omits the `dashboard` key.",
    )
    parser.add_argument(
        "--base-dir",
        default=".",
        help="Repository root (cwd is used when omitted).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print commands instead of executing them.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print progress information.",
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="Validate recorded annex metadata without running cargo xtask.",
    )
    parser.add_argument(
        "--suffix-filter",
        dest="suffix_filter",
        action="append",
        default=[],
        help="Limit execution to annex jobs matching the given suffix (repeatable).",
    )
    parser.add_argument(
        "--cycle-filter",
        dest="cycle_filter",
        action="append",
        default=[],
        help="Limit execution to annex jobs matching the given cycle (repeatable).",
    )
    return parser.parse_args()


def load_jobs(path: Path) -> List[Dict[str, Any]]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, list):
        raise ValueError(f"Annex jobs file must contain a JSON array: {path}")
    return data


def ensure_exists(path: Path, description: str) -> None:
    if not path.exists():
        raise FileNotFoundError(f"{description} not found: {path}")


def sanitize_suffix_for_path(suffix: str) -> str:
    # Preserve leading dots to keep path layout stable.
    return suffix


def job_output_path(suffix: str, cycle: str) -> Path:
    return Path("docs/source/sns/reports") / sanitize_suffix_for_path(suffix) / f"{cycle}.md"


def job_dashboard_artifact_path(suffix: str, cycle: str, dashboard_path: Path) -> Path:
    artifact_name = dashboard_path.name
    return (
        Path("artifacts/sns/regulatory")
        / sanitize_suffix_for_path(suffix)
        / cycle
        / artifact_name
    )


def job_regulatory_entry_path(job: Dict[str, Any]) -> Path:
    if "regulatory_entry" in job:
        return Path(job["regulatory_entry"])
    jurisdiction = job.get("jurisdiction")
    if not jurisdiction:
        raise ValueError("job requires `jurisdiction` or explicit `regulatory_entry`")
    cycle = job["cycle"]
    return Path("docs/source/sns/regulatory") / jurisdiction / f"{cycle}.md"


def job_portal_entry_path(job: Dict[str, Any]) -> Optional[Path]:
    if "portal_entry" in job:
        return Path(job["portal_entry"])
    jurisdiction = job.get("jurisdiction")
    cycle = job.get("cycle")
    if not jurisdiction or not cycle:
        return None
    candidate = Path("docs/portal/docs/sns/regulatory") / f"{jurisdiction}-{cycle}.md"
    return candidate if candidate.exists() else None


def _normalize_suffix_filter(value: str) -> str:
    text = value.strip()
    if not text:
        raise ValueError("suffix filter cannot be empty")
    return text if text.startswith(".") else f".{text}"


def _normalize_cycle_filter(value: str) -> str:
    text = value.strip()
    if not text:
        raise ValueError("cycle filter cannot be empty")
    return text


def _job_matches_filters(
    job: Dict[str, Any],
    suffix_filters: Sequence[str],
    cycle_filters: Sequence[str],
) -> bool:
    if suffix_filters and job.get("suffix") not in suffix_filters:
        return False
    if cycle_filters and job.get("cycle") not in cycle_filters:
        return False
    return True


def apply_job_filters(
    jobs: Sequence[Dict[str, Any]],
    suffix_filters: Sequence[str],
    cycle_filters: Sequence[str],
) -> List[Dict[str, Any]]:
    if not suffix_filters and not cycle_filters:
        return list(jobs)
    return [
        job
        for job in jobs
        if _job_matches_filters(job, suffix_filters=suffix_filters, cycle_filters=cycle_filters)
    ]


def run_job(
    runner: Sequence[str],
    job: Dict[str, Any],
    default_dashboard: Path,
    dry_run: bool,
    verbose: bool,
) -> None:
    suffix = job.get("suffix")
    cycle = job.get("cycle")
    if not suffix or not cycle:
        raise ValueError(f"job missing suffix and/or cycle: {job}")

    dashboard = Path(job.get("dashboard", default_dashboard))
    ensure_exists(dashboard, "dashboard export")

    output = Path(job.get("output", job_output_path(suffix, cycle)))
    output.parent.mkdir(parents=True, exist_ok=True)

    artifact = Path(
        job.get("dashboard_artifact", job_dashboard_artifact_path(suffix, cycle, dashboard))
    )
    artifact.parent.mkdir(parents=True, exist_ok=True)

    regulatory_entry = job_regulatory_entry_path(job)
    ensure_exists(regulatory_entry, "regulatory memo")
    portal_entry = job_portal_entry_path(job)
    if portal_entry is not None:
        ensure_exists(portal_entry, "portal memo")

    command = list(runner) + [
        "--suffix",
        suffix,
        "--cycle",
        cycle,
        "--dashboard",
        str(dashboard),
        "--dashboard-artifact",
        str(artifact),
        "--output",
        str(output),
        "--regulatory-entry",
        str(regulatory_entry),
    ]
    if portal_entry is not None:
        command += ["--portal-entry", str(portal_entry)]
    if verbose or dry_run:
        print(" ".join(shlex.quote(arg) for arg in command))
    if not dry_run:
        subprocess.run(command, check=True)


def regulatory_marker_id(suffix: str, cycle: str) -> str:
    trimmed = suffix.strip().lstrip(".")
    cleaned = "".join(ch.lower() if ch.isalnum() else "-" for ch in trimmed)
    normalized = cleaned.strip("-")
    if not normalized:
        return f"global-{cycle}"
    return f"{normalized}-{cycle}"


def _relative_to_root(path: Path, base_dir: Path) -> str:
    try:
        rel = path.resolve().relative_to(base_dir)
    except ValueError:
        return path.resolve().as_posix()
    return rel.as_posix()


def _extract_backticked_value(line: str) -> str:
    start = line.find("`")
    end = line.rfind("`")
    if start == -1 or end == -1 or end <= start:
        raise ValueError(f"malformed annex line (missing backticks): {line!r}")
    return line[start + 1 : end]


def _read_annex_block(path: Path, marker_id: str) -> str:
    text = path.read_text(encoding="utf-8")
    start_marker = f"<!-- sns-annex:{marker_id}:start -->"
    end_marker = f"<!-- sns-annex:{marker_id}:end -->"
    start = text.find(start_marker)
    if start == -1:
        raise ValueError(f"{path} missing annex marker {start_marker}")
    end = text.find(end_marker, start)
    if end == -1:
        raise ValueError(f"{path} missing annex marker {end_marker}")
    return text[start : end + len(end_marker)]


def _parse_annex_fields(block: str) -> Dict[str, str]:
    fields: Dict[str, str] = {}
    for raw_line in block.splitlines():
        line = raw_line.strip()
        if line.startswith("- Annex report:"):
            fields["annex_report"] = _extract_backticked_value(line)
        elif line.startswith("- Dashboard export:"):
            fields["dashboard_export"] = _extract_backticked_value(line)
        elif line.startswith("- Dashboard SHA-256:"):
            fields["dashboard_sha256"] = _extract_backticked_value(line)
        elif line.startswith("- Generated:"):
            fields["generated"] = _extract_backticked_value(line)
    missing = {"annex_report", "dashboard_export", "dashboard_sha256"}.difference(fields)
    if missing:
        raise ValueError(f"annex block missing fields: {', '.join(sorted(missing))}")
    return fields


def _sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def _validate_annex_block(
    block_owner: Path,
    marker_id: str,
    expected_annex: str,
    expected_artifact: str,
    expected_digest: str,
) -> None:
    block = _read_annex_block(block_owner, marker_id)
    fields = _parse_annex_fields(block)
    if fields["annex_report"] != expected_annex:
        raise ValueError(
            f"{block_owner} annex path mismatch: recorded {fields['annex_report']}, expected {expected_annex}"
        )
    if fields["dashboard_export"] != expected_artifact:
        raise ValueError(
            f"{block_owner} dashboard export mismatch: recorded {fields['dashboard_export']}, expected {expected_artifact}"
        )
    if fields["dashboard_sha256"].lower() != expected_digest.lower():
        raise ValueError(
            f"{block_owner} dashboard digest mismatch: recorded {fields['dashboard_sha256']}, expected {expected_digest}"
        )


def validate_job(
    job: Dict[str, Any],
    default_dashboard: Path,
    base_dir: Path,
) -> None:
    suffix = job.get("suffix")
    cycle = job.get("cycle")
    if not suffix or not cycle:
        raise ValueError(f"job missing suffix/cycle: {job}")

    dashboard = Path(job.get("dashboard", default_dashboard))
    ensure_exists(dashboard, "dashboard export")

    output = Path(job.get("output", job_output_path(suffix, cycle)))
    if not output.exists():
        print(f"[sns-annex] skipping {suffix}/{cycle}: annex output missing at {output}", file=sys.stderr)
        return

    artifact = Path(
        job.get("dashboard_artifact", job_dashboard_artifact_path(suffix, cycle, dashboard))
    )
    if not artifact.exists():
        print(
            f"[sns-annex] skipping {suffix}/{cycle}: dashboard artifact missing at {artifact}",
            file=sys.stderr,
        )
        return

    regulatory_entry = job_regulatory_entry_path(job)
    if not regulatory_entry.exists():
        print(
            f"[sns-annex] skipping {suffix}/{cycle}: regulatory memo missing at {regulatory_entry}",
            file=sys.stderr,
        )
        return
    portal_entry = job_portal_entry_path(job)
    if portal_entry is not None and not portal_entry.exists():
        print(
            f"[sns-annex] skipping {suffix}/{cycle}: portal memo missing at {portal_entry}",
            file=sys.stderr,
        )
        return

    marker_id = regulatory_marker_id(suffix, cycle)
    annex_path = _relative_to_root(output, base_dir)
    artifact_path = _relative_to_root(artifact, base_dir)
    digest = _sha256_file(artifact)

    _validate_annex_block(regulatory_entry, marker_id, annex_path, artifact_path, digest)
    if portal_entry is not None:
        _validate_annex_block(portal_entry, marker_id, annex_path, artifact_path, digest)


def validate_jobs(
    jobs: Iterable[Dict[str, Any]],
    default_dashboard: Path,
    base_dir: Path,
) -> None:
    for job in jobs:
        validate_job(job, default_dashboard, base_dir)


def main() -> int:
    args = parse_args()
    base_dir = Path(args.base_dir).resolve()
    os.chdir(base_dir)
    jobs = load_jobs(Path(args.jobs))
    try:
        suffix_filters = tuple(_normalize_suffix_filter(value) for value in args.suffix_filter)
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    try:
        cycle_filters = tuple(_normalize_cycle_filter(value) for value in args.cycle_filter)
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    jobs = apply_job_filters(jobs, suffix_filters=suffix_filters, cycle_filters=cycle_filters)
    if not jobs:
        print("[sns-annex] no annex jobs matched the provided filters", file=sys.stderr)
        return 1
    default_dashboard = Path(args.default_dashboard)
    ensure_exists(default_dashboard, "default dashboard export")
    if args.check_only:
        validate_jobs(jobs=jobs, default_dashboard=default_dashboard, base_dir=base_dir)
        return 0
    runner = shlex.split(args.runner)
    if not runner:
        raise ValueError("runner command cannot be empty")
    for job in jobs:
        run_job(
            runner=runner,
            job=job,
            default_dashboard=default_dashboard,
            dry_run=args.dry_run,
            verbose=args.verbose,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
