#!/usr/bin/env python3
"""Deploy one authenticated four-validator Taira v21 fresh-reset cohort.

Without ``--apply`` this command is strictly read-only: it verifies one signed
rollout admission plus its independently parsed qualified BOI handoff, binds
the exact signed archive and 13-artifact inventory to the binary, supervisor,
reset manifest, and exact four configs, then authenticates the
current launchd cohort, disk headroom, and a read-only directory fsync barrier.
An explicitly authorized reset of an already-degraded testnet may use
``--allow-absent-old-child`` only when an exact loaded old supervisor has
neither a PID file nor any child process. ``--apply`` additionally requires root,
re-verifies admission under the deployment lock, atomically consumes its
receipt in the canonical protected replay ledger, and installs
content-addressed root-owned code and validates all four configs before
mutating the old cohort.  The
receipt consumption is restored if deployment never reaches that first cohort
mutation.  The rollout replaces all four LaunchDaemons as one cohort, proves
ordinary node readiness, the exact seven-lane/five-dataspace Taira topology,
and advancing consensus, and proves one supervised child can restart without
replacing its supervisor. Any failed rollout restores the prior cohort.

The controller never prints config contents, process command lines, HTTP
bodies, or other runtime signing material.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import dataclasses
import fcntl
import grp
import hashlib
import json
import os
import plistlib
import pwd
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable, NoReturn, Optional, Sequence

try:
    from scripts import build_privacy_v1_boi_handoff as boi_handoff
    from scripts import deploy_taira_v21_reset_authority as deploy_authority
    from scripts import deploy_taira_v21_reset_health as deploy_health
    from scripts.operator_http_headers import load_operator_context_from_file
    from scripts import render_taira_validator_bundle as validator_renderer
    from scripts import taira_authority_client
    from scripts import taira_rollout_admission as rollout_admission
    from scripts import taira_constants
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import build_privacy_v1_boi_handoff as boi_handoff
    import deploy_taira_v21_reset_authority as deploy_authority
    import deploy_taira_v21_reset_health as deploy_health
    from operator_http_headers import load_operator_context_from_file
    import render_taira_validator_bundle as validator_renderer
    import taira_authority_client
    import taira_rollout_admission as rollout_admission
    import taira_constants

PEER_COUNT = taira_constants.PEER_COUNT
CHAIN_ID = taira_constants.CHAIN_ID
CHAIN_DISCRIMINANT = taira_constants.CHAIN_DISCRIMINANT
NETWORK_NAME = taira_constants.NETWORK_NAME
NETWORK_ID = taira_constants.NETWORK_ID
PROTOCOL_VERSION = 4
TAIRA_LANE_COUNT = 7
UNIVERSAL_DATASPACE_ID = 0
DPN_DATASPACE_ID = 10
IS_DATASPACE_ID = 6647857470246403404
IS2_DATASPACE_ID = 8477022798449861195
CBSI_DATASPACE_ID = 20
CORE_LANE_ALIAS = "core"
GOVERNANCE_LANE_ALIAS = "governance"
ZK_LANE_ALIAS = "zk"
DPN_LANE_ALIAS = "dpn"
EXTERNAL_POC_LANE_ALIAS = "external-poc"
BOI_MOBILE_LANE_ALIAS = "boi-mobile"
CBSI_LANE_ALIAS = "cbsi"
TAIRA_PHYSICAL_DATASPACES = (
    ("universal", UNIVERSAL_DATASPACE_ID),
    ("dpn", DPN_DATASPACE_ID),
    ("is", IS_DATASPACE_ID),
    ("is2", IS2_DATASPACE_ID),
    ("cbsi", CBSI_DATASPACE_ID),
)
TAIRA_LANE_DATASPACE_BINDINGS = (
    (0, CORE_LANE_ALIAS, "universal", UNIVERSAL_DATASPACE_ID),
    (1, GOVERNANCE_LANE_ALIAS, "universal", UNIVERSAL_DATASPACE_ID),
    (2, ZK_LANE_ALIAS, "universal", UNIVERSAL_DATASPACE_ID),
    (3, DPN_LANE_ALIAS, "dpn", DPN_DATASPACE_ID),
    (4, EXTERNAL_POC_LANE_ALIAS, "is", IS_DATASPACE_ID),
    (5, BOI_MOBILE_LANE_ALIAS, "is2", IS2_DATASPACE_ID),
    (6, CBSI_LANE_ALIAS, "cbsi", CBSI_DATASPACE_ID),
)
NODE_STORAGE_BUDGET_BYTES = 64 * 1024 * 1024 * 1024
NODE_STORAGE_BUDGET_POLICY = "bounded-64-gib-per-validator"
NODE_STORAGE_WEIGHTS = {
    "kura_blocks_bps": 7_499,
    "wsv_snapshots_bps": 2_000,
    "sorafs_bps": 1,
    "soranet_spool_bps": 250,
    "soravpn_spool_bps": 250,
}
DEFAULT_MINIMUM_FREE_BYTES = 16 * 1024 * 1024 * 1024
DEFAULT_MAXIMUM_FSYNC_LATENCY_MS = 250
DEFAULT_HEALTH_TIMEOUT_SECONDS = 240
RESTART_PROOF_TIMEOUT_SECONDS = 45
# Config validation is CPU-local and normally completes in seconds. Keep a
# stuck hostile candidate from consuming 4 x 180 seconds before rollout health.
CONFIG_CHECK_TIMEOUT_SECONDS = 30
MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024
MAX_CONFIG_BYTES = 2 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_HTTP_BYTES = 4 * 1024 * 1024
MAX_PROCESS_ARGUMENT_BYTES = 1024 * 1024
MAX_PROCESS_ARGUMENTS = 256
DARWIN_CTL_KERN = 1
DARWIN_KERN_PROCARGS2 = 49
MAX_TERMINAL_UNHEALTHY_BYTES = 1024
EXTERNAL_TOOL_UID_ENV = "IROHA_TAIRA_EXTERNAL_TOOL_UID"
EXTERNAL_TOOL_GID_ENV = "IROHA_TAIRA_EXTERNAL_TOOL_GID"
MAX_RESTART_LOG_DELTA_BYTES = 8 * 1024 * 1024
RESTART_LOG_PREFIX_GUARD_BYTES = 4 * 1024
MAX_BUNDLE_BYTES = 64 * 1024 * 1024 * 1024
KAGEMUSHA_CONFIG_PROJECTION_SCHEMA = "iroha.taira.kagemusha-config-projection.v1"
KAGEMUSHA_MANIFEST_DIRECTORY_INVENTORY_SCHEMA = (
    "iroha.taira.kagemusha-manifest-directory-inventory.v1"
)
KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH = Path("policy/release-policy-v1.norito")
KAGEMUSHA_ARTIFACT_RELATIVE_PATH = Path("catalog")
KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH = Path(
    "seals/catalog-qualification-v1.norito"
)
KAGEMUSHA_MAX_DECODED_BYTES = 256 * 1024 * 1024
MAX_KAGEMUSHA_RELEASE_POLICY_BYTES = 64 * 1024
MAX_KAGEMUSHA_QUALIFICATION_SEAL_BYTES = 8 * 1024 * 1024
MAX_KAGEMUSHA_MANIFEST_BYTES = 1024 * 1024
MAX_KAGEMUSHA_CATALOG_RELEASES = 16
MAX_KAGEMUSHA_MANIFEST_DIGEST_SIDECAR_BYTES = 65
KAGEMUSHA_CONFIG_PROJECTION_KEYS = frozenset(
    {
        "schema",
        "release_root",
        "release_policy_path",
        "artifact_dir",
        "catalog_qualification_seal_path",
        "max_decoded_bytes",
    }
)
KAGEMUSHA_RESET_MANIFEST_FIELDS = frozenset(
    {
        "kagemusha_release_root",
        "kagemusha_release_policy_sha256",
        "kagemusha_activation_authority",
        "kagemusha_config_projection",
        "kagemusha_config_projection_sha256",
    }
)
KAGEMUSHA_OFFLINE_TABLE = ("settlement", "offline")
KAGEMUSHA_MANAGED_OFFLINE_FIELDS: dict[str, str] = {
    "kagemusha_release_policy_path": "string",
    "kagemusha_artifact_dir": "string",
    "kagemusha_catalog_qualification_seal_path": "string",
    "kagemusha_max_decoded_bytes": "integer",
}
SNAPSHOT_LOAD_SUCCESS_MARKER = b"Successfully loaded the state from a snapshot"
SNAPSHOT_LOAD_FALLBACK_MARKERS = (
    b"Snapshot restore is disabled by configuration",
    b"Didn't find a state snapshot; creating an empty state",
    b"Failed to load state snapshot; checking whether Kura can rebuild from an empty state",
    b"Kura retains the configured-primary replay floor; rebuilding state from blocks",
)
EMPTY_TREE_SHA256 = hashlib.sha256().hexdigest()
LABELS = tuple(
    f"io.soramitsu.taira.validator-{index}" for index in range(1, PEER_COUNT + 1)
)
SLUGS = taira_constants.SLUGS
TORII_PORTS = tuple(29_080 + index for index in range(PEER_COUNT))
P2P_PORTS = tuple(33_337 + index for index in range(PEER_COUNT))
TOP_LEVEL_NAMES = {
    "base-config.toml",
    "genesis.json",
    "genesis.signed.nrt",
    "rendered",
    "reset-manifest.json",
    "validator-roster.toml",
}
VALIDATOR_NAMES = {"codec", "config.toml", "configs", "manifests", "runtime", "storage"}
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
GENESIS_PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}")
RECEIPT_PUBLIC_PAYLOAD_RE = re.compile(r"(?:02|03)[0-9a-f]{64}")
LIFECYCLE_NODE_ID_RE = re.compile(
    r"taira-node:receipt-signer:secp256k1:sha256:[0-9a-f]{64}"
)
BLOCK_HASH_RE = re.compile(
    r"(?:hash:)?([0-9A-Fa-f]{64})(?:#[0-9A-Fa-f]{4})?"
)
TERMINAL_UNHEALTHY_SCHEMA = "taira-terminal-unhealthy-v1"
LIFECYCLE_STATE_SCHEMA = "iroha.taira.peer-supervisor-lifecycle-state.v1"
LIFECYCLE_BINDING_DOMAIN = (
    b"iroha.taira.peer-supervisor-lifecycle-binding.v1\x00"
)
INSTALL_ROOT = Path("/Library/SORA/Taira")
LAUNCH_DAEMONS = Path("/Library/LaunchDaemons")
DEFAULT_SUPERVISOR_PYTHON = Path("/usr/bin/python3")
SYSTEM_PYTHON_DEVELOPER_DIR = "/Library/Developer/CommandLineTools"
DEPLOYMENT_LOCK = INSTALL_ROOT / "deploy-v21.lock"
ADMISSION_REPLAY_LEDGER = INSTALL_ROOT / "rollout-admission-replay-v1.json"
ADMISSION_REPLAY_LEDGER_MODE = 0o644
MACOS_ACL_INSPECTOR = Path("/bin/ls")
MACOS_ACL_CLEARER = Path("/bin/chmod")
MACOS_ACL_COMMAND_TIMEOUT_SECONDS = 5
MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES = 64 * 1024
DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT = (
    "iroha.taira.deploy-authenticated-run-nonce.v1"
)
DEPLOY_AUTHORIZATION_LEASE_CONTRACT = (
    "iroha.taira.deploy-authorization-lease.v1"
)
DEPLOY_RESULT_BINDING_CONTRACT = "iroha.taira.deploy-result-binding.v1"
DEPLOYMENT_OUTCOME_ATTRIBUTE = "_taira_authority_deployment_outcome"
COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT = (
    "iroha.taira.complete-source-identity-attestation.v1"
)
DEPLOY_ISSUANCE_BARRIER = (
    "missing preprovisioned iroha.taira.deploy-authenticated-run-nonce.v1: "
    "neither workflow run ID nor attempt may authorize deployment or replay "
    "consumption; missing preprovisioned "
    "iroha.taira.complete-source-identity-attestation.v1: a root-owned authority "
    "record must independently bind source commit, DPN validator release commit, "
    "the exact canonical Cargo.lock digest, and workspace source-manifest digest "
    "(or one stronger immutable candidate identity); deployment is disabled for "
    "both dry-run and apply before identity, admission, or path inspection"
)
class DeploymentError(RuntimeError):
    """Raised when an identity, safety, rollout, or rollback gate fails."""

def _mark_deployment_outcome(
    error: BaseException, outcome: str
) -> BaseException:
    """Carry the authority's terminal rollback classification to the caller."""
    setattr(error, DEPLOYMENT_OUTCOME_ATTRIBUTE, outcome)
    return error

def fail(message: str) -> NoReturn:
    """Raise one redaction-safe deployment refusal."""
    raise DeploymentError(message)

def require_deploy_issuance_contracts() -> None:
    """Authenticate the fixed deploy-issuance binding and live service."""
    try:
        taira_authority_client.preflight("deploy-issuance")
    except taira_authority_client.TairaAuthorityClientError as error:
        raise DeploymentError(f"{DEPLOY_ISSUANCE_BARRIER}: {error}") from error


_DEPLOY_AUTHORITY = deploy_authority.DeploymentAuthorityProjection(
    artifact_factory=taira_authority_client.Artifact,
    canonical_json_bytes=taira_authority_client.canonical_json_bytes,
    bounds=deploy_authority.ArtifactBounds(
        binary=MAX_BINARY_BYTES,
        config=MAX_CONFIG_BYTES,
        manifest=MAX_MANIFEST_BYTES,
        supervisor=4 * 1024 * 1024,
        kagemusha_policy=MAX_KAGEMUSHA_RELEASE_POLICY_BYTES,
        kagemusha_qualification_seal=MAX_KAGEMUSHA_QUALIFICATION_SEAL_BYTES,
        kagemusha_manifest=MAX_KAGEMUSHA_MANIFEST_BYTES,
        kagemusha_manifest_sidecar=MAX_KAGEMUSHA_MANIFEST_DIGEST_SIDECAR_BYTES,
    ),
    contracts=deploy_authority.AuthorityContracts(
        complete_source=COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT,
        run_assignment=DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT,
        lease_authorization=DEPLOY_AUTHORIZATION_LEASE_CONTRACT,
        result_binding=DEPLOY_RESULT_BINDING_CONTRACT,
    ),
    qualified_handoff_manifest=boi_handoff.QUALIFIED_HANDOFF_MANIFEST,
    qualified_handoff_maximum=boi_handoff.MAX_HANDOFF_MANIFEST_BYTES,
)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Decode JSON while rejecting ambiguous duplicate object members."""
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member {key!r}")
        result[key] = value
    return result

def require_sha256(value: object, label: str) -> str:
    """Require a lowercase SHA-256 literal."""
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be one lowercase SHA-256 digest")
    return value

def require_distinct_signing_fingerprints(
    release_fingerprint: object,
    qualification_fingerprint: object,
) -> tuple[str, str]:
    """Require separately pinned release and BOI qualification authorities."""
    release = require_sha256(release_fingerprint, "trusted signing fingerprint")
    qualification = require_sha256(
        qualification_fingerprint,
        "trusted BOI qualification signing fingerprint",
    )
    if release == qualification:
        fail("release and BOI qualification signing identities must be distinct")
    return release, qualification

def require_genesis_expected_hash(value: object) -> str:
    """Require one canonical Iroha hash suitable as a genesis trust root."""
    value = require_sha256(value, "genesis expected hash")
    if int(value[-2:], 16) & 1 == 0:
        fail("genesis expected hash must carry the Iroha marker bit")
    return value

def require_commit(value: object, label: str = "expected source commit") -> str:
    """Require one full nonzero lowercase Git object id."""
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        fail(f"{label} must be one full nonzero lowercase Git object id")
    return value

def _run_bounded_macos_acl_command(
    program: Path, option: str, path: Path, label: str
) -> subprocess.CompletedProcess[bytes]:
    """Run one absolute macOS ACL tool with bounded time and retained output."""
    if not program.is_absolute() or program not in {
        MACOS_ACL_INSPECTOR,
        MACOS_ACL_CLEARER,
    }:
        fail(f"macOS ACL command is not one pinned absolute tool: {program}")
    try:
        result = subprocess.run(
            [str(program), option, str(path)],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=MACOS_ACL_COMMAND_TIMEOUT_SECONDS,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError(
            f"bounded macOS ACL command failed for {label}: {path}"
        ) from error
    if (
        len(result.stdout) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        or len(result.stderr) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
    ):
        fail(f"macOS ACL command output exceeded its bound for {label}: {path}")
    return result

def require_acl_free_path(
    path: Path,
    label: str,
    *,
    descriptor: Optional[int] = None,
) -> os.stat_result:
    """Require a stable path, and on macOS prove it has no extended ACL."""
    before = path.lstat()
    if descriptor is not None and metadata_identity(
        os.fstat(descriptor)
    ) != metadata_identity(before):
        fail(f"{label} name does not identify its opened inode: {path}")
    if sys.platform == "darwin":
        result = _run_bounded_macos_acl_command(
            MACOS_ACL_INSPECTOR, "-ldeq", path, label
        )
        if (
            result.returncode != 0
            or result.stderr
            or not result.stdout.endswith(b"\n")
            or result.stdout.count(b"\n") != 1
        ):
            fail(f"{label} must not have an extended ACL: {path}")
    after = path.lstat()
    if metadata_identity(after) != metadata_identity(before):
        fail(f"{label} changed during ACL validation: {path}")
    if descriptor is not None and metadata_identity(
        os.fstat(descriptor)
    ) != metadata_identity(after):
        fail(f"{label} name changed from its opened inode: {path}")
    return after

def clear_owned_temporary_acl(path: Path, descriptor: int, label: str) -> None:
    """Clear inherited macOS ACLs only on one unpublished, owned temp inode."""
    before_path = path.lstat()
    before_opened = os.fstat(descriptor)
    if (
        metadata_identity(before_path) != metadata_identity(before_opened)
        or not stat.S_ISREG(before_opened.st_mode)
        or before_opened.st_nlink != 1
        or before_opened.st_uid != os.geteuid()
    ):
        fail(f"{label} temporary name does not identify its owned opened inode")
    if sys.platform == "darwin":
        result = _run_bounded_macos_acl_command(MACOS_ACL_CLEARER, "-N", path, label)
        if result.returncode != 0 or result.stdout or result.stderr:
            fail(f"failed to clear inherited ACL from {label}: {path}")
    after_path = path.lstat()
    after_opened = os.fstat(descriptor)
    stable_before = (
        before_opened.st_dev,
        before_opened.st_ino,
        before_opened.st_mode,
        before_opened.st_uid,
        before_opened.st_gid,
        before_opened.st_nlink,
        before_opened.st_size,
        before_opened.st_mtime_ns,
    )
    stable_after = (
        after_opened.st_dev,
        after_opened.st_ino,
        after_opened.st_mode,
        after_opened.st_uid,
        after_opened.st_gid,
        after_opened.st_nlink,
        after_opened.st_size,
        after_opened.st_mtime_ns,
    )
    if stable_after != stable_before or metadata_identity(
        after_path
    ) != metadata_identity(after_opened):
        fail(f"{label} changed while its inherited ACL was cleared")
    require_acl_free_path(path, label, descriptor=descriptor)

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

def growing_file_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return identity fields that remain stable while a regular file grows."""
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
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

