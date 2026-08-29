"""Regression test for audited panic recovery boundaries."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def test_panic_recovery_boundary_guard() -> None:
    root = Path(__file__).resolve().parents[2]
    completed = subprocess.run(
        [sys.executable, str(root / "scripts/check_panic_recovery_boundaries.py")],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
