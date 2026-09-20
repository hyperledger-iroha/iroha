#!/usr/bin/env python3
"""Authenticate a Sumeragi v2 release candidate before running its code.

This file is a trust root, not part of the candidate trust chain.  A release
operator MUST install it outside the candidate checkout and authenticate its
bytes together with every digest-pinned adjacent bootstrap component before
starting Python.  The bootstrap's checks of its own and component digests are
useful evidence, but cannot make an untrusted bootstrap closure trustworthy.
Invoke it with the protected interpreter as
``/absolute/python3 -I -B -S /absolute/bootstrap_sumeragi_v2_release.py ...``;
isolated, no-bytecode, no-site startup is enforced before candidate inspection.
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
import ast
import importlib
import importlib.machinery
import base64
import binascii
from dataclasses import dataclass
import hashlib
import fcntl
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import selectors
import select
import socket
import shutil
import stat
import subprocess
import sys
import sysconfig
import tarfile
import threading
import time
import types
from typing import Any, Iterable, Protocol


_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_FORMAL_REPLAY_PRINCIPAL_RE = re.compile(
    r"[A-Za-z0-9][A-Za-z0-9_.@+-]{0,127}"
)
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
        "b464b36f2ad4bf07c7ec969f14d97b7d29b99e27dd74b1f47ecd1dcaabf0014c"
    ),
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "2e997ee27e45fdf6651cd1e94689e08d348078e688ab34862d8d6396c6887ba5"
    ),
    "write_sumeragi_v2_release_receipt_gate_evidence.py": (
        "c881a4f0e313b7c00823f62fa2d4e766c04c3b365eeb1847423c7a30cba7a9f6"
    ),
    "write_sumeragi_v2_release_receipt_publication.py": (
        "d32bb675f783227f514f00f542ea5fef78507f08f4587fff684057fbe4c1ce9c"
    ),
}
_BOOTSTRAP_COMPONENT_FILES = (
    "bootstrap_sumeragi_v2_release_receipt_replay.py",
)
_BOOTSTRAP_COMPONENT_SHA256 = {
    "bootstrap_sumeragi_v2_release_receipt_replay.py": (
        "06e5d09c2971525119a68c874937547f47ed20d2a061f4738673a6c44b97d239"
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
    "IROHA_RELEASE_APALACHE_BIN",
    "IROHA_RELEASE_CANCEL_REQUEST_PATH",
    "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT",
    "IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256",
    "IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL",
    "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT",
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
    "formal_replay_release",
    "seed_matrix_completion",
    "seed_matrix_summary",
    "seed_matrix_run_logs",
    "seed_matrix_localnet_manifest_index",
    "seed_matrix_localnet_manifests",
    "chaos_completion",
    "chaos_log",
    "multilane_scaling",
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
    ("irohad_taira", "release/iroha3d_taira"),
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
_SCALING_SAFE_COMPONENT_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
_PREBUILT_INVOCATION_RE = re.compile(r"invocation\.[A-Za-z0-9]+")
_PREBUILT_TRIPLE_RE = re.compile(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_.]+)+")
_MAX_TERMINAL_ARTIFACT_BYTES = 4 * 1024 * 1024 * 1024
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
_DEFAULT_SCALING_PREFLIGHT_TIMEOUT_SECONDS = 28_800
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
    "--formal-replay-source-receipt",
    "--formal-replay-release-root",
    "--expected-formal-replay-signature-sha256",
    "--formal-replay-principal",
    "--seed-completion",
    "--chaos-completion",
    "--g4p-completion",
    "--g12-seed-completion",
    "--g12-fault-soak-completion",
    "--scaling-execution-record",
    "--expected-scaling-execution-sha256",
    "--sdk-dependency-archive",
    "--sdk-dependency-input-inventory",
    "--sdk-dependency-final-work-inventory",
    "--runtime-tool-probe-manifest",
    "--runtime-tool-probe-result",
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
        "--formal-replay-source-receipt",
        "--formal-replay-release-root",
        "--seed-completion",
        "--chaos-completion",
        "--g4p-completion",
        "--g12-seed-completion",
        "--g12-fault-soak-completion",
        "--scaling-execution-record",
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
        or value["python_flags"] != ["-I", "-B", "-S"]
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
    scaling_execution: FileSnapshot,
) -> dict[str, tuple[str, str | bool]]:
    """Reconstruct every validator value from authenticated terminal records."""

    try:
        authentication = receipt["authentication"]
        release_identity = authentication["release_identity"]
        bootstrap = authentication["bootstrap"]
        trust = release_identity["trust_policy"]
        receipt_evidence = receipt["evidence"]
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

    _scaling_require(type(scaling_execution) is FileSnapshot
        and scaling_execution.path == evidence/'scaling-execution.json'
        and scaling_execution.mode == _DATA_MODE)
    _require_unchanged(scaling_execution, 'original parent scaling record',
                      maximum_bytes=max(scaling_execution.size, 1))
    scaling = receipt_evidence.get('multilane_scaling')
    _scaling_require(type(scaling) is dict and scaling.get('parent_execution') == dict(
        archive_id='release-scaling.parent-execution.v1', sha256=scaling_execution.sha256,
        size_bytes=scaling_execution.size, mode='0400'))
    formal_replay = receipt_evidence.get("formal_replay_release")
    if (
        not isinstance(formal_replay, dict)
        or not isinstance(formal_replay.get("principal"), str)
        or not isinstance(formal_replay.get("signature"), dict)
        or not isinstance(formal_replay["signature"].get("sha256"), str)
    ):
        raise BootstrapError("terminal receipt formal replay release is malformed")
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
        "--formal-replay-source-receipt": (
            "path", artifact_path("formal_replay_release", "source_receipt")
        ),
        "--formal-replay-release-root": (
            "path",
            str(Path(artifact_path("formal_replay_release", "receipt")).parent),
        ),
        "--expected-formal-replay-signature-sha256": (
            "text", formal_replay["signature"]["sha256"]
        ),
        "--formal-replay-principal": ("text", formal_replay["principal"]),
        "--seed-completion": ("path", artifact_path("seed_matrix_completion")),
        "--chaos-completion": ("path", artifact_path("chaos_completion")),
        "--g4p-completion": (
            "path", artifact_path("g4p_multilane", "completion")
        ),
        "--g12-seed-completion": (
            "path", artifact_path("g12_cross_dataspace", "seed_completion")
        ),
        "--g12-fault-soak-completion": (
            "path", artifact_path("g12_cross_dataspace", "fault_soak_completion")
        ),
        "--scaling-execution-record": ("path", str(scaling_execution.path)),
        "--expected-scaling-execution-sha256": ("text", scaling_execution.sha256),
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


def _require_sdk_source_manifest_pruned(
    evidence_fd: int, snapshot: FileSnapshot
) -> None:
    """Require the bootstrap-private SDK source manifest to remain absent."""

    name = "sdk-dependency-bundle-manifest.json"
    if snapshot.path.name != name:
        raise BootstrapError("SDK source manifest archive binding is invalid")
    try:
        os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
    except FileNotFoundError:
        return
    except OSError as error:
        raise BootstrapError(
            "bootstrap-private SDK source manifest pruning is indeterminate"
        ) from error
    raise BootstrapError(
        "bootstrap-private SDK source manifest survived acknowledgment pruning"
    )


def _prune_authenticated_sdk_source_manifest(
    evidence_fd: int, snapshot: FileSnapshot
) -> None:
    """Prune the exact SDK source manifest after its authenticated handoff."""

    name = "sdk-dependency-bundle-manifest.json"
    if snapshot.path.name != name:
        raise BootstrapError("SDK source manifest archive binding is invalid")
    current = _read_file_at(
        evidence_fd,
        name,
        snapshot.path,
        "bootstrap-private SDK source manifest",
        maximum_bytes=_MAX_SDK_MANIFEST_BYTES,
    )
    if current != snapshot:
        raise BootstrapError(
            "bootstrap-private SDK source manifest changed before pruning"
        )
    try:
        os.unlink(name, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
    except OSError as error:
        raise BootstrapError(
            "could not prune bootstrap-private SDK source manifest"
        ) from error
    _require_sdk_source_manifest_pruned(evidence_fd, snapshot)


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


@dataclass(frozen=True)
class PassedDescriptor:
    """Original borrowed descriptor identity at this exact child launch."""

    number: int
    device: int
    inode: int
    mode: int
    owner: int
    flags: int


def _validated_pass_fds(descriptors: tuple[int, ...]) -> tuple[PassedDescriptor, ...]:
    if (type(descriptors) is not tuple or len(descriptors) > 2
            or any(type(fd) is not int or not 3 <= fd < (1 << 20) for fd in descriptors)
            or len(set(descriptors)) != len(descriptors)):
        raise BootstrapError("protected command descriptor set is invalid")
    result = []
    try:
        for fd in descriptors:
            info = os.fstat(fd)
            if os.get_inheritable(fd):
                raise BootstrapError("parent descriptors must be close-on-exec")
            result.append(PassedDescriptor(fd, info.st_dev, info.st_ino,
                info.st_mode, info.st_uid, fcntl.fcntl(fd, fcntl.F_GETFL)))
    except OSError as error:
        raise BootstrapError("protected command descriptor is unavailable") from error
    return tuple(result)


@dataclass(frozen=True)
class TerminalCommandObservation:
    """Actual original child outcome; violations never replace its return code."""

    pid: int
    argv: tuple[str, ...]
    cwd: str
    environment_sha256: str
    descriptors: tuple[PassedDescriptor, ...]
    started_ns: int
    deadline_ns: int
    completed_ns: int | None
    returncode: int
    stdout_sha256: str
    stderr_sha256: str
    stdout_bytes: int
    stderr_bytes: int
    violations: tuple[str, ...]


class CommandObservationOwner:
    """One-use in-memory owner populated only around the original Popen/wait."""

    def __init__(self) -> None:
        self._phase = "new"
        self._process = None
        self._terminal = None
        self._reaped = False

    @property
    def terminal(self) -> TerminalCommandObservation | None:
        """Return the observed terminal result, including rejected-limit exits."""
        return self._terminal

    def _begin(self, argv, cwd, environment, descriptors, started_ns, deadline_ns):
        if self._phase != "new":
            raise BootstrapError("command observation owner is already consumed")
        self._phase = "starting"
        self._input = (tuple(argv), str(cwd),
            hashlib.sha256(_canonical_json(environment)).hexdigest(),
            descriptors, started_ns, deadline_ns)

    def _spawned(self, process):
        self._process = process
        self._phase = "running"

    def _waited(self, process, returncode):
        if self._process is not process:
            raise BootstrapError("natural wait lost its original child")
        self._reaped = True
        self._returncode = returncode

    def _finish(self, process, completed_ns, returncode, digests, counts, violations):
        if self._phase != "running" or self._process is not process:
            raise BootstrapError("command observation lost its original child")
        argv, cwd, environment, descriptors, started_ns, deadline_ns = self._input
        self._terminal = TerminalCommandObservation(process.pid, argv, cwd,
            environment, descriptors, started_ns, deadline_ns, completed_ns,
            returncode, digests["stdout"].hexdigest(), digests["stderr"].hexdigest(),
            counts["stdout"], counts["stderr"], tuple(violations))
        self._phase = "terminal"


@dataclass(frozen=True)
class FixedScalingLaunch:
    """Trusted preparation output; no generic executable/argument dispatch."""

    python: Path
    source_root: Path
    argv: tuple[str, ...]
    launch_input_fd: int
    launch_input_sha256: str
    seed_fd: int
    cwd: Path
    environment: dict[str, str]
    timeout_seconds: int
    maximum_output_bytes: int
    evidence_root: Path
    manifest_max_bytes: int
    report_max_bytes: int
    original_started_ns: int
    deadline_ns: int


@dataclass(frozen=True)
class ScalingArtifactObservation:
    """Stable parent-retained artifact bytes after the actual collector exit."""

    relative_path: str
    sha256: str
    size_bytes: int
    mode: int


@dataclass(frozen=True)
class ParentScalingObservation:
    """Original process observation joined to retained manifest/report bytes."""

    invocation_sha256: str
    command: TerminalCommandObservation
    launch_input_sha256: str
    artifacts: tuple[ScalingArtifactObservation, ...]
    original_started_ns: int


class FixedScalingOperation(Protocol):
    """Trusted bootstrap integration contract; never deserialize an operation.

    The shipped bootstrap creates its sole lazy retained production operation.
    prepare owns source/runtime admission; validate retains those exact inputs
    through child completion; verify_publication checks complete canonical
    archive semantics; close releases them only after natural child completion.
    These callbacks do not supply an exit code or a caller-owned success record.
    """

    def prepare(self) -> FixedScalingLaunch: ...
    def validate(self, launch: FixedScalingLaunch) -> None: ...
    def child_finished(self, launch: FixedScalingLaunch,
                       observation: TerminalCommandObservation | None) -> None: ...
    def verify_publication(self, launch: FixedScalingLaunch,
                           observation: ParentScalingObservation) -> None: ...
    def close(self) -> None: ...


def _scaling_require(condition: bool) -> None:
    if not condition:
        raise BootstrapError("fixed scaling handoff failed")


def _scaling_launch_check(launch: FixedScalingLaunch) -> None:
    _scaling_require(type(launch) is FixedScalingLaunch)
    for value in (launch.python, launch.source_root, launch.cwd, launch.evidence_root):
        _scaling_require(type(value) is type(Path('/')) and value.is_absolute()
            and str(value) == os.path.abspath(value))
    _scaling_require(type(launch.launch_input_sha256) is str
        and _DIGEST_RE.fullmatch(launch.launch_input_sha256) is not None)
    expected = (str(launch.python), '-I', '-B', '-S',
        str(launch.source_root / 'scripts/nexus/run_multilane_scaling_gate.py'),
        '--launch-input-fd', str(launch.launch_input_fd),
        '--launch-input-sha256', launch.launch_input_sha256,
        '--seed-fd', str(launch.seed_fd))
    _scaling_require(type(launch.argv) is tuple and launch.argv == expected)
    _scaling_require(type(launch.environment) is dict and all(
        type(k) is str and type(v) is str for k, v in launch.environment.items()))
    for value, cap in ((launch.timeout_seconds, 30 * 24 * 3600),
            (launch.maximum_output_bytes, 16 * 1024 * 1024),
            (launch.manifest_max_bytes, 1024 * 1024),
            (launch.report_max_bytes, 1024 * 1024)):
        _scaling_require(type(value) is int and 0 < value <= cap)
    descriptors = _validated_pass_fds((launch.launch_input_fd, launch.seed_fd))
    _scaling_require(type(launch.original_started_ns) is int
        and type(launch.deadline_ns) is int and 0 < launch.original_started_ns < launch.deadline_ns
        and launch.deadline_ns - launch.original_started_ns <= launch.timeout_seconds * 1_000_000_000
        and launch.original_started_ns <= int(time.monotonic() * 1_000_000_000) < launch.deadline_ns)
    _scaling_require(stat.S_ISREG(descriptors[0].mode)
        and stat.S_ISFIFO(descriptors[1].mode)
        and all(row.flags & os.O_ACCMODE == os.O_RDONLY for row in descriptors)
        and descriptors[1].flags & os.O_NONBLOCK != 0)


def _scaling_socket_identity(endpoint: socket.socket) -> tuple[int, int]:
    info = os.fstat(endpoint.fileno())
    _scaling_require(stat.S_ISSOCK(info.st_mode))
    return info.st_dev, info.st_ino


def _scaling_close_socket(endpoint: socket.socket, identity) -> None:
    fd = endpoint.detach()
    if fd < 0:
        return
    try:
        info = os.fstat(fd)
        if (info.st_dev, info.st_ino) == identity:
            os.close(fd)
    except OSError:
        pass


def _scaling_decode_request(raw: bytes) -> dict[str, str]:
    _scaling_require(0 < len(raw) <= 4096 and raw.isascii())
    def pairs(items):
        result = {}
        for key, value in items:
            _scaling_require(key not in result)
            result[key] = value
        return result
    try:
        value = json.loads(raw, object_pairs_hook=pairs)
        _scaling_require(type(value) is dict and set(value) == {
            'operation', 'invocation_sha256', 'challenge'})
        _scaling_require(value['operation'] == 'fixed-scaling' and all(
            type(value[key]) is str and _DIGEST_RE.fullmatch(value[key]) is not None
            for key in ('invocation_sha256', 'challenge')))
        _scaling_require(_canonical_json(value) == raw)
        return value
    except (ValueError, TypeError, RecursionError) as error:
        raise BootstrapError("fixed scaling request is invalid") from error


class FixedScalingHandoff:
    """One fixed request serviced by the original release runner's parent.

    The channel does not authorize arbitrary commands. No deadline runs during
    the potentially long ordinary build/formal/soak corridor. A frame deadline
    starts at its first byte and never renews; collector scope comes from its
    admitted operation. The parent retains actual process observations.
    """

    def __init__(self, invocation_sha256: str, operation: FixedScalingOperation,
                 *, frame_timeout_seconds: int = 30) -> None:
        _scaling_require(type(invocation_sha256) is str
            and _DIGEST_RE.fullmatch(invocation_sha256) is not None)
        _scaling_require(type(frame_timeout_seconds) is int
            and 0 < frame_timeout_seconds <= 300)
        self.invocation_sha256 = invocation_sha256
        self.challenge = secrets.token_hex(32)
        self._operation = operation
        self._frame_timeout = frame_timeout_seconds
        self._parent, self._runner = socket.socketpair()
        self._parent.set_inheritable(False)
        self._runner.set_inheritable(False)
        self._parent.setblocking(False)
        self._parent_identity = _scaling_socket_identity(self._parent)
        self._runner_identity = _scaling_socket_identity(self._runner)
        self._phase = 'new'
        self._runner_process = None
        self._runner_reaped = False
        self._frame_end = None
        self._request = bytearray()
        self._error = None
        self._response = None
        self._command = CommandObservationOwner()
        self._observation = None
        self._artifact_handles = []
        self._operation_closed = False
        self._launch = None
        self._child_finished = False

    @property
    def runner_descriptor(self) -> int:
        """Endpoint for only the outer runner and its designated handoff."""
        _scaling_require(self._phase == 'new')
        _scaling_require(_scaling_socket_identity(self._runner) == self._runner_identity)
        return self._runner.fileno()

    @property
    def command_observation(self) -> TerminalCommandObservation | None:
        """Actual collector terminal status, even when publication fails."""
        return self._command.terminal

    @property
    def observation(self) -> ParentScalingObservation | None:
        """Verified original process/artifact join; never reconstructed from JSON."""
        return self._observation if self._phase == 'complete' else None

    @property
    def response(self) -> dict[str, Any] | None:
        """Return a copy of the one response, which alone is not release authority."""
        return dict(self._response) if self._response is not None else None

    def _started(self, process) -> None:
        _scaling_require(self._phase == 'new' and self._runner_process is None)
        self._runner_process = process
        _scaling_require(_scaling_socket_identity(self._runner) == self._runner_identity)
        self._phase = 'waiting'
        _scaling_close_socket(self._runner, self._runner_identity)

    def _runner_waited(self, process, returncode) -> None:
        _scaling_require(self._runner_process is process and type(returncode) is int)
        self._runner_reaped = True
        self._runner_returncode = returncode

    def _retain_artifact(self, launch, name, maximum):
        path = launch.evidence_root / name
        parent = os.open(launch.evidence_root,
            os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
        fd = None
        try:
            root = os.fstat(parent)
            _scaling_require(stat.S_ISDIR(root.st_mode) and root.st_uid == os.geteuid()
                and stat.S_IMODE(root.st_mode) == 0o700)
            fd = os.open(name, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK,
                dir_fd=parent)
            before = os.fstat(fd)
            _scaling_require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) in (0o400, 0o600)
                and 0 < before.st_size <= maximum)
            digest = hashlib.sha256()
            offset = 0
            while offset < before.st_size:
                part = os.pread(fd, min(65536, before.st_size-offset), offset)
                _scaling_require(bool(part)); digest.update(part); offset += len(part)
            identity = lambda x: (x.st_dev, x.st_ino, x.st_mode, x.st_uid, x.st_nlink,
                x.st_size, x.st_mtime_ns, x.st_ctime_ns)
            _scaling_require(identity(before) == identity(os.fstat(fd))
                == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)))
            self._artifact_handles.append((parent, fd, name, identity(before),
                (root.st_dev, root.st_ino, root.st_mode, root.st_uid), launch.evidence_root))
            return ScalingArtifactObservation(name, digest.hexdigest(), offset,
                stat.S_IMODE(before.st_mode))
        except BaseException:
            if fd is not None: os.close(fd)
            os.close(parent)
            raise

    def revalidate_observation(self) -> ParentScalingObservation:
        """Rehash original retained publication handles before parent receipt join."""
        _scaling_require(self._observation is not None)
        observed = []
        for parent, fd, name, pin, root_pin, root_path in self._artifact_handles:
            root = os.fstat(parent)
            pathname = root_path.lstat()
            _scaling_require((root.st_dev, root.st_ino, root.st_mode, root.st_uid)
                == root_pin == (pathname.st_dev, pathname.st_ino, pathname.st_mode, pathname.st_uid))
            identity = lambda x: (x.st_dev, x.st_ino, x.st_mode, x.st_uid, x.st_nlink,
                x.st_size, x.st_mtime_ns, x.st_ctime_ns)
            _scaling_require(identity(os.fstat(fd)) == pin
                == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)))
            digest = hashlib.sha256(); offset = 0
            while offset < pin[5]:
                chunk = os.pread(fd, min(65536, pin[5]-offset), offset)
                _scaling_require(bool(chunk)); digest.update(chunk); offset += len(chunk)
            _scaling_require(identity(os.fstat(fd)) == pin)
            observed.append(ScalingArtifactObservation(name, digest.hexdigest(), offset,
                stat.S_IMODE(pin[2])))
        _scaling_require(tuple(observed) == self._observation.artifacts)
        return self._observation

    def _execute(self) -> None:
        self._phase = 'consumed'
        launch = self._operation.prepare()
        self._launch = launch
        try:
            _scaling_launch_check(launch)
            _scaling_require(self._operation.validate(launch) is None)
            result = _run_bounded(launch.python, launch.argv[1:], cwd=launch.cwd,
                environment=dict(launch.environment), timeout_seconds=launch.timeout_seconds,
                maximum_output_bytes=launch.maximum_output_bytes,
                pass_fds=(launch.launch_input_fd, launch.seed_fd), observation=self._command,
                deadline_ns=launch.deadline_ns)
        finally:
            # None means Popen never returned a child. A missing observation
            # after an owned spawn is unknown; retain the operation in that case.
            if self._command._process is None or (self._command._reaped
                    and self._command.terminal is not None):
                _scaling_require(self._operation.child_finished(launch,self._command.terminal) is None)
                self._child_finished = True
        _scaling_require(self._operation.validate(launch) is None)
        terminal = self._command.terminal
        _scaling_require(terminal is not None and result.returncode == terminal.returncode)
        if result.returncode != 0:
            raise BootstrapError("fixed collector returned a nonzero status")
        artifacts = (self._retain_artifact(launch, 'manifest.json', launch.manifest_max_bytes),
            self._retain_artifact(launch, 'report.json', launch.report_max_bytes))
        observation = ParentScalingObservation(self.invocation_sha256, terminal,
            launch.launch_input_sha256, artifacts, launch.original_started_ns)
        _scaling_require(self._operation.verify_publication(launch, observation) is None)
        _scaling_require(self._operation.validate(launch) is None)
        self._observation = observation
        self.revalidate_observation()

    def _close_operation(self):
        _scaling_require(self._launch is None or self._child_finished)
        _scaling_require(self._operation.close() is None)
        self._operation_closed = True

    def _reply(self, success) -> None:
        terminal = self._command.terminal
        value = {'operation': 'fixed-scaling', 'invocation_sha256': self.invocation_sha256,
            'challenge': self.challenge, 'gate_status': 0 if success else 2,
            'process_returncode': terminal.returncode if terminal is not None else None,
            'manifest_sha256': self._observation.artifacts[0].sha256 if success else None,
            'report_sha256': self._observation.artifacts[1].sha256 if success else None}
        self._response = value
        raw = _canonical_json(value)
        self._parent.settimeout(self._frame_timeout)
        self._parent.sendall(len(raw).to_bytes(4, 'big') + raw)
        self._parent.shutdown(socket.SHUT_WR)

    def _finish_request(self):
        _scaling_require(len(self._request) >= 4)
        size = int.from_bytes(self._request[:4], 'big')
        _scaling_require(0 < size <= 4096 and len(self._request) == size + 4)
        value = _scaling_decode_request(bytes(self._request[4:]))
        _scaling_require(value['invocation_sha256'] == self.invocation_sha256
            and value['challenge'] == self.challenge)
        self._execute()

    def _service(self):
        if self._phase != 'waiting': return
        try:
            _scaling_require(_scaling_socket_identity(self._parent) == self._parent_identity)
            if self._frame_end is not None and time.monotonic() > self._frame_end:
                raise BootstrapError("fixed scaling request frame expired")
            readable, _, _ = select.select([self._parent], [], [], 0.05)
            if not readable: return
            raw = self._parent.recv(4101 - len(self._request))
            if self._frame_end is not None and time.monotonic() > self._frame_end:
                raise BootstrapError("fixed scaling request frame expired")
            if raw:
                if self._frame_end is None:
                    self._frame_end = time.monotonic() + self._frame_timeout
                self._request.extend(raw)
                _scaling_require(len(self._request) <= 4100)
                if len(self._request) >= 4:
                    expected = int.from_bytes(self._request[:4], 'big')
                    _scaling_require(0 < expected <= 4096 and len(self._request) <= expected+4)
                return
            # EOF is required before dispatch, so trailing or duplicate frames
            # cannot start a second child after a first request was accepted.
            self._finish_request()
            self._close_operation()
            self._reply(True)
            self._phase = 'complete'
        except BaseException as error:
            self._error = error
            self._phase = 'failed'
            if not self._operation_closed:
                try:
                    self._close_operation()
                except BaseException: pass
            try: self._reply(False)
            except BaseException: pass
        finally:
            if self._phase in ('complete', 'failed'):
                try:
                    if not self._operation_closed:
                        self._close_operation()
                except BaseException as error:
                    self._error = error
                    self._phase = 'failed'
                _scaling_close_socket(self._parent, self._parent_identity)

    def wait_runner(self, process) -> int:
        """Service one fixed channel and always reap the original runner naturally."""
        _scaling_require(self._runner_process is process and self._phase == 'waiting')
        error = None
        try:
            while process.poll() is None:
                self._service()
                if self._phase != 'waiting': time.sleep(0.05)
        except BaseException as caught:
            error = caught
            self._error = caught
            self._phase = 'failed'
            try: self._reply(False)
            except BaseException: pass
            _scaling_close_socket(self._parent, self._parent_identity)
        finally:
            returncode, wait_error = _wait_naturally(process)
            self._runner_waited(process, returncode)
            if error is None: error = wait_error
        if error is not None: raise error
        if self._error is not None: raise self._error
        if returncode == 0:
            _scaling_require(self._phase == 'complete' and self._observation is not None)
        return returncode

    def close(self) -> None:
        """Release retained channels/artifacts after the originating runner wait."""
        # The process owner calls this only after wait; a caller cannot turn an
        # active collector or runner into completed cleanup by closing handles.
        _scaling_require(self._runner_process is None or self._runner_reaped)
        for endpoint, pin in ((self._parent, self._parent_identity),
                              (self._runner, self._runner_identity)):
            _scaling_close_socket(endpoint, pin)
        for parent, fd, name, pin, root_pin, root_path in self._artifact_handles:
            for number, expected in ((fd,pin[:2]),(parent,root_pin[:2])):
                try:
                    info = os.fstat(number)
                    if (info.st_dev,info.st_ino) == expected: os.close(number)
                except OSError: pass
        self._artifact_handles.clear()
        if not self._operation_closed:
            self._close_operation()


def _wait_naturally(process):
    """Retain an original child through interruptions without any process signal."""
    first_error = None
    while True:
        try:
            return process.wait(), first_error
        except BaseException as error:
            if first_error is None: first_error = error
            try: time.sleep(0.05)
            except BaseException: pass


class BootstrapPreflightArchive:
    """Original protected writer for the fixed durable preflight subtree.

    The process owner controls when this sink may be released. Stream contents
    are discarded after publication; only bounded streaming snapshots remain.
    """

    def __init__(self, evidence: Path, evidence_fd: int):
        self.path = evidence / 'scaling-preflight'
        self._evidence, self._evidence_fd = evidence, evidence_fd
        self._evidence_pin = self._identity(os.fstat(evidence_fd))
        self._descriptor = self._descriptor_pin = None
        self._phase, self._caps, self._files = 'new', {}, {}

    @staticmethod
    def _identity(info):
        return (info.st_dev, info.st_ino, stat.S_IMODE(info.st_mode), info.st_uid)

    def _owned_descriptor(self):
        return _capture_descriptor_pin(self._descriptor)

    def _guard(self):
        _scaling_require(self._phase in ('writing', 'sealed'))
        parent = os.fstat(self._evidence_fd)
        _scaling_require(self._identity(parent) == self._evidence_pin
            == self._identity(self._evidence.lstat())
            and stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.getuid()
            and stat.S_IMODE(parent.st_mode) == _DIRECTORY_MODE)
        opened = os.fstat(self._descriptor)
        named = os.stat('scaling-preflight', dir_fd=self._evidence_fd, follow_symlinks=False)
        _scaling_require(self._owned_descriptor() == self._descriptor_pin
            and self._identity(opened) == self._identity(named)
            and stat.S_ISDIR(named.st_mode) and named.st_uid == os.getuid()
            and stat.S_IMODE(named.st_mode) == _DIRECTORY_MODE)
        names = set()
        with os.scandir(self._descriptor) as entries:
            for index, entry in enumerate(entries):
                _scaling_require(index < len(self._caps))
                names.add(entry.name)
        _scaling_require(names == set(self._files))

    def create(self, caps):
        """Allocate only after the original preflight deadline has started."""
        _scaling_require(self._phase == 'new' and type(caps) is dict
            and {'inventory.json', 'index.json'} <= caps.keys()
            and all(type(name) is str and re.fullmatch(
                r'(?:inventory\.json|index\.json|unit-[0-9]{3}\.(?:command\.json|result\.json|stdout|stderr))', name)
                and type(cap) is int and cap > 0 for name, cap in caps.items()))
        self._caps = dict(caps)
        try:
            parent = os.fstat(self._evidence_fd)
            _scaling_require(self._identity(parent) == self._evidence_pin
                == self._identity(self._evidence.lstat())
                and stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.getuid()
                and stat.S_IMODE(parent.st_mode) == _DIRECTORY_MODE)
            os.mkdir('scaling-preflight', _DIRECTORY_MODE, dir_fd=self._evidence_fd)
            created = os.stat('scaling-preflight', dir_fd=self._evidence_fd, follow_symlinks=False)
            self._descriptor, self._descriptor_pin = _open_capture_descriptor('scaling-preflight',
                os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW,
                (created.st_dev, created.st_ino, stat.S_IFDIR, created.st_uid),
                'durable preflight directory',
                dir_fd=self._evidence_fd)
            self._phase = 'writing'
            self._guard()
            os.fsync(self._evidence_fd)
        except BaseException:
            self._phase = 'failed'
            raise

    def write(self, name, data, maximum_bytes):
        """Publish one exact original buffer and retain only its digest metadata."""
        self._guard()
        _scaling_require(self._phase == 'writing' and name in self._caps
            and name not in self._files and type(data) is bytes
            and type(maximum_bytes) is int and maximum_bytes == self._caps[name]
            and len(data) <= maximum_bytes)
        if name == 'index.json':
            _scaling_require(set(self._files) == self._caps.keys() - {'index.json'})
        try:
            original = _write_artifact(self.path, self._descriptor, name, data, _DATA_MODE)
            snapshot = _capture_large_file_at(self._descriptor, name, self.path / name,
                'durable preflight member', maximum_bytes=maximum_bytes)
            _scaling_require(snapshot.sha256 == original.sha256 and snapshot.size == len(data)
                and snapshot.mode == _DATA_MODE and snapshot.owner == os.getuid()
                and snapshot.nlink == 1
                and (snapshot.device, snapshot.inode) == (original.device, original.inode))
            self._files[name] = snapshot
            if name == 'index.json': self._phase = 'sealed'
            self._guard()
            return snapshot
        except BaseException:
            self._phase = 'failed'
            raise

    def verify(self):
        """Recheck the original bounded file identities without retaining log bytes."""
        self._guard()
        for name, expected in self._files.items():
            actual = _capture_large_file_at(self._descriptor, name, self.path / name,
                'durable preflight member', maximum_bytes=self._caps[name])
            _scaling_require(actual == expected)
        self._guard()

    @property
    def rows(self):
        """Original metadata for the shared data codec's exact portable census."""
        self._guard()
        return tuple(dict(relative_path=name, size_bytes=row.size, sha256=row.sha256,
                          mode=f'{row.mode:04o}', max_bytes=self._caps[name])
                     for name, row in sorted(self._files.items()))

    def close(self):
        """Release only this retained descriptor; never delete populated evidence."""
        if self._phase == 'closed': return
        if self._descriptor is not None:
            _close_capture_descriptor(self._descriptor, self._descriptor_pin)
        self._phase = 'closed'


