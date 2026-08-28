#!/usr/bin/env python3
"""Bring up, verify, inspect, or stop one disposable four-peer Taira devnet.

``up`` requires an explicit owner-only Inrou guest-canary workspace, builds the
current Kagami, fixed-FD Taira daemon, CLI, and SoraFS node; asks Kagami
for a fresh four-validator NPoS Nexus network using the canonical Taira chain
id; validates all four configs; starts the peers; and proves finality with one
signed ``iroha tx ping`` submission followed by the typed transaction-status
waiter.
The generated profile enables the sole first-release Inrou backend, one
PortableVM per validator, with four canonical same-host identities. ``up`` is
therefore intentionally available only on a qualified Linux AArch64/KVM host;
the daemon remains authoritative for the complete identity, immutable runtime
closure, namespace, cgroup, QMP, and firewall preflight.
The required ``--inrou-canary-dir`` stages operator-supplied guest assets with
the current CLI, preseeds the exact SoraFS commitments into all four disjoint
stores, executes the onboarding, faucet, final-canary, bundle, guest,
discovery, and service transactions as one ordered chain of durable prepared
envelopes, and
verifies the canonical four-replica public route. Successful
``up`` and ``check`` results require the resulting exact V1 guest-workload
qualification record. ``check`` additionally revalidates the retained input,
stage, qualifying CLI/source/target identity, account-signed service status,
manifest hashes, and all four routed identities without submitting a mutation
or ping.
The generated network lives in one marked directory and is replaced on the
next ``up``.  There is no release authority, promotion state, evidence bundle,
soak, or rollback workflow.
"""

from __future__ import annotations

import argparse
import errno
import fcntl
import grp
import hashlib
import importlib.util
import json
import os
import platform
import pwd
import re
import select
import signal
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unicodedata
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path
from typing import Any, NoReturn, TypeVar

try:
    from taira_constants import (
        CHAIN_ID as DEFAULT_CHAIN_ID,
        CHAIN_DISCRIMINANT as DEFAULT_CHAIN_DISCRIMINANT,
        PEER_COUNT,
        network_id_from_genesis_hash,
    )
except ModuleNotFoundError:
    from scripts.taira_constants import (
        CHAIN_ID as DEFAULT_CHAIN_ID,
        CHAIN_DISCRIMINANT as DEFAULT_CHAIN_DISCRIMINANT,
        PEER_COUNT,
        network_id_from_genesis_hash,
    )


REPO_ROOT = Path(__file__).resolve().parents[1]
_I105_ACCOUNT_DECODER: Callable[..., bytes] | None = None
DEFAULT_DIR = Path("/var/lib/iroha-taira-devnet")
DEFAULT_API_PORT = 29_080
DEFAULT_P2P_PORT = 33_337
DEFAULT_OPERATION_TIMEOUT_SECONDS = 300
# Four optimized daemons plus the Nexus/AMX lane pipeline routinely need more
# than the ten-second view-zero deadline derived from Kagami's generic
# one-second localnet cadence.  The five-second proposal cadence deliberately
# trades a few seconds of smoke-test latency for a robust fifty-second
# view-zero deadline.
DEFAULT_BLOCK_CADENCE_MS = 5_000
MARKER = ".iroha-taira-devnet"
MARKER_BODY = "managed by scripts/taira_devnet.py\n"
NETWORK_CLEANUP_QUARANTINE = ".network-cleanup"
MAX_BUNDLE_TEXT_BYTES = 8 * 1024 * 1024
MAX_LOG_TAIL_BYTES = 64 * 1024
MAX_HTTP_RESPONSE_BYTES = 1024 * 1024
MAX_MARKER_BYTES = 128
MAX_PROCESS_RECORD_BYTES = 4096
TAIRA_PROCESS_RECORD_SCHEMA_VERSION_V1 = 1
TAIRA_PROCESS_RECORD_KEYS_V1 = frozenset(
    {
        "schema_version",
        "peer_index",
        "pid",
        "boot_id",
        "start_time_ticks",
        "executable_path",
        "executable_device",
        "executable_inode",
        "argv",
        "uid",
        "gid",
        "session_id",
        "process_group_id",
    }
)
TAIRA_PROCESS_RECORD_RUNTIME_KEYS_V1 = TAIRA_PROCESS_RECORD_KEYS_V1 - {
    "schema_version",
    "peer_index",
}
LINUX_BOOT_ID_PATH = Path("/proc/sys/kernel/random/boot_id")
LOWER_BOOT_ID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
)
BUILD_ENV_REMOVALS = (
    "CARGO_BUILD_TARGET",
    "CARGO_INCREMENTAL",
    "CARGO_TARGET_DIR",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "VERGEN_GIT_SHA",
    "IROHA_GIT_COMMIT_HASH",
)
TAIRA_BUILD_PROFILE = "local-release"
RUNTIME_SIGNER_DIRECTORY = Path("runtime") / "taira-runtime-signers"
RUNTIME_SIGNER_FILE_BYTES = 71
TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES = 68_719_476_736
TAIRA_NEXUS_STORAGE_WEIGHTS = (
    ("kura_blocks_bps", 6_000),
    ("wsv_snapshots_bps", 2_000),
    ("sorafs_bps", 2_000),
)
STORAGE_WEIGHT_BASIS_POINTS = 10_000
TAIRA_SORAFS_MAX_CAPACITY_BYTES = 13_743_895_347
TAIRA_SORACLOUD_HYDRATION_CONCURRENCY = 4
TAIRA_SORACLOUD_PREPARED_RUNTIME_CACHE_CAPACITY = 4
TAIRA_INROU_IDENTITY_BASE = 70_000
TAIRA_INROU_IDENTITY_SLOTS = PEER_COUNT
TAIRA_INROU_IDENTITY_NAME_PREFIX = "iroha-inrou-"
TAIRA_INROU_VM_CAPACITY = 1
TAIRA_INROU_MAX_CPU_MILLIS = 8_000
TAIRA_INROU_MAX_MEMORY_BYTES = 8 * 1024 * 1024 * 1024
TAIRA_INROU_MAX_STORAGE_BYTES = 64 * 1024 * 1024 * 1024
TAIRA_INROU_GUEST_IMAGE_MAX_BYTES = 10 * 1024 * 1024 * 1024
TAIRA_INROU_START_GRACE_MS = 30_000
TAIRA_INROU_STOP_GRACE_MS = 10_000
TAIRA_INROU_EGRESS_RATE_PER_MINUTE = 600
TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE = 100 * 1024 * 1024
INROU_CANARY_CONTAINER_FILE = "container_manifest.json"
INROU_CANARY_SERVICE_FILE = "service_manifest.json"
INROU_CANARY_BUNDLE_FILE = "bundle.tgz"
INROU_CANARY_GUEST_DIRECTORY = Path("inrou") / "aarch64"
INROU_CANARY_GUEST_FILES = ("vmlinux", "rootfs.ext4", "initrd.img")
INROU_CANARY_INPUT_SNAPSHOT_DIRECTORY = Path("taira-inrou-input")
INROU_GUEST_QUALIFICATION_FILE = "inrou_guest_qualification.json"
INROU_GUEST_QUALIFICATION_SCHEMA_VERSION_V1 = 1
MAX_INROU_GUEST_QUALIFICATION_BYTES = 64 * 1024
MAX_INROU_OPERATOR_PRESEED_RECEIPT_BYTES = 64 * 1024
MAX_INROU_OPERATOR_PRESEED_STDERR_BYTES = 64 * 1024
INROU_OPERATOR_PRESEED_POLL_SECONDS = 0.01
INROU_OPERATOR_PRESEED_RELEASE_ACK_V1 = (
    b'{"schema_version":1,"status":"released"}\n'
)
MAX_INROU_CANARY_MANIFEST_BYTES = 1024 * 1024
MAX_INROU_CANARY_BUNDLE_BYTES = 512 * 1024 * 1024
MAX_INROU_CANARY_GUEST_BYTES = TAIRA_INROU_GUEST_IMAGE_MAX_BYTES
INROU_STAGE_DIRECTORY = Path("runtime") / "taira-inrou-stage"
INROU_STAGE_RECEIPT_FILE = "receipt.json"
INROU_STAGE_CONTAINER_FILE = "container.json"
INROU_STAGE_SERVICE_FILE = "service.json"
INROU_STAGE_BUNDLE_PAYLOAD = Path("payloads") / "bundle.bin"
INROU_STAGE_GUEST_PAYLOAD = Path("payloads") / "guest"
INROU_STAGE_DISCOVERY_PAYLOAD = Path("payloads") / "discovery"
INROU_STAGE_DISCOVERY_DOCUMENT = INROU_STAGE_DISCOVERY_PAYLOAD / "index.json"
INROU_STAGE_BUNDLE_MANIFEST = Path("manifests") / "bundle.to"
INROU_STAGE_GUEST_MANIFEST = Path("manifests") / "aarch64.to"
INROU_STAGE_DISCOVERY_MANIFEST = Path("manifests") / "discovery.to"
MAX_INROU_STAGE_RECEIPT_BYTES = 64 * 1024
PREPARED_CANARY_DIRECTORY = Path("runtime") / "taira-prepared-canary-v1"
LOCALNET_ONBOARDING_TOKEN_FILE = Path("runtime") / "onboarding.token"
MAX_PREPARED_ENVELOPE_BYTES = 4 * 1024 * 1024
PREPARED_MUTATION_PHASE = "pre_edge"
PREPARED_EXECUTION_LEASE_MIN_SECONDS = 15 * 60
PREPARED_RECOVERY_INITIAL_BACKOFF_SECONDS = 0.25
PREPARED_RECOVERY_MAX_BACKOFF_SECONDS = 2.0
PREPARED_RECOVERY_MAX_ATTEMPTS = 256
PREPARED_WRITE_CHILDREN = (
    ("onboarding", "onboarding", "onboarding"),
    ("faucet", "faucet", "faucet"),
    ("write_canary", "final-canary", "final_canary"),
)
PREPARED_INROU_CHILDREN = (
    ("inrou_bundle_pin", "bundle-pin", "bundle_pin"),
    ("inrou_guest_pin", "guest-pin", "guest_pin"),
    ("inrou_discovery_pin", "discovery-pin", "discovery_pin"),
    ("inrou_canary", "service-mutation", "service_mutation"),
)
LINUX_KVM_GET_API_VERSION = 0xAE00
LINUX_KVM_API_VERSION = 12
INROU_CANARY_SERVICE_VERSION_PREFIX_V1 = "artifact-"
INROU_CANARY_ROUTE_HOST_V1 = "taira.sora.org"
INROU_CANARY_ROUTE_PREFIX_V1 = "/api/v1/inrou-canary"
INROU_CANARY_HEALTH_PATH_V1 = f"{INROU_CANARY_ROUTE_PREFIX_V1}/health"
INROU_CANARY_REPORT_KEYS_V1 = frozenset(
    {
        "command",
        "status",
        "public_root",
        "checks",
        "warnings",
        "failures",
        "service_name",
        "service_version",
        "mutation_mode",
        "route_host",
        "route_path",
        "active_host_adverts",
        "hosted_replica_count",
        "bundle_hash",
        "bundle_content_cid",
        "bundle_manifest_digest_hex",
        "guest_content_cid",
        "guest_manifest_digest_hex",
        "discovery_payload_dir",
        "discovery_document_hash",
        "discovery_content_cid",
        "discovery_manifest_digest_hex",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "deployment_bundle_hash",
        "container_manifest_hash",
        "service_manifest_hash",
        "observed_at_unix_ms",
        "replica_identities",
        "local_placement",
        "authorization_sha256",
        "authorization_nonce",
        "mutation_kind",
        "mutation_phase",
        "idempotency_key",
        "operation",
        "transaction_hash_hex",
        "prepared_envelope_sha256",
        "prepared_envelope_size",
        "recovery_outcome",
        "applied_block_height",
        "evidence",
        "execution_expires_at_unix_ms",
        "fee_payment",
        "fee_quote",
    }
)
INROU_CANARY_CHECK_KEYS_V1 = frozenset({"name", "http_status", "ok", "detail"})
INROU_CANARY_REPLICA_KEYS_V1 = frozenset(
    {
        "replica_slot",
        "identity",
        "response_sha256",
        "app_data_marker_sha256",
        "boot_sequence",
        "guest_boot_id_sha256",
    }
)
INROU_LOCAL_PLACEMENT_KEYS_V1 = frozenset(
    {"peer_id", "validator_account_id", "replica_slot", "placement_incarnation"}
)
PREPARED_REPORT_BASE_KEYS_V1 = frozenset(
    {"command", "status", "public_root", "checks", "warnings", "failures"}
)
PREPARED_REPORT_MUTATION_KEYS_V1 = frozenset(
    {
        "authorization_sha256",
        "authorization_nonce",
        "mutation_kind",
        "mutation_phase",
        "idempotency_key",
        "operation",
        "transaction_hash_hex",
        "prepared_envelope_sha256",
        "prepared_envelope_size",
        "recovery_outcome",
        "applied_block_height",
        "evidence",
        "execution_expires_at_unix_ms",
    }
)
PREPARED_REPORT_FEE_KEYS_V1 = frozenset({"fee_payment", "fee_quote"})
INROU_CHECK_REPORT_KEYS_V1 = frozenset(
    {
        "command",
        "status",
        "public_root",
        "checks",
        "warnings",
        "failures",
        "service_name",
        "service_version",
        "route_host",
        "route_path",
        "active_host_adverts",
        "hosted_replica_count",
        "bundle_hash",
        "bundle_content_cid",
        "bundle_manifest_digest_hex",
        "guest_content_cid",
        "guest_manifest_digest_hex",
        "discovery_payload_dir",
        "discovery_document_hash",
        "discovery_content_cid",
        "discovery_manifest_digest_hex",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "deployment_bundle_hash",
        "container_manifest_hash",
        "service_manifest_hash",
        "observed_at_unix_ms",
        "replica_identities",
        "local_placement",
    }
)
COMPILED_TOOL_EVIDENCE_KEYS_V1 = frozenset({"path", "sha256", "bytes"})
COMPILED_TOOLCHAIN_NAMES_V1 = (
    "kagami",
    "iroha3d_taira",
    "iroha",
    "sorafs-node",
)
SOURCE_OBSERVATION_KEYS_V1 = frozenset(
    {
        "branch",
        "git_head",
        "observation_scope",
        "observed_nonignored_worktree_sha256",
        "cargo_source_consumption",
    }
)
INROU_GUEST_QUALIFICATION_KEYS_V1 = frozenset(
    {
        "schema_version",
        "inrou_guest_workload_qualification",
        "inrou_canary_input_content_sha256",
        "inrou_operator_preseed",
        "inrou_canary",
        "inrou_restart",
        "source_observation",
        "target_triple",
        "toolchain",
    }
)
INROU_OPERATOR_PRESEED_RECEIPT_KEYS_V1 = frozenset(
    {
        "schema_version",
        "status",
        "mode",
        "max_capacity_bytes",
        "targets",
        "artifacts",
    }
)
INROU_OPERATOR_PRESEED_TARGET_KEYS_V1 = frozenset(
    {"validator_account_id", "peer_id", "store_root"}
)
INROU_OPERATOR_PRESEED_ARTIFACT_KEYS_V1 = frozenset(
    {
        "manifest_digest_blake3",
        "payload_digest_blake3",
        "content_length",
        "store_count",
    }
)
INROU_RESTART_PROOF_KEYS_V1 = frozenset(
    {
        "schema_version",
        "peer_index",
        "local_placement",
        "processes_before",
        "processes_after",
        "height_before",
        "height_after",
        "start_script_sha256",
        "stop_script_sha256",
        "build_identity",
        "inrou_check_started_at_unix_ms",
        "inrou_check_finished_at_unix_ms",
        "inrou_check",
    }
)
INROU_RESTART_BUILD_IDENTITY_KEYS_V1 = frozenset(
    {"git_commit_sha", "target_triple"}
)
INROU_STAGE_RECEIPT_KEYS_V1 = frozenset(
    {
        "schema_version",
        "sorafs_retention_epoch",
        "mutation_mode",
        "placement_targets",
        "service_name",
        "service_version",
        "container_file",
        "service_file",
        "bundle_payload_file",
        "bundle_manifest_file",
        "bundle_hash",
        "bundle_content_cid",
        "bundle_manifest_digest_hex",
        "guest_isa",
        "guest_payload_dir",
        "guest_manifest_file",
        "guest_content_cid",
        "guest_manifest_digest_hex",
        "discovery_payload_dir",
        "discovery_manifest_file",
        "discovery_document_hash",
        "discovery_content_cid",
        "discovery_manifest_digest_hex",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "container_manifest_hash",
        "service_manifest_hash",
    }
)
SORAFS_CONTENT_CID_V1_RE = re.compile(r"b[a-z2-7]{58}")
LOWER_32_BYTE_HEX_RE = re.compile(r"[0-9a-f]{64}")
IROHA_HASH_LITERAL_RE = re.compile(r"hash:[0-9A-F]{64}#[0-9A-F]{4}")
LOWER_GIT_COMMIT_RE = re.compile(r"[0-9a-f]{40}")
LINUX_AARCH64_TARGET_RE = re.compile(
    r"aarch64-[a-z0-9_]+-linux(?:-[a-z0-9_.+]+)?"
)
TAIRA_QUALIFICATION_BRANCH = "optimizations"
MCP_PROTOCOL_VERSION_V1 = "2025-06-18"


@dataclass(frozen=True)
class InrouCanaryInputEvidence:
    """Pinned identity and content for one fixed canary input file."""

    relative_path: str
    path: Path
    identity: tuple[int, int, int, int, int, int, int, int]
    sha256: str


@dataclass(frozen=True)
class InrouCanaryWorkspaceEvidence:
    """Complete fixed-layout canary workspace observation held through staging."""

    root: Path
    directory_identities: tuple[
        tuple[str, tuple[int, int, int, int, int, int]], ...
    ]
    inputs: tuple[InrouCanaryInputEvidence, ...]
    content_sha256: str


@dataclass(frozen=True)
class TrustedInrouGuestArtifact:
    """Exact SoraFS identity trusted by every validator's Inrou runtime."""

    manifest_digest_hex: str
    content_cid: str


# Keep this inventory beside the commands that consume the surfaces.  A
# successful `--help` is not sufficient: clap still accepts an existing parent
# command when one of the leaf options used below has drifted away.
CLI_SURFACES: tuple[tuple[str, tuple[str, ...], tuple[str, ...]], ...] = (
    (
        "kagami",
        ("localnet",),
        (
            "--out-dir",
            "--sora-profile",
            "--consensus-mode",
            "--peers",
            "--bind-host",
            "--public-host",
            "--chain-id",
            "--base-api-port",
            "--base-p2p-port",
            "--block-cadence-ms",
        ),
    ),
    (
        "iroha3d_taira",
        (),
        ("--sora", "--config", "--genesis-manifest-json", "--check-config"),
    ),
    ("iroha", (), ("--config", "--machine", "--output-format", "--fee-payer")),
    ("iroha", ("tx", "ping"), ("--no-wait", "--log-level", "--msg")),
    (
        "iroha",
        ("tx", "status"),
        ("--hash", "--wait", "--timeout-ms", "--poll-interval-ms"),
    ),
)
INROU_CANARY_CLI_SURFACES: tuple[
    tuple[str, tuple[str, ...], tuple[str, ...]], ...
] = (
    (
        "iroha",
        ("taira", "inrou-stage"),
        (
            "--mode",
            "--container",
            "--service",
            "--bundle-file",
            "--sorafs-retention-epoch",
            "--placement-target",
            "--stage-dir",
            "--bind-validator-config-dir",
            "--json",
        ),
    ),
    (
        "iroha",
        ("taira", "inrou-canary"),
        (
            "--mode",
            "--operation",
            "--public-root",
            "--stage-dir",
            "--authorization-sha256",
            "--authorization-nonce",
            "--mutation-phase",
            "--idempotency-key",
            "--execution-expires-at-unix-ms",
            "--prepare-envelope",
            "--prepared-output-fd",
            "--submit-prepared-envelope-fd",
            "--recover-prepared-envelope-fd",
            "--prerequisite-envelope-fd",
            "--timeout-secs",
            "--json",
        ),
    ),
    (
        "iroha",
        ("taira", "write-canary"),
        (
            "--operation",
            "--public-root",
            "--onboarding-token-file",
            "--faucet-authority",
            "--faucet-asset-id",
            "--faucet-amount",
            "--authorization-sha256",
            "--authorization-nonce",
            "--mutation-phase",
            "--idempotency-key",
            "--execution-expires-at-unix-ms",
            "--prepare-envelope",
            "--prepared-output-fd",
            "--submit-prepared-envelope-fd",
            "--recover-prepared-envelope-fd",
            "--prerequisite-envelope-fd",
            "--json",
        ),
    ),
    (
        "iroha",
        ("taira", "inrou-check"),
        ("--mode", "--public-root", "--stage-dir", "--timeout-secs", "--json"),
    ),
    (
        "sorafs-node",
        (),
        (
            "preseed-session",
            "--target",
            "--max-capacity-bytes",
            "--verify-only",
            "--manifest",
            "--payload",
            "--payload-dir",
        ),
    ),
)


class DevnetError(RuntimeError):
    """A disposable Taira operation failed."""


class AmbiguousRetainedEnvelopeAction(DevnetError):
    """A retained-envelope child failed after it may have contacted Torii."""


def fail(message: str) -> NoReturn:
    """Raise a concise operator-facing error."""

    raise DevnetError(message)


ParallelInput = TypeVar("ParallelInput")
ParallelOutput = TypeVar("ParallelOutput")


def parallel_map(
    values: Sequence[ParallelInput],
    operation: Callable[[ParallelInput], ParallelOutput],
) -> list[ParallelOutput]:
    """Run independent bounded peer work concurrently and retain input order."""

    if len(values) < 2:
        return [operation(value) for value in values]
    with ThreadPoolExecutor(
        max_workers=min(PEER_COUNT, len(values)),
        thread_name_prefix="taira-devnet",
    ) as executor:
        futures = [executor.submit(operation, value) for value in values]
        return [future.result() for future in futures]


def _decode_canonical_i105_account_id(
    value: str,
    *,
    expected_discriminant: int | None = None,
) -> bytes:
    """Decode through the repository's dependency-free canonical AccountId codec."""

    global _I105_ACCOUNT_DECODER
    if _I105_ACCOUNT_DECODER is None:
        codec_path = REPO_ROOT / "python" / "iroha_torii_client" / "_account_id.py"
        spec = importlib.util.spec_from_file_location(
            "taira_devnet_account_id_codec", codec_path
        )
        if spec is None or spec.loader is None:
            fail("cannot load the canonical I105 AccountId codec")
        codec = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(codec)
        decoder = getattr(codec, "decode_canonical_i105_account_id", None)
        if not callable(decoder):
            fail("canonical I105 AccountId codec omits its decoder")
        _I105_ACCOUNT_DECODER = decoder
    return _I105_ACCOUNT_DECODER(
        value,
        expected_discriminant=expected_discriminant,
    )


def _fee_quote_account_ids_have_same_identity(left: Any, right: Any) -> bool:
    """Compare exact displays by the universal account-controller bytes."""

    if not isinstance(left, str) or not isinstance(right, str):
        return False
    try:
        return _decode_canonical_i105_account_id(
            left
        ) == _decode_canonical_i105_account_id(right)
    except (TypeError, ValueError):
        return False


def _fee_quote_program_ids_have_same_identity(left: Any, right: Any) -> bool:
    """Compare sponsor programs by controller identity plus exact name."""

    return (
        isinstance(left, dict)
        and isinstance(right, dict)
        and set(left) == {"sponsor", "name"}
        and set(right) == {"sponsor", "name"}
        and left["name"] == right["name"]
        and _fee_quote_account_ids_have_same_identity(
            left["sponsor"], right["sponsor"]
        )
    )


def _fee_quote_intents_have_same_identity(left: Any, right: Any) -> bool:
    """Compare exact fee intents while treating sponsor I105 displays as identity."""

    if left == right:
        return True
    if (
        not isinstance(left, dict)
        or not isinstance(right, dict)
        or set(left) != {"payer", "value"}
        or set(right) != {"payer", "value"}
        or left["payer"] != "sponsor"
        or right["payer"] != "sponsor"
        or not isinstance(left["value"], dict)
        or not isinstance(right["value"], dict)
    ):
        return False
    fields = {
        "program_id",
        "program_revision",
        "charge_limits",
        "gas_limit",
    }
    left_value = left["value"]
    right_value = right["value"]
    return (
        set(left_value) == fields
        and set(right_value) == fields
        and _fee_quote_program_ids_have_same_identity(
            left_value["program_id"], right_value["program_id"]
        )
        and left_value["program_revision"] == right_value["program_revision"]
        and left_value["charge_limits"] == right_value["charge_limits"]
        and left_value["gas_limit"] == right_value["gas_limit"]
    )


def is_canonical_iroha_hash_hex(value: object) -> bool:
    """Return whether ``value`` is exact lowercase marked Iroha Hash bytes."""

    if not isinstance(value, str) or len(value) != 64:
        return False
    try:
        decoded = bytes.fromhex(value)
    except ValueError:
        return False
    return len(decoded) == 32 and decoded.hex() == value and decoded[-1] & 1 == 1


def is_canonical_inrou_service_version(value: object) -> bool:
    """Return whether ``value`` is the artifact prefix plus one Iroha Hash."""

    return (
        isinstance(value, str)
        and value.startswith(INROU_CANARY_SERVICE_VERSION_PREFIX_V1)
        and is_canonical_iroha_hash_hex(
            value[len(INROU_CANARY_SERVICE_VERSION_PREFIX_V1) :]
        )
    )


def json_loads_no_duplicates(payload: str | bytes | bytearray) -> object:
    """Decode JSON while rejecting duplicate object members at every depth."""

    def unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON field {key}")
            result[key] = value
        return result

    return json.loads(payload, object_pairs_hook=unique_json_object)


def _probe_linux_kvm_api_version(path: Path) -> int:
    """Open one direct KVM character device and return its kernel API version."""

    metadata = os.stat(path, follow_symlinks=False)
    if not stat.S_ISCHR(metadata.st_mode):
        fail(f"Inrou qualification requires a direct KVM character device: {path}")
    flags = os.O_RDWR | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        return int(fcntl.ioctl(descriptor, LINUX_KVM_GET_API_VERSION))
    finally:
        os.close(descriptor)


def require_canonical_inrou_nss_identities() -> None:
    """Require the four locked local identities used by the same-host cohort."""

    allowed_shells = {
        "/usr/sbin/nologin",
        "/sbin/nologin",
        "/usr/bin/false",
        "/bin/false",
    }
    for slot in range(TAIRA_INROU_IDENTITY_SLOTS):
        name = f"{TAIRA_INROU_IDENTITY_NAME_PREFIX}{slot}"
        identifier = TAIRA_INROU_IDENTITY_BASE + slot
        try:
            named_user = pwd.getpwnam(name)
            numbered_user = pwd.getpwuid(identifier)
            named_group = grp.getgrnam(name)
            numbered_group = grp.getgrgid(identifier)
        except KeyError as error:
            fail(
                "Taira Inrou V1 qualification requires canonical local NSS "
                f"identity {name} with uid/gid {identifier}: {error}"
            )
        if (
            named_user.pw_name != name
            or named_user.pw_uid != identifier
            or named_user.pw_gid != identifier
            or numbered_user.pw_name != name
            or numbered_user.pw_uid != identifier
            or numbered_user.pw_gid != identifier
            or named_user.pw_dir != "/nonexistent"
            or named_user.pw_shell not in allowed_shells
        ):
            fail(
                f"canonical local NSS user {name} must be uid/gid {identifier}, "
                "home /nonexistent, and locked behind nologin/false"
            )
        if (
            named_group.gr_name != name
            or named_group.gr_gid != identifier
            or numbered_group.gr_name != name
            or numbered_group.gr_gid != identifier
            or list(named_group.gr_mem)
            or list(numbered_group.gr_mem)
        ):
            fail(
                f"canonical local NSS group {name} must be empty with gid {identifier}"
            )
        for candidate in grp.getgrall():
            if candidate.gr_name != name and name in candidate.gr_mem:
                fail(
                    f"canonical local NSS user {name} must not belong to "
                    f"supplementary group {candidate.gr_name}"
                )


def require_inrou_qualification_host(
    *,
    system: str | None = None,
    machine: str | None = None,
    effective_uid: int | None = None,
    kvm_probe: Callable[[Path], int] | None = None,
    identity_probe: Callable[[], None] | None = None,
) -> None:
    """Reject hosts that cannot qualify the mandatory Taira PortableVM path."""

    system = platform.system() if system is None else system
    machine = platform.machine() if machine is None else machine
    effective_uid = os.geteuid() if effective_uid is None else effective_uid
    if system != "Linux":
        fail("Taira Inrou V1 qualification requires Linux")
    if machine.lower() != "aarch64":
        fail("Taira Inrou V1 qualification requires Linux AArch64")
    if effective_uid != 0:
        fail("Taira Inrou V1 qualification must start as uid 0")
    (require_canonical_inrou_nss_identities if identity_probe is None else identity_probe)()
    probe = _probe_linux_kvm_api_version if kvm_probe is None else kvm_probe
    try:
        api_version = probe(Path("/dev/kvm"))
    except (OSError, ValueError) as error:
        fail(f"Taira Inrou V1 qualification cannot use /dev/kvm: {error}")
    if api_version != LINUX_KVM_API_VERSION:
        fail(
            "Taira Inrou V1 qualification requires Linux KVM API version "
            f"{LINUX_KVM_API_VERSION}, found {api_version}"
        )


Runner = Callable[..., subprocess.CompletedProcess[str]]
Request = Callable[[str, object | None], tuple[int, object | None]]


def run_command(
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: float | None = None,
    capture_output: bool = True,
    pass_fds: Sequence[int] = (),
) -> subprocess.CompletedProcess[str]:
    """Run one command and surface its useful trailing diagnostics."""

    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            env=env,
            timeout=timeout,
            capture_output=capture_output,
            text=True,
            errors="surrogateescape",
            check=False,
            pass_fds=tuple(pass_fds),
        )
    except subprocess.TimeoutExpired:
        fail(f"{Path(command[0]).name} timed out after {timeout:g}s")
    except (OSError, UnicodeError) as error:
        fail(f"cannot execute {Path(command[0]).name}: {error}")
    if completed.returncode != 0:
        stderr = completed.stderr or ""
        stdout = completed.stdout or ""
        detail = (stderr.strip() or stdout.strip())[-6000:]
        fail(f"{Path(command[0]).name} failed: {detail or completed.returncode}")
    return completed


def submitted_transaction_hash(completed: subprocess.CompletedProcess[str]) -> str:
    """Extract the raw 32-byte hash accepted by ``iroha tx status``."""

    try:
        payload = json_loads_no_duplicates(completed.stdout or "")
    except (TypeError, ValueError):
        fail("signed ping did not return its JSON transaction receipt")
    if not isinstance(payload, dict) or set(payload) != {
        "hash",
        "transaction",
        "fee_quote",
    }:
        fail("signed ping transaction receipt violates the exact V1 schema")
    if not isinstance(payload["transaction"], dict) or not isinstance(
        payload["fee_quote"], dict
    ):
        fail("signed ping transaction receipt violates the exact V1 schema")
    _validate_fee_quote_v1(payload["fee_quote"], "signed ping receipt.fee_quote")
    value = payload["hash"]
    match = (
        re.fullmatch(r"hash:([0-9a-f]{63}[13579bdf])#[0-9A-F]{4}", value)
        if isinstance(value, str)
        else None
    )
    if match is None or not is_canonical_iroha_hash_hex(match.group(1)):
        fail("signed ping returned an invalid transaction hash")
    return match.group(1)


def require_applied_transaction(
    completed: subprocess.CompletedProcess[str], expected_hash: str
) -> None:
    """Require the typed pipeline waiter to confirm the submitted transaction."""

    try:
        payload = json_loads_no_duplicates(completed.stdout or "")
    except (TypeError, ValueError):
        fail("transaction status waiter did not return JSON")
    expected_fields = {
        "hash",
        "terminal_kind",
        "attempts",
        "elapsed_ms",
        "block_height",
        "scope",
        "resolved_from",
        "final",
    }
    if not isinstance(payload, dict) or set(payload) != expected_fields:
        fail("transaction status waiter violates the exact V1 schema")
    actual_hash = payload["hash"]
    terminal_kind = payload["terminal_kind"]
    attempts = payload["attempts"]
    elapsed_ms = payload["elapsed_ms"]
    block_height = payload["block_height"]
    final = payload["final"]
    integer_fields_are_exact = (
        isinstance(attempts, int)
        and not isinstance(attempts, bool)
        and attempts > 0
        and isinstance(elapsed_ms, int)
        and not isinstance(elapsed_ms, bool)
        and elapsed_ms >= 0
        and isinstance(block_height, int)
        and not isinstance(block_height, bool)
        and block_height > 0
    )
    final_is_exact = (
        isinstance(final, dict)
        and set(final) == {"hash", "status", "scope", "resolved_from"}
        and final.get("hash") == expected_hash
        and final.get("scope") == "global"
        and final.get("resolved_from") == "state"
        and isinstance(final.get("status"), dict)
        and set(final["status"]) == {"kind", "block_height"}
        and final["status"].get("kind") == "Applied"
        and final["status"].get("block_height") == block_height
    )
    if (
        not is_canonical_iroha_hash_hex(actual_hash)
        or actual_hash != expected_hash
        or terminal_kind != "Applied"
        or payload["scope"] != "global"
        or payload["resolved_from"] != "state"
        or not integer_fields_are_exact
        or not final_is_exact
    ):
        fail("signed ping did not reach Applied pipeline finality")


def require_executable(path: Path) -> Path:
    """Require one regular executable file."""

    path = path.expanduser().absolute()
    if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
        fail(f"required executable is unavailable: {path}")
    return path


