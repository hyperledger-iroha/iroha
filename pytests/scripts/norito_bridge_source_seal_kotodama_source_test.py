"""Canonical Kotodama source-role admission; no Cargo or native provider work."""
from __future__ import annotations

import importlib.util
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("norito_kotodama_source_role", ROOT / "scripts/norito_bridge_source_seal.py")
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seal)


def test_actual_numbered_compiler_source_retains_original_bytes() -> None:
    relative = "crates/kotodama_lang/src/compiler/fixtures/v1/c106.ko"
    original = (ROOT / relative).read_bytes()
    assert b"context::authority(1)" in original
    assert seal._read_public_source_bytes(ROOT, relative) == original


def test_kotodama_source_name_uses_the_existing_code_role(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    relative = "crates/example/src/ordinary_credential_union.ko"
    source = root / relative
    source.parent.mkdir(parents=True)
    original = b"seiyaku PublicFixture { fn main() {} }\n"
    source.write_bytes(original)
    assert seal._read_public_source_bytes(root, relative) == original


@pytest.mark.parametrize("relative", [
    "crates/example/private/c106.ko",
    "crates/example/materials/c106.ko",
    "crates/example/credentials/c106.ko",
    "crates/example/operations/c106.ko",
    "crates/example/src/vultr_fixture.ko",
    "crates/example/src/sydneycreds.ko",
    "crates/example/src/.env.ko",
    "crates/example/fixtures/deadbeef",
    "crates/example/fixtures/c106.unreviewed",
])
def test_kotodama_role_preserves_refusal_before_open(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    source = root / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(b"PUBLIC SENTINEL ONLY\n")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no content descriptor")) as opened:
        with pytest.raises(RuntimeError, match="prohibited|admitted public filename"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


def test_kotodama_leaf_alias_preserves_refusal_before_open(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    (root / "original.ko").write_bytes(b"seiyaku PublicFixture {}\n")
    (root / "alias.ko").symlink_to("original.ko")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no content descriptor")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, "alias.ko")
    opened.assert_not_called()
