#!/usr/bin/env python3
"""Verify a published SCCP release-note attachment bundle."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import re
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
        "pytests/scripts/sccp_substrate_destination_evidence_test.py",
        "pytests/scripts/sccp_substrate_live_evidence_test.py",
        "pytests/scripts/sccp_substrate_source_evidence_test.py",
        "pytests/scripts/sccp_ton_destination_evidence_test.py",
        "pytests/scripts/sccp_ton_live_evidence_test.py",
        "pytests/scripts/sccp_ton_source_state_evidence_test.py",
        "pytests/scripts/sccp_tron_live_evidence_test.py",
        "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
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
    ),
    "java-android": (
        "java -version",
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests",
        "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests",
        "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
    ),
    "dotnet-sdk": (
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
    ),
    "contract-smoke": (
        "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
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
        "package declarations expose Ethereum mainnet SCCP facade methods",
        "package declarations expose BSC mainnet Parlia finality evidence hooks",
    ),
    "python-sdk": (" passed in ",),
    "swift-sdk": ("0 failures",),
    "kotlin-sdk": ("version \"21", "BUILD SUCCESSFUL"),
    "java-android": ("version \"21", "BUILD SUCCESSFUL"),
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
ACTIVE_LAUNCH_DOMAIN = SCCP_DOMAIN_ETH
ACTIVE_LAUNCH_CHAIN = "eth"
ACTIVE_LAUNCH_POLICY = "EthereumMainnetLane"
ACTIVE_LAUNCH_DISPLAY = "Ethereum mainnet"
ACTIVE_LAUNCH_EVM_CHAIN_ID_EVIDENCE = {
    "eth": "`eth_chainId == 0x1` (1)",
    "bsc": "`eth_chainId == 0x38` (56)",
}.get(ACTIVE_LAUNCH_CHAIN, "the configured mainnet chain id")
ACTIVE_LAUNCH_EVM_CHAIN_ID_MARKER = {
    "eth": "eth_chainId == 0x1",
    "bsc": "eth_chainId == 0x38",
}.get(ACTIVE_LAUNCH_CHAIN, "eth_chainId")
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
EVM_EXPECTED_RPC_CHAIN_IDS = {
    SCCP_DOMAIN_ETH: 1,
    SCCP_DOMAIN_BSC: 56,
}
BSC_CHAIN_PROFILES = {
    "bsc": {
        "rpc_chain_id": 56,
        "route_allowlist_id": "sccp:bsc:route-allowlist:bsc-mainnet:v1",
    },
    "bsc-testnet": {
        "rpc_chain_id": 97,
        "route_allowlist_id": "sccp:bsc:route-allowlist:bsc-testnet:v1",
    },
}


def _chain_matches_domain(domain: int, chain: str) -> bool:
    if domain == SCCP_DOMAIN_BSC:
        return chain in BSC_CHAIN_PROFILES
    return chain == ALL_LANES_CHAIN_BY_DOMAIN.get(domain)


def _expected_chain_label(domain: int) -> str | None:
    if domain == SCCP_DOMAIN_BSC:
        return "bsc or bsc-testnet"
    return ALL_LANES_CHAIN_BY_DOMAIN.get(domain)


def _bsc_profile_for_chain(chain: Any) -> dict[str, Any]:
    if chain == "bsc-testnet":
        return BSC_CHAIN_PROFILES["bsc-testnet"]
    return BSC_CHAIN_PROFILES["bsc"]


def _expected_evm_rpc_chain_id(domain: int, chain: Any = None) -> int:
    if domain == SCCP_DOMAIN_BSC:
        return int(_bsc_profile_for_chain(chain)["rpc_chain_id"])
    return EVM_EXPECTED_RPC_CHAIN_IDS[domain]


def _route_allowlist_chain_and_id(
    domain: int,
    chain: Any = None,
) -> tuple[str, str] | None:
    if domain == SCCP_DOMAIN_BSC:
        selected_chain = "bsc-testnet" if chain == "bsc-testnet" else "bsc"
        return (
            selected_chain,
            str(BSC_CHAIN_PROFILES[selected_chain]["route_allowlist_id"]),
        )
    route_chain = ALL_LANES_CHAIN_BY_DOMAIN.get(domain)
    route_allowlist_id = ALL_LANES_ROUTE_ALLOWLIST_ID_BY_DOMAIN.get(domain)
    if route_chain is None or route_allowlist_id is None:
        return None
    return route_chain, route_allowlist_id
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
    "native_evm_prover_bundle",
    "cryptographic_evidence",
    "user_prover_submission_surfaces",
}
READINESS_MARKDOWN_REQUIRED_HEADINGS = (
    "## Evidence Inputs",
    "## Production Corridor",
    "## Release Checklist",
    "## Cryptographic Evidence",
    "## User Prover Submission Surfaces",
    "## Native Prover Bundle",
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
    ACTIVE_LAUNCH_EVM_CHAIN_ID_MARKER,
    "finalized` block tag",
    "Governed live deployment evidence",
    "Public release notes",
    "--native-evm-prover-bundle",
    "sccp-native-evm-groth16-prover-bundle-v1",
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
NATIVE_EVM_PROVER_BUNDLE_SUMMARY_KEYS = {
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
    "sdk_artifacts",
    "validation_status",
    "validation_blockers",
}
NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_KEYS = {
    "sdk",
    "implementation",
    "implementation_hash",
    "implementation_artifact",
}
CRYPTOGRAPHIC_EVIDENCE_KEYS = {
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
ETHEREUM_INBOUND_ADVERSARIAL_SDK_TEST_MARKERS = (
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "EthereumMainnetSccp rejects failed or drifted receipt evidence before proving",
            "receipt status must be 0x1",
            "beaconFinality.executionReceiptsRoot",
            "EthereumMainnetSccp validates source bridge logs in receipt evidence",
            "sourceEventLog(), sourceEventLog()",
            "/exactly 2 topics/u",
            "/source event log data must be 0x/u",
            "/source event digest must not be zero/u",
            "/removed logs/u",
            'for (const missingField of ["transactionHash", "blockHash", "blockNumber"])',
            '["transaction_hash", hex32("ab"), "receipt.logs[0].transactionHash"]',
            '["block_hash", hex32("ac"), "receipt.logs[0].blockHash"]',
            '["block_number", "0x1235", "receipt.logs[0].blockNumber"]',
            "must not use multiple aliases",
            "receipt_proof_hash: receiptProofHash",
            'receiptProofHash: hex32("00")',
            "receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof)",
            "/requires receiptProof/u",
            "/transactionHash must not be zero/u",
            "/blockHash must not be zero/u",
            "/receipt\\.transactionHash must not be zero/u",
            "/receipt\\.blockHash must not be zero/u",
            "/block\\.hash must not be zero/u",
            "/block\\.receiptsRoot must not be zero/u",
            "EthereumMainnetSccp inbound prover receives immutable evidence snapshots",
            "Object.isFrozen(evidence.receiptProof.receiptTrieProofNodes[0])",
            'evidence.beaconFinality.finalityBranch.push(hex32("99"))',
            "EthereumMainnetSccp collectInboundEvidenceFromReceipt snapshots consensus evidence",
            "Object.isFrozen(evidence.block.mutableWitness.branch)",
            "mutablePayload[0] = 0x7e",
            "evidence.beaconFinality.mutableWitness.branch",
            "EthereumMainnetSccp requires linked local prover functions",
            "ERR_SCCP_ETH_INBOUND_PROVER_UNAVAILABLE",
            "assert.equal(executionRequests, 0)",
            "SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES",
            "/proofBytes must be at most/u",
            "assert.equal(submitterCalled, false)",
            "/requires receipt source event validation/u",
            '["executionBlockHash", /executionBlockHash must not be zero/u]',
            '["executionReceiptsRoot", /executionReceiptsRoot must not be zero/u]',
            '["beaconFinalizedRoot", /beaconFinalizedRoot must not be zero/u]',
            '["syncCommitteeRoot", /syncCommitteeRoot must not be zero/u]',
            "/receiptProof\\.executionBlockNumber must match beaconFinality\\.executionBlockNumber/u",
            "/receiptProof\\.executionBlockHash must match beaconFinality\\.executionBlockHash/u",
            "/receiptProof\\.executionReceiptsRoot must match beaconFinality\\.executionReceiptsRoot/u",
            "/receiptProof\\.beaconFinalizedRoot must match beaconFinality\\.finalizedHeaderRoot/u",
            "/receiptProof\\.syncCommitteeRoot must match beaconFinality\\.syncCommitteeRoot/u",
            "/receiptProof\\.beaconSlot must match beaconFinality\\.beaconSlot/u",
            "/receiptProof\\.sourceEventDigest must match receipt source event/u",
            "SAMPLE_SYNC_COMMITTEE_BITS",
            "LOW_SYNC_COMMITTEE_BITS",
            "/beaconFinality\\.finalityBranch/u",
            "/beaconFinality\\.syncCommitteeBits/u",
            "/beaconFinality\\.syncCommitteeBits must contain Ethereum sync committee supermajority/u",
            "/beaconFinality\\.syncCommitteeParticipation must match syncCommitteeBits/u",
            "/beaconFinality\\.syncSignatureSlot must cover beaconFinality\\.beaconSlot/u",
            "/beaconFinality\\.syncCommitteeSignature must not be zero/u",
            "/sync_committee_bits must contain Ethereum sync committee supermajority/u",
            "/sync_committee_signature must not be zero/u",
            "aliasOnlyProverCalls",
            "assert.equal(alias in evidence.beaconFinality, false)",
            "Ethereum receipt-proof transcript rejects empty trie and finality branches",
            'fullReceipt(0, { transaction_index: "0x0" })',
            '["transaction_hash", hex32("ab"), "receipt.transactionHash"]',
            '["receipts_root", hex32("ab"), "block.receiptsRoot"]',
            '["block_hash", hex32("ab"), "blockReceipts.blockHash"]',
            '["cumulative_gas_used", "0x5208", "receipt.cumulativeGasUsed"]',
            '["logs_bloom", `0x${"11".repeat(256)}`, "receipt.logsBloom"]',
            "receiptTrieProofNodes: []",
            "inclusionBranch: []",
            "sourceDomain: SCCP_DOMAIN_BSC",
            "/sourceDomain must be ETH/u",
        ),
    ),
    (
        "python/iroha_torii_client/sccp.py",
        (
            "_normalize_ethereum_mainnet_finality_branch",
            "beaconFinality.finalityBranch",
            "must contain 6 siblings",
            "_normalize_ethereum_mainnet_finality_sync_committee_bits",
            "must contain Ethereum sync committee supermajority",
            "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot",
            "receiptProof.syncCommitteeRoot must match beaconFinality.syncCommitteeRoot",
            "receiptProof.beaconSlot must match beaconFinality.beaconSlot",
            "_clone_prover_callback_request(evidence), options",
            "return _clone_prover_callback_request(evidence)",
            '_require_native_recursive_proof_bytes(proof_copy, "proofBytes")',
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "ETHEREUM_FINALITY_BRANCH",
            "LOW_ETHEREUM_SYNC_COMMITTEE_BITS",
            'evidence["beacon_finality"]["finality_branch"]',
            "finalityBranch must contain 6 siblings",
            "beaconFinality.syncCommitteeParticipation",
            "receiptProof.beaconFinalizedRoot",
            "receiptProof.syncCommitteeRoot",
            "receiptProof.beaconSlot",
            "test_ethereum_mainnet_sccp_collects_immutable_evidence_snapshot_from_mutable_inputs",
            "ConsensusProvider",
            'evidence["block"]["mutableWitness"]["branch"].append(HEX32_A)',
            'finality_witness["bytes"][0] = 0x7E',
            "test_ethereum_mainnet_sccp_inbound_prover_receives_immutable_evidence_snapshot",
            'evidence["beacon_finality"]["finality_branch"].append(HEX32_A)',
            "mutable_payload[0] = 0x7E",
            'mutable_receipt_proof_nodes[0] = "0x" + "99" * 32',
            'callback_evidence["block"]["mutableWitness"]["bytes"] == b"\\xbb"',
            "SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1",
            "proofBytes must be at most",
            'assert submitted == [b"\\x0a\\x0b\\x0c"]',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            'invalidPublicInputs("receipt.status")',
            'invalidPublicInputs("beaconFinality.executionReceiptsRoot")',
            "wrongTopicReceipt",
            "extraTopicReceipt",
            'invalidPublicInputs("receipt.logs[0].topics")',
            "nonEmptyDataReceipt",
            'invalidPublicInputs("receipt.logs[0].data")',
            "zeroDigestReceipt",
            'invalidPublicInputs("receipt.logs[0].topics[1]")',
            "duplicateLogReceipt",
            "removedLogReceipt",
            'invalidPublicInputs("receipt.logs")',
            'for missingField in ["transactionHash", "blockHash", "blockNumber"]',
            "EthereumMainnetInboundEvidence(receiptProofHash: receiptProofHash)",
            'String(repeating: "00", count: 32)',
            'receiptProofHash + " "',
            'XCTFail("prover callback must not run without receiptProof")',
            'XCTFail("prover callback must not run without source event validation")',
            'invalidPublicInputs("receiptProof")',
            'missingFinalityBranchFinality.removeValue(forKey: "finalityBranch")',
            'invalidPublicInputs("beaconFinality.finalityBranch")',
            'invalidPublicInputs("beaconFinality.syncCommitteeBits")',
            'mismatchedSyncParticipationFinality["syncCommitteeParticipation"] = "341"',
            'underQuorumSyncBitsFinality["syncCommitteeBits"] = "0x01" + String(repeating: "00", count: 63)',
            '.invalidPublicInputs("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")',
            'staleSyncSignatureSlotFinality["syncSignatureSlot"] = "31"',
            'zeroSyncCommitteeSignatureFinality["syncCommitteeSignature"] = "0x" + String(repeating: "00", count: 96)',
            '.zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature")',
            "let aliasOnlyFinality: [String: Any]",
            "XCTAssertFalse(finality.keys.contains(alias))",
            '"finalized_header_root", "0x" + String(repeating: "13", count: 32)',
            '"sync_committee_root", "0x" + String(repeating: "14", count: 32)',
            '"beacon_slot", "33", "beaconFinality.beaconSlot"',
            '"transaction_hash", "0x" + String(repeating: "ab", count: 32)',
            '"block_hash", "0x" + String(repeating: "ac", count: 32)',
            '"block_number", "0x1235", "receipt.logs[0].blockNumber"',
            '.invalidRlp("blockReceipts[0].transactionHash")',
            '.invalidRlp("receipt.cumulativeGasUsed")',
            '.invalidRlp("receipt.logsBloom")',
            'receiptTrieProofNodes: []',
            '.invalidValidatorSet("receiptTrieProofNodes")',
            'inclusionBranch: []',
            '.invalidBranch("inclusionBranch")',
            "testEthereumMainnetInboundProverReceivesCallbackEvidenceSnapshot",
            "testEthereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            "testBscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            "XCTAssertEqual(consensusProvider.calls, 1)",
            "sccpNativeRecursiveMaxProofBytes + 1",
            '.invalidPublicInputs("proofBytes")',
            "finalityWitness.setObject(\"changed\", forKey: \"new\" as NSString)",
            "XCTAssertNil(evidence.beaconFinality?[\"mutableWitness\"] as? NSMutableDictionary)",
            "mutableReceiptProofNode[0] = 0xff",
            "blockReceiptsWitness.add(\"changed\")",
            "sourceDomain: sccpDomainBsc",
            "sourceDomain: sccpDomainEthereum",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashesTest.kt",
        (
            "emptyEvmReceiptNodes",
            "receiptTrieProofNodes = emptyList()",
            "emptyEvmInclusionBranch",
            "inclusionBranch = emptyList()",
            "inclusionBranch must not be empty",
            "emptyBscInclusionBranch",
            "bscDomainEvmReceiptProof",
            "sourceDomain must be ETH",
            "ethDomainBscReceiptProof",
            "sourceDomain must be BSC",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            'receipt + ("status" to "0x0")',
            'beaconFinality + ("executionReceiptsRoot"',
            '"logs" to listOf(sourceEventLog, sourceEventLog)',
            '"0x" + "66".repeat(32)',
            '"data" to "0x01"',
            '"0x" + "00".repeat(32)',
            'sourceEventLog + ("removed" to true)',
            "SccpEthereumMainnet.sourceEventTopic()",
            "receiptProof.executionReceiptsRoot",
            'for (missingField in listOf("transactionHash", "blockHash", "blockNumber"))',
            "EthereumMainnetInboundEvidence(receiptProofHash = receiptProofHash)",
            "receiptProofHash must not be zero",
            'receiptProofHash + " "',
            "val missingReceiptProof = assertFailsWith<IllegalArgumentException>",
            'missingReceiptProof.message?.contains("receiptProof")',
            "prebuiltProofOnlyProverCalls",
            'prebuiltProofWithoutSourceEvent.message?.contains("receipt source event validation")',
            'missingFinalityBranch.message?.contains("beaconFinality.finalityBranch")',
            'missingSyncBits.message?.contains("beaconFinality.syncCommitteeBits")',
            'mismatchedSyncParticipation.message?.contains("beaconFinality.syncCommitteeParticipation")',
            'underQuorumSyncBits.message?.contains("beaconFinality.syncCommitteeBits")',
            'sync_committee_bits must contain Ethereum sync committee supermajority',
            'staleSyncSignatureSlot.message?.contains("beaconFinality.syncSignatureSlot")',
            'zeroSyncCommitteeSignature.message?.contains("beaconFinality.syncCommitteeSignature")',
            'beaconFinalityUpdateJson(syncCommitteeSignature = "0x" + "00".repeat(96))',
            "val aliasOnlyFinality = mapOf<String, Any?>",
            "assertTrue(alias !in finality)",
            'Triple("finalized_header_root", "0x" + "13".repeat(32), "beaconFinality.finalizedHeaderRoot")',
            'Triple("sync_committee_root", "0x" + "14".repeat(32), "beaconFinality.syncCommitteeRoot")',
            'Triple("beacon_slot", "33", "beaconFinality.beaconSlot")',
            'Triple("transaction_hash", "0x" + "ab".repeat(32), "receipt.logs[0].transactionHash")',
            'Triple("block_hash", "0x" + "ac".repeat(32), "receipt.logs[0].blockHash")',
            'Triple("block_number", "0x1235", "receipt.logs[0].blockNumber")',
            'Triple("transaction_hash", "0x" + "ac".repeat(32), "receipt.transactionHash")',
            'blockReceipts[0].transactionHash',
            'receipt + ("cumulative_gas_used" to "0x5208")',
            'receipt + ("logs_bloom" to ("0x" + "00".repeat(256)))',
            "ethereumMainnetInboundProverReceivesCallbackEvidenceSnapshot",
            "ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            "bscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            'assertFalse(collectedReceipt?.get("mutableWitness") === receiptWitness)',
            "SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1",
            'oversizedSubmit.message?.contains("proofBytes must be at most")',
            'finalityWitness["new"] = "changed"',
            "finalityBranchWitness.add(\"0x\" + \"99\".repeat(32))",
            "mutableReceiptProofNode[0] = 0x7c",
            'assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)',
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "Ethereum inbound collection must reject failed receipts",
            "beaconFinality.executionReceiptsRoot",
            'duplicateReceipt.put("logs"',
            "source-event validation must reject duplicate matching events",
            "Ethereum source-event validation must reject extra source-event topics",
            "Ethereum source-event validation must reject non-empty source-event data",
            "Ethereum source-event validation must reject zero source-event digest",
            "Ethereum source-event validation must reject removed logs",
            "EthereumMainnetSccp.sourceEventTopic()",
            'Arrays.asList("transactionHash", "blockHash", "blockNumber")',
            "hash-only receiptProofHash evidence",
            '"0x" + repeat("00", 32)',
            'receiptProofHash + " "',
            "Ethereum inbound proving must reject hash-only receipt proof evidence",
            "Ethereum inbound prover must not run without receipt proof material",
            "prebuiltProofOnlyProverCalls",
            "Ethereum inbound proving must reject proof-only evidence without source event validation",
            "Ethereum inbound proving must reject missing finality branch",
            "Ethereum inbound proving must reject missing sync-committee bits",
            'mismatchedSyncParticipationFinality.put("syncCommitteeParticipation", "341")',
            "Ethereum inbound proving must reject under-quorum sync-committee bits",
            "Beacon REST provider must reject under-quorum sync committee aggregate bits",
            'staleSyncSignatureSlotFinality.put("syncSignatureSlot", "31")',
            "Ethereum inbound proving must reject zero sync-committee signatures",
            "Beacon REST provider must reject zero sync committee aggregate signatures",
            'aliasOnlyFinality.put("execution_block_number", "0x1234")',
            "callback finality must not retain alias",
            "final Object[][] conflictingFinalityAliases",
            '"finalized_header_root", "0x" + repeat("13", 32), "beaconFinality.finalizedHeaderRoot"',
            '"sync_committee_root", "0x" + repeat("14", 32), "beaconFinality.syncCommitteeRoot"',
            "final Object[][] conflictingLogAliases",
            '"transaction_hash", "0x" + repeat("ab", 32), "receipt.logs[0].transactionHash"',
            '"block_hash", "0x" + repeat("ac", 32), "receipt.logs[0].blockHash"',
            "final String[][] receiptAliasConflicts",
            "blockReceipts[0].transactionHash",
            'conflictingGas.put("cumulative_gas_used", "0x5208")',
            'conflictingBloom.put("logs_bloom", "0x" + repeat("00", 256))',
            "ethereumMainnetInboundProverReceivesCallbackEvidenceSnapshot",
            "ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            "bscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            'collectedReceipt.get("mutableWitness") != receiptWitness',
            "Ethereum inbound prover output must reject oversized proof bytes",
            "Ethereum inbound submitter must reject oversized proof bytes",
            "BSC collection consensus callback must receive a receipt witness snapshot",
            'finalityWitness.put("new", "changed");',
            'finalityBranchWitness.add("0x" + repeat("99", 32));',
            "mutableReceiptProofNode[0] = 0x7c;",
            "Ethereum inbound callback receipt bytes must be detached",
            "Ethereum proof engine must receive a callback request snapshot",
            "Ethereum receipt-proof transcript must reject empty receiptTrieProofNodes",
            "Ethereum receipt-proof transcript must reject empty inclusionBranch",
            "Ethereum receipt-proof transcript must reject BSC sourceDomain",
            "BSC receipt-proof transcript must reject ETH sourceDomain",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "failedReceipt",
            "receiptProof.executionReceiptsRoot",
            "driftedFinalityReceiptsRoot",
            "wrongTopicLog",
            "extraTopicReceipt",
            'Assert.Contains("exactly 2 topics", extraTopic.Message)',
            "nonEmptyDataReceipt",
            'Assert.Contains("data must be 0x", nonEmptyData.Message)',
            "zeroDigestReceipt",
            'Assert.Contains("digest must not be zero", zeroDigest.Message)',
            "duplicateReceipt",
            'Assert.Contains("removed logs", removedSourceEventLog.Message)',
            'foreach (var missingField in new[] { "transactionHash", "blockHash", "blockNumber" })',
            "Assert.Null(receiptProofHashOnlyEvidence.ReceiptProof)",
            "ReceiptProofHash must not be zero",
            'ExpectedReceiptProofHash + " "',
            "missingReceiptProofProver",
            'Assert.Contains("receiptProof", missingReceiptProof.Message)',
            "unanchoredReceiptProofProver",
            'Assert.Contains("receipt source event validation", unanchoredReceiptProof.Message)',
            'missingFinalityBranchFinality.Remove("finalityBranch")',
            'Assert.Contains("beaconFinality.finalityBranch", missingFinalityBranch.Message)',
            'Assert.Contains("beaconFinality.syncCommitteeBits", missingSyncBits.Message)',
            "mismatchedSyncParticipationFinality",
            'Assert.Contains("beaconFinality.syncCommitteeParticipation", mismatchedSyncParticipation.Message)',
            "underQuorumSyncBitsFinality",
            'Assert.Contains("beaconFinality.syncCommitteeBits", underQuorumSyncBits.Message)',
            'Assert.Contains("sync_committee_bits must contain Ethereum sync committee supermajority", underQuorumSyncAggregate.Message)',
            '["syncSignatureSlot"] = "31"',
            'Assert.Contains("beaconFinality.syncSignatureSlot", staleSyncSignatureSlot.Message)',
            'Assert.Contains("beaconFinality.syncCommitteeSignature", zeroSyncCommitteeSignature.Message)',
            'Assert.Contains("sync_committee_signature must not be zero", zeroSyncAggregateSignature.Message)',
            "var aliasOnlyFinality = new Dictionary<string, object?>",
            "Assert.False(finality.ContainsKey(alias))",
            '("finalized_header_root", "0x" + string.Concat(Enumerable.Repeat("13", 32)), "beaconFinality.finalizedHeaderRoot")',
            '("sync_committee_root", "0x" + string.Concat(Enumerable.Repeat("14", 32)), "beaconFinality.syncCommitteeRoot")',
            '("beacon_slot", "33", "beaconFinality.beaconSlot")',
            '("transaction_hash", "0x" + new string(\'d\', 64), "receipt.logs[0].transactionHash")',
            '("block_hash", "0x" + new string(\'a\', 64), "receipt.logs[0].blockHash")',
            '("block_number", "0x1235", "receipt.logs[0].blockNumber")',
            '("transaction_hash", "0x" + new string(\'a\', 64), "receipt.transactionHash")',
            'Assert.Contains("blockReceipts[0].transactionHash", indexedHashAliasConflict.Message);',
            '["cumulative_gas_used"] = "0x5208"',
            '["logs_bloom"] = logsBloom',
            "InboundProverReceivesCallbackEvidenceSnapshot",
            "CollectInboundEvidenceSnapshotsConsensusBoundary",
            "BscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
            'Assert.NotSame(receiptWitness, collectedReceipt?["mutableWitness"])',
            "EthereumMainnetSccp.NativeRecursiveMaxProofBytes + 1",
            'Assert.Contains("proofBytes must be at most", oversizedSubmit.Message)',
            'Assert.False(returnedFinalitySnapshot.ContainsKey("new"))',
            'finalityBranchWitness.Add("0x" + new string(\'9\', 64));',
            "mutableReceiptProofNode[0] = 0x7c;",
            'Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(receiptNestedSnapshot["bytes"]))',
            "callbackRequest.PublicSignalWords[0] = \"0x\" + new string('f', 64);",
            "Assert.NotEqual(ExpectedPublicSignalWords[0], prover.Request.PublicSignalWords[0]);",
            "Assert.Throws<ArgumentException>(() => BuildBytes(sourceDomain: 2));",
            "Assert.Throws<ArgumentException>(() => BuildBytes(nodes: Array.Empty<byte[]>()));",
            "Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: Array.Empty<byte[]>()));",
        ),
    ),
)
ETHEREUM_OUTBOUND_PRECALLBACK_SDK_TEST_MARKERS = (
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert.equal(outboundProverCalled, false)",
            "ERR_SCCP_ETH_NATIVE_PROVER_ARTIFACTS_UNAVAILABLE",
            "verified native EVM prover artifacts",
            "Ethereum mainnet SCCP outbound from",
            "submittedTxs[3].from",
            "assert.notDeepStrictEqual(",
            "Array.from(callbackPublicInputsBytes),",
            "assert.deepEqual(Array.from(proofResult.bundleBytes), [1, 2, 3]);",
            'proofArtifactHash: hex32("91")',
            "proofArtifactHash and provingKeyHash must be supplied together",
            "proofArtifactHash and provingKeyHash must match request",
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "destinationBindingHash must match destinationBinding",
            "outbound_prover_called = False",
            "assert not outbound_prover_called",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "Ethereum outbound prover callback must not see BSC requests",
            "XCTAssertFalse(outboundProverCalled)",
            "Ethereum outbound facade must reject forged destinationBindingHash before returning request",
            "forgedBindingHashRequest",
            "String(repeating: \"99\", count: 32)",
            "proofArtifactHash: String(repeating: \"91\", count: 32)",
            ".invalidPublicInputs(\"proofArtifactHash/provingKeyHash\")",
            ".zeroField(\"proofArtifactHash\")",
            "artifactResult.proofArtifactHash",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "Ethereum outbound prover callback must not see BSC requests",
            "outboundProverCalled",
            'request.copy(destinationBindingHash = "0x" + "99".repeat(32))',
            'proofArtifactHash = "91".repeat(32)',
            "proofArtifactHash and provingKeyHash",
            "artifactResult.proofArtifactHash",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert !outboundProverCalled[0]",
            "Ethereum wrapProofResult must reject forged destinationBindingHash",
            "evmRequestWithDestinationBindingHash",
            "partial proof artifact metadata must be rejected",
            "zero proof artifact hash must be rejected",
            "proof result must carry proof artifact hash",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "Ethereum outbound prover callback must not see BSC requests",
            "Assert.Null(guardedProver.Request)",
            "request with { DestinationBindingHash = \"0x\" + new string('9', 64) }",
            "ProofArtifactHash = \"0x\" + new string('9', 64)",
            "artifactResult.ProofArtifactHash",
            "proofResult with",
        ),
    ),
)
ETHEREUM_LOCAL_ADMISSION_SDK_TEST_MARKERS = (
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "EthereumMainnetSccp builds ETH -> SORA local-admission submissions",
            "buildEthereumMainnetSccpLocalAdmissionSubmission(input)",
            "new EthereumMainnetSccp().buildLocalAdmissionSubmission(input)",
            "input.proofBytes[0] = 99",
            "proofBytes must not be all zero",
            "publicInputsBytes must not be all zero",
            "bundleBytes must not be all zero",
            "envelopeBytes must not be empty",
            "envelopeBytes must not be all zero",
            "statementHash must not be zero",
            "sourceVerifierMaterialHash must not be zero",
            "sourceAdapterEngineDeploymentHash must not be zero",
            "metadata is not canonical",
            'proofFamily: "debug-proof-family"',
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "test_ethereum_mainnet_sccp_builds_local_admission_submission",
            "build_ethereum_mainnet_sccp_local_admission_submission(input_value)",
            "EthereumMainnetSccp().build_local_admission_submission",
            'match="proofBytes must not be all zero"',
            'match="publicInputsBytes must not be all zero"',
            'match="bundleBytes must not be all zero"',
            'match="envelopeBytes must not be empty"',
            'match="envelopeBytes must not be all zero"',
            'match="statementHash must not be zero"',
            'match="sourceVerifierMaterialHash must not be zero"',
            'match="sourceAdapterEngineDeploymentHash must not be zero"',
            'match="metadata is not canonical"',
            '"proof_family": "debug-proof-family"',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "testEthereumMainnetSccpBuildsLocalAdmissionSubmission",
            "buildEthereumMainnetSccpLocalAdmissionSubmission(input)",
            "EthereumMainnetSccp().buildLocalAdmissionSubmission(input)",
            ".invalidPublicInputs(\"ETH -> SORA\")",
            ".allZeroProof",
            ".emptyProof",
            ".zeroField(\"statementHash\")",
            ".zeroField(\"sourceVerifierMaterialHash\")",
            ".zeroField(\"sourceAdapterEngineDeploymentHash\")",
            ".invalidPublicInputs(\"localAdmission.metadata\")",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "ethereumMainnetFacadeBuildsLocalAdmissionSubmission",
            "SccpEthereumMainnet.buildLocalAdmissionSubmission(input)",
            "EthereumMainnetSccp().buildLocalAdmissionSubmission(input)",
            "input.proofBytes[0] = 99",
            "proofBytes = byteArrayOf(0, 0)",
            "publicInputsBytes = byteArrayOf(0, 0)",
            "bundleBytes = byteArrayOf(0, 0)",
            "envelopeBytes = byteArrayOf()",
            "envelopeBytes = byteArrayOf(0, 0)",
            'statementHash = "0x" + "00".repeat(32)',
            'sourceVerifierMaterialHash = "0x" + "00".repeat(32)',
            'sourceAdapterEngineDeploymentHash = "0x" + "00".repeat(32)',
            'proofFamily = "debug-proof-family"',
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "ethereumMainnetFacadeBuildsLocalAdmissionSubmission",
            "EthereumMainnetSccp.buildLocalAdmissionSubmission(input)",
            "new EthereumMainnetSccp().buildLocalAdmission(input)",
            "input.proofBytes()[0] = 99",
            "Ethereum local admission must reject all-zero proof bytes",
            "Ethereum local admission must reject all-zero public input bytes",
            "Ethereum local admission must reject all-zero bundle bytes",
            "Ethereum local admission must reject empty envelope bytes",
            "Ethereum local admission must reject all-zero envelope bytes",
            "Ethereum local admission must reject zero statement hashes",
            "Ethereum local admission must reject zero source material hashes",
            "Ethereum local admission must reject zero source adapter deployment hashes",
            "Ethereum local admission must reject stale proof families",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "LocalAdmissionSubmissionWrapsNativeEthereumOutput",
            "EthereumMainnetSccp.BuildLocalAdmissionSubmission(input)",
            "Assert.Equal([1, 2, 3], submission.ProofBytes)",
            "input.ProofBytes[0] = 99",
            "SourceDomain = BscMainnetSccp.DomainBsc",
            "ProofBytes = [0, 0]",
            "PublicInputsBytes = [0, 0]",
            "BundleBytes = [0, 0]",
            "EnvelopeBytes = []",
            "EnvelopeBytes = [0, 0]",
            "StatementHash = \"0x\" + new string('0', 64)",
            "SourceVerifierMaterialHash = \"0x\" + new string('0', 64)",
            "SourceAdapterEngineDeploymentHash = \"0x\" + new string('0', 64)",
            "ProofFamily = \"debug-proof-family\"",
        ),
    ),
)
ETHEREUM_OUTBOUND_PROVIDER_VALIDATION_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "let providerValidated = false;",
            "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
            'if (typeof submit === "function")',
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "let providerValidated = false;",
            "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
            'if (typeof submit === "function")',
        ),
    ),
    (
        "python/iroha_torii_client/sccp.py",
        (
            'provider = options.get("execution_provider", self.execution_provider)',
            "await self.validate_execution_provider_mainnet(provider)",
            "return await _maybe_await(submitter(dict(submission), options))",
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "guarded_submit_called = False",
            "execution_provider=WrongChainProvider()",
            "assert guarded_submit_called is False",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        (
            "if let executionProvider {",
            "_ = try await validateExecutionProviderMainnet(executionProvider)",
            "return try await outboundSubmitFunction(submission)",
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        (
            "executionProvider?.let { validateExecutionProviderMainnet(it) }",
            "return submitter.submit(buildEthereumCalldata(input))",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        (
            "if (executionProvider != null) {",
            "validateExecutionProviderMainnet(executionProvider);",
            "return outboundSubmitter.submit(buildEthereumCalldata(input));",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "IEthereumMainnetExecutionProvider? executionProvider",
            "ValidateExecutionProviderMainnetAsync(",
            "return await outboundSubmitter.SubmitAsync(submission, cancellationToken)",
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_ethereum_sdks_validate_provider_before_outbound_submitter",
            "Ethereum outbound submitter paths must honor configured mainnet providers",
        ),
    ),
)
ETHEREUM_RECEIPT_ROOT_ZERO_SDK_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
    ),
    (
        "javascript/iroha_js/test/sccpSolanaProver.test.js",
        (
            "canonicalEvmReceiptRootMptValue(SCCP_ZERO_HASH_V1)",
            "must not be zero",
        ),
    ),
    (
        "javascript/iroha_js/test/package_dist.test.js",
        (
            'canonicalEvmReceiptRootMptValue(`0x${"00".repeat(32)}`)',
            "must not be zero",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            "public func canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "canonicalEvmReceiptRootMptValue(receiptRoot: zeroHash)",
            "XCTAssertThrowsError",
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            "fun canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))',
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashesTest.kt",
        (
            "SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "assertFailsWith<IllegalArgumentException>",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            "public static byte[] canonicalEvmReceiptRootMptValue(final String receiptRoot)",
            'fields.add(rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot")))',
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/SourceSccpProofsTests.java",
        (
            "SourceSccpProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "expectThrows",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "public static byte[] CanonicalEvmSccpReceiptProofBytes",
            "payload.Write(RpcHexToBytes(executionReceiptsRoot, nameof(executionReceiptsRoot), 32));",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "BuildBytes(executionReceiptsRoot: zeroRoot)",
            "BuildBytes(syncCommitteeRoot: zeroRoot)",
        ),
    ),
)
ETHEREUM_RECEIPT_RLP_ZERO_TOPIC_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "`receipt.logs[${index}].topics[${topicIndex}]`",
            "{ nonzero: false }",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "zeroTopicReceiptTrieProof",
            "topics: [hex32(\"00\")]",
        ),
    ),
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "method=f\"receipt.logs[{log_index}].topics[{topic_index}]\"",
            "nonzero=False",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_accepts_zero_log_topic_in_receipt_rlp",
            '"topics": ["0x" + "00" * 32]',
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            'field: "receipt.logs[\\(index)].topics[\\(topicIndex)]"',
            "nonzero: false",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "zeroTopicProof",
            '"topics": ["0x" + String(repeating: "00", count: 32)]',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            '"receipt.logs[$index].topics[$topicIndex]"',
            "nonzero = false",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "zeroTopicProof",
            '"topics" to listOf("0x" + "00".repeat(32))',
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            '"receipt.logs[" + index + "].topics[" + topicIndex + "]"',
            "false,\n                    false)))",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "zeroTopicProof",
            "generic Ethereum receipt RLP must allow zero log topics",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            '$"receipt.logs[{index}].topics[{topicIndex}]"',
            "nonZero: false",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "zeroTopicProof",
            '["topics"] = new object?[] { "0x" + new string(\'0\', 64) }',
        ),
    ),
)
ETHEREUM_RECEIPT_RLP_ZERO_ADDRESS_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "`receipt.logs[${index}].address`",
            "{ nonzero: false }",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "zeroAddressReceiptTrieProof",
            'address: `0x${"00".repeat(20)}`',
        ),
    ),
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "method=f\"receipt.logs[{log_index}].address\"",
            "nonzero=False",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_accepts_zero_log_address_in_receipt_rlp",
            '"address": "0x" + "00" * 20',
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            'field: "receipt.logs[\\(index)].address"',
            "nonzero: false",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "zeroAddressProof",
            '"address": "0x" + String(repeating: "00", count: 20)',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            '"receipt.logs[$index].address"',
            "nonzero = false",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "zeroAddressProof",
            '"address" to "0x" + "00".repeat(20)',
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            '"receipt.logs[" + index + "].address"',
            "false,\n                          false))",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "zeroAddressProof",
            "generic Ethereum receipt RLP must allow zero log addresses",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            '$"receipt.logs[{index}].address"',
            "nonZero: false",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "zeroAddressProof",
            '["address"] = "0x" + new string(\'0\', 40)',
        ),
    ),
)
ETHEREUM_RECEIPT_SOURCE_EVENT_CONTEXT_MARKERS = (
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "log_transaction_hash = _rpc_fixed_hex_data(",
            "log_block_hash = _rpc_fixed_hex_data(",
            "log_block_number = _rpc_quantity(",
            "source event log transactionHash does not match receipt",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_rejects_source_event_missing_context_fields",
            'for field in ("transactionHash", "blockHash", "blockNumber")',
        ),
    ),
)
ETHEREUM_RECEIPT_SOURCE_EVENT_MODE_MARKERS = (
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "allow_receipt_only_evidence: bool = False",
            "source_bridge_address is required for SCCP source-event evidence",
            "--allow-receipt-only-evidence",
            '"evidence_mode": (',
            '"source_event_validated": source_event_digest is not None',
            '"receipt_only_evidence": source_event_digest is None',
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_requires_explicit_receipt_only_mode_without_source_bridge",
            "test_collect_receipt_proof_allows_explicit_receipt_only_mode",
            "test_cli_requires_source_bridge_or_explicit_receipt_only_mode",
            "test_cli_exposes_explicit_receipt_only_mode",
        ),
    ),
)
ETHEREUM_RECEIPT_SOURCE_EVENT_ZERO_DIGEST_MARKERS = (
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "method=f\"receipt.logs[{index}].topics[1]\"",
            "raise RuntimeError(f\"{method} returned zero data\")",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_rejects_zero_source_event_digest",
            '"topics": [module.EVM_SOURCE_EVENT_TOPIC, "0x" + "00" * 32]',
            "zero source event digest was accepted",
        ),
    ),
)
ETHEREUM_RECEIPT_RPC_DUPLICATE_JSON_MARKERS = (
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "_json_object_without_duplicate_keys",
            "JSON-RPC returned duplicate JSON key",
            "object_pairs_hook=_json_object_without_duplicate_keys",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "FakeRawResponse",
            "test_collect_receipt_proof_rejects_duplicate_json_rpc_result_keys",
            "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields",
            "duplicate JSON-RPC result keys were accepted",
            "duplicate JSON receipt fields were accepted",
        ),
    ),
)
ETHEREUM_RECEIPT_BLOCK_TRANSACTION_HASH_MARKERS = (
    (
        "scripts/sccp_evm_receipt_proof_evidence.py",
        (
            "seen_transaction_hashes: set[bytes] = set()",
            'method=f"block receipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_receipt_trie_builder_rejects_duplicate_transaction_hashes",
            'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]',
            "duplicate block receipt transaction hashes were accepted",
        ),
    ),
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "fullReceipt(1, { transactionHash: TX_HASH })",
            "transactionHash values must be unique",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            "var seenTransactionHashes = Set<Data>()",
            'field: "blockReceipts[\\(index)].transactionHash"',
            "blockReceipts.transactionHash",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "duplicateHashReceipt",
            '.invalidRlp("blockReceipts.transactionHash")',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            "val seenTransactionHashes = HashSet<String>(receipts.size)",
            '"blockReceipts[$index].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "duplicateHashReceipt",
            "transactionHash values must be unique",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            "final Set<String> seenTransactionHashes = new HashSet<String>();",
            '"blockReceipts[" + index + "].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "duplicateHashReceipt",
            "receipt proof builder must reject duplicate block receipt transaction hashes",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "var seenTransactionHashes = new HashSet<string>(StringComparer.Ordinal);",
            '$"blockReceipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique.",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "duplicateTransactionHashReceipt",
            "transactionHash values must be unique",
        ),
    ),
)
ETHEREUM_JS_RECEIPT_ADMISSION_GUARD_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "Ethereum mainnet receipt proof construction requires beaconFinality.",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            "const receiptTransactionHash = requireEthereumRpcHexData(",
            'const blockHash = requireEthereumRpcHexData(block.hash, "block.hash", 32);',
            "const executionBlockHash = nonZeroHex32Bytes(",
            "const executionReceiptsRoot = nonZeroHex32Bytes(",
            "const beaconFinalizedRoot = nonZeroHex32Bytes(",
            "const syncCommitteeRoot = nonZeroHex32Bytes(",
            "await prove(immutableProverCallbackValue(evidence), options)",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "Ethereum mainnet receipt proof construction requires beaconFinality.",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            "const receiptTransactionHash = requireEthereumRpcHexData(",
            'const blockHash = requireEthereumRpcHexData(block.hash, "block.hash", 32);',
            "const executionBlockHash = nonZeroHex32Bytes(",
            "const executionReceiptsRoot = nonZeroHex32Bytes(",
            "const beaconFinalizedRoot = nonZeroHex32Bytes(",
            "const syncCommitteeRoot = nonZeroHex32Bytes(",
            "await prove(immutableProverCallbackValue(evidence), options)",
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_ethereum_js_dist_keeps_receipt_admission_guards",
            "Published JS must keep source receipt-proof admission checks in dist",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            'for (const field of ["finalizedHeaderRoot", "syncCommitteeRoot", "beaconSlot"])',
            "receipt proof construction requires beaconFinality\\\\.${field}",
        ),
    ),
)
ETHEREUM_SDK_RECEIPT_METADATA_GUARD_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        (
            '"blockReceipts.transactionHash"',
            '"blockReceipts.blockHash"',
            '"blockReceipts.blockNumber"',
            '"blockReceipts.receiptRlp"',
            "canonicalEvmReceiptRlp(currentReceipt)",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            "receiptType <= 0x7f",
            "let admittedType = UInt8(receiptType)",
            "(0x01...0x04).contains(admittedType)",
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "SccpSourceProofs.canonicalEvmReceiptRlp(receipt)",
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            "typed receipt type must fit one byte below 0x80",
            "val admittedType = receiptType.toInt()",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        (
            "eth_getBlockReceipts target receipt must match transactionHash",
            "eth_getBlockReceipts target receipt blockHash must match receipt",
            "eth_getBlockReceipts target receipt blockNumber must match receipt",
            "eth_getBlockReceipts target receipt RLP must match receipt",
            "SourceSccpProofs.canonicalEvmReceiptRlp(receipt)",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            "typed receipt type must fit one byte below 0x80",
            "final int admittedType = receiptType.intValue()",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "blockReceipts.transactionHash must match transactionHash.",
            "blockReceipts.blockHash must match receipt.",
            "blockReceipts.blockNumber must match receipt.",
            "blockReceipts.receiptRlp must match receipt.",
            "typed receipt type is not supported for Ethereum mainnet receipt proofs.",
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_ethereum_sdks_keep_receipt_metadata_guards",
            "Ethereum SDK receipt-proof builders must reject block-receipt metadata drift",
        ),
    ),
)
ETHEREUM_NATIVE_RECEIPT_FINALITY_GUARD_MARKERS = (
    (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        (
            "guard let beaconSlotInput = try Self.strictFirstPresent(",
            "guard let finalizedRootInput = try Self.strictFirstPresent(",
            "guard let syncCommitteeRootInput = try Self.strictFirstPresent(",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "for (missingField, label) in [",
            ".invalidPublicInputs(label)",
            '("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot")',
            '("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot")',
            '("beaconSlot", "beaconFinality.beaconSlot")',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        (
            "beaconFinality.beaconSlot is required for receiptProof",
            "beaconFinality.finalizedHeaderRoot is required for receiptProof",
            "beaconFinality.syncCommitteeRoot is required for receiptProof",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "for ((field, label) in listOf(",
            "beaconFinality = beaconFinality - field",
            '"finalizedHeaderRoot" to "beaconFinality.finalizedHeaderRoot"',
            '"syncCommitteeRoot" to "beaconFinality.syncCommitteeRoot"',
            '"beaconSlot" to "beaconFinality.beaconSlot"',
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        (
            "beaconFinality.beaconSlot is required for receiptProof",
            "beaconFinality.finalizedHeaderRoot is required for receiptProof",
            "beaconFinality.syncCommitteeRoot is required for receiptProof",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "for (final String[] missingFinalityCase :",
            "collection must reject missing",
            '{"finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot"}',
            '{"syncCommitteeRoot", "beaconFinality.syncCommitteeRoot"}',
            '{"beaconSlot", "beaconFinality.beaconSlot"}',
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "BeaconSlot = NormalizeUnsignedInteger(",
            "BeaconFinalizedRoot = NormalizeRpcHex(",
            "SyncCommitteeRoot = NormalizeRpcHex(",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "foreach (var (missingField, label) in new[]",
            "incompleteFinality.Remove(missingField);",
            '("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot")',
            '("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot")',
            '("beaconSlot", "beaconFinality.beaconSlot")',
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_ethereum_native_sdks_keep_receipt_finality_guards",
            "Native SDK receipt-proof builders must require Beacon finality roots",
        ),
    ),
)
ETHEREUM_NONCANONICAL_CHAIN_ID_TEST_MARKERS = (
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            'for (const chainId of ["1", 1, "0x01", "0X1", " 0x1", "0x1 "])',
            "canonical JSON-RPC quantity",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            'chainId: "0x01"',
            '.invalidPublicInputs("eth_chainId")',
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            'EthereumMainnetExecutionProvider { _, _ -> "0x01" }',
            "EthereumMainnetInboundEvidence(receipt = receipt)",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            '(method, params) -> "0x01"',
            "leading-zero eth_chainId RPC",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            'new ExecutionProviderStub("0x01", receipt, block)',
            "ValidateExecutionProviderMainnetAsync",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
        (
            "test_collect_receipt_proof_rejects_noncanonical_chain_id_quantity",
            'rpc_response("0x01")',
        ),
    ),
)
ETHEREUM_BEACON_REST_FINALIZED_HEADER_SHAPE_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "/body_root must be 32 bytes/u",
            "/signature must be 96 bytes/u",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        (
            'for field in ["parent_root", "state_root", "body_root"]',
            '"\\(label).data.header.message.\\(field)"',
            '"\\(label).data.header.signature"',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            '("parent_root", String(repeating: "01", count: 32))',
            'invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.data.header.signature")',
            'String(repeating: "12", count: 95)',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        (
            'for (field in listOf("parent_root", "state_root", "body_root"))',
            '"$label.data.header.message.$field"',
            '"$label.data.header.signature"',
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            '"parent_root" to "01"',
            '"body_root" to "03"',
            '"12".repeat(95)',
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        (
            'Arrays.asList("parent_root", "state_root", "body_root")',
            'label + ".data.header.message." + field',
            'label + ".data.header.signature"',
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            '{"parent_root", "01"}',
            "repeat(\"12\", 95)",
            "Beacon REST provider must reject malformed finalized header signatures",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            'foreach (var field in new[] { "parent_root", "state_root", "body_root" })',
            '"{label}.data.header.message.{field}"',
            '"{label}.data.header.signature"',
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            '("parent_root", "01")',
            'string.Concat(Enumerable.Repeat("12", 95))',
            'Assert.Contains("signature", malformedSignature.Message)',
        ),
    ),
)
ETHEREUM_BEACON_REST_EXECUTION_PAYLOAD_BINDING_MARKERS = (
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "ethereumMainnetBeaconRestBlockIdForTarget",
            "/eth/v1/beacon/headers/${encodeURIComponent(targetBlockId.id)}",
            "/eth/v1/beacon/blocks/${encodeURIComponent(targetBlockId.id)}/root",
            "/eth/v2/beacon/blocks/${encodeURIComponent(targetBlockId.id)}",
            "execution_payload",
            "const executionBlockHash = requireEthereumRpcHexData(",
            "const executionReceiptsRoot = requireEthereumRpcHexData(",
            "const finalizedBlockRoot = requireEthereumRpcHexData(",
            "const finalizedCheckpointRoot = requireEthereumRpcHexData(",
            "const syncCommitteeRoot = requireEthereumRpcHexData(",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumMainnetBeaconRestFinalityUpdateSummary",
            "normalizeEthereumMainnetFinalityBranch",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "must contain 6 siblings",
            "must contain at least one participant",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "ethereumMainnetBeaconRestBlockIdForTarget",
            "/eth/v1/beacon/headers/${encodeURIComponent(targetBlockId.id)}",
            "/eth/v1/beacon/blocks/${encodeURIComponent(targetBlockId.id)}/root",
            "/eth/v2/beacon/blocks/${encodeURIComponent(targetBlockId.id)}",
            "execution_payload",
            "const executionBlockHash = requireEthereumRpcHexData(",
            "const executionReceiptsRoot = requireEthereumRpcHexData(",
            "const finalizedBlockRoot = requireEthereumRpcHexData(",
            "const finalizedCheckpointRoot = requireEthereumRpcHexData(",
            "const syncCommitteeRoot = requireEthereumRpcHexData(",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumMainnetBeaconRestFinalityUpdateSummary",
            "normalizeEthereumMainnetFinalityBranch",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "must contain 6 siblings",
            "must contain at least one participant",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        (
            "/eth/v1/beacon/genesis",
            "/eth/v1/beacon/headers/64",
            "/eth/v1/beacon/blocks/64/root",
            "/eth/v2/beacon/blocks/64",
            "/eth/v1/beacon/light_client/finality_update",
            "validFinalityUpdate",
            "SAMPLE_FINALITY_BRANCH",
            "assert.deepEqual(evidence.beaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH)",
            "syncCommitteeParticipation",
            "/sync_committee_bits must contain at least one participant/u",
            "/finality_branch is required/u",
            "/finality_branch must contain 6 siblings/u",
            "/finalizedHeaderRoot must not be zero/u",
            "/finalizedBlockRoot must not be zero/u",
            "/finalizedCheckpointRoot must not be zero/u",
            "/syncCommitteeRoot must not be zero/u",
            "/requires beaconSlot, beaconBlockRoot, or block\\.timestamp/u",
            "/finalized target header must be finalized/u",
            "/historical target blocks require an ancestry proof/u",
            "/beaconFinality\\.executionBlockHash must not be zero/u",
            "/beaconFinality\\.executionReceiptsRoot must not be zero/u",
            "/beaconFinality\\.finalizedHeaderRoot must not be zero/u",
            "/beaconFinality\\.syncCommitteeRoot must not be zero/u",
            "/finalized block root must match finalized header root/u",
            "/execution payload block_hash must match block.hash/u",
            "/execution payload block_number must match block.number/u",
            "/execution payload receipts_root must match block.receiptsRoot/u",
        ),
    ),
    (
        "javascript/iroha_js/index.d.ts",
        (
            "syncCommitteeBits?: string;",
            "syncCommitteeSignature?: string;",
            "syncSignatureSlot?: string | number | bigint;",
            "signatureSlot?: string | number | bigint;",
            "finalityBranch?: readonly string[];",
            "finality_branch?: readonly string[];",
            "syncCommitteeParticipation?: string | number | bigint;",
            "readonly finalityBranch?: readonly string[];",
            "readonly syncCommitteeBits?: string;",
            "readonly syncCommitteeSignature?: string;",
        ),
    ),
    (
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "syncCommitteeBits\\?: string;",
            "syncCommitteeSignature\\?: string;",
            "syncSignatureSlot\\?: string \\| number \\| bigint;",
            "finalityBranch\\?: readonly string\\[\\];",
            "syncCommitteeParticipation\\?: string \\| number \\| bigint;",
            "readonly finalityBranch\\?: readonly string\\[\\];",
            "readonly syncCommitteeBits\\?: string;",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        (
            "beaconRestBlockIdForTarget",
            'path: "/eth/v1/beacon/headers/\\(targetBlockId.id)"',
            'path: "/eth/v1/beacon/blocks/\\(targetBlockId.id)/root"',
            'path: "/eth/v2/beacon/blocks/\\(targetBlockId.id)"',
            'path: "/eth/v1/beacon/light_client/finality_update"',
            "BeaconRestFinalityUpdateSummary",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "normalizeFinalityBranch(",
            '"finalityBranch": finalityUpdate.finalityBranch',
            "syncCommitteeParticipation",
            "public let syncCommitteeBits: String?",
            'value["syncCommitteeBits"] = syncCommitteeBits',
            "strictFirstPresent(",
            "normalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            'invalidPublicInputs("beaconRest.targetHeader.slot")',
            'invalidPublicInputs("beaconRest.targetHeader.finalizedSlot")',
            'invalidPublicInputs("beaconRest.targetHeader.ancestryProof")',
            'invalidPublicInputs("beaconRest.finalizedBlockRoot")',
            'invalidPublicInputs("beaconRest.executionPayload.blockHash")',
            'invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "testEthereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "testEthereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "/eth/v1/beacon/genesis",
            "/eth/v1/beacon/headers/32",
            "/eth/v1/beacon/blocks/64/root",
            "/eth/v2/beacon/blocks/64",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumBeaconFinalityUpdateJson(",
            "ethereumFinalityBranch",
            'XCTAssertEqual(finality["finalityBranch"] as? [String], Self.ethereumFinalityBranch)',
            "includeFinalityBranch: false",
            "finalityBranch: Array(Self.ethereumFinalityBranch.prefix(5))",
            "syncCommitteeParticipation",
            'syncCommitteeBits: "0x01" + String(repeating: "00", count: 63)',
            'conflictingSyncBitsFinality["sync_committee_bits"]',
            '"finalized_header_root", "0x" + String(repeating: "13", count: 32)',
            '.zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")',
            '"timestamp": "0x364"',
            "ethereumBeaconBlockRootJson(",
            "ethereumBeaconBlockJson(",
            'invalidPublicInputs("beaconRest.targetHeader.ancestryProof")',
            'invalidPublicInputs("beaconRest.finalizedBlockRoot")',
            'invalidPublicInputs("beaconRest.executionPayload.blockHash")',
            'invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        (
            "beaconRestBlockIdForTarget",
            '"/eth/v1/beacon/headers/${targetBlockId.id}"',
            '"/eth/v1/beacon/blocks/${targetBlockId.id}/root"',
            '"/eth/v2/beacon/blocks/${targetBlockId.id}"',
            '"/eth/v1/beacon/light_client/finality_update"',
            "ethereumBeaconRestFinalityUpdateSummary",
            "normalizeEthereumBeaconRestFinalityBranch",
            '"finalityBranch" to finalityUpdate.finalityBranch',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "ethereumBeaconRestSyncCommitteeParticipation",
            "val syncCommitteeBits: String? = null",
            'syncCommitteeBits?.let { "syncCommitteeBits" to it }',
            "strictFirstPresent(",
            "execution_payload",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "beaconFinalityUpdateJson(",
            "ethereumFinalityBranch",
            'assertEquals(ethereumFinalityBranch, evidence.beaconFinality?.get("finalityBranch"))',
            "includeFinalityBranch = false",
            "finalityBranch = ethereumFinalityBranch.take(5)",
            "syncCommitteeParticipation",
            "ethereumSyncCommitteeSupermajorityBits",
            '"sync_committee_bits" to ("0x02" + "00".repeat(63))',
            'Triple("finalized_header_root", "0x" + "13".repeat(32), "beaconFinality.finalizedHeaderRoot")',
            "sync_committee_bits must contain at least one participant",
            '"timestamp" to "0x364"',
            "beaconBlockRootJson(",
            "beaconBlockJson(",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        (
            "beaconRestBlockIdForTarget",
            '"/eth/v1/beacon/headers/" + targetBlockId.id',
            '"/eth/v1/beacon/blocks/" + targetBlockId.id + "/root"',
            '"/eth/v2/beacon/blocks/" + targetBlockId.id',
            '"/eth/v1/beacon/light_client/finality_update"',
            "beaconRestFinalityUpdateSummary",
            "normalizeBeaconRestFinalityBranch",
            'evidence.put("finalityBranch", finalityUpdate.finalityBranch)',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "beaconRestSyncCommitteeParticipation",
            "String syncCommitteeBits,",
            'value.put("syncCommitteeBits", syncCommitteeBits)',
            "strictFirstPresent(",
            "normalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "beaconFinalityUpdateJson(",
            "ETHEREUM_FINALITY_BRANCH",
            "Beacon REST provider must reject missing finality branch",
            "Beacon REST provider must reject malformed finality branch",
            "syncCommitteeParticipation",
            "ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS",
            'conflictingSyncBitsFinality.put("sync_committee_bits"',
            "final Object[][] conflictingFinalityAliases",
            "sync_committee_bits must contain at least one participant",
            '"timestamp", "0x364"',
            "beaconBlockRootJson(",
            "beaconBlockJson(",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "BeaconRestBlockIdForTargetAsync",
            '$"/eth/v1/beacon/headers/{targetBlockId.Id}"',
            '$"/eth/v1/beacon/blocks/{targetBlockId.Id}/root"',
            '$"/eth/v2/beacon/blocks/{targetBlockId.Id}"',
            '"/eth/v1/beacon/light_client/finality_update"',
            "BeaconRestFinalityUpdateSummary",
            "NormalizeFinalityBranch(",
            '["finalityBranch"] = finalityUpdate.FinalityBranch',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "SyncCommitteeParticipation",
            "string? SyncCommitteeBits = null",
            'value["syncCommitteeBits"] = SyncCommitteeBits',
            "StrictFirstPresent(",
            "NormalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            "EthExecutionPayloadHeaderRootFromRlp",
            "EthBeaconBodyRootFromExecutionPayloadBranch",
            "EthBeaconBlockHeaderRoot",
            "SszMerkleRootFromBranch(",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "BeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "BeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "BeaconFinalityUpdateJson(",
            "EthereumFinalityBranch",
            'Assert.Equal(EthereumFinalityBranch, Assert.IsAssignableFrom<IReadOnlyList<string>>(evidence.BeaconFinality?["finalityBranch"]))',
            "includeFinalityBranch: false",
            "finalityBranch: EthereumFinalityBranch.Take(5).ToArray()",
            "syncCommitteeParticipation",
            "EthereumSyncCommitteeSupermajorityBits",
            '["sync_committee_bits"] = "0x02" + string.Concat(Enumerable.Repeat("00", 63))',
            "sync_committee_bits must contain at least one participant",
            '["timestamp"] = "0x364"',
            "BeaconBlockRootJson(",
            "BeaconBlockJson(",
            "BeaconExecutionPayloadSszRootsMatchSharedVector",
            "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
            "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
            "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
    ),
)
ETHEREUM_SYNC_COMMITTEE_ROSTER_MARKERS = (
    (
        "crates/iroha_sccp/src/lib.rs",
        (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: usize = 512;",
            ".all(|weight| *weight == 1)",
            "eth_sync_committee_transition_transcript_requires_mainnet_rosters",
            "SCCP_ETH_MAINNET_SYNC_COMMITTEE_SUPERMAJORITY_AUTHORITIES",
        ),
    ),
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpSolanaProver.test.js",
        (
            "syncCommitteeFixture(0x11, 0xaa)",
            "assert.equal(nextSyncCommitteePayload.length, 81925)",
            "signersBitmap(342)",
        ),
    ),
    (
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "syncCommitteePublicKeys = Array.from({ length: 512 }",
            "assert.equal(payload.length, 81925)",
            "ETH sync-committee payload helpers",
        ),
    ),
    (
        "python/iroha_torii_client/sccp.py",
        (
            "_SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[{index}] must be 1 for Ethereum mainnet",
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "sync_committee_fixture(0x11, 0xAA)",
            "assert len(next_payload) == 81925",
            "signers_bitmap(342)",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            "sccpEthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteeWeights[index] == 1",
            'signersBitmap.count == (syncCommitteePublicKeys.count + 7) / 8',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "ethereumSyncCommitteeBytes(_ byte: UInt8, count: Int)",
            "XCTAssertEqual(nextSyncPayload.count, 81_925)",
            "Self.ethereumSyncCommitteeSignersBitmap(342)",
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: Int = 512",
            "syncCommitteeWeights[$index] must be 1 for Ethereum mainnet",
            "signersBitmap.size == (syncCommitteePublicKeys.size + 7) / 8",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashesTest.kt",
        (
            "List(512) { index ->",
            "assertEquals(81925, nextSyncPayload.size)",
            "syncCommitteeSignersBitmap(342)",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "indexedSyncCommitteeBytes(0x11, 48, index)",
            'syncCommitteeWeights = List(512) { "1" }',
            "syncCommitteeRoot must match syncCommitteePayload",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "must be 1 for Ethereum mainnet",
            "(syncCommitteePublicKeys.size() + 7) / 8",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/SourceSccpProofsTests.java",
        (
            "syncCommitteeBytes(0x11, 48)",
            "nextSyncPayload.length == 81925",
            "syncCommitteeSignersBitmap(342)",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "for (int index = 0; index < 512; index++)",
            "indexedSyncCommitteeBytes(0x11, 48, index)",
            "syncCommitteeRoot must match syncCommitteePayload",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "EthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteePayload must contain exactly",
            "must be 1 for Ethereum mainnet",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "Assert.Equal(81925, syncCommitteePayload.Length)",
            "CompressedSyncCommitteePayload()",
            "WeightedSyncCommitteePayload()",
        ),
    ),
)
ETHEREUM_SOURCE_BRIDGE_CONFIG_MARKERS = (
    (
        "scripts/sccp_eth_source_bridge_evidence.py",
        (
            "def eth_source_bridge_config_hash(",
            "source_bridge_network_id must be Ethereum mainnet chain id 1",
            "ETH_SOURCE_BRIDGE_CONFIG_PREFIX",
        ),
    ),
    (
        "scripts/sccp_all_lanes_evidence.py",
        (
            "def _check_eth_source_bridge_config_hash(",
            "source_bridge_config_hash does not match ETH bridge address",
        ),
    ),
    (
        "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
        (
            "test_eth_source_bridge_config_hash_binds_mainnet_lane_and_code_hash",
            "invalid ETH source bridge config hash input was accepted",
        ),
    ),
    (
        "javascript/iroha_js/src/sccp.js",
        (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
    ),
    (
        "javascript/iroha_js/dist/sccp.js",
        (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
    ),
    (
        "javascript/iroha_js/test/sccpSolanaProver.test.js",
        (
            "sourceBridgeNetworkId must be Ethereum mainnet chain id",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
    ),
    (
        "IrohaSwift/Sources/IrohaSwift/SccpSourceProofHashes.swift",
        (
            "ethSourceBridgeConfigHash(",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "sourceBridgeNetworkId",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
    ),
    (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashes.kt",
        (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/SourceSccpProofHashesTest.kt",
        (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
    ),
    (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/SourceSccpProofs.java",
        (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/SourceSccpProofsTests.java",
        (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
    ),
    (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        (
            "SourceBridgeConfigHash must match the Ethereum mainnet source bridge config fields.",
            "NormalizeEthereumMainnetNetworkId(input.NetworkId)",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs",
        (
            "ExpectedSourceBridgeConfigHash",
            "SourceBridgeConfigHash = \"0x\" + new string('9', 64)",
        ),
    ),
)
ETHEREUM_EVM_SOURCE_LIVE_PRODUCTION_MARKERS = (
    (
        "scripts/sccp_evm_source_live_evidence.py",
        (
            'return "finalized" if domain == SCCP_DOMAIN_ETH else "latest"',
            "eth_chainId for {chain} lane must be canonical mainnet chain id",
            "deployment transaction receipt status must be 0x1",
            "deployment receipt contractAddress does not match source bridge",
            "deployment transaction hash does not match requested deployment transaction",
            "deployment transaction to must be null for contract creation",
            "deployment transaction input must not be empty or zero",
            "deployment receipt blockHash does not match eth_getBlockByNumber",
            "eth_getBlockByNumber receiptsRoot",
            "source bridge code hash at deployment receipt block does not",
            "source bridge runtime bytecode at deployment receipt block does",
            "deployment receipt block is newer than the finalized execution block",
            "deployment receipt block hash does not match the finalized execution block",
            "source bridge runtime bytecode hash must match bridge_code_hash",
            "deployment receipt block receiptsRoot metadata must be verified",
            "Ethereum source deployment receipt block finality metadata must be verified",
            "source verifier material hash metadata must match canonical inputs",
            "source adapter engine deployment hash metadata must match canonical inputs",
            "expected source verifier material hash argument must match ",
            "expected source adapter engine deployment hash argument must match ",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_source_live_evidence_test.py",
        (
            "test_evm_source_live_evidence_rejects_rpc_and_code_hash_drift",
            "test_evm_source_live_rejects_deployment_transaction_readback_drift",
            "test_evm_source_live_rejects_missing_or_drifted_receipt_contract_address",
            "test_evm_source_live_rejects_receipt_block_hash_drift",
            "test_evm_source_live_rejects_receipt_block_number_drift",
            "test_evm_source_live_rejects_unfinalized_deployment_receipt_block",
            "test_evm_source_live_rejects_finalized_deployment_receipt_hash_drift",
            "test_evm_source_live_rejects_zero_receipt_block_receipts_root",
            "test_evm_source_live_rejects_receipt_block_code_hash_drift",
            "test_evm_source_live_toml_revalidates_imported_summary_metadata",
            "test_evm_source_live_toml_requires_independent_pins",
        ),
    ),
)
ETHEREUM_EVM_LIVE_DESTINATION_PRODUCTION_MARKERS = (
    (
        "scripts/sccp_evm_live_evidence.py",
        (
            "eth_chainId for {chain} lane must be canonical mainnet chain id",
            "verifierCodeHash() does not match eth_getCode runtime bytecode",
            "verifierKeyHash() does not match verifier verifyingKeyHash()",
            "destinationBindingHash() does not match canonical live deployment inputs",
            "bridge runtime bytecode hash must match bridge_code_hash",
            "verifier runtime bytecode hash must match verifier_code_hash",
            "verifier key hash metadata must match verifyingKeyHash",
            "destination binding hash metadata must match canonical live inputs",
            "destination binding key metadata must match canonical inputs",
            "route-canary MessageProofAccepted destinationBindingHash does not",
            "route-canary MessageProofAccepted verifierBackendHash does not",
            "route-canary MessageProofAccepted proofFamilyHash does not match",
            "route-canary MessageProofAccepted networkId does not match networkId()",
            "route-canary transaction calldata must call",
            "submitSccpMessageProof(bytes,bytes32[6],bytes32)",
            "route-canary proofBytes must be a 384-byte Groth16 tuple",
            "route-canary proofBytes must not be all zero",
            "route-canary proof version must be 1",
            "route-canary proof sourceDomain does not match expectedSourceDomain()",
            "usedMessageProofs(bytes32) is false",
            'and transaction.get("message_proof_used") is True',
        ),
    ),
    (
        "pytests/scripts/sccp_evm_live_evidence_test.py",
        (
            "test_live_evm_evidence_rejects_verifier_code_hash_drift",
            "test_live_evm_evidence_rejects_bridge_code_hash_drift",
            "test_live_evm_evidence_rejects_bridge_destination_binding_drift",
            "test_live_evm_full_toml_revalidates_imported_summary_metadata",
            "test_live_evm_route_canary_rejects_unverified_transaction_metadata",
            "route_canary_call_data_mutator",
            "proofBytes offset must be 256 bytes",
            "publicInputs[0] must match event messageId",
            "targetDomain does not match expectedTargetDomain()",
            "publicInputs[3] must match event commitmentRoot",
            "statementHash must match accepted event",
            "proofBytes must be a 384-byte Groth16 tuple",
            "proofBytes must not be all zero",
            "proof version must be 1",
            "proof sourceDomain does not match expectedSourceDomain()",
            "usedMessageProofs(bytes32) is false",
        ),
    ),
)
ETHEREUM_ROUTE_CANARY_FINALIZED_RECEIPT_BLOCK_MARKERS = (
    (
        "scripts/sccp_evm_live_evidence.py",
        (
            "def _route_canary_finalized_block_summary(",
            '"eth_getBlockByNumber"',
            '["finalized", False]',
            "route-canary receipt block is newer than the finalized execution block",
            "route-canary receipt block hash does not match the finalized execution block",
            '"receipt_block_finalized": True',
            'and transaction.get("receipt_block_finalized") is True',
            'route_canary_transaction.get("receipt_block_finalized") is True',
            'receipt_block_finalized=finalized_block["receipt_block_finalized"]',
        ),
    ),
    (
        "scripts/sccp_evm_destination_evidence.py",
        (
            'EVM_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:evm-route-canary-evidence:v4"',
            "receipt_block_finalized: bool",
            "receipt_block_finalized must be a boolean for EVM route canaries",
            'receipt_block_finalized=values["receipt_block_finalized"]',
            "route_canary_receipt_block_finalized",
            "--route-canary-receipt-block-finalized",
            "from finalized live reads",
            "evm_route_canary_receipt_block_finalized",
        ),
    ),
    (
        "scripts/sccp_all_lanes_evidence.py",
        (
            "evm_route_canary_receipt_block_finalized",
            "_comment_evm_route_canary_receipt_block_finalized",
            "EVM route canary receipt block finalized metadata must be true",
            "receipt_block_finalized=receipt_block_finalized",
            'canary["receipt_block_finalized"] = True',
        ),
    ),
    (
        "pytests/scripts/sccp_evm_live_evidence_test.py",
        (
            "route_canary_finalized_block_number",
            'params[0] == "finalized"',
            '"receipt_block_finalized"] is True',
            '"receipt_block_finalized"] is False',
            "evm_route_canary_receipt_block_finalized = true",
            "receipt_block_finalized=True",
            "receipt_block_finalized=False",
            "transaction to does not match destination bridge",
            'block_tag="finalized" if finality_expected else "latest"',
            "test_live_evm_bsc_default_latest_route_canary_stays_diagnostic",
            "receipt block is newer than the finalized execution block",
            "receipt block hash does not match the finalized execution block",
        ),
    ),
    (
        "pytests/scripts/sccp_all_lanes_evidence_test.py",
        (
            "test_all_lanes_rejects_evm_route_canary_missing_finalized_receipt_state",
            "_comment_evm_route_canary_receipt_block_finalized",
            "receipt_block_finalized=True",
            "receipt block finalized metadata must be true",
        ),
    ),
    (
        "crates/iroha_sccp/src/lib.rs",
        (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            'b"iroha:sccp:evm-route-canary-evidence:v4"',
            "push_u8(&mut out, u8::from(receipt_block_finalized));",
            "|| !receipt_block_finalized",
            "allowlist.evm_route_canary_receipt_block_finalized = Some(true);",
            "non-finalized diagnostic EVM route canary hash",
            "evm_route_canary_evidence_hash_matches_destination_script_vector",
            "84b93b0050b6bc9696ba55d56a8c957171e6a4ebd2f242b683762d52d88db9d7",
        ),
    ),
    (
        "crates/iroha_config/src/parameters/user.rs",
        (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            "evm_route_canary_receipt_block_finalized: self.evm_route_canary_receipt_block_finalized",
        ),
    ),
    (
        "crates/iroha_core/src/smartcontracts/isi/world.rs",
        (
            "evm_route_canary_receipt_block_finalized: configured",
            "configured_sccp_all_lanes_launch_rejects_evm_non_finalized_route_canary",
        ),
    ),
)
ETHEREUM_EVM_BLOCK_TAG_METADATA_MARKERS = (
    (
        "scripts/sccp_evm_source_live_evidence.py",
        (
            'sccp_evm_source_block_tag = "',
            "--block-tag finalized",
        ),
    ),
    (
        "scripts/sccp_evm_live_evidence.py",
        (
            'sccp_evm_block_tag = "',
            "--block-tag finalized",
        ),
    ),
    (
        "scripts/sccp_eth_source_bridge_evidence.py",
        (
            'sccp_evm_source_block_tag = "',
            "Ethereum source TOML requires --block-tag finalized",
        ),
    ),
    (
        "scripts/sccp_evm_destination_evidence.py",
        (
            'sccp_evm_block_tag = "',
            "Ethereum destination TOML requires --block-tag finalized",
        ),
    ),
    (
        "scripts/sccp_bsc_source_bridge_evidence.py",
        (
            'sccp_evm_source_block_tag = "',
            '"latest"',
        ),
    ),
    (
        "scripts/sccp_all_lanes_evidence.py",
        (
            '"sccp_evm_source_rpc_chain_id": "_comment_evm_source_rpc_chain_id"',
            '"sccp_evm_source_block_tag": "_comment_evm_source_block_tag"',
            '"sccp_evm_rpc_chain_id": "_comment_evm_rpc_chain_id"',
            '"sccp_evm_block_tag": "_comment_evm_block_tag"',
            "EVM source live RPC chain-id must be canonical for {profile.chain}",
            "EVM live RPC chain-id must be canonical for {profile.chain}",
            "Ethereum source live block-tag metadata must be finalized",
            "Ethereum destination live block-tag metadata must be finalized",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_source_live_evidence_test.py",
        (
            "test_evm_source_live_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_source_block_tag = "finalized"',
        ),
    ),
    (
        "pytests/scripts/sccp_evm_live_evidence_test.py",
        (
            "test_live_evm_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_block_tag = "finalized"',
        ),
    ),
    (
        "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
        (
            "test_eth_source_toml_rejects_nonfinalized_block_tag",
            "Ethereum source TOML requires --block-tag finalized",
        ),
    ),
    (
        "pytests/scripts/sccp_evm_destination_evidence_test.py",
        (
            "test_evm_destination_eth_toml_rejects_nonfinalized_block_tag",
            "Ethereum destination TOML requires --block-tag finalized",
        ),
    ),
    (
        "pytests/scripts/sccp_all_lanes_evidence_test.py",
        (
            "test_all_lanes_rejects_ethereum_nonfinalized_evm_live_metadata",
            '# sccp_evm_source_block_tag = "finalized"',
            '# sccp_evm_block_tag = "finalized"',
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_evm_evidence_keeps_block_tag_metadata_guards",
            "Ethereum production evidence must keep finalized block-tag tripwires",
        ),
    ),
)
ETHEREUM_EVM_SOURCE_ADAPTER_DEPLOYMENT_GATE_MARKERS = (
    (
        "crates/iroha_sccp/src/lib.rs",
        (
            "fn sccp_evm_source_adapter_deployment_unblocks_production_for_domain(",
            "deployment.source_bridge_network_id == material.source_bridge_network_id",
            "deployment.source_bridge_config_hash == material.source_bridge_config_hash",
            "sccp_source_bridge_config_hash_is_production_ready(material)",
            "wrong_network_deployment.source_bridge_network_id = sccp_bsc_mainnet_network_id_word_v1();",
            "wrong_config_deployment.source_bridge_config_hash[0] ^= 0x01;",
            "wrong_emitter_deployment.source_bridge_emitter_address = [0x99; 20].to_vec();",
        ),
    ),
)
ETHEREUM_LAUNCH_POLICY_SELECTOR_MARKERS = (
    (
        "crates/iroha_sccp/src/lib.rs",
        (
            "fn sccp_lane_production_ready_under_launch_policy_v1(",
            "SccpLaunchModeV1::EthereumMainnetLane => domain == SCCP_DOMAIN_ETH",
            "fn ethereum_launch_policy_opens_only_eth_lane_independently_of_all_lanes()",
            "EthereumMainnetLane must let production-ready ETH open before all lanes are ready",
            "EthereumMainnetLane must not open BSC even when BSC-shaped components are ready",
            "EthereumMainnetLane must still fail closed when ETH evidence is incomplete",
            "AllLanesAtOnce must continue to wait for every advertised lane",
            "BscMainnetLane must not open ETH",
        ),
    ),
)
ETHEREUM_LAUNCH_POLICY_DOCUMENTATION_MARKERS = (
    (
        "docs/source/bridge_proofs.md",
        (
            "active launch policy is Ethereum-mainnet lane readiness",
            "mainnet source-proof, source-adapter deployment",
            "Non-Ethereum lanes remain",
            "fail-closed until their own launch policy opens",
            "active Ethereum launch lane",
            "with the first-release Ethereum-mainnet launch policy",
        ),
    ),
)
ETHEREUM_LAUNCH_POLICY_DOCUMENTATION_FORBIDDEN_MARKERS = (
    "active launch policy is BSC-mainnet lane readiness",
    "active BSC launch lane",
    "with the first-release BSC-mainnet launch policy",
)
ETHEREUM_CORE_RANGE_FINALITY_BINDING_MARKERS = (
    (
        "crates/iroha_core/src/smartcontracts/isi/world.rs",
        (
            "fn validate_sccp_bridge_proof_range_matches_artifact(",
            "artifact.public_inputs.finality_height",
            "proof.range.start_height == finality_height && proof.range.end_height == finality_height",
            "SCCP message proof range must match finality height",
            "validate_sccp_bridge_proof_range_matches_artifact(proof, &artifact)?;",
        ),
    ),
    (
        "crates/iroha_core/tests/bridge_proofs.rs",
        (
            "fn submit_configured_eth_source_adapter_proof_rejects_outer_range_replay_after_ethereum_lane_launch",
            "proof.range = BridgeProofRange",
            "ETH source proofs must bind the outer range to artifact finality",
            "SCCP message proof range must match finality height",
        ),
    ),
)
ETHEREUM_CORE_MESSAGE_REPLAY_GUARD_MARKERS = (
    (
        "crates/iroha_core/src/smartcontracts/isi/world.rs",
        (
            "struct SccpMessageKey",
            "fn sccp_message_key_from_bridge_proof(",
            "fn find_existing_sccp_message_proof(",
            "rec.status != iroha_data_model::proof::ProofStatus::Verified",
            "if !bridge.proof.pinned",
            "SCCP message proof records must be pinned for replay protection",
            "SCCP message proof replays existing message proof",
            "is_some_and(|bridge| bridge.proof.pinned)",
        ),
    ),
    (
        "crates/iroha_core/tests/bridge_proofs.rs",
        (
            "fn manual_prune_keeps_pinned_bridge_proofs",
            "fn submit_configured_eth_source_adapter_proof_rejects_unpinned_message_after_ethereum_lane_launch",
            "fn submit_configured_eth_source_adapter_proof_rejects_message_id_replay_after_ethereum_lane_launch",
            "fn submit_configured_eth_source_adapter_proof_ignores_rejected_history_after_ethereum_lane_launch",
            "fn submit_configured_eth_source_adapter_proof_ignores_unpinned_history_after_ethereum_lane_launch",
            "fn submit_configured_eth_source_adapter_proof_ignores_malformed_history_after_ethereum_lane_launch",
            "manual pruning must not remove pinned bridge records",
            "ETH source proofs must be pinned for durable replay protection",
            "ETH source proofs must reject replayed SCCP message ids",
            "ETH source proofs must ignore non-canonical SCCP message history",
        ),
    ),
)
ETHEREUM_TORII_PINNED_MESSAGE_PROOF_MARKERS = (
    (
        "crates/iroha_torii/src/routing.rs",
        (
            "fn bridge_proof_from_sccp_message_bundle(",
            "pinned: true",
            "if !bridge.proof.pinned",
            "SCCP message bridge proofs must be pinned for core replay protection",
            "unpinned SCCP message records must not be served as source-chain envelopes",
            "bridge_proof_from_sccp_message_bundle_builds_taira_tron_xor_diagnostic_when_allowed",
            "verified_bridge_record_extracts_non_sora_message_bundle_candidate",
        ),
    ),
)
SCCP_UNREADY_TRANSPARENT_PROOF_CONFIG_MARKERS = (
    (
        "crates/iroha_config/src/parameters/user.rs",
        (
            "pub sccp_allow_unready_transparent_proofs: bool",
            "sccp_allow_unready_transparent_proofs: self.sccp_allow_unready_transparent_proofs",
        ),
    ),
    (
        "configs/soranexus/taira/bootstrap_kaigi_localnet.sh",
        (
            "sccp_allow_unready_transparent_proofs = true",
        ),
    ),
    (
        "configs/soranexus/taira/config.toml",
        (
            "sccp_allow_unready_transparent_proofs = false",
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "def test_release_readiness_sccp_allow_unready_transparent_proofs_is_config_only",
            "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS",
        ),
    ),
)
SCCP_UNREADY_TRANSPARENT_PROOF_FORBIDDEN_ENV_PATHS = (
    "crates/iroha_config/src/parameters/user.rs",
    "configs/soranexus/taira/taira-irohad.service",
    "configs/soranexus/taira/bootstrap_kaigi_localnet.sh",
)
SCCP_UNREADY_TRANSPARENT_PROOF_FORBIDDEN_ENV = (
    "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS"
)
CONTRACT_SMOKE_ETH_MAINNET_NETWORK_ID_MARKERS = (
    (
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        (
            "const ethMainnetNetworkId = ethers.zeroPadValue(ethers.toBeHex(1), 32);",
            "const bscMainnetNetworkId = ethers.zeroPadValue(ethers.toBeHex(56), 32);",
            'callExceptionWithReason("Network id must be ETH mainnet")',
            'callExceptionWithReason("Network id must be BSC mainnet or testnet")',
            "networkId = ethMainnetNetworkId",
            "const networkId = ethMainnetNetworkId;",
            "assert.equal(acceptedGroth16Logs[0].args.networkId, networkId);",
        ),
    ),
)
CONTRACT_SMOKE_EVM_PRODUCTION_SURFACE_MARKERS = (
    (
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        (
            'entry.name === "MessageProofAccepted"',
            '"destinationBindingHash"',
            '"verifierBackendHash"',
            '"proofFamilyHash"',
            'callExceptionWithReason("Verifier code hash mismatch")',
            'callExceptionWithReason("Verifier key hash is required")',
            'callExceptionWithReason("Verifier key hash mismatch")',
            "assert.equal(await groth16Bridge.verifierCodeHash(), groth16VerifierCodeHash);",
            "assert.equal(await groth16Bridge.verifierKeyHash(), groth16VerifierKeyHash);",
            'callExceptionWithReason("Destination binding hash is required")',
            'callExceptionWithReason("Unexpected Groth16 proof length")',
            'callExceptionWithReason("G2 point is zero")',
            'callExceptionWithReason("G1 point is zero")',
            'callExceptionWithReason("Groth16 proof verification failed")',
            "await groth16Bridge.destinationBindingHash()",
            "assert.equal(acceptedGroth16Logs.length, 1);",
            "assert.equal(acceptedGroth16Logs[0].args.messageId, messageId);",
            "acceptedGroth16Logs[0].args.destinationBindingHash,",
            "acceptedGroth16Logs[0].args.verifierBackendHash,",
            "acceptedGroth16Logs[0].args.proofFamilyHash,",
            "assert.equal(acceptedGroth16Logs[0].args.networkId, networkId);",
            "assert.equal(await groth16Bridge.usedMessageProofs(messageId), true);",
            'callExceptionWithReason("Message proof already used")',
        ),
    ),
)
NATIVE_SCCP_NO_WASM_READINESS_TEST_MARKERS = (
    (
        "scripts/sccp_release_readiness_report.py",
        (
            "NATIVE_EVM_PROVER_FORBIDDEN_PAYLOAD_MARKERS = (",
            "class DuplicateJsonKeyError",
            "object_pairs_hook=_reject_duplicate_json_keys",
            "native EVM Groth16 prover bundle JSON contains duplicate key",
            "def _native_evm_prover_forbidden_payload_blockers(",
            "_native_evm_prover_forbidden_payload_blockers(artifact_path, label)",
            "def _native_evm_prover_hash_role_blockers(",
            "must not be empty",
            "must not duplicate",
            "must not reuse",
            "canonical non-zero 32-byte hex value",
            'b"snarkjs"',
            'b"remoteprover"',
        ),
    ),
    (
        "scripts/sccp_release_bundle.py",
        (
            "class DuplicateJsonKeyError",
            "object_pairs_hook=_reject_duplicate_json_keys",
            "native EVM Groth16 prover bundle JSON contains duplicate key",
            "def _native_evm_prover_payload_sources(",
        ),
    ),
    (
        "pytests/scripts/sccp_release_readiness_report_test.py",
        (
            "BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS = {",
            "NATIVE_LOCAL_PROVER_SOURCE_GLOBS = {",
            "NATIVE_EVM_PROVER_BUNDLE_PARSER_MARKERS = {",
            "NATIVE_EVM_PROVER_ARTIFACT_VERIFIER_MARKERS = {",
            "rejectDuplicateJsonObjectKeys",
            "nativeProverBundle.duplicateJsonKey",
            "nativeProverBundle JSON is invalid",
            "Duplicate JSON object key",
            "normalizeCanonicalNativeEvmProverBundleHex32",
            "requireEthereumMainnetNativeEvmProverBundleHashRoleSeparation",
            "requireNativeEvmProverBundleKnownFields",
            "evmNormalizeNativeEvmProverBundleHex32",
            "evmRequireNativeEvmProverBundleHashRoleSeparation",
            "requireManifestKeys",
            "normalizeNativeEvmProverBundleHex32",
            "requireNativeEvmProverBundleHashRoleSeparation",
            "NormalizeNativeEvmProverBundleHex32",
            "RequireNativeEvmProverBundleHashRoleSeparation",
            "RequireManifestKeys",
            "canonical lowercase 0x-prefixed 32-byte hex",
            "hashes must be role-separated",
            "contains duplicate JSON key",
            "contains unknown field",
            "must not use multiple aliases",
            "isCanonicalDecimalText",
            "canonical decimal integer",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "nativeProverBundle.verifierKeyHash must match destinationBinding",
            "submission requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
            "BuildEthereumCalldataUnchecked",
            "requireEthereumMainnetVerifiedNativeEvmProverArtifactsForProofResult",
            '"javascript/iroha_js/dist/sccp.js"',
            '"javascript/iroha_js/dist/index.js"',
            '"javascript/iroha_js/index.d.ts"',
            "def test_release_readiness_bsc_sdk_sources_are_native_local_prover_only",
            "def test_release_readiness_ethereum_sdk_sources_are_native_local_prover_only",
            "def test_release_readiness_all_public_sccp_sdk_sources_are_native_local_prover_only",
            "def test_release_readiness_native_evm_prover_bundle_manifest_parsers_are_sdk_owned",
            "def test_release_readiness_native_evm_prover_artifact_verifiers_are_sdk_owned",
            "def test_release_readiness_report_blocks_duplicate_native_evm_prover_json_keys",
            "def test_release_readiness_report_blocks_empty_native_evm_prover_payload",
            "def test_release_readiness_report_blocks_reused_native_evm_prover_role_hash",
            "def test_release_readiness_report_blocks_noncanonical_native_evm_prover_hash",
            "def test_release_readiness_report_blocks_reused_native_evm_prover_audit_hash",
            "def test_release_readiness_report_blocks_native_evm_prover_forbidden_payload_marker",
            "def test_release_readiness_native_local_prover_guard_covers_identifier_variants",
            '"remoteProver"',
            '"remote prover"',
            '"remote_prover"',
            '"remote-prover"',
            '"proverEndpoint"',
            '"prover_endpoint"',
        ),
    ),
    (
        "pytests/scripts/sccp_release_bundle_test.py",
        (
            "def test_release_bundle_rejects_duplicate_native_evm_prover_json_keys",
            "native EVM Groth16 prover bundle JSON contains duplicate key: bundle_id",
        ),
    ),
    (
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "function assertBrowserMainnetSccpArtifactsStayJsOnlyAndLocalProverOwned()",
            '"dist/sccp.js": DIST_SCCP_TEXT',
            '"dist/index.js": DIST_INDEX_TEXT',
            '"index.d.ts": DECLARATIONS_TEXT',
            "browser SCCP no-WASM guard catches remote-prover identifier variants",
            "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned",
            "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned",
            "parseEthereumMainnetNativeEvmProverBundleManifest(JSON.stringify(nativeProverBundle)",
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "WebAssembly.compile(bytes)",
            "import './proof.wasm'",
            "fallback remote prover",
            "const proverEndpoint = endpoint",
        ),
    ),
    (
        "pytests/scripts/sccp_release_bundle_test.py",
        (
            "def test_release_bundle_rejects_empty_native_evm_prover_payload",
            "def test_release_bundle_rejects_native_evm_prover_forbidden_payload_marker",
            "def test_release_bundle_verifier_rejects_empty_native_evm_prover_payload",
            "def test_release_bundle_verifier_rejects_reused_native_evm_prover_role_hash",
            "def test_release_bundle_verifier_rejects_noncanonical_native_evm_prover_hash",
            "def test_release_bundle_verifier_rejects_reused_native_evm_prover_audit_hash",
            "def test_release_bundle_verifier_rejects_native_evm_prover_forbidden_payload_marker",
            "native proof artifact imports proof.wasm",
        ),
    ),
)
ETHEREUM_DATA_COLLECTION_FORBIDDEN_PATTERNS = {
    "Torii": re.compile(r"\bTorii\b"),
    "torii": re.compile(r"\btorii\b"),
    "proxy": re.compile(r"\bproxy\b", re.IGNORECASE),
    "embedded HTTP client": re.compile(
        r"\b(fetch|XMLHttpRequest|requests|URLSession|HttpURLConnection|HttpClient)\b"
    ),
}
ETHEREUM_DATA_COLLECTION_REGIONS = {
    "js-sdk": (
        "javascript/iroha_js/src/sccp.js",
        "  async validateExecutionProviderMainnet",
        "  async submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "js-dist": (
        "javascript/iroha_js/dist/sccp.js",
        "  async validateExecutionProviderMainnet",
        "  async submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "python-sdk": (
        "python/iroha_torii_client/sccp.py",
        "    async def validate_execution_provider_mainnet",
        "    async def submit_inbound_to_iroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "_evm_facade_collect_finality",
        ),
    ),
    "swift-sdk": (
        "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
        "    public func validateExecutionProviderMainnet",
        "    public func submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "kotlin-sdk": (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
        "    fun validateExecutionProviderMainnet",
        "    fun submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "java-android": (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/EthereumMainnetSccp.java",
        "  public Object validateExecutionProviderMainnet()",
        "  public Object submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "dotnet-sdk": (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/EthereumMainnetSccp.cs",
        "    public static async ValueTask<object?> ValidateExecutionProviderMainnetAsync",
        "    public static async ValueTask<object?> SubmitInboundToIrohaAsync",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "CollectFinalityEvidenceAsync",
        ),
    ),
}
BSC_INBOUND_ADVERSARIAL_SDK_TEST_MARKERS = (
    (
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
        (
            "BscMainnetSccp requires full receipt proof evidence before inbound proving",
            "calledWithHashOnly",
            "/requires receiptProof/u",
            "callbackEvidence.receiptProof.blockHash",
            "callbackEvidence.sourceEventDigest",
            "/requires receipt source event validation/u",
            "/receiptProof\\.sourceEventDigest must match receipt source event/u",
            "malformedSourceLogCases",
            "SCCP source event log must contain exactly 2 topics",
            "missingTransactionHashLog",
        ),
    ),
    (
        "python/iroha_torii_client/tests/sccp_test.py",
        (
            "called_with_hash_only",
            'match="requires receiptProof"',
            'evidence["receipt_proof"]["block_hash"]',
            'receipt_proof_evidence["receipt_proof"]["block_hash"]',
            'evidence["source_event_digest"]',
            "called_without_source_event",
            'match="requires receipt source event validation"',
            'match="receiptProof.sourceEventDigest"',
            "malformed_source_log_cases",
            "exactly 2 topics",
            "missing_transaction_hash_log",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
        (
            "BscMainnetReceiptProof(",
            "calledWithHashOnly",
            'hashOnly.message?.contains("receiptProof")',
            "assertEquals(receiptProofHash, evidence.receiptProofHash)",
            "calledWithoutSourceEvent",
            'noSourceEvent.message?.contains("receipt source event validation")',
            'driftedSourceEvent.message?.contains("receiptProof.sourceEventDigest")',
            "extraTopicBscSourceLog",
            "nonEmptyDataBscSourceLog",
            "missingBscSourceContextLog",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
        (
            "BscMainnetReceiptProof(",
            "calledWithHashOnly",
            "XCTAssertFalse(calledWithHashOnly)",
            "XCTAssertEqual(evidence.receiptProofHash, receiptProofHash)",
            "missingSourceEventCallbackCalled",
            '.invalidPublicInputs("receipt.sourceEvent")',
            '.invalidPublicInputs("receiptProof.sourceEventDigest")',
            "extraTopicBscSourceReceipt",
            "nonEmptyDataBscSourceReceipt",
            "missingBscSourceContextLog",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
        (
            "BscMainnetSccp.ReceiptProof",
            "calledWithHashOnly",
            "BSC inbound proving must reject hash-only receipt proof evidence",
            "receiptProofHash.equals(evidence.receiptProofHash())",
            "calledWithoutSourceEvent",
            "receipt source event validation",
            "receiptProof.sourceEventDigest",
            "sourceEventDigest.equals(evidence.sourceEventDigest())",
            "extraTopicBscSourceLog",
            "nonEmptyDataBscSourceLog",
            "missingBscSourceContextLog",
        ),
    ),
    (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpBscMainnetTests.cs",
        (
            "BscMainnetReceiptProof",
            "hashOnlyProver.Calls",
            "BscSccpReceiptProofHash",
            "Assert.Equal(0, hashOnlyProver.Calls)",
            "Assert.Equal(receiptProofHash, evidence.ReceiptProofHash)",
            "Assert.Equal(sourceEventDigest, evidence.SourceEventDigest)",
            "Assert.Equal(0, noSourceEventProver.Calls)",
            "Assert.Equal(0, driftedSourceProver.Calls)",
            "extraTopicBscSourceReceipt",
            "nonEmptyDataBscSourceReceipt",
            "missingBscSourceContextLog",
        ),
    ),
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
    "evm_live_metadata",
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
ALL_LANES_EVM_LIVE_METADATA_KEYS = {
    "required",
    "ready",
    "source_rpc_chain_id",
    "source_block_tag",
    "destination_rpc_chain_id",
    "destination_block_tag",
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
    "receipt_block_finalized",
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


def _artifact(path: Path) -> dict[str, Any]:
    if path.is_symlink():
        raise ValueError(f"release artifact path must not be a symlink: {path}")
    artifact_path = str(path)
    control_character = _path_control_character(artifact_path)
    if control_character is not None:
        raise ValueError(
            "release artifact path contains control character "
            f"{control_character}: {artifact_path!r}"
        )
    payload = path.read_bytes()
    return {
        "path": artifact_path,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


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


def _sdk_test_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...],
    *,
    label: str,
) -> list[str]:
    """Return source-inventory errors for SDK test marker scans."""

    errors: list[str] = []
    for raw_path, markers in inventory:
        path = Path(raw_path)
        display_path = str(path)
        if not path.is_absolute():
            display_path = path.as_posix()
            path = ROOT / path
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            errors.append(
                f"{label} SDK test inventory {display_path} is not UTF-8 "
                f"text: {exc}"
            )
            continue
        except OSError as exc:
            errors.append(
                f"{label} SDK test inventory {display_path} cannot be read: "
                f"{exc}"
            )
            continue
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{label} SDK test inventory {display_path} missing "
                    f"marker: {marker}"
                )
    return errors


def _source_marker_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...],
    *,
    label: str,
) -> list[str]:
    """Return source-inventory errors for marker scans."""

    errors: list[str] = []
    for raw_path, markers in inventory:
        path = Path(raw_path)
        display_path = str(path)
        if not path.is_absolute():
            display_path = path.as_posix()
            path = ROOT / path
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            errors.append(f"{label} source inventory {display_path} is not UTF-8 text: {exc}")
            continue
        except OSError as exc:
            errors.append(f"{label} source inventory {display_path} cannot be read: {exc}")
            continue
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{label} source inventory {display_path} missing marker: {marker}"
                )
    return errors


def _source_region(
    path: Path,
    *,
    start_marker: str,
    end_marker: str,
    label: str,
) -> tuple[str | None, list[str]]:
    """Return the source region delimited by two stable markers."""

    try:
        source = path.read_text(encoding="utf-8")
    except UnicodeDecodeError as exc:
        return None, [f"{label} source {path.as_posix()} is not UTF-8 text: {exc}"]
    except OSError as exc:
        return None, [f"{label} source {path.as_posix()} cannot be read: {exc}"]
    start = source.find(start_marker)
    if start == -1:
        return None, [f"{label} source {path.as_posix()} missing start marker: {start_marker}"]
    end = source.find(end_marker, start + len(start_marker))
    if end == -1:
        return None, [f"{label} source {path.as_posix()} missing end marker: {end_marker}"]
    return source[start:end], []


def _ethereum_data_collection_no_proxy_inventory_errors(
    regions: dict[str, tuple[str | Path, str, str, tuple[str, ...]]] | None = None,
) -> list[str]:
    """Return errors for Ethereum SDK data collection proxy-fallback guards."""

    if regions is None:
        regions = ETHEREUM_DATA_COLLECTION_REGIONS
    errors: list[str] = []
    for sdk, (raw_path, start_marker, end_marker, required_markers) in regions.items():
        path = Path(raw_path)
        display_path = path.as_posix()
        if not path.is_absolute():
            path = ROOT / path
        region, region_errors = _source_region(
            path,
            start_marker=start_marker,
            end_marker=end_marker,
            label=f"Ethereum mainnet {sdk} data collection",
        )
        errors.extend(region_errors)
        if region is None:
            continue
        for marker in required_markers:
            if marker not in region:
                errors.append(
                    "Ethereum mainnet "
                    f"{sdk} data collection source {display_path} missing "
                    f"provider marker: {marker}"
                )
        for label, pattern in ETHEREUM_DATA_COLLECTION_FORBIDDEN_PATTERNS.items():
            if pattern.search(region):
                errors.append(
                    "Ethereum mainnet "
                    f"{sdk} data collection source {display_path} contains "
                    f"forbidden {label}"
                )
    return errors


def _ethereum_inbound_adversarial_sdk_test_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for Ethereum inbound adversarial SDK tests."""

    if inventory is None:
        inventory = ETHEREUM_INBOUND_ADVERSARIAL_SDK_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet inbound adversarial",
    )


