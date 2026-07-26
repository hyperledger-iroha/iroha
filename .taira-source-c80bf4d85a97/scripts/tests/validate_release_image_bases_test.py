from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "validate_release_image_bases.py"


def run(builder: str, runtime: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--builder",
            builder,
            "--runtime",
            runtime,
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def test_accepts_exact_digest_refs() -> None:
    result = run(
        f"registry.example/builder@sha256:{'a' * 64}",
        f"registry.example/runtime@sha256:{'b' * 64}",
    )
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize(
    "mutable",
    (
        "",
        "rust:slim-bookworm",
        "registry.example/builder:latest",
        f"Registry.example/builder@sha256:{'a' * 64}",
        f"registry.example/builder@sha256:{'A' * 64}",
    ),
)
def test_rejects_missing_or_mutable_refs(mutable: str) -> None:
    result = run(
        mutable,
        f"registry.example/runtime@sha256:{'b' * 64}",
    )
    assert result.returncode != 0
    assert "builder base image" in result.stderr