class ReleaseInvocationRoot:
    """Retained original invocation directory; cleanup remains with its existing owner."""

    def __init__(self):
        raise BootstrapError("use allocate_release_invocation_root")

    @property
    def path(self) -> Path:
        return self._snapshot.path

    @property
    def base(self) -> Path:
        return self._base

    @property
    def descriptor(self) -> int:
        self.validate()
        return self._root_fd

    @property
    def snapshot(self) -> DirectorySnapshot:
        """Initial snapshot; content timestamps may change during normal construction."""
        return self._snapshot

    def validate(self) -> None:
        _scaling_require(not self._closed)
        identity=lambda value:(value.st_dev,value.st_ino,stat.S_IMODE(value.st_mode),value.st_uid)
        base=os.fstat(self._base_fd)
        _scaling_require(identity(base)==self._base_pin==identity(self._base.lstat())
            and stat.S_ISDIR(base.st_mode))
        root=os.fstat(self._root_fd)
        current=os.stat(self.path.name,dir_fd=self._base_fd,follow_symlinks=False)
        expected=(self._snapshot.device,self._snapshot.inode,self._snapshot.mode,self._snapshot.owner)
        _scaling_require(identity(root)==expected==identity(current)
            and stat.S_ISDIR(root.st_mode) and stat.S_ISDIR(current.st_mode))

    def close(self) -> None:
        """Close only owned descriptors; never delete populated or empty outputs."""
        if self._closed: return
        for fd,pin in ((self._root_fd,(self._snapshot.device,self._snapshot.inode)),
                      (self._base_fd,self._base_pin[:2])):
            try:
                info=os.fstat(fd)
                if (info.st_dev,info.st_ino)==pin: os.close(fd)
            except OSError: pass
        self._closed=True


