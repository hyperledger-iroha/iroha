#!/usr/bin/env python3
"""Helper that scaffolds red-team drill artefacts and report files."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

from . import red_team_common

DEFAULT_TEMPLATE = red_team_common.DEFAULT_TEMPLATE
DEFAULT_REPORTS_DIR = red_team_common.DEFAULT_REPORTS_DIR
DEFAULT_ARTIFACTS_DIR = red_team_common.DEFAULT_ARTIFACTS_DIR


@dataclass(frozen=True)
class ScaffoldResult:
    """Outcome details for a scaffolding operation."""

    report_path: Path
    artifact_root: Path
    created_dirs: Sequence[Path]
    created_report: bool


def _render_template(template: str, replacements: dict[str, str]) -> str:
    rendered = template
    for placeholder, replacement in replacements.items():
        rendered = rendered.replace(placeholder, replacement)
    return rendered


def scaffold_drill(
    *,
    scenario_id: str,
    scenario_name: str,
    scenario_class: str | None,
    window: str | None,
    operators: str | None,
    git_sha: str | None,
    sorafs_cid: str | None,
    template_path: Path = DEFAULT_TEMPLATE,
    reports_dir: Path = DEFAULT_REPORTS_DIR,
    artifacts_root: Path = DEFAULT_ARTIFACTS_DIR,
    force: bool = False,
) -> ScaffoldResult:
    """Create the report stub and artefact directories for a drill."""

    try:
        scenario_date, slug = red_team_common.parse_scenario_id(scenario_id)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(str(exc)) from exc
    month_label = scenario_date.strftime("%Y-%m")
    report_filename = f"{month_label}-mod-red-team-{slug}.md"
    report_path = reports_dir / report_filename
    if report_path.exists() and not force:
        raise FileExistsError(f"{report_path} already exists (pass --force to overwrite)")

    reports_dir.mkdir(parents=True, exist_ok=True)
    artifacts_base = red_team_common.resolve_paths(
        scenario_id,
        artifacts_root=artifacts_root,
    ).artifact_root
    subdirs: Iterable[str] = (
        "dashboards",
        "alerts",
        "logs",
        "manifests",
        "evidence",
    )
    created_dirs: list[Path] = []
    artifacts_base.mkdir(parents=True, exist_ok=True)
    for name in subdirs:
        path = artifacts_base / name
        path.mkdir(exist_ok=True)
        created_dirs.append(path)

    template_text = template_path.read_text(encoding="utf-8")
    replacements: dict[str, str] = {
        "<SCENARIO NAME>": scenario_name,
        "<YYYYMMDD>-<scenario>": scenario_id,
    }
    if scenario_class:
        replacements["`smuggling | bribery | gateway | ...`"] = f"`{scenario_class}`"
    if window:
        replacements["<YYYY-MM-DD HH:MMZ – HH:MMZ>"] = window
    if operators:
        replacements["<names / handles>"] = operators
    if git_sha:
        replacements["<git SHA>"] = git_sha
    if sorafs_cid:
        replacements["<cid>"] = sorafs_cid

    rendered = _render_template(template_text, replacements)
    report_path.write_text(rendered, encoding="utf-8")

    checklist_path = artifacts_base / "README.md"
    checklist_path.write_text(
        (
            f"# Red-Team Drill Artefacts — {scenario_id}\n\n"
            f"- Scenario: {scenario_name} ({scenario_id})\n"
            f"- Report: `{report_path}`\n"
            f"- Created at: {scenario_date.isoformat()}\n\n"
            "## Next Actions\n"
            "- [ ] Export dashboards into `dashboards/`\n"
            "- [ ] Archive Alertmanager bundle into `alerts/`\n"
            "- [ ] Store CLI logs and Torii traces under `logs/`\n"
            "- [ ] Drop Norito manifests/proofs into `manifests/`\n"
            "- [ ] Attach evidence bundles (pcap, screenshots) into `evidence/`\n"
        ),
        encoding="utf-8",
    )

    return ScaffoldResult(
        report_path=report_path,
        artifact_root=artifacts_base,
        created_dirs=tuple(created_dirs),
        created_report=True,
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scenario-id",
        required=True,
        help="Identifier matching YYYYMMDD-slug (used for filenames and directories).",
    )
    parser.add_argument(
        "--scenario-name",
        required=True,
        help="Human readable scenario name written into the report.",
    )
    parser.add_argument(
        "--scenario-class",
        help="Scenario class inserted into the metadata (e.g. smuggling, bribery).",
    )
    parser.add_argument(
        "--window",
        help="Execution window, e.g. '2026-02-20 09:00Z – 12:00Z'.",
    )
    parser.add_argument(
        "--operators",
        help="Comma-separated operator names to pre-fill the report metadata.",
    )
    parser.add_argument(
        "--git-sha",
        help="Optional git commit used for dashboard snapshots.",
    )
    parser.add_argument(
        "--sorafs-cid",
        help="Optional SoraFS CID recorded in the metadata block.",
    )
    parser.add_argument(
        "--template",
        type=Path,
        default=DEFAULT_TEMPLATE,
        help=f"Path to the report template (default: {DEFAULT_TEMPLATE}).",
    )
    parser.add_argument(
        "--reports-dir",
        type=Path,
        default=DEFAULT_REPORTS_DIR,
        help=f"Directory for report files (default: {DEFAULT_REPORTS_DIR}).",
    )
    parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=DEFAULT_ARTIFACTS_DIR,
        help=f"Root directory for evidence bundles (default: {DEFAULT_ARTIFACTS_DIR}).",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing report files if they are present.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    result = scaffold_drill(
        scenario_id=args.scenario_id,
        scenario_name=args.scenario_name,
        scenario_class=args.scenario_class,
        window=args.window,
        operators=args.operators,
        git_sha=args.git_sha,
        sorafs_cid=args.sorafs_cid,
        template_path=args.template,
        reports_dir=args.reports_dir,
        artifacts_root=args.artifacts_dir,
        force=args.force,
    )
    print(f"Report created at {result.report_path}")
    print(f"Artifacts root prepared at {result.artifact_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
