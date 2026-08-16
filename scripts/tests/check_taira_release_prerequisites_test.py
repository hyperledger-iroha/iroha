"""Tests for the Taira release source-prerequisite gate."""

from __future__ import annotations

import ast
import json
import subprocess
import sys
from pathlib import Path

from scripts import check_taira_release_prerequisites as readiness

REPO_ROOT = Path(__file__).resolve().parents[2]


def _function(source: str) -> ast.FunctionDef:
    node = ast.parse(source).body[0]
    assert isinstance(node, ast.FunctionDef)
    return node


def test_unconditional_refusal_detection_is_narrow() -> None:
    """Only a docstring/input discard followed by raise or fail is blocked."""

    assert readiness.is_unconditional_refusal(
        _function(
            'def gate(value):\n    """closed"""\n    del value\n    _fail("no")\n'
        )
    )
    assert readiness.is_unconditional_refusal(
        _function('def gate():\n    raise RuntimeError("no")\n')
    )
    assert not readiness.is_unconditional_refusal(
        _function(
            'def gate(authority):\n    if not authority:\n        fail("no")\n    return authority\n'
        )
    )


def test_current_release_barriers_are_reported_before_build() -> None:
    """The gate names every intentionally unprovisioned critical-path stage."""

    reports = readiness.unresolved_prerequisites(REPO_ROOT)
    assert [report["stage"] for report in reports] == [
        prerequisite.stage for prerequisite in readiness.PREREQUISITES
    ]
    assert all(int(report["line"]) > 0 for report in reports)


def test_cli_is_machine_readable_and_fail_closed() -> None:
    """An unresolved repository returns nonzero JSON rather than starting Cargo."""

    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts/check_taira_release_prerequisites.py"),
            "--repository",
            str(REPO_ROOT),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    payload = json.loads(result.stdout)
    assert result.returncode == 1
    assert payload["ready"] is False
    assert len(payload["unresolved"]) == len(readiness.PREREQUISITES)
