#!/usr/bin/env python3
"""Validate and aggregate source-bound Sumeragi v2 release evidence."""

from __future__ import annotations

import argparse
import base64
import csv
from dataclasses import dataclass
import hashlib
import io
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import signal
import shutil
import stat
import subprocess
import sys
import time
from typing import Any

try:
    from sumeragi_v2_localnet_manifest import (
        LocalnetManifestError,
        canonical_localnet_manifest,
    )
except ModuleNotFoundError:
    from scripts.sumeragi_v2_localnet_manifest import (
        LocalnetManifestError,
        canonical_localnet_manifest,
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
_MAX_REPLAY_OUTPUT_BYTES = 4 * 1024 * 1024
_MAX_LOCALNET_MANIFEST_INDEX_BYTES = 1024 * 1024
_MAX_LOCALNET_MANIFEST_BYTES = 64 * 1024 * 1024
_REPLAY_TIMEOUT_SECONDS = 120
_FROZEN_BOOTSTRAP_SHA256 = (
    "568269f0431494b4f29092f461822f50a0a060cf596e3b126ba34da4a19e1180"
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
    "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial "
    "scheduler/post-decision/recovery/effect-capacity/ingress-causal-freshness "
    "mutations, bounded TLC, trace replay, and production Verus"
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
_PRODUCTION_TEST_COUNT = 509
_PRODUCTION_MODULES = (
    (
        "production-kura-progress-durability",
        "kura::tests",
        13,
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
        29,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 30),
    ("production-v2-core", "sumeragi::v2_core::tests", 25),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 12),
    ("production-v2-core-reducer", "sumeragi::v2_core::reducer::tests", 2),
    ("production-v2-core-wal", "sumeragi::v2_core::wal::tests", 1),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        3,
    ),
    (
        "production-v2-equivocation-evidence",
        "sumeragi::evidence::tests",
        1,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 42),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 1),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 59),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 29),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 37),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 26),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 53),
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
        1,
    ),
    (
        "production-v2-integration-runner",
        "sumeragi_v2_runner",
        4,
    ),
    (
        "production-p2p-peer-reliable-flush",
        "peer::run::tests",
        8,
    ),
    (
        "production-p2p-shared-source-byte-geometry",
        "peer::shared_byte_budget_tests",
        8,
    ),
    (
        "production-p2p-network-reliable-actor",
        "network::tests",
        56,
    ),
    (
        "production-p2p-source-memory-geometry",
        "network::inbound_source_memory_bound_tests",
        1,
    ),
    (
        "production-p2p-waiter-rank-geometry",
        "network::handle_update_tests",
        1,
    ),
    (
        "production-irohad-consensus-message-control",
        "consensus_message_control::tests",
        7,
    ),
    (
        "production-irohad-network-relay",
        "network_relay_tests",
        2,
    ),
    (
        "production-irohad-authenticated-via",
        "tests::relay_fairness",
        6,
    ),
    (
        "production-irohad-genesis-reply-geometry",
        "genesis_bootstrap::tests",
        4,
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
_JS_STATUS_PATTERN = (
    "getSumeragiStatusTyped (validates and normalizes authoritative v2 status|"
    "accepts the local-control liveness blocker|accepts the unsafe-proposal ignore reason|"
    "accepts all twelve ignore reasons at the bound)"
)
_PYTHON_STATUS_TESTS = (
    "python/iroha_torii_client/tests/test_client.py::"
    "test_get_sumeragi_status_parses_authoritative_v2_snapshot",
    "python/iroha_torii_client/tests/test_client.py::"
    "test_get_sumeragi_status_accepts_local_control_pending_liveness_blocker",
    "python/iroha_torii_client/tests/test_client.py::"
    "test_get_sumeragi_status_accepts_unsafe_proposal_ignore_reason",
    "python/iroha_torii_client/tests/test_client.py::"
    "test_get_sumeragi_status_accepts_all_twelve_ignore_reasons_at_the_bound",
)
_CROSS_SDK_TESTS = (
    "sumeragi_v2_cross_sdk_fixtures::shared_sdk_accept_fixtures_are_exact_current_rust_encodings",
    "sumeragi_v2_cross_sdk_fixtures::shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation",
)
_JS_STATUS_TESTS = (
    "getSumeragiStatusTyped validates and normalizes authoritative v2 status",
    "getSumeragiStatusTyped accepts the local-control liveness blocker",
    "getSumeragiStatusTyped accepts the unsafe-proposal ignore reason",
    "getSumeragiStatusTyped accepts all twelve ignore reasons at the bound",
)


def _canonical_production_tests(repo_root: Path) -> list[str]:
    runner = repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    try:
        source = runner.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise ReceiptError("release runner inventory is not UTF-8") from error
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
                    "genesis_bootstrap::tests::",
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


def _production_module_command(module: str) -> str:
    if module == "sumeragi_v2_runner":
        return (
            "cargo test --locked -p integration_tests --test "
            "sumeragi_v2_runner_isolated "
            f"{_PRODUCTION_INTEGRATION_MODULE} -- --test-threads=1"
        )
    if module in {
        "peer::run::tests",
        "network::tests",
        "network::inbound_source_memory_bound_tests",
        "network::handle_update_tests",
    }:
        return f"cargo test --locked -p iroha_p2p --lib {module} -- --test-threads=1"
    if module in {
        "consensus_message_control::tests",
        "network_relay_tests",
        "tests::relay_fairness",
        "genesis_bootstrap::tests",
    }:
        return (
            "cargo test --locked -p irohad --bin irohad "
            "--features test-network-message-control "
            f"{module} -- --test-threads=1"
        )
    if module.startswith("parameters::"):
        return f"cargo test --locked -p iroha_config --lib {module} -- --test-threads=1"
    if module in _DATA_MODEL_PRODUCTION_MODULES:
        return (
            "cargo test --locked -p iroha_data_model --lib "
            f"{module} -- --test-threads=1"
        )
    return f"cargo test --locked -p iroha_core --lib {module} -- --test-threads=1"


def _corridor_legs() -> list[tuple[str, str, int, str]]:
    legs = [
        (
            leg_id,
            "cargo-module",
            count,
            _production_module_command(module),
        )
        for leg_id, module, count in _PRODUCTION_MODULES
    ]
    legs.append(
        (
            "status-rust",
            "cargo-exact",
            1,
            "cargo test --locked -p iroha_data_model --lib "
            f"{_DATA_STATUS_TEST} -- --test-threads=1",
        )
    )
    legs.append(
        (
            "lane-certificate-rust",
            "cargo-exact",
            1,
            "cargo test --locked -p iroha_data_model --lib "
            f"{_DATA_LANE_CERTIFICATE_TEST} -- --exact --test-threads=1",
        )
    )
    legs.extend(
        (
            (
                "source-sealed-workspace-clippy",
                "command",
                0,
                "cargo clippy --workspace --all-targets -- -D warnings",
            ),
            (
                "source-sealed-workspace-tests",
                "command",
                0,
                "cargo test --locked --workspace",
            ),
            (
                "source-sealed-irohad-tests",
                "command",
                0,
                "cargo test --locked -p irohad --bin irohad "
                "--features test-network-message-control",
            ),
        )
    )
    legs.extend(
        (
            f"taira-contract-{index}",
            "cargo-exact",
            1,
            "cargo test --locked -p integration_tests --test consensus_and_da "
            f"{test} -- --exact --test-threads=1",
        )
        for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
    )
    legs.extend(
        (
            (
                "cross-sdk-rust",
                "cargo-exact",
                2,
                "cargo test --locked -p iroha_data_model --test "
                "iroha_data_model_group_02 sumeragi_v2_cross_sdk_fixtures:: "
                "-- --test-threads=1",
            ),
            (
                "status-javascript",
                "node",
                4,
                "node --test --test-reporter=tap "
                f"--test-name-pattern={_JS_STATUS_PATTERN} "
                "javascript/iroha_js/test/toriiClient.test.js",
            ),
            (
                "status-python",
                "pytest",
                4,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider " + " ".join(_PYTHON_STATUS_TESTS),
            ),
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
                11,
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
                71,
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
                189,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_release_receipt_test.py",
            ),
            (
                "preflight-proof-fidelity",
                "pytest",
                1045,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_proof_ledger_test.py "
                "pytests/scripts/sumeragi_v2_verus_evidence_test.py "
                "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py",
            ),
            (
                "preflight-formal-launcher",
                "pytest",
                16,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_formal_release_test.py",
            ),
            (
                "preflight-taira-soak",
                "pytest",
                39,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/taira_v2_soak_test.py "
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


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _regular_file(path: Path, name: str) -> Path:
    if not path.is_file() or path.is_symlink():
        raise ReceiptError(f"{name} is not a regular file: {path}")
    return path.resolve(strict=True)


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
) -> EvidenceSnapshot:
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
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(
                descriptor, min(1024 * 1024, maximum_bytes + 1 - total)
            )
            if not chunk:
                break
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


def _run_bounded_replay(
    executable: Path,
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
) -> tuple[int, bytes, bytes]:
    try:
        process = subprocess.Popen(
            (str(executable), *arguments),
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        raise ReceiptError("archived Git replay could not be started") from error
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
    deadline = time.monotonic() + _REPLAY_TIMEOUT_SECONDS
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _abort_replay(process)
                raise ReceiptError("archived Git replay exceeded its timeout")
            for key, _ in selector.select(min(remaining, 0.25)):
                stream_name, stream = key.data
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fd)
                    stream.close()
                    continue
                buffers[stream_name].extend(chunk)
                if sum(len(value) for value in buffers.values()) > _MAX_REPLAY_OUTPUT_BYTES:
                    _abort_replay(process)
                    raise ReceiptError("archived Git replay output exceeds its closed limit")
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            _abort_replay(process)
            raise ReceiptError("archived Git replay exceeded its timeout")
        try:
            status = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            _abort_replay(process)
            raise ReceiptError("archived Git replay exceeded its timeout") from error
    except BaseException:
        if process.poll() is None:
            _abort_replay(process)
        raise
    finally:
        selector.close()
        for stream in (process.stdout, process.stderr):
            if not stream.closed:
                stream.close()
    return status, bytes(buffers["stdout"]), bytes(buffers["stderr"])


def _run_required_replay(
    git: Path,
    arguments: list[str],
    *,
    root: Path,
    environment: dict[str, str],
    name: str,
) -> tuple[bytes, bytes]:
    status, stdout, stderr = _run_bounded_replay(
        git, arguments, cwd=root, environment=environment
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
    candidate_identity_path: Path,
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
    root_lock = _regular_file(root / "Cargo.lock", "release-root Cargo.lock")
    if (
        root_lock.stat().st_size != len(archives["cargo_lock"]["data"])
        or _sha256(root_lock) != archive_digests["cargo_lock"]
    ):
        raise ReceiptError("release-root Cargo.lock does not match its archive")

    candidate_path = _regular_file(candidate_identity_path, "candidate identity")
    candidate_bytes = candidate_path.read_bytes()
    if candidate_bytes != _canonical_json(candidate):
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
        != hashlib.sha256(candidate_bytes).hexdigest()
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
    )
    if replay_raw_stderr or replay_raw != raw_commit:
        raise ReceiptError("archived Git raw commit replay does not match its archive")
    verify_status, _, _ = _run_bounded_replay(
        git,
        [*actual_config, "verify-commit", "--raw", candidate["head_commit"]],
        cwd=root,
        environment=expected_environment,
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
        if (
            (current["device"], current["inode"], current["owner"])
            != (archive["device"], archive["inode"], archive["owner"])
            or current["data"] != archive["data"]
        ):
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


def _snapshot_receipt_artifact(snapshot: EvidenceSnapshot) -> dict[str, Any]:
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
    candidate_identity_path: Path,
    sealed: dict[str, Any],
    expected_signer_fingerprint: str,
    signature_archives: dict[str, dict[str, Any]],
    runner_logs_sealed: bool,
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
    current_candidate_path = _regular_file(candidate_identity_path, "candidate identity")
    if identity_snapshot.data != current_candidate_path.read_bytes():
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

    bootstrap_git = snapshots["trusted_git"].path
    bootstrap_git_environment = _closed_replay_environment(directory)

    def bootstrap_git_line(arguments: list[str], name: str) -> str:
        stdout, stderr = _run_required_replay(
            bootstrap_git,
            arguments,
            root=candidate_root,
            environment=bootstrap_git_environment,
            name=f"bootstrap candidate {name}",
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
    runner_tool_sources: dict[str, EvidenceSnapshot] = {}
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
        source = _read_evidence_snapshot(
            Path(source_path),
            f"bootstrap runner tool source {name}",
            maximum_bytes=_MAX_TOOL_BYTES,
            expected_mode=source_mode,
            allowed_owners={0, os.geteuid()},
            require_single_link=False,
            executable=True,
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


def _load_identity(path: Path, name: str) -> dict[str, Any]:
    path = _regular_file(path, name)
    try:
        data = path.read_bytes()
    except OSError as error:
        raise ReceiptError(f"{name} could not be read") from error
    value = _decode_canonical_json(data, name)
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
    return value


def _load_tsv(path: Path, name: str) -> tuple[Path, dict[str, str]]:
    path = _regular_file(path, name)
    fields: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error
    for line in lines:
        parts = line.split("\t")
        if len(parts) != 2 or not parts[0] or parts[0] in fields:
            raise ReceiptError(f"{name} contains malformed or duplicate fields")
        fields[parts[0]] = parts[1]
    return path, fields


def _require_fields(fields: dict[str, str], expected: set[str], name: str) -> None:
    if set(fields) != expected:
        raise ReceiptError(f"{name} fields do not match its completion schema")


def _artifact(path: Path) -> dict[str, str]:
    path = path.resolve(strict=True)
    return {"path": str(path), "sha256": _sha256(path)}


def _formal_artifacts(
    completion_path: Path,
    fields: dict[str, str],
    sealed: dict[str, Any],
    checker_environment: dict[str, str],
    repo_root: Path,
) -> tuple[Path, Path, Path, Path, Path, Path, Path, Path, Path, Path]:
    ledger = _regular_file(
        completion_path.with_name("proof_coverage.json"), "formal proof ledger"
    )
    checker = repo_root / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
    expected_completion_fields = {
        "schema_version",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "formal_gate_log_sha256",
        "proof_coverage_sha256",
        "proof_evidence_sha256",
        "verus_evidence_sha256",
        "verus_log_sha256",
        "cross_tool_evidence_sha256",
        "harness_cargo_lock_sha256",
        "formal_toolchain_sha256",
        "tlaps_resource_jsonl_sha256",
        "tlaps_resource_summary_sha256",
    }
    _require_fields(
        fields,
        expected_completion_fields,
        "formal completion",
    )
    expected = {
        "schema_version": "1",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
    }
    if any(fields.get(name) != value for name, value in expected.items()):
        raise ReceiptError("formal completion is not bound to the release identity")

    gate_log = _regular_file(
        completion_path.with_name("formal-gate.log"), "formal gate log"
    )
    evidence = _regular_file(
        completion_path.with_name("proof_evidence.json"), "formal proof evidence"
    )
    verus_evidence = _regular_file(
        completion_path.with_name("verus_evidence.json"),
        "formal Verus evidence",
    )
    verus_log = _regular_file(
        completion_path.with_name("verus.log"), "formal Verus log"
    )
    cross_tool_evidence = _regular_file(
        completion_path.with_name("cross_tool_evidence.json"),
        "formal cross-tool evidence",
    )
    harness_lock = _regular_file(
        completion_path.with_name("harness-Cargo.lock"), "formal harness lock"
    )
    toolchain_path = _regular_file(
        completion_path.with_name("formal-toolchain.tsv"), "formal toolchain"
    )
    tlaps_resource_jsonl = _regular_file(
        completion_path.with_name("tlaps_resource.jsonl"), "TLAPS resource samples"
    )
    tlaps_resource_summary = _regular_file(
        completion_path.with_name("tlaps_resource_summary.json"),
        "TLAPS resource summary",
    )
    for artifact, digest_field, name in (
        (gate_log, "formal_gate_log_sha256", "formal gate log"),
        (ledger, "proof_coverage_sha256", "formal proof ledger"),
        (evidence, "proof_evidence_sha256", "formal proof evidence"),
        (verus_evidence, "verus_evidence_sha256", "formal Verus evidence"),
        (verus_log, "verus_log_sha256", "formal Verus log"),
        (
            cross_tool_evidence,
            "cross_tool_evidence_sha256",
            "formal cross-tool evidence",
        ),
        (harness_lock, "harness_cargo_lock_sha256", "formal harness lock"),
        (toolchain_path, "formal_toolchain_sha256", "formal toolchain"),
        (
            tlaps_resource_jsonl,
            "tlaps_resource_jsonl_sha256",
            "TLAPS resource samples",
        ),
        (
            tlaps_resource_summary,
            "tlaps_resource_summary_sha256",
            "TLAPS resource summary",
        ),
    ):
        if _sha256(artifact) != fields[digest_field]:
            raise ReceiptError(f"{name} digest mismatch")
    resource_summary = _decode_canonical_json(
        tlaps_resource_summary.read_bytes(), "TLAPS resource summary"
    )
    if (
        resource_summary.get("schema_version") != 1
        or resource_summary.get("event") != "summary"
        or resource_summary.get("exit_reason") != "completed"
        or resource_summary.get("exit_status") != 0
        or resource_summary.get("memory_limit_bytes") != 2 * 1024 * 1024 * 1024
        or resource_summary.get("sample_interval_seconds") != 0.25
        or not isinstance(resource_summary.get("peak_memory_bytes"), int)
        or resource_summary["peak_memory_bytes"] < 0
        or resource_summary["peak_memory_bytes"]
        > resource_summary["memory_limit_bytes"]
    ):
        raise ReceiptError("TLAPS resource summary is not a successful bounded release run")
    if fields["harness_cargo_lock_sha256"] != _HARNESS_LOCK_SHA256:
        raise ReceiptError("formal harness lock is not the pinned dependency graph")
    cross_tool_result = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--ledger",
            str(ledger),
            "--print-cross-tool-obligations",
        ],
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=checker_environment,
    )
    if cross_tool_result.returncode != 0:
        raise ReceiptError(
            "archived formal ledger has an invalid cross-tool evidence requirement"
        )
    if not cross_tool_result.stdout.strip():
        raise ReceiptError(
            "archived formal release ledger does not require cross-tool evidence"
        )
    toolchain_path, toolchain = _load_tsv(toolchain_path, "formal toolchain")
    _require_fields(
        toolchain,
        {
            "schema_version",
            "java_path",
            "java_sha256",
            "tlapm_path",
            "tlapm_sha256",
            "tla2tools_path",
            "tla2tools_sha256",
            "verus_path",
            "verus_sha256",
            "cargo_verus_path",
            "cargo_verus_sha256",
            "tlc_profile",
            "tlaps_threads",
        },
        "formal toolchain",
    )
    if (
        toolchain["schema_version"] != "1"
        or toolchain["tlc_profile"] != "ci"
        or toolchain["tlaps_threads"] != "1"
    ):
        raise ReceiptError("formal toolchain does not describe the pinned release profile")
    for tool in ("java", "tlapm", "tla2tools", "verus", "cargo_verus"):
        raw_path = Path(toolchain[f"{tool}_path"])
        if not raw_path.is_absolute():
            raise ReceiptError(f"formal {tool} path is not absolute")
        tool_path = _regular_file(raw_path, f"formal {tool} tool")
        digest = toolchain[f"{tool}_sha256"]
        if not _DIGEST_RE.fullmatch(digest) or _sha256(tool_path) != digest:
            raise ReceiptError(f"formal {tool} tool digest mismatch")
    try:
        log_lines = gate_log.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError("formal gate log is not UTF-8") from error
    if (
        not log_lines
        or log_lines[-1] != _FORMAL_FINAL_MARKER
        or log_lines.count(_FORMAL_FINAL_MARKER) != 1
    ):
        raise ReceiptError("formal gate log lacks its one exact final success marker")

    verus_checker = (
        repo_root / "scripts" / "formal" / "sumeragi_v2_verus_evidence.py"
    )
    verus_result = subprocess.run(
        [
            sys.executable,
            str(verus_checker),
            "validate",
            "--root",
            str(repo_root),
            "--evidence",
            str(verus_evidence),
            "--log",
            str(verus_log),
        ],
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=checker_environment,
    )
    if verus_result.returncode != 0:
        raise ReceiptError("archived formal Verus evidence failed validation")
    checker_args = [
        sys.executable,
        str(checker),
        "--ledger",
        str(ledger),
        "--release",
        "--evidence",
        str(evidence),
        "--verus-evidence",
        str(verus_evidence),
        "--verus-log",
        str(verus_log),
        "--cross-tool-evidence",
        str(cross_tool_evidence),
    ]
    result = subprocess.run(
        checker_args,
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=checker_environment,
    )
    if result.returncode != 0:
        raise ReceiptError("archived formal ledger/evidence failed release validation")
    return (
        gate_log,
        ledger,
        evidence,
        verus_evidence,
        verus_log,
        cross_tool_evidence,
        harness_lock,
        toolchain_path,
        tlaps_resource_jsonl,
        tlaps_resource_summary,
    )


def _test_count_from_log(lines: list[str], kind: str, name: str) -> int:
    if kind.startswith("cargo-"):
        running = [
            match
            for line in lines
            if (match := re.fullmatch(r"running ([0-9]+) tests?", line))
        ]
        results = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
                    r"0 measured; [0-9]+ filtered out; finished in .+",
                    line,
                )
            )
        ]
        if (
            len(running) != 1
            or len(results) != 1
            or running[0].group(1) != results[0].group(1)
        ):
            raise ReceiptError(f"{name} has an ambiguous Cargo transcript")
        return int(results[0].group(1))
    if kind == "pytest":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s", line
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(f"{name} has an ambiguous pytest transcript")
        return int(matches[0].group(1))
    if kind == "node":
        matches = [
            match
            for line in lines
            if (match := re.fullmatch(r"# pass ([0-9]+)", line))
        ]
        if len(matches) != 1 or lines.count("# fail 0") != 1:
            raise ReceiptError(f"{name} has an ambiguous Node transcript")
        return int(matches[0].group(1))
    if kind == "command":
        return 0
    raise ReceiptError(f"{name} has unknown leg kind {kind}")