def _ethereum_outbound_precallback_sdk_test_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH outbound pre-callback SDK tests."""

    if inventory is None:
        inventory = ETHEREUM_OUTBOUND_PRECALLBACK_SDK_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet outbound pre-callback",
    )


def _ethereum_local_admission_sdk_test_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH local-admission SDK hardening tests."""

    if inventory is None:
        inventory = ETHEREUM_LOCAL_ADMISSION_SDK_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet local-admission",
    )


def _ethereum_outbound_provider_validation_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for outbound provider validation guards."""

    if inventory is None:
        inventory = ETHEREUM_OUTBOUND_PROVIDER_VALIDATION_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet outbound provider validation",
    )


def _ethereum_receipt_root_zero_sdk_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH receipt-root zero SDK guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_ROOT_ZERO_SDK_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet receipt-root zero rejection",
    )


def _ethereum_receipt_rlp_zero_topic_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH receipt RLP zero-topic guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_RLP_ZERO_TOPIC_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet receipt RLP zero-topic",
    )


def _ethereum_receipt_rlp_zero_address_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH receipt RLP zero-address guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_RLP_ZERO_ADDRESS_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet receipt RLP zero-address",
    )


def _ethereum_receipt_source_event_context_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH source-event log context guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_SOURCE_EVENT_CONTEXT_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet source-event context",
    )


def _ethereum_receipt_source_event_mode_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH source-event evidence mode guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_SOURCE_EVENT_MODE_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet source-event evidence mode",
    )


def _ethereum_receipt_source_event_zero_digest_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH source-event zero digest guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_SOURCE_EVENT_ZERO_DIGEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet source-event zero digest",
    )


def _ethereum_receipt_rpc_duplicate_json_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH receipt RPC duplicate-key guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_RPC_DUPLICATE_JSON_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet receipt RPC duplicate JSON",
    )


def _ethereum_receipt_block_transaction_hash_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH block receipt transaction hash guards."""

    if inventory is None:
        inventory = ETHEREUM_RECEIPT_BLOCK_TRANSACTION_HASH_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet block receipt transactionHash",
    )


