#!/usr/bin/env python3
"""Deploy one authenticated four-validator Taira v21 fresh-reset cohort.

Without ``--apply`` this command is strictly read-only: it authenticates the
binary, supervisor, reset bundle, current four-job launchd cohort, disk
headroom, and a read-only directory fsync barrier.  ``--apply`` additionally
requires root, installs content-addressed root-owned code, replaces all four
LaunchDaemons as one cohort, proves mandatory offline readiness and advancing
consensus, and proves one supervised child can restart without replacing its
supervisor.  Any failure after the old cohort is stopped restores all four old
plists and jobs.

The controller never prints config contents, process command lines, HTTP
bodies, or other runtime signing material.
"""

from __future__ import annotations

import argparse
import contextlib
import dataclasses
import fcntl
import grp
import hashlib
import json
import os
import plistlib
import pwd
import re
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable, NoReturn, Sequence


PEER_COUNT = 4
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
CHAIN_DISCRIMINANT = 369
OFFLINE_ASSET_ID = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
OFFLINE_ASSET_SCALE = 2
OFFLINE_CAPABILITY = "cash_handoff_v1"
OFFLINE_BRIDGE_ABI = 21
NODE_STORAGE_BUDGET_BYTES = 64 * 1024 * 1024 * 1024
NODE_STORAGE_BUDGET_POLICY = "bounded-64-gib-per-validator"
NODE_STORAGE_WEIGHTS = {
    "kura_blocks_bps": 7_500,
    "wsv_snapshots_bps": 2_000,
    "sorafs_bps": 0,
    "soranet_spool_bps": 250,
    "soravpn_spool_bps": 250,
}
DEFAULT_MINIMUM_FREE_BYTES = 16 * 1024 * 1024 * 1024
DEFAULT_MAXIMUM_FSYNC_LATENCY_MS = 250
DEFAULT_HEALTH_TIMEOUT_SECONDS = 240
RESTART_PROOF_TIMEOUT_SECONDS = 45
MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024
MAX_CONFIG_BYTES = 2 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_HTTP_BYTES = 4 * 1024 * 1024
MAX_RELEASE_FILE_BYTES = 4 * 1024 * 1024 * 1024
MAX_BUNDLE_BYTES = 64 * 1024 * 1024 * 1024
EXPECTED_RELEASE_FILE_COUNT = 16
RELEASE_ATTESTATION_FILE_NAME = "release-attestation-v4.norito"
RELEASE_POLICY_FILE_NAME = "release-policy-v1.norito"
RELEASE_CATALOG_DIRECTORY_NAME = "catalog"
EXPECTED_RELEASE_FILE_NAMES = {
    "cryptographic-review.evidence",
    "manifest.json",
    "manifest.norito",
    "manifest.norito.sha256",
    "physical-device-benchmark.evidence",
    "promotion-record-v4.norito",
    RELEASE_ATTESTATION_FILE_NAME,
    "step-ep.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "topup-finality-roster-v4.norito",
}
MINIMUM_RELEASE_WITHDRAWAL_HEIGHT = 1_000_000
EMPTY_TREE_SHA256 = hashlib.sha256().hexdigest()
LABELS = tuple(
    f"io.soramitsu.taira.validator-{index}" for index in range(1, PEER_COUNT + 1)
)
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))
TORII_PORTS = tuple(29_080 + index for index in range(PEER_COUNT))
P2P_PORTS = tuple(33_337 + index for index in range(PEER_COUNT))
TOP_LEVEL_NAMES = {
    "base-config.toml",
    "genesis.json",
    "genesis.signed.nrt",
    "kagemusha",
    "operator-identity.json",
    "rendered",
    "reset-manifest.json",
    "validator-roster.toml",
    "validator-secrets.toml",
}
VALIDATOR_NAMES = {"codec", "config.toml", "configs", "manifests", "runtime", "storage"}
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
BLOCK_HASH_RE = re.compile(r"(?:hash:)?([0-9A-Fa-f]{64})")
INSTALL_ROOT = Path("/Library/SORA/Taira")
LAUNCH_DAEMONS = Path("/Library/LaunchDaemons")
DEFAULT_SUPERVISOR_PYTHON = Path("/usr/bin/python3")
DEPLOYMENT_LOCK = INSTALL_ROOT / "deploy-v21.lock"


class DeploymentError(RuntimeError):
    """Raised when an identity, safety, rollout, or rollback gate fails."""


def fail(message: str) -> NoReturn:
    """Raise one redaction-safe deployment refusal."""

    raise DeploymentError(message)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Decode JSON while rejecting ambiguous duplicate object members."""

    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member {key!r}")
        result[key] = value
    return result


def require_sha256(value: str, label: str) -> str:
    """Require a lowercase SHA-256 literal."""

    if SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def require_commit(value: str, label: str = "expected source commit") -> str:
    """Require one full nonzero lowercase Git object id."""

    if COMMIT_RE.fullmatch(value) is None or value == "0" * 40:
        fail(f"{label} must be one full nonzero lowercase Git object id")
    return value


def canonical_path(path: Path, label: str) -> Path:
    """Require an existing absolute canonical path without aliases."""

    if not path.is_absolute():
        fail(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise DeploymentError(f"{label} is unavailable: {path}") from error
    if resolved != path:
        fail(f"{label} must be canonical and symlink-free: {path}")
    return path


def metadata_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return stable regular-file identity fields used around reads."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def open_regular(path: Path, maximum_bytes: int) -> tuple[int, os.stat_result]:
    """Open one bounded single-link regular file without following links."""

    before = path.lstat()
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        fail(f"unsafe or oversized regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    after = os.fstat(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        os.close(descriptor)
        fail(f"file changed while it was opened: {path}")
    return descriptor, after


def read_regular(path: Path, maximum_bytes: int) -> tuple[bytes, os.stat_result]:
    """Read a bounded regular file and recheck its complete identity."""

    descriptor, before = open_regular(path, maximum_bytes)
    try:
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        fail(f"file changed while it was read: {path}")
    return b"".join(chunks), after


def sha256_regular(path: Path, maximum_bytes: int) -> tuple[str, os.stat_result]:
    """Hash a stable bounded regular file through a no-follow descriptor."""

    descriptor, before = open_regular(path, maximum_bytes)
    try:
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        fail(f"file changed while it was hashed: {path}")
    return digest.hexdigest(), after


def parse_json_bytes(raw: bytes, label: str) -> dict[str, Any]:
    """Decode one canonical JSON object without duplicate members."""

    try:
        payload = json.loads(raw, object_pairs_hook=reject_duplicate_keys)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        raise DeploymentError(f"{label} is invalid JSON") from error
    if not isinstance(payload, dict):
        fail(f"{label} must be a JSON object")
    return payload


def require_private_entry(path: Path, owner_uid: int, owner_gid: int, *, directory: bool) -> os.stat_result:
    """Require one owner-private bundle entry with a stable type and owner."""

    info = path.lstat()
    expected = stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    if (
        stat.S_ISLNK(info.st_mode)
        or not expected
        or info.st_uid != owner_uid
        or info.st_gid != owner_gid
        or stat.S_IMODE(info.st_mode) & 0o077
        or (not directory and info.st_nlink != 1)
    ):
        fail(f"unsafe owner-private bundle entry: {path}")
    return info


def require_exact_names(path: Path, expected: set[str], label: str) -> None:
    """Require a directory to contain an exact, alias-free name set."""

    actual = {entry.name for entry in path.iterdir()}
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        fail(f"{label} inventory is not exact (missing={missing}, extra={extra})")


def inspect_private_bundle_tree(bundle: Path, owner_uid: int, owner_gid: int) -> int:
    """Reject aliases, special files, loose permissions, or an oversized bundle."""

    total = 0
    for current, directory_names, file_names in os.walk(bundle, followlinks=False):
        directory_names.sort()
        file_names.sort()
        current_path = Path(current)
        require_private_entry(current_path, owner_uid, owner_gid, directory=True)
        for name in directory_names:
            require_private_entry(current_path / name, owner_uid, owner_gid, directory=True)
        for name in file_names:
            info = require_private_entry(
                current_path / name, owner_uid, owner_gid, directory=False
            )
            total += info.st_size
            if total > MAX_BUNDLE_BYTES:
                fail("reset bundle exceeds the bounded 64 GiB deployment corridor")
    return total


def parse_toml(path: Path, owner_uid: int, owner_gid: int) -> dict[str, Any]:
    """Decode one bounded private validator config."""

    require_private_entry(path, owner_uid, owner_gid, directory=False)
    raw, _ = read_regular(path, MAX_CONFIG_BYTES)
    try:
        payload = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise DeploymentError(f"validator config is invalid TOML: {path}") from error
    if not isinstance(payload, dict):
        fail(f"validator config is not a TOML table: {path}")
    return payload


def address_port(value: object, label: str) -> int:
    """Extract the TCP port from one canonical ``addr:HOST:PORT#CRC`` literal."""

    if not isinstance(value, str) or not value.startswith("addr:") or "#" not in value:
        fail(f"{label} is not a canonical address literal")
    address = value[5:].split("#", 1)[0]
    _, separator, port_text = address.rpartition(":")
    if not separator or not port_text.isascii() or not port_text.isdecimal():
        fail(f"{label} has no canonical TCP port")
    port = int(port_text)
    if not 1 <= port <= 65_535:
        fail(f"{label} TCP port is out of range")
    return port


@dataclasses.dataclass(frozen=True)
class ReleaseTreeSeal:
    """One post-hash release-tree entry identity used for O(1) cutover checks."""

    relative_path: str
    identity: tuple[int, ...]


def release_tree_snapshot(
    root: Path, owner_uid: int, owner_gid: int
) -> tuple[str, tuple[ReleaseTreeSeal, ...]]:
    """Hash one exact release tree and bind every entry's final stat identity."""

    digest = hashlib.sha256()
    paths = sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix())
    seals: list[ReleaseTreeSeal] = [
        ReleaseTreeSeal(".", metadata_identity(root.lstat()))
    ]
    for path in paths:
        relative_text = path.relative_to(root).as_posix()
        relative = relative_text.encode()
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode):
            fail(f"release tree contains a symlink: {path}")
        if stat.S_ISDIR(info.st_mode):
            info = require_private_entry(path, owner_uid, owner_gid, directory=True)
            digest.update(b"d\0" + relative + b"\0")
        else:
            require_private_entry(path, owner_uid, owner_gid, directory=False)
            file_sha256, info = sha256_regular(path, MAX_RELEASE_FILE_BYTES)
            digest.update(b"f\0" + relative + b"\0")
            digest.update(info.st_size.to_bytes(8, "big"))
            digest.update(bytes.fromhex(file_sha256))
        seals.append(ReleaseTreeSeal(relative_text, metadata_identity(info)))

    final_paths = sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    )
    if [path.relative_to(root).as_posix() for path in final_paths] != [
        seal.relative_path for seal in seals[1:]
    ]:
        fail("release tree inventory changed while it was hashed")
    for seal in seals:
        path = root if seal.relative_path == "." else root / seal.relative_path
        if metadata_identity(path.lstat()) != seal.identity:
            fail(f"release tree entry changed while it was hashed: {path}")
    return digest.hexdigest(), tuple(seals)