def _corridor_artifacts(
    completion_path: Path,
    fields: dict[str, str],
    sealed: dict[str, Any],
    repo_root: Path,
) -> tuple[Path, Path, list[Path]]:
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
            "summary_sha256",
            "production_required_tests_sha256",
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
            "bash_path",
            "bash_sha256",
            "git_path",
            "git_sha256",
            "cargo_home_path",
            "repo_cargo_config_sha256",
            "tlc_profile",
            "tlaps_threads",
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
    for tool in ("java", "cargo", "rustc", "python3", "node", "bash", "git"):
        tool_path = Path(fields[f"{tool}_path"])
        if not tool_path.is_absolute():
            raise ReceiptError(f"corridor {tool} path is not absolute")
        tool_path = _regular_file(tool_path, f"corridor {tool} tool")
        digest = fields[f"{tool}_sha256"]
        if not _DIGEST_RE.fullmatch(digest) or _sha256(tool_path) != digest:
            raise ReceiptError(f"corridor {tool} tool digest mismatch")
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
    repo_cargo_config = _regular_file(
        repo_root / ".cargo" / "config.toml", "repository Cargo config"
    )
    if (
        not _DIGEST_RE.fullmatch(fields["repo_cargo_config_sha256"])
        or _sha256(repo_cargo_config) != fields["repo_cargo_config_sha256"]
    ):
        raise ReceiptError("repository Cargo config digest mismatch")

    summary = _regular_file(completion_path.with_name("summary.tsv"), "corridor summary")
    required_path = _regular_file(
        completion_path.with_name("production-required-tests.tsv"),
        "corridor production inventory",
    )
    if _sha256(summary) != fields["summary_sha256"]:
        raise ReceiptError("corridor summary digest mismatch")
    if _sha256(required_path) != fields["production_required_tests_sha256"]:
        raise ReceiptError("corridor production inventory digest mismatch")

    try:
        with required_path.open(encoding="utf-8", newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            if tuple(reader.fieldnames or ()) != ("module", "test"):
                raise ReceiptError("corridor production inventory fields are not canonical")
            required_rows = list(reader)
    except UnicodeDecodeError as error:
        raise ReceiptError("corridor production inventory is not UTF-8") from error
    if len(required_rows) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError(
            "corridor production inventory must contain exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    required_names = [row.get("test", "") for row in required_rows]
    if len(set(required_names)) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError("corridor production inventory contains duplicate tests")
    if required_names != _canonical_production_tests(repo_root):
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
        with summary.open(encoding="utf-8", newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            if tuple(reader.fieldnames or ()) != _CORRIDOR_SUMMARY_FIELDS:
                raise ReceiptError("corridor summary fields are not canonical")
            rows = list(reader)
    except UnicodeDecodeError as error:
        raise ReceiptError("corridor summary is not UTF-8") from error
    expected_legs = _corridor_legs()
    if len(rows) != len(expected_legs):
        raise ReceiptError("corridor summary must contain every exact release leg")
    logs: list[Path] = []
    module_for_leg = {leg_id: module for leg_id, module, _ in _PRODUCTION_MODULES}
    exact_cargo_tests: dict[str, tuple[str, ...]] = {
        "status-rust": (_DATA_STATUS_TEST,),
        "lane-certificate-rust": (_DATA_LANE_CERTIFICATE_TEST,),
        "cross-sdk-rust": _CROSS_SDK_TESTS,
    }
    exact_cargo_tests.update(
        {
            f"taira-contract-{index}": (test,)
            for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
        }
    )
    for index, (row, expected_leg) in enumerate(zip(rows, expected_legs, strict=True)):
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
        log = _regular_file(completion_path.parent / expected_log, f"corridor log {index}")
        if _sha256(log) != digest:
            raise ReceiptError(f"corridor log {index} digest mismatch")
        try:
            lines = log.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError as error:
            raise ReceiptError(f"corridor log {index} is not UTF-8") from error
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
        if kind == "cargo-exact":
            for test in exact_cargo_tests[leg_id]:
                if lines.count(f"test {test} ... ok") != 1:
                    raise ReceiptError(
                        f"corridor exact Cargo leg {leg_id} lacks its named test"
                    )
        if kind == "node":
            for test_index, test in enumerate(_JS_STATUS_TESTS, 1):
                if (
                    lines.count(f"# Subtest: {test}") != 1
                    or lines.count(f"ok {test_index} - {test}") != 1
                ):
                    raise ReceiptError(
                        "corridor Node leg lacks its exact TAP subtest result"
                    )
        logs.append(log)
    return summary, required_path, logs


def _seed_run_logs(seed_path: Path, summary: Path, manifest: str) -> list[Path]:
    try:
        with summary.open(encoding="utf-8", newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            if tuple(reader.fieldnames or ()) != _SEED_SUMMARY_FIELDS:
                raise ReceiptError("seed summary fields are not canonical")
            rows = list(reader)
    except UnicodeDecodeError as error:
        raise ReceiptError("seed summary is not UTF-8") from error
    if len(rows) != _SEED_RUN_COUNT:
        raise ReceiptError(
            f"seed summary must contain exactly {_SEED_RUN_COUNT} run rows"
        )

    run_logs = []
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
            f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={manifest} "
            "IROHA_TEST_REQUIRE_NETWORK=1 "
            "IROHA_TEST_NETWORK_START_ATTEMPTS=1 "
            "IROHA_TEST_SKIP_BUILD=0 "
            "IROHA_TEST_ALLOW_REENTRANT_BUILD=1 "
            "IROHA_TEST_BUILD_TIMEOUT_MS=3600 "
            "IROHA_TEST_PROCESS_TIMEOUT_MS=300 "
            "IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 "
            f"IROHA_TEST_NETWORK_BASE_SEED={expected_seed} "
            "TEST_NETWORK_TMP_DIR=${SEED_MATRIX_EVIDENCE_DIRECTORY}/"
            f"{localnet} "
            "IROHA_TEST_NETWORK_KEEP_DIRS=1 "
            "cargo test --locked -p integration_tests --test "
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
        run_log = _regular_file(seed_path.parent / output, f"seed run log {index}")
        if _sha256(run_log) != digest:
            raise ReceiptError(f"seed run log {index} digest mismatch")
        try:
            lines = run_log.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError as error:
            raise ReceiptError(f"seed run log {index} is not UTF-8") from error
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
        run_logs.append(run_log)
    return run_logs


def _seed_localnet_manifests(
    seed_path: Path, fields: dict[str, str]
) -> tuple[Path, list[Path]]:
    if (
        fields["localnet_manifest_count"] != str(_SEED_RUN_COUNT)
        or fields["localnet_manifests_path"] != "localnet-manifests.tsv"
        or not _DIGEST_RE.fullmatch(fields["localnet_manifests_sha256"])
    ):
        raise ReceiptError("seed completion has an invalid localnet manifest binding")
    index_path = _regular_file(
        seed_path.parent / fields["localnet_manifests_path"],
        "seed localnet manifest index",
    )
    index_snapshot = _read_evidence_snapshot(
        index_path,
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

    manifests: list[Path] = []
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
        manifest_path = _regular_file(
            manifest_candidate,
            f"seed localnet manifest {index}",
        )
        snapshot = _read_evidence_snapshot(
            manifest_path,
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
        manifests.append(manifest_path)
    return index_path, manifests


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
    repository_root_path: Path,
    runner_logs_sealed: bool = False,
) -> dict[str, Any]:
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
    candidate = _load_identity(candidate_identity_path, "candidate identity")
    sealed = _load_identity(sealed_identity_path, "sealed identity")
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if candidate[field] != sealed[field]:
            raise ReceiptError(f"candidate and sealed identity disagree on {field}")
    if sealed["head_tree"] != sealed["index_tree"]:
        raise ReceiptError("sealed release index tree is not HEAD")
    # All code was compiled and all child evidence was produced after sealing,
    # so child completions bind the sealed permission-aware manifest. The
    # candidate manifest remains independently recorded in the final receipt.
    manifest = sealed["workspace_source_manifest_sha256"]

    release_authentication, signature_archives = _validate_signature_evidence(
        candidate_identity_path=candidate_identity_path,
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
        candidate_identity_path=candidate_identity_path,
        sealed=sealed,
        expected_signer_fingerprint=expected_signer_fingerprint,
        signature_archives=signature_archives,
        runner_logs_sealed=runner_logs_sealed,
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

    corridor_path, corridor_completion = _load_tsv(
        corridor_completion_path, "corridor completion"
    )
    corridor_summary, corridor_required, corridor_logs = _corridor_artifacts(
        corridor_path, corridor_completion, sealed, repo_root
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
        formal_cross_tool_evidence,
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
        or seed["completed_runs"] != str(_SEED_RUN_COUNT)
        or seed["expected_runs"] != str(_SEED_RUN_COUNT)
    ):
        raise ReceiptError("seed completion does not describe the exact release matrix")
    seed_summary = _regular_file(seed_path.with_name("summary.tsv"), "seed summary")
    if _sha256(seed_summary) != seed["summary_sha256"]:
        raise ReceiptError("seed completion summary digest mismatch")
    seed_run_logs = _seed_run_logs(seed_path, seed_summary, manifest)
    seed_localnet_manifest_index, seed_localnet_manifests = (
        _seed_localnet_manifests(seed_path, seed)
    )

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
    chaos_log = _regular_file(chaos_path.with_name("chaos-100k.log"), "chaos log")
    if _sha256(chaos_log) != chaos["log_sha256"]:
        raise ReceiptError("chaos completion log digest mismatch")
    try:
        chaos_lines = chaos_log.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError("chaos log is not UTF-8") from error
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

    taira_path, taira = _load_tsv(taira_completion_path, "Taira completion")
    _require_fields(
        taira,
        {
            "schema_version",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
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
    ):
        raise ReceiptError("Taira completion is not bound to the exact release identity")
    taira_evidence = _regular_file(
        taira_path.with_name("taira_v2_24h_soak.json"), "Taira evidence"
    )
    if _sha256(taira_evidence) != taira["evidence_sha256"]:
        raise ReceiptError("Taira completion evidence digest mismatch")
    taira_log = _regular_file(
        taira_path.with_name("taira-v2-24h.log"), "Taira run log"
    )
    if _sha256(taira_log) != taira["log_sha256"]:
        raise ReceiptError("Taira completion log digest mismatch")
    try:
        taira_lines = taira_log.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError("Taira run log is not UTF-8") from error
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
    taira_result = subprocess.run(
        [
            sys.executable,
            str(taira_checker),
            str(taira_evidence),
            "--source-manifest",
            manifest,
            "--build-root",
            str(repo_root / "target" / "sumeragi-v2-release" / manifest),
            "--repo-root",
            str(repo_root),
        ],
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=checker_environment,
    )
    if taira_result.returncode != 0:
        raise ReceiptError("archived Taira evidence failed release validation")

    return {
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
            "corridor_logs": [_artifact(path) for path in corridor_logs],
            "formal_completion": _artifact(formal_path),
            "formal_gate_log": _artifact(formal_log),
            "formal_proof_coverage": _artifact(formal_ledger),
            "formal_proof_evidence": _artifact(formal_evidence),
            "formal_verus_evidence": _artifact(formal_verus_evidence),
            "formal_verus_log": _artifact(formal_verus_log),
            "formal_cross_tool_evidence": _artifact(formal_cross_tool_evidence),
            "formal_harness_lock": _artifact(formal_harness_lock),
            "formal_toolchain": _artifact(formal_toolchain),
            "formal_tlaps_resource_jsonl": _artifact(formal_tlaps_resource_jsonl),
            "formal_tlaps_resource_summary": _artifact(formal_tlaps_resource_summary),
            "seed_matrix_completion": _artifact(seed_path),
            "seed_matrix_summary": _artifact(seed_summary),
            "seed_matrix_run_logs": [_artifact(path) for path in seed_run_logs],
            "seed_matrix_localnet_manifest_index": _artifact(
                seed_localnet_manifest_index
            ),
            "seed_matrix_localnet_manifests": [
                _artifact(path) for path in seed_localnet_manifests
            ],
            "chaos_completion": _artifact(chaos_path),
            "chaos_log": _artifact(chaos_log),
            "taira_completion": _artifact(taira_path),
            "taira_evidence": _artifact(taira_evidence),
            "taira_run_log": _artifact(taira_log),
        },
    }


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
    expected_sha256: str,
    expected_mode: int | None = None,
    expected_owner: int | None = None,
    expected_nlink: int | None = None,
    expected_size: int | None = None,
) -> PathContract:
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
        if observed_sha != expected_sha256 or size != opened.st_size:
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
    candidate_identity_path: Path,
    sealed_identity_path: Path,
) -> list[PathContract | DirectoryContract]:
    records = list(_iter_artifact_records(receipt["authentication"])) + list(
        _iter_artifact_records(receipt["evidence"])
    )
    records.extend(
        (
            {
                "path": str(_regular_file(candidate_identity_path, "candidate identity")),
                "sha256": _sha256(candidate_identity_path),
                "owner_uid": os.geteuid(),
                "nlink": 1,
            },
            {
                "path": str(_regular_file(sealed_identity_path, "sealed identity")),
                "sha256": _sha256(sealed_identity_path),
                "owner_uid": os.geteuid(),
                "nlink": 1,
            },
        )
    )
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
        Path(receipt["authentication"]["bootstrap"]["candidate_root"]),
        Path(receipt["authentication"]["bootstrap"]["runner"]["tool_directory"]),
        Path(receipt["authentication"]["release_identity"]["release_root"]),
    }
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
        os.close(directory_fd)


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
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--verify-existing",
        action="store_true",
        help="rebuild and durably verify an existing no-clobber receipt",
    )
    args = parser.parse_args()
    try:
        receipt = build_receipt(
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
            repository_root_path=args.repository_root,
            runner_logs_sealed=args.verify_existing,
        )
        snapshots = _snapshot_receipt_inputs(
            receipt,
            candidate_identity_path=args.candidate_identity,
            sealed_identity_path=args.sealed_identity,
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
            final_snapshots = _snapshot_receipt_inputs(
                receipt,
                candidate_identity_path=args.candidate_identity,
                sealed_identity_path=args.sealed_identity,
            )
            final_snapshots.append(
                _existing_receipt_contract(args.output, receipt_bytes)
            )
            _fsync_receipt_inputs(final_snapshots)
    except (OSError, ReceiptError) as error:
        print(f"Sumeragi v2 release receipt error: {error}", file=sys.stderr)
        return 1
    action = "verified" if args.verify_existing else "published"
    print(
        f"Sumeragi v2 aggregate release receipt {action}: {args.output.resolve()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
