"""Regression tests for provider-neutral XCFramework device-pool validation."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[2] / "scripts/check_xcframework_device_pool.py"
SPEC = importlib.util.spec_from_file_location("check_xcframework_device_pool", MODULE_PATH)
DEVICE_POOL = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["check_xcframework_device_pool"] = DEVICE_POOL
SPEC.loader.exec_module(DEVICE_POOL)  # type: ignore[attr-defined]


def _write_matrix(path: Path, *, include_hardware: bool) -> None:
    matrix = {
        "iphone_sim": "platform=iOS Simulator,name=iPhone 15",
        "ipad_sim": "platform=iOS Simulator,name=iPad (10th generation)",
        "mac_fallback": "platform=macOS,arch=arm64,variant=Designed for iPad",
    }
    if include_hardware:
        matrix["strongbox"] = "platform=iOS,name=iPhone 14 Pro"
    path.write_text(json.dumps(matrix), encoding="utf-8")


def test_software_simulator_matrix_does_not_require_hardware(tmp_path: Path) -> None:
    matrix_path = tmp_path / "matrix.json"
    _write_matrix(matrix_path, include_hardware=False)

    matrix = DEVICE_POOL.load_matrix(matrix_path)

    assert set(matrix) == {"iphone_sim", "ipad_sim", "mac_fallback"}


def test_hardware_matrix_validation_is_explicit_opt_in(tmp_path: Path) -> None:
    matrix_path = tmp_path / "matrix.json"
    _write_matrix(matrix_path, include_hardware=False)

    with pytest.raises(SystemExit, match="strongbox"):
        DEVICE_POOL.load_matrix(matrix_path, require_hardware_lane=True)

    _write_matrix(matrix_path, include_hardware=True)
    matrix = DEVICE_POOL.load_matrix(matrix_path, require_hardware_lane=True)
    assert matrix["strongbox"] == "platform=iOS,name=iPhone 14 Pro"