def managed_root(path: Path, *, create: bool) -> Path:
    """Resolve a narrowly marked directory owned by this script."""

    path = path.expanduser().absolute()
    resolved = path.resolve(strict=False)
    if path != resolved:
        fail(f"refusing non-direct devnet directory path: {path}")
    path = resolved
    forbidden = {Path("/"), REPO_ROOT, Path.home().absolute()}
    if path in forbidden:
        fail(f"refusing unsafe devnet directory: {path}")
    parent = path.parent
    try:
        parent_metadata = parent.lstat()
    except OSError as error:
        fail(f"cannot inspect devnet parent directory {parent}: {error}")
    if (
        stat.S_ISLNK(parent_metadata.st_mode)
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != os.geteuid()
        or parent_metadata.st_mode & 0o022
        or parent.resolve(strict=True) != parent
    ):
        fail(
            "devnet parent must be one direct directory owned by effective uid "
            f"{os.geteuid()} and non-writable by group/other: {parent}"
        )
    marker = path / MARKER
    if not path.exists():
        if not create:
            fail(f"no Taira devnet exists at {path}; run `up` first")
        try:
            path.mkdir(mode=0o700)
        except OSError as error:
            fail(f"cannot create devnet directory {path}: {error}")
    if not path.is_dir():
        fail(f"devnet path is not a directory: {path}")
    root_metadata = path.lstat()
    if root_metadata.st_uid != os.geteuid() or root_metadata.st_mode & 0o022:
        fail(
            "devnet directory must be owned by effective uid "
            f"{os.geteuid()} and non-writable by group/other: {path}"
        )
    if marker.exists() or marker.is_symlink():
        if marker.is_symlink() or read_bounded_text(
            marker,
            limit=MAX_MARKER_BYTES,
            label="devnet marker",
        ) != MARKER_BODY:
            fail(f"invalid devnet marker: {marker}")
        marker_metadata = marker.lstat()
        if (
            not stat.S_ISREG(marker_metadata.st_mode)
            or marker_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(marker_metadata.st_mode) != 0o600
            or marker_metadata.st_nlink != 1
        ):
            fail(f"devnet marker lacks direct owner-only custody: {marker}")
    elif any(path.iterdir()):
        fail(f"refusing unmarked non-empty directory: {path}")
    elif create:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(marker, flags, 0o600)
        try:
            os.fchmod(descriptor, 0o600)
            body = MARKER_BODY.encode("utf-8")
            written = os.write(descriptor, body)
            if written != len(body):
                fail(f"could not write complete devnet marker: {marker}")
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    else:
        fail(f"devnet marker is missing: {marker}")
    path.chmod(0o700)
    return path.resolve(strict=True)


def network_dir(root: Path) -> Path:
    """Return the sole disposable network directory below a managed root."""

    return root / "network"


def require_network_bundle(root: Path) -> Path:
    """Require the minimal generated files that identify this owned cohort."""

    target = network_dir(root)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    if not target.is_dir():
        fail(f"no generated Taira network exists at {target}; run `up` first")
    required = [
        target / "client.toml",
        target / "genesis.expected_hash",
        target / "start.sh",
        target / "stop.sh",
    ]
    required.extend(target / f"peer{index}.toml" for index in range(PEER_COUNT))
    for path in required:
        if path.is_symlink() or not path.is_file():
            fail(f"generated Taira network is incomplete: missing {path.name}")
    require_runtime_signer_files(target)
    return target


def runtime_signer_paths(target: Path) -> tuple[Path, ...]:
    """Return the four fixed runtime signer files without reading their contents."""

    return tuple(
        target / RUNTIME_SIGNER_DIRECTORY / f"peer{index}.private_key"
        for index in range(PEER_COUNT)
    )


def require_runtime_signer_files(target: Path) -> None:
    """Require four distinct owner-only single-link key files."""

    directory = target / RUNTIME_SIGNER_DIRECTORY
    if directory.is_symlink() or not directory.is_dir():
        fail(f"generated Taira runtime signer directory is missing: {directory}")
    identities: set[tuple[int, int]] = set()
    for path in runtime_signer_paths(target):
        if path.is_symlink():
            fail(f"refusing symlinked Taira runtime signer file: {path}")
        try:
            metadata = path.stat()
        except OSError as error:
            fail(f"cannot inspect Taira runtime signer file {path}: {error}")
        if (
            not path.is_file()
            or metadata.st_uid != os.geteuid()
            or metadata.st_mode & 0o7777 != 0o600
            or metadata.st_nlink != 1
            or metadata.st_size != RUNTIME_SIGNER_FILE_BYTES
        ):
            fail(f"untrusted Taira runtime signer file: {path}")
        identity = (metadata.st_dev, metadata.st_ino)
        if identity in identities:
            fail("Taira peers must not share a runtime signer file")
        identities.add(identity)


def _stable_file_identity(metadata: os.stat_result) -> tuple[int, int, int, int, int]:
    """Return the pathname and descriptor fields that must stay fixed while reading."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _acquire_operation_lock(root: Path) -> int:
    """Hold the managed marker so concurrent commands cannot race cleanup."""

    marker = root / MARKER
    try:
        before = marker.lstat()
        descriptor = os.open(
            marker,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as error:
        fail(f"cannot open the Taira devnet operation lock {marker}: {error}")
    try:
        if _stable_file_identity(os.fstat(descriptor)) != _stable_file_identity(before):
            fail(f"Taira devnet marker changed while opening its operation lock: {marker}")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            fail(f"another Taira devnet operation is already active under {root}")
        after = marker.lstat()
        if _stable_file_identity(after) != _stable_file_identity(before):
            fail(f"Taira devnet marker changed while acquiring its operation lock: {marker}")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def read_stable_bytes(
    path: Path,
    *,
    limit: int,
    label: str,
    owner: int | None = None,
    exact_mode: int | None = None,
    require_nonempty: bool = False,
) -> bytes:
    """Read one bounded regular file through a stable no-follow descriptor."""

    try:
        before = path.lstat()
    except OSError as error:
        fail(f"{label} is missing or not a regular file: {path}: {error}")
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        fail(f"{label} is missing or not a direct single-link regular file: {path}")
    if owner is not None and before.st_uid != owner:
        fail(f"{label} is not owned by uid {owner}: {path}")
    if exact_mode is not None and stat.S_IMODE(before.st_mode) != exact_mode:
        fail(f"{label} does not have mode {exact_mode:04o}: {path}")
    if before.st_size > limit:
        fail(f"{label} exceeds the {limit}-byte safety bound: {path}")
    if require_nonempty and before.st_size == 0:
        fail(f"{label} is empty: {path}")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open {label} {path}: {error}")
    try:
        opened = os.fstat(descriptor)
        payload = bytearray()
        while len(payload) <= limit:
            try:
                chunk = os.read(descriptor, min(64 * 1024, limit + 1 - len(payload)))
            except OSError as error:
                fail(f"cannot read {label} {path}: {error}")
            if not chunk:
                break
            payload.extend(chunk)
        after_open = os.fstat(descriptor)
    finally:
        try:
            os.close(descriptor)
        except OSError as error:
            fail(f"cannot close {label} {path}: {error}")
    try:
        after_path = path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect {label} {path}: {error}")
    identity = _stable_file_identity(before)
    if (
        _stable_file_identity(opened) != identity
        or _stable_file_identity(after_open) != identity
        or _stable_file_identity(after_path) != identity
        or len(payload) != before.st_size
    ):
        fail(f"{label} changed while it was read: {path}")
    if len(payload) > limit:
        fail(f"{label} exceeds the {limit}-byte safety bound: {path}")
    return bytes(payload)


def read_bounded_text(path: Path, *, limit: int, label: str) -> str:
    """Read one stable regular UTF-8 bundle file within an exact byte bound."""

    try:
        return read_stable_bytes(path, limit=limit, label=label).decode("utf-8")
    except UnicodeDecodeError as error:
        fail(f"cannot read {label} {path}: {error}")


def quoted_assignment(path: Path, key: str) -> str:
    """Read one unique canonical quoted assignment from a generated TOML file."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    pattern = re.compile(rf'^\s*{re.escape(key)}\s*=\s*"([^"\\]*)"\s*$')
    values = [match.group(1) for line in text.splitlines() if (match := pattern.fullmatch(line))]
    if len(values) != 1:
        fail(f"generated config must contain one canonical {key} assignment: {path}")
    return values[0]


def _top_level_quoted_assignment_from_text(path: Path, text: str, key: str) -> str:
    """Parse one canonical quoted assignment before the first TOML table."""

    pattern = re.compile(rf'^\s*{re.escape(key)}\s*=\s*"([^"\\]*)"\s*$')
    values: list[str] = []
    for line in text.splitlines():
        if line.lstrip().startswith("["):
            break
        if match := pattern.fullmatch(line):
            values.append(match.group(1))
    if len(values) != 1:
        fail(f"generated config must contain one canonical top-level {key} assignment: {path}")
    return values[0]


def top_level_quoted_assignment(path: Path, key: str) -> str:
    """Read one canonical quoted assignment before the first TOML table."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    return _top_level_quoted_assignment_from_text(path, text, key)


def integer_assignment(path: Path, key: str) -> int:
    """Read one unique canonical non-negative integer assignment from TOML."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    pattern = re.compile(rf"^\s*{re.escape(key)}\s*=\s*(0|[1-9][0-9]*)\s*$")
    values = [
        int(match.group(1))
        for line in text.splitlines()
        if (match := pattern.fullmatch(line))
    ]
    if len(values) != 1:
        fail(f"generated config must contain one canonical {key} assignment: {path}")
    return values[0]


def require_bundle_identity(target: Path, roots: Sequence[str]) -> None:
    """Bind checks to the generated Taira chain and requested loopback ports."""

    client = target / "client.toml"
    if quoted_assignment(client, "chain") != DEFAULT_CHAIN_ID:
        fail(f"generated client config is not for canonical Taira: {client}")
    if (
        integer_assignment(client, "chain_discriminant")
        != DEFAULT_CHAIN_DISCRIMINANT
    ):
        fail(f"generated client config has the wrong Taira chain discriminant: {client}")
    if quoted_assignment(client, "torii_url") != roots[0]:
        fail(f"generated client Torii URL does not match requested ports: {client}")
    expected_hash = read_bounded_text(
        target / "genesis.expected_hash",
        limit=256,
        label="generated genesis hash",
    ).strip()
    try:
        expected_network_id = network_id_from_genesis_hash(expected_hash)
    except ValueError as error:
        fail(f"generated genesis hash is invalid: {target / 'genesis.expected_hash'}: {error}")
    if quoted_assignment(client, "network_id") != expected_network_id:
        fail(f"generated client network id does not match its genesis hash: {client}")

    for index, root in enumerate(roots):
        config = target / f"peer{index}.toml"
        if quoted_assignment(config, "chain") != DEFAULT_CHAIN_ID:
            fail(f"peer{index} config is not for canonical Taira: {config}")
        if (
            integer_assignment(config, "chain_discriminant")
            != DEFAULT_CHAIN_DISCRIMINANT
        ):
            fail(f"peer{index} config has the wrong Taira chain discriminant: {config}")
        if quoted_assignment(config, "expected_hash") != expected_network_id:
            fail(f"peer{index} config genesis hash does not match the generated bundle: {config}")
        port = root.removeprefix("http://127.0.0.1:").removesuffix("/")
        address = re.compile(
            rf'^address = "addr:127\.0\.0\.1:{re.escape(port)}#[0-9A-Fa-f]{{4}}"$'
        )
        text = read_bounded_text(
            config,
            limit=MAX_BUNDLE_TEXT_BYTES,
            label=f"peer{index} config",
        )
        if sum(address.fullmatch(line) is not None for line in text.splitlines()) != 1:
            fail(f"peer{index} Torii address does not match requested ports: {config}")


class LinuxPidfdProcessOps:
    """Linux-only process observations and control through stable pidfds."""

    def require_supported(self) -> None:
        if platform.system() != "Linux":
            fail("Taira process control requires Linux pidfds and procfs")
        if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
            fail("Taira process control requires pidfd_open and pidfd_send_signal")
        if not hasattr(os, "O_CLOEXEC") or not hasattr(os, "O_NOFOLLOW"):
            fail("Taira process control requires safe Linux procfs open flags")
        if not hasattr(select, "poll"):
            fail("Taira process control requires pollable pidfds")
        required = (
            Path("/proc"),
            Path("/proc/self/stat"),
            Path("/proc/self/status"),
            LINUX_BOOT_ID_PATH,
        )
        if any(not path.exists() for path in required):
            fail("Taira process control requires Linux procfs")
        try:
            descriptor = int(getattr(os, "pidfd_open")(os.getpid(), 0))
            try:
                getattr(signal, "pidfd_send_signal")(descriptor, 0, None, 0)
                poller = select.poll()
                poller.register(descriptor, select.POLLIN | select.POLLHUP)
                poller.poll(0)
            finally:
                os.close(descriptor)
        except (OSError, TypeError, ValueError) as error:
            fail(f"Taira process control cannot exercise native Linux pidfds: {error}")

    def open_pidfd(self, pid: int) -> int | None:
        self.require_supported()
        try:
            return int(getattr(os, "pidfd_open")(pid, 0))
        except ProcessLookupError:
            return None
        except OSError as error:
            if error.errno == errno.ESRCH:
                return None
            fail(f"cannot open pidfd for Taira process {pid}: {error}")

    def close_pidfd(self, descriptor: int) -> None:
        try:
            os.close(descriptor)
        except OSError as error:
            fail(f"cannot close Taira process pidfd: {error}")

    def send_signal(self, descriptor: int, signal_number: int) -> bool:
        self.require_supported()
        try:
            getattr(signal, "pidfd_send_signal")(
                descriptor,
                signal_number,
                None,
                0,
            )
            return True
        except ProcessLookupError:
            return False
        except OSError as error:
            if error.errno == errno.ESRCH:
                return False
            fail(f"cannot signal an exact Taira process through pidfd: {error}")

    def wait_readable(self, descriptor: int, timeout_seconds: float) -> bool:
        self.require_supported()
        if timeout_seconds < 0:
            fail("Taira pidfd wait timeout must be non-negative")
        poller = select.poll()
        poller.register(descriptor, select.POLLIN | select.POLLHUP)
        timeout_ms = min(int(timeout_seconds * 1000), 2_147_483_647)
        return bool(poller.poll(timeout_ms))

    @staticmethod
    def _read_proc(path: Path, limit: int) -> bytes:
        flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW
        descriptor = os.open(path, flags)
        try:
            payload = bytearray()
            while len(payload) <= limit:
                chunk = os.read(descriptor, min(16 * 1024, limit + 1 - len(payload)))
                if not chunk:
                    break
                payload.extend(chunk)
        finally:
            os.close(descriptor)
        if len(payload) > limit:
            fail(f"Linux procfs record exceeds its Taira safety bound: {path}")
        return bytes(payload)

    @staticmethod
    def _parse_stat(payload: bytes, pid: int) -> tuple[str, int, int, int]:
        try:
            suffix = payload.decode("ascii").rstrip("\n").rsplit(") ", 1)[1]
            fields = suffix.split()
            state = fields[0]
            process_group_id = int(fields[2])
            session_id = int(fields[3])
            start_time_ticks = int(fields[19])
        except (IndexError, UnicodeDecodeError, ValueError):
            fail(f"Linux procfs stat is malformed for Taira process {pid}")
        if len(state) != 1 or start_time_ticks <= 0:
            fail(f"Linux procfs stat is malformed for Taira process {pid}")
        return state, process_group_id, session_id, start_time_ticks

    @staticmethod
    def _parse_status(payload: bytes, pid: int) -> tuple[int, int]:
        try:
            text = payload.decode("ascii")
        except UnicodeDecodeError:
            fail(f"Linux procfs status is malformed for Taira process {pid}")
        values: dict[str, tuple[int, ...]] = {}
        for label in ("Uid", "Gid"):
            prefix = f"{label}:"
            rows = [line for line in text.splitlines() if line.startswith(prefix)]
            if len(rows) != 1:
                fail(f"Linux procfs status is malformed for Taira process {pid}")
            try:
                parsed = tuple(int(value) for value in rows[0][len(prefix) :].split())
            except ValueError:
                fail(f"Linux procfs status is malformed for Taira process {pid}")
            if len(parsed) != 4:
                fail(f"Linux procfs status is malformed for Taira process {pid}")
            values[label] = parsed
        return values["Uid"][1], values["Gid"][1]

    def observe(self, pid: int) -> dict[str, object] | None:
        self.require_supported()
        proc_root = Path("/proc") / str(pid)
        try:
            stat_before = self._read_proc(proc_root / "stat", 16 * 1024)
            state, process_group_id, session_id, start_time_ticks = self._parse_stat(
                stat_before,
                pid,
            )
            if state == "Z":
                return None
            cmdline = self._read_proc(proc_root / "cmdline", 64 * 1024)
            if not cmdline or not cmdline.endswith(b"\0"):
                return None
            raw_argv = cmdline[:-1].split(b"\0")
            if not raw_argv or any(not argument for argument in raw_argv):
                return None
            argv = [os.fsdecode(argument) for argument in raw_argv]
            executable_path = os.readlink(proc_root / "exe")
            executable = os.stat(proc_root / "exe")
            uid, gid = self._parse_status(
                self._read_proc(proc_root / "status", 128 * 1024),
                pid,
            )
            stat_after = self._read_proc(proc_root / "stat", 16 * 1024)
            cmdline_after = self._read_proc(proc_root / "cmdline", 64 * 1024)
            executable_path_after = os.readlink(proc_root / "exe")
            executable_after = os.stat(proc_root / "exe")
            uid_after, gid_after = self._parse_status(
                self._read_proc(proc_root / "status", 128 * 1024),
                pid,
            )
            if (
                self._parse_stat(stat_after, pid)
                != (state, process_group_id, session_id, start_time_ticks)
                or cmdline_after != cmdline
                or executable_path_after != executable_path
                or (executable_after.st_dev, executable_after.st_ino)
                != (executable.st_dev, executable.st_ino)
                or (uid_after, gid_after) != (uid, gid)
            ):
                fail(f"Taira process {pid} changed while observing procfs")
            boot_id = self._read_proc(LINUX_BOOT_ID_PATH, 128).decode("ascii").strip()
        except FileNotFoundError:
            return None
        except UnicodeDecodeError:
            fail(f"Linux boot identity is malformed while observing Taira process {pid}")
        return {
            "pid": pid,
            "boot_id": boot_id,
            "start_time_ticks": start_time_ticks,
            "executable_path": executable_path,
            "executable_device": executable.st_dev,
            "executable_inode": executable.st_ino,
            "argv": argv,
            "uid": uid,
            "gid": gid,
            "session_id": session_id,
            "process_group_id": process_group_id,
        }

    def process_ids(self) -> tuple[int, ...]:
        self.require_supported()
        try:
            return tuple(
                sorted(
                    int(entry.name)
                    for entry in Path("/proc").iterdir()
                    if entry.name.isdigit() and int(entry.name) > 1
                )
            )
        except OSError as error:
            fail(f"cannot enumerate Linux procfs for Taira process control: {error}")


_PROCESS_OPS: Any = LinuxPidfdProcessOps()


def _taira_process_record_path(target: Path, peer_index: int) -> Path:
    """Return one exact first-release Taira process-record path."""

    return target / f"peer{peer_index}.process.json"


def _reject_legacy_taira_pidfiles(target: Path) -> None:
    """Reject retired bare PID files without migrating or deleting them."""

    retired = sorted(path.name for path in target.glob("peer*.pid"))
    if retired:
        fail("retired Taira PID files are unsupported: " + ", ".join(retired))


def _canonical_taira_process_identity(
    value: object,
    target: Path,
    peer_index: int,
    *,
    context: str,
) -> dict[str, object]:
    """Validate one exact V1 process identity against its peer slot."""

    if not isinstance(value, dict) or set(value) != TAIRA_PROCESS_RECORD_KEYS_V1:
        fail(f"{context} violates the exact Taira process V1 schema")
    if (
        type(value.get("schema_version")) is not int
        or value["schema_version"] != TAIRA_PROCESS_RECORD_SCHEMA_VERSION_V1
    ):
        fail(f"{context} has the wrong schema version")
    if type(value.get("peer_index")) is not int or value["peer_index"] != peer_index:
        fail(f"{context} has the wrong peer index")
    pid = value.get("pid")
    integer_fields = {
        "pid": pid,
        "start_time_ticks": value.get("start_time_ticks"),
        "executable_device": value.get("executable_device"),
        "executable_inode": value.get("executable_inode"),
        "uid": value.get("uid"),
        "gid": value.get("gid"),
        "session_id": value.get("session_id"),
        "process_group_id": value.get("process_group_id"),
    }
    if any(type(item) is not int for item in integer_fields.values()):
        fail(f"{context} contains a non-integer process identity field")
    if (
        not 1 < pid <= 2_147_483_647
        or not 0 < integer_fields["start_time_ticks"] <= 18_446_744_073_709_551_615
        or not 0 <= integer_fields["executable_device"] <= 18_446_744_073_709_551_615
        or not 0 < integer_fields["executable_inode"] <= 18_446_744_073_709_551_615
        or not 0 <= integer_fields["uid"] <= 4_294_967_295
        or not 0 <= integer_fields["gid"] <= 4_294_967_295
        or integer_fields["session_id"] != pid
        or integer_fields["process_group_id"] != pid
    ):
        fail(f"{context} contains a malformed process identity")
    boot_id = value.get("boot_id")
    if not isinstance(boot_id, str) or LOWER_BOOT_ID_RE.fullmatch(boot_id) is None:
        fail(f"{context} contains a malformed Linux boot identity")
    executable_path = value.get("executable_path")
    if (
        not isinstance(executable_path, str)
        or not executable_path
        or "\0" in executable_path
        or not Path(executable_path).is_absolute()
        or Path(executable_path).name != "iroha3d_taira"
        or os.path.normpath(executable_path) != executable_path
    ):
        fail(f"{context} contains a malformed executable path")
    expected_argv = [
        executable_path,
        "--sora",
        "--config",
        str(target / f"peer{peer_index}.toml"),
    ]
    if value.get("argv") != expected_argv:
        fail(f"{context} does not bind the exact Taira daemon argv/config identity")
    return {
        key: list(item) if key == "argv" else item
        for key, item in value.items()
    }


def read_taira_process_identity(target: Path, peer_index: int) -> dict[str, object]:
    """Read one canonical owner-only V1 Taira process record."""

    path = _taira_process_record_path(target, peer_index)
    payload = read_stable_bytes(
        path,
        limit=MAX_PROCESS_RECORD_BYTES,
        label="Taira process record",
        owner=os.geteuid(),
        exact_mode=0o600,
        require_nonempty=True,
    )
    try:
        value = json_loads_no_duplicates(payload.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        fail(f"Taira process record is not canonical JSON: {path}")
    canonical = (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")
    if canonical != payload:
        fail(f"Taira process record is not canonical JSON: {path}")
    return _canonical_taira_process_identity(
        value,
        target,
        peer_index,
        context=f"peer{peer_index} process record",
    )


def _bind_taira_process_identity(
    identity: dict[str, object],
    *,
    ops: Any = None,
) -> int | None:
    """Bind a persisted identity to one pidfd without ever signaling by PID."""

    ops = _PROCESS_OPS if ops is None else ops
    descriptor = ops.open_pidfd(identity["pid"])
    if descriptor is None:
        return None
    keep = False
    try:
        if not ops.send_signal(descriptor, 0):
            return None
        observed = ops.observe(identity["pid"])
        if not ops.send_signal(descriptor, 0) or observed is None:
            return None
        if (
            observed.get("boot_id") != identity["boot_id"]
            or observed.get("start_time_ticks") != identity["start_time_ticks"]
        ):
            return None
        expected_runtime = {
            key: identity[key] for key in TAIRA_PROCESS_RECORD_RUNTIME_KEYS_V1
        }
        if observed != expected_runtime:
            fail(
                f"Taira process {identity['pid']} retained its incarnation but drifted "
                "from its executable or argv identity"
            )
        keep = True
        return descriptor
    finally:
        if not keep:
            ops.close_pidfd(descriptor)


def _matching_taira_processes(target: Path, *, ops: Any = None) -> dict[int, list[int]]:
    """Discover matching daemon argvs through pidfd-stabilized procfs reads."""

    ops = _PROCESS_OPS if ops is None else ops
    matches = {index: [] for index in range(PEER_COUNT)}
    expected_configs = {
        str(target / f"peer{index}.toml"): index for index in range(PEER_COUNT)
    }
    for pid in ops.process_ids():
        descriptor = ops.open_pidfd(pid)
        if descriptor is None:
            continue
        try:
            if not ops.send_signal(descriptor, 0):
                continue
            observed = ops.observe(pid)
            if not ops.send_signal(descriptor, 0) or observed is None:
                continue
            argv = observed.get("argv")
            if (
                not isinstance(argv, list)
                or len(argv) != 4
                or Path(str(argv[0])).name != "iroha3d_taira"
                or argv[1:3] != ["--sora", "--config"]
            ):
                continue
            peer_index = expected_configs.get(argv[3])
            if peer_index is not None:
                matches[peer_index].append(pid)
        finally:
            ops.close_pidfd(descriptor)
    return matches


def generated_script_sha256(path: Path) -> str:
    """Hash one stable owner-held generated lifecycle script."""

    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"generated lifecycle script is missing: {path}: {error}")
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
        or metadata.st_mode & 0o111 == 0
    ):
        fail(f"generated lifecycle script lacks direct executable custody: {path}")
    payload = read_stable_bytes(
        path,
        limit=MAX_BUNDLE_TEXT_BYTES,
        label="generated lifecycle script",
        owner=os.geteuid(),
        require_nonempty=True,
    )
    return hashlib.sha256(payload).hexdigest()


def require_running_cohort(
    target: Path,
    run: Runner,
) -> tuple[dict[str, object], ...]:
    """Require four exact record-bound processes through held Linux pidfds."""

    del run
    _PROCESS_OPS.require_supported()
    _reject_legacy_taira_pidfiles(target)
    identities: list[dict[str, object]] = []
    descriptors: list[int] = []
    try:
        for index in range(PEER_COUNT):
            config = target / f"peer{index}.toml"
            if config.is_symlink() or not config.is_file():
                fail(f"generated peer config is missing or unsafe: {config}")
            identity = read_taira_process_identity(target, index)
            descriptor = _bind_taira_process_identity(identity)
            if descriptor is None:
                fail(f"peer{index} process record does not bind a live exact process")
            identities.append(identity)
            descriptors.append(descriptor)
        pids = [identity["pid"] for identity in identities]
        if len(set(pids)) != PEER_COUNT:
            fail("generated peer process records do not identify four distinct processes")
        matches = _matching_taira_processes(target)
        for index, identity in enumerate(identities):
            if matches[index] != [identity["pid"]]:
                fail(
                    f"peer{index} process record is not the sole running process for its "
                    "generated config"
                )
        return tuple(identities)
    finally:
        for descriptor in descriptors:
            _PROCESS_OPS.close_pidfd(descriptor)


def require_stoppable_cohort(
    target: Path,
    run: Runner,
) -> tuple[dict[str, object], ...]:
    """Preflight a complete record cohort, allowing an already-exited incarnation."""

    del run
    _PROCESS_OPS.require_supported()
    _reject_legacy_taira_pidfiles(target)
    identities: list[dict[str, object]] = []
    descriptors: list[int | None] = []
    try:
        for index in range(PEER_COUNT):
            identity = read_taira_process_identity(target, index)
            identities.append(identity)
            descriptors.append(_bind_taira_process_identity(identity))
        if len({identity["pid"] for identity in identities}) != PEER_COUNT:
            fail("generated peer process records do not identify four distinct processes")
        matches = _matching_taira_processes(target)
        for index, (identity, descriptor) in enumerate(
            zip(identities, descriptors, strict=True)
        ):
            expected = [identity["pid"]] if descriptor is not None else []
            if matches[index] != expected:
                fail(
                    f"peer{index} process record does not exclusively bind the process "
                    "using its generated config"
                )
        return tuple(identities)
    finally:
        for descriptor in descriptors:
            if descriptor is not None:
                _PROCESS_OPS.close_pidfd(descriptor)


def require_partial_restart_cohort(
    target: Path,
    restarted_peer_index: int,
    expected_processes: Sequence[dict[str, object]],
    run: Runner,
) -> None:
    """Require only the selected restart peer to be absent from its exact cohort."""

    if restarted_peer_index not in range(PEER_COUNT):
        fail("selected restart peer index is outside the exact four-peer cohort")
    if (
        len(expected_processes) != PEER_COUNT
    ):
        fail("selected restart cleanup has malformed original process identities")
    del run
    _PROCESS_OPS.require_supported()
    _reject_legacy_taira_pidfiles(target)
    expected = tuple(
        _canonical_taira_process_identity(
            identity,
            target,
            index,
            context="selected restart original process identity",
        )
        for index, identity in enumerate(expected_processes)
    )
    selected_record = _taira_process_record_path(target, restarted_peer_index)
    if selected_record.exists() or selected_record.is_symlink():
        fail(f"selected restart peer still has a process record: {selected_record}")

    descriptors: list[int] = []
    try:
        matches = _matching_taira_processes(target)
        for index in range(PEER_COUNT):
            if index == restarted_peer_index:
                if matches[index]:
                    fail(f"selected restart peer{index} process is still running: {matches[index]}")
                continue
            identity = read_taira_process_identity(target, index)
            if identity != expected[index]:
                fail(
                    f"non-selected peer{index} changed while peer{restarted_peer_index} stopped"
                )
            descriptor = _bind_taira_process_identity(identity)
            if descriptor is None or matches[index] != [identity["pid"]]:
                fail(
                    f"non-selected peer{index} changed while peer{restarted_peer_index} stopped"
                )
            descriptors.append(descriptor)
    finally:
        for descriptor in descriptors:
            _PROCESS_OPS.close_pidfd(descriptor)


def stop_peer_for_restart(
    target: Path,
    peer_index: int,
    expected_processes: Sequence[dict[str, object]],
    run: Runner,
) -> None:
    """Stop exactly one owned peer with the generated selector and prove absence."""

    if require_running_cohort(target, run) != tuple(expected_processes):
        fail("Taira cohort process identities changed before the selected Inrou host restart")
    stop = target / "stop.sh"
    generated_script_sha256(stop)
    run(
        ["/bin/bash", str(stop), "--peer-index", str(peer_index)],
        cwd=target,
        timeout=30,
    )
    require_partial_restart_cohort(target, peer_index, expected_processes, run)


def start_peer_after_restart(
    target: Path,
    peer_index: int,
    expected_processes: Sequence[dict[str, object]],
    env: dict[str, str],
    run: Runner,
) -> tuple[dict[str, object], ...]:
    """Relaunch one absent peer and prove only its process incarnation changed."""

    require_partial_restart_cohort(target, peer_index, expected_processes, run)
    start = target / "start.sh"
    generated_script_sha256(start)
    run(
        ["/bin/bash", str(start), "--peer-index", str(peer_index)],
        cwd=target,
        env=env,
        timeout=60,
        capture_output=False,
    )
    current = require_running_cohort(target, run)
    before = expected_processes[peer_index]
    after = current[peer_index]
    if after == before:
        fail("selected Inrou host restart retained its pre-restart process identity")
    if (
        after["pid"] == before["pid"]
        and after["start_time_ticks"] == before["start_time_ticks"]
    ):
        fail("selected Inrou host restart reused a PID without a new start time")
    for index in range(PEER_COUNT):
        if index != peer_index and current[index] != expected_processes[index]:
            fail(f"non-selected peer{index} process changed during selected host restart")
    return current


def require_stopped_cohort(target: Path, run: Runner) -> None:
    """Prove teardown left neither process records nor matching processes."""

    del run
    _PROCESS_OPS.require_supported()
    _reject_legacy_taira_pidfiles(target)
    records = sorted(path.name for path in target.glob("peer*.process.json"))
    if records:
        fail(f"Taira teardown left peer process records: {', '.join(records)}")
    matches = _matching_taira_processes(target)
    residual = sorted(pid for values in matches.values() for pid in values)
    if residual:
        fail(f"Taira teardown left managed peer processes running: {residual}")


def stop_network(
    root: Path,
    run: Runner,
    *,
    tolerate_failure: bool = False,
    expected_partial_peer_index: int | None = None,
    expected_partial_processes: Sequence[dict[str, object]] | None = None,
) -> bool:
    """Stop only peers owned by the generated Kagami bundle."""

    try:
        _PROCESS_OPS.require_supported()
        target = network_dir(root)
        if target.is_symlink():
            fail(f"refusing symlinked network directory: {target}")
        if not target.exists():
            return True
        if not target.is_dir():
            fail(f"network path is not a directory: {target}")
        _reject_legacy_taira_pidfiles(target)
        record_paths = [
            _taira_process_record_path(target, index) for index in range(PEER_COUNT)
        ]
        present_record_paths = [
            path for path in record_paths if path.exists() or path.is_symlink()
        ]
        if not present_record_paths:
            require_stopped_cohort(target, run)
            return True
        if len(present_record_paths) == PEER_COUNT:
            # The generated stop script has process-control authority. Do not run
            # it until all four exact process records prove that the live cohort
            # is ours through held pidfds.
            require_stoppable_cohort(target, run)
        else:
            if (
                expected_partial_peer_index is None
                or expected_partial_processes is None
            ):
                fail(
                    "Taira teardown found a partial process-record cohort: "
                    + ", ".join(path.name for path in present_record_paths)
                )
            expected_present = {
                _taira_process_record_path(target, index)
                for index in range(PEER_COUNT)
                if index != expected_partial_peer_index
            }
            if set(present_record_paths) != expected_present:
                fail(
                    "selected restart cleanup found an unexpected partial process-record "
                    "cohort: "
                    + ", ".join(path.name for path in present_record_paths)
                )
            require_partial_restart_cohort(
                target,
                expected_partial_peer_index,
                expected_partial_processes,
                run,
            )
        stop = target / "stop.sh"
        if stop.is_symlink() or not stop.is_file():
            fail(f"generated Taira network is incomplete: missing safe {stop.name}")
        run(["/bin/bash", str(stop)], cwd=stop.parent, timeout=30)
        require_stopped_cohort(target, run)
        return True
    except (DevnetError, subprocess.TimeoutExpired) as error:
        if not tolerate_failure:
            raise
        print(f"warning: could not prove Taira cohort stopped: {error}", file=sys.stderr)
        return False


def _direct_root_owned_directory_identity(
    path: Path,
    *,
    label: str,
    expected_owner: int = 0,
) -> tuple[int, int, int]:
    """Pin one direct root-owned directory by device and inode."""

    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"cannot inspect {label} {path}: {error}")
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != expected_owner
        or metadata.st_mode & 0o022
        or resolved != path
    ):
        fail(
            f"{label} must be one direct directory owned by uid {expected_owner} "
            f"and non-writable by group/other: {path}"
        )
    return metadata.st_dev, metadata.st_ino, metadata.st_uid


def _require_no_cleanup_mount_crossing(target: Path, expected_device: int) -> None:
    """Refuse a cleanup tree containing a filesystem or bind-mount boundary."""

    if os.path.ismount(target):
        fail(f"network cleanup target must not be a mount point: {target}")
    for current, directories, _files in os.walk(target, followlinks=False):
        current_path = Path(current)
        try:
            current_metadata = current_path.lstat()
        except OSError as error:
            fail(f"cannot inspect network cleanup directory {current_path}: {error}")
        if current_metadata.st_dev != expected_device:
            fail(f"network cleanup must not cross a filesystem boundary: {current_path}")
        for name in list(directories):
            child = current_path / name
            try:
                child_metadata = child.lstat()
            except OSError as error:
                fail(f"cannot inspect network cleanup entry {child}: {error}")
            if stat.S_ISLNK(child_metadata.st_mode):
                directories.remove(name)
                continue
            if child_metadata.st_dev != expected_device or os.path.ismount(child):
                fail(f"network cleanup must not cross a mount boundary: {child}")


