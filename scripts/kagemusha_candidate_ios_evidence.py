"""Strict shared validation for signed physical-iOS Kagemusha lab evidence."""

from __future__ import annotations

import base64
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile
from typing import Any, Optional


SIGNED_EVIDENCE_SCHEMA = "iroha.kagemusha.ios_device_lab.signed_evidence.v1"
SESSION_SCHEMA = "iroha.kagemusha.ios_device_lab.session.v1"
LAUNCH_RECEIPT_SCHEMA = "iroha.kagemusha.ios_device_lab.launch_receipt.v1"
NATIVE_TRANSCRIPT_SCHEMA = "iroha.kagemusha.ios_device_lab.native_transcript.v1"
NATIVE_BUILD_SCHEMA = "iroha.kagemusha.apple_candidate_native_build.v1"
CODE_SIGN_MEASUREMENTS_SCHEMA = (
    "iroha.kagemusha.ios_device_lab.code_sign_measurements.v1"
)
TEST_RESULT_SCHEMA = "iroha.kagemusha.ios_device_lab.test_result.v1"
REVIEWED_SOURCE_CLOSURE_SCHEMA = "iroha.reviewed-source-closure.v1"
DEVICE_POLICY = "taira-testnet-physical-ios-xcode-paired-v1"
RESOURCE_CEILING_BYTES = 6_442_450_944
BRIDGE_ABI_VERSION = 23
CANDIDATE_XCODE_VERSION = "Xcode 26.6"
XCODE_BUILD_VERSION_RE = re.compile(r"Build version [A-Za-z0-9.]+")
MAX_RAW_ARTIFACT_BYTES = 5 * 1024 * 1024 * 1024
MAX_JSON_BYTES = 64 * 1024 * 1024
MAX_KEY_BYTES = 64 * 1024
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
ED25519_PKCS8_SEED_PREFIX = bytes.fromhex("302e020100300506032b657004220420")
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
SOURCE_DIFF_DOMAIN = b"iroha-source-diff-v1\0"
TRACKED_DIFF_DOMAIN = b"tracked-binary-diff-sha256\0"
UNTRACKED_MANIFEST_DOMAIN = b"untracked-path-blob-manifest-sha256\0"
SCENARIO_INVENTORY_DOMAIN = (
    b"iroha.kagemusha.android-candidate-scenario-inventory.v1\0"
)

SHA256_RE = re.compile(r"[0-9a-f]{64}")
GIT_COMMIT_RE = re.compile(r"[0-9a-f]{40}")
KEY_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}")
DECIMAL_RE = re.compile(r"(?:0|[1-9][0-9]*)")
TEAM_ID_RE = re.compile(r"[A-Z0-9]{10}")
CDHASH_RE = re.compile(r"[0-9a-f]{40}")

SIGNED_EVIDENCE_FIELDS = frozenset(
    {
        "schema",
        "version",
        "artifact_digests",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
    }
)
ARTIFACT_DIGEST_FIELDS = frozenset({"size_bytes", "sha256"})

NATIVE_ARTIFACTS = (
    ("step_eq_params_ipa", "step-eq.params-ipa.krv4"),
    ("step_eq_proving_key", "step-eq.proving-key.krv4"),
    ("step_eq_verifying_key", "step-eq.verifying-key.krv4"),
    ("step_eq_bootstrap_witness", "step-eq.bootstrap-witness.krv4"),
    ("step_ep_params_ipa", "step-ep.params-ipa.krv4"),
    ("step_ep_proving_key", "step-ep.proving-key.krv4"),
    ("step_ep_verifying_key", "step-ep.verifying-key.krv4"),
    ("step_ep_bootstrap_witness", "step-ep.bootstrap-witness.krv4"),
)

SCENARIO_FILES = (
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-nonce.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-nonce.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-nonce.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)

NATIVE_SUPPORT_RAW_ARTIFACT_PATHS = frozenset(
    {
        "build/NoritoBridgeCandidateLab.xcframework/Info.plist",
        "build/NoritoBridgeCandidateLab.xcframework/"
        ".kagemusha-candidate-evidence-lab-do-not-ship-v2",
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/connect_norito_bridge.h",
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/connect_norito_bridge_base.h",
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/module.modulemap",
    }
)

EXPECTED_RAW_ARTIFACT_PATHS = frozenset(
    {
        "input/session-v1.json",
        "input/candidate-v4.norito",
        "input/candidate-manifest-v4.norito",
        "input/topup-finality-roster-v4.norito",
        "input/reviewed-source-closure-v1.json",
        "input/native-build-manifest.json",
        "build/libNoritoBridgeCandidateLab.a",
        "build/code-sign-measurements-v1.json",
        "run/proof-test-result-v1.json",
        "run/restart-test-result-v1.json",
        "output/install-identity-v1.bin",
        "output/checkpoint-v1.norito",
        "output/proof-launch-receipt-v1.json",
        "output/native-transcript-v1.json",
        "output/restart-launch-receipt-v1.json",
    }
    | {f"input/artifacts/{name}" for _, name in NATIVE_ARTIFACTS}
    | {f"input/scenario/{name}" for name in SCENARIO_FILES}
    | NATIVE_SUPPORT_RAW_ARTIFACT_PATHS
)
EXPECTED_RAW_DIRECTORIES = frozenset(
    {
        "input",
        "input/artifacts",
        "input/scenario",
        "build",
        "build/NoritoBridgeCandidateLab.xcframework",
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64",
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers",
        "run",
        "output",
    }
)
RAW_JSON_ARTIFACT_PATHS = frozenset(
    {
        "input/session-v1.json",
        "input/reviewed-source-closure-v1.json",
        "input/native-build-manifest.json",
        "build/code-sign-measurements-v1.json",
        "run/proof-test-result-v1.json",
        "run/restart-test-result-v1.json",
        "output/proof-launch-receipt-v1.json",
        "output/native-transcript-v1.json",
        "output/restart-launch-receipt-v1.json",
    }
)

SESSION_FIELDS = frozenset(
    {
        "schema",
        "version",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "topup_finality_roster_sha256",
        "scenario_inventory_sha256",
        "native_build_manifest_sha256",
        "native_library_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "reviewed_source_closure_descriptor_sha256",
        "device_udid_sha256",
        "device_ecid_sha256",
        "device_serial_sha256",
        "expected_hardware_model",
        "expected_board_config",
        "expected_os_version",
        "expected_os_build",
    }
)

LAUNCH_COMMON_FIELDS = frozenset(
    {
        "schema",
        "version",
        "phase",
        "process_id",
        "launch_nonce_sha256",
        "recorded_at_utc",
        "monotonic_nanos",
        "resource_ceiling_bytes",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "topup_finality_roster_sha256",
        "scenario_inventory_sha256",
        "native_build_manifest_sha256",
        "native_library_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "reviewed_source_closure_descriptor_sha256",
        "install_identity_sha256",
        "checkpoint_size_bytes",
        "checkpoint_sha256",
        "device",
        "code_identity",
        "network_monitor",
        "network_samples",
        "url_protocol_observed_request_count",
        "device_attestation_policy",
        "app_attest_used",
    }
)
RESTART_ONLY_FIELDS = frozenset(
    {
        "native_transcript_size_bytes",
        "native_transcript_sha256",
        "proof_launch_receipt_sha256",
    }
)
DEVICE_FIELDS = frozenset(
    {
        "physical",
        "simulator",
        "platform",
        "hardware_model",
        "board_config",
        "os_version",
        "os_build",
        "udid_sha256",
        "ecid_sha256",
        "serial_sha256",
        "identifier_for_vendor_sha256",
        "boot_session_sha256",
    }
)
CODE_IDENTITY_FIELDS = frozenset(
    {
        "app_bundle_id",
        "app_version",
        "app_build",
        "app_executable_sha256",
        "test_bundle_id",
        "test_executable_sha256",
    }
)
NETWORK_SAMPLE_FIELDS = frozenset(
    {
        "label",
        "monotonic_nanos",
        "status",
        "expensive",
        "constrained",
        "wifi",
        "cellular",
        "wired_ethernet",
        "loopback",
    }
)
REQUIRED_NETWORK_LABELS = (
    "callback",
    "before",
    "through_before_native",
    "through_after_native",
    "after",
)

TRANSCRIPT_DIGEST_FIELDS = (
    "source_tree_sha256",
    "reviewed_source_closure_descriptor_sha256",
    "candidate_record_sha256",
    "candidate_manifest_sha256",
    "native_accepted_inventory_sha256",
    "scenario_inventory_sha256",
    "checkpoint_sha256",
    "init_result_sha256",
    "split_hop_01_result_sha256",
    "split_hop_02_result_sha256",
    "proof_launch_nonce_sha256",
    "restart_launch_nonce_sha256",
)
TRANSCRIPT_DURATION_FIELDS = (
    "candidate_install_duration_ns",
    "candidate_reinstall_duration_ns",
    "init_duration_ns",
    "append_hop_01_duration_ns",
    "append_hop_02_duration_ns",
    "validate_hop_01_duration_ns",
    "validate_hop_02_duration_ns",
    "validate_change_duration_ns",
    "verify_hop_01_duration_ns",
    "verify_hop_02_duration_ns",
    "redeem_hop_01_duration_ns",
    "redeem_hop_02_duration_ns",
    "redeem_change_duration_ns",
    "duplicate_rejection_duration_ns",
)
TRANSCRIPT_TRUE_FIELDS = (
    "physical_device_required",
    "process_restart_observed",
    "init_succeeded",
    "two_hop_append_succeeded",
    "all_branches_restored",
    "recipient_proofs_verified",
    "all_branches_fully_redeemed",
    "duplicate_input_rejected",
)
TRANSCRIPT_FALSE_FIELDS = (
    "simulator_accepted",
    "source_repo_dirty",
    "production_capability_observed",
)
TRANSCRIPT_FIELDS = frozenset(
    {
        "schema",
        "version",
        "platform",
        *TRANSCRIPT_TRUE_FIELDS,
        *TRANSCRIPT_FALSE_FIELDS,
        "generation",
        "source_commit",
        "bridge_abi_version",
        *TRANSCRIPT_DIGEST_FIELDS,
        "proof_process_id",
        "restart_process_id",
        "resource_ceiling_bytes",
        "proof_peak_rss_bytes",
        "restart_peak_rss_bytes",
        *TRANSCRIPT_DURATION_FIELDS,
        "proof_hops",
        "exact_operation_count",
        "initial_atomic_units",
        "first_recipient_atomic_units",
        "second_recipient_atomic_units",
        "sender_change_atomic_units",
        "redeemed_atomic_units",
        "final_unspent_atomic_units",
        "asset_scale",
        "duplicate_error_code",
        "artifact_inventory",
        "causal_events",
    }
)
CAUSAL_EVENT_FIELDS = frozenset(
    {
        "sequence",
        "phase",
        "operation",
        "outcome",
        "duration_nanos",
        "input_sha256",
        "output_sha256",
        "output_size_bytes",
        "rejection_classification",
        "exception_class",
        "error_message_sha256",
    }
)
INVENTORY_FIELDS = frozenset(
    {
        "role",
        "framed_size_bytes",
        "framed_sha256",
        "payload_size_bytes",
        "payload_sha256",
    }
)
CAUSAL_OPERATIONS = (
    "candidate_install",
    "build_init_request",
    "init",
    "build_append_hop_01_request",
    "append_hop_01",
    "build_append_hop_02_request",
    "append_hop_02",
    "candidate_reinstall_after_process_restart",
    "restore_init_result_after_restart",
    "restore_hop_01_result_after_restart",
    "restore_hop_02_result_after_restart",
    "validate_init_branch_after_restart",
    "validate_hop_01_change_continuity",
    "validate_hop_01_recipient_branch",
    "validate_hop_02_recipient_branch",
    "validate_sender_change_branch",
    "build_verify_first_recipient_proof_request",
    "verify_first_recipient_proof",
    "build_verify_multi_hop_recipient_proof_request",
    "verify_multi_hop_recipient_proof",
    "build_duplicate_input_request_from_observed_branch",
    "duplicate_input_rejection",
    "build_redeem_first_recipient_request",
    "redeem_first_recipient",
    "build_redeem_second_recipient_request",
    "redeem_second_recipient",
    "build_redeem_sender_change_request",
    "redeem_sender_change",
)