def _ethereum_js_receipt_admission_guard_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for browser receipt-proof admission guards."""

    if inventory is None:
        inventory = ETHEREUM_JS_RECEIPT_ADMISSION_GUARD_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet JS receipt admission",
    )


def _ethereum_sdk_receipt_metadata_guard_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for SDK receipt metadata binding guards."""

    if inventory is None:
        inventory = ETHEREUM_SDK_RECEIPT_METADATA_GUARD_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet SDK receipt metadata",
    )


def _ethereum_native_receipt_finality_guard_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for native receipt-proof finality guards."""

    if inventory is None:
        inventory = ETHEREUM_NATIVE_RECEIPT_FINALITY_GUARD_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet native receipt finality",
    )


def _ethereum_noncanonical_chain_id_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH noncanonical chain-id tests."""

    if inventory is None:
        inventory = ETHEREUM_NONCANONICAL_CHAIN_ID_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet noncanonical chain id",
    )


def _ethereum_beacon_rest_finalized_header_shape_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH Beacon REST finalized-header guards."""

    if inventory is None:
        inventory = ETHEREUM_BEACON_REST_FINALIZED_HEADER_SHAPE_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet Beacon REST finalized-header shape",
    )


def _ethereum_beacon_rest_execution_payload_binding_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH Beacon REST execution-payload guards."""

    if inventory is None:
        inventory = ETHEREUM_BEACON_REST_EXECUTION_PAYLOAD_BINDING_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet Beacon REST execution payload binding",
    )