def require_private_entry(
    path: Path, owner_uid: int, owner_gid: int, *, directory: bool
) -> os.stat_result:
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
            require_private_entry(
                current_path / name, owner_uid, owner_gid, directory=True
            )
        for name in file_names:
            info = require_private_entry(
                current_path / name, owner_uid, owner_gid, directory=False
            )
            total += info.st_size
            if total > MAX_BUNDLE_BYTES:
                fail("reset bundle exceeds the bounded 64 GiB deployment corridor")
    return total

CONFIG_PROJECTION_FIELDS: dict[tuple[str, ...], dict[str, str]] = {
    (): {
        "chain": "string",
        "chain_discriminant": "integer",
    },
    ("network",): {"address": "string"},
    ("torii",): {
        "address": "string",
        "receipt_public_key": "string",
        "receipt_private_key": "string",
    },
    ("nexus", "storage"): {"local_budget_bytes": "integer"},
    ("nexus", "storage", "disk_budget_weights"): {
        "kura_blocks_bps": "integer",
        "wsv_snapshots_bps": "integer",
        "sorafs_bps": "integer",
        "soranet_spool_bps": "integer",
        "soravpn_spool_bps": "integer",
    },
    ("genesis",): {
        "file": "string",
        "public_key": "string",
        "expected_hash": "string",
    },
}
OPTIONAL_CONFIG_PROJECTION_FIELDS: dict[tuple[str, ...], dict[str, str]] = {
    KAGEMUSHA_OFFLINE_TABLE: KAGEMUSHA_MANAGED_OFFLINE_FIELDS,
}
TOML_TABLE_RE = re.compile(r"^\[([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\]$")
TOML_ARRAY_TABLE_RE = re.compile(r"^\[\[([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\]\]$")
TOML_ASSIGNMENT_RE = re.compile(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$")
TOML_UNSIGNED_INTEGER_RE = re.compile(r"^(?:0|[1-9][0-9]*)$")
TOML_HEX_RE = re.compile(r"^[0-9A-Fa-f]+$")

def _strip_toml_comment(line: str, label: str, line_number: int) -> str:
    """Strip a TOML comment without mistaking ``#`` inside a string."""
    quote: Optional[str] = None
    escaped = False
    index = 0
    while index < len(line):
        character = line[index]
        if quote is None:
            if character == "#":
                return line[:index]
            if character in ('"', "'"):
                if line[index : index + 3] == character * 3:
                    fail(
                        f"{label} uses an unsupported multiline string "
                        f"at line {line_number}"
                    )
                quote = character
        elif quote == '"':
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quote = None
        elif character == "'":
            quote = None
        index += 1
    if quote is not None or escaped:
        fail(f"{label} has an unterminated string at line {line_number}")
    return line

def _decode_toml_string(value: str, label: str, line_number: int) -> str:
    """Decode one single-line TOML basic or literal string."""
    if len(value) < 2 or value[0] not in ('"', "'"):
        fail(f"{label} has a malformed string at line {line_number}")
    quote = value[0]
    output: list[str] = []
    index = 1
    while index < len(value):
        character = value[index]
        if character == quote:
            if index != len(value) - 1:
                fail(f"{label} has trailing data at line {line_number}")
            return "".join(output)
        if quote == "'" or character != "\\":
            if ord(character) < 0x20 or ord(character) == 0x7F:
                fail(f"{label} contains a control character at line {line_number}")
            output.append(character)
            index += 1
            continue
        index += 1
        if index >= len(value):
            fail(f"{label} has an incomplete escape at line {line_number}")
        escape = value[index]
        simple_escapes = {
            '"': '"',
            "\\": "\\",
            "b": "\b",
            "t": "\t",
            "n": "\n",
            "f": "\f",
            "r": "\r",
        }
        if escape in simple_escapes:
            output.append(simple_escapes[escape])
            index += 1
            continue
        if escape not in ("u", "U"):
            fail(f"{label} has an invalid escape at line {line_number}")
        width = 4 if escape == "u" else 8
        digits = value[index + 1 : index + 1 + width]
        if len(digits) != width or TOML_HEX_RE.fullmatch(digits) is None:
            fail(f"{label} has an invalid Unicode escape at line {line_number}")
        codepoint = int(digits, 16)
        if codepoint > 0x10FFFF or 0xD800 <= codepoint <= 0xDFFF:
            fail(f"{label} has an invalid Unicode scalar at line {line_number}")
        output.append(chr(codepoint))
        index += width + 1
    fail(f"{label} has an unterminated string at line {line_number}")

def _decode_projection_value(
    raw: str, value_type: str, label: str, line_number: int
) -> object:
    """Decode one required projected TOML scalar with no coercion."""
    value = raw.strip()
    if value_type == "string":
        return _decode_toml_string(value, label, line_number)
    if value_type == "boolean":
        if value == "true":
            return True
        if value == "false":
            return False
        fail(f"{label} has a malformed boolean at line {line_number}")
    if TOML_UNSIGNED_INTEGER_RE.fullmatch(value) is None:
        fail(f"{label} has a malformed integer at line {line_number}")
    return int(value)


def _contains_quoted_managed_kagemusha_key(
    line: str, label: str, line_number: int
) -> bool:
    """Detect managed keys hidden behind TOML quoted-key escapes."""

    index = 0
    while index < len(line):
        quote = line[index]
        if quote not in ('"', "'"):
            index += 1
            continue
        start = index
        index += 1
        escaped = False
        while index < len(line):
            character = line[index]
            if quote == '"' and not escaped and character == "\\":
                escaped = True
                index += 1
                continue
            if character == quote and not escaped:
                raw = line[start : index + 1]
                decoded = _decode_toml_string(raw, label, line_number)
                remainder = line[index + 1 :].lstrip()
                if (
                    decoded in KAGEMUSHA_MANAGED_OFFLINE_FIELDS
                    and remainder.startswith("=")
                ):
                    return True
                index += 1
                break
            escaped = False
            index += 1
        else:
            fail(f"{label} has an unterminated string at line {line_number}")
    return False


def parse_config_projection_text(text: str, label: str) -> dict[str, Any]:
    """Extract required and managed fail-closed validator fields."""
    projected: dict[str, Any] = {}
    current_table: tuple[str, ...] = ()
    seen_tables: set[tuple[str, ...]] = {()}
    seen_fields: set[tuple[tuple[str, ...], str]] = set()
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        line = _strip_toml_comment(raw_line, label, line_number).strip()
        if not line:
            continue
        if line.startswith("["):
            array_match = TOML_ARRAY_TABLE_RE.fullmatch(line)
            table_match = TOML_TABLE_RE.fullmatch(line)
            if array_match is None and table_match is None:
                fail(f"{label} has a malformed table header at line {line_number}")
            table_text = (
                array_match.group(1)
                if array_match is not None
                else table_match.group(1)
            )
            current_table = tuple(table_text.split("."))
            projected_table = (
                current_table in CONFIG_PROJECTION_FIELDS
                or current_table in OPTIONAL_CONFIG_PROJECTION_FIELDS
            )
            if projected_table:
                table_kind = (
                    "required"
                    if current_table in CONFIG_PROJECTION_FIELDS
                    else "managed"
                )
                if array_match is not None:
                    fail(
                        f"{label} declares {table_kind} table {table_text} as an array "
                        f"at line {line_number}"
                    )
                if current_table in seen_tables:
                    fail(
                        f"{label} duplicates {table_kind} table {table_text} "
                        f"at line {line_number}"
                    )
                seen_tables.add(current_table)
            continue
        assignment = TOML_ASSIGNMENT_RE.fullmatch(line)
        required = CONFIG_PROJECTION_FIELDS.get(current_table)
        optional = OPTIONAL_CONFIG_PROJECTION_FIELDS.get(current_table)
        projected_fields = required if required is not None else optional
        contains_managed_assignment = any(
            re.search(
                rf"(?<![A-Za-z0-9_-])[\"']?{re.escape(key)}[\"']?\s*=",
                line,
            )
            is not None
            for key in KAGEMUSHA_MANAGED_OFFLINE_FIELDS
        ) or _contains_quoted_managed_kagemusha_key(line, label, line_number)
        if assignment is None:
            if projected_fields is not None and any(
                re.match(rf"^{re.escape(key)}(?:\s|=|$)", line)
                for key in projected_fields
            ):
                fail(
                    f"{label} has a malformed projected assignment "
                    f"at line {line_number}"
                )
            if contains_managed_assignment:
                fail(
                    f"{label} has a noncanonical managed Kagemusha assignment "
                    f"at line {line_number}"
                )
            continue
        key, raw_value = assignment.groups()
        if contains_managed_assignment and not (
            current_table == KAGEMUSHA_OFFLINE_TABLE
            and key in KAGEMUSHA_MANAGED_OFFLINE_FIELDS
        ):
            fail(
                f"{label} has a noncanonical managed Kagemusha assignment "
                f"at line {line_number}"
            )
        if key in KAGEMUSHA_MANAGED_OFFLINE_FIELDS and (
            current_table != KAGEMUSHA_OFFLINE_TABLE
        ):
            fail(
                f"{label} places managed Kagemusha field {key} outside "
                "settlement.offline"
            )
        if projected_fields is None or key not in projected_fields:
            continue
        identity = (current_table, key)
        if identity in seen_fields:
            dotted = ".".join((*current_table, key))
            field_kind = "required" if required is not None else "managed"
            fail(
                f"{label} duplicates {field_kind} field {dotted} "
                f"at line {line_number}"
            )
        seen_fields.add(identity)
        value = _decode_projection_value(
            raw_value,
            projected_fields[key],
            label,
            line_number,
        )
        destination = projected
        for component in current_table:
            child = destination.setdefault(component, {})
            if not isinstance(child, dict):
                fail(f"{label} has an ambiguous required table projection")
            destination = child
        destination[key] = value
    missing = [
        ".".join((*table, key))
        for table, fields in CONFIG_PROJECTION_FIELDS.items()
        for key in fields
        if (table, key) not in seen_fields
    ]
    if missing:
        fail(f"{label} lacks required fields: {', '.join(sorted(missing))}")
    return projected

def parse_toml(path: Path, owner_uid: int, owner_gid: int) -> dict[str, Any]:
    """Extract one bounded private validator config projection."""
    require_private_entry(path, owner_uid, owner_gid, directory=False)
    raw, _ = read_regular(path, MAX_CONFIG_BYTES)
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise DeploymentError(f"validator config is not UTF-8: {path}") from error
    return parse_config_projection_text(text, f"validator config {path}")

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

def require_manifest_hash(
    manifest: dict[str, Any], field: str, path: Path, maximum: int
) -> tuple[str, os.stat_result]:
    """Require a file digest to equal its reset-manifest binding."""
    expected = manifest.get(field)
    if not isinstance(expected, str):
        fail(f"reset manifest omitted {field}")
    require_sha256(expected, f"reset manifest {field}")
    actual, info = sha256_regular(path, maximum)
    if actual != expected:
        fail(f"reset manifest {field} does not match {path.name}")
    return actual, info

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
    config_identity: tuple[int, ...]
    workdir_identity: tuple[int, ...]
    storage_identity: tuple[int, ...]
    workdir_device: int
    workdir_inode: int
    storage_device: int
    storage_inode: int

@dataclasses.dataclass(frozen=True)
class KagemushaExternalPathIdentity:
    """Stable identity of one protected external Kagemusha path."""

    path: Path
    identity: tuple[int, ...]
    directory: bool


@dataclasses.dataclass(frozen=True)
class ReceiptSignerPlan:
    """One secret-free receipt signer bound to a canonical validator slug."""

    slug: str
    node_id: str
    public_key: str


def require_receipt_signer_map(
    value: object,
    label: str,
) -> tuple[ReceiptSignerPlan, ...]:
    """Validate the exact ordered public receipt-signer projection."""

    if not isinstance(value, dict) or list(value) != list(SLUGS):
        fail(f"{label} must bind the exact ordered four validator slugs")
    result: list[ReceiptSignerPlan] = []
    seen_nodes: set[str] = set()
    seen_keys: set[str] = set()
    for slug in SLUGS:
        row = value.get(slug)
        if not isinstance(row, dict) or set(row) != {"node_id", "public_key"}:
            fail(f"{label} row for {slug} is not schema-exact")
        public = row.get("public_key")
        if (
            not isinstance(public, dict)
            or set(public) != {"algorithm", "payload_hex"}
            or public.get("algorithm") != "secp256k1"
            or not isinstance(public.get("payload_hex"), str)
            or RECEIPT_PUBLIC_PAYLOAD_RE.fullmatch(public["payload_hex"]) is None
        ):
            fail(f"{label} row for {slug} has a noncanonical receipt public key")
        canonical_key = (
            validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX
            + public["payload_hex"].upper()
        )
        try:
            derived_node_id = validator_renderer.receipt_node_id(canonical_key)
        except ValueError as error:
            raise DeploymentError(
                f"{label} row for {slug} has an invalid receipt public key"
            ) from error
        if row.get("node_id") != derived_node_id:
            fail(f"{label} row for {slug} node ID is not derived from its receipt key")
        if derived_node_id in seen_nodes or canonical_key in seen_keys:
            fail(f"{label} aliases receipt signer identities")
        seen_nodes.add(derived_node_id)
        seen_keys.add(canonical_key)
        result.append(ReceiptSignerPlan(slug, derived_node_id, canonical_key))
    return tuple(result)


def receipt_signer_public_map(
    signers: Sequence[ReceiptSignerPlan],
) -> dict[str, dict[str, object]]:
    """Serialize an already verified signer plan without private material."""

    if tuple(row.slug for row in signers) != SLUGS:
        fail("receipt signer plan is not the exact ordered validator set")
    return {
        row.slug: {
            "node_id": row.node_id,
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": row.public_key[
                    len(validator_renderer.RECEIPT_PUBLIC_KEY_PREFIX) :
                ].lower(),
            },
        }
        for row in signers
    }


@dataclasses.dataclass(frozen=True)
class KagemushaExternalReleasePlan:
    """Bounded external Kagemusha inputs observed without reading proving keys."""
    release_root: Path
    policy_path: Path
    artifact_dir: Path
    qualification_seal_path: Path
    expected_policy_sha256: str
    policy_sha256: Optional[str]
    qualification_seal_sha256: Optional[str]
    manifest_directory_digests: tuple[str, ...]
    manifest_directory_inventory_sha256: Optional[str]
    manifest_files: tuple[Path, ...]
    manifest_digest_sidecars: tuple[Path, ...]
    protected_path_identities: tuple[KagemushaExternalPathIdentity, ...]
    bounded_material_present: bool

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
    manifest_identity: tuple[int, ...]
    signed_genesis_identity: tuple[int, ...]
    receipt_signers: tuple[ReceiptSignerPlan, ...]
    peers: tuple[PeerPlan, ...]
    bundle_bytes: int
    free_bytes: int
    free_bytes_by_device: tuple[tuple[int, int], ...]
    fsync_latency_ms: float
    kagemusha_config_projection_sha256: Optional[str] = None
    kagemusha_external_release: Optional[KagemushaExternalReleasePlan] = None


def _canonical_kagemusha_config_projection_bytes(value: object) -> bytes:
    """Mirror the reset composer's canonical release-artifact JSON encoding."""

    try:
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
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise DeploymentError(
            "Kagemusha config projection is not canonical JSON"
        ) from error