NATIVE_BUILD_FIELDS = frozenset(
    {
        "schema",
        "version",
        "profile",
        "do_not_ship_marker",
        "candidate_feature_enabled",
        "production_capability_enabled",
        "bridge_abi_version",
        "target_triple",
        "architectures",
        "simulator_slice_present",
        "minimum_ios_version",
        "candidate_record_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "reviewed_source_closure_descriptor_sha256",
        "iphoneos_sdk_version",
        "xcode_version",
        "cargo_version_verbose",
        "rustc_version_verbose",
        "required_symbols",
        "files",
    }
)
NATIVE_BUILD_FILE_FIELDS = frozenset(
    {
        "NoritoBridgeCandidateLab.xcframework/Info.plist",
        "NoritoBridgeCandidateLab.xcframework/.kagemusha-candidate-evidence-lab-do-not-ship-v2",
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/libNoritoBridgeCandidateLab.a",
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge.h",
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge_base.h",
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/module.modulemap",
    }
)
NATIVE_BUILD_RAW_BINDINGS = {
    "NoritoBridgeCandidateLab.xcframework/Info.plist": (
        "build/NoritoBridgeCandidateLab.xcframework/Info.plist"
    ),
    "NoritoBridgeCandidateLab.xcframework/"
    ".kagemusha-candidate-evidence-lab-do-not-ship-v2": (
        "build/NoritoBridgeCandidateLab.xcframework/"
        ".kagemusha-candidate-evidence-lab-do-not-ship-v2"
    ),
    "NoritoBridgeCandidateLab.xcframework/ios-arm64/"
    "libNoritoBridgeCandidateLab.a": "build/libNoritoBridgeCandidateLab.a",
    "NoritoBridgeCandidateLab.xcframework/ios-arm64/"
    "Headers/connect_norito_bridge.h": (
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/connect_norito_bridge.h"
    ),
    "NoritoBridgeCandidateLab.xcframework/ios-arm64/"
    "Headers/connect_norito_bridge_base.h": (
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/connect_norito_bridge_base.h"
    ),
    "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/module.modulemap": (
        "build/NoritoBridgeCandidateLab.xcframework/ios-arm64/"
        "Headers/module.modulemap"
    ),
}
CODE_SIGN_MEASUREMENTS_FIELDS = frozenset(
    {
        "schema",
        "version",
        "app",
        "test",
        "native",
    }
)
CODE_SIGN_APP_FIELDS = frozenset(
    {
        "bundle_id",
        "version",
        "build",
        "identifier",
        "team_id",
        "cdhash",
        "executable_sha256",
        "entitlements_sha256",
        "provisioning_profile_sha256",
    }
)
CODE_SIGN_TEST_FIELDS = frozenset(
    {
        "bundle_id",
        "identifier",
        "team_id",
        "cdhash",
        "executable_sha256",
        "entitlements_sha256",
        "provisioning_profile_sha256",
    }
)
CODE_SIGN_NATIVE_FIELDS = frozenset(
    {
        "kind",
        "sha256",
        "build_manifest_sha256",
        "architectures",
        "simulator_slice_used",
    }
)
TEST_RESULT_FIELDS = frozenset(
    {
        "schema",
        "version",
        "phase",
        "test_status",
        "test_identifier",
        "launch_receipt_sha256",
        "native_transcript_sha256",
    }
)
REVIEWED_SOURCE_CLOSURE_FIELDS = frozenset(
    {
        "schema",
        "base_commit",
        "source_commit",
        "source_repo_dirty",
        "source_tree_sha256",
        "tracked_binary_diff_sha256",
        "untracked_file_count",
        "untracked_path_mode_blob_oid_manifest",
        "untracked_path_mode_blob_oid_manifest_sha256",
        "tracked_cargo_lock_size_bytes",
        "tracked_cargo_lock_sha256",
        "combined_source_fingerprint_sha256",
    }
)
REVIEWED_SOURCE_CLOSURE_ENTRY_FIELDS = frozenset(
    {
        "blob_sha256",
        "git_blob_oid",
        "git_mode",
        "path",
        "path_bytes_base64",
    }
)


class EvidenceError(ValueError):
    """Raised when evidence construction cannot proceed safely."""


class NewPrivateJsonPublicationUncertain(EvidenceError):
    """A new JSON output reached its final name without a confirmed commit."""

    def __init__(self, path: Path, inode_identity: tuple[int, int]) -> None:
        super().__init__("JSON output publication commit state is uncertain")
        self.path = path
        self.inode_identity = inode_identity


@dataclass(frozen=True)
class FileSnapshot:
    """One safely opened file's immutable validation input."""

    path: Path
    identity: tuple[int, ...]
    payload: bytes
    sha256: str
    size: int


@dataclass(frozen=True)
class RawArtifactSnapshot:
    """Exact raw-tree content and identities used by one validation decision."""

    root: Path
    directory_identities: dict[str, tuple[int, ...]]
    file_identities: dict[str, tuple[int, ...]]
    digests: dict[str, str]
    sizes: dict[str, int]
    json_payloads: dict[str, bytes]


def canonical_signature_payload(evidence: dict[str, Any]) -> bytes:
    """Return the signature payload, excluding exactly the two signature fields."""

    payload = {
        key: value
        for key, value in evidence.items()
        if key not in {"signature", "signature_payload_sha256"}
    }
    try:
        return json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise EvidenceError("signed evidence payload is not canonical strict JSON") from error


def canonical_json_bytes(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
            + b"\n"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise EvidenceError("value is not canonical strict JSON") from error


def _metadata_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _private_file_metadata(path: Path, label: str, maximum: int) -> os.stat_result:
    try:
        before = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"{label} is missing") from error
    except OSError as error:
        raise EvidenceError(f"{label} metadata could not be read") from error
    if stat.S_ISLNK(before.st_mode):
        raise EvidenceError(f"{label} must not be a symlink")
    if not stat.S_ISREG(before.st_mode):
        raise EvidenceError(f"{label} must be a regular file")
    if before.st_nlink != 1:
        raise EvidenceError(f"{label} must have exactly one hard link")
    if before.st_uid != os.geteuid():
        raise EvidenceError(f"{label} must be owned by the current user")
    if stat.S_IMODE(before.st_mode) & 0o077:
        raise EvidenceError(f"{label} must be owner-private")
    if before.st_size <= 0 or before.st_size > maximum:
        raise EvidenceError(f"{label} size is outside its bound")
    return before


def _snapshot_private_file(
    path: Path,
    label: str,
    *,
    maximum: int,
    retain_payload: bool,
) -> FileSnapshot:
    """Open once with ``O_NOFOLLOW`` and bind bytes to stable file metadata."""

    before = _private_file_metadata(path, label, maximum)
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if _metadata_identity(opened) != _metadata_identity(before):
            raise EvidenceError(f"{label} changed while opening")
        digest = hashlib.sha256()
        chunks: list[bytes] = []
        size = 0
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, maximum + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum:
                raise EvidenceError(f"{label} grew beyond its bound")
            digest.update(chunk)
            if retain_payload:
                chunks.append(chunk)
        after_open = os.fstat(descriptor)
        try:
            after_path = path.lstat()
        except OSError as error:
            raise EvidenceError(f"{label} disappeared while reading") from error
        identity = _metadata_identity(before)
        if (
            _metadata_identity(after_open) != identity
            or _metadata_identity(after_path) != identity
            or size != before.st_size
        ):
            raise EvidenceError(f"{label} changed while reading")
        return FileSnapshot(
            path=path,
            identity=identity,
            payload=b"".join(chunks) if retain_payload else b"",
            sha256=digest.hexdigest(),
            size=size,
        )
    finally:
        os.close(descriptor)


def read_private_file(
    path: Path,
    label: str,
    *,
    maximum: int = MAX_RAW_ARTIFACT_BYTES,
) -> bytes:
    """Read an owner-private, singly linked regular file without following aliases."""

    return _snapshot_private_file(
        path,
        label,
        maximum=maximum,
        retain_payload=True,
    ).payload


def _require_private_file_snapshot_unchanged(
    snapshot: FileSnapshot,
    label: str,
    *,
    maximum: int,
) -> None:
    current = _snapshot_private_file(
        snapshot.path,
        label,
        maximum=maximum,
        retain_payload=True,
    )
    if (
        current.identity != snapshot.identity
        or current.payload != snapshot.payload
        or current.sha256 != snapshot.sha256
        or current.size != snapshot.size
    ):
        raise EvidenceError(f"{label} changed after its immutable snapshot")


def _hash_private_file(path: Path, label: str) -> tuple[str, int]:
    snapshot = _snapshot_private_file(
        path,
        label,
        maximum=MAX_RAW_ARTIFACT_BYTES,
        retain_payload=False,
    )
    return snapshot.sha256, snapshot.size


def _validate_private_directory(path: Path, label: str) -> os.stat_result:
    try:
        value = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"{label} is missing") from error
    except OSError as error:
        raise EvidenceError(f"{label} metadata could not be read") from error
    if stat.S_ISLNK(value.st_mode):
        raise EvidenceError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(value.st_mode):
        raise EvidenceError(f"{label} must be a directory")
    if value.st_uid != os.geteuid():
        raise EvidenceError(f"{label} must be owned by the current user")
    if stat.S_IMODE(value.st_mode) & 0o077:
        raise EvidenceError(f"{label} must be owner-private")
    return value


def snapshot_raw_artifacts(artifact_root: Path) -> RawArtifactSnapshot:
    """Capture one exact raw tree for hashing and semantic validation."""

    root = artifact_root.absolute()
    root_before = _validate_private_directory(root, "artifact root")
    actual_directories: set[str] = set()
    actual_files: set[str] = set()
    directory_identities = {"": _metadata_identity(root_before)}
    file_identities: dict[str, tuple[int, ...]] = {}
    digests: dict[str, str] = {}
    sizes: dict[str, int] = {}
    json_payloads: dict[str, bytes] = {}

    for current_text, directory_names, file_names in os.walk(
        root, topdown=True, followlinks=False
    ):
        current = Path(current_text)
        for name in sorted(directory_names):
            child = current / name
            relative = child.relative_to(root).as_posix()
            metadata = _validate_private_directory(
                child, f"raw artifact directory {relative}"
            )
            actual_directories.add(relative)
            directory_identities[relative] = _metadata_identity(metadata)
        for name in sorted(file_names):
            child = current / name
            relative = child.relative_to(root).as_posix()
            if relative.startswith("/") or ".." in Path(relative).parts or "\\" in relative:
                raise EvidenceError("raw artifact path is not canonical")
            retain_payload = relative in RAW_JSON_ARTIFACT_PATHS
            if retain_payload:
                maximum = MAX_JSON_BYTES
            elif relative in NATIVE_SUPPORT_RAW_ARTIFACT_PATHS:
                maximum = 16 * 1024 * 1024
            else:
                maximum = MAX_RAW_ARTIFACT_BYTES
            snapshot = _snapshot_private_file(
                child,
                f"raw artifact {relative}",
                maximum=maximum,
                retain_payload=retain_payload,
            )
            actual_files.add(relative)
            file_identities[relative] = snapshot.identity
            digests[relative] = snapshot.sha256
            sizes[relative] = snapshot.size
            if retain_payload:
                json_payloads[relative] = snapshot.payload

    missing_directories = sorted(EXPECTED_RAW_DIRECTORIES - actual_directories)
    extra_directories = sorted(actual_directories - EXPECTED_RAW_DIRECTORIES)
    missing_files = sorted(EXPECTED_RAW_ARTIFACT_PATHS - actual_files)
    extra_files = sorted(actual_files - EXPECTED_RAW_ARTIFACT_PATHS)
    if missing_directories:
        raise EvidenceError(
            f"raw artifact tree is missing directories: {missing_directories}"
        )
    if extra_directories:
        raise EvidenceError(
            f"raw artifact tree contains extra directories: {extra_directories}"
        )
    if missing_files:
        raise EvidenceError(f"raw artifact tree is missing files: {missing_files}")
    if extra_files:
        raise EvidenceError(f"raw artifact tree contains extra files: {extra_files}")
    try:
        root_after = root.lstat()
    except OSError as error:
        raise EvidenceError("artifact root disappeared while scanning") from error
    if _metadata_identity(root_after) != _metadata_identity(root_before):
        raise EvidenceError("artifact root changed while scanning")
    return RawArtifactSnapshot(
        root=root,
        directory_identities=dict(sorted(directory_identities.items())),
        file_identities=dict(sorted(file_identities.items())),
        digests=dict(sorted(digests.items())),
        sizes=dict(sorted(sizes.items())),
        json_payloads=dict(sorted(json_payloads.items())),
    )