def _ethereum_sync_committee_roster_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for exact ETH mainnet sync-committee guards."""

    if inventory is None:
        inventory = ETHEREUM_SYNC_COMMITTEE_ROSTER_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet sync-committee roster",
    )


def _ethereum_source_bridge_config_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH source bridge config guards."""

    if inventory is None:
        inventory = ETHEREUM_SOURCE_BRIDGE_CONFIG_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet source bridge config",
    )


def _ethereum_evm_source_live_production_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH live EVM source production guards."""

    if inventory is None:
        inventory = ETHEREUM_EVM_SOURCE_LIVE_PRODUCTION_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet live EVM source production",
    )


def _ethereum_evm_live_destination_production_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH live EVM destination production guards."""

    if inventory is None:
        inventory = ETHEREUM_EVM_LIVE_DESTINATION_PRODUCTION_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet live EVM destination production",
    )


def _ethereum_route_canary_finalized_receipt_block_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for ETH route-canary receipt finality guards."""

    if inventory is None:
        inventory = ETHEREUM_ROUTE_CANARY_FINALIZED_RECEIPT_BLOCK_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="Ethereum mainnet route-canary finalized receipt block",
    )


def _ethereum_evm_block_tag_metadata_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for EVM finalized block-tag metadata guards."""

    if inventory is None:
        inventory = ETHEREUM_EVM_BLOCK_TAG_METADATA_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet EVM block-tag metadata",
    )


