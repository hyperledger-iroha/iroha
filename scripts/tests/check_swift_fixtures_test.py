"""Tests for the Swift fixture parity policy."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_swift_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_swift_fixtures", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write(root: Path, relative: str, contents: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def test_compare_only_manages_generated_descriptors(tmp_path: Path) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    write(source, "transaction_fixtures.manifest.json", "manifest")
    write(target, "transaction_payloads.json", "payloads")
    write(target, "transaction_fixtures.manifest.json", "manifest")

    write(source, "transfer_asset.norito", "canonical-only")
    write(target, "swift_mint_asset_basic.norito", "swift-only")
    write(target, "swift_parity_manifest.json", "{}")
    write(target, "js_email_identifier_request.json", "{}")
    write(target, "offline/kagemusha_peer_transport_v2.json", "{}")

    assert MODULE.compare(source, target) == ([], [], [])


def test_compare_rejects_non_swift_and_nested_norito_orphans(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    write(source, "transaction_fixtures.manifest.json", "manifest")
    write(target, "transaction_payloads.json", "payloads")
    write(target, "transaction_fixtures.manifest.json", "manifest")
    write(target, "transfer_asset.norito", "redundant")
    write(target, "nested/swift_looks_owned.norito", "nested-orphan")

    assert MODULE.compare(source, target) == (
        [],
        [
            Path("nested/swift_looks_owned.norito"),
            Path("transfer_asset.norito"),
        ],
        [],
    )


def test_compare_reports_descriptor_missing_and_content_drift(
    tmp_path: Path,
) -> None:
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
    import pytest

    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    target.mkdir()

    with pytest.raises(FileNotFoundError, match="missing canonical fixture"):
        MODULE.compare(source, target)