def _require_raw_snapshot_unchanged(snapshot: RawArtifactSnapshot) -> None:
    """Perform the final identity and content rescan for one decision."""

    current = snapshot_raw_artifacts(snapshot.root)
    if (
        current.directory_identities != snapshot.directory_identities
        or current.file_identities != snapshot.file_identities
        or current.digests != snapshot.digests
        or current.sizes != snapshot.sizes
        or current.json_payloads != snapshot.json_payloads
    ):
        raise EvidenceError(
            "raw artifact tree changed after its validated immutable snapshot"
        )


def scan_raw_artifacts(artifact_root: Path) -> tuple[dict[str, str], dict[str, int]]:
    """Return the exact raw artifact digest map after strict tree validation."""

    snapshot = snapshot_raw_artifacts(artifact_root)
    return snapshot.digests, snapshot.sizes


def _reject_constant(value: str) -> None:
    raise EvidenceError(f"non-finite JSON value is forbidden: {value}")


def _pairs_to_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"duplicate JSON key is forbidden: {key}")
        result[key] = value
    return result


def parse_strict_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload.decode("utf-8"),
            object_pairs_hook=_pairs_to_object,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, EvidenceError) as error:
        raise EvidenceError(f"{label} is not strict JSON: {error}") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be a JSON object")
    if payload != canonical_json_bytes(value):
        raise EvidenceError(f"{label} bytes are not canonical JSON")
    return value


def load_private_json(path: Path, label: str) -> dict[str, Any]:
    return parse_strict_json(
        read_private_file(path, label, maximum=MAX_JSON_BYTES),
        label,
    )


def _parse_snapshot_json(
    snapshot: RawArtifactSnapshot,
    relative: str,
    label: str,
) -> dict[str, Any]:
    try:
        payload = snapshot.json_payloads[relative]
    except KeyError as error:
        raise EvidenceError(f"{label} is missing from the raw snapshot") from error
    return parse_strict_json(payload, label)


def _exact_fields(
    value: Any,
    expected: frozenset[str],
    label: str,
    errors: list[str],
) -> Optional[dict[str, Any]]:
    if not isinstance(value, dict):
        errors.append(f"{label} must be an object")
        return None
    keys = set(value)
    missing = sorted(expected - keys)
    unknown = sorted(keys - expected)
    if missing:
        errors.append(f"{label} is missing fields: {missing}")
    if unknown:
        errors.append(f"{label} contains unknown fields: {unknown}")
    return value


def _is_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _require_int(
    value: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
    *,
    minimum: int = 0,
    exact: Optional[int] = None,
) -> Optional[int]:
    candidate = value.get(key)
    if not _is_int(candidate) or candidate < minimum:
        errors.append(f"{label} {key} must be an integer >= {minimum}")
        return None
    if exact is not None and candidate != exact:
        errors.append(f"{label} {key} must be {exact}")
        return None
    return candidate


def _require_string(
    value: dict[str, Any], key: str, label: str, errors: list[str]
) -> Optional[str]:
    candidate = value.get(key)
    if (
        not isinstance(candidate, str)
        or not candidate
        or candidate != candidate.strip()
        or any(ord(character) < 0x20 for character in candidate)
    ):
        errors.append(f"{label} {key} must be a non-empty canonical string")
        return None
    return candidate


def _require_digest(
    value: dict[str, Any], key: str, label: str, errors: list[str]
) -> Optional[str]:
    candidate = value.get(key)
    if (
        not isinstance(candidate, str)
        or SHA256_RE.fullmatch(candidate) is None
        or candidate == "0" * 64
    ):
        errors.append(f"{label} {key} must be nonzero lowercase SHA-256")
        return None
    return candidate


def _require_bool(
    value: dict[str, Any],
    key: str,
    expected: bool,
    label: str,
    errors: list[str],
) -> None:
    if value.get(key) is not expected:
        errors.append(f"{label} {key} must be {str(expected).lower()}")


def _validate_session(
    session: dict[str, Any],
    digests: dict[str, str],
    sizes: dict[str, int],
    errors: list[str],
) -> None:
    if _exact_fields(session, SESSION_FIELDS, "session", errors) is None:
        return
    if session.get("schema") != SESSION_SCHEMA:
        errors.append(f"session schema must be {SESSION_SCHEMA}")
    _require_int(session, "version", "session", errors, exact=1)
    _require_bool(session, "source_repo_dirty", False, "session", errors)
    digest_bindings = {
        "candidate_record_sha256": "input/candidate-v4.norito",
        "candidate_manifest_sha256": "input/candidate-manifest-v4.norito",
        "topup_finality_roster_sha256": "input/topup-finality-roster-v4.norito",
        "native_build_manifest_sha256": "input/native-build-manifest.json",
        "native_library_sha256": "build/libNoritoBridgeCandidateLab.a",
        "reviewed_source_closure_descriptor_sha256": (
            "input/reviewed-source-closure-v1.json"
        ),
    }
    for key, relative in digest_bindings.items():
        observed = _require_digest(session, key, "session", errors)
        if observed is not None and observed != digests.get(relative):
            errors.append(f"session {key} does not match {relative}")
    scenario_digest = _require_digest(
        session,
        "scenario_inventory_sha256",
        "session",
        errors,
    )
    if scenario_digest is not None:
        scenario_hasher = hashlib.sha256()
        scenario_hasher.update(SCENARIO_INVENTORY_DOMAIN)
        scenario_hasher.update(len(SCENARIO_FILES).to_bytes(4, "big"))
        for name in sorted(SCENARIO_FILES):
            relative = f"scenario/{name}".encode("utf-8")
            raw_relative = f"input/scenario/{name}"
            artifact_digest = digests.get(raw_relative)
            byte_size = sizes.get(raw_relative)
            if artifact_digest is None or byte_size is None:
                continue
            scenario_hasher.update(len(relative).to_bytes(4, "big"))
            scenario_hasher.update(relative)
            scenario_hasher.update(byte_size.to_bytes(8, "big"))
            scenario_hasher.update(bytes.fromhex(artifact_digest))
        if scenario_hasher.hexdigest() != scenario_digest:
            errors.append("session scenario_inventory_sha256 is not exact")
    for key in (
        "source_tree_sha256",
        "device_udid_sha256",
        "device_ecid_sha256",
        "device_serial_sha256",
    ):
        _require_digest(session, key, "session", errors)
    source_commit = _require_string(session, "source_commit", "session", errors)
    if source_commit is not None and GIT_COMMIT_RE.fullmatch(source_commit) is None:
        errors.append("session source_commit must be 40 lowercase git hex characters")
    for key in (
        "expected_hardware_model",
        "expected_board_config",
        "expected_os_version",
        "expected_os_build",
    ):
        _require_string(session, key, "session", errors)


def _validate_device(
    device: Any,
    session: dict[str, Any],
    label: str,
    errors: list[str],
) -> Optional[dict[str, Any]]:
    result = _exact_fields(device, DEVICE_FIELDS, label, errors)
    if result is None:
        return None
    _require_bool(result, "physical", True, label, errors)
    _require_bool(result, "simulator", False, label, errors)
    if result.get("platform") != "ios":
        errors.append(f"{label} platform must be ios")
    string_bindings = {
        "hardware_model": "expected_hardware_model",
        "board_config": "expected_board_config",
        "os_version": "expected_os_version",
        "os_build": "expected_os_build",
    }
    for key, session_key in string_bindings.items():
        observed = _require_string(result, key, label, errors)
        if observed is not None and observed != session.get(session_key):
            errors.append(f"{label} {key} must match session {session_key}")
    digest_bindings = {
        "udid_sha256": "device_udid_sha256",
        "ecid_sha256": "device_ecid_sha256",
        "serial_sha256": "device_serial_sha256",
    }
    for key, session_key in digest_bindings.items():
        observed = _require_digest(result, key, label, errors)
        if observed is not None and observed != session.get(session_key):
            errors.append(f"{label} {key} must match session {session_key}")
    for key in ("identifier_for_vendor_sha256", "boot_session_sha256"):
        _require_digest(result, key, label, errors)
    return result


def _validate_code_identity(
    code: Any, label: str, errors: list[str]
) -> Optional[dict[str, Any]]:
    result = _exact_fields(code, CODE_IDENTITY_FIELDS, label, errors)
    if result is None:
        return None
    for key in ("app_bundle_id", "app_version", "app_build", "test_bundle_id"):
        _require_string(result, key, label, errors)
    for key in ("app_executable_sha256", "test_executable_sha256"):
        _require_digest(result, key, label, errors)
    return result


def _validate_network_samples(
    samples: Any, label: str, errors: list[str]
) -> Optional[tuple[int, int]]:
    if not isinstance(samples, list) or len(samples) != len(REQUIRED_NETWORK_LABELS):
        errors.append(
            f"{label} must contain exactly {list(REQUIRED_NETWORK_LABELS)!r}"
        )
        return None
    labels: list[str] = []
    monotonic_values: list[int] = []
    for index, sample in enumerate(samples):
        sample_label = f"{label}[{index}]"
        item = _exact_fields(sample, NETWORK_SAMPLE_FIELDS, sample_label, errors)
        if item is None:
            continue
        observed_label = _require_string(item, "label", sample_label, errors)
        if observed_label is not None:
            labels.append(observed_label)
        monotonic = _require_int(
            item, "monotonic_nanos", sample_label, errors, minimum=1
        )
        if monotonic is not None:
            monotonic_values.append(monotonic)
        if item.get("status") != "unsatisfied":
            errors.append(f"{sample_label} status must be unsatisfied")
        for key in (
            "expensive",
            "constrained",
            "wifi",
            "cellular",
            "wired_ethernet",
            "loopback",
        ):
            if not isinstance(item.get(key), bool):
                errors.append(f"{sample_label} {key} must be boolean")
    if labels != list(REQUIRED_NETWORK_LABELS):
        errors.append(
            f"{label} labels must be unique and exactly ordered as "
            f"{list(REQUIRED_NETWORK_LABELS)!r}"
        )
    if len(monotonic_values) != len(REQUIRED_NETWORK_LABELS) or any(
        left >= right
        for left, right in zip(monotonic_values, monotonic_values[1:])
    ):
        errors.append(f"{label} monotonic_nanos values must strictly increase")
        return None
    return monotonic_values[0], monotonic_values[-1]


def _parse_recorded_at_utc(
    value: Any,
    label: str,
    errors: list[str],
) -> Optional[datetime]:
    if not isinstance(value, str) or re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T"
        r"[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{3})?Z",
        value,
    ) is None:
        errors.append(
            f"{label} must be canonical UTC ISO-8601 with optional milliseconds"
        )
        return None
    format_string = (
        "%Y-%m-%dT%H:%M:%S.%fZ" if "." in value else "%Y-%m-%dT%H:%M:%SZ"
    )
    try:
        parsed = datetime.strptime(value, format_string).replace(tzinfo=timezone.utc)
    except ValueError:
        errors.append(f"{label} must be a real UTC calendar timestamp")
        return None
    if (
        "." in value
        and parsed.strftime("%Y-%m-%dT%H:%M:%S.%fZ")[:-4] + "Z" != value
    ):
        errors.append(f"{label} milliseconds are not canonical")
        return None
    if "." not in value and parsed.strftime("%Y-%m-%dT%H:%M:%SZ") != value:
        errors.append(f"{label} is not canonical")
        return None
    return parsed


