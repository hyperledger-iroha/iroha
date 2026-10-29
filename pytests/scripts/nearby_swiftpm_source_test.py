"""Protect the pinned Swift Nearby closure and its fail-closed source verifier."""

from pathlib import Path
import json
import runpy
import shutil
import subprocess
import sys

import pytest


PACKAGE = Path(__file__).resolve().parents[2] / "vendor" / "nearby"
VERIFIER = runpy.run_path(str(PACKAGE / "verify_source.py"))


def test_current_reviewed_closure_is_exact():
    assert VERIFIER["validate"](PACKAGE) == 1416


def test_timestamp_patches_preserve_original_upstream_identities():
    records = json.loads((PACKAGE / "UPSTREAM.json").read_text())["files"]
    originals = {
        "connections/implementation/internal_payload_factory.cc":
            "fcbf77c0fe9ce983b4a1aeaea22ed02100e455eb",
        "internal/platform/implementation/shared/file.cc":
            "1c798269df4455b548d594341424d5d11e497bd6",
    }
    assert {name for name, value in records.items() if "local_patch" in value} == set(originals)
    for name, upstream_blob in originals.items():
        assert records[name]["git_blob"] == upstream_blob
        assert records[name]["local_patch"]["git_blob"] != upstream_blob
        assert records[name]["local_patch"]["sha256"] != records[name]["sha256"]
        assert records[name]["local_patch"]["reason"]


@pytest.mark.parametrize("path", ["connections/core.cc", "LICENSE", "upstream/Package.swift"])
def test_normal_relative_paths_are_accepted(path):
    assert VERIFIER["relative_path"](path) == path


@pytest.mark.parametrize("path", ["", ".", "/tmp/a", "../a", "a/../b", "a//b", "a/./b", "a\\b", "a\x00b", "C:/a"])
def test_ambiguous_or_escaping_paths_are_rejected(path):
    with pytest.raises(ValueError):
        VERIFIER["relative_path"](path)


@pytest.mark.parametrize("mutation", ["source", "patched_source", "missing", "extra", "metadata", "manifest", "owned", "file_link", "directory_link"])
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
    elif mutation == "patched_source":
        source = package / "internal/platform/implementation/shared/file.cc"
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
    assert "1416 reviewed files" in success.stdout
    failure = subprocess.run(command + ["--root", str(tmp_path)], capture_output=True, text=True)
    assert failure.returncode == 1
    assert "source verification failed" in failure.stderr