def allocate_release_invocation_root(candidate: Path, bootstrap_evidence: Path,
        cargo_cache: Path, *, base: Path | None = None) -> ReleaseInvocationRoot:
    """Allocate the shell's canonical external invocation root in its original parent.

    Keep root-owned sticky ancestry, exact 0700 ownership, shell-safe spelling and
    disjointness from source, bootstrap and Cargo-cache roots. Only an exact empty
    directory created here can be removed on allocation failure. The existing
    release cleanup helper owns all later populated-output cleanup.
    """
    if base is None:
        preferred=Path('/private/tmp')
        base=preferred if preferred.is_dir() and not preferred.is_symlink() else Path('/tmp')
        base=base.resolve(strict=True)
    base=_absolute_resolved_existing(base,'release invocation base')
    _scaling_require(_SAFE_PATH_RE.fullmatch(str(base)) is not None and os.pathsep not in str(base))
    protected=[]
    for path in (candidate,bootstrap_evidence,cargo_cache):
        _scaling_require(type(path) is type(Path('/')) and path.is_absolute()
            and path==Path(os.path.abspath(path)))
        protected.append(path.resolve(strict=False))
    base_fd=os.open(base,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC|os.O_NOFOLLOW)
    root_fd=None;name=None;pin=None
    try:
        info=os.fstat(base_fd)
        _scaling_require(stat.S_ISDIR(info.st_mode) and info.st_uid==0
            and info.st_mode & stat.S_ISVTX != 0
            and (info.st_dev,info.st_ino)==(base.lstat().st_dev,base.lstat().st_ino))
        name='iroha-sumeragi-v2-release.'+secrets.token_hex(16)
        path=base/name
        _scaling_require(_SAFE_PATH_RE.fullmatch(str(path)) is not None
            and os.pathsep not in str(path)
            and all(not _inside(path,other) and not _inside(other,path) for other in protected))
        os.mkdir(name,_DIRECTORY_MODE,dir_fd=base_fd)
        root_fd=os.open(name,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC|os.O_NOFOLLOW,dir_fd=base_fd)
        os.fchmod(root_fd,_DIRECTORY_MODE)
        opened=os.fstat(root_fd);pin=(opened.st_dev,opened.st_ino)
        snapshot=_private_directory_snapshot(path,'release invocation root')
        _scaling_require(pin==(snapshot.device,snapshot.inode))
        os.fsync(base_fd)
        owner=object.__new__(ReleaseInvocationRoot)
        owner._base=base;owner._base_fd=base_fd;owner._root_fd=root_fd
        owner._base_pin=(info.st_dev,info.st_ino,stat.S_IMODE(info.st_mode),info.st_uid)
        owner._snapshot=snapshot;owner._closed=False
        owner.validate()
        return owner
    except BaseException:
        if root_fd is not None: os.close(root_fd)
        if name is not None and pin is not None:
            try:
                current=os.stat(name,dir_fd=base_fd,follow_symlinks=False)
                if (current.st_dev,current.st_ino)==pin: os.rmdir(name,dir_fd=base_fd)
            except OSError: pass
        os.close(base_fd)
        raise


