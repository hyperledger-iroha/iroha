"""Execute the native release helper with a fake Cargo; no Rust build is performed."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from scripts import check_sorafs_release_automation as automation


REPO_ROOT = Path(__file__).resolve().parents[2]


def _run_helper(tmp_path: Path, mode: str) -> tuple[subprocess.CompletedProcess, list]:
    """Record both Cargo argument vectors and simulate bounded collection/execution outcomes."""
    executable = tmp_path / "cargo"
    executable.write_text(
        f"#!{sys.executable}\n"
        "import json, os, sys\n"
        "from pathlib import Path\n"
        "with Path(os.environ['NATIVE_TEST_CALLS']).open('a') as output:\n"
        "    output.write(json.dumps(sys.argv[1:]) + '\\n')\n"
        f"sentinels = {automation.SORAFS_NATIVE_AUTHORITY_SENTINELS!r}\n"
        "mode = os.environ['NATIVE_TEST_MODE']\n"
        "if '--list' in sys.argv:\n"
        "    if mode == 'listing_fails': sys.exit(2)\n"
        "    selected = list(sentinels)\n"
        "    if mode == 'empty': selected = []\n"
        "    if mode == 'missing': selected.pop(0)\n"
        "    if mode == 'duplicate': selected.append(selected[0])\n"
        "    for name in selected: print(name + ': test')\n"
        "elif mode == 'run_fails':\n"
        "    sys.exit(3)\n",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    log = tmp_path / "cargo-calls.jsonl"
    environment = {
        **os.environ,
        "PATH": str(tmp_path) + os.pathsep + os.defpath,
        "NATIVE_TEST_CALLS": str(log),
        "NATIVE_TEST_MODE": mode,
    }
    result = subprocess.run(
        ["bash", str(REPO_ROOT / automation.SORAFS_NATIVE_AUTHORITY_RUNTIME_SCRIPT)],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    return result, [json.loads(line) for line in log.read_text().splitlines()]


def test_native_authority_runtime_uses_identical_graph_and_filters_without_ignored_skips(tmp_path):
    result, calls = _run_helper(tmp_path, "success")
    assert result.returncode == 0, result.stderr
    common = ["test", "--locked"]
    for package in automation.SORAFS_NATIVE_AUTHORITY_PACKAGES:
        common.extend(("-p", package))
    common.extend(("--lib", "--", *automation.SORAFS_NATIVE_AUTHORITY_FILTERS))
    assert calls == [common + ["--list"], common + ["--include-ignored", "--nocapture"]]


@pytest.mark.parametrize("mode", ("empty", "missing", "duplicate", "listing_fails"))
def test_native_authority_runtime_rejects_incomplete_collection_before_execution(tmp_path, mode):
    result, calls = _run_helper(tmp_path, mode)
    assert result.returncode != 0
    assert len(calls) == 1
    assert calls[0][-1] == "--list"


def test_native_authority_runtime_propagates_execution_failure(tmp_path):
    result, calls = _run_helper(tmp_path, "run_fails")
    assert result.returncode == 3
    assert len(calls) == 2
    assert calls[1][-2:] == ["--include-ignored", "--nocapture"]
