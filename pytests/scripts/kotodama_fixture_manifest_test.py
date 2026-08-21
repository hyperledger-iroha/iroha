"""Tests for the sealed Kotodama compiler test-source inventories."""

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
LEGACY_MANIFEST = (
    ROOT
    / "crates"
    / "kotodama_lang"
    / "kotodama_legacy_test_sources_v1.manifest.json"
)


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
    legacy_payload = json.loads(LEGACY_MANIFEST.read_text(encoding="utf-8"))
    relative_manifest = MANIFEST.relative_to(ROOT)
    copied_manifest = destination / relative_manifest
    copied_manifest.parent.mkdir(parents=True)
    shutil.copy2(MANIFEST, copied_manifest)
    copied_legacy_manifest = destination / LEGACY_MANIFEST.relative_to(ROOT)
    shutil.copy2(LEGACY_MANIFEST, copied_legacy_manifest)
    for source in payload["source_files"]:
        source_path = ROOT / source["path"]
        copied_source = destination / source["path"]
        copied_source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, copied_source)
    for source_path, includes in checker.EXPECTED_TEST_INCLUDES.items():
        source_parent = Path(source_path).parent
        for include in includes:
            included_source = source_parent / include
            copied_source = destination / included_source
            copied_source.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / included_source, copied_source)
    for fixture in payload["fixtures"]:
        asset = ROOT / fixture["asset"]
        copied_asset = destination / fixture["asset"]
        copied_asset.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(asset, copied_asset)
    for source in legacy_payload["source_files"]:
        source_path = ROOT / source
        copied_source = destination / source
        copied_source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, copied_source)
    for fixture in legacy_payload["fixtures"]:
        asset = ROOT / fixture["path"]
        copied_asset = destination / fixture["path"]
        copied_asset.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(asset, copied_asset)
    return copied_manifest


def test_checked_in_inventory_is_complete_and_reconstructable() -> None:
    stats = checker.validate_manifest(ROOT, MANIFEST)
    assert stats.fixtures == 248
    assert stats.legacy_fixtures == 52
    assert stats.retained_templates == 53
    assert stats.tests == 563


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


def test_included_test_inventory_drift_fails_closed(tmp_path: Path) -> None:
    copied_manifest = _copy_fixture_tree(tmp_path)
    included_source = (
        tmp_path
        / "crates/kotodama_lang/src/compiler/tests/axt_remote_spend_access_tests.rs"
    )
    source = included_source.read_text(encoding="utf-8")
    included_source.write_text(
        source.replace(
            "fn codegen_rejects_noncanonical_or_invalid_literal_remote_spend_intents()",
            "fn changed_remote_spend_test_name()",
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(checker.ValidationError, match="test name/order inventory changed"):
        checker.validate_manifest(tmp_path, copied_manifest)


def test_legacy_payload_corruption_fails_closed(tmp_path: Path) -> None:
    copied_manifest = _copy_fixture_tree(tmp_path)
    legacy_manifest = tmp_path / LEGACY_MANIFEST.relative_to(ROOT)
    payload = json.loads(legacy_manifest.read_text(encoding="utf-8"))
    asset = tmp_path / payload["fixtures"][0]["path"]
    asset.write_bytes(asset.read_bytes() + b"\n")

    with pytest.raises(checker.ValidationError, match="byte length changed"):
        checker.validate_manifest(tmp_path, copied_manifest)


def test_legacy_payload_omission_fails_closed(tmp_path: Path) -> None:
    copied_manifest = _copy_fixture_tree(tmp_path)
    legacy_manifest = tmp_path / LEGACY_MANIFEST.relative_to(ROOT)
    payload = json.loads(legacy_manifest.read_text(encoding="utf-8"))
    asset = tmp_path / payload["fixtures"][0]["path"]
    asset.unlink()

    with pytest.raises(checker.ValidationError, match="legacy fixture is missing"):
        checker.validate_manifest(tmp_path, copied_manifest)


def test_repository_paths_cannot_escape_the_root() -> None:
    with pytest.raises(checker.ValidationError, match="repository-relative"):
        checker._relative_path("../escape.ko", "test.path")
