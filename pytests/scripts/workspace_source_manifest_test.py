"""Tests for the release source-manifest helper."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "compute_workspace_source_manifest.py"


def load_module():
    spec = importlib.util.spec_from_file_location("workspace_source_manifest", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_manifest_is_order_independent_and_content_sensitive(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "a.txt").write_text("alpha\n", encoding="utf-8")
    (tmp_path / "b.txt").write_text("beta\n", encoding="utf-8")

    first = module._manifest_for_paths(tmp_path, ["b.txt", "a.txt"])
    assert first == module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])

    (tmp_path / "a.txt").write_text("changed\n", encoding="utf-8")
    assert first != module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])


def test_manifest_distinguishes_deleted_and_symlink_entries(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "target-a").write_text("same\n", encoding="utf-8")
    (tmp_path / "target-b").write_text("same\n", encoding="utf-8")
    (tmp_path / "link").symlink_to("target-a")

    first = module._manifest_for_paths(tmp_path, ["link", "missing"])
    (tmp_path / "link").unlink()
    (tmp_path / "link").symlink_to("target-b")
    second = module._manifest_for_paths(tmp_path, ["link", "missing"])
    assert first != second

    (tmp_path / "missing").write_text("now present\n", encoding="utf-8")
    assert second != module._manifest_for_paths(tmp_path, ["link", "missing"])


def test_manifest_tracks_executable_mode(tmp_path: Path) -> None:
    module = load_module()
    script = tmp_path / "gate.sh"
    script.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    script.chmod(0o644)
    regular = module._manifest_for_paths(tmp_path, ["gate.sh"])
    script.chmod(0o755)
    executable = module._manifest_for_paths(tmp_path, ["gate.sh"])
    assert regular != executable
    assert os.access(script, os.X_OK)
