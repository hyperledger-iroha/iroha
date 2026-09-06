#!/usr/bin/env python3
"""Authenticate and snapshot provenance-bound release binaries."""

from __future__ import annotations

import argparse
import json
import re
import stat
import sys
from pathlib import Path


_CONTRACT = sys.modules.get("release_artifact_contract")
if _CONTRACT is None:
    raise RuntimeError(
        "release prebuilt verifier must run through run_isolated_release_tool.py"
    )
ReleaseArtifactError = _CONTRACT.ReleaseArtifactError
canonical_json_bytes = _CONTRACT.canonical_json_bytes
create_fresh_directory = _CONTRACT.create_fresh_directory
exclusive_write_bytes = _CONTRACT.exclusive_write_bytes
scan_inventory_paths = _CONTRACT.scan_inventory_paths
stable_hash_path = _CONTRACT.stable_hash_path
stable_read_relative = _CONTRACT.stable_read_relative


MANIFEST_NAME = "release-prebuilt-provenance.json"
SCHEMA = "iroha.release_prebuilt_provenance"
SCHEMA_VERSION = 1
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_BINARY_BYTES = 1024 * 1024 * 1024
_HEX_SHA256 = re.compile(r"[0-9a-f]{64}")
_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SAFE_TOKEN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,127}")
_FEATURE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+/-]{0,127}")


def _fail(message: str) -> None:
    raise ReleaseArtifactError(message)


def parse_binary_specs(specs: list[str]) -> dict[str, str]:
    """Parse an exact unique ``binary=package`` inventory."""

    result: dict[str, str] = {}
    for spec in specs:
        name, separator, package = spec.partition("=")
        if (
            not separator
            or _SAFE_TOKEN.fullmatch(name) is None
            or _SAFE_TOKEN.fullmatch(package) is None
        ):
            _fail("--binary must be one bounded binary=package pair")
        if name in result:
            _fail(f"duplicate expected prebuilt binary: {name}")
        result[name] = package
    if not result:
        _fail("at least one --binary is required")
    return result


def parse_features(raw: str) -> tuple[str, ...]:
    """Parse one canonical comma-separated selected-feature set."""

    if not raw:
        return ()
    features = raw.split(",")
    if any(_FEATURE.fullmatch(feature) is None for feature in features):
        _fail("selected features must be bounded Cargo feature tokens")
    canonical = tuple(sorted(set(features)))
    if len(canonical) != len(features):
        _fail("selected features must not contain duplicates")
    return canonical


