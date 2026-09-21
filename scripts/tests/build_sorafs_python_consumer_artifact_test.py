"""Parent entry-point refusals; no fabricated clean candidate or native execution."""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import build_sorafs_python_consumer_artifact as producer


def test_dirty_candidate_refuses_before_creating_or_executing(tmp_path, monkeypatch):
    work = ROOT / "target" / ("python-producer-uncreated-" + tmp_path.name)
    args = argparse.Namespace(source_root=ROOT, work_dir=work)
    monkeypatch.setattr(producer.native, "source_state", lambda _root: ("a" * 40, False))
    def unexpected(*_args, **_kwargs):
        raise AssertionError("dirty candidate must not execute any child")
    monkeypatch.setattr(producer, "run_python_process", unexpected)
    with pytest.raises(producer.ArtifactError, match="clean immutable"):
        producer.produce(args)
    assert not work.exists()


def test_existing_output_and_foreign_checkout_are_never_replaced(tmp_path):
    marker = tmp_path / "marker"; marker.write_bytes(b"original")
    for root, work in ((ROOT, tmp_path), (tmp_path, tmp_path / "new")):
        with pytest.raises(producer.ArtifactError):
            producer.produce(argparse.Namespace(source_root=root, work_dir=work))
    assert marker.read_bytes() == b"original"


def test_isolated_actual_cli_help_and_closed_arguments(tmp_path):
    command = [sys.executable, "-I", "-B", str(ROOT / "scripts/build_sorafs_python_consumer_artifact.py")]
    result = subprocess.run(command + ["--help"], capture_output=True, timeout=30)
    assert result.returncode == 0 and not result.stderr
    assert b"--runtime-manifest-sha256" in result.stdout
    assert b"--source-commit" in result.stdout and b"--source-manifest-sha256" in result.stdout
    result = subprocess.run(command + ["--execute-command", "true"], capture_output=True, timeout=30)
    assert result.returncode != 0 and b"usage:" in result.stderr
