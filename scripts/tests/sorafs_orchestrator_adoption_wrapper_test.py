"""Tests for the SoraFS orchestrator adoption wrapper."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "ci" / "check_sorafs_orchestrator_adoption.sh"


def run_wrapper(extra_env: dict[str, str | None]) -> subprocess.CompletedProcess[str]:
    """Run the adoption wrapper until early preflight exits."""

    env = os.environ.copy()
    for key in tuple(env):
        if key.startswith("SORAFS_") or key == "XTASK_SORAFS_ADOPTION_FLAGS":
            env.pop(key, None)
    for key, value in extra_env.items():
        if value is None:
            env.pop(key, None)
        else:
            env[key] = value

    return subprocess.run(
        ["bash", str(SCRIPT)],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def test_adoption_wrapper_rejects_relaxing_flags_without_override_id() -> None:
    result = run_wrapper(
        {
            "XTASK_SORAFS_ADOPTION_FLAGS": "--allow-single-source --require-direct-only",
            "SORAFS_ADOPTION_OVERRIDE_ID": None,
        }
    )

    assert result.returncode == 1
    assert (
        "relaxing XTASK_SORAFS_ADOPTION_FLAGS require "
        "SORAFS_ADOPTION_OVERRIDE_ID"
    ) in result.stderr
    assert "running orchestrator fixture" not in result.stdout
    assert "running orchestrator fixture" not in result.stderr


def test_adoption_wrapper_rejects_malformed_override_id() -> None:
    result = run_wrapper(
        {
            "XTASK_SORAFS_ADOPTION_FLAGS": "--allow-zero-weight",
            "SORAFS_ADOPTION_OVERRIDE_ID": "bad id",
        }
    )

    assert result.returncode == 1
    assert (
        "SORAFS_ADOPTION_OVERRIDE_ID must be 3-128 characters" in result.stderr
    )
    assert "running orchestrator fixture" not in result.stdout
    assert "running orchestrator fixture" not in result.stderr
