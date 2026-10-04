"""Check cross-package Python API names without loading native artifacts.

Actual installed-package imports remain a separate release gate. These source
controls catch removed Torii DTO names before they break the whole SDK import.
"""

from __future__ import annotations

import ast
from pathlib import Path


_PYTHON_ROOT = Path(__file__).resolve().parents[2]
_SDK_SOURCE = _PYTHON_ROOT / "iroha_python" / "src" / "iroha_python"
_TORII_SOURCE = _PYTHON_ROOT / "iroha_torii_client"
_NORITO_SOURCE = _PYTHON_ROOT / "norito_py" / "src" / "norito"


def _module_bindings(tree: ast.Module) -> set[str]:
    names: set[str] = set()
    for node in tree.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
            names.add(node.name)
        elif isinstance(node, (ast.Import, ast.ImportFrom)):
            names.update(alias.asname or alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                names.update(
                    item.id
                    for item in ast.walk(target)
                    if isinstance(item, ast.Name) and isinstance(item.ctx, ast.Store)
                )
    return names


def _source_tree(path: Path) -> ast.Module:
    return ast.parse(path.read_text(encoding="utf-8"), filename=str(path))


def test_python_sdk_named_torii_imports_exist_in_their_current_owner() -> None:
    missing: list[str] = []
    for source in sorted(_SDK_SOURCE.glob("*.py")):
        for node in ast.walk(_source_tree(source)):
            if (
                not isinstance(node, ast.ImportFrom)
                or not node.module
                or not node.module.startswith("iroha_torii_client.")
            ):
                continue
            owner = _TORII_SOURCE.joinpath(*node.module.split(".")[1:]).with_suffix(".py")
            assert owner.is_file(), (source.name, node.lineno, node.module)
            declared = _module_bindings(_source_tree(owner))
            for alias in node.names:
                if alias.name not in declared:
                    missing.append(f"{source.name}:{node.lineno}: {node.module}.{alias.name}")
    assert not missing, "missing current Torii API names: " + ", ".join(missing)


def test_python_public_client_imports_exist_in_the_current_client_source() -> None:
    declared = _module_bindings(_source_tree(_SDK_SOURCE / "client.py"))
    imports = [
        node
        for node in ast.walk(_source_tree(_SDK_SOURCE / "__init__.py"))
        if isinstance(node, ast.ImportFrom) and node.level == 1 and node.module == "client"
    ]
    assert imports
    missing = {alias.name for node in imports for alias in node.names} - declared
    assert not missing, missing


def test_validation_fee_public_source_retains_only_the_current_binding_type() -> None:
    for filename in ("client.py", "__init__.py"):
        tree = _source_tree(_SDK_SOURCE / filename)
        imported = {
            alias.name
            for node in ast.walk(tree)
            if isinstance(node, ast.ImportFrom)
            for alias in node.names
        }
        strings = {
            node.value
            for node in ast.walk(tree)
            if isinstance(node, ast.Constant) and isinstance(node.value, str)
        }
        for name in (
            "GovernanceValidationFeePayoutBinding",
            "GovernanceProposalValidationFeePayoutLifecycle",
        ):
            assert name in imported
            assert name in strings
        assert "GovernanceValidationFeePayoutRecipient" not in imported
        assert "GovernanceValidationFeePayoutRecipient" not in strings


def test_norito_declared_public_exports_have_module_bindings() -> None:
    tree = _source_tree(_NORITO_SOURCE / "__init__.py")
    exports = [
        ast.literal_eval(node.value)
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets)
    ]
    assert len(exports) == 1
    missing = set(exports[0]) - _module_bindings(tree)
    assert not missing, missing
