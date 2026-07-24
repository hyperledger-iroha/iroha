"""Tests for the closed SoraFS CLI release-manifest inventory."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

import pytest

from scripts import generate_sorafs_cli_release_manifest as release_manifest


VERSION = "1.2.3"
COMMIT = "ab" * 20
REPOSITORY = "hyperledger/iroha"
REF = "refs/tags/sorafs-cli-v1.2.3"


def _write_candidates(root: Path) -> None:
    for index, target in enumerate(release_manifest.TARGETS):
        candidate = root / f"sorafs-cli-{VERSION}-{target}"
        nested = candidate / "platform-archive"
        nested.mkdir(parents=True)
        binary = candidate / ("sorafs_cli.exe" if "windows" in target else "sorafs_cli")
        archive = nested / f"candidate-{index}.tar.gz"
        binary.write_bytes(f"binary-{target}\n".encode())
        archive.write_bytes(f"archive-{target}\n".encode())
        checksums = []
        for path in (archive, binary):
            relative = path.relative_to(candidate).as_posix()
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            checksums.append(f"{digest}  {relative}\n")
        (candidate / "SHA256SUMS").write_text(
            "".join(sorted(checksums)),
            encoding="utf-8",
        )


def _build(root: Path) -> dict[str, object]:
    return release_manifest.build_manifest(
        root,
        version=VERSION,
        commit=COMMIT,
        repository=REPOSITORY,
        ref=REF,
    )


def test_manifest_is_canonical_deterministic_and_checks_exact_inventory(
    tmp_path: Path,
) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)

    first = release_manifest.canonical_payload(_build(candidates))
    second = release_manifest.canonical_payload(_build(candidates))
    assert first == second
    assert first == (
        json.dumps(json.loads(first), indent=2, sort_keys=True) + "\n"
    ).encode()
    decoded = json.loads(first)
    assert decoded["schema"] == release_manifest.SCHEMA
    assert decoded["targets"] == list(release_manifest.TARGETS)
    assert decoded["artifact_count"] == 15
    assert len(decoded["artifacts"]) == 15

    manifest = tmp_path / "release_manifest.json"
    assert (
        release_manifest.main(
            [
                "create",
                "--artifacts-dir",
                str(candidates),
                "--version",
                VERSION,
                "--commit",
                COMMIT,
                "--repository",
                REPOSITORY,
                "--ref",
                REF,
                "--output",
                str(manifest),
            ]
        )
        == 0
    )
    assert manifest.read_bytes() == first
    assert (
        release_manifest.main(
            [
                "check",
                "--artifacts-dir",
                str(candidates),
                "--version",
                VERSION,
                "--commit",
                COMMIT,
                "--repository",
                REPOSITORY,
                "--ref",
                REF,
                "--manifest",
                str(manifest),
            ]
        )
        == 0
    )


def test_check_rejects_candidate_tampering(tmp_path: Path) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    manifest = tmp_path / "release_manifest.json"
    manifest.write_bytes(release_manifest.canonical_payload(_build(candidates)))

    target = release_manifest.TARGETS[0]
    (candidates / f"sorafs-cli-{VERSION}-{target}" / "sorafs_cli").write_bytes(
        b"tampered\n"
    )
    assert (
        release_manifest.main(
            [
                "check",
                "--artifacts-dir",
                str(candidates),
                "--version",
                VERSION,
                "--commit",
                COMMIT,
                "--repository",
                REPOSITORY,
                "--ref",
                REF,
                "--manifest",
                str(manifest),
            ]
        )
        == 1
    )


@pytest.mark.parametrize("mutation", ["missing-target", "extra-root", "incomplete-sums"])
def test_manifest_rejects_open_inventory(tmp_path: Path, mutation: str) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    target = release_manifest.TARGETS[0]
    candidate = candidates / f"sorafs-cli-{VERSION}-{target}"
    if mutation == "missing-target":
        for path in sorted(candidate.rglob("*"), reverse=True):
            if path.is_dir():
                path.rmdir()
            else:
                path.unlink()
        candidate.rmdir()
    elif mutation == "extra-root":
        (candidates / "unreviewed.txt").write_text("unexpected", encoding="utf-8")
    else:
        (candidate / "SHA256SUMS").write_text(
            (candidate / "SHA256SUMS").read_text(encoding="utf-8").splitlines(
                keepends=True
            )[0],
            encoding="utf-8",
        )

    with pytest.raises(release_manifest.ManifestError):
        _build(candidates)


def test_manifest_rejects_symlink_and_hardlink_candidates(tmp_path: Path) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    target = release_manifest.TARGETS[0]
    candidate = candidates / f"sorafs-cli-{VERSION}-{target}"
    symlink = candidate / "linked"
    symlink.symlink_to(candidate / "sorafs_cli")
    with pytest.raises(release_manifest.ManifestError, match="symlink"):
        _build(candidates)
    symlink.unlink()

    directory_symlink = candidate / "linked-directory"
    directory_symlink.symlink_to(
        candidate / "platform-archive",
        target_is_directory=True,
    )
    with pytest.raises(release_manifest.ManifestError, match="symlink"):
        _build(candidates)
    directory_symlink.unlink()

    hardlink = candidate / "hardlinked"
    os.link(candidate / "sorafs_cli", hardlink)
    with pytest.raises(release_manifest.ManifestError, match="hard link"):
        _build(candidates)


def test_manifest_candidate_discovery_enforces_depth_limit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    target = release_manifest.TARGETS[0]
    candidate = candidates / f"sorafs-cli-{VERSION}-{target}"
    (candidate / "platform-archive/too-deep").mkdir()
    monkeypatch.setattr(release_manifest, "MAX_CANDIDATE_TREE_DEPTH", 1)

    with pytest.raises(release_manifest.ManifestError, match="depth limit"):
        _build(candidates)


def test_manifest_candidate_discovery_enforces_entry_limit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    monkeypatch.setattr(release_manifest, "MAX_ENTRIES_PER_TARGET", 3)

    with pytest.raises(release_manifest.ManifestError, match="3-entry limit"):
        _build(candidates)
