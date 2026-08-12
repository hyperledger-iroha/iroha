"""Tests for the sealed Kotodama compiler test-source inventory."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_kotodama_test_sources.py"
MANIFEST = ROOT / "crates" / "kotodama_lang" / "kotodama_fixtures_v1.manifest.json"


def _load_checker():
    spec = importlib.util.spec_from_file_location("check_kotodama_test_sources", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = _load_checker()


def _copy_fixture_tree(destination: Path) -> Path:
    payload = json.loads(MANIFEST.read_text(encoding="utf-8"))
    relative_manifest = MANIFEST.relative_to(ROOT)
    copied_manifest = destination / relative_manifest
    copied_manifest.parent.mkdir(parents=True)
    shutil.copy2(MANIFEST, copied_manifest)
    for source in payload["source_files"]:
        source_path = ROOT / source["path"]
        copied_source = destination / source["path"]
        copied_source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, copied_source)
    for fixture in payload["fixtures"]:
        asset = ROOT / fixture["asset"]
        copied_asset = destination / fixture["asset"]
        copied_asset.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(asset, copied_asset)
    return copied_manifest


def test_checked_in_inventory_is_complete_and_reconstructable() -> None:
    stats = checker.validate_manifest(ROOT, MANIFEST)
    assert stats.fixtures == 248
    assert stats.retained_templates == 60
    assert stats.tests > 500


def test_payload_corruption_fails_closed(tmp_path: Path) -> None:
    copied_manifest = _copy_fixture_tree(tmp_path)
    payload = json.loads(copied_manifest.read_text(encoding="utf-8"))
    asset = tmp_path / payload["fixtures"][0]["asset"]
    asset.write_bytes(asset.read_bytes() + b"\n")

    with pytest.raises(checker.ValidationError, match="byte length changed"):
        checker.validate_manifest(tmp_path, copied_manifest)


def test_unknown_manifest_key_fails_closed(tmp_path: Path) -> None:
    copied_manifest = _copy_fixture_tree(tmp_path)
    payload = json.loads(copied_manifest.read_text(encoding="utf-8"))
    payload["unexpected"] = True
    copied_manifest.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(checker.ValidationError, match="unknown=.*unexpected"):
        checker.validate_manifest(tmp_path, copied_manifest)


def test_repository_paths_cannot_escape_the_root() -> None:
    with pytest.raises(checker.ValidationError, match="repository-relative"):
        checker._relative_path("../escape.ko", "test.path")