def _validate_launch_receipt(
    receipt: dict[str, Any],
    phase: str,
    session: dict[str, Any],
    digests: dict[str, str],
    sizes: dict[str, int],
    errors: list[str],
) -> None:
    expected_fields = (
        LAUNCH_COMMON_FIELDS
        if phase == "proof"
        else LAUNCH_COMMON_FIELDS | RESTART_ONLY_FIELDS
    )
    label = f"{phase} launch receipt"
    if _exact_fields(receipt, expected_fields, label, errors) is None:
        return
    if receipt.get("schema") != LAUNCH_RECEIPT_SCHEMA:
        errors.append(f"{label} schema must be {LAUNCH_RECEIPT_SCHEMA}")
    _require_int(receipt, "version", label, errors, exact=1)
    if receipt.get("phase") != phase:
        errors.append(f"{label} phase must be {phase}")
    _require_int(receipt, "process_id", label, errors, minimum=1)
    _require_digest(receipt, "launch_nonce_sha256", label, errors)
    _parse_recorded_at_utc(
        receipt.get("recorded_at_utc"),
        f"{label} recorded_at_utc",
        errors,
    )
    receipt_monotonic = _require_int(
        receipt, "monotonic_nanos", label, errors, minimum=1
    )
    _require_int(
        receipt,
        "resource_ceiling_bytes",
        label,
        errors,
        exact=RESOURCE_CEILING_BYTES,
    )
    _require_bool(receipt, "source_repo_dirty", False, label, errors)
    _require_bool(receipt, "app_attest_used", False, label, errors)
    if receipt.get("network_monitor") != "NWPathMonitor":
        errors.append(f"{label} network_monitor must be NWPathMonitor")
    if receipt.get("url_protocol_observed_request_count") != 0:
        errors.append(f"{label} observed network request count must be 0")
    if receipt.get("device_attestation_policy") != DEVICE_POLICY:
        errors.append(f"{label} device_attestation_policy must be {DEVICE_POLICY}")

    session_bindings = (
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "topup_finality_roster_sha256",
        "scenario_inventory_sha256",
        "native_build_manifest_sha256",
        "native_library_sha256",
        "source_commit",
        "source_tree_sha256",
        "reviewed_source_closure_descriptor_sha256",
    )
    for key in session_bindings:
        if receipt.get(key) != session.get(key):
            errors.append(f"{label} {key} must match session")
    for key in (
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "topup_finality_roster_sha256",
        "scenario_inventory_sha256",
        "native_build_manifest_sha256",
        "native_library_sha256",
        "source_tree_sha256",
        "reviewed_source_closure_descriptor_sha256",
        "install_identity_sha256",
        "checkpoint_sha256",
    ):
        _require_digest(receipt, key, label, errors)
    source_commit = receipt.get("source_commit")
    if not isinstance(source_commit, str) or GIT_COMMIT_RE.fullmatch(source_commit) is None:
        errors.append(f"{label} source_commit must be 40 lowercase git hex characters")
    if receipt.get("install_identity_sha256") != digests.get(
        "output/install-identity-v1.bin"
    ):
        errors.append(f"{label} install_identity_sha256 mismatch")
    if receipt.get("checkpoint_sha256") != digests.get("output/checkpoint-v1.norito"):
        errors.append(f"{label} checkpoint_sha256 mismatch")
    if receipt.get("checkpoint_size_bytes") != sizes.get("output/checkpoint-v1.norito"):
        errors.append(f"{label} checkpoint_size_bytes mismatch")
    _validate_device(receipt.get("device"), session, f"{label} device", errors)
    _validate_code_identity(receipt.get("code_identity"), f"{label} code_identity", errors)
    sample_window = _validate_network_samples(
        receipt.get("network_samples"), f"{label} network_samples", errors
    )
    if (
        sample_window is not None
        and receipt_monotonic is not None
        and sample_window[1] >= receipt_monotonic
    ):
        errors.append(
            f"{label} monotonic_nanos must follow every network-path sample"
        )

    if phase == "restart":
        if receipt.get("native_transcript_sha256") != digests.get(
            "output/native-transcript-v1.json"
        ):
            errors.append(f"{label} native_transcript_sha256 mismatch")
        if receipt.get("native_transcript_size_bytes") != sizes.get(
            "output/native-transcript-v1.json"
        ):
            errors.append(f"{label} native_transcript_size_bytes mismatch")
        if receipt.get("proof_launch_receipt_sha256") != digests.get(
            "output/proof-launch-receipt-v1.json"
        ):
            errors.append(f"{label} proof_launch_receipt_sha256 mismatch")


def _validate_inventory(
    transcript: dict[str, Any],
    digests: dict[str, str],
    sizes: dict[str, int],
    errors: list[str],
) -> None:
    inventory = transcript.get("artifact_inventory")
    if not isinstance(inventory, list) or len(inventory) != len(NATIVE_ARTIFACTS):
        errors.append("native transcript artifact_inventory must contain exactly 8 entries")
        return
    inventory_hasher = hashlib.sha256()
    inventory_is_canonical = True
    for index, ((expected_role, filename), raw) in enumerate(
        zip(NATIVE_ARTIFACTS, inventory)
    ):
        label = f"native transcript artifact_inventory[{index}]"
        item = _exact_fields(raw, INVENTORY_FIELDS, label, errors)
        if item is None:
            inventory_is_canonical = False
            continue
        role = item.get("role")
        if role != expected_role:
            errors.append(f"{label} role must be {expected_role}")
            inventory_is_canonical = False
        framed_size = _require_int(
            item, "framed_size_bytes", label, errors, minimum=1
        )
        payload_size = _require_int(
            item, "payload_size_bytes", label, errors, minimum=1
        )
        framed_digest = _require_digest(item, "framed_sha256", label, errors)
        payload_digest = _require_digest(item, "payload_sha256", label, errors)
        relative = f"input/artifacts/{filename}"
        if framed_size is not None and framed_size != sizes.get(relative):
            errors.append(f"{label} framed_size_bytes does not match {relative}")
        if framed_digest is not None and framed_digest != digests.get(relative):
            errors.append(f"{label} framed_sha256 does not match {relative}")
        if (
            framed_size is not None
            and payload_size is not None
            and payload_size > framed_size
        ):
            errors.append(f"{label} payload_size_bytes exceeds framed_size_bytes")
        if (
            not isinstance(role, str)
            or framed_size is None
            or payload_size is None
            or framed_digest is None
            or payload_digest is None
        ):
            inventory_is_canonical = False
        else:
            inventory_hasher.update(role.encode("utf-8"))
            inventory_hasher.update(b"\0")
            inventory_hasher.update(str(framed_size).encode("ascii"))
            inventory_hasher.update(b"\0")
            inventory_hasher.update(framed_digest.encode("ascii"))
            inventory_hasher.update(b"\0")
            inventory_hasher.update(str(payload_size).encode("ascii"))
            inventory_hasher.update(b"\0")
            inventory_hasher.update(payload_digest.encode("ascii"))
            inventory_hasher.update(b"\n")
    if (
        inventory_is_canonical
        and transcript.get("native_accepted_inventory_sha256")
        != inventory_hasher.hexdigest()
    ):
        errors.append(
            "native transcript native_accepted_inventory_sha256 is not exact"
        )


def _validate_causal_events(transcript: dict[str, Any], errors: list[str]) -> None:
    events = transcript.get("causal_events")
    if not isinstance(events, list) or len(events) != len(CAUSAL_OPERATIONS):
        errors.append("native transcript causal_events must contain exactly 28 entries")
        return
    for index, (operation, raw) in enumerate(zip(CAUSAL_OPERATIONS, events)):
        label = f"native transcript causal_events[{index}]"
        event = _exact_fields(raw, CAUSAL_EVENT_FIELDS, label, errors)
        if event is None:
            continue
        _require_int(event, "sequence", label, errors, exact=index + 1)
        expected_phase = "proof_launch" if index < 7 else "restart_launch"
        if event.get("phase") != expected_phase:
            errors.append(f"{label} phase must be {expected_phase}")
        if event.get("operation") != operation:
            errors.append(f"{label} operation must be {operation}")
        rejected = operation == "duplicate_input_rejection"
        expected_outcome = "rejected" if rejected else "succeeded"
        if event.get("outcome") != expected_outcome:
            errors.append(f"{label} outcome must be {expected_outcome}")
        _require_int(event, "duration_nanos", label, errors, minimum=1)
        _require_int(event, "output_size_bytes", label, errors, minimum=1)
        _require_digest(event, "input_sha256", label, errors)
        _require_digest(event, "output_sha256", label, errors)
        if event.get("exception_class") is not None:
            errors.append(f"{label} exception_class must be null")
        if rejected:
            if event.get("rejection_classification") != "duplicate_input":
                errors.append(f"{label} rejection_classification must be duplicate_input")
            error_digest = event.get("error_message_sha256")
            if (
                not isinstance(error_digest, str)
                or SHA256_RE.fullmatch(error_digest) is None
                or error_digest == "0" * 64
            ):
                errors.append(f"{label} error_message_sha256 must be nonzero SHA-256")
        else:
            if event.get("rejection_classification") is not None:
                errors.append(f"{label} rejection_classification must be null")
            if event.get("error_message_sha256") is not None:
                errors.append(f"{label} error_message_sha256 must be null")


def _decimal_amount(
    transcript: dict[str, Any], key: str, errors: list[str]
) -> Optional[int]:
    value = transcript.get(key)
    if not isinstance(value, str) or DECIMAL_RE.fullmatch(value) is None:
        errors.append(f"native transcript {key} must be a canonical decimal string")
        return None
    return int(value)


