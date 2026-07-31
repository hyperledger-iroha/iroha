"""Tests for the Python Norito RPC fixture mirror policy."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_python_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_python_fixtures", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write(root: Path, relative: str, contents: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def test_compare_only_manages_descriptors_and_rejects_redundant_blobs(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    write(source, "transaction_fixtures.manifest.json", "manifest")
    write(source, "transfer_asset.norito", "canonical")
    write(target, "transaction_payloads.json", "payloads")
    write(target, "transaction_fixtures.manifest.json", "manifest")
    write(target, "unrelated.json", "{}")

    assert MODULE.compare(source, target) == ([], [], [])

    write(target, "nested/transfer_asset.norito", "redundant")
    assert MODULE.compare(source, target) == (
        [],
        [Path("nested/transfer_asset.norito")],
        [],
    )


def test_compare_reports_missing_and_content_drift(tmp_path: Path) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "source")
    write(source, "transaction_fixtures.manifest.json", "source")
    write(target, "transaction_fixtures.manifest.json", "target")

    missing, extra, diffs = MODULE.compare(source, target)

    assert missing == [Path("transaction_payloads.json")]
    assert extra == []
    assert [
        (src.relative_to(source), dst.relative_to(target)) for src, dst in diffs
    ] == [
        (
            Path("transaction_fixtures.manifest.json"),
            Path("transaction_fixtures.manifest.json"),
        )
    ]


def test_compare_requires_both_canonical_descriptors(tmp_path: Path) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    target.mkdir()

    with pytest.raises(FileNotFoundError, match="missing canonical fixture"):
        MODULE.compare(source, target)
