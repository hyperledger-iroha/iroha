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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
        return [
            "SCCP public discovery documentation source inventory "
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
    except Exception:  # pragma: no cover - exercised through blocker text.
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
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests",
        "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
        "org.hyperledger.iroha.android.sccp.TonSccpProverTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
    ),
    "dotnet-sdk": (
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
    ),
    "contract-smoke": (
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
    "dotnet-sdk": ("Passed!",),
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
    ),
    "evidence-scripts": (
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
    ),
    "js-sdk": (re.compile(r"\bfail\s+[1-9]\d*\b", re.IGNORECASE),),
    "python-sdk": (
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+errors?\b", re.IGNORECASE),
    ),
    "swift-sdk": (re.compile(r"\b[1-9]\d*\s+failures?\b", re.IGNORECASE),),
    "kotlin-sdk": (re.compile(r"\bBUILD FAILED\b", re.IGNORECASE),),
    "java-android": (re.compile(r"\bBUILD FAILED\b", re.IGNORECASE),),
    "dotnet-sdk": (
        re.compile(r"\bFailed!\b", re.IGNORECASE),
        re.compile(r"\bFailed:\s*[1-9]\d*\b", re.IGNORECASE),
    ),
    "contract-smoke": (re.compile(r"\bfail\s+[1-9]\d*\b", re.IGNORECASE),),
    "core-admission": (
        re.compile(r"\btest result:\s+FAILED\b", re.IGNORECASE),
        re.compile(r"\b[1-9]\d*\s+failed\b", re.IGNORECASE),
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
    if any(marker in raw_name.lower() for marker in SENSITIVE_PUBLIC_FIELD_NAME_MARKERS):
        raise argparse.ArgumentTypeError(f"{label} name contains sensitive name")
    allowed = set("abcdefghijklmnopqrstuvwxyz0123456789-")
    if (
        any(character not in allowed for character in raw_name)
        or raw_name.startswith("-")
        or raw_name.endswith("-")
    ):
        raise argparse.ArgumentTypeError(f"{label} name contains malformed phase")
    return raw_name


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
    except (TypeError, ValueError):
        return False
    return len(raw) == 32 and any(raw) and value == f"0x{raw.hex()}"


def _is_hex32(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) != 66:
        return False
    try:
        raw = bytes.fromhex(value[2:])
    except (TypeError, ValueError):
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
    markdown_unsafe_character = _path_markdown_unsafe_character(value)
    if markdown_unsafe_character is not None:
        return None, [
            f"{prefix} path contains Markdown-unsafe character "
            f"{markdown_unsafe_character}"
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
    "private-key",
    "private_key",
    "password",
    "bearer",
    "authorization",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
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
        "validation_status": "passed" if not blockers else "blocked",
        "validation_blockers": blockers,
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
            if not any(
                position > first_phase_command_position
                for position in success_positions
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
    return [
        line.strip()
        for line in phase_block.splitlines()
        if line.lstrip().startswith("+ ")
    ]


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
        return [
            normalized
            for token in shlex.split(command, comments=True)
            if (normalized := token.strip("()"))
        ]
    except ValueError:
        return []


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
    if len(tokens) < 3 or _command_token_basename(tokens[0]) != "node":
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
        and _command_token_basename(tokens[0]) == "node"
        and tokens[1] == "--check"
        and tokens[2] == expected_path
    )


def _dotnet_sdk_command_matches(tokens: list[str]) -> bool:
    if len(tokens) != 6 or _command_token_basename(tokens[0]) != "dotnet":
        return False
    project_fragment, filter_fragment = PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "dotnet-sdk"
    ]
    project_tokens = shlex.split(project_fragment)
    expected_project = project_tokens[2] if len(project_tokens) > 2 else ""
    return (
        tokens[1] == "test"
        and tokens[2] == expected_project
        and tokens[3] == "--filter"
        and tokens[4].replace("\\|", "|") == filter_fragment.replace("\\|", "|")
        and tokens[5] == "--nologo"
    )


def _gradle_test_selector_matches(expected: str, actual: str) -> bool:
    if expected.endswith("."):
        return actual in (expected, f"{expected}*")
    return actual == expected


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


def _evidence_pytest_command_has_fragment(
    phase: str,
    command: str,
    fragment: str,
) -> bool:
    tokens = _phase_effective_command_tokens(command)
    if not tokens or not _command_token_basename(tokens[0]).startswith("python"):
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
        expected_harness_classes = [
            item for item in fragment.partition("=")[2].split(",") if item
        ]
        return bool(expected_harness_classes) and all(
            item in actual_harness_classes for item in expected_harness_classes
        ) and "org.hyperledger.iroha.android.GradleHarnessTests" in test_selectors
    if (
        fragment.startswith("org.hyperledger.iroha.android.sccp.")
        and fragment.endswith("Tests")
    ):
        return (
            fragment in actual_harness_classes
            and "org.hyperledger.iroha.android.GradleHarnessTests" in test_selectors
        )
    fragment_tokens = shlex.split(fragment)
    expected_tests = _command_option_values(fragment_tokens, "--tests")
    if expected_tests:
        return (
            all(part in tokens for part in fragment_tokens if part not in expected_tests)
            and all(
                expected in test_selectors
                for expected in expected_tests
            )
        )
    return all(part in tokens for part in shlex.split(fragment))


def _dotnet_sdk_command_has_fragment(command: str, fragment: str) -> bool:
    tokens = _phase_effective_command_tokens(command)
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
    if fragment not in command:
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


def _phase_block_has_output_fragment(phase_block: str, fragment: str) -> bool:
    return bool(_phase_block_output_fragment_line_indices(phase_block, fragment))


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
        _phase_block_output_fragment_line_indices(phase_block, fragment)
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
                any(
                    first_command_position < position < completion_position
                    for position in positions
                )
                for positions in success_positions_by_fragment
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


def _phase_transcript_errors(phase: str, artifact: dict[str, Any]) -> list[str]:
    path = Path(str(artifact["path"]))
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
        for fragment in success_fragments:
            if not _phase_block_has_output_fragment(phase_block, fragment):
                errors.append(
                    "evidence artifact is missing expected phase-block success marker: "
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
        for blocker in evidence_blockers:
            if (
                not isinstance(blocker, str)
                or not blocker
                or blocker.strip() != blocker
            ):
                add("SCCP evidence blocker must be a non-empty canonical string")
                continue
            if blocker.startswith(prefix):
                add(blocker)
            elif not blocker.startswith("domain "):
                add(blocker)
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
    for blocker in lane_blockers:
        if (
            not isinstance(blocker, str)
            or not blocker
            or blocker.strip() != blocker
        ):
            add(
                f"{prefix}active launch lane blocker must be a non-empty canonical string"
            )
        elif blocker.startswith(prefix):
            add(blocker)
        else:
            add(f"{prefix}{blocker}")
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
    for index, item in enumerate(value):
        issue = _native_evm_validation_blocker_issue(item, label, index)
        blockers.append(issue if issue is not None else item)
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


def _active_launch_lane_blockers_for_checklist(
    value: Any,
    lane_label: str,
) -> tuple[list[str], list[str]]:
    label = f"{lane_label}: active launch lane blockers"
    if not isinstance(value, list):
        return [], [f"{label} must be a list of non-empty canonical strings"]
    blockers: list[str] = []
    schema_blockers: list[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str) or not item or item.strip() != item:
            schema_blockers.append(
                f"{label}[{index}] must be a non-empty canonical string"
            )
        else:
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


def _active_launch_route_canary_metadata_blockers(
    lane_label: str,
    canary: dict[str, Any],
) -> list[str]:
    """Return active launch route-canary transaction metadata blockers."""

    blockers: list[str] = []
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
    if canary.get("receipt_block_finalized") is not True:
        blockers.append(f"{lane_label}: route canary receipt block must be finalized")
    return blockers


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
    supplied_hash = destination_binding.get("destination_binding_hash")
    expected_hash = destination_binding.get("expected_destination_binding_hash")
    if not _is_nonzero_hex32(supplied_hash):
        blockers.append(
            f"{lane_label}: governed deployment destination binding hash must be a canonical non-zero bytes32 hex string"
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
        _active_launch_route_canary_metadata_blockers(lane_label, canary)
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
            "title": f"Active {ACTIVE_LAUNCH_DISPLAY} SCCP lane has the required source, deployment, destination, and route records",
            "ready": not records_blockers,
            "blockers": records_blockers,
        },
        {
            "id": "governed_deployment_evidence",
            "title": f"{ACTIVE_LAUNCH_DISPLAY} source-adapter deployment and destination rollout are governed and hash-bound",
            "ready": not deployment_blockers,
            "blockers": deployment_blockers,
        },
        {
            "id": "route_allowlist_binding",
            "title": f"{ACTIVE_LAUNCH_DISPLAY} route allowlist binds the governed source and destination evidence",
            "ready": not route_blockers,
            "blockers": route_blockers,
        },
        {
            "id": "live_route_canary_evidence",
            "title": f"{ACTIVE_LAUNCH_DISPLAY} post-deploy route canary evidence is live, passed, and bound to the route",
            "ready": not canary_blockers,
            "blockers": canary_blockers,
        },
        {
            "id": "native_evm_groth16_prover_bundle",
            "title": f"{ACTIVE_LAUNCH_DISPLAY} browser and native SDKs ship an audited no-WASM, no-remote EVM Groth16 prover bundle",
            "ready": not native_prover_blockers,
            "blockers": native_prover_blockers,
        },
        {
            "id": "no_unresolved_blockers",
            "title": f"No active {ACTIVE_LAUNCH_DISPLAY} launch blockers remain",
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
        release_checklist["ready"] is True
        and corridor_ready
        and not launch_scope_constant_gate_blockers
        and not ethereum_launch_policy_selector_gate_blockers
        and not ethereum_launch_policy_documentation_gate_blockers
        and not public_discovery_documentation_gate_blockers
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


def _record_flags(records: dict[str, bool]) -> str:
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
    if isinstance(value, str) and value:
        return f"`{value}`"
    return "-"


def _audit_hashes_cell(value: Any) -> str:
    if not isinstance(value, dict) or not value:
        return "-"
    return "<br>".join(
        f"`{key}`: `{audit_hash}`"
        for key, audit_hash in sorted(value.items())
        if isinstance(key, str) and isinstance(audit_hash, str)
    ) or "-"


def _integer_cell(value: Any) -> str:
    if type(value) is int:
        return f"`{value}`"
    return "-"


def _boolean_cell(value: Any) -> str:
    if type(value) is bool:
        return "`true`" if value else "`false`"
    return "-"


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


def _sdk_helper_sets_cell(surface: dict[str, Any]) -> str:
    helper_sets = surface.get("sdk_helper_symbols_by_sdk")
    if not isinstance(helper_sets, dict):
        return "`<invalid sdk_helper_symbols_by_sdk>`"
    rows: list[str] = []
    for sdk in (*USER_PROVER_SDK_PHASES, EVM_NATIVE_DOTNET_PHASE):
        helpers = helper_sets.get(sdk)
        if not isinstance(helpers, list):
            continue
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
    return "<br>".join(value)


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
    return "<br>".join(value)


def _markdown_string_list_items(value: Any, *, field_label: str) -> list[str]:
    if not isinstance(value, list):
        return [f"- `<invalid {field_label}>`"]
    if not value:
        return ["- None"]
    if not all(isinstance(item, str) and item for item in value):
        return [f"- `<invalid {field_label}>`"]
    if any(_public_blocker_text_issue(item) is not None for item in value):
        return [f"- `<invalid {field_label}>`"]
    return [f"- {item}" for item in value]


def _render_markdown(report: dict[str, Any], *, max_blockers_per_lane: int) -> str:
    status = "READY" if report["production_ready"] is True else "NOT READY"
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
    for artifact in report["input_artifacts"]:
        lines.append(
            "| `{path}` | {bytes} | `{sha256}` |".format(
                path=artifact["path"],
                bytes=artifact["bytes"],
                sha256=artifact["sha256"],
            )
        )
    lines.extend(["", "## Production Corridor", ""])
    lines.append("| Phase | Status | Evidence Artifact | Evidence SHA-256 |")
    lines.append("| --- | --- | --- | --- |")
    for phase, phase_status in report["corridor"]["phases"].items():
        artifact = report["corridor"]["evidence_artifacts"].get(phase)
        artifact_path = f"`{artifact['path']}`" if artifact else "-"
        artifact_hash = f"`{artifact['sha256']}`" if artifact else "-"
        lines.append(
            f"| `{phase}` | {phase_status} | {artifact_path} | {artifact_hash} |"
        )

    lines.extend(["", "## Release Checklist", ""])
    lines.append("| Gate | Status | Blockers |")
    lines.append("| --- | --- | --- |")
    for item in report["release_checklist"]["items"]:
        item_status = "ready" if item["ready"] is True else "blocked"
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
        lines.append(f"| `{item['id']}` | {item_status} | {blocker_text} |")

    lines.extend(["", "## Cryptographic Evidence", ""])
    lines.append(
        "| Domain | Chain | EVM Source Chain ID | EVM Source Tag | "
        "EVM Destination Chain ID | EVM Destination Tag | "
        "Source Material | Source Deployment | "
        "Destination Binding | Source Gate | Source Gate Audits | "
        "Route Allowlist | Route Canary | Canary Source | Canary Tx | "
        "Canary Receipt Block | Canary Receipt Hash | Canary Receipt Finalized | "
        "Canary Receipts Root | Canary Message ID | Canary Block | "
        "Canary Timestamp |"
    )
    lines.append(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )
    for row in report["cryptographic_evidence"]:
        canary_source = row["route_canary_evidence_source"] or "-"
        if row["route_canary_evidence_bound"] is not True:
            canary_source = f"{canary_source} (unbound)"
        source_gate = _hash_cell(row["source_adapter_gate_hash"])
        if row["source_adapter_gate_required"] is False and source_gate == "-":
            source_gate = "not required"
        lines.append(
            "| {domain} | `{chain}` | `{evm_source_rpc_chain_id}` | "
            "`{evm_source_tag}` | `{evm_dest_rpc_chain_id}` | "
            "`{evm_dest_tag}` | "
            "{source} | {deploy} | {dest} | "
            "{source_gate} | {source_gate_audits} | {route} | {canary} | "
            "`{canary_source}` | {canary_tx} | {canary_receipt_block} | "
            "{canary_receipt_hash} | {canary_receipt_finalized} | "
            "{canary_receipts_root} | "
            "{canary_message_id} | {canary_block} | {canary_timestamp} |".format(
                domain=row["domain"],
                chain=row["chain"],
                evm_source_rpc_chain_id=row["evm_source_rpc_chain_id"] or "-",
                evm_source_tag=row["evm_source_block_tag"] or "-",
                evm_dest_rpc_chain_id=row["evm_destination_rpc_chain_id"] or "-",
                evm_dest_tag=row["evm_destination_block_tag"] or "-",
                source=_hash_cell(row["source_verifier_material_hash"]),
                deploy=_hash_cell(row["source_adapter_engine_deployment_hash"]),
                dest=_hash_cell(row["destination_binding_hash"]),
                source_gate=source_gate,
                source_gate_audits=_audit_hashes_cell(
                    row["source_adapter_gate_audit_hashes"]
                ),
                route=_hash_cell(row["route_allowlist_hash"]),
                canary=_hash_cell(row["route_canary_evidence_hash"]),
                canary_source=canary_source,
                canary_tx=_hash_cell(row["route_canary_transaction_hash"]),
                canary_receipt_block=_integer_cell(
                    row["route_canary_receipt_block_number"]
                ),
                canary_receipt_hash=_hash_cell(
                    row["route_canary_receipt_block_hash"]
                ),
                canary_receipt_finalized=_boolean_cell(
                    row["route_canary_receipt_block_finalized"]
                ),
                canary_receipts_root=_hash_cell(
                    row["route_canary_block_receipts_root"]
                ),
                canary_message_id=_hash_cell(row["route_canary_message_id"]),
                canary_block=_integer_cell(row["route_canary_block_number"]),
                canary_timestamp=_integer_cell(row["route_canary_block_timestamp"]),
            )
        )

    lines.extend(["", "## User Prover Submission Surfaces", ""])
    lines.append(
        "| Lanes | Proof Backend | SDK Helpers | On-chain Submission | "
        "Required Phases | Validation |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- |")
    for surface in report["user_prover_submission_surfaces"]:
        required_phases = ", ".join(
            f"`{phase}`" for phase in surface["required_phases"]
        )
        validation = surface["validation_status"]
        validation_blockers = surface.get("validation_blockers")
        if not isinstance(validation_blockers, list) or validation_blockers:
            validation += ": " + _user_prover_validation_blockers_cell(
                validation_blockers
            )
        lines.append(
            "| `{lanes}` | `{proof_backend}` | {sdk_helpers} | {submission} | "
            "{required_phases} | {validation} |".format(
                lanes=surface["lanes"],
                proof_backend=surface["proof_backend"],
                sdk_helpers=_sdk_helper_sets_cell(surface),
                submission=surface["on_chain_submission"],
                required_phases=required_phases,
                validation=validation,
            )
        )

    lines.extend(["", "## Native Prover Bundle", ""])
    lines.append(
        "| Required | Status | Artifact | SHA-256 | Proof Artifact | Proving Key | "
        "Verifier Key | Destination Binding | Parity Fixture | Self-Test | "
        "SDK Artifacts | Blockers |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    native_bundle = report["native_evm_prover_bundle"]
    native_artifact = native_bundle.get("artifact")
    artifact_path = "-"
    artifact_hash = "-"
    if isinstance(native_artifact, dict):
        artifact_path = f"`{native_artifact.get('path', '-')}`"
        artifact_hash = f"`{native_artifact.get('sha256', '-')}`"
    sdk_artifacts = native_bundle.get("sdk_artifacts")
    if isinstance(sdk_artifacts, list) and sdk_artifacts:
        sdk_cell = "<br>".join(
            "`{sdk}`: `{implementation}` {implementation_hash}".format(
                sdk=row.get("sdk", "-"),
                implementation=row.get("implementation", "-"),
                implementation_hash=(
                    f"`{row.get('implementation_hash')}`"
                    if row.get("implementation_hash")
                    else "-"
                ),
            )
            for row in sdk_artifacts
            if isinstance(row, dict)
        )
    else:
        sdk_cell = "-"
    native_blocker_text = _native_evm_validation_blockers_cell(
        native_bundle.get("validation_blockers")
    )
    parity_artifact = native_bundle.get("cross_sdk_fixture_parity_artifact")
    parity_cell = (
        f"`{parity_artifact.get('path')}`<br>`{parity_artifact.get('sha256')}`"
        if isinstance(parity_artifact, dict)
        else "-"
    )
    self_test_artifact = native_bundle.get("native_prover_self_test_artifact")
    self_test_cell = (
        f"`{self_test_artifact.get('path')}`<br>`{self_test_artifact.get('sha256')}`"
        if isinstance(self_test_artifact, dict)
        else "-"
    )
    lines.append(
        "| {required} | {status} | {artifact} | {artifact_hash} | "
        "{proof_artifact} | {proving_key} | {verifier_key} | {binding} | "
        "{parity_fixture} | {self_test} | {sdk_artifacts} | {blockers} |".format(
            required="yes" if native_bundle.get("required") is True else "no",
            status=native_bundle.get("validation_status", "blocked"),
            artifact=artifact_path,
            artifact_hash=artifact_hash,
            proof_artifact=(
                f"`{native_bundle.get('proof_artifact_hash')}`"
                if native_bundle.get("proof_artifact_hash")
                else "-"
            ),
            proving_key=(
                f"`{native_bundle.get('proving_key_hash')}`"
                if native_bundle.get("proving_key_hash")
                else "-"
            ),
            verifier_key=(
                f"`{native_bundle.get('verifier_key_hash')}`"
                if native_bundle.get("verifier_key_hash")
                else "-"
            ),
            binding=(
                f"`{native_bundle.get('destination_binding_hash')}`"
                if native_bundle.get("destination_binding_hash")
                else "-"
            ),
            parity_fixture=parity_cell,
            self_test=self_test_cell,
            sdk_artifacts=sdk_cell,
            blockers=native_blocker_text,
        )
    )

    lines.extend(["", "## Source Inventory", ""])
    lines.append("| Gate | Status | Blockers |")
    lines.append("| --- | --- | --- |")
    for gate in sorted(report["source_inventory"]):
        inventory = report["source_inventory"][gate]
        blocker_text = _markdown_string_list_cell(
            inventory.get("validation_blockers"),
            field_label="validation_blockers",
        )
        lines.append(
            "| `{gate}` | {status} | {blockers} |".format(
                gate=gate,
                status=inventory.get("validation_status", "blocked"),
                blockers=blocker_text,
            )
        )

    lines.extend(["", "## Lane Readiness", ""])
    lines.append("| Domain | Chain | Status | Records | Blockers |")
    lines.append("| --- | --- | --- | --- | --- |")
    for lane in report["evidence"]["lanes"]:
        lane_status = "ready" if lane["production_ready"] is True else "blocked"
        lane_blockers = lane.get("blockers")
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
        lines.append(
            "| {domain} | `{chain}` | {status} | {records} | {blockers} |".format(
                domain=lane["domain"],
                chain=lane["chain"],
                status=lane_status,
                records=_record_flags(lane["records"]),
                blockers=blocker_text,
            )
        )

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
            "- Governed live deployment evidence for immutable destination verifiers and source-chain verifier engines; offline placeholder or template-derived hashes keep the report blocked.",
            "- An audited `--native-evm-prover-bundle` manifest with `schema = sccp-native-evm-groth16-prover-bundle-v1`, `no_wasm = true`, `remote_prover_required = false`, and matching Ethereum destination binding/proving-key hashes.",
            f"- {SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE}",
            "- SCCP launch-scope source inventory must pin active launch policy constants and the supported launch-domain set across Rust, all-lanes evidence, and readiness tooling.",
            "- SCCP Ethereum launch-policy selector source inventory must pin the EthereumMainnetLane selector and negative cross-lane policy tests.",
            "- SCCP Ethereum launch-policy documentation source inventory must pin the active Ethereum-mainnet policy wording and reject stale BSC-only production-packaging text.",
            "- SCCP public discovery documentation source inventory must pin supported launch-lane and verifier-target wording so unsupported lanes cannot re-enter Torii discovery evidence silently.",
            "- SCCP Ethereum no-proxy data-collection source inventory must pin app-owned execution/Beacon provider reads and reject Torii proxy or embedded HTTP-client fallbacks across public SDKs.",
            "- SCCP Ethereum inbound adversarial source inventory must pin public SDK regressions for failed receipts, source-event drift, hash-only proof bypasses, immutable evidence snapshots, oversized proof bytes, finality mismatches, sync-committee quorum checks, and wrong-domain receipt transcripts before inbound source proofs can be accepted.",
            "- SCCP BSC inbound adversarial source inventory must pin public SDK regressions for hash-only proof bypasses, receipt-proof metadata binding, source-event digest drift, malformed source logs, and missing source-event validation before BSC inbound source proofs can be accepted.",
            "- SCCP TRON inbound adversarial source inventory must pin runtime duplicate source-event log rejection before TRON transaction-info receipts can satisfy inbound source-proof admission.",
            "- SCCP BSC route-config canonical-manifest source inventory must pin canonical JSON string, lowercase bytes32, lowercase EVM address, and network metadata rejection before governed TAIRA XOR overlays can satisfy production readiness.",
            "- SCCP TRON route-config canonical-manifest source inventory must pin canonical JSON string, lowercase bytes32, canonical Base58 address, and network metadata rejection before governed TAIRA XOR overlays can satisfy production readiness.",
            "- SCCP TRON runtime route-manifest source inventory must pin the TRON runtime route-manifest parser, mainnet metadata checks, dynamic destination-binding recomputation, and post-deploy anchor rejection before runtime config evidence can satisfy production readiness.",
            "- SCCP all-lanes route-canary scalar source inventory must pin canonical status/evidence-source schema blockers before all-lanes release-checklist route-canary readiness can pass.",
            "- SCCP all-lanes evidence-root schema source inventory must pin malformed evidence root, unknown section, and non-string section-key blockers before all-lanes evidence can satisfy production readiness.",
            "- SCCP all-lanes governed blocker schema source inventory must pin destination-rollout and route-allowlist blocker container rejection before governed evidence can satisfy production readiness.",
            "- SCCP all-lanes release-checklist exact-boolean source inventory must pin exact checklist-item aggregation, record-presence gates, CLI production-ready exits, source-adapter gate hash/audit replay rejection, and route-canary hash replay rejection before all-lanes evidence can satisfy production readiness.",
            "- SCCP active-launch checklist schema source inventory must pin the active launch checklist ready value, malformed lane metadata, and verifier recomputation before production readiness can pass.",
            "- SCCP route allowlist canary summary source inventory must pin optional canary-summary exactness, route-hash binding, and hash role-separation regressions before route profiles can be published as production evidence.",
            "- SCCP release manifest readiness-flags source inventory must pin exact boolean manifest generation, verifier boolean rejection, manifest/report equality checks, and all-lanes readiness recomputation before published bundle readiness can pass.",
            "- SCCP release manifest artifact-set/order source inventory must pin required artifact paths, manifest-root exclusion, unmanifested artifact/directory rejection, report-referenced artifact closure, malformed public artifact field-name classification, and canonical attachment order before published bundle readiness can pass.",
            "- SCCP release public blocker-list schema source inventory must pin canonical non-empty blocker strings, no surrounding whitespace, duplicate rejection, ready-surface empty-blocker checks, and invalid-marker rendering before published bundle readiness can pass.",
            "- SCCP release public scalar-text schema source inventory must pin canonical non-empty scalar text, fixed release-checklist item-id classification, public object-key classification for release-checklist titles, corridor phase keys, cryptographic-evidence chain/source labels, user-prover submission rows, all-lanes chain labels, all-lanes unknown object/audit keys, destination-binding keys, route-canary status/source fields, and redacted destination/Solana JSON-RPC/ProgramData/TON BoC/TRON route-canary scalar diagnostics before published bundle readiness can pass.",
            "- SCCP release-notes attachment invariants source inventory must pin canonical single top-level title/status block, no unexpected section headings, exact manifest handoff/root-exclusion block, canonical single artifact table scaffold/shape and position, self-row exclusion, contiguous exact ordered row-set binding, canonical blocker-section visibility, no noncanonical trailing content, and canonical attachment drift rejection before public bundle readiness can pass.",
            "- SCCP readiness Markdown invariants source inventory must pin verifier-owned public Markdown sections, evidence-input path/bytes/hash visibility, production-corridor phase/status visibility, production-corridor artifact/hash visibility, checklist gate/status visibility, checklist blocker-cell visibility, cryptographic row live-EVM visibility, cryptographic row core-hash visibility, cryptographic row route-canary visibility, lane-readiness status visibility, lane-readiness blocker-cell visibility, source-inventory gate/status visibility, source-inventory blocker-cell visibility, user-prover validation-status visibility, user-prover blocker-cell visibility, user-prover helper/phase row visibility, native-prover validation-status visibility, native-prover blocker-cell visibility, native-prover artifact/hash row visibility, native-prover support-artifact row visibility, source-inventory blocker visibility, invalid-marker rendering, malformed source-inventory gate-name, report-artifact path, and cryptographic-evidence row-domain/audit-key suppression, and canonical Markdown drift rejection before public bundle readiness can pass.",
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
            "- SCCP Ethereum source-bridge config source inventory must pin bridge-address/network/code-hash config hashing and negative config-drift tests.",
            "- SCCP Ethereum EVM source-adapter deployment source inventory must pin the active deployment gate, source-bridge network/config binding, and negative drift tests.",
            "- SCCP source-material template rejection source inventory must pin ETH, BSC, Solana, TON, and TRON evidence-script guards, aggregate all-lanes copied-evidence guards, strict release-bundle public JSON guards, and negative tests that reject built-in template verifier hashes before source material can satisfy production readiness.",
            "- SCCP source-material role validation source inventory must pin ETH, BSC, Solana, TON, and TRON zero-hash, role-reuse, canonical adapter-verifier, full-light-client audit role-separation guards, and redacted all-lanes-TOML/source validator/source-record/source-gate/TON-live-accountStates/address/code-BoC/TON-destination-code-BoC/TRON-live-API/metadata/full-TOML/TRON-witness-JSON blockers before source material can satisfy production readiness.",
            "- SCCP EVM contract smoke Ethereum mainnet network-id source inventory must pin ETH chain-id vectors, BSC rejection vectors, and accepted-event network-id assertions.",
            "- SCCP EVM contract smoke production-surface source inventory must pin verifier-code/key, destination-binding, domain-overflow, proof-shape, cross-deployment, and replay rejection smoke coverage.",
            "- SCCP Ethereum core range/finality binding source inventory must pin finality-height range binding in Core and negative outer-range replay tests.",
            "- SCCP Ethereum core message replay source inventory must pin durable pinned-record replay protection and negative replay/history tests.",
            "- SCCP Ethereum Torii pinned message proof source inventory must pin pinned message-proof extraction and negative unpinned-record serving tests.",
            "- Ethereum mainnet live EVM source production source inventory must pin canonical live source RPC chain ids, finalized block tags, deployment receipt binding, redacted JSON-RPC diagnostics, route canary calldata, and proof tuple drift tests.",
            "- Ethereum mainnet live EVM destination production source inventory must pin canonical live destination RPC chain ids, finalized block tags, runtime bytecode hashes, redacted runtime bytecode parser diagnostics, and destination production TOML evidence before production readiness can pass.",
            "- SCCP Ethereum route-canary finalized receipt-block source inventory must pin finalized receipt-block binding, TOML evidence fields, all-lanes comments, runtime hashing, and negative drift tests.",
            "- SCCP Ethereum EVM block-tag metadata source inventory must pin finalized source/destination block-tag evidence and negative drift tests.",
            "- SCCP native no-WASM/no-remote source inventory must pin public SDK parsers, artifact verifiers, self-tests, browser distribution guards, canonical native EVM prover SDK-id rejection, padded-SDK adversarial tests, adversarial manifest coverage, and redacted native payload artifact-path diagnostics.",
            "- SCCP release native-prover bundle schema source inventory must pin native EVM Groth16 manifest schema, readiness summary schema, artifact hash/path binding, copied-summary scalar exactness, and bundled-manifest drift rejection before published bundle readiness can pass.",
            "- SCCP proof-request bundle/source-proof source inventory must pin canonical bundle-byte and non-SORA source-proof rejection gates across Rust, JavaScript, Python, Swift, Kotlin/JVM, and Java Android.",
            "- SCCP phase-evidence source inventory must pin duplicate assignment and directory override rejection across readiness-report and release-bundle CLIs before corridor phase evidence can satisfy production readiness.",
            "- SCCP release corridor phase-transcript source inventory must pin exact phase markers, phase-local ordered non-negated/non-diagnostic shell-xtrace-free completion/success output after required commands in per-phase and full-corridor logs, phase-specific traced command shapes with exact pytest positional inputs, option-bound selectors, exact Gradle test command parsing, broad Kotlin package-suite selectors, exact Swift filter commands, exact Android harness class membership, exact Node test/check command files, exact .NET project/filter/nologo commands, exact no-suffix cargo/bash/java commands, and without bare-fragment shortcuts or shell-comment-hidden fragments, restricted cd wrappers, dry-run rejection, failure-marker scans, and forged-block rejection before corridor logs can satisfy public bundle readiness.",
            "- SCCP release bundle source-copy source inventory must pin symlink and control-character rejection for evidence inputs, phase evidence, native EVM prover manifests, and native prover payload sources before bundle copy can run.",
            "- SCCP release bundle output-path source inventory must pin symlink and control-character rejection for output directories before bundle generation can create or overwrite release artifacts.",
            "- SCCP release artifact path text source inventory must pin Markdown-unsafe character rejection for manifest artifact paths, readiness inputs, native prover manifest/payload paths, copied bundle filenames, and bundle filesystem entries before release notes can render artifact tables.",
            "- SCCP release input-provenance schema source inventory must pin canonical copied evidence input paths, unique input/input-artifact provenance, copied `evidence/NN-*.toml` layout, and recomputation from copied TOML before published bundle readiness can pass.",
            "- SCCP release public JSON-root schema source inventory must pin canonical manifest/readiness/all-lanes JSON serialization, duplicate-key rejection with malformed-key classification, non-UTF-8 fail-closed diagnostics, redacted source-inventory read diagnostics, and malformed manifest/readiness root-field classification before published bundle readiness can pass.",
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
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError) and (
        getattr(exc, "filename", None) is not None
        or getattr(exc, "filename2", None) is not None
    ):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.max_blockers_per_lane < 1:
        parser.error("--max-blockers-per-lane must be positive")

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
        OSError,
        RuntimeError,
        TypeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP release readiness report generation failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")

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