def verify_prebuilt_directory(
    directory: Path,
    *,
    trusted_manifest_sha256: str,
    source_commit: str,
    cargo_lock: Path,
    target: str,
    cargo_profile: str,
    selected_features: tuple[str, ...],
    binaries: dict[str, str],
    output_directory: Path,
) -> str:
    """Verify one authenticated provenance manifest and copy a private snapshot."""

    if _HEX_SHA256.fullmatch(trusted_manifest_sha256) is None:
        _fail("trusted provenance manifest SHA256 must be 64 lowercase hex")
    if _COMMIT.fullmatch(source_commit) is None:
        _fail("source commit must be one full lowercase hexadecimal identifier")
    if _SAFE_TOKEN.fullmatch(target) is None:
        _fail("target must be one bounded safe token")
    if cargo_profile != "deploy":
        _fail("prebuilt release provenance cargo profile must be deploy")
    if not binaries:
        _fail("prebuilt binary inventory must not be empty")

    inventory = scan_inventory_paths(directory)
    expected_inventory = sorted((MANIFEST_NAME, *binaries))
    if inventory != expected_inventory:
        _fail(
            "prebuilt directory inventory must contain exactly the provenance "
            "manifest and expected binaries"
        )

    manifest_info, manifest_payload = stable_read_relative(
        directory,
        MANIFEST_NAME,
        max_size=MAX_MANIFEST_BYTES,
        return_payload=True,
    )
    assert manifest_payload is not None
    if manifest_info.sha256 != trusted_manifest_sha256:
        _fail("prebuilt provenance manifest SHA256 is not the reviewed digest")
    try:
        manifest = json.loads(manifest_payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseArtifactError(
            f"prebuilt provenance manifest is not canonical UTF-8 JSON: {error}"
        ) from error
    if canonical_json_bytes(manifest) != manifest_payload:
        _fail("prebuilt provenance manifest must use canonical JSON encoding")
    if not isinstance(manifest, dict) or set(manifest) != {
        "schema",
        "schema_version",
        "source_commit",
        "cargo_lock_sha256",
        "target",
        "cargo_profile",
        "default_features",
        "selected_features",
        "binaries",
    }:
        _fail("prebuilt provenance manifest fields do not match schema v1")
    if type(manifest["schema_version"]) is not int:  # noqa: E721 - exact JSON type
        _fail("prebuilt provenance manifest schema_version must be an integer")
    if type(manifest["default_features"]) is not bool:  # noqa: E721 - exact JSON type
        _fail("prebuilt provenance manifest default_features must be a boolean")

    cargo_lock_info = stable_hash_path(cargo_lock)
    expected_metadata = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "source_commit": source_commit,
        "cargo_lock_sha256": cargo_lock_info.sha256,
        "target": target,
        "cargo_profile": cargo_profile,
        "default_features": True,
        "selected_features": list(selected_features),
    }
    for key, expected in expected_metadata.items():
        if manifest.get(key) != expected:
            _fail(f"prebuilt provenance manifest {key} does not match release input")

    rows = manifest["binaries"]
    if not isinstance(rows, list) or len(rows) != len(binaries):
        _fail("prebuilt provenance binary inventory is incomplete")
    expected_names = sorted(binaries)
    observed_names: list[str] = []
    snapshot_payloads: list[tuple[str, bytes]] = []
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "name",
            "package",
            "sha256",
            "size",
        }:
            _fail("prebuilt provenance binary row fields do not match schema v1")
        name = row["name"]
        package = row["package"]
        sha256 = row["sha256"]
        size = row["size"]
        if not isinstance(name, str) or name not in binaries:
            _fail("prebuilt provenance names an unexpected binary")
        if package != binaries[name]:
            _fail(f"prebuilt provenance package does not match binary {name}")
        if not isinstance(sha256, str) or _HEX_SHA256.fullmatch(sha256) is None:
            _fail(f"prebuilt provenance SHA256 is invalid for binary {name}")
        if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
            _fail(f"prebuilt provenance size is invalid for binary {name}")
        info, payload = stable_read_relative(
            directory,
            name,
            max_size=MAX_BINARY_BYTES,
            return_payload=True,
        )
        assert payload is not None
        if not info.mode & stat.S_IXUSR:
            _fail(f"prebuilt binary must be owner-executable: {name}")
        if info.sha256 != sha256 or info.size != size:
            _fail(f"prebuilt binary does not match authenticated provenance: {name}")
        observed_names.append(name)
        snapshot_payloads.append((name, payload))
    if observed_names != expected_names:
        _fail("prebuilt provenance binary rows must be unique and sorted by name")

    snapshot_root = create_fresh_directory(output_directory, mode=0o700)
    for name, payload in snapshot_payloads:
        exclusive_write_bytes(snapshot_root / name, payload, mode=0o755)
    return manifest_info.sha256


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True)
    parser.add_argument("--trusted-manifest-sha256", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--cargo-lock", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--cargo-profile", required=True)
    parser.add_argument("--features", default="")
    parser.add_argument("--binary", action="append", default=[])
    parser.add_argument("--output-directory", required=True)
    args = parser.parse_args()
    try:
        digest = verify_prebuilt_directory(
            Path(args.directory),
            trusted_manifest_sha256=args.trusted_manifest_sha256,
            source_commit=args.source_commit,
            cargo_lock=Path(args.cargo_lock),
            target=args.target,
            cargo_profile=args.cargo_profile,
            selected_features=parse_features(args.features),
            binaries=parse_binary_specs(args.binary),
            output_directory=Path(args.output_directory),
        )
    except (OSError, ReleaseArtifactError) as error:
        print(f"release prebuilt provenance verification failed: {error}", file=sys.stderr)
        return 1
    print(digest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
