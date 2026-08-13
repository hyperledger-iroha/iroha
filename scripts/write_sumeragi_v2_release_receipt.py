#!/usr/bin/env python3
"""Validate and aggregate source-bound Sumeragi v2 release evidence."""

from __future__ import annotations

import argparse
import base64
import csv
from dataclasses import dataclass
from datetime import datetime
import hashlib
import importlib.util
import io
import json
import math
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
import tempfile
import time
import types
from typing import Any, BinaryIO


_LOCALNET_MANIFEST_MODULE_PATH = Path(__file__).resolve(strict=True).with_name(
    "sumeragi_v2_localnet_manifest.py"
)
_LOCALNET_MANIFEST_SPEC = importlib.util.spec_from_file_location(
    "_sumeragi_v2_release_localnet_manifest",
    _LOCALNET_MANIFEST_MODULE_PATH,
)
if _LOCALNET_MANIFEST_SPEC is None or _LOCALNET_MANIFEST_SPEC.loader is None:
    raise RuntimeError("could not load the adjacent localnet manifest validator")
_LOCALNET_MANIFEST_MODULE = importlib.util.module_from_spec(
    _LOCALNET_MANIFEST_SPEC
)
_PREVIOUS_DONT_WRITE_BYTECODE = sys.dont_write_bytecode
sys.dont_write_bytecode = True
try:
    _LOCALNET_MANIFEST_SPEC.loader.exec_module(_LOCALNET_MANIFEST_MODULE)
finally:
    sys.dont_write_bytecode = _PREVIOUS_DONT_WRITE_BYTECODE
LocalnetManifestError = _LOCALNET_MANIFEST_MODULE.LocalnetManifestError
canonical_localnet_manifest = _LOCALNET_MANIFEST_MODULE.canonical_localnet_manifest

_RELEASE_RECEIPT_COMPONENT_FILES = (
    "write_sumeragi_v2_release_receipt_formal_artifacts.py",
    "write_sumeragi_v2_release_receipt_corridor_log.py",
    "write_sumeragi_v2_release_receipt_gate_evidence.py",
    "write_sumeragi_v2_release_receipt_publication.py",
)
_RELEASE_RECEIPT_COMPONENT_SHA256 = {
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "43a815d4257ad6296a48e125dfab52c5f31aabba5210f4154641164887e48886"
    ),
    "write_sumeragi_v2_release_receipt_corridor_log.py": (
        "f5c4e3bf8d8a86890abba38f559058df676e5a311aacead265ce0f999d6395bd"
    ),
    "write_sumeragi_v2_release_receipt_gate_evidence.py": (
        "0cc7e2a43479fb27305974559c331d4494df161cfc7c75fe9c51f324b09e058a"
    ),
    "write_sumeragi_v2_release_receipt_publication.py": (
        "d5f666eab695c3ca4668a3a3e1074a53b8fc63aac3d852036d0c20622e027b45"
    ),
}

_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SSH_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_SSH_ARMOR_BEGIN = b"-----BEGIN SSH SIGNATURE-----"
_SSH_ARMOR_END = b"-----END SSH SIGNATURE-----"
_UNSUPPORTED_ARMOR_MARKERS = (
    b"-----BEGIN PGP SIGNATURE-----",
    b"-----BEGIN SIGNED MESSAGE-----",
    b"-----BEGIN CERTIFICATE-----",
)
_TRAILER_VERSION = "Sumeragi-V2-Release-Identity-Version"
_TRAILER_MANIFEST = "Sumeragi-V2-Source-Manifest-SHA256"
_TRAILER_LOCK = "Sumeragi-V2-Cargo-Lock-SHA256"
_TRAILER_KEYS = (_TRAILER_VERSION, _TRAILER_MANIFEST, _TRAILER_LOCK)
_SIGNATURE_ARCHIVE_NAMES = {
    "attestation": "identity-attestation.json",
    "verify_transcript": "identity-transcript.json",
    "raw_commit": "identity-raw-commit",
    "cargo_lock": "identity-Cargo.lock",
    "ssh_allowed_signers": "identity-allowed-signers",
    "ssh_revocation": "identity-revocation",
    "git": "identity-git",
    "ssh_keygen": "identity-ssh-keygen",
}
_SIGNATURE_ARCHIVE_IDS = {
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}
_SIGNATURE_ATTESTATION_FORMAT = "iroha-sumeragi-v2-release-identity-attestation"
_SIGNATURE_TRANSCRIPT_FORMAT = "iroha-sumeragi-v2-release-identity-transcript"
_SIGNATURE_DATA_MODE = 0o400
_SIGNATURE_TOOL_MODE = 0o500
_SIGNATURE_DIRECTORY_MODE = 0o700
_MAX_SIGNATURE_JSON_BYTES = 8 * 1024 * 1024
_MAX_RAW_COMMIT_BYTES = 16 * 1024 * 1024
_MAX_LOCK_BYTES = 128 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_HELPER_BYTES = 16 * 1024 * 1024
_MAX_SDK_MANIFEST_BYTES = 256 * 1024 * 1024
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_FRAMEWORK_RUNTIME_BYTES = 4 * 1024 * 1024 * 1024
_MAX_FRAMEWORK_RUNTIME_MEMBERS = 250_000
_MAX_RUNNER_TOOL_TOTAL_BYTES = 4 * 1024 * 1024 * 1024
_MAX_REPLAY_OUTPUT_BYTES = 4 * 1024 * 1024
_MAX_LOCALNET_MANIFEST_INDEX_BYTES = 1024 * 1024
_MAX_LOCALNET_MANIFEST_BYTES = 64 * 1024 * 1024
_MAX_SCALING_JSON_BYTES = 16 * 1024 * 1024
_MAX_SCALING_BUNDLE_FILE_COUNT = 256
_MAX_SCALING_BUNDLE_DIRECTORY_COUNT = 512
_MAX_SCALING_BUNDLE_FILE_BYTES = 256 * 1024 * 1024
_MAX_SCALING_BUNDLE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
_MAX_G4P_TSV_BYTES = 1024 * 1024
_MAX_G4P_LOG_BYTES = 16 * 1024 * 1024
_MAX_G12_TSV_BYTES = 1024 * 1024
_MAX_G12_LOG_BYTES = 16 * 1024 * 1024
_MAX_PREBUILT_MANIFEST_BYTES = 32 * 1024
_MAX_PREBUILT_VERSION_TRANSCRIPT_BYTES = 64 * 1024
_MAX_PREBUILT_BINARY_BYTES = 2 * 1024 * 1024 * 1024
_MAX_RELEASE_TSV_BYTES = 16 * 1024 * 1024
_MAX_RELEASE_TEXT_BYTES = 256 * 1024 * 1024
_MAX_RELEASE_JSON_BYTES = 128 * 1024 * 1024
_MAX_SDK_INVENTORY_BYTES = 256 * 1024 * 1024
_MAX_SDK_ARCHIVE_BYTES = 64 * 1024 * 1024 * 1024
_MAX_SDK_RECORDS = 250_000
_SDK_GRADLE_DISTRIBUTION_URL = (
    "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
)
_SDK_GRADLE_WRAPPER_CACHE_KEY = "79n14ral3mx1ozqr3csh2u872"
_SDK_GRADLE_LAUNCHER_ARCHIVE_NAME = (
    "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
    f"{_SDK_GRADLE_WRAPPER_CACHE_KEY}/gradle-9.3.0/bin/gradle"
)
_SDK_SOURCE_MANIFEST_FORMAT = "iroha-sumeragi-v2-sdk-dependency-sources"
_SDK_SOURCE_INVENTORY_FORMAT = (
    "iroha-sumeragi-v2-sdk-dependency-source-inventory"
)
_MAX_TLAPS_RESOURCE_RECORDS = 1_000_000
_PREBUILT_MANIFEST_NAME = ".sumeragi-v2-prebuilt-binaries.tsv"
_PREBUILT_INVOCATION_RE = re.compile(r"invocation\.[A-Za-z0-9]+")
_PREBUILT_TRIPLE_RE = re.compile(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_.]+)+")
_PREBUILT_BINARY_SPECS = (
    ("irohad", "release/iroha3d"),
    (
        "irohad_message_control",
        "message-control/release/iroha3d",
    ),
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
        for prefix, _ in _PREBUILT_BINARY_SPECS
        for field in (
            f"{prefix}_relative_path",
            f"{prefix}_sha256",
            f"{prefix}_size_bytes",
            f"{prefix}_mode_octal",
        )
    ),
)
_SCALING_REQUIRED_TOOLING = (
    ("localnet", "scripts/deploy_localnet.sh"),
    ("load_generator", "scripts/tx_load.py"),
    ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
)
_REPLAY_TIMEOUT_SECONDS = 120
_FROZEN_BOOTSTRAP_SHA256 = (
    "54fdb6bca310890d4d5c195925ddafafb74c89ec7b33ce4cd339846177b5bdb4"
)
_BOOTSTRAP_COMPLETION_NAME = "BOOTSTRAP_COMPLETED.json"
_BOOTSTRAP_TRUSTED_ARCHIVES = {
    "allowed_signers": ("bootstrap-allowed-signers", _SIGNATURE_DATA_MODE),
    "bash": ("bash", _SIGNATURE_TOOL_MODE),
    "bootstrap": ("trusted-bootstrap.py", _SIGNATURE_DATA_MODE),
    "git": ("git", _SIGNATURE_TOOL_MODE),
    "identity_verifier": ("verify-identity.py", _SIGNATURE_DATA_MODE),
    "manifest_helper": ("compute-manifest.py", _SIGNATURE_DATA_MODE),
    "python": ("python3", _SIGNATURE_TOOL_MODE),
    "receipt_validator": ("validate-receipt.py", _SIGNATURE_DATA_MODE),
    "receipt_validator_support": (
        "sumeragi_v2_localnet_manifest.py",
        _SIGNATURE_DATA_MODE,
    ),
    "runtime_helper": ("copy-release-runtime.py", _SIGNATURE_DATA_MODE),
    "tool_probe_helper": ("probe-release-tools.py", _SIGNATURE_DATA_MODE),
    "approval_contract": ("release-approval-contract.py", _SIGNATURE_DATA_MODE),
    "approval_offline_toolchain_sdk": (
        "offline-toolchain-sdk.approval.v1.json",
        _SIGNATURE_DATA_MODE,
    ),
    "approval_formal_proof_tools": (
        "formal-proof-tools.approval.v1.json",
        _SIGNATURE_DATA_MODE,
    ),
    "approval_network_scale_soak": (
        "network-scale-soak.approval.v1.json",
        _SIGNATURE_DATA_MODE,
    ),
    "approval_final_bootstrap_publication": (
        "final-bootstrap-publication.approval.v1.json",
        _SIGNATURE_DATA_MODE,
    ),
    "sdk_dependency_bundle_manifest": (
        "sdk-dependency-bundle-manifest.json",
        _SIGNATURE_DATA_MODE,
    ),
    "revocation": ("bootstrap-revocation", _SIGNATURE_DATA_MODE),
    "runner_tool_manifest": ("runner-tool-manifest.json", _SIGNATURE_DATA_MODE),
    "ssh_keygen": ("ssh-keygen", _SIGNATURE_TOOL_MODE),
}
_RECEIPT_VALIDATOR_COMPONENT_SHA256 = {
    "write_sumeragi_v2_release_receipt_corridor_log.py": (
        "f5c4e3bf8d8a86890abba38f559058df676e5a311aacead265ce0f999d6395bd"
    ),
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "43a815d4257ad6296a48e125dfab52c5f31aabba5210f4154641164887e48886"
    ),
    "write_sumeragi_v2_release_receipt_gate_evidence.py": (
        "0cc7e2a43479fb27305974559c331d4494df161cfc7c75fe9c51f324b09e058a"
    ),
    "write_sumeragi_v2_release_receipt_publication.py": (
        "d5f666eab695c3ca4668a3a3e1074a53b8fc63aac3d852036d0c20622e027b45"
    ),
}
_BOOTSTRAP_COMPONENT_SHA256 = {
    "bootstrap_sumeragi_v2_release_receipt_replay.py": (
        "a11e17139adf7257126328d7f0c9f2903a6911c9ff4a81e50bb2818362f2b39b"
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
_APPROVAL_ATTESTATION_NAMES = {
    class_id: f"{class_id}.approval-attestation.v1.json"
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_SET_ATTESTATION_NAME = "release-approval-set-attestation.v1.json"
_APPROVAL_SET_ARCHIVE_ID = "release-approval.set-attestation.v1"
_APPROVAL_OPERATION_COUNTS = {
    "offline-toolchain-sdk": 23,
    "formal-proof-tools": 38,
    "network-scale-soak": 8,
    "final-bootstrap-publication": 8,
}
_BOOTSTRAP_IDENTITY_ARCHIVES = {
    "cargo_lock": ("identity-Cargo.lock", _SIGNATURE_DATA_MODE),
    "git": ("identity-git", _SIGNATURE_TOOL_MODE),
    "identity_attestation": ("identity-attestation.json", _SIGNATURE_DATA_MODE),
    "identity_transcript": ("identity-transcript.json", _SIGNATURE_DATA_MODE),
    "raw_commit": ("identity-raw-commit", _SIGNATURE_DATA_MODE),
    "ssh_allowed_signers": ("identity-allowed-signers", _SIGNATURE_DATA_MODE),
    "ssh_keygen": ("identity-ssh-keygen", _SIGNATURE_TOOL_MODE),
    "ssh_revocation": ("identity-revocation", _SIGNATURE_DATA_MODE),
    "verify_transcript": ("identity-transcript.json", _SIGNATURE_DATA_MODE),
}
_BOOTSTRAP_RUNNER_ENV_ALLOWLIST = {
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
_BOOTSTRAP_RUNNER_ENV_RE = re.compile(r"[A-Z][A-Z0-9_]*")
_BOOTSTRAP_SAFE_PATH_RE = re.compile(r"/[A-Za-z0-9_./+:-]+")
_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_FORMAL_FINAL_MARKER = (
    "Sumeragi v2 formal gate passed: source-bound TLAPS, all registered "
    "adversarial scheduler/readiness/indexed-height/item-carrier/reply-writer/"
    "recovery/ownership mutations, bounded TLC, trace replay, and production Verus"
)
_TLAPS_RESOURCE_MEMORY_LIMIT_BYTES = 2 * 1024 * 1024 * 1024
_TLAPS_RESOURCE_SAMPLE_INTERVAL_SECONDS = 0.25
_TLAPS_RESOURCE_PHYSICAL_FOOTPRINT_INTERVAL_SECONDS = 5.0
_TLAPS_RESOURCE_MEMORY_ENFORCEMENT_MODE = "max_rss_physical_footprint"
_TLAPS_RESOURCE_TIMESTAMP_RE = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z"
)
_SCALING_REPORT_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.validation.v1"
_SCALING_SAFE_PATH_COMPONENT_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
_APALACHE_VERSION = "0.52.2"
_APALACHE_LAUNCHER_SHA256 = (
    "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7"
)
_APALACHE_JAR_SHA256 = (
    "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a"
)
_APALACHE_REFINEMENT_RESULTS = (
    (
        "autoscale-lifecycle",
        "SumeragiV2AutoscaleLifecycle",
        "multilane_autoscale_lifecycle_fixed.cfg",
        "8",
    ),
    (
        "native-application-evidence",
        "SumeragiV2NativeApplicationEvidence",
        "multilane_native_application_evidence_fixed.cfg",
        "8",
    ),
    (
        "autonomous-reservation-carrier",
        "SumeragiV2AutonomousReservationCarrier",
        "multilane_autonomous_reservation_carrier_fixed.cfg",
        "10",
    ),
    (
        "queue-plan-admission-registry",
        "SumeragiV2QueuePlanAdmissionRegistry",
        "multilane_queue_plan_admission_registry_fixed.cfg",
        "8",
    ),
    (
        "kura-replica-retention",
        "SumeragiV2KuraReplicaRetention",
        "kura_replica_retention_fixed.cfg",
        "8",
    ),
)
_APALACHE_LAYOUT_ONLY_RESULTS = (
    # This row is bounded current-layout evidence, not a Rust transition theorem.
    (
        "inflight-first-release-layout",
        "SumeragiV2InFlightFirstRelease",
        "inflight_first_release_fixed.cfg",
        "18",
    ),
)
_APALACHE_RESULTS = (
    *_APALACHE_REFINEMENT_RESULTS,
    *_APALACHE_LAYOUT_ONLY_RESULTS,
)
_G12_SEED_PREFIX = "nexus-cross-dataspace-v1-seed-"
_G12_SEED_TEST = (
    "nexus::cross_dataspace_localnet::"
    "cross_dataspace_atomic_swap_is_all_or_nothing"
)
_G12_SOAK_TEST = (
    "nexus::cross_dataspace_localnet::"
    "cross_dataspace_two_hour_fault_soak_preserves_multilane_application"
)
_G4P_RELEASE_TESTS = (
    (
        "nexus_and_streaming",
        "nexus::autoscale_localnet::"
        "nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_"
        "rejects_stale_artifacts",
    ),
    (
        "nexus_and_streaming",
        "nexus::autoscale_localnet::"
        "nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart",
    ),
    (
        "nexus_and_streaming",
        "nexus::autoscale_localnet::"
        "nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_"
        "restart",
    ),
    (
        "native_amx_routing",
        "native_amx_rotating_validator_fault_soak_preserves_independent_"
        "participant_qcs",
    ),
)
_G4P_NATIVE_AMX_GROUPED_PRUNING_MARKER = (
    "[multilane-release-native-evidence] grouped_sources=2 "
    "durable_manifest=passed body_eviction_recovery=passed "
    "authenticated_remote_recovery=passed exact_once=passed"
)
_CHAOS_MARKER = (
    "SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 "
    "npos_heights=50000 total_heights=100000 supplied_commit_qcs=100000 "
    "supplied_tcs=75000 finalized_validators=400000 wal_append_restarts=314 "
    "fetch_restarts=312 store_restarts=312 validation_restarts=312 "
    "application_restarts=312 stale_generation_rejections=1562 "
    "deferred_fetch_completions=400936 deferred_store_completions=400624 "
    "deferred_validation_completions=400312 "
    "deferred_application_completions=400000 duplicate_commit_qcs=3124 "
    "reordered_commit_batches=75000 reordered_tc_batches=75000 "
    "insufficient_dual_qcs=1030 count_only_qcs=515 power_only_qcs=515 "
    "restart_interval=64 duplicate_interval=32 under_quorum_interval=97 "
    "certificate_source=external_fixture"
)
_CHAOS_FIXED_FIELDS = {
    "schema_version": "2",
    "permissioned_heights": "50000",
    "npos_heights": "50000",
    "completed_heights": "100000",
    "supplied_commit_qcs": "100000",
    "supplied_tcs": "75000",
    "finalized_validators": "400000",
    "wal_append_restarts": "314",
    "fetch_restarts": "312",
    "store_restarts": "312",
    "validation_restarts": "312",
    "application_restarts": "312",
    "stale_generation_rejections": "1562",
    "deferred_fetch_completions": "400936",
    "deferred_store_completions": "400624",
    "deferred_validation_completions": "400312",
    "deferred_application_completions": "400000",
    "duplicate_commit_qcs": "3124",
    "reordered_commit_batches": "75000",
    "reordered_tc_batches": "75000",
    "insufficient_dual_qcs": "1030",
    "count_only_qcs": "515",
    "power_only_qcs": "515",
    "restart_interval": "64",
    "duplicate_interval": "32",
    "under_quorum_interval": "97",
    "certificate_source": "external_fixture",
}
_HARNESS_LOCK_SHA256 = "9c49a60551d9f66c8786f2497cb107fb3214fb3420c4f5c23ba3d24814b3f97e"
_SEED_SCENARIOS = (
    "authoritative_v2_genesis_commits_on_every_validator",
    "authoritative_v2_finalizes_through_validator_restart",
    "taira_npos_leader_timeout_commits_within_rotation_bound",
    "real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release",
    "real_network_distinct_subject_prepare_qcs_converge_after_causal_release",
)
_SEED_RUNS_PER_SCENARIO = 32
_SEED_RUN_COUNT = len(_SEED_SCENARIOS) * _SEED_RUNS_PER_SCENARIO
_SEED_SUMMARY_FIELDS = (
    "profile",
    "source_manifest_sha256",
    "scenario",
    "seed",
    "result",
    "cargo_status",
    "tee_status",
    "run_log_sha256",
    "output",
    "localnet",
    "command",
)
_SEED_LOCALNET_MANIFEST_FIELDS = (
    "run_index",
    "localnet",
    "manifest",
    "manifest_sha256",
)
_CORRIDOR_SUMMARY_FIELDS = (
    "leg_index",
    "leg_id",
    "kind",
    "required_test_count",
    "observed_test_count",
    "command_status",
    "tee_status",
    "log_sha256",
    "log",
    "command",
)
_PRODUCTION_TEST_COUNT = 855
_G_UNIT_TEST_COUNT = 525
_G_UNIT_GROUPS = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
        319,
        "lib",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
        143,
        "lib",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
        9,
        "lib",
    ),
    (
        "required_multilane_config_runtime_focus_tests",
        "g-unit-iroha-config-runtime",
        "iroha_config",
        2,
        "test:sumeragi_v2_merge_runtime_config",
    ),
    (
        "required_multilane_config_fixtures_focus_tests",
        "g-unit-iroha-config-fixtures",
        "iroha_config",
        2,
        "test:fixtures",
    ),
    (
        "required_multilane_data_model_focus_tests",
        "g-unit-iroha-data-model",
        "iroha_data_model",
        8,
        "lib",
    ),
    (
        "required_multilane_torii_focus_tests",
        "g-unit-iroha-torii",
        "iroha_torii",
        39,
        "lib",
    ),
    (
        "required_multilane_torii_shared_focus_tests",
        "g-unit-iroha-torii-shared",
        "iroha_torii_shared",
        1,
        "lib",
    ),
    (
        "required_multilane_integration_lib_focus_tests",
        "g-unit-integration-tests",
        "integration_tests",
        2,
        "lib",
    ),
)
_PRODUCTION_MODULES = (
    ("production-kura-progress-durability", "kura::tests", 17),
    (
        "production-kura-lane-geometry",
        "kura::lane_geometry::tests",
        8,
    ),
    (
        "production-lane-relay-exact-ownership",
        "nexus::lane_relay::tests",
        4,
    ),
    (
        "production-authoritative-ingress",
        "sumeragi::authoritative_runtime_gate_tests",
        43,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 118),
    ("production-state-governance-unlock-audit", "state::tests", 1),
    ("production-v2-core", "sumeragi::v2_core::tests", 38),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 17),
    (
        "production-v2-core-wal",
        "sumeragi::v2_core::wal::byte_lifecycle_tests",
        1,
    ),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        8,
    ),
    (
        "production-v2-equivocation-evidence",
        "sumeragi::evidence::tests",
        1,
    ),
    (
        "production-v2-leader-wire-lifecycle-store",
        "sumeragi::serviced_candidate_store::tests",
        1,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 47),
    ("production-v2-body-store", "sumeragi::v2_body_store::tests", 2),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 3),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 72),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 61),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 68),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    (
        "production-v2-lifecycle-recovery",
        "sumeragi::v2_lifecycle_recovery::tests",
        5,
    ),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 37),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 133),
    (
        "production-v2-watchdog",
        "sumeragi::status::v2_liveness_watchdog_tests",
        19,
    ),
    (
        "production-kagemusha-finality",
        "zk::kagemusha_finality::tests",
        1,
    ),
    (
        "production-data-model-v2-finality",
        "block::consensus_v2::finality::tests",
        1,
    ),
    (
        "production-data-model-offline-compact-qc",
        "offline::kagemusha_v4_topup_provenance_tests",
        1,
    ),
    (
        "production-data-model-v2-context-identity",
        "block::consensus_v2::tests",
        2,
    ),
    (
        "production-v2-integration-runner",
        "sumeragi_v2_runner",
        4,
    ),
    (
        "production-p2p-peer-reliable-flush",
        "peer::run::tests",
        11,
    ),
    (
        "production-p2p-shared-source-byte-geometry",
        "peer::shared_byte_budget_tests",
        8,
    ),
    (
        "production-p2p-network-reliable-actor",
        "network::tests",
        84,
    ),
    (
        "production-p2p-source-memory-geometry",
        "network::inbound_source_memory_bound_tests",
        2,
    ),
    (
        "production-p2p-waiter-rank-geometry",
        "network::handle_update_tests",
        4,
    ),
    (
        "production-irohad-consensus-message-control",
        "consensus_message_control::tests",
        8,
    ),
    (
        "production-irohad-network-relay",
        "network_relay_tests",
        4,
    ),
    (
        "production-irohad-authenticated-via",
        "tests::relay_fairness",
        7,
    ),
    (
        "production-config-v2-exact-output-geometry",
        "parameters::actual::tests",
        2,
    ),
    (
        "production-config-v2-exact-output-root-parse",
        "parameters::user::duration_clamp_tests",
        5,
    ),
)
_PRODUCTION_INTEGRATION_MODULE = "sumeragi_v2_runner::prepare_qc_split_tests"
_DATA_MODEL_PRODUCTION_MODULES = (
    "block::consensus_v2::finality::tests",
    "offline::kagemusha_v4_topup_provenance_tests",
    "block::consensus_v2::tests",
)
_DATA_STATUS_TEST = (
    "block::consensus_v2::tests::"
    "status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry"
)
_DATA_LANE_CERTIFICATE_TEST = (
    "block::consensus::tests::lane_block_certificate_decodes_atomically_from_slice"
)
_TAIRA_CONTRACT_TESTS = (
    "taira_public_localnet::release_execution_profile_accepts_only_the_exact_positive_profile",
    "taira_public_localnet::release_execution_profile_rejects_wrong_or_blank_build_profiles",
    "taira_public_localnet::release_execution_profile_rejects_cargo_profile_mismatch",
    "taira_public_localnet::release_execution_profile_rejects_non_exact_offline_values",
    "taira_public_localnet::simulation_summary_json_records_release_profile_and_status_evidence",
    "taira_public_localnet::strict_restart::taira_localnet_restart_catchup_behavior",
)
_RUST_SDK_DIAGNOSTICS_TESTS = (
    "client::tests::get_sumeragi_status_prefers_norito_and_handles_json",
    "client::tests::get_sumeragi_status_rejects_unknown_json_fields",
    "client::tests::get_sumeragi_status_rejects_structurally_impossible_norito_and_json",
    "client::tests::get_sumeragi_status_json_requires_exact_json_media_type",
    "client::tests::get_sumeragi_diagnostics_verifies_lane_relay_envelopes",
    "client::tests::get_sumeragi_diagnostics_rejects_invalid_lane_relay_hash",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_autonomous_execution",
    "client::tests::get_sumeragi_diagnostics_rejects_duplicate_autonomous_execution_identity",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_native_amx_receipts_in_every_container",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_json_payload",
    "client::tests::get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields",
    "client::tests::get_sumeragi_diagnostics_rejects_unknown_json_fields",
    "client::tests::get_sumeragi_diagnostics_rejects_zero_npos_seed",
    "client::tests::get_sumeragi_diagnostics_requires_declared_current_media_type",
)
_CROSS_SDK_TESTS = (
    "sumeragi_v2_cross_sdk_fixtures::shared_sdk_accept_fixtures_are_exact_current_rust_encodings",
    "sumeragi_v2_cross_sdk_fixtures::shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation",
)
_NATIVE_AMX_GROUPED_PARITY_HARNESS = "ci/run_native_amx_v2_grouped_sdk_parity.sh"
_NATIVE_AMX_GROUPED_FIXTURE = "fixtures/sumeragi_v2/native_amx_v2_grouped.json"
_NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT = 55
_NATIVE_AMX_GROUPED_PARITY_SUITES = (
    ("openapi", 7),
    ("python", 62),
    ("javascript", 60),
    ("swift", 4),
    ("kotlin", 6),
    ("java", 5),
)
_SUMERAGI_SDK_DIAGNOSTICS_HARNESS = "ci/run_sumeragi_v2_sdk_diagnostics.sh"
_SUMERAGI_SDK_DIAGNOSTICS_SUITES = (
    ("python", 121),
    ("javascript", 88),
    ("swift", 17),
    ("kotlin", 26),
    ("java", 24),
)
_SDK_SOURCE_CLOSURE_RESOLVER = "ci/resolve_sumeragi_v2_sdk_source_closure.py"
_SDK_SOURCE_CLOSURE_MANIFEST = "ci/sumeragi_v2_sdk_source_closure.json"
_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE = "native-amx-v2-grouped"
_SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE = "sumeragi-v2-sdk-diagnostics"
_SDK_SOURCE_CLOSURE_SUITES = frozenset(
    {
        _NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE,
        _SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE,
    }
)




