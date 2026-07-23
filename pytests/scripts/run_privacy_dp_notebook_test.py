"""Regression tests for the privacy-DP notebook wrapper."""

import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
WRAPPER = REPO_ROOT / "scripts/telemetry/run_privacy_dp_notebook.sh"


def test_privacy_dp_notebook_wrapper_has_valid_bash_syntax() -> None:
    """Keep the non-interactive notebook entry point parseable by Bash."""

    result = subprocess.run(
        ["bash", "-n", str(WRAPPER)],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