@dataclass(frozen=True)
class ScalingSourceSelection:
    """Original bootstrap observations of the completed inner build inputs."""

    source: DirectorySnapshot
    identity: FileSnapshot
    source_paths: FileSnapshot
    binary_manifest: FileSnapshot
    rustc_version: FileSnapshot
    python_runtime_binding: FileSnapshot


def _prepare_scaling_source_selection(
    invocation: ReleaseInvocationRoot,
    candidate_identity: FileSnapshot,
    python: FileSnapshot,
    manifest_helper: FileSnapshot,
    rustc: FileSnapshot,
    evidence: Path,
    evidence_fd: int,
    framework_binding: bytes,
    environment: dict[str, str],
    timeout_seconds: int,
) -> ScalingSourceSelection:
    """Select build outputs from the retained root, never from a gate request.

    The designated handoff calls this only after the inner build has naturally
    completed. Sealing changes filesystem permission bits and consequently the
    workspace manifest; the signed commit/tree/lock identity must stay exact.
    The protected manifest helper independently verifies the sealed worktree's
    raw bytes against its index before any parent scaling module is imported.
    """
    _scaling_require(type(invocation) is ReleaseInvocationRoot)
    invocation.validate()
    _scaling_require(type(timeout_seconds) is int and timeout_seconds > 0
                     and type(framework_binding) is bytes
                     and 0 < len(framework_binding) <= _MAX_EVIDENCE_BYTES)
    for label, snapshot, maximum, executable in (
        ('scaling candidate identity', candidate_identity, _MAX_IDENTITY_BYTES, False),
        ('scaling selected Python', python, _MAX_TOOL_BYTES, True),
        ('scaling protected manifest helper', manifest_helper, _MAX_HELPER_BYTES, False),
        ('scaling selected rustc', rustc, _MAX_TOOL_BYTES, True),
    ):
        _require_unchanged(snapshot, label, maximum_bytes=maximum, executable=executable)
    original = _load_identity(candidate_identity.data)
    source_path = invocation.path / 'source'
    source = _sealed_directory_snapshot(source_path, 'scaling sealed source')
    identity = _read_file(invocation.path / 'sealed-identity.json',
                          'scaling sealed identity', maximum_bytes=_MAX_IDENTITY_BYTES)
    observed_bytes, observed = _compute_identity(python.path, manifest_helper.path,
                                                source_path, environment, timeout_seconds)
    _scaling_require(identity.data == observed_bytes)
    for field in ('head_commit', 'head_tree', 'index_tree', 'cargo_lock_sha256'):
        _scaling_require(observed[field] == original[field])
    _scaling_require(observed['index_tree'] == observed['head_tree'])
    source_digest = observed['workspace_source_manifest_sha256']
    source_paths_path = evidence / 'scaling-source-paths.txt'
    _scaling_require(not os.path.lexists(source_paths_path))
    paths_result = _run_bounded(python.path, (
        '-I', '-B', '-S', str(manifest_helper.path), '--root', str(source_path),
        '--write-path-list', str(source_paths_path)), cwd=source_path,
        environment=dict(environment), timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES)
    _scaling_require(type(paths_result) is CommandResult
                     and paths_result.returncode == 0 and paths_result.stderr == b''
                     and paths_result.stdout == (source_digest + '\n').encode('ascii'))
    source_paths = _read_file(source_paths_path, 'scaling exact source path list',
                             maximum_bytes=_MAX_HELPER_BYTES)
    _scaling_require(source_paths.size > 0)
    binary_manifest = _read_file(invocation.path / 'output' / 'sumeragi-v2-release'
        / source_digest / 'programs' / '.sumeragi-v2-prebuilt-binaries.tsv',
        'scaling native binary manifest', maximum_bytes=_MAX_HELPER_BYTES)
    version = _run_bounded(rustc.path, ('--version', '--verbose'), cwd=source_path,
        environment=dict(environment), timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES)
    _scaling_require(type(version) is CommandResult and version.returncode == 0
                     and version.stderr == b'' and version.stdout.endswith(b'\n')
                     and version.stdout.isascii() and b'\0' not in version.stdout
                     and b'\r' not in version.stdout)
    rustc_version = _write_artifact(evidence, evidence_fd, 'scaling-rustc-version.txt',
                                   version.stdout, _DATA_MODE)
    runtime_binding = _write_artifact(evidence, evidence_fd, 'scaling-python-runtime.json',
                                      framework_binding, _DATA_MODE)
    repeated, _ = _compute_identity(python.path, manifest_helper.path, source_path,
                                    environment, timeout_seconds)
    _scaling_require(repeated == observed_bytes)
    invocation.validate()
    _require_sealed_directory_unchanged(source, 'scaling sealed source')
    for label, snapshot, maximum, executable in (
        ('scaling candidate identity', candidate_identity, _MAX_IDENTITY_BYTES, False),
        ('scaling selected Python', python, _MAX_TOOL_BYTES, True),
        ('scaling protected manifest helper', manifest_helper, _MAX_HELPER_BYTES, False),
        ('scaling selected rustc', rustc, _MAX_TOOL_BYTES, True),
        ('scaling sealed identity', identity, _MAX_IDENTITY_BYTES, False),
        ('scaling exact source path list', source_paths, _MAX_HELPER_BYTES, False),
        ('scaling native binary manifest', binary_manifest, _MAX_HELPER_BYTES, False),
        ('scaling rustc version', rustc_version, _MAX_HELPER_OUTPUT_BYTES, False),
        ('scaling Python runtime binding', runtime_binding, _MAX_EVIDENCE_BYTES, False),
    ):
        _require_unchanged(snapshot, label, maximum_bytes=maximum, executable=executable)
    return ScalingSourceSelection(source, identity, source_paths, binary_manifest,
                                   rustc_version, runtime_binding)