def release_tree_sha256(root: Path, owner_uid: int, owner_gid: int) -> str:
    """Hash the release tree using the packager's canonical name/size projection."""

    digest, _ = release_tree_snapshot(root, owner_uid, owner_gid)
    return digest


def require_manifest_hash(manifest: dict[str, Any], field: str, path: Path, maximum: int) -> str:
    """Require a file digest to equal its reset-manifest binding."""

    expected = manifest.get(field)
    if not isinstance(expected, str):
        fail(f"reset manifest omitted {field}")
    require_sha256(expected, f"reset manifest {field}")
    actual, _ = sha256_regular(path, maximum)
    if actual != expected:
        fail(f"reset manifest {field} does not match {path.name}")
    return actual


@dataclasses.dataclass(frozen=True)
class PeerPlan:
    """Authenticated per-validator runtime paths and identities."""

    number: int
    label: str
    slug: str
    torii_port: int
    p2p_port: int
    workdir: Path
    storage: Path
    config: Path
    config_sha256: str
    workdir_device: int
    workdir_inode: int
    storage_device: int
    storage_inode: int


@dataclasses.dataclass(frozen=True)
class BundlePlan:
    """Complete authenticated dry-run result used by apply."""

    root: Path
    owner_uid: int
    owner_gid: int
    runtime_user: str
    runtime_group: str
    manifest: dict[str, Any]
    manifest_sha256: str
    release: ReleasePlan
    peers: tuple[PeerPlan, ...]
    bundle_bytes: int
    free_bytes: int
    free_bytes_by_device: tuple[tuple[int, int], ...]
    fsync_latency_ms: float


@dataclasses.dataclass(frozen=True)
class ReleasePlan:
    """Operator-pinned release identity and its single-copy cutover seal."""

    source_root: Path
    installed_root: Path
    tree_sha256: str
    tree_seals: tuple[ReleaseTreeSeal, ...]
    manifest_sha256: str
    release_policy_sha256: str
    release_attestation_sha256: str
    generation: str
    activation_height: int
    withdrawal_height: int
    max_proof_bytes: int
    asset_scale: int


