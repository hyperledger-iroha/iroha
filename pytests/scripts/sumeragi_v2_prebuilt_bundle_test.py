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


def _replace_manifest(bundle: Path, data: bytes) -> str:
    manifest = bundle / ".sumeragi-v2-prebuilt-binaries.tsv"
    manifest.chmod(0o600)
    manifest.write_bytes(data)
    manifest.chmod(0o400)
    return hashlib.sha256(data).hexdigest()


def _encode_fields(fields: list[tuple[str, str]]) -> bytes:
    return "".join(f"{key}\t{value}\n" for key, value in fields).encode()


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


@pytest.mark.parametrize("kind", ("file", "directory"))
def test_validate_rejects_every_unexpected_bundle_entry(
    tmp_path: Path,
    kind: str,
) -> None:
    fixture = _fixture(tmp_path)
    bundle = Path(fixture["bundle"])
    bundle.chmod(0o700)
    unexpected = bundle / "unexpected"
    if kind == "file":
        unexpected.write_bytes(b"not attested\n")
    else:
        unexpected.mkdir()
    bundle.chmod(0o500)

    with pytest.raises(PrebuiltBundleError, match="unexpected entry"):
        validate_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            bundle,
            str(fixture["manifest_sha256"]),
        )


@pytest.mark.parametrize(
    "artifact",
    ("bundle", "nested_directory", "binary", "manifest"),
)
def test_validate_rejects_wrong_published_modes(
    tmp_path: Path,
    artifact: str,
) -> None:
    fixture = _fixture(tmp_path)
    bundle = Path(fixture["bundle"])
    targets = {
        "bundle": bundle,
        "nested_directory": bundle / "release",
        "binary": bundle / "release" / "iroha3d",
        "manifest": bundle / ".sumeragi-v2-prebuilt-binaries.tsv",
    }
    targets[artifact].chmod(0o700)

    with pytest.raises(PrebuiltBundleError, match="mode|regular file"):
        validate_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            bundle,
            str(fixture["manifest_sha256"]),
        )


def test_validate_rejects_symlinked_expected_directory(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    bundle = Path(fixture["bundle"])
    message_control = bundle / "message-control"
    release = message_control / "release"
    bundle.chmod(0o700)
    message_control.chmod(0o700)
    release.chmod(0o700)
    (release / "iroha3d").unlink()
    release.rmdir()
    release.symlink_to(bundle / "release", target_is_directory=True)
    message_control.chmod(0o500)
    bundle.chmod(0o500)

    with pytest.raises(PrebuiltBundleError, match="directory is not real"):
        validate_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            bundle,
            str(fixture["manifest_sha256"]),
        )


@pytest.mark.parametrize(
    "mutation",
    (
        "legacy_schema",
        "wrong_profile",
        "wrong_bundle",
        "wrong_host",
        "wrong_target",
        "wrong_relative_path",
        "wrong_binary_digest",
        "wrong_binary_size",
        "noncanonical_binary_size",
        "wrong_binary_mode",
        "reordered",
        "duplicate",
        "missing",
        "carriage_return",
        "nul",
        "invalid_utf8",
        "oversized",
    ),
)
def test_validate_rejects_noncanonical_or_forged_manifest_fields(
    tmp_path: Path,
    mutation: str,
) -> None:
    fixture = _fixture(tmp_path)
    repo = Path(fixture["repo"])
    bundle = Path(fixture["bundle"])
    fields = _manifest_fields(bundle)
    values = dict(fields)
    if mutation == "legacy_schema":
        values["schema_version"] = "1"
    elif mutation == "wrong_profile":
        values["profile"] = "debug"
    elif mutation == "wrong_bundle":
        values["bundle_dir"] = str(bundle.parent / "invocation.forged")
    elif mutation == "wrong_host":
        values["host_triple"] = "../host"
    elif mutation == "wrong_target":
        values["target_triple"] = "target with spaces"
    elif mutation == "wrong_relative_path":
        values["irohad_relative_path"] = "../Cargo.lock"
    elif mutation == "wrong_binary_digest":
        values["irohad_sha256"] = "0" * 64
    elif mutation == "wrong_binary_size":
        values["irohad_size_bytes"] = str(
            int(values["irohad_size_bytes"]) + 1
        )
    elif mutation == "noncanonical_binary_size":
        values["irohad_size_bytes"] = "00"
    elif mutation == "wrong_binary_mode":
        values["irohad_mode_octal"] = "0700"
    fields = [(key, values[key]) for key, _value in fields]
    if mutation == "reordered":
        fields[0], fields[1] = fields[1], fields[0]
    elif mutation == "duplicate":
        fields.append(fields[-1])
    elif mutation == "missing":
        fields.pop()
    data = _encode_fields(fields)
    if mutation == "carriage_return":
        data = data.replace(b"\n", b"\r\n", 1)
    elif mutation == "nul":
        data = data.replace(b"\n", b"\0\n", 1)
    elif mutation == "invalid_utf8":
        data = data.replace(b"\n", b"\xff\n", 1)
    elif mutation == "oversized":
        data += b"x" * (32 * 1024)
    manifest_sha256 = _replace_manifest(bundle, data)

    with pytest.raises(PrebuiltBundleError):
        validate_bundle(repo, SOURCE_MANIFEST, bundle, manifest_sha256)


