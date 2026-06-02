#!/usr/bin/env python3
"""Verify a published SCCP release-note attachment bundle."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ALL_LANES_SCRIPT = ROOT / "scripts" / "sccp_all_lanes_evidence.py"
SCHEMA = "sccp-release-bundle-v1"
CORRIDOR_COMPLETION_SENTINEL = "SCCP production corridor completed."
CORRIDOR_DRY_RUN_SENTINEL = "SCCP production corridor dry run completed."
CORRIDOR_PHASE_MARKER_PREFIX = "==> SCCP production corridor: "
CORRIDOR_PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
)
PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS: dict[str, tuple[str, ...]] = {
    "rust-sccp": ("cargo test -p iroha_sccp -- --nocapture",),
    "evidence-scripts": (
        "python3 -m pytest -q pytests/scripts/check_sccp_production_corridor_test.py",
    ),
    "js-sdk": (
        "node --test",
        "javascript/iroha_js/test/sccpSolanaProver.test.js",
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
        "javascript/iroha_js/test/package_dist.test.js",
        "javascript/iroha_js/test/sccpPackageExports.test.js",
    ),
    "python-sdk": (
        "python3 -m pytest -q python/iroha_torii_client/tests/sccp_test.py",
    ),
    "swift-sdk": (
        "swift test --filter SccpSolanaProverTests --disable-swift-testing",
        "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions",
    ),
    "kotlin-sdk": (
        "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp.",
    ),
    "java-android": (
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
    ),
    "dotnet-sdk": (
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
    ),
    "contract-smoke": (
        "node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        "bash scripts/sccp_evm_contract_smoke.sh",
    ),
    "core-admission": ("cargo test -p iroha_core --test bridge_proofs -- --nocapture",),
}
PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS: dict[str, tuple[str, ...]] = {
    "rust-sccp": ("test result: ok",),
    "evidence-scripts": (" passed in ",),
    "js-sdk": (
        "fail 0",
        "pass ",
        "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned",
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned",
        "package declarations expose BSC mainnet Parlia finality evidence hooks",
    ),
    "python-sdk": (" passed in ",),
    "swift-sdk": ("0 failures",),
    "kotlin-sdk": ("BUILD SUCCESSFUL",),
    "java-android": ("BUILD SUCCESSFUL",),
    "dotnet-sdk": ("Passed!",),
    "contract-smoke": ("sccp_message_bridge_smoke: ok",),
    "core-admission": ("test result: ok",),
}
SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_DOMAIN_SOL = 3
SCCP_DOMAIN_TON = 4
SCCP_DOMAIN_TRON = 5
SCCP_DOMAIN_SORA_KUSAMA = 6
SCCP_DOMAIN_SORA_POLKADOT = 7
SCCP_DOMAIN_SORA2 = 8
ACTIVE_LAUNCH_DOMAIN = SCCP_DOMAIN_BSC
ACTIVE_LAUNCH_CHAIN = "bsc"
ACTIVE_LAUNCH_POLICY = "BscMainnetLane"
ACTIVE_LAUNCH_DISPLAY = f"{ACTIVE_LAUNCH_CHAIN.upper()} mainnet"
ALL_LANES_REQUIRED_DOMAINS = (
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
)
ALL_LANES_CHAIN_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "eth",
    SCCP_DOMAIN_BSC: "bsc",
    SCCP_DOMAIN_SOL: "sol",
    SCCP_DOMAIN_TON: "ton",
    SCCP_DOMAIN_TRON: "tron",
    SCCP_DOMAIN_SORA_KUSAMA: "sora-kusama",
    SCCP_DOMAIN_SORA_POLKADOT: "sora-polkadot",
    SCCP_DOMAIN_SORA2: "sora2",
}
ALL_LANES_ROUTE_ALLOWLIST_ID_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "sccp:eth:route-allowlist:ethereum-mainnet:v1",
    SCCP_DOMAIN_BSC: "sccp:bsc:route-allowlist:bsc-mainnet:v1",
    SCCP_DOMAIN_SOL: "sccp:sol:route-allowlist:solana-mainnet-beta:v1",
    SCCP_DOMAIN_TON: "sccp:ton:route-allowlist:ton-mainnet:v1",
    SCCP_DOMAIN_TRON: "sccp:tron:route-allowlist:tron-mainnet:v1",
    SCCP_DOMAIN_SORA_KUSAMA: "sccp:sora-kusama:route-allowlist:runtime:v1",
    SCCP_DOMAIN_SORA_POLKADOT: "sccp:sora-polkadot:route-allowlist:runtime:v1",
    SCCP_DOMAIN_SORA2: "sccp:sora2:route-allowlist:runtime:v1",
}
SOLANA_BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
SOLANA_BASE58_INDEX = {
    symbol: index for index, symbol in enumerate(SOLANA_BASE58_ALPHABET)
}
REQUIRED_ARTIFACT_PATHS = (
    "sccp-release-readiness.md",
    "sccp-release-readiness.json",
    "sccp-all-lanes-summary.json",
    "sccp-release-notes-attachment.md",
)
MANIFEST_ROOT_PATH = "manifest.json"
ARTIFACT_KEYS = {"path", "bytes", "sha256"}
MANIFEST_KEYS = {
    "schema",
    "production_ready",
    "release_checklist_ready",
    "corridor_ready",
    "blockers",
    "artifacts",
}
READINESS_REPORT_KEYS = {
    "production_ready",
    "evidence",
    "release_checklist",
    "corridor",
    "blockers",
    "inputs",
    "input_artifacts",
    "cryptographic_evidence",
    "user_prover_submission_surfaces",
}
READINESS_MARKDOWN_REQUIRED_HEADINGS = (
    "## Evidence Inputs",
    "## Production Corridor",
    "## Release Checklist",
    "## Cryptographic Evidence",
    "## User Prover Submission Surfaces",
    "## Lane Readiness",
    "## Blocking Items",
    "## Required Release Evidence",
)
READINESS_MARKDOWN_REQUIRED_RELEASE_EVIDENCE_MARKERS = (
    "scripts/check_sccp_production_corridor.sh",
    "--require-phase-evidence",
    "user-prover helper surface",
    "JavaScript/web source",
    f"{ACTIVE_LAUNCH_DISPLAY} launch-lane evidence",
    "Governed live deployment evidence",
    "Public release notes",
)
CRYPTOGRAPHIC_EVIDENCE_KEYS = {
    "domain",
    "chain",
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
    "destination_binding_hash",
    "route_allowlist_hash",
    "route_canary_evidence_hash",
    "route_canary_evidence_source",
    "route_canary_evidence_bound",
    "route_canary_block_number",
    "route_canary_block_timestamp",
    "source_adapter_gate_required",
    "source_adapter_gate_hash",
    "source_adapter_gate_audit_hashes",
}
USER_PROVER_SUBMISSION_SURFACE_KEYS = {
    "lanes",
    "proof_backend",
    "sdk_helper_symbols",
    "sdk_helper_symbols_by_sdk",
    "sdk_helpers",
    "on_chain_submission",
    "required_phases",
    "validation_status",
    "validation_blockers",
}
USER_PROVER_SDK_HOOK_MARKERS = {
    "js-sdk": ("witnessProvider", "proveFn"),
    "python-sdk": ("witness_provider", "prove"),
    "swift-sdk": ("WitnessProvider", "ProveFunction"),
    "kotlin-sdk": ("WitnessProvider", "ProofEngine"),
    "java-android": ("WitnessProvider", "ProofEngine"),
    "dotnet-sdk": (
        "InboundProver",
        "InboundSubmitter",
        "OutboundProver",
        "OutboundSubmitter",
    ),
}
USER_PROVER_SDK_PHASES = (
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
)


def _helper_matches_hook_marker(sdk: str, helper: str, marker: str) -> bool:
    if sdk == "python-sdk":
        return helper == marker
    return marker in helper
USER_PROVER_REQUIRED_PHASES = (
    *USER_PROVER_SDK_PHASES,
    "core-admission",
)
USER_PROVER_KNOWN_REQUIRED_PHASES = (
    *USER_PROVER_SDK_PHASES,
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
)
USER_PROVER_CONTRACT_SMOKE_BACKENDS = {
    "evm-groth16-bn254-v1",
    "tron-groth16-bn254-v1",
}
USER_PROVER_REQUIRED_LANE_BACKENDS = {
    "eth,bsc": "evm-groth16-bn254-v1",
    "tron": "tron-groth16-bn254-v1",
    "sol": "sccp-solana-recursive-mainnet-v1",
    "ton": "ton-contract-v1",
    "substrate": "substrate-runtime-v1",
}
USER_PROVER_ON_CHAIN_SUBMISSION_BY_LANE = {
    "eth,bsc": (
        "Torii bridge-proof submit payload with BN254 Groth16 "
        "proof_bytes_hex for the EVM verifier contract"
    ),
    "tron": (
        "Torii bridge-proof submit payload with BN254 Groth16 "
        "proof_bytes_hex for the TRON verifier contract"
    ),
    "sol": "Solana verifier-program instruction envelope",
    "ton": "TON internal message body BOC",
    "substrate": "Substrate runtime call envelope",
}
USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK = {
    "eth,bsc": {
        "js-sdk": (
            "buildEvmSccpProofRequest",
            "canonicalEvmSccpReceiptProofBytes",
            "evmSccpReceiptProofHash",
            "canonicalBscSccpReceiptProofBytes",
            "bscSccpReceiptProofHash",
            "buildBscMainnetSccpDestinationProofRequest",
            "wrapBscMainnetSccpDestinationProofResult",
            "EthereumMainnetSccp",
            "EthereumMainnetSccp.buildOutboundProofRequest",
            "EthereumMainnetSccp.proveOutboundToEthereum",
            "EthereumMainnetSccp.buildEthereumCalldata",
            "EthereumMainnetSccp.submitOutboundToEthereum",
            "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
            "EthereumMainnetSccp.proveInboundToSora",
            "EthereumMainnetSccp.submitInboundToIroha",
            "EthereumMainnetSccp.buildLocalAdmissionSubmission",
            "buildEthereumMainnetSccpLocalAdmissionSubmission",
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
        ),
        "python-sdk": (
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
        ),
        "swift-sdk": (
            "buildEvmSccpProofRequest",
            "canonicalEvmSccpReceiptProofBytes",
            "evmSccpReceiptProofHash",
            "canonicalBscSccpReceiptProofBytes",
            "bscSccpReceiptProofHash",
            "buildBscMainnetSccpDestinationProofRequest",
            "wrapBscMainnetSccpDestinationProofResult",
            "EthereumMainnetSccp",
            "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
            "EthereumMainnetSccp.proveInboundToSora",
            "EthereumMainnetSccp.submitInboundToIroha",
            "EthereumMainnetSccp.buildLocalAdmissionSubmission",
            "buildEthereumMainnetSccpLocalAdmissionSubmission",
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
        ),
        "kotlin-sdk": (
            "SccpEvm.buildProofRequest",
            "SccpSourceProofs.canonicalEvmReceiptProofBytes",
            "SccpSourceProofs.evmReceiptProofHash",
            "SccpSourceProofs.canonicalBscReceiptProofBytes",
            "SccpSourceProofs.bscReceiptProofHash",
            "SccpBsc.buildProofRequest",
            "EthereumMainnetSccp",
            "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
            "EthereumMainnetSccp.proveInboundToSora",
            "EthereumMainnetSccp.submitInboundToIroha",
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
        ),
        "java-android": (
            "EvmSccpProver.buildProofRequest",
            "SourceSccpProofs.canonicalEvmReceiptProofBytes",
            "SourceSccpProofs.evmReceiptProofHash",
            "SourceSccpProofs.canonicalBscReceiptProofBytes",
            "SourceSccpProofs.bscReceiptProofHash",
            "BscSccpProver.buildProofRequest",
            "EthereumMainnetSccp",
            "EthereumMainnetSccp.collectInboundEvidenceFromReceipt",
            "EthereumMainnetSccp.proveInboundToSora",
            "EthereumMainnetSccp.submitInboundToIroha",
            "EthereumMainnetSccp.buildLocalAdmissionSubmission",
            "EthereumMainnetSccp.buildLocalAdmission",
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
        ),
        "dotnet-sdk": (
            "EthereumMainnetSccp",
            "EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync",
            "EthereumMainnetSccp.ProveInboundToSoraAsync",
            "EthereumMainnetSccp.SubmitInboundToIrohaAsync",
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
        ),
    },
    "tron": {
        "js-sdk": (
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
        ),
        "python-sdk": (
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
        ),
        "swift-sdk": (
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
        ),
        "kotlin-sdk": (
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
        ),
        "java-android": (
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
        ),
    },
    "sol": {
        "js-sdk": (
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
        ),
        "python-sdk": (
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
        ),
        "swift-sdk": (
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
        ),
        "kotlin-sdk": (
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
        ),
        "java-android": (
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
        ),
    },
    "ton": {
        "js-sdk": (
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
        ),
        "python-sdk": (
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
        ),
        "swift-sdk": (
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
        ),
        "kotlin-sdk": (
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
        ),
        "java-android": (
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
        ),
    },
    "substrate": {
        "js-sdk": (
            "buildSubstrateSccpProofRequest",
            "buildSubstrateSccpRuntimeStorageProofRequest",
            "SubstrateSccpProver",
            "witnessProvider",
            "proveFn",
            "buildSubstrateSccpSubmission",
        ),
        "python-sdk": (
            "build_substrate_sccp_proof_request",
            "build_substrate_sccp_runtime_storage_proof_request",
            "SubstrateSccpProver",
            "witness_provider",
            "prove",
            "build_substrate_sccp_submission",
        ),
        "swift-sdk": (
            "buildSubstrateSccpProofRequest",
            "buildSubstrateSccpRuntimeStorageProofRequest",
            "SubstrateSccpProver",
            "SubstrateSccpWitnessProvider",
            "SubstrateSccpProver.ProveFunction",
            "buildSubstrateSccpSubmission",
        ),
        "kotlin-sdk": (
            "SccpSubstrate.buildProofRequest",
            "SccpSourceProofs.buildSubstrateRuntimeStorageProofRequest",
            "SubstrateSccpProver",
            "SubstrateSccpWitnessProvider",
            "SubstrateSccpProofEngine",
            "SccpSubstrate.buildSubmission",
        ),
        "java-android": (
            "SubstrateSccpProver.buildProofRequest",
            "SourceSccpProofs.buildSubstrateRuntimeStorageProofRequest",
            "SubstrateSccpProver",
            "SubstrateSccpProver.WitnessProvider",
            "SubstrateSccpProver.ProofEngine",
            "SubstrateSccpProver.buildSubmission",
        ),
    },
}
RELEASE_CHECKLIST_KEYS = {"ready", "items"}
RELEASE_CHECKLIST_ITEM_KEYS = {"id", "title", "ready", "blockers"}
CORRIDOR_KEYS = {
    "production_ready",
    "phases",
    "evidence_artifacts",
    "require_phase_evidence",
    "blockers",
}
ALL_LANES_SUMMARY_KEYS = {
    "production_ready",
    "required_domains",
    "lanes",
    "blockers",
    "release_checklist",
}
ALL_LANES_LANE_KEYS = {
    "domain",
    "chain",
    "records",
    "production_ready",
    "source_record_hashes",
    "source_adapter_gate",
    "destination_binding",
    "route_allowlist",
    "blockers",
}
ALL_LANES_RECORD_KEYS = {
    "source_verifier_material",
    "source_adapter_deployment",
    "destination_rollout",
    "route_allowlist",
}
ALL_LANES_SOURCE_RECORD_HASH_KEYS = {
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
}
ALL_LANES_SOURCE_ADAPTER_GATE_KEYS = {
    "required",
    "ready",
    "gate_hash",
    "audit_hashes",
    "blockers",
}
ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN = {
    SCCP_DOMAIN_SOL: {
        "solana_tower_replay_verifier_hash",
        "solana_full_accountsdb_lattice_verifier_hash",
        "solana_bank_fork_choice_verifier_hash",
        "solana_full_light_client_gate_hash",
    },
    SCCP_DOMAIN_TON: {
        "ton_masterchain_config_verifier_hash",
        "ton_validator_set_transition_verifier_hash",
        "ton_shard_accounts_dictionary_verifier_hash",
        "ton_full_light_client_gate_hash",
    },
    SCCP_DOMAIN_TRON: {"tron_dpos_source_gate_hash"},
    SCCP_DOMAIN_SORA_KUSAMA: {"substrate_runtime_storage_gate_hash"},
    SCCP_DOMAIN_SORA_POLKADOT: {"substrate_runtime_storage_gate_hash"},
    SCCP_DOMAIN_SORA2: {"substrate_runtime_storage_gate_hash"},
}
ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN = {
    SCCP_DOMAIN_SOL: "solana_full_light_client_gate_hash",
    SCCP_DOMAIN_TON: "ton_full_light_client_gate_hash",
    SCCP_DOMAIN_TRON: "tron_dpos_source_gate_hash",
    SCCP_DOMAIN_SORA_KUSAMA: "substrate_runtime_storage_gate_hash",
    SCCP_DOMAIN_SORA_POLKADOT: "substrate_runtime_storage_gate_hash",
    SCCP_DOMAIN_SORA2: "substrate_runtime_storage_gate_hash",
}
ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS = {
    "destination_binding_hash",
    "destination_binding_key",
    "expected_destination_binding_hash",
    "expected_destination_binding_hash_matches",
    "recomputed",
}
ALL_LANES_DESTINATION_BINDING_OPTIONAL_KEYS = {
    "destination_bridge_address",
    "destination_network_id",
}
ALL_LANES_DESTINATION_BINDING_KEYS = (
    ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS
    | ALL_LANES_DESTINATION_BINDING_OPTIONAL_KEYS
)
ALL_LANES_EVM_DESTINATION_DOMAINS = {SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC}
ALL_LANES_STATIC_DESTINATION_DOMAINS = {
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
}
ALL_LANES_ROUTE_ALLOWLIST_KEYS = {
    "route_allowlist_hash",
    "expected_route_allowlist_hash",
    "expected_route_allowlist_hash_matches",
    "route_canary",
}
ALL_LANES_ROUTE_CANARY_COMMON_KEYS = {
    "status",
    "evidence_hash",
    "evidence_source",
    "route_allowlist_hash",
    "destination_binding_hash",
    "evidence_bound",
}
ALL_LANES_EVM_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "transaction_hash",
    "log_index",
    "receipt_block_number",
    "receipt_block_hash",
    "block_receipts_root",
    "call_data_sha256",
    "message_id",
    "payload_hash",
    "target_domain",
    "statement_hash",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "proof_version",
    "proof_source_domain",
    "message_proof_used",
}
ALL_LANES_TRON_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "transaction_id",
    "transaction_owner_address",
    "block_number",
    "block_timestamp",
    "log_index",
    "message_id",
    "call_data_sha256",
    "payload_hash",
    "target_domain",
    "statement_hash",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "proof_version",
    "proof_source_domain",
    "message_proof_used",
    "raw_data_owner_matches_transaction",
    "signature_sha256",
    "signature_recovered_address",
    "signature_recovers_to_owner",
}
ALL_LANES_SOLANA_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "solana_programdata_address",
    "solana_programdata_slot",
}
ALL_LANES_TON_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "ton_account_state_hash",
    "ton_last_transaction_hash",
    "ton_last_transaction_lt",
}
ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS = ALL_LANES_ROUTE_CANARY_COMMON_KEYS | {
    "substrate_finalized_head",
    "substrate_runtime_code_hash",
    "substrate_runtime_spec_version",
    "substrate_runtime_transaction_version",
}
ALL_LANES_ROUTE_CANARY_KEYS_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: ALL_LANES_EVM_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_BSC: ALL_LANES_EVM_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SOL: ALL_LANES_SOLANA_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_TON: ALL_LANES_TON_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_TRON: ALL_LANES_TRON_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA_KUSAMA: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA_POLKADOT: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
    SCCP_DOMAIN_SORA2: ALL_LANES_SUBSTRATE_ROUTE_CANARY_KEYS,
}
ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN = {
    SCCP_DOMAIN_ETH: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_BSC: "evm_message_proof_accepted_transaction",
    SCCP_DOMAIN_SOL: "solana_live_programdata_snapshot",
    SCCP_DOMAIN_TON: "ton_live_account_snapshot",
    SCCP_DOMAIN_TRON: "tron_message_proof_accepted_transaction",
    SCCP_DOMAIN_SORA_KUSAMA: "substrate_finalized_runtime_snapshot",
    SCCP_DOMAIN_SORA_POLKADOT: "substrate_finalized_runtime_snapshot",
    SCCP_DOMAIN_SORA2: "substrate_finalized_runtime_snapshot",
}


class DuplicateJsonKeyError(ValueError):
    """Raised when a public JSON root contains a duplicate object key."""

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


def _load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_keys,
    )


def _path_control_character(path: str) -> str | None:
    for character in path:
        if ord(character) < 0x20 or ord(character) == 0x7F:
            return repr(character)
    return None


def _canonical_json_text(payload: Any) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def _canonical_json_file_errors(label: str, path: Path, payload: Any) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        return [f"cannot load {label} JSON for canonical serialization check: {exc}"]
    if text != _canonical_json_text(payload):
        return [f"{label} JSON is not canonical release-bundle serialization"]
    return []


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _is_canonical_sha256_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(symbol in "0123456789abcdef" for symbol in value)
    )


def _load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _all_lanes_module() -> Any:
    return _load_module("_sccp_all_lanes_evidence", ALL_LANES_SCRIPT)


def _canonical_artifact_path(artifact: dict[str, Any]) -> tuple[str | None, list[str]]:
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return None, ["manifest artifact has no path"]
    control_character = _path_control_character(artifact_path)
    if control_character is not None:
        return None, [
            "manifest artifact path contains control character "
            f"{control_character}: {artifact_path!r}"
        ]
    if "\\" in artifact_path:
        return None, [f"manifest artifact path is not canonical: {artifact_path}"]
    path = PurePosixPath(artifact_path)
    if path.is_absolute() or ".." in path.parts:
        return None, [f"manifest artifact path escapes bundle: {artifact_path}"]
    if artifact_path != path.as_posix():
        return None, [f"manifest artifact path is not canonical: {artifact_path}"]
    return artifact_path, []


def _canonical_report_input_path_errors(value: Any) -> list[str]:
    if not isinstance(value, str) or not value:
        return ["readiness report inputs item must be a non-empty string"]
    control_character = _path_control_character(value)
    if control_character is not None:
        return [
            "readiness report inputs path contains control character "
            f"{control_character}: {value!r}"
        ]
    if "\\" in value:
        return [f"readiness report inputs path is not canonical: {value}"]
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        return [f"readiness report inputs path escapes bundle: {value}"]
    if value != path.as_posix():
        return [f"readiness report inputs path is not canonical: {value}"]
    return []


def _canonical_report_artifact_path_errors(label: str, value: str) -> list[str]:
    control_character = _path_control_character(value)
    if control_character is not None:
        return [
            f"{label} artifact path contains control character "
            f"{control_character}: {value!r}"
        ]
    if "\\" in value:
        return [f"{label} artifact path is not canonical: {value}"]
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        return [f"{label} artifact path escapes bundle: {value}"]
    if value != path.as_posix():
        return [f"{label} artifact path is not canonical: {value}"]
    return []


def _copied_input_layout_errors(label: str, index: int, value: Any) -> list[str]:
    if not isinstance(value, str) or _canonical_report_input_path_errors(value):
        return []
    expected_prefix = f"{index:02d}-"
    path = PurePosixPath(value)
    if (
        len(path.parts) != 2
        or path.parts[0] != "evidence"
        or not path.name.startswith(expected_prefix)
        or path.name == expected_prefix
        or not path.name.endswith(".toml")
    ):
        return [
            f"{label} path must use copied evidence layout "
            f"evidence/{expected_prefix}*.toml: {value}"
        ]
    return []


def _expected_phase_artifact_path(phase: str) -> str:
    return f"corridor/{phase}.log"


def _artifact_errors(bundle_dir: Path, artifact: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    artifact_path, path_errors = _canonical_artifact_path(artifact)
    for key in sorted(set(artifact) - ARTIFACT_KEYS):
        if artifact_path is None:
            errors.append(f"manifest artifact contains unknown field: {key}")
        else:
            errors.append(
                f"manifest artifact {artifact_path} contains unknown field: {key}"
            )
    if path_errors:
        return [*errors, *path_errors]
    assert artifact_path is not None
    if artifact_path == MANIFEST_ROOT_PATH:
        return [
            *errors,
            f"manifest must not list verifier root as an artifact: {artifact_path}",
        ]
    path = bundle_dir.joinpath(*PurePosixPath(artifact_path).parts)
    current = bundle_dir
    for part in PurePosixPath(artifact_path).parts:
        current = current / part
        if current.is_symlink():
            return [f"bundle artifact path uses symlink: {artifact_path}"]
    if not path.is_file():
        return [f"missing bundle artifact: {artifact_path}"]
    expected_bytes = artifact.get("bytes")
    expected_hash = artifact.get("sha256")
    actual_bytes = path.stat().st_size
    actual_hash = _sha256(path)
    if type(expected_bytes) is not int or expected_bytes < 0:
        errors.append(f"{artifact_path} bytes must be a non-negative integer")
    elif expected_bytes != actual_bytes:
        errors.append(
            f"{artifact_path} byte length mismatch: expected {expected_bytes}, got {actual_bytes}"
        )
    if not _is_canonical_sha256_text(expected_hash):
        errors.append(f"{artifact_path} sha256 must be a canonical SHA-256 hex string")
    elif expected_hash != actual_hash:
        errors.append(
            f"{artifact_path} sha256 mismatch: expected {expected_hash}, got {actual_hash}"
        )
    return errors


def _manifest_artifacts_by_path(
    artifacts: list[Any],
    errors: list[str],
) -> dict[str, dict[str, Any]]:
    by_path: dict[str, dict[str, Any]] = {}
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if path_errors or artifact_path is None:
            continue
        if artifact_path == MANIFEST_ROOT_PATH:
            continue
        if artifact_path in by_path:
            errors.append(f"duplicate manifest artifact path: {artifact_path}")
            continue
        by_path[artifact_path] = artifact
    return by_path


def _bundle_entry_paths(bundle_dir: Path, errors: list[str]) -> tuple[set[str], set[str]]:
    files: set[str] = set()
    directories: set[str] = set()
    try:
        candidates = sorted(bundle_dir.rglob("*"))
    except OSError as exc:
        errors.append(f"cannot enumerate bundle files: {exc}")
        return files, directories
    for candidate in candidates:
        try:
            relative = candidate.relative_to(bundle_dir).as_posix()
        except ValueError:
            errors.append(f"bundle file escapes bundle root: {candidate}")
            continue
        if candidate.is_symlink():
            errors.append(f"bundle contains symlink: {relative}")
            continue
        control_character = _path_control_character(relative)
        if control_character is not None:
            errors.append(
                "bundle contains entry path with control character "
                f"{control_character}: {relative!r}"
            )
            continue
        relative_path = PurePosixPath(relative)
        if (
            "\\" in relative
            or relative_path.is_absolute()
            or ".." in relative_path.parts
        ):
            errors.append(f"bundle contains non-canonical entry path: {relative}")
            continue
        if candidate.is_file():
            files.add(relative)
        elif candidate.is_dir():
            directories.add(relative)
        else:
            errors.append(f"bundle contains unsupported filesystem entry: {relative}")
    return files, directories


def _expected_bundle_directories(expected_paths: set[str]) -> set[str]:
    directories: set[str] = set()
    for expected_path in expected_paths:
        path = PurePosixPath(expected_path)
        parent = path.parent
        while parent != PurePosixPath("."):
            directories.add(parent.as_posix())
            parent = parent.parent
    return directories


def _check_report_artifact(
    errors: list[str],
    manifest_artifacts: dict[str, dict[str, Any]],
    artifact: Any,
    *,
    label: str,
) -> None:
    if not isinstance(artifact, dict):
        errors.append(f"{label} artifact is not an object")
        return
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        errors.append(f"{label} artifact has no path")
        return
    for key in sorted(set(artifact) - ARTIFACT_KEYS):
        errors.append(
            f"{label} artifact {artifact_path} contains unknown field: {key}"
        )
    expected_bytes = artifact.get("bytes")
    if type(expected_bytes) is not int or expected_bytes < 0:
        errors.append(
            f"{label} artifact bytes must be a non-negative integer for {artifact_path}"
        )
    expected_hash = artifact.get("sha256")
    if not _is_canonical_sha256_text(expected_hash):
        errors.append(
            f"{label} artifact sha256 must be a canonical SHA-256 hex string "
            f"for {artifact_path}"
        )
    path_errors = _canonical_report_artifact_path_errors(label, artifact_path)
    if path_errors:
        errors.extend(path_errors)
        return
    manifest_artifact = manifest_artifacts.get(artifact_path)
    if manifest_artifact is None:
        errors.append(f"{label} artifact is missing from manifest: {artifact_path}")
        return
    for field in ("bytes", "sha256"):
        if manifest_artifact.get(field) != artifact.get(field):
            errors.append(
                f"{label} artifact {field} mismatch for {artifact_path}: "
                f"manifest={manifest_artifact.get(field)!r}, "
                f"report={artifact.get(field)!r}"
            )


def _phase_transcript_errors(
    bundle_dir: Path,
    phase: str,
    artifact: Any,
) -> list[str]:
    if not isinstance(artifact, dict):
        return []
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str) or not artifact_path:
        return []
    canonical_path, path_errors = _canonical_artifact_path(artifact)
    if path_errors or canonical_path is None:
        return []
    path = bundle_dir.joinpath(*PurePosixPath(canonical_path).parts)
    try:
        transcript = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return [f"readiness report phase {phase} evidence artifact is not UTF-8 text"]
    except OSError as exc:
        return [
            f"readiness report phase {phase} evidence artifact cannot be read: {exc}"
        ]
    phase_block = _phase_transcript_block(phase, transcript)
    errors: list[str] = []
    if CORRIDOR_DRY_RUN_SENTINEL in transcript:
        errors.append(
            f"readiness report phase {phase} evidence artifact is a dry-run transcript"
        )
    if phase_block is None:
        errors.append(
            f"readiness report phase {phase} evidence artifact is missing the phase marker"
        )
    elif (
        CORRIDOR_COMPLETION_SENTINEL not in phase_block
        and not _transcript_has_full_corridor_completion(transcript)
    ):
        errors.append(
            "readiness report phase "
            f"{phase} evidence artifact is missing the phase-block completion sentinel"
        )
    required_fragments = PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.get(phase)
    if required_fragments is None:
        errors.append(
            "readiness report phase "
            f"{phase} evidence artifact has no expected command fragment configured"
        )
    elif phase_block is not None:
        for fragment in required_fragments:
            if not _phase_block_has_command_fragment(phase_block, fragment):
                errors.append(
                    "readiness report phase "
                    f"{phase} evidence artifact is missing expected "
                    f"phase-block command: {fragment}"
                )
    success_fragments = PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase)
    if success_fragments is None:
        errors.append(
            "readiness report phase "
            f"{phase} evidence artifact has no expected success fragment configured"
        )
    elif phase_block is not None:
        for fragment in success_fragments:
            if not _phase_block_has_output_fragment(phase_block, fragment):
                errors.append(
                    "readiness report phase "
                    f"{phase} evidence artifact is missing expected "
                    f"phase-block success marker: {fragment}"
                )
    return errors


def _phase_transcript_block(phase: str, transcript: str) -> str | None:
    marker = f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
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
        if lines[index].startswith(CORRIDOR_PHASE_MARKER_PREFIX):
            end = index
            break
    return "\n".join(lines[start:end])


def _transcript_has_full_corridor_completion(transcript: str) -> bool:
    lines = transcript.splitlines()
    marker_positions: list[int] = []
    for phase in CORRIDOR_PHASES:
        marker = f"{CORRIDOR_PHASE_MARKER_PREFIX}{phase}"
        try:
            marker_positions.append(lines.index(marker))
        except ValueError:
            return False
    completion_positions = [
        index
        for index, line in enumerate(lines)
        if CORRIDOR_COMPLETION_SENTINEL in line
    ]
    return bool(completion_positions) and max(completion_positions) > max(
        marker_positions
    )


def _phase_command_lines(phase_block: str) -> list[str]:
    return [
        line.strip()
        for line in phase_block.splitlines()
        if line.lstrip().startswith("+ ")
    ]


def _phase_block_has_command_fragment(phase_block: str, fragment: str) -> bool:
    return any(fragment in command for command in _phase_command_lines(phase_block))


def _phase_block_has_output_fragment(phase_block: str, fragment: str) -> bool:
    return any(
        fragment in line
        for line in phase_block.splitlines()
        if not line.lstrip().startswith("+ ")
    )


def _expected_cryptographic_evidence(evidence: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for lane in evidence.get("lanes", []):
        if not isinstance(lane, dict):
            continue
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
        source_gate = lane.get("source_adapter_gate")
        if not isinstance(source_gate, dict):
            source_gate = {}
        source_gate_audit_hashes = source_gate.get("audit_hashes")
        if not isinstance(source_gate_audit_hashes, dict):
            source_gate_audit_hashes = {}
        rows.append(
            {
                "domain": lane.get("domain"),
                "chain": lane.get("chain"),
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
                "route_canary_evidence_bound": bool(route_canary.get("evidence_bound")),
                "route_canary_block_number": route_canary.get("block_number"),
                "route_canary_block_timestamp": route_canary.get("block_timestamp"),
                "source_adapter_gate_required": bool(source_gate.get("required")),
                "source_adapter_gate_hash": source_gate.get("gate_hash") or "",
                "source_adapter_gate_audit_hashes": dict(
                    sorted(source_gate_audit_hashes.items())
                ),
            }
        )
    return rows


def _active_launch_lane(evidence: dict[str, Any]) -> dict[str, Any] | None:
    for lane in evidence.get("lanes", []):
        if isinstance(lane, dict) and lane.get("domain") == ACTIVE_LAUNCH_DOMAIN:
            return lane
    return None


def _active_launch_blockers(evidence: dict[str, Any]) -> list[str]:
    prefix = f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN}): "
    blockers: list[str] = []
    for blocker in evidence.get("blockers", []):
        if not isinstance(blocker, str):
            continue
        if blocker.startswith(prefix):
            blockers.append(blocker)
        elif not blocker.startswith("domain "):
            blockers.append(blocker)
    if _active_launch_lane(evidence) is None:
        blockers.append(
            f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN}): missing launch lane evidence"
        )
    return blockers


def _active_launch_release_checklist(evidence: dict[str, Any]) -> dict[str, Any]:
    lane = _active_launch_lane(evidence) or {}
    lane_label = f"domain {ACTIVE_LAUNCH_DOMAIN} ({ACTIVE_LAUNCH_CHAIN})"
    lane_blockers = [
        blocker
        for blocker in lane.get("blockers", [])
        if isinstance(blocker, str)
    ]
    records = lane.get("records")
    if not isinstance(records, dict):
        records = {}
    record_labels = {
        "source_verifier_material": "source verifier material",
        "source_adapter_deployment": "source adapter deployment",
        "destination_rollout": "destination rollout",
        "route_allowlist": "route allowlist",
    }
    records_blockers = [
        f"{lane_label}: missing {label}"
        for key, label in record_labels.items()
        if not records.get(key)
    ]
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
    route_blockers = [
        f"{lane_label}: {blocker}"
        for blocker in lane_blockers
        if "route allowlist" in blocker
    ]
    canary_blockers = [
        f"{lane_label}: {blocker}"
        for blocker in lane_blockers
        if "route canary" in blocker
    ]
    route_summary = lane.get("route_allowlist")
    if not isinstance(route_summary, dict):
        route_summary = {}
    canary = route_summary.get("route_canary")
    if not isinstance(canary, dict):
        canary = {}
    if canary.get("status") != "passed":
        canary_blockers.append(f"{lane_label}: route canary status is not passed")
    if not canary.get("evidence_hash"):
        canary_blockers.append(f"{lane_label}: route canary evidence hash is missing")
    if not canary.get("evidence_source"):
        canary_blockers.append(
            f"{lane_label}: live route canary evidence source is missing"
        )
    if canary.get("evidence_bound") is not True:
        canary_blockers.append(f"{lane_label}: route canary evidence is not bound")

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
            "id": "no_unresolved_blockers",
            "title": f"No active {ACTIVE_LAUNCH_DISPLAY} launch blockers remain",
            "ready": not launch_blockers,
            "blockers": launch_blockers,
        },
    ]
    return {
        "ready": all(item["ready"] for item in items),
        "items": items,
    }


def _readiness_markdown_record_flags(records: dict[str, bool]) -> str:
    labels = {
        "source_verifier_material": "source",
        "source_adapter_deployment": "deploy",
        "destination_rollout": "dest",
        "route_allowlist": "route",
    }
    return ", ".join(
        f"{label}={'yes' if records.get(field) else 'no'}"
        for field, label in labels.items()
    )


def _readiness_markdown_hash_cell(value: Any) -> str:
    if isinstance(value, str) and value:
        return f"`{value}`"
    return "-"


def _readiness_markdown_audit_hashes_cell(value: Any) -> str:
    if not isinstance(value, dict) or not value:
        return "-"
    return "<br>".join(
        f"`{key}`: `{audit_hash}`"
        for key, audit_hash in sorted(value.items())
        if isinstance(key, str) and isinstance(audit_hash, str)
    ) or "-"


def _readiness_markdown_integer_cell(value: Any) -> str:
    if type(value) is int:
        return f"`{value}`"
    return "-"


def _readiness_markdown_sdk_helper_sets_cell(surface: dict[str, Any]) -> str:
    helper_sets = surface.get("sdk_helper_symbols_by_sdk")
    if not isinstance(helper_sets, dict):
        return surface["sdk_helpers"]
    rows: list[str] = []
    lanes = surface.get("lanes")
    expected_helpers_by_sdk = (
        USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK.get(lanes, {})
        if isinstance(lanes, str)
        else {}
    )
    sdk_order = tuple(
        sdk
        for sdk in (*USER_PROVER_SDK_PHASES, "dotnet-sdk")
        if sdk in expected_helpers_by_sdk
    ) or USER_PROVER_SDK_PHASES
    for sdk in sdk_order:
        helpers = helper_sets.get(sdk)
        if not isinstance(helpers, list):
            continue
        helper_text = ", ".join(f"`{helper}`" for helper in helpers)
        rows.append(f"`{sdk}`: {helper_text}")
    return "<br>".join(rows) if rows else surface["sdk_helpers"]


def _render_readiness_markdown(
    report: dict[str, Any],
    *,
    max_blockers_per_lane: int,
) -> str:
    status = "READY" if report["production_ready"] else "NOT READY"
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
        item_status = "ready" if item["ready"] else "blocked"
        blockers = item["blockers"][:max_blockers_per_lane]
        blocker_text = "<br>".join(blockers) if blockers else "-"
        if len(item["blockers"]) > max_blockers_per_lane:
            remaining = len(item["blockers"]) - max_blockers_per_lane
            blocker_text += f"<br>... {remaining} more"
        lines.append(f"| `{item['id']}` | {item_status} | {blocker_text} |")

    lines.extend(["", "## Cryptographic Evidence", ""])
    lines.append(
        "| Domain | Chain | Source Material | Source Deployment | "
        "Destination Binding | Source Gate | Source Gate Audits | "
        "Route Allowlist | Route Canary | Canary Source | Canary Block | "
        "Canary Timestamp |"
    )
    lines.append(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )
    for row in report["cryptographic_evidence"]:
        canary_source = row["route_canary_evidence_source"] or "-"
        if not row["route_canary_evidence_bound"]:
            canary_source = f"{canary_source} (unbound)"
        source_gate = _readiness_markdown_hash_cell(row["source_adapter_gate_hash"])
        if not row["source_adapter_gate_required"] and source_gate == "-":
            source_gate = "not required"
        lines.append(
            "| {domain} | `{chain}` | {source} | {deploy} | {dest} | "
            "{source_gate} | {source_gate_audits} | {route} | {canary} | "
            "`{canary_source}` | {canary_block} | {canary_timestamp} |".format(
                domain=row["domain"],
                chain=row["chain"],
                source=_readiness_markdown_hash_cell(
                    row["source_verifier_material_hash"]
                ),
                deploy=_readiness_markdown_hash_cell(
                    row["source_adapter_engine_deployment_hash"]
                ),
                dest=_readiness_markdown_hash_cell(row["destination_binding_hash"]),
                source_gate=source_gate,
                source_gate_audits=_readiness_markdown_audit_hashes_cell(
                    row["source_adapter_gate_audit_hashes"]
                ),
                route=_readiness_markdown_hash_cell(row["route_allowlist_hash"]),
                canary=_readiness_markdown_hash_cell(
                    row["route_canary_evidence_hash"]
                ),
                canary_source=canary_source,
                canary_block=_readiness_markdown_integer_cell(
                    row["route_canary_block_number"]
                ),
                canary_timestamp=_readiness_markdown_integer_cell(
                    row["route_canary_block_timestamp"]
                ),
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
        if surface["validation_blockers"]:
            validation += ": " + "<br>".join(surface["validation_blockers"])
        lines.append(
            "| `{lanes}` | `{proof_backend}` | {sdk_helpers} | {submission} | "
            "{required_phases} | {validation} |".format(
                lanes=surface["lanes"],
                proof_backend=surface["proof_backend"],
                sdk_helpers=_readiness_markdown_sdk_helper_sets_cell(surface),
                submission=surface["on_chain_submission"],
                required_phases=required_phases,
                validation=validation,
            )
        )

    lines.extend(["", "## Lane Readiness", ""])
    lines.append("| Domain | Chain | Status | Records | Blockers |")
    lines.append("| --- | --- | --- | --- | --- |")
    for lane in report["evidence"]["lanes"]:
        lane_status = "ready" if lane["production_ready"] else "blocked"
        blockers = lane["blockers"][:max_blockers_per_lane]
        blocker_text = "<br>".join(blockers) if blockers else "-"
        if len(lane["blockers"]) > max_blockers_per_lane:
            remaining = len(lane["blockers"]) - max_blockers_per_lane
            blocker_text += f"<br>... {remaining} more"
        lines.append(
            "| {domain} | `{chain}` | {status} | {records} | {blockers} |".format(
                domain=lane["domain"],
                chain=lane["chain"],
                status=lane_status,
                records=_readiness_markdown_record_flags(lane["records"]),
                blockers=blocker_text,
            )
        )

    lines.extend(["", "## Blocking Items", ""])
    if report["blockers"]:
        for blocker in report["blockers"]:
            lines.append(f"- {blocker}")
    else:
        lines.append("- None")

    lines.extend(
        [
            "",
            "## Required Release Evidence",
            "",
            "- A passing `bash scripts/check_sccp_production_corridor.sh` run, recorded with `--require-phase-evidence` and one hashed `--phase-evidence` artifact for every passed phase.",
            "- Passing web/mobile SDK artifacts for the user-prover helper surface, including the JavaScript/web source, packaged `dist`, and TypeScript declaration exports used by portal builds.",
            f"- Complete {ACTIVE_LAUNCH_DISPLAY} launch-lane evidence containing source verifier material, source-adapter deployment, destination rollout, route allowlist, and route canary records; the all-lanes summary remains attached as diagnostic evidence for future lanes.",
            "- Governed live deployment evidence for immutable destination verifiers and source-chain verifier engines; offline placeholder or template-derived hashes keep the report blocked.",
            "- Public release notes must attach this report and the all-lanes JSON summary before production activation.",
        ]
    )
    return "\n".join(lines) + "\n"


def _expected_readiness_markdown(report: dict[str, Any]) -> str:
    ordered_report = copy.deepcopy(report)
    corridor = ordered_report.get("corridor")
    if isinstance(corridor, dict):
        for field in ("phases", "evidence_artifacts"):
            values = corridor.get(field)
            if not isinstance(values, dict):
                continue
            ordered = {
                phase: values[phase]
                for phase in CORRIDOR_PHASES
                if phase in values
            }
            for phase in sorted(set(values) - set(ordered)):
                ordered[phase] = values[phase]
            corridor[field] = ordered
    return _render_readiness_markdown(ordered_report, max_blockers_per_lane=4)


def _expected_release_notes_attachment(
    report: dict[str, Any],
    artifacts: list[Any],
) -> str:
    attachment_artifacts = [
        artifact
        for artifact in artifacts
        if (
            isinstance(artifact, dict)
            and artifact.get("path") != "sccp-release-notes-attachment.md"
        )
    ]
    status = "READY" if report["production_ready"] else "NOT READY"
    lines = [
        "# SCCP Public Release Notes Attachment",
        "",
        f"Status: {status}",
        "",
        (
            "Attach `manifest.json` plus every artifact below to the public release "
            "notes before production activation."
        ),
        "",
        (
            "`manifest.json` is the verifier root and is intentionally not listed "
            "in its own artifact table."
        ),
        "",
        "| Artifact | Bytes | SHA-256 |",
        "| --- | ---: | --- |",
    ]
    for artifact in attachment_artifacts:
        lines.append(
            "| `{path}` | {bytes} | `{sha256}` |".format(
                path=artifact["path"],
                bytes=artifact["bytes"],
                sha256=artifact["sha256"],
            )
        )
    if report["blockers"]:
        lines.extend(["", "## Blocking Items", ""])
        lines.extend(f"- {blocker}" for blocker in report["blockers"])
    return "\n".join(lines) + "\n"


def _markdown_sections(
    markdown: str,
    required_headings: tuple[str, ...],
) -> tuple[dict[str, str], list[str]]:
    errors: list[str] = []
    lines = markdown.splitlines()
    heading_positions: dict[str, int] = {}
    previous_position = -1
    for heading in required_headings:
        matches = [
            index for index, line in enumerate(lines) if line.strip() == heading
        ]
        if not matches:
            errors.append(f"readiness report Markdown missing section: {heading}")
            continue
        if len(matches) > 1:
            errors.append(f"readiness report Markdown repeats section: {heading}")
            continue
        position = matches[0]
        if position <= previous_position:
            errors.append(
                "readiness report Markdown section order is not canonical: "
                f"{heading}"
            )
        previous_position = position
        heading_positions[heading] = position

    sections: dict[str, str] = {}
    for heading, position in heading_positions.items():
        end = len(lines)
        for index in range(position + 1, len(lines)):
            if lines[index].startswith("## "):
                end = index
                break
        sections[heading] = "\n".join(lines[position + 1 : end])
    return sections, errors


def _markdown_missing_value_errors(
    section_name: str,
    section_text: str,
    value: Any,
    label: str,
) -> list[str]:
    if value is None or value == "":
        return []
    text = str(value)
    if text not in section_text:
        return [
            f"readiness report Markdown {section_name} section missing {label}: {text}"
        ]
    return []


def _readiness_markdown_invariant_errors(
    report: dict[str, Any],
    markdown: str,
) -> list[str]:
    errors: list[str] = []
    lines = markdown.splitlines()
    if not lines or lines[0] != "# SCCP Release Readiness Report":
        errors.append("readiness report Markdown missing canonical title")
    if not markdown.endswith("\n"):
        errors.append("readiness report Markdown must end with a newline")
    status = "READY" if report.get("production_ready") is True else "NOT READY"
    if f"Status: {status}" not in lines:
        errors.append(f"readiness report Markdown missing status line: {status}")

    sections, section_errors = _markdown_sections(
        markdown,
        READINESS_MARKDOWN_REQUIRED_HEADINGS,
    )
    errors.extend(section_errors)

    evidence_section = sections.get("## Evidence Inputs", "")
    input_artifacts = report.get("input_artifacts")
    if isinstance(input_artifacts, list):
        for artifact in input_artifacts:
            if not isinstance(artifact, dict):
                continue
            artifact_path = artifact.get("path")
            errors.extend(
                _markdown_missing_value_errors(
                    "Evidence Inputs",
                    evidence_section,
                    artifact_path,
                    "input artifact path",
                )
            )
            errors.extend(
                _markdown_missing_value_errors(
                    "Evidence Inputs",
                    evidence_section,
                    artifact.get("sha256"),
                    f"input artifact hash for {artifact_path}",
                )
            )

    corridor_section = sections.get("## Production Corridor", "")
    corridor = report.get("corridor")
    if isinstance(corridor, dict):
        phases = corridor.get("phases")
        if isinstance(phases, dict):
            for phase, phase_status in phases.items():
                errors.extend(
                    _markdown_missing_value_errors(
                        "Production Corridor",
                        corridor_section,
                        phase,
                        "phase",
                    )
                )
                errors.extend(
                    _markdown_missing_value_errors(
                        "Production Corridor",
                        corridor_section,
                        phase_status,
                        f"status for phase {phase}",
                    )
                )
        phase_artifacts = corridor.get("evidence_artifacts")
        if isinstance(phase_artifacts, dict):
            for phase, artifact in phase_artifacts.items():
                if not isinstance(artifact, dict):
                    continue
                errors.extend(
                    _markdown_missing_value_errors(
                        "Production Corridor",
                        corridor_section,
                        artifact.get("path"),
                        f"evidence artifact path for phase {phase}",
                    )
                )
                errors.extend(
                    _markdown_missing_value_errors(
                        "Production Corridor",
                        corridor_section,
                        artifact.get("sha256"),
                        f"evidence artifact hash for phase {phase}",
                    )
                )

    checklist_section = sections.get("## Release Checklist", "")
    release_checklist = report.get("release_checklist")
    if isinstance(release_checklist, dict):
        items = release_checklist.get("items")
        if isinstance(items, list):
            for item in items:
                if not isinstance(item, dict):
                    continue
                item_id = item.get("id")
                errors.extend(
                    _markdown_missing_value_errors(
                        "Release Checklist",
                        checklist_section,
                        item_id,
                        "gate",
                    )
                )
                item_status = "ready" if item.get("ready") is True else "blocked"
                errors.extend(
                    _markdown_missing_value_errors(
                        "Release Checklist",
                        checklist_section,
                        item_status,
                        f"status for gate {item_id}",
                    )
                )

    crypto_section = sections.get("## Cryptographic Evidence", "")
    crypto_rows = report.get("cryptographic_evidence")
    if isinstance(crypto_rows, list):
        for row in crypto_rows:
            if not isinstance(row, dict):
                continue
            domain = row.get("domain")
            chain = row.get("chain")
            errors.extend(
                _markdown_missing_value_errors(
                    "Cryptographic Evidence",
                    crypto_section,
                    domain,
                    "domain",
                )
            )
            errors.extend(
                _markdown_missing_value_errors(
                    "Cryptographic Evidence",
                    crypto_section,
                    chain,
                    f"chain for domain {domain}",
                )
            )
            for field in (
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
                "destination_binding_hash",
                "route_allowlist_hash",
                "route_canary_evidence_hash",
                "route_canary_evidence_source",
                "source_adapter_gate_hash",
            ):
                errors.extend(
                    _markdown_missing_value_errors(
                        "Cryptographic Evidence",
                        crypto_section,
                        row.get(field),
                        f"{field} for domain {domain}",
                    )
                )
            gate_audits = row.get("source_adapter_gate_audit_hashes")
            if isinstance(gate_audits, dict):
                for audit_key, audit_hash in gate_audits.items():
                    errors.extend(
                        _markdown_missing_value_errors(
                            "Cryptographic Evidence",
                            crypto_section,
                            audit_key,
                            f"source_adapter_gate_audit_hashes key for domain {domain}",
                        )
                    )
                    errors.extend(
                        _markdown_missing_value_errors(
                            "Cryptographic Evidence",
                            crypto_section,
                            audit_hash,
                            f"source_adapter_gate_audit_hashes value for domain {domain}",
                        )
                    )
            for field in (
                "route_canary_block_number",
                "route_canary_block_timestamp",
            ):
                errors.extend(
                    _markdown_missing_value_errors(
                        "Cryptographic Evidence",
                        crypto_section,
                        row.get(field),
                        f"{field} for domain {domain}",
                    )
                )

    surfaces_section = sections.get("## User Prover Submission Surfaces", "")
    surfaces = report.get("user_prover_submission_surfaces")
    if isinstance(surfaces, list):
        for surface in surfaces:
            if not isinstance(surface, dict):
                continue
            lanes = surface.get("lanes")
            for field in ("lanes", "proof_backend", "on_chain_submission"):
                errors.extend(
                    _markdown_missing_value_errors(
                        "User Prover Submission Surfaces",
                        surfaces_section,
                        surface.get(field),
                        f"{field} for lanes {lanes}",
                    )
                )
            helper_sets = surface.get("sdk_helper_symbols_by_sdk")
            if isinstance(helper_sets, dict):
                for sdk, helpers in helper_sets.items():
                    errors.extend(
                        _markdown_missing_value_errors(
                            "User Prover Submission Surfaces",
                            surfaces_section,
                            sdk,
                            f"SDK row for lanes {lanes}",
                        )
                    )
                    if not isinstance(helpers, list):
                        continue
                    for helper in helpers:
                        errors.extend(
                            _markdown_missing_value_errors(
                                "User Prover Submission Surfaces",
                                surfaces_section,
                                helper,
                                f"helper for lanes {lanes}",
                            )
                        )
            required_phases = surface.get("required_phases")
            if isinstance(required_phases, list):
                for phase in required_phases:
                    errors.extend(
                        _markdown_missing_value_errors(
                            "User Prover Submission Surfaces",
                            surfaces_section,
                            phase,
                            f"required phase for lanes {lanes}",
                        )
                    )

    lane_section = sections.get("## Lane Readiness", "")
    evidence = report.get("evidence")
    if isinstance(evidence, dict):
        lanes = evidence.get("lanes")
        if isinstance(lanes, list):
            for lane in lanes:
                if not isinstance(lane, dict):
                    continue
                domain = lane.get("domain")
                errors.extend(
                    _markdown_missing_value_errors(
                        "Lane Readiness",
                        lane_section,
                        domain,
                        "domain",
                    )
                )
                errors.extend(
                    _markdown_missing_value_errors(
                        "Lane Readiness",
                        lane_section,
                        lane.get("chain"),
                        f"chain for domain {domain}",
                    )
                )
                lane_status = (
                    "ready" if lane.get("production_ready") is True else "blocked"
                )
                errors.extend(
                    _markdown_missing_value_errors(
                        "Lane Readiness",
                        lane_section,
                        lane_status,
                        f"status for domain {domain}",
                    )
                )

    blockers_section = sections.get("## Blocking Items", "")
    blockers = report.get("blockers")
    if isinstance(blockers, list):
        if blockers:
            for blocker in blockers:
                errors.extend(
                    _markdown_missing_value_errors(
                        "Blocking Items",
                        blockers_section,
                        blocker,
                        "blocker",
                    )
                )
        elif "- None" not in blockers_section:
            errors.append("readiness report Markdown Blocking Items section missing - None")

    required_evidence_section = sections.get("## Required Release Evidence", "")
    for marker in READINESS_MARKDOWN_REQUIRED_RELEASE_EVIDENCE_MARKERS:
        errors.extend(
            _markdown_missing_value_errors(
                "Required Release Evidence",
                required_evidence_section,
                marker,
                "release evidence marker",
            )
        )

    return errors


def _manifest_artifact_paths_in_order(artifacts: list[Any]) -> list[str]:
    paths: list[str] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.append(artifact_path)
    return paths


def _expected_manifest_artifact_order(report: dict[str, Any]) -> list[str]:
    paths = [
        "sccp-release-readiness.md",
        "sccp-release-readiness.json",
        "sccp-all-lanes-summary.json",
        *_expected_input_paths(report),
    ]

    corridor = report.get("corridor")
    if isinstance(corridor, dict):
        phases = corridor.get("phases")
        phase_artifacts = corridor.get("evidence_artifacts")
        if isinstance(phases, dict) and isinstance(phase_artifacts, dict):
            for phase in CORRIDOR_PHASES:
                if phases.get(phase) != "passed":
                    continue
                artifact = phase_artifacts.get(phase)
                if not isinstance(artifact, dict):
                    continue
                artifact_path, path_errors = _canonical_artifact_path(artifact)
                if not path_errors and artifact_path is not None:
                    paths.append(artifact_path)

    paths.append("sccp-release-notes-attachment.md")
    return paths


def _expected_submission_surfaces(report: dict[str, Any]) -> list[dict[str, Any]]:
    corridor = report.get("corridor")
    phase_status = {}
    if isinstance(corridor, dict) and isinstance(corridor.get("phases"), dict):
        phase_status = corridor["phases"]

    surfaces: list[dict[str, Any]] = []
    for lanes, proof_backend in USER_PROVER_REQUIRED_LANE_BACKENDS.items():
        helper_sets = USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK[lanes]
        js_helpers = list(helper_sets["js-sdk"])
        sdk_order = tuple(
            sdk for sdk in (*USER_PROVER_SDK_PHASES, "dotnet-sdk") if sdk in helper_sets
        )
        required_phases = list(USER_PROVER_SDK_PHASES)
        if lanes == "eth,bsc":
            required_phases.append("dotnet-sdk")
        if proof_backend in USER_PROVER_CONTRACT_SMOKE_BACKENDS:
            required_phases.append("contract-smoke")
        required_phases.append("core-admission")
        blockers = [
            f"{phase} is {phase_status.get(phase, 'missing')}"
            for phase in required_phases
            if phase_status.get(phase) != "passed"
        ]
        surfaces.append(
            {
                "lanes": lanes,
                "proof_backend": proof_backend,
                "sdk_helper_symbols": js_helpers,
                "sdk_helper_symbols_by_sdk": {
                    sdk: list(helper_sets[sdk]) for sdk in sdk_order
                },
                "sdk_helpers": ", ".join(js_helpers),
                "on_chain_submission": USER_PROVER_ON_CHAIN_SUBMISSION_BY_LANE[
                    lanes
                ],
                "required_phases": required_phases,
                "validation_status": "passed" if not blockers else "blocked",
                "validation_blockers": blockers,
            }
        )
    return surfaces


def _corridor_phase_errors(corridor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    phases = corridor.get("phases")
    if not isinstance(phases, dict):
        return ["readiness report corridor phases is not an object"]
    phase_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(phase_artifacts, dict):
        phase_artifacts = {}

    expected_set = set(CORRIDOR_PHASES)
    for phase in sorted(set(phases) - expected_set):
        errors.append(f"readiness report corridor has unknown phase status: {phase}")
    for phase in CORRIDOR_PHASES:
        if phase not in phases:
            errors.append(f"readiness report corridor missing phase status: {phase}")
            continue
        status = phases[phase]
        if status != "passed":
            errors.append(
                f"readiness report corridor phase {phase} is not passed: {status!r}"
            )
        artifact = phase_artifacts.get(phase)
        if not isinstance(artifact, dict):
            errors.append(
                "readiness report corridor phase "
                f"{phase} has no hashed evidence artifact"
            )
            continue
        expected_path = _expected_phase_artifact_path(phase)
        if artifact.get("path") != expected_path:
            errors.append(
                "readiness report phase "
                f"{phase} evidence artifact path must be {expected_path}"
            )
    if corridor.get("blockers"):
        errors.append("readiness report production corridor contains blockers")
    return errors


def _expected_release_checklist(report: dict[str, Any]) -> dict[str, Any]:
    evidence = report.get("evidence")
    if not isinstance(evidence, dict):
        return {"ready": False, "items": []}
    return _active_launch_release_checklist(evidence)


def _boolean_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if type(payload.get(field)) is not bool:
        return [f"{label} {field} must be a boolean"]
    return []


def _list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if not isinstance(payload.get(field), list):
        return [f"{label} {field} must be a list"]
    return []


def _is_canonical_fixed_hex_text(value: Any, *, byte_length: int) -> bool:
    if not isinstance(value, str):
        return False
    if len(value) != 2 + byte_length * 2 or not value.startswith("0x"):
        return False
    text = value[2:]
    return all(symbol in "0123456789abcdef" for symbol in text)


def _is_canonical_hex32_text(value: Any) -> bool:
    return _is_canonical_fixed_hex_text(value, byte_length=32)


def _canonical_fixed_hex_bytes(value: Any, *, byte_length: int) -> bytes | None:
    if not _is_canonical_fixed_hex_text(value, byte_length=byte_length):
        return None
    assert isinstance(value, str)
    raw = bytes.fromhex(value[2:])
    if not any(raw):
        return None
    return raw


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little"))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    digest = hashlib.blake2b(digest_size=32)
    digest.update(prefix)
    digest.update(payload)
    return digest.digest()


def _canonical_route_allowlist_hash(
    *,
    domain: int,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> str | None:
    chain = ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
    route_allowlist_id = ALL_LANES_ROUTE_ALLOWLIST_ID_BY_DOMAIN.get(domain)
    if chain is None or route_allowlist_id is None:
        return None
    if len(
        {
            source_verifier_material_hash,
            source_adapter_engine_deployment_hash,
            destination_binding_hash,
        }
    ) != 3:
        return None

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_vec(payload, chain.encode("utf-8"))
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, route_allowlist_id.encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return "0x" + _prefixed_blake2b(
        b"sccp:route-allowlist:lane-evidence:v1",
        bytes(payload),
    ).hex()


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


def _u32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        return [f"{label} {field} must be a u32 integer"]
    return []


def _integer_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    positive: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if type(value) is not int or value < 0 or (positive and value == 0):
        qualifier = "positive " if positive else "non-negative "
        return [f"{label} {field} must be a {qualifier}integer"]
    return []


def _expected_u32_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    expected: int,
    expected_label: str,
) -> list[str]:
    errors = _u32_field_errors(label, payload, field)
    if errors:
        return errors
    if field in payload and payload.get(field) != expected:
        return [f"{label} {field} must be {expected_label}"]
    return []


def _true_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    errors = _boolean_field_errors(label, payload, field)
    if errors:
        return errors
    if field in payload and payload.get(field) is not True:
        return [f"{label} {field} must be true"]
    return []


def _cryptographic_evidence_row_schema_errors(row: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for key in sorted(CRYPTOGRAPHIC_EVIDENCE_KEYS - set(row)):
        errors.append(f"readiness report cryptographic evidence row missing field: {key}")
    if "domain" in row and type(row.get("domain")) is not int:
        errors.append("readiness report cryptographic evidence row domain must be an integer")
    if "chain" in row and (
        not isinstance(row.get("chain"), str) or not row.get("chain")
    ):
        errors.append(
            "readiness report cryptographic evidence row chain must be a non-empty string"
        )
    if "route_canary_evidence_bound" in row and (
        type(row.get("route_canary_evidence_bound")) is not bool
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_evidence_bound must be a boolean"
        )
    if "source_adapter_gate_required" in row and (
        type(row.get("source_adapter_gate_required")) is not bool
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "source_adapter_gate_required must be a boolean"
        )
    audit_hashes = row.get("source_adapter_gate_audit_hashes")
    if "source_adapter_gate_audit_hashes" in row and not isinstance(audit_hashes, dict):
        errors.append(
            "readiness report cryptographic evidence row "
            "source_adapter_gate_audit_hashes must be an object"
        )
        audit_hashes = {}
    if row.get("domain") != ACTIVE_LAUNCH_DOMAIN:
        return errors
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        errors.extend(
            _nonzero_fixed_hex_field_errors(
                "readiness report cryptographic evidence row",
                row,
                field,
                byte_length=32,
                type_label="bytes32",
            )
        )
    if "route_canary_evidence_source" in row and (
        not isinstance(row.get("route_canary_evidence_source"), str)
        or not row.get("route_canary_evidence_source")
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_evidence_source must be a non-empty string"
        )
    expected_canary_source = ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN.get(
        row.get("domain")
    )
    if (
        expected_canary_source is not None
        and isinstance(row.get("route_canary_evidence_source"), str)
        and row.get("route_canary_evidence_source") != expected_canary_source
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            f"route_canary_evidence_source must be {expected_canary_source}"
        )
    if row.get("domain") == SCCP_DOMAIN_TRON:
        errors.extend(
            _integer_field_errors(
                "readiness report cryptographic evidence row",
                row,
                "route_canary_block_number",
                positive=True,
            )
        )
        errors.extend(
            _integer_field_errors(
                "readiness report cryptographic evidence row",
                row,
                "route_canary_block_timestamp",
                positive=False,
            )
        )
    else:
        for field in ("route_canary_block_number", "route_canary_block_timestamp"):
            if field in row and row.get(field) is not None:
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be null for non-TRON lanes"
                )
    errors.extend(
        _empty_or_nonzero_fixed_hex_field_errors(
            "readiness report cryptographic evidence row",
            row,
            "source_adapter_gate_hash",
            byte_length=32,
            type_label="bytes32",
        )
    )
    if isinstance(audit_hashes, dict):
        for field, value in sorted(audit_hashes.items()):
            if not isinstance(field, str) or not field:
                errors.append(
                    "readiness report cryptographic evidence row "
                    "source_adapter_gate_audit_hashes contains an empty key"
                )
                continue
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    "readiness report cryptographic evidence row "
                    "source_adapter_gate_audit_hashes",
                    audit_hashes,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
    if row.get("source_adapter_gate_required") is True:
        gate_hash = row.get("source_adapter_gate_hash")
        expected_audit_keys = ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.get(
            row.get("domain")
        )
        if expected_audit_keys is None:
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_required must be false for this domain"
            )
        elif isinstance(audit_hashes, dict):
            for key in sorted(set(audit_hashes) - expected_audit_keys):
                errors.append(
                    "readiness report cryptographic evidence row "
                    "source_adapter_gate_audit_hashes contains unexpected "
                    f"field: {key}"
                )
            for key in sorted(expected_audit_keys - set(audit_hashes)):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"source_adapter_gate_audit_hashes missing field: {key}"
                )
        if not gate_hash:
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_hash must not be empty when required"
            )
        if not audit_hashes:
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_audit_hashes must not be empty when required"
            )
        if (
            _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
            and isinstance(audit_hashes, dict)
            and audit_hashes
            and gate_hash not in set(audit_hashes.values())
        ):
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_hash must match one "
                "source_adapter_gate_audit_hashes value"
            )
        expected_gate_key = ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN.get(
            row.get("domain")
        )
        expected_gate_hash = (
            audit_hashes.get(expected_gate_key)
            if expected_gate_key is not None and isinstance(audit_hashes, dict)
            else None
        )
        if (
            expected_gate_key is not None
            and _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
            and _is_canonical_fixed_hex_text(expected_gate_hash, byte_length=32)
            and gate_hash != expected_gate_hash
        ):
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_hash must match "
                f"source_adapter_gate_audit_hashes.{expected_gate_key}"
            )
    elif row.get("source_adapter_gate_required") is False:
        if row.get("domain") in ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN:
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_required must be true for this domain"
            )
        if row.get("source_adapter_gate_hash") not in (None, ""):
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_hash must be empty when gate is not required"
            )
        if audit_hashes:
            errors.append(
                "readiness report cryptographic evidence row "
                "source_adapter_gate_audit_hashes must be empty when gate is not required"
            )
    return errors


def _cryptographic_evidence_lane_binding_errors(
    crypto: list[Any],
    lanes: Any,
) -> list[str]:
    errors: list[str] = []
    if not isinstance(lanes, list):
        return errors
    seen_domains: set[int] = set()
    for index, row in enumerate(crypto):
        if not isinstance(row, dict):
            continue
        domain = row.get("domain")
        if type(domain) is int:
            if domain in seen_domains:
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{index} duplicates domain {domain}"
                )
            seen_domains.add(domain)
        if index >= len(lanes) or not isinstance(lanes[index], dict):
            continue
        lane = lanes[index]
        lane_domain = lane.get("domain")
        if type(domain) is int and type(lane_domain) is int and domain != lane_domain:
            errors.append(
                "readiness report cryptographic evidence row "
                f"{index} domain must match lane domain"
            )
        chain = row.get("chain")
        lane_chain = lane.get("chain")
        if (
            isinstance(chain, str)
            and chain
            and isinstance(lane_chain, str)
            and lane_chain
            and chain != lane_chain
        ):
            errors.append(
                "readiness report cryptographic evidence row "
                f"{index} chain must match lane chain"
            )
        field_bindings = (
            (
                "source_verifier_material_hash",
                ("source_record_hashes", "source_verifier_material_hash"),
            ),
            (
                "source_adapter_engine_deployment_hash",
                ("source_record_hashes", "source_adapter_engine_deployment_hash"),
            ),
            ("destination_binding_hash", ("destination_binding", "destination_binding_hash")),
            ("route_allowlist_hash", ("route_allowlist", "route_allowlist_hash")),
            (
                "route_canary_evidence_hash",
                ("route_allowlist", "route_canary", "evidence_hash"),
            ),
            (
                "route_canary_evidence_source",
                ("route_allowlist", "route_canary", "evidence_source"),
            ),
            (
                "route_canary_evidence_bound",
                ("route_allowlist", "route_canary", "evidence_bound"),
            ),
            (
                "route_canary_block_number",
                ("route_allowlist", "route_canary", "block_number"),
            ),
            (
                "route_canary_block_timestamp",
                ("route_allowlist", "route_canary", "block_timestamp"),
            ),
            ("source_adapter_gate_required", ("source_adapter_gate", "required")),
            ("source_adapter_gate_hash", ("source_adapter_gate", "gate_hash")),
            (
                "source_adapter_gate_audit_hashes",
                ("source_adapter_gate", "audit_hashes"),
            ),
        )
        for field, lane_path in field_bindings:
            if field not in row:
                continue
            expected: Any = lane
            for segment in lane_path:
                if not isinstance(expected, dict) or segment not in expected:
                    expected = None
                    break
                expected = expected[segment]
            if expected is not None and row.get(field) != expected:
                lane_field = ".".join(lane_path)
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{index} {field} must match embedded lane {lane_field}"
                )
    return errors


def _cryptographic_evidence_inventory_errors(crypto: list[Any]) -> list[str]:
    label = "readiness report cryptographic_evidence"
    errors: list[str] = []
    seen_domains: set[int] = set()
    for row in crypto:
        if not isinstance(row, dict):
            continue
        domain = row.get("domain")
        if type(domain) is not int:
            continue
        if domain in seen_domains:
            errors.append(f"{label} contains duplicate domain: {domain}")
        else:
            seen_domains.add(domain)
        expected_chain = ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
        if expected_chain is None:
            errors.append(f"{label} contains unknown domain: {domain}")
            continue
        chain = row.get("chain")
        if isinstance(chain, str) and chain and chain != expected_chain:
            errors.append(
                f"{label} chain mismatch for domain {domain}: "
                f"expected {expected_chain}, got {chain!r}"
            )
    for domain in ALL_LANES_REQUIRED_DOMAINS:
        if domain not in seen_domains:
            errors.append(f"{label} missing required domain: {domain}")
    return errors


def _non_empty_string_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    if not isinstance(payload.get(field), str) or not payload.get(field):
        return [f"{label} {field} must be a non-empty string"]
    return []


def _string_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, list) or (
        not allow_empty and not value
    ) or any(not isinstance(item, str) or not item for item in value):
        return [f"{label} {field} must be a list of non-empty strings"]
    return []


def _integer_list_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    allow_empty: bool,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, list) or (
        not allow_empty and not value
    ) or any(type(item) is not int for item in value):
        return [f"{label} {field} must be a list of integers"]
    return []


def _submission_surface_row_schema_errors(row: dict[str, Any]) -> list[str]:
    label = "readiness report user prover submission surface row"
    errors: list[str] = []
    for field in ("lanes", "proof_backend", "sdk_helpers", "on_chain_submission"):
        errors.extend(_non_empty_string_field_errors(label, row, field))
    errors.extend(
        _string_list_field_errors(label, row, "sdk_helper_symbols", allow_empty=False)
    )
    helper_symbols = row.get("sdk_helper_symbols")
    if isinstance(helper_symbols, list) and all(
        isinstance(item, str) and item for item in helper_symbols
    ):
        if len(helper_symbols) != len(set(helper_symbols)):
            errors.append(
                "readiness report user prover submission surface row "
                "sdk_helper_symbols contains duplicate symbols"
            )
        expected_helpers = ", ".join(helper_symbols)
        if row.get("sdk_helpers") != expected_helpers:
            errors.append(
                "readiness report user prover submission surface row "
                "sdk_helpers must match sdk_helper_symbols"
            )
    helper_sets = row.get("sdk_helper_symbols_by_sdk")
    if not isinstance(helper_sets, dict):
        errors.append(
            "readiness report user prover submission surface row "
            "sdk_helper_symbols_by_sdk must be an object"
        )
    else:
        lanes = row.get("lanes")
        expected_helpers_by_sdk = (
            USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK.get(lanes, {})
            if isinstance(lanes, str)
            else {}
        )
        expected_sdk_order = tuple(
            sdk
            for sdk in (*USER_PROVER_SDK_PHASES, "dotnet-sdk")
            if sdk in expected_helpers_by_sdk
        ) or USER_PROVER_SDK_PHASES
        expected_sdks = set(expected_sdk_order)
        for sdk in sorted(set(helper_sets) - expected_sdks):
            errors.append(
                "readiness report user prover submission surface row "
                f"sdk_helper_symbols_by_sdk contains unknown SDK: {sdk}"
            )
        for sdk in expected_sdk_order:
            helpers = helper_sets.get(sdk)
            if (
                not isinstance(helpers, list)
                or not helpers
                or any(not isinstance(item, str) or not item for item in helpers)
            ):
                errors.append(
                    "readiness report user prover submission surface row "
                    f"sdk_helper_symbols_by_sdk[{sdk}] must be a list of "
                    "non-empty strings"
                )
                continue
            if len(helpers) != len(set(helpers)):
                errors.append(
                    "readiness report user prover submission surface row "
                    f"sdk_helper_symbols_by_sdk[{sdk}] contains duplicate symbols"
                )
            for marker in USER_PROVER_SDK_HOOK_MARKERS.get(sdk, ()):
                if not any(
                    _helper_matches_hook_marker(sdk, helper, marker)
                    for helper in helpers
                ):
                    errors.append(
                        "readiness report user prover submission surface row "
                        f"sdk_helper_symbols_by_sdk[{sdk}] missing UI-owned "
                        f"hook marker: {marker}"
                    )
        js_helpers = helper_sets.get("js-sdk")
        if (
            isinstance(js_helpers, list)
            and isinstance(helper_symbols, list)
            and js_helpers != helper_symbols
        ):
            errors.append(
                "readiness report user prover submission surface row "
                "sdk_helper_symbols_by_sdk[js-sdk] must match sdk_helper_symbols"
            )
    errors.extend(
        _string_list_field_errors(label, row, "required_phases", allow_empty=False)
    )
    required_phases = row.get("required_phases")
    if isinstance(required_phases, list) and all(
        isinstance(item, str) and item for item in required_phases
    ):
        if len(required_phases) != len(set(required_phases)):
            errors.append(
                "readiness report user prover submission surface row "
                "required_phases contains duplicate phases"
            )
        for phase in sorted(
            set(required_phases) - set(USER_PROVER_KNOWN_REQUIRED_PHASES)
        ):
            errors.append(
                "readiness report user prover submission surface row "
                f"required_phases contains unknown phase: {phase}"
            )
        for phase in USER_PROVER_REQUIRED_PHASES:
            if phase not in required_phases:
                errors.append(
                    "readiness report user prover submission surface row "
                    f"required_phases missing required phase: {phase}"
                )
        proof_backend = row.get("proof_backend")
        if (
            proof_backend in USER_PROVER_CONTRACT_SMOKE_BACKENDS
            and "contract-smoke" not in required_phases
        ):
            errors.append(
                "readiness report user prover submission surface row "
                "required_phases missing required phase: contract-smoke"
            )
    if "validation_status" in row and row.get("validation_status") not in {
        "passed",
        "blocked",
    }:
        errors.append(
            "readiness report user prover submission surface row "
            "validation_status must be passed or blocked"
        )
    if row.get("validation_status") == "blocked":
        errors.append(
            "readiness report user prover submission surface row "
            "validation_status must be passed"
        )
    errors.extend(
        _string_list_field_errors(label, row, "validation_blockers", allow_empty=True)
    )
    blockers = row.get("validation_blockers")
    if isinstance(blockers, list) and blockers:
        errors.append(
            "readiness report user prover submission surface row "
            "validation_blockers must be empty"
        )
    return errors


def _submission_surface_inventory_errors(surfaces: list[Any]) -> list[str]:
    label = "readiness report user_prover_submission_surfaces"
    errors: list[str] = []
    seen_lanes: set[str] = set()
    for row in surfaces:
        if not isinstance(row, dict):
            continue
        lanes = row.get("lanes")
        proof_backend = row.get("proof_backend")
        if not isinstance(lanes, str) or not lanes:
            continue
        if lanes in seen_lanes:
            errors.append(f"{label} contains duplicate lanes row: {lanes}")
        else:
            seen_lanes.add(lanes)
        expected_backend = (
            USER_PROVER_REQUIRED_LANE_BACKENDS.get(lanes)
            if isinstance(lanes, str)
            else None
        )
        if expected_backend is None:
            errors.append(f"{label} contains unknown lanes row: {lanes}")
        elif proof_backend != expected_backend:
            errors.append(
                f"{label} proof_backend mismatch for lanes {lanes}: "
                f"expected {expected_backend}, got {proof_backend!r}"
            )
        expected_helpers_by_sdk = (
            USER_PROVER_REQUIRED_HELPERS_BY_LANE_SDK.get(lanes)
            if isinstance(lanes, str)
            else None
        )
        helper_sets = row.get("sdk_helper_symbols_by_sdk")
        if isinstance(expected_helpers_by_sdk, dict) and isinstance(helper_sets, dict):
            for sdk, expected_helpers in expected_helpers_by_sdk.items():
                helpers = helper_sets.get(sdk)
                if not isinstance(helpers, list):
                    continue
                for helper in expected_helpers:
                    if helper not in helpers:
                        errors.append(
                            f"{label} lanes {lanes} "
                            f"sdk_helper_symbols_by_sdk[{sdk}] missing "
                            f"required helper: {helper}"
                        )
            helper_symbols = row.get("sdk_helper_symbols")
            expected_js_helpers = expected_helpers_by_sdk.get("js-sdk", ())
            if isinstance(helper_symbols, list):
                for helper in expected_js_helpers:
                    if helper not in helper_symbols:
                        errors.append(
                            f"{label} lanes {lanes} sdk_helper_symbols "
                            f"missing required helper: {helper}"
                        )
    for lanes in USER_PROVER_REQUIRED_LANE_BACKENDS:
        if lanes not in seen_lanes:
            errors.append(f"{label} missing required lanes row: {lanes}")
    return errors


def _exact_object_key_errors(
    label: str,
    payload: dict[str, Any],
    allowed_keys: set[str],
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(payload) - allowed_keys):
        errors.append(f"{label} contains unknown field: {key}")
    for key in sorted(allowed_keys - set(payload)):
        errors.append(f"{label} missing field: {key}")
    return errors


def _fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    if field not in payload:
        return []
    if not _is_canonical_fixed_hex_text(payload.get(field), byte_length=byte_length):
        return [f"{label} {field} must be a canonical {type_label} hex string"]
    return []


def _nonzero_fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    errors = _fixed_hex_field_errors(
        label,
        payload,
        field,
        byte_length=byte_length,
        type_label=type_label,
    )
    if errors or field not in payload:
        return errors
    value = payload.get(field)
    if isinstance(value, str) and all(char == "0" for char in value[2:]):
        return [
            f"{label} {field} must be a non-zero canonical {type_label} hex string"
        ]
    return []


def _empty_or_nonzero_fixed_hex_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    byte_length: int,
    type_label: str,
) -> list[str]:
    if field not in payload:
        return []
    if payload.get(field) == "":
        return []
    errors = _nonzero_fixed_hex_field_errors(
        label,
        payload,
        field,
        byte_length=byte_length,
        type_label=type_label,
    )
    if errors:
        return [
            f"{label} {field} must be empty or a non-zero canonical "
            f"{type_label} hex string"
        ]
    return []


def _source_adapter_gate_coherence_errors(
    label: str,
    lane: dict[str, Any],
    source_gate: dict[str, Any],
) -> list[str]:
    domain = lane.get("domain")
    required = source_gate.get("required")
    ready = source_gate.get("ready")
    gate_hash = source_gate.get("gate_hash")
    audit_hashes = source_gate.get("audit_hashes")
    blockers = source_gate.get("blockers")

    if type(required) is not bool:
        return []
    errors: list[str] = []
    expected_audit_keys = (
        ALL_LANES_SOURCE_ADAPTER_GATE_AUDIT_KEYS_BY_DOMAIN.get(domain)
        if type(domain) is int
        else None
    )
    if expected_audit_keys is None:
        if required:
            errors.append(f"{label} required must be false for this lane domain")
            return errors
        if ready is not True:
            errors.append(f"{label} ready must be true when gate is not required")
        if isinstance(audit_hashes, dict) and audit_hashes:
            errors.append(
                f"{label} audit_hashes must be empty when gate is not required"
            )
        if gate_hash not in (None, ""):
            errors.append(f"{label} gate_hash must be empty when gate is not required")
        if isinstance(blockers, list) and blockers:
            errors.append(f"{label} blockers must be empty when gate is not required")
        return errors

    if not required:
        errors.append(f"{label} required must be true for this lane domain")
        return errors

    expected_gate_key = (
        ALL_LANES_SOURCE_ADAPTER_GATE_HASH_KEY_BY_DOMAIN.get(domain)
        if type(domain) is int
        else None
    )
    if isinstance(audit_hashes, dict):
        for key in sorted(set(audit_hashes) - expected_audit_keys):
            errors.append(f"{label} audit_hashes contains unexpected field: {key}")
        for key in sorted(expected_audit_keys - set(audit_hashes)):
            errors.append(f"{label} audit_hashes missing field: {key}")
        expected_gate_hash = (
            audit_hashes.get(expected_gate_key)
            if expected_gate_key is not None
            else None
        )
        if (
            expected_gate_key is not None
            and _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
            and _is_canonical_fixed_hex_text(expected_gate_hash, byte_length=32)
            and gate_hash != expected_gate_hash
        ):
            errors.append(
                f"{label} gate_hash must match audit_hashes.{expected_gate_key}"
            )
        role_fields: list[tuple[str, Any]] = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            role_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            role_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            role_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
            route_canary = route_allowlist.get("route_canary")
            if isinstance(route_canary, dict):
                role_fields.append(
                    (
                        "route_canary_evidence_hash",
                        route_canary.get("evidence_hash"),
                    )
                )
        role_fields.extend(
            (f"audit_hashes.{field}", value)
            for field, value in sorted(audit_hashes.items())
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(role_fields),
                byte_length=32,
            )
        )

    if type(ready) is not bool:
        return errors
    if not isinstance(blockers, list):
        return errors
    if not ready:
        errors.append(f"{label} ready must be true when gate is required")
    if blockers:
        errors.append(f"{label} blockers must be empty when gate is required")
    return errors


def _distinct_nonzero_hex_field_errors(
    label: str,
    fields: tuple[tuple[str, Any], ...],
    *,
    byte_length: int,
) -> list[str]:
    errors: list[str] = []
    seen: dict[str, str] = {}
    for field, value in fields:
        if (
            not _is_canonical_fixed_hex_text(value, byte_length=byte_length)
            or not isinstance(value, str)
            or all(char == "0" for char in value[2:])
        ):
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            errors.append(f"{label} {field} must not reuse {previous_field}")
            continue
        seen[value] = field
    return errors


def _decimal_text_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    *,
    positive: bool,
) -> list[str]:
    if field not in payload:
        return []
    if not _is_canonical_decimal_text(payload.get(field), positive=positive):
        qualifier = "positive " if positive else ""
        return [f"{label} {field} must be a canonical {qualifier}decimal string"]
    return []


def _tron_address_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not _is_canonical_tron_address_text(value):
        return [
            f"{label} {field} must be a non-zero canonical 0x41-prefixed "
            "21-byte hex string"
        ]
    return []


def _solana_pubkey_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
) -> list[str]:
    if field not in payload:
        return []
    value = payload.get(field)
    if not isinstance(value, str):
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    try:
        raw = _decode_solana_base58(value)
    except ValueError:
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    if len(raw) != 32 or not any(raw):
        return [f"{label} {field} must be a non-zero canonical base58 Solana address"]
    return []


def _decode_solana_base58(value: str) -> bytes:
    if value != value.strip() or not value:
        raise ValueError("not canonical base58")
    numeric = 0
    for symbol in value:
        digit = SOLANA_BASE58_INDEX.get(symbol)
        if digit is None:
            raise ValueError("not canonical base58")
        numeric = numeric * 58 + digit
    leading_zeros = len(value) - len(value.lstrip("1"))
    payload = (
        b""
        if numeric == 0
        else numeric.to_bytes((numeric.bit_length() + 7) // 8, "big")
    )
    return (b"\x00" * leading_zeros) + payload


def _is_canonical_tron_address_text(value: Any) -> bool:
    return (
        isinstance(value, str)
        and _is_canonical_fixed_hex_text(value, byte_length=21)
        and value.startswith("0x41")
        and any(byte != "0" for byte in value[4:])
    )


def _matching_text_field_errors(
    label: str,
    payload: dict[str, Any],
    field: str,
    expected: Any,
    expected_field: str,
) -> list[str]:
    if field not in payload or not isinstance(payload.get(field), str):
        return []
    if not isinstance(expected, str):
        return []
    if payload.get(field) != expected:
        return [f"{label} {field} must match {expected_field}"]
    return []


def _canonical_nonzero_fixed_hex_value(value: Any, *, byte_length: int) -> str | None:
    if not _is_canonical_fixed_hex_text(value, byte_length=byte_length):
        return None
    assert isinstance(value, str)
    if all(char == "0" for char in value[2:]):
        return None
    return value


def _route_canary_common_hash_role_errors(
    label: str,
    lane: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    source_hashes = lane.get("source_record_hashes")
    route_allowlist = lane.get("route_allowlist")
    destination_binding = lane.get("destination_binding")
    fields: list[tuple[str, Any]] = []
    if isinstance(source_hashes, dict):
        fields.extend(
            (
                (field, source_hashes.get(field))
                for field in (
                    "source_verifier_material_hash",
                    "source_adapter_engine_deployment_hash",
                )
            )
        )
    if isinstance(route_allowlist, dict):
        fields.append(
            ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
        )
    if isinstance(destination_binding, dict):
        fields.append(
            (
                "destination_binding_hash",
                destination_binding.get("destination_binding_hash"),
            )
        )
    fields.append(("evidence_hash", route_canary.get("evidence_hash")))
    return _distinct_nonzero_hex_field_errors(
        f"{label} hash role",
        tuple(fields),
        byte_length=32,
    )


def _route_allowlist_recompute_errors(
    label: str,
    lane: dict[str, Any],
    route_allowlist: dict[str, Any],
) -> list[str]:
    source_hashes = lane.get("source_record_hashes")
    destination_binding = lane.get("destination_binding")
    if not isinstance(source_hashes, dict) or not isinstance(destination_binding, dict):
        return []

    fields = (
        (
            "source_verifier_material_hash",
            source_hashes.get("source_verifier_material_hash"),
        ),
        (
            "source_adapter_engine_deployment_hash",
            source_hashes.get("source_adapter_engine_deployment_hash"),
        ),
        (
            "destination_binding_hash",
            destination_binding.get("destination_binding_hash"),
        ),
        ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash")),
    )
    errors = _distinct_nonzero_hex_field_errors(
        f"{label} governed hash role",
        fields,
        byte_length=32,
    )

    source_verifier_material_hash = _canonical_fixed_hex_bytes(
        source_hashes.get("source_verifier_material_hash"),
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _canonical_fixed_hex_bytes(
        source_hashes.get("source_adapter_engine_deployment_hash"),
        byte_length=32,
    )
    destination_binding_hash = _canonical_fixed_hex_bytes(
        destination_binding.get("destination_binding_hash"),
        byte_length=32,
    )
    route_allowlist_hash = _canonical_fixed_hex_bytes(
        route_allowlist.get("route_allowlist_hash"),
        byte_length=32,
    )
    if (
        source_verifier_material_hash is None
        or source_adapter_engine_deployment_hash is None
        or destination_binding_hash is None
        or route_allowlist_hash is None
    ):
        return errors
    expected = _canonical_route_allowlist_hash(
        domain=lane.get("domain"),
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if expected is not None and expected != route_allowlist.get("route_allowlist_hash"):
        errors.append(
            f"{label} route_allowlist_hash must recompute from source material, "
            "source adapter deployment, and destination binding hashes"
        )
    return errors


def _all_lanes_lane_label(label: str, index: int, lane: dict[str, Any]) -> str:
    domain = lane.get("domain")
    if type(domain) is int:
        return f"{label} lane domain {domain}"
    if isinstance(lane.get("chain"), str) and lane.get("chain"):
        return f"{label} lane {lane['chain']}"
    return f"{label} lane {index}"


def _all_lanes_route_canary_cross_lane_errors(
    label: str,
    lanes: Any,
) -> list[str]:
    if not isinstance(lanes, list):
        return []

    errors: list[str] = []
    governed_hashes: dict[str, tuple[str, str]] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            for field in (
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
            ):
                value = _canonical_nonzero_fixed_hex_value(
                    source_hashes.get(field),
                    byte_length=32,
                )
                if value is not None:
                    governed_hashes.setdefault(value, (lane_label, field))
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            value = _canonical_nonzero_fixed_hex_value(
                destination_binding.get("destination_binding_hash"),
                byte_length=32,
            )
            if value is not None:
                governed_hashes.setdefault(
                    value,
                    (lane_label, "destination_binding_hash"),
                )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            value = _canonical_nonzero_fixed_hex_value(
                route_allowlist.get("route_allowlist_hash"),
                byte_length=32,
            )
            if value is not None:
                governed_hashes.setdefault(value, (lane_label, "route_allowlist_hash"))

    seen_canaries: dict[str, str] = {}
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        route_allowlist = lane.get("route_allowlist")
        if not isinstance(route_allowlist, dict):
            continue
        route_canary = route_allowlist.get("route_canary")
        if not isinstance(route_canary, dict):
            continue
        evidence_hash = _canonical_nonzero_fixed_hex_value(
            route_canary.get("evidence_hash"),
            byte_length=32,
        )
        if evidence_hash is None:
            continue
        canary_label = f"{lane_label} route_allowlist route_canary"
        previous_canary_label = seen_canaries.get(evidence_hash)
        if previous_canary_label is not None:
            errors.append(
                f"{canary_label} evidence_hash must be distinct from "
                f"{previous_canary_label} route_canary evidence_hash"
            )
        else:
            seen_canaries[evidence_hash] = f"{lane_label} route_allowlist"
        governed = governed_hashes.get(evidence_hash)
        if governed is None:
            continue
        governed_lane_label, governed_field = governed
        if governed_lane_label == lane_label:
            continue
        errors.append(
            f"{canary_label} evidence_hash must not reuse {governed_field} "
            f"from {governed_lane_label}"
        )
    return errors


def _all_lanes_route_canary_schema_errors(
    label: str,
    lane: dict[str, Any],
    route_canary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    domain = lane.get("domain")
    expected_keys = ALL_LANES_ROUTE_CANARY_KEYS_BY_DOMAIN.get(
        domain,
        ALL_LANES_ROUTE_CANARY_COMMON_KEYS,
    )
    errors.extend(_exact_object_key_errors(label, route_canary, expected_keys))
    for field in (
        "evidence_hash",
        "route_allowlist_hash",
        "destination_binding_hash",
    ):
        errors.extend(
            _nonzero_fixed_hex_field_errors(
                label,
                route_canary,
                field,
                byte_length=32,
                type_label="bytes32",
            )
        )
    for field in ("status", "evidence_source"):
        errors.extend(_non_empty_string_field_errors(label, route_canary, field))
    if isinstance(route_canary.get("status"), str) and (
        route_canary.get("status") != "passed"
    ):
        errors.append(f"{label} status must be passed")
    expected_source = ALL_LANES_ROUTE_CANARY_SOURCE_BY_DOMAIN.get(domain)
    if (
        expected_source is not None
        and isinstance(route_canary.get("evidence_source"), str)
        and route_canary.get("evidence_source") != expected_source
    ):
        errors.append(f"{label} evidence_source must be {expected_source}")
    errors.extend(_true_field_errors(label, route_canary, "evidence_bound"))
    route_allowlist = lane.get("route_allowlist")
    if isinstance(route_allowlist, dict):
        expected_route_hash = route_allowlist.get("route_allowlist_hash")
        if (
            isinstance(expected_route_hash, str)
            and isinstance(route_canary.get("route_allowlist_hash"), str)
            and route_canary.get("route_allowlist_hash") != expected_route_hash
        ):
            errors.append(
                f"{label} route_allowlist_hash must match lane "
                "route_allowlist_hash"
            )
    destination_binding = lane.get("destination_binding")
    if isinstance(destination_binding, dict):
        expected_destination_hash = destination_binding.get("destination_binding_hash")
        if (
            isinstance(expected_destination_hash, str)
            and isinstance(route_canary.get("destination_binding_hash"), str)
            and route_canary.get("destination_binding_hash") != expected_destination_hash
        ):
            errors.append(
                f"{label} destination_binding_hash must match lane "
                "destination_binding_hash"
            )
    errors.extend(_route_canary_common_hash_role_errors(label, lane, route_canary))

    if domain in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC):
        for field in (
            "transaction_hash",
            "receipt_block_hash",
            "block_receipts_root",
            "call_data_sha256",
            "message_id",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
        ):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        evm_transcript_hash_fields = (
            "transaction_hash",
            "receipt_block_hash",
            "block_receipts_root",
            "call_data_sha256",
            "message_id",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "evidence_hash",
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} transcript hash",
                tuple(
                    (field, route_canary.get(field))
                    for field in evm_transcript_hash_fields
                ),
                byte_length=32,
            )
        )
        governed_hash_fields: list[tuple[str, Any]] = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in evm_transcript_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
        errors.extend(_u32_field_errors(label, route_canary, "log_index"))
        errors.extend(
            _integer_field_errors(
                label,
                route_canary,
                "receipt_block_number",
                positive=True,
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "target_domain",
                domain,
                "the lane domain",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_version",
                1,
                "1",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_source_domain",
                SCCP_DOMAIN_SORA,
                "SORA",
            )
        )
        errors.extend(_true_field_errors(label, route_canary, "message_proof_used"))
    elif domain == SCCP_DOMAIN_TRON:
        for field in (
            "transaction_id",
            "message_id",
            "call_data_sha256",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "signature_sha256",
        ):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        for field in ("transaction_owner_address", "signature_recovered_address"):
            errors.extend(_tron_address_field_errors(label, route_canary, field))
        transaction_owner_address = route_canary.get("transaction_owner_address")
        signature_recovered_address = route_canary.get("signature_recovered_address")
        if (
            _is_canonical_tron_address_text(transaction_owner_address)
            and _is_canonical_tron_address_text(signature_recovered_address)
            and signature_recovered_address != transaction_owner_address
        ):
            errors.append(
                f"{label} signature_recovered_address must match "
                "transaction_owner_address"
            )
        tron_transcript_hash_fields = (
            "transaction_id",
            "message_id",
            "call_data_sha256",
            "payload_hash",
            "statement_hash",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "signature_sha256",
            "evidence_hash",
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} transcript hash",
                tuple(
                    (field, route_canary.get(field))
                    for field in tron_transcript_hash_fields
                ),
                byte_length=32,
            )
        )
        governed_hash_fields: list[tuple[str, Any]] = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in tron_transcript_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
        errors.extend(
            _integer_field_errors(
                label,
                route_canary,
                "block_number",
                positive=True,
            )
        )
        errors.extend(
            _integer_field_errors(
                label,
                route_canary,
                "block_timestamp",
                positive=False,
            )
        )
        errors.extend(_u32_field_errors(label, route_canary, "log_index"))
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "target_domain",
                SCCP_DOMAIN_TRON,
                "TRON",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_version",
                1,
                "1",
            )
        )
        errors.extend(
            _expected_u32_field_errors(
                label,
                route_canary,
                "proof_source_domain",
                SCCP_DOMAIN_SORA,
                "SORA",
            )
        )
        for field in (
            "message_proof_used",
            "raw_data_owner_matches_transaction",
            "signature_recovers_to_owner",
        ):
            errors.extend(_true_field_errors(label, route_canary, field))
    elif domain == SCCP_DOMAIN_SOL:
        errors.extend(
            _solana_pubkey_field_errors(
                label,
                route_canary,
                "solana_programdata_address",
            )
        )
        errors.extend(
            _decimal_text_field_errors(
                label,
                route_canary,
                "solana_programdata_slot",
                positive=True,
            )
        )
    elif domain == SCCP_DOMAIN_TON:
        for field in ("ton_account_state_hash", "ton_last_transaction_hash"):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        errors.extend(
            _decimal_text_field_errors(
                label,
                route_canary,
                "ton_last_transaction_lt",
                positive=True,
            )
        )
        ton_hash_fields = (
            "ton_account_state_hash",
            "ton_last_transaction_hash",
            "evidence_hash",
        )
        governed_hash_fields = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in ton_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
    elif domain in (
        SCCP_DOMAIN_SORA_KUSAMA,
        SCCP_DOMAIN_SORA_POLKADOT,
        SCCP_DOMAIN_SORA2,
    ):
        for field in ("substrate_finalized_head", "substrate_runtime_code_hash"):
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    label,
                    route_canary,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        for field in (
            "substrate_runtime_spec_version",
            "substrate_runtime_transaction_version",
        ):
            errors.extend(
                _decimal_text_field_errors(
                    label,
                    route_canary,
                    field,
                    positive=False,
                )
            )
        substrate_hash_fields = (
            "substrate_finalized_head",
            "substrate_runtime_code_hash",
            "evidence_hash",
        )
        governed_hash_fields = []
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            governed_hash_fields.extend(
                (
                    (field, source_hashes.get(field))
                    for field in (
                        "source_verifier_material_hash",
                        "source_adapter_engine_deployment_hash",
                    )
                )
            )
        if isinstance(route_allowlist, dict):
            governed_hash_fields.append(
                ("route_allowlist_hash", route_allowlist.get("route_allowlist_hash"))
            )
        if isinstance(destination_binding, dict):
            governed_hash_fields.append(
                (
                    "destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                )
            )
        governed_hash_fields.extend(
            (field, route_canary.get(field)) for field in substrate_hash_fields
        )
        errors.extend(
            _distinct_nonzero_hex_field_errors(
                f"{label} hash role",
                tuple(governed_hash_fields),
                byte_length=32,
            )
        )
    return errors


def _all_lanes_lane_schema_errors(label: str, lanes: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(lanes, list):
        return errors
    for index, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            errors.append(f"{label} lane {index} is not an object")
            continue
        lane_label = _all_lanes_lane_label(label, index, lane)
        for key in sorted(set(lane) - ALL_LANES_LANE_KEYS):
            errors.append(f"{lane_label} contains unknown field: {key}")
        for key in sorted(ALL_LANES_LANE_KEYS - set(lane)):
            errors.append(f"{lane_label} missing field: {key}")
        if "domain" in lane and type(lane.get("domain")) is not int:
            errors.append(f"{lane_label} domain must be an integer")
        domain = lane.get("domain")
        expected_chain = (
            ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
            if type(domain) is int
            else None
        )
        if type(domain) is int and expected_chain is None:
            errors.append(f"{lane_label} domain must be a production remote domain")
        if "chain" in lane and (
            not isinstance(lane.get("chain"), str) or not lane.get("chain")
        ):
            errors.append(f"{lane_label} chain must be a non-empty string")
        elif expected_chain is not None and lane.get("chain") != expected_chain:
            errors.append(f"{lane_label} chain must be {expected_chain}")
        if domain == ACTIVE_LAUNCH_DOMAIN:
            errors.extend(_true_field_errors(lane_label, lane, "production_ready"))
        else:
            errors.extend(_boolean_field_errors(lane_label, lane, "production_ready"))
        for field in (
            "records",
            "source_record_hashes",
            "source_adapter_gate",
            "destination_binding",
            "route_allowlist",
        ):
            if field in lane and not isinstance(lane.get(field), dict):
                errors.append(f"{lane_label} {field} is not an object")
        errors.extend(
            _string_list_field_errors(lane_label, lane, "blockers", allow_empty=True)
        )
        blockers = lane.get("blockers")
        if domain == ACTIVE_LAUNCH_DOMAIN and isinstance(blockers, list) and blockers:
            errors.append(f"{lane_label} blockers must be empty")
        records = lane.get("records")
        if isinstance(records, dict):
            records_label = f"{lane_label} records"
            errors.extend(
                _exact_object_key_errors(
                    records_label,
                    records,
                    ALL_LANES_RECORD_KEYS,
                )
            )
            for field in ALL_LANES_RECORD_KEYS:
                if field in records:
                    if domain == ACTIVE_LAUNCH_DOMAIN or lane.get("production_ready") is True:
                        errors.extend(_true_field_errors(records_label, records, field))
                    else:
                        errors.extend(_boolean_field_errors(records_label, records, field))
        if domain != ACTIVE_LAUNCH_DOMAIN and lane.get("production_ready") is False:
            continue
        source_hashes = lane.get("source_record_hashes")
        if isinstance(source_hashes, dict):
            source_hashes_label = f"{lane_label} source_record_hashes"
            errors.extend(
                _exact_object_key_errors(
                    source_hashes_label,
                    source_hashes,
                    ALL_LANES_SOURCE_RECORD_HASH_KEYS,
                )
            )
            for field in ALL_LANES_SOURCE_RECORD_HASH_KEYS:
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        source_hashes_label,
                        source_hashes,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
        source_gate = lane.get("source_adapter_gate")
        if isinstance(source_gate, dict):
            source_gate_label = f"{lane_label} source_adapter_gate"
            errors.extend(
                _exact_object_key_errors(
                    source_gate_label,
                    source_gate,
                    ALL_LANES_SOURCE_ADAPTER_GATE_KEYS,
                )
            )
            errors.extend(
                _boolean_field_errors(source_gate_label, source_gate, "required")
            )
            errors.extend(
                _boolean_field_errors(source_gate_label, source_gate, "ready")
            )
            errors.extend(
                _empty_or_nonzero_fixed_hex_field_errors(
                    source_gate_label,
                    source_gate,
                    "gate_hash",
                    byte_length=32,
                    type_label="bytes32",
                )
            )
            gate_hash = source_gate.get("gate_hash")
            audit_hashes = source_gate.get("audit_hashes")
            if "audit_hashes" in source_gate and not isinstance(
                audit_hashes,
                dict,
            ):
                errors.append(f"{source_gate_label} audit_hashes is not an object")
            elif isinstance(audit_hashes, dict):
                for field, value in sorted(audit_hashes.items()):
                    if not isinstance(field, str) or not field:
                        errors.append(
                            f"{source_gate_label} audit_hashes contains an empty key"
                        )
                    elif not _is_canonical_fixed_hex_text(value, byte_length=32) or (
                        isinstance(value, str) and all(char == "0" for char in value[2:])
                    ):
                        errors.append(
                            f"{source_gate_label} audit_hashes {field} must be a "
                            "non-zero canonical bytes32 hex string"
                        )
            if source_gate.get("required") is True:
                if (
                    not _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
                    or (
                        isinstance(gate_hash, str)
                        and all(char == "0" for char in gate_hash[2:])
                    )
                ):
                    errors.append(
                        f"{source_gate_label} gate_hash must be a non-zero "
                        "canonical bytes32 hex string when required"
                    )
                if isinstance(audit_hashes, dict):
                    if not audit_hashes:
                        errors.append(
                            f"{source_gate_label} audit_hashes must not be empty "
                            "when required"
                        )
                    elif (
                        _is_canonical_fixed_hex_text(gate_hash, byte_length=32)
                        and isinstance(gate_hash, str)
                        and any(char != "0" for char in gate_hash[2:])
                        and not any(
                            gate_hash == value for value in audit_hashes.values()
                        )
                    ):
                        errors.append(
                            f"{source_gate_label} gate_hash must match one "
                            "audit_hashes value"
                        )
            errors.extend(
                _string_list_field_errors(
                    source_gate_label,
                    source_gate,
                    "blockers",
                    allow_empty=True,
                )
            )
            blockers = source_gate.get("blockers")
            if (
                source_gate.get("ready") is True
                and isinstance(blockers, list)
                and blockers
            ):
                errors.append(f"{source_gate_label} blockers must be empty when ready")
            errors.extend(
                _source_adapter_gate_coherence_errors(
                    source_gate_label,
                    lane,
                    source_gate,
                )
            )
        destination_binding = lane.get("destination_binding")
        if isinstance(destination_binding, dict):
            destination_label = f"{lane_label} destination_binding"
            for key in sorted(
                set(destination_binding) - ALL_LANES_DESTINATION_BINDING_KEYS
            ):
                errors.append(f"{destination_label} contains unknown field: {key}")
            for key in sorted(
                ALL_LANES_DESTINATION_BINDING_REQUIRED_KEYS
                - set(destination_binding)
            ):
                errors.append(f"{destination_label} missing field: {key}")
            errors.extend(
                _non_empty_string_field_errors(
                    destination_label,
                    destination_binding,
                    "destination_binding_key",
                )
            )
            for field in (
                "destination_binding_hash",
                "expected_destination_binding_hash",
                "destination_network_id",
            ):
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        destination_label,
                        destination_binding,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    destination_label,
                    destination_binding,
                    "destination_bridge_address",
                    byte_length=20,
                    type_label="20-byte",
                )
            )
            if domain in ALL_LANES_EVM_DESTINATION_DOMAINS:
                for field in ("destination_network_id", "destination_bridge_address"):
                    if field not in destination_binding:
                        errors.append(
                            f"{destination_label} {field} is required for "
                            "EVM-family lanes"
                        )
            elif domain == SCCP_DOMAIN_TRON:
                if "destination_network_id" not in destination_binding:
                    errors.append(
                        f"{destination_label} destination_network_id is required "
                        "for TRON lanes"
                    )
                if "destination_bridge_address" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_bridge_address is only "
                        "valid for EVM-family lanes"
                    )
            elif domain in ALL_LANES_STATIC_DESTINATION_DOMAINS:
                if "destination_network_id" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_network_id is only valid "
                        "for EVM-family or TRON lanes"
                    )
                if "destination_bridge_address" in destination_binding:
                    errors.append(
                        f"{destination_label} destination_bridge_address is only "
                        "valid for EVM-family lanes"
                    )
            errors.extend(
                _matching_text_field_errors(
                    destination_label,
                    destination_binding,
                    "expected_destination_binding_hash",
                    destination_binding.get("destination_binding_hash"),
                    "destination_binding_hash",
                )
            )
            errors.extend(
                _true_field_errors(
                    destination_label,
                    destination_binding,
                    "expected_destination_binding_hash_matches",
                )
            )
            errors.extend(
                _true_field_errors(
                    destination_label,
                    destination_binding,
                    "recomputed",
                )
            )
        route_allowlist = lane.get("route_allowlist")
        if isinstance(route_allowlist, dict):
            route_label = f"{lane_label} route_allowlist"
            errors.extend(
                _exact_object_key_errors(
                    route_label,
                    route_allowlist,
                    ALL_LANES_ROUTE_ALLOWLIST_KEYS,
                )
            )
            for field in ("route_allowlist_hash", "expected_route_allowlist_hash"):
                errors.extend(
                    _nonzero_fixed_hex_field_errors(
                        route_label,
                        route_allowlist,
                        field,
                        byte_length=32,
                        type_label="bytes32",
                    )
                )
            errors.extend(
                _matching_text_field_errors(
                    route_label,
                    route_allowlist,
                    "expected_route_allowlist_hash",
                    route_allowlist.get("route_allowlist_hash"),
                    "route_allowlist_hash",
                )
            )
            errors.extend(
                _true_field_errors(
                    route_label,
                    route_allowlist,
                    "expected_route_allowlist_hash_matches",
                )
            )
            errors.extend(
                _route_allowlist_recompute_errors(
                    route_label,
                    lane,
                    route_allowlist,
                )
            )
            route_canary = route_allowlist.get("route_canary")
            if not isinstance(route_canary, dict):
                errors.append(f"{route_label} route_canary is not an object")
            else:
                canary_label = f"{route_label} route_canary"
                errors.extend(
                    _all_lanes_route_canary_schema_errors(
                        canary_label,
                        lane,
                        route_canary,
                    )
                )
                errors.extend(
                    _matching_text_field_errors(
                        canary_label,
                        route_canary,
                        "route_allowlist_hash",
                        route_allowlist.get("route_allowlist_hash"),
                        "lane route_allowlist_hash",
                    )
                )
                if isinstance(destination_binding, dict):
                    errors.extend(
                        _matching_text_field_errors(
                            canary_label,
                            route_canary,
                            "destination_binding_hash",
                            destination_binding.get("destination_binding_hash"),
                            "lane destination_binding_hash",
                        )
                    )
    return errors


def _all_lanes_summary_schema_errors(
    label: str,
    summary: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(summary) - ALL_LANES_SUMMARY_KEYS):
        errors.append(f"{label} contains unknown field: {key}")
    for key in sorted(ALL_LANES_SUMMARY_KEYS - set(summary)):
        errors.append(f"{label} missing field: {key}")
    errors.extend(_boolean_field_errors(label, summary, "production_ready"))
    errors.extend(
        _integer_list_field_errors(
            label,
            summary,
            "required_domains",
            allow_empty=False,
        )
    )
    errors.extend(_list_field_errors(label, summary, "lanes"))
    errors.extend(_string_list_field_errors(label, summary, "blockers", allow_empty=True))
    blockers = summary.get("blockers")
    if isinstance(blockers, list):
        launch_blockers = _active_launch_blockers(summary)
        if launch_blockers:
            errors.append(f"{label} active {ACTIVE_LAUNCH_DISPLAY} launch blockers must be empty")
    errors.extend(_all_lanes_lane_schema_errors(label, summary.get("lanes")))
    errors.extend(_all_lanes_route_canary_cross_lane_errors(label, summary.get("lanes")))
    required_domains = summary.get("required_domains")
    lanes = summary.get("lanes")
    if (
        isinstance(required_domains, list)
        and all(type(domain) is int for domain in required_domains)
        and isinstance(lanes, list)
        and all(
            isinstance(lane, dict) and type(lane.get("domain")) is int
            for lane in lanes
        )
    ):
        lane_domains = [lane["domain"] for lane in lanes]
        if len(set(required_domains)) != len(required_domains):
            errors.append(f"{label} required_domains contains duplicate domains")
        if len(set(lane_domains)) != len(lane_domains):
            errors.append(f"{label} lanes contain duplicate domains")
        expected_domains = list(ALL_LANES_REQUIRED_DOMAINS)
        if required_domains != expected_domains:
            errors.append(
                f"{label} required_domains must be the production remote domains"
            )
        if lane_domains != expected_domains:
            errors.append(f"{label} lane domains must be the production remote domains")
        if required_domains != lane_domains:
            errors.append(f"{label} required_domains must match lane domains")
    if "release_checklist" in summary and not isinstance(
        summary.get("release_checklist"),
        dict,
    ):
        errors.append(f"{label} release_checklist is not an object")
    return errors


def _release_checklist_schema_errors(
    label: str,
    checklist: dict[str, Any],
    *,
    require_ready: bool = True,
) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(checklist) - RELEASE_CHECKLIST_KEYS):
        errors.append(f"{label} release_checklist contains unknown field: {key}")
    for key in sorted(RELEASE_CHECKLIST_KEYS - set(checklist)):
        errors.append(f"{label} release_checklist missing field: {key}")
    if require_ready:
        errors.extend(_true_field_errors(f"{label} release_checklist", checklist, "ready"))
    else:
        errors.extend(_boolean_field_errors(f"{label} release_checklist", checklist, "ready"))
    items = checklist.get("items")
    if not isinstance(items, list):
        errors.append(f"{label} release_checklist items is not a list")
        return errors
    for item in items:
        if not isinstance(item, dict):
            errors.append(f"{label} release_checklist item is not an object")
            continue
        item_id = item.get("id")
        item_label = (
            f"{label} release_checklist item {item_id}"
            if isinstance(item_id, str) and item_id
            else f"{label} release_checklist item"
        )
        for key in sorted(set(item) - RELEASE_CHECKLIST_ITEM_KEYS):
            errors.append(f"{item_label} contains unknown field: {key}")
        for key in sorted(RELEASE_CHECKLIST_ITEM_KEYS - set(item)):
            errors.append(f"{item_label} missing field: {key}")
        errors.extend(_non_empty_string_field_errors(item_label, item, "id"))
        errors.extend(_non_empty_string_field_errors(item_label, item, "title"))
        if require_ready:
            errors.extend(_true_field_errors(item_label, item, "ready"))
        else:
            errors.extend(_boolean_field_errors(item_label, item, "ready"))
        errors.extend(
            _string_list_field_errors(item_label, item, "blockers", allow_empty=True)
        )
        blockers = item.get("blockers")
        if require_ready and isinstance(blockers, list) and blockers:
            errors.append(f"{item_label} blockers must be empty")
    return errors


def _corridor_schema_errors(corridor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for key in sorted(set(corridor) - CORRIDOR_KEYS):
        errors.append(f"readiness report corridor contains unknown field: {key}")
    for key in sorted(CORRIDOR_KEYS - set(corridor)):
        errors.append(f"readiness report corridor missing field: {key}")
    if type(corridor.get("production_ready")) is not bool:
        errors.append("readiness report corridor production_ready is not a boolean")
    if type(corridor.get("require_phase_evidence")) is not bool:
        errors.append("readiness report corridor require_phase_evidence is not a boolean")
    if not isinstance(corridor.get("phases"), dict):
        errors.append("readiness report corridor phases is not an object")
    if not isinstance(corridor.get("evidence_artifacts"), dict):
        errors.append("readiness report corridor evidence_artifacts is not an object")
    errors.extend(
        _string_list_field_errors(
            "readiness report corridor",
            corridor,
            "blockers",
            allow_empty=True,
        )
    )
    blockers = corridor.get("blockers")
    if isinstance(blockers, list) and blockers:
        errors.append("readiness report corridor blockers must be empty")
    return errors


def _expected_input_paths(report: dict[str, Any]) -> list[str]:
    paths: list[str] = []
    input_artifacts = report.get("input_artifacts")
    if not isinstance(input_artifacts, list):
        return paths
    for artifact in input_artifacts:
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.append(artifact_path)
    return paths


def _input_provenance_schema_errors(report: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    inputs = report.get("inputs")
    if not isinstance(inputs, list) or not inputs:
        errors.append(
            "readiness report inputs must be a non-empty list of canonical paths"
        )
    else:
        seen_inputs: set[str] = set()
        for index, item in enumerate(inputs):
            item_errors = _canonical_report_input_path_errors(item)
            if item_errors:
                errors.extend(item_errors)
            errors.extend(
                _copied_input_layout_errors("readiness report inputs", index, item)
            )
            if isinstance(item, str) and item in seen_inputs:
                errors.append(
                    f"readiness report inputs contains duplicate path: {item}"
                )
            if isinstance(item, str):
                seen_inputs.add(item)

    input_artifacts = report.get("input_artifacts")
    if isinstance(input_artifacts, list):
        seen_artifacts: set[str] = set()
        for index, artifact in enumerate(input_artifacts):
            if not isinstance(artifact, dict):
                continue
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if path_errors or artifact_path is None:
                continue
            errors.extend(
                _copied_input_layout_errors(
                    "readiness report input_artifacts",
                    index,
                    artifact_path,
                )
            )
            if artifact_path in seen_artifacts:
                errors.append(
                    "readiness report input_artifacts contains duplicate path: "
                    f"{artifact_path}"
                )
            seen_artifacts.add(artifact_path)
    return errors


def _bundle_artifact_path(bundle_dir: Path, artifact: dict[str, Any]) -> Path | None:
    artifact_path, path_errors = _canonical_artifact_path(artifact)
    if path_errors or artifact_path is None:
        return None
    return bundle_dir.joinpath(*PurePosixPath(artifact_path).parts)


def _copied_input_summary(
    bundle_dir: Path,
    report: dict[str, Any],
    errors: list[str],
) -> dict[str, Any] | None:
    input_paths: list[Path] = []
    input_artifacts = report.get("input_artifacts")
    if not isinstance(input_artifacts, list) or not input_artifacts:
        errors.append("readiness report input_artifacts must be a non-empty list")
        return None
    for index, artifact in enumerate(input_artifacts):
        if not isinstance(artifact, dict):
            errors.append(f"readiness report input artifact {index} is not an object")
            return None
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if path_errors or artifact_path is None:
            errors.extend(
                f"readiness report input artifact {index}: {error}"
                for error in path_errors
            )
            return None
        path = _bundle_artifact_path(bundle_dir, artifact)
        if path is not None:
            input_paths.append(path)
    if not input_paths:
        errors.append("readiness report has no usable copied evidence inputs")
        return None
    module = _all_lanes_module()
    records = module.load_evidence_bundle(input_paths)
    return module.validate_evidence_bundle(records)


def _referenced_report_artifact_paths(report: dict[str, Any]) -> set[str]:
    paths = set(REQUIRED_ARTIFACT_PATHS)
    input_artifacts = report.get("input_artifacts")
    if isinstance(input_artifacts, list):
        for artifact in input_artifacts:
            if not isinstance(artifact, dict):
                continue
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if not path_errors and artifact_path is not None:
                paths.add(artifact_path)

    corridor = report.get("corridor")
    if not isinstance(corridor, dict):
        return paths
    phase_artifacts = corridor.get("evidence_artifacts")
    if not isinstance(phase_artifacts, dict):
        return paths
    phases = corridor.get("phases")
    if not isinstance(phases, dict):
        return paths
    for phase, status in phases.items():
        if status != "passed":
            continue
        artifact = phase_artifacts.get(phase)
        if not isinstance(artifact, dict):
            continue
        artifact_path, path_errors = _canonical_artifact_path(artifact)
        if not path_errors and artifact_path is not None:
            paths.add(artifact_path)
    return paths


def verify_bundle(bundle_dir: Path) -> dict[str, Any]:
    """Return a verification summary for an SCCP release bundle."""

    errors: list[str] = []
    if bundle_dir.is_symlink():
        return {
            "verified": False,
            "errors": [f"bundle root is a symlink: {bundle_dir}"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    if not bundle_dir.is_dir():
        return {
            "verified": False,
            "errors": [f"bundle root is not a directory: {bundle_dir}"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    manifest_path = bundle_dir / "manifest.json"
    manifest_sha256: str | None = None
    if manifest_path.is_symlink():
        return {
            "verified": False,
            "errors": ["manifest is a symlink: manifest.json"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    if not manifest_path.is_file():
        return {
            "verified": False,
            "errors": [f"missing manifest: {manifest_path}"],
            "artifacts": [],
            "manifest_sha256": None,
        }
    try:
        manifest_sha256 = _sha256(manifest_path)
    except OSError:
        manifest_sha256 = None
    try:
        manifest = _load_json(manifest_path)
    except UnicodeDecodeError as exc:
        return {
            "verified": False,
            "errors": [f"manifest JSON is not UTF-8 text: {exc}"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    except json.JSONDecodeError as exc:
        return {
            "verified": False,
            "errors": [f"manifest is not valid JSON: {exc}"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    except DuplicateJsonKeyError as exc:
        return {
            "verified": False,
            "errors": [f"manifest JSON contains duplicate key: {exc.key}"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    if not isinstance(manifest, dict):
        return {
            "verified": False,
            "errors": ["manifest is not a JSON object"],
            "artifacts": [],
            "manifest_sha256": manifest_sha256,
        }
    errors.extend(_canonical_json_file_errors("manifest", manifest_path, manifest))

    for key in sorted(set(manifest) - MANIFEST_KEYS):
        errors.append(f"manifest contains unknown top-level field: {key}")
    for key in sorted(MANIFEST_KEYS - set(manifest)):
        errors.append(f"manifest missing top-level field: {key}")
    if manifest.get("schema") != SCHEMA:
        errors.append(f"unexpected manifest schema: {manifest.get('schema')}")
    errors.extend(_boolean_field_errors("manifest", manifest, "production_ready"))
    errors.extend(
        _boolean_field_errors("manifest", manifest, "release_checklist_ready")
    )
    errors.extend(_boolean_field_errors("manifest", manifest, "corridor_ready"))
    errors.extend(_string_list_field_errors("manifest", manifest, "blockers", allow_empty=True))
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append("manifest artifacts must be a non-empty list")
        artifacts = []
    manifest_artifacts = _manifest_artifacts_by_path(artifacts, errors)
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            errors.append("manifest artifact entry is not an object")
            continue
        errors.extend(_artifact_errors(bundle_dir, artifact))
    expected_paths = set(manifest_artifacts) | {MANIFEST_ROOT_PATH}
    bundle_paths, bundle_directories = _bundle_entry_paths(bundle_dir, errors)
    for unexpected in sorted(bundle_paths - expected_paths):
        errors.append(f"bundle contains unmanifested artifact: {unexpected}")
    for missing in sorted(expected_paths - bundle_paths):
        errors.append(f"bundle is missing expected artifact file: {missing}")
    expected_directories = _expected_bundle_directories(expected_paths)
    for unexpected in sorted(bundle_directories - expected_directories):
        errors.append(f"bundle contains unmanifested directory: {unexpected}")

    for required_path in REQUIRED_ARTIFACT_PATHS:
        if required_path not in manifest_artifacts:
            errors.append(f"manifest missing required artifact: {required_path}")

    report_md_path = bundle_dir / "sccp-release-readiness.md"
    report_path = bundle_dir / "sccp-release-readiness.json"
    summary_path = bundle_dir / "sccp-all-lanes-summary.json"
    notes_path = bundle_dir / "sccp-release-notes-attachment.md"
    try:
        report = _load_json(report_path)
    except DuplicateJsonKeyError as exc:
        report = {}
        errors.append(f"readiness report JSON contains duplicate key: {exc.key}")
    except UnicodeDecodeError as exc:
        report = {}
        errors.append(f"readiness report JSON is not UTF-8 text: {exc}")
    except (OSError, json.JSONDecodeError) as exc:
        report = {}
        errors.append(f"cannot load readiness report JSON: {exc}")
    if not isinstance(report, dict) or not report:
        errors.append("readiness report JSON must be a non-empty object")
        report = {}
    else:
        errors.extend(
            _canonical_json_file_errors("readiness report", report_path, report)
        )
    for key in sorted(set(report) - READINESS_REPORT_KEYS):
        errors.append(
            f"readiness report contains unknown top-level field: {key}"
        )
    if report:
        for key in sorted(READINESS_REPORT_KEYS - set(report)):
            errors.append(f"readiness report missing top-level field: {key}")
    try:
        summary = _load_json(summary_path)
    except DuplicateJsonKeyError as exc:
        summary = {}
        errors.append(f"all-lanes summary JSON contains duplicate key: {exc.key}")
    except UnicodeDecodeError as exc:
        summary = {}
        errors.append(f"all-lanes summary JSON is not UTF-8 text: {exc}")
    except (OSError, json.JSONDecodeError) as exc:
        summary = {}
        errors.append(f"cannot load all-lanes summary JSON: {exc}")
    if not isinstance(summary, dict) or not summary:
        errors.append("all-lanes summary JSON must be a non-empty object")
        summary = {}
    else:
        errors.extend(
            _canonical_json_file_errors("all-lanes summary", summary_path, summary)
        )
    report_evidence: dict[str, Any] = {}
    report_release_checklist: dict[str, Any] = {}
    report_corridor: dict[str, Any] = {}
    summary_release_checklist: dict[str, Any] = {}
    if report:
        errors.extend(
            _boolean_field_errors("readiness report", report, "production_ready")
        )
        errors.extend(
            _string_list_field_errors(
                "readiness report",
                report,
                "blockers",
                allow_empty=True,
            )
        )
        raw_evidence = report.get("evidence")
        if not isinstance(raw_evidence, dict):
            errors.append("readiness report evidence is not an object")
        else:
            report_evidence = raw_evidence
            errors.extend(
                _all_lanes_summary_schema_errors(
                    "readiness report embedded evidence",
                    report_evidence,
                )
            )
        raw_release_checklist = report.get("release_checklist")
        if not isinstance(raw_release_checklist, dict):
            errors.append("readiness report release_checklist is not an object")
        else:
            report_release_checklist = raw_release_checklist
            errors.extend(
                _release_checklist_schema_errors(
                    "readiness report",
                    report_release_checklist,
                )
            )
        raw_corridor = report.get("corridor")
        if not isinstance(raw_corridor, dict):
            errors.append("readiness report corridor is not an object")
        else:
            report_corridor = raw_corridor
            errors.extend(_corridor_schema_errors(report_corridor))
    if summary:
        errors.extend(
            _all_lanes_summary_schema_errors("all-lanes summary", summary)
        )
        raw_summary_checklist = summary.get("release_checklist")
        if not isinstance(raw_summary_checklist, dict):
            errors.append("all-lanes summary release_checklist is not an object")
        else:
            summary_release_checklist = raw_summary_checklist
            errors.extend(
                _release_checklist_schema_errors(
                    "all-lanes summary",
                    summary_release_checklist,
                    require_ready=False,
                )
            )
    if report:
        referenced_paths = _referenced_report_artifact_paths(report)
        for unexpected in sorted(set(manifest_artifacts) - referenced_paths):
            errors.append(
                "manifest contains artifact not referenced by readiness report: "
                f"{unexpected}"
            )
        for missing in sorted(referenced_paths - set(manifest_artifacts)):
            errors.append(
                "manifest missing readiness report referenced artifact: "
                f"{missing}"
            )
        try:
            expected_order = _expected_manifest_artifact_order(report)
        except Exception as exc:
            errors.append(f"cannot compute canonical manifest artifact order: {exc}")
        else:
            if _manifest_artifact_paths_in_order(artifacts) != expected_order:
                errors.append(
                    "manifest artifact order does not match canonical "
                    "release bundle order"
                )
    if report:
        try:
            report_markdown = report_md_path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            errors.append(f"readiness report Markdown is not UTF-8 text: {exc}")
        except OSError as exc:
            errors.append(f"cannot load readiness report Markdown: {exc}")
        else:
            errors.extend(
                _readiness_markdown_invariant_errors(report, report_markdown)
            )
            try:
                expected_markdown = _expected_readiness_markdown(report)
            except Exception as exc:
                errors.append(f"cannot render readiness report Markdown: {exc}")
            else:
                if report_markdown != expected_markdown:
                    errors.append(
                        "readiness report Markdown does not match readiness report JSON"
                    )

    if report and not report.get("production_ready"):
        errors.append("readiness report is not production_ready")
    if report and report.get("blockers"):
        errors.append("readiness report contains blockers")
    if report:
        errors.extend(_input_provenance_schema_errors(report))
        report_inputs = report.get("inputs")
        if isinstance(report_inputs, list) and report_inputs != _expected_input_paths(report):
            errors.append(
                "readiness report inputs do not match copied input artifacts"
            )
    if report and _active_launch_blockers(report_evidence):
        errors.append(
            f"readiness report embedded evidence has active {ACTIVE_LAUNCH_DISPLAY} launch blockers"
        )
    if report and not report_release_checklist.get("ready"):
        errors.append("readiness report release_checklist is not ready")
    if report and report_release_checklist != _expected_release_checklist(report):
        errors.append(
            "readiness report release_checklist does not match embedded evidence"
        )
    if report and not report_corridor.get("production_ready"):
        errors.append("readiness report production corridor is not ready")
    if report and report_corridor.get("require_phase_evidence") is not True:
        errors.append("readiness report does not require hashed phase evidence")
    if report:
        errors.extend(_corridor_phase_errors(report_corridor))
    if summary and _active_launch_blockers(summary):
        errors.append(f"all-lanes summary has active {ACTIVE_LAUNCH_DISPLAY} launch blockers")
    if summary and not _active_launch_release_checklist(summary).get("ready"):
        errors.append(
            f"all-lanes summary active {ACTIVE_LAUNCH_DISPLAY} release checklist is not ready"
        )
    if report and summary and report_evidence != summary:
        errors.append("all-lanes summary does not match readiness report evidence")
    if report:
        try:
            copied_summary = _copied_input_summary(bundle_dir, report, errors)
        except Exception as exc:
            copied_summary = None
            errors.append(
                f"cannot recompute all-lanes summary from copied evidence: {exc}"
            )
        if copied_summary is not None:
            if summary and copied_summary != summary:
                errors.append(
                    "all-lanes summary does not match copied evidence inputs"
                )
            if report_evidence != copied_summary:
                errors.append(
                    "readiness report evidence does not match copied evidence inputs"
                )
    if report:
        report_input_artifacts = report.get("input_artifacts")
        if not isinstance(report_input_artifacts, list):
            report_input_artifacts = []
        for artifact in report_input_artifacts:
            _check_report_artifact(
                errors,
                manifest_artifacts,
                artifact,
                label="readiness report input",
            )
        corridor = report_corridor
        phase_artifacts = corridor.get("evidence_artifacts", {})
        if not isinstance(phase_artifacts, dict):
            errors.append("readiness report corridor evidence_artifacts is not an object")
            phase_artifacts = {}
        phases = corridor.get("phases", {})
        if isinstance(phases, dict):
            for phase in sorted(set(phase_artifacts) - set(phases)):
                errors.append(
                    "readiness report corridor has evidence artifact for "
                    f"unknown phase: {phase}"
                )
            for phase, status in phases.items():
                if status != "passed":
                    continue
                _check_report_artifact(
                    errors,
                    manifest_artifacts,
                    phase_artifacts.get(phase),
                    label=f"readiness report phase {phase}",
                )
                errors.extend(
                    _phase_transcript_errors(
                        bundle_dir,
                        phase,
                        phase_artifacts.get(phase),
                    )
                )
        else:
            errors.append("readiness report corridor phases is not an object")
        crypto = report.get("cryptographic_evidence")
        lanes = report_evidence.get("lanes", [])
        if not isinstance(crypto, list) or not crypto:
            errors.append("readiness report cryptographic_evidence is missing")
        elif isinstance(lanes, list) and len(crypto) != len(lanes):
            errors.append("readiness report cryptographic_evidence does not cover every lane")
        if isinstance(crypto, list):
            errors.extend(_cryptographic_evidence_inventory_errors(crypto))
            errors.extend(_cryptographic_evidence_lane_binding_errors(crypto, lanes))
            expected_crypto = _expected_cryptographic_evidence(report_evidence)
            if crypto != expected_crypto:
                errors.append(
                    "readiness report cryptographic_evidence does not match embedded lane evidence"
                )
        surfaces = report.get("user_prover_submission_surfaces")
        if not isinstance(surfaces, list) or not surfaces:
            errors.append("readiness report user_prover_submission_surfaces is missing")
        else:
            for row in surfaces:
                if not isinstance(row, dict):
                    errors.append(
                        "readiness report user prover submission surface row "
                        "is not an object"
                    )
                    continue
                for key in sorted(set(row) - USER_PROVER_SUBMISSION_SURFACE_KEYS):
                    errors.append(
                        "readiness report user prover submission surface row "
                        f"contains unknown field: {key}"
                    )
                for key in sorted(USER_PROVER_SUBMISSION_SURFACE_KEYS - set(row)):
                    errors.append(
                        "readiness report user prover submission surface row "
                        f"missing field: {key}"
                    )
                errors.extend(_submission_surface_row_schema_errors(row))
            errors.extend(_submission_surface_inventory_errors(surfaces))
            try:
                expected_surfaces = _expected_submission_surfaces(report)
            except Exception as exc:
                errors.append(
                    f"cannot render user prover submission surfaces: {exc}"
                )
            else:
                if surfaces != expected_surfaces:
                    errors.append(
                        "readiness report user_prover_submission_surfaces "
                        "does not match corridor phases"
                    )
        if isinstance(crypto, list):
            for row in crypto:
                if not isinstance(row, dict):
                    errors.append("readiness report cryptographic evidence row is not an object")
                    continue
                for key in sorted(set(row) - CRYPTOGRAPHIC_EVIDENCE_KEYS):
                    errors.append(
                        "readiness report cryptographic evidence row contains "
                        f"unknown field: {key}"
                    )
                errors.extend(_cryptographic_evidence_row_schema_errors(row))
                if row.get("domain") != ACTIVE_LAUNCH_DOMAIN:
                    continue
                if row.get("route_canary_evidence_bound") is not True:
                    errors.append(
                        "readiness report cryptographic evidence row has unbound route canary"
                    )
                for field in (
                    "source_verifier_material_hash",
                    "source_adapter_engine_deployment_hash",
                    "destination_binding_hash",
                    "route_allowlist_hash",
                    "route_canary_evidence_hash",
                    "route_canary_evidence_source",
                ):
                    if not row.get(field):
                        errors.append(
                            "readiness report cryptographic evidence row missing "
                            f"{field}"
                        )
    if manifest.get("production_ready") is not True:
        errors.append("manifest production_ready is not true")
    if manifest.get("release_checklist_ready") is not True:
        errors.append("manifest release_checklist_ready is not true")
    if manifest.get("corridor_ready") is not True:
        errors.append("manifest corridor_ready is not true")
    if manifest.get("blockers"):
        errors.append("manifest contains blockers")
    if report:
        if manifest.get("production_ready") != report.get("production_ready"):
            errors.append(
                "manifest production_ready does not match readiness report"
            )
        if manifest.get("blockers") != report.get("blockers"):
            errors.append("manifest blockers do not match readiness report blockers")
        if report_release_checklist:
            if manifest.get("release_checklist_ready") != report_release_checklist.get(
                "ready"
            ):
                errors.append(
                    "manifest release_checklist_ready does not match "
                    "readiness report release_checklist"
                )
        if report_corridor:
            if manifest.get("corridor_ready") != report_corridor.get(
                "production_ready"
            ):
                errors.append(
                    "manifest corridor_ready does not match readiness report corridor"
                )
    if summary:
        summary_launch_ready = not _active_launch_blockers(summary)
        if manifest.get("production_ready") != summary_launch_ready:
            errors.append(
                f"manifest production_ready does not match all-lanes summary active {ACTIVE_LAUNCH_DISPLAY} launch readiness"
            )
        summary_launch_checklist = _active_launch_release_checklist(summary)
        if summary_release_checklist:
            if manifest.get("release_checklist_ready") != summary_launch_checklist.get("ready"):
                errors.append(
                    "manifest release_checklist_ready does not match "
                    f"all-lanes summary active {ACTIVE_LAUNCH_DISPLAY} release checklist"
                )

    try:
        notes = notes_path.read_text(encoding="utf-8")
    except UnicodeDecodeError as exc:
        notes = ""
        errors.append(f"release-notes attachment is not UTF-8 text: {exc}")
    except OSError as exc:
        notes = ""
        errors.append(f"cannot load release-notes attachment: {exc}")
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        path = artifact.get("path")
        digest = artifact.get("sha256")
        if path == "sccp-release-notes-attachment.md":
            continue
        if isinstance(path, str) and path not in notes:
            errors.append(f"release notes attachment does not list {path}")
        if isinstance(digest, str) and digest not in notes:
            errors.append(f"release notes attachment does not list hash for {path}")
    if "manifest.json" not in notes:
        errors.append("release notes attachment does not list manifest.json")
    if report and notes:
        try:
            expected_notes = _expected_release_notes_attachment(report, artifacts)
        except Exception as exc:
            errors.append(f"cannot render release-notes attachment: {exc}")
        else:
            if notes != expected_notes:
                errors.append(
                    "release notes attachment does not match manifest and report"
                )

    return {
        "verified": not errors,
        "errors": errors,
        "artifacts": artifacts,
        "manifest_sha256": manifest_sha256,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify a generated SCCP release-note attachment bundle.",
    )
    parser.add_argument("bundle_dir", type=Path, help="Bundle directory to verify.")
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print the verification summary as JSON.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    summary = verify_bundle(args.bundle_dir)
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    elif summary["verified"]:
        print(f"SCCP release bundle verified: {args.bundle_dir}")
    else:
        print(f"SCCP release bundle verification failed: {args.bundle_dir}")
        for error in summary["errors"]:
            print(f"- {error}")
    return 0 if summary["verified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