def validate_release(
    bundle: Path,
    manifest: dict[str, Any],
    owner_uid: int,
    owner_gid: int,
    *,
    expected_manifest_sha256: str,
    expected_policy_sha256: str,
    expected_attestation_sha256: str,
) -> ReleasePlan:
    """Authenticate the exact operator-pinned single-release Kagemusha catalog."""

    root = bundle / "kagemusha"
    require_exact_names(
        root,
        {RELEASE_CATALOG_DIRECTORY_NAME, RELEASE_POLICY_FILE_NAME},
        "Kagemusha root",
    )
    policy_sha, _ = sha256_regular(root / RELEASE_POLICY_FILE_NAME, 64 * 1024)
    if (
        policy_sha != manifest.get("kagemusha_release_policy_sha256")
        or policy_sha != expected_policy_sha256
    ):
        fail("Kagemusha release policy does not match the reset manifest")
    catalog = root / RELEASE_CATALOG_DIRECTORY_NAME
    releases = list(catalog.iterdir())
    if len(releases) != 1 or SHA256_RE.fullmatch(releases[0].name) is None:
        fail("Kagemusha catalog must contain exactly one manifest-addressed release")
    release = releases[0]
    require_private_entry(release, owner_uid, owner_gid, directory=True)
    entries = list(release.iterdir())
    if (
        len(entries) != EXPECTED_RELEASE_FILE_COUNT
        or {path.name for path in entries} != EXPECTED_RELEASE_FILE_NAMES
        or any(path.is_dir() for path in entries)
    ):
        fail(
            "Kagemusha release does not contain the exact authenticated "
            f"{EXPECTED_RELEASE_FILE_COUNT}-file inventory"
        )
    manifest_norito_sha, _ = sha256_regular(
        release / "manifest.norito", MAX_MANIFEST_BYTES
    )
    if (
        manifest_norito_sha != release.name
        or manifest_norito_sha != manifest.get("kagemusha_manifest_sha256")
        or manifest_norito_sha != expected_manifest_sha256
    ):
        fail("Kagemusha manifest identity is not content-addressed")
    digest_body, _ = read_regular(release / "manifest.norito.sha256", 65)
    if digest_body != f"{manifest_norito_sha}\n".encode("ascii"):
        fail("Kagemusha manifest digest sidecar is invalid")
    release_json_raw, _ = read_regular(release / "manifest.json", MAX_MANIFEST_BYTES)
    release_json = parse_json_bytes(release_json_raw, "Kagemusha release manifest")
    attestation_sha, _ = sha256_regular(
        release / RELEASE_ATTESTATION_FILE_NAME, 2 * 1024 * 1024
    )
    withdrawal_height = release_json.get("withdrawal_height")
    max_proof_bytes = release_json.get("max_proof_bytes")
    if (
        release_json.get("chain_id") != CHAIN_ID
        or release_json.get("asset") != OFFLINE_ASSET_ID
        or release_json.get("asset_scale") != OFFLINE_ASSET_SCALE
        or release_json.get("activation_height") != 2
        or release_json.get("bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or release_json.get("generation") != "production-gate-real-artifacts-v4"
        or not isinstance(withdrawal_height, int)
        or isinstance(withdrawal_height, bool)
        or withdrawal_height < MINIMUM_RELEASE_WITHDRAWAL_HEIGHT
        or not isinstance(max_proof_bytes, int)
        or isinstance(max_proof_bytes, bool)
        or max_proof_bytes <= 0
        or release_json.get("release_attestation_sha256") != attestation_sha
        or attestation_sha != expected_attestation_sha256
        or manifest.get("kagemusha_release_attestation_sha256") != attestation_sha
    ):
        fail("Kagemusha release manifest is not the exact Taira ABI-21/V4 release")
    actual_tree, tree_seals = release_tree_snapshot(root, owner_uid, owner_gid)
    if actual_tree != manifest.get("kagemusha_release_tree_sha256"):
        fail("Kagemusha release tree does not match the reset manifest")
    return ReleasePlan(
        source_root=root,
        installed_root=INSTALL_ROOT / "releases" / actual_tree,
        tree_sha256=actual_tree,
        tree_seals=tree_seals,
        manifest_sha256=manifest_norito_sha,
        release_policy_sha256=policy_sha,
        release_attestation_sha256=attestation_sha,
        generation="production-gate-real-artifacts-v4",
        activation_height=2,
        withdrawal_height=withdrawal_height,
        max_proof_bytes=max_proof_bytes,
        asset_scale=OFFLINE_ASSET_SCALE,
    )


def validate_config_projection(
    config: dict[str, Any],
    bundle: Path,
    release_root: Path,
    *,
    torii_port: int,
    p2p_port: int,
) -> None:
    """Require exact public-Taira, storage, port, and mandatory-offline config."""

    if config.get("chain") != CHAIN_ID or config.get("chain_discriminant") != CHAIN_DISCRIMINANT:
        fail("validator config does not target canonical public Taira")
    network = config.get("network")
    torii = config.get("torii")
    nexus = config.get("nexus")
    settlement = config.get("settlement")
    genesis = config.get("genesis")
    if not all(isinstance(value, dict) for value in (network, torii, nexus, settlement, genesis)):
        fail("validator config lacks required network/Torii/Nexus/offline/genesis tables")
    assert isinstance(network, dict) and isinstance(torii, dict)
    assert isinstance(nexus, dict) and isinstance(settlement, dict) and isinstance(genesis, dict)
    if address_port(network.get("address"), "network.address") != p2p_port:
        fail(f"validator config P2P port is not exact {p2p_port}")
    if address_port(torii.get("address"), "torii.address") != torii_port:
        fail(f"validator config Torii port is not exact {torii_port}")
    storage = nexus.get("storage")
    if (
        not isinstance(storage, dict)
        or storage.get("local_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or storage.get("disk_budget_weights") != NODE_STORAGE_WEIGHTS
    ):
        fail("validator config lacks the exact bounded 64 GiB storage policy")
    offline = settlement.get("offline")
    commands = torii.get("kagemusha_commands")
    if (
        not isinstance(offline, dict)
        or offline.get("enabled") is not True
        or offline.get("escrow_required") is not True
        or not isinstance(commands, dict)
        or commands.get("enabled") is not True
    ):
        fail("validator config does not make offline cash mandatory")
    if offline.get("kagemusha_release_policy_path") != str(
        release_root / RELEASE_POLICY_FILE_NAME
    ) or offline.get("kagemusha_artifact_dir") != str(
        release_root / RELEASE_CATALOG_DIRECTORY_NAME
    ):
        fail("validator config does not bind the authenticated Kagemusha catalog")
    if genesis.get("file") != str(bundle / "genesis.signed.nrt"):
        fail("validator config does not bind the reset bundle signed genesis")


def measure_read_only_fsync(path: Path, maximum_ms: int) -> float:
    """Measure a read-only directory fsync barrier without creating any file."""

    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        started = time.monotonic_ns()
        os.fsync(descriptor)
        elapsed = (time.monotonic_ns() - started) / 1_000_000
    except OSError as error:
        raise DeploymentError(f"read-only fsync preflight failed for {path}") from error
    finally:
        os.close(descriptor)
    if elapsed > maximum_ms:
        fail(
            f"fsync latency {elapsed:.3f} ms exceeds the {maximum_ms} ms deployment bound"
        )
    return elapsed


def existing_ancestor(path: Path) -> Path:
    """Return the nearest existing ancestor of one absolute deployment path."""

    current = path
    while not current.exists():
        if current == current.parent:
            fail(f"deployment path has no existing ancestor: {path}")
        current = current.parent
    return current


def require_filesystem_headroom(
    paths: Sequence[Path], minimum_free_bytes: int
) -> tuple[tuple[int, int], ...]:
    """Require minimum free space on every distinct filesystem in ``paths``."""

    free_by_device: dict[int, int] = {}
    for path in paths:
        existing = existing_ancestor(path)
        info = existing.stat()
        free = shutil.disk_usage(existing).free
        prior = free_by_device.get(info.st_dev)
        free_by_device[info.st_dev] = free if prior is None else min(prior, free)
    for device, free in sorted(free_by_device.items()):
        if free < minimum_free_bytes:
            fail(
                f"filesystem device {device} has only {free} free bytes; "
                f"{minimum_free_bytes} are required"
            )
    return tuple(sorted(free_by_device.items()))


def validate_operator_release_identity(raw: bytes, release: ReleasePlan) -> None:
    """Bind the operator-facing artifact identity to the pinned release."""

    identity = parse_json_bytes(raw, "operator release identity")
    artifact = identity.get("artifact_set")
    if (
        identity.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or identity.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or identity.get("asset_definition_id") != OFFLINE_ASSET_ID
        or identity.get("asset_scale") != OFFLINE_ASSET_SCALE
        or not isinstance(artifact, dict)
        or artifact.get("generation") != release.generation
        or artifact.get("manifest_sha256") != release.manifest_sha256
        or artifact.get("release_policy_sha256") != release.release_policy_sha256
        or artifact.get("release_attestation_sha256")
        != release.release_attestation_sha256
        or artifact.get("activation_height") != release.activation_height
        or artifact.get("withdrawal_height") != release.withdrawal_height
        or artifact.get("max_proof_bytes") != release.max_proof_bytes
        or artifact.get("asset_scale") != release.asset_scale
    ):
        fail("operator release identity does not bind the exact pinned release")


def validate_bundle(
    bundle: Path,
    *,
    expected_binary_sha256: str,
    expected_source_commit: str,
    expected_kagemusha_manifest_sha256: str,
    expected_kagemusha_release_policy_sha256: str,
    expected_kagemusha_release_attestation_sha256: str,
    minimum_free_bytes: int,
    maximum_fsync_latency_ms: int,
) -> BundlePlan:
    """Authenticate a fresh v21 bundle without changing it or running binaries."""

    bundle = canonical_path(bundle, "reset bundle")
    root_info = bundle.lstat()
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        fail("reset bundle is not a real directory")
    if root_info.st_uid == 0 or stat.S_IMODE(root_info.st_mode) & 0o077:
        fail("reset bundle must be owned by a non-root runtime user and mode 0700")
    owner_uid, owner_gid = root_info.st_uid, root_info.st_gid
    try:
        runtime_user = pwd.getpwuid(owner_uid).pw_name
        runtime_group = grp.getgrgid(owner_gid).gr_name
    except KeyError as error:
        raise DeploymentError("reset bundle owner has no local user/group identity") from error
    require_exact_names(bundle, TOP_LEVEL_NAMES, "reset bundle")
    bundle_bytes = inspect_private_bundle_tree(bundle, owner_uid, owner_gid)

    manifest_path = bundle / "reset-manifest.json"
    manifest_raw, _ = read_regular(manifest_path, MAX_MANIFEST_BYTES)
    manifest = parse_json_bytes(manifest_raw, "reset manifest")
    manifest_sha256 = hashlib.sha256(manifest_raw).hexdigest()
    if (
        manifest.get("schema") != "taira-exact2f-reset-bundle"
        or manifest.get("peer_count") != PEER_COUNT
        or manifest.get("chain_id") != CHAIN_ID
        or manifest.get("chain_discriminant") != CHAIN_DISCRIMINANT
        or manifest.get("node_storage_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or manifest.get("node_storage_budget_weights") != NODE_STORAGE_WEIGHTS
        or manifest.get("nexus_storage_budget_policy") != NODE_STORAGE_BUDGET_POLICY
        or manifest.get("offline_release_policy")
        != "mandatory-authenticated-kagemusha-v4-activation-height-2"
        or manifest.get("offline_asset_definition_id") != OFFLINE_ASSET_ID
        or manifest.get("offline_asset_scale") != OFFLINE_ASSET_SCALE
    ):
        fail("reset manifest is not the exact bounded Taira v21 projection")
    if manifest.get("source_commit") != expected_source_commit:
        fail("reset manifest source commit does not match --expected-source-commit")
    if manifest.get("irohad_sha256") != expected_binary_sha256:
        fail("reset manifest binary does not match --expected-binary-sha256")
    require_manifest_hash(manifest, "signed_genesis_sha256", bundle / "genesis.signed.nrt", 64 * 1024 * 1024)
    require_manifest_hash(manifest, "unsigned_genesis_sha256", bundle / "genesis.json", 64 * 1024 * 1024)
    require_manifest_hash(manifest, "base_config_sha256", bundle / "base-config.toml", MAX_CONFIG_BYTES)
    operator_raw, _ = read_regular(bundle / "operator-identity.json", 64 * 1024)
    operator_sha = hashlib.sha256(operator_raw).hexdigest()
    if operator_sha != manifest.get("operator_identity_sha256"):
        fail("operator identity does not match the reset manifest")
    release = validate_release(
        bundle,
        manifest,
        owner_uid,
        owner_gid,
        expected_manifest_sha256=expected_kagemusha_manifest_sha256,
        expected_policy_sha256=expected_kagemusha_release_policy_sha256,
        expected_attestation_sha256=expected_kagemusha_release_attestation_sha256,
    )
    validate_operator_release_identity(operator_raw, release)
    if release.source_root.stat().st_dev != existing_ancestor(
        release.installed_root
    ).stat().st_dev:
        fail(
            "Kagemusha release source and root-owned release store are on "
            "different filesystems; one-copy atomic deployment is impossible"
        )

    rendered = bundle / "rendered"
    require_exact_names(rendered, {"genesis.json", *SLUGS}, "rendered validator root")
    rendered_genesis_sha, _ = sha256_regular(rendered / "genesis.json", 64 * 1024 * 1024)
    if rendered_genesis_sha != manifest.get("unsigned_genesis_sha256"):
        fail("rendered genesis differs from the reset manifest")
    config_hashes = manifest.get("configs")
    empty_hashes = manifest.get("prewarmed_storage_sha256")
    if not isinstance(config_hashes, dict) or set(config_hashes) != set(SLUGS):
        fail("reset manifest lacks the exact four validator config identities")
    if not isinstance(empty_hashes, dict) or empty_hashes != {
        slug: EMPTY_TREE_SHA256 for slug in SLUGS
    }:
        fail("reset manifest does not seal four empty storage trees")
    peers: list[PeerPlan] = []
    for index, (slug, label, torii_port, p2p_port) in enumerate(
        zip(SLUGS, LABELS, TORII_PORTS, P2P_PORTS, strict=True), start=1
    ):
        workdir = rendered / slug
        require_exact_names(workdir, VALIDATOR_NAMES, f"{slug} runtime root")
        workdir_info = require_private_entry(workdir, owner_uid, owner_gid, directory=True)
        storage = workdir / "storage"
        storage_info = require_private_entry(storage, owner_uid, owner_gid, directory=True)
        if any(storage.iterdir()):
            fail(f"fresh-reset storage is not empty: {slug}")
        config_path = workdir / "config.toml"
        config_sha, _ = sha256_regular(config_path, MAX_CONFIG_BYTES)
        expected_config_sha = config_hashes.get(slug)
        if not isinstance(expected_config_sha, str) or config_sha != expected_config_sha:
            fail(f"validator config does not match the reset manifest: {slug}")
        config = parse_toml(config_path, owner_uid, owner_gid)
        validate_config_projection(
            config,
            bundle,
            release.installed_root,
            torii_port=torii_port,
            p2p_port=p2p_port,
        )
        peers.append(
            PeerPlan(
                number=index,
                label=label,
                slug=slug,
                torii_port=torii_port,
                p2p_port=p2p_port,
                workdir=workdir,
                storage=storage,
                config=config_path,
                config_sha256=config_sha,
                workdir_device=workdir_info.st_dev,
                workdir_inode=workdir_info.st_ino,
                storage_device=storage_info.st_dev,
                storage_inode=storage_info.st_ino,
            )
        )

    free_by_device = require_filesystem_headroom(
        [
            bundle,
            release.installed_root,
            *(peer.storage for peer in peers),
            INSTALL_ROOT / "runtime",
        ],
        minimum_free_bytes,
    )
    free_bytes = min(free for _, free in free_by_device)
    latencies = [
        measure_read_only_fsync(peer.storage, maximum_fsync_latency_ms)
        for peer in peers
    ]
    return BundlePlan(
        root=bundle,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        runtime_user=runtime_user,
        runtime_group=runtime_group,
        manifest=manifest,
        manifest_sha256=manifest_sha256,
        release=release,
        peers=tuple(peers),
        bundle_bytes=bundle_bytes,
        free_bytes=free_bytes,
        free_bytes_by_device=free_by_device,
        fsync_latency_ms=max(latencies),
    )


def require_root_controlled_file(path: Path, *, executable: bool) -> os.stat_result:
    """Require a root-owned path no runtime user can replace or rewrite."""

    canonical_path(path, "root-controlled file")
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if stat.S_ISLNK(info.st_mode) or info.st_uid != 0 or stat.S_IMODE(info.st_mode) & 0o022:
            fail(f"root-controlled path has an unsafe component: {component}")
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                fail(f"root-controlled source is not a single-link regular file: {path}")
            if executable and not info.st_mode & 0o111:
                fail(f"root-controlled source is not executable: {path}")
        elif not stat.S_ISDIR(info.st_mode):
            fail(f"root-controlled ancestor is not a directory: {component}")
    return path.lstat()


@dataclasses.dataclass(frozen=True)
class SourcePlan:
    """Authenticated release binary and supervisor source identities."""

    binary: Path
    binary_sha256: str
    supervisor: Path
    supervisor_sha256: str
    python: Path


def validate_supervisor_python(path: Path) -> Path:
    """Require the root-controlled macOS system Python and a 3.9-compatible ABI."""

    python = canonical_path(path, "supervisor Python")
    if python != DEFAULT_SUPERVISOR_PYTHON:
        fail(f"supervisor Python must be exactly {DEFAULT_SUPERVISOR_PYTHON}")
    require_root_controlled_file(python, executable=True)
    try:
        probe = subprocess.run(
            [
                str(python),
                "-I",
                "-S",
                "-c",
                (
                    "import sys;"
                    "print('%d.%d.%d' % "
                    "(sys.version_info.major,sys.version_info.minor,"
                    "sys.version_info.micro))"
                ),
            ],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=5,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError("supervisor Python version probe failed") from error
    version_text = probe.stdout.strip()
    if (
        probe.returncode != 0
        or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version_text) is None
    ):
        fail("supervisor Python did not return one bounded semantic version")
    major, minor, _patch = (int(part) for part in version_text.split("."))
    if major != 3 or minor < 9:
        fail("supervisor Python must be Python >=3.9,<4")
    return python


def validate_sources(args: argparse.Namespace, bundle: BundlePlan) -> SourcePlan:
    """Authenticate root binary and owner-private supervisor without executing either."""

    binary = canonical_path(args.binary, "validator binary")
    require_root_controlled_file(binary, executable=True)
    binary_sha, _ = sha256_regular(binary, MAX_BINARY_BYTES)
    if binary_sha != args.expected_binary_sha256:
        fail("validator binary does not match --expected-binary-sha256")

    supervisor = canonical_path(args.supervisor, "supervisor source")
    supervisor_info = supervisor.lstat()
    if (
        not stat.S_ISREG(supervisor_info.st_mode)
        or stat.S_ISLNK(supervisor_info.st_mode)
        or supervisor_info.st_nlink != 1
        or supervisor_info.st_uid != bundle.owner_uid
        or supervisor_info.st_gid != bundle.owner_gid
        or stat.S_IMODE(supervisor_info.st_mode) & 0o077
    ):
        fail("supervisor source is not an owner-private runtime-user file")
    supervisor_sha, _ = sha256_regular(supervisor, 4 * 1024 * 1024)
    if supervisor_sha != args.expected_supervisor_sha256:
        fail("supervisor source does not match --expected-supervisor-sha256")

    python = validate_supervisor_python(args.supervisor_python)
    return SourcePlan(
        binary=binary,
        binary_sha256=binary_sha,
        supervisor=supervisor,
        supervisor_sha256=supervisor_sha,
        python=python,
    )


@dataclasses.dataclass(frozen=True)
class ProcessInfo:
    """Redaction-safe process identity used for exact parent/owner checks."""

    pid: int
    ppid: int
    uid: int
    argv: tuple[str, ...]


@dataclasses.dataclass(frozen=True)
class OldManagedIdentity:
    """Expected supervisor and child command identity for one rollback job."""

    supervisor_uid: int
    supervisor_argv: tuple[str, ...]
    child_uid: int
    child_argv: tuple[str, ...]
    pid_file: Path
    pid_file_gid: int


@dataclasses.dataclass(frozen=True)
class PlistSnapshot:
    """Exact old LaunchDaemon bytes, ownership, and managed process identity."""

    path: Path
    body: bytes
    mode: int
    uid: int
    gid: int
    managed: OldManagedIdentity


class SystemOps:
    """Small injectable boundary around launchd and process operations."""

    def run(self, command: Sequence[str]) -> subprocess.CompletedProcess[str]:
        """Run one noninteractive command without echoing its output on failure."""

        try:
            return subprocess.run(
                command,
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=60,
            )
        except subprocess.TimeoutExpired as error:
            raise DeploymentError("bounded system command timed out") from error

    def launchd_print(self, label: str) -> str | None:
        """Return launchd's system-domain record, or ``None`` when absent."""

        result = self.run(["/bin/launchctl", "print", f"system/{label}"])
        return result.stdout if result.returncode == 0 else None

    def bootout(self, label: str) -> None:
        """Unload one exact system-domain job."""

        result = self.run(["/bin/launchctl", "bootout", f"system/{label}"])
        if result.returncode != 0:
            fail(f"launchd bootout failed for {label} (status {result.returncode})")

    def bootstrap(self, plist: Path) -> None:
        """Load one exact LaunchDaemon plist."""

        result = self.run(["/bin/launchctl", "bootstrap", "system", str(plist)])
        if result.returncode != 0:
            fail(f"launchd bootstrap failed for {plist.name} (status {result.returncode})")

    def inspect_process(self, pid: int) -> ProcessInfo:
        """Read parent, uid, and complete argv for one macOS process."""

        result = self.run(
            ["/bin/ps", "-ww", "-p", str(pid), "-o", "ppid=", "-o", "uid=", "-o", "command="]
        )
        if result.returncode != 0 or not result.stdout.strip():
            fail(f"managed process is not running: pid {pid}")
        fields = result.stdout.strip().split(maxsplit=2)
        if len(fields) != 3:
            fail(f"could not parse managed process identity: pid {pid}")
        try:
            argv = tuple(shlex.split(fields[2]))
            return ProcessInfo(pid=pid, ppid=int(fields[0]), uid=int(fields[1]), argv=argv)
        except (ValueError, TypeError) as error:
            raise DeploymentError(f"could not parse managed process identity: pid {pid}") from error

    def terminate(self, pid: int) -> None:
        """Terminate exactly one already-authenticated managed child."""

        os.kill(pid, signal.SIGTERM)

    def process_exists(self, pid: int) -> bool:
        """Return whether a process still exists."""

        result = self.run(["/bin/ps", "-p", str(pid), "-o", "pid="])
        return result.returncode == 0 and bool(result.stdout.strip())


def launchd_pid(record: str | None, label: str) -> int:
    """Extract one positive supervisor PID from launchd's printed record."""

    if record is None:
        fail(f"launchd job is not loaded: {label}")
    matches = re.findall(r"(?m)^\s*pid\s*=\s*([0-9]+)\s*$", record)
    if len(matches) != 1 or int(matches[0]) <= 1:
        fail(f"launchd job has no unique running supervisor PID: {label}")
    return int(matches[0])


def required_option(argv: tuple[str, ...], option: str, label: str) -> str:
    """Return one exact option value from a managed supervisor command."""

    indices = [index for index, value in enumerate(argv) if value == option]
    if (
        len(indices) != 1
        or indices[0] + 1 >= len(argv)
        or argv[indices[0] + 1].startswith("--")
    ):
        fail(f"{label} supervisor lacks one exact {option} argument")
    return argv[indices[0] + 1]


def inspect_old_managed_identity(
    payload: dict[str, Any],
    label: str,
    supervisor_pid: int,
    ops: SystemOps,
) -> OldManagedIdentity:
    """Authenticate one old launchd supervisor and its exact managed child."""

    arguments = payload.get("ProgramArguments")
    runtime_user = payload.get("UserName")
    runtime_group = payload.get("GroupName")
    if (
        not isinstance(arguments, list)
        or not all(isinstance(value, str) for value in arguments)
        or not isinstance(runtime_user, str)
        or not isinstance(runtime_group, str)
    ):
        fail(f"old LaunchDaemon is not an explicit supervised job: {label}")
    try:
        uid = pwd.getpwnam(runtime_user).pw_uid
        gid = grp.getgrnam(runtime_group).gr_gid
    except KeyError as error:
        raise DeploymentError(f"old LaunchDaemon runtime identity is unknown: {label}") from error
    supervisor_argv = tuple(arguments)
    supervisor = ops.inspect_process(supervisor_pid)
    if (
        supervisor.ppid != 1
        or supervisor.uid != uid
        or supervisor.argv != supervisor_argv
    ):
        fail(f"old LaunchDaemon supervisor identity differs from its plist: {label}")
    pid_file = Path(required_option(supervisor_argv, "--pid-file", label))
    binary = required_option(supervisor_argv, "--binary", label)
    config = required_option(supervisor_argv, "--config", label)
    if not pid_file.is_absolute() or not Path(binary).is_absolute() or not Path(config).is_absolute():
        fail(f"old LaunchDaemon contains a non-absolute managed path: {label}")
    child_pid = parse_pid_file(pid_file, uid, gid)
    child = ops.inspect_process(child_pid)
    child_argv = (binary, "--sora", "--config", config)
    if child.ppid != supervisor_pid or child.uid != uid or child.argv != child_argv:
        fail(f"old LaunchDaemon child identity differs from its supervisor: {label}")
    return OldManagedIdentity(
        supervisor_uid=uid,
        supervisor_argv=supervisor_argv,
        child_uid=uid,
        child_argv=child_argv,
        pid_file=pid_file,
        pid_file_gid=gid,
    )


def verify_restored_snapshot(snapshot: PlistSnapshot, ops: SystemOps) -> None:
    """Require a restored old job to own its exact supervisor and child."""

    label = snapshot.path.stem
    supervisor_pid = launchd_pid(ops.launchd_print(label), label)
    supervisor = ops.inspect_process(supervisor_pid)
    expected = snapshot.managed
    if (
        supervisor.ppid != 1
        or supervisor.uid != expected.supervisor_uid
        or supervisor.argv != expected.supervisor_argv
    ):
        fail(f"restored LaunchDaemon supervisor identity is wrong: {label}")
    child_pid = parse_pid_file(
        expected.pid_file, expected.child_uid, expected.pid_file_gid
    )
    child = ops.inspect_process(child_pid)
    if (
        child.ppid != supervisor_pid
        or child.uid != expected.child_uid
        or child.argv != expected.child_argv
    ):
        fail(f"restored LaunchDaemon child identity is wrong: {label}")


def capture_old_cohort(ops: SystemOps) -> tuple[PlistSnapshot, ...]:
    """Read the exact four old plists and require all four jobs loaded."""

    snapshots: list[PlistSnapshot] = []
    for label in LABELS:
        path = LAUNCH_DAEMONS / f"{label}.plist"
        body, info = read_regular(path, MAX_MANIFEST_BYTES)
        if info.st_uid != 0 or stat.S_IMODE(info.st_mode) & 0o022:
            fail(f"old LaunchDaemon plist is not root-controlled: {path}")
        try:
            payload = plistlib.loads(body)
        except Exception as error:
            raise DeploymentError(f"old LaunchDaemon plist is invalid: {path}") from error
        if not isinstance(payload, dict) or payload.get("Label") != label:
            fail(f"old LaunchDaemon plist label mismatch: {path}")
        supervisor_pid = launchd_pid(ops.launchd_print(label), label)
        managed = inspect_old_managed_identity(payload, label, supervisor_pid, ops)
        snapshots.append(
            PlistSnapshot(
                path=path,
                body=body,
                mode=stat.S_IMODE(info.st_mode),
                uid=info.st_uid,
                gid=info.st_gid,
                managed=managed,
            )
        )
    return tuple(snapshots)


def wait_launchd_cohort_running(
    labels: Sequence[str], ops: SystemOps, timeout_seconds: float
) -> list[str]:
    """Wait for every loaded job to publish one positive supervisor PID."""

    deadline = time.monotonic() + timeout_seconds
    pending = set(labels)
    while pending and time.monotonic() < deadline:
        for label in tuple(pending):
            try:
                launchd_pid(ops.launchd_print(label), label)
            except DeploymentError:
                continue
            pending.remove(label)
        if pending:
            time.sleep(0.25)
    return sorted(pending)


def fsync_directory(path: Path) -> None:
    """Durably flush one directory after a publication rename."""

    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def ensure_root_directory(path: Path, mode: int) -> None:
    """Create or validate a root-owned non-writable installation directory."""

    if not path.is_absolute():
        fail(f"root installation directory is not absolute: {path}")
    pending: list[Path] = []
    current = path
    while not current.exists():
        pending.append(current)
        current = current.parent
    for component in [*reversed(current.parents), current]:
        info = component.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != 0
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"unsafe root installation ancestor: {component}")
    for component in reversed(pending):
        os.mkdir(component, mode)
        os.chown(component, 0, 0)
        os.chmod(component, mode)
        fsync_directory(component.parent)
    info = path.lstat()
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) & 0o022
    ):
        fail(f"unsafe root installation directory: {path}")


