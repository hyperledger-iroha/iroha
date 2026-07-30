#!/usr/bin/env python3
"""Helper for booking upcoming SNS KPI annex cycles (roadmap SN-8)."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from textwrap import dedent
from typing import Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
ANNEX_JOBS_PATH = REPO_ROOT / "specs" / "sns" / "regulatory" / "annex_jobs.json"
SUFFIX_LITERAL_RE = re.compile(r"\.[a-z0-9]{1,64}")


@dataclass(frozen=True)
class CycleContext:
    cycle: str
    suffixes: Sequence[str]
    dry_run: bool


@dataclass(frozen=True)
class AnnexMetadata:
    dashboard_path: str
    dashboard_sha256: str | None
    generated_at: str | None


def parse_args(argv: Sequence[str] | None = None) -> CycleContext:
    parser = argparse.ArgumentParser(description="Add SNS KPI annex cycle scaffolding")
    parser.add_argument("cycle", help="Cycle in YYYY-MM format (e.g. 2026-10)")
    parser.add_argument(
        "--suffix",
        action="append",
        dest="suffixes",
        required=True,
        metavar=".NAME",
        help="Suffix to update; repeat for each explicitly selected live SNS policy",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print actions without writing files")
    args = parser.parse_args(argv)

    if not re.fullmatch(r"\d{4}-\d{2}", args.cycle):
        parser.error("cycle must be formatted as YYYY-MM")
    suffixes = tuple(args.suffixes)
    seen_suffixes: set[str] = set()
    for suffix in suffixes:
        if not suffix.startswith("."):
            parser.error("suffix values must include the leading dot")
        if SUFFIX_LITERAL_RE.fullmatch(suffix) is None:
            parser.error(
                "suffix values must be a dot followed by 1-64 lowercase ASCII letters or digits"
            )
        if suffix in seen_suffixes:
            parser.error("suffix values must be unique")
        seen_suffixes.add(suffix)
    return CycleContext(cycle=args.cycle, suffixes=suffixes, dry_run=args.dry_run)


def resolve_annex_metadata(
    suffix: str, cycle: str, default_dashboard_path: str
) -> AnnexMetadata:
    annex_path = REPO_ROOT / "specs" / "sns" / "reports" / suffix / f"{cycle}.md"
    dashboard_path = default_dashboard_path
    dashboard_sha256: str | None = None
    generated_at: str | None = None

    if annex_path.exists():
        front_matter = parse_annex_front_matter(annex_path)
        dashboard_path = front_matter.get("dashboard_path", dashboard_path)
        dashboard_sha256 = front_matter.get("dashboard_sha256")
        generated_at = front_matter.get("generated_at")

    dashboard_abs = REPO_ROOT / dashboard_path
    if dashboard_sha256 is None and dashboard_abs.exists():
        dashboard_sha256 = sha256_hex(dashboard_abs)

    if generated_at is None:
        generated_at = first_present(
            lambda: file_mtime_iso(annex_path),
            lambda: file_mtime_iso(dashboard_abs),
        )

    return AnnexMetadata(
        dashboard_path=dashboard_path,
        dashboard_sha256=dashboard_sha256,
        generated_at=generated_at,
    )


def parse_annex_front_matter(path: Path) -> dict[str, str]:
    try:
        body = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return {}

    lines = body.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}

    collected: dict[str, str] = {}
    in_dashboard_block = False
    for line in lines[1:]:
        stripped = line.strip()
        if stripped == "---":
            break
        if not stripped:
            continue
        indent = len(line) - len(stripped)
        if indent == 0:
            in_dashboard_block = stripped.startswith("dashboard_export:")
        if stripped.startswith("generated_at:"):
            collected["generated_at"] = normalize_yaml_value(stripped.removeprefix("generated_at:"))
            continue
        if in_dashboard_block:
            if stripped.startswith("path:"):
                collected["dashboard_path"] = normalize_yaml_value(
                    stripped.removeprefix("path:")
                )
            elif stripped.startswith("sha256:"):
                collected["dashboard_sha256"] = normalize_yaml_value(
                    stripped.removeprefix("sha256:")
                )
    return collected


def normalize_yaml_value(raw: str) -> str | None:
    value = raw.strip()
    if value.lower() == "null" or value == "":
        return None
    if value.startswith('"') and value.endswith('"'):
        value = value[1:-1]
    elif value.startswith("'") and value.endswith("'"):
        value = value[1:-1]
    return value or None


def first_present(*suppliers):
    for supplier in suppliers:
        value = supplier()
        if value is not None:
            return value
    return None


def file_mtime_iso(path: Path) -> str | None:
    try:
        mtime = path.stat().st_mtime
    except FileNotFoundError:
        return None
    return dt.datetime.fromtimestamp(mtime, tz=dt.timezone.utc).isoformat()


def sha256_hex(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_annex_jobs(cycle: str, suffixes: Sequence[str], *, dry_run: bool) -> bool:
    jobs = json.loads(ANNEX_JOBS_PATH.read_text(encoding="utf-8"))
    existing = {(entry["suffix"], entry["cycle"]) for entry in jobs}
    changed = False
    for suffix in suffixes:
        key = (suffix, cycle)
        if key in existing:
            continue
        changed = True
        job = {
            "suffix": suffix,
            "cycle": cycle,
            "jurisdiction": "eu-dsa",
            "dashboard": "dashboards/grafana/sns_suffix_analytics.json",
        }
        print(f"[annex-jobs] append {job}")
        if not dry_run:
            jobs.append(job)

    if changed and not dry_run:
        jobs.sort(key=lambda entry: (entry["cycle"], entry["suffix"]))
        ANNEX_JOBS_PATH.write_text(json.dumps(jobs, indent=2) + "\n", encoding="utf-8")
    return changed


def ensure_annex_stub(
    cycle: str,
    suffix: str,
    *,
    dry_run: bool,
) -> bool:
    reports_dir = REPO_ROOT / "specs" / "sns" / "reports" / f"{suffix}"
    reports_dir.mkdir(parents=True, exist_ok=True)
    english_path = reports_dir / f"{cycle}.md"
    rel_english = english_path.relative_to(REPO_ROOT)
    dashboard_path = f"artifacts/sns/regulatory/{suffix}/{cycle}/sns_suffix_analytics.json"
    regulatory_target = f"specs/sns/regulatory/eu-dsa/{cycle}.md"

    if english_path.exists():
        created = False
    else:
        metadata = resolve_annex_metadata(suffix, cycle, dashboard_path)
        content = dedent(
            f"""
            ---
            title: SNS KPI Annex ({suffix})
            summary: Dashboard evidence bundle for {suffix} cycle {cycle} (pending export).
            suffix: "{suffix}"
            cycle: "{cycle}"
            status: pending
            generated_at: {json.dumps(metadata.generated_at) if metadata.generated_at else "null"}
            dashboard_uid: "sns-kpis"
            dashboard_title: "Sora Name Service KPIs"
            dashboard_refresh: "1m"
            dashboard_export:
              path: "{metadata.dashboard_path}"
              sha256: "{metadata.dashboard_sha256 or 'pending'}"
            regulatory_annex_target: "{regulatory_target}"
            ---

            # {suffix} KPI Annex — {cycle} (Pending)

            This placeholder file reserves the SN-8 annex slot for cycle {cycle}. Once the Grafana
            export is captured, rerun `scripts/run_sns_annex_jobs.py` so the helper can update the
            `generated_at` field, embed the final SHA-256, and attach the JSON artefact listed above.

            ## Checklist before filing

            - Capture the dashboard export for `{suffix}` and drop it under `{dashboard_path}`.
            - Re-run the annex helper to rewrite this file with the signed metadata.
            - Ensure the EU DSA memo at `{regulatory_target}` references the refreshed hash via
              the `sns-annex:{suffix.lstrip('.')}-{cycle}` marker.
            - Coordinate any public or translated presentation in `iroha-docs` once reviewers
              confirm the final English evidence.
            """
        ).strip() + "\n"
        print(f"[annex-report] create {rel_english.as_posix()}")
        if not dry_run:
            english_path.write_text(content, encoding="utf-8")
        created = True

    return created


def build_annex_section(cycle: str, suffixes: Sequence[str]) -> str:
    annex_blocks = []
    for suffix in suffixes:
        default_dashboard_path = (
            f"artifacts/sns/regulatory/{suffix}/{cycle}/sns_suffix_analytics.json"
        )
        metadata = resolve_annex_metadata(
            suffix, cycle, default_dashboard_path=default_dashboard_path
        )
        sha256 = metadata.dashboard_sha256 or "pending"
        generated_at = metadata.generated_at or "pending"
        annex_blocks.append(
            dedent(
                f"""
                <!-- sns-annex:{suffix.lstrip('.')}-{cycle}:start -->
                ### KPI Dashboard Annex ({suffix} — {cycle})
                - Annex report: specs/sns/reports/{suffix}/{cycle}.md
                - Dashboard export: {metadata.dashboard_path}
                - Dashboard SHA-256: {sha256}
                - Generated: {generated_at}
                <!-- sns-annex:{suffix.lstrip('.')}-{cycle}:end -->
                """
            ).strip()
        )
    return "\n\n".join(annex_blocks)


def ensure_memo_stub(
    cycle: str,
    suffixes: Sequence[str],
    *,
    dry_run: bool,
) -> bool:
    memo_dir = REPO_ROOT / "specs" / "sns" / "regulatory" / "eu-dsa"
    memo_dir.mkdir(parents=True, exist_ok=True)
    english_path = memo_dir / f"{cycle}.md"
    rel_english = english_path.relative_to(REPO_ROOT)
    if english_path.exists():
        created = False
    else:
        annex_section = build_annex_section(cycle, suffixes)
        body = dedent(
            f"""
            ---
            title: EU DSA Hosting & Transparency Guidance – Intake Memo
            jurisdiction: EU
            regulation: Digital Services Act (EU) – KPI annex program
            suffix_scope:
            {chr(10).join(f"  - {suffix}" for suffix in suffixes)}
            owners:
              guardian: guardian-board
              rapporteur: gov-council-seat-4
              steward_ack: sora-foundation-suffix-ops
            status: scheduled
            cycle: {cycle}
            ---

            ## 1. Intake Summary

            - **Bulletin:** Pending governance bulletin for cycle {cycle}.
            - **Key requirements:** Reserve the annex artefacts for this cycle, update the KPI hashes,
              and coordinate any public-facing summary in `iroha-docs`.
            - **Submission window:** Pending governance bulletin; update once the review window is announced.

            Ticket placeholder `SNS-REG-{cycle.replace('-', '')}` tracks the deliverables once the
            bulletin lands.

            ## 2. Impact Mapping

            | Obligation | Mapping | Notes |
            |------------|---------|-------|
            | Annex job scheduling | Append `{cycle}` to `specs/sns/regulatory/annex_jobs.json` and run the annex helper. | `scripts/run_sns_annex_jobs.py --jobs specs/sns/regulatory/annex_jobs.json` |
            | Public documentation | Coordinate any user-facing summary for `{cycle}` in `iroha-docs`. | Update it once the English evidence is final. |
            | Fee/dispute attachments | Capture surcharge + dispute artefacts alongside the dashboard export. | Store under `artifacts/sns/regulatory/<suffix>/{cycle}/`. |

            Memo hash (Blake3): pending signature.

            ## 3. Charter & KPI Updates

            Fill this section after Governance publishes the addendum for `{cycle}`.

            ## 4. Activation & Evidence

            Summarize annex execution steps, telemetry exports, and reviewer acknowledgements here once
            the artefacts are attached.

            ## 5. Attachments

            | Artifact | Path / Reference |
            |----------|------------------|
            | Bulletin digest | artifacts/sns/regulatory/eu-dsa/{cycle}/bulletin.sha256 |
            | Council addendum draft | specs/sns/governance_addenda/SN-CH-{cycle}-A1.md |
            | Fee disclosure samples | artifacts/sns/regulatory/<suffix>/{cycle}/fee_disclosure.json |
            | Dispute queue hashes | artifacts/sns/regulatory/<suffix>/{cycle}/dispute_queue_otlp.sha256 |

            {annex_section}
            """
        ).strip() + "\n"
        print(f"[regulatory-memo] create {rel_english.as_posix()}")
        if not dry_run:
            english_path.write_text(body, encoding="utf-8")
        created = True

    return created


def main() -> None:
    ctx = parse_args()

    changed = False
    changed |= ensure_annex_jobs(ctx.cycle, ctx.suffixes, dry_run=ctx.dry_run)
    for suffix in ctx.suffixes:
        changed |= ensure_annex_stub(ctx.cycle, suffix, dry_run=ctx.dry_run)
    changed |= ensure_memo_stub(ctx.cycle, ctx.suffixes, dry_run=ctx.dry_run)

    if ctx.dry_run:
        print("Done (dry-run)")
    else:
        print("Done" if changed else "No changes needed")


if __name__ == "__main__":
    main()
