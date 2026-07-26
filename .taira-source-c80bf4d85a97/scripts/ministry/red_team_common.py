"""Shared helpers for Ministry red-team tooling."""

from __future__ import annotations

import datetime as _dt
import re
from dataclasses import dataclass
from pathlib import Path

DEFAULT_TEMPLATE = Path("docs/source/ministry/reports/moderation_red_team_template.md")
DEFAULT_REPORTS_DIR = Path("docs/source/ministry/reports")
DEFAULT_ARTIFACTS_DIR = Path("artifacts/ministry/red-team")

_SCENARIO_RE = re.compile(r"^(?P<date>\d{8})-(?P<slug>[a-z0-9][a-z0-9-]+)$")


def parse_scenario_id(value: str) -> tuple[_dt.date, str]:
    """Validate and split a scenario id into date + slug."""

    match = _SCENARIO_RE.fullmatch(value)
    if not match:
        raise ValueError(
            "scenario id must match YYYYMMDD-slug (lowercase letters, digits, hyphen)."
        )
    parsed_date = _dt.datetime.strptime(match.group("date"), "%Y%m%d").date()
    return parsed_date, match.group("slug")


@dataclass(frozen=True)
class ScenarioPaths:
    """Materialized filesystem layout for a scenario."""

    scenario_id: str
    scenario_date: _dt.date
    slug: str
    month_label: str
    artifact_root: Path

    def ensure_subdir(self, name: str) -> Path:
        path = self.artifact_root / name
        path.mkdir(parents=True, exist_ok=True)
        return path


def resolve_paths(
    scenario_id: str,
    *,
    artifacts_root: Path = DEFAULT_ARTIFACTS_DIR,
) -> ScenarioPaths:
    """Resolve standard red-team directories for a scenario."""

    scenario_date, slug = parse_scenario_id(scenario_id)
    month_label = scenario_date.strftime("%Y-%m")
    artifact_root = artifacts_root / month_label / slug
    artifact_root.mkdir(parents=True, exist_ok=True)
    return ScenarioPaths(
        scenario_id=scenario_id,
        scenario_date=scenario_date,
        slug=slug,
        month_label=month_label,
        artifact_root=artifact_root,
    )
