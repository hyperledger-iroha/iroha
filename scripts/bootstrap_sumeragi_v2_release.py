#!/usr/bin/env python3
"""Authenticate a Sumeragi v2 release candidate before running its code.

This file is a trust root, not part of the candidate trust chain.  A release
operator MUST install it outside the candidate checkout and authenticate its
bytes together with every digest-pinned adjacent bootstrap component before
starting Python.  The bootstrap's checks of its own and component digests are
useful evidence, but cannot make an untrusted bootstrap closure trustworthy.
Invoke it with the protected interpreter as
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
import types
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
_RUNNER_TOOL_PROBE_OPERATION_IDS = {
    "awk": "release-tool.awk-program.v1",
    "basename": "release-tool.basename-path.v1",
    "cargo": "release-tool.cargo-version.v1",
    "cargo-verus": "release-tool.cargo-verus-help.v1",
    "cat": "release-tool.cat-file.v1",
    "chmod": "release-tool.chmod-mode.v1",
    "cmp": "release-tool.cmp-different-quiet.v1",
    "cp": "release-tool.cp-file.v1",
    "cut": "release-tool.cut-byte.v1",
    "diff": "release-tool.diff-different-brief.v1",
    "dirname": "release-tool.dirname-path.v1",
    "env": "release-tool.env-closed.v1",
    "find": "release-tool.find-file.v1",
    "git-index-pack": "release-tool.git-index-pack-empty.v1",
    "git-upload-pack": "release-tool.git-upload-pack-missing.v1",
    "grep": "release-tool.grep-exact.v1",
    "java": "release-tool.java-version.v1",
    "ln": "release-tool.ln-hardlink.v1",
    "ls": "release-tool.ls-entry.v1",
    "mkdir": "release-tool.mkdir-directory.v1",
    "mkfifo": "release-tool.mkfifo-fifo.v1",
    "mktemp": "release-tool.mktemp-file.v1",
    "mv": "release-tool.mv-file.v1",
    "node": "release-tool.node-exec-path.v1",
    "openssl": "release-tool.openssl-sha256.v1",
    "rm": "release-tool.rm-file.v1",
    "rmdir": "release-tool.rmdir-directory.v1",
    "rustc": "release-tool.rustc-version.v1",
    "sed": "release-tool.sed-first-line.v1",
    "sh": "release-tool.sh-builtin-output.v1",
    ("shasum" if sys.platform == "darwin" else "sha256sum"): (
        "release-tool.shasum-empty.v1"
        if sys.platform == "darwin"
        else "release-tool.sha256sum-empty.v1"
    ),
    "sleep": "release-tool.sleep-duration.v1",
    "swift": "release-tool.swift-version.v1",
    "tail": "release-tool.tail-last-line.v1",
    "tee": "release-tool.tee-file.v1",
    "tlapm": "release-tool.tlapm-version.v1",
    "tr": "release-tool.tr-byte.v1",
    "uname": "release-tool.uname-system.v1",
    "verus": "release-tool.verus-version.v1",
    "wc": "release-tool.wc-empty.v1",
    "xargs": "release-tool.xargs-protected-shell.v1",
}
_RECEIPT_VALIDATOR_COMPONENT_SHA256 = {
    "write_sumeragi_v2_release_receipt_corridor_log.py": (
        "6ff2d5337414bbbf74a9530cc1b2bd59bc62141a82a1319fa2a270b84e64ce8c"
    ),
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "43a815d4257ad6296a48e125dfab52c5f31aabba5210f4154641164887e48886"
    ),
    "write_sumeragi_v2_release_receipt_gate_evidence.py": (
        "dd67a4f7b7c321238bd08789cb54fb7704c3e309c9f1764baea275ff64a5e5ae"
    ),
    "write_sumeragi_v2_release_receipt_publication.py": (
        "d5f666eab695c3ca4668a3a3e1074a53b8fc63aac3d852036d0c20622e027b45"
    ),
}
_BOOTSTRAP_COMPONENT_FILES = (
    "bootstrap_sumeragi_v2_release_receipt_replay.py",
)
_BOOTSTRAP_COMPONENT_SHA256 = {
    "bootstrap_sumeragi_v2_release_receipt_replay.py": (
        "e336273e2a4322d125344b6bd5162fdd1a9dcfce874aa49497a03c30141bfd8b"
    ),
}
_APPROVAL_CLASS_IDS = (
    "offline-toolchain-sdk",
    "formal-proof-tools",
    "network-scale-soak",
    "final-bootstrap-publication",
)
_APPROVAL_INPUT_LABELS = {
    class_id: "approval_" + class_id.replace("-", "_")
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_ARCHIVE_NAMES = {
    class_id: f"{class_id}.approval.v1.json"
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_ATTESTATION_NAMES = {
    class_id: f"{class_id}.approval-attestation.v1.json"
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_SET_ATTESTATION_NAME = "release-approval-set-attestation.v1.json"
_APPROVAL_SET_ARCHIVE_ID = "release-approval.set-attestation.v1"
_APPROVAL_PRIVATE_PROVENANCE_FORMAT = (
    "iroha-sumeragi-v2-bootstrap-private-approval-provenance"
)
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
    "runtime_tool_probes",
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
    "--runtime-tool-probe-manifest",
    "--runtime-tool-probe-result",
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
        "--runtime-tool-probe-manifest",
        "--runtime-tool-probe-result",
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
        "--runtime-tool-probe-manifest": (
            "path", str(release_runner / "runtime-tool-probe-manifest.json")
        ),
        "--runtime-tool-probe-result": (
            "path", str(release_runner / "runtime-tool-probe-result.json")
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


def _execute_bootstrap_component(
    snapshot: FileSnapshot,
    filename: str,
) -> None:
    """Execute one digest-authenticated bootstrap component from captured bytes."""

    if (
        filename not in _BOOTSTRAP_COMPONENT_FILES
        or Path(filename).name != filename
        or set(_BOOTSTRAP_COMPONENT_FILES) != set(_BOOTSTRAP_COMPONENT_SHA256)
        or len(_BOOTSTRAP_COMPONENT_FILES) != len(_BOOTSTRAP_COMPONENT_SHA256)
        or snapshot.path.name != filename
        or snapshot.sha256 != _BOOTSTRAP_COMPONENT_SHA256[filename]
        or snapshot.size > _MAX_HELPER_BYTES
    ):
        raise BootstrapError(f"bootstrap component binding is invalid: {filename}")
    try:
        source = snapshot.data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError(
            f"bootstrap component is not UTF-8: {filename}"
        ) from error
    exec(
        compile(
            source,
            f"<release-bootstrap-component:{filename}>",
            "exec",
        ),
        globals(),
    )


_BOOTSTRAP_COMPONENT_SOURCES: dict[str, FileSnapshot] = {}
_BOOTSTRAP_SOURCE_DIRECTORY = Path(__file__).resolve(strict=True).parent
for _bootstrap_component_name in _BOOTSTRAP_COMPONENT_FILES:
    _bootstrap_component_snapshot = _read_file(
        _BOOTSTRAP_SOURCE_DIRECTORY / _bootstrap_component_name,
        f"protected bootstrap component {_bootstrap_component_name}",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    _execute_bootstrap_component(
        _bootstrap_component_snapshot,
        _bootstrap_component_name,
    )
    _BOOTSTRAP_COMPONENT_SOURCES[_bootstrap_component_name] = (
        _bootstrap_component_snapshot
    )
del _bootstrap_component_name, _bootstrap_component_snapshot




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


def _validate_tool_probe_result(
    value: Any,
    tools: dict[str, Any],
    *,
    archive_id_prefix: str,
) -> dict[str, Any]:
    """Authenticate one path-free result for the exact 41-command closure."""

    value = _require_exact_json_fields(
        value,
        {
            "format",
            "host_family",
            "probe_contract_sha256",
            "schema_version",
            "tool_count",
            "tools",
        },
        "release tool functional probes",
    )
    expected_host = "darwin" if sys.platform == "darwin" else "linux"
    results = value["tools"]
    if (
        value["format"]
        != "iroha-sumeragi-v2-release-tool-functional-probes"
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or value["host_family"] != expected_host
        or type(value["tool_count"]) is not int
        or value["tool_count"] != 41
        or set(tools) != _REQUIRED_RUNNER_TOOL_NAMES
        or set(_RUNNER_TOOL_PROBE_OPERATION_IDS)
        != _REQUIRED_RUNNER_TOOL_NAMES
        or not isinstance(results, dict)
        or set(results) != _REQUIRED_RUNNER_TOOL_NAMES
        or _DIGEST_RE.fullmatch(
            str(value["probe_contract_sha256"])
        )
        is None
    ):
        raise BootstrapError(
            "release tool functional probe inventory is not exact"
        )
    record_keys = {
        "archive_id",
        "exit_status",
        "invocation_sha256",
        "mode",
        "operation_id",
        "postcondition_sha256",
        "sha256",
        "size_bytes",
        "stderr_sha256",
        "stderr_size_bytes",
        "stdout_sha256",
        "stdout_size_bytes",
    }
    for name in sorted(tools):
        tool = tools[name]
        if isinstance(tool, dict):
            tool_size = tool.get("size_bytes")
            tool_sha256 = tool.get("sha256")
        else:
            tool_size = getattr(tool, "size", None)
            tool_sha256 = getattr(tool, "sha256", None)
        record = _require_exact_json_fields(
            results[name], record_keys, f"release tool probe {name}"
        )
        expected_status = (
            128
            if name in {"git-index-pack", "git-upload-pack"}
            else 1
            if name in {"cmp", "diff"}
            else 0
        )
        if (
            record["archive_id"] != f"{archive_id_prefix}.{name}.v1"
            or record["operation_id"]
            != _RUNNER_TOOL_PROBE_OPERATION_IDS[name]
            or record["mode"] != "0500"
            or type(record["exit_status"]) is not int
            or record["exit_status"] != expected_status
            or type(record["size_bytes"]) is not int
            or record["size_bytes"] != tool_size
            or record["sha256"] != tool_sha256
            or any(
                not isinstance(record[field], str)
                or _DIGEST_RE.fullmatch(record[field]) is None
                for field in (
                    "invocation_sha256",
                    "postcondition_sha256",
                    "sha256",
                    "stderr_sha256",
                    "stdout_sha256",
                )
            )
            or any(
                type(record[field]) is not int
                or not 0 <= record[field] <= 64 * 1024
                for field in ("stderr_size_bytes", "stdout_size_bytes")
            )
        ):
            raise BootstrapError(
                f"release tool functional probe {name} is not exact"
            )
    return value


def _run_tool_probe_closure(
    *,
    evidence: Path,
    evidence_fd: int,
    python: FileSnapshot,
    helper: FileSnapshot,
    tools: dict[str, FileSnapshot],
    timeout_seconds: int,
) -> tuple[FileSnapshot, FileSnapshot, dict[str, Any]]:
    """Probe copied runner tools before any signed-candidate code executes."""

    manifest_value = {
        "schema_version": 1,
        "tools": {
            name: {
                "archive_id": f"release-runner-tool.{name}.v1",
                "path": str(tools[name].path),
                "sha256": tools[name].sha256,
            }
            for name in sorted(tools)
        },
    }
    manifest = _write_artifact(
        evidence,
        evidence_fd,
        "runner-tool-probe-manifest.json",
        _canonical_json(manifest_value),
        _DATA_MODE,
    )
    probe_root = evidence / ".runner-tool-probe"
    result = _run_bounded(
        python.path,
        [
            "-I",
            "-S",
            str(helper.path),
            "--tool-manifest",
            str(manifest.path),
            "--expected-tool-manifest-sha256",
            manifest.sha256,
            "--probe-root",
            str(probe_root),
        ],
        cwd=evidence,
        environment=_closed_environment(evidence, [python.path.parent]),
        timeout_seconds=max(timeout_seconds, 41 * 10 + 30),
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    if result.returncode != 0 or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            f"protected runner-tool functional probes failed: {detail}"
        )
    try:
        value = json.loads(result.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootstrapError(
            "protected runner-tool functional probes returned invalid JSON"
        ) from error
    value = _validate_tool_probe_result(
        value, tools, archive_id_prefix="release-runner-tool"
    )
    if result.stdout != _canonical_json(value) or probe_root.exists() or probe_root.is_symlink():
        raise BootstrapError(
            "protected runner-tool functional probe result is not canonical"
        )
    result_snapshot = _write_artifact(
        evidence,
        evidence_fd,
        "runner-tool-probes.json",
        result.stdout,
        _DATA_MODE,
    )
    return manifest, result_snapshot, value


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


def _revalidate_bootstrap_components(
    sources: dict[str, FileSnapshot],
    archives: dict[str, FileSnapshot],
) -> None:
    """Reauthenticate the exact external and bootstrap-owned component bytes."""

    if (
        set(sources) != set(_BOOTSTRAP_COMPONENT_SHA256)
        or set(archives) != set(_BOOTSTRAP_COMPONENT_SHA256)
    ):
        raise BootstrapError("bootstrap component inventory changed")
    for name, expected_digest in sorted(_BOOTSTRAP_COMPONENT_SHA256.items()):
        _require_unchanged(
            sources[name],
            f"protected bootstrap component {name}",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        _require_unchanged(
            archives[name],
            f"archived bootstrap component {name}",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        if (
            sources[name].sha256 != expected_digest
            or archives[name].sha256 != expected_digest
            or sources[name].data != archives[name].data
        ):
            raise BootstrapError(f"bootstrap component {name} changed")


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
            "tool_probe_helper",
            args.tool_probe_helper,
            args.expected_tool_probe_helper_sha256,
            _MAX_HELPER_BYTES,
            False,
        ),
        (
            "approval_contract",
            args.approval_contract,
            args.expected_approval_contract_sha256,
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
    approval_source_module = _load_release_approval_contract(
        protected["approval_contract"]
    )
    approval_source_paths = {
        "offline-toolchain-sdk": args.offline_toolchain_sdk_approval,
        "formal-proof-tools": args.formal_proof_tools_approval,
        "network-scale-soak": args.network_scale_soak_approval,
        "final-bootstrap-publication": args.final_bootstrap_publication_approval,
    }
    try:
        source_approvals = approval_source_module.load_protected_release_approval_set(
            {
                approval_source_module.ReleaseApprovalClass(class_id): path
                for class_id, path in approval_source_paths.items()
            },
            expected_owner_uid=os.getuid(),
        )
    except approval_source_module.ReleaseApprovalError as error:
        raise BootstrapError(f"protected release approval rejected: {error}") from error
    source_approvals_by_class = {
        approval.class_id.value: approval for approval in source_approvals
    }
    approval_source_inodes: set[tuple[int, int]] = set()
    for class_id in _APPROVAL_CLASS_IDS:
        approval = source_approvals_by_class[class_id]
        label = _APPROVAL_INPUT_LABELS[class_id]
        snapshot = _protected_snapshot(
            approval_source_paths[class_id],
            approval.approval_sha256,
            f"{class_id} release approval",
            candidate=candidate,
            maximum_bytes=approval_source_module.MAX_APPROVAL_BYTES,
        )
        if (
            snapshot.data != approval.canonical_bytes
            or snapshot.mode != _DATA_MODE
            or snapshot.nlink != 1
            or snapshot.owner != os.getuid()
        ):
            raise BootstrapError(
                f"{class_id} release approval source metadata is not exact"
            )
        inode = (snapshot.device, snapshot.inode)
        if inode in approval_source_inodes:
            raise BootstrapError("release approval sources share one inode")
        approval_source_inodes.add(inode)
        protected[label] = snapshot
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
            "tool_probe_helper": "probe-release-tools.py",
            "approval_contract": "release-approval-contract.py",
            "sdk_dependency_bundle_manifest": (
                "sdk-dependency-bundle-manifest.json"
            ),
            "runner_tool_manifest": "runner-tool-manifest.json",
            "allowed_signers": "bootstrap-allowed-signers",
            "revocation": "bootstrap-revocation",
            **{
                _APPROVAL_INPUT_LABELS[class_id]: archive_name
                for class_id, archive_name in _APPROVAL_ARCHIVE_NAMES.items()
            },
        }
        archives: dict[str, FileSnapshot] = {}
        for label, source in protected.items():
            if label == "python" and _FRAMEWORK_PYTHON:
                continue
            mode = _TOOL_MODE if label in executable_labels else _DATA_MODE
            archives[label] = _write_artifact(
                evidence, evidence_fd, archive_names[label], source.data, mode
            )
        bootstrap_component_archives: dict[str, FileSnapshot] = {}
        for name, source in sorted(_BOOTSTRAP_COMPONENT_SOURCES.items()):
            _require_unchanged(
                source,
                f"protected bootstrap component {name}",
                maximum_bytes=_MAX_HELPER_BYTES,
            )
            component_archive = _write_artifact(
                evidence, evidence_fd, name, source.data, _DATA_MODE
            )
            bootstrap_component_archives[name] = component_archive
            archives[
                "bootstrap_component_" + name.removesuffix(".py").replace("-", "_")
            ] = component_archive
            _execute_bootstrap_component(component_archive, name)
        _revalidate_bootstrap_components(
            _BOOTSTRAP_COMPONENT_SOURCES, bootstrap_component_archives
        )
        receipt_component_sources: dict[str, FileSnapshot] = {}
        receipt_component_archives: dict[str, FileSnapshot] = {}
        component_presence = {
            name: os.path.lexists(protected["receipt_validator"].path.with_name(name))
            for name in _RECEIPT_VALIDATOR_COMPONENT_SHA256
        }
        if not all(component_presence.values()):
            raise BootstrapError(
                "protected receipt validator component closure is incomplete"
            )
        if component_presence:
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
        component_source_paths = tuple(
            str(snapshot.path)
            for snapshot in (
                protected["bootstrap"],
                protected["receipt_validator"],
                *_BOOTSTRAP_COMPONENT_SOURCES.values(),
                *receipt_component_sources.values(),
            )
        )
        component_private_directory = (
            evidence / f".component-private.{secrets.token_hex(16)}"
        )
        os.mkdir(
            component_private_directory.name,
            _DIRECTORY_MODE,
            dir_fd=evidence_fd,
        )
        os.fsync(evidence_fd)
        component_private_fd = os.open(
            component_private_directory,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            component_private_value = {
                "format": (
                    "iroha-sumeragi-v2-bootstrap-private-component-provenance"
                ),
                "schema_version": 1,
                "parents": {
                    "bootstrap": {
                        "source_path": str(protected["bootstrap"].path),
                        "archive_name": archives["bootstrap"].path.name,
                        "sha256": protected["bootstrap"].sha256,
                    },
                    "receipt_validator": {
                        "source_path": str(protected["receipt_validator"].path),
                        "archive_name": archives["receipt_validator"].path.name,
                        "sha256": protected["receipt_validator"].sha256,
                    },
                },
                "components": {
                    "bootstrap": {
                        name: {
                            "source_path": str(source.path),
                            "archive_name": bootstrap_component_archives[
                                name
                            ].path.name,
                            "mode": f"{bootstrap_component_archives[name].mode:04o}",
                            "sha256": source.sha256,
                            "size_bytes": source.size,
                        }
                        for name, source in sorted(
                            _BOOTSTRAP_COMPONENT_SOURCES.items()
                        )
                    },
                    "receipt_validator": {
                        name: {
                            "source_path": str(source.path),
                            "archive_name": receipt_component_archives[
                                name
                            ].path.name,
                            "mode": f"{receipt_component_archives[name].mode:04o}",
                            "sha256": source.sha256,
                            "size_bytes": source.size,
                        }
                        for name, source in sorted(
                            receipt_component_sources.items()
                        )
                    },
                },
            }
            component_private_provenance = _write_artifact(
                component_private_directory,
                component_private_fd,
                "bootstrap-private-component-provenance.json",
                _canonical_json(component_private_value),
                _DATA_MODE,
            )
        finally:
            os.close(component_private_fd)
        if _parse_canonical_json(
            component_private_provenance,
            "bootstrap-private component provenance",
        ) != component_private_value:
            raise BootstrapError(
                "bootstrap-private component provenance is not exact"
            )
        _cleanup(component_private_directory)
        if os.path.lexists(component_private_directory):
            raise BootstrapError(
                "bootstrap-private component provenance could not be pruned"
            )
        os.fsync(evidence_fd)
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

        (
            runner_tool_probe_manifest,
            runner_tool_probe_result,
            runner_tool_probe_value,
        ) = _run_tool_probe_closure(
            evidence=evidence,
            evidence_fd=evidence_fd,
            python=archives["python"],
            helper=archives["tool_probe_helper"],
            tools=runner_tool_archives,
            timeout_seconds=args.command_timeout_seconds,
        )

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

        approval_durations = _approval_duration_values(args)
        source_approval_expectations = _approval_expectations(
            approval_source_module,
            identity=identity,
            protected_tool_manifest_sha256=protected[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=args.approval_evidence_root_id,
            durations=approval_durations,
        )
        source_approvals = _load_bound_release_approvals(
            approval_source_module,
            approval_source_paths,
            source_approval_expectations,
        )
        archived_approval_module = _load_release_approval_contract(
            archives["approval_contract"]
        )
        archived_approval_paths = {
            class_id: archives[_APPROVAL_INPUT_LABELS[class_id]].path
            for class_id in _APPROVAL_CLASS_IDS
        }
        archived_approval_expectations = _approval_expectations(
            archived_approval_module,
            identity=identity,
            protected_tool_manifest_sha256=protected[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=args.approval_evidence_root_id,
            durations=approval_durations,
        )
        archived_approvals = _load_bound_release_approvals(
            archived_approval_module,
            archived_approval_paths,
            archived_approval_expectations,
        )
        if tuple(value.canonical_bytes for value in source_approvals) != tuple(
            value.canonical_bytes for value in archived_approvals
        ):
            raise BootstrapError(
                "archived release approvals differ from their protected inputs"
            )
        approval_attestations: dict[str, FileSnapshot] = {}
        for approval in archived_approvals:
            class_id = approval.class_id.value
            sanitized = approval.sanitized_archive()
            approval_attestations[class_id] = _write_artifact(
                evidence,
                evidence_fd,
                _APPROVAL_ATTESTATION_NAMES[class_id],
                sanitized.canonical_bytes,
                _DATA_MODE,
            )
        sanitized_approval_set = (
            archived_approval_module.sanitized_release_approval_set_archive(
                archived_approvals
            )
        )
        approval_set_attestation = _write_artifact(
            evidence,
            evidence_fd,
            _APPROVAL_SET_ATTESTATION_NAME,
            sanitized_approval_set.canonical_bytes,
            _DATA_MODE,
        )
        approval_marker_record = {
            "format": archived_approval_module.APPROVAL_SET_ARCHIVE_FORMAT,
            "schema_version": 1,
            "candidate_oid": identity["head_commit"],
            "candidate_tree": identity["head_tree"],
            "protected_tool_manifest_sha256": protected[
                "runner_tool_manifest"
            ].sha256,
            "evidence_root_id": args.approval_evidence_root_id,
            "expected_duration_seconds": approval_durations,
            "operation_plan_sha256": {
                approval_class.value: digest
                for approval_class, digest in (
                    archived_approval_module.APPROVAL_OPERATION_PLAN_SHA256.items()
                )
            },
            "class_attestations": {
                approval.class_id.value: _approval_archive_record(
                    approval_attestations[approval.class_id.value],
                    archive_id=(
                        archived_approval_module.APPROVAL_ARCHIVE_IDS[
                            approval.class_id
                        ]
                    ),
                    archive_name=_APPROVAL_ATTESTATION_NAMES[
                        approval.class_id.value
                    ],
                )
                for approval in archived_approvals
            },
            "set_attestation": _approval_archive_record(
                approval_set_attestation,
                archive_id=_APPROVAL_SET_ARCHIVE_ID,
                archive_name=_APPROVAL_SET_ATTESTATION_NAME,
            ),
        }
        _replay_release_approval_evidence(
            module=archived_approval_module,
            approval_paths=archived_approval_paths,
            identity=identity,
            protected_tool_manifest_sha256=protected[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=args.approval_evidence_root_id,
            durations=approval_durations,
            attestation_snapshots=approval_attestations,
            set_attestation_snapshot=approval_set_attestation,
            marker_record=approval_marker_record,
        )
        approval_private_directory = (
            evidence / f".approval-private.{secrets.token_hex(16)}"
        )
        os.mkdir(
            approval_private_directory.name,
            _DIRECTORY_MODE,
            dir_fd=evidence_fd,
        )
        os.fsync(evidence_fd)
        approval_private_fd = os.open(
            approval_private_directory,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            private_approval_provenance_value = {
                "format": _APPROVAL_PRIVATE_PROVENANCE_FORMAT,
                "schema_version": 1,
                "approval_contract": {
                    "source_path": str(protected["approval_contract"].path),
                    "archive_name": archives["approval_contract"].path.name,
                    "sha256": protected["approval_contract"].sha256,
                },
                "approvals": {
                    class_id: {
                        "source_path": str(
                            protected[_APPROVAL_INPUT_LABELS[class_id]].path
                        ),
                        "archive_name": archived_approval_paths[class_id].name,
                        "sha256": protected[
                            _APPROVAL_INPUT_LABELS[class_id]
                        ].sha256,
                    }
                    for class_id in _APPROVAL_CLASS_IDS
                },
                "sanitized": {
                    "class_attestations": {
                        class_id: _APPROVAL_ATTESTATION_NAMES[class_id]
                        for class_id in _APPROVAL_CLASS_IDS
                    },
                    "set_attestation": _APPROVAL_SET_ATTESTATION_NAME,
                },
            }
            approval_private_provenance = _write_artifact(
                approval_private_directory,
                approval_private_fd,
                "bootstrap-private-approval-provenance.json",
                _canonical_json(private_approval_provenance_value),
                _DATA_MODE,
            )
        finally:
            os.close(approval_private_fd)
        if _parse_canonical_json(
            approval_private_provenance,
            "bootstrap-private release approval provenance",
        ) != private_approval_provenance_value:
            raise BootstrapError(
                "bootstrap-private release approval provenance is not exact"
            )
        disclosed_paths = tuple(
            str(protected[label].path)
            for label in (
                "approval_contract",
                *(
                    _APPROVAL_INPUT_LABELS[class_id]
                    for class_id in _APPROVAL_CLASS_IDS
                ),
            )
        )
        public_approval_bytes = (
            *(snapshot.data for snapshot in approval_attestations.values()),
            approval_set_attestation.data,
            _canonical_json(approval_marker_record),
        )
        if any(
            path.encode("utf-8") in data
            for path in disclosed_paths
            for data in public_approval_bytes
        ):
            raise BootstrapError(
                "sanitized release approval evidence discloses an original path"
            )
        _cleanup(approval_private_directory)
        if os.path.lexists(approval_private_directory):
            raise BootstrapError(
                "bootstrap-private release approval provenance could not be pruned"
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
        _revalidate_bootstrap_components(
            _BOOTSTRAP_COMPONENT_SOURCES, bootstrap_component_archives
        )
        _replay_release_approval_evidence(
            module=archived_approval_module,
            approval_paths=archived_approval_paths,
            identity=identity,
            protected_tool_manifest_sha256=protected[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=args.approval_evidence_root_id,
            durations=approval_durations,
            attestation_snapshots=approval_attestations,
            set_attestation_snapshot=approval_set_attestation,
            marker_record=approval_marker_record,
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
            "SUMERAGI_V2_RELEASE_TOOL_PROBE_HELPER": str(
                archives["tool_probe_helper"].path
            ),
            "SUMERAGI_V2_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": protected[
                "tool_probe_helper"
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
            "IROHA_RELEASE_TOOL_PROBE_HELPER": str(
                archives["tool_probe_helper"].path
            ),
            "IROHA_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": protected[
                "tool_probe_helper"
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
        trusted_input_records["bootstrap"] = {
            **trusted_input_records["bootstrap"],
            "components": {
                name: {
                    "archive_id": (
                        "release-bootstrap.bootstrap-component.v1:" + name
                    ),
                    "archive_name": name,
                    "mode": f"{snapshot.mode:04o}",
                    "sha256": snapshot.sha256,
                    "size_bytes": snapshot.size,
                }
                for name, snapshot in sorted(
                    bootstrap_component_archives.items()
                )
            },
        }
        if framework_python_record is not None:
            trusted_input_records["python"] = {
                **trusted_input_records["python"],
                "archive_name": "python-runtime/bin/python3",
                "runtime": framework_python_record,
            }
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
            "release_approvals": approval_marker_record,
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
                "runner_tool_closure": {
                    "manifest": _artifact_record(
                        "runner_tool_probe_manifest",
                        runner_tool_probe_manifest,
                    ),
                    "result": _artifact_record(
                        "runner_tool_probes", runner_tool_probe_result
                    ),
                    "value": runner_tool_probe_value,
                },
            },
        }
        marker_bytes = _canonical_json(marker_value)
        if any(
            path.encode("utf-8") in marker_bytes
            for path in component_source_paths
        ):
            raise BootstrapError(
                "sanitized bootstrap component evidence discloses an original path"
            )
        marker = _publish_completion_marker(
            evidence,
            evidence_fd,
            marker_bytes,
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
            _revalidate_runner_tools(
                runner_tool_sources, runner_tool_archives, runner_tool_aliases
            )
            _revalidate_receipt_validator_components(
                receipt_component_sources, receipt_component_archives
            )
            _revalidate_bootstrap_components(
                _BOOTSTRAP_COMPONENT_SOURCES, bootstrap_component_archives
            )
            _replay_release_approval_evidence(
                module=archived_approval_module,
                approval_paths=archived_approval_paths,
                identity=identity,
                protected_tool_manifest_sha256=protected[
                    "runner_tool_manifest"
                ].sha256,
                evidence_root_id=args.approval_evidence_root_id,
                durations=approval_durations,
                attestation_snapshots=approval_attestations,
                set_attestation_snapshot=approval_set_attestation,
                marker_record=approval_marker_record,
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
            approval_record=approval_marker_record,
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
        _replay_release_approval_evidence(
            module=archived_approval_module,
            approval_paths=archived_approval_paths,
            identity=identity,
            protected_tool_manifest_sha256=protected[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=args.approval_evidence_root_id,
            durations=approval_durations,
            attestation_snapshots=approval_attestations,
            set_attestation_snapshot=approval_set_attestation,
            marker_record=approval_marker_record,
        )
        release_completion_value = {
            "schema_version": 2,
            "result": "release-complete",
            "bootstrap_completion_sha256": marker.sha256,
            "candidate_identity_sha256": identity_snapshot.sha256,
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "release_approvals": {
                "archive_id": _APPROVAL_SET_ARCHIVE_ID,
                "sha256": approval_set_attestation.sha256,
                "operation_plan_sha256": approval_marker_record[
                    "operation_plan_sha256"
                ],
            },
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
        final_bytes, final_identity = _compute_identity(
            archives["python"].path,
            archives["manifest_helper"].path,
            candidate,
            environment,
            args.command_timeout_seconds,
        )
        if final_bytes != identity_bytes or final_identity != identity:
            raise BootstrapError(
                "candidate identity changed before external completion"
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
        _revalidate_bootstrap_components(
            _BOOTSTRAP_COMPONENT_SOURCES, bootstrap_component_archives
        )
        for class_id, snapshot in approval_attestations.items():
            _require_unchanged(
                snapshot,
                f"sanitized release approval {class_id}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
            )
        _require_unchanged(
            approval_set_attestation,
            "sanitized release approval set",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
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
    parser.add_argument("--tool-probe-helper", type=Path, required=True)
    parser.add_argument("--expected-tool-probe-helper-sha256", required=True)
    parser.add_argument("--approval-contract", type=Path, required=True)
    parser.add_argument("--expected-approval-contract-sha256", required=True)
    parser.add_argument(
        "--offline-toolchain-sdk-approval", type=Path, required=True
    )
    parser.add_argument("--formal-proof-tools-approval", type=Path, required=True)
    parser.add_argument("--network-scale-soak-approval", type=Path, required=True)
    parser.add_argument(
        "--final-bootstrap-publication-approval", type=Path, required=True
    )
    parser.add_argument("--approval-evidence-root-id", required=True)
    parser.add_argument(
        "--offline-toolchain-sdk-duration-seconds",
        type=_positive_int,
        required=True,
    )
    parser.add_argument(
        "--formal-proof-tools-duration-seconds",
        type=_positive_int,
        required=True,
    )
    parser.add_argument(
        "--network-scale-soak-duration-seconds",
        type=_positive_int,
        required=True,
    )
    parser.add_argument(
        "--final-bootstrap-publication-duration-seconds",
        type=_positive_int,
        required=True,
    )
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
