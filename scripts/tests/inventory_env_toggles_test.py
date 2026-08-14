"""Tests for the Rust environment-toggle inventory."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "inventory_env_toggles.py"
SPEC = importlib.util.spec_from_file_location("inventory_env_toggles", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_classify_scope_treats_included_test_shards_as_tests() -> None:
    """Test-only Rust shards under ``src`` must not become production toggles."""

    test_paths = (
        Path("crates/norito/src/core/sequence_plan_helper_tests.rs"),
        Path("crates/iroha_core/src/privacy_release_evidence/tests.rs"),
        Path("crates/iroha_core/src/tx/empty_instruction_test.rs"),
    )

    assert all(MODULE.classify_scope(path) == "test" for path in test_paths)


def test_classify_scope_does_not_match_test_as_an_arbitrary_substring() -> None:
    """Production names containing ``test`` remain production sources."""

    assert MODULE.classify_scope(Path("crates/example/src/contest.rs")) == "prod"


def test_classify_scope_preserves_build_and_directory_roles() -> None:
    """The filename rule must preserve existing build and directory policy."""

    assert MODULE.classify_scope(Path("crates/example/build.rs")) == "build"
    assert MODULE.classify_scope(Path("crates/example/tests/support.rs")) == "test"
    assert MODULE.classify_scope(Path("tools/example/src/main.rs")) == "tool"
