"""Discover split Sumeragi v2 proof-ledger checker components."""

from __future__ import annotations

import ast
from pathlib import Path
import re


def proof_ledger_checker_components(source_root: Path) -> tuple[Path, ...]:
    """Return the complete, validated checker-component path set."""
    checker = (
        source_root
        / "scripts"
        / "formal"
        / "check_sumeragi_v2_proof_ledger.py"
    )
    tree = ast.parse(checker.read_text(encoding="utf-8"), filename=str(checker))
    manifests = [
        node.value
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name)
            and target.id == "_CHECKER_COMPONENT_FILES"
            for target in node.targets
        )
    ]
    if len(manifests) != 1:
        raise RuntimeError(
            f"{checker}: require exactly one _CHECKER_COMPONENT_FILES manifest"
        )
    names = ast.literal_eval(manifests[0])
    if (
        not isinstance(names, tuple)
        or not all(
            isinstance(name, str)
            and Path(name).name == name
            and re.fullmatch(r"sumeragi_v2_proof_ledger_[a-z0-9_]+\.py", name)
            for name in names
        )
        or len(names) != len(set(names))
    ):
        raise RuntimeError(f"{checker}: invalid checker component manifest")
    relatives = tuple(Path("scripts/formal") / name for name in names)
    for relative in relatives:
        source = source_root / relative
        if source.is_symlink() or not source.is_file():
            raise FileNotFoundError(
                f"proof-ledger checker component is unavailable: {source}"
            )
    discovered = tuple(
        sorted(
            path.name
            for path in checker.parent.glob("sumeragi_v2_proof_ledger_*.py")
        )
    )
    if tuple(sorted(names)) != discovered:
        raise RuntimeError(
            f"{checker}: checker component manifest does not match {discovered!r}"
        )
    return relatives


def terminal_output_path(
    evidence: dict[str, Path | str | list[Path]],
) -> Path:
    """Return the typed terminal-output path from a release fixture."""
    output = evidence["terminal_output"]
    assert isinstance(output, Path)
    return output
