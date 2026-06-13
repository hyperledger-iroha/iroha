"""Tests for the SCCP release-readiness report renderer."""

from __future__ import annotations

import hashlib
import importlib
import json
import re
import shlex
import subprocess
import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
VERIFY_SCRIPT = ROOT / "scripts" / "sccp_verify_release_bundle.py"
CORRIDOR_SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
ALL_LANES_TESTS = ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py"
PHASES = (
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
SDK_PHASES = ("js-sdk", "python-sdk", "swift-sdk", "kotlin-sdk", "java-android")
EVM_SDK_PHASES = (*SDK_PHASES, "dotnet-sdk")
JS_CALLBACK_HOOK_SYMBOLS = ("witnessProvider", "proveFn", "consensusProvider")
PYTHON_CALLBACK_HOOK_SYMBOLS = ("witness_provider", "prove", "consensus_provider")
EVM_EVIDENCE_SCRIPT_FRAGMENTS = (
    "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
    "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
    "pytests/scripts/sccp_evm_destination_evidence_test.py",
    "pytests/scripts/sccp_evm_live_evidence_test.py",
    "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
    "pytests/scripts/sccp_evm_source_live_evidence_test.py",
)
BSC_MAINNET_SDK_SOURCE_PATHS = {
    "js-sdk": ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
    "python-sdk": ROOT / "python" / "iroha_torii_client" / "sccp.py",
    "swift-sdk": ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
    "kotlin-sdk": (
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt"
    ),
    "java-android": (
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "BscMainnetSccp.java"
    ),
    "dotnet-sdk": (
        (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "BscMainnetSccp.cs"
        ),
        (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "BscMainnetSccpOutbound.cs"
        ),
    ),
}
ETHEREUM_MAINNET_SDK_SOURCE_PATHS = {
    **{
        sdk: path
        for sdk, path in BSC_MAINNET_SDK_SOURCE_PATHS.items()
        if sdk not in {"java-android", "dotnet-sdk"}
    },
    "java-android": (
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java"
    ),
    "dotnet-sdk": (
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs"
    ),
}
BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS = {
    "WebAssembly": re.compile(r"\bWebAssembly\b"),
    "wasm": re.compile(r"\bwasm\b", re.IGNORECASE),
    "snarkjs": re.compile(r"\bsnarkjs\b", re.IGNORECASE),
    "remoteProver": re.compile(r"\bremoteProver\b"),
    "remote prover": re.compile(r"\bremote prover\b", re.IGNORECASE),
    "remote_prover": re.compile(r"\bremote_prover\b", re.IGNORECASE),
    "remote-prover": re.compile(r"\bremote-prover\b", re.IGNORECASE),
    "proverUrl": re.compile(r"\bproverUrl\b"),
    "proverURL": re.compile(r"\bproverURL\b"),
    "prover_url": re.compile(r"\bprover_url\b", re.IGNORECASE),
    "proverEndpoint": re.compile(r"\bproverEndpoint\b"),
    "prover_endpoint": re.compile(r"\bprover_endpoint\b", re.IGNORECASE),
}
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
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
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
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
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
        ROOT / "python" / "iroha_torii_client" / "sccp.py",
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
        ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt",
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
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java",
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
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs",
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
NATIVE_LOCAL_PROVER_SOURCE_GLOBS = {
    "js-sdk": (
        "javascript/iroha_js/src/sccp.js",
        "javascript/iroha_js/src/index.js",
        "javascript/iroha_js/dist/sccp.js",
        "javascript/iroha_js/dist/index.js",
        "javascript/iroha_js/index.d.ts",
    ),
    "python-sdk": (
        "python/iroha_torii_client/sccp.py",
        "python/iroha_torii_client/__init__.py",
    ),
    "swift-sdk": (
        "IrohaSwift/Sources/IrohaSwift/Sccp*.swift",
        "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    ),
    "kotlin-sdk": (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/*.kt",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/BridgeProofSubmitRequest.kt",
    ),
    "java-android": (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/*.java",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/BridgeProofSubmitRequest.java",
    ),
    "dotnet-sdk": (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/*.cs",
    ),
}
NATIVE_EVM_PROVER_BUNDLE_PARSER_MARKERS = {
    "js-sdk": {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "parseEthereumMainnetNativeEvmProverBundleManifest",
            "parseEthereumMainnetNativeEvmProverParityFixture",
            "validateEthereumMainnetNativeEvmProverParityFixture",
            "SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1",
            "rejectDuplicateJsonObjectKeys",
            "JSON.parse(json)",
            "validateEthereumMainnetNativeEvmProverBundle(JSON.parse(json), options)",
            "normalizeCanonicalNativeEvmProverBundleHex32",
            "requireEthereumMainnetNativeEvmProverBundleHashRoleSeparation",
            "requireNativeEvmProverBundleKnownFields",
            "contains duplicate JSON key",
            "canonical lowercase 0x-prefixed 32-byte hex",
            "hashes must be role-separated",
            "contains unknown field",
            "must not use multiple aliases",
            "isCanonicalDecimalText",
            "normalizeNativeEvmProverArtifactPath",
            "must not contain URI schemes or drive prefixes",
            "path contains forbidden prover dependency marker",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "nativeEvmProverBundleRequiredAuditHashKeys",
            "auditHashes missing field",
            "auditHashes.${key}",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "index.js": (
            "parseEthereumMainnetNativeEvmProverBundleManifest",
            "parseEthereumMainnetNativeEvmProverParityFixture",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "parseEthereumMainnetNativeEvmProverBundleManifest",
            "parseEthereumMainnetNativeEvmProverParityFixture",
            "validateEthereumMainnetNativeEvmProverParityFixture",
            "SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1",
            "rejectDuplicateJsonObjectKeys",
            "JSON.parse(json)",
            "normalizeCanonicalNativeEvmProverBundleHex32",
            "requireEthereumMainnetNativeEvmProverBundleHashRoleSeparation",
            "requireNativeEvmProverBundleKnownFields",
            "contains duplicate JSON key",
            "canonical lowercase 0x-prefixed 32-byte hex",
            "hashes must be role-separated",
            "contains unknown field",
            "must not use multiple aliases",
            "isCanonicalDecimalText",
            "normalizeNativeEvmProverArtifactPath",
            "must not contain URI schemes or drive prefixes",
            "path contains forbidden prover dependency marker",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "nativeEvmProverBundleRequiredAuditHashKeys",
            "auditHashes missing field",
            "auditHashes.${key}",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "index.js": (
            "parseEthereumMainnetNativeEvmProverBundleManifest",
            "parseEthereumMainnetNativeEvmProverParityFixture",
        ),
        ROOT / "javascript" / "iroha_js" / "index.d.ts": (
            "parseEthereumMainnetNativeEvmProverBundleManifest",
            "parseEthereumMainnetNativeEvmProverParityFixture",
            "EthereumMainnetNativeEvmProverParityFixture",
            "EthereumMainnetNativeEvmProverParitySdkResult",
            "json: string",
            "proofArtifact?: string",
            "implementationArtifact?: string",
            "crossSdkFixtureParityArtifact?: string",
            "readonly proofArtifact: string",
            "readonly crossSdkFixtureParityArtifact: string",
            "EthereumMainnetNativeEvmProverAuditHashes",
            "no_wasm_no_remote_scan",
        ),
    },
    "swift-sdk": {
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
            "public init(jsonData: Data",
            "public init(jsonString: String",
            "public init(jsonObject object: [String: Any]",
            "EthereumMainnetNativeEvmProverParityFixture",
            "sccpEthNativeEvmProverParityFixtureSchemaV1",
            "nativeProverParityFixture",
            "rejectDuplicateJsonObjectKeys",
            "nativeProverBundle.duplicateJsonKey",
            "manifestBool",
            "CFBooleanGetTypeID",
            "expectedDestinationBindingHash",
            "evmNormalizeNativeEvmProverBundleHex32",
            "evmRequireNativeEvmProverBundleHashRoleSeparation",
            "requireManifestKeys",
            "isCanonicalDecimalText",
            "evmNormalizeNativeEvmProverArtifactPath",
            'value.contains(":")',
            "forbiddenPathMarkers",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "sccpEthNativeEvmProverRequiredAuditHashesV1",
            "manifestStringMap",
            "auditHashes.\\(key)",
        ),
    },
    "kotlin-sdk": {
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt": (
            "fun fromJson(",
            "fun fromJsonBytes(",
            "fun fromMap(",
            "EthereumMainnetNativeEvmProverParityFixture",
            "ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1",
            "nativeProverParityFixture",
            "manifestBoolean",
            "expectedDestinationBindingHash",
            "nativeProverBundle JSON is invalid",
            "normalizeNativeEvmProverBundleHex32",
            "requireNativeEvmProverBundleHashRoleSeparation",
            "requireManifestKeys",
            "contains unknown field",
            "must not use multiple aliases",
            "canonical decimal integer",
            "normalizeNativeEvmProverArtifactPath",
            "must not contain URI schemes or drive prefixes",
            "path contains forbidden prover dependency marker",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1",
            "manifestStringMap",
            "auditHashes.$key",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "client"
        / "JsonParser.kt": (
            "Duplicate JSON object key",
        ),
    },
    "java-android": {
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProver.java": (
            "fromJson(final String json",
            "fromJsonBytes(final byte[] payload",
            "public static EthereumMainnetNativeEvmProverBundle fromMap(",
            "EthereumMainnetNativeEvmProverParityFixture",
            "ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1",
            "nativeProverParityFixture",
            "final Map<String, Object> manifest",
            "manifestBoolean",
            "expectedDestinationBindingHash",
            "nativeProverBundle JSON is invalid",
            "normalizeNativeEvmProverBundleHex32",
            "requireNativeEvmProverBundleHashRoleSeparation",
            "requireManifestKeys",
            "contains unknown field",
            "must not use multiple aliases",
            "canonical decimal integer",
            "normalizeNativeEvmProverArtifactPath",
            "must not contain URI schemes or drive prefixes",
            "path contains forbidden prover dependency marker",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1",
            "manifestStringMap",
            "auditHashes.",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "client"
        / "JsonParser.java": (
            "Duplicate JSON object key",
        ),
    },
    "dotnet-sdk": {
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "FromJson(",
            "FromJsonBytes(",
            "FromJsonElement(",
            "EthereumMainnetNativeEvmProverParityFixture",
            "EthNativeEvmProverParityFixtureSchemaV1",
            "nativeProverParityFixture",
            "ManifestBool",
            "expectedDestinationBindingHash",
            "NormalizeNativeEvmProverBundleHex32",
            "RequireNativeEvmProverBundleHashRoleSeparation",
            "RequireManifestKeys",
            "contains duplicate JSON key",
            "canonical lowercase 0x-prefixed 32-byte hex",
            "hashes must be role-separated",
            "contains unknown field",
            "must not use multiple aliases",
            "canonical decimal integer",
            "NormalizeNativeEvmProverArtifactPath",
            "must not contain URI schemes or drive prefixes",
            "path contains forbidden prover dependency marker",
            "ProofArtifact",
            "ImplementationArtifact",
            "CrossSdkFixtureParityArtifact",
            "EthNativeEvmProverRequiredAuditHashesV1",
            "ManifestStringMap",
            "auditHashes.",
        ),
    },
}
NATIVE_EVM_PROVER_ARTIFACT_VERIFIER_MARKERS = {
    "js-sdk": {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "fromNativeProverBundle",
            "ethereumMainnetSccpConstructorOptionsFromBundleFactoryInput",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "sha256Hex32",
            "verifiedNativeEvmProverArtifacts = new WeakSet()",
            "immutableVerifiedNativeEvmProverArtifacts",
            "local native EVM prover artifact byte verifier",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "auditHashes.cross_sdk_fixture_parity",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParity",
            "parseEthereumMainnetNativeEvmProverParityFixture",
            "artifactResolver",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "nativeProverBundle verifierKeyHash must match destinationBinding",
            "SCCP_NATIVE_EVM_PROVER_FORBIDDEN_ARTIFACT_MARKERS",
            "SCCP_NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES_V1",
            "assertNativeEvmProverArtifactHasProductionSize",
            "must be at least",
            "contains forbidden prover dependency marker",
            "requireEthereumMainnetVerifiedNativeEvmProverArtifactsForRequest",
            "requireEthereumMainnetVerifiedNativeEvmProverArtifactsForProofResult",
            "requireEthereumMainnetNativeProverSelfTest",
            "runEthereumMainnetNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTestFn",
            "ERR_SCCP_ETH_NATIVE_PROVER_SELF_TEST_UNAVAILABLE",
            "Ethereum mainnet SCCP outbound prover requires a native prover self-test hook",
            "ERR_SCCP_ETH_NATIVE_PROVER_ARTIFACTS_UNAVAILABLE",
            "submission requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
            "SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1",
            "requireGroth16Bn254ProofTuple",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            "proofBytes.sourceDomain",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "index.js": (
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "runEthereumMainnetNativeProverSelfTest",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "fromNativeProverBundle",
            "ethereumMainnetSccpConstructorOptionsFromBundleFactoryInput",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "sha256Hex32",
            "implementationBytes are required",
            "local native EVM prover artifact byte verifier",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "auditHashes.cross_sdk_fixture_parity",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParity",
            "parseEthereumMainnetNativeEvmProverParityFixture",
            "artifactResolver",
            "proofArtifact",
            "implementationArtifact",
            "crossSdkFixtureParityArtifact",
            "nativeProverBundle verifierKeyHash must match destinationBinding",
            "SCCP_NATIVE_EVM_PROVER_FORBIDDEN_ARTIFACT_MARKERS",
            "SCCP_NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES_V1",
            "SCCP_NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES_V1",
            "assertNativeEvmProverArtifactHasProductionSize",
            "must be at least",
            "requireEthereumMainnetVerifiedNativeEvmProverArtifactsForRequest",
            "requireEthereumMainnetVerifiedNativeEvmProverArtifactsForProofResult",
            "requireEthereumMainnetNativeProverSelfTest",
            "runEthereumMainnetNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTestFn",
            "ERR_SCCP_ETH_NATIVE_PROVER_SELF_TEST_UNAVAILABLE",
            "Ethereum mainnet SCCP outbound prover requires a native prover self-test hook",
            "ERR_SCCP_ETH_NATIVE_PROVER_ARTIFACTS_UNAVAILABLE",
            "submission requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
            "SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1",
            "requireGroth16Bn254ProofTuple",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            "proofBytes.sourceDomain",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "index.js": (
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "runEthereumMainnetNativeProverSelfTest",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
        ),
        ROOT / "javascript" / "iroha_js" / "index.d.ts": (
            "EthereumMainnetNativeEvmProverArtifactsInput",
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "EthereumMainnetNativeEvmProverArtifactResolverMetadata",
            "EthereumMainnetNativeEvmProverArtifactBundleInput",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "EthereumMainnetSccpNativeProverBundleOptions",
            "static fromNativeProverBundle",
            "artifactResolver?:",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "nativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts",
            "verifiedNativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts",
            "readonly implementationHash: string",
            "implementationArtifact?: string",
            "crossSdkFixtureParityBytes?: BinaryLike",
            "readonly crossSdkFixtureParityHash: string",
            "readonly crossSdkFixtureParity: Readonly<EthereumMainnetNativeEvmProverParityFixture>",
            "readonly crossSdkFixtureParityArtifact: string",
            "EthereumMainnetNativeProverSelfTestContext",
            "EthereumMainnetNativeProverSelfTestFn",
            "runEthereumMainnetNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "verifies native prover artifact bytes against manifest hashes",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "EthereumMainnetSccp.fromNativeProverBundle",
            "pass nativeProverArtifacts to the constructor directly",
            "forgedDescriptorSelfTestCalled",
            "local native EVM prover artifact byte verifier",
            "artifactResolver(path, metadata)",
            "implementationBytes sha256",
            "implementationBytes are required",
            "unverifiedDescriptorMessage",
            "nativeProverArtifacts: { ...verified }",
            "crossSdkFixtureParityBytes is required",
            "crossSdkFixtureParityBytes sha256",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParity",
            "nativeProverBundle verifierKeyHash must match destinationBinding",
            "proofArtifact:",
            "implementationArtifact:",
            "crossSdkFixtureParityArtifact:",
            "flaggedArtifactBytes",
            "tinyProofArtifactBytes",
            "proofArtifactBytes must be at least 65536 bytes",
            "proofArtifactBytes contains forbidden prover dependency marker",
            "verified native EVM prover artifacts",
            "buildEthereumCalldata({ proofResult })",
            "runEthereumMainnetNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTest(context)",
            "missingSelfTestProverCalled",
            "tamperedSelfTestProverCalled",
            "native prover self-test hook",
            "sdkResults.javascript.proofHash must match proofHash",
            "rejects malformed Ethereum Groth16 proof tuples",
            "proofBytes\\.version",
            "BN254 base-field",
            "proofBytes\\.b",
            "proofBytes\\.c",
            "messageId must match",
            "sourceDomain must match",
            "commitmentRoot must match",
            "validates native prover self-test fixtures",
            "nativeProverParityFixture contains duplicate JSON key",
            "nativeProverSelfTestFixture contains duplicate JSON key",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpBscMainnet.test.js": (
            "BSC native prover artifact descriptors must be verifier-owned before callbacks run",
            "unverifiedDescriptorMessage",
            "local native EVM prover artifact byte verifier",
            "mainnetSelfTestCalled",
            "mainnetProverCalled",
            "testnetSelfTestCalled",
            "testnetProverCalled",
            "nativeProverArtifacts: { ...mainnetFixture.nativeProverArtifacts }",
            "nativeProverArtifacts: { ...testnetFixture.nativeProverArtifacts }",
            "runNativeProverSelfTest",
            "proveOutboundToBsc",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "package_dist.test.js": (
            "verifyEthereumMainnetNativeEvmProverArtifacts",
            "verifyEthereumMainnetNativeEvmProverArtifactsFromBundle",
            "fromNativeProverBundle",
            "EthereumMainnetSccpNativeProverBundleOptions",
            "artifactResolver(path)",
            "SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "EthereumMainnetNativeProverSelfTestFn",
            "EthereumMainnetNativeProverSelfTestContext",
            "runEthereumMainnetNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTestBytes",
            "nativeProverSelfTestHash",
            "nativeProverSelfTest",
            "ipfs:proof-artifact.bin",
            "artifacts/eth-mainnet/proof.wasm",
            "proofArtifactBytes must be at least 65536 bytes",
            "provingKeyBytes must be at least 65536 bytes",
            "verifierKeyBytes must be at least 128 bytes",
            "crossSdkFixtureParityBytes must be at least 128 bytes",
            "nativeProverSelfTestBytes must be at least 128 bytes",
            "implementationBytes must be at least 1024 bytes",
        ),
    },
    "swift-sdk": {
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
            "sccpNativeEvmProverArtifactHashAlgorithmV1",
            "func verifiedArtifacts(proofArtifactBytes: Data",
            "func verifiedArtifacts(sdk: String",
            "artifactResolver: (String) throws -> Data",
            "fromNativeProverBundle",
            "nativeProverBundle.verifiedArtifacts",
            "sccpNativeEvmProverArtifactSha256Hex",
            "sccpNativeEvmProverMinProofArtifactBytesV1",
            "sccpNativeEvmProverMinProvingKeyBytesV1",
            "sccpNativeEvmProverMinVerifierKeyBytesV1",
            "sccpNativeEvmProverMinImplementationBytesV1",
            "sccpNativeEvmProverMinSupportArtifactBytesV1",
            "sccpNativeEvmProverRequireProductionArtifactSize",
            ".minBytes",
            "implementationBytes",
            "invalidPublicInputs(\"implementationBytes\")",
            "nativeProverArtifacts.implementationHash",
            "nativeProverArtifacts.verifierKeyHash",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "auditHashes[\"cross_sdk_fixture_parity\"]",
            "EthereumMainnetNativeEvmProverParityFixture(",
            "crossSdkFixtureParityArtifact",
            "implementationArtifact",
            "nativeProverBundle.verifierKeyHash",
            "sccpNativeEvmProverForbiddenArtifactMarkers",
            "sccpNativeEvmProverRejectForbiddenArtifactMarkers",
            "requireVerifiedNativeProverArtifacts",
            "NativeProverSelfTestFunction",
            "requireNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTestFunction",
            "nativeProverSelfTestResult",
            "proofResult: EvmSccpProofResult",
            "nativeProverArtifacts",
            "requireEvmGroth16ProofTuple",
            "sccpGroth16Bn254ProofTupleInvalidField",
            "evmBn254BaseFieldModulus",
            "proofBytes.sourceDomain",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpGroth16Bn254.swift": (
            "func sccpGroth16Bn254ProofTupleInvalidField",
            "proofBytes.version",
            "proofBytes.b",
            "proofBytes.c",
            "proofBytes.sourceDomain",
            "sccpGroth16Bn254BaseFieldModulus",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "verifiedBundle.verifiedArtifacts",
            "verifiedArtifacts(sdk: \"swift\")",
            "EthereumMainnetSccp.fromNativeProverBundle",
            "factoryBoundRequest",
            "sccpNativeEvmProverArtifactHashAlgorithmV1",
            "implementationBytes",
            "nativeProverArtifacts.implementationHash",
            "nativeProverArtifacts.verifierKeyHash",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParityArtifact",
            "swiftImplementationArtifact",
            "Data(\"{}\".utf8)",
            "nativeProverBundle.verifierKeyHash",
            "nativeEvmProverArtifactBytes",
            "hashConsistentNativeEvmProverBundle",
            "proofArtifactBytes.minBytes",
            "provingKeyBytes.minBytes",
            "verifierKeyBytes.minBytes",
            "crossSdkFixtureParityBytes.minBytes",
            "nativeProverSelfTestBytes.minBytes",
            "implementationBytes.minBytes",
            "proof.wasm",
            "flaggedArtifactBytes",
            "proofArtifactBytes.forbiddenMarker",
            "Ethereum outbound prover must require verified native artifacts",
            ".invalidPublicInputs(\"nativeProverArtifacts\")",
            "runNativeProverSelfTest",
            "artifactBoundSelfTestCalled",
            "missingSelfTestHookProverCalled",
            "driftingSelfTestHookProverCalled",
            ".invalidPublicInputs(\"nativeProverSelfTestFunction\")",
            ".invalidPublicInputs(\"nativeProverSelfTestResult\")",
            "testRejectsMalformedEvmGroth16ProofTuple",
            "proofBytes.version",
            "proofBytes.a.x",
            "proofBytes.b",
            "proofBytes.c",
            "proofBytes.sourceDomain",
            "proofBytes.commitmentRoot",
            "nativeProverParityFixture.duplicateJsonKey",
            "nativeProverSelfTestFixture.duplicateJsonKey",
        ),
    },
    "kotlin-sdk": {
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt": (
            "NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "fun verifiedArtifacts(",
            "NativeEvmProverArtifactResolver",
            "artifactResolver: NativeEvmProverArtifactResolver",
            "fun fromNativeProverBundle(",
            "nativeProverBundle.verifiedArtifacts",
            'MessageDigest.getInstance("SHA-256")',
            "NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES_V1",
            "requireNativeEvmProverProductionArtifactSize",
            "must be at least",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "auditHashes[\"cross_sdk_fixture_parity\"]",
            "EthereumMainnetNativeEvmProverParityFixture.fromJsonBytes",
            "crossSdkFixtureParityArtifact",
            "implementationArtifact",
            "nativeProverBundle.verifierKeyHash must match destinationBinding",
            "nativeEvmProverForbiddenArtifactMarkers",
            "contains forbidden prover dependency marker",
            "requireVerifiedNativeProverArtifacts",
            "EthereumMainnetNativeProverSelfTest",
            "requireNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "verified native EVM prover artifacts",
            "submission requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
            "GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1",
            "requireGroth16ProofTuple",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            "proofBytes.sourceDomain",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "verifiedBundle.verifiedArtifacts",
            "verifiedBundle.verifiedArtifacts(\"kotlin\")",
            "EthereumMainnetSccp.fromNativeProverBundle",
            "factoryBoundRequest",
            "NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParityArtifact",
            "kotlinImplementationPath",
            "\"{}\".toByteArray()",
            "nativeProverBundle.verifierKeyHash",
            "nativeEvmProverArtifactBytes",
            "proofArtifactBytes must be at least 65536 bytes",
            "provingKeyBytes must be at least 65536 bytes",
            "verifierKeyBytes must be at least 128 bytes",
            "crossSdkFixtureParityBytes must be at least 128 bytes",
            "nativeProverSelfTestBytes must be at least 128 bytes",
            "implementationBytes must be at least 1024 bytes",
            "proof.wasm",
            "flaggedArtifactBytes",
            "proofArtifactBytes contains forbidden",
            "verified native EVM prover artifacts",
            "buildEthereumCalldata(EvmSccpSubmissionInput(result))",
            "runNativeProverSelfTest",
            "artifactBoundSelfTestCalled",
            "missingSelfTestHookProverCalled",
            "driftingSelfTestHookProverCalled",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "rejectsMalformedGroth16ProofTuple",
            "proofBytes.version",
            "BN254 base-field",
            "proofBytes.b",
            "proofBytes.c",
            "messageId must match",
            "sourceDomain must match",
            "commitmentRoot must match",
            "EthereumMainnetNativeEvmProverSelfTestFixture.fromJson",
            "Duplicate JSON object key: schema",
        ),
    },
    "java-android": {
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProver.java": (
            "NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts",
            "NativeEvmProverArtifactResolver",
            "artifactResolver.resolveArtifact",
            'MessageDigest.getInstance("SHA-256")',
            "NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_VERIFIER_KEY_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES_V1",
            "NATIVE_EVM_PROVER_MIN_SUPPORT_ARTIFACT_BYTES_V1",
            "requireNativeEvmProverProductionArtifactSize",
            "must be at least",
            "implementationBytes sha256",
            "implementationBytes are required",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "auditHashes.get(\"cross_sdk_fixture_parity\")",
            "EthereumMainnetNativeEvmProverParityFixture.fromJsonBytes",
            "crossSdkFixtureParityArtifact",
            "implementationArtifact",
            "nativeProverBundle.verifierKeyHash must match destinationBinding",
            "NATIVE_EVM_PROVER_FORBIDDEN_ARTIFACT_MARKERS",
            "contains forbidden prover dependency marker",
            "GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1",
            "requireGroth16ProofTuple",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            "proofBytes.sourceDomain",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java": (
            "fromNativeProverBundle",
            "nativeProverBundle.verifiedArtifacts",
            "requireVerifiedNativeProverArtifacts",
            "NativeProverSelfTest",
            "requireNativeProverSelfTest",
            "runNativeProverSelfTest",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "verified native EVM prover artifacts",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParity()",
            "auditHashes().get(\"cross_sdk_fixture_parity\")",
            "submission requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "verifiedBundle.verifiedArtifacts",
            "javaImplementationArtifact",
            "EthereumMainnetSccp.fromNativeProverBundle",
            "factoryBoundRequest",
            "NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "crossSdkFixtureParityHash",
            "crossSdkFixtureParityArtifact",
            "artifactBytesByPath",
            "\"{}\".getBytes(StandardCharsets.UTF_8)",
            "nativeProverBundle.verifierKeyHash",
            "nativeEvmProverArtifactBytes",
            "NativeEvmVerifierFixture",
            "proofArtifactBytes must be at least 65536 bytes",
            "provingKeyBytes must be at least 65536 bytes",
            "verifierKeyBytes must be at least 128 bytes",
            "crossSdkFixtureParityBytes must be at least 128 bytes",
            "nativeProverSelfTestBytes must be at least 128 bytes",
            "implementationBytes must be at least 1024 bytes",
            "proof.wasm",
            "flaggedArtifactBytes",
            "proofArtifactBytes contains forbidden",
            "verified native EVM prover artifacts",
            "buildEthereumCalldata(new EvmSccpProver.SubmissionInput(result))",
            "runNativeProverSelfTest",
            "artifactBoundSelfTestCalled",
            "missingSelfTestHookProverCalled",
            "driftingSelfTestHookProverCalled",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "rejectsMalformedGroth16ProofTuple",
            "proofBytes.version",
            "BN254 base-field",
            "proofBytes.b",
            "proofBytes.c",
            "messageId must match",
            "sourceDomain must match",
            "commitmentRoot must match",
            "EthereumMainnetNativeEvmProverSelfTestFixture.fromJson",
            "Duplicate JSON object key: schema",
        ),
    },
    "dotnet-sdk": {
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "NativeEvmProverArtifactHashAlgorithmV1",
            "VerifiedArtifacts(",
            "ProveOutboundToEthereumFromNativeProverBundleAsync",
            "BuildEthereumCalldataFromNativeProverBundle",
            "SubmitOutboundToEthereumFromNativeProverBundleAsync",
            "nativeProverBundle.VerifiedArtifacts",
            "Func<string, byte[]> artifactResolver",
            "artifactResolver(ProofArtifact)",
            "SHA256.HashData",
            "NativeEvmProverMinProofArtifactBytesV1",
            "NativeEvmProverMinProvingKeyBytesV1",
            "NativeEvmProverMinVerifierKeyBytesV1",
            "NativeEvmProverMinImplementationBytesV1",
            "NativeEvmProverMinSupportArtifactBytesV1",
            "RequireNativeEvmProverProductionArtifactSize",
            "must be at least",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "CrossSdkFixtureParityHash",
            "AuditHashes[\"cross_sdk_fixture_parity\"]",
            "EthereumMainnetNativeEvmProverParityFixture.FromJsonBytes",
            "CrossSdkFixtureParityArtifact",
            "ImplementationArtifact",
            "nativeProverBundle.verifierKeyHash must match destinationBinding",
            "NativeEvmProverForbiddenArtifactMarkers",
            "contains forbidden prover dependency marker",
            "RequireVerifiedNativeProverArtifacts",
            "IEthereumMainnetNativeProverSelfTest",
            "RequireNativeProverSelfTestAsync",
            "RunNativeProverSelfTestAsync",
            "NativeProverSelfTestResultEquals",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "EthereumMainnetNativeEvmProverArtifacts nativeProverArtifacts",
            "BuildEthereumCalldataUnchecked",
            "Ethereum mainnet calldata requires verified native EVM prover artifacts",
            "nativeProverArtifacts artifact hashes must match proofResult",
            "RequireGroth16Bn254ProofTuple",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            ".sourceDomain must match",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "verifiedBundle.VerifiedArtifacts",
            "verifiedBundle.VerifiedArtifacts(",
            "ProveOutboundToEthereumFromNativeProverBundleAsync",
            "BuildEthereumCalldataFromNativeProverBundle",
            "SubmitOutboundToEthereumFromNativeProverBundleAsync",
            "factoryBoundProver",
            "resolverSubmitter",
            "CrossSdkFixtureParityArtifact",
            "dotnetImplementationArtifact",
            "NativeEvmProverArtifactHashAlgorithmV1",
            "implementationBytes sha256",
            "implementationBytes are required",
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            "crossSdkFixtureParityBytes",
            "CrossSdkFixtureParityHash",
            "Encoding.UTF8.GetBytes(\"{}\")",
            "nativeProverBundle.verifierKeyHash",
            "NativeEvmProverArtifactBytes",
            "HashConsistentNativeEvmProverBundle",
            "proofArtifactBytes must be at least 65536 bytes",
            "provingKeyBytes must be at least 65536 bytes",
            "verifierKeyBytes must be at least 128 bytes",
            "crossSdkFixtureParityBytes must be at least 128 bytes",
            "nativeProverSelfTestBytes must be at least 128 bytes",
            "implementationBytes must be at least 1024 bytes",
            "proof.wasm",
            "flaggedArtifactBytes",
            "proofArtifactBytes contains forbidden",
            "artifactBoundProver",
            "verified native EVM prover artifacts",
            "NativeProverSelfTestStub",
            "RunNativeProverSelfTestAsync",
            "artifactBoundSelfTest",
            "missingSelfTestHookProver",
            "driftingSelfTestHookProver",
            "nativeProverSelfTest runner",
            "nativeProverSelfTest result",
            "OutboundProofPathRejectsCrossLaneAndMalformedProofs",
            "wrongMessageId",
            "wrongSourceDomain",
            "badG1Point",
            "EthereumMainnetNativeEvmProverSelfTestFixture.FromJson",
            "duplicate JSON key: schema",
        ),
    },
}
FORBIDDEN_PROVER_DEPENDENCY_SAMPLES = {
    "WebAssembly": "const runtime = WebAssembly;",
    "wasm": "const backend = 'wasm';",
    "snarkjs": "import snarkjs from 'snarkjs';",
    "remoteProver": "const remoteProver = true;",
    "remote prover": "use a remote prover fallback",
    "proverUrl": "const proverUrl = 'https://prover.invalid';",
    "proverEndpoint": "const proverEndpoint = '/prove';",
}


def phase_command_lines(fragments) -> list[str]:
    """Render required fragments as realistic production-corridor commands."""

    fragments = tuple(fragments)
    if not fragments:
        return []

    lines: list[str] = []
    if any(fragment.startswith("-m pytest") for fragment in fragments):
        tests: list[str] = []
        for fragment in fragments:
            if fragment.startswith("-m pytest"):
                parts = shlex.split(fragment)
                if "-q" in parts:
                    tests.extend(parts[parts.index("-q") + 1 :])
            elif fragment.startswith(("pytests/", "python/")):
                tests.append(fragment)
        return ["+ python3 -m pytest -q " + " ".join(dict.fromkeys(tests))]

    if any(
        fragment.startswith("--test ") or fragment.startswith("javascript/")
        for fragment in fragments
    ):
        tests: list[str] = []
        for fragment in fragments:
            if fragment.startswith("--test "):
                tests.extend(shlex.split(fragment)[1:])
            elif fragment.startswith("javascript/"):
                tests.append(fragment)
        return ["+ node --test " + " ".join(dict.fromkeys(tests))]

    if any(
        fragment.startswith("swift test ") or fragment.startswith("ToriiClientTests/")
        for fragment in fragments
    ):
        for fragment in fragments:
            if fragment.startswith("swift test "):
                lines.append(f"+ {fragment}")
            elif fragment.startswith("ToriiClientTests/"):
                lines.append(
                    "+ swift test --filter "
                    f"{fragment} --disable-swift-testing"
                )
        return lines

    has_android_fragments = any(
        fragment.startswith("ANDROID_HARNESS_MAINS=")
        or fragment.startswith("org.hyperledger.iroha.android.sccp.")
        or fragment.startswith("./gradlew :core:test")
        for fragment in fragments
    )
    if has_android_fragments:
        if "java -version" in fragments:
            lines.append("+ java -version")
        android_fragments = [
            fragment for fragment in fragments if fragment != "java -version"
        ]
        harness_assignment = next(
            (
                fragment
                for fragment in android_fragments
                if fragment.startswith("ANDROID_HARNESS_MAINS=")
            ),
            None,
        )
        harness_classes = [
            fragment
            for fragment in android_fragments
            if fragment.startswith("org.hyperledger.iroha.android.sccp.")
        ]
        gradle_fragments = [
            fragment
            for fragment in android_fragments
            if fragment.startswith("./gradlew :core:test")
        ]
        if harness_assignment is not None or harness_classes:
            harness_value = harness_assignment or "ANDROID_HARNESS_MAINS="
            for harness_class in harness_classes:
                if harness_class not in harness_value:
                    harness_value += "," + harness_class
            lines.append(
                f"+ env {harness_value} ./gradlew :core:test --console=plain "
                "--tests org.hyperledger.iroha.android.GradleHarnessTests"
            )
        for fragment in gradle_fragments:
            if "GradleHarnessTests" in fragment and (
                harness_assignment is not None or harness_classes
            ):
                continue
            lines.append(f"+ {fragment}")
        return lines

    if any(
        fragment.startswith("./gradlew :core-jvm:test")
        or fragment.startswith("org.hyperledger.iroha.sdk.sccp.")
        for fragment in fragments
    ) or fragments == ("java -version",):
        if "java -version" in fragments:
            lines.append("+ java -version")
        gradle_fragments = [
            fragment for fragment in fragments if fragment != "java -version"
        ]
        if gradle_fragments:
            gradle_test_classes = [
                fragment
                for fragment in gradle_fragments
                if fragment.startswith("org.hyperledger.iroha.sdk.sccp.")
            ]
            gradle_command = "+ ./gradlew :core-jvm:test --console=plain"
            if any(fragment.startswith("./gradlew ") for fragment in gradle_fragments):
                gradle_command = "+ " + " ".join(
                    fragment
                    for fragment in gradle_fragments
                    if not fragment.startswith("org.hyperledger.iroha.sdk.sccp.")
                )
            else:
                gradle_command += " " + " ".join(
                    f"--tests {fragment}" for fragment in gradle_fragments
                )
            for test_class in gradle_test_classes:
                if test_class not in gradle_command:
                    gradle_command += f" --tests {test_class}"
            lines.append(gradle_command)
        return lines

    if any(
        fragment.startswith("dotnet test ") or fragment.startswith("FullyQualifiedName")
        for fragment in fragments
    ):
        test_command = next(
            (
                fragment
                for fragment in fragments
                if fragment.startswith("dotnet test ")
            ),
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        )
        filter_fragment = next(
            (
                fragment
                for fragment in fragments
                if fragment.startswith("FullyQualifiedName")
            ),
            None,
        )
        if filter_fragment is not None:
            test_command += " --filter " + filter_fragment
        test_command += " --nologo"
        return [f"+ {test_command}"]

    if any(
        fragment.endswith(".test.mjs")
        or fragment.startswith("--check ")
        or fragment.startswith("bash scripts/")
        for fragment in fragments
    ):
        mjs_tests = [fragment for fragment in fragments if fragment.endswith(".test.mjs")]
        if mjs_tests:
            lines.append("+ node --test " + " ".join(mjs_tests))
        lines.extend(
            "+ node " + fragment
            for fragment in fragments
            if fragment.startswith("--check ")
        )
        lines.extend(
            f"+ {fragment}"
            for fragment in fragments
            if fragment.startswith("bash scripts/")
        )
        return lines

    return [f"+ {fragment}" for fragment in fragments]


def corridor_evidence_script_tests() -> tuple[str, ...]:
    """Return pytest files listed by the production corridor evidence phase."""

    script = CORRIDOR_SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"phase_evidence_scripts\(\) \{\n"
        r"\s+local tests=\(\n"
        r"(?P<body>.*?)"
        r"\n\s+\)\n"
        r"(?P<runner>\s+run_cmd .*\bpytest -q \"\$\{tests\[@\]\}\")",
        script,
        re.DOTALL,
    )
    assert match is not None, "phase_evidence_scripts test inventory not found"
    tests = []
    for raw_line in match.group("body").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        tests.append(line)
    return tuple(tests)


def corridor_android_harness_mains() -> tuple[str, ...]:
    """Return Java/Android harness mains listed by the production corridor."""

    script = CORRIDOR_SCRIPT.read_text(encoding="utf-8")
    match = re.search(r'android_harness_mains="(?P<body>[^"]+)"', script)
    assert match is not None, "java-android harness inventory not found"
    return tuple(match.group("body").split(","))


def complete_corridor_log(phases: tuple[str, ...] = PHASES) -> str:
    """Return a synthetic successful SCCP production-corridor transcript."""

    report = load_report_module()
    lines: list[str] = []
    for phase in phases:
        lines.append(f"==> SCCP production corridor: {phase}")
        lines.extend(
            phase_command_lines(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase])
        )
        lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase])
    return "\n".join(
        [*lines, ""]
    ) + "SCCP production corridor completed.\n"


def complete_corridor_log_with_success_before_command(
    forged_phase: str,
    phases: tuple[str, ...] = PHASES,
) -> str:
    """Return a full corridor transcript with one forged phase success order."""

    report = load_report_module()
    lines: list[str] = []
    for phase in phases:
        lines.append(f"==> SCCP production corridor: {phase}")
        if phase == forged_phase:
            lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase])
            lines.extend(
                phase_command_lines(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase])
            )
            continue
        lines.extend(
            phase_command_lines(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase])
        )
        lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase])
    return "\n".join(
        [*lines, ""]
    ) + "SCCP production corridor completed.\n"


def native_local_prover_source_paths() -> dict[str, list[Path]]:
    """Return SDK source files that must not depend on WASM or remote provers."""

    paths_by_sdk: dict[str, list[Path]] = {}
    for sdk, patterns in NATIVE_LOCAL_PROVER_SOURCE_GLOBS.items():
        paths: list[Path] = []
        for pattern in patterns:
            matches = sorted(ROOT.glob(pattern))
            if not matches:
                raise AssertionError(f"{sdk} native SCCP source glob matched no files: {pattern}")
            paths.extend(path for path in matches if path.is_file())
        paths_by_sdk[sdk] = paths
    return paths_by_sdk


def source_region(path: Path, start_marker: str, end_marker: str) -> str:
    """Return the source region delimited by two stable markers."""

    source = path.read_text(encoding="utf-8")
    start = source.find(start_marker)
    if start == -1:
        raise AssertionError(
            f"{path.relative_to(ROOT)} missing start marker: {start_marker}"
        )
    end = source.find(end_marker, start + len(start_marker))
    if end == -1:
        raise AssertionError(
            f"{path.relative_to(ROOT)} missing end marker: {end_marker}"
        )
    return source[start:end]


def write_downloaded_phase_artifacts(tmp_path: Path) -> Path:
    """Write synthetic downloaded CI artifacts for every corridor phase."""

    artifact_root = tmp_path / "phase-artifacts"
    for phase in PHASES:
        phase_dir = artifact_root / f"sccp-production-corridor-{phase}"
        phase_dir.mkdir(parents=True)
        (phase_dir / f"{phase}.log").write_text(
            complete_corridor_log((phase,)),
            encoding="utf-8",
        )
    return artifact_root


def load_all_lanes_helpers():
    """Load all-lanes fixture helpers without importing pytest test collection state."""

    spec = spec_from_file_location(
        "sccp_all_lanes_evidence_test_helpers",
        ALL_LANES_TESTS,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_report_module():
    """Load the readiness report module for structured helper assertions."""

    spec = spec_from_file_location("sccp_release_readiness_report_module", SCRIPT)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_verify_helpers():
    """Load release-bundle verifier helpers without running its CLI."""

    spec = spec_from_file_location(
        "sccp_release_bundle_verify_helpers_for_readiness",
        VERIFY_SCRIPT,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_release_readiness_report_redacts_verifier_helper_failures(
    monkeypatch,
) -> None:
    """Readiness source-inventory wrappers must not echo helper exceptions."""

    report = load_report_module()

    def fail_loader():
        raise RuntimeError("secret-token /tmp/operator/private/path")

    monkeypatch.setattr(report, "_load_release_bundle_verify_helpers", fail_loader)
    gate_helpers = sorted(
        (name, helper)
        for name, helper in vars(report).items()
        if name.endswith("_gate_inventory_errors") and callable(helper)
    )

    assert len(gate_helpers) >= 60
    for name, helper in gate_helpers:
        errors = helper()
        rendered = "\n".join(errors)

        assert errors, name
        assert "cannot run release-bundle verifier helper" in rendered
        assert "cannot run release-bundle verifier helper:" not in rendered
        assert "secret-token" not in rendered
        assert "/tmp/operator" not in rendered
        assert "RuntimeError" not in rendered
        assert "Traceback" not in rendered


def active_evm_live_chain_id(report):
    """Return the decimal EVM chain id required by the active launch lane."""

    return {
        "eth": "1",
        "bsc": "56",
    }.get(report.ACTIVE_LAUNCH_CHAIN)


def test_active_launch_evm_live_metadata_requires_canonical_decimal_chain_id() -> None:
    """Active launch metadata must not accept noncanonical chain ids."""

    report = load_report_module()
    label = f"domain {report.ACTIVE_LAUNCH_DOMAIN} ({report.ACTIVE_LAUNCH_CHAIN})"
    expected_chain_id = active_evm_live_chain_id(report)
    assert expected_chain_id is not None
    expected_source_blocker = (
        f"{label}: {report.ACTIVE_LAUNCH_DISPLAY} source live eth_chainId "
        f"must be canonical decimal chain id {expected_chain_id}"
    )
    expected_destination_blocker = (
        f"{label}: {report.ACTIVE_LAUNCH_DISPLAY} destination live eth_chainId "
        f"must be canonical decimal chain id {expected_chain_id}"
    )

    valid_lane = {
        "evm_live_metadata": {
            "source_rpc_chain_id": expected_chain_id,
            "source_block_tag": "finalized",
            "destination_rpc_chain_id": expected_chain_id,
            "destination_block_tag": "finalized",
        },
    }
    assert report._active_launch_evm_live_metadata_blockers(label, valid_lane) == []

    noncanonical_chain_ids = (
        "0x1",
        "01",
        " 1",
        "1 ",
        "+1",
        "1.0",
        "\uff11",
        "\u0661",
        1,
    )
    for noncanonical_chain_id in noncanonical_chain_ids:
        lane = {
            "evm_live_metadata": {
                "source_rpc_chain_id": noncanonical_chain_id,
                "source_block_tag": "finalized",
                "destination_rpc_chain_id": noncanonical_chain_id,
                "destination_block_tag": "finalized",
            },
        }

        blockers = report._active_launch_evm_live_metadata_blockers(label, lane)

        assert expected_source_blocker in blockers
        assert expected_destination_blocker in blockers

    for field, expected_blocker, absent_blocker in (
        (
            "source_rpc_chain_id",
            expected_source_blocker,
            expected_destination_blocker,
        ),
        (
            "destination_rpc_chain_id",
            expected_destination_blocker,
            expected_source_blocker,
        ),
    ):
        for noncanonical_chain_id in noncanonical_chain_ids:
            lane = {
                "evm_live_metadata": {
                    "source_rpc_chain_id": expected_chain_id,
                    "source_block_tag": "finalized",
                    "destination_rpc_chain_id": expected_chain_id,
                    "destination_block_tag": "finalized",
                },
            }
            lane["evm_live_metadata"][field] = noncanonical_chain_id

            blockers = report._active_launch_evm_live_metadata_blockers(label, lane)

            assert expected_blocker in blockers
            assert absent_blocker not in blockers


def fixed_hex32(seed: int) -> str:
    """Return a non-zero 32-byte hex fixture."""

    return "0x" + f"{seed % 256:02x}" * 32


def write_native_evm_prover_bundle(
    tmp_path: Path,
    evidence_path: Path,
    *,
    overrides: dict[str, object] | None = None,
) -> Path:
    """Write a synthetic audited native EVM prover bundle manifest."""

    report = load_report_module()
    evidence = report._load_evidence_summary([evidence_path])
    active_lane = report._active_launch_lane(evidence)
    assert active_lane is not None
    destination_binding = active_lane["destination_binding"][
        "destination_binding_hash"
    ]
    artifact_dir = tmp_path / "native-prover-artifacts"
    artifact_dir.mkdir(exist_ok=True)

    def write_artifact(name: str, content: bytes) -> tuple[str, str]:
        path = artifact_dir / name
        path.write_bytes(content)
        return (
            path.relative_to(tmp_path).as_posix(),
            "0x" + hashlib.sha256(content).hexdigest(),
        )

    def native_payload(
        label: str,
        size: int = report.NATIVE_EVM_PROVER_MIN_PAYLOAD_BYTES,
    ) -> bytes:
        content = (f"{label}\n").encode("utf-8")
        repeats = size // len(content) + 1
        return (content * repeats)[:size]

    proof_artifact, proof_artifact_hash = write_artifact(
        "proof-artifact.bin",
        native_payload(
            "ethereum mainnet sccp proof artifact v1",
            report.NATIVE_EVM_PROVER_MIN_PROOF_ARTIFACT_BYTES,
        ),
    )
    proving_key, proving_key_hash = write_artifact(
        "proving-key.bin",
        native_payload(
            "ethereum mainnet sccp proving key v1",
            report.NATIVE_EVM_PROVER_MIN_PROVING_KEY_BYTES,
        ),
    )
    verifier_key, verifier_key_hash = write_artifact(
        "verifier-key.bin",
        native_payload("ethereum mainnet sccp verifier key v1"),
    )
    sdk_artifacts = []
    for sdk, implementation in sorted(
        report.NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS.items()
    ):
        implementation_artifact, implementation_hash = write_artifact(
            f"{sdk}-implementation.bin",
            native_payload(
                "ethereum mainnet sccp "
                f"{sdk} {implementation} implementation v1",
                report.NATIVE_EVM_PROVER_MIN_IMPLEMENTATION_BYTES,
            ),
        )
        sdk_artifacts.append(
            {
                "sdk": sdk,
                "implementation": implementation,
                "prover_artifact_hash": proof_artifact_hash,
                "proving_key_hash": proving_key_hash,
                "implementation_artifact": implementation_artifact,
                "implementation_hash": implementation_hash,
            }
        )
    parity_vector = {
        "schema": report.NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA,
        "domain": report.ACTIVE_LAUNCH_DOMAIN,
        "chain": report.ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "proof_artifact_hash": proof_artifact_hash,
        "proving_key_hash": proving_key_hash,
        "verifier_key_hash": verifier_key_hash,
        "destination_binding_hash": destination_binding,
        "receipt_proof_hash": fixed_hex32(0xB1),
        "source_proof_hash": fixed_hex32(0xB2),
        "public_signal_words": [fixed_hex32(0xC0 + index) for index in range(9)],
        "calldata_hash": fixed_hex32(0xB3),
        "torii_submit_payload_hash": fixed_hex32(0xB4),
    }
    parity_vector["sdk_results"] = {
        sdk: {
            "receipt_proof_hash": parity_vector["receipt_proof_hash"],
            "source_proof_hash": parity_vector["source_proof_hash"],
            "destination_binding_hash": parity_vector["destination_binding_hash"],
            "public_signal_words": parity_vector["public_signal_words"],
            "calldata_hash": parity_vector["calldata_hash"],
            "torii_submit_payload_hash": parity_vector["torii_submit_payload_hash"],
        }
        for sdk in sorted(report.NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS)
    }
    parity_artifact, parity_hash = write_artifact(
        "cross-sdk-fixture-parity.json",
        (json.dumps(parity_vector, indent=2, sort_keys=True) + "\n").encode(
            "utf-8"
        ),
    )
    self_test_vector = {
        "schema": report.NATIVE_EVM_PROVER_SELF_TEST_SCHEMA,
        "domain": report.ACTIVE_LAUNCH_DOMAIN,
        "chain": report.ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "proof_artifact_hash": proof_artifact_hash,
        "proving_key_hash": proving_key_hash,
        "verifier_key_hash": verifier_key_hash,
        "destination_binding_hash": destination_binding,
        "request_hash": fixed_hex32(0xD1),
        "witness_hash": fixed_hex32(0xD2),
        "source_proof_hash": fixed_hex32(0xD3),
        "proof_hash": fixed_hex32(0xD4),
        "public_signal_words": [fixed_hex32(0xE0 + index) for index in range(9)],
        "calldata_hash": fixed_hex32(0xD5),
        "torii_submit_payload_hash": fixed_hex32(0xD6),
    }
    self_test_vector["sdk_results"] = {
        sdk: {
            "request_hash": self_test_vector["request_hash"],
            "witness_hash": self_test_vector["witness_hash"],
            "source_proof_hash": self_test_vector["source_proof_hash"],
            "proof_hash": self_test_vector["proof_hash"],
            "public_signal_words": self_test_vector["public_signal_words"],
            "calldata_hash": self_test_vector["calldata_hash"],
            "torii_submit_payload_hash": self_test_vector["torii_submit_payload_hash"],
        }
        for sdk in sorted(report.NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS)
    }
    self_test_artifact, self_test_hash = write_artifact(
        "native-prover-self-test.json",
        (json.dumps(self_test_vector, indent=2, sort_keys=True) + "\n").encode(
            "utf-8"
        ),
    )
    audit_hashes = {
        key: fixed_hex32(0xA1 + index)
        for index, key in enumerate(report.NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES)
    }
    audit_hashes["cross_sdk_fixture_parity"] = parity_hash
    audit_hashes["native_prover_self_test"] = self_test_hash
    payload: dict[str, object] = {
        "schema": report.NATIVE_EVM_PROVER_BUNDLE_SCHEMA,
        "bundle_id": report.NATIVE_EVM_PROVER_BUNDLE_ID,
        "domain": report.ACTIVE_LAUNCH_DOMAIN,
        "chain": report.ACTIVE_LAUNCH_CHAIN,
        "proof_backend": "evm-groth16-bn254-v1",
        "proof_artifact": proof_artifact,
        "proof_artifact_hash": proof_artifact_hash,
        "proving_key": proving_key,
        "proving_key_hash": proving_key_hash,
        "verifier_key": verifier_key,
        "verifier_key_hash": verifier_key_hash,
        "destination_binding_hash": destination_binding,
        "no_wasm": True,
        "remote_prover_required": False,
        "browser_implementation": "pure-typescript",
        "native_sdk_artifacts": sdk_artifacts,
        "cross_sdk_fixture_parity_artifact": parity_artifact,
        "native_prover_self_test_artifact": self_test_artifact,
        "audit_hashes": audit_hashes,
    }
    if overrides:
        payload.update(overrides)
    path = tmp_path / "native-evm-prover-bundle.json"
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def write_complete_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write a complete synthetic all-lanes evidence bundle for report tests."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    report = load_report_module()
    records = helpers.complete_bundle(evidence_module)
    evm_chain_id = active_evm_live_chain_id(report)
    if evm_chain_id is not None:
        for record in records["sccp_source_verifier_materials"]:
            if record.get("source_domain") == report.ACTIVE_LAUNCH_DOMAIN:
                record["_comment_evm_source_rpc_chain_id"] = evm_chain_id
                record["_comment_evm_source_block_tag"] = "finalized"
        for record in records["sccp_destination_rollouts"]:
            if record.get("domain") == report.ACTIVE_LAUNCH_DOMAIN:
                record["_comment_evm_rpc_chain_id"] = evm_chain_id
                record["_comment_evm_block_tag"] = "finalized"
    evidence = tmp_path / "complete.toml"
    evidence_payload = helpers.render_records(records)
    evidence.write_text(evidence_payload, encoding="utf-8")
    return evidence, evidence_payload


def test_release_readiness_active_launch_policy_is_ethereum_mainnet() -> None:
    """The release-readiness script must advertise the Ethereum launch lane."""

    report = load_report_module()

    assert report.ACTIVE_LAUNCH_DOMAIN == 1
    assert report.ACTIVE_LAUNCH_CHAIN == "eth"
    assert report.ACTIVE_LAUNCH_POLICY == "EthereumMainnetLane"
    assert report.ACTIVE_LAUNCH_DISPLAY == "Ethereum mainnet"
    assert (
        report.ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE
        == "evm_message_proof_accepted_transaction"
    )


def test_release_readiness_submission_surfaces_match_supported_launch_scope() -> None:
    """Public submission surfaces must match the supported SCCP launch lanes."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }

    surfaces = report._submission_surfaces(passed_phases)

    assert [surface["lanes"] for surface in surfaces] == [
        "eth,bsc",
        "tron",
        "sol",
        "ton",
    ]


def test_release_readiness_report_guards_launch_scope_constant_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin SCCP launch-scope constants."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_launch_scope_constant_gate_inventory_errors() == []

    sparse_rust = tmp_path / "lib.rs"
    sparse_rust.write_text(
        "pub const SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1: [u32; 4] = [];\n",
        encoding="utf-8",
    )
    sparse_all_lanes = tmp_path / "sccp_all_lanes_evidence.py"
    sparse_all_lanes.write_text(
        "SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS = (SCCP_DOMAIN_ETH,)\n",
        encoding="utf-8",
    )
    sparse_report = tmp_path / "sccp_release_readiness_report.py"
    sparse_report.write_text(
        'ACTIVE_LAUNCH_CHAIN = "bsc"\nACTIVE_LAUNCH_POLICY = "BscMainnetLane"\n',
        encoding="utf-8",
    )

    inventory = (
        (sparse_rust, verifier.SCCP_LAUNCH_SCOPE_CONSTANT_MARKERS[0][1]),
        (sparse_all_lanes, verifier.SCCP_LAUNCH_SCOPE_CONSTANT_MARKERS[1][1]),
        (sparse_report, verifier.SCCP_LAUNCH_SCOPE_CONSTANT_MARKERS[2][1]),
    )

    errors = report._sccp_launch_scope_constant_gate_inventory_errors(inventory)

    assert any(
        "SCCP launch-scope constants source inventory" in error
        and "SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1: [u32; 5]" in error
        for error in errors
    )
    assert any(
        "SCCP launch-scope constants source inventory" in error
        and "SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS = (" in error
        for error in errors
    )
    assert any(
        "SCCP launch-scope constants source inventory" in error
        and "ACTIVE_LAUNCH_DOMAIN = 1" in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_launch_policy_selector_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin the ETH-only launch selector."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_launch_policy_selector_gate_inventory_errors() == []

    sparse_rust = tmp_path / "lib.rs"
    sparse_rust.write_text(
        "fn sccp_lane_production_ready_under_launch_policy_v1()\n"
        "SccpLaunchModeV1::EthereumMainnetLane => domain == SCCP_DOMAIN_ETH\n",
        encoding="utf-8",
    )
    required_markers = verifier.ETHEREUM_LAUNCH_POLICY_SELECTOR_MARKERS[0][1]
    removed_marker = (
        "EthereumMainnetLane must not open BSC even when BSC-shaped components are ready"
    )

    errors = report._ethereum_launch_policy_selector_gate_inventory_errors(
        ((sparse_rust, required_markers),)
    )

    assert any(
        "Ethereum mainnet launch-policy selector source inventory" in error
        and str(sparse_rust) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_launch_policy_documentation_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin active Ethereum launch-policy docs."""

    report = load_report_module()
    assert report._ethereum_launch_policy_documentation_gate_inventory_errors() == []

    sparse_docs = tmp_path / "bridge_proofs.md"
    sparse_docs.write_text(
        "active launch policy is Ethereum-mainnet lane readiness\n"
        "BSC mainnet only when the configured BSC source-chain finality/inclusion\n",
        encoding="utf-8",
    )

    errors = report._ethereum_launch_policy_documentation_gate_inventory_errors(
        (
            (
                sparse_docs,
                (
                    "active launch policy is Ethereum-mainnet lane readiness",
                    "mainnet source-proof, source-adapter deployment",
                ),
            ),
        ),
        ("BSC mainnet only when the configured BSC source-chain finality/inclusion",),
    )

    assert any(
        "Ethereum mainnet launch-policy documentation source inventory" in error
        and str(sparse_docs) in error
        and "missing marker: mainnet source-proof, source-adapter deployment" in error
        for error in errors
    )
    assert any(
        "Ethereum mainnet launch-policy documentation source inventory" in error
        and str(sparse_docs) in error
        and (
            "contains stale marker: BSC mainnet only when the configured BSC "
            "source-chain finality/inclusion"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_public_discovery_documentation_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin public SCCP discovery docs."""

    report = load_report_module()
    assert report._sccp_public_discovery_documentation_gate_inventory_errors() == []

    sparse_docs = tmp_path / "bridge_proofs.md"
    sparse_docs.write_text(
        "supported launch lanes only: `eth`, `bsc`, `sol`, `ton`, and `tron`\n",
        encoding="utf-8",
    )

    errors = report._sccp_public_discovery_documentation_gate_inventory_errors(
        (
            (
                sparse_docs,
                (
                    "supported launch lanes only: `eth`, `bsc`, `sol`, `ton`, and `tron`",
                    "the intended verifier target (`EVM`, `Solana`, `TON`, or `TRON`)",
                ),
            ),
        ),
    )

    assert any(
        "SCCP public discovery documentation source inventory" in error
        and str(sparse_docs) in error
        and (
            "missing marker: the intended verifier target "
            "(`EVM`, `Solana`, `TON`, or `TRON`)"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_openapi_no_support_discovery_note(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Torii OpenAPI no-support wording."""

    report = load_report_module()
    verifier = load_verify_helpers()
    required_markers = verifier.SCCP_PUBLIC_DISCOVERY_DOCUMENTATION_MARKERS[1][1]
    removed_marker = (
        "SCCP \\\n"
        "             will not support Sub&#115;trate/Pol&#107;adot networks for now."
    )
    openapi = tmp_path / "openapi.rs"
    openapi.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._sccp_public_discovery_documentation_gate_inventory_errors(
        ((openapi, required_markers),)
    )

    assert any(
        "SCCP public discovery documentation source inventory" in error
        and str(openapi) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_data_collection_no_proxy_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin no-proxy Ethereum data collection."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_data_collection_no_proxy_gate_inventory_errors() == []

    regions = {}
    for sdk, (_path, start_marker, end_marker, required_markers) in (
        verifier.ETHEREUM_DATA_COLLECTION_REGIONS.items()
    ):
        sparse_sdk = tmp_path / f"{sdk}.txt"
        sparse_sdk.write_text(
            "\n".join(
                (
                    start_marker,
                    required_markers[0],
                    required_markers[1],
                    "return Torii.proxy.fetch();",
                    end_marker,
                    "",
                )
            ),
            encoding="utf-8",
        )
        regions[sdk] = (
            sparse_sdk,
            start_marker,
            end_marker,
            required_markers,
        )
    errors = report._ethereum_data_collection_no_proxy_gate_inventory_errors(
        regions
    )

    for sdk, (sparse_sdk, _start, _end, required_markers) in regions.items():
        assert any(
            f"Ethereum mainnet {sdk} data collection source" in error
            and str(sparse_sdk) in error
            and f"missing provider marker: {required_markers[2]}" in error
            for error in errors
        )
        assert any(
            f"Ethereum mainnet {sdk} data collection source" in error
            and str(sparse_sdk) in error
            and f"missing provider marker: {required_markers[3]}" in error
            for error in errors
        )
        assert any(
            f"Ethereum mainnet {sdk} data collection source" in error
            and str(sparse_sdk) in error
            and "contains forbidden Torii" in error
            for error in errors
        )
        assert any(
            f"Ethereum mainnet {sdk} data collection source" in error
            and str(sparse_sdk) in error
            and "contains forbidden proxy" in error
            for error in errors
        )
        assert any(
            f"Ethereum mainnet {sdk} data collection source" in error
            and str(sparse_sdk) in error
            and "contains forbidden embedded HTTP client" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_native_receipt_finality_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin native receipt finality guards."""

    report = load_report_module()
    assert report._ethereum_native_receipt_finality_gate_inventory_errors() == []

    cases = (
        (
            "EvmSccpProver.kt",
            (
                "beaconFinality.beaconSlot is required for receiptProof",
                "beaconFinality.syncCommitteeRoot is required for receiptProof",
            ),
            "beaconFinality.syncCommitteeRoot is required for receiptProof",
        ),
        (
            "SccpEvmProver.swift",
            (
                "guard let beaconSlotInput = try Self.strictFirstPresent(",
                "guard let finalizedRootInput = try Self.strictFirstPresent(",
            ),
            "guard let finalizedRootInput = try Self.strictFirstPresent(",
        ),
        (
            "EthereumMainnetSccp.cs",
            (
                "BeaconSlot = NormalizeUnsignedInteger(",
                "BeaconFinalizedRoot = NormalizeRpcHex(",
            ),
            "BeaconFinalizedRoot = NormalizeRpcHex(",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_native_receipt_finality_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet native receipt finality source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_beacon_rest_finalized_header_shape_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Beacon REST finalized-header guards."""

    report = load_report_module()
    assert (
        report._ethereum_beacon_rest_finalized_header_shape_gate_inventory_errors()
        == []
    )

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text(
        'for (const field of ["parent_root", "state_root", "body_root"])\n',
        encoding="utf-8",
    )
    errors = report._ethereum_beacon_rest_finalized_header_shape_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    'for (const field of ["parent_root", "state_root", "body_root"])',
                    "/signature must be 96 bytes/u",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet Beacon REST finalized-header shape SDK test inventory"
        in error
        and str(sparse_test) in error
        and "missing marker: /signature must be 96 bytes/u" in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_beacon_rest_execution_payload_binding_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Beacon REST execution binding guards."""

    report = load_report_module()
    assert (
        report._ethereum_beacon_rest_execution_payload_binding_gate_inventory_errors()
        == []
    )

    sparse_source = tmp_path / "sccp.js"
    sparse_source.write_text("/eth/v2/beacon/blocks/finalized\n", encoding="utf-8")
    errors = report._ethereum_beacon_rest_execution_payload_binding_gate_inventory_errors(
        (
            (
                sparse_source,
                (
                    "/eth/v2/beacon/blocks/finalized",
                    "/eth/v1/beacon/light_client/finality_update",
                    "execution payload receipts_root must match block.receiptsRoot",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet Beacon REST execution payload binding SDK test inventory"
        in error
        and str(sparse_source) in error
        and "missing marker: /eth/v1/beacon/light_client/finality_update" in error
        for error in errors
    )
    assert any(
        "Ethereum mainnet Beacon REST execution payload binding SDK test inventory"
        in error
        and (
            "missing marker: execution payload receipts_root must match "
            "block.receiptsRoot"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_inbound_adversarial_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum inbound adversarial guards."""

    report = load_report_module()
    assert report._ethereum_inbound_adversarial_gate_inventory_errors() == []

    cases = (
        (
            "sccpEthereumMainnet.test.js",
            "failedReceipt",
            "duplicateReceipt",
        ),
        (
            "sccp.py",
            "_normalize_ethereum_mainnet_finality_branch",
            "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot",
        ),
        (
            "sccp.py",
            "def canonical_evm_sccp_receipt_proof_bytes",
            'raise ValueError("sourceDomain must be ETH")',
        ),
        (
            "sccp_test.py",
            "ETHEREUM_FINALITY_BRANCH",
            "test_ethereum_mainnet_sccp_inbound_prover_receives_immutable_evidence_snapshot",
        ),
        (
            "sccp_test.py",
            "ETHEREUM_FINALITY_BRANCH",
            'canonical_evm_sccp_receipt_proof_bytes({**evm_input, "source_domain": SCCP_DOMAIN_BSC})',
        ),
        (
            "SccpSolanaProverTests.swift",
            'invalidPublicInputs("receipt.status")',
            "testEthereumMainnetInboundProverReceivesCallbackEvidenceSnapshot",
        ),
        (
            "SourceSccpProofHashesTest.kt",
            "emptyEvmReceiptNodes",
            "sourceDomain must be ETH",
        ),
        (
            "EvmSccpProverTest.kt",
            'receipt + ("status" to "0x0")',
            "ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary",
        ),
        (
            "EvmSccpProverTests.java",
            "Ethereum inbound collection must reject failed receipts",
            "Ethereum inbound proving must reject missing finality branch",
        ),
        (
            "SccpEthereumMainnetTests.cs",
            "failedReceipt",
            'Assert.Contains("beaconFinality.finalityBranch", missingFinalityBranch.Message)',
        ),
    )
    for index, (filename, present_marker, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(present_marker + "\n", encoding="utf-8")
        errors = report._ethereum_inbound_adversarial_gate_inventory_errors(
            (
                (
                    sparse_source,
                    (
                        present_marker,
                        removed_marker,
                    ),
                ),
            )
        )

        assert any(
            "Ethereum mainnet inbound adversarial SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_bsc_inbound_adversarial_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin BSC inbound adversarial guards."""

    report = load_report_module()
    assert report._bsc_inbound_adversarial_gate_inventory_errors() == []

    cases = (
        (
            "sccpBscMainnet.test.js",
            "BscMainnetSccp requires full receipt proof evidence before inbound proving",
            "callbackEvidence.receiptProof.blockHash",
        ),
        (
            "sccp_test.py",
            "called_with_hash_only",
            'evidence["source_event_digest"]',
        ),
        (
            "sccp.py",
            "def canonical_bsc_sccp_receipt_proof_bytes",
            'raise ValueError("sourceDomain must be BSC")',
        ),
        (
            "sccp_test.py",
            "called_with_hash_only",
            'canonical_bsc_sccp_receipt_proof_bytes({**bsc_input, "source_domain": SCCP_DOMAIN_ETH})',
        ),
        (
            "EvmSccpProverTest.kt",
            "BscMainnetReceiptProof(",
            "calledWithoutSourceEvent",
        ),
        (
            "SccpSolanaProverTests.swift",
            "BscMainnetReceiptProof(",
            "extraTopicBscSourceReceipt",
        ),
        (
            "EvmSccpProverTests.java",
            "BscMainnetSccp.ReceiptProof",
            "BSC inbound proving must reject hash-only receipt proof evidence",
        ),
        (
            "SccpBscMainnetTests.cs",
            "BscMainnetReceiptProof",
            "Assert.Equal(0, noSourceEventProver.Calls)",
        ),
    )
    for index, (filename, present_marker, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(present_marker + "\n", encoding="utf-8")
        errors = report._bsc_inbound_adversarial_gate_inventory_errors(
            (
                (
                    sparse_source,
                    (
                        present_marker,
                        removed_marker,
                    ),
                ),
            )
        )

        assert any(
            "BSC mainnet inbound adversarial SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_tron_inbound_adversarial_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin TRON inbound adversarial guards."""

    report = load_report_module()
    assert report._tron_inbound_adversarial_gate_inventory_errors() == []

    sparse_runtime = tmp_path / "lib.rs"
    sparse_runtime.write_text(
        "fn tron_transaction_info_mpt_value_binds_successful_sccp_log() {}\n",
        encoding="utf-8",
    )
    errors = report._tron_inbound_adversarial_gate_inventory_errors(
        (
            (
                sparse_runtime,
                (
                    "fn tron_transaction_info_mpt_value_binds_successful_sccp_log()",
                    "TRON transaction-info receipts must not contain duplicate matching SCCP logs",
                    "TRON production receipt admission must reject duplicate matching SCCP logs",
                ),
            ),
        )
    )

    assert any(
        "TRON mainnet inbound adversarial source inventory" in error
        and str(sparse_runtime) in error
        and (
            "missing marker: TRON transaction-info receipts must not contain "
            "duplicate matching SCCP logs"
        )
        in error
        for error in errors
    )
    assert any(
        "TRON mainnet inbound adversarial source inventory" in error
        and (
            "missing marker: TRON production receipt admission must reject "
            "duplicate matching SCCP logs"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_bsc_route_config_canonical_manifest_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin BSC route-config manifest guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._bsc_route_config_canonical_manifest_gate_inventory_errors() == []

    sparse_script = tmp_path / "sccp_bsc_taira_xor_deploy.mjs"
    sparse_script.write_text(
        "function canonicalRecordString(value, label) {}\n",
        encoding="utf-8",
    )
    errors = report._bsc_route_config_canonical_manifest_gate_inventory_errors(
        (
            (
                sparse_script,
                verifier.BSC_ROUTE_CONFIG_CANONICAL_MANIFEST_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and str(sparse_script) in error
        and "missing marker: function normalizeCanonicalEvmAddress" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: function normalizeCanonicalManifestText(value, label)"
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: route manifest chainIdHex must be canonical lowercase hex."
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: function postDeployLiveEvidenceProductionBlockers(record)"
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: POST_DEPLOY_LIVE_EVIDENCE_BLOCKER_KEYS" in error
        for error in errors
    )

    sparse_test = tmp_path / "sccp_bsc_taira_xor_deploy.test.mjs"
    sparse_test.write_text(
        "BSC route-config rejects malformed or foreign route manifests\n",
        encoding="utf-8",
    )
    errors = report._bsc_route_config_canonical_manifest_gate_inventory_errors(
        (
            (
                sparse_test,
                verifier.BSC_ROUTE_CONFIG_CANONICAL_MANIFEST_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and str(sparse_test) in error
        and "missing marker: bscNetwork: \"BSC-TESTNET\"" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and 'missing marker: productionReady: "true"' in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: fullTomlReady: 1" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: disabledReason: 1" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: contractAddress: 1" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: settlement\\.contractAlias aliases disagree" in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and 'missing marker: settlement_contract_address = "bsc-settlement-v1"'
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and 'missing marker: settlement_contract_alias = "taira-bsc-xor"'
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and "missing marker: sourceEventTransactionId: HASH_55.toUpperCase()"
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /productionReady requires empty postDeployLiveEvidence "
            "production blockers.*source_event_transaction_production_blockers: "
            "witness seal proof required/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /source_event_transaction_production_blockers must "
            "be a list/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /productionReady requires empty postDeployLiveEvidence "
            "production blockers.*post_deploy_production_blockers: route "
            "overlay still pending/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP BSC route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /full_toml_production_blockers\\[0\\].*non-empty "
            "canonical string/u"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_tron_route_config_canonical_manifest_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin TRON route-config manifest guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._tron_route_config_canonical_manifest_gate_inventory_errors() == []

    sparse_script = tmp_path / "sccp_tron_taira_xor_deploy.mjs"
    sparse_script.write_text(
        "function normalizeBytes32(value, label) {}\n",
        encoding="utf-8",
    )
    errors = report._tron_route_config_canonical_manifest_gate_inventory_errors(
        (
            (
                sparse_script,
                verifier.TRON_ROUTE_CONFIG_CANONICAL_MANIFEST_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and str(sparse_script) in error
        and "missing marker: POST_DEPLOY_LIVE_EVIDENCE_BLOCKER_KEYS" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: function readOptionalCanonicalManifestText(" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: post_deploy_production_blockers" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: route_canary_production_blockers" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and (
            "missing marker: route manifest postDeployLiveEvidence.fullTomlReady "
            "must be true or false"
        )
        in error
        for error in errors
    )

    sparse_test = tmp_path / "sccp_tron_taira_xor_deploy.test.mjs"
    sparse_test.write_text(
        "TRON route-config rejects malformed or foreign route manifests\n",
        encoding="utf-8",
    )
    errors = report._tron_route_config_canonical_manifest_gate_inventory_errors(
        (
            (
                sparse_test,
                verifier.TRON_ROUTE_CONFIG_CANONICAL_MANIFEST_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and str(sparse_test) in error
        and "missing marker: tronNetwork: \"TRON-MAINNET\"" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and 'missing marker: productionReady: "true"' in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and 'missing marker: postDeployReadbackChecked: "true"' in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: fullTomlReady: 1" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: disabledReason: 1" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: contractAddress: 1" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: settlement\\.contractAlias aliases disagree" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and 'missing marker: settlement_contract_address = "tron-settlement-v1"'
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: offlineFullTomlSha256: routeHash(\"offline-full-toml\").toUpperCase()"
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: sourceEventTransactionProductionBlockers: [" in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /source_event_transaction_production_blockers must "
            "be empty.*witness seal proof required/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and "missing marker: /postDeployLiveEvidence\\.productionBlockers must be a list/u"
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /productionReady requires empty postDeployLiveEvidence "
            "production blocker lists.*post_deploy_production_blockers: route "
            "overlay still pending/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /postDeployLiveEvidence\\.route_canary_production_"
            "blockers\\[0\\].*without surrounding whitespace/u"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON route-config canonical-manifest source inventory" in error
        and (
            "missing marker: /postDeployLiveEvidence\\.full_toml_production_"
            "blockers\\[0\\].*without surrounding whitespace/u"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_tron_runtime_route_manifest_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin TRON runtime route manifests."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._tron_runtime_route_manifest_gate_inventory_errors() == []

    sparse_source = tmp_path / "user.rs"
    sparse_source.write_text(
        "fn validate_tron_production_metadata() {}\n",
        encoding="utf-8",
    )
    errors = report._tron_runtime_route_manifest_gate_inventory_errors(
        (
            (
                sparse_source,
                verifier.TRON_RUNTIME_ROUTE_MANIFEST_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP TRON runtime route-manifest source inventory" in error
        and str(sparse_source) in error
        and "missing marker: fn expected_tron_destination_binding_key(" in error
        for error in errors
    )
    assert any(
        "SCCP TRON runtime route-manifest source inventory" in error
        and "missing marker: fn production_ready_tron_route_requires_offline_full_toml_hash()"
        in error
        for error in errors
    )


def test_release_readiness_report_guards_all_lanes_route_canary_scalar_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin all-lanes route-canary scalars."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._all_lanes_route_canary_scalar_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccp_all_lanes_evidence_test.py"
    sparse_test.write_text(
        "def test_all_lanes_release_checklist_rejects_malformed_route_canary_scalars():\n",
        encoding="utf-8",
    )
    errors = report._all_lanes_route_canary_scalar_gate_inventory_errors(
        (
            (
                sparse_test,
                verifier.ALL_LANES_ROUTE_CANARY_SCALAR_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP all-lanes route-canary scalar source inventory" in error
        and str(sparse_test) in error
        and 'missing marker: "status",' in error
        for error in errors
    )
    assert any(
        "SCCP all-lanes route-canary scalar source inventory" in error
        and 'missing marker: " evm_message_proof_accepted_transaction "'
        in error
        for error in errors
    )

    route_redaction_marker = "secret-token operator route material"
    sparse_test.write_text(
        "\n".join(
            marker
            for marker in verifier.ALL_LANES_ROUTE_CANARY_SCALAR_MARKERS[1][1]
            if marker != route_redaction_marker
        ),
        encoding="utf-8",
    )
    errors = report._all_lanes_route_canary_scalar_gate_inventory_errors(
        ((sparse_test, verifier.ALL_LANES_ROUTE_CANARY_SCALAR_MARKERS[1][1]),)
    )

    assert any(
        "SCCP all-lanes route-canary scalar source inventory" in error
        and str(sparse_test) in error
        and f"missing marker: {route_redaction_marker}" in error
        for error in errors
    )


def test_release_readiness_report_guards_all_lanes_evidence_root_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin all-lanes evidence-root schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._all_lanes_evidence_root_schema_gate_inventory_errors() == []

    required_script_markers = verifier.ALL_LANES_EVIDENCE_ROOT_SCHEMA_MARKERS[0][1]
    for index, removed_script_marker in enumerate(
        (
            "evidence bundle root must be an object",
            "def _evidence_unsupported_section_detail(",
            "unsupported evidence section with sensitive name",
            "unsupported evidence section with malformed name",
            "def _unexpected_record_field_detail(",
            "unexpected field with sensitive name",
            "unexpected field with malformed name",
            "unexpected non-string field name",
        )
    ):
        sparse_script = tmp_path / f"sccp_all_lanes_evidence_{index}.py"
        sparse_script.write_text(
            "\n".join(
                marker
                for marker in required_script_markers
                if marker != removed_script_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_evidence_root_schema_gate_inventory_errors(
            (
                (
                    sparse_script,
                    required_script_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes evidence-root schema source inventory" in error
            and str(sparse_script) in error
            and f"missing marker: {removed_script_marker}" in error
            for error in errors
        )

    required_test_markers = verifier.ALL_LANES_EVIDENCE_ROOT_SCHEMA_MARKERS[1][1]
    removed_marker = "def test_all_lanes_evidence_rejects_non_string_section_keys"
    sparse_tests = tmp_path / "sccp_all_lanes_evidence_test.py"
    sparse_tests.write_text(
        "\n".join(marker for marker in required_test_markers if marker != removed_marker),
        encoding="utf-8",
    )
    errors = report._all_lanes_evidence_root_schema_gate_inventory_errors(
        (
            (
                sparse_tests,
                required_test_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes evidence-root schema source inventory" in error
        and str(sparse_tests) in error
        and f"missing marker: {removed_marker}" in error
        for error in errors
    )

    removed_unknown_marker = "unsupported evidence section sccp_shadow_rollouts"
    sparse_unknown_tests = tmp_path / "sccp_all_lanes_evidence_unknown_test.py"
    sparse_unknown_tests.write_text(
        "\n".join(
            marker
            for marker in required_test_markers
            if marker != removed_unknown_marker
        ),
        encoding="utf-8",
    )
    errors = report._all_lanes_evidence_root_schema_gate_inventory_errors(
        (
            (
                sparse_unknown_tests,
                required_test_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes evidence-root schema source inventory" in error
        and str(sparse_unknown_tests) in error
        and f"missing marker: {removed_unknown_marker}" in error
        for error in errors
    )

    for index, removed_redaction_marker in enumerate(
        (
            "def test_all_lanes_evidence_redacts_unsafe_direct_section_names",
            "secret-token-direct-section",
            "route|operator-direct-section",
            "def test_all_lanes_evidence_redacts_unsafe_unknown_record_fields",
            "secret-token-material-field",
            "route|operator-material-field",
        )
    ):
        sparse_redaction_tests = tmp_path / f"sccp_all_lanes_redaction_{index}.py"
        sparse_redaction_tests.write_text(
            "\n".join(
                marker
                for marker in required_test_markers
                if marker != removed_redaction_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_evidence_root_schema_gate_inventory_errors(
            (
                (
                    sparse_redaction_tests,
                    required_test_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes evidence-root schema source inventory" in error
            and str(sparse_redaction_tests) in error
            and f"missing marker: {removed_redaction_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_all_lanes_governed_blocker_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin governed blocker schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._all_lanes_governed_blocker_schema_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccp_all_lanes_evidence_test.py"
    sparse_test.write_text(
        "def test_all_lanes_evidence_rejects_malformed_governed_blocker_containers():\n",
        encoding="utf-8",
    )
    errors = report._all_lanes_governed_blocker_schema_gate_inventory_errors(
        (
            (
                sparse_test,
                verifier.ALL_LANES_GOVERNED_BLOCKER_SCHEMA_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP all-lanes governed blocker schema source inventory" in error
        and str(sparse_test) in error
        and 'missing marker: eth_destination["blockers"] = "operator says destination rollout is ready"'
        in error
        for error in errors
    )
    assert any(
        "SCCP all-lanes governed blocker schema source inventory" in error
        and 'missing marker: sol_route["blockers"] = [" route canary still pending"]'
        in error
        for error in errors
    )

    test_markers = verifier.ALL_LANES_GOVERNED_BLOCKER_SCHEMA_MARKERS[1][1]
    for removed_marker_index, removed_marker in enumerate((
        "secret-token-governed-blocker",
        "operator|governed-blocker",
        "confusable_blocker",
    )):
        sparse_marker_test = tmp_path / (
            f"sccp_all_lanes_governed_marker_{removed_marker_index}.py"
        )
        sparse_marker_test.write_text(
            "\n".join(marker for marker in test_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._all_lanes_governed_blocker_schema_gate_inventory_errors(
            ((sparse_marker_test, test_markers),)
        )

        assert any(
            "SCCP all-lanes governed blocker schema source inventory" in error
            and str(sparse_marker_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_all_lanes_release_checklist_exact_boolean_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin all-lanes checklist checks."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors()
        == []
    )

    required_script_markers = (
        verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS[0][1]
    )
    sparse_script = tmp_path / "sccp_all_lanes_evidence.py"
    sparse_script.write_text(
        '"ready": all(item["ready"] is True for item in items)\n',
        encoding="utf-8",
    )
    errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
        (
            (
                sparse_script,
                required_script_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and str(sparse_script) in error
        and 'missing marker: records.get(key) is not True' in error
        for error in errors
    )
    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and 'missing marker: return 0 if summary["production_ready"] is True else 1'
        in error
        for error in errors
    )

    for removed_script_marker_index, removed_script_marker in enumerate((
        "def _unexpected_audit_hash_field_detail(",
        "source adapter gate audit hashes contains unexpected field with sensitive name",
        "source adapter gate audit hashes contains unexpected field with malformed name",
        "source adapter gate audit hashes contains non-string field name",
    )):
        sparse_script_marker = tmp_path / (
            f"sccp_all_lanes_evidence_{removed_script_marker_index}.py"
        )
        sparse_script_marker.write_text(
            "\n".join(
                marker
                for marker in required_script_markers
                if marker != removed_script_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
            (
                (
                    sparse_script_marker,
                    required_script_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes release-checklist exact-boolean source inventory"
            in error
            and str(sparse_script_marker) in error
            and f"missing marker: {removed_script_marker}" in error
            for error in errors
        )

    required_test_markers = verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS[
        1
    ][1]
    removed_marker = (
        "def test_all_lanes_release_checklist_rejects_source_gate_hash_role_replay"
    )
    sparse_tests = tmp_path / "sccp_all_lanes_evidence_test.py"
    sparse_tests.write_text(
        "\n".join(marker for marker in required_test_markers if marker != removed_marker),
        encoding="utf-8",
    )
    errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
        (
            (
                sparse_tests,
                required_test_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and str(sparse_tests) in error
        and f"missing marker: {removed_marker}" in error
        for error in errors
    )

    for removed_test_marker_index, removed_test_marker in enumerate((
        "def test_all_lanes_release_checklist_redacts_unsafe_source_gate_audit_fields",
        "secret-token-audit-field",
        "route|operator-audit-field",
        "secret-token-replayed-audit-field",
    )):
        sparse_redaction_tests = tmp_path / (
            f"sccp_all_lanes_redaction_{removed_test_marker_index}_test.py"
        )
        sparse_redaction_tests.write_text(
            "\n".join(
                marker
                for marker in required_test_markers
                if marker != removed_test_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
            (
                (
                    sparse_redaction_tests,
                    required_test_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes release-checklist exact-boolean source inventory"
            in error
            and str(sparse_redaction_tests) in error
            and f"missing marker: {removed_test_marker}" in error
            for error in errors
        )

    removed_gate_blocker_marker = "source_gate.checklist_empty"
    sparse_gate_blocker_tests = tmp_path / "sccp_all_lanes_source_gate_test.py"
    sparse_gate_blocker_tests.write_text(
        "\n".join(
            marker
            for marker in required_test_markers
            if marker != removed_gate_blocker_marker
        ),
        encoding="utf-8",
    )
    errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
        (
            (
                sparse_gate_blocker_tests,
                required_test_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and str(sparse_gate_blocker_tests) in error
        and f"missing marker: {removed_gate_blocker_marker}" in error
        for error in errors
    )

    removed_gate_summary_marker = "source_gate.summary_empty"
    sparse_gate_summary_tests = tmp_path / "sccp_all_lanes_source_gate_summary_test.py"
    sparse_gate_summary_tests.write_text(
        "\n".join(
            marker
            for marker in required_test_markers
            if marker != removed_gate_summary_marker
        ),
        encoding="utf-8",
    )
    errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
        (
            (
                sparse_gate_summary_tests,
                required_test_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and str(sparse_gate_summary_tests) in error
        and f"missing marker: {removed_gate_summary_marker}" in error
        for error in errors
    )

    required_rust_markers = verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS[
        2
    ][1]
    for removed_rust_marker_index, removed_rust_marker in enumerate((
        "fn solana_lane_route_canary_evidence_binds_live_programdata_snapshot",
        "BSC lane readiness must require live route canary evidence",
        "BSC route canary evidence must require finalized receipt-block metadata",
        "generic Solana route canary evidence must not satisfy lane readiness",
        "Solana route canary evidence must reject route allowlist/source material hash role reuse",
    )):
        sparse_rust = tmp_path / f"lib_{removed_rust_marker_index}.rs"
        sparse_rust.write_text(
            "\n".join(
                marker
                for marker in required_rust_markers
                if marker != removed_rust_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
            (
                (
                    sparse_rust,
                    required_rust_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes release-checklist exact-boolean source inventory"
            in error
            and str(sparse_rust) in error
            and f"missing marker: {removed_rust_marker}" in error
            for error in errors
        )

    required_sdk_markers = next(
        markers
        for path, markers in verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS
        if path == "python/iroha_torii_client/sccp.py"
    )
    removed_sdk_marker = "TRON route allowlist governed hashes"
    sparse_sdk = tmp_path / "sccp.py"
    sparse_sdk.write_text(
        "\n".join(marker for marker in required_sdk_markers if marker != removed_sdk_marker),
        encoding="utf-8",
    )
    errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
        (
            (
                sparse_sdk,
                required_sdk_markers,
            ),
        )
    )

    assert any(
        "SCCP all-lanes release-checklist exact-boolean source inventory" in error
        and str(sparse_sdk) in error
        and f"missing marker: {removed_sdk_marker}" in error
        for error in errors
    )

    required_python_test_markers = next(
        markers
        for path, markers in verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS
        if path == "python/iroha_torii_client/tests/sccp_test.py"
    )
    for removed_python_marker in (
        "package_root_solana_route_canary_governed_hash_reuse",
        "package_root_ton_route_canary_governed_hash_reuse",
        "package_root_tron_route_canary_governed_hash_reuse",
    ):
        sparse_python_tests = tmp_path / f"{removed_python_marker}.py"
        sparse_python_tests.write_text(
            "\n".join(
                marker
                for marker in required_python_test_markers
                if marker != removed_python_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
            (
                (
                    sparse_python_tests,
                    required_python_test_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes release-checklist exact-boolean source inventory"
            in error
            and str(sparse_python_tests) in error
            and f"missing marker: {removed_python_marker}" in error
            for error in errors
        )

    for source_path, removed_js_marker in (
        (
            "javascript/iroha_js/test/package_dist.test.js",
            "distSolanaRouteCanaryGovernedHashReuse",
        ),
        (
            "javascript/iroha_js/test/package_dist.test.js",
            "distTonRouteCanaryGovernedHashReuse",
        ),
        (
            "javascript/iroha_js/test/package_dist.test.js",
            "distTronRouteCanaryGovernedHashReuse",
        ),
        (
            "javascript/iroha_js/test/sccpPackageExports.test.js",
            "packageRootSolanaRouteCanaryGovernedHashReuse",
        ),
        (
            "javascript/iroha_js/test/sccpPackageExports.test.js",
            "packageRootTonRouteCanaryGovernedHashReuse",
        ),
        (
            "javascript/iroha_js/test/sccpPackageExports.test.js",
            "packageRootTronRouteCanaryGovernedHashReuse",
        ),
    ):
        required_js_markers = next(
            markers
            for path, markers in verifier.ALL_LANES_RELEASE_CHECKLIST_EXACT_BOOLEAN_MARKERS
            if path == source_path
        )
        sparse_js_tests = tmp_path / removed_js_marker
        sparse_js_tests.write_text(
            "\n".join(
                marker for marker in required_js_markers if marker != removed_js_marker
            ),
            encoding="utf-8",
        )
        errors = report._all_lanes_release_checklist_exact_boolean_gate_inventory_errors(
            (
                (
                    sparse_js_tests,
                    required_js_markers,
                ),
            )
        )

        assert any(
            "SCCP all-lanes release-checklist exact-boolean source inventory"
            in error
            and str(sparse_js_tests) in error
            and f"missing marker: {removed_js_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_active_launch_checklist_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin active-launch checklist schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._active_launch_checklist_schema_gate_inventory_errors() == []

    sparse_report = tmp_path / "sccp_release_readiness_report.py"
    sparse_report.write_text(
        "def _active_launch_release_checklist(evidence, native_prover_bundle):\n",
        encoding="utf-8",
    )
    errors = report._active_launch_checklist_schema_gate_inventory_errors(
        (
            (
                sparse_report,
                verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_report) in error
        and 'missing marker: release_checklist["ready"] is True' in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_report) in error
        and "missing marker: def _active_launch_evm_live_metadata_blockers("
        in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_report) in error
        and "missing marker: def _active_launch_blockers(" in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and "missing marker: _active_launch_route_allowlist_binding_blockers(lane_label, lane)"
        in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and "missing marker: source verifier material hash must not reuse source adapter engine deployment hash"
        in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and "missing marker: source live eth_chainId must be" in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and "missing marker: active EVM source adapter gate audit hashes must contain only evm_source_gate_hash"
        in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and "missing marker: route allowlist hash must match the expected canonical source, deployment, and destination binding hash"
        in error
        for error in errors
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and 'missing marker: ("block_receipts_root", "block receipts root"),'
        in error
        for error in errors
    )

    source_gate_test_marker = "source_adapter_gate.audit_hashes"
    sparse_tests = tmp_path / "sccp_release_readiness_report_test.py"
    sparse_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != source_gate_test_marker
        ),
        encoding="utf-8",
    )
    test_errors = report._active_launch_checklist_schema_gate_inventory_errors(
        (
            (
                sparse_tests,
                verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
            ),
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_tests) in error
        and f"missing marker: {source_gate_test_marker}" in error
        for error in test_errors
    )

    route_binding_test_marker = "route_allowlist.hash_mismatch"
    sparse_route_tests = tmp_path / "sccp_release_readiness_report_route_test.py"
    sparse_route_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != route_binding_test_marker
        ),
        encoding="utf-8",
    )
    route_test_errors = report._active_launch_checklist_schema_gate_inventory_errors(
        (
            (
                sparse_route_tests,
                verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
            ),
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_route_tests) in error
        and f"missing marker: {route_binding_test_marker}" in error
        for error in route_test_errors
    )

    route_canary_test_marker = (
        "route canary block receipts root must be a canonical non-zero bytes32 hex string"
    )
    sparse_canary_tests = tmp_path / "sccp_release_readiness_report_canary_test.py"
    sparse_canary_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != route_canary_test_marker
        ),
        encoding="utf-8",
    )
    canary_test_errors = report._active_launch_checklist_schema_gate_inventory_errors(
        (
            (
                sparse_canary_tests,
                verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
            ),
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_canary_tests) in error
        and f"missing marker: {route_canary_test_marker}" in error
        for error in canary_test_errors
    )

    evm_live_test_marker = "assert absent_blocker not in blockers"
    sparse_evm_live_tests = tmp_path / "sccp_release_readiness_report_evm_live_test.py"
    sparse_evm_live_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != evm_live_test_marker
        ),
        encoding="utf-8",
    )
    evm_live_test_errors = report._active_launch_checklist_schema_gate_inventory_errors(
        (
            (
                sparse_evm_live_tests,
                verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
            ),
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_evm_live_tests) in error
        and f"missing marker: {evm_live_test_marker}" in error
        for error in evm_live_test_errors
    )

    required_record_test_marker = "domain.string"
    sparse_required_record_tests = (
        tmp_path / "sccp_release_readiness_report_required_record_test.py"
    )
    sparse_required_record_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != required_record_test_marker
        ),
        encoding="utf-8",
    )
    required_record_test_errors = (
        report._active_launch_checklist_schema_gate_inventory_errors(
            (
                (
                    sparse_required_record_tests,
                    verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
                ),
            )
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_required_record_tests) in error
        and f"missing marker: {required_record_test_marker}" in error
        for error in required_record_test_errors
    )

    unresolved_blocker_test_marker = "top_level.numeric"
    sparse_unresolved_blocker_tests = (
        tmp_path / "sccp_release_readiness_report_unresolved_blocker_test.py"
    )
    sparse_unresolved_blocker_tests.write_text(
        "\n".join(
            marker
            for marker in verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1]
            if marker != unresolved_blocker_test_marker
        ),
        encoding="utf-8",
    )
    unresolved_blocker_test_errors = (
        report._active_launch_checklist_schema_gate_inventory_errors(
            (
                (
                    sparse_unresolved_blocker_tests,
                    verifier.ACTIVE_LAUNCH_CHECKLIST_SCHEMA_MARKERS[3][1],
                ),
            )
        )
    )
    assert any(
        "SCCP active-launch checklist schema source inventory" in error
        and str(sparse_unresolved_blocker_tests) in error
        and f"missing marker: {unresolved_blocker_test_marker}" in error
        for error in unresolved_blocker_test_errors
    )


def test_release_readiness_report_guards_release_manifest_readiness_flags_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin exact release manifest readiness flags."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_release_manifest_readiness_flags_gate_inventory_errors() == []

    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "def _release_bundle_manifest(output_dir, report, artifact_paths):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_manifest_readiness_flags_gate_inventory_errors(
        (
            (
                sparse_bundle,
                verifier.SCCP_RELEASE_MANIFEST_READINESS_FLAGS_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release manifest readiness-flags source inventory" in error
        and str(sparse_bundle) in error
        and "missing marker: Build the release manifest without truthy-coercing readiness fields."
        in error
        for error in errors
    )
    assert any(
        "SCCP release manifest readiness-flags source inventory" in error
        and 'missing marker: "corridor_ready": report["corridor"]["production_ready"]'
        in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_MANIFEST_READINESS_FLAGS_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_test_markers = {
        "def test_release_bundle_manifest_preserves_malformed_readiness_values",
        "def test_release_bundle_verifier_rejects_readiness_boolean_type_drift",
        "def test_release_bundle_verifier_rejects_manifest_readiness_claim_drift",
        "def test_release_bundle_rejects_manifest_drift_before_write",
        "def test_release_bundle_verifier_compares_summary_launch_ready_exactly",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_manifest_readiness_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_test_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_manifest_readiness_flags_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_test_markers):
        assert any(
            "SCCP release manifest readiness-flags source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_manifest_artifact_set_order_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin manifest artifact set/order checks."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_manifest_artifact_set_order_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _referenced_report_artifact_paths(report):\n",
        encoding="utf-8",
    )
    errors = (
        report._sccp_release_manifest_artifact_set_order_gate_inventory_errors(
            (
                (
                    sparse_verifier,
                    verifier.SCCP_RELEASE_MANIFEST_ARTIFACT_SET_ORDER_MARKERS[1][1],
                ),
            )
        )
    )

    assert any(
        "SCCP release manifest artifact-set/order source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: REQUIRED_ARTIFACT_PATHS = (" in error
        for error in errors
    )
    assert any(
        "SCCP release manifest artifact-set/order source inventory" in error
        and "missing marker: manifest artifact order does not match canonical"
        in error
        for error in errors
    )
    assert any(
        "SCCP release manifest artifact-set/order source inventory" in error
        and "missing marker: manifest artifact bytes must be a positive integer"
        in error
        for error in errors
    )

    test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_MANIFEST_ARTIFACT_SET_ORDER_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_markers = {
        "def test_release_bundle_verifier_rejects_manifest_root_self_listing",
        "def test_release_bundle_verifier_rejects_symlinked_bundle_root",
        "def test_release_bundle_verifier_rejects_non_directory_bundle_root",
        "def test_release_bundle_verifier_rejects_missing_manifest_without_path_leak",
        "def test_release_bundle_verifier_rejects_duplicate_manifest_artifact_paths",
        "def test_release_bundle_verifier_rejects_unmanifested_artifact",
        "def test_release_bundle_verifier_rejects_unmanifested_directory",
        "def test_release_bundle_verifier_rejects_unsupported_entry_without_path_leak",
        "def test_release_bundle_verifier_rejects_omitted_phase_artifact",
        "def test_release_bundle_verifier_rejects_extra_manifested_artifact",
        "def test_release_bundle_verifier_rejects_unknown_phase_artifact_reference",
        "def test_release_bundle_verifier_rejects_manifest_artifact_order_drift",
        "def test_release_bundle_verifier_rejects_unknown_artifact_fields",
        "def test_release_bundle_verifier_rejects_malformed_artifact_fields",
        "secret_token_artifact_field",
        "def test_release_bundle_verifier_rejects_artifact_field_type_drift",
        "def test_release_bundle_verifier_rejects_artifact_digest_text_drift",
        "def test_release_bundle_rejects_malformed_copied_artifacts_before_render",
        "def test_release_bundle_rejects_copied_artifact_hash_drift_before_render",
        "def test_release_bundle_rejects_manifest_drift_before_write",
    }
    sparse_test = tmp_path / "sccp_release_bundle_test.py"
    sparse_test.write_text(
        "\n".join(marker for marker in test_markers if marker not in removed_markers),
        encoding="utf-8",
    )
    errors = report._sccp_release_manifest_artifact_set_order_gate_inventory_errors(
        ((sparse_test, test_markers),)
    )

    for removed_marker in removed_markers:
        assert any(
            "SCCP release manifest artifact-set/order source inventory" in error
            and str(sparse_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_public_blocker_list_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin public blocker-list schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_blocker_list_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _string_list_field_errors(label, payload, field, *, allow_empty):\n",
        encoding="utf-8",
    )
    errors = (
        report._sccp_release_public_blocker_list_schema_gate_inventory_errors(
            (
                (
                    sparse_verifier,
                    verifier.SCCP_RELEASE_PUBLIC_BLOCKER_LIST_SCHEMA_MARKERS[0][1],
                ),
            )
        )
    )

    assert any(
        "SCCP release public blocker-list schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: must not contain duplicate strings" in error
        for error in errors
    )
    assert any(
        "SCCP release public blocker-list schema source inventory" in error
        and "missing marker: `<invalid blockers>`" in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_BLOCKER_LIST_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_markers = {
        "def test_release_bundle_allow_not_ready_rejects_noncanonical_root_blockers",
        "root.empty",
        "root.numeric",
        "root.null",
        "def test_release_bundle_rejects_malformed_copied_corridor_before_render",
        "operator|secret-token",
        "def test_release_bundle_rejects_copied_corridor_not_ready_before_render",
        "def test_release_bundle_verifier_rejects_all_lanes_list_scalar_type_drift",
        "def test_release_bundle_verifier_rejects_padded_public_blocker_strings",
        "def test_release_bundle_verifier_rejects_duplicate_public_blocker_strings",
        "def test_release_bundle_verifier_rejects_hostile_public_blocker_strings",
        "secret-token-public-blocker",
        "operator|public-blocker",
        "operator public blоcker",
        "def test_release_bundle_verifier_active_launch_blockers_reject_malformed_containers",
        "def test_release_bundle_verifier_rejects_malformed_active_launch_blockers",
        "def test_release_bundle_verifier_rejects_all_lanes_root_blockers",
        "def test_release_bundle_release_notes_mark_malformed_blocker_containers",
        "def test_release_bundle_verifier_markdown_invariants_require_invalid_blocker_markers",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_blocker_list_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_blocker_list_schema_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_markers):
        assert any(
            "SCCP release public blocker-list schema source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )

    readiness_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_BLOCKER_LIST_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_readiness_report_test.py"
    )
    removed_readiness_markers = {
        "def test_release_readiness_report_markdown_marks_malformed_blocker_containers",
        "def test_release_readiness_report_markdown_marks_hostile_public_blocker_strings",
        "def test_release_readiness_report_classifies_malformed_active_lane_blockers",
        "def test_release_readiness_report_blocks_malformed_native_prover_blockers",
    }
    sparse_readiness_test = tmp_path / "sccp_release_readiness_blocker_list_test.py"
    sparse_readiness_test.write_text(
        "\n".join(
            marker
            for marker in readiness_test_markers
            if marker not in removed_readiness_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_blocker_list_schema_gate_inventory_errors(
        ((sparse_readiness_test, readiness_test_markers),)
    )

    for removed_marker in sorted(removed_readiness_markers):
        assert any(
            "SCCP release public blocker-list schema source inventory" in error
            and str(sparse_readiness_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_public_scalar_text_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin public scalar-text schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_scalar_text_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _non_empty_string_field_errors(label, payload, field):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: def _cryptographic_evidence_row_schema_errors(" in error
        for error in errors
    )
    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and "missing marker: for field in (\"status\", \"evidence_source\"):" in error
        for error in errors
    )

    solana_script_markers = (
        "JSON-RPC returned duplicate JSON keys",
        "JSON-RPC {method} failed with HTTP {exc.code}",
        "JSON-RPC {method} request failed",
        "JSON-RPC {method} returned error response",
    )
    sparse_solana_script = tmp_path / "sccp_solana_live_evidence.py"
    sparse_solana_script.write_text(
        "\n".join(
            marker
            for marker in solana_script_markers
            if marker != "JSON-RPC {method} returned error response"
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_solana_script, solana_script_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_solana_script) in error
        and "missing marker: JSON-RPC {method} returned error response" in error
        for error in errors
    )

    tron_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "scripts/sccp_tron_live_evidence.py"
    )
    for removed_tron_script_index, removed_tron_script_marker in enumerate((
        'if source_event_digest is not None and getattr(args, "full_toml", False):',
        'raise ValueError(f"{label} must be ASCII") from None',
        'raise ValueError("--tron-pro-api-key-file cannot be read") from None',
        'raise ValueError("--witness-schedule-payload-file cannot be read") from None',
        'f"--witness-schedule-transition-json {index} must be JSON"',
        'f"TRON constant call {function_selector} failed"',
        'raise RuntimeError(f"{label} is not a valid TRON address") from None',
        "generated offline full TOML arguments are invalid",
        'f"/wallet/getcontract returned malformed {label} bytecode"',
        'f"/wallet/getcontract returned malformed {label} contract_address"',
        "except (argparse.ArgumentTypeError, TypeError, RuntimeError):",
        "except (argparse.ArgumentTypeError, TypeError, ValueError):",
        'f"TRON constant call {function_selector} returned non-hex data"',
        "except (RuntimeError, TypeError, ValueError):",
        "def _unsupported_tron_field_detail(",
        "field with sensitive name",
        "field with malformed name",
        "non-string field name",
        "witness schedule payload is invalid",
        'f"witness schedule transition {index} message is invalid"',
        'f"witness schedule transition {index} seal is invalid"',
        "witness seal solid-block message is invalid",
        "witness seal proof is invalid",
        "transaction source proof is invalid",
        "TRON API {endpoint} returned invalid JSON",
        "TRON API {endpoint} returned duplicate JSON keys",
    )):
        sparse_tron_script = (
            tmp_path / f"sccp_tron_live_evidence_{removed_tron_script_index}.py"
        )
        sparse_tron_script.write_text(
            "\n".join(
                marker
                for marker in tron_script_markers
                if marker != removed_tron_script_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
            ((sparse_tron_script, tron_script_markers),)
        )

        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_tron_script) in error
            and f"missing marker: {removed_tron_script_marker}" in error
            for error in errors
        )

    evm_live_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "scripts/sccp_evm_live_evidence.py"
    )
    evm_live_removed_marker = "generated EVM destination TOML arguments are invalid"
    sparse_evm_live_script = tmp_path / "sccp_evm_live_evidence.py"
    sparse_evm_live_script.write_text(
        "\n".join(
            marker
            for marker in evm_live_script_markers
            if marker != evm_live_removed_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_evm_live_script, evm_live_script_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_evm_live_script) in error
        and f"missing marker: {evm_live_removed_marker}" in error
        for error in errors
    )

    evm_source_live_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "scripts/sccp_evm_source_live_evidence.py"
    )
    evm_source_live_removed_marker = (
        "deployment receipt transactionHash must be a non-zero bytes32"
    )
    sparse_evm_source_live_script = tmp_path / "sccp_evm_source_live_evidence.py"
    sparse_evm_source_live_script.write_text(
        "\n".join(
            marker
            for marker in evm_source_live_script_markers
            if marker != evm_source_live_removed_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_evm_source_live_script, evm_source_live_script_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_evm_source_live_script) in error
        and f"missing marker: {evm_source_live_removed_marker}" in error
        for error in errors
    )

    all_lanes_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "scripts/sccp_all_lanes_evidence.py"
    )
    for all_lanes_removed_marker in (
        "except (argparse.ArgumentTypeError, SystemExit, TypeError, ValueError, RuntimeError):",
        "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
    ):
        sparse_all_lanes_script = tmp_path / "sccp_all_lanes_evidence.py"
        sparse_all_lanes_script.write_text(
            "\n".join(
                marker
                for marker in all_lanes_script_markers
                if marker != all_lanes_removed_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
            ((sparse_all_lanes_script, all_lanes_script_markers),)
        )

        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_all_lanes_script) in error
            and f"missing marker: {all_lanes_removed_marker}" in error
            for error in errors
        )

    solana_test_markers = (
        "def test_solana_json_rpc_redacts_transport_and_error_response_details",
        "def test_solana_json_rpc_redacts_invalid_json_parser_details",
        "def test_live_solana_account_data_redacts_base64_parser_causes",
        "def test_live_solana_metadata_base64_redacts_parser_causes",
        """parser_exception_types = (
        module.argparse.ArgumentTypeError,
        TypeError,
        ValueError,
    )""",
        "secret-token-solana-error",
        "secret-token invalid Solana JSON-RPC payload",
        "secret-token verifier_code_hash parser detail",
        "secret-token live account base64",
        "account_exception_types = (TypeError, ValueError)",
        "secret-token account-data decoder detail",
        "secret-token live metadata base64",
        "secret-token {label} decode detail",
        "duplicate JSON keys",
        'assert "secret-token" not in message',
    )
    sparse_solana_test = tmp_path / "sccp_solana_live_evidence_test.py"
    sparse_solana_test.write_text(
        "\n".join(
            marker
            for marker in solana_test_markers
            if marker != "secret-token-solana-error"
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_solana_test, solana_test_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_solana_test) in error
        and "missing marker: secret-token-solana-error" in error
        for error in errors
    )

    solana_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "scripts/sccp_solana_live_evidence.py"
    )
    solana_script_removed_marker = "JSON-RPC {method} returned invalid JSON"
    sparse_solana_script = tmp_path / "sccp_solana_live_evidence_invalid_json.py"
    sparse_solana_script.write_text(
        "\n".join(
            marker
            for marker in solana_script_markers
            if marker != solana_script_removed_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_solana_script, solana_script_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_solana_script) in error
        and f"missing marker: {solana_script_removed_marker}" in error
        for error in errors
    )

    for solana_script_removed_marker in (
        'raise RuntimeError(f"{label} account data is invalid base64") from None',
        'raise ValueError(f"{label} must be base64") from None',
        "except (TypeError, ValueError, binascii.Error):",
        "except (argparse.ArgumentTypeError, TypeError, ValueError):",
    ):
        sparse_solana_script = (
            tmp_path / f"sccp_solana_live_base64_{len(solana_script_removed_marker)}.py"
        )
        sparse_solana_script.write_text(
            "\n".join(
                marker
                for marker in solana_script_markers
                if marker != solana_script_removed_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
            ((sparse_solana_script, solana_script_markers),)
        )

        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_solana_script) in error
            and f"missing marker: {solana_script_removed_marker}" in error
            for error in errors
        )

    solana_payload_removed_marker = "secret-token invalid Solana JSON-RPC payload"
    sparse_solana_payload_test = tmp_path / "sccp_solana_live_evidence_payload_test.py"
    sparse_solana_payload_test.write_text(
        "\n".join(
            marker
            for marker in solana_test_markers
            if marker != solana_payload_removed_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_solana_payload_test, solana_test_markers),)
    )

    assert any(
        "SCCP release public scalar-text schema source inventory" in error
        and str(sparse_solana_payload_test) in error
        and f"missing marker: {solana_payload_removed_marker}" in error
        for error in errors
    )

    for solana_test_removed_marker in (
        """parser_exception_types = (
        module.argparse.ArgumentTypeError,
        TypeError,
        ValueError,
    )""",
        "secret-token verifier_code_hash parser detail",
        "secret-token {label} decode detail",
        "account_exception_types = (TypeError, ValueError)",
        "secret-token account-data decoder detail",
    ):
        sparse_solana_test = (
            tmp_path / f"sccp_solana_live_parser_{len(solana_test_removed_marker)}.py"
        )
        sparse_solana_test.write_text(
            "\n".join(
                marker
                for marker in solana_test_markers
                if marker != solana_test_removed_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
            ((sparse_solana_test, solana_test_markers),)
        )

        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_solana_test) in error
            and f"missing marker: {solana_test_removed_marker}" in error
            for error in errors
        )

    adversarial_marker_cases = (
        (
            "scripts/sccp_verify_release_bundle.py",
            "def _is_nonzero_hex32(value: Any) -> bool:",
        ),
        (
            "scripts/sccp_verify_release_bundle.py",
            "def _solana_pubkey_field_errors(",
        ),
        (
            "scripts/sccp_verify_release_bundle.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_release_readiness_report.py",
            "def _is_nonzero_hex32(value: Any) -> bool:",
        ),
        (
            "scripts/sccp_release_readiness_report.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_release_bundle.py",
            "def _is_canonical_solana_pubkey_text(value: Any) -> bool:",
        ),
        (
            "scripts/sccp_release_bundle.py",
            "except (TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token-ton-error",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token invalid TON accountStates payload",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token hash base64",
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            "except (TypeError, binascii.Error, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token-ton-api-key-file",
        ),
        (
            "pytests/scripts/sccp_ton_source_state_evidence_test.py",
            "secret-token-ton-source-hex",
        ),
        (
            "pytests/scripts/sccp_ton_source_state_evidence_test.py",
            "secret-token TON source hex TypeError detail",
        ),
        (
            "scripts/sccp_ton_source_state_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_ton_source_state_evidence.py",
            "except (OSError, SystemExit, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_ton_source_state_evidence_test.py",
            "for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
            "secret-token invalid EVM receipt JSON-RPC payload",
        ),
        (
            "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
            "secret-token-evm-receipt-hex",
        ),
        (
            "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
            "secret-token EVM receipt hex TypeError detail",
        ),
            (
                "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
                "for exception_type in (RuntimeError, TypeError, ValueError):",
            ),
        (
            "scripts/sccp_evm_receipt_proof_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_evm_receipt_proof_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError, argparse.ArgumentTypeError) as exc:",
        ),
        (
            "pytests/scripts/sccp_release_bundle_test.py",
            "secret-token-native-duplicate",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            "secret-token live account base64",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            "secret-token live metadata base64",
        ),
        (
            "scripts/sccp_solana_live_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            """parser_exception_types = (
        module.argparse.ArgumentTypeError,
        TypeError,
        ValueError,
    )""",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            "secret-token verifier_code_hash parser detail",
        ),
        (
            "pytests/scripts/sccp_solana_live_evidence_test.py",
            "secret-token {label} decode detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token solid block proof parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "source-event calldata rendered in full TOML mode",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "for exception_type in (TypeError, ValueError, RuntimeError):",
        ),
        (
            "scripts/sccp_tron_live_evidence.py",
            "    except (\n        OSError,\n        RuntimeError,\n        TypeError,\n        ValueError,\n        argparse.ArgumentTypeError,\n    ) as exc:",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token transaction source proof parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "def test_live_evidence_redacts_source_event_topic_parser_typeerror",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token source-event log topic0 parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "def test_live_evidence_redacts_route_canary_topic_parser_typeerror",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token route-canary log topic0 parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "def test_live_evidence_redacts_unsupported_transaction_result_fields",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token-result-field",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token-detail-field",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token witness schedule payload parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token witness schedule hash parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token transition message parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token transition seal parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token witness seal message parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token witness seal proof parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token TRON API error object",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token invalid JSON payload",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token invalid transition JSON",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token-duplicate-transition",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "0xsecret-token-bytecode",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token constant failure detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "0xsecret-token constant parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "secret-token generated full TOML parser detail",
        ),
        (
            "pytests/scripts/sccp_tron_live_evidence_test.py",
            "TRON API /wallet/triggerconstantcontract returned duplicate JSON keys",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "secret-token-tron-source-fixed-hex",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "secret-token-tron-source-runtime",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "secret-token-tron-source-file",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "secret-token-tron-source-address",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "secret-token TRON source hex TypeError detail",
        ),
        (
            "scripts/sccp_tron_source_bridge_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_tron_source_bridge_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_tron_source_bridge_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "0xsecret-token-source-bridge-runtime",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "secret-token invalid EVM source JSON-RPC payload",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "secret-token-evm-source-live-hex",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "secret-token EVM source live hex TypeError detail",
        ),
        (
            "scripts/sccp_evm_source_live_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_evm_source_live_evidence.py",
            "    except (\n        OSError,\n        RuntimeError,\n        TypeError,\n        ValueError,\n        argparse.ArgumentTypeError,\n    ) as exc:",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "secret-token {target_method} parser detail",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_evm_source_live_evidence_test.py",
            "for exception_type in (TypeError, RuntimeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
            "secret-token-eth-source-hex",
        ),
        (
            "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
            "secret-token-eth-source-runtime0",
        ),
        (
            "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
            "secret-token ETH source hex TypeError detail",
        ),
        (
            "scripts/sccp_eth_source_bridge_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_eth_source_bridge_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
            "secret-token-eth-source-file-path.hex",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "secret-token-bsc-source-hex",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "secret-token-bsc-source-runtime0",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "secret-token BSC source hex TypeError detail",
        ),
        (
            "scripts/sccp_bsc_source_bridge_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_bsc_source_bridge_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "secret-token-bsc-source-file-path.hex",
        ),
        (
            "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
            "secret-token-bsc-source-network",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "0xsecret-token-destination-runtime",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "secret-token invalid EVM JSON-RPC payload",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "secret-token EVM live hex TypeError detail",
        ),
        (
            "scripts/sccp_evm_live_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_evm_live_evidence.py",
            "    except (\n        OSError,\n        RuntimeError,\n        TypeError,\n        ValueError,\n        argparse.ArgumentTypeError,\n    ) as exc:",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "secret-token-evm-live-chain-id",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "secret-token-evm-live-network-id",
        ),
        (
            "pytests/scripts/sccp_evm_live_evidence_test.py",
            "secret-token generated EVM destination TOML parser detail",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token-evm-destination-domain",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token-evm-destination-bsc-network",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token-evm-destination-hex",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token-evm-destination-runtime",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token EVM destination hex TypeError detail",
        ),
        (
            "scripts/sccp_evm_destination_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_evm_destination_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_evm_destination_evidence_test.py",
            "secret-token-evm-destination-file-path.hex",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token-solana-fixed-hex",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token-solana-program-hex",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token Solana destination hex TypeError detail",
        ),
        (
            "scripts/sccp_solana_destination_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_solana_destination_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token-solana-destination-base64",
        ),
        (
            "scripts/sccp_solana_destination_evidence.py",
            "except (TypeError, ValueError, binascii.Error):",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token Solana destination base64 decoder detail",
        ),
        (
            "pytests/scripts/sccp_solana_destination_evidence_test.py",
            "secret-token-private-verifier.so",
        ),
        (
            "pytests/scripts/sccp_solana_source_state_evidence_test.py",
            "secret-token-solana-source-hex",
        ),
        (
            "pytests/scripts/sccp_solana_source_state_evidence_test.py",
            "secret-token Solana source hex TypeError detail",
        ),
        (
            "scripts/sccp_solana_source_state_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_solana_source_state_evidence.py",
            "except (OSError, SystemExit, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_solana_source_state_evidence_test.py",
            "for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-fixed-hex",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-code-hex",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token TON destination hex TypeError detail",
        ),
        (
            "scripts/sccp_ton_destination_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_ton_destination_evidence.py",
            "except (OSError, RuntimeError, TypeError, ValueError) as exc:",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "for exception_type in (RuntimeError, TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-code-boc",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token TON code BoC base64 TypeError detail",
        ),
        (
            "scripts/sccp_ton_destination_evidence.py",
            "except (TypeError, ValueError, binascii.Error):",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-file-path",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-account-status",
        ),
        (
            "pytests/scripts/sccp_ton_destination_evidence_test.py",
            "secret-token-ton-destination-last-lt",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token-route-metadata!",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token all-lanes base64",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def decode_comment_base64(field: str, label: str) -> bytes | None:",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def decode_base64(field: str, label: str) -> bytes | None:",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_solana_base64_callers_redact_typeerror_helper_causes",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token all-lanes {label} TypeError detail",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token {label} program bytes",
        ),
    )
    for inventory_path, removed_marker in adversarial_marker_cases:
        required_markers = next(
            markers
            for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
            if path == inventory_path
        )
        sparse_test = tmp_path / Path(inventory_path).name
        sparse_test.write_text(
            "\n".join(
                marker for marker in required_markers if marker != removed_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
            ((sparse_test, required_markers),)
        )

        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_markers = {
        "def test_release_bundle_verifier_rejects_padded_public_scalar_strings",
        'checklist["items"][0]["title"] = " All required lane records "',
        'lane["chain"] = " eth "',
        'lane["destination_binding"]["destination_binding_key"]',
        'route_canary["status"] = " passed "',
        'route_canary["evidence_source"] = " sccp-live-route-canary "',
        'crypto_row["route_canary_evidence_source"] = " sccp-live-route-canary "',
        'surface["on_chain_submission"]',
        "def test_release_bundle_verifier_rejects_release_checklist_field_type_drift",
        "def test_release_bundle_verifier_rejects_release_checklist_malformed_item_ids",
        "def test_release_bundle_verifier_rejects_all_lanes_list_scalar_type_drift",
        "def test_release_bundle_verifier_rejects_all_lanes_malformed_unknown_fields",
        "def test_release_bundle_verifier_rejects_all_lanes_destination_binding_field_shape",
        "def test_release_bundle_verifier_rejects_all_lanes_route_canary_field_drift",
        "def test_release_bundle_verifier_rejects_submission_surface_field_type_drift",
        "def test_release_bundle_verifier_rejects_corridor_malformed_unknown_fields",
        "def test_release_bundle_verifier_rejects_corridor_malformed_phase_keys",
        "def test_release_bundle_cli_redacts_top_level_exception_details",
        "for exception_type in (RuntimeError, TypeError, ValueError):",
        "def test_release_bundle_rejects_malformed_copied_corridor_phase_map_before_render",
        "def test_release_bundle_rejects_malformed_copied_crypto_evidence_before_render",
        "def test_release_bundle_rejects_malformed_copied_submission_surface_before_render",
        "def test_release_bundle_hex_predicates_redact_typeerror_parser_causes",
        "secret-token bundle hex TypeError detail",
        "def test_release_bundle_solana_pubkey_redacts_typeerror_parser_causes",
        "secret-token bundle Solana pubkey TypeError detail",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_scalar_text_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_markers):
        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )

    readiness_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SCALAR_TEXT_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_readiness_report_test.py"
    )
    removed_readiness_markers = {
        "def test_release_readiness_report_cli_redacts_top_level_exception_details",
        "for exception_type in (RuntimeError, TypeError, ValueError):",
        "def test_release_readiness_hex_predicates_redact_typeerror_parser_causes",
        "secret-token readiness hex TypeError detail",
    }
    sparse_readiness_test = tmp_path / "sccp_release_readiness_scalar_text_test.py"
    sparse_readiness_test.write_text(
        "\n".join(
            marker
            for marker in readiness_test_markers
            if marker not in removed_readiness_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_scalar_text_schema_gate_inventory_errors(
        ((sparse_readiness_test, readiness_test_markers),)
    )

    for removed_marker in sorted(removed_readiness_markers):
        assert any(
            "SCCP release public scalar-text schema source inventory" in error
            and str(sparse_readiness_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_notes_attachment_invariants_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin release-notes attachment invariants."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_notes_attachment_invariants_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _release_notes_attachment_invariant_errors(report, artifacts, notes):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_notes_attachment_invariants_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_NOTES_ATTACHMENT_INVARIANTS_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP release-notes attachment invariants source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: release notes attachment missing canonical title"
        in error
        for error in errors
    )
    assert any(
        "SCCP release-notes attachment invariants source inventory" in error
        and "missing marker: release notes attachment does not list manifest.json"
        in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_NOTES_ATTACHMENT_INVARIANTS_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_test_markers = {
        "def test_release_bundle_verifier_release_notes_invariants_require_status_and_blockers",
        "def test_release_bundle_verifier_rejects_release_notes_status_drift",
        "def test_release_bundle_release_note_status_compares_ready_exactly",
        "def test_release_bundle_release_notes_mark_malformed_blocker_containers",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_notes_invariants_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_test_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_notes_attachment_invariants_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_test_markers):
        assert any(
            "SCCP release-notes attachment invariants source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_readiness_markdown_invariants_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin public Markdown invariants."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_readiness_markdown_invariants_gate_inventory_errors() == []

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _readiness_markdown_invariant_errors(report, markdown):\n",
        encoding="utf-8",
    )
    errors = report._sccp_readiness_markdown_invariants_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_READINESS_MARKDOWN_INVARIANTS_MARKERS[1][1],
            ),
        )
    )

    assert any(
        "SCCP readiness Markdown invariants source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: READINESS_MARKDOWN_REQUIRED_SECTIONS" in error
        for error in errors
    )
    assert any(
        "SCCP readiness Markdown invariants source inventory" in error
        and "missing marker: readiness report Markdown does not match readiness report JSON"
        in error
        for error in errors
    )
    assert any(
        "SCCP readiness Markdown invariants source inventory" in error
        and "missing marker: SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE" in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_READINESS_MARKDOWN_INVARIANTS_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_test_markers = {
        "def test_release_bundle_verifier_markdown_invariants_require_public_sections",
        "def test_release_bundle_verifier_markdown_invariants_require_blocker_text",
        "def test_release_bundle_verifier_markdown_invariants_require_invalid_blocker_markers",
        "def test_release_bundle_verifier_suppresses_malformed_source_inventory_gate_markdown_leaks",
        "def test_release_bundle_verifier_suppresses_malformed_report_artifact_path_markdown_leaks",
        "def test_release_bundle_verifier_suppresses_crypto_evidence_malformed_markdown_leaks",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_readiness_md_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_test_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_readiness_markdown_invariants_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_test_markers):
        assert any(
            "SCCP readiness Markdown invariants source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_outbound_precallback_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum outbound pre-callback guards."""

    report = load_report_module()
    assert report._ethereum_outbound_precallback_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text(
        "Ethereum outbound prover callback must not see BSC requests\n",
        encoding="utf-8",
    )
    errors = report._ethereum_outbound_precallback_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    "Ethereum outbound prover callback must not see BSC requests",
                    "assert.equal(outboundProverCalled, false)",
                    "proofArtifactHash and provingKeyHash must match request",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet outbound pre-callback SDK test inventory" in error
        and str(sparse_test) in error
        and "missing marker: assert.equal(outboundProverCalled, false)" in error
        for error in errors
    )
    assert any(
        "Ethereum mainnet outbound pre-callback SDK test inventory" in error
        and "missing marker: proofArtifactHash and provingKeyHash must match request"
        in error
        for error in errors
    )

    implementation_cases = (
        (
            "sccp.js",
            "const proverArtifactRequestBytes =",
            "...proverArtifactRequestBytes,\n        ...publicSignalWordBytes,",
        ),
        (
            "sccp.py",
            "def _normalize_optional_groth16_prover_artifacts(",
            'prover_artifacts["proof_artifact_hash"]',
        ),
        (
            "SccpEvmProver.swift",
            "let proverArtifacts = try normalizeOptionalEvmGroth16ProverArtifacts(",
            "if let proverArtifacts {\n        try preimage.append(evmBytesFromHex32(proverArtifacts.proofArtifactHash",
        ),
        (
            "EvmSccpProver.kt",
            "val proverArtifacts = normalizeOptionalGroth16ProverArtifacts(",
            'preimage.write(hex32Bytes(proverArtifacts.proofArtifactHash, "proofArtifactHash"))',
        ),
        (
            "EvmSccpProver.java",
            "final Groth16ProverArtifacts proverArtifacts =",
            'write(preimage, hex32Bytes(proverArtifacts.proofArtifactHash(), "proofArtifactHash"))',
        ),
        (
            "EthereumMainnetSccp.cs",
            "var proverArtifacts = NormalizeOptionalGroth16ProverArtifacts(proofArtifactHash, provingKeyHash);",
            "payload.Write(HexToBytes(proverArtifacts.ProofArtifactHash, 32));",
        ),
    )
    for index, (filename, present_marker, removed_marker) in enumerate(
        implementation_cases
    ):
        sparse_impl = tmp_path / f"{index}_{filename}"
        sparse_impl.write_text(present_marker + "\n", encoding="utf-8")
        impl_errors = report._ethereum_outbound_precallback_gate_inventory_errors(
            (
                (
                    sparse_impl,
                    (
                        present_marker,
                        removed_marker,
                    ),
                ),
            )
        )

        assert any(
            "Ethereum mainnet outbound pre-callback SDK test inventory" in error
            and str(sparse_impl) in error
            and f"missing marker: {removed_marker}" in error
            for error in impl_errors
        )


def test_release_readiness_report_guards_ethereum_outbound_provider_validation_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum outbound provider guards."""

    report = load_report_module()
    assert report._ethereum_outbound_provider_validation_gate_inventory_errors() == []

    cases = (
        (
            "sccp.js",
            "let providerValidated = false;",
            "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
        ),
        (
            "sccp.dist.js",
            "let providerValidated = false;",
            'if (typeof submit === "function")',
        ),
        (
            "sccp.py",
            'provider = options.get("execution_provider", self.execution_provider)',
            "await self.validate_execution_provider_mainnet(provider)",
        ),
        (
            "sccp_test.py",
            "guarded_submit_called = False",
            "assert guarded_submit_called is False",
        ),
        (
            "SccpEvmProver.swift",
            "if let executionProvider {",
            "_ = try await validateExecutionProviderMainnet(executionProvider)",
        ),
        (
            "EvmSccpProver.kt",
            "executionProvider?.let { validateExecutionProviderMainnet(it) }",
            "return submitter.submit(buildEthereumCalldata(input))",
        ),
        (
            "EthereumMainnetSccp.java",
            "if (executionProvider != null) {",
            "validateExecutionProviderMainnet(executionProvider);",
        ),
        (
            "EthereumMainnetSccp.cs",
            "IEthereumMainnetExecutionProvider? executionProvider",
            "ValidateExecutionProviderMainnetAsync(",
        ),
    )
    for index, (filename, present_marker, removed_marker) in enumerate(cases):
        sparse_sdk = tmp_path / f"{index}_{filename}"
        sparse_sdk.write_text(present_marker + "\n", encoding="utf-8")
        errors = report._ethereum_outbound_provider_validation_gate_inventory_errors(
            (
                (
                    sparse_sdk,
                    (
                        present_marker,
                        removed_marker,
                    ),
                ),
            )
        )

        assert any(
            "Ethereum mainnet outbound provider validation source inventory" in error
            and str(sparse_sdk) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_local_admission_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum local-admission guards."""

    report = load_report_module()
    assert report._ethereum_local_admission_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text(
        "EthereumMainnetSccp builds ETH -> SORA local-admission submissions\n",
        encoding="utf-8",
    )
    errors = report._ethereum_local_admission_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    "EthereumMainnetSccp builds ETH -> SORA local-admission submissions",
                    "sourceVerifierMaterialHash must not be zero",
                    "metadata is not canonical",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet local-admission SDK test inventory" in error
        and str(sparse_test) in error
        and "missing marker: sourceVerifierMaterialHash must not be zero" in error
        for error in errors
    )
    assert any(
        "Ethereum mainnet local-admission SDK test inventory" in error
        and "missing marker: metadata is not canonical" in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_receipt_root_zero_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum receipt-root zero guards."""

    report = load_report_module()
    assert report._ethereum_receipt_root_zero_gate_inventory_errors() == []

    sparse_test = tmp_path / "SourceSccpProofHashesTest.kt"
    sparse_test.write_text(
        "SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)\n",
        encoding="utf-8",
    )
    errors = report._ethereum_receipt_root_zero_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    "SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
                    "assertFailsWith<IllegalArgumentException>",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet receipt-root zero rejection SDK test inventory" in error
        and str(sparse_test) in error
        and "missing marker: assertFailsWith<IllegalArgumentException>" in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_receipt_rlp_zero_topic_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum receipt-RLP zero-topic guards."""

    report = load_report_module()
    assert report._ethereum_receipt_rlp_zero_topic_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text("zeroTopicReceiptTrieProof\n", encoding="utf-8")
    errors = report._ethereum_receipt_rlp_zero_topic_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    "zeroTopicReceiptTrieProof",
                    'topics: [hex32("00")]',
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet receipt RLP zero-topic SDK test inventory" in error
        and str(sparse_test) in error
        and 'missing marker: topics: [hex32("00")]' in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_receipt_rlp_zero_address_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum receipt-RLP zero-address guards."""

    report = load_report_module()
    assert report._ethereum_receipt_rlp_zero_address_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text("zeroAddressReceiptTrieProof\n", encoding="utf-8")
    errors = report._ethereum_receipt_rlp_zero_address_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    "zeroAddressReceiptTrieProof",
                    'address: `0x${"00".repeat(20)}`',
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet receipt RLP zero-address SDK test inventory" in error
        and str(sparse_test) in error
        and 'missing marker: address: `0x${"00".repeat(20)}`' in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_receipt_source_event_context_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum source-event context guards."""

    report = load_report_module()
    assert report._ethereum_receipt_source_event_context_gate_inventory_errors() == []

    cases = (
        (
            "sccp_evm_receipt_proof_evidence_test.py",
            (
                "test_collect_receipt_proof_rejects_source_event_missing_context_fields",
                'for field in ("transactionHash", "blockHash", "blockNumber")',
            ),
            'for field in ("transactionHash", "blockHash", "blockNumber")',
        ),
        (
            "lib.rs",
            ("EVM source receipts must not contain duplicate matching SCCP logs",),
            "EVM source receipts must not contain duplicate matching SCCP logs",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_receipt_source_event_context_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet source-event context SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_receipt_source_event_mode_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum source-event mode guards."""

    report = load_report_module()
    assert report._ethereum_receipt_source_event_mode_gate_inventory_errors() == []

    cases = (
        (
            "sccp_evm_receipt_proof_evidence_test.py",
            (
                "test_collect_receipt_proof_requires_explicit_receipt_only_mode_without_source_bridge",
                "test_collect_receipt_proof_allows_explicit_receipt_only_mode",
            ),
            "test_collect_receipt_proof_allows_explicit_receipt_only_mode",
        ),
        (
            "sccp_evm_receipt_proof_evidence.py",
            (
                "allow_receipt_only_evidence: bool = False",
                "source_bridge_address is required for SCCP source-event evidence",
            ),
            "source_bridge_address is required for SCCP source-event evidence",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_receipt_source_event_mode_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet source-event evidence mode SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_receipt_source_event_zero_digest_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum source-event digest guards."""

    report = load_report_module()
    assert report._ethereum_receipt_source_event_zero_digest_gate_inventory_errors() == []

    cases = (
        (
            "sccp_evm_receipt_proof_evidence_test.py",
            (
                "test_collect_receipt_proof_rejects_zero_source_event_digest",
                "zero source event digest was accepted",
            ),
            "zero source event digest was accepted",
        ),
        (
            "sccp_evm_receipt_proof_evidence.py",
            (
                'method=f"receipt.logs[{index}].topics[1]"',
                'raise RuntimeError(f"{method} returned zero data")',
            ),
            'raise RuntimeError(f"{method} returned zero data")',
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_receipt_source_event_zero_digest_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet source-event zero digest SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_receipt_rpc_duplicate_json_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum receipt duplicate-key guards."""

    report = load_report_module()
    assert report._ethereum_receipt_rpc_duplicate_json_gate_inventory_errors() == []

    cases = (
        (
            "sccp_evm_receipt_proof_evidence_test.py",
            (
                "test_collect_receipt_proof_rejects_duplicate_json_rpc_result_keys",
                "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields",
                "test_receipt_json_rpc_redacts_transport_and_error_response_details",
            ),
            "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields",
        ),
        (
            "sccp_evm_receipt_proof_evidence.py",
            (
                "_json_object_without_duplicate_keys",
                "JSON-RPC returned duplicate JSON keys",
                "JSON-RPC {method} returned error response",
                "object_pairs_hook=_json_object_without_duplicate_keys",
            ),
            "object_pairs_hook=_json_object_without_duplicate_keys",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_receipt_rpc_duplicate_json_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet receipt RPC duplicate JSON SDK test inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_receipt_block_transaction_hash_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin block receipt tx-hash guards."""

    report = load_report_module()
    assert report._ethereum_receipt_block_transaction_hash_gate_inventory_errors() == []

    cases = (
        (
            "sccp_evm_receipt_proof_evidence_test.py",
            (
                "test_receipt_trie_builder_rejects_duplicate_transaction_hashes",
                'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]',
            ),
            'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]',
        ),
        (
            "sccp_evm_receipt_proof_evidence.py",
            (
                "seen_transaction_hashes: set[bytes] = set()",
                "block receipt transactionHash values must be unique",
            ),
            "block receipt transactionHash values must be unique",
        ),
        (
            "sccp.js",
            (
                "const seenTransactionHashes = new Set();",
                "block receipt transactionHash values must be unique",
            ),
            "const seenTransactionHashes = new Set();",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_receipt_block_transaction_hash_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet block receipt transactionHash SDK test inventory"
            in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_js_receipt_admission_guard_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin JS receipt admission guards."""

    report = load_report_module()
    assert report._ethereum_js_receipt_admission_guard_gate_inventory_errors() == []

    cases = (
        (
            "sccp.js",
            (
                "eth_getBlockReceipts target receipt must match transactionHash",
                "Ethereum mainnet receipt proof construction requires beaconFinality.",
            ),
            "Ethereum mainnet receipt proof construction requires beaconFinality.",
        ),
        (
            "sccp.js",
            (
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
                "await prove(immutableProverCallbackValue(evidence), options)",
            ),
            "await prove(immutableProverCallbackValue(evidence), options)",
        ),
        (
            "sccpEthereumMainnet.test.js",
            (
                'for (const field of ["finalizedHeaderRoot", "syncCommitteeRoot", "beaconSlot"])',
                "receipt proof construction requires beaconFinality\\\\.${field}",
            ),
            "receipt proof construction requires beaconFinality\\\\.${field}",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_js_receipt_admission_guard_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet JS receipt admission source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_sdk_receipt_metadata_guard_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin SDK receipt metadata guards."""

    report = load_report_module()
    assert report._ethereum_sdk_receipt_metadata_guard_gate_inventory_errors() == []

    cases = (
        (
            "SourceSccpProofHashes.kt",
            (
                "typed receipt type must fit one byte below 0x80",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
            "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        ),
        (
            "sccp.js",
            (
                "eth_getBlockReceipts target receipt RLP must match receipt",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
            "eth_getBlockReceipts target receipt RLP must match receipt",
        ),
        (
            "SccpEvmProver.swift",
            (
                '"blockReceipts.receiptRlp"',
                "canonicalEvmReceiptRlp(currentReceipt)",
            ),
            "canonicalEvmReceiptRlp(currentReceipt)",
        ),
    )
    for index, (filename, required_markers, removed_marker) in enumerate(cases):
        sparse_source = tmp_path / f"{index}_{filename}"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )
        errors = report._ethereum_sdk_receipt_metadata_guard_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "Ethereum mainnet SDK receipt metadata source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_noncanonical_chain_id_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin noncanonical Ethereum chain-id guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_noncanonical_chain_id_gate_inventory_errors() == []

    sparse_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_test.write_text("canonical JSON-RPC quantity\n", encoding="utf-8")
    errors = report._ethereum_noncanonical_chain_id_gate_inventory_errors(
        (
            (
                sparse_test,
                (
                    'for (const chainId of ["1", 1, "0x01", "0X1", " 0x1", "0x1 "])',
                    "canonical JSON-RPC quantity",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet noncanonical chain id SDK test inventory" in error
        and str(sparse_test) in error
        and (
            'missing marker: for (const chainId of ["1", 1, "0x01", '
            '"0X1", " 0x1", "0x1 "])'
        )
        in error
        for error in errors
    )

    native_cases = (
        (
            1,
            "SccpSolanaProverTests.swift",
            'let noncanonicalChainIds: [Any] = ["1", "0x01", "0X1", " 0x1", "0x1 ", 1]',
        ),
        (
            2,
            "EvmSccpProverTest.kt",
            'for (chainId in listOf<Any>("1", "0x01", "0X1", " 0x1", "0x1 ", 1L))',
        ),
        (
            3,
            "EvmSccpProverTests.java",
            'new Object[] {"1", "0x01", "0X1", " 0x1", "0x1 ", Long.valueOf(1L)}',
        ),
        (
            4,
            "SccpEthereumMainnetTests.cs",
            "foreach (var chainId in new object?[]",
        ),
    )
    for marker_index, filename, removed_marker in native_cases:
        required_markers = verifier.ETHEREUM_NONCANONICAL_CHAIN_ID_TEST_MARKERS[
            marker_index
        ][1]
        sparse_native_test = tmp_path / filename
        sparse_native_test.write_text(
            "\n".join(
                marker for marker in required_markers if marker != removed_marker
            ),
            encoding="utf-8",
        )
        native_errors = report._ethereum_noncanonical_chain_id_gate_inventory_errors(
            ((sparse_native_test, required_markers),)
        )

        assert any(
            "Ethereum mainnet noncanonical chain id SDK test inventory" in error
            and str(sparse_native_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in native_errors
        )

    receipt_vector_marker = (
        'for chain_id_result in ("0x01", "0X1", " 0x1", "0x1 ", 1):'
    )
    sparse_receipt_test = tmp_path / "sccp_evm_receipt_proof_evidence_test.py"
    sparse_receipt_test.write_text(
        "\n".join(
            marker
            for marker in verifier.ETHEREUM_NONCANONICAL_CHAIN_ID_TEST_MARKERS[5][1]
            if marker != receipt_vector_marker
        ),
        encoding="utf-8",
    )
    receipt_errors = report._ethereum_noncanonical_chain_id_gate_inventory_errors(
        (
            (
                sparse_receipt_test,
                verifier.ETHEREUM_NONCANONICAL_CHAIN_ID_TEST_MARKERS[5][1],
            ),
        )
    )
    assert any(
        "Ethereum mainnet noncanonical chain id SDK test inventory" in error
        and str(sparse_receipt_test) in error
        and f"missing marker: {receipt_vector_marker}" in error
        for error in receipt_errors
    )


def test_release_readiness_report_guards_ethereum_sync_committee_roster_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin exact mainnet sync-committee rosters."""

    report = load_report_module()
    assert report._ethereum_sync_committee_roster_gate_inventory_errors() == []

    sparse_source = tmp_path / "sccp.js"
    sparse_source.write_text(
        "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;\n",
        encoding="utf-8",
    )
    errors = report._ethereum_sync_committee_roster_gate_inventory_errors(
        (
            (
                sparse_source,
                (
                    "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
                    "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet sync-committee roster SDK test inventory" in error
        and str(sparse_source) in error
        and (
            "missing marker: syncCommitteeWeights[${index}] must be 1 "
            "for Ethereum mainnet"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_source_bridge_config_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum source-bridge config guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_source_bridge_config_gate_inventory_errors() == []

    required_markers = verifier.ETHEREUM_SOURCE_BRIDGE_CONFIG_MARKERS[0][1]
    removed_marker = "ETH_SOURCE_BRIDGE_CONFIG_PREFIX"
    sparse_source = tmp_path / "sccp_eth_source_bridge_evidence.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_source_bridge_config_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet source-bridge config source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def source_marker_inventory_with_one_marker_removed(
    tmp_path: Path,
    inventory: tuple[tuple[str | Path, tuple[str, ...]], ...],
    index: int,
) -> tuple[tuple[tuple[Path, tuple[str, ...]], ...], Path, str]:
    """Return a one-entry inventory fixture with one detectable marker removed."""

    original_path, required_markers = inventory[index]
    for removed_marker in required_markers:
        remaining_markers = tuple(
            marker for marker in required_markers if marker != removed_marker
        )
        if removed_marker not in "\n".join(remaining_markers):
            break
    else:
        raise AssertionError(f"{original_path} has no uniquely removable marker")

    sparse_source = tmp_path / f"source-inventory-{index}-{Path(original_path).name}"
    sparse_source.write_text("\n".join(remaining_markers), encoding="utf-8")
    return ((sparse_source, required_markers),), sparse_source, removed_marker


def test_release_readiness_report_guards_sccp_source_material_template_rejection_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin source-material template rejection."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_source_material_template_rejection_gate_inventory_errors()
        == []
    )

    for index, _entry in enumerate(
        verifier.SCCP_SOURCE_MATERIAL_TEMPLATE_REJECTION_MARKERS
    ):
        (
            sparse_inventory,
            sparse_source,
            removed_marker,
        ) = source_marker_inventory_with_one_marker_removed(
            tmp_path,
            verifier.SCCP_SOURCE_MATERIAL_TEMPLATE_REJECTION_MARKERS,
            index,
        )
        errors = report._sccp_source_material_template_rejection_gate_inventory_errors(
            sparse_inventory
        )

        assert any(
            "SCCP source-material template rejection source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_sccp_source_material_role_validation_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin source-material role validation."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_source_material_role_validation_gate_inventory_errors() == []

    for index, _entry in enumerate(
        verifier.SCCP_SOURCE_MATERIAL_ROLE_VALIDATION_MARKERS
    ):
        (
            sparse_inventory,
            sparse_source,
            removed_marker,
        ) = source_marker_inventory_with_one_marker_removed(
            tmp_path,
            verifier.SCCP_SOURCE_MATERIAL_ROLE_VALIDATION_MARKERS,
            index,
        )
        errors = report._sccp_source_material_role_validation_gate_inventory_errors(
            sparse_inventory
        )

        assert any(
            "SCCP source-material role validation source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )

    for inventory_path, removed_marker in (
        (
            "scripts/sccp_all_lanes_evidence.py",
            'raise ValueError(f"{label} must be base64") from None',
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            'raise ValueError(f"{label}:{line_number}: invalid metadata comment") from None',
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "except (TypeError, ValueError, binascii.Error):",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "except (argparse.ArgumentTypeError, TypeError, ValueError):",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def _minimal_toml_duplicate_key_detail(",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "duplicate key with sensitive name",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "duplicate key with malformed name",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def _toml_unsupported_section_detail(",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "except (SystemExit, TypeError, ValueError, RuntimeError):",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "except (argparse.ArgumentTypeError, SystemExit, TypeError, ValueError, RuntimeError):",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "unsupported zk section with sensitive name",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "unsupported zk section with malformed name",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_minimal_toml_parser_redacts_json_exception_causes",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "for exception_type in (TypeError, ValueError, RuntimeError):",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "for exception_type in (SystemExit, TypeError, ValueError, RuntimeError):",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_evidence_redacts_destination_binding_recompute_failures",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_evidence_redacts_destination_identity_failures",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_minimal_toml_parser_redacts_sensitive_duplicate_keys",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_minimal_toml_parser_redacts_unsupported_section_names",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_loader_redacts_unsupported_zk_section_names",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_metadata_comment_redacts_json_exception_causes",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_base64_helper_redacts_parser_causes",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "source_record_exception_types = (",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "source_validator_exception_types = (",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token string",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token-duplicate-key",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "route|operator-duplicate-key",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token-section",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "route|operator-section",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token-zk-section",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "route|operator-zk-section",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token comment",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token all-lanes base64",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token all-lanes hex TypeError detail",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "except (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def decode_comment_base64(field: str, label: str) -> bytes | None:",
        ),
        (
            "scripts/sccp_all_lanes_evidence.py",
            "def decode_base64(field: str, label: str) -> bytes | None:",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "def test_all_lanes_solana_base64_callers_redact_typeerror_helper_causes",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token all-lanes {label} TypeError detail",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token destination binding material",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "pytests/scripts/sccp_all_lanes_evidence_test.py",
            "for exception_type in (TypeError, ValueError):",
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            'raise RuntimeError(f"{label} must be 32-byte hex or base64") from None',
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            'raise RuntimeError("TON verifier account code_boc is invalid") from None',
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            'raise ValueError("TON live code BoC base64 metadata is invalid") from None',
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            "except (TypeError, binascii.Error, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "def test_live_ton_hash_decoder_redacts_base64_parser_causes",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token hash base64",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "for exception_type in (TypeError, ValueError):",
        ),
        (
            "pytests/scripts/sccp_ton_live_evidence_test.py",
            "secret-token {label} parser detail",
        ),
        (
            "scripts/sccp_ton_live_evidence.py",
            "except (argparse.ArgumentTypeError, TypeError, ValueError):",
        ),
    ):
        required_markers = next(
            markers
            for path, markers in verifier.SCCP_SOURCE_MATERIAL_ROLE_VALIDATION_MARKERS
            if path == inventory_path
        )
        sparse_source = tmp_path / f"all-lanes-{Path(inventory_path).name}"
        sparse_source.write_text(
            "\n".join(
                marker for marker in required_markers if marker != removed_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_source_material_role_validation_gate_inventory_errors(
            ((sparse_source, required_markers),)
        )

        assert any(
            "SCCP source-material role validation source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_evm_source_adapter_deployment_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum source-adapter gates."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._ethereum_evm_source_adapter_deployment_gate_inventory_errors() == []
    )

    required_markers = verifier.ETHEREUM_EVM_SOURCE_ADAPTER_DEPLOYMENT_GATE_MARKERS[
        0
    ][1]
    for index, removed_marker in enumerate(
        (
            "wrong_config_deployment.source_bridge_config_hash[0] ^= 0x01;",
            "source adapter deployment metadata must keep adapter VK and receipt hashes separated",
            "BSC facade must reject replayed deployment receipts",
            "ETH facade must reject replayed deployment receipts",
        )
    ):
        sparse_source = tmp_path / f"lib_{index}.rs"
        sparse_source.write_text(
            "\n".join(marker for marker in required_markers if marker != removed_marker),
            encoding="utf-8",
        )

        errors = report._ethereum_evm_source_adapter_deployment_gate_inventory_errors(
            (
                (
                    sparse_source,
                    required_markers,
                ),
            )
        )

        assert any(
            "Ethereum mainnet EVM source-adapter deployment gate source inventory"
            in error
            and str(sparse_source) in error
            and removed_marker in error
            for error in errors
        )


def test_release_readiness_report_guards_contract_smoke_eth_mainnet_network_id_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin EVM smoke ETH network-id guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._contract_smoke_eth_mainnet_network_id_gate_inventory_errors() == []

    required_markers = verifier.CONTRACT_SMOKE_ETH_MAINNET_NETWORK_ID_MARKERS[0][1]
    removed_marker = 'callExceptionWithReason("Network id must be ETH mainnet")'
    sparse_source = tmp_path / "sccp_message_bridge_smoke.js"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._contract_smoke_eth_mainnet_network_id_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "EVM contract smoke Ethereum mainnet network id source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_contract_smoke_evm_production_surface_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin EVM smoke production-surface guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._contract_smoke_evm_production_surface_gate_inventory_errors() == []

    required_markers = verifier.CONTRACT_SMOKE_EVM_PRODUCTION_SURFACE_MARKERS[0][1]
    removed_marker = 'callExceptionWithReason("Verifier key hash mismatch")'
    sparse_source = tmp_path / "sccp_message_bridge_smoke.js"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._contract_smoke_evm_production_surface_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "EVM contract smoke production surface source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_core_range_finality_binding_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Core range/finality binding."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_core_range_finality_binding_gate_inventory_errors() == []

    required_markers = verifier.ETHEREUM_CORE_RANGE_FINALITY_BINDING_MARKERS[0][1]
    removed_marker = "SCCP message proof range must match finality height"
    sparse_source = tmp_path / "world.rs"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_core_range_finality_binding_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet SCCP range finality binding source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_core_message_replay_guard_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Core message replay guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_core_message_replay_guard_gate_inventory_errors() == []

    required_markers = verifier.ETHEREUM_CORE_MESSAGE_REPLAY_GUARD_MARKERS[0][1]
    removed_marker = "SCCP message proof replays existing message proof"
    sparse_source = tmp_path / "world.rs"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_core_message_replay_guard_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet SCCP message replay guard source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_torii_pinned_message_proof_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Torii pinned message proofs."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_torii_pinned_message_proof_gate_inventory_errors() == []

    required_markers = verifier.ETHEREUM_TORII_PINNED_MESSAGE_PROOF_MARKERS[0][1]
    removed_marker = "SCCP message bridge proofs must be pinned for core replay protection"
    sparse_source = tmp_path / "routing.rs"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_torii_pinned_message_proof_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet Torii pinned message proof source inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_evm_source_live_production_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum live source evidence."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._ethereum_evm_source_live_production_gate_inventory_errors() == []

    required_markers = verifier.ETHEREUM_EVM_SOURCE_LIVE_PRODUCTION_MARKERS[0][1]
    removed_marker = "deployment receipt block receiptsRoot metadata must be verified"
    sparse_source = tmp_path / "sccp_evm_source_live_evidence.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "deployment receipt transactionHash must be a non-zero bytes32"
    sparse_source = tmp_path / "sccp_evm_source_live_receipt_fields.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "JSON-RPC {method} returned invalid JSON"
    sparse_source = tmp_path / "sccp_evm_source_live_invalid_json.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    source_test_markers = next(
        markers
        for path, markers in verifier.ETHEREUM_EVM_SOURCE_LIVE_PRODUCTION_MARKERS
        if path == "pytests/scripts/sccp_evm_source_live_evidence_test.py"
    )
    removed_marker = "secret-token invalid EVM source JSON-RPC payload"
    sparse_source_test = tmp_path / "sccp_evm_source_live_invalid_json_test.py"
    sparse_source_test.write_text(
        "\n".join(
            marker for marker in source_test_markers if marker != removed_marker
        ),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source_test,
                source_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source_test) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "secret-token {target_method} parser detail"
    sparse_source_test = tmp_path / "sccp_evm_source_live_evidence_test.py"
    sparse_source_test.write_text(
        "\n".join(
            marker for marker in source_test_markers if marker != removed_marker
        ),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source_test,
                source_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source_test) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "for exception_type in (TypeError, RuntimeError, ValueError):"
    sparse_source_test = tmp_path / "sccp_evm_source_live_type_error_test.py"
    sparse_source_test.write_text(
        "\n".join(
            marker for marker in source_test_markers if marker != removed_marker
        ),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
        (
            (
                sparse_source_test,
                source_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM source production SDK test inventory" in error
        and str(sparse_source_test) in error
        and removed_marker in error
        for error in errors
    )

    for removed_marker_index, removed_marker in enumerate(
        (
            "test_evm_source_live_cli_redacts_top_level_exception_details",
            "secret-token-evm-source-error",
        )
    ):
        sparse_source_test = tmp_path / (
            f"sccp_evm_source_live_cli_redaction_{removed_marker_index}_test.py"
        )
        sparse_source_test.write_text(
            "\n".join(
                marker for marker in source_test_markers if marker != removed_marker
            ),
            encoding="utf-8",
        )

        errors = report._ethereum_evm_source_live_production_gate_inventory_errors(
            (
                (
                    sparse_source_test,
                    source_test_markers,
                ),
            )
        )

        assert any(
            "Ethereum mainnet live EVM source production SDK test inventory"
            in error
            and str(sparse_source_test) in error
            and removed_marker in error
            for error in errors
        )


def test_release_readiness_report_guards_ethereum_evm_live_destination_production_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin Ethereum live destination evidence."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._ethereum_evm_live_destination_production_gate_inventory_errors()
        == []
    )

    required_markers = verifier.ETHEREUM_EVM_LIVE_DESTINATION_PRODUCTION_MARKERS[0][1]
    removed_marker = "route-canary proofBytes must not be all zero"
    sparse_source = tmp_path / "sccp_evm_live_evidence.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "generated EVM destination TOML arguments are invalid"
    sparse_source = tmp_path / "sccp_evm_live_evidence_generated_toml.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "JSON-RPC {method} returned invalid JSON"
    sparse_source = tmp_path / "sccp_evm_live_evidence_invalid_json.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_source,
                required_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )

    live_test_markers = next(
        markers
        for path, markers in verifier.ETHEREUM_EVM_LIVE_DESTINATION_PRODUCTION_MARKERS
        if path == "pytests/scripts/sccp_evm_live_evidence_test.py"
    )
    removed_marker = "secret-token invalid EVM JSON-RPC payload"
    sparse_live_test = tmp_path / "sccp_evm_live_evidence_invalid_json_test.py"
    sparse_live_test.write_text(
        "\n".join(marker for marker in live_test_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_live_test,
                live_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_live_test) in error
        and removed_marker in error
        for error in errors
    )

    removed_marker = "secret-token generated EVM destination TOML parser detail"
    sparse_live_test = tmp_path / "sccp_evm_live_evidence_test.py"
    sparse_live_test.write_text(
        "\n".join(marker for marker in live_test_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_live_test,
                live_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_live_test) in error
        and removed_marker in error
        for error in errors
    )

    for removed_marker_index, removed_marker in enumerate(
        (
            "test_evm_live_cli_redacts_top_level_exception_details",
            "secret-token-evm-error",
        )
    ):
        sparse_live_test = tmp_path / (
            f"sccp_evm_live_cli_redaction_{removed_marker_index}_test.py"
        )
        sparse_live_test.write_text(
            "\n".join(
                marker for marker in live_test_markers if marker != removed_marker
            ),
            encoding="utf-8",
        )

        errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
            (
                (
                    sparse_live_test,
                    live_test_markers,
                ),
            )
        )

        assert any(
            "Ethereum mainnet live EVM destination production SDK test inventory"
            in error
            and str(sparse_live_test) in error
            and removed_marker in error
            for error in errors
        )

    destination_test_markers = next(
        markers
        for path, markers in verifier.ETHEREUM_EVM_LIVE_DESTINATION_PRODUCTION_MARKERS
        if path == "pytests/scripts/sccp_evm_destination_evidence_test.py"
    )
    removed_marker = "secret-token-verifier-runtime"
    sparse_destination_test = tmp_path / "sccp_evm_destination_evidence_test.py"
    sparse_destination_test.write_text(
        "\n".join(
            marker
            for marker in destination_test_markers
            if marker != removed_marker
        ),
        encoding="utf-8",
    )

    errors = report._ethereum_evm_live_destination_production_gate_inventory_errors(
        (
            (
                sparse_destination_test,
                destination_test_markers,
            ),
        )
    )

    assert any(
        "Ethereum mainnet live EVM destination production SDK test inventory"
        in error
        and str(sparse_destination_test) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_ethereum_route_canary_finalized_receipt_block_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin route-canary receipt finality."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._ethereum_route_canary_finalized_receipt_block_gate_inventory_errors()
        == []
    )

    required_markers = verifier.ETHEREUM_ROUTE_CANARY_FINALIZED_RECEIPT_BLOCK_MARKERS[
        0
    ][1]
    removed_marker = 'receipt_block_finalized=finalized_block["receipt_block_finalized"]'
    sparse_source = tmp_path / "sccp_evm_live_evidence.py"
    sparse_source.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = (
        report._ethereum_route_canary_finalized_receipt_block_gate_inventory_errors(
            (
                (
                    sparse_source,
                    required_markers,
                ),
            )
        )
    )

    assert any(
        "Ethereum mainnet route-canary finalized receipt block SDK test inventory"
        in error
        and str(sparse_source) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_unready_transparent_proof_config_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin config-owned unready proof toggles."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_unready_transparent_proof_config_gate_inventory_errors() == []

    sparse_config = tmp_path / "user.rs"
    sparse_config.write_text(
        "pub sccp_allow_unready_transparent_proofs: bool\n"
        "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS\n",
        encoding="utf-8",
    )

    errors = report._sccp_unready_transparent_proof_config_gate_inventory_errors(
        (
            (
                sparse_config,
                verifier.SCCP_UNREADY_TRANSPARENT_PROOF_CONFIG_MARKERS[0][1],
            ),
        ),
        (sparse_config,),
    )

    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and str(sparse_config) in error
        and (
            "missing marker: sccp_allow_unready_transparent_proofs: "
            "self.sccp_allow_unready_transparent_proofs"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and str(sparse_config) in error
        and "contains forbidden environment override" in error
        for error in errors
    )

    sparse_bsc_test = tmp_path / "sccp_bsc_taira_xor_deploy.test.mjs"
    sparse_bsc_test.write_text(
        "BSC route-config refuses allow-unready for production-ready manifests\n",
        encoding="utf-8",
    )
    errors = report._sccp_unready_transparent_proof_config_gate_inventory_errors(
        (
            (
                sparse_bsc_test,
                verifier.SCCP_UNREADY_TRANSPARENT_PROOF_CONFIG_MARKERS[5][1],
            ),
        ),
        (),
    )

    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and str(sparse_bsc_test) in error
        and (
            "missing marker: assert.match(toml, "
            "/sccp_allow_unready_transparent_proofs = false/u);"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and (
            "missing marker: buildBscTairaXorRouteConfigToml(manifest, {"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and (
            "missing marker: BSC route-config rejects malformed allow-unready "
            "option values"
        )
        in error
        for error in errors
    )

    sparse_tron_test = tmp_path / "sccp_tron_taira_xor_deploy.test.mjs"
    sparse_tron_test.write_text(
        "TRON route-config refuses allow-unready for production-ready manifests\n",
        encoding="utf-8",
    )
    errors = report._sccp_unready_transparent_proof_config_gate_inventory_errors(
        (
            (
                sparse_tron_test,
                verifier.SCCP_UNREADY_TRANSPARENT_PROOF_CONFIG_MARKERS[6][1],
            ),
        ),
        (),
    )

    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and str(sparse_tron_test) in error
        and (
            'missing marker: buildTairaXorRouteConfigToml(manifest, { '
            '"allow-unready": "true" })'
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and (
            "missing marker: TRON route-config rejects malformed allow-unready "
            "option values"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP unready transparent-proof config-only source inventory" in error
        and (
            "missing marker: /production-ready route manifests cannot enable "
            "--allow-unready/u"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_tron_deploy_operator_boolean_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin TRON operator boolean guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._tron_deploy_operator_boolean_gate_inventory_errors() == []

    sparse_script = tmp_path / "sccp_tron_taira_xor_deploy.mjs"
    sparse_script.write_text(
        "function optionEnabled(options, key, fallback = false) {}\n",
        encoding="utf-8",
    )
    errors = report._tron_deploy_operator_boolean_gate_inventory_errors(
        (
            (
                sparse_script,
                verifier.TRON_DEPLOY_OPERATOR_BOOLEAN_MARKERS[0][1],
            ),
        )
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and str(sparse_script) in error
        and 'missing marker: if (options[key] === "true") return true;'
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and 'missing marker: throw new Error("--broadcast must be true or false");'
        in error
        for error in errors
    )

    sparse_test = tmp_path / "sccp_tron_taira_xor_deploy.test.mjs"
    sparse_test.write_text(
        "TRON deploy operator booleans reject malformed option values\n",
        encoding="utf-8",
    )
    errors = report._tron_deploy_operator_boolean_gate_inventory_errors(
        (
            (
                sparse_test,
                verifier.TRON_DEPLOY_OPERATOR_BOOLEAN_MARKERS[1][1],
            ),
        )
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and str(sparse_test) in error
        and "missing marker: /--force must be true or false/u" in error
        for error in errors
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and str(sparse_test) in error
        and (
            "missing marker: TRON route-manifest readiness booleans reject "
            "malformed option values"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and "missing marker: /--production-ready must be true or false/u" in error
        for error in errors
    )
    assert any(
        "SCCP TRON deploy operator boolean source inventory" in error
        and 'missing marker: "require-optional-packages"' in error
        for error in errors
    )


def test_release_readiness_report_guards_native_sccp_no_wasm_readiness_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin native no-WASM/no-remote guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._native_sccp_no_wasm_readiness_gate_inventory_errors() == []

    required_markers = verifier.NATIVE_SCCP_NO_WASM_READINESS_TEST_MARKERS[0][1]
    removed_marker = "def _native_evm_prover_forbidden_payload_blockers("
    readiness_script = tmp_path / "sccp_release_readiness_report.py"
    readiness_script.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._native_sccp_no_wasm_readiness_gate_inventory_errors(
        (
            (
                readiness_script,
                required_markers,
            ),
        )
    )

    assert any(
        "native SCCP no-WASM readiness SDK test inventory" in error
        and str(readiness_script) in error
        and removed_marker in error
        for error in errors
    )

    markers_by_path = dict(verifier.NATIVE_SCCP_NO_WASM_READINESS_TEST_MARKERS)
    package_dist_markers = markers_by_path[
        "javascript/iroha_js/test/package_dist.test.js"
    ]
    package_dist_regression_markers = (
        "browser SCCP no-WASM guard catches remote-prover identifier variants",
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned",
        "ipfs:proof-artifact.bin",
        "artifacts/eth-mainnet/proof.wasm",
        "WebAssembly.compile(bytes)",
        "import './proof.wasm'",
        "fallback remote prover",
        "const proverEndpoint = endpoint",
    )
    for marker in package_dist_regression_markers:
        assert marker in package_dist_markers

    sparse_package_dist = tmp_path / "package_dist.test.js"
    sparse_package_dist.write_text(
        "function assertBrowserMainnetSccpArtifactsStayJsOnlyAndLocalProverOwned() {}\n",
        encoding="utf-8",
    )

    errors = report._native_sccp_no_wasm_readiness_gate_inventory_errors(
        (
            (
                sparse_package_dist,
                package_dist_markers,
            ),
        )
    )

    for role_floor_marker in (
        "proofArtifactBytes must be at least 65536 bytes",
        "provingKeyBytes must be at least 65536 bytes",
        "verifierKeyBytes must be at least 128 bytes",
        "crossSdkFixtureParityBytes must be at least 128 bytes",
        "nativeProverSelfTestBytes must be at least 128 bytes",
        "implementationBytes must be at least 1024 bytes",
    ):
        assert any(
            "native SCCP no-WASM readiness SDK test inventory" in error
            and str(sparse_package_dist) in error
            and f"missing marker: {role_floor_marker}" in error
            for error in errors
        )
    for marker in package_dist_regression_markers:
        assert any(
            "native SCCP no-WASM readiness SDK test inventory" in error
            and str(sparse_package_dist) in error
            and f"missing marker: {marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_native_evm_canonical_sdk_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin padded native SDK-id regressions."""

    report = load_report_module()
    verifier = load_verify_helpers()
    markers_by_path = dict(verifier.NATIVE_SCCP_NO_WASM_READINESS_TEST_MARKERS)
    js_path = "javascript/iroha_js/test/sccpEthereumMainnet.test.js"
    required_markers = markers_by_path[js_path]
    removed_marker = 'sdk: " javascript "'
    sparse_js_test = tmp_path / "sccpEthereumMainnet.test.js"
    sparse_js_test.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = report._native_sccp_no_wasm_readiness_gate_inventory_errors(
        (
            (
                sparse_js_test,
                required_markers,
            ),
        )
    )

    assert any(
        "native SCCP no-WASM readiness SDK test inventory" in error
        and str(sparse_js_test) in error
        and removed_marker in error
        for error in errors
    )

    kotlin_path = (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/"
        "EvmSccpProverTest.kt"
    )
    kotlin_markers = markers_by_path[kotlin_path]
    kotlin_marker = "assertFalse(paddedSelfTestHookCalled)"
    sparse_kotlin_test = tmp_path / "EvmSccpProverTest.kt"
    sparse_kotlin_test.write_text(
        "\n".join(marker for marker in kotlin_markers if marker != kotlin_marker),
        encoding="utf-8",
    )
    errors = report._native_sccp_no_wasm_readiness_gate_inventory_errors(
        ((sparse_kotlin_test, kotlin_markers),)
    )

    assert any(
        "native SCCP no-WASM readiness SDK test inventory" in error
        and str(sparse_kotlin_test) in error
        and f"missing marker: {kotlin_marker}" in error
        for error in errors
    )

    java_path = (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/"
        "EvmSccpProverTests.java"
    )
    java_markers = markers_by_path[java_path]
    java_marker = "Ethereum native prover self-test callback must not run with padded sdk"
    sparse_java_test = tmp_path / "EvmSccpProverTests.java"
    sparse_java_test.write_text(
        "\n".join(marker for marker in java_markers if marker != java_marker),
        encoding="utf-8",
    )
    errors = report._native_sccp_no_wasm_readiness_gate_inventory_errors(
        ((sparse_java_test, java_markers),)
    )

    assert any(
        "native SCCP no-WASM readiness SDK test inventory" in error
        and str(sparse_java_test) in error
        and f"missing marker: {java_marker}" in error
        for error in errors
    )


def test_release_readiness_evidence_phase_requires_evm_script_suites() -> None:
    """The evidence phase transcript must prove the EVM evidence suites ran."""

    report = load_report_module()
    required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]

    for fragment in EVM_EVIDENCE_SCRIPT_FRAGMENTS:
        assert fragment in required_fragments


def test_release_readiness_evidence_phase_inventory_matches_corridor_runner() -> None:
    """The evidence transcript gate must track the runner's pytest inventory."""

    report = load_report_module()
    required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]

    for test_path in corridor_evidence_script_tests():
        assert any(test_path in fragment for fragment in required_fragments)


def test_release_readiness_evidence_phase_requires_retired_network_surface_scan() -> None:
    """Readiness reports must prove the retired-network surface scan ran."""

    report = load_report_module()
    retired_scan = "pytests/scripts/sccp_retired_network_surface_test.py"

    assert retired_scan in corridor_evidence_script_tests()
    assert retired_scan in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]


def test_release_readiness_verifier_reports_removed_retired_network_pipeline_doc_guard(
    tmp_path: Path,
) -> None:
    """Readiness coverage must keep translated pipeline-doc scan guards pinned."""

    verifier = load_verify_helpers()
    required_markers = verifier.SCCP_RETIRED_NETWORK_SURFACE_GUARD_MARKERS[0][1]
    removed_marker = "def test_retired_network_surface_scan_covers_pipeline_translations"
    guard = tmp_path / "sccp_retired_network_surface_test.py"
    guard.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = verifier._sccp_retired_network_surface_guard_inventory_errors(
        (
            (
                guard,
                required_markers,
            ),
        )
    )

    assert any(
        "SCCP retired network-surface guard source inventory" in error
        and str(guard) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_verifier_reports_removed_generic_no_support_note_guard(
    tmp_path: Path,
) -> None:
    """Readiness coverage must keep launch-scope no-support note guards pinned."""

    verifier = load_verify_helpers()
    required_markers = verifier.SCCP_RETIRED_NETWORK_SURFACE_GUARD_MARKERS[0][1]
    removed_marker = "def test_generic_no_support_note_stays_in_launch_scope_files"
    guard = tmp_path / "sccp_retired_network_surface_test.py"
    guard.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = verifier._sccp_retired_network_surface_guard_inventory_errors(
        (
            (
                guard,
                required_markers,
            ),
        )
    )

    assert any(
        "SCCP retired network-surface guard source inventory" in error
        and str(guard) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_verifier_reports_removed_specific_no_support_note_guard(
    tmp_path: Path,
) -> None:
    """Readiness coverage must keep exact launch-scope no-support wording pinned."""

    verifier = load_verify_helpers()
    required_markers = verifier.SCCP_RETIRED_NETWORK_SURFACE_GUARD_MARKERS[0][1]
    removed_marker = "def test_specific_no_support_note_stays_in_launch_scope_files"
    guard = tmp_path / "sccp_retired_network_surface_test.py"
    guard.write_text(
        "\n".join(marker for marker in required_markers if marker != removed_marker),
        encoding="utf-8",
    )

    errors = verifier._sccp_retired_network_surface_guard_inventory_errors(
        (
            (
                guard,
                required_markers,
            ),
        )
    )

    assert any(
        "SCCP retired network-surface guard source inventory" in error
        and str(guard) in error
        and removed_marker in error
        for error in errors
    )


def test_release_readiness_report_guards_retired_network_surface_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness source inventory must pin retired network-surface guards."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._sccp_retired_network_surface_gate_inventory_errors() == []

    for index, _entry in enumerate(
        verifier.SCCP_RETIRED_NETWORK_SURFACE_GUARD_MARKERS
    ):
        (
            sparse_inventory,
            sparse_source,
            removed_marker,
        ) = source_marker_inventory_with_one_marker_removed(
            tmp_path,
            verifier.SCCP_RETIRED_NETWORK_SURFACE_GUARD_MARKERS,
            index,
        )
        errors = report._sccp_retired_network_surface_gate_inventory_errors(
            sparse_inventory
        )

        assert any(
            "SCCP retired network-surface guard source inventory" in error
            and str(sparse_source) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_sccp_proof_request_bundle_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin SCCP proof-request bundle/source-proof gates."""

    report = load_report_module()
    assert report._sccp_proof_request_bundle_gate_inventory_errors() == []
    verifier = report._load_release_bundle_verify_helpers()
    inventory_paths = {
        str(path) for path, _ in verifier.SCCP_PROOF_REQUEST_BUNDLE_GATE_MARKERS
    }
    assert "IrohaSwift/Sources/IrohaSwift/SccpMessageProofBundle.swift" in inventory_paths
    assert (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/"
        "SccpMessageProofBundles.kt"
    ) in inventory_paths
    assert (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/"
        "SccpMessageProofBundles.java"
    ) in inventory_paths
    inventory_by_path = dict(verifier.SCCP_PROOF_REQUEST_BUNDLE_GATE_MARKERS)
    javascript_impl_markers = inventory_by_path["javascript/iroha_js/src/sccp.js"]
    assert (
        "proof.proofFamily !== SCCP_STARK_FRI_PROOF_FAMILY_V1"
        in javascript_impl_markers
    )
    assert (
        "sourceStateProof must be a TON source-state stark-fri-v1 proof"
        in javascript_impl_markers
    )
    assert (
        "export function wrapTonSccpSourceStateVerificationProof(proofBytes, request) {\n"
        "  const proofRequest = normalizeTonSourceStateProofRequestForWrapping(request);\n"
        "  const proof = copyBytes(toBytes(proofBytes, \"proofBytes\"));\n"
        "  requireSourceStateProofBytes(proof, \"proofBytes\");"
        in javascript_impl_markers
    )
    javascript_dist_markers = inventory_by_path["javascript/iroha_js/dist/sccp.js"]
    assert (
        "proof.proofFamily !== SCCP_STARK_FRI_PROOF_FAMILY_V1"
        in javascript_dist_markers
    )
    assert (
        "sourceStateProof must be a TON source-state stark-fri-v1 proof"
        in javascript_dist_markers
    )
    assert (
        "export function wrapTonSccpSourceStateVerificationProof(proofBytes, request) {\n"
        "  const proofRequest = normalizeTonSourceStateProofRequestForWrapping(request);\n"
        "  const proof = copyBytes(toBytes(proofBytes, \"proofBytes\"));\n"
        "  requireSourceStateProofBytes(proof, \"proofBytes\");"
        in javascript_dist_markers
    )
    javascript_test_markers = inventory_by_path[
        "javascript/iroha_js/test/sccpSolanaProver.test.js"
    ]
    assert "oversizedTonSourceStateProofBytes" in javascript_test_markers
    assert "oversizedTonCallbackProver" in javascript_test_markers
    assert 'proofFamily: "debug-proof-family"' in javascript_test_markers
    assert "TON source-state stark-fri-v1 proof" in javascript_test_markers
    javascript_package_dist_markers = inventory_by_path[
        "javascript/iroha_js/test/package_dist.test.js"
    ]
    assert (
        "package dist entrypoint enforces TON source-state proof cap"
        in javascript_package_dist_markers
    )
    assert (
        "oversizedTonDistSourceStateProofBytes"
        in javascript_package_dist_markers
    )
    assert "distTonDebugProofFamily" in javascript_package_dist_markers
    assert "TON source-state stark-fri-v1 proof" in javascript_package_dist_markers
    assert "oversizedTonDistCallbackProver" in javascript_package_dist_markers
    javascript_package_root_markers = inventory_by_path[
        "javascript/iroha_js/test/sccpPackageExports.test.js"
    ]
    assert (
        "published package root enforces TON source-state proof cap"
        in javascript_package_root_markers
    )
    assert (
        "samplePackageRootTonShardStateSourceStateInput"
        in javascript_package_root_markers
    )
    assert "packageRootTonDebugProofFamily" in javascript_package_root_markers
    assert "TON source-state stark-fri-v1 proof" in javascript_package_root_markers
    assert (
        "oversizedTonPackageRootSourceStateProofBytes"
        in javascript_package_root_markers
    )
    assert (
        "oversizedTonPackageRootCallbackProver"
        in javascript_package_root_markers
    )
    assert (
        "samplePackageRootEvmFamilyProofBundleFixture"
        in javascript_package_root_markers
    )
    assert (
        "published package root enforces SCCP proof-request bundle source-domain binding"
        in javascript_package_root_markers
    )
    assert "packageRootEvmSolanaSourceBundle" in javascript_package_root_markers
    assert "packageRootTronSolanaSourceBundle" in javascript_package_root_markers
    assert (
        "bundleBytes\\.sourceDomain must match sourceDomain"
        in javascript_package_root_markers
    )
    javascript_bsc_test_markers = inventory_by_path[
        "javascript/iroha_js/test/sccpBscMainnet.test.js"
    ]
    assert "tamperedBscBase64ProofResult" in javascript_bsc_test_markers
    assert (
        "sdk.buildBscCalldata({ proofResult: tamperedBscBase64ProofResult })"
        in javascript_bsc_test_markers
    )
    javascript_eth_test_markers = inventory_by_path[
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js"
    ]
    assert "tamperedEthereumBase64ProofResult" in javascript_eth_test_markers
    assert (
        "sdk.buildEthereumCalldata({ proofResult: tamperedEthereumBase64ProofResult })"
        in javascript_eth_test_markers
    )
    python_impl_markers = inventory_by_path["python/iroha_torii_client/sccp.py"]
    assert "def wrap_ton_sccp_source_state_verification_proof" in python_impl_markers
    assert "_require_source_state_proof_bytes(proof)" in python_impl_markers
    assert (
        'proof["proof_family"] != SCCP_STARK_FRI_PROOF_FAMILY_V1'
        in python_impl_markers
    )
    assert (
        "sourceStateProof must be a TON source-state stark-fri-v1 proof"
        in python_impl_markers
    )
    python_test_markers = inventory_by_path[
        "python/iroha_torii_client/tests/sccp_test.py"
    ]
    assert (
        "def test_ton_source_state_prover_wraps_shard_and_full_light_audit_role_proofs"
        in python_test_markers
    )
    assert (
        'oversized_proof_bytes = b"\\x01" * (SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1)'
        in python_test_markers
    )
    assert (
        "prove=lambda _request, _options: oversized_proof_bytes"
        in python_test_markers
    )
    assert '"proof_family": "debug-proof-family"' in python_test_markers
    assert "TON source-state stark-fri-v1 proof" in python_test_markers
    assert (
        "def test_package_root_ton_source_state_cap_uses_public_exports"
        in python_test_markers
    )
    assert "package_root_ton_debug_proof_family" in python_test_markers
    assert (
        "oversized_package_root_ton_source_state_proof"
        in python_test_markers
    )
    assert "def sample_token_add_bundle_fixture" in python_test_markers
    assert "lowercase_required_eip55_recipient" in python_test_markers
    assert "lowercase_required_eip55_sender" in python_test_markers
    assert "nul_prefixed_symbol_bundle" in python_test_markers
    assert "tampered_bsc_base64_proof_result" in python_test_markers
    assert "build_bsc_mainnet_sccp_destination_submission" in python_test_markers
    assert "tampered_ethereum_base64_proof_result" in python_test_markers
    assert "build_ethereum_calldata" in python_test_markers
    assert (
        "proofResult\\.proofBase64 must match proofResult\\.proofBytes"
        in python_test_markers
    )
    kotlin_ton_impl_markers = inventory_by_path[
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/TonSccpProver.kt"
    ]
    assert (
        "SOURCE_STATE_MAX_PROOF_BYTES: Int = NATIVE_RECURSIVE_MAX_PROOF_BYTES"
        in kotlin_ton_impl_markers
    )
    assert "proof.proofFamily == STARK_FRI_PROOF_FAMILY_V1" in kotlin_ton_impl_markers
    assert "proofBytes.size <= SOURCE_STATE_MAX_PROOF_BYTES" in kotlin_ton_impl_markers
    kotlin_ton_test_markers = inventory_by_path[
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/TonSccpProverTest.kt"
    ]
    assert (
        "oversizedSourceStateProofBytes = ByteArray(SccpTon.SOURCE_STATE_MAX_PROOF_BYTES + 1)"
        in kotlin_ton_test_markers
    )
    assert "oversizedCallbackProver" in kotlin_ton_test_markers
    assert 'proofFamily = "debug-proof-family"' in kotlin_ton_test_markers
    java_ton_impl_markers = inventory_by_path[
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/TonSccpProver.java"
    ]
    assert (
        "SOURCE_STATE_MAX_PROOF_BYTES = NATIVE_RECURSIVE_MAX_PROOF_BYTES"
        in java_ton_impl_markers
    )
    assert (
        "!STARK_FRI_PROOF_FAMILY_V1.equals(proof.proofFamily())"
        in java_ton_impl_markers
    )
    assert (
        "normalizedProofBytes.length > SOURCE_STATE_MAX_PROOF_BYTES"
        in java_ton_impl_markers
    )
    java_ton_test_markers = inventory_by_path[
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/TonSccpProverTests.java"
    ]
    assert "TonSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1" in java_ton_test_markers
    assert "oversizedCallbackProver" in java_ton_test_markers
    assert '"debug-proof-family"' in java_ton_test_markers
    assert (
        "TON source-state verification proof family must be stark-fri-v1"
        in java_ton_test_markers
    )
    kotlin_evm_test_markers = inventory_by_path[
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt"
    ]
    assert "tamperedBscBase64ProofResultError" in kotlin_evm_test_markers
    assert "tamperedEthereumBase64ProofResultError" in kotlin_evm_test_markers
    assert "SccpBsc.buildSubmission" in kotlin_evm_test_markers
    assert "buildEthereumCalldata" in kotlin_evm_test_markers
    java_evm_test_markers = inventory_by_path[
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java"
    ]
    assert "tamperedBscBase64ProofResult" in java_evm_test_markers
    assert (
        "Ethereum mainnet calldata helper must reject tampered proofBase64"
        in java_evm_test_markers
    )
    assert "BscSccpProver.buildSubmission" in java_evm_test_markers
    assert 'evmResultWithProofBase64(artifactBoundResult, "AAAA")' in java_evm_test_markers
    dotnet_eth_test_markers = inventory_by_path[
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpEthereumMainnetTests.cs"
    ]
    assert "OutboundCallbackAndSubmissionSnapshotsRejectMutation" in dotnet_eth_test_markers
    assert "EthereumMainnetSccp.BuildEthereumCalldata" in dotnet_eth_test_markers
    assert (
        "ProofBase64 = Convert.ToBase64String(mutatedProofBytes)"
        in dotnet_eth_test_markers
    )
    dotnet_bsc_test_markers = inventory_by_path[
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpBscMainnetTests.cs"
    ]
    assert "OutboundCallbackAndSubmissionSnapshotsRejectMutation" in dotnet_bsc_test_markers
    assert "BscMainnetSccp.BuildBscCalldata" in dotnet_bsc_test_markers
    assert (
        "ProofBase64 = Convert.ToBase64String(mutatedProofBytes)"
        in dotnet_bsc_test_markers
    )
    swift_ton_impl_markers = inventory_by_path[
        "IrohaSwift/Sources/IrohaSwift/SccpTonProver.swift"
    ]
    assert (
        "proof.proofBytes.count <= sccpSourceStateMaxProofBytes"
        in swift_ton_impl_markers
    )
    assert "proofBytes.count <= sccpSourceStateMaxProofBytes" in swift_ton_impl_markers
    assert "proof.proofFamily == tonStarkFriProofFamilyV1" in swift_ton_impl_markers
    swift_ton_test_markers = inventory_by_path[
        "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift"
    ]
    assert "oversizedTonSourceStateProofBytes" in swift_ton_test_markers
    assert "sccpSourceStateMaxProofBytes + 1" in swift_ton_test_markers
    assert "oversizedTonCallbackProver" in swift_ton_test_markers
    assert 'proofFamily: "debug-proof-family"' in swift_ton_test_markers
    assert "tamperedBscBase64ProofResult" in swift_ton_test_markers
    assert "tamperedEthereumBase64ProofResult" in swift_ton_test_markers
    assert (
        "buildBscMainnetSccpDestinationSubmission(EvmSccpSubmissionInput("
        in swift_ton_test_markers
    )
    assert "buildEthereumCalldata(EvmSccpSubmissionInput(" in swift_ton_test_markers
    assert 'invalidPublicInputs("proofResult.proofBase64")' in swift_ton_test_markers

    sparse_gate = tmp_path / "sccp_test.py"
    sparse_gate.write_text(
        "def test_ton_sccp_proof_request_rejects_noncanonical_or_mismatched_bundle_bytes():\n"
        "    pass\n",
        encoding="utf-8",
    )
    missing_marker = "sourceProofBytes required for non-SORA source bundle"

    errors = report._sccp_proof_request_bundle_gate_inventory_errors(
        (
            (
                sparse_gate,
                (
                    "def test_ton_sccp_proof_request_rejects_noncanonical_or_mismatched_bundle_bytes",
                    missing_marker,
                ),
            ),
        )
    )

    assert any(
        "SCCP proof-request bundle/source-proof gate source inventory" in error
        and str(sparse_gate) in error
        and missing_marker in error
        for error in errors
    )

    sparse_swift_impl = tmp_path / "SccpTonProver.swift"
    sparse_swift_impl.write_text(
        "private func requireTonSccpProofRequestBundleMatchesPublicInputs() {}\n"
        "private func decodeCanonicalTonSccpMessageProofBundleSummary() {}\n",
        encoding="utf-8",
    )
    missing_impl_marker = (
        "summary.sourceDomain == sccpDomainSora || !sourceProofBytes.isEmpty"
    )

    implementation_errors = report._sccp_proof_request_bundle_gate_inventory_errors(
        (
            (
                sparse_swift_impl,
                (
                    "func requireTonSccpProofRequestBundleMatchesPublicInputs",
                    "func decodeCanonicalTonSccpMessageProofBundleSummary",
                    missing_impl_marker,
                ),
            ),
        )
    )

    assert any(
        "SCCP proof-request bundle/source-proof gate source inventory" in error
        and str(sparse_swift_impl) in error
        and missing_impl_marker in error
        for error in implementation_errors
    )

    sparse_dist = tmp_path / "dist-sccp.js"
    sparse_dist.write_text(
        "const requireSccpProofRequestBundleMatchesPublicInputs = () => {};\n"
        "summary.sourceDomain !== SCCP_DOMAIN_SORA;\n",
        encoding="utf-8",
    )
    missing_dist_marker = 'toBytes(sourceProofBytes, "sourceProofBytes").length === 0'

    dist_errors = report._sccp_proof_request_bundle_gate_inventory_errors(
        (
            (
                sparse_dist,
                (
                    "const requireSccpProofRequestBundleMatchesPublicInputs = (",
                    "summary.sourceDomain !== SCCP_DOMAIN_SORA",
                    missing_dist_marker,
                    "sourceProofBytes required for non-SORA source bundle",
                ),
            ),
        )
    )

    assert any(
        "SCCP proof-request bundle/source-proof gate source inventory" in error
        and str(sparse_dist) in error
        and missing_dist_marker in error
        for error in dist_errors
    )

    package_root_required_markers = inventory_by_path[
        "javascript/iroha_js/test/sccpPackageExports.test.js"
    ]
    for index, removed_marker in enumerate(
        (
            "published package root enforces SCCP proof-request bundle source-domain binding",
            "packageRootEvmSolanaSourceBundle",
            "packageRootTronSolanaSourceBundle",
        )
    ):
        sparse_package_root = tmp_path / f"package-root-proof-request-{index}.js"
        sparse_package_root.write_text(
            "\n".join(
                marker
                for marker in package_root_required_markers
                if marker != removed_marker
            ),
            encoding="utf-8",
        )

        package_root_errors = report._sccp_proof_request_bundle_gate_inventory_errors(
            ((sparse_package_root, package_root_required_markers),)
        )

        assert any(
            "SCCP proof-request bundle/source-proof gate source inventory" in error
            and str(sparse_package_root) in error
            and f"missing marker: {removed_marker}" in error
            for error in package_root_errors
        ), removed_marker


def test_release_readiness_report_guards_sccp_phase_evidence_source_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin fail-closed phase evidence source handling."""

    report = load_report_module()
    assert report._sccp_phase_evidence_source_gate_inventory_errors() == []

    sparse_report = tmp_path / "sccp_release_readiness_report.py"
    sparse_report.write_text(
        "duplicate SCCP corridor phase evidence for\n",
        encoding="utf-8",
    )
    errors = report._sccp_phase_evidence_source_gate_inventory_errors(
        (
            (
                sparse_report,
                (
                    "duplicate SCCP corridor phase evidence for",
                    "already set by",
                    "cannot set from",
                    "source_labels",
                    "def _phase_evidence_source_label(",
                    "--phase-evidence {name}=<path>",
                    "def _phase_log_from_dir(",
                    "checked standard phase log layouts",
                    "def _parse_phase_assignment_name(",
                    "unknown SCCP corridor phase",
                    "phase result must use NAME=STATUS syntax",
                    "phase result name contains surrounding whitespace",
                    "phase result name contains Markdown-unsafe character",
                    "phase result name contains malformed phase",
                    "phase result status is empty",
                    "phase result status contains control character",
                    "phase result status contains non-ASCII character",
                    "phase result status contains surrounding whitespace",
                    "phase result status contains whitespace",
                    "phase result status must be passed, failed, skipped, or missing",
                    "phase evidence must use NAME=PATH syntax",
                    "phase evidence path must not be empty",
                    "phase evidence name contains surrounding whitespace",
                    "phase evidence name contains Markdown-unsafe character",
                    "phase evidence name contains malformed phase",
                ),
            ),
        )
    )

    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and str(sparse_report) in error
        and "missing marker: already set by" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: cannot set from" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: def _phase_evidence_source_label(" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: --phase-evidence {name}=<path>" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: def _phase_log_from_dir(" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: checked standard phase log layouts" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: unknown SCCP corridor phase" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase evidence name contains surrounding whitespace" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result name contains Markdown-unsafe character"
        in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result must use NAME=STATUS syntax" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase evidence must use NAME=PATH syntax" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase evidence path must not be empty" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase evidence name contains Markdown-unsafe character"
        in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result name contains malformed phase" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result status contains surrounding whitespace" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result status is empty" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result status contains control character" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result status contains non-ASCII character" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and "missing marker: phase result status contains whitespace" in error
        for error in errors
    )
    assert any(
        "SCCP phase evidence duplicate-input source inventory" in error
        and (
            "missing marker: phase result status must be passed, failed, "
            "skipped, or missing"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_release_corridor_phase_transcript_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin strict corridor phase transcript checks."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert report._release_corridor_phase_transcript_gate_inventory_errors() == []

    sparse_report = tmp_path / "sccp_release_readiness_report.py"
    sparse_report.write_text(
        "def _phase_transcript_errors(bundle_dir, phase, artifact):\n",
        encoding="utf-8",
    )
    errors = report._release_corridor_phase_transcript_gate_inventory_errors(
        (
            (
                sparse_report,
                (
                    "def _phase_transcript_errors(",
                    "def _phase_transcript_block(",
                    "def _known_corridor_phase_marker_lines(",
                    "def _unknown_corridor_phase_marker_lines(",
                    "def _transcript_has_multiple_known_phase_markers(",
                    "def _transcript_has_nonempty_line_before_first_phase_marker(",
                    "if lines[index] in known_markers:",
                    "def _phase_marker_count(",
                    "marker_positions != sorted(marker_positions)",
                    "first_phase_command_position = min(phase_command_positions)",
                    "position > first_phase_command_position",
                    "for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:",
                    "for fragment in PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase, ()):",
                    "line == CORRIDOR_COMPLETION_SENTINEL",
                    "def _phase_effective_command_tokens(",
                    "PYTEST_OPTIONS_WITH_VALUES",
                    "NODE_TEST_OPTIONS_WITH_VALUES",
                    "def _command_option_values(",
                    "def _command_has_option_value(",
                    "def _command_positional_tokens(",
                    "def _pytest_command_positionals(",
                    "def _pytest_expected_positionals_for_phase(",
                    "tuple(pytest_positionals) != expected_positionals",
                    "def _node_expected_test_files_for_phase(",
                    "def _node_test_command_files(",
                    "def _node_check_command_matches(",
                    "def _dotnet_sdk_command_matches(",
                    "def _phase_prefix_env_assignments(",
                    "def _android_harness_mains_classes(",
                    "def _gradle_test_selector_matches(",
                    "def _gradle_test_command_selectors(",
                    "actual_harness_classes = _android_harness_mains_classes(command)",
                    "tuple(tokens) == tuple(shlex.split(fragment))",
                    "_command_token_basename(tokens[0]) == fragment_tokens[0]",
                    "shlex.split(command, comments=True)",
                    'tokens.index("&&") == 2',
                    "def _effective_command_equals(",
                    "def _phase_block_command_fragment_line_indices(",
                    "def _phase_block_output_fragment_line_indices(",
                    "SUCCESS_OUTPUT_NEGATION_PATTERN",
                    "SUCCESS_OUTPUT_DIAGNOSTIC_PREFIX_PATTERN",
                    "SHELL_XTRACE_COMMAND_PATTERN",
                    "def _phase_output_line_has_success_fragment(",
                    "def _line_is_shell_xtrace_command(",
                    "SHELL_XTRACE_COMMAND_PATTERN.match(normalized_line)",
                    "def _phase_block_has_exact_output_line(",
                    "def _phase_block_has_completion_after_required_evidence(",
                    "first_command_position = min(command_positions_before_completion)",
                    "first_command_position < position < completion_position",
                    "def _phase_block_has_traced_command_after_completion(",
                    "def _transcript_has_traced_command_after_completion(",
                    "def _phase_block_has_nonempty_line_after_completion(",
                    "def _transcript_has_nonempty_line_after_completion(",
                    "ANSI_ESCAPE_PATTERN",
                    "ASCII_CONTROL_CHARACTER_PATTERN",
                    "def _phase_output_failure_scan_line(",
                    'unicodedata.category(character) != "Cf"',
                    "def _phase_diagnostic_fragment(",
                    'replace("|", "\\\\x7c")',
                    "_phase_diagnostic_fragment(fragment)",
                    "_phase_diagnostic_fragment(forbidden_marker)",
                    "evidence artifact contains unknown corridor phase marker",
                    "contains non-empty output before first phase marker",
                    "evidence artifact has duplicate phase marker",
                    "evidence artifact contains incomplete multi-phase corridor transcript",
                    "completion sentinel precedes required phase evidence",
                    "contains traced command after completion sentinel",
                    "contains non-empty output after completion sentinel",
                    "evidence artifact is missing expected phase-block success marker:",
                    "evidence artifact contains forbidden phase-block",
                ),
            ),
        )
    )

    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and str(sparse_report) in error
        and "missing marker: def _phase_transcript_block(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and str(sparse_report) in error
        and "missing marker: def _phase_marker_count(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: first_phase_command_position = min(phase_command_positions)"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: position > first_phase_command_position" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _known_corridor_phase_marker_lines(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _unknown_corridor_phase_marker_lines(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _transcript_has_multiple_known_phase_markers("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _transcript_has_nonempty_line_before_first_phase_marker("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_output_failure_scan_line(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and 'missing marker: unicodedata.category(character) != "Cf"' in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_diagnostic_fragment(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and 'missing marker: replace("|", "\\\\x7c")' in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: _phase_diagnostic_fragment(fragment)" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: _phase_diagnostic_fragment(forbidden_marker)" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: if lines[index] in known_markers:" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_effective_command_tokens(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _command_option_values(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _pytest_command_positionals(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _pytest_expected_positionals_for_phase(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: tuple(pytest_positionals) != expected_positionals" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _node_expected_test_files_for_phase(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _node_test_command_files(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _node_check_command_matches(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _dotnet_sdk_command_matches(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_prefix_env_assignments(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _android_harness_mains_classes(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _gradle_test_selector_matches(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _gradle_test_command_selectors(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: actual_harness_classes = "
            "_android_harness_mains_classes(command)"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: tuple(tokens) == tuple(shlex.split(fragment))" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: _command_token_basename(tokens[0]) == fragment_tokens[0]"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: shlex.split(command, comments=True)" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and 'missing marker: tokens.index("&&") == 2' in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _effective_command_equals(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: marker_positions != sorted(marker_positions)" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: evidence artifact contains unknown corridor "
            "phase marker"
            in error
        )
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: contains non-empty output before first phase marker"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: evidence artifact contains incomplete multi-phase "
            "corridor transcript"
            in error
        )
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: for fragment in PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: for fragment in "
            "PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS.get(phase, ()):"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: line == CORRIDOR_COMPLETION_SENTINEL" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_has_exact_output_line(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_command_fragment_line_indices("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_output_fragment_line_indices("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: SUCCESS_OUTPUT_NEGATION_PATTERN" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: SUCCESS_OUTPUT_DIAGNOSTIC_PREFIX_PATTERN" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: SHELL_XTRACE_COMMAND_PATTERN" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_output_line_has_success_fragment(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _line_is_shell_xtrace_command(" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: SHELL_XTRACE_COMMAND_PATTERN.match(normalized_line)"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_has_completion_after_required_evidence("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: first_command_position = "
            "min(command_positions_before_completion)"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: first_command_position < position < completion_position"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_has_traced_command_after_completion("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _transcript_has_traced_command_after_completion("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _phase_block_has_nonempty_line_after_completion("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: def _transcript_has_nonempty_line_after_completion("
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: evidence artifact has duplicate phase marker" in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: completion sentinel precedes required phase evidence"
            in error
        )
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: contains traced command after completion sentinel"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and "missing marker: contains non-empty output after completion sentinel"
        in error
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: evidence artifact is missing expected "
            "phase-block success marker:"
            in error
        )
        for error in errors
    )
    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and (
            "missing marker: evidence artifact contains forbidden phase-block"
            in error
        )
        for error in errors
    )

    runner_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_CORRIDOR_PHASE_TRANSCRIPT_MARKERS
        if path == "pytests/scripts/check_sccp_production_corridor_test.py"
    )
    removed_marker = "test_sccp_production_corridor_rejects_empty_log_dir"
    sparse_runner = tmp_path / "check_sccp_production_corridor_test.py"
    sparse_runner.write_text(
        "\n".join(marker for marker in runner_markers if marker != removed_marker),
        encoding="utf-8",
    )
    errors = report._release_corridor_phase_transcript_gate_inventory_errors(
        ((sparse_runner, runner_markers),)
    )

    assert any(
        "SCCP release corridor phase-transcript source inventory" in error
        and str(sparse_runner) in error
        and f"missing marker: {removed_marker}" in error
        for error in errors
    )


def test_release_readiness_report_guards_sccp_release_bundle_source_copy_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin fail-closed release bundle source copying."""

    report = load_report_module()
    assert report._sccp_release_bundle_source_copy_gate_inventory_errors() == []

    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "def _reject_path_control_characters(path, label):\n"
        "    raise SystemExit('contains control character')\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_bundle_source_copy_gate_inventory_errors(
        (
            (
                sparse_bundle,
                (
                    "def _reject_path_control_characters(",
                    "contains control character",
                    "release bundle source path must not be a symlink",
                    "release bundle source path ancestor must not be a symlink",
                ),
            ),
        )
    )

    assert any(
        "SCCP release bundle source-copy source inventory" in error
        and str(sparse_bundle) in error
        and "missing marker: release bundle source path must not be a symlink"
        in error
        for error in errors
    )
    assert any(
        "SCCP release bundle source-copy source inventory" in error
        and (
            "missing marker: release bundle source path ancestor must not be "
            "a symlink"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_sccp_release_bundle_output_path_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin fail-closed bundle output paths."""

    report = load_report_module()
    assert report._sccp_release_bundle_output_path_gate_inventory_errors() == []

    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "def _reject_symlinked_existing_output_path(output_dir):\n"
        "    pass\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_bundle_output_path_gate_inventory_errors(
        (
            (
                sparse_bundle,
                (
                    "def _reject_symlinked_existing_output_path(",
                    "release bundle output directory must not be a symlink",
                    "release bundle output directory ancestor must not be a symlink",
                ),
            ),
        )
    )

    assert any(
        "SCCP release bundle output-path source inventory" in error
        and str(sparse_bundle) in error
        and (
            "missing marker: release bundle output directory must not be "
            "a symlink"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP release bundle output-path source inventory" in error
        and (
            "missing marker: release bundle output directory ancestor must not "
            "be a symlink"
        )
        in error
        for error in errors
    )


def test_release_readiness_report_guards_sccp_release_artifact_path_text_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin Markdown-safe release artifact paths."""

    report = load_report_module()
    assert report._sccp_release_artifact_path_text_gate_inventory_errors() == []

    sparse_report = tmp_path / "sccp_release_readiness_report.py"
    sparse_report.write_text(
        "MARKDOWN_UNSAFE_PATH_CHARACTERS = frozenset('|`<>')\n"
        "def _path_markdown_unsafe_character(path):\n"
        "    return None\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_artifact_path_text_gate_inventory_errors(
        (
            (
                sparse_report,
                (
                    "MARKDOWN_UNSAFE_PATH_CHARACTERS",
                    "def _path_markdown_unsafe_character(",
                    "release artifact path contains Markdown-unsafe character",
                    "path contains Markdown-unsafe character",
                ),
            ),
        )
    )

    assert any(
        "SCCP release artifact path text source inventory" in error
        and str(sparse_report) in error
        and (
            "missing marker: release artifact path contains "
            "Markdown-unsafe character"
        )
        in error
        for error in errors
    )
    assert any(
        "SCCP release artifact path text source inventory" in error
        and "missing marker: path contains Markdown-unsafe character" in error
        for error in errors
    )

    verifier = load_verify_helpers()
    readiness_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_ARTIFACT_PATH_TEXT_MARKERS
        if path == "pytests/scripts/sccp_release_readiness_report_test.py"
    )
    removed_readiness_markers = {
        "test_release_readiness_report_blocks_missing_sccp_release_artifact_path_text_gate",
        "test_release_readiness_rejects_markdown_unsafe_artifact_paths",
        "test_release_readiness_rejects_padded_artifact_paths",
        "test_release_readiness_rejects_percent_encoded_artifact_traversal_paths",
        "secret-token-complete",
        'assert "secret-token" not in completed.stderr',
        "test_release_readiness_rejects_markdown_unsafe_native_evm_payload_paths",
        "test_release_readiness_rejects_control_character_native_evm_payload_paths",
        "test_release_readiness_rejects_padded_native_evm_payload_paths",
        "test_release_readiness_report_blocks_native_evm_prover_percent_encoded_path",
    }
    sparse_readiness_test = tmp_path / "sccp_release_readiness_artifact_path_test.py"
    sparse_readiness_test.write_text(
        "\n".join(
            marker
            for marker in readiness_test_markers
            if marker not in removed_readiness_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_artifact_path_text_gate_inventory_errors(
        ((sparse_readiness_test, readiness_test_markers),)
    )

    for marker in removed_readiness_markers:
        assert any(
            "SCCP release artifact path text source inventory" in error
            and str(sparse_readiness_test) in error
            and f"missing marker: {marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_input_provenance_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin copied input provenance schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_input_provenance_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _input_provenance_schema_errors(report):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_input_provenance_schema_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_INPUT_PROVENANCE_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release input-provenance schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: def _canonical_report_input_path_errors(" in error
        for error in errors
    )
    assert any(
        "SCCP release input-provenance schema source inventory" in error
        and "missing marker: readiness report inputs do not match copied input artifacts"
        in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_INPUT_PROVENANCE_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    removed_bundle_markers = {
        "test_release_bundle_rejects_missing_copied_report_inputs_before_render",
        "test_release_bundle_rejects_malformed_copied_input_provenance_before_render",
        "bundled report.inputs path must not contain surrounding whitespace",
        "bundled report.inputs path contains percent-encoded traversal segment",
        "bundled report.input_artifacts[3] path must not contain surrounding whitespace",
        "bundled report.input_artifacts[4] path contains percent-encoded traversal segment",
        "test_release_bundle_verifier_rejects_input_path_drift",
        "test_release_bundle_verifier_rejects_input_provenance_schema_drift",
        "test_release_bundle_verifier_rejects_report_artifact_path_drift",
        "test_release_bundle_verifier_rejects_copied_input_layout_drift",
        "secret-token-complete",
        "secret-token-renamed",
        'assert "secret-token" not in verified.stdout',
        "test_release_bundle_verifier_requires_copied_evidence_inputs",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_input_provenance_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_input_provenance_schema_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    for removed_marker in sorted(removed_bundle_markers):
        assert any(
            "SCCP release input-provenance schema source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_public_json_root_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin public JSON-root schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_json_root_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _load_json(path):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_public_json_root_schema_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_PUBLIC_JSON_ROOT_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release public JSON-root schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: object_pairs_hook=_reject_duplicate_json_keys" in error
        for error in errors
    )
    assert any(
        "SCCP release public JSON-root schema source inventory" in error
        and "missing marker: all-lanes summary JSON is not UTF-8 text" in error
        for error in errors
    )

    blocker_marker = 'f"{inventory_label} validation_blockers must be empty"'
    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "\n".join(
            marker
            for marker in verifier.SCCP_RELEASE_PUBLIC_JSON_ROOT_SCHEMA_MARKERS[2][1]
            if marker != blocker_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_json_root_schema_gate_inventory_errors(
        (
            (
                sparse_bundle,
                verifier.SCCP_RELEASE_PUBLIC_JSON_ROOT_SCHEMA_MARKERS[2][1],
            ),
        )
    )

    assert any(
        "SCCP release public JSON-root schema source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {blocker_marker}" in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_JSON_ROOT_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    source_inventory_gate_marker = (
        "contains unknown gate name with Markdown-unsafe character"
    )
    copied_source_inventory_blocker_markers = {
        source_inventory_gate_marker,
        "validation_status must be passed",
        "validation_blockers must be a list of non-empty strings",
        "validation_blockers must not contain duplicate strings",
        "validation_blockers must be empty",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_public_json_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in copied_source_inventory_blocker_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_json_root_schema_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    assert any(
        "SCCP release public JSON-root schema source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {source_inventory_gate_marker}" in error
        for error in errors
    )
    for removed_marker in sorted(
        copied_source_inventory_blocker_markers - {source_inventory_gate_marker}
    ):
        assert any(
            "SCCP release public JSON-root schema source inventory" in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )


def test_release_readiness_report_guards_release_public_markdown_text_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin public Markdown text schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_markdown_text_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        'report_md_path.read_text(encoding="utf-8")\n',
        encoding="utf-8",
    )
    errors = report._sccp_release_public_markdown_text_schema_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_PUBLIC_MARKDOWN_TEXT_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: release-notes attachment is not UTF-8 text" in error
        for error in errors
    )
    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and "missing marker: release notes attachment does not match manifest and report"
        in error
        for error in errors
    )

    readiness_drift_marker = (
        "readiness report Markdown does not match readiness report JSON"
    )
    release_notes_drift_marker = (
        "release notes attachment does not match manifest and report"
    )
    removed_bundle_markers = {
        readiness_drift_marker,
        release_notes_drift_marker,
    }
    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "\n".join(
            marker
            for marker in verifier.SCCP_RELEASE_PUBLIC_MARKDOWN_TEXT_SCHEMA_MARKERS[2][1]
            if marker not in removed_bundle_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_markdown_text_schema_gate_inventory_errors(
        (
            (
                sparse_bundle,
                verifier.SCCP_RELEASE_PUBLIC_MARKDOWN_TEXT_SCHEMA_MARKERS[2][1],
            ),
        )
    )

    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {readiness_drift_marker}" in error
        for error in errors
    )
    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {release_notes_drift_marker}" in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_MARKDOWN_TEXT_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    markdown_prewrite_marker = "test_release_bundle_rejects_markdown_drift_before_write"
    release_notes_prewrite_marker = (
        "test_release_bundle_rejects_release_notes_drift_before_write"
    )
    removed_bundle_test_markers = {
        markdown_prewrite_marker,
        release_notes_prewrite_marker,
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_public_markdown_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_bundle_test_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_markdown_text_schema_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {markdown_prewrite_marker}" in error
        for error in errors
    )
    assert any(
        "SCCP release public Markdown text schema source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {release_notes_prewrite_marker}" in error
        for error in errors
    )


def test_release_readiness_report_guards_release_public_crypto_evidence_binding_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin public crypto-evidence binding."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors()
        == []
    )

    verifier_markers = verifier.SCCP_RELEASE_PUBLIC_CRYPTO_EVIDENCE_BINDING_MARKERS[
        0
    ][1]
    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _cryptographic_evidence_inventory_errors(crypto):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier_markers,
            ),
        )
    )

    assert any(
        "SCCP release public cryptographic-evidence binding source inventory"
        in error
        and str(sparse_verifier) in error
        and "missing marker: def _cryptographic_evidence_row_schema_errors("
        in error
        for error in errors
    )
    assert any(
        "SCCP release public cryptographic-evidence binding source inventory"
        in error
        and str(sparse_verifier) in error
        and "missing marker: def _cryptographic_evidence_lane_binding_errors("
        in error
        for error in errors
    )
    assert any(
        "SCCP release public cryptographic-evidence binding source inventory"
        in error
        and "missing marker: readiness report cryptographic_evidence does not match embedded lane evidence"
        in error
        for error in errors
    )

    for removed_verifier_marker in (
        "SENSITIVE_PUBLIC_FIELD_NAME_MARKERS = (",
        "def _unexpected_source_adapter_gate_audit_field_blocker(",
        "contains unexpected field with sensitive name",
    ):
        sparse_verifier_marker = tmp_path / (
            f"sccp_verify_crypto_{removed_verifier_marker[:8]}.py"
        )
        sparse_verifier_marker.write_text(
            "\n".join(
                marker for marker in verifier_markers if marker != removed_verifier_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
            ((sparse_verifier_marker, verifier_markers),)
        )

        assert any(
            "SCCP release public cryptographic-evidence binding source inventory"
            in error
            and str(sparse_verifier_marker) in error
            and f"missing marker: {removed_verifier_marker}" in error
            for error in errors
        )

    bundle_script_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_CRYPTO_EVIDENCE_BINDING_MARKERS
        if path == "scripts/sccp_release_bundle.py"
    )
    for removed_bundle_script_marker in (
        "def _unexpected_source_adapter_gate_audit_field_error(",
        "contains unexpected field with sensitive name",
    ):
        sparse_bundle_script = tmp_path / (
            f"sccp_release_bundle_{removed_bundle_script_marker[:8]}.py"
        )
        sparse_bundle_script.write_text(
            "\n".join(
                marker
                for marker in bundle_script_markers
                if marker != removed_bundle_script_marker
            ),
            encoding="utf-8",
        )
        errors = report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
            ((sparse_bundle_script, bundle_script_markers),)
        )

        assert any(
            "SCCP release public cryptographic-evidence binding source inventory"
            in error
            and str(sparse_bundle_script) in error
            and f"missing marker: {removed_bundle_script_marker}" in error
            for error in errors
        )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_CRYPTO_EVIDENCE_BINDING_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    confusable_audit_suppression_marker = (
        "assert confusable_audit_key not in captured.err"
    )
    removed_schema_markers = {
        confusable_audit_suppression_marker,
        "test_release_bundle_verifier_rejects_crypto_evidence_zero_hashes",
        "test_release_bundle_verifier_rejects_crypto_evidence_domain_policy_drift",
        "test_release_bundle_verifier_rejects_crypto_evidence_field_type_drift",
        "test_release_bundle_redacts_sensitive_copied_source_gate_audit_fields_before_render",
        "test_release_bundle_verifier_redacts_sensitive_source_adapter_gate_audit_fields",
        "secret-token-audit-field",
        "audit_hashes contains unexpected field with sensitive name",
        "test_release_bundle_verifier_accepts_bsc_testnet_crypto_profile",
    }
    sparse_bundle_test = tmp_path / "sccp_release_bundle_crypto_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker not in removed_schema_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    assert any(
        "SCCP release public cryptographic-evidence binding source inventory"
        in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {confusable_audit_suppression_marker}" in error
        for error in errors
    )
    for removed_marker in sorted(
        marker
        for marker in removed_schema_markers
        if marker != confusable_audit_suppression_marker
    ):
        assert any(
            "SCCP release public cryptographic-evidence binding source inventory"
            in error
            and str(sparse_bundle_test) in error
            and f"missing marker: {removed_marker}" in error
            for error in errors
        )

    readiness_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_CRYPTO_EVIDENCE_BINDING_MARKERS
        if path == "pytests/scripts/sccp_release_readiness_report_test.py"
    )
    malformed_audit_container_marker = (
        'assert active_row["source_adapter_gate_audit_hashes"] == ["not", "an", "object"]'
    )
    sparse_readiness_test = tmp_path / "sccp_release_readiness_crypto_test.py"
    sparse_readiness_test.write_text(
        "\n".join(
            marker
            for marker in readiness_test_markers
            if marker != malformed_audit_container_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_crypto_evidence_binding_gate_inventory_errors(
        ((sparse_readiness_test, readiness_test_markers),)
    )

    assert any(
        "SCCP release public cryptographic-evidence binding source inventory"
        in error
        and str(sparse_readiness_test) in error
        and f"missing marker: {malformed_audit_container_marker}" in error
        for error in errors
    )


def test_release_readiness_report_guards_release_public_submission_surface_binding_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin public submission-surface binding."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_public_submission_surface_binding_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _submission_surface_inventory_errors(surfaces):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_public_submission_surface_binding_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS[
                    0
                ][1],
            ),
        )
    )

    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: def _expected_submission_surfaces(report:" in error
        for error in errors
    )
    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and "missing marker: readiness report user_prover_submission_surfaces does not match corridor phases"
        in error
        for error in errors
    )

    status_shape_marker = "validation_status must be passed or blocked"
    blocker_status_coupling_marker = (
        "validation_blockers must be empty when validation_status is passed"
    )
    blocker_marker = "validation_blockers must be empty"
    removed_bundle_markers = {
        status_shape_marker,
        blocker_status_coupling_marker,
        blocker_marker,
    }
    sparse_bundle = tmp_path / "sccp_release_bundle.py"
    sparse_bundle.write_text(
        "\n".join(
            marker
            for marker in verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS[
                2
            ][1]
            if marker not in removed_bundle_markers
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_submission_surface_binding_gate_inventory_errors(
        (
            (
                sparse_bundle,
                verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS[2][1],
            ),
        )
    )

    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {status_shape_marker}" in error
        for error in errors
    )
    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {blocker_status_coupling_marker}" in error
        for error in errors
    )
    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_bundle) in error
        and f"missing marker: {blocker_marker}" in error
        for error in errors
    )

    blocked_test_marker = (
        "test_release_bundle_rejects_blocked_copied_submission_surface_before_render"
    )
    sparse_bundle_test = tmp_path / "sccp_release_bundle_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS[
                3
            ][1]
            if marker != blocked_test_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_submission_surface_binding_gate_inventory_errors(
        (
            (
                sparse_bundle_test,
                verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS[3][1],
            ),
        )
    )

    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {blocked_test_marker}" in error
        for error in errors
    )

    bundle_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_PUBLIC_SUBMISSION_SURFACE_BINDING_MARKERS
        if path == "pytests/scripts/sccp_release_bundle_test.py"
    )
    confusable_sdk_suppression_marker = "assert confusable_sdk not in captured.err"
    sparse_bundle_test = tmp_path / "sccp_release_bundle_submission_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in bundle_test_markers
            if marker != confusable_sdk_suppression_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_public_submission_surface_binding_gate_inventory_errors(
        ((sparse_bundle_test, bundle_test_markers),)
    )

    assert any(
        "SCCP release public submission-surface binding source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {confusable_sdk_suppression_marker}" in error
        for error in errors
    )


def test_release_readiness_report_guards_release_native_prover_bundle_schema_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness coverage must pin native prover bundle schemas."""

    report = load_report_module()
    verifier = load_verify_helpers()
    assert (
        report._sccp_release_native_prover_bundle_schema_gate_inventory_errors()
        == []
    )

    sparse_verifier = tmp_path / "sccp_verify_release_bundle.py"
    sparse_verifier.write_text(
        "def _native_evm_prover_bundle_summary_schema_errors(status):\n",
        encoding="utf-8",
    )
    errors = report._sccp_release_native_prover_bundle_schema_gate_inventory_errors(
        (
            (
                sparse_verifier,
                verifier.SCCP_RELEASE_NATIVE_PROVER_BUNDLE_SCHEMA_MARKERS[0][1],
            ),
        )
    )

    assert any(
        "SCCP release native-prover bundle schema source inventory" in error
        and str(sparse_verifier) in error
        and "missing marker: def _native_evm_prover_bundle_status_from_payload("
        in error
        for error in errors
    )
    assert any(
        "SCCP release native-prover bundle schema source inventory" in error
        and "missing marker: readiness report native_evm_prover_bundle does not match bundled"
        in error
        for error in errors
    )

    blocked_test_marker = (
        "test_release_bundle_rejects_blocked_copied_native_evm_summary_before_render"
    )
    sparse_bundle_test = tmp_path / "sccp_release_bundle_test.py"
    sparse_bundle_test.write_text(
        "\n".join(
            marker
            for marker in verifier.SCCP_RELEASE_NATIVE_PROVER_BUNDLE_SCHEMA_MARKERS[
                3
            ][1]
            if marker != blocked_test_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_native_prover_bundle_schema_gate_inventory_errors(
        (
            (
                sparse_bundle_test,
                verifier.SCCP_RELEASE_NATIVE_PROVER_BUNDLE_SCHEMA_MARKERS[3][1],
            ),
        )
    )

    assert any(
        "SCCP release native-prover bundle schema source inventory" in error
        and str(sparse_bundle_test) in error
        and f"missing marker: {blocked_test_marker}" in error
        for error in errors
    )

    readiness_test_markers = next(
        markers
        for path, markers in verifier.SCCP_RELEASE_NATIVE_PROVER_BUNDLE_SCHEMA_MARKERS
        if path == "pytests/scripts/sccp_release_readiness_report_test.py"
    )
    malformed_blocker_marker = 'assert "o" not in readiness["blockers"]'
    sparse_readiness_test = tmp_path / "sccp_release_readiness_native_test.py"
    sparse_readiness_test.write_text(
        "\n".join(
            marker
            for marker in readiness_test_markers
            if marker != malformed_blocker_marker
        ),
        encoding="utf-8",
    )
    errors = report._sccp_release_native_prover_bundle_schema_gate_inventory_errors(
        ((sparse_readiness_test, readiness_test_markers),)
    )

    assert any(
        "SCCP release native-prover bundle schema source inventory" in error
        and str(sparse_readiness_test) in error
        and f"missing marker: {malformed_blocker_marker}" in error
        for error in errors
    )


def test_release_readiness_report_blocks_missing_sccp_proof_request_source_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when proof-request source gates are missing."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP proof-request bundle/source-proof gate source inventory "
        "sccp_test.py missing marker: sourceProofBytes required for non-SORA source bundle"
    )
    monkeypatch.setattr(
        report,
        "_sccp_proof_request_bundle_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_phase_evidence_source_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when phase evidence source guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP phase evidence duplicate-input source inventory "
        "scripts/sccp_release_bundle.py missing marker: already set by"
    )
    monkeypatch.setattr(
        report,
        "_sccp_phase_evidence_source_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["phase_evidence_source_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_corridor_phase_transcript_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when corridor transcript guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release corridor phase-transcript source inventory "
        "scripts/sccp_release_readiness_report.py missing marker: "
        "def _phase_transcript_block("
    )
    monkeypatch.setattr(
        report,
        "_release_corridor_phase_transcript_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_corridor_phase_transcript_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["phase_evidence_source_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_release_bundle_source_copy_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when release source-copy guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release bundle source-copy source inventory "
        "scripts/sccp_release_bundle.py missing marker: "
        "release bundle source path must not be a symlink"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_bundle_source_copy_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_bundle_source_copy_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["phase_evidence_source_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_release_bundle_output_path_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when release output-path guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release bundle output-path source inventory "
        "scripts/sccp_release_bundle.py missing marker: "
        "release bundle output directory must not be a symlink"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_bundle_output_path_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_bundle_output_path_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_bundle_source_copy_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_release_artifact_path_text_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when artifact path text guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release artifact path text source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "artifact path contains Markdown-unsafe character"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_artifact_path_text_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_artifact_path_text_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_bundle_output_path_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_input_provenance_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when input provenance schemas drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release input-provenance schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "readiness report inputs do not match copied input artifacts"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_input_provenance_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_input_provenance_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_artifact_path_text_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_json_root_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public JSON-root schemas drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public JSON-root schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "object_pairs_hook=_reject_duplicate_json_keys"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_json_root_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_public_json_root_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_input_provenance_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_markdown_text_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public Markdown text schemas drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public Markdown text schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "release-notes attachment is not UTF-8 text"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_markdown_text_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_public_markdown_text_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_public_json_root_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_crypto_evidence_binding_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public crypto binding drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public cryptographic-evidence binding source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "def _cryptographic_evidence_lane_binding_errors("
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_crypto_evidence_binding_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "release_public_crypto_evidence_binding_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_public_markdown_text_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_submission_surface_binding_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public submission binding drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public submission-surface binding source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "def _expected_submission_surfaces(report:"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_submission_surface_binding_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "release_public_submission_surface_binding_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "release_public_crypto_evidence_binding_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_native_prover_bundle_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when native bundle schema coverage drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release native-prover bundle schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "def _native_evm_prover_bundle_status_from_payload("
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_native_prover_bundle_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_native_prover_bundle_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "release_public_submission_surface_binding_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_manifest_readiness_flags_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when release manifest readiness checks drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release manifest readiness-flags source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "manifest production_ready is not true"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_manifest_readiness_flags_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_manifest_readiness_flags_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_artifact_path_text_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_manifest_artifact_set_order_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when manifest artifact-set checks drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release manifest artifact-set/order source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "manifest artifact order does not match canonical"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_manifest_artifact_set_order_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_manifest_artifact_set_order_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_manifest_readiness_flags_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_blocker_list_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public blocker-list schemas drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public blocker-list schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "must not contain duplicate strings"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_blocker_list_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_public_blocker_list_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_manifest_artifact_set_order_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_public_scalar_text_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public scalar-text schemas drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release public scalar-text schema source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "def _cryptographic_evidence_row_schema_errors("
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_public_scalar_text_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_public_scalar_text_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_public_blocker_list_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_release_notes_attachment_invariants_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when release-notes invariants drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP release-notes attachment invariants source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "release notes attachment does not list manifest.json"
    )
    monkeypatch.setattr(
        report,
        "_sccp_release_notes_attachment_invariants_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["release_notes_attachment_invariants_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_manifest_readiness_flags_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_readiness_markdown_invariants_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public Markdown invariants drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP readiness Markdown invariants source inventory "
        "scripts/sccp_verify_release_bundle.py missing marker: "
        "readiness report Markdown missing section"
    )
    monkeypatch.setattr(
        report,
        "_sccp_readiness_markdown_invariants_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["readiness_markdown_invariants_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["release_notes_attachment_invariants_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_retired_network_source_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when retired-network guards are missing."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP retired network-surface guard source inventory "
        "sccp_retired_network_surface_test.py missing marker: "
        "def test_generic_no_support_note_stays_in_launch_scope_files"
    )
    monkeypatch.setattr(
        report,
        "_sccp_retired_network_surface_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_launch_scope_source_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when launch-scope constants are unpinned."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP launch-scope constants source inventory "
        "sccp_release_readiness_report.py missing marker: ACTIVE_LAUNCH_DOMAIN = 1"
    )
    monkeypatch.setattr(
        report,
        "_sccp_launch_scope_constant_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_launch_policy_selector_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when the ETH-only selector guard drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet launch-policy selector source inventory "
        "lib.rs missing marker: EthereumMainnetLane must not open BSC even "
        "when BSC-shaped components are ready"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_launch_policy_selector_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_launch_policy_selector_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_launch_policy_documentation_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when active launch-policy docs drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet launch-policy documentation source inventory "
        "docs/source/bridge_proofs.md contains stale marker: BSC mainnet only "
        "when the configured BSC source-chain finality/inclusion"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_launch_policy_documentation_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_launch_policy_documentation_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_public_discovery_documentation_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when public discovery docs drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP public discovery documentation source inventory "
        "docs/source/bridge_proofs.md missing marker: "
        "the intended verifier target (`EVM`, `Solana`, `TON`, or `TRON`)"
    )
    monkeypatch.setattr(
        report,
        "_sccp_public_discovery_documentation_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"][
        "ethereum_launch_policy_documentation_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_data_collection_no_proxy_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Ethereum data collection can proxy."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet js-sdk data collection source "
        "javascript/iroha_js/src/sccp.js contains forbidden proxy"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_data_collection_no_proxy_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_data_collection_no_proxy_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_source_bridge_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_native_receipt_finality_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when native receipt finality guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet native receipt finality source inventory "
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt "
        "missing marker: beaconFinality.finalizedHeaderRoot is required for receiptProof"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_native_receipt_finality_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_source_bridge_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_beacon_rest_finalized_header_shape_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Beacon REST header guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet Beacon REST finalized-header shape SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        "/signature must be 96 bytes/u"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_beacon_rest_finalized_header_shape_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_beacon_rest_finalized_header_shape_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_inbound_adversarial_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when inbound adversarial guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet inbound adversarial SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        "duplicateReceipt"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_inbound_adversarial_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_inbound_adversarial_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_bsc_inbound_adversarial_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when BSC inbound adversarial guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "BSC mainnet inbound adversarial SDK test inventory "
        "javascript/iroha_js/test/sccpBscMainnet.test.js missing marker: "
        "callbackEvidence.receiptProof.blockHash"
    )
    monkeypatch.setattr(
        report,
        "_bsc_inbound_adversarial_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["bsc_inbound_adversarial_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_tron_inbound_adversarial_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when TRON inbound adversarial guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "TRON mainnet inbound adversarial source inventory "
        "crates/iroha_sccp/src/lib.rs missing marker: "
        "TRON transaction-info receipts must not contain duplicate matching SCCP logs"
    )
    monkeypatch.setattr(
        report,
        "_tron_inbound_adversarial_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["tron_inbound_adversarial_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_bsc_route_config_canonical_manifest_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when BSC route-config guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP BSC route-config canonical-manifest source inventory "
        "scripts/sccp_bsc_taira_xor_deploy.test.mjs missing marker: "
        "bscTokenAddress: BSC_TOKEN_ADDRESS.toUpperCase()"
    )
    monkeypatch.setattr(
        report,
        "_bsc_route_config_canonical_manifest_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["bsc_route_config_canonical_manifest_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["bsc_inbound_adversarial_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_tron_route_config_canonical_manifest_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when TRON route-config guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP TRON route-config canonical-manifest source inventory "
        "scripts/sccp_tron_taira_xor_deploy.test.mjs missing marker: "
        "tronNetwork: \"TRON-MAINNET\""
    )
    monkeypatch.setattr(
        report,
        "_tron_route_config_canonical_manifest_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["tron_route_config_canonical_manifest_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["bsc_route_config_canonical_manifest_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_tron_runtime_route_manifest_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when TRON runtime parser guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP TRON runtime route-manifest source inventory "
        "crates/iroha_config/src/parameters/user.rs missing marker: "
        "fn production_ready_tron_route_requires_offline_full_toml_hash()"
    )
    monkeypatch.setattr(
        report,
        "_tron_runtime_route_manifest_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["tron_runtime_route_manifest_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["tron_route_config_canonical_manifest_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_all_lanes_route_canary_scalar_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when all-lanes scalar guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP all-lanes route-canary scalar source inventory "
        "scripts/sccp_all_lanes_evidence.py missing marker: "
        "route canary status must be a non-empty canonical string"
    )
    monkeypatch.setattr(
        report,
        "_all_lanes_route_canary_scalar_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["all_lanes_route_canary_scalar_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["tron_route_config_canonical_manifest_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_all_lanes_governed_blocker_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when governed blocker schema guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP all-lanes governed blocker schema source inventory "
        "scripts/sccp_all_lanes_evidence.py missing marker: "
        "errors.extend(_blocker_list_errors(record, \"route allowlist\"))"
    )
    monkeypatch.setattr(
        report,
        "_all_lanes_governed_blocker_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["all_lanes_governed_blocker_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["all_lanes_route_canary_scalar_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_all_lanes_evidence_root_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when all-lanes evidence-root guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP all-lanes evidence-root schema source inventory "
        "scripts/sccp_all_lanes_evidence.py missing marker: "
        "evidence bundle root must be an object"
    )
    monkeypatch.setattr(
        report,
        "_all_lanes_evidence_root_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["all_lanes_evidence_root_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["all_lanes_route_canary_scalar_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_all_lanes_release_checklist_exact_boolean_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when all-lanes exact-boolean guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP all-lanes release-checklist exact-boolean source inventory "
        "scripts/sccp_all_lanes_evidence.py missing marker: "
        'return 0 if summary["production_ready"] is True else 1'
    )
    monkeypatch.setattr(
        report,
        "_all_lanes_release_checklist_exact_boolean_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "all_lanes_release_checklist_exact_boolean_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["all_lanes_governed_blocker_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_active_launch_checklist_schema_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when active checklist schema guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP active-launch checklist schema source inventory "
        "scripts/sccp_release_readiness_report.py missing marker: "
        'release_checklist["ready"] is True'
    )
    monkeypatch.setattr(
        report,
        "_active_launch_checklist_schema_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["active_launch_checklist_schema_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["all_lanes_governed_blocker_schema_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_outbound_precallback_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when outbound pre-callback guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet outbound pre-callback SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        "assert.equal(outboundProverCalled, false)"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_outbound_precallback_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_outbound_precallback_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_local_admission_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when local-admission guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet local-admission SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        "metadata is not canonical"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_local_admission_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_local_admission_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_outbound_provider_validation_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when outbound provider guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet outbound provider validation source inventory "
        "python/iroha_torii_client/sccp.py missing marker: "
        "await self.validate_execution_provider_mainnet(provider)"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_outbound_provider_validation_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_outbound_provider_validation_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_local_admission_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_root_zero_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when receipt-root zero guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet receipt-root zero rejection SDK test inventory "
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/"
        "SourceSccpProofHashesTest.kt missing marker: "
        "assertFailsWith<IllegalArgumentException>"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_root_zero_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_receipt_root_zero_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_rlp_zero_topic_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when receipt-RLP zero-topic guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet receipt RLP zero-topic SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        'topics: [hex32("00")]'
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_rlp_zero_topic_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_receipt_rlp_zero_topic_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_rlp_zero_address_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when receipt-RLP zero-address guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet receipt RLP zero-address SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        'address: `0x${"00".repeat(20)}`'
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_rlp_zero_address_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_receipt_rlp_zero_address_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_source_event_context_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source-event context guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet source-event context SDK test inventory "
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py missing marker: "
        'for field in ("transactionHash", "blockHash", "blockNumber")'
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_source_event_context_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_receipt_source_event_context_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_source_event_mode_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source-event mode guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet source-event evidence mode SDK test inventory "
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py missing marker: "
        "test_collect_receipt_proof_allows_explicit_receipt_only_mode"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_source_event_mode_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_receipt_source_event_mode_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_source_event_zero_digest_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source-event digest guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet source-event zero digest SDK test inventory "
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py missing marker: "
        "zero source event digest was accepted"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_source_event_zero_digest_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_receipt_source_event_zero_digest_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_rpc_duplicate_json_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when receipt duplicate-JSON guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet receipt RPC duplicate JSON SDK test inventory "
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py missing marker: "
        "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_rpc_duplicate_json_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_receipt_rpc_duplicate_json_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_receipt_block_transaction_hash_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when block receipt tx-hash guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet block receipt transactionHash SDK test inventory "
        "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py missing marker: "
        'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]'
    )
    monkeypatch.setattr(
        report,
        "_ethereum_receipt_block_transaction_hash_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_receipt_block_transaction_hash_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_js_receipt_admission_guard_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when JS receipt admission guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet JS receipt admission source inventory "
        "javascript/iroha_js/dist/sccp.js missing marker: "
        "Ethereum mainnet receipt proof construction requires beaconFinality."
    )
    monkeypatch.setattr(
        report,
        "_ethereum_js_receipt_admission_guard_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_js_receipt_admission_guard_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_sdk_receipt_metadata_guard_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when SDK receipt metadata guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet SDK receipt metadata source inventory "
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/"
        "SourceSccpProofHashes.kt missing marker: "
        "typed receipt type is not supported for Ethereum mainnet receipt proofs"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_sdk_receipt_metadata_guard_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_sdk_receipt_metadata_guard_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_native_receipt_finality_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_noncanonical_chain_id_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when noncanonical chain-id guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet noncanonical chain id SDK test inventory "
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js missing marker: "
        "canonical JSON-RPC quantity"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_noncanonical_chain_id_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_noncanonical_chain_id_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_beacon_rest_finalized_header_shape_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_beacon_rest_execution_payload_binding_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Beacon REST execution binding drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet Beacon REST execution payload binding SDK test inventory "
        "javascript/iroha_js/src/sccp.js missing marker: "
        "execution payload receipts_root must match block.receiptsRoot"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_beacon_rest_execution_payload_binding_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_beacon_rest_execution_payload_binding_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_beacon_rest_finalized_header_shape_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_sync_committee_roster_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when sync-committee roster guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet sync-committee roster SDK test inventory "
        "javascript/iroha_js/src/sccp.js missing marker: "
        "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_sync_committee_roster_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_sync_committee_roster_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_beacon_rest_execution_payload_binding_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_source_bridge_config_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source-bridge config guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet source-bridge config source inventory "
        "sccp_eth_source_bridge_evidence.py missing marker: "
        "ETH_SOURCE_BRIDGE_CONFIG_PREFIX"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_source_bridge_config_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_source_bridge_config_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_evm_source_adapter_deployment_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_source_material_template_rejection_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source-template rejection guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP source-material template rejection source inventory "
        "sccp_eth_source_bridge_evidence.py missing marker: "
        "template-derived {label} is not deployable"
    )
    monkeypatch.setattr(
        report,
        "_sccp_source_material_template_rejection_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["source_material_template_rejection_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_source_bridge_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_sccp_source_material_role_validation_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when source role-validation guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP source-material role validation source inventory "
        "sccp_eth_source_bridge_evidence.py missing marker: "
        "def _require_source_role_hash_separation("
    )
    monkeypatch.setattr(
        report,
        "_sccp_source_material_role_validation_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["source_material_role_validation_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["source_material_template_rejection_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_evm_source_adapter_deployment_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when active EVM adapter gates drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet EVM source-adapter deployment gate source inventory "
        "lib.rs missing marker: "
        "wrong_config_deployment.source_bridge_config_hash[0] ^= 0x01;"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_evm_source_adapter_deployment_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_evm_source_adapter_deployment_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["native_sccp_no_wasm_readiness_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_contract_smoke_eth_mainnet_network_id_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when ETH contract-smoke chain id drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "EVM contract smoke Ethereum mainnet network id source inventory "
        "sccp_message_bridge_smoke.js missing marker: "
        'callExceptionWithReason("Network id must be ETH mainnet")'
    )
    monkeypatch.setattr(
        report,
        "_contract_smoke_eth_mainnet_network_id_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["contract_smoke_eth_mainnet_network_id_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["contract_smoke_evm_production_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_contract_smoke_evm_production_surface_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when EVM contract-smoke coverage drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "EVM contract smoke production surface source inventory "
        "sccp_message_bridge_smoke.js missing marker: "
        'callExceptionWithReason("Verifier key hash mismatch")'
    )
    monkeypatch.setattr(
        report,
        "_contract_smoke_evm_production_surface_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["contract_smoke_evm_production_surface_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["contract_smoke_eth_mainnet_network_id_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_core_range_finality_binding_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Core range/finality binding drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet SCCP range finality binding source inventory "
        "world.rs missing marker: SCCP message proof range must match finality height"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_core_range_finality_binding_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_core_range_finality_binding_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_evm_source_adapter_deployment_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_core_message_replay_guard_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Core message replay guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet SCCP message replay guard source inventory "
        "world.rs missing marker: SCCP message proof replays existing message proof"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_core_message_replay_guard_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_core_message_replay_guard_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_core_range_finality_binding_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_torii_pinned_message_proof_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when Torii pinned proof serving drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet Torii pinned message proof source inventory "
        "routing.rs missing marker: SCCP message bridge proofs must be pinned "
        "for core replay protection"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_torii_pinned_message_proof_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_torii_pinned_message_proof_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_core_message_replay_guard_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_evm_source_live_production_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when live source evidence guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet live EVM source production SDK test inventory "
        "sccp_evm_source_live_evidence.py missing marker: "
        "deployment receipt block receiptsRoot metadata must be verified"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_evm_source_live_production_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_evm_live_destination_production_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_evm_live_destination_production_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when live destination guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet live EVM destination production SDK test inventory "
        "sccp_evm_live_evidence.py missing marker: "
        "route-canary proofBytes must not be all zero"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_evm_live_destination_production_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_evm_live_destination_production_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_route_canary_finalized_receipt_block_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when route-canary finality guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet route-canary finalized receipt block SDK test inventory "
        "sccp_evm_live_evidence.py missing marker: "
        'receipt_block_finalized=finalized_block["receipt_block_finalized"]'
    )
    monkeypatch.setattr(
        report,
        "_ethereum_route_canary_finalized_receipt_block_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"][
        "ethereum_route_canary_finalized_receipt_block_gate"
    ] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"][
        "ethereum_evm_live_destination_production_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_evm_block_tag_metadata_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_ethereum_evm_block_tag_metadata_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when finalized block-tag guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "Ethereum mainnet EVM block-tag metadata source inventory "
        "sccp_all_lanes_evidence.py missing marker: "
        "Ethereum source live block-tag metadata must be finalized"
    )
    monkeypatch.setattr(
        report,
        "_ethereum_evm_block_tag_metadata_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["ethereum_evm_block_tag_metadata_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["ethereum_evm_source_live_production_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"][
        "ethereum_evm_live_destination_production_gate"
    ] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_native_sccp_no_wasm_readiness_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when native no-WASM source guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "native SCCP no-WASM readiness SDK test inventory "
        "sccp_release_readiness_report.py missing marker: "
        "def _native_evm_prover_forbidden_payload_blockers("
    )
    monkeypatch.setattr(
        report,
        "_native_sccp_no_wasm_readiness_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["native_sccp_no_wasm_readiness_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_unready_transparent_proof_config_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when unready toggle ownership drifts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP unready transparent-proof config-only source inventory "
        "user.rs contains forbidden environment override: "
        "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS"
    )
    monkeypatch.setattr(
        report,
        "_sccp_unready_transparent_proof_config_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["retired_network_surface_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["ethereum_launch_policy_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["public_discovery_documentation_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_report_blocks_missing_tron_deploy_operator_boolean_gate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Production readiness must fail when TRON operator boolean guards drift."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    blocker = (
        "SCCP TRON deploy operator boolean source inventory "
        "sccp_tron_taira_xor_deploy.test.mjs missing marker: "
        "/--force must be true or false/u"
    )
    monkeypatch.setattr(
        report,
        "_tron_deploy_operator_boolean_gate_inventory_errors",
        lambda: [blocker],
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["production_ready"] is False
    assert blocker in readiness["blockers"]
    assert readiness["source_inventory"]["tron_deploy_operator_boolean_gate"] == {
        "validation_status": "blocked",
        "validation_blockers": [blocker],
    }
    assert readiness["source_inventory"]["unready_transparent_proof_config_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["launch_scope_constant_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }
    assert readiness["source_inventory"]["proof_request_bundle_gate"] == {
        "validation_status": "passed",
        "validation_blockers": [],
    }


def test_release_readiness_evidence_phase_accepts_pytest_runner_command_shape() -> None:
    """The transcript parser must accept the production corridor pytest command."""

    report = load_report_module()
    command = "+ python3 -m pytest -q " + " ".join(corridor_evidence_script_tests())

    for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]:
        assert report._phase_command_matches_required_fragment(
            "evidence-scripts",
            command,
            fragment,
        )


def test_release_readiness_phase_command_matchers_accept_corridor_dry_run() -> None:
    """All required phase commands must match the runner's real dry-run output."""

    report = load_report_module()
    completed = subprocess.run(
        [str(CORRIDOR_SCRIPT), "--dry-run"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    for phase, fragments in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.items():
        phase_block = report._phase_transcript_block(phase, completed.stdout)
        assert phase_block is not None, f"missing dry-run phase block: {phase}"
        for fragment in fragments:
            assert report._phase_block_has_command_fragment(
                phase,
                phase_block,
                fragment,
            ), f"{phase} dry-run command did not satisfy {fragment}"


def test_release_readiness_phase_command_matchers_reject_echoed_fragments() -> None:
    """Required phase fragments cannot be satisfied by traced echo commands."""

    report = load_report_module()

    for phase, fragments in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS.items():
        for fragment in fragments:
            command = f"+ echo {shlex.quote(fragment)}"
            assert not report._phase_command_matches_required_fragment(
                phase,
                command,
                fragment,
            ), f"{phase} accepted echoed command fragment: {fragment}"


def test_release_readiness_phase_command_matchers_reject_bare_fragments() -> None:
    """Non-command fragments cannot satisfy phase evidence by themselves."""

    report = load_report_module()
    cases = (
        ("evidence-scripts", "pytests/scripts/sccp_release_bundle_test.py"),
        ("js-sdk", "javascript/iroha_js/test/sccpPackageExports.test.js"),
        ("swift-sdk", "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions"),
        ("kotlin-sdk", "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest"),
        ("java-android", "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"),
        ("dotnet-sdk", "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests"),
        ("contract-smoke", "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js"),
    )

    for phase, fragment in cases:
        assert fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        assert not report._phase_command_matches_required_fragment(
            phase,
            f"+ {fragment}",
            fragment,
        ), f"{phase} accepted bare fragment: {fragment}"


def test_release_readiness_phase_command_matchers_reject_comment_fragments() -> None:
    """Required phase fragments cannot be satisfied inside shell comments."""

    report = load_report_module()
    cases = (
        (
            "rust-sccp",
            "+ cargo test -p iroha_core # cargo test -p iroha_sccp -- --nocapture",
            "cargo test -p iroha_sccp -- --nocapture",
        ),
        (
            "evidence-scripts",
            "+ python3 -m pytest -q "
            "pytests/scripts/check_sccp_production_corridor_test.py "
            "# pytests/scripts/sccp_release_bundle_test.py",
            "pytests/scripts/sccp_release_bundle_test.py",
        ),
        (
            "js-sdk",
            "+ node --test javascript/iroha_js/test/sccpSolanaProver.test.js "
            "# javascript/iroha_js/test/sccpPackageExports.test.js",
            "javascript/iroha_js/test/sccpPackageExports.test.js",
        ),
        (
            "contract-smoke",
            "+ node --test scripts/sccp_taira_xor_contract.test.mjs "
            "# --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
            "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        ),
    )

    for phase, command, fragment in cases:
        assert fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        assert not report._phase_command_matches_required_fragment(
            phase,
            command,
            fragment,
        ), f"{phase} accepted shell-commented command fragment: {fragment}"


def test_release_readiness_phase_command_matchers_reject_inert_option_values() -> None:
    """Required fragments must be selected by the command option that runs them."""

    report = load_report_module()
    cases = (
        (
            "evidence-scripts",
            "+ python3 -m pytest -q "
            "pytests/scripts/check_sccp_production_corridor_test.py "
            "--ignore pytests/scripts/sccp_release_bundle_test.py",
            "pytests/scripts/sccp_release_bundle_test.py",
        ),
        (
            "js-sdk",
            "+ node --test javascript/iroha_js/test/sccpSolanaProver.test.js "
            "--test-reporter javascript/iroha_js/test/sccpPackageExports.test.js",
            "javascript/iroha_js/test/sccpPackageExports.test.js",
        ),
        (
            "swift-sdk",
            "+ swift test --filter SccpSolanaProverTests "
            "--skip ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions "
            "--disable-swift-testing",
            "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions",
        ),
        (
            "swift-sdk",
            "+ swift test --filter OtherTests --skip swift test --filter "
            "SccpSolanaProverTests --disable-swift-testing",
            "swift test --filter SccpSolanaProverTests --disable-swift-testing",
        ),
        (
            "kotlin-sdk",
            "+ ./gradlew :core-jvm:test --console=plain "
            "--tests org.hyperledger.iroha.sdk.sccp.OtherTest "
            "--info org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
            "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
        ),
        (
            "java-android",
            "+ ./gradlew :core:test --console=plain "
            "--tests org.hyperledger.iroha.android.GradleHarnessTests "
            "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
            "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
        ),
        (
            "java-android",
            "+ env ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTestsExtra "
            "./gradlew :core:test --console=plain "
            "--tests org.hyperledger.iroha.android.GradleHarnessTests",
            "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests",
        ),
        (
            "java-android",
            "+ env ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.SourceSccpProofsTestsExtra "
            "./gradlew :core:test --console=plain "
            "--tests org.hyperledger.iroha.android.GradleHarnessTests",
            "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests",
        ),
        (
            "dotnet-sdk",
            "+ dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj "
            "--filter FullyQualifiedName~Other --logger "
            "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
            "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
        ),
        (
            "dotnet-sdk",
            "+ dotnet test tests/Other/Other.csproj "
            "--filter FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests "
            "--logger dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        ),
        (
            "contract-smoke",
            "+ node --test scripts/sccp_taira_xor_contract.test.mjs "
            "--test-reporter scripts/sccp_bsc_taira_xor_deploy.test.mjs",
            "scripts/sccp_bsc_taira_xor_deploy.test.mjs",
        ),
        (
            "contract-smoke",
            '+ node --eval "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js"',
            "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        ),
    )

    for phase, command, fragment in cases:
        assert fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        assert not report._phase_command_matches_required_fragment(
            phase,
            command,
            fragment,
        ), f"{phase} accepted inert option value: {fragment}"


def test_release_readiness_phase_command_matchers_reject_narrow_kotlin_selector() -> None:
    """The Kotlin package-suite selector cannot be replaced by one class."""

    report = load_report_module()
    fragment = "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp."

    assert fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["kotlin-sdk"]
    assert not report._phase_command_matches_required_fragment(
        "kotlin-sdk",
        "+ ./gradlew :core-jvm:test --console=plain "
        "--tests org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
        fragment,
    )
    assert report._phase_command_matches_required_fragment(
        "kotlin-sdk",
        "+ ./gradlew :core-jvm:test --console=plain "
        "--tests org.hyperledger.iroha.sdk.sccp.*",
        fragment,
    )


def test_release_readiness_phase_command_matchers_reject_extra_suffix_arguments() -> None:
    """Exact phase commands must not accept trailing arguments that alter scope."""

    report = load_report_module()
    cases = (
        (
            "evidence-scripts",
            "+ python3 -m pytest -q pytests/scripts/check_sccp_production_corridor_test.py "
            "pytests/scripts/sccp_release_bundle_test.py "
            "--ignore pytests/scripts/check_sccp_production_corridor_test.py",
            "-m pytest -q pytests/scripts/check_sccp_production_corridor_test.py",
        ),
        (
            "evidence-scripts",
            "+ python3 -m pytest -q "
            + " ".join(corridor_evidence_script_tests())
            + " pytests/scripts/unscoped_extra_sccp_test.py",
            "pytests/scripts/sccp_release_bundle_test.py",
        ),
        (
            "python-sdk",
            "+ python3 -m pytest -q python/iroha_torii_client/tests/sccp_test.py "
            "--deselect python/iroha_torii_client/tests/sccp_test.py::test_sccp",
            "-m pytest -q python/iroha_torii_client/tests/sccp_test.py",
        ),
        (
            "python-sdk",
            "+ python3 -m pytest -q python/iroha_torii_client/tests/sccp_test.py "
            "pytests/scripts/sccp_release_bundle_test.py",
            "-m pytest -q python/iroha_torii_client/tests/sccp_test.py",
        ),
        (
            "rust-sccp",
            "+ cargo test -p iroha_sccp -- --nocapture --skip sccp",
            "cargo test -p iroha_sccp -- --nocapture",
        ),
        (
            "core-admission",
            "+ cargo test -p iroha_core --test iroha_core_group_01 "
            "bridge_proofs:: -- --nocapture --skip bridge_proofs::",
            "cargo test -p iroha_core --test iroha_core_group_01 bridge_proofs:: -- --nocapture",
        ),
        (
            "contract-smoke",
            "+ bash scripts/sccp_evm_contract_smoke.sh --dry-run",
            "bash scripts/sccp_evm_contract_smoke.sh",
        ),
        (
            "kotlin-sdk",
            "+ java -version --dry-run",
            "java -version",
        ),
        (
            "java-android",
            "+ java -version --dry-run",
            "java -version",
        ),
        (
            "kotlin-sdk",
            "+ ./gradlew :core-jvm:test --console=plain "
            "--tests org.hyperledger.iroha.sdk.sccp.* "
            "--tests org.hyperledger.iroha.sdk.sccp.TonSccpProverTest --dry-run",
            "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp.",
        ),
        (
            "kotlin-sdk",
            "+ ./gradlew :core-jvm:test --console=plain "
            "--tests org.hyperledger.iroha.sdk.sccp.* "
            "--tests org.hyperledger.iroha.sdk.sccp.TonSccpProverTest --exclude-task test",
            "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
        ),
        (
            "java-android",
            "+ ./gradlew :core:test --console=plain "
            "--tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests --dry-run",
            "./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests",
        ),
        (
            "swift-sdk",
            "+ swift test --filter SccpSolanaProverTests "
            "--disable-swift-testing --skip SccpSolanaProverTests",
            "swift test --filter SccpSolanaProverTests --disable-swift-testing",
        ),
        (
            "js-sdk",
            "+ node --test javascript/iroha_js/test/sccpSolanaProver.test.js "
            "javascript/iroha_js/test/sccpEthereumMainnet.test.js "
            "javascript/iroha_js/test/sccpBscMainnet.test.js "
            "javascript/iroha_js/test/package_dist.test.js "
            "javascript/iroha_js/test/sccpPackageExports.test.js "
            "javascript/iroha_js/test/unscopedExtra.test.js",
            "javascript/iroha_js/test/sccpPackageExports.test.js",
        ),
        (
            "js-sdk",
            "+ node --test javascript/iroha_js/test/sccpSolanaProver.test.js "
            "javascript/iroha_js/test/sccpEthereumMainnet.test.js "
            "javascript/iroha_js/test/sccpBscMainnet.test.js "
            "javascript/iroha_js/test/package_dist.test.js "
            "javascript/iroha_js/test/sccpPackageExports.test.js || true",
            "javascript/iroha_js/test/sccpPackageExports.test.js",
        ),
        (
            "contract-smoke",
            "+ node --test scripts/sccp_bsc_taira_xor_deploy.test.mjs "
            "scripts/sccp_tron_taira_xor_deploy.test.mjs "
            "scripts/sccp_taira_xor_contract.test.mjs scripts/unscoped_extra.test.mjs",
            "scripts/sccp_taira_xor_contract.test.mjs",
        ),
        (
            "contract-smoke",
            "+ node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js "
            "--trace-warnings",
            "--check contracts/evm/sccp/test/sccp_message_bridge_smoke.js",
        ),
        (
            "dotnet-sdk",
            "+ dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj "
            "--filter FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests "
            "--nologo --logger trx",
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
        ),
        (
            "dotnet-sdk",
            "+ dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj "
            "--filter FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests "
            "--nologo || true",
            "FullyQualifiedName~SccpEthereumMainnetTests\\|FullyQualifiedName~SccpBscMainnetTests",
        ),
    )

    for phase, command, fragment in cases:
        assert fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        assert not report._phase_command_matches_required_fragment(
            phase,
            command,
            fragment,
        ), f"{phase} accepted extra suffix argument: {fragment}"


def test_release_readiness_phase_command_matchers_reject_short_circuited_fragments() -> None:
    """Required phase fragments cannot sit behind a failing shell prefix."""

    report = load_report_module()
    cases = (
        (
            "rust-sccp",
            "+ false && env CARGO_TARGET_DIR=target/sccp-production-corridor "
            "NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p iroha_sccp -- --nocapture",
            "cargo test -p iroha_sccp -- --nocapture",
        ),
        (
            "evidence-scripts",
            "+ false && python3 -m pytest -q "
            + " ".join(corridor_evidence_script_tests()),
            "pytests/scripts/sccp_release_readiness_report_test.py",
        ),
        (
            "js-sdk",
            "+ false && node --test javascript/iroha_js/test/sccpSolanaProver.test.js",
            "javascript/iroha_js/test/sccpSolanaProver.test.js",
        ),
        (
            "contract-smoke",
            "+ false && bash scripts/sccp_evm_contract_smoke.sh",
            "bash scripts/sccp_evm_contract_smoke.sh",
        ),
    )

    for phase, command, fragment in cases:
        assert not report._phase_command_matches_required_fragment(
            phase,
            command,
            fragment,
        ), f"{phase} accepted short-circuited command fragment: {fragment}"


def test_release_readiness_java_android_phase_requires_source_proof_harness() -> None:
    """Android readiness evidence must prove source-proof hardening ran."""

    report = load_report_module()
    source_harness = "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"
    ton_harness = "org.hyperledger.iroha.android.sccp.TonSccpProverTests"

    assert source_harness in corridor_android_harness_mains()
    assert ton_harness in corridor_android_harness_mains()
    assert source_harness in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "java-android"
    ]
    assert ton_harness in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "java-android"
    ]


def test_release_readiness_kotlin_phase_requires_ton_prover_test() -> None:
    """Kotlin readiness evidence must prove the TON proof-request tests ran."""

    report = load_report_module()
    ton_test = "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest"

    assert ton_test in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["kotlin-sdk"]
    assert report._phase_command_matches_required_fragment(
        "kotlin-sdk",
        "+ ./gradlew :core-jvm:test --console=plain "
        "--tests org.hyperledger.iroha.sdk.sccp.* "
        "--tests org.hyperledger.iroha.sdk.sccp.TonSccpProverTest",
        ton_test,
    )
    assert not report._phase_command_matches_required_fragment(
        "kotlin-sdk",
        "+ ./gradlew :core-jvm:test --console=plain "
        "--tests org.hyperledger.iroha.sdk.sccp.*",
        ton_test,
    )


def test_release_readiness_report_requires_evm_evidence_script_transcript(
    tmp_path: Path,
) -> None:
    """The report must reject evidence phase logs missing EVM evidence tests."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_evm_live_evidence_test.py"
    assert omitted_fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
            "evidence-scripts"
        ]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "evidence-scripts-without-evm-live.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is "
        f"missing expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def write_active_launch_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write only the active launch-lane evidence records."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    report = load_report_module()
    active_domain = report.ACTIVE_LAUNCH_DOMAIN
    records = helpers.complete_bundle(evidence_module)
    for section, domain_key in {
        "sccp_source_verifier_materials": "source_domain",
        "sccp_destination_rollouts": "domain",
        "sccp_route_allowlists": "domain",
    }.items():
        records[section] = [
            record
            for record in records[section]
            if record.get(domain_key) == active_domain
        ]
    evm_chain_id = active_evm_live_chain_id(report)
    if evm_chain_id is not None:
        for record in records["sccp_source_verifier_materials"]:
            record["_comment_evm_source_rpc_chain_id"] = evm_chain_id
            record["_comment_evm_source_block_tag"] = "finalized"
        for record in records["sccp_destination_rollouts"]:
            record["_comment_evm_rpc_chain_id"] = evm_chain_id
            record["_comment_evm_block_tag"] = "finalized"
    evidence = tmp_path / f"{report.ACTIVE_LAUNCH_CHAIN}-launch.toml"
    evidence_payload = helpers.render_records(records)
    evidence.write_text(evidence_payload, encoding="utf-8")
    return evidence, evidence_payload


def sdk_source_text(sdk: str) -> str:
    """Return the SDK source text that must expose readiness helper symbols."""

    if sdk == "js-sdk":
        paths = [
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            ROOT / "javascript" / "iroha_js" / "src" / "index.js",
        ]
    elif sdk == "python-sdk":
        paths = [ROOT / "python" / "iroha_torii_client" / "sccp.py"]
    elif sdk == "swift-sdk":
        source_root = ROOT / "IrohaSwift" / "Sources" / "IrohaSwift"
        paths = sorted(source_root.glob("Sccp*.swift")) + [
            source_root / "ToriiClient.swift"
        ]
    elif sdk == "kotlin-sdk":
        source_root = (
            ROOT
            / "kotlin"
            / "core-jvm"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "sdk"
        )
        paths = sorted((source_root / "sccp").glob("*.kt")) + [
            source_root / "client" / "BridgeProofSubmitRequest.kt"
        ]
    elif sdk == "java-android":
        source_root = (
            ROOT
            / "java"
            / "iroha_android"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "android"
        )
        paths = sorted((source_root / "sccp").glob("*.java")) + [
            source_root / "client" / "BridgeProofSubmitRequest.java"
        ]
    elif sdk == "dotnet-sdk":
        paths = sorted((ROOT / "csharp" / "src" / "Hyperledger.Iroha.Sdk" / "Sccp").glob("*.cs"))
    else:
        raise AssertionError(f"unhandled SCCP SDK phase: {sdk}")
    return "\n".join(path.read_text(encoding="utf-8") for path in paths)


def sdk_symbol_tokens(symbol: str) -> tuple[str, ...]:
    """Return source tokens that must be present for a readiness helper symbol."""

    if ".init(" in symbol:
        owner, _, rest = symbol.partition(".init(")
        return owner, rest.rstrip(")").rstrip(":")
    if "." in symbol:
        owner, member = symbol.rsplit(".", 1)
        return owner, member
    return (symbol,)


def sdk_symbol_export_tokens(symbol: str) -> tuple[str, ...]:
    """Return package-root tokens needed to expose a readiness helper symbol."""

    if ".init(" in symbol:
        owner, _, _ = symbol.partition(".init(")
        return (owner,)
    if "." in symbol:
        owner, _ = symbol.rsplit(".", 1)
        return (owner,)
    return (symbol,)


def sdk_source_paths(path_or_paths) -> tuple[Path, ...]:
    """Normalize a single SDK source path or split source path tuple."""

    return path_or_paths if isinstance(path_or_paths, tuple) else (path_or_paths,)


def source_display_path(path: Path) -> Path:
    """Return a stable path label for verifier diagnostics."""

    try:
        return path.relative_to(ROOT)
    except ValueError:
        return path


def native_local_prover_source_violations(
    source_paths,
    chain_label: str,
    *,
    source_overrides: dict[Path, str] | None = None,
) -> list[str]:
    """Collect SDK source violations for forbidden remote-prover dependencies."""

    source_overrides = source_overrides or {}
    violations: list[str] = []
    for sdk, path_or_paths in source_paths.items():
        for path in sdk_source_paths(path_or_paths):
            if path in source_overrides:
                source = source_overrides[path]
            elif path.is_file():
                source = path.read_text(encoding="utf-8")
            else:
                violations.append(
                    f"{sdk} missing {chain_label} source file: {source_display_path(path)}"
                )
                continue
            for label, pattern in BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS.items():
                if pattern.search(source):
                    violations.append(
                        f"{sdk} {source_display_path(path)} contains forbidden {label}"
                    )

    return violations


def helper_matches_hook_marker(sdk: str, helper: str, marker: str) -> bool:
    """Return whether a helper symbol satisfies a UI-owned hook marker."""

    if sdk == "python-sdk":
        return helper == marker
    return marker in helper


def test_release_readiness_sdk_helper_symbols_exist_in_sdk_sources() -> None:
    """Readiness helper maps must name SDK symbols that exist in source."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    surfaces = report._submission_surfaces(passed_phases)
    sources = {
        sdk: sdk_source_text(sdk)
        for surface in surfaces
        for sdk in surface["sdk_helper_symbols_by_sdk"]
    }
    missing: list[str] = []

    for surface in surfaces:
        for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items():
            source = sources[sdk]
            for symbol in symbols:
                absent_tokens = [
                    token for token in sdk_symbol_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} {sdk} {symbol}: {absent_tokens}"
                    )

    assert missing == []


def test_release_readiness_bsc_sdk_sources_are_native_local_prover_only() -> None:
    """BSC SDK facades must stay native/local-prover owned, with no WASM fallback."""

    assert native_local_prover_source_violations(
        BSC_MAINNET_SDK_SOURCE_PATHS,
        "BSC",
    ) == []


def test_release_readiness_ethereum_sdk_sources_are_native_local_prover_only() -> None:
    """Ethereum SDK facades must stay native/local-prover owned, with no WASM fallback."""

    assert native_local_prover_source_violations(
        ETHEREUM_MAINNET_SDK_SOURCE_PATHS,
        "Ethereum",
    ) == []


def test_release_readiness_native_source_scan_rejects_forbidden_prover_markers() -> None:
    """The SDK source scan must fail closed for every remote-prover marker."""

    sample_path = BSC_MAINNET_SDK_SOURCE_PATHS["js-sdk"]
    assert isinstance(sample_path, Path)
    for label, sample in FORBIDDEN_PROVER_DEPENDENCY_SAMPLES.items():
        violations = native_local_prover_source_violations(
            {"js-sdk": sample_path},
            "BSC",
            source_overrides={sample_path: f"export const sdk = true;\n{sample}\n"},
        )
        assert any(f"forbidden {label}" in violation for violation in violations)


def test_release_readiness_native_source_scan_checks_split_dotnet_sources() -> None:
    """Split .NET BSC facade files must all be scanned, not only the first path."""

    dotnet_paths = BSC_MAINNET_SDK_SOURCE_PATHS["dotnet-sdk"]
    assert isinstance(dotnet_paths, tuple)
    first_path, second_path = dotnet_paths
    violations = native_local_prover_source_violations(
        {"dotnet-sdk": dotnet_paths},
        "BSC",
        source_overrides={
            first_path: "namespace Hyperledger.Iroha.Sccp {}",
            second_path: "namespace Hyperledger.Iroha.Sccp { const string proverEndpoint = \"x\"; }",
        },
    )

    assert violations == [
        f"dotnet-sdk {source_display_path(second_path)} contains forbidden proverEndpoint"
    ]


def test_release_readiness_ethereum_data_collection_has_no_proxy_fallback() -> None:
    """Ethereum evidence collection must use app-owned providers."""

    violations: list[str] = []
    for sdk, region_config in ETHEREUM_DATA_COLLECTION_REGIONS.items():
        path, start_marker, end_marker, required_markers = region_config
        if not path.is_file():
            violations.append(
                f"{sdk} missing Ethereum data-collection source file: "
                f"{path.relative_to(ROOT)}"
            )
            continue
        region = source_region(path, start_marker, end_marker)
        for marker in required_markers:
            if marker not in region:
                violations.append(
                    f"{sdk} {path.relative_to(ROOT)} missing provider marker {marker}"
                )
        for label, pattern in ETHEREUM_DATA_COLLECTION_FORBIDDEN_PATTERNS.items():
            if pattern.search(region):
                violations.append(
                    f"{sdk} {path.relative_to(ROOT)} collection path contains forbidden {label}"
                )

    assert violations == []


def test_release_readiness_ethereum_js_dist_keeps_receipt_admission_guards() -> None:
    """Published JS must keep source receipt-proof admission checks in dist."""

    required_markers = (
        "eth_getBlockReceipts target receipt must match transactionHash",
        "eth_getBlockReceipts target receipt blockHash must match receipt",
        "eth_getBlockReceipts target receipt blockNumber must match receipt",
        "eth_getBlockReceipts target receipt RLP must match receipt",
        "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        "const receiptTransactionHash = requireEthereumRpcHexData(",
        'const blockHash = requireEthereumRpcHexData(block.hash, "block.hash", 32);',
        "const executionBlockHash = nonZeroHex32Bytes(",
        "const executionReceiptsRoot = nonZeroHex32Bytes(",
        "const beaconFinalizedRoot = nonZeroHex32Bytes(",
        "const syncCommitteeRoot = nonZeroHex32Bytes(",
        "await prove(immutableProverCallbackValue(evidence), options)",
    )
    checked_paths = (
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
    )
    violations: list[str] = []
    for path in checked_paths:
        source = path.read_text(encoding="utf-8")
        for marker in required_markers:
            if marker not in source:
                violations.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_ethereum_sdks_keep_receipt_metadata_guards() -> None:
    """Ethereum SDK receipt-proof builders must reject block-receipt metadata drift."""

    sdk_markers = {
        "js-src": ((
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            (
                "eth_getBlockReceipts target receipt must match transactionHash",
                "eth_getBlockReceipts target receipt blockHash must match receipt",
                "eth_getBlockReceipts target receipt blockNumber must match receipt",
                "eth_getBlockReceipts target receipt RLP must match receipt",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
        ),),
        "js-dist": ((
            ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
            (
                "eth_getBlockReceipts target receipt must match transactionHash",
                "eth_getBlockReceipts target receipt blockHash must match receipt",
                "eth_getBlockReceipts target receipt blockNumber must match receipt",
                "eth_getBlockReceipts target receipt RLP must match receipt",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
        ),),
        "swift-sdk": (
            (
                ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
                (
                    '"blockReceipts.transactionHash"',
                    '"blockReceipts.blockHash"',
                    '"blockReceipts.blockNumber"',
                    '"blockReceipts.receiptRlp"',
                    "canonicalEvmReceiptRlp(currentReceipt)",
                ),
            ),
            (
                ROOT
                / "IrohaSwift"
                / "Sources"
                / "IrohaSwift"
                / "SccpSourceProofHashes.swift",
                (
                    "receiptType <= 0x7f",
                    "let admittedType = UInt8(receiptType)",
                    "(0x01...0x04).contains(admittedType)",
                ),
            ),
        ),
        "kotlin-sdk": (
            (
                ROOT
                / "kotlin"
                / "core-jvm"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "sdk"
                / "sccp"
                / "EvmSccpProver.kt",
                (
                    "eth_getBlockReceipts target receipt must match transactionHash",
                    "eth_getBlockReceipts target receipt blockHash must match receipt",
                    "eth_getBlockReceipts target receipt blockNumber must match receipt",
                    "eth_getBlockReceipts target receipt RLP must match receipt",
                    "SccpSourceProofs.canonicalEvmReceiptRlp(receipt)",
                ),
            ),
            (
                ROOT
                / "kotlin"
                / "core-jvm"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "sdk"
                / "sccp"
                / "SourceSccpProofHashes.kt",
                (
                    "typed receipt type must fit one byte below 0x80",
                    "val admittedType = receiptType.toInt()",
                    "typed receipt type is not supported for Ethereum mainnet receipt proofs",
                ),
            ),
        ),
        "java-android": (
            (
                ROOT
                / "java"
                / "iroha_android"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "android"
                / "sccp"
                / "EthereumMainnetSccp.java",
                (
                    "eth_getBlockReceipts target receipt must match transactionHash",
                    "eth_getBlockReceipts target receipt blockHash must match receipt",
                    "eth_getBlockReceipts target receipt blockNumber must match receipt",
                    "eth_getBlockReceipts target receipt RLP must match receipt",
                    "SourceSccpProofs.canonicalEvmReceiptRlp(receipt)",
                ),
            ),
            (
                ROOT
                / "java"
                / "iroha_android"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "android"
                / "sccp"
                / "SourceSccpProofs.java",
                (
                    "typed receipt type must fit one byte below 0x80",
                    "final int admittedType = receiptType.intValue()",
                    "typed receipt type is not supported for Ethereum mainnet receipt proofs",
                ),
            ),
        ),
        "dotnet-sdk": ((
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "EthereumMainnetSccp.cs",
            (
                "blockReceipts.transactionHash must match transactionHash.",
                "blockReceipts.blockHash must match receipt.",
                "blockReceipts.blockNumber must match receipt.",
                "blockReceipts.receiptRlp must match receipt.",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs.",
            ),
        ),),
    }

    violations: list[str] = []
    for sdk, guarded_files in sdk_markers.items():
        for path, markers in guarded_files:
            source = path.read_text(encoding="utf-8")
            for marker in markers:
                if marker not in source:
                    violations.append(f"{sdk} {path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_ethereum_native_sdks_keep_receipt_finality_guards() -> None:
    """Native SDK receipt-proof builders must require Beacon finality roots."""

    native_markers = {
        "swift-sdk": (
            (
                ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
                (
                    "guard let beaconSlotInput = try Self.strictFirstPresent(",
                    "guard let finalizedRootInput = try Self.strictFirstPresent(",
                    "guard let syncCommitteeRootInput = try Self.strictFirstPresent(",
                ),
            ),
            (
                ROOT
                / "IrohaSwift"
                / "Tests"
                / "IrohaSwiftTests"
                / "SccpSolanaProverTests.swift",
                (
                    "for (missingField, label) in [",
                    ".invalidPublicInputs(label)",
                    '("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot")',
                    '("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot")',
                    '("beaconSlot", "beaconFinality.beaconSlot")',
                ),
            ),
        ),
        "kotlin-sdk": (
            (
                ROOT
                / "kotlin"
                / "core-jvm"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "sdk"
                / "sccp"
                / "EvmSccpProver.kt",
                (
                    "beaconFinality.beaconSlot is required for receiptProof",
                    "beaconFinality.finalizedHeaderRoot is required for receiptProof",
                    "beaconFinality.syncCommitteeRoot is required for receiptProof",
                ),
            ),
            (
                ROOT
                / "kotlin"
                / "core-jvm"
                / "src"
                / "test"
                / "kotlin"
                / "org"
                / "hyperledger"
                / "iroha"
                / "sdk"
                / "sccp"
                / "EvmSccpProverTest.kt",
                (
                    "for ((field, label) in listOf(",
                    "beaconFinality = beaconFinality - field",
                    '"finalizedHeaderRoot" to "beaconFinality.finalizedHeaderRoot"',
                    '"syncCommitteeRoot" to "beaconFinality.syncCommitteeRoot"',
                    '"beaconSlot" to "beaconFinality.beaconSlot"',
                ),
            ),
        ),
        "java-android": (
            (
                ROOT
                / "java"
                / "iroha_android"
                / "src"
                / "main"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "android"
                / "sccp"
                / "EthereumMainnetSccp.java",
                (
                    "beaconFinality.beaconSlot is required for receiptProof",
                    "beaconFinality.finalizedHeaderRoot is required for receiptProof",
                    "beaconFinality.syncCommitteeRoot is required for receiptProof",
                ),
            ),
            (
                ROOT
                / "java"
                / "iroha_android"
                / "src"
                / "test"
                / "java"
                / "org"
                / "hyperledger"
                / "iroha"
                / "android"
                / "sccp"
                / "EvmSccpProverTests.java",
                (
                    "for (final String[] missingFinalityCase :",
                    "collection must reject missing",
                    '{"finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot"}',
                    '{"syncCommitteeRoot", "beaconFinality.syncCommitteeRoot"}',
                    '{"beaconSlot", "beaconFinality.beaconSlot"}',
                ),
            ),
        ),
        "dotnet-sdk": (
            (
                ROOT
                / "csharp"
                / "src"
                / "Hyperledger.Iroha.Sdk"
                / "Sccp"
                / "EthereumMainnetSccp.cs",
                (
                    "BeaconSlot = NormalizeUnsignedInteger(",
                    "BeaconFinalizedRoot = NormalizeRpcHex(",
                    "SyncCommitteeRoot = NormalizeRpcHex(",
                ),
            ),
            (
                ROOT
                / "csharp"
                / "tests"
                / "Hyperledger.Iroha.Sdk.Tests"
                / "SccpEthereumMainnetTests.cs",
                (
                    "foreach (var (missingField, label) in new[]",
                    "incompleteFinality.Remove(missingField);",
                    '("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot")',
                    '("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot")',
                    '("beaconSlot", "beaconFinality.beaconSlot")',
                ),
            ),
        ),
    }

    violations: list[str] = []
    for sdk, guarded_files in native_markers.items():
        for path, markers in guarded_files:
            source = path.read_text(encoding="utf-8")
            for marker in markers:
                if marker not in source:
                    violations.append(f"{sdk} {path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_ethereum_sdks_validate_provider_before_outbound_submitter() -> None:
    """Ethereum outbound submitter paths must honor configured mainnet providers."""

    sdk_markers = {
        "js-src": (
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            (
                "let providerValidated = false;",
                "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
                "if (typeof submit === \"function\")",
            ),
        ),
        "js-dist": (
            ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
            (
                "let providerValidated = false;",
                "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
                "if (typeof submit === \"function\")",
            ),
        ),
        "python-sdk": (
            ROOT / "python" / "iroha_torii_client" / "sccp.py",
            (
                'provider = options.get("execution_provider", self.execution_provider)',
                "await self.validate_execution_provider_mainnet(provider)",
                "return await _maybe_await(submitter(dict(submission), options))",
            ),
        ),
        "swift-sdk": (
            ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
            (
                "if let executionProvider {",
                "_ = try await validateExecutionProviderMainnet(executionProvider)",
                "return try await outboundSubmitFunction(submission)",
            ),
        ),
        "kotlin-sdk": (
            ROOT
            / "kotlin"
            / "core-jvm"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "sdk"
            / "sccp"
            / "EvmSccpProver.kt",
            (
                "executionProvider?.let { validateExecutionProviderMainnet(it) }",
                "return submitter.submit(buildEthereumCalldata(input))",
            ),
        ),
        "java-android": (
            ROOT
            / "java"
            / "iroha_android"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "android"
            / "sccp"
            / "EthereumMainnetSccp.java",
            (
                "if (executionProvider != null) {",
                "validateExecutionProviderMainnet(executionProvider);",
                "return outboundSubmitter.submit(buildEthereumCalldata(input));",
            ),
        ),
        "dotnet-sdk": (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "EthereumMainnetSccp.cs",
            (
                "IEthereumMainnetExecutionProvider? executionProvider",
                "ValidateExecutionProviderMainnetAsync(",
                "return await outboundSubmitter.SubmitAsync(submission, cancellationToken)",
            ),
        ),
    }

    violations: list[str] = []
    for sdk, (path, markers) in sdk_markers.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                violations.append(f"{sdk} {path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_evm_evidence_keeps_block_tag_metadata_guards() -> None:
    """Ethereum production evidence must keep finalized block-tag tripwires."""

    guarded_files = {
        ROOT / "scripts" / "sccp_evm_source_live_evidence.py": (
            'sccp_evm_source_block_tag = "',
            "--block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
            'sccp_evm_block_tag = "',
            "--block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_eth_source_bridge_evidence.py": (
            'sccp_evm_source_block_tag = "',
            "Ethereum source TOML requires --block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_evm_destination_evidence.py": (
            'sccp_evm_block_tag = "',
            "Ethereum destination TOML requires --block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_bsc_source_bridge_evidence.py": (
            'sccp_evm_source_block_tag = "',
            '"latest"',
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            '"sccp_evm_source_rpc_chain_id": "_comment_evm_source_rpc_chain_id"',
            '"sccp_evm_source_block_tag": "_comment_evm_source_block_tag"',
            '"sccp_evm_rpc_chain_id": "_comment_evm_rpc_chain_id"',
            '"sccp_evm_block_tag": "_comment_evm_block_tag"',
            "EVM source live RPC chain-id must be canonical for {profile.chain}",
            "EVM live RPC chain-id must be canonical for {profile.chain}",
            "Ethereum source live block-tag metadata must be finalized",
            "Ethereum destination live block-tag metadata must be finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_source_live_evidence_test.py": (
            "test_evm_source_live_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_source_block_tag = "finalized"',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
            "test_live_evm_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_block_tag = "finalized"',
        ),
        ROOT / "pytests" / "scripts" / "sccp_eth_source_bridge_evidence_test.py": (
            "test_eth_source_toml_rejects_nonfinalized_block_tag",
            "Ethereum source TOML requires --block-tag finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_destination_evidence_test.py": (
            "test_evm_destination_eth_toml_rejects_nonfinalized_block_tag",
            "Ethereum destination TOML requires --block-tag finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py": (
            "test_all_lanes_rejects_ethereum_nonfinalized_evm_live_metadata",
            '# sccp_evm_source_block_tag = "finalized"',
            '# sccp_evm_block_tag = "finalized"',
        ),
    }

    violations: list[str] = []
    for path, markers in guarded_files.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                violations.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_report_guards_ethereum_evm_block_tag_metadata_gate_inventory(
    tmp_path: Path,
) -> None:
    """Readiness must delegate finalized block-tag marker checks to the verifier."""

    report = load_report_module()
    assert report._ethereum_evm_block_tag_metadata_gate_inventory_errors() == []

    sparse_script = tmp_path / "sccp_all_lanes_evidence.py"
    sparse_script.write_text(
        '"sccp_evm_source_block_tag": "_comment_evm_source_block_tag"\n',
        encoding="utf-8",
    )

    errors = report._ethereum_evm_block_tag_metadata_gate_inventory_errors(
        (
            (
                sparse_script,
                (
                    '"sccp_evm_source_block_tag": "_comment_evm_source_block_tag"',
                    "Ethereum source live block-tag metadata must be finalized",
                ),
            ),
        )
    )

    assert any(
        "Ethereum mainnet EVM block-tag metadata source inventory" in error
        and "missing marker: Ethereum source live block-tag metadata must be finalized"
        in error
        for error in errors
    )


def test_release_readiness_guards_evm_source_live_production_surface() -> None:
    """Ethereum source evidence must keep live production deployment guards."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_source_live_evidence.py": (
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
            "JSON-RPC returned duplicate JSON keys",
            "JSON-RPC {method} failed with HTTP {exc.code}",
            "JSON-RPC {method} request failed",
            "JSON-RPC {method} returned error response",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_source_live_evidence_test.py": (
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
            "test_evm_source_json_rpc_redacts_transport_and_error_response_details",
            "duplicate JSON keys",
        ),
    }

    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_launch_policy_selector() -> None:
    """Ethereum launch readiness must keep the ETH-only selector regression."""

    lib_rs = ROOT / "crates" / "iroha_sccp" / "src" / "lib.rs"
    markers = (
        "fn sccp_lane_production_ready_under_launch_policy_v1(",
        "SccpLaunchModeV1::EthereumMainnetLane => domain == SCCP_DOMAIN_ETH",
        "fn ethereum_launch_policy_opens_only_eth_lane_independently_of_all_lanes()",
        "EthereumMainnetLane must let production-ready ETH open before all lanes are ready",
        "EthereumMainnetLane must not open BSC even when BSC-shaped components are ready",
        "EthereumMainnetLane must still fail closed when ETH evidence is incomplete",
        "AllLanesAtOnce must continue to wait for every advertised lane",
        "BscMainnetLane must not open ETH",
    )

    source = lib_rs.read_text(encoding="utf-8")
    missing = [
        f"{lib_rs.relative_to(ROOT)} missing `{marker}`"
        for marker in markers
        if marker not in source
    ]

    assert missing == []


def test_release_readiness_guards_evm_live_destination_production_surface() -> None:
    """Ethereum destination evidence must keep live production binding guards."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
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
            "def _require_route_canary_groth16_bn254_proof_tuple(",
            "BN254 base-field element",
            "BN254 G1 point",
            "BN254 G2 point",
            "usedMessageProofs(bytes32) is false",
            'and transaction.get("message_proof_used") is True',
            "JSON-RPC returned duplicate JSON keys",
            "JSON-RPC {method} failed with HTTP {exc.code}",
            "JSON-RPC {method} request failed",
            "JSON-RPC {method} returned error response",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
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
            "proofBytes.a.x must be a BN254 base-field element",
            "proofBytes.a must be a BN254 G1 point",
            "proofBytes.b must be a BN254 G2 point",
            "proofBytes.c must be a BN254 G1 point",
            "usedMessageProofs(bytes32) is false",
            "test_evm_json_rpc_redacts_transport_and_error_response_details",
            "duplicate JSON keys",
        ),
    }

    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_evm_route_canary_finalized_receipt_block() -> None:
    """Ethereum route canaries must bind receipt blocks to finalized execution heads."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
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
        ROOT / "scripts" / "sccp_evm_destination_evidence.py": (
            'EVM_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:evm-route-canary-evidence:v4"',
            "receipt_block_finalized: bool",
            "receipt_block_finalized must be a boolean for EVM route canaries",
            'receipt_block_finalized=values["receipt_block_finalized"]',
            "route_canary_receipt_block_finalized",
            "--route-canary-receipt-block-finalized",
            "from finalized live reads",
            "evm_route_canary_receipt_block_finalized",
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            "evm_route_canary_receipt_block_finalized",
            "_comment_evm_route_canary_receipt_block_finalized",
            "EVM route canary receipt block finalized metadata must be true",
            "receipt_block_finalized=receipt_block_finalized",
            'canary["receipt_block_finalized"] = True',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
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
        ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py": (
            "test_all_lanes_rejects_evm_route_canary_missing_finalized_receipt_state",
            "_comment_evm_route_canary_receipt_block_finalized",
            "receipt_block_finalized=True",
            "receipt block finalized metadata must be true",
        ),
        ROOT / "crates" / "iroha_sccp" / "src" / "lib.rs": (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            'b"iroha:sccp:evm-route-canary-evidence:v4"',
            "push_u8(&mut out, u8::from(receipt_block_finalized));",
            "|| !receipt_block_finalized",
            "allowlist.evm_route_canary_receipt_block_finalized = Some(true);",
            "non-finalized diagnostic EVM route canary hash",
            "evm_route_canary_evidence_hash_matches_destination_script_vector",
            "84b93b0050b6bc9696ba55d56a8c957171e6a4ebd2f242b683762d52d88db9d7",
        ),
        ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs": (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            "evm_route_canary_receipt_block_finalized: self.evm_route_canary_receipt_block_finalized",
        ),
        ROOT / "crates" / "iroha_core" / "src" / "smartcontracts" / "isi" / "world.rs": (
            "evm_route_canary_receipt_block_finalized: configured",
            "configured_sccp_all_lanes_launch_rejects_evm_non_finalized_route_canary",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_all_public_sccp_sdk_sources_are_native_local_prover_only(
) -> None:
    """All public SCCP SDK artifacts must stay native/local-prover owned."""

    violations: list[str] = []
    for sdk, paths in native_local_prover_source_paths().items():
        for path in paths:
            source = path.read_text(encoding="utf-8")
            for label, pattern in BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS.items():
                if pattern.search(source):
                    violations.append(
                        f"{sdk} {path.relative_to(ROOT)} contains forbidden {label}"
                    )

    assert violations == []


def test_release_readiness_native_evm_prover_bundle_manifest_parsers_are_sdk_owned(
) -> None:
    """Primary SDKs must parse signed native prover bundle manifests locally."""

    missing: list[str] = []
    for sdk, paths in NATIVE_EVM_PROVER_BUNDLE_PARSER_MARKERS.items():
        for path, markers in paths.items():
            if not path.is_file():
                missing.append(f"{sdk} missing parser source file: {path.relative_to(ROOT)}")
                continue
            source = path.read_text(encoding="utf-8")
            for marker in markers:
                if marker not in source:
                    missing.append(
                        f"{sdk} {path.relative_to(ROOT)} missing native bundle parser marker `{marker}`"
                    )

    assert missing == []


def test_release_readiness_native_evm_prover_artifact_verifiers_are_sdk_owned(
) -> None:
    """Primary SDKs must verify native prover artifact bytes against bundle hashes locally."""

    missing: list[str] = []
    for sdk, paths in NATIVE_EVM_PROVER_ARTIFACT_VERIFIER_MARKERS.items():
        for path, markers in paths.items():
            if not path.is_file():
                missing.append(f"{sdk} missing artifact verifier source file: {path.relative_to(ROOT)}")
                continue
            source = path.read_text(encoding="utf-8")
            for marker in markers:
                if marker not in source:
                    missing.append(
                        f"{sdk} {path.relative_to(ROOT)} missing native artifact verifier marker `{marker}`"
                    )

    assert missing == []


def test_release_readiness_native_local_prover_guard_covers_identifier_variants() -> None:
    """The native/local-prover guard must catch common remote-prover spellings."""

    samples = {
        "WebAssembly": "const engine = WebAssembly.compile(bytes)",
        "wasm": "import './prover.wasm'",
        "snarkjs": "import snarkjs from 'snarkjs'",
        "remoteProver": "const remoteProver = endpoint",
        "remote prover": "fall back to a remote prover",
        "remote_prover": "remote_prover = 'https://example.invalid'",
        "remote-prover": "remote-prover endpoint",
        "proverUrl": "const proverUrl = config.prover",
        "proverURL": "const proverURL = config.prover",
        "prover_url": "prover_url = config.prover",
        "proverEndpoint": "const proverEndpoint = config.prover",
        "prover_endpoint": "prover_endpoint = config.prover",
    }

    missing = [
        label
        for label, sample in samples.items()
        if not BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS[label].search(sample)
    ]

    assert missing == []


def test_release_readiness_sdk_helper_symbols_are_unique() -> None:
    """Public user-prover helper rows must not hide missing hooks behind duplicates."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    duplicates: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        helper_symbols = surface["sdk_helper_symbols"]
        if len(helper_symbols) != len(set(helper_symbols)):
            duplicates.append(f"{surface['lanes']} default helper list")
        for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items():
            if len(symbols) != len(set(symbols)):
                duplicates.append(f"{surface['lanes']} {sdk}")

    assert duplicates == []


def test_release_readiness_js_helper_symbols_exist_in_portal_artifacts() -> None:
    """Web portal helper maps must exist in JS source, dist, and declarations."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    implementation_artifacts = {
        "src/sccp.js": ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
        "dist/sccp.js": ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
        "index.d.ts": ROOT / "javascript" / "iroha_js" / "index.d.ts",
    }
    package_entry_artifacts = {
        "src/index.js": ROOT / "javascript" / "iroha_js" / "src" / "index.js",
        "dist/index.js": ROOT / "javascript" / "iroha_js" / "dist" / "index.js",
    }
    implementation_artifact_text = {
        label: path.read_text(encoding="utf-8")
        for label, path in implementation_artifacts.items()
    }
    package_entry_artifact_text = {
        label: path.read_text(encoding="utf-8")
        for label, path in package_entry_artifacts.items()
    }
    missing: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        for symbol in surface["sdk_helper_symbols_by_sdk"]["js-sdk"]:
            for artifact, source in implementation_artifact_text.items():
                absent_tokens = [
                    token for token in sdk_symbol_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} js-sdk {symbol} missing from {artifact}: {absent_tokens}"
                    )
            if symbol in JS_CALLBACK_HOOK_SYMBOLS:
                continue
            for artifact, source in package_entry_artifact_text.items():
                absent_tokens = [
                    token for token in sdk_symbol_export_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} js-sdk {symbol} missing from {artifact}: {absent_tokens}"
                    )

    assert missing == []


def test_release_readiness_user_prover_surfaces_name_ui_hook_symbols() -> None:
    """Every public user-prover row must include the app-owned prover hooks."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    required_hook_markers = {
        "js-sdk": ("witnessProvider", "proveFn"),
        "python-sdk": ("witness_provider", "prove"),
        "swift-sdk": ("WitnessProvider", "ProveFunction"),
        "kotlin-sdk": ("WitnessProvider", "ProofEngine"),
        "java-android": ("WitnessProvider", "ProofEngine"),
        "dotnet-sdk": ("InboundProver", "InboundSubmitter"),
    }
    missing: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        for sdk, markers in required_hook_markers.items():
            symbols = surface["sdk_helper_symbols_by_sdk"].get(sdk)
            if symbols is None:
                continue
            for marker in markers:
                if not any(
                    helper_matches_hook_marker(sdk, symbol, marker)
                    for symbol in symbols
                ):
                    missing.append(f"{surface['lanes']} {sdk} missing {marker}")

    assert missing == []


def test_release_readiness_python_helper_symbols_are_package_root_exports() -> None:
    """Python app code must be able to import public SCCP helpers from the package root."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (*report.USER_PROVER_SDK_PHASES, "contract-smoke", "core-admission")
    }
    required_exports = sorted(
        {
            export
            for surface in report._submission_surfaces(passed_phases)
            for symbol in surface["sdk_helper_symbols_by_sdk"]["python-sdk"]
            for export in sdk_symbol_export_tokens(symbol)
            if symbol not in PYTHON_CALLBACK_HOOK_SYMBOLS
        }
    )

    original_path = sys.path[:]
    sys.path.insert(0, str(ROOT / "python"))
    try:
        package = importlib.import_module("iroha_torii_client")
    finally:
        sys.path[:] = original_path

    package_exports = set(getattr(package, "__all__", ()))
    missing_attrs = [
        symbol for symbol in required_exports if not hasattr(package, symbol)
    ]
    missing_all = [symbol for symbol in required_exports if symbol not in package_exports]

    assert missing_attrs == []
    assert missing_all == []


def test_release_readiness_report_blocks_without_evidence_or_corridor_results(
    tmp_path: Path,
) -> None:
    """A public readiness note must not pass without evidence and corridor proof."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        ["python3", str(SCRIPT), str(evidence)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "# SCCP Release Readiness Report" in completed.stdout
    assert "Status: NOT READY" in completed.stdout
    assert "| Path | Bytes | SHA-256 |" in completed.stdout
    assert hashlib.sha256(b"").hexdigest() in completed.stdout
    assert "## Release Checklist" in completed.stdout
    assert "## User Prover Submission Surfaces" in completed.stdout
    assert "`ton` | `ton-contract-v1`" in completed.stdout
    assert "buildTonSccpSubmission" in completed.stdout
    assert "`python-sdk`: `build_ton_sccp_proof_request`" in completed.stdout
    assert "`swift-sdk`: `buildTonSccpProofRequest`" in completed.stdout
    assert "ToriiBridgeProofSubmitRequest.init(evmSccpSubmission:)" in completed.stdout
    assert "TON internal message body BOC" in completed.stdout
    assert (
        "`js-sdk`, `python-sdk`, `swift-sdk`, `kotlin-sdk`, `java-android`"
        in completed.stdout
    )
    assert "blocked: js-sdk is missing<br>python-sdk is missing" in completed.stdout
    assert "`live_route_canary_evidence` | blocked" in completed.stdout
    assert "missing source verifier material" in completed.stdout
    assert "`contract-smoke` | missing" in completed.stdout
    assert "`core-admission`" in completed.stdout
    assert "packaged `dist`, and TypeScript declaration exports" in completed.stdout
    assert "source-adapter gate hash/audit replay rejection" in completed.stdout


def test_release_readiness_json_tracks_corridor_phase_results(tmp_path: Path) -> None:
    """JSON output must separate evidence blockers from validation corridor status."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = load_report_module()
    payload = json.loads(completed.stdout)
    assert payload["production_ready"] is False
    assert payload["corridor"]["production_ready"] is True
    assert payload["corridor"]["phases"]["contract-smoke"] == "passed"
    assert payload["corridor"]["evidence_artifacts"] == {}
    assert payload["corridor"]["require_phase_evidence"] is False
    assert payload["input_artifacts"] == [
        {
            "path": str(evidence),
            "bytes": 0,
            "sha256": hashlib.sha256(b"").hexdigest(),
        }
    ]
    assert "cryptographic_evidence" in payload
    assert payload["evidence"]["production_ready"] is False
    assert payload["release_checklist"]["ready"] is False
    assert any(
        item["id"] == "all_required_lane_records"
        for item in payload["release_checklist"]["items"]
    )
    surfaces = {
        surface["lanes"]: surface
        for surface in payload["user_prover_submission_surfaces"]
    }
    assert set(surfaces) == {"eth,bsc", "tron", "sol", "ton"}
    assert "ton" in surfaces
    assert surfaces["sol"]["proof_backend"] == "sccp-solana-recursive-mainnet-v1"
    assert surfaces["ton"]["proof_backend"] == "ton-contract-v1"
    assert surfaces["eth,bsc"]["proof_backend"] == "evm-groth16-bn254-v1"
    assert surfaces["tron"]["proof_backend"] == "tron-groth16-bn254-v1"
    assert "canonicalEvmSccpReceiptProofBytes" in surfaces["eth,bsc"]["sdk_helpers"]
    assert "canonicalBscSccpReceiptProofBytes" in surfaces["eth,bsc"]["sdk_helpers"]
    assert surfaces["eth,bsc"]["sdk_helper_symbols"] == list(
        report.EVM_JS_USER_PROVER_HELPERS
    )
    assert surfaces["eth,bsc"]["sdk_helpers"] == ", ".join(
        surfaces["eth,bsc"]["sdk_helper_symbols"]
    )
    assert set(surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]) == set(EVM_SDK_PHASES)
    assert (
        surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["js-sdk"]
        == surfaces["eth,bsc"]["sdk_helper_symbols"]
    )
    assert (
        "EthereumMainnetSccp.runNativeProverSelfTest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["js-sdk"]
    )
    assert (
        "runEthereumMainnetNativeProverSelfTest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["js-sdk"]
    )
    assert (
        "build_evm_sccp_proof_request"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.build_ethereum_calldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submit_outbound_to_ethereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "BscMainnetSccp.build_bsc_calldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "BscMainnetSccp.submit_outbound_to_bsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.collect_inbound_evidence_from_receipt"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.prove_inbound_to_sora"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "consensus_provider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "ToriiBridgeProofSubmitRequest.init(evmSccpSubmission:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EvmSccpProver.ProveFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.init(beaconFinalityEvidence:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.runNativeProverSelfTest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    assert (
        "EthereumMainnetSccp.OutboundSubmitFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.OutboundSubmitFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.init(parliaFinalityEvidence:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "SccpEvm.buildProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EvmSccpProofEngine"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.withBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.runNativeProverSelfTest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    assert (
        "EthereumMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.withParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EvmSccpProver.buildProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EvmSccpProver.ProofEngine"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.ConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.BeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.ReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "InboundEvidence.withBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.runNativeProverSelfTest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    assert (
        "EthereumMainnetSccp.OutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.OutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.ConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.ParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "InboundEvidence.withParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.BuildOutboundProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.ProveOutboundToEthereumAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.BuildEthereumCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.SubmitOutboundToEthereumAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.RunNativeProverSelfTestAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetOutboundProofRequestInput"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccpSubmission"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.WithBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.WithParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.BuildOutboundProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.ProveOutboundToBscAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.BuildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.SubmitOutboundToBscAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetOutboundProofRequestInput"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccpSubmission"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetInboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetOutboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetOutboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert "canonicalTronSccpReceiptStateProofBytes" in surfaces["tron"]["sdk_helpers"]
    assert (
        "canonicalTronSccpTransactionSourceProofBytes"
        in surfaces["tron"]["sdk_helpers"]
    )
    assert "TronSccpProver" in surfaces["tron"]["sdk_helper_symbols"]
    assert "witnessProvider" in surfaces["tron"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["tron"]["sdk_helper_symbols"]
    assert (
        "build_tron_sccp_bridge_proof_submit_payload"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "ToriiBridgeProofSubmitRequest.init(tronSccpSubmission:)"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "SccpSourceProofs.tronTransactionSourceProofHash"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "TronSccpProofEngine"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert "buildTonSccpSubmission" in surfaces["ton"]["sdk_helpers"]
    assert "TonSccpSourceStateProver" in surfaces["ton"]["sdk_helper_symbols"]
    assert (
        "build_ton_shard_state_proof_request"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SccpTon.buildShardStateProofRequest"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "buildSolanaSccpAccountsLtHashProofRequest"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert (
        "buildSolanaSccpFullLightClientAuditProofRequests"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert "SolanaSccpSourceStateProver" in surfaces["sol"]["sdk_helpers"]
    assert "SolanaSccpProver" in surfaces["sol"]["sdk_helper_symbols"]
    assert "witnessProvider" in surfaces["sol"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["sol"]["sdk_helper_symbols"]
    assert (
        "build_solana_sccp_accounts_lt_hash_proof_request"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SccpSolana.buildFullLightClientAuditProofRequests"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "SolanaSccpFullLightClientAuditProofEngine"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "SolanaSccpProver.SourceStateProver"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert "TON internal message body BOC" in surfaces["ton"]["on_chain_submission"]
    assert "buildTonShardStateProofRequest" in surfaces["ton"]["sdk_helpers"]
    assert (
        "buildTonSccpFullLightClientAuditProofRequests"
        in surfaces["ton"]["sdk_helpers"]
    )
    assert (
        "buildTonSccpValidatorSetTransitionProofRequest"
        in surfaces["ton"]["sdk_helpers"]
    )
    assert (
        "build_ton_sccp_masterchain_config_proof_request"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "TonSccpProver.buildShardAccountsDictionaryProofRequest"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "TonSccpProver.FullLightClientAuditProofEngine"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert "TonSccpSourceStateProver" in surfaces["ton"]["sdk_helpers"]
    assert "witnessProvider" in surfaces["ton"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["ton"]["sdk_helper_symbols"]
    assert (
        "buildSolanaSccpBankForkChoiceProofRequest"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert (
        "build_solana_sccp_full_accountsdb_lattice_proof_request"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SolanaSccpProver.buildTowerReplayProofRequest"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "SolanaSccpProver.FullLightClientAuditProofEngine"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert surfaces["ton"]["required_phases"] == [
        "js-sdk",
        "python-sdk",
        "swift-sdk",
        "kotlin-sdk",
        "java-android",
        "core-admission",
    ]
    assert surfaces["ton"]["validation_status"] == "passed"
    assert surfaces["ton"]["validation_blockers"] == []
    assert "eth,bsc" in surfaces
    assert (
        "buildEvmSccpBridgeProofSubmitPayload"
        in surfaces["eth,bsc"]["sdk_helpers"]
    )
    assert "dotnet-sdk" in surfaces["eth,bsc"]["required_phases"]
    assert "contract-smoke" in surfaces["eth,bsc"]["required_phases"]
    assert "core-admission" in surfaces["eth,bsc"]["required_phases"]
    assert any("missing source verifier material" in item for item in payload["blockers"])


def test_release_readiness_user_prover_surfaces_require_core_admission(
    tmp_path: Path,
) -> None:
    """User-side prover surfaces are blocked until on-chain admission is tested."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--phase-result",
            "core-admission=missing",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    assert payload["corridor"]["production_ready"] is False
    assert "core-admission is missing" in payload["corridor"]["blockers"]
    for surface in payload["user_prover_submission_surfaces"]:
        assert "core-admission" in surface["required_phases"]
        assert surface["validation_status"] == "blocked"
        assert "core-admission is missing" in surface["validation_blockers"]


def test_release_readiness_report_strict_phase_evidence_blocks_missing_artifacts(
    tmp_path: Path,
) -> None:
    """Strict release notes require hashed proof for every passed corridor phase."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert "| `rust-sccp` | passed | - | - |" in completed.stdout
    assert (
        "production corridor phase rust-sccp has no hashed evidence artifact"
        in completed.stdout
    )
    assert "`governed_deployment_evidence` | ready" in completed.stdout
    assert "`live_route_canary_evidence` | ready" in completed.stdout


def test_release_readiness_report_passes_for_complete_evidence_and_corridor(
    tmp_path: Path,
) -> None:
    """A complete all-lanes bundle plus passing corridor phases produces releasable notes."""

    evidence, evidence_payload = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "sccp-corridor.log"
    corridor_payload = complete_corridor_log()
    corridor_log.write_text(corridor_payload, encoding="utf-8")

    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_log}",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    assert "Status: READY" in completed.stdout
    assert (
        hashlib.sha256(evidence_payload.encode("utf-8")).hexdigest()
        in completed.stdout
    )
    assert (
        hashlib.sha256(corridor_payload.encode("utf-8")).hexdigest()
        in completed.stdout
    )
    assert f"| `rust-sccp` | passed | `{corridor_log}` |" in completed.stdout
    assert "## Release Checklist" in completed.stdout
    assert "## Cryptographic Evidence" in completed.stdout
    assert "## Native Prover Bundle" in completed.stdout
    assert "native-evm-prover-bundle.json" in completed.stdout
    assert "EVM Source Chain ID | EVM Source Tag | EVM Destination Chain ID" in (
        completed.stdout
    )
    assert "`eth` | `1` | `finalized` | `1` | `finalized`" in completed.stdout
    assert "`bsc` | `56` | `latest` | `56` | `latest`" in completed.stdout
    assert "Source Material | Source Deployment | Destination Binding" in (
        completed.stdout
    )
    assert "Source Gate | Source Gate Audits | Route Allowlist" in completed.stdout
    assert "Canary Tx | Canary Receipt Block | Canary Receipt Hash" in completed.stdout
    assert "Canary Receipt Finalized | Canary Receipts Root" in completed.stdout
    assert "Canary Receipts Root | Canary Message ID | Canary Block" in (
        completed.stdout
    )
    assert "`evm_message_proof_accepted_transaction`" in completed.stdout
    assert "`tron_message_proof_accepted_transaction`" in completed.stdout
    assert "`10144`" in completed.stdout
    assert "`1700144`" in completed.stdout
    assert "`solana_live_programdata_snapshot`" in completed.stdout
    assert "`ton_live_account_snapshot`" in completed.stdout
    assert "## User Prover Submission Surfaces" in completed.stdout
    assert "| `eth,bsc` | `evm-groth16-bn254-v1`" in completed.stdout
    assert "| `tron` | `tron-groth16-bn254-v1`" in completed.stdout
    assert "| `sol` | `sccp-solana-recursive-mainnet-v1`" in completed.stdout
    assert "| `ton` | `ton-contract-v1`" in completed.stdout
    assert " | passed |" in completed.stdout
    assert "`governed_deployment_evidence` | ready" in completed.stdout
    assert "`live_route_canary_evidence` | ready" in completed.stdout
    assert "## Blocking Items\n\n- None" in completed.stdout


def test_release_readiness_report_passes_with_only_active_launch_lane(
    tmp_path: Path,
) -> None:
    """Active launch readiness must not require future lanes to be complete."""

    report = load_report_module()
    active_domain = report.ACTIVE_LAUNCH_DOMAIN
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    payload = json.loads(completed.stdout)
    assert payload["production_ready"] is True
    assert payload["release_checklist"]["ready"] is True
    assert payload["native_evm_prover_bundle"]["validation_status"] == "passed"
    assert payload["evidence"]["production_ready"] is False
    assert all(
        f"domain {active_domain}" not in blocker for blocker in payload["blockers"]
    )
    active_crypto = next(
        row
        for row in payload["cryptographic_evidence"]
        if row["domain"] == active_domain
    )
    expected_chain_id = active_evm_live_chain_id(report)
    assert active_crypto["domain"] == active_domain
    assert active_crypto["evm_source_rpc_chain_id"] == expected_chain_id
    assert active_crypto["evm_source_block_tag"] == "finalized"
    assert active_crypto["evm_destination_rpc_chain_id"] == expected_chain_id
    assert active_crypto["evm_destination_block_tag"] == "finalized"
    assert isinstance(active_crypto["route_canary_transaction_hash"], str)
    assert active_crypto["route_canary_transaction_hash"].startswith("0x")
    assert type(active_crypto["route_canary_receipt_block_number"]) is int
    assert active_crypto["route_canary_receipt_block_number"] > 0
    assert isinstance(active_crypto["route_canary_receipt_block_hash"], str)
    assert active_crypto["route_canary_receipt_block_hash"].startswith("0x")
    assert active_crypto["route_canary_receipt_block_finalized"] is True
    assert isinstance(active_crypto["route_canary_block_receipts_root"], str)
    assert active_crypto["route_canary_block_receipts_root"].startswith("0x")
    assert isinstance(active_crypto["route_canary_message_id"], str)
    assert active_crypto["route_canary_message_id"].startswith("0x")
    blocked_future_lanes = [
        lane
        for lane in payload["evidence"]["lanes"]
        if lane["domain"] != active_domain and not lane["production_ready"]
    ]
    assert blocked_future_lanes
    future_crypto_rows = [
        row
        for row in payload["cryptographic_evidence"]
        if row["domain"] != active_domain
    ]
    assert future_crypto_rows
    assert all(
        row["route_canary_evidence_bound"] is False for row in future_crypto_rows
    )


def test_release_readiness_report_compares_checklist_ready_exactly(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Top-level readiness must not truthy-coerce checklist readiness."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)

    def malformed_launch_checklist(
        evidence_summary,
        native_prover_bundle=None,
    ):
        return {"ready": "true", "items": []}

    monkeypatch.setattr(
        report,
        "_active_launch_release_checklist",
        malformed_launch_checklist,
    )

    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    assert readiness["release_checklist"]["ready"] == "true"
    assert readiness["production_ready"] is False


def test_release_readiness_report_markdown_compares_row_ready_exactly(
    tmp_path: Path,
) -> None:
    """Markdown readiness rows must not truthy-coerce malformed flags."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    checklist_item = readiness["release_checklist"]["items"][0]
    checklist_item["ready"] = "true"
    checklist_item["blockers"] = []
    active_lane = next(
        lane
        for lane in readiness["evidence"]["lanes"]
        if lane["domain"] == report.ACTIVE_LAUNCH_DOMAIN
    )
    active_lane["production_ready"] = "true"
    active_lane["blockers"] = []
    active_lane["records"]["source_verifier_material"] = "true"
    active_crypto = next(
        row
        for row in readiness["cryptographic_evidence"]
        if row["domain"] == report.ACTIVE_LAUNCH_DOMAIN
    )
    active_crypto["route_canary_evidence_bound"] = "false"
    readiness["native_evm_prover_bundle"]["required"] = "true"

    markdown = report._render_markdown(readiness, max_blockers_per_lane=4)

    assert f"| `{checklist_item['id']}` | blocked | -" in markdown
    assert (
        f"| {report.ACTIVE_LAUNCH_DOMAIN} | `{report.ACTIVE_LAUNCH_CHAIN}` | "
        "blocked |"
    ) in markdown
    assert (
        "source=no, deploy=yes, dest=yes, route=yes"
        in markdown
    )
    assert (
        f"`{report.ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE} (unbound)`"
        in markdown
    )
    assert "| no | passed |" in markdown
    assert "| yes | passed |" not in markdown


def test_release_readiness_report_markdown_marks_malformed_blocker_containers(
    tmp_path: Path,
) -> None:
    """Markdown blocker cells must not flatten strings or crash on bad entries."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )
    readiness["native_evm_prover_bundle"]["validation_blockers"] = (
        "operator override"
    )
    readiness["source_inventory"]["proof_request_bundle_gate"][
        "validation_blockers"
    ] = ["", 1]
    readiness["user_prover_submission_surfaces"][0]["validation_status"] = "blocked"
    readiness["user_prover_submission_surfaces"][0]["validation_blockers"] = ""
    readiness["blockers"] = "operator override"

    markdown = report._render_markdown(readiness, max_blockers_per_lane=4)

    assert markdown.count("`<invalid validation_blockers>`") >= 3
    assert "- `<invalid blockers>`" in markdown
    assert "o<br>p<br>e<br>r<br>a<br>t<br>o<br>r" not in markdown
    assert "- o\n- p\n- e\n- r" not in markdown


def test_release_readiness_report_markdown_marks_hostile_public_blocker_strings(
    tmp_path: Path,
) -> None:
    """Markdown blocker cells must classify hostile public blocker strings."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )
    control_blocker = "operator\npublic-blocker"
    markdown_blocker = "operator|public-blocker"
    confusable_blocker = "operator public blоcker"
    sensitive_blocker = "operator secret-token-public-blocker"
    checklist_item = readiness["release_checklist"]["items"][0]
    checklist_item["blockers"] = [confusable_blocker]
    readiness["corridor"]["blockers"] = [markdown_blocker]
    readiness["source_inventory"]["proof_request_bundle_gate"][
        "validation_blockers"
    ] = [sensitive_blocker]
    active_lane = next(
        lane
        for lane in readiness["evidence"]["lanes"]
        if lane["domain"] == report.ACTIVE_LAUNCH_DOMAIN
    )
    active_lane["blockers"] = [markdown_blocker]
    readiness["blockers"] = [control_blocker]

    markdown = report._render_markdown(readiness, max_blockers_per_lane=4)

    assert f"| `{checklist_item['id']}` | ready | `<invalid blockers>` |" in markdown
    assert "`<invalid validation_blockers>`" in markdown
    assert "- `<invalid blockers>`" in markdown
    assert markdown.count("`<invalid blockers>`") >= 3
    assert "secret-token-public-blocker" not in markdown
    assert "operator|public-blocker" not in markdown
    assert "operator public blоcker" not in markdown
    assert "operator\npublic-blocker" not in markdown


def test_release_readiness_report_markdown_names_native_sdk_id_evidence(
    tmp_path: Path,
) -> None:
    """Release notes must name canonical native SDK-id evidence explicitly."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    markdown = report._render_markdown(readiness, max_blockers_per_lane=4)

    assert "canonical native EVM prover SDK-id rejection" in markdown
    assert "padded-SDK adversarial tests" in markdown


def test_release_readiness_report_markdown_names_unsupported_scope_note(
    tmp_path: Path,
) -> None:
    """Release notes must publish the explicit no-support launch-scope note."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    readiness = report._build_report(
        [evidence],
        ["all=passed"],
        [],
        require_phase_evidence=False,
        native_evm_prover_bundle=native_bundle,
    )

    markdown = report._render_markdown(readiness, max_blockers_per_lane=4)

    assert report.SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE in markdown


def test_release_readiness_report_preserves_malformed_crypto_evidence_values(
    tmp_path: Path,
) -> None:
    """Cryptographic evidence rows must not truthy-coerce malformed values."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    evidence_summary = report._load_evidence_summary([evidence])
    active_lane = report._active_launch_lane(evidence_summary)
    assert active_lane is not None

    active_lane["route_allowlist"]["route_canary"]["evidence_bound"] = "false"
    active_lane["source_adapter_gate"]["required"] = "false"
    active_lane["source_adapter_gate"]["gate_hash"] = 0
    active_lane["source_adapter_gate"]["audit_hashes"] = ["not", "an", "object"]

    active_row = next(
        row
        for row in report._cryptographic_evidence(evidence_summary)
        if row["domain"] == report.ACTIVE_LAUNCH_DOMAIN
    )

    assert active_row["route_canary_evidence_bound"] == "false"
    assert active_row["source_adapter_gate_required"] == "false"
    assert active_row["source_adapter_gate_hash"] == 0
    assert active_row["source_adapter_gate_audit_hashes"] == ["not", "an", "object"]


def test_release_readiness_hex_predicates_redact_typeerror_parser_causes(
    monkeypatch,
) -> None:
    """Release readiness hex checks must fail closed on parser TypeErrors."""

    report = load_report_module()

    class SecretBytes:
        @staticmethod
        def fromhex(_text):
            raise TypeError("secret-token readiness hex TypeError detail")

    monkeypatch.setattr(report, "bytes", SecretBytes, raising=False)

    assert report._is_nonzero_hex32(fixed_hex32(0x31)) is False
    assert report._is_hex32(fixed_hex32(0x32)) is False

    label = f"domain {report.ACTIVE_LAUNCH_DOMAIN} ({report.ACTIVE_LAUNCH_CHAIN})"
    canary = {
        "evidence_hash": fixed_hex32(0x33),
        "evidence_source": report.ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE,
        "transaction_hash": fixed_hex32(0x34),
        "receipt_block_hash": fixed_hex32(0x35),
        "block_receipts_root": fixed_hex32(0x36),
        "message_id": fixed_hex32(0x37),
        "receipt_block_number": 1,
        "receipt_block_finalized": True,
    }
    blockers = report._active_launch_route_canary_metadata_blockers(label, canary)
    rendered = "\n".join(blockers)

    assert (
        "route canary evidence hash must be a canonical non-zero bytes32 hex string"
        in rendered
    )
    assert "secret-token" not in rendered
    assert "TypeError" not in rendered


def test_release_readiness_report_blocks_malformed_active_route_canary_metadata(
    tmp_path: Path,
) -> None:
    """The release checklist must validate active route-canary transaction fields."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    receipt_block_number_exactness_cases = (
        (
            "receipt_block_number",
            "10144",
            "route canary receipt block number must be a positive integer",
        ),
        (
            "receipt_block_number",
            "0x1",
            "route canary receipt block number must be a positive integer",
        ),
        (
            "receipt_block_number",
            "+1",
            "route canary receipt block number must be a positive integer",
        ),
        (
            "receipt_block_number",
            "\uff11",
            "route canary receipt block number must be a positive integer",
        ),
        (
            "receipt_block_number",
            True,
            "route canary receipt block number must be a positive integer",
        ),
        (
            "receipt_block_number",
            False,
            "route canary receipt block number must be a positive integer",
        ),
    )
    route_canary_evidence_bound_exactness_cases = (
        (
            "evidence_bound",
            "true",
            "route canary evidence is not bound",
        ),
        (
            "evidence_bound",
            1,
            "route canary evidence is not bound",
        ),
        (
            "evidence_bound",
            False,
            "route canary evidence is not bound",
        ),
        (
            "evidence_bound",
            None,
            "route canary evidence is not bound",
        ),
    )
    route_canary_receipt_finalized_exactness_cases = (
        (
            "receipt_block_finalized",
            False,
            "route canary receipt block must be finalized",
        ),
        (
            "receipt_block_finalized",
            "true",
            "route canary receipt block must be finalized",
        ),
        (
            "receipt_block_finalized",
            1,
            "route canary receipt block must be finalized",
        ),
        (
            "receipt_block_finalized",
            None,
            "route canary receipt block must be finalized",
        ),
    )
    route_canary_status_exactness_cases = (
        (
            "status",
            None,
            "route canary status is not passed",
        ),
        (
            "status",
            "",
            "route canary status is not passed",
        ),
        (
            "status",
            " passed ",
            "route canary status is not passed",
        ),
        (
            "status",
            "passed ",
            "route canary status is not passed",
        ),
        (
            "status",
            1,
            "route canary status is not passed",
        ),
    )
    route_canary_evidence_source_exactness_cases = (
        (
            "evidence_source",
            "operator_note",
            "route canary evidence source must be evm_message_proof_accepted_transaction",
        ),
        (
            "evidence_source",
            report.ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE.upper(),
            "route canary evidence source must be evm_message_proof_accepted_transaction",
        ),
        (
            "evidence_source",
            None,
            "route canary evidence source must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            "",
            "route canary evidence source must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            f" {report.ACTIVE_LAUNCH_ROUTE_CANARY_EVIDENCE_SOURCE} ",
            "route canary evidence source must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            123,
            "route canary evidence source must be a non-empty canonical string",
        ),
    )
    route_canary_hex32_exactness_cases = (
        (
            "evidence_hash",
            None,
            "route canary evidence hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "evidence_hash",
            fixed_hex32(0x30).upper(),
            "route canary evidence hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "transaction_hash",
            "0x" + "00" * 32,
            "route canary transaction hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "receipt_block_hash",
            fixed_hex32(0x32).upper(),
            "route canary receipt block hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "block_receipts_root",
            None,
            "route canary block receipts root must be a canonical non-zero bytes32 hex string",
        ),
        (
            "message_id",
            1,
            "route canary message id must be a canonical non-zero bytes32 hex string",
        ),
    )
    cases = (
        (
            "evidence_hash",
            "0x" + "00" * 32,
            "route canary evidence hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "transaction_hash",
            None,
            "route canary transaction hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "transaction_hash",
            fixed_hex32(0x31).upper(),
            "route canary transaction hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "receipt_block_hash",
            "0x" + "00" * 32,
            "route canary receipt block hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "block_receipts_root",
            "0x" + "00" * 32,
            "route canary block receipts root must be a canonical non-zero bytes32 hex string",
        ),
        (
            "message_id",
            "0x" + "00" * 32,
            "route canary message id must be a canonical non-zero bytes32 hex string",
        ),
        (
            "receipt_block_number",
            0,
            "route canary receipt block number must be a positive integer",
        ),
        *receipt_block_number_exactness_cases,
        *route_canary_evidence_bound_exactness_cases,
        *route_canary_status_exactness_cases,
        *route_canary_evidence_source_exactness_cases,
        *route_canary_hex32_exactness_cases,
        *route_canary_receipt_finalized_exactness_cases,
    )

    for field, value, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        canary = active_lane["route_allowlist"]["route_canary"]
        if value is None:
            canary.pop(field, None)
        else:
            canary[field] = value

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        route_canary_item = item_by_id["live_route_canary_evidence"]

        assert checklist["ready"] is False, field
        assert route_canary_item["ready"] is False, field
        assert any(
            expected_blocker in blocker
            for blocker in route_canary_item["blockers"]
        ), field


def test_release_readiness_report_blocks_malformed_active_route_allowlist_binding(
    tmp_path: Path,
) -> None:
    """The release checklist must validate active route allowlist binding fields."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    expected_match_flag_exactness_cases = (
        (
            "route_allowlist.expected_route_allowlist_hash_matches",
            "true",
            "route allowlist expected hash match flag must be true",
        ),
        (
            "route_allowlist.expected_route_allowlist_hash_matches",
            1,
            "route allowlist expected hash match flag must be true",
        ),
        (
            "route_allowlist.expected_route_allowlist_hash_matches",
            None,
            "route allowlist expected hash match flag must be true",
        ),
    )
    cases = (
        (
            "route_allowlist.route_allowlist_hash",
            "0x" + "00" * 32,
            "route allowlist hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "route_allowlist.route_allowlist_hash",
            fixed_hex32(0x41).upper(),
            "route allowlist hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "route_allowlist.expected_route_allowlist_hash",
            None,
            "expected route allowlist hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "route_allowlist.expected_route_allowlist_hash_matches",
            False,
            "route allowlist expected hash match flag must be true",
        ),
        *expected_match_flag_exactness_cases,
        (
            "route_allowlist.hash_mismatch",
            fixed_hex32(0x42),
            "route allowlist hash must match the expected canonical source, deployment, and destination binding hash",
        ),
        (
            "source_record_hashes.source_verifier_material_hash",
            "0x" + "00" * 32,
            "route allowlist source verifier material hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "source_record_hashes.source_adapter_engine_deployment_hash",
            None,
            "route allowlist source adapter engine deployment hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "source_record_hashes.hash_reuse",
            None,
            "route allowlist source verifier material hash must not reuse source adapter engine deployment hash",
        ),
        (
            "destination_binding.destination_binding_hash",
            "0x" + "00" * 32,
            "route allowlist destination binding hash must be a canonical non-zero bytes32 hex string",
        ),
    )

    for path, value, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        route_allowlist = active_lane["route_allowlist"]
        if path == "route_allowlist.hash_mismatch":
            route_allowlist["expected_route_allowlist_hash"] = value
            route_allowlist["expected_route_allowlist_hash_matches"] = True
        elif path == "source_record_hashes.hash_reuse":
            source_hashes = active_lane["source_record_hashes"]
            source_hashes["source_adapter_engine_deployment_hash"] = source_hashes[
                "source_verifier_material_hash"
            ]
        else:
            section, field = path.split(".", 1)
            target = active_lane[section]
            if value is None:
                target.pop(field, None)
            else:
                target[field] = value

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        route_item = item_by_id["route_allowlist_binding"]

        assert checklist["ready"] is False, path
        assert route_item["ready"] is False, path
        assert any(expected_blocker in blocker for blocker in route_item["blockers"]), path


def test_release_readiness_report_blocks_malformed_active_governed_deployment_metadata(
    tmp_path: Path,
) -> None:
    """The release checklist must validate active deployment and binding metadata."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    expected_destination_match_flag_exactness_cases = (
        (
            "destination_binding.expected_destination_binding_hash_matches",
            "true",
            "governed deployment destination binding expected hash match flag must be true",
        ),
        (
            "destination_binding.expected_destination_binding_hash_matches",
            1,
            "governed deployment destination binding expected hash match flag must be true",
        ),
        (
            "destination_binding.expected_destination_binding_hash_matches",
            None,
            "governed deployment destination binding expected hash match flag must be true",
        ),
    )
    cases = (
        (
            "source_record_hashes.source_verifier_material_hash",
            "0x" + "00" * 32,
            "governed deployment source verifier material hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "source_record_hashes.source_adapter_engine_deployment_hash",
            fixed_hex32(0x51).upper(),
            "governed deployment source adapter engine deployment hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "source_record_hashes.hash_reuse",
            None,
            "governed deployment source verifier material hash must not reuse source adapter engine deployment hash",
        ),
        (
            "destination_binding.destination_binding_hash",
            "0x" + "00" * 32,
            "governed deployment destination binding hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "destination_binding.expected_destination_binding_hash",
            None,
            "governed deployment expected destination binding hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "destination_binding.expected_destination_binding_hash_matches",
            False,
            "governed deployment destination binding expected hash match flag must be true",
        ),
        *expected_destination_match_flag_exactness_cases,
        (
            "destination_binding.hash_mismatch",
            fixed_hex32(0x52),
            "governed deployment destination binding hash must match the expected canonical binding hash",
        ),
        (
            "source_adapter_gate",
            None,
            "source adapter gate summary is missing",
        ),
        (
            "source_adapter_gate.ready",
            False,
            "source adapter gate summary must be ready",
        ),
        (
            "source_adapter_gate.required",
            False,
            "active EVM source adapter gate summary must be required",
        ),
        (
            "source_adapter_gate.gate_hash",
            "",
            "active EVM source adapter gate hash must be a canonical non-zero bytes32 hex string",
        ),
        (
            "source_adapter_gate.audit_hashes",
            {"unexpected_gate_hash": fixed_hex32(0x54)},
            "active EVM source adapter gate audit hashes must contain only evm_source_gate_hash",
        ),
    )

    for path, value, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        if path == "source_record_hashes.hash_reuse":
            source_hashes = active_lane["source_record_hashes"]
            source_hashes["source_adapter_engine_deployment_hash"] = source_hashes[
                "source_verifier_material_hash"
            ]
        elif path == "destination_binding.hash_mismatch":
            active_lane["destination_binding"]["expected_destination_binding_hash"] = (
                value
            )
            active_lane["destination_binding"][
                "expected_destination_binding_hash_matches"
            ] = True
        elif path == "source_adapter_gate":
            active_lane.pop("source_adapter_gate", None)
        else:
            section, field = path.split(".", 1)
            target = active_lane[section]
            if value is None:
                target.pop(field, None)
            else:
                target[field] = value

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        deployment_item = item_by_id["governed_deployment_evidence"]

        assert checklist["ready"] is False, path
        assert deployment_item["ready"] is False, path
        assert any(
            expected_blocker in blocker
            for blocker in deployment_item["blockers"]
        ), path


def test_release_readiness_report_blocks_malformed_active_required_record_metadata(
    tmp_path: Path,
) -> None:
    """The release checklist must validate active lane identity and record flags."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    required_record_flag_exactness_cases = (
        (
            "records.source_verifier_material",
            "yes",
            "missing source verifier material",
        ),
        (
            "records.source_adapter_deployment",
            1,
            "missing source adapter deployment",
        ),
        (
            "records.destination_rollout",
            False,
            "missing destination rollout",
        ),
        (
            "records.route_allowlist",
            None,
            "missing route allowlist",
        ),
    )
    cases = (
        (
            "domain.string",
            "1",
            "missing launch lane evidence",
        ),
        (
            "chain",
            "bsc",
            "active launch lane chain must be eth",
        ),
        (
            "chain.padded",
            " eth",
            "active launch lane chain must be eth",
        ),
        (
            "production_ready",
            False,
            "active launch lane must be production ready",
        ),
        (
            "production_ready.string",
            "true",
            "active launch lane must be production ready",
        ),
        (
            "records",
            None,
            "required record summary is missing",
        ),
        *required_record_flag_exactness_cases,
        (
            "records.operator_override",
            True,
            "required record summary contains unknown field: operator_override",
        ),
    )

    for path, value, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        if path == "records":
            active_lane.pop("records", None)
        elif path.startswith("records."):
            _, field = path.split(".", 1)
            records = active_lane["records"]
            if value is None:
                records.pop(field, None)
            else:
                records[field] = value
        elif path == "domain.string":
            active_lane["domain"] = value
        elif path == "chain.padded":
            active_lane["chain"] = value
        elif path == "production_ready.string":
            active_lane["production_ready"] = value
        else:
            active_lane[path] = value

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        records_item = item_by_id["all_required_lane_records"]

        assert checklist["ready"] is False, path
        assert records_item["ready"] is False, path
        assert any(
            expected_blocker in blocker for blocker in records_item["blockers"]
        ), path


def test_release_readiness_report_classifies_malformed_active_required_record_fields(
    tmp_path: Path,
) -> None:
    """Malformed active required-record keys must not leak through checklist text."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    confusable_field = "operat\u043er_override"
    markdown_field = "operator|secret-token"
    cases = (
        (
            " operator_override ",
            "required record summary contains unknown field name with surrounding whitespace",
        ),
        (
            "operator\noverride",
            "required record summary contains unknown field name with control character",
        ),
        (
            "operator override",
            "required record summary contains unknown field name with whitespace",
        ),
        (
            markdown_field,
            "required record summary contains unknown field name with Markdown-unsafe character",
        ),
        (
            confusable_field,
            "required record summary contains unknown field name with non-ASCII character",
        ),
    )

    for field, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        active_lane["records"][field] = True

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        records_item = item_by_id["all_required_lane_records"]
        blockers = "\n".join(records_item["blockers"])

        assert checklist["ready"] is False, field
        assert records_item["ready"] is False, field
        assert expected_blocker in blockers, field
        assert markdown_field not in blockers
        assert confusable_field not in blockers


def test_release_readiness_report_blocks_active_lane_unresolved_blockers(
    tmp_path: Path,
) -> None:
    """The no-unresolved-blockers item must inspect active lane blockers directly."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    cases = (
        (
            "lane.text",
            ["operator launch hold"],
            [],
            "domain 1 (eth): operator launch hold",
        ),
        (
            "lane.scalar",
            "operator launch hold",
            [],
            "domain 1 (eth): active launch lane blocker summary is malformed",
        ),
        (
            "lane.empty",
            [""],
            [],
            "domain 1 (eth): active launch lane blocker must be a non-empty canonical string",
        ),
        (
            "lane.padded",
            [" padded "],
            [],
            "domain 1 (eth): active launch lane blocker must be a non-empty canonical string",
        ),
        (
            "lane.numeric",
            [123],
            [],
            "domain 1 (eth): active launch lane blocker must be a non-empty canonical string",
        ),
        (
            "lane.null",
            [None],
            [],
            "domain 1 (eth): active launch lane blocker must be a non-empty canonical string",
        ),
        (
            "top_level.duplicate",
            ["operator launch hold"],
            ["domain 1 (eth): operator launch hold"],
            "domain 1 (eth): operator launch hold",
        ),
        (
            "top_level.empty",
            [],
            [""],
            "SCCP evidence blocker must be a non-empty canonical string",
        ),
        (
            "top_level.padded",
            [],
            [" padded "],
            "SCCP evidence blocker must be a non-empty canonical string",
        ),
        (
            "top_level.numeric",
            [],
            [123],
            "SCCP evidence blocker must be a non-empty canonical string",
        ),
        (
            "top_level.null",
            [],
            [None],
            "SCCP evidence blocker must be a non-empty canonical string",
        ),
        (
            "top_level.scalar",
            [],
            "operator launch hold",
            "SCCP evidence blocker summary is malformed",
        ),
    )

    for case_id, lane_blockers, top_level_blockers, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        active_lane["blockers"] = lane_blockers
        evidence_summary["blockers"] = top_level_blockers

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        unresolved_item = item_by_id["no_unresolved_blockers"]

        assert checklist["ready"] is False, case_id
        assert unresolved_item["ready"] is False, case_id
        assert expected_blocker in unresolved_item["blockers"], case_id
        if lane_blockers == ["operator launch hold"] and top_level_blockers:
            assert unresolved_item["blockers"].count(expected_blocker) == 1


def test_release_readiness_report_classifies_malformed_active_lane_blockers(
    tmp_path: Path,
) -> None:
    """Malformed active lane blockers must fail every category that classifies them."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    category_item_ids = (
        "governed_deployment_evidence",
        "route_allowlist_binding",
        "live_route_canary_evidence",
    )
    cases = (
        (
            "operator launch hold",
            category_item_ids,
            "domain 1 (eth): active launch lane blockers must be a list of non-empty canonical strings",
        ),
        (
            [123],
            category_item_ids,
            "domain 1 (eth): active launch lane blockers[0] must be a non-empty canonical string",
        ),
        (
            [" padded "],
            category_item_ids,
            "domain 1 (eth): active launch lane blockers[0] must be a non-empty canonical string",
        ),
        (
            ["route canary operator launch hold"],
            ("live_route_canary_evidence",),
            "domain 1 (eth): route canary operator launch hold",
        ),
    )

    for lane_blockers, expected_item_ids, expected_blocker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        active_lane = report._active_launch_lane(evidence_summary)
        assert active_lane is not None
        active_lane["blockers"] = lane_blockers

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}

        assert checklist["ready"] is False, repr(lane_blockers)
        for item_id in expected_item_ids:
            item = item_by_id[item_id]
            assert item["ready"] is False, (repr(lane_blockers), item_id)
            assert expected_blocker in item["blockers"], (
                repr(lane_blockers),
                item_id,
            )
        if not isinstance(lane_blockers, list):
            assert "o" not in item_by_id["live_route_canary_evidence"]["blockers"]


def test_release_readiness_report_blocks_malformed_native_prover_blockers(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Native prover blocker containers must not be filtered or character-expanded."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    cases = (
        (
            "operator override",
            "native EVM prover validation_blockers must be a list of non-empty canonical strings",
            None,
            False,
        ),
        (
            [123],
            "native EVM prover validation_blockers[0] must be a non-empty canonical string",
            None,
            False,
        ),
        (
            [" padded "],
            "native EVM prover validation_blockers[0] must be a non-empty canonical string",
            " padded ",
            True,
        ),
        (
            ["operator\nnative-blocker"],
            "native EVM prover validation_blockers[0] contains control character",
            "operator\nnative-blocker",
            True,
        ),
        (
            ["operator|native-blocker"],
            "native EVM prover validation_blockers[0] contains Markdown-unsafe character",
            "operator|native-blocker",
            True,
        ),
        (
            ["operator native bl\u043ecker"],
            "native EVM prover validation_blockers[0] contains non-ASCII character",
            "operator native bl\u043ecker",
            True,
        ),
        (
            ["operator secret-token-native-blocker"],
            "native EVM prover validation_blockers[0] contains sensitive name",
            "operator secret-token-native-blocker",
            True,
        ),
        (
            ["operator launch hold"],
            "operator launch hold",
            None,
            False,
        ),
    )

    for blocker_value, expected_blocker, forbidden_text, invalid_marker in cases:
        evidence_summary = report._load_evidence_summary([evidence])
        native_status = report._native_evm_prover_bundle_status(
            native_bundle,
            evidence_summary,
        )
        native_status["validation_blockers"] = blocker_value

        checklist = report._active_launch_release_checklist(
            evidence_summary,
            native_status,
        )
        item_by_id = {item["id"]: item for item in checklist["items"]}
        native_item = item_by_id["native_evm_groth16_prover_bundle"]

        assert checklist["ready"] is False, repr(blocker_value)
        assert native_item["ready"] is False, repr(blocker_value)
        assert expected_blocker in native_item["blockers"]
        assert "secret-token" not in "\n".join(native_item["blockers"])

        monkeypatch.setattr(
            report,
            "_native_evm_prover_bundle_status",
            lambda *_args, status=native_status: status,
        )
        readiness = report._build_report(
            [evidence],
            ["all=passed"],
            [],
            require_phase_evidence=False,
            native_evm_prover_bundle=native_bundle,
        )

        assert readiness["production_ready"] is False, repr(blocker_value)
        assert expected_blocker in readiness["blockers"]
        assert "o" not in readiness["blockers"]
        assert "secret-token" not in "\n".join(readiness["blockers"])

        markdown = report._render_markdown(readiness, max_blockers_per_lane=4)
        if invalid_marker:
            assert "`<invalid validation_blockers>`" in markdown
        if forbidden_text is not None:
            assert forbidden_text not in markdown


def test_release_readiness_report_blocks_without_native_evm_prover_bundle(
    tmp_path: Path,
) -> None:
    """Ethereum launch readiness must require native no-WASM prover artifacts."""

    report = load_report_module()
    evidence, _ = write_active_launch_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    assert payload["production_ready"] is False
    assert payload["native_evm_prover_bundle"]["validation_status"] == "blocked"
    assert (
        "native EVM Groth16 prover bundle manifest is required"
        in payload["native_evm_prover_bundle"]["validation_blockers"]
    )
    checklist = {
        item["id"]: item for item in payload["release_checklist"]["items"]
    }
    assert checklist["native_evm_groth16_prover_bundle"]["ready"] is False
    assert all(
        f"domain {report.ACTIVE_LAUNCH_DOMAIN}" not in blocker
        for blocker in payload["blockers"]
    )


def test_release_readiness_report_blocks_wasm_or_remote_native_evm_prover_bundle(
    tmp_path: Path,
) -> None:
    """Metadata-only callbacks must not satisfy the native no-WASM prover gate."""

    missing = object()
    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle_boolean_exactness_cases = (
        (
            "no_wasm",
            False,
            "native EVM Groth16 prover bundle no_wasm must be true",
        ),
        (
            "no_wasm",
            "true",
            "native EVM Groth16 prover bundle no_wasm must be true",
        ),
        (
            "no_wasm",
            1,
            "native EVM Groth16 prover bundle no_wasm must be true",
        ),
        (
            "no_wasm",
            None,
            "native EVM Groth16 prover bundle no_wasm must be true",
        ),
        (
            "no_wasm",
            missing,
            "native EVM Groth16 prover bundle no_wasm must be true",
        ),
        (
            "remote_prover_required",
            True,
            "native EVM Groth16 prover bundle remote_prover_required must be false",
        ),
        (
            "remote_prover_required",
            "false",
            "native EVM Groth16 prover bundle remote_prover_required must be false",
        ),
        (
            "remote_prover_required",
            0,
            "native EVM Groth16 prover bundle remote_prover_required must be false",
        ),
        (
            "remote_prover_required",
            None,
            "native EVM Groth16 prover bundle remote_prover_required must be false",
        ),
        (
            "remote_prover_required",
            missing,
            "native EVM Groth16 prover bundle remote_prover_required must be false",
        ),
    )

    for index, (field, value, expected_blocker) in enumerate(
        native_bundle_boolean_exactness_cases
    ):
        case_dir = tmp_path / f"native-bundle-boolean-{index}"
        case_dir.mkdir()
        native_bundle = write_native_evm_prover_bundle(case_dir, evidence)
        if value is missing:
            payload = json.loads(native_bundle.read_text(encoding="utf-8"))
            payload.pop(field, None)
            native_bundle.write_text(
                json.dumps(payload, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
        else:
            payload = json.loads(native_bundle.read_text(encoding="utf-8"))
            payload[field] = value
            native_bundle.write_text(
                json.dumps(payload, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--format",
                "json",
                "--phase-result",
                "all=passed",
                "--native-evm-prover-bundle",
                str(native_bundle),
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1, field
        payload = json.loads(completed.stdout)
        blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
        assert expected_blocker in blockers, field
        assert payload["release_checklist"]["ready"] is False, field


def test_release_readiness_report_blocks_duplicate_native_evm_prover_json_keys(
    tmp_path: Path,
) -> None:
    """Signed native prover manifests must not rely on JSON last-key-wins parsing."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    manifest = native_bundle.read_text(encoding="utf-8")
    native_bundle.write_text(
        manifest.replace(
            '  "bundle_id": "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1",',
            '  "bundle_id": "sccp:eth:native-evm-groth16-prover:forged",\n'
            '  "bundle_id": "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1",',
            1,
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle JSON contains duplicate key: bundle_id"
        in blockers
    )
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_prover_nested_json_keys(
    tmp_path: Path,
) -> None:
    """Duplicate nested manifest keys must fail before audit hashes are trusted."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    manifest = native_bundle.read_text(encoding="utf-8")
    native_bundle.write_text(
        manifest.replace(
            '    "circuit_security_audit": "',
            '    "circuit_security_audit": "0x'
            + "f1" * 32
            + '",\n    "circuit_security_audit": "',
            1,
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle JSON contains duplicate key: "
        "circuit_security_audit"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_prover_sdk_artifact_keys(
    tmp_path: Path,
) -> None:
    """Duplicate SDK artifact row keys must fail before implementation hashes are trusted."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    manifest = native_bundle.read_text(encoding="utf-8")
    native_bundle.write_text(
        manifest.replace(
            '      "implementation_hash": "',
            '      "implementation_hash": "0x'
            + "f2" * 32
            + '",\n      "implementation_hash": "',
            1,
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    duplicate_marker = "native EVM Groth16 prover bundle JSON contains duplicate key: implementation_hash"
    assert duplicate_marker in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_malformed_duplicate_json_keys(
    tmp_path: Path,
) -> None:
    """Duplicate-key blockers must not echo malformed native manifest keys."""

    cases = (
        ("operator\\nnote", "control character", "operator\nnote"),
        ("operat\\u043er_note", "non-ASCII character", "operat\u043er_note"),
        (
            "secret-token-native-duplicate",
            "sensitive key name",
            "secret-token-native-duplicate",
        ),
    )
    for index, (encoded_key, expected_reason, decoded_key) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        evidence, _ = write_active_launch_evidence(case_dir)
        native_bundle = write_native_evm_prover_bundle(case_dir, evidence)
        manifest = native_bundle.read_text(encoding="utf-8")
        native_bundle.write_text(
            manifest.replace(
                "{\n",
                "{\n"
                f'  "{encoded_key}": "first",\n'
                f'  "{encoded_key}": "second",\n',
                1,
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--format",
                "json",
                "--phase-result",
                "all=passed",
                "--native-evm-prover-bundle",
                str(native_bundle),
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        payload = json.loads(completed.stdout)
        blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
        assert (
            "native EVM Groth16 prover bundle JSON contains duplicate key "
            f"with {expected_reason}"
        ) in blockers
        assert all(decoded_key not in blocker for blocker in blockers)
        assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_redacts_malformed_native_evm_prover_json(
    tmp_path: Path,
) -> None:
    """Native prover manifest JSON errors must not echo parser payloads."""

    cases = (
        (
            b'{"secret-token-native-manifest": ',
            "native EVM Groth16 prover bundle is not valid JSON",
        ),
        (
            b"\xffsecret-token-native-manifest",
            "native EVM Groth16 prover bundle is not UTF-8 text",
        ),
    )
    for index, (payload_bytes, expected_blocker) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        evidence, _ = write_active_launch_evidence(case_dir)
        native_bundle = write_native_evm_prover_bundle(case_dir, evidence)
        native_bundle.write_bytes(payload_bytes)

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--format",
                "json",
                "--phase-result",
                "all=passed",
                "--native-evm-prover-bundle",
                str(native_bundle),
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        payload = json.loads(completed.stdout)
        blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
        rendered = "\n".join(blockers)
        assert expected_blocker in blockers
        assert f"{expected_blocker}:" not in rendered
        assert "secret-token" not in rendered
        assert "Traceback" not in completed.stderr
        assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_redacts_malformed_native_evm_prover_fixture_json(
    tmp_path: Path,
) -> None:
    """Native prover nested JSON fixture errors must not echo parser payloads."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    parity_path.write_bytes(b'{"secret-token-parity-fixture": ' + b" " * 4096)
    self_test_path.write_bytes(b"\xff" + b"secret-token-self-test" * 256)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report_payload = json.loads(completed.stdout)
    blockers = report_payload["native_evm_prover_bundle"]["validation_blockers"]
    rendered = "\n".join(blockers)
    expected_parity = (
        "native EVM Groth16 prover bundle "
        "cross_sdk_fixture_parity_artifact is not valid JSON"
    )
    expected_self_test = (
        "native EVM Groth16 prover bundle "
        "native_prover_self_test_artifact is not UTF-8 text"
    )
    assert expected_parity in blockers
    assert expected_self_test in blockers
    assert f"{expected_parity}:" not in rendered
    assert f"{expected_self_test}:" not in rendered
    assert "secret-token" not in rendered
    assert "Traceback" not in completed.stderr
    assert report_payload["release_checklist"]["ready"] is False


def test_release_readiness_report_redacts_native_evm_payload_artifact_path_failures(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Native prover payload artifact path failures must stay category-only."""

    report = load_report_module()
    manifest_path = tmp_path / "native-bundle.json"
    manifest_path.write_text("{}\n", encoding="utf-8")
    for relative_path in ("proof.bin", "parity.json", "self-test.json"):
        (tmp_path / relative_path).write_bytes(b"x" * 2048)
    payload = {
        "proof_artifact": "proof.bin",
        "cross_sdk_fixture_parity_artifact": "parity.json",
        "native_prover_self_test_artifact": "self-test.json",
    }

    def secret_artifact_failure(_path: Path) -> dict[str, object]:
        raise ValueError("secret-token artifact path detail")

    monkeypatch.setattr(report, "_artifact", secret_artifact_failure)

    cases = (
        (
            report._native_evm_prover_payload_artifact(
                manifest_path,
                payload,
                "proof_artifact",
                "proof_artifact_hash",
                "proof_artifact",
            ),
            "native EVM Groth16 prover bundle proof_artifact "
            "artifact path metadata is invalid",
        ),
        (
            report._native_evm_prover_parity_fixture_status(manifest_path, payload),
            "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
            "artifact path metadata is invalid",
        ),
        (
            report._native_evm_prover_self_test_status(manifest_path, payload),
            "native EVM Groth16 prover bundle native_prover_self_test_artifact "
            "artifact path metadata is invalid",
        ),
    )

    rendered = []
    for (artifact, blockers), expected_blocker in cases:
        assert artifact is None
        assert expected_blocker in blockers
        rendered.extend(blockers)
    rendered_blockers = "\n".join(rendered)
    assert "secret-token" not in rendered_blockers
    assert "artifact path detail" not in rendered_blockers


def test_release_readiness_report_redacts_native_evm_manifest_artifact_path_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Native prover manifest artifact path failures must stay category-only."""

    report = load_report_module()
    manifest_path = tmp_path / "native-bundle.json"
    evidence, _ = write_active_launch_evidence(tmp_path)

    def secret_artifact_failure(_path: Path) -> dict[str, object]:
        raise ValueError("secret-token native manifest path detail")

    monkeypatch.setattr(report, "_artifact", secret_artifact_failure)

    evidence_summary = report._load_evidence_summary([evidence])
    native_status = report._native_evm_prover_bundle_status(
        manifest_path,
        evidence_summary,
    )
    blockers = native_status["validation_blockers"]
    rendered = "\n".join(blockers)

    assert (
        "native EVM Groth16 prover bundle artifact path metadata is invalid"
        in blockers
    )
    assert "secret-token" not in rendered
    assert "manifest path detail" not in rendered


def test_release_readiness_report_cli_redacts_top_level_exception_details(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    """Top-level readiness CLI exceptions must not echo sensitive payloads."""

    report = load_report_module()

    for exception_type in (RuntimeError, TypeError, ValueError):

        def fail_build(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(report, "_build_report", fail_build)
            try:
                report.main([str(tmp_path / "evidence.toml")])
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("readiness CLI accepted top-level build failure")

            captured = capsys.readouterr()
            assert "SCCP release readiness report generation failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_release_readiness_report_blocks_native_evm_prover_unknown_root_and_audit_fields(
    tmp_path: Path,
) -> None:
    """Native prover manifests must keep root and audit schemas exact."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    removed_audit = sorted(payload["audit_hashes"])[0]
    payload["operator_note"] = "not allowed"
    payload["audit_hashes"].pop(removed_audit)
    payload["audit_hashes"]["operator_note"] = fixed_hex32(0xD5)
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle contains unknown field: operator_note"
        in blockers
    )
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field: operator_note"
    ) in blockers
    assert (
        f"native EVM Groth16 prover bundle audit_hashes missing field: {removed_audit}"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_malformed_unknown_field_names(
    tmp_path: Path,
) -> None:
    """Native prover schema blockers must not echo malformed field names."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    confusable_root = "operat\u043er_note"
    confusable_audit = "aud\u0456t_note"
    secret_root = "secret_token_native_root_field"
    secret_audit = "secret_token_native_audit_field"
    payload[" operator_note "] = "not allowed"
    payload["operator\nnote"] = "not allowed"
    payload["operator note"] = "not allowed"
    payload["operator|note"] = "not allowed"
    payload[confusable_root] = "not allowed"
    payload[secret_root] = "not allowed"
    payload["audit_hashes"][" audit_note "] = fixed_hex32(0xD6)
    payload["audit_hashes"]["audit\nnote"] = fixed_hex32(0xD7)
    payload["audit_hashes"]["audit note"] = fixed_hex32(0xD8)
    payload["audit_hashes"]["audit|note"] = fixed_hex32(0xD9)
    payload["audit_hashes"][confusable_audit] = "not-a-hex32"
    payload["audit_hashes"][secret_audit] = fixed_hex32(0xDA)
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with control character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with Markdown-unsafe character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with non-ASCII character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle contains unknown field name "
        "with sensitive name"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with control character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with Markdown-unsafe character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with non-ASCII character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes contains unexpected "
        "field name with sensitive name"
    ) in blockers
    assert all(confusable_root not in blocker for blocker in blockers)
    assert all(confusable_audit not in blocker for blocker in blockers)
    assert all(secret_root not in blocker for blocker in blockers)
    assert all(secret_audit not in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_payload_hash_mismatch(
    tmp_path: Path,
) -> None:
    """Native prover readiness must verify the artifact bytes, not only metadata."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    (tmp_path / "native-prover-artifacts" / "proof-artifact.bin").write_bytes(
        b"tampered native proof artifact\n"
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact sha256 must match "
        "proof_artifact_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_empty_native_evm_prover_payload(
    tmp_path: Path,
) -> None:
    """Native prover payload files must carry real bytes, not empty hashes."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    proof_path = tmp_path / "native-prover-artifacts" / "proof-artifact.bin"
    proof_path.write_bytes(b"")
    proof_hash = "0x" + hashlib.sha256(b"").hexdigest()
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact_hash"] = proof_hash
    for artifact in payload["native_sdk_artifacts"]:
        artifact["prover_artifact_hash"] = proof_hash
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact must not be empty"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_tiny_native_evm_prover_payload(
    tmp_path: Path,
) -> None:
    """Native prover payload files must not be hash-consistent tiny placeholders."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    proof_path = tmp_path / "native-prover-artifacts" / "proof-artifact.bin"
    tiny_payload = b"tiny native proof artifact\n"
    proof_path.write_bytes(tiny_payload)
    proof_hash = "0x" + hashlib.sha256(tiny_payload).hexdigest()
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact_hash"] = proof_hash
    for artifact in payload["native_sdk_artifacts"]:
        artifact["prover_artifact_hash"] = proof_hash
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact must be at least "
        "65536 bytes"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_below_floor_native_evm_implementation(
    tmp_path: Path,
) -> None:
    """Native SDK implementation artifacts must meet the launch SDK byte floor."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    implementation_path = (
        tmp_path / "native-prover-artifacts" / "javascript-implementation.bin"
    )
    tiny_payload = b"tiny native javascript implementation\n"
    implementation_path.write_bytes(tiny_payload)
    implementation_hash = "0x" + hashlib.sha256(tiny_payload).hexdigest()
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    for artifact in payload["native_sdk_artifacts"]:
        if artifact["sdk"] == "javascript":
            artifact["implementation_hash"] = implementation_hash
            break
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle javascript implementation_artifact "
        "must be at least 1024 bytes"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_below_floor_native_evm_support_roles(
    tmp_path: Path,
) -> None:
    """Native verifier and support artifacts must meet role-specific floors."""

    cases = (
        (
            "verifier-key",
            "verifier_key",
            ("verifier_key_hash",),
            b"tiny native verifier key\n",
            "native EVM Groth16 prover bundle verifier_key must be at least 128 bytes",
        ),
        (
            "parity-fixture",
            "cross_sdk_fixture_parity_artifact",
            ("audit_hashes", "cross_sdk_fixture_parity"),
            b"{}\n",
            "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
            "must be at least 128 bytes",
        ),
        (
            "self-test-fixture",
            "native_prover_self_test_artifact",
            ("audit_hashes", "native_prover_self_test"),
            b"{}\n",
            "native EVM Groth16 prover bundle native_prover_self_test_artifact "
            "must be at least 128 bytes",
        ),
    )

    for label, path_field, hash_path, tiny_payload, expected_blocker in cases:
        case_dir = tmp_path / label
        case_dir.mkdir()
        evidence, _ = write_active_launch_evidence(case_dir)
        native_bundle = write_native_evm_prover_bundle(case_dir, evidence)
        payload = json.loads(native_bundle.read_text(encoding="utf-8"))
        artifact_path = case_dir / payload[path_field]
        artifact_path.write_bytes(tiny_payload)
        artifact_hash = "0x" + hashlib.sha256(tiny_payload).hexdigest()
        if hash_path[0] == "audit_hashes":
            payload["audit_hashes"][hash_path[1]] = artifact_hash
        else:
            payload[hash_path[0]] = artifact_hash
        native_bundle.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--format",
                "json",
                "--phase-result",
                "all=passed",
                "--native-evm-prover-bundle",
                str(native_bundle),
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1, label
        report_payload = json.loads(completed.stdout)
        blockers = report_payload["native_evm_prover_bundle"]["validation_blockers"]
        assert expected_blocker in blockers
        assert report_payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_reused_native_evm_prover_role_hash(
    tmp_path: Path,
) -> None:
    """Native prover artifact, key, and implementation hashes are separate roles."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proving_key"] = payload["proof_artifact"]
    payload["proving_key_hash"] = payload["proof_artifact_hash"]
    payload["native_sdk_artifacts"][0]["proving_key_hash"] = (
        payload["proving_key_hash"]
    )
    payload["native_sdk_artifacts"][0]["implementation_artifact"] = (
        payload["proof_artifact"]
    )
    payload["native_sdk_artifacts"][0]["implementation_hash"] = (
        payload["proof_artifact_hash"]
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proving_key_hash must not reuse "
        "proof_artifact_hash"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle "
        "native_sdk_artifacts[0].implementation_hash must not reuse "
        "proof_artifact_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_reused_native_evm_prover_artifact_paths(
    tmp_path: Path,
) -> None:
    """Native prover artifact paths must be unique across evidence roles."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    secret_role_path = (
        tmp_path / "native-prover-artifacts" / "secret-token-role-reuse.bin"
    )
    secret_role_path.write_bytes((tmp_path / payload["proving_key"]).read_bytes())
    secret_role = secret_role_path.relative_to(tmp_path).as_posix()
    payload["proof_artifact"] = secret_role
    payload["proving_key"] = secret_role
    payload["native_sdk_artifacts"][1]["implementation_artifact"] = (
        payload["verifier_key"]
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proving_key path must not reuse "
        "proof_artifact"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle "
        "native_sdk_artifacts[1].implementation_artifact path must not reuse "
        "verifier_key"
    ) in blockers
    assert "secret-token" not in "\n".join(blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_noncanonical_native_evm_prover_hash(
    tmp_path: Path,
) -> None:
    """Native prover hashes must be canonical lowercase 0x-prefixed hex."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["audit_hashes"]["circuit_security_audit"] = "0x" + "A1" * 32
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle audit_hashes.circuit_security_audit "
        "must be a "
        "canonical non-zero 32-byte hex value"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_prover_sdk_artifacts(
    tmp_path: Path,
) -> None:
    """Native prover manifests must not hide a required SDK behind a duplicate row."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    sdk_artifacts = payload["native_sdk_artifacts"]
    duplicate_sdk = sdk_artifacts[0]["sdk"]
    removed_sdk = sdk_artifacts[-1]["sdk"]
    sdk_artifacts[-1] = dict(sdk_artifacts[0])
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        f"native_sdk_artifacts contains duplicate sdk: {duplicate_sdk}"
    ) in blockers
    assert f"native_sdk_artifacts missing sdk: {removed_sdk}" in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_sdk_artifact_order(
    tmp_path: Path,
) -> None:
    """Native prover manifests must keep SDK artifact rows in canonical order."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["native_sdk_artifacts"] = list(reversed(payload["native_sdk_artifacts"]))
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert "native_sdk_artifacts must match expected SDK order" in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_malformed_native_evm_prover_sdk_artifacts(
    tmp_path: Path,
) -> None:
    """Native prover manifests must reject malformed SDK artifact rows."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["native_sdk_artifacts"][0] = []
    payload["native_sdk_artifacts"][1]["operator_note"] = "not allowed"
    payload["native_sdk_artifacts"][2].pop("implementation_hash")
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert "native_sdk_artifacts[0] must be an object" in blockers
    assert (
        "native_sdk_artifacts[1] contains unknown field: operator_note"
    ) in blockers
    assert "native_sdk_artifacts[2] missing field: implementation_hash" in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_sdk_artifact_malformed_unknown_field_names(
    tmp_path: Path,
) -> None:
    """Native prover SDK rows must reject unsafe unknown field names."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    confusable_field = "operat\u043er_sdk_note"
    secret_field = "secret_token_native_sdk_field"
    row = payload["native_sdk_artifacts"][0]
    row[" operator_sdk_note "] = "not allowed"
    row["operator\nsdk_note"] = "not allowed"
    row["operator sdk_note"] = "not allowed"
    row["operator|sdk_note"] = "not allowed"
    row[confusable_field] = "not allowed"
    row[secret_field] = "not allowed"
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native_sdk_artifacts[0] contains unknown field name with surrounding whitespace"
        in blockers
    )
    assert (
        "native_sdk_artifacts[0] contains unknown field name with control character"
        in blockers
    )
    assert "native_sdk_artifacts[0] contains unknown field name with whitespace" in blockers
    assert (
        "native_sdk_artifacts[0] contains unknown field name with Markdown-unsafe character"
        in blockers
    )
    assert (
        "native_sdk_artifacts[0] contains unknown field name with non-ASCII character"
        in blockers
    )
    assert (
        "native_sdk_artifacts[0] contains unknown field name with sensitive name"
        in blockers
    )
    assert all(confusable_field not in blocker for blocker in blockers)
    assert all(secret_field not in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_padded_native_evm_prover_sdk_artifacts(
    tmp_path: Path,
) -> None:
    """Native prover manifests must reject SDK ids normalized by trimming."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    missing_sdk = payload["native_sdk_artifacts"][0]["sdk"]
    payload["native_sdk_artifacts"][0]["sdk"] = f" {missing_sdk} "
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert "native_sdk_artifacts[0].sdk must not contain surrounding whitespace" in blockers
    assert f"native_sdk_artifacts missing sdk: {missing_sdk}" in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_malformed_native_evm_prover_sdk_artifact_ids(
    tmp_path: Path,
) -> None:
    """Native prover SDK artifact ids must reject unsafe spellings before matching."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    sdk_artifacts = payload["native_sdk_artifacts"]
    removed_sdks = [row["sdk"] for row in sdk_artifacts[:5]]
    confusable_sdk = "javas\u0441ript"
    sdk_artifacts[0]["sdk"] = "java script"
    sdk_artifacts[1]["sdk"] = "swift\nsdk"
    sdk_artifacts[2]["sdk"] = confusable_sdk
    sdk_artifacts[3]["sdk"] = "kotlin_sdk"
    sdk_artifacts[4]["sdk"] = "-dotnet"
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert "native_sdk_artifacts[0].sdk must not contain whitespace" in blockers
    assert "native_sdk_artifacts[1].sdk contains control character" in blockers
    assert "native_sdk_artifacts[2].sdk must be printable ASCII" in blockers
    assert "native_sdk_artifacts[3].sdk must be a lowercase SDK id" in blockers
    assert "native_sdk_artifacts[4].sdk must be a lowercase SDK id" in blockers
    for sdk in removed_sdks:
        assert f"native_sdk_artifacts missing sdk: {sdk}" in blockers
    assert confusable_sdk not in "\n".join(blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_sdk_artifact_value_drift(
    tmp_path: Path,
) -> None:
    """Native prover manifests must reject SDK artifact semantic drift."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    unknown_sdk = "rogue-sdk"
    secret_unknown_sdk = "secret-token-sdk"
    missing_sdk = payload["native_sdk_artifacts"][0]["sdk"]
    drift_implementation_sdk = payload["native_sdk_artifacts"][1]["sdk"]
    expected_implementation = payload["native_sdk_artifacts"][1]["implementation"]
    drift_sdk = payload["native_sdk_artifacts"][2]["sdk"]
    secret_row = dict(payload["native_sdk_artifacts"][3])
    secret_row["sdk"] = secret_unknown_sdk
    payload["native_sdk_artifacts"].append(secret_row)
    payload["native_sdk_artifacts"][0]["sdk"] = unknown_sdk
    payload["native_sdk_artifacts"][1]["implementation"] = "wrong-implementation"
    payload["native_sdk_artifacts"][2]["prover_artifact_hash"] = fixed_hex32(0xD3)
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert f"native_sdk_artifacts contains unknown sdk: {unknown_sdk}" in blockers
    assert "native_sdk_artifacts contains unknown sdk with sensitive name" in blockers
    assert f"native_sdk_artifacts missing sdk: {missing_sdk}" in blockers
    assert (
        f"{drift_implementation_sdk} implementation must be {expected_implementation}"
    ) in blockers
    assert (
        f"{drift_sdk} prover_artifact_hash must match proof_artifact_hash"
    ) in blockers
    assert secret_unknown_sdk not in "\n".join(blockers)
    assert secret_unknown_sdk not in completed.stdout
    assert secret_unknown_sdk not in completed.stderr
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_sdk_implementation_artifact_drift(
    tmp_path: Path,
) -> None:
    """Native prover manifests must reject SDK implementation artifact drift."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    uri_sdk = payload["native_sdk_artifacts"][0]["sdk"]
    hash_sdk = payload["native_sdk_artifacts"][1]["sdk"]
    payload["native_sdk_artifacts"][0]["implementation_artifact"] = (
        "ipfs:sdk-implementation.bin"
    )
    payload["native_sdk_artifacts"][1]["implementation_hash"] = fixed_hex32(0xD4)
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        f"native EVM Groth16 prover bundle {uri_sdk} implementation_artifact "
        "path must not contain URI schemes or drive prefixes"
    ) in blockers
    assert (
        f"native EVM Groth16 prover bundle {hash_sdk} implementation_artifact "
        "sha256 must match implementation_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_reused_native_evm_prover_audit_hash(
    tmp_path: Path,
) -> None:
    """Native prover audit hashes must be unique and role-separated."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["audit_hashes"]["circuit_security_audit"] = payload[
        "proof_artifact_hash"
    ]
    payload["audit_hashes"]["native_implementation_audit"] = payload[
        "native_sdk_artifacts"
    ][0]["implementation_hash"]
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = fixed_hex32(0xA1)
    payload["audit_hashes"]["no_wasm_no_remote_scan"] = fixed_hex32(0xA1)
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle audit_hashes.circuit_security_audit "
        "must not reuse "
        "proof_artifact_hash"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes.native_implementation_audit "
        "must not reuse "
        "native_sdk_artifacts[0].implementation_hash"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle audit_hashes.no_wasm_no_remote_scan "
        "must not duplicate audit_hashes.cross_sdk_fixture_parity"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_unlabeled_native_evm_prover_audits(
    tmp_path: Path,
) -> None:
    """Native prover audits must be named evidence fields, not filler hashes."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["audit_hashes"] = [fixed_hex32(0xA1)]
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle audit_hashes must be a non-empty object"
        in blockers
    )
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_missing_native_evm_parity_fixture(
    tmp_path: Path,
) -> None:
    """Native prover readiness must require concrete cross-SDK parity vectors."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload.pop("cross_sdk_fixture_parity_artifact")
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle missing field: "
        "cross_sdk_fixture_parity_artifact"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_tampered_native_evm_parity_fixture_hash(
    tmp_path: Path,
) -> None:
    """The cross-SDK parity vector file must match its named audit hash."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_path.write_text("{}\n", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sha256 must match audit_hashes.cross_sdk_fixture_parity"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_parity_fixture_keys(
    tmp_path: Path,
) -> None:
    """The cross-SDK parity fixture must reject duplicate JSON fields."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_bytes = (
        b'{"schema":"forged","schema":"'
        + payload["schema"].encode("utf-8")
        + b'"}\n'
    )
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    duplicate_marker = "cross_sdk_fixture_parity_artifact JSON contains duplicate key"
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "JSON contains duplicate key: schema"
    ) in blockers
    assert any(duplicate_marker in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_parity_fixture_malformed_duplicate_keys(
    tmp_path: Path,
) -> None:
    """Parity fixture duplicate-key blockers must not echo malformed names."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_text = '{"operator\\nnote":"first","operator\\nnote":"second"}\n'
    parity_bytes = parity_text.encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "JSON contains duplicate key with control character"
    ) in blockers
    assert all("operator\nnote" not in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_parity_fixture_malformed_unknown_field_names(
    tmp_path: Path,
) -> None:
    """Parity fixture unknown field blockers must not echo malformed keys."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    confusable_root = "operat\u043er_fixture_note"
    confusable_result = "operat\u043er_result_note"
    parity_payload[" fixture_note "] = "not allowed"
    parity_payload["fixture\nnote"] = "not allowed"
    parity_payload["fixture note"] = "not allowed"
    parity_payload["fixture|note"] = "not allowed"
    parity_payload[confusable_root] = "not allowed"
    result = parity_payload["sdk_results"]["javascript"]
    result[" result_note "] = "not allowed"
    result["result\nnote"] = "not allowed"
    result["result note"] = "not allowed"
    result["result|note"] = "not allowed"
    result[confusable_result] = "not allowed"
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "contains unknown field name with surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "contains unknown field name with control character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "contains unknown field name with whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "contains unknown field name with Markdown-unsafe character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "contains unknown field name with non-ASCII character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results.javascript contains unknown field name with surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results.javascript contains unknown field name with control character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results.javascript contains unknown field name with whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results.javascript contains unknown field name with Markdown-unsafe character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results.javascript contains unknown field name with non-ASCII character"
    ) in blockers
    assert all(confusable_root not in blocker for blocker in blockers)
    assert all(confusable_result not in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_parity_fixture_sdk_result_keys(
    tmp_path: Path,
) -> None:
    """Cross-SDK parity SDK rows must reject duplicate JSON fields."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_text = parity_path.read_text(encoding="utf-8")
    duplicate_key_offset = parity_text.index(
        '      "calldata_hash": "',
        parity_text.index('    "javascript": {'),
    )
    parity_bytes = (
        parity_text[:duplicate_key_offset]
        + '      "calldata_hash": "'
        + fixed_hex32(0xF1)
        + '",\n'
        + parity_text[duplicate_key_offset:]
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    duplicate_marker = "cross_sdk_fixture_parity_artifact JSON contains duplicate key"
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "JSON contains duplicate key: calldata_hash"
    ) in blockers
    assert any(duplicate_marker in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_self_test_keys(
    tmp_path: Path,
) -> None:
    """The native prover self-test fixture must reject duplicate JSON fields."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_bytes = (
        b'{"schema":"forged","schema":"'
        + payload["schema"].encode("utf-8")
        + b'"}\n'
    )
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    duplicate_marker = "native_prover_self_test_artifact JSON contains duplicate key"
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "JSON contains duplicate key: schema"
    ) in blockers
    assert any(duplicate_marker in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_duplicate_native_evm_self_test_sdk_result_keys(
    tmp_path: Path,
) -> None:
    """Native prover self-test SDK rows must reject duplicate JSON fields."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_text = self_test_path.read_text(encoding="utf-8")
    duplicate_key_offset = self_test_text.index(
        '      "proof_hash": "',
        self_test_text.index('    "kotlin": {'),
    )
    self_test_bytes = (
        self_test_text[:duplicate_key_offset]
        + '      "proof_hash": "'
        + fixed_hex32(0xF2)
        + '",\n'
        + self_test_text[duplicate_key_offset:]
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    duplicate_marker = "native_prover_self_test_artifact JSON contains duplicate key"
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "JSON contains duplicate key: proof_hash"
    ) in blockers
    assert any(duplicate_marker in blocker for blocker in blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_parity_fixture_sdk_drift(
    tmp_path: Path,
) -> None:
    """Every required SDK row in the parity vector must match the shared hashes."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_payload["sdk_results"]["javascript"]["calldata_hash"] = fixed_hex32(0xD1)
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle "
        "cross_sdk_fixture_parity_artifact sdk_results.javascript.calldata_hash "
        "must match calldata_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_missing_sdk_results(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must include every required SDK result row."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    removed_parity_sdk = sorted(parity_payload["sdk_results"])[0]
    parity_payload["sdk_results"].pop(removed_parity_sdk)
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    removed_self_test_sdk = sorted(self_test_payload["sdk_results"])[-1]
    self_test_payload["sdk_results"].pop(removed_self_test_sdk)
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results missing sdk: {removed_parity_sdk}"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results missing sdk: {removed_self_test_sdk}"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_malformed_sdk_results(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must carry SDK results as non-empty objects."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_payload["sdk_results"] = {}
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_payload["sdk_results"] = []
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results must be a non-empty object"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "sdk_results must be a non-empty object"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_malformed_sdk_result_rows(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must reject malformed SDK result rows."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_sdk = sorted(parity_payload["sdk_results"])[0]
    parity_payload["sdk_results"][parity_sdk] = []
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_sdk = sorted(self_test_payload["sdk_results"])[-1]
    self_test_payload["sdk_results"][self_test_sdk]["operator_note"] = "not allowed"
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results.{parity_sdk} must be an object"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results.{self_test_sdk} contains unknown field: operator_note"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_sdk_result_missing_fields(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must reject incomplete SDK result rows."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_sdk = sorted(parity_payload["sdk_results"])[0]
    parity_payload["sdk_results"][parity_sdk].pop("calldata_hash")
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_sdk = sorted(self_test_payload["sdk_results"])[-1]
    self_test_payload["sdk_results"][self_test_sdk].pop("proof_hash")
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results.{parity_sdk} missing field: calldata_hash"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results.{self_test_sdk} missing field: proof_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_sdk_result_value_drift(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must reject SDK row value drift."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_sdk = sorted(parity_payload["sdk_results"])[0]
    parity_signal_words = list(
        parity_payload["sdk_results"][parity_sdk]["public_signal_words"]
    )
    parity_signal_words[0] = fixed_hex32(0xE1)
    parity_payload["sdk_results"][parity_sdk][
        "public_signal_words"
    ] = parity_signal_words
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_sdk = sorted(self_test_payload["sdk_results"])[-1]
    self_test_payload["sdk_results"][self_test_sdk]["proof_hash"] = fixed_hex32(0xD2)
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results.{parity_sdk}.public_signal_words must match public_signal_words"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results.{self_test_sdk}.proof_hash must match proof_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_unknown_sdk_results(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixtures must not include unrecognized SDK rows."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    rogue_sdk = "rogue-sdk"
    secret_sdk = "secret-token-sdk"

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_result = next(iter(parity_payload["sdk_results"].values()))
    parity_payload["sdk_results"][rogue_sdk] = dict(parity_result)
    parity_payload["sdk_results"][secret_sdk] = dict(parity_result)
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_result = next(iter(self_test_payload["sdk_results"].values()))
    self_test_payload["sdk_results"][rogue_sdk] = dict(self_test_result)
    self_test_payload["sdk_results"][secret_sdk] = dict(self_test_result)
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results contains unknown sdk: {rogue_sdk}"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results contains unknown sdk with sensitive name"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results contains unknown sdk: {rogue_sdk}"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "sdk_results contains unknown sdk with sensitive name"
    ) in blockers
    assert secret_sdk not in "\n".join(blockers)
    assert secret_sdk not in completed.stdout
    assert secret_sdk not in completed.stderr
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_padded_sdk_results(
    tmp_path: Path,
) -> None:
    """Parity and self-test fixture SDK result keys must be canonical text."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_sdk = sorted(parity_payload["sdk_results"])[0]
    parity_padded_sdk = f" {parity_sdk} "
    parity_payload["sdk_results"][parity_padded_sdk] = parity_payload[
        "sdk_results"
    ].pop(parity_sdk)
    secret_padded_sdk = " secret-token-sdk "
    parity_payload["sdk_results"][secret_padded_sdk] = next(
        iter(parity_payload["sdk_results"].values())
    )
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_sdk = sorted(self_test_payload["sdk_results"])[-1]
    self_test_padded_sdk = f" {self_test_sdk} "
    self_test_payload["sdk_results"][self_test_padded_sdk] = self_test_payload[
        "sdk_results"
    ].pop(self_test_sdk)
    self_test_payload["sdk_results"][secret_padded_sdk] = next(
        iter(self_test_payload["sdk_results"].values())
    )
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results sdk key must not contain surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        f"sdk_results missing sdk: {parity_sdk}"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "sdk_results sdk key must not contain surrounding whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        f"sdk_results missing sdk: {self_test_sdk}"
    ) in blockers
    assert secret_padded_sdk.strip() not in "\n".join(blockers)
    assert secret_padded_sdk.strip() not in completed.stdout
    assert secret_padded_sdk.strip() not in completed.stderr
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_malformed_sdk_result_keys(
    tmp_path: Path,
) -> None:
    """Fixture SDK result keys must reject unsafe and non-canonical spellings."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_keys = sorted(parity_payload["sdk_results"])
    parity_payload["sdk_results"]["java script"] = parity_payload[
        "sdk_results"
    ].pop(parity_keys[0])
    parity_payload["sdk_results"]["swift\nsdk"] = parity_payload[
        "sdk_results"
    ].pop(parity_keys[1])
    parity_payload["sdk_results"]["kotlin_sdk"] = parity_payload[
        "sdk_results"
    ].pop(parity_keys[2])
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_keys = sorted(self_test_payload["sdk_results"])
    confusable_sdk = "javas\u0441ript"
    self_test_payload["sdk_results"][confusable_sdk] = self_test_payload[
        "sdk_results"
    ].pop(self_test_keys[0])
    self_test_payload["sdk_results"]["-swift"] = self_test_payload[
        "sdk_results"
    ].pop(self_test_keys[1])
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results sdk key must not contain whitespace"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results sdk key contains control character"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "sdk_results sdk key must be a lowercase SDK id"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "sdk_results sdk key must be printable ASCII"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "sdk_results sdk key must be a lowercase SDK id"
    ) in blockers
    assert confusable_sdk not in "\n".join(blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_fixture_public_signal_shape(
    tmp_path: Path,
) -> None:
    """Rehashed fixture vectors must keep canonical public signal words."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))

    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_payload["public_signal_words"] = parity_payload["public_signal_words"][:-1]
    for result in parity_payload["sdk_results"].values():
        result["public_signal_words"] = parity_payload["public_signal_words"]
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )

    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_payload["public_signal_words"][0] = "0x" + "zz" * 32
    for result in self_test_payload["sdk_results"].values():
        result["public_signal_words"] = self_test_payload["public_signal_words"]
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )

    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "public_signal_words must contain 9 words"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "public_signal_words[0] must be a canonical 32-byte hex value"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_parity_fixture_role_reuse(
    tmp_path: Path,
) -> None:
    """Parity fixture proof hashes must remain semantically role-separated."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    parity_path = tmp_path / payload["cross_sdk_fixture_parity_artifact"]
    parity_payload = json.loads(parity_path.read_text(encoding="utf-8"))
    parity_payload["source_proof_hash"] = parity_payload["receipt_proof_hash"]
    for result in parity_payload["sdk_results"].values():
        result["source_proof_hash"] = parity_payload["source_proof_hash"]
    parity_bytes = (
        json.dumps(parity_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parity_path.write_bytes(parity_bytes)
    payload["audit_hashes"]["cross_sdk_fixture_parity"] = (
        "0x" + hashlib.sha256(parity_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "source_proof_hash must not reuse receipt_proof_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_self_test_role_reuse(
    tmp_path: Path,
) -> None:
    """Native prover self-tests must not collapse distinct proof hash roles."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    self_test_path = tmp_path / payload["native_prover_self_test_artifact"]
    self_test_payload = json.loads(self_test_path.read_text(encoding="utf-8"))
    self_test_payload["proof_hash"] = self_test_payload["source_proof_hash"]
    for result in self_test_payload["sdk_results"].values():
        result["proof_hash"] = self_test_payload["proof_hash"]
    self_test_bytes = (
        json.dumps(self_test_payload, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    self_test_path.write_bytes(self_test_bytes)
    payload["audit_hashes"]["native_prover_self_test"] = (
        "0x" + hashlib.sha256(self_test_bytes).hexdigest()
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "proof_hash must not reuse source_proof_hash"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_forbidden_payload_marker(
    tmp_path: Path,
) -> None:
    """Native prover readiness must reject WASM/remote-prover markers in payloads."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    marker_payload = b"native proof artifact imports proof.wasm\n"
    proof_path = tmp_path / "native-prover-artifacts" / "proof-artifact.bin"
    proof_path.write_bytes(marker_payload)
    proof_hash = "0x" + hashlib.sha256(marker_payload).hexdigest()
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact_hash"] = proof_hash
    for artifact in payload["native_sdk_artifacts"]:
        artifact["prover_artifact_hash"] = proof_hash
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact contains forbidden "
        "prover dependency marker: wasm"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_path_escape(
    tmp_path: Path,
) -> None:
    """Native prover artifact paths must stay under the manifest directory."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(
        tmp_path,
        evidence,
        overrides={"proof_artifact": "../proof-artifact.bin"},
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact path must be relative "
        "and stay under the manifest directory"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_uri_scheme_path(
    tmp_path: Path,
) -> None:
    """Native prover artifact paths must not smuggle URI-like sources."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(
        tmp_path,
        evidence,
        overrides={"proof_artifact": "ipfs:proof-artifact.bin"},
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact path must not contain "
        "URI schemes or drive prefixes"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_percent_encoded_path(
    tmp_path: Path,
) -> None:
    """Native prover artifact paths must not smuggle encoded traversal."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(
        tmp_path,
        evidence,
        overrides={
            "proof_artifact": "native-prover-artifacts/%252e%252e/secret-token.bin"
        },
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact path contains "
        "percent-encoded traversal segment"
    ) in blockers
    assert "secret-token" not in "\n".join(blockers)
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_native_evm_prover_forbidden_path_marker(
    tmp_path: Path,
) -> None:
    """Native prover artifact filenames must not advertise WASM or remote proving."""

    evidence, _ = write_active_launch_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(
        tmp_path,
        evidence,
        overrides={"proof_artifact": "native-prover-artifacts/proof.wasm"},
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    blockers = payload["native_evm_prover_bundle"]["validation_blockers"]
    assert (
        "native EVM Groth16 prover bundle proof_artifact path contains "
        "forbidden prover dependency marker: wasm"
    ) in blockers
    assert payload["release_checklist"]["ready"] is False


def test_release_readiness_report_blocks_active_launch_evm_live_metadata_drift(
    tmp_path: Path,
) -> None:
    """Active Ethereum launch readiness must surface mainnet/finalized live-read drift."""

    evidence, evidence_payload = write_active_launch_evidence(tmp_path)
    replacements = (
        (
            '# sccp_evm_source_rpc_chain_id = "1"',
            '# sccp_evm_source_rpc_chain_id = "2"',
        ),
        (
            '# sccp_evm_source_block_tag = "finalized"',
            '# sccp_evm_source_block_tag = "latest"',
        ),
        ('# sccp_evm_rpc_chain_id = "1"', '# sccp_evm_rpc_chain_id = "2"'),
        ('# sccp_evm_block_tag = "finalized"', '# sccp_evm_block_tag = "latest"'),
    )
    for expected, replacement in replacements:
        assert expected in evidence_payload
        evidence_payload = evidence_payload.replace(expected, replacement, 1)
    evidence.write_text(evidence_payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    checklist = {
        item["id"]: item for item in payload["release_checklist"]["items"]
    }
    governed = checklist["governed_deployment_evidence"]
    assert governed["ready"] is False
    assert (
        "domain 1 (eth): Ethereum mainnet source live eth_chainId must be "
        "canonical decimal chain id 1"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet destination live eth_chainId must be "
        "canonical decimal chain id 1"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet source live block tag must be finalized"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet destination live block tag must be finalized"
        in governed["blockers"]
    )


def test_release_readiness_report_accepts_phase_evidence_dir(
    tmp_path: Path,
) -> None:
    """Strict reports can bind downloaded per-phase corridor log artifacts."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    phase_artifacts = write_downloaded_phase_artifacts(tmp_path)
    js_log = (
        phase_artifacts
        / "sccp-production-corridor-js-sdk"
        / "js-sdk.log"
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(phase_artifacts),
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    assert "Status: READY" in completed.stdout
    assert f"| `js-sdk` | passed | `{js_log}` |" in completed.stdout
    assert "## Blocking Items\n\n- None" in completed.stdout


def test_release_readiness_report_suppresses_missing_phase_evidence_dir_path(
    tmp_path: Path,
) -> None:
    """Missing phase evidence dir diagnostics must not echo local paths."""

    evidence, _ = write_complete_evidence(tmp_path)
    phase_artifacts = tmp_path / "secret-token-phase-artifacts"
    phase_artifacts.mkdir()

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(phase_artifacts),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode != 0
    assert "missing SCCP corridor evidence log for phase" in completed.stderr
    assert "checked standard phase log layouts" in completed.stderr
    assert "secret-token" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_requires_contract_smoke_node_success_evidence(
    tmp_path: Path,
) -> None:
    """Contract-smoke evidence must prove the deploy/config Node tests passed."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    report = load_report_module()
    for index, omitted_marker in enumerate(report.CONTRACT_SMOKE_NODE_SUCCESS_FRAGMENTS):
        success_markers = [
            marker
            for marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["contract-smoke"]
            if marker != omitted_marker
        ]
        corridor_log = tmp_path / f"contract-smoke-without-node-success-{index}.log"
        corridor_log.write_text(
            "\n".join(
                (
                    "==> SCCP production corridor: contract-smoke",
                    *phase_command_lines(
                        report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
                    ),
                    *success_markers,
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--require-phase-evidence",
                "--phase-result",
                "all=missing",
                "--phase-result",
                "contract-smoke=passed",
                "--phase-evidence",
                f"contract-smoke={corridor_log}",
                "--native-evm-prover-bundle",
                str(native_bundle),
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        assert "Status: NOT READY" in completed.stdout
        assert (
            "production corridor phase contract-smoke evidence artifact is missing "
            f"expected phase-block success marker: {omitted_marker}"
        ) in completed.stdout


def test_release_readiness_report_rejects_duplicate_phase_evidence_assignment(
    tmp_path: Path,
) -> None:
    """Explicit phase evidence must not overwrite an earlier assignment."""

    evidence, _ = write_complete_evidence(tmp_path)
    first_log = tmp_path / "rust-sccp-first.log"
    second_log = tmp_path / "rust-sccp-second.log"
    first_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")
    second_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={first_log}",
            "--phase-evidence",
            f"rust-sccp={second_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "duplicate SCCP corridor phase evidence for rust-sccp"
    ) in completed.stderr
    assert "already set by --phase-evidence rust-sccp=" in completed.stderr
    assert "cannot set from --phase-evidence rust-sccp=" in completed.stderr


def test_release_readiness_report_suppresses_duplicate_phase_evidence_paths(
    tmp_path: Path,
) -> None:
    """Duplicate phase-evidence diagnostics must not echo local paths."""

    evidence, _ = write_complete_evidence(tmp_path)
    first_log = tmp_path / "phase-evidence-secret-token-first.log"
    second_log = tmp_path / "phase-evidence-secret-token-second.log"
    first_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")
    second_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={first_log}",
            "--phase-evidence",
            f"rust-sccp={second_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "duplicate SCCP corridor phase evidence for rust-sccp"
    ) in completed.stderr
    assert "already set by --phase-evidence rust-sccp=<path>" in completed.stderr
    assert "cannot set from --phase-evidence rust-sccp=<path>" in completed.stderr
    assert "secret-token" not in completed.stderr


def test_release_readiness_report_rejects_padded_phase_result_name(
    tmp_path: Path,
) -> None:
    """Phase-result names must not be trim-normalized before readiness state."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            " rust-sccp =passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result name contains surrounding whitespace" in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_suppresses_phase_result_syntax_input(
    tmp_path: Path,
) -> None:
    """Phase-result syntax errors must not echo table-breaking operator input."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "rust|sccp",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result must use NAME=STATUS syntax" in completed.stderr
    assert "rust|sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_markdown_phase_result_name(
    tmp_path: Path,
) -> None:
    """Phase-result names must not echo table-breaking operator input."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "rust|sccp=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result name contains Markdown-unsafe character" in completed.stderr
    assert "rust|sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_malformed_phase_result_name(
    tmp_path: Path,
) -> None:
    """Phase-result names must fail closed before unknown-phase lookup."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "Rust-sccp=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result name contains malformed phase" in completed.stderr
    assert "Rust-sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_suppresses_unknown_phase_result_name(
    tmp_path: Path,
) -> None:
    """Unknown phase-result names must not echo operator-supplied strings."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "secret-token=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result name contains sensitive name" in completed.stderr
    assert "secret-token" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_padded_phase_result_status(
    tmp_path: Path,
) -> None:
    """Phase-result statuses must not be trim-normalized before readiness state."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "rust-sccp= passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase result status contains surrounding whitespace" in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_malformed_phase_result_status_values(
    tmp_path: Path,
) -> None:
    """Malformed phase-result statuses must be classified without echoing input."""

    evidence, _ = write_complete_evidence(tmp_path)
    cases = (
        ("rust-sccp=", "phase result status is empty", ""),
        (
            "rust-sccp=pass\x07ed",
            "phase result status contains control character",
            "pass\x07ed",
        ),
        (
            "rust-sccp=passéd",
            "phase result status contains non-ASCII character",
            "passéd",
        ),
        (
            "rust-sccp=pass ed",
            "phase result status contains whitespace",
            "pass ed",
        ),
    )
    for assignment, expected_error, leaked_value in cases:
        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--phase-result",
                assignment,
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 2
        assert expected_error in completed.stderr
        if leaked_value:
            assert leaked_value not in completed.stderr
        assert "Status:" not in completed.stdout


def test_release_readiness_report_suppresses_unknown_phase_result_status(
    tmp_path: Path,
) -> None:
    """Unknown phase-result statuses must not echo unsafe operator input."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--phase-result",
            "rust-sccp=passed|failed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "phase result status must be passed, failed, skipped, or missing"
        in completed.stderr
    )
    assert "passed|failed" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_padded_phase_evidence_name(
    tmp_path: Path,
) -> None:
    """Phase-evidence names must not be trim-normalized before artifact hashing."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "rust-sccp.log"
    corridor_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f" rust-sccp ={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence name contains surrounding whitespace" in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_suppresses_phase_evidence_syntax_input(
    tmp_path: Path,
) -> None:
    """Phase-evidence syntax errors must not echo table-breaking operator input."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            "rust|sccp",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence must use NAME=PATH syntax" in completed.stderr
    assert "rust|sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_empty_phase_evidence_path(
    tmp_path: Path,
) -> None:
    """Phase-evidence paths must be explicit before artifact hashing."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            "rust-sccp=",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence path must not be empty" in completed.stderr
    assert "rust-sccp=" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_markdown_phase_evidence_name(
    tmp_path: Path,
) -> None:
    """Phase-evidence names must not echo table-breaking operator input."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "rust-sccp.log"
    corridor_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust|sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence name contains Markdown-unsafe character" in completed.stderr
    assert "rust|sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_malformed_phase_evidence_name(
    tmp_path: Path,
) -> None:
    """Phase-evidence names must fail closed before artifact hashing."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "rust-sccp.log"
    corridor_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust_sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence name contains malformed phase" in completed.stderr
    assert "rust_sccp" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_suppresses_unknown_phase_evidence_name(
    tmp_path: Path,
) -> None:
    """Unknown phase-evidence names must not echo operator-supplied strings."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "rust-sccp.log"
    corridor_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"secret-token={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "phase evidence name contains sensitive name" in completed.stderr
    assert "secret-token" not in completed.stderr
    assert "Status:" not in completed.stdout


def test_release_readiness_report_rejects_phase_evidence_dir_override(
    tmp_path: Path,
) -> None:
    """Explicit evidence must not replace a downloaded phase artifact."""

    evidence, _ = write_complete_evidence(tmp_path)
    phase_artifacts = write_downloaded_phase_artifacts(tmp_path)
    override_log = tmp_path / "rust-sccp-override.log"
    override_log.write_text(complete_corridor_log(("rust-sccp",)), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence-dir",
            str(phase_artifacts),
            "--phase-evidence",
            f"rust-sccp={override_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "duplicate SCCP corridor phase evidence for rust-sccp"
    ) in completed.stderr
    assert "already set by --phase-evidence-dir" in completed.stderr
    assert "cannot set from --phase-evidence rust-sccp=" in completed.stderr


def test_release_readiness_report_rejects_forged_phase_log(
    tmp_path: Path,
) -> None:
    """A hashed phase artifact must be an actual corridor transcript."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-corridor.log"
    corridor_log.write_text("SCCP production corridor completed.\n", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_output_before_phase_marker(
    tmp_path: Path,
) -> None:
    """Failure output before the first phase marker cannot be hidden."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-prefix-output.log"
    corridor_log.write_text(
        "\n".join(
            (
                "1 failed before phase marker",
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "non-empty output before first phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_prefix_alias_phase_marker(
    tmp_path: Path,
) -> None:
    """A phase marker must match the claimed phase name exactly."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-prefix-alias.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp-forged",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_duplicate_phase_marker(
    tmp_path: Path,
) -> None:
    """A phase artifact cannot hide a duplicate claimed phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-duplicate-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "==> SCCP production corridor: rust-sccp",
                "test result: FAILED. 1 failed",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact has duplicate "
        "phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_prefix_marker_hidden_failure(
    tmp_path: Path,
) -> None:
    """A prefix-like marker line cannot truncate a passed block before failure output."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    failure_marker = report.PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS[
        "rust-sccp"
    ][0].pattern
    corridor_log = tmp_path / "forged-rust-sccp-prefix-hidden-failure.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "==> SCCP production corridor: rust-sccp-forged",
                "test result: FAILED. 1 failed",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        f"forbidden phase-block failure marker: {failure_marker}"
    ) in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "unknown corridor phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_partial_multi_phase_hidden_failure(
    tmp_path: Path,
) -> None:
    """A partial multi-phase transcript cannot pass as one complete phase."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-partial-multiphase.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "==> SCCP production corridor: js-sdk",
                "fail 1",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "incomplete multi-phase corridor transcript"
    ) in completed.stdout


def test_release_readiness_report_rejects_out_of_order_full_corridor_log(
    tmp_path: Path,
) -> None:
    """A full-corridor transcript must keep the runner's canonical phase order."""

    evidence, _ = write_complete_evidence(tmp_path)
    shuffled_phases = (PHASES[1], PHASES[0], *PHASES[2:])
    corridor_log = tmp_path / "forged-rust-sccp-out-of-order-full-corridor.log"
    corridor_log.write_text(complete_corridor_log(shuffled_phases), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "incomplete multi-phase corridor transcript"
    ) in completed.stdout


def test_release_readiness_report_rejects_full_corridor_success_before_command(
    tmp_path: Path,
) -> None:
    """Full-corridor transcripts must order each phase's success after command."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-rust-sccp-full-corridor-early-success.log"
    corridor_log.write_text(
        complete_corridor_log_with_success_before_command("rust-sccp"),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "incomplete multi-phase corridor transcript"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_expected_command(
    tmp_path: Path,
) -> None:
    """A phase artifact must contain the command for the claimed corridor phase."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-rust-sccp.log"
    corridor_log.write_text(
        "==> SCCP production corridor: rust-sccp\n"
        "phase rust-sccp passed\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout

def test_release_readiness_rejects_java_android_log_without_source_harness(
    tmp_path: Path,
) -> None:
    """The Android phase log must include source-proof harness selection."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "java-android-without-source-harness.log"
    source_harness = "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"
    corridor_log.write_text(
        "==> SCCP production corridor: java-android\n"
        "+ ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests\n"
        "BUILD SUCCESSFUL\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"java-android={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase java-android evidence artifact is missing "
        f"expected phase-block command: {source_harness}"
    ) in completed.stdout


def test_release_readiness_rejects_java_android_log_without_ton_harness(
    tmp_path: Path,
) -> None:
    """The Android phase log must include the TON proof-request harness."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "java-android-without-ton-harness.log"
    ton_harness = "org.hyperledger.iroha.android.sccp.TonSccpProverTests"
    corridor_log.write_text(
        "==> SCCP production corridor: java-android\n"
        "+ ANDROID_HARNESS_MAINS="
        "org.hyperledger.iroha.android.sccp.EvmSccpProverTests,"
        "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests,"
        "org.hyperledger.iroha.android.sccp.TronSccpProverTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests\n"
        "BUILD SUCCESSFUL\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"java-android={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase java-android evidence artifact is missing "
        f"expected phase-block command: {ton_harness}"
    ) in completed.stdout


def test_release_readiness_rejects_kotlin_log_without_ton_prover_test(
    tmp_path: Path,
) -> None:
    """The Kotlin phase log must include the TON proof-request test selector."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "kotlin-without-ton-prover-test.log"
    ton_test = "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest"
    corridor_log.write_text(
        "==> SCCP production corridor: kotlin-sdk\n"
        "+ java -version\n"
        "+ ./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp.*\n"
        'openjdk version "21.0.1"\n'
        "BUILD SUCCESSFUL\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"kotlin-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase kotlin-sdk evidence artifact is missing "
        f"expected phase-block command: {ton_test}"
    ) in completed.stdout


def test_release_readiness_report_requires_release_verifier_tests_in_evidence_phase(
    tmp_path: Path,
) -> None:
    """The evidence phase must prove release readiness and bundle verifiers ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_verifier_tests = (
        "pytests/scripts/sccp_release_bundle_test.py",
        "pytests/scripts/sccp_release_readiness_report_test.py",
    )
    for omitted in required_verifier_tests:
        assert omitted in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        required_fragments = [
            fragment
            for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
            if fragment != omitted
        ]
        corridor_log = tmp_path / f"evidence-scripts-without-{Path(omitted).stem}.log"
        corridor_log.write_text(
            "\n".join(
                (
                    "==> SCCP production corridor: evidence-scripts",
                    *phase_command_lines(required_fragments),
                    *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--require-phase-evidence",
                "--phase-result",
                "all=missing",
                "--phase-result",
                "evidence-scripts=passed",
                "--phase-evidence",
                f"evidence-scripts={corridor_log}",
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        assert "Status: NOT READY" in completed.stdout
        assert (
            "production corridor phase evidence-scripts evidence artifact is missing "
            f"expected phase-block command: {omitted}"
        ) in completed.stdout


def test_release_readiness_report_requires_retired_network_scan_evidence(
    tmp_path: Path,
) -> None:
    """The evidence phase must prove the retired-network surface scan ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_retired_network_surface_test.py"
    assert omitted_fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "evidence-scripts-without-retired-network-scan.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_output_only_retired_network_scan_evidence(
    tmp_path: Path,
) -> None:
    """The retired-network scan must be listed as a traced corridor command."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_retired_network_surface_test.py"
    assert omitted_fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "evidence-scripts-retired-network-output-only.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                omitted_fragment,
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_echoed_retired_network_scan_command(
    tmp_path: Path,
) -> None:
    """The retired-network scan path must appear on the pytest command line."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_retired_network_surface_test.py"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "evidence-scripts-retired-network-echo.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                f"+ echo {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_phase_completion(
    tmp_path: Path,
) -> None:
    """A phase artifact must prove completion in the claimed phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-no-phase-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "==> SCCP production corridor: js-sdk",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_command_line_only_completion_marker(
    tmp_path: Path,
) -> None:
    """A traced echo must not satisfy the phase completion marker."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-echoed-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "+ echo SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_nonexact_completion_marker(
    tmp_path: Path,
) -> None:
    """Completion must be the exact output line, not a substring."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-substring-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "not actually SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_completion_before_phase_evidence(
    tmp_path: Path,
) -> None:
    """Completion must appear after the commands and success output it certifies."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-early-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                "SCCP production corridor completed.",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact completion "
        "sentinel precedes required phase evidence"
    ) in completed.stdout


def test_release_readiness_report_rejects_success_marker_before_phase_command(
    tmp_path: Path,
) -> None:
    """Success markers must appear after the phase command they certify."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-early-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact completion "
        "sentinel precedes required phase evidence"
    ) in completed.stdout


def test_release_readiness_report_rejects_command_after_completion(
    tmp_path: Path,
) -> None:
    """A completed phase block cannot contain later traced commands."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-command-after-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "+ cargo test -p iroha_sccp -- --nocapture",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains traced "
        "command after completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_output_after_completion(
    tmp_path: Path,
) -> None:
    """Completion must be the final non-empty output in a phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-output-after-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "still writing output after completion",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact contains "
        "non-empty output after completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_command_line_only_full_completion_marker(
    tmp_path: Path,
) -> None:
    """A full-corridor completion fallback must be observed output."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    lines: list[str] = []
    for phase in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS:
        lines.append(f"==> SCCP production corridor: {phase}")
        if phase == "rust-sccp":
            lines.extend(
                phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                )
            )
            lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"])
    lines.extend(("+ echo SCCP production corridor completed.", ""))
    corridor_log = tmp_path / "forged-rust-sccp-echoed-full-completion.log"
    corridor_log.write_text("\n".join(lines), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_marker_only_full_corridor_completion(
    tmp_path: Path,
) -> None:
    """Full-corridor fallback must prove every phase block, not only markers."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    lines: list[str] = []
    for phase in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS:
        lines.append(f"==> SCCP production corridor: {phase}")
        if phase == "rust-sccp":
            lines.extend(
                phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                )
            )
            lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"])
    lines.extend(("SCCP production corridor completed.", ""))
    corridor_log = tmp_path / "forged-rust-sccp-marker-only-full-completion.log"
    corridor_log.write_text("\n".join(lines), encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_command_line_only_success_marker(
    tmp_path: Path,
) -> None:
    """Success markers must come from output, not traced echo commands."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    success_marker = report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-echoed-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                f"+ echo {success_marker}",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block success marker: {success_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_xtrace_success_marker(
    tmp_path: Path,
) -> None:
    """Nested shell xtrace output must not satisfy success evidence."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    success_marker = report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-xtrace-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                f"++ echo {success_marker}",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block success marker: {success_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_obfuscated_xtrace_success_marker(
    tmp_path: Path,
) -> None:
    """Control-obfuscated nested xtrace must not satisfy success evidence."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    success_marker = report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-obfuscated-xtrace-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                f"\x1b[32m+\u200c+ echo {success_marker}\x1b[0m",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block success marker: {success_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_negated_success_marker(
    tmp_path: Path,
) -> None:
    """Success marker text in a negated output line must not satisfy evidence."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    success_marker = report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-negated-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                f"not {success_marker}",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block success marker: {success_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_diagnostic_success_marker(
    tmp_path: Path,
) -> None:
    """Diagnostic prose that names a success marker must not satisfy evidence."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    success_marker = report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-diagnostic-success.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                f"diagnostic output contains {success_marker}",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block success marker: {success_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_failure_output_marker(
    tmp_path: Path,
) -> None:
    """A phase log cannot pass by mixing success output with failure counts."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    failure_marker = report.PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS[
        "evidence-scripts"
    ][0].pattern
    corridor_log = tmp_path / "forged-evidence-scripts-with-failures.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
                ),
                "1 failed, 9 passed in 0.42s",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact contains "
        f"forbidden phase-block failure marker: {failure_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_ansi_obfuscated_failure_output(
    tmp_path: Path,
) -> None:
    """Terminal-control-obfuscated failure markers must still block readiness."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    failure_marker = report.PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS[
        "evidence-scripts"
    ][0].pattern
    corridor_log = tmp_path / "forged-evidence-scripts-with-ansi-failure.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
                ),
                "1 \x1b[31mfa\x08iled\x1b[0m, 9 passed in 0.42s",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact contains "
        f"forbidden phase-block failure marker: {failure_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_unicode_format_obfuscated_failure_output(
    tmp_path: Path,
) -> None:
    """Zero-width/bidi-obfuscated failure markers must block readiness."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    failure_marker = report.PHASE_TRANSCRIPT_FORBIDDEN_OUTPUT_PATTERNS[
        "evidence-scripts"
    ][0].pattern
    corridor_log = tmp_path / "forged-evidence-scripts-with-unicode-failure.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
                ),
                "1 fa\u200c\u2066iled, 9 passed in 0.42s",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact contains "
        f"forbidden phase-block failure marker: {failure_marker}"
    ) in completed.stdout


def test_release_readiness_report_rejects_output_only_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required command fragments must come from traced corridor command lines."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-output-only.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"],
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout


def test_release_readiness_report_rejects_bare_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """A bare traced file path must not prove the phase command executed."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_release_readiness_report_test.py"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-evidence-scripts-bare-fragment.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                f"+ {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_short_circuited_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required phase commands must not be hidden behind a failing prefix."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-short-circuited-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                f"+ false && {required_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_comment_only_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required phase commands must not be hidden behind shell comments."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_release_bundle_test.py"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-evidence-scripts-comment-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                "+ python3 -m pytest -q "
                "pytests/scripts/check_sccp_production_corridor_test.py "
                f"# {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase evidence-scripts evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_inert_option_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required test selectors must not be hidden in inert command options."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["kotlin-sdk"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-kotlin-sdk-inert-option-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: kotlin-sdk",
                *phase_command_lines(required_fragments),
                "+ ./gradlew :core-jvm:test --console=plain "
                "--tests org.hyperledger.iroha.sdk.sccp.OtherTest "
                f"--info {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["kotlin-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "kotlin-sdk=passed",
            "--phase-evidence",
            f"kotlin-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase kotlin-sdk evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_prefix_android_harness_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Android harness classes must match exact comma-delimited class names."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["java-android"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-java-android-prefix-harness-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: java-android",
                *phase_command_lines(required_fragments),
                "+ env ANDROID_HARNESS_MAINS="
                f"{omitted_fragment}Extra ./gradlew :core:test --console=plain "
                "--tests org.hyperledger.iroha.android.GradleHarnessTests",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["java-android"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "java-android=passed",
            "--phase-evidence",
            f"java-android={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase java-android evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_narrow_kotlin_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """The Kotlin phase must prove the broad package selector ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = (
        "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp."
    )
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["kotlin-sdk"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-kotlin-sdk-narrow-selector-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: kotlin-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["kotlin-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "kotlin-sdk=passed",
            "--phase-evidence",
            f"kotlin-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase kotlin-sdk evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_gradle_dry_run_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Gradle dry-run output must not satisfy Kotlin phase evidence."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = (
        "./gradlew :core-jvm:test --console=plain --tests org.hyperledger.iroha.sdk.sccp."
    )
    ton_fragment = "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest"
    corridor_log = tmp_path / "forged-kotlin-sdk-gradle-dry-run-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: kotlin-sdk",
                "+ java -version",
                "+ ./gradlew :core-jvm:test --console=plain "
                "--tests org.hyperledger.iroha.sdk.sccp.* "
                f"--tests {ton_fragment} --dry-run",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["kotlin-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "kotlin-sdk=passed",
            "--phase-evidence",
            f"kotlin-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase kotlin-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_pytest_suffix_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Pytest phase evidence must not carry mutating suffix options."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["python-sdk"][0]
    corridor_log = tmp_path / "forged-python-sdk-pytest-suffix-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: python-sdk",
                "+ python3 -m pytest -q python/iroha_torii_client/tests/sccp_test.py "
                "--ignore python/iroha_torii_client/tests/sccp_test.py",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["python-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "python-sdk=passed",
            "--phase-evidence",
            f"python-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase python-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_pytest_extra_positional_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Pytest phase evidence must run the exact expected positional files."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["python-sdk"][0]
    corridor_log = tmp_path / "forged-python-sdk-pytest-extra-positional-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: python-sdk",
                "+ python3 -m pytest -q python/iroha_torii_client/tests/sccp_test.py "
                "pytests/scripts/sccp_release_bundle_test.py",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["python-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "python-sdk=passed",
            "--phase-evidence",
            f"python-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase python-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_node_extra_positional_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Node phase evidence must run exactly the expected SCCP test files."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"][0]
    corridor_log = tmp_path / "forged-js-sdk-extra-positional-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                "+ node --test javascript/iroha_js/test/sccpSolanaProver.test.js "
                "javascript/iroha_js/test/sccpEthereumMainnet.test.js "
                "javascript/iroha_js/test/sccpBscMainnet.test.js "
                "javascript/iroha_js/test/package_dist.test.js "
                "javascript/iroha_js/test/sccpPackageExports.test.js "
                "javascript/iroha_js/test/unscopedExtra.test.js",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_dotnet_suffix_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """.NET phase evidence must not carry suffix options or control tokens."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"][0]
    filter_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"][1]
    corridor_log = tmp_path / "forged-dotnet-sdk-suffix-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: dotnet-sdk",
                "+ dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj "
                f"--filter {filter_fragment} --nologo --logger trx",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["dotnet-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "dotnet-sdk=passed",
            "--phase-evidence",
            f"dotnet-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase dotnet-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_inert_swift_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Swift phase evidence must run the exact expected filter command."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "swift test --filter SccpSolanaProverTests --disable-swift-testing"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["swift-sdk"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-swift-sdk-inert-filter-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: swift-sdk",
                *phase_command_lines(required_fragments),
                "+ swift test --filter OtherTests --skip swift test --filter "
                "SccpSolanaProverTests --disable-swift-testing",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["swift-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "swift-sdk=passed",
            "--phase-evidence",
            f"swift-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase swift-sdk evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_suffix_argument_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required exact commands must not be followed by mutating suffix args."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"][0]
    corridor_log = tmp_path / "forged-rust-sccp-suffix-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                f"+ {required_fragment} --skip sccp",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_inert_dotnet_project_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """The .NET phase must run the expected test project, not log its path."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"][0]
    filter_fragment = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["dotnet-sdk"][1]
    corridor_log = tmp_path / "forged-dotnet-sdk-inert-project-command.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: dotnet-sdk",
                "+ dotnet test tests/Other/Other.csproj "
                f"--filter {filter_fragment} --logger {required_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["dotnet-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "dotnet-sdk=passed",
            "--phase-evidence",
            f"dotnet-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase dotnet-sdk evidence artifact is missing "
        f"expected phase-block command: {required_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_inert_contract_smoke_test_fragment(
    tmp_path: Path,
) -> None:
    """Contract smoke Node tests must be positional test files."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "scripts/sccp_bsc_taira_xor_deploy.test.mjs"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "forged-contract-smoke-inert-node-test.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke",
                *phase_command_lines(required_fragments),
                "+ node --test scripts/sccp_taira_xor_contract.test.mjs "
                f"--test-reporter {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["contract-smoke"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "contract-smoke=passed",
            "--phase-evidence",
            f"contract-smoke={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase contract-smoke evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_rejects_echoed_js_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """The JS phase must prove required tests came from the node test command."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "javascript/iroha_js/test/sccpPackageExports.test.js"
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "js-sdk-echoed-package-exports.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                f"+ echo {omitted_fragment}",
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


def test_release_readiness_report_requires_mobile_jdk21_transcripts(
    tmp_path: Path,
) -> None:
    """Mobile SDK phase evidence must prove the runner used JDK 21."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    jdk21_marker = 'version "21'

    for phase in ("kotlin-sdk", "java-android"):
        assert "java -version" in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        assert jdk21_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase]
        success_fragments = [
            fragment
            for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase]
            if fragment != jdk21_marker
        ]
        corridor_log = tmp_path / f"{phase}-without-jdk21-version.log"
        corridor_log.write_text(
            "\n".join(
                (
                    f"==> SCCP production corridor: {phase}",
                    *phase_command_lines(
                        report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
                    ),
                    *success_fragments,
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--require-phase-evidence",
                "--phase-result",
                "all=missing",
                "--phase-result",
                f"{phase}=passed",
                "--phase-evidence",
                f"{phase}={corridor_log}",
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        assert "Status: NOT READY" in completed.stdout
        assert (
            f"production corridor phase {phase} evidence artifact is missing "
            f"expected phase-block success marker: {jdk21_marker}"
        ) in completed.stdout


def test_release_readiness_report_requires_js_package_dist_transcript(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove source, dist, and package export tests ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != "javascript/iroha_js/test/package_dist.test.js"
    ]
    corridor_log = tmp_path / "js-sdk-without-package-dist.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        "expected phase-block command: javascript/iroha_js/test/package_dist.test.js"
    ) in completed.stdout


def test_release_readiness_report_requires_bsc_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the browser BSC path stayed native JS."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    bsc_no_wasm_marker = (
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert bsc_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != bsc_no_wasm_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-bsc-no-wasm-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {bsc_no_wasm_marker}"
    ) in completed.stdout


def test_release_readiness_report_requires_ethereum_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the browser Ethereum path stayed native JS."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    ethereum_no_wasm_marker = (
        "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert ethereum_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != ethereum_no_wasm_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-ethereum-no-wasm-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {ethereum_no_wasm_marker}"
    ) in completed.stdout


def test_release_readiness_sccp_allow_unready_transparent_proofs_is_config_only() -> None:
    """SCCP unready transparent-proof bypasses must be sourced from TOML config."""

    user_config = ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs"
    service = ROOT / "configs" / "soranexus" / "taira" / "taira-irohad.service"
    bootstrap = (
        ROOT / "configs" / "soranexus" / "taira" / "bootstrap_kaigi_localnet.sh"
    )
    taira_config = ROOT / "configs" / "soranexus" / "taira" / "config.toml"

    for path in (user_config, service, bootstrap):
        assert "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS" not in path.read_text(
            encoding="utf-8"
        )
    assert (
        "pub sccp_allow_unready_transparent_proofs: bool"
        in user_config.read_text(encoding="utf-8")
    )
    assert (
        "sccp_allow_unready_transparent_proofs = true"
        in bootstrap.read_text(encoding="utf-8")
    )
    assert (
        "sccp_allow_unready_transparent_proofs = false"
        in taira_config.read_text(encoding="utf-8")
    )


def test_release_readiness_report_requires_bsc_parlia_declaration_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the BSC Parlia declarations were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    declaration_marker = (
        "package declarations expose BSC mainnet Parlia finality evidence hooks"
    )
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-bsc-parlia-declaration-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {declaration_marker}"
    ) in completed.stdout


def test_release_readiness_report_requires_ethereum_facade_declaration_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the Ethereum facade declarations were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    declaration_marker = "package declarations expose Ethereum mainnet SCCP facade methods"
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-ethereum-facade-declaration-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {declaration_marker}"
    ) in completed.stdout


def test_release_readiness_report_requires_js_package_export_transcript(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove package-root SCCP helpers were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_export_test = "javascript/iroha_js/test/sccpPackageExports.test.js"
    assert required_export_test in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != required_export_test
    ]
    corridor_log = tmp_path / "js-sdk-without-package-exports.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {required_export_test}"
    ) in completed.stdout


def test_release_readiness_report_requires_js_mainnet_facade_transcripts(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove ETH/BSC mainnet facade tests ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_facade_tests = (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
    )
    for required_facade_test in required_facade_tests:
        assert (
            required_facade_test
            in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        )
        required_fragments = [
            fragment
            for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
            if fragment != required_facade_test
        ]
        corridor_log = tmp_path / f"js-sdk-without-{Path(required_facade_test).stem}.log"
        corridor_log.write_text(
            "\n".join(
                (
                    "==> SCCP production corridor: js-sdk",
                    *phase_command_lines(required_fragments),
                    *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--require-phase-evidence",
                "--phase-result",
                "all=missing",
                "--phase-result",
                "js-sdk=passed",
                "--phase-evidence",
                f"js-sdk={corridor_log}",
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        assert "Status: NOT READY" in completed.stdout
        assert (
            "production corridor phase js-sdk evidence artifact is missing "
            f"expected phase-block command: {required_facade_test}"
        ) in completed.stdout


def test_release_readiness_guards_ethereum_inbound_adversarial_sdk_tests() -> None:
    """Native/browser Ethereum inbound tests must retain adversarial evidence cases."""

    guarded_tests = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
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
        ROOT / "python" / "iroha_torii_client" / "sccp.py": (
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
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
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
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
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
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
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
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
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
    }
    missing: list[str] = []
    for path, markers in guarded_tests.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_outbound_precallback_sdk_tests() -> None:
    """Ethereum outbound facades must reject foreign lanes before callbacks."""

    guarded_tests = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert.equal(outboundProverCalled, false)",
            "ERR_SCCP_ETH_NATIVE_PROVER_ARTIFACTS_UNAVAILABLE",
            "verified native EVM prover artifacts",
            "Ethereum mainnet SCCP outbound from",
            "submittedTxs[3].from",
            "assert.notDeepStrictEqual(",
            "Array.from(callbackPublicInputsBytes),",
            "Array.from(proofResult.bundleBytes),",
            "Array.from(input.bundleBytes),",
            'proofArtifactHash: hex32("91")',
            "proofArtifactHash and provingKeyHash must be supplied together",
            "proofArtifactHash and provingKeyHash must match request",
        ),
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
            "destinationBindingHash must match destinationBinding",
            "outbound_prover_called = False",
            "assert not outbound_prover_called",
            "proof_artifact_hash",
            "proofResult proofArtifactHash and provingKeyHash must be supplied together",
            "proofResult proofArtifactHash and provingKeyHash must match request",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "Ethereum outbound prover callback must not see BSC requests",
            "outboundProverCalled",
            'request.copy(destinationBindingHash = "0x" + "99".repeat(32))',
            'proofArtifactHash = "91".repeat(32)',
            "proofArtifactHash and provingKeyHash",
            "artifactResult.proofArtifactHash",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert !outboundProverCalled[0]",
            "Ethereum wrapProofResult must reject forged destinationBindingHash",
            "evmRequestWithDestinationBindingHash",
            "partial proof artifact metadata must be rejected",
            "zero proof artifact hash must be rejected",
            "proof result must carry proof artifact hash",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "Ethereum outbound prover callback must not see BSC requests",
            "Assert.Null(guardedProver.Request)",
            "request with { DestinationBindingHash = \"0x\" + new string('9', 64) }",
            "ProofArtifactHash = \"0x\" + new string('9', 64)",
            "artifactResult.ProofArtifactHash",
            "proofResult with",
        ),
    }
    missing = []
    for path, markers in guarded_tests.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_root_zero_sdk_tests() -> None:
    """Ethereum SDK receipt-root helpers must reject zero typed MPT roots."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "canonicalEvmReceiptRootMptValue(SCCP_ZERO_HASH_V1)",
            "must not be zero",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "package_dist.test.js": (
            'canonicalEvmReceiptRootMptValue(`0x${"00".repeat(32)}`)',
            "must not be zero",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "public func canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "canonicalEvmReceiptRootMptValue(receiptRoot: zeroHash)",
            "XCTAssertThrowsError",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            "fun canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "assertFailsWith<IllegalArgumentException>",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            "public static byte[] canonicalEvmReceiptRootMptValue(final String receiptRoot)",
            'fields.add(rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot")))',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "SourceSccpProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "expectThrows",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "public static byte[] CanonicalEvmSccpReceiptProofBytes",
            "payload.Write(RpcHexToBytes(executionReceiptsRoot, nameof(executionReceiptsRoot), 32));",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "BuildBytes(executionReceiptsRoot: zeroRoot)",
            "BuildBytes(syncCommitteeRoot: zeroRoot)",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rlp_zero_topic_tests() -> None:
    """Ethereum receipt RLP builders must allow zero log topics."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "`receipt.logs[${index}].topics[${topicIndex}]`",
            "{ nonzero: false }",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "zeroTopicReceiptTrieProof",
            'topics: [hex32("00")]',
        ),
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{log_index}].topics[{topic_index}]\"",
            "nonzero=False",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_accepts_zero_log_topic_in_receipt_rlp",
            '"topics": ["0x" + "00" * 32]',
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            'field: "receipt.logs[\\(index)].topics[\\(topicIndex)]"',
            "nonzero: false",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "zeroTopicProof",
            '"topics": ["0x" + String(repeating: "00", count: 32)]',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            '"receipt.logs[$index].topics[$topicIndex]"',
            "nonzero = false",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "zeroTopicProof",
            '"topics" to listOf("0x" + "00".repeat(32))',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            '"receipt.logs[" + index + "].topics[" + topicIndex + "]"',
            "false,\n                    false)))",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "zeroTopicProof",
            "generic Ethereum receipt RLP must allow zero log topics",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            '$"receipt.logs[{index}].topics[{topicIndex}]"',
            "nonZero: false",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "zeroTopicProof",
            '["topics"] = new object?[] { "0x" + new string(\'0\', 64) }',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rlp_zero_address_tests() -> None:
    """Ethereum receipt RLP builders must allow zero log addresses."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "`receipt.logs[${index}].address`",
            "{ nonzero: false }",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "zeroAddressReceiptTrieProof",
            'address: `0x${"00".repeat(20)}`',
        ),
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{log_index}].address\"",
            "nonzero=False",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_accepts_zero_log_address_in_receipt_rlp",
            '"address": "0x" + "00" * 20',
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            'field: "receipt.logs[\\(index)].address"',
            "nonzero: false",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "zeroAddressProof",
            '"address": "0x" + String(repeating: "00", count: 20)',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            '"receipt.logs[$index].address"',
            "nonzero = false",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "zeroAddressProof",
            '"address" to "0x" + "00".repeat(20)',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            '"receipt.logs[" + index + "].address"',
            "false,\n                          false))",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "zeroAddressProof",
            "generic Ethereum receipt RLP must allow zero log addresses",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            '$"receipt.logs[{index}].address"',
            "nonZero: false",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "zeroAddressProof",
            '["address"] = "0x" + new string(\'0\', 40)',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_context_tests() -> None:
    """Ethereum source-event evidence must bind logs to receipt/block context."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "log_transaction_hash = _rpc_fixed_hex_data(",
            "log_block_hash = _rpc_fixed_hex_data(",
            "log_block_number = _rpc_quantity(",
            "source event log transactionHash does not match receipt",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_source_event_missing_context_fields",
            'for field in ("transactionHash", "blockHash", "blockNumber")',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_mode_tests() -> None:
    """Ethereum source-event evidence must be the default receipt collector mode."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "allow_receipt_only_evidence: bool = False",
            "source_bridge_address is required for SCCP source-event evidence",
            "--allow-receipt-only-evidence",
            '"evidence_mode": (',
            '"source_event_validated": source_event_digest is not None',
            '"receipt_only_evidence": source_event_digest is None',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_requires_explicit_receipt_only_mode_without_source_bridge",
            "test_collect_receipt_proof_allows_explicit_receipt_only_mode",
            "test_cli_requires_source_bridge_or_explicit_receipt_only_mode",
            "test_cli_exposes_explicit_receipt_only_mode",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_zero_digest_tests() -> None:
    """Ethereum source-event evidence must reject zero event digests."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{index}].topics[1]\"",
            "raise RuntimeError(f\"{method} returned zero data\")",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_zero_source_event_digest",
            '"topics": [module.EVM_SOURCE_EVENT_TOPIC, "0x" + "00" * 32]',
            "zero source event digest was accepted",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rpc_duplicate_json_tests() -> None:
    """Ethereum receipt evidence RPC parsing must reject duplicate JSON keys."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "_json_object_without_duplicate_keys",
            "JSON-RPC returned duplicate JSON keys",
            "JSON-RPC {method} failed with HTTP {exc.code}",
            "JSON-RPC {method} request failed",
            "JSON-RPC {method} returned invalid JSON",
            "JSON-RPC {method} returned error response",
            "object_pairs_hook=_json_object_without_duplicate_keys",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "FakeRawResponse",
            "test_collect_receipt_proof_rejects_duplicate_json_rpc_result_keys",
            "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields",
            "test_receipt_json_rpc_redacts_invalid_json_parser_details",
            "test_receipt_json_rpc_redacts_transport_and_error_response_details",
            "secret-token invalid EVM receipt JSON-RPC payload",
            "secret-token",
            "duplicate JSON-RPC result keys were accepted",
            "duplicate JSON receipt fields were accepted",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_block_receipt_transaction_hash_tests() -> None:
    """Ethereum block receipt proof inputs must reject duplicate tx hashes."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "seen_transaction_hashes: set[bytes] = set()",
            'method=f"block receipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_receipt_trie_builder_rejects_duplicate_transaction_hashes",
            'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]',
            "duplicate block receipt transaction hashes were accepted",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "fullReceipt(1, { transactionHash: TX_HASH })",
            "transactionHash values must be unique",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "var seenTransactionHashes = Set<Data>()",
            'field: "blockReceipts[\\(index)].transactionHash"',
            "blockReceipts.transactionHash",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "duplicateHashReceipt",
            '.invalidRlp("blockReceipts.transactionHash")',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            "val seenTransactionHashes = HashSet<String>(receipts.size)",
            '"blockReceipts[$index].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "duplicateHashReceipt",
            "transactionHash values must be unique",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            "final Set<String> seenTransactionHashes = new HashSet<String>();",
            '"blockReceipts[" + index + "].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "duplicateHashReceipt",
            "receipt proof builder must reject duplicate block receipt transaction hashes",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "var seenTransactionHashes = new HashSet<string>(StringComparer.Ordinal);",
            '$"blockReceipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique.",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "duplicateTransactionHashReceipt",
            "transactionHash values must be unique",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_noncanonical_chain_id_tests() -> None:
    """Ethereum mainnet collectors must reject noncanonical eth_chainId values."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            'for (const chainId of ["1", 1, "0x01", "0X1", " 0x1", "0x1 "])',
            "canonical JSON-RPC quantity",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            'let noncanonicalChainIds: [Any] = ["1", "0x01", "0X1", " 0x1", "0x1 ", 1]',
            '.invalidPublicInputs("eth_chainId")',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            'for (chainId in listOf<Any>("1", "0x01", "0X1", " 0x1", "0x1 ", 1L))',
            "EthereumMainnetInboundEvidence(receipt = receipt)",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            'new Object[] {"1", "0x01", "0X1", " 0x1", "0x1 ", Long.valueOf(1L)}',
            "noncanonical eth_chainId RPC",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "foreach (var chainId in new object?[]",
            '"0x01", "1", "0X1", " 0x1", "0x1 ", 1,',
            "ValidateExecutionProviderMainnetAsync",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_noncanonical_chain_id_quantity",
            'for chain_id_result in ("0x01", "0X1", " 0x1", "0x1 ", 1):',
            "rpc_response(chain_id_result)",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_beacon_rest_header_shape_tests() -> None:
    """Beacon REST providers must require finalized-header roots and signature."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "/body_root must be 32 bytes/u",
            "/signature must be 96 bytes/u",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
            'for field in ["parent_root", "state_root", "body_root"]',
            '"\\(label).data.header.message.\\(field)"',
            '"\\(label).data.header.signature"',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            '("parent_root", String(repeating: "01", count: 32))',
            'invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.data.header.signature")',
            'String(repeating: "12", count: 95)',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt": (
            'for (field in listOf("parent_root", "state_root", "body_root"))',
            '"$label.data.header.message.$field"',
            '"$label.data.header.signature"',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            '"parent_root" to "01"',
            '"body_root" to "03"',
            '"12".repeat(95)',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java": (
            'Arrays.asList("parent_root", "state_root", "body_root")',
            'label + ".data.header.message." + field',
            'label + ".data.header.signature"',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            '{"parent_root", "01"}',
            'repeat("12", 95)',
            "Beacon REST provider must reject malformed finalized header signatures",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            'foreach (var field in new[] { "parent_root", "state_root", "body_root" })',
            '"{label}.data.header.message.{field}"',
            '"{label}.data.header.signature"',
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            '("parent_root", "01")',
            'string.Concat(Enumerable.Repeat("12", 95))',
            'Assert.Contains("signature", malformedSignature.Message)',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_beacon_rest_execution_payload_tests() -> None:
    """Beacon REST providers must bind finalized execution payload fields."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
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
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
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
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
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
        ROOT / "javascript" / "iroha_js" / "index.d.ts": (
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
        ROOT / "javascript" / "iroha_js" / "test" / "package_dist.test.js": (
            "syncCommitteeBits\\?: string;",
            "syncCommitteeSignature\\?: string;",
            "syncSignatureSlot\\?: string \\| number \\| bigint;",
            "finalityBranch\\?: readonly string\\[\\];",
            "syncCommitteeParticipation\\?: string \\| number \\| bigint;",
            "readonly finalityBranch\\?: readonly string\\[\\];",
            "readonly syncCommitteeBits\\?: string;",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
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
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt": (
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
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
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
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java": (
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
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
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
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
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
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
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
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_sync_committee_roster_tests() -> None:
    """Ethereum mainnet sync-committee helpers must reject compressed rosters."""

    guarded_sources = {
        ROOT / "crates" / "iroha_sccp" / "src" / "lib.rs": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: usize = 512;",
            ".all(|weight| *weight == 1)",
            "eth_sync_committee_transition_transcript_requires_mainnet_rosters",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "syncCommitteeFixture(0x11, 0xaa)",
            "assert.equal(nextSyncCommitteePayload.length, 81925)",
            "signersBitmap(342)",
        ),
        ROOT / "python" / "iroha_torii_client" / "sccp.py": (
            "_SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[{index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
            "sync_committee_fixture(0x11, 0xAA)",
            "assert len(next_payload) == 81925",
            "signers_bitmap(342)",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "sccpEthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteeWeights[index] == 1",
            "signersBitmap.count == (syncCommitteePublicKeys.count + 7) / 8",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "ethereumSyncCommitteeBytes(_ byte: UInt8, count: Int)",
            "XCTAssertEqual(nextSyncPayload.count, 81_925)",
            "Self.ethereumSyncCommitteeSignersBitmap(342)",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: Int = 512",
            "syncCommitteeWeights[$index] must be 1 for Ethereum mainnet",
            "signersBitmap.size == (syncCommitteePublicKeys.size + 7) / 8",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "List(512) { index ->",
            "assertEquals(81925, nextSyncPayload.size)",
            "syncCommitteeSignersBitmap(342)",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "indexedSyncCommitteeBytes(0x11, 48, index)",
            'syncCommitteeWeights = List(512) { "1" }',
            "syncCommitteeRoot must match syncCommitteePayload",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "must be 1 for Ethereum mainnet",
            "(syncCommitteePublicKeys.size() + 7) / 8",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "syncCommitteeBytes(0x11, 48)",
            "nextSyncPayload.length == 81925",
            "syncCommitteeSignersBitmap(342)",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "for (int index = 0; index < 512; index++)",
            "indexedSyncCommitteeBytes(0x11, 48, index)",
            "syncCommitteeRoot must match syncCommitteePayload",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "EthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteePayload must contain exactly",
            "must be 1 for Ethereum mainnet",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "Assert.Equal(81925, syncCommitteePayload.Length)",
            "CompressedSyncCommitteePayload()",
            "WeightedSyncCommitteePayload()",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_bridge_config_tests() -> None:
    """Ethereum source bridge material must bind mainnet config hashes."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_eth_source_bridge_evidence.py": (
            "def eth_source_bridge_config_hash(",
            "source_bridge_network_id must be Ethereum mainnet chain id 1",
            "ETH_SOURCE_BRIDGE_CONFIG_PREFIX",
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            "def _check_eth_source_bridge_config_hash(",
            "source_bridge_config_hash does not match ETH bridge address",
        ),
        ROOT / "pytests" / "scripts" / "sccp_eth_source_bridge_evidence_test.py": (
            "test_eth_source_bridge_config_hash_binds_mainnet_lane_and_code_hash",
            "invalid ETH source bridge config hash input was accepted",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "sourceBridgeNetworkId must be Ethereum mainnet chain id",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "ethSourceBridgeConfigHash(",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "sourceBridgeNetworkId",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashes.kt": (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofs.java": (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "SourceBridgeConfigHash must match the Ethereum mainnet source bridge config fields.",
            "NormalizeEthereumMainnetNetworkId(input.NetworkId)",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "ExpectedSourceBridgeConfigHash",
            "SourceBridgeConfigHash = \"0x\" + new string('9', 64)",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_report_rejects_phase_command_outside_claimed_block(
    tmp_path: Path,
) -> None:
    """A full transcript must bind the command to the claimed phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-rust-sccp-block.log"
    corridor_log.write_text(
        "==> SCCP production corridor: rust-sccp\n"
        "phase rust-sccp passed\n"
        "==> SCCP production corridor: js-sdk\n"
        "+ cargo test -p iroha_sccp -- --nocapture\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_success_marker(
    tmp_path: Path,
) -> None:
    """A passed phase artifact must contain a phase-local success marker."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "command-only-rust-sccp.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block success marker: test result: ok"
    ) in completed.stdout


def test_release_readiness_report_rejects_symlinked_phase_evidence(
    tmp_path: Path,
) -> None:
    """Strict release notes must hash the actual phase artifact, not a symlink."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "sccp-corridor.log"
    corridor_log.write_text(complete_corridor_log(), encoding="utf-8")
    corridor_link = tmp_path / "secret-token-sccp-corridor-link.log"
    corridor_link.symlink_to(corridor_log)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_link}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "release artifact path must not be a symlink" in completed.stderr
    assert "secret-token" not in completed.stderr


def test_release_readiness_rejects_control_character_artifact_paths(
    tmp_path: Path,
) -> None:
    """Release-readiness artifact paths must be printable reviewer text."""

    _, payload = write_complete_evidence(tmp_path)
    evidence = tmp_path / "secret-token-complete\noperator.toml"
    evidence.write_text(payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "release artifact path contains control character '\\n'" in completed.stderr
    assert "secret-token" not in completed.stderr
    assert "secret-token" not in completed.stdout


def test_release_readiness_rejects_markdown_unsafe_artifact_paths(
    tmp_path: Path,
) -> None:
    """Release-readiness artifact paths must not break Markdown review tables."""

    _, payload = write_complete_evidence(tmp_path)
    evidence = tmp_path / "secret-token-complete|operator.toml"
    evidence.write_text(payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "release artifact path contains Markdown-unsafe character '|'"
    ) in completed.stderr
    assert "secret-token" not in completed.stderr
    assert "secret-token" not in completed.stdout


def test_release_readiness_rejects_padded_artifact_paths(
    tmp_path: Path,
) -> None:
    """Release-readiness artifact paths must not rely on trimming."""

    _, payload = write_complete_evidence(tmp_path)
    evidence = tmp_path / "secret-token-complete-operator.toml "
    evidence.write_text(payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert (
        "release artifact path must not contain surrounding whitespace"
        in completed.stderr
    )
    assert "secret-token" not in completed.stderr
    assert "secret-token" not in completed.stdout


def test_release_readiness_rejects_percent_encoded_artifact_traversal_paths(
    tmp_path: Path,
) -> None:
    """Release-readiness artifact paths must reject encoded parent segments."""

    _, payload = write_complete_evidence(tmp_path)
    for marker in ("%2e%2e", "%252525252e%252525252e"):
        evidence_dir = tmp_path / marker
        evidence_dir.mkdir()
        evidence = evidence_dir / "secret-token-complete-operator.toml"
        evidence.write_text(payload, encoding="utf-8")

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--format",
                "json",
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 2
        assert (
            "release artifact path contains percent-encoded traversal segment"
            in completed.stderr
        )
        assert "secret-token" not in completed.stderr
        assert "secret-token" not in completed.stdout


def test_release_readiness_rejects_markdown_unsafe_native_evm_payload_paths(
    tmp_path: Path,
) -> None:
    """Native prover payload paths must not break Markdown review tables."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    proof_path = tmp_path / "native-prover-artifacts" / "proof-artifact.bin"
    proof_unsafe = (
        tmp_path
        / "native-prover-artifacts"
        / "secret-token-proof-artifact|operator.bin"
    )
    proof_path.rename(proof_unsafe)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact"] = (
        "native-prover-artifacts/secret-token-proof-artifact|operator.bin"
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = json.loads(completed.stdout)
    native_status = report["native_evm_prover_bundle"]
    assert native_status["validation_status"] == "blocked"
    assert any(
        "native EVM Groth16 prover bundle proof_artifact path contains "
        "Markdown-unsafe character '|'"
        in blocker
        for blocker in native_status["validation_blockers"]
    )
    assert "secret-token" not in "\n".join(native_status["validation_blockers"])
    assert "secret-token" not in completed.stderr


def test_release_readiness_rejects_control_character_native_evm_payload_paths(
    tmp_path: Path,
) -> None:
    """Native prover payload path diagnostics must not leak local path text."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    proof_path = tmp_path / "native-prover-artifacts" / "proof-artifact.bin"
    proof_control = (
        tmp_path
        / "native-prover-artifacts"
        / "secret-token-proof-artifact\noperator.bin"
    )
    proof_path.rename(proof_control)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact"] = (
        "native-prover-artifacts/secret-token-proof-artifact\noperator.bin"
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = json.loads(completed.stdout)
    native_status = report["native_evm_prover_bundle"]
    assert native_status["validation_status"] == "blocked"
    assert any(
        "native EVM Groth16 prover bundle proof_artifact path contains "
        "control character '\\n'"
        in blocker
        for blocker in native_status["validation_blockers"]
    )
    assert "secret-token" not in "\n".join(native_status["validation_blockers"])
    assert "secret-token" not in completed.stderr


def test_release_readiness_rejects_missing_native_evm_payload_paths_without_path_leak(
    tmp_path: Path,
) -> None:
    """Missing native prover payload diagnostics must not leak path text."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact"] = "native-prover-artifacts/secret-token-proof.bin"
    payload["cross_sdk_fixture_parity_artifact"] = (
        "native-prover-artifacts/secret-token-parity.json"
    )
    payload["native_prover_self_test_artifact"] = (
        "native-prover-artifacts/secret-token-self-test.json"
    )
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = json.loads(completed.stdout)
    native_status = report["native_evm_prover_bundle"]
    assert native_status["validation_status"] == "blocked"
    blockers = "\n".join(native_status["validation_blockers"])
    assert (
        "native EVM Groth16 prover bundle proof_artifact file is missing "
        "or is not a regular file"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle cross_sdk_fixture_parity_artifact "
        "file is missing or is not a regular file"
    ) in blockers
    assert (
        "native EVM Groth16 prover bundle native_prover_self_test_artifact "
        "file is missing or is not a regular file"
    ) in blockers
    assert "secret-token" not in blockers
    assert "secret-token" not in completed.stderr


def test_release_readiness_rejects_padded_native_evm_payload_paths(
    tmp_path: Path,
) -> None:
    """Native prover payload paths must not be accepted after trimming."""

    evidence, _ = write_complete_evidence(tmp_path)
    native_bundle = write_native_evm_prover_bundle(tmp_path, evidence)
    payload = json.loads(native_bundle.read_text(encoding="utf-8"))
    payload["proof_artifact"] = f" {payload['proof_artifact']} "
    native_bundle.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--native-evm-prover-bundle",
            str(native_bundle),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = json.loads(completed.stdout)
    native_status = report["native_evm_prover_bundle"]
    assert native_status["validation_status"] == "blocked"
    assert any(
        "native EVM Groth16 prover bundle proof_artifact path must not contain "
        "surrounding whitespace"
        in blocker
        for blocker in native_status["validation_blockers"]
    )