def _canonical_unresolved_absolute_path(value: object, label: str) -> Path:
    """Require one canonical absolute path without requiring it to exist yet."""
    if not isinstance(value, str):
        fail(f"{label} must be one canonical absolute path")
    path = Path(value)
    if (
        not path.is_absolute()
        or path == Path("/")
        or value.startswith("//")
        or os.path.normpath(value) != value
        or any(ord(character) < 0x20 for character in value)
    ):
        fail(f"{label} must be one canonical non-root absolute path")
    return path

def _validate_kagemusha_manifest_projection(
    manifest: dict[str, Any], bundle: Path
) -> tuple[Optional[dict[str, object]], Optional[str], Optional[str]]:
    """Authenticate the optional reset-manifest Kagemusha config projection."""
    named_fields = {
        key for key in manifest if isinstance(key, str) and key.startswith("kagemusha_")
    }
    present = named_fields & KAGEMUSHA_RESET_MANIFEST_FIELDS
    unexpected = named_fields - KAGEMUSHA_RESET_MANIFEST_FIELDS
    if unexpected:
        fail(
            "reset manifest contains unsupported Kagemusha fields: "
            + ", ".join(sorted(unexpected))
        )
    if not present:
        return None, None, None
    if present != KAGEMUSHA_RESET_MANIFEST_FIELDS:
        missing = sorted(KAGEMUSHA_RESET_MANIFEST_FIELDS - present)
        fail(
            "reset manifest contains a partial Kagemusha projection; missing: "
            + ", ".join(missing)
        )
    release_root = _canonical_unresolved_absolute_path(
        manifest["kagemusha_release_root"],
        "reset manifest Kagemusha release root",
    )
    if (
        release_root == bundle
        or release_root.is_relative_to(bundle)
        or bundle.is_relative_to(release_root)
    ):
        fail("Kagemusha release root and validator reset bundle must be disjoint")
    expected_policy_sha256 = require_sha256(
        manifest["kagemusha_release_policy_sha256"],
        "reset manifest Kagemusha release policy SHA-256",
    )
    authority = manifest["kagemusha_activation_authority"]
    if (
        not isinstance(authority, str)
        or not authority
        or authority.strip() != authority
        or len(authority.encode("utf-8")) > 1024
        or any(ord(character) < 0x20 for character in authority)
    ):
        fail("reset manifest Kagemusha activation authority is noncanonical")
    projection = manifest["kagemusha_config_projection"]
    if not isinstance(projection, dict) or set(projection) != set(
        KAGEMUSHA_CONFIG_PROJECTION_KEYS
    ):
        fail("reset manifest Kagemusha config projection keys are not exact")
    expected_projection: dict[str, object] = {
        "schema": KAGEMUSHA_CONFIG_PROJECTION_SCHEMA,
        "release_root": str(release_root),
        "release_policy_path": str(
            release_root / KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
        ),
        "artifact_dir": str(release_root / KAGEMUSHA_ARTIFACT_RELATIVE_PATH),
        "catalog_qualification_seal_path": str(
            release_root / KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
        ),
        "max_decoded_bytes": KAGEMUSHA_MAX_DECODED_BYTES,
    }
    if projection != expected_projection:
        fail("reset manifest Kagemusha config projection is not canonical")
    projection_sha256 = require_sha256(
        manifest["kagemusha_config_projection_sha256"],
        "reset manifest Kagemusha config projection SHA-256",
    )
    canonical_projection_sha256 = hashlib.sha256(
        _canonical_kagemusha_config_projection_bytes(expected_projection)
    ).hexdigest()
    if projection_sha256 != canonical_projection_sha256:
        fail("reset manifest Kagemusha config projection SHA-256 is not canonical")
    return expected_projection, projection_sha256, expected_policy_sha256

def _managed_kagemusha_offline_projection(
    projection: Optional[dict[str, object]],
) -> dict[str, object]:
    """Project reset-manifest field names onto settlement.offline names."""
    if projection is None:
        return {}
    return {
        "kagemusha_release_policy_path": projection["release_policy_path"],
        "kagemusha_artifact_dir": projection["artifact_dir"],
        "kagemusha_catalog_qualification_seal_path": projection[
            "catalog_qualification_seal_path"
        ],
        "kagemusha_max_decoded_bytes": projection["max_decoded_bytes"],
    }


def _capture_root_controlled_kagemusha_paths(
    release_root: Path,
    *,
    directories: Sequence[Path] = (),
    files: Sequence[Path] = (),
    _trust_boundary: Path = Path("/"),
    _trusted_uid: int = 0,
) -> tuple[KagemushaExternalPathIdentity, ...]:
    """Require protected ancestry and snapshot release-root-local identities."""

    if (
        not release_root.is_absolute()
        or not _trust_boundary.is_absolute()
        or (
            release_root != _trust_boundary
            and not release_root.is_relative_to(_trust_boundary)
        )
    ):
        fail("Kagemusha release custody boundary is invalid")
    directory_targets = {release_root, *directories}
    file_targets = set(files)
    if directory_targets & file_targets:
        fail("Kagemusha protected path has conflicting file and directory types")

    expected_directory: dict[Path, bool] = {}
    for target, is_directory in (
        *((path, True) for path in directory_targets),
        *((path, False) for path in file_targets),
    ):
        if target != release_root and not target.is_relative_to(release_root):
            fail("Kagemusha protected path escapes its release root")
        prior = expected_directory.get(target)
        if prior is not None and prior != is_directory:
            fail("Kagemusha protected path has conflicting entry types")
        component = target
        expected_directory[component] = is_directory
        while component != _trust_boundary:
            parent = component.parent
            if parent == component:
                fail("Kagemusha protected path escapes its custody boundary")
            prior = expected_directory.get(parent)
            if prior is False:
                fail("Kagemusha protected path descends through a regular file")
            expected_directory[parent] = True
            component = parent

    snapshots: list[KagemushaExternalPathIdentity] = []
    for component in sorted(
        expected_directory,
        key=lambda path: (len(path.parts), str(path)),
    ):
        try:
            info = component.lstat()
        except OSError as error:
            raise DeploymentError(
                f"protected Kagemusha path is unavailable: {component}"
            ) from error
        is_directory = expected_directory[component]
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != _trusted_uid
            or stat.S_IMODE(info.st_mode) & 0o022
            or (is_directory and not stat.S_ISDIR(info.st_mode))
            or (
                not is_directory
                and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1)
            )
        ):
            fail(f"unsafe protected Kagemusha path component: {component}")
        stable = require_acl_free_path(
            component,
            "protected Kagemusha path component",
        )
        if metadata_identity(stable) != metadata_identity(info):
            fail(f"protected Kagemusha path changed during custody check: {component}")
        if component == release_root or component.is_relative_to(release_root):
            snapshots.append(
                KagemushaExternalPathIdentity(
                    path=component,
                    identity=metadata_identity(stable),
                    directory=is_directory,
                )
            )
    return tuple(snapshots)


def _merge_kagemusha_path_identities(
    *groups: Sequence[KagemushaExternalPathIdentity],
) -> tuple[KagemushaExternalPathIdentity, ...]:
    """Merge repeated custody snapshots while rejecting an in-flight change."""

    merged: dict[Path, KagemushaExternalPathIdentity] = {}
    for identity in (item for group in groups for item in group):
        prior = merged.get(identity.path)
        if prior is not None and prior != identity:
            fail(
                "protected Kagemusha path changed during validation: "
                f"{identity.path}"
            )
        merged[identity.path] = identity
    return tuple(merged[path] for path in sorted(merged, key=str))


def _optional_external_lstat(path: Path) -> Optional[os.stat_result]:
    """Return metadata, or ``None`` when an external staging path is unavailable."""
    try:
        return path.lstat()
    except (FileNotFoundError, PermissionError):
        return None

def _optional_bounded_external_digest(
    path: Path, maximum_bytes: int, label: str
) -> Optional[str]:
    """Hash one available canonical nonempty external regular file."""
    info = _optional_external_lstat(path)
    if info is None:
        return None
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or info.st_size <= 0
        or info.st_size > maximum_bytes
    ):
        fail(f"{label} is not one bounded nonempty regular file")
    canonical_path(path, label)
    digest, after = sha256_regular(path, maximum_bytes)
    if metadata_identity(after) != metadata_identity(info):
        fail(f"{label} changed during external release validation")
    return digest

def _inspect_kagemusha_manifest_directories(
    release_root: Path,
    artifact_dir: Path,
) -> tuple[
    tuple[str, ...],
    Optional[str],
    tuple[Path, ...],
    tuple[Path, ...],
    tuple[KagemushaExternalPathIdentity, ...],
]:
    """Bind only bounded manifests and sidecars, never recursive artifact payloads."""
    info = _optional_external_lstat(artifact_dir)
    if info is None:
        return (), None, (), (), ()
    artifact_identities = _capture_root_controlled_kagemusha_paths(
        release_root,
        directories=(artifact_dir,),
    )
    names: list[str] = []
    try:
        with os.scandir(artifact_dir) as entries:
            for entry in entries:
                names.append(entry.name)
                if len(names) > MAX_KAGEMUSHA_CATALOG_RELEASES:
                    fail("Kagemusha artifact root exceeds the 16-release bound")
    except PermissionError:
        return (), None, (), (), artifact_identities
    names.sort()
    if not names:
        fail("available Kagemusha artifact root contains no release directories")

    release_directories: list[Path] = []
    manifest_files: list[Path] = []
    digest_sidecars: list[Path] = []
    for name in names:
        if SHA256_RE.fullmatch(name) is None:
            fail("Kagemusha artifact root contains a noncanonical release directory")
        release_dir = artifact_dir / name
        manifest_path = release_dir / "manifest.norito"
        sidecar_path = release_dir / "manifest.norito.sha256"
        if _optional_external_lstat(manifest_path) is None:
            fail("available Kagemusha release directory lacks manifest.norito")
        if _optional_external_lstat(sidecar_path) is None:
            fail(
                "available Kagemusha release directory lacks a readable digest sidecar"
            )
        release_directories.append(release_dir)
        manifest_files.append(manifest_path)
        digest_sidecars.append(sidecar_path)

    catalog_identities = _capture_root_controlled_kagemusha_paths(
        release_root,
        directories=release_directories,
        files=(*manifest_files, *digest_sidecars),
    )
    for name, manifest_path, sidecar_path in zip(
        names,
        manifest_files,
        digest_sidecars,
    ):
        manifest_sha256 = _optional_bounded_external_digest(
            manifest_path,
            MAX_KAGEMUSHA_MANIFEST_BYTES,
            "Kagemusha canonical release manifest",
        )
        if manifest_sha256 is None:
            fail("protected Kagemusha release manifest became unavailable")
        if manifest_sha256 != name:
            fail("Kagemusha release directory does not equal manifest.norito SHA-256")
        try:
            sidecar, _ = read_regular(
                sidecar_path, MAX_KAGEMUSHA_MANIFEST_DIGEST_SIDECAR_BYTES
            )
        except (FileNotFoundError, PermissionError) as error:
            raise DeploymentError(
                "available Kagemusha release directory lacks a readable digest sidecar"
            ) from error
        if sidecar != f"{name}\n".encode("ascii"):
            fail("Kagemusha manifest digest sidecar is not canonical")

    inventory = {
        "schema": KAGEMUSHA_MANIFEST_DIRECTORY_INVENTORY_SCHEMA,
        "manifest_sha256": names,
    }
    inventory_sha256 = hashlib.sha256(
        taira_authority_client.canonical_json_bytes(inventory)
    ).hexdigest()
    return (
        tuple(names),
        inventory_sha256,
        tuple(manifest_files),
        tuple(digest_sidecars),
        _merge_kagemusha_path_identities(
            artifact_identities,
            catalog_identities,
        ),
    )

def _inspect_kagemusha_external_release(
    projection: Optional[dict[str, object]],
    expected_policy_sha256: Optional[str],
) -> Optional[KagemushaExternalReleasePlan]:
    """Hash bounded external inputs when present; preserve unavailable as blocked."""
    if projection is None:
        return None
    assert expected_policy_sha256 is not None
    release_root = Path(str(projection["release_root"]))
    policy_path = Path(str(projection["release_policy_path"]))
    artifact_dir = Path(str(projection["artifact_dir"]))
    qualification_seal_path = Path(
        str(projection["catalog_qualification_seal_path"])
    )
    protected_identities: tuple[KagemushaExternalPathIdentity, ...] = ()
    root_info = _optional_external_lstat(release_root)
    if root_info is not None:
        protected_identities = _capture_root_controlled_kagemusha_paths(
            release_root,
            directories=(release_root,),
        )
    if _optional_external_lstat(policy_path) is not None:
        protected_identities = _merge_kagemusha_path_identities(
            protected_identities,
            _capture_root_controlled_kagemusha_paths(
                release_root,
                files=(policy_path,),
            ),
        )
    policy_sha256 = _optional_bounded_external_digest(
        policy_path,
        MAX_KAGEMUSHA_RELEASE_POLICY_BYTES,
        "Kagemusha release policy",
    )
    if policy_sha256 is not None and policy_sha256 != expected_policy_sha256:
        fail("Kagemusha release policy differs from the reset-manifest digest")
    if _optional_external_lstat(qualification_seal_path) is not None:
        protected_identities = _merge_kagemusha_path_identities(
            protected_identities,
            _capture_root_controlled_kagemusha_paths(
                release_root,
                files=(qualification_seal_path,),
            ),
        )
    qualification_seal_sha256 = _optional_bounded_external_digest(
        qualification_seal_path,
        MAX_KAGEMUSHA_QUALIFICATION_SEAL_BYTES,
        "Kagemusha catalog qualification seal",
    )
    (
        manifest_digests,
        manifest_inventory_sha256,
        manifest_files,
        manifest_digest_sidecars,
        catalog_identities,
    ) = _inspect_kagemusha_manifest_directories(release_root, artifact_dir)
    protected_identities = _merge_kagemusha_path_identities(
        protected_identities,
        catalog_identities,
    )
    if protected_identities:
        protected_identities = _merge_kagemusha_path_identities(
            protected_identities,
            _capture_root_controlled_kagemusha_paths(
                release_root,
                directories=tuple(
                    identity.path
                    for identity in protected_identities
                    if identity.directory
                ),
                files=tuple(
                    identity.path
                    for identity in protected_identities
                    if not identity.directory
                ),
            ),
        )
    bounded_material_present = (
        policy_sha256 is not None
        and qualification_seal_sha256 is not None
        and manifest_inventory_sha256 is not None
    )
    return KagemushaExternalReleasePlan(
        release_root=release_root,
        policy_path=policy_path,
        artifact_dir=artifact_dir,
        qualification_seal_path=qualification_seal_path,
        expected_policy_sha256=expected_policy_sha256,
        policy_sha256=policy_sha256,
        qualification_seal_sha256=qualification_seal_sha256,
        manifest_directory_digests=manifest_digests,
        manifest_directory_inventory_sha256=manifest_inventory_sha256,
        manifest_files=manifest_files,
        manifest_digest_sidecars=manifest_digest_sidecars,
        protected_path_identities=protected_identities,
        bounded_material_present=bounded_material_present,
    )


def _kagemusha_is_configured(bundle: object) -> bool:
    """Return whether one validated bundle carries the managed projection."""

    return getattr(bundle, "kagemusha_config_projection_sha256", None) is not None


def _kagemusha_bounded_material_present(bundle: object) -> bool:
    """Return whether every bounded external input was protected and captured."""

    external = getattr(bundle, "kagemusha_external_release", None)
    return bool(external and getattr(external, "bounded_material_present", False))


def require_kagemusha_apply_material(bundle: object) -> None:
    """Reject configured apply before authority when external bytes are absent."""

    if _kagemusha_is_configured(bundle) and not _kagemusha_bounded_material_present(
        bundle
    ):
        fail(
            "configured Kagemusha apply requires protected bounded external "
            "release material before authority or receipt consumption"
        )


def require_kagemusha_external_release_unchanged(
    bundle: BundlePlan, *, phase: str
) -> None:
    """Reinspect every bounded external byte and protected identity."""

    external = getattr(bundle, "kagemusha_external_release", None)
    if external is None:
        return
    projection = bundle.manifest.get("kagemusha_config_projection")
    if not isinstance(projection, dict):
        fail(f"Kagemusha config projection disappeared {phase}")
    current = _inspect_kagemusha_external_release(
        projection,
        external.expected_policy_sha256,
    )
    if current != external:
        fail(f"protected Kagemusha external release changed {phase}")


