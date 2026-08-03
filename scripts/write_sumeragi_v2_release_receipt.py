#!/usr/bin/env python3
"""Validate and aggregate source-bound Sumeragi v2 release evidence."""

from __future__ import annotations

import argparse
import base64
import csv
from dataclasses import dataclass
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import selectors
import signal
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any


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
)

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
_SIGNATURE_DATA_MODE = 0o400
_SIGNATURE_TOOL_MODE = 0o500
_SIGNATURE_DIRECTORY_MODE = 0o700
_MAX_SIGNATURE_JSON_BYTES = 8 * 1024 * 1024
_MAX_RAW_COMMIT_BYTES = 16 * 1024 * 1024
_MAX_LOCK_BYTES = 128 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_HELPER_BYTES = 16 * 1024 * 1024
_MAX_TOOL_BYTES = 512 * 1024 * 1024
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
_PREBUILT_MANIFEST_NAME = ".sumeragi-v2-prebuilt-binaries.tsv"
_PREBUILT_INVOCATION_RE = re.compile(r"invocation\.[A-Za-z0-9]+")
_PREBUILT_TRIPLE_RE = re.compile(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_.]+)+")
_PREBUILT_BINARY_SPECS = (
    ("irohad", "release/irohad"),
    (
        "irohad_message_control",
        "message-control/release/irohad",
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
    "98f0a450fd0c25c890d77e3f5c0d13faca76ff3227797962c5dd33e5a29cd2f7"
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
    "revocation": ("bootstrap-revocation", _SIGNATURE_DATA_MODE),
    "runner_tool_manifest": ("runner-tool-manifest.json", _SIGNATURE_DATA_MODE),
    "ssh_keygen": ("ssh-keygen", _SIGNATURE_TOOL_MODE),
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
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
    "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
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
_PRODUCTION_TEST_COUNT = 834
_G_UNIT_TEST_COUNT = 474
_G_UNIT_GROUPS = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
        268,
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
    (
        "production-kura-progress-durability",
        "kura::tests",
        14,
    ),
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
        41,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 118),
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
    ("production-v2-adapter", "sumeragi::v2::tests", 46),
    ("production-v2-body-store", "sumeragi::v2_body_store::tests", 2),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 1),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 71),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 53),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 68),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    (
        "production-v2-lifecycle-recovery",
        "sumeragi::v2_lifecycle_recovery::tests",
        4,
    ),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 37),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 131),
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
)
_RUST_SDK_DIAGNOSTICS_TESTS = (
    "client::tests::get_sumeragi_status_prefers_norito_and_handles_json",
    "client::tests::get_sumeragi_status_rejects_unknown_json_fields",
    "client::tests::get_sumeragi_status_rejects_structurally_impossible_norito_and_json",
    "client::tests::get_sumeragi_status_json_requests_json_and_falls_back_to_norito",
    "client::tests::get_sumeragi_diagnostics_verifies_lane_relay_envelopes",
    "client::tests::get_sumeragi_diagnostics_rejects_invalid_lane_relay_hash",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_autonomous_execution",
    "client::tests::get_sumeragi_diagnostics_rejects_duplicate_autonomous_execution_identity",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_native_amx_receipts_in_every_container",
    "client::tests::get_sumeragi_diagnostics_rejects_malformed_json_payload",
    "client::tests::get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields",
    "client::tests::get_sumeragi_diagnostics_rejects_unknown_json_fields",
    "client::tests::get_sumeragi_diagnostics_rejects_zero_npos_seed",
    "client::tests::get_sumeragi_diagnostics_accepts_json_payload_without_content_type_header",
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
    ("javascript", 59),
    ("swift", 4),
    ("kotlin", 6),
    ("java", 5),
)
_SUMERAGI_SDK_DIAGNOSTICS_HARNESS = "ci/run_sumeragi_v2_sdk_diagnostics.sh"
_SUMERAGI_SDK_DIAGNOSTICS_SUITES = (
    ("python", 114),
    ("javascript", 88),
    ("swift", 17),
    ("kotlin", 15),
    ("java", 10),
)
_SUMERAGI_SDK_DIAGNOSTICS_SUITE_SOURCE_PATHS = (
    "ci/run_sumeragi_v2_sdk_diagnostics.sh",
    "ci/native_amx_v2_grouped_gradle_init.gradle",
    "python/iroha_python/tests/client_sumeragi_v2_status_test.py",
    "python/iroha_python/src/iroha_python/client.py",
    "python/iroha_torii_client/tests/test_client.py",
    "python/iroha_torii_client/client.py",
    "javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js",
    "javascript/iroha_js/test/toriiClient.test.js",
    "javascript/iroha_js/src/toriiClient.js",
    "javascript/iroha_js/scripts/build-dist.mjs",
    "javascript/iroha_js/package.json",
    "javascript/iroha_js/package-lock.json",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
    "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    "IrohaSwift/Package.swift",
    "IrohaSwift/Package.resolved",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/consensus/"
    "SumeragiDiagnosticsModelsTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/client/"
    "HttpClientTransportTest.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/"
    "SumeragiDiagnosticsModels.kt",
    "kotlin/core-jvm/build.gradle.kts",
    "kotlin/settings.gradle.kts",
    "kotlin/gradlew",
    "kotlin/gradle/wrapper/gradle-wrapper.jar",
    "kotlin/gradle/wrapper/gradle-wrapper.properties",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/consensus/"
    "SumeragiDiagnosticsModelsTests.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
    "SumeragiDiagnosticsModels.java",
    "java/iroha_android/core/build.gradle.kts",
    "java/iroha_android/settings.gradle.kts",
    "java/iroha_android/gradlew",
    "java/iroha_android/gradle/wrapper/gradle-wrapper.jar",
    "java/iroha_android/gradle/wrapper/gradle-wrapper.properties",
)
_NATIVE_AMX_GROUPED_SUITE_SOURCE_PATHS = (
    "ci/run_native_amx_v2_grouped_sdk_parity.sh",
    "ci/native_amx_v2_grouped_gradle_init.gradle",
    "crates/iroha_data_model/src/bin/native_amx_grouped.rs",
    "pytests/scripts/native_amx_v2_grouped_fixture_test.py",
    "python/iroha_python/tests/native_amx_v2_grouped_fixture_test.py",
    "python/iroha_python/src/iroha_python/client.py",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_torii_client/client.py",
    "python/iroha_torii_client/native_amx.py",
    "javascript/iroha_js/test/nativeAmxV2GroupedFixture.test.js",
    "javascript/iroha_js/src/toriiClient.js",
    "javascript/iroha_js/src/norito.js",
    "javascript/iroha_js/src/native.js",
    "javascript/iroha_js/scripts/build-dist.mjs",
    "javascript/iroha_js/scripts/native-build-provenance.mjs",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/package.json",
    "javascript/iroha_js/package-lock.json",
    "IrohaSwift/Tests/IrohaSwiftTests/NativeAmxV2GroupedFixtureTests.swift",
    "IrohaSwift/Sources/IrohaSwift/CanonicalNoritoEncoding.swift",
    "IrohaSwift/Sources/IrohaSwift/Crypto.swift",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/Norito.swift",
    "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    "IrohaSwift/Package.swift",
    "IrohaSwift/Package.resolved",
    "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "crates/connect_norito_bridge/src/lib.rs",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/consensus/"
    "NativeAmxV2GroupedFixtureTest.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/"
    "NativeAmxV2.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/"
    "SumeragiDiagnosticsModels.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/util/"
    "HashLiteral.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/"
    "IrohaHash.kt",
    "kotlin/core-jvm/build.gradle.kts",
    "kotlin/settings.gradle.kts",
    "kotlin/gradlew",
    "kotlin/gradle/wrapper/gradle-wrapper.jar",
    "kotlin/gradle/wrapper/gradle-wrapper.properties",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/consensus/"
    "NativeAmxV2GroupedFixtureTests.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
    "NativeAmxV2Models.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
    "SumeragiDiagnosticsModels.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/"
    "IrohaHash.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/util/"
    "HashLiteral.java",
    "java/iroha_android/core/build.gradle.kts",
    "java/iroha_android/settings.gradle.kts",
    "java/iroha_android/gradlew",
    "java/iroha_android/gradle/wrapper/gradle-wrapper.jar",
    "java/iroha_android/gradle/wrapper/gradle-wrapper.properties",
    "artifacts/openapi/torii.json",
    "artifacts/openapi/versions/current/torii.json",
)
def _native_amx_grouped_suite_source_manifest(repo_root: Path) -> str:
    digest = hashlib.sha256()
    for relative_path in _NATIVE_AMX_GROUPED_SUITE_SOURCE_PATHS:
        source = _bounded_path_contract(
            repo_root / relative_path,
            f"grouped Native AMX V2 suite source {relative_path}",
            maximum_bytes=_MAX_TOOL_BYTES,
            require_single_link=False,
        )
        digest.update(f"{relative_path}\t{source.sha256}\n".encode())
    return digest.hexdigest()


def _sumeragi_sdk_diagnostics_suite_source_manifest(repo_root: Path) -> str:
    digest = hashlib.sha256()
    for relative_path in _SUMERAGI_SDK_DIAGNOSTICS_SUITE_SOURCE_PATHS:
        source = _bounded_path_contract(
            repo_root / relative_path,
            f"Sumeragi v2 SDK diagnostics suite source {relative_path}",
            maximum_bytes=_MAX_TOOL_BYTES,
            require_single_link=False,
        )
        digest.update(f"{relative_path}\t{source.sha256}\n".encode())
    return digest.hexdigest()