def _validate_transcript(
    transcript: dict[str, Any],
    session: dict[str, Any],
    proof: dict[str, Any],
    restart: dict[str, Any],
    digests: dict[str, str],
    sizes: dict[str, int],
    errors: list[str],
) -> None:
    if _exact_fields(transcript, TRANSCRIPT_FIELDS, "native transcript", errors) is None:
        return
    if transcript.get("schema") != NATIVE_TRANSCRIPT_SCHEMA:
        errors.append(f"native transcript schema must be {NATIVE_TRANSCRIPT_SCHEMA}")
    _require_int(transcript, "version", "native transcript", errors, exact=1)
    if transcript.get("platform") != "ios":
        errors.append("native transcript platform must be ios")
    for key in TRANSCRIPT_TRUE_FIELDS:
        _require_bool(transcript, key, True, "native transcript", errors)
    for key in TRANSCRIPT_FALSE_FIELDS:
        _require_bool(transcript, key, False, "native transcript", errors)
    _require_string(transcript, "generation", "native transcript", errors)
    source_commit = _require_string(
        transcript, "source_commit", "native transcript", errors
    )
    if source_commit is not None and GIT_COMMIT_RE.fullmatch(source_commit) is None:
        errors.append("native transcript source_commit must be 40 lowercase git hex")
    _require_int(
        transcript,
        "bridge_abi_version",
        "native transcript",
        errors,
        exact=BRIDGE_ABI_VERSION,
    )
    for key in TRANSCRIPT_DIGEST_FIELDS:
        _require_digest(transcript, key, "native transcript", errors)
    _require_int(transcript, "proof_process_id", "native transcript", errors, minimum=1)
    _require_int(
        transcript, "restart_process_id", "native transcript", errors, minimum=1
    )
    _require_int(
        transcript,
        "resource_ceiling_bytes",
        "native transcript",
        errors,
        exact=RESOURCE_CEILING_BYTES,
    )
    for key in ("proof_peak_rss_bytes", "restart_peak_rss_bytes"):
        rss = _require_int(transcript, key, "native transcript", errors, minimum=1)
        if rss is not None and rss > RESOURCE_CEILING_BYTES:
            errors.append(f"native transcript {key} exceeds the fixed RSS ceiling")
    for key in TRANSCRIPT_DURATION_FIELDS:
        _require_int(transcript, key, "native transcript", errors, minimum=1)
    _require_int(transcript, "proof_hops", "native transcript", errors, exact=2)
    _require_int(
        transcript, "exact_operation_count", "native transcript", errors, exact=28
    )
    _require_int(transcript, "asset_scale", "native transcript", errors, minimum=0)
    _require_int(
        transcript,
        "duplicate_error_code",
        "native transcript",
        errors,
        minimum=-311,
        exact=-311,
    )

    initial = _decimal_amount(transcript, "initial_atomic_units", errors)
    first = _decimal_amount(transcript, "first_recipient_atomic_units", errors)
    second = _decimal_amount(transcript, "second_recipient_atomic_units", errors)
    change = _decimal_amount(transcript, "sender_change_atomic_units", errors)
    redeemed = _decimal_amount(transcript, "redeemed_atomic_units", errors)
    final_unspent = _decimal_amount(transcript, "final_unspent_atomic_units", errors)
    if (
        initial is not None
        and first is not None
        and second is not None
        and change is not None
    ):
        if min(initial, first, second, change) <= 0 or initial != first + second + change:
            errors.append("native transcript amount conservation is invalid")
    if initial is not None and redeemed is not None and redeemed != initial:
        errors.append("native transcript redeemed_atomic_units must equal initial_atomic_units")
    if final_unspent is not None and final_unspent != 0:
        errors.append("native transcript final_unspent_atomic_units must be 0")

    cross_bindings = (
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "scenario_inventory_sha256",
        "source_commit",
        "source_tree_sha256",
        "reviewed_source_closure_descriptor_sha256",
    )
    for key in cross_bindings:
        if transcript.get(key) != session.get(key):
            errors.append(f"native transcript {key} must match session")
    if transcript.get("checkpoint_sha256") != digests.get("output/checkpoint-v1.norito"):
        errors.append("native transcript checkpoint_sha256 mismatch")
    if transcript.get("proof_launch_nonce_sha256") != proof.get(
        "launch_nonce_sha256"
    ):
        errors.append("native transcript proof launch nonce must match proof receipt")
    if transcript.get("restart_launch_nonce_sha256") != restart.get(
        "launch_nonce_sha256"
    ):
        errors.append("native transcript restart launch nonce must match restart receipt")
    if transcript.get("proof_process_id") != proof.get("process_id"):
        errors.append("native transcript proof_process_id must match proof receipt")
    if transcript.get("restart_process_id") != restart.get("process_id"):
        errors.append("native transcript restart_process_id must match restart receipt")
    _validate_inventory(transcript, digests, sizes, errors)
    _validate_causal_events(transcript, errors)


def _validate_native_build_manifest(
    manifest: dict[str, Any],
    session: dict[str, Any],
    digests: dict[str, str],
    errors: list[str],
) -> None:
    if _exact_fields(manifest, NATIVE_BUILD_FIELDS, "native build manifest", errors) is None:
        return
    exact_values = {
        "schema": NATIVE_BUILD_SCHEMA,
        "version": 1,
        "profile": "physical-ios-candidate-evidence-lab",
        "do_not_ship_marker": "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
        "candidate_feature_enabled": True,
        "production_capability_enabled": False,
        "bridge_abi_version": BRIDGE_ABI_VERSION,
        "target_triple": "aarch64-apple-ios",
        "architectures": ["arm64"],
        "simulator_slice_present": False,
        "minimum_ios_version": "15.0",
        "source_repo_dirty": False,
    }
    for key, expected in exact_values.items():
        if manifest.get(key) != expected:
            errors.append(f"native build manifest {key} must be {expected!r}")
    for key in (
        "candidate_record_sha256",
        "source_commit",
        "source_tree_sha256",
        "reviewed_source_closure_descriptor_sha256",
    ):
        if manifest.get(key) != session.get(key):
            errors.append(f"native build manifest {key} must match session")
    for key in (
        "iphoneos_sdk_version",
        "xcode_version",
        "cargo_version_verbose",
        "rustc_version_verbose",
    ):
        candidate = manifest.get(key)
        if (
            not isinstance(candidate, str)
            or not candidate
            or candidate != candidate.strip()
            or "\x00" in candidate
        ):
            errors.append(
                f"native build manifest {key} must be a non-empty canonical tool string"
            )
    xcode_version = manifest.get("xcode_version")
    xcode_lines = xcode_version.split("\n") if isinstance(xcode_version, str) else []
    if (
        len(xcode_lines) != 2
        or xcode_lines[0] != CANDIDATE_XCODE_VERSION
        or XCODE_BUILD_VERSION_RE.fullmatch(xcode_lines[1]) is None
    ):
        errors.append(
            "native build manifest xcode_version must be exact Xcode 26.6 with one canonical build-version line"
        )
    symbols = manifest.get("required_symbols")
    expected_symbols = [
        "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1",
        "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1",
        "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
    ]
    if symbols != expected_symbols:
        errors.append("native build manifest required_symbols are not exact")
    files = _exact_fields(
        manifest.get("files"),
        NATIVE_BUILD_FILE_FIELDS,
        "native build manifest files",
        errors,
    )
    if files is not None:
        for key in NATIVE_BUILD_FILE_FIELDS:
            candidate = files.get(key)
            if (
                not isinstance(candidate, str)
                or SHA256_RE.fullmatch(candidate) is None
                or candidate == "0" * 64
            ):
                errors.append(f"native build manifest files[{key}] must be SHA-256")
        for manifest_relative, raw_relative in NATIVE_BUILD_RAW_BINDINGS.items():
            if files.get(manifest_relative) != digests.get(raw_relative):
                errors.append(
                    "native build manifest file digest mismatch for "
                    f"{manifest_relative}"
                )


def _validate_reviewed_source_closure(
    closure: dict[str, Any],
    session: dict[str, Any],
    errors: list[str],
) -> None:
    label = "reviewed source closure"
    if (
        _exact_fields(
            closure,
            REVIEWED_SOURCE_CLOSURE_FIELDS,
            label,
            errors,
        )
        is None
    ):
        return
    if closure.get("schema") != REVIEWED_SOURCE_CLOSURE_SCHEMA:
        errors.append(f"{label} schema must be {REVIEWED_SOURCE_CLOSURE_SCHEMA}")
    for key in ("base_commit", "source_commit"):
        candidate = closure.get(key)
        if (
            not isinstance(candidate, str)
            or GIT_COMMIT_RE.fullmatch(candidate) is None
            or candidate == "0" * 40
        ):
            errors.append(f"{label} {key} must be a nonzero lowercase Git commit")
        if candidate != session.get("source_commit"):
            errors.append(f"{label} {key} must match session source_commit")
    _require_bool(closure, "source_repo_dirty", False, label, errors)
    source_tree = _require_digest(closure, "source_tree_sha256", label, errors)
    if source_tree is not None and source_tree != session.get("source_tree_sha256"):
        errors.append(f"{label} source_tree_sha256 must match session")
    tracked_digest = _require_digest(
        closure,
        "tracked_binary_diff_sha256",
        label,
        errors,
    )
    manifest_digest = _require_digest(
        closure,
        "untracked_path_mode_blob_oid_manifest_sha256",
        label,
        errors,
    )
    _require_digest(closure, "tracked_cargo_lock_sha256", label, errors)
    combined_digest = _require_digest(
        closure,
        "combined_source_fingerprint_sha256",
        label,
        errors,
    )
    count = _require_int(
        closure,
        "untracked_file_count",
        label,
        errors,
        minimum=0,
    )
    if count is not None and count != 0:
        errors.append(f"{label} untracked_file_count must be 0")
    lock_size = _require_int(
        closure,
        "tracked_cargo_lock_size_bytes",
        label,
        errors,
        minimum=1,
    )
    if lock_size is not None and lock_size > 16 * 1024 * 1024:
        errors.append(f"{label} tracked_cargo_lock_size_bytes exceeds 16 MiB")

    raw_manifest = closure.get("untracked_path_mode_blob_oid_manifest")
    if not isinstance(raw_manifest, list):
        errors.append(
            f"{label} untracked_path_mode_blob_oid_manifest must be an array"
        )
        return
    if count is not None and len(raw_manifest) != count:
        errors.append(f"{label} untracked manifest count must be exact")
    path_values: list[bytes] = []
    manifest_bytes = bytearray()
    for index, raw_entry in enumerate(raw_manifest):
        entry_label = f"{label} untracked manifest[{index}]"
        entry = _exact_fields(
            raw_entry,
            REVIEWED_SOURCE_CLOSURE_ENTRY_FIELDS,
            entry_label,
            errors,
        )
        if entry is None:
            continue
        _require_digest(entry, "blob_sha256", entry_label, errors)
        object_id = entry.get("git_blob_oid")
        if (
            not isinstance(object_id, str)
            or GIT_COMMIT_RE.fullmatch(object_id) is None
            or object_id == "0" * 40
        ):
            errors.append(f"{entry_label} git_blob_oid must be nonzero lowercase SHA-1")
        if entry.get("git_mode") not in {"100644", "100755"}:
            errors.append(f"{entry_label} git_mode must be 100644 or 100755")
        display_path = entry.get("path")
        encoded_path = entry.get("path_bytes_base64")
        decoded_path: Optional[bytes] = None
        if (
            not isinstance(display_path, str)
            or not display_path
            or not isinstance(encoded_path, str)
            or not encoded_path
        ):
            errors.append(f"{entry_label} path fields must be non-empty strings")
        else:
            try:
                decoded_path = base64.b64decode(encoded_path, validate=True)
            except (ValueError, base64.binascii.Error):
                errors.append(f"{entry_label} path_bytes_base64 must be canonical Base64")
            if decoded_path is not None:
                components = decoded_path.split(b"/")
                if (
                    not decoded_path
                    or decoded_path.startswith(b"/")
                    or decoded_path.endswith(b"/")
                    or b"\0" in decoded_path
                    or any(component in {b"", b".", b".."} for component in components)
                    or components[0] == b".git"
                    or decoded_path == b"Cargo.lock"
                ):
                    errors.append(f"{entry_label} path is unsafe")
                if base64.b64encode(decoded_path).decode("ascii") != encoded_path:
                    errors.append(
                        f"{entry_label} path_bytes_base64 is not canonical Base64"
                    )
                if os.fsdecode(decoded_path) != display_path:
                    errors.append(
                        f"{entry_label} path display and path bytes do not match"
                    )
                path_values.append(decoded_path)
        try:
            manifest_bytes.extend(canonical_json_bytes(entry))
        except EvidenceError as error:
            errors.append(f"{entry_label} is not canonical JSON: {error}")
    if path_values != sorted(set(path_values)):
        errors.append(f"{label} untracked manifest paths must be unique and sorted")
    observed_manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
    if (
        manifest_digest is not None
        and observed_manifest_digest != manifest_digest
    ):
        errors.append(f"{label} untracked manifest SHA-256 is not self-consistent")
    if tracked_digest is not None and count is not None:
        derived_dirty = tracked_digest != EMPTY_SHA256 or count != 0
        if tracked_digest != EMPTY_SHA256:
            errors.append(f"{label} tracked_binary_diff_sha256 must identify an empty diff")
        if derived_dirty:
            errors.append(
                f"{label} must have an empty tracked diff and no untracked files"
            )
    if (
        tracked_digest is not None
        and manifest_digest is not None
        and combined_digest is not None
    ):
        combined = hashlib.sha256()
        combined.update(SOURCE_DIFF_DOMAIN)
        combined.update(TRACKED_DIFF_DOMAIN)
        combined.update(bytes.fromhex(tracked_digest))
        combined.update(UNTRACKED_MANIFEST_DOMAIN)
        combined.update(bytes.fromhex(manifest_digest))
        if combined.hexdigest() != combined_digest:
            errors.append(f"{label} combined source fingerprint is not self-consistent")


