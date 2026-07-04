"""Tests for SoraFS drill-log telemetry helpers."""

from __future__ import annotations

import subprocess
import textwrap
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TELEMETRY_DIR = REPO_ROOT / "scripts" / "telemetry"
LOG_SCRIPT = TELEMETRY_DIR / "log_sorafs_drill.sh"
VALIDATE_SCRIPT = TELEMETRY_DIR / "validate_drill_log.sh"

DRILL_LOG_HEADER = textwrap.dedent(
    """\
    ---
    title: SoraFS Chaos Drill Log
    summary: Registry of executed chaos drills and incident rehearsals.
    ---

    | Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
    |------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|
    """
)


def run_script(*args: str) -> subprocess.CompletedProcess[str]:
    """Run a shell helper and return its completed process."""
    return subprocess.run(
        ["bash", *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def validate_log(path: Path) -> subprocess.CompletedProcess[str]:
    """Run the drill-log validator for `path`."""
    return run_script(str(VALIDATE_SCRIPT), str(path))


def test_log_sorafs_drill_escapes_table_cells_and_validates(tmp_path: Path) -> None:
    log_path = tmp_path / "nested" / "drill-log.md"

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway | outage\nnorth",
        "--status",
        "follow-up",
        "--date",
        "2026-07-04",
        "--start",
        "10:00Z",
        "--end",
        "10:15Z",
        "--ic",
        "IC | primary",
        "--scribe",
        "scribe | one",
        "--notes",
        "needs | action\nnext",
        "--link",
        "https://example.invalid/drill|case",
        "--log",
        str(log_path),
    )

    assert result.returncode == 0, result.stderr
    source = log_path.read_text(encoding="utf-8")
    assert "Gateway \\&#124; outage north" in source
    assert "IC \\&#124; primary" in source
    assert "scribe \\&#124; one" in source
    assert "needs \\&#124; action next" in source
    assert "https://example.invalid/drill\\&#124;case" in source

    validated = validate_log(log_path)
    assert validated.returncode == 0, validated.stderr
    assert "Drill log OK: 1 entries validated." in validated.stdout


def test_log_sorafs_drill_rejects_unknown_status_without_writing(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "done",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "--status must be one of pass, fail, follow-up, scheduled" in result.stderr
    assert not log_path.exists()


def test_log_sorafs_drill_rejects_semantically_invalid_date_and_time(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "pass",
        "--date",
        "2026-02-31",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "--date is not a valid calendar date" in result.stderr
    assert not log_path.exists()

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "pass",
        "--start",
        "24:00Z",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "--start is not a valid UTC time" in result.stderr
    assert not log_path.exists()


def test_log_sorafs_drill_rejects_symlinked_log_without_writing(
    tmp_path: Path,
) -> None:
    linked_target = tmp_path / "target.md"
    linked_target.write_text("sentinel\n", encoding="utf-8")
    log_path = tmp_path / "drill-log.md"
    log_path.symlink_to(linked_target)

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "pass",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "drill log must not be a symlink" in result.stderr
    assert linked_target.read_text(encoding="utf-8") == "sentinel\n"


def test_log_sorafs_drill_rejects_symlinked_parent_without_writing(
    tmp_path: Path,
) -> None:
    real_dir = tmp_path / "real"
    real_dir.mkdir()
    linked_parent = tmp_path / "alias"
    linked_parent.symlink_to(real_dir, target_is_directory=True)
    log_path = linked_parent / "drill-log.md"

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "pass",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "drill log parent must not be a symlink" in result.stderr
    assert not (real_dir / "drill-log.md").exists()


def test_log_sorafs_drill_rejects_parent_directory_segments(
    tmp_path: Path,
) -> None:
    nested = tmp_path / "nested"
    nested.mkdir()
    log_path = nested / ".." / "drill-log.md"

    result = run_script(
        str(LOG_SCRIPT),
        "--scenario",
        "Gateway outage",
        "--status",
        "pass",
        "--log",
        str(log_path),
    )

    assert result.returncode == 1
    assert "parent must not contain parent-directory segments" in result.stderr
    assert not (tmp_path / "drill-log.md").exists()


def test_validate_drill_log_rejects_invalid_status(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | done | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid status 'done'" in result.stderr


def test_validate_drill_log_rejects_non_canonical_status_case(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | PASS | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid status 'PASS'" in result.stderr
    assert "Allowed exact lowercase values" in result.stderr


def test_validate_drill_log_rejects_missing_separator_row(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        textwrap.dedent(
            """\
            | Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
            | 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |
            """
        ),
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "drill log separator mismatch" in result.stderr
    assert "Gateway outage" in result.stderr


def test_validate_drill_log_rejects_malformed_separator_row(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER.replace(
            "|------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|",
            "|------|----------|--------|--------------------|--------|-------------|-----------|-------|---|",
        )
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "drill log separator mismatch" in result.stderr


def test_validate_drill_log_rejects_symlinked_log(tmp_path: Path) -> None:
    linked_target = tmp_path / "target.md"
    linked_target.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )
    log_path = tmp_path / "drill-log.md"
    log_path.symlink_to(linked_target)

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "drill log must not be a symlink" in result.stderr


def test_validate_drill_log_rejects_symlinked_parent(tmp_path: Path) -> None:
    real_dir = tmp_path / "real"
    real_dir.mkdir()
    log_target = real_dir / "drill-log.md"
    log_target.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )
    linked_parent = tmp_path / "alias"
    linked_parent.symlink_to(real_dir, target_is_directory=True)

    result = validate_log(linked_parent / "drill-log.md")

    assert result.returncode == 1
    assert "drill log parent must not be a symlink" in result.stderr


def test_validate_drill_log_rejects_unescaped_pipe_columns(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway | outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "has 10 columns; expected 9" in result.stderr


def test_validate_drill_log_rejects_malformed_date_and_times(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 07/04/2026 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid date '07/04/2026'. Expected YYYY-MM-DD" in result.stderr

    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10am | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )
    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid start time '10am'. Expected HH:MMZ" in result.stderr

    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15 | notes | link |\n",
        encoding="utf-8",
    )
    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid end time '10:15'. Expected HH:MMZ or -" in result.stderr

    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-02-31 | Gateway outage | pass | IC | scribe | 10:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )
    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid date '2026-02-31'. Expected a real calendar date" in result.stderr

    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 24:00Z | 10:15Z | notes | link |\n",
        encoding="utf-8",
    )
    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid start time '24:00Z'. Expected a real UTC time" in result.stderr

    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 | Gateway outage | pass | IC | scribe | 10:00Z | 10:60Z | notes | link |\n",
        encoding="utf-8",
    )
    result = validate_log(log_path)

    assert result.returncode == 1
    assert "invalid end time '10:60Z'. Expected a real UTC time" in result.stderr


def test_validate_drill_log_rejects_empty_scenario(tmp_path: Path) -> None:
    log_path = tmp_path / "drill-log.md"
    log_path.write_text(
        DRILL_LOG_HEADER
        + "| 2026-07-04 |  | scheduled | IC | scribe | 10:00Z | - | notes | link |\n",
        encoding="utf-8",
    )

    result = validate_log(log_path)

    assert result.returncode == 1
    assert "row 1 has empty scenario" in result.stderr
