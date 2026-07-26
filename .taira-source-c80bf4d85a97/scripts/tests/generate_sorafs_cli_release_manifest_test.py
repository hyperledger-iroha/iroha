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
        payload_paths = release_manifest.required_candidate_payload_paths(
            VERSION,
            target,
        )
        for relative in sorted(payload_paths):
            path = candidate / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            prefix = (
                "common"
                if relative in release_manifest.COMMON_TARGET_FILES
                else str(index)
            )
            path.write_bytes(f"{prefix}:{relative}\n".encode())
        checksums = []
        for relative in sorted(payload_paths):
            path = candidate / relative
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
    expected_artifact_count = sum(
        len(release_manifest.required_candidate_payload_paths(VERSION, target))
        + 1
        for target in release_manifest.TARGETS
    )
    assert decoded["artifact_count"] == expected_artifact_count
    assert len(decoded["artifacts"]) == expected_artifact_count

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


@pytest.mark.parametrize(
    "mutation",
    [
        "missing-target",
        "extra-root",
        "incomplete-sums",
        "missing-required",
        "unexpected-candidate-file",
        "empty-scan-evidence",
    ],
)
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
    elif mutation == "incomplete-sums":
        (candidate / "SHA256SUMS").write_text(
            (candidate / "SHA256SUMS").read_text(encoding="utf-8").splitlines(
                keepends=True
            )[0],
            encoding="utf-8",
        )
    elif mutation == "missing-required":
        (candidate / "ROLLBACK-YANK.md").unlink()
        checksum_lines = (
            candidate / "SHA256SUMS"
        ).read_text(encoding="utf-8").splitlines(keepends=True)
        (candidate / "SHA256SUMS").write_text(
            "".join(
                line
                for line in checksum_lines
                if not line.endswith("  ROLLBACK-YANK.md\n")
            ),
            encoding="utf-8",
        )
    elif mutation == "unexpected-candidate-file":
        unexpected = candidate / "unreviewed.bin"
        unexpected.write_bytes(b"not part of the release contract")
        digest = hashlib.sha256(unexpected.read_bytes()).hexdigest()
        with (candidate / "SHA256SUMS").open("a", encoding="utf-8") as handle:
            handle.write(f"{digest}  unreviewed.bin\n")
    else:
        empty = candidate / f"sorafs-cli-{target}.spdx.json"
        empty.write_bytes(b"")
        checksum_lines = (
            candidate / "SHA256SUMS"
        ).read_text(encoding="utf-8").splitlines(keepends=True)
        empty_relative = empty.relative_to(candidate).as_posix()
        empty_digest = hashlib.sha256(b"").hexdigest()
        (candidate / "SHA256SUMS").write_text(
            "".join(
                f"{empty_digest}  {empty_relative}\n"
                if line.endswith(f"  {empty_relative}\n")
                else line
                for line in checksum_lines
            ),
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


def test_manifest_rejects_release_wide_file_drift_across_targets(
    tmp_path: Path,
) -> None:
    candidates = tmp_path / "candidates"
    candidates.mkdir()
    _write_candidates(candidates)
    target = release_manifest.TARGETS[-1]
    candidate = candidates / f"sorafs-cli-{VERSION}-{target}"
    common_file = candidate / "version-map.toml"
    common_file.write_bytes(b"substituted target-specific version map\n")
    common_digest = hashlib.sha256(common_file.read_bytes()).hexdigest()
    checksum_lines = (
        candidate / "SHA256SUMS"
    ).read_text(encoding="utf-8").splitlines(keepends=True)
    (candidate / "SHA256SUMS").write_text(
        "".join(
            f"{common_digest}  version-map.toml\n"
            if line.endswith("  version-map.toml\n")
            else line
            for line in checksum_lines
        ),
        encoding="utf-8",
    )

    with pytest.raises(release_manifest.ManifestError, match="differ across"):
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