def ensure_runtime_directory(path: Path, uid: int, gid: int) -> None:
    """Create or validate one owner-private runtime state directory."""

    if path.exists() or path.is_symlink():
        info = path.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != uid
            or info.st_gid != gid
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            fail(f"unsafe runtime state directory: {path}")
        return
    os.mkdir(path, 0o700)
    os.chown(path, uid, gid)
    os.chmod(path, 0o700)
    fsync_directory(path.parent)


@contextlib.contextmanager
def exclusive_deployment_lock() -> Any:
    """Hold one root-owned nonblocking lock across capture, apply, and rollback."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    existed = DEPLOYMENT_LOCK.exists()
    descriptor = os.open(
        DEPLOYMENT_LOCK,
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != 0
            or info.st_gid != 0
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            fail("deployment lock is not an exact root:wheel 0600 regular file")
        if not existed:
            os.fsync(descriptor)
            fsync_directory(DEPLOYMENT_LOCK.parent)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise DeploymentError(
                "another Taira reset controller holds the deployment lock"
            ) from error
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def install_immutable(source: Path, destination: Path, expected_sha256: str) -> os.stat_result:
    """Install authenticated bytes as root:wheel 0555 without overwriting."""

    if destination.exists() or destination.is_symlink():
        require_root_controlled_file(destination, executable=True)
        actual, info = sha256_regular(destination, MAX_BINARY_BYTES)
        if actual != expected_sha256 or stat.S_IMODE(info.st_mode) != 0o555 or info.st_gid != 0:
            fail(f"existing immutable installation has the wrong identity: {destination}")
        return info
    parent = destination.parent
    if not parent.exists():
        ensure_root_directory(parent, 0o755)
    parent_info = parent.lstat()
    if stat.S_IMODE(parent_info.st_mode) & 0o022:
        fail(f"immutable installation parent is writable: {parent}")
    temporary = parent / f".{destination.name}.{os.getpid()}.tmp"
    source_fd = -1
    output_fd = -1
    try:
        source_fd, source_before = open_regular(source, MAX_BINARY_BYTES)
        output_fd = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o500,
        )
        digest = hashlib.sha256()
        while chunk := os.read(source_fd, 1024 * 1024):
            digest.update(chunk)
            offset = 0
            while offset < len(chunk):
                offset += os.write(output_fd, chunk[offset:])
        if metadata_identity(source_before) != metadata_identity(os.fstat(source_fd)):
            fail(f"immutable source changed while copied: {source}")
        if digest.hexdigest() != expected_sha256:
            fail(f"immutable source digest changed while copied: {source}")
        os.fsync(output_fd)
        os.fchown(output_fd, 0, 0)
        os.fchmod(output_fd, 0o555)
        os.fsync(output_fd)
        os.close(source_fd)
        source_fd = -1
        os.close(output_fd)
        output_fd = -1
        os.replace(temporary, destination)
        fsync_directory(parent)
    finally:
        if source_fd >= 0:
            os.close(source_fd)
        if output_fd >= 0:
            os.close(output_fd)
        temporary.unlink(missing_ok=True)
    actual, info = sha256_regular(destination, MAX_BINARY_BYTES)
    if actual != expected_sha256:
        fail(f"immutable installation changed after publication: {destination}")
    return info


def atomic_replace_owned(path: Path, body: bytes, *, mode: int, uid: int, gid: int) -> None:
    """Atomically replace a root-controlled plist with authenticated bytes."""

    if path.is_symlink():
        fail(f"refusing to replace symlink: {path}")
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        offset = 0
        while offset < len(body):
            offset += os.write(descriptor, body[offset:])
        os.fchown(descriptor, uid, gid)
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    try:
        os.replace(temporary, path)
        fsync_directory(path.parent)
    finally:
        temporary.unlink(missing_ok=True)


def render_plist(
    peer: PeerPlan,
    bundle: BundlePlan,
    sources: SourcePlan,
    *,
    installed_binary: Path,
    binary_info: os.stat_result,
    installed_supervisor: Path,
    runtime_root: Path,
) -> bytes:
    """Render one fresh known-argument LaunchDaemon with all five stat fields."""

    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    log_file = runtime_root / "logs" / f"validator-{peer.number}-supervisor.log"
    arguments = [
        str(sources.python),
        str(installed_supervisor),
        "--binary",
        str(installed_binary),
        "--binary-sha256",
        sources.binary_sha256,
        "--binary-device",
        str(binary_info.st_dev),
        "--binary-inode",
        str(binary_info.st_ino),
        "--binary-size",
        str(binary_info.st_size),
        "--binary-mtime-ns",
        str(binary_info.st_mtime_ns),
        "--binary-ctime-ns",
        str(binary_info.st_ctime_ns),
        "--config",
        str(peer.config),
        "--config-sha256",
        peer.config_sha256,
        "--workdir",
        str(peer.workdir),
        "--workdir-device",
        str(peer.workdir_device),
        "--workdir-inode",
        str(peer.workdir_inode),
        "--storage-dir",
        str(peer.storage),
        "--storage-device",
        str(peer.storage_device),
        "--storage-inode",
        str(peer.storage_inode),
        "--pid-file",
        str(pid_file),
        "--initial-backoff-seconds",
        "1.0",
        "--maximum-backoff-seconds",
        "30.0",
        "--stable-uptime-seconds",
        "120.0",
    ]
    payload = {
        "Label": peer.label,
        "ProgramArguments": arguments,
        "WorkingDirectory": str(peer.workdir),
        "RunAtLoad": True,
        "KeepAlive": True,
        "ThrottleInterval": 10,
        "ProcessType": "Background",
        "ExitTimeOut": 60,
        "AbandonProcessGroup": False,
        "UserName": bundle.runtime_user,
        "GroupName": bundle.runtime_group,
        "EnvironmentVariables": {
            "GENESIS": str(bundle.root / "genesis.signed.nrt"),
            "KURA_STORE_DIR": str(peer.storage / "kura"),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "RUST_LOG": "info",
            "SNAPSHOT_STORE_DIR": str(peer.storage / "snapshot"),
            "ZK_HALO2_ENABLED": "true",
        },
        "StandardOutPath": str(log_file),
        "StandardErrorPath": str(log_file),
    }
    return plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)


def require_bundle_runtime_unchanged(bundle: BundlePlan) -> None:
    """Recheck mutable config/storage identities immediately before cutover."""

    manifest_raw, _ = read_regular(bundle.root / "reset-manifest.json", MAX_MANIFEST_BYTES)
    if hashlib.sha256(manifest_raw).hexdigest() != bundle.manifest_sha256:
        fail("reset manifest changed after preflight")
    signed_sha, _ = sha256_regular(
        bundle.root / "genesis.signed.nrt", 64 * 1024 * 1024
    )
    if signed_sha != bundle.manifest.get("signed_genesis_sha256"):
        fail("signed genesis changed after preflight")
    current_paths = sorted(
        bundle.release.source_root.rglob("*"),
        key=lambda item: item.relative_to(bundle.release.source_root).as_posix(),
    )
    expected_paths = [seal.relative_path for seal in bundle.release.tree_seals[1:]]
    if [
        path.relative_to(bundle.release.source_root).as_posix()
        for path in current_paths
    ] != expected_paths:
        fail("Kagemusha release inventory changed after preflight")
    for seal in bundle.release.tree_seals:
        path = (
            bundle.release.source_root
            if seal.relative_path == "."
            else bundle.release.source_root / seal.relative_path
        )
        if metadata_identity(path.lstat()) != seal.identity:
            fail(f"Kagemusha release entry changed after preflight: {seal.relative_path}")
    for peer in bundle.peers:
        workdir_info = require_private_entry(
            peer.workdir, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        storage_info = require_private_entry(
            peer.storage, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        if (
            workdir_info.st_dev != peer.workdir_device
            or workdir_info.st_ino != peer.workdir_inode
            or storage_info.st_dev != peer.storage_device
            or storage_info.st_ino != peer.storage_inode
        ):
            fail(f"fresh-reset runtime path changed after preflight: {peer.slug}")
        config_sha, _ = sha256_regular(peer.config, MAX_CONFIG_BYTES)
        if config_sha != peer.config_sha256:
            fail(f"validator config changed after preflight: {peer.slug}")
        if any(peer.storage.iterdir()):
            fail(f"fresh-reset storage changed after preflight: {peer.slug}")


def rewrite_release_tree_ownership(
    root: Path,
    *,
    uid: int,
    gid: int,
    file_mode: int,
    directory_mode: int,
) -> None:
    """Rewrite one exact release tree without copying its multi-gigabyte files."""

    paths = sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix())
    files: list[Path] = []
    directories: list[Path] = [root]
    for path in paths:
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode):
            fail(f"release tree contains a symlink during ownership rewrite: {path}")
        if stat.S_ISDIR(info.st_mode):
            directories.append(path)
        elif stat.S_ISREG(info.st_mode) and info.st_nlink == 1:
            files.append(path)
        else:
            fail(f"release tree contains an unsafe entry during ownership rewrite: {path}")
    for path in files:
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, file_mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    for path in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, directory_mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)


def move_release_to_root_store(bundle: BundlePlan) -> Path:
    """Move and harden the sole release copy in its content-addressed root store."""

    source = bundle.release.source_root
    destination = bundle.release.installed_root
    release_store = destination.parent
    ensure_root_directory(release_store, 0o755)
    if destination.exists() or destination.is_symlink():
        fail(f"content-addressed release destination already exists: {destination}")
    if source.stat().st_dev != release_store.stat().st_dev:
        fail("Kagemusha release move crossed filesystems")
    try:
        rewrite_release_tree_ownership(
            source,
            uid=0,
            gid=bundle.owner_gid,
            file_mode=0o440,
            directory_mode=0o550,
        )
        os.rename(source, destination)
        fsync_directory(source.parent)
        fsync_directory(release_store)
    except BaseException:
        if source.exists() and not source.is_symlink():
            rewrite_release_tree_ownership(
                source,
                uid=bundle.owner_uid,
                gid=bundle.owner_gid,
                file_mode=0o600,
                directory_mode=0o700,
            )
        raise
    return destination


def restore_release_to_bundle(bundle: BundlePlan) -> None:
    """Restore the single release copy after a failed cohort rollout."""

    source = bundle.release.installed_root
    destination = bundle.release.source_root
    if destination.exists() or destination.is_symlink():
        fail("cannot restore Kagemusha release over an existing bundle path")
    if not source.is_dir() or source.is_symlink():
        fail("root-owned Kagemusha release is unavailable for rollback")
    os.rename(source, destination)
    fsync_directory(source.parent)
    fsync_directory(destination.parent)
    rewrite_release_tree_ownership(
        destination,
        uid=bundle.owner_uid,
        gid=bundle.owner_gid,
        file_mode=0o600,
        directory_mode=0o700,
    )


def http_json(url: str, timeout: float = 2.0) -> dict[str, Any]:
    """Fetch one bounded JSON response without retaining error bodies."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "application/json", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")
    return parse_json_bytes(body, f"health response from {url}")