def _canonical_production_tests(
    repo_root: Path,
    runner_snapshot: EvidenceSnapshot | None = None,
) -> list[str]:
    if runner_snapshot is None:
        runner_snapshot = _bounded_evidence_snapshot(
            repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
            "release runner inventory",
            maximum_bytes=_MAX_POLICY_BYTES,
            require_single_link=False,
        )
    source = _decode_lf_text(runner_snapshot, "release runner inventory")
    marker = "required_production_liveness_tests=(\n"
    if source.count(marker) != 1:
        raise ReceiptError("release runner lacks one canonical production inventory")
    body = source.split(marker, 1)[1].split("\n)", 1)[0]
    tests = [line.strip() for line in body.splitlines() if line.strip()]
    if (
        len(tests) != _PRODUCTION_TEST_COUNT
        or len(set(tests)) != _PRODUCTION_TEST_COUNT
        or any(
            not test.startswith(
                (
                    "sumeragi::",
                    "sumeragi_v2_runner::",
                    "block::",
                    "offline::",
                    "zk::",
                    "merge_sidecar::",
                    "kura::",
                    "nexus::",
                    "peer::",
                    "network::",
                    "consensus_message_control::tests::",
                    "network_relay_tests::",
                    "tests::relay_fairness::",
                    "parameters::",
                )
            )
            for test in tests
        )
    ):
        raise ReceiptError(
            "release runner production inventory is not exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    return tests


def _canonical_g_unit_rows(
    repo_root: Path,
    runner_snapshot: EvidenceSnapshot | None = None,
) -> list[tuple[str, str, str]]:
    if runner_snapshot is None:
        runner_snapshot = _bounded_evidence_snapshot(
            repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
            "release runner G-UNIT inventory",
            maximum_bytes=_MAX_POLICY_BYTES,
            require_single_link=False,
        )
    source = _decode_lf_text(runner_snapshot, "release runner G-UNIT inventory")
    rows: list[tuple[str, str, str]] = []
    for array_name, leg_id, package, expected_count, cargo_target in _G_UNIT_GROUPS:
        marker = f"{array_name}=(\n"
        if source.count(marker) != 1:
            raise ReceiptError(
                f"release runner lacks one canonical {array_name} G-UNIT inventory"
            )
        body = source.split(marker, 1)[1].split("\n)", 1)[0]
        tests = [
            line.strip()
            for line in body.splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        if (
            len(tests) != expected_count
            or len(set(tests)) != expected_count
            or any(
                re.fullmatch(
                    (
                        r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+"
                        if cargo_target == "lib"
                        else r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*"
                    ),
                    test,
                )
                is None
                for test in tests
            )
        ):
            raise ReceiptError(
                f"release runner {array_name} inventory is not exactly "
                f"{expected_count} distinct tests"
            )
        rows.extend((leg_id, package, test) for test in tests)
    names = [test for _, _, test in rows]
    if len(rows) != _G_UNIT_TEST_COUNT or len(set(names)) != _G_UNIT_TEST_COUNT:
        raise ReceiptError(
            f"release runner G-UNIT inventory is not exactly "
            f"{_G_UNIT_TEST_COUNT} globally distinct tests"
        )
    return rows


def _g_unit_leg_command(array_name: str, package: str, cargo_target: str) -> str:
    if cargo_target == "lib":
        target = "--lib"
    elif cargo_target.startswith("test:"):
        test_target = cargo_target.removeprefix("test:")
        if re.fullmatch(r"[A-Za-z0-9_]+", test_target) is None:
            raise ReceiptError("G-UNIT test target is not canonical")
        target = f"--test {test_target}"
    else:
        raise ReceiptError("G-UNIT Cargo target is not canonical")
    return (
        f"for test in {array_name}; do cargo test --locked --offline "
        f'-p {package} {target} "$test" -- --exact --test-threads=1; done'
    )


def _production_module_command(module: str) -> str:
    if module == "sumeragi_v2_runner":
        return (
            "cargo test --locked --offline -p integration_tests --test "
            "sumeragi_v2_runner_isolated "
            f"{_PRODUCTION_INTEGRATION_MODULE} -- --test-threads=1"
        )
    if module in {
        "peer::run::tests",
        "network::tests",
        "network::inbound_source_memory_bound_tests",
        "network::handle_update_tests",
    }:
        return (
            "cargo test --locked --offline -p iroha_p2p --lib "
            f"{module} -- --test-threads=1"
        )
    if module in {
        "consensus_message_control::tests",
        "network_relay_tests",
        "tests::relay_fairness",
    }:
        return (
            "cargo test --locked --offline -p irohad --bin irohad "
            "--features test-network-message-control "
            f"{module} -- --test-threads=1"
        )
    if module.startswith("parameters::"):
        return (
            "cargo test --locked --offline -p iroha_config --lib "
            f"{module} -- --test-threads=1"
        )
    if module in _DATA_MODEL_PRODUCTION_MODULES:
        return (
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{module} -- --test-threads=1"
        )
    return (
        "cargo test --locked --offline -p iroha_core --lib "
        f"{module} -- --test-threads=1"
    )


def _corridor_legs() -> list[tuple[str, str, int, str]]:
    legs = [
        (
            leg_id,
            "cargo-focus",
            count,
            _g_unit_leg_command(array_name, package, cargo_target),
        )
        for array_name, leg_id, package, count, cargo_target in _G_UNIT_GROUPS
    ]
    legs.extend(
        (
            (
                leg_id,
                "cargo-module",
                count,
                _production_module_command(module),
            )
            for leg_id, module, count in _PRODUCTION_MODULES
        )
    )
    legs.append(
        (
            "status-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_STATUS_TEST} -- --test-threads=1",
        )
    )
    legs.append(
        (
            "lane-certificate-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_LANE_CERTIFICATE_TEST} -- --exact --test-threads=1",
        )
    )
    legs.extend(
        (
            (
                "source-sealed-workspace-format",
                "command",
                0,
                "cargo fmt --all -- --check",
            ),
            (
                "source-sealed-legacy-codec-guard",
                "command",
                0,
                "bash scripts/check_no_legacy_codec.sh",
            ),
            (
                "source-sealed-workspace-build",
                "command",
                0,
                "cargo build --locked --offline --workspace",
            ),
            (
                "source-sealed-workspace-clippy",
                "command",
                0,
                "cargo clippy --locked --offline --workspace --all-targets "
                "-- -D warnings",
            ),
            (
                "source-sealed-workspace-tests",
                "command",
                0,
                "cargo test --locked --offline --workspace",
            ),
            (
                "source-sealed-irohad-tests",
                "command",
                0,
                "cargo test --locked --offline -p irohad --bin irohad "
                "--features test-network-message-control",
            ),
        )
    )
    legs.extend(
        (
            f"taira-contract-{index}",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p integration_tests "
            "--test consensus_and_da "
            f"{test} -- --exact --test-threads=1",
        )
        for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
    )
    legs.append(
        (
            "cross-sdk-rust",
            "cargo-exact",
            2,
            "cargo test --locked --offline -p iroha_data_model --test "
            "iroha_data_model_group_02 sumeragi_v2_cross_sdk_fixtures:: "
            "-- --test-threads=1",
        )
    )
    legs.append(
        (
            "native-amx-rust-fixture-check",
            "command",
            0,
            "cargo run --locked --offline -p iroha_data_model --bin "
            "sumeragi_v2_wire_fixtures -- --check",
        )
    )
    legs.extend(
        (
            f"native-amx-grouped-{surface}",
            "native-amx-sdk",
            count,
            f"bash {_NATIVE_AMX_GROUPED_PARITY_HARNESS} {surface}",
        )
        for surface, count in _NATIVE_AMX_GROUPED_PARITY_SUITES
    )
    legs.append(
        (
            "sumeragi-diagnostics-rust",
            "cargo-exact",
            len(_RUST_SDK_DIAGNOSTICS_TESTS),
            "cargo test --locked --offline -p iroha --lib "
            "client::tests::get_sumeragi_ -- --test-threads=1",
        )
    )
    legs.extend(
        (
            f"sumeragi-diagnostics-{surface}",
            "sdk-diagnostics",
            count,
            f"bash {_SUMERAGI_SDK_DIAGNOSTICS_HARNESS} {surface}",
        )
        for surface, count in _SUMERAGI_SDK_DIAGNOSTICS_SUITES
    )
    legs.extend(
        (
            (
                "preflight-source-seal",
                "pytest",
                30,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/workspace_source_manifest_test.py "
                "pytests/scripts/seal_workspace_source_test.py",
            ),
            (
                "preflight-seed-launcher",
                "pytest",
                14,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_prior_invocation_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_ambiguous_test_summary "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_cargo_failure_through_tee "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_bundle_tampering_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_marker_durability_failure_is_not_terminal "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_source_drift_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_refuses_uninspected_stale_lock "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_unsafe_retained_localnet_entries",
            ),
            (
                "preflight-chaos-launcher",
                "pytest",
                5,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_chaos_release_test.py",
            ),
            (
                "preflight-release-identity",
                "pytest",
                68,
                "SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN="
                "$IROHA_RELEASE_SSH_KEYGEN_BIN PYTHONDONTWRITEBYTECODE=1 "
                "PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_identity_signature_test.py",
            ),
            (
                "preflight-release-bootstrap",
                "pytest",
                82,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_test.py",
            ),
            (
                "preflight-release-bootstrap-validator",
                "pytest",
                37,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_validator_test.py",
            ),
            (
                "preflight-release-receipt",
                "pytest",
                320,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_receipt_test.py "
                "pytests/scripts/sumeragi_v2_release_receipt_components_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_shell_test.py",
            ),
            (
                "preflight-multilane-scaling",
                "pytest",
                52,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "scripts/tests/validate_multilane_scaling_evidence_test.py "
                "scripts/tests/run_multilane_scaling_gate_test.py",
            ),
            (
                "preflight-proof-fidelity",
                "pytest",
                4249,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_proof_ledger_test.py "
                "pytests/scripts/sumeragi_v2_verus_evidence_test.py "
                "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_legacy_layout_only_claim "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_state_order_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_missing_direct_release_action "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_tla_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_verus_snapshot_stutter_proof_removal "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_layout_contract_rejects_membership_only_lane_authorship",
            ),
            (
                "preflight-formal-launcher",
                "pytest",
                26,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_formal_release_test.py",
            ),
            (
                "preflight-taira-soak",
                "pytest",
                42,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_pins_complete_profile_and_runs_exactly_one_test "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_inventory "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_execution_output "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_bundle_tampering_before_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_marker_durability_failure_is_not_terminal "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_profile_override_arguments_before_cargo "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_a_concurrent_source_bound_soak "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_does_not_promote_provisional_evidence_when_validation_fails "
                "pytests/scripts/taira_v2_soak_evidence_test.py",
            ),
        )
    )
    return legs


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
    expected_release_root = directory / "release-runner" / "source"
    if release_root != expected_release_root:
        raise ReceiptError(
            "sealed release root must be the exact bootstrap release-runner source"
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


def _abort_replay(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        try:
            process.kill()
        except OSError:
            pass
    try:
        process.wait(timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        pass


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
                *tuple(expected.path.parent.parents)[:3],
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
                start_new_session=True,
            )
        except OSError as error:
            raise ReceiptError(f"{name} could not be started") from error
        assert process.stdout is not None and process.stderr is not None
        selector = selectors.DefaultSelector()
        streams = {
            process.stdout.fileno(): ("stdout", process.stdout),
            process.stderr.fileno(): ("stderr", process.stderr),
        }
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        for descriptor, item in streams.items():
            os.set_blocking(descriptor, False)
            selector.register(descriptor, selectors.EVENT_READ, item)
        stdin_offset = 0
        if process.stdin is not None:
            os.set_blocking(process.stdin.fileno(), False)
            if stdin_data:
                selector.register(
                    process.stdin.fileno(),
                    selectors.EVENT_WRITE,
                    ("stdin", process.stdin),
                )
            else:
                process.stdin.close()
        deadline = time.monotonic() + _REPLAY_TIMEOUT_SECONDS
        try:
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    _abort_replay(process)
                    raise ReceiptError(f"{name} exceeded its timeout")
                for key, _ in selector.select(min(remaining, 0.25)):
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
                            selector.unregister(key.fd)
                            stream.close()
                            continue
                        if written <= 0:
                            _abort_replay(process)
                            raise ReceiptError(
                                f"{name} stdin write made no progress"
                            )
                        stdin_offset += written
                        if stdin_offset == len(stdin_data):
                            selector.unregister(key.fd)
                            stream.close()
                        continue
                    try:
                        chunk = os.read(key.fd, 64 * 1024)
                    except BlockingIOError:
                        continue
                    if not chunk:
                        selector.unregister(key.fd)
                        stream.close()
                        continue
                    buffers[stream_name].extend(chunk)
                    if (
                        sum(len(value) for value in buffers.values())
                        > maximum_output_bytes
                    ):
                        _abort_replay(process)
                        raise ReceiptError(
                            f"{name} output exceeds its closed limit"
                        )
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _abort_replay(process)
                raise ReceiptError(f"{name} exceeded its timeout")
            try:
                status = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired as error:
                _abort_replay(process)
                raise ReceiptError(f"{name} exceeded its timeout") from error
        except BaseException:
            if process.poll() is None:
                _abort_replay(process)
            raise
        finally:
            selector.close()
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()
        _revalidate_execution_inputs(
            contracts, descriptors, directory_contracts
        )
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
        watched_contracts=(checker_snapshot,),
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
        {
            "schema_version",
            "release_identity",
            "release_identity_sha256",
            "tools",
            "policies",
            "verification",
            "evidence",
        },
        "release signature attestation",
    )
    if type(attestation["schema_version"]) is not int or attestation["schema_version"] != 2:
        raise ReceiptError("release signature attestation has the wrong schema version")
    if (
        attestation["release_identity"] != candidate
        or attestation["release_identity_sha256"]
        != candidate_snapshot.sha256
    ):
        raise ReceiptError("release signature attestation is not bound to exact candidate bytes")

    expected_archive_names = {
        label: archive_name
        for label, archive_name in _SIGNATURE_ARCHIVE_NAMES.items()
        if label != "attestation"
    }
    evidence_records = _require_exact_json_fields(
        attestation["evidence"],
        set(expected_archive_names),
        "release signature attestation evidence",
    )
    for label, archive_name in expected_archive_names.items():
        mode = (
            _SIGNATURE_TOOL_MODE if label in {"git", "ssh_keygen"} else _SIGNATURE_DATA_MODE
        )
        expected_record = _artifact_metadata(
            archives[label]["data"], archive_name, mode
        )
        record = _require_exact_json_fields(
            evidence_records[label],
            {"archive_name", "mode", "sha256", "size_bytes"},
            f"release signature attestation evidence for {label}",
        )
        if type(record["size_bytes"]) is not int or record != expected_record:
            raise ReceiptError(
                f"release signature attestation evidence for {label} is not exact"
            )

    tools = _require_exact_json_fields(
        attestation["tools"], {"git", "ssh_keygen"}, "attested release tools"
    )
    _validate_tool_metadata(
        tools["git"],
        name="attested Git tool",
        archive=archives["git"],
        archive_name=_SIGNATURE_ARCHIVE_NAMES["git"],
        protected_sha256=protected_git,
    )
    _validate_tool_metadata(
        tools["ssh_keygen"],
        name="attested ssh-keygen tool",
        archive=archives["ssh_keygen"],
        archive_name=_SIGNATURE_ARCHIVE_NAMES["ssh_keygen"],
        protected_sha256=protected_ssh,
    )

    policies = _require_exact_json_fields(
        attestation["policies"],
        {
            "expected_signer_fingerprint",
            "signature_format",
            "ssh_allowed_signers",
            "ssh_revocation",
        },
        "attested release policies",
    )
    expected_policies = {
        "expected_signer_fingerprint": protected_fingerprint,
        "signature_format": "ssh",
        "ssh_allowed_signers": _protected_metadata(
            archives["ssh_allowed_signers"]["data"],
            _SIGNATURE_ARCHIVE_NAMES["ssh_allowed_signers"],
            _SIGNATURE_DATA_MODE,
            protected_allowed,
        ),
        "ssh_revocation": _protected_metadata(
            archives["ssh_revocation"]["data"],
            _SIGNATURE_ARCHIVE_NAMES["ssh_revocation"],
            _SIGNATURE_DATA_MODE,
            protected_revocation,
        ),
    }
    for policy_name in ("ssh_allowed_signers", "ssh_revocation"):
        policy_record = _require_exact_json_fields(
            policies[policy_name],
            {
                "archive_name",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
            },
            f"attested {policy_name} policy",
        )
        if type(policy_record["size_bytes"]) is not int:
            raise ReceiptError(f"attested {policy_name} policy has a non-integer size")
    if policies != expected_policies:
        raise ReceiptError("attested release policies do not match out-of-band trust policy")

    verification = _require_exact_json_fields(
        attestation["verification"],
        {
            "status",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
        },
        "attested release verification",
    )
    if (
        verification["status"] != "G"
        or verification["signer_fingerprint"] != protected_fingerprint
        or verification["primary_key_fingerprint"] != ""
        or not isinstance(verification["allowed_signers_principal"], str)
        or not verification["allowed_signers_principal"]
        or any(
            ord(character) < 0x20 or ord(character) == 0x7F
            for character in verification["allowed_signers_principal"]
        )
    ):
        raise ReceiptError("attested release verification is not one exact trusted signer")

    transcript = _decode_canonical_json(
        archives["verify_transcript"]["data"], "release signature transcript"
    )
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
        "release signature transcript",
    )
    if type(transcript["schema_version"]) is not int or transcript["schema_version"] != 2:
        raise ReceiptError("release signature transcript has the wrong schema version")
    if transcript["archive_names"] != expected_archive_names:
        raise ReceiptError("release signature transcript archive mapping is not exact")
    if transcript["candidate_commit_oid"] != candidate["head_commit"]:
        raise ReceiptError("release signature transcript does not use the immutable candidate OID")
    if transcript["tools"] != tools or transcript["policies"] != policies:
        raise ReceiptError("release signature transcript disagrees with its attestation")
    expected_environment = _closed_replay_environment(directory)
    if transcript["environment"] != expected_environment:
        raise ReceiptError("release signature transcript environment is not closed")

    commands = _require_exact_json_fields(
        transcript["commands"],
        {"show_signature_metadata", "verify_commit"},
        "release signature transcript commands",
    )
    verify_record = _decode_command_record(
        commands["verify_commit"], "release verify-commit command"
    )
    show_record = _decode_command_record(
        commands["show_signature_metadata"], "release signature-metadata command"
    )
    probes = _require_exact_json_fields(
        transcript["tool_probes"],
        {"ssh_keygen_usage"},
        "release signature transcript probes",
    )
    ssh_probe = _decode_command_record(
        probes["ssh_keygen_usage"], "release ssh-keygen probe"
    )

    verify_argv = verify_record["argv"]
    if len(verify_argv) < 4 or verify_argv[-3:] != [
        "verify-commit",
        "--raw",
        candidate["head_commit"],
    ]:
        raise ReceiptError("historical verify-commit did not use the immutable candidate OID")
    staged_git = _historical_stage_path(
        verify_argv[0], directory, _SIGNATURE_ARCHIVE_NAMES["git"], "historical Git"
    )
    historical_config = verify_argv[1:-3]
    if len(historical_config) != 16:
        raise ReceiptError("historical Git signature policy has the wrong arity")
    assignments = {}
    for index in (5, 7, 9):
        key, marker, value = historical_config[index].partition("=")
        if not marker:
            raise ReceiptError("historical Git signature policy lacks a path assignment")
        assignments[key] = value
    staged_ssh = _historical_stage_path(
        assignments.get("gpg.ssh.program", ""),
        directory,
        _SIGNATURE_ARCHIVE_NAMES["ssh_keygen"],
        "historical ssh-keygen",
    )
    staged_allowed = _historical_stage_path(
        assignments.get("gpg.ssh.allowedSignersFile", ""),
        directory,
        _SIGNATURE_ARCHIVE_NAMES["ssh_allowed_signers"],
        "historical allowed-signers policy",
    )
    staged_revocation = _historical_stage_path(
        assignments.get("gpg.ssh.revocationFile", ""),
        directory,
        _SIGNATURE_ARCHIVE_NAMES["ssh_revocation"],
        "historical revocation policy",
    )
    expected_historical_config = _signature_config(
        str(staged_ssh), str(staged_allowed), str(staged_revocation)
    )
    if (
        historical_config != expected_historical_config
        or transcript["policy_overrides"] != expected_historical_config
        or show_record["argv"]
        != [
            str(staged_git),
            *expected_historical_config,
            "show",
            "--no-patch",
            "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
            candidate["head_commit"],
        ]
        or ssh_probe["argv"] != [str(staged_ssh), "-?"]
    ):
        raise ReceiptError("historical signature command mapping is not canonical")

    placeholder = "${EVIDENCE_DIRECTORY}"
    replay_config = _signature_config(
        f"{placeholder}/{_SIGNATURE_ARCHIVE_NAMES['ssh_keygen']}",
        f"{placeholder}/{_SIGNATURE_ARCHIVE_NAMES['ssh_allowed_signers']}",
        f"{placeholder}/{_SIGNATURE_ARCHIVE_NAMES['ssh_revocation']}",
    )
    replay = _require_exact_json_fields(
        transcript["replay"],
        {
            "candidate_root",
            "evidence_directory",
            "environment",
            "policy_overrides",
        },
        "release signature replay mapping",
    )
    replay_environment = {
        key: value.replace(str(directory), placeholder)
        for key, value in expected_environment.items()
    }
    if replay != {
        "candidate_root": "${CANDIDATE_ROOT}",
        "evidence_directory": placeholder,
        "environment": replay_environment,
        "policy_overrides": replay_config,
    }:
        raise ReceiptError("release signature replay mapping is not canonical")
    placeholder_git = f"{placeholder}/{_SIGNATURE_ARCHIVE_NAMES['git']}"
    placeholder_ssh = f"{placeholder}/{_SIGNATURE_ARCHIVE_NAMES['ssh_keygen']}"
    if (
        verify_record["replay_argv"]
        != [
            placeholder_git,
            *replay_config,
            "verify-commit",
            "--raw",
            candidate["head_commit"],
        ]
        or show_record["replay_argv"]
        != [
            placeholder_git,
            *replay_config,
            "show",
            "--no-patch",
            "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
            candidate["head_commit"],
        ]
        or ssh_probe["replay_argv"] != [placeholder_ssh, "-?"]
    ):
        raise ReceiptError("release transcript replay argument vectors are not canonical")
    if verify_record["exit_status"] != 0 or show_record["exit_status"] != 0:
        raise ReceiptError("historical Git signature verification did not succeed")
    if ssh_probe["exit_status"] < 0:
        raise ReceiptError("historical ssh-keygen probe did not execute")
    if show_record["_stderr_bytes"]:
        raise ReceiptError("historical Git signature metadata has unexpected stderr")
    historical_metadata = _verification_metadata(
        show_record["_stdout_bytes"], "historical Git signature metadata"
    )
    if historical_metadata != (
        verification["status"],
        verification["signer_fingerprint"],
        verification["primary_key_fingerprint"],
        verification["allowed_signers_principal"],
    ):
        raise ReceiptError("historical Git metadata disagrees with its attestation")

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
    verify_status, _, _ = _run_bounded_replay(
        git,
        [*actual_config, "verify-commit", "--raw", candidate["head_commit"]],
        cwd=root,
        environment=expected_environment,
        executable_contract=git_contract,
    )
    if verify_status != 0:
        raise ReceiptError("archived Git cryptographic signature replay failed")
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
    if replay_metadata != historical_metadata:
        raise ReceiptError("archived Git signature replay changed signer metadata")
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
        "allowed_signers_principal": verification["allowed_signers_principal"],
        "release_root": str(root),
        "archive_directory": str(directory),
        "trust_policy": {
            "git_sha256": protected_git,
            "ssh_keygen_sha256": protected_ssh,
            "allowed_signers_sha256": protected_allowed,
            "revocation_sha256": protected_revocation,
            "signer_fingerprint": protected_fingerprint,
        },
        "attested_tools": tools,
        "attested_policies": policies,
        "replay": {
            "performed": True,
            "candidate_root_placeholder": replay["candidate_root"],
            "evidence_directory_placeholder": replay["evidence_directory"],
            "archive_names": expected_archive_names,
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


def _validate_bootstrap_identity_documents(
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
) -> tuple[dict[str, Any], dict[str, Any]]:
    expected_marker_sha = _require_digest(
        expected_completion_sha256, "expected bootstrap completion digest"
    )
    directory, directory_stat = _private_evidence_directory(
        evidence_dir_path, "bootstrap evidence directory"
    )
    candidate_root = _release_root(candidate_root_path)
    release_root = _release_root(release_root_path)
    expected_release_root = directory / "release-runner" / "source"
    if release_root != expected_release_root:
        raise ReceiptError("sealed release root is not the exact bootstrap runner source")
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
            "candidate_root",
            "candidate_identity",
            "candidate_identity_sha256",
            "trusted_inputs",
            "identity_verification",
            "runner",
            "trusted_execution_probes",
        },
        "bootstrap completion marker",
    )
    if type(marker["schema_version"]) is not int or marker["schema_version"] != 1:
        raise ReceiptError("bootstrap completion marker has the wrong schema version")
    if marker["trust_boundary"] != {
        "bootstrap_authentication": "external prerequisite",
        "release_image_and_dynamic_loader": "external prerequisite",
        "same_uid_and_trusted_ancestor_owners": True,
    } or type(marker["trust_boundary"].get("same_uid_and_trusted_ancestor_owners")) is not bool:
        raise ReceiptError("bootstrap completion marker has the wrong trust boundary")
    if (
        marker["candidate_root"] != str(candidate_root)
        or marker["candidate_identity"] != candidate
        or marker["candidate_identity_sha256"] != identity_snapshot.sha256
    ):
        raise ReceiptError("bootstrap completion marker has the wrong candidate identity")
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if candidate[field] != sealed[field]:
            raise ReceiptError(f"sealed worktree does not reproduce bootstrap {field}")

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
    trusted_sources: dict[str, dict[str, Any]] = {}
    evidence_inodes: set[tuple[int, int]] = {
        (marker_snapshot.device, marker_snapshot.inode),
        (identity_snapshot.device, identity_snapshot.inode),
    }
    executable_labels = {"python", "git", "ssh_keygen", "bash"}
    for label, (archive_name, archive_mode) in _BOOTSTRAP_TRUSTED_ARCHIVES.items():
        record = _require_exact_json_fields(
            trusted_records[label],
            {
                "archive_name",
                "archive_mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_mode",
                "source_path",
            },
            f"bootstrap trusted input {label}",
        )
        maximum_bytes = (
            _MAX_TOOL_BYTES
            if label in executable_labels
            else _MAX_POLICY_BYTES
            if label in {"allowed_signers", "revocation"}
            else _MAX_HELPER_BYTES
        )
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
        source_mode = _octal_mode(record["source_mode"], f"bootstrap {label} source mode")
        source_path_value = record["source_path"]
        if not isinstance(source_path_value, str):
            raise ReceiptError(f"bootstrap {label} source path is not text")
        source_path = Path(source_path_value)
        source = _read_evidence_snapshot(
            source_path,
            f"bootstrap trusted {label} source",
            maximum_bytes=maximum_bytes,
            expected_mode=source_mode,
            allowed_owners={0, os.geteuid()},
            require_single_link=False,
            executable=label in executable_labels,
        )
        if source.path == archive.path or candidate_root == source.path or candidate_root in source.path.parents:
            raise ReceiptError(f"bootstrap trusted {label} source crosses candidate boundary")
        expected_record = {
            "archive_name": archive_name,
            "archive_mode": f"{archive_mode:04o}",
            "observed_sha256": source.sha256,
            "protected_sha256": source.sha256,
            "size_bytes": source.size,
            "source_mode": f"{source.mode:04o}",
            "source_path": str(source.path),
        }
        if type(record["size_bytes"]) is not int or record != expected_record:
            raise ReceiptError(f"bootstrap trusted input {label} record is not exact")
        if archive.data != source.data:
            raise ReceiptError(f"bootstrap archived {label} differs from protected source")
        if label == "bootstrap" and source.sha256 != _FROZEN_BOOTSTRAP_SHA256:
            raise ReceiptError("bootstrap trusted source is not the frozen trust root")
        trusted_digests[label] = source.sha256
        trusted_sources[label] = _snapshot_receipt_artifact(source)

    identity_records = _require_exact_json_fields(
        marker["identity_verification"],
        set(_BOOTSTRAP_IDENTITY_ARCHIVES),
        "bootstrap identity verification inventory",
    )
    identity_snapshots: dict[str, EvidenceSnapshot] = {
        "trusted_allowed_signers": snapshots["trusted_allowed_signers"],
        "trusted_revocation": snapshots["trusted_revocation"],
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
            "argv",
            "closed_path_resolution",
            "environment_without_self_digest",
            "mode",
            "output",
            "path",
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
        runner["path"] != str(runner_path)
        or runner["sha256"] != runner_snapshot.sha256
        or type(runner["size_bytes"]) is not int
        or runner["size_bytes"] != runner_snapshot.size
        or runner["argv"]
        != [str(directory / "bash"), str(runner_path), "--release"]
        or runner["closed_path_resolution"]
        != {
            "bash": str(directory / "bash"),
            "git": str(directory / "git"),
            "python3": str(directory / "python3"),
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
        {"stderr_path", "stdout_path", "active_mode", "sealed_mode"},
        "bootstrap runner output",
    )
    if output_contract != {
        "stderr_path": str(directory / "runner-stderr.log"),
        "stdout_path": str(directory / "runner-stdout.log"),
        "active_mode": "0600",
        "sealed_mode": "0400",
    }:
        raise ReceiptError("bootstrap runner output contract is not exact")
    expected_log_mode = 0o400 if runner_logs_sealed else 0o600
    for output_path in (
        Path(output_contract["stdout_path"]),
        Path(output_contract["stderr_path"]),
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
    tool_directory, _ = _private_evidence_directory(
        Path(runner["tool_directory"]), "bootstrap runner tool directory"
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
                "alias_name",
                "alias_path",
                "sha256",
                "size_bytes",
                "source_mode",
                "source_path",
            },
            f"bootstrap runner tool {name}",
        )
        alias_path = tool_directory / name
        alias_metadata = alias_path.lstat()
        source_path = marker_record["source_path"]
        if not isinstance(source_path, str) or manifest_record.get("path") != source_path:
            raise ReceiptError(f"bootstrap runner tool {name} source path is wrong")
        if (
            marker_record["alias_name"] != name
            or marker_record["alias_path"] != str(alias_path)
            or not stat.S_ISLNK(alias_metadata.st_mode)
            or alias_metadata.st_uid != os.geteuid()
            or alias_metadata.st_nlink != 1
            or os.readlink(alias_path) != source_path
        ):
            raise ReceiptError(f"bootstrap runner tool {name} alias binding is wrong")
        source_mode = _octal_mode(
            marker_record["source_mode"], f"bootstrap runner tool {name} source mode"
        )
        source = _bounded_path_contract(
            Path(source_path),
            f"bootstrap runner tool source {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            expected_mode=source_mode,
            allowed_owners={0, os.geteuid()},
            require_single_link=False,
            executable=True,
        )
        runner_tool_total_bytes += source.size
        if runner_tool_total_bytes > _MAX_RUNNER_TOOL_TOTAL_BYTES:
            raise ReceiptError(
                "bootstrap runner tools exceed their aggregate byte limit"
            )
        if source.mode & 0o022:
            raise ReceiptError(f"bootstrap runner tool {name} source is writable")
        for ancestor in (source.path.parent, *source.path.parent.parents):
            ancestor_metadata = ancestor.lstat()
            if (
                stat.S_ISLNK(ancestor_metadata.st_mode)
                or not stat.S_ISDIR(ancestor_metadata.st_mode)
                or ancestor_metadata.st_uid not in {0, os.geteuid()}
                or stat.S_IMODE(ancestor_metadata.st_mode) & 0o022
            ):
                raise ReceiptError(
                    f"bootstrap runner tool {name} has an unsafe ancestor"
                )
        expected_record = {
            "alias_name": name,
            "alias_path": str(alias_path),
            "sha256": source.sha256,
            "size_bytes": source.size,
            "source_mode": f"{source.mode:04o}",
            "source_path": str(source.path),
        }
        if (
            marker_record != expected_record
            or manifest_record.get("sha256") != source.sha256
            or alias_path.resolve(strict=True) != source.path
        ):
            raise ReceiptError(f"bootstrap runner tool {name} integrity binding is wrong")
        runner_tool_sources[name] = source
    closed_path_entries = [str(directory), str(tool_directory)]

    environment = runner["environment_without_self_digest"]
    if not isinstance(environment, dict) or any(
        not isinstance(key, str)
        or not isinstance(value, str)
        or "\0" in key
        or "\0" in value
        for key, value in environment.items()
    ):
        raise ReceiptError("bootstrap runner environment is malformed")
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
    fixed_keys = set(base_environment) | set(policy_environment) | set(alias_environment)
    extras = {key: value for key, value in environment.items() if key not in fixed_keys}
    if any(
        _BOOTSTRAP_RUNNER_ENV_RE.fullmatch(key) is None
        or key not in _BOOTSTRAP_RUNNER_ENV_ALLOWLIST
        for key in extras
    ) or environment != {**base_environment, **extras, **policy_environment, **alias_environment}:
        raise ReceiptError("bootstrap runner environment is not the closed frozen environment")
    if environment.get("IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST") != str(
        expected_scaling_manifest_path
    ):
        raise ReceiptError(
            "bootstrap runner G-SCALE manifest is not the receipt manifest"
        )
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
    for command, expected in runner["closed_path_resolution"].items():
        discovered = shutil.which(command, path=environment["PATH"])
        if discovered is None or Path(discovered).resolve(strict=True) != Path(expected):
            raise ReceiptError(f"bootstrap closed PATH does not resolve protected {command}")

    probes = _require_exact_json_fields(
        marker["trusted_execution_probes"],
        {"bash", "python"},
        "bootstrap trusted execution probes",
    )
    expected_probes = {
        "bash": {
            "argv": [str(directory / "bash"), "-c", ":"],
            "exit_status": 0,
        },
        "python": {
            "argv": [
                str(directory / "python3"),
                "-I",
                "-S",
                "-c",
                "raise SystemExit(0)",
            ],
            "exit_status": 0,
        },
    }
    if probes != expected_probes or any(
        type(probes[label]["exit_status"]) is not int for label in probes
    ):
        raise ReceiptError("bootstrap trusted execution probes are not exact")

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
        "schema_version": 1,
        "completion_sha256": marker_snapshot.sha256,
        "frozen_bootstrap_sha256": _FROZEN_BOOTSTRAP_SHA256,
        "candidate_root": str(candidate_root),
        "candidate_identity_sha256": identity_snapshot.sha256,
        "candidate_commit_oid": candidate["head_commit"],
        "candidate_tree_oid": candidate["head_tree"],
        "runner": {
            "path": str(runner_path),
            "sha256": runner_snapshot.sha256,
            "mode": f"{runner_snapshot.mode:04o}",
            "argv": runner["argv"],
            "closed_path_resolution": runner["closed_path_resolution"],
            "output": output_contract,
            "tool_directory": str(tool_directory),
            "tools": runner_tools,
            "self_digest_environment_variables": runner[
                "self_digest_environment_variables"
            ],
        },
        "signer_fingerprint": expected_signer_fingerprint,
        "allowed_signers_principal": verification["allowed_signers_principal"],
        "trusted_input_digests": trusted_digests,
        "trusted_input_sources": trusted_sources,
    }
    bootstrap_evidence = {
        "completion": _snapshot_receipt_artifact(marker_snapshot),
        "candidate_identity": _snapshot_receipt_artifact(identity_snapshot),
        "runner": _snapshot_receipt_artifact(runner_snapshot),
        "candidate_cargo_lock": _snapshot_receipt_artifact(candidate_lock),
        "trusted_inputs": {
            label: _snapshot_receipt_artifact(snapshots["trusted_" + label])
            for label in _BOOTSTRAP_TRUSTED_ARCHIVES
        },
        "identity_verification": {
            label: _snapshot_receipt_artifact(snapshot)
            for label, snapshot in identity_snapshots.items()
            if not label.startswith("trusted_")
        },
        "runner_tools": {
            label: _snapshot_receipt_artifact(snapshot)
            for label, snapshot in sorted(runner_tool_sources.items())
        },
    }
    return authentication, bootstrap_evidence


def _load_identity(
    path: Path, name: str
) -> tuple[EvidenceSnapshot, dict[str, Any]]:
    snapshot = _bounded_evidence_snapshot(
        path,
        name,
        maximum_bytes=_MAX_SIGNATURE_JSON_BYTES,
    )
    value = _decode_canonical_json(snapshot.data, name)
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise ReceiptError(f"{name} fields do not match the release identity schema")
    if type(value.get("schema_version")) is not int or value["schema_version"] != 1:
        raise ReceiptError(f"{name} has the wrong schema version")
    for field in ("head_commit", "head_tree", "index_tree"):
        item = value.get(field)
        if not isinstance(item, str) or not _OBJECT_ID_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase Git object ID")
    object_widths = {
        len(value[field]) for field in ("head_commit", "head_tree", "index_tree")
    }
    if len(object_widths) != 1:
        raise ReceiptError(f"{name} mixes Git object formats")
    for field in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        item = value.get(field)
        if not isinstance(item, str) or not _DIGEST_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase SHA-256 digest")
    if value["head_tree"] != value["index_tree"]:
        raise ReceiptError(f"{name} does not describe one clean Git tree")
    return snapshot, value


def _load_tsv(
    path: Path,
    name: str,
    *,
    maximum_bytes: int = _MAX_RELEASE_TSV_BYTES,
) -> tuple[EvidenceSnapshot, dict[str, str]]:
    snapshot = _bounded_evidence_snapshot(
        path,
        name,
        maximum_bytes=maximum_bytes,
    )
    return snapshot, _tsv_fields_from_snapshot(snapshot, name)


def _require_fields(fields: dict[str, str], expected: set[str], name: str) -> None:
    if set(fields) != expected:
        raise ReceiptError(f"{name} fields do not match its completion schema")


def _artifact(snapshot: EvidenceSnapshot | PathContract) -> dict[str, str]:
    return {"path": str(snapshot.path), "sha256": snapshot.sha256}


def _execute_release_receipt_component(filename: str) -> None:
    """Execute one reviewed receipt component in this module namespace."""

    if (
        filename not in _RELEASE_RECEIPT_COMPONENT_FILES
        or Path(filename).name != filename
    ):
        raise RuntimeError(f"invalid release receipt component: {filename!r}")
    path = Path(__file__).with_name(filename)
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"release receipt component is unavailable: {path}")
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise RuntimeError(
            f"release receipt component could not be read: {path}"
        ) from error
    exec(compile(source, str(path), "exec"), globals())