def _ethereum_evm_source_adapter_deployment_gate_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for EVM source-adapter deployment gates."""

    if inventory is None:
        inventory = ETHEREUM_EVM_SOURCE_ADAPTER_DEPLOYMENT_GATE_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet EVM source-adapter deployment gate",
    )


def _ethereum_launch_policy_selector_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for the Ethereum launch-policy selector."""

    if inventory is None:
        inventory = ETHEREUM_LAUNCH_POLICY_SELECTOR_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet launch-policy selector",
    )


def _ethereum_launch_policy_documentation_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
    forbidden_markers: tuple[str, ...] | None = None,
) -> list[str]:
    """Return inventory errors for active Ethereum launch-policy docs."""

    if inventory is None:
        inventory = ETHEREUM_LAUNCH_POLICY_DOCUMENTATION_MARKERS
    if forbidden_markers is None:
        forbidden_markers = ETHEREUM_LAUNCH_POLICY_DOCUMENTATION_FORBIDDEN_MARKERS
    label = "Ethereum mainnet launch-policy documentation"
    errors = _source_marker_inventory_errors(inventory, label=label)
    for raw_path, _markers in inventory:
        path = Path(raw_path)
        display_path = str(path)
        if not path.is_absolute():
            display_path = path.as_posix()
            path = ROOT / path
        try:
            source = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for marker in forbidden_markers:
            if marker in source:
                errors.append(
                    f"{label} source inventory {display_path} contains stale marker: {marker}"
                )
    return errors


