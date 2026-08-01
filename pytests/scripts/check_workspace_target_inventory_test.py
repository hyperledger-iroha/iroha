"""Tests for the ordinary-workspace target inventory guard."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_workspace_target_inventory.py"
SPEC = importlib.util.spec_from_file_location("check_workspace_target_inventory", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
TARGET_INVENTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TARGET_INVENTORY)


def test_repository_target_inventory() -> None:
    metadata = TARGET_INVENTORY.load_metadata(ROOT)

    assert TARGET_INVENTORY.check_metadata(metadata) == []


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