def _validate_code_sign_section(
    section: Any,
    expected_fields: frozenset[str],
    label: str,
    errors: list[str],
) -> Optional[dict[str, Any]]:
    value = _exact_fields(section, expected_fields, label, errors)
    if value is None:
        return None
    for key in ("bundle_id", "identifier", "team_id", "cdhash"):
        _require_string(value, key, label, errors)
    if expected_fields == CODE_SIGN_APP_FIELDS:
        _require_string(value, "version", label, errors)
        _require_string(value, "build", label, errors)
    if value.get("identifier") != value.get("bundle_id"):
        errors.append(f"{label} identifier must equal bundle_id")
    team_id = value.get("team_id")
    if not isinstance(team_id, str) or TEAM_ID_RE.fullmatch(team_id) is None:
        errors.append(f"{label} team_id must be 10 uppercase alphanumeric characters")
    cdhash = value.get("cdhash")
    if (
        not isinstance(cdhash, str)
        or CDHASH_RE.fullmatch(cdhash) is None
        or cdhash == "0" * 40
    ):
        errors.append(f"{label} cdhash must be nonzero lowercase 40-character hex")
    for key in (
        "executable_sha256",
        "entitlements_sha256",
        "provisioning_profile_sha256",
    ):
        _require_digest(value, key, label, errors)
    return value


def _validate_code_sign_measurements(
    measurements: dict[str, Any],
    proof: dict[str, Any],
    restart: dict[str, Any],
    digests: dict[str, str],
    errors: list[str],
) -> None:
    label = "code-sign measurements"
    if (
        _exact_fields(
            measurements,
            CODE_SIGN_MEASUREMENTS_FIELDS,
            label,
            errors,
        )
        is None
    ):
        return
    if measurements.get("schema") != CODE_SIGN_MEASUREMENTS_SCHEMA:
        errors.append(f"{label} schema must be {CODE_SIGN_MEASUREMENTS_SCHEMA}")
    _require_int(measurements, "version", label, errors, exact=1)
    app = _validate_code_sign_section(
        measurements.get("app"),
        CODE_SIGN_APP_FIELDS,
        f"{label} app",
        errors,
    )
    test = _validate_code_sign_section(
        measurements.get("test"),
        CODE_SIGN_TEST_FIELDS,
        f"{label} test",
        errors,
    )
    native = _exact_fields(
        measurements.get("native"),
        CODE_SIGN_NATIVE_FIELDS,
        f"{label} native",
        errors,
    )
    if app is not None and test is not None and app.get("team_id") != test.get(
        "team_id"
    ):
        errors.append(f"{label} app and test team_id must match")
    if native is not None:
        if native.get("kind") != "static_library_bound_into_signed_test_bundle":
            errors.append(f"{label} native kind is not exact")
        native_digest = _require_digest(native, "sha256", f"{label} native", errors)
        manifest_digest = _require_digest(
            native,
            "build_manifest_sha256",
            f"{label} native",
            errors,
        )
        if native_digest is not None and native_digest != digests.get(
            "build/libNoritoBridgeCandidateLab.a"
        ):
            errors.append(f"{label} native sha256 does not match the native archive")
        if manifest_digest is not None and manifest_digest != digests.get(
            "input/native-build-manifest.json"
        ):
            errors.append(
                f"{label} native build_manifest_sha256 does not match the build manifest"
            )
        if native.get("architectures") != ["arm64"]:
            errors.append(f"{label} native architectures must be exactly ['arm64']")
        _require_bool(
            native,
            "simulator_slice_used",
            False,
            f"{label} native",
            errors,
        )

    receipt_bindings = (
        ("app_bundle_id", app, "bundle_id"),
        ("app_version", app, "version"),
        ("app_build", app, "build"),
        ("app_executable_sha256", app, "executable_sha256"),
        ("test_bundle_id", test, "bundle_id"),
        ("test_executable_sha256", test, "executable_sha256"),
    )
    for phase, receipt in (("proof", proof), ("restart", restart)):
        code_identity = receipt.get("code_identity")
        if not isinstance(code_identity, dict):
            continue
        for receipt_key, section, measurement_key in receipt_bindings:
            if section is not None and code_identity.get(receipt_key) != section.get(
                measurement_key
            ):
                errors.append(
                    f"{label} {measurement_key} must match {phase} receipt "
                    f"code_identity {receipt_key}"
                )


def _validate_test_result(
    result: dict[str, Any],
    phase: str,
    digests: dict[str, str],
    errors: list[str],
) -> None:
    label = f"{phase} test result"
    if _exact_fields(result, TEST_RESULT_FIELDS, label, errors) is None:
        return
    if result.get("schema") != TEST_RESULT_SCHEMA:
        errors.append(f"{label} schema must be {TEST_RESULT_SCHEMA}")
    _require_int(result, "version", label, errors, exact=1)
    if result.get("phase") != phase:
        errors.append(f"{label} phase must be {phase}")
    if result.get("test_status") != "passed":
        errors.append(f"{label} test_status must be passed")
    expected_identifier = (
        "KagemushaCandidateEvidenceLabTests/"
        "KagemushaCandidateEvidenceLabTests/"
        f"test{phase.capitalize()}Phase"
    )
    if result.get("test_identifier") != expected_identifier:
        errors.append(f"{label} test_identifier must be {expected_identifier}")
    receipt_relative = f"output/{phase}-launch-receipt-v1.json"
    receipt_digest = _require_digest(
        result,
        "launch_receipt_sha256",
        label,
        errors,
    )
    if receipt_digest is not None and receipt_digest != digests.get(receipt_relative):
        errors.append(f"{label} launch_receipt_sha256 does not match {receipt_relative}")
    if phase == "proof":
        if result.get("native_transcript_sha256") is not None:
            errors.append(f"{label} native_transcript_sha256 must be null")
    else:
        transcript_digest = _require_digest(
            result,
            "native_transcript_sha256",
            label,
            errors,
        )
        if transcript_digest is not None and transcript_digest != digests.get(
            "output/native-transcript-v1.json"
        ):
            errors.append(
                f"{label} native_transcript_sha256 does not match the native transcript"
            )


def validate_raw_evidence(
    snapshot: RawArtifactSnapshot,
) -> list[str]:
    """Validate the cross-launch raw evidence bundle."""

    errors: list[str] = []
    digests = snapshot.digests
    sizes = snapshot.sizes
    try:
        session = _parse_snapshot_json(
            snapshot,
            "input/session-v1.json",
            "session",
        )
        proof = _parse_snapshot_json(
            snapshot,
            "output/proof-launch-receipt-v1.json",
            "proof launch receipt",
        )
        restart = _parse_snapshot_json(
            snapshot,
            "output/restart-launch-receipt-v1.json",
            "restart launch receipt",
        )
        transcript = _parse_snapshot_json(
            snapshot,
            "output/native-transcript-v1.json",
            "native transcript",
        )
        native_build = _parse_snapshot_json(
            snapshot,
            "input/native-build-manifest.json",
            "native build manifest",
        )
        reviewed_source_closure = _parse_snapshot_json(
            snapshot,
            "input/reviewed-source-closure-v1.json",
            "reviewed source closure",
        )
        code_sign = _parse_snapshot_json(
            snapshot,
            "build/code-sign-measurements-v1.json",
            "code-sign measurements",
        )
        proof_test_result = _parse_snapshot_json(
            snapshot,
            "run/proof-test-result-v1.json",
            "proof test result",
        )
        restart_test_result = _parse_snapshot_json(
            snapshot,
            "run/restart-test-result-v1.json",
            "restart test result",
        )
    except EvidenceError as error:
        return [str(error)]

    _validate_session(session, digests, sizes, errors)
    _validate_launch_receipt(proof, "proof", session, digests, sizes, errors)
    _validate_launch_receipt(restart, "restart", session, digests, sizes, errors)
    _validate_native_build_manifest(native_build, session, digests, errors)
    _validate_reviewed_source_closure(reviewed_source_closure, session, errors)
    _validate_code_sign_measurements(
        code_sign,
        proof,
        restart,
        digests,
        errors,
    )
    _validate_test_result(proof_test_result, "proof", digests, errors)
    _validate_test_result(restart_test_result, "restart", digests, errors)
    _validate_transcript(
        transcript,
        session,
        proof,
        restart,
        digests,
        sizes,
        errors,
    )

    if proof.get("process_id") == restart.get("process_id"):
        errors.append("proof and restart process IDs must be distinct")
    if proof.get("launch_nonce_sha256") == restart.get("launch_nonce_sha256"):
        errors.append("proof and restart launch nonces must be distinct")
    if proof.get("install_identity_sha256") != restart.get(
        "install_identity_sha256"
    ):
        errors.append("proof and restart install identity must match")
    if proof.get("checkpoint_sha256") != restart.get("checkpoint_sha256"):
        errors.append("proof and restart checkpoint identity must match")
    if proof.get("device") != restart.get("device"):
        errors.append(
            "proof and restart hashed UDID/ECID/serial/boot device identity must match"
        )
    if proof.get("code_identity") != restart.get("code_identity"):
        errors.append("proof and restart code identity must match")
    proof_monotonic = proof.get("monotonic_nanos")
    restart_monotonic = restart.get("monotonic_nanos")
    if (
        not _is_int(proof_monotonic)
        or not _is_int(restart_monotonic)
        or proof_monotonic >= restart_monotonic
    ):
        errors.append(
            "proof launch monotonic_nanos must be strictly before restart launch"
        )
    restart_samples = restart.get("network_samples")
    if (
        _is_int(proof_monotonic)
        and isinstance(restart_samples, list)
        and restart_samples
        and isinstance(restart_samples[0], dict)
    ):
        restart_first_sample = restart_samples[0].get("monotonic_nanos")
        if (
            not _is_int(restart_first_sample)
            or proof_monotonic >= restart_first_sample
        ):
            errors.append(
                "proof receipt must precede the restart network-path window"
            )
    proof_recorded = _parse_recorded_at_utc(
        proof.get("recorded_at_utc"),
        "proof launch recorded_at_utc",
        errors,
    )
    restart_recorded = _parse_recorded_at_utc(
        restart.get("recorded_at_utc"),
        "restart launch recorded_at_utc",
        errors,
    )
    if (
        proof_recorded is not None
        and restart_recorded is not None
        and proof_recorded >= restart_recorded
    ):
        errors.append(
            "proof launch recorded_at_utc must be strictly before restart launch"
        )
    if sizes.get("output/install-identity-v1.bin") != 32:
        errors.append("install identity must be exactly 32 bytes")
    return errors


def _key_file_metadata(path: Path, label: str, *, private: bool) -> os.stat_result:
    try:
        value = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"{label} is missing") from error
    except OSError as error:
        raise EvidenceError(f"{label} metadata could not be read") from error
    if stat.S_ISLNK(value.st_mode):
        raise EvidenceError(f"{label} must not be a symlink")
    if not stat.S_ISREG(value.st_mode):
        raise EvidenceError(f"{label} must be a regular file")
    if value.st_nlink != 1:
        raise EvidenceError(f"{label} must have exactly one hard link")
    if value.st_uid not in {0, os.geteuid()}:
        raise EvidenceError(f"{label} must be owned by root or the current user")
    if value.st_size <= 0 or value.st_size > MAX_KEY_BYTES:
        raise EvidenceError(f"{label} size is outside its bound")
    mode = stat.S_IMODE(value.st_mode)
    if private and (value.st_uid != os.geteuid() or mode & 0o077):
        raise EvidenceError(f"{label} must be owner-private")
    if not private and (
        (value.st_uid == os.geteuid() and mode & 0o077)
        or (value.st_uid == 0 and mode & 0o022)
    ):
        raise EvidenceError(f"{label} permissions are not trusted")
    parent = path.parent
    try:
        parent_value = parent.lstat()
    except OSError as error:
        raise EvidenceError(f"{label} parent metadata could not be read") from error
    if (
        stat.S_ISLNK(parent_value.st_mode)
        or not stat.S_ISDIR(parent_value.st_mode)
        or parent_value.st_uid not in {0, os.geteuid()}
    ):
        raise EvidenceError(f"{label} parent must be a trusted real directory")
    parent_mode = stat.S_IMODE(parent_value.st_mode)
    if (
        parent_value.st_uid == os.geteuid()
        and parent_mode & 0o077
        or parent_value.st_uid == 0
        and parent_mode & 0o022
    ):
        raise EvidenceError(f"{label} parent permissions are not trusted")
    return value


