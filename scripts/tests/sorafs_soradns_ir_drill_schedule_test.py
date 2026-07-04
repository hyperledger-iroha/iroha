"""Tests for the SoraDNS IR drill scheduler used by SoraFS drill evidence."""

from __future__ import annotations

import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEDULE_SCRIPT = REPO_ROOT / "scripts" / "telemetry" / "schedule_soradns_ir_drill.sh"
VALIDATE_SCRIPT = REPO_ROOT / "scripts" / "telemetry" / "validate_drill_log.sh"


def run_scheduler(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the scheduler helper and return its completed process."""
    return subprocess.run(
        ["bash", str(SCHEDULE_SCRIPT), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def validate_log(path: Path) -> subprocess.CompletedProcess[str]:
    """Run the drill-log validator for `path`."""
    return subprocess.run(
        ["bash", str(VALIDATE_SCRIPT), str(path)],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def test_schedule_soradns_ir_drill_writes_valid_scheduled_entry(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_scheduler(
        "--date",
        "2026-07-02",
        "--start",
        "14:30Z",
        "--ic",
        "Transparency IC",
        "--scribe",
        "Ops Scribe",
        "--notes",
        "review transparency drill cadence",
        "--log",
        str(log_path),
    )

    assert result.returncode == 0, result.stderr
    assert "SoraDNS transparency IR drill (Q3 2026)" in result.stdout
    source = log_path.read_text(encoding="utf-8")
    assert "| 2026-07-02 | SoraDNS transparency IR drill (Q3 2026) | scheduled |" in source
    assert "| Transparency IC | Ops Scribe | 14:30Z | - |" in source

    validated = validate_log(log_path)
    assert validated.returncode == 0, validated.stderr
    assert "Drill log OK: 1 entries validated." in validated.stdout


def test_schedule_soradns_ir_drill_rejects_invalid_date_without_traceback(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_scheduler("--date", "2026-02-31", "--log", str(log_path))

    assert result.returncode == 1
    assert "--date is not a valid calendar date" in result.stderr
    assert "Traceback" not in result.stderr
    assert not log_path.exists()


def test_schedule_soradns_ir_drill_rejects_invalid_start_without_writing(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_scheduler("--start", "24:00Z", "--log", str(log_path))

    assert result.returncode == 1
    assert "--start is not a valid UTC time" in result.stderr
    assert "Traceback" not in result.stderr
    assert not log_path.exists()


def test_schedule_soradns_ir_drill_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "drill-log.md"

    result = run_scheduler("--date", "--log", str(log_path))

    assert result.returncode == 1
    assert "--date requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert "Traceback" not in result.stderr
    assert not log_path.exists()

    result = run_scheduler("--date")

    assert result.returncode == 1
    assert "--date requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert "Traceback" not in result.stderr
