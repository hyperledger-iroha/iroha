#!/usr/bin/env python3
"""Attest and dispatch the pre-provisioned Taira release controller bundle.

This file is the reviewed source for the fixed runner command
``/usr/local/libexec/iroha-taira-release-controller-v1``.  GitHub Actions must
never run this file from a checkout, feed privileged Python through stdin, or
create a privileged controller tree.  Trusted runner provisioning installs the
launcher and its exact root-owned closure before a release is requested.

The launcher verifies its own inode and digest, the complete installed closure,
the workflow commit, the controller version, and a root-owned runner identity
before it will dispatch one allow-listed operation.  Child processes receive a
fixed, secret-free environment; all authority paths and credentials must be
passed explicitly to the operation that owns them.
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
from pathlib import Path, PurePosixPath
from typing import Callable, NoReturn, Sequence

try:
    from scripts import taira_privacy_rollout_contract as rollout_observation
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_privacy_rollout_contract as rollout_observation

SCHEMA = "iroha.taira.release_controller_closure"
SCHEMA_VERSION = 1
MANIFEST_NAME = "authority-controller-v1.json"
CONTROLLER_VERSION = "1"
RUNNER_TRUST_SCHEMA_VERSION = 2
CONTROLLER_COMMAND = Path("/usr/local/libexec/iroha-taira-release-controller-v1")
CONTROLLER_ROOT = Path("/usr/local/libexec/iroha-taira-release-controller-v1.d")
RUNNER_TRUST_FILE = Path("/etc/iroha/taira-release-runner-v1.json")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
SHA256_RE = re.compile(r"[0-9a-f]{64}")
TRUST_ID_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,127}")
REPOSITORY_RE = re.compile(
    r"[a-z0-9.-]+(?::[1-9][0-9]{0,4})?/[a-z0-9]+(?:[._/-][a-z0-9]+)*"
)
SUFFIX_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,47}")
VERSION_RE = re.compile(r"[1-9][0-9]*\.[0-9]+\.[0-9]+")
MAX_CONTROLLER_BYTES = 8 * 1024 * 1024
MAX_RUNNER_TRUST_BYTES = 256 * 1024
MAX_TRUSTED_EXECUTABLE_BYTES = 256 * 1024 * 1024
MAX_HANDOFF_FILES = 20_000
MAX_HANDOFF_FILE_BYTES = 2 * 1024 * 1024 * 1024
MAX_HANDOFF_TOTAL_BYTES = 8 * 1024 * 1024 * 1024
MAX_OPERATION_ARG_BYTES = 16 * 1024
HANDOFF_MANIFEST = "handoff-inventory-v1.json"
BOI_QUALIFICATION_ISOLATION_CONTRACT = (
    "iroha.taira.boi-native-isolation-broker.v1"
)
BOI_QUALIFICATION_RUN_BINDING_CONTRACT = (
    "iroha.taira.boi-authenticated-run-nonce.v1"
)
COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT = (
    "iroha.taira.complete-source-identity-attestation.v1"
)
BOI_QUALIFICATION_ISSUANCE_BARRIER = (
    "missing preprovisioned iroha.taira.boi-native-isolation-broker.v1: "
    "candidate archive parsing, ABI loading/symbol inspection, wheel and worker "
    "probes must run under the attested runtime UID/GID with no_new_privs, "
    "closed inherited fds, a scrubbed environment, RLIMIT and stdout/stderr "
    "bounds, a network-denying sandbox, a new session/process-group kill, and "
    "residual-descendant validation; the distinct pinned qualification signer "
    "must be reachable only through an authority-UID-authenticated endpoint "
    "inaccessible to runtime, after every runtime child has exited and candidate "
    "hashes have been rechecked; missing preprovisioned "
    "iroha.taira.boi-authenticated-run-nonce.v1: caller workflow run ID/attempt "
    "must not authorize qualification or replay identity; missing preprovisioned "
    "iroha.taira.complete-source-identity-attestation.v1: a root-owned authority "
    "record must independently bind source commit, DPN validator release commit, "
    "the exact canonical Cargo.lock digest, and workspace source-manifest digest "
    "(or one stronger immutable candidate identity); caller-echoed values are not "
    "release authority"
)
DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT = (
    "iroha.taira.deploy-authenticated-run-nonce.v1"
)
DEPLOY_ISSUANCE_BARRIER = (
    "missing preprovisioned iroha.taira.deploy-authenticated-run-nonce.v1: "
    "neither workflow run ID nor attempt may authorize deployment or replay "
    "consumption; missing preprovisioned "
    "iroha.taira.complete-source-identity-attestation.v1: a root-owned authority "
    "record must independently bind source commit, DPN validator release commit, "
    "the exact canonical Cargo.lock digest, and workspace source-manifest digest "
    "(or one stronger immutable candidate identity); deploy-reset is disabled for "
    "both dry-run and apply before attestation or path inspection"
)
AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA = (
    "iroha.taira.authenticated-rollout-observation-authority.v1"
)
AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE = (
    "iroha.taira.authenticated-rollout-observation-replay.v1"
)
AUTHENTICATED_ROLLOUT_OBSERVATION_ISSUANCE_BARRIER = (
    "missing preprovisioned "
    "iroha.taira.authenticated-rollout-observation-authority.v1: rollout "
    "verification and publication require a canonical authority-origin envelope "
    "under a separately pinned trust root inaccessible to runtime, deploy, "
    "candidate, release, and publication signers; it must bind exact plan and "
    "observation bytes, admitted/deployed candidate and source identity, "
    "qualification and deploy receipts, four-peer/public-Torii, supervisor, host, "
    "installation and installed-controller identities, plus a fresh run nonce, "
    "issued time, expiry, and replay identity in "
    "iroha.taira.authenticated-rollout-observation-replay.v1; path ownership, "
    "self-hashes, workflow IDs, caller markers, environment values, signer reuse, "
    "stale runs, splices, and legacy unsigned observations cannot provision it"
)
PYTHON_ENV_SCRUBBER = (
    "import os,runpy,sys;"
    "names=('HOME','LANG','LC_ALL','PATH','TMPDIR');"
    "clean={name:os.environ[name] for name in names};"
    "os.environ.clear();os.environ.update(clean);"
    "path=sys.argv[1];sys.argv=sys.argv[1:];"
    "runpy.run_path(path,run_name='__main__')"
)

COMMON_FILES = (
    "scripts/compute_workspace_source_manifest.py",
    "scripts/seal_taira_release_controllers.py",
    "scripts/taira_authority_client.py",
)
LINUX_FILES = COMMON_FILES + (
    "scripts/build_privacy_v1_boi_handoff.py",
    "scripts/check_native_sdk_abi22_artifact.py",
    "scripts/finalize_taira_rollout_authority.py",
    "scripts/generate_release_manifest.py",
    "scripts/release_artifact_contract.py",
    "scripts/release_manifest_signing.py",
    "scripts/snapshot_taira_public_privacy_inputs.py",
    "scripts/taira_privacy_protocol_receipt.py",
    "scripts/taira_release_authority.py",
    "scripts/taira_rollout_admission.py",
)
MACOS_FILES = COMMON_FILES + (
    "configs/soranexus/taira/check_mcp_rollout.sh",
    "scripts/build_privacy_v1_boi_handoff.py",
    "scripts/build_taira_public_v2_prerequisite_handoff.py",
    "scripts/build_taira_rollout_candidate.py",
    "scripts/capture_taira_macos_four_peer_receipt.py",
    "scripts/capture_taira_privacy_protocol_four_peer_receipt.py",
    "scripts/check_native_sdk_abi22_artifact.py",
    "scripts/close_taira_publication_handoff.py",
    "scripts/close_taira_qualification_handoff.py",
    "scripts/deploy_taira_v21_reset.py",
    "scripts/deploy_taira_v21_reset_authority.py",
    "scripts/deploy_taira_v21_reset_health.py",
    "scripts/extract_authenticated_taira_privacy_release.py",
    "scripts/generate_release_manifest.py",
    "scripts/prepare_taira_empty_reset_bundle.py",
    "scripts/publish_taira_rollout.py",
    "scripts/release_artifact_contract.py",
    "scripts/release_manifest_signing.py",
    "scripts/render_taira_validator_bundle.py",
    "scripts/taira_constants.py",
    "scripts/taira_peer_supervisor.py",
    "scripts/taira_privacy_action_driver_ipc.py",
    "scripts/taira_privacy_governance_authority.py",
    "scripts/taira_privacy_protocol_receipt.py",
    "scripts/taira_privacy_sealed_controller.py",
    "scripts/taira_privacy_verange_case_plan.py",
    "scripts/taira_privacy_rollout_contract.py",
    "scripts/taira_release_authority.py",
    "scripts/taira_rollout_admission.py",
    "scripts/write_release_sha256sums.py",
    "configs/soranexus/taira/privacy_rollout_plan_v1.json",
)
PLATFORM_FILES = {"linux": LINUX_FILES, "macos": MACOS_FILES}

# An operation is available only on the runner role that needs it.  In
# particular, no role which executes a source-built validator can invoke a
# release signer or publish operation.
ROLE_OPERATIONS: dict[str, tuple[str, set[str]]] = {
    "public-input-authority": ("linux", {"snapshot-public-privacy"}),
    "linux-authority": ("linux", {"finalize-linux"}),
    "macos-qualification": (
        "macos",
        {
            "extract-privacy",
            "prepare-reset",
            "capture-four-peer",
        },
    ),
    "macos-candidate-authority": (
        "macos",
        {"assemble-candidate"},
    ),
    "linux-boi-qualification": (
        "linux",
        {"admit", "assemble-boi"},
    ),
    "macos-deploy": (
        "macos",
        {
            "extract-privacy",
            "prepare-reset",
            "deploy-reset",
            "check-public",
            "verify-privacy-rollout",
        },
    ),
    "macos-publish": (
        "macos",
        {
            "build-public-soak-candidate",
            "build-public-soak-publication",
            "publish-rollout",
        },
    ),
}
# Root is retained only by installed orchestration whose filesystem transition
# genuinely requires it.  Capture is a controller-owned composite: its hostile
# validator/harness child runs as the runtime identity, and only the installed
# close helper runs as root after the private receipt has been produced.
ROOT_REQUIRED_OPERATIONS = frozenset(
    {
        "snapshot-public-privacy",
        "capture-four-peer",
        "deploy-reset",
        "publish-rollout",
    }
)

PYTHON_OPERATIONS = {
    "snapshot-public-privacy": "scripts/snapshot_taira_public_privacy_inputs.py",
    "finalize-linux": "scripts/finalize_taira_rollout_authority.py",
    "extract-privacy": "scripts/extract_authenticated_taira_privacy_release.py",
    "prepare-reset": "scripts/prepare_taira_empty_reset_bundle.py",
    "capture-four-peer": "scripts/capture_taira_macos_four_peer_receipt.py",
    "assemble-candidate": "scripts/build_taira_rollout_candidate.py",
    "assemble-boi": "scripts/build_privacy_v1_boi_handoff.py",
    "deploy-reset": "scripts/deploy_taira_v21_reset.py",
    "build-public-soak-candidate": (
        "scripts/build_taira_public_v2_prerequisite_handoff.py"
    ),
    "build-public-soak-publication": (
        "scripts/build_taira_public_v2_prerequisite_handoff.py"
    ),
    "publish-rollout": "scripts/publish_taira_rollout.py",
    "verify-privacy-rollout": "scripts/taira_privacy_rollout_contract.py",
    "admit": "scripts/taira_rollout_admission.py",
}
QUALIFICATION_CLOSE_HELPER = "scripts/close_taira_qualification_handoff.py"
PRIVACY_CAPTURE_HELPER = "scripts/capture_taira_privacy_protocol_four_peer_receipt.py"
PUBLICATION_CLOSE_HELPER = "scripts/close_taira_publication_handoff.py"
BASH_OPERATIONS = {
    "check-public": "configs/soranexus/taira/check_mcp_rollout.sh",
}

OPERATION_FLAGS: dict[str, set[str]] = {
    "snapshot-public-privacy": {"--source", "--output", "--forbidden-root"},
    "finalize-linux": {
        "--evidence-root", "--archive", "--output-dir", "--commit",
        "--dpn-validator-release-commit",
        "--source-date-epoch", "--checkout-root", "--controller-manifest",
        "--public-privacy-input-dir", "--controller-digest", "--external-signer", "--signing-public-key",
        "--trusted-signing-fingerprint", "--release-manifest-verifier",
        "--trusted-release-manifest-verifier-sha256",
    },
    "extract-privacy": {
        "--archive", "--authority-dir", "--source-commit", "--cargo-lock-sha256",
        "--dpn-validator-release-commit",
        "--workspace-source-manifest-sha256", "--trusted-signing-fingerprint",
        "--release-manifest-verifier", "--trusted-release-manifest-verifier-sha256",
        "--output-dir",
    },
    "prepare-reset": {
        "--source-bundle", "--source-bundle-sha256", "--privacy-release-dir",
        "--genesis-external-signer", "--trusted-genesis-external-signer-sha256",
        "--onboarding-token-hash-tool", "--irohad-sha256", "--source-commit",
        "--dpn-validator-release-commit",
        "--cargo-lock-sha256", "--workspace-source-manifest-sha256",
        "--controller-manifest", "--controller-digest", "--output-bundle",
        "--kagemusha-release-root", "--kagemusha-activation-authority",
    },
    "capture-four-peer": {
        "--reset-bundle", "--validator-binary", "--supervisor",
        "--privacy-action-driver", "--privacy-network-driver", "--privacy-jindo-driver",
        "--linux-archive", "--exact12-matrix",
        "--artifact-handoff-sha256", "--source-commit", "--cargo-lock-sha256",
        "--dpn-validator-release-commit",
        "--workspace-source-manifest-sha256", "--restart-generation",
        "--source-identity", "--output", "--health-timeout-seconds",
    },
    "assemble-candidate": {
        "--source-commit", "--cargo-lock-sha256",
        "--dpn-validator-release-commit",
        "--workspace-source-manifest-sha256", "--source-date-epoch",
        "--linux-archive", "--linux-authority-dir", "--boi-artifact-handoff-dir",
        "--macos-receipt",
        "--privacy-protocol-evidence-dir",
        "--expected-receipt-id", "--controller-manifest", "--controller-digest",
        "--trusted-signing-fingerprint", "--release-manifest-verifier",
        "--trusted-release-manifest-verifier-sha256", "--external-signer",
        "--signing-public-key", "--output-directory",
    },
    "assemble-boi": {
        "--artifact-handoff-root", "--candidate-archive",
        "--candidate-authority-dir",
        "--expected-source-commit", "--expected-dpn-validator-release-commit",
        "--expected-cargo-lock-sha256",
        "--expected-workspace-source-manifest-sha256", "--expected-receipt-id",
        "--trusted-signing-fingerprint", "--release-manifest-verifier",
        "--trusted-release-manifest-verifier-sha256",
        "--qualification-external-signer",
        "--trusted-qualification-external-signer-sha256",
        "--qualification-signing-public-key",
        "--trusted-qualification-signing-fingerprint",
        "--workflow-run-id", "--workflow-run-attempt", "--output",
    },
    "deploy-reset": {
        "--bundle", "--binary", "--supervisor", "--admission-archive",
        "--admission-authority-dir", "--boi-qualified-handoff-root",
        "--expected-source-commit",
        "--expected-dpn-validator-release-commit",
        "--expected-cargo-lock-sha256", "--expected-workspace-source-manifest-sha256",
        "--expected-receipt-id", "--expected-artifact-handoff-sha256",
        "--expected-production-reset-manifest-sha256", "--trusted-signing-fingerprint",
        "--trusted-boi-qualification-public-key",
        "--trusted-boi-qualification-signing-fingerprint",
        "--expected-boi-qualification-host-id",
        "--expected-boi-qualification-installation-id",
        "--expected-boi-qualification-controller-digest",
        "--expected-workflow-run-id", "--expected-workflow-run-attempt",
        "--release-manifest-verifier", "--trusted-release-manifest-verifier-sha256",
        "--health-timeout-seconds", "--apply",
    },
    "publish-rollout": {
        "--candidate-root",
        "--expected-source-commit",
        "--expected-dpn-validator-release-commit",
        "--expected-cargo-lock-sha256",
        "--expected-workspace-source-manifest-sha256",
        "--expected-qualification-receipt-id",
        "--repository",
        "--suffix",
        "--rollout-plan",
        "--rollout-result",
        "--rollout-authority-envelope",
        "--rollout-durable-receipt",
    },
    "build-public-soak-candidate": {
        "--candidate-root",
        "--output",
    },
    "build-public-soak-publication": {
        "--candidate-root",
        "--candidate-handoff",
        "--publication-root",
        "--output",
    },
    "admit": {
        "--output", "--archive", "--authority-dir", "--expected-source-commit",
        "--expected-dpn-validator-release-commit",
        "--expected-cargo-lock-sha256", "--expected-workspace-source-manifest-sha256",
        "--expected-receipt-id", "--replay-ledger", "--trusted-signing-fingerprint",
        "--release-manifest-verifier", "--trusted-release-manifest-verifier-sha256",
    },
    "check-public": {
        "--public-root", "--validator-root", "--require-all-validators",
        "--expected-git-sha", "--expected-dpn-validator-release-commit",
        "--write-config",
    },
    "verify-privacy-rollout": {"--result"},
}
BOOLEAN_FLAGS = {"--apply", "--require-all-validators"}
REPEATED_FLAGS = {"--validator-root"}
OUTPUT_PATH_FLAGS = {
    "--output", "--output-dir", "--output-bundle", "--runtime-root",
    "--output-directory",
}
IMMUTABLE_HANDOFF_OUTPUT_PREFIXES = {
    "snapshot-public-privacy": "public-input-",
    "capture-four-peer": "qualification-receipt-",
    "assemble-boi": "boi-qualified-",
    "build-public-soak-candidate": "public-soak-candidate-",
    "build-public-soak-publication": "public-soak-publication-",
}
INPUT_PATH_FLAGS = {
    "--source", "--evidence-root", "--archive", "--checkout-root",
    "--public-privacy-input-dir",
    "--controller-manifest", "--external-signer", "--signing-public-key",
    "--qualification-external-signer", "--qualification-signing-public-key",
    "--trusted-boi-qualification-public-key",
    "--release-manifest-verifier", "--authority-dir", "--source-bundle",
    "--privacy-release-dir", "--genesis-external-signer",
    "--kagemusha-release-root",
    "--onboarding-token-hash-tool", "--reset-bundle", "--validator-binary",
    "--supervisor", "--linux-archive", "--linux-authority-dir",
    "--boi-artifact-handoff-dir", "--macos-receipt",
    "--privacy-protocol-evidence-dir",
    "--privacy-action-driver", "--privacy-network-driver", "--privacy-jindo-driver",
    "--exact12-matrix",
    "--bundle", "--binary", "--admission-archive",
    "--admission-authority-dir", "--boi-qualified-handoff-root",
    "--replay-ledger", "--source-identity",
    "--artifact-handoff-root", "--candidate-archive",
    "--candidate-authority-dir",
    "--candidate-root",
    "--candidate-handoff", "--publication-root",
    "--result", "--write-config",
    "--rollout-plan", "--rollout-result",
    "--rollout-authority-envelope", "--rollout-durable-receipt",
}
KAGEMUSHA_PREPARE_RESET_FLAGS = frozenset(
    {"--kagemusha-release-root", "--kagemusha-activation-authority"}
)
POSITIONAL_COMMANDS = {
    "assemble-candidate": {"assemble"},
    "admit": {"verify", "init-replay-ledger"},
}
REQUIRED_FLAGS: dict[tuple[str, str | None], set[str]] = {
    ("snapshot-public-privacy", None): OPERATION_FLAGS["snapshot-public-privacy"],
    ("finalize-linux", None): OPERATION_FLAGS["finalize-linux"],
    ("extract-privacy", None): OPERATION_FLAGS["extract-privacy"],
    # Ordinary qualification resets remain supported.  A Kagemusha reset is
    # selected only by supplying its complete release-root/authority pair.
    ("prepare-reset", None): (
        OPERATION_FLAGS["prepare-reset"] - KAGEMUSHA_PREPARE_RESET_FLAGS
    ),
    ("capture-four-peer", None): OPERATION_FLAGS["capture-four-peer"],
    ("assemble-candidate", "assemble"): OPERATION_FLAGS["assemble-candidate"],
    ("assemble-boi", None): OPERATION_FLAGS["assemble-boi"],
    ("deploy-reset", None): OPERATION_FLAGS["deploy-reset"] - {"--apply"},
    ("build-public-soak-candidate", None): OPERATION_FLAGS[
        "build-public-soak-candidate"
    ],
    ("build-public-soak-publication", None): OPERATION_FLAGS[
        "build-public-soak-publication"
    ],
    ("publish-rollout", None): OPERATION_FLAGS["publish-rollout"],
    ("admit", "init-replay-ledger"): {"--output"},
    ("admit", "verify"): OPERATION_FLAGS["admit"] - {"--output"},
    ("check-public", None): {
        "--public-root", "--validator-root", "--require-all-validators",
        "--expected-git-sha", "--expected-dpn-validator-release-commit",
        "--write-config",
    },
    ("verify-privacy-rollout", None): {"--result"},
}

# Caller-selected executable paths are never authorized merely because they
# happen to live below a private runner directory.  Every authority/tool
# executable is pinned by the root-owned runner trust record for the exact
# operation and flag that consumes it.  The validator binary is deliberately
# excluded: it is hostile release input and may only come from a root-frozen
# handoff, then run under the dedicated runtime identity.
TRUSTED_EXECUTABLE_FLAGS = frozenset(
    {
        "--external-signer",
        "--qualification-external-signer",
        "--genesis-external-signer",
        "--onboarding-token-hash-tool",
        "--oras",
        "--release-manifest-verifier",
        "--supervisor",
    }
)
EXECUTABLE_DIGEST_FLAGS = {
    "--genesis-external-signer": "--trusted-genesis-external-signer-sha256",
    "--qualification-external-signer": (
        "--trusted-qualification-external-signer-sha256"
    ),
    "--release-manifest-verifier": "--trusted-release-manifest-verifier-sha256",
}
SEALED_EXECUTABLE_DEPENDENCIES: dict[str, dict[str, str]] = {
    "publish-rollout": {
        "--oras": "--trusted-oras-sha256",
        "--external-signer": "--trusted-external-signer-sha256",
        "--release-manifest-verifier": (
            "--trusted-release-manifest-verifier-sha256"
        ),
    }
}
SEALED_INPUT_DEPENDENCIES: dict[str, set[str]] = {
    "publish-rollout": {"--registry-config", "--signing-public-key"}
}
TRUSTED_LITERAL_FLAGS: dict[str, set[str]] = {
    "assemble-boi": {
        "--trusted-signing-fingerprint",
        "--trusted-qualification-signing-fingerprint",
    },
    "deploy-reset": {
        "--trusted-signing-fingerprint",
        "--trusted-boi-qualification-signing-fingerprint",
        "--expected-boi-qualification-host-id",
        "--expected-boi-qualification-installation-id",
        "--expected-boi-qualification-controller-digest",
    },
    "publish-rollout": {
        "--expected-oras-version",
        "--repository",
        "--suffix",
        "--trusted-signing-fingerprint",
    }
}
SOURCE_COMMIT_FLAGS = {
    "finalize-linux": "--commit",
    "extract-privacy": "--source-commit",
    "prepare-reset": "--source-commit",
    "capture-four-peer": "--source-commit",
    "assemble-candidate": "--source-commit",
    "assemble-boi": "--expected-source-commit",
    "deploy-reset": "--expected-source-commit",
    "publish-rollout": "--expected-source-commit",
    "admit": "--expected-source-commit",
    "check-public": "--expected-git-sha",
}
ROLE_OPERATION_IDENTITY: dict[tuple[str, str], str] = {
    ("public-input-authority", "snapshot-public-privacy"): "root",
    ("linux-authority", "finalize-linux"): "authority",
    ("macos-qualification", "extract-privacy"): "runtime",
    ("macos-qualification", "prepare-reset"): "runtime",
    ("macos-qualification", "capture-four-peer"): "runtime",
    ("macos-candidate-authority", "assemble-candidate"): "authority",
    ("linux-boi-qualification", "admit"): "authority",
    ("linux-boi-qualification", "assemble-boi"): "authority",
    ("macos-deploy", "extract-privacy"): "runtime",
    ("macos-deploy", "prepare-reset"): "runtime",
    ("macos-deploy", "deploy-reset"): "root",
    ("macos-deploy", "check-public"): "staging",
    ("macos-deploy", "verify-privacy-rollout"): "authority",
    ("macos-publish", "build-public-soak-candidate"): "authority",
    ("macos-publish", "build-public-soak-publication"): "authority",
    ("macos-publish", "publish-rollout"): "authority",
}

# Root orchestration may pass the installed supervisor to a runtime service and
# may pass a native verifier to deploy's admission preflight.  The trust record
# still binds the identity under which that external executable is permitted to
# execute; a root run_as value is never valid.
EXECUTABLE_RUN_AS_OVERRIDES = {
    ("capture-four-peer", "--supervisor"): "runtime",
    ("deploy-reset", "--supervisor"): "runtime",
    ("deploy-reset", "--release-manifest-verifier"): "authority",
}
SENSITIVE_TRUSTED_INPUT_FLAGS = frozenset(
    {
        "--registry-config",
        "--result",
        "--signing-public-key",
        "--qualification-signing-public-key",
        "--trusted-boi-qualification-public-key",
        "--source",
        "--source-bundle",
        "--write-config",
    }
)
ROLLOUT_OBSERVATION_INPUT_FLAGS = frozenset(
    {
        "--rollout-plan",
        "--rollout-result",
        "--rollout-authority-envelope",
        "--rollout-durable-receipt",
    }
)


class ControllerSealError(RuntimeError):
    """The installed release-controller trust contract was not satisfied."""


def _fail(message: str) -> NoReturn:
    raise ControllerSealError(message)


def _require_authenticated_rollout_observation_authority() -> None:
    """Authenticate the fixed observation service before controller I/O."""

    try:
        rollout_observation.require_authenticated_rollout_observation_authority_provisioned()
    except rollout_observation.RolloutContractError as error:
        raise ControllerSealError(
            f"{AUTHENTICATED_ROLLOUT_OBSERVATION_ISSUANCE_BARRIER}: {error}"
        ) from error


def canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def controller_digest(manifest_bytes: bytes) -> str:
    return hashlib.sha256(
        b"iroha.taira.release-controller-closure.v1\0" + manifest_bytes
    ).hexdigest()


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _validate_relative(value: object) -> str:
    if not isinstance(value, str):
        _fail("controller manifest path must be a string")
    candidate = PurePosixPath(value)
    if (
        candidate.is_absolute()
        or not candidate.parts
        or any(part in {"", ".", ".."} for part in candidate.parts)
        or candidate.as_posix() != value
    ):
        _fail(f"controller manifest path is not canonical: {value!r}")
    return value


def _read_stable(path: Path, maximum: int) -> bytes:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
    ):
        _fail(f"controller is not one bounded single-link regular file: {path}")
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"controller changed while opening: {path}")
        payload = bytearray()
        while len(payload) < before.st_size:
            chunk = os.read(descriptor, min(64 * 1024, before.st_size - len(payload)))
            if not chunk:
                _fail(f"controller was truncated while reading: {path}")
            payload.extend(chunk)
        if os.read(descriptor, 1) or _identity(os.fstat(descriptor)) != _identity(before):
            _fail(f"controller changed while reading: {path}")
    finally:
        os.close(descriptor)
    if _identity(path.lstat()) != _identity(before):
        _fail(f"controller path changed while reading: {path}")
    return bytes(payload)


def _read_relative_stable(
    root: Path,
    relative: str,
    maximum: int,
    *,
    expected_uid: int | None = None,
    expected_gid: int | None = None,
    expected_mode: int | None = None,
) -> bytes:
    """Read a relative file without following any intermediate path component."""

    _validate_relative(relative)
    root_before = root.lstat()
    if not stat.S_ISDIR(root_before.st_mode) or stat.S_ISLNK(root_before.st_mode):
        _fail("handoff root is not one real directory")
    directory_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    file_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptors: list[int] = [os.open(root, directory_flags)]
    anchors: list[tuple[int, str, tuple[int, ...]]] = []
    try:
        if _identity(os.fstat(descriptors[0])) != _identity(root_before):
            _fail("handoff root changed while opening")
        parts = PurePosixPath(relative).parts
        current = descriptors[0]
        for component in parts[:-1]:
            before = os.stat(component, dir_fd=current, follow_symlinks=False)
            if not stat.S_ISDIR(before.st_mode) or stat.S_ISLNK(before.st_mode):
                _fail(f"handoff intermediate path is not a real directory: {relative}")
            child = os.open(component, directory_flags, dir_fd=current)
            descriptors.append(child)
            if _identity(os.fstat(child)) != _identity(before):
                _fail(f"handoff intermediate path changed while opening: {relative}")
            anchors.append((current, component, _identity(before)))
            current = child
        before = os.stat(parts[-1], dir_fd=current, follow_symlinks=False)
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1
            or before.st_size <= 0
            or before.st_size > maximum
            or (expected_uid is not None and before.st_uid != expected_uid)
            or (expected_gid is not None and before.st_gid != expected_gid)
            or (
                expected_mode is not None
                and stat.S_IMODE(before.st_mode) != expected_mode
            )
        ):
            _fail(f"handoff file is not one bounded single-link inode: {relative}")
        descriptor = os.open(parts[-1], file_flags, dir_fd=current)
        descriptors.append(descriptor)
        if _identity(os.fstat(descriptor)) != _identity(before):
            _fail(f"handoff file changed while opening: {relative}")
        anchors.append((current, parts[-1], _identity(before)))
        payload = bytearray()
        # The inode size is checked against an operation-specific cap before
        # allocating or buffering any file bytes.
        while len(payload) < before.st_size:
            chunk = os.read(descriptor, min(64 * 1024, before.st_size - len(payload)))
            if not chunk:
                _fail(f"handoff file was truncated while reading: {relative}")
            payload.extend(chunk)
        if os.read(descriptor, 1) or _identity(os.fstat(descriptor)) != _identity(before):
            _fail(f"handoff file changed while reading: {relative}")
        for (parent_fd, component, expected), opened_fd in zip(
            anchors,
            descriptors[1:],
        ):
            if (
                _identity(os.fstat(opened_fd)) != expected
                or _identity(
                    os.stat(
                        component,
                        dir_fd=parent_fd,
                        follow_symlinks=False,
                    )
                )
                != expected
            ):
                _fail(f"handoff path changed after descriptor read: {relative}")
        if _identity(os.fstat(descriptors[0])) != _identity(root_before):
            _fail("handoff root changed while reading")
        return bytes(payload)
    except OSError as exc:
        raise ControllerSealError(
            f"failed to read handoff path without following links: {relative}: {exc}"
        ) from exc
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def _relative_file_names_fd(
    root: Path,
    *,
    expected_uid: int | None = None,
    expected_gid: int | None = None,
    expected_file_mode: int | None = None,
    expected_directory_mode: int | None = None,
) -> list[str]:
    """Enumerate a tree using no-follow directory descriptors."""

    directory_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    root_before = root.lstat()
    root_fd = os.open(root, directory_flags)
    try:
        if _identity(os.fstat(root_fd)) != _identity(root_before):
            _fail("handoff root changed while enumerating")
        names: list[str] = []

        def walk(directory_fd: int, prefix: tuple[str, ...]) -> None:
            for name in sorted(os.listdir(directory_fd)):
                if not name or name in {".", ".."} or "/" in name:
                    _fail("handoff contains a noncanonical directory entry")
                info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                relative = "/".join((*prefix, name))
                if stat.S_ISLNK(info.st_mode):
                    _fail(f"handoff contains a symlink: {relative}")
                if stat.S_ISDIR(info.st_mode):
                    if (
                        (expected_uid is not None and info.st_uid != expected_uid)
                        or (expected_gid is not None and info.st_gid != expected_gid)
                        or (
                            expected_directory_mode is not None
                            and stat.S_IMODE(info.st_mode)
                            != expected_directory_mode
                        )
                    ):
                        _fail(
                            f"handoff directory ownership or mode differs: {relative}"
                        )
                    child = os.open(name, directory_flags, dir_fd=directory_fd)
                    try:
                        if _identity(os.fstat(child)) != _identity(info):
                            _fail(f"handoff directory changed while opening: {relative}")
                        walk(child, (*prefix, name))
                        if (
                            _identity(os.fstat(child)) != _identity(info)
                            or _identity(
                                os.stat(
                                    name,
                                    dir_fd=directory_fd,
                                    follow_symlinks=False,
                                )
                            )
                            != _identity(info)
                        ):
                            _fail(
                                f"handoff directory changed after enumeration: {relative}"
                            )
                    finally:
                        os.close(child)
                elif stat.S_ISREG(info.st_mode):
                    if info.st_nlink != 1:
                        _fail(f"handoff contains a hard-linked file: {relative}")
                    if (
                        (expected_uid is not None and info.st_uid != expected_uid)
                        or (expected_gid is not None and info.st_gid != expected_gid)
                        or (
                            expected_file_mode is not None
                            and stat.S_IMODE(info.st_mode) != expected_file_mode
                        )
                    ):
                        _fail(
                            f"handoff file ownership or mode differs: {relative}"
                        )
                    names.append(relative)
                else:
                    _fail(f"handoff contains a non-file entry: {relative}")

        walk(root_fd, ())
        if _identity(os.fstat(root_fd)) != _identity(root_before):
            _fail("handoff root changed while enumerating")
        return sorted(names)
    finally:
        os.close(root_fd)


def _require_canonical(path: Path, label: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must use one absolute lexical path")
    try:
        resolved = path.resolve(strict=True)
    except OSError as exc:
        raise ControllerSealError(f"cannot resolve {label}: {exc}") from exc
    if resolved != path:
        _fail(f"{label} must use its canonical physical path")
    return path


def _strict_positive_identity(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        _fail(f"{label} must be one positive integer")
    return value


def _ancestry_snapshot(path: Path) -> list[tuple[Path, tuple[int, ...]]]:
    """Capture every physical component from ``/`` through ``path``."""

    canonical = _require_canonical(path, "trusted path")
    rows: list[tuple[Path, tuple[int, ...]]] = []
    current = Path("/")
    rows.append((current, _identity(current.lstat())))
    for component in canonical.parts[1:]:
        current /= component
        rows.append((current, _identity(current.lstat())))
    return rows


def _revalidate_ancestry(rows: Sequence[tuple[Path, tuple[int, ...]]]) -> None:
    for path, expected in rows:
        if _identity(path.lstat()) != expected:
            _fail(f"trusted path ancestry changed during validation: {path}")


def _validate_identity_root(
    path: Path,
    label: str,
    uid: int,
    gid: int,
    *,
    controller_uid: int = 0,
    controller_gid: int = 0,
) -> Path:
    """Require an owner-private identity root behind controller-held parents."""

    canonical = _require_canonical(path, f"{label} root")
    rows = _ancestry_snapshot(canonical)
    leaf = canonical.lstat()
    if (
        not stat.S_ISDIR(leaf.st_mode)
        or stat.S_ISLNK(leaf.st_mode)
        or leaf.st_uid != uid
        or leaf.st_gid != gid
        or stat.S_IMODE(leaf.st_mode) != 0o700
    ):
        _fail(f"{label} root is not owned by its exact identity at mode 0700")
    for parent, expected in rows[:-1]:
        mode = expected[2]
        if (
            not stat.S_ISDIR(mode)
            or stat.S_ISLNK(mode)
            or expected[4] != controller_uid
            or expected[5] != controller_gid
            or mode & 0o022
        ):
            _fail(f"{label} root ancestry is not controller-owned and nonwritable")
    _revalidate_ancestry(rows)
    return canonical


def _validate_handoff_root(
    path: Path,
    *,
    controller_uid: int = 0,
    controller_gid: int = 0,
) -> Path:
    canonical = _require_canonical(path, "immutable handoff root")
    rows = _ancestry_snapshot(canonical)
    leaf = canonical.lstat()
    if (
        not stat.S_ISDIR(leaf.st_mode)
        or stat.S_ISLNK(leaf.st_mode)
        or leaf.st_uid != controller_uid
        or leaf.st_gid != controller_gid
        or stat.S_IMODE(leaf.st_mode) != 0o711
    ):
        _fail("immutable handoff root is not controller-owned exact mode 0711")
    for _component, expected in rows:
        mode = expected[2]
        if (
            not stat.S_ISDIR(mode)
            or stat.S_ISLNK(mode)
            or expected[4] != controller_uid
            or expected[5] != controller_gid
            or mode & 0o022
        ):
            _fail("immutable handoff root ancestry is not controller-held")
    _revalidate_ancestry(rows)
    return canonical


def _expected_operation_identity(role: str, operation: str) -> str:
    identity = ROLE_OPERATION_IDENTITY.get((role, operation))
    if identity is None:
        _fail("runner role lacks an exact operation identity")
    return identity


def _expected_executable_identity(role: str, operation: str, flag: str) -> str:
    identity = EXECUTABLE_RUN_AS_OVERRIDES.get(
        (operation, flag), _expected_operation_identity(role, operation)
    )
    if identity not in {"staging", "runtime", "authority"}:
        _fail("external executable cannot be authorized to inherit root")
    return identity


def _trusted_executable_flags(operation: str) -> set[str]:
    return (
        OPERATION_FLAGS[operation] & TRUSTED_EXECUTABLE_FLAGS
    ) | set(SEALED_EXECUTABLE_DEPENDENCIES.get(operation, {}))


def _trusted_input_flags(operation: str) -> set[str]:
    return (
        OPERATION_FLAGS[operation] & SENSITIVE_TRUSTED_INPUT_FLAGS
    ) | SEALED_INPUT_DEPENDENCIES.get(operation, set())


def _expected_executable_digest_flag(operation: str, flag: str) -> str | None:
    sealed = SEALED_EXECUTABLE_DEPENDENCIES.get(operation, {})
    if flag in sealed:
        return sealed[flag]
    return EXECUTABLE_DIGEST_FLAGS.get(flag)


def _validate_trusted_literal(flag: str, value: str) -> None:
    if flag == "--repository":
        if (
            REPOSITORY_RE.fullmatch(value) is None
            or ".." in value
            or "//" in value
        ):
            _fail("trusted OCI repository literal is noncanonical")
        return
    if flag == "--suffix":
        if value and (SUFFIX_RE.fullmatch(value) is None or ".." in value):
            _fail("trusted OCI suffix literal is noncanonical")
        return
    if flag == "--expected-oras-version":
        if VERSION_RE.fullmatch(value) is None:
            _fail("trusted ORAS version literal is noncanonical")
        return
    if flag in {
        "--trusted-signing-fingerprint",
        "--trusted-qualification-signing-fingerprint",
        "--trusted-boi-qualification-signing-fingerprint",
        "--expected-boi-qualification-controller-digest",
    }:
        if SHA256_RE.fullmatch(value) is None:
            _fail("trusted signing fingerprint literal is noncanonical")
        return
    if flag in {
        "--expected-boi-qualification-host-id",
        "--expected-boi-qualification-installation-id",
    }:
        if TRUST_ID_RE.fullmatch(value) is None:
            _fail("trusted BOI qualification identity literal is noncanonical")
        return
    _fail("trusted literal flag is not allow-listed")


def _require_distinct_release_and_qualification_signers(
    trusted_values: Sequence[dict[str, str]],
) -> None:
    """Reject a runner trust record that collapses the two signing roles."""

    values = {
        (row.get("operation"), row.get("flag")): row.get("value")
        for row in trusted_values
    }
    for operation, qualification_flag in (
        ("assemble-boi", "--trusted-qualification-signing-fingerprint"),
        ("deploy-reset", "--trusted-boi-qualification-signing-fingerprint"),
    ):
        release = values.get((operation, "--trusted-signing-fingerprint"))
        qualification = values.get((operation, qualification_flag))
        if release is None and qualification is None:
            continue
        if (
            not isinstance(release, str)
            or SHA256_RE.fullmatch(release) is None
            or not isinstance(qualification, str)
            or SHA256_RE.fullmatch(qualification) is None
        ):
            _fail("release and qualification signer trust is incomplete")
        if release == qualification:
            _fail("release and BOI qualification signing identities must be distinct")


def _require_attested_source_commit(
    operation: str,
    option_values: dict[str, list[str]],
    attestation: dict[str, object],
) -> None:
    """Bind only the commit to the closure; this is not complete source authority."""

    flag = SOURCE_COMMIT_FLAGS.get(operation)
    if flag is None or flag not in option_values:
        return
    source_commit = attestation.get("source_commit")
    if (
        not isinstance(source_commit, str)
        or COMMIT_RE.fullmatch(source_commit) is None
        or option_values.get(flag) != [source_commit]
    ):
        _fail("controller operation source commit differs from installed attestation")


def _validate_trusted_executable_path(
    path: Path, expected_sha256: str, *, exact_mode: int | None = None
) -> None:
    """Pin one executable and every ancestor to stable root-owned inodes."""

    if SHA256_RE.fullmatch(expected_sha256) is None:
        _fail("trusted executable SHA-256 is not canonical")
    canonical = _require_canonical(path, "trusted executable")
    rows = _ancestry_snapshot(canonical)
    leaf = canonical.lstat()
    if (
        not stat.S_ISREG(leaf.st_mode)
        or stat.S_ISLNK(leaf.st_mode)
        or leaf.st_nlink != 1
        or leaf.st_uid != 0
        or leaf.st_gid != 0
        or leaf.st_mode & 0o022
        or not leaf.st_mode & 0o111
        or (
            exact_mode is not None
            and stat.S_IMODE(leaf.st_mode) != exact_mode
        )
    ):
        _fail("trusted executable is not one root-owned nonwritable executable")
    for _component, expected in rows[:-1]:
        mode = expected[2]
        if (
            not stat.S_ISDIR(mode)
            or stat.S_ISLNK(mode)
            or expected[4] != 0
            or expected[5] != 0
            or mode & 0o022
        ):
            _fail("trusted executable ancestry is not root-owned and nonwritable")
    payload = _read_stable(canonical, MAX_TRUSTED_EXECUTABLE_BYTES)
    if hashlib.sha256(payload).hexdigest() != expected_sha256:
        _fail("trusted executable digest differs")
    _revalidate_ancestry(rows)


def _path_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _validate_trusted_input_path(
    path: Path,
    identity_roots: dict[str, Path],
    identity_ids: dict[str, tuple[int, int]],
) -> None:
    """Validate one exact pre-provisioned input without granting a subtree."""

    canonical = _require_canonical(path, "trusted input")
    rows = _ancestry_snapshot(canonical)
    owner_uid = 0
    owner_gid = 0
    containing_identity: str | None = None
    for name, root in identity_roots.items():
        if _path_within(canonical, root):
            containing_identity = name
            owner_uid, owner_gid = identity_ids[name]
            break
    leaf = canonical.lstat()
    if (
        stat.S_ISLNK(leaf.st_mode)
        or not (stat.S_ISREG(leaf.st_mode) or stat.S_ISDIR(leaf.st_mode))
        or (stat.S_ISREG(leaf.st_mode) and leaf.st_nlink != 1)
        or leaf.st_mode & 0o022
        or leaf.st_uid not in {0, owner_uid}
        or leaf.st_gid not in {0, owner_gid}
    ):
        _fail("trusted input is not one protected regular file or directory")
    if containing_identity is None:
        for _component, expected in rows[:-1]:
            mode = expected[2]
            if (
                not stat.S_ISDIR(mode)
                or stat.S_ISLNK(mode)
                or expected[4] != 0
                or expected[5] != 0
                or mode & 0o022
            ):
                _fail("trusted input ancestry is not root-owned and nonwritable")
    _revalidate_ancestry(rows)


def _validate_root_owned_release_root(path: Path) -> Path:
    """Authorize one canonical release root held outside runner identities."""

    canonical = _require_canonical(path, "Kagemusha release root")
    if canonical == Path("/"):
        _fail("Kagemusha release root cannot be the filesystem root")
    rows = _ancestry_snapshot(canonical)
    for _component, expected in rows:
        mode = expected[2]
        if (
            not stat.S_ISDIR(mode)
            or stat.S_ISLNK(mode)
            or expected[4] != 0
            or expected[5] != 0
            or mode & 0o022
        ):
            _fail(
                "Kagemusha release root ancestry must be root-owned and nonwritable"
            )
    _revalidate_ancestry(rows)
    return canonical


def _validate_privacy_rollout_input(
    path: Path,
    *,
    identity_root: Path,
    identity_uid: int,
    identity_gid: int,
    label: str,
    maximum: int,
) -> None:
    """Require one immutable owner-private canary/observation input."""

    canonical = _require_identity_path(
        path,
        identity_root,
        identity_uid,
        identity_gid,
        label=label,
    )
    info = canonical.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_nlink != 1
        or info.st_uid != identity_uid
        or info.st_gid != identity_gid
        or stat.S_IMODE(info.st_mode) != 0o400
        or info.st_size <= 0
        or info.st_size > maximum
    ):
        _fail(f"{label} must be one owner-private exact-mode-0400 regular file")
    _read_stable(canonical, maximum)


def _validate_publisher_trusted_input(
    path: Path,
    flag: str,
    authority_uid: int,
    authority_gid: int,
    authority_root: Path,
    trusted_fingerprint: str,
) -> None:
    """Enforce the publisher's exact config/key ownership and byte contract."""

    info = path.lstat()
    if flag == "--registry-config":
        parent = path.parent.lstat()
        if (
            not _path_within(path, authority_root)
            or not stat.S_ISDIR(parent.st_mode)
            or stat.S_ISLNK(parent.st_mode)
            or parent.st_uid != authority_uid
            or parent.st_gid != authority_gid
            or stat.S_IMODE(parent.st_mode) != 0o700
            or not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != authority_uid
            or info.st_gid != authority_gid
            or stat.S_IMODE(info.st_mode) != 0o400
            or info.st_size <= 0
        ):
            _fail("publisher registry config identity differs")
        _read_stable(path, MAX_RUNNER_TRUST_BYTES)
        return
    if flag == "--signing-public-key":
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != 0
            or info.st_gid != 0
            or stat.S_IMODE(info.st_mode) != 0o444
            or info.st_size != 32
        ):
            _fail("publisher signing public key identity differs")
        if hashlib.sha256(_read_stable(path, 32)).hexdigest() != trusted_fingerprint:
            _fail("publisher signing public key fingerprint differs")
        return
    _fail("publisher trusted input flag differs")