def _snapshot_key_file(path: Path, label: str, *, private: bool) -> FileSnapshot:
    """Capture one key without ever passing the caller's pathname to crypto."""

    absolute = path.absolute()
    before = _key_file_metadata(absolute, label, private=private)
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(absolute, flags)
    except OSError as error:
        raise EvidenceError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        identity = _metadata_identity(before)
        if _metadata_identity(opened) != identity:
            raise EvidenceError(f"{label} changed while opening")
        chunks: list[bytes] = []
        size = 0
        while True:
            chunk = os.read(descriptor, min(16 * 1024, MAX_KEY_BYTES + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > MAX_KEY_BYTES:
                raise EvidenceError(f"{label} grew beyond its bound")
            chunks.append(chunk)
        after_open = os.fstat(descriptor)
        try:
            after_path = absolute.lstat()
        except OSError as error:
            raise EvidenceError(f"{label} disappeared while reading") from error
        if (
            _metadata_identity(after_open) != identity
            or _metadata_identity(after_path) != identity
            or size != before.st_size
        ):
            raise EvidenceError(f"{label} changed while reading")
        payload = b"".join(chunks)
        return FileSnapshot(
            path=absolute,
            identity=identity,
            payload=payload,
            sha256=hashlib.sha256(payload).hexdigest(),
            size=size,
        )
    finally:
        os.close(descriptor)


def _require_key_snapshot_unchanged(
    snapshot: FileSnapshot,
    label: str,
    *,
    private: bool,
) -> None:
    current = _snapshot_key_file(snapshot.path, label, private=private)
    if (
        current.identity != snapshot.identity
        or current.payload != snapshot.payload
        or current.sha256 != snapshot.sha256
        or current.size != snapshot.size
    ):
        raise EvidenceError(f"{label} changed after its immutable snapshot")


def _decode_canonical_pem(
    payload: bytes,
    label: str,
    header: bytes,
    footer: bytes,
) -> bytes:
    lines = payload.splitlines(keepends=True)
    if (
        len(lines) != 3
        or lines[0] != header + b"\n"
        or lines[2] != footer + b"\n"
        or not lines[1].endswith(b"\n")
    ):
        raise EvidenceError(f"{label} must use canonical single-line PEM")
    try:
        der = base64.b64decode(lines[1][:-1], validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise EvidenceError(f"{label} PEM body is not canonical Base64") from error
    canonical = header + b"\n" + base64.b64encode(der) + b"\n" + footer + b"\n"
    if payload != canonical:
        raise EvidenceError(f"{label} PEM bytes are not canonical")
    return der


def _public_key_der_from_payload(payload: bytes) -> bytes:
    der = _decode_canonical_pem(
        payload,
        "public key",
        b"-----BEGIN PUBLIC KEY-----",
        b"-----END PUBLIC KEY-----",
    )
    if len(der) != 44 or not der.startswith(ED25519_SPKI_PREFIX):
        raise EvidenceError("public key must be Ed25519")
    return der


def _private_seed_from_payload(payload: bytes) -> bytes:
    der = _decode_canonical_pem(
        payload,
        "private key",
        b"-----BEGIN PRIVATE KEY-----",
        b"-----END PRIVATE KEY-----",
    )
    if len(der) != 48 or not der.startswith(ED25519_PKCS8_SEED_PREFIX):
        raise EvidenceError("private key must be canonical Ed25519 PKCS#8")
    return der[len(ED25519_PKCS8_SEED_PREFIX) :]


_ED25519_P = 2**255 - 19
_ED25519_L = 2**252 + 27742317777372353535851937790883648493
_ED25519_D = (-121665 * pow(121666, _ED25519_P - 2, _ED25519_P)) % _ED25519_P
_ED25519_I = pow(2, (_ED25519_P - 1) // 4, _ED25519_P)
_ED25519_IDENTITY = (0, 1, 1, 0)


def _ed25519_point_add(
    left: tuple[int, int, int, int],
    right: tuple[int, int, int, int],
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _ED25519_P
    b = (y1 + x1) * (y2 + x2) % _ED25519_P
    c = 2 * _ED25519_D * t1 * t2 % _ED25519_P
    d = 2 * z1 * z2 % _ED25519_P
    e = b - a
    f = d - c
    g = d + c
    h = b + a
    return (
        e * f % _ED25519_P,
        g * h % _ED25519_P,
        f * g % _ED25519_P,
        e * h % _ED25519_P,
    )


def _ed25519_scalar_multiply(
    point: tuple[int, int, int, int],
    scalar: int,
) -> tuple[int, int, int, int]:
    result = _ED25519_IDENTITY
    addend = point
    while scalar:
        if scalar & 1:
            result = _ed25519_point_add(result, addend)
        addend = _ed25519_point_add(addend, addend)
        scalar >>= 1
    return result


def _ed25519_points_equal(
    left: tuple[int, int, int, int],
    right: tuple[int, int, int, int],
) -> bool:
    return (
        (left[0] * right[2] - right[0] * left[2]) % _ED25519_P == 0
        and (left[1] * right[2] - right[1] * left[2]) % _ED25519_P == 0
    )


def _ed25519_recover_x(y: int, sign: int) -> int:
    numerator = (y * y - 1) % _ED25519_P
    denominator = (_ED25519_D * y * y + 1) % _ED25519_P
    x_squared = numerator * pow(denominator, _ED25519_P - 2, _ED25519_P)
    x = pow(x_squared, (_ED25519_P + 3) // 8, _ED25519_P)
    if (x * x - x_squared) % _ED25519_P != 0:
        x = x * _ED25519_I % _ED25519_P
    if (x * x - x_squared) % _ED25519_P != 0:
        raise EvidenceError("Ed25519 point is not on the curve")
    if x == 0 and sign:
        raise EvidenceError("Ed25519 point encoding has a forbidden sign bit")
    if x & 1 != sign:
        x = _ED25519_P - x
    return x


_ED25519_BASE_Y = 4 * pow(5, _ED25519_P - 2, _ED25519_P) % _ED25519_P
_ED25519_BASE_X = _ed25519_recover_x(_ED25519_BASE_Y, 0)
_ED25519_BASE = (
    _ED25519_BASE_X,
    _ED25519_BASE_Y,
    1,
    _ED25519_BASE_X * _ED25519_BASE_Y % _ED25519_P,
)


def _ed25519_encode_point(point: tuple[int, int, int, int]) -> bytes:
    inverse = pow(point[2], _ED25519_P - 2, _ED25519_P)
    x = point[0] * inverse % _ED25519_P
    y = point[1] * inverse % _ED25519_P
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _ed25519_decode_point(encoded: bytes, label: str) -> tuple[int, int, int, int]:
    if len(encoded) != 32:
        raise EvidenceError(f"{label} must be exactly 32 bytes")
    encoded_integer = int.from_bytes(encoded, "little")
    sign = encoded_integer >> 255
    y = encoded_integer & ((1 << 255) - 1)
    if y >= _ED25519_P:
        raise EvidenceError(f"{label} is not a canonical Ed25519 point")
    x = _ed25519_recover_x(y, sign)
    point = (x, y, 1, x * y % _ED25519_P)
    if _ed25519_encode_point(point) != encoded:
        raise EvidenceError(f"{label} is not a canonical Ed25519 point")
    if _ed25519_points_equal(
        _ed25519_scalar_multiply(point, 8),
        _ED25519_IDENTITY,
    ) or not _ed25519_points_equal(
        _ed25519_scalar_multiply(point, _ED25519_L),
        _ED25519_IDENTITY,
    ):
        raise EvidenceError(f"{label} is not in the prime-order Ed25519 subgroup")
    return point


def _ed25519_public_from_seed(seed: bytes) -> bytes:
    if len(seed) != 32:
        raise EvidenceError("Ed25519 seed must be exactly 32 bytes")
    expanded = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(expanded[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return _ed25519_encode_point(_ed25519_scalar_multiply(_ED25519_BASE, scalar))


def _sign_ed25519_seed(seed: bytes, payload: bytes) -> bytes:
    expanded = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(expanded[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    public_key = _ed25519_encode_point(
        _ed25519_scalar_multiply(_ED25519_BASE, scalar)
    )
    nonce = int.from_bytes(
        hashlib.sha512(expanded[32:] + payload).digest(),
        "little",
    ) % _ED25519_L
    encoded_r = _ed25519_encode_point(
        _ed25519_scalar_multiply(_ED25519_BASE, nonce)
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + payload).digest(),
        "little",
    ) % _ED25519_L
    encoded_s = ((nonce + challenge * scalar) % _ED25519_L).to_bytes(
        32, "little"
    )
    return encoded_r + encoded_s


def _verify_ed25519_bytes(
    public_key: bytes,
    payload: bytes,
    signature: bytes,
) -> None:
    if len(signature) != 64:
        raise EvidenceError("Ed25519 signature must be exactly 64 bytes")
    public_point = _ed25519_decode_point(public_key, "Ed25519 public key")
    encoded_r = signature[:32]
    try:
        r_point = _ed25519_decode_point(encoded_r, "Ed25519 signature R")
    except EvidenceError as error:
        raise EvidenceError("signed evidence signature verification failed") from error
    scalar_s = int.from_bytes(signature[32:], "little")
    if scalar_s >= _ED25519_L:
        raise EvidenceError("signed evidence signature verification failed")
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + payload).digest(),
        "little",
    ) % _ED25519_L
    left = _ed25519_scalar_multiply(_ED25519_BASE, scalar_s)
    right = _ed25519_point_add(
        r_point,
        _ed25519_scalar_multiply(public_point, challenge),
    )
    if not _ed25519_points_equal(left, right):
        raise EvidenceError("signed evidence signature verification failed")


def public_key_der(public_key_path: Path) -> bytes:
    snapshot = _snapshot_key_file(public_key_path, "public key", private=False)
    der = _public_key_der_from_payload(snapshot.payload)
    _require_key_snapshot_unchanged(snapshot, "public key", private=False)
    return der


def signer_public_key_sha256(public_key_path: Path) -> str:
    return hashlib.sha256(public_key_der(public_key_path)).hexdigest()


def sign_ed25519(private_key_path: Path, payload: bytes) -> bytes:
    snapshot = _snapshot_key_file(private_key_path, "private key", private=True)
    signature = _sign_ed25519_seed(
        _private_seed_from_payload(snapshot.payload),
        payload,
    )
    _require_key_snapshot_unchanged(snapshot, "private key", private=True)
    return signature


def verify_ed25519(public_key_path: Path, payload: bytes, signature: bytes) -> None:
    snapshot = _snapshot_key_file(public_key_path, "public key", private=False)
    der = _public_key_der_from_payload(snapshot.payload)
    _verify_ed25519_bytes(der[len(ED25519_SPKI_PREFIX) :], payload, signature)
    _require_key_snapshot_unchanged(snapshot, "public key", private=False)


def _validate_key_id(key_id: str, label: str = "signer key id") -> None:
    if KEY_ID_RE.fullmatch(key_id) is None:
        raise EvidenceError(
            f"{label} must match [A-Za-z0-9][A-Za-z0-9._-]{{0,127}}"
        )


def build_signed_evidence(
    artifact_root: Path,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
) -> dict[str, Any]:
    """Validate the raw bundle, then assemble and sign its exact digest map."""

    _validate_key_id(signer_key_id)
    private_key = _snapshot_key_file(
        private_key_path,
        "private key",
        private=True,
    )
    public_key = _snapshot_key_file(
        public_key_path,
        "public key",
        private=False,
    )
    private_seed = _private_seed_from_payload(private_key.payload)
    public_der = _public_key_der_from_payload(public_key.payload)
    public_bytes = public_der[len(ED25519_SPKI_PREFIX) :]
    if _ed25519_public_from_seed(private_seed) != public_bytes:
        raise EvidenceError("private and public Ed25519 keys do not match")
    raw_snapshot = snapshot_raw_artifacts(artifact_root)
    digests = raw_snapshot.digests
    sizes = raw_snapshot.sizes
    raw_errors = validate_raw_evidence(raw_snapshot)
    if raw_errors:
        raise EvidenceError("; ".join(raw_errors))
    evidence: dict[str, Any] = {
        "schema": SIGNED_EVIDENCE_SCHEMA,
        "version": 1,
        "artifact_digests": {
            relative: {
                "size_bytes": sizes[relative],
                "sha256": digest,
            }
            for relative, digest in digests.items()
        },
        "signer_key_id": signer_key_id,
        "signer_public_key_sha256": hashlib.sha256(public_der).hexdigest(),
        "signature_algorithm": "ed25519",
    }
    payload = canonical_signature_payload(evidence)
    signature = _sign_ed25519_seed(private_seed, payload)
    _verify_ed25519_bytes(public_bytes, payload, signature)
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = signature.hex()
    _require_raw_snapshot_unchanged(raw_snapshot)
    _require_key_snapshot_unchanged(private_key, "private key", private=True)
    _require_key_snapshot_unchanged(public_key, "public key", private=False)
    return evidence


def validate_signed_evidence(
    evidence_path: Path,
    artifact_root: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
) -> list[str]:
    """Validate signature, exact raw inventory, and physical-iOS semantics."""

    errors: list[str] = []
    evidence_snapshot: Optional[FileSnapshot] = None
    trusted_key_snapshot: Optional[FileSnapshot] = None
    try:
        _validate_key_id(trusted_key_id, "trusted key id")
        try:
            evidence_absolute = evidence_path.resolve(strict=True)
            root_absolute = artifact_root.resolve(strict=True)
        except OSError as error:
            raise EvidenceError(
                "signed evidence or artifact root path could not be resolved"
            ) from error
        try:
            evidence_absolute.relative_to(root_absolute)
        except ValueError:
            pass
        else:
            errors.append("signed evidence file must stay outside artifact root")
        evidence_snapshot = _snapshot_private_file(
            evidence_absolute,
            "signed evidence",
            maximum=MAX_JSON_BYTES,
            retain_payload=True,
        )
        evidence = parse_strict_json(
            evidence_snapshot.payload,
            "signed evidence",
        )
        trusted_key_snapshot = _snapshot_key_file(
            trusted_public_key_path,
            "public key",
            private=False,
        )
        trusted_public_der = _public_key_der_from_payload(
            trusted_key_snapshot.payload
        )
    except EvidenceError as error:
        return errors + [str(error)]

    if _exact_fields(
        evidence, SIGNED_EVIDENCE_FIELDS, "signed evidence", errors
    ) is None:
        return errors
    if evidence.get("schema") != SIGNED_EVIDENCE_SCHEMA:
        errors.append(f"signed evidence schema must be {SIGNED_EVIDENCE_SCHEMA}")
    _require_int(evidence, "version", "signed evidence", errors, exact=1)
    if evidence.get("signer_key_id") != trusted_key_id:
        errors.append("signed evidence signer_key_id must match trusted CLI key id")
    if evidence.get("signature_algorithm") != "ed25519":
        errors.append("signed evidence signature_algorithm must be ed25519")
    trusted_digest = hashlib.sha256(trusted_public_der).hexdigest()
    observed_public_digest = _require_digest(
        evidence, "signer_public_key_sha256", "signed evidence", errors
    )
    if (
        trusted_digest is not None
        and observed_public_digest is not None
        and observed_public_digest != trusted_digest
    ):
        errors.append("signed evidence public key digest must match trusted public key")

    try:
        payload = canonical_signature_payload(evidence)
    except EvidenceError as error:
        errors.append(str(error))
        return errors
    expected_payload_digest = hashlib.sha256(payload).hexdigest()
    observed_payload_digest = _require_digest(
        evidence, "signature_payload_sha256", "signed evidence", errors
    )
    if (
        observed_payload_digest is not None
        and observed_payload_digest != expected_payload_digest
    ):
        errors.append("signed evidence signature_payload_sha256 mismatch")
    signature_text = evidence.get("signature")
    signature: Optional[bytes] = None
    if (
        isinstance(signature_text, str)
        and len(signature_text) == 128
        and re.fullmatch(r"[0-9a-f]{128}", signature_text) is not None
    ):
        signature = bytes.fromhex(signature_text)
    else:
        errors.append("signed evidence signature must be 64 lowercase hex bytes")

    raw_snapshot: Optional[RawArtifactSnapshot] = None
    try:
        raw_snapshot = snapshot_raw_artifacts(root_absolute)
        digests = raw_snapshot.digests
        sizes = raw_snapshot.sizes
    except EvidenceError as error:
        errors.append(str(error))
        digests, sizes = {}, {}
    artifact_digests = evidence.get("artifact_digests")
    if not isinstance(artifact_digests, dict):
        errors.append("signed evidence artifact_digests must be an object")
    else:
        if set(artifact_digests) != EXPECTED_RAW_ARTIFACT_PATHS:
            missing = sorted(EXPECTED_RAW_ARTIFACT_PATHS - set(artifact_digests))
            extra = sorted(set(artifact_digests) - EXPECTED_RAW_ARTIFACT_PATHS)
            errors.append(
                "signed evidence artifact_digests keys are not exact "
                f"(missing={missing}, extra={extra})"
            )
        expected_artifact_digests = {
            relative: {
                "size_bytes": sizes[relative],
                "sha256": digest,
            }
            for relative, digest in digests.items()
        }
        for relative, raw_binding in artifact_digests.items():
            binding = _exact_fields(
                raw_binding,
                ARTIFACT_DIGEST_FIELDS,
                f"signed evidence artifact_digests[{relative!r}]",
                errors,
            )
            if binding is None:
                continue
            digest = binding.get("sha256")
            size = binding.get("size_bytes")
            if (
                not isinstance(digest, str)
                or SHA256_RE.fullmatch(digest) is None
                or digest == "0" * 64
            ):
                errors.append(
                    f"signed evidence artifact_digests[{relative!r}].sha256 "
                    "must be nonzero SHA-256"
                )
            elif digests.get(relative) != digest:
                errors.append(f"signed evidence artifact digest mismatch for {relative}")
            if not _is_int(size) or size <= 0:
                errors.append(
                    f"signed evidence artifact_digests[{relative!r}].size_bytes "
                    "must be a positive integer"
                )
            elif sizes.get(relative) != size:
                errors.append(f"signed evidence artifact size mismatch for {relative}")
        if digests and artifact_digests != expected_artifact_digests:
            errors.append("signed evidence artifact_digests must equal the exact raw tree")

    if raw_snapshot is not None:
        errors.extend(validate_raw_evidence(raw_snapshot))
    if signature is not None and trusted_digest is not None:
        try:
            _verify_ed25519_bytes(
                trusted_public_der[len(ED25519_SPKI_PREFIX) :],
                payload,
                signature,
            )
        except EvidenceError as error:
            errors.append(str(error))
    if raw_snapshot is not None:
        try:
            _require_raw_snapshot_unchanged(raw_snapshot)
        except EvidenceError as error:
            errors.append(str(error))
    if trusted_key_snapshot is not None:
        try:
            _require_key_snapshot_unchanged(
                trusted_key_snapshot,
                "public key",
                private=False,
            )
        except EvidenceError as error:
            errors.append(str(error))
    if evidence_snapshot is not None:
        try:
            _require_private_file_snapshot_unchanged(
                evidence_snapshot,
                "signed evidence",
                maximum=MAX_JSON_BYTES,
            )
        except EvidenceError as error:
            errors.append(str(error))
    return errors


def write_private_json(path: Path, value: dict[str, Any]) -> None:
    """Atomically write owner-private signed evidence."""

    parent = path.parent.absolute()
    _validate_private_directory(parent, "signed evidence output parent")
    try:
        existing = path.lstat()
    except FileNotFoundError:
        existing = None
    except OSError as error:
        raise EvidenceError("signed evidence output metadata could not be read") from error
    if existing is not None and (
        stat.S_ISLNK(existing.st_mode)
        or not stat.S_ISREG(existing.st_mode)
        or existing.st_nlink != 1
    ):
        raise EvidenceError(
            "signed evidence output must be a singly linked regular non-symlink file"
        )
    payload = canonical_json_bytes(value)
    descriptor, temporary_text = tempfile.mkstemp(
        prefix=f".{path.name}.", dir=os.fspath(parent)
    )
    temporary = Path(temporary_text)
    try:
        os.fchmod(descriptor, 0o600)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short signed evidence write")
            offset += written
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        os.replace(temporary, path)
        path.chmod(0o600)
        directory_flags = os.O_RDONLY
        if hasattr(os, "O_DIRECTORY"):
            directory_flags |= os.O_DIRECTORY
        directory_descriptor = os.open(parent, directory_flags)
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except OSError as error:
        raise EvidenceError("signed evidence output could not be written durably") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def write_new_private_json(path: Path, value: dict[str, Any]) -> FileSnapshot:
    """Durably publish owner-private JSON without replacing any existing entry."""

    parent = path.parent.absolute()
    _validate_private_directory(parent, "JSON output parent")
    target = parent / path.name
    if path.name in {"", ".", ".."}:
        raise EvidenceError("JSON output must name one file")
    payload = canonical_json_bytes(value)
    descriptor, temporary_text = tempfile.mkstemp(
        prefix=f".{target.name}.", dir=os.fspath(parent)
    )
    temporary = Path(temporary_text)
    inode_identity: tuple[int, int] | None = None
    link_attempted = False
    linked = False
    published: FileSnapshot | None = None
    operation_error: Exception | None = None
    try:
        os.fchmod(descriptor, 0o600)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short JSON output write")
            offset += written
        os.fsync(descriptor)
        created = os.fstat(descriptor)
        inode_identity = (created.st_dev, created.st_ino)
        closing = descriptor
        descriptor = -1
        os.close(closing)
        link_attempted = True
        os.link(temporary, target, follow_symlinks=False)
        linked = True
        directory_flags = os.O_RDONLY
        if hasattr(os, "O_DIRECTORY"):
            directory_flags |= os.O_DIRECTORY
        directory_descriptor = os.open(parent, directory_flags)
        try:
            os.fsync(directory_descriptor)
            temporary.unlink()
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
        published = _snapshot_private_file(
            target,
            "published JSON output",
            maximum=MAX_JSON_BYTES,
            retain_payload=True,
        )
        if (
            inode_identity is None
            or published.identity[:2] != inode_identity
            or published.payload != payload
        ):
            raise EvidenceError("published JSON output bytes changed")
    except Exception as error:
        operation_error = error

    cleanup_error: OSError | None = None
    if descriptor >= 0:
        closing = descriptor
        descriptor = -1
        try:
            os.close(closing)
        except OSError as error:
            cleanup_error = error
    try:
        temporary.unlink()
    except FileNotFoundError:
        pass
    except OSError as error:
        if cleanup_error is None:
            cleanup_error = error

    error = operation_error if operation_error is not None else cleanup_error
    if error is not None:
        if inode_identity is not None and (
            linked or (link_attempted and not isinstance(error, FileExistsError))
        ):
            raise NewPrivateJsonPublicationUncertain(target, inode_identity) from error
        if isinstance(error, FileExistsError):
            raise EvidenceError("JSON output already exists") from error
        if isinstance(error, EvidenceError):
            raise error
        raise EvidenceError("JSON output could not be published") from error
    assert published is not None
    return published
