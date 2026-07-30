"""Regression tests for maintained shell entrypoints."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SHELL_ENTRYPOINTS = (
    REPO_ROOT / "examples/ios/NoritoDemoXcode/Scripts/generate-keys.sh",
)


@pytest.mark.parametrize("script", SHELL_ENTRYPOINTS)
def test_shell_entrypoint_parses(script: Path) -> None:
    """Keep malformed here-documents and control-flow terminators out of scripts."""

    subprocess.run(["bash", "-n", str(script)], check=True)


def test_key_generator_help_does_not_invoke_cargo() -> None:
    """The iOS demo key helper should expose help without requiring the toolchain."""

    result = subprocess.run(
        ["bash", str(SHELL_ENTRYPOINTS[0]), "--help"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0
    assert "Usage: generate-keys.sh <account-name>" in result.stdout
