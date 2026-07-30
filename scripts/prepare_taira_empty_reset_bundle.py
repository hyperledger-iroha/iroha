#!/usr/bin/env python3
"""Clone sealed Taira reset inputs into a bundle with brand-new empty storage."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import tempfile


PEER_COUNT = 4
TOP_LEVEL_FILES = (
    "genesis.signed.nrt",
    "genesis.json",
    "base-config.toml",
    "validator-roster.toml",
    "validator-secrets.toml",
    "reset-manifest.json",
)
VALIDATOR_TREES = ("codec", "runtime", "configs", "manifests")
RUNTIME_SIDECARS = (
    "onboarding-signer.key",
    "onboarding-token",
    "faucet-signer.key",
)
DEFAULT_MINIMUM_FREE_BYTES = 16 * 1024 * 1024 * 1024


def fail(message: str) -> "NoReturn":
    raise RuntimeError(message)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def require_minimum_free_space(path: Path, minimum_free_bytes: int) -> int:
    if minimum_free_bytes < 0:
        fail("minimum free bytes must be non-negative")
    free_bytes = shutil.disk_usage(path).free
    if free_bytes < minimum_free_bytes:
        fail(
            "insufficient free space for a Taira reset bundle: "
            f"{free_bytes} bytes available, {minimum_free_bytes} required"
        )
    return free_bytes


def require_sha256(value: str, name: str) -> str:
    if re.fullmatch(r"[0-9a-f]{64}", value) is None:
        fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def require_source_commit(value: str) -> str:
    if re.fullmatch(r"[0-9a-f]{40}", value) is None or value == "0" * 40:
        fail("source commit must be a nonzero lowercase Git object id")
    return value


def retarget_bundle_paths(
    encoded_config: bytes, source_bundle: Path, output_bundle: Path
) -> bytes:
    source_prefix = os.fsencode(source_bundle)
    output_prefix = os.fsencode(output_bundle)
    if source_prefix not in encoded_config:
        fail("rendered validator config does not reference its source bundle")
    retargeted = encoded_config.replace(source_prefix, output_prefix)
    if source_prefix in retargeted:
        fail("rendered validator config retained a source-bundle path")
    return retargeted


def require_private_directory(path: Path) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        fail(f"unsafe private directory identity: {path}")


def require_private_regular_file(path: Path) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        fail(f"unsafe private file identity: {path}")


def copy_private_file(source: Path, destination: Path) -> None:
    require_private_regular_file(source)
    write_private_file(destination, source.read_bytes())


def write_private_file(destination: Path, contents: bytes) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.",
        suffix=".tmp",
        dir=destination.parent,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as output_stream:
            output_stream.write(contents)
            output_stream.flush()
            os.fsync(output_stream.fileno())
        os.chmod(temporary, 0o600)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def copy_private_tree(source: Path, destination: Path) -> None:
    require_private_directory(source)
    destination.mkdir(mode=0o700)
    for current, directory_names, file_names in os.walk(
        source, followlinks=False
    ):
        directory_names.sort()
        file_names.sort()
        current_path = Path(current)
        relative = current_path.relative_to(source)
        output_directory = destination / relative
        output_directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chmod(output_directory, 0o700)
        for name in directory_names:
            input_directory = current_path / name
            require_private_directory(input_directory)
            output = output_directory / name
            output.mkdir(mode=0o700)
        for name in file_names:
            copy_private_file(current_path / name, output_directory / name)


def atomic_write_json(path: Path, payload: dict[str, object]) -> None:
    encoded = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o600)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("--source-bundle", type=Path, required=True)
    parser.add_argument("--output-bundle", type=Path, required=True)
    parser.add_argument("--irohad-sha256", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument(
        "--minimum-free-bytes",
        type=int,
        default=DEFAULT_MINIMUM_FREE_BYTES,
        help=(
            "fail before copying unless the output filesystem has at least "
            "this many free bytes"
        ),
    )
    args = parser.parse_args()
    irohad_sha256 = require_sha256(args.irohad_sha256, "irohad SHA-256")
    source_commit = require_source_commit(args.source_commit)

    source = args.source_bundle
    output = args.output_bundle
    if not source.is_absolute() or source.resolve(strict=True) != source:
        fail("source bundle must be an absolute canonical path")
    require_private_directory(source)
    if not output.is_absolute():
        fail("output bundle must be an absolute path")
    if output.exists() or output.is_symlink():
        fail("output bundle already exists")
    if output.parent.resolve(strict=True) != output.parent:
        fail("output parent must be canonical")
    require_private_directory(output.parent)
    free_bytes_before_copy = require_minimum_free_space(
        output.parent, args.minimum_free_bytes
    )

    output.mkdir(mode=0o700)
    try:
        for relative in TOP_LEVEL_FILES:
            copy_private_file(source / relative, output / relative)

        rendered_source = source / "rendered"
        rendered_output = output / "rendered"
        require_private_directory(rendered_source)
        rendered_output.mkdir(mode=0o700)
        copy_private_file(
            rendered_source / "genesis.json",
            rendered_output / "genesis.json",
        )

        for peer_index in range(1, PEER_COUNT + 1):
            slug = f"taira-validator-{peer_index}"
            validator_source = rendered_source / slug
            validator_output = rendered_output / slug
            require_private_directory(validator_source)
            validator_output.mkdir(mode=0o700)
            config_source = validator_source / "config.toml"
            require_private_regular_file(config_source)
            write_private_file(
                validator_output / "config.toml",
                retarget_bundle_paths(
                    config_source.read_bytes(), source, output
                ),
            )
            for tree in VALIDATOR_TREES:
                copy_private_tree(
                    validator_source / tree,
                    validator_output / tree,
                )
            storage = validator_output / "storage"
            storage.mkdir(mode=0o700)
            if any(storage.iterdir()):
                fail(f"new storage is not empty: {storage}")

        manifest_path = output / "reset-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if (
            not isinstance(manifest, dict)
            or manifest.get("schema") != "taira-exact2f-reset-bundle"
            or manifest.get("peer_count") != PEER_COUNT
        ):
            fail("unexpected reset manifest")

        slugs = [f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1)]
        manifest["signed_genesis_sha256"] = sha256(
            output / "genesis.signed.nrt"
        )
        manifest["irohad_sha256"] = irohad_sha256
        manifest["source_commit"] = source_commit
        manifest["base_config_sha256"] = sha256(output / "base-config.toml")
        manifest["configs"] = {
            slug: sha256(rendered_output / slug / "config.toml")
            for slug in slugs
        }
        manifest["governance_manifests"] = {
            slug: sha256(
                rendered_output / slug / "manifests/governance.manifest.json"
            )
            for slug in slugs
        }
        manifest["runtime_sidecars"] = {
            slug: {
                name: sha256(rendered_output / slug / "runtime" / name)
                for name in RUNTIME_SIDECARS
            }
            for slug in slugs
        }
        empty_tree_sha256 = hashlib.sha256().hexdigest()
        manifest["prewarmed_storage_sha256"] = {
            slug: empty_tree_sha256 for slug in slugs
        }
        atomic_write_json(manifest_path, manifest)

        for slug in slugs:
            storage = rendered_output / slug / "storage"
            if any(storage.iterdir()):
                fail(f"new storage became non-empty: {storage}")

        print(
            json.dumps(
                {
                    "bundle": str(output),
                    "empty_storage_sha256": empty_tree_sha256,
                    "free_bytes_before_copy": free_bytes_before_copy,
                    "irohad_sha256": irohad_sha256,
                    "peer_count": PEER_COUNT,
                },
                sort_keys=True,
            )
        )
    except BaseException:
        shutil.rmtree(output)
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