class ReceiptError(RuntimeError):
    """Release evidence is missing, ambiguous, or cross-source."""


@dataclass(frozen=True)
class EvidenceSnapshot:
    """Stable identity and bytes for a release-evidence input."""

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
class PathContract:
    """Streaming snapshot used to detect late aggregate-evidence mutation."""

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
class DirectoryContract:
    """Opened directory identity revalidated around terminal publication."""

    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    mtime_ns: int
    ctime_ns: int


def _snapshot_contract(snapshot: EvidenceSnapshot) -> PathContract:
    """Discard retained bytes while preserving the exact opened-file identity."""
    return PathContract(
        path=snapshot.path,
        sha256=snapshot.sha256,
        device=snapshot.device,
        inode=snapshot.inode,
        mode=snapshot.mode,
        owner=snapshot.owner,
        nlink=snapshot.nlink,
        size=snapshot.size,
        mtime_ns=snapshot.mtime_ns,
        ctime_ns=snapshot.ctime_ns,
    )


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def _require_digest(value: str, name: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise ReceiptError(f"{name} must be one lowercase SHA-256 digest")
    return value


def _require_signer_fingerprint(value: str) -> str:
    if _SSH_FINGERPRINT_RE.fullmatch(value) is None:
        raise ReceiptError(
            "expected signer fingerprint must be one OpenSSH SHA256 fingerprint"
        )
    return value


def _decode_canonical_json(data: bytes, name: str) -> dict[str, Any]:
    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ReceiptError(f"{name} contains a duplicate JSON field")
            value[key] = item
        return value

    try:
        value = json.loads(
            data.decode("utf-8"), object_pairs_hook=reject_duplicate_pairs
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReceiptError(f"{name} is not canonical UTF-8 JSON") from error
    if not isinstance(value, dict) or data != _canonical_json(value):
        raise ReceiptError(f"{name} is not canonical UTF-8 JSON")
    return value


def _read_signature_archive(
    path: Path,
    name: str,
    *,
    expected_mode: int,
    maximum_bytes: int,
) -> dict[str, Any]:
    """Read one archived inode without following a link or accepting a race."""
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode):
        raise ReceiptError(f"{name} path must be resolved and non-symlinked")
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or stat.S_IMODE(before.st_mode) != expected_mode
        or before.st_nlink != 1
    ):
        raise ReceiptError(
            f"{name} must be owner-owned, singly linked, and exact mode "
            f"{expected_mode:04o}"
        )
    if before.st_size > maximum_bytes:
        raise ReceiptError(f"{name} exceeds its closed size limit")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino)
            != (before.st_dev, before.st_ino)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != expected_mode
            or opened.st_nlink != 1
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while True:
            remaining = maximum_bytes + 1 - total
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise ReceiptError(f"{name} exceeds its closed size limit")
        after = os.fstat(descriptor)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_uid",
            "st_nlink",
            "st_mode",
        )
        if any(getattr(after, field) != getattr(opened, field) for field in stable_fields):
            raise ReceiptError(f"{name} changed while it was read")
        return {
            "path": path,
            "data": b"".join(chunks),
            "device": opened.st_dev,
            "inode": opened.st_ino,
            "mode": stat.S_IMODE(opened.st_mode),
            "owner": opened.st_uid,
            "nlink": opened.st_nlink,
            "size": opened.st_size,
            "mtime_ns": opened.st_mtime_ns,
            "ctime_ns": opened.st_ctime_ns,
        }
    finally:
        os.close(descriptor)


