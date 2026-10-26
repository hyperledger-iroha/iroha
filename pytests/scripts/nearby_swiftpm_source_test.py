"""Protect the pinned Swift Nearby closure and its fail-closed source verifier."""

from pathlib import Path
import runpy
import shutil
import subprocess
import sys

import pytest


PACKAGE = Path(__file__).resolve().parents[2] / "vendor" / "nearby"
VERIFIER = runpy.run_path(str(PACKAGE / "verify_source.py"))


def test_current_upstream_closure_is_exact():
    assert VERIFIER["validate"](PACKAGE) == 1416


@pytest.mark.parametrize("path", ["connections/core.cc", "LICENSE", "upstream/Package.swift"])
def test_normal_relative_paths_are_accepted(path):
    assert VERIFIER["relative_path"](path) == path


@pytest.mark.parametrize("path", ["", ".", "/tmp/a", "../a", "a/../b", "a//b", "a/./b", "a\\b", "a\x00b", "C:/a"])
def test_ambiguous_or_escaping_paths_are_rejected(path):
    with pytest.raises(ValueError):
        VERIFIER["relative_path"](path)


@pytest.mark.parametrize("mutation", ["source", "missing", "extra", "metadata", "manifest", "owned", "file_link", "directory_link"])
def test_package_mutations_fail_closed(tmp_path, mutation):
    package = tmp_path / "nearby"
    shutil.copytree(
        PACKAGE,
        package,
        ignore=shutil.ignore_patterns(".build", ".swiftpm", "__pycache__"),
    )
    source = package / "connections/core.cc"
    if mutation == "source":
        source.write_bytes(source.read_bytes() + b"\n// altered\n")
    elif mutation == "missing":
        source.unlink()
    elif mutation == "extra":
        (package / "connections/unreviewed.cc").write_text("// unexpected source\n")
    elif mutation == "metadata":
        path = package / "UPSTREAM.json"
        path.write_bytes(path.read_bytes() + b"\n")
    elif mutation == "manifest":
        path = package / "Package.swift"
        path.write_bytes(path.read_bytes() + b"\n// changed packaging\n")
    elif mutation == "owned":
        (package / "IROHA-PATCH.md").unlink()
    elif mutation == "file_link":
        source.unlink()
        source.symlink_to(PACKAGE / "connections/core.cc")
    elif mutation == "directory_link":
        (package / ".build").symlink_to(tmp_path, target_is_directory=True)
    with pytest.raises((ValueError, OSError)):
        VERIFIER["validate"](package)


def test_command_line_reports_success_and_failure(tmp_path):
    command = [sys.executable, str(PACKAGE / "verify_source.py")]
    success = subprocess.run(command, capture_output=True, text=True)
    assert success.returncode == 0
    assert "1416 exact upstream files" in success.stdout
    failure = subprocess.run(command + ["--root", str(tmp_path)], capture_output=True, text=True)
    assert failure.returncode == 1
    assert "source verification failed" in failure.stderr
