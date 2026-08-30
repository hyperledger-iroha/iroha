"""Adversarial tests for the Taira Rust-sysroot content seal."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SEALER = ROOT / "scripts/hash_taira_rust_toolchain.py"
DOMAIN = b"iroha.taira.rust-toolchain-tree.v1\0"


def _tree(tmp_path: Path, name: str = "sysroot") -> Path:
    root = (tmp_path / name).resolve()
    (root / "bin").mkdir(parents=True)
    (root / "lib/rustlib").mkdir(parents=True)
    (root / "bin/rustc").write_bytes(b"compiler\x00bytes\n")
    (root / "bin/rustc").chmod(0o755)
    (root / "lib/library.rlib").write_bytes(b"library\n")
    (root / "lib/rustlib/manifest.in").write_bytes(b"component\n")
    os.symlink("library.rlib", root / "lib/current.rlib")
    return root


def _run(root: Path, output: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(SEALER),
            "--sysroot",
            str(root),
            "--manifest-out",
            str(output),
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def _seal(tmp_path: Path, name: str = "sysroot") -> tuple[Path, dict[str, object]]:
    root = _tree(tmp_path, name)
    output = (tmp_path / f"{name}.json").resolve()
    result = _run(root, output)
    assert result.returncode == 0, result.stderr
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert result.stdout == payload["tree_sha256"] + "\n"
    return root, payload


def test_seals_exact_sorted_tree_and_independent_domain_digest(tmp_path: Path) -> None:
    root, payload = _seal(tmp_path)

    identity = payload["tree_identity"]
    encoded = json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()
    expected = hashlib.sha256(
        DOMAIN + len(encoded).to_bytes(8, "big") + encoded
    ).hexdigest()
    assert payload["tree_sha256"] == expected
    assert payload["root_path"] == str(root)
    entries = identity["entries"]
    assert [entry["path"] for entry in entries] == sorted(
        entry["path"] for entry in entries
    )
    by_path = {entry["path"]: entry for entry in entries}
    assert by_path["bin/rustc"] == {
        "kind": "file",
        "mode": "0755",
        "path": "bin/rustc",
        "sha256": hashlib.sha256(b"compiler\x00bytes\n").hexdigest(),
        "size_bytes": 15,
    }
    assert by_path["lib/current.rlib"]["kind"] == "symlink"
    assert by_path["lib/current.rlib"]["target"] == "library.rlib"
    assert stat.S_IMODE((tmp_path / "sysroot.json").stat().st_mode) == 0o600


def test_seal_is_independent_of_creation_order_and_timestamps(tmp_path: Path) -> None:
    first, first_payload = _seal(tmp_path, "first")
    second = (tmp_path / "second").resolve()
    (second / "lib/rustlib").mkdir(parents=True)
    (second / "bin").mkdir()
    (second / "lib/rustlib/manifest.in").write_bytes(b"component\n")
    os.symlink("library.rlib", second / "lib/current.rlib")
    (second / "lib/library.rlib").write_bytes(b"library\n")
    (second / "bin/rustc").write_bytes(b"compiler\x00bytes\n")
    (second / "bin/rustc").chmod(0o755)
    for path in (first, *first.rglob("*"), second, *second.rglob("*")):
        if not path.is_symlink():
            os.utime(path, (1_000_000, 1_000_000))
    output = (tmp_path / "second.json").resolve()

    result = _run(second, output)

    assert result.returncode == 0, result.stderr
    second_payload = json.loads(output.read_text())
    assert second_payload["tree_sha256"] == first_payload["tree_sha256"]


@pytest.mark.parametrize("mutation", ["bytes", "mode", "path", "link"])
def test_every_semantic_tree_mutation_changes_the_seal(
    tmp_path: Path, mutation: str
) -> None:
    root, first = _seal(tmp_path, "baseline")
    clone = _tree(tmp_path, "changed")
    if mutation == "bytes":
        (clone / "lib/library.rlib").write_bytes(b"changed\n")
    elif mutation == "mode":
        (clone / "lib/library.rlib").chmod(0o600)
    elif mutation == "path":
        (clone / "lib/rustlib/manifest.in").rename(
            clone / "lib/rustlib/renamed.in"
        )
    else:
        (clone / "lib/current.rlib").unlink()
        os.symlink("rustlib/manifest.in", clone / "lib/current.rlib")
    output = (tmp_path / "changed.json").resolve()

    result = _run(clone, output)

    assert result.returncode == 0, result.stderr
    changed = json.loads(output.read_text())
    assert changed["tree_sha256"] != first["tree_sha256"]
    assert root.exists()


def test_rejects_external_and_dangling_symlinks(tmp_path: Path) -> None:
    for target in ("../../outside", "missing"):
        root = _tree(tmp_path, f"case-{target.replace('/', '-')}")
        (root / "lib/current.rlib").unlink()
        os.symlink(target, root / "lib/current.rlib")
        output = (tmp_path / f"{root.name}.json").resolve()

        result = _run(root, output)

        assert result.returncode != 0
        assert "escapes or is dangling" in result.stderr
        assert not output.exists()


def test_rejects_absolute_symlink_targets_even_when_they_resolve_inside(
    tmp_path: Path,
) -> None:
    root = _tree(tmp_path)
    (root / "lib/current.rlib").unlink()
    os.symlink(root / "lib/library.rlib", root / "lib/current.rlib")
    output = (tmp_path / "manifest.json").resolve()

    result = _run(root, output)

    assert result.returncode != 0
    assert "symlink target is unsafe" in result.stderr
    assert not output.exists()


def test_rejects_multiply_linked_files(tmp_path: Path) -> None:
    root = _tree(tmp_path)
    os.link(root / "lib/library.rlib", root / "lib/alias.rlib")
    output = (tmp_path / "manifest.json").resolve()

    result = _run(root, output)

    assert result.returncode != 0
    assert "singly linked" in result.stderr
    assert not output.exists()


def test_rejects_noncanonical_roots_and_outputs(tmp_path: Path) -> None:
    root = _tree(tmp_path)
    root_link = (tmp_path / "sysroot-link").resolve(strict=False)
    os.symlink(root, root_link)
    output = (tmp_path / "manifest.json").resolve()
    existing = (tmp_path / "existing.json").resolve()
    existing.write_bytes(b"preserve\n")

    cases = (
        (Path("relative"), output),
        (root_link, output),
        (root, Path("relative.json")),
        (root, existing),
        (root, root / "inside.json"),
    )
    for candidate_root, candidate_output in cases:
        result = _run(candidate_root, candidate_output)
        assert result.returncode != 0
    assert existing.read_bytes() == b"preserve\n"


def test_removes_manifest_if_tree_changes_after_publication(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _tree(tmp_path)
    output = (tmp_path / "manifest.json").resolve()
    spec = importlib.util.spec_from_file_location("toolchain_sealer_under_test", SEALER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    original = module._revalidate
    calls = 0

    def fail_second_revalidation(snapshots: object) -> None:
        nonlocal calls
        calls += 1
        if calls == 2:
            raise module.TreeHashError("simulated post-publication mutation")
        original(snapshots)

    monkeypatch.setattr(module, "_revalidate", fail_second_revalidation)

    with pytest.raises(module.TreeHashError, match="post-publication mutation"):
        module.seal(root, output)

    assert not output.exists()