def _read_evidence_snapshot(
    path: Path,
    name: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None = None,
    allowed_owners: set[int] | None = None,
    require_single_link: bool = True,
    executable: bool = False,
    retain_bytes: bool = True,
) -> EvidenceSnapshot | PathContract:
    """Capture one bounded regular file with closed pathname semantics."""
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise ReceiptError(f"{name} must be a resolved regular non-symlink file")
    if expected_mode is not None and stat.S_IMODE(before.st_mode) != expected_mode:
        raise ReceiptError(f"{name} must have exact mode {expected_mode:04o}")
    if allowed_owners is not None and before.st_uid not in allowed_owners:
        raise ReceiptError(f"{name} has an untrusted owner")
    if require_single_link and before.st_nlink != 1:
        raise ReceiptError(f"{name} must have exactly one hard link")
    if executable and before.st_mode & 0o111 == 0:
        raise ReceiptError(f"{name} must be executable")
    if before.st_size > maximum_bytes:
        raise ReceiptError(f"{name} exceeds its closed size limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
            or opened.st_uid != before.st_uid
            or opened.st_nlink != before.st_nlink
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        chunks: list[bytes] | None = [] if retain_bytes else None
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(
                descriptor, min(1024 * 1024, maximum_bytes + 1 - total)
            )
            if not chunk:
                break
            digest.update(chunk)
            if chunks is not None:
                chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise ReceiptError(f"{name} exceeds its closed size limit")
        after = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_uid",
            "st_nlink",
        )
        if any(getattr(after, field) != getattr(opened, field) for field in fields):
            raise ReceiptError(f"{name} changed while it was read")
        if chunks is None:
            return PathContract(
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
        assert chunks is not None
        return EvidenceSnapshot(
            path=path,
            data=b"".join(chunks),
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


def _bounded_evidence_snapshot(
    path: Path,
    name: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None = None,
    allowed_owners: set[int] | None = None,
    require_single_link: bool = True,
    executable: bool = False,
) -> EvidenceSnapshot:
    """Capture one bounded evidence file and retain the exact validated bytes."""
    try:
        snapshot = _read_evidence_snapshot(
            path,
            name,
            maximum_bytes=maximum_bytes,
            expected_mode=expected_mode,
            allowed_owners=allowed_owners,
            require_single_link=require_single_link,
            executable=executable,
        )
    except ReceiptError as error:
        if str(error) == f"{name} is unavailable":
            raise ReceiptError(f"{name} is not a regular file: {path}") from error
        raise
    if not isinstance(snapshot, EvidenceSnapshot):
        raise AssertionError("retained evidence snapshot unexpectedly omitted bytes")
    return snapshot


def _bounded_path_contract(
    path: Path,
    name: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None = None,
    allowed_owners: set[int] | None = None,
    require_single_link: bool = True,
    executable: bool = False,
) -> PathContract:
    """Hash one bounded evidence file without retaining its full contents."""
    contract = _read_evidence_snapshot(
        path,
        name,
        maximum_bytes=maximum_bytes,
        expected_mode=expected_mode,
        allowed_owners=allowed_owners,
        require_single_link=require_single_link,
        executable=executable,
        retain_bytes=False,
    )
    if not isinstance(contract, PathContract):
        raise AssertionError("streamed evidence snapshot unexpectedly retained bytes")
    return contract


def _decode_lf_text(snapshot: EvidenceSnapshot, name: str) -> str:
    """Decode one exact, bounded, NUL-free LF-only text snapshot."""
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} is not canonical LF-only text")
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error


def _tsv_fields_from_snapshot(
    snapshot: EvidenceSnapshot, name: str
) -> dict[str, str]:
    text = _decode_lf_text(snapshot, name)
    fields: dict[str, str] = {}
    for line in text[:-1].split("\n"):
        parts = line.split("\t")
        if (
            len(parts) != 2
            or not parts[0]
            or parts[0] in fields
            or "\n" in parts[1]
        ):
            raise ReceiptError(f"{name} contains malformed or duplicate fields")
        fields[parts[0]] = parts[1]
    return fields


def _release_root(path: Path) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError("release root path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise ReceiptError("release root is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
    ):
        raise ReceiptError("release root must be one resolved non-symlink directory")
    return resolved


def _signature_archives(
    *,
    release_root: Path,
    attestation_path: Path,
    transcript_path: Path,
    raw_commit_path: Path,
    cargo_lock_path: Path,
    allowed_signers_path: Path,
    revocation_path: Path,
    git_path: Path,
    ssh_keygen_path: Path,
) -> tuple[Path, dict[str, dict[str, Any]]]:
    supplied = {
        "attestation": (attestation_path, _SIGNATURE_DATA_MODE, _MAX_SIGNATURE_JSON_BYTES),
        "verify_transcript": (
            transcript_path,
            _SIGNATURE_DATA_MODE,
            _MAX_SIGNATURE_JSON_BYTES,
        ),
        "raw_commit": (raw_commit_path, _SIGNATURE_DATA_MODE, _MAX_RAW_COMMIT_BYTES),
        "cargo_lock": (cargo_lock_path, _SIGNATURE_DATA_MODE, _MAX_LOCK_BYTES),
        "ssh_allowed_signers": (
            allowed_signers_path,
            _SIGNATURE_DATA_MODE,
            _MAX_POLICY_BYTES,
        ),
        "ssh_revocation": (
            revocation_path,
            _SIGNATURE_DATA_MODE,
            _MAX_POLICY_BYTES,
        ),
        "git": (git_path, _SIGNATURE_TOOL_MODE, _MAX_TOOL_BYTES),
        "ssh_keygen": (ssh_keygen_path, _SIGNATURE_TOOL_MODE, _MAX_TOOL_BYTES),
    }
    parents: set[Path] = set()
    for label, (path, _, _) in supplied.items():
        if path.name != _SIGNATURE_ARCHIVE_NAMES[label]:
            raise ReceiptError(f"release {label} archive has the wrong exact name")
        if not path.is_absolute() or Path(os.path.abspath(path)) != path:
            raise ReceiptError(f"release {label} archive path is not absolute")
        parents.add(path.parent)
    if len(parents) != 1:
        raise ReceiptError("release signature archives do not share one directory")
    directory = next(iter(parents))
    try:
        resolved_directory = directory.resolve(strict=True)
        directory_metadata = directory.lstat()
    except OSError as error:
        raise ReceiptError("release signature archive directory is unavailable") from error
    if (
        resolved_directory != directory
        or stat.S_ISLNK(directory_metadata.st_mode)
        or not stat.S_ISDIR(directory_metadata.st_mode)
        or directory_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(directory_metadata.st_mode) != _SIGNATURE_DIRECTORY_MODE
    ):
        raise ReceiptError(
            "release signature archive directory must be owner-owned with exact mode 0700"
        )
    if release_root == directory or directory in release_root.parents or release_root in directory.parents:
        raise ReceiptError(
            "sealed release root must be external to the bootstrap archive"
        )
    archives = {
        label: _read_signature_archive(
            path,
            f"release {label} archive",
            expected_mode=mode,
            maximum_bytes=maximum,
        )
        for label, (path, mode, maximum) in supplied.items()
    }
    for label, (_, _, maximum) in supplied.items():
        archives[label]["maximum_bytes"] = maximum
        archives[label]["directory_device"] = directory_metadata.st_dev
        archives[label]["directory_inode"] = directory_metadata.st_ino
    inode_keys = {
        (archive["device"], archive["inode"]) for archive in archives.values()
    }
    if len(inode_keys) != len(archives):
        raise ReceiptError("release signature archives must be distinct inodes")
    return directory, archives


def _commit_object_id(raw_commit: bytes, hexadecimal_length: int) -> str:
    framed = b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
    if hexadecimal_length == 40:
        return hashlib.sha1(framed, usedforsecurity=False).hexdigest()
    if hexadecimal_length == 64:
        return hashlib.sha256(framed).hexdigest()
    raise ReceiptError("release identity uses an unsupported Git object format")


def _commit_headers_and_message(
    raw_commit: bytes,
) -> tuple[list[tuple[bytes, list[bytes]]], bytes]:
    raw_headers, separator, message = raw_commit.partition(b"\n\n")
    if not separator or b"\r" in raw_headers or b"\0" in raw_headers:
        raise ReceiptError("raw commit has malformed LF-only headers")
    records: list[tuple[bytes, list[bytes]]] = []
    for line in raw_headers.split(b"\n"):
        if line.startswith(b" "):
            if not records:
                raise ReceiptError("raw commit has an orphan folded header")
            records[-1][1].append(line[1:])
            continue
        key, marker, value = line.partition(b" ")
        if not marker or not key or any(byte < 0x21 or byte > 0x7E for byte in key):
            raise ReceiptError("raw commit has a malformed header")
        records.append((key, [value]))
    return records, message


def _validate_raw_commit(raw_commit: bytes, identity: dict[str, Any]) -> None:
    commit_oid = identity["head_commit"]
    if _commit_object_id(raw_commit, len(commit_oid)) != commit_oid:
        raise ReceiptError("raw commit bytes do not reproduce the candidate commit OID")
    records, message = _commit_headers_and_message(raw_commit)
    signatures = [(key, values) for key, values in records if key.startswith(b"gpgsig")]
    if len(signatures) != 1 or signatures[0][0] != b"gpgsig":
        raise ReceiptError("raw commit must contain exactly one SSH signature header")
    signature = b"\n".join(signatures[0][1])
    if any(marker in signature for marker in _UNSUPPORTED_ARMOR_MARKERS):
        raise ReceiptError("raw commit uses a non-SSH signature format")
    signature_lines = signature.split(b"\n")
    if (
        len(signature_lines) < 3
        or signature_lines[0] != _SSH_ARMOR_BEGIN
        or signature_lines[-1] != _SSH_ARMOR_END
    ):
        raise ReceiptError("raw commit lacks exact SSH signature armor")
    encoded = b"".join(signature_lines[1:-1])
    try:
        decoded_signature = base64.b64decode(encoded, validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ReceiptError("raw commit contains malformed SSH signature armor") from error
    if not decoded_signature:
        raise ReceiptError("raw commit contains empty SSH signature armor")

    trees: list[str] = []
    for key, values in records:
        if key == b"tree":
            if len(values) != 1:
                raise ReceiptError("raw commit has a folded tree header")
            try:
                trees.append(values[0].decode("ascii"))
            except UnicodeDecodeError as error:
                raise ReceiptError("raw commit has a malformed tree header") from error
    if trees != [identity["head_tree"]]:
        raise ReceiptError("raw commit tree does not match the candidate identity")
    if b"\r" in message or b"\0" in message:
        raise ReceiptError("raw commit has a malformed LF-only message")
    try:
        message_text = message.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReceiptError("raw commit message is not UTF-8") from error
    if not message_text.endswith("\n"):
        raise ReceiptError("raw commit message lacks a terminal LF")
    expected_trailers = [
        f"{_TRAILER_VERSION}: 1",
        f"{_TRAILER_MANIFEST}: {identity['workspace_source_manifest_sha256']}",
        f"{_TRAILER_LOCK}: {identity['cargo_lock_sha256']}",
    ]
    lines = message_text[:-1].split("\n")
    recognized: list[int] = []
    folded_keys = {key.casefold() for key in _TRAILER_KEYS}
    for index, line in enumerate(lines):
        key, marker, _ = line.partition(":")
        if marker and key.casefold() in folded_keys:
            recognized.append(index)
    if (
        len(lines) < 5
        or lines[-3:] != expected_trailers
        or lines[-4] != ""
        or not lines[-5]
        or recognized != list(range(len(lines) - 3, len(lines)))
    ):
        raise ReceiptError(
            "raw commit lacks the exact terminal Sumeragi v2 release trailer block"
        )


def _require_exact_json_fields(
    value: Any, fields: set[str], name: str
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ReceiptError(f"{name} fields do not match its canonical schema")
    return value


def _artifact_metadata(
    data: bytes, archive_name: str, mode: int
) -> dict[str, Any]:
    return {
        "archive_name": archive_name,
        "mode": f"{mode:04o}",
        "sha256": hashlib.sha256(data).hexdigest(),
        "size_bytes": len(data),
    }


def _protected_metadata(
    data: bytes, archive_name: str, mode: int, protected_sha256: str
) -> dict[str, Any]:
    observed = hashlib.sha256(data).hexdigest()
    return {
        "archive_name": archive_name,
        "mode": f"{mode:04o}",
        "observed_sha256": observed,
        "protected_sha256": protected_sha256,
        "size_bytes": len(data),
    }


def _validate_allowed_signers_policy(data: bytes) -> None:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReceiptError("archived SSH allowed-signers policy is not UTF-8") from error
    if "\r" in text or "\0" in text or not text.endswith("\n"):
        raise ReceiptError("archived SSH allowed-signers policy is not LF-only text")
    active = [
        line
        for line in text.splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if len(active) != 1:
        raise ReceiptError(
            "archived SSH allowed-signers policy must have exactly one active line"
        )
    folded = active[0].casefold()
    if "cert-authority" in folded or "-cert-v01@openssh.com" in folded:
        raise ReceiptError("SSH certificate-authority policies are not accepted in v1")
    if re.search(
        r"(?<![a-z0-9_-])valid-(?:after|before)(?==|[,\s])", folded
    ):
        raise ReceiptError(
            "time-bounded SSH allowed-signers policies are not accepted in v1"
        )


def _decode_command_record(value: Any, name: str) -> dict[str, Any]:
    record = _require_exact_json_fields(
        value,
        {
            "argv",
            "replay_argv",
            "exit_status",
            "stdout_base64",
            "stdout_sha256",
            "stdout_size_bytes",
            "stderr_base64",
            "stderr_sha256",
            "stderr_size_bytes",
        },
        name,
    )
    for field in ("argv", "replay_argv"):
        argv = record[field]
        if (
            not isinstance(argv, list)
            or not argv
            or any(
                not isinstance(argument, str)
                or not argument
                or "\0" in argument
                or "\r" in argument
                or "\n" in argument
                for argument in argv
            )
        ):
            raise ReceiptError(f"{name}.{field} is not one closed argument vector")
    if type(record["exit_status"]) is not int:
        raise ReceiptError(f"{name}.exit_status is not an integer")
    for stream in ("stdout", "stderr"):
        encoded = record[f"{stream}_base64"]
        digest = record[f"{stream}_sha256"]
        size = record[f"{stream}_size_bytes"]
        if not isinstance(encoded, str):
            raise ReceiptError(f"{name}.{stream}_base64 is not text")
        try:
            data = base64.b64decode(encoded, validate=True)
        except (ValueError, base64.binascii.Error) as error:
            raise ReceiptError(f"{name}.{stream}_base64 is malformed") from error
        if base64.b64encode(data).decode("ascii") != encoded:
            raise ReceiptError(f"{name}.{stream}_base64 is not canonical")
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or hashlib.sha256(data).hexdigest() != digest
            or type(size) is not int
            or size < 0
            or size != len(data)
        ):
            raise ReceiptError(f"{name}.{stream} integrity fields do not match")
        record[f"_{stream}_bytes"] = data
    return record


def _decode_sanitized_operation(
    value: Any,
    name: str,
    *,
    operation_id: str,
    exit_status: int,
) -> dict[str, Any]:
    record = _require_exact_json_fields(
        value,
        {
            "operation_id",
            "exit_status",
            "stdout_sha256",
            "stdout_size_bytes",
            "stderr_sha256",
            "stderr_size_bytes",
        },
        name,
    )
    if (
        record["operation_id"] != operation_id
        or type(record["exit_status"]) is not int
        or record["exit_status"] != exit_status
    ):
        raise ReceiptError(f"{name} has the wrong operation binding")
    for stream in ("stdout", "stderr"):
        digest = record[f"{stream}_sha256"]
        size = record[f"{stream}_size_bytes"]
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or type(size) is not int
            or size < 0
            or size > _MAX_SIGNATURE_JSON_BYTES
        ):
            raise ReceiptError(f"{name} has invalid {stream} metadata")
    return record


def _operation_matches_result(
    operation: dict[str, Any],
    status: int,
    stdout: bytes,
    stderr: bytes,
) -> bool:
    return (
        operation["exit_status"] == status
        and operation["stdout_sha256"] == hashlib.sha256(stdout).hexdigest()
        and operation["stdout_size_bytes"] == len(stdout)
        and operation["stderr_sha256"] == hashlib.sha256(stderr).hexdigest()
        and operation["stderr_size_bytes"] == len(stderr)
    )


def _verification_metadata(data: bytes, name: str) -> tuple[str, str, str, str]:
    if not data.endswith(b"\0\n"):
        raise ReceiptError(f"{name} has malformed Git signature metadata")
    fields = data[:-2].split(b"\0")
    if len(fields) != 4:
        raise ReceiptError(f"{name} has malformed Git signature metadata")
    try:
        decoded = tuple(field.decode("utf-8") for field in fields)
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} has non-UTF-8 Git signature metadata") from error
    if any(
        any(ord(character) < 0x20 or ord(character) == 0x7F for character in field)
        for field in decoded
    ):
        raise ReceiptError(f"{name} has control characters in signature metadata")
    status, fingerprint, primary_fingerprint, signer = decoded
    if (
        status != "G"
        or _SSH_FINGERPRINT_RE.fullmatch(fingerprint) is None
        or primary_fingerprint
        or not signer
    ):
        raise ReceiptError(f"{name} is not one trusted SSH signature result")
    return status, fingerprint, primary_fingerprint, signer


def _historical_stage_path(
    value: str, directory: Path, archive_name: str, name: str
) -> Path:
    path = Path(value)
    expected_name = re.compile(
        rf"\.{re.escape(archive_name)}\.stage\.[0-9a-f]{{32}}"
    )
    if (
        not path.is_absolute()
        or path.parent != directory
        or expected_name.fullmatch(path.name) is None
        or os.path.lexists(path)
    ):
        raise ReceiptError(f"{name} is not one retired helper staging path")
    return path


def _signature_config(
    ssh_keygen: str, allowed_signers: str, revocation_file: str
) -> list[str]:
    return [
        "-c",
        "gpg.format=ssh",
        "-c",
        "gpg.minTrustLevel=fully",
        "-c",
        f"gpg.ssh.program={ssh_keygen}",
        "-c",
        f"gpg.ssh.allowedSignersFile={allowed_signers}",
        "-c",
        f"gpg.ssh.revocationFile={revocation_file}",
        "-c",
        f"gpg.program={ssh_keygen}",
        "-c",
        f"gpg.openpgp.program={ssh_keygen}",
        "-c",
        f"gpg.x509.program={ssh_keygen}",
    ]


def _require_config_path(values: Any, key: str, name: str) -> str:
    if not isinstance(values, list) or len(values) % 2 != 0:
        raise ReceiptError(f"{name} policy vector is malformed")
    assignments: dict[str, str] = {}
    for index in range(0, len(values), 2):
        assignment = values[index + 1]
        if values[index] != "-c" or not isinstance(assignment, str):
            raise ReceiptError(f"{name} policy vector is malformed")
        assignment_key, marker, assignment_value = assignment.partition("=")
        if (
            not marker
            or not assignment_key
            or not assignment_value
            or "\0" in assignment_value
            or assignment_key in assignments
        ):
            raise ReceiptError(f"{name} policy vector is malformed")
        assignments[assignment_key] = assignment_value
    value = assignments.get(key)
    if value is None:
        raise ReceiptError(f"{name} policy vector omits {key}")
    return value


def _closed_replay_environment(directory: Path) -> dict[str, str]:
    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": str(directory),
        "LANG": "C",
        "LANGUAGE": "C",
        "LC_ALL": "C",
        "PATH": os.defpath,
        "TZ": "UTC",
        "XDG_CONFIG_HOME": str(directory),
    }
    if sys.platform == "darwin":
        environment["__CF_USER_TEXT_ENCODING"] = f"0x{os.geteuid():X}:0x1:0xE"
    return environment


def _execution_contract(
    value: EvidenceSnapshot | PathContract,
) -> PathContract:
    return value if isinstance(value, PathContract) else _snapshot_contract(value)


def _signature_archive_path_contract(archive: dict[str, Any]) -> PathContract:
    return PathContract(
        path=archive["path"],
        sha256=hashlib.sha256(archive["data"]).hexdigest(),
        device=archive["device"],
        inode=archive["inode"],
        mode=archive["mode"],
        owner=archive["owner"],
        nlink=archive["nlink"],
        size=archive["size"],
        mtime_ns=archive["mtime_ns"],
        ctime_ns=archive["ctime_ns"],
    )


def _capture_execution_inputs(
    contracts: tuple[PathContract, ...],
) -> tuple[list[int], list[DirectoryContract]]:
    descriptors: list[int] = []
    directories: dict[Path, DirectoryContract] = {}
    try:
        for index, expected in enumerate(contracts):
            current = _capture_path_contract(
                expected.path,
                f"execution input {index}",
                expected_sha256=expected.sha256,
                expected_mode=expected.mode,
                expected_owner=expected.owner,
                expected_nlink=expected.nlink,
                expected_size=expected.size,
            )
            if current != expected or (index == 0 and current.mode & 0o111 == 0):
                raise ReceiptError(
                    f"execution input {index} changed before process creation"
                )
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(expected.path, flags)
            descriptors.append(descriptor)
            opened = os.fstat(descriptor)
            if (
                (opened.st_dev, opened.st_ino)
                != (expected.device, expected.inode)
                or stat.S_IMODE(opened.st_mode) != expected.mode
                or opened.st_uid != expected.owner
                or opened.st_nlink != expected.nlink
                or opened.st_size != expected.size
                or opened.st_mtime_ns != expected.mtime_ns
                or opened.st_ctime_ns != expected.ctime_ns
            ):
                raise ReceiptError(
                    f"execution input {index} changed while pinned"
                )
            ancestors = (
                expected.path.parent,
                *tuple(expected.path.parent.parents)[:2],
            )
            for ancestor in ancestors:
                ancestor_metadata = ancestor.lstat()
                if (
                    ancestor_metadata.st_uid not in {0, os.geteuid()}
                    or stat.S_IMODE(ancestor_metadata.st_mode) & 0o022
                ):
                    break
                directories.setdefault(
                    ancestor,
                    _capture_directory_contract(
                        ancestor,
                        f"execution input {index} ancestor",
                    ),
                )
    except BaseException:
        for descriptor in descriptors:
            try:
                os.close(descriptor)
            except OSError:
                pass
        raise
    return descriptors, list(directories.values())


def _revalidate_execution_inputs(
    contracts: tuple[PathContract, ...],
    descriptors: list[int],
    directories: list[DirectoryContract],
) -> None:
    for index, (expected, descriptor) in enumerate(zip(contracts, descriptors)):
        held = os.fstat(descriptor)
        if (
            (held.st_dev, held.st_ino) != (expected.device, expected.inode)
            or stat.S_IMODE(held.st_mode) != expected.mode
            or held.st_uid != expected.owner
            or held.st_nlink != expected.nlink
            or held.st_size != expected.size
            or held.st_mtime_ns != expected.mtime_ns
            or held.st_ctime_ns != expected.ctime_ns
        ):
            raise ReceiptError(f"execution input {index} changed while pinned")
        current = _capture_path_contract(
            expected.path,
            f"execution input {index}",
            expected_sha256=expected.sha256,
            expected_mode=expected.mode,
            expected_owner=expected.owner,
            expected_nlink=expected.nlink,
            expected_size=expected.size,
        )
        if current != expected:
            raise ReceiptError(
                f"execution input {index} changed during process execution"
            )
    for index, expected in enumerate(directories):
        current = _capture_directory_contract(
            expected.path, f"execution input ancestor {index}"
        )
        if current != expected:
            raise ReceiptError(
                f"execution input ancestor {index} changed during process execution"
            )