# Inserted into the already protected bootstrap, before _run_bounded.
_SCALING_PARENT_EXTENSIONS = (
    'scripts/nexus/scaling_preflight_archive.py',
    'scripts/nexus/scaling_release_preflight.py',
    'scripts/nexus/scaling_archive_data.py',
    'scripts/nexus/scaling_release_provisioning.py',
    'scripts/nexus/scaling_release_record.py',
    'scripts/sumeragi_v2_release_scaling_operation.py',
)


class ScalingParentModules:
    """Captured authenticated source modules; no ambient source or pyc import.

    Source selection has already reproduced the sealed index/workspace. The
    bootstrap repeats that source verification around initial module loading.
    Retained module identities survive safe close for the final pure projection;
    closing this loader does not claim old pathnames remain live afterward.
    """
    def __init__(self, selected: ScalingSourceSelection):
        _scaling_require(type(selected) is ScalingSourceSelection)
        self._selected = selected
        self._snapshots, self._modules = {}, {}
        self._closed = False
        self._installed = False
        self._source = selected.source
        self._previous_cache_prefix = sys.pycache_prefix
        self._cache_prefix = str(selected.source_paths.path.parent/'scaling-unused-parent-cache')
        self._cache_active = False
        try:
            # Some authenticated modules use SourceFileLoader directly, outside
            # this finder. A nonexistent cache namespace prevents inherited pyc
            # reads there; -B alone only prevents writing new bytecode.
            _scaling_require(sys.dont_write_bytecode is True
                and not os.path.lexists(self._cache_prefix))
            sys.pycache_prefix = self._cache_prefix
            self._cache_active = True
            registry = self._capture('scripts/nexus/scaling_cli_bootstrap.py')
            parsed = ast.parse(registry.data)
            matches = [node.value for node in parsed.body if isinstance(node, ast.Assign)
                and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id == 'PYTHON_SOURCE_FILES']
            _scaling_require(len(matches) == 1)
            names = ast.literal_eval(matches[0])
            _scaling_require(type(names) is tuple and 1 <= len(names) <= 128
                and len(set(names)) == len(names))
            for name in (*names, *_SCALING_PARENT_EXTENSIONS): self._capture(name)
            self.verify()
            sys.meta_path.insert(0, self)
            self._installed = True
            source = self.load('compute_workspace_source_manifest')
            members = source.read_source_path_list(selected.source_paths.path)
            _scaling_require(all(str(row.path.relative_to(self._source.path)) in members
                                 for row in self._snapshots.values()))
            self.verify()
        except BaseException:
            self.release()
            raise

    def _capture(self, relative):
        _scaling_require(type(relative) is str
            and re.fullmatch(r'scripts/(?:nexus/)?[A-Za-z_][A-Za-z_0-9]*\.py', relative))
        name = Path(relative).stem
        _scaling_require(name not in sys.stdlib_module_names)
        path = self._source.path / relative
        if name in self._snapshots:
            _scaling_require(self._snapshots[name].path == path)
            return self._snapshots[name]
        _scaling_require(name not in sys.modules and len(self._snapshots) < 132)
        row = _read_file(path, 'sealed scaling parent module', maximum_bytes=_MAX_HELPER_BYTES)
        _scaling_require(row.mode in (0o400, 0o500) and row.nlink == 1
                         and row.owner == os.getuid())
        self._snapshots[name] = row
        return row

    def find_spec(self, fullname, path=None, target=None):
        if fullname not in self._snapshots: return None
        _scaling_require(not self._closed and target is None and path is None)
        return importlib.machinery.ModuleSpec(fullname, self,
            origin=str(self._snapshots[fullname].path))

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        _scaling_require(not self._closed and module.__name__ in self._snapshots
                         and module.__name__ not in self._modules)
        row = self._snapshots[module.__name__]
        _require_unchanged(row, 'captured scaling parent module', maximum_bytes=_MAX_HELPER_BYTES)
        module.__file__ = str(row.path)
        module.__package__ = ''
        self._modules[module.__name__] = module
        exec(compile(row.data, str(row.path), 'exec', dont_inherit=True), module.__dict__)
        self.verify()

    def load(self, name):
        _scaling_require(not self._closed and name in self._snapshots)
        value = importlib.import_module(name)
        _scaling_require(self._modules.get(name) is value)
        self.verify()
        return value

    def verify(self):
        _scaling_require(not self._closed and self._cache_active
            and sys.dont_write_bytecode is True
            and sys.pycache_prefix == self._cache_prefix
            and not os.path.lexists(self._cache_prefix))
        _require_sealed_directory_unchanged(self._source, 'scaling module source')
        _require_unchanged(self._selected.source_paths, 'scaling source path list',
                           maximum_bytes=_MAX_HELPER_BYTES)
        for row in self._snapshots.values():
            _require_unchanged(row, 'captured scaling module', maximum_bytes=_MAX_HELPER_BYTES)
        self.verify_retained()

    def verify_retained(self):
        for name, module in self._modules.items():
            _scaling_require(sys.modules.get(name) is module
                and module.__file__ == str(self._snapshots[name].path)
                and module.__loader__ is self
                and module.__spec__.loader is self
                and module.__spec__.origin == module.__file__)

    def close(self):
        if self._closed: return
        try:
            self.verify()
        finally:
            # Remove only this finder even when its source/module check failed.
            # Loaded objects remain retained until the enclosing owner releases.
            sys.meta_path[:] = [value for value in sys.meta_path if value is not self]
            self._installed, self._closed = False, True
            self._restore_cache_prefix()

    def _restore_cache_prefix(self):
        if self._cache_active and sys.pycache_prefix == self._cache_prefix:
            sys.pycache_prefix = self._previous_cache_prefix
        self._cache_active = False

    def release(self):
        """Release only this loader's identities; never foreign replacement modules."""
        if self._installed and self in sys.meta_path: sys.meta_path.remove(self)
        self._installed, self._closed = False, True
        self._restore_cache_prefix()
        for name, module in reversed(tuple(self._modules.items())):
            if sys.modules.get(name) is module: del sys.modules[name]