def validate_config_projection(
    config: dict[str, Any],
    bundle: Path,
    *,
    torii_port: int,
    p2p_port: int,
    genesis_public_key: str,
    genesis_expected_hash: str,
    receipt_signer: ReceiptSignerPlan,
    kagemusha_offline_projection: Optional[dict[str, object]] = None,
) -> None:
    """Require exact public-Taira, storage, port, and genesis configuration."""
    if (
        config.get("chain") != CHAIN_ID
        or config.get("chain_discriminant") != CHAIN_DISCRIMINANT
    ):
        fail("validator config does not target canonical public Taira")
    network = config.get("network")
    torii = config.get("torii")
    nexus = config.get("nexus")
    genesis = config.get("genesis")
    if not all(
        isinstance(value, dict)
        for value in (network, torii, nexus, genesis)
    ):
        fail("validator config lacks required network/Torii/Nexus/genesis tables")
    assert isinstance(network, dict) and isinstance(torii, dict)
    assert isinstance(nexus, dict) and isinstance(genesis, dict)
    if address_port(network.get("address"), "network.address") != p2p_port:
        fail(f"validator config P2P port is not exact {p2p_port}")
    if address_port(torii.get("address"), "torii.address") != torii_port:
        fail(f"validator config Torii port is not exact {torii_port}")
    receipt_public_key = torii.get("receipt_public_key")
    receipt_private_key = torii.get("receipt_private_key")
    if (
        not isinstance(receipt_public_key, str)
        or not isinstance(receipt_private_key, str)
    ):
        fail("validator config lacks its explicit Torii receipt keypair")
    try:
        node_id = validator_renderer.validate_receipt_keypair(
            receipt_public_key,
            receipt_private_key,
            "validator config",
        )
    except ValueError as error:
        raise DeploymentError("validator config Torii receipt keypair is invalid") from error
    if (
        receipt_signer.public_key != receipt_public_key
        or receipt_signer.node_id != node_id
    ):
        fail("validator config Torii receipt signer differs from the reset manifest")
    storage = nexus.get("storage")
    if (
        not isinstance(storage, dict)
        or storage.get("local_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or storage.get("disk_budget_weights") != NODE_STORAGE_WEIGHTS
    ):
        fail("validator config lacks the exact bounded 64 GiB storage policy")
    if (
        genesis.get("file") != str(bundle / "genesis.signed.nrt")
        or genesis.get("public_key") != genesis_public_key
        or genesis.get("expected_hash") != genesis_expected_hash
    ):
        fail(
            "validator config does not bind the reset bundle signed genesis, "
            "public key, and exact expected hash"
        )
    settlement = config.get("settlement")
    offline = settlement.get("offline") if isinstance(settlement, dict) else None
    actual_kagemusha = {
        key: offline[key]
        for key in KAGEMUSHA_MANAGED_OFFLINE_FIELDS
        if isinstance(offline, dict) and key in offline
    }
    expected_kagemusha = kagemusha_offline_projection or {}
    if actual_kagemusha != expected_kagemusha:
        fail(
            "validator config managed Kagemusha projection differs from the "
            "reset manifest"
        )

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

def validate_bundle(
    bundle: Path,
    *,
    expected_reset_manifest_sha256: str,
    expected_binary_sha256: str,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
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
        raise DeploymentError(
            "reset bundle owner has no local user/group identity"
        ) from error
    require_exact_names(bundle, TOP_LEVEL_NAMES, "reset bundle")
    bundle_bytes = inspect_private_bundle_tree(bundle, owner_uid, owner_gid)

    manifest_path = bundle / "reset-manifest.json"
    manifest_raw, manifest_info = read_regular(manifest_path, MAX_MANIFEST_BYTES)
    manifest = parse_json_bytes(manifest_raw, "reset manifest")
    manifest_sha256 = hashlib.sha256(manifest_raw).hexdigest()
    if manifest_sha256 != expected_reset_manifest_sha256:
        fail("reset manifest does not match the verified admission receipt")
    if (
        manifest.get("schema") != "taira-exact2f-reset-bundle"
        or manifest.get("peer_count") != PEER_COUNT
        or manifest.get("chain_id") != CHAIN_ID
        or manifest.get("chain_discriminant") != CHAIN_DISCRIMINANT
        or manifest.get("node_storage_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or manifest.get("node_storage_budget_weights") != NODE_STORAGE_WEIGHTS
        or manifest.get("nexus_storage_budget_policy") != NODE_STORAGE_BUDGET_POLICY
    ):
        fail("reset manifest is not the exact bounded Taira v21 projection")
    if manifest.get("source_commit") != expected_source_commit:
        fail("reset manifest source commit does not match verified admission")
    if (
        manifest.get("dpn_validator_release_commit")
        != expected_dpn_validator_release_commit
    ):
        fail("reset manifest DPN release commit does not match verified admission")
    if manifest.get("irohad_sha256") != expected_binary_sha256:
        fail("reset manifest binary does not match the verified admission receipt")
    (
        kagemusha_projection,
        kagemusha_projection_sha256,
        kagemusha_policy_sha256,
    ) = _validate_kagemusha_manifest_projection(manifest, bundle)
    kagemusha_offline_projection = _managed_kagemusha_offline_projection(
        kagemusha_projection
    )
    genesis_public_key = manifest.get("genesis_public_key")
    if (
        not isinstance(genesis_public_key, str)
        or GENESIS_PUBLIC_KEY_RE.fullmatch(genesis_public_key) is None
    ):
        fail("reset manifest lacks one canonical genesis public key")
    genesis_expected_hash = require_genesis_expected_hash(
        manifest.get("genesis_expected_hash")
    )
    _, signed_genesis_info = require_manifest_hash(
        manifest,
        "signed_genesis_sha256",
        bundle / "genesis.signed.nrt",
        64 * 1024 * 1024,
    )
    require_manifest_hash(
        manifest,
        "unsigned_genesis_sha256",
        bundle / "genesis.json",
        64 * 1024 * 1024,
    )
    require_manifest_hash(
        manifest,
        "base_config_sha256",
        bundle / "base-config.toml",
        MAX_CONFIG_BYTES,
    )
    rendered = bundle / "rendered"
    require_exact_names(rendered, {"genesis.json", *SLUGS}, "rendered validator root")
    rendered_genesis_sha, _ = sha256_regular(
        rendered / "genesis.json", 64 * 1024 * 1024
    )
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
    receipt_signers = require_receipt_signer_map(
        manifest.get("receipt_signers"),
        "reset manifest receipt signer map",
    )
    peers: list[PeerPlan] = []
    peer_columns = (SLUGS, LABELS, TORII_PORTS, P2P_PORTS)
    if any(len(column) != PEER_COUNT for column in peer_columns):
        fail("internal Taira peer projection is not exactly four entries")
    for index, (slug, label, torii_port, p2p_port) in enumerate(
        zip(*peer_columns), start=1
    ):
        workdir = rendered / slug
        require_exact_names(workdir, VALIDATOR_NAMES, f"{slug} runtime root")
        workdir_info = require_private_entry(
            workdir, owner_uid, owner_gid, directory=True
        )
        storage = workdir / "storage"
        storage_info = require_private_entry(
            storage, owner_uid, owner_gid, directory=True
        )
        if any(storage.iterdir()):
            fail(f"fresh-reset storage is not empty: {slug}")
        config_path = workdir / "config.toml"
        config_sha, config_info = sha256_regular(config_path, MAX_CONFIG_BYTES)
        expected_config_sha = config_hashes.get(slug)
        if (
            not isinstance(expected_config_sha, str)
            or config_sha != expected_config_sha
        ):
            fail(f"validator config does not match the reset manifest: {slug}")
        config = parse_toml(config_path, owner_uid, owner_gid)
        validate_config_projection(
            config,
            bundle,
            torii_port=torii_port,
            p2p_port=p2p_port,
            genesis_public_key=genesis_public_key,
            genesis_expected_hash=genesis_expected_hash,
            receipt_signer=receipt_signers[index - 1],
            kagemusha_offline_projection=kagemusha_offline_projection,
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
                config_identity=metadata_identity(config_info),
                workdir_identity=metadata_identity(workdir_info),
                storage_identity=metadata_identity(storage_info),
                workdir_device=workdir_info.st_dev,
                workdir_inode=workdir_info.st_ino,
                storage_device=storage_info.st_dev,
                storage_inode=storage_info.st_ino,
            )
        )

    kagemusha_external_release = _inspect_kagemusha_external_release(
        kagemusha_projection,
        kagemusha_policy_sha256,
    )
    free_by_device = require_filesystem_headroom(
        [
            bundle,
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
        manifest_identity=metadata_identity(manifest_info),
        signed_genesis_identity=metadata_identity(signed_genesis_info),
        receipt_signers=receipt_signers,
        peers=tuple(peers),
        bundle_bytes=bundle_bytes,
        free_bytes=free_bytes,
        free_bytes_by_device=free_by_device,
        fsync_latency_ms=max(latencies),
        kagemusha_config_projection_sha256=kagemusha_projection_sha256,
        kagemusha_external_release=kagemusha_external_release,
    )

def require_root_controlled_file(path: Path, *, executable: bool) -> os.stat_result:
    """Require a root-owned path no runtime user can replace or rewrite."""

    canonical_path(path, "root-controlled file")
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != 0
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"root-controlled path has an unsafe component: {component}")
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                fail(
                    f"root-controlled source is not a single-link regular file: {path}"
                )
            if executable and not info.st_mode & 0o111:
                fail(f"root-controlled source is not executable: {path}")
        elif not stat.S_ISDIR(info.st_mode):
            fail(f"root-controlled ancestor is not a directory: {component}")
        require_acl_free_path(component, "root-controlled path component")
    return path.lstat()

def require_system_python_launcher(path: Path) -> os.stat_result:
    """Require the exact immutable macOS launcher, permitting its system hardlinks."""

    if path != DEFAULT_SUPERVISOR_PYTHON:
        fail(f"supervisor Python must be exactly {DEFAULT_SUPERVISOR_PYTHON}")
    canonical_path(path, "supervisor Python")
    components = [*reversed(path.parents), path]
    launcher_info: Optional[os.stat_result] = None
    for index, component in enumerate(components):
        info = component.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != 0
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"system Python path has an unsafe component: {component}")
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode) or info.st_nlink < 1:
                fail("system Python launcher is not a regular file")
            if not info.st_mode & 0o111:
                fail("system Python launcher is not executable")
            launcher_info = info
        elif not stat.S_ISDIR(info.st_mode):
            fail(f"system Python ancestor is not a directory: {component}")
        require_acl_free_path(component, "system Python path component")
    if launcher_info is None:
        fail("system Python launcher identity is unavailable")
    after = path.lstat()
    if metadata_identity(after) != metadata_identity(launcher_info):
        fail("system Python launcher changed during validation")
    return after

@dataclasses.dataclass(frozen=True)
class AdmissionPlan:
    """One complete archive verification bound to immutable deployment bytes."""

    archive: Path
    archive_state: rollout_admission.StableFile
    authority_dir: Path
    authority_state: tuple[tuple[str, rollout_admission.StableFile], ...]
    boi_qualified_handoff: boi_handoff.QualifiedBoiSnapshot
    replay_ledger: Path
    receipt_id: str
    artifact_handoff_sha256: str
    boi_artifact_inventory_sha256: str
    boi_qualified_inventory_sha256: str
    boi_qualification_receipt_id: str
    archive_sha256: str
    privacy_protocol_receipt_id: str
    release_manifest_sha256: str
    source_commit: str
    dpn_validator_release_commit: str
    cargo_lock_sha256: str
    workspace_source_manifest_sha256: str
    reset_manifest_sha256: str
    binary_sha256: str
    supervisor_sha256: str
    validator_config_sha256: tuple[tuple[str, str], ...]
    receipt_signers: tuple[ReceiptSignerPlan, ...]
    restart_generation: str
    signer_fingerprint_sha256: str
    release_manifest_verifier_sha256: str

@dataclasses.dataclass(frozen=True)
class SourcePlan:
    """Authenticated release binary and supervisor source identities."""

    binary: Path
    binary_sha256: str
    supervisor: Path
    supervisor_sha256: str
    python: Path
    python_identity: tuple[int, ...]

