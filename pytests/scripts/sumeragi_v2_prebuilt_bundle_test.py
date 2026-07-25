"""Contract tests for private Sumeragi v2 release-binary bundles."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat

import pytest

from scripts.sumeragi_v2_prebuilt_bundle import (
    PrebuiltBundleError,
    create_bundle,
    prepare_cache,
    validate_bundle,
)


SOURCE_MANIFEST = "a" * 64
RELATIVE_BINARIES = (
    ("irohad", "release/iroha3d", "default"),
    (
        "irohad_message_control",
        "message-control/release/iroha3d",
        "message-control",
    ),
    ("iroha", "release/iroha", "default"),
    ("kagami", "release/kagami", "default"),
)


def _fixture(tmp_path: Path) -> dict[str, object]:
    repo = tmp_path.resolve() / "repo"
    repo.mkdir()
    (repo / "Cargo.lock").write_bytes(b"fixture-lock-v1\n")
    source_root = repo / "target" / "sumeragi-v2-release" / SOURCE_MANIFEST
    default_cache = source_root / "program-build-cache" / "default"
    message_cache = source_root / "program-build-cache" / "message-control"
    prepare_cache(repo, SOURCE_MANIFEST, default_cache, message_cache)
    for label, relative, cache_name in RELATIVE_BINARIES:
        cache = default_cache if cache_name == "default" else message_cache
        source_relative = (
            Path(relative)
            if cache_name == "default"
            else Path("release") / Path(relative).name
        )
        binary = cache / source_relative
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_bytes(f"#!/bin/sh\n# {label}\n".encode())
        binary.chmod(0o755)
    versions = source_root / "versions"
    versions.mkdir()
    cargo_version = versions / "cargo.txt"
    rustc_version = versions / "rustc.txt"
    cargo_version.write_bytes(b"cargo 1.99.0 (fixture)\n")
    rustc_version.write_bytes(
        b"rustc 1.99.0 (fixture)\n"
        b"binary: rustc\n"
        b"commit-hash: fixture\n"
        b"commit-date: 2099-01-01\n"
        b"host: fixture-host\n"
        b"release: 1.99.0\n"
        b"LLVM version: 99.0.0\n"
    )
    programs = source_root / "programs"
    bundle, manifest_sha256 = create_bundle(
        repo,
        SOURCE_MANIFEST,
        default_cache,
        message_cache,
        programs,
        cargo_version,
        rustc_version,
    )
    return {
        "repo": repo,
        "default_cache": default_cache,
        "message_cache": message_cache,
        "programs": programs,
        "cargo_version": cargo_version,
        "rustc_version": rustc_version,
        "bundle": bundle,
        "manifest_sha256": manifest_sha256,
    }


def _manifest_fields(bundle: Path) -> list[tuple[str, str]]:
    return [
        tuple(line.split("\t", 1))  # type: ignore[misc]
        for line in (bundle / ".sumeragi-v2-prebuilt-binaries.tsv")
        .read_text(encoding="utf-8")
        .splitlines()
    ]


def test_create_publishes_exact_v2_manifest_and_read_only_single_link_bundle(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    repo = fixture["repo"]
    bundle = fixture["bundle"]
    manifest_sha256 = fixture["manifest_sha256"]
    assert isinstance(repo, Path)
    assert isinstance(bundle, Path)
    assert isinstance(manifest_sha256, str)

    fields = _manifest_fields(bundle)
    assert len(fields) == 25
    values = dict(fields)
    assert fields[:9] == [
        ("schema_version", "2"),
        ("source_manifest_sha256", SOURCE_MANIFEST),
        (
            "cargo_lock_sha256",
            hashlib.sha256((repo / "Cargo.lock").read_bytes()).hexdigest(),
        ),
        (
            "cargo_version_sha256",
            hashlib.sha256(b"cargo 1.99.0 (fixture)\n").hexdigest(),
        ),
        (
            "rustc_version_sha256",
            hashlib.sha256(
                Path(fixture["rustc_version"]).read_bytes()
            ).hexdigest(),
        ),
        ("host_triple", "fixture-host"),
        ("target_triple", "fixture-host"),
        ("profile", "release"),
        ("bundle_dir", str(bundle)),
    ]
    assert bundle.name.startswith("invocation.")
    assert stat.S_IMODE(bundle.stat().st_mode) == 0o500
    manifest = bundle / ".sumeragi-v2-prebuilt-binaries.tsv"
    assert stat.S_IMODE(manifest.stat().st_mode) == 0o400
    assert manifest.stat().st_nlink == 1
    assert hashlib.sha256(manifest.read_bytes()).hexdigest() == manifest_sha256
    for label, relative, _cache_name in RELATIVE_BINARIES:
        binary = bundle / relative
        assert values[f"{label}_relative_path"] == relative
        assert values[f"{label}_mode_octal"] == "0500"
        assert values[f"{label}_size_bytes"] == str(binary.stat().st_size)
        assert values[f"{label}_sha256"] == hashlib.sha256(
            binary.read_bytes()
        ).hexdigest()
        assert stat.S_IMODE(binary.stat().st_mode) == 0o500
        assert binary.stat().st_nlink == 1
        current = bundle
        for component in Path(relative).parts[:-1]:
            current /= component
            assert stat.S_IMODE(current.stat().st_mode) == 0o500

    validate_bundle(repo, SOURCE_MANIFEST, bundle, manifest_sha256)


def test_create_always_allocates_a_fresh_invocation_bundle(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    second, second_digest = create_bundle(
        Path(fixture["repo"]),
        SOURCE_MANIFEST,
        Path(fixture["default_cache"]),
        Path(fixture["message_cache"]),
        Path(fixture["programs"]),
        Path(fixture["cargo_version"]),
        Path(fixture["rustc_version"]),
    )
    assert second != fixture["bundle"]
    assert second.parent == fixture["programs"]
    assert second_digest == hashlib.sha256(
        (second / ".sumeragi-v2-prebuilt-binaries.tsv").read_bytes()
    ).hexdigest()


def test_validate_rejects_forged_external_manifest_anchor(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    with pytest.raises(
        PrebuiltBundleError,
        match="does not match the inherited anchor",
    ):
        validate_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            Path(fixture["bundle"]),
            "0" * 64,
        )


@pytest.mark.parametrize("mutation", ("binary", "symlink", "hardlink", "manifest"))
def test_validate_rejects_mutated_or_non_private_artifacts(
    tmp_path: Path,
    mutation: str,
) -> None:
    fixture = _fixture(tmp_path)
    repo = Path(fixture["repo"])
    bundle = Path(fixture["bundle"])
    manifest_sha256 = str(fixture["manifest_sha256"])
    binary = bundle / "release" / "iroha3d"
    release_dir = binary.parent
    bundle.chmod(0o700)
    release_dir.chmod(0o700)
    if mutation == "binary":
        binary.chmod(0o700)
        binary.write_bytes(b"tampered\n")
        binary.chmod(0o500)
    elif mutation == "symlink":
        binary.unlink()
        binary.symlink_to(repo / "Cargo.lock")
    elif mutation == "hardlink":
        binary.chmod(0o700)
        alias = release_dir / "alias"
        os.link(binary, alias)
        binary.chmod(0o500)
    else:
        manifest = bundle / ".sumeragi-v2-prebuilt-binaries.tsv"
        manifest.chmod(0o600)
        manifest.write_bytes(manifest.read_bytes() + b"unexpected\tfield\n")
        manifest.chmod(0o400)
        manifest_sha256 = hashlib.sha256(manifest.read_bytes()).hexdigest()
    release_dir.chmod(0o500)
    bundle.chmod(0o500)

    with pytest.raises(PrebuiltBundleError):
        validate_bundle(repo, SOURCE_MANIFEST, bundle, manifest_sha256)
