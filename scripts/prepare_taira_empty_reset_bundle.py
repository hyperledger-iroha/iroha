#!/usr/bin/env python3
"""Compose a fresh four-peer Taira reset from an authenticated privacy release.

The sealed source reset contributes only its exact public roster, owner-private
validator/shared secrets, and static codec/config sidecars.  The authenticated
Linux release contributes the reviewed privacy plan, peer-1 config template,
genesis template, and broker public export.  An independently provisioned,
digest-pinned external software signer binds and signs that genesis. The controller
never reads, snapshots, or passes persistent genesis private-key material.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from pathlib import Path
from typing import Any, BinaryIO, NoReturn

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import extract_authenticated_taira_privacy_release as privacy_release
    from . import render_taira_validator_bundle as renderer
    from . import seal_taira_release_controllers as controller_seal
    from . import taira_constants
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_output_fd,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
    )
except ImportError:
    import extract_authenticated_taira_privacy_release as privacy_release
    import render_taira_validator_bundle as renderer
    import seal_taira_release_controllers as controller_seal
    import taira_constants
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_output_fd,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
    )


PEER_COUNT = taira_constants.PEER_COUNT
SLUGS = taira_constants.SLUGS
CHAIN_ID = taira_constants.CHAIN_ID
CHAIN_DISCRIMINANT = taira_constants.CHAIN_DISCRIMINANT
SOURCE_TOP_LEVEL_NAMES = {
    "genesis.signed.nrt",
    "genesis.json",
    "base-config.toml",
    "validator-roster.toml",
    "validator-secrets.toml",
    "reset-manifest.json",
    "rendered",
}
SOURCE_VALIDATOR_NAMES = {
    "codec",
    "config.toml",
    "configs",
    "manifests",
    "runtime",
    "storage",
}
OUTPUT_TOP_LEVEL_NAMES = SOURCE_TOP_LEVEL_NAMES - {"validator-secrets.toml"}
STATIC_TREES = ("codec", "configs")
RUNTIME_SIDECARS = (
    "onboarding-signer.key",
    "onboarding-token",
    "faucet-signer.key",
)
DEFAULT_MINIMUM_FREE_BYTES = 16 * 1024 * 1024 * 1024
MAX_PRIVATE_FILE_BYTES = 64 * 1024 * 1024
MAX_RESET_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_CONFIG_BYTES = 4 * 1024 * 1024
MAX_GENESIS_SIGNER_OUTPUT_BYTES = 2 * 1024 * 1024
MAX_NATIVE_TOOL_BYTES = 512 * 1024 * 1024
MAX_SOURCE_BUNDLE_FILES = 16_384
MAX_SOURCE_BUNDLE_BYTES = 2 * 1024 * 1024 * 1024
SOURCE_BUNDLE_DIGEST_SCHEMA = "iroha.taira.private-reset-source.inventory.v1"
GENESIS_PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}")
KAGEMUSHA_IMMUTABLE_ACTIVATION_PERMISSIONS = frozenset(
    {
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
    }
)
KAGEMUSHA_CONFIG_PROJECTION_SCHEMA = "iroha.taira.kagemusha-config-projection.v1"
KAGEMUSHA_MANAGED_OFFLINE_FIELDS = frozenset(
    {
        "kagemusha_release_policy_path",
        "kagemusha_artifact_dir",
        "kagemusha_catalog_qualification_seal_path",
        "kagemusha_max_decoded_bytes",
    }
)


def fail(message: str) -> NoReturn:
    raise RuntimeError(message)


def sha256(path: Path) -> str:
    return stable_hash_path(path).sha256


def _require_kagemusha_activation_authority_permissions(
    genesis_payload: bytes,
    authority: str | None,
) -> str:
    """Require both immutable Kagemusha grants on one explicit genesis authority."""

    if (
        not isinstance(authority, str)
        or authority.strip() != authority
        or not authority
        or len(authority.encode("utf-8")) > 1024
        or any(ord(character) < 0x20 for character in authority)
    ):
        fail(
            "--kagemusha-activation-authority must be the exact nonempty canonical "
            "genesis account id"
        )
    genesis = _strict_json(genesis_payload, "authenticated privacy genesis")
    transactions = genesis.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        fail("authenticated privacy genesis has no transaction instruction stream")
    effective_permissions: set[str] = set()
    for transaction in transactions:
        if not isinstance(transaction, dict):
            fail("authenticated privacy genesis contains a malformed transaction")
        instructions = transaction.get("instructions")
        if not isinstance(instructions, list):
            fail("authenticated privacy genesis contains a malformed instruction stream")
        for instruction in instructions:
            if not isinstance(instruction, dict) or len(instruction) != 1:
                continue
            operation, body = next(iter(instruction.items()))
            if operation not in {"Grant", "Revoke"} or not isinstance(body, dict):
                continue
            permission = body.get("Permission")
            if not isinstance(permission, dict) or permission.get("destination") != authority:
                continue
            permission_object = permission.get("object")
            if not isinstance(permission_object, dict):
                continue
            name = permission_object.get("name")
            if (
                name not in KAGEMUSHA_IMMUTABLE_ACTIVATION_PERMISSIONS
                or permission_object.get("payload") is not None
            ):
                continue
            if operation == "Grant":
                effective_permissions.add(name)
            else:
                effective_permissions.discard(name)
    missing = sorted(
        KAGEMUSHA_IMMUTABLE_ACTIVATION_PERMISSIONS - effective_permissions
    )
    if missing:
        fail(
            "authenticated privacy genesis does not grant the explicit Kagemusha "
            f"activation authority `{authority}` both immutable permissions; missing: "
            + ", ".join(missing)
        )
    return authority


def _kagemusha_release_policy_sha256(release_root: Path) -> str:
    artifact_dir = release_root / renderer.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
    qualification_seal = (
        release_root / renderer.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
    )
    if artifact_dir.exists() or artifact_dir.is_symlink():
        fail(
            "Kagemusha artifact directory must not exist before exact-network release generation"
        )
    if qualification_seal.exists() or qualification_seal.is_symlink():
        fail("Kagemusha qualification seal must not exist before genesis signing")
    policy = release_root / renderer.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    try:
        return stable_hash_path(policy, max_size=MAX_CONFIG_BYTES).sha256
    except (OSError, ReleaseArtifactError) as error:
        raise RuntimeError(
            "configured Kagemusha release policy must exist as one stable canonical file "
            "before genesis signing"
        ) from error


def _kagemusha_config_projection(release_root: Path) -> dict[str, object]:
    """Return the canonical final runtime projection derived from one release root."""

    return {
        "schema": KAGEMUSHA_CONFIG_PROJECTION_SCHEMA,
        "release_root": str(release_root),
        "release_policy_path": str(
            release_root / renderer.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
        ),
        "artifact_dir": str(
            release_root / renderer.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
        ),
        "catalog_qualification_seal_path": str(
            release_root / renderer.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
        ),
        "max_decoded_bytes": renderer.KAGEMUSHA_MAX_DECODED_BYTES,
    }


def _kagemusha_config_projection_sha256(release_root: Path) -> str:
    """Hash the canonical final Kagemusha runtime projection."""

    return hashlib.sha256(
        canonical_json_bytes(_kagemusha_config_projection(release_root))
    ).hexdigest()


def _require_rendered_kagemusha_config_projection(
    output: Path,
    release_root: Path | None,
    *,
    include_qualification_seal: bool,
) -> dict[str, object] | None:
    """Require the exact same managed Kagemusha projection on all four peers."""

    final_projection = (
        _kagemusha_config_projection(release_root)
        if release_root is not None
        else None
    )
    expected_offline: dict[str, object] = {}
    if final_projection is not None:
        expected_offline = {
            "kagemusha_release_policy_path": final_projection[
                "release_policy_path"
            ],
            "kagemusha_artifact_dir": final_projection["artifact_dir"],
            "kagemusha_max_decoded_bytes": final_projection[
                "max_decoded_bytes"
            ],
        }
        if include_qualification_seal:
            expected_offline["kagemusha_catalog_qualification_seal_path"] = (
                final_projection["catalog_qualification_seal_path"]
            )

    for slug in SLUGS:
        config_path = output / "rendered" / slug / "config.toml"
        config = renderer._load_toml(config_path)
        settlement = config.get("settlement")
        offline = (
            settlement.get("offline") if isinstance(settlement, dict) else None
        )
        actual_offline = offline if isinstance(offline, dict) else {}
        managed = {
            key: actual_offline[key]
            for key in KAGEMUSHA_MANAGED_OFFLINE_FIELDS
            if key in actual_offline
        }
        if managed != expected_offline:
            fail(
                f"rendered {slug} config does not carry the exact managed "
                "Kagemusha release projection"
            )
    return final_projection


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


def _require_canonical(path: Path, label: str, *, directory: bool) -> None:
    if not path.is_absolute():
        fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as exc:
        raise RuntimeError(f"cannot inspect {label}: {exc}") from exc
    expected = stat.S_ISDIR if directory else stat.S_ISREG
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not expected(metadata.st_mode)
    ):
        fail(
            f"{label} must be a canonical non-symlink {'directory' if directory else 'file'}"
        )


def require_private_directory(path: Path) -> None:
    _require_canonical(path, f"private directory {path}", directory=True)
    metadata = path.lstat()
    if (
        metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        fail(f"unsafe private directory identity: {path}")


def require_private_regular_file(path: Path) -> None:
    _require_canonical(path, f"private file {path}", directory=False)
    metadata = path.lstat()
    if (
        metadata.st_nlink != 1
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        fail(f"unsafe private file identity: {path}")


def read_private_file(path: Path, maximum_bytes: int = MAX_PRIVATE_FILE_BYTES) -> bytes:
    require_private_regular_file(path)
    info, payload = stable_read_path(path, max_size=maximum_bytes)
    if info.size <= 0:
        fail(f"private file must be non-empty: {path}")
    return payload


def _strict_json(payload: bytes, label: str) -> dict[str, object]:
    def object_from_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                fail(f"{label} repeats JSON field {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(payload.decode("utf-8"), object_pairs_hook=object_from_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"{label} is not strict UTF-8 JSON") from exc
    if not isinstance(value, dict):
        fail(f"{label} must be a JSON object")
    return value


def _require_exact_names(path: Path, expected: set[str], label: str) -> None:
    require_private_directory(path)
    actual = set(os.listdir(path))
    if actual != expected:
        fail(
            f"{label} inventory is not exact: expected={sorted(expected)}, actual={sorted(actual)}"
        )


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


def copy_private_file(source: Path, destination: Path) -> None:
    write_private_file(destination, read_private_file(source))


def copy_private_tree(source: Path, destination: Path) -> None:
    require_private_directory(source)
    if destination.exists() or destination.is_symlink():
        fail(f"private tree destination already exists: {destination}")
    destination.mkdir(mode=0o700)
    for current, directory_names, file_names in os.walk(source, followlinks=False):
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
            (output_directory / name).mkdir(mode=0o700, exist_ok=True)
        for name in file_names:
            copy_private_file(current_path / name, output_directory / name)


def atomic_write_json(path: Path, payload: dict[str, object]) -> None:
    encoded = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    write_private_file(path, encoded)


def _load_authenticated_privacy_release(
    root: Path,
    *,
    source_commit: str,
    dpn_validator_release_commit: str,
    cargo_lock_sha256: str,
    workspace_source_manifest_sha256: str,
) -> tuple[dict[str, bytes], dict[str, object], str]:
    require_private_directory(root)
    expected_names = {privacy_release.OUTPUT_MANIFEST, *privacy_release.PRIVACY_INPUTS}
    try:
        inventory = scan_inventory_paths(root)
    except ReleaseArtifactError as exc:
        raise RuntimeError(
            f"cannot inspect authenticated privacy release: {exc}"
        ) from exc
    if inventory != sorted(expected_names):
        fail("authenticated privacy release inventory is not exact")
    manifest_path = root / privacy_release.OUTPUT_MANIFEST
    manifest_payload = read_private_file(manifest_path, MAX_RESET_MANIFEST_BYTES)
    manifest = _strict_json(manifest_payload, "authenticated privacy release manifest")
    if canonical_json_bytes(manifest) != manifest_payload:
        fail("authenticated privacy release manifest is not canonical JSON")
    if (
        manifest.get("schema") != privacy_release.SCHEMA
        or manifest.get("schema_version") != privacy_release.SCHEMA_VERSION
        or set(manifest)
        != {
            "schema",
            "schema_version",
            "source",
            "linux_archive",
            "authority",
            "rollout_manifest_sha256",
            "privacy_inputs",
        }
    ):
        fail("authenticated privacy release manifest schema is not exact")
    expected_source = {
        "commit": source_commit,
        "dpn_validator_release_commit": dpn_validator_release_commit,
        "cargo_lock_sha256": cargo_lock_sha256,
        "workspace_source_manifest_sha256": workspace_source_manifest_sha256,
    }
    if manifest.get("source") != expected_source:
        fail("authenticated privacy release source differs from the macOS build source")
    linux_archive = manifest.get("linux_archive")
    if not isinstance(linux_archive, dict) or set(linux_archive) != {
        "name",
        "sha256",
        "size",
    }:
        fail("authenticated privacy release archive identity is not exact")
    archive_name = linux_archive.get("name")
    archive_size = linux_archive.get("size")
    if (
        not isinstance(archive_name, str)
        or Path(archive_name).name != archive_name
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*\.tar\.gz", archive_name) is None
        or not archive_name.endswith(".tar.gz")
        or isinstance(archive_size, bool)
        or not isinstance(archive_size, int)
        or archive_size <= 0
    ):
        fail("authenticated privacy release archive identity is invalid")
    require_sha256(str(linux_archive.get("sha256")), "Linux archive SHA-256")
    authority = manifest.get("authority")
    if not isinstance(authority, dict) or set(authority) != {
        "manifest_sha256",
        "native_verifier_sha256",
        "signer_fingerprint_sha256",
    }:
        fail("authenticated privacy release authority identity is not exact")
    for field in sorted(authority):
        require_sha256(str(authority[field]), f"authority {field}")
    require_sha256(
        str(manifest.get("rollout_manifest_sha256")), "rollout manifest SHA-256"
    )
    rows = manifest.get("privacy_inputs")
    if not isinstance(rows, dict) or set(rows) != set(privacy_release.PRIVACY_INPUTS):
        fail("authenticated privacy release does not bind exactly four inputs")
    payloads: dict[str, bytes] = {}
    for name, contract in privacy_release.PRIVACY_INPUTS.items():
        row = rows.get(name)
        if not isinstance(row, dict) or set(row) != {"rollout_path", "sha256", "size"}:
            fail(f"authenticated privacy input row is not exact: {name}")
        if row.get("rollout_path") != contract["rollout_path"]:
            fail(f"authenticated privacy input path changed: {name}")
        digest = require_sha256(str(row.get("sha256")), f"{name} SHA-256")
        size = row.get("size")
        if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
            fail(f"authenticated privacy input size is invalid: {name}")
        payload = read_private_file(root / name, int(contract["max_bytes"]))
        if len(payload) != size or hashlib.sha256(payload).hexdigest() != digest:
            fail(f"authenticated privacy input differs from its manifest: {name}")
        payloads[name] = payload
    return payloads, manifest, hashlib.sha256(manifest_payload).hexdigest()


def _load_source_manifest(source: Path) -> dict[str, object]:
    _require_exact_names(source, SOURCE_TOP_LEVEL_NAMES, "sealed source reset")
    rendered = source / "rendered"
    _require_exact_names(rendered, {"genesis.json", *SLUGS}, "sealed rendered reset")
    for slug in SLUGS:
        _require_exact_names(
            rendered / slug, SOURCE_VALIDATOR_NAMES, f"sealed {slug} runtime"
        )
    manifest = _strict_json(
        read_private_file(source / "reset-manifest.json", MAX_RESET_MANIFEST_BYTES),
        "sealed reset manifest",
    )
    required = {
        "schema": "taira-exact2f-reset-bundle",
        "peer_count": PEER_COUNT,
        "chain_id": CHAIN_ID,
        "chain_discriminant": CHAIN_DISCRIMINANT,
        "node_storage_budget_bytes": 68_719_476_736,
        "node_storage_budget_weights": {
            "kura_blocks_bps": 7499,
            "wsv_snapshots_bps": 2000,
            "sorafs_bps": 1,
            "soranet_spool_bps": 250,
            "soravpn_spool_bps": 250,
        },
        "nexus_storage_budget_policy": "bounded-64-gib-per-validator",
    }
    for key, expected in required.items():
        if manifest.get(key) != expected:
            fail(f"sealed reset manifest changed mandatory field {key!r}")
    return manifest


def _source_bundle_inventory(
    source: Path,
) -> tuple[list[tuple[str, object]], str]:
    """Capture the complete meaningful source tree and its protected digest."""

    require_private_directory(source)
    try:
        paths = scan_inventory_paths(source)
    except ReleaseArtifactError as exc:
        raise RuntimeError(f"cannot inspect sealed source reset: {exc}") from exc
    if not paths or len(paths) > MAX_SOURCE_BUNDLE_FILES:
        fail("sealed source reset file count is outside the release bound")

    captures: list[tuple[str, object]] = []
    rows: list[dict[str, object]] = []
    total_bytes = 0
    for relative in paths:
        try:
            info = stable_hash_relative(
                source,
                relative,
                max_size=MAX_PRIVATE_FILE_BYTES,
            )
        except ReleaseArtifactError as exc:
            raise RuntimeError(
                f"cannot pin sealed source reset file {relative!r}: {exc}"
            ) from exc
        if info.mode != 0o600 or info.link_count != 1:
            fail(
                f"sealed source reset file is not exact owner-private mode 0600: {relative}"
            )
        total_bytes += info.size
        if total_bytes > MAX_SOURCE_BUNDLE_BYTES:
            fail("sealed source reset exceeds the aggregate release bound")
        captures.append((relative, info))
        rows.append(
            {
                "path": relative,
                "sha256": info.sha256,
                "size": info.size,
                "mode": info.mode,
            }
        )
    digest_payload = {
        "schema": SOURCE_BUNDLE_DIGEST_SCHEMA,
        "files": rows,
    }
    digest = hashlib.sha256(canonical_json_bytes(digest_payload)).hexdigest()
    return captures, digest


def source_bundle_sha256(source: Path) -> str:
    """Return the closed, path-and-mode-bound digest used by release operators."""

    _load_source_manifest(source)
    _, digest = _source_bundle_inventory(source)
    return digest


def _snapshot_authenticated_source_bundle(
    source: Path,
    destination: Path,
    expected_sha256: str,
) -> tuple[dict[str, object], str]:
    """Authenticate then copy source bytes through pinned descriptors before use."""

    expected_sha256 = require_sha256(
        expected_sha256, "sealed source reset bundle SHA-256"
    )
    _load_source_manifest(source)
    captures, captured_sha256 = _source_bundle_inventory(source)
    if captured_sha256 != expected_sha256:
        fail("sealed source reset bundle differs from its protected inventory digest")
    if destination.exists() or destination.is_symlink():
        fail("sealed source reset snapshot destination already exists")
    destination.mkdir(mode=0o700)

    for relative, expected in captures:
        output_path = destination / relative
        output_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        current = output_path.parent
        while current != destination.parent:
            os.chmod(current, 0o700)
            if current == destination:
                break
            current = current.parent
        try:
            with stable_open_relative(source, relative, expected=expected) as input_fd:
                opened = os.fstat(input_fd)
                if (
                    opened.st_uid != os.getuid()
                    or opened.st_gid != os.getgid()
                    or stat.S_IMODE(opened.st_mode) != 0o600
                    or opened.st_nlink != 1
                ):
                    fail(f"sealed source reset file identity is unsafe: {relative}")
                with exclusive_output_fd(output_path, mode=0o600) as output_fd:
                    while chunk := os.read(input_fd, 1024 * 1024):
                        view = memoryview(chunk)
                        while view:
                            written = os.write(output_fd, view)
                            if written <= 0:
                                fail(
                                    "short write while snapshotting sealed source reset"
                                )
                            view = view[written:]
        except ReleaseArtifactError as exc:
            raise RuntimeError(
                f"sealed source reset changed during snapshot: {relative}: {exc}"
            ) from exc
        copied = stable_hash_path(output_path, max_size=MAX_PRIVATE_FILE_BYTES)
        if (
            copied.sha256 != expected.sha256
            or copied.size != expected.size
            or copied.mode != 0o600
            or copied.link_count != 1
        ):
            fail(f"sealed source reset snapshot differs for {relative}")

    rendered = destination / "rendered"
    rendered.mkdir(mode=0o700, exist_ok=True)
    for slug in SLUGS:
        peer = rendered / slug
        peer.mkdir(mode=0o700, exist_ok=True)
        for name in SOURCE_VALIDATOR_NAMES - {"config.toml"}:
            (peer / name).mkdir(mode=0o700, exist_ok=True)
    source_manifest = _load_source_manifest(destination)
    _, snapshot_sha256 = _source_bundle_inventory(destination)
    if snapshot_sha256 != expected_sha256:
        fail("sealed source reset snapshot inventory digest changed")
    return source_manifest, snapshot_sha256


def _executable_identity(path: Path, label: str) -> tuple[object, ...]:
    _require_canonical(path, label, directory=False)
    info = path.lstat()
    if info.st_nlink != 1 or info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        fail(f"{label} must have one link and no group/world write permission")
    if not os.access(path, os.X_OK):
        fail(f"{label} is not executable")
    stable = stable_hash_path(path, max_size=MAX_NATIVE_TOOL_BYTES)
    return (
        stable.sha256,
        stable.size,
        stable.mode,
        stable.device,
        stable.inode,
        stable.mtime_ns,
        stable.ctime_ns,
        stable.link_count,
    )


def _snapshot_executable(
    source: Path,
    destination: Path,
    expected_identity: tuple[object, ...],
    label: str,
) -> tuple[object, ...]:
    """Pin one native-tool inode, copy its bytes, and return snapshot identity."""

    source_info = stable_hash_path(source, max_size=MAX_NATIVE_TOOL_BYTES)
    source_identity = (
        source_info.sha256,
        source_info.size,
        source_info.mode,
        source_info.device,
        source_info.inode,
        source_info.mtime_ns,
        source_info.ctime_ns,
        source_info.link_count,
    )
    if source_identity != expected_identity:
        fail(f"{label} changed before its executable snapshot")
    with (
        stable_open_relative(
            source.parent, source.name, expected=source_info
        ) as input_descriptor,
        exclusive_output_fd(destination, mode=0o600) as output_descriptor,
    ):
        while chunk := os.read(input_descriptor, 1024 * 1024):
            view = memoryview(chunk)
            while view:
                written = os.write(output_descriptor, view)
                if written <= 0:
                    fail(f"short write while snapshotting {label}")
                view = view[written:]
    os.chmod(destination, 0o700)
    snapshot_identity = _executable_identity(destination, f"{label} snapshot")
    if snapshot_identity[0] != expected_identity[0]:
        fail(f"{label} snapshot differs from the reviewed executable")
    return snapshot_identity


def _exclusive_binary_output(path: Path) -> BinaryIO:
    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        os.fchmod(descriptor, 0o600)
        return os.fdopen(descriptor, "wb")
    except BaseException:
        os.close(descriptor)
        raise


def _sealed_controller_manifest_path() -> Path:
    return Path(__file__).resolve().parent.parent / controller_seal.MANIFEST_NAME


def _sign_genesis(
    *,
    external_signer: Path,
    trusted_external_signer_sha256: str,
    rendered_genesis: Path,
    peer_one_config: Path,
    signed_genesis: Path,
    temporary_root: Path,
) -> str:
    identity = _executable_identity(external_signer, "external genesis signer")
    if identity[0] != trusted_external_signer_sha256:
        fail("external genesis signer differs from its trusted SHA-256")
    expected_hash_path = temporary_root / "genesis.expected_hash"
    stdout_path = temporary_root / "genesis-signer.stdout"
    stderr_path = temporary_root / "genesis-signer.stderr"
    signer_snapshot = temporary_root / "genesis-signer.snapshot"
    snapshot_identity = _snapshot_executable(
        external_signer,
        signer_snapshot,
        identity,
        "external genesis signer",
    )
    try:
        command = [
            str(signer_snapshot),
            "--unsigned-genesis",
            str(rendered_genesis),
            "--peer-config",
            str(peer_one_config),
            "--bound-manifest-out",
            str(rendered_genesis),
            "--signed-genesis-out",
            str(signed_genesis),
            "--expected-hash-out",
            str(expected_hash_path),
        ]
        with (
            _exclusive_binary_output(stdout_path) as stdout,
            _exclusive_binary_output(stderr_path) as stderr,
        ):
            completed = subprocess.run(
                command,
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                timeout=600,
                umask=0o077,
                env={
                    "HOME": str(temporary_root),
                    "LANG": "C",
                    "LC_ALL": "C",
                    "PATH": "/usr/bin:/bin",
                    "TMPDIR": str(temporary_root),
                },
            )
            stdout.flush()
            stderr.flush()
            os.fsync(stdout.fileno())
            os.fsync(stderr.fileno())
        require_private_regular_file(stdout_path)
        require_private_regular_file(stderr_path)
        if (
            stdout_path.lstat().st_size > MAX_GENESIS_SIGNER_OUTPUT_BYTES
            or stderr_path.lstat().st_size > MAX_GENESIS_SIGNER_OUTPUT_BYTES
        ):
            fail("external genesis signer emitted oversized diagnostics")
        if _executable_identity(external_signer, "external genesis signer") != identity:
            fail("external genesis signer changed during genesis signing")
        if (
            _executable_identity(signer_snapshot, "external genesis signer snapshot")
            != snapshot_identity
        ):
            fail("external genesis signer snapshot changed during genesis signing")
        if completed.returncode != 0:
            fail(
                "external genesis signer refused Taira genesis signing "
                f"with exit status {completed.returncode}"
            )
        signed = read_private_file(signed_genesis, 64 * 1024 * 1024)
        bound = read_private_file(rendered_genesis, 64 * 1024 * 1024)
        if not signed or not bound:
            fail("external genesis signer emitted empty signed or bound genesis")
        expected_hash = read_private_file(expected_hash_path, 256)
        try:
            expected_text = expected_hash.decode("ascii")
        except UnicodeDecodeError as exc:
            raise RuntimeError(
                "external genesis signer expected hash is not ASCII"
            ) from exc
        if (
            re.fullmatch(r"[0-9a-f]{64}\n", expected_text) is None
            or int(expected_text[-3:-1], 16) & 1 == 0
        ):
            fail("external genesis signer emitted a noncanonical genesis expected hash")
        return expected_text[:-1]
    finally:
        signer_snapshot.unlink(missing_ok=True)
        stdout_path.unlink(missing_ok=True)
        stderr_path.unlink(missing_ok=True)
        expected_hash_path.unlink(missing_ok=True)


def _privacy_projection(config_path: Path) -> dict[str, Any]:
    payload = renderer._load_toml(config_path)
    torii = payload.get("torii")
    genesis = payload.get("genesis")
    nexus = payload.get("nexus")
    if (
        not isinstance(torii, dict)
        or not isinstance(genesis, dict)
        or not isinstance(nexus, dict)
    ):
        fail(f"rendered config lacks Torii/genesis/Nexus tables: {config_path}")
    return payload


def _validate_rendered_configs(output: Path, expected_hash: str) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for index, slug in enumerate(SLUGS, start=1):
        root = output / "rendered" / slug
        config_path = root / "config.toml"
        config = _privacy_projection(config_path)
        genesis = config["genesis"]
        if (
            genesis.get("file") != str(output / "genesis.signed.nrt")
            or genesis.get("expected_hash") != expected_hash
        ):
            fail(f"rendered {slug} config is not bound to the signed bundle genesis")
        issuer = config["torii"].get("privacy_bootle_lantern_issuer")
        if not isinstance(issuer, dict):
            fail(f"rendered {slug} config lacks the privacy issuer table")
        if issuer.get("enabled") is not (index == 1):
            fail("privacy issuer must be enabled on validator 1 only")
        expected_state_dir = root / "runtime/privacy/bootle-lantern/issuer"
        if issuer.get("state_dir") != str(expected_state_dir):
            fail(f"rendered {slug} privacy state is not bundle-local")
        if index > 1 and set(issuer) != renderer.TAIRA_PRIVACY_ISSUER_BASE_FIELDS:
            fail(f"rendered {slug} retains dormant privacy issuer bindings")
        registry = config["nexus"].get("registry")
        if not isinstance(registry, dict) or (
            registry.get("manifest_directory") != str(root / "manifests")
            or registry.get("cache_directory") != str(root / "manifests")
        ):
            fail(f"rendered {slug} governance manifests are not bundle-local")
        hashes[slug] = sha256(config_path)
    return hashes


def _chmod_private_tree(root: Path) -> None:
    for current, directories, files in os.walk(root, followlinks=False):
        current_path = Path(current)
        os.chmod(current_path, 0o700)
        for name in directories:
            path = current_path / name
            if path.is_symlink():
                fail(f"reset output contains a symlink: {path}")
            os.chmod(path, 0o700)
        for name in files:
            path = current_path / name
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1
            ):
                fail(f"reset output contains an unsafe file: {path}")
            os.chmod(path, 0o600)


def prepare(args: argparse.Namespace) -> dict[str, object]:
    irohad_sha256 = require_sha256(args.irohad_sha256, "irohad SHA-256")
    source_bundle_sha256 = require_sha256(
        args.source_bundle_sha256, "sealed source reset bundle SHA-256"
    )
    source_commit = require_source_commit(args.source_commit)
    dpn_validator_release_commit = require_source_commit(
        args.dpn_validator_release_commit
    )
    cargo_lock_sha256 = require_sha256(args.cargo_lock_sha256, "Cargo.lock SHA-256")
    workspace_source_manifest_sha256 = require_sha256(
        args.workspace_source_manifest_sha256, "workspace source manifest SHA-256"
    )
    controller_digest = require_sha256(
        args.controller_digest, "sealed release controller digest"
    )
    controller_manifest = args.controller_manifest
    expected_controller_manifest = _sealed_controller_manifest_path()
    if controller_manifest != expected_controller_manifest:
        fail("controller manifest is not the sibling of the sealed reset controller")
    try:
        controller_seal.verify(
            controller_manifest.parent,
            controller_digest,
            "macos",
            source_commit,
        )
    except controller_seal.ControllerSealError as exc:
        raise RuntimeError(f"sealed release controller differs: {exc}") from exc
    controller_manifest_sha256 = stable_hash_path(
        controller_manifest, max_size=MAX_RESET_MANIFEST_BYTES
    ).sha256
    source = args.source_bundle
    output = args.output_bundle
    privacy_root = args.privacy_release_dir
    require_private_directory(source)
    trusted_genesis_signer_sha256 = require_sha256(
        args.trusted_genesis_external_signer_sha256,
        "external genesis signer SHA-256",
    )
    genesis_signer_identity = _executable_identity(
        args.genesis_external_signer, "external genesis signer"
    )
    if genesis_signer_identity[0] != trusted_genesis_signer_sha256:
        fail("external genesis signer differs from its trusted SHA-256")
    token_hash_tool_identity = _executable_identity(
        args.onboarding_token_hash_tool,
        "native onboarding-token hash tool",
    )
    if args.genesis_external_signer == args.onboarding_token_hash_tool:
        fail("external genesis signer and onboarding-token hash tool must be distinct")
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
    privacy_payloads, authenticated_manifest, authenticated_manifest_sha = (
        _load_authenticated_privacy_release(
            privacy_root,
            source_commit=source_commit,
            dpn_validator_release_commit=dpn_validator_release_commit,
            cargo_lock_sha256=cargo_lock_sha256,
            workspace_source_manifest_sha256=workspace_source_manifest_sha256,
        )
    )
    kagemusha_activation_authority: str | None = None
    kagemusha_release_policy_sha256: str | None = None
    if args.kagemusha_release_root is not None:
        kagemusha_activation_authority = (
            _require_kagemusha_activation_authority_permissions(
                privacy_payloads["genesis.json"],
                args.kagemusha_activation_authority,
            )
        )
        kagemusha_release_policy_sha256 = _kagemusha_release_policy_sha256(
            args.kagemusha_release_root
        )
    elif args.kagemusha_activation_authority is not None:
        fail(
            "--kagemusha-activation-authority requires --kagemusha-release-root"
        )

    source_snapshot_cleanup = tempfile.TemporaryDirectory(
        prefix="taira-authenticated-source-reset-", dir=output.parent
    )
    snapshot_root = Path(source_snapshot_cleanup.name).resolve(strict=True)
    os.chmod(snapshot_root, 0o700)
    source_snapshot = snapshot_root / "source"
    try:
        source_manifest, authenticated_source_sha256 = (
            _snapshot_authenticated_source_bundle(
                source,
                source_snapshot,
                source_bundle_sha256,
            )
        )
    except BaseException:
        source_snapshot_cleanup.cleanup()
        raise
    source = source_snapshot

    try:
        output.mkdir(mode=0o700)
    except BaseException:
        source_snapshot_cleanup.cleanup()
        raise
    try:
        copy_private_file(
            source / "validator-roster.toml", output / "validator-roster.toml"
        )
        write_private_file(output / "base-config.toml", privacy_payloads["config.toml"])
        rendered = output / "rendered"

        with tempfile.TemporaryDirectory(
            prefix="taira-privacy-reset-signing-", dir=output.parent
        ) as temporary_name:
            temporary = Path(temporary_name).resolve(strict=True)
            os.chmod(temporary, 0o700)
            token_hash_tool_snapshot = temporary / "onboarding-token-hash.snapshot"
            token_hash_snapshot_identity = _snapshot_executable(
                args.onboarding_token_hash_tool,
                token_hash_tool_snapshot,
                token_hash_tool_identity,
                "native onboarding-token hash tool",
            )
            genesis_template = temporary / "release-genesis.json"
            write_private_file(genesis_template, privacy_payloads["genesis.json"])
            written = renderer.render_bundle(
                output / "base-config.toml",
                output / "validator-roster.toml",
                rendered,
                secrets_path=source / "validator-secrets.toml",
                base_genesis_path=genesis_template,
                genesis_expected_hash=renderer.GENESIS_EXPECTED_HASH_PLACEHOLDER,
                bundle_root=output,
                onboarding_token_hash_tool=token_hash_tool_snapshot,
                kagemusha_release_root=args.kagemusha_release_root,
                include_kagemusha_qualification_seal=False,
            )
            if [path.parent.name for path in written] != list(SLUGS):
                fail(
                    "sealed roster does not define the exact ordered four Taira validators"
                )
            for slug in SLUGS:
                source_peer = source / "rendered" / slug
                output_peer = rendered / slug
                for tree in STATIC_TREES:
                    copy_private_tree(source_peer / tree, output_peer / tree)
            _validate_rendered_configs(
                output, renderer.GENESIS_EXPECTED_HASH_PLACEHOLDER
            )
            _require_rendered_kagemusha_config_projection(
                output,
                args.kagemusha_release_root,
                include_qualification_seal=False,
            )
            if (
                _executable_identity(
                    args.onboarding_token_hash_tool,
                    "native onboarding-token hash tool",
                )
                != token_hash_tool_identity
                or _executable_identity(
                    token_hash_tool_snapshot,
                    "native onboarding-token hash tool snapshot",
                )
                != token_hash_snapshot_identity
            ):
                fail("native onboarding-token hash tool changed before signing")
            signing_command = rendered / "genesis-signing-command.txt"
            if not signing_command.is_file() or signing_command.is_symlink():
                fail("renderer did not emit its private-key-file signing guidance")
            guidance = signing_command.read_text(encoding="utf-8")
            required_guidance = (
                '"$TAIRA_GENESIS_EXTERNAL_SIGNER"',
                "--unsigned-genesis",
                "--peer-config",
                "--bound-manifest-out",
                "--signed-genesis-out",
                "--expected-hash-out",
            )
            if any(value not in guidance for value in required_guidance) or (
                "private-key" in guidance.lower() or "kagami" in guidance.lower()
            ):
                fail("renderer emitted unsafe external genesis signing guidance")
            signing_command.unlink()
            os.chmod(rendered / "genesis.json", 0o600)
            expected_hash = _sign_genesis(
                external_signer=args.genesis_external_signer,
                trusted_external_signer_sha256=trusted_genesis_signer_sha256,
                rendered_genesis=rendered / "genesis.json",
                peer_one_config=rendered / SLUGS[0] / "config.toml",
                signed_genesis=output / "genesis.signed.nrt",
                temporary_root=temporary,
            )
            if args.kagemusha_release_root is not None and (
                _kagemusha_release_policy_sha256(args.kagemusha_release_root)
                != kagemusha_release_policy_sha256
            ):
                fail("Kagemusha release policy changed during genesis signing")
            bound_genesis = read_private_file(
                rendered / "genesis.json", 64 * 1024 * 1024
            )
            if kagemusha_activation_authority is not None:
                _require_kagemusha_activation_authority_permissions(
                    bound_genesis,
                    kagemusha_activation_authority,
                )
            write_private_file(output / "genesis.json", bound_genesis)

            written = renderer.render_bundle(
                output / "base-config.toml",
                output / "validator-roster.toml",
                rendered,
                secrets_path=source / "validator-secrets.toml",
                base_genesis_path=None,
                genesis_expected_hash=expected_hash,
                bundle_root=output,
                onboarding_token_hash_tool=token_hash_tool_snapshot,
                kagemusha_release_root=args.kagemusha_release_root,
                include_kagemusha_qualification_seal=True,
            )
            if [path.parent.name for path in written] != list(SLUGS):
                fail("post-signing renderer changed the exact four-validator roster")
            if (
                _executable_identity(
                    args.onboarding_token_hash_tool,
                    "native onboarding-token hash tool",
                )
                != token_hash_tool_identity
                or _executable_identity(
                    token_hash_tool_snapshot,
                    "native onboarding-token hash tool snapshot",
                )
                != token_hash_snapshot_identity
            ):
                fail("native onboarding-token hash tool changed during rendering")

        config_hashes = _validate_rendered_configs(output, expected_hash)
        kagemusha_config_projection = (
            _require_rendered_kagemusha_config_projection(
                output,
                args.kagemusha_release_root,
                include_qualification_seal=True,
            )
        )
        release_config = renderer._load_toml(output / "base-config.toml")
        genesis_table = release_config.get("genesis")
        genesis_public_key = (
            genesis_table.get("public_key") if isinstance(genesis_table, dict) else None
        )
        if (
            not isinstance(genesis_public_key, str)
            or GENESIS_PUBLIC_KEY_RE.fullmatch(genesis_public_key) is None
        ):
            fail(
                "reviewed release config lacks one canonical Ed25519 genesis public key"
            )

        for slug in SLUGS:
            storage = rendered / slug / "storage"
            storage.mkdir(mode=0o700)
            if any(storage.iterdir()):
                fail(f"new storage is not empty: {storage}")

        input_rows = authenticated_manifest["privacy_inputs"]
        if not isinstance(input_rows, dict):
            fail("authenticated privacy release input bindings changed")
        manifest = {
            key: source_manifest[key]
            for key in (
                "schema",
                "peer_count",
                "chain_id",
                "chain_discriminant",
                "node_storage_budget_bytes",
                "node_storage_budget_weights",
                "nexus_storage_budget_policy",
            )
        }
        manifest.update(
            {
                "source_commit": source_commit,
                "dpn_validator_release_commit": dpn_validator_release_commit,
                "cargo_lock_sha256": cargo_lock_sha256,
                "workspace_source_manifest_sha256": workspace_source_manifest_sha256,
                "release_controller": {
                    "digest": controller_digest,
                    "manifest_sha256": controller_manifest_sha256,
                    "platform": "macos",
                },
                "source_reset_bundle_sha256": authenticated_source_sha256,
                "irohad_sha256": irohad_sha256,
                "onboarding_token_hash_tool_sha256": token_hash_tool_identity[0],
                "genesis_public_key": genesis_public_key,
                "genesis_expected_hash": expected_hash,
                "signed_genesis_sha256": sha256(output / "genesis.signed.nrt"),
                "unsigned_genesis_sha256": sha256(output / "genesis.json"),
                "bound_genesis_manifest_sha256": sha256(output / "genesis.json"),
                "base_config_sha256": sha256(output / "base-config.toml"),
                "configs": config_hashes,
                "governance_manifests": {
                    slug: sha256(rendered / slug / "manifests/governance.manifest.json")
                    for slug in SLUGS
                },
                "runtime_sidecars": {
                    slug: {
                        name: sha256(rendered / slug / "runtime" / name)
                        for name in RUNTIME_SIDECARS
                    }
                    for slug in SLUGS
                },
                "prewarmed_storage_sha256": {
                    slug: hashlib.sha256().hexdigest() for slug in SLUGS
                },
                "privacy_bootstrap_release": {
                    "schema": "iroha.taira.signed_privacy_reset.v1",
                    "authenticated_snapshot_manifest_sha256": authenticated_manifest_sha,
                    "rollout_manifest_sha256": authenticated_manifest[
                        "rollout_manifest_sha256"
                    ],
                    "linux_archive": authenticated_manifest["linux_archive"],
                    "authority": authenticated_manifest["authority"],
                    "reviewed_inputs": {
                        name: {
                            "sha256": input_rows[name]["sha256"],
                            "size": input_rows[name]["size"],
                        }
                        for name in privacy_release.PRIVACY_INPUTS
                    },
                    "designated_issuer_validator": SLUGS[0],
                    "bound_genesis_manifest_sha256": sha256(output / "genesis.json"),
                    "signed_genesis_sha256": sha256(output / "genesis.signed.nrt"),
                    "validator_config_sha256": config_hashes,
                },
            }
        )
        if args.kagemusha_release_root is not None:
            manifest["kagemusha_release_root"] = str(args.kagemusha_release_root)
            manifest["kagemusha_release_policy_sha256"] = (
                kagemusha_release_policy_sha256
            )
            manifest["kagemusha_activation_authority"] = (
                kagemusha_activation_authority
            )
            if kagemusha_config_projection is None:
                fail("Kagemusha final config projection was not authenticated")
            manifest["kagemusha_config_projection"] = kagemusha_config_projection
            manifest["kagemusha_config_projection_sha256"] = hashlib.sha256(
                canonical_json_bytes(kagemusha_config_projection)
            ).hexdigest()
        atomic_write_json(output / "reset-manifest.json", manifest)
        _chmod_private_tree(output)
        _require_exact_names(output, OUTPUT_TOP_LEVEL_NAMES, "fresh signed reset")
        _require_exact_names(rendered, {"genesis.json", *SLUGS}, "fresh rendered reset")
        for slug in SLUGS:
            _require_exact_names(
                rendered / slug, SOURCE_VALIDATOR_NAMES, f"fresh {slug} runtime"
            )
            if any((rendered / slug / "storage").iterdir()):
                fail(f"new storage became non-empty: {slug}")
        directory_fd = os.open(output, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
        result = {
            "bundle": str(output),
            "empty_storage_sha256": hashlib.sha256().hexdigest(),
            "free_bytes_before_copy": free_bytes_before_copy,
            "genesis_expected_hash": expected_hash,
            "irohad_sha256": irohad_sha256,
            "peer_count": PEER_COUNT,
            "privacy_snapshot_manifest_sha256": authenticated_manifest_sha,
            "controller_digest": controller_digest,
        }
        if args.kagemusha_release_root is not None:
            result["kagemusha_release_root"] = str(args.kagemusha_release_root)
            result["kagemusha_release_policy_sha256"] = (
                kagemusha_release_policy_sha256
            )
            result["kagemusha_activation_authority"] = (
                kagemusha_activation_authority
            )
            result["kagemusha_config_projection_sha256"] = (
                _kagemusha_config_projection_sha256(args.kagemusha_release_root)
            )
        return result
    except BaseException:
        shutil.rmtree(output)
        raise
    finally:
        source_snapshot_cleanup.cleanup()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--source-bundle", type=Path, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--privacy-release-dir", type=Path, required=True)
    parser.add_argument("--genesis-external-signer", type=Path, required=True)
    parser.add_argument("--trusted-genesis-external-signer-sha256", required=True)
    parser.add_argument("--onboarding-token-hash-tool", type=Path, required=True)
    parser.add_argument(
        "--kagemusha-release-root",
        type=Path,
        help=(
            "absolute root-controlled Kagemusha policy/catalog/seal root, disjoint "
            "from the validator reset bundle"
        ),
    )
    parser.add_argument(
        "--kagemusha-activation-authority",
        help=(
            "exact account id that will execute Kagemusha activation; with a release "
            "root, the authenticated genesis must directly grant this account both "
            "immutable activation permissions"
        ),
    )
    parser.add_argument("--output-bundle", type=Path, required=True)
    parser.add_argument("--irohad-sha256", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--controller-manifest", type=Path, required=True)
    parser.add_argument("--controller-digest", required=True)
    parser.add_argument(
        "--minimum-free-bytes",
        type=int,
        default=DEFAULT_MINIMUM_FREE_BYTES,
        help=(
            "fail before materialization unless the output filesystem has at "
            "least this many free bytes"
        ),
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = prepare(args)
    except (
        OSError,
        ReleaseArtifactError,
        RuntimeError,
        subprocess.SubprocessError,
    ) as exc:
        print(f"fresh Taira privacy reset refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