def _ethereum_core_range_finality_binding_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for SCCP core range/finality binding guards."""

    if inventory is None:
        inventory = ETHEREUM_CORE_RANGE_FINALITY_BINDING_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet SCCP range finality binding",
    )


def _ethereum_core_message_replay_guard_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for SCCP core message replay guards."""

    if inventory is None:
        inventory = ETHEREUM_CORE_MESSAGE_REPLAY_GUARD_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet SCCP message replay guard",
    )


def _ethereum_torii_pinned_message_proof_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for Torii SCCP pinned message-proof submission."""

    if inventory is None:
        inventory = ETHEREUM_TORII_PINNED_MESSAGE_PROOF_MARKERS
    return _source_marker_inventory_errors(
        inventory,
        label="Ethereum mainnet Torii pinned message proof",
    )


def _sccp_unready_transparent_proof_config_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
    forbidden_paths: tuple[str | Path, ...] | None = None,
) -> list[str]:
    """Return inventory errors for config-owned unready SCCP proof toggles."""

    if inventory is None:
        inventory = SCCP_UNREADY_TRANSPARENT_PROOF_CONFIG_MARKERS
    if forbidden_paths is None:
        forbidden_paths = SCCP_UNREADY_TRANSPARENT_PROOF_FORBIDDEN_ENV_PATHS
    errors = _source_marker_inventory_errors(
        inventory,
        label="SCCP unready transparent-proof config-only",
    )
    for raw_path in forbidden_paths:
        path = Path(raw_path)
        display_path = str(path)
        if not path.is_absolute():
            display_path = path.as_posix()
            path = ROOT / path
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            errors.append(
                "SCCP unready transparent-proof config-only source inventory "
                f"{display_path} is not UTF-8 text: {exc}"
            )
            continue
        except OSError as exc:
            errors.append(
                "SCCP unready transparent-proof config-only source inventory "
                f"{display_path} cannot be read: {exc}"
            )
            continue
        if SCCP_UNREADY_TRANSPARENT_PROOF_FORBIDDEN_ENV in source:
            errors.append(
                "SCCP unready transparent-proof config-only source inventory "
                f"{display_path} contains forbidden environment override: "
                f"{SCCP_UNREADY_TRANSPARENT_PROOF_FORBIDDEN_ENV}"
            )
    return errors


def _contract_smoke_eth_mainnet_network_id_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for the EVM smoke ETH mainnet network id vector."""

    if inventory is None:
        inventory = CONTRACT_SMOKE_ETH_MAINNET_NETWORK_ID_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="EVM contract smoke Ethereum mainnet network id",
    )


def _contract_smoke_evm_production_surface_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for EVM bridge/verifier production smoke coverage."""

    if inventory is None:
        inventory = CONTRACT_SMOKE_EVM_PRODUCTION_SURFACE_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="EVM contract smoke production surface",
    )


def _native_sccp_no_wasm_readiness_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return inventory errors for native/local-prover readiness guards."""

    if inventory is None:
        inventory = NATIVE_SCCP_NO_WASM_READINESS_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="native SCCP no-WASM readiness",
    )


def _bsc_inbound_adversarial_sdk_test_inventory_errors(
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...] | None = None,
) -> list[str]:
    """Return source-inventory errors for BSC inbound adversarial SDK tests."""

    if inventory is None:
        inventory = BSC_INBOUND_ADVERSARIAL_SDK_TEST_MARKERS
    return _sdk_test_inventory_errors(
        inventory,
        label="BSC mainnet inbound adversarial",
    )


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
                "route_canary_evidence_bound": bool(route_canary.get("evidence_bound")),
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


def _active_launch_evm_live_metadata_blockers(
    lane_label: str,
    lane: dict[str, Any],
) -> list[str]:
    evm_live_metadata = lane.get("evm_live_metadata")
    if not isinstance(evm_live_metadata, dict):
        evm_live_metadata = {}
    expected_chain_ids = {
        "eth": {"1", "0x1"},
        "bsc": {"56", "0x38"},
    }.get(ACTIVE_LAUNCH_CHAIN, set())
    expected_chain_id_label = {
        "eth": "1 (0x1)",
        "bsc": "56 (0x38)",
    }.get(ACTIVE_LAUNCH_CHAIN, "the configured mainnet chain id")

    blockers: list[str] = []
    if evm_live_metadata.get("source_rpc_chain_id") not in expected_chain_ids:
        blockers.append(
            f"{lane_label}: {ACTIVE_LAUNCH_DISPLAY} source live eth_chainId must be {expected_chain_id_label}"
        )
    if evm_live_metadata.get("destination_rpc_chain_id") not in expected_chain_ids:
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


def _is_nonzero_hex32(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) != 66:
        return False
    try:
        raw = bytes.fromhex(value[2:])
    except ValueError:
        return False
    return len(raw) == 32 and any(raw) and value == f"0x{raw.hex()}"


def _native_evm_manifest_relative_path(
    value: Any,
    label: str,
) -> tuple[PurePosixPath | None, list[str]]:
    prefix = f"native EVM Groth16 prover bundle {label}"
    if not isinstance(value, str) or not value:
        return None, [
            f"{prefix} path must be a non-empty relative POSIX file path"
        ]
    control_character = _path_control_character(value)
    if control_character is not None:
        return None, [
            f"{prefix} path contains control character {control_character}: {value!r}"
        ]
    if "\\" in value:
        return None, [f"{prefix} path must use POSIX separators"]
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
    except OSError as exc:
        return [
            f"{prefix} cannot be scanned for forbidden prover dependency markers: {exc}"
        ]

    return [
        f"{prefix} contains forbidden prover dependency marker: "
        f"{marker.decode('ascii')}"
        for marker in NATIVE_EVM_PROVER_FORBIDDEN_PAYLOAD_MARKERS
        if marker in payload
    ]


