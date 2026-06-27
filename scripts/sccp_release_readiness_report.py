#!/usr/bin/env python3
"""Render SCCP release-readiness notes from evidence and validation results."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import shlex
import subprocess
import sys
import unicodedata
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[1]
ALL_LANES_SCRIPT = ROOT / "scripts" / "sccp_all_lanes_evidence.py"
VERIFY_RELEASE_BUNDLE_SCRIPT = ROOT / "scripts" / "sccp_verify_release_bundle.py"
ACTIVE_LAUNCH_DOMAIN = 1
ACTIVE_LAUNCH_CHAIN = "eth"
ACTIVE_LAUNCH_POLICY = "EthereumMainnetLane"
ACTIVE_LAUNCH_DISPLAY = "Ethereum mainnet"
SCCP_DOMAIN_SORA = 0
ALL_LANES_CHAIN_BY_DOMAIN = {
    1: "eth",
    2: "bsc",
    3: "sol",
    4: "ton",
    5: "tron",
}
MESSAGE_PROOF_ROUTE_CANARY_DOMAINS = frozenset(
    domain
    for domain, chain in ALL_LANES_CHAIN_BY_DOMAIN.items()
    if chain in {"eth", "bsc", "tron"}
)
ALL_LANES_REQUIRED_DOMAINS = tuple(ALL_LANES_CHAIN_BY_DOMAIN)
ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN = {
    1: frozenset(("evm_source_gate_hash",)),
    2: frozenset(("evm_source_gate_hash",)),
    3: frozenset(
        (
            "solana_tower_replay_verifier_hash",
            "solana_full_accountsdb_lattice_verifier_hash",
            "solana_bank_fork_choice_verifier_hash",
            "solana_full_light_client_gate_hash",
        )
    ),
    4: frozenset(
        (
            "ton_masterchain_config_verifier_hash",
            "ton_validator_set_transition_verifier_hash",
            "ton_shard_accounts_dictionary_verifier_hash",
            "ton_full_light_client_gate_hash",
        )
    ),
    5: frozenset(("tron_dpos_source_gate_hash",)),
}
ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN = {
    1: "evm_source_gate_hash",
    2: "evm_source_gate_hash",
    3: "solana_full_light_client_gate_hash",
    4: "ton_full_light_client_gate_hash",
    5: "tron_dpos_source_gate_hash",
}
READINESS_REPORT_PUBLIC_FIELDS = (
    "production_ready",
    "evidence",
    "release_checklist",
    "corridor",
    "blockers",
    "inputs",
    "input_artifacts",
    "native_evm_prover_bundle",
    "source_inventory",
    "cryptographic_evidence",
    "user_prover_submission_surfaces",
)
RELEASE_CHECKLIST_PUBLIC_FIELDS = frozenset(("ready", "items"))
RELEASE_CHECKLIST_ITEM_PUBLIC_FIELDS = frozenset(("id", "title", "ready", "blockers"))
INPUT_ARTIFACT_PUBLIC_FIELDS = frozenset(("path", "bytes", "sha256"))
CORRIDOR_PUBLIC_FIELDS = frozenset(
    (
        "production_ready",
        "phases",
        "evidence_artifacts",
        "require_phase_evidence",
        "blockers",
    )
)
CORRIDOR_PUBLIC_PHASE_STATUSES = frozenset(("passed", "failed", "skipped", "missing"))
SOURCE_INVENTORY_PUBLIC_FIELDS = frozenset(("validation_status", "validation_blockers"))
USER_PROVER_SUBMISSION_SURFACE_PUBLIC_FIELDS = frozenset(
    (
        "lanes",
        "proof_backend",
        "sdk_helper_symbols",
        "sdk_helper_symbols_by_sdk",
        "sdk_helpers",
        "on_chain_submission",
        "required_phases",
        "validation_status",
        "validation_blockers",
    )
)
CRYPTOGRAPHIC_EVIDENCE_PUBLIC_FIELDS = frozenset(
    (
        "domain",
        "chain",
        "evm_source_rpc_chain_id",
        "evm_source_block_tag",
        "evm_destination_rpc_chain_id",
        "evm_destination_block_tag",
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
        "route_canary_evidence_source",
        "route_canary_evidence_bound",
        "route_canary_message_proof_used",
        "route_canary_raw_data_owner_matches_transaction",
        "route_canary_signature_recovers_to_owner",
        "route_canary_log_index",
        "route_canary_target_domain",
        "route_canary_proof_version",
        "route_canary_proof_source_domain",
        "route_canary_call_data_sha256",
        "route_canary_payload_hash",
        "route_canary_statement_hash",
        "route_canary_commitment_root",
        "route_canary_finality_height",
        "route_canary_finality_block_hash",
        "route_canary_transaction_hash",
        "route_canary_receipt_block_number",
        "route_canary_receipt_block_hash",
        "route_canary_receipt_block_finalized",
        "route_canary_block_receipts_root",
        "route_canary_message_id",
        "route_canary_block_number",
        "route_canary_block_timestamp",
        "source_adapter_gate_required",
        "source_adapter_gate_hash",
        "source_adapter_gate_audit_hashes",
    )
)
NATIVE_EVM_PROVER_BUNDLE_PUBLIC_FIELDS = frozenset(
    (
        "required",
        "schema",
        "artifact",
        "bundle_id",
        "lanes",
        "proof_backend",
        "proof_artifact",
        "proof_artifact_hash",
        "proving_key",
        "proving_key_hash",
        "verifier_key",
        "verifier_key_hash",
        "destination_binding_hash",
        "audit_hashes",
        "cross_sdk_fixture_parity_artifact",
        "native_prover_self_test_artifact",
        "sdk_artifacts",
        "validation_status",
        "validation_blockers",
    )
)
NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_PUBLIC_FIELDS = frozenset(
    (
        "sdk",
        "implementation",
        "implementation_hash",
        "implementation_artifact",
    )
)
CRYPTOGRAPHIC_EVIDENCE_TEXT_FIELDS = frozenset(
    (
        "evm_source_rpc_chain_id",
        "evm_source_block_tag",
        "evm_destination_rpc_chain_id",
        "evm_destination_block_tag",
        "route_canary_evidence_source",
    )
)
CRYPTOGRAPHIC_EVIDENCE_HASH_FIELDS = frozenset(
    (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
        "route_canary_call_data_sha256",
        "route_canary_payload_hash",
        "route_canary_statement_hash",
        "route_canary_commitment_root",
        "route_canary_finality_height",
        "route_canary_finality_block_hash",
        "route_canary_transaction_hash",
        "route_canary_receipt_block_hash",
        "route_canary_block_receipts_root",
        "route_canary_message_id",
        "source_adapter_gate_hash",
    )
)
CRYPTOGRAPHIC_EVIDENCE_INTEGER_FIELDS = frozenset(
    (
        "route_canary_receipt_block_number",
        "route_canary_log_index",
        "route_canary_target_domain",
        "route_canary_proof_version",
        "route_canary_proof_source_domain",
        "route_canary_block_number",
        "route_canary_block_timestamp",
    )
)
ACTIVE_LAUNCH_RELEASE_CHECKLIST_ITEM_IDS = (
    "all_required_lane_records",
    "governed_deployment_evidence",
    "route_allowlist_binding",
    "live_route_canary_evidence",
    "native_evm_groth16_prover_bundle",
    "no_unresolved_blockers",
)
ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES = {
    "all_required_lane_records": (
        f"Active {ACTIVE_LAUNCH_DISPLAY} SCCP lane has the required source, "
        "deployment, destination, and route records"
    ),
    "governed_deployment_evidence": (
        f"{ACTIVE_LAUNCH_DISPLAY} source-adapter deployment and destination "
        "rollout are governed and hash-bound"
    ),
    "route_allowlist_binding": (
        f"{ACTIVE_LAUNCH_DISPLAY} route allowlist binds the governed source and "
        "destination evidence"
    ),
    "live_route_canary_evidence": (
        f"{ACTIVE_LAUNCH_DISPLAY} post-deploy route canary evidence is live, "
        "passed, and bound to the route"
    ),
    "native_evm_groth16_prover_bundle": (
        f"{ACTIVE_LAUNCH_DISPLAY} browser and native SDKs ship an audited "
        "no-WASM, no-remote EVM Groth16 prover bundle"
    ),
    "no_unresolved_blockers": (
        f"No active {ACTIVE_LAUNCH_DISPLAY} launch blockers remain"
    ),
}
ACTIVE_LAUNCH_EVM_CHAIN_ID_EVIDENCE = {
    "eth": "`eth_chainId == 0x1` (1)",
    "bsc": "`eth_chainId == 0x38` (56)",
}.get(ACTIVE_LAUNCH_CHAIN, "the configured mainnet chain id")
ACTIVE_LAUNCH_EVM_DECIMAL_CHAIN_ID = {
    "eth": "1",
    "bsc": "56",
}.get(ACTIVE_LAUNCH_CHAIN)
SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE = (
    "SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now."
)
SCCP_NOT_REMAINING_WORK_SCOPE_NOTE = (
    "Do not track Sub&#115;trate/Pol&#107;adot relayers, route manifests, proof "
    "fixtures, SDK helpers, or public discovery routes as remaining SCCP launch "
    "work in this cycle."
)
ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE = "evm_message_proof_accepted_transaction"
CORRIDOR_SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
CORRIDOR_COMPLETION_SENTINEL = "SCCP production corridor completed."
CORRIDOR_DRY_RUN_SENTINEL = "SCCP production corridor dry run completed."
CORRIDOR_PHASE_MARKER_PREFIX = "==> SCCP production corridor: "
USER_PROVER_SDK_PHASES = (
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
)
USER_PROVER_CHAIN_PHASES = (*USER_PROVER_SDK_PHASES, "core-admission")
EVM_NATIVE_DOTNET_PHASE = "dotnet-sdk"
DOTNET_VERSION_SUCCESS_PREFIX = "SCCP .NET SDK version: 8."
DOTNET_VERSION_SUCCESS_PATTERN = re.compile(
    r"^SCCP \.NET SDK version: 8\.0\.[1-9][0-9]*$",
)
DOTNET_WINDOWS_OS_SUCCESS_LINE = "SCCP .NET SDK OS: Windows"
DOTNET_RID_SUCCESS_PREFIX = "SCCP .NET SDK RID: win-"
DOTNET_RID_SUCCESS_PATTERN = re.compile(
    r"^SCCP \.NET SDK RID: win-(?:x64|x86|arm64|arm)$",
)
DOTNET_RID_ARCHITECTURES = {
    "win-x64": "x64",
    "win-x86": "x86",
    "win-arm64": "arm64",
    "win-arm": "arm",
}
DOTNET_ARCHITECTURE_SUCCESS_PREFIX = "SCCP .NET SDK Architecture:"
DOTNET_ARCHITECTURE_SUCCESS_PATTERN = re.compile(
    rf"^{re.escape(DOTNET_ARCHITECTURE_SUCCESS_PREFIX)} (?:x64|x86|arm64|arm)$",
)
DOTNET_BRIDGE_PATH_SUCCESS_PREFIX = "connect_norito_bridge native bridge:"
DOTNET_BRIDGE_PATH_SUCCESS_PATTERN = re.compile(
    rf"^{re.escape(DOTNET_BRIDGE_PATH_SUCCESS_PREFIX)} (?P<path>.+)$",
)
DOTNET_BRIDGE_PATH_COMPONENT_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+$")
DOTNET_BRIDGE_PATH_DRIVE_PATTERN = re.compile(r"^[A-Za-z]:$")
DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX = "connect_norito_bridge native bridge sha256:"
DOTNET_BRIDGE_SHA256_SUCCESS_PATTERN = re.compile(
    rf"^{re.escape(DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX)} [0-9a-f]{{64}}$",
)
DOTNET_TEST_PASSED_SUCCESS_FRAGMENT = "Passed!"
DOTNET_TEST_ASSEMBLY_SUCCESS_SUFFIX = (
    "Hyperledger.Iroha.Sdk.Tests.dll (net8.0)"
)
DOTNET_TEST_DURATION_SUCCESS_PATTERN = (
    r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[ ]+(?:ms|s|m|h)"
    r"(?:[ ]+(?:0|[1-9][0-9]*)(?:\.[0-9]+)?[ ]+(?:ms|s|m|h))*"
)
DOTNET_TEST_PASSED_SUCCESS_PATTERN = re.compile(
    r"^[ ]*Passed![ ]+-[ ]+Failed:[ ]+0,[ ]+Passed:[ ]+(?P<passed>[1-9][0-9]*),"
    r"[ ]+Skipped:[ ]+(?P<skipped>0),"
    rf"[ ]+Total:[ ]+(?P<total>[1-9][0-9]*),[ ]+Duration:[ ]+"
    rf"(?P<duration>{DOTNET_TEST_DURATION_SUCCESS_PATTERN})[ ]+-[ ]+"
    rf"{re.escape(DOTNET_TEST_ASSEMBLY_SUCCESS_SUFFIX)}$",
)
DOTNET_TRX_SUCCESS_PREFIX = "SCCP .NET SDK TRX:"
DOTNET_TRX_SUCCESS_PATTERN = re.compile(
    r"^SCCP \.NET SDK TRX: "
    r"csharp/tests/Hyperledger\.Iroha\.Sdk\.Tests/"
    r"TestResults/sccp-dotnet-sdk\.trx$",
)
DOTNET_TRX_BYTES_SUCCESS_PREFIX = "SCCP .NET SDK TRX bytes:"
DOTNET_TRX_BYTES_SUCCESS_PATTERN = re.compile(
    r"^SCCP \.NET SDK TRX bytes: [1-9][0-9]*$",
)
DOTNET_SUCCESS_OUTSIDE_WINDOW_ERROR_FRAGMENT = (
    ".NET success marker appears outside its required command window"
)
PHASE_SUCCESS_OUTSIDE_WINDOW_ERROR_FRAGMENT = (
    "success marker appears outside its required command window"
)
PHASE_DUPLICATE_COMMAND_ERROR_FRAGMENT = "command appears more than once"
PHASE_DUPLICATE_SUCCESS_ERROR_FRAGMENT = (
    "success marker appears more times than required command windows"
)
NATIVE_EVM_PROVER_BUNDLE_SCHEMA = "sccp-native-evm-groth16-prover-bundle-v1"
NATIVE_EVM_PROVER_BUNDLE_ID = (
    "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1"
)
NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS = {
    "javascript": "pure-typescript",
    "swift": "native-swift",
    "kotlin": "native-kotlin",
    "java-android": "native-java",
    "dotnet": "native-csharp",
}
NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES = (
    "circuit_security_audit",
    "native_implementation_audit",
    "reproducible_build_attestation",
    "cross_sdk_fixture_parity",
    "native_prover_self_test",
    "no_wasm_no_remote_scan",
)
NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA = (
    "sccp-ethereum-mainnet-native-evm-cross-sdk-fixture-parity-v1"
)
NATIVE_EVM_PROVER_PARITY_FIXTURE_REQUIRED_KEYS = {
    "schema",
    "domain",
    "chain",
    "proof_backend",
    "proof_artifact_hash",
    "proving_key_hash",
    "verifier_key_hash",
    "destination_binding_hash",
    "receipt_proof_hash",
    "source_proof_hash",
    "public_signal_words",
    "calldata_hash",
    "torii_submit_payload_hash",
    "sdk_results",
}
NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS = {
    "receipt_proof_hash",
    "source_proof_hash",
    "destination_binding_hash",
    "public_signal_words",
    "calldata_hash",
    "torii_submit_payload_hash",
}
NATIVE_EVM_PROVER_PARITY_HASH_ROLE_KEYS = (
    "receipt_proof_hash",
    "source_proof_hash",
    "calldata_hash",
    "torii_submit_payload_hash",
)
NATIVE_EVM_PROVER_SELF_TEST_SCHEMA = (
    "sccp-ethereum-mainnet-native-evm-prover-self-test-v1"
)
NATIVE_EVM_PROVER_SELF_TEST_REQUIRED_KEYS = {
    "schema",
    "domain",
    "chain",
    "proof_backend",
    "proof_artifact_hash",
    "proving_key_hash",
    "verifier_key_hash",
    "destination_binding_hash",
    "request_hash",
    "witness_hash",
    "source_proof_hash",
    "proof_hash",
    "public_signal_words",
    "calldata_hash",
    "torii_submit_payload_hash",
    "sdk_results",
}
NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS = {
    "request_hash",
    "witness_hash",
    "source_proof_hash",
    "proof_hash",
    "public_signal_words",
    "calldata_hash",
    "torii_submit_payload_hash",
}
NATIVE_EVM_PROVER_SELF_TEST_HASH_ROLE_KEYS = (
    "request_hash",
    "witness_hash",
    "source_proof_hash",
    "proof_hash",
    "calldata_hash",
    "torii_submit_payload_hash",
)
NATIVE_EVM_PROVER_BUNDLE_REQUIRED_KEYS = {
    "schema",
    "bundle_id",
    "domain",
    "chain",
    "proof_backend",
    "proof_artifact",
    "proof_artifact_hash",
    "proving_key",
    "proving_key_hash",
    "verifier_key",
    "verifier_key_hash",
    "destination_binding_hash",
    "no_wasm",
    "remote_prover_required",
    "browser_implementation",
    "native_sdk_artifacts",
    "cross_sdk_fixture_parity_artifact",
    "native_prover_self_test_artifact",
    "audit_hashes",
}
NATIVE_EVM_PROVER_SDK_ARTIFACT_KEYS = {
    "sdk",
    "implementation",
    "prover_artifact_hash",
    "proving_key_hash",
    "implementation_artifact",
    "implementation_hash",
}
NATIVE_EVM_PROVER_FORBIDDEN_PAYLOAD_MARKERS = (
    b"webassembly",
    b"wasm",
    b"snarkjs",
    b"remoteprover",
    b"remote prover",
    b"remote_prover",
    b"prover_url",
    b"prover-url",
    b"proverendpoint",
    b"prover endpoint",
)
NATIVE_EVM_PROVER_FORBIDDEN_PATH_MARKERS = (
    "webassembly",
    "wasm",
    "snarkjs",
    "remoteprover",
    "remote-prover",
    "remote_prover",
    "remote prover",
    "prover-url",
    "prover_url",
    "proverendpoint",
    "prover-endpoint",
    "prover_endpoint",
    "prover endpoint",
)
NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES = 128
NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES = 1024
NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES = 64 * 1024
NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES = 64 * 1024
NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES = 128
NATIVE_EVM_PROVER_MIN_PAYLOAD_BYTES = NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES


class DuplicateJsonKeyError(ValueError):
    """Raised when a JSON object contains a duplicate key."""

    def __init__(self, key: str) -> None:
        super().__init__(key)
        self.key = key


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for key, value in pairs:
        if key in payload:
            raise DuplicateJsonKeyError(key)
        payload[key] = value
    return payload


def _load_json_without_duplicate_keys(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_keys,
    )


def _load_release_bundle_verify_helpers() -> Any:
    """Load release-bundle verifier helpers used by readiness source gates."""

    spec = importlib.util.spec_from_file_location(
        "sccp_release_bundle_verify_helpers_for_readiness_report",
        VERIFY_RELEASE_BUNDLE_SCRIPT,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("release-bundle verifier helper module cannot be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _sccp_proof_request_bundle_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for SCCP proof-request bundle gates."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_proof_request_bundle_gate_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP proof-request bundle/source-proof gate source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_phase_evidence_source_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for SCCP phase-evidence inputs."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_phase_evidence_source_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP phase evidence duplicate-input source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _release_corridor_phase_transcript_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for strict corridor transcript checks."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_corridor_phase_transcript_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release corridor phase-transcript source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_bundle_source_copy_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for fail-closed release bundle copies."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_release_bundle_source_copy_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release bundle source-copy source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_bundle_output_path_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for release bundle output paths."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_release_bundle_output_path_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release bundle output-path source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_artifact_path_text_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Markdown-safe artifact paths."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_release_artifact_path_text_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release artifact path text source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_input_provenance_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for copied input provenance schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_input_provenance_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release input-provenance schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_json_root_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public JSON-root schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_json_root_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public JSON-root schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_markdown_text_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public Markdown text schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_markdown_text_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public Markdown text schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public cryptographic-evidence binding."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_crypto_evidence_binding_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public cryptographic-evidence binding source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_submission_surface_binding_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public submission-surface binding."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_submission_surface_binding_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public submission-surface binding source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_manifest_readiness_flags_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for exact release manifest readiness flags."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_manifest_readiness_flags_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release manifest readiness-flags source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_route_allowlist_canary_summary_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for route-allowlist canary summaries."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_route_allowlist_canary_summary_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP route allowlist canary summary source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_transparent_openverify_summary_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for transparent OpenVerify summaries."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_transparent_openverify_summary_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP transparent OpenVerify summary source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_manifest_artifact_set_order_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for release manifest artifact set/order."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_manifest_artifact_set_order_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release manifest artifact-set/order source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_blocker_list_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public release blocker-list schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_blocker_list_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public blocker-list schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_public_scalar_text_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public release scalar-text schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_public_scalar_text_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release public scalar-text schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_notes_attachment_invariants_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for release-notes attachment invariants."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_notes_attachment_invariants_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release-notes attachment invariants source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_readiness_markdown_invariants_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for readiness Markdown invariants."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_readiness_markdown_invariants_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP readiness Markdown invariants source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_retired_network_surface_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for retired network-surface guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_retired_network_surface_guard_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP retired network-surface guard source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_launch_scope_constant_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for SCCP launch-scope constants."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_sccp_launch_scope_constant_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP launch-scope constants source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_launch_policy_selector_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for the Ethereum launch-policy selector."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_launch_policy_selector_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet launch-policy selector source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_launch_policy_documentation_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
    forbidden_markers: tuple[str, ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for active Ethereum launch-policy docs."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_launch_policy_documentation_inventory_errors",
        )
        if inventory is None and forbidden_markers is None:
            return list(helper())
        return list(helper(inventory, forbidden_markers))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet launch-policy documentation source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_public_discovery_documentation_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
    forbidden_markers: tuple[str, ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for public SCCP discovery docs."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_public_discovery_documentation_inventory_errors",
        )
        if inventory is None and forbidden_markers is None:
            return list(helper())
        return list(helper(inventory, forbidden_markers))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP public discovery documentation source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _bsc_groth16_material_documentation_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for BSC Groth16 material docs."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_bsc_groth16_material_documentation_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "BSC Groth16 material documentation source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _bsc_groth16_material_evidence_guard_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for BSC Groth16 material evidence guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_bsc_groth16_material_evidence_guard_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "BSC Groth16 material evidence guard source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_data_collection_no_proxy_gate_inventory_errors(
    regions: dict[str, tuple[str | Path, str, str, tuple[str, ...]]] | None = None,
) -> list[str]:
    """Return SCCP Ethereum no-proxy data-collection source inventory errors."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_data_collection_no_proxy_inventory_errors")
        if regions is None:
            return list(helper())
        return list(helper(regions))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet no-proxy data-collection source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_inbound_adversarial_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum inbound adversarial guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_inbound_adversarial_sdk_test_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet inbound adversarial source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _bsc_inbound_adversarial_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for BSC inbound adversarial guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_bsc_inbound_adversarial_sdk_test_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "BSC mainnet inbound adversarial source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _tron_inbound_adversarial_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for TRON inbound adversarial guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_tron_inbound_adversarial_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "TRON mainnet inbound adversarial source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _bsc_route_config_canonical_manifest_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for BSC route-config manifest guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_bsc_route_config_canonical_manifest_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP BSC route-config canonical-manifest source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _tron_route_config_canonical_manifest_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for TRON route-config manifest guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_tron_route_config_canonical_manifest_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP TRON route-config canonical-manifest source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _tron_runtime_route_manifest_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for TRON runtime route-manifest guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_tron_runtime_route_manifest_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP TRON runtime route-manifest source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _all_lanes_route_canary_scalar_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for all-lanes route-canary scalar guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_all_lanes_route_canary_scalar_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP all-lanes route-canary scalar source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _all_lanes_evidence_root_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for all-lanes evidence-root schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_all_lanes_evidence_root_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP all-lanes evidence-root schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _all_lanes_governed_blocker_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for all-lanes governed blocker schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_all_lanes_governed_blocker_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP all-lanes governed blocker schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for all-lanes exact-boolean checklist guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_all_lanes_release_checklist_exact_boolean_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP all-lanes release-checklist exact-boolean source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _active_launch_checklist_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for active-launch checklist schemas."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_active_launch_checklist_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP active-launch checklist schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_outbound_precallback_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum outbound pre-callback guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_outbound_precallback_sdk_test_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet outbound pre-callback source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_outbound_provider_validation_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum outbound provider guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_outbound_provider_validation_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet outbound provider validation source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_local_admission_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum local-admission guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_local_admission_sdk_test_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet local-admission source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_root_zero_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum receipt-root zero guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_root_zero_sdk_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet receipt-root zero source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_rlp_zero_topic_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum receipt RLP zero-topic guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_rlp_zero_topic_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet receipt RLP zero-topic source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_rlp_zero_address_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum receipt RLP zero-address guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_rlp_zero_address_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet receipt RLP zero-address source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_source_event_context_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum source-event context guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_source_event_context_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet source-event context source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_source_event_mode_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum source-event mode guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_source_event_mode_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet source-event evidence mode source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_source_event_zero_digest_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum source-event digest guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_source_event_zero_digest_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet source-event zero digest source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_rpc_duplicate_json_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum receipt duplicate-JSON guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_rpc_duplicate_json_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet receipt RPC duplicate JSON source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_receipt_block_transaction_hash_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum block receipt tx-hash guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_receipt_block_transaction_hash_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet block receipt transactionHash source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_js_receipt_admission_guard_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum JS receipt admission guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_js_receipt_admission_guard_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet JS receipt admission source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_sdk_receipt_metadata_guard_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum SDK receipt metadata guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_sdk_receipt_metadata_guard_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet SDK receipt metadata source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_native_receipt_finality_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for native receipt-proof finality guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_native_receipt_finality_guard_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet native receipt finality source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_noncanonical_chain_id_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for noncanonical Ethereum chain-id guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_noncanonical_chain_id_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet noncanonical chain id source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_beacon_rest_finalized_header_shape_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Beacon REST finalized-header guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_beacon_rest_finalized_header_shape_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet Beacon REST finalized-header shape source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_beacon_rest_execution_payload_binding_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Beacon REST execution-payload guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_beacon_rest_execution_payload_binding_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet Beacon REST execution payload binding source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_sync_committee_roster_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum sync-committee rosters."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_sync_committee_roster_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet sync-committee roster source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_unready_transparent_proof_config_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
    forbidden_paths: tuple[str | Path, ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for config-owned unready proof toggles."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_unready_transparent_proof_config_inventory_errors",
        )
        if inventory is None and forbidden_paths is None:
            return list(helper())
        return list(helper(inventory, forbidden_paths))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP unready transparent-proof config-only source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _tron_deploy_operator_boolean_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for TRON deploy operator booleans."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_tron_deploy_operator_boolean_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP TRON deploy operator boolean source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_source_bridge_config_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return SCCP Ethereum source-bridge config source inventory errors."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_source_bridge_config_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet source-bridge config source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_source_material_template_rejection_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for template-derived source material guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_source_material_template_rejection_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP source-material template rejection source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_source_material_role_validation_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for source-material role validation guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_source_material_role_validation_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP source-material role validation source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_evm_source_adapter_deployment_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum EVM source-adapter gates."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_evm_source_adapter_deployment_gate_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet EVM source-adapter deployment gate source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _contract_smoke_eth_mainnet_network_id_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return SCCP EVM contract smoke Ethereum mainnet network-id source inventory errors."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_contract_smoke_eth_mainnet_network_id_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "EVM contract smoke Ethereum mainnet network id source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _contract_smoke_evm_production_surface_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return SCCP EVM contract smoke production-surface source inventory errors."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_contract_smoke_evm_production_surface_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "EVM contract smoke production surface source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_core_range_finality_binding_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Core range/finality binding."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_core_range_finality_binding_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet SCCP range finality binding source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_core_message_replay_guard_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Core message replay guards."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_core_message_replay_guard_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet SCCP message replay guard source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_torii_pinned_message_proof_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Torii pinned message proofs."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_torii_pinned_message_proof_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet Torii pinned message proof source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_evm_source_live_production_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum live source evidence."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_evm_source_live_production_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet live EVM source production source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_evm_live_destination_production_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum live destination evidence."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_evm_live_destination_production_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet live EVM destination production source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_route_canary_finalized_receipt_block_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum route-canary finality."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_ethereum_route_canary_finalized_receipt_block_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet route-canary finalized receipt block source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _ethereum_evm_block_tag_metadata_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum EVM block-tag metadata."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_ethereum_evm_block_tag_metadata_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "Ethereum mainnet EVM block-tag metadata source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _native_sccp_no_wasm_readiness_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for native no-WASM/no-remote readiness."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(verifier, "_native_sccp_no_wasm_readiness_inventory_errors")
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "native SCCP no-WASM/no-remote readiness source inventory "
            "cannot run release-bundle verifier helper"
        ]


def _sccp_release_native_prover_bundle_schema_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for native prover bundle schema/binding."""

    try:
        verifier = _load_release_bundle_verify_helpers()
        helper = getattr(
            verifier,
            "_sccp_release_native_prover_bundle_schema_inventory_errors",
        )
        if inventory is None:
            return list(helper())
        return list(helper(inventory))
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP release native-prover bundle schema source inventory "
            "cannot run release-bundle verifier helper"
        ]


PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS: dict[str, tuple[str, ...]] = {
    "rust-sccp": ("cargo test -p iroha_sccp -- --nocapture",),
    "evidence-scripts": (
        "-m pytest -q pytests/scripts/check_sccp_production_corridor_test.py",
        "pytests/scripts/sccp_release_bundle_test.py",
        "pytests/scripts/sccp_release_readiness_report_test.py",
        "pytests/scripts/sccp_all_lanes_evidence_test.py",
        "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
        "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
        "pytests/scripts/sccp_evm_destination_evidence_test.py",
        "pytests/scripts/sccp_evm_live_evidence_test.py",
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        "pytests/scripts/sccp_evm_source_live_evidence_test.py",
        "pytests/scripts/sccp_solana_destination_evidence_test.py",
        "pytests/scripts/sccp_solana_live_evidence_test.py",
        "pytests/scripts/sccp_solana_source_state_evidence_test.py",
        "pytests/scripts/sccp_ton_destination_evidence_test.py",
        "pytests/scripts/sccp_ton_live_evidence_test.py",
        "pytests/scripts/sccp_ton_source_state_evidence_test.py",
        "pytests/scripts/sccp_tron_live_evidence_test.py",
        "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
        "pytests/scripts/sccp_retired_network_surface_test.py",
    ),
    "js-sdk": (
        "--test javascript/iroha_js/test/sccpSolanaProver.test.js",
        "javascript/iroha_js/test/sccpSolanaProver.test.js",
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
        "javascript/iroha_js/test/package_dist.test.js",
        "javascript/iroha_js/test/sccpPackageExports.test.js",
    ),
    "python-sdk": (
        "-m pytest -q python/iroha_torii_client/tests/sccp_test.py",
    ),
    "swift-sdk": (
        "swift test --filter SccpSolanaProverTests --disable-swift-testing",
        "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions",
    ),
    "kotlin-sdk": (
        "java -version",
        "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp.",
        "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
    ),
    "java-android": (
        "java -version",
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests,"
        "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests,"
        "org.hyperledger.iroha.android.sccp.TonSccpProverTests,"
        "org.hyperledger.iroha.android.sccp.TronSccpProverTests",
        "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
        "org.hyperledger.iroha.android.sccp.TonSccpProverTests",
        "org.hyperledger.iroha.android.sccp.TronSccpProverTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
    ),
    "dotnet-sdk": (
        "dotnet --version",
        "dotnet --info",
        "cargo build -p connect_norito_bridge",
        "dotnet restore Hyperledger.Iroha.Sdk.sln",
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        "FullyQualifiedName~Sccp",
        "sccp-dotnet-sdk.trx",
    ),
    "contract-smoke": (
        "scripts/sccp_bsc_groth16_material.test.mjs",
        "scripts/sccp_bsc_taira_xor_deploy.test.mjs",
        "scripts/sccp_tron_taira_xor_deploy.test.mjs",
        "scripts/sccp_taira_xor_contract.test.mjs",
        "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        "bash scripts/sccp_evm_contract_smoke.sh",
    ),
    "core-admission": (
        "cargo test -p iroha_core --test iroha_core_group_01 bridge_proofs:: -- --nocapture",
    ),
}
CONTRACT_SMOKE_NODE_SUCCESS_FRAGMENTS = (
    "fail 0",
    "materialize rejects zkeys that fail Powers-of-Tau verification",
    "materialize refuses stale transcript materialize commands without PTAU binding",
    "proof-self-test rejects witness calculators that accept adversarial assignments",
    "finalize-attestations refuses production blockers after signed request matching",
    "BSC route-config requires explicit post-deploy evidence for production-ready manifests",
    "route manifest draft binds deployment evidence, verifier material, and TAIRA burn-record contract",
    "TAIRA XOR SCCP burn-record contract compiles as IVM ZK proved artifact",
)
PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS: dict[str, tuple[str, ...]] = {
    "rust-sccp": ("test result: ok",),
    "evidence-scripts": (" passed in ",),
    "js-sdk": (
        "fail 0",
        "pass ",
        "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned",
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned",
        "package declarations expose Ethereum mainnet SCCP facade methods",
        "package declarations expose BSC mainnet Parlia finality evidence hooks",
    ),
    "python-sdk": (" passed in ",),
    "swift-sdk": ("0 failures",),
    "kotlin-sdk": ("version \"21", "BUILD SUCCESSFUL"),
    "java-android": ("version \"21", "BUILD SUCCESSFUL"),
    "dotnet-sdk": (
        DOTNET_VERSION_SUCCESS_PREFIX,
        DOTNET_WINDOWS_OS_SUCCESS_LINE,
        DOTNET_RID_SUCCESS_PREFIX,
        DOTNET_ARCHITECTURE_SUCCESS_PREFIX,
        DOTNET_BRIDGE_PATH_SUCCESS_PREFIX,
        DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX,
        DOTNET_TEST_PASSED_SUCCESS_FRAGMENT,
        DOTNET_TRX_SUCCESS_PREFIX,
        DOTNET_TRX_BYTES_SUCCESS_PREFIX,
    ),
    "contract-smoke": (
        *CONTRACT_SMOKE_NODE_SUCCESS_FRAGMENTS,
        "sccp_message_bridge_smoke: ok",
    ),
    "core-admission": ("test result: ok",),
}
PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS: dict[str, tuple[re.Pattern[str], ...]] = {
    "rust-sccp": (
        re.compile(r"\btest result:\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\btest\s+\S+\s+\.\.\.\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\bfailures:", re.IGNORECASE),
        re.compile(r"\bthread '[^']+' panicked\b", re.IGNORECASE),
        re.compile(r"\bpanicked at\b", re.IGNORECASE),
        re.compile(r"\berror(?:\[[A-Z0-9]+\])?:", re.IGNORECASE),
        re.compile(r"\bcould not compile\b", re.IGNORECASE),
        re.compile(r"\baborting due to\b", re.IGNORECASE),
    ),
    "evidence-scripts": (
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
        re.compile(r"\bTraceback \(most recent call last\):"),
        re.compile(r"\bERROR collecting\b", re.IGNORECASE),
        re.compile(r"\bFAILED\s+\S+"),
        re.compile(
            r"\b(?:AssertionError|ImportError|ModuleNotFoundError|RuntimeError|ValueError)\b"
        ),
        re.compile(r"\bInterrupted:\s+[1-9]\d*\s+errors?\b", re.IGNORECASE),
        re.compile(r"\bINTERNALERROR>", re.IGNORECASE),
    ),
    "js-sdk": (
        re.compile(r"\bfail\s+[1-9]\d*\b", re.IGNORECASE),
        re.compile(r"\bnot\s+ok\b", re.IGNORECASE),
        re.compile(r"\bERR_[A-Z0-9_]+\b"),
        re.compile(
            r"\b(?:AssertionError|TypeError|ReferenceError|SyntaxError|RangeError)\b"
        ),
        re.compile(r"\b(?:uncaughtException|unhandledRejection)\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
    ),
    "python-sdk": (
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
        re.compile(r"\bTraceback \(most recent call last\):"),
        re.compile(r"\bERROR collecting\b", re.IGNORECASE),
        re.compile(r"\bFAILED\s+\S+"),
        re.compile(
            r"\b(?:AssertionError|ImportError|ModuleNotFoundError|RuntimeError|ValueError)\b"
        ),
        re.compile(r"\bInterrupted:\s+[1-9]\d*\s+errors?\b", re.IGNORECASE),
    ),
    "swift-sdk": (
        re.compile(r"\b[1-9]\d*\s+failures?\b", re.IGNORECASE),
        re.compile(r"\berror:", re.IGNORECASE),
        re.compile(r"\bTest Case\b.*\bfailed\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+tests?\s+failed\b", re.IGNORECASE),
        re.compile(r"\bfatal error\b", re.IGNORECASE),
        re.compile(r"\bfailed\s+to\s+(?:build|compile|emit|run)\b", re.IGNORECASE),
    ),
    "kotlin-sdk": (
        re.compile(r"\bBUILD FAILED\b", re.IGNORECASE),
        re.compile(r"\bFAILURE:\s+Build failed\b", re.IGNORECASE),
        re.compile(r"\bExecution failed for task\b", re.IGNORECASE),
        re.compile(r"\bTask\s+:[^\n]*\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\bThere were failing tests\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+tests?\s+failed\b", re.IGNORECASE),
        re.compile(r"\bCompilation failed\b", re.IGNORECASE),
        re.compile(r"\bCould not (?:compile|resolve|create|determine)\b", re.IGNORECASE),
    ),
    "java-android": (
        re.compile(r"\bBUILD FAILED\b", re.IGNORECASE),
        re.compile(r"\bFAILURE:\s+Build failed\b", re.IGNORECASE),
        re.compile(r"\bExecution failed for task\b", re.IGNORECASE),
        re.compile(r"\bTask\s+:[^\n]*\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\bThere were failing tests\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+tests?\s+failed\b", re.IGNORECASE),
        re.compile(r"\bCompilation failed\b", re.IGNORECASE),
        re.compile(r"\bCould not (?:compile|resolve|create|determine)\b", re.IGNORECASE),
    ),
    "dotnet-sdk": (
        re.compile(r"\bFailed!\b", re.IGNORECASE),
        re.compile(r"\bFailed:\s*[1-9]\d*\b", re.IGNORECASE),
        re.compile(
            r"\berror\s+(?:CS|MSB|NETSDK|NU|CA|IL|BC)\d+\b", re.IGNORECASE
        ),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
        re.compile(r"\berrors?:\s*[1-9]\d*\b", re.IGNORECASE),
        re.compile(r"\bfailed\s+to\s+(?:restore|build|load|run)\b", re.IGNORECASE),
        re.compile(r"\b(?:restore|build)\s+failed\b", re.IGNORECASE),
        re.compile(r"\bthe\s+build\s+failed\b", re.IGNORECASE),
    ),
    "contract-smoke": (
        re.compile(r"\bfail\s+[1-9]\d*\b", re.IGNORECASE),
        re.compile(r"\bnot\s+ok\b", re.IGNORECASE),
        re.compile(r"\bERR_[A-Z0-9_]+\b"),
        re.compile(
            r"\b(?:AssertionError|TypeError|ReferenceError|SyntaxError|RangeError)\b"
        ),
        re.compile(r"\b(?:uncaughtException|unhandledRejection)\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
        re.compile(r"\b(?:ParserError|DeclarationError|CompilerError):"),
        re.compile(r"\bError:"),
        re.compile(r"\bnpm ERR!", re.IGNORECASE),
        re.compile(r"\bcommand not found\b", re.IGNORECASE),
        re.compile(r"\bNo such file or directory\b", re.IGNORECASE),
    ),
    "core-admission": (
        re.compile(r"\btest result:\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\btest\s+\S+\s+\.\.\.\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\bfailures:", re.IGNORECASE),
        re.compile(r"\bthread '[^']+' panicked\b", re.IGNORECASE),
        re.compile(r"\bpanicked at\b", re.IGNORECASE),
        re.compile(r"\berror(?:\[[A-Z0-9]+\])?:", re.IGNORECASE),
        re.compile(r"\bcould not compile\b", re.IGNORECASE),
        re.compile(r"\baborting due to\b", re.IGNORECASE),
    ),
}
PYTEST_OPTIONS_WITH_VALUES = frozenset(
    (
        "-k",
        "-m",
        "-o",
        "--basetemp",
        "--confcutdir",
        "--deselect",
        "--ignore",
        "--ignore-glob",
        "--junitxml",
        "--log-file",
        "--rootdir",
    )
)
NODE_TEST_OPTIONS_WITH_VALUES = frozenset(
    (
        "--conditions",
        "--experimental-loader",
        "--import",
        "--loader",
        "--require",
        "--test-name-pattern",
        "--test-reporter",
        "--test-reporter-destination",
        "--test-shard",
    )
)
ANSI_ESCAPE_PATTERN = re.compile(
    r"\x1b(?:"
    r"\[[0-?]*[ -/]*[@-~]"
    r"|\][^\x07]*(?:\x07|\x1b\\)"
    r"|[PX^_][^\x1b]*(?:\x1b\\)"
    r"|[@-Z\\-_]"
    r")"
)
ASCII_CONTROL_CHARACTER_PATTERN = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")
SUCCESS_OUTPUT_NEGATION_PATTERN = re.compile(
    r"\b(?:not|missing|without|no|never|failed|expected|omitted)\b",
    re.IGNORECASE,
)
SUCCESS_OUTPUT_DIAGNOSTIC_PREFIX_PATTERN = re.compile(
    r"\b(?:contains?|diagnostic|found|line|marker|output|reported|saw|seen|success|text)\b",
    re.IGNORECASE,
)
SHELL_XTRACE_COMMAND_PATTERN = re.compile(r"^\s*\++\s+")
EVM_JS_USER_PROVER_HELPERS = (
    "buildEvmSccpProofRequest",
    "canonicalEvmSccpReceiptProofBytes",
    "evmSccpReceiptProofHash",
    "canonicalBscSccpReceiptProofBytes",
    "bscSccpReceiptProofHash",
    "buildBscMainnetSccpDestinationProofRequest",
    "wrapBscMainnetSccpDestinationProofResult",
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.runNativeProverSelfTest",
    "EthereumMainnetSccp.buildOutboundProofRequest",
    "EthereumMainnetSccp.proveOutboundToEthereum",
    "EthereumMainnetSccp.buildEthereumCalldata",
    "EthereumMainnetSccp.submitOutboundToEthereum",
    "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
    "EthereumMainnetSccp.proveInboundToSora",
    "EthereumMainnetSccp.submitInboundToIroha",
    "EthereumMainnetSccp.buildLocalAdmissionSubmission",
    "buildEthereumMainnetSccpLocalAdmissionSubmission",
    "runEthereumMainnetNativeProverSelfTest",
    "consensusProvider",
    "BscMainnetSccpProver",
    "BscMainnetSccp",
    "BscMainnetSccp.collectInboundEvidenceFromReceipt",
    "BscMainnetSccp.proveInboundToSora",
    "BscMainnetSccp.submitInboundToIroha",
    "BscMainnetSccp.buildLocalAdmissionSubmission",
    "BscMainnetSccp.buildBscCalldata",
    "BscMainnetSccp.submitOutboundToBsc",
    "buildBscMainnetSccpDestinationSubmission",
    "buildBscMainnetSccpLocalAdmissionSubmission",
    "EvmSccpProver",
    "witnessProvider",
    "proveFn",
    "buildEvmSccpSubmission",
    "buildEvmSccpBridgeProofSubmitPayload",
)
EVM_PYTHON_USER_PROVER_HELPERS = (
    "build_evm_sccp_proof_request",
    "canonical_evm_sccp_receipt_proof_bytes",
    "evm_sccp_receipt_proof_hash",
    "canonical_bsc_sccp_receipt_proof_bytes",
    "bsc_sccp_receipt_proof_hash",
    "build_bsc_mainnet_sccp_destination_proof_request",
    "wrap_bsc_mainnet_sccp_destination_proof_result",
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.build_outbound_proof_request",
    "EthereumMainnetSccp.prove_outbound_to_ethereum",
    "EthereumMainnetSccp.build_ethereum_calldata",
    "EthereumMainnetSccp.submit_outbound_to_ethereum",
    "EthereumMainnetSccp.collect_inbound_evidence_from_receipt",
    "EthereumMainnetSccp.prove_inbound_to_sora",
    "EthereumMainnetSccp.submit_inbound_to_iroha",
    "EthereumMainnetSccp.build_local_admission_submission",
    "build_ethereum_mainnet_sccp_local_admission_submission",
    "consensus_provider",
    "BscMainnetSccpProver",
    "BscMainnetSccp",
    "BscMainnetSccp.collect_inbound_evidence_from_receipt",
    "BscMainnetSccp.prove_inbound_to_sora",
    "BscMainnetSccp.submit_inbound_to_iroha",
    "BscMainnetSccp.build_local_admission_submission",
    "BscMainnetSccp.build_bsc_calldata",
    "BscMainnetSccp.submit_outbound_to_bsc",
    "build_bsc_mainnet_sccp_destination_submission",
    "build_bsc_mainnet_sccp_local_admission_submission",
    "EvmSccpProver",
    "witness_provider",
    "prove",
    "build_evm_sccp_submission",
    "build_evm_sccp_bridge_proof_submit_payload",
)
EVM_SWIFT_USER_PROVER_HELPERS = (
    "buildEvmSccpProofRequest",
    "canonicalEvmSccpReceiptProofBytes",
    "evmSccpReceiptProofHash",
    "canonicalBscSccpReceiptProofBytes",
    "bscSccpReceiptProofHash",
    "buildBscMainnetSccpDestinationProofRequest",
    "wrapBscMainnetSccpDestinationProofResult",
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.runNativeProverSelfTest",
    "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
    "EthereumMainnetSccp.proveInboundToSora",
    "EthereumMainnetSccp.submitInboundToIroha",
    "EthereumMainnetSccp.buildLocalAdmissionSubmission",
    "buildEthereumMainnetSccpLocalAdmissionSubmission",
    "EthereumMainnetSccp.buildOutboundProofRequest",
    "EthereumMainnetSccp.proveOutboundToEthereum",
    "EthereumMainnetSccp.buildEthereumCalldata",
    "EthereumMainnetSccp.submitOutboundToEthereum",
    "EthereumMainnetSccp.OutboundSubmitFunction",
    "EthereumMainnetConsensusProvider",
    "EthereumMainnetBeaconFinalityEvidence",
    "EthereumMainnetReceiptProof",
    "EthereumMainnetInboundEvidence.init(beaconFinalityEvidence:)",
    "BscMainnetSccpProver",
    "BscMainnetSccp",
    "BscMainnetSccp.collectInboundEvidenceFromReceipt",
    "BscMainnetSccp.proveInboundToSora",
    "BscMainnetSccp.submitInboundToIroha",
    "BscMainnetSccp.buildLocalAdmissionSubmission",
    "BscMainnetSccp.buildBscCalldata",
    "BscMainnetSccp.submitOutboundToBsc",
    "BscMainnetSccp.OutboundSubmitFunction",
    "BscMainnetConsensusProvider",
    "BscMainnetParliaFinalityEvidence",
    "BscMainnetInboundEvidence.init(parliaFinalityEvidence:)",
    "buildBscMainnetSccpDestinationSubmission",
    "buildBscMainnetSccpLocalAdmissionSubmission",
    "EvmSccpProver",
    "EvmSccpWitnessProvider",
    "EvmSccpProver.ProveFunction",
    "buildEvmSccpSubmission",
    "ToriiBridgeProofSubmitRequest.init(evmSccpSubmission:)",
)
EVM_KOTLIN_USER_PROVER_HELPERS = (
    "SccpEvm.buildProofRequest",
    "SccpSourceProofs.canonicalEvmReceiptProofBytes",
    "SccpSourceProofs.evmReceiptProofHash",
    "SccpSourceProofs.canonicalBscReceiptProofBytes",
    "SccpSourceProofs.bscReceiptProofHash",
    "SccpBsc.buildProofRequest",
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.runNativeProverSelfTest",
    "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
    "EthereumMainnetSccp.proveInboundToSora",
    "EthereumMainnetSccp.submitInboundToIroha",
    "EthereumMainnetSccp.buildOutboundProofRequest",
    "EthereumMainnetSccp.proveOutboundToEthereum",
    "EthereumMainnetSccp.buildEthereumCalldata",
    "EthereumMainnetSccp.submitOutboundToEthereum",
    "EthereumMainnetConsensusProvider",
    "EthereumMainnetBeaconFinalityEvidence",
    "EthereumMainnetReceiptProof",
    "EthereumMainnetInboundEvidence.withBeaconFinalityEvidence",
    "EthereumMainnetOutboundSubmitter",
    "SccpEthereumMainnet.buildLocalAdmissionSubmission",
    "EthereumMainnetLocalAdmissionSubmissionInput",
    "BscSccpProver",
    "BscMainnetSccp",
    "BscMainnetSccp.collectInboundEvidenceFromReceipt",
    "BscMainnetSccp.proveInboundToSora",
    "BscMainnetSccp.submitInboundToIroha",
    "BscMainnetSccp.buildLocalAdmissionSubmission",
    "BscMainnetSccp.buildBscCalldata",
    "BscMainnetSccp.submitOutboundToBsc",
    "BscMainnetConsensusProvider",
    "BscMainnetParliaFinalityEvidence",
    "BscMainnetInboundEvidence.withParliaFinalityEvidence",
    "BscMainnetOutboundSubmitter",
    "SccpBsc.buildSubmission",
    "SccpBsc.buildLocalAdmissionSubmission",
    "BscMainnetLocalAdmissionSubmissionInput",
    "EvmSccpProver",
    "EvmSccpWitnessProvider",
    "EvmSccpProofEngine",
    "SccpEvm.buildSubmission",
    "BridgeProofSubmitRequest.fromEvmSccpSubmission",
)
EVM_JAVA_ANDROID_USER_PROVER_HELPERS = (
    "EvmSccpProver.buildProofRequest",
    "SourceSccpProofs.canonicalEvmReceiptProofBytes",
    "SourceSccpProofs.evmReceiptProofHash",
    "SourceSccpProofs.canonicalBscReceiptProofBytes",
    "SourceSccpProofs.bscReceiptProofHash",
    "BscSccpProver.buildProofRequest",
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.runNativeProverSelfTest",
    "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
    "EthereumMainnetSccp.proveInboundToSora",
    "EthereumMainnetSccp.submitInboundToIroha",
    "EthereumMainnetSccp.buildLocalAdmissionSubmission",
    "EthereumMainnetSccp.buildLocalAdmission",
    "EthereumMainnetSccp.buildOutboundProofRequest",
    "EthereumMainnetSccp.proveOutboundToEthereum",
    "EthereumMainnetSccp.buildEthereumCalldata",
    "EthereumMainnetSccp.submitOutboundToEthereum",
    "EthereumMainnetSccp.ConsensusProvider",
    "EthereumMainnetSccp.BeaconFinalityEvidence",
    "EthereumMainnetSccp.ReceiptProof",
    "InboundEvidence.withBeaconFinalityEvidence",
    "EthereumMainnetSccp.OutboundSubmitter",
    "EthereumMainnetSccp.LocalAdmissionSubmissionInput",
    "BscSccpProver",
    "BscMainnetSccp",
    "BscMainnetSccp.collectInboundEvidenceFromReceipt",
    "BscMainnetSccp.proveInboundToSora",
    "BscMainnetSccp.submitInboundToIroha",
    "BscMainnetSccp.buildLocalAdmissionSubmission",
    "BscMainnetSccp.buildLocalAdmission",
    "BscMainnetSccp.buildBscCalldata",
    "BscMainnetSccp.submitOutboundToBsc",
    "BscMainnetSccp.ConsensusProvider",
    "BscMainnetSccp.ParliaFinalityEvidence",
    "InboundEvidence.withParliaFinalityEvidence",
    "BscMainnetSccp.OutboundSubmitter",
    "BscSccpProver.buildSubmission",
    "BscMainnetSccp.LocalAdmissionSubmissionInput",
    "EvmSccpProver",
    "EvmSccpProver.WitnessProvider",
    "EvmSccpProver.ProofEngine",
    "EvmSccpProver.buildSubmission",
    "BridgeProofSubmitRequest.fromEvmSccpSubmission",
)
EVM_DOTNET_USER_PROVER_HELPERS = (
    "EthereumMainnetSccp",
    "EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync",
    "EthereumMainnetSccp.ProveInboundToSoraAsync",
    "EthereumMainnetSccp.SubmitInboundToIrohaAsync",
    "EthereumMainnetSccp.RunNativeProverSelfTestAsync",
    "EthereumMainnetSccp.BuildOutboundProofRequest",
    "EthereumMainnetSccp.ProveOutboundToEthereumAsync",
    "EthereumMainnetSccp.BuildEthereumCalldata",
    "EthereumMainnetSccp.SubmitOutboundToEthereumAsync",
    "EthereumMainnetSccp.BuildLocalAdmissionSubmission",
    "EthereumMainnetSccp.DestinationBinding",
    "EthereumMainnetSccp.DestinationBindingHash",
    "IEthereumMainnetExecutionProvider",
    "IEthereumMainnetConsensusProvider",
    "EthereumMainnetBeaconFinalityEvidence",
    "EthereumMainnetReceiptProof",
    "EthereumMainnetTransparentPublicInputs",
    "EthereumMainnetOutboundProofRequestInput",
    "EthereumMainnetOutboundProofRequest",
    "EthereumMainnetOutboundProofResult",
    "EthereumMainnetSccpSubmission",
    "EthereumMainnetLocalAdmissionSubmissionInput",
    "EthereumMainnetInboundEvidence.WithBeaconFinalityEvidence",
    "IEthereumMainnetInboundProver",
    "IEthereumMainnetInboundSubmitter",
    "IEthereumMainnetOutboundProver",
    "IEthereumMainnetOutboundSubmitter",
    "BscMainnetSccp",
    "BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync",
    "BscMainnetSccp.ProveInboundToSoraAsync",
    "BscMainnetSccp.SubmitInboundToIrohaAsync",
    "BscMainnetSccp.BuildLocalAdmissionSubmission",
    "BscMainnetSccp.BuildOutboundProofRequest",
    "BscMainnetSccp.ProveOutboundToBscAsync",
    "BscMainnetSccp.BuildBscCalldata",
    "BscMainnetSccp.SubmitOutboundToBscAsync",
    "BscMainnetSccp.DestinationBinding",
    "BscMainnetSccp.DestinationBindingHash",
    "IBscMainnetExecutionProvider",
    "IBscMainnetConsensusProvider",
    "BscMainnetParliaFinalityEvidence",
    "BscMainnetTransparentPublicInputs",
    "BscMainnetOutboundProofRequestInput",
    "BscMainnetOutboundProofRequest",
    "BscMainnetOutboundProofResult",
    "BscMainnetSccpSubmission",
    "BscMainnetLocalAdmissionSubmissionInput",
    "BscMainnetInboundEvidence.WithParliaFinalityEvidence",
    "IBscMainnetInboundProver",
    "IBscMainnetInboundSubmitter",
    "IBscMainnetOutboundProver",
    "IBscMainnetOutboundSubmitter",
)
TRON_JS_USER_PROVER_HELPERS = (
    "buildTronSccpProofRequest",
    "canonicalTronSccpReceiptProofBytes",
    "canonicalTronSccpReceiptStateProofBytes",
    "canonicalTronSccpTransactionSourceProofBytes",
    "tronSccpTransactionSourceProofHash",
    "TronSccpProver",
    "witnessProvider",
    "proveFn",
    "buildTronSccpSubmission",
    "buildTronSccpBridgeProofSubmitPayload",
)
TRON_PYTHON_USER_PROVER_HELPERS = (
    "build_tron_sccp_proof_request",
    "canonical_tron_sccp_receipt_proof_bytes",
    "canonical_tron_sccp_receipt_state_proof_bytes",
    "canonical_tron_sccp_transaction_source_proof_bytes",
    "tron_sccp_transaction_source_proof_hash",
    "TronSccpProver",
    "witness_provider",
    "prove",
    "build_tron_sccp_submission",
    "build_tron_sccp_bridge_proof_submit_payload",
)
TRON_SWIFT_USER_PROVER_HELPERS = (
    "buildTronSccpProofRequest",
    "canonicalTronSccpReceiptProofBytes",
    "canonicalTronSccpReceiptStateProofBytes",
    "canonicalTronSccpTransactionSourceProofBytes",
    "tronSccpTransactionSourceProofHash",
    "TronSccpProver",
    "TronSccpWitnessProvider",
    "TronSccpProver.ProveFunction",
    "buildTronSccpSubmission",
    "ToriiBridgeProofSubmitRequest.init(tronSccpSubmission:)",
)
TRON_KOTLIN_USER_PROVER_HELPERS = (
    "SccpTron.buildProofRequest",
    "SccpSourceProofs.canonicalTronReceiptProofBytes",
    "SccpSourceProofs.canonicalTronReceiptStateProofBytes",
    "SccpSourceProofs.canonicalTronTransactionSourceProofBytes",
    "SccpSourceProofs.tronTransactionSourceProofHash",
    "TronSccpProver",
    "TronSccpWitnessProvider",
    "TronSccpProofEngine",
    "SccpTron.buildSubmission",
    "BridgeProofSubmitRequest.fromTronSccpSubmission",
)
TRON_JAVA_ANDROID_USER_PROVER_HELPERS = (
    "TronSccpProver.buildProofRequest",
    "SourceSccpProofs.canonicalTronReceiptProofBytes",
    "SourceSccpProofs.canonicalTronReceiptStateProofBytes",
    "SourceSccpProofs.canonicalTronTransactionSourceProofBytes",
    "SourceSccpProofs.tronTransactionSourceProofHash",
    "TronSccpProver",
    "TronSccpProver.WitnessProvider",
    "TronSccpProver.ProofEngine",
    "TronSccpProver.buildSubmission",
    "BridgeProofSubmitRequest.fromTronSccpSubmission",
)
SOLANA_JS_USER_PROVER_HELPERS = (
    "buildSolanaSccpProofRequest",
    "buildSolanaSccpAccountsLtHashProofRequest",
    "buildSolanaSccpTowerReplayProofRequest",
    "buildSolanaSccpFullAccountsdbLatticeProofRequest",
    "buildSolanaSccpBankForkChoiceProofRequest",
    "buildSolanaSccpFullLightClientAuditProofRequests",
    "SolanaSccpSourceStateProver",
    "SolanaSccpProver",
    "witnessProvider",
    "proveFn",
    "buildSolanaSccpSubmission",
)
SOLANA_PYTHON_USER_PROVER_HELPERS = (
    "build_solana_sccp_proof_request",
    "build_solana_sccp_accounts_lt_hash_proof_request",
    "build_solana_sccp_tower_replay_proof_request",
    "build_solana_sccp_full_accountsdb_lattice_proof_request",
    "build_solana_sccp_bank_fork_choice_proof_request",
    "build_solana_sccp_full_light_client_audit_proof_requests",
    "SolanaSccpSourceStateProver",
    "SolanaSccpProver",
    "witness_provider",
    "prove",
    "build_solana_sccp_submission",
)
SOLANA_SWIFT_USER_PROVER_HELPERS = (
    "buildSolanaSccpProofRequest",
    "buildSolanaSccpAccountsLtHashProofRequest",
    "buildSolanaSccpTowerReplayProofRequest",
    "buildSolanaSccpFullAccountsdbLatticeProofRequest",
    "buildSolanaSccpBankForkChoiceProofRequest",
    "buildSolanaSccpFullLightClientAuditProofRequests",
    "SolanaSccpSourceStateProver",
    "SolanaSccpProver",
    "SolanaSccpWitnessProvider",
    "SolanaSccpProver.ProveFunction",
    "SolanaSccpSourceStateProver.AccountsLtHashProveFunction",
    "SolanaSccpSourceStateProver.FullLightClientAuditProveFunction",
    "buildSolanaSccpSubmission",
)
SOLANA_KOTLIN_USER_PROVER_HELPERS = (
    "SccpSolana.buildProofRequest",
    "SccpSolana.buildAccountsLtHashProofRequest",
    "SccpSolana.buildTowerReplayProofRequest",
    "SccpSolana.buildFullAccountsdbLatticeProofRequest",
    "SccpSolana.buildBankForkChoiceProofRequest",
    "SccpSolana.buildFullLightClientAuditProofRequests",
    "SolanaSccpSourceStateProver",
    "SolanaSccpProver",
    "SolanaSccpWitnessProvider",
    "SolanaSccpProofEngine",
    "SolanaSccpAccountsLtHashProofEngine",
    "SolanaSccpFullLightClientAuditProofEngine",
    "SccpSolana.buildSubmission",
)
SOLANA_JAVA_ANDROID_USER_PROVER_HELPERS = (
    "SolanaSccpProver.buildProofRequest",
    "SolanaSccpProver.buildAccountsLtHashProofRequest",
    "SolanaSccpProver.buildTowerReplayProofRequest",
    "SolanaSccpProver.buildFullAccountsdbLatticeProofRequest",
    "SolanaSccpProver.buildBankForkChoiceProofRequest",
    "SolanaSccpProver.buildFullLightClientAuditProofRequests",
    "SolanaSccpProver.SourceStateProver",
    "SolanaSccpProver",
    "SolanaSccpProver.WitnessProvider",
    "SolanaSccpProver.ProofEngine",
    "SolanaSccpProver.AccountsLtHashProofEngine",
    "SolanaSccpProver.FullLightClientAuditProofEngine",
    "SolanaSccpProver.buildSubmission",
)
TON_JS_USER_PROVER_HELPERS = (
    "buildTonSccpProofRequest",
    "buildTonShardStateProofRequest",
    "buildTonSccpMasterchainConfigProofRequest",
    "buildTonSccpValidatorSetTransitionProofRequest",
    "buildTonSccpShardAccountsDictionaryProofRequest",
    "buildTonSccpFullLightClientAuditProofRequests",
    "TonSccpSourceStateProver",
    "TonSccpProver",
    "witnessProvider",
    "proveFn",
    "buildTonSccpSubmission",
)
TON_PYTHON_USER_PROVER_HELPERS = (
    "build_ton_sccp_proof_request",
    "build_ton_shard_state_proof_request",
    "build_ton_sccp_masterchain_config_proof_request",
    "build_ton_sccp_validator_set_transition_proof_request",
    "build_ton_sccp_shard_accounts_dictionary_proof_request",
    "build_ton_sccp_full_light_client_audit_proof_requests",
    "TonSccpSourceStateProver",
    "TonSccpProver",
    "witness_provider",
    "prove",
    "build_ton_sccp_submission",
)
TON_SWIFT_USER_PROVER_HELPERS = (
    "buildTonSccpProofRequest",
    "buildTonShardStateProofRequest",
    "buildTonSccpMasterchainConfigProofRequest",
    "buildTonSccpValidatorSetTransitionProofRequest",
    "buildTonSccpShardAccountsDictionaryProofRequest",
    "buildTonSccpFullLightClientAuditProofRequests",
    "TonSccpSourceStateProver",
    "TonSccpProver",
    "TonSccpWitnessProvider",
    "TonSccpProver.ProveFunction",
    "TonSccpSourceStateProver.ShardStateProveFunction",
    "TonSccpSourceStateProver.FullLightClientAuditProveFunction",
    "buildTonSccpSubmission",
)
TON_KOTLIN_USER_PROVER_HELPERS = (
    "SccpTon.buildProofRequest",
    "SccpTon.buildShardStateProofRequest",
    "SccpTon.buildMasterchainConfigProofRequest",
    "SccpTon.buildValidatorSetTransitionProofRequest",
    "SccpTon.buildShardAccountsDictionaryProofRequest",
    "SccpTon.buildFullLightClientAuditProofRequests",
    "TonSccpSourceStateProver",
    "TonSccpProver",
    "TonSccpWitnessProvider",
    "TonSccpProofEngine",
    "TonSccpShardStateProofEngine",
    "TonSccpFullLightClientAuditProofEngine",
    "SccpTon.buildSubmission",
)
TON_JAVA_ANDROID_USER_PROVER_HELPERS = (
    "TonSccpProver.buildProofRequest",
    "TonSccpProver.buildShardStateProofRequest",
    "TonSccpProver.buildMasterchainConfigProofRequest",
    "TonSccpProver.buildValidatorSetTransitionProofRequest",
    "TonSccpProver.buildShardAccountsDictionaryProofRequest",
    "TonSccpProver.buildFullLightClientAuditProofRequests",
    "TonSccpProver.SourceStateProver",
    "TonSccpProver",
    "TonSccpProver.WitnessProvider",
    "TonSccpProver.ProofEngine",
    "TonSccpProver.ShardStateProofEngine",
    "TonSccpProver.FullLightClientAuditProofEngine",
    "TonSccpProver.buildSubmission",
)
def _sdk_helper_sets(
    js: tuple[str, ...],
    python: tuple[str, ...],
    swift: tuple[str, ...],
    kotlin: tuple[str, ...],
    java_android: tuple[str, ...],
    dotnet: tuple[str, ...] | None = None,
) -> dict[str, tuple[str, ...]]:
    helpers = {
        "js-sdk": js,
        "python-sdk": python,
        "swift-sdk": swift,
        "kotlin-sdk": kotlin,
        "java-android": java_android,
    }
    if dotnet is not None:
        helpers[EVM_NATIVE_DOTNET_PHASE] = dotnet
    return helpers


def _helper_text(helpers: tuple[str, ...]) -> str:
    return ", ".join(helpers)


USER_PROVER_SUBMISSION_SURFACES: tuple[dict[str, Any], ...] = (
    {
        "lanes": "eth,bsc",
        "proof_backend": "evm-groth16-bn254-v1",
        "sdk_helper_symbols": EVM_JS_USER_PROVER_HELPERS,
        "sdk_helper_symbols_by_sdk": _sdk_helper_sets(
            EVM_JS_USER_PROVER_HELPERS,
            EVM_PYTHON_USER_PROVER_HELPERS,
            EVM_SWIFT_USER_PROVER_HELPERS,
            EVM_KOTLIN_USER_PROVER_HELPERS,
            EVM_JAVA_ANDROID_USER_PROVER_HELPERS,
            EVM_DOTNET_USER_PROVER_HELPERS,
        ),
        "sdk_helpers": _helper_text(EVM_JS_USER_PROVER_HELPERS),
        "on_chain_submission": (
            "Torii bridge-proof submit payload with BN254 Groth16 "
            "proof_bytes_hex for the EVM verifier contract"
        ),
        "required_phases": (
            *USER_PROVER_SDK_PHASES,
            EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        ),
    },
    {
        "lanes": "tron",
        "proof_backend": "tron-groth16-bn254-v1",
        "sdk_helper_symbols": TRON_JS_USER_PROVER_HELPERS,
        "sdk_helper_symbols_by_sdk": _sdk_helper_sets(
            TRON_JS_USER_PROVER_HELPERS,
            TRON_PYTHON_USER_PROVER_HELPERS,
            TRON_SWIFT_USER_PROVER_HELPERS,
            TRON_KOTLIN_USER_PROVER_HELPERS,
            TRON_JAVA_ANDROID_USER_PROVER_HELPERS,
        ),
        "sdk_helpers": _helper_text(TRON_JS_USER_PROVER_HELPERS),
        "on_chain_submission": (
            "Torii bridge-proof submit payload with BN254 Groth16 "
            "proof_bytes_hex for the TRON verifier contract"
        ),
        "required_phases": (
            *USER_PROVER_SDK_PHASES,
            "contract-smoke",
            "core-admission",
        ),
    },
    {
        "lanes": "sol",
        "proof_backend": "sccp-solana-recursive-mainnet-v1",
        "sdk_helper_symbols": SOLANA_JS_USER_PROVER_HELPERS,
        "sdk_helper_symbols_by_sdk": _sdk_helper_sets(
            SOLANA_JS_USER_PROVER_HELPERS,
            SOLANA_PYTHON_USER_PROVER_HELPERS,
            SOLANA_SWIFT_USER_PROVER_HELPERS,
            SOLANA_KOTLIN_USER_PROVER_HELPERS,
            SOLANA_JAVA_ANDROID_USER_PROVER_HELPERS,
        ),
        "sdk_helpers": _helper_text(SOLANA_JS_USER_PROVER_HELPERS),
        "on_chain_submission": "Solana verifier-program instruction envelope",
        "required_phases": USER_PROVER_CHAIN_PHASES,
    },
    {
        "lanes": "ton",
        "proof_backend": "ton-contract-v1",
        "sdk_helper_symbols": TON_JS_USER_PROVER_HELPERS,
        "sdk_helper_symbols_by_sdk": _sdk_helper_sets(
            TON_JS_USER_PROVER_HELPERS,
            TON_PYTHON_USER_PROVER_HELPERS,
            TON_SWIFT_USER_PROVER_HELPERS,
            TON_KOTLIN_USER_PROVER_HELPERS,
            TON_JAVA_ANDROID_USER_PROVER_HELPERS,
        ),
        "sdk_helpers": _helper_text(TON_JS_USER_PROVER_HELPERS),
        "on_chain_submission": "TON internal message body BOC",
        "required_phases": USER_PROVER_CHAIN_PHASES,
    },
)
USER_PROVER_REQUIRED_LANE_BACKENDS = {
    surface["lanes"]: surface["proof_backend"]
    for surface in USER_PROVER_SUBMISSION_SURFACES
}
USER_PROVER_ON_CHAIN_SUBMISSION_BY_LANE = {
    surface["lanes"]: surface["on_chain_submission"]
    for surface in USER_PROVER_SUBMISSION_SURFACES
}
USER_PROVER_REQUIRED_PHASES_BY_LANE = {
    surface["lanes"]: tuple(surface["required_phases"])
    for surface in USER_PROVER_SUBMISSION_SURFACES
}
USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK = {
    surface["lanes"]: {
        sdk: tuple(symbols)
        for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items()
    }
    for surface in USER_PROVER_SUBMISSION_SURFACES
}


def _load_all_lanes_module() -> Any:
    spec = importlib.util.spec_from_file_location(
        "_sccp_all_lanes_evidence",
        ALL_LANES_SCRIPT,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {ALL_LANES_SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _corridor_phases() -> list[str]:
    completed = subprocess.run(
        ["bash", str(CORRIDOR_SCRIPT), "--list"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    phases = [
        line.strip()
        for line in completed.stdout.splitlines()
        if line.startswith("  ")
    ]
    if not phases:
        raise RuntimeError("SCCP production corridor runner did not list phases")
    return phases


def _normalize_phase_status(value: str) -> str:
    # Source-inventory marker: phase result status contains surrounding whitespace
    if not value:
        raise argparse.ArgumentTypeError("phase result status is empty")
    if _path_control_character(value) is not None:
        raise argparse.ArgumentTypeError(
            "phase result status contains control character"
        )
    if not value.isascii():
        raise argparse.ArgumentTypeError(
            "phase result status contains non-ASCII character"
        )
    if value.strip() != value:
        raise argparse.ArgumentTypeError(
            "phase result status contains surrounding whitespace"
        )
    if any(character.isspace() for character in value):
        raise argparse.ArgumentTypeError("phase result status contains whitespace")
    normalized = value.lower()
    if normalized in {"pass", "passed", "ok", "success", "successful", "green"}:
        return "passed"
    if normalized in {"fail", "failed", "failure", "red"}:
        return "failed"
    if normalized in {"skip", "skipped"}:
        return "skipped"
    if normalized in {"missing", "unknown", "pending", "not-run", "not_run"}:
        return "missing"
    raise argparse.ArgumentTypeError(
        "phase result status must be passed, failed, skipped, or missing"
    )


def _parse_phase_assignment_name(raw_name: str, label: str) -> str:
    # Source-inventory markers:
    # - phase result name contains surrounding whitespace
    # - phase result name contains Markdown-unsafe character
    # - phase result name contains sensitive name
    # - phase result name contains malformed phase
    # - phase evidence name contains surrounding whitespace
    # - phase evidence name contains Markdown-unsafe character
    # - phase evidence name contains sensitive name
    # - phase evidence name contains malformed phase
    if not raw_name:
        raise argparse.ArgumentTypeError(f"{label} name is empty")
    if _path_control_character(raw_name) is not None:
        raise argparse.ArgumentTypeError(f"{label} name contains control character")
    if not raw_name.isascii():
        raise argparse.ArgumentTypeError(f"{label} name contains non-ASCII character")
    if raw_name.strip() != raw_name:
        raise argparse.ArgumentTypeError(
            f"{label} name contains surrounding whitespace"
        )
    if any(character.isspace() for character in raw_name):
        raise argparse.ArgumentTypeError(f"{label} name contains whitespace")
    if _path_markdown_unsafe_character(raw_name) is not None:
        raise argparse.ArgumentTypeError(
            f"{label} name contains Markdown-unsafe character"
        )
    if any(
        marker in raw_name.lower()
        for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
    ):
        raise argparse.ArgumentTypeError(f"{label} name contains sensitive name")
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in raw_name)
        or raw_name.startswith("-")
        or raw_name.endswith("-")
    ):
        raise argparse.ArgumentTypeError(f"{label} name contains malformed phase")
    return raw_name


def _phase_evidence_path_error(path_text: str) -> str | None:
    # Source-inventory markers:
    # - phase evidence path must not contain surrounding whitespace
    # - phase evidence path contains control character
    # - phase evidence path contains non-ASCII character
    # - phase evidence path contains Markdown-unsafe character
    # - phase evidence path contains sensitive name
    # - phase evidence path contains percent-encoded traversal segment
    if path_text.strip() != path_text:
        return "phase evidence path must not contain surrounding whitespace"
    if _path_control_character(path_text) is not None:
        return "phase evidence path contains control character"
    if not path_text.isascii():
        return "phase evidence path contains non-ASCII character"
    if _path_markdown_unsafe_character(path_text) is not None:
        return "phase evidence path contains Markdown-unsafe character"
    if any(
        marker in path_text.lower()
        for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
    ):
        return "phase evidence path contains sensitive name"
    if _path_percent_encoded_traversal(path_text) is not None:
        return "phase evidence path contains percent-encoded traversal segment"
    return None


def _phase_evidence_directory_path_error(path_text: str) -> str | None:
    # Source-inventory markers:
    # - phase evidence directory path must not contain surrounding whitespace
    # - phase evidence directory path contains control character
    # - phase evidence directory path contains non-ASCII character
    # - phase evidence directory path contains Markdown-unsafe character
    # - phase evidence directory path contains sensitive name
    # - phase evidence directory path contains percent-encoded traversal segment
    if path_text.strip() != path_text:
        return "phase evidence directory path must not contain surrounding whitespace"
    if _path_control_character(path_text) is not None:
        return "phase evidence directory path contains control character"
    if not path_text.isascii():
        return "phase evidence directory path contains non-ASCII character"
    if _path_markdown_unsafe_character(path_text) is not None:
        return "phase evidence directory path contains Markdown-unsafe character"
    if any(
        marker in path_text.lower()
        for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
    ):
        return "phase evidence directory path contains sensitive name"
    if _path_percent_encoded_traversal(path_text) is not None:
        return "phase evidence directory path contains percent-encoded traversal segment"
    return None


def _readiness_output_path_error(path_text: str) -> str | None:
    # Source-inventory markers:
    # - readiness report output path must not contain surrounding whitespace
    # - readiness report output path contains control character
    # - readiness report output path contains non-ASCII character
    # - readiness report output path contains Markdown-unsafe character
    # - readiness report output path contains sensitive name
    # - readiness report output path contains percent-encoded traversal segment
    if path_text.strip() != path_text:
        return "readiness report output path must not contain surrounding whitespace"
    if _path_control_character(path_text) is not None:
        return "readiness report output path contains control character"
    if not path_text.isascii():
        return "readiness report output path contains non-ASCII character"
    if _path_markdown_unsafe_character(path_text) is not None:
        return "readiness report output path contains Markdown-unsafe character"
    if any(
        marker in path_text.lower()
        for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
    ):
        return "readiness report output path contains sensitive name"
    if _path_percent_encoded_traversal(path_text) is not None:
        return "readiness report output path contains percent-encoded traversal segment"
    return None


def _parse_phase_results(values: list[str], phases: list[str]) -> dict[str, str]:
    results = {phase: "missing" for phase in phases}
    for raw in values:
        if "=" not in raw:
            raise argparse.ArgumentTypeError(
                "phase result must use NAME=STATUS syntax"
            )
        name, status = raw.split("=", 1)
        name = _parse_phase_assignment_name(name, "phase result")
        normalized = _normalize_phase_status(status)
        if name == "all":
            results = {phase: normalized for phase in phases}
            continue
        if name not in results:
            raise argparse.ArgumentTypeError("unknown SCCP corridor phase")
        results[name] = normalized
    return results


def _phase_evidence_source_label(name: str) -> str:
    return f"--phase-evidence {name}=<path>"


def _path_control_character(path: str) -> str | None:
    for character in path:
        if ord(character) < 0x20 or ord(character) == 0x7F:
            return repr(character)
    return None


MARKDOWN_UNSAFE_PATH_CHARACTERS = frozenset("|`<>")


def _path_markdown_unsafe_character(path: str) -> str | None:
    for character in path:
        if character in MARKDOWN_UNSAFE_PATH_CHARACTERS:
            return repr(character)
    return None


def _path_percent_encoded_traversal(path: str) -> str | None:
    decoded = path
    seen = {decoded}
    for _ in range(32):
        if "%" not in decoded:
            return None
        decoded = unquote(decoded)
        if decoded in seen:
            return None
        seen.add(decoded)
        decoded_path = PurePosixPath(decoded)
        if (
            decoded_path.is_absolute()
            or ".." in decoded_path.parts
            or "\\" in decoded
            or decoded != decoded_path.as_posix()
        ):
            return repr(path)
    if "%" in decoded:
        return repr(path)
    return None


def _artifact(path: Path) -> dict[str, Any]:
    if path.is_symlink():
        raise ValueError("release artifact path must not be a symlink")
    artifact_path = str(path)
    if artifact_path.strip() != artifact_path:
        raise ValueError("release artifact path must not contain surrounding whitespace")
    control_character = _path_control_character(artifact_path)
    if control_character is not None:
        raise ValueError(
            "release artifact path contains control character "
            f"{control_character}"
        )
    if not artifact_path.isascii():
        raise ValueError("release artifact path contains non-ASCII character")
    markdown_unsafe_character = _path_markdown_unsafe_character(artifact_path)
    if markdown_unsafe_character is not None:
        raise ValueError(
            "release artifact path contains Markdown-unsafe character "
            f"{markdown_unsafe_character}"
        )
    percent_traversal = _path_percent_encoded_traversal(artifact_path)
    if percent_traversal is not None:
        raise ValueError(
            "release artifact path contains percent-encoded traversal segment"
        )
    if any(
        marker in path.name.lower()
        for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
    ):
        raise ValueError("release artifact path contains sensitive name")
    payload = path.read_bytes()
    return {
        "path": artifact_path,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def _is_nonzero_hex32(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) != 66:
        return False
    try:
        raw = bytes.fromhex(value[2:])
    except (SystemExit, RuntimeError, TypeError, ValueError):
        return False
    return len(raw) == 32 and any(raw) and value == f"0x{raw.hex()}"


def _source_adapter_gate_template_hashes(domain: Any) -> tuple[bytes, ...]:
    if type(domain) is not int:
        return ()
    all_lanes = _load_all_lanes_module()
    profile = all_lanes.LANE_PROFILES.get(domain)
    if profile is None:
        return ()
    return tuple(all_lanes._source_material_template_hashes(profile).values())


def _public_cryptographic_source_adapter_gate_template_hash_errors(
    row_label: str,
    domain: Any,
    gate_hash: Any,
    audit_hashes: dict[str, Any],
) -> list[str]:
    """Return public-row blockers when source-gate hashes replay templates."""

    if type(domain) is not int:
        return []
    template_hashes = _source_adapter_gate_template_hashes(domain)
    if not template_hashes:
        return []
    errors: list[str] = []
    if _is_nonzero_hex32(gate_hash):
        assert isinstance(gate_hash, str)
        if bytes.fromhex(gate_hash[2:]) in template_hashes:
            errors.append(
                f"{row_label} source_adapter_gate_hash must be deployed gate "
                "evidence, not built-in template material"
            )
    expected_audit_keys = ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.get(
        domain,
        frozenset(),
    )
    for audit_field in sorted(expected_audit_keys):
        audit_hash = audit_hashes.get(audit_field)
        if not _is_nonzero_hex32(audit_hash):
            continue
        assert isinstance(audit_hash, str)
        if bytes.fromhex(audit_hash[2:]) in template_hashes:
            errors.append(
                f"{row_label} source_adapter_gate_audit_hashes {audit_field} "
                "must be deployed audit evidence, not built-in template material"
            )
    return errors


def _is_canonical_sha256_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(symbol in "0123456789abcdef" for symbol in value)
    )


def _is_nonzero_canonical_sha256_text(value: Any) -> bool:
    return _is_canonical_sha256_text(value) and any(symbol != "0" for symbol in value)


def _sha256_text_errors(label: str, value: Any) -> list[str]:
    if not _is_canonical_sha256_text(value):
        return [f"{label} sha256 must be a canonical SHA-256 hex string"]
    if not _is_nonzero_canonical_sha256_text(value):
        return [f"{label} sha256 must be a non-zero canonical SHA-256 hex string"]
    return []


def _native_evm_prover_artifact_metadata_blockers(
    artifact: Any,
    label: str,
) -> list[str]:
    prefix = f"native EVM Groth16 prover bundle {label}"
    if not isinstance(artifact, dict):
        return [f"{prefix} artifact metadata must be an object"]

    blockers: list[str] = []
    artifact_path = artifact.get("path")
    if (
        not isinstance(artifact_path, str)
        or not artifact_path
        or artifact_path.strip() != artifact_path
        or _path_control_character(artifact_path) is not None
        or _path_markdown_unsafe_character(artifact_path) is not None
        or _path_percent_encoded_traversal(artifact_path) is not None
    ):
        blockers.append(f"{prefix} artifact path metadata is invalid")

    artifact_bytes = artifact.get("bytes")
    if type(artifact_bytes) is not int or artifact_bytes < 0:
        blockers.append(f"{prefix} artifact bytes metadata is invalid")

    if not _is_nonzero_canonical_sha256_text(artifact.get("sha256")):
        blockers.append(f"{prefix} artifact sha256 metadata is invalid")

    return blockers


def _is_hex32(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) != 66:
        return False
    try:
        raw = bytes.fromhex(value[2:])
    except (SystemExit, RuntimeError, TypeError, ValueError):
        return False
    return len(raw) == 32 and value == f"0x{raw.hex()}"


def _native_evm_manifest_relative_path(
    value: Any,
    label: str,
) -> tuple[PurePosixPath | None, list[str]]:
    prefix = f"native EVM Groth16 prover bundle {label}"
    if not isinstance(value, str) or not value:
        return None, [
            f"{prefix} path must be a non-empty relative POSIX file path"
        ]
    if value.strip() != value:
        return None, [f"{prefix} path must not contain surrounding whitespace"]
    control_character = _path_control_character(value)
    if control_character is not None:
        return None, [
            f"{prefix} path contains control character {control_character}"
        ]
    if not value.isascii():
        return None, [f"{prefix} path contains non-ASCII character"]
    markdown_unsafe_character = _path_markdown_unsafe_character(value)
    if markdown_unsafe_character is not None:
        return None, [
            f"{prefix} path contains Markdown-unsafe character"
        ]
    percent_traversal = _path_percent_encoded_traversal(value)
    if percent_traversal is not None:
        return None, [
            f"{prefix} path contains percent-encoded traversal segment"
        ]
    if ":" in value:
        return None, [
            f"{prefix} path must not contain URI schemes or drive prefixes"
        ]
    if "\\" in value:
        return None, [f"{prefix} path must use POSIX separators"]
    normalized_value = value.lower()
    for marker in NATIVE_EVM_PROVER_FORBIDDEN_PATH_MARKERS:
        if marker in normalized_value:
            return None, [
                f"{prefix} path contains forbidden prover dependency marker: {marker}"
            ]
    if any(marker in normalized_value for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return None, [f"{prefix} path contains sensitive name"]
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or ".." in path.parts
        or not path.parts
        or value != path.as_posix()
    ):
        return None, [
            f"{prefix} path must be relative and stay under the manifest directory"
        ]
    return path, []


def _native_evm_prover_forbidden_payload_blockers(
    artifact_path: Path,
    label: str,
) -> list[str]:
    prefix = f"native EVM Groth16 prover bundle {label}"
    try:
        payload = artifact_path.read_bytes().lower()
    except OSError:
        return [
            f"{prefix} cannot be scanned for forbidden prover dependency markers"
        ]

    return [
        f"{prefix} contains forbidden prover dependency marker: "
        f"{marker.decode('ascii')}"
        for marker in NATIVE_EVM_PROVER_FORBIDDEN_PAYLOAD_MARKERS
        if marker in payload
    ]


def _native_evm_prover_payload_artifact(
    manifest_path: Path | None,
    payload: dict[str, Any],
    path_field: str,
    hash_field: str,
    label: str,
    min_bytes: int = NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES,
) -> tuple[dict[str, Any] | None, list[str]]:
    if path_field not in payload:
        return None, []
    relative_path, blockers = _native_evm_manifest_relative_path(
        payload.get(path_field),
        label,
    )
    if manifest_path is None or relative_path is None:
        return None, blockers

    artifact_path = manifest_path.parent.joinpath(*relative_path.parts)
    prefix = f"native EVM Groth16 prover bundle {label}"
    try:
        if not artifact_path.is_file():
            blockers.append(
                f"{prefix} file is missing or is not a regular file"
            )
            return None, blockers
        artifact = _artifact(artifact_path)
    except OSError:
        blockers.append(f"{prefix} cannot be read")
        return None, blockers
    except ValueError:
        blockers.append(f"{prefix} artifact path metadata is invalid")
        return None, blockers

    metadata_blockers = _native_evm_prover_artifact_metadata_blockers(
        artifact,
        label,
    )
    if metadata_blockers:
        blockers.extend(metadata_blockers)
        return None, blockers

    if artifact["bytes"] == 0:
        blockers.append(f"{prefix} must not be empty")
    elif artifact["bytes"] < min_bytes:
        blockers.append(f"{prefix} must be at least {min_bytes} bytes")

    expected_hash = payload.get(hash_field)
    actual_hash = f"0x{artifact['sha256']}"
    if isinstance(expected_hash, str) and actual_hash != expected_hash:
        blockers.append(f"{prefix} sha256 must match {hash_field}")
    blockers.extend(
        _native_evm_prover_forbidden_payload_blockers(artifact_path, label)
    )
    return artifact, blockers


def _native_evm_prover_bundle_artifact_summary(
    artifacts: Any,
    proof_artifact_hash: Any,
    proving_key_hash: Any,
    manifest_path: Path | None,
) -> tuple[list[dict[str, Any]], list[str]]:
    blockers: list[str] = []
    if not isinstance(artifacts, list) or not artifacts:
        return [], ["native_sdk_artifacts must be a non-empty list"]

    rows: list[dict[str, Any]] = []
    by_sdk: dict[str, dict[str, Any]] = {}
    semantic_sdk_order: list[str] = []
    for index, artifact in enumerate(artifacts):
        label = f"native_sdk_artifacts[{index}]"
        if not isinstance(artifact, dict):
            blockers.append(f"{label} must be an object")
            continue
        for key in sorted(set(artifact) - NATIVE_EVM_PROVER_SDK_ARTIFACT_KEYS):
            blockers.append(
                _native_evm_prover_field_name_blocker(label, key, "unknown")
            )
        for key in sorted(NATIVE_EVM_PROVER_SDK_ARTIFACT_KEYS - set(artifact)):
            blockers.append(f"{label} missing field: {key}")
        sdk = artifact.get("sdk")
        implementation = artifact.get("implementation")
        sdk_key_blocker = _native_evm_prover_sdk_artifact_key_blocker(label, sdk)
        if sdk_key_blocker is not None:
            blockers.append(sdk_key_blocker)
            continue
        semantic_sdk_order.append(sdk)
        if sdk in by_sdk:
            if _native_evm_sdk_name_has_sensitive_marker(sdk):
                blockers.append(
                    "native_sdk_artifacts contains duplicate sdk with sensitive name"
                )
            else:
                blockers.append(f"native_sdk_artifacts contains duplicate sdk: {sdk}")
        expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS.get(sdk)
        if expected_implementation is None:
            if _native_evm_sdk_name_has_sensitive_marker(sdk):
                blockers.append(
                    "native_sdk_artifacts contains unknown sdk with sensitive name"
                )
                continue
            blockers.append(f"native_sdk_artifacts contains unknown sdk: {sdk}")
        elif implementation != expected_implementation:
            blockers.append(
                f"{sdk} implementation must be {expected_implementation}"
            )
        if artifact.get("prover_artifact_hash") != proof_artifact_hash:
            blockers.append(f"{sdk} prover_artifact_hash must match proof_artifact_hash")
        if artifact.get("proving_key_hash") != proving_key_hash:
            blockers.append(f"{sdk} proving_key_hash must match proving_key_hash")
        if not _is_nonzero_hex32(artifact.get("implementation_hash")):
            blockers.append(
                f"{sdk} implementation_hash must be a canonical non-zero 32-byte hex value"
            )
        implementation_artifact, artifact_blockers = (
            _native_evm_prover_payload_artifact(
                manifest_path,
                artifact,
                "implementation_artifact",
                "implementation_hash",
                f"{sdk} implementation_artifact",
                NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES,
            )
        )
        blockers.extend(artifact_blockers)
        row = {
            "sdk": sdk,
            "implementation": implementation,
            "implementation_hash": artifact.get("implementation_hash", ""),
            "implementation_artifact": implementation_artifact,
        }
        rows.append(row)
        by_sdk[sdk] = row

    for sdk in sorted(set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS) - set(by_sdk)):
        blockers.append(f"native_sdk_artifacts missing sdk: {sdk}")
    if semantic_sdk_order != sorted(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS):
        blockers.append("native_sdk_artifacts must match expected SDK order")

    return sorted(rows, key=lambda row: row["sdk"]), blockers


def _native_evm_prover_sdk_artifact_key_blocker(
    label: str,
    sdk: Any,
) -> str | None:
    if not isinstance(sdk, str) or not sdk:
        return f"{label}.sdk must be a non-empty string"
    if _path_control_character(sdk) is not None:
        return f"{label}.sdk contains control character"
    if not sdk.isascii():
        return f"{label}.sdk must be printable ASCII"
    if sdk.strip() != sdk:
        return f"{label}.sdk must not contain surrounding whitespace"
    if any(character.isspace() for character in sdk):
        return f"{label}.sdk must not contain whitespace"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in sdk)
        or sdk.startswith("-")
        or sdk.endswith("-")
    ):
        return f"{label}.sdk must be a lowercase SDK id"
    return None


def _native_evm_prover_sdk_results_by_sdk(
    prefix: str,
    sdk_results: Any,
) -> tuple[dict[str, Any], list[str]]:
    if not isinstance(sdk_results, dict) or not sdk_results:
        return {}, [f"{prefix} sdk_results must be a non-empty object"]

    blockers: list[str] = []
    canonical_results: dict[str, Any] = {}
    for sdk, result in sorted(sdk_results.items()):
        sdk_key_blocker = _native_evm_prover_sdk_result_key_blocker(prefix, sdk)
        if sdk_key_blocker is not None:
            blockers.append(sdk_key_blocker)
            continue
        canonical_results[sdk] = result

    for sdk in sorted(
        set(canonical_results) - set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS)
    ):
        if _native_evm_sdk_name_has_sensitive_marker(sdk):
            blockers.append(
                f"{prefix} sdk_results contains unknown sdk with sensitive name"
            )
        else:
            blockers.append(f"{prefix} sdk_results contains unknown sdk: {sdk}")
    for sdk in sorted(
        set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS) - set(canonical_results)
    ):
        blockers.append(f"{prefix} sdk_results missing sdk: {sdk}")
    return canonical_results, blockers


def _native_evm_prover_sdk_result_key_blocker(
    prefix: str,
    sdk: Any,
) -> str | None:
    if not isinstance(sdk, str) or not sdk:
        return f"{prefix} sdk_results contains malformed sdk key"
    if _path_control_character(sdk) is not None:
        return f"{prefix} sdk_results sdk key contains control character"
    if not sdk.isascii():
        return f"{prefix} sdk_results sdk key must be printable ASCII"
    if sdk.strip() != sdk:
        return f"{prefix} sdk_results sdk key must not contain surrounding whitespace"
    if any(character.isspace() for character in sdk):
        return f"{prefix} sdk_results sdk key must not contain whitespace"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in sdk)
        or sdk.startswith("-")
        or sdk.endswith("-")
    ):
        return f"{prefix} sdk_results sdk key must be a lowercase SDK id"
    return None


def _native_evm_prover_field_name_blocker(
    label: str,
    key: Any,
    field_kind: str,
) -> str:
    if not isinstance(key, str) or not key:
        return f"{label} contains malformed {field_kind} field name"
    if _path_control_character(key) is not None:
        return f"{label} contains {field_kind} field name with control character"
    if not key.isascii():
        return f"{label} contains {field_kind} field name with non-ASCII character"
    if key.strip() != key:
        return f"{label} contains {field_kind} field name with surrounding whitespace"
    if any(character.isspace() for character in key):
        return f"{label} contains {field_kind} field name with whitespace"
    if _path_markdown_unsafe_character(key) is not None:
        return (
            f"{label} contains {field_kind} field name with Markdown-unsafe "
            "character"
        )
    if any(marker in key.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return f"{label} contains {field_kind} field name with sensitive name"
    return f"{label} contains {field_kind} field: {key}"


SENSITIVE_PUBLIC_FIELD_NAME_MARKERS = (
    "secret-token",
    "secret key",
    "secret-key",
    "secret_key",
    "private key",
    "private-key",
    "private_key",
    "password",
    "bearer",
    "authorization",
    "access key",
    "access-key",
    "access_key",
    "api key",
    "api-key",
    "api_key",
    "client secret",
    "client-secret",
    "client_secret",
    "credential",
    "credentials",
    "auth header",
    "auth-header",
    "auth_header",
    "mnemonic",
    "seed phrase",
    "seed-phrase",
    "seed_phrase",
    "signing key",
    "signing-key",
    "signing_key",
    "token",
)


def _native_evm_sdk_name_has_sensitive_marker(sdk: str) -> bool:
    return any(marker in sdk.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS)


def _native_evm_sdk_name_blocker(label: str, sdk: str, issue: str) -> str:
    if _native_evm_sdk_name_has_sensitive_marker(sdk):
        return f"{label} contains {issue} sdk with sensitive name"
    return f"{label} contains {issue} sdk: {sdk}"


def _required_record_summary_unknown_field_blocker(
    lane_label: str,
    key: Any,
) -> str:
    return _native_evm_prover_field_name_blocker(
        f"{lane_label}: required record summary",
        key,
        "unknown",
    )


def _native_evm_prover_duplicate_json_key_blocker(label: str, key: Any) -> str:
    if not isinstance(key, str) or not key:
        return f"{label} JSON contains malformed duplicate key"
    if _path_control_character(key) is not None:
        return f"{label} JSON contains duplicate key with control character"
    if not key.isascii():
        return f"{label} JSON contains duplicate key with non-ASCII character"
    if key.strip() != key:
        return f"{label} JSON contains duplicate key with surrounding whitespace"
    if any(character.isspace() for character in key):
        return f"{label} JSON contains duplicate key with whitespace"
    if _path_markdown_unsafe_character(key) is not None:
        return f"{label} JSON contains duplicate key with Markdown-unsafe character"
    if any(marker in key.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return f"{label} JSON contains duplicate key with sensitive key name"
    return f"{label} JSON contains duplicate key: {key}"


def _native_evm_prover_parity_fixture_status(
    manifest_path: Path | None,
    payload: dict[str, Any],
) -> tuple[dict[str, Any] | None, list[str]]:
    label = "cross_sdk_fixture_parity_artifact"
    prefix = f"native EVM Groth16 prover bundle {label}"
    relative_path, blockers = _native_evm_manifest_relative_path(
        payload.get(label),
        label,
    )
    if manifest_path is None or relative_path is None:
        return None, blockers

    artifact_path = manifest_path.parent.joinpath(*relative_path.parts)
    try:
        if not artifact_path.is_file():
            blockers.append(
                f"{prefix} file is missing or is not a regular file"
            )
            return None, blockers
        artifact = _artifact(artifact_path)
    except OSError:
        blockers.append(f"{prefix} cannot be read")
        return None, blockers
    except ValueError:
        blockers.append(f"{prefix} artifact path metadata is invalid")
        return None, blockers

    metadata_blockers = _native_evm_prover_artifact_metadata_blockers(
        artifact,
        label,
    )
    if metadata_blockers:
        blockers.extend(metadata_blockers)
        return None, blockers

    if artifact["bytes"] == 0:
        blockers.append(f"{prefix} must not be empty")
    elif artifact["bytes"] < NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES:
        blockers.append(
            f"{prefix} must be at least "
            f"{NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES} bytes"
        )

    audit_hashes = payload.get("audit_hashes")
    expected_hash = (
        audit_hashes.get("cross_sdk_fixture_parity")
        if isinstance(audit_hashes, dict)
        else None
    )
    actual_hash = f"0x{artifact['sha256']}"
    if isinstance(expected_hash, str) and actual_hash != expected_hash:
        blockers.append(
            f"{prefix} sha256 must match audit_hashes.cross_sdk_fixture_parity"
        )

    try:
        fixture = _load_json_without_duplicate_keys(artifact_path)
    except DuplicateJsonKeyError as exc:
        blockers.append(_native_evm_prover_duplicate_json_key_blocker(prefix, exc.key))
        fixture = {}
    except json.JSONDecodeError:
        blockers.append(f"{prefix} is not valid JSON")
        fixture = {}
    except UnicodeDecodeError:
        blockers.append(f"{prefix} is not UTF-8 text")
        fixture = {}
    except OSError:
        blockers.append(f"{prefix} cannot be read as JSON")
        fixture = {}

    if not isinstance(fixture, dict):
        blockers.append(f"{prefix} must be a JSON object")
        fixture = {}

    for key in sorted(set(fixture) - NATIVE_EVM_PROVER_PARITY_FIXTURE_REQUIRED_KEYS):
        blockers.append(
            _native_evm_prover_field_name_blocker(prefix, key, "unknown")
        )
    for key in sorted(NATIVE_EVM_PROVER_PARITY_FIXTURE_REQUIRED_KEYS - set(fixture)):
        blockers.append(f"{prefix} missing field: {key}")

    expected_fields = {
        "schema": NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA,
        "domain": ACTIVE_LAUNCH_DOMAIN,
        "chain": ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "proof_artifact_hash": payload.get("proof_artifact_hash"),
        "proving_key_hash": payload.get("proving_key_hash"),
        "verifier_key_hash": payload.get("verifier_key_hash"),
        "destination_binding_hash": payload.get("destination_binding_hash"),
    }
    for key, expected in expected_fields.items():
        if key in fixture and fixture.get(key) != expected:
            blockers.append(f"{prefix} {key} must match native prover bundle")

    for key in (
        "receipt_proof_hash",
        "source_proof_hash",
        "calldata_hash",
        "torii_submit_payload_hash",
    ):
        if key in fixture and not _is_nonzero_hex32(fixture.get(key)):
            blockers.append(
                f"{prefix} {key} must be a canonical non-zero 32-byte hex value"
            )
    blockers.extend(
        _native_evm_prover_fixture_hash_role_blockers(
            prefix,
            fixture,
            NATIVE_EVM_PROVER_PARITY_HASH_ROLE_KEYS,
        )
    )

    public_signal_words = fixture.get("public_signal_words")
    if not isinstance(public_signal_words, list) or len(public_signal_words) != 9:
        blockers.append(f"{prefix} public_signal_words must contain 9 words")
        public_signal_words = []
    else:
        for index, word in enumerate(public_signal_words):
            if not _is_hex32(word):
                blockers.append(
                    f"{prefix} public_signal_words[{index}] must be a canonical 32-byte hex value"
                )

    sdk_results = fixture.get("sdk_results")
    sdk_results, sdk_result_blockers = _native_evm_prover_sdk_results_by_sdk(
        prefix,
        sdk_results,
    )
    blockers.extend(sdk_result_blockers)
    for sdk, result in sorted(sdk_results.items()):
        result_label = f"{label} sdk_results.{sdk}"
        if not isinstance(result, dict):
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label} must be an object"
            )
            continue
        for key in sorted(set(result) - NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS):
            blockers.append(
                _native_evm_prover_field_name_blocker(
                    f"native EVM Groth16 prover bundle {result_label}",
                    key,
                    "unknown",
                )
            )
        for key in sorted(NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS - set(result)):
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label} missing field: {key}"
            )
        for key in (
            "receipt_proof_hash",
            "source_proof_hash",
            "destination_binding_hash",
            "calldata_hash",
            "torii_submit_payload_hash",
        ):
            if key in result and result.get(key) != fixture.get(key):
                blockers.append(
                    f"native EVM Groth16 prover bundle {result_label}.{key} must match {key}"
                )
        if result.get("public_signal_words") != public_signal_words:
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label}.public_signal_words "
                "must match public_signal_words"
            )

    blockers.extend(
        _native_evm_prover_forbidden_payload_blockers(artifact_path, label)
    )
    return artifact, blockers


def _native_evm_prover_self_test_status(
    manifest_path: Path | None,
    payload: dict[str, Any],
) -> tuple[dict[str, Any] | None, list[str]]:
    label = "native_prover_self_test_artifact"
    prefix = f"native EVM Groth16 prover bundle {label}"
    relative_path, blockers = _native_evm_manifest_relative_path(
        payload.get(label),
        label,
    )
    if manifest_path is None or relative_path is None:
        return None, blockers

    artifact_path = manifest_path.parent.joinpath(*relative_path.parts)
    try:
        if not artifact_path.is_file():
            blockers.append(
                f"{prefix} file is missing or is not a regular file"
            )
            return None, blockers
        artifact = _artifact(artifact_path)
    except OSError:
        blockers.append(f"{prefix} cannot be read")
        return None, blockers
    except ValueError:
        blockers.append(f"{prefix} artifact path metadata is invalid")
        return None, blockers

    metadata_blockers = _native_evm_prover_artifact_metadata_blockers(
        artifact,
        label,
    )
    if metadata_blockers:
        blockers.extend(metadata_blockers)
        return None, blockers

    if artifact["bytes"] == 0:
        blockers.append(f"{prefix} must not be empty")
    elif artifact["bytes"] < NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES:
        blockers.append(
            f"{prefix} must be at least "
            f"{NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES} bytes"
        )

    audit_hashes = payload.get("audit_hashes")
    expected_hash = (
        audit_hashes.get("native_prover_self_test")
        if isinstance(audit_hashes, dict)
        else None
    )
    actual_hash = f"0x{artifact['sha256']}"
    if isinstance(expected_hash, str) and actual_hash != expected_hash:
        blockers.append(
            f"{prefix} sha256 must match audit_hashes.native_prover_self_test"
        )

    try:
        fixture = _load_json_without_duplicate_keys(artifact_path)
    except DuplicateJsonKeyError as exc:
        blockers.append(_native_evm_prover_duplicate_json_key_blocker(prefix, exc.key))
        fixture = {}
    except json.JSONDecodeError:
        blockers.append(f"{prefix} is not valid JSON")
        fixture = {}
    except UnicodeDecodeError:
        blockers.append(f"{prefix} is not UTF-8 text")
        fixture = {}
    except OSError:
        blockers.append(f"{prefix} cannot be read as JSON")
        fixture = {}

    if not isinstance(fixture, dict):
        blockers.append(f"{prefix} must be a JSON object")
        fixture = {}

    for key in sorted(set(fixture) - NATIVE_EVM_PROVER_SELF_TEST_REQUIRED_KEYS):
        blockers.append(
            _native_evm_prover_field_name_blocker(prefix, key, "unknown")
        )
    for key in sorted(NATIVE_EVM_PROVER_SELF_TEST_REQUIRED_KEYS - set(fixture)):
        blockers.append(f"{prefix} missing field: {key}")

    expected_fields = {
        "schema": NATIVE_EVM_PROVER_SELF_TEST_SCHEMA,
        "domain": ACTIVE_LAUNCH_DOMAIN,
        "chain": ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "proof_artifact_hash": payload.get("proof_artifact_hash"),
        "proving_key_hash": payload.get("proving_key_hash"),
        "verifier_key_hash": payload.get("verifier_key_hash"),
        "destination_binding_hash": payload.get("destination_binding_hash"),
    }
    for key, expected in expected_fields.items():
        if key in fixture and fixture.get(key) != expected:
            blockers.append(f"{prefix} {key} must match native prover bundle")

    for key in (
        "request_hash",
        "witness_hash",
        "source_proof_hash",
        "proof_hash",
        "calldata_hash",
        "torii_submit_payload_hash",
    ):
        if key in fixture and not _is_nonzero_hex32(fixture.get(key)):
            blockers.append(
                f"{prefix} {key} must be a canonical non-zero 32-byte hex value"
            )
    blockers.extend(
        _native_evm_prover_fixture_hash_role_blockers(
            prefix,
            fixture,
            NATIVE_EVM_PROVER_SELF_TEST_HASH_ROLE_KEYS,
        )
    )

    public_signal_words = fixture.get("public_signal_words")
    if not isinstance(public_signal_words, list) or len(public_signal_words) != 9:
        blockers.append(f"{prefix} public_signal_words must contain 9 words")
        public_signal_words = []
    else:
        for index, word in enumerate(public_signal_words):
            if not _is_hex32(word):
                blockers.append(
                    f"{prefix} public_signal_words[{index}] must be a canonical 32-byte hex value"
                )

    sdk_results = fixture.get("sdk_results")
    sdk_results, sdk_result_blockers = _native_evm_prover_sdk_results_by_sdk(
        prefix,
        sdk_results,
    )
    blockers.extend(sdk_result_blockers)
    for sdk, result in sorted(sdk_results.items()):
        result_label = f"{label} sdk_results.{sdk}"
        if not isinstance(result, dict):
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label} must be an object"
            )
            continue
        for key in sorted(set(result) - NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS):
            blockers.append(
                _native_evm_prover_field_name_blocker(
                    f"native EVM Groth16 prover bundle {result_label}",
                    key,
                    "unknown",
                )
            )
        for key in sorted(NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS - set(result)):
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label} missing field: {key}"
            )
        for key in (
            "request_hash",
            "witness_hash",
            "source_proof_hash",
            "proof_hash",
            "calldata_hash",
            "torii_submit_payload_hash",
        ):
            if key in result and result.get(key) != fixture.get(key):
                blockers.append(
                    f"native EVM Groth16 prover bundle {result_label}.{key} must match {key}"
                )
        if result.get("public_signal_words") != public_signal_words:
            blockers.append(
                f"native EVM Groth16 prover bundle {result_label}.public_signal_words "
                "must match public_signal_words"
            )

    blockers.extend(
        _native_evm_prover_forbidden_payload_blockers(artifact_path, label)
    )
    return artifact, blockers


def _native_evm_prover_fixture_hash_role_blockers(
    prefix: str,
    fixture: dict[str, Any],
    roles: tuple[str, ...],
) -> list[str]:
    blockers: list[str] = []
    seen: dict[str, str] = {}
    for role in roles:
        value = fixture.get(role)
        if not _is_nonzero_hex32(value):
            continue
        previous_role = seen.get(value)
        if previous_role is not None:
            blockers.append(f"{prefix} {role} must not reuse {previous_role}")
            continue
        seen[value] = role
    return blockers


def _native_evm_prover_hash_role_blockers(payload: dict[str, Any]) -> list[str]:
    roles = [
        ("proof_artifact_hash", payload.get("proof_artifact_hash")),
        ("proving_key_hash", payload.get("proving_key_hash")),
        ("verifier_key_hash", payload.get("verifier_key_hash")),
        ("destination_binding_hash", payload.get("destination_binding_hash")),
    ]
    sdk_artifacts = payload.get("native_sdk_artifacts")
    if isinstance(sdk_artifacts, list):
        for index, artifact in enumerate(sdk_artifacts):
            if isinstance(artifact, dict):
                roles.append(
                    (
                        f"native_sdk_artifacts[{index}].implementation_hash",
                        artifact.get("implementation_hash"),
                    )
                )

    blockers: list[str] = []
    seen: dict[str, str] = {}
    for role, value in roles:
        if not _is_nonzero_hex32(value):
            continue
        previous_role = seen.get(value)
        if previous_role is not None:
            blockers.append(
                f"native EVM Groth16 prover bundle {role} must not reuse "
                f"{previous_role}"
            )
            continue
        seen[value] = role
    return blockers


def _native_evm_prover_path_role_blockers(payload: dict[str, Any]) -> list[str]:
    roles = [
        ("proof_artifact", payload.get("proof_artifact")),
        ("proving_key", payload.get("proving_key")),
        ("verifier_key", payload.get("verifier_key")),
        (
            "cross_sdk_fixture_parity_artifact",
            payload.get("cross_sdk_fixture_parity_artifact"),
        ),
        (
            "native_prover_self_test_artifact",
            payload.get("native_prover_self_test_artifact"),
        ),
    ]
    sdk_artifacts = payload.get("native_sdk_artifacts")
    if isinstance(sdk_artifacts, list):
        for index, artifact in enumerate(sdk_artifacts):
            if isinstance(artifact, dict):
                roles.append(
                    (
                        f"native_sdk_artifacts[{index}].implementation_artifact",
                        artifact.get("implementation_artifact"),
                    )
                )

    blockers: list[str] = []
    seen: dict[str, str] = {}
    for role, value in roles:
        relative_path, path_errors = _native_evm_manifest_relative_path(value, role)
        if path_errors or relative_path is None:
            continue
        path = relative_path.as_posix()
        previous_role = seen.get(path)
        if previous_role is not None:
            blockers.append(
                f"native EVM Groth16 prover bundle {role} path must not reuse "
                f"{previous_role}"
            )
            continue
        seen[path] = role
    return blockers


def _native_evm_prover_bundle_status(
    path: Path | None,
    evidence: dict[str, Any],
) -> dict[str, Any]:
    artifact: dict[str, Any] | None = None
    payload: Any = {}
    blockers: list[str] = []
    if path is None:
        blockers.append("native EVM Groth16 prover bundle manifest is required")
    else:
        try:
            artifact = _artifact(path)
            payload = _load_json_without_duplicate_keys(path)
        except DuplicateJsonKeyError as exc:
            blockers.append(
                _native_evm_prover_duplicate_json_key_blocker(
                    "native EVM Groth16 prover bundle",
                    exc.key,
                )
            )
        except json.JSONDecodeError:
            blockers.append("native EVM Groth16 prover bundle is not valid JSON")
        except UnicodeDecodeError:
            blockers.append("native EVM Groth16 prover bundle is not UTF-8 text")
        except OSError:
            blockers.append("native EVM Groth16 prover bundle cannot be read")
        except ValueError:
            blockers.append(
                "native EVM Groth16 prover bundle artifact path metadata is invalid"
            )

    if not isinstance(payload, dict):
        blockers.append("native EVM Groth16 prover bundle must be a JSON object")
        payload = {}

    for key in sorted(set(payload) - NATIVE_EVM_PROVER_BUNDLE_REQUIRED_KEYS):
        blockers.append(
            _native_evm_prover_field_name_blocker(
                "native EVM Groth16 prover bundle",
                key,
                "unknown",
            )
        )
    for key in sorted(NATIVE_EVM_PROVER_BUNDLE_REQUIRED_KEYS - set(payload)):
        blockers.append(f"native EVM Groth16 prover bundle missing field: {key}")

    expected_fields = {
        "schema": NATIVE_EVM_PROVER_BUNDLE_SCHEMA,
        "bundle_id": NATIVE_EVM_PROVER_BUNDLE_ID,
        "domain": ACTIVE_LAUNCH_DOMAIN,
        "chain": ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "browser_implementation": "pure-typescript",
    }
    for key, expected in expected_fields.items():
        if key in payload and payload.get(key) != expected:
            blockers.append(f"native EVM Groth16 prover bundle {key} must be {expected!r}")
    if payload.get("no_wasm") is not True:
        blockers.append("native EVM Groth16 prover bundle no_wasm must be true")
    if payload.get("remote_prover_required") is not False:
        blockers.append(
            "native EVM Groth16 prover bundle remote_prover_required must be false"
        )
    for key in (
        "proof_artifact_hash",
        "proving_key_hash",
        "verifier_key_hash",
        "destination_binding_hash",
    ):
        if key in payload and not _is_nonzero_hex32(payload.get(key)):
            blockers.append(
                f"native EVM Groth16 prover bundle {key} must be a canonical non-zero 32-byte hex value"
            )
    blockers.extend(_native_evm_prover_hash_role_blockers(payload))
    blockers.extend(_native_evm_prover_path_role_blockers(payload))

    lane = _active_launch_lane(evidence) or {}
    destination_binding = lane.get("destination_binding")
    if not isinstance(destination_binding, dict):
        destination_binding = {}
    expected_destination_binding = destination_binding.get("destination_binding_hash")
    if (
        expected_destination_binding
        and payload.get("destination_binding_hash") != expected_destination_binding
    ):
        blockers.append(
            "native EVM Groth16 prover bundle destination_binding_hash must match "
            f"{ACTIVE_LAUNCH_DISPLAY} destination binding evidence"
        )

    audit_hashes = payload.get("audit_hashes")
    if not isinstance(audit_hashes, dict) or not audit_hashes:
        blockers.append(
            "native EVM Groth16 prover bundle audit_hashes must be a non-empty object"
        )
        audit_hashes = {}
    else:
        for key in sorted(set(audit_hashes) - set(NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES)):
            blockers.append(
                _native_evm_prover_field_name_blocker(
                    "native EVM Groth16 prover bundle audit_hashes",
                    key,
                    "unexpected",
                )
            )
        for key in sorted(set(NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES) - set(audit_hashes)):
            blockers.append(
                "native EVM Groth16 prover bundle "
                f"audit_hashes missing field: {key}"
            )
        semantic_audit_hashes = {
            key: audit_hashes[key]
            for key in NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES
            if key in audit_hashes
        }
        reserved_audit_hash_roles = {
            "proof_artifact_hash": payload.get("proof_artifact_hash"),
            "proving_key_hash": payload.get("proving_key_hash"),
            "verifier_key_hash": payload.get("verifier_key_hash"),
            "destination_binding_hash": payload.get("destination_binding_hash"),
        }
        sdk_artifact_rows = payload.get("native_sdk_artifacts")
        if isinstance(sdk_artifact_rows, list):
            for sdk_index, sdk_artifact in enumerate(sdk_artifact_rows):
                if isinstance(sdk_artifact, dict):
                    reserved_audit_hash_roles[
                        f"native_sdk_artifacts[{sdk_index}].implementation_hash"
                    ] = sdk_artifact.get("implementation_hash")
        seen_audit_hashes: dict[str, str] = {}
        for key, audit_hash in sorted(semantic_audit_hashes.items()):
            if not _is_nonzero_hex32(audit_hash):
                blockers.append(
                    "native EVM Groth16 prover bundle "
                    f"audit_hashes.{key} must be a canonical non-zero 32-byte hex value"
                )
                continue
            previous_key = seen_audit_hashes.get(audit_hash)
            if previous_key is not None:
                blockers.append(
                    "native EVM Groth16 prover bundle "
                    f"audit_hashes.{key} must not duplicate "
                    f"audit_hashes.{previous_key}"
                )
            seen_audit_hashes[audit_hash] = key
            for role, role_hash in reserved_audit_hash_roles.items():
                if audit_hash == role_hash:
                    blockers.append(
                        "native EVM Groth16 prover bundle "
                        f"audit_hashes.{key} must not reuse {role}"
                    )

    proof_artifact, proof_artifact_blockers = _native_evm_prover_payload_artifact(
        path,
        payload,
        "proof_artifact",
        "proof_artifact_hash",
        "proof_artifact",
        NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES,
    )
    blockers.extend(proof_artifact_blockers)
    proving_key, proving_key_blockers = _native_evm_prover_payload_artifact(
        path,
        payload,
        "proving_key",
        "proving_key_hash",
        "proving_key",
        NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES,
    )
    blockers.extend(proving_key_blockers)
    verifier_key, verifier_key_blockers = _native_evm_prover_payload_artifact(
        path,
        payload,
        "verifier_key",
        "verifier_key_hash",
        "verifier_key",
        NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES,
    )
    blockers.extend(verifier_key_blockers)

    sdk_artifacts, sdk_blockers = _native_evm_prover_bundle_artifact_summary(
        payload.get("native_sdk_artifacts"),
        payload.get("proof_artifact_hash"),
        payload.get("proving_key_hash"),
        path,
    )
    blockers.extend(sdk_blockers)
    parity_artifact, parity_blockers = _native_evm_prover_parity_fixture_status(
        path,
        payload,
    )
    blockers.extend(parity_blockers)
    self_test_artifact, self_test_blockers = _native_evm_prover_self_test_status(
        path,
        payload,
    )
    blockers.extend(self_test_blockers)
    validation_blockers = list(dict.fromkeys(blockers))

    return {
        "required": True,
        "schema": payload.get("schema", NATIVE_EVM_PROVER_BUNDLE_SCHEMA),
        "artifact": artifact,
        "bundle_id": payload.get("bundle_id", ""),
        "lanes": ACTIVE_LAUNCH_CHAIN,
        "proof_backend": payload.get("proof_backend", "evm-groth16-bn254-v1"),
        "proof_artifact": proof_artifact,
        "proof_artifact_hash": payload.get("proof_artifact_hash", ""),
        "proving_key": proving_key,
        "proving_key_hash": payload.get("proving_key_hash", ""),
        "verifier_key": verifier_key,
        "verifier_key_hash": payload.get("verifier_key_hash", ""),
        "destination_binding_hash": payload.get("destination_binding_hash", ""),
        "audit_hashes": dict(
            sorted(
                (
                    key,
                    audit_hashes[key],
                )
                for key in NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES
                if key in audit_hashes
            )
        ),
        "cross_sdk_fixture_parity_artifact": parity_artifact,
        "native_prover_self_test_artifact": self_test_artifact,
        "sdk_artifacts": sdk_artifacts,
        "validation_status": "passed" if not validation_blockers else "blocked",
        "validation_blockers": validation_blockers,
    }


def _phase_log_from_dir(directory: Path, phase: str) -> Path:
    candidates = (
        directory / f"{phase}.log",
        directory / "dist" / "sccp-production-corridor" / f"{phase}.log",
        directory / f"sccp-production-corridor-{phase}" / f"{phase}.log",
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(
        "missing SCCP corridor evidence log for phase "
        f"{phase}; checked standard phase log layouts"
    )


def _phase_transcript_block(phase: str, transcript: str) -> str | None:
    marker = f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
    known_markers = _known_corridor_phase_marker_lines()
    lines = transcript.splitlines()
    start: int | None = None
    for index, line in enumerate(lines):
        if line == marker:
            start = index
            break
    if start is None:
        return None
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index] in known_markers:
            end = index
            break
    return "\n".join(lines[start:end])


def _known_corridor_phase_marker_lines() -> set[str]:
    return {
        f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
        for phase in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS
    }


def _unknown_corridor_phase_marker_lines(transcript: str) -> list[str]:
    known_markers = _known_corridor_phase_marker_lines()
    return [
        line
        for line in transcript.splitlines()
        if line.startswith(CORRIDOR_PHASE_MARKER_PREFIX) and line not in known_markers
    ]


def _transcript_has_multiple_known_phase_markers(transcript: str) -> bool:
    known_markers = _known_corridor_phase_marker_lines()
    markers = {
        line
        for line in transcript.splitlines()
        if line in known_markers
    }
    return len(markers) > 1


def _transcript_has_nonempty_line_before_first_phase_marker(transcript: str) -> bool:
    known_markers = _known_corridor_phase_marker_lines()
    lines = transcript.splitlines()
    for first_marker_index, line in enumerate(lines):
        if line in known_markers:
            return any(
                bool(prefix_line.strip())
                for prefix_line in lines[:first_marker_index]
            )
    return False


def _phase_marker_count(phase: str, transcript: str) -> int:
    marker = f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
    return sum(1 for line in transcript.splitlines() if line == marker)


def _transcript_has_full_corridor_completion(transcript: str) -> bool:
    lines = transcript.splitlines()
    if _transcript_has_nonempty_line_before_first_phase_marker(transcript):
        return False
    marker_positions: list[int] = []
    command_positions_by_fragment: list[list[int]] = []
    success_positions_by_fragment: list[list[int]] = []
    for phase in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS:
        marker = f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
        try:
            marker_positions.append(lines.index(marker))
        except ValueError:
            return False
        if _phase_marker_count(phase, transcript) != 1:
            return False
        phase_block = _phase_transcript_block(phase, transcript)
        if phase_block is None:
            return False
        phase_command_positions: list[int] = []
        for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:
            command_positions = _phase_block_command_fragment_line_indices(
                phase, phase_block, fragment
            )
            if not command_positions:
                return False
            phase_command_positions.extend(command_positions)
            block_offset = marker_positions[-1]
            command_positions_by_fragment.append(
                [block_offset + position for position in command_positions]
            )
        if not phase_command_positions:
            return False
        first_phase_command_position = min(phase_command_positions)
        for fragment in PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase, ()):
            success_positions = _phase_block_output_fragment_line_indices(
                phase_block, fragment
            )
            if not success_positions:
                return False
            if not _phase_success_fragment_has_position_after_required_command(
                phase,
                phase_block,
                fragment,
                success_positions,
                first_phase_command_position,
            ):
                return False
            block_offset = marker_positions[-1]
            success_positions_by_fragment.append(
                [block_offset + position for position in success_positions]
            )
        if _phase_block_forbidden_output_marker(phase, phase_block) is not None:
            return False
    if marker_positions != sorted(marker_positions):
        return False
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False
    completion_position = max(completion_positions)
    if _transcript_has_nonempty_line_after_completion(transcript):
        return False
    if _transcript_has_traced_command_after_completion(transcript):
        return False
    return (
        completion_position > max(marker_positions)
        and all(
            any(position < completion_position for position in positions)
            for positions in command_positions_by_fragment
        )
        and all(
            any(position < completion_position for position in positions)
            for positions in success_positions_by_fragment
        )
    )


def _phase_command_lines(phase_block: str) -> list[str]:
    command_lines: list[str] = []
    for line in phase_block.splitlines():
        if line.lstrip().startswith("+ "):
            command_lines.append(line.strip())
            continue
        normalized_line = _phase_output_failure_scan_line(line)
        if (
            normalized_line != line
            and normalized_line.lstrip().startswith("+ ")
        ):
            command_lines.append(normalized_line.strip())
    return command_lines


def _line_is_shell_xtrace_command(line: str) -> bool:
    if SHELL_XTRACE_COMMAND_PATTERN.match(line) is not None:
        return True
    normalized_line = _phase_output_failure_scan_line(line)
    return (
        normalized_line != line
        and SHELL_XTRACE_COMMAND_PATTERN.match(normalized_line) is not None
    )


def _phase_command_tokens(command: str) -> list[str]:
    command = command.strip()
    if command.startswith("+ "):
        command = command[2:]
    try:
        raw_tokens = shlex.split(command, comments=True)
    except ValueError:
        return []
    wrapped_command = _phase_command_has_runner_cd_wrapper(raw_tokens)
    tokens: list[str] = []
    for index, token in enumerate(raw_tokens):
        normalized = token
        if wrapped_command and index == 0 and normalized.startswith("("):
            normalized = normalized[1:]
        if wrapped_command and index == len(raw_tokens) - 1 and normalized.endswith(")"):
            normalized = normalized[:-1]
        if normalized:
            tokens.append(normalized)
    return tokens


def _phase_command_has_runner_cd_wrapper(raw_tokens: list[str]) -> bool:
    return (
        len(raw_tokens) >= 4
        and raw_tokens[0] == "(cd"
        and bool(raw_tokens[1])
        and raw_tokens[2] == "&&"
        and raw_tokens[-1].endswith(")")
    )


def _phase_command_runner_cd_dir(command: str) -> str | None:
    command = command.strip()
    if command.startswith("+ "):
        command = command[2:]
    try:
        raw_tokens = shlex.split(command, comments=True)
    except ValueError:
        return None
    if not _phase_command_has_runner_cd_wrapper(raw_tokens):
        return None
    return raw_tokens[1]


def _path_basename_text(path: str) -> str:
    return PurePosixPath(path.replace("\\", "/").rstrip("/")).name


def _phase_command_expected_wrapper_basenames(
    phase: str,
    command: str,
) -> tuple[str, ...]:
    tokens = _phase_effective_command_tokens(command)
    if not tokens:
        return ()
    command_name = _command_token_basename(tokens[0])
    if phase == "swift-sdk" and command_name == "swift":
        return ("IrohaSwift",)
    if phase == "kotlin-sdk" and command_name == "gradlew":
        return ("kotlin",)
    if phase == "java-android" and command_name == "gradlew":
        return ("iroha_android",)
    if phase == EVM_NATIVE_DOTNET_PHASE and command_name == "dotnet":
        return ("csharp",)
    return ()


def _phase_command_has_unexpected_runner_cd_wrapper(
    phase: str,
    command: str,
) -> bool:
    cd_dir = _phase_command_runner_cd_dir(command)
    if cd_dir is None:
        return False
    expected_basenames = _phase_command_expected_wrapper_basenames(phase, command)
    if not expected_basenames:
        return False
    return _path_basename_text(cd_dir) not in expected_basenames


def _phase_block_has_unexpected_runner_cd_wrapper(
    phase: str,
    phase_block: str,
) -> bool:
    return any(
        _phase_command_has_unexpected_runner_cd_wrapper(phase, command)
        for command in _phase_command_lines(phase_block)
    )


def _phase_command_has_unsupported_parenthesized_group(command: str) -> bool:
    command = command.strip()
    if command.startswith("+ "):
        command = command[2:]
    try:
        raw_tokens = shlex.split(command, comments=True)
    except ValueError:
        return False
    return bool(
        raw_tokens
        and raw_tokens[0].startswith("(")
        and not _phase_command_has_runner_cd_wrapper(raw_tokens)
    )


def _phase_block_has_unsupported_parenthesized_group(phase_block: str) -> bool:
    return any(
        _phase_command_has_unsupported_parenthesized_group(command)
        for command in _phase_command_lines(phase_block)
    )


def _phase_command_is_parseable(command: str) -> bool:
    command = command.strip()
    if command.startswith("+ "):
        command = command[2:]
    try:
        shlex.split(command, comments=True)
    except ValueError:
        return False
    return True


def _phase_block_has_unparseable_command(phase_block: str) -> bool:
    return any(
        not _phase_command_is_parseable(command)
        for command in _phase_command_lines(phase_block)
    )


def _command_token_basename(token: str) -> str:
    return PurePosixPath(token).name


def _command_token_is_env_assignment(token: str) -> bool:
    name, separator, _ = token.partition("=")
    return bool(separator and name and name.replace("_", "").isalnum())


def _command_option_values(tokens: list[str], option: str) -> list[str]:
    values: list[str] = []
    index = 0
    option_prefix = f"{option}="
    while index < len(tokens):
        token = tokens[index]
        if token == option:
            if index + 1 < len(tokens):
                values.append(tokens[index + 1])
            index += 2
            continue
        if token.startswith(option_prefix):
            values.append(token[len(option_prefix) :])
        index += 1
    return values


def _command_has_option_value(tokens: list[str], option: str, expected: str) -> bool:
    return expected in _command_option_values(tokens, option)


def _command_positional_tokens(
    tokens: list[str],
    start_index: int = 0,
    options_with_values: frozenset[str] = frozenset(),
) -> list[str]:
    positionals: list[str] = []
    skip_next = False
    end_of_options = False
    for token in tokens[start_index:]:
        if skip_next:
            skip_next = False
            continue
        if not end_of_options and token == "--":
            end_of_options = True
            continue
        if not end_of_options:
            if token in options_with_values:
                skip_next = True
                continue
            if any(token.startswith(f"{option}=") for option in options_with_values):
                continue
            if token.startswith("-"):
                continue
        positionals.append(token)
    return positionals


def _phase_prefix_env_assignments(command: str) -> list[str]:
    tokens = _phase_command_tokens(command)
    if (
        "&&" in tokens
        and tokens.index("&&") == 2
        and tokens[:1] == ["cd"]
        and bool(tokens[1])
        and not tokens[1].startswith("-")
    ):
        tokens = tokens[tokens.index("&&") + 1 :]
    if tokens[:1] == ["env"]:
        tokens = tokens[1:]
    assignments: list[str] = []
    while tokens and _command_token_is_env_assignment(tokens[0]):
        assignments.append(tokens[0])
        tokens = tokens[1:]
    return assignments


def _android_harness_mains_classes(command: str) -> list[str]:
    harness_value: str | None = None
    for assignment in _phase_prefix_env_assignments(command):
        name, separator, value = assignment.partition("=")
        if separator and name == "ANDROID_HARNESS_MAINS":
            harness_value = value
    if harness_value is None:
        return []
    return [item for item in harness_value.split(",") if item]


DOTNET_PHASE_ALLOWED_ENV_ASSIGNMENTS = frozenset(
    (
        "DOTNET_ROOT",
        "DOTNET_CLI_TELEMETRY_OPTOUT",
        "DOTNET_CLI_UI_LANGUAGE",
        "PATH",
    )
)
DOTNET_PHASE_REQUIRED_ENV_ASSIGNMENTS = frozenset(
    (
        "DOTNET_ROOT",
        "DOTNET_CLI_TELEMETRY_OPTOUT",
        "DOTNET_CLI_UI_LANGUAGE",
    )
)
DOTNET_PHASE_FIXED_ENV_VALUES = {
    "DOTNET_CLI_TELEMETRY_OPTOUT": "1",
    "DOTNET_CLI_UI_LANGUAGE": "en",
}
DOTNET_PATH_LIST_EMPTY_SEGMENT_MARKERS = ("::", ";;", ":;", ";:")


def _dotnet_phase_command_dir(command_token: str) -> str | None:
    command_path = _dotnet_normalized_path_text(command_token)
    parent, separator, leaf = command_path.rpartition("/")
    if not separator or not leaf:
        return None
    return parent or "/"


def _dotnet_phase_command_uses_bridge_path(tokens: list[str]) -> bool:
    return (
        len(tokens) >= 2
        and _command_token_basename(tokens[0]) == "dotnet"
        and tokens[1] in ("restore", "test")
    )


def _dotnet_phase_path_value_matches_bridge_dir(
    value: str,
    bridge_library_dir: str | None,
) -> bool:
    if bridge_library_dir is None:
        return False
    expected = _dotnet_normalized_path_text(bridge_library_dir)
    actual = _dotnet_normalized_path_text(value)
    if not expected:
        return False
    if actual == expected:
        return True
    for separator in (":", ";"):
        prefix = f"{expected}{separator}"
        if not actual.startswith(prefix):
            continue
        remainder = actual[len(prefix) :]
        return (
            bool(remainder)
            and not remainder.startswith((":", ";"))
            and not _dotnet_path_list_has_empty_segment(remainder)
        )
    return False


def _dotnet_path_list_has_empty_segment(value: str) -> bool:
    return value.endswith((":", ";")) or any(
        marker in value for marker in DOTNET_PATH_LIST_EMPTY_SEGMENT_MARKERS
    )


def _dotnet_phase_command_has_noncanonical_env_prefix(
    command: str,
    bridge_library_dir: str | None = None,
) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if not tokens or _command_token_basename(tokens[0]) != "dotnet":
        return False
    assignments = _phase_prefix_env_assignments(command)
    if not assignments:
        return False
    seen: dict[str, str] = {}
    for assignment in assignments:
        name, separator, value = assignment.partition("=")
        if (
            not separator
            or name not in DOTNET_PHASE_ALLOWED_ENV_ASSIGNMENTS
            or name in seen
            or not value
        ):
            return True
        expected_value = DOTNET_PHASE_FIXED_ENV_VALUES.get(name)
        if expected_value is not None and value != expected_value:
            return True
        seen[name] = value
    if not DOTNET_PHASE_REQUIRED_ENV_ASSIGNMENTS <= set(seen):
        return True
    command_dir = _dotnet_phase_command_dir(tokens[0])
    if command_dir is None:
        return True
    return (
        _dotnet_normalized_path_text(seen["DOTNET_ROOT"]) != command_dir
    ) or (
        "PATH" in seen
        and (
            not _dotnet_phase_command_uses_bridge_path(tokens)
            or not _dotnet_phase_path_value_matches_bridge_dir(
                seen["PATH"],
                bridge_library_dir,
            )
        )
    )


def _dotnet_phase_block_has_noncanonical_env_prefix(phase_block: str) -> bool:
    bridge_library_dir = _dotnet_phase_bridge_library_dir(phase_block)
    return any(
        _dotnet_phase_command_has_noncanonical_env_prefix(command, bridge_library_dir)
        for command in _phase_command_lines(phase_block)
    )


def _phase_effective_command_tokens(command: str) -> list[str]:
    tokens = _phase_command_tokens(command)
    if (
        "&&" in tokens
        and tokens.index("&&") == 2
        and tokens[:1] == ["cd"]
        and bool(tokens[1])
        and not tokens[1].startswith("-")
    ):
        tokens = tokens[tokens.index("&&") + 1 :]
    if tokens[:1] == ["env"]:
        tokens = tokens[1:]
    while tokens and _command_token_is_env_assignment(tokens[0]):
        tokens = tokens[1:]
    return tokens


def _effective_command_starts_with(command: str, sequence: tuple[str, ...]) -> bool:
    tokens = _phase_effective_command_tokens(command)
    return tuple(tokens[: len(sequence)]) == sequence


def _effective_command_equals(command: str, sequence: tuple[str, ...]) -> bool:
    return tuple(_phase_effective_command_tokens(command)) == sequence


def _rust_sccp_command_has_fragment(command: str, _fragment: str) -> bool:
    return _effective_command_equals(
        command,
        ("cargo", "test", "-p", "iroha_sccp", "--", "--nocapture"),
    )


def _pytest_fragment_positionals(fragment: str) -> list[str]:
    tokens = shlex.split(fragment)
    if "-q" not in tokens:
        return []
    return _command_positional_tokens(
        tokens,
        tokens.index("-q") + 1,
        PYTEST_OPTIONS_WITH_VALUES,
    )


def _pytest_command_positionals(tokens: list[str], module_index: int) -> list[str]:
    if (
        module_index != 1
        or len(tokens) <= module_index + 3
        or tokens[module_index : module_index + 3] != ["-m", "pytest", "-q"]
    ):
        return []
    positionals = tokens[module_index + 3 :]
    if any(token.startswith("-") for token in positionals):
        return []
    return positionals


def _pytest_expected_positionals_for_phase(phase: str) -> tuple[str, ...]:
    positionals: list[str] = []
    for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ()):
        if fragment.startswith("-m pytest"):
            positionals.extend(_pytest_fragment_positionals(fragment))
        elif fragment.startswith(("pytests/", "python/")):
            positionals.append(fragment)
    return tuple(positionals)


def _append_unique(items: list[str], value: str) -> None:
    if value not in items:
        items.append(value)


def _command_token_looks_like_node_runner(token: str) -> bool:
    basename = _command_token_basename(token).lower()
    return bool(re.search(r"(^|[-_.])node(?:[0-9.]+)?($|[-_.])", basename))


def _node_expected_test_files_for_phase(phase: str) -> tuple[str, ...]:
    test_files: list[str] = []
    for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ()):
        if fragment.startswith("--test "):
            for item in shlex.split(fragment)[1:]:
                _append_unique(test_files, item)
        elif fragment.endswith((".test.js", ".test.mjs")):
            _append_unique(test_files, fragment)
    return tuple(test_files)


def _node_test_command_files(tokens: list[str]) -> tuple[str, ...]:
    if len(tokens) < 3 or not _command_token_looks_like_node_runner(tokens[0]):
        return ()
    if tokens[1] != "--test":
        return ()
    test_files = tokens[2:]
    if any(token.startswith("-") for token in test_files):
        return ()
    return tuple(test_files)


def _node_check_command_matches(tokens: list[str], expected_path: str) -> bool:
    return (
        len(tokens) == 3
        and _command_token_looks_like_node_runner(tokens[0])
        and tokens[1] == "--check"
        and tokens[2] == expected_path
    )


def _dotnet_sdk_command_matches(tokens: list[str]) -> bool:
    if len(tokens) != 8 or _command_token_basename(tokens[0]) != "dotnet":
        return False
    project_fragment = next(
        (
            fragment
            for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"]
            if fragment.startswith("dotnet test ")
        ),
        "",
    )
    filter_fragment = next(
        (
            fragment
            for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"]
            if fragment.startswith("FullyQualifiedName")
        ),
        "",
    )
    project_tokens = shlex.split(project_fragment)
    expected_project = project_tokens[2] if len(project_tokens) > 2 else ""
    return (
        tokens[1] == "test"
        and tokens[2] == expected_project
        and tokens[3] == "--filter"
        and tokens[4].replace("\\|", "|") == filter_fragment.replace("\\|", "|")
        and tokens[5] == "--nologo"
        and tokens[6] == "--logger"
        and tokens[7] == "trx;LogFileName=sccp-dotnet-sdk.trx"
    )


def _dotnet_setup_command_matches(tokens: list[str]) -> bool:
    if not tokens or _command_token_basename(tokens[0]) != "dotnet":
        return False
    return (
        (len(tokens) == 2 and tokens[1] in ("--version", "--info"))
        or (
            len(tokens) == 3
            and tokens[1] == "restore"
            and tokens[2] == "Hyperledger.Iroha.Sdk.sln"
        )
    )


def _dotnet_phase_block_forbidden_test_command(phase_block: str) -> bool:
    """Return whether the .NET phase ran a non-canonical test command."""

    for command in _phase_command_lines(phase_block):
        tokens = _phase_effective_command_tokens(command)
        if (
            len(tokens) >= 2
            and _command_token_basename(tokens[0]) == "dotnet"
            and tokens[1] == "test"
            and not _dotnet_sdk_command_matches(tokens)
        ):
            return True
    return False


def _dotnet_phase_block_forbidden_setup_command(phase_block: str) -> bool:
    """Return whether the .NET phase ran a non-canonical setup command."""

    for command in _phase_command_lines(phase_block):
        tokens = _phase_effective_command_tokens(command)
        if not tokens or _command_token_basename(tokens[0]) != "dotnet":
            continue
        if len(tokens) >= 2 and tokens[1] == "test":
            continue
        if not _dotnet_setup_command_matches(tokens):
            return True
    return False


def _dotnet_bridge_build_target_dir(command: str) -> str | None:
    tokens = _phase_effective_command_tokens(command)
    if not (
        len(tokens) == 4
        and _command_token_basename(tokens[0]) == "cargo"
        and tokens[1] == "build"
        and tokens[2] == "-p"
        and tokens[3] == "connect_norito_bridge"
    ):
        return None
    target_dirs = [
        value
        for assignment in _phase_prefix_env_assignments(command)
        for name, separator, value in (assignment.partition("="),)
        if separator and name == "CARGO_TARGET_DIR" and value
    ]
    if len(target_dirs) != 1:
        return None
    return target_dirs[0]


def _dotnet_bridge_build_has_noncanonical_env_prefix(command: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if not (
        len(tokens) == 4
        and _command_token_basename(tokens[0]) == "cargo"
        and tokens[1:] == ["build", "-p", "connect_norito_bridge"]
    ):
        return False
    assignments = _phase_prefix_env_assignments(command)
    if not assignments:
        return False
    return len(assignments) != 1 or not assignments[0].startswith("CARGO_TARGET_DIR=")


def _dotnet_phase_block_has_noncanonical_bridge_build_env_prefix(
    phase_block: str,
) -> bool:
    return any(
        _dotnet_bridge_build_has_noncanonical_env_prefix(command)
        for command in _phase_command_lines(phase_block)
    )


def _dotnet_bridge_build_command_matches(command: str) -> bool:
    return _dotnet_bridge_build_target_dir(command) is not None


def _dotnet_normalized_path_text(path: str) -> str:
    return path.replace("\\", "/").rstrip("/")


def _dotnet_phase_bridge_library_dir(phase_block: str) -> str | None:
    bridge_dirs = []
    for line in phase_block.splitlines():
        match = DOTNET_BRIDGE_PATH_SUCCESS_PATTERN.fullmatch(line)
        if match is None or not _dotnet_bridge_path_success_line_matches(line):
            continue
        bridge_path = _dotnet_normalized_path_text(match.group("path"))
        suffix = "/connect_norito_bridge.dll"
        if bridge_path.endswith(suffix):
            bridge_dirs.append(bridge_path[: -len(suffix)])
    if len(bridge_dirs) != 1 or not bridge_dirs[0]:
        return None
    return bridge_dirs[0]


def _dotnet_phase_block_bridge_path_matches_target_dir(phase_block: str) -> bool:
    target_dirs = [
        target_dir
        for command in _phase_command_lines(phase_block)
        if (target_dir := _dotnet_bridge_build_target_dir(command)) is not None
    ]
    bridge_paths = [
        match.group("path")
        for line in phase_block.splitlines()
        if (match := DOTNET_BRIDGE_PATH_SUCCESS_PATTERN.fullmatch(line)) is not None
        and _dotnet_bridge_path_success_line_matches(line)
    ]
    if len(target_dirs) != 1 or len(bridge_paths) != 1:
        return False
    target_dir = _dotnet_normalized_path_text(target_dirs[0])
    bridge_path = _dotnet_normalized_path_text(bridge_paths[0])
    if not target_dir:
        return False
    return bridge_path == f"{target_dir}/debug/connect_norito_bridge.dll"


def _gradle_test_selector_matches(expected: str, actual: str) -> bool:
    if expected.endswith("."):
        return actual in (expected, f"{expected}*")
    return actual == expected


def _command_token_looks_like_python_runner(token: str) -> bool:
    basename = _command_token_basename(token).lower()
    return bool(re.search(r"(^|[-_.])python(?:[0-9.]+)?($|[-_.])", basename))


def _gradle_test_command_selectors(tokens: list[str], task: str) -> list[str]:
    if (
        len(tokens) < 5
        or not tokens
        or _command_token_basename(tokens[0]) != "gradlew"
        or tokens[1] != task
        or tokens[2] != "--console=plain"
    ):
        return []
    selectors: list[str] = []
    index = 3
    while index < len(tokens):
        if tokens[index] != "--tests" or index + 1 >= len(tokens):
            return []
        selectors.append(tokens[index + 1])
        index += 2
    return selectors


def _gradle_test_selectors_match_expected(
    expected_selectors: tuple[str, ...],
    actual_selectors: list[str],
) -> bool:
    return len(actual_selectors) == len(expected_selectors) and all(
        _gradle_test_selector_matches(expected, actual)
        for expected, actual in zip(expected_selectors, actual_selectors)
    )


def _kotlin_sdk_expected_test_selectors() -> tuple[str, ...]:
    selectors: list[str] = []
    for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["kotlin-sdk"]:
        if fragment.startswith("./gradlew "):
            selectors.extend(_command_option_values(shlex.split(fragment), "--tests"))
        elif fragment.startswith("org.hyperledger.iroha.sdk.sccp."):
            selectors.append(fragment)
    return tuple(selectors)


def _java_android_expected_harness_classes() -> tuple[str, ...]:
    assignment = next(
        (
            fragment
            for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["java-android"]
            if fragment.startswith("ANDROID_HARNESS_MAINS=")
        ),
        "",
    )
    return tuple(item for item in assignment.partition("=")[2].split(",") if item)


def _java_android_harness_command_matches(
    command: str,
    test_selectors: list[str],
) -> bool:
    return (
        tuple(_android_harness_mains_classes(command))
        == _java_android_expected_harness_classes()
        and tuple(test_selectors) == ("org.hyperledger.iroha.android.GradleHarnessTests",)
    )


def _evidence_pytest_command_has_fragment(
    phase: str,
    command: str,
    fragment: str,
) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if not tokens or not _command_token_looks_like_python_runner(tokens[0]):
        return False
    module_index = next(
        (
            index
            for index in range(len(tokens) - 1)
            if tokens[index] == "-m" and tokens[index + 1] == "pytest"
        ),
        None,
    )
    if module_index is None or "-q" not in tokens[module_index + 2 :]:
        return False
    pytest_positionals = _pytest_command_positionals(tokens, module_index)
    if not pytest_positionals:
        return False
    expected_positionals = _pytest_expected_positionals_for_phase(phase)
    if tuple(pytest_positionals) != expected_positionals:
        return False
    if fragment.startswith("pytests/"):
        return fragment in pytest_positionals
    if fragment.startswith("python/"):
        return fragment in pytest_positionals
    if fragment.startswith("-m pytest"):
        expected_positionals = _pytest_fragment_positionals(fragment)
        return bool(expected_positionals) and all(
            item in pytest_positionals for item in expected_positionals
        )
    return fragment in command


def _js_sdk_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    expected_files = _node_expected_test_files_for_phase("js-sdk")
    test_files = _node_test_command_files(tokens)
    if test_files != expected_files:
        return False
    if fragment.startswith("--test "):
        return all(part in test_files for part in shlex.split(fragment)[1:])
    return fragment in test_files


def _swift_sdk_command_has_fragment(command: str, fragment: str) -> bool:
    if not _effective_command_starts_with(command, ("swift", "test")):
        return False
    tokens = _phase_effective_command_tokens(command)
    if fragment.startswith("swift test "):
        return tuple(tokens) == tuple(shlex.split(fragment))
    return tuple(tokens) == (
        "swift",
        "test",
        "--filter",
        fragment,
        "--disable-swift-testing",
    )


def _kotlin_sdk_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if fragment == "java -version":
        return tuple(tokens) == ("java", "-version")
    test_selectors = _gradle_test_command_selectors(tokens, ":core-jvm:test")
    if not test_selectors:
        return False
    if not _gradle_test_selectors_match_expected(
        _kotlin_sdk_expected_test_selectors(),
        test_selectors,
    ):
        return False
    if fragment.startswith("./gradlew "):
        fragment_tokens = shlex.split(fragment)
        expected_tests = _command_option_values(fragment_tokens, "--tests")
        return all(
            any(
                _gradle_test_selector_matches(expected, actual)
                for actual in test_selectors
            )
            for expected in expected_tests
        )
    if fragment.startswith("org.hyperledger.iroha.sdk.sccp."):
        return fragment in test_selectors
    return any(
        _gradle_test_selector_matches("org.hyperledger.iroha.sdk.sccp.", token)
        for token in test_selectors
    )


def _java_android_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if fragment == "java -version":
        return tuple(tokens) == ("java", "-version")
    test_selectors = _gradle_test_command_selectors(tokens, ":core:test")
    if not test_selectors:
        return False
    actual_harness_classes = _android_harness_mains_classes(command)
    if fragment.startswith("ANDROID_HARNESS_MAINS="):
        return _java_android_harness_command_matches(command, test_selectors)
    if (
        fragment.startswith("org.hyperledger.iroha.android.sccp.")
        and fragment.endswith("Tests")
    ):
        return (
            fragment in _java_android_expected_harness_classes()
            and _java_android_harness_command_matches(command, test_selectors)
        )
    fragment_tokens = shlex.split(fragment)
    expected_tests = _command_option_values(fragment_tokens, "--tests")
    if expected_tests:
        if expected_tests == ["org.hyperledger.iroha.android.GradleHarnessTests"]:
            return _java_android_harness_command_matches(command, test_selectors)
        return (
            all(part in tokens for part in fragment_tokens if part not in expected_tests)
            and tuple(test_selectors) == tuple(expected_tests)
            and not actual_harness_classes
        )
    return all(part in tokens for part in shlex.split(fragment))


def _dotnet_sdk_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if fragment == "dotnet --version":
        return (
            len(tokens) == 2
            and _command_token_basename(tokens[0]) == "dotnet"
            and tokens[1] == "--version"
        )
    if fragment == "dotnet --info":
        return (
            len(tokens) == 2
            and _command_token_basename(tokens[0]) == "dotnet"
            and tokens[1] == "--info"
        )
    if fragment == "dotnet restore Hyperledger.Iroha.Sdk.sln":
        return (
            len(tokens) == 3
            and _command_token_basename(tokens[0]) == "dotnet"
            and tuple(tokens[1:]) == ("restore", "Hyperledger.Iroha.Sdk.sln")
        )
    if not _dotnet_sdk_command_matches(tokens):
        return False
    if fragment.startswith("FullyQualifiedName"):
        return tokens[4].replace("\\|", "|") == fragment.replace("\\|", "|")
    if fragment.startswith("dotnet test "):
        fragment_tokens = shlex.split(fragment)
        return (
            _command_token_basename(tokens[0]) == fragment_tokens[0]
            and tuple(tokens[1 : len(fragment_tokens)]) == tuple(fragment_tokens[1:])
        )
    if fragment == "sccp-dotnet-sdk.trx":
        return any(fragment in token for token in tokens[6:])
    return all(part in tokens for part in shlex.split(fragment))


def _contract_smoke_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if fragment.endswith(".test.mjs"):
        test_files = _node_test_command_files(tokens)
        return test_files == _node_expected_test_files_for_phase(
            "contract-smoke"
        ) and fragment in test_files
    if fragment.startswith("--check "):
        expected_path = shlex.split(fragment)[1]
        return _node_check_command_matches(tokens, expected_path)
    return _effective_command_equals(
        command,
        ("bash", "scripts/sccp_evm_contract_smoke.sh"),
    )


def _core_admission_command_has_fragment(command: str, _fragment: str) -> bool:
    return _effective_command_equals(
        command,
        (
            "cargo",
            "test",
            "-p",
            "iroha_core",
            "--test",
            "iroha_core_group_01",
            "bridge_proofs::",
            "--",
            "--nocapture",
        ),
    )


def _phase_command_matches_required_fragment(
    phase: str,
    command: str,
    fragment: str,
) -> bool:
    if fragment not in command and not (
        phase == "java-android" and fragment.startswith("ANDROID_HARNESS_MAINS=")
    ):
        return False
    if phase == "evidence-scripts":
        return _evidence_pytest_command_has_fragment(phase, command, fragment)
    if phase == "rust-sccp":
        return _rust_sccp_command_has_fragment(command, fragment)
    if phase == "js-sdk":
        return _js_sdk_command_has_fragment(command, fragment)
    if phase == "python-sdk":
        return _evidence_pytest_command_has_fragment(phase, command, fragment)
    if phase == "swift-sdk":
        return _swift_sdk_command_has_fragment(command, fragment)
    if phase == "kotlin-sdk":
        return _kotlin_sdk_command_has_fragment(command, fragment)
    if phase == "java-android":
        return _java_android_command_has_fragment(command, fragment)
    if phase == "dotnet-sdk":
        if fragment == "cargo build -p connect_norito_bridge":
            return _dotnet_bridge_build_command_matches(command)
        return _dotnet_sdk_command_has_fragment(command, fragment)
    if phase == "contract-smoke":
        return _contract_smoke_command_has_fragment(command, fragment)
    if phase == "core-admission":
        return _core_admission_command_has_fragment(command, fragment)
    return True


def _phase_block_has_command_fragment(
    phase: str,
    phase_block: str,
    fragment: str,
) -> bool:
    return bool(_phase_block_command_fragment_line_indices(phase, phase_block, fragment))


def _phase_block_has_ordered_command_fragments(
    phase: str,
    phase_block: str,
    fragments: tuple[str, ...],
) -> bool:
    previous_position = -1
    for fragment in fragments:
        later_positions = [
            position
            for position in _phase_block_command_fragment_line_indices(
                phase,
                phase_block,
                fragment,
            )
            if position > previous_position
        ]
        if not later_positions:
            return False
        previous_position = min(later_positions)
    return True


def _phase_block_duplicate_command_fragment(
    phase: str,
    phase_block: str,
    fragments: tuple[str, ...],
) -> str | None:
    for fragment in fragments:
        positions = _phase_block_command_fragment_line_indices(
            phase,
            phase_block,
            fragment,
        )
        if len(positions) > 1:
            return fragment
    return None


def _phase_block_has_ordered_output_fragments(
    phase_block: str,
    fragments: tuple[str, ...],
) -> bool:
    previous_position = -1
    for fragment in fragments:
        later_positions = [
            position
            for position in _phase_block_output_fragment_line_indices(
                phase_block,
                fragment,
            )
            if position > previous_position
        ]
        if not later_positions:
            return False
        previous_position = min(later_positions)
    return True


def _phase_block_output_fragments_are_strictly_ordered(
    phase_block: str,
    fragments: tuple[str, ...],
) -> bool:
    previous_max_position = -1
    for fragment in fragments:
        positions = _phase_block_output_fragment_line_indices(phase_block, fragment)
        if not positions or min(positions) <= previous_max_position:
            return False
        previous_max_position = max(positions)
    return True


def _phase_block_duplicate_output_fragment(
    phase_block: str,
    fragments: tuple[str, ...],
) -> str | None:
    for fragment in fragments:
        if len(_phase_block_output_fragment_line_indices(phase_block, fragment)) > 1:
            return fragment
    return None


def _phase_success_fragment_required_command_fragment(
    phase: str,
    fragment: str,
) -> str | None:
    required_command_fragments = _phase_success_fragment_required_command_fragments(
        phase, fragment
    )
    if not required_command_fragments:
        return None
    return required_command_fragments[0]


def _phase_success_fragment_required_command_fragments(
    phase: str,
    fragment: str,
) -> tuple[str, ...]:
    if phase == "swift-sdk" and fragment == "0 failures":
        return (
            "swift test --filter SccpSolanaProverTests --disable-swift-testing",
            "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions",
        )
    if phase in ("kotlin-sdk", "java-android") and fragment == 'version "21':
        return ("java -version",)
    if phase == "kotlin-sdk" and fragment == "BUILD SUCCESSFUL":
        return (
            "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp.",
        )
    if phase == "java-android" and fragment == "BUILD SUCCESSFUL":
        return (
            "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests",
            "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
        )
    if phase == "contract-smoke":
        if fragment == "sccp_message_bridge_smoke: ok":
            return ("bash scripts/sccp_evm_contract_smoke.sh",)
        if fragment in CONTRACT_SMOKE_NODE_SUCCESS_FRAGMENTS:
            return ("scripts/sccp_taira_xor_contract.test.mjs",)
    if phase == "dotnet-sdk":
        if fragment == DOTNET_VERSION_SUCCESS_PREFIX:
            return ("dotnet --version",)
        if fragment in (
            DOTNET_WINDOWS_OS_SUCCESS_LINE,
            DOTNET_RID_SUCCESS_PREFIX,
            DOTNET_ARCHITECTURE_SUCCESS_PREFIX,
        ):
            return ("dotnet --info",)
        if fragment in (
            DOTNET_BRIDGE_PATH_SUCCESS_PREFIX,
            DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX,
        ):
            return ("cargo build -p connect_norito_bridge",)
        if fragment in (
            DOTNET_TEST_PASSED_SUCCESS_FRAGMENT,
            DOTNET_TRX_SUCCESS_PREFIX,
            DOTNET_TRX_BYTES_SUCCESS_PREFIX,
        ):
            return (
                "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
            )
    return ()


def _phase_success_command_windows(
    phase: str,
    phase_block: str,
    fragment: str,
    fallback_position: int,
    ceiling_position: int | None = None,
) -> list[tuple[int, int | None]]:
    required_command_fragments = _phase_success_fragment_required_command_fragments(
        phase, fragment
    )
    if not required_command_fragments:
        return [(fallback_position, ceiling_position)]

    phase_required_fragments = PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ())
    windows: list[tuple[int, int | None]] = []
    for required_command_fragment in required_command_fragments:
        required_success_command_positions = _phase_block_command_fragment_line_indices(
            phase,
            phase_block,
            required_command_fragment,
        )
        if ceiling_position is not None:
            required_success_command_positions = [
                position
                for position in required_success_command_positions
                if position < ceiling_position
            ]
        if not required_success_command_positions:
            return []
        anchor_position = max(required_success_command_positions)
        try:
            required_fragment_index = phase_required_fragments.index(
                required_command_fragment
            )
        except ValueError:
            next_required_fragments: tuple[str, ...] = ()
        else:
            next_required_fragments = phase_required_fragments[
                required_fragment_index + 1 :
            ]
        later_command_positions: list[int] = []
        for later_fragment in next_required_fragments:
            later_command_positions.extend(
                position
                for position in _phase_block_command_fragment_line_indices(
                    phase,
                    phase_block,
                    later_fragment,
                )
                if position > anchor_position
                and (ceiling_position is None or position < ceiling_position)
            )
        window_ceiling = (
            min(later_command_positions)
            if later_command_positions
            else ceiling_position
        )
        windows.append((anchor_position, window_ceiling))
    return windows


def _phase_success_fragment_has_position_after_required_command(
    phase: str,
    phase_block: str,
    fragment: str,
    success_positions: list[int],
    fallback_position: int,
) -> bool:
    windows = _phase_success_command_windows(
        phase,
        phase_block,
        fragment,
        fallback_position,
    )
    if not windows:
        return False
    return all(
        any(
            position > anchor_position
            and (window_ceiling is None or position < window_ceiling)
            for position in success_positions
        )
        for anchor_position, window_ceiling in windows
    )


def _phase_success_fragment_positions_are_only_in_required_command_windows(
    phase: str,
    phase_block: str,
    fragment: str,
    success_positions: list[int],
    fallback_position: int,
) -> bool:
    windows = _phase_success_command_windows(
        phase,
        phase_block,
        fragment,
        fallback_position,
    )
    if not windows:
        return False
    return all(
        any(
            position > anchor_position
            and (window_ceiling is None or position < window_ceiling)
            for anchor_position, window_ceiling in windows
        )
        for position in success_positions
    )


def _phase_success_fragment_has_position_before_completion(
    phase: str,
    phase_block: str,
    fragment: str,
    success_positions: list[int],
    fallback_position: int,
    completion_position: int,
) -> bool:
    windows = _phase_success_command_windows(
        phase,
        phase_block,
        fragment,
        fallback_position,
        completion_position,
    )
    if not windows:
        return False
    return all(
        any(
            anchor_position < position < window_ceiling
            for position in success_positions
        )
        for anchor_position, window_ceiling in windows
        if window_ceiling is not None
    )


def _phase_block_has_output_fragment(
    phase: str,
    phase_block: str,
    fragment: str,
) -> bool:
    success_positions = _phase_block_output_fragment_line_indices(phase_block, fragment)
    if not success_positions:
        return False
    return _phase_success_fragment_has_position_after_required_command(
        phase,
        phase_block,
        fragment,
        success_positions,
        -1,
    )


def _phase_block_command_fragment_line_indices(
    phase: str,
    phase_block: str,
    fragment: str,
) -> list[int]:
    return [
        index
        for index, line in enumerate(phase_block.splitlines())
        if line.lstrip().startswith("+ ")
        and _phase_command_matches_required_fragment(phase, line.strip(), fragment)
    ]


def _phase_block_output_fragment_line_indices(
    phase_block: str,
    fragment: str,
) -> list[int]:
    return [
        index
        for index, line in enumerate(phase_block.splitlines())
        if _phase_output_line_has_success_fragment(line, fragment)
    ]


def _phase_output_line_has_success_fragment(line: str, fragment: str) -> bool:
    if _line_is_shell_xtrace_command(line):
        return False
    normalized_line = _phase_output_failure_scan_line(line)
    scan_lines = (line,) if normalized_line == line else (line, normalized_line)
    if fragment == DOTNET_VERSION_SUCCESS_PREFIX:
        return any(
            DOTNET_VERSION_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_WINDOWS_OS_SUCCESS_LINE:
        return any(scan_line == DOTNET_WINDOWS_OS_SUCCESS_LINE for scan_line in scan_lines)
    if fragment == DOTNET_RID_SUCCESS_PREFIX:
        return any(
            DOTNET_RID_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_ARCHITECTURE_SUCCESS_PREFIX:
        return any(
            DOTNET_ARCHITECTURE_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_BRIDGE_PATH_SUCCESS_PREFIX:
        return any(
            _dotnet_bridge_path_success_line_matches(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX:
        return any(
            DOTNET_BRIDGE_SHA256_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_TEST_PASSED_SUCCESS_FRAGMENT:
        return any(
            _dotnet_test_passed_success_line_matches(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_TRX_SUCCESS_PREFIX:
        return any(
            DOTNET_TRX_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    if fragment == DOTNET_TRX_BYTES_SUCCESS_PREFIX:
        return any(
            DOTNET_TRX_BYTES_SUCCESS_PATTERN.fullmatch(scan_line)
            for scan_line in scan_lines
        )
    for scan_line in scan_lines:
        position = scan_line.find(fragment)
        if position < 0:
            continue
        prefix = scan_line[:position]
        if SUCCESS_OUTPUT_NEGATION_PATTERN.search(
            prefix
        ) or SUCCESS_OUTPUT_DIAGNOSTIC_PREFIX_PATTERN.search(prefix):
            continue
        return True
    return False


def _dotnet_test_passed_success_line_matches(line: str) -> bool:
    match = DOTNET_TEST_PASSED_SUCCESS_PATTERN.fullmatch(line)
    if match is None:
        return False
    passed = int(match.group("passed"))
    skipped = int(match.group("skipped"))
    total = int(match.group("total"))
    return total == passed + skipped and skipped == 0


def _dotnet_phase_block_rid_architecture_markers_match(phase_block: str) -> bool:
    rid_architectures: list[str] = []
    architectures: list[str] = []
    for line in phase_block.splitlines():
        if _line_is_shell_xtrace_command(line):
            continue
        normalized_line = _phase_output_failure_scan_line(line)
        scan_lines = (line,) if normalized_line == line else (line, normalized_line)
        for scan_line in scan_lines:
            if DOTNET_RID_SUCCESS_PATTERN.fullmatch(scan_line):
                rid = scan_line.split(":", 1)[1].strip()
                rid_architectures.append(DOTNET_RID_ARCHITECTURES[rid])
                break
            if DOTNET_ARCHITECTURE_SUCCESS_PATTERN.fullmatch(scan_line):
                architecture = scan_line.split(":", 1)[1].strip()
                architectures.append(architecture)
                break
    if len(rid_architectures) != 1 or len(architectures) != 1:
        return True
    return rid_architectures[0] == architectures[0]


def _dotnet_bridge_path_success_line_matches(line: str) -> bool:
    match = DOTNET_BRIDGE_PATH_SUCCESS_PATTERN.fullmatch(line)
    if match is None:
        return False
    path = match.group("path")
    raw_parts = re.split(r"[\\/]", path)
    if raw_parts and raw_parts[0] == "":
        raw_parts = raw_parts[1:]
    if any(part in {"", ".", ".."} for part in raw_parts):
        return False
    if any(
        any(char.isspace() or ord(char) < 0x20 for char in part)
        for part in raw_parts
    ):
        return False
    for index, part in enumerate(raw_parts):
        if index == 0 and DOTNET_BRIDGE_PATH_DRIVE_PATTERN.fullmatch(part):
            continue
        if DOTNET_BRIDGE_PATH_COMPONENT_PATTERN.fullmatch(part) is None:
            return False
    return (
        len(raw_parts) >= 3
        and raw_parts[-2] == "debug"
        and raw_parts[-1] == "connect_norito_bridge.dll"
    )


def _phase_block_has_exact_output_line(phase_block: str, expected: str) -> bool:
    return any(
        line == expected
        for line in phase_block.splitlines()
        if not _line_is_shell_xtrace_command(line)
    )


def _phase_block_has_completion_after_required_evidence(
    phase: str,
    phase_block: str,
) -> bool:
    completion_positions = [
        index
        for index, line in enumerate(phase_block.splitlines())
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False

    command_positions_by_fragment = [
        _phase_block_command_fragment_line_indices(phase, phase_block, fragment)
        for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ())
    ]
    success_positions_by_fragment = [
        (fragment, _phase_block_output_fragment_line_indices(phase_block, fragment))
        for fragment in PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase, ())
    ]
    for completion_position in completion_positions:
        command_positions_before_completion: list[int] = []
        for positions in command_positions_by_fragment:
            positions_before_completion = [
                position for position in positions if position < completion_position
            ]
            if not positions_before_completion:
                break
            command_positions_before_completion.extend(positions_before_completion)
        else:
            if not command_positions_before_completion:
                continue
            first_command_position = min(command_positions_before_completion)
            if all(
                _phase_success_fragment_has_position_before_completion(
                    phase,
                    phase_block,
                    fragment,
                    positions,
                    first_command_position,
                    completion_position,
                )
                for fragment, positions in success_positions_by_fragment
            ):
                return True
    return False


def _phase_block_has_traced_command_after_completion(phase_block: str) -> bool:
    lines = phase_block.splitlines()
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False
    first_completion = min(completion_positions)
    return any(
        index > first_completion and _line_is_shell_xtrace_command(line)
        for index, line in enumerate(lines)
    )


def _phase_block_has_nonempty_line_after_completion(phase_block: str) -> bool:
    lines = phase_block.splitlines()
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False
    first_completion = min(completion_positions)
    return any(
        index > first_completion and bool(line.strip())
        for index, line in enumerate(lines)
    )


def _transcript_has_traced_command_after_completion(transcript: str) -> bool:
    lines = transcript.splitlines()
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False
    first_completion = min(completion_positions)
    return any(
        index > first_completion and _line_is_shell_xtrace_command(line)
        for index, line in enumerate(lines)
    )


def _transcript_has_nonempty_line_after_completion(transcript: str) -> bool:
    lines = transcript.splitlines()
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if line == CORRIDOR_COMPLETION_SENTINEL
        and not line.lstrip().startswith("+ ")
    ]
    if not completion_positions:
        return False
    first_completion = min(completion_positions)
    return any(
        index > first_completion and bool(line.strip())
        for index, line in enumerate(lines)
    )


def _phase_output_failure_scan_line(line: str) -> str:
    """Return output text normalized for failure-marker scanning."""

    stripped_line = ASCII_CONTROL_CHARACTER_PATTERN.sub(
        "",
        ANSI_ESCAPE_PATTERN.sub("", line),
    )
    return "".join(
        character
        for character in stripped_line
        if unicodedata.category(character) != "Cf"
    )


def _phase_diagnostic_fragment(fragment: str) -> str:
    """Return a public-safe phase marker fragment for Markdown diagnostics."""

    if (
        fragment.strip() != fragment
        or _path_control_character(fragment) is not None
        or not fragment.isascii()
        or _path_markdown_unsafe_character(fragment) is not None
    ):
        return (
            repr(fragment)
            .replace("|", "\\x7c")
            .replace("`", "\\x60")
            .replace("<", "\\x3c")
            .replace(">", "\\x3e")
        )
    return fragment


def _phase_block_forbidden_output_marker(phase: str, phase_block: str) -> str | None:
    for line in phase_block.splitlines():
        if _line_is_shell_xtrace_command(line):
            continue
        normalized_line = _phase_output_failure_scan_line(line)
        scan_lines = (line,) if normalized_line == line else (line, normalized_line)
        for pattern in PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS.get(phase, ()):
            if any(pattern.search(scan_line) for scan_line in scan_lines):
                return pattern.pattern
    return None


def _phase_transcript_artifact_path(artifact: Any) -> tuple[Path | None, list[str]]:
    if not isinstance(artifact, dict):
        return None, ["evidence artifact cannot be checked: malformed artifact row"]
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return None, ["evidence artifact cannot be checked: missing artifact path"]
    if (
        artifact_path.strip() != artifact_path
        or _path_control_character(artifact_path) is not None
        or _path_markdown_unsafe_character(artifact_path) is not None
        or _path_percent_encoded_traversal(artifact_path) is not None
    ):
        return None, ["evidence artifact cannot be checked: unsafe artifact path"]
    return Path(artifact_path), []


def _phase_transcript_errors(phase: str, artifact: Any) -> list[str]:
    path, artifact_path_errors = _phase_transcript_artifact_path(artifact)
    if artifact_path_errors:
        return artifact_path_errors
    assert path is not None
    try:
        transcript = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return ["evidence artifact is not UTF-8 text"]
    phase_block = _phase_transcript_block(phase, transcript)
    errors: list[str] = []
    if CORRIDOR_DRY_RUN_SENTINEL in transcript:
        errors.append("evidence artifact is a dry-run transcript")
    if _unknown_corridor_phase_marker_lines(transcript):
        errors.append("evidence artifact contains unknown corridor phase marker")
    if _transcript_has_nonempty_line_before_first_phase_marker(transcript):
        errors.append(
            "evidence artifact contains non-empty output before first phase marker"
        )
    if phase_block is None:
        errors.append("evidence artifact is missing the phase marker")
    elif _phase_marker_count(phase, transcript) > 1:
        errors.append("evidence artifact has duplicate phase marker")
    elif (
        not _phase_block_has_exact_output_line(
            phase_block, CORRIDOR_COMPLETION_SENTINEL
        )
        and not _transcript_has_full_corridor_completion(transcript)
    ):
        errors.append(
            "evidence artifact is missing the phase-block completion sentinel"
        )
    elif (
        _phase_block_has_exact_output_line(phase_block, CORRIDOR_COMPLETION_SENTINEL)
        and not _phase_block_has_completion_after_required_evidence(
            phase, phase_block
        )
        and not _transcript_has_full_corridor_completion(transcript)
    ):
        errors.append(
            "evidence artifact completion sentinel precedes required phase evidence"
        )
    if (
        phase_block is not None
        and _phase_block_has_traced_command_after_completion(phase_block)
    ):
        errors.append(
            "evidence artifact contains traced command after completion sentinel"
        )
    if (
        phase_block is not None
        and _phase_block_has_nonempty_line_after_completion(phase_block)
    ):
        errors.append(
            "evidence artifact contains non-empty output after completion sentinel"
        )
    if phase_block is not None and _phase_block_has_unparseable_command(phase_block):
        errors.append("evidence artifact contains unparseable traced command")
    if (
        phase_block is not None
        and _phase_block_has_unsupported_parenthesized_group(phase_block)
    ):
        errors.append(
            "evidence artifact contains unsupported parenthesized traced command"
        )
    if (
        phase_block is not None
        and _phase_block_has_unexpected_runner_cd_wrapper(phase, phase_block)
    ):
        errors.append(
            "evidence artifact contains unexpected runner cd wrapper directory"
        )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and _dotnet_phase_block_has_noncanonical_env_prefix(phase_block)
    ):
        errors.append(
            "evidence artifact .NET phase used a non-canonical environment prefix"
        )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and _dotnet_phase_block_has_noncanonical_bridge_build_env_prefix(phase_block)
    ):
        errors.append(
            "evidence artifact .NET native bridge build used a non-canonical environment prefix"
        )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and not _phase_block_has_ordered_command_fragments(
            phase,
            phase_block,
            (
                "dotnet --version",
                "dotnet --info",
                "cargo build -p connect_norito_bridge",
                "dotnet restore Hyperledger.Iroha.Sdk.sln",
                "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
            ),
        )
    ):
        errors.append(
            "evidence artifact .NET commands are not in required version-info-bridge-restore-test order"
        )
    if phase_block is not None:
        duplicate_phase_command = _phase_block_duplicate_command_fragment(
            phase,
            phase_block,
            PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ()),
        )
        if duplicate_phase_command is not None:
            duplicate_command_error = (
                ".NET command appears more than once:"
                if phase == EVM_NATIVE_DOTNET_PHASE
                else f"{PHASE_DUPLICATE_COMMAND_ERROR_FRAGMENT}:"
            )
            errors.append(
                f"evidence artifact {duplicate_command_error} "
                f"{_phase_diagnostic_fragment(duplicate_phase_command)}"
            )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and not _dotnet_phase_block_bridge_path_matches_target_dir(phase_block)
    ):
        errors.append(
            "evidence artifact .NET native bridge path does not match "
            "CARGO_TARGET_DIR/debug handoff"
        )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and _dotnet_phase_block_forbidden_test_command(phase_block)
    ):
        errors.append("evidence artifact .NET phase ran a non-canonical dotnet test command")
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and _dotnet_phase_block_forbidden_setup_command(phase_block)
    ):
        errors.append("evidence artifact .NET phase ran a non-canonical dotnet setup command")
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and not _dotnet_phase_block_rid_architecture_markers_match(phase_block)
    ):
        errors.append("evidence artifact .NET RID and architecture markers disagree")
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and not _phase_block_output_fragments_are_strictly_ordered(
            phase_block,
            (
                DOTNET_BRIDGE_PATH_SUCCESS_PREFIX,
                DOTNET_BRIDGE_SHA256_SUCCESS_PREFIX,
            ),
        )
    ):
        errors.append(
            "evidence artifact .NET native bridge markers are not in required path-sha256 order"
        )
    if (
        phase == EVM_NATIVE_DOTNET_PHASE
        and phase_block is not None
        and not _phase_block_output_fragments_are_strictly_ordered(
            phase_block,
            (
                DOTNET_TEST_PASSED_SUCCESS_FRAGMENT,
                DOTNET_TRX_SUCCESS_PREFIX,
                DOTNET_TRX_BYTES_SUCCESS_PREFIX,
            ),
        )
    ):
        errors.append(
            "evidence artifact .NET success markers are not in required passed-trx-bytes order"
        )
    if phase == EVM_NATIVE_DOTNET_PHASE and phase_block is not None:
        duplicate_dotnet_marker = _phase_block_duplicate_output_fragment(
            phase_block,
            PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase, ()),
        )
        if duplicate_dotnet_marker is not None:
            errors.append(
                "evidence artifact .NET evidence marker appears more than once: "
                f"{_phase_diagnostic_fragment(duplicate_dotnet_marker)}"
            )
    if (
        phase_block is not None
        and _transcript_has_multiple_known_phase_markers(transcript)
        and not _transcript_has_full_corridor_completion(transcript)
    ):
        errors.append(
            "evidence artifact contains incomplete multi-phase corridor transcript"
        )
    required_fragments = PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase)
    if required_fragments is None:
        errors.append("evidence artifact has no expected command fragment configured")
    elif phase_block is not None:
        for fragment in required_fragments:
            if not _phase_block_has_command_fragment(phase, phase_block, fragment):
                errors.append(
                    "evidence artifact is missing expected phase-block command: "
                    f"{_phase_diagnostic_fragment(fragment)}"
                )
    success_fragments = PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase)
    if success_fragments is None:
        errors.append("evidence artifact has no expected success fragment configured")
    elif phase_block is not None:
        phase_command_positions = [
            position
            for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase, ())
            for position in _phase_block_command_fragment_line_indices(
                phase,
                phase_block,
                fragment,
            )
        ]
        first_phase_command_position = (
            min(phase_command_positions) if phase_command_positions else -1
        )
        for fragment in success_fragments:
            if not _phase_block_has_output_fragment(phase, phase_block, fragment):
                errors.append(
                    "evidence artifact is missing expected phase-block success marker: "
                    f"{_phase_diagnostic_fragment(fragment)}"
                )
            elif (
                not _phase_success_fragment_positions_are_only_in_required_command_windows(
                    phase,
                    phase_block,
                    fragment,
                    _phase_block_output_fragment_line_indices(phase_block, fragment),
                    first_phase_command_position,
                )
            ):
                outside_window_error = (
                    DOTNET_SUCCESS_OUTSIDE_WINDOW_ERROR_FRAGMENT
                    if phase == EVM_NATIVE_DOTNET_PHASE
                    else PHASE_SUCCESS_OUTSIDE_WINDOW_ERROR_FRAGMENT
                )
                errors.append(
                    f"evidence artifact {outside_window_error}: "
                    f"{_phase_diagnostic_fragment(fragment)}"
                )
            else:
                success_positions = _phase_block_output_fragment_line_indices(
                    phase_block,
                    fragment,
                )
                expected_windows = _phase_success_command_windows(
                    phase,
                    phase_block,
                    fragment,
                    first_phase_command_position,
                )
                if len(success_positions) > len(expected_windows):
                    errors.append(
                        f"evidence artifact {PHASE_DUPLICATE_SUCCESS_ERROR_FRAGMENT}: "
                        f"{_phase_diagnostic_fragment(fragment)}"
                    )
    if phase_block is not None:
        forbidden_marker = _phase_block_forbidden_output_marker(phase, phase_block)
        if forbidden_marker is not None:
            errors.append(
                "evidence artifact contains forbidden phase-block failure marker: "
                f"{_phase_diagnostic_fragment(forbidden_marker)}"
            )
    return errors


def _parse_phase_evidence(
    values: list[str],
    phases: list[str],
    phase_status: dict[str, str],
    phase_evidence_dir: Path | None,
) -> dict[str, dict[str, Any]]:
    artifacts: dict[str, dict[str, Any]] = {}
    source_labels: dict[str, str] = {}

    def assign(phase: str, artifact: dict[str, Any], label: str) -> None:
        previous = source_labels.get(phase)
        if previous is not None:
            raise argparse.ArgumentTypeError(
                f"duplicate SCCP corridor phase evidence for {phase}: "
                f"already set by {previous}, cannot set from {label}"
            )
        artifacts[phase] = artifact
        source_labels[phase] = label

    if phase_evidence_dir is not None:
        dir_error = _phase_evidence_directory_path_error(str(phase_evidence_dir))
        if dir_error is not None:
            raise argparse.ArgumentTypeError(dir_error)
        for phase in phases:
            if phase_status.get(phase) == "passed":
                assign(
                    phase,
                    _artifact(_phase_log_from_dir(phase_evidence_dir, phase)),
                    "--phase-evidence-dir",
                )
    for raw in values:
        if "=" not in raw:
            raise argparse.ArgumentTypeError(
                "phase evidence must use NAME=PATH syntax"
            )
        name, path_text = raw.split("=", 1)
        name = _parse_phase_assignment_name(name, "phase evidence")
        if not path_text:
            raise argparse.ArgumentTypeError(
                "phase evidence path must not be empty"
            )
        path_error = _phase_evidence_path_error(path_text)
        if path_error is not None:
            raise argparse.ArgumentTypeError(path_error)
        artifact = _artifact(Path(path_text))
        label = _phase_evidence_source_label(name)
        if name == "all":
            for phase in phases:
                assign(phase, artifact, label)
            continue
        if name not in phases:
            raise argparse.ArgumentTypeError("unknown SCCP corridor phase")
        assign(name, artifact, label)
    return artifacts


def _load_evidence_summary(paths: list[Path]) -> dict[str, Any]:
    module = _load_all_lanes_module()
    records = module.load_evidence_bundle(paths)
    return module.validate_evidence_bundle(records)


def _active_launch_lane(evidence: dict[str, Any]) -> dict[str, Any] | None:
    for lane in evidence.get("lanes", []):
        if isinstance(lane, dict) and lane.get("domain") == ACTIVE_LAUNCH_DOMAIN:
            return lane
    return None


def _active_launch_blockers(evidence: dict[str, Any]) -> list[str]:
    prefix = f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN}): "
    blockers: list[str] = []

    def add(blocker: str) -> None:
        if blocker not in blockers:
            blockers.append(blocker)

    evidence_blockers = evidence.get("blockers")
    if not isinstance(evidence_blockers, list):
        add("SCCP evidence blocker summary is malformed")
    else:
        seen_evidence_blockers: set[str] = set()
        duplicate_evidence_blocker_reported = False
        for blocker in evidence_blockers:
            if (
                not isinstance(blocker, str)
                or not blocker
                or blocker.strip() != blocker
            ):
                add("SCCP evidence blocker must be a non-empty canonical string")
                continue
            if blocker.startswith(prefix):
                canonical_blocker = blocker
            elif not blocker.startswith("domain "):
                canonical_blocker = blocker
            else:
                continue
            if canonical_blocker in seen_evidence_blockers:
                if canonical_blocker in blockers:
                    blockers.remove(canonical_blocker)
                if not duplicate_evidence_blocker_reported:
                    add("SCCP evidence blockers must not contain duplicate strings")
                    duplicate_evidence_blocker_reported = True
                continue
            seen_evidence_blockers.add(canonical_blocker)
            add(canonical_blocker)
    lane = _active_launch_lane(evidence)
    if lane is None:
        add(
            f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN}): missing launch lane evidence"
        )
        return blockers

    lane_blockers = lane.get("blockers")
    if not isinstance(lane_blockers, list):
        add(f"{prefix}active launch lane blocker summary is malformed")
        return blockers
    seen_lane_blockers: set[str] = set()
    duplicate_lane_blocker_reported = False
    for blocker in lane_blockers:
        if (
            not isinstance(blocker, str)
            or not blocker
            or blocker.strip() != blocker
        ):
            add(
                f"{prefix}active launch lane blocker must be a non-empty canonical string"
            )
            continue
        if blocker.startswith(prefix):
            canonical_blocker = blocker
        elif not blocker.startswith("domain "):
            canonical_blocker = f"{prefix}{blocker}"
        else:
            continue
        if canonical_blocker in seen_lane_blockers:
            if canonical_blocker in blockers:
                blockers.remove(canonical_blocker)
            if not duplicate_lane_blocker_reported:
                add(f"{prefix}active launch lane blockers must not contain duplicate strings")
                duplicate_lane_blocker_reported = True
            continue
        seen_lane_blockers.add(canonical_blocker)
        add(canonical_blocker)
    return blockers


def _string_list_or_schema_blockers(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str) or not item or item.strip() != item:
            blockers.append(
                f"{label}[{index}] must be a non-empty canonical string"
            )
        else:
            blockers.append(item)
    return blockers


def _native_evm_validation_blocker_issue(
    item: Any,
    label: str,
    index: int,
) -> str | None:
    item_label = f"{label}[{index}]"
    if not isinstance(item, str) or not item or item.strip() != item:
        return f"{item_label} must be a non-empty canonical string"
    if _path_control_character(item) is not None:
        return f"{item_label} contains control character"
    if not item.isascii():
        return f"{item_label} contains non-ASCII character"
    if _path_markdown_unsafe_character(item) is not None:
        return f"{item_label} contains Markdown-unsafe character"
    if any(marker in item.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        # Source-inventory marker: validation_blockers[0] contains sensitive name
        return f"{item_label} contains sensitive name"
    return None


def _native_evm_validation_blockers(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    seen: set[str] = set()
    duplicate_reported = False
    for index, item in enumerate(value):
        issue = _native_evm_validation_blocker_issue(item, label, index)
        if issue is not None:
            blockers.append(issue)
            continue
        if item in seen:
            blockers = [blocker for blocker in blockers if blocker != item]
            if not duplicate_reported:
                blockers.append(f"{label} must not contain duplicate strings")
                duplicate_reported = True
            continue
        seen.add(item)
        blockers.append(item)
    return blockers


def _public_blocker_text_issue(item: Any) -> str | None:
    if not isinstance(item, str) or not item or item.strip() != item:
        return "non-empty canonical string"
    if _path_control_character(item) is not None:
        return "control character"
    if not item.isascii():
        return "non-ASCII character"
    if _path_markdown_unsafe_character(item) is not None:
        return "Markdown-unsafe character"
    if any(marker in item.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return "sensitive name"
    return None


def _public_blocker_list_duplicate_error(value: Any, label: str) -> str | None:
    """Return a bounded blocker when a public blocker list repeats values."""

    if not isinstance(value, list):
        return None
    seen: set[str] = set()
    for item in value:
        if _public_blocker_text_issue(item) is not None:
            continue
        if item in seen:
            return f"{label} must not contain duplicate strings"
        seen.add(item)
    return None


def _active_launch_lane_blockers_for_checklist(
    value: Any,
    lane_label: str,
) -> tuple[list[str], list[str]]:
    label = f"{lane_label}: active launch lane blockers"
    if not isinstance(value, list):
        return [], [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    schema_blockers: list[str] = []
    seen_blockers: set[str] = set()
    duplicate_reported = False
    for index, item in enumerate(value):
        if not isinstance(item, str) or not item or item.strip() != item:
            schema_blockers.append(
                f"{label}[{index}] must be a non-empty canonical string"
            )
            continue
        if item in seen_blockers:
            blockers = [blocker for blocker in blockers if blocker != item]
            if not duplicate_reported:
                schema_blockers.append(f"{label} must not contain duplicate strings")
                duplicate_reported = True
            continue
        seen_blockers.add(item)
        blockers.append(item)
    return blockers, schema_blockers


def _active_launch_evm_live_metadata_blockers(
    lane_label: str,
    lane: dict[str, Any],
) -> list[str]:
    """Return EVM live-read blockers for the active launch lane."""

    evm_live_metadata = lane.get("evm_live_metadata")
    if not isinstance(evm_live_metadata, dict):
        evm_live_metadata = {}
    expected_chain_id = ACTIVE_LAUNCH_EVM_DECIMAL_CHAIN_ID
    expected_chain_id_label = (
        f"canonical decimal chain id {expected_chain_id}"
        if expected_chain_id is not None
        else "the configured mainnet chain id"
    )

    blockers: list[str] = []
    source_chain_id = evm_live_metadata.get("source_rpc_chain_id")
    if not (
        _is_canonical_decimal_text(source_chain_id, positive=True)
        and source_chain_id == expected_chain_id
    ):
        blockers.append(
            f"{lane_label}: {ACTIVE_LAUNCH_DISPLAY} source live eth_chainId must be {expected_chain_id_label}"
        )
    destination_chain_id = evm_live_metadata.get("destination_rpc_chain_id")
    if not (
        _is_canonical_decimal_text(destination_chain_id, positive=True)
        and destination_chain_id == expected_chain_id
    ):
        blockers.append(
            f"{lane_label}: {ACTIVE_LAUNCH_DISPLAY} destination live eth_chainId must be {expected_chain_id_label}"
        )
    if evm_live_metadata.get("source_block_tag") != "finalized":
        blockers.append(
            f"{lane_label}: {ACTIVE_LAUNCH_DISPLAY} source live block tag must be finalized"
        )
    if evm_live_metadata.get("destination_block_tag") != "finalized":
        blockers.append(
            f"{lane_label}: {ACTIVE_LAUNCH_DISPLAY} destination live block tag must be finalized"
        )
    return blockers


def _is_canonical_decimal_text(value: Any, *, positive: bool) -> bool:
    if not isinstance(value, str) or not value:
        return False
    if not all(symbol in "0123456789" for symbol in value):
        return False
    if len(value) > 1 and value.startswith("0"):
        return False
    if positive and value == "0":
        return False
    return True


def _active_launch_source_record_hash_role_blockers(
    lane_label: str,
    source_hashes: dict[str, Any],
    evidence_label: str,
) -> list[str]:
    """Return blockers for reused active-launch source record hash roles."""

    source_verifier_material_hash = source_hashes.get("source_verifier_material_hash")
    source_adapter_engine_deployment_hash = source_hashes.get(
        "source_adapter_engine_deployment_hash"
    )
    if (
        _is_nonzero_hex32(source_verifier_material_hash)
        and _is_nonzero_hex32(source_adapter_engine_deployment_hash)
        and source_verifier_material_hash == source_adapter_engine_deployment_hash
    ):
        return [
            f"{lane_label}: {evidence_label} source verifier material hash must not reuse source adapter engine deployment hash"
        ]
    return []


def _active_launch_hash_role_reuse_blockers(
    lane_label: str,
    evidence_label: str,
    role_label: str,
    role_hash: Any,
    prior_roles: tuple[tuple[str, Any], ...],
) -> list[str]:
    """Return blockers when an active launch hash role reuses earlier evidence."""

    if not _is_nonzero_hex32(role_hash):
        return []
    return [
        f"{lane_label}: {evidence_label} {role_label} must not reuse {prior_label}"
        for prior_label, prior_hash in prior_roles
        if _is_nonzero_hex32(prior_hash) and role_hash == prior_hash
    ]


def _active_launch_route_canary_metadata_blockers(
    lane_label: str,
    canary: dict[str, Any],
    upstream_hash_roles: tuple[tuple[str, Any], ...] = (),
) -> list[str]:
    """Return active launch route-canary transaction metadata blockers."""

    blockers = _active_launch_route_canary_blocker_container_errors(
        lane_label,
        canary,
    )
    if not _is_nonzero_hex32(canary.get("evidence_hash")):
        blockers.append(
            f"{lane_label}: route canary evidence hash must be a canonical non-zero bytes32 hex string"
        )
    evidence_source = canary.get("evidence_source")
    if (
        not isinstance(evidence_source, str)
        or not evidence_source
        or evidence_source.strip() != evidence_source
    ):
        blockers.append(
            f"{lane_label}: route canary evidence source must be a non-empty canonical string"
        )
    elif evidence_source != ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE:
        blockers.append(
            f"{lane_label}: route canary evidence source must be {ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE}"
        )
    for field, label in (
        ("transaction_hash", "transaction hash"),
        ("receipt_block_hash", "receipt block hash"),
        ("block_receipts_root", "block receipts root"),
        ("message_id", "message id"),
    ):
        if not _is_nonzero_hex32(canary.get(field)):
            blockers.append(
                f"{lane_label}: route canary {label} must be a canonical non-zero bytes32 hex string"
            )
    if (
        type(canary.get("receipt_block_number")) is not int
        or canary.get("receipt_block_number") <= 0
    ):
        blockers.append(
            f"{lane_label}: route canary receipt block number must be a positive integer"
        )
    if (
        "message_proof_used" in canary
        and type(canary.get("message_proof_used")) is not bool
    ):
        blockers.append(
            f"{lane_label}: route canary message_proof_used must be boolean"
        )
    if canary.get("message_proof_used") is not True:
        blockers.append(f"{lane_label}: route canary message proof must be used")
    if (
        "receipt_block_finalized" in canary
        and type(canary.get("receipt_block_finalized")) is not bool
    ):
        blockers.append(
            f"{lane_label}: route canary receipt_block_finalized must be boolean"
        )
    if canary.get("receipt_block_finalized") is not True:
        blockers.append(f"{lane_label}: route canary receipt block must be finalized")
    canary_hash_roles = (
        ("evidence hash", canary.get("evidence_hash")),
        ("transaction hash", canary.get("transaction_hash")),
        ("receipt block hash", canary.get("receipt_block_hash")),
        ("block receipts root", canary.get("block_receipts_root")),
        ("message id", canary.get("message_id")),
    )
    for index, (role_label, role_hash) in enumerate(canary_hash_roles):
        blockers.extend(
            _active_launch_hash_role_reuse_blockers(
                lane_label,
                "route canary",
                role_label,
                role_hash,
                (*upstream_hash_roles, *canary_hash_roles[:index]),
            )
        )
    return blockers


def _active_launch_route_canary_blocker_container_errors(
    lane_label: str,
    canary: dict[str, Any],
) -> list[str]:
    """Return blockers for copied active route-canary blocker containers."""

    canary_blockers = canary.get("blockers", [])
    label = f"{lane_label}: route canary blockers"
    if not isinstance(canary_blockers, list):
        # Source-inventory marker: route canary blockers must be a list of non-empty canonical strings
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    for index, blocker in enumerate(canary_blockers):
        issue = _public_blocker_text_issue(blocker)
        if issue == "non-empty canonical string":
            blockers.append(f"{label}[{index}] must be a non-empty canonical string")
        elif issue is not None:
            blockers.append(f"{label}[{index}] contains {issue}")
    duplicate_error = _public_blocker_list_duplicate_error(canary_blockers, label)
    if duplicate_error is not None:
        # Source-inventory marker: route canary blockers must not contain duplicate strings
        blockers.append(duplicate_error)
    if canary_blockers:
        # Source-inventory marker: route canary blockers must be empty
        blockers.append(f"{label} must be empty")
    return blockers


def _active_launch_route_canary_upstream_hash_roles(
    lane: dict[str, Any],
) -> tuple[tuple[str, Any], ...]:
    """Return upstream active-launch hash roles a route canary must not replay."""

    source_hashes = lane.get("source_record_hashes")
    if not isinstance(source_hashes, dict):
        source_hashes = {}
    destination_binding = lane.get("destination_binding")
    if not isinstance(destination_binding, dict):
        destination_binding = {}
    source_gate = lane.get("source_adapter_gate")
    if not isinstance(source_gate, dict):
        source_gate = {}
    route_summary = lane.get("route_allowlist")
    if not isinstance(route_summary, dict):
        route_summary = {}
    return (
        (
            "source verifier material hash",
            source_hashes.get("source_verifier_material_hash"),
        ),
        (
            "source adapter engine deployment hash",
            source_hashes.get("source_adapter_engine_deployment_hash"),
        ),
        (
            "destination binding hash",
            destination_binding.get("destination_binding_hash"),
        ),
        ("source adapter gate hash", source_gate.get("gate_hash")),
        ("route allowlist hash", route_summary.get("route_allowlist_hash")),
    )


def _active_launch_governed_deployment_metadata_blockers(
    lane_label: str,
    lane: dict[str, Any],
) -> list[str]:
    """Return active launch governed deployment metadata blockers."""

    blockers: list[str] = []
    source_hashes = lane.get("source_record_hashes")
    if not isinstance(source_hashes, dict):
        source_hashes = {}
    for field, label in (
        ("source_verifier_material_hash", "source verifier material hash"),
        (
            "source_adapter_engine_deployment_hash",
            "source adapter engine deployment hash",
        ),
    ):
        if not _is_nonzero_hex32(source_hashes.get(field)):
            blockers.append(
                f"{lane_label}: governed deployment {label} must be a canonical non-zero bytes32 hex string"
            )
    source_hash_roles = (
        (
            "source verifier material hash",
            source_hashes.get("source_verifier_material_hash"),
        ),
        (
            "source adapter engine deployment hash",
            source_hashes.get("source_adapter_engine_deployment_hash"),
        ),
    )
    blockers.extend(
        _active_launch_source_record_hash_role_blockers(
            lane_label,
            source_hashes,
            "governed deployment",
        )
    )

    destination_binding = lane.get("destination_binding")
    if not isinstance(destination_binding, dict):
        destination_binding = {}
    blockers.extend(
        _active_launch_destination_binding_blocker_container_errors(
            lane_label,
            destination_binding,
        )
    )
    supplied_hash = destination_binding.get("destination_binding_hash")
    expected_hash = destination_binding.get("expected_destination_binding_hash")
    if not _is_nonzero_hex32(supplied_hash):
        blockers.append(
            f"{lane_label}: governed deployment destination binding hash must be a canonical non-zero bytes32 hex string"
        )
    blockers.extend(
        _active_launch_hash_role_reuse_blockers(
            lane_label,
            "governed deployment",
            "destination binding hash",
            supplied_hash,
            source_hash_roles,
        )
    )
    if not _is_nonzero_hex32(expected_hash):
        blockers.append(
            f"{lane_label}: governed deployment expected destination binding hash must be a canonical non-zero bytes32 hex string"
        )
    if (
        _is_nonzero_hex32(supplied_hash)
        and _is_nonzero_hex32(expected_hash)
        and supplied_hash != expected_hash
    ):
        blockers.append(
            f"{lane_label}: governed deployment destination binding hash must match the expected canonical binding hash"
        )
    if destination_binding.get("expected_destination_binding_hash_matches") is not True:
        blockers.append(
            f"{lane_label}: governed deployment destination binding expected hash match flag must be true"
        )

    source_gate = lane.get("source_adapter_gate")
    if not isinstance(source_gate, dict):
        return blockers + [f"{lane_label}: source adapter gate summary is missing"]
    blockers.extend(
        _active_launch_source_adapter_gate_blocker_container_errors(
            lane_label,
            source_gate,
        )
    )
    if source_gate.get("ready") is not True:
        blockers.append(f"{lane_label}: source adapter gate summary must be ready")
    if source_gate.get("required") is not True:
        blockers.append(
            f"{lane_label}: active EVM source adapter gate summary must be required"
        )
    gate_hash = source_gate.get("gate_hash")
    if not _is_nonzero_hex32(gate_hash):
        blockers.append(
            f"{lane_label}: active EVM source adapter gate hash must be a canonical non-zero bytes32 hex string"
        )
    blockers.extend(
        _active_launch_hash_role_reuse_blockers(
            lane_label,
            "governed deployment",
            "source adapter gate hash",
            gate_hash,
            (
                *source_hash_roles,
                ("destination binding hash", supplied_hash),
            ),
        )
    )
    audit_hashes = source_gate.get("audit_hashes")
    if not isinstance(audit_hashes, dict):
        blockers.append(
            f"{lane_label}: active EVM source adapter gate audit hashes must be an object"
        )
    elif set(audit_hashes) != {"evm_source_gate_hash"}:
        blockers.append(
            f"{lane_label}: active EVM source adapter gate audit hashes must contain only evm_source_gate_hash"
        )
    elif audit_hashes.get("evm_source_gate_hash") != gate_hash:
        blockers.append(
            f"{lane_label}: active EVM source adapter gate hash must match audit hash evm_source_gate_hash"
        )
    return blockers


def _active_launch_source_adapter_gate_blocker_container_errors(
    lane_label: str,
    source_gate: dict[str, Any],
) -> list[str]:
    """Return blockers for copied active source-adapter gate blocker containers."""

    gate_blockers = source_gate.get("blockers", [])
    label = f"{lane_label}: source adapter gate blockers"
    if not isinstance(gate_blockers, list):
        # Source-inventory marker: source adapter gate blockers must be a list of non-empty canonical strings
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    for index, blocker in enumerate(gate_blockers):
        issue = _public_blocker_text_issue(blocker)
        if issue == "non-empty canonical string":
            blockers.append(f"{label}[{index}] must be a non-empty canonical string")
        elif issue is not None:
            blockers.append(f"{label}[{index}] contains {issue}")
    duplicate_error = _public_blocker_list_duplicate_error(gate_blockers, label)
    if duplicate_error is not None:
        # Source-inventory marker: source adapter gate blockers must not contain duplicate strings
        blockers.append(duplicate_error)
    if gate_blockers:
        # Source-inventory marker: source adapter gate blockers must be empty
        blockers.append(f"{label} must be empty")
    return blockers


def _active_launch_destination_binding_blocker_container_errors(
    lane_label: str,
    destination_binding: dict[str, Any],
) -> list[str]:
    """Return blockers for copied active destination rollout blocker containers."""

    destination_blockers = destination_binding.get("blockers", [])
    label = f"{lane_label}: destination rollout blockers"
    if not isinstance(destination_blockers, list):
        # Source-inventory marker: destination rollout blockers must be a list of non-empty canonical strings
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    for index, blocker in enumerate(destination_blockers):
        issue = _public_blocker_text_issue(blocker)
        if issue == "non-empty canonical string":
            blockers.append(f"{label}[{index}] must be a non-empty canonical string")
        elif issue is not None:
            blockers.append(f"{label}[{index}] contains {issue}")
    duplicate_error = _public_blocker_list_duplicate_error(destination_blockers, label)
    if duplicate_error is not None:
        # Source-inventory marker: destination rollout blockers must not contain duplicate strings
        blockers.append(duplicate_error)
    if destination_blockers:
        # Source-inventory marker: destination rollout blockers must be empty
        blockers.append(f"{label} must be empty")
    return blockers


def _active_launch_required_record_metadata_blockers(
    lane_label: str,
    lane: dict[str, Any],
    record_labels: dict[str, str],
) -> list[str]:
    """Return active launch record identity and presence blockers."""

    if not lane:
        return [f"{lane_label}: missing launch lane evidence"]
    blockers: list[str] = []
    if lane.get("domain") != ACTIVE_LAUNCH_DOMAIN:
        blockers.append(
            f"{lane_label}: active launch lane domain must be {ACTIVE_LAUNCH_DOMAIN}"
        )
    if lane.get("chain") != ACTIVE_LAUNCH_CHAIN:
        blockers.append(
            f"{lane_label}: active launch lane chain must be {ACTIVE_LAUNCH_CHAIN}"
        )
    if lane.get("production_ready") is not True:
        blockers.append(f"{lane_label}: active launch lane must be production ready")

    records = lane.get("records")
    if not isinstance(records, dict):
        return blockers + [f"{lane_label}: required record summary is missing"]
    for key in sorted(set(records) - set(record_labels)):
        blockers.append(_required_record_summary_unknown_field_blocker(lane_label, key))
    for key, label in record_labels.items():
        if records.get(key) is not True:
            blockers.append(f"{lane_label}: missing {label}")
    return blockers


def _active_launch_route_allowlist_binding_blockers(
    lane_label: str,
    lane: dict[str, Any],
) -> list[str]:
    """Return active launch route-allowlist binding blockers."""

    blockers: list[str] = []
    source_hashes = lane.get("source_record_hashes")
    if not isinstance(source_hashes, dict):
        source_hashes = {}
    destination_binding = lane.get("destination_binding")
    if not isinstance(destination_binding, dict):
        destination_binding = {}
    route_summary = lane.get("route_allowlist")
    if not isinstance(route_summary, dict):
        return [f"{lane_label}: route allowlist summary is missing"]

    blockers.extend(
        _active_launch_route_allowlist_blocker_container_errors(
            lane_label,
            route_summary,
        )
    )
    for field, label in (
        ("source_verifier_material_hash", "source verifier material hash"),
        (
            "source_adapter_engine_deployment_hash",
            "source adapter engine deployment hash",
        ),
    ):
        if not _is_nonzero_hex32(source_hashes.get(field)):
            blockers.append(
                f"{lane_label}: route allowlist {label} must be a canonical non-zero bytes32 hex string"
            )
    source_hash_roles = (
        (
            "source verifier material hash",
            source_hashes.get("source_verifier_material_hash"),
        ),
        (
            "source adapter engine deployment hash",
            source_hashes.get("source_adapter_engine_deployment_hash"),
        ),
    )
    blockers.extend(
        _active_launch_source_record_hash_role_blockers(
            lane_label,
            source_hashes,
            "route allowlist",
        )
    )
    if not _is_nonzero_hex32(destination_binding.get("destination_binding_hash")):
        blockers.append(
            f"{lane_label}: route allowlist destination binding hash must be a canonical non-zero bytes32 hex string"
        )

    supplied_hash = route_summary.get("route_allowlist_hash")
    expected_hash = route_summary.get("expected_route_allowlist_hash")
    if not _is_nonzero_hex32(supplied_hash):
        blockers.append(
            f"{lane_label}: route allowlist hash must be a canonical non-zero bytes32 hex string"
        )
    blockers.extend(
        _active_launch_hash_role_reuse_blockers(
            lane_label,
            "route allowlist",
            "hash",
            supplied_hash,
            (
                *source_hash_roles,
                (
                    "destination binding hash",
                    destination_binding.get("destination_binding_hash"),
                ),
            ),
        )
    )
    if not _is_nonzero_hex32(expected_hash):
        blockers.append(
            f"{lane_label}: expected route allowlist hash must be a canonical non-zero bytes32 hex string"
        )
    if (
        _is_nonzero_hex32(supplied_hash)
        and _is_nonzero_hex32(expected_hash)
        and supplied_hash != expected_hash
    ):
        blockers.append(
            f"{lane_label}: route allowlist hash must match the expected canonical source, deployment, and destination binding hash"
        )
    if route_summary.get("expected_route_allowlist_hash_matches") is not True:
        blockers.append(
            f"{lane_label}: route allowlist expected hash match flag must be true"
        )
    return blockers


def _active_launch_route_allowlist_blocker_container_errors(
    lane_label: str,
    route_summary: dict[str, Any],
) -> list[str]:
    """Return blockers for copied active route-allowlist blocker containers."""

    route_blockers = route_summary.get("blockers", [])
    label = f"{lane_label}: route allowlist blockers"
    if not isinstance(route_blockers, list):
        # Source-inventory marker: route allowlist blockers must be a list of non-empty canonical strings
        return [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    for index, blocker in enumerate(route_blockers):
        issue = _public_blocker_text_issue(blocker)
        if issue == "non-empty canonical string":
            blockers.append(f"{label}[{index}] must be a non-empty canonical string")
        elif issue is not None:
            blockers.append(f"{label}[{index}] contains {issue}")
    duplicate_error = _public_blocker_list_duplicate_error(route_blockers, label)
    if duplicate_error is not None:
        # Source-inventory marker: route allowlist blockers must not contain duplicate strings
        blockers.append(duplicate_error)
    if route_blockers:
        # Source-inventory marker: route allowlist blockers must be empty
        blockers.append(f"{label} must be empty")
    return blockers


def _active_launch_release_checklist(
    evidence: dict[str, Any],
    native_prover_bundle: dict[str, Any],
) -> dict[str, Any]:
    lane = _active_launch_lane(evidence) or {}
    lane_label = f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN})"
    lane_blockers, lane_blocker_schema_errors = (
        _active_launch_lane_blockers_for_checklist(
            lane.get("blockers"),
            lane_label,
        )
    )
    records = lane.get("records")
    if not isinstance(records, dict):
        records = {}
    record_labels = {
        "source_verifier_material": "source verifier material",
        "source_adapter_deployment": "source adapter deployment",
        "destination_rollout": "destination rollout",
        "route_allowlist": "route allowlist",
    }
    records_blockers = _active_launch_required_record_metadata_blockers(
        lane_label,
        lane,
        record_labels,
    )
    deployment_blockers = [
        f"{lane_label}: {blocker}"
        for blocker in lane_blockers
        if any(
            token in blocker
            for token in (
                "source adapter",
                "deployment",
                "destination",
                "binding",
                "verifier",
                "rollout",
            )
        )
    ]
    deployment_blockers.extend(lane_blocker_schema_errors)
    if lane:
        deployment_blockers.extend(
            _active_launch_evm_live_metadata_blockers(lane_label, lane)
        )
        deployment_blockers.extend(
            _active_launch_governed_deployment_metadata_blockers(lane_label, lane)
        )
    route_blockers = [
        f"{lane_label}: {blocker}"
        for blocker in lane_blockers
        if "route allowlist" in blocker
    ]
    route_blockers.extend(lane_blocker_schema_errors)
    if lane:
        route_blockers.extend(
            _active_launch_route_allowlist_binding_blockers(lane_label, lane)
        )
    canary_blockers = [
        f"{lane_label}: {blocker}"
        for blocker in lane_blockers
        if "route canary" in blocker
    ]
    canary_blockers.extend(lane_blocker_schema_errors)
    route_summary = lane.get("route_allowlist")
    if not isinstance(route_summary, dict):
        route_summary = {}
    canary = route_summary.get("route_canary")
    if not isinstance(canary, dict):
        canary = {}
    if canary.get("status") != "passed":
        canary_blockers.append(f"{lane_label}: route canary status is not passed")
    canary_blockers.extend(
        _active_launch_route_canary_metadata_blockers(
            lane_label,
            canary,
            _active_launch_route_canary_upstream_hash_roles(lane),
        )
    )
    if (
        "evidence_bound" in canary
        and type(canary.get("evidence_bound")) is not bool
    ):
        canary_blockers.append(
            f"{lane_label}: route canary evidence_bound must be boolean"
        )
    if canary.get("evidence_bound") is not True:
        canary_blockers.append(f"{lane_label}: route canary evidence is not bound")

    native_prover_blockers = _native_evm_validation_blockers(
        native_prover_bundle.get("validation_blockers"),
        "native EVM prover validation_blockers",
    )
    launch_blockers = _active_launch_blockers(evidence)
    items = [
        {
            "id": "all_required_lane_records",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "all_required_lane_records"
            ],
            "ready": not records_blockers,
            "blockers": records_blockers,
        },
        {
            "id": "governed_deployment_evidence",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "governed_deployment_evidence"
            ],
            "ready": not deployment_blockers,
            "blockers": deployment_blockers,
        },
        {
            "id": "route_allowlist_binding",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "route_allowlist_binding"
            ],
            "ready": not route_blockers,
            "blockers": route_blockers,
        },
        {
            "id": "live_route_canary_evidence",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "live_route_canary_evidence"
            ],
            "ready": not canary_blockers,
            "blockers": canary_blockers,
        },
        {
            "id": "native_evm_groth16_prover_bundle",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "native_evm_groth16_prover_bundle"
            ],
            "ready": not native_prover_blockers,
            "blockers": native_prover_blockers,
        },
        {
            "id": "no_unresolved_blockers",
            "title": ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[
                "no_unresolved_blockers"
            ],
            "ready": not launch_blockers,
            "blockers": launch_blockers,
        },
    ]
    return {
        "ready": all(item["ready"] is True for item in items),
        "items": items,
    }


def _input_artifacts(paths: list[Path]) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in paths:
        artifacts.append(_artifact(path))
    return artifacts


def _submission_surfaces(phase_status: dict[str, str]) -> list[dict[str, Any]]:
    surfaces: list[dict[str, Any]] = []
    for base in USER_PROVER_SUBMISSION_SURFACES:
        surface = dict(base)
        helper_symbols = list(surface["sdk_helper_symbols"])
        helper_symbols_by_sdk = {
            sdk: list(symbols)
            for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items()
        }
        required_phases = list(surface["required_phases"])
        blockers = [
            f"{phase} is {phase_status.get(phase, 'missing')}"
            for phase in required_phases
            if phase_status.get(phase) != "passed"
        ]
        surface["sdk_helper_symbols"] = helper_symbols
        surface["sdk_helper_symbols_by_sdk"] = helper_symbols_by_sdk
        surface["sdk_helpers"] = ", ".join(helper_symbols)
        surface["required_phases"] = required_phases
        surface["validation_status"] = "passed" if not blockers else "blocked"
        surface["validation_blockers"] = blockers
        surfaces.append(surface)
    return surfaces


def _release_checklist_ready_value(release_checklist: Any) -> bool:
    """Return exact release-checklist readiness without truthy coercion."""
    return (
        isinstance(release_checklist, dict)
        and release_checklist.get("ready") is True
    )


def _release_checklist_root_blockers(release_checklist: Any) -> list[str]:
    """Return schema blockers for malformed release-checklist roots."""
    if not isinstance(release_checklist, dict):
        return ["release checklist must be an object"]
    if type(release_checklist.get("ready")) is not bool:
        return ["release checklist ready must be boolean"]
    return []


def _build_report(
    paths: list[Path],
    phase_results: list[str],
    phase_evidence: list[str],
    *,
    require_phase_evidence: bool,
    phase_evidence_dir: Path | None = None,
    native_evm_prover_bundle: Path | None = None,
) -> dict[str, Any]:
    phases = _corridor_phases()
    phase_status = _parse_phase_results(phase_results, phases)
    phase_artifacts = _parse_phase_evidence(
        phase_evidence,
        phases,
        phase_status,
        phase_evidence_dir,
    )
    input_artifacts = _input_artifacts(paths)
    evidence = _load_evidence_summary(paths)
    native_prover_bundle = _native_evm_prover_bundle_status(
        native_evm_prover_bundle,
        evidence,
    )
    proof_request_bundle_gate_blockers = (
        _sccp_proof_request_bundle_gate_inventory_errors()
    )
    phase_evidence_source_gate_blockers = (
        _sccp_phase_evidence_source_gate_inventory_errors()
    )
    release_corridor_phase_transcript_gate_blockers = (
        _release_corridor_phase_transcript_gate_inventory_errors()
    )
    release_bundle_source_copy_gate_blockers = (
        _sccp_release_bundle_source_copy_gate_inventory_errors()
    )
    release_bundle_output_path_gate_blockers = (
        _sccp_release_bundle_output_path_gate_inventory_errors()
    )
    release_artifact_path_text_gate_blockers = (
        _sccp_release_artifact_path_text_gate_inventory_errors()
    )
    release_input_provenance_schema_gate_blockers = (
        _sccp_release_input_provenance_schema_gate_inventory_errors()
    )
    release_public_json_root_schema_gate_blockers = (
        _sccp_release_public_json_root_schema_gate_inventory_errors()
    )
    release_public_markdown_text_schema_gate_blockers = (
        _sccp_release_public_markdown_text_schema_gate_inventory_errors()
    )
    release_public_crypto_evidence_binding_gate_blockers = (
        _sccp_release_public_crypto_evidence_binding_gate_inventory_errors()
    )
    release_public_submission_surface_binding_gate_blockers = (
        _sccp_release_public_submission_surface_binding_gate_inventory_errors()
    )
    route_allowlist_canary_summary_gate_blockers = (
        _sccp_route_allowlist_canary_summary_gate_inventory_errors()
    )
    transparent_openverify_summary_gate_blockers = (
        _sccp_transparent_openverify_summary_gate_inventory_errors()
    )
    release_manifest_readiness_flags_gate_blockers = (
        _sccp_release_manifest_readiness_flags_gate_inventory_errors()
    )
    release_manifest_artifact_set_order_gate_blockers = (
        _sccp_release_manifest_artifact_set_order_gate_inventory_errors()
    )
    release_public_blocker_list_schema_gate_blockers = (
        _sccp_release_public_blocker_list_schema_gate_inventory_errors()
    )
    release_public_scalar_text_schema_gate_blockers = (
        _sccp_release_public_scalar_text_schema_gate_inventory_errors()
    )
    release_notes_attachment_invariants_gate_blockers = (
        _sccp_release_notes_attachment_invariants_gate_inventory_errors()
    )
    readiness_markdown_invariants_gate_blockers = (
        _sccp_readiness_markdown_invariants_gate_inventory_errors()
    )
    retired_network_surface_gate_blockers = (
        _sccp_retired_network_surface_gate_inventory_errors()
    )
    launch_scope_constant_gate_blockers = (
        _sccp_launch_scope_constant_gate_inventory_errors()
    )
    ethereum_launch_policy_selector_gate_blockers = (
        _ethereum_launch_policy_selector_gate_inventory_errors()
    )
    ethereum_launch_policy_documentation_gate_blockers = (
        _ethereum_launch_policy_documentation_gate_inventory_errors()
    )
    public_discovery_documentation_gate_blockers = (
        _sccp_public_discovery_documentation_gate_inventory_errors()
    )
    bsc_groth16_material_documentation_gate_blockers = (
        _bsc_groth16_material_documentation_gate_inventory_errors()
    )
    bsc_groth16_material_evidence_guard_gate_blockers = (
        _bsc_groth16_material_evidence_guard_gate_inventory_errors()
    )
    ethereum_data_collection_no_proxy_gate_blockers = (
        _ethereum_data_collection_no_proxy_gate_inventory_errors()
    )
    ethereum_inbound_adversarial_gate_blockers = (
        _ethereum_inbound_adversarial_gate_inventory_errors()
    )
    bsc_inbound_adversarial_gate_blockers = (
        _bsc_inbound_adversarial_gate_inventory_errors()
    )
    tron_inbound_adversarial_gate_blockers = (
        _tron_inbound_adversarial_gate_inventory_errors()
    )
    bsc_route_config_canonical_manifest_gate_blockers = (
        _bsc_route_config_canonical_manifest_gate_inventory_errors()
    )
    tron_route_config_canonical_manifest_gate_blockers = (
        _tron_route_config_canonical_manifest_gate_inventory_errors()
    )
    tron_runtime_route_manifest_gate_blockers = (
        _tron_runtime_route_manifest_gate_inventory_errors()
    )
    all_lanes_route_canary_scalar_gate_blockers = (
        _all_lanes_route_canary_scalar_gate_inventory_errors()
    )
    all_lanes_evidence_root_schema_gate_blockers = (
        _all_lanes_evidence_root_schema_gate_inventory_errors()
    )
    all_lanes_governed_blocker_schema_gate_blockers = (
        _all_lanes_governed_blocker_schema_gate_inventory_errors()
    )
    all_lanes_release_checklist_exact_boolean_gate_blockers = (
        _all_lanes_release_checklist_exact_boolean_gate_inventory_errors()
    )
    active_launch_checklist_schema_gate_blockers = (
        _active_launch_checklist_schema_gate_inventory_errors()
    )
    ethereum_outbound_precallback_gate_blockers = (
        _ethereum_outbound_precallback_gate_inventory_errors()
    )
    ethereum_outbound_provider_validation_gate_blockers = (
        _ethereum_outbound_provider_validation_gate_inventory_errors()
    )
    ethereum_local_admission_gate_blockers = (
        _ethereum_local_admission_gate_inventory_errors()
    )
    ethereum_receipt_root_zero_gate_blockers = (
        _ethereum_receipt_root_zero_gate_inventory_errors()
    )
    ethereum_receipt_rlp_zero_topic_gate_blockers = (
        _ethereum_receipt_rlp_zero_topic_gate_inventory_errors()
    )
    ethereum_receipt_rlp_zero_address_gate_blockers = (
        _ethereum_receipt_rlp_zero_address_gate_inventory_errors()
    )
    ethereum_receipt_source_event_context_gate_blockers = (
        _ethereum_receipt_source_event_context_gate_inventory_errors()
    )
    ethereum_receipt_source_event_mode_gate_blockers = (
        _ethereum_receipt_source_event_mode_gate_inventory_errors()
    )
    ethereum_receipt_source_event_zero_digest_gate_blockers = (
        _ethereum_receipt_source_event_zero_digest_gate_inventory_errors()
    )
    ethereum_receipt_rpc_duplicate_json_gate_blockers = (
        _ethereum_receipt_rpc_duplicate_json_gate_inventory_errors()
    )
    ethereum_receipt_block_transaction_hash_gate_blockers = (
        _ethereum_receipt_block_transaction_hash_gate_inventory_errors()
    )
    ethereum_js_receipt_admission_guard_gate_blockers = (
        _ethereum_js_receipt_admission_guard_gate_inventory_errors()
    )
    ethereum_sdk_receipt_metadata_guard_gate_blockers = (
        _ethereum_sdk_receipt_metadata_guard_gate_inventory_errors()
    )
    ethereum_native_receipt_finality_gate_blockers = (
        _ethereum_native_receipt_finality_gate_inventory_errors()
    )
    ethereum_noncanonical_chain_id_gate_blockers = (
        _ethereum_noncanonical_chain_id_gate_inventory_errors()
    )
    ethereum_beacon_rest_finalized_header_shape_gate_blockers = (
        _ethereum_beacon_rest_finalized_header_shape_gate_inventory_errors()
    )
    ethereum_beacon_rest_execution_payload_binding_gate_blockers = (
        _ethereum_beacon_rest_execution_payload_binding_gate_inventory_errors()
    )
    ethereum_sync_committee_roster_gate_blockers = (
        _ethereum_sync_committee_roster_gate_inventory_errors()
    )
    unready_transparent_proof_config_gate_blockers = (
        _sccp_unready_transparent_proof_config_gate_inventory_errors()
    )
    tron_deploy_operator_boolean_gate_blockers = (
        _tron_deploy_operator_boolean_gate_inventory_errors()
    )
    ethereum_source_bridge_config_gate_blockers = (
        _ethereum_source_bridge_config_gate_inventory_errors()
    )
    source_material_template_rejection_gate_blockers = (
        _sccp_source_material_template_rejection_gate_inventory_errors()
    )
    source_material_role_validation_gate_blockers = (
        _sccp_source_material_role_validation_gate_inventory_errors()
    )
    ethereum_evm_source_adapter_deployment_gate_blockers = (
        _ethereum_evm_source_adapter_deployment_gate_inventory_errors()
    )
    contract_smoke_eth_mainnet_network_id_gate_blockers = (
        _contract_smoke_eth_mainnet_network_id_gate_inventory_errors()
    )
    contract_smoke_evm_production_surface_gate_blockers = (
        _contract_smoke_evm_production_surface_gate_inventory_errors()
    )
    ethereum_core_range_finality_binding_gate_blockers = (
        _ethereum_core_range_finality_binding_gate_inventory_errors()
    )
    ethereum_core_message_replay_guard_gate_blockers = (
        _ethereum_core_message_replay_guard_gate_inventory_errors()
    )
    ethereum_torii_pinned_message_proof_gate_blockers = (
        _ethereum_torii_pinned_message_proof_gate_inventory_errors()
    )
    ethereum_evm_source_live_production_gate_blockers = (
        _ethereum_evm_source_live_production_gate_inventory_errors()
    )
    ethereum_evm_live_destination_production_gate_blockers = (
        _ethereum_evm_live_destination_production_gate_inventory_errors()
    )
    ethereum_route_canary_finalized_receipt_block_gate_blockers = (
        _ethereum_route_canary_finalized_receipt_block_gate_inventory_errors()
    )
    ethereum_evm_block_tag_metadata_gate_blockers = (
        _ethereum_evm_block_tag_metadata_gate_inventory_errors()
    )
    native_sccp_no_wasm_readiness_gate_blockers = (
        _native_sccp_no_wasm_readiness_gate_inventory_errors()
    )
    release_native_prover_bundle_schema_gate_blockers = (
        _sccp_release_native_prover_bundle_schema_gate_inventory_errors()
    )
    source_inventory = {
        "launch_scope_constant_gate": {
            "validation_status": (
                "passed" if not launch_scope_constant_gate_blockers else "blocked"
            ),
            "validation_blockers": launch_scope_constant_gate_blockers,
        },
        "ethereum_launch_policy_selector_gate": {
            "validation_status": (
                "passed"
                if not ethereum_launch_policy_selector_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_launch_policy_selector_gate_blockers,
        },
        "ethereum_launch_policy_documentation_gate": {
            "validation_status": (
                "passed"
                if not ethereum_launch_policy_documentation_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_launch_policy_documentation_gate_blockers,
        },
        "public_discovery_documentation_gate": {
            "validation_status": (
                "passed"
                if not public_discovery_documentation_gate_blockers
                else "blocked"
            ),
            "validation_blockers": public_discovery_documentation_gate_blockers,
        },
        "bsc_groth16_material_documentation_gate": {
            "validation_status": (
                "passed"
                if not bsc_groth16_material_documentation_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                bsc_groth16_material_documentation_gate_blockers
            ),
        },
        "bsc_groth16_material_evidence_guard_gate": {
            "validation_status": (
                "passed"
                if not bsc_groth16_material_evidence_guard_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                bsc_groth16_material_evidence_guard_gate_blockers
            ),
        },
        "ethereum_data_collection_no_proxy_gate": {
            "validation_status": (
                "passed"
                if not ethereum_data_collection_no_proxy_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_data_collection_no_proxy_gate_blockers,
        },
        "ethereum_inbound_adversarial_gate": {
            "validation_status": (
                "passed" if not ethereum_inbound_adversarial_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_inbound_adversarial_gate_blockers,
        },
        "bsc_inbound_adversarial_gate": {
            "validation_status": (
                "passed" if not bsc_inbound_adversarial_gate_blockers else "blocked"
            ),
            "validation_blockers": bsc_inbound_adversarial_gate_blockers,
        },
        "tron_inbound_adversarial_gate": {
            "validation_status": (
                "passed" if not tron_inbound_adversarial_gate_blockers else "blocked"
            ),
            "validation_blockers": tron_inbound_adversarial_gate_blockers,
        },
        "bsc_route_config_canonical_manifest_gate": {
            "validation_status": (
                "passed"
                if not bsc_route_config_canonical_manifest_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                bsc_route_config_canonical_manifest_gate_blockers
            ),
        },
        "tron_route_config_canonical_manifest_gate": {
            "validation_status": (
                "passed"
                if not tron_route_config_canonical_manifest_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                tron_route_config_canonical_manifest_gate_blockers
            ),
        },
        "tron_runtime_route_manifest_gate": {
            "validation_status": (
                "passed" if not tron_runtime_route_manifest_gate_blockers else "blocked"
            ),
            "validation_blockers": tron_runtime_route_manifest_gate_blockers,
        },
        "all_lanes_route_canary_scalar_gate": {
            "validation_status": (
                "passed"
                if not all_lanes_route_canary_scalar_gate_blockers
                else "blocked"
            ),
            "validation_blockers": all_lanes_route_canary_scalar_gate_blockers,
        },
        "all_lanes_evidence_root_schema_gate": {
            "validation_status": (
                "passed"
                if not all_lanes_evidence_root_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": all_lanes_evidence_root_schema_gate_blockers,
        },
        "all_lanes_governed_blocker_schema_gate": {
            "validation_status": (
                "passed"
                if not all_lanes_governed_blocker_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": all_lanes_governed_blocker_schema_gate_blockers,
        },
        "all_lanes_release_checklist_exact_boolean_gate": {
            "validation_status": (
                "passed"
                if not all_lanes_release_checklist_exact_boolean_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                all_lanes_release_checklist_exact_boolean_gate_blockers
            ),
        },
        "active_launch_checklist_schema_gate": {
            "validation_status": (
                "passed"
                if not active_launch_checklist_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": active_launch_checklist_schema_gate_blockers,
        },
        "ethereum_outbound_precallback_gate": {
            "validation_status": (
                "passed" if not ethereum_outbound_precallback_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_outbound_precallback_gate_blockers,
        },
        "ethereum_outbound_provider_validation_gate": {
            "validation_status": (
                "passed"
                if not ethereum_outbound_provider_validation_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_outbound_provider_validation_gate_blockers
            ),
        },
        "ethereum_local_admission_gate": {
            "validation_status": (
                "passed" if not ethereum_local_admission_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_local_admission_gate_blockers,
        },
        "ethereum_receipt_root_zero_gate": {
            "validation_status": (
                "passed" if not ethereum_receipt_root_zero_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_receipt_root_zero_gate_blockers,
        },
        "ethereum_receipt_rlp_zero_topic_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_rlp_zero_topic_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_receipt_rlp_zero_topic_gate_blockers,
        },
        "ethereum_receipt_rlp_zero_address_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_rlp_zero_address_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_receipt_rlp_zero_address_gate_blockers,
        },
        "ethereum_receipt_source_event_context_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_source_event_context_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_receipt_source_event_context_gate_blockers
            ),
        },
        "ethereum_receipt_source_event_mode_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_source_event_mode_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_receipt_source_event_mode_gate_blockers,
        },
        "ethereum_receipt_source_event_zero_digest_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_source_event_zero_digest_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_receipt_source_event_zero_digest_gate_blockers
            ),
        },
        "ethereum_receipt_rpc_duplicate_json_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_rpc_duplicate_json_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_receipt_rpc_duplicate_json_gate_blockers,
        },
        "ethereum_receipt_block_transaction_hash_gate": {
            "validation_status": (
                "passed"
                if not ethereum_receipt_block_transaction_hash_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_receipt_block_transaction_hash_gate_blockers
            ),
        },
        "ethereum_js_receipt_admission_guard_gate": {
            "validation_status": (
                "passed"
                if not ethereum_js_receipt_admission_guard_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_js_receipt_admission_guard_gate_blockers,
        },
        "ethereum_sdk_receipt_metadata_guard_gate": {
            "validation_status": (
                "passed"
                if not ethereum_sdk_receipt_metadata_guard_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_sdk_receipt_metadata_guard_gate_blockers,
        },
        "ethereum_native_receipt_finality_gate": {
            "validation_status": (
                "passed"
                if not ethereum_native_receipt_finality_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_native_receipt_finality_gate_blockers,
        },
        "ethereum_noncanonical_chain_id_gate": {
            "validation_status": (
                "passed" if not ethereum_noncanonical_chain_id_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_noncanonical_chain_id_gate_blockers,
        },
        "ethereum_beacon_rest_finalized_header_shape_gate": {
            "validation_status": (
                "passed"
                if not ethereum_beacon_rest_finalized_header_shape_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_beacon_rest_finalized_header_shape_gate_blockers
            ),
        },
        "ethereum_beacon_rest_execution_payload_binding_gate": {
            "validation_status": (
                "passed"
                if not ethereum_beacon_rest_execution_payload_binding_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_beacon_rest_execution_payload_binding_gate_blockers
            ),
        },
        "ethereum_sync_committee_roster_gate": {
            "validation_status": (
                "passed" if not ethereum_sync_committee_roster_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_sync_committee_roster_gate_blockers,
        },
        "ethereum_source_bridge_config_gate": {
            "validation_status": (
                "passed" if not ethereum_source_bridge_config_gate_blockers else "blocked"
            ),
            "validation_blockers": ethereum_source_bridge_config_gate_blockers,
        },
        "source_material_template_rejection_gate": {
            "validation_status": (
                "passed"
                if not source_material_template_rejection_gate_blockers
                else "blocked"
            ),
            "validation_blockers": source_material_template_rejection_gate_blockers,
        },
        "source_material_role_validation_gate": {
            "validation_status": (
                "passed"
                if not source_material_role_validation_gate_blockers
                else "blocked"
            ),
            "validation_blockers": source_material_role_validation_gate_blockers,
        },
        "ethereum_evm_source_adapter_deployment_gate": {
            "validation_status": (
                "passed"
                if not ethereum_evm_source_adapter_deployment_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_evm_source_adapter_deployment_gate_blockers,
        },
        "contract_smoke_eth_mainnet_network_id_gate": {
            "validation_status": (
                "passed"
                if not contract_smoke_eth_mainnet_network_id_gate_blockers
                else "blocked"
            ),
            "validation_blockers": contract_smoke_eth_mainnet_network_id_gate_blockers,
        },
        "contract_smoke_evm_production_surface_gate": {
            "validation_status": (
                "passed"
                if not contract_smoke_evm_production_surface_gate_blockers
                else "blocked"
            ),
            "validation_blockers": contract_smoke_evm_production_surface_gate_blockers,
        },
        "ethereum_core_range_finality_binding_gate": {
            "validation_status": (
                "passed"
                if not ethereum_core_range_finality_binding_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_core_range_finality_binding_gate_blockers,
        },
        "ethereum_core_message_replay_guard_gate": {
            "validation_status": (
                "passed"
                if not ethereum_core_message_replay_guard_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_core_message_replay_guard_gate_blockers,
        },
        "ethereum_torii_pinned_message_proof_gate": {
            "validation_status": (
                "passed"
                if not ethereum_torii_pinned_message_proof_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_torii_pinned_message_proof_gate_blockers,
        },
        "ethereum_evm_source_live_production_gate": {
            "validation_status": (
                "passed"
                if not ethereum_evm_source_live_production_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_evm_source_live_production_gate_blockers,
        },
        "ethereum_evm_live_destination_production_gate": {
            "validation_status": (
                "passed"
                if not ethereum_evm_live_destination_production_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_evm_live_destination_production_gate_blockers
            ),
        },
        "ethereum_route_canary_finalized_receipt_block_gate": {
            "validation_status": (
                "passed"
                if not ethereum_route_canary_finalized_receipt_block_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                ethereum_route_canary_finalized_receipt_block_gate_blockers
            ),
        },
        "ethereum_evm_block_tag_metadata_gate": {
            "validation_status": (
                "passed"
                if not ethereum_evm_block_tag_metadata_gate_blockers
                else "blocked"
            ),
            "validation_blockers": ethereum_evm_block_tag_metadata_gate_blockers,
        },
        "native_sccp_no_wasm_readiness_gate": {
            "validation_status": (
                "passed"
                if not native_sccp_no_wasm_readiness_gate_blockers
                else "blocked"
            ),
            "validation_blockers": native_sccp_no_wasm_readiness_gate_blockers,
        },
        "release_native_prover_bundle_schema_gate": {
            "validation_status": (
                "passed"
                if not release_native_prover_bundle_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                release_native_prover_bundle_schema_gate_blockers
            ),
        },
        "proof_request_bundle_gate": {
            "validation_status": (
                "passed" if not proof_request_bundle_gate_blockers else "blocked"
            ),
            "validation_blockers": proof_request_bundle_gate_blockers,
        },
        "phase_evidence_source_gate": {
            "validation_status": (
                "passed" if not phase_evidence_source_gate_blockers else "blocked"
            ),
            "validation_blockers": phase_evidence_source_gate_blockers,
        },
        "release_corridor_phase_transcript_gate": {
            "validation_status": (
                "passed"
                if not release_corridor_phase_transcript_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_corridor_phase_transcript_gate_blockers,
        },
        "release_bundle_source_copy_gate": {
            "validation_status": (
                "passed" if not release_bundle_source_copy_gate_blockers else "blocked"
            ),
            "validation_blockers": release_bundle_source_copy_gate_blockers,
        },
        "release_bundle_output_path_gate": {
            "validation_status": (
                "passed" if not release_bundle_output_path_gate_blockers else "blocked"
            ),
            "validation_blockers": release_bundle_output_path_gate_blockers,
        },
        "release_artifact_path_text_gate": {
            "validation_status": (
                "passed" if not release_artifact_path_text_gate_blockers else "blocked"
            ),
            "validation_blockers": release_artifact_path_text_gate_blockers,
        },
        "release_input_provenance_schema_gate": {
            "validation_status": (
                "passed"
                if not release_input_provenance_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_input_provenance_schema_gate_blockers,
        },
        "release_public_json_root_schema_gate": {
            "validation_status": (
                "passed"
                if not release_public_json_root_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_public_json_root_schema_gate_blockers,
        },
        "release_public_markdown_text_schema_gate": {
            "validation_status": (
                "passed"
                if not release_public_markdown_text_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_public_markdown_text_schema_gate_blockers,
        },
        "release_public_crypto_evidence_binding_gate": {
            "validation_status": (
                "passed"
                if not release_public_crypto_evidence_binding_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                release_public_crypto_evidence_binding_gate_blockers
            ),
        },
        "release_public_submission_surface_binding_gate": {
            "validation_status": (
                "passed"
                if not release_public_submission_surface_binding_gate_blockers
                else "blocked"
            ),
            "validation_blockers": (
                release_public_submission_surface_binding_gate_blockers
            ),
        },
        "route_allowlist_canary_summary_gate": {
            "validation_status": (
                "passed"
                if not route_allowlist_canary_summary_gate_blockers
                else "blocked"
            ),
            "validation_blockers": route_allowlist_canary_summary_gate_blockers,
        },
        "transparent_openverify_summary_gate": {
            "validation_status": (
                "passed"
                if not transparent_openverify_summary_gate_blockers
                else "blocked"
            ),
            "validation_blockers": transparent_openverify_summary_gate_blockers,
        },
        "release_manifest_readiness_flags_gate": {
            "validation_status": (
                "passed"
                if not release_manifest_readiness_flags_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_manifest_readiness_flags_gate_blockers,
        },
        "release_manifest_artifact_set_order_gate": {
            "validation_status": (
                "passed"
                if not release_manifest_artifact_set_order_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_manifest_artifact_set_order_gate_blockers,
        },
        "release_public_blocker_list_schema_gate": {
            "validation_status": (
                "passed"
                if not release_public_blocker_list_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_public_blocker_list_schema_gate_blockers,
        },
        "release_public_scalar_text_schema_gate": {
            "validation_status": (
                "passed"
                if not release_public_scalar_text_schema_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_public_scalar_text_schema_gate_blockers,
        },
        "release_notes_attachment_invariants_gate": {
            "validation_status": (
                "passed"
                if not release_notes_attachment_invariants_gate_blockers
                else "blocked"
            ),
            "validation_blockers": release_notes_attachment_invariants_gate_blockers,
        },
        "readiness_markdown_invariants_gate": {
            "validation_status": (
                "passed"
                if not readiness_markdown_invariants_gate_blockers
                else "blocked"
            ),
            "validation_blockers": readiness_markdown_invariants_gate_blockers,
        },
        "retired_network_surface_gate": {
            "validation_status": (
                "passed" if not retired_network_surface_gate_blockers else "blocked"
            ),
            "validation_blockers": retired_network_surface_gate_blockers,
        },
        "unready_transparent_proof_config_gate": {
            "validation_status": (
                "passed"
                if not unready_transparent_proof_config_gate_blockers
                else "blocked"
            ),
            "validation_blockers": unready_transparent_proof_config_gate_blockers,
        },
        "tron_deploy_operator_boolean_gate": {
            "validation_status": (
                "passed"
                if not tron_deploy_operator_boolean_gate_blockers
                else "blocked"
            ),
            "validation_blockers": tron_deploy_operator_boolean_gate_blockers,
        },
    }
    release_checklist = _active_launch_release_checklist(evidence, native_prover_bundle)
    release_checklist_root_blockers = _release_checklist_root_blockers(
        release_checklist
    )
    failed_phases = [
        phase for phase, status in phase_status.items() if status != "passed"
    ]
    missing_phase_evidence = [
        phase
        for phase, status in phase_status.items()
        if require_phase_evidence and status == "passed" and phase not in phase_artifacts
    ]
    invalid_phase_evidence: dict[str, list[str]] = {
        phase: errors
        for phase, artifact in phase_artifacts.items()
        if phase_status.get(phase) == "passed"
        for errors in [_phase_transcript_errors(phase, artifact)]
        if errors
    }
    corridor_ready = (
        not failed_phases
        and not missing_phase_evidence
        and not invalid_phase_evidence
    )
    production_ready = (
        _release_checklist_ready_value(release_checklist)
        and not release_checklist_root_blockers
        and corridor_ready
        and not launch_scope_constant_gate_blockers
        and not ethereum_launch_policy_selector_gate_blockers
        and not ethereum_launch_policy_documentation_gate_blockers
        and not public_discovery_documentation_gate_blockers
        and not bsc_groth16_material_documentation_gate_blockers
        and not bsc_groth16_material_evidence_guard_gate_blockers
        and not ethereum_data_collection_no_proxy_gate_blockers
        and not ethereum_inbound_adversarial_gate_blockers
        and not bsc_inbound_adversarial_gate_blockers
        and not tron_inbound_adversarial_gate_blockers
        and not bsc_route_config_canonical_manifest_gate_blockers
        and not tron_route_config_canonical_manifest_gate_blockers
        and not tron_runtime_route_manifest_gate_blockers
        and not all_lanes_route_canary_scalar_gate_blockers
        and not all_lanes_evidence_root_schema_gate_blockers
        and not all_lanes_governed_blocker_schema_gate_blockers
        and not all_lanes_release_checklist_exact_boolean_gate_blockers
        and not active_launch_checklist_schema_gate_blockers
        and not ethereum_outbound_precallback_gate_blockers
        and not ethereum_outbound_provider_validation_gate_blockers
        and not ethereum_local_admission_gate_blockers
        and not ethereum_receipt_root_zero_gate_blockers
        and not ethereum_receipt_rlp_zero_topic_gate_blockers
        and not ethereum_receipt_rlp_zero_address_gate_blockers
        and not ethereum_receipt_source_event_context_gate_blockers
        and not ethereum_receipt_source_event_mode_gate_blockers
        and not ethereum_receipt_source_event_zero_digest_gate_blockers
        and not ethereum_receipt_rpc_duplicate_json_gate_blockers
        and not ethereum_receipt_block_transaction_hash_gate_blockers
        and not ethereum_js_receipt_admission_guard_gate_blockers
        and not ethereum_sdk_receipt_metadata_guard_gate_blockers
        and not ethereum_native_receipt_finality_gate_blockers
        and not ethereum_noncanonical_chain_id_gate_blockers
        and not ethereum_beacon_rest_finalized_header_shape_gate_blockers
        and not ethereum_beacon_rest_execution_payload_binding_gate_blockers
        and not ethereum_sync_committee_roster_gate_blockers
        and not ethereum_source_bridge_config_gate_blockers
        and not source_material_template_rejection_gate_blockers
        and not source_material_role_validation_gate_blockers
        and not ethereum_evm_source_adapter_deployment_gate_blockers
        and not contract_smoke_eth_mainnet_network_id_gate_blockers
        and not contract_smoke_evm_production_surface_gate_blockers
        and not ethereum_core_range_finality_binding_gate_blockers
        and not ethereum_core_message_replay_guard_gate_blockers
        and not ethereum_torii_pinned_message_proof_gate_blockers
        and not ethereum_evm_source_live_production_gate_blockers
        and not ethereum_evm_live_destination_production_gate_blockers
        and not ethereum_route_canary_finalized_receipt_block_gate_blockers
        and not ethereum_evm_block_tag_metadata_gate_blockers
        and not native_sccp_no_wasm_readiness_gate_blockers
        and not release_native_prover_bundle_schema_gate_blockers
        and not proof_request_bundle_gate_blockers
        and not phase_evidence_source_gate_blockers
        and not release_corridor_phase_transcript_gate_blockers
        and not release_bundle_source_copy_gate_blockers
        and not release_bundle_output_path_gate_blockers
        and not release_artifact_path_text_gate_blockers
        and not release_input_provenance_schema_gate_blockers
        and not release_public_json_root_schema_gate_blockers
        and not release_public_markdown_text_schema_gate_blockers
        and not release_public_crypto_evidence_binding_gate_blockers
        and not release_public_submission_surface_binding_gate_blockers
        and not route_allowlist_canary_summary_gate_blockers
        and not transparent_openverify_summary_gate_blockers
        and not release_manifest_readiness_flags_gate_blockers
        and not release_manifest_artifact_set_order_gate_blockers
        and not release_public_blocker_list_schema_gate_blockers
        and not release_public_scalar_text_schema_gate_blockers
        and not release_notes_attachment_invariants_gate_blockers
        and not readiness_markdown_invariants_gate_blockers
        and not retired_network_surface_gate_blockers
        and not unready_transparent_proof_config_gate_blockers
        and not tron_deploy_operator_boolean_gate_blockers
    )
    blockers = _active_launch_blockers(evidence)
    blockers.extend(release_checklist_root_blockers)
    blockers.extend(
        _native_evm_validation_blockers(
            native_prover_bundle.get("validation_blockers"),
            "native EVM prover validation_blockers",
        )
    )
    blockers.extend(launch_scope_constant_gate_blockers)
    blockers.extend(ethereum_launch_policy_selector_gate_blockers)
    blockers.extend(ethereum_launch_policy_documentation_gate_blockers)
    blockers.extend(public_discovery_documentation_gate_blockers)
    blockers.extend(bsc_groth16_material_documentation_gate_blockers)
    blockers.extend(bsc_groth16_material_evidence_guard_gate_blockers)
    blockers.extend(ethereum_data_collection_no_proxy_gate_blockers)
    blockers.extend(ethereum_inbound_adversarial_gate_blockers)
    blockers.extend(bsc_inbound_adversarial_gate_blockers)
    blockers.extend(tron_inbound_adversarial_gate_blockers)
    blockers.extend(bsc_route_config_canonical_manifest_gate_blockers)
    blockers.extend(tron_route_config_canonical_manifest_gate_blockers)
    blockers.extend(tron_runtime_route_manifest_gate_blockers)
    blockers.extend(all_lanes_route_canary_scalar_gate_blockers)
    blockers.extend(all_lanes_evidence_root_schema_gate_blockers)
    blockers.extend(all_lanes_governed_blocker_schema_gate_blockers)
    blockers.extend(all_lanes_release_checklist_exact_boolean_gate_blockers)
    blockers.extend(active_launch_checklist_schema_gate_blockers)
    blockers.extend(ethereum_outbound_precallback_gate_blockers)
    blockers.extend(ethereum_outbound_provider_validation_gate_blockers)
    blockers.extend(ethereum_local_admission_gate_blockers)
    blockers.extend(ethereum_receipt_root_zero_gate_blockers)
    blockers.extend(ethereum_receipt_rlp_zero_topic_gate_blockers)
    blockers.extend(ethereum_receipt_rlp_zero_address_gate_blockers)
    blockers.extend(ethereum_receipt_source_event_context_gate_blockers)
    blockers.extend(ethereum_receipt_source_event_mode_gate_blockers)
    blockers.extend(ethereum_receipt_source_event_zero_digest_gate_blockers)
    blockers.extend(ethereum_receipt_rpc_duplicate_json_gate_blockers)
    blockers.extend(ethereum_receipt_block_transaction_hash_gate_blockers)
    blockers.extend(ethereum_js_receipt_admission_guard_gate_blockers)
    blockers.extend(ethereum_sdk_receipt_metadata_guard_gate_blockers)
    blockers.extend(ethereum_native_receipt_finality_gate_blockers)
    blockers.extend(ethereum_noncanonical_chain_id_gate_blockers)
    blockers.extend(ethereum_beacon_rest_finalized_header_shape_gate_blockers)
    blockers.extend(ethereum_beacon_rest_execution_payload_binding_gate_blockers)
    blockers.extend(ethereum_sync_committee_roster_gate_blockers)
    blockers.extend(ethereum_source_bridge_config_gate_blockers)
    blockers.extend(source_material_template_rejection_gate_blockers)
    blockers.extend(source_material_role_validation_gate_blockers)
    blockers.extend(ethereum_evm_source_adapter_deployment_gate_blockers)
    blockers.extend(contract_smoke_eth_mainnet_network_id_gate_blockers)
    blockers.extend(contract_smoke_evm_production_surface_gate_blockers)
    blockers.extend(ethereum_core_range_finality_binding_gate_blockers)
    blockers.extend(ethereum_core_message_replay_guard_gate_blockers)
    blockers.extend(ethereum_torii_pinned_message_proof_gate_blockers)
    blockers.extend(ethereum_evm_source_live_production_gate_blockers)
    blockers.extend(ethereum_evm_live_destination_production_gate_blockers)
    blockers.extend(ethereum_route_canary_finalized_receipt_block_gate_blockers)
    blockers.extend(ethereum_evm_block_tag_metadata_gate_blockers)
    blockers.extend(native_sccp_no_wasm_readiness_gate_blockers)
    blockers.extend(release_native_prover_bundle_schema_gate_blockers)
    blockers.extend(proof_request_bundle_gate_blockers)
    blockers.extend(phase_evidence_source_gate_blockers)
    blockers.extend(release_corridor_phase_transcript_gate_blockers)
    blockers.extend(release_bundle_source_copy_gate_blockers)
    blockers.extend(release_bundle_output_path_gate_blockers)
    blockers.extend(release_artifact_path_text_gate_blockers)
    blockers.extend(release_input_provenance_schema_gate_blockers)
    blockers.extend(release_public_json_root_schema_gate_blockers)
    blockers.extend(release_public_markdown_text_schema_gate_blockers)
    blockers.extend(release_public_crypto_evidence_binding_gate_blockers)
    blockers.extend(release_public_submission_surface_binding_gate_blockers)
    blockers.extend(route_allowlist_canary_summary_gate_blockers)
    blockers.extend(transparent_openverify_summary_gate_blockers)
    blockers.extend(release_manifest_readiness_flags_gate_blockers)
    blockers.extend(release_manifest_artifact_set_order_gate_blockers)
    blockers.extend(release_public_blocker_list_schema_gate_blockers)
    blockers.extend(release_public_scalar_text_schema_gate_blockers)
    blockers.extend(release_notes_attachment_invariants_gate_blockers)
    blockers.extend(readiness_markdown_invariants_gate_blockers)
    blockers.extend(retired_network_surface_gate_blockers)
    blockers.extend(unready_transparent_proof_config_gate_blockers)
    blockers.extend(tron_deploy_operator_boolean_gate_blockers)
    blockers.extend(
        f"production corridor phase {phase} is {phase_status[phase]}"
        for phase in failed_phases
    )
    blockers.extend(
        f"production corridor phase {phase} has no hashed evidence artifact"
        for phase in missing_phase_evidence
    )
    blockers.extend(
        f"production corridor phase {phase} {error}"
        for phase, errors in invalid_phase_evidence.items()
        for error in errors
    )
    return {
        "production_ready": production_ready,
        "evidence": evidence,
        "release_checklist": release_checklist,
        "corridor": {
            "production_ready": corridor_ready,
            "phases": phase_status,
            "evidence_artifacts": phase_artifacts,
            "require_phase_evidence": require_phase_evidence,
            "blockers": [
                f"{phase} is {phase_status[phase]}" for phase in failed_phases
            ]
            + [
                f"{phase} has no hashed evidence artifact"
                for phase in missing_phase_evidence
            ]
            + [
                f"{phase} {error}"
                for phase, errors in invalid_phase_evidence.items()
                for error in errors
            ],
        },
        "blockers": blockers,
        "inputs": [str(path) for path in paths],
        "input_artifacts": input_artifacts,
        "native_evm_prover_bundle": native_prover_bundle,
        "source_inventory": source_inventory,
        "cryptographic_evidence": _cryptographic_evidence(evidence),
        "user_prover_submission_surfaces": _submission_surfaces(phase_status),
    }


def _record_flags(records: Any) -> str:
    if not isinstance(records, dict):
        records = {}
    labels = {
        "source_verifier_material": "source",
        "source_adapter_deployment": "deploy",
        "destination_rollout": "dest",
        "route_allowlist": "route",
    }
    return ", ".join(
        f"{label}={'yes' if records.get(field) is True else 'no'}"
        for field, label in labels.items()
    )


def _lane_readiness_markdown_cells(
    lane: Any,
) -> tuple[str, str, str, str, Any]:
    if not isinstance(lane, dict):
        return "-", "-", "blocked", _record_flags({}), [
            "lane summary must be an object"
        ]

    domain = lane.get("domain")
    chain = lane.get("chain")
    domain_cell = str(domain) if type(domain) is int else "-"
    chain_cell = (
        f"`{chain}`"
        if type(domain) is int and chain == ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
        else "-"
    )
    lane_status = "ready" if lane.get("production_ready") is True else "blocked"
    return (
        domain_cell,
        chain_cell,
        lane_status,
        _record_flags(lane.get("records")),
        lane.get("blockers"),
    )


def _cryptographic_evidence(evidence: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for lane in evidence.get("lanes", []):
        source_hashes = lane.get("source_record_hashes")
        if not isinstance(source_hashes, dict):
            source_hashes = {}
        destination_binding = lane.get("destination_binding")
        if not isinstance(destination_binding, dict):
            destination_binding = {}
        route_allowlist = lane.get("route_allowlist")
        if not isinstance(route_allowlist, dict):
            route_allowlist = {}
        route_canary = route_allowlist.get("route_canary")
        if not isinstance(route_canary, dict):
            route_canary = {}
        route_canary_evidence_bound = route_canary.get("evidence_bound")
        if route_canary_evidence_bound is None:
            route_canary_evidence_bound = False
        source_gate = lane.get("source_adapter_gate")
        if not isinstance(source_gate, dict):
            source_gate = {}
        source_gate_audit_hashes = source_gate.get("audit_hashes")
        evm_live_metadata = lane.get("evm_live_metadata")
        if not isinstance(evm_live_metadata, dict):
            evm_live_metadata = {}
        rows.append(
            {
                "domain": lane.get("domain"),
                "chain": lane.get("chain"),
                "evm_source_rpc_chain_id": evm_live_metadata.get(
                    "source_rpc_chain_id",
                    "",
                ),
                "evm_source_block_tag": evm_live_metadata.get("source_block_tag", ""),
                "evm_destination_rpc_chain_id": evm_live_metadata.get(
                    "destination_rpc_chain_id",
                    "",
                ),
                "evm_destination_block_tag": evm_live_metadata.get(
                    "destination_block_tag",
                    "",
                ),
                "source_verifier_material_hash": source_hashes.get(
                    "source_verifier_material_hash"
                ),
                "source_adapter_engine_deployment_hash": source_hashes.get(
                    "source_adapter_engine_deployment_hash"
                ),
                "destination_binding_hash": destination_binding.get(
                    "destination_binding_hash"
                ),
                "route_allowlist_hash": route_allowlist.get("route_allowlist_hash"),
                "route_canary_evidence_hash": route_canary.get("evidence_hash"),
                "route_canary_evidence_source": route_canary.get("evidence_source"),
                "route_canary_evidence_bound": route_canary_evidence_bound,
                "route_canary_message_proof_used": route_canary.get(
                    "message_proof_used"
                ),
                "route_canary_raw_data_owner_matches_transaction": route_canary.get(
                    "raw_data_owner_matches_transaction"
                ),
                "route_canary_signature_recovers_to_owner": route_canary.get(
                    "signature_recovers_to_owner"
                ),
                "route_canary_log_index": route_canary.get("log_index"),
                "route_canary_target_domain": route_canary.get("target_domain"),
                "route_canary_proof_version": route_canary.get("proof_version"),
                "route_canary_proof_source_domain": route_canary.get(
                    "proof_source_domain"
                ),
                "route_canary_call_data_sha256": route_canary.get(
                    "call_data_sha256"
                ),
                "route_canary_payload_hash": route_canary.get("payload_hash"),
                "route_canary_statement_hash": route_canary.get("statement_hash"),
                "route_canary_commitment_root": route_canary.get("commitment_root"),
                "route_canary_finality_height": route_canary.get("finality_height"),
                "route_canary_finality_block_hash": route_canary.get(
                    "finality_block_hash"
                ),
                "route_canary_transaction_hash": route_canary.get("transaction_hash"),
                "route_canary_receipt_block_number": route_canary.get(
                    "receipt_block_number"
                ),
                "route_canary_receipt_block_hash": route_canary.get(
                    "receipt_block_hash"
                ),
                "route_canary_receipt_block_finalized": route_canary.get(
                    "receipt_block_finalized"
                ),
                "route_canary_block_receipts_root": route_canary.get(
                    "block_receipts_root"
                ),
                "route_canary_message_id": route_canary.get("message_id"),
                "route_canary_block_number": route_canary.get("block_number"),
                "route_canary_block_timestamp": route_canary.get("block_timestamp"),
                "source_adapter_gate_required": source_gate.get("required"),
                "source_adapter_gate_hash": source_gate.get("gate_hash", ""),
                "source_adapter_gate_audit_hashes": (
                    dict(sorted(source_gate_audit_hashes.items()))
                    if isinstance(source_gate_audit_hashes, dict)
                    else source_gate_audit_hashes
                ),
            }
        )
    return rows


def _hash_cell(value: Any) -> str:
    if _is_nonzero_hex32(value):
        return f"`{value}`"
    return "-"


def _markdown_text_cell(value: Any) -> str:
    if (
        isinstance(value, str)
        and _public_blocker_text_issue(value) is None
        and not any(character.isspace() for character in value)
    ):
        return f"`{value}`"
    return "`-`"


def _audit_hashes_cell(value: Any) -> str:
    if not isinstance(value, dict) or not value:
        return "-"
    rows: list[str] = []
    for key, audit_hash in sorted(value.items(), key=lambda item: str(item[0])):
        if (
            not isinstance(key, str)
            or _public_blocker_text_issue(key) is not None
            or not _is_nonzero_hex32(audit_hash)
        ):
            return "`<invalid source_adapter_gate_audit_hashes>`"
        rows.append(f"`{key}`: `{audit_hash}`")
    return "<br>".join(rows) if rows else "-"


def _integer_cell(value: Any) -> str:
    if type(value) is int:
        return f"`{value}`"
    return "-"


def _boolean_cell(value: Any) -> str:
    if type(value) is bool:
        return "`true`" if value else "`false`"
    return "-"


def _cryptographic_evidence_markdown_row_cells(row: Any) -> list[str]:
    if not isinstance(row, dict):
        row = {}
    domain = row.get("domain")
    chain = row.get("chain")
    domain_cell = str(domain) if type(domain) is int else "-"
    chain_cell = (
        f"`{chain}`"
        if type(domain) is int and chain == ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
        else "-"
    )
    canary_source = row.get("route_canary_evidence_source")
    safe_canary_source = (
        canary_source
        if isinstance(canary_source, str)
        and _public_blocker_text_issue(canary_source) is None
        else "-"
    )
    if row.get("route_canary_evidence_bound") is not True:
        safe_canary_source = f"{safe_canary_source} (unbound)"
    source_gate = _hash_cell(row.get("source_adapter_gate_hash"))
    if row.get("source_adapter_gate_required") is False and source_gate == "-":
        source_gate = "not required"
    return [
        domain_cell,
        chain_cell,
        _markdown_text_cell(row.get("evm_source_rpc_chain_id")),
        _markdown_text_cell(row.get("evm_source_block_tag")),
        _markdown_text_cell(row.get("evm_destination_rpc_chain_id")),
        _markdown_text_cell(row.get("evm_destination_block_tag")),
        _hash_cell(row.get("source_verifier_material_hash")),
        _hash_cell(row.get("source_adapter_engine_deployment_hash")),
        _hash_cell(row.get("destination_binding_hash")),
        source_gate,
        _audit_hashes_cell(row.get("source_adapter_gate_audit_hashes")),
        _hash_cell(row.get("route_allowlist_hash")),
        _hash_cell(row.get("route_canary_evidence_hash")),
        f"`{safe_canary_source}`",
        _boolean_cell(row.get("route_canary_message_proof_used")),
        _boolean_cell(row.get("route_canary_raw_data_owner_matches_transaction")),
        _boolean_cell(row.get("route_canary_signature_recovers_to_owner")),
        _integer_cell(row.get("route_canary_log_index")),
        _integer_cell(row.get("route_canary_target_domain")),
        _integer_cell(row.get("route_canary_proof_version")),
        _integer_cell(row.get("route_canary_proof_source_domain")),
        _hash_cell(row.get("route_canary_call_data_sha256")),
        _hash_cell(row.get("route_canary_payload_hash")),
        _hash_cell(row.get("route_canary_statement_hash")),
        _hash_cell(row.get("route_canary_commitment_root")),
        _hash_cell(row.get("route_canary_finality_height")),
        _hash_cell(row.get("route_canary_finality_block_hash")),
        _hash_cell(row.get("route_canary_transaction_hash")),
        _integer_cell(row.get("route_canary_receipt_block_number")),
        _hash_cell(row.get("route_canary_receipt_block_hash")),
        _boolean_cell(row.get("route_canary_receipt_block_finalized")),
        _hash_cell(row.get("route_canary_block_receipts_root")),
        _hash_cell(row.get("route_canary_message_id")),
        _integer_cell(row.get("route_canary_block_number")),
        _integer_cell(row.get("route_canary_block_timestamp")),
    ]


def _helper_symbol_is_markdown_safe(symbol: Any) -> bool:
    if not isinstance(symbol, str) or not symbol:
        return False
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in symbol):
        return False
    if not symbol.isascii() or symbol.strip() != symbol:
        return False
    if any(character.isspace() for character in symbol):
        return False
    if any(character in "|`<>" for character in symbol):
        return False
    allowed = set(
        "abcdefghijklmnopqrstuvwxyz"
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        "0123456789"
        "._:()"
    )
    return symbol[0].isalpha() and all(character in allowed for character in symbol)


def _sdk_helper_sets_cell(surface: Any) -> str:
    if not isinstance(surface, dict):
        return "`<invalid sdk_helper_symbols_by_sdk>`"
    lanes = surface.get("lanes")
    expected_helper_sets = (
        USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK.get(lanes)
        if isinstance(lanes, str)
        else None
    )
    if expected_helper_sets is None:
        return "`<invalid sdk_helper_symbols_by_sdk>`"
    helper_sets = surface.get("sdk_helper_symbols_by_sdk")
    if not isinstance(helper_sets, dict):
        return "`<invalid sdk_helper_symbols_by_sdk>`"
    if set(helper_sets) != set(expected_helper_sets):
        return "`<invalid sdk_helper_symbols_by_sdk>`"
    rows: list[str] = []
    for sdk, expected_helpers in expected_helper_sets.items():
        helpers = helper_sets.get(sdk)
        if not isinstance(helpers, list) or tuple(helpers) != expected_helpers:
            return "`<invalid sdk_helper_symbols_by_sdk>`"
        if any(not _helper_symbol_is_markdown_safe(helper) for helper in helpers):
            return "`<invalid sdk_helper_symbols_by_sdk>`"
        helper_text = ", ".join(f"`{helper}`" for helper in helpers)
        rows.append(f"`{sdk}`: {helper_text}")
    return "<br>".join(rows) if rows else "`<invalid sdk_helper_symbols_by_sdk>`"


def _markdown_string_list_cell(value: Any, *, field_label: str) -> str:
    if not isinstance(value, list):
        return f"`<invalid {field_label}>`"
    if not value:
        return "-"
    if not all(isinstance(item, str) and item for item in value):
        return f"`<invalid {field_label}>`"
    if any(_public_blocker_text_issue(item) is not None for item in value):
        return f"`<invalid {field_label}>`"
    if _public_blocker_list_duplicate_error(value, field_label) is not None:
        return f"`<invalid {field_label}>`"
    return "<br>".join(value)


def _user_prover_validation_blockers_cell(value: Any) -> str:
    field_label = "validation_blockers"
    if not isinstance(value, list):
        return f"`<invalid {field_label}>`"
    if not value:
        return "-"
    if not all(isinstance(item, str) and item for item in value):
        return f"`<invalid {field_label}>`"
    for item in value:
        if _path_control_character(item) is not None:
            return f"`<invalid {field_label}>`"
        if not item.isascii() or item.strip() != item:
            return f"`<invalid {field_label}>`"
        if _path_markdown_unsafe_character(item) is not None:
            return f"`<invalid {field_label}>`"
        if any(marker in item.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
            return f"`<invalid {field_label}>`"
    if _public_blocker_list_duplicate_error(value, field_label) is not None:
        return f"`<invalid {field_label}>`"
    return "<br>".join(value)


def _user_prover_surface_code_cell(
    value: Any,
    *,
    field_label: str,
    expected: str | None,
) -> str:
    if (
        isinstance(value, str)
        and expected is not None
        and value == expected
        and _public_blocker_text_issue(value) is None
        and not any(character.isspace() for character in value)
    ):
        return f"`{value}`"
    return f"`<invalid {field_label}>`"


def _user_prover_surface_text_cell(
    value: Any,
    *,
    field_label: str,
    expected: str | None,
) -> str:
    if (
        isinstance(value, str)
        and expected is not None
        and value == expected
        and _public_blocker_text_issue(value) is None
    ):
        return value
    return f"`<invalid {field_label}>`"


def _user_prover_surface_required_phases_cell(surface: dict[str, Any]) -> str:
    lanes = surface.get("lanes")
    expected_phases = (
        USER_PROVER_REQUIRED_PHASES_BY_LANE.get(lanes)
        if isinstance(lanes, str)
        else None
    )
    required_phases = surface.get("required_phases")
    if (
        expected_phases is not None
        and isinstance(required_phases, list)
        and tuple(required_phases) == expected_phases
        and all(
            isinstance(phase, str)
            and _public_blocker_text_issue(phase) is None
            and not any(character.isspace() for character in phase)
            for phase in required_phases
        )
    ):
        return ", ".join(f"`{phase}`" for phase in required_phases)
    return "`<invalid required_phases>`"


def _user_prover_surface_validation_cell(surface: dict[str, Any]) -> str:
    validation_status = surface.get("validation_status")
    validation = validation_status if validation_status in {"passed", "blocked"} else "blocked"
    validation_issues: list[str] = []
    if validation_status not in {"passed", "blocked"}:
        validation_issues.append("`<invalid validation_status>`")
    validation_blockers = surface.get("validation_blockers")
    if not isinstance(validation_blockers, list) or validation_blockers:
        validation_issues.append(
            _user_prover_validation_blockers_cell(validation_blockers)
        )
    if validation_issues:
        validation += ": " + "<br>".join(validation_issues)
    return validation


def _user_prover_surface_markdown_row_cells(surface: Any) -> list[str]:
    if not isinstance(surface, dict):
        return [
            "`<invalid lanes>`",
            "`<invalid proof_backend>`",
            "`<invalid sdk_helper_symbols_by_sdk>`",
            "`<invalid on_chain_submission>`",
            "`<invalid required_phases>`",
            "blocked: submission surface must be an object",
        ]
    lanes = surface.get("lanes")
    expected_backend = (
        USER_PROVER_REQUIRED_LANE_BACKENDS.get(lanes)
        if isinstance(lanes, str)
        else None
    )
    expected_submission = (
        USER_PROVER_ON_CHAIN_SUBMISSION_BY_LANE.get(lanes)
        if isinstance(lanes, str)
        else None
    )
    return [
        _user_prover_surface_code_cell(
            lanes,
            field_label="lanes",
            expected=(
                lanes
                if isinstance(lanes, str)
                and lanes in USER_PROVER_REQUIRED_LANE_BACKENDS
                else None
            ),
        ),
        _user_prover_surface_code_cell(
            surface.get("proof_backend"),
            field_label="proof_backend",
            expected=expected_backend,
        ),
        _sdk_helper_sets_cell(surface),
        _user_prover_surface_text_cell(
            surface.get("on_chain_submission"),
            field_label="on_chain_submission",
            expected=expected_submission,
        ),
        _user_prover_surface_required_phases_cell(surface),
        _user_prover_surface_validation_cell(surface),
    ]


def _native_evm_validation_blockers_cell(value: Any) -> str:
    field_label = "validation_blockers"
    if not isinstance(value, list):
        return f"`<invalid {field_label}>`"
    if not value:
        return "-"
    for index, item in enumerate(value):
        if (
            _native_evm_validation_blocker_issue(
                item,
                "native EVM prover validation_blockers",
                index,
            )
            is not None
        ):
            return f"`<invalid {field_label}>`"
    if _public_blocker_list_duplicate_error(value, field_label) is not None:
        return f"`<invalid {field_label}>`"
    return "<br>".join(value)


def _native_evm_markdown_path_is_safe(value: Any) -> bool:
    if (
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or not value.isascii()
        or _path_control_character(value) is not None
        or _path_markdown_unsafe_character(value) is not None
        or _path_percent_encoded_traversal(value) is not None
        or "\\" in value
        or ":" in value
        or any(
            marker in value.lower()
            for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
        )
    ):
        return False
    path = PurePosixPath(value)
    return (
        bool(path.parts)
        and not path.is_absolute()
        and ".." not in path.parts
        and value == path.as_posix()
    )


def _native_evm_markdown_text_is_safe(value: Any) -> bool:
    return (
        isinstance(value, str)
        and _public_blocker_text_issue(value) is None
        and not any(character.isspace() for character in value)
    )


def _native_evm_artifact_path_cell(
    artifact: Any,
    *,
    field_label: str,
) -> str:
    if not isinstance(artifact, dict):
        return "-"
    artifact_path = artifact.get("path")
    if _native_evm_markdown_path_is_safe(artifact_path):
        return f"`{artifact_path}`"
    return f"`<invalid {field_label}>`"


def _native_evm_artifact_hash_cell(
    artifact: Any,
    *,
    field_label: str,
) -> str:
    if not isinstance(artifact, dict):
        return "-"
    artifact_hash = artifact.get("sha256")
    if _is_nonzero_canonical_sha256_text(artifact_hash):
        return f"`{artifact_hash}`"
    return f"`<invalid {field_label}.sha256>`"


def _native_evm_hex32_cell(value: Any, *, field_label: str) -> str:
    if value is None:
        return "-"
    if _is_nonzero_hex32(value):
        return f"`{value}`"
    return f"`<invalid {field_label}>`"


def _native_evm_support_artifact_cell(
    artifact: Any,
    *,
    field_label: str,
) -> str:
    if not isinstance(artifact, dict):
        return "-"
    artifact_path = artifact.get("path")
    artifact_hash = artifact.get("sha256")
    if (
        _native_evm_markdown_path_is_safe(artifact_path)
        and _is_nonzero_canonical_sha256_text(artifact_hash)
    ):
        return f"`{artifact_path}`<br>`{artifact_hash}`"
    return f"`<invalid {field_label}>`"


def _native_evm_sdk_artifacts_cell(value: Any) -> str:
    if not isinstance(value, list) or not value:
        return "-"
    by_sdk: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict):
            return "`<invalid sdk_artifacts>`"
        sdk = row.get("sdk")
        if (
            not isinstance(sdk, str)
            or sdk in by_sdk
            or sdk not in NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS
        ):
            return "`<invalid sdk_artifacts>`"
        by_sdk[sdk] = row
    if set(by_sdk) != set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS):
        return "`<invalid sdk_artifacts>`"
    rows: list[str] = []
    for sdk in sorted(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS):
        row = by_sdk[sdk]
        implementation = row.get("implementation")
        expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS[sdk]
        implementation_hash = row.get("implementation_hash")
        if (
            implementation != expected_implementation
            or not _native_evm_markdown_text_is_safe(sdk)
            or not _native_evm_markdown_text_is_safe(implementation)
            or not _is_nonzero_hex32(implementation_hash)
        ):
            return "`<invalid sdk_artifacts>`"
        rows.append(f"`{sdk}`: `{implementation}` `{implementation_hash}`")
    return "<br>".join(rows)


def _native_evm_bundle_status_cell(native_bundle: dict[str, Any]) -> str:
    status = native_bundle.get("validation_status")
    return status if status in {"passed", "blocked"} else "blocked"


def _native_evm_bundle_blockers_cell(native_bundle: dict[str, Any]) -> str:
    blockers = _native_evm_validation_blockers_cell(
        native_bundle.get("validation_blockers")
    )
    if native_bundle.get("validation_status") in {"passed", "blocked"}:
        return blockers
    invalid_status = "`<invalid validation_status>`"
    if blockers == "-":
        return invalid_status
    return invalid_status + "<br>" + blockers


def _native_evm_bundle_markdown_row_cells(native_bundle: Any) -> list[str]:
    if not isinstance(native_bundle, dict):
        return [
            "no",
            "blocked",
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
            "native EVM prover bundle must be an object",
        ]
    artifact = native_bundle.get("artifact")
    return [
        "yes" if native_bundle.get("required") is True else "no",
        _native_evm_bundle_status_cell(native_bundle),
        _native_evm_artifact_path_cell(artifact, field_label="artifact"),
        _native_evm_artifact_hash_cell(artifact, field_label="artifact"),
        _native_evm_hex32_cell(
            native_bundle.get("proof_artifact_hash"),
            field_label="proof_artifact_hash",
        ),
        _native_evm_hex32_cell(
            native_bundle.get("proving_key_hash"),
            field_label="proving_key_hash",
        ),
        _native_evm_hex32_cell(
            native_bundle.get("verifier_key_hash"),
            field_label="verifier_key_hash",
        ),
        _native_evm_hex32_cell(
            native_bundle.get("destination_binding_hash"),
            field_label="destination_binding_hash",
        ),
        _native_evm_support_artifact_cell(
            native_bundle.get("cross_sdk_fixture_parity_artifact"),
            field_label="cross_sdk_fixture_parity_artifact",
        ),
        _native_evm_support_artifact_cell(
            native_bundle.get("native_prover_self_test_artifact"),
            field_label="native_prover_self_test_artifact",
        ),
        _native_evm_sdk_artifacts_cell(native_bundle.get("sdk_artifacts")),
        _native_evm_bundle_blockers_cell(native_bundle),
    ]


def _source_inventory_gate_is_markdown_safe(gate: Any) -> bool:
    if not isinstance(gate, str) or not gate:
        return False
    if _path_control_character(gate) is not None:
        return False
    if not gate.isascii() or gate.strip() != gate:
        return False
    if any(character.isspace() for character in gate):
        return False
    if _path_markdown_unsafe_character(gate) is not None:
        return False
    if any(marker in gate.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return False
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789_")
    return (
        all(character in allowed for character in gate)
        and not gate.startswith("_")
        and not gate.endswith("_")
    )


def _source_inventory_gate_cell(gate: Any) -> str:
    if _source_inventory_gate_is_markdown_safe(gate):
        return f"`{gate}`"
    return "`<invalid gate>`"


def _source_inventory_status_cell(inventory: Any) -> str:
    if isinstance(inventory, dict) and inventory.get("validation_status") in {
        "passed",
        "blocked",
    }:
        return inventory["validation_status"]
    return "blocked"


def _source_inventory_blockers_cell(inventory: Any) -> str:
    if not isinstance(inventory, dict):
        return "source inventory gate must be an object"
    blockers = _markdown_string_list_cell(
        inventory.get("validation_blockers"),
        field_label="validation_blockers",
    )
    if inventory.get("validation_status") in {"passed", "blocked"}:
        return blockers
    if blockers == "-":
        return "`<invalid validation_status>`"
    return "`<invalid validation_status>`<br>" + blockers


def _source_inventory_markdown_rows(source_inventory: Any) -> list[list[str]]:
    if not isinstance(source_inventory, dict):
        return [
            [
                "`<invalid gate>`",
                "blocked",
                "source inventory must be an object",
            ]
        ]
    rows: list[list[str]] = []
    for gate, inventory in sorted(source_inventory.items(), key=lambda item: str(item[0])):
        rows.append(
            [
                _source_inventory_gate_cell(gate),
                _source_inventory_status_cell(inventory),
                _source_inventory_blockers_cell(inventory),
            ]
        )
    return rows


def _release_checklist_item_id_cell(item: Any) -> str:
    if isinstance(item, dict) and _source_inventory_gate_is_markdown_safe(
        item.get("id")
    ):
        return f"`{item['id']}`"
    return "`<invalid id>`"


def _release_checklist_item_status_cell(item: Any) -> str:
    return "ready" if isinstance(item, dict) and item.get("ready") is True else "blocked"


def _release_checklist_item_blockers_cell(
    item: Any,
    *,
    max_blockers_per_lane: int,
) -> str:
    if not isinstance(item, dict):
        return "release checklist item must be an object"
    item_blockers = item.get("blockers")
    blockers = (
        item_blockers[:max_blockers_per_lane]
        if isinstance(item_blockers, list)
        else item_blockers
    )
    blocker_text = _markdown_string_list_cell(blockers, field_label="blockers")
    if (
        isinstance(item_blockers, list)
        and len(item_blockers) > max_blockers_per_lane
    ):
        remaining = len(item_blockers) - max_blockers_per_lane
        blocker_text += f"<br>... {remaining} more"
    return blocker_text


def _release_checklist_markdown_rows(
    release_checklist: Any,
    *,
    max_blockers_per_lane: int,
) -> list[list[str]]:
    if not isinstance(release_checklist, dict):
        return [["`<invalid id>`", "blocked", "release checklist must be an object"]]
    items = release_checklist.get("items")
    if not isinstance(items, list):
        return [
            [
                "`<invalid id>`",
                "blocked",
                "release checklist items must be a list",
            ]
        ]
    return [
        [
            _release_checklist_item_id_cell(item),
            _release_checklist_item_status_cell(item),
            _release_checklist_item_blockers_cell(
                item,
                max_blockers_per_lane=max_blockers_per_lane,
            ),
        ]
        for item in items
    ]


def _cryptographic_evidence_markdown_rows(value: Any) -> list[list[str]]:
    if not isinstance(value, list):
        return [_cryptographic_evidence_markdown_row_cells(value)]
    return [_cryptographic_evidence_markdown_row_cells(row) for row in value]


def _user_prover_surface_markdown_rows(value: Any) -> list[list[str]]:
    if not isinstance(value, list):
        return [_user_prover_surface_markdown_row_cells(value)]
    return [_user_prover_surface_markdown_row_cells(surface) for surface in value]


def _markdown_artifact_path_cell(artifact: Any, *, field_label: str) -> str:
    if not isinstance(artifact, dict):
        return f"`<invalid {field_label}>`"
    artifact_path = artifact.get("path")
    if _native_evm_markdown_path_is_safe(artifact_path):
        return f"`{artifact_path}`"
    return f"`<invalid {field_label}>`"


def _markdown_artifact_bytes_cell(artifact: Any) -> str:
    if isinstance(artifact, dict) and type(artifact.get("bytes")) is int:
        artifact_bytes = artifact["bytes"]
        if artifact_bytes > 0:
            return str(artifact_bytes)
    return "`<invalid bytes>`"


def _markdown_artifact_hash_cell(artifact: Any, *, field_label: str) -> str:
    invalid_label = (
        field_label if field_label == "sha256" else f"{field_label}.sha256"
    )
    if not isinstance(artifact, dict):
        return f"`<invalid {invalid_label}>`"
    artifact_hash = artifact.get("sha256")
    if _is_nonzero_canonical_sha256_text(artifact_hash):
        return f"`{artifact_hash}`"
    return f"`<invalid {invalid_label}>`"


def _input_artifact_markdown_rows(input_artifacts: Any) -> list[list[str]]:
    if not isinstance(input_artifacts, list):
        return [
            [
                "`<invalid path>`",
                "`<invalid bytes>`",
                "`<invalid sha256>`",
            ]
        ]
    return [
        [
            _markdown_artifact_path_cell(artifact, field_label="path"),
            _markdown_artifact_bytes_cell(artifact),
            _markdown_artifact_hash_cell(artifact, field_label="sha256"),
        ]
        for artifact in input_artifacts
    ]


def _corridor_phase_key_blocker(label: str, phase: Any) -> str | None:
    if not isinstance(phase, str) or not phase:
        return f"{label} contains malformed phase"
    if _path_control_character(phase) is not None:
        return f"{label} contains phase with control character"
    if not phase.isascii():
        return f"{label} contains phase with non-ASCII character"
    if phase.strip() != phase:
        return f"{label} contains phase with surrounding whitespace"
    if any(character.isspace() for character in phase):
        return f"{label} contains phase with whitespace"
    if _path_markdown_unsafe_character(phase) is not None:
        return f"{label} contains phase with Markdown-unsafe character"
    if any(marker in phase.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return f"{label} contains phase with sensitive name"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in phase)
        or phase.startswith("-")
        or phase.endswith("-")
    ):
        return f"{label} contains malformed phase"
    return None


def _corridor_phase_cell(phase: Any) -> str:
    if _corridor_phase_key_blocker("readiness report corridor phases", phase) is None:
        return f"`{phase}`"
    return "`<invalid phase>`"


def _corridor_phase_status_cell(phase_status: Any) -> str:
    return (
        phase_status
        if phase_status in {"passed", "failed", "skipped", "missing"}
        else "`<invalid status>`"
    )


def _corridor_artifact_path_cell(artifact: Any) -> str:
    if artifact is None:
        return "-"
    return _markdown_artifact_path_cell(
        artifact,
        field_label="evidence_artifact",
    )


def _corridor_artifact_hash_cell(artifact: Any) -> str:
    if artifact is None:
        return "-"
    return _markdown_artifact_hash_cell(
        artifact,
        field_label="evidence_artifact",
    )


def _corridor_markdown_rows(corridor: Any) -> list[list[str]]:
    if not isinstance(corridor, dict):
        return [
            [
                "`<invalid phase>`",
                "`<invalid status>`",
                "-",
                "-",
            ]
        ]
    phases = corridor.get("phases")
    if not isinstance(phases, dict):
        return [
            [
                "`<invalid phase>`",
                "`<invalid status>`",
                "-",
                "-",
            ]
        ]
    evidence_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(evidence_artifacts, dict):
        evidence_artifacts = {}
    return [
        [
            _corridor_phase_cell(phase),
            _corridor_phase_status_cell(phase_status),
            _corridor_artifact_path_cell(evidence_artifacts.get(phase)),
            _corridor_artifact_hash_cell(evidence_artifacts.get(phase)),
        ]
        for phase, phase_status in phases.items()
    ]


def _markdown_string_list_items(value: Any, *, field_label: str) -> list[str]:
    if not isinstance(value, list):
        return [f"- `<invalid {field_label}>`"]
    if not value:
        return ["- None"]
    if not all(isinstance(item, str) and item for item in value):
        return [f"- `<invalid {field_label}>`"]
    if any(_public_blocker_text_issue(item) is not None for item in value):
        return [f"- `<invalid {field_label}>`"]
    if _public_blocker_list_duplicate_error(value, field_label) is not None:
        return [f"- `<invalid {field_label}>`"]
    return [f"- {item}" for item in value]


def _lane_readiness_markdown_rows(
    evidence: Any,
    *,
    max_blockers_per_lane: int,
) -> list[list[str]]:
    lanes = evidence.get("lanes") if isinstance(evidence, dict) else None
    if not isinstance(lanes, list):
        lanes = [None]
    rows: list[list[str]] = []
    for lane in lanes:
        (
            domain_cell,
            chain_cell,
            lane_status,
            records_cell,
            lane_blockers,
        ) = _lane_readiness_markdown_cells(lane)
        blockers = (
            lane_blockers[:max_blockers_per_lane]
            if isinstance(lane_blockers, list)
            else lane_blockers
        )
        blocker_text = _markdown_string_list_cell(blockers, field_label="blockers")
        if (
            isinstance(lane_blockers, list)
            and len(lane_blockers) > max_blockers_per_lane
        ):
            remaining = len(lane_blockers) - max_blockers_per_lane
            blocker_text += f"<br>... {remaining} more"
        rows.append(
            [
                domain_cell,
                chain_cell,
                lane_status,
                records_cell,
                blocker_text,
            ]
        )
    return rows


def _readiness_status_markdown_label(report: Any) -> str:
    """Return a fail-closed public readiness status label."""
    if not isinstance(report, dict):
        return "NOT READY"
    return "READY" if report.get("production_ready") is True else "NOT READY"


def _render_markdown(report: Any, *, max_blockers_per_lane: int) -> str:
    status = _readiness_status_markdown_label(report)
    if not isinstance(report, dict):
        report = {}
    lines = [
        "# SCCP Release Readiness Report",
        "",
        f"Status: {status}",
        "",
        "## Evidence Inputs",
        "",
    ]
    lines.append("| Path | Bytes | SHA-256 |")
    lines.append("| --- | ---: | --- |")
    for row in _input_artifact_markdown_rows(report.get("input_artifacts")):
        lines.append("| " + " | ".join(row) + " |")
    lines.extend(["", "## Production Corridor", ""])
    lines.append("| Phase | Status | Evidence Artifact | Evidence SHA-256 |")
    lines.append("| --- | --- | --- | --- |")
    for row in _corridor_markdown_rows(report.get("corridor")):
        lines.append("| " + " | ".join(row) + " |")

    lines.extend(["", "## Release Checklist", ""])
    lines.append("| Gate | Status | Blockers |")
    lines.append("| --- | --- | --- |")
    for row in _release_checklist_markdown_rows(
        report.get("release_checklist"),
        max_blockers_per_lane=max_blockers_per_lane,
    ):
        lines.append("| " + " | ".join(row) + " |")

    lines.extend(["", "## Cryptographic Evidence", ""])
    lines.append(
        "| Domain | Chain | EVM Source Chain ID | EVM Source Tag | "
        "EVM Destination Chain ID | EVM Destination Tag | "
        "Source Material | Source Deployment | "
        "Destination Binding | Source Gate | Source Gate Audits | "
        "Route Allowlist | Route Canary | Canary Source | "
        "Canary Message Proof | Canary TRON Owner | Canary TRON Signature | "
        "Canary Log Index | Canary Target Domain | Canary Proof Version | "
        "Canary Proof Source | "
        "Canary Call Data | Canary Payload | Canary Statement | "
        "Canary Commitment | Canary Finality Height | Canary Finality Block | "
        "Canary Tx | Canary Receipt Block | Canary Receipt Hash | "
        "Canary Receipt Finalized | "
        "Canary Receipts Root | Canary Message ID | Canary Block | "
        "Canary Timestamp |"
    )
    lines.append(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )
    for row in _cryptographic_evidence_markdown_rows(
        report.get("cryptographic_evidence")
    ):
        lines.append(
            "| "
            + " | ".join(row)
            + " |"
        )

    lines.extend(["", "## User Prover Submission Surfaces", ""])
    lines.append(
        "| Lanes | Proof Backend | SDK Helpers | On-chain Submission | "
        "Required Phases | Validation |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- |")
    for row in _user_prover_surface_markdown_rows(
        report.get("user_prover_submission_surfaces")
    ):
        lines.append("| " + " | ".join(row) + " |")

    lines.extend(["", "## Native Prover Bundle", ""])
    lines.append(
        "| Required | Status | Artifact | SHA-256 | Proof Artifact | Proving Key | "
        "Verifier Key | Destination Binding | Parity Fixture | Self-Test | "
        "SDK Artifacts | Blockers |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    lines.append(
        "| "
        + " | ".join(
            _native_evm_bundle_markdown_row_cells(
                report.get("native_evm_prover_bundle")
            )
        )
        + " |"
    )

    lines.extend(["", "## Source Inventory", ""])
    lines.append("| Gate | Status | Blockers |")
    lines.append("| --- | --- | --- |")
    for row in _source_inventory_markdown_rows(report.get("source_inventory")):
        lines.append("| " + " | ".join(row) + " |")

    lines.extend(["", "## Lane Readiness", ""])
    lines.append("| Domain | Chain | Status | Records | Blockers |")
    lines.append("| --- | --- | --- | --- | --- |")
    for row in _lane_readiness_markdown_rows(
        report.get("evidence"),
        max_blockers_per_lane=max_blockers_per_lane,
    ):
        lines.append("| " + " | ".join(row) + " |")

    lines.extend(["", "## Blocking Items", ""])
    lines.extend(
        _markdown_string_list_items(report.get("blockers"), field_label="blockers")
    )

    lines.extend(
        [
            "",
            "## Required Release Evidence",
            "",
            "- A passing `bash scripts/check_sccp_production_corridor.sh` run, recorded with `--require-phase-evidence` and one hashed `--phase-evidence` artifact for every passed phase.",
            "- Passing web/mobile SDK artifacts for the user-prover helper surface, including the JavaScript/web source, packaged `dist`, and TypeScript declaration exports used by portal builds.",
            f"- Complete {ACTIVE_LAUNCH_DISPLAY} launch-lane evidence containing source verifier material, source-adapter deployment, destination rollout, route allowlist, and route canary records; the all-lanes summary remains attached as diagnostic evidence for future lanes.",
            f"- {ACTIVE_LAUNCH_DISPLAY} source and destination EVM live reads must report {ACTIVE_LAUNCH_EVM_CHAIN_ID_EVIDENCE} and be pinned to the `finalized` block tag in both the all-lanes summary and readiness cryptographic-evidence table.",
            f"- {ACTIVE_LAUNCH_DISPLAY} route-canary transaction metadata must include a canonical non-zero transaction hash, finalized receipt block number/hash, receipts root, message id, and `{ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE}` evidence source before launch readiness can pass.",
            "- Governed live deployment evidence for immutable destination verifiers and source-chain verifier engines; offline placeholder or template-derived hashes keep the report blocked. Required source-verifier evidence by lane: Ethereum recursive source-adapter verifier deployment and remaining beacon light-client update/state branches are not complete for the SCCP inbound path; BSC recursive source-adapter verifier deployment is not complete for the SCCP inbound path; Solana audited Tower replay, full-bank AccountsDB lattice, bank/fork-choice, and source-adapter verifier deployment evidence is not complete for the SCCP inbound path; TON governed full-light-client verifier deployment, canary, and source-adapter deployment evidence are not complete for the SCCP inbound path; TRON transaction-Merkle source-call verifier deployment is not complete for the SCCP inbound path.",
            "- Windows `.NET 8.0.x` SCCP SDK phase evidence must include the full C# SCCP test run filtered by `FullyQualifiedName~Sccp`, canonical-case rejection coverage for proof-request, message-bundle, source-proof, and optional Groth16 artifact hash fields, including uppercase byte aliases and `0X` public-input, statement, bundle/source-proof, proof-artifact, and proving-key hashes, the `SCCP .NET SDK version:` marker emitted after `dotnet --version`, phase commands in `dotnet --version`, `dotnet --info`, `cargo build -p connect_norito_bridge`, `dotnet restore`, then strict `dotnet test` order, no restore/build diagnostics such as `error NU*`/`CS*`/`MSB*`/`NETSDK*`/`CA*`, non-zero `Error(s)` counts, `Failed to restore`, or restore/build failed markers, exact host markers `SCCP .NET SDK OS: Windows`, `SCCP .NET SDK RID: win-{x64,x86,arm64,arm}`, and `SCCP .NET SDK Architecture: {x64,x86,arm64,arm}` emitted after `dotnet --info`, exact native bridge markers `connect_norito_bridge native bridge: ...connect_norito_bridge.dll` and `connect_norito_bridge native bridge sha256: <64 lowercase hex>` emitted after `cargo build -p connect_norito_bridge`, `dotnet restore Hyperledger.Iroha.Sdk.sln` before the strict `dotnet test` command, a non-zero passed VSTest summary, the strict `SCCP .NET SDK TRX: .../sccp-dotnet-sdk.trx` marker, and `SCCP .NET SDK TRX bytes: <positive integer>` marker each emitted exactly once after the strict `dotnet test` command, with the summary from `Hyperledger.Iroha.Sdk.Tests.dll (net8.0)` reporting `Failed: 0`, `Skipped: 0`, `Total == Passed`, and a numeric unit duration, and with a positive TRX byte count plus a TRX marker that full-matches the direct C# test project `TestResults/sccp-dotnet-sdk.trx` path, and with direct VSTest-shaped TRX XML that is rooted at `TestRun`, keeps `UnitTestResult` rows directly under `Results`, keeps `UnitTest` definitions directly under `TestDefinitions`, names `Hyperledger.Iroha.Sdk.Tests.dll`, is at most 16777216 bytes, contains no DTD or entity declarations, uses unique TRX `UnitTest` and `Execution` ids, contains exactly the VSTest passed-test count of `UnitTestResult` rows, contains only `UnitTestResult` rows bound by `testId` or `executionId` to `Hyperledger.Iroha.Sdk.Tests.dll` SCCP test definitions whose names or classes contain an exact `Sccp...` test token, when both TRX identifiers are present, `testId` and `executionId` must bind the same SCCP test definition, when present `UnitTestResult` `testName` must match the bound SCCP test definition name and carry an exact `Sccp...` token, SCCP TRX test definition/result names used for binding must be unpadded and control-character-free, contains at least one passed SCCP `UnitTestResult`, and contains no failed, skipped, timed-out, or aborted SCCP `UnitTestResult`. Canonical `.NET` SCCP marker lines must use a single literal space after the colon; VSTest summary label/value and number/unit separators must be present, padding must use ordinary spaces only, and tab/control-whitespace separators remain forged evidence. Traced restore/test `PATH` prefixes must start with the printed `connect_norito_bridge.dll` directory and must not contain empty path-list segments. Named or traversal subdirectories before or after `TestResults` remain forged evidence and cannot satisfy release readiness. Windows backslash or drive-qualified TRX marker paths remain forged evidence too.",
            "- An audited `--native-evm-prover-bundle` manifest with `schema = sccp-native-evm-groth16-prover-bundle-v1`, `no_wasm = true`, `remote_prover_required = false`, and matching Ethereum destination binding/proving-key hashes.",
            f"- {SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE}",
            f"- {SCCP_NOT_REMAINING_WORK_SCOPE_NOTE}",
            "- SCCP launch-scope source inventory must pin active launch policy constants, the canonical Sora Nexus finality chain id, and the supported launch-domain set across Rust, all-lanes evidence, and readiness tooling.",
            "- SCCP Ethereum launch-policy selector source inventory must pin the EthereumMainnetLane selector and negative cross-lane policy tests.",
            "- SCCP Ethereum launch-policy documentation source inventory must pin the active Ethereum-mainnet policy wording and reject stale BSC-only production-packaging text.",
            "- SCCP public discovery documentation source inventory must pin supported launch-lane and verifier-target wording so unsupported lanes cannot re-enter Torii discovery evidence silently.",
            "- BSC Groth16 material documentation source inventory must pin PTAU-bound zkey verification, attestation request and finalize flow, and proof self-test operator steps before public bundle readiness can pass.",
            "- BSC Groth16 material evidence guard source inventory must pin closed review/audit evidence schemas, alias-conflict rejection, safe relative evidence path validation, non-symlink report/transcript reads, bounded public evidence files, required review/audit evidence flags, and adversarial tests before public bundle readiness can pass.",
            "- SCCP Ethereum no-proxy data-collection source inventory must pin app-owned execution/Beacon provider reads and reject Torii proxy or embedded HTTP-client fallbacks across public SDKs.",
            "- SCCP Ethereum inbound adversarial source inventory must pin public SDK regressions for failed receipts, source-event drift, hash-only proof bypasses, immutable evidence snapshots, oversized proof bytes, finality mismatches, sync-committee quorum checks, and wrong-domain receipt transcripts before inbound source proofs can be accepted.",
            "- SCCP BSC inbound adversarial source inventory must pin public SDK regressions for hash-only proof bypasses, receipt-proof metadata binding, source-event digest drift, malformed source logs, and missing source-event validation before BSC inbound source proofs can be accepted.",
            "- SCCP TRON inbound adversarial source inventory must pin runtime duplicate source-event log rejection before TRON transaction-info receipts can satisfy inbound source-proof admission.",
            "- SCCP BSC route-config canonical-manifest source inventory must pin canonical JSON string, lowercase bytes32, lowercase EVM address, and network metadata rejection before governed TAIRA XOR overlays can satisfy production readiness.",
            "- SCCP TRON route-config canonical-manifest source inventory must pin canonical JSON string, duplicate-alias, handoff-placeholder, lowercase bytes32, canonical Base58 address, and network metadata rejection before governed TAIRA XOR overlays can satisfy production readiness.",
            "- SCCP TRON runtime route-manifest source inventory must pin the TRON runtime route-manifest parser, mainnet metadata checks, dynamic destination-binding recomputation, and post-deploy anchor rejection before runtime config evidence can satisfy production readiness.",
            "- SCCP all-lanes route-canary scalar source inventory must pin canonical status/evidence-source schema blockers before all-lanes release-checklist route-canary readiness can pass.",
            "- SCCP all-lanes evidence-root schema source inventory must pin malformed evidence root, unknown section, and non-string section-key blockers before all-lanes evidence can satisfy production readiness.",
            "- SCCP all-lanes governed blocker schema source inventory must pin destination-rollout and route-allowlist blocker container rejection before governed evidence can satisfy production readiness.",
            "- SCCP all-lanes release-checklist exact-boolean source inventory must pin exact checklist-item aggregation, record-presence gates, CLI production-ready exits, source-adapter gate hash/audit replay rejection, route-canary hash replay rejection, and upstream route-canary hash replay rejection before all-lanes evidence can satisfy production readiness.",
            "- SCCP active-launch checklist schema source inventory must pin the active launch checklist ready value, malformed release-checklist roots, malformed lane metadata, and verifier recomputation before production readiness can pass.",
            "- SCCP route allowlist canary summary source inventory must pin optional canary-summary exactness, route-hash binding, and hash role-separation regressions before route profiles can be published as production evidence.",
            "- SCCP release manifest readiness-flags source inventory must pin exact boolean manifest generation, malformed readiness-root suppression, verifier boolean rejection, manifest/report equality checks, and all-lanes readiness recomputation before published bundle readiness can pass.",
            "- SCCP release manifest artifact-set/order source inventory must pin required artifact paths, manifest-root exclusion, unmanifested artifact/directory rejection, report-referenced artifact closure, malformed public artifact field-name classification, and canonical attachment order before published bundle readiness can pass.",
            "- SCCP release public blocker-list schema source inventory must pin canonical non-empty blocker strings, no surrounding whitespace, duplicate rejection, ready-surface empty-blocker checks, and invalid-marker rendering before published bundle readiness can pass.",
            "- SCCP release public scalar-text schema source inventory must pin canonical non-empty scalar text, fixed release-checklist item-id classification, public object-key classification for release-checklist titles, corridor phase keys, cryptographic-evidence chain/source labels, user-prover submission rows, all-lanes chain labels, all-lanes unknown object/audit keys, destination-binding keys, route-canary status/source fields, and redacted destination/Solana JSON-RPC/ProgramData/TON BoC/TRON route-canary scalar diagnostics before published bundle readiness can pass.",
            "- SCCP release-notes attachment invariants source inventory must pin canonical single top-level title/status block, Markdown short-indented heading recognition, Setext heading rejection, no unexpected section headings, exact manifest handoff/root-exclusion block, canonical single artifact table scaffold/shape and position, self-row exclusion, release-note artifact-row suppression, contiguous exact ordered row-set binding, canonical blocker-section visibility, no noncanonical trailing content, and canonical attachment drift rejection before public bundle readiness can pass.",
            "- SCCP readiness Markdown invariants source inventory must pin verifier-owned public Markdown sections, canonical top-level title/status block, Markdown short-indented heading recognition, Setext heading rejection, exact public section-heading spelling, no unexpected public section headings, repeated public section headings, noncanonical required-section order, canonical Required Release Evidence bullet spelling, top-level readiness status fail-closed rendering, evidence-input path/bytes/hash visibility, evidence-input row suppression, production-corridor phase/status visibility, production-corridor artifact/hash visibility, production-corridor row suppression, checklist gate/status visibility, checklist blocker-cell visibility, release-checklist row suppression, cryptographic row live-EVM visibility, cryptographic row core-hash visibility, cryptographic row route-canary visibility, cryptographic-evidence root suppression, lane-readiness status visibility, lane-readiness blocker-cell visibility, lane-readiness root suppression, source-inventory gate/status visibility, source-inventory blocker-cell visibility, user-prover validation-status visibility, user-prover blocker-cell visibility, user-prover helper/phase row visibility, user-prover root suppression, native-prover validation-status visibility, native-prover blocker-cell visibility, native-prover artifact/hash row visibility, native-prover support-artifact row visibility, source-inventory blocker visibility, invalid-marker rendering, malformed source-inventory gate-name, source-inventory row suppression, report-artifact path, and cryptographic-evidence row-domain/audit-key suppression, native-prover row suppression, and canonical Markdown drift rejection before public bundle readiness can pass.",
            "- SCCP transparent OpenVerify summary source inventory must pin schema/verifier-key manifest binding, canonical six-column public-input decoding, and malformed-column adversarial coverage before proof metadata can be published.",
            "- SCCP Ethereum outbound pre-callback source inventory must pin public SDK regressions that reject foreign-lane outbound requests, forged destination bindings, missing or partial proof-artifact hashes, zero proof-artifact hashes, and callback-visible proof material before outbound prover callbacks can run.",
            "- SCCP Ethereum outbound provider-validation source inventory must pin public SDK and facade guards that validate app-supplied Ethereum mainnet execution providers before outbound submitter callbacks can run.",
            "- SCCP Ethereum local-admission source inventory must pin public SDK regressions that reject mutated proof bytes, all-zero proof/public-input/bundle/envelope bytes, empty envelopes, zero statement/source-material/source-adapter hashes, and stale proof-family metadata before local admission payloads can be submitted.",
            "- SCCP Ethereum receipt-root zero source inventory must pin public SDK regressions that reject all-zero typed receipt roots before receipt-proof bytes can be built.",
            "- SCCP Ethereum receipt RLP zero-topic source inventory must pin public SDK and evidence helpers that preserve zero log topics in generic receipt RLP before SCCP source-event ABI filtering runs.",
            "- SCCP Ethereum receipt RLP zero-address source inventory must pin public SDK and evidence helpers that preserve zero log addresses in generic receipt RLP before SCCP source-event ABI filtering runs.",
            "- SCCP Ethereum source-event context source inventory must pin receipt-proof evidence guards that bind source-event logs to receipt transaction hash, block hash, and block number before source-event evidence is accepted.",
            "- SCCP Ethereum source-event evidence-mode source inventory must pin receipt-proof evidence guards that require source-bridge validation or an explicit receipt-only mode before receipt proof summaries can be emitted.",
            "- SCCP Ethereum source-event zero-digest source inventory must pin receipt-proof evidence guards that reject all-zero source-event digests before source-event evidence can be accepted.",
            "- SCCP Ethereum receipt RPC duplicate-JSON source inventory must pin evidence-script guards that reject duplicate JSON-RPC result or receipt keys and redact receipt RPC transport/error details before receipt proof evidence can be parsed.",
            "- SCCP Ethereum block receipt transaction-hash source inventory must pin evidence and SDK guards that reject duplicate transaction hashes in block receipt lists before receipt trie proofs can be built.",
            "- SCCP Ethereum JavaScript receipt-admission source inventory must pin source/dist guards for matching block receipts, beacon finality, typed-receipt rejection, and immutable prover-callback evidence before browser local proving can run.",
            "- SCCP Ethereum SDK receipt-metadata source inventory must pin public SDK guards for block-receipt metadata binding and typed-receipt rejection before receipt proof builders can run.",
            "- SCCP Ethereum native receipt-finality source inventory must pin Swift/Kotlin/JVM/Java Android/.NET receipt-proof builders to require finalized-header root, sync-committee root, and beacon slot before local proving can run.",
            "- SCCP Ethereum noncanonical chain-id source inventory must pin public SDK and evidence-script regressions that reject noncanonical Ethereum eth_chainId quantities such as 0x01, uppercase, padded, numeric, or whitespace-wrapped values before local source-proof evidence can be accepted.",
            "- SCCP Ethereum Beacon REST finalized-header shape source inventory must pin public SDK validators and negative tests for non-zero parent/state/body roots plus 96-byte finalized-header signatures before local finality evidence can be accepted.",
            "- SCCP Ethereum Beacon REST execution-payload binding source inventory must pin Beacon target-header/root/block reads, light-client finality-update evidence, execution block-hash/receipts-root binding, and C# SSZ root parity vectors before local finality evidence can be accepted.",
            "- SCCP Ethereum sync-committee roster source inventory must pin exact 512-authority mainnet rosters, unit validator weights, 342-participant quorum fixtures, and 81,925-byte next-sync-committee payload vectors across public SDKs before local finality evidence can be accepted.",
            "- SCCP Ethereum source-bridge config source inventory must pin bridge-address/network/code-hash config hashing, source-bridge network-id/code-hash role-reuse rejection, and negative config-drift tests.",
            "- SCCP Ethereum EVM source-adapter deployment source inventory must pin the active deployment gate, source-bridge network/config binding, ETH/BSC deployment helper coverage, and negative drift tests.",
            "- SCCP source-material template rejection source inventory must pin ETH, BSC, Solana, TON, and TRON evidence-script guards, aggregate all-lanes copied-evidence guards, strict release-bundle public JSON guards, and negative tests that reject built-in template verifier hashes before source material can satisfy production readiness.",
            "- SCCP source-material role validation source inventory must pin ETH, BSC, Solana, TON, and TRON zero-hash, role-reuse, canonical adapter-verifier, full-light-client audit role-separation, descriptor control-field drift, TRON source-call contract/owner role-separation guards, C#/.NET ETH/BSC source-material vectors, and redacted all-lanes-TOML/source validator/source-record/source-gate/TON-live-accountStates/address/code-BoC/TON-destination-code-BoC/TRON-live-API/metadata/full-TOML/TRON-witness-JSON blockers before source material can satisfy production readiness.",
            "- SCCP EVM contract smoke Ethereum mainnet network-id source inventory must pin ETH chain-id vectors, BSC rejection vectors, and accepted-event network-id assertions.",
            "- SCCP EVM contract smoke production-surface source inventory must pin verifier-code/key, destination-binding, domain-overflow, proof-shape, cross-deployment, and replay rejection smoke coverage.",
            "- SCCP Ethereum core range/finality binding source inventory must pin finality-height range binding in Core and negative outer-range replay tests.",
            "- SCCP Ethereum core message replay source inventory must pin durable pinned-record replay protection and negative replay/history tests.",
            "- SCCP Ethereum Torii pinned message proof source inventory must pin pinned message-proof extraction and negative unpinned-record serving tests.",
            "- Ethereum mainnet live EVM source production source inventory must pin canonical live source RPC chain ids, ETH/BSC source-live lane coverage, finalized block tags, deployment receipt binding, redacted JSON-RPC diagnostics, route canary calldata, and proof tuple drift tests.",
            "- Ethereum mainnet live EVM destination production source inventory must pin canonical live destination RPC chain ids, ETH/BSC destination-live lane coverage, finalized block tags, runtime bytecode hashes, redacted runtime bytecode parser diagnostics, and destination production TOML evidence before production readiness can pass.",
            "- SCCP Ethereum route-canary finalized receipt-block source inventory must pin finalized receipt-block binding, TOML evidence fields, all-lanes comments, runtime hashing, and negative drift tests.",
            "- SCCP Ethereum EVM block-tag metadata source inventory must pin finalized source/destination block-tag evidence and negative drift tests.",
            "- SCCP native no-WASM/no-remote source inventory must pin public SDK parsers, artifact verifiers, self-tests, browser distribution guards, canonical native EVM prover SDK-id rejection, padded-SDK adversarial tests, adversarial manifest coverage, and redacted native payload artifact-path diagnostics.",
            "- SCCP release native-prover bundle schema source inventory must pin native EVM Groth16 manifest schema, readiness summary schema, artifact hash/path binding, copied-summary scalar exactness, and bundled-manifest drift rejection before published bundle readiness can pass.",
            "- SCCP proof-request bundle/source-proof source inventory must pin canonical bundle-byte, SORA-empty source-proof, and decoded non-SORA source-proof binding gates across Rust, JavaScript, Python, Swift, Kotlin/JVM, Java Android, and C#/.NET.",
            "- SCCP phase-evidence source inventory must pin duplicate assignment and directory override rejection across readiness-report and release-bundle CLIs before corridor phase evidence can satisfy production readiness.",
            "- SCCP release corridor phase-transcript source inventory must pin exact phase markers, phase-local ordered non-negated/non-diagnostic shell-xtrace-free completion/success output after required commands in per-phase and full-corridor logs, phase-specific traced command shapes with exact pytest positional inputs, option-bound selectors, exact Gradle test command parsing, exact Kotlin Gradle selector list, exact Swift filter commands, exact Java Android harness class list including TRON, exact Node test/check command files, exact .NET project/filter/nologo commands, exact no-suffix cargo/bash/java commands, and without bare-fragment shortcuts or shell-comment-hidden fragments, restricted cd wrappers, dry-run rejection, failure-marker scans, and forged-block rejection before corridor logs can satisfy public bundle readiness.",
            "- SCCP release bundle source-copy source inventory must pin symlink, control-character, non-ASCII filename, and secret-looking filename rejection for evidence inputs, phase evidence, native EVM prover manifests, and native prover payload sources before bundle copy can run.",
            "- SCCP release bundle output-path source inventory must pin symlink and control-character rejection for output directories before bundle generation can create or overwrite release artifacts.",
            "- SCCP release artifact path text source inventory must pin Markdown-unsafe, non-ASCII, and secret-looking path rejection for manifest artifact paths, readiness inputs, native prover manifest/payload paths, copied bundle filenames, and bundle filesystem entries before release notes can render artifact tables.",
            "- SCCP release input-provenance schema source inventory must pin canonical copied evidence input paths, unique input/input-artifact provenance, copied `evidence/NN-*.toml` layout, and recomputation from copied TOML before published bundle readiness can pass.",
            "- SCCP release public JSON-root schema source inventory must pin canonical manifest/readiness/all-lanes JSON serialization, Rust SCCP helper JSON canonicalization, category-only Rust SCCP JSON enum diagnostics, duplicate-key rejection with malformed-key classification, non-UTF-8 fail-closed diagnostics, redacted source-inventory read diagnostics, and malformed manifest/readiness root-field classification before published bundle readiness can pass.",
            "- SCCP release public Markdown text schema source inventory must pin UTF-8 readiness/release-note Markdown loading and canonical text drift rejection before published bundle readiness can pass.",
            "- SCCP release public cryptographic-evidence binding source inventory must pin production-domain inventory, row-key and audit-key classification, lane-field binding, canonical row recomputation, Markdown row-domain/audit-key suppression, and active route-canary binding rejection before published bundle readiness can pass.",
            "- SCCP release public submission-surface binding source inventory must pin lane/backend inventory, per-SDK helper inventory, verifier-owned surface recomputation, and corridor-phase binding before published bundle readiness can pass.",
            "- SCCP retired network-surface source inventory must pin the launch-scope no-support note and active-tree scan so retired runtime-network integrations cannot re-enter release evidence silently.",
            "- SCCP unready transparent-proof source inventory must pin the diagnostic `allow_unready` toggle as config-owned, reject environment override paths, and reject production-ready BSC/TRON route configs that force the unready toggle back on.",
            "- SCCP TRON deploy operator boolean source inventory must pin malformed operator-boolean rejection before TRON deploy helper evidence can satisfy production readiness.",
            "- Public release notes must attach this report and the all-lanes JSON summary before production activation.",
        ]
    )
    return "\n".join(lines) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            f"Render a public SCCP release-readiness report from {ACTIVE_LAUNCH_DISPLAY} "
            "launch-lane evidence, all-lanes diagnostics, and "
            "production-corridor validation results."
        )
    )
    parser.add_argument(
        "toml",
        nargs="+",
        type=Path,
        help="TOML evidence snippet or full config containing [zk] SCCP records.",
    )
    parser.add_argument(
        "--phase-result",
        action="append",
        default=[],
        metavar="PHASE=STATUS",
        help=(
            "Production-corridor phase status. Repeat for each phase, or use "
            "all=passed after a full corridor run."
        ),
    )
    parser.add_argument(
        "--phase-evidence",
        action="append",
        default=[],
        metavar="PHASE=PATH",
        help=(
            "Hash a production-corridor run artifact for one phase, or use "
            "all=PATH to bind the same full-run log to every phase."
        ),
    )
    parser.add_argument(
        "--phase-evidence-dir",
        type=Path,
        help=(
            "Directory containing hashed production-corridor phase logs. The "
            "report accepts <dir>/<phase>.log, "
            "<dir>/dist/sccp-production-corridor/<phase>.log, or downloaded "
            "CI artifact folders named sccp-production-corridor-<phase>."
        ),
    )
    parser.add_argument(
        "--require-phase-evidence",
        action="store_true",
        help=(
            "Keep the report blocked unless every passed corridor phase has a "
            "hashed --phase-evidence artifact."
        ),
    )
    parser.add_argument(
        "--native-evm-prover-bundle",
        type=Path,
        help=(
            "Hash and validate the audited Ethereum mainnet no-WASM native EVM "
            "Groth16 prover bundle manifest."
        ),
    )
    parser.add_argument(
        "--format",
        choices=("markdown", "json"),
        default="markdown",
        help="Report output format.",
    )
    parser.add_argument(
        "--max-blockers-per-lane",
        type=int,
        default=4,
        help="Maximum lane blockers to show in the markdown table.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the report to this path instead of stdout.",
    )
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "secret key",
    "secret-key",
    "secret_key",
    "private key",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer",
    "authorization",
    "access key",
    "access-key",
    "access_key",
    "api key",
    "api-key",
    "api_key",
    "client secret",
    "client-secret",
    "client_secret",
    "credential",
    "credentials",
    "auth header",
    "auth-header",
    "auth_header",
    "mnemonic",
    "seed phrase",
    "seed-phrase",
    "seed_phrase",
    "signing key",
    "signing-key",
    "signing_key",
    "session",
    "token",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, SystemExit):
        return fallback
    if isinstance(exc, OSError) and (
        getattr(exc, "filename", None) is not None
        or getattr(exc, "filename2", None) is not None
    ):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    if not text.isascii():
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def _canonical_public_report_blockers(
    value: Any,
) -> tuple[list[str], list[str]]:
    if not isinstance(value, list):
        return [], [
            "readiness report blockers must be a list of non-empty canonical strings"
        ]
    blockers: list[str] = []
    errors: list[str] = []
    for index, blocker in enumerate(value):
        issue = _public_blocker_text_issue(blocker)
        if issue is None:
            blockers.append(blocker)
        elif issue == "non-empty canonical string":
            errors.append(f"readiness report blockers[{index}] must be a {issue}")
        else:
            errors.append(f"readiness report blockers[{index}] contains {issue}")
    duplicate_error = _public_blocker_list_duplicate_error(
        value,
        "readiness report blockers",
    )
    if duplicate_error is not None:
        errors.append(duplicate_error)
        blockers = []
    return blockers, errors


def _readiness_report_unknown_field_blocker(field: Any) -> str:
    return _native_evm_prover_field_name_blocker(
        "readiness report",
        field,
        "unknown top-level",
    )


def _public_release_checklist_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public release-checklist payloads."""

    if not isinstance(value, dict):
        return ["readiness report release_checklist must be an object"]

    errors: list[str] = []
    for field in sorted(
        (field for field in value if field not in RELEASE_CHECKLIST_PUBLIC_FIELDS),
        key=str,
    ):
        errors.append(
            _native_evm_prover_field_name_blocker(
                "readiness report release_checklist",
                field,
                "unknown",
            )
        )

    if type(value.get("ready")) is not bool:
        errors.append("readiness report release_checklist ready must be boolean")

    items = value.get("items")
    if not isinstance(items, list):
        errors.append("readiness report release_checklist items must be a list")
        return errors

    seen_item_ids: set[str] = set()
    for index, item in enumerate(items):
        item_label = f"readiness report release_checklist items[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{item_label} must be an object")
            continue
        for field in sorted(
            (field for field in item if field not in RELEASE_CHECKLIST_ITEM_PUBLIC_FIELDS),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(item_label, field, "unknown")
            )
        item_id = item.get("id")
        if not _source_inventory_gate_is_markdown_safe(item_id):
            errors.append(f"{item_label} id must be a non-empty canonical string")
            item_key: str | None = None
        elif item_id not in ACTIVE_LAUNCH_RELEASE_CHECKLIST_ITEM_IDS:
            errors.append(f"{item_label} id must be a required checklist id")
            item_key = None
        elif item_id in seen_item_ids:
            errors.append(
                f"readiness report release_checklist item {item_id} is duplicated"
            )
            item_key = item_id
        else:
            seen_item_ids.add(item_id)
            item_key = item_id
        title = item.get("title")
        if not isinstance(title, str) or not title or title.strip() != title:
            errors.append(f"{item_label} title must be a non-empty canonical string")
        elif _path_control_character(title) is not None:
            errors.append(f"{item_label} title contains control character")
        elif not title.isascii():
            errors.append(f"{item_label} title contains non-ASCII character")
        elif _path_markdown_unsafe_character(title) is not None:
            errors.append(f"{item_label} title contains Markdown-unsafe character")
        elif any(
            marker in title.lower()
            for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS
        ):
            errors.append(f"{item_label} title contains sensitive value")
        elif (
            item_key is None
            or title != ACTIVE_LAUNCH_RELEASE_CHECKLIST_TITLES[item_key]
        ):
            errors.append(f"{item_label} title must match the canonical checklist title")
        item_ready = item.get("ready")
        if type(item_ready) is not bool:
            errors.append(f"{item_label} ready must be boolean")
        elif value.get("ready") is True and item_ready is not True:
            errors.append(
                f"{item_label} ready must be true when release_checklist ready is true"
            )
        blockers = item.get("blockers")
        if not isinstance(blockers, list):
            errors.append(
                f"{item_label} blockers must be a list of non-empty canonical strings"
            )
            continue
        for blocker_index, blocker in enumerate(blockers):
            issue = _public_blocker_text_issue(blocker)
            if issue is None:
                continue
            if issue == "non-empty canonical string":
                errors.append(
                    f"{item_label} blockers[{blocker_index}] must be a {issue}"
                )
            else:
                errors.append(f"{item_label} blockers[{blocker_index}] contains {issue}")
        duplicate_error = _public_blocker_list_duplicate_error(
            blockers,
            f"{item_label} blockers",
        )
        if duplicate_error is not None:
            errors.append(duplicate_error)
        if item_ready is True and blockers:
            errors.append(f"{item_label} blockers must be empty when ready is true")
    for item_id in ACTIVE_LAUNCH_RELEASE_CHECKLIST_ITEM_IDS:
        if item_id not in seen_item_ids:
            errors.append(f"readiness report release_checklist missing item {item_id}")
    return errors


def _public_input_artifact_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public input artifact rows."""

    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        return ["readiness report input_artifacts must be a list of objects"]

    errors: list[str] = []
    seen_paths: set[str] = set()
    for index, artifact in enumerate(value):
        artifact_label = f"readiness report input_artifacts[{index}]"
        for field in sorted(
            (field for field in artifact if field not in INPUT_ARTIFACT_PUBLIC_FIELDS),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(
                    artifact_label,
                    field,
                    "unknown",
                )
            )
        for field in sorted(INPUT_ARTIFACT_PUBLIC_FIELDS - set(artifact)):
            errors.append(f"{artifact_label} missing field: {field}")
        if "path" in artifact and not _native_evm_markdown_path_is_safe(
            artifact.get("path")
        ):
            errors.append(f"{artifact_label} path must be a canonical public path")
        elif "path" in artifact:
            artifact_path = artifact.get("path")
            if artifact_path in seen_paths:
                errors.append("readiness report input_artifacts contains duplicate path")
            seen_paths.add(artifact_path)
        artifact_bytes = artifact.get("bytes")
        if "bytes" in artifact and (
            type(artifact_bytes) is not int or artifact_bytes <= 0
        ):
            errors.append(f"{artifact_label} bytes must be a positive integer")
        if "sha256" in artifact:
            errors.extend(_sha256_text_errors(artifact_label, artifact.get("sha256")))
    return errors


def _public_corridor_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public corridor summaries."""

    if not isinstance(value, dict):
        return ["readiness report corridor must be an object"]

    errors: list[str] = []
    for field in sorted((field for field in value if field not in CORRIDOR_PUBLIC_FIELDS), key=str):
        errors.append(
            _native_evm_prover_field_name_blocker(
                "readiness report corridor",
                field,
                "unknown",
            )
        )
    for field in sorted(CORRIDOR_PUBLIC_FIELDS - set(value)):
        errors.append(f"readiness report corridor missing field: {field}")

    production_ready = value.get("production_ready")
    if type(production_ready) is not bool:
        errors.append("readiness report corridor production_ready must be boolean")
    require_phase_evidence = value.get("require_phase_evidence")
    if type(require_phase_evidence) is not bool:
        errors.append("readiness report corridor require_phase_evidence must be boolean")

    blockers = value.get("blockers")
    if not isinstance(blockers, list):
        errors.append(
            "readiness report corridor blockers must be a list of non-empty "
            "canonical strings"
        )
    else:
        for blocker_index, blocker in enumerate(blockers):
            issue = _public_blocker_text_issue(blocker)
            if issue is None:
                continue
            if issue == "non-empty canonical string":
                errors.append(
                    "readiness report corridor blockers"
                    f"[{blocker_index}] must be a {issue}"
                )
            else:
                errors.append(
                    "readiness report corridor blockers"
                    f"[{blocker_index}] contains {issue}"
                )
        duplicate_error = _public_blocker_list_duplicate_error(
            blockers,
            "readiness report corridor blockers",
        )
        if duplicate_error is not None:
            errors.append(duplicate_error)
        if production_ready is True and blockers:
            errors.append(
                "readiness report corridor blockers must be empty when "
                "production_ready is true"
            )

    phases = value.get("phases")
    known_phases: list[str] = []
    if not isinstance(phases, dict):
        errors.append("readiness report corridor phases must be an object")
    else:
        try:
            known_phases = _corridor_phases()
        except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
            errors.append(
                "readiness report corridor phase inventory cannot run corridor helper"
            )
        known_phase_set = set(known_phases)
        for phase in sorted(set(phases) - known_phase_set, key=str):
            phase_error = _corridor_phase_key_blocker(
                "readiness report corridor phases",
                phase,
            )
            if phase_error is not None:
                errors.append(phase_error)
            else:
                errors.append(
                    f"readiness report corridor contains unknown phase: {phase}"
                )
        for phase in known_phases:
            if phase not in phases:
                errors.append(
                    f"readiness report corridor missing phase status: {phase}"
                )
                continue
            status = phases.get(phase)
            if status not in CORRIDOR_PUBLIC_PHASE_STATUSES:
                errors.append(
                    "readiness report corridor phase "
                    f"{phase} status must be passed, failed, skipped, or missing"
                )
            elif production_ready is True and status != "passed":
                errors.append(
                    f"readiness report corridor phase {phase} must be passed "
                    "when production_ready is true"
                )

    evidence_artifacts = value.get("evidence_artifacts")
    if not isinstance(evidence_artifacts, dict):
        errors.append("readiness report corridor evidence_artifacts must be an object")
    else:
        known_phase_set = set(known_phases)
        for phase in sorted(set(evidence_artifacts) - known_phase_set, key=str):
            phase_error = _corridor_phase_key_blocker(
                "readiness report corridor evidence_artifacts",
                phase,
            )
            if phase_error is not None:
                errors.append(phase_error)
            else:
                errors.append(
                    "readiness report corridor evidence_artifacts contains "
                    f"unknown phase: {phase}"
                )
        for phase, artifact in sorted(evidence_artifacts.items(), key=lambda item: str(item[0])):
            if _corridor_phase_key_blocker(
                "readiness report corridor evidence_artifacts",
                phase,
            ) is not None:
                continue
            artifact_errors = _public_input_artifact_errors([artifact])
            for error in artifact_errors:
                errors.append(
                    error.replace(
                        "readiness report input_artifacts[0]",
                        f"readiness report corridor evidence_artifacts.{phase}",
                    )
                )

    if (
        production_ready is True
        and require_phase_evidence is True
        and isinstance(phases, dict)
        and isinstance(evidence_artifacts, dict)
    ):
        for phase in known_phases:
            if phases.get(phase) == "passed" and phase not in evidence_artifacts:
                errors.append(
                    "readiness report corridor phase "
                    f"{phase} has no hashed evidence artifact"
                )

    return errors


def _public_native_evm_artifact_errors(
    value: Any,
    label: str,
    *,
    expected_hash: Any = None,
    require_hash_match: bool = True,
) -> list[str]:
    """Return bounded blockers for malformed native-prover artifact metadata."""

    if not isinstance(value, dict):
        return [f"{label} must be an object"]

    errors: list[str] = []
    for field in sorted(
        (field for field in value if field not in INPUT_ARTIFACT_PUBLIC_FIELDS),
        key=str,
    ):
        errors.append(_native_evm_prover_field_name_blocker(label, field, "unknown"))
    for field in sorted(INPUT_ARTIFACT_PUBLIC_FIELDS - set(value)):
        errors.append(f"{label} missing field: {field}")

    artifact_path = value.get("path")
    if "path" in value and not _native_evm_markdown_path_is_safe(artifact_path):
        errors.append(f"{label} path must be a canonical public path")
    artifact_bytes = value.get("bytes")
    if "bytes" in value and (type(artifact_bytes) is not int or artifact_bytes < 0):
        errors.append(f"{label} bytes must be a non-negative integer")
    artifact_hash = value.get("sha256")
    if "sha256" in value:
        errors.extend(_sha256_text_errors(label, artifact_hash))
    if (
        require_hash_match
        and expected_hash is not None
        and _is_nonzero_canonical_sha256_text(artifact_hash)
        and _is_nonzero_hex32(expected_hash)
        and f"0x{artifact_hash}" != expected_hash
    ):
        errors.append(f"{label} sha256 must match the paired bundle hash")
    return errors


def _public_native_evm_sdk_id_error(sdk: Any) -> str | None:
    """Return a bounded blocker for malformed native-prover SDK ids."""

    if (
        not isinstance(sdk, str)
        or not sdk
        or _public_blocker_text_issue(sdk) is not None
        or any(character.isspace() for character in sdk)
    ):
        return "sdk must be a canonical SDK id"
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in sdk)
        or sdk.startswith("-")
        or sdk.endswith("-")
    ):
        return "sdk must be a canonical SDK id"
    if sdk not in NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS:
        return "sdk must be a required SDK id"
    return None


def _public_native_evm_audit_hash_errors(
    value: dict[str, Any],
    label: str,
    *,
    require_complete: bool = True,
) -> list[str]:
    """Return bounded blockers for public native-prover audit hashes."""

    audit_hashes = value.get("audit_hashes")
    if not isinstance(audit_hashes, dict):
        return [f"{label} audit_hashes must be a non-empty object"]
    if require_complete and not audit_hashes:
        return [f"{label} audit_hashes must be a non-empty object"]

    errors: list[str] = []
    for field in sorted(
        set(audit_hashes) - set(NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES),
        key=str,
    ):
        errors.append(
            _native_evm_prover_field_name_blocker(
                f"{label} audit_hashes",
                field,
                "unexpected",
            )
        )
    if require_complete:
        for field in sorted(
            set(NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES) - set(audit_hashes)
        ):
            errors.append(f"{label} audit_hashes missing field: {field}")

    reserved_hash_roles = {
        "proof_artifact_hash": value.get("proof_artifact_hash"),
        "proving_key_hash": value.get("proving_key_hash"),
        "verifier_key_hash": value.get("verifier_key_hash"),
        "destination_binding_hash": value.get("destination_binding_hash"),
    }
    sdk_artifacts = value.get("sdk_artifacts")
    if isinstance(sdk_artifacts, list):
        for index, row in enumerate(sdk_artifacts):
            if isinstance(row, dict):
                reserved_hash_roles[f"sdk_artifacts[{index}].implementation_hash"] = (
                    row.get("implementation_hash")
                )

    seen_audit_hashes: dict[str, str] = {}
    for field in NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES:
        if field not in audit_hashes:
            continue
        audit_hash = audit_hashes.get(field)
        if not _is_nonzero_hex32(audit_hash):
            errors.append(
                f"{label} audit_hashes.{field} must be a canonical non-zero "
                "32-byte hex value"
            )
            continue
        if not require_complete:
            continue
        previous_field = seen_audit_hashes.get(audit_hash)
        if previous_field is not None:
            errors.append(
                f"{label} audit_hashes.{field} must not duplicate "
                f"audit_hashes.{previous_field}"
            )
        seen_audit_hashes[audit_hash] = field
        for role, role_hash in reserved_hash_roles.items():
            if audit_hash == role_hash:
                errors.append(f"{label} audit_hashes.{field} must not reuse {role}")
    return errors


def _public_native_evm_sdk_artifact_errors(
    value: Any,
    label: str,
    *,
    require_hash_match: bool = True,
    require_complete: bool = True,
) -> list[str]:
    """Return bounded blockers for public native-prover SDK artifacts."""

    if not isinstance(value, list):
        return [f"{label} sdk_artifacts must be a non-empty list"]
    if require_complete and not value:
        return [f"{label} sdk_artifacts must be a non-empty list"]

    errors: list[str] = []
    seen_sdks: set[str] = set()
    semantic_sdk_order: list[str] = []
    for index, row in enumerate(value):
        row_label = f"{label} sdk_artifacts[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_label} must be an object")
            continue
        for field in sorted(
            (
                field
                for field in row
                if field not in NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_PUBLIC_FIELDS
            ),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(row_label, field, "unknown")
            )
        for field in sorted(
            NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_PUBLIC_FIELDS - set(row)
        ):
            errors.append(f"{row_label} missing field: {field}")

        sdk = row.get("sdk")
        sdk_error = _public_native_evm_sdk_id_error(sdk)
        if sdk_error is not None:
            errors.append(f"{row_label} {sdk_error}")
            continue

        assert isinstance(sdk, str)
        semantic_sdk_order.append(sdk)
        if require_complete and sdk in seen_sdks:
            errors.append(f"{row_label} sdk is duplicated")
        seen_sdks.add(sdk)

        expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS[sdk]
        if row.get("implementation") != expected_implementation:
            errors.append(f"{row_label} implementation must match the required SDK")
        if not _is_nonzero_hex32(row.get("implementation_hash")):
            errors.append(
                f"{row_label} implementation_hash must be a canonical non-zero "
                "32-byte hex value"
            )
        errors.extend(
            _public_native_evm_artifact_errors(
                row.get("implementation_artifact"),
                f"{row_label} implementation_artifact",
                expected_hash=row.get("implementation_hash"),
                require_hash_match=require_hash_match,
            )
        )

    if require_complete:
        for sdk in sorted(set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS) - seen_sdks):
            errors.append(f"{label} sdk_artifacts missing sdk: {sdk}")
        if semantic_sdk_order != sorted(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS):
            errors.append(f"{label} sdk_artifacts must match expected SDK order")
    return errors


def _public_native_evm_prover_bundle_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public native-prover summaries."""

    label = "readiness report native_evm_prover_bundle"
    # Source-inventory markers:
    # - readiness report native_evm_prover_bundle missing field
    # - readiness report native_evm_prover_bundle validation_blockers
    if not isinstance(value, dict):
        return [f"{label} must be an object"]

    errors: list[str] = []
    for field in sorted(
        (field for field in value if field not in NATIVE_EVM_PROVER_BUNDLE_PUBLIC_FIELDS),
        key=str,
    ):
        errors.append(_native_evm_prover_field_name_blocker(label, field, "unknown"))
    for field in sorted(NATIVE_EVM_PROVER_BUNDLE_PUBLIC_FIELDS - set(value)):
        errors.append(f"{label} missing field: {field}")

    if value.get("required") is not True:
        errors.append(f"{label} required must be true")

    validation_status = value.get("validation_status")
    validation_blockers = value.get("validation_blockers")
    manifest_load_blocker_prefixes = (
        "native EVM Groth16 prover bundle JSON contains duplicate key:",
    )
    manifest_load_blockers = {
        "native EVM Groth16 prover bundle manifest is required",
        "native EVM Groth16 prover bundle is not valid JSON",
        "native EVM Groth16 prover bundle is not UTF-8 text",
        "native EVM Groth16 prover bundle cannot be read",
        "native EVM Groth16 prover bundle artifact path metadata is invalid",
        "native EVM Groth16 prover bundle must be a JSON object",
        "native EVM Groth16 prover bundle JSON contains duplicate key with control character",
        "native EVM Groth16 prover bundle JSON contains duplicate key with non-ASCII character",
        "native EVM Groth16 prover bundle JSON contains duplicate key with sensitive key name",
    }
    manifest_load_blocked = (
        validation_status == "blocked"
        and isinstance(validation_blockers, list)
        and any(
            blocker in manifest_load_blockers
            or any(
                blocker.startswith(prefix)
                for prefix in manifest_load_blocker_prefixes
            )
            for blocker in validation_blockers
            if isinstance(blocker, str)
        )
    )

    expected_fields = {
        "schema": NATIVE_EVM_PROVER_BUNDLE_SCHEMA,
        "bundle_id": NATIVE_EVM_PROVER_BUNDLE_ID,
        "lanes": ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
    }
    if not manifest_load_blocked:
        for field, expected in expected_fields.items():
            if field in value and value.get(field) != expected:
                errors.append(f"{label} {field} must match the canonical native bundle")

        for field in (
            "proof_artifact_hash",
            "proving_key_hash",
            "verifier_key_hash",
            "destination_binding_hash",
        ):
            if field in value and not _is_nonzero_hex32(value.get(field)):
                errors.append(
                    f"{label} {field} must be a canonical non-zero 32-byte hex value"
                )

        audit_hashes = value.get("audit_hashes")
        support_artifact_hashes = audit_hashes if isinstance(audit_hashes, dict) else {}
        for artifact_field, hash_field in (
            ("artifact", None),
            ("proof_artifact", "proof_artifact_hash"),
            ("proving_key", "proving_key_hash"),
            ("verifier_key", "verifier_key_hash"),
            ("cross_sdk_fixture_parity_artifact", "cross_sdk_fixture_parity"),
            ("native_prover_self_test_artifact", "native_prover_self_test"),
        ):
            if validation_status == "blocked" and value.get(artifact_field) is None:
                continue
            errors.extend(
                _public_native_evm_artifact_errors(
                    value.get(artifact_field),
                    f"{label} {artifact_field}",
                    expected_hash=(
                        value.get(hash_field)
                        if hash_field in value
                        else support_artifact_hashes.get(hash_field)
                    )
                    if hash_field is not None
                    else None,
                    require_hash_match=validation_status != "blocked",
                )
            )

        errors.extend(
            _public_native_evm_audit_hash_errors(
                value,
                label,
                require_complete=validation_status != "blocked",
            )
        )
        errors.extend(
            _public_native_evm_sdk_artifact_errors(
                value.get("sdk_artifacts"),
                label,
                require_hash_match=validation_status != "blocked",
                require_complete=validation_status != "blocked",
            )
        )

    if validation_status not in {"passed", "blocked"}:
        errors.append(f"{label} validation_status must be passed or blocked")

    if not isinstance(validation_blockers, list):
        errors.append(
            f"{label} validation_blockers must be a list of non-empty "
            "canonical strings"
        )
    else:
        for blocker_index, blocker in enumerate(validation_blockers):
            issue = _public_blocker_text_issue(blocker)
            if issue is None:
                continue
            if issue == "non-empty canonical string":
                errors.append(
                    f"{label} validation_blockers[{blocker_index}] must be a "
                    f"{issue}"
                )
            else:
                errors.append(
                    f"{label} validation_blockers[{blocker_index}] contains {issue}"
                )
        duplicate_error = _public_blocker_list_duplicate_error(
            validation_blockers,
            f"{label} validation_blockers",
        )
        if duplicate_error is not None:
            errors.append(duplicate_error)
        if validation_status == "passed" and validation_blockers:
            errors.append(
                f"{label} validation_blockers must be empty when "
                "validation_status is passed"
            )
        if validation_status == "blocked" and not validation_blockers:
            errors.append(
                f"{label} validation_blockers must be non-empty when "
                "validation_status is blocked"
            )

    return errors


def _public_embedded_evidence_errors(value: Any) -> list[str]:
    """Return blockers for non-canonical embedded all-lanes public evidence."""

    if not isinstance(value, dict):
        return ["readiness report evidence must be an object"]

    try:
        all_lanes = _load_all_lanes_module()
        public_summary = all_lanes._public_summary_payload(value)
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        return [
            "readiness report evidence cannot run all-lanes public summary validator"
        ]

    if public_summary != value:
        return [
            "readiness report evidence must be a canonical public all-lanes summary"
        ]
    return []


def _redacted_native_evm_artifact_summary(
    value: Any,
    *,
    redact_unsafe_path: bool,
) -> Any:
    """Return copied artifact metadata with unsafe public paths redacted."""

    if not isinstance(value, dict):
        return value
    redacted = dict(value)
    if (
        redact_unsafe_path
        and "path" in redacted
        and not _native_evm_markdown_path_is_safe(redacted.get("path"))
    ):
        redacted["path"] = "redacted"
    return redacted


def _redacted_public_native_evm_prover_bundle(value: dict[str, Any]) -> dict[str, Any]:
    """Return a copied native-prover summary safe for public JSON rendering."""

    redacted = dict(value)
    blocked = redacted.get("validation_status") == "blocked"
    if blocked:
        audit_hashes = redacted.get("audit_hashes")
        if isinstance(audit_hashes, dict):
            redacted["audit_hashes"] = {
                field: audit_hash
                for field, audit_hash in audit_hashes.items()
                if _is_nonzero_hex32(audit_hash)
            }
    for field in (
        "artifact",
        "proof_artifact",
        "proving_key",
        "verifier_key",
        "cross_sdk_fixture_parity_artifact",
        "native_prover_self_test_artifact",
    ):
        if field in redacted:
            redacted[field] = _redacted_native_evm_artifact_summary(
                redacted.get(field),
                redact_unsafe_path=blocked,
            )

    sdk_artifacts = redacted.get("sdk_artifacts")
    if isinstance(sdk_artifacts, list):
        copied_rows: list[Any] = []
        for row in sdk_artifacts:
            if isinstance(row, dict):
                expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS.get(
                    row.get("sdk")
                )
                if blocked and (
                    _public_native_evm_sdk_id_error(row.get("sdk")) is not None
                    or not _is_nonzero_hex32(row.get("implementation_hash"))
                    or row.get("implementation") != expected_implementation
                ):
                    continue
                copied = dict(row)
                copied["implementation_artifact"] = (
                    _redacted_native_evm_artifact_summary(
                        copied.get("implementation_artifact"),
                        redact_unsafe_path=blocked,
                    )
                )
                if blocked and not isinstance(copied.get("implementation_artifact"), dict):
                    continue
                copied_rows.append(copied)
            else:
                copied_rows.append(row)
        redacted["sdk_artifacts"] = copied_rows
    return redacted


def _public_source_inventory_gate_error(gate: Any) -> str | None:
    """Return a bounded blocker for malformed public source-inventory gates."""

    label = "readiness report source_inventory"
    if not isinstance(gate, str) or not gate:
        return f"{label} contains malformed gate name"
    if _path_control_character(gate) is not None:
        return f"{label} contains gate name with control character"
    if not gate.isascii():
        return f"{label} contains gate name with non-ASCII character"
    if gate.strip() != gate:
        return f"{label} contains gate name with surrounding whitespace"
    if any(character.isspace() for character in gate):
        return f"{label} contains gate name with whitespace"
    if _path_markdown_unsafe_character(gate) is not None:
        return f"{label} contains gate name with Markdown-unsafe character"
    if any(marker in gate.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        return f"{label} contains gate name with sensitive name"
    if not _source_inventory_gate_is_markdown_safe(gate):
        return f"{label} contains malformed gate name"
    return None


def _source_inventory_required_public_gates() -> frozenset[str]:
    """Return the strict source-inventory gate set used by the bundle verifier."""

    verifier = _load_release_bundle_verify_helpers()
    gates = getattr(verifier, "SOURCE_INVENTORY_REQUIRED_GATES")
    if not isinstance(gates, (set, frozenset)) or not all(
        isinstance(gate, str) for gate in gates
    ):
        raise RuntimeError("release-bundle verifier source-inventory gates are invalid")
    return frozenset(gates)


def _public_source_inventory_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public source-inventory rows."""

    if not isinstance(value, dict):
        return ["readiness report source_inventory must be an object"]

    errors: list[str] = []
    required_gates = frozenset()
    try:
        required_gates = _source_inventory_required_public_gates()
    except (Exception, SystemExit):  # pragma: no cover - exercised through blocker text.
        errors.append(
            "readiness report source_inventory required gate inventory cannot run "
            "release-bundle verifier helper"
        )

    if required_gates:
        for gate in sorted(set(value) - required_gates, key=str):
            if _public_source_inventory_gate_error(gate) is None:
                errors.append(
                    f"readiness report source_inventory contains unknown gate: {gate}"
                )
        for gate in sorted(required_gates - set(value)):
            errors.append(
                f"readiness report source_inventory missing required gate: {gate}"
            )

    for index, (gate, inventory) in enumerate(
        sorted(value.items(), key=lambda item: str(item[0]))
    ):
        gate_error = _public_source_inventory_gate_error(gate)
        if gate_error is not None:
            errors.append(gate_error)
        inventory_label = f"readiness report source_inventory[{index}]"
        if not isinstance(inventory, dict):
            errors.append(f"{inventory_label} must be an object")
            continue
        for field in sorted(
            (
                field
                for field in inventory
                if field not in SOURCE_INVENTORY_PUBLIC_FIELDS
            ),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(
                    inventory_label,
                    field,
                    "unknown",
                )
            )
        for field in sorted(SOURCE_INVENTORY_PUBLIC_FIELDS - set(inventory)):
            errors.append(f"{inventory_label} missing field: {field}")
        validation_status = inventory.get("validation_status")
        if "validation_status" in inventory and validation_status not in {"passed", "blocked"}:
            errors.append(
                f"{inventory_label} validation_status must be passed or blocked"
            )
        validation_blockers = inventory.get("validation_blockers")
        if "validation_blockers" in inventory:
            if not isinstance(validation_blockers, list):
                errors.append(
                    f"{inventory_label} validation_blockers must be a list of "
                    "non-empty canonical strings"
                )
            else:
                for blocker_index, blocker in enumerate(validation_blockers):
                    issue = _public_blocker_text_issue(blocker)
                    if issue is None:
                        continue
                    if issue == "non-empty canonical string":
                        errors.append(
                            f"{inventory_label} validation_blockers[{blocker_index}] "
                            f"must be a {issue}"
                        )
                    else:
                        errors.append(
                            f"{inventory_label} validation_blockers[{blocker_index}] "
                            f"contains {issue}"
                        )
                duplicate_error = _public_blocker_list_duplicate_error(
                    validation_blockers,
                    f"{inventory_label} validation_blockers",
                )
                if duplicate_error is not None:
                    errors.append(duplicate_error)
                if validation_status == "passed" and validation_blockers:
                    errors.append(
                        f"{inventory_label} validation_blockers must be empty when "
                        "validation_status is passed"
                    )
                if validation_status == "blocked" and not validation_blockers:
                    errors.append(
                        f"{inventory_label} validation_blockers must be non-empty "
                        "when validation_status is blocked"
                    )
    return errors


def _public_user_prover_submission_surface_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public user-prover rows."""

    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        return [
            "readiness report user_prover_submission_surfaces must be a list of objects"
        ]

    errors: list[str] = []
    seen_lanes: set[str] = set()
    expected_lanes = tuple(
        surface["lanes"] for surface in USER_PROVER_SUBMISSION_SURFACES
    )
    for index, surface in enumerate(value):
        surface_label = f"readiness report user_prover_submission_surfaces[{index}]"
        for field in sorted(
            (
                field
                for field in surface
                if field not in USER_PROVER_SUBMISSION_SURFACE_PUBLIC_FIELDS
            ),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(
                    surface_label,
                    field,
                    "unknown",
                )
            )
        for field in sorted(
            USER_PROVER_SUBMISSION_SURFACE_PUBLIC_FIELDS - set(surface)
        ):
            errors.append(f"{surface_label} missing field: {field}")

        lanes = surface.get("lanes")
        if not isinstance(lanes, str) or lanes not in USER_PROVER_REQUIRED_LANE_BACKENDS:
            errors.append(f"{surface_label} lanes must be a required lane set")
            expected_backend = None
            expected_submission = None
            expected_phases = None
            expected_helper_sets = None
        else:
            expected_backend = USER_PROVER_REQUIRED_LANE_BACKENDS[lanes]
            expected_submission = USER_PROVER_ON_CHAIN_SUBMISSION_BY_LANE[lanes]
            expected_phases = USER_PROVER_REQUIRED_PHASES_BY_LANE[lanes]
            expected_helper_sets = USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK[lanes]
            if lanes in seen_lanes:
                errors.append(f"{surface_label} lanes is duplicated")
            else:
                seen_lanes.add(lanes)

        if surface.get("proof_backend") != expected_backend:
            errors.append(f"{surface_label} proof_backend must match the required lane")
        if surface.get("on_chain_submission") != expected_submission:
            errors.append(
                f"{surface_label} on_chain_submission must match the required lane"
            )

        helper_symbols = surface.get("sdk_helper_symbols")
        expected_js_helpers = (
            expected_helper_sets.get("js-sdk")
            if isinstance(expected_helper_sets, dict)
            else None
        )
        if (
            expected_js_helpers is None
            or not isinstance(helper_symbols, list)
            or tuple(helper_symbols) != expected_js_helpers
            or any(not _helper_symbol_is_markdown_safe(helper) for helper in helper_symbols)
        ):
            errors.append(f"{surface_label} sdk_helper_symbols must match expected helpers")
        if surface.get("sdk_helpers") != (
            ", ".join(expected_js_helpers) if expected_js_helpers is not None else None
        ):
            errors.append(f"{surface_label} sdk_helpers must match sdk_helper_symbols")

        helper_sets = surface.get("sdk_helper_symbols_by_sdk")
        if not isinstance(helper_sets, dict) or not isinstance(
            expected_helper_sets,
            dict,
        ):
            errors.append(
                f"{surface_label} sdk_helper_symbols_by_sdk must be an object"
            )
        else:
            if set(helper_sets) != set(expected_helper_sets):
                errors.append(
                    f"{surface_label} sdk_helper_symbols_by_sdk must contain the "
                    "required SDKs"
                )
            for sdk, expected_helpers in expected_helper_sets.items():
                helpers = helper_sets.get(sdk)
                if (
                    not isinstance(helpers, list)
                    or tuple(helpers) != expected_helpers
                    or any(
                        not _helper_symbol_is_markdown_safe(helper)
                        for helper in helpers
                    )
                ):
                    errors.append(
                        f"{surface_label} sdk_helper_symbols_by_sdk[{sdk}] "
                        "must match expected helpers"
                    )

        required_phases = surface.get("required_phases")
        if (
            expected_phases is None
            or not isinstance(required_phases, list)
            or tuple(required_phases) != expected_phases
            or any(_public_blocker_text_issue(phase) is not None for phase in required_phases)
        ):
            errors.append(f"{surface_label} required_phases must match expected phases")

        validation_status = surface.get("validation_status")
        if validation_status not in {"passed", "blocked"}:
            errors.append(f"{surface_label} validation_status must be passed or blocked")
        validation_blockers = surface.get("validation_blockers")
        if not isinstance(validation_blockers, list):
            errors.append(
                f"{surface_label} validation_blockers must be a list of "
                "non-empty canonical strings"
            )
        else:
            for blocker_index, blocker in enumerate(validation_blockers):
                issue = _public_blocker_text_issue(blocker)
                if issue is None:
                    continue
                if issue == "non-empty canonical string":
                    errors.append(
                        f"{surface_label} validation_blockers[{blocker_index}] "
                        f"must be a {issue}"
                    )
                else:
                    errors.append(
                        f"{surface_label} validation_blockers[{blocker_index}] "
                        f"contains {issue}"
                    )
            duplicate_error = _public_blocker_list_duplicate_error(
                validation_blockers,
                f"{surface_label} validation_blockers",
            )
            if duplicate_error is not None:
                errors.append(duplicate_error)
            if validation_status == "passed" and validation_blockers:
                errors.append(
                    f"{surface_label} validation_blockers must be empty when "
                    "validation_status is passed"
                )
            if validation_status == "blocked" and not validation_blockers:
                errors.append(
                    f"{surface_label} validation_blockers must be non-empty when "
                    "validation_status is blocked"
                )
    for lanes in expected_lanes:
        if lanes not in seen_lanes:
            errors.append(
                "readiness report user_prover_submission_surfaces missing "
                f"lane set {lanes}"
            )
    return errors


def _public_crypto_text_is_safe(value: Any) -> bool:
    return (
        value is None
        or value == ""
        or (
            isinstance(value, str)
            and _public_blocker_text_issue(value) is None
            and not any(character.isspace() for character in value)
        )
    )


def _public_cryptographic_evidence_errors(value: Any) -> list[str]:
    """Return bounded blockers for malformed public cryptographic evidence rows."""

    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        return ["readiness report cryptographic_evidence must be a list of objects"]

    errors: list[str] = []
    seen_domains: set[int] = set()
    for index, row in enumerate(value):
        row_label = f"readiness report cryptographic_evidence[{index}]"
        for field in sorted(
            (
                field
                for field in row
                if field not in CRYPTOGRAPHIC_EVIDENCE_PUBLIC_FIELDS
            ),
            key=str,
        ):
            errors.append(
                _native_evm_prover_field_name_blocker(
                    row_label,
                    field,
                    "unknown",
                )
            )
        for field in sorted(CRYPTOGRAPHIC_EVIDENCE_PUBLIC_FIELDS - set(row)):
            errors.append(f"{row_label} missing field: {field}")

        domain = row.get("domain")
        if type(domain) is not int:
            errors.append(f"{row_label} domain must be an integer")
        elif domain in seen_domains:
            errors.append(
                f"readiness report cryptographic_evidence contains duplicate domain: {domain}"
            )
        else:
            seen_domains.add(domain)
            if domain not in ALL_LANES_CHAIN_BY_DOMAIN:
                errors.append(
                    f"readiness report cryptographic_evidence contains unknown domain: {domain}"
                )
        chain = row.get("chain")
        if (
            type(domain) is not int
            or not isinstance(chain, str)
            or chain != ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
        ):
            errors.append(f"{row_label} chain must match the domain")

        for field in sorted(CRYPTOGRAPHIC_EVIDENCE_TEXT_FIELDS):
            if field in row and not _public_crypto_text_is_safe(row.get(field)):
                errors.append(f"{row_label} {field} must be a canonical public string")
        for field in sorted(CRYPTOGRAPHIC_EVIDENCE_HASH_FIELDS):
            value_for_field = row.get(field)
            if value_for_field in (None, ""):
                continue
            if not _is_nonzero_hex32(value_for_field):
                errors.append(
                    f"{row_label} {field} must be a canonical non-zero bytes32 "
                    "hex string"
                )
        for field in sorted(CRYPTOGRAPHIC_EVIDENCE_INTEGER_FIELDS):
            value_for_field = row.get(field)
            if value_for_field is None:
                continue
            if type(value_for_field) is not int:
                errors.append(f"{row_label} {field} must be an integer")
        for field in (
            "route_canary_log_index",
            "route_canary_target_domain",
            "route_canary_proof_version",
            "route_canary_proof_source_domain",
        ):
            value_for_field = row.get(field)
            if value_for_field is None:
                continue
            if (
                type(value_for_field) is not int
                or value_for_field < 0
                or value_for_field > 0xFFFF_FFFF
            ):
                errors.append(f"{row_label} {field} must be a non-negative u32 integer")
        errors.extend(
            _public_cryptographic_route_canary_hash_role_errors(row_label, row)
        )
        if type(row.get("route_canary_evidence_bound")) is not bool:
            errors.append(f"{row_label} route_canary_evidence_bound must be boolean")
        if (
            row.get("route_canary_message_proof_used") is not None
            and type(row.get("route_canary_message_proof_used")) is not bool
        ):
            errors.append(
                f"{row_label} route_canary_message_proof_used must be boolean"
            )
        for field in (
            "route_canary_raw_data_owner_matches_transaction",
            "route_canary_signature_recovers_to_owner",
        ):
            if row.get(field) is not None and type(row.get(field)) is not bool:
                errors.append(f"{row_label} {field} must be boolean")
        if (
            row.get("route_canary_receipt_block_finalized") is not None
            and type(row.get("route_canary_receipt_block_finalized")) is not bool
        ):
            errors.append(
                f"{row_label} route_canary_receipt_block_finalized must be boolean"
            )
        has_route_canary_evidence = bool(row.get("route_canary_evidence_hash"))
        has_message_proof_route_canary_evidence = (
            type(domain) is int
            and domain in MESSAGE_PROOF_ROUTE_CANARY_DOMAINS
            and has_route_canary_evidence
        )
        if (
            has_message_proof_route_canary_evidence
            and row.get("route_canary_message_proof_used") is not True
        ):
            # Source-inventory marker: route_canary_message_proof_used must be true for message-proof route canary evidence
            errors.append(
                f"{row_label} route_canary_message_proof_used must be true for "
                "message-proof route canary evidence"
            )
        if has_message_proof_route_canary_evidence:
            for field in (
                "route_canary_call_data_sha256",
                "route_canary_payload_hash",
                "route_canary_statement_hash",
                "route_canary_commitment_root",
                "route_canary_finality_height",
                "route_canary_finality_block_hash",
            ):
                if not _is_nonzero_hex32(row.get(field)):
                    # Source-inventory marker: route-canary public transcript hashes must be non-zero bytes32 for message-proof route canary evidence
                    errors.append(
                        f"{row_label} {field} must be a canonical non-zero "
                        "bytes32 hex string for message-proof route canary evidence"
                    )
            scalar_expectations = (
                ("route_canary_log_index", None, "a non-negative u32 integer"),
                ("route_canary_target_domain", domain, "the lane domain"),
                ("route_canary_proof_version", 1, "1"),
                ("route_canary_proof_source_domain", SCCP_DOMAIN_SORA, "SORA"),
            )
            for field, expected, expected_label in scalar_expectations:
                value_for_field = row.get(field)
                if expected is None:
                    if (
                        type(value_for_field) is not int
                        or value_for_field < 0
                        or value_for_field > 0xFFFF_FFFF
                    ):
                        # Source-inventory marker: route-canary public scalar proof context must be exact for message-proof route canary evidence
                        errors.append(
                            f"{row_label} {field} must be {expected_label} "
                            "for message-proof route canary evidence"
                        )
                elif value_for_field != expected:
                    # Source-inventory marker: route-canary public scalar proof context must be exact for message-proof route canary evidence
                    errors.append(
                        f"{row_label} {field} must be {expected_label} for "
                        "message-proof route canary evidence"
                    )
        if (
            type(domain) is int
            and domain not in MESSAGE_PROOF_ROUTE_CANARY_DOMAINS
            and row.get("route_canary_message_proof_used") is not None
        ):
            # Source-inventory marker: route_canary_message_proof_used must be null for lanes without message-proof route canary evidence
            errors.append(
                f"{row_label} route_canary_message_proof_used must be null for "
                "lanes without message-proof route canary evidence"
            )
        if type(domain) is int and domain not in MESSAGE_PROOF_ROUTE_CANARY_DOMAINS:
            for field in (
                "route_canary_log_index",
                "route_canary_target_domain",
                "route_canary_proof_version",
                "route_canary_proof_source_domain",
                "route_canary_call_data_sha256",
                "route_canary_payload_hash",
                "route_canary_statement_hash",
                "route_canary_commitment_root",
                "route_canary_finality_height",
                "route_canary_finality_block_hash",
            ):
                if row.get(field) is not None:
                    # Source-inventory marker: route-canary public transcript proof context must be null for lanes without message-proof route canary evidence
                    # Source-inventory marker: route-canary public scalar proof context must be null for lanes without message-proof route canary evidence
                    errors.append(
                        f"{row_label} {field} must be null for lanes without "
                        "message-proof route canary evidence"
                    )
        if (
            type(domain) is int
            and domain == 5
            and has_route_canary_evidence
        ):
            for field in (
                "route_canary_raw_data_owner_matches_transaction",
                "route_canary_signature_recovers_to_owner",
            ):
                if row.get(field) is not True:
                    # Source-inventory marker: TRON route-canary public owner/signature flags must be true for TRON route canary evidence
                    errors.append(
                        f"{row_label} {field} must be true for TRON route "
                        "canary evidence"
                    )
        if (
            type(domain) is int
            and domain != 5
        ):
            for field in (
                "route_canary_raw_data_owner_matches_transaction",
                "route_canary_signature_recovers_to_owner",
            ):
                if row.get(field) is not None:
                    # Source-inventory marker: TRON route-canary public owner/signature flags must be null for non-TRON lanes
                    errors.append(
                        f"{row_label} {field} must be null for non-TRON "
                        "route canary evidence"
                    )
        if type(row.get("source_adapter_gate_required")) is not bool:
            errors.append(f"{row_label} source_adapter_gate_required must be boolean")

        audit_hashes = row.get("source_adapter_gate_audit_hashes")
        semantic_audit_hashes: dict[str, Any] = {}
        if not isinstance(audit_hashes, dict):
            if audit_hashes not in (None, {}):
                errors.append(
                    f"{row_label} source_adapter_gate_audit_hashes must be an object"
                )
        else:
            for audit_field, audit_hash in sorted(
                audit_hashes.items(),
                key=lambda item: str(item[0]),
            ):
                audit_label = f"{row_label} source_adapter_gate_audit_hashes"
                if not isinstance(audit_field, str):
                    errors.append(f"{audit_label} contains malformed audit field name")
                    continue
                if _public_blocker_text_issue(audit_field) is not None:
                    errors.append(f"{audit_label} contains malformed audit field name")
                    continue
                semantic_audit_hashes[audit_field] = audit_hash
                if not _is_nonzero_hex32(audit_hash):
                    errors.append(
                        f"{audit_label} {audit_field} must be a canonical non-zero "
                        "bytes32 hex string"
                    )
        errors.extend(
            _public_cryptographic_source_adapter_gate_hash_role_errors(
                row_label,
                row,
                semantic_audit_hashes,
            )
        )
        errors.extend(
            _public_cryptographic_source_adapter_gate_template_hash_errors(
                row_label,
                domain,
                row.get("source_adapter_gate_hash"),
                semantic_audit_hashes,
            )
        )

        gate_required = row.get("source_adapter_gate_required")
        expected_audit_keys = (
            ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.get(domain)
            if type(domain) is int and chain == ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
            else None
        )
        gate_hash = row.get("source_adapter_gate_hash")
        if gate_required is True:
            audit_label = f"{row_label} source_adapter_gate_audit_hashes"
            if expected_audit_keys is None:
                errors.append(
                    f"{row_label} source_adapter_gate_required must be false "
                    "for this domain"
                )
            else:
                for audit_field in sorted(set(semantic_audit_hashes) - expected_audit_keys):
                    errors.append(f"{audit_label} contains unexpected field: {audit_field}")
                for audit_field in sorted(expected_audit_keys - set(semantic_audit_hashes)):
                    errors.append(f"{audit_label} missing field: {audit_field}")
            if not gate_hash:
                errors.append(
                    f"{row_label} source_adapter_gate_hash must not be empty when required"
                )
            if not semantic_audit_hashes:
                errors.append(
                    f"{audit_label} must not be empty when required"
                )
            if (
                _is_nonzero_hex32(gate_hash)
                and semantic_audit_hashes
                and gate_hash not in set(semantic_audit_hashes.values())
            ):
                errors.append(
                    f"{row_label} source_adapter_gate_hash must match one "
                    "source_adapter_gate_audit_hashes value"
                )
            expected_gate_key = (
                ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN.get(domain)
                if type(domain) is int and chain == ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
                else None
            )
            expected_gate_hash = (
                semantic_audit_hashes.get(expected_gate_key)
                if expected_gate_key is not None
                else None
            )
            if (
                expected_gate_key is not None
                and _is_nonzero_hex32(gate_hash)
                and _is_nonzero_hex32(expected_gate_hash)
                and gate_hash != expected_gate_hash
            ):
                errors.append(
                    f"{row_label} source_adapter_gate_hash must match "
                    f"source_adapter_gate_audit_hashes.{expected_gate_key}"
                )
        elif gate_required is False:
            if expected_audit_keys is not None:
                errors.append(
                    f"{row_label} source_adapter_gate_required must be true for this domain"
                )
            if gate_hash not in (None, ""):
                # Source-inventory marker: source_adapter_gate_hash must be empty when gate is not required
                errors.append(
                    f"{row_label} source_adapter_gate_hash must be empty when "
                    "gate is not required"
                )
            if audit_hashes:
                # Source-inventory marker: source_adapter_gate_audit_hashes must be empty when gate is not required
                errors.append(
                    f"{row_label} source_adapter_gate_audit_hashes must be empty "
                    "when gate is not required"
                )
    for domain in ALL_LANES_REQUIRED_DOMAINS:
        if domain not in seen_domains:
            errors.append(
                f"readiness report cryptographic_evidence missing required domain: {domain}"
            )
    return errors


def _public_cryptographic_source_adapter_gate_hash_role_errors(
    row_label: str,
    row: dict[str, Any],
    semantic_audit_hashes: dict[str, Any],
) -> list[str]:
    """Return public-row blockers when source-gate audit hashes reuse roles."""

    errors: list[str] = []
    seen: dict[str, str] = {}
    fields: list[tuple[str, Any]] = [
        ("source_verifier_material_hash", row.get("source_verifier_material_hash")),
        (
            "source_adapter_engine_deployment_hash",
            row.get("source_adapter_engine_deployment_hash"),
        ),
        ("destination_binding_hash", row.get("destination_binding_hash")),
        ("route_allowlist_hash", row.get("route_allowlist_hash")),
        ("route_canary_evidence_hash", row.get("route_canary_evidence_hash")),
        # Source-inventory marker: public source_adapter_gate audit hashes must not reuse route_canary_message_id
        ("route_canary_transaction_hash", row.get("route_canary_transaction_hash")),
        (
            "route_canary_receipt_block_hash",
            row.get("route_canary_receipt_block_hash"),
        ),
        (
            "route_canary_block_receipts_root",
            row.get("route_canary_block_receipts_root"),
        ),
        ("route_canary_message_id", row.get("route_canary_message_id")),
    ]
    fields.extend(
        (f"source_adapter_gate_audit_hashes.{field}", value)
        for field, value in sorted(semantic_audit_hashes.items())
    )
    for field, value in fields:
        if not _is_nonzero_hex32(value):
            continue
        assert isinstance(value, str)
        prior_field = seen.get(value)
        if prior_field is not None:
            errors.append(
                f"{row_label} source_adapter_gate hash role {field} "
                f"must not reuse {prior_field}"
            )
            continue
        seen[value] = field
    return errors


def _public_cryptographic_route_canary_hash_role_errors(
    row_label: str,
    row: dict[str, Any],
) -> list[str]:
    """Return public-row blockers when route-canary transcript hashes are replayed."""

    errors: list[str] = []
    seen: dict[str, str] = {}
    for field in (
        "route_canary_transaction_hash",
        "route_canary_receipt_block_hash",
        "route_canary_block_receipts_root",
        "route_canary_message_id",
        "route_canary_evidence_hash",
    ):
        value = row.get(field)
        if not _is_nonzero_hex32(value):
            continue
        assert isinstance(value, str)
        prior_field = seen.get(value)
        if prior_field is not None:
            errors.append(
                f"{row_label} route_canary hash role {field} "
                f"must not reuse {prior_field}"
            )
            continue
        seen[value] = field
    return errors


def _public_report_payload(report: Any) -> dict[str, Any]:
    """Return a fail-closed public readiness report payload."""

    if not isinstance(report, dict):
        return {
            "production_ready": False,
            "blockers": ["readiness report must be an object"],
        }
    report = dict(report)

    blockers: list[str] = []
    public_report_fields = set(READINESS_REPORT_PUBLIC_FIELDS)
    for field in sorted(
        (field for field in report if field not in public_report_fields),
        key=str,
    ):
        blockers.append(_readiness_report_unknown_field_blocker(field))

    if "blockers" not in report:
        blockers.append("readiness report blockers missing")
    else:
        public_blockers, blocker_errors = _canonical_public_report_blockers(
            report.get("blockers"),
        )
        blockers.extend(public_blockers)
        blockers.extend(blocker_errors)

    production_ready = report.get("production_ready")
    if type(production_ready) is not bool:
        blockers.append("readiness report production_ready must be boolean")

    root_errors: dict[str, str] = {}
    object_root_messages = {
        "evidence": "readiness report evidence must be an object",
        "release_checklist": "readiness report release_checklist must be an object",
        "corridor": "readiness report corridor must be an object",
        "source_inventory": "readiness report source_inventory must be an object",
    }
    for field, message in object_root_messages.items():
        if field in report and not isinstance(report.get(field), dict):
            root_errors[field] = message
    inputs = report.get("inputs")
    public_input_paths: list[str] | None = None
    if "inputs" in report:
        if not isinstance(inputs, list) or not all(
            isinstance(item, str)
            and item
            and _public_blocker_text_issue(item) is None
            for item in inputs
        ):
            root_errors["inputs"] = (
                "readiness report inputs must be a list of canonical strings"
            )
        elif not all(_native_evm_markdown_path_is_safe(item) for item in inputs):
            root_errors["inputs"] = (
                "readiness report inputs must be a list of canonical public paths"
            )
        elif not inputs:
            root_errors["inputs"] = (
                "readiness report inputs must be a non-empty list of canonical strings"
            )
        elif len(set(inputs)) != len(inputs):
            root_errors["inputs"] = "readiness report inputs contains duplicate path"
        else:
            public_input_paths = list(inputs)

    input_artifacts = report.get("input_artifacts")
    if "input_artifacts" in report:
        if not isinstance(input_artifacts, list) or not all(
            isinstance(item, dict) for item in input_artifacts
        ):
            root_errors["input_artifacts"] = (
                "readiness report input_artifacts must be a list of objects"
            )
        elif not input_artifacts:
            root_errors["input_artifacts"] = (
                "readiness report input_artifacts must be a non-empty list of objects"
            )

    list_root_messages = {
        "cryptographic_evidence": (
            "readiness report cryptographic_evidence must be a list of objects"
        ),
        "user_prover_submission_surfaces": (
            "readiness report user_prover_submission_surfaces must be a list of objects"
        ),
    }
    for field, message in list_root_messages.items():
        value = report.get(field)
        if field in report and (
            not isinstance(value, list) or not all(isinstance(item, dict) for item in value)
        ):
            root_errors[field] = message
    native_bundle = report.get("native_evm_prover_bundle")
    if "native_evm_prover_bundle" in report and not isinstance(native_bundle, dict):
        root_errors["native_evm_prover_bundle"] = (
            "readiness report native_evm_prover_bundle must be an object"
        )
    elif isinstance(native_bundle, dict):
        native_bundle = _redacted_public_native_evm_prover_bundle(native_bundle)
        report["native_evm_prover_bundle"] = native_bundle
    if "input_artifacts" in report and "input_artifacts" not in root_errors:
        input_artifact_errors = _public_input_artifact_errors(
            report.get("input_artifacts")
        )
        if input_artifact_errors:
            blockers.extend(input_artifact_errors)
            root_errors["input_artifacts"] = (
                "readiness report input_artifacts is invalid"
            )
    if (
        public_input_paths is not None
        and "input_artifacts" in report
        and "input_artifacts" not in root_errors
        and isinstance(input_artifacts, list)
    ):
        artifact_paths = [artifact.get("path") for artifact in input_artifacts]
        if artifact_paths != public_input_paths:
            root_errors["inputs"] = (
                "readiness report inputs do not match copied input_artifacts"
            )
            root_errors["input_artifacts"] = (
                "readiness report input_artifacts do not match inputs"
            )
    if "corridor" in report and "corridor" not in root_errors:
        corridor_errors = _public_corridor_errors(report.get("corridor"))
        if corridor_errors:
            blockers.extend(corridor_errors)
            root_errors["corridor"] = "readiness report corridor is invalid"
    if (
        "native_evm_prover_bundle" in report
        and "native_evm_prover_bundle" not in root_errors
    ):
        native_bundle_errors = _public_native_evm_prover_bundle_errors(
            report.get("native_evm_prover_bundle")
        )
        if native_bundle_errors:
            blockers.extend(native_bundle_errors)
            root_errors["native_evm_prover_bundle"] = (
                "readiness report native_evm_prover_bundle is invalid"
            )
    if "evidence" in report and "evidence" not in root_errors:
        evidence_errors = _public_embedded_evidence_errors(report.get("evidence"))
        if evidence_errors:
            blockers.extend(evidence_errors)
            root_errors["evidence"] = "readiness report evidence is invalid"
    if "source_inventory" in report and "source_inventory" not in root_errors:
        source_inventory_errors = _public_source_inventory_errors(
            report.get("source_inventory")
        )
        if source_inventory_errors:
            blockers.extend(source_inventory_errors)
            root_errors["source_inventory"] = (
                "readiness report source_inventory is invalid"
            )
    if (
        "user_prover_submission_surfaces" in report
        and "user_prover_submission_surfaces" not in root_errors
    ):
        user_prover_errors = _public_user_prover_submission_surface_errors(
            report.get("user_prover_submission_surfaces")
        )
        if user_prover_errors:
            blockers.extend(user_prover_errors)
            root_errors["user_prover_submission_surfaces"] = (
                "readiness report user_prover_submission_surfaces is invalid"
            )
    if "cryptographic_evidence" in report and "cryptographic_evidence" not in root_errors:
        cryptographic_evidence_errors = _public_cryptographic_evidence_errors(
            report.get("cryptographic_evidence")
        )
        if cryptographic_evidence_errors:
            blockers.extend(cryptographic_evidence_errors)
            root_errors["cryptographic_evidence"] = (
                "readiness report cryptographic_evidence is invalid"
            )
    if "release_checklist" in report and "release_checklist" not in root_errors:
        release_checklist_errors = _public_release_checklist_errors(
            report.get("release_checklist")
        )
        if release_checklist_errors:
            blockers.extend(release_checklist_errors)
            root_errors["release_checklist"] = (
                "readiness report release_checklist is invalid"
            )
    blockers.extend(root_errors.values())

    public_report = {
        field: report[field]
        for field in READINESS_REPORT_PUBLIC_FIELDS
        if field in report and field not in root_errors
    }
    public_report["production_ready"] = production_ready is True and not blockers
    public_report["blockers"] = blockers
    return public_report


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.max_blockers_per_lane < 1:
        parser.error("--max-blockers-per-lane must be positive")
    if args.output is not None:
        output_path_error = _readiness_output_path_error(str(args.output))
        if output_path_error is not None:
            parser.error(output_path_error)

    try:
        report = _build_report(
            args.toml,
            args.phase_result,
            args.phase_evidence,
            require_phase_evidence=args.require_phase_evidence,
            phase_evidence_dir=args.phase_evidence_dir,
            native_evm_prover_bundle=args.native_evm_prover_bundle,
        )
    except (
        argparse.ArgumentTypeError,
        OSError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP release readiness report generation failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")

    report = _public_report_payload(report)
    if args.format == "json":
        output = json.dumps(report, indent=2, sort_keys=True) + "\n"
    else:
        output = _render_markdown(
            report,
            max_blockers_per_lane=args.max_blockers_per_lane,
        )

    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")
    return 0 if report["production_ready"] is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
