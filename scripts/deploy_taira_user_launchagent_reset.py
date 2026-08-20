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

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from scripts import compose_taira_nevo_reset_genesis as nevo_composer
    from scripts import deploy_taira_v21_reset as reset_bundle
    from scripts.release_artifact_contract import (
        stable_hash_path,
        verify_private_python_source_closure,
    )
except ModuleNotFoundError:  # Direct execution sets sys.path to scripts/.
    import compose_taira_nevo_reset_genesis as nevo_composer
    import deploy_taira_v21_reset as reset_bundle
    from release_artifact_contract import (
        stable_hash_path,
        verify_private_python_source_closure,
    )


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
LOCAL_TESTNET_SOURCE_CLOSURE_SCHEMA = (
    "iroha.taira.local-testnet-reset-source-closure.v1"
)
LOCAL_TESTNET_PYTHON = Path("/opt/homebrew/bin/python3")
LOCAL_TESTNET_SOURCE_CLOSURE_FILES = (
    "configs/soranexus/taira/config.toml",
    "configs/soranexus/taira/genesis.json",
    "configs/soranexus/taira/privacy_bootstrap_plan.json",
    "scripts/build_privacy_v1_boi_handoff.py",
    "scripts/check_native_sdk_abi22_artifact.py",
    "scripts/compose_taira_nevo_reset_genesis.py",
    "scripts/compute_workspace_source_manifest.py",
    "scripts/deploy_taira_user_launchagent_reset.py",
    "scripts/deploy_taira_v21_reset.py",
    "scripts/deploy_taira_v21_reset_authority.py",
    "scripts/deploy_taira_v21_reset_health.py",
    "scripts/extract_authenticated_taira_privacy_release.py",
    "scripts/inspect_taira_local_reset_source_closure.py",
    "scripts/inspect_taira_local_reviewed_inputs.py",
    "scripts/iso_operator_auth.py",
    "scripts/operator_http_headers.py",
    "scripts/prepare_taira_empty_reset_bundle.py",
    "scripts/release_artifact_contract.py",
    "scripts/release_manifest_signing.py",
    "scripts/render_taira_validator_bundle.py",
    "scripts/seal_taira_release_controllers.py",
    "scripts/taira_authority_client.py",
    "scripts/taira_constants.py",
    "scripts/taira_privacy_protocol_receipt.py",
    "scripts/taira_privacy_rollout_contract.py",
    "scripts/taira_release_authority.py",
    "scripts/taira_rollout_admission.py",
)
LOCAL_TESTNET_REVIEWED_INPUT_FILES = (
    "config.toml",
    "genesis.json",
    "nevo-reset.review.json",
    "privacy_bootstrap_plan.json",
)

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
        "genesis_native_verifier",
        "genesis_native_verifier_sha256",
        "operator_status_client",
        "operator_status_client_sha256",
        "genesis_external_signer_sha256",
        "genesis_public_key",
        "genesis_expected_hash",
        "genesis_artifact_linkage_sha256",
        "nevo_review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "pre_sign_rendered_genesis_sha256",
        "native_verifier_peer_config_set_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "source_commit",
        "dpn_validator_release_commit",
        "limits",
    }
)
PRODUCTION_PRIVACY_ACTIVATION_KEYS = frozenset(
    {"privacy_native_verifier_sha256"}
)
LOCAL_TESTNET_PRIVACY_ACTIVATION_KEYS = frozenset(
    {
        "local_reviewed_inputs_identity_sha256",
        "local_testnet_source_closure_sha256",
        "local_testnet_python_sha256",
    }
)
PRIVACY_INPUT_ACTIVATION_KEYS = (
    PRODUCTION_PRIVACY_ACTIVATION_KEYS | LOCAL_TESTNET_PRIVACY_ACTIVATION_KEYS
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
MAX_GENESIS_BYTES = 64 * 1024 * 1024
MAX_VERIFIER_OUTPUT_BYTES = 2 * 1024 * 1024
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
    genesis_native_verifier: Path
    genesis_native_verifier_sha256: str
    operator_status_client: Path
    operator_status_client_sha256: str
    genesis_external_signer_sha256: str
    genesis_public_key: str
    genesis_expected_hash: str
    genesis_artifact_linkage_sha256: str
    nevo_review_sha256: str
    reviewed_unsigned_genesis_sha256: str
    pre_sign_rendered_genesis_sha256: str
    native_verifier_peer_config_set_sha256: str
    bound_genesis_manifest_sha256: str
    signed_genesis_sha256: str
    privacy_native_verifier_sha256: str | None
    local_reviewed_inputs_identity_sha256: str | None
    local_testnet_source_closure_sha256: str | None
    local_testnet_python_sha256: str | None
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
    genesis_native_verifier_identity: tuple[int, int, int, int, int, int]
    operator_status_client_identity: tuple[int, int, int, int, int, int]
    validator_artifact_inventory: Mapping[str, Mapping[str, object]]


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


def _require_genesis_public_key(value: object) -> str:
    if (
        not isinstance(value, str)
        or reset_bundle.GENESIS_PUBLIC_KEY_RE.fullmatch(value) is None
    ):
        fail("genesis_public_key must be one canonical Ed25519 public multihash")
    return value


def _require_genesis_expected_hash(value: object) -> str:
    digest = require_sha256(value, "genesis_expected_hash")
    if int(digest, 16) == 0:
        fail("genesis_expected_hash must be nonzero")
    return digest


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


def local_testnet_source_closure() -> tuple[dict[str, object], str]:
    """Recompute the exact user-owned preparation/deployment source closure."""

    if tuple(sorted(set(LOCAL_TESTNET_SOURCE_CLOSURE_FILES))) != (
        LOCAL_TESTNET_SOURCE_CLOSURE_FILES
    ):
        raise AssertionError("local-testnet source closure inventory is not exact")
    repository = Path(__file__).resolve().parent.parent
    rows: list[dict[str, object]] = []
    for relative in LOCAL_TESTNET_SOURCE_CLOSURE_FILES:
        digest, info = hash_regular(
            repository / relative,
            MAX_GENESIS_BYTES,
            f"local-testnet source closure {relative}",
        )
        rows.append(
            {"path": relative, "sha256": digest, "size": info.st_size}
        )
    manifest: dict[str, object] = {
        "schema": LOCAL_TESTNET_SOURCE_CLOSURE_SCHEMA,
        "files": rows,
    }
    digest = hashlib.sha256(_artifact_canonical_json(manifest)).hexdigest()
    return manifest, digest


def local_testnet_python_sha256(expected_sha256: object) -> str:
    """Bind the local controller to the exact explicit Python 3.11+ binary."""

    expected = require_sha256(expected_sha256, "local testnet Python SHA-256")
    if sys.version_info < (3, 11):
        fail("local testnet reset requires Python 3.11 or newer")
    try:
        invoked = LOCAL_TESTNET_PYTHON.resolve(strict=True)
        running = Path(sys.executable).resolve(strict=True)
        info = invoked.lstat()
    except OSError as error:
        raise ResetError(f"local testnet Python cannot be resolved: {error}") from error
    if (
        invoked != running
        or not stat.S_ISREG(info.st_mode)
        or info.st_uid != UID
        or info.st_nlink != 1
        or info.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
    ):
        fail("local testnet Python runtime has unsafe custody or path")
    observed = stable_hash_path(invoked, max_size=MAX_BINARY_BYTES).sha256
    if observed != expected:
        fail("local testnet Python differs from the activation")
    return observed


def require_local_testnet_source_runtime(
    expected_sha256: str,
    expected_python_sha256: str,
) -> None:
    """Bind local reset execution to one isolated exact owner-private tree."""

    expected = require_sha256(
        expected_sha256,
        "local testnet source closure SHA-256",
    )
    manifest, observed = local_testnet_source_closure()
    if observed != expected:
        fail("local testnet source closure differs from the activation")
    local_testnet_python_sha256(expected_python_sha256)
    verify_private_python_source_closure(
        SCRIPT_DIR.parent,
        manifest,
        expected,
        owner_uid=UID,
        entrypoint="scripts/deploy_taira_user_launchagent_reset.py",
        require_isolated_runtime=True,
    )


def parse_toml(path: Path, owner_uid: int, label: str) -> tuple[dict[str, Any], str]:
    body, _ = read_regular(
        path,
        MAX_CONFIG_BYTES,
        label,
        owner_uid=owner_uid,
        exact_mode=0o600,
    )
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
    privacy_input_keys = set(payload) & PRIVACY_INPUT_ACTIVATION_KEYS
    if privacy_input_keys not in {
        PRODUCTION_PRIVACY_ACTIVATION_KEYS,
        LOCAL_TESTNET_PRIVACY_ACTIVATION_KEYS,
    } or set(payload) != MANIFEST_KEYS | privacy_input_keys:
        fail("activation manifest schema is not exact")
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
    verifier_value = payload.get("genesis_native_verifier")
    operator_client_value = payload.get("operator_status_client")
    if (
        not isinstance(bundle_value, str)
        or not isinstance(binary_value, str)
        or not isinstance(verifier_value, str)
        or not isinstance(operator_client_value, str)
    ):
        fail("activation candidate paths must be strings")
    bundle = Path(bundle_value)
    binary = Path(binary_value)
    genesis_native_verifier = Path(verifier_value)
    operator_status_client = Path(operator_client_value)
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
    verifier_parts = relative_descendant(
        genesis_native_verifier,
        layout.releases,
        "candidate native genesis verifier",
        minimum_parts=2,
    )
    operator_client_parts = relative_descendant(
        operator_status_client,
        layout.releases,
        "candidate native operator status client",
        minimum_parts=2,
    )
    if (
        len(bundle_parts) != 1
        or bundle_parts[0] != generation
        or len(binary_parts) != 2
        or binary_parts[-1] != "iroha3d"
        or verifier_parts != (*binary_parts[:-1], "kagami")
        or operator_client_parts
        != (*binary_parts[:-1], "taira_operator_status")
    ):
        fail("candidate reset bundle, binary, or native verifier path shape is not exact")
    require_no_symlink_ancestry(bundle, layout.taira_root, "candidate reset bundle")
    require_no_symlink_ancestry(binary, layout.taira_root, "candidate binary")
    require_no_symlink_ancestry(
        genesis_native_verifier,
        layout.taira_root,
        "candidate native genesis verifier",
    )
    require_no_symlink_ancestry(
        operator_status_client,
        layout.taira_root,
        "candidate native operator status client",
    )
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
        genesis_native_verifier=genesis_native_verifier,
        genesis_native_verifier_sha256=require_sha256(
            payload.get("genesis_native_verifier_sha256"),
            "genesis_native_verifier_sha256",
        ),
        operator_status_client=operator_status_client,
        operator_status_client_sha256=require_sha256(
            payload.get("operator_status_client_sha256"),
            "operator_status_client_sha256",
        ),
        genesis_external_signer_sha256=require_sha256(
            payload.get("genesis_external_signer_sha256"),
            "genesis_external_signer_sha256",
        ),
        genesis_public_key=_require_genesis_public_key(
            payload.get("genesis_public_key")
        ),
        genesis_expected_hash=_require_genesis_expected_hash(
            payload.get("genesis_expected_hash")
        ),
        genesis_artifact_linkage_sha256=require_sha256(
            payload.get("genesis_artifact_linkage_sha256"),
            "genesis_artifact_linkage_sha256",
        ),
        nevo_review_sha256=require_sha256(
            payload.get("nevo_review_sha256"), "nevo_review_sha256"
        ),
        reviewed_unsigned_genesis_sha256=require_sha256(
            payload.get("reviewed_unsigned_genesis_sha256"),
            "reviewed_unsigned_genesis_sha256",
        ),
        pre_sign_rendered_genesis_sha256=require_sha256(
            payload.get("pre_sign_rendered_genesis_sha256"),
            "pre_sign_rendered_genesis_sha256",
        ),
        native_verifier_peer_config_set_sha256=require_sha256(
            payload.get("native_verifier_peer_config_set_sha256"),
            "native_verifier_peer_config_set_sha256",
        ),
        bound_genesis_manifest_sha256=require_sha256(
            payload.get("bound_genesis_manifest_sha256"),
            "bound_genesis_manifest_sha256",
        ),
        signed_genesis_sha256=require_sha256(
            payload.get("signed_genesis_sha256"), "signed_genesis_sha256"
        ),
        privacy_native_verifier_sha256=(
            require_sha256(
                payload.get("privacy_native_verifier_sha256"),
                "privacy_native_verifier_sha256",
            )
            if "privacy_native_verifier_sha256" in privacy_input_keys
            else None
        ),
        local_reviewed_inputs_identity_sha256=(
            require_sha256(
                payload.get("local_reviewed_inputs_identity_sha256"),
                "local_reviewed_inputs_identity_sha256",
            )
            if "local_reviewed_inputs_identity_sha256" in privacy_input_keys
            else None
        ),
        local_testnet_source_closure_sha256=(
            require_sha256(
                payload.get("local_testnet_source_closure_sha256"),
                "local_testnet_source_closure_sha256",
            )
            if "local_testnet_source_closure_sha256" in privacy_input_keys
            else None
        ),
        local_testnet_python_sha256=(
            require_sha256(
                payload.get("local_testnet_python_sha256"),
                "local_testnet_python_sha256",
            )
            if "local_testnet_python_sha256" in privacy_input_keys
            else None
        ),
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
    for directory, directory_label in (
        (workdir.parent, f"predecessor release root {label}"),
        (workdir, f"predecessor workdir {label}"),
        (workdir / "storage", f"predecessor storage {label}"),
    ):
        directory_info = directory.lstat()
        if (
            stat.S_ISLNK(directory_info.st_mode)
            or not stat.S_ISDIR(directory_info.st_mode)
            or directory_info.st_uid != UID
            or stat.S_IMODE(directory_info.st_mode) != 0o700
        ):
            fail(f"{directory_label} custody is not exact")
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
    binary_sha, binary_info = hash_regular(
        binary,
        MAX_BINARY_BYTES,
        f"predecessor binary {label}",
        owner_uid=UID,
    )
    if stat.S_IMODE(binary_info.st_mode) & 0o111 == 0:
        fail(f"predecessor binary is not executable: {label}")
    genesis_sha, _ = hash_regular(
        genesis,
        64 * 1024 * 1024,
        f"predecessor genesis {label}",
        owner_uid=UID,
        exact_mode=0o600,
    )
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
        bundle / "genesis.signed.nrt",
        bundle / "base-config.toml",
        bundle / "reset-manifest.json",
        bundle / "validator-roster.toml",
        bundle / "rendered/genesis.json",
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
            lines = text.splitlines()
            try:
                start = lines.index("\targuments = {") + 1
                end = lines.index("\t}", start)
            except ValueError as error:
                raise ResetError(
                    f"loaded LaunchAgent omitted its argument block: {label}"
                ) from error
            loaded_arguments = [
                line.removeprefix("\t\t")
                for line in lines[start:end]
                if line.startswith("\t\t")
            ]
            if loaded_arguments != arguments:
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


def _artifact_canonical_json(value: object) -> bytes:
    return (
        json.dumps(
            value,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def _ordered_sha256_set(digests: Sequence[str]) -> str:
    return hashlib.sha256(("\n".join(digests) + "\n").encode("ascii")).hexdigest()


def _require_exact_json_object(
    path: Path,
    maximum: int,
    label: str,
) -> tuple[dict[str, object], bytes]:
    raw, _ = read_regular(
        path,
        maximum,
        label,
        owner_uid=UID,
        exact_mode=0o600,
    )
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ResetError(f"{label} is not strict UTF-8 JSON") from error
    if not isinstance(value, dict):
        fail(f"{label} must be one JSON object")
    return value, raw


def _validate_validator_artifact_inventory(
    bundle: Path,
    raw_inventory: object,
) -> dict[str, dict[str, object]]:
    if not isinstance(raw_inventory, dict) or set(raw_inventory) != set(SLUGS):
        fail("reset manifest validator artifact inventory is not exact")
    inventory: dict[str, dict[str, object]] = {}
    allowed_prefixes = ("codec/", "configs/", "manifests/", "runtime/")
    for slug in SLUGS:
        raw_rows = raw_inventory.get(slug)
        if not isinstance(raw_rows, dict) or set(raw_rows) != {"directories", "files"}:
            fail(f"reset manifest artifact inventory is malformed: {slug}")
        raw_directories = raw_rows.get("directories")
        raw_files = raw_rows.get("files")
        if (
            not isinstance(raw_directories, list)
            or not all(isinstance(path, str) for path in raw_directories)
            or raw_directories != sorted(set(raw_directories))
            or not isinstance(raw_files, dict)
            or "config.toml" not in raw_files
        ):
            fail(f"reset manifest artifact inventory rows are malformed: {slug}")
        directories: list[str] = []
        for relative in raw_directories:
            if (
                not isinstance(relative, str)
                or Path(relative).is_absolute()
                or ".." in Path(relative).parts
                or "storage" in Path(relative).parts
                or not Path(relative).parts
                or Path(relative).parts[0]
                not in {"codec", "configs", "manifests", "runtime"}
            ):
                fail(f"reset manifest artifact directory is not admitted: {slug}/{relative}")
            directories.append(relative)
        rows: dict[str, str] = {}
        for relative, digest in raw_files.items():
            if (
                not isinstance(relative, str)
                or Path(relative).is_absolute()
                or ".." in Path(relative).parts
                or "storage" in Path(relative).parts
                or (
                    relative != "config.toml"
                    and not relative.startswith(allowed_prefixes)
                )
            ):
                fail(f"reset manifest artifact path is not admitted: {slug}/{relative}")
            rows[relative] = require_sha256(
                digest,
                f"validator artifact {slug}/{relative}",
            )
        inventory[slug] = {"directories": directories, "files": rows}
    _rehash_validator_artifact_inventory(bundle, inventory)
    return inventory


def _candidate_artifact_paths(root: Path) -> tuple[set[str], set[str]]:
    paths: set[str] = set()
    directory_paths: set[str] = set()
    for current, directories, files in os.walk(root, followlinks=False):
        current_path = Path(current)
        relative_directory = current_path.relative_to(root)
        retained: list[str] = []
        for name in directories:
            path = current_path / name
            relative = (relative_directory / name).as_posix()
            info = path.lstat()
            if current_path == root and name == "storage":
                if (
                    stat.S_ISLNK(info.st_mode)
                    or not stat.S_ISDIR(info.st_mode)
                    or any(path.iterdir())
                ):
                    fail("candidate peer-root storage is not one empty real directory")
                continue
            if name == "storage":
                fail(f"candidate artifact tree contains nested storage: {relative}")
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISDIR(info.st_mode)
                or info.st_uid != UID
                or stat.S_IMODE(info.st_mode) != 0o700
            ):
                fail(f"candidate artifact directory is unsafe: {relative}")
            directory_paths.add(relative)
            retained.append(name)
        directories[:] = retained
        for name in files:
            path = current_path / name
            relative = (relative_directory / name).as_posix()
            if relative.startswith("storage/"):
                fail("candidate storage contains a bootstrap artifact")
            paths.add(relative)
            if path.is_symlink():
                fail(f"candidate artifact is a symlink: {relative}")
    return paths, directory_paths


def _rehash_validator_artifact_inventory(
    bundle: Path,
    inventory: Mapping[str, Mapping[str, object]],
) -> None:
    for slug in SLUGS:
        root = bundle / "rendered" / slug
        expected = inventory.get(slug)
        if expected is None:
            fail(f"candidate bootstrap artifact inventory changed: {slug}")
        expected_files = expected.get("files")
        expected_directories = expected.get("directories")
        actual_files, actual_directories = _candidate_artifact_paths(root)
        if (
            not isinstance(expected_files, Mapping)
            or not isinstance(expected_directories, list)
            or actual_files != set(expected_files)
            or actual_directories != set(expected_directories)
        ):
            fail(f"candidate bootstrap artifact inventory changed: {slug}")
        for relative, digest in expected_files.items():
            actual, _ = hash_regular(
                root / relative,
                MAX_GENESIS_BYTES,
                f"candidate bootstrap artifact {slug}/{relative}",
                owner_uid=UID,
                exact_mode=0o600,
            )
            if actual != digest:
                fail(f"candidate bootstrap artifact changed: {slug}/{relative}")


def _require_local_testnet_source_closure(
    activation: Activation,
    manifest: Mapping[str, object],
) -> dict[str, object]:
    closure = manifest.get("local_testnet_source_closure")
    if not isinstance(closure, dict) or set(closure) != {"schema", "files", "sha256"}:
        fail("local-testnet reset source closure schema is not exact")
    rows = closure.get("files")
    if (
        closure.get("schema") != LOCAL_TESTNET_SOURCE_CLOSURE_SCHEMA
        or not isinstance(rows, list)
        or len(rows) != len(LOCAL_TESTNET_SOURCE_CLOSURE_FILES)
    ):
        fail("local-testnet reset source closure inventory is not exact")
    for expected_path, row in zip(LOCAL_TESTNET_SOURCE_CLOSURE_FILES, rows):
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            fail("local-testnet reset source closure row is not exact")
        size = row.get("size")
        if (
            row.get("path") != expected_path
            or require_sha256(row.get("sha256"), f"source closure {expected_path}")
            != row.get("sha256")
            or isinstance(size, bool)
            or not isinstance(size, int)
            or size <= 0
        ):
            fail("local-testnet reset source closure row is invalid")
    projected = {"schema": closure["schema"], "files": rows}
    digest = hashlib.sha256(_artifact_canonical_json(projected)).hexdigest()
    if (
        digest != closure.get("sha256")
        or digest != activation.local_testnet_source_closure_sha256
    ):
        fail("local-testnet reset source closure differs from activation")
    current, current_digest = local_testnet_source_closure()
    if current != projected or current_digest != digest:
        fail("local-testnet reset source closure differs from the executing code")
    python_binding = manifest.get("local_testnet_python")
    if (
        not isinstance(python_binding, Mapping)
        or set(python_binding) != {"path", "sha256"}
        or python_binding.get("path") != str(LOCAL_TESTNET_PYTHON)
        or python_binding.get("sha256") != activation.local_testnet_python_sha256
    ):
        fail("local-testnet Python binding differs from activation")
    local_testnet_python_sha256(python_binding.get("sha256"))
    plan_payload, _ = read_regular(
        Path(__file__).resolve().parent.parent
        / "configs/soranexus/taira/privacy_bootstrap_plan.json",
        MAX_CONFIG_BYTES,
        "local-testnet privacy staging plan",
    )
    try:
        plan = json.loads(plan_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ResetError("local-testnet privacy staging plan is invalid JSON") from error
    if (
        not isinstance(plan, dict)
        or set(plan)
        != {
            "bootle_lantern_issuer",
            "chain_discriminant",
            "chain_id",
            "genesis_authority",
            "governance_permission",
            "governance_rollout",
            "network_id",
            "privacy_catalog",
            "schema",
            "schema_version",
        }
        or plan.get("schema") != "iroha.taira.privacy_bootstrap_plan.v1"
        or plan.get("schema_version") != 1
        or plan.get("network_id", object()) is not None
    ):
        fail("local-testnet privacy plan is not the exact null-NetworkId staging schema")
    return projected


def _require_issuer_disabled(config: Mapping[str, object], label: str) -> None:
    torii = config.get("torii")
    issuer = (
        torii.get("privacy_bootle_lantern_issuer")
        if isinstance(torii, Mapping)
        else None
    )
    provider_binding_fields = {
        "issuer_id_hex",
        "policy_id_hex",
        "runtime_provider_registry_handle",
        "runtime_provider_registry_revision",
        "runtime_provider_registry_policy_digest_hex",
    }
    if (
        not isinstance(issuer, Mapping)
        or issuer.get("enabled") is not False
        or set(issuer) & provider_binding_fields
    ):
        fail(f"{label} does not keep Bootle/Lantern issuance exactly disabled")


def _require_local_testnet_reviewed_release(
    activation: Activation,
    manifest: Mapping[str, object],
    privacy_release: Mapping[str, object],
    review: Mapping[str, object],
) -> None:
    expected_keys = {
        "schema",
        "reviewed_inputs",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "validator_config_sha256",
        "nevo_reset_review",
        "authority_claim",
        "issuer_state",
        "post_genesis_issuer_enablement_required",
        "reviewed_inputs_identity_sha256",
        "source",
    }
    if set(privacy_release) != expected_keys:
        fail("local-testnet reviewed release schema is not exact")
    if (
        privacy_release.get("authority_claim")
        != "none-user-authorized-same-host-testnet"
        or privacy_release.get("issuer_state") != "disabled-no-broker"
        or privacy_release.get("post_genesis_issuer_enablement_required") is not True
    ):
        fail("local-testnet reset makes an invalid authority or issuer claim")
    reviewed_inputs = privacy_release.get("reviewed_inputs")
    if not isinstance(reviewed_inputs, Mapping) or set(reviewed_inputs) != set(
        LOCAL_TESTNET_REVIEWED_INPUT_FILES
    ):
        fail("local-testnet reset reviewed input inventory is not exactly four files")
    for name in LOCAL_TESTNET_REVIEWED_INPUT_FILES:
        row = reviewed_inputs.get(name)
        if not isinstance(row, Mapping) or set(row) != {"sha256", "size"}:
            fail(f"local-testnet reviewed input row is not exact: {name}")
        size = row.get("size")
        require_sha256(row.get("sha256"), f"local-testnet reviewed input {name}")
        if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
            fail(f"local-testnet reviewed input size is invalid: {name}")

    closure = manifest.get("local_testnet_source_closure")
    if not isinstance(closure, Mapping) or not isinstance(closure.get("files"), list):
        fail("local-testnet reset source closure is unavailable")
    closure_rows = {
        row["path"]: row
        for row in closure["files"]
        if isinstance(row, Mapping) and isinstance(row.get("path"), str)
    }
    artifact_paths = {
        "config.toml": activation.bundle / "base-config.toml",
        "genesis.json": activation.bundle / "genesis.reviewed-unsigned.json",
        "nevo-reset.review.json": activation.bundle / "nevo-reset.review.json",
    }
    for name, path in artifact_paths.items():
        digest, info = hash_regular(
            path,
            MAX_GENESIS_BYTES,
            f"local-testnet reviewed input {name}",
            owner_uid=UID,
            exact_mode=0o600,
        )
        row = reviewed_inputs[name]
        if digest != row["sha256"] or info.st_size != row["size"]:
            fail(f"local-testnet reviewed input changed: {name}")
    for name, closure_path in {
        "config.toml": "configs/soranexus/taira/config.toml",
        "privacy_bootstrap_plan.json": (
            "configs/soranexus/taira/privacy_bootstrap_plan.json"
        ),
    }.items():
        source_row = closure_rows.get(closure_path)
        reviewed_row = reviewed_inputs[name]
        if (
            not isinstance(source_row, Mapping)
            or source_row.get("sha256") != reviewed_row["sha256"]
            or source_row.get("size") != reviewed_row["size"]
        ):
            fail(f"local-testnet reviewed input is not source-pinned: {name}")

    source = privacy_release.get("source")
    expected_source = {
        "commit": activation.source_commit,
        "dpn_validator_release_commit": activation.dpn_validator_release_commit,
        "cargo_lock_sha256": manifest.get("cargo_lock_sha256"),
        "workspace_source_manifest_sha256": manifest.get(
            "workspace_source_manifest_sha256"
        ),
    }
    if source != expected_source:
        fail("local-testnet reviewed input source identity changed")
    reviewed_identity_manifest = {
        "schema": "iroha.taira.local_testnet_reviewed_inputs.v1",
        "authority_claim": "none-user-authorized-same-host-testnet",
        "source": source,
        "privacy_inputs": reviewed_inputs,
    }
    reviewed_identity = hashlib.sha256(
        _artifact_canonical_json(reviewed_identity_manifest)
    ).hexdigest()
    if (
        reviewed_identity != privacy_release.get("reviewed_inputs_identity_sha256")
        or reviewed_identity != activation.local_reviewed_inputs_identity_sha256
        or reviewed_identity != manifest.get("local_reviewed_inputs_identity_sha256")
    ):
        fail("local-testnet reviewed input identity differs from activation")
    if (
        privacy_release.get("bound_genesis_manifest_sha256")
        != activation.bound_genesis_manifest_sha256
        or privacy_release.get("signed_genesis_sha256")
        != activation.signed_genesis_sha256
        or privacy_release.get("validator_config_sha256") != manifest.get("configs")
    ):
        fail("local-testnet reviewed release artifact bindings changed")

    review_record = privacy_release.get("nevo_reset_review")
    expected_review_record = {
        "schema": review.get("schema"),
        "sha256": reviewed_inputs["nevo-reset.review.json"]["sha256"],
        "public_inputs_sha256": review.get("public_inputs_sha256"),
        "unsigned_genesis_sha256": review.get("unsigned_genesis_sha256"),
        "public_identities": review.get("public_identities"),
        "credential_hash_bindings": review.get("credential_hash_bindings"),
    }
    if review_record != expected_review_record:
        fail("local-testnet NEVO review projection changed")

    base_config, _ = parse_toml(
        activation.bundle / "base-config.toml",
        UID,
        "local-testnet base config",
    )
    _require_issuer_disabled(base_config, "local-testnet base config")
    for slug in SLUGS:
        peer_config, _ = parse_toml(
            activation.bundle / "rendered" / slug / "config.toml",
            UID,
            f"local-testnet peer config {slug}",
        )
        _require_issuer_disabled(peer_config, f"local-testnet peer config {slug}")


def _require_nevo_genesis_integrity(
    activation: Activation,
    manifest: Mapping[str, object],
) -> tuple[dict[str, object], dict[str, dict[str, object]]]:
    linkage = manifest.get("genesis_artifact_linkage")
    if not isinstance(linkage, dict):
        fail("reset manifest genesis artifact linkage is absent")
    linkage_keys = {
        "schema",
        "review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "validator_roster_sha256",
        "pre_sign_rendered_genesis_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "genesis_expected_hash",
        "genesis_public_key",
        "external_signer_sha256",
        "native_genesis_verifier_sha256",
        "operator_status_client_sha256",
        "native_genesis_verifier_receipt_sha256",
        "native_verifier_peer_config_set_sha256",
    }
    privacy_release = manifest.get("privacy_bootstrap_release")
    if not isinstance(privacy_release, Mapping):
        fail("reset manifest privacy bootstrap release is absent")
    if privacy_release.get("schema") == "iroha.taira.signed_privacy_reset.v1":
        privacy_binding_key = "privacy_native_verifier_sha256"
        privacy_binding_value = activation.privacy_native_verifier_sha256
        if (
            activation.local_reviewed_inputs_identity_sha256 is not None
            or activation.local_testnet_source_closure_sha256 is not None
            or activation.local_testnet_python_sha256 is not None
            or "local_testnet_source_closure" in manifest
            or "local_testnet_python" in manifest
        ):
            fail("production reset activation claims local-testnet reviewed inputs")
        release_controller = manifest.get("release_controller")
        if (
            not isinstance(release_controller, Mapping)
            or set(release_controller) != {"digest", "manifest_sha256", "platform"}
            or release_controller.get("platform") != "macos"
        ):
            fail("production reset lacks its exact release controller binding")
        require_sha256(release_controller.get("digest"), "release controller digest")
        require_sha256(
            release_controller.get("manifest_sha256"),
            "release controller manifest SHA-256",
        )
    elif (
        privacy_release.get("schema")
        == "iroha.taira.local_testnet_reviewed_reset.v1"
    ):
        privacy_binding_key = "local_reviewed_inputs_identity_sha256"
        privacy_binding_value = activation.local_reviewed_inputs_identity_sha256
        if (
            activation.privacy_native_verifier_sha256 is not None
            or activation.local_testnet_source_closure_sha256 is None
            or activation.local_testnet_python_sha256 is None
            or "release_controller" in manifest
        ):
            fail("local-testnet reset activation claims production verifier authority")
        _require_local_testnet_source_closure(activation, manifest)
    else:
        fail("reset manifest privacy bootstrap release schema is unsupported")
    if privacy_binding_value is None:
        fail("activation lacks its exact privacy input-mode binding")
    linkage_keys.add(privacy_binding_key)
    if set(linkage) != linkage_keys:
        fail("reset manifest genesis artifact linkage schema is not exact")
    if linkage.get("schema") != "iroha.taira.nevo-genesis-artifact-linkage.v1":
        fail("reset manifest genesis artifact linkage version is unsupported")
    linkage_sha256 = hashlib.sha256(_artifact_canonical_json(linkage)).hexdigest()
    if (
        linkage_sha256 != manifest.get("genesis_artifact_linkage_sha256")
        or linkage_sha256 != activation.genesis_artifact_linkage_sha256
    ):
        fail("genesis artifact linkage differs from activation")

    file_bindings = (
        (
            "review_sha256",
            activation.nevo_review_sha256,
            activation.bundle / "nevo-reset.review.json",
            MAX_GENESIS_BYTES,
        ),
        (
            "reviewed_unsigned_genesis_sha256",
            activation.reviewed_unsigned_genesis_sha256,
            activation.bundle / "genesis.reviewed-unsigned.json",
            MAX_GENESIS_BYTES,
        ),
        (
            "pre_sign_rendered_genesis_sha256",
            activation.pre_sign_rendered_genesis_sha256,
            activation.bundle / "genesis.pre-sign-rendered.json",
            MAX_GENESIS_BYTES,
        ),
        (
            "bound_genesis_manifest_sha256",
            activation.bound_genesis_manifest_sha256,
            activation.bundle / "genesis.json",
            MAX_GENESIS_BYTES,
        ),
        (
            "signed_genesis_sha256",
            activation.signed_genesis_sha256,
            activation.bundle / "genesis.signed.nrt",
            MAX_GENESIS_BYTES,
        ),
    )
    observed: dict[str, str] = {}
    for field, activated, path, maximum in file_bindings:
        digest, _ = hash_regular(
            path,
            maximum,
            f"genesis linkage {field}",
            owner_uid=UID,
            exact_mode=0o600,
        )
        if linkage.get(field) != digest or activated != digest:
            fail(f"genesis linkage file differs from activation: {field}")
        observed[field] = digest
    validator_roster_sha256, _ = hash_regular(
        activation.bundle / "validator-roster.toml",
        MAX_GENESIS_BYTES,
        "genesis linkage validator roster",
        owner_uid=UID,
        exact_mode=0o600,
    )
    if (
        linkage.get("validator_roster_sha256") != validator_roster_sha256
        or manifest.get("validator_roster_sha256") != validator_roster_sha256
    ):
        fail("genesis linkage validator roster differs from the exact bundle")
    scalar_bindings = {
        "genesis_expected_hash": (
            "genesis_expected_hash",
            activation.genesis_expected_hash,
        ),
        "genesis_public_key": ("genesis_public_key", activation.genesis_public_key),
        "external_signer_sha256": (
            "genesis_external_signer_sha256",
            activation.genesis_external_signer_sha256,
        ),
        "native_genesis_verifier_sha256": (
            "genesis_native_verifier_sha256",
            activation.genesis_native_verifier_sha256,
        ),
        "operator_status_client_sha256": (
            "operator_status_client_sha256",
            activation.operator_status_client_sha256,
        ),
        "native_verifier_peer_config_set_sha256": (
            "native_verifier_peer_config_set_sha256",
            activation.native_verifier_peer_config_set_sha256,
        ),
    }
    for field, (manifest_field, activated) in scalar_bindings.items():
        if linkage.get(field) != activated or manifest.get(manifest_field) != activated:
            fail(f"genesis linkage scalar differs from activation: {field}")
    if (
        require_sha256(linkage.get(privacy_binding_key), privacy_binding_key)
        != privacy_binding_value
        or manifest.get(privacy_binding_key) != privacy_binding_value
    ):
        fail("privacy input-mode binding differs from activation")

    receipt = manifest.get("genesis_native_verifier_receipt")
    receipt_sha256 = manifest.get("genesis_native_verifier_receipt_sha256")
    if not isinstance(receipt, dict) or not isinstance(receipt_sha256, str):
        fail("reset manifest lacks the native genesis verifier receipt")
    require_sha256(receipt_sha256, "native genesis verifier receipt SHA-256")
    if (
        hashlib.sha256(_artifact_canonical_json(receipt)).hexdigest()
        != receipt_sha256
        or linkage.get("native_genesis_verifier_receipt_sha256")
        != receipt_sha256
    ):
        fail("native genesis verifier receipt digest is not linked")
    config_manifest = manifest.get("configs")
    if not isinstance(config_manifest, Mapping) or set(config_manifest) != set(SLUGS):
        fail("reset manifest peer config digest set is not exact")
    peer_config_sha256: list[str] = []
    for slug in SLUGS:
        digest, _ = hash_regular(
            activation.bundle / "rendered" / slug / "config.toml",
            MAX_GENESIS_BYTES,
            f"native verifier peer config {slug}",
            owner_uid=UID,
            exact_mode=0o600,
        )
        if config_manifest.get(slug) != digest:
            fail(f"reset manifest peer config digest changed: {slug}")
        peer_config_sha256.append(digest)
    peer_config_set_sha256 = _ordered_sha256_set(peer_config_sha256)
    if peer_config_set_sha256 != activation.native_verifier_peer_config_set_sha256:
        fail("native verifier peer config set differs from activation")
    expected_receipt = {
        "schema": "iroha.kagami.prepared-genesis-verification.v2",
        "status": "verified",
        "reviewed_manifest_sha256": observed[
            "reviewed_unsigned_genesis_sha256"
        ],
        "validator_roster_sha256": validator_roster_sha256,
        "bound_manifest_sha256": observed["bound_genesis_manifest_sha256"],
        "pre_sign_manifest_sha256": observed[
            "pre_sign_rendered_genesis_sha256"
        ],
        "signed_genesis_sha256": observed["signed_genesis_sha256"],
        "peer_config_sha256": peer_config_sha256,
        "peer_config_set_sha256": peer_config_set_sha256,
        "genesis_public_key": activation.genesis_public_key,
        "expected_hash": activation.genesis_expected_hash,
        "validator_count": PEER_COUNT,
        "reviewed_transform_passed": True,
        "allowed_transform_passed": True,
        "staged_context_passed": True,
        "full_core_validation_passed": True,
    }
    if receipt != expected_receipt:
        fail("native genesis verifier receipt does not describe this exact bundle")

    reviewed, reviewed_raw = _require_exact_json_object(
        activation.bundle / "genesis.reviewed-unsigned.json",
        MAX_GENESIS_BYTES,
        "reviewed unsigned NEVO genesis",
    )
    del reviewed
    review, review_raw = _require_exact_json_object(
        activation.bundle / "nevo-reset.review.json",
        MAX_GENESIS_BYTES,
        "NEVO reset review",
    )
    try:
        verified_review = nevo_composer.verify_reviewed_payloads(
            unsigned_genesis_bytes=reviewed_raw,
            review_bytes=review_raw,
            base_genesis_bytes=nevo_composer._read_bounded_regular(
                nevo_composer.CHECKED_IN_TAIRA_GENESIS,
                nevo_composer.MAX_BASE_GENESIS_BYTES,
                "sealed canonical Taira genesis",
            ),
            base_config_bytes=nevo_composer._read_bounded_regular(
                nevo_composer.REPO_ROOT / "configs/soranexus/taira/config.toml",
                nevo_composer.MAX_BASE_CONFIG_BYTES,
                "sealed canonical Taira config",
            ),
        )
    except (KeyError, nevo_composer.CompositionError) as error:
        raise ResetError(f"NEVO review cannot be deterministically recomposed: {error}") from error
    if review.get("unsigned_genesis_sha256") != observed[
        "reviewed_unsigned_genesis_sha256"
    ]:
        fail("NEVO review does not hash the exact reviewed unsigned genesis")
    if privacy_release.get("schema") == "iroha.taira.local_testnet_reviewed_reset.v1":
        _require_local_testnet_reviewed_release(
            activation,
            manifest,
            privacy_release,
            verified_review,
        )

    inventory = _validate_validator_artifact_inventory(
        activation.bundle,
        manifest.get("validator_artifact_inventory"),
    )
    return expected_receipt, inventory


def _run_native_genesis_verifier(
    activation: Activation,
    expected_receipt: Mapping[str, object],
) -> tuple[int, int, int, int, int, int]:
    verifier_sha, verifier_info = hash_regular(
        activation.genesis_native_verifier,
        MAX_BINARY_BYTES,
        "candidate native genesis verifier",
        owner_uid=UID,
    )
    if (
        verifier_sha != activation.genesis_native_verifier_sha256
        or not stat.S_ISREG(verifier_info.st_mode)
        or stat.S_IMODE(verifier_info.st_mode) & 0o111 == 0
    ):
        fail("candidate native genesis verifier differs from activation")
    command = [
        str(activation.genesis_native_verifier),
        "genesis",
        "validate-prepared",
        "--reviewed-manifest",
        str(activation.bundle / "genesis.reviewed-unsigned.json"),
        "--validator-roster",
        str(activation.bundle / "validator-roster.toml"),
        "--bound-manifest",
        str(activation.bundle / "genesis.json"),
        "--pre-sign-manifest",
        str(activation.bundle / "genesis.pre-sign-rendered.json"),
        "--signed-genesis",
        str(activation.bundle / "genesis.signed.nrt"),
    ]
    for slug in SLUGS:
        command.extend(
            (
                "--peer-config",
                str(activation.bundle / "rendered" / slug / "config.toml"),
            )
        )
    command.extend(
        (
            "--genesis-public-key",
            activation.genesis_public_key,
            "--expected-hash",
            activation.genesis_expected_hash,
        )
    )
    try:
        completed = subprocess.run(
            command,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=120,
            env={
                "HOME": str(activation.bundle),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TMPDIR": str(activation.bundle),
            },
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ResetError("native genesis verifier could not run") from error
    if (
        completed.returncode != 0
        or len(completed.stdout) > MAX_VERIFIER_OUTPUT_BYTES
        or len(completed.stderr) > MAX_VERIFIER_OUTPUT_BYTES
    ):
        fail("native genesis verifier refused the candidate bundle")
    try:
        receipt = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ResetError("native genesis verifier emitted an invalid receipt") from error
    if receipt != expected_receipt:
        fail("native genesis verifier receipt changed at user-controller preflight")
    verifier_sha_after, verifier_info_after = hash_regular(
        activation.genesis_native_verifier,
        MAX_BINARY_BYTES,
        "candidate native genesis verifier",
        owner_uid=UID,
    )
    if (
        verifier_sha_after != verifier_sha
        or metadata_identity(verifier_info_after) != metadata_identity(verifier_info)
    ):
        fail("candidate native genesis verifier changed during preflight")
    return metadata_identity(verifier_info)


def _require_operator_status_client(
    activation: Activation,
) -> tuple[int, int, int, int, int, int]:
    digest, info = hash_regular(
        activation.operator_status_client,
        MAX_BINARY_BYTES,
        "candidate native operator status client",
        owner_uid=UID,
    )
    if (
        digest != activation.operator_status_client_sha256
        or not stat.S_ISREG(info.st_mode)
        or stat.S_IMODE(info.st_mode) & 0o111 == 0
    ):
        fail("candidate native operator status client differs from activation")
    return metadata_identity(info)


def build_plan(
    activation: Activation,
    *,
    layout: Layout = PRODUCTION_LAYOUT,
    launchctl: LaunchctlOps | None = None,
    validate_bundle_fn: Callable[..., Any] = reset_bundle.validate_bundle,
    validate_genesis_integrity_fn: Callable[
        [Activation, Mapping[str, object]],
        tuple[dict[str, object], dict[str, dict[str, object]]],
    ] = _require_nevo_genesis_integrity,
    run_native_genesis_verifier_fn: Callable[
        [Activation, Mapping[str, object]],
        tuple[int, int, int, int, int, int],
    ] = _run_native_genesis_verifier,
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
    native_verifier_receipt, validator_artifact_inventory = (
        validate_genesis_integrity_fn(activation, bundle_plan.manifest)
    )
    genesis_native_verifier_identity = run_native_genesis_verifier_fn(
        activation,
        native_verifier_receipt,
    )
    operator_status_client_identity = _require_operator_status_client(activation)
    network_hash = bundle_plan.manifest.get("genesis_expected_hash")
    if network_hash != activation.genesis_expected_hash:
        fail("reset manifest genesis expected hash differs from activation")
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
        genesis_native_verifier_identity=genesis_native_verifier_identity,
        operator_status_client_identity=operator_status_client_identity,
        validator_artifact_inventory=validator_artifact_inventory,
    )


class _RejectRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        del request, fp, code, msg, headers, newurl
        return None


class HealthClient:
    def __init__(
        self,
        operator_status_client: Path | None = None,
        operator_status_client_sha256: str | None = None,
    ) -> None:
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}),
            _RejectRedirects(),
        )
        self.operator_status_client = operator_status_client
        self.operator_status_client_sha256 = operator_status_client_sha256

    def _protected_status(
        self,
        port: int,
        network_id: str,
        private_key_file: Path,
    ) -> dict[str, object]:
        client = self.operator_status_client
        expected_sha256 = self.operator_status_client_sha256
        if client is None or expected_sha256 is None:
            fail("native operator status client is unavailable")
        before_sha256, before_info = hash_regular(
            client,
            MAX_BINARY_BYTES,
            "native operator status client",
            owner_uid=UID,
        )
        if (
            before_sha256 != expected_sha256
            or not stat.S_ISREG(before_info.st_mode)
            or stat.S_IMODE(before_info.st_mode) & 0o111 == 0
        ):
            fail("native operator status client differs before protected read")
        command = [
            str(client),
            "--torii-url",
            f"http://127.0.0.1:{port}/",
            "--network-id",
            network_id,
            "--operator-private-key-file",
            str(private_key_file),
            "--timeout-ms",
            "2000",
        ]
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            try:
                process = subprocess.Popen(
                    command,
                    stdin=subprocess.DEVNULL,
                    stdout=stdout,
                    stderr=stderr,
                    cwd="/",
                    env={"LANG": "C", "LC_ALL": "C"},
                    close_fds=True,
                    start_new_session=True,
                )
            except OSError as error:
                raise ResetError("native operator status client could not start") from error
            deadline = time.monotonic() + 4.0
            while process.poll() is None:
                if (
                    time.monotonic() >= deadline
                    or os.fstat(stdout.fileno()).st_size > MAX_HTTP_BYTES
                    or os.fstat(stderr.fileno()).st_size > 64 * 1024
                ):
                    process.kill()
                    process.wait()
                    fail("native operator status client exceeded its runtime bound")
                time.sleep(0.01)
            if (
                os.fstat(stdout.fileno()).st_size > MAX_HTTP_BYTES
                or os.fstat(stderr.fileno()).st_size > 64 * 1024
            ):
                fail("native operator status client exceeded its output bound")
            stdout.seek(0)
            payload = stdout.read(MAX_HTTP_BYTES + 1)
            if process.returncode != 0:
                fail("native operator status client refused the protected read")
        after_sha256, after_info = hash_regular(
            client,
            MAX_BINARY_BYTES,
            "native operator status client",
            owner_uid=UID,
        )
        if (
            after_sha256 != before_sha256
            or metadata_identity(after_info) != metadata_identity(before_info)
        ):
            fail("native operator status client changed during protected read")
        try:
            value = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ResetError(
                "native operator status client returned invalid JSON"
            ) from error
        if not isinstance(value, dict):
            fail("native operator status client did not return one JSON object")
        return value

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

    def peer_sample(
        self,
        port: int,
        network_id: str,
        private_key_file: Path,
    ) -> dict[str, object]:
        root = f"http://127.0.0.1:{port}"
        self._request(f"{root}/health", parse_json=False)
        self._request(f"{root}/readyz", parse_json=False)
        status = self._request(f"{root}/status", parse_json=True)
        sumeragi = self._protected_status(port, network_id, private_key_file)
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
        mode = context.get("mode")
        if (
            context.get("validator_count") != PEER_COUNT
            or not isinstance(quorum, dict)
            or quorum.get("min_signers") != 3
            or isinstance(quorum.get("total_power"), bool)
            or not isinstance(quorum.get("total_power"), int)
            or quorum["total_power"] < PEER_COUNT
            or not isinstance(mode, dict)
            or set(mode) != {"mode", "details"}
            or mode.get("mode") not in {"permissioned", "npos"}
            or mode.get("details") is not None
        ):
            fail("validator does not expose the exact four-validator 3-of-4 quorum")
        block_hash = subject.get("block_hash")
        if not isinstance(block_hash, str):
            fail("validator omitted its committed block hash")
        match = BLOCK_HASH_RE.fullmatch(block_hash)
        if match is None:
            fail("validator committed block hash is not canonical")
        commit_qc = sumeragi.get("last_commit_qc")
        if not isinstance(commit_qc, dict):
            fail("validator omitted its durable CommitQC")
        certificate = commit_qc.get("certificate")
        if not isinstance(certificate, dict):
            fail("validator CommitQC certificate is malformed")
        round_record = certificate.get("round")
        phase = certificate.get("phase")
        if (
            not isinstance(round_record, dict)
            or round_record.get("height") != committed
            or isinstance(round_record.get("view"), bool)
            or not isinstance(round_record.get("view"), int)
            or round_record["view"] < 0
            or not isinstance(phase, dict)
            or set(phase) != {"phase", "details"}
            or phase.get("phase") != "commit"
            or phase.get("details") is not None
            or certificate.get("subject") != subject
        ):
            fail("validator CommitQC does not bind its durable committed subject")
        signer_count = commit_qc.get("signer_count")
        minimum_signers = commit_qc.get("min_signers")
        signed_power = commit_qc.get("signed_power")
        total_power = commit_qc.get("total_power")
        if (
            commit_qc.get("validator_count") != PEER_COUNT
            or signer_count != 3
            or minimum_signers != 3
            or isinstance(signed_power, bool)
            or not isinstance(signed_power, int)
            or isinstance(total_power, bool)
            or not isinstance(total_power, int)
            or total_power != quorum["total_power"]
            or signed_power > total_power
            or signed_power * 3 <= total_power * 2
            or (mode.get("mode") == "permissioned" and signed_power != signer_count)
        ):
            fail("validator durable CommitQC lacks the exact 3-of-4 quorum")
        fingerprints: dict[str, str] = {}
        for field in (
            "height_context_id",
            "node_fingerprint",
            "build_fingerprint",
            "config_fingerprint",
        ):
            value = sumeragi.get(field)
            if value in (None, "", {}):
                fail(f"validator omitted its {field}")
            fingerprints[field] = json.dumps(
                value,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
        return {
            "port": port,
            "height": committed,
            "block_hash": match.group(1).lower(),
            "validator_count": PEER_COUNT,
            "minimum_signers": 3,
            "signer_count": signer_count,
            "signed_power": signed_power,
            "total_power": quorum["total_power"],
            **fingerprints,
        }

    def fleet_sample(
        self,
        ports: Sequence[int],
        network_id: str,
        private_key_files: Sequence[Path],
    ) -> FleetSample:
        if (
            len(ports) != PEER_COUNT
            or len(private_key_files) != PEER_COUNT
            or len(set(private_key_files)) != PEER_COUNT
        ):
            fail("health authentication requires exactly four ordered peer key files")
        samples = tuple(
            self.peer_sample(port, network_id, private_key_file)
            for port, private_key_file in zip(ports, private_key_files)
        )
        if len({sample["port"] for sample in samples}) != PEER_COUNT:
            fail("health sample is not the exact four-validator cohort")
        heights = {sample["height"] for sample in samples}
        hashes = {sample["block_hash"] for sample in samples}
        if len(heights) != 1 or len(hashes) != 1:
            fail("four validators disagree on their durable committed frontier")
        for field in (
            "height_context_id",
            "build_fingerprint",
            "config_fingerprint",
        ):
            if len({sample[field] for sample in samples}) != 1:
                fail(f"four validators disagree on {field}")
        if len({sample["node_fingerprint"] for sample in samples}) != PEER_COUNT:
            fail("four validator roots do not expose four distinct node identities")
        return FleetSample(
            height=int(next(iter(heights))),
            block_hash=str(next(iter(hashes))),
            peers=samples,
        )

    def wait_fleet(
        self,
        ports: Sequence[int],
        network_id: str,
        private_key_files: Sequence[Path],
        limits: Limits,
    ) -> tuple[FleetSample, FleetSample]:
        if (
            len(private_key_files) != PEER_COUNT
            or len(set(private_key_files)) != PEER_COUNT
        ):
            fail("health authentication requires four distinct ordered peer key files")
        deadline = time.monotonic() + limits.startup_timeout_seconds
        last_error: Exception | None = None
        first: FleetSample | None = None
        while time.monotonic() < deadline:
            try:
                first = self.fleet_sample(ports, network_id, private_key_files)
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
                current = self.fleet_sample(ports, network_id, private_key_files)
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


def require_candidate_inputs_unchanged(plan: ResetPlan) -> None:
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
    receipt, inventory = _require_nevo_genesis_integrity(
        plan.activation,
        plan.bundle_plan.manifest,
    )
    if inventory != plan.validator_artifact_inventory:
        fail("candidate validator artifact inventory changed after planning")
    verifier_identity = _run_native_genesis_verifier(plan.activation, receipt)
    if verifier_identity != plan.genesis_native_verifier_identity:
        fail("candidate native genesis verifier identity changed after planning")
    operator_client_identity = _require_operator_status_client(plan.activation)
    if operator_client_identity != plan.operator_status_client_identity:
        fail("candidate native operator status client identity changed after planning")


def require_plan_unchanged(plan: ResetPlan) -> None:
    require_candidate_inputs_unchanged(plan)
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
    rollback_private_key_files: Sequence[Path],
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
                rollback_private_key_files,
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


def _require_exact_operator_key_paths(
    plan: ResetPlan,
    candidate: Sequence[Path],
    predecessor: Sequence[Path],
) -> None:
    expected_candidate = tuple(
        peer.workdir / "runtime/validator-signer.key" for peer in plan.candidate
    )
    expected_predecessor = tuple(
        peer.workdir / "runtime/validator-signer.key" for peer in plan.predecessor
    )
    if tuple(candidate) != expected_candidate:
        fail("candidate operator keys are not the exact ordered peer runtime paths")
    if tuple(predecessor) != expected_predecessor:
        fail("rollback operator keys are not the exact ordered predecessor runtime paths")


def apply_reset(
    plan: ResetPlan,
    *,
    confirmation: str,
    operator_private_key_files: Sequence[Path],
    rollback_operator_private_key_files: Sequence[Path],
    layout: Layout = PRODUCTION_LAYOUT,
    ops: LaunchctlOps | None = None,
    health: HealthClient | None = None,
) -> dict[str, object]:
    if confirmation != plan.activation.confirmation:
        fail("destructive confirmation does not bind the exact activation manifest")
    _require_exact_operator_key_paths(
        plan,
        operator_private_key_files,
        rollback_operator_private_key_files,
    )
    ops = ops or LaunchctlOps()
    health = health or HealthClient(
        plan.activation.operator_status_client,
        plan.activation.operator_status_client_sha256,
    )
    acquire_lock(layout.lock_path, layout.taira_root)
    mutated = False
    try:
        ops.require_initial_cohort()
        for peer in plan.predecessor:
            ops.require_loaded_definition(peer.plist_path, peer.plist_body)
        require_plan_unchanged(plan)
        _require_exact_operator_key_paths(
            plan,
            operator_private_key_files,
            rollback_operator_private_key_files,
        )
        old_fleet = health.wait_fleet(
            [peer.torii_port for peer in plan.predecessor],
            plan.predecessor[0].network_id,
            rollback_operator_private_key_files,
            plan.activation.limits,
        )
        prepare_archive(plan, old_fleet, layout)
        require_plan_unchanged(plan)
        _require_exact_operator_key_paths(
            plan,
            operator_private_key_files,
            rollback_operator_private_key_files,
        )
        try:
            mutated = True
            stop_all(ops, LABELS)
            require_plan_unchanged(plan)
            require_predecessor_artifacts_unchanged(plan.predecessor)
            install_candidate_plists(plan)
            require_candidate_inputs_unchanged(plan)
            bootstrap_all(ops, plan.candidate)
            candidate_fleet = health.wait_fleet(
                [peer.torii_port for peer in plan.candidate],
                plan.network_id,
                operator_private_key_files,
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
                    rollback_operator_private_key_files,
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
        "genesis_native_verifier": str(plan.activation.genesis_native_verifier),
        "genesis_native_verifier_sha256": (
            plan.activation.genesis_native_verifier_sha256
        ),
        "genesis_external_signer_sha256": (
            plan.activation.genesis_external_signer_sha256
        ),
        "operator_status_client": str(plan.activation.operator_status_client),
        "operator_status_client_sha256": (
            plan.activation.operator_status_client_sha256
        ),
        "genesis_public_key": plan.activation.genesis_public_key,
        "genesis_expected_hash": plan.activation.genesis_expected_hash,
        "genesis_artifact_linkage_sha256": (
            plan.activation.genesis_artifact_linkage_sha256
        ),
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
    value.add_argument("--operator-private-key-file", type=Path, action="append")
    value.add_argument(
        "--rollback-operator-private-key-file", type=Path, action="append"
    )
    return value


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    require_runtime_identity()
    activation = load_activation(args.manifest)
    if activation.local_testnet_source_closure_sha256 is not None:
        require_local_testnet_source_runtime(
            activation.local_testnet_source_closure_sha256,
            activation.local_testnet_python_sha256,
        )
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
    if (
        args.confirm_reset is None
        or args.operator_private_key_file is None
        or len(args.operator_private_key_file) != PEER_COUNT
        or args.rollback_operator_private_key_file is None
        or len(args.rollback_operator_private_key_file) != PEER_COUNT
    ):
        fail(
            "--apply requires --confirm-reset plus exactly four ordered candidate and four ordered rollback --operator-private-key-file arguments"
        )
    result = apply_reset(
        plan,
        confirmation=args.confirm_reset,
        operator_private_key_files=args.operator_private_key_file,
        rollback_operator_private_key_files=args.rollback_operator_private_key_file,
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