class BootstrapScalingOperation:
    """Lazy original-parent preparation, fixed execution and record publication."""
    def __init__(self, invocation: ReleaseInvocationRoot, candidate_identity: FileSnapshot,
                 python: FileSnapshot, manifest_helper: FileSnapshot, rustc: FileSnapshot,
                 plan: FileSnapshot, budget: FileSnapshot, handoff_helper: FileSnapshot,
                 evidence: Path, evidence_fd: int, framework_binding: bytes,
                 environment: dict[str, str], command_timeout_seconds: int,
                 installed_dependencies: Path, machine_id: str, storage_model: str,
                 observation_overhead_seconds: int, preflight_timeout_seconds: int):
        _scaling_require(type(invocation) is ReleaseInvocationRoot)
        invocation.validate()
        for row in (candidate_identity, python, manifest_helper, rustc, plan, budget, handoff_helper):
            _scaling_require(type(row) is FileSnapshot)
        _scaling_require(type(observation_overhead_seconds) is int
                         and 0 < observation_overhead_seconds <= 600)
        for value in (machine_id, storage_model):
            _scaling_require(type(value) is str and 0 < len(value) <= 512
                and all(32 <= ord(char) < 127 for char in value))
        self._invocation = invocation
        self._original = (candidate_identity, python, manifest_helper, rustc, plan, budget, handoff_helper)
        self._evidence, self._evidence_fd = evidence, evidence_fd
        self._framework = framework_binding
        self._environment = tuple(sorted(environment.items()))
        self._timeout = command_timeout_seconds
        self._dependencies = installed_dependencies
        self._labels = (machine_id, storage_model)
        self._overhead = observation_overhead_seconds
        _scaling_require(type(preflight_timeout_seconds) is int and 600 <= preflight_timeout_seconds <= 86400)
        self._preflight_timeout = preflight_timeout_seconds
        self._phase = 'new'
        self._loader = self._prepared = self._operation = self._record = None
        self._observation = self._verification = self._inputs = None
        self._record_api = self._encoder = None
        self._verifier_python_owner = self._verifier_python_json = None
        self._selection = None
        self._preflight = None
        self.invocation_sha256 = hashlib.sha256(_canonical_json(dict(
            operation='iroha.sumeragi_v2.fixed_scaling.parent.v1',
            root=str(invocation.path), inputs=[row.sha256 for row in self._original],
            dependencies=str(installed_dependencies), labels=list(self._labels),
            observation_overhead_seconds=observation_overhead_seconds,
            preflight_timeout_seconds=preflight_timeout_seconds))).hexdigest()
        self._invocation_pin = self.invocation_sha256
        self._check_original()

    def _check_original(self):
        self._invocation.validate()
        _scaling_require(self.invocation_sha256 == self._invocation_pin)
        for index, row in enumerate(self._original):
            _require_unchanged(row, 'original parent scaling input',
                maximum_bytes=_MAX_TOOL_BYTES if index in (1, 3) else _MAX_HELPER_BYTES,
                executable=index in (1, 3))

    def _check_selected_source(self):
        self._check_original()
        row = self._selection
        _scaling_require(type(row) is ScalingSourceSelection)
        raw, _ = _compute_identity(self._original[1].path, self._original[2].path,
            row.source.path, dict(self._environment), self._timeout)
        _scaling_require(raw == row.identity.data)
        _require_sealed_directory_unchanged(row.source, 'selected scaling source')

    def prepare(self):
        _scaling_require(self._phase == 'new')
        self._phase = 'preparing'
        try:
            self._check_original()
            candidate, python, helper, rustc, plan, budget, _ = self._original
            self._selection = selected = _prepare_scaling_source_selection(self._invocation,
                candidate, python, helper, rustc, self._evidence, self._evidence_fd,
                self._framework, dict(self._environment), self._timeout)
            self._check_selected_source()
            self._loader = ScalingParentModules(selected)
            preflight = self._loader.load('scaling_release_preflight')
            dependency_api = self._loader.load('scaling_cli_bootstrap')
            self._preflight = preflight.CompleteScalingPreflight(
                selected.source.path, self._dependencies,
                self._invocation.path/'runtime/scaling-preflight', python.path,
                dict(self._environment), self._preflight_timeout,
                preflight.PreflightApi(CommandObservationOwner, _run_bounded,
                    _read_file, _require_unchanged), dependency_api,
                archive=BootstrapPreflightArchive(self._evidence, self._evidence_fd),
                candidate_identity=_load_identity(selected.identity.data),
                invocation_sha256=self.invocation_sha256)
            self._preflight.run()
            self._preflight.verify()
            self._check_selected_source()
            provisioning = self._loader.load('scaling_release_provisioning')
            self._check_selected_source()
            identity = _load_identity(selected.identity.data)
            root = self._invocation.path
            choices = provisioning.ParentScalingSelection(selected.source.path,
                selected.source_paths.path, selected.source_paths.sha256,
                identity['workspace_source_manifest_sha256'], root/'target', root/'output',
                selected.binary_manifest.path.parent, selected.binary_manifest.sha256,
                selected.rustc_version.path, self._evidence,
                selected.python_runtime_binding.path, selected.python_runtime_binding.sha256,
                self._dependencies, plan.path, plan.sha256, budget.path, budget.sha256,
                root/'runtime/scaling-control', root/'output/scaling', root/'runtime/scaling-work',
                self._labels[0], self._labels[1], identity['head_commit'],
                self._environment, self._overhead)
            # Seed is created once, remains runtime-only and enters the existing
            # retained pipe owner. No socket request selects or carries it.
            self._prepared = provisioning.PreparedScalingInputs.prepare(choices, secrets.token_hex(32))
            self._inputs = self._prepared.verification_inputs()
            self._stage_verifier_python()
            concrete = self._loader.load('sumeragi_v2_release_scaling_operation')
            self._record_api = self._loader.load('scaling_release_record')
            self._encoder = self._record_api.encode_parent_execution
            api = concrete.BootstrapScalingApi(FixedScalingLaunch, CommandObservationOwner,
                TerminalCommandObservation, ParentScalingObservation, ScalingArtifactObservation,
                CommandResult, _run_bounded, _validated_pass_fds)
            self._operation = concrete.FixedScalingOperation(self._prepared, self.invocation_sha256, api)
            self._launch = self._operation.prepare()
            self._phase = 'borrowed'
            self.validate(self._launch)
            return self._launch
        except BaseException:
            # Preflight owns its original children independently and close() refuses
            # unknown terminal state. Collector preparation does not spawn: a
            # returned concrete launch remains our original no-spawn borrow.
            if self._phase == 'borrowed':
                self._operation.child_finished(self._launch, None)
            self._phase = 'failed'
            self.close()
            raise

    def _stage_verifier_python(self):
        """Retain original nonsecret package content for fresh receipt processes."""
        self._prepared.validate()
        python_contract = self._loader.load('scaling_cli_bootstrap')
        # The exact prepared owner already guards this originating dependency
        # object, its paths and package bytes. Do not rescan an installed path.
        original = self._prepared._dependencies
        source = original.paths.source_root
        expected = python_contract.dependency_package_census(source)
        os.mkdir('scaling-verifier-python', _DIRECTORY_MODE, dir_fd=self._evidence_fd)
        root = self._evidence/'scaling-verifier-python'
        python_contract.stage_dependency_source(source, root/'source')
        self._verifier_python_owner = python_contract.PythonDependencies.provision(
            root/'source', root/'bundle', root/'inventory.json')
        self._verifier_python_owner.verify()
        _scaling_require(python_contract.dependency_package_census(root/'source') == expected
            and python_contract.dependency_package_census(root/'bundle') == expected)
        self._verifier_python_json = _canonical_json(dict(
            inventory_sha256=self._verifier_python_owner.paths.inventory_sha256,
            files=list(expected)))
        self._prepared.validate()
        self._verifier_python_owner.verify()

    def validate(self, launch):
        _scaling_require(self._phase in ('borrowed', 'reaped', 'verified')
                         and launch is self._launch)
        self._check_original()
        self._loader.verify()
        self._preflight.verify()
        self._operation.validate(launch)
        if self._record is not None:
            _require_unchanged(self._record, 'original scaling execution record',
                              maximum_bytes=self._record_api.MAX_RECORD_BYTES)

    def child_finished(self, launch, observation):
        _scaling_require(self._phase == 'borrowed' and launch is self._launch)
        self._operation.child_finished(launch, observation)
        self._phase = 'reaped'

    def verify_publication(self, launch, observation):
        _scaling_require(self._phase == 'reaped' and launch is self._launch)
        self.validate(launch)
        self._operation.verify_publication(launch, observation)
        self._observation = observation
        self._verification = self._operation.verification
        raw = self._encode_record(observation)
        self._record = _write_artifact(self._evidence, self._evidence_fd,
                                      'scaling-execution.json', raw, _DATA_MODE)
        self._operation.validate(launch)
        _scaling_require(time.monotonic_ns() < launch.deadline_ns)
        self._phase = 'verified'

    def _encode_record(self, observation):
        _scaling_require(observation is self._observation
            and self._operation.verification is self._verification
            and self._record_api.encode_parent_execution is self._encoder)
        self._loader.verify_retained()
        self._preflight.verify_retained()
        self._verifier_python_owner.verify()
        return self._encoder(_load_identity(self._selection.identity.data), self._inputs,
            observation, self._verification, observation_overhead_seconds=self._overhead,
            verifier_python=json.loads(self._verifier_python_json),
            preflight=self._preflight.archive_binding)

    def revalidate_final(self, observation):
        """Reproject the same retained objects after natural runner completion."""
        _scaling_require(self._phase == 'closed' and self._record is not None)
        _scaling_require(self._encode_record(observation) == self._record.data)
        _require_unchanged(self._record, 'original scaling execution record',
                          maximum_bytes=self._record_api.MAX_RECORD_BYTES)
        return self._record

    @property
    def record_api(self):
        _scaling_require(self._phase == 'closed' and self._record is not None)
        self._loader.verify_retained()
        return self._record_api

    def close(self):
        if self._phase == 'closed': return
        if self._operation is not None: self._operation.close()
        elif self._prepared is not None: self._prepared.close()
        if self._preflight is not None: self._preflight.close()
        if self._loader is not None: self._loader.close()
        self._phase = 'closed'

    def release(self):
        """Final module release only after the original child owners allow close."""
        # Establish natural terminal ownership before exceptional cleanup can
        # release the retained source or archive descriptors.
        if self._operation is not None: self._operation.close()
        elif self._prepared is not None: self._prepared.close()
        if self._preflight is not None: self._preflight.require_terminal()
        try:
            self.close()
        finally:
            self._release_retained()

    def _release_retained(self):
        try:
            if self._verifier_python_owner is not None: self._verifier_python_owner.close()
        finally:
            try:
                if self._preflight is not None: self._preflight.release()
            finally:
                if self._loader is not None: self._loader.release()


