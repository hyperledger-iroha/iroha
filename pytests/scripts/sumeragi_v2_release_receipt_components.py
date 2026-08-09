"""Discover source-isolated Sumeragi v2 release components."""

from __future__ import annotations

import ast
from pathlib import Path
import re


def _declared_components(
    source_root: Path,
    *,
    parent_relative: Path,
    assignment: str,
    filename_pattern: str,
    discovery_pattern: str,
    label: str,
) -> tuple[Path, ...]:
    """Return one exact, regular, non-symlink component closure."""

    parent = source_root / parent_relative
    tree = ast.parse(parent.read_text(encoding="utf-8"), filename=str(parent))
    manifests = [
        node.value
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name)
            and target.id == assignment
            for target in node.targets
        )
    ]
    if len(manifests) != 1:
        raise RuntimeError(
            f"{parent}: require exactly one {assignment} manifest"
        )
    names = ast.literal_eval(manifests[0])
    if (
        not isinstance(names, tuple)
        or not all(
            isinstance(name, str)
            and Path(name).name == name
            and re.fullmatch(filename_pattern, name)
            for name in names
        )
        or len(names) != len(set(names))
    ):
        raise RuntimeError(f"{parent}: invalid {label} component manifest")
    relatives = tuple(parent_relative.parent / name for name in names)
    for relative in relatives:
        source = source_root / relative
        if source.is_symlink() or not source.is_file():
            raise FileNotFoundError(
                f"{label} component is unavailable: {source}"
            )
    discovered = tuple(
        sorted(
            path.name
            for path in parent.parent.glob(discovery_pattern)
        )
    )
    if tuple(sorted(names)) != discovered:
        raise RuntimeError(
            f"{parent}: {label} component manifest does not match {discovered!r}"
        )
    return relatives


def proof_ledger_checker_components(source_root: Path) -> tuple[Path, ...]:
    """Return the complete, validated checker-component path set."""

    return _declared_components(
        source_root,
        parent_relative=Path("scripts/formal/check_sumeragi_v2_proof_ledger.py"),
        assignment="_CHECKER_COMPONENT_FILES",
        filename_pattern=r"sumeragi_v2_proof_ledger_[a-z0-9_]+\.py",
        discovery_pattern="sumeragi_v2_proof_ledger_*.py",
        label="proof-ledger checker",
    )


def release_receipt_writer_components(source_root: Path) -> tuple[Path, ...]:
    """Return the complete, validated release-receipt component path set."""

    return _declared_components(
        source_root,
        parent_relative=Path("scripts/write_sumeragi_v2_release_receipt.py"),
        assignment="_RELEASE_RECEIPT_COMPONENT_FILES",
        filename_pattern=r"write_sumeragi_v2_release_receipt_[a-z0-9_]+\.py",
        discovery_pattern="write_sumeragi_v2_release_receipt_*.py",
        label="release receipt",
    )


def terminal_output_path(
    evidence: dict[str, Path | str | list[Path]],
) -> Path:
    """Return the typed terminal-output path from a release fixture."""
    output = evidence["terminal_output"]
    assert isinstance(output, Path)
    return output
