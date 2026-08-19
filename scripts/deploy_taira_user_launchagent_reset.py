#!/usr/bin/env python3
"""Guarded fresh reset for the exact four-validator Taira user LaunchAgent cohort.

The default invocation is a read-only plan.  Mutation requires ``--apply`` and
an exact confirmation string containing the activation-manifest SHA-256.  The
controller consumes the already authenticated fresh-reset bundle format used by
``deploy_taira_v21_reset.py``; it does not build, edit, or sign genesis.

Private operator material is accepted only as a runtime file argument for the
bounded consensus checks.  It is never copied into the evidence archive.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import plistlib
import pwd
import grp
import re
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
from typing import Any, Callable, Iterable, Mapping, Sequence
import urllib.error
import urllib.parse
import urllib.request

try:
    from scripts import deploy_taira_v21_reset as reset_bundle
    from scripts.operator_http_headers import load_operator_context_from_file
except ModuleNotFoundError:  # Direct execution sets sys.path to scripts/.
    import deploy_taira_v21_reset as reset_bundle
    from operator_http_headers import load_operator_context_from_file


SCHEMA = "iroha.taira.user-launchagent-reset.v1"
PEER_COUNT = 4
UID = 501
USER = "administrator"
GROUP = "staff"
DOMAIN = "user/501"
LABELS = tuple(
    f"org.sora.taira.user.validator-{number}"
    for number in range(1, PEER_COUNT + 1)
)
SYSTEM_LABELS = tuple(
    f"io.soramitsu.taira.validator-{number}"
    for number in range(1, PEER_COUNT + 1)
)
SLUGS = tuple(f"taira-validator-{number}" for number in range(1, PEER_COUNT + 1))
TORII_PORTS = tuple(29_080 + offset for offset in range(PEER_COUNT))
P2P_PORTS = tuple(33_337 + offset for offset in range(PEER_COUNT))

HOME = Path("/Users/administrator")
TAIRA_ROOT = HOME / "apps/dpn-test/taira"
LAUNCH_AGENTS = HOME / "Library/LaunchAgents"
RESET_MANIFESTS = TAIRA_ROOT / "reset-manifests"
RESET_BUNDLES = TAIRA_ROOT / "reset-bundles"
RELEASES = TAIRA_ROOT / "releases"
ROLLBACK_ROOT = TAIRA_ROOT / "rollback/user-launchagent"
LOG_ROOT = TAIRA_ROOT / "logs/user-launchagent"
LOCK_PATH = TAIRA_ROOT / ".user-launchagent-reset.lock"

MANIFEST_KEYS = frozenset(
    {
        "schema",
        "generation",
        "uid",
        "launchctl_domain",
        "labels",
        "bundle",
        "reset_manifest_sha256",
        "binary",
        "binary_sha256",
        "source_commit",
        "dpn_validator_release_commit",
        "limits",
    }
)
LIMIT_KEYS = frozenset(
    {
        "minimum_free_bytes",
        "maximum_fsync_latency_ms",
        "startup_timeout_seconds",
        "stability_timeout_seconds",
        "poll_interval_seconds",
    }
)
OLD_PLIST_KEYS = frozenset(
    {
        "Label",
        "ProgramArguments",
        "WorkingDirectory",
        "KeepAlive",
        "ProcessType",
        "ThrottleInterval",
        "StandardOutPath",
        "StandardErrorPath",
    }
)
MAX_ACTIVATION_MANIFEST_BYTES = 64 * 1024
MAX_PLIST_BYTES = 1024 * 1024
MAX_CONFIG_BYTES = 4 * 1024 * 1024
MAX_HTTP_BYTES = 2 * 1024 * 1024
MAX_BINARY_BYTES = 1024 * 1024 * 1024
MIN_FREE_BYTES = 16 * 1024 * 1024 * 1024
GENERATION_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
NETWORK_ID_RE = re.compile(r"hash:[0-9A-F]{64}#[0-9A-F]{4}\Z")
BLOCK_HASH_RE = re.compile(
    r"(?:hash:)?([0-9A-Fa-f]{64})(?:#[0-9A-Fa-f]{4})?\Z"
)
USER_LABEL_RE = re.compile(r"org[.]sora[.]taira[.]user[.]validator-[0-9]+")
RETIRED_NAMESPACE_WINDOW = 10
RETIRED_NAMESPACE_ROLLING = 0xD8F50D9988183A2E
RETIRED_NAMESPACE_SHA256 = (
    "a71a7c7011f53a1bab3642ec2ce12593f05230ace8de1e3e7645f69efac1443d"
)
ROLLING_BASE = 257
ROLLING_MASK = (1 << 64) - 1
ROLLING_REMOVE_FACTOR = 2_617_856_364_451_727_617


class ResetError(RuntimeError):
    """One fail-closed user-cohort invariant was not met."""


def fail(message: str) -> None:
    raise ResetError(message)


@dataclasses.dataclass(frozen=True)
class Layout:
    home: Path = HOME
    taira_root: Path = TAIRA_ROOT
    launch_agents: Path = LAUNCH_AGENTS
    reset_manifests: Path = RESET_MANIFESTS
    reset_bundles: Path = RESET_BUNDLES
    releases: Path = RELEASES
    rollback_root: Path = ROLLBACK_ROOT
    log_root: Path = LOG_ROOT
    lock_path: Path = LOCK_PATH


PRODUCTION_LAYOUT = Layout()


@dataclasses.dataclass(frozen=True)
class Limits:
    minimum_free_bytes: int
    maximum_fsync_latency_ms: int
    startup_timeout_seconds: float
    stability_timeout_seconds: float
    poll_interval_seconds: float


@dataclasses.dataclass(frozen=True)
class Activation:
    path: Path
    raw: bytes
    sha256: str
    generation: str
    bundle: Path
    reset_manifest_sha256: str
    binary: Path
    binary_sha256: str
    source_commit: str
    dpn_validator_release_commit: str
    limits: Limits

    @property
    def confirmation(self) -> str:
        return f"RESET-TAIRA-USER-501:{self.sha256}"


@dataclasses.dataclass(frozen=True)
class CandidatePeer:
    number: int
    label: str
    slug: str
    plist_path: Path
    plist_body: bytes
    plist_sha256: str
    workdir: Path
    config: Path
    config_sha256: str
    storage: Path
    torii_port: int


@dataclasses.dataclass(frozen=True)
class PredecessorPeer:
    number: int
    label: str
    plist_path: Path
    plist_body: bytes
    plist_sha256: str
    binary: Path
    binary_sha256: str
    workdir: Path
    config: Path
    config_sha256: str
    storage: Path
    storage_device: int
    storage_inode: int
    genesis: Path
    genesis_sha256: str
    network_id: str
    torii_port: int


@dataclasses.dataclass(frozen=True)
class FleetSample:
    height: int
    block_hash: str
    peers: tuple[dict[str, object], ...]


@dataclasses.dataclass(frozen=True)
class ResetPlan:
    activation: Activation
    network_id: str
    bundle_plan: Any
    candidate: tuple[CandidatePeer, ...]
    predecessor: tuple[PredecessorPeer, ...]
    archive_dir: Path
    log_dir: Path
    binary_identity: tuple[int, int, int, int, int, int]


def canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def metadata_identity(info: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_size,
    )


def require_exact_keys(value: Mapping[str, object], expected: frozenset[str], label: str) -> None:
    if set(value) != expected:
        fail(f"{label} keys are not exact")


def require_int(
    value: object,
    label: str,
    *,
    minimum: int,
    maximum: int,
) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < minimum
        or value > maximum
    ):
        fail(f"{label} must be an integer from {minimum} through {maximum}")
    return value


def require_number(
    value: object,
    label: str,
    *,
    minimum: float,
    maximum: float,
) -> float:
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or value < minimum
        or value > maximum
    ):
        fail(f"{label} must be from {minimum} through {maximum}")
    return float(value)


def require_sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be one lowercase SHA-256")
    return value


def require_commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        fail(f"{label} must be one full lowercase Git commit")
    return value


def require_absolute_lexical(path: Path, label: str) -> Path:
    if not path.is_absolute() or ".." in path.parts:
        fail(f"{label} must be one absolute non-traversing path")
    return path


def relative_descendant(
    path: Path,
    root: Path,
    label: str,
    *,
    minimum_parts: int,
) -> tuple[str, ...]:
    require_absolute_lexical(path, label)
    try:
        relative = path.relative_to(root)
    except ValueError:
        fail(f"{label} must remain under {root}")
    parts = relative.parts
    if len(parts) < minimum_parts or any(part in {"", ".", ".."} for part in parts):
        fail(f"{label} is too broad")
    return parts


def require_no_symlink_ancestry(path: Path, root: Path, label: str) -> None:
    """Reject links from one trusted root through the exact target."""

    relative_descendant(path, root, label, minimum_parts=0)
    current = root
    try:
        root_info = root.lstat()
    except OSError as error:
        raise ResetError(f"{label} trusted root is unavailable") from error
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        fail(f"{label} trusted root is not one real directory")
    parts = path.relative_to(root).parts
    for index, part in enumerate(parts):
        current = current / part
        try:
            info = current.lstat()
        except FileNotFoundError:
            return
        except OSError as error:
            raise ResetError(f"{label} ancestry is unavailable: {current}") from error
        if stat.S_ISLNK(info.st_mode):
            fail(f"{label} contains a symlink: {current}")
        if index + 1 < len(parts) and not stat.S_ISDIR(info.st_mode):
            fail(f"{label} contains a non-directory ancestor: {current}")


def read_regular(
    path: Path,
    maximum: int,
    label: str,
    *,
    owner_uid: int | None = None,
    exact_mode: int | None = None,
) -> tuple[bytes, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise ResetError(f"{label} is unavailable: {path}") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum
    ):
        fail(f"{label} must be one bounded regular non-linked file")
    if owner_uid is not None and before.st_uid != owner_uid:
        fail(f"{label} has the wrong owner")
    if exact_mode is not None and stat.S_IMODE(before.st_mode) != exact_mode:
        fail(f"{label} has the wrong mode")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if metadata_identity(opened) != metadata_identity(before):
            fail(f"{label} changed while opening")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        body = b"".join(chunks)
    finally:
        os.close(descriptor)
    if len(body) > maximum:
        fail(f"{label} exceeds its size bound")
    after = path.lstat()
    if metadata_identity(after) != metadata_identity(before):
        fail(f"{label} changed while reading")
    return body, before


def hash_regular(
    path: Path,
    maximum: int,
    label: str,
    *,
    owner_uid: int | None = None,
    exact_mode: int | None = None,
) -> tuple[str, os.stat_result]:
    body, info = read_regular(
        path,
        maximum,
        label,
        owner_uid=owner_uid,
        exact_mode=exact_mode,
    )
    return hashlib.sha256(body).hexdigest(), info


def parse_toml(path: Path, owner_uid: int, label: str) -> tuple[dict[str, Any], str]:
    body, _ = read_regular(path, MAX_CONFIG_BYTES, label, owner_uid=owner_uid)
    try:
        value = tomllib.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ResetError(f"{label} is not valid UTF-8 TOML") from error
    if not isinstance(value, dict):
        fail(f"{label} is not a TOML table")
    return value, hashlib.sha256(body).hexdigest()


def parse_address_port(value: object, label: str) -> int:
    if not isinstance(value, str):
        fail(f"{label} is not an address literal")
    match = re.fullmatch(r"addr:(?:127[.]0[.]0[.]1|localhost):([0-9]{1,5})#[0-9A-F]{4}", value)
    if match is None:
        fail(f"{label} is not one canonical loopback address")
    port = int(match.group(1))
    if not 0 < port < 65_536:
        fail(f"{label} port is invalid")
    return port


def load_activation(path: Path, layout: Layout = PRODUCTION_LAYOUT) -> Activation:
    path = require_absolute_lexical(path, "activation manifest")
    relative_descendant(
        path,
        layout.reset_manifests,
        "activation manifest",
        minimum_parts=1,
    )
    require_no_symlink_ancestry(path, layout.taira_root, "activation manifest")
    raw, _ = read_regular(
        path,
        MAX_ACTIVATION_MANIFEST_BYTES,
        "activation manifest",
        owner_uid=UID,
        exact_mode=0o600,
    )
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ResetError("activation manifest is not valid UTF-8 JSON") from error
    if not isinstance(payload, dict):
        fail("activation manifest must be one JSON object")
    require_exact_keys(payload, MANIFEST_KEYS, "activation manifest")
    if (
        payload.get("schema") != SCHEMA
        or payload.get("uid") != UID
        or payload.get("launchctl_domain") != DOMAIN
        or payload.get("labels") != list(LABELS)
    ):
        fail("activation manifest does not name the exact user/501 four-validator cohort")
    generation = payload.get("generation")
    if not isinstance(generation, str) or GENERATION_RE.fullmatch(generation) is None:
        fail("activation generation is not canonical")
    bundle_value = payload.get("bundle")
    binary_value = payload.get("binary")
    if not isinstance(bundle_value, str) or not isinstance(binary_value, str):
        fail("activation candidate paths must be strings")
    bundle = Path(bundle_value)
    binary = Path(binary_value)
    bundle_parts = relative_descendant(
        bundle,
        layout.reset_bundles,
        "candidate reset bundle",
        minimum_parts=1,
    )
    binary_parts = relative_descendant(
        binary,
        layout.releases,
        "candidate binary",
        minimum_parts=2,
    )
    if len(bundle_parts) != 1 or len(binary_parts) != 2 or binary_parts[-1] != "iroha3d":
        fail("candidate reset bundle or binary path shape is not exact")
    require_no_symlink_ancestry(bundle, layout.taira_root, "candidate reset bundle")
    require_no_symlink_ancestry(binary, layout.taira_root, "candidate binary")
    limits = payload.get("limits")
    if not isinstance(limits, dict):
        fail("activation limits must be one object")
    require_exact_keys(limits, LIMIT_KEYS, "activation limits")
    parsed_limits = Limits(
        minimum_free_bytes=require_int(
            limits.get("minimum_free_bytes"),
            "minimum_free_bytes",
            minimum=MIN_FREE_BYTES,
            maximum=1024 * 1024 * 1024 * 1024,
        ),
        maximum_fsync_latency_ms=require_int(
            limits.get("maximum_fsync_latency_ms"),
            "maximum_fsync_latency_ms",
            minimum=1,
            maximum=10_000,
        ),
        startup_timeout_seconds=require_number(
            limits.get("startup_timeout_seconds"),
            "startup_timeout_seconds",
            minimum=30,
            maximum=600,
        ),
        stability_timeout_seconds=require_number(
            limits.get("stability_timeout_seconds"),
            "stability_timeout_seconds",
            minimum=5,
            maximum=300,
        ),
        poll_interval_seconds=require_number(
            limits.get("poll_interval_seconds"),
            "poll_interval_seconds",
            minimum=0.25,
            maximum=5,
        ),
    )
    return Activation(
        path=path,
        raw=raw,
        sha256=hashlib.sha256(raw).hexdigest(),
        generation=generation,
        bundle=bundle,
        reset_manifest_sha256=require_sha256(
            payload.get("reset_manifest_sha256"), "reset_manifest_sha256"
        ),
        binary=binary,
        binary_sha256=require_sha256(payload.get("binary_sha256"), "binary_sha256"),
        source_commit=require_commit(payload.get("source_commit"), "source_commit"),
        dpn_validator_release_commit=require_commit(
            payload.get("dpn_validator_release_commit"),
            "dpn_validator_release_commit",
        ),
        limits=parsed_limits,
    )


def generated_plist(
    peer: Any,
    *,
    label: str,
    binary: Path,
    log_dir: Path,
) -> bytes:
    payload = {
        "Label": label,
        "ProgramArguments": [str(binary), "--sora", "--config", str(peer.config)],
        "WorkingDirectory": str(peer.workdir),
        "KeepAlive": True,
        "ProcessType": "Standard",
        "ThrottleInterval": 30,
        "StandardOutPath": str(log_dir / f"{label}.stdout.log"),
        "StandardErrorPath": str(log_dir / f"{label}.stderr.log"),
    }
    return plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)


def parse_old_plist(
    number: int,
    path: Path,
    layout: Layout,
) -> PredecessorPeer:
    label = LABELS[number - 1]
    body, info = read_regular(
        path,
        MAX_PLIST_BYTES,
        f"predecessor plist {label}",
        owner_uid=UID,
        exact_mode=0o600,
    )
    try:
        payload = plistlib.loads(body)
    except Exception as error:
        raise ResetError(f"predecessor plist is invalid: {label}") from error
    if not isinstance(payload, dict) or set(payload) != OLD_PLIST_KEYS:
        fail(f"predecessor plist keys are not exact: {label}")
    arguments = payload.get("ProgramArguments")
    if (
        payload.get("Label") != label
        or payload.get("KeepAlive") is not True
        or payload.get("ProcessType") != "Standard"
        or payload.get("ThrottleInterval") != 30
        or not isinstance(arguments, list)
        or len(arguments) != 4
        or arguments[1:3] != ["--sora", "--config"]
        or not all(isinstance(item, str) for item in arguments)
    ):
        fail(f"predecessor plist semantics are not exact: {label}")
    binary = Path(arguments[0])
    config = Path(arguments[3])
    workdir_value = payload.get("WorkingDirectory")
    if not isinstance(workdir_value, str):
        fail(f"predecessor plist working directory is invalid: {label}")
    workdir = Path(workdir_value)
    release_parts = relative_descendant(
        workdir,
        layout.releases,
        f"predecessor workdir {label}",
        minimum_parts=2,
    )
    if (
        len(release_parts) != 2
        or release_parts[-1] != SLUGS[number - 1]
        or config != workdir / "config.toml"
        or binary != workdir.parent / "iroha3d"
    ):
        fail(f"predecessor release paths are not exact: {label}")
    for candidate_path, candidate_label in (
        (binary, f"predecessor binary {label}"),
        (config, f"predecessor config {label}"),
        (workdir, f"predecessor workdir {label}"),
        (workdir / "storage", f"predecessor storage {label}"),
    ):
        require_no_symlink_ancestry(candidate_path, layout.taira_root, candidate_label)
    stdout = payload.get("StandardOutPath")
    stderr = payload.get("StandardErrorPath")
    if (
        not isinstance(stdout, str)
        or not isinstance(stderr, str)
        or Path(stdout) != workdir / "logs/iroha3d.stdout.log"
        or Path(stderr) != workdir / "logs/iroha3d.stderr.log"
    ):
        fail(f"predecessor log paths are not exact: {label}")
    config_payload, config_sha = parse_toml(config, UID, f"predecessor config {label}")
    genesis_table = config_payload.get("genesis")
    torii_table = config_payload.get("torii")
    if not isinstance(genesis_table, dict) or not isinstance(torii_table, dict):
        fail(f"predecessor config lacks genesis or Torii: {label}")
    genesis_value = genesis_table.get("file")
    network_id = genesis_table.get("expected_hash")
    if not isinstance(genesis_value, str) or not isinstance(network_id, str):
        fail(f"predecessor config lacks genesis binding: {label}")
    genesis = Path(genesis_value)
    if (
        genesis != workdir.parent / "genesis.signed.nrt"
        or NETWORK_ID_RE.fullmatch(network_id) is None
    ):
        fail(f"predecessor genesis binding is not exact: {label}")
    require_no_symlink_ancestry(genesis, layout.taira_root, f"predecessor genesis {label}")
    torii_port = parse_address_port(torii_table.get("address"), f"{label} torii.address")
    if torii_port != TORII_PORTS[number - 1]:
        fail(f"predecessor Torii port is not exact: {label}")
    storage = workdir / "storage"
    storage_info = storage.lstat()
    if not stat.S_ISDIR(storage_info.st_mode) or stat.S_ISLNK(storage_info.st_mode):
        fail(f"predecessor storage is not one real directory: {label}")
    binary_sha, _ = hash_regular(binary, MAX_BINARY_BYTES, f"predecessor binary {label}")
    genesis_sha, _ = hash_regular(genesis, 64 * 1024 * 1024, f"predecessor genesis {label}")
    return PredecessorPeer(
        number=number,
        label=label,
        plist_path=path,
        plist_body=body,
        plist_sha256=hashlib.sha256(body).hexdigest(),
        binary=binary,
        binary_sha256=binary_sha,
        workdir=workdir,
        config=config,
        config_sha256=config_sha,
        storage=storage,
        storage_device=storage_info.st_dev,
        storage_inode=storage_info.st_ino,
        genesis=genesis,
        genesis_sha256=genesis_sha,
        network_id=network_id,
        torii_port=torii_port,
    )


def require_coherent_predecessor(peers: Sequence[PredecessorPeer]) -> None:
    if len(peers) != PEER_COUNT:
        fail("predecessor cohort is not exactly four peers")
    baseline = peers[0]
    for peer in peers[1:]:
        if (
            peer.binary != baseline.binary
            or peer.binary_sha256 != baseline.binary_sha256
            or peer.genesis != baseline.genesis
            or peer.genesis_sha256 != baseline.genesis_sha256
            or peer.network_id != baseline.network_id
        ):
            fail("predecessor cohort mixes binary or genesis identities")
    if len({(peer.storage_device, peer.storage_inode) for peer in peers}) != PEER_COUNT:
        fail("predecessor cohort reuses a storage directory")


def contains_retired_namespace(body: bytes) -> bool:
    """Detect the retired ten-byte namespace without retaining its spelling."""

    lowered = body.lower()
    window = RETIRED_NAMESPACE_WINDOW
    if len(lowered) < window:
        return False
    rolling = 0
    for value in lowered[:window]:
        rolling = ((rolling * ROLLING_BASE) + value) & ROLLING_MASK
    for offset in range(0, len(lowered) - window + 1):
        candidate = lowered[offset : offset + window]
        if (
            rolling == RETIRED_NAMESPACE_ROLLING
            and hashlib.sha256(candidate).hexdigest() == RETIRED_NAMESPACE_SHA256
        ):
            return True
        if offset + window < len(lowered):
            rolling = (
                (
                    rolling
                    - lowered[offset] * ROLLING_REMOVE_FACTOR
                )
                * ROLLING_BASE
                + lowered[offset + window]
            ) & ROLLING_MASK
    return False


def require_nevo_bundle(bundle: Path, peers: Sequence[Any]) -> None:
    """Reject the discarded test namespace in every public reset projection."""

    public_paths = [
        bundle / "genesis.json",
        bundle / "reset-manifest.json",
        *(peer.config for peer in peers),
    ]
    for path in public_paths:
        body, _ = read_regular(path, MAX_CONFIG_BYTES * 16, f"public reset projection {path.name}")
        if contains_retired_namespace(body):
            fail(f"public reset projection contains the retired namespace: {path}")
    genesis, _ = read_regular(
        bundle / "genesis.json", 64 * 1024 * 1024, "unsigned NEVO genesis"
    )
    if b"nevo.dpn" not in genesis.lower():
        fail("unsigned reset genesis does not provision nevo.dpn")


class LaunchctlOps:
    def __init__(self, *, timeout_seconds: float = 10.0) -> None:
        self.timeout_seconds = timeout_seconds

    def run(self, arguments: Sequence[str], *, check: bool) -> subprocess.CompletedProcess[bytes]:
        try:
            result = subprocess.run(
                list(arguments),
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=self.timeout_seconds,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ResetError(f"bounded command failed: {arguments[0]}") from error
        if check and result.returncode != 0:
            fail(f"bounded command returned {result.returncode}: {' '.join(arguments[:3])}")
        return result

    def job_loaded(self, domain: str, label: str) -> bool:
        return self.job_output(domain, label) is not None

    def job_output(self, domain: str, label: str) -> bytes | None:
        result = self.run(["/bin/launchctl", "print", f"{domain}/{label}"], check=False)
        if result.returncode not in {0, 113}:
            # launchctl commonly returns 113 for an absent service.  Other
            # failures are not safely distinguishable from an unavailable domain.
            fail(f"launchctl could not inspect {domain}/{label}")
        if result.returncode != 0:
            return None
        if len(result.stdout) > MAX_HTTP_BYTES:
            fail(f"launchctl output exceeds its bound: {domain}/{label}")
        return result.stdout

    def exact_user_labels(self) -> set[str]:
        result = self.run(["/bin/launchctl", "print", DOMAIN], check=True)
        if len(result.stdout) > MAX_HTTP_BYTES:
            fail("launchctl user-domain output exceeds its bound")
        try:
            output = result.stdout.decode("utf-8", errors="strict")
        except UnicodeDecodeError as error:
            raise ResetError("launchctl domain output is not UTF-8") from error
        return set(USER_LABEL_RE.findall(output))

    def require_initial_cohort(self) -> None:
        observed = self.exact_user_labels()
        if observed != set(LABELS):
            fail("user/501 does not contain exactly the four managed validator labels")
        for label in LABELS:
            if not self.job_loaded(DOMAIN, label):
                fail(f"managed user LaunchAgent is not loaded: {label}")
        for label in SYSTEM_LABELS:
            if self.job_loaded("system", label):
                fail(f"dormant system validator label is loaded: {label}")

    def require_loaded_definition(self, plist_path: Path, plist_body: bytes) -> None:
        try:
            payload = plistlib.loads(plist_body)
        except Exception as error:
            raise ResetError(f"cannot inspect expected plist: {plist_path}") from error
        if not isinstance(payload, dict) or not isinstance(payload.get("Label"), str):
            fail(f"expected plist is malformed: {plist_path}")
        label = payload["Label"]
        output = self.job_output(DOMAIN, label)
        if output is None:
            fail(f"managed user LaunchAgent is not loaded: {label}")
        try:
            text = output.decode("utf-8", errors="strict")
        except UnicodeDecodeError as error:
            raise ResetError(f"launchctl job output is not UTF-8: {label}") from error
        arguments = payload.get("ProgramArguments")
        required_lines = {
            f"\tpath = {plist_path}",
            "\ttype = LaunchAgent",
            "\tstate = running",
            f"\tprogram = {arguments[0] if isinstance(arguments, list) else ''}",
            f"\tworking directory = {payload.get('WorkingDirectory', '')}",
            f"\tstdout path = {payload.get('StandardOutPath', '')}",
            f"\tstderr path = {payload.get('StandardErrorPath', '')}",
            f"\tdomain = {DOMAIN}",
        }
        observed_lines = set(text.splitlines())
        if not required_lines.issubset(observed_lines):
            fail(f"loaded LaunchAgent definition differs from its exact plist: {label}")
        if isinstance(arguments, list):
            for argument in arguments:
                if f"\t\t{argument}" not in observed_lines:
                    fail(f"loaded LaunchAgent arguments differ from its exact plist: {label}")

    def bootout(self, label: str) -> None:
        self.run(["/bin/launchctl", "bootout", f"{DOMAIN}/{label}"], check=True)

    def bootstrap(self, plist_path: Path) -> None:
        self.run(
            ["/bin/launchctl", "bootstrap", DOMAIN, str(plist_path)],
            check=True,
        )

    def wait_absent(self, labels: Iterable[str], timeout_seconds: float = 15.0) -> None:
        pending = set(labels)
        deadline = time.monotonic() + timeout_seconds
        while pending and time.monotonic() < deadline:
            pending = {label for label in pending if self.job_loaded(DOMAIN, label)}
            if pending:
                time.sleep(0.1)
        if pending:
            fail(f"LaunchAgents did not stop within the bound: {sorted(pending)}")


def build_plan(
    activation: Activation,
    *,
    layout: Layout = PRODUCTION_LAYOUT,
    launchctl: LaunchctlOps | None = None,
    validate_bundle_fn: Callable[..., Any] = reset_bundle.validate_bundle,
) -> ResetPlan:
    if launchctl is not None:
        launchctl.require_initial_cohort()
    archive_dir = layout.rollback_root / activation.generation
    log_dir = layout.log_root / activation.generation
    relative_descendant(
        archive_dir,
        layout.rollback_root,
        "rollback archive directory",
        minimum_parts=1,
    )
    relative_descendant(
        log_dir,
        layout.log_root,
        "candidate log directory",
        minimum_parts=1,
    )
    require_no_symlink_ancestry(archive_dir, layout.taira_root, "rollback archive directory")
    require_no_symlink_ancestry(log_dir, layout.taira_root, "candidate log directory")
    if archive_dir.exists() or log_dir.exists():
        fail("activation generation already has rollback evidence or candidate logs")
    bundle_plan = validate_bundle_fn(
        activation.bundle,
        expected_reset_manifest_sha256=activation.reset_manifest_sha256,
        expected_binary_sha256=activation.binary_sha256,
        expected_source_commit=activation.source_commit,
        expected_dpn_validator_release_commit=(
            activation.dpn_validator_release_commit
        ),
        minimum_free_bytes=activation.limits.minimum_free_bytes,
        maximum_fsync_latency_ms=activation.limits.maximum_fsync_latency_ms,
        headroom_anchor=layout.taira_root,
    )
    if bundle_plan.root != activation.bundle or len(bundle_plan.peers) != PEER_COUNT:
        fail("authenticated reset bundle path or peer count changed")
    if bundle_plan.owner_uid != UID:
        fail("authenticated reset bundle is not owned by administrator")
    binary_sha, binary_info = hash_regular(
        activation.binary,
        MAX_BINARY_BYTES,
        "candidate iroha3d binary",
        owner_uid=UID,
    )
    if (
        binary_sha != activation.binary_sha256
        or not stat.S_ISREG(binary_info.st_mode)
        or stat.S_IMODE(binary_info.st_mode) & 0o111 == 0
    ):
        fail("candidate binary does not match its executable manifest binding")
    require_nevo_bundle(activation.bundle, bundle_plan.peers)
    network_hash = bundle_plan.manifest.get("genesis_expected_hash")
    if not isinstance(network_hash, str):
        fail("reset manifest lacks its genesis expected hash")
    network_id = reset_bundle.validator_renderer._format_literal(
        "hash", network_hash.upper()
    )
    if NETWORK_ID_RE.fullmatch(network_id) is None:
        fail("reset manifest did not produce one canonical NetworkId")
    candidate: list[CandidatePeer] = []
    candidate_storage_identities: set[tuple[int, int]] = set()
    for number, (label, slug, peer, torii_port) in enumerate(
        zip(LABELS, SLUGS, bundle_plan.peers, TORII_PORTS),
        start=1,
    ):
        if peer.slug != slug or peer.torii_port != torii_port:
            fail("authenticated bundle peer order differs from the user cohort")
        if (
            peer.workdir != activation.bundle / "rendered" / slug
            or peer.config != peer.workdir / "config.toml"
            or peer.storage != peer.workdir / "storage"
        ):
            fail("authenticated candidate config or storage path is not exact")
        storage_info = peer.storage.lstat()
        storage_identity = (storage_info.st_dev, storage_info.st_ino)
        if (
            stat.S_ISLNK(storage_info.st_mode)
            or not stat.S_ISDIR(storage_info.st_mode)
            or any(peer.storage.iterdir())
            or storage_identity in candidate_storage_identities
        ):
            fail("candidate storage is not four distinct fresh bundle directories")
        candidate_storage_identities.add(storage_identity)
        plist_path = layout.launch_agents / f"{label}.plist"
        body = generated_plist(peer, label=label, binary=activation.binary, log_dir=log_dir)
        candidate.append(
            CandidatePeer(
                number=number,
                label=label,
                slug=slug,
                plist_path=plist_path,
                plist_body=body,
                plist_sha256=hashlib.sha256(body).hexdigest(),
                workdir=peer.workdir,
                config=peer.config,
                config_sha256=peer.config_sha256,
                storage=peer.storage,
                torii_port=torii_port,
            )
        )
    predecessor = tuple(
        parse_old_plist(
            number,
            layout.launch_agents / f"{label}.plist",
            layout,
        )
        for number, label in enumerate(LABELS, start=1)
    )
    require_coherent_predecessor(predecessor)
    predecessor_storage_identities = {
        (peer.storage_device, peer.storage_inode) for peer in predecessor
    }
    if candidate_storage_identities & predecessor_storage_identities:
        fail("candidate storage aliases predecessor rollback storage")
    if launchctl is not None:
        for peer in predecessor:
            launchctl.require_loaded_definition(peer.plist_path, peer.plist_body)
    if predecessor[0].network_id == network_id:
        fail("fresh reset candidate reuses the predecessor NetworkId")
    return ResetPlan(
        activation=activation,
        network_id=network_id,
        bundle_plan=bundle_plan,
        candidate=tuple(candidate),
        predecessor=predecessor,
        archive_dir=archive_dir,
        log_dir=log_dir,
        binary_identity=metadata_identity(binary_info),
    )


class _RejectRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        del request, fp, code, msg, headers, newurl
        return None


class HealthClient:
    def __init__(self) -> None:
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}),
            _RejectRedirects(),
        )

    def _request(
        self,
        url: str,
        *,
        headers: Mapping[str, str] | None = None,
        parse_json: bool,
    ) -> object:
        parsed = urllib.parse.urlsplit(url)
        if (
            parsed.scheme != "http"
            or parsed.hostname not in {"127.0.0.1", "localhost"}
            or parsed.username is not None
            or parsed.password is not None
            or parsed.fragment
        ):
            fail("health URL must be an absolute credential-free loopback URL")
        request = urllib.request.Request(
            url,
            method="GET",
            headers={"Accept": "application/json", **(headers or {})},
        )
        try:
            with self.opener.open(request, timeout=2.0) as response:
                if response.status != 200:
                    fail(f"health endpoint returned HTTP {response.status}")
                body = response.read(MAX_HTTP_BYTES + 1)
        except (OSError, urllib.error.URLError, TimeoutError) as error:
            raise ResetError(f"health endpoint is unavailable: {parsed.path}") from error
        if len(body) > MAX_HTTP_BYTES:
            fail("health response exceeds its byte bound")
        if not parse_json:
            return None
        try:
            value = json.loads(body)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ResetError(f"health endpoint returned invalid JSON: {parsed.path}") from error
        if not isinstance(value, dict):
            fail(f"health endpoint did not return one object: {parsed.path}")
        return value

    def unsigned_liveness(self, ports: Sequence[int]) -> None:
        for port in ports:
            root = f"http://127.0.0.1:{port}"
            self._request(f"{root}/health", parse_json=False)
            self._request(f"{root}/readyz", parse_json=False)

    def peer_sample(self, port: int, operator_context: Any) -> dict[str, object]:
        root = f"http://127.0.0.1:{port}"
        self._request(f"{root}/health", parse_json=False)
        self._request(f"{root}/readyz", parse_json=False)
        status = self._request(f"{root}/status", parse_json=True)
        target = "/v1/sumeragi/status"
        headers = operator_context.headers("GET", target, b"")
        sumeragi = self._request(
            f"{root}{target}",
            headers=headers,
            parse_json=True,
        )
        assert isinstance(status, dict) and isinstance(sumeragi, dict)
        blocks = status.get("blocks")
        height = sumeragi.get("height")
        committed = sumeragi.get("last_committed_height")
        context = sumeragi.get("height_context")
        subject = sumeragi.get("last_committed_subject")
        if (
            isinstance(blocks, bool)
            or not isinstance(blocks, int)
            or blocks < 1
            or isinstance(height, bool)
            or not isinstance(height, int)
            or height < 1
            or isinstance(committed, bool)
            or not isinstance(committed, int)
            or committed != blocks
            or committed > height
            or sumeragi.get("protocol_version") != 4
            or sumeragi.get("restart_required") is not False
            or not isinstance(context, dict)
            or not isinstance(subject, dict)
        ):
            fail("validator durable height or reducer status is not coherent")
        quorum = context.get("quorum")
        if (
            context.get("validator_count") != PEER_COUNT
            or not isinstance(quorum, dict)
            or quorum.get("min_signers") != 3
            or isinstance(quorum.get("total_power"), bool)
            or not isinstance(quorum.get("total_power"), int)
            or quorum["total_power"] < PEER_COUNT
        ):
            fail("validator does not expose the exact four-validator 3-of-4 quorum")
        block_hash = subject.get("block_hash")
        if not isinstance(block_hash, str):
            fail("validator omitted its committed block hash")
        match = BLOCK_HASH_RE.fullmatch(block_hash)
        if match is None:
            fail("validator committed block hash is not canonical")
        return {
            "port": port,
            "height": committed,
            "block_hash": match.group(1).lower(),
            "validator_count": PEER_COUNT,
            "minimum_signers": 3,
            "total_power": quorum["total_power"],
        }

    def fleet_sample(self, ports: Sequence[int], operator_context: Any) -> FleetSample:
        samples = tuple(self.peer_sample(port, operator_context) for port in ports)
        if len(samples) != PEER_COUNT or len({sample["port"] for sample in samples}) != PEER_COUNT:
            fail("health sample is not the exact four-validator cohort")
        heights = {sample["height"] for sample in samples}
        hashes = {sample["block_hash"] for sample in samples}
        if len(heights) != 1 or len(hashes) != 1:
            fail("four validators disagree on their durable committed frontier")
        return FleetSample(
            height=int(next(iter(heights))),
            block_hash=str(next(iter(hashes))),
            peers=samples,
        )

    def wait_fleet(
        self,
        ports: Sequence[int],
        network_id: str,
        private_key_file: Path,
        limits: Limits,
    ) -> tuple[FleetSample, FleetSample]:
        context = load_operator_context_from_file(network_id, private_key_file)
        deadline = time.monotonic() + limits.startup_timeout_seconds
        last_error: Exception | None = None
        first: FleetSample | None = None
        while time.monotonic() < deadline:
            try:
                first = self.fleet_sample(ports, context)
                break
            except (ResetError, OSError) as error:
                last_error = error
                time.sleep(limits.poll_interval_seconds)
        if first is None:
            raise ResetError(f"four-validator readiness did not converge: {last_error}")
        stable_deadline = time.monotonic() + limits.stability_timeout_seconds
        time.sleep(limits.poll_interval_seconds)
        while time.monotonic() < stable_deadline:
            try:
                current = self.fleet_sample(ports, context)
                same_frontier = (
                    current.height == first.height
                    and current.block_hash == first.block_hash
                )
                advanced_frontier = (
                    current.height > first.height
                    and current.block_hash != first.block_hash
                )
                if same_frontier or advanced_frontier:
                    return first, current
            except (ResetError, OSError) as error:
                last_error = error
            time.sleep(limits.poll_interval_seconds)
        raise ResetError(f"four-validator frontier did not remain coherent: {last_error}")


def require_runtime_identity(layout: Layout = PRODUCTION_LAYOUT) -> None:
    if sys.platform != "darwin":
        fail("user LaunchAgent reset is supported only on macOS")
    if os.geteuid() != UID or os.getuid() != UID:
        fail("user LaunchAgent reset must run directly as administrator uid 501")
    try:
        account = pwd.getpwuid(UID)
        group = grp.getgrgid(account.pw_gid)
    except KeyError as error:
        raise ResetError("administrator/staff identity is unavailable") from error
    if account.pw_name != USER or group.gr_name != GROUP:
        fail("uid 501 is not the exact administrator:staff runtime identity")
    for path, label in (
        (layout.home, "administrator home"),
        (layout.taira_root, "Taira root"),
        (layout.launch_agents, "LaunchAgents root"),
    ):
        info = path.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != UID
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            fail(f"{label} must be one administrator-owned mode-0700 directory")


def ensure_private_directory(path: Path, root: Path) -> None:
    relative = relative_descendant(path, root, "private output directory", minimum_parts=1)
    current = root
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or root_info.st_uid != UID
    ):
        fail("private output root is unsafe")
    for part in relative:
        current = current / part
        try:
            info = current.lstat()
        except FileNotFoundError:
            current.mkdir(mode=0o700)
            info = current.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or info.st_uid != UID
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            fail(f"private output directory is unsafe: {current}")


def atomic_write(path: Path, body: bytes, *, mode: int, replace: bool) -> None:
    parent = path.parent
    parent_info = parent.lstat()
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or stat.S_ISLNK(parent_info.st_mode)
        or parent_info.st_uid != UID
    ):
        fail(f"atomic output parent is unsafe: {parent}")
    if path.exists() or path.is_symlink():
        existing = path.lstat()
        if (
            not replace
            or not stat.S_ISREG(existing.st_mode)
            or stat.S_ISLNK(existing.st_mode)
            or existing.st_nlink != 1
            or existing.st_uid != UID
        ):
            fail(f"atomic output target is unsafe: {path}")
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.next-", dir=parent)
    temporary = Path(temporary_name)
    try:
        os.fchmod(descriptor, mode)
        os.fchown(descriptor, UID, os.getgid())
        with os.fdopen(descriptor, "wb", closefd=True) as output:
            output.write(body)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if temporary.exists():
            temporary.unlink()
    final = path.lstat()
    if (
        not stat.S_ISREG(final.st_mode)
        or final.st_uid != UID
        or stat.S_IMODE(final.st_mode) != mode
        or final.st_nlink != 1
    ):
        fail(f"atomic output did not retain exact custody: {path}")


def write_json(path: Path, value: object, *, replace: bool = False) -> None:
    atomic_write(path, canonical_json(value), mode=0o600, replace=replace)


def peer_evidence(peer: PredecessorPeer) -> dict[str, object]:
    return {
        "number": peer.number,
        "label": peer.label,
        "plist": str(peer.plist_path),
        "plist_sha256": peer.plist_sha256,
        "binary": str(peer.binary),
        "binary_sha256": peer.binary_sha256,
        "workdir": str(peer.workdir),
        "config": str(peer.config),
        "config_sha256": peer.config_sha256,
        "storage": str(peer.storage),
        "storage_device": peer.storage_device,
        "storage_inode": peer.storage_inode,
        "genesis": str(peer.genesis),
        "genesis_sha256": peer.genesis_sha256,
        "network_id": peer.network_id,
        "torii_port": peer.torii_port,
    }


def fleet_evidence(samples: tuple[FleetSample, FleetSample]) -> dict[str, object]:
    before, after = samples
    return {
        "before": dataclasses.asdict(before),
        "after": dataclasses.asdict(after),
    }


def prepare_archive(
    plan: ResetPlan,
    old_fleet: tuple[FleetSample, FleetSample],
    layout: Layout,
) -> None:
    ensure_private_directory(plan.archive_dir, layout.taira_root)
    ensure_private_directory(plan.log_dir, layout.taira_root)
    atomic_write(
        plan.archive_dir / "activation-manifest.json",
        plan.activation.raw,
        mode=0o600,
        replace=False,
    )
    for peer in plan.predecessor:
        atomic_write(
            plan.archive_dir / f"{peer.label}.plist",
            peer.plist_body,
            mode=0o600,
            replace=False,
        )
    write_json(
        plan.archive_dir / "predecessor.json",
        {
            "schema": "iroha.taira.user-launchagent-predecessor.v1",
            "captured_at": utc_now(),
            "activation_manifest_sha256": plan.activation.sha256,
            "launchctl_domain": DOMAIN,
            "labels": list(LABELS),
            "fleet": fleet_evidence(old_fleet),
            "peers": [peer_evidence(peer) for peer in plan.predecessor],
        },
    )


def require_plan_unchanged(plan: ResetPlan) -> None:
    activation_sha, _ = hash_regular(
        plan.activation.path,
        MAX_ACTIVATION_MANIFEST_BYTES,
        "activation manifest",
        owner_uid=UID,
        exact_mode=0o600,
    )
    if activation_sha != plan.activation.sha256:
        fail("activation manifest changed after planning")
    binary_sha, binary_info = hash_regular(
        plan.activation.binary,
        MAX_BINARY_BYTES,
        "candidate binary",
        owner_uid=UID,
    )
    if (
        binary_sha != plan.activation.binary_sha256
        or metadata_identity(binary_info) != plan.binary_identity
    ):
        fail("candidate binary changed after planning")
    reset_bundle.require_mutable_bundle_identities(
        plan.bundle_plan, phase="before user reset"
    )
    for peer in plan.predecessor:
        body, _ = read_regular(
            peer.plist_path,
            MAX_PLIST_BYTES,
            f"predecessor plist {peer.label}",
            owner_uid=UID,
            exact_mode=0o600,
        )
        if hashlib.sha256(body).hexdigest() != peer.plist_sha256:
            fail(f"predecessor plist changed after planning: {peer.label}")
    require_predecessor_artifacts_unchanged(plan.predecessor)


def require_predecessor_artifacts_unchanged(
    peers: Sequence[PredecessorPeer],
) -> None:
    """Prove rollback artifacts and storage roots were not replaced."""

    binary_cache: dict[Path, str] = {}
    genesis_cache: dict[Path, str] = {}
    for peer in peers:
        try:
            storage_info = peer.storage.lstat()
        except OSError as error:
            raise ResetError(f"predecessor storage disappeared: {peer.label}") from error
        if (
            stat.S_ISLNK(storage_info.st_mode)
            or not stat.S_ISDIR(storage_info.st_mode)
            or (storage_info.st_dev, storage_info.st_ino)
            != (peer.storage_device, peer.storage_inode)
        ):
            fail(f"predecessor storage identity changed: {peer.label}")
        config_sha, _ = hash_regular(
            peer.config,
            MAX_CONFIG_BYTES,
            f"predecessor config {peer.label}",
            owner_uid=UID,
        )
        if config_sha != peer.config_sha256:
            fail(f"predecessor config changed: {peer.label}")
        if peer.binary not in binary_cache:
            binary_cache[peer.binary] = hash_regular(
                peer.binary,
                MAX_BINARY_BYTES,
                "predecessor binary",
            )[0]
        if binary_cache[peer.binary] != peer.binary_sha256:
            fail("predecessor binary changed")
        if peer.genesis not in genesis_cache:
            genesis_cache[peer.genesis] = hash_regular(
                peer.genesis,
                64 * 1024 * 1024,
                "predecessor signed genesis",
            )[0]
        if genesis_cache[peer.genesis] != peer.genesis_sha256:
            fail("predecessor signed genesis changed")


def archive_inventory(path: Path) -> dict[str, str]:
    inventory: dict[str, str] = {}
    for child in sorted(path.iterdir(), key=lambda item: item.name):
        if child.name == "inventory.json":
            continue
        body, _ = read_regular(child, MAX_CONFIG_BYTES, f"archive file {child.name}")
        inventory[child.name] = hashlib.sha256(body).hexdigest()
    return inventory


def stop_all(ops: LaunchctlOps, labels: Sequence[str]) -> None:
    errors: list[str] = []
    for label in labels:
        try:
            if ops.job_loaded(DOMAIN, label):
                ops.bootout(label)
        except ResetError:
            errors.append(label)
    if errors:
        fail(f"failed to stop the complete user cohort: {errors}")
    ops.wait_absent(labels)


def install_candidate_plists(plan: ResetPlan) -> None:
    for peer in plan.candidate:
        atomic_write(peer.plist_path, peer.plist_body, mode=0o600, replace=True)


def bootstrap_all(
    ops: LaunchctlOps,
    peers: Sequence[CandidatePeer | PredecessorPeer],
) -> None:
    started = time.monotonic()
    for peer in peers:
        ops.bootstrap(peer.plist_path)
    if time.monotonic() - started > 10.0:
        fail("four user LaunchAgents exceeded the ten-second bootstrap spread")
    for peer in peers:
        ops.require_loaded_definition(peer.plist_path, peer.plist_body)


def rollback(
    plan: ResetPlan,
    ops: LaunchctlOps,
    health: HealthClient,
    rollback_private_key_file: Path,
    reason: BaseException,
) -> None:
    errors: list[str] = []
    stop_errors: list[str] = []
    for label in LABELS:
        try:
            if ops.job_loaded(DOMAIN, label):
                ops.bootout(label)
        except ResetError as error:
            stop_errors.append(f"{label}:{error}")
    try:
        ops.wait_absent(LABELS)
    except ResetError as error:
        errors.append(f"stop:{error}")
    if not errors and stop_errors:
        # A service may have exited between print and bootout.  Absence of the
        # complete cohort is the authoritative safety condition for restore.
        stop_errors.clear()
    for peer in plan.predecessor:
        try:
            if peer.plist_path.exists() and not peer.plist_path.is_symlink():
                current, _ = read_regular(
                    peer.plist_path,
                    MAX_PLIST_BYTES,
                    f"rollback target {peer.label}",
                    owner_uid=UID,
                    exact_mode=0o600,
                )
                current_sha = hashlib.sha256(current).hexdigest()
                allowed = {peer.plist_sha256, plan.candidate[peer.number - 1].plist_sha256}
                if current_sha not in allowed:
                    fail(f"rollback target changed outside this transaction: {peer.label}")
            atomic_write(
                peer.plist_path,
                peer.plist_body,
                mode=0o600,
                replace=True,
            )
        except ResetError as error:
            errors.append(f"restore:{peer.label}:{error}")
    if not errors:
        try:
            require_predecessor_artifacts_unchanged(plan.predecessor)
        except ResetError as error:
            errors.append(f"artifacts:{error}")
    if not errors:
        try:
            bootstrap_all(ops, plan.predecessor)
            restored_fleet = health.wait_fleet(
                [peer.torii_port for peer in plan.predecessor],
                plan.predecessor[0].network_id,
                rollback_private_key_file,
                plan.activation.limits,
            )
        except ResetError as error:
            errors.append(f"restart:{error}")
            restored_fleet = None
    else:
        restored_fleet = None
    receipt = {
        "schema": "iroha.taira.user-launchagent-rollback.v1",
        "recorded_at": utc_now(),
        "activation_manifest_sha256": plan.activation.sha256,
        "reason": str(reason)[:1024],
        "restored": not errors,
        "errors": [*stop_errors, *errors],
        "fleet": fleet_evidence(restored_fleet) if restored_fleet is not None else None,
    }
    write_json(plan.archive_dir / "rollback.json", receipt)
    write_json(plan.archive_dir / "inventory.json", archive_inventory(plan.archive_dir))
    if stop_errors or errors:
        fail("automatic predecessor rollback failed; retained owner-private evidence archive")


def acquire_lock(path: Path, root: Path) -> None:
    relative_descendant(path, root, "deployment lock", minimum_parts=1)
    try:
        path.mkdir(mode=0o700)
    except FileExistsError as error:
        raise ResetError(f"another user LaunchAgent reset holds {path}") from error


def release_lock(path: Path) -> None:
    try:
        path.rmdir()
    except OSError:
        pass


def apply_reset(
    plan: ResetPlan,
    *,
    confirmation: str,
    operator_private_key_file: Path,
    rollback_operator_private_key_file: Path,
    layout: Layout = PRODUCTION_LAYOUT,
    ops: LaunchctlOps | None = None,
    health: HealthClient | None = None,
) -> dict[str, object]:
    if confirmation != plan.activation.confirmation:
        fail("destructive confirmation does not bind the exact activation manifest")
    ops = ops or LaunchctlOps()
    health = health or HealthClient()
    acquire_lock(layout.lock_path, layout.taira_root)
    mutated = False
    try:
        ops.require_initial_cohort()
        for peer in plan.predecessor:
            ops.require_loaded_definition(peer.plist_path, peer.plist_body)
        require_plan_unchanged(plan)
        old_fleet = health.wait_fleet(
            [peer.torii_port for peer in plan.predecessor],
            plan.predecessor[0].network_id,
            rollback_operator_private_key_file,
            plan.activation.limits,
        )
        prepare_archive(plan, old_fleet, layout)
        require_plan_unchanged(plan)
        try:
            mutated = True
            stop_all(ops, LABELS)
            require_predecessor_artifacts_unchanged(plan.predecessor)
            install_candidate_plists(plan)
            bootstrap_all(ops, plan.candidate)
            candidate_fleet = health.wait_fleet(
                [peer.torii_port for peer in plan.candidate],
                plan.network_id,
                operator_private_key_file,
                plan.activation.limits,
            )
            require_predecessor_artifacts_unchanged(plan.predecessor)
            for peer in plan.candidate:
                current, _ = read_regular(
                    peer.plist_path,
                    MAX_PLIST_BYTES,
                    f"installed candidate plist {peer.label}",
                    owner_uid=UID,
                    exact_mode=0o600,
                )
                if hashlib.sha256(current).hexdigest() != peer.plist_sha256:
                    fail(f"installed candidate plist changed: {peer.label}")
            result = {
                "schema": "iroha.taira.user-launchagent-reset-applied.v1",
                "applied_at": utc_now(),
                "activation_manifest_sha256": plan.activation.sha256,
                "network_id": plan.network_id,
                "launchctl_domain": DOMAIN,
                "labels": list(LABELS),
                "candidate_fleet": fleet_evidence(candidate_fleet),
                "candidate_plists": {
                    peer.label: peer.plist_sha256 for peer in plan.candidate
                },
                "predecessor_retained": True,
                "predecessor_storage_untouched": True,
                "archive": str(plan.archive_dir),
            }
            write_json(plan.archive_dir / "applied.json", result)
            write_json(plan.archive_dir / "inventory.json", archive_inventory(plan.archive_dir))
            return result
        except BaseException as error:
            if mutated:
                rollback(
                    plan,
                    ops,
                    health,
                    rollback_operator_private_key_file,
                    error,
                )
            raise
    finally:
        release_lock(layout.lock_path)


def plan_projection(plan: ResetPlan) -> dict[str, object]:
    return {
        "schema": "iroha.taira.user-launchagent-reset-plan.v1",
        "mode": "dry-run",
        "activation_manifest": str(plan.activation.path),
        "activation_manifest_sha256": plan.activation.sha256,
        "confirmation_required": plan.activation.confirmation,
        "launchctl_domain": DOMAIN,
        "labels": list(LABELS),
        "network_id": plan.network_id,
        "bundle": str(plan.activation.bundle),
        "binary": str(plan.activation.binary),
        "binary_sha256": plan.activation.binary_sha256,
        "archive": str(plan.archive_dir),
        "candidate_logs": str(plan.log_dir),
        "candidate_plists": {
            peer.label: {
                "path": str(peer.plist_path),
                "sha256": peer.plist_sha256,
                "config": str(peer.config),
                "config_sha256": peer.config_sha256,
                "storage": str(peer.storage),
                "torii_port": peer.torii_port,
            }
            for peer in plan.candidate
        },
        "predecessor": [peer_evidence(peer) for peer in plan.predecessor],
        "mutated": False,
    }


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(description=__doc__)
    value.add_argument("--manifest", required=True, type=Path)
    value.add_argument("--apply", action="store_true")
    value.add_argument("--confirm-reset")
    value.add_argument("--operator-private-key-file", type=Path)
    value.add_argument("--rollback-operator-private-key-file", type=Path)
    return value


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    require_runtime_identity()
    activation = load_activation(args.manifest)
    ops = LaunchctlOps()
    plan = build_plan(activation, launchctl=ops)
    if not args.apply:
        if any(
            value is not None
            for value in (
                args.confirm_reset,
                args.operator_private_key_file,
                args.rollback_operator_private_key_file,
            )
        ):
            fail("dry-run does not accept confirmation or runtime signing inputs")
        HealthClient().unsigned_liveness([peer.torii_port for peer in plan.predecessor])
        print(json.dumps(plan_projection(plan), sort_keys=True, indent=2))
        return 0
    if args.confirm_reset is None or args.operator_private_key_file is None:
        fail("--apply requires --confirm-reset and --operator-private-key-file")
    rollback_key = args.rollback_operator_private_key_file or args.operator_private_key_file
    result = apply_reset(
        plan,
        confirmation=args.confirm_reset,
        operator_private_key_file=args.operator_private_key_file,
        rollback_operator_private_key_file=rollback_key,
        ops=ops,
    )
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ResetError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(70)