def _run_bounded(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    maximum_output_bytes: int,
    pass_fds: tuple[int, ...] = (),
    observation: CommandObservationOwner | None = None,
    deadline_ns: int | None = None,
) -> CommandResult:
    descriptors = _validated_pass_fds(pass_fds)
    if (type(timeout_seconds) is not int or timeout_seconds <= 0
            or type(maximum_output_bytes) is not int or maximum_output_bytes <= 0):
        raise BootstrapError("protected command bounds are invalid")
    if observation is not None and type(observation) is not CommandObservationOwner:
        raise BootstrapError("protected command observation owner is invalid")
    argv = [str(executable), *arguments]
    started = time.monotonic()
    deadline = started + timeout_seconds
    started_ns = int(started * 1_000_000_000)
    has_original_deadline = deadline_ns is not None
    if deadline_ns is None:
        deadline_ns = int(deadline * 1_000_000_000)
    elif (type(deadline_ns) is not int or not started_ns < deadline_ns
            or deadline_ns > started_ns + timeout_seconds * 1_000_000_000):
        raise BootstrapError("protected command original deadline is invalid or expired")
    deadline = deadline_ns / 1_000_000_000
    if observation is not None:
        observation._begin(argv, cwd, environment, descriptors, started_ns, deadline_ns)
    if _validated_pass_fds(pass_fds) != descriptors:
        raise BootstrapError("protected command descriptors changed before launch")
    if has_original_deadline and int(time.monotonic() * 1_000_000_000) >= deadline_ns:
        raise BootstrapError("protected command original deadline expired before launch")
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            pass_fds=pass_fds,
        )
    except OSError as error:
        raise BootstrapError(f"could not execute protected command {executable}") from error
    if observation is not None:
        observation._spawned(process)
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    digests = {name: hashlib.sha256() for name in buffers}
    counts = {name: 0 for name in buffers}
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
            digests[label].update(chunk)
            counts[label] += len(chunk)
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
    drain_threads = []
    started_threads: list[threading.Thread] = []
    supervision_error: BaseException | None = None
    try:
        assert process.stdout is not None and process.stderr is not None
        drain_threads = [
            threading.Thread(
                target=drain,
                args=(label, stream),
                name=f"bootstrap-{label}-drain",
            )
            for label, stream in drain_specs
        ]
        for thread in drain_threads:
            thread.start()
            started_threads.append(thread)
        while process.poll() is None:
            if _validated_pass_fds(pass_fds) != descriptors:
                raise BootstrapError("protected command descriptors changed during execution")
            if time.monotonic() > deadline:
                runtime_limit_exceeded = True
            time.sleep(0.05)
    except BaseException as error:
        supervision_error = error
    finally:
        try:
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
        except BaseException as error:
            drain_errors.append(error)
        try:
            # This is deliberately unbounded: neither a latched policy
            # violation nor an observer exception authorizes child control.
            returncode, wait_error = _wait_naturally(process)
            if observation is not None:
                observation._waited(process, returncode)
            if supervision_error is None:
                supervision_error = wait_error
        finally:
            for thread in started_threads:
                while True:
                    try:
                        thread.join()
                        break
                    except BaseException as error:
                        if supervision_error is None: supervision_error = error
            for stream in (process.stdout, process.stderr):
                try: stream.close()
                except BaseException as error: drain_errors.append(error)
    completed_ns = None
    descriptor_error = None
    try:
        completed = time.monotonic()
        completed_ns = int(completed * 1_000_000_000)
        if completed > deadline:
            runtime_limit_exceeded = True
        if _validated_pass_fds(pass_fds) != descriptors:
            raise BootstrapError("protected command descriptors changed before terminal observation")
    except BaseException as error:
        descriptor_error = error
        if supervision_error is None: supervision_error = error
    violations = []
    if supervision_error is not None: violations.append("supervision")
    if descriptor_error is not None: violations.append("terminal_identity_or_clock")
    if len(started_threads) != len(drain_threads): violations.append("drain_start")
    if drain_errors: violations.append("drain")
    if runtime_limit_exceeded: violations.append("runtime")
    if output_limit_exceeded: violations.append("output")
    if observation is not None:
        observation._finish(process, completed_ns, returncode, digests, counts, violations)
    if supervision_error is not None:
        raise supervision_error
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
    scaling_handoff: FixedScalingHandoff | None = None,
) -> CommandResult:
    """Run the release runner with private regular-file diagnostic sinks.

    The runner owns Cargo, rustc, validator, formal, chaos, and soak processes.
    Their in-scope operations have their own protocol and harness deadlines;
    direct regular-file descriptors avoid relay backpressure. The bootstrap
    leaves cancellation to the runner's cooperative gate boundaries and waits
    for the in-flight runner to finish naturally.
    """

    if scaling_handoff is not None and type(scaling_handoff) is not FixedScalingHandoff:
        raise BootstrapError("release scaling handoff owner is invalid")
    inherited = (scaling_handoff.runner_descriptor,) if scaling_handoff is not None else ()
    inherited_pins = _validated_pass_fds(inherited)
    argv = [str(executable), *arguments]
    if _validated_pass_fds(inherited) != inherited_pins:
        raise BootstrapError("release runner descriptor changed before launch")
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout_descriptor,
            stderr=stderr_descriptor,
            close_fds=True,
            pass_fds=inherited,
        )
    except OSError as error:
        raise RunnerLaunchError(
            f"could not execute protected command {executable}"
        ) from error
    if scaling_handoff is None:
        returncode, wait_error = _wait_naturally(process)
        if wait_error is not None: raise wait_error
    else:
        try:
            scaling_handoff._started(process)
            returncode = scaling_handoff.wait_runner(process)
        except BaseException:
            returncode, _ = _wait_naturally(process)
            scaling_handoff._runner_waited(process, returncode)
            raise
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


def _capture_descriptor_pin(descriptor):
    """Bind the original capture slot, including access and inheritance flags."""
    metadata = os.fstat(descriptor)
    return ((metadata.st_dev, metadata.st_ino, stat.S_IFMT(metadata.st_mode), metadata.st_uid),
            fcntl.fcntl(descriptor, fcntl.F_GETFL), fcntl.fcntl(descriptor, fcntl.F_GETFD))


def _require_capture_descriptor(descriptor, pin, label):
    try:
        if _capture_descriptor_pin(descriptor) == pin:
            return
    except OSError:
        pass
    raise BootstrapError(f"{label} original descriptor changed")


def _close_capture_descriptor(descriptor, pin):
    try:
        if _capture_descriptor_pin(descriptor) == pin:
            os.close(descriptor)
    except OSError:
        pass


def _open_capture_descriptor(path, flags, expected_identity, label, *, dir_fd=None):
    """Acquire one selected read-only slot without closing a reused error slot."""
    descriptor = os.open(path, flags, **({} if dir_fd is None else {'dir_fd': dir_fd}))
    mask = os.O_ACCMODE | os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC
    def admitted(pin):
        return (pin[0] == expected_identity and pin[1] & mask == flags & mask
                and pin[2] == fcntl.FD_CLOEXEC)
    try:
        pin = _capture_descriptor_pin(descriptor)
        if not admitted(pin):
            raise BootstrapError(f"{label} original descriptor changed while opened")
        return descriptor, pin
    except BaseException:
        # The selected inode and explicit open flags also bound cleanup when
        # initial fstat/fcntl acquisition itself fails before a full pin exists.
        try:
            pin = _capture_descriptor_pin(descriptor)
            if admitted(pin):
                _close_capture_descriptor(descriptor, pin)
        except OSError:
            pass
        raise


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
    parent_pin = _capture_descriptor_pin(parent_fd)
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
        descriptor, descriptor_pin = _open_capture_descriptor(name, flags,
            (before.st_dev, before.st_ino, stat.S_IFMT(before.st_mode), before.st_uid), label, dir_fd=parent_fd)
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
            _require_capture_descriptor(parent_fd, parent_pin, label)
            _require_capture_descriptor(descriptor, descriptor_pin, label)
            block = os.read(descriptor, min(1024 * 1024, maximum_bytes - total + 1))
            _require_capture_descriptor(descriptor, descriptor_pin, label)
            _require_capture_descriptor(parent_fd, parent_pin, label)
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
        _require_capture_descriptor(descriptor, descriptor_pin, label)
        _require_capture_descriptor(parent_fd, parent_pin, label)
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
        _close_capture_descriptor(descriptor, descriptor_pin)


def _capture_bounded_large_file(path: Path, label: str, *, maximum_bytes: int) -> LargeFileSnapshot:
    """Capture through a checked current parent with an explicit allocation cap."""
    _scaling_require(type(maximum_bytes) is int and maximum_bytes >= 0)
    if not path.is_absolute() or path.resolve() != path:
        raise BootstrapError(f"{label} path is not canonical")
    parent = path.parent.lstat()
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW
    descriptor, descriptor_pin = _open_capture_descriptor(path.parent, flags,
        (parent.st_dev, parent.st_ino, stat.S_IFMT(parent.st_mode), parent.st_uid), label)
    try:
        opened = os.fstat(descriptor)
        identity = lambda row: (row.st_dev,row.st_ino,row.st_mode,row.st_uid)
        if not stat.S_ISDIR(parent.st_mode) or identity(parent) != identity(opened):
            raise BootstrapError(f"{label} parent changed while it was opened")
        _require_capture_descriptor(descriptor, descriptor_pin, label)
        result = _capture_large_file_at(descriptor,path.name,path,label,maximum_bytes=maximum_bytes)
        _require_capture_descriptor(descriptor, descriptor_pin, label)
        if identity(path.parent.lstat()) != identity(opened):
            raise BootstrapError(f"{label} parent changed during capture")
        return result
    finally:
        _close_capture_descriptor(descriptor, descriptor_pin)