def _run_bounded_replay(
    executable: Path,
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    name: str = "archived Git replay",
    maximum_output_bytes: int = _MAX_REPLAY_OUTPUT_BYTES,
    executable_contract: EvidenceSnapshot | PathContract | None = None,
    watched_contracts: tuple[EvidenceSnapshot | PathContract, ...] = (),
    stdin_data: bytes | None = None,
) -> tuple[int, bytes, bytes]:
    if executable_contract is None:
        executable_contract = _bounded_path_contract(
            executable,
            f"{name} executable",
            maximum_bytes=_MAX_TOOL_BYTES,
            allowed_owners={0, os.geteuid()},
            require_single_link=False,
            executable=True,
        )
    normalized_contracts: list[PathContract] = [
        _execution_contract(executable_contract),
        *(_execution_contract(item) for item in watched_contracts),
    ]
    by_path: dict[Path, PathContract] = {}
    for contract in normalized_contracts:
        previous = by_path.get(contract.path)
        if previous is not None and previous != contract:
            raise ReceiptError(f"{name} has conflicting execution input contracts")
        by_path[contract.path] = contract
    contracts = tuple(by_path.values())
    if contracts[0].path != executable:
        raise ReceiptError(f"{name} executable path does not match its contract")
    descriptors, directory_contracts = _capture_execution_inputs(contracts)
    try:
        selector = selectors.DefaultSelector()
        deadline = time.monotonic() + _REPLAY_TIMEOUT_SECONDS
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        # Bounds determine the eventual verdict; they never control the child.
        # Retain only the capped prefix while draining both streams through EOF.
        retained_output_bytes = 0
        output_limit_exceeded = False
        runtime_limit_exceeded = False
        pending_violation: BaseException | None = None

        def latch(violation: BaseException) -> None:
            nonlocal pending_violation
            if pending_violation is None:
                pending_violation = violation

        def register_stream(
            stream: BinaryIO,
            events: int,
            data: tuple[str, BinaryIO],
        ) -> None:
            while True:
                descriptor: int | None = None
                try:
                    descriptor = stream.fileno()
                    os.set_blocking(descriptor, False)
                    selector.register(descriptor, events, data)
                    return
                except BaseException as error:
                    latch(error)
                    if descriptor is not None:
                        try:
                            selector.get_key(descriptor)
                        except KeyError:
                            pass
                        except BaseException as lookup_error:
                            latch(lookup_error)
                        else:
                            return

        try:
            process = subprocess.Popen(
                (str(executable), *arguments),
                cwd=cwd,
                env=environment,
                stdin=(
                    subprocess.PIPE
                    if stdin_data is not None
                    else subprocess.DEVNULL
                ),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
            )
        except OSError as error:
            selector.close()
            raise ReceiptError(f"{name} could not be started") from error
        output_streams = tuple(
            (stream, label)
            for stream, label in (
                (process.stdout, "stdout"),
                (process.stderr, "stderr"),
            )
            if stream is not None
        )
        if len(output_streams) != 2:
            latch(ReceiptError(f"{name} output pipes are unavailable"))
        for stream, label in output_streams:
            register_stream(
                stream,
                selectors.EVENT_READ,
                (label, stream),
            )
        stdin_offset = 0
        if process.stdin is not None:
            if stdin_data:
                register_stream(
                    process.stdin,
                    selectors.EVENT_WRITE,
                    ("stdin", process.stdin),
                )
            else:
                while True:
                    try:
                        process.stdin.close()
                        break
                    except BaseException as error:
                        latch(error)
        while True:
            try:
                if not selector.get_map():
                    break
                remaining = deadline - time.monotonic()
                if remaining <= 0 and not runtime_limit_exceeded:
                    runtime_limit_exceeded = True
                    latch(ReceiptError(f"{name} exceeded its timeout"))
                events = selector.select(
                    0.25 if runtime_limit_exceeded else min(remaining, 0.25)
                )
            except BaseException as error:
                latch(error)
                continue
            for key, _ in events:
                stream_name, stream = key.data
                if stream_name == "stdin":
                    assert stdin_data is not None
                    try:
                        written = os.write(
                            key.fd,
                            stdin_data[
                                stdin_offset : stdin_offset + 64 * 1024
                            ],
                        )
                    except BlockingIOError:
                        continue
                    except BrokenPipeError:
                        try:
                            selector.unregister(key.fd)
                        except BaseException as error:
                            latch(error)
                            continue
                        try:
                            stream.close()
                        except BaseException as error:
                            latch(error)
                        latch(
                            ReceiptError(
                                f"{name} cancelled its bounded stdin replay"
                            )
                        )
                        continue
                    except BaseException as error:
                        latch(error)
                        continue
                    if written <= 0:
                        try:
                            selector.unregister(key.fd)
                        except BaseException as error:
                            latch(error)
                            continue
                        try:
                            stream.close()
                        except BaseException as error:
                            latch(error)
                        latch(
                            ReceiptError(
                                f"{name} stdin write made no progress"
                            )
                        )
                        continue
                    stdin_offset += written
                    if stdin_offset == len(stdin_data):
                        try:
                            selector.unregister(key.fd)
                        except BaseException as error:
                            latch(error)
                            continue
                        try:
                            stream.close()
                        except BaseException as error:
                            latch(error)
                    continue
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                except BaseException as error:
                    latch(error)
                    continue
                if not chunk:
                    try:
                        selector.unregister(key.fd)
                    except BaseException as error:
                        latch(error)
                    continue
                try:
                    retained_capacity = max(
                        maximum_output_bytes - retained_output_bytes, 0
                    )
                    retained = chunk[:retained_capacity]
                    buffers[stream_name].extend(retained)
                    retained_output_bytes += len(retained)
                    if (
                        len(retained) != len(chunk)
                        and not output_limit_exceeded
                    ):
                        output_limit_exceeded = True
                        latch(
                            ReceiptError(
                                f"{name} output exceeds its closed limit"
                            )
                        )
                except BaseException as error:
                    latch(error)
        while True:
            try:
                status = process.wait()
                break
            except BaseException as error:
                latch(error)
        try:
            if time.monotonic() > deadline and not runtime_limit_exceeded:
                runtime_limit_exceeded = True
                latch(ReceiptError(f"{name} exceeded its timeout"))
        except BaseException as error:
            latch(error)
        try:
            selector.close()
        except BaseException as error:
            latch(error)
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None and not stream.closed:
                try:
                    stream.close()
                except BaseException as error:
                    latch(error)
        try:
            _revalidate_execution_inputs(
                contracts, descriptors, directory_contracts
            )
        except BaseException as error:
            latch(error)
        if pending_violation is not None:
            raise pending_violation
        return status, bytes(buffers["stdout"]), bytes(buffers["stderr"])
    finally:
        for descriptor in descriptors:
            try:
                os.close(descriptor)
            except OSError:
                pass


