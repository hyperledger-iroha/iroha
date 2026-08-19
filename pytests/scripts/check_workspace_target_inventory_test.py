"""Tests for the ordinary-workspace target inventory guard."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_workspace_target_inventory.py"
SPEC = importlib.util.spec_from_file_location("check_workspace_target_inventory", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
TARGET_INVENTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TARGET_INVENTORY)


def test_repository_target_inventory() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)

    assert TARGET_INVENTORY.check_metadata(metadata) == []


def test_musubi_fixture_owner_is_declared_but_never_default() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    target = ("iroha_data_model", "musubi_fixtures")

    assert TARGET_INVENTORY.EXPECTED_DECLARED_BIN_COUNT == 114
    assert target in TARGET_INVENTORY.all_workspace_bins(metadata)
    assert target not in TARGET_INVENTORY.resolved_default_bins(metadata)


def test_external_software_signer_is_declared_but_never_default() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    target = ("irohad", "sorafs_external_software_signer")

    assert target in TARGET_INVENTORY.all_workspace_bins(metadata)
    assert target not in TARGET_INVENTORY.resolved_default_bins(metadata)


def test_external_software_signer_requires_explicit_release_opt_in() -> None:
    manifest = tomllib.loads(
        (ROOT / "crates" / "irohad" / "Cargo.toml").read_text(encoding="utf-8")
    )
    marker = "external-software-signer-bin"
    signer = next(
        target
        for target in manifest["bin"]
        if target["name"] == "sorafs_external_software_signer"
    )

    assert manifest["features"][marker] == ["daemon"]
    assert marker not in manifest["features"]["default"]
    assert signer["required-features"] == [marker]

    caller_markers = {
        "scripts/build_canonical_binaries.sh": "--features external-software-signer-bin",
        "ci/check_sorafs_cli_release.sh": "--features external-software-signer-bin",
        ".github/workflows/sorafs-cli-release.yml": (
            "--features external-software-signer-bin"
        ),
        ".github/workflows/publish_taira_validator.yml": (
            "embedded-soracloud-runtime,external-software-signer-bin,zk-stark"
        ),
        "scripts/build_release_bundle.sh": (
            "--features irohad/external-software-signer-bin"
        ),
        "configs/soranexus/taira/build_taira_rollout_bundle.sh": (
            "embedded-soracloud-runtime,external-software-signer-bin,zk-stark"
        ),
        "Dockerfile": 'ARG FEATURES="external-software-signer-bin"',
    }
    for relative, expected in caller_markers.items():
        assert expected in (ROOT / relative).read_text(encoding="utf-8")


def test_rejects_default_tool_and_retired_alias() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(
        package for package in modified["packages"] if package["name"] == "iroha_cli"
    )
    package["targets"].extend(
        [
            {
                "kind": ["bin"],
                "name": "fixture_refresher",
                "required-features": [],
            },
            {
                "kind": ["bin"],
                "name": "iroha3",
                "required-features": ["dev-tools"],
            },
        ]
    )

    errors = TARGET_INVENTORY.check_metadata(modified)

    assert any("non-shipping binaries enabled by default" in error for error in errors)
    assert any("declared binary count" in error for error in errors)
    assert any("retired compatibility binaries are declared" in error for error in errors)


def test_rejects_removed_developer_tool() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(
        package for package in modified["packages"] if package["name"] == "ivm"
    )
    package["targets"] = [
        target
        for target in package["targets"]
        if target["name"] != "ivm_fixture_export"
    ]

    errors = TARGET_INVENTORY.check_metadata(modified)

    assert any("declared binary count" in error for error in errors)


def test_rejects_retired_irohad_daemon_alias() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(
        package for package in modified["packages"] if package["name"] == "irohad"
    )
    daemon = next(target for target in package["targets"] if target["name"] == "iroha3d")
    daemon["name"] = "irohad"

    errors = TARGET_INVENTORY.check_metadata(modified)

    assert any("shipping binaries no longer enabled by default" in error for error in errors)
    assert any("non-shipping binaries enabled by default" in error for error in errors)
    assert any("retired compatibility binaries are declared" in error for error in errors)