def _require_large_file_unchanged(
    snapshot: LargeFileSnapshot, label: str
) -> None:
    if _capture_bounded_large_file(snapshot.path, label, maximum_bytes=snapshot.size) != snapshot:
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
            "-B",
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
            "-B",
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
    marker_record = _framework_python_marker_record(inventory)
    launcher = marker_record["relocation"]["artifacts"]["launcher"]
    if (
        archive.mode != _TOOL_MODE
        or inventory.mode != _DATA_MODE
        or (
            launcher["source"]["mode"],
            launcher["source"]["sha256"],
            launcher["source"]["size_bytes"],
        )
        != (
            f"{protected_python.mode:04o}",
            protected_python.sha256,
            protected_python.size,
        )
        or (
            launcher["derived"]["mode"],
            launcher["derived"]["sha256"],
            launcher["derived"]["size_bytes"],
        )
        != (f"{archive.mode:04o}", archive.sha256, archive.size)
    ):
        raise BootstrapError(
            "archived framework Python relocation binding is not exact"
        )
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
            "-B",
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
            "-B",
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
    if (sys.flags.isolated != 1 or sys.flags.dont_write_bytecode != 1
            or sys.flags.no_site != 1):
        raise BootstrapError(
            "bootstrap must be started by protected Python with -I -B -S"
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
        ("scaling_plan", args.scaling_plan, args.expected_scaling_plan_sha256, 1024 * 1024, False),
        ("scaling_budget", args.scaling_budget, args.expected_scaling_budget_sha256, 1024 * 1024, False),
        ("scaling_handoff_helper", args.scaling_handoff_helper,
            args.expected_scaling_handoff_helper_sha256, _MAX_HELPER_BYTES, False),
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
            "runtime_helper_cli",
            args.runtime_helper_cli,
            args.expected_runtime_helper_cli_sha256,
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
    if 'CARGO_HOME' not in runner_extra_environment:
        raise BootstrapError("release requires an explicit protected CARGO_HOME input")
    scaling_cargo_cache = _absolute_resolved_existing(
        Path(runner_extra_environment['CARGO_HOME']), 'original release Cargo cache')
    scaling_dependencies = _absolute_resolved_existing(
        args.scaling_dependency_source, 'original scaling dependency source')
    if (not scaling_dependencies.is_dir() or _inside(scaling_dependencies, candidate)
            or not scaling_cargo_cache.is_dir()):
        raise BootstrapError("scaling dependency and Cargo cache roots must be original external directories")
    formal_replay_environment_names = {
        "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT",
        "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT",
        "IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256",
        "IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL",
    }
    if not formal_replay_environment_names.issubset(runner_extra_environment):
        raise BootstrapError(
            "production bootstrap requires one externally signed formal replay bundle"
        )
    formal_replay_source = _absolute_resolved_existing(
        Path(
            runner_extra_environment[
                "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT"
            ]
        ),
        "formal replay source receipt",
    )
    formal_replay_release_root = _absolute_resolved_existing(
        Path(
            runner_extra_environment[
                "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT"
            ]
        ),
        "formal replay release root",
    )
    if (
        formal_replay_source.name != "receipt.json"
        or not formal_replay_source.is_file()
        or formal_replay_source.is_symlink()
        or not formal_replay_release_root.is_dir()
        or formal_replay_release_root.is_symlink()
        or _DIGEST_RE.fullmatch(
            runner_extra_environment[
                "IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256"
            ]
        )
        is None
        or _FORMAL_REPLAY_PRINCIPAL_RE.fullmatch(
            runner_extra_environment[
                "IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL"
            ]
        )
        is None
    ):
        raise BootstrapError(
            "formal replay signing inputs are not canonical V1 release inputs"
        )
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
    scaling_invocation: ReleaseInvocationRoot | None = None
    scaling_operation: BootstrapScalingOperation | None = None
    scaling_handoff: FixedScalingHandoff | None = None
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
            "scaling_plan": "scaling-plan.json",
            "scaling_budget": "scaling-budget.json",
            "scaling_handoff_helper": "scaling-handoff.py",
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
            "runtime_helper_cli": "copy_sumeragi_v2_release_cargo_cache_cli.py",
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
            ["-I", "-B", "-S", "-c", python_probe_code],
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
            "-B",
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

        if framework_python_record is None:
            raise BootstrapError("fixed scaling requires the authenticated framework Python runtime")
        scaling_invocation = allocate_release_invocation_root(candidate, evidence, scaling_cargo_cache)
        scaling_operation = BootstrapScalingOperation(scaling_invocation, identity_snapshot,
            archives['python'], archives['manifest_helper'], runner_tool_archives['rustc'],
            archives['scaling_plan'], archives['scaling_budget'], archives['scaling_handoff_helper'],
            evidence, evidence_fd, _canonical_json(framework_python_record), environment,
            args.command_timeout_seconds, scaling_dependencies, args.scaling_machine_id,
            args.scaling_storage_model, args.scaling_observation_overhead_seconds,
            args.scaling_preflight_timeout_seconds)
        scaling_handoff = FixedScalingHandoff(scaling_operation.invocation_sha256, scaling_operation)
        scaling_environment = {
            'IROHA_RELEASE_INVOCATION_ROOT': str(scaling_invocation.path),
            'IROHA_RELEASE_TEMP_BASE': str(scaling_invocation.base),
            'IROHA_RELEASE_SCALING_GATE_FD': str(scaling_handoff.runner_descriptor),
            'IROHA_RELEASE_SCALING_INVOCATION_SHA256': scaling_operation.invocation_sha256,
            'IROHA_RELEASE_SCALING_CHALLENGE': scaling_handoff.challenge,
            'IROHA_RELEASE_SCALING_HANDOFF_HELPER_SHA256': archives['scaling_handoff_helper'].sha256,
        }
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
                **scaling_environment,
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
                "scaling_handoff": scaling_environment,
                "scaling_preflight_timeout_seconds": args.scaling_preflight_timeout_seconds,
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
                        "-B",
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
            scaling_handoff=scaling_handoff,
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
        original_scaling_observation = scaling_handoff.revalidate_observation()
        scaling_execution = scaling_operation.revalidate_final(original_scaling_observation)
        scaling_record_api = scaling_operation.record_api

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
            scaling_execution=scaling_execution,
        )
        if retained_result_snapshot is None or retained_validation_ack is None:
            raise BootstrapError("production release lacks protected retained result and validator acknowledgment")
        if retained_result_snapshot is not None:
            retained_failure_cleanup = _private_directory_snapshot(
                retained_release_root, "retained release cleanup root"
            )
        _prune_authenticated_sdk_source_manifest(
            evidence_fd, archives["sdk_dependency_bundle_manifest"]
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
            scaling_execution=scaling_execution,
            scaling_record_api=scaling_record_api,
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
            scaling_execution=scaling_execution,
        )
        _scaling_require(scaling_operation.revalidate_final(
            scaling_handoff.revalidate_observation()) is scaling_execution)
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
        _scaling_require(scaling_operation.revalidate_final(
            scaling_handoff.revalidate_observation()) is scaling_execution)
        scaling_projection = scaling_record_api.receipt_projection(
            scaling_record_api.decode_parent_execution(scaling_execution.data))
        release_completion_value = {
            "schema_version": 2,
            "result": "release-complete",
            "scaling_execution": scaling_projection,
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
            if label == "sdk_dependency_bundle_manifest":
                _require_sdk_source_manifest_pruned(evidence_fd, snapshot)
                continue
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
        _scaling_require(scaling_operation.revalidate_final(
            scaling_handoff.revalidate_observation()) is scaling_execution)
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
        # No failure cleanup may release inputs or delete files beneath a child
        # whose original natural wait is still unresolved.
        if scaling_handoff is not None: scaling_handoff.close()
        if scaling_operation is not None: scaling_operation.release()
        if scaling_invocation is not None: scaling_invocation.close()
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


def _scaling_preflight_timeout(value: str) -> int:
    """Separate bounded preflight duration; helper and experiment clocks differ."""
    try:
        seconds = int(value)
    except (TypeError, ValueError) as error:
        raise argparse.ArgumentTypeError("preflight timeout must be integer seconds") from error
    if not 600 <= seconds <= 86400:
        raise argparse.ArgumentTypeError("preflight timeout must be 600..86400 seconds")
    return seconds


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--scaling-plan", type=Path, required=True)
    parser.add_argument("--expected-scaling-plan-sha256", required=True)
    parser.add_argument("--scaling-budget", type=Path, required=True)
    parser.add_argument("--expected-scaling-budget-sha256", required=True)
    parser.add_argument("--scaling-handoff-helper", type=Path, required=True)
    parser.add_argument("--expected-scaling-handoff-helper-sha256", required=True)
    parser.add_argument("--scaling-dependency-source", type=Path, required=True)
    parser.add_argument("--scaling-machine-id", required=True)
    parser.add_argument("--scaling-storage-model", required=True)
    parser.add_argument("--scaling-observation-overhead-seconds", type=_positive_int, required=True)
    parser.add_argument("--scaling-preflight-timeout-seconds", type=_scaling_preflight_timeout,
        default=_DEFAULT_SCALING_PREFLIGHT_TIMEOUT_SECONDS,
        help="one complete preflight deadline in seconds (600..86400; default 28800)")
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
    parser.add_argument("--runtime-helper-cli", type=Path, required=True)
    parser.add_argument("--expected-runtime-helper-cli-sha256", required=True)
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