def require_safe_cleanup_target(
    root: Path,
    target: Path,
    *,
    expected_owner: int | None = None,
) -> tuple[int, int, int] | None:
    """Validate the exact privileged tree before recursive replacement."""

    if expected_owner is None:
        expected_owner = os.geteuid()
    root_identity = _direct_root_owned_directory_identity(
        root,
        label="managed devnet root",
        expected_owner=expected_owner,
    )
    if target.parent != root or target.name != "network":
        fail(f"network cleanup target is outside the managed root: {target}")
    try:
        target_metadata = target.lstat()
    except FileNotFoundError:
        return None
    except OSError as error:
        fail(f"cannot inspect network cleanup target {target}: {error}")
    if (
        stat.S_ISLNK(target_metadata.st_mode)
        or not stat.S_ISDIR(target_metadata.st_mode)
        or target_metadata.st_uid != expected_owner
        or target_metadata.st_mode & 0o022
        or target.resolve(strict=True) != target
    ):
        fail(
            "network cleanup target must be one direct directory owned by uid "
            f"{expected_owner} and non-writable by group/other: {target}"
        )
    if target_metadata.st_dev != root_identity[0]:
        fail(f"network cleanup target must share the managed root filesystem: {target}")
    _require_no_cleanup_mount_crossing(target, root_identity[0])
    if not getattr(shutil.rmtree, "avoids_symlink_attacks", False):
        fail("platform recursive removal cannot protect the Taira cleanup tree")
    return target_metadata.st_dev, target_metadata.st_ino, target_metadata.st_uid


def destroy_network(
    root: Path,
    target: Path,
    expected_identity: tuple[int, int, int] | None,
) -> bool:
    """Quarantine and destroy one previously pinned, stopped network tree."""

    quarantine = root / NETWORK_CLEANUP_QUARANTINE
    if quarantine.exists() or quarantine.is_symlink():
        fail(f"a prior network cleanup quarantine requires recovery: {quarantine}")
    current_identity = require_safe_cleanup_target(root, target)
    if current_identity != expected_identity:
        fail(f"network cleanup target changed before destruction: {target}")
    if current_identity is None:
        return False
    try:
        os.rename(target, quarantine)
    except OSError as error:
        fail(f"cannot quarantine the proven network cleanup target {target}: {error}")
    fsync_directory(root, label="managed devnet root")
    quarantined_identity = _direct_root_owned_directory_identity(
        quarantine,
        label="network cleanup quarantine",
        expected_owner=current_identity[2],
    )
    if quarantined_identity != current_identity:
        fail(
            "network cleanup target changed while it was quarantined; preserved at "
            f"{quarantine}"
        )
    _require_no_cleanup_mount_crossing(quarantine, current_identity[0])
    shutil.rmtree(quarantine)
    fsync_directory(root, label="managed devnet root")
    if quarantine.exists() or quarantine.is_symlink():
        fail(f"network cleanup quarantine survived destruction: {quarantine}")
    if target.exists() or target.is_symlink():
        fail(f"a new network cleanup target appeared during destruction: {target}")
    return True


def reset_network(
    root: Path,
    run: Runner,
    expected_root_identity: tuple[int, int, int],
) -> Path:
    """Stop and replace the one script-owned throwaway network directory."""

    if (
        _direct_root_owned_directory_identity(
            root,
            label="managed devnet root",
            expected_owner=os.geteuid(),
        )
        != expected_root_identity
    ):
        fail(f"managed devnet root changed before network replacement: {root}")
    target = network_dir(root)
    target_identity = require_safe_cleanup_target(root, target)
    stop_network(root, run, tolerate_failure=False)
    destroy_network(root, target, target_identity)
    return target


def cargo_build_command(
    profile: str,
    target_dir: Path,
    target_triple: str,
) -> list[str]:
    """Return the mandatory current-workspace Taira qualification build."""

    command = [
        str(REPO_ROOT / "scripts" / "cargo_fast.sh"),
        "--target-dir",
        str(target_dir),
        "--no-sccache",
        "--",
        "build",
        "--locked",
        "--profile",
        profile,
        "--target",
        target_triple,
        "-p",
        "iroha_kagami",
        "--bin",
        "kagami",
        "-p",
        "irohad",
        "--bin",
        "iroha3d_taira",
        "-p",
        "iroha_cli",
        "--bin",
        "iroha",
        "-p",
        "sorafs_node",
        "--bin",
        "sorafs-node",
    ]
    return command


def cargo_build_env(rustc: Path) -> dict[str, str]:
    """Return an environment consistent with ``cargo_fast --no-sccache``."""

    env = os.environ.copy()
    for name in BUILD_ENV_REMOVALS:
        env.pop(name, None)
    env["RUSTC"] = str(rustc)
    return env


def rustc_host_target(run: Runner) -> tuple[Path, str]:
    """Select one native Linux/AArch64 compiler and its exact artifact triple."""

    rustc_name = shutil.which("rustc")
    if rustc_name is None:
        fail("Taira Inrou qualification requires rustc on PATH")
    # Preserve a rustup proxy. Resolving ``~/.cargo/bin/rustc`` commonly yields
    # the multi-call ``rustup`` binary, which is not a rustc invocation when its
    # basename changes and therefore cannot report or compile for the host.
    rustc = Path(rustc_name).expanduser().absolute()
    if not rustc.is_file() or not os.access(rustc, os.X_OK):
        fail(f"selected rustc is not executable: {rustc}")
    completed = run([str(rustc), "-vV"], cwd=REPO_ROOT, timeout=20)
    host_lines = [
        line.removeprefix("host:").strip()
        for line in (completed.stdout or "").splitlines()
        if line.startswith("host:")
    ]
    if len(host_lines) != 1:
        fail("rustc did not report one canonical host target")
    target_triple = host_lines[0]
    if LINUX_AARCH64_TARGET_RE.fullmatch(target_triple) is None:
        fail(
            "Taira Inrou qualification requires a native Linux/AArch64 Rust target, "
            f"found `{target_triple}`"
        )
    return rustc, target_triple


def qualification_target_dir(path: Path) -> tuple[Path, tuple[int, int]]:
    """Create or validate one direct owner-controlled Cargo target directory."""

    path = path.expanduser().absolute()
    resolved = path.resolve(strict=False)
    if path != resolved:
        fail(f"Taira qualification target directory must be a direct path: {path}")
    if path in {Path("/"), Path.home().absolute(), REPO_ROOT}:
        fail(f"refusing unsafe Taira qualification target directory: {path}")
    owner = os.geteuid()
    parent = path.parent
    try:
        parent_metadata = parent.lstat()
    except OSError as error:
        fail(f"cannot inspect Taira qualification target parent {parent}: {error}")
    if (
        stat.S_ISLNK(parent_metadata.st_mode)
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != owner
        or parent_metadata.st_mode & 0o022
        or parent.resolve(strict=True) != parent
    ):
        fail(
            "Taira qualification target parent must be one direct directory "
            f"owned by uid {owner} and non-writable by group/other: {parent}"
        )
    if not path.exists():
        try:
            path.mkdir(mode=0o700)
        except OSError as error:
            fail(f"cannot create Taira qualification target directory {path}: {error}")
    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"cannot inspect Taira qualification target directory {path}: {error}")
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != owner
        or metadata.st_mode & 0o022
        or path.resolve(strict=True) != path
    ):
        fail(
            "Taira qualification target must be one direct directory owned by uid "
            f"{owner} and non-writable by group/other: {path}"
        )
    return path, (metadata.st_dev, metadata.st_ino)


def qualification_bin_dir(
    target_dir: Path,
    target_triple: str,
    target_identity: tuple[int, int],
) -> tuple[Path, tuple[int, int]]:
    """Create and pin the direct target/profile ancestry used for output removal."""

    target_metadata = target_dir.lstat()
    if (target_metadata.st_dev, target_metadata.st_ino) != target_identity:
        fail("Taira qualification target directory changed before output custody")
    owner = os.geteuid()
    device = target_metadata.st_dev
    current = target_dir
    for component in (target_triple, TAIRA_BUILD_PROFILE):
        current = current / component
        if not current.exists() and not current.is_symlink():
            try:
                current.mkdir(mode=0o700)
            except OSError as error:
                fail(f"cannot create Taira qualification output directory {current}: {error}")
        try:
            metadata = current.lstat()
        except OSError as error:
            fail(f"cannot inspect Taira qualification output directory {current}: {error}")
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != owner
            or metadata.st_dev != device
            or metadata.st_mode & 0o022
            or current.resolve(strict=True) != current
        ):
            fail(
                "Taira qualification output ancestry must contain only direct directories "
                f"owned by uid {owner} on the target filesystem: {current}"
            )
    metadata = current.lstat()
    return current, (metadata.st_dev, metadata.st_ino)


def clear_qualification_binary(path: Path) -> None:
    """Remove one exact generated binary so Cargo must produce it in this invocation."""

    if not path.exists() and not path.is_symlink():
        return
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
    ):
        fail(f"refusing non-regular qualification binary output: {path}")
    path.unlink()


def _source_observation_field(observation: Any, name: bytes, value: bytes) -> None:
    """Feed one length-delimited field into the worktree-observation digest."""

    observation.update(len(name).to_bytes(4, "big"))
    observation.update(name)
    observation.update(len(value).to_bytes(8, "big"))
    observation.update(value)


def _untracked_source_content(path: Path, metadata: os.stat_result) -> tuple[bytes, bytes]:
    """Return one stable untracked entry type and content digest."""

    if stat.S_ISLNK(metadata.st_mode):
        try:
            target = os.fsencode(os.readlink(path))
            after = path.lstat()
        except OSError as error:
            fail(f"cannot inspect untracked source symlink {path}: {error}")
        if (after.st_dev, after.st_ino, after.st_mtime_ns, after.st_ctime_ns) != (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        ):
            fail(f"untracked source changed while hashing it: {path}")
        return b"symlink", hashlib.sha256(target).digest()
    if not stat.S_ISREG(metadata.st_mode):
        fail(f"untracked source is not a regular file or symlink: {path}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open untracked source {path}: {error}")
    digest = hashlib.sha256()
    try:
        try:
            opened = os.fstat(descriptor)
            if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino):
                fail(f"untracked source changed while opening it: {path}")
            with os.fdopen(descriptor, "rb", closefd=True) as stream:
                descriptor = -1
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
                after = os.fstat(stream.fileno())
        except OSError as error:
            fail(f"cannot hash untracked source {path}: {error}")
    finally:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError as error:
                fail(f"cannot close qualifying executable {path}: {error}")
    try:
        pathname_after = path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect untracked source {path}: {error}")
    if (
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        pathname_after.st_dev,
        pathname_after.st_ino,
        pathname_after.st_size,
        pathname_after.st_mtime_ns,
        pathname_after.st_ctime_ns,
    ) != (
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    ):
        fail(f"untracked source changed while hashing it: {path}")
    return b"file", digest.digest()


def current_source_observation(run: Runner) -> dict[str, str]:
    """Observe HEAD and the non-ignored worktree without claiming build custody.

    This digest is a pre/post race detector.  It cannot prove which inputs
    Cargo, rustc, build scripts, dependency caches, or repository/user Cargo
    configuration consumed, so the public report states that limitation
    explicitly instead of presenting the observation as source provenance.
    """

    branch = (
        run(
            ["git", "branch", "--show-current"],
            cwd=REPO_ROOT,
            timeout=20,
        ).stdout
        or ""
    ).strip()
    if branch != TAIRA_QUALIFICATION_BRANCH:
        fail(
            "Taira Inrou qualification requires branch "
            f"`{TAIRA_QUALIFICATION_BRANCH}`, found `{branch or 'detached HEAD'}`"
        )
    git_head = (
        run(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, timeout=20).stdout or ""
    ).strip()
    if LOWER_GIT_COMMIT_RE.fullmatch(git_head) is None:
        fail("Taira Inrou qualification could not resolve one canonical Git HEAD")
    tracked_diff = (
        run(
            ["git", "diff", "--binary", "--no-ext-diff", "HEAD", "--", "."],
            cwd=REPO_ROOT,
            timeout=60,
        ).stdout
        or ""
    )
    untracked_output = (
        run(
            ["git", "ls-files", "--others", "--exclude-standard", "-z"],
            cwd=REPO_ROOT,
            timeout=30,
        ).stdout
        or ""
    )
    untracked = sorted(path for path in untracked_output.split("\0") if path)
    observation = hashlib.sha256()
    _source_observation_field(
        observation,
        b"domain",
        b"iroha.taira.nonignored-worktree-observation.v1",
    )
    _source_observation_field(observation, b"git-head", git_head.encode("ascii"))
    _source_observation_field(
        observation,
        b"tracked-diff",
        tracked_diff.encode("utf-8", errors="surrogateescape"),
    )
    _source_observation_field(
        observation,
        b"untracked-count",
        len(untracked).to_bytes(8, "big"),
    )
    for relative in untracked:
        relative_path = Path(relative)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            fail(f"Git reported an unsafe untracked source path: {relative}")
        path = REPO_ROOT / relative_path
        try:
            metadata = path.lstat()
        except OSError as error:
            fail(f"cannot inspect untracked source {path}: {error}")
        entry_type, content_digest = _untracked_source_content(path, metadata)
        _source_observation_field(
            observation,
            b"untracked-path",
            relative.encode("utf-8", errors="surrogateescape"),
        )
        _source_observation_field(
            observation,
            b"untracked-mode",
            stat.S_IMODE(metadata.st_mode).to_bytes(4, "big"),
        )
        _source_observation_field(observation, b"untracked-type", entry_type)
        _source_observation_field(
            observation,
            b"untracked-size",
            metadata.st_size.to_bytes(8, "big"),
        )
        _source_observation_field(
            observation,
            b"untracked-content-sha256",
            content_digest,
        )
    return {
        "branch": branch,
        "git_head": git_head,
        "observation_scope": "git_head_tracked_diff_nonignored_untracked",
        "observed_nonignored_worktree_sha256": observation.hexdigest(),
        "cargo_source_consumption": "not_proven",
    }


def binary_paths(
    args: argparse.Namespace,
    run: Runner,
) -> tuple[Path, Path, Path, Path, str]:
    """Build the mandatory current-workspace qualification binaries."""

    target_dir, target_identity = qualification_target_dir(args.target_dir)
    rustc, target_triple = rustc_host_target(run)
    bin_dir, bin_dir_identity = qualification_bin_dir(
        target_dir,
        target_triple,
        target_identity,
    )
    names = ["kagami", "iroha3d_taira", "iroha", "sorafs-node"]
    for name in names:
        clear_qualification_binary(bin_dir / name)
    print(f"Building current Taira binaries ({TAIRA_BUILD_PROFILE})...", flush=True)
    run(
        cargo_build_command(
            TAIRA_BUILD_PROFILE,
            target_dir,
            target_triple,
        ),
        cwd=REPO_ROOT,
        env=cargo_build_env(rustc),
        timeout=args.build_timeout_seconds,
        capture_output=False,
    )
    if (
        qualification_bin_dir(target_dir, target_triple, target_identity)[1]
        != bin_dir_identity
    ):
        fail("Taira qualification output directory changed during the Cargo build")
    kagami, irohad, iroha = (
        require_executable(bin_dir / name)
        for name in ("kagami", "iroha3d_taira", "iroha")
    )
    sorafs_node = require_executable(bin_dir / "sorafs-node")
    return kagami, irohad, iroha, sorafs_node, target_triple


def executable_evidence(path: Path) -> dict[str, int | str]:
    """Hash one direct executable while proving its filesystem identity is stable."""

    path = require_executable(path)
    try:
        before = path.lstat()
    except OSError as error:
        fail(f"cannot inspect qualifying executable {path}: {error}")
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        fail(f"qualifying executable must be one direct single-link file: {path}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open qualifying executable {path}: {error}")
    digest = hashlib.sha256()
    try:
        try:
            opened = os.fstat(descriptor)
            if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
                fail(f"qualifying executable changed while opening it: {path}")
            with os.fdopen(descriptor, "rb", closefd=True) as stream:
                descriptor = -1
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
                after_read = os.fstat(stream.fileno())
        except OSError as error:
            fail(f"cannot hash qualifying executable {path}: {error}")
    finally:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError as error:
                fail(f"cannot close qualifying executable {path}: {error}")
    try:
        after = path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect qualifying executable {path}: {error}")
    identity_before = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    identity_opened = (
        after_read.st_dev,
        after_read.st_ino,
        after_read.st_size,
        after_read.st_mtime_ns,
        after_read.st_ctime_ns,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    if identity_before != identity_opened or identity_before != identity_after:
        fail(f"qualifying executable changed while hashing it: {path}")
    return {"sha256": digest.hexdigest(), "bytes": before.st_size}


def compiled_toolchain_evidence(
    kagami: Path,
    irohad: Path,
    iroha: Path,
    sorafs_node: Path,
) -> dict[str, dict[str, int | str]]:
    """Bind every current-workspace binary used by one qualification run."""

    binaries = {
        "kagami": kagami,
        "iroha3d_taira": irohad,
        "iroha": iroha,
        "sorafs-node": sorafs_node,
    }
    items = tuple(binaries.items())

    def inspect(item: tuple[str, Path]) -> tuple[str, dict[str, int | str]]:
        name, path = item
        return name, {"path": str(require_executable(path)), **executable_evidence(path)}

    return dict(parallel_map(items, inspect))


def help_has_option(help_text: str, option: str) -> bool:
    """Match one complete long option without accepting a longer lookalike."""

    return re.search(rf"(?<![\w-]){re.escape(option)}(?![\w-])", help_text) is not None


def preflight_cli_surfaces(
    kagami: Path,
    irohad: Path,
    iroha: Path,
    sorafs_node: Path,
    run: Runner,
    *,
    full_doctor: bool,
) -> None:
    """Prove every compiled command used by ``up`` before replacing a cohort."""

    binaries: dict[str, Path] = {
        "kagami": kagami,
        "iroha3d_taira": irohad,
        "iroha": iroha,
        "sorafs-node": sorafs_node,
    }
    surfaces = [*CLI_SURFACES, *INROU_CANARY_CLI_SURFACES]
    if full_doctor:
        surfaces.append(
            ("iroha", ("taira", "doctor"), ("--public-root", "--json"))
        )
    def validate_surface(
        surface_spec: tuple[str, tuple[str, ...], tuple[str, ...]],
    ) -> None:
        binary_name, subcommands, required_options = surface_spec
        command = [str(binaries[binary_name]), *subcommands, "--help"]
        completed = run(command, cwd=REPO_ROOT, timeout=20)
        help_text = "\n".join((completed.stdout or "", completed.stderr or ""))
        missing = [
            option for option in required_options if not help_has_option(help_text, option)
        ]
        if missing:
            surface = " ".join((binary_name, *subcommands))
            fail(
                f"compiled CLI surface `{surface}` is missing current options: "
                + ", ".join(missing)
            )

    parallel_map(surfaces, validate_surface)


def generate_network(
    target: Path,
    kagami: Path,
    api_port: int,
    p2p_port: int,
    block_cadence_ms: int,
    run: Runner,
) -> None:
    """Generate exactly one fresh-key, four-validator Taira network."""

    run(
        [
            str(kagami),
            "localnet",
            "--out-dir",
            str(target),
            "--sora-profile",
            "nexus",
            "--consensus-mode",
            "npos",
            "--peers",
            str(PEER_COUNT),
            "--bind-host",
            "127.0.0.1",
            "--public-host",
            "127.0.0.1",
            "--chain-id",
            DEFAULT_CHAIN_ID,
            "--base-api-port",
            str(api_port),
            "--base-p2p-port",
            str(p2p_port),
            "--block-cadence-ms",
            str(block_cadence_ms),
        ],
        cwd=REPO_ROOT,
        timeout=None,
        capture_output=False,
    )


def validate_configs(
    target: Path,
    irohad: Path,
    trusted_guest: TrustedInrouGuestArtifact,
    run: Runner,
) -> None:
    """Run the current daemon's offline validator for every generated peer."""

    require_canonical_taira_profiles(target, trusted_guest)
    def validate(index: int) -> None:
        config = target / f"peer{index}.toml"
        run(
            [
                str(irohad),
                "--sora",
                "--config",
                str(config),
                "--genesis-manifest-json",
                str(target / "genesis.json"),
                "--check-config",
            ],
            cwd=target,
            timeout=120,
        )

    parallel_map(tuple(range(PEER_COUNT)), validate)


def http_request(url: str, payload: object | None = None) -> tuple[int, object | None]:
    """Send one local Torii GET/JSON POST and decode JSON when present."""

    body = None if payload is None else json.dumps(payload).encode("utf-8")
    plain_text_probe = url.rstrip("/").endswith(("/health", "/readyz"))
    headers = {"Accept": "text/plain" if plain_text_probe else "application/json"}
    if body is not None:
        headers["Content-Type"] = "application/json"
        if url.rstrip("/").endswith("/v1/mcp"):
            headers["MCP-Protocol-Version"] = MCP_PROTOCOL_VERSION_V1
    request = urllib.request.Request(url, data=body, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=3) as response:
            status = response.status
            body = response.read(MAX_HTTP_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as error:
        return error.code, None
    except (OSError, ValueError):
        return 0, None
    if len(body) > MAX_HTTP_RESPONSE_BYTES:
        fail(f"HTTP response exceeds the {MAX_HTTP_RESPONSE_BYTES}-byte safety bound: {url}")
    if not body:
        return status, None
    try:
        return status, json_loads_no_duplicates(body)
    except (UnicodeDecodeError, ValueError):
        return status, body.decode("utf-8", errors="replace")


def torii_roots(api_port: int) -> tuple[str, ...]:
    """Return all four loopback Torii roots."""

    return tuple(f"http://127.0.0.1:{api_port + index}/" for index in range(PEER_COUNT))


def bundle_torii_roots(target: Path) -> tuple[str, ...]:
    """Derive the owned cohort's Torii roots from its generated client config."""

    client = target / "client.toml"
    value = quoted_assignment(client, "torii_url")
    match = re.fullmatch(r"http://127\.0\.0\.1:([0-9]{1,5})/", value)
    if match is None:
        fail(f"generated client Torii URL is not a canonical loopback root: {client}")
    base_port = int(match.group(1))
    if base_port == 0 or base_port + PEER_COUNT - 1 > 65_535:
        fail(f"generated client Torii port cannot address four peers: {client}")
    return torii_roots(base_port)


def read_height(root: str, request: Request) -> int:
    """Read the canonical committed block height from ``/status/blocks``."""

    status, payload = request(root + "status/blocks", None)
    if status != 200 or isinstance(payload, bool) or not isinstance(payload, int):
        fail(f"invalid /status/blocks response from {root} (HTTP {status})")
    return payload


def require_cluster_build_identity(
    roots: Sequence[str],
    expected_git_head: str,
    expected_target_triple: str,
    request: Request,
) -> None:
    """Require every running validator to expose the current Linux/AArch64 build."""

    def check(root: str) -> None:
        status, payload = request(root + "status", None)
        build = payload.get("build") if isinstance(payload, dict) else None
        git_commit_sha = build.get("git_commit_sha") if isinstance(build, dict) else None
        target_triple = build.get("target_triple") if isinstance(build, dict) else None
        if status != 200 or git_commit_sha != expected_git_head:
            fail(
                f"validator build identity does not match source HEAD at {root} "
                f"(HTTP {status})"
            )
        if target_triple != expected_target_triple:
            fail(f"validator build target does not match the native compiler at {root}")

    parallel_map(roots, check)


def require_cli_build_identity(
    target: Path,
    iroha: Path,
    expected_git_head: str,
    run: Runner,
    timeout_seconds: float,
) -> None:
    """Require the transaction client to advertise the same source HEAD."""

    completed = run(
        [
            str(iroha),
            "--machine",
            "-c",
            str(target / "client.toml"),
            "--output-format",
            "json",
            "tools",
            "version",
        ],
        cwd=target,
        timeout=timeout_seconds,
    )
    try:
        payload = json_loads_no_duplicates(completed.stdout or "")
    except (TypeError, ValueError):
        fail("current Iroha CLI did not return its JSON build identity")
    if (
        not isinstance(payload, dict)
        or set(payload) != {"client_git_sha", "client_version", "server_version"}
        or payload.get("client_git_sha") != expected_git_head
    ):
        fail("current Iroha CLI build identity does not match source HEAD")


def check_sumeragi_status(root: str, request: Request) -> bool:
    """Require the exact unauthenticated consensus-status contract when reachable."""

    url = root + "v1/sumeragi/status"
    status, _payload = request(url, None)
    if status == 0:
        return False
    if status != 401:
        fail(
            "Sumeragi status must enforce the current unauthenticated HTTP 401 "
            f"contract at {root} (HTTP {status})"
        )
    return True


def wait_for_cluster(
    roots: Sequence[str],
    timeout: float,
    request: Request,
    *,
    above: int | None = None,
) -> list[int]:
    """Wait for four ready peers at one converged height, optionally advanced."""

    deadline = time.monotonic() + timeout
    last = "not reachable"
    while time.monotonic() < deadline:
        # These probes ignore an unavailable/protected status route but make a
        # published fail-stop or watchdog blocker terminal immediately.  Keep
        # them outside the retryable readiness block so a serious consensus
        # diagnosis is not hidden behind a generic convergence timeout.
        if not all(
            parallel_map(roots, lambda root: check_sumeragi_status(root, request))
        ):
            last = "one or more Sumeragi status routes are not reachable yet"
            time.sleep(0.5)
            continue

        def ready_height(root: str) -> int:
            for endpoint in ("health", "readyz"):
                status, _ = request(root + endpoint, None)
                if not 200 <= status < 300:
                    fail(f"{root}{endpoint} returned HTTP {status}")
            return read_height(root, request)

        try:
            heights = parallel_map(roots, ready_height)
            if len(set(heights)) == 1 and (above is None or heights[0] > above):
                return heights
            last = f"heights={heights}, required_above={above}"
        except DevnetError as error:
            last = str(error)
        time.sleep(0.5)
    fail(f"four-peer cluster did not converge within {timeout:g}s: {last}")


def check_mcp(root: str, request: Request) -> None:
    """Verify the enabled MCP endpoint can initialize and list current tools."""

    url = root + "v1/mcp"
    initialize = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION_V1,
            "capabilities": {},
            "clientInfo": {"name": "taira-devnet-smoke", "version": "1"},
        },
    }
    status, initialized = request(url, initialize)
    initialized_result = (
        initialized.get("result") if isinstance(initialized, dict) else None
    )
    if (
        status != 200
        or not isinstance(initialized, dict)
        or initialized.get("jsonrpc") != "2.0"
        or initialized.get("id") != 1
        or "error" in initialized
        or not isinstance(initialized_result, dict)
        or initialized_result.get("protocolVersion") != MCP_PROTOCOL_VERSION_V1
    ):
        fail(f"MCP initialize failed at {url} (HTTP {status})")
    initialized_notification = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }
    status, notification_response = request(url, initialized_notification)
    if status != 202 or notification_response is not None:
        fail(f"MCP initialized notification failed at {url} (HTTP {status})")
    tools_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {},
    }
    status, tools_response = request(url, tools_request)
    result = tools_response.get("result") if isinstance(tools_response, dict) else None
    tools = result.get("tools") if isinstance(result, dict) else None
    if (
        status != 200
        or not isinstance(tools_response, dict)
        or tools_response.get("jsonrpc") != "2.0"
        or tools_response.get("id") != 2
        or "error" in tools_response
        or not isinstance(tools, list)
        or not tools
        or any(
            not isinstance(tool, dict)
            or not isinstance(tool.get("name"), str)
            or not tool["name"].startswith("iroha.")
            for tool in tools
        )
    ):
        fail(f"MCP tools/list returned no tools at {url} (HTTP {status})")


def check_all_mcp(roots: Sequence[str], request: Request) -> None:
    """Verify the live MCP handshake and curated tools on every validator."""

    parallel_map(roots, lambda root: check_mcp(root, request))


def run_full_doctor(target: Path, iroha: Path, root: str, run: Runner) -> None:
    """Run the broad public-product diagnostic only when explicitly requested."""

    run(
        [
            str(iroha),
            "-c",
            str(target / "client.toml"),
            "taira",
            "doctor",
            "--public-root",
            root.rstrip("/"),
            "--json",
        ],
        cwd=target,
        timeout=120,
    )


def _require_custodied_directory_chain(path: Path, *, label: str) -> Path:
    """Require a canonical direct chain unavailable to a less-privileged writer."""

    candidate = path.expanduser().absolute()
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve {label} {candidate}: {error}")
    if candidate != resolved:
        fail(f"{label} must use one canonical direct path without symlinks: {candidate}")
    effective_owner = os.geteuid()
    current = Path("/")
    for component in candidate.parts[1:]:
        current /= component
        try:
            metadata = current.lstat()
        except OSError as error:
            fail(f"cannot inspect {label} ancestor {current}: {error}")
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            fail(f"{label} ancestor must be one direct directory: {current}")
        if metadata.st_uid not in {0, effective_owner} or metadata.st_mode & 0o022:
            fail(
                f"{label} ancestor must be owned by root or uid {effective_owner} "
                f"and non-writable by group/other: {current}"
            )
    return candidate


def _require_owner_only_entry(
    path: Path,
    *,
    directory: bool,
    label: str,
) -> os.stat_result:
    """Require one direct euid-owned 0700 directory or 0600 regular file."""

    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"cannot inspect {label} {path}: {error}")
    expected_mode = 0o700 if directory else 0o600
    expected_kind = "directory" if directory else "regular file"
    is_expected_kind = (
        stat.S_ISDIR(metadata.st_mode)
        if directory
        else stat.S_ISREG(metadata.st_mode)
    )
    if stat.S_ISLNK(metadata.st_mode) or not is_expected_kind:
        fail(f"{label} must be one direct {expected_kind}: {path}")
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != expected_mode:
        fail(
            f"{label} must be owned by uid {os.geteuid()} with mode "
            f"{expected_mode:04o}: {path}"
        )
    if not directory and metadata.st_nlink != 1:
        fail(f"{label} must have exactly one hard link: {path}")
    return metadata