def _require_root_owned_file(
    path: Path,
    label: str,
    *,
    exact_mode: int,
    maximum: int = MAX_CONTROLLER_BYTES,
) -> bytes:
    payload = _read_stable(_require_canonical(path, label), maximum)
    info = path.lstat()
    if (
        info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) != exact_mode
    ):
        _fail(f"{label} must be root-owned and exact mode {exact_mode:04o}")
    return payload


def _relative_file_names(root: Path) -> list[str]:
    names: list[str] = []
    for current, directories, files in os.walk(root, followlinks=False):
        current_path = Path(current)
        for name in directories:
            path = current_path / name
            if stat.S_ISLNK(path.lstat().st_mode):
                _fail(f"installed controller tree contains a symlink: {path}")
        for name in files:
            names.append((current_path / name).relative_to(root).as_posix())
    return sorted(names)


def inspect_handoff(
    root: Path,
    expected_kind: str,
    staging_root: Path,
    controller_uid: int,
    stage_name: str,
    controller_gid: int | None = None,
) -> dict[str, object]:
    if controller_gid is None:
        controller_gid = os.getegid()
    """Validate an Actions artifact as inert bytes before any phase consumes it."""

    root = _require_canonical(root, "downloaded handoff root")
    root_info = root.lstat()
    if not stat.S_ISDIR(root_info.st_mode) or stat.S_ISLNK(root_info.st_mode):
        _fail("downloaded handoff root must be a non-symlink directory")
    if TRUST_ID_RE.fullmatch(expected_kind) is None:
        _fail("expected handoff kind is absent or noncanonical")
    if TRUST_ID_RE.fullmatch(stage_name) is None:
        _fail("handoff stage name is absent or noncanonical")
    staging_root = _require_canonical(staging_root, "controller staging root")
    staging_info = staging_root.lstat()
    if (
        not stat.S_ISDIR(staging_info.st_mode)
        or stat.S_ISLNK(staging_info.st_mode)
        or staging_info.st_uid != controller_uid
        or staging_info.st_gid != controller_gid
        or stat.S_IMODE(staging_info.st_mode) != 0o711
    ):
        _fail("controller staging root must be root-owned exact mode 0711")
    if os.geteuid() != controller_uid:
        _fail("controller handoff inspection requires root")
    manifest_payload = _read_relative_stable(
        root, HANDOFF_MANIFEST, MAX_CONTROLLER_BYTES
    )
    try:
        manifest = json.loads(manifest_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ControllerSealError("handoff inventory is invalid JSON") from exc
    if canonical_json_bytes(manifest) != manifest_payload:
        _fail("handoff inventory is not canonical JSON")
    if not isinstance(manifest, dict) or set(manifest) != {
        "files",
        "kind",
        "schema",
        "schema_version",
    }:
        _fail("handoff inventory fields differ")
    if (
        manifest.get("schema") != "iroha.taira.release_handoff"
        or manifest.get("schema_version") != 1
        or manifest.get("kind") != expected_kind
    ):
        _fail("handoff inventory identity differs")
    rows = manifest.get("files")
    if not isinstance(rows, list) or not rows or len(rows) > MAX_HANDOFF_FILES:
        _fail("handoff inventory has an invalid file count")
    row_paths: list[str] = []
    total_size = 0
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            _fail("handoff inventory row fields differ")
        relative = _validate_relative(row["path"])
        if relative == HANDOFF_MANIFEST:
            _fail("handoff inventory must not recursively describe itself")
        digest = row["sha256"]
        size = row["size"]
        if (
            not isinstance(digest, str)
            or SHA256_RE.fullmatch(digest) is None
            or not isinstance(size, int)
            or isinstance(size, bool)
            or size <= 0
            or size > MAX_HANDOFF_FILE_BYTES
        ):
            _fail(f"handoff metadata is invalid: {relative}")
        payload = _read_relative_stable(root, relative, MAX_HANDOFF_FILE_BYTES)
        if len(payload) != size or hashlib.sha256(payload).hexdigest() != digest:
            _fail(f"downloaded handoff file differs from inventory: {relative}")
        row_paths.append(relative)
        total_size += size
        if total_size > MAX_HANDOFF_TOTAL_BYTES:
            _fail("downloaded handoff exceeds the total size limit")
    if row_paths != sorted(set(row_paths)):
        _fail("handoff inventory paths are duplicated or unsorted")
    actual = _relative_file_names_fd(root)
    if actual != sorted([HANDOFF_MANIFEST, *row_paths]):
        _fail("downloaded handoff contains an unexpected or missing path")
    stage = staging_root / stage_name
    if stage.exists() or stage.is_symlink():
        _fail("controller handoff stage must be fresh")
    stage.mkdir(mode=0o700)
    try:
        for row in rows:
            relative = str(row["path"])
            payload = _read_relative_stable(root, relative, MAX_HANDOFF_FILE_BYTES)
            if (
                len(payload) != row["size"]
                or hashlib.sha256(payload).hexdigest() != row["sha256"]
            ):
                _fail(f"handoff changed while staging: {relative}")
            destination = stage / relative
            destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            descriptor = os.open(
                destination,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC,
                0o400,
            )
            try:
                view = memoryview(payload)
                while view:
                    written = os.write(descriptor, view)
                    if written <= 0:
                        _fail(f"short handoff staging write: {relative}")
                    view = view[written:]
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        manifest_target = stage / HANDOFF_MANIFEST
        descriptor = os.open(
            manifest_target,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC,
            0o400,
        )
        try:
            os.write(descriptor, manifest_payload)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        file_mode = 0o444
        directory_mode = 0o555
        for current, directories, files in os.walk(stage, topdown=False):
            for name in files:
                (Path(current) / name).chmod(file_mode)
            for name in directories:
                (Path(current) / name).chmod(directory_mode)
            Path(current).chmod(directory_mode)
        stage_fd = os.open(
            stage,
            os.O_RDONLY
            | os.O_CLOEXEC
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        staging_fd = os.open(
            staging_root,
            os.O_RDONLY
            | os.O_CLOEXEC
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fsync(stage_fd)
            os.fsync(staging_fd)
        finally:
            os.close(staging_fd)
            os.close(stage_fd)
        _revalidate_staged_roots(
            staging_root,
            {stage},
            expected_owner=controller_uid,
            expected_group=controller_gid,
        )
        return {
            "file_count": len(row_paths),
            "handoff_digest": hashlib.sha256(manifest_payload).hexdigest(),
            "kind": expected_kind,
            "staged_root": str(stage),
            "total_size": total_size,
        }
    except BaseException:
        for current, directories, files in os.walk(stage, topdown=False):
            for name in files:
                (Path(current) / name).chmod(0o600)
            for name in directories:
                (Path(current) / name).chmod(0o700)
            Path(current).chmod(0o700)
        shutil.rmtree(stage, ignore_errors=True)
        raise


def verify(
    root: Path,
    expected_digest: str,
    platform_name: str,
    source_commit: str,
    *,
    fixed_root: Path = CONTROLLER_ROOT,
) -> dict[str, object]:
    """Verify the exact pre-installed closure used by release controllers."""

    root = _require_canonical(root, "installed controller root")
    if root != fixed_root:
        _fail("controller root is not the fixed pre-provisioned path")
    if platform_name not in PLATFORM_FILES:
        _fail("controller platform must be exactly linux or macos")
    if SHA256_RE.fullmatch(expected_digest) is None:
        _fail("expected controller digest must be lowercase SHA-256")
    if COMMIT_RE.fullmatch(source_commit) is None:
        _fail("expected controller source commit must be lowercase 40-hex")
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or root_info.st_uid != 0
        or root_info.st_gid != 0
        or stat.S_IMODE(root_info.st_mode) != 0o555
    ):
        _fail("installed controller root is not root-owned exact mode 0555")

    manifest_path = root / MANIFEST_NAME
    manifest_bytes = _require_root_owned_file(
        manifest_path, "controller manifest", exact_mode=0o444
    )
    try:
        manifest = json.loads(manifest_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ControllerSealError("controller manifest is invalid JSON") from exc
    if canonical_json_bytes(manifest) != manifest_bytes:
        _fail("controller manifest is not canonical JSON")
    if controller_digest(manifest_bytes) != expected_digest:
        _fail("installed controller manifest digest differs")
    if (
        not isinstance(manifest, dict)
        or set(manifest) != {
            "files",
            "platform",
            "schema",
            "schema_version",
            "source_commit",
        }
        or manifest.get("schema") != SCHEMA
        or manifest.get("schema_version") != SCHEMA_VERSION
        or manifest.get("platform") != platform_name
        or manifest.get("source_commit") != source_commit
    ):
        _fail("installed controller manifest identity differs")
    rows = manifest.get("files")
    if not isinstance(rows, list):
        _fail("controller manifest files must be an array")
    expected_files = sorted(PLATFORM_FILES[platform_name])
    row_paths: list[str] = []
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            _fail("controller manifest file row differs")
        relative = _validate_relative(row["path"])
        digest = row["sha256"]
        size = row["size"]
        if (
            not isinstance(digest, str)
            or SHA256_RE.fullmatch(digest) is None
            or not isinstance(size, int)
            or isinstance(size, bool)
            or size <= 0
        ):
            _fail(f"controller manifest metadata is invalid: {relative}")
        payload = _require_root_owned_file(
            root / relative, f"installed controller {relative}", exact_mode=0o444
        )
        if len(payload) != size or hashlib.sha256(payload).hexdigest() != digest:
            _fail(f"installed controller differs from its manifest: {relative}")
        row_paths.append(relative)
    if row_paths != expected_files:
        _fail("controller manifest does not describe the exact platform closure")
    if _relative_file_names(root) != sorted([MANIFEST_NAME, *expected_files]):
        _fail("installed controller tree contains an unexpected path")
    for current, directories, _files in os.walk(root):
        for name in directories:
            info = (Path(current) / name).lstat()
            if (
                not stat.S_ISDIR(info.st_mode)
                or stat.S_ISLNK(info.st_mode)
                or info.st_uid != 0
                or info.st_gid != 0
                or stat.S_IMODE(info.st_mode) != 0o555
            ):
                _fail("installed controller subdirectory identity differs")
    return {
        "controller_digest": expected_digest,
        "controller_manifest": str(manifest_path),
        "controller_root": str(root),
        "platform": platform_name,
        "source_commit": source_commit,
    }


def _attest(
    *,
    expected_launcher_sha256: str,
    expected_controller_digest: str,
    expected_version: str,
    expected_host_id: str,
    expected_installation_id: str,
    expected_uid: str,
    source_commit: str,
    platform_name: str,
    role: str,
    command_path: Path = CONTROLLER_COMMAND,
    controller_root: Path = CONTROLLER_ROOT,
    runner_trust_file: Path = RUNNER_TRUST_FILE,
    required_controller_uid: int = 0,
    required_controller_gid: int = 0,
) -> dict[str, object]:
    if SHA256_RE.fullmatch(expected_launcher_sha256) is None:
        _fail("expected launcher SHA-256 must be exactly 64 lowercase hex")
    if expected_version != CONTROLLER_VERSION:
        _fail("expected controller version differs from this launcher")
    if TRUST_ID_RE.fullmatch(expected_host_id) is None:
        _fail("expected authority host identity is absent or noncanonical")
    if TRUST_ID_RE.fullmatch(expected_installation_id) is None:
        _fail("expected controller installation identity is absent or noncanonical")
    if not expected_uid.isascii() or not expected_uid.isdecimal():
        _fail("expected controller UID is absent or noncanonical")
    expected_uid_number = int(expected_uid)
    if expected_uid != str(expected_uid_number) or expected_uid_number < 0:
        _fail("expected controller UID is not canonical")
    role_contract = ROLE_OPERATIONS.get(role)
    if role_contract is None or role_contract[0] != platform_name:
        _fail("runner role is not valid for the requested platform")
    if expected_uid_number != required_controller_uid:
        _fail("installed release controller must execute as root")
    launcher = _require_root_owned_file(
        command_path, "fixed controller launcher", exact_mode=0o555
    )
    if hashlib.sha256(launcher).hexdigest() != expected_launcher_sha256:
        _fail("fixed controller launcher digest differs")
    trust_payload = _require_root_owned_file(
        runner_trust_file,
        "release runner trust record",
        exact_mode=0o444,
        maximum=MAX_RUNNER_TRUST_BYTES,
    )
    try:
        trust = json.loads(trust_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ControllerSealError("release runner trust record is invalid JSON") from exc
    if canonical_json_bytes(trust) != trust_payload:
        _fail("release runner trust record is not canonical JSON")
    if not isinstance(trust, dict) or set(trust) != {
        "authority_gid",
        "authority_root",
        "authority_uid",
        "handoff_root",
        "host_id",
        "installation_id",
        "platform",
        "role",
        "runtime_gid",
        "runtime_root",
        "runtime_uid",
        "schema",
        "schema_version",
        "staging_root",
        "staging_gid",
        "staging_uid",
        "trusted_executables",
        "trusted_inputs",
        "trusted_values",
        "uid",
    }:
        _fail("release runner trust record fields differ")
    if (
        trust.get("schema") != "iroha.taira.release_runner_trust"
        or trust.get("schema_version") != RUNNER_TRUST_SCHEMA_VERSION
        or trust.get("host_id") != expected_host_id
        or trust.get("installation_id") != expected_installation_id
        or trust.get("platform") != platform_name
        or trust.get("role") != role
        or trust.get("uid") != expected_uid_number
        or os.geteuid() != expected_uid_number
    ):
        _fail("release runner host, installation, role, platform, or UID differs")
    identities: dict[str, dict[str, object]] = {}
    for name in ("staging", "runtime", "authority"):
        raw_root = trust.get(f"{name}_root")
        if not isinstance(raw_root, str):
            _fail(f"release runner {name} root is absent")
        identities[name] = {
            "gid": _strict_positive_identity(
                trust.get(f"{name}_gid"), f"release runner {name} GID"
            ),
            "root": raw_root,
            "uid": _strict_positive_identity(
                trust.get(f"{name}_uid"), f"release runner {name} UID"
            ),
        }
    identity_uids = [int(identities[name]["uid"]) for name in identities]
    identity_pairs = [
        (int(identities[name]["uid"]), int(identities[name]["gid"]))
        for name in identities
    ]
    if len(set(identity_uids)) != 3 or len(set(identity_pairs)) != 3:
        _fail("staging, runtime, and authority identities must be pairwise distinct")
    sudo_identity: dict[str, int] = {}
    for variable in ("SUDO_UID", "SUDO_GID"):
        raw_value = os.environ.get(variable)
        if (
            raw_value is None
            or not raw_value.isascii()
            or not raw_value.isdecimal()
        ):
            _fail("release controller requires canonical sudo caller identity")
        number = int(raw_value)
        if raw_value != str(number) or number <= 0:
            _fail("release controller requires canonical sudo caller identity")
        sudo_identity[variable] = number
    if (
        sudo_identity["SUDO_UID"] != identities["staging"]["uid"]
        or sudo_identity["SUDO_GID"] != identities["staging"]["gid"]
    ):
        _fail("sudo caller differs from the attested release runner identity")
    identity_roots: dict[str, Path] = {}
    identity_ids: dict[str, tuple[int, int]] = {}
    for name, identity in identities.items():
        uid = int(identity["uid"])
        gid = int(identity["gid"])
        root_path = _validate_identity_root(
            Path(str(identity["root"])),
            name,
            uid,
            gid,
            controller_uid=required_controller_uid,
            controller_gid=required_controller_gid,
        )
        identity_roots[name] = root_path
        identity_ids[name] = (uid, gid)
    roots = list(identity_roots.values())
    if len(set(roots)) != 3 or any(
        left != right and (_path_within(left, right) or _path_within(right, left))
        for left in roots
        for right in roots
    ):
        _fail("release runner identity roots must be distinct and non-nested")
    raw_handoff_root = trust.get("handoff_root")
    if not isinstance(raw_handoff_root, str):
        _fail("immutable handoff root is absent")
    handoff_root = _validate_handoff_root(
        Path(raw_handoff_root),
        controller_uid=required_controller_uid,
        controller_gid=required_controller_gid,
    )
    if any(
        _path_within(handoff_root, root) or _path_within(root, handoff_root)
        for root in roots
    ):
        _fail("immutable handoff root must be separate from every identity root")
    closure = verify(
        controller_root,
        expected_controller_digest,
        platform_name,
        source_commit,
        fixed_root=controller_root,
    )

    raw_trusted_values = trust.get("trusted_values")
    if not isinstance(raw_trusted_values, list):
        _fail("trusted_values must be one canonical array")
    trusted_values: list[dict[str, str]] = []
    value_keys: list[tuple[str, str, str]] = []
    for row in raw_trusted_values:
        if not isinstance(row, dict) or set(row) != {"flag", "operation", "value"}:
            _fail("trusted literal record fields differ")
        operation = row.get("operation")
        flag = row.get("flag")
        value = row.get("value")
        if (
            not isinstance(operation, str)
            or operation not in role_contract[1]
            or not isinstance(flag, str)
            or flag not in TRUSTED_LITERAL_FLAGS.get(operation, set())
            or not isinstance(value, str)
        ):
            _fail("trusted literal operation or flag is not valid for this role")
        _validate_trusted_literal(flag, value)
        record = {"flag": flag, "operation": operation, "value": value}
        trusted_values.append(record)
        value_keys.append((operation, flag, value))
    if value_keys != sorted(set(value_keys)):
        _fail("trusted literal records are duplicated or unsorted")
    expected_value_pairs = sorted(
        (operation, flag)
        for operation in role_contract[1]
        for flag in TRUSTED_LITERAL_FLAGS.get(operation, set())
    )
    if sorted((row[0], row[1]) for row in value_keys) != expected_value_pairs:
        _fail("trusted literal records do not exactly cover this runner role")
    _require_distinct_release_and_qualification_signers(trusted_values)

    raw_trusted_inputs = trust.get("trusted_inputs")
    if not isinstance(raw_trusted_inputs, list):
        _fail("trusted_inputs must be one canonical array")
    trusted_inputs: list[dict[str, str]] = []
    input_keys: list[tuple[str, str, str]] = []
    for row in raw_trusted_inputs:
        if not isinstance(row, dict) or set(row) != {"flag", "operation", "path"}:
            _fail("trusted input record fields differ")
        operation = row.get("operation")
        flag = row.get("flag")
        raw_path = row.get("path")
        if (
            not isinstance(operation, str)
            or operation not in role_contract[1]
            or not isinstance(flag, str)
            or flag not in _trusted_input_flags(operation)
            or not isinstance(raw_path, str)
        ):
            _fail("trusted input operation or flag is not valid for this role")
        path = _require_canonical(Path(raw_path), "trusted input")
        _validate_trusted_input_path(path, identity_roots, identity_ids)
        if operation == "check-public" and flag == "--write-config":
            _validate_privacy_rollout_input(
                path,
                identity_root=identity_roots["staging"],
                identity_uid=identity_ids["staging"][0],
                identity_gid=identity_ids["staging"][1],
                label="dedicated post-cutover canary client config",
                maximum=MAX_RUNNER_TRUST_BYTES,
            )
        elif operation == "verify-privacy-rollout" and flag == "--result":
            _validate_privacy_rollout_input(
                path,
                identity_root=identity_roots["authority"],
                identity_uid=identity_ids["authority"][0],
                identity_gid=identity_ids["authority"][1],
                label="controller-owned privacy rollout observation",
                maximum=MAX_CONTROLLER_BYTES,
            )
        if operation == "publish-rollout":
            fingerprint = next(
                (
                    record["value"]
                    for record in trusted_values
                    if record["operation"] == operation
                    and record["flag"] == "--trusted-signing-fingerprint"
                ),
                "",
            )
            _validate_publisher_trusted_input(
                path,
                flag,
                identity_ids["authority"][0],
                identity_ids["authority"][1],
                identity_roots["authority"],
                fingerprint,
            )
        record = {"flag": flag, "operation": operation, "path": str(path)}
        trusted_inputs.append(record)
        input_keys.append((operation, flag, str(path)))
    if input_keys != sorted(set(input_keys)):
        _fail("trusted input records are duplicated or unsorted")
    expected_input_pairs = sorted(
        (operation, flag)
        for operation in role_contract[1]
        for flag in _trusted_input_flags(operation)
    )
    if sorted((row[0], row[1]) for row in input_keys) != expected_input_pairs:
        _fail("trusted input records do not exactly cover this runner role")

    raw_trusted_executables = trust.get("trusted_executables")
    if not isinstance(raw_trusted_executables, list):
        _fail("trusted_executables must be one canonical array")
    trusted_executables: list[dict[str, object]] = []
    executable_keys: list[tuple[str, str, str, str, str, str]] = []
    for row in raw_trusted_executables:
        if not isinstance(row, dict) or set(row) != {
            "digest_flag",
            "flag",
            "operation",
            "path",
            "run_as",
            "sha256",
        }:
            _fail("trusted executable record fields differ")
        operation = row.get("operation")
        flag = row.get("flag")
        raw_path = row.get("path")
        digest = row.get("sha256")
        digest_flag = row.get("digest_flag")
        run_as = row.get("run_as")
        if (
            not isinstance(operation, str)
            or operation not in role_contract[1]
            or not isinstance(flag, str)
            or flag not in _trusted_executable_flags(operation)
            or not isinstance(raw_path, str)
            or not isinstance(digest, str)
            or digest_flag != _expected_executable_digest_flag(operation, flag)
            or run_as != _expected_executable_identity(role, operation, flag)
        ):
            _fail("trusted executable operation, flag, digest flag, or run_as differs")
        path = _require_canonical(Path(raw_path), "trusted executable")
        if flag == "--supervisor" and path != (
            controller_root / "scripts/taira_peer_supervisor.py"
        ):
            _fail("controller supervisor must be the exact installed closure path")
        _validate_trusted_executable_path(
            path,
            digest,
            exact_mode=0o555 if operation == "publish-rollout" else None,
        )
        record: dict[str, object] = {
            "digest_flag": digest_flag,
            "flag": flag,
            "operation": operation,
            "path": str(path),
            "run_as": run_as,
            "sha256": digest,
        }
        trusted_executables.append(record)
        executable_keys.append(
            (
                operation,
                flag,
                str(path),
                digest,
                "" if digest_flag is None else str(digest_flag),
                str(run_as),
            )
        )
    if executable_keys != sorted(set(executable_keys)):
        _fail("trusted executable records are duplicated or unsorted")
    expected_executable_pairs = sorted(
        (operation, flag)
        for operation in role_contract[1]
        for flag in _trusted_executable_flags(operation)
    )
    if sorted((row[0], row[1]) for row in executable_keys) != expected_executable_pairs:
        _fail("trusted executable records do not exactly cover this runner role")
    return {
        **closure,
        "controller_version": CONTROLLER_VERSION,
        "controller_gid": required_controller_gid,
        "launcher_sha256": expected_launcher_sha256,
        "role": role,
        "authority_gid": identity_ids["authority"][1],
        "authority_root": str(identity_roots["authority"]),
        "authority_uid": identity_ids["authority"][0],
        "handoff_root": str(handoff_root),
        "runtime_gid": identity_ids["runtime"][1],
        "runtime_root": str(identity_roots["runtime"]),
        "runtime_uid": identity_ids["runtime"][0],
        "staging_gid": identity_ids["staging"][1],
        "staging_root": str(identity_roots["staging"]),
        "staging_uid": identity_ids["staging"][0],
        "trusted_executables": trusted_executables,
        "trusted_inputs": trusted_inputs,
        "trusted_values": trusted_values,
        "host_id": expected_host_id,
        "installation_id": expected_installation_id,
        "invoking_gid": sudo_identity["SUDO_GID"],
        "invoking_uid": sudo_identity["SUDO_UID"],
        "uid": expected_uid_number,
    }


def _child_environment(
    external_tool_identity: tuple[int, int] | None = None,
) -> dict[str, str]:
    # Never forward the workflow environment wholesale.  In particular this
    # prevents release/registry credentials from leaking to external signer
    # descendants that do not own them.
    environment = {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "TMPDIR": "/var/tmp",
    }
    if sys.platform == "win32":  # pragma: no cover - release runners are Unix
        for name in ("SYSTEMROOT", "WINDIR"):
            if name in os.environ:
                environment[name] = os.environ[name]
    if external_tool_identity is not None:
        uid, gid = external_tool_identity
        if uid <= 0 or gid <= 0:
            _fail("external tool execution identity is invalid")
        environment["IROHA_TAIRA_EXTERNAL_TOOL_UID"] = str(uid)
        environment["IROHA_TAIRA_EXTERNAL_TOOL_GID"] = str(gid)
    return environment


def _existing_ancestor(path: Path) -> Path:
    current = path
    while not current.exists():
        if current == current.parent:
            _fail(f"operation path has no existing ancestor: {path}")
        current = current.parent
    return current.resolve(strict=True)


def _identity_contract(
    attestation: dict[str, object], name: str
) -> tuple[int, int, Path]:
    return (
        int(attestation[f"{name}_uid"]),
        int(attestation[f"{name}_gid"]),
        Path(str(attestation[f"{name}_root"])),
    )


def _revalidate_bound_roots(attestation: dict[str, object]) -> None:
    controller_uid = int(attestation["uid"])
    controller_gid = int(attestation.get("controller_gid", 0))
    for name in ("staging", "runtime", "authority"):
        uid, gid, root = _identity_contract(attestation, name)
        _validate_identity_root(
            root,
            name,
            uid,
            gid,
            controller_uid=controller_uid,
            controller_gid=controller_gid,
        )
    _validate_handoff_root(
        Path(str(attestation["handoff_root"])),
        controller_uid=controller_uid,
        controller_gid=controller_gid,
    )


def _require_identity_path(
    path: Path,
    root: Path,
    uid: int,
    gid: int,
    *,
    label: str,
    allow_root: bool = False,
) -> Path:
    canonical = _require_canonical(path, label)
    if (canonical == root and not allow_root) or not _path_within(canonical, root):
        _fail(f"{label} is outside its exact identity root")
    rows = _ancestry_snapshot(canonical)
    root_index = next(
        (index for index, (component, _value) in enumerate(rows) if component == root),
        None,
    )
    if root_index is None:
        _fail(f"{label} does not descend from its identity root")
    for component, expected in rows[root_index + 1 :]:
        mode = expected[2]
        if (
            stat.S_ISLNK(mode)
            or expected[4] not in {0, uid}
            or expected[5] not in {0, gid}
            or mode & 0o022
            or not (stat.S_ISDIR(mode) or stat.S_ISREG(mode))
            or (stat.S_ISREG(mode) and expected[3] != 1)
        ):
            _fail(f"{label} is not protected by its exact identity: {component}")
    _revalidate_ancestry(rows)
    return canonical


def _trusted_input_for(
    attestation: dict[str, object], operation: str, flag: str, path: Path
) -> bool:
    return any(
        isinstance(row, dict)
        and row.get("operation") == operation
        and row.get("flag") == flag
        and row.get("path") == str(path)
        for row in attestation["trusted_inputs"]  # type: ignore[union-attr]
    )


def _trusted_input_path_for(
    attestation: dict[str, object], operation: str, flag: str
) -> Path:
    matches = [
        row
        for row in attestation["trusted_inputs"]  # type: ignore[union-attr]
        if isinstance(row, dict)
        and row.get("operation") == operation
        and row.get("flag") == flag
    ]
    if len(matches) != 1 or not isinstance(matches[0].get("path"), str):
        _fail("operation input lacks one exact trusted input record")
    return Path(str(matches[0]["path"]))


def _trusted_value_for(
    attestation: dict[str, object], operation: str, flag: str
) -> str:
    matches = [
        row
        for row in attestation["trusted_values"]  # type: ignore[union-attr]
        if isinstance(row, dict)
        and row.get("operation") == operation
        and row.get("flag") == flag
    ]
    if len(matches) != 1 or not isinstance(matches[0].get("value"), str):
        _fail("operation literal lacks one exact trusted literal record")
    value = str(matches[0]["value"])
    _validate_trusted_literal(flag, value)
    return value


def _trusted_executable_for(
    attestation: dict[str, object], operation: str, flag: str, path: Path
) -> dict[str, object]:
    matches = [
        row
        for row in attestation["trusted_executables"]  # type: ignore[union-attr]
        if isinstance(row, dict)
        and row.get("operation") == operation
        and row.get("flag") == flag
        and row.get("path") == str(path)
    ]
    if len(matches) != 1:
        _fail("operation executable lacks one exact trusted executable record")
    return matches[0]


def _operation_option_values(
    operation: str, operation_args: Sequence[str]
) -> tuple[str | None, dict[str, list[str]]]:
    values = list(operation_args)
    subcommand: str | None = None
    if operation in POSITIONAL_COMMANDS:
        if not values:
            _fail("controller operation subcommand is absent")
        subcommand = values.pop(0)
    result: dict[str, list[str]] = {}
    index = 0
    while index < len(values):
        flag = values[index]
        index += 1
        if flag in BOOLEAN_FLAGS:
            result.setdefault(flag, [])
            continue
        if index >= len(values):
            _fail(f"controller operation option lacks one value: {flag}")
        result.setdefault(flag, []).append(values[index])
        index += 1
    return subcommand, result


def _validate_operation_args(
    operation: str,
    operation_args: Sequence[str],
    attestation: dict[str, object],
) -> tuple[set[Path], set[Path]]:
    allowed = OPERATION_FLAGS[operation]
    values = list(operation_args)
    positional = POSITIONAL_COMMANDS.get(operation)
    subcommand: str | None = None
    if positional is not None:
        if not values:
            _fail("controller operation subcommand is absent or not allow-listed")
        subcommand = values.pop(0)
        if subcommand not in positional:
            _fail("controller operation subcommand is absent or not allow-listed")
    seen: set[str] = set()
    counts: dict[str, int] = {}
    option_values: dict[str, list[str]] = {}
    staged_roots: set[Path] = set()
    output_paths: set[Path] = set()
    handoff_root = Path(str(attestation["handoff_root"]))
    role = str(attestation["role"])
    operation_identity = _expected_operation_identity(role, operation)
    identity_contracts = {
        name: _identity_contract(attestation, name)
        for name in ("staging", "runtime", "authority")
    }
    index = 0
    while index < len(values):
        flag = values[index]
        if (
            not flag.startswith("--")
            or "=" in flag
            or "\n" in flag
            or flag not in allowed
            or (flag in seen and flag not in REPEATED_FLAGS)
        ):
            _fail(f"controller operation option is not allow-listed: {flag!r}")
        seen.add(flag)
        counts[flag] = counts.get(flag, 0) + 1
        index += 1
        if flag in BOOLEAN_FLAGS:
            option_values.setdefault(flag, [])
            continue
        if index >= len(values):
            _fail(f"controller operation option lacks one value: {flag}")
        value = values[index]
        index += 1
        if (
            (not value and flag != "--suffix")
            or len(value.encode("utf-8")) > MAX_OPERATION_ARG_BYTES
            or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
            or value.startswith("--")
        ):
            _fail(f"controller operation option value is unsafe: {flag}")
        option_values.setdefault(flag, []).append(value)
        if flag not in INPUT_PATH_FLAGS | OUTPUT_PATH_FLAGS:
            continue
        path = Path(value)
        if not path.is_absolute() or Path(os.path.abspath(path)) != path:
            _fail(f"controller operation path is not absolute: {flag}")
        if flag in OUTPUT_PATH_FLAGS:
            immutable_prefix = IMMUTABLE_HANDOFF_OUTPUT_PREFIXES.get(operation)
            if immutable_prefix is not None:
                if (
                    flag != "--output"
                    or path.parent != handoff_root
                    or re.fullmatch(
                        rf"{re.escape(immutable_prefix)}[1-9][0-9]{{0,19}}-[1-9][0-9]{{0,9}}",
                        path.name,
                    )
                    is None
                    or path.exists()
                    or path.is_symlink()
                ):
                    _fail(
                        "immutable authority handoff output must be one exact child "
                        "of the attested root-owned staging directory"
                    )
                output_paths.add(path)
                continue
            if operation_identity == "root":
                _fail("root orchestration cannot receive an arbitrary output path")
            output_uid, output_gid, output_root = identity_contracts[
                operation_identity
            ]
            parent = _require_identity_path(
                path.parent,
                output_root,
                output_uid,
                output_gid,
                label=f"controller output parent for {flag}",
                allow_root=True,
            )
            info = parent.lstat()
            if not stat.S_ISDIR(info.st_mode):
                _fail(f"controller output parent is not a directory: {flag}")
            if path.exists() or path.is_symlink():
                _fail(f"controller output must be fresh: {flag}")
            output_paths.add(path)
            continue
        canonical = path.resolve(strict=True)
        if canonical != path:
            _fail(f"controller input path is not canonical: {flag}")
        if operation == "prepare-reset" and flag == "--kagemusha-release-root":
            _validate_root_owned_release_root(canonical)
            continue
        if flag in TRUSTED_EXECUTABLE_FLAGS:
            continue
        try:
            relative = canonical.relative_to(handoff_root)
        except ValueError:
            relative = None
        if relative is not None and relative.parts:
            staged_roots.add(handoff_root / relative.parts[0])
            continue
        if flag in {"--validator-binary", "--binary", "--source-identity"}:
            _fail(f"{flag} must come from one root-frozen handoff")
        controller_root = Path(str(attestation["controller_root"]))
        if canonical == controller_root or controller_root in canonical.parents:
            if flag == "--controller-manifest" and canonical != (
                controller_root / MANIFEST_NAME
            ):
                _fail("controller manifest path is not the exact installed manifest")
            continue
        if operation == "publish-rollout" and flag in ROLLOUT_OBSERVATION_INPUT_FLAGS:
            uid, gid, root = identity_contracts["authority"]
            _validate_privacy_rollout_input(
                canonical,
                identity_root=root,
                identity_uid=uid,
                identity_gid=gid,
                label=f"authenticated rollout observation input {flag}",
                maximum=MAX_CONTROLLER_BYTES,
            )
            continue
        if flag in SENSITIVE_TRUSTED_INPUT_FLAGS:
            if not _trusted_input_for(attestation, operation, flag, canonical):
                _fail("operation input lacks its exact trusted input record")
            if operation == "check-public" and flag == "--write-config":
                uid, gid, root = identity_contracts["staging"]
                _validate_privacy_rollout_input(
                    canonical,
                    identity_root=root,
                    identity_uid=uid,
                    identity_gid=gid,
                    label="dedicated post-cutover canary client config",
                    maximum=MAX_RUNNER_TRUST_BYTES,
                )
            elif operation == "verify-privacy-rollout" and flag == "--result":
                uid, gid, root = identity_contracts["authority"]
                _validate_privacy_rollout_input(
                    canonical,
                    identity_root=root,
                    identity_uid=uid,
                    identity_gid=gid,
                    label="controller-owned privacy rollout observation",
                    maximum=MAX_CONTROLLER_BYTES,
                )
            continue
        if flag == "--forbidden-root":
            # This path is only a negative assertion supplied to the installed
            # snapshot helper; authorizing it grants no read or execution.
            continue
        allowed_identity_names = {operation_identity}
        if operation in {"capture-four-peer", "deploy-reset"}:
            allowed_identity_names.add("runtime")
        if operation == "snapshot-public-privacy":
            allowed_identity_names.add("staging")
        matched_identity = False
        for name in allowed_identity_names - {"root"}:
            uid, gid, root = identity_contracts[name]
            if _path_within(canonical, root):
                _require_identity_path(
                    canonical,
                    root,
                    uid,
                    gid,
                    label=f"controller input for {flag}",
                )
                matched_identity = True
                break
        if not matched_identity and not _trusted_input_for(
            attestation, operation, flag, canonical
        ):
            _fail(f"external controller input lacks an exact trust record: {flag}")
    if operation == "prepare-reset":
        kagemusha_flags_seen = seen & KAGEMUSHA_PREPARE_RESET_FLAGS
        if (
            kagemusha_flags_seen
            and kagemusha_flags_seen != KAGEMUSHA_PREPARE_RESET_FLAGS
        ):
            _fail(
                "Kagemusha release root and activation authority must be supplied together"
            )
    required = REQUIRED_FLAGS[(operation, subcommand)]
    missing = sorted(required - seen)
    if missing:
        _fail(f"controller operation mandatory options are absent: {missing}")
    _require_attested_source_commit(operation, option_values, attestation)
    unexpected_for_subcommand: set[str] = set()
    if operation == "admit" and subcommand == "init-replay-ledger":
        unexpected_for_subcommand = seen - {"--output"}
    elif operation == "admit" and subcommand == "verify":
        unexpected_for_subcommand = seen & {"--output"}
    if unexpected_for_subcommand:
        _fail("controller operation options do not belong to its subcommand")
    if operation == "check-public" and counts.get("--validator-root") != 4:
        _fail("public rollout check requires exactly four validator roots")
    for flag in sorted(seen & TRUSTED_LITERAL_FLAGS.get(operation, set())):
        values_for_flag = option_values.get(flag, [])
        trusted_value = _trusted_value_for(attestation, operation, flag)
        if values_for_flag != [trusted_value]:
            _fail("controller operation literal differs from sealed trust")
    if operation == "publish-rollout":
        candidate_values = option_values.get("--candidate-root", [])
        if len(candidate_values) != 1:
            _fail("publication candidate root is absent")
        candidate = Path(candidate_values[0])
        if (
            candidate.parent != handoff_root
            or re.fullmatch(r"publish-candidate-[1-9][0-9]{0,19}-[1-9][0-9]{0,9}", candidate.name)
            is None
        ):
            _fail("publication candidate is not one exact frozen handoff root")
        for flag in (
            "--expected-source-commit",
            "--expected-dpn-validator-release-commit",
        ):
            if option_values.get(flag, [""])[0] == "" or COMMIT_RE.fullmatch(
                option_values[flag][0]
            ) is None:
                _fail("publication source commit field is noncanonical")
        for flag in (
            "--expected-cargo-lock-sha256",
            "--expected-workspace-source-manifest-sha256",
            "--expected-qualification-receipt-id",
        ):
            if option_values.get(flag, [""])[0] == "" or SHA256_RE.fullmatch(
                option_values[flag][0]
            ) is None:
                _fail("publication digest field is noncanonical")
    for flag in sorted(seen & TRUSTED_EXECUTABLE_FLAGS):
        values_for_flag = option_values.get(flag, [])
        if len(values_for_flag) != 1:
            _fail("trusted executable flag lacks one exact path")
        executable = Path(values_for_flag[0])
        record = _trusted_executable_for(
            attestation, operation, flag, executable
        )
        expected_run_as = _expected_executable_identity(role, operation, flag)
        if record.get("run_as") != expected_run_as:
            _fail("trusted executable run_as identity differs")
        digest_flag = record.get("digest_flag")
        if digest_flag is not None:
            digest_values = option_values.get(str(digest_flag), [])
            if digest_values != [record.get("sha256")]:
                _fail("trusted executable digest argument differs from trust")
        _validate_trusted_executable_path(
            executable, str(record.get("sha256", ""))
        )
    _revalidate_staged_roots(
        handoff_root,
        staged_roots,
        expected_owner=int(attestation["uid"]),
        expected_group=int(attestation.get("controller_gid", 0)),
    )
    return staged_roots, output_paths


def _revalidate_staged_roots(
    staging_root: Path,
    staged_roots: set[Path],
    *,
    expected_owner: int,
    expected_group: int,
) -> None:
    staging_info = staging_root.lstat()
    if (
        not stat.S_ISDIR(staging_info.st_mode)
        or stat.S_ISLNK(staging_info.st_mode)
        or staging_info.st_uid != expected_owner
        or staging_info.st_gid != expected_group
        or stat.S_IMODE(staging_info.st_mode) != 0o711
    ):
        _fail("immutable handoff staging parent was replaced")
    for stage in staged_roots:
        if stage.parent != staging_root:
            _fail("immutable staged handoff escaped its attested parent")
        info = stage.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or info.st_uid != expected_owner
            or info.st_gid != expected_group
            or stat.S_IMODE(info.st_mode) != 0o555
        ):
            _fail("immutable staged handoff root was replaced after inspection")
        # Recheck all inventory-bound bytes immediately before dispatch.  The
        # staged tree is immutable to the Actions runner on privileged roles.
        manifest_payload = _read_relative_stable(
            stage,
            HANDOFF_MANIFEST,
            MAX_CONTROLLER_BYTES,
            expected_uid=expected_owner,
            expected_gid=expected_group,
            expected_mode=0o444,
        )
        try:
            manifest = json.loads(manifest_payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ControllerSealError("staged handoff manifest is invalid") from exc
        if (
            canonical_json_bytes(manifest) != manifest_payload
            or not isinstance(manifest, dict)
            or set(manifest) != {"files", "kind", "schema", "schema_version"}
            or manifest.get("schema") != "iroha.taira.release_handoff"
            or manifest.get("schema_version") != 1
            or TRUST_ID_RE.fullmatch(str(manifest.get("kind", ""))) is None
        ):
            _fail("staged handoff manifest differs")
        rows = manifest["files"]
        if not isinstance(rows, list) or not rows or len(rows) > MAX_HANDOFF_FILES:
            _fail("staged handoff manifest file count differs")
        expected_paths: list[str] = []
        for row in rows:
            if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
                _fail("staged handoff inventory row differs")
            relative_path = _validate_relative(row["path"])
            digest = row["sha256"]
            size = row["size"]
            if (
                not isinstance(digest, str)
                or SHA256_RE.fullmatch(digest) is None
                or not isinstance(size, int)
                or isinstance(size, bool)
                or size <= 0
                or size > MAX_HANDOFF_FILE_BYTES
            ):
                _fail("staged handoff inventory metadata differs")
            payload = _read_relative_stable(
                stage,
                relative_path,
                MAX_HANDOFF_FILE_BYTES,
                expected_uid=expected_owner,
                expected_gid=expected_group,
                expected_mode=0o444,
            )
            if (
                len(payload) != size
                or hashlib.sha256(payload).hexdigest() != digest
            ):
                _fail("staged handoff changed after inspection")
            expected_paths.append(relative_path)
        if expected_paths != sorted(set(expected_paths)):
            _fail("staged handoff inventory ordering differs")
        if _relative_file_names_fd(
            stage,
            expected_uid=expected_owner,
            expected_gid=expected_group,
            expected_file_mode=0o444,
            expected_directory_mode=0o555,
        ) != sorted([HANDOFF_MANIFEST, *expected_paths]):
            _fail("staged handoff inventory changed after inspection")


def _drop_to_attested_user(uid: int, gid: int) -> None:
    if os.geteuid() != 0 or uid <= 0 or gid <= 0:
        _fail("authority privilege drop identity is invalid")
    os.setgroups([])
    os.setgid(gid)
    os.setuid(uid)
    os.umask(0o077)
    if os.geteuid() != uid or os.getegid() != gid or os.getgroups():
        _fail("authority privilege drop did not reach the exact attested identity")


def _privilege_drop_preexec(uid: int, gid: int) -> Callable[[], None]:
    """Build the sole child-side privilege transition; the parent stays root."""

    if uid <= 0 or gid <= 0:
        _fail("child privilege identity is invalid")

    def drop() -> None:
        _drop_to_attested_user(uid, gid)

    return drop


def _validate_operation_outputs(
    paths: set[Path], expected_uid: int, expected_gid: int | None = None
) -> None:
    directory_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    file_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )

    def validate_identity(info: os.stat_result, label: str) -> None:
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != expected_uid
            or (expected_gid is not None and info.st_gid != expected_gid)
            or info.st_mode & 0o022
        ):
            _fail("controller operation output ownership or mode differs")

    def visit(parent_fd: int, leaf_name: str, label: str) -> tuple[int, ...]:
        info = os.stat(leaf_name, dir_fd=parent_fd, follow_symlinks=False)
        validate_identity(info, label)
        if stat.S_ISREG(info.st_mode):
            if info.st_nlink != 1 or info.st_size <= 0:
                _fail("controller operation output is not one nonempty inode")
            descriptor = os.open(leaf_name, file_flags, dir_fd=parent_fd)
            try:
                if _identity(os.fstat(descriptor)) != _identity(info):
                    _fail("controller operation output changed while opening")
                total = 0
                while total < info.st_size:
                    chunk = os.read(
                        descriptor,
                        min(1024 * 1024, info.st_size - total),
                    )
                    if not chunk:
                        _fail("controller operation output was truncated")
                    total += len(chunk)
                if os.read(descriptor, 1):
                    _fail("controller operation output grew while inspected")
                if _identity(os.fstat(descriptor)) != _identity(info):
                    _fail("controller operation output changed while inspected")
            finally:
                os.close(descriptor)
            if _identity(
                os.stat(leaf_name, dir_fd=parent_fd, follow_symlinks=False)
            ) != _identity(info):
                _fail("controller operation output path changed while inspected")
            return _identity(info)
        if not stat.S_ISDIR(info.st_mode):
            _fail("controller operation output is a special file")
        directory_fd = os.open(leaf_name, directory_flags, dir_fd=parent_fd)
        try:
            if _identity(os.fstat(directory_fd)) != _identity(info):
                _fail("controller operation output directory changed while opening")
            for child_name in sorted(os.listdir(directory_fd)):
                if not child_name or child_name in {".", ".."} or "/" in child_name:
                    _fail("controller operation output name is noncanonical")
                visit(directory_fd, child_name, f"{label}/{child_name}")
            if _identity(os.fstat(directory_fd)) != _identity(info):
                _fail("controller operation output directory changed while inspected")
        finally:
            os.close(directory_fd)
        if _identity(
            os.stat(leaf_name, dir_fd=parent_fd, follow_symlinks=False)
        ) != _identity(info):
            _fail("controller operation output directory path changed while inspected")
        return _identity(info)

    def open_parent(path: Path) -> tuple[int, os.stat_result]:
        current_fd = os.open("/", directory_flags)
        current_identity = os.fstat(current_fd)
        try:
            for component in path.parent.parts[1:]:
                before = os.stat(
                    component,
                    dir_fd=current_fd,
                    follow_symlinks=False,
                )
                next_fd = os.open(component, directory_flags, dir_fd=current_fd)
                opened = os.fstat(next_fd)
                if (
                    not stat.S_ISDIR(opened.st_mode)
                    or _identity(opened) != _identity(before)
                ):
                    os.close(next_fd)
                    _fail("controller operation output parent changed while opening")
                os.close(current_fd)
                current_fd = next_fd
                current_identity = opened
            return current_fd, current_identity
        except BaseException:
            os.close(current_fd)
            raise

    for path in paths:
        if (
            not path.is_absolute()
            or Path(os.path.abspath(path)) != path
            or not path.exists()
            or path.is_symlink()
            or path.resolve(strict=True) != path
        ):
            _fail("controller operation did not create its canonical output")
        parent_fd, parent_identity = open_parent(path)
        try:
            output_identity = visit(parent_fd, path.name, str(path))
            if _identity(os.fstat(parent_fd)) != _identity(parent_identity):
                _fail("controller operation output parent changed while inspected")
        finally:
            os.close(parent_fd)
        reopened_parent_fd, reopened_parent_identity = open_parent(path)
        try:
            if (
                _identity(reopened_parent_identity) != _identity(parent_identity)
                or _identity(
                    os.stat(
                        path.name,
                        dir_fd=reopened_parent_fd,
                        follow_symlinks=False,
                    )
                )
                != output_identity
            ):
                _fail("controller operation output path was replaced after inspection")
        finally:
            os.close(reopened_parent_fd)


def _validate_successful_operation_outputs(
    operation: str,
    output_paths: set[Path],
    attestation: dict[str, object],
    operation_identity: str,
) -> None:
    """Apply the output identity contract selected by the sealed operation."""

    if operation in IMMUTABLE_HANDOFF_OUTPUT_PREFIXES:
        _validate_operation_outputs(
            output_paths,
            int(attestation["uid"]),
            int(attestation.get("controller_gid", 0)),
        )
        return
    if operation_identity == "root":
        return
    output_uid, output_gid, _output_root = _identity_contract(
        attestation, operation_identity
    )
    _validate_operation_outputs(output_paths, output_uid, output_gid)


def _dispatch_command(
    command: Sequence[str],
    run_as: tuple[int, int] | None,
    external_tool_identity: tuple[int, int] | None = None,
) -> int:
    preexec_fn = None
    if run_as is not None:
        preexec_fn = _privilege_drop_preexec(*run_as)
    result = subprocess.run(
        list(command),
        check=False,
        cwd=CONTROLLER_ROOT,
        env=_child_environment(external_tool_identity),
        close_fds=True,
        pass_fds=(),
        stdin=subprocess.DEVNULL,
        restore_signals=True,
        preexec_fn=preexec_fn,
    )
    return result.returncode


def _dispatch_installed_python(
    relative: str,
    operation_args: Sequence[str],
    run_as: tuple[int, int] | None,
    external_tool_identity: tuple[int, int] | None = None,
) -> int:
    for value in operation_args:
        if (
            not isinstance(value, str)
            or not value
            or len(value.encode("utf-8")) > MAX_OPERATION_ARG_BYTES
            or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
        ):
            _fail("controller child argv contains an unsafe value")
    return _dispatch_command(
        [
            "/usr/bin/python3",
            "-I",
            "-S",
            "-c",
            PYTHON_ENV_SCRUBBER,
            str(CONTROLLER_ROOT / relative),
            *operation_args,
        ],
        run_as,
        external_tool_identity,
    )


def _dispatch(
    operation: str,
    operation_args: Sequence[str],
    run_as: tuple[int, int] | None = None,
    external_tool_identity: tuple[int, int] | None = None,
) -> int:
    if operation in {"verify-privacy-rollout", "publish-rollout"}:
        _require_authenticated_rollout_observation_authority()
    if operation == "verify-privacy-rollout":
        return _dispatch_installed_python(
            PYTHON_OPERATIONS[operation],
            [
                "verify-result",
                "--plan",
                str(
                    CONTROLLER_ROOT
                    / "configs/soranexus/taira/privacy_rollout_plan_v1.json"
                ),
                *operation_args,
            ],
            run_as,
            external_tool_identity,
        )
    if operation in PYTHON_OPERATIONS:
        return _dispatch_installed_python(
            PYTHON_OPERATIONS[operation],
            operation_args,
            run_as,
            external_tool_identity,
        )
    elif operation in BASH_OPERATIONS:
        return _dispatch_command(
            [
                "/bin/bash",
                str(CONTROLLER_ROOT / BASH_OPERATIONS[operation]),
                *operation_args,
            ],
            run_as,
            external_tool_identity,
        )
    else:  # defense in depth; argparse and role checks reject this first.
        _fail("controller operation is not allow-listed")


def _remove_controller_owned_tree(path: Path, parent: Path) -> None:
    """Remove only an exact controller-created child without following links."""

    if path.parent != parent or not path.name:
        _fail("controller cleanup target escaped its exact parent")
    if path.is_symlink():
        path.unlink()
        return
    if not path.exists():
        return
    if path.is_file():
        path.chmod(0o600)
        path.unlink()
        return
    for current, directories, files in os.walk(path, topdown=False, followlinks=False):
        current_path = Path(current)
        for name in files:
            child = current_path / name
            if child.is_symlink():
                child.unlink()
            else:
                child.chmod(0o600)
        for name in directories:
            child = current_path / name
            if child.is_symlink():
                child.unlink()
            else:
                child.chmod(0o700)
        current_path.chmod(0o700)
    if not shutil.rmtree.avoids_symlink_attacks:
        _fail("platform lacks symlink-safe controller scratch cleanup")
    shutil.rmtree(path)


def _capture_helper_args(
    operation_args: Sequence[str],
    receipt: Path,
    runtime_root: Path,
    privacy_output: Path,
    privacy_work: Path,
) -> tuple[list[str], list[str], Path, Path]:
    _subcommand, option_values = _operation_option_values(
        "capture-four-peer", operation_args
    )
    required = {
        flag: option_values.get(flag, [])
        for flag in (
            "--artifact-handoff-sha256",
            "--exact12-matrix",
            "--linux-archive",
            "--output",
            "--privacy-action-driver",
            "--privacy-jindo-driver",
            "--privacy-network-driver",
            "--source-identity",
            "--validator-binary",
        )
    }
    if any(len(values) != 1 for values in required.values()):
        _fail("capture composite lacks one exact privacy qualification input")
    transformed: list[str] = []
    values = list(operation_args)
    index = 0
    while index < len(values):
        flag = values[index]
        index += 1
        if flag in BOOLEAN_FLAGS:
            transformed.append(flag)
            continue
        value = values[index]
        index += 1
        if flag in {
            "--exact12-matrix",
            "--linux-archive",
            "--privacy-action-driver",
            "--privacy-jindo-driver",
            "--privacy-network-driver",
            "--source-identity",
        }:
            continue
        transformed.extend((flag, str(receipt) if flag == "--output" else value))
    transformed.extend(("--runtime-root", str(runtime_root)))
    privacy_args = [
        "--validator-binary",
        required["--validator-binary"][0],
        "--action-driver",
        required["--privacy-action-driver"][0],
        "--network-driver",
        required["--privacy-network-driver"][0],
        "--jindo-driver",
        required["--privacy-jindo-driver"][0],
        "--linux-archive",
        required["--linux-archive"][0],
        "--exact12-matrix",
        required["--exact12-matrix"][0],
        "--source-identity",
        required["--source-identity"][0],
        "--artifact-handoff-sha256",
        required["--artifact-handoff-sha256"][0],
        "--output-directory",
        str(privacy_output),
        "--work-directory",
        str(privacy_work),
    ]
    return (
        transformed,
        privacy_args,
        Path(required["--output"][0]),
        Path(required["--source-identity"][0]),
    )


def _dispatch_capture_composite(
    operation_args: Sequence[str], attestation: dict[str, object]
) -> int:
    """Capture and freeze one qualification receipt without a caller byte gap."""

    handoff_root = Path(str(attestation["handoff_root"]))
    runtime_uid, runtime_gid, _runtime_identity_root = _identity_contract(
        attestation, "runtime"
    )
    scratch = Path(
        tempfile.mkdtemp(prefix=".qualification-capture-", dir=handoff_root)
    )
    if scratch.parent != handoff_root or scratch.is_symlink():
        _fail("capture scratch escaped the immutable handoff root")
    scratch.chmod(0o711)
    runtime_work = scratch / "runtime-work"
    runtime_work.mkdir(mode=0o700)
    os.chown(runtime_work, runtime_uid, runtime_gid)
    receipt = runtime_work / "four-peer-receipt-v2.json"
    harness_root = runtime_work / "harness"
    privacy_output = runtime_work / "privacy-protocol-four-peer-v2"
    privacy_work = runtime_work / "privacy-work"
    final_output: Path | None = None
    completed = False
    try:
        (
            helper_args,
            privacy_args,
            final_output,
            source_identity,
        ) = _capture_helper_args(
            operation_args,
            receipt,
            harness_root,
            privacy_output,
            privacy_work,
        )
        if final_output is None:  # defense in depth for static path narrowing
            _fail("capture composite final output is absent")
        result = _dispatch_installed_python(
            PRIVACY_CAPTURE_HELPER,
            privacy_args,
            (runtime_uid, runtime_gid),
        )
        if result != 0:
            return result
        _validate_operation_outputs({privacy_output}, runtime_uid, runtime_gid)
        result = _dispatch(
            "capture-four-peer",
            helper_args,
            (runtime_uid, runtime_gid),
        )
        if result != 0:
            return result
        _validate_operation_outputs(
            {privacy_output, receipt}, runtime_uid, runtime_gid
        )
        result = _dispatch_installed_python(
            QUALIFICATION_CLOSE_HELPER,
            [
                "--receipt",
                str(receipt),
                "--privacy-protocol-evidence-dir",
                str(privacy_output),
                "--source-identity",
                str(source_identity),
                "--output",
                str(final_output),
            ],
            None,
        )
        if result != 0:
            return result
        _validate_operation_outputs(
            {final_output},
            int(attestation["uid"]),
            int(attestation.get("controller_gid", 0)),
        )
        completed = True
        return 0
    finally:
        _remove_controller_owned_tree(scratch, handoff_root)
        if (
            not completed
            and final_output is not None
            and (final_output.exists() or final_output.is_symlink())
        ):
            _remove_controller_owned_tree(final_output, handoff_root)


def _dispatch_boi_composite(
    operation_args: Sequence[str], attestation: dict[str, object]
) -> int:
    """Refuse the former authority-process native-probe/signing composite."""

    del operation_args, attestation
    _fail(BOI_QUALIFICATION_ISSUANCE_BARRIER)


def _publish_public_soak_prerequisite_handoff(
    output: Path,
    payload: bytes,
    *,
    kind: str,
    handoff_root: Path,
    controller_uid: int,
    controller_gid: int,
) -> None:
    """Close one compact prerequisite file into a root-owned exact handoff."""

    if output.parent != handoff_root or output.exists() or output.is_symlink():
        _fail("public-soak prerequisite handoff output is not fresh and exact")
    if not payload or len(payload) > MAX_CONTROLLER_BYTES:
        _fail("public-soak prerequisite handoff payload is not bounded")
    leaf = "public-soak-prerequisite-v1.json"
    manifest = canonical_json_bytes(
        {
            "files": [
                {
                    "path": leaf,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "size": len(payload),
                }
            ],
            "kind": kind,
            "schema": "iroha.taira.release_handoff",
            "schema_version": 1,
        }
    )
    output.mkdir(mode=0o700)
    completed = False
    try:
        os.chown(output, controller_uid, controller_gid)
        output.chmod(0o700)
        for name, body in ((leaf, payload), (HANDOFF_MANIFEST, manifest)):
            descriptor = os.open(
                output / name,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_NOFOLLOW", 0)
                | getattr(os, "O_CLOEXEC", 0),
                0o400,
            )
            try:
                os.fchown(descriptor, controller_uid, controller_gid)
                os.fchmod(descriptor, 0o444)
                view = memoryview(body)
                while view:
                    written = os.write(descriptor, view)
                    if written <= 0:
                        _fail("public-soak prerequisite handoff write was short")
                    view = view[written:]
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        output_descriptor = os.open(
            output,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(output_descriptor)
        finally:
            os.close(output_descriptor)
        output.chmod(0o555)
        root_descriptor = os.open(
            handoff_root,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(root_descriptor)
        finally:
            os.close(root_descriptor)
        completed = True
    finally:
        if not completed:
            _remove_controller_owned_tree(output, handoff_root)


def _dispatch_public_soak_prerequisite_composite(
    operation: str,
    operation_args: Sequence[str],
    attestation: dict[str, object],
) -> int:
    """Inject the current publisher attestation into one path-only producer."""

    command = {
        "build-public-soak-candidate": "candidate",
        "build-public-soak-publication": "publication",
    }.get(operation)
    if command is None:
        _fail("public-soak prerequisite operation is not exact")
    authority_uid, authority_gid, authority_root = _identity_contract(
        attestation, "authority"
    )
    controller_uid = int(attestation["uid"])
    controller_gid = int(attestation.get("controller_gid", 0))
    handoff_root = Path(str(attestation["handoff_root"]))
    _subcommand, values = _operation_option_values(operation, operation_args)
    output_values = values.get("--output", [])
    if len(output_values) != 1:
        _fail("public-soak prerequisite output is absent")
    final_output = Path(output_values[0])
    scratch = Path(
        tempfile.mkdtemp(prefix=".public-soak-prerequisite-", dir=authority_root)
    )
    try:
        if scratch.parent != authority_root or scratch.is_symlink():
            _fail("public-soak prerequisite scratch escaped the authority root")
        os.chown(scratch, authority_uid, authority_gid)
        scratch.chmod(0o700)
        provenance = scratch / "publisher-controller-attestation.json"
        private_output = scratch / "public-soak-prerequisite-v1.json"
        descriptor = os.open(
            provenance,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            0o400,
        )
        try:
            os.fchown(descriptor, authority_uid, authority_gid)
            os.fchmod(descriptor, 0o400)
            payload = canonical_json_bytes(attestation)
            offset = 0
            while offset < len(payload):
                written = os.write(descriptor, payload[offset:])
                if written <= 0:
                    _fail("public-soak prerequisite attestation write was short")
                offset += written
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        scratch_descriptor = os.open(
            scratch,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(scratch_descriptor)
        finally:
            os.close(scratch_descriptor)
        transformed = list(operation_args)
        transformed[transformed.index("--output") + 1] = str(private_output)
        result = _dispatch_installed_python(
            PYTHON_OPERATIONS[operation],
            [
                command,
                *transformed,
                "--publisher-controller-attestation",
                str(provenance),
            ],
            (authority_uid, authority_gid),
        )
        if result != 0:
            return result
        _validate_operation_outputs(
            {private_output}, authority_uid, authority_gid
        )
        payload = _read_stable(private_output, MAX_CONTROLLER_BYTES)
        _publish_public_soak_prerequisite_handoff(
            final_output,
            payload,
            kind=(
                "public-soak-candidate-prerequisite"
                if command == "candidate"
                else "public-soak-publication-prerequisite"
            ),
            handoff_root=handoff_root,
            controller_uid=controller_uid,
            controller_gid=controller_gid,
        )
        return 0
    finally:
        _remove_controller_owned_tree(scratch, authority_root)


def _dispatch_publication_composite(
    operation_args: Sequence[str], attestation: dict[str, object]
) -> int:
    """Refuse publication until observation authority is provisioned."""

    _require_authenticated_rollout_observation_authority()
    _subcommand, option_values = _operation_option_values(
        "publish-rollout", operation_args
    )
    authority_uid, authority_gid, authority_root = _identity_contract(
        attestation, "authority"
    )
    controller_uid = int(attestation["uid"])
    controller_gid = int(attestation.get("controller_gid", 0))
    handoff_root = Path(str(attestation["handoff_root"]))
    receipt_id = option_values["--expected-qualification-receipt-id"][0]
    final_output = handoff_root / f"publication-receipt-{receipt_id}"
    if final_output.exists() or final_output.is_symlink():
        _fail("public publication handoff must be fresh")

    repository = _trusted_value_for(
        attestation, "publish-rollout", "--repository"
    )
    suffix = _trusted_value_for(attestation, "publish-rollout", "--suffix")
    oras_version = _trusted_value_for(
        attestation, "publish-rollout", "--expected-oras-version"
    )
    signing_fingerprint = _trusted_value_for(
        attestation, "publish-rollout", "--trusted-signing-fingerprint"
    )
    registry_config = _trusted_input_path_for(
        attestation, "publish-rollout", "--registry-config"
    )
    signing_public_key = _trusted_input_path_for(
        attestation, "publish-rollout", "--signing-public-key"
    )
    _validate_publisher_trusted_input(
        registry_config,
        "--registry-config",
        authority_uid,
        authority_gid,
        authority_root,
        signing_fingerprint,
    )
    _validate_publisher_trusted_input(
        signing_public_key,
        "--signing-public-key",
        authority_uid,
        authority_gid,
        authority_root,
        signing_fingerprint,
    )

    executable_values: dict[str, tuple[Path, str]] = {}
    for flag in sorted(SEALED_EXECUTABLE_DEPENDENCIES["publish-rollout"]):
        matches = [
            row
            for row in attestation["trusted_executables"]  # type: ignore[union-attr]
            if isinstance(row, dict)
            and row.get("operation") == "publish-rollout"
            and row.get("flag") == flag
        ]
        if len(matches) != 1:
            _fail("publisher executable lacks one sealed trust record")
        record = matches[0]
        path = Path(str(record.get("path", "")))
        digest = str(record.get("sha256", ""))
        if (
            record.get("run_as") != "authority"
            or record.get("digest_flag")
            != _expected_executable_digest_flag("publish-rollout", flag)
        ):
            _fail("publisher executable trust identity differs")
        _validate_trusted_executable_path(path, digest, exact_mode=0o555)
        executable_values[flag] = (path, digest)

    scratch = Path(
        tempfile.mkdtemp(prefix=".publish-rollout-", dir=authority_root)
    )
    completed = False
    try:
        if scratch.parent != authority_root or scratch.is_symlink():
            _fail("publisher scratch escaped the authority root")
        os.chown(scratch, authority_uid, authority_gid)
        scratch.chmod(0o700)
        terminal = scratch / "terminal"
        publisher_args = [
            "--candidate-root",
            option_values["--candidate-root"][0],
            "--expected-source-commit",
            option_values["--expected-source-commit"][0],
            "--expected-dpn-validator-release-commit",
            option_values["--expected-dpn-validator-release-commit"][0],
            "--expected-cargo-lock-sha256",
            option_values["--expected-cargo-lock-sha256"][0],
            "--expected-workspace-source-manifest-sha256",
            option_values["--expected-workspace-source-manifest-sha256"][0],
            "--expected-qualification-receipt-id",
            receipt_id,
            "--repository",
            repository,
            "--suffix",
            suffix,
            "--authority-uid",
            str(authority_uid),
            "--scratch-parent",
            str(scratch),
            "--registry-config",
            str(registry_config),
            "--oras",
            str(executable_values["--oras"][0]),
            "--trusted-oras-sha256",
            executable_values["--oras"][1],
            "--expected-oras-version",
            oras_version,
            "--external-signer",
            str(executable_values["--external-signer"][0]),
            "--trusted-external-signer-sha256",
            executable_values["--external-signer"][1],
            "--signing-public-key",
            str(signing_public_key),
            "--trusted-signing-fingerprint",
            signing_fingerprint,
            "--release-manifest-verifier",
            str(executable_values["--release-manifest-verifier"][0]),
            "--trusted-release-manifest-verifier-sha256",
            executable_values["--release-manifest-verifier"][1],
            "--terminal-handoff",
            str(terminal),
            "--rollout-plan",
            option_values["--rollout-plan"][0],
            "--rollout-result",
            option_values["--rollout-result"][0],
            "--rollout-authority-envelope",
            option_values["--rollout-authority-envelope"][0],
            "--rollout-durable-receipt",
            option_values["--rollout-durable-receipt"][0],
        ]
        result = _dispatch(
            "publish-rollout",
            publisher_args,
            (authority_uid, authority_gid),
            (authority_uid, authority_gid),
        )
        if result != 0:
            return result
        _validate_operation_outputs({terminal}, authority_uid, authority_gid)
        result = _dispatch_installed_python(
            PUBLICATION_CLOSE_HELPER,
            [
                "--source-parent",
                str(scratch),
                "--handoff-root",
                str(handoff_root),
                "--expected-authority-uid",
                str(authority_uid),
                "--expected-authority-gid",
                str(authority_gid),
                "--expected-controller-uid",
                str(controller_uid),
                "--expected-controller-gid",
                str(controller_gid),
                "--expected-qualification-receipt-id",
                receipt_id,
                "--expected-signing-fingerprint",
                signing_fingerprint,
                "--expected-source-commit",
                option_values["--expected-source-commit"][0],
                "--expected-dpn-validator-release-commit",
                option_values["--expected-dpn-validator-release-commit"][0],
                "--expected-cargo-lock-sha256",
                option_values["--expected-cargo-lock-sha256"][0],
                "--expected-workspace-source-manifest-sha256",
                option_values[
                    "--expected-workspace-source-manifest-sha256"
                ][0],
                "--rollout-plan",
                option_values["--rollout-plan"][0],
                "--rollout-result",
                option_values["--rollout-result"][0],
                "--rollout-authority-envelope",
                option_values["--rollout-authority-envelope"][0],
                "--rollout-durable-receipt",
                option_values["--rollout-durable-receipt"][0],
            ],
            None,
        )
        if result != 0:
            return result
        _validate_operation_outputs(
            {final_output}, controller_uid, controller_gid
        )
        completed = True
        return 0
    finally:
        _remove_controller_owned_tree(scratch, authority_root)
        if not completed and (final_output.exists() or final_output.is_symlink()):
            _remove_controller_owned_tree(final_output, handoff_root)


def _add_attestation_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--expected-launcher-sha256", required=True)
    parser.add_argument("--expected-controller-digest", required=True)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--expected-host-id", required=True)
    parser.add_argument("--expected-installation-id", required=True)
    parser.add_argument("--expected-uid", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--platform", choices=sorted(PLATFORM_FILES), required=True)
    parser.add_argument("--role", choices=sorted(ROLE_OPERATIONS), required=True)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="command", required=True)
    attest_parser = subparsers.add_parser("attest", allow_abbrev=False)
    _add_attestation_arguments(attest_parser)
    inspect_parser = subparsers.add_parser(
        "inspect-handoff",
        allow_abbrev=False,
    )
    _add_attestation_arguments(inspect_parser)
    inspect_parser.add_argument("--root", required=True)
    inspect_parser.add_argument("--expected-kind", required=True)
    inspect_parser.add_argument("--stage-name", required=True)
    run_parser = subparsers.add_parser("run", allow_abbrev=False)
    _add_attestation_arguments(run_parser)
    run_parser.add_argument("operation", choices=sorted(PYTHON_OPERATIONS | BASH_OPERATIONS))
    run_parser.add_argument("operation_args", nargs=argparse.REMAINDER)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.command == "run" and args.operation == "assemble-boi":
            _fail(BOI_QUALIFICATION_ISSUANCE_BARRIER)
        if args.command == "run" and args.operation == "deploy-reset":
            _fail(DEPLOY_ISSUANCE_BARRIER)
        if args.command == "run" and args.operation in {
            "verify-privacy-rollout",
            "publish-rollout",
        }:
            _require_authenticated_rollout_observation_authority()
        attestation = _attest(
            expected_launcher_sha256=args.expected_launcher_sha256,
            expected_controller_digest=args.expected_controller_digest,
            expected_version=args.expected_version,
            expected_host_id=args.expected_host_id,
            expected_installation_id=args.expected_installation_id,
            expected_uid=args.expected_uid,
            source_commit=args.source_commit,
            platform_name=args.platform,
            role=args.role,
        )
        if args.command == "attest":
            sys.stdout.buffer.write(canonical_json_bytes(attestation))
            return 0
        if args.command == "inspect-handoff":
            result = {
                **attestation,
                **inspect_handoff(
                    Path(args.root),
                    args.expected_kind,
                    Path(str(attestation["handoff_root"])),
                    int(attestation["uid"]),
                    args.stage_name,
                    controller_gid=int(attestation.get("controller_gid", 0)),
                ),
            }
            sys.stdout.buffer.write(canonical_json_bytes(result))
            return 0
        allowed = ROLE_OPERATIONS[args.role][1]
        if args.operation not in allowed:
            _fail("controller operation is not permitted for this runner role")
        operation_args = list(args.operation_args)
        if operation_args[:1] == ["--"]:
            operation_args.pop(0)
        staged_roots, output_paths = _validate_operation_args(
            args.operation, operation_args, attestation
        )
        operation_identity = _expected_operation_identity(args.role, args.operation)
        runs_as_root = operation_identity == "root"
        root_composite = args.operation in {
            "assemble-boi",
            "capture-four-peer",
            "publish-rollout",
        }
        if root_composite and runs_as_root:
            _fail("root composite child identity contract differs")
        if not root_composite and (
            (args.operation in ROOT_REQUIRED_OPERATIONS) != runs_as_root
        ):
            _fail("root-required operation identity contract differs")
        if args.operation == "capture-four-peer":
            result = _dispatch_capture_composite(operation_args, attestation)
        elif args.operation == "assemble-boi":
            result = _dispatch_boi_composite(operation_args, attestation)
        elif args.operation in {
            "build-public-soak-candidate",
            "build-public-soak-publication",
        }:
            result = _dispatch_public_soak_prerequisite_composite(
                args.operation, operation_args, attestation
            )
        elif args.operation == "publish-rollout":
            result = _dispatch_publication_composite(operation_args, attestation)
        else:
            run_as = None
            external_tool_identity = None
            if not runs_as_root:
                child_uid, child_gid, _child_root = _identity_contract(
                    attestation, operation_identity
                )
                run_as = (child_uid, child_gid)
            elif args.operation == "deploy-reset":
                authority_uid, authority_gid, _authority_root = _identity_contract(
                    attestation, "authority"
                )
                external_tool_identity = (authority_uid, authority_gid)
            result = _dispatch(
                args.operation,
                operation_args,
                run_as,
                external_tool_identity,
            )
        _revalidate_staged_roots(
            Path(str(attestation["handoff_root"])),
            staged_roots,
            expected_owner=int(attestation["uid"]),
            expected_group=int(attestation.get("controller_gid", 0)),
        )
        _revalidate_bound_roots(attestation)
        if result == 0:
            _validate_successful_operation_outputs(
                args.operation,
                output_paths,
                attestation,
                operation_identity,
            )
        return result
    except (ControllerSealError, OSError, subprocess.SubprocessError) as exc:
        print(f"Taira installed-controller error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