for _release_receipt_component in _RELEASE_RECEIPT_COMPONENT_FILES:
    _execute_release_receipt_component(_release_receipt_component)

for _release_receipt_symbol in (
    "_validate_multilane_apalache_evidence",
    "_validate_formal_snapshot_replays",
    "_formal_artifacts",
    "_test_count_from_log",
):
    if not callable(globals().get(_release_receipt_symbol)):
        raise RuntimeError(
            "release receipt component lacks required symbol "
            f"{_release_receipt_symbol}"
        )


def _prebuilt_directory(path: Path, name: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o500
        or metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            f"{name} must be an owner-owned resolved non-symlink directory "
            "with exact mode 0500"
        )
    return path


def _prebuilt_workspace_target(repo_root: Path) -> Path:
    alias = repo_root / "target"
    try:
        alias_metadata = alias.lstat()
        resolved = alias.resolve(strict=True)
        resolved_metadata = resolved.lstat()
    except (OSError, RuntimeError) as error:
        raise ReceiptError("release workspace target authority is unavailable") from error
    if (
        not (stat.S_ISDIR(alias_metadata.st_mode) or stat.S_ISLNK(alias_metadata.st_mode))
        or Path(os.path.abspath(resolved)) != resolved
        or resolved.resolve(strict=True) != resolved
        or stat.S_ISLNK(resolved_metadata.st_mode)
        or not stat.S_ISDIR(resolved_metadata.st_mode)
        or resolved_metadata.st_uid != os.geteuid()
        or (not stat.S_ISLNK(alias_metadata.st_mode) and resolved != alias)
    ):
        raise ReceiptError(
            "release workspace target authority must resolve to one owner-owned real directory"
        )
    return resolved


def _prebuilt_directory_inventory(
    path: Path, expected_names: set[str], name: str
) -> None:
    names: list[str] = []
    try:
        with os.scandir(path) as iterator:
            for entry in iterator:
                if len(names) >= len(expected_names):
                    raise ReceiptError(
                        f"{name} contains more entries than its exact closed inventory"
                    )
                if (
                    _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(entry.name) is None
                    and entry.name != _PREBUILT_MANIFEST_NAME
                ):
                    raise ReceiptError(f"{name} does not have its exact closed inventory")
                names.append(entry.name)
    except OSError as error:
        raise ReceiptError(f"{name} cannot be enumerated") from error
    if len(names) != len(set(names)) or set(names) != expected_names:
        raise ReceiptError(f"{name} does not have its exact closed inventory")


def _prebuilt_version_transcripts(
    *,
    bundle_dir: Path,
    fields: dict[str, str],
    corridor_fields: dict[str, str],
) -> dict[str, dict[str, Any]]:
    tool_specs = (
        (
            "cargo",
            Path(corridor_fields["cargo_path"]),
            ["--version"],
            fields["cargo_version_sha256"],
        ),
        (
            "rustc",
            Path(corridor_fields["rustc_path"]),
            ["-vV"],
            fields["rustc_version_sha256"],
        ),
    )
    results: dict[str, dict[str, Any]] = {}
    environment = _closed_replay_environment(bundle_dir)
    for tool, executable, arguments, expected_digest in tool_specs:
        tool_label = "Cargo" if tool == "cargo" else tool
        contract = _capture_path_contract(
            executable,
            f"authenticated corridor {tool} tool",
            expected_sha256=corridor_fields[f"{tool}_sha256"],
            expected_owner=os.geteuid(),
            expected_nlink=1,
        )
        if contract.mode & 0o111 == 0:
            raise ReceiptError(f"authenticated corridor {tool} tool is not executable")
        status, stdout, stderr = _run_bounded_replay(
            executable,
            arguments,
            cwd=bundle_dir,
            environment=environment,
            name=f"authenticated {tool} version probe",
            maximum_output_bytes=_MAX_PREBUILT_VERSION_TRANSCRIPT_BYTES,
            executable_contract=contract,
        )
        if status != 0 or stderr or not stdout.endswith(b"\n"):
            raise ReceiptError(
                f"authenticated {tool} version probe did not produce exact stdout"
            )
        if b"\r" in stdout or b"\0" in stdout:
            raise ReceiptError(
                f"authenticated {tool} version probe output is not LF-only text"
            )
        observed_digest = hashlib.sha256(stdout).hexdigest()
        if observed_digest != expected_digest:
            raise ReceiptError(
                f"prebuilt manifest {tool_label} version digest does not match "
                "the authenticated tool"
            )
        try:
            lines = stdout.decode("utf-8").splitlines()
        except UnicodeDecodeError as error:
            raise ReceiptError(
                f"authenticated {tool} version probe output is not UTF-8"
            ) from error
        if tool == "cargo":
            if lines != [corridor_fields["cargo_version"]]:
                raise ReceiptError(
                    "authenticated Cargo version probe disagrees with corridor"
                )
        else:
            version = re.fullmatch(
                r"rustc ([0-9]+\.[0-9]+\.[0-9]+) "
                r"\(([0-9a-f]{7,40}) ([0-9]{4}-[0-9]{2}-[0-9]{2})\)",
                corridor_fields["rustc_version"],
            )
            expected_keys = (
                "binary",
                "commit-hash",
                "commit-date",
                "host",
                "release",
                "LLVM version",
            )
            parsed: dict[str, str] = {}
            if (
                version is None
                or not lines
                or lines[0] != corridor_fields["rustc_version"]
            ):
                raise ReceiptError(
                    "authenticated rustc version probe has the wrong version line"
                )
            for line in lines[1:]:
                key, separator, value = line.partition(": ")
                if not separator or key in parsed or not value:
                    raise ReceiptError(
                        "authenticated rustc version probe is not exact rustc -vV output"
                    )
                parsed[key] = value
            if (
                tuple(parsed) != expected_keys
                or parsed["binary"] != "rustc"
                or re.fullmatch(r"[0-9a-f]{40}", parsed["commit-hash"]) is None
                or not parsed["commit-hash"].startswith(version.group(2))
                or parsed["commit-date"] != version.group(3)
                or parsed["host"] != fields["host_triple"]
                or parsed["release"] != version.group(1)
                or re.fullmatch(
                    r"[0-9]+\.[0-9]+(?:\.[0-9]+)?",
                    parsed["LLVM version"],
                )
                is None
            ):
                raise ReceiptError(
                    "authenticated rustc version probe is not exact rustc -vV output"
                )
        after = _capture_path_contract(
            executable,
            f"authenticated corridor {tool} tool after version probe",
            expected_sha256=contract.sha256,
            expected_mode=contract.mode,
            expected_owner=contract.owner,
            expected_nlink=contract.nlink,
            expected_size=contract.size,
        )
        if after != contract:
            raise ReceiptError(
                f"authenticated corridor {tool} tool changed during version probe"
            )
        results[tool] = {
            "argv": [str(executable), *arguments],
            "sha256": observed_digest,
            "size_bytes": len(stdout),
        }
    return results


