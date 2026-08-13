#!/usr/bin/env python3
"""Authenticate a Sumeragi v2 release candidate before running its code.

This file is a trust root, not part of the candidate trust chain.  A release
operator MUST install it outside the candidate checkout and authenticate its
bytes (and the expected digest supplied below) before starting Python.  The
bootstrap's check of its own digest is useful evidence, but cannot make an
untrusted bootstrap trustworthy.  Invoke it with the protected interpreter as
``/absolute/python3 -I -S /absolute/bootstrap_sumeragi_v2_release.py ...``;
isolated, no-site startup is enforced before any candidate data is inspected.
The external launcher must also provide a loader-clean environment and
authenticate the release-host image and dynamic libraries: those events occur
before this Python code can enforce its closed child environments.

The release-host account and every owner of an ancestor of the trusted inputs,
candidate, and evidence directory are part of the trust boundary.  This tool
rejects symlinks and revalidates bytes, modes, and inodes, but it does not claim
to withstand a malicious same-UID process or a malicious trusted ancestor that
can swap pathnames between checks.
"""

from __future__ import annotations

import argparse
import base64
import binascii
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import selectors
import shutil
import stat
import subprocess
import sys
import sysconfig
import tarfile
import threading
import time
from typing import Any, Iterable


_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SAFE_PATH_RE = re.compile(r"/[A-Za-z0-9_./+:-]+")
_RUNNER_ENV_RE = re.compile(r"[A-Z][A-Z0-9_]*")
_RUNNER_TOOL_NAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")
_FRAMEWORK_PYTHON = (
    sys.platform == "darwin"
    and isinstance(sysconfig.get_config_var("PYTHONFRAMEWORK"), str)
    and bool(sysconfig.get_config_var("PYTHONFRAMEWORK"))
)
_RELEASE_SHELL_UTILITY_NAMES = frozenset(
    {
        "awk",
        "basename",
        "cat",
        "chmod",
        "cmp",
        "cp",
        "cut",
        "diff",
        "dirname",
        "env",
        "find",
        "grep",
        "ln",
        "ls",
        "mkdir",
        "mkfifo",
        "mktemp",
        "mv",
        "openssl",
        "rm",
        "rmdir",
        "sed",
        "sh",
        "sleep",
        "tail",
        "tee",
        "tr",
        "uname",
        "wc",
        "xargs",
        "shasum" if sys.platform == "darwin" else "sha256sum",
    }
)
_RELEASE_LANGUAGE_TOOL_NAMES = frozenset(
    {
        "cargo",
        "cargo-verus",
        "git-index-pack",
        "git-upload-pack",
        "java",
        "node",
        "rustc",
        "swift",
        "tlapm",
        "verus",
    }
)
_REQUIRED_RUNNER_TOOL_NAMES = (
    _RELEASE_SHELL_UTILITY_NAMES | _RELEASE_LANGUAGE_TOOL_NAMES
)
_RECEIPT_VALIDATOR_COMPONENT_SHA256 = {
    "write_sumeragi_v2_release_receipt_corridor_log.py": (
        "bc6c901f9e011b38ba49392e99457bfd21eb365c8744c144887907229a2ee117"
    ),
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "43a815d4257ad6296a48e125dfab52c5f31aabba5210f4154641164887e48886"
    ),
}
_RUNNER_ENV_ALLOWLIST = {
    "CARGO_HOME",
    "CARGO_NET_GIT_FETCH_WITH_CLI",
    "CARGO_NET_OFFLINE",
    "IROHA_RELEASE_CANCEL_REQUEST_PATH",
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
    "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    "IROHA_RELEASE_TLA2TOOLS_JAR",
    "NIX_SSL_CERT_FILE",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SSL_CERT_FILE",
}
_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_EVIDENCE_KEYS = {
    "cargo_lock",
    "git",
    "raw_commit",
    "ssh_allowed_signers",
    "ssh_keygen",
    "ssh_revocation",
    "verify_transcript",
}
_IDENTITY_ARCHIVE_IDS = {
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}
_IDENTITY_ATTESTATION_FORMAT = "iroha-sumeragi-v2-release-identity-attestation"
_IDENTITY_TRANSCRIPT_FORMAT = "iroha-sumeragi-v2-release-identity-transcript"
_IDENTITY_PRIVATE_PROVENANCE_FORMAT = (
    "iroha-sumeragi-v2-release-identity-bootstrap-private-provenance"
)
_SSH_BEGIN = b"-----BEGIN SSH SIGNATURE-----"
_SSH_END = b"-----END SSH SIGNATURE-----"
_TRAILER_VERSION = "Sumeragi-V2-Release-Identity-Version"
_TRAILER_MANIFEST = "Sumeragi-V2-Source-Manifest-SHA256"
_TRAILER_LOCK = "Sumeragi-V2-Cargo-Lock-SHA256"
_ATTESTATION_KEYS = {
    "format",
    "schema_version",
    "candidate",
    "archives",
}
_TERMINAL_EVIDENCE_KEYS = {
    "bootstrap",
    "release_signature_attestation",
    "release_signature_transcript",
    "release_signature_raw_commit",
    "release_signature_cargo_lock",
    "release_signature_allowed_signers",
    "release_signature_revocation",
    "release_signature_git",
    "release_signature_ssh_keygen",
    "corridor_completion",
    "corridor_summary",
    "corridor_production_inventory",
    "g_unit_focused_test_inventory",
    "corridor_logs",
    "cargo_cache_input",
    "cargo_cache_input_inventory",
    "cargo_cache_final_inventory",
    "sdk_dependencies",
    "prebuilt_binary_bundle",
    "formal_completion",
    "formal_gate_log",
    "formal_proof_coverage",
    "formal_proof_evidence",
    "formal_verus_evidence",
    "formal_verus_log",
    "formal_multilane_apalache_evidence",
    "formal_cross_tool_evidence",
    "formal_production_trace_extraction_evidence",
    "formal_harness_lock",
    "formal_toolchain",
    "formal_tlaps_resource_jsonl",
    "formal_tlaps_resource_summary",
    "seed_matrix_completion",
    "seed_matrix_summary",
    "seed_matrix_run_logs",
    "seed_matrix_localnet_manifest_index",
    "seed_matrix_localnet_manifests",
    "chaos_completion",
    "chaos_log",
    "taira_completion",
    "taira_evidence",
    "taira_run_log",
    "multilane_scaling_bundle",
    "multilane_scaling_retained_validator",
    "multilane_scaling_trust_anchors",
    "g4p_multilane",
    "g12_cross_dataspace",
}
_TERMINAL_SIMPLE_ARTIFACT_KEYS = {"path", "sha256"}
_TERMINAL_FULL_ARTIFACT_KEYS = {
    "path",
    "sha256",
    "size_bytes",
    "mode",
    "owner_uid",
    "nlink",
}
_PREBUILT_BINARY_SPECS = (
    ("irohad", "release/iroha3d"),
    ("irohad_message_control", "message-control/release/iroha3d"),
    ("iroha", "release/iroha"),
    ("kagami", "release/kagami"),
)
_PREBUILT_MANIFEST_FIELDS = (
    "schema_version",
    "source_manifest_sha256",
    "cargo_lock_sha256",
    "cargo_version_sha256",
    "rustc_version_sha256",
    "host_triple",
    "target_triple",
    "profile",
    "bundle_dir",
    *(
        field
        for role, _relative in _PREBUILT_BINARY_SPECS
        for field in (
            f"{role}_relative_path",
            f"{role}_sha256",
            f"{role}_size_bytes",
            f"{role}_mode_octal",
        )
    ),
)
_SCALING_REQUIRED_TOOLING = (
    ("localnet", "scripts/deploy_localnet.sh"),
    ("load_generator", "scripts/tx_load.py"),
    ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
)
_SCALING_DIGEST_ENVIRONMENT = {
    "trial_harness_sha256": "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    "configuration_sha256": "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    "irohad_sha256": "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "iroha_cli_sha256": "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
}
_SCALING_SAFE_COMPONENT_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
_PREBUILT_INVOCATION_RE = re.compile(r"invocation\.[A-Za-z0-9]+")
_PREBUILT_TRIPLE_RE = re.compile(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_.]+)+")
_MAX_TERMINAL_ARTIFACT_BYTES = 4 * 1024 * 1024 * 1024
_MAX_SCALING_BUNDLE_FILE_COUNT = 256
_MAX_SCALING_BUNDLE_DIRECTORY_COUNT = 512
_MAX_SCALING_BUNDLE_FILE_BYTES = 256 * 1024 * 1024
_MAX_SCALING_BUNDLE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
_TRANSCRIPT_KEYS = {
    "format",
    "schema_version",
    "archive_ids",
    "candidate_commit_oid",
    "operations",
}
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_HELPER_BYTES = 16 * 1024 * 1024
_MAX_SDK_MANIFEST_BYTES = 256 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_IDENTITY_BYTES = 64 * 1024
_MAX_EVIDENCE_BYTES = 128 * 1024 * 1024
_MAX_TERMINAL_RECEIPT_BYTES = 64 * 1024 * 1024
_MAX_HELPER_OUTPUT_BYTES = 16 * 1024 * 1024
_MAX_RUNNER_TOOLS = 256
_MAX_RETAINED_RECORDS = 250_000
_MAX_RETAINED_DEPTH = 128
_MAX_RETAINED_PATH_BYTES = 4096
_MAX_RETAINED_FILE_BYTES = 4 * 1024 * 1024 * 1024
_MAX_RETAINED_TOTAL_BYTES = 64 * 1024 * 1024 * 1024
_MAX_VALIDATOR_DIAGNOSTIC_BYTES = 64 * 1024
_MAX_VALIDATOR_FAILURE_MARKER_BYTES = 64 * 1024
_DEFAULT_COMMAND_TIMEOUT_SECONDS = 600
_DIRECTORY_MODE = 0o700
_TOOL_MODE = 0o500
_DATA_MODE = 0o400
_COOPERATIVE_CANCELLED_STATUS = 125
_RECEIPT_VALIDATION_FAILED_STATUS = 74
_VALIDATOR_OPTION_ORDER = (
    "--candidate-identity",
    "--sealed-identity",
    "--release-root",
    "--bootstrap-completion",
    "--bootstrap-evidence-dir",
    "--bootstrap-identity",
    "--bootstrap-attestation",
    "--bootstrap-transcript",
    "--expected-bootstrap-completion-sha256",
    "--bootstrap-candidate-root",
    "--bootstrap-runner",
    "--signature-attestation",
    "--signature-transcript",
    "--signature-raw-commit",
    "--signature-cargo-lock",
    "--signature-allowed-signers",
    "--signature-revocation",
    "--signature-git",
    "--signature-ssh-keygen",
    "--expected-git-sha256",
    "--expected-ssh-keygen-sha256",
    "--expected-allowed-signers-sha256",
    "--expected-revocation-sha256",
    "--expected-signer-fingerprint",
    "--corridor-completion",
    "--formal-completion",
    "--seed-completion",
    "--chaos-completion",
    "--taira-completion",
    "--g4p-completion",
    "--g12-seed-completion",
    "--g12-fault-soak-completion",
    "--scaling-evidence-manifest",
    "--sdk-dependency-archive",
    "--sdk-dependency-input-inventory",
    "--sdk-dependency-final-work-inventory",
    "--expected-scaling-trial-harness-sha256",
    "--expected-scaling-configuration-sha256",
    "--expected-scaling-irohad-sha256",
    "--expected-scaling-iroha-cli-sha256",
    "--repository-root",
    "--output",
    "--verify-existing",
    "--validation-ack",
    "--source-manifest-sha256",
)
_VALIDATOR_PATH_OPTIONS = frozenset(
    {
        "--candidate-identity",
        "--sealed-identity",
        "--release-root",
        "--bootstrap-completion",
        "--bootstrap-evidence-dir",
        "--bootstrap-identity",
        "--bootstrap-attestation",
        "--bootstrap-transcript",
        "--bootstrap-candidate-root",
        "--bootstrap-runner",
        "--signature-attestation",
        "--signature-transcript",
        "--signature-raw-commit",
        "--signature-cargo-lock",
        "--signature-allowed-signers",
        "--signature-revocation",
        "--signature-git",
        "--signature-ssh-keygen",
        "--corridor-completion",
        "--formal-completion",
        "--seed-completion",
        "--chaos-completion",
        "--taira-completion",
        "--g4p-completion",
        "--g12-seed-completion",
        "--g12-fault-soak-completion",
        "--scaling-evidence-manifest",
        "--sdk-dependency-archive",
        "--sdk-dependency-input-inventory",
        "--sdk-dependency-final-work-inventory",
        "--repository-root",
        "--output",
        "--validation-ack",
    }
)
_CANCELLATION_REQUEST_BYTES = (
    b'{"reason":"operator-request","schema_version":1}\n'
)


class BootstrapError(RuntimeError):
    """A closed bootstrap prerequisite or postcondition failed."""


class RunnerLaunchError(BootstrapError):
    """The authenticated runner never acquired a child process."""


@dataclass(frozen=True)
class FileSnapshot:
    """Stable bytes and metadata for one non-symlink regular file."""

    path: Path
    data: bytes
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    size: int
    mtime_ns: int
    ctime_ns: int

    @property
    def sha256(self) -> str:
        """Return the SHA-256 digest of the captured bytes."""

        return hashlib.sha256(self.data).hexdigest()


@dataclass(frozen=True)
class DirectorySnapshot:
    """Stable identity and metadata for one private non-symlink directory."""

    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class LargeFileSnapshot:
    """Stable metadata and streaming digest for a potentially large file."""

    path: Path
    sha256: str
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    size: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class SymlinkSnapshot:
    """Stable identity and exact target for one private runner alias."""

    path: Path
    target: str
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class CommandResult:
    """Bounded command outcome."""

    returncode: int
    stdout: bytes
    stderr: bytes


