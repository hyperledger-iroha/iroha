"""Discover source-isolated Sumeragi v2 release components."""

from __future__ import annotations

import ast
from pathlib import Path
import re
import shutil
import subprocess
import sys


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


def install_cache_helper(source_root: Path, repository_root: Path) -> None:
    """Install the production cache helper in an external sealed fixture."""
    scripts = source_root / "scripts"
    scripts.mkdir()
    shutil.copy2(
        repository_root / "scripts" / "copy_sumeragi_v2_release_cargo_cache.py",
        scripts / "copy_sumeragi_v2_release_cargo_cache.py",
    )
    shutil.copy2(
        repository_root
        / "scripts"
        / "copy_sumeragi_v2_release_cargo_cache_cli.py",
        scripts / "copy_sumeragi_v2_release_cargo_cache_cli.py",
    )


def fixture_corridor_legs(
    writer_symbols: dict[str, object], cargo_path: Path
) -> object:
    """Return receipt fixture legs bound to its authenticated Cargo path."""

    corridor_legs = writer_symbols["_corridor_legs"]
    assert callable(corridor_legs)
    return corridor_legs(str(cargo_path.resolve()))


def run_fixture_cargo_cache_copy(
    runner: Path, source_home: Path | None, cargo_home: Path, inventory: Path
) -> subprocess.CompletedProcess[str]:
    """Execute the production cache copier against fixture roots."""

    helper = runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")
    arguments = [
        sys.executable, "-I", "-S", str(helper),
        "--cargo-home", str(cargo_home), "--inventory", str(inventory),
    ]
    if source_home is None:
        arguments.append("--final")
    else:
        arguments.extend(("--source-cargo-home", str(source_home)))
    return subprocess.run(
        arguments,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def fixture_cargo_cache_input(
    runner: Path, source_home: Path, artifact_root: Path
) -> tuple[Path, Path, Path, Path, dict[str, str]]:
    """Create one private Cargo cache copy and its production-format inventory."""

    source_home.mkdir(mode=0o700)
    registry_cache = source_home / "registry" / "cache"
    registry_cache.mkdir(parents=True, mode=0o700)
    registry_cache.parent.chmod(0o700)
    registry_cache.chmod(0o700)
    crate = registry_cache / "fixture.crate"
    crate.write_bytes(b"fixture registry cache bytes\n")
    crate.chmod(0o600)
    (registry_cache / "fixture-copy").symlink_to(crate.name)
    git_db = source_home / "git" / "db"
    git_db.mkdir(parents=True, mode=0o700)
    git_db.parent.chmod(0o700)
    git_db.chmod(0o700)
    git_head = git_db / "HEAD"
    git_head.write_bytes(b"ref: refs/heads/main\n")
    git_head.chmod(0o600)
    cargo_home = artifact_root / "cargo-home"
    for runtime_name in ("home", "tmp", "cache"):
        (artifact_root / runtime_name).mkdir(mode=0o700)
        (artifact_root / runtime_name).chmod(0o700)
    cargo_home.mkdir(mode=0o700)
    cargo_home.chmod(0o700)
    inventory = artifact_root / "cargo-cache-input.json"
    result = run_fixture_cargo_cache_copy(
        runner, source_home, cargo_home, inventory
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    final_inventory = artifact_root / "cargo-cache-final.json"
    final_result = run_fixture_cargo_cache_copy(
        runner, None, cargo_home, final_inventory
    )
    if final_result.returncode != 0:
        raise RuntimeError(final_result.stderr)
    runtime_fields = {
        "runtime_home_path": str((artifact_root / "home").resolve()),
        "runtime_tmpdir_path": str((artifact_root / "tmp").resolve()),
        "runtime_tmp_path": str((artifact_root / "tmp").resolve()),
        "runtime_temp_path": str((artifact_root / "tmp").resolve()),
        "runtime_cache_path": str((artifact_root / "cache").resolve()),
    }
    return cargo_home, inventory, final_inventory, source_home, runtime_fields