def _prebuilt_binary_bundle(
    *,
    manifest_path: Path,
    expected_manifest_sha256: str,
    fields: dict[str, str],
    sealed: dict[str, Any],
    repo_root: Path,
) -> dict[str, Any]:
    expected_manifest_sha256 = _require_digest(
        expected_manifest_sha256, "prebuilt binary manifest digest"
    )
    if manifest_path.name != _PREBUILT_MANIFEST_NAME:
        raise ReceiptError("prebuilt binary manifest has the wrong filename")
    workspace_target = _prebuilt_workspace_target(repo_root)
    expected_programs = (
        workspace_target
        / "sumeragi-v2-release"
        / sealed["workspace_source_manifest_sha256"]
        / "programs"
    )
    bundle_dir = manifest_path.parent
    if (
        bundle_dir.parent != expected_programs
        or _PREBUILT_INVOCATION_RE.fullmatch(bundle_dir.name) is None
    ):
        raise ReceiptError(
            "prebuilt binary manifest is outside its exact source-bound "
            "invocation bundle"
        )
    for path, name in (
        (bundle_dir, "prebuilt invocation bundle"),
        (bundle_dir / "release", "prebuilt release directory"),
        (bundle_dir / "message-control", "prebuilt message-control directory"),
        (
            bundle_dir / "message-control" / "release",
            "prebuilt message-control release directory",
        ),
    ):
        _prebuilt_directory(path, name)
    _prebuilt_directory_inventory(
        bundle_dir,
        {_PREBUILT_MANIFEST_NAME, "release", "message-control"},
        "prebuilt invocation bundle",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "release",
        {"irohad", "iroha", "kagami"},
        "prebuilt release directory",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "message-control",
        {"release"},
        "prebuilt message-control directory",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "message-control" / "release",
        {"irohad"},
        "prebuilt message-control release directory",
    )

    manifest = _read_evidence_snapshot(
        manifest_path,
        "prebuilt binary manifest",
        maximum_bytes=_MAX_PREBUILT_MANIFEST_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    if manifest.sha256 != expected_manifest_sha256:
        raise ReceiptError(
            "prebuilt binary manifest does not match its externally carried digest"
        )
    rows = _decode_g12_tsv(manifest, "prebuilt binary manifest")
    if (
        len(rows) != len(_PREBUILT_MANIFEST_FIELDS)
        or tuple(row[0] for row in rows) != _PREBUILT_MANIFEST_FIELDS
        or any(len(row) != 2 for row in rows)
    ):
        raise ReceiptError(
            "prebuilt binary manifest does not contain its exact ordered 25 fields"
        )
    manifest_fields = {row[0]: row[1] for row in rows}
    canonical_data = "".join(
        f"{name}\t{manifest_fields[name]}\n"
        for name in _PREBUILT_MANIFEST_FIELDS
    ).encode("utf-8")
    if manifest.data != canonical_data:
        raise ReceiptError("prebuilt binary manifest TSV is not canonical")
    if (
        manifest_fields["schema_version"] != "2"
        or manifest_fields["source_manifest_sha256"]
        != sealed["workspace_source_manifest_sha256"]
        or manifest_fields["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or manifest_fields["profile"] != "release"
        or manifest_fields["bundle_dir"] != str(bundle_dir)
        or _PREBUILT_TRIPLE_RE.fullmatch(manifest_fields["host_triple"]) is None
        or manifest_fields["target_triple"] != manifest_fields["host_triple"]
    ):
        raise ReceiptError(
            "prebuilt binary manifest is not bound to the exact release identity"
        )
    for name in ("cargo_version_sha256", "rustc_version_sha256"):
        _require_digest(manifest_fields[name], f"prebuilt manifest {name}")
    cargo_lock = _read_evidence_snapshot(
        repo_root / "Cargo.lock",
        "retained release Cargo.lock",
        maximum_bytes=_MAX_LOCK_BYTES,
        allowed_owners={os.geteuid()},
    )
    if cargo_lock.sha256 != manifest_fields["cargo_lock_sha256"]:
        raise ReceiptError(
            "prebuilt binary manifest Cargo.lock digest does not match retained source"
        )

    binaries: list[dict[str, Any]] = []
    for prefix, relative in _PREBUILT_BINARY_SPECS:
        size_text = manifest_fields[f"{prefix}_size_bytes"]
        if (
            re.fullmatch(r"[1-9][0-9]*", size_text) is None
            or int(size_text) > _MAX_PREBUILT_BINARY_BYTES
            or manifest_fields[f"{prefix}_relative_path"] != relative
            or manifest_fields[f"{prefix}_mode_octal"] != "0500"
        ):
            raise ReceiptError(
                f"prebuilt manifest {prefix} metadata is not exact and bounded"
            )
        digest = _require_digest(
            manifest_fields[f"{prefix}_sha256"],
            f"prebuilt manifest {prefix} digest",
        )
        pure_relative = PurePosixPath(relative)
        binary = _read_evidence_snapshot(
            bundle_dir.joinpath(*pure_relative.parts),
            f"prebuilt {prefix} binary",
            maximum_bytes=_MAX_PREBUILT_BINARY_BYTES,
            expected_mode=0o500,
            allowed_owners={os.geteuid()},
            executable=True,
            retain_bytes=False,
        )
        if binary.sha256 != digest or binary.size != int(size_text):
            raise ReceiptError(
                f"prebuilt {prefix} binary identity does not match manifest"
            )
        binaries.append(
            {
                "role": prefix,
                "relative_path": relative,
                **_snapshot_receipt_artifact(binary),
            }
        )

    return {
        "schema_version": 2,
        "manifest": _snapshot_receipt_artifact(manifest),
        "source_manifest_sha256": manifest_fields["source_manifest_sha256"],
        "cargo_lock_sha256": manifest_fields["cargo_lock_sha256"],
        "cargo_version_sha256": manifest_fields["cargo_version_sha256"],
        "rustc_version_sha256": manifest_fields["rustc_version_sha256"],
        "host_triple": manifest_fields["host_triple"],
        "target_triple": manifest_fields["target_triple"],
        "profile": manifest_fields["profile"],
        "bundle_dir": str(bundle_dir),
        "version_transcripts": _prebuilt_version_transcripts(
            bundle_dir=bundle_dir,
            fields=manifest_fields,
            corridor_fields=fields,
        ),
        "binaries": binaries,
    }


def _corridor_artifacts(
    completion: EvidenceSnapshot,
    fields: dict[str, str],
    sealed: dict[str, Any],
    repo_root: Path,
    bootstrap_runner_tools: dict[str, Any],
) -> tuple[
    PathContract,
    PathContract,
    PathContract,
    list[PathContract],
    dict[str, Any],
]:
    completion_path = completion.path
    _require_fields(
        fields,
        {
            "schema_version",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "leg_count",
            "production_required_test_count",
            "g_unit_expected_test_count",
            "g_unit_passed_test_count",
            "summary_sha256",
            "production_required_tests_sha256",
            "g_unit_inventory_sha256",
            "java_path",
            "java_sha256",
            "cargo_path",
            "cargo_sha256",
            "cargo_version",
            "rustc_path",
            "rustc_sha256",
            "rustc_version",
            "python3_path",
            "python3_sha256",
            "node_path",
            "node_sha256",
            "swift_path",
            "swift_sha256",
            "swift_version",
            "bash_path",
            "bash_sha256",
            "git_path",
            "git_sha256",
            "cargo_home_path",
            "repo_cargo_config_sha256",
            "native_amx_grouped_fixture_sha256",
            "native_amx_grouped_suite_source_manifest_sha256",
            "native_amx_grouped_negative_control_count",
            "tlc_profile",
            "tlaps_threads",
            "prebuilt_manifest_path",
            "prebuilt_manifest_sha256",
        },
        "corridor completion",
    )
    expected_identity = {
        "schema_version": "1",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "leg_count": str(len(_corridor_legs())),
        "production_required_test_count": str(_PRODUCTION_TEST_COUNT),
        "g_unit_expected_test_count": str(_G_UNIT_TEST_COUNT),
        "g_unit_passed_test_count": str(_G_UNIT_TEST_COUNT),
        "native_amx_grouped_negative_control_count": str(
            _NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT
        ),
        "tlc_profile": "ci",
        "tlaps_threads": "1",
    }
    if any(fields.get(name) != value for name, value in expected_identity.items()):
        raise ReceiptError("corridor completion is not the exact release preflight")
    if (
        fields["cargo_version"] != "cargo 1.93.1 (083ac5135 2025-12-15)"
        or fields["rustc_version"]
        != "rustc 1.93.1 (01f6ddf75 2026-02-11)"
    ):
        raise ReceiptError("corridor Rust tools do not match rust-toolchain.toml")
    for tool in ("cargo", "rustc"):
        runner_record = bootstrap_runner_tools.get(tool)
        if (
            not isinstance(runner_record, dict)
            or fields[f"{tool}_path"] != runner_record.get("source_path")
            or fields[f"{tool}_sha256"] != runner_record.get("sha256")
        ):
            raise ReceiptError(
                f"corridor {tool} is not the authenticated bootstrap runner tool"
            )
    for tool in (
        "java",
        "cargo",
        "rustc",
        "python3",
        "node",
        "swift",
        "bash",
        "git",
    ):
        tool_path = Path(fields[f"{tool}_path"])
        if not tool_path.is_absolute():
            raise ReceiptError(f"corridor {tool} path is not absolute")
        tool_contract = _bounded_path_contract(
            tool_path,
            f"corridor {tool} tool",
            maximum_bytes=_MAX_TOOL_BYTES,
            require_single_link=False,
        )
        digest = fields[f"{tool}_sha256"]
        if not _DIGEST_RE.fullmatch(digest) or tool_contract.sha256 != digest:
            raise ReceiptError(f"corridor {tool} tool digest mismatch")
    if not fields["swift_version"].strip():
        raise ReceiptError("corridor Swift tool version is blank")
    cargo_home = Path(fields["cargo_home_path"])
    if (
        not cargo_home.is_absolute()
        or not cargo_home.is_dir()
        or cargo_home.is_symlink()
    ):
        raise ReceiptError("corridor Cargo home is not an isolated directory")
    for config_name in ("config", "config.toml"):
        config = cargo_home / config_name
        if config.exists() or config.is_symlink():
            raise ReceiptError("corridor Cargo home contains external configuration")
    repo_cargo_config = _bounded_path_contract(
        repo_root / ".cargo" / "config.toml",
        "repository Cargo config",
        maximum_bytes=_MAX_POLICY_BYTES,
    )
    if (
        not _DIGEST_RE.fullmatch(fields["repo_cargo_config_sha256"])
        or repo_cargo_config.sha256 != fields["repo_cargo_config_sha256"]
    ):
        raise ReceiptError("repository Cargo config digest mismatch")
    grouped_fixture = _bounded_path_contract(
        repo_root / _NATIVE_AMX_GROUPED_FIXTURE,
        "grouped Native AMX V2 fixture",
        maximum_bytes=_MAX_LOCALNET_MANIFEST_BYTES,
    )
    if (
        not _DIGEST_RE.fullmatch(fields["native_amx_grouped_fixture_sha256"])
        or grouped_fixture.sha256
        != fields["native_amx_grouped_fixture_sha256"]
    ):
        raise ReceiptError("grouped Native AMX V2 fixture digest mismatch")
    expected_suite_manifest = _native_amx_grouped_suite_source_manifest(repo_root)
    if (
        not _DIGEST_RE.fullmatch(
            fields["native_amx_grouped_suite_source_manifest_sha256"]
        )
        or fields["native_amx_grouped_suite_source_manifest_sha256"]
        != expected_suite_manifest
    ):
        raise ReceiptError(
            "grouped Native AMX V2 suite-source manifest digest mismatch"
        )

    release_runner = _bounded_evidence_snapshot(
        repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
        "release runner inventory",
        maximum_bytes=_MAX_POLICY_BYTES,
        require_single_link=False,
    )
    summary = _bounded_evidence_snapshot(
        completion_path.with_name("summary.tsv"),
        "corridor summary",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    required_snapshot = _bounded_evidence_snapshot(
        completion_path.with_name("production-required-tests.tsv"),
        "corridor production inventory",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    g_unit_snapshot = _bounded_evidence_snapshot(
        completion_path.with_name("g-unit-required-tests.tsv"),
        "corridor G-UNIT inventory",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    if summary.sha256 != fields["summary_sha256"]:
        raise ReceiptError("corridor summary digest mismatch")
    if required_snapshot.sha256 != fields["production_required_tests_sha256"]:
        raise ReceiptError("corridor production inventory digest mismatch")
    if g_unit_snapshot.sha256 != fields["g_unit_inventory_sha256"]:
        raise ReceiptError("corridor G-UNIT inventory digest mismatch")

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(
                    required_snapshot, "corridor production inventory"
                ),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != ("module", "test"):
            raise ReceiptError("corridor production inventory fields are not canonical")
        required_rows = list(reader)
    except csv.Error as error:
        raise ReceiptError(
            "corridor production inventory is malformed TSV"
        ) from error
    if len(required_rows) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError(
            "corridor production inventory must contain exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    required_names = [row.get("test", "") for row in required_rows]
    if len(set(required_names)) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError("corridor production inventory contains duplicate tests")
    if required_names != _canonical_production_tests(repo_root, release_runner):
        raise ReceiptError("corridor production inventory is not the canonical release list")
    module_counts = {module: count for _, module, count in _PRODUCTION_MODULES}
    required_by_module: dict[str, list[str]] = {module: [] for module in module_counts}
    for row in required_rows:
        if None in row or set(row) != {"module", "test"}:
            raise ReceiptError("corridor production inventory has extra columns")
        module = row["module"]
        test = row["test"]
        if module not in module_counts or not test.startswith(f"{module}::"):
            raise ReceiptError("corridor production inventory has an invalid module binding")
        required_by_module[module].append(test)
    if any(
        len(required_by_module[module]) != expected
        for module, expected in module_counts.items()
    ):
        raise ReceiptError("corridor production inventory module counts are not exact")

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(g_unit_snapshot, "corridor G-UNIT inventory"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != ("leg_id", "crate", "test"):
            raise ReceiptError("corridor G-UNIT inventory fields are not canonical")
        g_unit_rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("corridor G-UNIT inventory is malformed TSV") from error
    canonical_g_unit_rows = _canonical_g_unit_rows(repo_root, release_runner)
    if len(g_unit_rows) != _G_UNIT_TEST_COUNT:
        raise ReceiptError(
            f"corridor G-UNIT inventory must contain exactly "
            f"{_G_UNIT_TEST_COUNT} tests"
        )
    for index, (row, expected_row) in enumerate(
        zip(g_unit_rows, canonical_g_unit_rows)
    ):
        expected_leg, expected_package, expected_test = expected_row
        if (
            None in row
            or set(row) != {"leg_id", "crate", "test"}
            or row
            != {
                "leg_id": expected_leg,
                "crate": expected_package,
                "test": expected_test,
            }
        ):
            raise ReceiptError(
                f"corridor G-UNIT inventory row {index} is not canonical"
            )

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(summary, "corridor summary"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != _CORRIDOR_SUMMARY_FIELDS:
            raise ReceiptError("corridor summary fields are not canonical")
        rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("corridor summary is malformed TSV") from error
    expected_legs = _corridor_legs()
    if len(rows) != len(expected_legs):
        raise ReceiptError("corridor summary must contain every exact release leg")
    logs: list[PathContract] = []
    module_for_leg = {leg_id: module for leg_id, module, _ in _PRODUCTION_MODULES}
    g_unit_tests_by_leg: dict[str, list[str]] = {
        leg_id: [] for _, leg_id, _, _, _ in _G_UNIT_GROUPS
    }
    for leg_id, _, test in canonical_g_unit_rows:
        g_unit_tests_by_leg[leg_id].append(test)
    exact_cargo_tests: dict[str, tuple[str, ...]] = {
        "status-rust": (_DATA_STATUS_TEST,),
        "lane-certificate-rust": (_DATA_LANE_CERTIFICATE_TEST,),
        "cross-sdk-rust": _CROSS_SDK_TESTS,
        "sumeragi-diagnostics-rust": _RUST_SDK_DIAGNOSTICS_TESTS,
    }
    exact_cargo_tests.update(
        {
            f"taira-contract-{index}": (test,)
            for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
        }
    )
    # The exact-length equality above provides the same fail-closed guarantee
    # as ``zip(strict=True)`` while retaining the repository's Python 3.9
    # compatibility.
    for index, (row, expected_leg) in enumerate(zip(rows, expected_legs)):
        leg_id, kind, required_count, command = expected_leg
        expected_log = f"logs/{index:02d}-{leg_id}.log"
        expected_row = {
            "leg_index": str(index),
            "leg_id": leg_id,
            "kind": kind,
            "required_test_count": str(required_count),
            "command_status": "0",
            "tee_status": "0",
            "log": expected_log,
            "command": command,
        }
        if None in row or set(row) != set(_CORRIDOR_SUMMARY_FIELDS) or any(
            row.get(name) != value for name, value in expected_row.items()
        ):
            raise ReceiptError(f"corridor summary row {index} is not the exact release leg")
        digest = row.get("log_sha256", "")
        if not _DIGEST_RE.fullmatch(digest):
            raise ReceiptError(f"corridor summary row {index} has an invalid log digest")
        log = _bounded_evidence_snapshot(
            completion_path.parent / expected_log,
            f"corridor log {index}",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if log.sha256 != digest:
            raise ReceiptError(f"corridor log {index} digest mismatch")
        lines = _decode_lf_text(log, f"corridor log {index}").splitlines()
        observed = _test_count_from_log(lines, kind, f"corridor log {index}")
        if row.get("observed_test_count") != str(observed):
            raise ReceiptError(f"corridor summary row {index} has the wrong observed count")
        if kind == "cargo-module":
            if observed == 0 or observed < required_count:
                raise ReceiptError(f"corridor module {leg_id} ran too few tests")
            module = module_for_leg[leg_id]
            for test in required_by_module[module]:
                if lines.count(f"test {test} ... ok") != 1:
                    raise ReceiptError(
                        f"corridor module {leg_id} lacks one required passing test"
                    )
        elif observed != required_count:
            raise ReceiptError(f"corridor leg {leg_id} has the wrong passing count")
        if kind == "cargo-focus":
            expected_tests = g_unit_tests_by_leg.get(leg_id)
            if expected_tests is None or len(expected_tests) != required_count:
                raise ReceiptError(
                    f"corridor G-UNIT leg {leg_id} has no exact inventory binding"
                )
            passing_tests = [
                match.group(1)
                for line in lines
                if (
                    match := re.fullmatch(
                        r"test ([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*) \.\.\. ok",
                        line,
                    )
                )
            ]
            if passing_tests != expected_tests:
                raise ReceiptError(
                    f"corridor G-UNIT leg {leg_id} lacks one required "
                    "passing test or contains an unexpected result"
                )
        if kind == "cargo-exact":
            for test in exact_cargo_tests[leg_id]:
                if lines.count(f"test {test} ... ok") != 1:
                    raise ReceiptError(
                        f"corridor exact Cargo leg {leg_id} lacks its named test"
                    )
        if kind == "native-amx-sdk":
            surface = leg_id.removeprefix("native-amx-grouped-")
            expected_marker = (
                f"native-amx-v2-grouped-parity surface={surface} "
                f"tests={observed} "
                "fixture_sha256="
                f"{fields['native_amx_grouped_fixture_sha256']} "
                "suite_source_manifest_sha256="
                f"{fields['native_amx_grouped_suite_source_manifest_sha256']}"
            )
            if lines.count(expected_marker) != 1:
                raise ReceiptError(
                    f"corridor grouped Native AMX V2 {surface} leg is not "
                    "bound to the exact fixture and suite sources"
                )
        if kind == "sdk-diagnostics":
            surface = leg_id.removeprefix("sumeragi-diagnostics-")
            expected_marker = (
                f"sumeragi-v2-sdk-diagnostics surface={surface} "
                f"tests={observed} suite_source_manifest_sha256="
                f"{_sumeragi_sdk_diagnostics_suite_source_manifest(repo_root)}"
            )
            if lines.count(expected_marker) != 1:
                raise ReceiptError(
                    f"corridor Sumeragi v2 SDK diagnostics {surface} leg is "
                    "not bound to the exact suite sources"
                )
        logs.append(_snapshot_contract(log))
    manifest_path = Path(fields["prebuilt_manifest_path"])
    prebuilt_bundle = _prebuilt_binary_bundle(
        manifest_path=manifest_path,
        expected_manifest_sha256=fields["prebuilt_manifest_sha256"],
        fields=fields,
        sealed=sealed,
        repo_root=repo_root,
    )
    return (
        _snapshot_contract(summary),
        _snapshot_contract(required_snapshot),
        _snapshot_contract(g_unit_snapshot),
        logs,
        prebuilt_bundle,
    )


def _seed_run_logs(
    seed_completion: EvidenceSnapshot,
    summary: EvidenceSnapshot,
    manifest: str,
    repo_root: Path,
    prebuilt_bundle_dir: Path,
    prebuilt_manifest_sha256: str,
) -> list[PathContract]:
    seed_path = seed_completion.path
    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(summary, "seed summary"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != _SEED_SUMMARY_FIELDS:
            raise ReceiptError("seed summary fields are not canonical")
        rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("seed summary is malformed TSV") from error
    if len(rows) != _SEED_RUN_COUNT:
        raise ReceiptError(
            f"seed summary must contain exactly {_SEED_RUN_COUNT} run rows"
        )

    run_logs = []
    source_bound_root = repo_root / "target" / "sumeragi-v2-release" / manifest
    cargo_target_dir = source_bound_root / "test-suite"
    program_target_dir = prebuilt_bundle_dir
    irohad = program_target_dir / "release" / "irohad"
    message_control_irohad = (
        program_target_dir / "message-control" / "release" / "irohad"
    )
    iroha = program_target_dir / "release" / "iroha"
    kagami = program_target_dir / "release" / "kagami"
    for index, row in enumerate(rows):
        if None in row or set(row) != set(_SEED_SUMMARY_FIELDS):
            raise ReceiptError(f"seed summary row {index} has extra or missing columns")
        scenario = _SEED_SCENARIOS[index // _SEED_RUNS_PER_SCENARIO]
        seed_index = index % _SEED_RUNS_PER_SCENARIO
        expected_seed = (
            scenario if seed_index == 0 else f"{scenario}:seed:{seed_index:02d}"
        )
        output = f"runs/run-{index:03d}.log"
        localnet = f"localnets/run-{index:03d}"
        expected_command = (
            f"CARGO_TARGET_DIR={cargo_target_dir} "
            f"IROHA_TEST_TARGET_DIR={program_target_dir} "
            f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={manifest} "
            f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={prebuilt_manifest_sha256} "
            f"TEST_NETWORK_BIN_IROHAD={irohad} "
            f"TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL={message_control_irohad} "
            f"TEST_NETWORK_BIN_IROHA={iroha} "
            f"KAGAMI_BIN={kagami} "
            "CARGO_NET_OFFLINE=true "
            "IROHA_TEST_REQUIRE_NETWORK=1 "
            "IROHA_TEST_NETWORK_START_ATTEMPTS=1 "
            "IROHA_TEST_SKIP_BUILD=1 "
            "IROHA_TEST_ALLOW_REENTRANT_BUILD=0 "
            "IROHA_TEST_BUILD_PROFILE=release "
            "PROFILE=release "
            "IROHA_TEST_BUILD_TIMEOUT_MS=3600 "
            "IROHA_TEST_PROCESS_TIMEOUT_MS=300 "
            "IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 "
            f"IROHA_TEST_NETWORK_BASE_SEED={expected_seed} "
            "TEST_NETWORK_TMP_DIR=${SEED_MATRIX_EVIDENCE_DIRECTORY}/"
            f"{localnet} "
            "IROHA_TEST_NETWORK_KEEP_DIRS=1 "
            "cargo test --locked --offline -p integration_tests --test "
            "sumeragi_v2_runner_isolated "
            f"sumeragi_v2_runner::{scenario} -- --exact --nocapture "
            "--test-threads=1"
        )
        if (
            row.get("profile") != "release"
            or row.get("source_manifest_sha256") != manifest
            or row.get("scenario") != scenario
            or row.get("seed") != expected_seed
            or row.get("result") != "passed"
            or row.get("cargo_status") != "0"
            or row.get("tee_status") != "0"
            or row.get("output") != output
            or row.get("localnet") != localnet
            or row.get("command") != expected_command
        ):
            raise ReceiptError(f"seed summary row {index} is not the exact release run")
        digest = row.get("run_log_sha256")
        if not isinstance(digest, str) or not _DIGEST_RE.fullmatch(digest):
            raise ReceiptError(f"seed summary row {index} has an invalid log digest")
        run_log = _bounded_evidence_snapshot(
            seed_path.parent / output,
            f"seed run log {index}",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if run_log.sha256 != digest:
            raise ReceiptError(f"seed run log {index} digest mismatch")
        lines = _decode_lf_text(run_log, f"seed run log {index}").splitlines()
        running = [
            line for line in lines if re.fullmatch(r"running [0-9]+ tests?", line)
        ]
        results = [line for line in lines if line.startswith("test result:")]
        passing = [
            line
            for line in results
            if re.fullmatch(
                r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
                r"[0-9]+ filtered out; finished in .+",
                line,
            )
        ]
        test_prefix = (
            f"test sumeragi_v2_runner::{scenario} ... "
            f"{scenario}: deterministic network seed = {expected_seed}"
        )
        prefix_positions = [
            position for position, line in enumerate(lines) if line == test_prefix
        ]
        ok_positions = [position for position, line in enumerate(lines) if line == "ok"]
        if (
            running != ["running 1 test"]
            or len(results) != 1
            or len(passing) != 1
            or len(prefix_positions) != 1
            or len(ok_positions) != 1
            or prefix_positions[0] >= ok_positions[0]
        ):
            raise ReceiptError(
                f"seed run log {index} does not prove its one exact passing scenario"
            )
        run_logs.append(_snapshot_contract(run_log))
    return run_logs


def _seed_localnet_manifests(
    seed_completion: EvidenceSnapshot, fields: dict[str, str]
) -> tuple[PathContract, list[PathContract]]:
    seed_path = seed_completion.path
    if (
        fields["localnet_manifest_count"] != str(_SEED_RUN_COUNT)
        or fields["localnet_manifests_path"] != "localnet-manifests.tsv"
        or not _DIGEST_RE.fullmatch(fields["localnet_manifests_sha256"])
    ):
        raise ReceiptError("seed completion has an invalid localnet manifest binding")
    index_snapshot = _bounded_evidence_snapshot(
        seed_path.parent / fields["localnet_manifests_path"],
        "seed localnet manifest index",
        maximum_bytes=_MAX_LOCALNET_MANIFEST_INDEX_BYTES,
    )
    if index_snapshot.sha256 != fields["localnet_manifests_sha256"]:
        raise ReceiptError("seed localnet manifest index digest mismatch")
    try:
        reader = csv.DictReader(
            io.StringIO(index_snapshot.data.decode("utf-8")), delimiter="\t"
        )
        if tuple(reader.fieldnames or ()) != _SEED_LOCALNET_MANIFEST_FIELDS:
            raise ReceiptError("seed localnet manifest index fields are not canonical")
        rows = list(reader)
    except UnicodeDecodeError as error:
        raise ReceiptError("seed localnet manifest index is not UTF-8") from error
    if len(rows) != _SEED_RUN_COUNT:
        raise ReceiptError(
            f"seed localnet manifest index must contain exactly {_SEED_RUN_COUNT} rows"
        )

    records: list[tuple[int, str, str, str]] = []
    canonical_index_lines = ["\t".join(_SEED_LOCALNET_MANIFEST_FIELDS)]
    for index, row in enumerate(rows):
        localnet = f"localnets/run-{index:03d}"
        relative_manifest = f"localnet-manifests/run-{index:03d}.tsv"
        path_field = f"localnet_manifest_{index:03d}_path"
        digest_field = f"localnet_manifest_{index:03d}_sha256"
        digest = row.get("manifest_sha256", "")
        expected_row = {
            "run_index": str(index),
            "localnet": localnet,
            "manifest": relative_manifest,
            "manifest_sha256": digest,
        }
        if (
            None in row
            or set(row) != set(_SEED_LOCALNET_MANIFEST_FIELDS)
            or row != expected_row
            or not _DIGEST_RE.fullmatch(digest)
            or fields[path_field] != relative_manifest
            or fields[digest_field] != digest
        ):
            raise ReceiptError(
                f"seed localnet manifest index row {index} is not canonical"
            )
        records.append((index, localnet, relative_manifest, digest))
        canonical_index_lines.append(
            "\t".join((str(index), localnet, relative_manifest, digest))
        )
    canonical_index = ("\n".join(canonical_index_lines) + "\n").encode("utf-8")
    if index_snapshot.data != canonical_index:
        raise ReceiptError("seed localnet manifest index bytes are not canonical")

    manifests: list[PathContract] = []
    for index, localnet, relative_manifest, digest in records:
        manifest_candidate = seed_path.parent / relative_manifest
        try:
            resolved_manifest = manifest_candidate.resolve(strict=True)
        except (OSError, RuntimeError) as error:
            raise ReceiptError(
                f"seed localnet manifest {index} is unavailable"
            ) from error
        if resolved_manifest != manifest_candidate:
            raise ReceiptError(f"seed localnet manifest {index} escaped its archive")
        snapshot = _bounded_evidence_snapshot(
            manifest_candidate,
            f"seed localnet manifest {index}",
            maximum_bytes=_MAX_LOCALNET_MANIFEST_BYTES,
        )
        if snapshot.sha256 != digest:
            raise ReceiptError(f"seed localnet manifest {index} digest mismatch")
        try:
            expected = canonical_localnet_manifest(seed_path.parent / localnet)
        except LocalnetManifestError as error:
            raise ReceiptError(
                f"seed retained localnet {index} is unsafe or unstable: {error}"
            ) from error
        if snapshot.data != expected:
            raise ReceiptError(
                f"seed localnet manifest {index} does not match retained content"
            )
        manifests.append(_snapshot_contract(snapshot))
    return _snapshot_contract(index_snapshot), manifests


def _scan_scaling_bundle(
    root: Path,
) -> tuple[list[tuple[str, Path, os.stat_result]], list[str], int]:
    try:
        root_metadata = root.lstat()
    except OSError as error:
        raise ReceiptError("scaling evidence bundle root is unavailable") from error
    if (
        root.resolve(strict=True) != root
        or stat.S_ISLNK(root_metadata.st_mode)
        or not stat.S_ISDIR(root_metadata.st_mode)
        or root_metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            "scaling evidence bundle root must be an owner-owned resolved "
            "non-symlink directory"
        )

    files: list[tuple[str, Path, os.stat_result]] = []
    directories: list[str] = []
    inodes: dict[tuple[int, int], str] = {}
    total_bytes = 0

    def visit(directory: Path, prefix: PurePosixPath | None) -> None:
        nonlocal total_bytes
        try:
            with os.scandir(directory) as iterator:
                entries = sorted(iterator, key=lambda entry: entry.name)
        except OSError as error:
            raise ReceiptError(
                "scaling evidence bundle directory cannot be enumerated"
            ) from error
        for entry in entries:
            component = entry.name
            if _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(component) is None:
                raise ReceiptError(
                    "scaling evidence bundle contains an unsafe path component"
                )
            relative_path = (
                PurePosixPath(component)
                if prefix is None
                else prefix / component
            )
            relative = relative_path.as_posix()
            path = directory / component
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise ReceiptError(
                    f"scaling evidence bundle entry is unavailable: {relative}"
                ) from error
            if stat.S_ISLNK(metadata.st_mode):
                raise ReceiptError(
                    f"scaling evidence bundle contains a symlink: {relative}"
                )
            if stat.S_ISDIR(metadata.st_mode):
                directories.append(relative)
                if len(directories) > _MAX_SCALING_BUNDLE_DIRECTORY_COUNT:
                    raise ReceiptError(
                        "scaling evidence bundle exceeds its directory-count limit"
                    )
                visit(path, relative_path)
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise ReceiptError(
                    f"scaling evidence bundle contains a nonregular entry: {relative}"
                )
            if metadata.st_uid != os.geteuid():
                raise ReceiptError(
                    f"scaling evidence bundle file has an untrusted owner: {relative}"
                )
            if metadata.st_nlink != 1:
                raise ReceiptError(
                    f"scaling evidence bundle file has a hard-link alias: {relative}"
                )
            inode = (metadata.st_dev, metadata.st_ino)
            alias = inodes.get(inode)
            if alias is not None:
                raise ReceiptError(
                    "scaling evidence bundle files are hard-link aliases: "
                    f"{alias} and {relative}"
                )
            inodes[inode] = relative
            if metadata.st_size > _MAX_SCALING_BUNDLE_FILE_BYTES:
                raise ReceiptError(
                    f"scaling evidence bundle file exceeds its size limit: {relative}"
                )
            total_bytes += metadata.st_size
            if total_bytes > _MAX_SCALING_BUNDLE_TOTAL_BYTES:
                raise ReceiptError(
                    "scaling evidence bundle exceeds its aggregate size limit"
                )
            files.append((relative, path, metadata))
            if len(files) > _MAX_SCALING_BUNDLE_FILE_COUNT:
                raise ReceiptError(
                    "scaling evidence bundle exceeds its file-count limit"
                )

    visit(root, None)
    files.sort(key=lambda item: item[0])
    directories.sort()
    return files, directories, total_bytes


def _capture_scaling_bundle(
    root: Path,
) -> tuple[list[tuple[str, PathContract]], list[str], int]:
    scanned, directories, _ = _scan_scaling_bundle(root)
    directory_contracts = [
        _capture_directory_contract(root, "scaling evidence bundle root")
    ]
    directory_contracts.extend(
        _capture_directory_contract(
            root.joinpath(*PurePosixPath(relative).parts),
            f"scaling evidence bundle directory {index}",
        )
        for index, relative in enumerate(directories)
    )
    files: list[tuple[str, PathContract]] = []
    for index, (relative, path, metadata) in enumerate(scanned):
        contract = _capture_path_contract(
            path,
            f"scaling evidence bundle file {index}",
            expected_sha256=None,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=metadata.st_size,
        )
        files.append((relative, contract))

    final_scan, final_directories, final_total = _scan_scaling_bundle(root)
    if [item[0] for item in final_scan] != [item[0] for item in scanned]:
        raise ReceiptError("scaling evidence bundle file inventory changed while read")
    if final_directories != directories:
        raise ReceiptError(
            "scaling evidence bundle directory inventory changed while read"
        )
    for index, ((_, contract), (_, _, metadata)) in enumerate(
        zip(files, final_scan)
    ):
        observed = (
            metadata.st_dev,
            metadata.st_ino,
            stat.S_IMODE(metadata.st_mode),
            metadata.st_uid,
            metadata.st_nlink,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        )
        expected = (
            contract.device,
            contract.inode,
            contract.mode,
            contract.owner,
            contract.nlink,
            contract.size,
            contract.mtime_ns,
            contract.ctime_ns,
        )
        if observed != expected:
            raise ReceiptError(
                f"scaling evidence bundle file {index} changed after hashing"
            )
    for index, contract in enumerate(directory_contracts):
        if (
            _capture_directory_contract(
                contract.path, f"scaling evidence stable directory {index}"
            )
            != contract
        ):
            raise ReceiptError(
                "scaling evidence bundle directory changed while files were hashed"
            )
    if final_total != sum(contract.size for _, contract in files):
        raise ReceiptError("scaling evidence bundle size changed while read")
    return files, directories, final_total


def _load_scaling_json(path: Path, name: str) -> tuple[bytes, dict[str, Any]]:
    snapshot = _read_evidence_snapshot(
        path,
        name,
        maximum_bytes=_MAX_SCALING_JSON_BYTES,
        allowed_owners={os.geteuid()},
    )

    def reject_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ReceiptError(f"{name} contains a duplicate JSON field")
            value[key] = item
        return value

    def reject_constant(value: str) -> None:
        raise ReceiptError(f"{name} contains a nonfinite JSON value: {value}")

    try:
        value = json.loads(
            snapshot.data.decode("utf-8"),
            object_pairs_hook=reject_pairs,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReceiptError(f"{name} is not strict UTF-8 JSON") from error
    if not isinstance(value, dict):
        raise ReceiptError(f"{name} must contain one JSON object")
    return snapshot.data, value


def _scaling_ref_path(
    value: Any,
    *,
    root: Path,
    contracts: dict[str, PathContract],
    name: str,
) -> tuple[str, PathContract]:
    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        raise ReceiptError(f"{name} is not one canonical scaling artifact reference")
    relative = value.get("path")
    digest = value.get("sha256")
    if not isinstance(relative, str) or not isinstance(digest, str):
        raise ReceiptError(f"{name} scaling artifact reference is malformed")
    pure = PurePosixPath(relative)
    if (
        relative != pure.as_posix()
        or pure.is_absolute()
        or not pure.parts
        or any(
            part in {"", ".", ".."}
            or _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(part) is None
            for part in pure.parts
        )
    ):
        raise ReceiptError(f"{name} is not a safe normalized in-bundle path")
    contract = contracts.get(relative)
    if contract is None:
        raise ReceiptError(f"{name} is absent from the scaling bundle inventory")
    if _require_digest(digest, f"{name} digest") != contract.sha256:
        raise ReceiptError(f"{name} digest does not match the scaling bundle")
    expected_path = root.joinpath(*pure.parts)
    if contract.path != expected_path:
        raise ReceiptError(f"{name} resolves outside the scaling evidence bundle")
    return relative, contract


def _path_contract_artifact(contract: PathContract) -> dict[str, Any]:
    return {
        "path": str(contract.path),
        "sha256": contract.sha256,
        "size_bytes": contract.size,
        "mode": f"{contract.mode:04o}",
        "owner_uid": contract.owner,
        "nlink": contract.nlink,
    }


def _validate_scaling_evidence(
    *,
    manifest_path: Path,
    sealed: dict[str, Any],
    repo_root: Path,
    checker_environment: dict[str, str],
    expected_trial_harness_sha256: str,
    expected_configuration_sha256: str,
    expected_irohad_sha256: str,
    expected_iroha_cli_sha256: str,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    expected_trial_harness_sha256 = _require_digest(
        expected_trial_harness_sha256,
        "expected scaling trial harness digest",
    )
    expected_configuration_sha256 = _require_digest(
        expected_configuration_sha256,
        "expected scaling configuration digest",
    )
    expected_irohad_sha256 = _require_digest(
        expected_irohad_sha256,
        "expected scaling irohad digest",
    )
    expected_iroha_cli_sha256 = _require_digest(
        expected_iroha_cli_sha256,
        "expected scaling iroha CLI digest",
    )
    if (
        not manifest_path.is_absolute()
        or Path(os.path.abspath(manifest_path)) != manifest_path
        or manifest_path.name != "scaling_evidence.json"
    ):
        raise ReceiptError(
            "scaling evidence manifest must be the absolute normalized "
            "scaling_evidence.json bundle root file"
        )
    root = manifest_path.parent
    files, directories, total_bytes = _capture_scaling_bundle(root)
    contracts = dict(files)
    manifest_contract = contracts.get("scaling_evidence.json")
    report_contract = contracts.get("validation_report.json")
    if manifest_contract is None:
        raise ReceiptError("scaling evidence manifest is absent from its bundle")
    if report_contract is None:
        raise ReceiptError(
            "scaling evidence bundle lacks canonical validation_report.json"
        )
    if manifest_contract.path != manifest_path:
        raise ReceiptError("scaling evidence manifest path is not its exact bundle file")

    manifest_data, manifest = _load_scaling_json(
        manifest_path, "scaling evidence manifest"
    )
    report_data, report = _load_scaling_json(
        report_contract.path, "scaling validation report"
    )
    if set(report) != {
        "schema",
        "result",
        "manifest_sha256",
        "errors",
        "metrics",
    }:
        raise ReceiptError("scaling validation report fields are not canonical")
    if (
        report.get("schema") != _SCALING_REPORT_SCHEMA
        or report.get("result") != "pass"
        or report.get("errors") != []
        or not isinstance(report.get("metrics"), dict)
        or report.get("manifest_sha256") != manifest_contract.sha256
    ):
        raise ReceiptError(
            "scaling validation report is not an exact pass for this manifest"
        )
    canonical_report = (
        json.dumps(report, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    if report_data != canonical_report:
        raise ReceiptError("scaling validation report is not canonical JSON")
    if hashlib.sha256(report_data).hexdigest() != report_contract.sha256:
        raise ReceiptError("scaling validation report changed while decoded")

    _, identity_contract = _scaling_ref_path(
        manifest.get("identity"),
        root=root,
        contracts=contracts,
        name="scaling identity",
    )
    identity_data, identity = _load_scaling_json(
        identity_contract.path, "scaling identity"
    )
    if hashlib.sha256(identity_data).hexdigest() != identity_contract.sha256:
        raise ReceiptError("scaling identity changed while decoded")
    software = identity.get("software")
    if not isinstance(software, dict):
        raise ReceiptError("scaling identity lacks its software binding")
    if software.get("source_revision") != sealed["head_commit"]:
        raise ReceiptError(
            "scaling identity source_revision is not the sealed head_commit"
        )
    if (
        software.get("workspace_source_sha256")
        != sealed["workspace_source_manifest_sha256"]
    ):
        raise ReceiptError(
            "scaling identity workspace_source_sha256 is not the sealed "
            "workspace manifest"
        )
    if software.get("irohad_sha256") != expected_irohad_sha256:
        raise ReceiptError(
            "scaling identity irohad_sha256 is not the authenticated digest"
        )
    if software.get("iroha_cli_sha256") != expected_iroha_cli_sha256:
        raise ReceiptError(
            "scaling identity iroha_cli_sha256 is not the authenticated digest"
        )

    _, configuration_contract = _scaling_ref_path(
        manifest.get("configuration"),
        root=root,
        contracts=contracts,
        name="scaling configuration",
    )
    if configuration_contract.sha256 != expected_configuration_sha256:
        raise ReceiptError(
            "scaling configuration is not the authenticated digest"
        )
    _, trial_harness_contract = _scaling_ref_path(
        manifest.get("trial_harness"),
        root=root,
        contracts=contracts,
        name="scaling trial harness",
    )
    if trial_harness_contract.sha256 != expected_trial_harness_sha256:
        raise ReceiptError(
            "scaling trial harness is not the authenticated digest"
        )

    tooling = manifest.get("tooling")
    if (
        not isinstance(tooling, list)
        or len(tooling) != len(_SCALING_REQUIRED_TOOLING)
    ):
        raise ReceiptError(
            "scaling tooling does not contain the exact retained tool set"
        )
    retained_tooling: list[tuple[str, str, PathContract]] = []
    for index, ((role, source_path), entry) in enumerate(
        zip(_SCALING_REQUIRED_TOOLING, tooling)
    ):
        if (
            not isinstance(entry, dict)
            or set(entry) != {"role", "source_path", "artifact"}
            or entry.get("role") != role
            or entry.get("source_path") != source_path
        ):
            raise ReceiptError(
                f"scaling tooling entry {index} is not the retained {role} tool"
            )
        _, archived_tool = _scaling_ref_path(
            entry.get("artifact"),
            root=root,
            contracts=contracts,
            name=f"scaling archived {role} tool",
        )
        retained_path = repo_root.joinpath(*PurePosixPath(source_path).parts)
        retained_tool = _capture_path_contract(
            retained_path,
            f"retained scaling {role} tool",
            expected_sha256=None,
            expected_owner=os.geteuid(),
            expected_nlink=1,
        )
        if archived_tool.sha256 != retained_tool.sha256:
            raise ReceiptError(
                f"scaling archived {role} tool is not the retained sealed tool"
            )
        retained_tooling.append((role, source_path, retained_tool))

    retained_validator = (
        repo_root
        / "scripts"
        / "nexus"
        / "validate_multilane_scaling_evidence.py"
    )
    retained_contract = _capture_path_contract(
        retained_validator,
        "retained scaling evidence validator",
        expected_sha256=None,
        expected_owner=os.geteuid(),
        expected_nlink=1,
    )
    _, archived_validator = _scaling_ref_path(
        manifest.get("validator"),
        root=root,
        contracts=contracts,
        name="archived scaling validator",
    )
    if archived_validator.sha256 != retained_contract.sha256:
        raise ReceiptError(
            "archived scaling validator is not the retained sealed validator"
        )

    with tempfile.TemporaryDirectory(prefix="sumeragi-v2-scaling-replay-") as temporary:
        replay_report = Path(temporary).resolve(strict=True) / "validation_report.json"
        status, _, _ = _run_bounded_python_validator(
            retained_validator,
            [
                str(manifest_path),
                "--expected-source-revision",
                sealed["head_commit"],
                "--expected-workspace-source-sha256",
                sealed["workspace_source_manifest_sha256"],
                "--expected-validator-sha256",
                retained_contract.sha256,
                "--expected-trial-harness-sha256",
                expected_trial_harness_sha256,
                "--expected-configuration-sha256",
                expected_configuration_sha256,
                "--expected-irohad-sha256",
                expected_irohad_sha256,
                "--expected-iroha-cli-sha256",
                expected_iroha_cli_sha256,
                "--expected-repository-root",
                str(repo_root),
                "--report",
                str(replay_report),
                "--quiet",
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="retained scaling evidence validator",
        )
        if status != 0:
            raise ReceiptError(
                "scaling evidence bundle failed retained-validator revalidation"
            )
        replay_data, replay = _load_scaling_json(
            replay_report, "recomputed scaling validation report"
        )
        if replay != report or replay_data != report_data:
            raise ReceiptError(
                "scaling validation report does not match retained revalidation"
            )

    final_files, final_directories, final_total = _capture_scaling_bundle(root)
    if (
        final_files != files
        or final_directories != directories
        or final_total != total_bytes
    ):
        raise ReceiptError(
            "scaling evidence bundle changed during retained revalidation"
        )
    final_retained = _capture_path_contract(
        retained_validator,
        "retained scaling evidence validator after replay",
        expected_sha256=retained_contract.sha256,
        expected_mode=retained_contract.mode,
        expected_owner=retained_contract.owner,
        expected_nlink=retained_contract.nlink,
        expected_size=retained_contract.size,
    )
    if final_retained != retained_contract:
        raise ReceiptError("retained scaling evidence validator changed during replay")
    for role, _, retained_tool in retained_tooling:
        final_tool = _capture_path_contract(
            retained_tool.path,
            f"retained scaling {role} tool after replay",
            expected_sha256=retained_tool.sha256,
            expected_mode=retained_tool.mode,
            expected_owner=retained_tool.owner,
            expected_nlink=retained_tool.nlink,
            expected_size=retained_tool.size,
        )
        if final_tool != retained_tool:
            raise ReceiptError(
                f"retained scaling {role} tool changed during replay"
            )
    if hashlib.sha256(manifest_data).hexdigest() != manifest_contract.sha256:
        raise ReceiptError("scaling evidence manifest changed while decoded")

    bundle = {
        "root": str(root),
        "file_count": len(files),
        "total_size_bytes": total_bytes,
        "directories": directories,
        "files": [
            {
                "relative_path": relative,
                **_path_contract_artifact(contract),
            }
            for relative, contract in files
        ],
    }
    trust_anchors = {
        "trial_harness_sha256": expected_trial_harness_sha256,
        "configuration_sha256": expected_configuration_sha256,
        "irohad_sha256": expected_irohad_sha256,
        "iroha_cli_sha256": expected_iroha_cli_sha256,
        "repository_root": str(repo_root),
        "retained_tooling": [
            {
                "role": role,
                "source_path": source_path,
                **_path_contract_artifact(contract),
            }
            for role, source_path, contract in retained_tooling
        ],
    }
    return bundle, _path_contract_artifact(retained_contract), trust_anchors


def _read_g12_snapshot(
    path: Path, name: str, *, maximum_bytes: int
) -> EvidenceSnapshot:
    return _read_evidence_snapshot(
        path,
        name,
        maximum_bytes=maximum_bytes,
        allowed_owners={os.geteuid()},
    )


def _decode_g12_tsv(
    snapshot: EvidenceSnapshot,
    name: str,
    *,
    expected_header: tuple[str, ...] | None = None,
) -> list[list[str]]:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} must be terminal-LF, LF-only TSV")
    try:
        text = data.decode("utf-8")
        rows = list(csv.reader(io.StringIO(text), delimiter="\t", strict=True))
    except (UnicodeDecodeError, csv.Error) as error:
        raise ReceiptError(f"{name} is not strict UTF-8 TSV") from error
    if not rows or any(not row or any(not field for field in row) for row in rows):
        raise ReceiptError(f"{name} contains an empty TSV field")
    if expected_header is not None and tuple(rows[0]) != expected_header:
        raise ReceiptError(f"{name} does not have its exact canonical header")
    return rows


def _g12_completion_fields(
    snapshot: EvidenceSnapshot, name: str
) -> dict[str, str]:
    rows = _decode_g12_tsv(snapshot, name)
    fields: dict[str, str] = {}
    for row in rows:
        if len(row) != 2 or row[0] in fields:
            raise ReceiptError(f"{name} contains malformed or duplicate fields")
        fields[row[0]] = row[1]
    return fields


def _validate_g12_log(snapshot: EvidenceSnapshot, name: str, test: str) -> None:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} is not terminal-LF, LF-only output")
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error
    results = [line for line in lines if line.startswith("test result:")]
    if (
        lines.count("running 1 test") != 1
        or lines.count(f"test {test} ... ok") != 1
        or len(results) != 1
        or re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"[0-9]+ filtered out; finished in .+",
            results[0],
        )
        is None
    ):
        raise ReceiptError(f"{name} does not prove one exact passing G-12P test")


def _require_g12_directory_inventory(
    directory: Path, expected_names: set[str], name: str
) -> None:
    try:
        metadata = directory.lstat()
        entries = list(directory.iterdir())
    except OSError as error:
        raise ReceiptError(f"{name} directory is unavailable") from error
    if (
        directory.resolve(strict=True) != directory
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            f"{name} directory must be an owner-owned resolved non-symlink directory"
        )
    actual_names: set[str] = set()
    for entry in entries:
        if (
            _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(entry.name) is None
            or entry.name in actual_names
        ):
            raise ReceiptError(f"{name} directory contains an unsafe entry")
        actual_names.add(entry.name)
        try:
            entry_metadata = entry.lstat()
        except OSError as error:
            raise ReceiptError(f"{name} directory entry is unavailable") from error
        if (
            stat.S_ISLNK(entry_metadata.st_mode)
            or not stat.S_ISREG(entry_metadata.st_mode)
            or entry_metadata.st_uid != os.geteuid()
            or entry_metadata.st_nlink != 1
        ):
            raise ReceiptError(
                f"{name} directory contains a nonregular or aliased artifact"
            )
    if actual_names != expected_names:
        raise ReceiptError(
            f"{name} directory inventory differs from its exact evidence schema"
        )


def _validate_g4p_log(snapshot: EvidenceSnapshot, name: str, test: str) -> None:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} is not terminal-LF, LF-only output")
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error
    results = [line for line in lines if line.startswith("test result:")]
    native_marker_count = lines.count(_G4P_NATIVE_AMX_GROUPED_PRUNING_MARKER)
    expected_native_marker_count = int(test == _G4P_RELEASE_TESTS[3][1])
    release_marker_count = int(
        test in (_G4P_RELEASE_TESTS[0][1], _G4P_RELEASE_TESTS[3][1])
    )
    if (
        lines.count("running 1 test") != 1
        or lines.count(f"test {test} ... ok") != 1
        or lines.count(f"[multilane-release-gate] started: {test}")
        != release_marker_count
        or lines.count(f"[multilane-release-gate] completed: {test}")
        != release_marker_count
        or native_marker_count != expected_native_marker_count
        or any("developer opt-out" in line for line in lines)
        or len(results) != 1
        or re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"[0-9]+ filtered out; finished in .+",
            results[0],
        )
        is None
    ):
        raise ReceiptError(
            f"{name} does not prove one exact passing mandatory G-4P test"
        )


def _validate_g4p_evidence(
    *,
    completion_path: Path,
    sealed: dict[str, Any],
    prebuilt_manifest_sha256: str,
) -> dict[str, Any]:
    completion = _read_g12_snapshot(
        completion_path,
        "G-4P completion",
        maximum_bytes=_MAX_G4P_TSV_BYTES,
    )
    completion_fields = _g12_completion_fields(completion, "G-4P completion")
    expected_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "skipped_runs",
        "native_grouped_pruning_evidence",
        "runs_sha256",
    }
    if set(completion_fields) != expected_fields:
        raise ReceiptError("G-4P completion fields are not canonical")
    expected_identity = {
        "schema_version": "1",
        "mode": "mandatory-four-peer-multilane-release",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "expected_runs": "4",
        "passed_runs": "4",
        "failed_runs": "0",
        "skipped_runs": "0",
        "native_grouped_pruning_evidence": "passed",
    }
    if any(
        completion_fields.get(field) != value
        for field, value in expected_identity.items()
    ):
        raise ReceiptError(
            "G-4P completion is not exact passing release-bound accounting"
        )
    _require_digest(completion_fields["runs_sha256"], "G-4P run summary digest")

    summary = _read_g12_snapshot(
        completion.path.with_name("runs.tsv"),
        "G-4P run summary",
        maximum_bytes=_MAX_G4P_TSV_BYTES,
    )
    if summary.sha256 != completion_fields["runs_sha256"]:
        raise ReceiptError("G-4P run summary digest mismatch")
    rows = _decode_g12_tsv(
        summary,
        "G-4P run summary",
        expected_header=("target", "test", "status", "log_sha256", "log"),
    )
    if len(rows) != len(_G4P_RELEASE_TESTS) + 1:
        raise ReceiptError("G-4P run summary must contain exactly four runs")

    logs: list[EvidenceSnapshot] = []
    expected_names = {"COMPLETED.tsv", "runs.tsv"}
    for index, ((target, test), row) in enumerate(
        zip(_G4P_RELEASE_TESTS, rows[1:])
    ):
        expected_log = f"run-{index:02d}-{target}.log"
        if (
            len(row) != 5
            or tuple(row[:3]) != (target, test, "passed")
            or _DIGEST_RE.fullmatch(row[3]) is None
            or row[4] != expected_log
        ):
            raise ReceiptError(f"G-4P run summary row {index} is not canonical")
        log = _read_g12_snapshot(
            completion.path.with_name(expected_log),
            f"G-4P run log {index}",
            maximum_bytes=_MAX_G4P_LOG_BYTES,
        )
        if log.sha256 != row[3]:
            raise ReceiptError(f"G-4P run log {index} digest mismatch")
        _validate_g4p_log(log, f"G-4P run log {index}", test)
        logs.append(log)
        expected_names.add(expected_log)
    _require_g12_directory_inventory(
        completion.path.parent,
        expected_names,
        "G-4P evidence",
    )

    return {
        "schema_version": 1,
        "completion": _snapshot_receipt_artifact(completion),
        "run_summary": _snapshot_receipt_artifact(summary),
        "run_logs": [
            _snapshot_receipt_artifact(snapshot) for snapshot in logs
        ],
    }


def _validate_g12_evidence(
    *,
    seed_completion_path: Path,
    fault_soak_completion_path: Path,
    sealed: dict[str, Any],
    prebuilt_manifest_sha256: str,
) -> dict[str, Any]:
    seed_completion = _read_g12_snapshot(
        seed_completion_path,
        "G-12P seed completion",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    seed_fields = _g12_completion_fields(
        seed_completion, "G-12P seed completion"
    )
    expected_seed_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "process_retry_runs",
        "runs_sha256",
    }
    if set(seed_fields) != expected_seed_fields:
        raise ReceiptError("G-12P seed completion fields are not canonical")
    expected_identity = {
        "schema_version": "1",
        "mode": "deterministic-seed-matrix",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "expected_runs": "10",
        "passed_runs": "10",
        "failed_runs": "0",
        "process_retry_runs": "0",
    }
    if any(seed_fields.get(field) != value for field, value in expected_identity.items()):
        raise ReceiptError(
            "G-12P seed completion is not exact passing release-bound accounting"
        )
    _require_digest(seed_fields["runs_sha256"], "G-12P seed summary digest")
    seed_summary_path = seed_completion.path.with_name("runs.tsv")
    seed_summary = _read_g12_snapshot(
        seed_summary_path,
        "G-12P seed summary",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    if seed_summary.sha256 != seed_fields["runs_sha256"]:
        raise ReceiptError("G-12P seed summary digest mismatch")
    rows = _decode_g12_tsv(
        seed_summary,
        "G-12P seed summary",
        expected_header=(
            "ordinal",
            "seed",
            "status",
            "process_retries",
            "log_sha256",
            "log",
        ),
    )
    if len(rows) != 11:
        raise ReceiptError("G-12P seed summary must contain exactly ten runs")
    seed_logs: list[EvidenceSnapshot] = []
    expected_seed_names = {"COMPLETED.tsv", "runs.tsv"}
    for ordinal, row in enumerate(rows[1:]):
        expected_log = f"seed-{ordinal:02d}.log"
        expected_row = (
            str(ordinal),
            f"{_G12_SEED_PREFIX}{ordinal:02d}",
            "passed",
            "0",
        )
        if (
            len(row) != 6
            or tuple(row[:4]) != expected_row
            or row[5] != expected_log
            or _DIGEST_RE.fullmatch(row[4]) is None
        ):
            raise ReceiptError(
                f"G-12P seed summary row {ordinal} is not canonical"
            )
        log = _read_g12_snapshot(
            seed_completion.path.with_name(expected_log),
            f"G-12P seed log {ordinal}",
            maximum_bytes=_MAX_G12_LOG_BYTES,
        )
        if log.sha256 != row[4]:
            raise ReceiptError(f"G-12P seed log {ordinal} digest mismatch")
        _validate_g12_log(log, f"G-12P seed log {ordinal}", _G12_SEED_TEST)
        seed_logs.append(log)
        expected_seed_names.add(expected_log)
    _require_g12_directory_inventory(
        seed_completion.path.parent,
        expected_seed_names,
        "G-12P seed evidence",
    )

    soak_completion = _read_g12_snapshot(
        fault_soak_completion_path,
        "G-12P fault-soak completion",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    soak_fields = _g12_completion_fields(
        soak_completion, "G-12P fault-soak completion"
    )
    expected_soak_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "seed",
        "duration_seconds",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "process_retry_runs",
        "log_sha256",
    }
    if set(soak_fields) != expected_soak_fields:
        raise ReceiptError("G-12P fault-soak completion fields are not canonical")
    expected_soak = {
        "schema_version": "1",
        "mode": "two-hour-fault-soak",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "seed": f"{_G12_SEED_PREFIX}00",
        "duration_seconds": "7200",
        "expected_runs": "1",
        "passed_runs": "1",
        "failed_runs": "0",
        "process_retry_runs": "0",
    }
    if any(soak_fields.get(field) != value for field, value in expected_soak.items()):
        raise ReceiptError(
            "G-12P fault-soak completion is not exact passing "
            "release-bound accounting"
        )
    _require_digest(soak_fields["log_sha256"], "G-12P fault-soak log digest")
    soak_log = _read_g12_snapshot(
        soak_completion.path.with_name("fault-soak.log"),
        "G-12P fault-soak log",
        maximum_bytes=_MAX_G12_LOG_BYTES,
    )
    if soak_log.sha256 != soak_fields["log_sha256"]:
        raise ReceiptError("G-12P fault-soak log digest mismatch")
    _validate_g12_log(soak_log, "G-12P fault-soak log", _G12_SOAK_TEST)
    _require_g12_directory_inventory(
        soak_completion.path.parent,
        {"COMPLETED.tsv", "fault-soak.log"},
        "G-12P fault-soak evidence",
    )
    if seed_completion.path.parent == soak_completion.path.parent:
        raise ReceiptError("G-12P seed and fault-soak evidence must be distinct")

    return {
        "seed_completion": _snapshot_receipt_artifact(seed_completion),
        "seed_summary": _snapshot_receipt_artifact(seed_summary),
        "seed_run_logs": [
            _snapshot_receipt_artifact(snapshot) for snapshot in seed_logs
        ],
        "fault_soak_completion": _snapshot_receipt_artifact(soak_completion),
        "fault_soak_log": _snapshot_receipt_artifact(soak_log),
    }


def build_receipt(
    *,
    candidate_identity_path: Path,
    sealed_identity_path: Path,
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
    bootstrap_completion_path: Path,
    bootstrap_evidence_dir_path: Path,
    bootstrap_identity_path: Path,
    bootstrap_attestation_path: Path,
    bootstrap_transcript_path: Path,
    expected_bootstrap_completion_sha256: str,
    bootstrap_candidate_root_path: Path,
    bootstrap_runner_path: Path,
    corridor_completion_path: Path,
    formal_completion_path: Path,
    seed_completion_path: Path,
    chaos_completion_path: Path,
    taira_completion_path: Path,
    g4p_completion_path: Path,
    g12_seed_completion_path: Path,
    g12_fault_soak_completion_path: Path,
    scaling_evidence_manifest_path: Path,
    expected_scaling_trial_harness_sha256: str,
    expected_scaling_configuration_sha256: str,
    expected_scaling_irohad_sha256: str,
    expected_scaling_iroha_cli_sha256: str,
    repository_root_path: Path,
    runner_logs_sealed: bool = False,
) -> tuple[dict[str, Any], PathContract, PathContract]:
    """Validate every completion artifact and return one aggregate receipt."""

    repo_root = repository_root_path.resolve(strict=True)
    if (
        not repository_root_path.is_absolute()
        or Path(os.path.abspath(repository_root_path)) != repository_root_path
        or repo_root != repository_root_path
        or repo_root != release_root_path
        or repo_root.is_symlink()
        or not repo_root.is_dir()
    ):
        raise ReceiptError(
            "repository root must be the exact retained sealed release root"
        )
    candidate_snapshot, candidate = _load_identity(
        candidate_identity_path, "candidate identity"
    )
    sealed_snapshot, sealed = _load_identity(
        sealed_identity_path, "sealed identity"
    )
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if candidate[field] != sealed[field]:
            raise ReceiptError(f"candidate and sealed identity disagree on {field}")
    if sealed["head_tree"] != sealed["index_tree"]:
        raise ReceiptError("sealed release index tree is not HEAD")
    # All code was compiled and all child evidence was produced after sealing,
    # so child completions bind the sealed permission-aware manifest. The
    # candidate manifest remains independently recorded in the final receipt.
    manifest = sealed["workspace_source_manifest_sha256"]
    expected_scaling_trial_harness_sha256 = _require_digest(
        expected_scaling_trial_harness_sha256,
        "expected scaling trial harness digest",
    )
    expected_scaling_configuration_sha256 = _require_digest(
        expected_scaling_configuration_sha256,
        "expected scaling configuration digest",
    )
    expected_scaling_irohad_sha256 = _require_digest(
        expected_scaling_irohad_sha256,
        "expected scaling irohad digest",
    )
    expected_scaling_iroha_cli_sha256 = _require_digest(
        expected_scaling_iroha_cli_sha256,
        "expected scaling iroha CLI digest",
    )

    release_authentication, signature_archives = _validate_signature_evidence(
        candidate_snapshot=candidate_snapshot,
        candidate=candidate,
        release_root_path=release_root_path,
        signature_attestation_path=signature_attestation_path,
        signature_transcript_path=signature_transcript_path,
        signature_raw_commit_path=signature_raw_commit_path,
        signature_cargo_lock_path=signature_cargo_lock_path,
        signature_allowed_signers_path=signature_allowed_signers_path,
        signature_revocation_path=signature_revocation_path,
        signature_git_path=signature_git_path,
        signature_ssh_keygen_path=signature_ssh_keygen_path,
        expected_git_sha256=expected_git_sha256,
        expected_ssh_keygen_sha256=expected_ssh_keygen_sha256,
        expected_allowed_signers_sha256=expected_allowed_signers_sha256,
        expected_revocation_sha256=expected_revocation_sha256,
        expected_signer_fingerprint=expected_signer_fingerprint,
    )
    bootstrap_authentication, bootstrap_evidence = _validate_bootstrap_evidence(
        completion_path=bootstrap_completion_path,
        evidence_dir_path=bootstrap_evidence_dir_path,
        identity_path=bootstrap_identity_path,
        attestation_path=bootstrap_attestation_path,
        transcript_path=bootstrap_transcript_path,
        expected_completion_sha256=expected_bootstrap_completion_sha256,
        candidate_root_path=bootstrap_candidate_root_path,
        runner_path=bootstrap_runner_path,
        release_root_path=release_root_path,
        candidate=candidate,
        candidate_snapshot=candidate_snapshot,
        sealed=sealed,
        expected_signer_fingerprint=expected_signer_fingerprint,
        signature_archives=signature_archives,
        runner_logs_sealed=runner_logs_sealed,
        expected_scaling_manifest_path=scaling_evidence_manifest_path,
        expected_scaling_trial_harness_sha256=(
            expected_scaling_trial_harness_sha256
        ),
        expected_scaling_configuration_sha256=(
            expected_scaling_configuration_sha256
        ),
        expected_scaling_irohad_sha256=expected_scaling_irohad_sha256,
        expected_scaling_iroha_cli_sha256=expected_scaling_iroha_cli_sha256,
    )
    checker_environment = _closed_replay_environment(
        Path(signature_archives["git"]["path"]).parent
    )
    checker_environment.update(
        {
            "PATH": str(Path(signature_archives["git"]["path"]).parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONHASHSEED": "0",
        }
    )

    (
        scaling_bundle,
        retained_scaling_validator,
        scaling_trust_anchors,
    ) = _validate_scaling_evidence(
        manifest_path=scaling_evidence_manifest_path,
        sealed=sealed,
        repo_root=repo_root,
        checker_environment=checker_environment,
        expected_trial_harness_sha256=(
            expected_scaling_trial_harness_sha256
        ),
        expected_configuration_sha256=(
            expected_scaling_configuration_sha256
        ),
        expected_irohad_sha256=expected_scaling_irohad_sha256,
        expected_iroha_cli_sha256=expected_scaling_iroha_cli_sha256,
    )
    corridor_path, corridor_completion = _load_tsv(
        corridor_completion_path, "corridor completion"
    )
    (
        corridor_summary,
        corridor_required,
        corridor_g_unit_inventory,
        corridor_logs,
        prebuilt_binary_bundle,
    ) = _corridor_artifacts(
        corridor_path,
        corridor_completion,
        sealed,
        repo_root,
        bootstrap_authentication["runner"]["tools"],
    )
    prebuilt_manifest_sha256 = prebuilt_binary_bundle["manifest"]["sha256"]
    g4p_evidence = _validate_g4p_evidence(
        completion_path=g4p_completion_path,
        sealed=sealed,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )
    g12_evidence = _validate_g12_evidence(
        seed_completion_path=g12_seed_completion_path,
        fault_soak_completion_path=g12_fault_soak_completion_path,
        sealed=sealed,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )

    formal_path, formal_completion = _load_tsv(
        formal_completion_path, "formal completion"
    )
    (
        formal_log,
        formal_ledger,
        formal_evidence,
        formal_verus_evidence,
        formal_verus_log,
        formal_multilane_apalache_evidence,
        formal_cross_tool_evidence,
        formal_production_trace_extraction_evidence,
        formal_harness_lock,
        formal_toolchain,
        formal_tlaps_resource_jsonl,
        formal_tlaps_resource_summary,
    ) = _formal_artifacts(
        formal_path, formal_completion, sealed, checker_environment, repo_root
    )
    seed_path, seed = _load_tsv(seed_completion_path, "seed completion")
    seed_manifest_fields = {
        "localnet_manifest_count",
        "localnet_manifests_path",
        "localnet_manifests_sha256",
    }
    for index in range(_SEED_RUN_COUNT):
        seed_manifest_fields.add(f"localnet_manifest_{index:03d}_path")
        seed_manifest_fields.add(f"localnet_manifest_{index:03d}_sha256")
    _require_fields(
        seed,
        {
            "schema_version",
            "profile",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "prebuilt_manifest_sha256",
            "completed_runs",
            "expected_runs",
            "summary_sha256",
        }
        | seed_manifest_fields,
        "seed completion",
    )
    if (
        seed["schema_version"] != "2"
        or seed["profile"] != "release"
        or seed["head_commit"] != sealed["head_commit"]
        or seed["head_tree"] != sealed["head_tree"]
        or seed["source_manifest_sha256"] != manifest
        or seed["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or seed["prebuilt_manifest_sha256"] != prebuilt_manifest_sha256
        or seed["completed_runs"] != str(_SEED_RUN_COUNT)
        or seed["expected_runs"] != str(_SEED_RUN_COUNT)
    ):
        raise ReceiptError("seed completion does not describe the exact release matrix")
    seed_summary = _bounded_evidence_snapshot(
        seed_path.path.with_name("summary.tsv"),
        "seed summary",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    if seed_summary.sha256 != seed["summary_sha256"]:
        raise ReceiptError("seed completion summary digest mismatch")
    seed_run_logs = _seed_run_logs(
        seed_path,
        seed_summary,
        manifest,
        repo_root,
        Path(prebuilt_binary_bundle["bundle_dir"]),
        prebuilt_manifest_sha256,
    )
    seed_localnet_manifest_index, seed_localnet_manifests = (
        _seed_localnet_manifests(seed_path, seed)
    )
    seed_summary_contract = _snapshot_contract(seed_summary)
    del seed_summary

    chaos_path, chaos = _load_tsv(chaos_completion_path, "chaos completion")
    _require_fields(
        chaos,
        {
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "log_sha256",
        }
        | set(_CHAOS_FIXED_FIELDS),
        "chaos completion",
    )
    expected_chaos = {
        **_CHAOS_FIXED_FIELDS,
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": manifest,
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
    }
    if any(chaos.get(field) != value for field, value in expected_chaos.items()):
        raise ReceiptError(
            "chaos completion does not match the exact release identity and reducer schedule"
        )
    chaos_log = _bounded_evidence_snapshot(
        chaos_path.path.with_name("chaos-100k.log"),
        "chaos log",
        maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
    )
    if chaos_log.sha256 != chaos["log_sha256"]:
        raise ReceiptError("chaos completion log digest mismatch")
    chaos_lines = _decode_lf_text(chaos_log, "chaos log").splitlines()
    chaos_results = [line for line in chaos_lines if line.startswith("test result:")]
    chaos_test_prefix = (
        "test accelerated_100_000_block_chaos_preserves_chain_prefix ... "
    )
    chaos_completion_line = chaos_test_prefix + "ok"
    if (
        chaos_lines.count("running 1 test") != 1
        or len(chaos_results) != 1
        or not re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"9 filtered out; finished in .+",
            chaos_results[0],
        )
        or sum(chaos_test_prefix in line for line in chaos_lines) != 1
        or chaos_lines.count(chaos_completion_line) != 1
        or chaos_lines.count(_CHAOS_MARKER) != 1
    ):
        raise ReceiptError(
            "chaos log does not prove its one exact passing release test"
        )
    chaos_log_contract = _snapshot_contract(chaos_log)
    del chaos_log

    taira_path, taira = _load_tsv(taira_completion_path, "Taira completion")
    _require_fields(
        taira,
        {
            "schema_version",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "prebuilt_manifest_sha256",
            "evidence_sha256",
            "log_sha256",
        },
        "Taira completion",
    )
    if (
        taira["schema_version"] != "1"
        or taira["head_commit"] != sealed["head_commit"]
        or taira["head_tree"] != sealed["head_tree"]
        or taira["source_manifest_sha256"] != manifest
        or taira["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or taira["prebuilt_manifest_sha256"] != prebuilt_manifest_sha256
    ):
        raise ReceiptError("Taira completion is not bound to the exact release identity")
    taira_evidence = _bounded_evidence_snapshot(
        taira_path.path.with_name("taira_v2_24h_soak.json"),
        "Taira evidence",
        maximum_bytes=_MAX_RELEASE_JSON_BYTES,
    )
    if taira_evidence.sha256 != taira["evidence_sha256"]:
        raise ReceiptError("Taira completion evidence digest mismatch")
    taira_log = _bounded_evidence_snapshot(
        taira_path.path.with_name("taira-v2-24h.log"),
        "Taira run log",
        maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
    )
    if taira_log.sha256 != taira["log_sha256"]:
        raise ReceiptError("Taira completion log digest mismatch")
    taira_lines = _decode_lf_text(taira_log, "Taira run log").splitlines()
    taira_results = [line for line in taira_lines if line.startswith("test result:")]
    if (
        taira_lines.count("running 1 test") != 1
        or len(taira_results) != 1
        or not re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"[0-9]+ filtered out; finished in .+",
            taira_results[0],
        )
        or sum(
            "test taira_public_localnet::"
            "taira_profile_24h_packet_impairment_and_restart_soak ... " in line
            for line in taira_lines
        )
        != 1
    ):
        raise ReceiptError("Taira log does not prove its one exact passing soak")
    taira_checker = repo_root / "scripts" / "check_taira_v2_soak_evidence.py"
    with tempfile.TemporaryDirectory(
        prefix="sumeragi-v2-taira-snapshot-replay-"
    ) as temporary:
        replay_evidence = (
            Path(temporary).resolve(strict=True) / taira_evidence.path.name
        )
        try:
            replay_evidence.write_bytes(taira_evidence.data)
            replay_evidence.chmod(0o400)
        except OSError as error:
            raise ReceiptError(
                "Taira snapshot replay could not materialize captured evidence"
            ) from error
        taira_status, _, _ = _run_bounded_python_validator(
            taira_checker,
            [
                str(replay_evidence),
                "--source-manifest",
                manifest,
                "--build-root",
                str(repo_root / "target" / "sumeragi-v2-release" / manifest),
                "--repo-root",
                str(repo_root),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived Taira evidence validator",
        )
        if taira_status != 0:
            raise ReceiptError("archived Taira evidence failed release validation")
    taira_evidence_contract = _snapshot_contract(taira_evidence)
    taira_log_contract = _snapshot_contract(taira_log)
    del taira_evidence, taira_log

    receipt = {
        "schema_version": 1,
        "protocol": "sumeragi-v2",
        "result": "release-complete",
        "identity": {
            "head_commit": sealed["head_commit"],
            "head_tree": sealed["head_tree"],
            "index_tree": sealed["index_tree"],
            "cargo_lock_sha256": sealed["cargo_lock_sha256"],
            "candidate_source_manifest_sha256": candidate[
                "workspace_source_manifest_sha256"
            ],
            "sealed_source_manifest_sha256": manifest,
        },
        "authentication": {
            "schema_version": 2,
            "bootstrap": bootstrap_authentication,
            "release_identity": release_authentication,
        },
        "evidence": {
            "bootstrap": bootstrap_evidence,
            "release_signature_attestation": signature_archives["attestation"],
            "release_signature_transcript": signature_archives[
                "verify_transcript"
            ],
            "release_signature_raw_commit": signature_archives["raw_commit"],
            "release_signature_cargo_lock": signature_archives["cargo_lock"],
            "release_signature_allowed_signers": signature_archives[
                "ssh_allowed_signers"
            ],
            "release_signature_revocation": signature_archives["ssh_revocation"],
            "release_signature_git": signature_archives["git"],
            "release_signature_ssh_keygen": signature_archives["ssh_keygen"],
            "corridor_completion": _artifact(corridor_path),
            "corridor_summary": _artifact(corridor_summary),
            "corridor_production_inventory": _artifact(corridor_required),
            "g_unit_focused_test_inventory": _artifact(
                corridor_g_unit_inventory
            ),
            "corridor_logs": [_artifact(path) for path in corridor_logs],
            "prebuilt_binary_bundle": prebuilt_binary_bundle,
            "formal_completion": _artifact(formal_path),
            "formal_gate_log": _artifact(formal_log),
            "formal_proof_coverage": _artifact(formal_ledger),
            "formal_proof_evidence": _artifact(formal_evidence),
            "formal_verus_evidence": _artifact(formal_verus_evidence),
            "formal_verus_log": _artifact(formal_verus_log),
            "formal_multilane_apalache_evidence": _artifact(
                formal_multilane_apalache_evidence
            ),
            "formal_cross_tool_evidence": _artifact(formal_cross_tool_evidence),
            "formal_production_trace_extraction_evidence": _artifact(
                formal_production_trace_extraction_evidence
            ),
            "formal_harness_lock": _artifact(formal_harness_lock),
            "formal_toolchain": _artifact(formal_toolchain),
            "formal_tlaps_resource_jsonl": _artifact(formal_tlaps_resource_jsonl),
            "formal_tlaps_resource_summary": _artifact(formal_tlaps_resource_summary),
            "seed_matrix_completion": _artifact(seed_path),
            "seed_matrix_summary": _artifact(seed_summary_contract),
            "seed_matrix_run_logs": [_artifact(path) for path in seed_run_logs],
            "seed_matrix_localnet_manifest_index": _artifact(
                seed_localnet_manifest_index
            ),
            "seed_matrix_localnet_manifests": [
                _artifact(path) for path in seed_localnet_manifests
            ],
            "chaos_completion": _artifact(chaos_path),
            "chaos_log": _artifact(chaos_log_contract),
            "taira_completion": _artifact(taira_path),
            "taira_evidence": _artifact(taira_evidence_contract),
            "taira_run_log": _artifact(taira_log_contract),
            "multilane_scaling_bundle": scaling_bundle,
            "multilane_scaling_retained_validator": retained_scaling_validator,
            "multilane_scaling_trust_anchors": scaling_trust_anchors,
            "g4p_multilane": g4p_evidence,
            "g12_cross_dataspace": g12_evidence,
        },
    }
    return (
        receipt,
        _snapshot_contract(candidate_snapshot),
        _snapshot_contract(sealed_snapshot),
    )


def _iter_artifact_records(value: Any) -> Any:
    if isinstance(value, dict):
        if (
            "path" in value
            and "sha256" in value
            and isinstance(value["path"], str)
            and isinstance(value["sha256"], str)
        ):
            yield value
        for child in value.values():
            yield from _iter_artifact_records(child)
    elif isinstance(value, list):
        for child in value:
            yield from _iter_artifact_records(child)


def _capture_path_contract(
    path: Path,
    name: str,
    *,
    expected_sha256: str | None,
    expected_mode: int | None = None,
    expected_owner: int | None = None,
    expected_nlink: int | None = None,
    expected_size: int | None = None,
) -> PathContract:
    if expected_sha256 is not None:
        _require_digest(expected_sha256, f"{name} digest")
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
        raise ReceiptError(f"{name} mode changed before receipt publication")
    if expected_owner is not None and before.st_uid != expected_owner:
        raise ReceiptError(f"{name} owner changed before receipt publication")
    if expected_nlink is not None and before.st_nlink != expected_nlink:
        raise ReceiptError(f"{name} link count changed before receipt publication")
    if expected_size is not None and before.st_size != expected_size:
        raise ReceiptError(f"{name} size changed before receipt publication")
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
            or opened.st_mode != before.st_mode
            or opened.st_uid != before.st_uid
            or opened.st_nlink != before.st_nlink
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > 4 * 1024 * 1024 * 1024:
                raise ReceiptError(f"{name} exceeds the aggregate evidence size limit")
            digest.update(chunk)
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
            raise ReceiptError(f"{name} changed while it was hashed")
        observed_sha = digest.hexdigest()
        if (
            (expected_sha256 is not None and observed_sha != expected_sha256)
            or size != opened.st_size
        ):
            raise ReceiptError(f"{name} digest changed before receipt publication")
        return PathContract(
            path=path,
            sha256=observed_sha,
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


def _snapshot_receipt_inputs(
    receipt: dict[str, Any],
    *,
    candidate_identity: PathContract,
    sealed_identity: PathContract,
) -> list[PathContract | DirectoryContract]:
    records = list(_iter_artifact_records(receipt["authentication"])) + list(
        _iter_artifact_records(receipt["evidence"])
    )
    for name, expected in (
        ("candidate identity", candidate_identity),
        ("sealed identity", sealed_identity),
    ):
        contract = _capture_path_contract(
            expected.path,
            name,
            expected_sha256=expected.sha256,
            expected_mode=expected.mode,
            expected_owner=expected.owner,
            expected_nlink=expected.nlink,
            expected_size=expected.size,
        )
        if contract != expected:
            raise ReceiptError(f"{name} changed after semantic validation")
        records.append(_path_contract_artifact(contract))
    by_path: dict[Path, dict[str, Any]] = {}
    for record in records:
        path = Path(record["path"])
        previous = by_path.get(path)
        if previous is not None:
            comparable = {key: value for key, value in record.items() if key != "path"}
            previous_comparable = {
                key: value for key, value in previous.items() if key != "path"
            }
            common = set(comparable) & set(previous_comparable)
            if any(comparable[key] != previous_comparable[key] for key in common):
                raise ReceiptError("aggregate receipt contains conflicting artifact aliases")
            continue
        by_path[path] = record

    scaling_bundle = receipt["evidence"].get("multilane_scaling_bundle")
    if not isinstance(scaling_bundle, dict):
        raise ReceiptError("aggregate receipt lacks its scaling bundle inventory")
    scaling_root_raw = scaling_bundle.get("root")
    scaling_files_raw = scaling_bundle.get("files")
    scaling_directories_raw = scaling_bundle.get("directories")
    if (
        not isinstance(scaling_root_raw, str)
        or not isinstance(scaling_files_raw, list)
        or not isinstance(scaling_directories_raw, list)
    ):
        raise ReceiptError("aggregate receipt scaling bundle inventory is malformed")
    scaling_root = Path(scaling_root_raw)
    expected_scaling_files: list[str] = []
    expected_scaling_size = 0
    for index, record in enumerate(scaling_files_raw):
        if not isinstance(record, dict):
            raise ReceiptError("aggregate receipt scaling file record is malformed")
        relative = record.get("relative_path")
        size = record.get("size_bytes")
        path_value = record.get("path")
        if (
            not isinstance(relative, str)
            or type(size) is not int
            or not isinstance(path_value, str)
            or Path(path_value)
            != scaling_root.joinpath(*PurePosixPath(relative).parts)
        ):
            raise ReceiptError(
                f"aggregate receipt scaling file {index} path is malformed"
            )
        expected_scaling_files.append(relative)
        expected_scaling_size += size
    if expected_scaling_files != sorted(expected_scaling_files) or len(
        expected_scaling_files
    ) != len(set(expected_scaling_files)):
        raise ReceiptError(
            "aggregate receipt scaling files are not one deterministic inventory"
        )
    if (
        scaling_bundle.get("file_count") != len(expected_scaling_files)
        or scaling_bundle.get("total_size_bytes") != expected_scaling_size
        or any(not isinstance(item, str) for item in scaling_directories_raw)
        or scaling_directories_raw != sorted(scaling_directories_raw)
    ):
        raise ReceiptError("aggregate receipt scaling bundle accounting is inconsistent")
    current_scaling, current_directories, current_scaling_size = (
        _scan_scaling_bundle(scaling_root)
    )
    if (
        [item[0] for item in current_scaling] != expected_scaling_files
        or current_directories != scaling_directories_raw
        or current_scaling_size != expected_scaling_size
    ):
        raise ReceiptError(
            "scaling evidence bundle inventory changed before receipt publication"
        )

    prebuilt_bundle = receipt["evidence"].get("prebuilt_binary_bundle")
    if (
        not isinstance(prebuilt_bundle, dict)
        or prebuilt_bundle.get("schema_version") != 2
        or not isinstance(prebuilt_bundle.get("bundle_dir"), str)
        or not isinstance(prebuilt_bundle.get("manifest"), dict)
        or not isinstance(prebuilt_bundle.get("binaries"), list)
    ):
        raise ReceiptError("aggregate receipt lacks its prebuilt binary bundle")
    prebuilt_root = Path(prebuilt_bundle["bundle_dir"])
    prebuilt_manifest = prebuilt_bundle["manifest"]
    prebuilt_binaries = prebuilt_bundle["binaries"]
    if prebuilt_manifest.get("path") != str(prebuilt_root / _PREBUILT_MANIFEST_NAME):
        raise ReceiptError("aggregate receipt prebuilt manifest path is malformed")
    expected_binary_paths = [
        (prefix, relative, prebuilt_root.joinpath(*PurePosixPath(relative).parts))
        for prefix, relative in _PREBUILT_BINARY_SPECS
    ]
    if len(prebuilt_binaries) != len(expected_binary_paths):
        raise ReceiptError("aggregate receipt prebuilt binary inventory is incomplete")
    for index, (record, (prefix, relative, path)) in enumerate(
        zip(prebuilt_binaries, expected_binary_paths)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != prefix
            or record.get("relative_path") != relative
            or record.get("path") != str(path)
        ):
            raise ReceiptError(
                f"aggregate receipt prebuilt binary {index} path is malformed"
            )
    prebuilt_directories = {
        prebuilt_root,
        prebuilt_root / "release",
        prebuilt_root / "message-control",
        prebuilt_root / "message-control" / "release",
    }
    for path, name in (
        (prebuilt_root, "aggregate prebuilt invocation bundle"),
        (prebuilt_root / "release", "aggregate prebuilt release directory"),
        (
            prebuilt_root / "message-control",
            "aggregate prebuilt message-control directory",
        ),
        (
            prebuilt_root / "message-control" / "release",
            "aggregate prebuilt message-control release directory",
        ),
    ):
        _prebuilt_directory(path, name)
    _prebuilt_directory_inventory(
        prebuilt_root,
        {_PREBUILT_MANIFEST_NAME, "release", "message-control"},
        "aggregate prebuilt invocation bundle",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "release",
        {"irohad", "iroha", "kagami"},
        "aggregate prebuilt release directory",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "message-control",
        {"release"},
        "aggregate prebuilt message-control directory",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "message-control" / "release",
        {"irohad"},
        "aggregate prebuilt message-control release directory",
    )

    g4p = receipt["evidence"].get("g4p_multilane")
    if not isinstance(g4p, dict) or g4p.get("schema_version") != 1:
        raise ReceiptError("aggregate receipt lacks its G-4P evidence")
    try:
        g4p_root = Path(g4p["completion"]["path"]).parent
    except (KeyError, TypeError) as error:
        raise ReceiptError("aggregate receipt G-4P evidence is malformed") from error

    g12 = receipt["evidence"].get("g12_cross_dataspace")
    if not isinstance(g12, dict):
        raise ReceiptError("aggregate receipt lacks its G-12P evidence")
    try:
        g12_seed_root = Path(g12["seed_completion"]["path"]).parent
        g12_soak_root = Path(g12["fault_soak_completion"]["path"]).parent
    except (KeyError, TypeError) as error:
        raise ReceiptError("aggregate receipt G-12P evidence is malformed") from error

    evidence = receipt["evidence"]
    durability_families = (
        (
            "corridor",
            "corridor_completion",
            (
                "corridor_completion",
                "corridor_summary",
                "corridor_production_inventory",
                "g_unit_focused_test_inventory",
                "corridor_logs",
            ),
        ),
        (
            "formal",
            "formal_completion",
            (
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
            ),
        ),
        (
            "seed",
            "seed_matrix_completion",
            (
                "seed_matrix_completion",
                "seed_matrix_summary",
                "seed_matrix_run_logs",
                "seed_matrix_localnet_manifest_index",
                "seed_matrix_localnet_manifests",
            ),
        ),
        (
            "chaos",
            "chaos_completion",
            ("chaos_completion", "chaos_log"),
        ),
        (
            "Taira",
            "taira_completion",
            ("taira_completion", "taira_evidence", "taira_run_log"),
        ),
    )
    family_roots: set[Path] = set()
    family_directories: set[Path] = set()
    for family, completion_key, member_keys in durability_families:
        completion_record = evidence.get(completion_key)
        if (
            not isinstance(completion_record, dict)
            or not isinstance(completion_record.get("path"), str)
        ):
            raise ReceiptError(
                f"aggregate receipt {family} completion path is malformed"
            )
        root = Path(completion_record["path"]).parent
        family_roots.add(root)
        family_directories.add(root)
        for member_key in member_keys:
            member = evidence.get(member_key)
            if member is None:
                raise ReceiptError(
                    f"aggregate receipt {family} durability inventory is incomplete"
                )
            records = list(_iter_artifact_records(member))
            if not records:
                raise ReceiptError(
                    f"aggregate receipt {family} durability inventory is malformed"
                )
            for record in records:
                parent = Path(record["path"]).parent
                if parent != root and root not in parent.parents:
                    raise ReceiptError(
                        f"aggregate receipt {family} artifact escaped its evidence root"
                    )
                while True:
                    family_directories.add(parent)
                    if parent == root:
                        break
                    parent = parent.parent

    snapshots: list[PathContract | DirectoryContract] = []
    inodes: dict[tuple[int, int], Path] = {}
    for index, (path, record) in enumerate(by_path.items()):
        mode_value = record.get("mode")
        mode = _octal_mode(mode_value, f"aggregate evidence {index} mode") if mode_value else None
        owner = record.get("owner_uid", os.geteuid())
        nlink = record.get("nlink", 1)
        size = record.get("size_bytes")
        if type(owner) is not int or type(nlink) is not int or (
            size is not None and type(size) is not int
        ):
            raise ReceiptError("aggregate receipt artifact metadata has non-integer fields")
        snapshot = _capture_path_contract(
            path,
            f"aggregate evidence {index}",
            expected_sha256=record["sha256"],
            expected_mode=mode,
            expected_owner=owner,
            expected_nlink=nlink,
            expected_size=size,
        )
        inode_key = (snapshot.device, snapshot.inode)
        alias = inodes.get(inode_key)
        if alias is not None and alias != path:
            raise ReceiptError("aggregate receipt evidence contains a hard-link alias")
        inodes[inode_key] = path
        snapshots.append(snapshot)
    evidence_root = Path(
        receipt["evidence"]["bootstrap"]["completion"]["path"]
    ).parent
    directory_paths = {
        evidence_root,
        scaling_root,
        g4p_root,
        g12_seed_root,
        g12_soak_root,
        Path(receipt["authentication"]["bootstrap"]["candidate_root"]),
        Path(receipt["authentication"]["bootstrap"]["runner"]["tool_directory"]),
        Path(receipt["authentication"]["release_identity"]["release_root"]),
    }
    directory_paths.update(family_roots)
    directory_paths.update(family_directories)
    directory_paths.update(prebuilt_directories)
    directory_paths.update(
        scaling_root.joinpath(*PurePosixPath(relative).parts)
        for relative in scaling_directories_raw
    )
    for path in by_path:
        parent = path.parent
        while parent == evidence_root or evidence_root in parent.parents:
            directory_paths.add(parent)
            if parent == evidence_root:
                break
            parent = parent.parent
    for index, path in enumerate(
        sorted(directory_paths, key=lambda item: (-len(item.parts), str(item)))
    ):
        snapshots.append(
            _capture_directory_contract(
                path,
                f"aggregate evidence directory {index}",
            )
        )
    return snapshots


def _capture_directory_contract(path: Path, name: str) -> DirectoryContract:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise ReceiptError(f"{name} must be a resolved non-symlink directory")
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
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
            raise ReceiptError(f"{name} changed while it was opened")
        return DirectoryContract(
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


def _revalidate_receipt_inputs(
    snapshots: list[PathContract | DirectoryContract],
    *,
    ignored_directories: frozenset[Path] = frozenset(),
) -> None:
    for index, snapshot in enumerate(snapshots):
        if isinstance(snapshot, DirectoryContract):
            if snapshot.path in ignored_directories:
                continue
            current_directory = _capture_directory_contract(
                snapshot.path, f"aggregate evidence directory {index}"
            )
            if current_directory != snapshot:
                raise ReceiptError(
                    f"aggregate evidence directory {index} changed before publication"
                )
            continue
        current = _capture_path_contract(
            snapshot.path,
            f"aggregate evidence {index}",
            expected_sha256=snapshot.sha256,
            expected_mode=snapshot.mode,
            expected_owner=snapshot.owner,
            expected_nlink=snapshot.nlink,
            expected_size=snapshot.size,
        )
        if current != snapshot:
            raise ReceiptError(f"aggregate evidence {index} changed before publication")


def _fsync_receipt_inputs(
    snapshots: list[PathContract | DirectoryContract],
    *,
    ignored_directories: frozenset[Path] = frozenset(),
) -> None:
    """Synchronize every evidence file and then its directories bottom-up."""

    ordered = [
        *[item for item in snapshots if isinstance(item, PathContract)],
        *sorted(
            (
                item
                for item in snapshots
                if isinstance(item, DirectoryContract)
                and item.path not in ignored_directories
            ),
            key=lambda item: (-len(item.path.parts), str(item.path)),
        ),
    ]
    for index, snapshot in enumerate(ordered):
        if isinstance(snapshot, DirectoryContract):
            current = _capture_directory_contract(
                snapshot.path, f"durability directory {index}"
            )
            flags = (
                os.O_RDONLY
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_CLOEXEC", 0)
            )
        else:
            current = _capture_path_contract(
                snapshot.path,
                f"durability evidence {index}",
                expected_sha256=snapshot.sha256,
                expected_mode=snapshot.mode,
                expected_owner=snapshot.owner,
                expected_nlink=snapshot.nlink,
                expected_size=snapshot.size,
            )
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if current != snapshot:
            raise ReceiptError(f"durability input {index} changed before fsync")
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(snapshot.path, flags)
        except OSError as error:
            raise ReceiptError(f"durability input {index} could not be opened") from error
        try:
            opened = os.fstat(descriptor)
            if (
                (opened.st_dev, opened.st_ino)
                != (snapshot.device, snapshot.inode)
                or stat.S_IMODE(opened.st_mode) != snapshot.mode
                or opened.st_uid != snapshot.owner
                or opened.st_nlink != snapshot.nlink
            ):
                raise ReceiptError(f"durability input {index} changed while opened")
            os.fsync(descriptor)
            after = os.fstat(descriptor)
            fields = (
                "st_dev",
                "st_ino",
                "st_mode",
                "st_uid",
                "st_nlink",
                "st_mtime_ns",
                "st_ctime_ns",
            )
            if isinstance(snapshot, PathContract):
                fields += ("st_size",)
            if any(
                getattr(after, field) != getattr(opened, field) for field in fields
            ):
                raise ReceiptError(f"durability input {index} changed during fsync")
        except OSError as error:
            raise ReceiptError(f"durability input {index} fsync failed") from error
        finally:
            os.close(descriptor)
    _revalidate_receipt_inputs(
        snapshots, ignored_directories=ignored_directories
    )


def _existing_receipt_contract(output: Path, data: bytes) -> PathContract:
    return _capture_path_contract(
        output,
        "existing terminal receipt",
        expected_sha256=hashlib.sha256(data).hexdigest(),
        expected_mode=0o400,
        expected_owner=os.geteuid(),
        expected_nlink=1,
        expected_size=len(data),
    )


def _complete_write(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        try:
            written = os.write(descriptor, view)
        except InterruptedError:
            continue
        if written <= 0:
            raise ReceiptError("terminal receipt write made no progress")
        view = view[written:]


def _owned_unlink_name(
    directory_fd: int, name: str, device: int, inode: int
) -> bool:
    try:
        metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    except (FileNotFoundError, OSError):
        return False
    if (
        not stat.S_ISREG(metadata.st_mode)
        or (metadata.st_dev, metadata.st_ino) != (device, inode)
    ):
        return False
    try:
        os.unlink(name, dir_fd=directory_fd)
    except OSError:
        return False
    return True


def _publish_terminal_receipt(
    output: Path,
    data: bytes,
    *,
    revalidate: Any,
) -> Path:
    if not output.is_absolute() or Path(os.path.abspath(output)) != output:
        raise ReceiptError("terminal receipt path must be absolute and normalized")
    parent, parent_stat = _private_evidence_directory(
        output.parent, "terminal receipt output directory"
    )
    if output.name in {"", ".", ".."} or "/" in output.name or "\0" in output.name:
        raise ReceiptError("terminal receipt output name is invalid")
    for ancestor in (parent, *parent.parents):
        metadata = ancestor.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, os.geteuid()}
        ):
            raise ReceiptError("terminal receipt output has an unsafe ancestor")
    if os.path.lexists(output):
        raise ReceiptError("terminal receipt output already exists; overwrite is forbidden")
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    try:
        directory_fd = os.open(parent, directory_flags)
    except OSError as error:
        raise ReceiptError("terminal receipt output directory could not be opened") from error
    staged_name = f".{output.name}.stage.{secrets.token_hex(16)}"
    staged_device = -1
    staged_inode = -1
    try:
        opened_parent = os.fstat(directory_fd)
        if (
            (opened_parent.st_dev, opened_parent.st_ino)
            != (parent_stat.st_dev, parent_stat.st_ino)
            or opened_parent.st_uid != os.geteuid()
            or stat.S_IMODE(opened_parent.st_mode) != 0o700
        ):
            raise ReceiptError("terminal receipt output directory changed while opened")
        flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(staged_name, flags, 0o600, dir_fd=directory_fd)
        try:
            staged_open = os.fstat(descriptor)
            staged_device, staged_inode = staged_open.st_dev, staged_open.st_ino
            if not stat.S_ISREG(staged_open.st_mode) or staged_open.st_nlink != 1:
                raise ReceiptError("terminal receipt stage is not one private regular file")
            _complete_write(descriptor, data)
            os.fchmod(descriptor, 0o400)
            os.fsync(descriptor)
            after_write = os.fstat(descriptor)
            if (
                after_write.st_uid != os.geteuid()
                or after_write.st_nlink != 1
                or stat.S_IMODE(after_write.st_mode) != 0o400
                or after_write.st_size != len(data)
            ):
                raise ReceiptError("terminal receipt stage metadata is not exact")
            os.lseek(descriptor, 0, os.SEEK_SET)
            staged_data = bytearray()
            while len(staged_data) < len(data):
                chunk = os.read(descriptor, min(1024 * 1024, len(data) - len(staged_data)))
                if not chunk:
                    break
                staged_data.extend(chunk)
            if bytes(staged_data) != data:
                raise ReceiptError("terminal receipt stage bytes failed verification")
        finally:
            os.close(descriptor)
        revalidate()
        if os.path.lexists(output):
            raise ReceiptError("terminal receipt output appeared before publication")
        os.link(
            staged_name,
            output.name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        os.fsync(directory_fd)
        published = os.stat(output.name, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(published.st_mode)
            or (published.st_dev, published.st_ino) != (staged_device, staged_inode)
            or stat.S_IMODE(published.st_mode) != 0o400
            or published.st_nlink != 2
        ):
            raise ReceiptError("terminal receipt link changed at publication")
        if not _owned_unlink_name(directory_fd, staged_name, staged_device, staged_inode):
            raise ReceiptError("terminal receipt staging link could not be retired")
        os.fsync(directory_fd)
        final = _capture_path_contract(
            output,
            "published terminal receipt",
            expected_sha256=hashlib.sha256(data).hexdigest(),
            expected_mode=0o400,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=len(data),
        )
        if (final.device, final.inode) != (staged_device, staged_inode):
            raise ReceiptError("terminal receipt inode changed after publication")
        revalidate()
        return output
    except BaseException as error:
        if staged_inode >= 0:
            _owned_unlink_name(directory_fd, output.name, staged_device, staged_inode)
        if staged_inode >= 0:
            _owned_unlink_name(directory_fd, staged_name, staged_device, staged_inode)
        try:
            os.fsync(directory_fd)
        except OSError:
            pass
        if isinstance(error, ReceiptError):
            raise
        if isinstance(error, OSError):
            raise ReceiptError("terminal receipt publication failed closed") from error
        raise
    finally:
        try:
            os.close(directory_fd)
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-identity", type=Path, required=True)
    parser.add_argument("--sealed-identity", type=Path, required=True)
    parser.add_argument("--release-root", type=Path, required=True)
    parser.add_argument("--signature-attestation", type=Path, required=True)
    parser.add_argument("--signature-transcript", type=Path, required=True)
    parser.add_argument("--signature-raw-commit", type=Path, required=True)
    parser.add_argument("--signature-cargo-lock", type=Path, required=True)
    parser.add_argument("--signature-allowed-signers", type=Path, required=True)
    parser.add_argument("--signature-revocation", type=Path, required=True)
    parser.add_argument("--signature-git", type=Path, required=True)
    parser.add_argument("--signature-ssh-keygen", type=Path, required=True)
    parser.add_argument("--expected-git-sha256", required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--expected-revocation-sha256", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--bootstrap-completion", type=Path, required=True)
    parser.add_argument("--bootstrap-evidence-dir", type=Path, required=True)
    parser.add_argument("--bootstrap-identity", type=Path, required=True)
    parser.add_argument("--bootstrap-attestation", type=Path, required=True)
    parser.add_argument("--bootstrap-transcript", type=Path, required=True)
    parser.add_argument(
        "--expected-bootstrap-completion-sha256", required=True
    )
    parser.add_argument("--bootstrap-candidate-root", type=Path, required=True)
    parser.add_argument("--bootstrap-runner", type=Path, required=True)
    parser.add_argument("--corridor-completion", type=Path, required=True)
    parser.add_argument("--formal-completion", type=Path, required=True)
    parser.add_argument("--seed-completion", type=Path, required=True)
    parser.add_argument("--chaos-completion", type=Path, required=True)
    parser.add_argument("--taira-completion", type=Path, required=True)
    parser.add_argument("--g4p-completion", type=Path, required=True)
    parser.add_argument("--g12-seed-completion", type=Path, required=True)
    parser.add_argument("--g12-fault-soak-completion", type=Path, required=True)
    parser.add_argument("--scaling-evidence-manifest", type=Path, required=True)
    parser.add_argument(
        "--expected-scaling-trial-harness-sha256", required=True
    )
    parser.add_argument(
        "--expected-scaling-configuration-sha256", required=True
    )
    parser.add_argument("--expected-scaling-irohad-sha256", required=True)
    parser.add_argument("--expected-scaling-iroha-cli-sha256", required=True)
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--verify-existing",
        action="store_true",
        help="rebuild and durably verify an existing no-clobber receipt",
    )
    args = parser.parse_args()
    try:
        receipt, candidate_identity, sealed_identity = build_receipt(
            candidate_identity_path=args.candidate_identity,
            sealed_identity_path=args.sealed_identity,
            release_root_path=args.release_root,
            signature_attestation_path=args.signature_attestation,
            signature_transcript_path=args.signature_transcript,
            signature_raw_commit_path=args.signature_raw_commit,
            signature_cargo_lock_path=args.signature_cargo_lock,
            signature_allowed_signers_path=args.signature_allowed_signers,
            signature_revocation_path=args.signature_revocation,
            signature_git_path=args.signature_git,
            signature_ssh_keygen_path=args.signature_ssh_keygen,
            expected_git_sha256=args.expected_git_sha256,
            expected_ssh_keygen_sha256=args.expected_ssh_keygen_sha256,
            expected_allowed_signers_sha256=args.expected_allowed_signers_sha256,
            expected_revocation_sha256=args.expected_revocation_sha256,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            bootstrap_completion_path=args.bootstrap_completion,
            bootstrap_evidence_dir_path=args.bootstrap_evidence_dir,
            bootstrap_identity_path=args.bootstrap_identity,
            bootstrap_attestation_path=args.bootstrap_attestation,
            bootstrap_transcript_path=args.bootstrap_transcript,
            expected_bootstrap_completion_sha256=(
                args.expected_bootstrap_completion_sha256
            ),
            bootstrap_candidate_root_path=args.bootstrap_candidate_root,
            bootstrap_runner_path=args.bootstrap_runner,
            corridor_completion_path=args.corridor_completion,
            formal_completion_path=args.formal_completion,
            seed_completion_path=args.seed_completion,
            chaos_completion_path=args.chaos_completion,
            taira_completion_path=args.taira_completion,
            g4p_completion_path=args.g4p_completion,
            g12_seed_completion_path=args.g12_seed_completion,
            g12_fault_soak_completion_path=args.g12_fault_soak_completion,
            scaling_evidence_manifest_path=args.scaling_evidence_manifest,
            expected_scaling_trial_harness_sha256=(
                args.expected_scaling_trial_harness_sha256
            ),
            expected_scaling_configuration_sha256=(
                args.expected_scaling_configuration_sha256
            ),
            expected_scaling_irohad_sha256=(
                args.expected_scaling_irohad_sha256
            ),
            expected_scaling_iroha_cli_sha256=(
                args.expected_scaling_iroha_cli_sha256
            ),
            repository_root_path=args.repository_root,
            runner_logs_sealed=args.verify_existing,
        )
        snapshots = _snapshot_receipt_inputs(
            receipt,
            candidate_identity=candidate_identity,
            sealed_identity=sealed_identity,
        )
        expected_output = (
            args.bootstrap_evidence_dir
            / "release-runner"
            / "output"
            / "release"
            / "RELEASE_COMPLETED.json"
        )
        if args.output != expected_output:
            raise ReceiptError(
                "terminal receipt is not the exact bootstrap release output path"
            )
        receipt_bytes = _canonical_json(receipt)
        if args.verify_existing:
            terminal = _existing_receipt_contract(args.output, receipt_bytes)
            verification_snapshots = [*snapshots, terminal]
            _fsync_receipt_inputs(verification_snapshots)
            _revalidate_receipt_inputs(verification_snapshots)
        else:
            _fsync_receipt_inputs(snapshots)
            mutable_directory = frozenset({args.output.parent})
            _publish_terminal_receipt(
                args.output,
                receipt_bytes,
                revalidate=lambda: _revalidate_receipt_inputs(
                    snapshots, ignored_directories=mutable_directory
                ),
            )
    except (OSError, ReceiptError) as error:
        print(f"Sumeragi v2 release receipt error: {error}", file=sys.stderr)
        return 1
    action = "verified" if args.verify_existing else "published"
    print(
        f"Sumeragi v2 aggregate release receipt {action}: {args.output}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