def _native_evm_prover_payload_artifact(
    manifest_path: Path | None,
    manifest_artifact: dict[str, Any] | None,
    payload: dict[str, Any],
    path_field: str,
    hash_field: str,
    label: str,
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
                f"{prefix} file is missing or is not a regular file: "
                f"{relative_path.as_posix()}"
            )
            return None, blockers
        artifact = _artifact(artifact_path)
    except OSError as exc:
        blockers.append(f"{prefix} cannot be read: {exc}")
        return None, blockers
    except ValueError as exc:
        blockers.append(f"{prefix} {exc}")
        return None, blockers

    if artifact["bytes"] == 0:
        blockers.append(f"{prefix} must not be empty")

    if isinstance(manifest_artifact, dict):
        manifest_artifact_path = manifest_artifact.get("path")
        if isinstance(manifest_artifact_path, str) and manifest_artifact_path:
            manifest_relative_path = PurePosixPath(manifest_artifact_path)
            artifact["path"] = (
                manifest_relative_path.parent.joinpath(relative_path).as_posix()
            )

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
    manifest_artifact: dict[str, Any] | None,
) -> tuple[list[dict[str, Any]], list[str]]:
    blockers: list[str] = []
    if not isinstance(artifacts, list) or not artifacts:
        return [], ["native_sdk_artifacts must be a non-empty list"]

    rows: list[dict[str, Any]] = []
    by_sdk: dict[str, dict[str, Any]] = {}
    for index, artifact in enumerate(artifacts):
        label = f"native_sdk_artifacts[{index}]"
        if not isinstance(artifact, dict):
            blockers.append(f"{label} must be an object")
            continue
        for key in sorted(set(artifact) - NATIVE_EVM_PROVER_SDK_ARTIFACT_KEYS):
            blockers.append(f"{label} contains unknown field: {key}")
        for key in sorted(NATIVE_EVM_PROVER_SDK_ARTIFACT_KEYS - set(artifact)):
            blockers.append(f"{label} missing field: {key}")
        sdk = artifact.get("sdk")
        implementation = artifact.get("implementation")
        if not isinstance(sdk, str) or not sdk:
            blockers.append(f"{label}.sdk must be a non-empty string")
            continue
        if sdk in by_sdk:
            blockers.append(f"native_sdk_artifacts contains duplicate sdk: {sdk}")
        expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS.get(sdk)
        if expected_implementation is None:
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
                manifest_artifact,
                artifact,
                "implementation_artifact",
                "implementation_hash",
                f"{sdk} implementation_artifact",
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

    return sorted(rows, key=lambda row: row["sdk"]), blockers


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


def _native_evm_prover_bundle_status_from_payload(
    artifact: dict[str, Any] | None,
    manifest_path: Path | None,
    payload: Any,
    evidence: dict[str, Any],
    blockers: list[str] | None = None,
) -> dict[str, Any]:
    blockers = list(blockers or [])
    if not isinstance(payload, dict):
        blockers.append("native EVM Groth16 prover bundle must be a JSON object")
        payload = {}

    for key in sorted(set(payload) - NATIVE_EVM_PROVER_BUNDLE_REQUIRED_KEYS):
        blockers.append(f"native EVM Groth16 prover bundle contains unknown field: {key}")
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
    if not isinstance(audit_hashes, list) or not audit_hashes:
        blockers.append("native EVM Groth16 prover bundle audit_hashes must be non-empty")
        audit_hashes = []
    else:
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
        seen_audit_hashes: dict[str, int] = {}
        for index, audit_hash in enumerate(audit_hashes):
            if not _is_nonzero_hex32(audit_hash):
                blockers.append(
                    "native EVM Groth16 prover bundle "
                    f"audit_hashes[{index}] must be a canonical non-zero 32-byte hex value"
                )
                continue
            previous_index = seen_audit_hashes.get(audit_hash)
            if previous_index is not None:
                blockers.append(
                    "native EVM Groth16 prover bundle "
                    f"audit_hashes[{index}] must not duplicate "
                    f"audit_hashes[{previous_index}]"
                )
            seen_audit_hashes[audit_hash] = index
            for role, role_hash in reserved_audit_hash_roles.items():
                if audit_hash == role_hash:
                    blockers.append(
                        "native EVM Groth16 prover bundle "
                        f"audit_hashes[{index}] must not reuse {role}"
                    )

    proof_artifact, proof_artifact_blockers = _native_evm_prover_payload_artifact(
        manifest_path,
        artifact,
        payload,
        "proof_artifact",
        "proof_artifact_hash",
        "proof_artifact",
    )
    blockers.extend(proof_artifact_blockers)
    proving_key, proving_key_blockers = _native_evm_prover_payload_artifact(
        manifest_path,
        artifact,
        payload,
        "proving_key",
        "proving_key_hash",
        "proving_key",
    )
    blockers.extend(proving_key_blockers)
    verifier_key, verifier_key_blockers = _native_evm_prover_payload_artifact(
        manifest_path,
        artifact,
        payload,
        "verifier_key",
        "verifier_key_hash",
        "verifier_key",
    )
    blockers.extend(verifier_key_blockers)

    sdk_artifacts, sdk_blockers = _native_evm_prover_bundle_artifact_summary(
        payload.get("native_sdk_artifacts"),
        payload.get("proof_artifact_hash"),
        payload.get("proving_key_hash"),
        manifest_path,
        artifact,
    )
    blockers.extend(sdk_blockers)

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
        "audit_hashes": list(audit_hashes),
        "sdk_artifacts": sdk_artifacts,
        "validation_status": "passed" if not blockers else "blocked",
        "validation_blockers": blockers,
    }


def _missing_native_evm_prover_bundle_status() -> dict[str, Any]:
    return _native_evm_prover_bundle_status_from_payload(
        None,
        None,
        {},
        {},
        ["native EVM Groth16 prover bundle manifest is required"],
    )


def _expected_native_evm_prover_bundle_status(
    bundle_dir: Path,
    report: dict[str, Any],
    evidence: dict[str, Any],
) -> dict[str, Any]:
    raw_status = report.get("native_evm_prover_bundle")
    if not isinstance(raw_status, dict):
        return _missing_native_evm_prover_bundle_status()
    artifact = raw_status.get("artifact")
    if not isinstance(artifact, dict):
        return _missing_native_evm_prover_bundle_status()
    artifact_path = _bundle_artifact_path(bundle_dir, artifact)
    if artifact_path is None:
        return _missing_native_evm_prover_bundle_status()
    blockers: list[str] = []
    try:
        payload = _load_json(artifact_path)
    except DuplicateKeyError as exc:
        payload = {}
        blockers.append(f"native EVM Groth16 prover bundle contains duplicate key: {exc.key}")
    except UnicodeDecodeError as exc:
        payload = {}
        blockers.append(f"native EVM Groth16 prover bundle is not UTF-8 text: {exc}")
    except json.JSONDecodeError as exc:
        payload = {}
        blockers.append(f"native EVM Groth16 prover bundle is not valid JSON: {exc}")
    except OSError as exc:
        payload = {}
        blockers.append(f"native EVM Groth16 prover bundle cannot be read: {exc}")
    return _native_evm_prover_bundle_status_from_payload(
        artifact,
        artifact_path,
        payload,
        evidence,
        blockers,
    )


def _native_evm_prover_bundle_summary_schema_errors(status: dict[str, Any]) -> list[str]:
    label = "readiness report native_evm_prover_bundle"
    errors: list[str] = []
    for key in sorted(set(status) - NATIVE_EVM_PROVER_BUNDLE_SUMMARY_KEYS):
        errors.append(f"{label} contains unknown field: {key}")
    for key in sorted(NATIVE_EVM_PROVER_BUNDLE_SUMMARY_KEYS - set(status)):
        errors.append(f"{label} missing field: {key}")
    if status.get("required") is not True:
        errors.append(f"{label} required must be true")
    for field in (
        "schema",
        "bundle_id",
        "lanes",
        "proof_backend",
        "proof_artifact_hash",
        "proving_key_hash",
        "verifier_key_hash",
        "destination_binding_hash",
    ):
        errors.extend(_non_empty_string_field_errors(label, status, field))
    if status.get("schema") != NATIVE_EVM_PROVER_BUNDLE_SCHEMA:
        errors.append(f"{label} schema must be {NATIVE_EVM_PROVER_BUNDLE_SCHEMA}")
    if status.get("bundle_id") != NATIVE_EVM_PROVER_BUNDLE_ID:
        errors.append(f"{label} bundle_id must be {NATIVE_EVM_PROVER_BUNDLE_ID}")
    if status.get("lanes") != ACTIVE_LAUNCH_CHAIN:
        errors.append(f"{label} lanes must be {ACTIVE_LAUNCH_CHAIN}")
    if status.get("proof_backend") != "evm-groth16-bn254-v1":
        errors.append(f"{label} proof_backend must be evm-groth16-bn254-v1")
    for field in (
        "proof_artifact_hash",
        "proving_key_hash",
        "verifier_key_hash",
        "destination_binding_hash",
    ):
        if field in status and not _is_nonzero_hex32(status.get(field)):
            errors.append(
                f"{label} {field} must be a canonical non-zero 32-byte hex value"
            )
    artifact = status.get("artifact")
    if not isinstance(artifact, dict):
        errors.append(f"{label} artifact must be an object")
    else:
        for key in sorted(set(artifact) - ARTIFACT_KEYS):
            errors.append(f"{label} artifact contains unknown field: {key}")
        path_errors = _canonical_artifact_path(artifact)[1]
        errors.extend(f"{label} artifact {error}" for error in path_errors)
    for artifact_field, hash_field in (
        ("proof_artifact", "proof_artifact_hash"),
        ("proving_key", "proving_key_hash"),
        ("verifier_key", "verifier_key_hash"),
    ):
        artifact = status.get(artifact_field)
        if not isinstance(artifact, dict):
            errors.append(f"{label} {artifact_field} must be an object")
            continue
        for key in sorted(set(artifact) - ARTIFACT_KEYS):
            errors.append(f"{label} {artifact_field} contains unknown field: {key}")
        path_errors = _canonical_artifact_path(artifact)[1]
        errors.extend(f"{label} {artifact_field} {error}" for error in path_errors)
        artifact_hash = artifact.get("sha256")
        expected_hash = status.get(hash_field)
        if (
            _is_canonical_sha256_text(artifact_hash)
            and isinstance(expected_hash, str)
            and f"0x{artifact_hash}" != expected_hash
        ):
            errors.append(f"{label} {artifact_field} sha256 must match {hash_field}")
    errors.extend(_string_list_field_errors(label, status, "audit_hashes", allow_empty=False))
    audit_hashes = status.get("audit_hashes")
    if isinstance(audit_hashes, list):
        for index, value in enumerate(audit_hashes):
            if not _is_nonzero_hex32(value):
                errors.append(
                    f"{label} audit_hashes[{index}] must be a canonical non-zero 32-byte hex value"
                )
    sdk_artifacts = status.get("sdk_artifacts")
    if not isinstance(sdk_artifacts, list) or not sdk_artifacts:
        errors.append(f"{label} sdk_artifacts must be a non-empty list")
    else:
        seen_sdks: set[str] = set()
        for index, row in enumerate(sdk_artifacts):
            row_label = f"{label} sdk_artifacts[{index}]"
            if not isinstance(row, dict):
                errors.append(f"{row_label} must be an object")
                continue
            for key in sorted(set(row) - NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_KEYS):
                errors.append(f"{row_label} contains unknown field: {key}")
            for key in sorted(NATIVE_EVM_PROVER_SDK_ARTIFACT_SUMMARY_KEYS - set(row)):
                errors.append(f"{row_label} missing field: {key}")
            sdk = row.get("sdk")
            if not isinstance(sdk, str) or not sdk:
                errors.append(f"{row_label} sdk must be a non-empty string")
                continue
            if sdk in seen_sdks:
                errors.append(f"{label} sdk_artifacts contains duplicate sdk: {sdk}")
            seen_sdks.add(sdk)
            expected_implementation = NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS.get(sdk)
            if expected_implementation is None:
                errors.append(f"{label} sdk_artifacts contains unknown sdk: {sdk}")
            elif row.get("implementation") != expected_implementation:
                errors.append(
                    f"{row_label} implementation must be {expected_implementation}"
                )
            if not _is_nonzero_hex32(row.get("implementation_hash")):
                errors.append(
                    f"{row_label} implementation_hash must be a canonical non-zero 32-byte hex value"
                )
            implementation_artifact = row.get("implementation_artifact")
            if not isinstance(implementation_artifact, dict):
                errors.append(f"{row_label} implementation_artifact must be an object")
            else:
                for key in sorted(set(implementation_artifact) - ARTIFACT_KEYS):
                    errors.append(
                        f"{row_label} implementation_artifact contains unknown field: {key}"
                    )
                path_errors = _canonical_artifact_path(implementation_artifact)[1]
                errors.extend(
                    f"{row_label} implementation_artifact {error}"
                    for error in path_errors
                )
                artifact_hash = implementation_artifact.get("sha256")
                if (
                    _is_canonical_sha256_text(artifact_hash)
                    and isinstance(row.get("implementation_hash"), str)
                    and f"0x{artifact_hash}" != row.get("implementation_hash")
                ):
                    errors.append(
                        f"{row_label} implementation_artifact sha256 must match "
                        "implementation_hash"
                    )
        for sdk in sorted(set(NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS) - seen_sdks):
            errors.append(f"{label} sdk_artifacts missing sdk: {sdk}")
    if status.get("validation_status") != "passed":
        errors.append(f"{label} validation_status must be passed")
    errors.extend(_string_list_field_errors(label, status, "validation_blockers", allow_empty=True))
    if status.get("validation_blockers"):
        errors.append(f"{label} validation_blockers must be empty")
    return errors


def _active_launch_release_checklist(
    evidence: dict[str, Any],
    native_prover_bundle: dict[str, Any] | None = None,
) -> dict[str, Any]:
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
    if lane:
        deployment_blockers.extend(
            _active_launch_evm_live_metadata_blockers(lane_label, lane)
        )
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

    if native_prover_bundle is None:
        native_prover_bundle = _missing_native_evm_prover_bundle_status()
    native_prover_blockers = [
        blocker
        for blocker in native_prover_bundle.get("validation_blockers", [])
        if isinstance(blocker, str)
    ]
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


