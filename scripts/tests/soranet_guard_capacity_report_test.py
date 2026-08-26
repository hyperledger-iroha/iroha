"""Regressions for the SoraNet guard-capacity report helper."""

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).parents[1] / "soranet_guard_capacity_report.py"
SPEC = importlib.util.spec_from_file_location("soranet_guard_capacity_report", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


def metrics_for_mode(mode: str) -> str:
    return "\n".join(
        f'{metric}{{mode="{mode}"}} 0' for metric in REPORT.METRIC_KEYS.values()
    )


def test_report_is_directly_executable(tmp_path: Path) -> None:
    metrics = tmp_path / "metrics.txt"
    metrics.write_text(metrics_for_mode("entry"), encoding="utf-8")

    completed = subprocess.run(
        [MODULE_PATH, metrics, "--mode", "entry"],
        check=True,
        capture_output=True,
        text=True,
    )

    assert "Mode: entry" in completed.stdout


def test_cli_rejects_scrape_without_requested_mode(tmp_path: Path) -> None:
    metrics = tmp_path / "metrics.txt"
    metrics.write_text(metrics_for_mode("entry"), encoding="utf-8")

    completed = subprocess.run(
        [MODULE_PATH, metrics, "--mode", "exit"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 2
    assert "metrics for mode 'exit' are missing required series" in completed.stderr


def test_recommendation_does_not_reduce_cooldown_below_one() -> None:
    metrics = REPORT.Metrics(success=10, throttled=1)

    assert REPORT.recommendation(metrics, 8, 1) == (
        "handshake_cooldown_millis is already at the minimum 1; "
        "investigate other throttling causes"
    )


@pytest.mark.parametrize(
    ("arguments", "message"),
    [
        (["--mode", "entyr"], "invalid choice"),
        (["--max-circuits", "0"], "--max-circuits must be greater than zero"),
        (
            ["--handshake-cooldown", "-1"],
            "--handshake-cooldown must be greater than zero",
        ),
    ],
)
def test_cli_rejects_invalid_operator_inputs(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    arguments: list[str],
    message: str,
) -> None:
    metrics = tmp_path / "metrics.txt"
    metrics.write_text("", encoding="utf-8")
    monkeypatch.setattr(sys, "argv", [str(MODULE_PATH), str(metrics), *arguments])

    with pytest.raises(SystemExit) as error:
        REPORT.main()

    assert error.value.code == 2
    assert message in capsys.readouterr().err


@pytest.mark.parametrize(("current_limit", "current_cooldown"), [(0, 1), (1, 0)])
def test_recommendation_rejects_nonpositive_settings(
    current_limit: int, current_cooldown: int
) -> None:
    with pytest.raises(ValueError, match="must be greater than zero"):
        REPORT.recommendation(REPORT.Metrics(), current_limit, current_cooldown)