def _run_bounded_python_validator(
    checker: Path,
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    name: str,
    maximum_output_bytes: int = _MAX_REPLAY_OUTPUT_BYTES,
    watched_contracts: tuple[EvidenceSnapshot | PathContract, ...] = (),
) -> tuple[int, bytes, bytes]:
    checker_snapshot = _bounded_evidence_snapshot(
        checker,
        f"{name} source",
        maximum_bytes=_MAX_HELPER_BYTES,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    interpreter = Path(sys.executable).resolve(strict=True)
    interpreter_contract = _bounded_path_contract(
        interpreter,
        f"{name} Python interpreter",
        maximum_bytes=_MAX_TOOL_BYTES,
        allowed_owners={0, os.geteuid()},
        require_single_link=False,
        executable=True,
    )
    loader = (
        "import os,sys\n"
        "path=sys.argv[1]\n"
        "source=sys.stdin.buffer.read()\n"
        "sys.argv=[path,*sys.argv[2:]]\n"
        "sys.path[0]=os.path.dirname(path)\n"
        "scope={'__name__':'__main__','__file__':path,"
        "'__package__':None,'__cached__':None}\n"
        "exec(compile(source,path,'exec'),scope,scope)\n"
    )
    return _run_bounded_replay(
        interpreter,
        ["-I", "-S", "-c", loader, str(checker), *arguments],
        cwd=cwd,
        environment=environment,
        name=name,
        maximum_output_bytes=maximum_output_bytes,
        executable_contract=interpreter_contract,
        watched_contracts=(checker_snapshot, *watched_contracts),
        stdin_data=checker_snapshot.data,
    )


def _run_required_replay(
    git: Path,
    arguments: list[str],
    *,
    root: Path,
    environment: dict[str, str],
    name: str,
    executable_contract: EvidenceSnapshot | PathContract,
) -> tuple[bytes, bytes]:
    status, stdout, stderr = _run_bounded_replay(
        git,
        arguments,
        cwd=root,
        environment=environment,
        name=f"archived Git {name}",
        executable_contract=executable_contract,
    )
    if status != 0:
        raise ReceiptError(f"archived Git rejected {name}")
    return stdout, stderr


def _validate_tool_metadata(
    value: Any,
    *,
    name: str,
    archive: dict[str, Any],
    archive_name: str,
    protected_sha256: str,
) -> dict[str, Any]:
    record = _require_exact_json_fields(
        value,
        {
            "archive_name",
            "mode",
            "observed_sha256",
            "protected_sha256",
            "size_bytes",
            "source_path",
        },
        name,
    )
    data = archive["data"]
    expected = {
        "archive_name": archive_name,
        "mode": f"{_SIGNATURE_TOOL_MODE:04o}",
        "observed_sha256": hashlib.sha256(data).hexdigest(),
        "protected_sha256": protected_sha256,
        "size_bytes": len(data),
    }
    if type(record["size_bytes"]) is not int or any(
        record.get(field) != expected_value for field, expected_value in expected.items()
    ):
        raise ReceiptError(f"{name} does not match its protected archived tool")
    source_path = record["source_path"]
    if (
        not isinstance(source_path, str)
        or not source_path
        or "\0" in source_path
        or "\r" in source_path
        or "\n" in source_path
        or not Path(source_path).is_absolute()
        or Path(os.path.abspath(source_path)) != Path(source_path)
    ):
        raise ReceiptError(f"{name}.source_path is not one absolute normalized path")
    return record


def _validate_signature_evidence(
    *,
    candidate_snapshot: EvidenceSnapshot,
    candidate: dict[str, Any],
    release_root_path: Path,
    signature_attestation_path: Path,
    signature_transcript_path: Path,
    signature_raw_commit_path: Path,
    signature_cargo_lock_path: Path,
    signature_allowed_signers_path: Path,
    signature_revocation_path: Path,
    signature_git_path: Path,
    signature_ssh_keygen_path: Path,
    expected_git_sha256: str,
    expected_ssh_keygen_sha256: str,
    expected_allowed_signers_sha256: str,
    expected_revocation_sha256: str,
    expected_signer_fingerprint: str,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    """Validate, structurally replay, and cryptographically replay SSH evidence."""

    protected_git = _require_digest(expected_git_sha256, "expected Git digest")
    protected_ssh = _require_digest(
        expected_ssh_keygen_sha256, "expected ssh-keygen digest"
    )
    protected_allowed = _require_digest(
        expected_allowed_signers_sha256, "expected allowed-signers digest"
    )
    protected_revocation = _require_digest(
        expected_revocation_sha256, "expected revocation-policy digest"
    )
    protected_fingerprint = _require_signer_fingerprint(expected_signer_fingerprint)
    root = _release_root(release_root_path)
    directory, archives = _signature_archives(
        release_root=root,
        attestation_path=signature_attestation_path,
        transcript_path=signature_transcript_path,
        raw_commit_path=signature_raw_commit_path,
        cargo_lock_path=signature_cargo_lock_path,
        allowed_signers_path=signature_allowed_signers_path,
        revocation_path=signature_revocation_path,
        git_path=signature_git_path,
        ssh_keygen_path=signature_ssh_keygen_path,
    )
    archive_digests = {
        label: hashlib.sha256(archive["data"]).hexdigest()
        for label, archive in archives.items()
    }
    for label, expected in (
        ("git", protected_git),
        ("ssh_keygen", protected_ssh),
        ("ssh_allowed_signers", protected_allowed),
        ("ssh_revocation", protected_revocation),
    ):
        if archive_digests[label] != expected:
            raise ReceiptError(
                f"release {label} archive does not match its out-of-band digest"
            )
    _validate_allowed_signers_policy(archives["ssh_allowed_signers"]["data"])
    if archive_digests["cargo_lock"] != candidate["cargo_lock_sha256"]:
        raise ReceiptError("archived Cargo.lock does not match the candidate identity")
    root_lock = _bounded_evidence_snapshot(
        root / "Cargo.lock",
        "release-root Cargo.lock",
        maximum_bytes=_MAX_LOCK_BYTES,
    )
    if (
        root_lock.size != len(archives["cargo_lock"]["data"])
        or root_lock.sha256 != archive_digests["cargo_lock"]
    ):
        raise ReceiptError("release-root Cargo.lock does not match its archive")

    if candidate_snapshot.data != _canonical_json(candidate):
        raise ReceiptError("candidate identity bytes are not canonical")
    attestation = _decode_canonical_json(
        archives["attestation"]["data"], "release signature attestation"
    )
    _require_exact_json_fields(
        attestation,
        {"format", "schema_version", "candidate", "archives"},
        "release signature attestation",
    )
    if (
        attestation["format"] != _SIGNATURE_ATTESTATION_FORMAT
        or type(attestation["schema_version"]) is not int
        or attestation["schema_version"] != 3
    ):
        raise ReceiptError("release signature attestation has the wrong schema version")
    attested_candidate = _require_exact_json_fields(
        attestation["candidate"],
        {
            "commit_oid",
            "tree_oid",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "release_identity_sha256",
        },
        "release signature attestation candidate",
    )
    if attested_candidate != {
        "commit_oid": candidate["head_commit"],
        "tree_oid": candidate["head_tree"],
        "source_manifest_sha256": candidate[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": candidate["cargo_lock_sha256"],
        "release_identity_sha256": candidate_snapshot.sha256,
    }:
        raise ReceiptError("release signature attestation is not bound to exact candidate bytes")

    evidence_records = _require_exact_json_fields(
        attestation["archives"],
        set(_SIGNATURE_ARCHIVE_IDS),
        "release signature attestation evidence",
    )
    for label, archive_id in _SIGNATURE_ARCHIVE_IDS.items():
        mode = (
            _SIGNATURE_TOOL_MODE if label in {"git", "ssh_keygen"} else _SIGNATURE_DATA_MODE
        )
        expected_record = {
            "archive_id": archive_id,
            "mode": f"{mode:04o}",
            "sha256": archive_digests[label],
            "size_bytes": len(archives[label]["data"]),
        }
        record = _require_exact_json_fields(
            evidence_records[label],
            {"archive_id", "mode", "sha256", "size_bytes"},
            f"release signature attestation evidence for {label}",
        )
        if type(record["size_bytes"]) is not int or record != expected_record:
            raise ReceiptError(
                f"release signature attestation evidence for {label} is not exact"
            )

    transcript = _decode_canonical_json(
        archives["verify_transcript"]["data"], "release signature transcript"
    )
    _require_exact_json_fields(
        transcript,
        {"format", "schema_version", "archive_ids", "candidate_commit_oid", "operations"},
        "release signature transcript",
    )
    if (
        transcript["format"] != _SIGNATURE_TRANSCRIPT_FORMAT
        or type(transcript["schema_version"]) is not int
        or transcript["schema_version"] != 3
    ):
        raise ReceiptError("release signature transcript has the wrong schema version")
    if transcript["archive_ids"] != _SIGNATURE_ARCHIVE_IDS:
        raise ReceiptError("release signature transcript archive mapping is not exact")
    if transcript["candidate_commit_oid"] != candidate["head_commit"]:
        raise ReceiptError("release signature transcript does not use the immutable candidate OID")
    expected_environment = _closed_replay_environment(directory)
    operations = _require_exact_json_fields(
        transcript["operations"],
        {"show_signature_metadata", "verify_commit", "ssh_keygen_usage"},
        "release signature transcript operations",
    )
    verify_record = _decode_sanitized_operation(
        operations["verify_commit"],
        "release verify-commit operation",
        operation_id="git.verify-commit.ssh.v1",
        exit_status=0,
    )
    show_record = _decode_sanitized_operation(
        operations["show_signature_metadata"],
        "release signature-metadata operation",
        operation_id="git.show-signature-metadata.ssh.v1",
        exit_status=0,
    )
    ssh_probe = _decode_sanitized_operation(
        operations["ssh_keygen_usage"],
        "release ssh-keygen operation",
        operation_id="ssh-keygen.usage-probe.v1",
        exit_status=1,
    )

    raw_commit = archives["raw_commit"]["data"]
    _validate_raw_commit(raw_commit, candidate)
    git = archives["git"]["path"]
    git_contract = _signature_archive_path_contract(archives["git"])
    actual_config = _signature_config(
        str(archives["ssh_keygen"]["path"]),
        str(archives["ssh_allowed_signers"]["path"]),
        str(archives["ssh_revocation"]["path"]),
    )

    def require_ascii_line(arguments: list[str], name: str) -> str:
        stdout, stderr = _run_required_replay(
            git,
            arguments,
            root=root,
            environment=expected_environment,
            name=name,
            executable_contract=git_contract,
        )
        if (
            stderr
            or not stdout.endswith(b"\n")
            or b"\n" in stdout[:-1]
            or b"\r" in stdout
            or b"\0" in stdout
        ):
            raise ReceiptError(f"archived Git returned malformed {name}")
        try:
            return stdout[:-1].decode("ascii")
        except UnicodeDecodeError as error:
            raise ReceiptError(f"archived Git returned malformed {name}") from error

    if require_ascii_line(["rev-parse", "--show-toplevel"], "top-level path") != str(root):
        raise ReceiptError("release root is not the archived Git exact top-level")
    if (
        require_ascii_line(
            ["rev-parse", "--verify", "HEAD^{commit}"], "HEAD commit"
        )
        != candidate["head_commit"]
        or require_ascii_line(
            [
                "rev-parse",
                "--verify",
                f"{candidate['head_commit']}^{{tree}}",
            ],
            "candidate tree",
        )
        != candidate["head_tree"]
    ):
        raise ReceiptError("release root HEAD/tree does not match the candidate identity")
    replay_raw, replay_raw_stderr = _run_required_replay(
        git,
        ["cat-file", "commit", candidate["head_commit"]],
        root=root,
        environment=expected_environment,
        name="immutable raw commit",
        executable_contract=git_contract,
    )
    if replay_raw_stderr or replay_raw != raw_commit:
        raise ReceiptError("archived Git raw commit replay does not match its archive")
    verify_status, verify_stdout, verify_stderr = _run_bounded_replay(
        git,
        [*actual_config, "verify-commit", "--raw", candidate["head_commit"]],
        cwd=root,
        environment=expected_environment,
        executable_contract=git_contract,
    )
    if verify_status != 0:
        raise ReceiptError("archived Git cryptographic signature replay failed")
    if not _operation_matches_result(
        verify_record, verify_status, verify_stdout, verify_stderr
    ):
        raise ReceiptError(
            "archived Git verify-commit replay disagrees with sanitized transcript"
        )
    replay_show, replay_show_stderr = _run_required_replay(
        git,
        [
            *actual_config,
            "show",
            "--no-patch",
            "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
            candidate["head_commit"],
        ],
        root=root,
        environment=expected_environment,
        name="signature metadata replay",
        executable_contract=git_contract,
    )
    if replay_show_stderr:
        raise ReceiptError("archived Git signature metadata replay wrote stderr")
    replay_metadata = _verification_metadata(
        replay_show, "archived Git signature metadata replay"
    )
    if (
        replay_metadata[1] != protected_fingerprint
        or not _operation_matches_result(
            show_record, 0, replay_show, replay_show_stderr
        )
    ):
        raise ReceiptError(
            "archived Git signature replay disagrees with protected policy or transcript"
        )
    ssh_probe_status, ssh_probe_stdout, ssh_probe_stderr = _run_bounded_replay(
        archives["ssh_keygen"]["path"],
        ["-?"],
        cwd=directory,
        environment=expected_environment,
        name="archived ssh-keygen usage probe",
        executable_contract=_signature_archive_path_contract(
            archives["ssh_keygen"]
        ),
    )
    if not _operation_matches_result(
        ssh_probe, ssh_probe_status, ssh_probe_stdout, ssh_probe_stderr
    ):
        raise ReceiptError(
            "archived ssh-keygen replay disagrees with sanitized transcript"
        )
    if (
        require_ascii_line(
            ["rev-parse", "--verify", "HEAD^{commit}"], "final HEAD commit"
        )
        != candidate["head_commit"]
        or require_ascii_line(
            [
                "rev-parse",
                "--verify",
                f"{candidate['head_commit']}^{{tree}}",
            ],
            "final candidate tree",
        )
        != candidate["head_tree"]
        or require_ascii_line(["rev-parse", "--show-toplevel"], "final top-level path")
        != str(root)
    ):
        raise ReceiptError("release root identity changed during signature replay")

    # Re-read every prerequisite after executing the archived tools. The helper
    # publishes the attestation last, but the aggregate receipt must independently
    # reject post-publication inode swaps or content drift.
    for label, archive in archives.items():
        current = _read_signature_archive(
            archive["path"],
            f"release {label} archive",
            expected_mode=archive["mode"],
            maximum_bytes=archive["maximum_bytes"],
        )
        stable_fields = (
            "device",
            "inode",
            "mode",
            "owner",
            "nlink",
            "size",
            "mtime_ns",
            "ctime_ns",
            "data",
        )
        if any(current[field] != archive[field] for field in stable_fields):
            raise ReceiptError(f"release {label} archive changed during replay")
    final_directory = directory.lstat()
    first_archive = archives["attestation"]
    if (
        stat.S_ISLNK(final_directory.st_mode)
        or not stat.S_ISDIR(final_directory.st_mode)
        or final_directory.st_uid != os.geteuid()
        or stat.S_IMODE(final_directory.st_mode) != _SIGNATURE_DIRECTORY_MODE
        or (final_directory.st_dev, final_directory.st_ino)
        != (
            first_archive["directory_device"],
            first_archive["directory_inode"],
        )
    ):
        raise ReceiptError("release signature archive directory changed during replay")

    authentication = {
        "schema_version": 1,
        "signature_format": "ssh",
        "verification_status": "G",
        "candidate_commit_oid": candidate["head_commit"],
        "candidate_tree_oid": candidate["head_tree"],
        "signer_fingerprint": protected_fingerprint,
        "primary_key_fingerprint": "",
        "allowed_signers_principal": replay_metadata[3],
        "trust_policy": {
            "git_sha256": protected_git,
            "ssh_keygen_sha256": protected_ssh,
            "allowed_signers_sha256": protected_allowed,
            "revocation_sha256": protected_revocation,
            "signer_fingerprint": protected_fingerprint,
        },
        "replay": {
            "performed": True,
            "archive_ids": dict(_SIGNATURE_ARCHIVE_IDS),
        },
    }
    receipt_archives = {
        label: {
            "path": str(archive["path"]),
            "sha256": archive_digests[label],
            "size_bytes": len(archive["data"]),
            "mode": f"{archive['mode']:04o}",
            "owner_uid": archive["owner"],
            "nlink": archive["nlink"],
        }
        for label, archive in archives.items()
    }
    return authentication, receipt_archives


def _private_evidence_directory(path: Path, name: str) -> tuple[Path, os.stat_result]:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} must be an absolute normalized path")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise ReceiptError(f"{name} must be owner-owned with exact mode 0700")
    return path, metadata


def _snapshot_receipt_artifact(
    snapshot: EvidenceSnapshot | PathContract,
) -> dict[str, Any]:
    return {
        "path": str(snapshot.path),
        "sha256": snapshot.sha256,
        "size_bytes": snapshot.size,
        "mode": f"{snapshot.mode:04o}",
        "owner_uid": snapshot.owner,
        "nlink": snapshot.nlink,
    }


def _octal_mode(value: Any, name: str) -> int:
    if not isinstance(value, str) or re.fullmatch(r"[0-7]{4}", value) is None:
        raise ReceiptError(f"{name} is not one canonical four-digit mode")
    return int(value, 8)


def _bootstrap_identity_archive_names() -> dict[str, str]:
    return {
        label: archive_name
        for label, (archive_name, _) in _BOOTSTRAP_IDENTITY_ARCHIVES.items()
        if label not in {"identity_attestation", "identity_transcript"}
    }


def _validate_legacy_bootstrap_identity_documents(
    *,
    directory: Path,
    identity: dict[str, Any],
    identity_snapshot: EvidenceSnapshot,
    snapshots: dict[str, EvidenceSnapshot],
    expected_signer_fingerprint: str,
    trusted_digests: dict[str, str],
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    attestation = _decode_canonical_json(
        snapshots["identity_attestation"].data, "bootstrap identity attestation"
    )
    transcript = _decode_canonical_json(
        snapshots["identity_transcript"].data, "bootstrap identity transcript"
    )
    _require_exact_json_fields(
        attestation,
        {
            "schema_version",
            "release_identity",
            "release_identity_sha256",
            "tools",
            "policies",
            "verification",
            "evidence",
        },
        "bootstrap identity attestation",
    )
    if type(attestation["schema_version"]) is not int or attestation["schema_version"] != 2:
        raise ReceiptError("bootstrap identity attestation has the wrong schema version")
    if (
        attestation["release_identity"] != identity
        or attestation["release_identity_sha256"] != identity_snapshot.sha256
    ):
        raise ReceiptError("bootstrap identity attestation is not bound to exact identity bytes")
    verification = _require_exact_json_fields(
        attestation["verification"],
        {
            "status",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
        },
        "bootstrap identity verification",
    )
    if (
        verification["status"] != "G"
        or verification["signer_fingerprint"] != expected_signer_fingerprint
        or verification["primary_key_fingerprint"] != ""
        or not isinstance(verification["allowed_signers_principal"], str)
        or not verification["allowed_signers_principal"]
    ):
        raise ReceiptError("bootstrap identity verification has the wrong SSH signer")

    tools = _require_exact_json_fields(
        attestation["tools"], {"git", "ssh_keygen"}, "bootstrap identity tools"
    )
    tool_expectations = {
        "git": ("identity-git", trusted_digests["git"]),
        "ssh_keygen": ("identity-ssh-keygen", trusted_digests["ssh_keygen"]),
    }
    for label, (archive_name, digest) in tool_expectations.items():
        record = _require_exact_json_fields(
            tools[label],
            {
                "archive_name",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_path",
            },
            f"bootstrap identity {label} tool",
        )
        source_name = "git" if label == "git" else "ssh-keygen"
        expected = {
            "archive_name": archive_name,
            "mode": "0500",
            "observed_sha256": digest,
            "protected_sha256": digest,
            "size_bytes": snapshots[label].size,
            "source_path": str(directory / source_name),
        }
        if type(record["size_bytes"]) is not int or record != expected:
            raise ReceiptError(f"bootstrap identity {label} tool record is not exact")

    policies = _require_exact_json_fields(
        attestation["policies"],
        {
            "expected_signer_fingerprint",
            "signature_format",
            "ssh_allowed_signers",
            "ssh_revocation",
        },
        "bootstrap identity policies",
    )
    if (
        policies["expected_signer_fingerprint"] != expected_signer_fingerprint
        or policies["signature_format"] != "ssh"
    ):
        raise ReceiptError("bootstrap identity signature policy is not exact SSH policy")
    policy_expectations = {
        "ssh_allowed_signers": (
            "identity-allowed-signers",
            "bootstrap-allowed-signers",
            trusted_digests["allowed_signers"],
        ),
        "ssh_revocation": (
            "identity-revocation",
            "bootstrap-revocation",
            trusted_digests["revocation"],
        ),
    }
    for label, (archive_name, source_name, digest) in policy_expectations.items():
        record = _require_exact_json_fields(
            policies[label],
            {
                "archive_name",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
            },
            f"bootstrap identity {label} policy",
        )
        expected = {
            "archive_name": archive_name,
            "mode": "0400",
            "observed_sha256": digest,
            "protected_sha256": digest,
            "size_bytes": snapshots[label].size,
        }
        if type(record["size_bytes"]) is not int or record != expected:
            raise ReceiptError(f"bootstrap identity {label} policy record is not exact")
        if snapshots[label].data != snapshots[
            "trusted_" + ("allowed_signers" if label == "ssh_allowed_signers" else "revocation")
        ].data:
            raise ReceiptError(f"bootstrap identity {label} differs from trusted policy")
        if not (directory / source_name).is_file():
            raise ReceiptError(f"bootstrap identity {label} source archive is unavailable")

    evidence = _require_exact_json_fields(
        attestation["evidence"],
        set(_bootstrap_identity_archive_names()),
        "bootstrap identity attestation evidence",
    )
    for label, archive_name in _bootstrap_identity_archive_names().items():
        record = _require_exact_json_fields(
            evidence[label],
            {"archive_name", "mode", "sha256", "size_bytes"},
            f"bootstrap identity evidence {label}",
        )
        snapshot_label = "identity_transcript" if label == "verify_transcript" else label
        snapshot = snapshots[snapshot_label]
        mode = _SIGNATURE_TOOL_MODE if label in {"git", "ssh_keygen"} else _SIGNATURE_DATA_MODE
        expected = {
            "archive_name": archive_name,
            "mode": f"{mode:04o}",
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
        }
        if type(record["size_bytes"]) is not int or record != expected:
            raise ReceiptError(f"bootstrap identity evidence {label} is not exact")

    _require_exact_json_fields(
        transcript,
        {
            "schema_version",
            "archive_names",
            "candidate_commit_oid",
            "environment",
            "policy_overrides",
            "policies",
            "replay",
            "tools",
            "commands",
            "tool_probes",
        },
        "bootstrap identity transcript",
    )
    archive_names = _bootstrap_identity_archive_names()
    if (
        type(transcript["schema_version"]) is not int
        or transcript["schema_version"] != 2
        or transcript["archive_names"] != archive_names
        or transcript["candidate_commit_oid"] != identity["head_commit"]
        or transcript["tools"] != tools
        or transcript["policies"] != policies
        or transcript["environment"] != _closed_replay_environment(directory)
    ):
        raise ReceiptError("bootstrap identity transcript binding is not exact")
    commands = _require_exact_json_fields(
        transcript["commands"],
        {"show_signature_metadata", "verify_commit"},
        "bootstrap identity transcript commands",
    )
    show = _decode_command_record(
        commands["show_signature_metadata"], "bootstrap signature metadata command"
    )
    verify = _decode_command_record(
        commands["verify_commit"], "bootstrap verify-commit command"
    )
    probes = _require_exact_json_fields(
        transcript["tool_probes"],
        {"ssh_keygen_usage"},
        "bootstrap identity transcript probes",
    )
    probe = _decode_command_record(probes["ssh_keygen_usage"], "bootstrap ssh-keygen probe")
    if verify["exit_status"] != 0 or show["exit_status"] != 0 or probe["exit_status"] < 0:
        raise ReceiptError("bootstrap identity transcript records a failed command")
    historical_metadata = _verification_metadata(
        show["_stdout_bytes"], "bootstrap historical signature metadata"
    )
    if historical_metadata != (
        verification["status"],
        verification["signer_fingerprint"],
        verification["primary_key_fingerprint"],
        verification["allowed_signers_principal"],
    ):
        raise ReceiptError("bootstrap signature metadata disagrees with attestation")
    historical_git = _historical_stage_path(
        verify["argv"][0], directory, "identity-git", "bootstrap historical Git"
    )
    historical_show_git = _historical_stage_path(
        show["argv"][0],
        directory,
        "identity-git",
        "bootstrap historical metadata Git",
    )
    historical_ssh = _historical_stage_path(
        probe["argv"][0],
        directory,
        "identity-ssh-keygen",
        "bootstrap historical ssh-keygen",
    )
    if historical_show_git != historical_git:
        raise ReceiptError("bootstrap identity commands used different historical Git stages")
    historical_allowed = _historical_stage_path(
        _require_config_path(
            transcript["policy_overrides"],
            "gpg.ssh.allowedSignersFile",
            "bootstrap historical allowed-signers",
        ),
        directory,
        "identity-allowed-signers",
        "bootstrap historical allowed-signers",
    )
    historical_revocation = _historical_stage_path(
        _require_config_path(
            transcript["policy_overrides"],
            "gpg.ssh.revocationFile",
            "bootstrap historical revocation",
        ),
        directory,
        "identity-revocation",
        "bootstrap historical revocation",
    )
    expected_historical_config = _signature_config(
        str(historical_ssh), str(historical_allowed), str(historical_revocation)
    )
    if transcript["policy_overrides"] != expected_historical_config:
        raise ReceiptError("bootstrap identity historical signature policy is not canonical")
    expected_show_suffix = [
        "show",
        "--no-patch",
        "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
        identity["head_commit"],
    ]
    if (
        verify["argv"]
        != [
            str(historical_git),
            *expected_historical_config,
            "verify-commit",
            "--raw",
            identity["head_commit"],
        ]
        or show["argv"]
        != [str(historical_git), *expected_historical_config, *expected_show_suffix]
        or probe["argv"] != [str(historical_ssh), "-?"]
    ):
        raise ReceiptError("bootstrap identity historical command vectors are not canonical")
    replay = _require_exact_json_fields(
        transcript["replay"],
        {"candidate_root", "evidence_directory", "environment", "policy_overrides"},
        "bootstrap identity replay",
    )
    placeholder = "${EVIDENCE_DIRECTORY}"
    replay_environment = {
        key: value.replace(str(directory), placeholder)
        for key, value in _closed_replay_environment(directory).items()
    }
    expected_replay_config = _signature_config(
        f"{placeholder}/identity-ssh-keygen",
        f"{placeholder}/identity-allowed-signers",
        f"{placeholder}/identity-revocation",
    )
    if replay != {
        "candidate_root": "${CANDIDATE_ROOT}",
        "evidence_directory": placeholder,
        "environment": replay_environment,
        "policy_overrides": expected_replay_config,
    }:
        raise ReceiptError("bootstrap identity replay mapping is not canonical")
    if (
        verify["replay_argv"]
        != [
            f"{placeholder}/identity-git",
            *expected_replay_config,
            "verify-commit",
            "--raw",
            identity["head_commit"],
        ]
        or show["replay_argv"]
        != [
            f"{placeholder}/identity-git",
            *expected_replay_config,
            *expected_show_suffix,
        ]
        or probe["replay_argv"]
        != [f"{placeholder}/identity-ssh-keygen", "-?"]
    ):
        raise ReceiptError("bootstrap identity replay command vectors are not canonical")
    return attestation, transcript, verification


def _validate_bootstrap_identity_documents(
    *,
    directory: Path,
    identity: dict[str, Any],
    identity_snapshot: EvidenceSnapshot,
    snapshots: dict[str, EvidenceSnapshot],
    expected_signer_fingerprint: str,
    trusted_digests: dict[str, str],
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    """Validate the path-free bootstrap identity documents from archived bytes."""

    del directory
    attestation = _decode_canonical_json(
        snapshots["identity_attestation"].data, "bootstrap identity attestation"
    )
    transcript = _decode_canonical_json(
        snapshots["identity_transcript"].data, "bootstrap identity transcript"
    )
    _require_exact_json_fields(
        attestation,
        {"format", "schema_version", "candidate", "archives"},
        "bootstrap identity attestation",
    )
    if (
        attestation["format"] != _SIGNATURE_ATTESTATION_FORMAT
        or type(attestation["schema_version"]) is not int
        or attestation["schema_version"] != 3
    ):
        raise ReceiptError(
            "bootstrap identity attestation has the wrong sanitized schema"
        )
    candidate = _require_exact_json_fields(
        attestation["candidate"],
        {
            "commit_oid",
            "tree_oid",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "release_identity_sha256",
        },
        "bootstrap identity attestation candidate",
    )
    if candidate != {
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": identity_snapshot.sha256,
    }:
        raise ReceiptError(
            "bootstrap identity attestation does not bind the candidate"
        )
    archives = _require_exact_json_fields(
        attestation["archives"],
        set(_SIGNATURE_ARCHIVE_IDS),
        "bootstrap identity attestation archives",
    )
    for label, archive_id in _SIGNATURE_ARCHIVE_IDS.items():
        snapshot_label = (
            "identity_transcript" if label == "verify_transcript" else label
        )
        snapshot = snapshots[snapshot_label]
        mode = (
            _SIGNATURE_TOOL_MODE
            if label in {"git", "ssh_keygen"}
            else _SIGNATURE_DATA_MODE
        )
        record = _require_exact_json_fields(
            archives[label],
            {"archive_id", "mode", "sha256", "size_bytes"},
            f"bootstrap identity archive {label}",
        )
        if record != {
            "archive_id": archive_id,
            "mode": f"{mode:04o}",
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
        } or type(record["size_bytes"]) is not int:
            raise ReceiptError(
                f"bootstrap identity archive {label} does not match its bytes"
            )
    protected_labels = {
        "git": "git",
        "ssh_keygen": "ssh_keygen",
        "ssh_allowed_signers": "allowed_signers",
        "ssh_revocation": "revocation",
    }
    for label, trusted_label in protected_labels.items():
        trusted = snapshots["trusted_" + trusted_label]
        if (
            snapshots[label].sha256 != trusted_digests[trusted_label]
            or snapshots[label].data != trusted.data
        ):
            raise ReceiptError(
                f"bootstrap identity archive {label} differs from protected input"
            )

    _require_exact_json_fields(
        transcript,
        {"format", "schema_version", "archive_ids", "candidate_commit_oid", "operations"},
        "bootstrap identity transcript",
    )
    if (
        transcript["format"] != _SIGNATURE_TRANSCRIPT_FORMAT
        or type(transcript["schema_version"]) is not int
        or transcript["schema_version"] != 3
        or transcript["archive_ids"] != _SIGNATURE_ARCHIVE_IDS
        or transcript["candidate_commit_oid"] != identity["head_commit"]
    ):
        raise ReceiptError("bootstrap identity transcript binding is not exact")
    operations = _require_exact_json_fields(
        transcript["operations"],
        {"show_signature_metadata", "verify_commit", "ssh_keygen_usage"},
        "bootstrap identity transcript operations",
    )
    _decode_sanitized_operation(
        operations["show_signature_metadata"],
        "bootstrap signature metadata operation",
        operation_id="git.show-signature-metadata.ssh.v1",
        exit_status=0,
    )
    _decode_sanitized_operation(
        operations["verify_commit"],
        "bootstrap verify-commit operation",
        operation_id="git.verify-commit.ssh.v1",
        exit_status=0,
    )
    _decode_sanitized_operation(
        operations["ssh_keygen_usage"],
        "bootstrap ssh-keygen operation",
        operation_id="ssh-keygen.usage-probe.v1",
        exit_status=1,
    )
    try:
        active = [
            line
            for line in snapshots["ssh_allowed_signers"].data.decode(
                "utf-8"
            ).splitlines()
            if line and not line.startswith("#")
        ]
    except UnicodeDecodeError as error:
        raise ReceiptError("bootstrap allowed-signers policy is not UTF-8") from error
    if len(active) != 1 or not active[0].split():
        raise ReceiptError(
            "bootstrap allowed-signers policy has no unique principal"
        )
    verification = {
        "status": "G",
        "signer_fingerprint": expected_signer_fingerprint,
        "primary_key_fingerprint": "",
        "allowed_signers_principal": active[0].split()[0],
    }
    return attestation, transcript, verification


def _framework_runtime_projection(
    records: Any, name: str
) -> list[dict[str, Any]]:
    """Return the path-free, canonical projection of a Python runtime inventory."""

    if not isinstance(records, list):
        raise ReceiptError(f"{name} records are not a list")
    projected: list[dict[str, Any]] = []
    for record in records:
        if not isinstance(record, dict):
            raise ReceiptError(f"{name} member is malformed")
        kind = record.get("kind")
        private_keys = {
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
        public_keys = {
            "directory": {"path", "kind", "mode"},
            "file": {"path", "kind", "mode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        keys = set(record)
        if private_keys is not None and keys == private_keys:
            assert public_keys is not None
            record = {key: record[key] for key in public_keys}
            keys = set(record)
        if public_keys is None or keys != public_keys:
            raise ReceiptError(f"{name} member schema is not exact")
        relative = record.get("path")
        mode = record.get("mode")
        if (
            not isinstance(relative, str)
            or not relative
            or relative.startswith("/")
            or PurePosixPath(relative).as_posix() != relative
            or ".." in PurePosixPath(relative).parts
            or not isinstance(mode, str)
            or re.fullmatch(r"[0-7]{4}", mode) is None
        ):
            raise ReceiptError(f"{name} member path or mode is unsafe")
        if kind == "file":
            if (
                type(record["size"]) is not int
                or record["size"] < 0
                or not isinstance(record["sha256"], str)
                or _DIGEST_RE.fullmatch(record["sha256"]) is None
            ):
                raise ReceiptError(f"{name} file metadata is invalid")
        elif kind == "symlink" and (
            not isinstance(record["target"], str) or not record["target"]
        ):
            raise ReceiptError(f"{name} symlink target is invalid")
        projected.append(dict(record))
    projected.sort(key=lambda record: record["path"])
    if len(projected) > _MAX_FRAMEWORK_RUNTIME_MEMBERS or len(
        {record["path"] for record in projected}
    ) != len(projected):
        raise ReceiptError(f"{name} member inventory is not unique and bounded")
    return projected


def _validate_framework_python_runtime(
    value: Any, directory: Path
) -> list[PathContract | DirectoryContract]:
    """Authenticate every member of the loader-complete framework Python archive."""

    runtime = _require_exact_json_fields(
        value,
        {
            "format",
            "schema_version",
            "archive_root",
            "root_mode",
            "executable",
            "inventory",
            "record_count",
            "file_bytes",
            "records",
        },
        "framework Python runtime",
    )
    if (
        runtime["format"] != "iroha-sumeragi-v2-framework-python-runtime"
        or type(runtime["schema_version"]) is not int
        or runtime["schema_version"] != 1
        or runtime["archive_root"] != "python-runtime"
        or runtime["root_mode"] != "0500"
        or runtime["executable"] != "bin/python3"
    ):
        raise ReceiptError("framework Python runtime binding is not exact")
    inventory_record = _require_exact_json_fields(
        runtime["inventory"],
        {"archive_name", "mode", "sha256", "size_bytes"},
        "framework Python runtime inventory record",
    )
    if (
        inventory_record["archive_name"] != "python-runtime-input.json"
        or _octal_mode(
            inventory_record["mode"], "framework Python runtime inventory mode"
        )
        != _SIGNATURE_DATA_MODE
    ):
        raise ReceiptError("framework Python runtime inventory binding is wrong")
    inventory = _bounded_evidence_snapshot(
        directory / "python-runtime-input.json",
        "framework Python runtime inventory",
        maximum_bytes=_MAX_SIGNATURE_JSON_BYTES,
        expected_mode=_SIGNATURE_DATA_MODE,
        allowed_owners={os.geteuid()},
    )
    if (
        inventory.sha256
        != _require_digest(
            inventory_record["sha256"],
            "framework Python runtime inventory digest",
        )
        or type(inventory_record["size_bytes"]) is not int
        or inventory_record["size_bytes"] != inventory.size
    ):
        raise ReceiptError(
            "framework Python runtime inventory bytes do not match the marker"
        )
    private_inventory = _decode_canonical_json(
        inventory.data, "framework Python runtime inventory"
    )
    inventory_keys = {
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
    runtime_root = directory / "python-runtime"
    if (
        set(private_inventory) != inventory_keys
        or private_inventory["format"]
        != "iroha-sumeragi-v2-private-framework-python-runtime"
        or type(private_inventory["schema_version"]) is not int
        or private_inventory["schema_version"] != 1
        or private_inventory["runtime_root"] != str(runtime_root)
        or private_inventory["source_disclosure"] != "withheld"
        or not isinstance(private_inventory["input_records"], list)
        or type(private_inventory["input_record_count"]) is not int
        or private_inventory["input_record_count"] < 0
        or type(private_inventory["input_file_bytes"]) is not int
        or private_inventory["input_file_bytes"] < 0
    ):
        raise ReceiptError(
            "private framework Python runtime inventory contract is wrong"
        )
    expected = _framework_runtime_projection(
        runtime["records"], "framework Python runtime"
    )
    private = _framework_runtime_projection(
        private_inventory["records"], "private framework Python runtime"
    )
    if expected != private:
        raise ReceiptError(
            "framework Python marker does not bind the private member inventory"
        )
    expected_count = runtime["record_count"]
    expected_bytes = runtime["file_bytes"]
    if (
        type(expected_count) is not int
        or expected_count != len(expected)
        or type(private_inventory["record_count"]) is not int
        or private_inventory["record_count"] != expected_count
        or type(expected_bytes) is not int
        or expected_bytes
        != sum(record["size"] for record in expected if record["kind"] == "file")
        or type(private_inventory["file_bytes"]) is not int
        or private_inventory["file_bytes"] != expected_bytes
        or expected_bytes > _MAX_FRAMEWORK_RUNTIME_BYTES
    ):
        raise ReceiptError("framework Python runtime member accounting is not exact")

    try:
        root_metadata = runtime_root.lstat()
    except OSError as error:
        raise ReceiptError("framework Python runtime root is unavailable") from error
    if (
        runtime_root.resolve(strict=True) != runtime_root
        or stat.S_ISLNK(root_metadata.st_mode)
        or not stat.S_ISDIR(root_metadata.st_mode)
        or root_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(root_metadata.st_mode) != 0o500
    ):
        raise ReceiptError("framework Python runtime root metadata is not exact")

    observed: list[dict[str, Any]] = []
    contracts: list[PathContract | DirectoryContract] = [
        _snapshot_contract(inventory),
        _capture_directory_contract(runtime_root, "framework Python runtime root"),
    ]
    total_bytes = 0

    def walk(root: Path, relative_root: str = "") -> None:
        nonlocal total_bytes
        try:
            entries = tuple(sorted(os.scandir(root), key=lambda entry: entry.name))
        except OSError as error:
            raise ReceiptError("framework Python runtime directory is unreadable") from error
        if len(observed) + len(entries) > _MAX_FRAMEWORK_RUNTIME_MEMBERS:
            raise ReceiptError("framework Python runtime contains too many members")
        for entry in entries:
            name = entry.name
            relative = name if not relative_root else f"{relative_root}/{name}"
            if not name or name in {".", ".."} or "/" in name or "\0" in name:
                raise ReceiptError("framework Python runtime has an unsafe member name")
            path = root / name
            try:
                metadata = path.lstat()
            except OSError as error:
                raise ReceiptError(
                    f"framework Python runtime member is unavailable: {relative}"
                ) from error
            if metadata.st_uid != os.geteuid():
                raise ReceiptError(
                    f"framework Python runtime member has the wrong owner: {relative}"
                )
            mode = f"{stat.S_IMODE(metadata.st_mode):04o}"
            if stat.S_ISDIR(metadata.st_mode):
                contract = _capture_directory_contract(
                    path, f"framework Python runtime directory {relative}"
                )
                contracts.append(contract)
                observed.append(
                    {"path": relative, "kind": "directory", "mode": mode}
                )
                walk(path, relative)
                if _capture_directory_contract(
                    path, f"framework Python runtime directory {relative}"
                ) != contract:
                    raise ReceiptError(
                        f"framework Python runtime directory changed: {relative}"
                    )
            elif stat.S_ISREG(metadata.st_mode):
                contract = _bounded_path_contract(
                    path,
                    f"framework Python runtime file {relative}",
                    maximum_bytes=_MAX_TOOL_BYTES,
                    expected_mode=stat.S_IMODE(metadata.st_mode),
                    allowed_owners={os.geteuid()},
                    require_single_link=True,
                )
                total_bytes += contract.size
                if total_bytes > _MAX_FRAMEWORK_RUNTIME_BYTES:
                    raise ReceiptError("framework Python runtime exceeds its byte bound")
                contracts.append(contract)
                observed.append(
                    {
                        "path": relative,
                        "kind": "file",
                        "mode": mode,
                        "size": contract.size,
                        "sha256": contract.sha256,
                    }
                )
            elif stat.S_ISLNK(metadata.st_mode):
                target = os.readlink(path)
                after = path.lstat()
                if (
                    not stat.S_ISLNK(after.st_mode)
                    or (after.st_dev, after.st_ino, after.st_uid, after.st_mtime_ns, after.st_ctime_ns)
                    != (
                        metadata.st_dev,
                        metadata.st_ino,
                        metadata.st_uid,
                        metadata.st_mtime_ns,
                        metadata.st_ctime_ns,
                    )
                    or os.readlink(path) != target
                ):
                    raise ReceiptError(
                        f"framework Python runtime symlink changed: {relative}"
                    )
                observed.append(
                    {
                        "path": relative,
                        "kind": "symlink",
                        "mode": mode,
                        "target": target,
                    }
                )
            else:
                raise ReceiptError(
                    f"framework Python runtime contains a special member: {relative}"
                )

    walk(runtime_root)
    observed.sort(key=lambda record: record["path"])
    if observed != expected or total_bytes != expected_bytes:
        raise ReceiptError("framework Python runtime members differ from the marker")
    by_path = {record["path"]: record for record in observed}
    stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
    required = {
        "bin": "directory",
        "bin/python3": "file",
        "Python3": "file",
        "Resources": "directory",
        "Resources/Python.app/Contents/MacOS/Python": "file",
        "lib": "directory",
        f"lib/{stdlib_name}": "directory",
        f"lib/{stdlib_name}/lib-dynload": "directory",
    }
    if (
        {PurePosixPath(path).parts[0] for path in by_path}
        != {"bin", "Python3", "Resources", "lib"}
        or any(by_path.get(path, {}).get("kind") != kind for path, kind in required.items())
    ):
        raise ReceiptError("framework Python runtime indispensable layout is incomplete")
    for relative, record in by_path.items():
        if record["kind"] != "symlink":
            continue
        target = PurePosixPath(record["target"])
        if target.is_absolute():
            raise ReceiptError(
                f"framework Python runtime symlink is absolute: {relative}"
            )
        parts = list(PurePosixPath(relative).parts[:-1])
        for part in target.parts:
            if part in {"", "."}:
                continue
            if part == "..":
                if not parts:
                    raise ReceiptError(
                        f"framework Python runtime symlink escapes: {relative}"
                    )
                parts.pop()
            else:
                parts.append(part)
        if not parts or parts[0] not in {"Python3", "Resources", "lib"}:
            raise ReceiptError(
                f"framework Python runtime symlink leaves its closure: {relative}"
            )
        for index in range(1, len(parts) + 1):
            target_path = "/".join(parts[:index])
            target_record = by_path.get(target_path)
            if (
                not isinstance(target_record, dict)
                or (index < len(parts) and target_record["kind"] != "directory")
                or (
                    index == len(parts)
                    and target_record["kind"] not in {"directory", "file"}
                )
            ):
                raise ReceiptError(
                    f"framework Python runtime symlink target is not exact: {relative}"
                )
    return contracts


def _validate_and_replay_tool_probe_closure(
    *,
    manifest_path: Path,
    result_path: Path,
    expected_value: Any | None,
    tools: dict[str, EvidenceSnapshot | PathContract],
    python: EvidenceSnapshot | PathContract,
    helper: EvidenceSnapshot | PathContract,
    archive_id_prefix: str,
    probe_root: Path,
) -> tuple[EvidenceSnapshot, EvidenceSnapshot, dict[str, Any]]:
    """Authenticate and independently replay one path-free 41-tool result."""

    manifest = _bounded_evidence_snapshot(
        manifest_path,
        "release tool probe manifest",
        maximum_bytes=1024 * 1024,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    result = _bounded_evidence_snapshot(
        result_path,
        "release tool probe result",
        maximum_bytes=1024 * 1024,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    manifest_value = _require_exact_json_fields(
        _decode_canonical_json(manifest.data, "release tool probe manifest"),
        {"schema_version", "tools"},
        "release tool probe manifest",
    )
    manifest_tools = manifest_value["tools"]
    result_value = _require_exact_json_fields(
        _decode_canonical_json(result.data, "release tool probe result"),
        {
            "format",
            "host_family",
            "probe_contract_sha256",
            "schema_version",
            "tool_count",
            "tools",
        },
        "release tool probe result",
    )
    results = result_value["tools"]
    expected_host = "darwin" if sys.platform == "darwin" else "linux"
    if (
        type(manifest_value["schema_version"]) is not int
        or manifest_value["schema_version"] != 1
        or not isinstance(manifest_tools, dict)
        or set(manifest_tools) != set(tools)
        or len(tools) != 41
        or result_value["format"]
        != "iroha-sumeragi-v2-release-tool-functional-probes"
        or type(result_value["schema_version"]) is not int
        or result_value["schema_version"] != 1
        or result_value["host_family"] != expected_host
        or type(result_value["tool_count"]) is not int
        or result_value["tool_count"] != 41
        or not isinstance(results, dict)
        or set(results) != set(tools)
        or (expected_value is not None and expected_value != result_value)
    ):
        raise ReceiptError("release tool probe closure is not exact")
    for name, tool in tools.items():
        manifest_record = _require_exact_json_fields(
            manifest_tools[name],
            {"archive_id", "path", "sha256"},
            f"release tool probe manifest {name}",
        )
        result_record = _require_exact_json_fields(
            results[name],
            {
                "archive_id", "exit_status", "invocation_sha256", "mode",
                "operation_id", "postcondition_sha256", "sha256",
                "size_bytes", "stderr_sha256", "stderr_size_bytes",
                "stdout_sha256", "stdout_size_bytes",
            },
            f"release tool probe result {name}",
        )
        if (
            manifest_record["archive_id"]
            != f"{archive_id_prefix}.{name}.v1"
            or manifest_record["path"] != str(tool.path)
            or manifest_record["sha256"] != tool.sha256
            or result_record["archive_id"]
            != f"{archive_id_prefix}.{name}.v1"
            or result_record["sha256"] != tool.sha256
            or result_record["mode"] != "0500"
            or type(result_record["size_bytes"]) is not int
            or result_record["size_bytes"] != tool.size
        ):
            raise ReceiptError(f"release tool probe {name} binding is wrong")
    replay_environment = _closed_replay_environment(probe_root.parent)
    replay_environment.update(
        {
            "PATH": str(python.path.parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONHASHSEED": "0",
        }
    )
    status, stdout, stderr = _run_bounded_replay(
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
        cwd=probe_root.parent,
        environment=replay_environment,
        name="release tool functional-probe replay",
        maximum_output_bytes=1024 * 1024,
        executable_contract=python,
        watched_contracts=(helper, manifest, *tools.values()),
    )
    if (
        status != 0
        or stderr
        or stdout != result.data
        or probe_root.exists()
        or probe_root.is_symlink()
    ):
        raise ReceiptError("release tool functional-probe replay failed")
    return manifest, result, result_value


def _load_release_approval_contract(snapshot: EvidenceSnapshot) -> Any:
    """Load the approval API from one authenticated bootstrap archive."""

    module_name = "_sumeragi_v2_release_approval_" + snapshot.sha256
    module = types.ModuleType(module_name)
    module.__file__ = str(snapshot.path)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        exec(compile(snapshot.data, str(snapshot.path), "exec"), module.__dict__)
    except BaseException as error:
        raise ReceiptError(
            "archived release approval contract could not be loaded"
        ) from error
    finally:
        sys.modules.pop(module_name, None)
    required = (
        "APPROVAL_ARCHIVE_IDS",
        "APPROVAL_CLASS_ORDER",
        "APPROVAL_OPERATION_PLAN_SHA256",
        "APPROVAL_SET_ARCHIVE_FORMAT",
        "ReleaseApprovalClass",
        "ReleaseApprovalError",
        "build_release_approval_expectations",
        "load_protected_release_approval_set",
        "require_release_approval_binding",
        "sanitized_release_approval_set_archive",
    )
    if any(not hasattr(module, name) for name in required):
        raise ReceiptError("archived release approval contract API is incomplete")
    if tuple(value.value for value in module.APPROVAL_CLASS_ORDER) != _APPROVAL_CLASS_IDS:
        raise ReceiptError("archived release approval class order is not exact")
    return module


def _release_approval_archive_record(
    value: Any,
    *,
    label: str,
    archive_id: str,
    archive_name: str,
    snapshot: EvidenceSnapshot,
) -> None:
    record = _require_exact_json_fields(
        value,
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        label,
    )
    if (
        record["archive_id"] != archive_id
        or record["archive_name"] != archive_name
        or _octal_mode(record["mode"], f"{label} mode")
        != _SIGNATURE_DATA_MODE
        or _require_digest(record["sha256"], f"{label} digest")
        != snapshot.sha256
        or type(record["size_bytes"]) is not int
        or record["size_bytes"] != snapshot.size
    ):
        raise ReceiptError(f"{label} archive binding is not exact")


def _validate_release_approval_evidence(
    value: Any,
    *,
    directory: Path,
    identity: dict[str, Any],
    snapshots: dict[str, EvidenceSnapshot],
) -> tuple[dict[str, Any], list[PathContract]]:
    """Replay the raw four-class approval set and path-free attestations."""

    marker = _require_exact_json_fields(
        value,
        {
            "format",
            "schema_version",
            "candidate_oid",
            "candidate_tree",
            "protected_tool_manifest_sha256",
            "evidence_root_id",
            "expected_duration_seconds",
            "operation_plan_sha256",
            "class_attestations",
            "set_attestation",
        },
        "release approvals",
    )
    module = _load_release_approval_contract(snapshots["trusted_approval_contract"])
    tool_manifest_sha256 = snapshots["trusted_runner_tool_manifest"].sha256
    if (
        marker["format"] != module.APPROVAL_SET_ARCHIVE_FORMAT
        or type(marker["schema_version"]) is not int
        or marker["schema_version"] != 1
        or marker["candidate_oid"] != identity["head_commit"]
        or marker["candidate_tree"] != identity["head_tree"]
        or marker["protected_tool_manifest_sha256"] != tool_manifest_sha256
    ):
        raise ReceiptError("release approval candidate/tool binding is not exact")
    durations = _require_exact_json_fields(
        marker["expected_duration_seconds"],
        set(_APPROVAL_CLASS_IDS),
        "release approval durations",
    )
    if any(type(item) is not int for item in durations.values()):
        raise ReceiptError("release approval durations are not exact integers")
    expected_plan_digests = {
        approval_class.value: digest
        for approval_class, digest in module.APPROVAL_OPERATION_PLAN_SHA256.items()
    }
    if marker["operation_plan_sha256"] != expected_plan_digests:
        raise ReceiptError("release approval operation plans are not exact")
    try:
        expectations = module.build_release_approval_expectations(
            candidate_oid=identity["head_commit"],
            candidate_tree=identity["head_tree"],
            protected_tool_manifest_sha256=tool_manifest_sha256,
            evidence_root_id=marker["evidence_root_id"],
            offline_toolchain_sdk_duration_seconds=durations[
                "offline-toolchain-sdk"
            ],
            formal_proof_tools_duration_seconds=durations[
                "formal-proof-tools"
            ],
            network_scale_soak_duration_seconds=durations[
                "network-scale-soak"
            ],
            final_bootstrap_publication_duration_seconds=durations[
                "final-bootstrap-publication"
            ],
        )
        approvals = module.load_protected_release_approval_set(
            {
                module.ReleaseApprovalClass(class_id): snapshots[
                    "trusted_" + _APPROVAL_INPUT_LABELS[class_id]
                ].path
                for class_id in _APPROVAL_CLASS_IDS
            },
            expectations=expectations,
            expected_owner_uid=os.geteuid(),
        )
        for approval in approvals:
            module.require_release_approval_binding(
                approval, expectations[approval.class_id]
            )
    except module.ReleaseApprovalError as error:
        raise ReceiptError(f"release approval replay failed: {error}") from error
    if {
        approval.class_id.value: len(approval.operations)
        for approval in approvals
    } != _APPROVAL_OPERATION_COUNTS:
        raise ReceiptError("release approval operation counts are not exact")
    class_records = _require_exact_json_fields(
        marker["class_attestations"],
        set(_APPROVAL_CLASS_IDS),
        "release approval class attestations",
    )
    contracts: list[PathContract] = []
    for approval in approvals:
        class_id = approval.class_id.value
        archive_name = _APPROVAL_ATTESTATION_NAMES[class_id]
        snapshot = _read_evidence_snapshot(
            directory / archive_name,
            f"sanitized release approval {class_id}",
            maximum_bytes=_MAX_HELPER_BYTES,
            expected_mode=_SIGNATURE_DATA_MODE,
            allowed_owners={os.geteuid()},
        )
        sanitized = approval.sanitized_archive()
        if snapshot.data != sanitized.canonical_bytes or snapshot.sha256 != sanitized.sha256:
            raise ReceiptError(
                f"sanitized release approval {class_id} is not exact"
            )
        _release_approval_archive_record(
            class_records[class_id],
            label=f"release approval {class_id}",
            archive_id=module.APPROVAL_ARCHIVE_IDS[approval.class_id],
            archive_name=archive_name,
            snapshot=snapshot,
        )
        contracts.append(_snapshot_contract(snapshot))
    set_snapshot = _read_evidence_snapshot(
        directory / _APPROVAL_SET_ATTESTATION_NAME,
        "sanitized release approval set",
        maximum_bytes=_MAX_HELPER_BYTES,
        expected_mode=_SIGNATURE_DATA_MODE,
        allowed_owners={os.geteuid()},
    )
    sanitized_set = module.sanitized_release_approval_set_archive(approvals)
    if (
        set_snapshot.data != sanitized_set.canonical_bytes
        or set_snapshot.sha256 != sanitized_set.sha256
    ):
        raise ReceiptError("sanitized release approval set is not exact")
    _release_approval_archive_record(
        marker["set_attestation"],
        label="release approval set",
        archive_id=_APPROVAL_SET_ARCHIVE_ID,
        archive_name=_APPROVAL_SET_ATTESTATION_NAME,
        snapshot=set_snapshot,
    )
    contracts.append(_snapshot_contract(set_snapshot))
    return marker, contracts


def _validate_bootstrap_evidence(
    *,
    completion_path: Path,
    evidence_dir_path: Path,
    identity_path: Path,
    attestation_path: Path,
    transcript_path: Path,
    expected_completion_sha256: str,
    candidate_root_path: Path,
    runner_path: Path,
    release_root_path: Path,
    candidate: dict[str, Any],
    candidate_snapshot: EvidenceSnapshot,
    sealed: dict[str, Any],
    expected_signer_fingerprint: str,
    signature_archives: dict[str, dict[str, Any]],
    runner_logs_sealed: bool,
    expected_scaling_manifest_path: Path,
    expected_scaling_trial_harness_sha256: str,
    expected_scaling_configuration_sha256: str,
    expected_scaling_irohad_sha256: str,
    expected_scaling_iroha_cli_sha256: str,
    bootstrap_private_inputs_available: bool,
) -> tuple[
    dict[str, Any],
    dict[str, Any],
    list[PathContract | DirectoryContract],
]:
    expected_marker_sha = _require_digest(
        expected_completion_sha256, "expected bootstrap completion digest"
    )
    directory, directory_stat = _private_evidence_directory(
        evidence_dir_path, "bootstrap evidence directory"
    )
    candidate_root = _release_root(candidate_root_path)
    release_root = _release_root(release_root_path)
    if release_root == directory or directory in release_root.parents or release_root in directory.parents:
        raise ReceiptError("sealed release root is not external to the bootstrap archive")
    if candidate_root == release_root:
        raise ReceiptError("bootstrap candidate and sealed release roots must be distinct")
    if (
        directory == candidate_root
        or candidate_root in directory.parents
        or directory in candidate_root.parents
    ):
        raise ReceiptError("bootstrap evidence directory must be external to candidate root")
    exact_paths = {
        "completion": (completion_path, _BOOTSTRAP_COMPLETION_NAME),
        "identity": (identity_path, "candidate-identity.json"),
        "attestation": (attestation_path, "identity-attestation.json"),
        "transcript": (transcript_path, "identity-transcript.json"),
    }
    for label, (path, basename) in exact_paths.items():
        if path != directory / basename:
            raise ReceiptError(f"bootstrap {label} path is not its exact evidence path")
    signature_directory = Path(signature_archives["attestation"]["path"]).parent
    if signature_directory != directory or any(
        Path(record["path"]).parent != directory
        for record in signature_archives.values()
    ):
        raise ReceiptError(
            "release signature artifacts must be direct bootstrap evidence children"
        )
    marker_snapshot = _read_evidence_snapshot(
        completion_path,
        "bootstrap completion marker",
        maximum_bytes=_MAX_SIGNATURE_JSON_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    if marker_snapshot.sha256 != expected_marker_sha:
        raise ReceiptError("bootstrap completion marker does not match out-of-band digest")
    identity_snapshot = _read_evidence_snapshot(
        identity_path,
        "bootstrap candidate identity",
        maximum_bytes=64 * 1024,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    bootstrap_identity = _decode_canonical_json(
        identity_snapshot.data, "bootstrap candidate identity"
    )
    if bootstrap_identity != candidate or identity_snapshot.data != _canonical_json(candidate):
        raise ReceiptError("bootstrap candidate identity differs from release candidate")
    if identity_snapshot.data != candidate_snapshot.data:
        raise ReceiptError("bootstrap and current candidate identity bytes differ")

    marker = _decode_canonical_json(marker_snapshot.data, "bootstrap completion marker")
    _require_exact_json_fields(
        marker,
        {
            "schema_version",
            "trust_boundary",
            "candidate_identity",
            "candidate_identity_sha256",
            "trusted_inputs",
            "release_approvals",
            "identity_verification",
            "runner",
            "trusted_execution_probes",
        },
        "bootstrap completion marker",
    )
    if type(marker["schema_version"]) is not int or marker["schema_version"] != 2:
        raise ReceiptError("bootstrap completion marker has the wrong schema version")
    if marker["trust_boundary"] != {
        "bootstrap_authentication": "external prerequisite",
        "release_image_and_dynamic_loader": "external prerequisite",
        "same_uid_and_trusted_ancestor_owners": True,
    } or type(marker["trust_boundary"].get("same_uid_and_trusted_ancestor_owners")) is not bool:
        raise ReceiptError("bootstrap completion marker has the wrong trust boundary")
    if (
        marker["candidate_identity"] != candidate
        or marker["candidate_identity_sha256"] != identity_snapshot.sha256
    ):
        raise ReceiptError("bootstrap completion marker has the wrong candidate identity")
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if candidate[field] != sealed[field]:
            raise ReceiptError(f"sealed independent mirror does not reproduce bootstrap {field}")

    trusted_records = _require_exact_json_fields(
        marker["trusted_inputs"],
        set(_BOOTSTRAP_TRUSTED_ARCHIVES),
        "bootstrap trusted inputs",
    )
    snapshots: dict[str, EvidenceSnapshot] = {
        "completion": marker_snapshot,
        "identity": identity_snapshot,
    }
    trusted_digests: dict[str, str] = {}
    trusted_archives: dict[str, dict[str, Any]] = {}
    evidence_inodes: set[tuple[int, int]] = {
        (marker_snapshot.device, marker_snapshot.inode),
        (identity_snapshot.device, identity_snapshot.inode),
    }
    framework_runtime_contracts: list[PathContract | DirectoryContract] = []
    framework_python = False
    executable_labels = {"python", "git", "ssh_keygen", "bash"}
    for label, (default_archive_name, archive_mode) in _BOOTSTRAP_TRUSTED_ARCHIVES.items():
        raw_record = trusted_records[label]
        framework_record = (
            label == "python"
            and isinstance(raw_record, dict)
            and "runtime" in raw_record
        )
        validator_components = (
            label == "receipt_validator"
        )
        bootstrap_components = (
            label == "bootstrap"
        )
        record = _require_exact_json_fields(
            raw_record,
            ({
                "archive_id",
                "archive_name",
                "mode",
                "sha256",
                "size_bytes",
            }
            | ({"runtime"} if framework_record else set())
            | ({"components"} if bootstrap_components else set())
            | ({"components"} if validator_components else set())),
            f"bootstrap trusted input {label}",
        )
        archive_name = (
            "python-runtime/bin/python3"
            if framework_record
            else default_archive_name
        )
        if label == "python" and record["archive_name"] != archive_name:
            raise ReceiptError("bootstrap trusted Python archive name is not exact")
        maximum_bytes = (
            _MAX_TOOL_BYTES
            if label in executable_labels
            else _MAX_POLICY_BYTES
            if label in {"allowed_signers", "revocation"}
            else _MAX_SDK_MANIFEST_BYTES
            if label == "sdk_dependency_bundle_manifest"
            else _MAX_HELPER_BYTES
        )
        if label == "sdk_dependency_bundle_manifest" and not (
            bootstrap_private_inputs_available
        ):
            archive_path = directory / archive_name
            if os.path.lexists(archive_path):
                raise ReceiptError(
                    "bootstrap-private SDK source manifest survived "
                    "acknowledgment pruning"
                )
            archive_digest = _require_digest(
                record["sha256"],
                "bootstrap archived SDK dependency manifest digest",
            )
            if (
                record["archive_id"]
                != "release-bootstrap.sdk-dependency-bundle-manifest.v1"
                or record["archive_name"] != archive_name
                or record["mode"] != f"{archive_mode:04o}"
                or type(record["size_bytes"]) is not int
                or record["size_bytes"] < 0
                or record["size_bytes"] > maximum_bytes
            ):
                raise ReceiptError(
                    "bootstrap trusted input sdk_dependency_bundle_manifest "
                    "record is not exact"
                )
            trusted_digests[label] = archive_digest
            trusted_archives[label] = {
                "archive_id": record["archive_id"],
                "mode": record["mode"],
                "sha256": archive_digest,
                "size_bytes": record["size_bytes"],
            }
            continue
        archive = _read_evidence_snapshot(
            directory / archive_name,
            f"bootstrap archived {label}",
            maximum_bytes=maximum_bytes,
            expected_mode=archive_mode,
            allowed_owners={os.geteuid()},
            executable=label in executable_labels,
        )
        snapshots["trusted_" + label] = archive
        inode = (archive.device, archive.inode)
        if inode in evidence_inodes:
            raise ReceiptError("bootstrap trusted archives contain an inode alias")
        evidence_inodes.add(inode)
        expected_record = {
            "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
            "archive_name": archive_name,
            "mode": f"{archive_mode:04o}",
            "sha256": archive.sha256,
            "size_bytes": archive.size,
        }
        public_record = {
            key: record[key]
            for key in ("archive_id", "archive_name", "mode", "sha256", "size_bytes")
        }
        if (
            type(record["size_bytes"]) is not int
            or public_record != expected_record
        ):
            raise ReceiptError(f"bootstrap trusted input {label} record is not exact")
        if label == "bootstrap" and archive.sha256 != _FROZEN_BOOTSTRAP_SHA256:
            raise ReceiptError("bootstrap trusted source is not the frozen trust root")
        if framework_record:
            framework_python = True
            framework_runtime_contracts = _validate_framework_python_runtime(
                record["runtime"], directory
            )
        elif label == "python" and (
            Path(sys.executable).resolve(strict=True) == archive.path
            and sys.platform == "darwin"
            and isinstance(sysconfig.get_config_var("PYTHONFRAMEWORK"), str)
            and bool(sysconfig.get_config_var("PYTHONFRAMEWORK"))
        ):
            raise ReceiptError(
                "framework Python trusted input omits its archived runtime closure"
            )
        if bootstrap_components:
            components = _require_exact_json_fields(
                record["components"],
                set(_BOOTSTRAP_COMPONENT_SHA256),
                "bootstrap components",
            )
            for name, expected_digest in sorted(
                _BOOTSTRAP_COMPONENT_SHA256.items()
            ):
                component_record = _require_exact_json_fields(
                    components[name],
                    {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
                    f"bootstrap component {name}",
                )
                if (
                    component_record["archive_id"]
                    != "release-bootstrap.bootstrap-component.v1:" + name
                    or component_record["archive_name"] != name
                    or _octal_mode(
                        component_record["mode"],
                        f"bootstrap component {name} mode",
                    )
                    != _SIGNATURE_DATA_MODE
                    or _require_digest(
                        component_record["sha256"],
                        f"bootstrap component {name} digest",
                    )
                    != expected_digest
                ):
                    raise ReceiptError(
                        f"bootstrap component {name} binding is wrong"
                    )
                component = _bounded_evidence_snapshot(
                    directory / name,
                    f"bootstrap component {name}",
                    maximum_bytes=_MAX_HELPER_BYTES,
                    expected_mode=_SIGNATURE_DATA_MODE,
                    allowed_owners={os.geteuid()},
                )
                if (
                    component.sha256 != expected_digest
                    or type(component_record["size_bytes"]) is not int
                    or component_record["size_bytes"] != component.size
                ):
                    raise ReceiptError(
                        f"bootstrap component {name} bytes are wrong"
                    )
                snapshots["trusted_bootstrap_component:" + name] = component
        if validator_components:
            components = _require_exact_json_fields(
                record["components"],
                set(_RECEIPT_VALIDATOR_COMPONENT_SHA256),
                "bootstrap receipt validator components",
            )
            for name, expected_digest in sorted(
                _RECEIPT_VALIDATOR_COMPONENT_SHA256.items()
            ):
                component_record = _require_exact_json_fields(
                    components[name],
                    {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
                    f"bootstrap receipt validator component {name}",
                )
                if (
                    component_record["archive_id"]
                    != "release-bootstrap.receipt-validator-component.v1:" + name
                    or component_record["archive_name"] != name
                    or _octal_mode(
                        component_record["mode"],
                        f"bootstrap receipt validator component {name} mode",
                    )
                    != _SIGNATURE_DATA_MODE
                    or _require_digest(
                        component_record["sha256"],
                        f"bootstrap receipt validator component {name} digest",
                    )
                    != expected_digest
                ):
                    raise ReceiptError(
                        f"bootstrap receipt validator component {name} binding is wrong"
                    )
                component = _bounded_evidence_snapshot(
                    directory / name,
                    f"bootstrap receipt validator component {name}",
                    maximum_bytes=_MAX_HELPER_BYTES,
                    expected_mode=_SIGNATURE_DATA_MODE,
                    allowed_owners={os.geteuid()},
                )
                if (
                    component.sha256 != expected_digest
                    or type(component_record["size_bytes"]) is not int
                    or component_record["size_bytes"] != component.size
                ):
                    raise ReceiptError(
                        f"bootstrap receipt validator component {name} bytes are wrong"
                    )
                snapshots["trusted_receipt_validator_component:" + name] = component
        trusted_digests[label] = archive.sha256
        trusted_archives[label] = {
            "archive_id": record["archive_id"],
            "mode": record["mode"],
            "sha256": archive.sha256,
            "size_bytes": archive.size,
        }

    release_approvals, release_approval_contracts = (
        _validate_release_approval_evidence(
            marker["release_approvals"],
            directory=directory,
            identity=candidate,
            snapshots=snapshots,
        )
    )
    framework_runtime_contracts.extend(release_approval_contracts)

    identity_records = _require_exact_json_fields(
        marker["identity_verification"],
        set(_BOOTSTRAP_IDENTITY_ARCHIVES),
        "bootstrap identity verification inventory",
    )
    identity_snapshots: dict[str, EvidenceSnapshot] = {
        "trusted_allowed_signers": snapshots["trusted_allowed_signers"],
        "trusted_git": snapshots["trusted_git"],
        "trusted_revocation": snapshots["trusted_revocation"],
        "trusted_ssh_keygen": snapshots["trusted_ssh_keygen"],
    }
    archive_path_snapshots: dict[Path, EvidenceSnapshot] = {}
    for label, (archive_name, mode) in _BOOTSTRAP_IDENTITY_ARCHIVES.items():
        record = _require_exact_json_fields(
            identity_records[label],
            {"archive_name", "mode", "sha256", "size_bytes"},
            f"bootstrap identity verification {label}",
        )
        path = directory / archive_name
        if path in archive_path_snapshots:
            snapshot = archive_path_snapshots[path]
        else:
            snapshot = _read_evidence_snapshot(
                path,
                f"bootstrap identity evidence {label}",
                maximum_bytes=_MAX_LOCK_BYTES,
                expected_mode=mode,
                allowed_owners={os.geteuid()},
                executable=mode == _SIGNATURE_TOOL_MODE,
            )
            archive_path_snapshots[path] = snapshot
            inode = (snapshot.device, snapshot.inode)
            if inode in evidence_inodes:
                raise ReceiptError("bootstrap identity evidence contains an inode alias")
            evidence_inodes.add(inode)
        expected_record = {
            "archive_name": archive_name,
            "mode": f"{mode:04o}",
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
        }
        if type(record["size_bytes"]) is not int or record != expected_record:
            raise ReceiptError(f"bootstrap identity verification {label} is not exact")
        identity_snapshots[label] = snapshot
    if identity_snapshots["identity_transcript"] != identity_snapshots["verify_transcript"]:
        raise ReceiptError("bootstrap transcript alias does not resolve to one exact inode")
    if identity_snapshots["identity_attestation"].path != attestation_path:
        raise ReceiptError("bootstrap attestation input path is not exact")
    if identity_snapshots["identity_transcript"].path != transcript_path:
        raise ReceiptError("bootstrap transcript input path is not exact")

    _, _, verification = _validate_bootstrap_identity_documents(
        directory=directory,
        identity=candidate,
        identity_snapshot=identity_snapshot,
        snapshots=identity_snapshots,
        expected_signer_fingerprint=expected_signer_fingerprint,
        trusted_digests=trusted_digests,
    )
    _validate_raw_commit(identity_snapshots["raw_commit"].data, candidate)
    if identity_snapshots["cargo_lock"].sha256 != candidate["cargo_lock_sha256"]:
        raise ReceiptError("bootstrap identity Cargo.lock has the wrong digest")
    candidate_lock = _read_evidence_snapshot(
        candidate_root / "Cargo.lock",
        "bootstrap candidate Cargo.lock",
        maximum_bytes=_MAX_LOCK_BYTES,
        allowed_owners={os.geteuid()},
    )
    snapshots["candidate_cargo_lock"] = candidate_lock
    if (
        candidate_lock.sha256 != candidate["cargo_lock_sha256"]
        or candidate_lock.data != identity_snapshots["cargo_lock"].data
    ):
        raise ReceiptError("bootstrap candidate Cargo.lock differs from authenticated archive")

    bootstrap_git_snapshot = snapshots["trusted_git"]
    bootstrap_git = bootstrap_git_snapshot.path
    bootstrap_git_environment = _closed_replay_environment(directory)

    def bootstrap_git_line(arguments: list[str], name: str) -> str:
        stdout, stderr = _run_required_replay(
            bootstrap_git,
            arguments,
            root=candidate_root,
            environment=bootstrap_git_environment,
            name=f"bootstrap candidate {name}",
            executable_contract=bootstrap_git_snapshot,
        )
        if (
            stderr
            or not stdout.endswith(b"\n")
            or b"\n" in stdout[:-1]
            or b"\r" in stdout
            or b"\0" in stdout
        ):
            raise ReceiptError(f"bootstrap Git returned malformed {name}")
        try:
            return stdout[:-1].decode("ascii")
        except UnicodeDecodeError as error:
            raise ReceiptError(f"bootstrap Git returned malformed {name}") from error

    if (
        bootstrap_git_line(["rev-parse", "--show-toplevel"], "top-level")
        != str(candidate_root)
        or bootstrap_git_line(
            ["rev-parse", "--verify", "HEAD^{commit}"], "HEAD commit"
        )
        != candidate["head_commit"]
        or bootstrap_git_line(
            [
                "rev-parse",
                "--verify",
                f"{candidate['head_commit']}^{{tree}}",
            ],
            "candidate tree",
        )
        != candidate["head_tree"]
    ):
        raise ReceiptError("bootstrap candidate Git identity is not exact")
    candidate_raw, candidate_raw_stderr = _run_required_replay(
        bootstrap_git,
        ["cat-file", "commit", candidate["head_commit"]],
        root=candidate_root,
        environment=bootstrap_git_environment,
        name="bootstrap candidate raw commit",
        executable_contract=bootstrap_git_snapshot,
    )
    if candidate_raw_stderr or candidate_raw != identity_snapshots["raw_commit"].data:
        raise ReceiptError("bootstrap candidate raw commit differs from authenticated archive")
    if identity_snapshots["raw_commit"].sha256 != signature_archives["raw_commit"]["sha256"]:
        raise ReceiptError("bootstrap and current raw commit evidence differ")
    for bootstrap_label, signature_label in (
        ("cargo_lock", "cargo_lock"),
        ("git", "git"),
        ("ssh_keygen", "ssh_keygen"),
        ("ssh_allowed_signers", "ssh_allowed_signers"),
        ("ssh_revocation", "ssh_revocation"),
    ):
        if identity_snapshots[bootstrap_label].sha256 != signature_archives[signature_label][
            "sha256"
        ]:
            raise ReceiptError(f"bootstrap and current {signature_label} evidence differ")

    runner = _require_exact_json_fields(
        marker["runner"],
        {
            "archive_id",
            "invocation",
            "closed_path_resolution",
            "environment_sha256",
            "mode",
            "output",
            "self_digest_environment_variables",
            "sha256",
            "size_bytes",
            "tool_directory",
            "tools",
        },
        "bootstrap runner",
    )
    expected_runner_path = candidate_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    if runner_path != expected_runner_path:
        raise ReceiptError("bootstrap runner input is not the candidate release runner")
    runner_mode = _octal_mode(runner["mode"], "bootstrap runner mode")
    runner_snapshot = _read_evidence_snapshot(
        runner_path,
        "bootstrap candidate runner",
        maximum_bytes=16 * 1024 * 1024,
        expected_mode=runner_mode,
        allowed_owners={os.geteuid()},
    )
    snapshots["runner"] = runner_snapshot
    if (
        runner["archive_id"] != "release-candidate.runner.v1"
        or runner["invocation"] != {
            "profile": "release",
            "operation_id": "sumeragi-v2.release.v1",
            "arguments": ["--release"],
            "bash_archive_id": "release-bootstrap.bash.v1",
        }
        or runner["sha256"] != runner_snapshot.sha256
        or type(runner["size_bytes"]) is not int
        or runner["size_bytes"] != runner_snapshot.size
        or runner["closed_path_resolution"]
        != {
            "bash": "release-bootstrap.bash.v1",
            "git": "release-bootstrap.git.v1",
            "python3": "release-bootstrap.python.v1",
        }
        or runner["self_digest_environment_variables"]
        != [
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
        ]
    ):
        raise ReceiptError("bootstrap runner binding is not exact")
    output_contract = _require_exact_json_fields(
        runner["output"],
        {
            "stderr_archive_id",
            "stderr_name",
            "stdout_archive_id",
            "stdout_name",
            "active_mode",
            "sealed_mode",
        },
        "bootstrap runner output",
    )
    if output_contract != {
        "stderr_archive_id": "release-bootstrap.runner-stderr.v1",
        "stderr_name": "runner-stderr.log",
        "stdout_archive_id": "release-bootstrap.runner-stdout.v1",
        "stdout_name": "runner-stdout.log",
        "active_mode": "0600",
        "sealed_mode": "0400",
    }:
        raise ReceiptError("bootstrap runner output contract is not exact")
    expected_log_mode = 0o400 if runner_logs_sealed else 0o600
    for output_path in (
        directory / output_contract["stdout_name"],
        directory / output_contract["stderr_name"],
    ):
        metadata = output_path.lstat()
        if (
            output_path.parent != directory
            or stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != expected_log_mode
        ):
            raise ReceiptError("bootstrap runner active log metadata is not exact")
    if runner["tool_directory"] != "runner-bin":
        raise ReceiptError("bootstrap runner tool directory id is not exact")
    tool_directory, _ = _private_evidence_directory(
        directory / "runner-bin", "bootstrap runner tool directory"
    )
    tool_archive_directory, _ = _private_evidence_directory(
        directory / "runner-tools", "bootstrap runner tool archive directory"
    )
    if tool_directory != directory / "runner-bin":
        raise ReceiptError("bootstrap runner tool directory is not exact")
    try:
        tool_manifest = _decode_canonical_json(
            snapshots["trusted_runner_tool_manifest"].data,
            "bootstrap runner tool manifest",
        )
    except KeyError as error:
        raise ReceiptError("bootstrap runner tool manifest is missing") from error
    tool_manifest = _require_exact_json_fields(
        tool_manifest, {"schema_version", "tools"}, "bootstrap runner tool manifest"
    )
    manifest_tools = tool_manifest["tools"]
    runner_tools = runner["tools"]
    if (
        type(tool_manifest["schema_version"]) is not int
        or tool_manifest["schema_version"] != 1
        or not isinstance(manifest_tools, dict)
        or not manifest_tools
        or len(manifest_tools) > 256
        or not isinstance(runner_tools, dict)
        or set(runner_tools) != set(manifest_tools)
    ):
        raise ReceiptError("bootstrap runner tool inventory is not exact")
    runner_tool_sources: dict[str, PathContract] = {}
    runner_tool_total_bytes = 0
    for name in sorted(manifest_tools):
        if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", name) is None or name in {
            "bash",
            "git",
            "python3",
            "ssh-keygen",
        }:
            raise ReceiptError("bootstrap runner tool alias is unsafe")
        manifest_record = _require_exact_json_fields(
            manifest_tools[name], {"path", "sha256"}, f"runner manifest tool {name}"
        )
        marker_record = _require_exact_json_fields(
            runner_tools[name],
            {
                "archive_id",
                "alias_name",
                "archive_name",
                "mode",
                "sha256",
                "size_bytes",
            },
            f"bootstrap runner tool {name}",
        )
        alias_path = tool_directory / name
        alias_metadata = alias_path.lstat()
        archive_path = tool_archive_directory / name
        relative_target = os.path.relpath(archive_path, alias_path.parent)
        if (
            marker_record["alias_name"] != name
            or marker_record["archive_id"] != f"release-runner-tool.{name}.v1"
            or marker_record["archive_name"] != f"runner-tools/{name}"
            or not stat.S_ISLNK(alias_metadata.st_mode)
            or alias_metadata.st_uid != os.geteuid()
            or alias_metadata.st_nlink != 1
            or os.readlink(alias_path) != relative_target
        ):
            raise ReceiptError(f"bootstrap runner tool {name} alias binding is wrong")
        source = _bounded_path_contract(
            archive_path,
            f"bootstrap archived runner tool {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            expected_mode=0o500,
            allowed_owners={os.geteuid()},
            require_single_link=True,
            executable=True,
        )
        runner_tool_total_bytes += source.size
        if runner_tool_total_bytes > _MAX_RUNNER_TOOL_TOTAL_BYTES:
            raise ReceiptError(
                "bootstrap runner tools exceed their aggregate byte limit"
            )
        expected_record = {
            "archive_id": f"release-runner-tool.{name}.v1",
            "alias_name": name,
            "archive_name": f"runner-tools/{name}",
            "mode": "0500",
            "sha256": source.sha256,
            "size_bytes": source.size,
        }
        if (
            marker_record != expected_record
            or manifest_record.get("sha256") != source.sha256
            or alias_path.resolve(strict=True) != source.path
        ):
            raise ReceiptError(f"bootstrap runner tool {name} integrity binding is wrong")
        runner_tool_sources[name] = source
    python_archive_path = snapshots["trusted_python"].path
    closed_path_entries = [str(directory)]
    if framework_python:
        closed_path_entries.append(str(python_archive_path.parent))
    closed_path_entries.append(str(tool_directory))

    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in {
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            "PWD",
            "SHLVL",
            "_",
            "__CF_USER_TEXT_ENCODING",
        }
    }
    if not isinstance(environment, dict) or any(
        not isinstance(key, str)
        or not isinstance(value, str)
        or "\0" in key
        or "\0" in value
        for key, value in environment.items()
    ):
        raise ReceiptError("bootstrap runner environment is malformed")
    _require_digest(
        runner["environment_sha256"], "bootstrap runner environment digest"
    )
    base_environment = {
        "HOME": str(directory / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join(closed_path_entries),
        "TMPDIR": str(directory / "tmp"),
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
    policy_environment = {
        "SUMERAGI_V2_RELEASE_RUNTIME_HELPER": str(
            snapshots["trusted_runtime_helper"].path
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": trusted_digests[
            "runtime_helper"
        ],
        "SUMERAGI_V2_RELEASE_TOOL_PROBE_HELPER": str(
            snapshots["trusted_tool_probe_helper"].path
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": trusted_digests[
            "tool_probe_helper"
        ],
        "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(directory / "ssh-keygen"),
        "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": trusted_digests["git"],
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": trusted_digests[
            "ssh_keygen"
        ],
        "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": expected_signer_fingerprint,
        "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(
            directory / "bootstrap-allowed-signers"
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": trusted_digests[
            "allowed_signers"
        ],
        "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(
            directory / "bootstrap-revocation"
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": trusted_digests[
            "revocation"
        ],
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(completion_path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(attestation_path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(transcript_path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(identity_path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(directory),
    }
    alias_environment = {
        key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): value
        for key, value in policy_environment.items()
        if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
    }
    alias_environment.update({
        "IROHA_RELEASE_RUNTIME_HELPER": str(
            snapshots["trusted_runtime_helper"].path
        ),
        "IROHA_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": trusted_digests[
            "runtime_helper"
        ],
        "IROHA_RELEASE_TOOL_PROBE_HELPER": str(
            snapshots["trusted_tool_probe_helper"].path
        ),
        "IROHA_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": trusted_digests[
            "tool_probe_helper"
        ],
        "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST": str(
            directory / "sdk-dependency-bundle-manifest.json"
        ),
        "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256": (
            trusted_digests["sdk_dependency_bundle_manifest"]
        ),
    })
    fixed_keys = set(base_environment) | set(policy_environment) | set(alias_environment)
    extras = {key: value for key, value in environment.items() if key not in fixed_keys}
    if any(
        _BOOTSTRAP_RUNNER_ENV_RE.fullmatch(key) is None
        or key not in _BOOTSTRAP_RUNNER_ENV_ALLOWLIST
        for key in extras
    ) or environment != {**base_environment, **extras, **policy_environment, **alias_environment}:
        raise ReceiptError("bootstrap runner environment is not the closed frozen environment")
    if hashlib.sha256(_canonical_json(environment)).hexdigest() != runner[
        "environment_sha256"
    ]:
        raise ReceiptError("bootstrap runner environment digest is not exact")
    expected_scaling_environment = {
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": (
            expected_scaling_trial_harness_sha256
        ),
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": (
            expected_scaling_configuration_sha256
        ),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": expected_scaling_irohad_sha256,
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": (
            expected_scaling_iroha_cli_sha256
        ),
    }
    if any(
        environment.get(name) != value
        for name, value in expected_scaling_environment.items()
    ):
        raise ReceiptError(
            "bootstrap runner G-SCALE trust anchors are not the receipt trust anchors"
        )
    for child_name in ("home", "tmp"):
        child, _ = _private_evidence_directory(
            directory / child_name, f"bootstrap {child_name} directory"
        )
        if child.parent != directory:
            raise ReceiptError(f"bootstrap {child_name} directory escaped evidence root")
    expected_closed_paths = {
        "bash": snapshots["trusted_bash"].path,
        "git": snapshots["trusted_git"].path,
        "python3": python_archive_path,
    }
    for command, expected in expected_closed_paths.items():
        discovered = shutil.which(command, path=environment["PATH"])
        if discovered is None or Path(discovered).resolve(strict=True) != expected:
            raise ReceiptError(f"bootstrap closed PATH does not resolve protected {command}")

    probes = _require_exact_json_fields(
        marker["trusted_execution_probes"],
        {"bash", "python", "runner_tool_closure"},
        "bootstrap trusted execution probes",
    )
    python_probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
    python_probe_stdout = f"{python_archive_path}\n".encode()
    expected_probes = {
        "bash": {
            "argv": [str(directory / "bash"), "-c", ":"],
            "exit_status": 0,
        },
        "python": {
            "argv": [
                str(python_archive_path),
                "-I",
                "-S",
                "-c",
                python_probe_code,
            ],
            "expected_executable": (
                "python-runtime/bin/python3" if framework_python else "python3"
            ),
            "exit_status": 0,
            "stdout_sha256": hashlib.sha256(python_probe_stdout).hexdigest(),
            "stdout_size_bytes": len(python_probe_stdout),
        },
    }
    if {label: probes[label] for label in expected_probes} != expected_probes or any(
        type(probes[label]["exit_status"]) is not int for label in expected_probes
    ):
        raise ReceiptError("bootstrap trusted execution probes are not exact")
    tool_probe_closure = _require_exact_json_fields(
        probes["runner_tool_closure"],
        {"manifest", "result", "value"},
        "bootstrap runner tool probes",
    )
    for key, name, archive_id in (
        (
            "manifest",
            "runner-tool-probe-manifest.json",
            "release-bootstrap.runner-tool-probe-manifest.v1",
        ),
        (
            "result",
            "runner-tool-probes.json",
            "release-bootstrap.runner-tool-probes.v1",
        ),
    ):
        record = _require_exact_json_fields(
            tool_probe_closure[key],
            {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
            f"bootstrap runner tool probe {key}",
        )
        if (
            record["archive_id"] != archive_id
            or record["archive_name"] != name
            or record["mode"] != "0400"
        ):
            raise ReceiptError(
                f"bootstrap runner tool probe {key} archive binding is wrong"
            )
    (
        bootstrap_tool_probe_manifest,
        bootstrap_tool_probe_result,
        _,
    ) = _validate_and_replay_tool_probe_closure(
        manifest_path=directory / "runner-tool-probe-manifest.json",
        result_path=directory / "runner-tool-probes.json",
        expected_value=tool_probe_closure["value"],
        tools=runner_tool_sources,
        python=snapshots["trusted_python"],
        helper=snapshots["trusted_tool_probe_helper"],
        archive_id_prefix="release-runner-tool",
        probe_root=directory / ".receipt-runner-tool-probe",
    )
    for key, snapshot in (
        ("manifest", bootstrap_tool_probe_manifest),
        ("result", bootstrap_tool_probe_result),
    ):
        record = tool_probe_closure[key]
        if (
            record["sha256"] != snapshot.sha256
            or type(record["size_bytes"]) is not int
            or record["size_bytes"] != snapshot.size
        ):
            raise ReceiptError(
                f"bootstrap runner tool probe {key} bytes are wrong"
            )
    framework_runtime_contracts.extend(
        (
            _snapshot_contract(bootstrap_tool_probe_manifest),
            _snapshot_contract(bootstrap_tool_probe_result),
        )
    )

    final_directory = directory.lstat()
    if (
        stat.S_ISLNK(final_directory.st_mode)
        or not stat.S_ISDIR(final_directory.st_mode)
        or final_directory.st_uid != os.geteuid()
        or stat.S_IMODE(final_directory.st_mode) != 0o700
        or (final_directory.st_dev, final_directory.st_ino)
        != (directory_stat.st_dev, directory_stat.st_ino)
    ):
        raise ReceiptError("bootstrap evidence directory changed during validation")
    authentication = {
        "schema_version": 2,
        "completion_sha256": marker_snapshot.sha256,
        "frozen_bootstrap_sha256": _FROZEN_BOOTSTRAP_SHA256,
        "candidate_identity_sha256": identity_snapshot.sha256,
        "candidate_commit_oid": candidate["head_commit"],
        "candidate_tree_oid": candidate["head_tree"],
        "runner": {
            "archive_id": runner["archive_id"],
            "sha256": runner_snapshot.sha256,
            "mode": f"{runner_snapshot.mode:04o}",
            "invocation": runner["invocation"],
            "closed_path_resolution": runner["closed_path_resolution"],
            "output": output_contract,
            "tool_directory": "runner-bin",
            "tools": runner_tools,
            "environment_sha256": runner["environment_sha256"],
            "self_digest_environment_variables": runner[
                "self_digest_environment_variables"
            ],
        },
        "signer_fingerprint": expected_signer_fingerprint,
        "allowed_signers_principal": verification["allowed_signers_principal"],
        "trusted_input_digests": trusted_digests,
        "trusted_input_archives": trusted_archives,
        "release_approvals": {
            "archive_id": _APPROVAL_SET_ARCHIVE_ID,
            "sha256": release_approvals["set_attestation"]["sha256"],
            "operation_plan_sha256": release_approvals[
                "operation_plan_sha256"
            ],
        },
    }
    def sanitized_record(
        snapshot: EvidenceSnapshot | PathContract,
        archive_id: str,
    ) -> dict[str, Any]:
        return {
            "archive_id": archive_id,
            "mode": f"{snapshot.mode:04o}",
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
        }
    bootstrap_evidence = {
        "completion": sanitized_record(
            marker_snapshot, "release-bootstrap.completion.v2"
        ),
        "candidate_identity": sanitized_record(
            identity_snapshot, "release-bootstrap.candidate-identity.v1"
        ),
        "runner": sanitized_record(
            runner_snapshot, "release-candidate.runner.v1"
        ),
        "candidate_cargo_lock": sanitized_record(
            candidate_lock, "release-candidate.cargo-lock.v1"
        ),
        "trusted_inputs": {
            label: trusted_archives[label]
            for label in _BOOTSTRAP_TRUSTED_ARCHIVES
        },
        "identity_verification": {
            label: sanitized_record(
                snapshot,
                (
                    _SIGNATURE_ARCHIVE_IDS["verify_transcript"]
                    if label == "identity_transcript"
                    else f"release-bootstrap.identity-{label.replace('_', '-')}.v1"
                ),
            )
            for label, snapshot in identity_snapshots.items()
            if not label.startswith("trusted_")
        },
        "runner_tools": {
            label: runner_tools[label]
            for label in sorted(runner_tool_sources)
        },
        "release_approvals": release_approvals,
    }
    return authentication, bootstrap_evidence, framework_runtime_contracts




def _execute_release_receipt_component(filename: str) -> None:
    """Authenticate and execute one reviewed adjacent receipt component."""

    if (
        filename not in _RELEASE_RECEIPT_COMPONENT_FILES
        or Path(filename).name != filename
        or set(_RELEASE_RECEIPT_COMPONENT_FILES)
        != set(_RELEASE_RECEIPT_COMPONENT_SHA256)
        or len(_RELEASE_RECEIPT_COMPONENT_FILES)
        != len(_RELEASE_RECEIPT_COMPONENT_SHA256)
    ):
        raise RuntimeError(f"invalid release receipt component: {filename!r}")
    path = Path(__file__).resolve(strict=True).with_name(filename)
    try:
        before = path.lstat()
    except OSError as error:
        raise RuntimeError(
            f"release receipt component is unavailable: {filename}"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_size > _MAX_HELPER_BYTES
    ):
        raise RuntimeError(
            f"release receipt component is unavailable: {filename}"
        )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise RuntimeError(
            f"release receipt component could not be opened: {filename}"
        ) from error
    try:
        opened = os.fstat(descriptor)
        stable = (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_size", "st_mtime_ns", "st_ctime_ns",
        )
        if not stat.S_ISREG(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in stable
        ):
            raise RuntimeError(
                f"release receipt component changed while opened: {filename}"
            )
        chunks: list[bytes] = []
        total = 0
        while True:
            block = os.read(
                descriptor,
                min(1024 * 1024, _MAX_HELPER_BYTES + 1 - total),
            )
            if not block:
                break
            chunks.append(block)
            total += len(block)
            if total > _MAX_HELPER_BYTES:
                raise RuntimeError(
                    f"release receipt component is oversized: {filename}"
                )
        after = os.fstat(descriptor)
        current = path.lstat()
        if total != opened.st_size or any(
            getattr(after, field) != getattr(opened, field)
            or getattr(current, field) != getattr(opened, field)
            for field in stable
        ):
            raise RuntimeError(
                f"release receipt component changed while read: {filename}"
            )
        payload = b"".join(chunks)
    finally:
        os.close(descriptor)
    if (
        hashlib.sha256(payload).hexdigest()
        != _RELEASE_RECEIPT_COMPONENT_SHA256[filename]
    ):
        raise RuntimeError(
            f"release receipt component has the wrong digest: {filename}"
        )
    try:
        source = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise RuntimeError(
            f"release receipt component is not UTF-8: {filename}"
        ) from error
    exec(
        compile(
            source,
            f"<release-receipt-component:{filename}>",
            "exec",
        ),
        globals(),
    )


for _release_receipt_component in _RELEASE_RECEIPT_COMPONENT_FILES:
    _execute_release_receipt_component(_release_receipt_component)

for _release_receipt_symbol in (
    "_validate_multilane_apalache_evidence",
    "_validate_formal_snapshot_replays",
    "_formal_artifacts",
    "_sdk_suite_source_manifest",
    "_test_count_from_log",
    "_prebuilt_artifact_root",
    "_prebuilt_release_roots",
    "_prebuilt_directory",
    "_load_identity",
    "_validate_tlaps_resource_evidence",
    "_runtime_tool_probe_evidence",
    "build_receipt",
    "_snapshot_receipt_inputs",
    "_publish_terminal_receipt",
    "main",
):
    if not callable(globals().get(_release_receipt_symbol)):
        raise RuntimeError(
            "release receipt component lacks required symbol "
            f"{_release_receipt_symbol}"
        )


if __name__ == "__main__":
    raise SystemExit(main())
