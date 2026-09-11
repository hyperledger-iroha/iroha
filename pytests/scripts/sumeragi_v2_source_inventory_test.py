"""Canonical source-inventory ownership and fail-closed loader contracts."""

from __future__ import annotations

import ast
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
OWNER_NAME = "sumeragi_v2_proof_ledger_source_inventory.py"
OWNER = ROOT / "scripts" / "formal" / OWNER_NAME
CHECKER = ROOT / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"


def load_module(path: Path, name: str):
    """Load the actual independent reader or fixture component under test."""
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(name, None)
        raise
    return module


def checker_bootstrap(checker_path: Path = CHECKER) -> dict:
    """Execute the actual checker prefix through its inventory bootstrap."""
    source = CHECKER.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(CHECKER))
    prefix = []
    for node in tree.body:
        prefix.append(node)
        if (
            isinstance(node, ast.Expr)
            and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name)
            and node.value.func.id == "_execute_checker_component"
        ):
            assert ast.literal_eval(node.value.args[0]) == OWNER_NAME
            break
    else:
        pytest.fail("checker has no inventory bootstrap")
    namespace = {"__file__": str(checker_path), "__name__": "_inventory_bootstrap"}
    exec(compile(ast.Module(body=prefix, type_ignores=[]), str(CHECKER), "exec"), namespace)
    return namespace


def reader():
    """Load the canonical AST reader without importing the aggregate checker."""
    return load_module(
        ROOT / "scripts/formal/sumeragi_v2_multilane_reviewed_rust_source.py",
        "_source_inventory_reader_test",
    )


def test_inventory_has_one_declaration_owner_and_exact_component_bootstrap() -> None:
    """The data owner bootstraps once and belongs to the sealed component set."""
    tree = ast.parse(OWNER.read_text(encoding="utf-8"))
    assert isinstance(tree.body[0], ast.Expr)
    assert isinstance(tree.body[0].value, ast.Constant)
    declarations = tree.body[1:]
    assert all(isinstance(node, ast.Assign) for node in declarations)
    assert [node.targets[0].id for node in declarations] == [
        "_CHECKER_COMPONENT_FILES",
        "_KURA_PRODUCTION_COMPONENT_FILES",
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
    ]
    loaded = checker_bootstrap()
    calls = []
    for node in ast.parse(CHECKER.read_text(encoding="utf-8")).body:
        if (
            isinstance(node, ast.Expr)
            and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name)
            and node.value.func.id == "_execute_checker_component"
        ):
            calls.append(ast.literal_eval(node.value.args[0]))
    assert calls[0] == OWNER_NAME
    assert len(calls) == len(set(calls))
    assert sorted(calls) == sorted(loaded["_CHECKER_COMPONENT_FILES"])
    assert loaded["_CHECKER_COMPONENT_FILES"].count(OWNER_NAME) == 1
    assert loaded["_REVIEWED_RUST_INCLUDE_MANIFESTS"][
        "crates/iroha_data_model/src/block/consensus_v2_tests.rs"
    ] == ("consensus_v2_context_tests.rs", "consensus_v2_json_tests.rs")


def test_ast_reader_authenticates_the_same_inventory_as_checker_bootstrap() -> None:
    """Both consumers resolve the same owner and canonical expanded mapping."""
    module = reader()
    assert module.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE == OWNER.relative_to(ROOT)
    assert module._CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS == []
    errors: list[str] = []
    manifest = module._decode_reviewed_rust_include_manifest(OWNER, errors)
    assert errors == []
    assert manifest == checker_bootstrap()["_REVIEWED_RUST_INCLUDE_MANIFESTS"]


def test_inventory_bootstrap_rejects_missing_or_symlinked_owner(tmp_path: Path) -> None:
    """The existing regular-file loader remains the only execution path."""
    checker = tmp_path / CHECKER.name
    with pytest.raises(RuntimeError, match="checker component is unavailable"):
        checker_bootstrap(checker)
    (tmp_path / OWNER_NAME).symlink_to(OWNER)
    with pytest.raises(RuntimeError, match="checker component is unavailable"):
        checker_bootstrap(checker)


def test_ast_reader_has_no_former_owner_fallback(tmp_path: Path) -> None:
    """A former-owner copy cannot satisfy the new exact inventory path."""
    module = reader()
    former = tmp_path / "scripts/formal/sumeragi_v2_proof_ledger_source_seal_contracts.py"
    former.parent.mkdir(parents=True)
    former.write_bytes(OWNER.read_bytes())
    errors: list[str] = []
    module._validate_reviewed_rust_include_manifest(tmp_path, errors)
    assert errors
    assert any(OWNER_NAME in error for error in errors)
    destination = tmp_path / module.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE
    destination.symlink_to(OWNER)
    errors.clear()
    module._validate_reviewed_rust_include_manifest(tmp_path, errors)
    assert any("regular non-symlink file" in error for error in errors)


@pytest.mark.parametrize("mutation", ["omitted_child", "duplicate_assignment", "executable_payload"])
def test_ast_reader_rejects_inventory_substitution(tmp_path: Path, mutation: str) -> None:
    """Path migration retains digest, unique-assignment and data-only checks."""
    module = reader()
    source = OWNER.read_text(encoding="utf-8")
    if mutation == "omitted_child":
        source = source.replace("        'consensus_v2_context_tests.rs',\n", "")
        expected = "manifest digest must equal"
    elif mutation == "duplicate_assignment":
        source += "\n_KURA_PRODUCTION_COMPONENT_FILES = ()\n"
        expected = "exactly one _KURA_PRODUCTION_COMPONENT_FILES"
    else:
        source = "raise AssertionError('untrusted inventory must not execute')\n"
        expected = "exactly one"
    target = tmp_path / OWNER_NAME
    target.write_text(source, encoding="utf-8")
    errors: list[str] = []
    module._decode_reviewed_rust_include_manifest(target, errors)
    assert any(expected in error for error in errors), errors


def test_release_fixture_retains_canonical_owner_and_checker_protocol(tmp_path: Path) -> None:
    """The source-isolated fixture retains its owner and all stub exit behavior."""
    module = load_module(
        ROOT / "pytests/scripts/sumeragi_v2_release_receipt_components.py",
        "_source_inventory_receipt_components_test",
    )
    formal = tmp_path / "scripts/formal"
    formal.mkdir(parents=True)
    module.install_checker_fixture(formal)
    relative = Path("scripts/formal") / OWNER_NAME
    assert module.proof_ledger_checker_components(tmp_path) == (relative,)
    ledger = tmp_path / "ledger.json"
    ledger.write_text(json.dumps({"obligations": [{"id": "fixture", "status": "cross_tool_proved"}]}))
    command = [sys.executable, "-I", "-S", str(formal / CHECKER.name), "--ledger", str(ledger)]
    result = subprocess.run(command + ["--print-cross-tool-obligations"], check=False, capture_output=True, text=True)
    assert result.returncode == 0
    assert result.stdout == "fixture\n"
    result = subprocess.run(command + ["--release"], check=False, capture_output=True, text=True)
    assert result.returncode == 81
    (formal / OWNER_NAME).unlink()
    with pytest.raises(FileNotFoundError):
        module.proof_ledger_checker_components(tmp_path)