def http_ok(url: str, timeout: float = 2.0) -> None:
    """Require one bounded HTTP 200 response without parsing or retaining its body."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "*/*", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")


def require_uint(value: object, label: str, *, positive: bool = False) -> int:
    """Require one non-boolean unsigned JSON integer."""

    if not isinstance(value, int) or isinstance(value, bool) or value < int(positive):
        fail(f"{label} is not a valid unsigned integer")
    return value


def normalized_block_hash(value: object, label: str) -> str:
    """Normalize a canonical Iroha block hash to lowercase hexadecimal."""

    if not isinstance(value, str):
        fail(f"{label} is not a block hash")
    match = BLOCK_HASH_RE.fullmatch(value)
    if match is None:
        fail(f"{label} is not a canonical block hash")
    return match.group(1).lower()


def nested(payload: dict[str, Any], *keys: str) -> object:
    """Return a nested mapping value, or ``None`` on a missing object."""

    current: object = payload
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def tagged_unit(
    value: object, key: str, label: str, allowed: set[str]
) -> str:
    """Decode one canonical tagged-unit status value."""

    if (
        not isinstance(value, dict)
        or set(value) != {key, "details"}
        or not isinstance(value.get(key), str)
        or value.get(key) not in allowed
        or value.get("details") is not None
    ):
        fail(f"{label} is not a canonical tagged unit")
    tag = value[key]
    assert isinstance(tag, str)
    return tag


def published_source_commit(status: dict[str, Any]) -> str:
    """Read the exact full build commit from public node status."""

    build = status.get("build")
    if not isinstance(build, dict):
        fail("/status omitted its build identity")
    for key in ("git_commit_sha", "git_sha", "commit_sha", "commit"):
        value = build.get(key)
        if isinstance(value, str) and COMMIT_RE.fullmatch(value.lower()):
            return value.lower()
    fail("/status omitted one full build Git commit")


@dataclasses.dataclass(frozen=True)
class PeerSample:
    """Coherent committed/offline identity observed from one validator."""

    label: str
    height: int
    block_hash: str
    context: str
    node: str
    build: str
    config: str
    offline_release: str


@dataclasses.dataclass(frozen=True)
class FleetSample:
    """One exact four-validator common-commit sample."""

    height: int
    block_hash: str
    context: str
    build: str
    config: str
    offline_release: str
    nodes: tuple[str, ...]


HttpGetter = Callable[[str, float], dict[str, Any]]
HealthGetter = Callable[[str, float], None]


def validate_peer_health(
    peer: PeerPlan,
    bundle: BundlePlan,
    expected_source_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> PeerSample:
    """Validate readiness, durable consensus, and offline identity for one peer."""

    root = f"http://127.0.0.1:{peer.torii_port}"
    health_getter(f"{root}/health", 2.0)
    ready = getter(f"{root}/readyz", 2.0)
    if (
        ready.get("live") is not True
        or ready.get("mandatory") is not True
        or ready.get("ready") is not True
        or ready.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or ready.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or ready.get("blockers") != []
    ):
        fail(f"{peer.label} /readyz is not mandatory-offline ready")

    status = getter(f"{root}/status", 2.0)
    blocks = require_uint(status.get("blocks"), f"{peer.label} /status.blocks", positive=True)
    if published_source_commit(status) != expected_source_commit:
        fail(f"{peer.label} publishes the wrong build source commit")

    sumeragi = getter(f"{root}/v1/sumeragi/status", 2.0)
    if sumeragi.get("protocol_version") != 3 or sumeragi.get("restart_required") is not False:
        fail(f"{peer.label} is not running one restart-clean Sumeragi v2 reducer")
    reducer_height = require_uint(
        sumeragi.get("height"), f"{peer.label} reducer height", positive=True
    )
    committed = require_uint(
        sumeragi.get("last_committed_height"),
        f"{peer.label} last_committed_height",
        positive=True,
    )
    if committed != blocks:
        fail(f"{peer.label} /status.blocks differs from durable committed height")
    if committed > reducer_height:
        fail(f"{peer.label} committed height is ahead of its reducer height")
    context_record = sumeragi.get("height_context")
    if not isinstance(context_record, dict):
        fail(f"{peer.label} omitted its frozen height context")
    validator_count = require_uint(
        context_record.get("validator_count"),
        f"{peer.label} frozen validator count",
        positive=True,
    )
    quorum = context_record.get("quorum")
    if not isinstance(quorum, dict):
        fail(f"{peer.label} omitted its frozen quorum")
    context_min_signers = require_uint(
        quorum.get("min_signers"), f"{peer.label} frozen minimum signers"
    )
    context_total_power = require_uint(
        quorum.get("total_power"), f"{peer.label} frozen total power", positive=True
    )
    mode = tagged_unit(
        context_record.get("mode"),
        "mode",
        f"{peer.label} consensus mode",
        {"permissioned", "npos"},
    )
    if (
        validator_count != PEER_COUNT
        or context_min_signers != 3
        or context_total_power < PEER_COUNT
        or (mode == "permissioned" and context_total_power != PEER_COUNT)
    ):
        fail(f"{peer.label} frozen context is not the exact four-validator quorum")
    subject = sumeragi.get("last_committed_subject")
    if not isinstance(subject, dict):
        fail(f"{peer.label} omitted the durable committed subject")
    block_hash = normalized_block_hash(subject.get("block_hash"), f"{peer.label} committed block")
    qc_height = require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "height"),
        f"{peer.label} CommitQC height",
        positive=True,
    )
    if qc_height != committed:
        fail(f"{peer.label} CommitQC height differs from committed height")
    require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "view"),
        f"{peer.label} CommitQC view",
    )
    tagged_unit(
        nested(sumeragi, "last_commit_qc", "certificate", "phase"),
        "phase",
        f"{peer.label} CommitQC phase",
        {"commit"},
    )
    qc_subject = nested(sumeragi, "last_commit_qc", "certificate", "subject")
    if qc_subject != subject:
        fail(f"{peer.label} CommitQC subject differs from committed subject")
    commit_record = sumeragi.get("last_commit_qc")
    assert isinstance(commit_record, dict)
    commit_validators = require_uint(
        commit_record.get("validator_count"),
        f"{peer.label} CommitQC validator count",
        positive=True,
    )
    commit_signers = require_uint(
        commit_record.get("signer_count"), f"{peer.label} CommitQC signer count"
    )
    commit_min_signers = require_uint(
        commit_record.get("min_signers"), f"{peer.label} CommitQC minimum signers"
    )
    commit_signed_power = require_uint(
        commit_record.get("signed_power"), f"{peer.label} CommitQC signed power"
    )
    commit_total_power = require_uint(
        commit_record.get("total_power"),
        f"{peer.label} CommitQC total power",
        positive=True,
    )
    if (
        commit_validators != PEER_COUNT
        or commit_min_signers != 3
        or not 3 <= commit_signers <= PEER_COUNT
        or commit_signed_power > commit_total_power
        or commit_signed_power * 3 <= commit_total_power * 2
    ):
        fail(f"{peer.label} durable CommitQC lacks the exact four-validator quorum")
    context = sumeragi.get("height_context_id")
    node_fingerprint = sumeragi.get("node_fingerprint")
    build_fingerprint = sumeragi.get("build_fingerprint")
    config_fingerprint = sumeragi.get("config_fingerprint")
    if any(value in (None, "", {}) for value in (context, node_fingerprint, build_fingerprint, config_fingerprint)):
        fail(f"{peer.label} omitted a required reducer fingerprint")

    query = urllib.parse.urlencode({"asset_definition_id": OFFLINE_ASSET_ID})
    offline = getter(f"{root}/v1/offline/readiness?{query}", 2.0)
    offline_height = require_uint(
        offline.get("evaluated_block_height"),
        f"{peer.label} offline evaluated height",
        positive=True,
    )
    offline_hash = normalized_block_hash(
        offline.get("evaluated_block_hash"), f"{peer.label} offline evaluated block"
    )
    if (
        offline.get("ready") is not True
        or offline.get("blockers") != []
        or offline.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or offline.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or offline.get("asset_definition_id") != OFFLINE_ASSET_ID
        or offline.get("asset_scale") != OFFLINE_ASSET_SCALE
        or offline_height != committed
        or offline_hash != block_hash
    ):
        fail(f"{peer.label} offline readiness is not bound to its committed block")
    artifact = offline.get("artifact_set")
    if (
        not isinstance(artifact, dict)
        or artifact.get("generation") != bundle.release.generation
        or artifact.get("manifest_sha256") != bundle.release.manifest_sha256
        or artifact.get("release_policy_sha256")
        != bundle.release.release_policy_sha256
        or artifact.get("release_attestation_sha256")
        != bundle.release.release_attestation_sha256
        or artifact.get("activation_height") != bundle.release.activation_height
        or artifact.get("withdrawal_height") != bundle.release.withdrawal_height
        or artifact.get("max_proof_bytes") != bundle.release.max_proof_bytes
        or artifact.get("asset_scale") != bundle.release.asset_scale
    ):
        fail(f"{peer.label} offline release differs from the reset bundle")

    canonical = lambda value: json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    return PeerSample(
        label=peer.label,
        height=committed,
        block_hash=block_hash,
        context=canonical(context),
        node=canonical(node_fingerprint),
        build=canonical(build_fingerprint),
        config=canonical(config_fingerprint),
        offline_release=canonical(artifact),
    )


def capture_fleet(
    bundle: BundlePlan,
    expected_source_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Require all four direct validators to expose one exact common commit."""

    samples = [
        validate_peer_health(
            peer,
            bundle,
            expected_source_commit,
            getter=getter,
            health_getter=health_getter,
        )
        for peer in bundle.peers
    ]
    baseline = samples[0]
    for sample in samples[1:]:
        for field in (
            "height",
            "block_hash",
            "context",
            "build",
            "config",
            "offline_release",
        ):
            if getattr(sample, field) != getattr(baseline, field):
                fail(f"four-validator fleet disagrees on {field}")
    nodes = tuple(sorted(sample.node for sample in samples))
    if len(set(nodes)) != PEER_COUNT:
        fail("four validator roots do not expose four distinct node identities")
    return FleetSample(
        height=baseline.height,
        block_hash=baseline.block_hash,
        context=baseline.context,
        build=baseline.build,
        config=baseline.config,
        offline_release=baseline.offline_release,
        nodes=nodes,
    )


