"""Regression tests for shell entrypoints used by documentation and examples."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SHELL_ENTRYPOINTS = (
    REPO_ROOT / "docs/portal/scripts/sorafs-package-preview.sh",
    REPO_ROOT / "examples/ios/NoritoDemoXcode/Scripts/generate-keys.sh",
)


@pytest.mark.parametrize("script", SHELL_ENTRYPOINTS)
def test_shell_entrypoint_parses(script: Path) -> None:
    """Keep malformed here-documents and control-flow terminators out of scripts."""

    subprocess.run(["bash", "-n", str(script)], check=True)


def test_key_generator_help_does_not_invoke_cargo() -> None:
    """The iOS demo key helper should expose help without requiring the toolchain."""

    result = subprocess.run(
        ["bash", str(SHELL_ENTRYPOINTS[1]), "--help"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0
    assert "Usage: generate-keys.sh <account-name>" in result.stdout


def test_sorafs_preview_rejects_missing_config_path() -> None:
    """A missing option value should produce a stable usage error, not nounset."""

    result = subprocess.run(
        ["bash", str(SHELL_ENTRYPOINTS[0]), "--config"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 2
    assert result.stderr.strip() == "--config requires a path"


@pytest.mark.skipif(shutil.which("node") is None, reason="Node.js is unavailable")
def test_sorafs_preview_reads_json_config(tmp_path: Path) -> None:
    """JSON config values should reach the manifest submission invocation."""

    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    (artifact_dir / "preview-site.tar.gz").touch()
    (artifact_dir / "checksums.sha256").touch()

    config = tmp_path / "preview.json"
    config.write_text(
        json.dumps(
            {
                "torii_url": "https://preview.example",
                "authority": "alice@preview",
                "private_key": "dummy-private-key",
                "submitted_epoch": 17,
            }
        ),
        encoding="utf-8",
    )

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    cargo_log = tmp_path / "cargo.log"
    fake_cargo = fake_bin / "cargo"
    fake_cargo.write_text(
        '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "${CARGO_LOG:?}"\n',
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)

    env = os.environ.copy()
    env.update(
        {
            "ARTIFACT_DIR": str(artifact_dir),
            "CARGO_LOG": str(cargo_log),
            "PATH": f"{fake_bin}{os.pathsep}{env['PATH']}",
        }
    )
    result = subprocess.run(
        ["bash", str(SHELL_ENTRYPOINTS[0]), "--config", str(config)],
        check=False,
        capture_output=True,
        text=True,
        env=env,
    )

    assert result.returncode == 0, result.stderr
    invocations = cargo_log.read_text(encoding="utf-8").splitlines()
    assert len(invocations) == 3
    submit = invocations[-1]
    assert "--torii-url=https://preview.example" in submit
    assert "--authority=alice@preview" in submit
    assert "--private-key=dummy-private-key" in submit
    assert "--submitted-epoch=17" in submit