def _readiness_markdown_boolean_cell(value: Any) -> str:
    if type(value) is bool:
        return "`true`" if value else "`false`"
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
        if not row["route_canary_evidence_bound"]:
            canary_source = f"{canary_source} (unbound)"
        source_gate = _readiness_markdown_hash_cell(row["source_adapter_gate_hash"])
        if not row["source_adapter_gate_required"] and source_gate == "-":
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
                canary_tx=_readiness_markdown_hash_cell(
                    row["route_canary_transaction_hash"]
                ),
                canary_receipt_block=_readiness_markdown_integer_cell(
                    row["route_canary_receipt_block_number"]
                ),
                canary_receipt_hash=_readiness_markdown_hash_cell(
                    row["route_canary_receipt_block_hash"]
                ),
                canary_receipt_finalized=_readiness_markdown_boolean_cell(
                    row["route_canary_receipt_block_finalized"]
                ),
                canary_receipts_root=_readiness_markdown_hash_cell(
                    row["route_canary_block_receipts_root"]
                ),
                canary_message_id=_readiness_markdown_hash_cell(
                    row["route_canary_message_id"]
                ),
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

    lines.extend(["", "## Native Prover Bundle", ""])
    lines.append(
        "| Required | Status | Artifact | SHA-256 | Proof Artifact | Proving Key | "
        "Verifier Key | Destination Binding | SDK Artifacts | Blockers |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
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
    native_blockers = native_bundle.get("validation_blockers") or []
    native_blocker_text = "<br>".join(native_blockers) if native_blockers else "-"
    lines.append(
        "| {required} | {status} | {artifact} | {artifact_hash} | "
        "{proof_artifact} | {proving_key} | {verifier_key} | {binding} | "
        "{sdk_artifacts} | {blockers} |".format(
            required="yes" if native_bundle.get("required") else "no",
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
            sdk_artifacts=sdk_cell,
            blockers=native_blocker_text,
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
            f"- {ACTIVE_LAUNCH_DISPLAY} source and destination EVM live reads must report {ACTIVE_LAUNCH_EVM_CHAIN_ID_EVIDENCE} and be pinned to the `finalized` block tag in both the all-lanes summary and readiness cryptographic-evidence table.",
            "- Governed live deployment evidence for immutable destination verifiers and source-chain verifier engines; offline placeholder or template-derived hashes keep the report blocked.",
            "- An audited `--native-evm-prover-bundle` manifest with `schema = sccp-native-evm-groth16-prover-bundle-v1`, `no_wasm = true`, `remote_prover_required = false`, and matching Ethereum destination binding/proving-key hashes.",
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
                "evm_source_block_tag",
                "evm_destination_block_tag",
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
                "destination_binding_hash",
                "route_allowlist_hash",
                "route_canary_evidence_hash",
                "route_canary_evidence_source",
                "route_canary_transaction_hash",
                "route_canary_receipt_block_hash",
                "route_canary_block_receipts_root",
                "route_canary_message_id",
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
                "route_canary_receipt_block_number",
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
    native_bundle = report.get("native_evm_prover_bundle")
    if isinstance(native_bundle, dict):
        artifact = native_bundle.get("artifact")
        if isinstance(artifact, dict):
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if not path_errors and artifact_path is not None:
                paths.append(artifact_path)
        for artifact_field in ("proof_artifact", "proving_key", "verifier_key"):
            artifact = native_bundle.get(artifact_field)
            if isinstance(artifact, dict):
                artifact_path, path_errors = _canonical_artifact_path(artifact)
                if not path_errors and artifact_path is not None:
                    paths.append(artifact_path)
        sdk_artifacts = native_bundle.get("sdk_artifacts")
        if isinstance(sdk_artifacts, list):
            for row in sdk_artifacts:
                if not isinstance(row, dict):
                    continue
                artifact = row.get("implementation_artifact")
                if isinstance(artifact, dict):
                    artifact_path, path_errors = _canonical_artifact_path(artifact)
                    if not path_errors and artifact_path is not None:
                        paths.append(artifact_path)

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
    native_prover_bundle = report.get("native_evm_prover_bundle")
    if not isinstance(native_prover_bundle, dict):
        native_prover_bundle = _missing_native_evm_prover_bundle_status()
    return _active_launch_release_checklist(evidence, native_prover_bundle)


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
    chain: Any = None,
) -> str | None:
    route_profile = _route_allowlist_chain_and_id(domain, chain)
    if route_profile is None:
        return None
    route_chain, route_allowlist_id = route_profile
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
    _push_vec(payload, route_chain.encode("utf-8"))
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


def _cryptographic_evidence_row_schema_errors(
    row: dict[str, Any],
    *,
    enforce_evm_live_tags: bool = True,
) -> list[str]:
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
    for field in ("evm_source_rpc_chain_id", "evm_destination_rpc_chain_id"):
        if field in row and not isinstance(row.get(field), str):
            errors.append(
                "readiness report cryptographic evidence row "
                f"{field} must be a string"
            )
    for field in ("evm_source_block_tag", "evm_destination_block_tag"):
        if field in row and not isinstance(row.get(field), str):
            errors.append(
                "readiness report cryptographic evidence row "
                f"{field} must be a string"
            )
    domain = row.get("domain")
    if domain == SCCP_DOMAIN_ETH:
        for field in ("evm_source_rpc_chain_id", "evm_destination_rpc_chain_id"):
            if (
                enforce_evm_live_tags
                and isinstance(row.get(field), str)
                and (
                    not _is_canonical_decimal_text(row.get(field), positive=True)
                    or int(row[field], 10)
                    != EVM_EXPECTED_RPC_CHAIN_IDS[SCCP_DOMAIN_ETH]
                )
            ):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be Ethereum mainnet chain id 1"
                )
        for field in ("evm_source_block_tag", "evm_destination_block_tag"):
            if (
                enforce_evm_live_tags
                and isinstance(row.get(field), str)
                and row.get(field) != "finalized"
            ):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be finalized for Ethereum mainnet"
                )
    elif domain == SCCP_DOMAIN_BSC:
        bsc_has_evm_evidence = any(
            row.get(field)
            for field in (
                "source_verifier_material_hash",
                "source_adapter_engine_deployment_hash",
                "destination_binding_hash",
                "route_allowlist_hash",
                "route_canary_evidence_hash",
            )
        )
        for field in ("evm_source_rpc_chain_id", "evm_destination_rpc_chain_id"):
            if (
                enforce_evm_live_tags
                and bsc_has_evm_evidence
                and isinstance(row.get(field), str)
                and (
                    not _is_canonical_decimal_text(row.get(field), positive=True)
                    or int(row[field], 10)
                    != _expected_evm_rpc_chain_id(SCCP_DOMAIN_BSC, row.get("chain"))
                )
            ):
                expected_chain_id = _expected_evm_rpc_chain_id(
                    SCCP_DOMAIN_BSC,
                    row.get("chain"),
                )
                expected_chain = (
                    "bsc-testnet" if row.get("chain") == "bsc-testnet" else "bsc"
                )
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be BSC chain id {expected_chain_id} for {expected_chain}"
                )
        for field in ("evm_source_block_tag", "evm_destination_block_tag"):
            if (
                enforce_evm_live_tags
                and bsc_has_evm_evidence
                and isinstance(row.get(field), str)
                and not row.get(field)
            ):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be non-empty for BSC EVM evidence"
                )
    elif domain in ALL_LANES_CHAIN_BY_DOMAIN:
        for field in ("evm_source_rpc_chain_id", "evm_destination_rpc_chain_id"):
            if isinstance(row.get(field), str) and row.get(field):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be empty for non-EVM lanes"
                )
        for field in ("evm_source_block_tag", "evm_destination_block_tag"):
            if isinstance(row.get(field), str) and row.get(field):
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be empty for non-EVM lanes"
                )
    if "route_canary_evidence_bound" in row and (
        type(row.get("route_canary_evidence_bound")) is not bool
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_evidence_bound must be a boolean"
        )
    if "route_canary_receipt_block_finalized" in row and (
        row.get("route_canary_receipt_block_finalized") is not None
        and type(row.get("route_canary_receipt_block_finalized")) is not bool
    ):
        errors.append(
            "readiness report cryptographic evidence row "
            "route_canary_receipt_block_finalized must be a boolean or null"
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
    evm_canary_hash_fields = (
        "route_canary_transaction_hash",
        "route_canary_receipt_block_hash",
        "route_canary_block_receipts_root",
        "route_canary_message_id",
    )
    if row.get("domain") in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC) and row.get(
        "route_canary_evidence_hash"
    ):
        for field in evm_canary_hash_fields:
            errors.extend(
                _nonzero_fixed_hex_field_errors(
                    "readiness report cryptographic evidence row",
                    row,
                    field,
                    byte_length=32,
                    type_label="bytes32",
                )
            )
        errors.extend(
            _integer_field_errors(
                "readiness report cryptographic evidence row",
                row,
                "route_canary_receipt_block_number",
                positive=True,
            )
        )
        if row.get("route_canary_receipt_block_finalized") is not True:
            errors.append(
                "readiness report cryptographic evidence row "
                "route_canary_receipt_block_finalized must be true for finalized "
                "EVM route canary evidence"
            )
    else:
        for field in (
            *evm_canary_hash_fields,
            "route_canary_receipt_block_number",
            "route_canary_receipt_block_finalized",
        ):
            if field in row and row.get(field) is not None:
                errors.append(
                    "readiness report cryptographic evidence row "
                    f"{field} must be null for lanes without EVM route canary evidence"
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
                "evm_source_rpc_chain_id",
                ("evm_live_metadata", "source_rpc_chain_id"),
            ),
            ("evm_source_block_tag", ("evm_live_metadata", "source_block_tag")),
            (
                "evm_destination_rpc_chain_id",
                ("evm_live_metadata", "destination_rpc_chain_id"),
            ),
            (
                "evm_destination_block_tag",
                ("evm_live_metadata", "destination_block_tag"),
            ),
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
                "route_canary_transaction_hash",
                ("route_allowlist", "route_canary", "transaction_hash"),
            ),
            (
                "route_canary_receipt_block_number",
                ("route_allowlist", "route_canary", "receipt_block_number"),
            ),
            (
                "route_canary_receipt_block_hash",
                ("route_allowlist", "route_canary", "receipt_block_hash"),
            ),
            (
                "route_canary_receipt_block_finalized",
                ("route_allowlist", "route_canary", "receipt_block_finalized"),
            ),
            (
                "route_canary_block_receipts_root",
                ("route_allowlist", "route_canary", "block_receipts_root"),
            ),
            (
                "route_canary_message_id",
                ("route_allowlist", "route_canary", "message_id"),
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
        expected_chain = _expected_chain_label(domain)
        if expected_chain is None:
            errors.append(f"{label} contains unknown domain: {domain}")
            continue
        chain = row.get("chain")
        if isinstance(chain, str) and chain and not _chain_matches_domain(domain, chain):
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
        chain=lane.get("chain"),
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
        errors.extend(
            _true_field_errors(label, route_canary, "receipt_block_finalized")
        )
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
        expected_chain = _expected_chain_label(domain) if type(domain) is int else None
        if type(domain) is int and expected_chain is None:
            errors.append(f"{lane_label} domain must be a production remote domain")
        if "chain" in lane and (
            not isinstance(lane.get("chain"), str) or not lane.get("chain")
        ):
            errors.append(f"{lane_label} chain must be a non-empty string")
        elif expected_chain is not None and not _chain_matches_domain(
            domain,
            lane.get("chain"),
        ):
            errors.append(f"{lane_label} chain must be {expected_chain}")
        if domain == ACTIVE_LAUNCH_DOMAIN:
            errors.extend(_true_field_errors(lane_label, lane, "production_ready"))
        else:
            errors.extend(_boolean_field_errors(lane_label, lane, "production_ready"))
        for field in (
            "records",
            "source_record_hashes",
            "source_adapter_gate",
            "evm_live_metadata",
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
        evm_metadata = lane.get("evm_live_metadata")
        if isinstance(evm_metadata, dict):
            metadata_label = f"{lane_label} evm_live_metadata"
            errors.extend(
                _exact_object_key_errors(
                    metadata_label,
                    evm_metadata,
                    ALL_LANES_EVM_LIVE_METADATA_KEYS,
                )
            )
            errors.extend(
                _boolean_field_errors(metadata_label, evm_metadata, "required")
            )
            errors.extend(_boolean_field_errors(metadata_label, evm_metadata, "ready"))
            for field in (
                "source_rpc_chain_id",
                "source_block_tag",
                "destination_rpc_chain_id",
                "destination_block_tag",
            ):
                if field in evm_metadata and not isinstance(
                    evm_metadata.get(field),
                    str,
                ):
                    errors.append(f"{metadata_label} {field} must be a string")
            if domain in ALL_LANES_EVM_DESTINATION_DOMAINS:
                errors.extend(_true_field_errors(metadata_label, evm_metadata, "required"))
                if domain == ACTIVE_LAUNCH_DOMAIN:
                    errors.extend(_true_field_errors(metadata_label, evm_metadata, "ready"))
                for field in (
                    "source_rpc_chain_id",
                    "source_block_tag",
                    "destination_rpc_chain_id",
                    "destination_block_tag",
                ):
                    if not evm_metadata.get(field):
                        errors.append(f"{metadata_label} {field} must be present")
                expected_chain_id = _expected_evm_rpc_chain_id(
                    domain,
                    lane.get("chain"),
                )
                for field in ("source_rpc_chain_id", "destination_rpc_chain_id"):
                    value = evm_metadata.get(field)
                    if isinstance(value, str) and (
                        not _is_canonical_decimal_text(value, positive=True)
                        or int(value, 10) != expected_chain_id
                    ):
                        errors.append(
                            f"{metadata_label} {field} must be canonical chain id "
                            f"{expected_chain_id}"
                        )
                if domain == SCCP_DOMAIN_ETH:
                    for field in ("source_block_tag", "destination_block_tag"):
                        if evm_metadata.get(field) != "finalized":
                            errors.append(
                                f"{metadata_label} {field} must be finalized "
                                "for Ethereum mainnet"
                            )
            else:
                if evm_metadata.get("required") is not False:
                    errors.append(
                        f"{metadata_label} required must be false for non-EVM lanes"
                    )
                if evm_metadata.get("ready") is not True:
                    errors.append(
                        f"{metadata_label} ready must be true for non-EVM lanes"
                    )
                for field in (
                    "source_rpc_chain_id",
                    "source_block_tag",
                    "destination_rpc_chain_id",
                    "destination_block_tag",
                ):
                    if evm_metadata.get(field) not in ("", None):
                        errors.append(
                            f"{metadata_label} {field} must be empty for non-EVM lanes"
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

    native_bundle = report.get("native_evm_prover_bundle")
    if isinstance(native_bundle, dict):
        artifact = native_bundle.get("artifact")
        if isinstance(artifact, dict):
            artifact_path, path_errors = _canonical_artifact_path(artifact)
            if not path_errors and artifact_path is not None:
                paths.add(artifact_path)
        for artifact_field in ("proof_artifact", "proving_key", "verifier_key"):
            artifact = native_bundle.get(artifact_field)
            if isinstance(artifact, dict):
                artifact_path, path_errors = _canonical_artifact_path(artifact)
                if not path_errors and artifact_path is not None:
                    paths.add(artifact_path)
        sdk_artifacts = native_bundle.get("sdk_artifacts")
        if isinstance(sdk_artifacts, list):
            for row in sdk_artifacts:
                if not isinstance(row, dict):
                    continue
                artifact = row.get("implementation_artifact")
                if isinstance(artifact, dict):
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
    report_native_bundle: dict[str, Any] = {}
    summary_release_checklist: dict[str, Any] = {}
    report_evidence_schema_valid = False
    report_release_checklist_schema_valid = False
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
            evidence_schema_errors = _all_lanes_summary_schema_errors(
                "readiness report embedded evidence",
                report_evidence,
            )
            errors.extend(evidence_schema_errors)
            report_evidence_schema_valid = not evidence_schema_errors
        raw_release_checklist = report.get("release_checklist")
        if not isinstance(raw_release_checklist, dict):
            errors.append("readiness report release_checklist is not an object")
        else:
            report_release_checklist = raw_release_checklist
            release_checklist_schema_errors = _release_checklist_schema_errors(
                "readiness report",
                report_release_checklist,
            )
            errors.extend(release_checklist_schema_errors)
            report_release_checklist_schema_valid = not release_checklist_schema_errors
        raw_corridor = report.get("corridor")
        if not isinstance(raw_corridor, dict):
            errors.append("readiness report corridor is not an object")
        else:
            report_corridor = raw_corridor
            errors.extend(_corridor_schema_errors(report_corridor))
        raw_native_bundle = report.get("native_evm_prover_bundle")
        if not isinstance(raw_native_bundle, dict):
            errors.append("readiness report native_evm_prover_bundle is not an object")
        else:
            report_native_bundle = raw_native_bundle
            errors.extend(
                _native_evm_prover_bundle_summary_schema_errors(
                    report_native_bundle
                )
            )
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
    if (
        report
        and report_evidence_schema_valid
        and report_release_checklist_schema_valid
        and report_release_checklist != _expected_release_checklist(report)
    ):
        errors.append(
            "readiness report release_checklist does not match embedded evidence"
        )
    if report and report_native_bundle:
        expected_native_bundle = _expected_native_evm_prover_bundle_status(
            bundle_dir,
            report,
            report_evidence,
        )
        for blocker in expected_native_bundle.get("validation_blockers", []):
            errors.append(f"bundled native EVM prover manifest blocker: {blocker}")
        if report_native_bundle != expected_native_bundle:
            errors.append(
                "readiness report native_evm_prover_bundle does not match bundled "
                "native prover manifest"
            )
    if report and not report_corridor.get("production_ready"):
        errors.append("readiness report production corridor is not ready")
    if report and report_corridor.get("require_phase_evidence") is not True:
        errors.append("readiness report does not require hashed phase evidence")
    if report:
        errors.extend(_corridor_phase_errors(report_corridor))
    errors.extend(_ethereum_inbound_adversarial_sdk_test_inventory_errors())
    errors.extend(_ethereum_outbound_precallback_sdk_test_inventory_errors())
    errors.extend(_ethereum_local_admission_sdk_test_inventory_errors())
    errors.extend(_ethereum_outbound_provider_validation_inventory_errors())
    errors.extend(_ethereum_receipt_root_zero_sdk_inventory_errors())
    errors.extend(_ethereum_receipt_rlp_zero_topic_inventory_errors())
    errors.extend(_ethereum_receipt_rlp_zero_address_inventory_errors())
    errors.extend(_ethereum_receipt_source_event_context_inventory_errors())
    errors.extend(_ethereum_receipt_source_event_mode_inventory_errors())
    errors.extend(_ethereum_receipt_source_event_zero_digest_inventory_errors())
    errors.extend(_ethereum_receipt_rpc_duplicate_json_inventory_errors())
    errors.extend(_ethereum_receipt_block_transaction_hash_inventory_errors())
    errors.extend(_ethereum_js_receipt_admission_guard_inventory_errors())
    errors.extend(_ethereum_sdk_receipt_metadata_guard_inventory_errors())
    errors.extend(_ethereum_native_receipt_finality_guard_inventory_errors())
    errors.extend(_ethereum_noncanonical_chain_id_inventory_errors())
    errors.extend(_ethereum_beacon_rest_finalized_header_shape_inventory_errors())
    errors.extend(_ethereum_beacon_rest_execution_payload_binding_inventory_errors())
    errors.extend(_ethereum_sync_committee_roster_inventory_errors())
    errors.extend(_ethereum_source_bridge_config_inventory_errors())
    errors.extend(_ethereum_evm_source_adapter_deployment_gate_inventory_errors())
    errors.extend(_ethereum_launch_policy_selector_inventory_errors())
    errors.extend(_ethereum_launch_policy_documentation_inventory_errors())
    errors.extend(_ethereum_core_range_finality_binding_inventory_errors())
    errors.extend(_ethereum_core_message_replay_guard_inventory_errors())
    errors.extend(_ethereum_torii_pinned_message_proof_inventory_errors())
    errors.extend(_ethereum_evm_source_live_production_inventory_errors())
    errors.extend(_ethereum_evm_live_destination_production_inventory_errors())
    errors.extend(_ethereum_route_canary_finalized_receipt_block_inventory_errors())
    errors.extend(_ethereum_evm_block_tag_metadata_inventory_errors())
    errors.extend(_sccp_unready_transparent_proof_config_inventory_errors())
    errors.extend(_contract_smoke_eth_mainnet_network_id_inventory_errors())
    errors.extend(_contract_smoke_evm_production_surface_inventory_errors())
    errors.extend(_native_sccp_no_wasm_readiness_inventory_errors())
    errors.extend(_ethereum_data_collection_no_proxy_inventory_errors())
    errors.extend(_bsc_inbound_adversarial_sdk_test_inventory_errors())
    if summary and _active_launch_blockers(summary):
        errors.append(f"all-lanes summary has active {ACTIVE_LAUNCH_DISPLAY} launch blockers")
    if summary and not _active_launch_release_checklist(
        summary,
        report_native_bundle or _missing_native_evm_prover_bundle_status(),
    ).get("ready"):
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
        if report_native_bundle:
            _check_report_artifact(
                errors,
                manifest_artifacts,
                report_native_bundle.get("artifact"),
                label="readiness report native EVM prover bundle",
            )
            for artifact_field in ("proof_artifact", "proving_key", "verifier_key"):
                _check_report_artifact(
                    errors,
                    manifest_artifacts,
                    report_native_bundle.get(artifact_field),
                    label=f"readiness report native EVM prover {artifact_field}",
                )
            sdk_artifacts = report_native_bundle.get("sdk_artifacts")
            if isinstance(sdk_artifacts, list):
                for row in sdk_artifacts:
                    if not isinstance(row, dict):
                        continue
                    sdk = row.get("sdk", "-")
                    _check_report_artifact(
                        errors,
                        manifest_artifacts,
                        row.get("implementation_artifact"),
                        label=(
                            "readiness report native EVM prover "
                            f"{sdk} implementation_artifact"
                        ),
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
        lane_by_domain: dict[int, dict[str, Any]] = {}
        if isinstance(lanes, list):
            lane_by_domain = {
                lane["domain"]: lane
                for lane in lanes
                if isinstance(lane, dict) and type(lane.get("domain")) is int
            }
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
                domain = row.get("domain")
                lane = lane_by_domain.get(domain)
                enforce_evm_live_tags = (
                    domain == ACTIVE_LAUNCH_DOMAIN
                    or (isinstance(lane, dict) and lane.get("production_ready") is True)
                )
                errors.extend(
                    _cryptographic_evidence_row_schema_errors(
                        row,
                        enforce_evm_live_tags=enforce_evm_live_tags,
                    )
                )
                if domain != ACTIVE_LAUNCH_DOMAIN:
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
        summary_native_bundle = report_native_bundle or _missing_native_evm_prover_bundle_status()
        summary_launch_checklist = _active_launch_release_checklist(
            summary,
            summary_native_bundle,
        )
        summary_launch_ready = bool(summary_launch_checklist.get("ready"))
        if manifest.get("production_ready") != summary_launch_ready:
            errors.append(
                f"manifest production_ready does not match all-lanes summary active {ACTIVE_LAUNCH_DISPLAY} launch readiness"
            )
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
