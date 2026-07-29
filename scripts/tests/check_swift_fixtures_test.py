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


def test_compare_ignores_intentional_swift_only_fixtures(tmp_path: Path) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "shared.norito", "shared")
    write(target, "shared.norito", "shared")

    write(source, "transaction_payload.json", "android-only")
    write(target, "transaction_payload.json", "swift-only")
    write(target, "swift_mint_asset_basic.norito", "swift-only")
    write(target, "swift_parity_manifest.json", "{}")
    write(target, "js_email_identifier_request.json", "{}")
    write(target, "offline/kagemusha_peer_transport_v2.json", "{}")

    assert MODULE.compare(source, target) == ([], [], [])


def test_compare_reports_managed_missing_extra_and_content_drift(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "missing.norito", "missing")
    write(source, "transaction_payloads.json", "source")
    write(target, "transaction_payloads.json", "target")
    write(target, "future_trigger_instructions.json", "extra")

    missing, extra, diffs = MODULE.compare(source, target)

    assert missing == [Path("missing.norito")]
    assert extra == [Path("future_trigger_instructions.json")]
    assert [
        (src.relative_to(source), dst.relative_to(target)) for src, dst in diffs
    ] == [(Path("transaction_payloads.json"), Path("transaction_payloads.json"))]


def test_root_only_exclusions_do_not_hide_nested_managed_files(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "nested/swift_contract.norito", "source")
    write(source, "nested/transaction_payload.json", "source")
    write(target, "nested/swift_contract.norito", "target")
    write(target, "nested/transaction_payload.json", "target")

    missing, extra, diffs = MODULE.compare(source, target)

    assert missing == []
    assert extra == []
    assert {
        src.relative_to(source) for src, _ in diffs
    } == {
        Path("nested/swift_contract.norito"),
        Path("nested/transaction_payload.json"),
    }