def _canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _validator_invocation_value_sha256(kind: str, value: str | bool) -> str:
    payload = json.dumps(
        {"kind": kind, "value": value},
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _validate_validator_invocation(
    value: Any,
    *,
    expected_values: dict[str, tuple[str, str | bool]],
) -> None:
    """Independently recompute and authenticate a validator invocation digest."""

    if not isinstance(value, dict) or set(value) != {
        "profile",
        "operation",
        "python_flags",
        "validator",
        "ordered_options",
        "invocation_sha256",
    }:
        raise BootstrapError("receipt validator invocation binding is malformed")
    options = value["ordered_options"]
    if (
        value["profile"] != "release"
        or value["operation"] != "verify-existing-and-ack"
        or value["python_flags"] != ["-I", "-S"]
        or value["validator"] != "protected:validate-receipt.py"
        or not isinstance(options, list)
        or len(options) != len(_VALIDATOR_OPTION_ORDER)
        or not isinstance(value["invocation_sha256"], str)
        or _DIGEST_RE.fullmatch(value["invocation_sha256"]) is None
    ):
        raise BootstrapError("receipt validator invocation contract is not exact")
    if set(expected_values) != set(_VALIDATOR_OPTION_ORDER):
        raise BootstrapError(
            "receipt validator invocation reconstruction is incomplete"
        )
    for expected_name, binding in zip(_VALIDATOR_OPTION_ORDER, options):
        expected_kind = (
            "flag"
            if expected_name == "--verify-existing"
            else "path"
            if expected_name in _VALIDATOR_PATH_OPTIONS
            else "text"
        )
        if (
            not isinstance(binding, dict)
            or set(binding)
            != {"name", "value_kind", "normalized_value_sha256"}
            or binding["name"] != expected_name
            or binding["value_kind"] != expected_kind
            or not isinstance(binding["normalized_value_sha256"], str)
            or _DIGEST_RE.fullmatch(binding["normalized_value_sha256"]) is None
        ):
            raise BootstrapError(
                "receipt validator ordered option binding is not exact"
            )
        known = expected_values.get(expected_name)
        if known is None:
            raise BootstrapError(
                "receipt validator invocation reconstruction is incomplete"
            )
        kind, normalized = known
        if (
            (kind == "flag" and normalized is not True)
            or (kind in {"path", "text"} and not isinstance(normalized, str))
            or (
                kind == "path"
                and isinstance(normalized, str)
                and normalized != os.path.abspath(os.path.normpath(normalized))
            )
        ):
            raise BootstrapError(
                "receipt validator reconstructed option value is not canonical"
            )
        if (
            kind != expected_kind
            or binding["normalized_value_sha256"]
            != _validator_invocation_value_sha256(kind, normalized)
        ):
            raise BootstrapError(
                "receipt validator normalized option value is not exact"
            )
    invocation = {
        "profile": value["profile"],
        "operation": value["operation"],
        "python_flags": value["python_flags"],
        "validator": value["validator"],
        "ordered_options": options,
    }
    payload = json.dumps(
        invocation,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    if hashlib.sha256(payload).hexdigest() != value["invocation_sha256"]:
        raise BootstrapError("receipt validator invocation digest changed")


def _terminal_validator_invocation_values(
    receipt: dict[str, Any],
    *,
    evidence: Path,
    candidate: Path,
    release_runner: Path,
    receipt_path: Path,
    acknowledgment_path: Path,
    source_manifest_sha256: str,
    authenticated_environment: dict[str, str],
) -> dict[str, tuple[str, str | bool]]:
    """Reconstruct every validator value from authenticated terminal records."""

    try:
        authentication = receipt["authentication"]
        release_identity = authentication["release_identity"]
        bootstrap = authentication["bootstrap"]
        trust = release_identity["trust_policy"]
        receipt_evidence = receipt["evidence"]
        scaling_trust = receipt_evidence["multilane_scaling_trust_anchors"]
    except (KeyError, TypeError) as error:
        raise BootstrapError(
            "terminal receipt lacks validator invocation authentication"
        ) from error

    def artifact_path(*names: str) -> str:
        item: Any = receipt_evidence
        try:
            for name in names:
                item = item[name]
        except (KeyError, TypeError) as error:
            raise BootstrapError(
                "terminal receipt lacks a validator invocation artifact"
            ) from error
        if not isinstance(item, dict):
            raise BootstrapError(
                "terminal receipt validator invocation artifact is malformed"
            )
        rendered = item.get("path")
        if isinstance(rendered, str):
            return rendered
        known_archives = {
            ("bootstrap", "completion"): "BOOTSTRAP_COMPLETED.json",
            ("bootstrap", "candidate_identity"): "candidate-identity.json",
            (
                "bootstrap",
                "identity_verification",
                "identity_attestation",
            ): "identity-attestation.json",
            (
                "bootstrap",
                "identity_verification",
                "identity_transcript",
            ): "identity-transcript.json",
            ("release_signature_attestation",): "identity-attestation.json",
            ("release_signature_transcript",): "identity-transcript.json",
            ("release_signature_raw_commit",): "identity-raw-commit",
            ("release_signature_cargo_lock",): "identity-Cargo.lock",
            ("release_signature_allowed_signers",): "identity-allowed-signers",
            ("release_signature_revocation",): "identity-revocation",
            ("release_signature_git",): "identity-git",
            ("release_signature_ssh_keygen",): "identity-ssh-keygen",
        }
        archive_name = known_archives.get(names)
        if archive_name is None:
            raise BootstrapError(
                "terminal receipt validator invocation artifact is malformed"
            )
        return str(evidence / archive_name)

    scaling_bundle = receipt_evidence.get("multilane_scaling_bundle")
    scaling_files = (
        scaling_bundle.get("files") if isinstance(scaling_bundle, dict) else None
    )
    if not isinstance(scaling_files, list):
        raise BootstrapError("terminal receipt scaling inventory is malformed")
    scaling_manifest_records = [
        item
        for item in scaling_files
        if isinstance(item, dict)
        and item.get("relative_path") == "scaling_evidence.json"
    ]
    scaling_manifest_path = authenticated_environment.get(
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
    )
    if len(scaling_manifest_records) != 1 or not isinstance(
        scaling_manifest_path, str
    ):
        raise BootstrapError("terminal receipt scaling manifest path is not exact")
    source = release_runner / "source"
    return {
        "--candidate-identity": (
            "path", artifact_path("bootstrap", "candidate_identity")
        ),
        "--sealed-identity": ("path", str(release_runner / "sealed-identity.json")),
        "--release-root": ("path", str(source)),
        "--bootstrap-completion": (
            "path", artifact_path("bootstrap", "completion")
        ),
        "--bootstrap-evidence-dir": ("path", str(evidence)),
        "--bootstrap-identity": (
            "path", artifact_path("bootstrap", "candidate_identity")
        ),
        "--bootstrap-attestation": (
            "path",
            artifact_path(
                "bootstrap", "identity_verification", "identity_attestation"
            ),
        ),
        "--bootstrap-transcript": (
            "path",
            artifact_path(
                "bootstrap", "identity_verification", "identity_transcript"
            ),
        ),
        "--expected-bootstrap-completion-sha256": (
            "text", bootstrap["completion_sha256"]
        ),
        "--bootstrap-candidate-root": ("path", str(candidate)),
        "--bootstrap-runner": (
            "path", str(candidate / "scripts" / "run_sumeragi_v2_release_gates.sh")
        ),
        "--signature-attestation": (
            "path", artifact_path("release_signature_attestation")
        ),
        "--signature-transcript": (
            "path", artifact_path("release_signature_transcript")
        ),
        "--signature-raw-commit": (
            "path", artifact_path("release_signature_raw_commit")
        ),
        "--signature-cargo-lock": (
            "path", artifact_path("release_signature_cargo_lock")
        ),
        "--signature-allowed-signers": (
            "path", artifact_path("release_signature_allowed_signers")
        ),
        "--signature-revocation": (
            "path", artifact_path("release_signature_revocation")
        ),
        "--signature-git": (
            "path", artifact_path("release_signature_git")
        ),
        "--signature-ssh-keygen": (
            "path", artifact_path("release_signature_ssh_keygen")
        ),
        "--expected-git-sha256": ("text", trust["git_sha256"]),
        "--expected-ssh-keygen-sha256": ("text", trust["ssh_keygen_sha256"]),
        "--expected-allowed-signers-sha256": (
            "text", trust["allowed_signers_sha256"]
        ),
        "--expected-revocation-sha256": ("text", trust["revocation_sha256"]),
        "--expected-signer-fingerprint": ("text", trust["signer_fingerprint"]),
        "--corridor-completion": ("path", artifact_path("corridor_completion")),
        "--formal-completion": ("path", artifact_path("formal_completion")),
        "--seed-completion": ("path", artifact_path("seed_matrix_completion")),
        "--chaos-completion": ("path", artifact_path("chaos_completion")),
        "--taira-completion": ("path", artifact_path("taira_completion")),
        "--g4p-completion": (
            "path", artifact_path("g4p_multilane", "completion")
        ),
        "--g12-seed-completion": (
            "path", artifact_path("g12_cross_dataspace", "seed_completion")
        ),
        "--g12-fault-soak-completion": (
            "path", artifact_path("g12_cross_dataspace", "fault_soak_completion")
        ),
        "--scaling-evidence-manifest": ("path", scaling_manifest_path),
        "--sdk-dependency-archive": (
            "path", str(release_runner / "sdk-dependency-bundle.tar")
        ),
        "--sdk-dependency-input-inventory": (
            "path", str(release_runner / "sdk-dependency-input.json")
        ),
        "--sdk-dependency-final-work-inventory": (
            "path", str(release_runner / "sdk-dependency-work-final.json")
        ),
        "--expected-scaling-trial-harness-sha256": (
            "text", scaling_trust["trial_harness_sha256"]
        ),
        "--expected-scaling-configuration-sha256": (
            "text", scaling_trust["configuration_sha256"]
        ),
        "--expected-scaling-irohad-sha256": (
            "text", scaling_trust["irohad_sha256"]
        ),
        "--expected-scaling-iroha-cli-sha256": (
            "text", scaling_trust["iroha_cli_sha256"]
        ),
        "--repository-root": ("path", str(source)),
        "--output": ("path", str(receipt_path)),
        "--verify-existing": ("flag", True),
        "--validation-ack": ("path", str(acknowledgment_path)),
        "--source-manifest-sha256": ("text", source_manifest_sha256),
    }


def _require_digest(value: str, label: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise BootstrapError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _absolute_resolved_existing(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise BootstrapError(f"{label} must be an absolute resolved path")
    absolute = Path(os.path.abspath(path))
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if path != absolute or path != resolved:
        raise BootstrapError(f"{label} must be an absolute resolved non-symlink path")
    return path


def _inside(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _read_file(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    path = _absolute_resolved_existing(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise BootstrapError(f"{label} must be a regular non-symlink file")
    if executable and before.st_mode & 0o111 == 0:
        raise BootstrapError(f"{label} must be executable")
    if before.st_size > maximum_bytes:
        raise BootstrapError(f"{label} exceeds its closed size limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise BootstrapError(f"{label} exceeds its closed size limit")
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
        ) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            stat.S_IMODE(opened.st_mode),
        ):
            raise BootstrapError(f"{label} changed while it was read")
        return FileSnapshot(
            path,
            b"".join(chunks),
            opened.st_dev,
            opened.st_ino,
            stat.S_IMODE(opened.st_mode),
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _read_file_at(
    parent_fd: int,
    name: str,
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
) -> FileSnapshot:
    """Read one bounded regular file relative to a held parent directory."""

    if name in {"", ".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError(f"{label} has an unsafe leaf name")
    try:
        before = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_size > maximum_bytes
    ):
        raise BootstrapError(f"{label} is not one bounded regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, dir_fd=parent_fd)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        stable = (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_size", "st_mtime_ns", "st_ctime_ns",
        )
        if not stat.S_ISREG(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in stable
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while True:
            block = os.read(
                descriptor, min(1024 * 1024, maximum_bytes + 1 - total)
            )
            if not block:
                break
            chunks.append(block)
            total += len(block)
            if total > maximum_bytes:
                raise BootstrapError(f"{label} exceeds its closed size limit")
        after = os.fstat(descriptor)
        current = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        if total != opened.st_size or any(
            getattr(after, field) != getattr(opened, field)
            or getattr(current, field) != getattr(opened, field)
            for field in stable
        ):
            raise BootstrapError(f"{label} changed while it was read")
        return FileSnapshot(
            path,
            b"".join(chunks),
            opened.st_dev,
            opened.st_ino,
            stat.S_IMODE(opened.st_mode),
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_unchanged(
    snapshot: FileSnapshot,
    label: str,
    *,
    maximum_bytes: int,
    executable: bool = False,
) -> None:
    current = _read_file(
        snapshot.path,
        label,
        maximum_bytes=maximum_bytes,
        executable=executable,
    )
    if current != snapshot:
        raise BootstrapError(f"{label} changed during the release bootstrap")


def _protected_snapshot(
    path: Path,
    expected_digest: str,
    label: str,
    *,
    candidate: Path,
    maximum_bytes: int,
    executable: bool = False,
) -> FileSnapshot:
    snapshot = _read_file(
        path, label, maximum_bytes=maximum_bytes, executable=executable
    )
    if _inside(snapshot.path, candidate):
        raise BootstrapError(f"{label} must be installed outside the candidate root")
    if snapshot.sha256 != _require_digest(expected_digest, f"expected {label} digest"):
        raise BootstrapError(f"{label} does not match its protected SHA-256")
    return snapshot


def _prepare_evidence_directory(path: Path, candidate: Path) -> tuple[Path, int]:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise BootstrapError("evidence directory must be an absolute normalized path")
    if path.exists() or path.is_symlink():
        raise BootstrapError("evidence directory already exists; overwrite is forbidden")
    parent = _absolute_resolved_existing(path.parent, "evidence-directory parent")
    parent_stat = parent.lstat()
    if (
        not stat.S_ISDIR(parent_stat.st_mode)
        or parent_stat.st_uid != os.getuid()
        or stat.S_IMODE(parent_stat.st_mode) != _DIRECTORY_MODE
    ):
        raise BootstrapError(
            "evidence-directory parent must be owner-owned with exact mode 0700"
        )
    path = parent / path.name
    if _SAFE_PATH_RE.fullmatch(str(path)) is None or os.pathsep in str(path):
        raise BootstrapError("evidence directory must use the shell-safe release path alphabet")
    if _inside(path, candidate):
        raise BootstrapError("evidence directory must be outside the candidate root")
    created = False
    try:
        os.mkdir(path, _DIRECTORY_MODE)
        created = True
        os.chmod(path, _DIRECTORY_MODE, follow_symlinks=False)
        parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            parent_flags |= os.O_NOFOLLOW
        parent_fd = os.open(parent, parent_flags)
        try:
            os.fsync(parent_fd)
        finally:
            os.close(parent_fd)
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(path, flags)
    except OSError as error:
        if created:
            try:
                os.rmdir(path)
            except OSError:
                pass
        raise BootstrapError("private evidence directory could not be created") from error
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(opened.st_mode)
        or stat.S_IMODE(opened.st_mode) != _DIRECTORY_MODE
        or opened.st_uid != os.getuid()
    ):
        os.close(descriptor)
        try:
            os.rmdir(path)
        except OSError:
            pass
        raise BootstrapError("evidence directory must be owner-owned with exact mode 0700")
    return path, descriptor


def _write_artifact(
    directory: Path,
    directory_fd: int,
    name: str,
    data: bytes,
    mode: int,
) -> FileSnapshot:
    if not name or name in {".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError("invalid bootstrap evidence name")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory_fd)
        try:
            os.fchmod(descriptor, mode)
            view = memoryview(data)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    raise BootstrapError(f"short write for bootstrap evidence {name}")
                view = view[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.fsync(directory_fd)
    except OSError as error:
        raise BootstrapError(f"could not publish bootstrap evidence {name}") from error
    return _read_file(
        directory / name,
        f"bootstrap evidence {name}",
        maximum_bytes=max(len(data), 1),
        executable=mode == _TOOL_MODE,
    )


def _publish_completion_marker(
    directory: Path,
    directory_fd: int,
    data: bytes,
    *,
    final_name: str = "BOOTSTRAP_COMPLETED.json",
) -> FileSnapshot:
    if (
        not final_name
        or final_name in {".", ".."}
        or "/" in final_name
        or "\0" in final_name
    ):
        raise BootstrapError("invalid bootstrap completion marker name")
    temporary_name = f".{final_name}.stage.{secrets.token_hex(16)}"
    staged: FileSnapshot | None = None
    completed = False

    def unlink_owned(name: str) -> None:
        if staged is None:
            return
        try:
            metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except OSError:
            return
        if (
            stat.S_ISREG(metadata.st_mode)
            and (metadata.st_dev, metadata.st_ino) == (staged.device, staged.inode)
        ):
            try:
                os.unlink(name, dir_fd=directory_fd)
            except OSError:
                pass

    try:
        staged = _write_artifact(
            directory,
            directory_fd,
            temporary_name,
            data,
            _DATA_MODE,
        )
        os.link(
            temporary_name,
            final_name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        os.fsync(directory_fd)
        marker = _read_file(
            directory / final_name,
            "bootstrap completion marker",
            maximum_bytes=max(len(data), 1),
        )
        if (
            marker.device,
            marker.inode,
            marker.mode,
            marker.owner,
            marker.nlink,
            marker.data,
        ) != (
            staged.device,
            staged.inode,
            staged.mode,
            os.getuid(),
            2,
            staged.data,
        ):
            raise BootstrapError("bootstrap completion marker changed at publication")
        os.unlink(temporary_name, dir_fd=directory_fd)
        os.fsync(directory_fd)
        published = _read_file(
            directory / final_name,
            "bootstrap completion marker",
            maximum_bytes=max(len(data), 1),
        )
        if (
            published.device,
            published.inode,
            published.mode,
            published.owner,
            published.nlink,
            published.data,
        ) != (
            marker.device,
            marker.inode,
            marker.mode,
            os.getuid(),
            1,
            marker.data,
        ):
            raise BootstrapError("bootstrap completion marker changed after publication")
        completed = True
        return published
    except OSError as error:
        raise BootstrapError("bootstrap completion marker could not be published") from error
    finally:
        if staged is not None and not completed:
            unlink_owned(final_name)
            unlink_owned(temporary_name)
            try:
                os.fsync(directory_fd)
            except OSError:
                pass


def _run_bounded(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    maximum_output_bytes: int,
) -> CommandResult:
    argv = [str(executable), *arguments]
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise BootstrapError(f"could not execute protected command {executable}") from error
    assert process.stdout is not None and process.stderr is not None
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    # Bounds determine the eventual verdict; they never control the child.
    # Retain only the capped prefix while draining both streams through EOF.
    retained_output_bytes = 0
    output_limit_exceeded = False
    runtime_limit_exceeded = False
    drain_errors: list[BaseException] = []
    output_lock = threading.Lock()

    def retain(label: str, chunk: bytes) -> None:
        nonlocal retained_output_bytes, output_limit_exceeded
        with output_lock:
            retained_capacity = max(
                maximum_output_bytes - retained_output_bytes, 0
            )
            retained = chunk[:retained_capacity]
            buffers[label].extend(retained)
            retained_output_bytes += len(retained)
            if len(retained) != len(chunk):
                output_limit_exceeded = True

    def drain(label: str, stream: Any) -> None:
        try:
            while True:
                chunk = stream.read(64 * 1024)
                if not chunk:
                    return
                retain(label, chunk)
        except BaseException as error:
            drain_errors.append(error)

    drain_specs = (
        ("stdout", process.stdout),
        ("stderr", process.stderr),
    )
    drain_threads = [
        threading.Thread(
            target=drain,
            args=(label, stream),
            name=f"bootstrap-{label}-drain",
        )
        for label, stream in drain_specs
    ]
    started_threads: list[threading.Thread] = []
    supervision_error: BaseException | None = None
    deadline = time.monotonic() + timeout_seconds
    try:
        for thread in drain_threads:
            thread.start()
            started_threads.append(thread)
        while process.poll() is None:
            if time.monotonic() > deadline:
                runtime_limit_exceeded = True
            time.sleep(0.05)
    except BaseException as error:
        supervision_error = error
    finally:
        missing_specs = drain_specs[len(started_threads) :]
        if missing_specs:
            # Thread creation failure is itself only an observer failure. Keep
            # every still-unowned pipe open and drain it in this thread so the
            # child remains free to reach natural completion.
            fallback = selectors.DefaultSelector()
            try:
                for label, stream in missing_specs:
                    os.set_blocking(stream.fileno(), False)
                    fallback.register(stream, selectors.EVENT_READ, label)
                while fallback.get_map():
                    for key, _ in fallback.select(1.0):
                        try:
                            chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                        except BlockingIOError:
                            continue
                        if chunk:
                            retain(key.data, chunk)
                        else:
                            fallback.unregister(key.fileobj)
            except BaseException as error:
                drain_errors.append(error)
            finally:
                fallback.close()
        try:
            # This is deliberately unbounded: neither a latched policy
            # violation nor an observer exception authorizes child control.
            returncode = process.wait()
        finally:
            for thread in started_threads:
                thread.join()
            process.stdout.close()
            process.stderr.close()
    if supervision_error is not None:
        raise supervision_error
    if time.monotonic() > deadline:
        runtime_limit_exceeded = True
    if len(started_threads) != len(drain_threads):
        raise BootstrapError("protected command output drain could not start")
    if drain_errors:
        raise BootstrapError("protected command output drain failed") from drain_errors[0]
    if runtime_limit_exceeded:
        raise BootstrapError("protected command exceeded its bounded runtime")
    if output_limit_exceeded:
        raise BootstrapError("protected command exceeded its bounded output limit")
    return CommandResult(
        returncode, bytes(buffers["stdout"]), bytes(buffers["stderr"])
    )


def _run_release_runner(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    stdout_descriptor: int,
    stderr_descriptor: int,
) -> CommandResult:
    """Run the release runner with private regular-file diagnostic sinks.

    The runner owns Cargo, rustc, validator, formal, chaos, and soak processes.
    Their in-scope operations have their own protocol and harness deadlines;
    direct regular-file descriptors avoid relay backpressure. The bootstrap
    leaves cancellation to the runner's cooperative gate boundaries and waits
    for the in-flight runner to finish naturally.
    """

    argv = [str(executable), *arguments]
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout_descriptor,
            stderr=stderr_descriptor,
            close_fds=True,
        )
    except OSError as error:
        raise RunnerLaunchError(
            f"could not execute protected command {executable}"
        ) from error
    returncode = process.wait()
    return CommandResult(returncode, b"", b"")


def _open_runner_log(directory_fd: int, name: str) -> int:
    """Create one owner-only, no-clobber regular file for runner output."""

    if name in {"", ".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError("runner log name is invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, 0o600, dir_fd=directory_fd)
    except OSError as error:
        raise BootstrapError(f"could not create private runner log {name}") from error
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != 0o600
    ):
        os.close(descriptor)
        raise BootstrapError(f"private runner log {name} has unsafe metadata")
    os.fsync(directory_fd)
    return descriptor


def _capture_large_file(path: Path, label: str) -> LargeFileSnapshot:
    """Hash one stable regular file without retaining its contents in memory."""

    path = _absolute_resolved_existing(path, label)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise BootstrapError(f"{label} must be a regular non-symlink file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or opened.st_mode != before.st_mode
            or opened.st_uid != before.st_uid
            or opened.st_nlink != before.st_nlink
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            digest.update(chunk)
        after = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(getattr(after, field) != getattr(opened, field) for field in fields):
            raise BootstrapError(f"{label} changed while it was hashed")
        if size != opened.st_size:
            raise BootstrapError(f"{label} has inconsistent size metadata")
        return LargeFileSnapshot(
            path=path,
            sha256=digest.hexdigest(),
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            size=opened.st_size,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _capture_large_file_at(
    parent_fd: int,
    name: str,
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
) -> LargeFileSnapshot:
    """Hash one bounded regular file relative to a held directory."""

    if name in {"", ".", ".."} or "/" in name or "\0" in name:
        raise BootstrapError(f"{label} has an unsafe leaf name")
    try:
        before = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_size > maximum_bytes
    ):
        raise BootstrapError(f"{label} is not one bounded regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, dir_fd=parent_fd)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        stable = (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_size", "st_mtime_ns", "st_ctime_ns",
        )
        if not stat.S_ISREG(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in stable
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        digest = hashlib.sha256()
        total = 0
        while True:
            block = os.read(descriptor, 1024 * 1024)
            if not block:
                break
            total += len(block)
            if total > maximum_bytes:
                raise BootstrapError(f"{label} exceeds its closed size limit")
            digest.update(block)
        after = os.fstat(descriptor)
        current = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        if total != opened.st_size or any(
            getattr(after, field) != getattr(opened, field)
            or getattr(current, field) != getattr(opened, field)
            for field in stable
        ):
            raise BootstrapError(f"{label} changed while it was hashed")
        return LargeFileSnapshot(
            path=path,
            sha256=digest.hexdigest(),
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            size=opened.st_size,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_large_file_unchanged(
    snapshot: LargeFileSnapshot, label: str
) -> None:
    if _capture_large_file(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed after it was sealed")


def _seal_runner_log(
    descriptor: int, path: Path, label: str
) -> LargeFileSnapshot:
    """Flush, make immutable-by-mode, and snapshot a completed runner log."""

    os.fsync(descriptor)
    os.fchmod(descriptor, _DATA_MODE)
    os.fsync(descriptor)
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != _DATA_MODE
    ):
        raise BootstrapError(f"{label} has unsafe final metadata")
    return _capture_large_file(path, label)


def _closed_environment(
    evidence: Path,
    extra_path: list[Path],
    extra_values: dict[str, str] | None = None,
) -> dict[str, str]:
    path_entries: list[str] = [str(evidence)]
    for entry in extra_path:
        rendered = str(entry)
        if rendered not in path_entries:
            path_entries.append(rendered)
    environment = {
        "HOME": str(evidence / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join(path_entries),
        "TMPDIR": str(evidence / "tmp"),
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": "core.hooksPath",
        "GIT_CONFIG_VALUE_0": os.devnull,
        "GIT_CONFIG_KEY_1": "core.fsmonitor",
        "GIT_CONFIG_VALUE_1": "false",
        "GIT_TERMINAL_PROMPT": "0",
    }
    if extra_values:
        environment.update(extra_values)
    return environment


def _require_command_resolution(
    name: str,
    expected: Path,
    environment: dict[str, str],
    label: str,
) -> None:
    discovered = shutil.which(name, path=environment["PATH"])
    if discovered is None:
        raise BootstrapError(f"closed PATH does not expose protected {label}")
    try:
        resolved = Path(discovered).resolve(strict=True)
    except OSError as error:
        raise BootstrapError(f"closed PATH has an invalid {label} alias") from error
    if resolved != expected:
        raise BootstrapError(f"closed PATH resolves {name} to an unprotected executable")


def _load_identity(data: bytes) -> dict[str, Any]:
    if len(data) > _MAX_IDENTITY_BYTES:
        raise BootstrapError("candidate identity exceeds its closed size limit")
    try:
        value = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootstrapError("trusted manifest helper returned invalid identity JSON") from error
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise BootstrapError("trusted manifest helper returned the wrong identity schema")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise BootstrapError("candidate identity must use first-release schema 1")
    for key in ("head_commit", "head_tree", "index_tree"):
        if not isinstance(value[key], str) or _OBJECT_ID_RE.fullmatch(value[key]) is None:
            raise BootstrapError(f"candidate identity has invalid {key}")
    for key in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        if not isinstance(value[key], str) or _DIGEST_RE.fullmatch(value[key]) is None:
            raise BootstrapError(f"candidate identity has invalid {key}")
    canonical = _canonical_json(value)
    if data != canonical:
        raise BootstrapError("candidate identity is not canonical JSON")
    return value


def _compute_identity(
    python: Path,
    helper: Path,
    candidate: Path,
    environment: dict[str, str],
    timeout_seconds: int,
) -> tuple[bytes, dict[str, Any]]:
    result = _run_bounded(
        python,
        [
            "-I",
            "-S",
            str(helper),
            "--root",
            str(candidate),
            "--release-identity-json",
        ],
        cwd=candidate,
        environment=environment,
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(f"trusted manifest helper rejected candidate: {detail}")
    if result.stderr:
        raise BootstrapError("trusted manifest helper emitted unexpected stderr")
    return result.stdout, _load_identity(result.stdout)


def _parse_canonical_json(snapshot: FileSnapshot, label: str) -> dict[str, Any]:
    try:
        value = json.loads(snapshot.data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootstrapError(f"{label} is not valid JSON") from error
    if not isinstance(value, dict) or snapshot.data != _canonical_json(value):
        raise BootstrapError(f"{label} must be one canonical JSON object")
    return value


def _validate_allowed_signers_policy(data: bytes) -> None:
    try:
        text_value = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("SSH allowed-signers policy must be UTF-8 text") from error
    if "\r" in text_value or "\0" in text_value or not text_value.endswith("\n"):
        raise BootstrapError("SSH allowed-signers policy must be LF-only text")
    active = [
        line
        for line in text_value.splitlines()
        if line and not line.startswith("#")
    ]
    if len(active) != 1:
        raise BootstrapError(
            "SSH allowed-signers file must contain exactly one active key"
        )
    folded = active[0].casefold()
    if "cert-authority" in folded or "-cert-v01@openssh.com" in folded:
        raise BootstrapError(
            "SSH certificate-authority and certificate keys are not accepted in v1"
        )
    if "valid-after=" in folded or "valid-before=" in folded:
        raise BootstrapError(
            "time-bounded SSH allowed-signers policies are not accepted in v1"
        )


def _require_exact_json_fields(
    value: Any, expected: set[str], label: str
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise BootstrapError(f"{label} has the wrong schema")
    return value


def _private_directory_snapshot(path: Path, label: str) -> DirectorySnapshot:
    path = _absolute_resolved_existing(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
        or stat.S_IMODE(before.st_mode) != _DIRECTORY_MODE
        or before.st_uid != os.getuid()
    ):
        raise BootstrapError(f"{label} must be owner-owned with exact mode 0700")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        return DirectorySnapshot(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_directory_unchanged(snapshot: DirectorySnapshot, label: str) -> None:
    if _private_directory_snapshot(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed during terminal receipt validation")


def _sealed_directory_snapshot(path: Path, label: str) -> DirectorySnapshot:
    path = _absolute_resolved_existing(path, label)
    before = path.lstat()
    mode = stat.S_IMODE(before.st_mode)
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
        or before.st_uid != os.getuid()
        or mode & 0o222
    ):
        raise BootstrapError(
            f"{label} must be an owner-owned, non-writable sealed directory"
        )
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        return DirectorySnapshot(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_sealed_directory_unchanged(
    snapshot: DirectorySnapshot, label: str
) -> None:
    if _sealed_directory_snapshot(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed after sealed-source validation")


def _fsync_sealed_tree(root: Path) -> None:
    """Synchronize retained sealed source files and directories bottom-up."""

    root = _absolute_resolved_existing(root, "retained sealed source")
    directories: list[Path] = []
    for current_text, names, files in os.walk(root, topdown=True, followlinks=False):
        current = Path(current_text)
        directories.append(current)
        for name in [*names, *files]:
            path = current / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode):
                continue
            if stat.S_ISDIR(metadata.st_mode):
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise BootstrapError("retained sealed source contains a special file")
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(path, flags)
            try:
                opened = os.fstat(descriptor)
                if (
                    not stat.S_ISREG(opened.st_mode)
                    or (opened.st_dev, opened.st_ino)
                    != (metadata.st_dev, metadata.st_ino)
                    or opened.st_mode != metadata.st_mode
                    or opened.st_uid != metadata.st_uid
                    or opened.st_size != metadata.st_size
                ):
                    raise BootstrapError(
                        "retained sealed source changed while opened for fsync"
                    )
                os.fsync(descriptor)
                after = os.fstat(descriptor)
                if after != opened:
                    raise BootstrapError(
                        "retained sealed source changed while it was synchronized"
                    )
            finally:
                os.close(descriptor)
    for directory in sorted(
        directories, key=lambda item: (-len(item.parts), str(item))
    ):
        flags = (
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(directory, flags)
        try:
            before = os.fstat(descriptor)
            os.fsync(descriptor)
            if os.fstat(descriptor) != before:
                raise BootstrapError(
                    "retained sealed directory changed while it was synchronized"
                )
        finally:
            os.close(descriptor)


def _terminal_directory_snapshot(path: Path, label: str) -> DirectorySnapshot:
    """Capture one resolved terminal-evidence directory without mode assumptions."""

    path = _absolute_resolved_existing(path, label)
    try:
        before = path.lstat()
    except OSError as error:
        raise BootstrapError(f"{label} is unavailable") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
        or before.st_uid != os.getuid()
    ):
        raise BootstrapError(f"{label} must be an owner-owned non-symlink directory")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BootstrapError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise BootstrapError(f"{label} changed while it was opened")
        return DirectorySnapshot(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_terminal_directory_unchanged(
    snapshot: DirectorySnapshot, label: str
) -> None:
    if _terminal_directory_snapshot(snapshot.path, label) != snapshot:
        raise BootstrapError(f"{label} changed during protected receipt validation")


def _terminal_mode(value: Any, label: str) -> int:
    if not isinstance(value, str) or re.fullmatch(r"[0-7]{4}", value) is None:
        raise BootstrapError(f"{label} mode is not canonical")
    return int(value, 8)


def _terminal_relative_path(value: Any, label: str) -> tuple[str, ...]:
    if not isinstance(value, str) or not value or value.startswith("/"):
        raise BootstrapError(f"{label} is not a safe relative path")
    parts = tuple(value.split("/"))
    if (
        "/".join(parts) != value
        or any(
            part in {"", ".", ".."}
            or _SCALING_SAFE_COMPONENT_RE.fullmatch(part) is None
            for part in parts
        )
    ):
        raise BootstrapError(f"{label} is not a safe relative path")
    return parts


def _validate_terminal_release_evidence(
    *,
    receipt_evidence: dict[str, Any],
    evidence: Path,
    release_root: Path,
    receipt_identity: dict[str, Any],
    runner_record: dict[str, Any],
    authenticated_environment: dict[str, str],
) -> tuple[list[LargeFileSnapshot], list[DirectorySnapshot]]:
    """Validate and freeze every newly protected terminal-evidence input."""

    artifact_snapshots: list[LargeFileSnapshot] = []
    directory_snapshots: list[DirectorySnapshot] = []
    artifact_paths: dict[Path, str] = {}
    artifact_inodes: dict[tuple[int, int], str] = {}
    directories: dict[Path, DirectorySnapshot] = {}
    directory_inodes: dict[tuple[int, int], str] = {}
    if hashlib.sha256(_canonical_json(authenticated_environment)).hexdigest() != runner_record.get(
        "environment_sha256"
    ):
        raise BootstrapError("terminal runner environment digest is not exact")

    def capture_directory(
        path: Path,
        label: str,
        *,
        containment_root: Path = evidence,
        expected_mode: int | None = None,
    ) -> DirectorySnapshot:
        existing = directories.get(path)
        if existing is not None:
            if expected_mode is not None and existing.mode != expected_mode:
                raise BootstrapError(f"{label} has the wrong mode")
            return existing
        snapshot = _terminal_directory_snapshot(path, label)
        if not _inside(snapshot.path, containment_root):
            raise BootstrapError(f"{label} escaped its authenticated containment root")
        if expected_mode is not None and snapshot.mode != expected_mode:
            raise BootstrapError(f"{label} has the wrong mode")
        inode = (snapshot.device, snapshot.inode)
        alias = directory_inodes.get(inode)
        if alias is not None:
            raise BootstrapError(f"terminal evidence directories alias: {alias} and {label}")
        directory_inodes[inode] = label
        directories[snapshot.path] = snapshot
        directory_snapshots.append(snapshot)
        return snapshot

    def capture_artifact(
        record: Any,
        label: str,
        *,
        full: bool,
        extra_fields: frozenset[str] = frozenset(),
        expected_path: Path | None = None,
        maximum_bytes: int = _MAX_TERMINAL_ARTIFACT_BYTES,
        expected_mode: int | None = None,
        containment_root: Path = evidence,
    ) -> LargeFileSnapshot:
        expected_fields = (
            _TERMINAL_FULL_ARTIFACT_KEYS
            if full
            else _TERMINAL_SIMPLE_ARTIFACT_KEYS
        ) | set(extra_fields)
        record = _require_exact_json_fields(record, expected_fields, label)
        rendered = record["path"]
        digest = record["sha256"]
        if not isinstance(rendered, str) or not isinstance(digest, str):
            raise BootstrapError(f"{label} path or digest is not text")
        _require_digest(digest, f"{label} digest")
        path = _absolute_resolved_existing(Path(rendered), label)
        if not _inside(path, containment_root):
            raise BootstrapError(f"{label} escaped its authenticated containment root")
        if expected_path is not None and path != expected_path:
            raise BootstrapError(f"{label} has the wrong contained path")
        alias = artifact_paths.get(path)
        if alias is not None:
            raise BootstrapError(f"terminal evidence path is multiply carried: {alias} and {label}")
        snapshot = _capture_large_file(path, label)
        if snapshot.size > maximum_bytes:
            raise BootstrapError(f"{label} exceeds its closed size limit")
        if snapshot.sha256 != digest:
            raise BootstrapError(f"{label} changed: digest does not match its bytes")
        if snapshot.owner != os.getuid() or snapshot.nlink != 1:
            raise BootstrapError(f"{label} must be owner-owned and single-link")
        if expected_mode is not None and snapshot.mode != expected_mode:
            raise BootstrapError(f"{label} has the wrong mode")
        if full:
            size = record["size_bytes"]
            owner = record["owner_uid"]
            nlink = record["nlink"]
            if (
                type(size) is not int
                or size < 0
                or type(owner) is not int
                or owner < 0
                or type(nlink) is not int
                or nlink < 1
                or size != snapshot.size
                or _terminal_mode(record["mode"], label) != snapshot.mode
                or owner != snapshot.owner
                or nlink != snapshot.nlink
            ):
                raise BootstrapError(f"{label} metadata does not match its file")
        inode = (snapshot.device, snapshot.inode)
        inode_alias = artifact_inodes.get(inode)
        if inode_alias is not None:
            raise BootstrapError(
                f"terminal evidence files are inode aliases: {inode_alias} and {label}"
            )
        artifact_paths[path] = label
        artifact_inodes[inode] = label
        artifact_snapshots.append(snapshot)
        capture_directory(
            snapshot.path.parent,
            f"{label} parent directory",
            containment_root=containment_root,
        )
        return snapshot

    def capture_archive(
        record: Any,
        label: str,
        *,
        archive_id: str,
        expected_path: Path,
        maximum_bytes: int = _MAX_TERMINAL_ARTIFACT_BYTES,
        expected_mode: int | None = None,
        containment_root: Path = evidence,
        extra_fields: frozenset[str] = frozenset(),
    ) -> LargeFileSnapshot:
        record = _require_exact_json_fields(
            record,
            {"archive_id", "mode", "sha256", "size_bytes"} | set(extra_fields),
            label,
        )
        if record["archive_id"] != archive_id:
            raise BootstrapError(f"{label} has the wrong archive id")
        snapshot = _capture_large_file(expected_path, label)
        if (
            not _inside(snapshot.path, containment_root)
            or snapshot.size > maximum_bytes
            or snapshot.owner != os.getuid()
            or snapshot.nlink != 1
            or record["sha256"] != snapshot.sha256
            or type(record["size_bytes"]) is not int
            or record["size_bytes"] != snapshot.size
            or _terminal_mode(record["mode"], label) != snapshot.mode
            or (expected_mode is not None and snapshot.mode != expected_mode)
        ):
            raise BootstrapError(
                f"{label} changed or its archive metadata is not exact"
            )
        inode = (snapshot.device, snapshot.inode)
        if snapshot.path in artifact_paths or inode in artifact_inodes:
            raise BootstrapError(f"{label} aliases another terminal artifact")
        artifact_paths[snapshot.path] = label
        artifact_inodes[inode] = label
        artifact_snapshots.append(snapshot)
        capture_directory(
            snapshot.path.parent,
            f"{label} parent directory",
            containment_root=containment_root,
        )
        return snapshot

    def require_inventory(
        path: Path,
        expected: set[str],
        label: str,
        *,
        containment_root: Path = evidence,
        expected_mode: int | None = None,
    ) -> None:
        capture_directory(
            path,
            label,
            containment_root=containment_root,
            expected_mode=expected_mode,
        )
        try:
            with os.scandir(path) as iterator:
                entries = list(iterator)
        except OSError as error:
            raise BootstrapError(f"{label} cannot be enumerated") from error
        if {entry.name for entry in entries} != expected:
            raise BootstrapError(
                f"{label} changed or has the wrong closed inventory"
            )
        for entry in entries:
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise BootstrapError(f"{label} entry is unavailable") from error
            if stat.S_ISLNK(metadata.st_mode) or not (
                stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)
            ):
                raise BootstrapError(f"{label} contains an unsafe entry")

    simple_specs = (
        (
            "g_unit_focused_test_inventory",
            "g-unit-required-tests.tsv",
            "corridor_completion",
            16 * 1024 * 1024,
        ),
        (
            "formal_multilane_apalache_evidence",
            "multilane_apalache_evidence.tsv",
            "formal_completion",
            16 * 1024 * 1024,
        ),
        (
            "formal_tlaps_resource_jsonl",
            "tlaps_resource.jsonl",
            "formal_completion",
            256 * 1024 * 1024,
        ),
        (
            "formal_tlaps_resource_summary",
            "tlaps_resource_summary.json",
            "formal_completion",
            128 * 1024 * 1024,
        ),
    )
    for label, filename, family_completion, maximum_bytes in simple_specs:
        family = receipt_evidence[family_completion]
        if not isinstance(family, dict) or not isinstance(family.get("path"), str):
            raise BootstrapError(f"terminal release evidence {family_completion} is malformed")
        family_path = _absolute_resolved_existing(
            Path(family["path"]), f"terminal receipt {family_completion}"
        )
        capture_artifact(
            receipt_evidence[label],
            f"terminal receipt {label}",
            full=False,
            expected_path=family_path.with_name(filename),
            maximum_bytes=maximum_bytes,
        )
        capture_directory(family_path.parent, f"terminal {label} family directory")

    cargo_cache = _require_exact_json_fields(
        receipt_evidence["cargo_cache_input"],
        {
            "schema_version",
            "inventory",
            "final_inventory",
            "runtime_inventory",
            "runtime_environment_sha256",
            "runtime_directories",
            "cargo_home",
            "source_cargo_home_disclosure",
            "input_root_count",
            "input_record_count",
            "input_file_count",
        },
        "terminal Cargo-cache authentication",
    )
    if (
        type(cargo_cache["schema_version"]) is not int
        or cargo_cache["schema_version"] != 2
        or cargo_cache["source_cargo_home_disclosure"] != "withheld"
        or any(
            type(cargo_cache[name]) is not int or cargo_cache[name] < 0
            for name in ("input_root_count", "input_record_count", "input_file_count")
        )
    ):
        raise BootstrapError("terminal Cargo-cache authentication is malformed")
    cargo_file_specs = (
        (
            "inventory",
            "release-cargo-cache.input-inventory.v1",
            evidence / "cargo-cache-input.json",
        ),
        (
            "final_inventory",
            "release-cargo-cache.final-inventory.v1",
            evidence / "cargo-cache-final.json",
        ),
        (
            "runtime_inventory",
            "release-runtime.inventory.v1",
            evidence.parent / "runtime-input.json",
        ),
    )
    for field, archive_id, expected_path in cargo_file_specs:
        capture_archive(
            cargo_cache[field],
            f"terminal Cargo-cache {field}",
            archive_id=archive_id,
            expected_path=expected_path,
            maximum_bytes=16 * 1024 * 1024,
            expected_mode=_DATA_MODE,
            containment_root=evidence.parent,
        )
    if (
        receipt_evidence["cargo_cache_input_inventory"] != cargo_cache["inventory"]
        or receipt_evidence["cargo_cache_final_inventory"]
        != cargo_cache["final_inventory"]
    ):
        raise BootstrapError("terminal Cargo-cache inventory aliases disagree")
    expected_runtime_environment = {
        "runtime_home_path": str(evidence / "home"),
        "runtime_tmpdir_path": str(evidence / "tmp"),
        "runtime_tmp_path": str(evidence / "tmp"),
        "runtime_temp_path": str(evidence / "tmp"),
        "runtime_cache_path": str(evidence / "cache"),
    }
    if cargo_cache["runtime_environment_sha256"] != hashlib.sha256(
        _canonical_json(expected_runtime_environment)
    ).hexdigest():
        raise BootstrapError("terminal runtime environment digest is not exact")
    runtime_directories = _require_exact_json_fields(
        cargo_cache["runtime_directories"],
        {"cache", "home", "tmp"},
        "terminal runtime directories",
    )
    for name in ("cache", "home", "tmp"):
        if runtime_directories[name] != {
            "archive_id": f"release-runtime.directory.{name}.v1",
            "mode": "0700",
        }:
            raise BootstrapError(
                f"terminal runtime directory {name} authentication is malformed"
            )
    if cargo_cache["cargo_home"] != {
        "archive_id": "release-cargo-cache.home.v1",
        "mode": "0700",
    }:
        raise BootstrapError("terminal Cargo home authentication is malformed")

    sdk = _require_exact_json_fields(
        receipt_evidence["sdk_dependencies"],
        {
            "schema_version", "source_disclosure", "source_manifest_sha256",
            "source_state_sha256", "archive", "input_inventory",
            "final_work_inventory",
        },
        "terminal SDK dependencies",
    )
    if (
        type(sdk["schema_version"]) is not int
        or sdk["schema_version"] != 1
        or sdk["source_disclosure"] != "withheld"
        or not isinstance(sdk["source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(sdk["source_manifest_sha256"]) is None
        or not isinstance(sdk["source_state_sha256"], str)
        or _DIGEST_RE.fullmatch(sdk["source_state_sha256"]) is None
    ):
        raise BootstrapError("terminal SDK dependency identity is malformed")
    try:
        sdk_manifest_record = receipt_evidence["bootstrap"]["trusted_inputs"][
            "sdk_dependency_bundle_manifest"
        ]
    except (KeyError, TypeError) as error:
        raise BootstrapError(
            "terminal SDK dependency manifest authentication is absent"
        ) from error
    if (
        not isinstance(sdk_manifest_record, dict)
        or sdk_manifest_record.get("sha256") != sdk["source_manifest_sha256"]
    ):
        raise BootstrapError(
            "terminal SDK dependency manifest digest is not bootstrap-bound"
        )
    sdk_root = release_root.parent
    sdk_specs = (
        (
            "archive", "release-sdk-dependencies.bundle.v1",
            "sdk-dependency-bundle.tar", _MAX_RETAINED_TOTAL_BYTES,
        ),
        (
            "input_inventory", "release-sdk-dependencies.input-inventory.v1",
            "sdk-dependency-input.json", 256 * 1024 * 1024,
        ),
        (
            "final_work_inventory", "release-sdk-dependencies.work-final.v1",
            "sdk-dependency-work-final.json", 256 * 1024 * 1024,
        ),
    )
    sdk_snapshots: dict[str, LargeFileSnapshot] = {}
    for field, archive_id, archive_name, maximum_bytes in sdk_specs:
        record = _require_exact_json_fields(
            sdk[field],
            {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
            f"terminal SDK {field}",
        )
        if record["archive_name"] != archive_name:
            raise BootstrapError(f"terminal SDK {field} archive name is not exact")
        sdk_snapshots[field] = capture_archive(
            record,
            f"terminal SDK {field}",
            archive_id=archive_id,
            expected_path=sdk_root / archive_name,
            maximum_bytes=maximum_bytes,
            expected_mode=_DATA_MODE,
            containment_root=sdk_root,
            extra_fields=frozenset({"archive_name"}),
        )

    def sdk_document(field: str, label: str) -> dict[str, Any]:
        snapshot = _read_file(
            sdk_snapshots[field].path,
            label,
            maximum_bytes=256 * 1024 * 1024,
        )
        if snapshot.sha256 != sdk_snapshots[field].sha256:
            raise BootstrapError(f"{label} changed before semantic replay")
        return _parse_canonical_json(snapshot, label)

    def sdk_records(
        value: Any, label: str, root_mode: str,
    ) -> tuple[list[dict[str, Any]], int]:
        if not isinstance(value, list) or not value or len(value) > _MAX_RETAINED_RECORDS:
            raise BootstrapError(f"{label} record inventory is not bounded")
        paths: list[str] = []
        by_path: dict[str, dict[str, Any]] = {}
        file_bytes = 0
        for index, record in enumerate(value):
            if not isinstance(record, dict):
                raise BootstrapError(f"{label} record {index} is malformed")
            kind = record.get("kind")
            expected_fields = {
                "directory": {"path", "kind", "mode"},
                "file": {"path", "kind", "mode", "size", "sha256"},
                "symlink": {"path", "kind", "mode", "target"},
            }.get(kind)
            path = record.get("path")
            if (
                expected_fields is None
                or set(record) != expected_fields
                or not isinstance(path, str)
                or (path != "." and (
                    PurePosixPath(path).is_absolute()
                    or PurePosixPath(path).as_posix() != path
                    or not PurePosixPath(path).parts
                    or any(part in {"", ".", ".."} for part in PurePosixPath(path).parts)
                ))
                or not isinstance(record.get("mode"), str)
                or re.fullmatch(r"[0-7]{4}", record["mode"]) is None
            ):
                raise BootstrapError(f"{label} record {index} is not path-free")
            if kind == "file":
                size = record["size"]
                if (
                    type(size) is not int
                    or not 0 <= size <= _MAX_RETAINED_FILE_BYTES
                    or not isinstance(record["sha256"], str)
                    or _DIGEST_RE.fullmatch(record["sha256"]) is None
                ):
                    raise BootstrapError(f"{label} file record is malformed")
                file_bytes += size
                if file_bytes > _MAX_RETAINED_TOTAL_BYTES:
                    raise BootstrapError(f"{label} file bytes exceed their bound")
            elif kind == "symlink":
                target = record["target"]
                if (
                    not isinstance(target, str)
                    or "\0" in target
                    or PurePosixPath(target).is_absolute()
                    or PurePosixPath(target).as_posix() != target
                ):
                    raise BootstrapError(f"{label} symlink target is unsafe")
            paths.append(path)
            by_path[path] = record
        if (
            value[0] != {"path": ".", "kind": "directory", "mode": root_mode}
            or len(set(paths)) != len(paths)
            or paths[1:] != sorted(paths[1:])
        ):
            raise BootstrapError(f"{label} ordering or root is not exact")
        for path, record in by_path.items():
            if path == ".":
                continue
            pure = PurePosixPath(path)
            parent = pure.parent.as_posix()
            if by_path.get(parent, {}).get("kind") != "directory":
                raise BootstrapError(f"{label} member lacks its exact parent")
            if record["kind"] == "symlink":
                parts = list(pure.parent.parts) if parent != "." else []
                for part in PurePosixPath(record["target"]).parts:
                    if part in {"", "."}:
                        continue
                    if part == "..":
                        if not parts:
                            raise BootstrapError(f"{label} symlink escapes its root")
                        parts.pop()
                    else:
                        parts.append(part)
                if ("/".join(parts) or ".") not in by_path:
                    raise BootstrapError(f"{label} symlink target is not inventoried")
        return value, file_bytes

    input_document = _require_exact_json_fields(
        sdk_document("input_inventory", "terminal SDK input inventory"),
        {
            "format", "schema_version", "archive_id", "source_disclosure",
            "source_manifest_sha256", "source_state_sha256", "bindings",
            "archive", "record_count", "file_bytes", "records",
            "work_initial_record_count", "work_initial_file_bytes",
            "work_initial_records",
        },
        "terminal SDK input inventory",
    )
    input_records, input_bytes = sdk_records(
        input_document["records"], "terminal SDK input inventory", "0500"
    )
    initial_records, initial_bytes = sdk_records(
        input_document["work_initial_records"],
        "terminal SDK initial-work inventory", "0700",
    )
    if (
        input_document["format"] != "iroha-sumeragi-v2-sdk-dependency-bundle"
        or input_document["schema_version"] != 1
        or input_document["archive_id"] != "release-sdk-dependencies.bundle.v1"
        or input_document["source_disclosure"] != "withheld"
        or input_document["source_manifest_sha256"] != sdk["source_manifest_sha256"]
        or input_document["source_state_sha256"] != sdk["source_state_sha256"]
        or input_document["archive"] != sdk["archive"]
        or input_document["record_count"] != len(input_records)
        or input_document["file_bytes"] != input_bytes
        or input_document["work_initial_record_count"] != len(initial_records)
        or input_document["work_initial_file_bytes"] != initial_bytes
        or not isinstance(input_document["bindings"], dict)
    ):
        raise BootstrapError("terminal SDK input inventory binding is not exact")
    input_by_path = {str(record["path"]): record for record in input_records}
    bindings = _require_exact_json_fields(
        input_document["bindings"], {"node", "swiftpm", "gradle"},
        "terminal SDK dependency bindings",
    )
    node_binding = _require_exact_json_fields(
        bindings["node"],
        {
            "node_modules_archive_name", "package_lock_archive_name",
            "package_lock_sha256", "installed_lock_sha256",
        },
        "terminal SDK Node binding",
    )
    swift_binding = _require_exact_json_fields(
        bindings["swiftpm"],
        {
            "cache_archive_name", "package_resolved_archive_name",
            "package_resolved_sha256", "resolved_revisions",
        },
        "terminal SDK SwiftPM binding",
    )
    gradle_binding = _require_exact_json_fields(
        bindings["gradle"],
        {
            "distribution_archive_name", "distribution_sha256",
            "distribution_url", "gradle_user_home_archive_name",
            "launcher_archive_name", "wrapper_cache_key", "version",
            "wrapper_properties_sha256",
        },
        "terminal SDK Gradle binding",
    )
    wrapper_digests = _require_exact_json_fields(
        gradle_binding["wrapper_properties_sha256"], {"java", "kotlin"},
        "terminal SDK Gradle wrapper digests",
    )
    gradle_url = (
        "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
    )
    gradle_key = "79n14ral3mx1ozqr3csh2u872"
    gradle_launcher = (
        "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
        f"{gradle_key}/gradle-9.3.0/bin/gradle"
    )
    if (
        node_binding["node_modules_archive_name"] != "node/node_modules"
        or node_binding["package_lock_archive_name"] != "node/package-lock.json"
        or swift_binding["cache_archive_name"] != "swiftpm/cache"
        or swift_binding["package_resolved_archive_name"]
        != "swiftpm/Package.resolved"
        or gradle_binding["distribution_archive_name"]
        != "gradle/gradle-9.3.0-bin.zip"
        or gradle_binding["distribution_url"] != gradle_url
        or gradle_binding["gradle_user_home_archive_name"]
        != "gradle/gradle-user-home"
        or gradle_binding["launcher_archive_name"] != gradle_launcher
        or gradle_binding["wrapper_cache_key"] != gradle_key
        or gradle_binding["version"] != "9.3.0"
    ):
        raise BootstrapError("terminal SDK path-free bindings are not exact")
    for digest, label in (
        (node_binding["package_lock_sha256"], "terminal SDK package lock"),
        (node_binding["installed_lock_sha256"], "terminal SDK installed lock"),
        (swift_binding["package_resolved_sha256"], "terminal SDK Package.resolved"),
        (gradle_binding["distribution_sha256"], "terminal SDK Gradle distribution"),
        (wrapper_digests["java"], "terminal SDK Java wrapper"),
        (wrapper_digests["kotlin"], "terminal SDK Kotlin wrapper"),
    ):
        _require_digest(digest, label)
    revisions = swift_binding["resolved_revisions"]
    if not isinstance(revisions, list) or not revisions:
        raise BootstrapError("terminal SDK SwiftPM revisions are absent")
    revision_identities: list[str] = []
    for item in revisions:
        item = _require_exact_json_fields(
            item, {"identity", "checkout", "revision", "tree"},
            "terminal SDK SwiftPM revision",
        )
        if (
            not isinstance(item["identity"], str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", item["identity"])
            is None
            or not isinstance(item["checkout"], str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", item["checkout"])
            is None
            or not isinstance(item["revision"], str)
            or _OBJECT_ID_RE.fullmatch(item["revision"]) is None
            or not isinstance(item["tree"], str)
            or _OBJECT_ID_RE.fullmatch(item["tree"]) is None
        ):
            raise BootstrapError("terminal SDK SwiftPM revision is malformed")
        revision_identities.append(item["identity"])
    if revision_identities != sorted(set(revision_identities)):
        raise BootstrapError("terminal SDK SwiftPM revisions are not unique")
    expected_file_digests = {
        "node/package-lock.json": node_binding["package_lock_sha256"],
        "node/node_modules/.package-lock.json": node_binding["installed_lock_sha256"],
        "swiftpm/Package.resolved": swift_binding["package_resolved_sha256"],
        "gradle/gradle-9.3.0-bin.zip": gradle_binding["distribution_sha256"],
        "gradle/java-gradle-wrapper.properties": wrapper_digests["java"],
        "gradle/kotlin-gradle-wrapper.properties": wrapper_digests["kotlin"],
    }
    if any(
        input_by_path.get(path, {}).get("kind") != "file"
        or input_by_path[path].get("sha256") != digest
        for path, digest in expected_file_digests.items()
    ) or any(
        input_by_path.get(path, {}).get("kind") != "directory"
        for path in (
            "node/node_modules", "swiftpm/cache", "swiftpm/cache/checkouts",
            "swiftpm/cache/repositories", "gradle/gradle-user-home",
        )
    ) or (
        input_by_path.get(gradle_launcher, {}).get("kind") != "file"
        or int(str(input_by_path[gradle_launcher]["mode"]), 8) & 0o111 == 0
    ):
        raise BootstrapError("terminal SDK bindings do not match retained members")

    sdk_source_snapshot = _read_file(
        evidence / "sdk-dependency-bundle-manifest.json",
        "bootstrap-private SDK dependency source manifest",
        maximum_bytes=256 * 1024 * 1024,
    )
    if (
        sdk_source_snapshot.sha256 != sdk["source_manifest_sha256"]
        or sdk_source_snapshot.mode != _DATA_MODE
        or sdk_source_snapshot.owner != os.getuid()
        or sdk_source_snapshot.nlink != 1
    ):
        raise BootstrapError("terminal SDK private manifest changed")
    source_manifest = _require_exact_json_fields(
        _parse_canonical_json(
            sdk_source_snapshot, "bootstrap-private SDK dependency source manifest",
        ),
        {"format", "schema_version", "git", "node", "swiftpm", "gradle"},
        "bootstrap-private SDK dependency source manifest",
    )
    source_git = _require_exact_json_fields(
        source_manifest["git"], {"executable", "sha256"},
        "bootstrap-private SDK Git binding",
    )
    source_node = _require_exact_json_fields(
        source_manifest["node"],
        {"node_modules_root", "node_modules_inventory", "package_lock_sha256"},
        "bootstrap-private SDK Node source",
    )
    source_swift = _require_exact_json_fields(
        source_manifest["swiftpm"],
        {
            "cache_root", "cache_inventory", "package_resolved_sha256",
            "resolved_revisions",
        },
        "bootstrap-private SDK SwiftPM source",
    )
    source_gradle = _require_exact_json_fields(
        source_manifest["gradle"],
        {
            "distribution_archive", "distribution_sha256", "distribution_url",
            "gradle_user_home", "gradle_user_home_inventory",
            "java_wrapper_properties_sha256", "kotlin_wrapper_properties_sha256",
            "version", "wrapper_cache_key",
        },
        "bootstrap-private SDK Gradle source",
    )
    try:
        bootstrap_git_record = receipt_evidence["bootstrap"]["trusted_inputs"]["git"]
    except (KeyError, TypeError) as error:
        raise BootstrapError("terminal SDK protected Git binding is absent") from error
    for value, label in (
        (source_git["executable"], "bootstrap-private SDK Git"),
        (source_node["node_modules_root"], "bootstrap-private node_modules"),
        (source_swift["cache_root"], "bootstrap-private SwiftPM cache"),
        (source_gradle["distribution_archive"], "bootstrap-private Gradle ZIP"),
        (source_gradle["gradle_user_home"], "bootstrap-private Gradle home"),
    ):
        if (
            not isinstance(value, str)
            or "\0" in value
            or not Path(value).is_absolute()
            or value != os.path.abspath(os.path.normpath(value))
        ):
            raise BootstrapError(f"{label} path is not exact")
    if (
        source_manifest["format"]
        != "iroha-sumeragi-v2-sdk-dependency-sources"
        or type(source_manifest["schema_version"]) is not int
        or source_manifest["schema_version"] != 2
        or not isinstance(bootstrap_git_record, dict)
        or source_git["sha256"] != bootstrap_git_record.get("sha256")
        or source_node["package_lock_sha256"]
        != node_binding["package_lock_sha256"]
        or source_swift["package_resolved_sha256"]
        != swift_binding["package_resolved_sha256"]
        or source_swift["resolved_revisions"] != revisions
        or source_gradle["distribution_sha256"]
        != gradle_binding["distribution_sha256"]
        or source_gradle["distribution_url"] != gradle_url
        or source_gradle["java_wrapper_properties_sha256"]
        != wrapper_digests["java"]
        or source_gradle["kotlin_wrapper_properties_sha256"]
        != wrapper_digests["kotlin"]
        or source_gradle["version"] != "9.3.0"
        or source_gradle["wrapper_cache_key"] != gradle_key
    ):
        raise BootstrapError("terminal SDK private bindings disagree with receipt")

    def source_inventory(
        value: Any, label: str,
    ) -> list[dict[str, Any]]:
        inventory = _require_exact_json_fields(
            value,
            {
                "format", "schema_version", "record_count", "file_bytes",
                "records_sha256", "records",
            },
            label,
        )
        raw_records = inventory["records"]
        if (
            not isinstance(raw_records, list)
            or not raw_records
            or not isinstance(raw_records[0], dict)
            or raw_records[0].get("path") != "."
            or raw_records[0].get("kind") != "directory"
            or not isinstance(raw_records[0].get("mode"), str)
        ):
            raise BootstrapError(f"{label} root is malformed")
        records, file_bytes = sdk_records(
            raw_records, label, raw_records[0]["mode"],
        )
        record_payload = json.dumps(
            records, ensure_ascii=True, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
        if (
            inventory["format"]
            != "iroha-sumeragi-v2-sdk-dependency-source-inventory"
            or type(inventory["schema_version"]) is not int
            or inventory["schema_version"] != 1
            or inventory["record_count"] != len(records)
            or inventory["file_bytes"] != file_bytes
            or inventory["records_sha256"]
            != hashlib.sha256(record_payload).hexdigest()
        ):
            raise BootstrapError(f"{label} accounting is not exact")
        return records

    source_specs = (
        (
            "node/node_modules", source_node["node_modules_inventory"],
            "terminal SDK private Node inventory",
        ),
        (
            "swiftpm/cache", source_swift["cache_inventory"],
            "terminal SDK private SwiftPM inventory",
        ),
        (
            "gradle/gradle-user-home",
            source_gradle["gradle_user_home_inventory"],
            "terminal SDK private Gradle inventory",
        ),
    )
    source_maps: dict[str, dict[str, dict[str, Any]]] = {}
    for prefix, raw_inventory, label in source_specs:
        records = source_inventory(raw_inventory, label)
        source_maps[prefix] = {str(record["path"]): record for record in records}
        projected: list[dict[str, Any]] = []
        for source_record in records:
            projected_record = dict(source_record)
            relative = str(source_record["path"])
            projected_record["path"] = (
                prefix if relative == "." else f"{prefix}/{relative}"
            )
            if source_record["kind"] == "directory":
                projected_record["mode"] = "0500"
            elif source_record["kind"] == "file":
                projected_record["mode"] = (
                    "0500"
                    if int(str(source_record["mode"]), 8) & 0o111
                    else "0400"
                )
            projected.append(projected_record)
        retained = [
            record for record in input_records
            if record["path"] == prefix
            or str(record["path"]).startswith(prefix + "/")
        ]
        if sorted(projected, key=lambda item: str(item["path"])) != retained:
            raise BootstrapError(f"{label} does not reproduce the retained subtree")
    swift_source_records = source_maps["swiftpm/cache"]
    swift_top = {
        path for path in swift_source_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    checkouts = {str(item["checkout"]) for item in revisions}
    observed_checkouts = {
        path.removeprefix("checkouts/")
        for path, record in swift_source_records.items()
        if path.startswith("checkouts/")
        and "/" not in path.removeprefix("checkouts/")
        and record.get("kind") == "directory"
    }
    if (
        swift_top != {"checkouts", "repositories"}
        or observed_checkouts != checkouts
        or any(
            swift_source_records.get(
                f"checkouts/{checkout}/.git/HEAD", {}
            ).get("kind") != "file"
            for checkout in checkouts
        )
    ):
        raise BootstrapError("terminal SDK SwiftPM source topology is not exact")
    gradle_source_records = source_maps["gradle/gradle-user-home"]
    gradle_top = {
        path for path in gradle_source_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    gradle_cache_root = f"wrapper/dists/gradle-9.3.0-bin/{gradle_key}"
    if (
        gradle_top != {"caches", "wrapper"}
        or gradle_source_records.get("caches/9.3.0", {}).get("kind")
        != "directory"
        or gradle_source_records.get("caches/modules-2", {}).get("kind")
        != "directory"
        or gradle_source_records.get(gradle_cache_root, {}).get("kind")
        != "directory"
        or gradle_source_records.get(
            f"{gradle_cache_root}/gradle-9.3.0-bin.zip.ok", {}
        ).get("kind") != "file"
        or gradle_source_records.get(
            gradle_launcher.removeprefix("gradle/gradle-user-home/"), {}
        ).get("kind") != "file"
    ):
        raise BootstrapError("terminal SDK Gradle source topology is not exact")
    final_document = _require_exact_json_fields(
        sdk_document("final_work_inventory", "terminal SDK final-work inventory"),
        {
            "format", "schema_version", "archive_id",
            "sdk_dependency_inventory_sha256", "record_count", "file_bytes",
            "records",
        },
        "terminal SDK final-work inventory",
    )
    final_records, final_bytes = sdk_records(
        final_document["records"], "terminal SDK final-work inventory", "0700"
    )
    if (
        final_document["format"]
        != "iroha-sumeragi-v2-sdk-dependency-work-final"
        or final_document["schema_version"] != 1
        or final_document["archive_id"]
        != "release-sdk-dependencies.work-final.v1"
        or final_document["sdk_dependency_inventory_sha256"]
        != sdk_snapshots["input_inventory"].sha256
        or final_document["record_count"] != len(final_records)
        or final_document["file_bytes"] != final_bytes
        or final_records != initial_records
        or final_bytes != initial_bytes
    ):
        raise BootstrapError("terminal SDK final-work inventory is not exact")
    expected_members = {
        ("sdk-inputs" if record["path"] == "." else f"sdk-inputs/{record['path']}"): record
        for record in input_records
    }
    control_names = {
        "node/package-lock.json",
        "node/node_modules/.package-lock.json",
        "swiftpm/Package.resolved",
        "gradle/java-gradle-wrapper.properties",
        "gradle/kotlin-gradle-wrapper.properties",
        *(
            f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
            for item in revisions
        ),
    }
    controls: dict[str, bytes] = {}
    try:
        with tarfile.open(sdk_snapshots["archive"].path, mode="r:") as archive:
            members = archive.getmembers()
            if len(members) != len(expected_members) or {item.name for item in members} != set(expected_members):
                raise BootstrapError("terminal SDK tar inventory is not exact")
            for member in members:
                record = expected_members[member.name]
                kind = record["kind"]
                if (
                    member.uid != 0 or member.gid != 0 or member.mtime != 0
                    or member.mode != int(record["mode"], 8)
                    or (kind == "directory" and not member.isdir())
                    or (kind == "symlink" and (not member.issym() or member.linkname != record["target"]))
                    or (kind == "file" and (not member.isfile() or member.size != record["size"]))
                ):
                    raise BootstrapError("terminal SDK tar member metadata changed")
                if kind == "file":
                    stream = archive.extractfile(member)
                    if stream is None:
                        raise BootstrapError("terminal SDK tar member is unavailable")
                    digest = hashlib.sha256()
                    relative = member.name.removeprefix("sdk-inputs/")
                    captured = bytearray()
                    while block := stream.read(1024 * 1024):
                        digest.update(block)
                        if relative in control_names:
                            captured.extend(block)
                            if len(captured) > 16 * 1024 * 1024:
                                raise BootstrapError(
                                    "terminal SDK control file exceeds its bound"
                                )
                    if digest.hexdigest() != record["sha256"]:
                        raise BootstrapError("terminal SDK tar member digest changed")
                    if relative in control_names:
                        controls[relative] = bytes(captured)
    except (OSError, tarfile.TarError) as error:
        raise BootstrapError("terminal SDK dependency tar is malformed") from error

    def control_json(name: str) -> dict[str, Any]:
        data = controls.get(name)
        if data is None:
            raise BootstrapError(f"terminal SDK control file is absent: {name}")
        try:
            value = json.loads(data)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BootstrapError(f"terminal SDK control file is malformed: {name}") from error
        if not isinstance(value, dict) or data != _canonical_json(value):
            raise BootstrapError(f"terminal SDK control file is noncanonical: {name}")
        return value

    package_lock = control_json("node/package-lock.json")
    installed_lock = control_json("node/node_modules/.package-lock.json")
    if (
        package_lock.get("lockfileVersion") != 3
        or installed_lock.get("lockfileVersion") != 3
        or (package_lock.get("name"), package_lock.get("version"))
        != (installed_lock.get("name"), installed_lock.get("version"))
        or not isinstance(package_lock.get("packages"), dict)
        or not isinstance(installed_lock.get("packages"), dict)
        or not installed_lock["packages"]
        or any(
            package_lock["packages"].get(name) != value
            for name, value in installed_lock["packages"].items()
        )
    ):
        raise BootstrapError("terminal SDK Node locks do not bind the closure")
    package_resolved = control_json("swiftpm/Package.resolved")
    resolved_pairs = sorted(
        (
            {
                "identity": pin.get("identity"),
                "revision": pin.get("state", {}).get("revision"),
            }
            for pin in package_resolved.get("pins", [])
            if isinstance(pin, dict) and isinstance(pin.get("state"), dict)
        ),
        key=lambda item: str(item["identity"]),
    ) if isinstance(package_resolved.get("pins"), list) else []
    if (
        package_resolved.get("version") != 2
        or resolved_pairs != [
            {"identity": item["identity"], "revision": item["revision"]}
            for item in revisions
        ]
    ):
        raise BootstrapError("terminal SDK Swift revisions are not exact")
    for item in revisions:
        head = controls.get(
            f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
        )
        try:
            observed_head = head.decode("ascii", "strict").strip()
        except (AttributeError, UnicodeDecodeError) as error:
            raise BootstrapError("terminal SDK Swift checkout HEAD is malformed") from error
        if observed_head != item["revision"]:
            raise BootstrapError("terminal SDK Swift checkout HEAD changed")
    for kind in ("java", "kotlin"):
        try:
            lines = controls[
                f"gradle/{kind}-gradle-wrapper.properties"
            ].decode("utf-8").splitlines()
        except (KeyError, UnicodeDecodeError) as error:
            raise BootstrapError("terminal SDK Gradle wrapper is malformed") from error
        values = dict(
            line.split("=", 1) for line in lines
            if line and not line.startswith("#") and "=" in line
        )
        if values.get("distributionUrl") != gradle_url.replace(":", r"\:", 1):
            raise BootstrapError("terminal SDK Gradle wrapper URL changed")
        checksum = values.get("distributionSha256Sum")
        if checksum is not None and checksum != gradle_binding["distribution_sha256"]:
            raise BootstrapError("terminal SDK Gradle wrapper digest changed")

    prebuilt = _require_exact_json_fields(
        receipt_evidence["prebuilt_binary_bundle"],
        {
            "schema_version",
            "archive_id",
            "manifest",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "cargo_version_sha256",
            "rustc_version_sha256",
            "host_triple",
            "target_triple",
            "profile",
            "version_transcripts",
            "binaries",
        },
        "terminal prebuilt binary bundle",
    )
    if type(prebuilt["schema_version"]) is not int or prebuilt["schema_version"] != 3:
        raise BootstrapError("terminal prebuilt binary bundle has the wrong schema")
    for field in (
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "cargo_version_sha256",
        "rustc_version_sha256",
    ):
        if not isinstance(prebuilt[field], str):
            raise BootstrapError(f"terminal prebuilt {field} is malformed")
        _require_digest(prebuilt[field], f"terminal prebuilt {field}")
    if (
        prebuilt["source_manifest_sha256"]
        != receipt_identity["sealed_source_manifest_sha256"]
        or prebuilt["cargo_lock_sha256"] != receipt_identity["cargo_lock_sha256"]
        or prebuilt["profile"] != "release"
        or not isinstance(prebuilt["host_triple"], str)
        or _PREBUILT_TRIPLE_RE.fullmatch(prebuilt["host_triple"]) is None
        or prebuilt["target_triple"] != prebuilt["host_triple"]
        or not isinstance(prebuilt["archive_id"], str)
        or not prebuilt["archive_id"].startswith("release-prebuilt.bundle.v1:")
    ):
        raise BootstrapError("terminal prebuilt binary bundle identity is not exact")
    release_invocation_root = release_root.parent
    artifact_root = _absolute_resolved_existing(
        release_invocation_root / "output", "terminal release artifact root"
    )
    cargo_target_root = release_invocation_root / "target"
    invocation_id = prebuilt["archive_id"].partition(":")[2]
    if _PREBUILT_INVOCATION_RE.fullmatch(invocation_id) is None:
        raise BootstrapError("terminal prebuilt archive id is malformed")
    prebuilt_root = _absolute_resolved_existing(
        artifact_root
        / "sumeragi-v2-release"
        / receipt_identity["sealed_source_manifest_sha256"]
        / "programs"
        / invocation_id,
        "terminal prebuilt bundle directory",
    )
    if (
        artifact_root != release_invocation_root / "output"
        or cargo_target_root != release_invocation_root / "target"
        or _inside(artifact_root, cargo_target_root)
        or _inside(cargo_target_root, artifact_root)
        or _inside(artifact_root, release_root)
        or _inside(cargo_target_root, release_root)
        or prebuilt_root.parent
        != (
            artifact_root
            / "sumeragi-v2-release"
            / receipt_identity["sealed_source_manifest_sha256"]
            / "programs"
        )
        or _PREBUILT_INVOCATION_RE.fullmatch(prebuilt_root.name) is None
    ):
        raise BootstrapError("terminal prebuilt bundle is outside the sealed invocation root")
    capture_directory(
        artifact_root,
        "terminal release artifact root",
        containment_root=artifact_root,
        expected_mode=_DIRECTORY_MODE,
    )
    prebuilt_manifest_snapshot = capture_archive(
        prebuilt["manifest"],
        "terminal prebuilt manifest",
        archive_id="release-prebuilt.manifest.v2",
        expected_path=prebuilt_root / ".sumeragi-v2-prebuilt-binaries.tsv",
        maximum_bytes=32 * 1024,
        expected_mode=_DATA_MODE,
        containment_root=artifact_root,
    )
    prebuilt_manifest_file = _read_file(
        prebuilt_manifest_snapshot.path,
        "terminal prebuilt manifest contents",
        maximum_bytes=32 * 1024,
    )
    if (
        prebuilt_manifest_file.sha256 != prebuilt_manifest_snapshot.sha256
        or prebuilt_manifest_file.device != prebuilt_manifest_snapshot.device
        or prebuilt_manifest_file.inode != prebuilt_manifest_snapshot.inode
        or prebuilt_manifest_file.mode != prebuilt_manifest_snapshot.mode
        or prebuilt_manifest_file.owner != prebuilt_manifest_snapshot.owner
        or prebuilt_manifest_file.nlink != prebuilt_manifest_snapshot.nlink
        or prebuilt_manifest_file.size != prebuilt_manifest_snapshot.size
        or prebuilt_manifest_file.mtime_ns != prebuilt_manifest_snapshot.mtime_ns
        or prebuilt_manifest_file.ctime_ns != prebuilt_manifest_snapshot.ctime_ns
    ):
        raise BootstrapError("terminal prebuilt manifest changed while it was decoded")
    try:
        manifest_text = prebuilt_manifest_file.data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("terminal prebuilt manifest is not UTF-8") from error
    manifest_lines = manifest_text.splitlines(keepends=True)
    if (
        len(manifest_lines) != len(_PREBUILT_MANIFEST_FIELDS)
        or any(
            not line.endswith("\n")
            or line.endswith("\r\n")
            or line.count("\t") != 1
            for line in manifest_lines
        )
    ):
        raise BootstrapError("terminal prebuilt manifest is not one exact TSV inventory")
    manifest_rows = tuple(line[:-1].split("\t", 1) for line in manifest_lines)
    if tuple(row[0] for row in manifest_rows) != _PREBUILT_MANIFEST_FIELDS:
        raise BootstrapError("terminal prebuilt manifest field order is not exact")
    manifest_fields = dict(manifest_rows)
    if (
        manifest_fields["schema_version"] != "2"
        or manifest_fields["source_manifest_sha256"]
        != prebuilt["source_manifest_sha256"]
        or manifest_fields["cargo_lock_sha256"] != prebuilt["cargo_lock_sha256"]
        or manifest_fields["cargo_version_sha256"]
        != prebuilt["cargo_version_sha256"]
        or manifest_fields["rustc_version_sha256"]
        != prebuilt["rustc_version_sha256"]
        or manifest_fields["host_triple"] != prebuilt["host_triple"]
        or manifest_fields["target_triple"] != prebuilt["target_triple"]
        or manifest_fields["profile"] != prebuilt["profile"]
        or manifest_fields["bundle_dir"] != str(prebuilt_root)
    ):
        raise BootstrapError("terminal prebuilt receipt diverges from its manifest")
    transcripts = _require_exact_json_fields(
        prebuilt["version_transcripts"], {"cargo", "rustc"}, "terminal prebuilt transcripts"
    )
    runner_tools = runner_record.get("tools")
    if not isinstance(runner_tools, dict):
        raise BootstrapError("terminal runner tool inventory is malformed")
    for tool in ("cargo", "rustc"):
        transcript = _require_exact_json_fields(
            transcripts[tool],
            {"operation_id", "tool_archive_id", "sha256", "size_bytes"},
            f"terminal {tool} transcript",
        )
        if (
            transcript["operation_id"] != f"{tool}.version.v1"
            or transcript["tool_archive_id"] != f"release-runner-tool.{tool}.v1"
            or transcript["sha256"] != prebuilt[f"{tool}_version_sha256"]
            or type(transcript["size_bytes"]) is not int
            or not 0 < transcript["size_bytes"] <= 64 * 1024
        ):
            raise BootstrapError(f"terminal {tool} transcript is malformed")
        authenticated_tool = runner_tools.get(tool)
        if not isinstance(authenticated_tool, dict):
            raise BootstrapError(f"terminal runner omits authenticated {tool}")
        authenticated_archive = authenticated_tool.get("archive_name")
        authenticated_sha256 = authenticated_tool.get("sha256")
        if not isinstance(authenticated_archive, str) or not isinstance(
            authenticated_sha256, str
        ):
            raise BootstrapError(f"terminal runner authenticated {tool} is malformed")
        bootstrap_evidence_root = authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR"
        )
        if not isinstance(bootstrap_evidence_root, str):
            raise BootstrapError("terminal runner evidence root is unavailable")
        executable = _absolute_resolved_existing(
            Path(bootstrap_evidence_root) / authenticated_archive,
            f"terminal runner authenticated {tool} executable",
        )
        authenticated_digest = _require_digest(
            authenticated_sha256, f"terminal runner authenticated {tool} digest"
        )
        executable_snapshot = _capture_large_file(
            executable, f"terminal {tool} transcript executable"
        )
        if executable_snapshot.sha256 != authenticated_digest:
            raise BootstrapError(
                f"terminal {tool} transcript executable digest is not authenticated"
            )
        if (
            executable_snapshot.owner != os.getuid()
            or executable_snapshot.nlink != 1
            or executable_snapshot.mode & 0o111 == 0
        ):
            raise BootstrapError(
                f"terminal {tool} transcript executable is not exact and owner-controlled"
            )
    binaries = prebuilt["binaries"]
    if not isinstance(binaries, list) or len(binaries) != len(_PREBUILT_BINARY_SPECS):
        raise BootstrapError("terminal prebuilt binary inventory is incomplete")
    for index, ((role, relative), record) in enumerate(
        zip(_PREBUILT_BINARY_SPECS, binaries)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != role
            or record.get("relative_path") != relative
            or manifest_fields[f"{role}_relative_path"] != relative
            or manifest_fields[f"{role}_sha256"] != record.get("sha256")
            or manifest_fields[f"{role}_size_bytes"]
            != str(record.get("size_bytes"))
            or manifest_fields[f"{role}_mode_octal"] != record.get("mode")
        ):
            raise BootstrapError(f"terminal prebuilt binary {index} identity is not exact")
        capture_archive(
            record,
            f"terminal prebuilt binary {index}",
            archive_id=f"release-prebuilt.binary.{role}.v1",
            extra_fields=frozenset({"role", "relative_path"}),
            expected_path=prebuilt_root.joinpath(*relative.split("/")),
            maximum_bytes=2 * 1024 * 1024 * 1024,
            expected_mode=_TOOL_MODE,
            containment_root=artifact_root,
        )
    require_inventory(
        prebuilt_root,
        {".sumeragi-v2-prebuilt-binaries.tsv", "release", "message-control"},
        "terminal prebuilt invocation directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "release",
        {"iroha3d", "iroha", "kagami"},
        "terminal prebuilt release directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "message-control",
        {"release"},
        "terminal prebuilt message-control directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "message-control" / "release",
        {"iroha3d"},
        "terminal prebuilt message-control release directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )

    scaling = _require_exact_json_fields(
        receipt_evidence["multilane_scaling_bundle"],
        {"archive_id", "file_count", "total_size_bytes", "directories", "files"},
        "terminal scaling bundle",
    )
    if scaling["archive_id"] != "release-scaling.bundle.v1":
        raise BootstrapError("terminal scaling bundle archive id is malformed")
    manifest_environment = authenticated_environment.get(
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
    )
    if not isinstance(manifest_environment, str):
        raise BootstrapError("authenticated runner omits scaling manifest")
    scaling_manifest_input = _absolute_resolved_existing(
        Path(manifest_environment), "authenticated scaling manifest"
    )
    scaling_root = _absolute_resolved_existing(
        scaling_manifest_input.parent, "terminal scaling bundle root"
    )
    capture_directory(
        scaling_root,
        "terminal scaling bundle root",
        containment_root=scaling_root,
    )
    scaling_directories = scaling["directories"]
    scaling_files = scaling["files"]
    if (
        not isinstance(scaling_directories, list)
        or len(scaling_directories) > _MAX_SCALING_BUNDLE_DIRECTORY_COUNT
        or not isinstance(scaling_files, list)
        or len(scaling_files) > _MAX_SCALING_BUNDLE_FILE_COUNT
        or type(scaling["file_count"]) is not int
        or scaling["file_count"] != len(scaling_files)
        or type(scaling["total_size_bytes"]) is not int
        or not 0 <= scaling["total_size_bytes"] <= _MAX_SCALING_BUNDLE_TOTAL_BYTES
    ):
        raise BootstrapError("terminal scaling bundle inventory is malformed")
    parsed_directories: list[str] = []
    for index, relative in enumerate(scaling_directories):
        parts = _terminal_relative_path(relative, f"terminal scaling directory {index}")
        parsed_directories.append(relative)
        capture_directory(
            scaling_root.joinpath(*parts),
            f"terminal scaling directory {index}",
            containment_root=scaling_root,
        )
    if parsed_directories != sorted(set(parsed_directories)):
        raise BootstrapError("terminal scaling directories are not sorted and unique")
    parsed_files: list[str] = []
    total_size = 0
    scaling_manifest: Path | None = None
    for index, record in enumerate(scaling_files):
        if not isinstance(record, dict):
            raise BootstrapError(f"terminal scaling file {index} is malformed")
        relative = record.get("relative_path")
        parts = _terminal_relative_path(relative, f"terminal scaling file {index}")
        parsed_files.append(relative)
        snapshot = capture_archive(
            record,
            f"terminal scaling file {index}",
            archive_id="release-scaling.file.v1:" + relative,
            expected_path=scaling_root.joinpath(*parts),
            maximum_bytes=_MAX_SCALING_BUNDLE_FILE_BYTES,
            containment_root=scaling_root,
            extra_fields=frozenset({"relative_path"}),
        )
        total_size += snapshot.size
        if relative == "scaling_evidence.json":
            scaling_manifest = snapshot.path
    if parsed_files != sorted(set(parsed_files)) or total_size != scaling["total_size_bytes"]:
        raise BootstrapError("terminal scaling files are not one exact sorted inventory")
    if scaling_manifest is None:
        raise BootstrapError("terminal scaling bundle omits scaling_evidence.json")
    live_files: list[str] = []
    live_directories: list[str] = []
    for current, names, filenames in os.walk(scaling_root, followlinks=False):
        current_path = Path(current)
        for name in names:
            path = current_path / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise BootstrapError("terminal scaling bundle contains an unsafe directory")
            live_directories.append(path.relative_to(scaling_root).as_posix())
        for name in filenames:
            path = current_path / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
                raise BootstrapError("terminal scaling bundle contains an unsafe file")
            live_files.append(path.relative_to(scaling_root).as_posix())
    if sorted(live_directories) != parsed_directories or sorted(live_files) != parsed_files:
        raise BootstrapError("terminal scaling bundle live inventory does not match receipt")

    retained_validator = capture_archive(
        receipt_evidence["multilane_scaling_retained_validator"],
        "terminal retained scaling validator",
        archive_id="release-scaling.retained-validator.v1",
        expected_path=release_root / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py",
        containment_root=release_root,
    )
    trust_anchors = _require_exact_json_fields(
        receipt_evidence["multilane_scaling_trust_anchors"],
        {
            "trial_harness_sha256",
            "configuration_sha256",
            "irohad_sha256",
            "iroha_cli_sha256",
            "retained_tooling",
        },
        "terminal scaling trust anchors",
    )
    for field, environment_name in _SCALING_DIGEST_ENVIRONMENT.items():
        value = authenticated_environment.get(environment_name)
        if not isinstance(value, str):
            raise BootstrapError(f"authenticated runner environment omits {environment_name}")
        _require_digest(value, f"authenticated {environment_name}")
        if trust_anchors[field] != value:
            raise BootstrapError(f"terminal scaling trust anchor {field} is not authenticated")
    if manifest_environment != str(scaling_manifest):
        raise BootstrapError("terminal scaling manifest is not the authenticated runner input")
    retained_tooling = trust_anchors["retained_tooling"]
    if not isinstance(retained_tooling, list) or len(retained_tooling) != len(_SCALING_REQUIRED_TOOLING):
        raise BootstrapError("terminal scaling retained tooling inventory is incomplete")
    for index, ((role, source_path), record) in enumerate(
        zip(_SCALING_REQUIRED_TOOLING, retained_tooling)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != role
        ):
            raise BootstrapError(f"terminal retained scaling tool {index} identity is not exact")
        capture_archive(
            record,
            f"terminal retained scaling tool {index}",
            archive_id=f"release-scaling.retained-tool.{role}.v1",
            extra_fields=frozenset({"role"}),
            expected_path=release_root.joinpath(*source_path.split("/")),
            containment_root=release_root,
        )
    if retained_validator.mode & 0o111 == 0:
        raise BootstrapError("terminal retained scaling validator is not executable")

    g4p = _require_exact_json_fields(
        receipt_evidence["g4p_multilane"],
        {"schema_version", "completion", "run_summary", "run_logs"},
        "terminal G-4P evidence",
    )
    if type(g4p["schema_version"]) is not int or g4p["schema_version"] != 1:
        raise BootstrapError("terminal G-4P evidence has the wrong schema")
    if not isinstance(g4p["completion"], dict) or not isinstance(g4p["completion"].get("path"), str):
        raise BootstrapError("terminal G-4P completion is malformed")
    g4p_root = Path(g4p["completion"]["path"]).parent
    capture_artifact(
        g4p["completion"],
        "terminal G-4P completion",
        full=True,
        expected_path=g4p_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g4p["run_summary"],
        "terminal G-4P run summary",
        full=True,
        expected_path=g4p_root / "runs.tsv",
        maximum_bytes=1024 * 1024,
    )
    g4p_logs = g4p["run_logs"]
    g4p_names = (
        "run-00-nexus_and_streaming.log",
        "run-01-nexus_and_streaming.log",
        "run-02-nexus_and_streaming.log",
        "run-03-native_amx_routing.log",
    )
    if not isinstance(g4p_logs, list) or len(g4p_logs) != len(g4p_names):
        raise BootstrapError("terminal G-4P run-log inventory is incomplete")
    for index, (record, filename) in enumerate(zip(g4p_logs, g4p_names)):
        capture_artifact(
            record,
            f"terminal G-4P run log {index}",
            full=True,
            expected_path=g4p_root / filename,
            maximum_bytes=16 * 1024 * 1024,
        )
    require_inventory(g4p_root, {"COMPLETED.tsv", "runs.tsv", *g4p_names}, "terminal G-4P directory")

    g12 = _require_exact_json_fields(
        receipt_evidence["g12_cross_dataspace"],
        {
            "seed_completion",
            "seed_summary",
            "seed_run_logs",
            "fault_soak_completion",
            "fault_soak_log",
        },
        "terminal G-12 evidence",
    )
    if not isinstance(g12["seed_completion"], dict) or not isinstance(
        g12["seed_completion"].get("path"), str
    ):
        raise BootstrapError("terminal G-12 seed completion is malformed")
    if not isinstance(g12["fault_soak_completion"], dict) or not isinstance(
        g12["fault_soak_completion"].get("path"), str
    ):
        raise BootstrapError("terminal G-12 fault-soak completion is malformed")
    seed_root = Path(g12["seed_completion"]["path"]).parent
    soak_root = Path(g12["fault_soak_completion"]["path"]).parent
    if seed_root == soak_root:
        raise BootstrapError("terminal G-12 seed and soak roots are not distinct")
    capture_artifact(
        g12["seed_completion"],
        "terminal G-12 seed completion",
        full=True,
        expected_path=seed_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g12["seed_summary"],
        "terminal G-12 seed summary",
        full=True,
        expected_path=seed_root / "runs.tsv",
        maximum_bytes=1024 * 1024,
    )
    seed_logs = g12["seed_run_logs"]
    seed_names = tuple(f"seed-{ordinal:02d}.log" for ordinal in range(10))
    if not isinstance(seed_logs, list) or len(seed_logs) != len(seed_names):
        raise BootstrapError("terminal G-12 seed-log inventory is incomplete")
    for index, (record, filename) in enumerate(zip(seed_logs, seed_names)):
        capture_artifact(
            record,
            f"terminal G-12 seed log {index}",
            full=True,
            expected_path=seed_root / filename,
            maximum_bytes=16 * 1024 * 1024,
        )
    capture_artifact(
        g12["fault_soak_completion"],
        "terminal G-12 fault-soak completion",
        full=True,
        expected_path=soak_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g12["fault_soak_log"],
        "terminal G-12 fault-soak log",
        full=True,
        expected_path=soak_root / "fault-soak.log",
        maximum_bytes=16 * 1024 * 1024,
    )
    require_inventory(seed_root, {"COMPLETED.tsv", "runs.tsv", *seed_names}, "terminal G-12 seed directory")
    require_inventory(soak_root, {"COMPLETED.tsv", "fault-soak.log"}, "terminal G-12 soak directory")

    return artifact_snapshots, directory_snapshots


def _retained_release_layout(
    evidence: Path,
    evidence_fd: int,
    *,
    candidate: Path | None = None,
    authenticated_environment: dict[str, str] | None = None,
    expected_receipt: dict[str, Any] | None = None,
) -> tuple[Path, Path, Path, FileSnapshot | None, FileSnapshot | None, FileSnapshot | None]:
    """Authenticate the outer-published retained tree through held directories."""

    result_name = "release-runner-result.json"
    try:
        os.stat(result_name, dir_fd=evidence_fd, follow_symlinks=False)
    except FileNotFoundError:
        release_runner = evidence / "release-runner"
        return (
            release_runner,
            release_runner / "output" / "release" / "RELEASE_COMPLETED.json",
            release_runner / "sealed-identity.json",
            None,
            None,
            None,
        )
    except OSError as error:
        raise BootstrapError("protected outer release result is unavailable") from error
    result_path = evidence / result_name
    result_snapshot = _read_file_at(
        evidence_fd,
        result_name,
        result_path,
        "protected outer release result",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if (
        result_snapshot.mode != _DATA_MODE
        or result_snapshot.owner != os.getuid()
        or result_snapshot.nlink != 1
    ):
        raise BootstrapError("protected outer release result metadata is not exact")
    result = _require_exact_json_fields(
        _parse_canonical_json(result_snapshot, "protected outer release result"),
        {
            "format", "schema_version", "invocation_archive_id",
            "source_archive_id",
            "source_manifest_sha256", "sealed_identity", "receipt", "inventory",
            "receipt_validation",
        },
        "protected outer release result",
    )
    if (
        result["format"] != "iroha-sumeragi-v2-retained-release-evidence"
        or type(result["schema_version"]) is not int
        or result["schema_version"] != 2
        or result["invocation_archive_id"] != "release-retained.invocation.v1"
        or result["source_archive_id"] != "release-retained.source.v1"
        or not isinstance(result["source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(result["source_manifest_sha256"]) is None
    ):
        raise BootstrapError("protected outer release result schema is not exact")

    private_name = "release-runner-private-provenance.json"
    private_path = evidence / private_name
    private_snapshot = _read_file_at(
        evidence_fd,
        private_name,
        private_path,
        "bootstrap-private retained provenance",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if (
        private_snapshot.mode != _DATA_MODE
        or private_snapshot.owner != os.getuid()
        or private_snapshot.nlink != 1
    ):
        raise BootstrapError(
            "bootstrap-private retained provenance metadata is not exact"
        )
    private = _require_exact_json_fields(
        _parse_canonical_json(
            private_snapshot, "bootstrap-private retained provenance"
        ),
        {"format", "schema_version", "invocation_root", "source_root", "artifacts"},
        "bootstrap-private retained provenance",
    )
    if (
        private["format"]
        != "iroha-sumeragi-v2-bootstrap-private-retained-provenance"
        or type(private["schema_version"]) is not int
        or private["schema_version"] != 1
        or not isinstance(private["invocation_root"], str)
        or not isinstance(private["source_root"], str)
    ):
        raise BootstrapError(
            "bootstrap-private retained provenance schema is not exact"
        )
    release_runner = _absolute_resolved_existing(
        Path(private["invocation_root"]), "retained release evidence root"
    )
    source = _absolute_resolved_existing(
        Path(private["source_root"]), "retained sealed source"
    )
    if (
        source != release_runner / "source"
        or _inside(release_runner, evidence)
        or _inside(evidence, release_runner)
    ):
        raise BootstrapError("retained release evidence is not an exact external root")

    root_snapshot = _private_directory_snapshot(
        release_runner, "retained release evidence root"
    )
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        root_fd = os.open(release_runner, directory_flags)
    except OSError as error:
        raise BootstrapError("retained release evidence root could not be held") from error
    root_opened = os.fstat(root_fd)
    if (
        not stat.S_ISDIR(root_opened.st_mode)
        or (root_opened.st_dev, root_opened.st_ino)
        != (root_snapshot.device, root_snapshot.inode)
        or stat.S_IMODE(root_opened.st_mode) != root_snapshot.mode
        or root_opened.st_uid != root_snapshot.owner
    ):
        os.close(root_fd)
        raise BootstrapError("retained release evidence root changed while opened")

    protected_specs = {
        "receipt": (
            "RELEASE_COMPLETED.json",
            _MAX_TERMINAL_RECEIPT_BYTES,
            "output/release/RELEASE_COMPLETED.json",
            "release-terminal.receipt.v1",
        ),
        "sealed_identity": (
            "sealed-identity.json", _MAX_IDENTITY_BYTES, "sealed-identity.json",
            "release-retained.identity.v1",
        ),
        "inventory": (
            "release-retained-inventory.json",
            256 * 1024 * 1024,
            "retained-evidence-inventory.json",
            "release-retained.inventory.v2",
        ),
        "receipt_validation": (
            "receipt-validation-ack.json",
            _MAX_EVIDENCE_BYTES,
            "receipt-validation-ack.json",
            "release-retained.receipt-validation-ack.v3",
        ),
    }
    private_artifacts = _require_exact_json_fields(
        private["artifacts"], set(protected_specs),
        "bootstrap-private retained artifacts",
    )
    protected: dict[str, FileSnapshot] = {}
    binding_records: dict[str, dict[str, Any]] = {}
    try:
        for field, (filename, maximum, relative, archive_id) in protected_specs.items():
            record = _require_exact_json_fields(
                result[field], {"archive_id", "mode", "sha256", "size_bytes"},
                f"retained {field} binding",
            )
            expected_local = release_runner.joinpath(*relative.split("/"))
            expected_protected = evidence / filename
            private_record = _require_exact_json_fields(
                private_artifacts[field], {"path", "protected_path"},
                f"bootstrap-private retained {field}",
            )
            if (
                private_record != {
                    "path": str(expected_local),
                    "protected_path": str(expected_protected),
                }
                or record["archive_id"] != archive_id
                or record["mode"] != "0400"
                or not isinstance(record["sha256"], str)
                or _DIGEST_RE.fullmatch(record["sha256"]) is None
                or type(record["size_bytes"]) is not int
                or record["size_bytes"] < 0
            ):
                raise BootstrapError(f"retained {field} binding is not exact")
            copied = _read_file_at(
                evidence_fd,
                filename,
                expected_protected,
                f"protected retained {field}",
                maximum_bytes=maximum,
            )
            if (
                copied.sha256 != record["sha256"]
                or copied.size != record["size_bytes"]
                or copied.mode != _DATA_MODE
                or copied.owner != os.getuid()
                or copied.nlink != 1
            ):
                raise BootstrapError(f"retained {field} protected copy is not exact")
            protected[field] = copied
            binding_records[field] = record

        inventory_local = _read_file_at(
            root_fd,
            "retained-evidence-inventory.json",
            release_runner / "retained-evidence-inventory.json",
            "retained evidence inventory",
            maximum_bytes=256 * 1024 * 1024,
        )
        if (
            inventory_local.sha256 != protected["inventory"].sha256
            or inventory_local.size != protected["inventory"].size
            or inventory_local.owner != os.getuid()
            or inventory_local.nlink != 1
            or inventory_local.mode != _DATA_MODE
        ):
            raise BootstrapError("retained inventory local and protected copies disagree")
        inventory_snapshot = protected["inventory"]
        inventory = _require_exact_json_fields(
            _parse_canonical_json(inventory_snapshot, "retained release inventory"),
            {
                "format", "schema_version", "invocation_archive_id",
                "source_archive_id",
                "source_manifest_sha256", "record_count", "file_bytes", "records",
            },
            "retained release inventory",
        )
        records = inventory["records"]
        if (
            inventory["format"] != result["format"]
            or type(inventory["schema_version"]) is not int
            or inventory["schema_version"] != 2
            or inventory["invocation_archive_id"]
            != result["invocation_archive_id"]
            or inventory["source_archive_id"] != result["source_archive_id"]
            or inventory["source_manifest_sha256"]
            != result["source_manifest_sha256"]
            or type(records) is not list
            or type(inventory["record_count"]) is not int
            or inventory["record_count"] != len(records)
            or not 0 <= inventory["record_count"] <= _MAX_RETAINED_RECORDS
            or type(inventory["file_bytes"]) is not int
            or not 0 <= inventory["file_bytes"] <= _MAX_RETAINED_TOTAL_BYTES
        ):
            raise BootstrapError("retained release inventory contract is not exact")
        for index, record in enumerate(records):
            if not isinstance(record, dict):
                raise BootstrapError(f"retained release inventory record {index} is not exact")
            kind = record.get("kind")
            expected_keys = (
                {"path", "kind", "mode"}
                if kind == "directory"
                else {"path", "kind", "mode", "size", "sha256"}
                if kind == "file"
                else set()
            )
            relative = record.get("path")
            mode = record.get("mode")
            if (
                set(record) != expected_keys
                or not isinstance(relative, str)
                or not relative
                or relative.startswith("/")
                or any(part in {"", ".", ".."} for part in relative.split("/"))
                or len(relative.encode()) > _MAX_RETAINED_PATH_BYTES
                or len(relative.split("/")) > _MAX_RETAINED_DEPTH
                or not isinstance(mode, str)
                or re.fullmatch(r"[0-7]{4}", mode) is None
                or (
                    kind == "file"
                    and (
                        type(record.get("size")) is not int
                        or not 0 <= record["size"] <= _MAX_RETAINED_FILE_BYTES
                        or not isinstance(record.get("sha256"), str)
                        or _DIGEST_RE.fullmatch(record["sha256"]) is None
                    )
                )
            ):
                raise BootstrapError(f"retained release inventory record {index} is not exact")

        observed: list[dict[str, Any]] = []
        local_files: dict[str, LargeFileSnapshot] = {}
        file_bytes = 0
        record_count = 0
        stable_directory_fields = (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_mtime_ns", "st_ctime_ns",
        )

        def directory_names(descriptor: int, label: str) -> tuple[str, ...]:
            names: list[str] = []
            try:
                with os.scandir(descriptor) as entries:
                    for entry in entries:
                        names.append(entry.name)
                        if len(names) > _MAX_RETAINED_RECORDS:
                            raise BootstrapError(f"{label} contains too many entries")
            except OSError as error:
                raise BootstrapError(f"{label} could not be enumerated") from error
            return tuple(sorted(names))

        def walk(descriptor: int, relative_directory: str) -> None:
            nonlocal file_bytes, record_count
            before = os.fstat(descriptor)
            names = directory_names(
                descriptor, f"retained {relative_directory or '.'}"
            )
            for name in names:
                if not relative_directory and name in {
                    "source", "retained-evidence-inventory.json",
                }:
                    continue
                if name.startswith((".owned-quarantine.", ".owned-quiescent.")):
                    raise BootstrapError("retained release contains a cleanup quarantine")
                relative = name if not relative_directory else f"{relative_directory}/{name}"
                if (
                    len(relative.encode()) > _MAX_RETAINED_PATH_BYTES
                    or len(relative.split("/")) > _MAX_RETAINED_DEPTH
                ):
                    raise BootstrapError("retained release path exceeds its bound")
                try:
                    metadata = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                except OSError as error:
                    raise BootstrapError(f"retained release entry is unavailable: {relative}") from error
                record_count += 1
                if (
                    record_count > _MAX_RETAINED_RECORDS
                    or metadata.st_uid != os.getuid()
                    or stat.S_ISLNK(metadata.st_mode)
                ):
                    raise BootstrapError(f"retained release entry is unsafe: {relative}")
                path = release_runner.joinpath(*relative.split("/"))
                if stat.S_ISDIR(metadata.st_mode):
                    observed.append({
                        "path": relative,
                        "kind": "directory",
                        "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
                    })
                    try:
                        child = os.open(name, directory_flags, dir_fd=descriptor)
                    except OSError as error:
                        raise BootstrapError(f"retained directory could not be opened: {relative}") from error
                    try:
                        opened = os.fstat(child)
                        if not stat.S_ISDIR(opened.st_mode) or any(
                            getattr(opened, field) != getattr(metadata, field)
                            for field in stable_directory_fields
                        ):
                            raise BootstrapError(f"retained directory changed: {relative}")
                        walk(child, relative)
                    finally:
                        os.close(child)
                    current = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                    if any(
                        getattr(current, field) != getattr(metadata, field)
                        for field in stable_directory_fields
                    ):
                        raise BootstrapError(f"retained directory changed: {relative}")
                elif stat.S_ISREG(metadata.st_mode):
                    if metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) & 0o022:
                        raise BootstrapError(f"retained release file is unsafe: {relative}")
                    snapshot = _capture_large_file_at(
                        descriptor,
                        name,
                        path,
                        f"retained release entry {relative}",
                        maximum_bytes=_MAX_RETAINED_FILE_BYTES,
                    )
                    file_bytes += snapshot.size
                    if file_bytes > _MAX_RETAINED_TOTAL_BYTES:
                        raise BootstrapError("retained release files exceed their total bound")
                    observed.append({
                        "path": relative,
                        "kind": "file",
                        "mode": f"{snapshot.mode:04o}",
                        "size": snapshot.size,
                        "sha256": snapshot.sha256,
                    })
                    local_files[relative] = snapshot
                else:
                    raise BootstrapError(f"retained release entry is special: {relative}")
            after = os.fstat(descriptor)
            if names != directory_names(
                descriptor, f"retained {relative_directory or '.'}"
            ) or any(
                getattr(after, field) != getattr(before, field)
                for field in stable_directory_fields
            ):
                raise BootstrapError(
                    f"retained directory changed while read: {relative_directory or '.'}"
                )

        walk(root_fd, "")
        if (
            observed != records
            or inventory["file_bytes"] != file_bytes
            or inventory["record_count"] != record_count
            or _private_directory_snapshot(
                release_runner, "retained release evidence root"
            ) != root_snapshot
        ):
            raise BootstrapError("retained release exact inventory changed")
        for field, (_, _, relative, _) in protected_specs.items():
            if field == "inventory":
                local_digest = inventory_local.sha256
                local_size = inventory_local.size
            else:
                local = local_files.get(relative)
                if local is None:
                    raise BootstrapError(f"retained {field} is absent from the exact tree")
                local_digest = local.sha256
                local_size = local.size
            record = binding_records[field]
            if (
                record["sha256"] != local_digest
                or record["size_bytes"] != local_size
            ):
                raise BootstrapError(f"retained {field} local binding changed")
    finally:
        os.close(root_fd)

    ack_snapshot = protected["receipt_validation"]
    ack = _require_exact_json_fields(
        _parse_canonical_json(ack_snapshot, "receipt validation acknowledgment"),
        {"format", "schema_version", "profile", "sealed_source", "receipt", "validator", "invocation", "exit_status", "stdout", "stderr"},
        "receipt validation acknowledgment",
    )
    receipt_record = _require_exact_json_fields(
        ack["receipt"], {"archive_id", "mode", "sha256", "size_bytes"},
        "ack receipt",
    )
    source_record = _require_exact_json_fields(
        ack["sealed_source"], {"archive_id", "manifest_sha256"},
        "ack sealed source",
    )
    validator_record = _require_exact_json_fields(
        ack["validator"],
        {"archive_id", "sha256", "bootstrap_completion_sha256"},
        "ack validator",
    )
    stdout_record = _require_exact_json_fields(
        ack["stdout"], {"sha256", "size_bytes"}, "ack stdout"
    )
    stderr_record = _require_exact_json_fields(
        ack["stderr"], {"sha256", "size_bytes"}, "ack stderr"
    )
    if expected_receipt is None:
        try:
            expected_receipt = json.loads(protected["receipt"].data)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BootstrapError("protected terminal receipt is malformed") from error
        if not isinstance(expected_receipt, dict):
            raise BootstrapError("protected terminal receipt is malformed")
    invocation_record = ack["invocation"]
    if candidate is None or authenticated_environment is None:
        raise BootstrapError(
            "retained release validation lacks private invocation provenance"
        )
    local_receipt_path = (
        release_runner / "output" / "release" / "RELEASE_COMPLETED.json"
    )
    _validate_validator_invocation(
        invocation_record,
        expected_values=_terminal_validator_invocation_values(
            expected_receipt,
            evidence=evidence,
            candidate=candidate,
            release_runner=release_runner,
            receipt_path=local_receipt_path,
            acknowledgment_path=release_runner / "receipt-validation-ack.json",
            source_manifest_sha256=result["source_manifest_sha256"],
            authenticated_environment=authenticated_environment,
        ),
    )
    expected_stdout = (
        f"Sumeragi v2 aggregate release receipt verified: {local_receipt_path}\n"
    ).encode()
    validator_snapshot = _read_file_at(
        evidence_fd,
        "validate-receipt.py",
        evidence / "validate-receipt.py",
        "archived receipt validator",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    if (
        ack["format"] != "iroha-sumeragi-v2-receipt-validation-ack"
        or type(ack["schema_version"]) is not int or ack["schema_version"] != 3
        or ack["profile"] != "release"
        or source_record != {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": result["source_manifest_sha256"],
        }
        or receipt_record != {
            "archive_id": "release-terminal.receipt.v1",
            "mode": f"{protected['receipt'].mode:04o}",
            "sha256": protected["receipt"].sha256,
            "size_bytes": protected["receipt"].size,
        }
        or validator_record["archive_id"]
        != "release-bootstrap.receipt-validator.v1"
        or validator_record["sha256"] != validator_snapshot.sha256
        or not isinstance(validator_record["bootstrap_completion_sha256"], str)
        or _DIGEST_RE.fullmatch(validator_record["bootstrap_completion_sha256"]) is None
        or type(receipt_record["size_bytes"]) is not int
        or type(ack["exit_status"]) is not int or ack["exit_status"] != 0
        or type(stdout_record["size_bytes"]) is not int
        or type(stderr_record["size_bytes"]) is not int
        or stdout_record != {
            "sha256": hashlib.sha256(expected_stdout).hexdigest(),
            "size_bytes": len(expected_stdout),
        }
        or stderr_record != {
            "sha256": hashlib.sha256(b"").hexdigest(),
            "size_bytes": 0,
        }
    ):
        raise BootstrapError("receipt validation acknowledgment contract is not exact")
    _remove_completed_runner_log(
        evidence_fd,
        private_snapshot,
        "bootstrap-private retained provenance",
    )
    try:
        os.stat(private_name, dir_fd=evidence_fd, follow_symlinks=False)
    except FileNotFoundError:
        pass
    except OSError as error:
        raise BootstrapError(
            "bootstrap-private retained provenance cleanup is indeterminate"
        ) from error
    else:
        raise BootstrapError(
            "bootstrap-private retained provenance survived authentication"
        )
    return release_runner, protected["receipt"].path, protected["sealed_identity"].path, result_snapshot, inventory_snapshot, ack_snapshot


def _receipt_validation_failure(
    evidence: Path,
    evidence_fd: int,
    identity: dict[str, Any],
    identity_snapshot: FileSnapshot,
    bootstrap_marker: FileSnapshot,
    protected_validator: FileSnapshot,
) -> tuple[FileSnapshot, dict[str, FileSnapshot]]:
    """Authenticate the bounded failure record published after root cleanup."""

    marker_snapshot = _read_file_at(
        evidence_fd,
        "RECEIPT_VALIDATION_FAILED.json",
        evidence / "RECEIPT_VALIDATION_FAILED.json",
        "receipt validation failure marker",
        maximum_bytes=_MAX_VALIDATOR_FAILURE_MARKER_BYTES,
    )
    marker = _require_exact_json_fields(
        _parse_canonical_json(marker_snapshot, "receipt validation failure marker"),
        {
            "format", "schema_version", "result", "stage", "profile",
            "bootstrap_completion_sha256", "candidate_identity",
            "sealed_source_manifest_sha256", "receipt", "validator", "argv",
            "diagnostics", "invocation_cleanup",
        },
        "receipt validation failure marker",
    )
    candidate = _require_exact_json_fields(
        marker["candidate_identity"], {"sha256", "head_commit", "head_tree"},
        "receipt validation failure candidate identity",
    )
    receipt = _require_exact_json_fields(
        marker["receipt"], {"disclosure", "sha256", "size_bytes"},
        "receipt validation failure receipt",
    )
    validator = _require_exact_json_fields(
        marker["validator"], {"archive_name", "sha256", "exit_status"},
        "receipt validation failure validator",
    )
    argv = _require_exact_json_fields(
        marker["argv"],
        {
            "profile",
            "python_flags",
            "validator",
            "operation",
            "invocation_binding",
        },
        "receipt validation failure argv",
    )
    diagnostics = _require_exact_json_fields(
        marker["diagnostics"], {"stdout", "stderr"},
        "receipt validation failure diagnostics",
    )
    if (
        marker["format"] != "iroha-sumeragi-v2-receipt-validation-failure"
        or type(marker["schema_version"]) is not int
        or marker["schema_version"] != 2
        or marker["result"] != "release-failed"
        or marker["stage"] != "protected-receipt-validation"
        or marker["profile"] != "release"
        or marker["bootstrap_completion_sha256"] != bootstrap_marker.sha256
        or candidate != {
            "sha256": identity_snapshot.sha256,
            "head_commit": identity["head_commit"],
            "head_tree": identity["head_tree"],
        }
        or not isinstance(marker["sealed_source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(marker["sealed_source_manifest_sha256"]) is None
        or receipt.get("disclosure") != "unverified-no-retention"
        or not isinstance(receipt.get("sha256"), str)
        or _DIGEST_RE.fullmatch(receipt["sha256"]) is None
        or type(receipt.get("size_bytes")) is not int
        or receipt["size_bytes"] < 0
        or validator.get("archive_name") != "validate-receipt.py"
        or validator.get("sha256") != protected_validator.sha256
        or type(validator.get("exit_status")) is not int
        or not 1 <= validator["exit_status"] <= 255
        or argv != {
            "profile": "release",
            "python_flags": ["-I", "-S"],
            "validator": "protected:validate-receipt.py",
            "operation": "verify-existing-and-ack",
            "invocation_binding": "not-published-validation-failed",
        }
        or marker["invocation_cleanup"] != "complete"
    ):
        raise BootstrapError("receipt validation failure marker contract is not exact")

    streams: dict[str, FileSnapshot] = {}
    for name in ("stdout", "stderr"):
        record = _require_exact_json_fields(
            diagnostics[name],
            {
                "name", "sha256", "captured_size_bytes", "observed_size_bytes",
                "truncated", "mode",
            },
            f"receipt validator {name} diagnostic",
        )
        expected_name = f"receipt-validator-failure.{name}"
        if (
            record.get("name") != expected_name
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
            or type(record.get("captured_size_bytes")) is not int
            or not 0 <= record["captured_size_bytes"] <= _MAX_VALIDATOR_DIAGNOSTIC_BYTES
            or type(record.get("observed_size_bytes")) is not int
            or record["observed_size_bytes"] < record["captured_size_bytes"]
            or type(record.get("truncated")) is not bool
            or record["truncated"]
            != (record["observed_size_bytes"] > record["captured_size_bytes"])
            or record.get("mode") != "0400"
        ):
            raise BootstrapError(f"receipt validator {name} diagnostic contract is not exact")
        snapshot = _read_file_at(
            evidence_fd,
            expected_name,
            evidence / expected_name,
            f"receipt validator {name} diagnostic",
            maximum_bytes=_MAX_VALIDATOR_DIAGNOSTIC_BYTES,
        )
        if (
            snapshot.sha256 != record["sha256"]
            or snapshot.size != record["captured_size_bytes"]
            or snapshot.mode != _DATA_MODE
            or snapshot.owner != os.getuid()
            or snapshot.nlink != 1
        ):
            raise BootstrapError(f"receipt validator {name} diagnostic changed")
        streams[name] = snapshot
    for name in (
        "BOOTSTRAP_RELEASE_COMPLETED.json", "RELEASE_COMPLETED.json",
        "receipt-validation-ack.json", "release-retained-inventory.json",
        "release-runner-private-provenance.json",
        "release-runner-result.json", "sealed-identity.json",
    ):
        try:
            os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        except FileNotFoundError:
            continue
        except OSError as error:
            raise BootstrapError("could not inspect failure-only evidence") from error
        raise BootstrapError("receipt validation failure retained success evidence")
    return marker_snapshot, streams


def _remove_completed_runner_log(
    evidence_fd: int, snapshot: LargeFileSnapshot, label: str
) -> None:
    """Remove a completed bootstrap-owned runner log at a quiescent boundary."""

    try:
        current = os.stat(snapshot.path.name, dir_fd=evidence_fd, follow_symlinks=False)
    except OSError as error:
        raise BootstrapError(f"{label} became unavailable before cleanup") from error
    if (
        not stat.S_ISREG(current.st_mode)
        or (current.st_dev, current.st_ino) != (snapshot.device, snapshot.inode)
        or current.st_uid != snapshot.owner
        or current.st_nlink != snapshot.nlink
        or current.st_size != snapshot.size
        or stat.S_IMODE(current.st_mode) != snapshot.mode
    ):
        raise BootstrapError(f"{label} changed before cleanup")
    try:
        os.unlink(snapshot.path.name, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
    except OSError as error:
        raise BootstrapError(f"could not remove {label}") from error


def _prune_receipt_validation_failure(
    evidence: Path,
    evidence_fd: int,
    marker: FileSnapshot,
    streams: dict[str, FileSnapshot],
) -> None:
    """Retain only authenticated bounded bootstrap-owned failure diagnostics."""

    retained = {
        marker.path.name: marker,
        **{snapshot.path.name: snapshot for snapshot in streams.values()},
    }
    try:
        with os.scandir(evidence_fd) as entries:
            names = tuple(sorted(entry.name for entry in entries))
    except OSError as error:
        raise BootstrapError("could not enumerate failure-only evidence") from error
    for name in names:
        if name in retained:
            continue
        try:
            metadata = os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        except OSError as error:
            raise BootstrapError("failure-only evidence entry became unavailable") from error
        if metadata.st_uid != os.getuid():
            raise BootstrapError("failure-only cleanup refuses an unowned entry")
        path = evidence / name
        if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
            _cleanup(path)
            if os.path.lexists(path):
                raise BootstrapError("failure-only directory cleanup did not complete")
            continue
        if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode)):
            raise BootstrapError("failure-only cleanup refuses a special entry")
        current = os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
            raise BootstrapError("failure-only cleanup entry was replaced")
        try:
            os.unlink(name, dir_fd=evidence_fd)
        except OSError as error:
            raise BootstrapError("failure-only evidence could not be pruned") from error
    os.fsync(evidence_fd)
    try:
        observed = set(os.listdir(evidence_fd))
    except OSError as error:
        raise BootstrapError("failure-only retained inventory is unavailable") from error
    if observed != set(retained):
        raise BootstrapError("failure-only retained inventory is not exact")
    for name, snapshot in retained.items():
        _require_unchanged(
            snapshot,
            f"retained failure diagnostic {name}",
            maximum_bytes=max(snapshot.size, 1),
        )


def _validate_terminal_receipt(
    *,
    evidence: Path,
    candidate: Path,
    bootstrap_marker: FileSnapshot,
    bootstrap_sha256: str,
    identity_snapshot: FileSnapshot,
    identity: dict[str, Any],
    runner_snapshot: FileSnapshot,
    runner_record: dict[str, Any],
    protected: dict[str, FileSnapshot],
    identity_attestation: dict[str, Any],
    expected_signer_fingerprint: str,
    authenticated_environment: dict[str, str],
    release_runner: Path | None = None,
    receipt_path: Path | None = None,
) -> tuple[
    FileSnapshot,
    dict[str, Any],
    list[LargeFileSnapshot],
    list[DirectorySnapshot],
]:
    release_runner = release_runner or evidence / "release-runner"
    output = release_runner / "output"
    release = output / "release"
    directories = [
        _private_directory_snapshot(release_runner, "release-runner directory"),
        _private_directory_snapshot(output, "release output directory"),
        _private_directory_snapshot(release, "terminal receipt directory"),
    ]
    receipt_path = receipt_path or release / "RELEASE_COMPLETED.json"
    receipt_snapshot = _read_file(
        receipt_path,
        "terminal release receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    if (
        receipt_snapshot.mode != _DATA_MODE
        or receipt_snapshot.owner != os.getuid()
        or receipt_snapshot.nlink != 1
    ):
        raise BootstrapError(
            "terminal release receipt must be owner-owned, single-link, and mode 0400"
        )
    receipt = _parse_canonical_json(receipt_snapshot, "terminal release receipt")
    _require_exact_json_fields(
        receipt,
        {"schema_version", "protocol", "result", "identity", "authentication", "evidence"},
        "terminal release receipt",
    )
    if (
        type(receipt["schema_version"]) is not int
        or receipt["schema_version"] != 1
        or receipt["protocol"] != "sumeragi-v2"
        or receipt["result"] != "release-complete"
    ):
        raise BootstrapError("terminal release receipt does not record release completion")
    receipt_evidence = _require_exact_json_fields(
        receipt["evidence"], _TERMINAL_EVIDENCE_KEYS, "terminal release evidence"
    )
    bootstrap_evidence = _require_exact_json_fields(
        receipt_evidence["bootstrap"],
        {
            "completion",
            "candidate_identity",
            "runner",
            "candidate_cargo_lock",
            "trusted_inputs",
            "identity_verification",
            "runner_tools",
        },
        "terminal bootstrap evidence",
    )
    if (
        not isinstance(bootstrap_evidence["trusted_inputs"], dict)
        or set(bootstrap_evidence["trusted_inputs"]) != set(protected)
        or not isinstance(bootstrap_evidence["identity_verification"], dict)
        or not isinstance(bootstrap_evidence["runner_tools"], dict)
        or set(bootstrap_evidence["runner_tools"])
        != set(runner_record["tools"])
    ):
        raise BootstrapError("terminal bootstrap evidence inventory is not exact")
    for label in (
        "corridor_completion",
        "formal_completion",
        "formal_verus_evidence",
        "formal_verus_log",
        "formal_cross_tool_evidence",
        "seed_matrix_completion",
        "chaos_completion",
        "taira_completion",
    ):
        record = receipt_evidence[label]
        if (
            not isinstance(record, dict)
            or not isinstance(record.get("path"), str)
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
        ):
            raise BootstrapError(f"terminal release evidence {label} is malformed")

    receipt_identity = _require_exact_json_fields(
        receipt["identity"],
        {
            "head_commit",
            "head_tree",
            "index_tree",
            "cargo_lock_sha256",
            "candidate_source_manifest_sha256",
            "sealed_source_manifest_sha256",
        },
        "terminal release receipt identity",
    )
    expected_identity = {
        "head_commit": identity["head_commit"],
        "head_tree": identity["head_tree"],
        "index_tree": identity["index_tree"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "candidate_source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
    }
    if any(receipt_identity.get(key) != value for key, value in expected_identity.items()):
        raise BootstrapError("terminal release receipt has the wrong candidate identity")
    if (
        not isinstance(receipt_identity["sealed_source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(receipt_identity["sealed_source_manifest_sha256"])
        is None
    ):
        raise BootstrapError("terminal release receipt has an invalid sealed-source digest")
    terminal_artifacts, terminal_evidence_directories = (
        _validate_terminal_release_evidence(
            receipt_evidence=receipt_evidence,
            evidence=output,
            release_root=release_runner / "source",
            receipt_identity=receipt_identity,
            runner_record=runner_record,
            authenticated_environment=authenticated_environment,
        )
    )

    authentication = _require_exact_json_fields(
        receipt["authentication"],
        {"schema_version", "bootstrap", "release_identity"},
        "terminal release authentication",
    )
    if type(authentication["schema_version"]) is not int or authentication["schema_version"] != 2:
        raise BootstrapError("terminal release authentication has the wrong schema version")
    bootstrap = _require_exact_json_fields(
        authentication["bootstrap"],
        {
            "schema_version",
            "completion_sha256",
            "frozen_bootstrap_sha256",
            "candidate_identity_sha256",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "runner",
            "signer_fingerprint",
            "allowed_signers_principal",
            "trusted_input_digests",
            "trusted_input_archives",
        },
        "terminal release bootstrap authentication",
    )
    if (
        type(bootstrap["schema_version"]) is not int
        or bootstrap["schema_version"] != 2
        or bootstrap["completion_sha256"] != bootstrap_marker.sha256
        or bootstrap["frozen_bootstrap_sha256"] != bootstrap_sha256
        or bootstrap["candidate_identity_sha256"] != identity_snapshot.sha256
        or bootstrap["candidate_commit_oid"] != identity["head_commit"]
        or bootstrap["candidate_tree_oid"] != identity["head_tree"]
    ):
        raise BootstrapError("terminal release receipt has the wrong bootstrap binding")
    expected_trusted_digests = {
        label: snapshot.sha256 for label, snapshot in sorted(protected.items())
    }
    if bootstrap["trusted_input_digests"] != expected_trusted_digests:
        raise BootstrapError("terminal release receipt has wrong trusted-input digests")
    marker_json = _parse_canonical_json(bootstrap_marker, "bootstrap completion marker")
    marker_trusted = marker_json.get("trusted_inputs")
    if not isinstance(marker_trusted, dict):
        raise BootstrapError("bootstrap marker trusted-input inventory is malformed")
    expected_trusted_archives = {
        label: {
            key: record[key]
            for key in ("archive_id", "mode", "sha256", "size_bytes")
        }
        for label, record in sorted(marker_trusted.items())
        if isinstance(record, dict)
    }
    if bootstrap["trusted_input_archives"] != expected_trusted_archives:
        raise BootstrapError("terminal release receipt has wrong trusted-input archives")
    if bootstrap["signer_fingerprint"] != expected_signer_fingerprint:
        raise BootstrapError("terminal release receipt has the wrong protected signer")
    if runner_snapshot.path != candidate / "scripts" / "run_sumeragi_v2_release_gates.sh":
        raise BootstrapError("terminal release receipt has the wrong runner root binding")
    receipt_runner = _require_exact_json_fields(
        bootstrap["runner"],
        {
            "archive_id",
            "sha256",
            "mode",
            "invocation",
            "closed_path_resolution",
            "output",
            "tool_directory",
            "tools",
            "environment_sha256",
            "self_digest_environment_variables",
        },
        "terminal release bootstrap runner",
    )
    expected_runner = {
        "archive_id": runner_record["archive_id"],
        "sha256": runner_snapshot.sha256,
        "mode": f"{runner_snapshot.mode:04o}",
        "invocation": runner_record["invocation"],
        "closed_path_resolution": runner_record["closed_path_resolution"],
        "output": runner_record["output"],
        "tool_directory": runner_record["tool_directory"],
        "tools": runner_record["tools"],
        "environment_sha256": runner_record["environment_sha256"],
        "self_digest_environment_variables": runner_record[
            "self_digest_environment_variables"
        ],
    }
    if receipt_runner != expected_runner:
        raise BootstrapError("terminal release receipt has the wrong runner binding")
    release_identity = _require_exact_json_fields(
        authentication["release_identity"],
        {
            "schema_version",
            "signature_format",
            "verification_status",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
            "trust_policy",
            "replay",
        },
        "terminal release identity authentication",
    )
    expected_release_identity = {
        "schema_version": 1,
        "signature_format": "ssh",
        "verification_status": "G",
        "candidate_commit_oid": identity["head_commit"],
        "candidate_tree_oid": identity["head_tree"],
        "signer_fingerprint": bootstrap["signer_fingerprint"],
        "allowed_signers_principal": bootstrap["allowed_signers_principal"],
    }
    for field, expected in expected_release_identity.items():
        if (
            field == "schema_version"
            and type(release_identity[field]) is not int
        ) or release_identity[field] != expected:
            raise BootstrapError(
                f"terminal release receipt has the wrong release identity {field}"
            )
    expected_trust_policy = {
        "git_sha256": protected["git"].sha256,
        "ssh_keygen_sha256": protected["ssh_keygen"].sha256,
        "allowed_signers_sha256": protected["allowed_signers"].sha256,
        "revocation_sha256": protected["revocation"].sha256,
        "signer_fingerprint": expected_signer_fingerprint,
    }
    if (
        release_identity["primary_key_fingerprint"] != ""
        or release_identity["trust_policy"] != expected_trust_policy
        or not isinstance(release_identity["replay"], dict)
        or release_identity["replay"].get("performed") is not True
        or release_identity["replay"].get("archive_ids") != _IDENTITY_ARCHIVE_IDS
    ):
        raise BootstrapError("terminal release identity trust evidence is not exact")
    return (
        receipt_snapshot,
        receipt,
        terminal_artifacts,
        [*directories, *terminal_evidence_directories],
    )


def _fsync_file_snapshot(snapshot: FileSnapshot, label: str) -> None:
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(snapshot.path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            (opened.st_dev, opened.st_ino) != (snapshot.device, snapshot.inode)
            or stat.S_IMODE(opened.st_mode) != snapshot.mode
            or opened.st_uid != snapshot.owner
            or opened.st_nlink != snapshot.nlink
            or opened.st_size != snapshot.size
        ):
            raise BootstrapError(f"{label} changed before fsync")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )


def _validate_retained_source(
    *,
    evidence: Path,
    receipt: dict[str, Any],
    candidate_identity: dict[str, Any],
    python: Path,
    manifest_helper: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    release_runner: Path | None = None,
    sealed_identity_path: Path | None = None,
) -> tuple[FileSnapshot, dict[str, Any], DirectorySnapshot]:
    release_runner = release_runner or evidence / "release-runner"
    sealed_root = release_runner / "source"
    sealed_directory = _sealed_directory_snapshot(
        sealed_root, "retained sealed source root"
    )
    sealed_identity_snapshot = _read_file(
        sealed_identity_path or release_runner / "sealed-identity.json",
        "retained sealed identity",
        maximum_bytes=_MAX_IDENTITY_BYTES,
    )
    if (
        sealed_identity_snapshot.mode != _DATA_MODE
        or sealed_identity_snapshot.owner != os.getuid()
        or sealed_identity_snapshot.nlink != 1
    ):
        raise BootstrapError("retained sealed identity metadata is not exact")
    sealed_identity = _load_identity(sealed_identity_snapshot.data)
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if sealed_identity[field] != candidate_identity[field]:
            raise BootstrapError(
                f"retained sealed identity disagrees with candidate {field}"
            )
    receipt_identity = receipt["identity"]
    if receipt_identity["sealed_source_manifest_sha256"] != sealed_identity[
        "workspace_source_manifest_sha256"
    ]:
        raise BootstrapError("terminal receipt does not bind the retained sealed root")
    recomputed_bytes, recomputed_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if (
        recomputed_bytes != sealed_identity_snapshot.data
        or recomputed_identity != sealed_identity
    ):
        raise BootstrapError("retained sealed source does not reproduce its identity")
    _fsync_sealed_tree(sealed_root)
    _fsync_file_snapshot(sealed_identity_snapshot, "retained sealed identity")
    final_bytes, final_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if final_bytes != recomputed_bytes or final_identity != recomputed_identity:
        raise BootstrapError("retained sealed source changed during durability closure")
    _require_sealed_directory_unchanged(
        sealed_directory, "retained sealed source root"
    )
    return sealed_identity_snapshot, sealed_identity, sealed_directory


def _receipt_artifact_path(
    receipt: dict[str, Any], label: str, evidence: Path
) -> Path:
    return _receipt_nested_artifact_path(receipt, (label,), evidence)


def _receipt_nested_artifact_path(
    receipt: dict[str, Any], fields: tuple[str | int, ...], containment_root: Path
) -> Path:
    value: Any = receipt.get("evidence")
    rendered_fields: list[str] = []
    for field in fields:
        rendered_fields.append(str(field))
        try:
            value = value[field]
        except (KeyError, IndexError, TypeError) as error:
            raise BootstrapError(
                f"terminal receipt omits {'.'.join(rendered_fields)}"
            ) from error
    record = value
    label = ".".join(rendered_fields)
    if not isinstance(record, dict) or not {"path", "sha256"}.issubset(record):
        raise BootstrapError(f"terminal receipt omits {label}")
    rendered = record["path"]
    digest = record["sha256"]
    if not isinstance(rendered, str) or not isinstance(digest, str):
        raise BootstrapError(f"terminal receipt {label} path is not text")
    _require_digest(digest, f"terminal receipt {label} digest")
    path = _absolute_resolved_existing(Path(rendered), f"terminal receipt {label}")
    if not _inside(path, containment_root):
        raise BootstrapError(
            f"terminal receipt {label} escaped its authenticated containment root"
        )
    snapshot = _capture_large_file(path, f"terminal receipt {label}")
    if snapshot.sha256 != digest:
        raise BootstrapError(f"terminal receipt {label} digest changed")
    return path


def _receipt_scaling_manifest_path(
    receipt: dict[str, Any], authenticated_environment: dict[str, str]
) -> Path:
    bundle = receipt.get("evidence", {}).get("multilane_scaling_bundle")
    if (
        not isinstance(bundle, dict)
        or bundle.get("archive_id") != "release-scaling.bundle.v1"
        or not isinstance(bundle.get("files"), list)
    ):
        raise BootstrapError("terminal receipt omits its scaling bundle")
    matching = [
        record
        for record in bundle["files"]
        if isinstance(record, dict)
        and record.get("relative_path") == "scaling_evidence.json"
    ]
    if len(matching) != 1:
        raise BootstrapError("terminal receipt scaling manifest inventory is not exact")
    rendered = authenticated_environment.get(
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
    )
    if not isinstance(rendered, str):
        raise BootstrapError("authenticated runner omits its scaling manifest")
    path = _absolute_resolved_existing(
        Path(rendered), "authenticated scaling manifest"
    )
    snapshot = _capture_large_file(path, "authenticated scaling manifest")
    record = matching[0]
    if (
        record.get("archive_id")
        != "release-scaling.file.v1:scaling_evidence.json"
        or record.get("sha256") != snapshot.sha256
        or record.get("size_bytes") != snapshot.size
        or _terminal_mode(record.get("mode"), "terminal scaling manifest")
        != snapshot.mode
    ):
        raise BootstrapError("terminal scaling manifest authentication is not exact")
    return path


def _run_protected_receipt_validator(
    *,
    evidence: Path,
    candidate: Path,
    receipt: dict[str, Any],
    receipt_snapshot: FileSnapshot,
    sealed_identity_snapshot: FileSnapshot,
    sealed_root: Path,
    archives: dict[str, FileSnapshot],
    protected: dict[str, FileSnapshot],
    identity_snapshot: FileSnapshot,
    identity_outputs: dict[str, Path],
    bootstrap_marker: FileSnapshot,
    expected_signer_fingerprint: str,
    environment: dict[str, str],
    timeout_seconds: int,
) -> CommandResult:
    release_output = sealed_root.parent / "output"
    local_receipt_path = release_output / "release" / "RELEASE_COMPLETED.json"
    local_receipt = _read_file(
        local_receipt_path,
        "retained terminal receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    if (
        local_receipt.data != receipt_snapshot.data
        or local_receipt.sha256 != receipt_snapshot.sha256
        or local_receipt.size != receipt_snapshot.size
        or local_receipt.mode != receipt_snapshot.mode
    ):
        raise BootstrapError(
            "retained and protected terminal receipt copies disagree"
        )

    def release_artifact_root(fields: tuple[str | int, ...]) -> Path:
        value: Any = receipt.get("evidence")
        for field in fields:
            try:
                value = value[field]
            except (KeyError, IndexError, TypeError) as error:
                raise BootstrapError(
                    f"terminal receipt omits {'.'.join(map(str, fields))}"
                ) from error
        if not isinstance(value, dict) or not isinstance(value.get("path"), str):
            raise BootstrapError(
                f"terminal receipt omits {'.'.join(map(str, fields))}"
            )
        path = Path(value["path"])
        if not _inside(path, release_output):
            raise BootstrapError(
                "terminal receipt "
                f"{'.'.join(map(str, fields))} artifact is outside its exact "
                "release output"
            )
        return release_output

    def receipt_artifact(label: str) -> Path:
        return _receipt_artifact_path(
            receipt, label, release_artifact_root((label,))
        )

    def nested_artifact(fields: tuple[str | int, ...]) -> Path:
        return _receipt_nested_artifact_path(
            receipt, fields, release_artifact_root(fields)
        )
    scaling_digests: dict[str, str] = {}
    for field, environment_name in _SCALING_DIGEST_ENVIRONMENT.items():
        value = environment.get(environment_name)
        if not isinstance(value, str):
            raise BootstrapError(
                f"protected receipt validation lacks {environment_name}"
            )
        scaling_digests[field] = _require_digest(
            value, f"protected {environment_name}"
        )
    arguments = [
        "-I",
        "-S",
        str(archives["receipt_validator"].path),
        "--candidate-identity",
        str(identity_snapshot.path),
        "--sealed-identity",
        str(sealed_identity_snapshot.path),
        "--release-root",
        str(sealed_root),
        "--signature-attestation",
        str(identity_outputs["attestation"]),
        "--signature-transcript",
        str(identity_outputs["transcript"]),
        "--signature-raw-commit",
        str(identity_outputs["raw_commit"]),
        "--signature-cargo-lock",
        str(identity_outputs["cargo_lock"]),
        "--signature-allowed-signers",
        str(identity_outputs["allowed"]),
        "--signature-revocation",
        str(identity_outputs["revocation"]),
        "--signature-git",
        str(identity_outputs["git"]),
        "--signature-ssh-keygen",
        str(identity_outputs["ssh"]),
        "--expected-git-sha256",
        protected["git"].sha256,
        "--expected-ssh-keygen-sha256",
        protected["ssh_keygen"].sha256,
        "--expected-allowed-signers-sha256",
        protected["allowed_signers"].sha256,
        "--expected-revocation-sha256",
        protected["revocation"].sha256,
        "--expected-signer-fingerprint",
        expected_signer_fingerprint,
        "--bootstrap-completion",
        str(bootstrap_marker.path),
        "--bootstrap-evidence-dir",
        str(evidence),
        "--bootstrap-identity",
        str(identity_snapshot.path),
        "--bootstrap-attestation",
        str(identity_outputs["attestation"]),
        "--bootstrap-transcript",
        str(identity_outputs["transcript"]),
        "--expected-bootstrap-completion-sha256",
        bootstrap_marker.sha256,
        "--bootstrap-candidate-root",
        str(candidate),
        "--bootstrap-runner",
        str(candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"),
        "--corridor-completion",
        str(receipt_artifact("corridor_completion")),
        "--formal-completion",
        str(receipt_artifact("formal_completion")),
        "--seed-completion",
        str(receipt_artifact("seed_matrix_completion")),
        "--chaos-completion",
        str(receipt_artifact("chaos_completion")),
        "--taira-completion",
        str(receipt_artifact("taira_completion")),
        "--g4p-completion",
        str(
            nested_artifact(("g4p_multilane", "completion"))
        ),
        "--g12-seed-completion",
        str(
            nested_artifact(("g12_cross_dataspace", "seed_completion"))
        ),
        "--g12-fault-soak-completion",
        str(
            nested_artifact(
                ("g12_cross_dataspace", "fault_soak_completion")
            )
        ),
        "--scaling-evidence-manifest",
        str(_receipt_scaling_manifest_path(receipt, environment)),
        "--sdk-dependency-archive",
        str(sealed_root.parent / "sdk-dependency-bundle.tar"),
        "--sdk-dependency-input-inventory",
        str(sealed_root.parent / "sdk-dependency-input.json"),
        "--sdk-dependency-final-work-inventory",
        str(sealed_root.parent / "sdk-dependency-work-final.json"),
        "--expected-scaling-trial-harness-sha256",
        scaling_digests["trial_harness_sha256"],
        "--expected-scaling-configuration-sha256",
        scaling_digests["configuration_sha256"],
        "--expected-scaling-irohad-sha256",
        scaling_digests["irohad_sha256"],
        "--expected-scaling-iroha-cli-sha256",
        scaling_digests["iroha_cli_sha256"],
        "--repository-root",
        str(sealed_root),
        "--output",
        str(local_receipt.path),
        "--replay-existing",
    ]
    result = _run_bounded(
        archives["python"].path,
        arguments,
        cwd=sealed_root,
        environment={
            key: value
            for key, value in environment.items()
            if key
            not in {
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "PYTHONDONTWRITEBYTECODE",
                "PYTHONHASHSEED",
            }
        },
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    expected_stdout = (
        f"Sumeragi v2 aggregate release receipt replayed: "
        f"{local_receipt.path}\n"
    ).encode()
    if result.returncode != 0 or result.stdout != expected_stdout or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            "protected receipt validator rejected terminal receipt "
            f"with status {result.returncode}: {detail}"
        )
    _require_unchanged(
        local_receipt,
        "protected-validator retained terminal receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    return result


def _validate_command_record(
    record: Any,
    label: str,
    *,
    require_success: bool,
) -> None:
    expected_keys = {
        "argv",
        "replay_argv",
        "exit_status",
        "stdout_base64",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_base64",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    if not isinstance(record, dict) or set(record) != expected_keys:
        raise BootstrapError(f"identity transcript has invalid {label} evidence")
    for key in ("argv", "replay_argv"):
        value = record[key]
        if not isinstance(value, list) or not value or not all(
            isinstance(argument, str) for argument in value
        ):
            raise BootstrapError(f"identity transcript has invalid {label} {key}")
    exit_status = record["exit_status"]
    if type(exit_status) is not int or exit_status < 0:
        raise BootstrapError(f"identity transcript has invalid {label} exit status")
    if require_success and exit_status != 0:
        raise BootstrapError(f"identity transcript records failed {label}")
    for stream in ("stdout", "stderr"):
        encoded = record[f"{stream}_base64"]
        digest = record[f"{stream}_sha256"]
        size = record[f"{stream}_size_bytes"]
        if not isinstance(encoded, str) or not isinstance(digest, str):
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        if _DIGEST_RE.fullmatch(digest) is None or type(size) is not int or size < 0:
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise BootstrapError(
                f"identity transcript has invalid {label} {stream} encoding"
            ) from error
        if len(decoded) != size or hashlib.sha256(decoded).hexdigest() != digest:
            raise BootstrapError(
                f"identity transcript has inconsistent {label} {stream} evidence"
            )


def _validate_sanitized_operation(
    value: Any,
    label: str,
    *,
    operation_id: str,
    exit_status: int,
) -> None:
    expected = {
        "operation_id",
        "exit_status",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    if not isinstance(value, dict) or set(value) != expected:
        raise BootstrapError(
            f"identity transcript has invalid {label} operation"
        )
    if (
        value["operation_id"] != operation_id
        or type(value["exit_status"]) is not int
        or value["exit_status"] != exit_status
    ):
        raise BootstrapError(
            f"identity transcript has the wrong {label} operation binding"
        )
    for stream in ("stdout", "stderr"):
        digest = value[f"{stream}_sha256"]
        size = value[f"{stream}_size_bytes"]
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or type(size) is not int
            or size < 0
            or size > _MAX_HELPER_OUTPUT_BYTES
        ):
            raise BootstrapError(
                f"identity transcript has invalid {label} {stream} metadata"
            )


def _validate_raw_commit(raw: bytes, identity: dict[str, Any]) -> None:
    """Authenticate archived commit bytes against the candidate and trailers."""

    headers, separator, message = raw.partition(b"\n\n")
    if not separator or b"\r" in headers or b"\0" in headers:
        raise BootstrapError("identity raw commit has malformed headers")
    records: list[tuple[bytes, list[bytes]]] = []
    for line in headers.split(b"\n"):
        if line.startswith(b" "):
            if not records:
                raise BootstrapError("identity raw commit has an orphan folded header")
            records[-1][1].append(line[1:])
            continue
        key, marker, field = line.partition(b" ")
        if not marker or not key or any(byte < 0x21 or byte > 0x7E for byte in key):
            raise BootstrapError("identity raw commit has a malformed header")
        records.append((key, [field]))
    trees = [values for key, values in records if key == b"tree"]
    if trees != [[identity["head_tree"].encode("ascii")]]:
        raise BootstrapError("identity raw commit tree does not match the candidate")
    signatures = [values for key, values in records if key.startswith(b"gpgsig")]
    if len(signatures) != 1 or not any(key == b"gpgsig" for key, _ in records):
        raise BootstrapError("identity raw commit must contain exactly one SSH signature")
    signature = b"\n".join(signatures[0])
    lines = signature.split(b"\n")
    if len(lines) < 3 or lines[0] != _SSH_BEGIN or lines[-1] != _SSH_END:
        raise BootstrapError("identity raw commit has invalid SSH signature armor")
    try:
        if not base64.b64decode(b"".join(lines[1:-1]), validate=True):
            raise ValueError
    except (ValueError, binascii.Error) as error:
        raise BootstrapError("identity raw commit has malformed SSH signature data") from error
    if b"\r" in message or b"\0" in message:
        raise BootstrapError("identity raw commit has a malformed LF-only message")
    try:
        text = message.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("identity raw commit message is not UTF-8") from error
    expected = [
        f"{_TRAILER_VERSION}: 1",
        f"{_TRAILER_MANIFEST}: {identity['workspace_source_manifest_sha256']}",
        f"{_TRAILER_LOCK}: {identity['cargo_lock_sha256']}",
    ]
    text_lines = text[:-1].split("\n") if text.endswith("\n") else []
    trailer_keys = {
        _TRAILER_VERSION.casefold(),
        _TRAILER_MANIFEST.casefold(),
        _TRAILER_LOCK.casefold(),
    }
    recognized = [
        index
        for index, line in enumerate(text_lines)
        if ":" in line and line.partition(":")[0].casefold() in trailer_keys
    ]
    terminal = list(range(len(text_lines) - 3, len(text_lines)))
    if (
        len(text_lines) < 5
        or text_lines[-4] != ""
        or not text_lines[-5]
        or text_lines[-3:] != expected
        or recognized != terminal
    ):
        raise BootstrapError("identity raw commit has the wrong release trailer block")
    framed = b"commit " + str(len(raw)).encode("ascii") + b"\0" + raw
    observed_oid = (
        hashlib.sha1(framed, usedforsecurity=False).hexdigest()
        if len(identity["head_commit"]) == 40
        else hashlib.sha256(framed).hexdigest()
    )
    if observed_oid != identity["head_commit"]:
        raise BootstrapError("identity raw commit bytes do not reproduce HEAD")


def _validate_legacy_identity_evidence(
    directory: Path,
    identity: dict[str, Any],
    identity_bytes: bytes,
    expected: dict[str, str],
) -> tuple[dict[str, FileSnapshot], dict[str, Any], dict[str, Any]]:
    attestation = _read_file(
        directory / "identity-attestation.json",
        "identity attestation",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    transcript = _read_file(
        directory / "identity-transcript.json",
        "identity transcript",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    attestation_json = _parse_canonical_json(attestation, "identity attestation")
    transcript_json = _parse_canonical_json(transcript, "identity transcript")
    if attestation.mode != _DATA_MODE or transcript.mode != _DATA_MODE:
        raise BootstrapError("identity attestation and transcript must have exact mode 0400")
    if set(attestation_json) != _ATTESTATION_KEYS:
        raise BootstrapError("identity attestation has the wrong schema")
    if set(transcript_json) != _TRANSCRIPT_KEYS:
        raise BootstrapError("identity transcript has the wrong schema")
    if (
        type(attestation_json.get("schema_version")) is not int
        or attestation_json["schema_version"] != 2
    ):
        raise BootstrapError("identity attestation must use schema version 2")
    if (
        type(transcript_json.get("schema_version")) is not int
        or transcript_json["schema_version"] != 2
    ):
        raise BootstrapError("identity transcript must use schema version 2")
    if attestation_json.get("release_identity") != identity:
        raise BootstrapError("identity attestation does not bind the candidate identity")
    if attestation_json.get("release_identity_sha256") != hashlib.sha256(
        identity_bytes
    ).hexdigest():
        raise BootstrapError("identity attestation has the wrong identity digest")
    verification = attestation_json.get("verification")
    if (
        not isinstance(verification, dict)
        or verification.get("signer_fingerprint") != expected["fingerprint"]
    ):
        raise BootstrapError("identity attestation has the wrong signer fingerprint")
    if verification.get("status") != "G":
        raise BootstrapError("identity attestation is not a good SSH signature")
    if verification.get("primary_key_fingerprint") != "":
        raise BootstrapError("identity attestation is not first-release SSH metadata")
    if not isinstance(verification.get("allowed_signers_principal"), str) or not verification.get(
        "allowed_signers_principal"
    ):
        raise BootstrapError("identity attestation omits its allowed-signers principal")
    tools = attestation_json.get("tools")
    if not isinstance(tools, dict):
        raise BootstrapError("identity attestation omits protected tools")
    for key, digest_key in (("git", "git"), ("ssh_keygen", "ssh")):
        item = tools.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0500":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    policies = attestation_json.get("policies")
    if not isinstance(policies, dict) or policies.get("signature_format") != "ssh":
        raise BootstrapError("identity attestation does not bind SSH policy")
    if policies.get("expected_signer_fingerprint") != expected["fingerprint"]:
        raise BootstrapError("identity attestation has the wrong protected fingerprint")
    for key, digest_key in (("ssh_allowed_signers", "allowed"), ("ssh_revocation", "revocation")):
        item = policies.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0400":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    evidence = attestation_json.get("evidence")
    if not isinstance(evidence, dict) or set(evidence) != _EVIDENCE_KEYS:
        raise BootstrapError("identity attestation has the wrong evidence inventory")
    snapshots: dict[str, FileSnapshot] = {
        "identity_attestation": attestation,
        "identity_transcript": transcript,
    }
    expected_archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    seen_names: set[str] = set()
    for label, record in evidence.items():
        if not isinstance(record, dict):
            raise BootstrapError(f"identity evidence record {label} is invalid")
        name = record.get("archive_name")
        if (
            not isinstance(name, str)
            or not name
            or name in {".", ".."}
            or "/" in name
            or name in seen_names
        ):
            raise BootstrapError(f"identity evidence record {label} has an invalid archive name")
        seen_names.add(name)
        if name != expected_archive_names[label]:
            raise BootstrapError(f"identity evidence {label} has the wrong archive name")
        mode_text = record.get("mode")
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        if mode_text != f"{expected_mode:04o}":
            raise BootstrapError(f"identity evidence {label} has the wrong protected mode")
        digest = record.get("sha256")
        if digest is None:
            digest = record.get("observed_sha256")
        if not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None:
            raise BootstrapError(f"identity evidence {label} has an invalid digest")
        size = record.get("size_bytes")
        if type(size) is not int or size < 0 or size > _MAX_EVIDENCE_BYTES:
            raise BootstrapError(f"identity evidence {label} has an invalid size")
        snapshot = _read_file(
            directory / name,
            f"identity evidence {label}",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
            executable=expected_mode == _TOOL_MODE,
        )
        if (
            snapshot.mode != expected_mode
            or len(snapshot.data) != size
            or snapshot.sha256 != digest
        ):
            raise BootstrapError(f"identity evidence {label} does not match its attestation")
        snapshots[label] = snapshot
    _validate_allowed_signers_policy(snapshots["ssh_allowed_signers"].data)
    transcript_record = evidence["verify_transcript"]
    transcript_digest = transcript_record.get("sha256")
    if transcript_digest is None:
        transcript_digest = transcript_record.get("observed_sha256")
    if (
        transcript_record.get("archive_name") != transcript.path.name
        or transcript_digest != transcript.sha256
    ):
        raise BootstrapError("identity transcript does not match its attested evidence record")
    if transcript_json.get("candidate_commit_oid") != identity["head_commit"]:
        raise BootstrapError("identity transcript has the wrong candidate commit")
    if transcript_json.get("archive_names") != expected_archive_names:
        raise BootstrapError("identity transcript has the wrong replay archive mapping")
    if transcript_json.get("tools") != tools or transcript_json.get("policies") != policies:
        raise BootstrapError("identity transcript disagrees with the attestation")
    commands = transcript_json.get("commands")
    if not isinstance(commands, dict) or set(commands) != {
        "show_signature_metadata",
        "verify_commit",
    }:
        raise BootstrapError("identity transcript has the wrong command inventory")
    _validate_command_record(
        commands["show_signature_metadata"],
        "show-signature command",
        require_success=True,
    )
    _validate_command_record(
        commands["verify_commit"],
        "verify-commit command",
        require_success=True,
    )
    probes = transcript_json.get("tool_probes")
    if not isinstance(probes, dict) or set(probes) != {"ssh_keygen_usage"}:
        raise BootstrapError("identity transcript has the wrong tool-probe inventory")
    _validate_command_record(
        probes["ssh_keygen_usage"],
        "ssh-keygen probe",
        require_success=False,
    )
    if tools["git"]["size_bytes"] != evidence["git"]["size_bytes"]:
        raise BootstrapError("identity Git size disagrees with its evidence")
    if tools["ssh_keygen"]["size_bytes"] != evidence["ssh_keygen"]["size_bytes"]:
        raise BootstrapError("identity ssh-keygen size disagrees with its evidence")
    if (
        policies["ssh_allowed_signers"]["size_bytes"]
        != evidence["ssh_allowed_signers"]["size_bytes"]
    ):
        raise BootstrapError("allowed-signers size disagrees with its evidence")
    if (
        policies["ssh_revocation"]["size_bytes"]
        != evidence["ssh_revocation"]["size_bytes"]
    ):
        raise BootstrapError("SSH revocation size disagrees with its evidence")
    return snapshots, attestation_json, transcript_json


def _validate_identity_evidence(
    directory: Path,
    identity: dict[str, Any],
    identity_bytes: bytes,
    expected: dict[str, str],
) -> tuple[dict[str, FileSnapshot], dict[str, Any], dict[str, Any]]:
    """Authenticate path-free identity documents against local archive bytes."""

    attestation = _read_file(
        directory / "identity-attestation.json",
        "identity attestation",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    transcript = _read_file(
        directory / "identity-transcript.json",
        "identity transcript",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if attestation.mode != _DATA_MODE or transcript.mode != _DATA_MODE:
        raise BootstrapError(
            "identity attestation and transcript must have exact mode 0400"
        )
    attestation_json = _parse_canonical_json(
        attestation, "identity attestation"
    )
    transcript_json = _parse_canonical_json(transcript, "identity transcript")
    if set(attestation_json) != _ATTESTATION_KEYS:
        raise BootstrapError("identity attestation has the wrong schema")
    if set(transcript_json) != _TRANSCRIPT_KEYS:
        raise BootstrapError("identity transcript has the wrong schema")
    if (
        attestation_json["format"] != _IDENTITY_ATTESTATION_FORMAT
        or type(attestation_json["schema_version"]) is not int
        or attestation_json["schema_version"] != 3
        or transcript_json["format"] != _IDENTITY_TRANSCRIPT_FORMAT
        or type(transcript_json["schema_version"]) is not int
        or transcript_json["schema_version"] != 3
    ):
        raise BootstrapError("identity documents must use sanitized schema 3")
    candidate = attestation_json["candidate"]
    if not isinstance(candidate, dict) or candidate != {
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
    }:
        raise BootstrapError("identity attestation does not bind the candidate")

    archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    records = attestation_json["archives"]
    if not isinstance(records, dict) or set(records) != set(_IDENTITY_ARCHIVE_IDS):
        raise BootstrapError("identity attestation has the wrong archive inventory")
    snapshots: dict[str, FileSnapshot] = {
        "identity_attestation": attestation,
        "identity_transcript": transcript,
    }
    for label, archive_id in _IDENTITY_ARCHIVE_IDS.items():
        record = records[label]
        if (
            not isinstance(record, dict)
            or set(record) != {"archive_id", "mode", "sha256", "size_bytes"}
            or record["archive_id"] != archive_id
        ):
            raise BootstrapError(
                f"identity archive {label} has an invalid protected record"
            )
        expected_mode = (
            _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        )
        if record["mode"] != f"{expected_mode:04o}":
            raise BootstrapError(f"identity archive {label} has the wrong mode")
        digest = record["sha256"]
        size = record["size_bytes"]
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or type(size) is not int
            or size < 0
            or size > _MAX_EVIDENCE_BYTES
        ):
            raise BootstrapError(
                f"identity archive {label} has invalid integrity metadata"
            )
        snapshot = (
            transcript
            if label == "verify_transcript"
            else _read_file(
                directory / archive_names[label],
                f"identity archive {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                executable=expected_mode == _TOOL_MODE,
            )
        )
        if (
            snapshot.mode != expected_mode
            or snapshot.sha256 != digest
            or snapshot.size != size
        ):
            raise BootstrapError(
                f"identity archive {label} does not match authenticated bytes"
            )
        snapshots[label] = snapshot
    for label, digest_key in (
        ("git", "git"),
        ("ssh_keygen", "ssh"),
        ("ssh_allowed_signers", "allowed"),
        ("ssh_revocation", "revocation"),
    ):
        if snapshots[label].sha256 != expected[digest_key]:
            raise BootstrapError(
                f"identity archive {label} has the wrong protected digest"
            )
    if snapshots["cargo_lock"].sha256 != identity["cargo_lock_sha256"]:
        raise BootstrapError("identity Cargo.lock archive has the wrong digest")
    _validate_allowed_signers_policy(snapshots["ssh_allowed_signers"].data)

    if (
        transcript_json["archive_ids"] != _IDENTITY_ARCHIVE_IDS
        or transcript_json["candidate_commit_oid"] != identity["head_commit"]
    ):
        raise BootstrapError("identity transcript has the wrong archive binding")
    operations = transcript_json["operations"]
    if not isinstance(operations, dict) or set(operations) != {
        "show_signature_metadata",
        "verify_commit",
        "ssh_keygen_usage",
    }:
        raise BootstrapError("identity transcript has the wrong operation inventory")
    _validate_sanitized_operation(
        operations["show_signature_metadata"],
        "show-signature",
        operation_id="git.show-signature-metadata.ssh.v1",
        exit_status=0,
    )
    _validate_sanitized_operation(
        operations["verify_commit"],
        "verify-commit",
        operation_id="git.verify-commit.ssh.v1",
        exit_status=0,
    )
    _validate_sanitized_operation(
        operations["ssh_keygen_usage"],
        "ssh-keygen",
        operation_id="ssh-keygen.usage-probe.v1",
        exit_status=1,
    )
    _validate_raw_commit(snapshots["raw_commit"].data, identity)
    return snapshots, attestation_json, transcript_json


def _validate_private_identity_provenance(
    snapshot: FileSnapshot,
    *,
    identity: dict[str, Any],
    identity_snapshot: FileSnapshot,
    candidate: Path,
    private_outputs: dict[str, Path],
    private_snapshots: dict[str, FileSnapshot],
    protected: dict[str, FileSnapshot],
) -> None:
    """Authenticate the path-bearing verifier record before deleting it."""

    value = _parse_canonical_json(
        snapshot, "bootstrap-private identity provenance"
    )
    _require_exact_json_fields(
        value,
        {
            "format",
            "schema_version",
            "candidate",
            "outputs",
            "archive_names",
            "tools",
            "policies",
            "verification",
            "execution",
            "sanitized_transcript",
        },
        "bootstrap-private identity provenance",
    )
    if (
        value["format"] != _IDENTITY_PRIVATE_PROVENANCE_FORMAT
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
    ):
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong schema"
        )
    private_directory = snapshot.path.parent
    expected_candidate = {
        "root_path": str(candidate),
        "identity_source_path": str(identity_snapshot.path),
        "cargo_lock_source_path": str(candidate / "Cargo.lock"),
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": identity_snapshot.sha256,
    }
    if value["candidate"] != expected_candidate:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong candidate"
        )
    expected_outputs = {
        "attestation": str(private_outputs["attestation"]),
        "bootstrap-private provenance": str(private_outputs["provenance"]),
        "verify transcript": str(private_outputs["transcript"]),
        "raw commit": str(private_outputs["raw_commit"]),
        "Cargo.lock archive": str(private_outputs["cargo_lock"]),
        "SSH allowed-signers archive": str(private_outputs["allowed"]),
        "SSH revocation-policy archive": str(private_outputs["revocation"]),
        "Git archive": str(private_outputs["git"]),
        "ssh-keygen archive": str(private_outputs["ssh"]),
    }
    if value["outputs"] != expected_outputs:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong output paths"
        )
    expected_archive_names = {
        "cargo_lock": private_outputs["cargo_lock"].name,
        "git": private_outputs["git"].name,
        "raw_commit": private_outputs["raw_commit"].name,
        "ssh_allowed_signers": private_outputs["allowed"].name,
        "ssh_keygen": private_outputs["ssh"].name,
        "ssh_revocation": private_outputs["revocation"].name,
        "verify_transcript": private_outputs["transcript"].name,
    }
    if value["archive_names"] != expected_archive_names:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong archive names"
        )
    tools = value["tools"]
    if not isinstance(tools, dict) or set(tools) != {"git", "ssh_keygen"}:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong tool inventory"
        )
    tool_expectations = {
        "git": ("git", "git", _IDENTITY_ARCHIVE_IDS["git"]),
        "ssh_keygen": ("ssh", "ssh_keygen", _IDENTITY_ARCHIVE_IDS["ssh_keygen"]),
    }
    for label, (snapshot_label, protected_label, archive_id) in tool_expectations.items():
        record = _require_exact_json_fields(
            tools[label],
            {
                "archive_id",
                "archive_path",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_path",
            },
            f"bootstrap-private identity tool {label}",
        )
        archived = private_snapshots[snapshot_label]
        if (
            record["archive_id"] != archive_id
            or record["mode"] != "0500"
            or record["observed_sha256"] != archived.sha256
            or record["protected_sha256"] != protected[protected_label].sha256
            or record["size_bytes"] != archived.size
            or record["source_path"] != str(protected[protected_label].path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity tool {label} binding is not exact"
            )
        archive_path = Path(record["archive_path"])
        if (
            archive_path.parent != private_directory
            or not archive_path.name.startswith(
                "." + expected_archive_names[
                    "git" if label == "git" else "ssh_keygen"
                ] + ".stage."
            )
            or os.path.lexists(archive_path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity tool {label} stage is invalid"
            )

    policies = _require_exact_json_fields(
        value["policies"],
        {
            "expected_signer_fingerprint",
            "signature_format",
            "ssh_allowed_signers",
            "ssh_revocation",
        },
        "bootstrap-private identity policies",
    )
    if (
        policies["expected_signer_fingerprint"]
        != value["verification"].get("signer_fingerprint")
        or policies["signature_format"] != "ssh"
    ):
        raise BootstrapError(
            "bootstrap-private identity signature policy is not exact"
        )
    for label, private_label, protected_label in (
        ("ssh_allowed_signers", "allowed", "allowed_signers"),
        ("ssh_revocation", "revocation", "revocation"),
    ):
        record = _require_exact_json_fields(
            policies[label],
            {
                "archive_id",
                "archive_path",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_path",
            },
            f"bootstrap-private identity policy {label}",
        )
        archived = private_snapshots[private_label]
        if (
            record["archive_id"] != _IDENTITY_ARCHIVE_IDS[label]
            or record["mode"] != "0400"
            or record["observed_sha256"] != archived.sha256
            or record["protected_sha256"] != protected[protected_label].sha256
            or record["size_bytes"] != archived.size
            or record["source_path"] != str(protected[protected_label].path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity policy {label} binding is not exact"
            )
    verification = _require_exact_json_fields(
        value["verification"],
        {
            "signature_format",
            "status",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
        },
        "bootstrap-private identity verification",
    )
    if (
        verification["signature_format"] != "ssh"
        or verification["status"] != "G"
        or verification["primary_key_fingerprint"] != ""
        or _FINGERPRINT_RE.fullmatch(
            str(verification["signer_fingerprint"])
        )
        is None
        or not isinstance(verification["allowed_signers_principal"], str)
        or not verification["allowed_signers_principal"]
    ):
        raise BootstrapError(
            "bootstrap-private identity verification is not trusted SSH"
        )
    execution = _require_exact_json_fields(
        value["execution"],
        {
            "environment",
            "policy_overrides",
            "replay",
            "commands",
            "tool_probes",
        },
        "bootstrap-private identity execution",
    )
    if (
        not isinstance(execution["environment"], dict)
        or execution["environment"].get("HOME") != str(private_directory)
        or not isinstance(execution["policy_overrides"], list)
    ):
        raise BootstrapError(
            "bootstrap-private identity execution environment is not exact"
        )
    commands = execution["commands"]
    if not isinstance(commands, dict) or set(commands) != {
        "show_signature_metadata",
        "verify_commit",
    }:
        raise BootstrapError(
            "bootstrap-private identity command inventory is not exact"
        )
    _validate_command_record(
        commands["show_signature_metadata"],
        "bootstrap-private show-signature",
        require_success=True,
    )
    _validate_command_record(
        commands["verify_commit"],
        "bootstrap-private verify-commit",
        require_success=True,
    )
    probes = execution["tool_probes"]
    if not isinstance(probes, dict) or set(probes) != {"ssh_keygen_usage"}:
        raise BootstrapError(
            "bootstrap-private identity probe inventory is not exact"
        )
    _validate_command_record(
        probes["ssh_keygen_usage"],
        "bootstrap-private ssh-keygen",
        require_success=False,
    )
    transcript_record = value["sanitized_transcript"]
    expected_transcript_record = {
        "archive_id": _IDENTITY_ARCHIVE_IDS["verify_transcript"],
        "mode": "0400",
        "sha256": private_snapshots["transcript"].sha256,
        "size_bytes": private_snapshots["transcript"].size,
    }
    if transcript_record != expected_transcript_record:
        raise BootstrapError(
            "bootstrap-private provenance does not bind sanitized transcript"
        )


def _artifact_record(label: str, archive: FileSnapshot) -> dict[str, Any]:
    return {
        "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
        "archive_name": archive.path.name,
        "mode": f"{archive.mode:04o}",
        "sha256": archive.sha256,
        "size_bytes": archive.size,
    }


def _framework_python_marker_record(
    inventory_snapshot: FileSnapshot,
) -> dict[str, Any]:
    """Project the private helper inventory into a path-free marker record."""

    inventory = _parse_canonical_json(
        inventory_snapshot, "framework Python runtime inventory",
    )
    required = {
        "format",
        "schema_version",
        "runtime_root",
        "record_count",
        "file_bytes",
        "records",
        "source_disclosure",
        "input_record_count",
        "input_file_bytes",
        "input_records",
    }
    if (
        set(inventory) != required
        or inventory["format"]
        != "iroha-sumeragi-v2-private-framework-python-runtime"
        or type(inventory["schema_version"]) is not int
        or inventory["schema_version"] != 1
        or inventory["source_disclosure"] != "withheld"
        or not isinstance(inventory["records"], list)
        or not isinstance(inventory["input_records"], list)
    ):
        raise BootstrapError(
            "framework Python runtime helper returned the wrong inventory"
        )
    sanitized: list[dict[str, Any]] = []
    for record in inventory["records"]:
        if not isinstance(record, dict):
            raise BootstrapError(
                "framework Python runtime inventory member is malformed"
            )
        kind = record.get("kind")
        keys = {
            "directory": {"path", "kind", "device", "inode", "mode"},
            "file": {
                "path",
                "kind",
                "device",
                "inode",
                "mode",
                "size",
                "sha256",
            },
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        path = record.get("path")
        if (
            keys is None
            or set(record) != keys
            or not isinstance(path, str)
            or not path
            or path.startswith("/")
            or ".." in Path(path).parts
            or Path(path).as_posix() != path
            or not isinstance(record.get("mode"), str)
            or re.fullmatch(r"[0-7]{4}", record["mode"]) is None
        ):
            raise BootstrapError(
                "framework Python runtime inventory member is not exact"
            )
        projected = {
            key: record[key]
            for key in (
                ("path", "kind", "mode")
                if kind == "directory"
                else ("path", "kind", "mode", "size", "sha256")
                if kind == "file"
                else ("path", "kind", "mode", "target")
            )
        }
        if (
            kind == "file"
            and (
                type(projected["size"]) is not int
                or projected["size"] < 0
                or not isinstance(projected["sha256"], str)
                or _DIGEST_RE.fullmatch(projected["sha256"]) is None
            )
        ) or (
            kind == "symlink"
            and (
                not isinstance(projected["target"], str)
                or not projected["target"]
            )
        ):
            raise BootstrapError(
                "framework Python runtime inventory member metadata is invalid"
            )
        sanitized.append(projected)
    sanitized.sort(key=lambda record: record["path"])
    file_bytes = sum(
        record["size"] for record in sanitized if record["kind"] == "file"
    )
    if (
        type(inventory["record_count"]) is not int
        or inventory["record_count"] != len(sanitized)
        or type(inventory["file_bytes"]) is not int
        or inventory["file_bytes"] != file_bytes
        or sanitized
        != sorted(sanitized, key=lambda record: str(record["path"]))
    ):
        raise BootstrapError(
            "framework Python runtime inventory accounting is not exact"
        )
    return {
        "format": "iroha-sumeragi-v2-framework-python-runtime",
        "schema_version": 1,
        "archive_root": "python-runtime",
        "root_mode": "0500",
        "executable": "bin/python3",
        "inventory": {
            "archive_name": "python-runtime-input.json",
            "mode": f"{inventory_snapshot.mode:04o}",
            "sha256": inventory_snapshot.sha256,
            "size_bytes": inventory_snapshot.size,
        },
        "record_count": len(sanitized),
        "file_bytes": file_bytes,
        "records": sanitized,
    }


def _copy_framework_python_archive(
    *,
    evidence: Path,
    protected_python: FileSnapshot,
    runtime_helper: FileSnapshot,
    timeout_seconds: int,
) -> tuple[FileSnapshot, FileSnapshot, dict[str, Any]]:
    """Create one complete protected framework-Python archive via the helper."""

    runtime_root = evidence / "python-runtime"
    inventory_path = evidence / "python-runtime-input.json"
    environment = _closed_environment(evidence, [])
    result = _run_bounded(
        protected_python.path,
        [
            "-I",
            "-S",
            str(runtime_helper.path),
            "--copy-framework-python",
            "--runtime-root",
            str(runtime_root),
            "--runtime-inventory",
            str(inventory_path),
        ],
        cwd=evidence,
        environment=environment,
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    if result.returncode != 0 or result.stdout or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            f"protected framework Python archive copy failed: {detail}"
        )
    archive = _read_file(
        runtime_root / "bin" / "python3",
        "archived framework Python",
        maximum_bytes=_MAX_TOOL_BYTES,
        executable=True,
    )
    inventory = _read_file(
        inventory_path,
        "framework Python runtime inventory",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if (
        archive.data != protected_python.data
        or archive.mode != _TOOL_MODE
        or inventory.mode != _DATA_MODE
    ):
        raise BootstrapError(
            "archived framework Python does not match its protected executable"
        )
    marker_record = _framework_python_marker_record(inventory)
    if (
        _parse_canonical_json(
            inventory, "framework Python runtime inventory",
        )["runtime_root"]
        != str(runtime_root)
    ):
        raise BootstrapError(
            "framework Python runtime inventory names the wrong archive root"
        )
    return archive, inventory, marker_record


def _verify_framework_python_archive(
    *,
    evidence: Path,
    protected_python: FileSnapshot,
    runtime_helper: FileSnapshot,
    inventory: FileSnapshot,
    marker_record: dict[str, Any],
    timeout_seconds: int,
) -> None:
    """Reauthenticate the source and every archived member after construction."""

    result = _run_bounded(
        protected_python.path,
        [
            "-I",
            "-S",
            str(runtime_helper.path),
            "--verify-framework-python",
            "--runtime-root",
            str(evidence / "python-runtime"),
            "--runtime-inventory",
            str(inventory.path),
        ],
        cwd=evidence,
        environment=_closed_environment(evidence, []),
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    if result.returncode != 0 or result.stdout or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            f"protected framework Python archive verification failed: {detail}"
        )
    _require_unchanged(
        inventory,
        "framework Python runtime inventory",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if _framework_python_marker_record(inventory) != marker_record:
        raise BootstrapError(
            "framework Python runtime marker projection changed"
        )


def _protected_size_limit(label: str, executable_labels: set[str]) -> int:
    if label in executable_labels:
        return _MAX_TOOL_BYTES
    if label in {"allowed_signers", "revocation"}:
        return _MAX_POLICY_BYTES
    if label == "sdk_dependency_bundle_manifest":
        return _MAX_SDK_MANIFEST_BYTES
    return _MAX_HELPER_BYTES


def _parse_runner_environment(values: list[str]) -> dict[str, str]:
    result: dict[str, str] = {}
    for value in values:
        name, separator, assigned = value.partition("=")
        if (
            not separator
            or _RUNNER_ENV_RE.fullmatch(name) is None
            or name not in _RUNNER_ENV_ALLOWLIST
        ):
            raise BootstrapError(
                "runner environment entries must use an explicitly allowed NAME=VALUE"
            )
        if name in result or "\0" in assigned:
            raise BootstrapError("runner environment entries must be unique and NUL-free")
        result[name] = assigned
    return result


def _cancellation_control_path(
    environment: dict[str, str], candidate: Path
) -> Path | None:
    rendered = environment.get("IROHA_RELEASE_CANCEL_REQUEST_PATH")
    if rendered is None:
        return None
    path = Path(rendered)
    if (
        not path.is_absolute()
        or path != Path(os.path.abspath(path))
        or _SAFE_PATH_RE.fullmatch(str(path)) is None
        or os.pathsep in str(path)
        or path.name in {"", ".", ".."}
    ):
        raise BootstrapError(
            "cooperative cancellation path must be absolute, normalized, and shell-safe"
        )
    parent = _absolute_resolved_existing(
        path.parent, "cooperative cancellation directory"
    )
    metadata = parent.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or stat.S_IMODE(metadata.st_mode) != _DIRECTORY_MODE
    ):
        raise BootstrapError(
            "cooperative cancellation directory must be owner-owned with exact mode 0700"
        )
    path = parent / path.name
    if _inside(path, candidate):
        raise BootstrapError(
            "cooperative cancellation path must be outside the candidate root"
        )
    return path


def _read_cancellation_request(path: Path) -> FileSnapshot:
    request = _read_file(
        path,
        "cooperative cancellation request",
        maximum_bytes=len(_CANCELLATION_REQUEST_BYTES),
    )
    if (
        request.data != _CANCELLATION_REQUEST_BYTES
        or request.owner != os.getuid()
        or request.nlink != 1
        or request.mode & 0o077
    ):
        raise BootstrapError(
            "cooperative cancellation request is not canonical, private, and owner-bound"
        )
    return request


def _publish_cancellation_result(
    *,
    evidence: Path,
    evidence_fd: int,
    request_path: Path | None,
    candidate: Path,
    identity: dict[str, Any],
    identity_snapshot: FileSnapshot,
    bootstrap_marker: FileSnapshot,
    runner_snapshot: FileSnapshot,
    runner_logs: dict[str, LargeFileSnapshot],
) -> FileSnapshot:
    if request_path is None:
        raise BootstrapError(
            "runner returned cooperative cancellation without a bound request path"
        )
    for forbidden in (
        evidence / "BOOTSTRAP_RELEASE_COMPLETED.json",
        evidence / "release-runner" / "output" / "release" / "RELEASE_COMPLETED.json",
    ):
        if forbidden.exists() or forbidden.is_symlink():
            raise BootstrapError(
                "cooperative cancellation cannot coexist with release completion evidence"
            )
    request = _read_cancellation_request(request_path)
    value = {
        "schema_version": 2,
        "result": "release-cancelled",
        "reason": "operator-request",
        "bootstrap_completion_sha256": bootstrap_marker.sha256,
        "candidate_identity_sha256": identity_snapshot.sha256,
        "candidate_commit_oid": identity["head_commit"],
        "candidate_tree_oid": identity["head_tree"],
        "request": {
            "archive_id": "release-bootstrap.cancellation-request.v1",
            "sha256": request.sha256,
            "size_bytes": request.size,
            "mode": f"{request.mode:04o}",
            "owner_uid": request.owner,
            "nlink": request.nlink,
        },
        "runner": {
            "archive_id": "release-candidate.runner.v1",
            "sha256": runner_snapshot.sha256,
            "mode": f"{runner_snapshot.mode:04o}",
            "exit_status": _COOPERATIVE_CANCELLED_STATUS,
            "logs": {
                label: {
                    "archive_id": f"release-bootstrap.runner-{label}.v1",
                    "sha256": snapshot.sha256,
                    "size_bytes": snapshot.size,
                    "mode": f"{snapshot.mode:04o}",
                }
                for label, snapshot in sorted(runner_logs.items())
            },
        },
    }
    cancelled = _publish_completion_marker(
        evidence,
        evidence_fd,
        _canonical_json(value),
        final_name="BOOTSTRAP_CANCELLED.json",
    )
    if (
        cancelled.mode != _DATA_MODE
        or cancelled.owner != os.getuid()
        or cancelled.nlink != 1
    ):
        raise BootstrapError("external cancellation marker metadata is not exact")
    _require_unchanged(
        request,
        "cooperative cancellation request",
        maximum_bytes=len(_CANCELLATION_REQUEST_BYTES),
    )
    _require_unchanged(
        bootstrap_marker,
        "bootstrap completion marker",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    _require_unchanged(
        identity_snapshot,
        "candidate identity evidence",
        maximum_bytes=_MAX_IDENTITY_BYTES,
    )
    _require_unchanged(
        runner_snapshot,
        "signed candidate release runner",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    for label, snapshot in runner_logs.items():
        _require_large_file_unchanged(
            snapshot, f"cancelled release runner {label} log"
        )
    return cancelled


def _require_nonwritable_ancestors(path: Path, label: str) -> None:
    for ancestor in (path.parent, *path.parent.parents):
        metadata = ancestor.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, os.getuid()}
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            raise BootstrapError(
                f"{label} has a writable, symlinked, or untrusted ancestor"
            )


def _load_runner_tool_manifest(
    snapshot: FileSnapshot, candidate: Path
) -> dict[str, FileSnapshot]:
    manifest = _parse_canonical_json(snapshot, "runner tool manifest")
    _require_exact_json_fields(
        manifest, {"schema_version", "tools"}, "runner tool manifest"
    )
    tools = manifest["tools"]
    if (
        type(manifest["schema_version"]) is not int
        or manifest["schema_version"] != 1
        or not isinstance(tools, dict)
        or set(tools) != _REQUIRED_RUNNER_TOOL_NAMES
        or len(tools) > _MAX_RUNNER_TOOLS
    ):
        raise BootstrapError(
            "runner tool manifest does not contain the exact first-release command closure"
        )
    reserved = {"bash", "git", "python3", "ssh-keygen"}
    snapshots: dict[str, FileSnapshot] = {}
    inodes: set[tuple[int, int]] = set()
    for name in sorted(tools):
        if (
            not isinstance(name, str)
            or _RUNNER_TOOL_NAME_RE.fullmatch(name) is None
            or name in reserved
            or os.pathsep in name
        ):
            raise BootstrapError("runner tool manifest has an unsafe alias")
        record = _require_exact_json_fields(
            tools[name], {"path", "sha256"}, f"runner tool {name}"
        )
        if not isinstance(record["path"], str):
            raise BootstrapError(f"runner tool {name} path is not text")
        source = _protected_snapshot(
            Path(record["path"]),
            _require_digest(record["sha256"], f"runner tool {name} digest"),
            f"runner tool {name}",
            candidate=candidate,
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        if (
            source.owner not in {0, os.getuid()}
            or source.mode & 0o022
            or os.pathsep in str(source.path)
        ):
            raise BootstrapError(f"runner tool {name} source is writable or untrusted")
        _require_nonwritable_ancestors(source.path, f"runner tool {name}")
        inode = (source.device, source.inode)
        if inode in inodes:
            raise BootstrapError("runner tool manifest contains an executable inode alias")
        inodes.add(inode)
        snapshots[name] = source
    return snapshots


def _runner_tool_record(
    name: str, archive: FileSnapshot, alias: SymlinkSnapshot
) -> dict[str, Any]:
    return {
        "archive_id": f"release-runner-tool.{name}.v1",
        "alias_name": name,
        "archive_name": f"runner-tools/{name}",
        "mode": f"{archive.mode:04o}",
        "sha256": archive.sha256,
        "size_bytes": archive.size,
    }


def _runner_alias_snapshot(path: Path, target: Path, label: str) -> SymlinkSnapshot:
    relative_target = os.path.relpath(target, path.parent)
    metadata = path.lstat()
    if (
        not stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or os.readlink(path) != relative_target
        or path.resolve(strict=True) != target
    ):
        raise BootstrapError(f"{label} is not one exact protected symlink alias")
    return SymlinkSnapshot(
        path=path,
        target=relative_target,
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=stat.S_IMODE(metadata.st_mode),
        owner=metadata.st_uid,
        nlink=metadata.st_nlink,
        mtime_ns=metadata.st_mtime_ns,
        ctime_ns=metadata.st_ctime_ns,
    )


def _revalidate_runner_tools(
    sources: dict[str, FileSnapshot],
    archives: dict[str, FileSnapshot],
    aliases: dict[str, SymlinkSnapshot],
) -> None:
    if set(sources) != set(archives) or set(sources) != set(aliases):
        raise BootstrapError("runner tool alias inventory changed")
    for name in sorted(sources):
        _require_unchanged(
            sources[name],
            f"runner tool {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        _require_unchanged(
            archives[name],
            f"archived runner tool {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            executable=True,
        )
        current_alias = _runner_alias_snapshot(
            aliases[name].path, archives[name].path, f"runner tool alias {name}"
        )
        if current_alias != aliases[name]:
            raise BootstrapError(f"runner tool alias {name} changed")


def _revalidate_receipt_validator_components(
    sources: dict[str, FileSnapshot],
    archives: dict[str, FileSnapshot],
) -> None:
    """Reauthenticate the complete reviewed receipt-validator module closure."""

    if set(sources) != set(archives):
        raise BootstrapError("receipt validator component inventory changed")
    for name in sorted(sources):
        _require_unchanged(
            sources[name],
            f"protected receipt validator component {name}",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        _require_unchanged(
            archives[name],
            f"archived receipt validator component {name}",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        if sources[name].data != archives[name].data:
            raise BootstrapError(
                f"archived receipt validator component {name} changed"
            )


def _cleanup(path: Path) -> None:
    try:
        parent = path.parent.resolve(strict=True)
        parent_fd = os.open(
            parent,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
    except FileNotFoundError:
        return
    except OSError as error:
        print(f"warning: could not remove failed bootstrap evidence: {error}", file=sys.stderr)
        return
    try:
        expected = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(expected.st_mode) or expected.st_uid != os.getuid():
            return

        def remove_tree(directory_fd: int, label: str) -> None:
            with os.scandir(directory_fd) as entries:
                names = tuple(sorted(entry.name for entry in entries))
            for name in names:
                metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                    child = os.open(
                        name,
                        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
                        | getattr(os, "O_NOFOLLOW", 0),
                        dir_fd=directory_fd,
                    )
                    try:
                        opened = os.fstat(child)
                        if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino) or opened.st_uid != os.getuid():
                            raise BootstrapError(f"failed cleanup entry changed: {label}/{name}")
                        # macOS may attach ``com.apple.macl`` to copied app
                        # bundles and reject even a no-op chmod.  App-bundle
                        # directories are sealed owner-private and already
                        # writable specifically so failure cleanup can unlink
                        # their protected contents.
                        if stat.S_IMODE(opened.st_mode) & 0o700 != 0o700:
                            os.fchmod(
                                child, stat.S_IMODE(opened.st_mode) | 0o700
                            )
                        remove_tree(child, f"{label}/{name}")
                    finally:
                        os.close(child)
                    current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                    if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
                        raise BootstrapError(f"failed cleanup entry was replaced: {label}/{name}")
                    os.rmdir(name, dir_fd=directory_fd)
                elif stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
                    current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                    if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
                        raise BootstrapError(f"failed cleanup entry was replaced: {label}/{name}")
                    os.unlink(name, dir_fd=directory_fd)
                else:
                    raise BootstrapError(f"failed cleanup refuses special entry: {label}/{name}")

        root_fd = os.open(
            path.name,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        try:
            opened = os.fstat(root_fd)
            if (opened.st_dev, opened.st_ino) != (expected.st_dev, expected.st_ino):
                return
            if stat.S_IMODE(opened.st_mode) & 0o700 != 0o700:
                os.fchmod(root_fd, stat.S_IMODE(opened.st_mode) | 0o700)
            remove_tree(root_fd, path.name)
        finally:
            os.close(root_fd)
        current = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        if (current.st_dev, current.st_ino) != (expected.st_dev, expected.st_ino):
            raise BootstrapError("failed cleanup root was replaced")
        os.rmdir(path.name, dir_fd=parent_fd)
    except (FileNotFoundError, OSError, BootstrapError) as error:
        print(f"warning: could not remove failed bootstrap evidence: {error}", file=sys.stderr)
    finally:
        os.close(parent_fd)


def bootstrap(args: argparse.Namespace) -> int:
    if not sys.flags.isolated or not sys.flags.no_site:
        raise BootstrapError(
            "bootstrap must be started by protected Python with both -I and -S"
        )
    candidate = _absolute_resolved_existing(args.candidate_root, "candidate root")
    if not candidate.is_dir():
        raise BootstrapError("candidate root must be a directory")
    if _SAFE_PATH_RE.fullmatch(str(candidate)) is None:
        raise BootstrapError("candidate root must use the shell-safe release path alphabet")
    bootstrap_path = _absolute_resolved_existing(Path(__file__), "release bootstrap")
    if _inside(bootstrap_path, candidate):
        raise BootstrapError("release bootstrap must be installed outside the candidate root")

    protected_specs = (
        ("bootstrap", bootstrap_path, args.expected_bootstrap_sha256, _MAX_HELPER_BYTES, False),
        ("python", args.python_bin, args.expected_python_sha256, _MAX_TOOL_BYTES, True),
        ("git", args.git_bin, args.expected_git_sha256, _MAX_TOOL_BYTES, True),
        ("ssh_keygen", args.ssh_keygen_bin, args.expected_ssh_keygen_sha256, _MAX_TOOL_BYTES, True),
        ("bash", args.bash_bin, args.expected_bash_sha256, _MAX_TOOL_BYTES, True),
        (
            "manifest_helper",
            args.manifest_helper,
            args.expected_manifest_helper_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "identity_verifier",
            args.identity_verifier,
            args.expected_identity_verifier_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "receipt_validator",
            args.receipt_validator,
            args.expected_receipt_validator_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "receipt_validator_support",
            args.receipt_validator_support,
            args.expected_receipt_validator_support_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "runtime_helper",
            args.runtime_helper,
            args.expected_runtime_helper_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "sdk_dependency_bundle_manifest",
            args.sdk_dependency_bundle_manifest,
            args.expected_sdk_dependency_bundle_manifest_sha256,
            _MAX_SDK_MANIFEST_BYTES,
            False,
        ),
        (
            "runner_tool_manifest",
            args.runner_tool_manifest,
            args.expected_runner_tool_manifest_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "allowed_signers",
            args.ssh_allowed_signers,
            args.expected_ssh_allowed_signers_sha256,
            _MAX_POLICY_BYTES,
            False,
        ),
        (
            "revocation",
            args.ssh_revocation_file,
            args.expected_ssh_revocation_sha256,
            _MAX_POLICY_BYTES,
            False,
        ),
    )
    protected: dict[str, FileSnapshot] = {}
    executable_labels = {"python", "git", "ssh_keygen", "bash"}
    for label, path, digest, maximum, executable in protected_specs:
        protected[label] = _protected_snapshot(
            path,
            digest,
            label.replace("_", " "),
            candidate=candidate,
            maximum_bytes=maximum,
            executable=executable,
        )
    if protected["python"].path != Path(sys.executable).resolve(strict=True):
        raise BootstrapError("bootstrap must already be running under the protected Python")
    if not protected["allowed_signers"].data:
        raise BootstrapError("SSH allowed-signers policy must not be empty")
    if _FINGERPRINT_RE.fullmatch(args.expected_signer_fingerprint) is None:
        raise BootstrapError("expected signer fingerprint is invalid")

    runner_path = candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_snapshot = _read_file(
        runner_path,
        "signed candidate release runner",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    if not _inside(runner_snapshot.path, candidate):
        raise BootstrapError("candidate release runner escaped the candidate root")
    runner_tool_sources = _load_runner_tool_manifest(
        protected["runner_tool_manifest"], candidate
    )
    runner_extra_environment = _parse_runner_environment(args.runner_environment)
    cancellation_request_path = _cancellation_control_path(
        runner_extra_environment, candidate
    )

    evidence, evidence_fd = _prepare_evidence_directory(args.evidence_dir, candidate)
    evidence_directory_stat = os.fstat(evidence_fd)
    success = False
    retained_failure_cleanup: DirectorySnapshot | None = None
    runner_stdout_descriptor: int | None = None
    runner_stderr_descriptor: int | None = None
    runner_logs: dict[str, LargeFileSnapshot] = {}
    try:
        for child in ("home", "tmp", "runner-tools"):
            os.mkdir(child, _DIRECTORY_MODE, dir_fd=evidence_fd)
        os.mkdir("runner-bin", _DIRECTORY_MODE, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
        runner_stdout_path = evidence / "runner-stdout.log"
        runner_stderr_path = evidence / "runner-stderr.log"
        runner_stdout_descriptor = _open_runner_log(
            evidence_fd, runner_stdout_path.name
        )
        runner_stderr_descriptor = _open_runner_log(
            evidence_fd, runner_stderr_path.name
        )
        archive_names = {
            "bootstrap": "trusted-bootstrap.py",
            "python": (
                "python-runtime/bin/python3"
                if _FRAMEWORK_PYTHON
                else "python3"
            ),
            "git": "git",
            "ssh_keygen": "ssh-keygen",
            "bash": "bash",
            "manifest_helper": "compute-manifest.py",
            "identity_verifier": "verify-identity.py",
            "receipt_validator": "validate-receipt.py",
            "receipt_validator_support": "sumeragi_v2_localnet_manifest.py",
            "runtime_helper": "copy-release-runtime.py",
            "sdk_dependency_bundle_manifest": (
                "sdk-dependency-bundle-manifest.json"
            ),
            "runner_tool_manifest": "runner-tool-manifest.json",
            "allowed_signers": "bootstrap-allowed-signers",
            "revocation": "bootstrap-revocation",
        }
        archives: dict[str, FileSnapshot] = {}
        for label, source in protected.items():
            if label == "python" and _FRAMEWORK_PYTHON:
                continue
            mode = _TOOL_MODE if label in executable_labels else _DATA_MODE
            archives[label] = _write_artifact(
                evidence, evidence_fd, archive_names[label], source.data, mode
            )
        receipt_component_sources: dict[str, FileSnapshot] = {}
        receipt_component_archives: dict[str, FileSnapshot] = {}
        component_presence = {
            name: os.path.lexists(protected["receipt_validator"].path.with_name(name))
            for name in _RECEIPT_VALIDATOR_COMPONENT_SHA256
        }
        if any(component_presence.values()):
            if not all(component_presence.values()):
                raise BootstrapError(
                    "protected receipt validator component closure is incomplete"
                )
            for name, expected_digest in sorted(
                _RECEIPT_VALIDATOR_COMPONENT_SHA256.items()
            ):
                source = _read_file(
                    protected["receipt_validator"].path.with_name(name),
                    f"protected receipt validator component {name}",
                    maximum_bytes=_MAX_HELPER_BYTES,
                )
                if source.sha256 != expected_digest:
                    raise BootstrapError(
                        f"protected receipt validator component {name} has the wrong digest"
                    )
                receipt_component_sources[name] = source
                component_archive = _write_artifact(
                    evidence, evidence_fd, name, source.data, _DATA_MODE
                )
                receipt_component_archives[name] = component_archive
                archives[
                    "receipt_validator_component_"
                    + name.removesuffix(".py").replace("-", "_")
                ] = component_archive
        framework_python_inventory: FileSnapshot | None = None
        framework_python_record: dict[str, Any] | None = None
        if _FRAMEWORK_PYTHON:
            (
                archives["python"],
                framework_python_inventory,
                framework_python_record,
            ) = _copy_framework_python_archive(
                evidence=evidence,
                protected_python=protected["python"],
                runtime_helper=archives["runtime_helper"],
                timeout_seconds=args.command_timeout_seconds,
            )
            _verify_framework_python_archive(
                evidence=evidence,
                protected_python=protected["python"],
                runtime_helper=archives["runtime_helper"],
                inventory=framework_python_inventory,
                marker_record=framework_python_record,
                timeout_seconds=args.command_timeout_seconds,
            )

        runner_tools = evidence / "runner-tools"
        runner_tools_flags = (
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            runner_tools_flags |= os.O_NOFOLLOW
        runner_tools_fd = os.open(runner_tools, runner_tools_flags)
        try:
            runner_tool_archives = {
                name: _write_artifact(
                    runner_tools, runner_tools_fd, name, source.data, _TOOL_MODE
                )
                for name, source in sorted(runner_tool_sources.items())
            }
        finally:
            os.close(runner_tools_fd)

        runner_bin = evidence / "runner-bin"
        runner_bin_flags = (
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            runner_bin_flags |= os.O_NOFOLLOW
        runner_bin_fd = os.open(runner_bin, runner_bin_flags)
        try:
            runner_tool_aliases: dict[str, SymlinkSnapshot] = {}
            for name, archive in sorted(runner_tool_archives.items()):
                relative_target = os.path.relpath(archive.path, runner_bin)
                os.symlink(relative_target, name, dir_fd=runner_bin_fd)
                os.fsync(runner_bin_fd)
                runner_tool_aliases[name] = _runner_alias_snapshot(
                    runner_bin / name, archive.path, f"runner tool alias {name}"
                )
        finally:
            os.close(runner_bin_fd)

        environment = _closed_environment(
            evidence,
            [
                *([archives["python"].path.parent] if _FRAMEWORK_PYTHON else []),
                runner_bin,
            ],
        )
        _require_command_resolution(
            "git", archives["git"].path, environment, "archived Git"
        )
        _require_command_resolution(
            "python3", archives["python"].path, environment, "archived Python"
        )
        _require_command_resolution(
            "bash", archives["bash"].path, environment, "archived Bash"
        )
        python_probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
        python_probe = _run_bounded(
            archives["python"].path,
            ["-I", "-S", "-c", python_probe_code],
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        expected_python_stdout = f"{archives['python'].path}\n".encode()
        if (
            python_probe.returncode != 0
            or python_probe.stdout != expected_python_stdout
            or python_probe.stderr
        ):
            raise BootstrapError(
                "archived protected Python did not report its archived executable"
            )
        bash_probe = _run_bounded(
            archives["bash"].path,
            ["-c", ":"],
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        if bash_probe.returncode != 0 or bash_probe.stdout or bash_probe.stderr:
            raise BootstrapError("archived protected Bash is not relocatable")
        identity_bytes, identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        identity_snapshot = _write_artifact(
            evidence, evidence_fd, "candidate-identity.json", identity_bytes, _DATA_MODE
        )

        identity_outputs = {
            "attestation": evidence / "identity-attestation.json",
            "transcript": evidence / "identity-transcript.json",
            "raw_commit": evidence / "identity-raw-commit",
            "cargo_lock": evidence / "identity-Cargo.lock",
            "allowed": evidence / "identity-allowed-signers",
            "revocation": evidence / "identity-revocation",
            "git": evidence / "identity-git",
            "ssh": evidence / "identity-ssh-keygen",
        }
        identity_private_directory = (
            evidence / f".identity-private.{secrets.token_hex(16)}"
        )
        os.mkdir(identity_private_directory.name, _DIRECTORY_MODE, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
        identity_private_outputs = {
            label: identity_private_directory / path.name
            for label, path in identity_outputs.items()
        }
        identity_private_outputs["provenance"] = (
            identity_private_directory / "bootstrap-private-provenance.json"
        )
        verifier_arguments = [
            "-I",
            "-S",
            str(archives["identity_verifier"].path),
            "--root", str(candidate),
            "--identity", str(identity_snapshot.path),
            "--git-bin", str(archives["git"].path),
            "--original-git-path", str(protected["git"].path),
            "--expected-git-sha256", protected["git"].sha256,
            "--ssh-keygen-bin", str(archives["ssh_keygen"].path),
            "--original-ssh-keygen-path", str(protected["ssh_keygen"].path),
            "--expected-ssh-keygen-sha256", protected["ssh_keygen"].sha256,
            "--expected-signer-fingerprint", args.expected_signer_fingerprint,
            "--ssh-allowed-signers", str(archives["allowed_signers"].path),
            "--original-ssh-allowed-signers-path",
            str(protected["allowed_signers"].path),
            "--expected-ssh-allowed-signers-sha256", protected["allowed_signers"].sha256,
            "--ssh-revocation-file", str(archives["revocation"].path),
            "--original-ssh-revocation-path", str(protected["revocation"].path),
            "--expected-ssh-revocation-sha256", protected["revocation"].sha256,
            "--attestation-output", str(identity_private_outputs["attestation"]),
            "--bootstrap-private-provenance-output",
            str(identity_private_outputs["provenance"]),
            "--verify-transcript-output", str(identity_private_outputs["transcript"]),
            "--raw-commit-output", str(identity_private_outputs["raw_commit"]),
            "--cargo-lock-output", str(identity_private_outputs["cargo_lock"]),
            "--ssh-allowed-signers-output", str(identity_private_outputs["allowed"]),
            "--ssh-revocation-output", str(identity_private_outputs["revocation"]),
            "--git-archive-output", str(identity_private_outputs["git"]),
            "--ssh-keygen-archive-output", str(identity_private_outputs["ssh"]),
        ]
        verifier = _run_bounded(
            archives["python"].path,
            verifier_arguments,
            cwd=evidence,
            environment=environment,
            timeout_seconds=args.command_timeout_seconds,
            maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
        )
        if verifier.returncode != 0:
            detail = verifier.stderr.decode("utf-8", "replace").strip()
            raise BootstrapError(f"trusted identity verifier rejected candidate: {detail}")
        if verifier.stdout or verifier.stderr:
            raise BootstrapError("trusted identity verifier emitted unexpected output")

        private_identity_snapshots = {
            label: _read_file(
                path,
                f"bootstrap-private identity {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                executable=label in {"git", "ssh"},
            )
            for label, path in identity_private_outputs.items()
        }
        expected_private_modes = {
            label: _TOOL_MODE if label in {"git", "ssh"} else _DATA_MODE
            for label in identity_private_outputs
        }
        for label, snapshot in private_identity_snapshots.items():
            if snapshot.mode != expected_private_modes[label]:
                raise BootstrapError(
                    f"bootstrap-private identity {label} has the wrong mode"
                )
        _validate_private_identity_provenance(
            private_identity_snapshots["provenance"],
            identity=identity,
            identity_snapshot=identity_snapshot,
            candidate=candidate,
            private_outputs=identity_private_outputs,
            private_snapshots=private_identity_snapshots,
            protected=protected,
        )
        identity_copy_labels = {
            "attestation": "attestation",
            "transcript": "transcript",
            "raw_commit": "raw_commit",
            "cargo_lock": "cargo_lock",
            "allowed": "allowed",
            "revocation": "revocation",
            "git": "git",
            "ssh": "ssh",
        }
        for label, private_label in identity_copy_labels.items():
            mode = _TOOL_MODE if label in {"git", "ssh"} else _DATA_MODE
            _write_artifact(
                evidence,
                evidence_fd,
                identity_outputs[label].name,
                private_identity_snapshots[private_label].data,
                mode,
            )
        _cleanup(identity_private_directory)
        if os.path.lexists(identity_private_directory):
            raise BootstrapError(
                "bootstrap-private identity provenance could not be pruned"
            )
        os.fsync(evidence_fd)

        for label, snapshot in protected.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                label.replace("_", " "),
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        for label, snapshot in archives.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                f"archived {label.replace('_', ' ')}",
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        if framework_python_inventory is not None:
            assert framework_python_record is not None
            _verify_framework_python_archive(
                evidence=evidence,
                protected_python=protected["python"],
                runtime_helper=archives["runtime_helper"],
                inventory=framework_python_inventory,
                marker_record=framework_python_record,
                timeout_seconds=args.command_timeout_seconds,
            )
        _revalidate_runner_tools(
            runner_tool_sources, runner_tool_archives, runner_tool_aliases
        )
        _revalidate_receipt_validator_components(
            receipt_component_sources, receipt_component_archives
        )
        evidence_snapshots, identity_attestation, identity_transcript = (
            _validate_identity_evidence(
            evidence,
            identity,
            identity_bytes,
            {
                "git": protected["git"].sha256,
                "ssh": protected["ssh_keygen"].sha256,
                "allowed": protected["allowed_signers"].sha256,
                "revocation": protected["revocation"].sha256,
                "fingerprint": args.expected_signer_fingerprint,
            },
            )
        )
        recomputed_bytes, recomputed_identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        if recomputed_bytes != identity_bytes or recomputed_identity != identity:
            raise BootstrapError("candidate identity changed after authentication")
        _require_unchanged(
            runner_snapshot,
            "signed candidate release runner",
            maximum_bytes=_MAX_HELPER_BYTES,
        )

        completion_path = evidence / "BOOTSTRAP_COMPLETED.json"
        policy_environment_without_self_digest = {
            "SUMERAGI_V2_RELEASE_RUNTIME_HELPER": str(
                archives["runtime_helper"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": protected[
                "runtime_helper"
            ].sha256,
            "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(archives["ssh_keygen"].path),
            "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": protected["git"].sha256,
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": protected[
                "ssh_keygen"
            ].sha256,
            "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": (
                args.expected_signer_fingerprint
            ),
            "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(
                archives["allowed_signers"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": (
                protected["allowed_signers"].sha256
            ),
            "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(
                archives["revocation"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": protected[
                "revocation"
            ].sha256,
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(completion_path),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(
                identity_outputs["attestation"]
            ),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(
                identity_outputs["transcript"]
            ),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(identity_snapshot.path),
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(evidence),
        }
        alias_environment_without_self_digest = {
            key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): value
            for key, value in policy_environment_without_self_digest.items()
            if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
        }
        alias_environment_without_self_digest.update({
            "IROHA_RELEASE_RUNTIME_HELPER": str(archives["runtime_helper"].path),
            "IROHA_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": protected[
                "runtime_helper"
            ].sha256,
            "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST": str(
                archives["sdk_dependency_bundle_manifest"].path
            ),
            "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256": (
                protected["sdk_dependency_bundle_manifest"].sha256
            ),
        })
        runner_environment_without_self_digest = _closed_environment(
            evidence,
            [
                *([archives["python"].path.parent] if _FRAMEWORK_PYTHON else []),
                runner_bin,
            ],
            {
                **runner_extra_environment,
                **policy_environment_without_self_digest,
                **alias_environment_without_self_digest,
            },
        )
        self_digest_variables = [
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
        ]

        trusted_input_records = {
            label: _artifact_record(label, archives[label])
            for label in sorted(protected)
        }
        if framework_python_record is not None:
            trusted_input_records["python"] = {
                **trusted_input_records["python"],
                "archive_name": "python-runtime/bin/python3",
                "runtime": framework_python_record,
            }
        if receipt_component_archives:
            trusted_input_records["receipt_validator"] = {
                **trusted_input_records["receipt_validator"],
                "components": {
                    name: {
                        "archive_id": (
                            "release-bootstrap.receipt-validator-component.v1:"
                            + name
                        ),
                        "archive_name": name,
                        "mode": f"{snapshot.mode:04o}",
                        "sha256": snapshot.sha256,
                        "size_bytes": snapshot.size,
                    }
                    for name, snapshot in sorted(
                        receipt_component_archives.items()
                    )
                },
            }
        marker_value = {
            "schema_version": 2,
            "trust_boundary": {
                "bootstrap_authentication": "external prerequisite",
                "release_image_and_dynamic_loader": "external prerequisite",
                "same_uid_and_trusted_ancestor_owners": True,
            },
            "candidate_identity": identity,
            "candidate_identity_sha256": identity_snapshot.sha256,
            "trusted_inputs": trusted_input_records,
            "identity_verification": {
                label: {
                    "archive_name": snapshot.path.name,
                    "mode": f"{snapshot.mode:04o}",
                    "sha256": snapshot.sha256,
                    "size_bytes": len(snapshot.data),
                }
                for label, snapshot in sorted(evidence_snapshots.items())
            },
            "runner": {
                "archive_id": "release-candidate.runner.v1",
                "invocation": {
                    "profile": "release",
                    "operation_id": "sumeragi-v2.release.v1",
                    "arguments": ["--release"],
                    "bash_archive_id": "release-bootstrap.bash.v1",
                },
                "closed_path_resolution": {
                    "bash": "release-bootstrap.bash.v1",
                    "git": "release-bootstrap.git.v1",
                    "python3": "release-bootstrap.python.v1",
                },
                "environment_sha256": hashlib.sha256(
                    _canonical_json(runner_environment_without_self_digest)
                ).hexdigest(),
                "mode": f"{runner_snapshot.mode:04o}",
                "output": {
                    "stderr_archive_id": "release-bootstrap.runner-stderr.v1",
                    "stderr_name": runner_stderr_path.name,
                    "stdout_archive_id": "release-bootstrap.runner-stdout.v1",
                    "stdout_name": runner_stdout_path.name,
                    "active_mode": "0600",
                    "sealed_mode": "0400",
                },
                "tool_directory": "runner-bin",
                "tools": {
                    name: _runner_tool_record(
                        name, runner_tool_archives[name], runner_tool_aliases[name]
                    )
                    for name in sorted(runner_tool_sources)
                },
                "self_digest_environment_variables": self_digest_variables,
                "sha256": runner_snapshot.sha256,
                "size_bytes": len(runner_snapshot.data),
            },
            "trusted_execution_probes": {
                "bash": {
                    "argv": [str(archives["bash"].path), "-c", ":"],
                    "exit_status": bash_probe.returncode,
                },
                "python": {
                    "argv": [
                        str(archives["python"].path),
                        "-I",
                        "-S",
                        "-c",
                        python_probe_code,
                    ],
                    "expected_executable": (
                        "python-runtime/bin/python3"
                        if _FRAMEWORK_PYTHON
                        else "python3"
                    ),
                    "exit_status": python_probe.returncode,
                    "stdout_sha256": hashlib.sha256(
                        python_probe.stdout
                    ).hexdigest(),
                    "stdout_size_bytes": len(python_probe.stdout),
                },
            },
        }
        marker = _publish_completion_marker(
            evidence,
            evidence_fd,
            _canonical_json(marker_value),
        )
        runner_environment = {
            **runner_environment_without_self_digest,
            self_digest_variables[0]: marker.sha256,
            self_digest_variables[1]: marker.sha256,
        }
        assert runner_stdout_descriptor is not None
        assert runner_stderr_descriptor is not None
        runner = _run_release_runner(
            archives["bash"].path,
            [str(runner_path), "--release"],
            cwd=candidate,
            environment=runner_environment,
            stdout_descriptor=runner_stdout_descriptor,
            stderr_descriptor=runner_stderr_descriptor,
        )
        runner_status = runner.returncode if runner.returncode >= 0 else 128 - runner.returncode
        runner_logs = {
            "stdout": _seal_runner_log(
                runner_stdout_descriptor,
                runner_stdout_path,
                "release runner stdout log",
            ),
            "stderr": _seal_runner_log(
                runner_stderr_descriptor,
                runner_stderr_path,
                "release runner stderr log",
            ),
        }
        os.close(runner_stdout_descriptor)
        runner_stdout_descriptor = None
        os.close(runner_stderr_descriptor)
        runner_stderr_descriptor = None
        os.fsync(evidence_fd)

        post_error: BootstrapError | None = None
        try:
            for label, snapshot in protected.items():
                maximum = _protected_size_limit(label, executable_labels)
                _require_unchanged(
                    snapshot,
                    label.replace("_", " "),
                    maximum_bytes=maximum,
                    executable=label in executable_labels,
                )
            for label, snapshot in archives.items():
                maximum = _protected_size_limit(label, executable_labels)
                _require_unchanged(
                    snapshot,
                    f"archived {label.replace('_', ' ')}",
                    maximum_bytes=maximum,
                    executable=label in executable_labels,
                )
            if framework_python_inventory is not None:
                assert framework_python_record is not None
                _verify_framework_python_archive(
                    evidence=evidence,
                    protected_python=protected["python"],
                    runtime_helper=archives["runtime_helper"],
                    inventory=framework_python_inventory,
                    marker_record=framework_python_record,
                    timeout_seconds=args.command_timeout_seconds,
                )
            _revalidate_runner_tools(
                runner_tool_sources, runner_tool_archives, runner_tool_aliases
            )
            _revalidate_receipt_validator_components(
                receipt_component_sources, receipt_component_archives
            )
            _require_unchanged(
                identity_snapshot,
                "candidate identity evidence",
                maximum_bytes=_MAX_IDENTITY_BYTES,
            )
            for label, snapshot in evidence_snapshots.items():
                _require_unchanged(
                    snapshot,
                    f"identity evidence {label}",
                    maximum_bytes=_MAX_EVIDENCE_BYTES,
                    executable=snapshot.mode == _TOOL_MODE,
                )
            _require_unchanged(
                marker,
                "bootstrap completion marker",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
            )
            _require_unchanged(
                runner_snapshot,
                "signed candidate release runner",
                maximum_bytes=_MAX_HELPER_BYTES,
            )
            final_bytes, final_identity = _compute_identity(
                archives["python"].path,
                archives["manifest_helper"].path,
                candidate,
                environment,
                args.command_timeout_seconds,
            )
            if final_bytes != identity_bytes or final_identity != identity:
                raise BootstrapError("candidate identity changed while the signed runner executed")
            directory_stat = os.fstat(evidence_fd)
            pathname_stat = evidence.lstat()
            if (
                not stat.S_ISDIR(pathname_stat.st_mode)
                or (directory_stat.st_dev, directory_stat.st_ino)
                != (evidence_directory_stat.st_dev, evidence_directory_stat.st_ino)
                or (pathname_stat.st_dev, pathname_stat.st_ino)
                != (evidence_directory_stat.st_dev, evidence_directory_stat.st_ino)
                or stat.S_IMODE(directory_stat.st_mode) != _DIRECTORY_MODE
                or stat.S_IMODE(pathname_stat.st_mode) != _DIRECTORY_MODE
                or directory_stat.st_uid != os.getuid()
                or pathname_stat.st_uid != os.getuid()
            ):
                raise BootstrapError(
                    "bootstrap evidence directory changed while the runner executed"
                )
        except BootstrapError as error:
            post_error = error
        except OSError as error:
            post_error = BootstrapError(
                f"post-run bootstrap evidence became unavailable: {error}"
            )

        if runner_status == _RECEIPT_VALIDATION_FAILED_STATUS:
            if post_error is not None:
                raise post_error
            failure_marker, failure_streams = _receipt_validation_failure(
                evidence,
                evidence_fd,
                identity,
                identity_snapshot,
                marker,
                protected["receipt_validator"],
            )
            _prune_receipt_validation_failure(
                evidence, evidence_fd, failure_marker, failure_streams
            )
            success = True
            try:
                print(
                    "protected receipt validation failed; bounded diagnostics: "
                    f"{failure_marker.path} sha256={failure_marker.sha256}",
                    file=sys.stderr,
                )
            except OSError:
                pass
            return 2
        if runner_status == _COOPERATIVE_CANCELLED_STATUS:
            if post_error is not None:
                raise post_error
            cancelled = _publish_cancellation_result(
                evidence=evidence,
                evidence_fd=evidence_fd,
                request_path=cancellation_request_path,
                candidate=candidate,
                identity=identity,
                identity_snapshot=identity_snapshot,
                bootstrap_marker=marker,
                runner_snapshot=runner_snapshot,
                runner_logs=runner_logs,
            )
            success = True
            try:
                print(
                    "Sumeragi v2 release cancelled cooperatively after natural "
                    f"runner completion: {cancelled.path} sha256={cancelled.sha256}",
                    file=sys.stderr,
                )
            except OSError:
                pass
            return _COOPERATIVE_CANCELLED_STATUS
        if runner_status != 0:
            if post_error is not None:
                print(f"post-run bootstrap validation also failed: {post_error}", file=sys.stderr)
            return runner_status
        if post_error is not None:
            raise post_error

        (
            retained_release_root,
            retained_receipt_path,
            retained_identity_path,
            retained_result_snapshot,
            retained_inventory_snapshot,
            retained_validation_ack,
        ) = _retained_release_layout(
            evidence,
            evidence_fd,
            candidate=candidate,
            authenticated_environment=runner_environment_without_self_digest,
        )
        if retained_result_snapshot is None or retained_validation_ack is None:
            raise BootstrapError("production release lacks protected retained result and validator acknowledgment")
        if retained_result_snapshot is not None:
            retained_failure_cleanup = _private_directory_snapshot(
                retained_release_root, "retained release cleanup root"
            )
        (
            terminal_receipt,
            terminal_receipt_value,
            terminal_artifacts,
            terminal_directories,
        ) = _validate_terminal_receipt(
            evidence=evidence,
            candidate=candidate,
            bootstrap_marker=marker,
            bootstrap_sha256=protected["bootstrap"].sha256,
            identity_snapshot=identity_snapshot,
            identity=identity,
            runner_snapshot=runner_snapshot,
            runner_record=marker_value["runner"],
            protected=protected,
            identity_attestation=identity_attestation,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            authenticated_environment=runner_environment_without_self_digest,
            release_runner=retained_release_root,
            receipt_path=retained_receipt_path,
        )
        _require_unchanged(
            terminal_receipt,
            "terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        for index, directory in enumerate(terminal_directories):
            _require_terminal_directory_unchanged(
                directory, f"terminal release directory {index}"
            )
        for index, artifact in enumerate(terminal_artifacts):
            _require_large_file_unchanged(
                artifact, f"terminal release artifact {index}"
            )
        sealed_identity_snapshot, sealed_identity, sealed_directory = (
            _validate_retained_source(
                evidence=evidence,
                receipt=terminal_receipt_value,
                candidate_identity=identity,
                python=archives["python"].path,
                manifest_helper=archives["manifest_helper"].path,
                environment=runner_environment,
                timeout_seconds=args.command_timeout_seconds,
                release_runner=retained_release_root,
                sealed_identity_path=retained_identity_path,
            )
        )
        # The runner invokes the protected receipt validator before publishing
        # its acknowledgment. Replay that same protected validator here from
        # the bootstrap-owned archive after the retained source has been
        # authenticated.  The snapshots checked immediately below make any
        # mutation performed by a compromised validator fail closed rather
        # than allowing the validator to authenticate its own changes.
        _run_protected_receipt_validator(
            evidence=evidence,
            candidate=candidate,
            receipt=terminal_receipt_value,
            receipt_snapshot=terminal_receipt,
            sealed_identity_snapshot=sealed_identity_snapshot,
            sealed_root=sealed_directory.path,
            archives=archives,
            protected=protected,
            identity_snapshot=identity_snapshot,
            identity_outputs=identity_outputs,
            bootstrap_marker=marker,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            environment=runner_environment,
            timeout_seconds=args.command_timeout_seconds,
        )
        ack = _parse_canonical_json(retained_validation_ack, "receipt validation acknowledgment")
        if ack["validator"]["bootstrap_completion_sha256"] != marker.sha256:
            raise BootstrapError("receipt validation acknowledgment names the wrong bootstrap")
        for snapshot, label in (
            (retained_result_snapshot, "protected outer release result"),
            (retained_inventory_snapshot, "protected retained release inventory"),
            (retained_validation_ack, "protected receipt validation acknowledgment"),
        ):
            if snapshot is not None:
                _require_unchanged(snapshot, label, maximum_bytes=max(snapshot.size, 1))
        _require_unchanged(
            terminal_receipt,
            "protected-validator terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        _require_unchanged(
            sealed_identity_snapshot,
            "protected-validator sealed identity",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        _require_sealed_directory_unchanged(
            sealed_directory, "protected-validator retained sealed source"
        )
        for index, artifact in enumerate(terminal_artifacts):
            _require_large_file_unchanged(
                artifact, f"protected-validator terminal release artifact {index}"
            )
        for index, directory in enumerate(terminal_directories):
            _require_terminal_directory_unchanged(
                directory, f"protected-validator terminal release directory {index}"
            )
        release_completion_value = {
            "schema_version": 2,
            "result": "release-complete",
            "bootstrap_completion_sha256": marker.sha256,
            "candidate_identity_sha256": identity_snapshot.sha256,
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "runner": {
                "archive_id": "release-candidate.runner.v1",
                "sha256": runner_snapshot.sha256,
                "mode": f"{runner_snapshot.mode:04o}",
                "logs": {
                    label: {
                        "archive_id": f"release-bootstrap.runner-{label}.v1",
                        "sha256": snapshot.sha256,
                        "size_bytes": snapshot.size,
                        "mode": f"{snapshot.mode:04o}",
                    }
                    for label, snapshot in sorted(runner_logs.items())
                },
            },
            "retained_source": {
                "archive_id": "release-retained.source.v1",
                "identity_archive_id": "release-retained.identity.v1",
                "identity_sha256": sealed_identity_snapshot.sha256,
                "source_manifest_sha256": sealed_identity[
                    "workspace_source_manifest_sha256"
                ],
                "mode": f"{sealed_directory.mode:04o}",
            },
            "receipt_validator": {
                "archive_id": "release-bootstrap.receipt-validator.v1",
                "sha256": protected["receipt_validator"].sha256,
                "exit_status": ack["exit_status"],
                "ack_archive_id": "release-retained.receipt-validation-ack.v3",
                "ack_sha256": (
                    retained_validation_ack.sha256
                    if retained_validation_ack is not None else None
                ),
            },
            "terminal_receipt": {
                "archive_id": "release-terminal.receipt.v1",
                "sha256": terminal_receipt.sha256,
                "size_bytes": terminal_receipt.size,
                "mode": f"{terminal_receipt.mode:04o}",
            },
        }
        release_completion = _publish_completion_marker(
            evidence,
            evidence_fd,
            _canonical_json(release_completion_value),
            final_name="BOOTSTRAP_RELEASE_COMPLETED.json",
        )
        if (
            release_completion.mode != _DATA_MODE
            or release_completion.owner != os.getuid()
            or release_completion.nlink != 1
        ):
            raise BootstrapError("external release completion marker metadata is not exact")

        # Close the publication window: success is returned only if both the
        # receipt and every trust input still match the snapshots that produced
        # the external no-clobber marker.
        _require_unchanged(
            terminal_receipt,
            "terminal release receipt",
            maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
        )
        for index, directory in enumerate(terminal_directories):
            _require_terminal_directory_unchanged(
                directory, f"terminal release directory {index}"
            )
        for index, artifact in enumerate(terminal_artifacts):
            _require_large_file_unchanged(
                artifact, f"terminal release artifact {index}"
            )
        for label, snapshot in protected.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                label.replace("_", " "),
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        for label, snapshot in archives.items():
            maximum = _protected_size_limit(label, executable_labels)
            _require_unchanged(
                snapshot,
                f"archived {label.replace('_', ' ')}",
                maximum_bytes=maximum,
                executable=label in executable_labels,
            )
        _revalidate_runner_tools(
            runner_tool_sources, runner_tool_archives, runner_tool_aliases
        )
        _revalidate_receipt_validator_components(
            receipt_component_sources, receipt_component_archives
        )
        _require_unchanged(
            identity_snapshot,
            "candidate identity evidence",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        for label, snapshot in evidence_snapshots.items():
            _require_unchanged(
                snapshot,
                f"identity evidence {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                executable=snapshot.mode == _TOOL_MODE,
            )
        _require_unchanged(
            marker,
            "bootstrap completion marker",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
        )
        _require_unchanged(
            runner_snapshot,
            "signed candidate release runner",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        for label, snapshot in runner_logs.items():
            _require_large_file_unchanged(
                snapshot, f"release runner {label} log"
            )
        _require_unchanged(
            sealed_identity_snapshot,
            "retained sealed identity",
            maximum_bytes=_MAX_IDENTITY_BYTES,
        )
        _require_sealed_directory_unchanged(
            sealed_directory, "retained sealed source root"
        )
        final_bytes, final_identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        if final_bytes != identity_bytes or final_identity != identity:
            raise BootstrapError(
                "candidate identity changed during external completion publication"
            )
        success = True
        try:
            print(
                "Sumeragi v2 external release completion: "
                f"{release_completion.path} sha256={release_completion.sha256}",
                file=sys.stderr,
            )
        except OSError:
            # The no-clobber marker is the authoritative result; a detached or
            # closed diagnostic stream must not turn durable success into an
            # ambiguous failed invocation.
            pass
        return 0
    finally:
        for descriptor in (runner_stdout_descriptor, runner_stderr_descriptor):
            if descriptor is not None:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        try:
            os.close(evidence_fd)
        except OSError:
            if success:
                raise BootstrapError("could not close successful bootstrap evidence")
        if not success:
            if retained_failure_cleanup is not None:
                try:
                    if _private_directory_snapshot(
                        retained_failure_cleanup.path,
                        "retained release cleanup root",
                    ) == retained_failure_cleanup:
                        _cleanup(retained_failure_cleanup.path)
                except (BootstrapError, OSError):
                    pass
            _cleanup(evidence)


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--expected-bootstrap-sha256", required=True)
    parser.add_argument("--python-bin", type=Path, required=True)
    parser.add_argument("--expected-python-sha256", required=True)
    parser.add_argument("--git-bin", type=Path, required=True)
    parser.add_argument("--expected-git-sha256", required=True)
    parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--manifest-helper", type=Path, required=True)
    parser.add_argument("--expected-manifest-helper-sha256", required=True)
    parser.add_argument("--identity-verifier", type=Path, required=True)
    parser.add_argument("--expected-identity-verifier-sha256", required=True)
    parser.add_argument("--receipt-validator", type=Path, required=True)
    parser.add_argument("--expected-receipt-validator-sha256", required=True)
    parser.add_argument("--receipt-validator-support", type=Path, required=True)
    parser.add_argument(
        "--expected-receipt-validator-support-sha256", required=True
    )
    parser.add_argument("--runtime-helper", type=Path, required=True)
    parser.add_argument("--expected-runtime-helper-sha256", required=True)
    parser.add_argument(
        "--sdk-dependency-bundle-manifest", type=Path, required=True
    )
    parser.add_argument(
        "--expected-sdk-dependency-bundle-manifest-sha256", required=True
    )
    parser.add_argument("--runner-tool-manifest", type=Path, required=True)
    parser.add_argument("--expected-runner-tool-manifest-sha256", required=True)
    parser.add_argument("--bash-bin", type=Path, required=True)
    parser.add_argument("--expected-bash-sha256", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--ssh-allowed-signers", type=Path, required=True)
    parser.add_argument("--expected-ssh-allowed-signers-sha256", required=True)
    parser.add_argument("--ssh-revocation-file", type=Path, required=True)
    parser.add_argument("--expected-ssh-revocation-sha256", required=True)
    parser.add_argument("--runner-environment", action="append", default=[])
    parser.add_argument(
        "--command-timeout-seconds",
        type=_positive_int,
        default=_DEFAULT_COMMAND_TIMEOUT_SECONDS,
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        return bootstrap(args)
    except BootstrapError as error:
        print(f"Sumeragi v2 release bootstrap failed: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"Sumeragi v2 release bootstrap failed closed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
