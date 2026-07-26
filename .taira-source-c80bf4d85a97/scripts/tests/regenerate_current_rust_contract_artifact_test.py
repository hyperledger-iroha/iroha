"""Tests for exact-current Rust/Kotodama admission fixture regeneration."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "regenerate_current_rust_contract_artifact.py"
)
SPEC = importlib.util.spec_from_file_location(
    "regenerate_current_rust_contract_artifact", MODULE_PATH
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_git_blob_id_matches_git_object_format() -> None:
    assert MODULE._git_blob_id(b"test content\n") == (
        "d670460b4b4aece5915caf5c68d12f560a9fe3e4"
    )


def test_manifest_hash_accepts_only_canonical_literals() -> None:
    digest = "ab" * 32
    manifest = {"abi_hash": f"hash:{digest.upper()}#1234"}
    assert MODULE._manifest_hash(manifest, "abi_hash") == digest

    for invalid in (digest, f"hash:{digest}#1234", f"hash:{digest.upper()}"):
        with pytest.raises(MODULE.FixtureError, match="noncanonical abi_hash"):
            MODULE._manifest_hash({"abi_hash": invalid}, "abi_hash")


def test_rust_verifier_requires_the_exact_field_inventory(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    outputs = iter(
        (
            subprocess.CompletedProcess([], 0, "", ""),
            subprocess.CompletedProcess(
                [],
                0,
                "\n".join(
                    (
                        f"code_hash_hex={'01' * 32}",
                        f"abi_hash_hex={'02' * 32}",
                        "header_len=49",
                        "code_offset=897",
                        "entrypoint_count=2",
                    )
                )
                + "\n",
                "",
            ),
        )
    )
    monkeypatch.setattr(MODULE, "_run", lambda _command: next(outputs))
    rlib = tmp_path / "deps" / "libivm-0123456789abcdef.rlib"
    artifact = tmp_path / "artifact.to"

    assert MODULE._rust_verifier(
        Path("rustc"), rlib, artifact, tmp_path
    ) == {
        "code_hash_hex": "01" * 32,
        "abi_hash_hex": "02" * 32,
        "header_len": 49,
        "code_offset": 897,
        "entrypoint_count": 2,
    }


def test_check_reports_stale_fixture_diff(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture.json"
    fixture.write_text('{"old": true}\n', encoding="utf-8")

    with pytest.raises(MODULE.FixtureError, match="fixture is stale") as raised:
        MODULE._check('{"new": true}\n', fixture)

    assert '"old": true' in str(raised.value)
    assert '"new": true' in str(raised.value)


def test_atomic_write_preserves_fixture_mode(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture.json"
    fixture.write_text("old\n", encoding="utf-8")
    fixture.chmod(0o640)

    MODULE._atomic_write(fixture, "new\n")

    assert fixture.read_text(encoding="utf-8") == "new\n"
    assert os.stat(fixture).st_mode & 0o777 == 0o640