def wait_for_fleet_sample(
    bundle: BundlePlan,
    expected_source_commit: str,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Retry startup/alignment failures until one coherent sample is available."""

    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            return capture_fleet(
                bundle,
                expected_source_commit,
                getter=getter,
                health_getter=health_getter,
            )
        except (DeploymentError, OSError) as error:
            last_error = error
            time.sleep(1)
    raise DeploymentError(f"four-validator readiness did not converge: {last_error}")


def wait_for_advancement(
    bundle: BundlePlan,
    expected_source_commit: str,
    previous: FleetSample,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Require a later common height with a different common block hash."""

    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            current = capture_fleet(
                bundle,
                expected_source_commit,
                getter=getter,
                health_getter=health_getter,
            )
            if (
                current.height > previous.height
                and current.block_hash != previous.block_hash
                and current.build == previous.build
                and current.config == previous.config
                and current.offline_release == previous.offline_release
                and current.nodes == previous.nodes
            ):
                return current
            last_error = DeploymentError("fleet has not advanced one stable common release")
        except (DeploymentError, OSError) as error:
            last_error = error
        time.sleep(1)
    raise DeploymentError(f"four-validator consensus did not advance: {last_error}")


def parse_pid_file(path: Path, uid: int, gid: int) -> int:
    """Read one private managed-child PID file through a no-follow descriptor."""

    body, info = read_regular(path, 64)
    if info.st_uid != uid or info.st_gid != gid or stat.S_IMODE(info.st_mode) & 0o077:
        fail(f"managed PID file has an unsafe owner or mode: {path}")
    try:
        text = body.decode("ascii")
    except UnicodeDecodeError as error:
        raise DeploymentError(f"managed PID file is not ASCII: {path}") from error
    if re.fullmatch(r"[1-9][0-9]*\n", text) is None or int(text) <= 1:
        fail(f"managed PID file is invalid: {path}")
    return int(text)


def expected_supervisor_argv(plist_body: bytes) -> tuple[str, ...]:
    """Return the exact generated supervisor argv from a plist."""

    try:
        payload = plistlib.loads(plist_body)
    except Exception as error:
        raise DeploymentError("generated LaunchDaemon plist is invalid") from error
    arguments = payload.get("ProgramArguments") if isinstance(payload, dict) else None
    if not isinstance(arguments, list) or not all(isinstance(value, str) for value in arguments):
        fail("generated LaunchDaemon plist lacks exact ProgramArguments")
    return tuple(arguments)


def verify_managed_peer(
    peer: PeerPlan,
    bundle: BundlePlan,
    runtime_root: Path,
    plist_body: bytes,
    installed_binary: Path,
    ops: SystemOps,
) -> tuple[int, int]:
    """Require one launchd supervisor and its exact single validator child."""

    supervisor_pid = launchd_pid(ops.launchd_print(peer.label), peer.label)
    supervisor = ops.inspect_process(supervisor_pid)
    if supervisor.uid != bundle.owner_uid:
        fail(f"{peer.label} supervisor has an unexpected owner")
    if supervisor.argv != expected_supervisor_argv(plist_body):
        fail(f"{peer.label} supervisor command differs from the generated plist")
    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    child_pid = parse_pid_file(pid_file, bundle.owner_uid, bundle.owner_gid)
    child = ops.inspect_process(child_pid)
    expected_child = (str(installed_binary), "--sora", "--config", str(peer.config))
    if child.ppid != supervisor_pid or child.uid != bundle.owner_uid or child.argv != expected_child:
        fail(f"{peer.label} PID file does not name its exact managed validator child")
    return supervisor_pid, child_pid


def restart_proof(
    bundle: BundlePlan,
    expected_source_commit: str,
    runtime_root: Path,
    plist_bodies: dict[str, bytes],
    installed_binary: Path,
    baseline: FleetSample,
    ops: SystemOps,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Terminate one exact child and prove O(1)-sealed independent recovery."""

    peer = bundle.peers[0]
    supervisor_pid, child_pid = verify_managed_peer(
        peer,
        bundle,
        runtime_root,
        plist_bodies[peer.label],
        installed_binary,
        ops,
    )
    ops.terminate(child_pid)
    deadline = time.monotonic() + RESTART_PROOF_TIMEOUT_SECONDS
    new_child: int | None = None
    while time.monotonic() < deadline:
        try:
            current_supervisor, candidate = verify_managed_peer(
                peer,
                bundle,
                runtime_root,
                plist_bodies[peer.label],
                installed_binary,
                ops,
            )
            if current_supervisor != supervisor_pid:
                fail("restart proof replaced the launchd-owned supervisor")
            if candidate != child_pid:
                new_child = candidate
                break
        except (DeploymentError, OSError):
            pass
        time.sleep(0.25)
    if new_child is None:
        fail("managed validator child did not restart within 45 seconds")
    if ops.process_exists(child_pid):
        fail("old managed validator child remained alive after restart")
    return wait_for_advancement(
        bundle,
        expected_source_commit,
        baseline,
        deadline,
        getter=getter,
        health_getter=health_getter,
    )


def rollback_cohort(snapshots: Sequence[PlistSnapshot], ops: SystemOps) -> None:
    """Unload any new jobs, restore every old plist, and reload all four old jobs."""

    errors: list[str] = []
    for snapshot in snapshots:
        if ops.launchd_print(snapshot.path.stem) is None:
            continue
        try:
            ops.bootout(snapshot.path.stem)
        except (DeploymentError, OSError):
            errors.append(f"bootout:{snapshot.path.stem}")
    for snapshot in snapshots:
        try:
            atomic_replace_owned(
                snapshot.path,
                snapshot.body,
                mode=snapshot.mode,
                uid=snapshot.uid,
                gid=snapshot.gid,
            )
        except (DeploymentError, OSError):
            errors.append(f"restore:{snapshot.path.stem}")
    for snapshot in snapshots:
        try:
            ops.bootstrap(snapshot.path)
        except (DeploymentError, OSError):
            errors.append(f"bootstrap:{snapshot.path.stem}")
    deadline = time.monotonic() + 30
    pending = {snapshot.path.stem: snapshot for snapshot in snapshots}
    while pending and time.monotonic() < deadline:
        for label, snapshot in tuple(pending.items()):
            try:
                verify_restored_snapshot(snapshot, ops)
            except (DeploymentError, OSError):
                continue
            del pending[label]
        if pending:
            time.sleep(0.25)
    errors.extend(f"verify:{label}" for label in sorted(pending))
    if errors:
        fail("four-job rollback was incomplete: " + ", ".join(errors))


def install_runtime_layout(bundle: BundlePlan) -> Path:
    """Create the root-controlled parent and owner-private reset runtime leaf."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    runtime_parent = INSTALL_ROOT / "runtime"
    ensure_root_directory(runtime_parent, 0o755)
    runtime_root = runtime_parent / bundle.manifest_sha256
    ensure_runtime_directory(runtime_root, bundle.owner_uid, bundle.owner_gid)
    ensure_runtime_directory(runtime_root / "pids", bundle.owner_uid, bundle.owner_gid)
    ensure_runtime_directory(runtime_root / "logs", bundle.owner_uid, bundle.owner_gid)
    return runtime_root


def apply_reset(
    args: argparse.Namespace,
    bundle: BundlePlan,
    sources: SourcePlan,
    old_cohort: Sequence[PlistSnapshot],
    *,
    ops: SystemOps | None = None,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> dict[str, Any]:
    """Install and validate one fresh reset, rolling all four jobs back on failure."""

    if os.geteuid() != 0:
        fail("--apply requires root")
    if len(old_cohort) != PEER_COUNT:
        fail("--apply requires exactly four authenticated rollback plists")
    ops = ops or SystemOps()

    ensure_root_directory(INSTALL_ROOT, 0o755)
    binary_store = INSTALL_ROOT / "binaries"
    supervisor_store = INSTALL_ROOT / "supervisors"
    ensure_root_directory(binary_store, 0o755)
    ensure_root_directory(supervisor_store, 0o755)
    binary_dir = binary_store / sources.binary_sha256
    supervisor_dir = supervisor_store / sources.supervisor_sha256
    installed_binary = binary_dir / "irohad"
    installed_supervisor = supervisor_dir / "taira_peer_supervisor.py"
    if binary_dir.exists() and not (installed_binary.exists() or installed_binary.is_symlink()):
        fail("content-addressed binary directory is incomplete")
    if supervisor_dir.exists() and not (
        installed_supervisor.exists() or installed_supervisor.is_symlink()
    ):
        fail("content-addressed supervisor directory is incomplete")
    if not binary_dir.exists():
        ensure_root_directory(binary_dir, 0o755)
    if not supervisor_dir.exists():
        ensure_root_directory(supervisor_dir, 0o755)
    binary_info = install_immutable(
        sources.binary, installed_binary, sources.binary_sha256
    )
    install_immutable(
        sources.supervisor, installed_supervisor, sources.supervisor_sha256
    )
    require_exact_names(binary_dir, {"irohad"}, "content-addressed binary directory")
    require_exact_names(
        supervisor_dir,
        {"taira_peer_supervisor.py"},
        "content-addressed supervisor directory",
    )
    os.chmod(binary_dir, 0o555)
    os.chmod(supervisor_dir, 0o555)
    fsync_directory(binary_store)
    fsync_directory(supervisor_store)
    require_root_controlled_file(installed_binary, executable=True)
    require_root_controlled_file(installed_supervisor, executable=True)
    runtime_root = install_runtime_layout(bundle)
    require_filesystem_headroom(
        [
            bundle.root,
            bundle.release.installed_root,
            runtime_root,
            *(peer.storage for peer in bundle.peers),
        ],
        args.minimum_free_bytes,
    )

    plist_bodies = {
        peer.label: render_plist(
            peer,
            bundle,
            sources,
            installed_binary=installed_binary,
            binary_info=binary_info,
            installed_supervisor=installed_supervisor,
            runtime_root=runtime_root,
        )
        for peer in bundle.peers
    }
    for label, body in plist_bodies.items():
        payload = plistlib.loads(body)
        if payload.get("Label") != label:
            fail(f"generated LaunchDaemon label mismatch: {label}")

    cohort_mutated = False
    release_moved = False
    guarded_signals = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    previous_handlers = {
        signum: signal.getsignal(signum) for signum in guarded_signals
    }

    def request_rollback(signum: int, _frame: object) -> NoReturn:
        raise DeploymentError(
            f"controller received signal {signum}; rolling back the four-job cohort"
        )

    for signum in guarded_signals:
        signal.signal(signum, request_rollback)
    try:
        # Recheck all four old jobs immediately before the first cohort change.
        require_bundle_runtime_unchanged(bundle)
        move_release_to_root_store(bundle)
        release_moved = True
        for snapshot in old_cohort:
            current_body, current_info = read_regular(
                snapshot.path, MAX_MANIFEST_BYTES
            )
            if (
                current_body != snapshot.body
                or stat.S_IMODE(current_info.st_mode) != snapshot.mode
                or current_info.st_uid != snapshot.uid
                or current_info.st_gid != snapshot.gid
            ):
                fail(f"old LaunchDaemon changed after dry-run capture: {snapshot.path.name}")
            verify_restored_snapshot(snapshot, ops)
        cohort_mutated = True
        # Stop the entire old cohort before publishing or starting any new job.
        for snapshot in old_cohort:
            ops.bootout(snapshot.path.stem)
        for snapshot in old_cohort:
            if ops.launchd_print(snapshot.path.stem) is not None:
                fail(f"old LaunchDaemon remained loaded: {snapshot.path.stem}")

        # Publish all four fresh plists before bootstrapping the first new job.
        for peer in bundle.peers:
            atomic_replace_owned(
                LAUNCH_DAEMONS / f"{peer.label}.plist",
                plist_bodies[peer.label],
                mode=0o644,
                uid=0,
                gid=0,
            )
        for peer in bundle.peers:
            ops.bootstrap(LAUNCH_DAEMONS / f"{peer.label}.plist")

        health_deadline = time.monotonic() + args.health_timeout_seconds
        baseline = wait_for_fleet_sample(
            bundle,
            args.expected_source_commit,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
        )
        advanced = wait_for_advancement(
            bundle,
            args.expected_source_commit,
            baseline,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
        )
        for peer in bundle.peers:
            verify_managed_peer(
                peer,
                bundle,
                runtime_root,
                plist_bodies[peer.label],
                installed_binary,
                ops,
            )
        restarted = restart_proof(
            bundle,
            args.expected_source_commit,
            runtime_root,
            plist_bodies,
            installed_binary,
            advanced,
            ops,
            getter=getter,
            health_getter=health_getter,
        )
    except BaseException as rollout_error:
        # A second termination request must not interrupt the rollback itself.
        for signum in guarded_signals:
            signal.signal(signum, signal.SIG_IGN)
        rollback_error: BaseException | None = None
        if cohort_mutated:
            try:
                rollback_cohort(old_cohort, ops)
            except BaseException as error:
                rollback_error = error
        if rollback_error is None and release_moved:
            try:
                restore_release_to_bundle(bundle)
            except BaseException as error:
                rollback_error = error
        if rollback_error is not None:
            combined = DeploymentError(
                "Taira reset failed and its exact cohort/release rollback did not complete"
            )
            if hasattr(combined, "add_note"):
                combined.add_note(
                    f"rollout failure: {type(rollout_error).__name__}: {rollout_error}"
                )
            raise combined from rollback_error
        raise
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)

    return {
        "applied": True,
        "binary": str(installed_binary),
        "binary_sha256": sources.binary_sha256,
        "bundle": str(bundle.root),
        "end_block_hash": restarted.block_hash,
        "end_height": restarted.height,
        "mandatory_offline": True,
        "peer_count": PEER_COUNT,
        "release": str(bundle.release.installed_root),
        "release_tree_sha256": bundle.release.tree_sha256,
        "restart_proof": "passed",
        "source_commit": args.expected_source_commit,
        "start_height": baseline.height,
        "supervisor": str(installed_supervisor),
        "supervisor_sha256": sources.supervisor_sha256,
    }


def build_parser() -> argparse.ArgumentParser:
    """Build the exact single-command dry-run/apply interface."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--expected-binary-sha256", required=True)
    parser.add_argument("--supervisor", type=Path, required=True)
    parser.add_argument("--expected-supervisor-sha256", required=True)
    parser.add_argument(
        "--supervisor-python",
        type=Path,
        default=DEFAULT_SUPERVISOR_PYTHON,
    )
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-kagemusha-manifest-sha256", required=True)
    parser.add_argument("--expected-kagemusha-release-policy-sha256", required=True)
    parser.add_argument(
        "--expected-kagemusha-release-attestation-sha256", required=True
    )
    parser.add_argument(
        "--health-timeout-seconds",
        type=int,
        default=DEFAULT_HEALTH_TIMEOUT_SECONDS,
    )
    parser.add_argument(
        "--minimum-free-bytes",
        type=int,
        default=DEFAULT_MINIMUM_FREE_BYTES,
    )
    parser.add_argument(
        "--maximum-fsync-latency-ms",
        type=int,
        default=DEFAULT_MAXIMUM_FSYNC_LATENCY_MS,
    )
    parser.add_argument("--apply", action="store_true")
    return parser


def validate_arguments(args: argparse.Namespace) -> None:
    """Validate scalar inputs before reading deployment paths."""

    args.expected_binary_sha256 = require_sha256(
        args.expected_binary_sha256, "expected binary SHA-256"
    )
    args.expected_supervisor_sha256 = require_sha256(
        args.expected_supervisor_sha256, "expected supervisor SHA-256"
    )
    args.expected_kagemusha_manifest_sha256 = require_sha256(
        args.expected_kagemusha_manifest_sha256,
        "expected Kagemusha manifest SHA-256",
    )
    args.expected_kagemusha_release_policy_sha256 = require_sha256(
        args.expected_kagemusha_release_policy_sha256,
        "expected Kagemusha release policy SHA-256",
    )
    args.expected_kagemusha_release_attestation_sha256 = require_sha256(
        args.expected_kagemusha_release_attestation_sha256,
        "expected Kagemusha release attestation SHA-256",
    )
    args.expected_source_commit = require_commit(args.expected_source_commit)
    if args.health_timeout_seconds <= 0:
        fail("--health-timeout-seconds must be positive")
    if args.minimum_free_bytes < DEFAULT_MINIMUM_FREE_BYTES:
        fail(
            f"--minimum-free-bytes may not be below {DEFAULT_MINIMUM_FREE_BYTES}"
        )
    if not 1 <= args.maximum_fsync_latency_ms <= DEFAULT_MAXIMUM_FSYNC_LATENCY_MS:
        fail(
            "--maximum-fsync-latency-ms must be positive and may not exceed 250"
        )


def execute(args: argparse.Namespace, *, ops: SystemOps | None = None) -> dict[str, Any]:
    """Run the read-only preflight and optional guarded apply transaction."""

    validate_arguments(args)
    if args.apply and os.geteuid() != 0:
        fail("--apply requires root; no changes were made")
    bundle = validate_bundle(
        args.bundle,
        expected_binary_sha256=args.expected_binary_sha256,
        expected_source_commit=args.expected_source_commit,
        expected_kagemusha_manifest_sha256=args.expected_kagemusha_manifest_sha256,
        expected_kagemusha_release_policy_sha256=(
            args.expected_kagemusha_release_policy_sha256
        ),
        expected_kagemusha_release_attestation_sha256=(
            args.expected_kagemusha_release_attestation_sha256
        ),
        minimum_free_bytes=args.minimum_free_bytes,
        maximum_fsync_latency_ms=args.maximum_fsync_latency_ms,
    )
    sources = validate_sources(args, bundle)
    system_ops = ops or SystemOps()
    if not args.apply:
        old_cohort = capture_old_cohort(system_ops)
        return {
            "applied": False,
            "binary_sha256": sources.binary_sha256,
            "bundle": str(bundle.root),
            "bundle_bytes": bundle.bundle_bytes,
            "free_bytes": bundle.free_bytes,
            "fsync_latency_ms": round(bundle.fsync_latency_ms, 3),
            "mandatory_offline": True,
            "mode": "read-only-dry-run",
            "peer_count": PEER_COUNT,
            "release_attestation_sha256": (
                bundle.release.release_attestation_sha256
            ),
            "release_manifest_sha256": bundle.release.manifest_sha256,
            "release_policy_sha256": bundle.release.release_policy_sha256,
            "release_tree_sha256": bundle.release.tree_sha256,
            "source_commit": args.expected_source_commit,
            "supervisor_sha256": sources.supervisor_sha256,
        }
    with exclusive_deployment_lock():
        old_cohort = capture_old_cohort(system_ops)
        return apply_reset(args, bundle, sources, old_cohort, ops=system_ops)


def main(argv: list[str] | None = None) -> int:
    """Run the controller and emit only one redaction-safe JSON summary."""

    args = build_parser().parse_args(argv)
    try:
        report = execute(args)
    except (DeploymentError, OSError, ValueError) as error:
        print(f"Taira v21 reset refused: {error}", file=sys.stderr)
        return 2
    print(json.dumps(report, ensure_ascii=True, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
