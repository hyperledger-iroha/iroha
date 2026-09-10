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

    assert TARGET_INVENTORY.EXPECTED_DECLARED_BIN_COUNT == 103
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
        "scripts/build_canonical_binaries.sh": (
            "--features irohad/external-software-signer-bin,iroha_cli/cli"
        ),
        "ci/check_sorafs_cli_release.sh": "--features external-software-signer-bin",
        ".github/workflows/sorafs-cli-release.yml": (
            "--features external-software-signer-bin"
        ),
        "Dockerfile": 'ARG FEATURES="external-software-signer-bin"',
    }
    for relative, expected in caller_markers.items():
        assert expected in (ROOT / relative).read_text(encoding="utf-8")

    # The bundle packages authenticated prebuilt artifacts; it verifies their
    # explicit signer feature instead of launching its own Cargo build.
    bundle = (ROOT / "scripts/build_release_bundle.sh").read_text(encoding="utf-8")
    for expected in (
        'provenance_features+=",irohad/external-software-signer-bin"',
        'provenance_features="irohad/external-software-signer-bin"',
        '"$repo_root/scripts/verify_release_prebuilt_provenance.py"',
        '--features "$provenance_features"',
    ):
        assert expected in bundle


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


def test_reviewed_inventory_preserves_the_existing_default_ceiling() -> None:
    assert TARGET_INVENTORY.BASELINE_DEFAULT_BIN_COUNT == 92
    assert TARGET_INVENTORY.BASELINE_DECLARED_BIN_COUNT == 116
    assert TARGET_INVENTORY.MAX_DEFAULT_BIN_COUNT == 24
    assert len(TARGET_INVENTORY.EXPECTED_DEFAULT_BINS) == 23
    assert len(TARGET_INVENTORY.EXPECTED_DECLARED_BINS) == 103
    assert TARGET_INVENTORY.EXPECTED_DEFAULT_BINS <= TARGET_INVENTORY.EXPECTED_DECLARED_BINS


def test_taira_custody_launcher_is_an_explicit_shipping_owner() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    target = ("irohad", "iroha3d_taira")
    assert target in TARGET_INVENTORY.all_workspace_bins(metadata)
    assert target in TARGET_INVENTORY.resolved_default_bins(metadata)
    manifest = tomllib.loads((ROOT / "crates/irohad/Cargo.toml").read_text())
    launcher = next(row for row in manifest["bin"] if row["name"] == target[1])
    assert launcher["path"] == "src/bin/iroha3d_taira.rs"
    assert launcher["required-features"] == ["daemon"]
    assert "daemon" in manifest["features"]["default"]
    assert "irohad::taira_runtime_signer::main_entry();" in (
        ROOT / "crates/irohad/src/bin/iroha3d_taira.rs"
    ).read_text()


def test_validator_capability_has_only_the_canonical_iroha_binary() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    assert ("sorafs_manifest", "sorafs-validate") not in (
        TARGET_INVENTORY.all_workspace_bins(metadata)
    )
    assert ("iroha_cli", "iroha") in TARGET_INVENTORY.resolved_default_bins(metadata)


def test_rejects_developer_owner_replacement_at_unchanged_count() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(row for row in modified["packages"] if row["name"] == "ivm")
    target = next(row for row in package["targets"] if row["name"] == "ivm_fixture_export")
    target["name"] = "unreviewed_fixture_export"
    assert len(TARGET_INVENTORY.all_workspace_bins(modified)) == 103
    assert TARGET_INVENTORY.resolved_default_bins(modified) == (
        TARGET_INVENTORY.resolved_default_bins(metadata)
    )
    errors = TARGET_INVENTORY.check_metadata(modified)
    assert any("reviewed binary owners are no longer declared" in error for error in errors)
    assert any("unreviewed binary owners are declared" in error for error in errors)
    assert not any("declared binary count" in error for error in errors)


def test_rejects_new_default_enablement_of_an_existing_developer_owner() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(row for row in modified["packages"] if row["name"] == "ivm")
    target = next(row for row in package["targets"] if row["name"] == "ivm_fixture_export")
    target["required-features"] = []
    second_target = next(row for row in package["targets"] if row["name"] == "gas_probe")
    second_target["required-features"] = []
    assert TARGET_INVENTORY.all_workspace_bins(modified) == (
        TARGET_INVENTORY.all_workspace_bins(metadata)
    )
    errors = TARGET_INVENTORY.check_metadata(modified)
    assert any("non-shipping binaries enabled by default" in error for error in errors)
    assert any("default binary count 25 exceeds 24" in error for error in errors)
    assert not any("declared binary count" in error for error in errors)


def test_instruction_builder_has_only_the_canonical_iroha_binary() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    assert ("sorafs_car", "sorafs_tx_stdin_builder") not in (
        TARGET_INVENTORY.all_workspace_bins(metadata)
    )
    assert ("iroha_cli", "iroha") in TARGET_INVENTORY.resolved_default_bins(metadata)


def test_rejects_unreviewed_default_owner_within_unchanged_ceiling() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)
    modified = copy.deepcopy(metadata)
    package = next(row for row in modified["packages"] if row["name"] == "ivm")
    target = next(row for row in package["targets"] if row["name"] == "ivm_fixture_export")
    target["required-features"] = []
    assert len(TARGET_INVENTORY.resolved_default_bins(modified)) == 24
    errors = TARGET_INVENTORY.check_metadata(modified)
    assert any("non-shipping binaries enabled by default" in error for error in errors)
    assert not any("exceeds" in error for error in errors)
    assert not any("declared binary count" in error for error in errors)
