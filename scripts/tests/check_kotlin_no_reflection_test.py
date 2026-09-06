"""Exercise production reflection enforcement across the canonical JVM modules."""

from pathlib import Path
import os
import subprocess

import pytest


GUARD = Path(__file__).resolve().parents[1] / "check_kotlin_no_reflection.sh"


@pytest.mark.parametrize("module", ["core-jvm", "client-android", "kagemusha-wallet-android", "tools"])
def test_every_production_module_rejects_reflective_discovery(tmp_path: Path, module: str) -> None:
    subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True)
    source = tmp_path / "kotlin" / module / "src/main/kotlin/Probe.kt"
    source.parent.mkdir(parents=True)
    source.write_text('fun discover() = Class.forName("example.Provider")\n', encoding="utf-8")
    rejected = subprocess.run(["bash", str(GUARD)], cwd=tmp_path, capture_output=True, text=True)
    assert rejected.returncode == 1
    assert "Reflection is forbidden" in rejected.stderr

    source.write_text('fun configured(value: String) = value\n', encoding="utf-8")
    accepted = subprocess.run(["bash", str(GUARD)], cwd=tmp_path, capture_output=True, text=True)
    assert accepted.returncode == 0, accepted.stdout + accepted.stderr


def test_missing_production_sources_cannot_report_success(tmp_path: Path) -> None:
    subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True)
    result = subprocess.run(["bash", str(GUARD)], cwd=tmp_path, capture_output=True, text=True)
    assert result.returncode == 1
    assert "production source roots are missing" in result.stderr


def test_scanner_failure_cannot_report_clean_sources(tmp_path: Path) -> None:
    subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True)
    source = tmp_path / "kotlin/tools/src/main/kotlin/Probe.kt"
    source.parent.mkdir(parents=True)
    source.write_text("fun value() = 1\n", encoding="utf-8")
    commands = tmp_path / "commands"
    commands.mkdir()
    scanner = commands / "rg"
    scanner.write_text("#!/bin/sh\nexit 2\n", encoding="utf-8")
    scanner.chmod(0o755)
    environment = dict(os.environ, PATH=str(commands) + os.pathsep + os.environ["PATH"])
    result = subprocess.run(
        ["bash", str(GUARD)], cwd=tmp_path, env=environment, capture_output=True, text=True,
    )
    assert result.returncode == 2
    assert "reflection scan failed" in result.stderr
    assert "reflection-free" not in result.stdout