def _canary_file_identity(
    metadata: os.stat_result,
) -> tuple[int, int, int, int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_uid,
        stat.S_IMODE(metadata.st_mode),
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _canary_directory_identity(
    metadata: os.stat_result,
) -> tuple[int, int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_uid,
        stat.S_IMODE(metadata.st_mode),
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _stable_canary_input_digest(
    path: Path,
    expected: os.stat_result,
    *,
    limit: int,
    label: str,
) -> str:
    """Hash one direct file while pinning its pathname and descriptor identity."""

    expected_identity = _canary_file_identity(expected)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open {label} {path}: {error}")
    digest = hashlib.sha256()
    exact_bytes = 0
    try:
        try:
            opened = os.fstat(descriptor)
        except OSError as error:
            fail(f"cannot inspect opened {label} {path}: {error}")
        if _canary_file_identity(opened) != expected_identity:
            fail(f"{label} changed identity while it was opened: {path}")
        while True:
            try:
                chunk = os.read(descriptor, 1024 * 1024)
            except OSError as error:
                fail(f"cannot hash {label} {path}: {error}")
            if not chunk:
                break
            exact_bytes += len(chunk)
            if exact_bytes > limit:
                fail(f"{label} exceeds the fixed {limit}-byte bound: {path}")
            digest.update(chunk)
        try:
            after = os.fstat(descriptor)
        except OSError as error:
            fail(f"cannot re-inspect opened {label} {path}: {error}")
    finally:
        try:
            os.close(descriptor)
        except OSError as error:
            fail(f"cannot close {label} {path}: {error}")
    try:
        pathname_after = path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect {label} {path}: {error}")
    if (
        _canary_file_identity(after) != expected_identity
        or _canary_file_identity(pathname_after) != expected_identity
        or exact_bytes != expected.st_size
    ):
        fail(f"{label} changed while it was hashed: {path}")
    return digest.hexdigest()


def _canary_workspace_directory_evidence(
    workspace: Path,
) -> tuple[tuple[str, tuple[int, int, int, int, int, int]], ...]:
    directories = (
        (".", workspace),
        ("inrou", workspace / "inrou"),
        (
            INROU_CANARY_GUEST_DIRECTORY.as_posix(),
            workspace / INROU_CANARY_GUEST_DIRECTORY,
        ),
    )
    return tuple(
        (
            relative,
            _canary_directory_identity(
                _require_owner_only_entry(
                    directory,
                    directory=True,
                    label="Inrou canary workspace directory",
                )
            ),
        )
        for relative, directory in directories
    )


def _canary_content_digest(inputs: Sequence[InrouCanaryInputEvidence]) -> str:
    digest = hashlib.sha256()
    _source_observation_field(
        digest,
        b"domain",
        b"iroha.taira.inrou-canary-input-content.v1",
    )
    _source_observation_field(
        digest,
        b"entry-count",
        len(inputs).to_bytes(8, "big"),
    )
    for evidence in inputs:
        _source_observation_field(
            digest,
            b"path",
            evidence.relative_path.encode("ascii"),
        )
        _source_observation_field(
            digest,
            b"bytes",
            evidence.identity[5].to_bytes(8, "big"),
        )
        _source_observation_field(
            digest,
            b"sha256",
            bytes.fromhex(evidence.sha256),
        )
    return digest.hexdigest()


def require_inrou_canary_workspace(path: Path) -> InrouCanaryWorkspaceEvidence:
    """Pin the fixed owner-only input surface consumed by the native stager."""

    workspace = _require_custodied_directory_chain(
        path,
        label="Inrou canary workspace",
    )
    inrou = workspace / "inrou"
    guest_directory = workspace / INROU_CANARY_GUEST_DIRECTORY
    directory_evidence = _canary_workspace_directory_evidence(workspace)
    _require_exact_directory_entries(
        workspace,
        {
            INROU_CANARY_CONTAINER_FILE,
            INROU_CANARY_SERVICE_FILE,
            INROU_CANARY_BUNDLE_FILE,
            "inrou",
        },
        label="Inrou canary workspace",
    )
    _require_exact_directory_entries(
        inrou,
        {"aarch64"},
        label="Inrou canary guest root",
    )
    _require_exact_directory_entries(
        guest_directory,
        set(INROU_CANARY_GUEST_FILES),
        label="Inrou canary AArch64 guest directory",
    )
    specifications = (
        (INROU_CANARY_CONTAINER_FILE, MAX_INROU_CANARY_MANIFEST_BYTES),
        (INROU_CANARY_SERVICE_FILE, MAX_INROU_CANARY_MANIFEST_BYTES),
        (INROU_CANARY_BUNDLE_FILE, MAX_INROU_CANARY_BUNDLE_BYTES),
        *(
            (
                (INROU_CANARY_GUEST_DIRECTORY / name).as_posix(),
                MAX_INROU_CANARY_GUEST_BYTES,
            )
            for name in INROU_CANARY_GUEST_FILES
        ),
    )
    inspected: list[tuple[str, Path, os.stat_result, int]] = []
    guest_bytes = 0
    for relative, limit in specifications:
        fixture = workspace / relative
        metadata = _require_owner_only_entry(
            fixture,
            directory=False,
            label="Inrou canary input",
        )
        if metadata.st_size == 0 or metadata.st_size > limit:
            fail(f"Inrou canary input must contain 1..={limit} bytes: {fixture}")
        if relative.startswith(f"{INROU_CANARY_GUEST_DIRECTORY.as_posix()}/"):
            guest_bytes += metadata.st_size
            if guest_bytes > MAX_INROU_CANARY_GUEST_BYTES:
                fail(
                    "Inrou canary guest inputs exceed the exact aggregate "
                    f"{MAX_INROU_CANARY_GUEST_BYTES}-byte bound"
                )
        inspected.append((relative, fixture, metadata, limit))
    inputs = tuple(
        InrouCanaryInputEvidence(
            relative_path=relative,
            path=fixture,
            identity=_canary_file_identity(metadata),
            sha256=_stable_canary_input_digest(
                fixture,
                metadata,
                limit=limit,
                label="Inrou canary input",
            ),
        )
        for relative, fixture, metadata, limit in inspected
    )
    if (
        _require_custodied_directory_chain(
            workspace,
            label="Inrou canary workspace",
        )
        != workspace
        or _canary_workspace_directory_evidence(workspace) != directory_evidence
    ):
        fail(f"Inrou canary workspace changed while it was inspected: {workspace}")
    _require_exact_directory_entries(
        workspace,
        {
            INROU_CANARY_CONTAINER_FILE,
            INROU_CANARY_SERVICE_FILE,
            INROU_CANARY_BUNDLE_FILE,
            "inrou",
        },
        label="Inrou canary workspace",
    )
    _require_exact_directory_entries(inrou, {"aarch64"}, label="Inrou canary guest root")
    _require_exact_directory_entries(
        guest_directory,
        set(INROU_CANARY_GUEST_FILES),
        label="Inrou canary AArch64 guest directory",
    )
    return InrouCanaryWorkspaceEvidence(
        root=workspace,
        directory_identities=directory_evidence,
        inputs=inputs,
        content_sha256=_canary_content_digest(inputs),
    )


def _paths_overlap(left: Path, right: Path) -> bool:
    return left == right or left in right.parents or right in left.parents


def required_inrou_canary_workspace(
    args: argparse.Namespace,
) -> InrouCanaryWorkspaceEvidence:
    """Validate the mandatory canary contract before mutating the devnet root."""

    configured = args.inrou_canary_dir
    if configured is None:
        fail("Taira devnet up requires --inrou-canary-dir")
    workspace = require_inrou_canary_workspace(configured)
    workspace_root = workspace.root
    managed_candidate = args.dir.expanduser().absolute().resolve(strict=False)
    target_candidate = (
        args.target_dir.expanduser().absolute().resolve(strict=False)
        if args.target_dir is not None
        else managed_candidate / "cargo-target"
    )
    disallowed = (
        ("repository", REPO_ROOT.resolve(strict=True)),
        ("disposable devnet directory", managed_candidate),
        ("qualification target directory", target_candidate),
    )
    for label, candidate in disallowed:
        if _paths_overlap(workspace_root, candidate):
            fail(f"--inrou-canary-dir and the {label} must be disjoint")
    return workspace


def _section_assignment_from_text(
    path: Path,
    text: str,
    section: str,
    key: str,
) -> str:
    """Parse one scalar assignment from one exact generated TOML section."""

    header = re.compile(r"^\s*\[([^]]+)]\s*$")
    quoted = re.compile(rf'^\s*{re.escape(key)}\s*=\s*"([^"\\]*)"\s*$')
    bare = re.compile(rf"^\s*{re.escape(key)}\s*=\s*([^#\s]+)\s*$")
    current: str | None = None
    values: list[str] = []
    for line in text.splitlines():
        if match := header.fullmatch(line):
            current = match.group(1)
            continue
        if current != section:
            continue
        if match := quoted.fullmatch(line):
            values.append(match.group(1))
        elif match := bare.fullmatch(line):
            values.append(match.group(1))
    if len(values) != 1:
        fail(f"generated config must contain one {section}.{key} assignment: {path}")
    return values[0]


def section_assignment(path: Path, section: str, key: str) -> str:
    """Read one unescaped scalar assignment from one exact generated TOML section."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    return _section_assignment_from_text(path, text, section, key)


def require_canonical_inrou_placement_targets(
    value: object,
    context: str,
) -> tuple[dict[str, str], ...]:
    """Require four exact validator/peer placement identities."""

    if not isinstance(value, list) or len(value) != PEER_COUNT:
        fail(f"{context} must contain exactly four placement targets")
    targets: list[dict[str, str]] = []
    validator_controllers: set[bytes] = set()
    peer_ids: set[str] = set()
    for index, target in enumerate(value):
        if (
            not isinstance(target, dict)
            or set(target) != {"validator_account_id", "peer_id"}
        ):
            fail(f"{context}[{index}] violates the exact V1 target schema")
        validator_account_id = target.get("validator_account_id")
        peer_id = target.get("peer_id")
        try:
            controller = _decode_canonical_i105_account_id(
                validator_account_id,
                expected_discriminant=DEFAULT_CHAIN_DISCRIMINANT,
            )
        except (TypeError, ValueError):
            fail(f"{context}[{index}] has a malformed canonical validator account")
        if (
            not isinstance(peer_id, str)
            or not peer_id
            or peer_id.strip() != peer_id
            or any(character.isspace() for character in peer_id)
            or "," in peer_id
        ):
            fail(f"{context}[{index}] has a malformed peer ID")
        if controller in validator_controllers:
            fail(f"{context} repeats a validator account")
        if peer_id in peer_ids:
            fail(f"{context} repeats a peer ID")
        validator_controllers.add(controller)
        peer_ids.add(peer_id)
        targets.append(
            {
                "validator_account_id": validator_account_id,
                "peer_id": peer_id,
            }
        )
    return tuple(targets)


def _taira_inrou_placement_targets_from_profiles(
    profiles: Sequence[tuple[Path, str]],
) -> tuple[dict[str, str], ...]:
    """Derive four placement identities from one stable config snapshot."""

    if len(profiles) != PEER_COUNT:
        fail("generated Taira placement snapshot must contain exactly four peers")
    values = []
    for peer_index, (config, text) in enumerate(profiles):
        if config.name != f"peer{peer_index}.toml":
            fail("generated Taira placement snapshot is not in canonical peer order")
        values.append(
            {
                "validator_account_id": _section_assignment_from_text(
                    config,
                    text,
                    "soracloud_runtime.submission.signer",
                    "authority",
                ),
                "peer_id": _top_level_quoted_assignment_from_text(
                    config,
                    text,
                    "public_key",
                ),
            }
        )
    return require_canonical_inrou_placement_targets(
        values,
        "generated Taira Inrou placement inventory",
    )


def taira_inrou_placement_targets(target: Path) -> tuple[dict[str, str], ...]:
    """Derive the four staged placement identities from generated peer configs."""

    profiles = tuple(
        (
            config,
            read_bounded_text(
                config,
                limit=MAX_BUNDLE_TEXT_BYTES,
                label=f"peer{peer_index} config",
            ),
        )
        for peer_index, config in enumerate(_taira_peer_configs(target))
    )
    return _taira_inrou_placement_targets_from_profiles(profiles)


@dataclass(frozen=True)
class TrustedLocalnetFaucetPolicy:
    """Exact faucet policy independently configured on every validator."""

    authority: str
    asset_definition_id: str
    amount: str


def require_trusted_localnet_faucet_policy(
    target: Path,
) -> TrustedLocalnetFaucetPolicy:
    """Require one enabled, identical faucet policy across all four peers."""

    policies: list[TrustedLocalnetFaucetPolicy] = []
    for peer_index in range(PEER_COUNT):
        config = target / f"peer{peer_index}.toml"
        if section_assignment(config, "torii.faucet", "enabled") != "true":
            fail(f"peer{peer_index} does not enable its generated faucet policy: {config}")
        policy = TrustedLocalnetFaucetPolicy(
            authority=section_assignment(config, "torii.faucet", "authority"),
            asset_definition_id=section_assignment(
                config, "torii.faucet", "asset_definition_id"
            ),
            amount=section_assignment(config, "torii.faucet", "amount"),
        )
        if any(
            not value
            or value != value.strip()
            or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
            for value in (
                policy.authority,
                policy.asset_definition_id,
                policy.amount,
            )
        ):
            fail(f"peer{peer_index} has a malformed generated faucet policy: {config}")
        policies.append(policy)
    if len(set(policies)) != 1:
        fail("generated Taira validators do not share one exact faucet policy")
    return policies[0]


_CANONICAL_TOML_HEADER = re.compile(
    r"^\s*(\[\[|\[)([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)(\]\]|\])\s*$"
)
_CANONICAL_TOML_ASSIGNMENT = re.compile(
    r"^\s*([A-Za-z][A-Za-z0-9_-]*)\s*=\s*(\S(?:.*\S)?)\s*$"
)
_NEXUS_STORAGE_SECTION = "nexus.storage"
_NEXUS_STORAGE_WEIGHTS_SECTION = "nexus.storage.disk_budget_weights"
_SORAFS_STORAGE_SECTION = "sorafs.storage"
_SORACLOUD_RUNTIME_SECTION = "soracloud_runtime"
_SORACLOUD_RUNTIME_EGRESS_SECTION = "soracloud_runtime.egress"
_SORACLOUD_RUNTIME_INROU_SECTION = "soracloud_runtime.inrou"


def _generated_config_sections(
    path: Path, text: str
) -> tuple[list[str], list[tuple[str, bool, int, int]]]:
    """Split canonical Kagami TOML into bounded table sections."""

    lines = text.splitlines(keepends=True)
    headers: list[tuple[str, bool, int]] = []
    for index, line in enumerate(lines):
        if not line.lstrip().startswith("["):
            continue
        match = _CANONICAL_TOML_HEADER.fullmatch(line.rstrip("\r\n"))
        if match is None or (match.group(1) == "[[") != (match.group(3) == "]]"):
            fail(f"generated config contains an unexpected TOML section header: {path}")
        headers.append((match.group(2), match.group(1) == "[[", index))
    sections = [
        (
            name,
            is_array,
            start,
            headers[offset + 1][2] if offset + 1 < len(headers) else len(lines),
        )
        for offset, (name, is_array, start) in enumerate(headers)
    ]
    return lines, sections


def _storage_section_assignments(
    path: Path,
    lines: Sequence[str],
    section: tuple[str, bool, int, int],
) -> dict[str, str]:
    """Read the exact scalar assignments from one generated storage table."""

    name, _, start, end = section
    assignments: dict[str, str] = {}
    for line in lines[start + 1 : end]:
        if not line.strip():
            continue
        match = _CANONICAL_TOML_ASSIGNMENT.fullmatch(line.rstrip("\r\n"))
        if match is None:
            fail(f"generated {name} contains an unexpected entry: {path}")
        key, value = match.groups()
        if key in assignments:
            fail(f"generated {name} contains duplicate `{key}` assignments: {path}")
        assignments[key] = value
    return assignments


def _one_storage_section(
    path: Path,
    sections: Sequence[tuple[str, bool, int, int]],
    name: str,
) -> tuple[str, bool, int, int]:
    """Require one non-array generated storage table with the exact name."""

    matches = [section for section in sections if section[0] == name]
    if len(matches) != 1 or matches[0][1]:
        fail(f"generated config must contain one [{name}] table: {path}")
    return matches[0]


def _require_exact_keys(
    path: Path,
    section: str,
    assignments: dict[str, str],
    expected: set[str],
) -> None:
    """Reject missing or additional assignments in a generated storage table."""

    actual = set(assignments)
    if actual != expected:
        missing = ", ".join(sorted(expected - actual)) or "none"
        unexpected = ", ".join(sorted(actual - expected)) or "none"
        fail(
            f"generated [{section}] has the wrong assignment set "
            f"(missing: {missing}; unexpected: {unexpected}): {path}"
        )


def _canonical_nonnegative_integer(path: Path, field: str, value: str) -> int:
    """Decode one canonical decimal integer from a generated Taira config."""

    if re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        fail(f"generated {field} must be one canonical non-negative integer: {path}")
    return int(value)


def _expected_peer_sorafs_dir(target: Path, peer_index: int) -> Path:
    """Return one peer's disjoint canonical SoraFS store root."""

    return (target / "state" / f"peer{peer_index}" / "sorafs").resolve(
        strict=False
    )


def _taira_storage_sections(
    path: Path,
    text: str,
) -> tuple[
    list[str],
    list[tuple[str, bool, int, int]],
    dict[str, dict[str, str]],
]:
    """Require only the exact canonical Taira storage table topology."""

    lines, sections = _generated_config_sections(path, text)
    allowed = {
        _NEXUS_STORAGE_SECTION,
        _NEXUS_STORAGE_WEIGHTS_SECTION,
        _SORAFS_STORAGE_SECTION,
    }
    related = [
        section
        for section in sections
        if section[0] == _NEXUS_STORAGE_SECTION
        or section[0].startswith(f"{_NEXUS_STORAGE_SECTION}.")
        or section[0] == _SORAFS_STORAGE_SECTION
        or section[0].startswith(f"{_SORAFS_STORAGE_SECTION}.")
    ]
    unexpected = sorted({section[0] for section in related if section[0] not in allowed})
    if unexpected:
        fail(
            "generated config contains unexpected storage sections "
            f"{', '.join(f'[{name}]' for name in unexpected)}: {path}"
        )
    selected = {
        name: _one_storage_section(path, sections, name)
        for name in sorted(allowed)
    }
    assignments = {
        name: _storage_section_assignments(path, lines, section)
        for name, section in selected.items()
    }
    return lines, sections, assignments


def taira_inrou_identity(peer_index: int) -> tuple[str, int, int]:
    """Return one canonical same-host Taira Inrou identity slot."""

    if not 0 <= peer_index < TAIRA_INROU_IDENTITY_SLOTS:
        fail(f"Taira Inrou identity slot is outside 0..{TAIRA_INROU_IDENTITY_SLOTS}")
    identifier = TAIRA_INROU_IDENTITY_BASE + peer_index
    return f"{TAIRA_INROU_IDENTITY_NAME_PREFIX}{peer_index}", identifier, identifier


def fsync_directory(path: Path, *, label: str) -> None:
    """Durably publish directory-entry changes or fail closed."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open {label} {path}: {error}")
    try:
        os.fsync(descriptor)
    except OSError as error:
        fail(f"cannot synchronize {label} {path}: {error}")
    finally:
        try:
            os.close(descriptor)
        except OSError as error:
            fail(f"cannot close {label} {path}: {error}")


def _require_canonical_taira_runtime_profile(
    config: Path,
    lines: Sequence[str],
    runtime: tuple[str, bool, int, int],
) -> None:
    """Require the exact first-release Taira Soracloud resource profile."""

    assignments = _storage_section_assignments(config, lines, runtime)
    _require_exact_keys(
        config,
        _SORACLOUD_RUNTIME_SECTION,
        assignments,
        {
            "state_dir",
            "production_mode",
            "hydration_concurrency",
            "prepared_runtime_cache_capacity",
        },
    )
    expected = {
        "production_mode": "true",
        "hydration_concurrency": str(TAIRA_SORACLOUD_HYDRATION_CONCURRENCY),
        "prepared_runtime_cache_capacity": str(
            TAIRA_SORACLOUD_PREPARED_RUNTIME_CACHE_CAPACITY
        ),
    }
    if any(assignments[field] != value for field, value in expected.items()):
        fail(
            "generated [soracloud_runtime] is not the exact first-release "
            f"Taira resource profile: {config}"
        )


def _taira_peer_configs(target: Path) -> tuple[Path, ...]:
    """Return the exact four generated peer config paths."""

    expected_files = {f"peer{index}.toml" for index in range(PEER_COUNT)}
    actual_files = {path.name for path in target.glob("peer*.toml")}
    if actual_files != expected_files:
        fail("generated Taira network must contain exactly peer0.toml through peer3.toml")
    return tuple(target / f"peer{index}.toml" for index in range(PEER_COUNT))


def _require_canonical_taira_storage_profile(
    target: Path,
    peer_index: int,
    config: Path,
    text: str,
) -> tuple[list[str], list[tuple[str, bool, int, int]]]:
    """Validate one exact Taira V1 storage profile and its cap math."""

    lines, sections, assignments = _taira_storage_sections(config, text)
    nexus = assignments[_NEXUS_STORAGE_SECTION]
    weights = assignments[_NEXUS_STORAGE_WEIGHTS_SECTION]
    sorafs = assignments[_SORAFS_STORAGE_SECTION]
    _require_exact_keys(
        config,
        _NEXUS_STORAGE_SECTION,
        nexus,
        {"local_budget_bytes"},
    )
    expected_weight_fields = {key for key, _ in TAIRA_NEXUS_STORAGE_WEIGHTS}
    _require_exact_keys(
        config,
        _NEXUS_STORAGE_WEIGHTS_SECTION,
        weights,
        expected_weight_fields,
    )
    _require_exact_keys(
        config,
        _SORAFS_STORAGE_SECTION,
        sorafs,
        {"data_dir", "enabled", "max_capacity_bytes"},
    )
    aggregate = _canonical_nonnegative_integer(
        config,
        "nexus.storage.local_budget_bytes",
        nexus["local_budget_bytes"],
    )
    parsed_weights = {
        key: _canonical_nonnegative_integer(
            config,
            f"nexus.storage.disk_budget_weights.{key}",
            weights[key],
        )
        for key in expected_weight_fields
    }
    capacity = _canonical_nonnegative_integer(
        config,
        "sorafs.storage.max_capacity_bytes",
        sorafs["max_capacity_bytes"],
    )
    expected_dir = _expected_peer_sorafs_dir(target, peer_index)
    if aggregate != TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES:
        fail(f"peer{peer_index} has the wrong Taira storage aggregate: {config}")
    parsed_weight_tuple = tuple(
        (key, parsed_weights[key]) for key, _ in TAIRA_NEXUS_STORAGE_WEIGHTS
    )
    if parsed_weight_tuple != TAIRA_NEXUS_STORAGE_WEIGHTS:
        fail(f"peer{peer_index} has the wrong Taira storage weights: {config}")
    if sum(parsed_weights.values()) != STORAGE_WEIGHT_BASIS_POINTS:
        fail(f"peer{peer_index} Taira storage weights do not sum to 10000 bps: {config}")
    computed_capacity = (
        aggregate * parsed_weights["sorafs_bps"] // STORAGE_WEIGHT_BASIS_POINTS
    )
    if computed_capacity != TAIRA_SORAFS_MAX_CAPACITY_BYTES or capacity != computed_capacity:
        fail(f"peer{peer_index} has the wrong computed SoraFS capacity: {config}")
    if sorafs["enabled"] != "false" or sorafs["data_dir"] != f'"{expected_dir}"':
        fail(f"peer{peer_index} does not use its disabled disjoint SoraFS root: {config}")
    return lines, sections


def _validated_trusted_inrou_guest_artifact(
    artifact: TrustedInrouGuestArtifact,
) -> TrustedInrouGuestArtifact:
    """Require one canonical V1 guest manifest/content identity."""

    if not isinstance(artifact, TrustedInrouGuestArtifact):
        fail("Taira Inrou trusted guest identity is missing")
    if (
        not isinstance(artifact.manifest_digest_hex, str)
        or LOWER_32_BYTE_HEX_RE.fullmatch(artifact.manifest_digest_hex) is None
    ):
        fail("Taira Inrou trusted guest manifest digest is malformed")
    if (
        not isinstance(artifact.content_cid, str)
        or SORAFS_CONTENT_CID_V1_RE.fullmatch(artifact.content_cid) is None
    ):
        fail("Taira Inrou trusted guest content CID is malformed")
    return artifact


def _require_canonical_taira_egress(
    config: Path,
    lines: list[str],
    sections: list[tuple[str, bool, int, int]],
) -> None:
    """Validate the one closed Taira runtime egress policy."""

    _one_storage_section(config, sections, _SORACLOUD_RUNTIME_SECTION)
    egress = _one_storage_section(config, sections, _SORACLOUD_RUNTIME_EGRESS_SECTION)
    assignments = _storage_section_assignments(config, lines, egress)
    _require_exact_keys(
        config,
        _SORACLOUD_RUNTIME_EGRESS_SECTION,
        assignments,
        {"default_allow", "allowed_hosts", "rate_per_minute", "max_bytes_per_minute"},
    )
    expected = {
        "default_allow": "false",
        "allowed_hosts": "[]",
        "rate_per_minute": str(TAIRA_INROU_EGRESS_RATE_PER_MINUTE),
        "max_bytes_per_minute": str(TAIRA_INROU_EGRESS_MAX_BYTES_PER_MINUTE),
    }
    if assignments != expected:
        fail(f"generated Taira config has the wrong egress budgets: {config}")


def _validated_generated_taira_profiles(target: Path) -> tuple[tuple[Path, str], ...]:
    """Read and validate Kagami's complete Taira base profiles once."""

    validated: list[tuple[Path, str]] = []
    for peer_index, config in enumerate(_taira_peer_configs(target)):
        text = read_bounded_text(
            config,
            limit=MAX_BUNDLE_TEXT_BYTES,
            label=f"peer{peer_index} config",
        )
        lines, sections = _require_canonical_taira_storage_profile(
            target, peer_index, config, text
        )
        runtime = _one_storage_section(
            config, sections, _SORACLOUD_RUNTIME_SECTION
        )
        _require_canonical_taira_runtime_profile(config, lines, runtime)
        _require_canonical_taira_egress(config, lines, sections)
        retained = [
            section[0]
            for section in sections
            if section[0] == _SORACLOUD_RUNTIME_INROU_SECTION
            or section[0].startswith(f"{_SORACLOUD_RUNTIME_INROU_SECTION}.")
        ]
        if retained:
            fail(
                "generated Taira base config unexpectedly contains Inrou tables "
                f"{', '.join(f'[{name}]' for name in retained)}: {config}"
            )
        validated.append((config, text))
    return tuple(validated)


def require_generated_taira_profiles(target: Path) -> None:
    """Validate Kagami's complete Taira base profile before guest staging."""

    _validated_generated_taira_profiles(target)


def require_canonical_taira_profiles(
    target: Path,
    trusted_guest: TrustedInrouGuestArtifact,
) -> None:
    """Validate exact four-peer profiles bound to one trusted guest artifact."""

    trusted_guest = _validated_trusted_inrou_guest_artifact(trusted_guest)
    for peer_index, config in enumerate(_taira_peer_configs(target)):
        text = read_bounded_text(
            config,
            limit=MAX_BUNDLE_TEXT_BYTES,
            label=f"peer{peer_index} config",
        )
        lines, sections = _require_canonical_taira_storage_profile(
            target, peer_index, config, text
        )
        runtime = _one_storage_section(
            config, sections, _SORACLOUD_RUNTIME_SECTION
        )
        _require_canonical_taira_runtime_profile(config, lines, runtime)
        _require_canonical_taira_egress(config, lines, sections)
        inrou = _one_storage_section(config, sections, _SORACLOUD_RUNTIME_INROU_SECTION)
        related = [
            section[0]
            for section in sections
            if section[0].startswith(f"{_SORACLOUD_RUNTIME_INROU_SECTION}.")
        ]
        if related:
            fail(
                "generated config contains non-V1 Inrou selector tables "
                f"{', '.join(f'[{name}]' for name in related)}: {config}"
            )
        inrou_assignments = _storage_section_assignments(config, lines, inrou)
        expected_inrou_keys = {
            "enabled",
            "portable_vm_uid",
            "portable_vm_gid",
            "trusted_guest_manifest_digest_hex",
            "trusted_guest_content_cid",
            "guest_image_max_bytes",
            "max_cpu_millis",
            "max_memory_bytes",
            "max_storage_bytes",
            "start_grace_ms",
            "stop_grace_ms",
        }
        _require_exact_keys(
            config,
            _SORACLOUD_RUNTIME_INROU_SECTION,
            inrou_assignments,
            expected_inrou_keys,
        )
        _, expected_uid, expected_gid = taira_inrou_identity(peer_index)
        expected_inrou = {
            "enabled": "true",
            "portable_vm_uid": str(expected_uid),
            "portable_vm_gid": str(expected_gid),
            "trusted_guest_manifest_digest_hex": f'"{trusted_guest.manifest_digest_hex}"',
            "trusted_guest_content_cid": f'"{trusted_guest.content_cid}"',
            "guest_image_max_bytes": str(TAIRA_INROU_GUEST_IMAGE_MAX_BYTES),
            "max_cpu_millis": str(TAIRA_INROU_MAX_CPU_MILLIS),
            "max_memory_bytes": str(TAIRA_INROU_MAX_MEMORY_BYTES),
            "max_storage_bytes": str(TAIRA_INROU_MAX_STORAGE_BYTES),
            "start_grace_ms": str(TAIRA_INROU_START_GRACE_MS),
            "stop_grace_ms": str(TAIRA_INROU_STOP_GRACE_MS),
        }
        if inrou_assignments != expected_inrou:
            fail(f"peer{peer_index} has the wrong PortableVM V1 profile: {config}")


def _require_exact_directory_entries(
    directory: Path,
    expected: set[str],
    *,
    label: str,
) -> None:
    """Reject missing or additional entries in one fixed stage directory."""

    try:
        actual = {entry.name for entry in directory.iterdir()}
    except OSError as error:
        fail(f"cannot inspect {label} {directory}: {error}")
    if actual != expected:
        missing = ", ".join(sorted(expected - actual)) or "none"
        unexpected = ", ".join(sorted(actual - expected)) or "none"
        fail(
            f"{label} has the wrong fixed layout "
            f"(missing: {missing}; unexpected: {unexpected}): {directory}"
        )


def require_inrou_stage(stage_dir: Path) -> None:
    """Require the exact owner-only stage layout emitted by the current CLI."""

    _require_owner_only_entry(
        stage_dir,
        directory=True,
        label="native Taira Inrou stage",
    )
    manifests = stage_dir / "manifests"
    payloads = stage_dir / "payloads"
    guest = stage_dir / INROU_STAGE_GUEST_PAYLOAD
    discovery = stage_dir / INROU_STAGE_DISCOVERY_PAYLOAD
    for directory in (manifests, payloads, guest, discovery):
        _require_owner_only_entry(
            directory,
            directory=True,
            label="native Taira Inrou stage directory",
        )
    _require_exact_directory_entries(
        stage_dir,
        {
            INROU_STAGE_RECEIPT_FILE,
            INROU_STAGE_CONTAINER_FILE,
            INROU_STAGE_SERVICE_FILE,
            "manifests",
            "payloads",
        },
        label="native Taira Inrou stage",
    )
    _require_exact_directory_entries(
        manifests,
        {
            INROU_STAGE_BUNDLE_MANIFEST.name,
            INROU_STAGE_GUEST_MANIFEST.name,
            INROU_STAGE_DISCOVERY_MANIFEST.name,
        },
        label="native Taira Inrou manifest stage",
    )
    _require_exact_directory_entries(
        payloads,
        {
            INROU_STAGE_BUNDLE_PAYLOAD.name,
            INROU_STAGE_GUEST_PAYLOAD.name,
            INROU_STAGE_DISCOVERY_PAYLOAD.name,
        },
        label="native Taira Inrou payload stage",
    )
    _require_exact_directory_entries(
        discovery,
        {INROU_STAGE_DISCOVERY_DOCUMENT.name},
        label="native Taira Inrou discovery payload stage",
    )
    required_files = (
        stage_dir / INROU_STAGE_RECEIPT_FILE,
        stage_dir / INROU_STAGE_CONTAINER_FILE,
        stage_dir / INROU_STAGE_SERVICE_FILE,
        stage_dir / INROU_STAGE_BUNDLE_PAYLOAD,
        stage_dir / INROU_STAGE_BUNDLE_MANIFEST,
        stage_dir / INROU_STAGE_GUEST_MANIFEST,
        stage_dir / INROU_STAGE_DISCOVERY_DOCUMENT,
        stage_dir / INROU_STAGE_DISCOVERY_MANIFEST,
    )
    for path in required_files:
        metadata = _require_owner_only_entry(
            path,
            directory=False,
            label="native Taira Inrou staged file",
        )
        if metadata.st_size == 0:
            fail(f"native Taira Inrou staged file is empty: {path}")

    guest_files = 0
    pending = [guest]
    while pending:
        directory = pending.pop()
        try:
            entries = list(directory.iterdir())
        except OSError as error:
            fail(f"cannot inspect native Taira Inrou guest stage {directory}: {error}")
        for entry in entries:
            try:
                metadata = entry.lstat()
            except OSError as error:
                fail(f"cannot inspect native Taira Inrou guest entry {entry}: {error}")
            if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                _require_owner_only_entry(
                    entry,
                    directory=True,
                    label="native Taira Inrou guest directory",
                )
                pending.append(entry)
            elif stat.S_ISREG(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                metadata = _require_owner_only_entry(
                    entry,
                    directory=False,
                    label="native Taira Inrou guest file",
                )
                if metadata.st_size == 0:
                    fail(f"native Taira Inrou guest file is empty: {entry}")
                guest_files += 1
            else:
                fail(f"native Taira Inrou guest stage contains a non-file entry: {entry}")
    if guest_files == 0:
        fail(f"native Taira Inrou guest payload stage is empty: {guest}")


def _read_inrou_stage_receipt(stage_dir: Path) -> dict[str, Any]:
    """Read the staged receipt through one stable owner-only descriptor."""

    path = stage_dir / INROU_STAGE_RECEIPT_FILE
    payload = read_stable_bytes(
        path,
        limit=MAX_INROU_STAGE_RECEIPT_BYTES,
        label="native Taira Inrou stage receipt",
        owner=os.geteuid(),
        exact_mode=0o600,
        require_nonempty=True,
    )
    try:
        receipt = json_loads_no_duplicates(payload.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        fail("native Taira Inrou stage receipt is not JSON")
    if not isinstance(receipt, dict) or set(receipt) != INROU_STAGE_RECEIPT_KEYS_V1:
        fail("native Taira Inrou stage receipt violates the exact V1 schema")
    expected = {
        "schema_version": 1,
        "mutation_mode": "deploy",
        "service_name": "taira_inrou_canary",
        "container_file": str(INROU_STAGE_CONTAINER_FILE),
        "service_file": str(INROU_STAGE_SERVICE_FILE),
        "bundle_payload_file": str(INROU_STAGE_BUNDLE_PAYLOAD),
        "bundle_manifest_file": str(INROU_STAGE_BUNDLE_MANIFEST),
        "guest_isa": "aarch64",
        "guest_payload_dir": str(INROU_STAGE_GUEST_PAYLOAD),
        "guest_manifest_file": str(INROU_STAGE_GUEST_MANIFEST),
        "discovery_payload_dir": str(INROU_STAGE_DISCOVERY_PAYLOAD),
        "discovery_manifest_file": str(INROU_STAGE_DISCOVERY_MANIFEST),
    }
    if any(receipt.get(field) != value for field, value in expected.items()):
        fail("native Taira Inrou stage receipt is not the exact V1 deploy layout")
    retention_epoch = receipt.get("sorafs_retention_epoch")
    if type(retention_epoch) is not int or not 1 <= retention_epoch < 1 << 64:
        fail("native Taira Inrou stage receipt has a malformed SoraFS retention epoch")
    require_canonical_inrou_placement_targets(
        receipt.get("placement_targets"),
        "native Taira Inrou stage receipt placement_targets",
    )
    service_version = receipt.get("service_version")
    if not is_canonical_inrou_service_version(service_version):
        fail("native Taira Inrou stage receipt has a malformed artifact-derived service_version")
    for field in (
        "bundle_hash",
        "discovery_document_hash",
        "container_manifest_hash",
        "service_manifest_hash",
    ):
        value = receipt.get(field)
        if not is_canonical_iroha_hash_hex(value):
            fail(f"native Taira Inrou stage receipt has malformed {field}")
    for field in (
        "bundle_manifest_digest_hex",
        "guest_manifest_digest_hex",
        "discovery_manifest_digest_hex",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or LOWER_32_BYTE_HEX_RE.fullmatch(value) is None:
            fail(f"native Taira Inrou stage receipt has malformed {field}")
    for field in (
        "bundle_content_cid",
        "guest_content_cid",
        "discovery_content_cid",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or SORAFS_CONTENT_CID_V1_RE.fullmatch(value) is None:
            fail(f"native Taira Inrou stage receipt has malformed {field}")
    discovery_cid = receipt["discovery_content_cid"]
    if receipt.get("public_discovery_url") != (
        f"https://taira.sora.org/sorafs/cid/{discovery_cid}/index.json"
    ) or receipt.get("public_discovery_cid_host_url") != (
        f"https://{discovery_cid}.sorafs.taira.sora.org/index.json"
    ):
        fail("native Taira Inrou stage receipt has noncanonical discovery URLs")
    return receipt


def require_inrou_stage_guest_artifact(stage_dir: Path) -> TrustedInrouGuestArtifact:
    """Return the exact guest identity from one fully custodied V1 stage."""

    require_inrou_stage(stage_dir)
    receipt = _read_inrou_stage_receipt(stage_dir)
    return _validated_trusted_inrou_guest_artifact(
        TrustedInrouGuestArtifact(
            manifest_digest_hex=receipt["guest_manifest_digest_hex"],
            content_cid=receipt["guest_content_cid"],
        )
    )


def _require_inrou_stage_placement_targets_match(
    expected_targets: Sequence[dict[str, str]],
    stage_dir: Path,
) -> None:
    """Bind a retained stage to one validated placement snapshot."""

    expected = {
        (item["validator_account_id"], item["peer_id"])
        for item in expected_targets
    }
    receipt = _read_inrou_stage_receipt(stage_dir)
    actual = {
        (item["validator_account_id"], item["peer_id"])
        for item in require_canonical_inrou_placement_targets(
            receipt.get("placement_targets"),
            "native Taira Inrou stage receipt placement_targets",
        )
    }
    if actual != expected:
        fail("native Taira Inrou stage placement targets differ from generated validators")


def require_inrou_stage_placement_targets(target: Path, stage_dir: Path) -> None:
    """Bind a retained stage to the same four generated validator clients."""

    _require_inrou_stage_placement_targets_match(
        taira_inrou_placement_targets(target),
        stage_dir,
    )


def _require_unchanged_inrou_canary_workspace(
    expected: InrouCanaryWorkspaceEvidence,
    *,
    phase: str,
) -> None:
    if require_inrou_canary_workspace(expected.root) != expected:
        fail(f"Inrou canary workspace changed {phase}")


def _copy_pinned_inrou_canary_input(
    evidence: InrouCanaryInputEvidence,
    destination: Path,
) -> None:
    """Copy one observed input through pinned descriptors into a fresh file."""

    try:
        pathname_before = evidence.path.lstat()
    except OSError as error:
        fail(f"cannot inspect pinned Inrou canary input {evidence.path}: {error}")
    if _canary_file_identity(pathname_before) != evidence.identity:
        fail(f"Inrou canary input changed identity before staging: {evidence.path}")
    source_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    destination_flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        source_descriptor = os.open(evidence.path, source_flags)
    except OSError as error:
        fail(f"cannot open pinned Inrou canary input {evidence.path}: {error}")
    destination_descriptor = -1
    digest = hashlib.sha256()
    exact_bytes = 0
    try:
        try:
            source_opened = os.fstat(source_descriptor)
        except OSError as error:
            fail(f"cannot inspect opened Inrou canary input {evidence.path}: {error}")
        if _canary_file_identity(source_opened) != evidence.identity:
            fail(f"Inrou canary input changed while it was opened: {evidence.path}")
        try:
            destination_descriptor = os.open(destination, destination_flags, 0o600)
        except OSError as error:
            fail(f"cannot create pinned Inrou canary snapshot {destination}: {error}")
        while True:
            try:
                chunk = os.read(source_descriptor, 1024 * 1024)
            except OSError as error:
                fail(f"cannot read pinned Inrou canary input {evidence.path}: {error}")
            if not chunk:
                break
            exact_bytes += len(chunk)
            digest.update(chunk)
            pending = memoryview(chunk)
            while pending:
                try:
                    written = os.write(destination_descriptor, pending)
                except OSError as error:
                    fail(f"cannot write pinned Inrou canary snapshot {destination}: {error}")
                if written <= 0:
                    fail(
                        "short write while creating pinned Inrou canary snapshot "
                        f"{destination}"
                    )
                pending = pending[written:]
        try:
            os.fchmod(destination_descriptor, 0o600)
            os.fsync(destination_descriptor)
            source_after = os.fstat(source_descriptor)
        except OSError as error:
            fail(f"cannot seal pinned Inrou canary snapshot {destination}: {error}")
    finally:
        if destination_descriptor >= 0:
            try:
                os.close(destination_descriptor)
            except OSError as error:
                fail(f"cannot close pinned Inrou canary snapshot {destination}: {error}")
        try:
            os.close(source_descriptor)
        except OSError as error:
            fail(f"cannot close pinned Inrou canary input {evidence.path}: {error}")
    try:
        pathname_after = evidence.path.lstat()
    except OSError as error:
        fail(f"cannot re-inspect pinned Inrou canary input {evidence.path}: {error}")
    if (
        _canary_file_identity(source_after) != evidence.identity
        or _canary_file_identity(pathname_after) != evidence.identity
        or exact_bytes != evidence.identity[5]
        or digest.hexdigest() != evidence.sha256
    ):
        fail(f"Inrou canary input changed while it was staged: {evidence.path}")


def snapshot_inrou_canary_workspace(
    target: Path,
    expected: InrouCanaryWorkspaceEvidence,
) -> InrouCanaryWorkspaceEvidence:
    """Copy the exact observed canary tree into root-custodied devnet storage."""

    _require_custodied_directory_chain(target, label="generated Taira network")
    snapshot_root = target / INROU_CANARY_INPUT_SNAPSHOT_DIRECTORY
    if snapshot_root.exists() or snapshot_root.is_symlink():
        fail(f"refusing to reuse an Inrou canary input snapshot: {snapshot_root}")
    try:
        snapshot_root.mkdir(mode=0o700)
        (snapshot_root / "inrou").mkdir(mode=0o700)
        (snapshot_root / INROU_CANARY_GUEST_DIRECTORY).mkdir(mode=0o700)
    except OSError as error:
        fail(f"cannot create Inrou canary input snapshot {snapshot_root}: {error}")
    _require_custodied_directory_chain(
        snapshot_root,
        label="Inrou canary input snapshot",
    )
    for evidence in expected.inputs:
        destination = snapshot_root / evidence.relative_path
        _copy_pinned_inrou_canary_input(evidence, destination)
    snapshot = require_inrou_canary_workspace(snapshot_root)
    if snapshot.content_sha256 != expected.content_sha256:
        fail("Inrou canary input snapshot does not match the observed workspace")
    return snapshot


def prepare_inrou_stage(
    target: Path,
    iroha: Path,
    workspace: InrouCanaryWorkspaceEvidence,
    timeout_seconds: float,
    run: Runner,
) -> Path:
    """Build the fixed deploy stage before any validator opens its SoraFS root."""

    snapshot = snapshot_inrou_canary_workspace(target, workspace)
    inputs = {evidence.relative_path: evidence.path for evidence in snapshot.inputs}
    container = inputs[INROU_CANARY_CONTAINER_FILE]
    service = inputs[INROU_CANARY_SERVICE_FILE]
    bundle = inputs[INROU_CANARY_BUNDLE_FILE]
    stage_dir = target / INROU_STAGE_DIRECTORY
    if stage_dir.exists() or stage_dir.is_symlink():
        fail(f"refusing to reuse an existing Taira Inrou stage: {stage_dir}")
    retention_epoch = time.time_ns() // 1_000_000_000 + max(
        30 * 24 * 60 * 60,
        max(1, int(timeout_seconds)) * 16,
    )
    placement_targets = taira_inrou_placement_targets(target)
    placement_arguments = [
        argument
        for placement in placement_targets
        for argument in (
            "--placement-target",
            f"{placement['validator_account_id']},{placement['peer_id']}",
        )
    ]
    run(
        [
            str(iroha),
            "-c",
            str(target / "client.toml"),
            "taira",
            "inrou-stage",
            "--mode",
            "deploy",
            "--container",
            str(container),
            "--service",
            str(service),
            "--bundle-file",
            str(bundle),
            "--sorafs-retention-epoch",
            str(retention_epoch),
            *placement_arguments,
            "--stage-dir",
            str(stage_dir),
            "--bind-validator-config-dir",
            str(target),
            "--json",
        ],
        cwd=target,
        timeout=timeout_seconds + 300,
    )
    _require_unchanged_inrou_canary_workspace(
        snapshot,
        phase="while the compiled stager consumed its snapshot",
    )
    require_inrou_stage(stage_dir)
    trusted_guest = require_inrou_stage_guest_artifact(stage_dir)
    require_canonical_taira_profiles(target, trusted_guest)
    require_inrou_stage_placement_targets(target, stage_dir)
    return stage_dir


def _drain_inrou_operator_preseed_pipe(
    stream: Any,
    output: bytearray,
    *,
    limit: int,
    label: str,
) -> bool:
    """Drain one nonblocking child pipe and report whether it reached EOF."""

    while True:
        try:
            chunk = os.read(stream.fileno(), 16 * 1024)
        except BlockingIOError:
            return False
        except OSError as error:
            fail(f"failed to read Inrou operator-preseed {label}: {error}")
        if not chunk:
            return True
        output.extend(chunk)
        if len(output) > limit:
            fail(f"Inrou operator-preseed {label} exceeds its exact safety bound")


def _terminate_inrou_operator_preseed_process(process: Any) -> None:
    """Boundedly terminate only the task-owned preseed helper process."""

    if process.poll() is not None:
        return
    try:
        process.terminate()
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=1)
    except OSError:
        return


def run_locked_inrou_operator_preseed_session(
    command: Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    verify_ready_receipt: Callable[[bytes], dict[str, Any]],
) -> dict[str, Any]:
    """Accept one bounded ready barrier while every requested store lock is live."""

    if timeout_seconds <= 0:
        fail("Inrou operator-preseed timeout must be positive")
    deadline = time.monotonic() + timeout_seconds
    try:
        process = subprocess.Popen(
            list(command),
            cwd=cwd,
            env={"LC_ALL": "C"},
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        fail(f"cannot execute Inrou operator-preseed helper: {error}")
    if process.stdin is None or process.stdout is None or process.stderr is None:
        _terminate_inrou_operator_preseed_process(process)
        fail("Inrou operator-preseed helper did not expose its three required pipes")
    stdout = process.stdout
    stderr = process.stderr
    try:
        os.set_blocking(stdout.fileno(), False)
        os.set_blocking(stderr.fileno(), False)
    except OSError as error:
        _terminate_inrou_operator_preseed_process(process)
        process.stdin.close()
        stdout.close()
        stderr.close()
        fail(f"cannot make Inrou operator-preseed pipes nonblocking: {error}")
    stdout_bytes = bytearray()
    stderr_bytes = bytearray()
    stdout_eof = False
    stderr_eof = False
    released = False
    try:
        while True:
            if time.monotonic() >= deadline:
                fail("Inrou operator-preseed helper exceeded its ready-barrier deadline")
            stdout_eof = (
                _drain_inrou_operator_preseed_pipe(
                    stdout,
                    stdout_bytes,
                    limit=MAX_INROU_OPERATOR_PRESEED_RECEIPT_BYTES,
                    label="stdout",
                )
                or stdout_eof
            )
            stderr_eof = (
                _drain_inrou_operator_preseed_pipe(
                    stderr,
                    stderr_bytes,
                    limit=MAX_INROU_OPERATOR_PRESEED_STDERR_BYTES,
                    label="stderr",
                )
                or stderr_eof
            )
            status = process.poll()
            newline = stdout_bytes.find(b"\n")
            if newline >= 0:
                if (
                    newline + 1 != len(stdout_bytes)
                    or b"\r" in stdout_bytes[:newline]
                    or status is not None
                ):
                    fail(
                        "Inrou operator-preseed helper did not retain one live canonical "
                        "ready barrier"
                    )
                break
            if stdout_eof or status is not None:
                detail = bytes(stderr_bytes).decode("utf-8", errors="replace").strip()
                fail(
                    "Inrou operator-preseed helper exited before its ready barrier"
                    + (f": {detail}" if detail else "")
                )
            time.sleep(
                min(
                    INROU_OPERATOR_PRESEED_POLL_SECONDS,
                    max(0.0, deadline - time.monotonic()),
                )
            )

        ready_length = len(stdout_bytes)
        receipt = verify_ready_receipt(bytes(stdout_bytes))
        if time.monotonic() >= deadline:
            fail("Inrou operator-preseed receipt validation exceeded its deadline")
        stdout_eof = (
            _drain_inrou_operator_preseed_pipe(
                stdout,
                stdout_bytes,
                limit=MAX_INROU_OPERATOR_PRESEED_RECEIPT_BYTES,
                label="stdout",
            )
            or stdout_eof
        )
        stderr_eof = (
            _drain_inrou_operator_preseed_pipe(
                stderr,
                stderr_bytes,
                limit=MAX_INROU_OPERATOR_PRESEED_STDERR_BYTES,
                label="stderr",
            )
            or stderr_eof
        )
        if len(stdout_bytes) != ready_length:
            fail("Inrou operator-preseed helper emitted trailing stdout during validation")
        if process.poll() is not None:
            fail("Inrou operator-preseed helper exited while its receipt was validated")

        # The helper owns all four store locks until this exact EOF. Only the
        # fully validated canonical receipt authorizes releasing the barrier;
        # its exact acknowledgement proves the helper observed that EOF.
        process.stdin.close()
        released = True
        while True:
            if time.monotonic() >= deadline:
                fail("Inrou operator-preseed helper exceeded its release deadline")
            stdout_eof = (
                _drain_inrou_operator_preseed_pipe(
                    stdout,
                    stdout_bytes,
                    limit=MAX_INROU_OPERATOR_PRESEED_RECEIPT_BYTES,
                    label="stdout",
                )
                or stdout_eof
            )
            stderr_eof = (
                _drain_inrou_operator_preseed_pipe(
                    stderr,
                    stderr_bytes,
                    limit=MAX_INROU_OPERATOR_PRESEED_STDERR_BYTES,
                    label="stderr",
                )
                or stderr_eof
            )
            status = process.poll()
            if status is not None and stdout_eof and stderr_eof:
                break
            time.sleep(
                min(
                    INROU_OPERATOR_PRESEED_POLL_SECONDS,
                    max(0.0, deadline - time.monotonic()),
                )
            )
        if status != 0:
            detail = bytes(stderr_bytes).decode("utf-8", errors="replace").strip()
            fail(
                f"Inrou operator-preseed helper failed after readiness with status {status}"
                + (f": {detail}" if detail else "")
            )
        expected_stdout = (
            bytes(stdout_bytes[:ready_length])
            + INROU_OPERATOR_PRESEED_RELEASE_ACK_V1
        )
        if bytes(stdout_bytes) != expected_stdout:
            fail(
                "Inrou operator-preseed helper did not emit the exact EOF release "
                "acknowledgement"
            )
        return receipt
    except BaseException:
        _terminate_inrou_operator_preseed_process(process)
        raise
    finally:
        if not released and not process.stdin.closed:
            process.stdin.close()
        stdout.close()
        stderr.close()


def _inrou_stage_payload_content_lengths(stage_dir: Path) -> dict[str, int]:
    """Measure the three already-custodied stage payloads without following links."""

    bundle = _require_owner_only_entry(
        stage_dir / INROU_STAGE_BUNDLE_PAYLOAD,
        directory=False,
        label="native Taira Inrou bundle payload",
    ).st_size
    guest = 0
    pending = [stage_dir / INROU_STAGE_GUEST_PAYLOAD]
    while pending:
        directory = pending.pop()
        try:
            entries = list(directory.iterdir())
        except OSError as error:
            fail(f"cannot inspect native Taira Inrou guest payload {directory}: {error}")
        for entry in entries:
            try:
                metadata = entry.lstat()
            except OSError as error:
                fail(f"cannot inspect native Taira Inrou guest payload {entry}: {error}")
            if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                _require_owner_only_entry(
                    entry,
                    directory=True,
                    label="native Taira Inrou guest payload directory",
                )
                pending.append(entry)
            elif stat.S_ISREG(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                guest += _require_owner_only_entry(
                    entry,
                    directory=False,
                    label="native Taira Inrou guest payload file",
                ).st_size
            else:
                fail(f"native Taira Inrou guest payload has an unsafe entry: {entry}")
    discovery = _require_owner_only_entry(
        stage_dir / INROU_STAGE_DISCOVERY_DOCUMENT,
        directory=False,
        label="native Taira Inrou discovery document",
    ).st_size
    if any(not 0 < value < 1 << 64 for value in (bundle, guest, discovery)):
        fail("native Taira Inrou stage has invalid payload geometry")
    return {"bundle": bundle, "guest": guest, "discovery": discovery}


def require_canonical_inrou_operator_preseed_receipt(
    value: object,
    target: Path,
    stage_dir: Path,
) -> dict[str, Any]:
    """Bind one V1 ready receipt to the exact stage and four config pairs."""

    require_inrou_stage_placement_targets(target, stage_dir)
    trusted_guest = require_inrou_stage_guest_artifact(stage_dir)
    require_canonical_taira_profiles(target, trusted_guest)
    store_roots = [
        _require_custodied_directory_chain(
            _expected_peer_sorafs_dir(target, index),
            label=f"peer{index} Taira Inrou preseed root",
        )
        for index in range(PEER_COUNT)
    ]
    if not isinstance(value, dict) or set(value) != INROU_OPERATOR_PRESEED_RECEIPT_KEYS_V1:
        fail("Inrou operator-preseed receipt violates the exact V1 schema")
    if (
        type(value.get("schema_version")) is not int
        or value["schema_version"] != 1
        or value.get("status") != "ready"
        or value.get("mode") != "ingest"
        or type(value.get("max_capacity_bytes")) is not int
        or value["max_capacity_bytes"] != TAIRA_SORAFS_MAX_CAPACITY_BYTES
    ):
        fail("Inrou operator-preseed receipt is not the exact ingest-ready V1 mode")
    expected_targets = [
        {
            "validator_account_id": placement["validator_account_id"],
            "peer_id": placement["peer_id"],
            "store_root": str(store_root),
        }
        for placement, store_root in zip(
            taira_inrou_placement_targets(target), store_roots, strict=True
        )
    ]
    targets = value.get("targets")
    if (
        not isinstance(targets, list)
        or len(targets) != PEER_COUNT
        or any(
            not isinstance(item, dict)
            or set(item) != INROU_OPERATOR_PRESEED_TARGET_KEYS_V1
            for item in targets
        )
        or targets != expected_targets
    ):
        fail(
            "Inrou operator-preseed receipt differs from the exact configured "
            "account+peer targets"
        )
    stage = _read_inrou_stage_receipt(stage_dir)
    content_lengths = _inrou_stage_payload_content_lengths(stage_dir)
    expected_artifacts = {
        stage["bundle_manifest_digest_hex"]: content_lengths["bundle"],
        stage["guest_manifest_digest_hex"]: content_lengths["guest"],
        stage["discovery_manifest_digest_hex"]: content_lengths["discovery"],
    }
    artifacts = value.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != 3:
        fail("Inrou operator-preseed receipt must contain the three exact stage artifacts")
    manifest_digests = [
        artifact.get("manifest_digest_blake3")
        if isinstance(artifact, dict)
        else None
        for artifact in artifacts
    ]
    if manifest_digests != sorted(expected_artifacts):
        fail("Inrou operator-preseed artifacts are not the exact canonical digest order")
    canonical_artifacts = []
    for index, artifact in enumerate(artifacts):
        if (
            not isinstance(artifact, dict)
            or set(artifact) != INROU_OPERATOR_PRESEED_ARTIFACT_KEYS_V1
        ):
            fail(f"Inrou operator-preseed artifact {index} violates the exact V1 schema")
        manifest_digest = artifact.get("manifest_digest_blake3")
        expected_content_length = expected_artifacts.get(manifest_digest)
        payload_digest = artifact.get("payload_digest_blake3")
        content_length = artifact.get("content_length")
        store_count = artifact.get("store_count")
        if (
            expected_content_length is None
            or not isinstance(payload_digest, str)
            or LOWER_32_BYTE_HEX_RE.fullmatch(payload_digest) is None
            or type(content_length) is not int
            or content_length != expected_content_length
            or type(store_count) is not int
            or store_count != PEER_COUNT
        ):
            fail(f"Inrou operator-preseed artifact {index} differs from the exact stage")
        canonical_artifacts.append(
            {
                "manifest_digest_blake3": manifest_digest,
                "payload_digest_blake3": payload_digest,
                "content_length": content_length,
                "store_count": PEER_COUNT,
            }
        )
    return {
        "schema_version": 1,
        "status": "ready",
        "mode": "ingest",
        "max_capacity_bytes": TAIRA_SORAFS_MAX_CAPACITY_BYTES,
        "targets": expected_targets,
        "artifacts": canonical_artifacts,
    }


def require_canonical_inrou_operator_preseed_receipt_line(
    payload: bytes,
    target: Path,
    stage_dir: Path,
) -> dict[str, Any]:
    """Require the helper's sole newline-terminated canonical compact JSON line."""

    if (
        not payload.endswith(b"\n")
        or not payload[:-1]
        or b"\n" in payload[:-1]
        or b"\r" in payload[:-1]
    ):
        fail("Inrou operator-preseed helper emitted a noncanonical receipt line")
    try:
        decoded = json_loads_no_duplicates(payload[:-1].decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        fail("Inrou operator-preseed helper emitted invalid V1 JSON")
    receipt = require_canonical_inrou_operator_preseed_receipt(
        decoded,
        target,
        stage_dir,
    )
    canonical = (
        json.dumps(receipt, ensure_ascii=False, separators=(",", ":")) + "\n"
    ).encode("utf-8")
    if payload != canonical:
        fail("Inrou operator-preseed helper receipt is not canonical compact JSON")
    return receipt


def preseed_inrou_stage(
    target: Path,
    sorafs_node: Path,
    stage_dir: Path,
    trusted_guest: TrustedInrouGuestArtifact,
    timeout_seconds: float,
) -> dict[str, Any]:
    """Preseed all three stage artifacts under one four-store ready barrier."""

    require_inrou_stage_placement_targets(target, stage_dir)
    if require_inrou_stage_guest_artifact(stage_dir) != trusted_guest:
        fail("Taira Inrou stage guest identity changed before SoraFS preseed")
    require_canonical_taira_profiles(target, trusted_guest)
    data_dirs = [
        _expected_peer_sorafs_dir(target, index)
        for index in range(PEER_COUNT)
    ]
    if len(set(data_dirs)) != PEER_COUNT:
        fail("Taira Inrou preseed requires four disjoint SoraFS roots")
    for data_dir in data_dirs:
        try:
            data_dir.mkdir(parents=True, mode=0o700, exist_ok=True)
        except OSError as error:
            fail(f"cannot create Taira Inrou preseed root {data_dir}: {error}")
        _require_custodied_directory_chain(data_dir, label="Taira Inrou preseed root")
    placements = taira_inrou_placement_targets(target)
    command = [str(sorafs_node), "preseed-session"]
    command.extend(
        f"--target={placement['validator_account_id']},{placement['peer_id']},{data_dir}"
        for placement, data_dir in zip(placements, data_dirs, strict=True)
    )
    command.append(f"--max-capacity-bytes={TAIRA_SORAFS_MAX_CAPACITY_BYTES}")
    for manifest, source_flag, source in (
        (
            stage_dir / INROU_STAGE_BUNDLE_MANIFEST,
            "--payload",
            stage_dir / INROU_STAGE_BUNDLE_PAYLOAD,
        ),
        (
            stage_dir / INROU_STAGE_GUEST_MANIFEST,
            "--payload-dir",
            stage_dir / INROU_STAGE_GUEST_PAYLOAD,
        ),
        (
            stage_dir / INROU_STAGE_DISCOVERY_MANIFEST,
            "--payload-dir",
            stage_dir / INROU_STAGE_DISCOVERY_PAYLOAD,
        ),
    ):
        command.extend((f"--manifest={manifest}", f"{source_flag}={source}"))
    return run_locked_inrou_operator_preseed_session(
        command,
        cwd=target,
        timeout_seconds=timeout_seconds + 300,
        verify_ready_receipt=lambda payload: require_canonical_inrou_operator_preseed_receipt_line(
            payload,
            target,
            stage_dir,
        ),
    )


def require_canonical_inrou_local_placement(
    value: object,
    context: str,
) -> dict[str, Any]:
    """Require one exact node-local placement identity from the Torii root."""

    if not isinstance(value, dict) or set(value) != INROU_LOCAL_PLACEMENT_KEYS_V1:
        fail(f"{context} violates the exact V1 local-placement schema")
    peer_id = value.get("peer_id")
    validator_account_id = value.get("validator_account_id")
    if (
        not isinstance(peer_id, str)
        or not peer_id
        or peer_id.strip() != peer_id
        or any(character.isspace() for character in peer_id)
    ):
        fail(f"{context} has a malformed peer_id")
    try:
        # The canonical decoder also re-encodes the controller bytes. Pinning
        # the discriminant makes this the one exact testnet I105 rendering.
        _decode_canonical_i105_account_id(
            validator_account_id,
            expected_discriminant=DEFAULT_CHAIN_DISCRIMINANT,
        )
    except (TypeError, ValueError):
        fail(f"{context} has a malformed canonical testnet I105 validator_account_id")
    replica_slot = value.get("replica_slot")
    if type(replica_slot) is not int or replica_slot not in range(1, PEER_COUNT + 1):
        fail(f"{context} has a malformed replica_slot")
    if not is_canonical_iroha_hash_hex(value.get("placement_incarnation")):
        fail(f"{context} has a malformed placement_incarnation")
    return value


def peer_index_for_local_placement(target: Path, placement: object) -> int:
    """Map one exact receipt account+peer pair to one generated peer config."""

    canonical = require_canonical_inrou_local_placement(
        placement,
        "compiled Taira Inrou local placement",
    )
    placements = taira_inrou_placement_targets(target)
    matches = [
        index
        for index, expected in enumerate(placements)
        if expected["validator_account_id"] == canonical["validator_account_id"]
        and expected["peer_id"] == canonical["peer_id"]
    ]
    if len(matches) != 1:
        fail(
            "Inrou local placement account+peer pair does not map to exactly one "
            "generated peer config"
        )
    return matches[0]


def require_canonical_inrou_replicas(value: object, context: str) -> list[dict[str, Any]]:
    """Require the four ordered V1 replica identities and durable boot evidence."""

    if not isinstance(value, list) or len(value) != PEER_COUNT:
        fail(f"{context} must contain four replica identities")
    app_data_markers: set[str] = set()
    guest_boot_ids: set[str] = set()
    for expected_slot, replica in enumerate(value, start=1):
        if (
            not isinstance(replica, dict)
            or set(replica) != INROU_CANARY_REPLICA_KEYS_V1
        ):
            fail(f"{context} has a malformed replica identity")
        slot = replica.get("replica_slot")
        if (
            type(slot) is not int
            or slot != expected_slot
            or replica.get("identity") != f"taira_inrou_canary:replica:{slot}"
        ):
            fail(f"{context} has a non-canonical replica identity")
        for field in (
            "response_sha256",
            "app_data_marker_sha256",
            "guest_boot_id_sha256",
        ):
            digest = replica.get(field)
            if not isinstance(digest, str) or LOWER_32_BYTE_HEX_RE.fullmatch(digest) is None:
                fail(f"{context} has a malformed {field}")
        app_data_markers.add(replica["app_data_marker_sha256"])
        guest_boot_ids.add(replica["guest_boot_id_sha256"])
        boot_sequence = replica.get("boot_sequence")
        if type(boot_sequence) is not int or not 1 <= boot_sequence < 1 << 64:
            fail(f"{context} has a malformed boot_sequence")
    if len(app_data_markers) != PEER_COUNT:
        fail(f"{context} repeats a durable app-data marker")
    if len(guest_boot_ids) != PEER_COUNT:
        fail(f"{context} repeats a guest boot identity")
    return value


def require_canonical_inrou_canary_receipt(
    receipt: object,
    expected_public_root: str,
    started_at_unix_ms: int | None = None,
    finished_at_unix_ms: int | None = None,
) -> dict[str, Any]:
    """Require one exact compiled four-replica Inrou success receipt."""

    if not isinstance(receipt, dict):
        fail("compiled Taira Inrou canary receipt is not a JSON object")
    if set(receipt) != INROU_CANARY_REPORT_KEYS_V1:
        fail("compiled Taira Inrou canary receipt violates the exact V1 schema")
    if (
        receipt.get("command") != "taira_inrou_canary"
        or receipt.get("status") != "ok"
        or receipt.get("public_root") != expected_public_root
        or receipt.get("mutation_mode") != "deploy"
        or receipt.get("service_name") != "taira_inrou_canary"
        or receipt.get("route_host") != INROU_CANARY_ROUTE_HOST_V1
        or receipt.get("route_path") != INROU_CANARY_HEALTH_PATH_V1
        or receipt.get("warnings") != []
        or receipt.get("failures") != []
    ):
        fail("compiled Taira Inrou canary did not report exact V1 deploy success")
    service_version = receipt.get("service_version")
    if not is_canonical_inrou_service_version(service_version):
        fail("compiled Taira Inrou canary has a malformed artifact-derived service_version")
    for field in ("active_host_adverts", "hosted_replica_count"):
        value = receipt.get(field)
        if type(value) is not int or value != PEER_COUNT:
            fail(f"compiled Taira Inrou canary receipt requires {field}=4")
    observed_at_unix_ms = receipt.get("observed_at_unix_ms")
    if (
        type(observed_at_unix_ms) is not int
        or not 1 <= observed_at_unix_ms < 1 << 64
    ):
        fail("compiled Taira Inrou canary receipt has malformed observed_at_unix_ms")
    if (started_at_unix_ms is None) != (finished_at_unix_ms is None):
        fail("compiled Taira Inrou canary freshness window is incomplete")
    if started_at_unix_ms is not None and finished_at_unix_ms is not None:
        if (
            type(started_at_unix_ms) is not int
            or type(finished_at_unix_ms) is not int
            or observed_at_unix_ms < started_at_unix_ms
            or observed_at_unix_ms > finished_at_unix_ms
        ):
            fail("compiled Taira Inrou canary evidence is not fresh for this invocation")

    for field in (
        "bundle_hash",
        "discovery_document_hash",
        "deployment_bundle_hash",
        "container_manifest_hash",
        "service_manifest_hash",
        "transaction_hash_hex",
    ):
        value = receipt.get(field)
        if not is_canonical_iroha_hash_hex(value):
            fail(f"compiled Taira Inrou canary receipt has malformed {field}")
    for field in (
        "prepared_envelope_sha256",
        "authorization_sha256",
        "idempotency_key",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or LOWER_32_BYTE_HEX_RE.fullmatch(value) is None:
            fail(f"compiled Taira Inrou canary receipt has malformed {field}")
    for field in (
        "bundle_content_cid",
        "guest_content_cid",
        "discovery_content_cid",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or SORAFS_CONTENT_CID_V1_RE.fullmatch(value) is None:
            fail(f"compiled Taira Inrou canary receipt has malformed {field}")
    for field in (
        "bundle_manifest_digest_hex",
        "guest_manifest_digest_hex",
        "discovery_manifest_digest_hex",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or LOWER_32_BYTE_HEX_RE.fullmatch(value) is None:
            fail(f"compiled Taira Inrou canary receipt has malformed {field}")
    nonce = receipt.get("authorization_nonce")
    expires_at_unix_ms = receipt.get("execution_expires_at_unix_ms")
    prepared_size = receipt.get("prepared_envelope_size")
    applied_height = receipt.get("applied_block_height")
    if (
        not isinstance(nonce, str)
        or re.fullmatch(r"[a-z0-9_-]{32}", nonce) is None
        or receipt.get("mutation_kind") != "inrou_canary"
        or receipt.get("mutation_phase") != PREPARED_MUTATION_PHASE
        or receipt.get("operation") != "service_mutation"
        or receipt.get("idempotency_key")
        != prepared_child_idempotency_key(
            nonce, PREPARED_MUTATION_PHASE, "inrou_canary"
        )
        or receipt.get("recovery_outcome") != "Applied"
        or type(expires_at_unix_ms) is not int
        or expires_at_unix_ms <= 0
        or type(prepared_size) is not int
        or prepared_size <= 0
        or prepared_size > MAX_PREPARED_ENVELOPE_BYTES
        or type(applied_height) is not int
        or applied_height <= 0
        or not is_canonical_iroha_hash_hex(receipt.get("evidence"))
        or receipt.get("evidence") != receipt.get("transaction_hash_hex")
    ):
        fail("compiled Taira Inrou canary receipt has malformed prepared evidence")
    _validate_fee_payment_v1(
        receipt.get("fee_payment"), "compiled Taira Inrou canary receipt.fee_payment"
    )
    _validate_fee_quote_v1(
        receipt.get("fee_quote"), "compiled Taira Inrou canary receipt.fee_quote"
    )

    discovery_cid = receipt["discovery_content_cid"]
    if (
        receipt.get("discovery_payload_dir") != str(INROU_STAGE_DISCOVERY_PAYLOAD)
        or receipt.get("public_discovery_url")
        != f"https://taira.sora.org/sorafs/cid/{discovery_cid}/index.json"
        or receipt.get("public_discovery_cid_host_url")
        != f"https://{discovery_cid}.sorafs.taira.sora.org/index.json"
    ):
        fail("compiled Taira Inrou canary has noncanonical discovery identity")
    checks = receipt.get("checks")
    if not isinstance(checks, list) or len(checks) != 3:
        fail("compiled Taira Inrou canary receipt has malformed checks")
    expected_checks = (
        (
            "inrou_authoritative_status",
            "active_adverts=4, hosted_replicas=4",
        ),
        (
            "inrou_public_routes",
            "observed distinct durable identities and guest boots for replica slots "
            "1, 2, 3, and 4",
        ),
        (
            "inrou_public_discovery",
            "current and revision authority plus public path and CID-host bytes, headers, and hash are exact",
        ),
    )
    for check, (name, detail) in zip(checks, expected_checks, strict=True):
        if not isinstance(check, dict) or set(check) != INROU_CANARY_CHECK_KEYS_V1:
            fail(f"compiled Taira Inrou canary check violates the V1 schema: {name}")
        if (
            check.get("name") != name
            or check.get("ok") is not True
            or type(check.get("http_status")) is not int
            or check.get("detail") != detail
        ):
            fail(f"compiled Taira Inrou canary check is malformed: {name}")
        if check["http_status"] != 200:
            fail(f"compiled Taira Inrou canary check did not pass: {name}")

    replicas = require_canonical_inrou_replicas(
        receipt.get("replica_identities"),
        "compiled Taira Inrou canary receipt",
    )
    placement = require_canonical_inrou_local_placement(
        receipt.get("local_placement"),
        "compiled Taira Inrou canary receipt.local_placement",
    )
    if placement["replica_slot"] not in {
        replica["replica_slot"] for replica in replicas
    }:
        fail("compiled Taira Inrou canary local placement has no matching replica identity")
    return receipt


def require_canonical_inrou_check_receipt(
    receipt: object,
    expected_public_root: str,
    stored_deploy_receipt: dict[str, Any],
    started_at_unix_ms: int,
    finished_at_unix_ms: int,
) -> dict[str, Any]:
    """Require one fresh exact read-only four-replica Inrou evidence receipt."""

    if not isinstance(receipt, dict):
        fail("compiled Taira Inrou check receipt is not a JSON object")
    if set(receipt) != INROU_CHECK_REPORT_KEYS_V1:
        fail("compiled Taira Inrou check receipt violates the exact V1 schema")
    if (
        receipt.get("command") != "taira_inrou_check"
        or receipt.get("status") != "ok"
        or receipt.get("public_root") != expected_public_root
        or receipt.get("service_name") != "taira_inrou_canary"
        or receipt.get("route_host") != INROU_CANARY_ROUTE_HOST_V1
        or receipt.get("route_path") != INROU_CANARY_HEALTH_PATH_V1
        or receipt.get("warnings") != []
        or receipt.get("failures") != []
    ):
        fail("compiled Taira Inrou check did not report exact V1 live success")
    service_version = receipt.get("service_version")
    if not is_canonical_inrou_service_version(service_version):
        fail("compiled Taira Inrou check has a malformed artifact-derived service_version")
    for retired in ("mutation_mode", "submitted_tx_hash", "mutation_response_digest"):
        if retired in receipt:
            fail(f"compiled Taira Inrou check exposed mutation-only field {retired}")
    for field in ("active_host_adverts", "hosted_replica_count"):
        value = receipt.get(field)
        if type(value) is not int or value != PEER_COUNT:
            fail(f"compiled Taira Inrou check receipt requires {field}=4")
    for field in (
        "bundle_hash",
        "discovery_document_hash",
        "deployment_bundle_hash",
        "container_manifest_hash",
        "service_manifest_hash",
    ):
        value = receipt.get(field)
        if not is_canonical_iroha_hash_hex(value):
            fail(f"compiled Taira Inrou check receipt has malformed {field}")
    for field in (
        "bundle_manifest_digest_hex",
        "guest_manifest_digest_hex",
        "discovery_manifest_digest_hex",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or LOWER_32_BYTE_HEX_RE.fullmatch(value) is None:
            fail(f"compiled Taira Inrou check receipt has malformed {field}")
    for field in (
        "bundle_content_cid",
        "guest_content_cid",
        "discovery_content_cid",
    ):
        value = receipt.get(field)
        if not isinstance(value, str) or SORAFS_CONTENT_CID_V1_RE.fullmatch(value) is None:
            fail(f"compiled Taira Inrou check receipt has malformed {field}")
    live_placement = require_canonical_inrou_local_placement(
        receipt.get("local_placement"),
        "compiled Taira Inrou check receipt.local_placement",
    )
    stored_placement = require_canonical_inrou_local_placement(
        stored_deploy_receipt.get("local_placement"),
        "stored Taira Inrou deploy receipt.local_placement",
    )
    if live_placement != stored_placement:
        fail("fresh Inrou check identity differs from stored deploy field local_placement")
    for field in (
        "service_name",
        "service_version",
        "route_host",
        "route_path",
        "bundle_hash",
        "bundle_content_cid",
        "bundle_manifest_digest_hex",
        "guest_content_cid",
        "guest_manifest_digest_hex",
        "discovery_payload_dir",
        "discovery_document_hash",
        "discovery_content_cid",
        "discovery_manifest_digest_hex",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "deployment_bundle_hash",
        "container_manifest_hash",
        "service_manifest_hash",
    ):
        if receipt.get(field) != stored_deploy_receipt.get(field):
            fail(f"fresh Inrou check identity differs from stored deploy field {field}")
    observed_at_unix_ms = receipt.get("observed_at_unix_ms")
    if (
        isinstance(observed_at_unix_ms, bool)
        or not isinstance(observed_at_unix_ms, int)
        or observed_at_unix_ms < started_at_unix_ms
        or observed_at_unix_ms > finished_at_unix_ms
    ):
        fail("compiled Taira Inrou check evidence is not fresh for this invocation")

    discovery_cid = receipt["discovery_content_cid"]
    if (
        receipt.get("discovery_payload_dir") != str(INROU_STAGE_DISCOVERY_PAYLOAD)
        or receipt.get("public_discovery_url")
        != f"https://taira.sora.org/sorafs/cid/{discovery_cid}/index.json"
        or receipt.get("public_discovery_cid_host_url")
        != f"https://{discovery_cid}.sorafs.taira.sora.org/index.json"
    ):
        fail("compiled Taira Inrou check has noncanonical discovery identity")

    checks = receipt.get("checks")
    if not isinstance(checks, list) or len(checks) != 3:
        fail("compiled Taira Inrou check receipt has malformed checks")
    expected_checks = (
        ("inrou_authoritative_status", "active_adverts=4, hosted_replicas=4"),
        (
            "inrou_public_routes",
            "observed distinct durable identities and guest boots for replica slots "
            "1, 2, 3, and 4",
        ),
        (
            "inrou_public_discovery",
            "current and revision authority plus public path and CID-host bytes, headers, and hash are exact",
        ),
    )
    for check, (name, detail) in zip(checks, expected_checks, strict=True):
        if not isinstance(check, dict) or set(check) != INROU_CANARY_CHECK_KEYS_V1:
            fail(f"compiled Taira Inrou check violates the V1 check schema: {name}")
        if (
            check.get("name") != name
            or check.get("ok") is not True
            or type(check.get("http_status")) is not int
            or check.get("http_status") != 200
            or check.get("detail") != detail
        ):
            fail(f"compiled Taira Inrou check did not pass: {name}")

    replicas = require_canonical_inrou_replicas(
        receipt.get("replica_identities"),
        "compiled Taira Inrou check receipt",
    )
    stored_replicas = require_canonical_inrou_replicas(
        stored_deploy_receipt.get("replica_identities"),
        "stored Taira Inrou deploy receipt",
    )
    for stored, live in zip(stored_replicas, replicas, strict=True):
        if live["identity"] != stored["identity"]:
            fail("fresh Inrou check has a substituted replica identity")
        if live["app_data_marker_sha256"] != stored["app_data_marker_sha256"]:
            fail("fresh Inrou check durable app-data marker differs from deployment")
    return receipt


def _require_restart_process_vector(
    value: object,
    target: Path,
    context: str,
) -> tuple[dict[str, object], ...]:
    """Require four exact V1 process identities in generated peer order."""

    if not isinstance(value, list) or len(value) != PEER_COUNT:
        fail(f"{context} must contain four exact peer process identities")
    processes = tuple(
        _canonical_taira_process_identity(
            identity,
            target,
            index,
            context=f"{context}[{index}]",
        )
        for index, identity in enumerate(value)
    )
    if len({identity["pid"] for identity in processes}) != PEER_COUNT:
        fail(f"{context} must identify four distinct live peer processes")
    return processes


def require_inrou_restart_transition(
    deploy_receipt: dict[str, Any],
    restart_receipt: dict[str, Any],
) -> None:
    """Require one durable reboot on the locally placed replica and no other drift."""

    placement = require_canonical_inrou_local_placement(
        deploy_receipt.get("local_placement"),
        "stored Taira Inrou deploy receipt.local_placement",
    )
    if restart_receipt.get("local_placement") != placement:
        fail("post-restart Inrou local placement differs from deployment")
    before_rows = require_canonical_inrou_replicas(
        deploy_receipt.get("replica_identities"),
        "stored Taira Inrou deploy receipt",
    )
    after_rows = require_canonical_inrou_replicas(
        restart_receipt.get("replica_identities"),
        "post-restart Taira Inrou check receipt",
    )
    restarted_slot = placement["replica_slot"]
    for before, after in zip(before_rows, after_rows, strict=True):
        slot = before["replica_slot"]
        if after["app_data_marker_sha256"] != before["app_data_marker_sha256"]:
            fail(f"Inrou replica slot {slot} lost its durable app-data marker on restart")
        if slot == restarted_slot:
            if after["boot_sequence"] != before["boot_sequence"] + 1:
                fail("selected Inrou replica boot sequence did not advance exactly once")
            if after["guest_boot_id_sha256"] == before["guest_boot_id_sha256"]:
                fail("selected Inrou replica guest boot identity did not change")
            if after["response_sha256"] == before["response_sha256"]:
                fail("selected Inrou replica health response did not change after reboot")
        elif after != before:
            fail(f"non-selected Inrou replica slot {slot} changed during exact-host restart")


def require_inrou_live_continuity(
    restart_receipt: dict[str, Any],
    live_receipt: dict[str, Any],
) -> None:
    """Reject durable rollback while allowing fully evidenced later guest boots."""

    if live_receipt.get("local_placement") != restart_receipt.get("local_placement"):
        fail("live Inrou local placement differs from restart qualification")
    stored_rows = require_canonical_inrou_replicas(
        restart_receipt.get("replica_identities"),
        "stored post-restart Taira Inrou receipt",
    )
    live_rows = require_canonical_inrou_replicas(
        live_receipt.get("replica_identities"),
        "fresh Taira Inrou continuity receipt",
    )
    for stored, live in zip(stored_rows, live_rows, strict=True):
        slot = stored["replica_slot"]
        if live["app_data_marker_sha256"] != stored["app_data_marker_sha256"]:
            fail(f"Inrou replica slot {slot} durable marker changed after qualification")
        if live["boot_sequence"] < stored["boot_sequence"]:
            fail(f"Inrou replica slot {slot} durable boot sequence rolled back")
        if live["boot_sequence"] == stored["boot_sequence"]:
            if (
                live["guest_boot_id_sha256"] != stored["guest_boot_id_sha256"]
                or live["response_sha256"] != stored["response_sha256"]
            ):
                fail(f"Inrou replica slot {slot} broke same-boot continuity")
        elif (
            live["guest_boot_id_sha256"] == stored["guest_boot_id_sha256"]
            or live["response_sha256"] == stored["response_sha256"]
        ):
            fail(f"Inrou replica slot {slot} advanced without a new guest boot identity")


def require_inrou_restart_proof(
    target: Path,
    expected_public_root: str,
    deploy_receipt: dict[str, Any],
    source_observation: dict[str, str],
    target_triple: str,
    value: object,
) -> dict[str, Any]:
    """Require the exact persisted V1 selected-validator restart proof."""

    if not isinstance(value, dict) or set(value) != INROU_RESTART_PROOF_KEYS_V1:
        fail("Inrou restart proof violates the exact V1 schema")
    if type(value.get("schema_version")) is not int or value["schema_version"] != 1:
        fail("Inrou restart proof has a non-V1 schema version")
    placement = require_canonical_inrou_local_placement(
        value.get("local_placement"),
        "Inrou restart proof.local_placement",
    )
    if placement != deploy_receipt.get("local_placement"):
        fail("Inrou restart proof placement differs from its deploy receipt")
    peer_index = value.get("peer_index")
    if type(peer_index) is not int or peer_index not in range(PEER_COUNT):
        fail("Inrou restart proof has a malformed peer_index")
    if peer_index != peer_index_for_local_placement(target, placement):
        fail("Inrou restart proof peer_index does not match local placement peer_id")

    processes_before = _require_restart_process_vector(
        value.get("processes_before"),
        target,
        "Inrou restart proof.processes_before",
    )
    processes_after = _require_restart_process_vector(
        value.get("processes_after"),
        target,
        "Inrou restart proof.processes_after",
    )
    before = processes_before[peer_index]
    after = processes_after[peer_index]
    if after == before:
        fail("Inrou restart proof did not replace the selected validator process")
    if (
        after["pid"] == before["pid"]
        and after["start_time_ticks"] == before["start_time_ticks"]
    ):
        fail("Inrou restart proof reused a PID without a new process start time")
    for index in range(PEER_COUNT):
        if index != peer_index and processes_after[index] != processes_before[index]:
            fail("Inrou restart proof changed a non-selected validator process")

    height_before = value.get("height_before")
    height_after = value.get("height_after")
    if (
        type(height_before) is not int
        or type(height_after) is not int
        or height_before <= 0
        or height_after <= height_before
    ):
        fail("Inrou restart proof does not show a strictly advancing cluster height")
    for field in ("start_script_sha256", "stop_script_sha256"):
        digest = value.get(field)
        if not isinstance(digest, str) or LOWER_32_BYTE_HEX_RE.fullmatch(digest) is None:
            fail(f"Inrou restart proof has a malformed {field}")
    if value["start_script_sha256"] != generated_script_sha256(target / "start.sh"):
        fail("generated start script changed after Inrou restart qualification")
    if value["stop_script_sha256"] != generated_script_sha256(target / "stop.sh"):
        fail("generated stop script changed after Inrou restart qualification")

    build_identity = value.get("build_identity")
    if (
        not isinstance(build_identity, dict)
        or set(build_identity) != INROU_RESTART_BUILD_IDENTITY_KEYS_V1
        or build_identity.get("git_commit_sha") != source_observation.get("git_head")
        or build_identity.get("target_triple") != target_triple
    ):
        fail("Inrou restart proof has a substituted validator build identity")
    started_at = value.get("inrou_check_started_at_unix_ms")
    finished_at = value.get("inrou_check_finished_at_unix_ms")
    if (
        type(started_at) is not int
        or type(finished_at) is not int
        or started_at <= 0
        or finished_at < started_at
    ):
        fail("Inrou restart proof has a malformed live-check observation window")
    restart_receipt = require_canonical_inrou_check_receipt(
        value.get("inrou_check"),
        expected_public_root,
        deploy_receipt,
        started_at,
        finished_at,
    )
    require_inrou_restart_transition(deploy_receipt, restart_receipt)
    return value


def _read_inrou_guest_qualification_payload(path: Path) -> object:
    """Read one stable owner-only qualification record without following links."""

    payload = read_stable_bytes(
        path,
        limit=MAX_INROU_GUEST_QUALIFICATION_BYTES,
        label="Inrou guest qualification record",
        owner=os.geteuid(),
        exact_mode=0o600,
        require_nonempty=True,
    )
    try:
        decoded = json_loads_no_duplicates(payload.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        fail("Inrou guest qualification record is not canonical JSON")
    canonical = (json.dumps(decoded, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )
    if payload != canonical:
        fail("Inrou guest qualification record is not canonical JSON")
    return decoded


def require_source_observation_evidence(value: object) -> dict[str, str]:
    """Require the exact retained branch and non-ignored worktree observation."""

    if not isinstance(value, dict) or set(value) != SOURCE_OBSERVATION_KEYS_V1:
        fail("Inrou guest qualification has malformed source observation evidence")
    branch = value.get("branch")
    git_head = value.get("git_head")
    digest = value.get("observed_nonignored_worktree_sha256")
    if branch != TAIRA_QUALIFICATION_BRANCH:
        fail("Inrou guest qualification was not produced from branch `optimizations`")
    if not isinstance(git_head, str) or LOWER_GIT_COMMIT_RE.fullmatch(git_head) is None:
        fail("Inrou guest qualification has a malformed source HEAD")
    if not isinstance(digest, str) or LOWER_32_BYTE_HEX_RE.fullmatch(digest) is None:
        fail("Inrou guest qualification has a malformed worktree observation digest")
    if (
        value.get("observation_scope")
        != "git_head_tracked_diff_nonignored_untracked"
        or value.get("cargo_source_consumption") != "not_proven"
    ):
        fail("Inrou guest qualification has a non-V1 source observation scope")
    return {
        "branch": branch,
        "git_head": git_head,
        "observation_scope": "git_head_tracked_diff_nonignored_untracked",
        "observed_nonignored_worktree_sha256": digest,
        "cargo_source_consumption": "not_proven",
    }


def require_compiled_tool_evidence(
    name: str,
    value: object,
    target_triple: str,
) -> dict[str, int | str]:
    """Require one retained compiled binary's exact path and content evidence."""

    if not isinstance(value, dict) or set(value) != COMPILED_TOOL_EVIDENCE_KEYS_V1:
        fail(f"Inrou guest qualification has malformed {name} evidence")
    path = value.get("path")
    sha256 = value.get("sha256")
    byte_count = value.get("bytes")
    if (
        not isinstance(path, str)
        or not path
        or not Path(path).is_absolute()
        or str(Path(path)) != path
    ):
        fail(f"Inrou guest qualification has a non-canonical {name} path")
    artifact_path = Path(path)
    if (
        artifact_path.resolve(strict=False) != artifact_path
        or ".." in artifact_path.parts
        or artifact_path.name != name
        or artifact_path.parent.name != TAIRA_BUILD_PROFILE
        or artifact_path.parent.parent.name != target_triple
    ):
        fail(f"Inrou guest qualification has a {name} path outside its target profile")
    if not isinstance(sha256, str) or LOWER_32_BYTE_HEX_RE.fullmatch(sha256) is None:
        fail(f"Inrou guest qualification has a malformed {name} hash")
    if isinstance(byte_count, bool) or not isinstance(byte_count, int) or byte_count <= 0:
        fail(f"Inrou guest qualification has a malformed {name} byte count")
    return {"path": path, "sha256": sha256, "bytes": byte_count}


def require_compiled_toolchain_evidence(
    value: object,
    target_triple: str,
) -> dict[str, dict[str, int | str]]:
    """Require all and only the four compiled tools used by qualification."""

    if not isinstance(value, dict) or set(value) != set(COMPILED_TOOLCHAIN_NAMES_V1):
        fail("Inrou guest qualification has malformed compiled toolchain evidence")
    toolchain = {
        name: require_compiled_tool_evidence(name, value[name], target_triple)
        for name in COMPILED_TOOLCHAIN_NAMES_V1
    }
    profile_directories = {
        str(Path(str(evidence["path"])).parent) for evidence in toolchain.values()
    }
    if len(profile_directories) != 1:
        fail("Inrou guest qualification binaries do not share one target profile")
    return toolchain


def require_inrou_guest_qualification(
    target: Path,
    expected_public_root: str,
) -> dict[str, Any]:
    """Require the exact persisted V1 guest-workload qualification record."""

    record = _read_inrou_guest_qualification_payload(
        target / INROU_GUEST_QUALIFICATION_FILE
    )
    if not isinstance(record, dict) or set(record) != INROU_GUEST_QUALIFICATION_KEYS_V1:
        fail("Inrou guest qualification record violates the exact V1 schema")
    if (
        type(record.get("schema_version")) is not int
        or record["schema_version"] != INROU_GUEST_QUALIFICATION_SCHEMA_VERSION_V1
        or record.get("inrou_guest_workload_qualification") != "verified"
    ):
        fail("Inrou guest qualification record is not verified V1 evidence")
    input_digest = record.get("inrou_canary_input_content_sha256")
    if (
        not isinstance(input_digest, str)
        or LOWER_32_BYTE_HEX_RE.fullmatch(input_digest) is None
    ):
        fail("Inrou guest qualification record has a malformed input digest")
    target_triple = record.get("target_triple")
    if (
        not isinstance(target_triple, str)
        or LINUX_AARCH64_TARGET_RE.fullmatch(target_triple) is None
    ):
        fail("Inrou guest qualification record has a malformed target identity")
    source_observation = require_source_observation_evidence(
        record.get("source_observation")
    )
    toolchain = require_compiled_toolchain_evidence(
        record.get("toolchain"),
        target_triple,
    )
    inrou_canary = require_canonical_inrou_canary_receipt(
        record.get("inrou_canary"),
        expected_public_root,
    )
    retained_guest = require_inrou_stage_guest_artifact(
        target / INROU_STAGE_DIRECTORY
    )
    if retained_guest != TrustedInrouGuestArtifact(
        manifest_digest_hex=inrou_canary["guest_manifest_digest_hex"],
        content_cid=inrou_canary["guest_content_cid"],
    ):
        fail("retained Inrou stage guest identity differs from qualification evidence")
    inrou_operator_preseed = require_canonical_inrou_operator_preseed_receipt(
        record.get("inrou_operator_preseed"),
        target,
        target / INROU_STAGE_DIRECTORY,
    )
    inrou_restart = require_inrou_restart_proof(
        target,
        expected_public_root,
        inrou_canary,
        source_observation,
        target_triple,
        record.get("inrou_restart"),
    )
    return {
        "schema_version": INROU_GUEST_QUALIFICATION_SCHEMA_VERSION_V1,
        "inrou_guest_workload_qualification": "verified",
        "inrou_canary_input_content_sha256": input_digest,
        "inrou_operator_preseed": inrou_operator_preseed,
        "inrou_canary": inrou_canary,
        "inrou_restart": inrou_restart,
        "source_observation": source_observation,
        "target_triple": target_triple,
        "toolchain": toolchain,
    }


def write_inrou_guest_qualification(
    target: Path,
    expected_public_root: str,
    input_content_sha256: str,
    inrou_operator_preseed: dict[str, Any],
    inrou_canary: dict[str, Any],
    inrou_restart: dict[str, Any],
    source_observation: dict[str, str],
    target_triple: str,
    toolchain: dict[str, dict[str, int | str]],
) -> dict[str, Any]:
    """Publish and read back one owner-only exact V1 qualification record."""

    if (
        not isinstance(input_content_sha256, str)
        or LOWER_32_BYTE_HEX_RE.fullmatch(input_content_sha256) is None
    ):
        fail("cannot persist a malformed Inrou canary input digest")
    canonical_source_observation = require_source_observation_evidence(source_observation)
    canonical_inrou_operator_preseed = require_canonical_inrou_operator_preseed_receipt(
        inrou_operator_preseed,
        target,
        target / INROU_STAGE_DIRECTORY,
    )
    canonical_inrou_canary = require_canonical_inrou_canary_receipt(
        inrou_canary,
        expected_public_root,
    )
    record = {
        "schema_version": INROU_GUEST_QUALIFICATION_SCHEMA_VERSION_V1,
        "inrou_guest_workload_qualification": "verified",
        "inrou_canary_input_content_sha256": input_content_sha256,
        "inrou_operator_preseed": canonical_inrou_operator_preseed,
        "inrou_canary": canonical_inrou_canary,
        "inrou_restart": require_inrou_restart_proof(
            target,
            expected_public_root,
            canonical_inrou_canary,
            canonical_source_observation,
            target_triple,
            inrou_restart,
        ),
        "source_observation": canonical_source_observation,
        "target_triple": target_triple,
        "toolchain": require_compiled_toolchain_evidence(toolchain, target_triple),
    }
    payload = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )
    if len(payload) > MAX_INROU_GUEST_QUALIFICATION_BYTES:
        fail("Inrou guest qualification record exceeds its exact safety bound")
    path = target / INROU_GUEST_QUALIFICATION_FILE
    if path.exists() or path.is_symlink():
        fail("refusing to replace an existing Inrou guest qualification record")
    try:
        descriptor, temporary_name = tempfile.mkstemp(
            dir=target,
            prefix=f".{INROU_GUEST_QUALIFICATION_FILE}.",
        )
    except OSError as error:
        fail(f"cannot stage Inrou guest qualification record: {error}")
    temporary = Path(temporary_name)
    try:
        try:
            os.fchmod(descriptor, 0o600)
            with os.fdopen(descriptor, "wb", closefd=True) as output:
                descriptor = -1
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
            os.link(temporary, path, follow_symlinks=False)
            temporary.unlink()
            fsync_directory(target, label="Inrou guest qualification directory")
        except OSError as error:
            fail(f"cannot publish Inrou guest qualification record: {error}")
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        temporary.unlink(missing_ok=True)
    return require_inrou_guest_qualification(target, expected_public_root)


def prepared_child_idempotency_key(nonce: str, phase: str, kind: str) -> str:
    """Derive the exact public-reset child idempotency key."""

    digest = hashlib.sha256()
    for frame in (
        b"iroha:taira:public-reset:child-idempotency:v1\0",
        nonce.encode("ascii"),
        phase.encode("ascii"),
        kind.encode("ascii"),
    ):
        digest.update(len(frame).to_bytes(8, "big"))
        digest.update(frame)
    return digest.hexdigest()


def _prepared_canary_authorization(
    target: Path,
    stage_dir: Path,
    timeout_seconds: float,
) -> tuple[str, str, int]:
    """Create one runtime-only authorization identity for the disposable cohort."""

    nonce = os.urandom(16).hex()
    genesis_hash = read_stable_bytes(
        target / "genesis.expected_hash",
        limit=256,
        label="generated genesis hash",
        require_nonempty=True,
    )
    stage_receipt = read_stable_bytes(
        stage_dir / INROU_STAGE_RECEIPT_FILE,
        limit=MAX_INROU_STAGE_RECEIPT_BYTES,
        label="native Taira Inrou stage receipt",
        owner=os.geteuid(),
        exact_mode=0o600,
        require_nonempty=True,
    )
    digest = hashlib.sha256()
    for frame in (
        b"iroha:taira:disposable-prepared-canary:v1\0",
        nonce.encode("ascii"),
        genesis_hash,
        stage_receipt,
    ):
        digest.update(len(frame).to_bytes(8, "big"))
        digest.update(frame)
    lease_seconds = max(
        PREPARED_EXECUTION_LEASE_MIN_SECONDS,
        max(1, int(timeout_seconds)) * 8,
    )
    expires_at_unix_ms = time.time_ns() // 1_000_000 + lease_seconds * 1_000
    return digest.hexdigest(), nonce, expires_at_unix_ms


def _prepare_prepared_canary_directory(target: Path) -> Path:
    """Create the fresh owner-only durable envelope directory."""

    directory = target / PREPARED_CANARY_DIRECTORY
    if directory.exists() or directory.is_symlink():
        fail("prepared canary envelope directory already exists")
    try:
        directory.mkdir(mode=0o700)
    except OSError as error:
        fail(f"cannot create prepared canary envelope directory: {error}")
    metadata = directory.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or directory.resolve(strict=True) != directory
    ):
        fail("prepared canary envelope directory lacks exact owner-only custody")
    return directory


def _exact_v1_object(
    value: Any,
    fields: frozenset[str],
    context: str,
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != fields:
        fail(f"{context} must contain exactly the V1 fields")
    return value


def _exact_v1_string(value: Any, context: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or any(character.isspace() for character in value)
    ):
        fail(f"{context} must be one exact nonempty token")
    return value


def _exact_v1_u64(value: Any, context: str, *, positive: bool = False) -> int:
    minimum = 1 if positive else 0
    if type(value) is not int or not minimum <= value < 1 << 64:
        fail(f"{context} must be an exact {'positive ' if positive else ''}u64")
    return value


def _exact_v1_lower_hex(
    value: Any,
    context: str,
    *,
    exact_bytes: int | None = None,
    max_bytes: int | None = None,
) -> str:
    if not isinstance(value, str) or re.fullmatch(r"(?:[0-9a-f]{2})+", value) is None:
        fail(f"{context} must be nonempty lowercase whole-byte hexadecimal")
    byte_length = len(value) // 2
    if exact_bytes is not None and byte_length != exact_bytes:
        fail(f"{context} must contain exactly {exact_bytes} bytes")
    if max_bytes is not None and byte_length > max_bytes:
        fail(f"{context} exceeds its {max_bytes}-byte V1 bound")
    return value


def _validate_quantity_v1(value: Any, context: str) -> Fraction:
    if (
        not isinstance(value, str)
        or len(value) > 155
        or re.fullmatch(r"(?:0|[1-9][0-9]*)(?:\.[0-9]{0,27}[1-9])?", value)
        is None
    ):
        fail(f"{context} must be one canonical V1 Quantity string")
    return Fraction(value)


def _validate_asset_definition_id_v1(value: Any, context: str) -> None:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[1-9A-HJ-NP-Za-km-z]{20,40}", value) is None
    ):
        fail(f"{context} must be one canonical Base58 asset-definition address")


def _validate_tagged_null_v1(
    value: Any,
    context: str,
    allowed_kinds: frozenset[str],
) -> None:
    tagged = _exact_v1_object(value, frozenset({"kind", "value"}), context)
    if tagged["kind"] not in allowed_kinds or tagged["value"] is not None:
        fail(f"{context} has an invalid closed V1 variant")


def _validate_fee_charge_limit_v1(
    value: Any,
    context: str,
) -> tuple[str, str, Fraction]:
    charge = _exact_v1_object(
        value,
        frozenset({"kind", "asset_definition_id", "max_amount"}),
        context,
    )
    _validate_tagged_null_v1(
        charge["kind"],
        f"{context}.kind",
        frozenset({"nexus", "pipeline_gas"}),
    )
    _validate_asset_definition_id_v1(
        charge["asset_definition_id"], f"{context}.asset_definition_id"
    )
    amount = _validate_quantity_v1(charge["max_amount"], f"{context}.max_amount")
    if amount <= 0:
        fail(f"{context}.max_amount must be positive")
    return charge["kind"]["kind"], charge["asset_definition_id"], amount


def _validate_fee_sponsor_program_id_v1(value: Any, context: str) -> None:
    program = _exact_v1_object(
        value,
        frozenset({"sponsor", "name"}),
        context,
    )
    _validate_fee_account_id_v1(program["sponsor"], f"{context}.sponsor")
    name = _exact_v1_string(program["name"], f"{context}.name")
    try:
        encoded_name = name.encode("utf-8")
    except UnicodeEncodeError:
        fail(f"{context}.name must be one exact canonical sponsor-program name")
    if (
        len(encoded_name) > 255
        or unicodedata.normalize("NFC", name) != name
        or any(
            unicodedata.category(character) == "Cc"
            or character in "\u061c\u200e\u200f"
            or "\u202a" <= character <= "\u202e"
            or "\u2066" <= character <= "\u2069"
            or character in "@#$/"
            for character in name
        )
    ):
        fail(f"{context}.name must be one exact canonical sponsor-program name")


def _validate_fee_account_id_v1(value: Any, context: str) -> str:
    account_id = _exact_v1_string(value, context)
    try:
        _decode_canonical_i105_account_id(account_id)
    except (TypeError, ValueError):
        fail(f"{context} must be one exact canonical I105 account id")
    return account_id


def _validate_fee_payment_v1(value: Any, context: str) -> None:
    payment = _exact_v1_object(value, frozenset({"payer", "value"}), context)
    payer = payment["payer"]
    if payer == "authority":
        fields = frozenset({"charge_limits", "gas_limit"})
    elif payer == "sponsor":
        fields = frozenset(
            {"program_id", "program_revision", "charge_limits", "gas_limit"}
        )
    else:
        fail(f"{context}.payer has an unknown V1 variant")
    body = _exact_v1_object(payment["value"], fields, f"{context}.value")
    limits = body["charge_limits"]
    if not isinstance(limits, list):
        fail(f"{context}.value.charge_limits must be an array")
    previous_kind_rank = -1
    kind_ranks = {"nexus": 0, "pipeline_gas": 1}
    for index, charge in enumerate(limits):
        kind, _, _ = _validate_fee_charge_limit_v1(
            charge, f"{context}.value.charge_limits[{index}]"
        )
        kind_rank = kind_ranks[kind]
        if kind_rank <= previous_kind_rank:
            fail(f"{context}.value.charge_limits are not canonical and unique")
        previous_kind_rank = kind_rank
    gas_limit = body["gas_limit"]
    if gas_limit is not None:
        _exact_v1_u64(gas_limit, f"{context}.value.gas_limit", positive=True)
    if payer == "sponsor":
        _validate_fee_sponsor_program_id_v1(
            body["program_id"], f"{context}.value.program_id"
        )
        _exact_v1_u64(
            body["program_revision"],
            f"{context}.value.program_revision",
            positive=True,
        )


def _validate_fee_quote_v1(
    value: Any,
    context: str,
    *,
    expected_fee_payment: Any | None = None,
    expected_authority: Any | None = None,
) -> None:
    quote = _exact_v1_object(
        value,
        frozenset({"intent", "observation", "components", "capacities", "decision"}),
        context,
    )
    _validate_fee_payment_v1(quote["intent"], f"{context}.intent")
    if expected_fee_payment is not None and not _fee_quote_intents_have_same_identity(
        quote["intent"], expected_fee_payment
    ):
        fail(f"{context}.intent differs from the exact prepared fee payment")
    observation = _exact_v1_object(
        quote["observation"],
        frozenset({"ledger_time_ms", "next_block_height", "route_dataspace_id"}),
        f"{context}.observation",
    )
    _exact_v1_u64(
        observation["ledger_time_ms"], f"{context}.observation.ledger_time_ms"
    )
    _exact_v1_u64(
        observation["next_block_height"],
        f"{context}.observation.next_block_height",
        positive=True,
    )
    _exact_v1_u64(
        observation["route_dataspace_id"],
        f"{context}.observation.route_dataspace_id",
    )
    components = quote["components"]
    if not isinstance(components, list):
        fail(f"{context}.components must be an array")
    for index, component in enumerate(components):
        _validate_fee_charge_limit_v1(component, f"{context}.components[{index}]")
    intent = quote["intent"]
    intent_body = intent["value"]
    if components != intent_body["charge_limits"]:
        fail(f"{context}.components must exactly match the quoted fee intent")
    capacities = quote["capacities"]
    if not isinstance(capacities, list):
        fail(f"{context}.capacities must be an array")
    capacity_amount_fields = (
        "vault_balance",
        "reserve_floor",
        "block_remaining",
        "program_epoch_remaining",
        "beneficiary_epoch_remaining",
    )
    capacity_fields = frozenset(("asset_definition_id", *capacity_amount_fields))
    parsed_capacities: list[tuple[str, dict[str, Fraction]]] = []
    for index, raw_capacity in enumerate(capacities):
        capacity_context = f"{context}.capacities[{index}]"
        capacity = _exact_v1_object(raw_capacity, capacity_fields, capacity_context)
        _validate_asset_definition_id_v1(
            capacity["asset_definition_id"],
            f"{capacity_context}.asset_definition_id",
        )
        amounts = {}
        for field in capacity_amount_fields:
            amounts[field] = _validate_quantity_v1(
                capacity[field], f"{capacity_context}.{field}"
            )
        parsed_capacities.append((capacity["asset_definition_id"], amounts))
    decision = _exact_v1_object(
        quote["decision"], frozenset({"status", "value"}), f"{context}.decision"
    )
    if decision["status"] != "accepted":
        fail(f"{context}.decision.status must be accepted")
    decision_value = _exact_v1_object(
        decision["value"],
        frozenset({"debit_source", "program_revision"}),
        f"{context}.decision.value",
    )
    debit = _exact_v1_object(
        decision_value["debit_source"],
        frozenset({"kind", "value"}),
        f"{context}.decision.value.debit_source",
    )
    revision = decision_value["program_revision"]
    if intent["payer"] == "authority":
        if debit["kind"] != "account" or revision is not None or capacities:
            fail(f"{context}.decision does not match its authority-paid intent")
        debit_account = _validate_fee_account_id_v1(
            debit["value"], f"{context}.decision.value.debit_source.value"
        )
        if expected_authority is not None and not _fee_quote_account_ids_have_same_identity(
            debit_account, expected_authority
        ):
            fail(f"{context}.decision debits a substituted authority")
    else:
        if debit["kind"] != "sponsor_program":
            fail(f"{context}.decision does not match its sponsored intent")
        _validate_fee_sponsor_program_id_v1(
            debit["value"], f"{context}.decision.value.debit_source.value"
        )
        _exact_v1_u64(
            revision,
            f"{context}.decision.value.program_revision",
            positive=True,
        )
        if (
            not _fee_quote_program_ids_have_same_identity(
                debit["value"], intent_body["program_id"]
            )
            or revision != intent_body["program_revision"]
        ):
            fail(f"{context}.decision differs from its exact sponsored intent")
        required_by_asset: dict[str, Fraction] = {}
        for index, component in enumerate(components):
            _, asset_definition_id, amount = _validate_fee_charge_limit_v1(
                component, f"{context}.components[{index}]"
            )
            required_by_asset[asset_definition_id] = (
                required_by_asset.get(asset_definition_id, Fraction()) + amount
            )
        expected_assets = sorted(required_by_asset)
        if [asset for asset, _ in parsed_capacities] != expected_assets:
            fail(
                f"{context}.capacities must contain exactly one canonical entry "
                "for every sponsored fee asset"
            )
        for asset_definition_id, amounts in parsed_capacities:
            required = required_by_asset[asset_definition_id]
            if amounts["vault_balance"] < amounts["reserve_floor"] + required:
                fail(f"{context}.capacities cannot cover the quoted vault charge")
            for field in (
                "block_remaining",
                "program_epoch_remaining",
                "beneficiary_epoch_remaining",
            ):
                if amounts[field] < required:
                    fail(f"{context}.capacities cannot cover the quoted {field}")


def _validate_public_reset_binding_v1(
    value: Any,
    context: str,
    expected_kind: str,
) -> dict[str, Any]:
    binding = _exact_v1_object(
        value,
        frozenset(
            {
                "schema",
                "authorization_sha256",
                "authorization_nonce",
                "kind",
                "phase",
                "idempotency_key",
                "execution_expires_at_unix_ms",
            }
        ),
        context,
    )
    if (
        binding["schema"] != "iroha.taira.public-reset.mutation-binding.v1"
        or binding["kind"] != expected_kind
    ):
        fail(f"{context} has a substituted V1 identity")
    _exact_v1_lower_hex(
        binding["authorization_sha256"],
        f"{context}.authorization_sha256",
        exact_bytes=32,
    )
    nonce = binding["authorization_nonce"]
    if not isinstance(nonce, str) or re.fullmatch(r"[a-z0-9_-]{32}", nonce) is None:
        fail(f"{context}.authorization_nonce is not exact V1")
    _exact_v1_string(binding["phase"], f"{context}.phase")
    _exact_v1_lower_hex(
        binding["idempotency_key"], f"{context}.idempotency_key", exact_bytes=32
    )
    _exact_v1_u64(
        binding["execution_expires_at_unix_ms"],
        f"{context}.execution_expires_at_unix_ms",
        positive=True,
    )
    return binding


def _validate_inrou_binding_v1(
    value: Any,
    context: str,
    expected_kind: str,
) -> dict[str, Any]:
    binding = _exact_v1_object(
        value,
        frozenset(
            {
                "authorization_sha256",
                "authorization_nonce",
                "kind",
                "phase",
                "idempotency_key",
                "execution_expires_at_unix_ms",
            }
        ),
        context,
    )
    if binding["kind"] != expected_kind:
        fail("prepared canary envelope closure has a substituted child binding kind")
    _exact_v1_lower_hex(
        binding["authorization_sha256"],
        f"{context}.authorization_sha256",
        exact_bytes=32,
    )
    nonce = binding["authorization_nonce"]
    if not isinstance(nonce, str) or re.fullmatch(r"[a-z0-9_-]{32}", nonce) is None:
        fail(f"{context}.authorization_nonce is not exact V1")
    _exact_v1_string(binding["phase"], f"{context}.phase")
    _exact_v1_lower_hex(
        binding["idempotency_key"], f"{context}.idempotency_key", exact_bytes=32
    )
    _exact_v1_u64(
        binding["execution_expires_at_unix_ms"],
        f"{context}.execution_expires_at_unix_ms",
        positive=True,
    )
    return binding


def _validate_hash_literal_v1(value: Any, context: str) -> None:
    if not isinstance(value, str) or IROHA_HASH_LITERAL_RE.fullmatch(value) is None:
        fail(f"{context} must be a canonical Iroha hash literal")


def _validate_signature_v1(value: Any, context: str) -> None:
    if not isinstance(value, str) or re.fullmatch(r"(?:[0-9A-F]{2})+", value) is None:
        fail(f"{context} must be one canonical uppercase signature literal")


def _validate_account_alias_name_v1(value: Any, context: str) -> None:
    alias = _exact_v1_object(
        value, frozenset({"label", "domain", "dataspace"}), context
    )
    _exact_v1_string(alias["label"], f"{context}.label")
    _exact_v1_string(alias["dataspace"], f"{context}.dataspace")
    if alias["domain"] is not None:
        _exact_v1_string(alias["domain"], f"{context}.domain")


def _validate_resolved_account_alias_v1(value: Any, context: str) -> None:
    alias = _exact_v1_object(
        value, frozenset({"canonical_name", "dataspace_id"}), context
    )
    _validate_account_alias_name_v1(alias["canonical_name"], f"{context}.canonical_name")
    _exact_v1_u64(alias["dataspace_id"], f"{context}.dataspace_id")


def _validate_alias_intent_v1(value: Any, context: str) -> None:
    tagged = _exact_v1_object(value, frozenset({"kind", "intent"}), context)
    if tagged["kind"] != "account_alias":
        fail(f"{context}.kind must be account_alias")
    intent = _exact_v1_object(
        tagged["intent"],
        frozenset({"alias", "target_account", "provision", "role"}),
        f"{context}.intent",
    )
    _validate_resolved_account_alias_v1(intent["alias"], f"{context}.intent.alias")
    _exact_v1_string(intent["target_account"], f"{context}.intent.target_account")
    _validate_tagged_null_v1(
        intent["provision"],
        f"{context}.intent.provision",
        frozenset({"existing", "create"}),
    )
    _validate_tagged_null_v1(
        intent["role"],
        f"{context}.intent.role",
        frozenset({"primary", "additional"}),
    )


def _validate_alias_target_v1(value: Any, context: str) -> None:
    tagged = _exact_v1_object(value, frozenset({"kind", "resource"}), context)
    if tagged["kind"] != "account_alias":
        fail(f"{context}.kind must be account_alias")
    _validate_resolved_account_alias_v1(tagged["resource"], f"{context}.resource")


def _validate_alias_disposition_v1(value: Any, context: str) -> None:
    _validate_tagged_null_v1(
        value,
        context,
        frozenset({"no_op", "repair", "create", "conflict"}),
    )


def _validate_alias_quote_guard_v1(value: Any, context: str) -> None:
    guard = _exact_v1_object(
        value,
        frozenset(
            {
                "expected_policy_version",
                "expected_payment_asset",
                "max_amount",
                "valid_until_ms",
            }
        ),
        context,
    )
    policy_version = guard["expected_policy_version"]
    if type(policy_version) is not int or not 0 <= policy_version <= 0xFFFF:
        fail(f"{context}.expected_policy_version must be a u16")
    _validate_asset_definition_id_v1(
        guard["expected_payment_asset"], f"{context}.expected_payment_asset"
    )
    _validate_quantity_v1(guard["max_amount"], f"{context}.max_amount")
    _exact_v1_u64(guard["valid_until_ms"], f"{context}.valid_until_ms", positive=True)


def _validate_alias_lease_quote_v1(value: Any, context: str) -> None:
    quote = _exact_v1_object(
        value,
        frozenset(
            {
                "target",
                "pricing_class",
                "exact_amount",
                "guard",
                "expires_at_ms",
                "grace_expires_at_ms",
                "redemption_expires_at_ms",
            }
        ),
        context,
    )
    _validate_alias_target_v1(quote["target"], f"{context}.target")
    pricing_class = quote["pricing_class"]
    if type(pricing_class) is not int or not 0 <= pricing_class <= 0xFF:
        fail(f"{context}.pricing_class must be a u8")
    _validate_quantity_v1(quote["exact_amount"], f"{context}.exact_amount")
    _validate_alias_quote_guard_v1(quote["guard"], f"{context}.guard")
    for field in ("expires_at_ms", "grace_expires_at_ms", "redemption_expires_at_ms"):
        _exact_v1_u64(quote[field], f"{context}.{field}", positive=True)


def _validate_alias_frame_v1(value: Any, context: str) -> None:
    frame = _exact_v1_object(
        value, frozenset({"wire_id", "framed_payload"}), context
    )
    _exact_v1_string(frame["wire_id"], f"{context}.wire_id")
    payload = frame["framed_payload"]
    if not isinstance(payload, list):
        fail(f"{context}.framed_payload must be an array")
    for index, byte in enumerate(payload):
        if type(byte) is not int or not 0 <= byte <= 0xFF:
            fail(f"{context}.framed_payload[{index}] must be a byte")


def _validate_onboarding_receipt_v1(value: Any, context: str) -> None:
    receipt = _exact_v1_object(
        value, frozenset({"body", "plan_hash", "signature"}), context
    )
    body = _exact_v1_object(
        receipt["body"],
        frozenset(
            {
                "version",
                "request",
                "authority",
                "network_id",
                "anchor",
                "resource",
                "acquisition",
                "quote_guard",
                "instructions",
                "owner_auto_renew_instruction",
                "valid_until_ms",
            }
        ),
        f"{context}.body",
    )
    if body["version"] != 1 or type(body["version"]) is not int:
        fail(f"{context}.body.version must be 1")
    request = _exact_v1_object(
        body["request"],
        frozenset({"version", "alias", "account_id", "permissions"}),
        f"{context}.body.request",
    )
    if request["version"] != 1 or type(request["version"]) is not int:
        fail(f"{context}.body.request.version must be 1")
    _exact_v1_string(request["alias"], f"{context}.body.request.alias")
    _exact_v1_string(request["account_id"], f"{context}.body.request.account_id")
    permissions = request["permissions"]
    if not isinstance(permissions, list):
        fail(f"{context}.body.request.permissions must be an array")
    for index, permission in enumerate(permissions):
        _exact_v1_string(permission, f"{context}.body.request.permissions[{index}]")
    _exact_v1_string(body["authority"], f"{context}.body.authority")
    _validate_hash_literal_v1(body["network_id"], f"{context}.body.network_id")
    anchor = _exact_v1_object(
        body["anchor"],
        frozenset({"block_height", "block_hash"}),
        f"{context}.body.anchor",
    )
    _exact_v1_u64(
        anchor["block_height"], f"{context}.body.anchor.block_height", positive=True
    )
    _validate_hash_literal_v1(anchor["block_hash"], f"{context}.body.anchor.block_hash")
    resource = _exact_v1_object(
        body["resource"],
        frozenset({"intent", "disposition", "quote", "instruction_index"}),
        f"{context}.body.resource",
    )
    _validate_alias_intent_v1(resource["intent"], f"{context}.body.resource.intent")
    _validate_alias_disposition_v1(
        resource["disposition"], f"{context}.body.resource.disposition"
    )
    if resource["quote"] is not None:
        _validate_alias_lease_quote_v1(
            resource["quote"], f"{context}.body.resource.quote"
        )
    instruction_index = resource["instruction_index"]
    if instruction_index is not None and (
        type(instruction_index) is not int or not 0 <= instruction_index <= 0xFFFFFFFF
    ):
        fail(f"{context}.body.resource.instruction_index must be null or u32")
    acquisition = _exact_v1_object(
        body["acquisition"],
        frozenset({"term_years", "pricing_class_hint"}),
        f"{context}.body.acquisition",
    )
    if type(acquisition["term_years"]) is not int or not 1 <= acquisition["term_years"] <= 0xFF:
        fail(f"{context}.body.acquisition.term_years must be a positive u8")
    pricing_hint = acquisition["pricing_class_hint"]
    if pricing_hint is not None and (
        type(pricing_hint) is not int or not 0 <= pricing_hint <= 0xFF
    ):
        fail(f"{context}.body.acquisition.pricing_class_hint must be null or u8")
    _validate_alias_quote_guard_v1(
        body["quote_guard"], f"{context}.body.quote_guard"
    )
    instructions = body["instructions"]
    if not isinstance(instructions, list):
        fail(f"{context}.body.instructions must be an array")
    for index, instruction in enumerate(instructions):
        _validate_alias_frame_v1(instruction, f"{context}.body.instructions[{index}]")
    if body["owner_auto_renew_instruction"] is not None:
        _validate_alias_frame_v1(
            body["owner_auto_renew_instruction"],
            f"{context}.body.owner_auto_renew_instruction",
        )
    _exact_v1_u64(
        body["valid_until_ms"], f"{context}.body.valid_until_ms", positive=True
    )
    _validate_hash_literal_v1(receipt["plan_hash"], f"{context}.plan_hash")
    _validate_signature_v1(receipt["signature"], f"{context}.signature")


def _validate_faucet_claim_v1(value: Any, context: str) -> None:
    claim = _exact_v1_object(
        value,
        frozenset({"account_id", "pow_anchor_height", "pow_nonce_hex"}),
        context,
    )
    _exact_v1_string(claim["account_id"], f"{context}.account_id")
    _exact_v1_u64(
        claim["pow_anchor_height"], f"{context}.pow_anchor_height", positive=True
    )
    _exact_v1_lower_hex(
        claim["pow_nonce_hex"], f"{context}.pow_nonce_hex", max_bytes=32
    )


def _validate_prepared_onboarding_v1(
    value: Any,
    context: str,
    root_binding: dict[str, Any],
) -> None:
    prepared = _exact_v1_object(
        value,
        frozenset(
            {
                "schema",
                "binding",
                "operation",
                "receipt",
                "semantic_hash_hex",
                "account_id",
                "alias",
                "disposition",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "fee_payment",
                "server_signature",
            }
        ),
        context,
    )
    if (
        prepared["schema"] != "iroha.taira.prepared-transaction.v1"
        or prepared["operation"] != "onboarding"
        or prepared["binding"] != root_binding
    ):
        fail(f"{context} has a substituted prepared-onboarding identity")
    _validate_onboarding_receipt_v1(prepared["receipt"], f"{context}.receipt")
    for field in (
        "semantic_hash_hex",
        "transaction_hash_hex",
        "signed_transaction_wire_sha256",
    ):
        _exact_v1_lower_hex(prepared[field], f"{context}.{field}", exact_bytes=32)
    _exact_v1_string(prepared["account_id"], f"{context}.account_id")
    _exact_v1_string(prepared["alias"], f"{context}.alias")
    _validate_alias_disposition_v1(prepared["disposition"], f"{context}.disposition")
    _exact_v1_lower_hex(
        prepared["signed_transaction_wire_hex"],
        f"{context}.signed_transaction_wire_hex",
    )
    _validate_fee_payment_v1(prepared["fee_payment"], f"{context}.fee_payment")
    _validate_signature_v1(prepared["server_signature"], f"{context}.server_signature")


def _validate_prepared_onboarding_proof_required_v1(
    value: Any,
    context: str,
    root_binding: dict[str, Any],
) -> None:
    wrapper = _exact_v1_object(
        value, frozenset({"schema", "receipt", "result"}), context
    )
    if wrapper["schema"] != "iroha.taira.prepared-onboarding-proof-required.v1":
        fail(f"{context}.schema is not the proof-required V1 schema")
    _validate_onboarding_receipt_v1(wrapper["receipt"], f"{context}.receipt")
    result = _exact_v1_object(
        wrapper["result"],
        frozenset(
            {
                "schema",
                "binding",
                "operation",
                "outcome",
                "proof_kind",
                "semantic_hash_hex",
                "account_id",
                "alias",
                "disposition",
                "server_signature",
            }
        ),
        f"{context}.result",
    )
    if (
        result["schema"] != "iroha.accounts.onboard.prepare-proof-required.v1"
        or result["binding"] != root_binding
        or result["operation"] != "onboarding"
        or result["outcome"] != "ProofRequired"
        or result["proof_kind"] != "account_alias_current_state"
    ):
        fail(f"{context}.result has a substituted proof-required identity")
    _exact_v1_lower_hex(
        result["semantic_hash_hex"], f"{context}.result.semantic_hash_hex", exact_bytes=32
    )
    _exact_v1_string(result["account_id"], f"{context}.result.account_id")
    _exact_v1_string(result["alias"], f"{context}.result.alias")
    _validate_alias_disposition_v1(
        result["disposition"], f"{context}.result.disposition"
    )
    _validate_signature_v1(
        result["server_signature"], f"{context}.result.server_signature"
    )


def _validate_prepared_faucet_v1(
    value: Any,
    context: str,
    root_binding: dict[str, Any],
) -> None:
    prepared = _exact_v1_object(
        value,
        frozenset(
            {
                "schema",
                "binding",
                "operation",
                "claim",
                "semantic_hash_hex",
                "account_id",
                "asset_definition_id",
                "asset_id",
                "amount",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "fee_payment",
                "server_signature",
            }
        ),
        context,
    )
    if (
        prepared["schema"] != "iroha.taira.prepared-transaction.v1"
        or prepared["operation"] != "faucet"
        or prepared["binding"] != root_binding
    ):
        fail(f"{context} has a substituted prepared-faucet identity")
    _validate_faucet_claim_v1(prepared["claim"], f"{context}.claim")
    for field in (
        "semantic_hash_hex",
        "transaction_hash_hex",
        "signed_transaction_wire_sha256",
    ):
        _exact_v1_lower_hex(prepared[field], f"{context}.{field}", exact_bytes=32)
    _exact_v1_string(prepared["account_id"], f"{context}.account_id")
    _validate_asset_definition_id_v1(
        prepared["asset_definition_id"], f"{context}.asset_definition_id"
    )
    _exact_v1_string(prepared["asset_id"], f"{context}.asset_id")
    _validate_quantity_v1(prepared["amount"], f"{context}.amount")
    _exact_v1_lower_hex(
        prepared["signed_transaction_wire_hex"],
        f"{context}.signed_transaction_wire_hex",
    )
    _validate_fee_payment_v1(prepared["fee_payment"], f"{context}.fee_payment")
    _validate_signature_v1(prepared["server_signature"], f"{context}.server_signature")


def _validate_final_canary_v1(
    value: Any,
    context: str,
    root_binding: dict[str, Any],
    expected_authority: str,
) -> None:
    prepared = _exact_v1_object(
        value,
        frozenset(
            {
                "schema",
                "binding",
                "operation",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "semantic_hash_hex",
                "fee_payment",
                "fee_quote",
            }
        ),
        context,
    )
    if (
        prepared["schema"] != "iroha.taira.prepared-transaction.v1"
        or prepared["operation"] != "final_canary"
        or prepared["binding"] != root_binding
    ):
        fail(f"{context} has a substituted final-canary identity")
    for field in (
        "transaction_hash_hex",
        "signed_transaction_wire_sha256",
        "semantic_hash_hex",
    ):
        _exact_v1_lower_hex(prepared[field], f"{context}.{field}", exact_bytes=32)
    _exact_v1_lower_hex(
        prepared["signed_transaction_wire_hex"],
        f"{context}.signed_transaction_wire_hex",
    )
    _validate_fee_payment_v1(prepared["fee_payment"], f"{context}.fee_payment")
    _validate_fee_quote_v1(
        prepared["fee_quote"],
        f"{context}.fee_quote",
        expected_fee_payment=prepared["fee_payment"],
        expected_authority=expected_authority,
    )


def _validate_inrou_stage_v1(value: Any, context: str) -> None:
    stage = _exact_v1_object(
        value,
        frozenset(
            {
                "service_name",
                "service_version",
                "route_host",
                "route_path_prefix",
                "healthcheck_path",
                "stage_mode",
                "bundle_hash",
                "bundle_content_cid",
                "bundle_manifest_digest_hex",
                "guest_content_cid",
                "guest_manifest_digest_hex",
                "discovery_payload_dir",
                "discovery_document_hash",
                "discovery_content_cid",
                "discovery_manifest_digest_hex",
                "public_discovery_url",
                "public_discovery_cid_host_url",
                "deployment_bundle_hash",
                "container_manifest_hash",
                "service_manifest_hash",
            }
        ),
        context,
    )
    for field, field_value in stage.items():
        _exact_v1_string(field_value, f"{context}.{field}")


def _validate_prepared_inrou_v1(
    value: Any,
    context: str,
    root_binding: dict[str, Any],
    expected_operation: str,
    expected_authority: str,
) -> None:
    prepared = _exact_v1_object(
        value,
        frozenset(
            {
                "schema",
                "binding",
                "operation",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "fee_payment",
                "fee_quote",
            }
        ),
        context,
    )
    if (
        prepared["schema"] != "iroha.taira.prepared-soracloud-transaction.v1"
        or prepared["binding"] != root_binding
        or prepared["operation"] != expected_operation
    ):
        fail(f"{context} has a substituted prepared-Inrou identity")
    for field in ("transaction_hash_hex", "signed_transaction_wire_sha256"):
        _exact_v1_lower_hex(prepared[field], f"{context}.{field}", exact_bytes=32)
    _exact_v1_lower_hex(
        prepared["signed_transaction_wire_hex"],
        f"{context}.signed_transaction_wire_hex",
    )
    _validate_fee_payment_v1(prepared["fee_payment"], f"{context}.fee_payment")
    _validate_fee_quote_v1(
        prepared["fee_quote"],
        f"{context}.fee_quote",
        expected_fee_payment=prepared["fee_payment"],
        expected_authority=expected_authority,
    )


def _validate_prepared_envelope_v1(
    envelope: Any,
    expected_public_root: str,
    expected_kind: str,
    expected_tags: set[str],
) -> dict[str, Any]:
    is_inrou = expected_kind.startswith("inrou_")
    root_fields = {
        "schema",
        "binding",
        "public_root",
        "chain_id",
        "network_id",
        "authority",
        "operation",
    }
    if is_inrou:
        root_fields.add("stage")
    root = _exact_v1_object(envelope, frozenset(root_fields), "prepared envelope")
    if (
        root["schema"] != "iroha.taira.prepared-mutation-envelope.v1"
        or root["public_root"] != expected_public_root
        or root["chain_id"] != DEFAULT_CHAIN_ID
    ):
        fail("prepared canary envelope closure has a substituted root identity")
    _exact_v1_string(root["network_id"], "prepared envelope.network_id")
    _exact_v1_string(root["authority"], "prepared envelope.authority")
    if is_inrou:
        binding = _validate_inrou_binding_v1(
            root["binding"], "prepared envelope.binding", expected_kind
        )
        _validate_inrou_stage_v1(root["stage"], "prepared envelope.stage")
    else:
        binding = _validate_public_reset_binding_v1(
            root["binding"], "prepared envelope.binding", expected_kind
        )
    tagged = _exact_v1_object(
        root["operation"],
        frozenset({"kind", "envelope"}),
        "prepared envelope.operation",
    )
    tag = tagged["kind"]
    if tag not in expected_tags:
        fail("prepared canary envelope closure has a substituted operation tag")
    payload = tagged["envelope"]
    context = "prepared envelope.operation.envelope"
    if tag == "onboarding_prepared":
        _validate_prepared_onboarding_v1(payload, context, binding)
    elif tag == "onboarding_proof_required":
        _validate_prepared_onboarding_proof_required_v1(payload, context, binding)
    elif tag == "faucet_prepared":
        _validate_prepared_faucet_v1(payload, context, binding)
    elif tag == "final_canary":
        _validate_final_canary_v1(payload, context, binding, root["authority"])
    elif tag in {
        "inrou_bundle_pin",
        "inrou_guest_pin",
        "inrou_discovery_pin",
        "inrou_canary",
    }:
        expected_operation = {
            "inrou_bundle_pin": "bundle_pin",
            "inrou_guest_pin": "guest_pin",
            "inrou_discovery_pin": "discovery_pin",
            "inrou_canary": "service_mutation",
        }[tag]
        _validate_prepared_inrou_v1(
            payload,
            context,
            binding,
            expected_operation,
            root["authority"],
        )
    else:
        fail("prepared canary envelope has an unsupported V1 operation tag")
    return binding


def _read_prepared_envelope(path: Path) -> tuple[bytes, dict[str, Any]]:
    """Read one immutable owner-only prepared envelope without following links."""

    payload = read_stable_bytes(
        path,
        limit=MAX_PREPARED_ENVELOPE_BYTES,
        label="prepared canary envelope",
        owner=os.geteuid(),
        exact_mode=0o600,
        require_nonempty=True,
    )
    if not payload.endswith(b"\n"):
        fail("prepared canary envelope is oversized or lacks its canonical newline")
    try:
        value = json_loads_no_duplicates(payload.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        fail("prepared canary envelope is not UTF-8 JSON")
    if not isinstance(value, dict):
        fail("prepared canary envelope root is not an object")
    canonical = (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")
    if canonical != payload:
        fail("prepared canary envelope is not exact canonical newline JSON")
    return payload, value


def _run_prepare_envelope(
    command: list[str],
    envelope_path: Path,
    predecessor_path: Path | None,
    target: Path,
    timeout_seconds: float,
    run: Runner,
) -> subprocess.CompletedProcess[str]:
    """Run one non-mutating prepare action into a fresh retained file."""

    flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        output_fd = os.open(envelope_path, flags, 0o600)
    except OSError as error:
        fail(f"cannot create prepared canary envelope: {error}")
    predecessor_fd: int | None = None
    try:
        command.extend(
            ["--prepare-envelope", "--prepared-output-fd", str(output_fd)]
        )
        inherited = [output_fd]
        if predecessor_path is not None:
            _read_prepared_envelope(predecessor_path)
            predecessor_fd = os.open(
                predecessor_path,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
            command.extend(
                ["--prerequisite-envelope-fd", str(predecessor_fd)]
            )
            inherited.append(predecessor_fd)
        completed = run(
            command,
            cwd=target,
            timeout=timeout_seconds + 30,
            pass_fds=tuple(inherited),
        )
        try:
            os.fsync(output_fd)
            fsync_directory(
                envelope_path.parent,
                label="prepared canary envelope directory",
            )
        except OSError as error:
            fail(f"cannot durably retain prepared canary envelope: {error}")
    finally:
        if predecessor_fd is not None:
            os.close(predecessor_fd)
        os.close(output_fd)
    return completed


def _run_retained_envelope_action(
    command: list[str],
    action_flag: str,
    envelope_path: Path,
    target: Path,
    timeout_seconds: float,
    run: Runner,
) -> subprocess.CompletedProcess[str]:
    """Submit or recover only the exact bytes in one retained envelope."""

    _read_prepared_envelope(envelope_path)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(envelope_path, flags)
    except OSError as error:
        fail(f"cannot open retained prepared envelope: {error}")
    try:
        command.extend([action_flag, str(descriptor)])
        try:
            return run(
                command,
                cwd=target,
                timeout=timeout_seconds + 30,
                pass_fds=(descriptor,),
            )
        except (DevnetError, subprocess.TimeoutExpired) as error:
            raise AmbiguousRetainedEnvelopeAction(str(error)) from error
    finally:
        os.close(descriptor)


def _prepared_report(
    completed: subprocess.CompletedProcess[str],
    *,
    command_name: str,
    public_root: str,
    authorization_sha256: str,
    authorization_nonce: str,
    kind: str,
    operation: str,
    idempotency_key: str,
    expires_at_unix_ms: int,
    envelope_path: Path,
    fee_fields: bool,
    service_applied: bool = False,
) -> dict[str, Any]:
    """Require one exact prepared-operation report and retained-byte identity."""

    try:
        receipt = json_loads_no_duplicates(completed.stdout or "")
    except (TypeError, ValueError):
        fail("compiled prepared canary child did not return JSON")
    if not isinstance(receipt, dict):
        fail("compiled prepared canary child receipt is not an object")
    outcome = receipt.get("recovery_outcome")
    expected_keys = PREPARED_REPORT_BASE_KEYS_V1 | PREPARED_REPORT_MUTATION_KEYS_V1
    if fee_fields:
        expected_keys |= PREPARED_REPORT_FEE_KEYS_V1
    if command_name == "taira_inrou_canary":
        expected_keys |= {"mutation_mode"}
    if service_applied and outcome == "Applied":
        expected_keys = INROU_CANARY_REPORT_KEYS_V1
    if set(receipt) != expected_keys:
        fail("compiled prepared canary child receipt violates the exact V1 schema")
    payload, envelope = _read_prepared_envelope(envelope_path)
    expected_tags = {
        "onboarding": {"onboarding_prepared", "onboarding_proof_required"},
        "faucet": {"faucet_prepared"},
        "write_canary": {"final_canary"},
        "inrou_bundle_pin": {"inrou_bundle_pin"},
        "inrou_guest_pin": {"inrou_guest_pin"},
        "inrou_discovery_pin": {"inrou_discovery_pin"},
        "inrou_canary": {"inrou_canary"},
    }.get(kind)
    if expected_tags is None:
        fail("compiled prepared canary child has an unsupported V1 kind")
    binding = _validate_prepared_envelope_v1(
        envelope,
        public_root,
        kind,
        expected_tags,
    )
    tagged = envelope["operation"]
    checks_are_valid = (
        service_applied and outcome == "Applied"
    ) or receipt.get("checks") == []
    if (
        receipt.get("command") != command_name
        or receipt.get("status") != "ok"
        or receipt.get("public_root") != public_root
        or not checks_are_valid
        or receipt.get("warnings") != []
        or receipt.get("failures") != []
        or receipt.get("authorization_sha256") != authorization_sha256
        or receipt.get("authorization_nonce") != authorization_nonce
        or receipt.get("mutation_kind") != kind
        or receipt.get("mutation_phase") != PREPARED_MUTATION_PHASE
        or receipt.get("idempotency_key") != idempotency_key
        or receipt.get("operation") != operation
        or (
            command_name == "taira_inrou_canary"
            and receipt.get("mutation_mode") != "deploy"
        )
        or receipt.get("execution_expires_at_unix_ms") != expires_at_unix_ms
        or receipt.get("prepared_envelope_sha256")
        != hashlib.sha256(payload).hexdigest()
        or receipt.get("prepared_envelope_size") != len(payload)
        or outcome
        not in {"Prepared", "ProofRequired", "Applied", "Pending", "Rejected"}
    ):
        fail("compiled prepared canary child receipt changed its exact binding")
    if (
        binding.get("authorization_sha256") != authorization_sha256
        or binding.get("authorization_nonce") != authorization_nonce
        or binding.get("kind") != kind
        or binding.get("phase") != PREPARED_MUTATION_PHASE
        or binding.get("idempotency_key") != idempotency_key
        or binding.get("execution_expires_at_unix_ms") != expires_at_unix_ms
    ):
        fail("retained prepared canary envelope changed its exact binding")
    transaction_hash = receipt.get("transaction_hash_hex")
    if transaction_hash is not None and not is_canonical_iroha_hash_hex(
        transaction_hash
    ):
        fail("prepared canary receipt has a malformed transaction hash")
    if outcome == "Prepared" and (
        transaction_hash is None
        or receipt.get("applied_block_height") is not None
        or receipt.get("evidence") is not None
    ):
        fail("Prepared child receipt has invalid transaction or terminal evidence")
    if outcome == "ProofRequired" and (
        kind != "onboarding"
        or tagged.get("kind") != "onboarding_proof_required"
        or transaction_hash is not None
        or receipt.get("applied_block_height") is not None
        or not isinstance(receipt.get("evidence"), str)
        or LOWER_32_BYTE_HEX_RE.fullmatch(receipt["evidence"]) is None
    ):
        fail("ProofRequired onboarding receipt violates its nonterminal proof contract")
    if outcome == "Applied":
        if transaction_hash is None:
            if (
                kind != "onboarding"
                or tagged.get("kind") != "onboarding_proof_required"
                or receipt.get("applied_block_height") is not None
            ):
                fail("only freshly proven onboarding may apply without a transaction")
        elif (
            type(receipt.get("applied_block_height")) is not int
            or receipt["applied_block_height"] <= 0
        ):
            fail("Applied prepared transaction omits its positive block height")
        evidence = receipt.get("evidence")
        if not isinstance(evidence, str) or not evidence:
            fail("Applied prepared child omits its exact evidence")
        if transaction_hash is None:
            if LOWER_32_BYTE_HEX_RE.fullmatch(evidence) is None:
                fail("freshly proven onboarding has malformed semantic evidence")
        elif not is_canonical_iroha_hash_hex(evidence) or evidence != transaction_hash:
            fail("Applied prepared child evidence differs from its transaction hash")
    if fee_fields:
        operation_payload = tagged.get("envelope")
        if not isinstance(operation_payload, dict):
            fail("prepared fee-bearing child omits its exact operation envelope")
        if (
            receipt.get("fee_payment") != operation_payload.get("fee_payment")
            or receipt.get("fee_quote") != operation_payload.get("fee_quote")
        ):
            fail("compiled prepared child fee evidence differs from its retained envelope")
        _validate_fee_payment_v1(
            receipt.get("fee_payment"), "compiled prepared child receipt.fee_payment"
        )
        _validate_fee_quote_v1(
            receipt.get("fee_quote"),
            "compiled prepared child receipt.fee_quote",
            expected_fee_payment=receipt.get("fee_payment"),
            expected_authority=envelope.get("authority"),
        )
    return receipt


def _base_write_canary_command(
    target: Path,
    iroha: Path,
    public_root: str,
    operation_cli: str,
    kind: str,
    authorization_sha256: str,
    authorization_nonce: str,
    expires_at_unix_ms: int,
    *,
    include_onboarding_token: bool,
) -> tuple[list[str], str]:
    idempotency_key = prepared_child_idempotency_key(
        authorization_nonce, PREPARED_MUTATION_PHASE, kind
    )
    command = [
        str(iroha),
        "-c",
        str(target / "client.toml"),
        "--fee-payer",
        "authority",
        "taira",
        "write-canary",
        "--public-root",
        public_root,
        "--operation",
        operation_cli,
        "--authorization-sha256",
        authorization_sha256,
        "--authorization-nonce",
        authorization_nonce,
        "--mutation-phase",
        PREPARED_MUTATION_PHASE,
        "--idempotency-key",
        idempotency_key,
        "--execution-expires-at-unix-ms",
        str(expires_at_unix_ms),
    ]
    if include_onboarding_token and kind == "onboarding":
        command.extend(
            [
                "--onboarding-token-file",
                str(target / LOCALNET_ONBOARDING_TOKEN_FILE),
            ]
        )
    if kind == "faucet":
        faucet_policy = require_trusted_localnet_faucet_policy(target)
        command.extend(
            [
                "--faucet-authority",
                faucet_policy.authority,
                "--faucet-asset-id",
                faucet_policy.asset_definition_id,
                "--faucet-amount",
                faucet_policy.amount,
            ]
        )
    command.append("--json")
    return command, idempotency_key


def _base_inrou_canary_command(
    target: Path,
    iroha: Path,
    public_root: str,
    stage_dir: Path,
    operation_cli: str,
    kind: str,
    authorization_sha256: str,
    authorization_nonce: str,
    expires_at_unix_ms: int,
    timeout_seconds: float,
) -> tuple[list[str], str]:
    idempotency_key = prepared_child_idempotency_key(
        authorization_nonce, PREPARED_MUTATION_PHASE, kind
    )
    return (
        [
            str(iroha),
            "-c",
            str(target / "client.toml"),
            "--fee-payer",
            "authority",
            "taira",
            "inrou-canary",
            "--public-root",
            public_root,
            "--stage-dir",
            str(stage_dir),
            "--mode",
            "deploy",
            "--operation",
            operation_cli,
            "--authorization-sha256",
            authorization_sha256,
            "--authorization-nonce",
            authorization_nonce,
            "--mutation-phase",
            PREPARED_MUTATION_PHASE,
            "--idempotency-key",
            idempotency_key,
            "--execution-expires-at-unix-ms",
            str(expires_at_unix_ms),
            "--timeout-secs",
            str(max(1, int(timeout_seconds))),
            "--json",
        ],
        idempotency_key,
    )


def _converge_prepared_child(
    *,
    prepare_command: list[str],
    submit_command: list[str],
    recover_command: list[str],
    envelope_path: Path,
    predecessor_path: Path | None,
    target: Path,
    timeout_seconds: float,
    run: Runner,
    report_args: dict[str, Any],
) -> dict[str, Any]:
    """Prepare once, submit at most once, and recover until one exact child is Applied."""

    deadline = time.monotonic() + timeout_seconds
    prepared = _run_prepare_envelope(
        prepare_command,
        envelope_path,
        predecessor_path,
        target,
        timeout_seconds,
        run,
    )
    receipt = _prepared_report(prepared, envelope_path=envelope_path, **report_args)
    preparation_outcome = receipt["recovery_outcome"]
    if preparation_outcome not in {"Prepared", "ProofRequired"}:
        fail("prepared child did not produce a forward-safe preparation outcome")
    proof_required = preparation_outcome == "ProofRequired"
    submit_was_ambiguous = False
    last_ambiguous_error: AmbiguousRetainedEnvelopeAction | None = None
    if proof_required:
        # The envelope is durable before this point. A no-op prepare result is
        # never terminal and never submittable; recovery performs one fresh
        # atomic account-and-alias observation against the retained result.
        receipt = None
    else:
        try:
            submitted = _run_retained_envelope_action(
                submit_command,
                "--submit-prepared-envelope-fd",
                envelope_path,
                target,
                max(1.0, deadline - time.monotonic()),
                run,
            )
        except AmbiguousRetainedEnvelopeAction as error:
            # The process may have lost its response after Torii accepted the exact
            # retained bytes. Never resubmit: recovery is the only safe next step.
            submit_was_ambiguous = True
            last_ambiguous_error = error
            receipt = None
        else:
            receipt = _prepared_report(
                submitted,
                envelope_path=envelope_path,
                **report_args,
            )
    recovery_attempted = False
    recovery_attempts = 0
    recovery_backoff = PREPARED_RECOVERY_INITIAL_BACKOFF_SECONDS
    while (
        receipt is None or receipt["recovery_outcome"] == "Pending"
    ) and (
        time.monotonic() < deadline
        or ((submit_was_ambiguous or proof_required) and not recovery_attempted)
    ) and recovery_attempts < PREPARED_RECOVERY_MAX_ATTEMPTS:
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(recovery_backoff, remaining))
        recovery_attempted = True
        recovery_attempts += 1
        try:
            recovered = _run_retained_envelope_action(
                recover_command.copy(),
                "--recover-prepared-envelope-fd",
                envelope_path,
                target,
                max(1.0, deadline - time.monotonic()),
                run,
            )
        except AmbiguousRetainedEnvelopeAction as error:
            last_ambiguous_error = error
            receipt = None
            recovery_backoff = min(
                PREPARED_RECOVERY_MAX_BACKOFF_SECONDS,
                recovery_backoff * 2,
            )
            continue
        receipt = _prepared_report(
            recovered,
            envelope_path=envelope_path,
            **report_args,
        )
        recovery_backoff = min(
            PREPARED_RECOVERY_MAX_BACKOFF_SECONDS,
            recovery_backoff * 2,
        )
    if receipt is None:
        detail = (
            f": {last_ambiguous_error}"
            if last_ambiguous_error is not None
            else ""
        )
        fail(
            "prepared canary child has no authoritative recovery outcome after "
            f"{recovery_attempts} recovery attempts{detail}"
        )
    if receipt["recovery_outcome"] != "Applied":
        fail(
            "prepared canary child did not reach Applied: "
            f"{receipt['recovery_outcome']} ({receipt.get('evidence')})"
        )
    return receipt


def require_prepared_canary_closure(
    target: Path,
    expected_public_root: str,
    stored_deploy_receipt: dict[str, Any] | None = None,
) -> tuple[Path, ...]:
    """Require all and only the seven exact retained prepared envelopes."""

    directory = target / PREPARED_CANARY_DIRECTORY
    try:
        metadata = directory.lstat()
    except OSError as error:
        fail(f"prepared canary envelope closure is missing: {error}")
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or directory.resolve(strict=True) != directory
    ):
        fail("prepared canary envelope closure lacks owner-only custody")
    specifications = (
        (
            "00-onboarding.json",
            "onboarding",
            {"onboarding_prepared", "onboarding_proof_required"},
        ),
        ("01-faucet.json", "faucet", {"faucet_prepared"}),
        ("02-final-canary.json", "write_canary", {"final_canary"}),
        ("03-bundle-pin.json", "inrou_bundle_pin", {"inrou_bundle_pin"}),
        ("04-guest-pin.json", "inrou_guest_pin", {"inrou_guest_pin"}),
        (
            "05-discovery-pin.json",
            "inrou_discovery_pin",
            {"inrou_discovery_pin"},
        ),
        ("06-service-mutation.json", "inrou_canary", {"inrou_canary"}),
    )
    expected_names = {name for name, _kind, _tags in specifications}
    actual_names = {entry.name for entry in directory.iterdir()}
    if actual_names != expected_names:
        fail("prepared canary envelope closure contains missing or unexpected files")
    paths: list[Path] = []
    common: tuple[str, str, int] | None = None
    for name, kind, tags in specifications:
        path = directory / name
        payload, envelope = _read_prepared_envelope(path)
        binding = _validate_prepared_envelope_v1(
            envelope,
            expected_public_root,
            kind,
            tags,
        )
        if (
            binding.get("phase") != PREPARED_MUTATION_PHASE
            or binding.get("idempotency_key")
            != prepared_child_idempotency_key(
                str(binding.get("authorization_nonce")), PREPARED_MUTATION_PHASE, kind
            )
        ):
            fail("prepared canary envelope closure has a substituted child")
        identity = (
            str(binding.get("authorization_sha256")),
            str(binding.get("authorization_nonce")),
            binding.get("execution_expires_at_unix_ms"),
        )
        if (
            LOWER_32_BYTE_HEX_RE.fullmatch(identity[0]) is None
            or re.fullmatch(r"[a-z0-9_-]{32}", identity[1]) is None
            or type(identity[2]) is not int
            or identity[2] <= 0
        ):
            fail("prepared canary envelope closure has a malformed authorization")
        if common is None:
            common = identity
        elif identity != common:
            fail("prepared canary envelope closure spans multiple authorizations")
        if (
            name == "06-service-mutation.json"
            and stored_deploy_receipt is not None
            and stored_deploy_receipt.get("prepared_envelope_sha256")
            != hashlib.sha256(payload).hexdigest()
        ):
            fail("stored Inrou receipt differs from its retained prepared envelope")
        paths.append(path)
    return tuple(paths)


def run_inrou_canary(
    target: Path,
    iroha: Path,
    root: str,
    stage_dir: Path,
    timeout_seconds: float,
    run: Runner,
) -> dict[str, Any]:
    """Execute and verify the exact seven-child prepared canary chain."""

    started_at_unix_ms = time.time_ns() // 1_000_000
    require_inrou_stage(stage_dir)
    public_root = root.rstrip("/")
    onboarding_token = target / LOCALNET_ONBOARDING_TOKEN_FILE
    if not onboarding_token.is_file() or onboarding_token.is_symlink():
        fail("generated localnet onboarding token is unavailable")
    directory = _prepare_prepared_canary_directory(target)
    authorization_sha256, authorization_nonce, expires_at_unix_ms = (
        _prepared_canary_authorization(target, stage_dir, timeout_seconds)
    )
    predecessor: Path | None = None
    for index, (kind, operation_cli, operation_report) in enumerate(
        PREPARED_WRITE_CHILDREN
    ):
        path = directory / f"{index:02d}-{operation_cli}.json"
        prepare_command, idempotency_key = _base_write_canary_command(
            target,
            iroha,
            public_root,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            include_onboarding_token=True,
        )
        submit_command, _ = _base_write_canary_command(
            target,
            iroha,
            public_root,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            include_onboarding_token=True,
        )
        recover_command, _ = _base_write_canary_command(
            target,
            iroha,
            public_root,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            include_onboarding_token=False,
        )
        _converge_prepared_child(
            prepare_command=prepare_command,
            submit_command=submit_command,
            recover_command=recover_command,
            envelope_path=path,
            predecessor_path=predecessor,
            target=target,
            timeout_seconds=timeout_seconds,
            run=run,
            report_args={
                "command_name": "taira_write_canary",
                "public_root": public_root,
                "authorization_sha256": authorization_sha256,
                "authorization_nonce": authorization_nonce,
                "kind": kind,
                "operation": operation_report,
                "idempotency_key": idempotency_key,
                "expires_at_unix_ms": expires_at_unix_ms,
                "fee_fields": kind == "write_canary",
            },
        )
        predecessor = path
    final_receipt: dict[str, Any] | None = None
    for offset, (kind, operation_cli, operation_report) in enumerate(
        PREPARED_INROU_CHILDREN,
        start=len(PREPARED_WRITE_CHILDREN),
    ):
        path = directory / f"{offset:02d}-{operation_cli}.json"
        prepare_command, idempotency_key = _base_inrou_canary_command(
            target,
            iroha,
            public_root,
            stage_dir,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            timeout_seconds,
        )
        submit_command, _ = _base_inrou_canary_command(
            target,
            iroha,
            public_root,
            stage_dir,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            timeout_seconds,
        )
        recover_command, _ = _base_inrou_canary_command(
            target,
            iroha,
            public_root,
            stage_dir,
            operation_cli,
            kind,
            authorization_sha256,
            authorization_nonce,
            expires_at_unix_ms,
            timeout_seconds,
        )
        final_receipt = _converge_prepared_child(
            prepare_command=prepare_command,
            submit_command=submit_command,
            recover_command=recover_command,
            envelope_path=path,
            predecessor_path=predecessor,
            target=target,
            timeout_seconds=timeout_seconds,
            run=run,
            report_args={
                "command_name": "taira_inrou_canary",
                "public_root": public_root,
                "authorization_sha256": authorization_sha256,
                "authorization_nonce": authorization_nonce,
                "kind": kind,
                "operation": operation_report,
                "idempotency_key": idempotency_key,
                "expires_at_unix_ms": expires_at_unix_ms,
                "fee_fields": True,
                "service_applied": kind == "inrou_canary",
            },
        )
        predecessor = path
    if final_receipt is None:
        fail("prepared Inrou canary chain produced no service receipt")
    finished_at_unix_ms = time.time_ns() // 1_000_000
    receipt = require_canonical_inrou_canary_receipt(
        final_receipt,
        public_root,
        started_at_unix_ms,
        finished_at_unix_ms,
    )
    peer_index_for_local_placement(target, receipt["local_placement"])
    require_prepared_canary_closure(target, public_root, receipt)
    return receipt


def run_inrou_check(
    target: Path,
    iroha: Path,
    root: str,
    stage_dir: Path,
    timeout_seconds: float,
    stored_deploy_receipt: dict[str, Any],
    run: Runner,
) -> tuple[dict[str, Any], int, int]:
    """Revalidate the retained stage and collect one fresh read-only live receipt."""

    require_inrou_stage(stage_dir)
    public_root = root.rstrip("/")
    started_at_unix_ms = time.time_ns() // 1_000_000
    completed = run(
        [
            str(iroha),
            "-c",
            str(target / "client.toml"),
            "taira",
            "inrou-check",
            "--mode",
            "deploy",
            "--public-root",
            public_root,
            "--stage-dir",
            str(stage_dir),
            "--timeout-secs",
            str(max(1, int(timeout_seconds))),
            "--json",
        ],
        cwd=target,
        timeout=timeout_seconds + 30,
    )
    finished_at_unix_ms = time.time_ns() // 1_000_000
    try:
        receipt = json_loads_no_duplicates(completed.stdout or "")
    except (TypeError, ValueError):
        fail("compiled Taira Inrou check did not return its JSON evidence")
    canonical_receipt = require_canonical_inrou_check_receipt(
        receipt,
        public_root,
        stored_deploy_receipt,
        started_at_unix_ms,
        finished_at_unix_ms,
    )
    peer_index_for_local_placement(target, canonical_receipt["local_placement"])
    return (
        canonical_receipt,
        started_at_unix_ms,
        finished_at_unix_ms,
    )


def qualify_inrou_host_restart(
    target: Path,
    iroha: Path,
    roots: Sequence[str],
    stage_dir: Path,
    deploy_receipt: dict[str, Any],
    peer_index: int,
    processes_before: Sequence[dict[str, object]],
    height_before: int,
    start_script_sha256: str,
    stop_script_sha256: str,
    source_observation: dict[str, str],
    target_triple: str,
    env: dict[str, str],
    timeout_seconds: float,
    run: Runner,
    request: Request,
) -> tuple[dict[str, Any], list[int]]:
    """Restart the exact local Inrou host and prove durable guest continuity."""

    if generated_script_sha256(target / "start.sh") != start_script_sha256:
        fail("generated start script changed before selected Inrou host restart")
    if generated_script_sha256(target / "stop.sh") != stop_script_sha256:
        fail("generated stop script changed before selected Inrou host restart")
    print(f"Restarting exact Inrou host peer{peer_index}...", flush=True)
    stop_peer_for_restart(target, peer_index, processes_before, run)
    if generated_script_sha256(target / "start.sh") != start_script_sha256:
        fail("generated start script changed while selected Inrou host was stopped")
    processes_after = start_peer_after_restart(
        target,
        peer_index,
        processes_before,
        env,
        run,
    )
    heights_after = wait_for_cluster(
        roots,
        timeout_seconds,
        request,
        above=height_before,
    )
    require_cluster_build_identity(
        roots,
        source_observation["git_head"],
        target_triple,
        request,
    )
    require_cli_build_identity(
        target,
        iroha,
        source_observation["git_head"],
        run,
        timeout_seconds,
    )
    inrou_check, check_started_at, check_finished_at = run_inrou_check(
        target,
        iroha,
        roots[0],
        stage_dir,
        timeout_seconds,
        deploy_receipt,
        run,
    )
    proof = {
        "schema_version": 1,
        "peer_index": peer_index,
        "local_placement": deploy_receipt["local_placement"],
        "processes_before": list(processes_before),
        "processes_after": list(processes_after),
        "height_before": height_before,
        "height_after": heights_after[0],
        "start_script_sha256": start_script_sha256,
        "stop_script_sha256": stop_script_sha256,
        "build_identity": {
            "git_commit_sha": source_observation["git_head"],
            "target_triple": target_triple,
        },
        "inrou_check_started_at_unix_ms": check_started_at,
        "inrou_check_finished_at_unix_ms": check_finished_at,
        "inrou_check": inrou_check,
    }
    return (
        require_inrou_restart_proof(
            target,
            roots[0].rstrip("/"),
            deploy_receipt,
            source_observation,
            target_triple,
            proof,
        ),
        heights_after,
    )


def reprove_retained_onboarding(
    target: Path,
    iroha: Path,
    public_root: str,
    timeout_seconds: float,
    run: Runner,
) -> dict[str, Any]:
    """Freshly classify the durable onboarding envelope without submitting it."""

    envelope_path = target / PREPARED_CANARY_DIRECTORY / "00-onboarding.json"
    _payload, envelope = _read_prepared_envelope(envelope_path)
    binding = envelope.get("binding")
    if not isinstance(binding, dict):
        fail("retained onboarding envelope omits its exact binding")
    authorization_sha256 = binding.get("authorization_sha256")
    authorization_nonce = binding.get("authorization_nonce")
    idempotency_key = binding.get("idempotency_key")
    expires_at_unix_ms = binding.get("execution_expires_at_unix_ms")
    if (
        not isinstance(authorization_sha256, str)
        or not isinstance(authorization_nonce, str)
        or not isinstance(idempotency_key, str)
        or type(expires_at_unix_ms) is not int
    ):
        fail("retained onboarding binding is malformed")
    recover_command, _ = _base_write_canary_command(
        target,
        iroha,
        public_root,
        "onboarding",
        "onboarding",
        authorization_sha256,
        authorization_nonce,
        expires_at_unix_ms,
        include_onboarding_token=False,
    )
    completed = _run_retained_envelope_action(
        recover_command,
        "--recover-prepared-envelope-fd",
        envelope_path,
        target,
        timeout_seconds,
        run,
    )
    receipt = _prepared_report(
        completed,
        command_name="taira_write_canary",
        public_root=public_root,
        authorization_sha256=authorization_sha256,
        authorization_nonce=authorization_nonce,
        kind="onboarding",
        operation="onboarding",
        idempotency_key=idempotency_key,
        expires_at_unix_ms=expires_at_unix_ms,
        envelope_path=envelope_path,
        fee_fields=False,
    )
    if receipt["recovery_outcome"] != "Applied":
        fail("fresh retained onboarding proof is not Applied")
    return receipt


def dump_logs(target: Path) -> None:
    """Print bounded daemon log tails without reading configs or key files."""

    for index in range(PEER_COUNT):
        path = target / f"peer{index}.log"
        if not path.is_file() or path.is_symlink():
            continue
        try:
            with path.open("rb") as stream:
                stream.seek(0, os.SEEK_END)
                start = max(0, stream.tell() - MAX_LOG_TAIL_BYTES)
                stream.seek(start)
                payload = stream.read(MAX_LOG_TAIL_BYTES)
        except OSError as error:
            print(f"\n--- cannot read {path}: {error} ---", file=sys.stderr)
            continue
        if start:
            _, separator, payload = payload.partition(b"\n")
            if not separator:
                payload = b""
        lines = payload.decode("utf-8", errors="replace").splitlines()[-40:]
        print(f"\n--- {path} (last {len(lines)} lines) ---", file=sys.stderr)
        print("\n".join(lines), file=sys.stderr)


def up(
    args: argparse.Namespace,
    *,
    run: Runner = run_command,
    request: Request = http_request,
) -> dict[str, Any]:
    """Replace the disposable network and prove one signed transaction finalizes."""

    _PROCESS_OPS.require_supported()
    require_inrou_qualification_host()
    inrou_canary_workspace = required_inrou_canary_workspace(args)
    root = managed_root(args.dir, create=True)
    root_identity = _direct_root_owned_directory_identity(
        root,
        label="managed devnet root",
        expected_owner=os.geteuid(),
    )
    target = network_dir(root)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    if args.target_dir is None:
        args.target_dir = root / "cargo-target"
    requested_target_dir = args.target_dir.expanduser().absolute().resolve(strict=False)
    if (
        requested_target_dir == target
        or requested_target_dir in target.parents
        or target in requested_target_dir.parents
    ):
        fail(
            "Taira Cargo target and disposable network directories must not overlap: "
            f"{requested_target_dir} and {target}"
        )
    source_observation = current_source_observation(run)
    kagami, irohad, iroha, sorafs_node, target_triple = binary_paths(args, run)
    if current_source_observation(run) != source_observation:
        fail(
            "observed non-ignored worktree changed while building the Taira "
            "qualification toolchain"
        )
    toolchain_evidence = compiled_toolchain_evidence(kagami, irohad, iroha, sorafs_node)
    preflight_cli_surfaces(
        kagami,
        irohad,
        iroha,
        sorafs_node,
        run,
        full_doctor=args.full_doctor,
    )
    _require_unchanged_inrou_canary_workspace(
        inrou_canary_workspace,
        phase="before the disposable cohort was replaced",
    )
    target = reset_network(root, run, root_identity)
    roots = torii_roots(args.base_api_port)
    inrou_stage: Path
    inrou_canary_outcome: dict[str, Any]
    restart_cleanup_peer_index: int | None = None
    restart_cleanup_processes: tuple[dict[str, object], ...] | None = None
    try:
        print("Generating a fresh four-validator Taira network...", flush=True)
        generate_network(
            target,
            kagami,
            args.base_api_port,
            args.base_p2p_port,
            args.block_cadence_ms,
            run,
        )
        require_generated_taira_profiles(target)
        require_bundle_identity(target, roots)
        print("Building and pre-seeding the exact Taira Inrou stage...", flush=True)
        inrou_stage = prepare_inrou_stage(
            target,
            iroha,
            inrou_canary_workspace,
            args.timeout_seconds,
            run,
        )
        trusted_guest = require_inrou_stage_guest_artifact(inrou_stage)
        require_canonical_taira_profiles(target, trusted_guest)
        validate_configs(target, irohad, trusted_guest, run)
        inrou_operator_preseed = preseed_inrou_stage(
            target,
            sorafs_node,
            inrou_stage,
            trusted_guest,
            args.timeout_seconds,
        )
        initial_start_script_sha256 = generated_script_sha256(target / "start.sh")
        initial_stop_script_sha256 = generated_script_sha256(target / "stop.sh")
        env = os.environ.copy()
        env.update(
            {
                "IROHAD_BIN": str(irohad),
                "IROHA_CLI": str(iroha),
                # The generated localnet start script can maintain a long-lived
                # faucet reserve. A disposable deployment owns no predecessor
                # state, so that retry loop only delays the authoritative smoke.
                "IROHA_LOCALNET_FAUCET_RESERVE_RETRIES": "0",
            }
        )
        run(
            ["/bin/bash", str(target / "start.sh")],
            cwd=target,
            env=env,
            timeout=60,
            capture_output=False,
        )
        initial_processes = require_running_cohort(target, run)
        # Health/readiness can become available before genesis is committed.
        # Do not quote or submit a signed transaction against the empty height-0
        # state, where the freshly generated authority is not registered yet.
        baseline = wait_for_cluster(roots, args.timeout_seconds, request, above=0)
        require_cluster_build_identity(
            roots,
            source_observation["git_head"],
            target_triple,
            request,
        )
        require_cli_build_identity(
            target,
            iroha,
            source_observation["git_head"],
            run,
            args.timeout_seconds,
        )
        print(
            f"Four validators converged at height {baseline[0]}; submitting signed smoke...",
            flush=True,
        )
        submitted = run(
            [
                str(iroha),
                "--machine",
                "-c",
                str(target / "client.toml"),
                "--fee-payer",
                "authority",
                "--output-format",
                "json",
                "tx",
                "ping",
                "--no-wait",
                "--log-level",
                "INFO",
                "--msg",
                "taira-devnet-ready",
            ],
            cwd=target,
            timeout=args.timeout_seconds,
        )
        transaction_hash = submitted_transaction_hash(submitted)
        print(
            f"Submitted {transaction_hash}; waiting for typed Applied status...",
            flush=True,
        )
        waited = run(
            [
                str(iroha),
                "--machine",
                "-c",
                str(target / "client.toml"),
                "--output-format",
                "json",
                "tx",
                "status",
                "--hash",
                transaction_hash,
                "--wait",
                "--timeout-ms",
                str(max(1, int(args.timeout_seconds * 1000))),
                "--poll-interval-ms",
                "250",
            ],
            cwd=target,
            timeout=args.timeout_seconds + 5,
        )
        require_applied_transaction(waited, transaction_hash)
        print("Signed smoke reached Applied; waiting for four-peer convergence...", flush=True)
        final = wait_for_cluster(roots, args.timeout_seconds, request, above=max(baseline))
        check_all_mcp(roots, request)
        inrou_canary_outcome = run_inrou_canary(
            target,
            iroha,
            roots[0],
            inrou_stage,
            args.timeout_seconds,
            run,
        )
        if trusted_guest != TrustedInrouGuestArtifact(
            manifest_digest_hex=inrou_canary_outcome["guest_manifest_digest_hex"],
            content_cid=inrou_canary_outcome["guest_content_cid"],
        ):
            fail("deployed Inrou canary guest identity differs from the trusted stage")
        final = wait_for_cluster(
            roots,
            args.timeout_seconds,
            request,
            above=max(final),
        )
        restart_peer_index = peer_index_for_local_placement(
            target,
            inrou_canary_outcome["local_placement"],
        )
        restart_cleanup_processes = require_running_cohort(target, run)
        if restart_cleanup_processes != initial_processes:
            fail("Taira peer process identities changed before selected Inrou host restart")
        restart_cleanup_peer_index = restart_peer_index
        inrou_restart, final = qualify_inrou_host_restart(
            target,
            iroha,
            roots,
            inrou_stage,
            inrou_canary_outcome,
            restart_peer_index,
            restart_cleanup_processes,
            max(final),
            initial_start_script_sha256,
            initial_stop_script_sha256,
            source_observation,
            target_triple,
            env,
            args.timeout_seconds,
            run,
            request,
        )
        restart_cleanup_peer_index = None
        restart_cleanup_processes = None
        if args.full_doctor:
            run_full_doctor(target, iroha, roots[0], run)
        if current_source_observation(run) != source_observation:
            fail("observed non-ignored worktree changed during Taira qualification")
        if compiled_toolchain_evidence(kagami, irohad, iroha, sorafs_node) != toolchain_evidence:
            fail("compiled Taira toolchain changed during qualification")
        guest_qualification = write_inrou_guest_qualification(
            target,
            roots[0].rstrip("/"),
            inrou_canary_workspace.content_sha256,
            inrou_operator_preseed,
            inrou_canary_outcome,
            inrou_restart,
            source_observation,
            target_triple,
            toolchain_evidence,
        )
    except (DevnetError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        cleanup_identity: tuple[int, int, int] | None = None
        cleanup_target_was_validated = False
        try:
            cleanup_identity = require_safe_cleanup_target(root, target)
            cleanup_target_was_validated = True
        except DevnetError as cleanup_error:
            print(
                f"warning: cannot safely identify failed Taira network: {cleanup_error}",
                file=sys.stderr,
            )
        stopped = stop_network(
            root,
            run,
            tolerate_failure=True,
            expected_partial_peer_index=restart_cleanup_peer_index,
            expected_partial_processes=restart_cleanup_processes,
        )
        dump_logs(target)
        if stopped and cleanup_target_was_validated:
            try:
                destroy_network(root, target, cleanup_identity)
            except DevnetError as cleanup_error:
                print(
                    f"warning: could not destroy stopped Taira network: {cleanup_error}",
                    file=sys.stderr,
                )
        if isinstance(error, subprocess.TimeoutExpired):
            fail(f"command timed out: {error.cmd}")
        if isinstance(error, KeyboardInterrupt):
            fail("Taira devnet startup was interrupted; bounded cohort teardown was attempted")
        raise
    report = {
        "directory": str(target),
        "client_config": str(target / "client.toml"),
        "torii_roots": list(roots),
        "baseline_height": baseline[0],
        "final_height": final[0],
        "transaction_hash": transaction_hash,
        "terminal_status": "Applied",
        "configured_inrou_vm_capacity_per_peer": TAIRA_INROU_VM_CAPACITY,
        "inrou_startup_boundary_qualified_peers": PEER_COUNT,
        "inrou_operator_preseed": guest_qualification["inrou_operator_preseed"],
        "inrou_canary": guest_qualification["inrou_canary"],
        "inrou_restart": guest_qualification["inrou_restart"],
        "inrou_guest_workload_qualification": "verified",
        "inrou_canary_input_content_sha256": guest_qualification[
            "inrou_canary_input_content_sha256"
        ],
        "source_observation": {
            **source_observation,
            "target_triple": target_triple,
            "stability_checks": "matched_before_after_build_and_qualification",
        },
        "toolchain": toolchain_evidence,
    }
    return report


def check(
    args: argparse.Namespace,
    *,
    run: Runner = run_command,
    request: Request = http_request,
) -> dict[str, Any]:
    """Revalidate retained qualification and collect fresh live read-only evidence."""

    _PROCESS_OPS.require_supported()
    root = managed_root(args.dir, create=False)
    target = require_network_bundle(root)
    roots = (
        bundle_torii_roots(target)
        if args.base_api_port is None
        else torii_roots(args.base_api_port)
    )
    guest_qualification = require_inrou_guest_qualification(
        target,
        roots[0].rstrip("/"),
    )
    require_prepared_canary_closure(
        target,
        roots[0].rstrip("/"),
        guest_qualification["inrou_canary"],
    )
    trusted_guest = require_inrou_stage_guest_artifact(target / INROU_STAGE_DIRECTORY)
    require_inrou_stage_placement_targets(target, target / INROU_STAGE_DIRECTORY)
    expected_trusted_guest = TrustedInrouGuestArtifact(
        manifest_digest_hex=guest_qualification["inrou_canary"][
            "guest_manifest_digest_hex"
        ],
        content_cid=guest_qualification["inrou_canary"]["guest_content_cid"],
    )
    if trusted_guest != expected_trusted_guest:
        fail("retained Inrou stage guest identity differs from qualification evidence")
    require_canonical_taira_profiles(target, trusted_guest)
    require_bundle_identity(target, roots)
    retained_input = require_inrou_canary_workspace(
        target / INROU_CANARY_INPUT_SNAPSHOT_DIRECTORY
    )
    if (
        retained_input.content_sha256
        != guest_qualification["inrou_canary_input_content_sha256"]
    ):
        fail("retained Inrou canary input snapshot digest changed after qualification")
    source_observation = current_source_observation(run)
    if source_observation != guest_qualification["source_observation"]:
        fail("current source observation differs from the retained Inrou qualification")
    toolchain = guest_qualification["toolchain"]

    def current_tool(name: str) -> tuple[str, dict[str, int | str]]:
        retained = toolchain[name]
        path = Path(str(retained["path"]))
        return name, {"path": str(path), **executable_evidence(path)}

    current_toolchain = dict(parallel_map(COMPILED_TOOLCHAIN_NAMES_V1, current_tool))
    for name in COMPILED_TOOLCHAIN_NAMES_V1:
        current = current_toolchain[name]
        retained = toolchain[name]
        if current != retained:
            fail(f"compiled {name} binary changed after Inrou qualification")
    iroha = Path(str(toolchain["iroha"]["path"]))
    current_processes = require_running_cohort(target, run)
    retained_processes = _require_restart_process_vector(
        guest_qualification["inrou_restart"].get("processes_after"),
        target,
        "retained Inrou restart proof.processes_after",
    )
    if current_processes != retained_processes:
        fail("current Taira process identities differ from restart qualification")
    heights = wait_for_cluster(roots, args.timeout_seconds, request)
    if heights[0] < guest_qualification["inrou_restart"]["height_after"]:
        fail("current Taira cluster height rolled back below restart qualification")
    require_cluster_build_identity(
        roots,
        guest_qualification["source_observation"]["git_head"],
        guest_qualification["target_triple"],
        request,
    )
    require_cli_build_identity(
        target,
        iroha,
        guest_qualification["source_observation"]["git_head"],
        run,
        args.timeout_seconds,
    )
    onboarding_live_proof = reprove_retained_onboarding(
        target,
        iroha,
        roots[0].rstrip("/"),
        args.timeout_seconds,
        run,
    )
    check_all_mcp(roots, request)
    inrou_live_check, _check_started_at, _check_finished_at = run_inrou_check(
        target,
        iroha,
        roots[0],
        target / INROU_STAGE_DIRECTORY,
        args.timeout_seconds,
        guest_qualification["inrou_canary"],
        run,
    )
    require_inrou_live_continuity(
        guest_qualification["inrou_restart"]["inrou_check"],
        inrou_live_check,
    )
    report = {
        "directory": str(target),
        "torii_roots": list(roots),
        "height": heights[0],
        "configured_inrou_vm_capacity_per_peer": TAIRA_INROU_VM_CAPACITY,
        "configured_peers": PEER_COUNT,
        "inrou_operator_preseed": guest_qualification["inrou_operator_preseed"],
        "inrou_stored_deploy_receipt": guest_qualification["inrou_canary"],
        "inrou_restart": guest_qualification["inrou_restart"],
        "inrou_live_check": inrou_live_check,
        "onboarding_live_proof": onboarding_live_proof,
        "inrou_guest_workload_qualification": "verified",
        "inrou_canary_input_content_sha256": guest_qualification[
            "inrou_canary_input_content_sha256"
        ],
        "source_observation": guest_qualification["source_observation"],
        "target_triple": guest_qualification["target_triple"],
        "toolchain": toolchain,
    }
    return report


def down(args: argparse.Namespace, *, run: Runner = run_command) -> dict[str, Any]:
    """Stop the peers and destroy the complete disposable network tree."""

    _PROCESS_OPS.require_supported()
    root = managed_root(args.dir, create=False)
    target = network_dir(root)
    target_identity = require_safe_cleanup_target(root, target)
    stop_network(root, run)
    destroy_network(root, target, target_identity)
    return {
        "directory": str(target),
        "network_destroyed": True,
        "stopped": True,
    }


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument(
        "--dir", type=Path, default=DEFAULT_DIR, help="managed disposable directory"
    )
    commands = result.add_subparsers(dest="command", required=True)

    up_parser = commands.add_parser("up", help="replace, start, and verify the devnet")
    up_parser.add_argument(
        "--target-dir",
        type=Path,
        help=(
            "owner-controlled Cargo target directory; defaults to cargo-target "
            "inside the managed devnet root"
        ),
    )
    up_parser.add_argument(
        "--build-timeout-seconds",
        type=float,
        help="optional Cargo build deadline; the default lets a cold build finish",
    )
    up_parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=DEFAULT_OPERATION_TIMEOUT_SECONDS,
        help="deadline for each startup, transaction, and convergence phase",
    )
    up_parser.add_argument("--base-api-port", type=int, default=DEFAULT_API_PORT)
    up_parser.add_argument("--base-p2p-port", type=int, default=DEFAULT_P2P_PORT)
    up_parser.add_argument(
        "--block-cadence-ms",
        type=int,
        default=DEFAULT_BLOCK_CADENCE_MS,
        help="signed cadence used to derive robust local consensus deadlines",
    )
    up_parser.add_argument(
        "--full-doctor",
        action="store_true",
        help="also require the broad public Taira product surface",
    )
    up_parser.add_argument(
        "--inrou-canary-dir",
        type=Path,
        required=True,
        help=(
            "required external owner-only workspace containing container_manifest.json, "
            "service_manifest.json, bundle.tgz, and their referenced Inrou guest assets"
        ),
    )
    up_parser.set_defaults(handler=up)

    check_parser = commands.add_parser("check", help="read four-peer readiness and height")
    check_parser.add_argument("--timeout-seconds", type=float, default=20)
    check_parser.add_argument(
        "--base-api-port",
        type=int,
        help="override the generated bundle's Torii base port",
    )
    check_parser.set_defaults(handler=check)

    down_parser = commands.add_parser(
        "down", help="stop the disposable peers and destroy the generated network"
    )
    down_parser.set_defaults(handler=down)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    """Run the selected disposable devnet operation."""

    args = parser().parse_args(argv)
    lock_descriptor: int | None = None
    try:
        _PROCESS_OPS.require_supported()
        root = managed_root(args.dir, create=args.command == "up")
        args.dir = root
        lock_descriptor = _acquire_operation_lock(root)
        report = args.handler(args)
    except DevnetError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    finally:
        if lock_descriptor is not None:
            os.close(lock_descriptor)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