def test_validate_rejects_source_or_lock_drift(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    repo = Path(fixture["repo"])
    bundle = Path(fixture["bundle"])
    manifest_sha256 = str(fixture["manifest_sha256"])

    with pytest.raises(PrebuiltBundleError):
        validate_bundle(repo, "b" * 64, bundle, manifest_sha256)

    (repo / "Cargo.lock").write_bytes(b"fixture-lock-v2\n")
    with pytest.raises(PrebuiltBundleError, match="Cargo.lock digest mismatch"):
        validate_bundle(repo, SOURCE_MANIFEST, bundle, manifest_sha256)


def test_validate_rejects_cross_bundle_manifest_replay(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    repo = Path(fixture["repo"])
    first = Path(fixture["bundle"])
    second, _second_digest = create_bundle(
        repo,
        SOURCE_MANIFEST,
        Path(fixture["default_cache"]),
        Path(fixture["message_cache"]),
        Path(fixture["programs"]),
        Path(fixture["cargo_version"]),
        Path(fixture["rustc_version"]),
    )
    first_manifest = first / ".sumeragi-v2-prebuilt-binaries.tsv"
    replayed_digest = _replace_manifest(second, first_manifest.read_bytes())

    with pytest.raises(PrebuiltBundleError, match="base identity mismatch"):
        validate_bundle(repo, SOURCE_MANIFEST, second, replayed_digest)


def test_validate_rejects_relative_bundle_path(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    bundle = Path(fixture["bundle"])
    relative = Path(os.path.relpath(bundle, Path.cwd()))
    with pytest.raises(PrebuiltBundleError, match="absolute and normalized"):
        validate_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            relative,
            str(fixture["manifest_sha256"]),
        )


@pytest.mark.parametrize(
    "mutation",
    ("empty", "no_newline", "carriage_return", "nul", "oversized", "symlink", "hardlink"),
)
def test_create_rejects_malformed_or_non_private_tool_stdout(
    tmp_path: Path,
    mutation: str,
) -> None:
    fixture = _fixture(tmp_path)
    cargo_version = Path(fixture["cargo_version"])
    if mutation == "empty":
        cargo_version.write_bytes(b"")
    elif mutation == "no_newline":
        cargo_version.write_bytes(b"cargo fixture")
    elif mutation == "carriage_return":
        cargo_version.write_bytes(b"cargo fixture\r\n")
    elif mutation == "nul":
        cargo_version.write_bytes(b"cargo\0fixture\n")
    elif mutation == "oversized":
        cargo_version.write_bytes(b"x" * (64 * 1024) + b"\n")
    elif mutation == "symlink":
        cargo_version.unlink()
        cargo_version.symlink_to(fixture["rustc_version"])
    else:
        alias = cargo_version.with_suffix(".alias")
        os.link(cargo_version, alias)

    with pytest.raises(PrebuiltBundleError):
        create_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            Path(fixture["default_cache"]),
            Path(fixture["message_cache"]),
            Path(fixture["programs"]),
            cargo_version,
            Path(fixture["rustc_version"]),
        )


def test_create_rejects_symlinked_build_output(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    output = Path(fixture["default_cache"]) / "release" / "iroha3d"
    output.unlink()
    output.symlink_to(Path(fixture["repo"]) / "Cargo.lock")

    with pytest.raises(PrebuiltBundleError, match="non-symlink"):
        create_bundle(
            Path(fixture["repo"]),
            SOURCE_MANIFEST,
            Path(fixture["default_cache"]),
            Path(fixture["message_cache"]),
            Path(fixture["programs"]),
            Path(fixture["cargo_version"]),
            Path(fixture["rustc_version"]),
        )
