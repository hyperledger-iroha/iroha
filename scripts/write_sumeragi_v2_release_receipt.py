#!/usr/bin/env python3
"""Validate and aggregate source-bound Sumeragi v2 release evidence."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_FORMAL_FINAL_MARKER = (
    "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler "
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
    "real_network_divergent_prepare_qcs_converge_after_ordered_release",
)
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
_PRODUCTION_TEST_COUNT = 166
_PRODUCTION_MODULES = (
    (
        "production-authoritative-ingress",
        "sumeragi::authoritative_runtime_gate_tests",
        8,
    ),
    ("production-v2-core", "sumeragi::v2_core::tests", 12),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 2),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        3,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 24),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 1),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 37),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 13),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 20),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 2),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 10),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 17),
    (
        "production-v2-watchdog",
        "sumeragi::status::v2_liveness_watchdog_tests",
        14,
    ),
)
_DATA_STATUS_TEST = (
    "block::consensus_v2::tests::"
    "status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry"
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
            not test.startswith("sumeragi::") for test in tests
        )
    ):
        raise ReceiptError(
            "release runner production inventory is not exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    return tests


def _corridor_legs() -> list[tuple[str, str, int, str]]:
    legs = [
        (
            leg_id,
            "cargo-module",
            count,
            f"cargo test --locked -p iroha_core --lib {module} -- --test-threads=1",
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
                10,
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
                "test_mocked_seed_matrix_refuses_uninspected_stale_lock",
            ),
            (
                "preflight-chaos-launcher",
                "pytest",
                5,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_chaos_release_test.py",
            ),
            (
                "preflight-release-receipt",
                "pytest",
                37,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_release_receipt_test.py",
            ),
            (
                "preflight-formal-launcher",
                "pytest",
                12,
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


def _load_identity(path: Path, name: str) -> dict[str, Any]:
    path = _regular_file(path, name)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReceiptError(f"{name} is not canonical JSON: {error}") from error
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise ReceiptError(f"{name} fields do not match the release identity schema")
    if value.get("schema_version") != 1:
        raise ReceiptError(f"{name} has the wrong schema version")
    for field in ("head_commit", "head_tree", "index_tree"):
        item = value.get(field)
        if not isinstance(item, str) or not _OBJECT_ID_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase Git object ID")
    for field in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        item = value.get(field)
        if not isinstance(item, str) or not _DIGEST_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase SHA-256 digest")
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
) -> tuple[Path, Path, Path, Path, Path]:
    _require_fields(
        fields,
        {
            "schema_version",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "formal_gate_log_sha256",
            "proof_coverage_sha256",
            "proof_evidence_sha256",
            "harness_cargo_lock_sha256",
            "formal_toolchain_sha256",
        },
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
    ledger = _regular_file(
        completion_path.with_name("proof_coverage.json"), "formal proof ledger"
    )
    evidence = _regular_file(
        completion_path.with_name("proof_evidence.json"), "formal proof evidence"
    )
    harness_lock = _regular_file(
        completion_path.with_name("harness-Cargo.lock"), "formal harness lock"
    )
    toolchain_path = _regular_file(
        completion_path.with_name("formal-toolchain.tsv"), "formal toolchain"
    )
    for artifact, digest_field, name in (
        (gate_log, "formal_gate_log_sha256", "formal gate log"),
        (ledger, "proof_coverage_sha256", "formal proof ledger"),
        (evidence, "proof_evidence_sha256", "formal proof evidence"),
        (harness_lock, "harness_cargo_lock_sha256", "formal harness lock"),
        (toolchain_path, "formal_toolchain_sha256", "formal toolchain"),
    ):
        if _sha256(artifact) != fields[digest_field]:
            raise ReceiptError(f"{name} digest mismatch")
    if fields["harness_cargo_lock_sha256"] != _HARNESS_LOCK_SHA256:
        raise ReceiptError("formal harness lock is not the pinned dependency graph")
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
        or toolchain["tlaps_threads"] != "4"
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

    repo_root = Path(__file__).resolve().parents[1]
    checker = repo_root / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
    result = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--ledger",
            str(ledger),
            "--release",
            "--evidence",
            str(evidence),
        ],
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise ReceiptError("archived formal ledger/evidence failed release validation")
    return gate_log, ledger, evidence, harness_lock, toolchain_path


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
    raise ReceiptError(f"{name} has unknown leg kind {kind}")


def _corridor_artifacts(
    completion_path: Path,
    fields: dict[str, str],
    sealed: dict[str, Any],
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
        "tlaps_threads": "4",
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
    repo_root = Path(__file__).resolve().parents[1]
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
    if len(rows) != 128:
        raise ReceiptError("seed summary must contain exactly 128 run rows")

    run_logs = []
    for index, row in enumerate(rows):
        if None in row or set(row) != set(_SEED_SUMMARY_FIELDS):
            raise ReceiptError(f"seed summary row {index} has extra or missing columns")
        scenario = _SEED_SCENARIOS[index // 32]
        seed_index = index % 32
        expected_seed = (
            scenario if seed_index == 0 else f"{scenario}:seed:{seed_index:02d}"
        )
        output = f"runs/run-{index:03d}.log"
        localnet = f"localnets/run-{index:03d}"
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
            or not row.get("command")
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


def build_receipt(
    *,
    candidate_identity_path: Path,
    sealed_identity_path: Path,
    corridor_completion_path: Path,
    formal_completion_path: Path,
    seed_completion_path: Path,
    chaos_completion_path: Path,
    taira_completion_path: Path,
) -> dict[str, Any]:
    """Validate every completion artifact and return one aggregate receipt."""

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

    corridor_path, corridor_completion = _load_tsv(
        corridor_completion_path, "corridor completion"
    )
    corridor_summary, corridor_required, corridor_logs = _corridor_artifacts(
        corridor_path, corridor_completion, sealed
    )

    formal_path, formal_completion = _load_tsv(
        formal_completion_path, "formal completion"
    )
    formal_log, formal_ledger, formal_evidence, formal_harness_lock, formal_toolchain = _formal_artifacts(
        formal_path, formal_completion, sealed
    )
    seed_path, seed = _load_tsv(seed_completion_path, "seed completion")
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
        },
        "seed completion",
    )
    if (
        seed["schema_version"] != "1"
        or seed["profile"] != "release"
        or seed["head_commit"] != sealed["head_commit"]
        or seed["head_tree"] != sealed["head_tree"]
        or seed["source_manifest_sha256"] != manifest
        or seed["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or seed["completed_runs"] != "128"
        or seed["expected_runs"] != "128"
    ):
        raise ReceiptError("seed completion does not describe the exact release matrix")
    seed_summary = _regular_file(seed_path.with_name("summary.tsv"), "seed summary")
    if _sha256(seed_summary) != seed["summary_sha256"]:
        raise ReceiptError("seed completion summary digest mismatch")
    seed_run_logs = _seed_run_logs(seed_path, seed_summary, manifest)

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
    repo_root = Path(__file__).resolve().parents[1]
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
        "evidence": {
            "corridor_completion": _artifact(corridor_path),
            "corridor_summary": _artifact(corridor_summary),
            "corridor_production_inventory": _artifact(corridor_required),
            "corridor_logs": [_artifact(path) for path in corridor_logs],
            "formal_completion": _artifact(formal_path),
            "formal_gate_log": _artifact(formal_log),
            "formal_proof_coverage": _artifact(formal_ledger),
            "formal_proof_evidence": _artifact(formal_evidence),
            "formal_harness_lock": _artifact(formal_harness_lock),
            "formal_toolchain": _artifact(formal_toolchain),
            "seed_matrix_completion": _artifact(seed_path),
            "seed_matrix_summary": _artifact(seed_summary),
            "seed_matrix_run_logs": [_artifact(path) for path in seed_run_logs],
            "chaos_completion": _artifact(chaos_path),
            "chaos_log": _artifact(chaos_log),
            "taira_completion": _artifact(taira_path),
            "taira_evidence": _artifact(taira_evidence),
            "taira_run_log": _artifact(taira_log),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-identity", type=Path, required=True)
    parser.add_argument("--sealed-identity", type=Path, required=True)
    parser.add_argument("--corridor-completion", type=Path, required=True)
    parser.add_argument("--formal-completion", type=Path, required=True)
    parser.add_argument("--seed-completion", type=Path, required=True)
    parser.add_argument("--chaos-completion", type=Path, required=True)
    parser.add_argument("--taira-completion", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        receipt = build_receipt(
            candidate_identity_path=args.candidate_identity,
            sealed_identity_path=args.sealed_identity,
            corridor_completion_path=args.corridor_completion,
            formal_completion_path=args.formal_completion,
            seed_completion_path=args.seed_completion,
            chaos_completion_path=args.chaos_completion,
            taira_completion_path=args.taira_completion,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.output.with_name(f".{args.output.name}.{os.getpid()}")
        temporary.write_text(
            json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        os.replace(temporary, args.output)
    except (OSError, ReceiptError) as error:
        print(f"Sumeragi v2 release receipt error: {error}", file=sys.stderr)
        return 1
    print(f"Sumeragi v2 aggregate release receipt: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