def validate_supervisor_python(path: Path) -> tuple[Path, tuple[int, ...]]:
    """Resolve the system launcher to its root-controlled Python.app executable."""

    python = canonical_path(path, "supervisor Python")
    if python != DEFAULT_SUPERVISOR_PYTHON:
        fail(f"supervisor Python must be exactly {DEFAULT_SUPERVISOR_PYTHON}")
    launcher_before = require_system_python_launcher(python)
    try:
        probe = subprocess.run(
            [
                str(python),
                "-I",
                "-S",
                "-c",
                (
                    "import os,sys;"
                    "print('%d.%d.%d' % "
                    "(sys.version_info.major,sys.version_info.minor,"
                    "sys.version_info.micro));"
                    "print(os.fsencode(sys.base_prefix).hex())"
                ),
            ],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=5,
            env={
                "DEVELOPER_DIR": SYSTEM_PYTHON_DEVELOPER_DIR,
                "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            },
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError("supervisor Python version probe failed") from error
    lines = probe.stdout.splitlines()
    if (
        probe.returncode != 0
        or len(lines) != 2
        or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", lines[0]) is None
        or re.fullmatch(r"(?:[0-9a-f]{2})+", lines[1]) is None
        or len(probe.stdout.encode("utf-8")) > 4096
    ):
        fail("supervisor Python did not return one bounded runtime identity")
    version_text, base_prefix_hex = lines
    major, minor, _patch = (int(part) for part in version_text.split("."))
    if major != 3 or minor < 9:
        fail("supervisor Python must be Python >=3.9,<4")
    try:
        base_prefix = Path(os.fsdecode(bytes.fromhex(base_prefix_hex)))
    except (ValueError, TypeError) as error:
        raise DeploymentError("supervisor Python returned an invalid base prefix") from error
    runtime = base_prefix / "Resources/Python.app/Contents/MacOS/Python"
    runtime = canonical_path(runtime, "supervisor Python runtime")
    runtime_before = require_root_controlled_file(runtime, executable=True)
    try:
        direct_probe = subprocess.run(
            [
                str(runtime),
                "-I",
                "-S",
                "-c",
                (
                    "import os,sys;"
                    "print('%d.%d.%d' % "
                    "(sys.version_info.major,sys.version_info.minor,"
                    "sys.version_info.micro));"
                    "print(os.fsencode(sys.executable).hex())"
                ),
            ],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=5,
            env={
                "DEVELOPER_DIR": SYSTEM_PYTHON_DEVELOPER_DIR,
                "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            },
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError("supervisor Python runtime probe failed") from error
    expected_runtime_hex = os.fsencode(runtime).hex()
    if (
        direct_probe.returncode != 0
        or direct_probe.stdout.splitlines() != [version_text, expected_runtime_hex]
        or len(direct_probe.stdout.encode("utf-8")) > 4096
    ):
        fail("supervisor Python runtime did not preserve its exact identity")
    launcher_after = require_system_python_launcher(python)
    runtime_after = require_root_controlled_file(runtime, executable=True)
    if (
        metadata_identity(launcher_before) != metadata_identity(launcher_after)
        or metadata_identity(runtime_before) != metadata_identity(runtime_after)
    ):
        fail("supervisor Python identity changed during validation")
    return runtime, metadata_identity(runtime_after)

def validate_sources(
    args: argparse.Namespace,
    bundle: BundlePlan,
    admission: AdmissionPlan,
) -> SourcePlan:
    """Authenticate root binary and owner-private supervisor without executing either."""

    binary = canonical_path(args.binary, "validator binary")
    require_root_controlled_file(binary, executable=True)
    binary_sha, _ = sha256_regular(binary, MAX_BINARY_BYTES)
    if binary_sha != admission.binary_sha256:
        fail("validator binary does not match the verified admission receipt")

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
    if supervisor_sha != admission.supervisor_sha256:
        fail("supervisor source does not match the verified admission receipt")

    python, python_identity = validate_supervisor_python(args.supervisor_python)
    return SourcePlan(
        binary=binary,
        binary_sha256=binary_sha,
        supervisor=supervisor,
        supervisor_sha256=supervisor_sha,
        python=python,
        python_identity=python_identity,
    )

def _stable_admission_file(
    path: Path, label: str
) -> rollout_admission.StableFile:
    """Capture one verifier-library stable file identity with a local error."""

    try:
        return rollout_admission.stable_hash_path(path)
    except rollout_admission.ReleaseArtifactError as error:
        raise DeploymentError(f"cannot stably read {label}: {error}") from error

def require_protected_replay_ledger(
    path: Path,
) -> rollout_admission.ReplayLedgerSnapshot:
    """Require the sole canonical root-controlled deployment replay ledger."""

    if path != ADMISSION_REPLAY_LEDGER:
        fail("deployment admission must use the canonical replay-ledger path")
    ledger = canonical_path(path, "deployment admission replay ledger")
    info = require_root_controlled_file(ledger, executable=False)
    if (
        info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) != ADMISSION_REPLAY_LEDGER_MODE
    ):
        fail("deployment admission replay ledger must be exact root:wheel 0644")
    try:
        snapshot = rollout_admission.load_replay_ledger(ledger)
    except (
        rollout_admission.ReleaseArtifactError,
        rollout_admission.TairaRolloutAdmissionError,
    ) as error:
        raise DeploymentError(
            f"invalid deployment admission replay ledger: {error}"
        ) from error
    after = require_root_controlled_file(ledger, executable=False)
    if (
        metadata_identity(after) != metadata_identity(info)
        or _stable_admission_file(ledger, "deployment admission replay ledger")
        != snapshot.file
    ):
        fail("deployment admission replay ledger changed during validation")
    return snapshot

def verify_deployment_admission(args: argparse.Namespace) -> AdmissionPlan:
    """Run the archive-only verifier with every independent trust input."""

    archive = canonical_path(args.admission_archive, "admission archive")
    authority_dir = canonical_path(
        args.admission_authority_dir, "admission authority directory"
    )
    verifier = canonical_path(
        args.release_manifest_verifier, "release-manifest verifier"
    )
    ledger = ADMISSION_REPLAY_LEDGER
    require_protected_replay_ledger(ledger)
    before_archive = _stable_admission_file(archive, "admission archive")
    source = rollout_admission.SourceIdentity(
        commit=args.expected_source_commit,
        dpn_validator_release_commit=args.expected_dpn_validator_release_commit,
        cargo_lock_sha256=args.expected_cargo_lock_sha256,
        workspace_source_manifest_sha256=(
            args.expected_workspace_source_manifest_sha256
        ),
    )
    try:
        result = rollout_admission.verify_admission(
            archive_path=archive,
            authority_dir=authority_dir,
            expected_source=source,
            expected_receipt_id=args.expected_receipt_id,
            replay_ledger_path=ledger,
            trusted_signing_fingerprint=args.trusted_signing_fingerprint,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=(
                args.trusted_release_manifest_verifier_sha256
            ),
        )
    except (
        OSError,
        ValueError,
        rollout_admission.ReleaseArtifactError,
        rollout_admission.ReleaseManifestSignatureError,
        rollout_admission.TairaRolloutAdmissionError,
        rollout_admission.tarfile.TarError,
    ) as error:
        raise DeploymentError(
            f"rollout admission verification failed: {error}"
        ) from error
    after_archive = _stable_admission_file(archive, "admission archive")
    if before_archive != after_archive:
        fail("admission archive was substituted around verification")

    expected_fields = {
        "artifact_handoff_sha256",
        "archive_sha256",
        "boi_artifact_inventory_sha256",
        "deployment_performed",
        "linux_authority_manifest_sha256",
        "macos_end_block_hash",
        "macos_end_height",
        "peer_count",
        "privacy_protocol_receipt_id",
        "receipt_id",
        "release_manifest_sha256",
        "release_manifest_verifier_sha256",
        "receipt_signers",
        "reset_manifest_sha256",
        "restart_generation",
        "schema",
        "schema_version",
        "signer_fingerprint_sha256",
        "source",
        "supervisor_sha256",
        "validator_binary_sha256",
        "validator_config_sha256",
        "verified",
    }
    if set(result) != expected_fields:
        fail("rollout admission verifier returned a non-canonical result shape")
    if (
        result["verified"] is not True
        or result["deployment_performed"] is not False
        or result["schema"] != rollout_admission.VERIFICATION_SCHEMA
        or result["schema_version"] != rollout_admission.VERIFICATION_SCHEMA_VERSION
        or result["peer_count"] != PEER_COUNT
        or result["receipt_id"] != args.expected_receipt_id
        or result["artifact_handoff_sha256"]
        != args.expected_artifact_handoff_sha256
        or result["archive_sha256"] != before_archive.sha256
        or result["source"] != source.as_dict()
        or result["signer_fingerprint_sha256"]
        != args.trusted_signing_fingerprint
        or result["release_manifest_verifier_sha256"]
        != args.trusted_release_manifest_verifier_sha256
    ):
        fail("rollout admission verifier result does not match trusted inputs")

    raw_configs = result["validator_config_sha256"]
    if not isinstance(raw_configs, dict) or set(raw_configs) != set(SLUGS):
        fail("rollout admission did not bind the exact four validator configs")
    config_digests = tuple(
        (slug, require_sha256(raw_configs[slug], f"verified {slug} config SHA-256"))
        for slug in SLUGS
    )
    receipt_signers = require_receipt_signer_map(
        result["receipt_signers"],
        "rollout admission receipt signer map",
    )
    try:
        if rollout_admission.scan_inventory_paths(authority_dir) != list(
            rollout_admission.FINAL_AUTHORITY_FILES
        ):
            fail("deployment candidate authority inventory is not exact")
        authority_state = tuple(
            (
                relative,
                rollout_admission.stable_hash_relative(authority_dir, relative),
            )
            for relative in rollout_admission.FINAL_AUTHORITY_FILES
        )
        boi_snapshot = boi_handoff.verify_qualified_boi_handoff(
            Path(args.boi_qualified_handoff_root),
            candidate_archive=archive,
            candidate_authority_dir=authority_dir,
            expected_source=source,
            expected_receipt_id=args.expected_receipt_id,
            replay_ledger_path=ledger,
            trusted_signing_fingerprint=args.trusted_signing_fingerprint,
            trusted_qualification_public_key_path=(
                args.trusted_boi_qualification_public_key
            ),
            trusted_qualification_signing_fingerprint=(
                args.trusted_boi_qualification_signing_fingerprint
            ),
            expected_qualification_host_id=(
                args.expected_boi_qualification_host_id
            ),
            expected_qualification_installation_id=(
                args.expected_boi_qualification_installation_id
            ),
            expected_controller_closure_digest=(
                args.expected_boi_qualification_controller_digest
            ),
            expected_workflow_run_id=args.expected_workflow_run_id,
            expected_workflow_run_attempt=args.expected_workflow_run_attempt,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=(
                args.trusted_release_manifest_verifier_sha256
            ),
        )
    except (
        OSError,
        rollout_admission.ReleaseArtifactError,
        boi_handoff.BoiHandoffError,
    ) as error:
        raise DeploymentError(
            f"qualified BOI handoff verification failed: {error}"
        ) from error
    if (
        boi_snapshot.candidate_archive_sha256 != result["archive_sha256"]
        or boi_snapshot.candidate_boi_artifact_inventory_sha256
        != result["boi_artifact_inventory_sha256"]
        or boi_snapshot.candidate_release_manifest_sha256
        != result["release_manifest_sha256"]
        or boi_snapshot.source != result["source"]
    ):
        fail("qualified BOI handoff differs from the exact signed admission")
    return AdmissionPlan(
        archive=archive,
        archive_state=before_archive,
        authority_dir=authority_dir,
        authority_state=authority_state,
        boi_qualified_handoff=boi_snapshot,
        replay_ledger=ledger,
        receipt_id=require_sha256(result["receipt_id"], "verified receipt ID"),
        artifact_handoff_sha256=require_sha256(
            result["artifact_handoff_sha256"],
            "verified macOS build handoff SHA-256",
        ),
        boi_artifact_inventory_sha256=require_sha256(
            result["boi_artifact_inventory_sha256"],
            "verified BOI artifact inventory SHA-256",
        ),
        boi_qualified_inventory_sha256=require_sha256(
            boi_snapshot.boi_inventory_sha256,
            "verified qualified BOI inventory SHA-256",
        ),
        boi_qualification_receipt_id=require_sha256(
            boi_snapshot.qualification_receipt_id,
            "verified signed BOI qualification receipt ID",
        ),
        archive_sha256=require_sha256(
            result["archive_sha256"], "verified archive SHA-256"
        ),
        privacy_protocol_receipt_id=require_sha256(
            result["privacy_protocol_receipt_id"],
            "verified privacy protocol receipt ID",
        ),
        release_manifest_sha256=require_sha256(
            result["release_manifest_sha256"],
            "verified candidate release manifest SHA-256",
        ),
        source_commit=args.expected_source_commit,
        dpn_validator_release_commit=args.expected_dpn_validator_release_commit,
        cargo_lock_sha256=args.expected_cargo_lock_sha256,
        workspace_source_manifest_sha256=(
            args.expected_workspace_source_manifest_sha256
        ),
        reset_manifest_sha256=require_sha256(
            result["reset_manifest_sha256"],
            "verified reset manifest SHA-256",
        ),
        binary_sha256=require_sha256(
            result["validator_binary_sha256"],
            "verified validator binary SHA-256",
        ),
        supervisor_sha256=require_sha256(
            result["supervisor_sha256"], "verified supervisor SHA-256"
        ),
        validator_config_sha256=config_digests,
        receipt_signers=receipt_signers,
        restart_generation=require_sha256(
            result["restart_generation"], "verified restart generation"
        ),
        signer_fingerprint_sha256=args.trusted_signing_fingerprint,
        release_manifest_verifier_sha256=(
            args.trusted_release_manifest_verifier_sha256
        ),
    )

def require_admission_archive_unchanged(admission: AdmissionPlan) -> None:
    """Reject candidate or qualified-BOI changes after successful verification."""

    if (
        _stable_admission_file(admission.archive, "admission archive")
        != admission.archive_state
    ):
        fail("verified admission archive was substituted before rollout")
    try:
        if rollout_admission.scan_inventory_paths(admission.authority_dir) != list(
            rollout_admission.FINAL_AUTHORITY_FILES
        ):
            fail("verified admission authority inventory changed before rollout")
        for relative, before in admission.authority_state:
            if (
                rollout_admission.stable_hash_relative(
                    admission.authority_dir, relative
                )
                != before
            ):
                fail("verified admission authority changed before rollout")
        boi_handoff.recheck_qualified_boi_handoff(
            admission.boi_qualified_handoff
        )
    except (
        rollout_admission.ReleaseArtifactError,
        boi_handoff.BoiHandoffError,
    ) as error:
        raise DeploymentError(
            f"verified BOI admission evidence changed before rollout: {error}"
        ) from error

def require_inputs_match_admission(
    bundle: BundlePlan,
    sources: SourcePlan,
    admission: AdmissionPlan,
) -> None:
    """Bind every deployable byte identity to the verified signed receipt."""

    # The signed receipt attests the secret-free qualification topology.  A
    # production reset is separately authenticated by its protected source and
    # may intentionally contain different keys and endpoint configuration.
    if (
        sources.binary_sha256 != admission.binary_sha256
        or sources.supervisor_sha256 != admission.supervisor_sha256
        or bundle.manifest.get("source_commit") != admission.source_commit
        or bundle.manifest.get("dpn_validator_release_commit")
        != admission.dpn_validator_release_commit
        or bundle.receipt_signers != admission.receipt_signers
    ):
        fail(
            "deployment executable or receipt-signer inputs do not match the "
            "verified qualification receipt"
        )

def require_admission_bound_inputs_unchanged(
    bundle: BundlePlan,
    sources: SourcePlan,
    admission: AdmissionPlan,
) -> None:
    """Recheck receipt-bound mutable sources under the deployment lock."""

    require_bundle_runtime_unchanged(bundle)
    binary_sha256, _ = sha256_regular(sources.binary, MAX_BINARY_BYTES)
    supervisor_sha256, _ = sha256_regular(sources.supervisor, 4 * 1024 * 1024)
    python_info = require_root_controlled_file(sources.python, executable=True)
    if (
        binary_sha256 != admission.binary_sha256
        or supervisor_sha256 != admission.supervisor_sha256
        or metadata_identity(python_info) != sources.python_identity
    ):
        fail("receipt-bound binary, supervisor, or Python changed after admission")
    require_inputs_match_admission(bundle, sources, admission)

@dataclasses.dataclass(frozen=True)
class ProcessInfo:
    """Redaction-safe process identity used for exact parent/owner checks."""

    pid: int
    ppid: int
    uid: int
    argv: tuple[str, ...]

def parse_darwin_procargs2(raw: bytes) -> tuple[str, ...]:
    """Parse one bounded ``KERN_PROCARGS2`` payload into its exact argv."""

    if len(raw) < ctypes.sizeof(ctypes.c_int) or len(raw) > MAX_PROCESS_ARGUMENT_BYTES:
        fail("managed process native argument payload has an invalid size")
    argc = ctypes.c_int.from_buffer_copy(raw[: ctypes.sizeof(ctypes.c_int)]).value
    if argc < 1 or argc > MAX_PROCESS_ARGUMENTS:
        fail("managed process native argument count is outside its bound")
    cursor = ctypes.sizeof(ctypes.c_int)
    executable_end = raw.find(b"\0", cursor)
    if executable_end <= cursor:
        fail("managed process native executable path is incomplete")
    try:
        executable = raw[cursor:executable_end].decode("utf-8")
    except UnicodeDecodeError as error:
        raise DeploymentError(
            "managed process native executable path is not UTF-8"
        ) from error
    cursor = executable_end + 1
    while cursor < len(raw) and raw[cursor] == 0:
        cursor += 1
    arguments: list[str] = []
    for _index in range(argc):
        argument_end = raw.find(b"\0", cursor)
        if argument_end < cursor:
            fail("managed process native argument vector is incomplete")
        argument_raw = raw[cursor:argument_end]
        if not argument_raw:
            fail("managed process native argument vector contains an empty argument")
        try:
            arguments.append(argument_raw.decode("utf-8"))
        except UnicodeDecodeError as error:
            raise DeploymentError(
                "managed process native argument vector is not UTF-8"
            ) from error
        cursor = argument_end + 1
    argv = tuple(arguments)
    if argv[0] != executable:
        fail("managed process native executable path differs from argv[0]")
    return argv

def read_darwin_process_argv(pid: int) -> tuple[str, ...]:
    """Read one process argv through bounded, NUL-delimited Darwin sysctl data."""

    if sys.platform != "darwin" or pid <= 1:
        fail(f"native managed process argument inspection is unavailable: pid {pid}")
    libc = ctypes.CDLL(None, use_errno=True)
    sysctl = libc.sysctl
    sysctl.argtypes = (
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_uint,
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.c_void_p,
        ctypes.c_size_t,
    )
    sysctl.restype = ctypes.c_int
    mib = (ctypes.c_int * 3)(DARWIN_CTL_KERN, DARWIN_KERN_PROCARGS2, pid)
    size = ctypes.c_size_t()
    if sysctl(mib, 3, None, ctypes.byref(size), None, 0) != 0:
        fail(f"could not size native managed process arguments: pid {pid}")
    if size.value < ctypes.sizeof(ctypes.c_int) or size.value > MAX_PROCESS_ARGUMENT_BYTES:
        fail(f"native managed process argument buffer is outside its bound: pid {pid}")
    buffer = ctypes.create_string_buffer(size.value)
    actual = ctypes.c_size_t(size.value)
    if (
        sysctl(
            mib,
            3,
            ctypes.cast(buffer, ctypes.c_void_p),
            ctypes.byref(actual),
            None,
            0,
        )
        != 0
    ):
        fail(f"could not read native managed process arguments: pid {pid}")
    if actual.value > size.value:
        fail(f"native managed process argument payload grew during capture: pid {pid}")
    return parse_darwin_procargs2(buffer.raw[: actual.value])

@dataclasses.dataclass(frozen=True)
class RestartLogCursor:
    """Bounded append-only cursor for one authenticated supervisor log inode."""

    path: Path
    identity: tuple[int, ...]
    offset: int
    guard_offset: int
    guard_sha256: str

@dataclasses.dataclass(frozen=True)
class OldManagedIdentity:
    """Expected supervisor and child command identity for one rollback job."""

    supervisor_uid: int
    supervisor_argv: tuple[str, ...]
    child_uid: int
    child_argv: tuple[str, ...]
    pid_file: Path
    pid_file_gid: int
    child_was_present: bool

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

    def launchd_print(self, label: str) -> Optional[str]:
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
            fail(
                f"launchd bootstrap failed for {plist.name} (status {result.returncode})"
            )

    def inspect_process(self, pid: int) -> ProcessInfo:
        """Read a stable parent, uid, and native argv for one macOS process."""

        def numeric_identity() -> tuple[int, int]:
            result = self.run(
                [
                    "/bin/ps",
                    "-p",
                    str(pid),
                    "-o",
                    "ppid=",
                    "-o",
                    "uid=",
                ]
            )
            if result.returncode != 0 or not result.stdout.strip():
                fail(f"managed process is not running: pid {pid}")
            fields = result.stdout.split()
            if len(fields) != 2:
                fail(f"could not parse managed process identity: pid {pid}")
            try:
                ppid, uid = (int(field) for field in fields)
            except (ValueError, TypeError) as error:
                raise DeploymentError(
                    f"could not parse managed process identity: pid {pid}"
                ) from error
            if ppid < 0 or uid < 0:
                fail(f"managed process identity is invalid: pid {pid}")
            return ppid, uid

        before = numeric_identity()
        argv_before = read_darwin_process_argv(pid)
        argv_after = read_darwin_process_argv(pid)
        after = numeric_identity()
        if before != after or argv_before != argv_after:
            fail(f"managed process identity changed during capture: pid {pid}")
        return ProcessInfo(pid=pid, ppid=before[0], uid=before[1], argv=argv_before)

    def child_pids(self, parent_pid: int) -> tuple[int, ...]:
        """Return the stable PID inventory currently parented by one process."""

        result = self.run(["/bin/ps", "-axo", "pid=", "-o", "ppid="])
        if result.returncode != 0:
            fail(f"could not inspect children of managed process: pid {parent_pid}")
        children: list[int] = []
        for line in result.stdout.splitlines():
            fields = line.split()
            if len(fields) != 2:
                fail("could not parse the managed process child inventory")
            try:
                pid, ppid = (int(field) for field in fields)
            except ValueError as error:
                raise DeploymentError(
                    "could not parse the managed process child inventory"
                ) from error
            if pid < 1 or ppid < 0:
                fail("managed process child inventory contains an invalid PID")
            if ppid == parent_pid:
                children.append(pid)
        return tuple(sorted(children))

    def terminate(self, pid: int) -> None:
        """Terminate exactly one already-authenticated managed child."""

        os.kill(pid, signal.SIGTERM)

    def process_exists(self, pid: int) -> bool:
        """Return whether a process still exists."""

        result = self.run(["/bin/ps", "-p", str(pid), "-o", "pid="])
        return result.returncode == 0 and bool(result.stdout.strip())

def launchd_pid(record: Optional[str], label: str) -> int:
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
    *,
    allow_absent_child: bool = False,
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
        raise DeploymentError(
            f"old LaunchDaemon runtime identity is unknown: {label}"
        ) from error
    plist_supervisor_argv = tuple(arguments)
    supervisor = ops.inspect_process(supervisor_pid)
    if (
        supervisor.ppid != 1
        or supervisor.uid != uid
        or supervisor.argv != plist_supervisor_argv
    ):
        fail(f"old LaunchDaemon supervisor identity differs from its plist: {label}")
    pid_file = Path(required_option(plist_supervisor_argv, "--pid-file", label))
    binary = required_option(plist_supervisor_argv, "--binary", label)
    config = required_option(plist_supervisor_argv, "--config", label)
    if (
        not pid_file.is_absolute()
        or not Path(binary).is_absolute()
        or not Path(config).is_absolute()
    ):
        fail(f"old LaunchDaemon contains a non-absolute managed path: {label}")
    child_argv = (binary, "--sora", "--config", config)
    child_was_present = verify_old_managed_child(
        label,
        supervisor_pid,
        uid,
        gid,
        pid_file,
        child_argv,
        ops,
        allow_absent=allow_absent_child,
    )
    return OldManagedIdentity(
        supervisor_uid=uid,
        supervisor_argv=supervisor.argv,
        child_uid=uid,
        child_argv=child_argv,
        pid_file=pid_file,
        pid_file_gid=gid,
        child_was_present=child_was_present,
    )

def verify_old_managed_child(
    label: str,
    supervisor_pid: int,
    uid: int,
    gid: int,
    pid_file: Path,
    child_argv: tuple[str, ...],
    ops: SystemOps,
    *,
    allow_absent: bool,
) -> bool:
    """Authenticate one old child or an explicitly allowed childless supervisor."""

    try:
        pid_file.lstat()
    except FileNotFoundError:
        if not allow_absent:
            fail(f"old LaunchDaemon managed-child PID file is absent: {label}")
        for _sample in range(2):
            try:
                pid_file.lstat()
            except FileNotFoundError:
                pass
            else:
                fail(
                    "old LaunchDaemon managed-child PID file appeared during "
                    f"capture: {label}"
                )
            if ops.child_pids(supervisor_pid):
                fail(
                    "old LaunchDaemon PID file is absent while its supervisor "
                    f"still owns a child: {label}"
                )
            try:
                pid_file.lstat()
            except FileNotFoundError:
                continue
            fail(
                "old LaunchDaemon managed-child PID file appeared during "
                f"capture: {label}"
            )
        return False

    child_pid = parse_pid_file(pid_file, uid, gid)
    child = ops.inspect_process(child_pid)
    if child.ppid != supervisor_pid or child.uid != uid or child.argv != child_argv:
        fail(f"old LaunchDaemon child identity differs from its supervisor: {label}")
    if ops.child_pids(supervisor_pid) != (child_pid,):
        fail(
            f"old LaunchDaemon supervisor does not own exactly its managed child: {label}"
        )
    return True

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
    verify_old_managed_child(
        label,
        supervisor_pid,
        expected.child_uid,
        expected.pid_file_gid,
        expected.pid_file,
        expected.child_argv,
        ops,
        allow_absent=not expected.child_was_present,
    )

def capture_old_cohort(
    ops: SystemOps,
    *,
    allow_absent_child: bool = False,
) -> tuple[PlistSnapshot, ...]:
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
            raise DeploymentError(
                f"old LaunchDaemon plist is invalid: {path}"
            ) from error
        if not isinstance(payload, dict) or payload.get("Label") != label:
            fail(f"old LaunchDaemon plist label mismatch: {path}")
        supervisor_pid = launchd_pid(ops.launchd_print(label), label)
        managed = inspect_old_managed_identity(
            payload,
            label,
            supervisor_pid,
            ops,
            allow_absent_child=allow_absent_child,
        )
        if launchd_pid(ops.launchd_print(label), label) != supervisor_pid:
            fail(f"old LaunchDaemon supervisor changed during capture: {label}")
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
        require_acl_free_path(component, "root installation ancestor")
    for component in reversed(pending):
        os.mkdir(component, mode)
        os.chown(component, 0, 0)
        os.chmod(component, mode)
        require_acl_free_path(component, "new root installation directory")
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
    require_acl_free_path(path, "root installation directory")

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

def install_immutable(
    source: Path, destination: Path, expected_sha256: str
) -> os.stat_result:
    """Install authenticated bytes as root:wheel 0555 without overwriting."""

    if destination.exists() or destination.is_symlink():
        require_root_controlled_file(destination, executable=True)
        actual, info = sha256_regular(destination, MAX_BINARY_BYTES)
        if (
            actual != expected_sha256
            or stat.S_IMODE(info.st_mode) != 0o555
            or info.st_gid != 0
        ):
            fail(
                f"existing immutable installation has the wrong identity: {destination}"
            )
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
        clear_owned_temporary_acl(
            temporary, output_fd, "immutable installation staging file"
        )
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
    published = require_root_controlled_file(destination, executable=True)
    if metadata_identity(published) != metadata_identity(info):
        fail(
            f"immutable installation identity changed after publication: {destination}"
        )
    return published

def atomic_replace_owned(
    path: Path, body: bytes, *, mode: int, uid: int, gid: int
) -> None:
    """Atomically replace a root-controlled plist with authenticated bytes."""

    if path.is_symlink():
        fail(f"refusing to replace symlink: {path}")
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    descriptor = -1
    temporary_created = False
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            mode,
        )
        temporary_created = True
        try:
            offset = 0
            while offset < len(body):
                offset += os.write(descriptor, body[offset:])
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, mode)
            os.fsync(descriptor)
            clear_owned_temporary_acl(
                temporary,
                descriptor,
                "root-controlled publication staging file",
            )
        finally:
            os.close(descriptor)
            descriptor = -1
        os.replace(temporary, path)
        fsync_directory(path.parent)
        require_root_controlled_file(path, executable=False)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if temporary_created:
            temporary.unlink(missing_ok=True)

@dataclasses.dataclass
class AdmissionReceiptConsumption:
    """One protected receipt publication awaiting the irreversible cutover."""

    admission: AdmissionPlan
    prior_payload: bytes
    consumed_payload: bytes
    rollout_started: bool = False

    def mark_rollout_started(self) -> None:
        """Commit replay consumption immediately before the first cohort change."""

        if self.rollout_started:
            fail("deployment rollout start was marked more than once")
        require_admission_archive_unchanged(self.admission)
        published = require_protected_replay_ledger(self.admission.replay_ledger)
        if published.payload != self.consumed_payload:
            fail("consumed admission receipt changed before rollout start")
        self.rollout_started = True

def _restore_unstarted_receipt_consumption(
    transaction: AdmissionReceiptConsumption,
) -> None:
    """Restore the exact prior ledger if this transaction still owns its update."""

    current = require_protected_replay_ledger(transaction.admission.replay_ledger)
    if current.payload == transaction.prior_payload:
        return
    if current.payload != transaction.consumed_payload:
        fail(
            "admission replay ledger changed outside this unstarted deployment; "
            "automatic restoration is unsafe"
        )
    atomic_replace_owned(
        transaction.admission.replay_ledger,
        transaction.prior_payload,
        mode=ADMISSION_REPLAY_LEDGER_MODE,
        uid=0,
        gid=0,
    )
    restored = require_protected_replay_ledger(transaction.admission.replay_ledger)
    if restored.payload != transaction.prior_payload:
        fail("admission replay ledger restoration did not persist exact prior bytes")

@contextlib.contextmanager
def consume_admission_receipt(
    admission: AdmissionPlan,
) -> Any:
    """Atomically consume one receipt, restoring it until rollout begins."""

    require_admission_archive_unchanged(admission)
    prior = require_protected_replay_ledger(admission.replay_ledger)
    receipt_ids = (
        admission.receipt_id,
        admission.boi_qualification_receipt_id,
    )
    if len(set(receipt_ids)) != 2:
        fail("candidate and BOI qualification replay identities collide")
    if any(item in prior.consumed_receipt_ids for item in receipt_ids):
        fail(
            "verified candidate or BOI qualification receipt was already consumed "
            "under deployment lock"
        )
    consumed_ids = sorted((*prior.consumed_receipt_ids, *receipt_ids))
    try:
        consumed_payload = rollout_admission.canonical_replay_ledger_bytes(
            consumed_ids
        )
    except rollout_admission.TairaRolloutAdmissionError as error:
        raise DeploymentError(
            f"cannot encode admission replay ledger: {error}"
        ) from error
    if len(consumed_payload) > rollout_admission.MAX_JSON_BYTES:
        fail("admission replay ledger has no capacity for another receipt")
    transaction = AdmissionReceiptConsumption(
        admission=admission,
        prior_payload=prior.payload,
        consumed_payload=consumed_payload,
    )
    try:
        atomic_replace_owned(
            admission.replay_ledger,
            consumed_payload,
            mode=ADMISSION_REPLAY_LEDGER_MODE,
            uid=0,
            gid=0,
        )
        published = require_protected_replay_ledger(admission.replay_ledger)
        if (
            published.payload != consumed_payload
            or published.consumed_receipt_ids != tuple(consumed_ids)
        ):
            fail("admission receipt consumption was not published atomically")
        yield transaction
        if not transaction.rollout_started:
            fail("deployment returned without beginning its admitted rollout")
        committed = require_protected_replay_ledger(admission.replay_ledger)
        if committed.payload != consumed_payload:
            fail("consumed admission receipt changed during rollout")
    except BaseException as deployment_error:
        if not transaction.rollout_started:
            try:
                _restore_unstarted_receipt_consumption(transaction)
            except BaseException as rollback_error:
                combined = DeploymentError(
                    "deployment did not begin and admission receipt rollback failed"
                )
                _mark_deployment_outcome(combined, "rollback-failed")
                if hasattr(combined, "add_note"):
                    combined.add_note(
                        "deployment failure: "
                        f"{type(deployment_error).__name__}: {deployment_error}"
                    )
                raise combined from rollback_error
        raise

def render_plist(
    peer: PeerPlan,
    bundle: BundlePlan,
    sources: SourcePlan,
    *,
    installed_binary: Path,
    binary_info: os.stat_result,
    installed_supervisor: Path,
    runtime_root: Path,
    restart_generation: str,
    lifecycle_journal_root: Path,
    authenticated_node_id: str,
) -> bytes:
    """Render one fresh LaunchDaemon bound to authenticated lifecycle identity."""

    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    expected_lifecycle_root = runtime_root / "lifecycle" / peer.slug
    if (
        not runtime_root.is_absolute()
        or ".." in runtime_root.parts
        or lifecycle_journal_root != expected_lifecycle_root
        or LIFECYCLE_NODE_ID_RE.fullmatch(authenticated_node_id) is None
    ):
        fail("lifecycle journal path or authenticated node ID is not canonical")
    terminal_binding = supervisor_terminal_binding(
        sources.binary_sha256,
        binary_info,
        peer.config_sha256,
        restart_generation,
    )
    terminal_file = terminal_unhealthy_path(runtime_root, peer, terminal_binding)
    log_file = runtime_root / "logs" / f"validator-{peer.number}-supervisor.log"
    arguments = [
        str(sources.python),
        "-I",
        "-S",
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
        "--terminal-unhealthy-file",
        str(terminal_file),
        "--restart-generation",
        restart_generation,
        "--lifecycle-journal-root",
        str(lifecycle_journal_root),
        "--validator-id",
        peer.slug,
        "--node-id",
        authenticated_node_id,
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

def require_mutable_bundle_identities(bundle: BundlePlan, *, phase: str) -> None:
    """Recheck every mutable bundle and bounded external release identity."""

    manifest_raw, manifest_info = read_regular(
        bundle.root / "reset-manifest.json", MAX_MANIFEST_BYTES
    )
    if hashlib.sha256(manifest_raw).hexdigest() != bundle.manifest_sha256:
        fail(f"reset manifest changed {phase}")
    if metadata_identity(manifest_info) != bundle.manifest_identity:
        fail(f"reset manifest identity changed {phase}")
    signed_sha, signed_info = sha256_regular(
        bundle.root / "genesis.signed.nrt", 64 * 1024 * 1024
    )
    if signed_sha != bundle.manifest.get("signed_genesis_sha256"):
        fail(f"signed genesis changed {phase}")
    if metadata_identity(signed_info) != bundle.signed_genesis_identity:
        fail(f"signed genesis identity changed {phase}")
    for peer in bundle.peers:
        require_exact_names(peer.workdir, VALIDATOR_NAMES, f"{peer.slug} runtime root")
        workdir_info = require_private_entry(
            peer.workdir, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        storage_info = require_private_entry(
            peer.storage, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        config_sha, config_info = sha256_regular(peer.config, MAX_CONFIG_BYTES)
        if config_sha != peer.config_sha256:
            fail(f"validator config changed {phase}: {peer.slug}")
        if metadata_identity(config_info) != peer.config_identity:
            fail(f"validator config identity changed {phase}: {peer.slug}")
        if any(peer.storage.iterdir()):
            fail(f"fresh-reset storage changed {phase}: {peer.slug}")
        workdir_after = require_private_entry(
            peer.workdir, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        storage_after = require_private_entry(
            peer.storage, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        if (
            metadata_identity(workdir_info) != peer.workdir_identity
            or metadata_identity(workdir_after) != peer.workdir_identity
            or metadata_identity(storage_info) != peer.storage_identity
            or metadata_identity(storage_after) != peer.storage_identity
        ):
            fail(f"fresh-reset runtime path changed {phase}: {peer.slug}")
    require_kagemusha_external_release_unchanged(bundle, phase=phase)

def require_bundle_runtime_unchanged(bundle: BundlePlan) -> None:
    """Recheck all mutable bundle inputs after preflight."""

    require_mutable_bundle_identities(bundle, phase="after preflight")

def _drop_config_check_privileges(uid: int, gid: int) -> Callable[[], None]:
    """Return the sole child setup permitted for hostile binary config checks."""

    if uid <= 0 or gid <= 0:
        fail("binary config validation requires a non-root runtime identity")

    def drop() -> None:
        os.setgroups([])
        os.setgid(gid)
        os.setuid(uid)
        os.umask(0o077)

    return drop

def validate_installed_peer_configs(
    installed_binary: Path,
    bundle: BundlePlan,
    *,
    runner: Callable[..., Any] = subprocess.run,
) -> None:
    """Have the exact installed binary validate every config and its inputs."""

    if len(bundle.peers) != PEER_COUNT:
        fail("binary config validation requires exactly four peer configs")
    for peer in bundle.peers:
        try:
            result = runner(
                [
                    str(installed_binary),
                    "--sora",
                    "--config",
                    str(peer.config),
                    "--check-config",
                ],
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=CONFIG_CHECK_TIMEOUT_SECONDS,
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                preexec_fn=_drop_config_check_privileges(
                    bundle.owner_uid,
                    bundle.owner_gid,
                ),
            )
        except subprocess.TimeoutExpired as error:
            raise DeploymentError(
                f"installed binary config check timed out: {peer.slug}"
            ) from error
        except OSError as error:
            raise DeploymentError(
                f"installed binary config check could not execute: {peer.slug}"
            ) from error
        if result.returncode != 0:
            fail(
                "installed binary rejected validator config/genesis/catalog "
                f"(peer={peer.slug}, status={result.returncode})"
            )


def validate_dry_run_kagemusha_exact_config(
    sources: SourcePlan,
    bundle: BundlePlan,
    *,
    checker: Callable[[Path, BundlePlan], None] = validate_installed_peer_configs,
) -> bool:
    """Run semantic checks only through an exact final-path installed candidate."""

    if not _kagemusha_is_configured(bundle):
        return False
    if not _kagemusha_bounded_material_present(bundle):
        return False
    installed_binary = (
        INSTALL_ROOT / "binaries" / sources.binary_sha256 / "iroha3d"
    )
    if _optional_external_lstat(installed_binary) is None:
        return False
    installed_info = require_root_controlled_file(installed_binary, executable=True)
    installed_sha256, hashed_info = sha256_regular(
        installed_binary,
        MAX_BINARY_BYTES,
    )
    if (
        installed_sha256 != sources.binary_sha256
        or metadata_identity(installed_info) != metadata_identity(hashed_info)
    ):
        fail("installed dry-run candidate does not match the admitted binary")
    require_mutable_bundle_identities(
        bundle,
        phase="before exact dry-run config validation",
    )
    checker(installed_binary, bundle)
    require_mutable_bundle_identities(
        bundle,
        phase="after exact dry-run config validation",
    )
    return True


deploy_health.configure_runtime(
    deployment_error=DeploymentError,
    fail_callback=fail,
    parse_json=parse_json_bytes,
    load_operator_context=(
        lambda network_id, private_key_file: load_operator_context_from_file(
            network_id, private_key_file
        )
    ),
    require_acl_free=require_acl_free_path,
    metadata_identity_callback=metadata_identity,
    require_lifecycle_node_ids=(
        lambda bundle: require_authenticated_lifecycle_node_ids(bundle)
    ),
    receipt_signer_map=receipt_signer_public_map,
    max_http_bytes=MAX_HTTP_BYTES,
    max_terminal_unhealthy_bytes=MAX_TERMINAL_UNHEALTHY_BYTES,
    block_hash_re=BLOCK_HASH_RE,
    commit_re=COMMIT_RE,
    sha256_re=SHA256_RE,
    lifecycle_node_id_re=LIFECYCLE_NODE_ID_RE,
    peer_count=PEER_COUNT,
    slugs=SLUGS,
    lane_count=TAIRA_LANE_COUNT,
    lane_dataspace_bindings=TAIRA_LANE_DATASPACE_BINDINGS,
    physical_dataspaces=TAIRA_PHYSICAL_DATASPACES,
    terminal_unhealthy_schema=TERMINAL_UNHEALTHY_SCHEMA,
    lifecycle_state_schema=LIFECYCLE_STATE_SCHEMA,
    lifecycle_binding_domain=LIFECYCLE_BINDING_DOMAIN,
)

http_json = deploy_health.http_json
http_ok = deploy_health.http_ok
_RejectRedirects = deploy_health._RejectRedirects
build_operator_http_getter = deploy_health.build_operator_http_getter
require_uint = deploy_health.require_uint
normalized_block_hash = deploy_health.normalized_block_hash
nested = deploy_health.nested
tagged_unit = deploy_health.tagged_unit
published_source_commit = deploy_health.published_source_commit
published_dpn_validator_release_commit = deploy_health.published_dpn_validator_release_commit
PeerSample = deploy_health.PeerSample
FleetSample = deploy_health.FleetSample
RestartProofResult = deploy_health.RestartProofResult
HttpGetter = deploy_health.HttpGetter
HealthGetter = deploy_health.HealthGetter
TerminalChecker = deploy_health.TerminalChecker
no_terminal_check = deploy_health.no_terminal_check
deployment_completed_at_unix_ms = deploy_health.deployment_completed_at_unix_ms
deployed_config_set_sha256 = deploy_health.deployed_config_set_sha256
deployed_topology_sha256 = deploy_health.deployed_topology_sha256
supervisor_terminal_binding = deploy_health.supervisor_terminal_binding
supervisor_lifecycle_binding = deploy_health.supervisor_lifecycle_binding
deployed_receipt_signer_map = deploy_health.deployed_receipt_signer_map
terminal_unhealthy_path = deploy_health.terminal_unhealthy_path
require_terminal_marker = deploy_health.require_terminal_marker
require_no_terminal_unhealthy = deploy_health.require_no_terminal_unhealthy
validate_peer_health = deploy_health.validate_peer_health
capture_fleet = deploy_health.capture_fleet
wait_for_fleet_sample = deploy_health.wait_for_fleet_sample
wait_for_advancement = deploy_health.wait_for_advancement

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
    if not isinstance(arguments, list) or not all(
        isinstance(value, str) for value in arguments
    ):
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
    if supervisor.ppid != 1 or supervisor.uid != bundle.owner_uid:
        fail(f"{peer.label} supervisor has an unexpected owner")
    if supervisor.argv != expected_supervisor_argv(plist_body):
        fail(f"{peer.label} supervisor command differs from the generated plist")
    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    child_pid = parse_pid_file(pid_file, bundle.owner_uid, bundle.owner_gid)
    child = ops.inspect_process(child_pid)
    expected_child = (str(installed_binary), "--sora", "--config", str(peer.config))
    if (
        child.ppid != supervisor_pid
        or child.uid != bundle.owner_uid
        or child.argv != expected_child
    ):
        fail(f"{peer.label} PID file does not name its exact managed validator child")
    return supervisor_pid, child_pid

def _open_growing_regular(path: Path) -> tuple[int, os.stat_result]:
    """Open one single-link growing regular file without following aliases."""

    try:
        before = path.lstat()
    except OSError as error:
        raise DeploymentError("restart supervisor log is unavailable") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        fail("restart supervisor log is not a regular file")
    if before.st_nlink != 1:
        fail("restart supervisor log must have exactly one link")
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
    except OSError as error:
        raise DeploymentError("restart supervisor log could not be opened") from error
    try:
        opened = os.fstat(descriptor)
        named = path.lstat()
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or growing_file_identity(before) != growing_file_identity(opened)
            or growing_file_identity(opened) != growing_file_identity(named)
        ):
            fail("restart supervisor log changed while it was opened")
    except BaseException:
        os.close(descriptor)
        raise
    return descriptor, opened

def _require_safe_restart_log_owner_mode(
    info: os.stat_result, owner_uid: int, owner_gid: int
) -> None:
    """Require a launchd- or runtime-owned append log with a safe mode."""

    if (
        info.st_uid not in {0, owner_uid}
        or info.st_gid not in {0, owner_gid}
        or stat.S_IMODE(info.st_mode) not in {0o600, 0o640, 0o644}
    ):
        fail("restart supervisor log has an unsafe owner or mode")

def _pread_exact(descriptor: int, offset: int, length: int) -> bytes:
    """Read exactly one bounded region without changing a shared file offset."""

    chunks: list[bytes] = []
    consumed = 0
    while consumed < length:
        try:
            chunk = os.pread(
                descriptor,
                min(1024 * 1024, length - consumed),
                offset + consumed,
            )
        except OSError as error:
            raise DeploymentError("restart supervisor log read failed") from error
        if not chunk:
            fail("restart supervisor log was truncated while it was read")
        chunks.append(chunk)
        consumed += len(chunk)
    return b"".join(chunks)

def bind_restart_log_cursor(
    path: Path, owner_uid: int, owner_gid: int
) -> RestartLogCursor:
    """Bind the pre-restart end of one safe supervisor log inode."""

    descriptor, opened = _open_growing_regular(path)
    try:
        _require_safe_restart_log_owner_mode(opened, owner_uid, owner_gid)
        offset = opened.st_size
        guard_offset = max(0, offset - RESTART_LOG_PREFIX_GUARD_BYTES)
        guard = _pread_exact(descriptor, guard_offset, offset - guard_offset)
        after = os.fstat(descriptor)
        named = path.lstat()
        if (
            growing_file_identity(after) != growing_file_identity(opened)
            or growing_file_identity(named) != growing_file_identity(opened)
            or after.st_size < offset
        ):
            fail("restart supervisor log changed while its cursor was bound")
        return RestartLogCursor(
            path=path,
            identity=growing_file_identity(opened),
            offset=offset,
            guard_offset=guard_offset,
            guard_sha256=hashlib.sha256(guard).hexdigest(),
        )
    finally:
        os.close(descriptor)

def read_restart_log_delta(cursor: RestartLogCursor) -> bytes:
    """Read a bounded post-cursor delta from the same append-only log inode."""

    descriptor, opened = _open_growing_regular(cursor.path)
    try:
        if growing_file_identity(opened) != cursor.identity:
            fail("restart supervisor log inode was replaced")
        if opened.st_size < cursor.offset:
            fail("restart supervisor log was truncated after restart began")
        guard = _pread_exact(
            descriptor,
            cursor.guard_offset,
            cursor.offset - cursor.guard_offset,
        )
        if hashlib.sha256(guard).hexdigest() != cursor.guard_sha256:
            fail("restart supervisor log prefix changed after restart began")
        end = opened.st_size
        delta_size = end - cursor.offset
        if delta_size > MAX_RESTART_LOG_DELTA_BYTES:
            fail("restart supervisor log delta exceeded its bound")
        delta = _pread_exact(descriptor, cursor.offset, delta_size)
        after = os.fstat(descriptor)
        named = cursor.path.lstat()
        if (
            growing_file_identity(after) != cursor.identity
            or growing_file_identity(named) != cursor.identity
            or after.st_size < end
        ):
            fail("restart supervisor log changed while its delta was read")
        return delta
    finally:
        os.close(descriptor)

def require_snapshot_backed_restart(cursor: RestartLogCursor) -> None:
    """Require exactly one snapshot restore and forbid empty-state fallback."""

    delta = read_restart_log_delta(cursor)
    if any(marker in delta for marker in SNAPSHOT_LOAD_FALLBACK_MARKERS):
        fail("managed validator restart entered a snapshot fallback path")
    if delta.count(SNAPSHOT_LOAD_SUCCESS_MARKER) != 1:
        fail("managed validator restart did not load exactly one state snapshot")

def restart_proof(
    bundle: BundlePlan,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    runtime_root: Path,
    plist_bodies: dict[str, bytes],
    installed_binary: Path,
    baseline: FleetSample,
    ops: SystemOps,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> RestartProofResult:
    """Terminate one exact child and measure bounded snapshot-backed recovery."""

    terminal_checker()
    peer = bundle.peers[0]
    supervisor_pid, child_pid = verify_managed_peer(
        peer,
        bundle,
        runtime_root,
        plist_bodies[peer.label],
        installed_binary,
        ops,
    )
    log_cursor = bind_restart_log_cursor(
        runtime_root / "logs" / f"validator-{peer.number}-supervisor.log",
        bundle.owner_uid,
        bundle.owner_gid,
    )
    deadline = time.monotonic() + RESTART_PROOF_TIMEOUT_SECONDS
    restart_started_ns = time.monotonic_ns()
    ops.terminate(child_pid)
    new_child: Optional[int] = None
    while time.monotonic() < deadline:
        terminal_checker()
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
    terminal_checker()
    advanced = wait_for_advancement(
        bundle,
        expected_source_commit,
        expected_dpn_validator_release_commit,
        baseline,
        deadline,
        getter=getter,
        health_getter=health_getter,
        terminal_checker=terminal_checker,
    )
    require_snapshot_backed_restart(log_cursor)
    final_supervisor, final_child = verify_managed_peer(
        peer,
        bundle,
        runtime_root,
        plist_bodies[peer.label],
        installed_binary,
        ops,
    )
    if final_supervisor != supervisor_pid or final_child != new_child:
        fail("restart proof changed the authenticated supervisor or replacement child")
    terminal_checker()
    restart_completed_ns = time.monotonic_ns()
    if restart_completed_ns < restart_started_ns:
        fail("monotonic restart timer moved backwards")
    duration_ns = restart_completed_ns - restart_started_ns
    duration_ms = (duration_ns + 999_999) // 1_000_000
    if duration_ms > RESTART_PROOF_TIMEOUT_SECONDS * 1_000:
        fail("managed validator snapshot-backed restart exceeded 45 seconds")
    return RestartProofResult(fleet=advanced, duration_ms=duration_ms)

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
    ensure_runtime_directory(
        runtime_root / "terminal", bundle.owner_uid, bundle.owner_gid
    )
    install_lifecycle_journal_layout(bundle, runtime_root)
    return runtime_root


def lifecycle_journal_root(runtime_root: Path, peer: PeerPlan) -> Path:
    """Return the fixed owner-private journal root for one public validator."""

    return runtime_root / "lifecycle" / peer.slug


def install_lifecycle_journal_layout(
    bundle: BundlePlan, runtime_root: Path
) -> dict[str, Path]:
    """Provision four distinct owner-private lifecycle journal roots."""

    parent = runtime_root / "lifecycle"
    ensure_runtime_directory(parent, bundle.owner_uid, bundle.owner_gid)
    roots: dict[str, Path] = {}
    for peer in bundle.peers:
        root = lifecycle_journal_root(runtime_root, peer)
        if root in roots.values():
            fail("lifecycle journal roots are not distinct")
        ensure_runtime_directory(root, bundle.owner_uid, bundle.owner_gid)
        roots[peer.label] = root
    if set(roots) != set(LABELS) or len(set(roots.values())) != PEER_COUNT:
        fail("lifecycle journal layout is not the exact four-peer projection")
    return roots


def require_authenticated_lifecycle_node_ids(
    bundle: BundlePlan,
) -> dict[str, str]:
    """Return only config-, manifest-, and qualification-bound lifecycle IDs."""

    verified = require_receipt_signer_map(
        bundle.manifest.get("receipt_signers"),
        "reset manifest receipt signer map",
    )
    if verified != bundle.receipt_signers:
        fail("authenticated receipt signer plan changed after bundle validation")
    if tuple(row.slug for row in verified) != tuple(peer.slug for peer in bundle.peers):
        fail("authenticated receipt signer order differs from the deploy peer plan")
    return {row.slug: row.node_id for row in verified}


def apply_reset(
    args: argparse.Namespace,
    bundle: BundlePlan,
    sources: SourcePlan,
    old_cohort: Sequence[PlistSnapshot],
    *,
    rollout_starter: Callable[[], None],
    ops: Optional[SystemOps] = None,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    config_checker: Callable[
        [Path, BundlePlan], None
    ] = validate_installed_peer_configs,
) -> dict[str, Any]:
    """Validate and install one fresh reset, rolling back every moved component."""

    if os.geteuid() != 0:
        fail("--apply requires root")
    if len(old_cohort) != PEER_COUNT:
        fail("--apply requires exactly four authenticated rollback plists")
    require_kagemusha_apply_material(bundle)
    lifecycle_node_ids = require_authenticated_lifecycle_node_ids(bundle)
    ops = ops or SystemOps()

    ensure_root_directory(INSTALL_ROOT, 0o755)
    binary_store = INSTALL_ROOT / "binaries"
    supervisor_store = INSTALL_ROOT / "supervisors"
    ensure_root_directory(binary_store, 0o755)
    ensure_root_directory(supervisor_store, 0o755)
    binary_dir = binary_store / sources.binary_sha256
    supervisor_dir = supervisor_store / sources.supervisor_sha256
    installed_binary = binary_dir / "iroha3d"
    installed_supervisor = supervisor_dir / "taira_peer_supervisor.py"
    if binary_dir.exists() and not (
        installed_binary.exists() or installed_binary.is_symlink()
    ):
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
    require_exact_names(binary_dir, {"iroha3d"}, "content-addressed binary directory")
    require_exact_names(
        supervisor_dir,
        {"taira_peer_supervisor.py"},
        "content-addressed supervisor directory",
    )
    os.chmod(binary_dir, 0o555)
    os.chmod(supervisor_dir, 0o555)
    fsync_directory(binary_dir)
    fsync_directory(supervisor_dir)
    fsync_directory(binary_store)
    fsync_directory(supervisor_store)
    require_root_controlled_file(installed_binary, executable=True)
    require_root_controlled_file(installed_supervisor, executable=True)
    runtime_root = install_runtime_layout(bundle)
    require_filesystem_headroom(
        [
            bundle.root,
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
            restart_generation=args.restart_generation,
            lifecycle_journal_root=lifecycle_journal_root(runtime_root, peer),
            authenticated_node_id=lifecycle_node_ids[peer.slug],
        )
        for peer in bundle.peers
    }
    terminal_bindings = {
        peer.label: supervisor_terminal_binding(
            sources.binary_sha256,
            binary_info,
            peer.config_sha256,
            args.restart_generation,
        )
        for peer in bundle.peers
    }
    deployed_receipt_signers = deployed_receipt_signer_map(
        bundle,
        sources,
        binary_info,
        args.restart_generation,
    )
    for label, body in plist_bodies.items():
        payload = plistlib.loads(body)
        if payload.get("Label") != label:
            fail(f"generated LaunchDaemon label mismatch: {label}")

    cohort_mutated = False
    guarded_signals = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    previous_handlers = {signum: signal.getsignal(signum) for signum in guarded_signals}

    def request_rollback(signum: int, _frame: object) -> NoReturn:
        raise DeploymentError(
            f"controller received signal {signum}; rolling back the four-job cohort"
        )

    for signum in guarded_signals:
        signal.signal(signum, request_rollback)
    try:
        # Recheck all four old jobs immediately before the first cohort change.
        require_bundle_runtime_unchanged(bundle)
        config_checker(installed_binary, bundle)
        require_kagemusha_external_release_unchanged(
            bundle,
            phase="after exact installed-binary config validation",
        )
        for snapshot in old_cohort:
            current_body, current_info = read_regular(snapshot.path, MAX_MANIFEST_BYTES)
            if (
                current_body != snapshot.body
                or stat.S_IMODE(current_info.st_mode) != snapshot.mode
                or current_info.st_uid != snapshot.uid
                or current_info.st_gid != snapshot.gid
            ):
                fail(
                    f"old LaunchDaemon changed after dry-run capture: {snapshot.path.name}"
                )
            verify_restored_snapshot(snapshot, ops)
        require_mutable_bundle_identities(bundle, phase="immediately before cutover")
        # Close the asynchronous-signal window between durable replay
        # consumption and the first external cohort mutation.  Once the first
        # bootout is attempted, its side effects cannot be proven absent, so
        # rollback and replay protection must both remain committed.
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, guarded_signals)
        try:
            rollout_starter()
            cohort_mutated = True
            ops.bootout(old_cohort[0].path.stem)
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        # Stop the rest of the old cohort before publishing any new job.
        for snapshot in old_cohort[1:]:
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

        terminal_checker = lambda: require_no_terminal_unhealthy(
            bundle, runtime_root, terminal_bindings
        )
        health_deadline = time.monotonic() + args.health_timeout_seconds
        baseline = wait_for_fleet_sample(
            bundle,
            args.expected_source_commit,
            args.expected_dpn_validator_release_commit,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
        advanced = wait_for_advancement(
            bundle,
            args.expected_source_commit,
            args.expected_dpn_validator_release_commit,
            baseline,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
        terminal_checker()
        for peer in bundle.peers:
            verify_managed_peer(
                peer,
                bundle,
                runtime_root,
                plist_bodies[peer.label],
                installed_binary,
                ops,
            )
        terminal_checker()
        restart_result = restart_proof(
            bundle,
            args.expected_source_commit,
            args.expected_dpn_validator_release_commit,
            runtime_root,
            plist_bodies,
            installed_binary,
            advanced,
            ops,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
        restarted = restart_result.fleet
        require_kagemusha_external_release_unchanged(
            bundle,
            phase="after final restart proof",
        )
    except BaseException as rollout_error:
        # A second termination request must not interrupt cohort rollback.
        for signum in guarded_signals:
            signal.signal(signum, signal.SIG_IGN)
        if cohort_mutated:
            try:
                rollback_cohort(old_cohort, ops)
            except BaseException as rollback_error:
                combined = DeploymentError(
                    "Taira reset failed and the exact four-validator cohort rollback did not complete"
                )
                _mark_deployment_outcome(combined, "rollback-failed")
                if hasattr(combined, "add_note"):
                    combined.add_note(
                        f"rollout failure: {type(rollout_error).__name__}: {rollout_error}"
                    )
                raise combined from rollback_error
        _mark_deployment_outcome(rollout_error, "rolled-back")
        raise
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)

    report: dict[str, Any] = {
        "applied": True,
        "absent_old_children": sorted(
            snapshot.path.stem
            for snapshot in old_cohort
            if not snapshot.managed.child_was_present
        ),
        "binary": str(installed_binary),
        "binary_sha256": sources.binary_sha256,
        "bundle": str(bundle.root),
        "chain_id": CHAIN_ID,
        "config_set_sha256": deployed_config_set_sha256(bundle),
        "deployment_completed_at_unix_ms": deployment_completed_at_unix_ms(),
        "genesis_block_hash": require_genesis_expected_hash(
            bundle.manifest.get("genesis_expected_hash")
        ),
        "nexus_topology": restarted.nexus_topology,
        "network_id": NETWORK_ID,
        "network_name": NETWORK_NAME,
        "protocol_version": PROTOCOL_VERSION,
        "signed_genesis_sha256": require_sha256(
            bundle.manifest.get("signed_genesis_sha256"),
            "deployed signed genesis SHA-256",
        ),
        "topology_sha256": deployed_topology_sha256(restarted.nexus_topology),
        "end_block_hash": restarted.block_hash,
        "end_height": restarted.height,
        "peer_count": PEER_COUNT,
        "receipt_signers": deployed_receipt_signers,
        "restart_generation": args.restart_generation,
        "restart_duration_ms": restart_result.duration_ms,
        "restart_proof": "passed",
        "source_commit": args.expected_source_commit,
        "dpn_validator_release_commit": args.expected_dpn_validator_release_commit,
        "start_height": baseline.height,
        "supervisor": str(installed_supervisor),
        "supervisor_sha256": sources.supervisor_sha256,
    }
    report.update(
        _kagemusha_report_fields(bundle, exact_binary_config_verified=True)
    )
    return report

def build_parser() -> argparse.ArgumentParser:
    """Build the exact single-command dry-run/apply interface."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--supervisor", type=Path, required=True)
    parser.add_argument("--admission-archive", type=Path, required=True)
    parser.add_argument("--admission-authority-dir", type=Path, required=True)
    parser.add_argument("--boi-qualified-handoff-root", type=Path, required=True)
    parser.add_argument(
        "--supervisor-python",
        type=Path,
        default=DEFAULT_SUPERVISOR_PYTHON,
    )
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-dpn-validator-release-commit", required=True)
    parser.add_argument("--expected-cargo-lock-sha256", required=True)
    parser.add_argument(
        "--expected-workspace-source-manifest-sha256", required=True
    )
    parser.add_argument("--expected-receipt-id", required=True)
    parser.add_argument("--expected-artifact-handoff-sha256", required=True)
    parser.add_argument("--expected-production-reset-manifest-sha256", required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument(
        "--trusted-boi-qualification-public-key", type=Path, required=True
    )
    parser.add_argument(
        "--trusted-boi-qualification-signing-fingerprint", required=True
    )
    parser.add_argument("--expected-boi-qualification-host-id", required=True)
    parser.add_argument(
        "--expected-boi-qualification-installation-id", required=True
    )
    parser.add_argument(
        "--expected-boi-qualification-controller-digest", required=True
    )
    parser.add_argument("--expected-workflow-run-id", type=int, required=True)
    parser.add_argument("--expected-workflow-run-attempt", type=int, required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument(
        "--trusted-release-manifest-verifier-sha256", required=True
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
    parser.add_argument("--operator-network-id")
    parser.add_argument("--operator-private-key-file", type=Path)
    parser.add_argument(
        "--allow-absent-old-child",
        action="store_true",
        help=(
            "explicitly authorize capture of an already-degraded old job only "
            "when its exact loaded supervisor has no PID file and no child "
            "process; stale or mismatched children remain fatal"
        ),
    )
    parser.add_argument("--apply", action="store_true")
    return parser

def validate_arguments(args: argparse.Namespace) -> None:
    """Validate scalar inputs before reading deployment paths."""

    args.expected_source_commit = require_commit(args.expected_source_commit)
    args.expected_dpn_validator_release_commit = require_commit(
        args.expected_dpn_validator_release_commit
    )
    args.expected_cargo_lock_sha256 = require_sha256(
        args.expected_cargo_lock_sha256, "expected Cargo.lock SHA-256"
    )
    args.expected_workspace_source_manifest_sha256 = require_sha256(
        args.expected_workspace_source_manifest_sha256,
        "expected workspace source manifest SHA-256",
    )
    args.expected_receipt_id = require_sha256(
        args.expected_receipt_id, "expected receipt ID"
    )
    args.expected_artifact_handoff_sha256 = require_sha256(
        args.expected_artifact_handoff_sha256,
        "expected macOS build handoff SHA-256",
    )
    args.expected_production_reset_manifest_sha256 = require_sha256(
        args.expected_production_reset_manifest_sha256,
        "expected production reset-manifest SHA-256",
    )
    (
        args.trusted_signing_fingerprint,
        args.trusted_boi_qualification_signing_fingerprint,
    ) = require_distinct_signing_fingerprints(
        args.trusted_signing_fingerprint,
        args.trusted_boi_qualification_signing_fingerprint,
    )
    args.trusted_release_manifest_verifier_sha256 = require_sha256(
        args.trusted_release_manifest_verifier_sha256,
        "trusted release-manifest verifier SHA-256",
    )
    if args.health_timeout_seconds <= 0:
        fail("--health-timeout-seconds must be positive")
    if args.minimum_free_bytes < DEFAULT_MINIMUM_FREE_BYTES:
        fail(f"--minimum-free-bytes may not be below {DEFAULT_MINIMUM_FREE_BYTES}")
    if not 1 <= args.maximum_fsync_latency_ms <= DEFAULT_MAXIMUM_FSYNC_LATENCY_MS:
        fail("--maximum-fsync-latency-ms must be positive and may not exceed 250")
    if args.apply:
        operator_network_id = getattr(args, "operator_network_id", None)
        operator_private_key_file = getattr(args, "operator_private_key_file", None)
        if not operator_network_id or operator_private_key_file is None:
            fail(
                "--apply requires --operator-network-id and --operator-private-key-file"
            )
        if not operator_private_key_file.is_absolute():
            fail("--operator-private-key-file must be an absolute runtime-only path")

def require_sealed_external_tool_identity() -> Optional[tuple[int, int]]:
    """Require the controller-provided non-root identity before root verification."""

    raw_uid = os.environ.get(EXTERNAL_TOOL_UID_ENV)
    raw_gid = os.environ.get(EXTERNAL_TOOL_GID_ENV)
    if (raw_uid is None) != (raw_gid is None):
        fail("sealed external-tool identity is incomplete")
    if raw_uid is None:
        if os.geteuid() == 0:
            fail("root deployment lacks the sealed external-tool identity")
        return None
    assert raw_gid is not None
    if (
        not raw_uid.isascii()
        or not raw_uid.isdecimal()
        or not raw_gid.isascii()
        or not raw_gid.isdecimal()
    ):
        fail("sealed external-tool identity is noncanonical")
    uid = int(raw_uid)
    gid = int(raw_gid)
    if raw_uid != str(uid) or raw_gid != str(gid) or uid <= 0 or gid <= 0:
        fail("sealed external-tool identity must contain positive canonical IDs")
    if os.geteuid() != 0 and (uid, gid) != (os.geteuid(), os.getegid()):
        fail("sealed external-tool identity differs from the current identity")
    return None if os.geteuid() != 0 else (uid, gid)


_kagemusha_authority_subject = _DEPLOY_AUTHORITY.kagemusha_subject
_kagemusha_authority_artifacts = _DEPLOY_AUTHORITY.kagemusha_artifacts
_kagemusha_report_fields = _DEPLOY_AUTHORITY.report_fields
_deploy_authority_subject = _DEPLOY_AUTHORITY.subject
_deploy_authority_artifacts = _DEPLOY_AUTHORITY.artifacts

_deploy_result_sha256 = _DEPLOY_AUTHORITY.result_sha256

def _authorize_deploy_lease(
    admission: AdmissionPlan,
    bundle: BundlePlan,
    sources: SourcePlan,
    *,
    apply: bool,
    kagemusha_exact_binary_config_verified: bool = False,
) -> taira_authority_client.AuthorityResult:
    try:
        return taira_authority_client.authorize(
            "deploy-issuance",
            _deploy_authority_subject(
                admission,
                bundle,
                sources,
                exact_binary_config_verified=(
                    kagemusha_exact_binary_config_verified
                ),
            ),
            artifacts=_deploy_authority_artifacts(admission, bundle, sources),
            disposition="apply" if apply else "dry-run",
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise DeploymentError(
            f"deploy-issuance authority refused the exact deployment: {error}"
        ) from error


def _finalize_deploy_lease(
    admission: AdmissionPlan,
    bundle: BundlePlan,
    sources: SourcePlan,
    lease: taira_authority_client.AuthorityResult,
    *,
    outcome: str,
    result: dict[str, object],
) -> taira_authority_client.AuthorityResult:
    """Bind a consumed lease to success or the completed rollback outcome."""

    try:
        return taira_authority_client.finalize_deployment(
            _deploy_authority_subject(admission, bundle, sources),
            lease=lease,
            outcome=outcome,
            result_sha256=_deploy_result_sha256(outcome, result),
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise DeploymentError(
            "deploy-issuance authority could not persist the terminal result: "
            f"{error}"
        ) from error

def _execute_after_provisioned_authority_contracts(
    args: argparse.Namespace, *, ops: Optional[SystemOps] = None
) -> dict[str, Any]:
    """Latent deployment path reached only after installed authority evolves."""

    require_sealed_external_tool_identity()
    validate_arguments(args)
    if args.apply and os.geteuid() != 0:
        fail("--apply requires root; no changes were made")
    admission = verify_deployment_admission(args)
    bundle = validate_bundle(
        args.bundle,
        expected_reset_manifest_sha256=args.expected_production_reset_manifest_sha256,
        expected_binary_sha256=admission.binary_sha256,
        expected_source_commit=admission.source_commit,
        expected_dpn_validator_release_commit=(
            admission.dpn_validator_release_commit
        ),
        minimum_free_bytes=args.minimum_free_bytes,
        maximum_fsync_latency_ms=args.maximum_fsync_latency_ms,
    )
    sources = validate_sources(args, bundle, admission)
    require_inputs_match_admission(bundle, sources, admission)
    if args.apply:
        require_kagemusha_apply_material(bundle)
    args.restart_generation = admission.restart_generation
    system_ops = ops or SystemOps()
    capture_options: dict[str, bool] = {
        "allow_absent_child": args.allow_absent_old_child,
    }
    if not args.apply:
        kagemusha_exact_binary_config_verified = (
            validate_dry_run_kagemusha_exact_config(sources, bundle)
        )
        old_cohort = capture_old_cohort(system_ops, **capture_options)
        require_admission_archive_unchanged(admission)
        require_mutable_bundle_identities(
            bundle,
            phase="immediately before dry-run authority",
        )
        lease = _authorize_deploy_lease(
            admission,
            bundle,
            sources,
            apply=False,
            kagemusha_exact_binary_config_verified=(
                kagemusha_exact_binary_config_verified
            ),
        )
        require_mutable_bundle_identities(
            bundle,
            phase="immediately after dry-run authority",
        )
        kagemusha_fields = _kagemusha_report_fields(
            bundle,
            exact_binary_config_verified=(
                kagemusha_exact_binary_config_verified
            ),
        )
        kagemusha_blocked = (
            kagemusha_fields["kagemusha_config_projection_sha256"] is not None
            and kagemusha_fields["kagemusha_external_release_verified"] is False
        )
        report = {
            "admission_archive_sha256": admission.archive_sha256,
            "admission_receipt_consumed": False,
            "admission_receipt_id": admission.receipt_id,
            "applied": False,
            "boi_artifact_inventory_sha256": (
                admission.boi_artifact_inventory_sha256
            ),
            "boi_qualified_inventory_sha256": (
                admission.boi_qualified_inventory_sha256
            ),
            "boi_qualification_receipt_id": (
                admission.boi_qualification_receipt_id
            ),
            "absent_old_children": sorted(
                snapshot.path.stem
                for snapshot in old_cohort
                if not snapshot.managed.child_was_present
            ),
            "binary_sha256": sources.binary_sha256,
            "bundle": str(bundle.root),
            "bundle_bytes": bundle.bundle_bytes,
            "free_bytes": bundle.free_bytes,
            "fsync_latency_ms": round(bundle.fsync_latency_ms, 3),
            "deployment_ready": not kagemusha_blocked,
            "mode": (
                (
                    "blocked-kagemusha-semantic-validation-dry-run"
                    if kagemusha_fields[
                        "kagemusha_external_release_material_present"
                    ]
                    else "blocked-kagemusha-external-release-dry-run"
                )
                if kagemusha_blocked
                else "verified-read-only-dry-run"
            ),
            "deploy_authority_operation_id": lease.operation_id,
            "deploy_authority_status": lease.status,
            "peer_count": PEER_COUNT,
            "restart_generation": args.restart_generation,
            "source_commit": args.expected_source_commit,
            "dpn_validator_release_commit": (
                args.expected_dpn_validator_release_commit
            ),
            "supervisor_sha256": sources.supervisor_sha256,
        }
        report.update(kagemusha_fields)
        return report
    # This refusal deliberately precedes the deployment lock and replay-ledger
    # consumption: a cohort without receipt-signer-bound lifecycle identity may
    # not begin even a recoverable apply transaction.
    require_authenticated_lifecycle_node_ids(bundle)
    with exclusive_deployment_lock():
        locked_admission = verify_deployment_admission(args)
        if locked_admission != admission:
            fail("verified admission identity changed before the deployment lock")
        require_admission_archive_unchanged(locked_admission)
        require_admission_bound_inputs_unchanged(bundle, sources, locked_admission)
        old_cohort = capture_old_cohort(system_ops, **capture_options)
        operator_getter = build_operator_http_getter(
            args.operator_network_id,
            args.operator_private_key_file,
        )
        require_admission_bound_inputs_unchanged(bundle, sources, locked_admission)
        require_admission_archive_unchanged(locked_admission)
        lease = _authorize_deploy_lease(
            locked_admission, bundle, sources, apply=True
        )
        try:
            with consume_admission_receipt(locked_admission) as consumption:
                report = apply_reset(
                    args,
                    bundle,
                    sources,
                    old_cohort,
                    rollout_starter=consumption.mark_rollout_started,
                    ops=system_ops,
                    getter=operator_getter,
                )
        except BaseException as deployment_error:
            outcome = getattr(
                deployment_error,
                DEPLOYMENT_OUTCOME_ATTRIBUTE,
                "rolled-back",
            )
            if outcome not in {"rolled-back", "rollback-failed"}:
                outcome = "rollback-failed"
            failure_result = {
                "error": str(deployment_error),
                "error_type": type(deployment_error).__name__,
            }
            try:
                _finalize_deploy_lease(
                    locked_admission,
                    bundle,
                    sources,
                    lease,
                    outcome=outcome,
                    result=failure_result,
                )
            except DeploymentError as finalization_error:
                if hasattr(finalization_error, "add_note"):
                    finalization_error.add_note(
                        "deployment result before authority finalization: "
                        f"{type(deployment_error).__name__}: {deployment_error}"
                    )
                raise finalization_error from deployment_error
            raise
        finalization = _finalize_deploy_lease(
            locked_admission,
            bundle,
            sources,
            lease,
            outcome="success",
            result=report,
        )
        report.update(
            {
                "admission_archive_sha256": locked_admission.archive_sha256,
                "admission_receipt_consumed": True,
                "admission_receipt_id": locked_admission.receipt_id,
                "boi_artifact_inventory_sha256": (
                    locked_admission.boi_artifact_inventory_sha256
                ),
                "boi_qualified_inventory_sha256": (
                    locked_admission.boi_qualified_inventory_sha256
                ),
                "boi_qualification_receipt_id": (
                    locked_admission.boi_qualification_receipt_id
                ),
                "deploy_authority_operation_id": lease.operation_id,
                "deploy_authority_status": lease.status,
                "deploy_authority_final_status": finalization.status,
                "deploy_authority_result_receipt_sha256": hashlib.sha256(
                    finalization.durable_receipt_bytes
                ).hexdigest(),
            }
        )
        return report

def execute(
    args: argparse.Namespace, *, ops: Optional[SystemOps] = None
) -> dict[str, Any]:
    """Authenticate issuance before any identity, path, or admission read."""

    require_deploy_issuance_contracts()
    return _execute_after_provisioned_authority_contracts(args, ops=ops)

def main(argv: Optional[list[str]] = None) -> int:
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
