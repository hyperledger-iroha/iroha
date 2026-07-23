"""Focused tests for the production NoritoBridge dependency-closure seal."""

from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "norito_bridge_source_seal.py"
SPEC = importlib.util.spec_from_file_location("norito_bridge_source_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SOURCE_SEAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SOURCE_SEAL)


def _git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout


@pytest.fixture
def source_fixture(tmp_path: Path) -> Path:
    root = tmp_path / "iroha"
    (root / "bridge-src").mkdir(parents=True)
    (root / ".gitignore").write_text("Cargo.lock\nbridge-src/*.cache\n", encoding="utf-8")
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (root / "Cargo.lock").write_text("lock-v1\n", encoding="utf-8")
    (root / "bridge-src/lib.rs").write_text("pub fn bridge() {}\n", encoding="utf-8")
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "Source Seal Test")
    _git(root, "config", "user.email", "source-seal@example.invalid")
    _git(root, "add", ".gitignore", "Cargo.toml", "bridge-src/lib.rs")
    _git(root, "commit", "-qm", "fixture")
    return root


def test_explicit_ignored_root_input_is_fingerprinted(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]

    listed = SOURCE_SEAL.listed_files(source_fixture, inputs)
    before = SOURCE_SEAL.fingerprint(source_fixture, inputs)
    (source_fixture / "Cargo.lock").write_text("lock-v2\n", encoding="utf-8")
    after = SOURCE_SEAL.fingerprint(source_fixture, inputs)

    assert "Cargo.lock" in listed
    assert before != after
    # The lock remains intentionally ignored/untracked; its exact bytes are
    # bound by the fingerprint rather than misclassified as Git dirt.
    assert SOURCE_SEAL.status(source_fixture, inputs) == ""


def test_nonignored_untracked_dependency_input_is_dirty(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    (source_fixture / "bridge-src/new.rs").write_text("pub fn new_input() {}\n", encoding="utf-8")

    listed = SOURCE_SEAL.listed_files(source_fixture, inputs)
    status = SOURCE_SEAL.status(source_fixture, inputs)

    assert "bridge-src/new.rs" in listed
    assert "?? bridge-src/new.rs" in status


def test_unnamed_policy_ignored_file_stays_outside_seal(source_fixture: Path) -> None:
    inputs = ["Cargo.lock", "Cargo.toml", "bridge-src"]
    (source_fixture / "bridge-src/local.cache").write_text("generated\n", encoding="utf-8")

    assert "bridge-src/local.cache" not in SOURCE_SEAL.listed_files(source_fixture, inputs)
    assert SOURCE_SEAL.status(source_fixture, inputs) == ""


def test_explicit_symlink_is_rejected(source_fixture: Path) -> None:
    lock = source_fixture / "Cargo.lock"
    lock.unlink()
    lock.symlink_to("Cargo.toml")

    with pytest.raises(RuntimeError, match="explicit source-seal input is symlinked: Cargo.lock"):
        SOURCE_SEAL.listed_files(source_fixture, ["Cargo.lock", "Cargo.toml"])
