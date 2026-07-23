"""Focused tests for explicit SNS annex-cycle selection."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "add_sns_annex_cycle.py"
SPEC = importlib.util.spec_from_file_location("add_sns_annex_cycle", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)  # type: ignore[attr-defined]


def test_parse_args_requires_an_explicit_suffix(capsys: pytest.CaptureFixture[str]) -> None:
    with pytest.raises(SystemExit) as captured:
        MODULE.parse_args(["2026-10"])

    assert captured.value.code == 2
    assert "--suffix" in capsys.readouterr().err


def test_parse_args_preserves_repeated_explicit_suffixes() -> None:
    context = MODULE.parse_args(
        ["2026-10", "--suffix", ".example", "--suffix", ".community", "--dry-run"]
    )

    assert context.cycle == "2026-10"
    assert context.suffixes == (".example", ".community")
    assert context.dry_run is True


def test_parse_args_rejects_suffix_without_leading_dot(
    capsys: pytest.CaptureFixture[str],
) -> None:
    with pytest.raises(SystemExit) as captured:
        MODULE.parse_args(["2026-10", "--suffix", "example"])

    assert captured.value.code == 2
    assert "leading dot" in capsys.readouterr().err


@pytest.mark.parametrize(
    "suffix", ["../escape", ".../escape", ".UPPER", ".bad-name", " .example"]
)
def test_parse_args_rejects_noncanonical_or_traversing_suffix(
    suffix: str, capsys: pytest.CaptureFixture[str]
) -> None:
    with pytest.raises(SystemExit) as captured:
        MODULE.parse_args(["2026-10", "--suffix", suffix])

    assert captured.value.code == 2
    assert "suffix values must" in capsys.readouterr().err


def test_parse_args_rejects_duplicate_suffixes(
    capsys: pytest.CaptureFixture[str],
) -> None:
    with pytest.raises(SystemExit) as captured:
        MODULE.parse_args(
            ["2026-10", "--suffix", ".example", "--suffix", ".example"]
        )

    assert captured.value.code == 2
    assert "must be unique" in capsys.readouterr().err
