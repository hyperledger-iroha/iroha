#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SDK_PARITY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "$ROOT_DIR" "$MODE" <<'PY'
import re
import sys
from fnmatch import fnmatchcase
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides = {}

KAGEMUSHA_HALO2_CANONICAL_VK_HASH_V1 = (
    "ad4d2ce680df32288d382c8b6403108d7174ca0ba9e558bd93a693f9d770b256"
)
KAGEMUSHA_HALO2_STALE_CANONICAL_VK_HASH_V1 = (
    "3493ea067302cab2180cef8f5dc60e0e6751ab9bb0c850286e2aaace2f863c25"
)

REQUIRED_C_SYMBOLS = (
    "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
    "connect_norito_kagemusha_recursive_spend_init",
    "connect_norito_kagemusha_recursive_spend_append",
    "connect_norito_kagemusha_recursive_spend_transition_profile_init",
    "connect_norito_kagemusha_recursive_spend_transition_profile_append",
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
    "connect_norito_kagemusha_recursive_spend_verify",
    "connect_norito_kagemusha_recursive_spend_redeem",
)

REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS = (
    "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
    "connect_norito_kagemusha_verify_recursive_compact_payment_token",
)

REQUIRED_RECORD_BACKED_KAGEMUSHA_C_SYMBOLS = (
    "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records",
    "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes",
)

REQUIRED_JS_NATIVE_METHODS = (
    "kagemushaRecursiveSpendInit",
    "kagemushaRecursiveSpendAppend",
    "kagemushaRecursiveSpendTransitionProfileInit",
    "kagemushaRecursiveSpendTransitionProfileAppend",
    "kagemushaRecursiveSpendLineageAppendBoundary",
    "kagemushaRecursiveSpendLineageWitnessFromInitResult",
    "kagemushaRecursiveSpendLineageWitnessAppendResult",
    "kagemushaRecursiveSpendVerify",
    "kagemushaRecursiveSpendRedeem",
)

REQUIRED_RECURSIVE_COMPACT_JS_METHODS = (
    "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
    "kagemushaVerifyRecursiveCompactPaymentToken",
)

REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_METHODS = (
    "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
    "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
)

REQUIRED_RECURSIVE_COMPACT_JS_PUBLIC_EXPORTS = (
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
    "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
    "isKagemushaRecursiveCompactUnavailable",
    *REQUIRED_RECURSIVE_COMPACT_JS_METHODS,
)

REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_PUBLIC_EXPORTS = (
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
    "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
    *REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_METHODS,
)

REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS = (
    "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
)

REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS = (
    "isKagemushaCompactPaymentTokenNativeAvailable",
    "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
    *REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
)

REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS = (
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
    "isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen",
    "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
    "kagemushaRecursiveSpendLineageKeyArtifactsForAppend",
    "kagemushaRecursiveSpendLineageKeyArtifacts",
    "validateKagemushaRecursiveSpendLineageKeyArtifacts",
)

REQUIRED_JS_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_EXPORTS = (
    "buildKagemushaInstructionArchiveInstruction",
    "buildKagemushaInstructionTransaction",
    "buildKagemushaRecursiveRedeemTransaction",
)

REQUIRED_PYTHON_NATIVE_METHODS = (
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
)

REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS = (
    "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
    "kagemusha_verify_recursive_compact_payment_token",
)

REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS = (
    "kagemusha_recursive_spend_compact_payment_token_from_bundle",
    "kagemusha_verify_recursive_spend_compact_payment_token_projection",
    "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
)

REQUIRED_RECURSIVE_COMPACT_PYTHON_PUBLIC_METHODS = (
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
    "is_kagemusha_recursive_compact_payment_token_prover_available",
    "is_kagemusha_recursive_compact_payment_token_verifier_available",
    "is_kagemusha_recursive_compact_unavailable",
)

REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_PUBLIC_METHODS = (
    "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
    "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
    *REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS,
)

REQUIRED_PYTHON_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_METHODS = (
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
    "KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME",
    "KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
    "KagemushaInstructionArchiveType",
    "kagemusha_instruction_archive_instruction",
    "kagemusha_recursive_redeem_instruction",
    "build_kagemusha_instruction_transaction",
    "build_kagemusha_recursive_redeem_transaction",
)

REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_PUBLIC_METHODS = (
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
    "KagemushaRecursiveSpendLineageKeyArtifacts",
    "is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len",
    "kagemusha_recursive_spend_lineage_key_artifacts_for_init",
    "kagemusha_recursive_spend_lineage_key_artifacts_for_append",
    "kagemusha_recursive_spend_lineage_key_artifacts",
    "validate_kagemusha_recursive_spend_lineage_key_artifacts",
)

REQUIRED_PUBLIC_METHODS = (
    "initSpend",
    "appendSpend",
    "transitionProfileInit",
    "transitionProfileAppend",
    "lineageAppendBoundary",
    "lineageWitnessFromInitResult",
    "lineageWitnessAppendResult",
    "verifySpend",
    "redeemSpend",
)

REQUIRED_JS_PUBLIC_EXPORTS = REQUIRED_JS_NATIVE_METHODS + (
    "isKagemushaRecursiveSpendNativeAvailable",
    "preferredKagemushaOfflineSpendMode",
    "canRedeemKagemushaRecursiveSpendWitnessless",
    "requiresKagemushaRecursiveSpendLineageWitnessForRedeem",
    "canAppendKagemushaRecursiveSpendWitnesslessLineage",
    "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "isSupportedKagemushaRecursiveSpendPreviousProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend",
    "isSupportedKagemushaRecursiveSpendAppendProofTransition",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    "preferredKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canProveKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId",
    "requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend",
) + REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS

REQUIRED_PYTHON_PUBLIC_METHODS = REQUIRED_PYTHON_NATIVE_METHODS + (
    "is_kagemusha_recursive_spend_available",
    "preferred_kagemusha_offline_spend_mode",
    "can_redeem_kagemusha_recursive_spend_witnessless",
    "requires_kagemusha_recursive_spend_lineage_witness_for_redeem",
    "can_append_kagemusha_recursive_spend_witnessless_lineage",
    "normalize_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "is_supported_kagemusha_recursive_spend_previous_proof_circuit_id",
    "requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append",
    "is_supported_kagemusha_recursive_spend_append_proof_transition",
    "preferred_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "can_select_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append",
) + REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_PUBLIC_METHODS + REQUIRED_PYTHON_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_METHODS

JNI_METHODS = (
    "nativeBridgeAbiVersion",
    "nativeInitSpend",
    "nativeAppendSpend",
    "nativeTransitionProfileInit",
    "nativeTransitionProfileAppend",
    "nativeLineageAppendBoundary",
    "nativeLineageWitnessFromInitResult",
    "nativeLineageWitnessAppendResult",
    "nativeVerifySpend",
    "nativeRedeemSpend",
)

REQUIRED_RECURSIVE_COMPACT_JNI_METHODS = (
    "nativeBridgeAbiVersion",
    "nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
    "nativeRecursiveSpendCompactPaymentTokenFromBundle",
    "nativeVerifyRecursiveCompactPaymentToken",
    "nativeVerifyRecursiveSpendCompactPaymentTokenProjection",
    "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
)

SOURCE_PATHS = (
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "crates/connect_norito_bridge/include/NoritoBridge.h",
    "crates/iroha_core/src/zk.rs",
    "crates/iroha_data_model/src/offline/mod.rs",
    "crates/iroha_js_host/src/lib.rs",
    "docs/source/offline_kagemusha.md",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift",
    "IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift",
    "IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift",
    "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaInstructionArchives.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteHalo2Prover.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionBuilderTests.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteV2Test.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchives.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteHalo2Prover.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchivesTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteV2Test.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/src/crypto.browser.js",
    "javascript/iroha_js/dist/crypto.browser.js",
    "javascript/iroha_js/src/index.js",
    "javascript/iroha_js/dist/index.js",
    "javascript/iroha_js/src/transaction.js",
    "javascript/iroha_js/dist/transaction.js",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/package.json",
    "javascript/iroha_js/package-lock.json",
    "javascript/iroha_js/test/crypto.browser.test.js",
    "javascript/iroha_js/test/transactionBuilder.test.js",
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
    "javascript/iroha_js/test/package_dist.test.js",
    "javascript/iroha_js/test/privacyNative.test.js",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_python/src/iroha_python/crypto.py",
    "python/iroha_python/src/iroha_python/kagemusha.py",
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
    "python/iroha_python/src/iroha_python/tx.py",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
    "python/iroha_python/tests/kagemusha_test.py",
    "python/iroha_python/tests/privacy_catalog_test.py",
    "python/iroha_python/tests/crypto_algorithms_test.py",
    "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj",
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/KagemushaInstructionArchiveInstruction.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionEncodingContext.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionInstruction.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs",
)

NATIVE_MANIFEST_PATHS = (
    "crates/connect_norito_bridge/Cargo.toml",
    "crates/iroha_js_host/Cargo.toml",
    "python/iroha_python/iroha_python_rs/Cargo.toml",
)

SDK_README_PATHS = (
    "IrohaSwift/README.md",
    "java/iroha_android/README.md",
    "kotlin/README.md",
    "csharp/README.md",
    "javascript/iroha_js/README.md",
    "python/iroha_python/README.md",
)

WORKFLOW_PATH = ".github/workflows/pr_kagemusha_payload_bench.yml"
JS_PARITY_TEST_PATH = "javascript/iroha_js/test/kagemushaFfiContractParity.test.js"
MAIN_JOB = "kagemusha_payload_bench"
NATIVE_BRIDGE_JOB = "kagemusha_native_bridge_tests"
PYTHON_SDK_JOB = "kagemusha_python_sdk_tests"
JVM_SDK_JOB = "kagemusha_jvm_sdk_tests"
SWIFT_SDK_JOB = "kagemusha_swift_sdk_parse"
CSHARP_SDK_JOB = "kagemusha_csharp_sdk_tests"
JS_SDK_JOB = "kagemusha_javascript_sdk_tests"
WORKFLOW_REQUIRED_PATHS = SOURCE_PATHS + (
    *SDK_README_PATHS,
    *NATIVE_MANIFEST_PATHS,
    WORKFLOW_PATH,
    JS_PARITY_TEST_PATH,
    "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
    "ci/check_no_tracked_python_bytecode.sh",
    "ci/check_kagemusha_recursive_spend_python_sdk.sh",
    "ci/check_kagemusha_recursive_spend_jvm_sdk.sh",
    "ci/check_kagemusha_recursive_spend_swift_sdk.sh",
    "ci/check_kagemusha_recursive_spend_csharp_sdk.sh",
    "ci/check_kagemusha_recursive_spend_js_sdk.sh",
)

SDK_PRIVACY_WORKFLOW_INVENTORY_PATHS = (
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/test/privacyNative.test.js",
    "python/iroha_python/src/iroha_python/crypto.py",
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
    "python/iroha_python/tests/privacy_catalog_test.py",
    "python/iroha_python/tests/crypto_algorithms_test.py",
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
)
SDK_PARITY_MAIN_COMMAND = "ci/check_kagemusha_recursive_spend_sdk_parity.sh"
PYTHON_BYTECODE_COMMAND = "bash ci/check_no_tracked_python_bytecode.sh"
NATIVE_BRIDGE_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_spend_ffi_rejects_invalid_archives_without_output --lib -- --test-threads=1"
NATIVE_BRIDGE_EMPTY_NESTED_PALLAS_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_spend_ffi_rejects_empty_nested_pallas_archives_without_output --lib -- --test-threads=1"
NATIVE_BRIDGE_LINEAGE_WITNESS_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_spend_lineage_witness_ffi_rejects_invalid_inputs_without_output --lib -- --test-threads=1"
NATIVE_BRIDGE_APPEND_BOUNDARY_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_spend_lineage_append_boundary_ffi_rejects_semantic_profile_archives --lib -- --test-threads=1"
JS_HOST_APPEND_BOUNDARY_TEST_COMMAND = "cargo test -p iroha_js_host kagemusha_recursive_spend_lineage_append_boundary_rejects_duplicate_current_outputs --lib -- --test-threads=1"
NATIVE_BRIDGE_OVERSIZED_LENGTH_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_compact_ffi_rejects_oversized_lengths_without_output --lib -- --test-threads=1"
NATIVE_BRIDGE_UNANCHORED_COMPACT_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_verified_compact_token_ffi_rejects --lib -- --test-threads=1"
NATIVE_BRIDGE_UNANCHORED_VALID_COMPACT_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_unanchored_compact_token_ffi_rejects_valid_bundle_without_records --lib -- --test-threads=1"
NATIVE_BRIDGE_RECORD_COMPACT_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_verified_record_compact_token_ffi_rejects_bad_records --lib -- --test-threads=1"
NATIVE_BRIDGE_RECORD_RECURSIVE_AGGREGATION_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_verified_record_recursive_aggregation_proof_bundle_ffi_rejects_adversarial_inputs --lib -- --test-threads=1"
NATIVE_BRIDGE_RECURSIVE_COMPACT_TEST_COMMAND = "RUST_MIN_STACK=67108864 CARGO_PROFILE_TEST_OPT_LEVEL=3 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p connect_norito_bridge kagemusha_recursive_compact_ffi_fails_closed_and_rejects_adversarial_inputs --lib -- --test-threads=1"
NATIVE_BRIDGE_RECURSIVE_COMPACT_WINDOWED_RECORD_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_compact_ffi_rejects_windowed_records_before_unavailable --lib -- --test-threads=1"
PYTHON_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_python_sdk.sh"
PYTHON_HOST_APPEND_BOUNDARY_TEST_COMMAND = "cargo test -p iroha_python_rs kagemusha_recursive_spend_lineage_append_boundary_python_rejects_duplicate_current_outputs --lib -- --test-threads=1"
JVM_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_jvm_sdk.sh"
SWIFT_SDK_PARSE_COMMAND = "ci/check_kagemusha_recursive_spend_swift_sdk.sh"
CSHARP_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_csharp_sdk.sh"
JS_SDK_INSTALL_COMMAND = "npm ci --prefix javascript/iroha_js"
JS_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_js_sdk.sh"
MAIN_JOB_NEEDS_LINE = (
    "    needs: [kagemusha_native_bridge_tests, kagemusha_swift_sdk_parse, "
    "kagemusha_csharp_sdk_tests, kagemusha_javascript_sdk_tests, "
    "kagemusha_jvm_sdk_tests, kagemusha_python_sdk_tests]"
)
SDK_PARITY_NEGATIVE_CONTROL_COMMANDS = (
    (
        "SDK surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control",
    ),
    (
        "SDK workflow path negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-workflow",
    ),
    (
        "native manifest workflow path negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-manifest-workflow",
    ),
    (
        "JavaScript browser helper negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-browser-helper",
    ),
    (
        "JavaScript lineage key artifact copy negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-lineage-key-artifact-copy",
    ),
    (
        "JavaScript lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-lineage-key-package-binding",
    ),
    (
        "JavaScript Kagemusha instruction transaction builder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-kagemusha-instruction-transaction-builder",
    ),
    (
        "JavaScript/Python native output header negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-python-native-output-headers",
    ),
    (
        "Python Kagemusha instruction transaction builder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-kagemusha-instruction-transaction-builder",
    ),
    (
        "C# Kagemusha instruction transaction builder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-kagemusha-instruction-transaction-builder",
    ),
    (
        "Python lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-lineage-key-package-binding",
    ),
    (
        "C# lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-lineage-key-package-binding",
    ),
    (
        "Swift lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-lineage-key-package-binding",
    ),
    (
        "C# lineage witness availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-lineage-witness-availability-probe",
    ),
    (
        "C# lineage witness append availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-lineage-witness-append-availability-probe",
    ),
    (
        "Swift lineage witness availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-lineage-witness-availability-probe",
    ),
    (
        "Swift lineage witness append availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-lineage-witness-append-availability-probe",
    ),
    (
        "Kotlin/JVM lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-lineage-key-package-binding",
    ),
    (
        "Android Java lineage key package binding negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-android-lineage-key-package-binding",
    ),
    (
        "Kotlin/JVM lineage witness availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-lineage-witness-availability-probe",
    ),
    (
        "Kotlin/JVM lineage witness append availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-lineage-witness-append-availability-probe",
    ),
    (
        "Android Java lineage witness availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-android-lineage-witness-availability-probe",
    ),
    (
        "Android Java lineage witness append availability probe negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-android-lineage-witness-append-availability-probe",
    ),
    (
        "JavaScript lineage key artifact readonly declarations negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-lineage-readonly-declarations",
    ),
    (
        "SDK archive input ownership negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-archive-input-copy",
    ),
    (
        "SDK lineage proving key artifact copy negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-lineage-proving-key-copy",
    ),
    (
        "SDK public helper surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-helper-surface",
    ),
    (
        "SDK README boundary negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-boundary",
    ),
    (
        "SDK README proof-chain accumulator negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-proof-chain-accumulator",
    ),
    (
        "offline Kagemusha doc accumulator boundary negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-offline-doc-native-owned-accumulator-boundary",
    ),
    (
        "offline Kagemusha doc instruction transaction surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-offline-doc-instruction-transaction-surface",
    ),
    (
        "SDK proof-chain accumulator public-input negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-proof-chain-accumulator-input",
    ),
    (
        "SDK accumulator digest public-input negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-accumulator-digest-inputs",
    ),
    (
        "SDK accumulator boundary digest public-input negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-accumulator-boundary-digest-inputs",
    ),
    (
        "SDK README availability surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-availability-surface",
    ),
    (
        "SDK README recursive compact unavailable negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-recursive-compact-unavailable",
    ),
    (
        "SDK README compact projection verifier negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-compact-projection-verifier",
    ),
    (
        "SDK README stale Reserved-lineage wording negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-stale-future-lineage",
    ),
    (
        "SDK README native output C# boundary negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-native-output-csharp",
    ),
    (
        "cross-SDK helper-body negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-cross-sdk-helper-bodies",
    ),
    (
        "cross-SDK preferred-mode fallback negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-cross-sdk-preferred-mode-fallback",
    ),
    (
        "mobile Halo2 canonical VK hash negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-mobile-halo2-vk-hash",
    ),
    (
        "Rust recursive compact unavailable classifier negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-rust-recursive-compact-unavailable-classifier",
    ),
    (
        "SDK recursive compact unavailable helper negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-recursive-compact-unavailable-helper",
    ),
    (
        "ABI-7 recursive compact verifier surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-recursive-compact-verifier-surface",
    ),
    (
        "ABI-7 recursive compact key-package arity negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-recursive-compact-key-package-arity",
    ),
    (
        "Python recursive compact probe arity negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-recursive-compact-probe-arity",
    ),
    (
        "JavaScript recursive compact key-package dispatch negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-recursive-compact-key-package-dispatch",
    ),
    (
        "JavaScript package dist recursive compact declaration negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-recursive-compact-declarations",
    ),
    (
        "JavaScript package dist accumulator digest declaration negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-accumulator-digest-declarations",
    ),
    (
        "JavaScript package dist accumulator digest denylist negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-accumulator-digest-denylist",
    ),
    (
        "JavaScript package dist terminal accumulator digest denylist negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-terminal-accumulator-digest-denylist",
    ),
    (
        "JavaScript package dist declaration sweep negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-declaration-sweep",
    ),
    (
        "JavaScript package dist Nexus declaration sweep negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-nexus-declaration-sweep",
    ),
    (
        "JavaScript package dist Kotodama declaration sweep negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-package-dist-kotodama-declaration-sweep",
    ),
    (
        "JavaScript TypeScript recursive compact key-package declaration negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-dts-recursive-compact-key-package",
    ),
    (
        "Python recursive compact root re-export negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-recursive-compact-root-export",
    ),
    (
        "recursive spend compact projection surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-recursive-spend-compact-projection-surface",
    ),
    (
        "JavaScript compact projection block-height validation negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-compact-projection-block-height-validation",
    ),
    (
        "Python recursive spend compact projection root export negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-recursive-spend-compact-projection-root-export",
    ),
    (
        "JVM compact projection unsigned block-height negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-compact-projection-unsigned-block-height",
    ),
    (
        "native bridge zero-envelope Pallas guard negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-zero-envelope-pallas-guard",
    ),
    (
        "Kagemusha ABI probe bounds negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-kagemusha-abi-probe-bounds",
    ),
    (
        "Kagemusha probe rejection shape negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-kagemusha-probe-rejection-shape",
    ),
    (
        "SDK negative-control workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-negative-controls-workflow",
    ),
    (
        "SDK commented-command workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-negative-controls-comment-workflow",
    ),
    (
        "SDK main guard workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-main-guard-workflow",
    ),
    (
        "tracked Python bytecode workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-bytecode-workflow",
    ),
    (
        "native bridge workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-job-workflow",
    ),
    (
        "native bridge runner workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-runner-workflow",
    ),
    (
        "native bridge Rust cache workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-cache-workflow",
    ),
    (
        "native bridge test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-test-workflow",
    ),
    (
        "native bridge windowed-record ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-windowed-record-order-workflow",
    ),
    (
        "native bridge dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-native-bridge-needs-workflow",
    ),
    (
        "Python SDK workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-job-workflow",
    ),
    (
        "Python SDK runner workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-runner-workflow",
    ),
    (
        "Python SDK setup workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-setup-workflow",
    ),
    (
        "Python SDK version workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-version-workflow",
    ),
    (
        "Python SDK setup ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-setup-order-workflow",
    ),
    (
        "Python SDK Rust cache workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-rust-cache-workflow",
    ),
    (
        "Python SDK timeout workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-timeout-workflow",
    ),
    (
        "Python SDK version script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-version-script",
    ),
    (
        "Python SDK override script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-override-script",
    ),
    (
        "Python SDK resolver script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-resolver-script",
    ),
    (
        "Python SDK major script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-major-script",
    ),
    (
        "Python SDK stale venv rebuild script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-venv-rebuild-script",
    ),
    (
        "Python SDK native build script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-native-build-script",
    ),
    (
        "Python SDK venv activation script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-venv-activation-script",
    ),
    (
        "Python SDK bytecode script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-bytecode-script",
    ),
    (
        "Python SDK test filter script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-test-filter-script",
    ),
    (
        "Python SDK workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-workflow-inventory",
    ),
    (
        "Python lineage frozen key copy negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-lineage-frozen-copy",
    ),
    (
        "Python SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-test-workflow",
    ),
    (
        "Python host test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-host-test-workflow",
    ),
    (
        "Python SDK dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-needs-workflow",
    ),
    (
        "JVM SDK workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-job-workflow",
    ),
    (
        "JVM SDK runner workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-runner-workflow",
    ),
    (
        "JVM SDK Java setup workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-java-setup-workflow",
    ),
    (
        "JVM SDK Java distribution workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-java-distribution-workflow",
    ),
    (
        "JVM SDK Java version workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-java-version-workflow",
    ),
    (
        "JVM SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-test-workflow",
    ),
    (
        "JVM SDK test filter script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-test-filter-script",
    ),
    (
        "JVM SDK workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-workflow-inventory",
    ),
    (
        "JVM SDK Android workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-android-workflow-inventory",
    ),
    (
        "JVM SDK JDK 21 script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-jdk21-script",
    ),
    (
        "JVM SDK Java home override script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-java-home-override-script",
    ),
    (
        "JVM SDK inherited Java home rejection script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-java-home-reject-script",
    ),
    (
        "JVM recursive compact verifier availability negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-recursive-compact-verifier-availability",
    ),
    (
        "JVM recursive compact shape classifier negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-recursive-compact-shape-classifier",
    ),
    (
        "Mobile recursive spend native output header negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-mobile-recursive-spend-native-output-headers",
    ),
    (
        "JVM Offline Note V2 decoder placeholder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-offline-note-v2-decoder-placeholder",
    ),
    (
        "JVM Offline Note V2 instruction wrapper negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-offline-note-v2-instruction-wrapper",
    ),
    (
        "JVM Offline Note V2 instruction decoder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-offline-note-v2-instruction-decoder",
    ),
    (
        "Offline Note V2 canonical instruction wire-name negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-offline-note-v2-canonical-instruction-wire-names",
    ),
    (
        "Swift Offline Note V2 decoder placeholder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-offline-note-v2-decoder-placeholder",
    ),
    (
        "Swift Offline Note V2 instruction decoder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-offline-note-v2-instruction-decoder",
    ),
    (
        "JVM SDK Android harness script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-android-harness-script",
    ),
    (
        "JVM SDK test ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-test-order-workflow",
    ),
    (
        "JVM SDK dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-jvm-sdk-needs-workflow",
    ),
    (
        "Swift SDK workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-job-workflow",
    ),
    (
        "Swift SDK runner workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-runner-workflow",
    ),
    (
        "Swift SDK parse workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-parse-workflow",
    ),
    (
        "Swift SDK parse surface script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-parse-surface-script",
    ),
    (
        "Swift SDK privacy parse script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-privacy-parse-script",
    ),
    (
        "Swift SDK workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-workflow-inventory",
    ),
    (
        "Swift SDK source workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-source-workflow-inventory",
    ),
    (
        "Swift UC4 diagnostic skip negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-uc4-skip",
    ),
    (
        "Swift lineage key artifact Data copy negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-lineage-data-copy",
    ),
    (
        "Swift recursive compact verifier bool negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-recursive-compact-verifier-bool",
    ),
    (
        "Swift recursive compact verifier availability negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-recursive-compact-verifier-availability",
    ),
    (
        "Swift Kagemusha native output cap negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-kagemusha-native-output-cap",
    ),
    (
        "Swift recursive spend native output header negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-native-output-headers",
    ),
    (
        "Swift recursive spend native input header negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-native-input-headers",
    ),
    (
        "Swift Kagemusha instruction transaction builder negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-kagemusha-instruction-transaction-builder",
    ),
    (
        "Swift SDK version script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-version-script",
    ),
    (
        "Swift SDK compiler override script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-override-script",
    ),
    (
        "Swift SDK dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-swift-sdk-needs-workflow",
    ),
    (
        "C# SDK workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-job-workflow",
    ),
    (
        "C# SDK setup workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-setup-workflow",
    ),
    (
        "C# SDK dotnet version workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-dotnet-version-workflow",
    ),
    (
        "C# SDK setup ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-setup-order-workflow",
    ),
    (
        "C# SDK dotnet version script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-dotnet-version-script",
    ),
    (
        "C# SDK dotnet info script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-dotnet-info-script",
    ),
    (
        "C# SDK dotnet override script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-dotnet-override-script",
    ),
    (
        "C# SDK dotnet major script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-dotnet-major-script",
    ),
    (
        "C# SDK native bridge script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-native-bridge-script",
    ),
    (
        "C# SDK native library evidence script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-native-library-evidence-script",
    ),
    (
        "C# SDK test filter script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-test-filter-script",
    ),
    (
        "C# SDK workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-workflow-inventory",
    ),
    (
        "C# archive wrapper copy negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-archive-copy",
    ),
    (
        "C# recursive compact verifier unavailable negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-recursive-compact-verifier-unavailable",
    ),
    (
        "C# SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-test-workflow",
    ),
    (
        "C# SDK dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-csharp-sdk-needs-workflow",
    ),
    (
        "JavaScript SDK workflow job negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-job-workflow",
    ),
    (
        "JavaScript SDK runner workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-runner-workflow",
    ),
    (
        "JavaScript SDK Node setup workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-setup-workflow",
    ),
    (
        "JavaScript SDK Node version workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-version-workflow",
    ),
    (
        "JavaScript SDK Node version script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-version-script",
    ),
    (
        "JavaScript SDK Node override script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-override-script",
    ),
    (
        "JavaScript SDK Node resolver script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-resolver-script",
    ),
    (
        "JavaScript SDK Node major script negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-major-script",
    ),
    (
        "JavaScript SDK Node cache workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-cache-workflow",
    ),
    (
        "JavaScript SDK Node setup ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-node-setup-order-workflow",
    ),
    (
        "JavaScript SDK install workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-install-workflow",
    ),
    (
        "JavaScript SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-test-workflow",
    ),
    (
        "JavaScript SDK transaction-builder test filter negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-transaction-builder-filter-script",
    ),
    (
        "JavaScript SDK privacy native test filter negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-privacy-native-filter-script",
    ),
    (
        "JavaScript SDK workflow inventory negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-workflow-inventory",
    ),
    (
        "SDK privacy workflow inventory matrix negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-privacy-workflow-inventory-matrix",
    ),
    (
        "JavaScript SDK install ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-install-order-workflow",
    ),
    (
        "JavaScript SDK test ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-test-order-workflow",
    ),
    (
        "JavaScript SDK dependency workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-sdk-needs-workflow",
    ),
    (
        "SDK parity meta-test workflow path negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-parity-meta-test-workflow",
    ),
    (
        "SDK negative-control ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-negative-controls-order-workflow",
    ),
)


class ParityError(RuntimeError):
    pass


def read_sources():
    texts = {}
    missing = []
    for relative in SOURCE_PATHS + SDK_README_PATHS:
        path = root / relative
        if not path.exists():
            missing.append(relative)
            continue
        texts[relative] = path.read_text(encoding="utf-8")
    if missing:
        raise ParityError("missing source files: " + ", ".join(missing))
    return texts


def read(relative):
    if relative in text_overrides:
        return text_overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def names_from_matches(text, pattern):
    return set(re.findall(pattern, text, re.S))


def require(condition, message, errors):
    if not condition:
        errors.append(message)


def require_contains(texts, relative, needles, label, errors):
    text = texts[relative]
    for needle in needles:
        require(needle in text, f"{label} missing {needle}", errors)


def require_regex(texts, relative, pattern, label, errors, flags=0):
    text = texts[relative]
    require(re.search(pattern, text, flags) is not None, f"{label} missing pattern {pattern}", errors)


def require_not_regex(texts, relative, pattern, label, errors, flags=0):
    text = texts[relative]
    require(
        re.search(pattern, text, flags) is None,
        f"{label} contains forbidden pattern {pattern}",
        errors,
    )


def require_same_set(actual, expected, label, errors):
    actual = set(actual)
    expected = set(expected)
    require(
        actual == expected,
        (
            f"{label} drifted; missing={sorted(expected - actual)} "
            f"extra={sorted(actual - expected)}"
        ),
        errors,
    )


def workflow_trigger_paths():
    paths = []
    in_paths = False
    for line in read(WORKFLOW_PATH).splitlines():
        if line.strip() == "paths:":
            in_paths = True
            continue
        if not in_paths:
            continue
        match = re.match(r'\s+-\s+"([^"]+)"\s*$', line)
        if match is not None:
            paths.append(match.group(1))
            continue
        if line and not line.startswith("      "):
            break
    return paths


def check_workflow_paths(errors):
    trigger_paths = workflow_trigger_paths()
    missing = [
        relative
        for relative in WORKFLOW_REQUIRED_PATHS
        if not any(fnmatchcase(relative, pattern) for pattern in trigger_paths)
    ]
    require(
        not missing,
        "Kagemusha payload workflow paths do not cover SDK parity sources: "
        + ", ".join(missing),
        errors,
    )


def workflow_real_benchmark_index(workflow, errors):
    benchmark_match = re.search(
        r"^\s+run:\s+ci/check_kagemusha_recursive_spend_payload_bench\.sh\s*$",
        workflow,
        re.M,
    )
    require(
        benchmark_match is not None,
        "Kagemusha payload workflow must run the real payload benchmark",
        errors,
    )
    return None if benchmark_match is None else benchmark_match.start()


def workflow_command_match(workflow, command):
    return re.search(rf"(?m)^\s+(?:run:\s+)?{re.escape(command)}\s*$", workflow)


def workflow_job_block(workflow, job_name):
    match = re.search(
        rf"(?ms)^  {re.escape(job_name)}:\s*\n(?P<block>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)",
        workflow,
    )
    return None if match is None else match.group("block")


def workflow_job_needs(workflow, job_name):
    block = workflow_job_block(workflow, job_name)
    if block is None:
        return set()
    match = re.search(r"(?m)^\s+needs:\s*(?P<needs>.+?)\s*$", block)
    if match is None:
        return set()
    needs = match.group("needs").strip()
    if needs.startswith("[") and needs.endswith("]"):
        return {item.strip() for item in needs[1:-1].split(",") if item.strip()}
    return {needs}


def workflow_sdk_main_guard_index(workflow, errors):
    command_match = re.search(
        rf"(?m)^\s+run:\s+{re.escape(SDK_PARITY_MAIN_COMMAND)}\s*$",
        workflow,
    )
    require(
        command_match is not None,
        "Kagemusha payload workflow must run the main SDK parity guard",
        errors,
    )
    return None if command_match is None else command_match.start()


def check_workflow_runs_sdk_main_guard(errors):
    workflow = read(WORKFLOW_PATH)
    bytecode_match = workflow_command_match(workflow, PYTHON_BYTECODE_COMMAND)
    benchmark_index = workflow_real_benchmark_index(workflow, errors)
    main_guard_index = workflow_sdk_main_guard_index(workflow, errors)
    if benchmark_index is None or main_guard_index is None:
        return
    require(
        bytecode_match is not None,
        "Kagemusha payload workflow must reject tracked Python bytecode artifacts",
        errors,
    )
    if bytecode_match is not None:
        require(
            bytecode_match.start() < main_guard_index,
            "Kagemusha payload workflow must reject tracked Python bytecode before the main SDK parity guard",
            errors,
        )
    require(
        main_guard_index < benchmark_index,
        "Kagemusha payload workflow must run the main SDK parity guard before the real benchmark",
        errors,
    )


def check_workflow_runs_sdk_negative_controls(errors):
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow, errors)
    main_guard_index = workflow_sdk_main_guard_index(workflow, errors)
    if benchmark_index is None or main_guard_index is None:
        return
    for label, command in SDK_PARITY_NEGATIVE_CONTROL_COMMANDS:
        command_match = workflow_command_match(workflow, command)
        require(
            command_match is not None,
            f"Kagemusha payload workflow must run the SDK parity {label}",
            errors,
        )
        if command_match is not None:
            require(
                command_match.start() < benchmark_index,
                f"Kagemusha payload workflow must run the SDK parity {label} before the real benchmark",
                errors,
            )
            require(
                command_match.start() < main_guard_index,
                f"Kagemusha payload workflow must run the SDK parity {label} before the main SDK parity guard",
                errors,
            )


def check_workflow_runs_native_bridge_tests(errors):
    workflow = read(WORKFLOW_PATH)
    job_block = workflow_job_block(workflow, NATIVE_BRIDGE_JOB)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    require(
        job_block is not None,
        "Kagemusha payload workflow must define the native bridge test job",
        errors,
    )
    if job_block is None:
        return
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Kagemusha payload workflow must run native bridge tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Kagemusha payload workflow must cache Rust artifacts for native bridge tests",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the native recursive spend bridge smoke test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_EMPTY_NESTED_PALLAS_TEST_COMMAND)
        is not None,
        "Kagemusha payload workflow must run the native recursive spend empty nested-Pallas bridge test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_LINEAGE_WITNESS_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the native lineage-witness bridge invalid-input test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_APPEND_BOUNDARY_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the native append-boundary semantic-profile bridge test",
        errors,
    )
    require(
        workflow_command_match(job_block, JS_HOST_APPEND_BOUNDARY_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the JS host append-boundary duplicate-output test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_OVERSIZED_LENGTH_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the native Kagemusha oversized-length FFI test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_UNANCHORED_COMPACT_TEST_COMMAND)
        is not None,
        "Kagemusha payload workflow must run the native unanchored compact-token invalid-input tests",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_UNANCHORED_VALID_COMPACT_TEST_COMMAND)
        is not None,
        "Kagemusha payload workflow must run the native unanchored compact-token valid-bundle rejection test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_RECORD_COMPACT_TEST_COMMAND) is not None,
        "Kagemusha payload workflow must run the native record-backed compact-token adversarial test",
        errors,
    )
    require(
        workflow_command_match(job_block, NATIVE_BRIDGE_RECORD_RECURSIVE_AGGREGATION_TEST_COMMAND)
        is not None,
        "Kagemusha payload workflow must run the native record-backed recursive aggregation adversarial test",
        errors,
    )
    recursive_compact_match = workflow_command_match(
        job_block, NATIVE_BRIDGE_RECURSIVE_COMPACT_TEST_COMMAND
    )
    windowed_record_match = workflow_command_match(
        job_block, NATIVE_BRIDGE_RECURSIVE_COMPACT_WINDOWED_RECORD_TEST_COMMAND
    )
    require(
        recursive_compact_match is not None,
        "Kagemusha payload workflow must run the native recursive compact bridge adversarial test",
        errors,
    )
    require(
        windowed_record_match is not None,
        "Kagemusha payload workflow must run the native recursive compact windowed-record bridge test",
        errors,
    )
    if recursive_compact_match is not None and windowed_record_match is not None:
        require(
            windowed_record_match.start() < recursive_compact_match.start(),
            "Kagemusha payload workflow must run the native recursive compact windowed-record bridge test before the heavyweight recursive compact adversarial test",
            errors,
        )
    require(
        NATIVE_BRIDGE_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the native bridge test job",
        errors,
    )


def check_workflow_runs_python_sdk_tests(errors):
    workflow = read(WORKFLOW_PATH)
    job_block = workflow_job_block(workflow, PYTHON_SDK_JOB)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    require(
        job_block is not None,
        "Kagemusha payload workflow must define the Python SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-python@v5\s*$", job_block)
    version_match = re.search(r'(?m)^\s+python-version:\s+"3\.11"\s*$', job_block)
    host_command_match = workflow_command_match(job_block, PYTHON_HOST_APPEND_BOUNDARY_TEST_COMMAND)
    command_match = workflow_command_match(job_block, PYTHON_SDK_TEST_COMMAND)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Kagemusha payload workflow must run Python SDK tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+timeout-minutes:\s+45\s*$", job_block) is not None,
        "Kagemusha payload workflow must allow 45 minutes for Python native builds",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Kagemusha payload workflow must cache Rust artifacts for Python native builds",
        errors,
    )
    require(
        setup_match is not None,
        "Kagemusha payload workflow must set up Python for Python SDK tests",
        errors,
    )
    require(
        version_match is not None,
        "Kagemusha payload workflow must pin Python 3.11 for Python SDK tests",
        errors,
    )
    require(
        host_command_match is not None,
        "Kagemusha payload workflow must run the Python PyO3 append-boundary host test",
        errors,
    )
    require(
        command_match is not None,
        "Kagemusha payload workflow must run the Python recursive spend SDK tests",
        errors,
    )
    if setup_match is not None and host_command_match is not None:
        require(
            setup_match.start() < host_command_match.start(),
            "Kagemusha payload workflow must set up Python before running PyO3 host tests",
            errors,
        )
    if host_command_match is not None and command_match is not None:
        require(
            host_command_match.start() < command_match.start(),
            "Kagemusha payload workflow must run PyO3 host tests before package-level Python SDK tests",
            errors,
        )
    if setup_match is not None and command_match is not None:
        require(
            setup_match.start() < command_match.start(),
            "Kagemusha payload workflow must set up Python before running Python SDK tests",
            errors,
        )
    require(
        PYTHON_SDK_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the Python SDK test job",
        errors,
    )


def check_workflow_runs_jvm_sdk_tests(errors):
    workflow = read(WORKFLOW_PATH)
    job_block = workflow_job_block(workflow, JVM_SDK_JOB)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    require(
        job_block is not None,
        "Kagemusha payload workflow must define the JVM SDK test job",
        errors,
    )
    if job_block is None:
        return
    java_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-java@v4\s*$", job_block)
    java_distribution_match = re.search(r'(?m)^\s+distribution:\s+"temurin"\s*$', job_block)
    java_version_match = re.search(r'(?m)^\s+java-version:\s+"21"\s*$', job_block)
    command_match = workflow_command_match(job_block, JVM_SDK_TEST_COMMAND)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Kagemusha payload workflow must run JVM SDK tests on Ubuntu",
        errors,
    )
    require(
        java_setup_match is not None,
        "Kagemusha payload workflow must set up Java for JVM SDK tests",
        errors,
    )
    require(
        java_distribution_match is not None,
        "Kagemusha payload workflow must use Temurin Java for JVM SDK tests",
        errors,
    )
    require(
        java_version_match is not None,
        "Kagemusha payload workflow must pin Java 21 for JVM SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Kagemusha payload workflow must run the JVM recursive spend SDK tests",
        errors,
    )
    if java_setup_match is not None and command_match is not None:
        require(
            java_setup_match.start() < command_match.start(),
            "Kagemusha payload workflow must set up Java before running JVM SDK tests",
            errors,
        )
    require(
        JVM_SDK_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the JVM SDK test job",
        errors,
    )


def check_workflow_runs_swift_sdk_parse(errors):
    workflow = read(WORKFLOW_PATH)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    command_match = workflow_command_match(workflow, SWIFT_SDK_PARSE_COMMAND)
    require(
        re.search(rf"(?m)^  {re.escape(SWIFT_SDK_JOB)}:\s*$", workflow) is not None,
        "Kagemusha payload workflow must define the Swift SDK parse job",
        errors,
    )
    require(
        re.search(r"(?m)^\s+runs-on:\s+macos-latest\s*$", workflow) is not None,
        "Kagemusha payload workflow must run the Swift SDK parse job on macOS",
        errors,
    )
    require(
        command_match is not None,
        "Kagemusha payload workflow must run the Swift recursive spend SDK parse check",
        errors,
    )
    require(
        SWIFT_SDK_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the Swift SDK parse job",
        errors,
    )


def check_workflow_runs_csharp_sdk_tests(errors):
    workflow = read(WORKFLOW_PATH)
    job_block = workflow_job_block(workflow, CSHARP_SDK_JOB)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    command_match = workflow_command_match(job_block or "", CSHARP_SDK_TEST_COMMAND)
    dotnet_setup_match = re.search(
        r"(?m)^\s+- uses:\s+actions/setup-dotnet@v4\s*$",
        job_block or "",
    )
    dotnet_version_match = re.search(
        r"(?m)^\s+dotnet-version:\s+8\.0\.x\s*$",
        job_block or "",
    )
    require(
        job_block is not None,
        "Kagemusha payload workflow must define the C# SDK test job",
        errors,
    )
    if job_block is None:
        return
    require(
        dotnet_setup_match is not None,
        "Kagemusha payload workflow must set up dotnet for C# SDK tests",
        errors,
    )
    require(
        dotnet_version_match is not None,
        "Kagemusha payload workflow must pin dotnet 8 for C# SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Kagemusha payload workflow must run the C# recursive spend SDK tests",
        errors,
    )
    if dotnet_setup_match is not None and command_match is not None:
        require(
            dotnet_setup_match.start() < command_match.start(),
            "Kagemusha payload workflow must set up dotnet before running C# SDK tests",
            errors,
        )
    require(
        CSHARP_SDK_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the C# SDK test job",
        errors,
    )


def check_workflow_runs_javascript_sdk_tests(errors):
    workflow = read(WORKFLOW_PATH)
    job_block = workflow_job_block(workflow, JS_SDK_JOB)
    benchmark_needs = workflow_job_needs(workflow, MAIN_JOB)
    require(
        job_block is not None,
        "Kagemusha payload workflow must define the JavaScript SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-node@v4\s*$", job_block)
    node_version_match = re.search(r'(?m)^\s+node-version:\s+"20"\s*$', job_block)
    node_cache_match = re.search(
        r"(?m)^\s+cache-dependency-path:\s+javascript/iroha_js/package-lock\.json\s*$",
        job_block,
    )
    install_match = workflow_command_match(job_block, JS_SDK_INSTALL_COMMAND)
    test_match = workflow_command_match(job_block, JS_SDK_TEST_COMMAND)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Kagemusha payload workflow must run JavaScript SDK tests on Ubuntu",
        errors,
    )
    require(
        setup_match is not None,
        "Kagemusha payload workflow must set up Node for JavaScript SDK tests",
        errors,
    )
    require(
        node_version_match is not None,
        "Kagemusha payload workflow must pin Node 20 for JavaScript SDK tests",
        errors,
    )
    require(
        node_cache_match is not None,
        "Kagemusha payload workflow must cache JavaScript SDK dependencies by package-lock",
        errors,
    )
    require(
        install_match is not None,
        "Kagemusha payload workflow must install JavaScript SDK dependencies",
        errors,
    )
    require(
        test_match is not None,
        "Kagemusha payload workflow must run the JavaScript recursive spend SDK tests",
        errors,
    )
    if setup_match is not None and install_match is not None:
        require(
            setup_match.start() < install_match.start(),
            "Kagemusha payload workflow must set up Node before installing JavaScript SDK dependencies",
            errors,
        )
    if install_match is not None and test_match is not None:
        require(
            install_match.start() < test_match.start(),
            "Kagemusha payload workflow must install JavaScript SDK dependencies before running JavaScript tests",
            errors,
        )
    require(
        JS_SDK_JOB in benchmark_needs,
        "Kagemusha payload benchmark job must wait for the JavaScript SDK test job",
        errors,
    )


def check_javascript_sdk_script(errors):
    script = read(JS_SDK_TEST_COMMAND)
    require(
        'NODE_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN:-}"' in script,
        "Kagemusha JavaScript SDK script must keep the documented Node override variable",
        errors,
    )
    require(
        "resolve_node_20_bin()" in script and "is_node_20_bin()" in script,
        "Kagemusha JavaScript SDK script must resolve Node 20 before falling back to node",
        errors,
    )
    require(
        'NODE_BIN="$(resolve_node_20_bin)"' in script,
        "Kagemusha JavaScript SDK script must use the Node 20 resolver",
        errors,
    )
    require(
        'NODE_VERSION="$("${NODE_BIN}" --version)"' in script,
        "Kagemusha JavaScript SDK script must print the selected Node version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${NODE_VERSION}"' in script,
        "Kagemusha JavaScript SDK script must emit the selected Node version",
        errors,
    )
    require(
        "v20.*) ;;" in script,
        "Kagemusha JavaScript SDK script must reject non-Node-20 runtimes",
        errors,
    )
    require(
        "Kagemusha recursive spend|Kagemusha record-backed|Kagemusha .* SDK runner|browser crypto exposes native-only helpers as safe stubs|buildKagemusha" in script
        and "privacy native availability probes build and verify with Norito request archives" in script
        and "privacy native wrappers require binary Norito request archives" in script
        and "test/crypto.browser.test.js" in script
        and "test/kagemushaFfiContractParity.test.js" in script
        and "test/kagemushaRecursiveSpend.test.js" in script
        and "test/package_dist.test.js" in script
        and "test/privacyNative.test.js" in script
        and "test/transactionBuilder.test.js" in script,
        "Kagemusha JavaScript SDK script must run recursive spend, browser-stub, privacy native, package-dist, transaction-builder, and runtime-gate meta tests",
        errors,
    )


def check_js_parity_meta_test(errors):
    text = read(JS_PARITY_TEST_PATH)
    require(
        "recursive Kagemusha SDK parity negative controls fail when drift is undetected" in text,
        "JavaScript recursive Kagemusha parity meta-test must pin SDK parity negative controls",
        errors,
    )
    for _label, command in SDK_PARITY_NEGATIVE_CONTROL_COMMANDS:
        prefix = f"{SDK_PARITY_MAIN_COMMAND} "
        mode = command[len(prefix):] if command.startswith(prefix) else command
        require(
            f'"{mode}"' in text,
            f"JavaScript recursive Kagemusha parity meta-test must pin {mode}",
            errors,
        )


def check_c_bridge(texts, errors):
    rust = texts["crates/connect_norito_bridge/src/lib.rs"]
    header = texts["crates/connect_norito_bridge/include/connect_norito_bridge.h"]
    umbrella = texts["crates/connect_norito_bridge/include/NoritoBridge.h"]

    rust_exports = names_from_matches(
        rust,
        r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
        r"(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\(",
    )
    header_exports = names_from_matches(
        header,
        r"int32_t\s+(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\(",
    )
    require_same_set(rust_exports, REQUIRED_C_SYMBOLS, "Rust C recursive Kagemusha exports", errors)
    require_same_set(header_exports, REQUIRED_C_SYMBOLS, "C header recursive Kagemusha declarations", errors)
    rust_record_exports = names_from_matches(
        rust,
        r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
        r"(connect_norito_kagemusha_(?:prove_verified_compact_payment_token_with_records|prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes))\s*\(",
    )
    header_record_exports = names_from_matches(
        header,
        r"int32_t\s+(connect_norito_kagemusha_(?:prove_verified_compact_payment_token_with_records|prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes))\s*\(",
    )
    require_same_set(
        rust_record_exports,
        REQUIRED_RECORD_BACKED_KAGEMUSHA_C_SYMBOLS,
        "Rust C record-backed Kagemusha prover exports",
        errors,
    )
    require_same_set(
        header_record_exports,
        REQUIRED_RECORD_BACKED_KAGEMUSHA_C_SYMBOLS,
        "C header record-backed Kagemusha prover declarations",
        errors,
    )
    require(
        re.search(
            r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_bridge_abi_version\s*\(',
            rust,
        )
        is not None,
        "Rust C bridge missing connect_norito_bridge_abi_version export",
        errors,
    )
    require(
        re.search(
            r"uint32_t\s+connect_norito_bridge_abi_version\s*\(\s*void\s*\)\s*;",
            header,
        )
        is not None,
        "C header missing connect_norito_bridge_abi_version declaration",
        errors,
    )
    require('#include "connect_norito_bridge.h"' in umbrella, "NoritoBridge.h must include connect_norito_bridge.h", errors)
    require_regex(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*7\s*;",
        "C bridge ABI version",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        (
            "zero-envelope Pallas archive",
            "zero-envelope nested Pallas archives",
        ),
        "Rust C recursive spend nested Pallas guard",
        errors,
    )

    for package in ("android", "sdk"):
        for method in JNI_METHODS:
            symbol = f"Java_org_hyperledger_iroha_{package}_offline_KagemushaRecursiveSpendProver_{method}"
            require(symbol in rust, f"JNI export missing {symbol}", errors)


def check_recursive_compact_surface(texts, errors):
    rust = texts["crates/connect_norito_bridge/src/lib.rs"]
    header = texts["crates/connect_norito_bridge/include/connect_norito_bridge.h"]

    rust_exports = names_from_matches(
        rust,
        r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
        r"(connect_norito_kagemusha_(?:prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes|verify_recursive_compact_payment_token))\s*\(",
    )
    header_exports = names_from_matches(
        header,
        r"int32_t\s+(connect_norito_kagemusha_(?:prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes|verify_recursive_compact_payment_token))\s*\(",
    )
    require_same_set(
        rust_exports,
        REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS,
        "Rust recursive compact C bridge exports",
        errors,
    )
    require_same_set(
        header_exports,
        REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS,
        "C header recursive compact declarations",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/include/connect_norito_bridge.h",
        (
            "uint8_t* out_valid",
            "KagemushaRecursiveCompactVerifierKeysV1",
            "Malformed archives and malformed token bindings return ERR_KAGEMUSHA_PROVE.",
            "Shape-valid tokens with invalid proof bodies return success with `*out_valid = 0`.",
        ),
        "C header recursive compact verifier contract",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        (
            "*out_valid = 0",
            "is_kagemusha_recursive_compact_unavailable_error",
            "preverify_kagemusha_recursive_compact_payment_token",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
            "KagemushaRecursiveCompactKeyArtifactsV1",
            "KagemushaRecursiveCompactVerifierKeysV1",
            "recursive_compact_key_artifacts_norito_ptr",
            "recursive_compact_verifier_keys_norito_ptr",
            "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
            "malformed Pallas opening archives before proving",
            "detached valid Pallas opening archives before proving",
            "kagemusha_recursive_compact_ffi_rejects_windowed_records_before_unavailable",
            "windowed recursive compact verifier records must reject before proving",
            "height-windowed recursive compact record bundles must clear stale output lengths",
            "valid multi-hop recursive compact Pallas archives must produce a package-backed token",
            "shape-valid ABI-7 compact tokens with invalid proof bodies must return a soft invalid result",
            "sentinel-spoofed compact token",
            "must not spoof the unavailable sentinel through interpolated circuit ids",
            "shape-valid envelopes with stale folded-token bindings must hard-fail before soft invalid",
            "malformed public-input bindings before returning a soft invalid result",
            "oversized recursive compact record-bundle input must clear stale output pointers",
            "oversized recursive compact Pallas envelope input must clear stale output pointers",
            "KagemushaRecursiveCompactUnavailable",
        ),
        "Rust recursive compact verifier contract",
        errors,
    )
    require_regex(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        r"fn\s+is_kagemusha_recursive_compact_unavailable_error\(err:\s*&str\)\s*->\s*bool\s*\{\s*"
        r"matches!\(\s*err,\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE"
        r"\s*\|\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE\s*\)\s*\}",
        "Rust recursive compact unavailable classifier",
        errors,
    )
    require_regex(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        r"connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
        r"[\s\S]*prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts"
        r"\(\s*&record_bundle,\s*&pallas_open_envelopes_archive,\s*&key_artifacts,",
        "Rust recursive compact C package-backed Pallas prover",
        errors,
    )
    require_regex(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        r"fn\s+java_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
        r"[\s\S]*prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts"
        r"\(\s*&record_bundle,\s*pallas_open_envelopes_archive,\s*&key_artifacts,",
        "Rust recursive compact JNI package-backed Pallas prover",
        errors,
    )
    require_contains(
        texts,
        "crates/iroha_core/src/zk.rs",
        (
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
            "multi-hop proving requires the append verifier batch to be composed into the compact proof",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN",
        ),
        "Rust recursive compact one-hop and multi-hop diagnostics",
        errors,
    )
    for package in ("android", "sdk"):
        for method in REQUIRED_RECURSIVE_COMPACT_JNI_METHODS:
            symbol = (
                "Java_org_hyperledger_iroha_"
                f"{package}_offline_KagemushaRecursiveCompactPaymentTokenProver_{method}"
            )
            require(symbol in rust, f"recursive compact JNI export missing {symbol}", errors)

    require_contains(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        (
            "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
            "Project a recursive spend bundle into an ABI-7 recursive compact-token archive.",
            "Verify a projected recursive spend compact-token archive against a lineage verifier record.",
            "KagemushaRecursiveSpendBundleV1",
            "KagemushaCompactPaymentToken",
            "VerifyingKeyRecord",
            "kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "verify_kagemusha_recursive_spend_compact_payment_token_projection_archives",
            "java_kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "nativeRecursiveSpendCompactPaymentTokenFromBundle",
            "kagemusha_recursive_spend_compact_projection_ffi_returns_bound_token",
            "kagemusha_recursive_spend_compact_projection_verifier_ffi_rejects_malformed_inputs",
        ),
        "Rust recursive spend compact projection bridge",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/include/connect_norito_bridge.h",
        (
            "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
            "KagemushaRecursiveSpendBundleV1",
            "KagemushaCompactPaymentToken",
            "VerifyingKeyRecord",
            "out_compact_token_ptr",
            "out_valid",
        ),
        "C header recursive spend compact projection declaration",
        errors,
    )
    require_contains(
        texts,
        "crates/iroha_js_host/src/lib.rs",
        [f'js_name = "{name}"' for name in REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_METHODS]
        + [
            'js_name = "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight"',
            "Project a recursive spend bundle into an ABI-7 recursive compact Kagemusha payment token.",
            "Verify a projected recursive spend compact Kagemusha payment token against a lineage verifier record.",
            "KagemushaRecursiveSpendBundleV1",
            "VerifyingKeyRecord",
            "kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "verify_kagemusha_recursive_spend_compact_payment_token_projection_inner",
            "serialize Kagemusha recursive spend compact payment-token archive",
            "kagemusha_recursive_spend_compact_projection_js_host_binds_bundle",
            "kagemusha_recursive_spend_compact_projection_verifier_js_host_rejects_malformed_inputs",
            "forged recursive proof public-input binding must reject",
        ],
        "Node recursive spend compact projection export",
        errors,
    )

    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_PUBLIC_EXPORTS
            + (
                "hasKagemushaRecursiveSpendCompactPaymentTokenProjectionNative",
                "hasKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNative",
                'typeof native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle !== "function"',
                'typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection !== "function"',
                'typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight !== "function"',
                "native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
                "native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(",
                "native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
                "const checkedBlockHeight = normalizeKagemushaBlockHeight(blockHeight);",
                "checkedBlockHeight === null",
                "normalizeKagemushaBlockHeight(blockHeight)",
                "blockHeight must be a number or bigint",
                "blockHeight must be an integer",
                "blockHeight must be non-negative",
                "blockHeight number must be a safe integer; use bigint for larger u64 values",
                "blockHeight must fit in u64",
                '"bundleArchive"',
                '"verifierRecordArchive"',
                "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7 with the compact projection symbol",
                "recursive spend compact Kagemusha payment-token projection verifier requires native bridge ABI 7 with the compact projection verifier symbols",
                "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result",
            ),
            "JavaScript recursive spend compact projection gate",
            errors,
        )
    for relative in (
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_PUBLIC_EXPORTS
            + (
                "return false;",
                'unsupported("kagemushaRecursiveSpendCompactPaymentTokenFromBundle")',
                'unsupported("kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection")',
            ),
            "JavaScript browser recursive spend compact projection stubs",
            errors,
        )
    for relative in ("javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_PUBLIC_EXPORTS,
            f"{relative} recursive spend compact projection re-exports",
            errors,
        )
    require_contains(
        texts,
        "javascript/iroha_js/index.d.ts",
        (
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(): boolean",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(): boolean",
            "kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
            "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(",
            "blockHeight?: number | bigint | null",
        ),
        "JavaScript recursive spend compact projection declarations",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        (
            "Kagemusha recursive spend compact projection probes availability and validates native output",
            "Kagemusha recursive spend compact projection verifier probes and delegates",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
            "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
            "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
            "bundleArchive must be a valid Norito archive",
            "bundleArchive must contain a non-empty Norito payload",
            "verifierRecordArchive must be a valid Norito archive",
            "compact projection symbol",
            "compact projection verifier symbols",
            "rejectMalformedProbe(\"recursive-spend-compact-projection\"",
            "0xffff_ffff_ffff_ffffn",
            "0x1_0000_0000_0000_0000n",
            "Number.MAX_SAFE_INTEGER + 1",
            "blockHeight must be a number or bigint",
            "blockHeight must be an integer",
            "blockHeight must be non-negative",
            "blockHeight number must be a safe integer; use bigint for larger u64 values",
            "blockHeight must fit in u64",
        ),
        "JavaScript recursive spend compact projection tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/crypto.browser.test.js",
        (
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
            "browser build must not expose native recursive spend compact projection",
            "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
        ),
        "JavaScript browser recursive spend compact projection tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_JS_PUBLIC_EXPORTS,
        "JavaScript package recursive spend compact projection exports",
        errors,
    )

    require_contains(
        texts,
        "crates/iroha_js_host/src/lib.rs",
        [f'js_name = "{name}"' for name in REQUIRED_RECURSIVE_COMPACT_JS_METHODS]
        + [
            "napi::Result<bool>",
            "Ok(false)",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
            "preverify_kagemusha_recursive_compact_payment_token",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "Kagemusha recursive compact Pallas open-envelope archive",
            "invalid Kagemusha recursive compact record-backed Pallas preflight",
            "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
            "ensure_kagemusha_recursive_archive_len",
            "encoded Kagemusha archive exceeds",
            "Kagemusha record bundle archive must not exceed",
            "pallasOpenEnvelopesArchive must not exceed",
            "Kagemusha recursive compact payment token archive must not exceed",
            "detached valid recursive compact Pallas archive must reject",
            "valid multi-hop recursive compact archive must produce a token",
            "public-input hash mismatch",
            "sentinel-spoofed recursive compact token must reject",
            "circuit id `forged::",
        ],
        "Node recursive compact verifier export",
        errors,
    )
    require_regex(
        texts,
        "crates/iroha_js_host/src/lib.rs",
        r"fn\s+is_kagemusha_recursive_compact_unavailable_error\(err:\s*&str\)\s*->\s*bool\s*\{\s*"
        r"matches!\(\s*err,\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE"
        r"\s*\|\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE\s*\)\s*\}",
        "Node recursive compact unavailable classifier",
        errors,
    )

    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_COMPACT_JS_PUBLIC_EXPORTS
            + (
                "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7",
                '"kagemusha-recursive-compact-v1"',
                "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
                "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
                "isKagemushaRecursiveCompactUnavailable(error)",
                "hasKagemushaRecursiveCompactPaymentTokenVerifierNative",
                'typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"',
                "recursiveCompactVerifierKeysArchive",
                "/\\b(?:archive|Norito|probe)\\b/i.test(error.message)",
                "toOwnedKagemushaArchiveBuffer",
                'const compactToken = toOwnedKagemushaArchiveBuffer(',
                'const recursiveCompactVerifierKeys = toOwnedKagemushaArchiveBuffer(',
                '"compactTokenArchive"',
                '"recursiveCompactVerifierKeysArchive"',
                "recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol",
                "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
            ),
            "JavaScript recursive compact verifier gate",
            errors,
        )
    for relative in (
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_COMPACT_JS_PUBLIC_EXPORTS
            + (
                "return false;",
                "isKagemushaRecursiveCompactUnavailable(error)",
                'unsupported("kagemushaVerifyRecursiveCompactPaymentToken")',
            ),
            "JavaScript browser recursive compact stubs",
            errors,
        )
    for relative in ("javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"):
        require_contains(
            texts,
            relative,
            REQUIRED_RECURSIVE_COMPACT_JS_PUBLIC_EXPORTS,
            f"{relative} recursive compact re-exports",
            errors,
        )
    require_contains(
        texts,
        "javascript/iroha_js/index.d.ts",
        (
            "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION: 7",
            "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1:",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT:",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT:",
            "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(): boolean",
            "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(): boolean",
            "isKagemushaRecursiveCompactUnavailable(error: unknown): boolean",
            "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(",
            "kagemushaVerifyRecursiveCompactPaymentToken(",
        ),
        "JavaScript recursive compact TypeScript declarations",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        (
            "kagemushaVerifyRecursiveCompactPaymentToken",
            "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
            "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(), true",
            "recursive compact Kagemusha payment-token verifier requires native bridge ABI 7",
            "with the compact verifier symbol",
            "compactTokenArchive must be a valid Norito archive",
            "compactTokenArchive must contain a non-empty Norito payload",
            "recursive compact proof composition unavailable",
            "Kagemusha recursive compact proof unavailable",
            "Kagemusha recursive compact verifier unavailable",
            "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
            "isKagemushaRecursiveCompactUnavailable(",
        ),
        "JavaScript recursive compact verifier tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "kagemushaVerifyRecursiveCompactPaymentToken",
            "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
            "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
            "isKagemushaRecursiveCompactUnavailable",
        ),
        "JavaScript package recursive compact verifier exports",
        errors,
    )

    wrapper = "python/iroha_python/src/iroha_python/kagemusha.py"
    init = "python/iroha_python/src/iroha_python/__init__.py"
    host = "python/iroha_python/iroha_python_rs/src/lib.rs"
    require_contains(
        texts,
        wrapper,
        REQUIRED_RECURSIVE_COMPACT_PYTHON_PUBLIC_METHODS
        + (
            "_RECURSIVE_COMPACT_TOKEN_METHOD",
            '"kagemusha_prove_verified_recursive_compact_payment_token"',
            '"_with_records_and_pallas_open_envelopes"',
            "_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD",
            '"kagemusha_verify_recursive_compact_payment_token"',
            "globals()[_RECURSIVE_COMPACT_TOKEN_METHOD]",
            "globals()[_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD]",
            "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7",
            '"kagemusha-recursive-compact-v1"',
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
            "def is_kagemusha_recursive_compact_unavailable",
            "bridge ABI 7 with compact prover and verifier symbols",
            "bridge ABI 7 with the compact verifier symbol",
            '("archive", "norito", "probe")',
            '_assert_kagemusha_norito_archive(compact_token, "compact_token_archive")',
            "returned non-boolean result",
        ),
        "Python recursive compact verifier surface",
        errors,
    )
    wrapper_text = texts[wrapper]
    require(
        re.search(
            r"_probe_native_archive_method\(\s*module,\s*_RECURSIVE_COMPACT_TOKEN_METHOD,\s*"
            r"_MALFORMED_NATIVE_PROBE_ARCHIVE,\s*_MALFORMED_NATIVE_PROBE_ARCHIVE,\s*"
            r"_MALFORMED_NATIVE_PROBE_ARCHIVE,\s*\)",
            wrapper_text,
        )
        is not None,
        "Python recursive compact prover availability probe must pass record, Pallas, and key-artifact probe archives",
        errors,
    )
    require(
        len(
            re.findall(
                r"_probe_native_archive_method\(\s*module,\s*_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,\s*"
                r"_MALFORMED_NATIVE_PROBE_ARCHIVE,\s*_MALFORMED_NATIVE_PROBE_ARCHIVE,\s*\)",
                wrapper_text,
            )
        )
        >= 2,
        "Python recursive compact verifier availability probes must pass compact-token and verifier-key probe archives",
        errors,
    )
    require_contains(
        texts,
        init,
        REQUIRED_RECURSIVE_COMPACT_PYTHON_PUBLIC_METHODS
        + REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS,
        "Python package recursive compact re-exports",
        errors,
    )
    require_contains(
        texts,
        host,
        [f'name = "{name}"' for name in REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS]
        + [
            "preverify_kagemusha_recursive_compact_payment_token",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "Kagemusha recursive compact Pallas open-envelope archive",
            "invalid Kagemusha recursive compact record-backed Pallas preflight",
            "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
            "ensure_kagemusha_recursive_archive_len",
            "encoded Kagemusha archive exceeds",
            "Kagemusha recursive compact record bundle archive must not exceed",
            "pallas_open_envelopes_archive must not exceed",
            "Kagemusha recursive compact payment token archive must not exceed",
            "detached valid Pallas archive",
            "valid multi-hop recursive compact archive must produce a token",
            "public-input hash mismatch",
            "sentinel-spoofed recursive compact token must reject",
            "circuit id `forged::",
        ],
        "Python PyO3 recursive compact exports",
        errors,
    )
    require_regex(
        texts,
        host,
        r"fn\s+is_kagemusha_recursive_compact_unavailable_error\(err:\s*&str\)\s*->\s*bool\s*\{\s*"
        r"matches!\(\s*err,\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE"
        r"\s*\|\s*iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE\s*\)\s*\}",
        "Python PyO3 recursive compact unavailable classifier",
        errors,
    )
    for name in REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS:
        require_regex(
            texts,
            host,
            rf"wrap_pyfunction!\s*\(\s*{name}_py\s*,\s*module\s*\)",
            f"Python PyO3 module registration for {name}",
            errors,
        )
    require_contains(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        (
            "kagemusha_verify_recursive_compact_payment_token",
            "is_kagemusha_recursive_compact_payment_token_verifier_available",
            "compact_token_archive must be a valid Norito archive",
            "compact_token_archive must contain a non-empty Norito payload",
            "proof composition unavailable",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
            "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
            "is_kagemusha_recursive_compact_unavailable",
            "Kagemusha recursive compact proof unavailable",
            "Kagemusha recursive compact verifier unavailable",
            "returned non-boolean result",
        ),
        "Python recursive compact verifier tests",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_PUBLIC_METHODS
        + (
            "_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD",
            '"kagemusha_recursive_spend_compact_payment_token_from_bundle"',
            "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD",
            '"kagemusha_verify_recursive_spend_compact_payment_token_projection"',
            "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD",
            '"kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
            "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
            "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
            "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD]",
            "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD]",
            "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD]",
            "_verify_recursive_spend_compact_payment_token_projection_at_height",
            '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
            '_assert_kagemusha_norito_archive(verifier_record, "verifier_record_archive")',
            "_validate_kagemusha_block_height",
            "block_height must be an integer",
            "block_height must be non-negative",
            "block_height must fit in u64",
            "native bridge ABI 7 with the compact projection symbol",
            "compact projection verifier symbols",
        ),
        "Python recursive spend compact projection surface",
        errors,
    )
    require_contains(
        texts,
        init,
        REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_PUBLIC_METHODS,
        "Python package recursive spend compact projection re-exports",
        errors,
    )
    require_contains(
        texts,
        host,
        [f'name = "{name}"' for name in REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS]
        + [
            'name = "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
            "KagemushaRecursiveSpendBundleV1",
            "KagemushaCompactPaymentToken",
            "VerifyingKeyRecord",
            "kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "verify_kagemusha_recursive_spend_compact_payment_token_projection_inner",
            "failed to encode Kagemusha recursive spend compact payment token",
            "kagemusha_recursive_spend_compact_projection_python_binds_bundle",
            "kagemusha_recursive_spend_compact_projection_verifier_python_rejects_malformed_inputs",
            "forged recursive proof public-input binding must reject",
        ],
        "Python PyO3 recursive spend compact projection export",
        errors,
    )
    for name in REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS:
        require_regex(
            texts,
            host,
            rf"wrap_pyfunction!\s*\(\s*{name}_py\s*,\s*module\s*\)",
            f"Python PyO3 module registration for {name}",
            errors,
        )
    require_contains(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        (
            "test_recursive_spend_compact_projection_probes_and_delegates",
            "test_recursive_spend_compact_projection_verifier_probes_and_delegates",
            "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
            "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
            "kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "kagemusha_verify_recursive_spend_compact_payment_token_projection",
            "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
            "bundle_archive must be a valid Norito archive",
            "bundle_archive must contain a non-empty Norito payload",
            "verifier_record_archive must be a valid Norito archive",
            "block_height must be an integer",
            "block_height must be non-negative",
            "block_height must fit in u64",
            "compact projection symbol",
            "compact projection verifier symbols",
        ),
        "Python recursive spend compact projection tests",
        errors,
    )

    swift_wrapper = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"
    swift_bridge = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    require_contains(
        texts,
        swift_wrapper,
        (
            "KagemushaRecursiveCompactPaymentTokenProver",
            "requiredBridgeAbiVersion: UInt32 = 7",
            'recursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1"',
            "public static var isVerifierNativeAvailable",
            "isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
            "public static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "public static func verifyRecursiveCompactPaymentToken",
            "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
            "Kagemusha recursive compact-token archive must not be empty.",
            "oversizedRecordBundleArchive",
            "oversizedPallasOpenEnvelopesArchive",
            "Kagemusha verified fold record bundle archive must not exceed",
            "Kagemusha Pallas open-envelope archive must not exceed",
            "try requireValidInputArchive(",
            "try requireValidRecursiveCompactTokenArchive(token)",
            "requireValidRecursiveCompactTokenArchive(compactTokenArchive)",
            "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
            "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
            "Kagemusha recursive compact-token archive must be a valid Norito archive.",
            "Kagemusha recursive compact-token archive must contain a non-empty Norito payload.",
            "recursiveCompactUnavailable",
            "append verifier batch",
            "Kagemusha recursive compact-token archive was rejected by the native verifier.",
            "public static var isProjectionNativeAvailable",
            "public static var isProjectionVerifierNativeAvailable",
            "public static func recursiveSpendCompactPaymentTokenFromBundle",
            "public static func verifyRecursiveSpendCompactPaymentTokenProjection",
            "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable",
            "bridgeAvailable: NoritoNativeBridge.shared",
            ".isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
            "Kagemusha recursive spend bundle archive must be a valid Norito archive.",
            "Kagemusha recursive spend bundle archive must contain a non-empty Norito payload.",
            "Kagemusha verifier record archive must be a valid Norito archive.",
            "Kagemusha verifier record archive must contain a non-empty Norito payload.",
        ),
        "Swift recursive compact wrapper",
        errors,
    )
    require_contains(
        texts,
        swift_bridge,
        REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS
        + (
            "kagemushaVerifyRecursiveCompactPaymentTokenFn",
            "probeKagemushaRecursiveCompactPaymentTokenVerifierFunction",
            "kagemushaVerifyRecursiveCompactPaymentTokenFn != nil",
            "isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
            "kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk",
            "normalizeKagemushaRecursiveCompactVerifierOutput",
            "invalidKagemushaVerifierOutput",
            "copyKagemushaNativeArchiveOutput",
            "length <= CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)",
            "return try Self.copyKagemushaNativeArchiveOutput(",
            "kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
            "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn",
            "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
            "func kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
            "func verifyKagemushaRecursiveSpendCompactPaymentTokenProjection(",
            "bundleArchive: Data",
            "verifierRecordArchive: Data",
            "probeKagemushaArchiveFunction(kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn)",
            "probeKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierFunction",
        ),
        "Swift recursive compact bridge verifier probe",
        errors,
    )
    require_contains(
        texts,
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
        (
            "testVerifyRejectsEmptyCompactTokenArchiveBeforeBridgeCall",
            "testVerifyRejectsMalformedCompactTokenArchiveBeforeBridgeCall",
            "testVerifyRejectsOversizedCompactTokenArchiveBeforeBridgeCall",
            "testVerifyRejectsEmptyPayloadCompactTokenArchiveBeforeBridgeCall",
            "testRejectsMalformedInputArchivesBeforeBridgeCall",
            "testRejectsOversizedInputArchivesBeforeBridgeCall",
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "malformedKagemushaNoritoArchives",
            "compressed[22] = 0x01",
            "unsupportedFlags[39] = NoritoHeader.varintOffsets",
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "kagemushaNoritoFrameWithHeaderPadding",
            "Data([0x7f])",
            "Data(repeating: 0, count: 65)",
            ".oversizedRecordBundleArchive",
            ".oversizedPallasOpenEnvelopesArchive",
            ".oversizedCompactTokenArchive",
            "must not exceed",
            "testReturnsValidNativeOutput",
            "validKagemushaNoritoArchive",
            "testVerifyReturnsNativeBoolean",
            "testVerifyRequiresVerifierNativeAvailabilityAfterInputValidation",
            "testNativeBridgeRejectsInvalidVerifierBooleanOutput",
            "testNativeBridgeCopiesBoundedKagemushaOutputAndFreesNativePointer",
            "testNativeBridgeRejectsOversizedKagemushaOutputBeforeCopying",
            "valid: 2",
            "status: -312",
            "invalidKagemushaVerifierOutput",
            "testVerifyNilNativeResultIsBridgeUnavailable",
            "testNativeRecursiveCompactUnavailableIsDistinctFromProofRejection",
            "testVerifyNativeRejectionIsVerificationRejected",
            "testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall",
            "testProjectionRequiresBridgeAfterInputValidation",
            "testProjectionRejectsMalformedNativeOutput",
            "testProjectionReturnsValidNativeOutput",
            "testProjectionVerifierRejectsMalformedVerifierRecordBeforeBridgeCall",
            "testProjectionVerifierRequiresNativeAvailabilityAfterInputValidation",
            "testProjectionVerifierReturnsNativeBoolean",
            ".oversizedBundleArchive",
            ".invalidBundleArchive",
            ".emptyBundlePayload",
            ".invalidVerifierRecordArchive",
            ".emptyVerifierRecordPayload",
            ".oversizedVerifierRecordArchive",
        ),
        "Swift recursive compact verifier tests",
        errors,
    )
    require_regex(
        texts,
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
        r"testRejectsMalformedNativeOutput[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"testProjectionRejectsMalformedNativeOutput[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive compact native output header guard tests",
        errors,
    )
    require_regex(
        texts,
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
        r"testRejectsInvalidKeyArtifactsArchiveBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"testRejectsMalformedInputArchivesBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive compact input header guard tests",
        errors,
    )
    require_regex(
        texts,
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
        r"testProjectionVerifierRejectsMalformedVerifierRecordBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"testVerifyRejectsMalformedCompactTokenArchiveBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"testVerifyRejectsInvalidVerifierKeysArchiveBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)",
        "Swift recursive compact input header guard tests",
        errors,
    )

    jvm_files = (
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            "Kotlin recursive compact wrapper",
            "REQUIRED_BRIDGE_ABI_VERSION: Int = 7",
            "fun isVerifierNativeAvailable(): Boolean",
            "fun isProjectionVerifierNativeAvailable(): Boolean",
            "recursiveCompactVerifierKeysArchive: ByteArray?",
            "fun verifyRecursiveSpendCompactPaymentTokenProjection(",
            "fun verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
            "blockHeight: String?",
            "blockHeight: BigInteger?",
            "private fun parseUnsignedBlockHeight(blockHeight: String?): Long",
            "private fun parseUnsignedBlockHeight(blockHeight: BigInteger?): Long",
            "blockHeight must be a canonical unsigned decimal integer",
            "blockHeight must fit in u64",
            "private val nativeVerifierAvailable: Boolean = loadVerifierLibrary()",
            "private val nativeProjectionVerifierAvailable: Boolean = loadProjectionVerifierLibrary()",
            "check(nativeVerifierAvailable)",
            "check(nativeProjectionVerifierAvailable)",
            "private fun loadVerifierLibrary(): Boolean",
            "private fun loadProjectionVerifierLibrary(): Boolean",
            "val compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "recursiveCompactVerifierKeysArchive, \"recursiveCompactVerifierKeysArchive\"",
            "val verifierRecord = ownedNativeInput(verifierRecordArchive, \"verifierRecordArchive\")",
            "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
            "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
            "nativeVerifyRecursiveCompactPaymentToken(ByteArray(0), ByteArray(0))",
            "nativeVerifyRecursiveSpendCompactPaymentTokenProjection(ByteArray(0), ByteArray(0))",
            "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            "Android Java recursive compact wrapper",
            "REQUIRED_BRIDGE_ABI_VERSION = 7",
            "public static boolean isVerifierNativeAvailable()",
            "public static boolean isProjectionVerifierNativeAvailable()",
            "final byte[] recursiveCompactVerifierKeysArchive",
            "public static boolean verifyRecursiveSpendCompactPaymentTokenProjection(",
            "public static boolean verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
            "final String blockHeight",
            "final BigInteger blockHeight",
            "private static long parseUnsignedBlockHeight(final String blockHeight)",
            "private static long parseUnsignedBlockHeight(final BigInteger blockHeight)",
            "blockHeight must be a canonical unsigned decimal integer",
            "blockHeight must fit in u64",
            "NATIVE_VERIFIER_AVAILABLE = loadVerifierLibrary()",
            "NATIVE_PROJECTION_VERIFIER_AVAILABLE = loadProjectionVerifierLibrary()",
            "requireVerifierNative()",
            "requireProjectionVerifierNative()",
            "private static boolean loadVerifierLibrary()",
            "private static boolean loadProjectionVerifierLibrary()",
            "final byte[] compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "recursiveCompactVerifierKeysArchive, \"recursiveCompactVerifierKeysArchive\"",
            "final byte[] verifierRecord",
            "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
            "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
            "nativeVerifyRecursiveCompactPaymentToken(new byte[0], new byte[0])",
            "nativeVerifyRecursiveSpendCompactPaymentTokenProjection(",
            "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
        ),
    )
    for relative, label, *snippets in jvm_files:
        require_contains(
            texts,
            relative,
            (
                "KagemushaRecursiveCompactPaymentTokenProver",
                '"kagemusha-recursive-compact-v1"',
                "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
                "recursiveSpendCompactPaymentTokenFromBundle",
                "nativeVerifyRecursiveCompactPaymentToken",
                "nativeRecursiveSpendCompactPaymentTokenFromBundle",
                "nativeVerifyRecursiveSpendCompactPaymentTokenProjection",
                "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
                *snippets,
                "isRecursiveCompactUnavailable",
                "Kagemusha recursive compact proof composition is unavailable",
                "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
                "recursive compact-token prover/verifier is not available",
            ),
            label,
            errors,
        )
    require_regex(
        texts,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
        r"fun\s+verifyRecursiveCompactPaymentToken\([^)]*compactTokenArchive:\s+ByteArray\?[^)]*recursiveCompactVerifierKeysArchive:\s+ByteArray\?[^)]*\)\s*:\s*Boolean\s*\{[^}]*val\s+compactToken\s*=\s*ownedNativeInput\(compactTokenArchive,\s+\"compactTokenArchive\"\)[^}]*val\s+verifierKeys\s*=\s*ownedNativeInput\(recursiveCompactVerifierKeysArchive,\s+\"recursiveCompactVerifierKeysArchive\"\)",
        "Kotlin recursive compact archive input copy",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
        r"public\s+static\s+boolean\s+verifyRecursiveCompactPaymentToken\([^)]*final\s+byte\[\]\s+compactTokenArchive[^)]*final\s+byte\[\]\s+recursiveCompactVerifierKeysArchive[^)]*\)\s*\{[^}]*final\s+byte\[\]\s+compactToken\s*=\s*ownedNativeInput\(compactTokenArchive,\s+\"compactTokenArchive\"\)[^}]*final\s+byte\[\]\s+verifierKeys\s*=\s*ownedNativeInput\(\s*recursiveCompactVerifierKeysArchive,\s+\"recursiveCompactVerifierKeysArchive\"\)",
        "Android Java recursive compact archive input copy",
        errors,
        flags=re.S,
    )
    require_contains(
        texts,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
        ("KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable()",),
        "Kotlin recursive compact spend-mode selector",
        errors,
    )
    require_contains(
        texts,
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
        ("KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable()",),
        "Android Java recursive compact spend-mode selector",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        (
            "fn java_jlong_to_u64_bits(value: jni::sys::jlong) -> u64",
            "let height = block_height.map(java_jlong_to_u64_bits);",
            "kagemusha_recursive_spend_compact_projection_jni_height_uses_raw_u64_bits",
            "java_jlong_to_u64_bits(i64::MIN)",
            "java_jlong_to_u64_bits(-1), u64::MAX",
        ),
        "Rust JNI recursive compact projection raw u64 block-height carrier",
        errors,
    )
    require_not_regex(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        r"Some\(value\)\s+if\s+value\s*<\s*0\s*=>\s*return\s+Err\(\"blockHeight must be non-negative\"\.to_owned\(\)\)",
        "Rust JNI recursive compact projection block-height carrier",
        errors,
    )
    require_contains(
        texts,
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
        (
            "KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION",
            "KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable()",
            "KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable()",
            ".recursiveSpendCompactPaymentTokenFromBundle(ByteArray(0))",
            "KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(",
            "validRecursiveCompactVerifierKeys",
            "recursiveCompactVerifierKeysArchive must not be empty",
            ".verifyRecursiveSpendCompactPaymentTokenProjection(",
            ".verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
            "KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable",
            "isRecursiveCompactUnavailable(null)",
            "IllegalArgumentException()",
            "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
            "public instance column 0 must contain exactly one row; found 2",
            "envelope verifier-key hash mismatch",
            "valid Norito archive",
            "non-empty Norito payload",
            "bundleArchive must be a valid Norito archive",
            "bundleArchive must contain a non-empty Norito payload",
            "verifierRecordArchive must be a valid Norito archive",
            "verifierRecordArchive must contain a non-empty Norito payload",
            "blockHeight must be non-negative",
            "Long.MAX_VALUE",
            "\"9223372036854775808\"",
            "BigInteger(\"18446744073709551615\")",
            "\"18446744073709551616\"",
            "blockHeight must be a canonical unsigned decimal integer",
            "blockHeight must fit in u64",
            "blockHeight must not be null",
        ),
        "Kotlin recursive compact verifier tests",
        errors,
    )
    require_contains(
        texts,
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
        (
            "KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION",
            "KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable()",
            "KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable()",
            ".recursiveSpendCompactPaymentTokenFromBundle(new byte[0])",
            "KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(",
            "validRecursiveCompactVerifierKeys",
            "recursiveCompactVerifierKeysArchive must not be empty",
            ".verifyRecursiveSpendCompactPaymentTokenProjection(",
            ".verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
            "KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable",
            "isRecursiveCompactUnavailable(null)",
            "new IllegalArgumentException())",
            "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
            "public instance column 0 must contain exactly one row; found 2",
            "envelope verifier-key hash mismatch",
            "compactTokenArchive must be a valid Norito archive",
            "compactTokenArchive must contain a non-empty Norito payload",
            "bundleArchive must be a valid Norito archive",
            "bundleArchive must contain a non-empty Norito payload",
            "verifierRecordArchive must be a valid Norito archive",
            "verifierRecordArchive must contain a non-empty Norito payload",
            "blockHeight must be non-negative",
            "Long.MAX_VALUE",
            "\"9223372036854775808\"",
            "new BigInteger(\"18446744073709551615\")",
            "\"18446744073709551616\"",
            "blockHeight must be a canonical unsigned decimal integer",
            "blockHeight must fit in u64",
            "blockHeight must not be null",
        ),
        "Android Java recursive compact verifier tests",
        errors,
    )

    csharp = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    require_contains(
        texts,
        csharp,
        REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS
        + (
            "RecursiveCompactRequiredBridgeAbiVersion = 7",
            "IsRecursiveCompactPaymentTokenProverAvailable",
            "IsRecursiveCompactPaymentTokenVerifierAvailable",
            "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
            "TryProbeRecursiveCompactPaymentTokenSurface",
            "TryProbeRecursiveCompactPaymentTokenVerifierSymbol",
            "TryProbeRecursiveSpendCompactPaymentTokenProjectionSymbol",
            "TryProbeRecursiveSpendCompactPaymentTokenProjectionVerifierSymbol",
            "public static KagemushaRecursiveCompactPaymentTokenArchive RecursiveSpendCompactPaymentTokenFromBundle(",
            "ReadOnlySpan<byte> bundleArchive)",
            "ReadOnlySpan<byte> recursiveCompactVerifierKeysArchive)",
            "public static bool VerifyRecursiveSpendCompactPaymentTokenProjection(",
            "RequireValidInputArchive",
            "RequireValidRecursiveCompactTokenArchive(compactToken)",
            "PrivacyNative.IsNoritoV1Archive(compactTokenArchive)",
            "Record bundle archive",
            "Pallas open-envelopes archive",
            "must be a valid Norito archive.",
            "must contain a non-empty Norito payload.",
            "RequireValidNativeOutput(symbol, result)",
            "returned invalid Norito archive",
            "returned empty Norito payload",
            "Compact token archive must be a valid Norito archive.",
            "Compact token archive must contain a non-empty Norito payload.",
            "Recursive compact verifier keys archive",
            "RecursiveCompactUnavailableBridgeErrorCode = -312",
            "code == RecursiveCompactUnavailableBridgeErrorCode",
            "recursive compact proof composition",
            "out byte valid",
            "NativeRecursiveSpendCompactPaymentTokenFromBundle",
            "NativeVerifyRecursiveSpendCompactPaymentTokenProjection",
            "NativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
            "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
            "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
        ),
        "C# recursive compact wrapper",
        errors,
    )
    require_regex(
        texts,
        csharp,
        r"internal\s+static\s+bool\s+NormalizeRecursiveCompactVerifierOutput\(string\s+symbol,\s+int\s+code,\s+byte\s+valid\)\s*\{\s*if\s*\(code\s*!=\s*0\)\s*\{\s*if\s*\(code\s*==\s*RecursiveCompactUnavailableBridgeErrorCode\)",
        "C# recursive compact verifier unavailable mapping",
        errors,
        flags=re.S,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "IsRecursiveCompactPaymentTokenVerifierAvailable",
            "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
            "VerifyRecursiveCompactPaymentToken(",
            "validRecursiveCompactVerifierKeys",
            "Recursive compact verifier keys archive must not be empty",
            "VerifyRecursiveSpendCompactPaymentTokenProjection",
            "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
            "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
            "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
            "RecursiveSpendNativeReadBridgeOutputReportsRecursiveCompactUnavailable",
            "AssertRejectsMalformedBridgeOutput",
            "AssertRejectsMalformedBridgeOutput(compressed)",
            "AssertRejectsMalformedBridgeOutput(unsupportedFlags)",
            "AssertRejectsMalformedBridgeOutput(invalidFieldBitset)",
            "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
            "RecursiveSpendCompactProjectionVerifierRejectsInvalidInputsBeforeLoadingNativeBridge",
            "NormalizeRecursiveCompactVerifierOutput",
            "RecursiveCompactUnavailableBridgeErrorCode",
            "valid Norito archive",
            "non-empty Norito payload",
            "KagemushaNoritoFrameWithPayload",
            "KagemushaNoritoFrame",
        ),
        "C# recursive compact verifier tests",
        errors,
    )


def check_recursive_compact_sdk_key_package_arity(texts, errors):
    swift = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"
    kotlin = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"
    android = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"
    csharp = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    dts = "javascript/iroha_js/index.d.ts"

    forbidden = (
        (
            swift,
            r"recursiveCompactKeyArtifactsArchive:\s*Data\s*=\s*Data\s*\(",
            "Swift recursive compact prover public key-package argument",
        ),
        (
            swift,
            r"recursiveCompactVerifierKeysArchive:\s*Data\s*=\s*Data\s*\(",
            "Swift recursive compact verifier public key-package argument",
        ),
        (
            kotlin,
            r"fun\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
            r"\(\s*recordBundleArchive:\s*ByteArray\?,\s*pallasOpenEnvelopesArchive:\s*ByteArray\?,?\s*\)"
            r"\s*:\s*ByteArray",
            "Kotlin recursive compact prover public key-package arity",
        ),
        (
            kotlin,
            r"fun\s+verifyRecursiveCompactPaymentToken\s*"
            r"\(\s*compactTokenArchive:\s*ByteArray\?,?\s*\)\s*:\s*Boolean",
            "Kotlin recursive compact verifier public key-package arity",
        ),
        (
            android,
            r"public\s+static\s+byte\[\]\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
            r"\(\s*final\s+byte\[\]\s+recordBundleArchive\s*,\s*final\s+byte\[\]\s+pallasOpenEnvelopesArchive\s*\)",
            "Android Java recursive compact prover public key-package arity",
        ),
        (
            android,
            r"public\s+static\s+boolean\s+verifyRecursiveCompactPaymentToken\s*"
            r"\(\s*final\s+byte\[\]\s+compactTokenArchive\s*\)",
            "Android Java recursive compact verifier public key-package arity",
        ),
        (
            csharp,
            r"public\s+static\s+KagemushaRecursiveCompactPaymentTokenArchive\s+"
            r"ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
            r"\(\s*ReadOnlySpan<byte>\s+recordBundleArchive\s*,\s*ReadOnlySpan<byte>\s+pallasOpenEnvelopesArchive\s*\)",
            "C# recursive compact prover public key-package arity",
        ),
        (
            csharp,
            r"public\s+static\s+bool\s+VerifyRecursiveCompactPaymentToken\s*"
            r"\(\s*ReadOnlySpan<byte>\s+compactTokenArchive\s*\)",
            "C# recursive compact verifier public key-package arity",
        ),
        (
            dts,
            r"recursiveCompactKeyArtifactsArchive\?:\s*BinaryLike",
            "JavaScript TypeScript recursive compact prover key-package declaration",
        ),
        (
            dts,
            r"recursiveCompactVerifierKeysArchive\?:\s*BinaryLike",
            "JavaScript TypeScript recursive compact verifier key-package declaration",
        ),
        (
            dts,
            r"export\s+function\s+kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
            r"\(\s*recordBundleArchive:\s*BinaryLike\s*,\s*pallasOpenEnvelopesArchive:\s*BinaryLike\s*,?\s*\)"
            r"\s*:\s*Buffer",
            "JavaScript TypeScript recursive compact prover key-package arity",
        ),
        (
            dts,
            r"export\s+function\s+kagemushaVerifyRecursiveCompactPaymentToken\s*"
            r"\(\s*compactTokenArchive:\s*BinaryLike\s*,?\s*\)\s*:\s*boolean",
            "JavaScript TypeScript recursive compact verifier key-package arity",
        ),
    )
    for relative, pattern, label in forbidden:
        require_not_regex(texts, relative, pattern, label, errors, flags=re.S)

    require_regex(
        texts,
        swift,
        r"public\s+static\s+func\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
        r"\(\s*recordBundleArchive:\s*Data\s*,\s*pallasOpenEnvelopesArchive:\s*Data\s*,\s*"
        r"recursiveCompactKeyArtifactsArchive:\s*Data\s*\)\s*throws\s*->\s*Data",
        "Swift recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        swift,
        r"public\s+static\s+func\s+verifyRecursiveCompactPaymentToken\s*"
        r"\(\s*compactTokenArchive:\s*Data\s*,\s*recursiveCompactVerifierKeysArchive:\s*Data\s*\)"
        r"\s*throws\s*->\s*Bool",
        "Swift recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        kotlin,
        r"fun\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
        r"\(\s*recordBundleArchive:\s*ByteArray\?,\s*pallasOpenEnvelopesArchive:\s*ByteArray\?,\s*"
        r"recursiveCompactKeyArtifactsArchive:\s*ByteArray\?,\s*\)\s*:\s*ByteArray",
        "Kotlin recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        kotlin,
        r"fun\s+verifyRecursiveCompactPaymentToken\s*"
        r"\(\s*compactTokenArchive:\s*ByteArray\?,\s*recursiveCompactVerifierKeysArchive:\s*ByteArray\?,\s*\)"
        r"\s*:\s*Boolean",
        "Kotlin recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        android,
        r"public\s+static\s+byte\[\]\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
        r"\(\s*final\s+byte\[\]\s+recordBundleArchive\s*,\s*final\s+byte\[\]\s+pallasOpenEnvelopesArchive\s*,\s*"
        r"final\s+byte\[\]\s+recursiveCompactKeyArtifactsArchive\s*\)",
        "Android Java recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        android,
        r"public\s+static\s+boolean\s+verifyRecursiveCompactPaymentToken\s*"
        r"\(\s*final\s+byte\[\]\s+compactTokenArchive\s*,\s*final\s+byte\[\]\s+recursiveCompactVerifierKeysArchive\s*\)",
        "Android Java recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        csharp,
        r"public\s+static\s+KagemushaRecursiveCompactPaymentTokenArchive\s+"
        r"ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
        r"\(\s*ReadOnlySpan<byte>\s+recordBundleArchive\s*,\s*ReadOnlySpan<byte>\s+pallasOpenEnvelopesArchive\s*,\s*"
        r"ReadOnlySpan<byte>\s+recursiveCompactKeyArtifactsArchive\s*\)",
        "C# recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        csharp,
        r"public\s+static\s+bool\s+VerifyRecursiveCompactPaymentToken\s*"
        r"\(\s*ReadOnlySpan<byte>\s+compactTokenArchive\s*,\s*ReadOnlySpan<byte>\s+recursiveCompactVerifierKeysArchive\s*\)",
        "C# recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        dts,
        r"export\s+function\s+kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*"
        r"\(\s*recordBundleArchive:\s*BinaryLike\s*,\s*pallasOpenEnvelopesArchive:\s*BinaryLike\s*,\s*"
        r"recursiveCompactKeyArtifactsArchive:\s*BinaryLike\s*,\s*\)\s*:\s*Buffer\s*;",
        "JavaScript TypeScript recursive compact wrapper",
        errors,
        flags=re.S,
    )
    require_regex(
        texts,
        dts,
        r"export\s+function\s+kagemushaVerifyRecursiveCompactPaymentToken\s*"
        r"\(\s*compactTokenArchive:\s*BinaryLike\s*,\s*recursiveCompactVerifierKeysArchive:\s*BinaryLike\s*,\s*\)"
        r"\s*:\s*boolean\s*;",
        "JavaScript TypeScript recursive compact wrapper",
        errors,
        flags=re.S,
    )


def check_record_backed_javascript_surface(texts, errors):
    require_contains(
        texts,
        "crates/iroha_js_host/src/lib.rs",
        [f'js_name = "{name}"' for name in REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS]
        + [
            "prove_verified_kagemusha_compact_payment_token_from_record_bundle",
            "prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive",
            "KAGEMUSHA_FOLDED_CIRCUIT_ID",
            "KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID",
            "serialize Kagemusha compact payment-token archive",
            "serialize Kagemusha recursive aggregation proof-bundle archive",
        ],
        "Node record-backed Kagemusha prover exports",
        errors,
    )

    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS
            + (
                'typeof native.kagemushaProveVerifiedCompactPaymentTokenWithRecords !== "function"',
                "native.kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
                "native.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
                "toOwnedKagemushaArchiveBuffer",
                'const recordBundle = toOwnedKagemushaArchiveBuffer(',
                'const pallasOpenEnvelopes = toOwnedKagemushaArchiveBuffer(',
                '"recordBundleArchive"',
                '"pallasOpenEnvelopesArchive"',
                "Kagemusha compact payment-token prover requires native bridge ABI 6 with compact-token prover symbol",
                "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6 with recursive aggregation prover symbol",
                "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
                "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
            ),
            "JavaScript record-backed Kagemusha prover wrappers",
            errors,
        )
    for relative in (
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(
            texts,
            relative,
            REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS
            + (
                "return false;",
                'unsupported("kagemushaProveVerifiedCompactPaymentTokenWithRecords")',
                'unsupported(\n    "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes"',
            ),
            "JavaScript browser record-backed Kagemusha stubs",
            errors,
        )
    for relative in ("javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"):
        require_contains(
            texts,
            relative,
            REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS,
            f"{relative} record-backed Kagemusha re-exports",
            errors,
        )
    require_contains(
        texts,
        "javascript/iroha_js/index.d.ts",
        (
            "isKagemushaCompactPaymentTokenNativeAvailable(): boolean",
            "isKagemushaRecursiveAggregationProofBundleNativeAvailable(): boolean",
            "kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
            "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
        ),
        "JavaScript record-backed Kagemusha TypeScript declarations",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        (
            "Kagemusha record-backed JS builders probe availability and validate native output",
            "isKagemushaCompactPaymentTokenNativeAvailable",
            "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
            "recordBundleArchive must be a valid Norito archive",
            "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
            "returned invalid Norito archive",
            "returned empty Norito payload",
        ),
        "JavaScript record-backed Kagemusha runtime tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/crypto.browser.test.js",
        REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS
        + (
            "browser build must not expose native compact-token prover",
            "browser build must not expose native recursive aggregation prover",
        ),
        "JavaScript browser record-backed Kagemusha tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_PUBLIC_EXPORTS,
        "JavaScript package record-backed Kagemusha exports",
        errors,
    )


def check_rust_policy_constants(texts, errors):
    relative = "crates/iroha_data_model/src/offline/mod.rs"
    require_regex(
        texts,
        relative,
        r'KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1:\s*&str\s*=\s*"kagemusha-recursive-aggregation-v1"',
        "Rust aggregation circuit id",
        errors,
    )
    require_regex(
        texts,
        relative,
        r'KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1:\s*&str\s*=\s*"kagemusha-recursive-spend-lineage-v1"',
        "Rust Reserved-lineage circuit id",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1\s*:\s*u32\s*=\s*64\s*;",
        "Rust witnessless max hops",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1\s*:\s*bool\s*=\s*true\s*;",
        "Rust transition-circuit wired flag",
        errors,
    )
    require_contains(
        texts,
        relative,
        (
            "proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
            "hop_count >= 1",
            "hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "previous_hop_count >= 1",
            "previous_hop_count < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
        ),
        "Rust witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1\s*:\s*usize\s*=\s*1\s*;",
        "Rust previous-proof open envelope count",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES\s*:\s*usize\s*=\s*8\s*\*\s*1024\s*\*\s*1024\s*;",
        "Rust previous-proof open envelope max bytes",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES\s*:\s*usize\s*=\s*128\s*;",
        "Rust Pallas open-envelope transcript label max bytes",
        errors,
    )

def check_node_host(texts, errors):
    relative = "crates/iroha_js_host/src/lib.rs"
    require_contains(
        texts,
        relative,
        [f'js_name = "{name}"' for name in REQUIRED_JS_NATIVE_METHODS],
        "Node NAPI host",
        errors,
    )
    require_regex(
        texts,
        relative,
        r"pub\s+fn\s+connect_norito_bridge_abi_version\s*\(\)\s*->\s*u32\s*\{\s*7\s*\}",
        "Node NAPI bridge ABI version",
        errors,
    )


def check_jvm_sdk_script_pins_jdk21(texts, errors):
    script = read(JVM_SDK_TEST_COMMAND)
    require(
        'JAVA_HOME_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME:-}"' in script,
        "Kagemusha JVM SDK script must keep the documented Java home override variable",
        errors,
    )
    require(
        "KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME must point to a JDK 21 home." in script,
        "Kagemusha JVM SDK script must reject invalid explicit Java home overrides",
        errors,
    )
    require(
        "JAVA_HOME must point to a JDK 21 home for Kagemusha recursive spend JVM SDK tests." in script,
        "Kagemusha JVM SDK script must reject inherited non-JDK-21 JAVA_HOME values",
        errors,
    )
    require(
        "is_java_21_home()" in script,
        "Kagemusha JVM SDK script must validate Java homes as JDK 21",
        errors,
    )
    require(
        'version[[:space:]]+\\"21(\\.|\\")' in script,
        "Kagemusha JVM SDK script must match Java 21 version output",
        errors,
    )
    require(
        "java -version" in script,
        "Kagemusha JVM SDK script must print the selected Java version",
        errors,
    )
    for test_class in (
        "org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProverTest",
        "org.hyperledger.iroha.sdk.offline.KagemushaInstructionArchivesTest",
        "org.hyperledger.iroha.sdk.offline.OfflineNoteTest",
        "org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test",
        "org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest",
    ):
        require(
            f"--tests {test_class}" in script,
            f"Kagemusha JVM SDK script must run {test_class}",
            errors,
        )
    require(
        "KagemushaRecursiveAggregationProofBundleProver.java" in script,
        "Kagemusha JVM SDK script must compile the Android recursive aggregation prover wrapper",
        errors,
    )
    require(
        (
            "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest,"
            "org.hyperledger.iroha.android.offline.OfflineNoteV2Test,"
            "org.hyperledger.iroha.android.offline.OfflineNoteTest,"
            "org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest,"
            "org.hyperledger.iroha.android.tx.TransactionBuilderTests"
        )
        in script,
        "Kagemusha JVM SDK script must run the focused Android Kagemusha harness mains",
        errors,
    )
    require(
        ":core:test" in script
        and "--tests org.hyperledger.iroha.android.GradleHarnessTests" in script,
        "Kagemusha JVM SDK script must run the Android Gradle harness test",
        errors,
    )


def check_javascript(texts, errors):
    constants = (
        "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION = 6",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = true",
        "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1",
        "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024",
        "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128",
        'KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND = "halo2/ipa"',
        '"kagemusha-recursive-aggregation-v1"',
        '"kagemusha-recursive-spend-lineage-v1"',
        '"iroha:kagemusha:v1:recursive-spend-transition-profile"',
        '"iroha:kagemusha:v1:recursive-spend-transition-profile-digest"',
        '"iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"',
        '"iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"',
        '"iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"',
        '"iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"',
        '"iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"',
    )
    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(texts, relative, constants, f"{relative} constants", errors)
        require_contains(texts, relative, REQUIRED_JS_PUBLIC_EXPORTS, f"{relative} public API", errors)

    for relative in ("javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"):
        require_contains(
            texts,
            relative,
            (
                "const KAGEMUSHA_MAX_BRIDGE_ABI_VERSION = 0xffff_ffff",
                "Number.isSafeInteger(version)",
                "version >= 0",
                "version <= KAGEMUSHA_MAX_BRIDGE_ABI_VERSION",
            ),
            f"{relative} Kagemusha ABI probe bounds",
            errors,
        )

    for relative in (
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        "javascript/iroha_js/test/package_dist.test.js",
    ):
        require_contains(
            texts,
            relative,
            (
                "Number.NaN",
                "Number.POSITIVE_INFINITY",
                "Number.MAX_SAFE_INTEGER + 1",
                "0x1_0000_0000",
                "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false",
                "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(),",
            ),
            f"{relative} Kagemusha ABI probe test coverage",
            errors,
        )

    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "package dist Kagemusha recursive compact requires key packages before native dispatch",
            "recursiveCompactKeyArtifactsArchive must be a Buffer, string, or ArrayBuffer view",
            "recursiveCompactKeyArtifactsArchive must not be empty",
            "recursiveCompactVerifierKeysArchive must be a Buffer, string, or ArrayBuffer view",
            "recursiveCompactVerifierKeysArchive must not be empty",
            "assert.notStrictEqual(calls[0][1][2], keyArtifacts)",
            "assert.notStrictEqual(calls[1][1][1], verifierKeys)",
        ),
        "JavaScript package dist recursive compact key-package dispatch coverage",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "package declarations expose recursive compact key-package signatures",
            'packageJson.types, "./index.d.ts"',
            'packageJson.exports["."].types, "./index.d.ts"',
            'packageJson.exports["./crypto"].types, "./index.d.ts"',
            'packageJson.files.includes("index.d.ts")',
            "recursiveCompactKeyArtifactsArchive: BinaryLike,",
            "recursiveCompactVerifierKeysArchive: BinaryLike,",
            "recursive compact key packages must not be optional",
        ),
        "JavaScript package dist recursive compact declaration coverage",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        (
            "package declarations keep accumulator digests native-owned",
            "PACKAGE_DECLARATION_TEXTS",
            "connect.browser.d.ts",
            "nexus-app.d.ts",
            "kotodama-compiler.d.ts",
            "for (const [name, declarationsText] of PACKAGE_DECLARATION_TEXTS)",
            "lineageDigest|LineageDigest|lineage_digest",
            "aggregationTranscriptDigest|AggregationTranscriptDigest|aggregation_transcript_digest",
            "fixedWindowTableScheduleDigest|FixedWindowTableScheduleDigest|fixed_window_table_schedule_digest",
            "fixedWindowSharedTableManifestDigest|FixedWindowSharedTableManifestDigest|fixed_window_shared_table_manifest_digest",
            "fixedWindowTableBaseDigest|FixedWindowTableBaseDigest|fixed_window_table_base_digest",
            "verifierWitnessBatchDigest|VerifierWitnessBatchDigest|verifier_witness_batch_digest",
            "recursiveProofChainDigest|RecursiveProofChainDigest|recursive_proof_chain_digest",
            "proofChainDigest|ProofChainDigest|proof_chain_digest",
            "transitionProfileBindingDigest|TransitionProfileBindingDigest|transition_profile_binding_digest",
            "appendOpeningPreflightDigest|AppendOpeningPreflightDigest|append_opening_preflight_digest",
            "appendBoundaryDigest|AppendBoundaryDigest|append_boundary_digest",
            "recursiveVerifierScalarProjectionDigest|RecursiveVerifierScalarProjectionDigest|recursive_verifier_scalar_projection_digest",
            "previousAccumulatorDigest|PreviousAccumulatorDigest|previous_accumulator_digest",
            "resultingAccumulatorDigest|ResultingAccumulatorDigest|resulting_accumulator_digest",
            "accumulatorDigest|AccumulatorDigest|accumulator_digest",
            "${name}: recursive accumulator digests must remain native-owned",
        ),
        "JavaScript package dist accumulator digest declaration coverage",
        errors,
    )

    for relative in ("javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"):
        require_contains(texts, relative, REQUIRED_JS_PUBLIC_EXPORTS, f"{relative} re-exports", errors)
        require_contains(
            texts,
            relative,
            REQUIRED_JS_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_EXPORTS,
            f"{relative} Kagemusha instruction transaction re-exports",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
                "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
                "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
                "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
                "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
                "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
                "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
            ),
            f"{relative} constant re-exports",
            errors,
        )

    require_contains(
        texts,
        "javascript/iroha_js/index.d.ts",
        REQUIRED_JS_PUBLIC_EXPORTS
        + (
            "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION: 6",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: 64",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: true",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: 1",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: 8388608",
            "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: 128",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1:",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1:",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1:",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1:",
            "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN:",
            "KagemushaRecursiveSpendLineageKeyArtifactOpeningLen",
            "KagemushaRecursiveSpendLineageKeyArtifacts",
            "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND:",
            "kagemushaRecursiveSpendLineageKeyArtifactsForInit(",
            "validateKagemushaRecursiveSpendLineageKeyArtifacts(",
            "readonly proofCircuitId:",
            "readonly verifierOpeningLen:",
            "readonly lineageVerifierKeyBackend:",
            "readonly lineageVerifierKey: Buffer;",
            "readonly lineageProvingKeyArchive: Buffer;",
            "readonly isInitArtifact: boolean;",
            "readonly isAppendArtifact: boolean;",
        ),
        "JavaScript TypeScript declarations",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/index.d.ts",
        REQUIRED_JS_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_EXPORTS
        + (
            "KagemushaInstructionArchiveType",
            '"KagemushaTransfer"',
            '"RedeemKagemushaRecursive"',
            "KagemushaInstructionArchiveInput",
            "KagemushaInstructionTransactionInput",
            "KagemushaRecursiveRedeemTransactionBaseInput",
            "KagemushaRecursiveRedeemArchiveInput",
            "KagemushaRecursiveRedeemTransactionInput",
            "KagemushaInstructionArchive:",
            "bytes_base64: string;",
            "redeemRequestArchive: BinaryLike;",
        ),
        "JavaScript TypeScript Kagemusha instruction transaction declarations",
        errors,
    )
    for relative in ("javascript/iroha_js/src/transaction.js", "javascript/iroha_js/dist/transaction.js"):
        require_contains(
            texts,
            relative,
            REQUIRED_JS_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_EXPORTS
            + (
                "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
                "iroha_data_model::isi::offline::KagemushaTransfer",
                "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
                "validateKagemushaInstructionArchive",
                "noritoSchemaHash",
                "noritoCrc64",
                "bytesBase64 must be canonical standard base64",
                "KagemushaInstructionArchive",
                "KagemushaTransfer",
                "RedeemKagemushaRecursive",
                "kagemushaRecursiveSpendRedeem",
                "kagemushaRecursiveRedeem.redeemRequestArchive",
            ),
            f"{relative} Kagemusha instruction transaction builder",
            errors,
        )
        require_regex(
            texts,
            relative,
            r"buildKagemushaRecursiveRedeemTransaction[\s\S]*?kagemushaRecursiveSpendRedeem[\s\S]*?buildKagemushaInstructionTransaction",
            f"{relative} Kagemusha recursive redeem transaction flow",
            errors,
        )
    require_contains(
        texts,
        "crates/iroha_js_host/src/lib.rs",
        (
            "fn kagemusha_instruction_archive_from_json",
            "remove_case_insensitive(&mut map, \"KagemushaInstructionArchive\")",
            "KagemushaTransfer",
            "RedeemKagemushaRecursive",
            "ensure_kagemusha_recursive_archive_len(archive.len(), \"Kagemusha instruction archive\")",
            "KagemushaInstructionArchive.bytes_base64 must be canonical standard base64",
            "build_transaction_from_instructions_json_accepts_kagemusha_instruction_archive",
            "kagemusha_instruction_archive_json_rejects_adversarial_inputs",
        ),
        "iroha_js_host Kagemusha instruction transaction archive decoder",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/transactionBuilder.test.js",
        (
            "buildKagemushaInstructionArchiveInstruction normalizes archive bytes",
            "schema must match RedeemKagemushaRecursive",
            "checksum is invalid",
            "must not be compressed",
            "flags: 0x08",
            "flags: 0x20",
            "invalidPadding[40] = 0xff",
            "paddingLength: 65",
            "valid KagemushaTransfer Norito archive",
            "bytesBase64 must be canonical standard base64",
            "buildKagemushaInstructionTransaction wraps one archive instruction",
            "buildKagemushaRecursiveRedeemTransaction derives instruction before signing",
            "redeemRequestArchive must be a Buffer or ArrayBuffer view",
            "redeem native rejected",
        ),
        "JavaScript Kagemusha instruction transaction builder tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/src/crypto.js",
        [f'typeof native.{name} !== "function"' for name in REQUIRED_JS_NATIVE_METHODS],
        "JavaScript native availability gate",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/src/crypto.js",
        [f"native.{name}" for name in REQUIRED_JS_NATIVE_METHODS],
        "JavaScript malformed-archive probes",
        errors,
    )
    for relative in ("javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"):
        require_contains(
            texts,
            relative,
            (
                "kagemushaRecursiveSpendOutputToBuffer",
                "toOwnedBuffer(value, name)",
                "Buffer.from(toBuffer(value, name))",
                "toKagemushaArchiveView(value, name)",
                "toOwnedKagemushaArchiveBuffer(value, name)",
                "const view = toKagemushaArchiveView(value, name)",
                "view.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
                "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
                'const recordBundle = toOwnedKagemushaArchiveBuffer(',
                'const compactToken = toOwnedKagemushaArchiveBuffer(',
                "outputView.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
                "const output = Buffer.from(outputView)",
                "assertKagemushaNoritoArchive(",
                "previousWitnessArchive",
                "compactTokenArchive",
                "native ${operation} returned invalid Norito archive",
                "native ${operation} returned empty Norito payload",
            ),
            f"{relative} native output Norito guard",
            errors,
        )
    require_contains(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
            (
                "Kagemusha recursive spend helpers reject oversized request archives before native calls",
                "requestArchive must not exceed",
                "recordBundleArchive must not exceed",
                "pallasOpenEnvelopesArchive must not exceed",
                "previousWitnessArchive must not exceed",
                "compactTokenArchive must not exceed",
                "Kagemusha recursive spend helpers reject malformed Norito request archives before native calls",
                "Kagemusha recursive spend helpers reject empty-payload Norito request archives before native calls",
                "requestArchive must be a valid Norito archive",
                "recordBundleArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "previousWitnessArchive must contain a non-empty Norito payload",
                "Kagemusha recursive spend lineage helpers pass owned archive copies to native",
                "Uint8Array.from(kagemushaInputArchive(0xa2))",
                "initRequest[6] = 0x7f",
                "assert.notStrictEqual(calls[0][1], initRequest)",
                "kagemushaInputArchive",
                "Kagemusha recursive spend helpers reject malformed Norito native outputs",
                "Kagemusha recursive spend helpers reject empty-payload Norito native outputs",
                "native kagemushaRecursiveSpendRedeem returned invalid Norito archive",
                "native kagemushaRecursiveSpendRedeem returned empty Norito payload",
                "assertRejectsMalformedNativeRedeemOutput",
                "compressed[22] = 1",
                "unsupportedFlags[39] = 0x08",
                "invalidFieldBitset[39] = 0x20",
                "kagemushaNoritoFrameWithHeaderPadding",
                "Buffer.from([0x7f])",
                "Buffer.alloc(65)",
                "kagemushaNoritoFrameWithPayload",
            ),
        "JavaScript native output Norito guard tests",
        errors,
    )
    require_regex(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        r"Kagemusha recursive spend helpers reject malformed Norito native outputs[\s\S]*"
        r"invalidFieldBitset\[39\] = 0x20[\s\S]*"
        r"assertRejectsMalformedNativeRedeemOutput\(invalidFieldBitset\)[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "JavaScript recursive spend native output header guard tests",
        errors,
    )
    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(
            texts,
            relative,
            (
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
                "proofCircuitId === KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
                "Number.isInteger(hopCount)",
                "hopCount >= 1",
                "hopCount <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
                "Number.isInteger(previousHopCount)",
                "previousHopCount >= 1",
                "previousHopCount < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            ),
            f"{relative} witnessless Reserved-lineage helper bounds",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen",
                "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
                "kagemushaRecursiveSpendLineageKeyArtifactsForAppend",
                "validateKagemushaRecursiveSpendLineageKeyArtifacts",
                "kagemushaLineageKeyArtifactBytes",
                "validateKagemushaRecursiveSpendLineageKeyArtifactPackageBinding",
                "kagemushaLineageVerifierKeyEnvelopeCircuitId",
                "kagemushaLineageProvingKeyArchivePayload",
                "kagemushaLineageProvingKeyArchive(",
                "kagemushaDecodeLineageProvingKeyArchivePayload",
                "kagemushaReadNoritoField",
                "kagemushaDecodeNoritoString",
                "kagemushaDecodeNoritoByteVec",
                "varint is non-canonical",
                "varint exceeds u64 length space",
                "shift >= 63n && chunk > 1n",
                "value > BigInt(Number.MAX_SAFE_INTEGER)",
                "encodedLength > 1",
                "KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
                "KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1",
                "KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG",
                "PRIVACY_NORITO_FIELD_BITSET_FLAG",
                "kagemushaVerifyingKeyCommitment",
                "KAGEMUSHA_ZK1_TLV_CID1",
                "KAGEMUSHA_ZK1_TLV_IPAK",
                "KAGEMUSHA_ZK1_TLV_H2VK",
                "archivePayload.includes(circuitIdBytes)",
                "archivePayload.includes(verifierKeyCommitment)",
                "archive.circuitFamily !== proofCircuitId",
                "!archive.vkCommitment.equals(verifierKeyCommitment)",
                "archive.provingKey.length === 0",
                "storedLineageVerifierKey",
                "storedLineageProvingKeyArchive",
                "get lineageVerifierKey()",
                "get lineageProvingKeyArchive()",
                "Buffer.from(storedLineageVerifierKey)",
                '"proof_circuit_id"',
                '"verifier_opening_len"',
                '"lineage_verifier_key"',
                '"lineage_proving_key_archive"',
            ),
            f"{relative} portable lineage key artifact package",
            errors,
        )

    require_contains(
        texts,
        "javascript/iroha_js/test/package_dist.test.js",
        REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS
        + (
            "isSupportedKagemushaRecursiveSpendLineageKeyArtifactOpeningLen(openingLen)",
            "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
            "kagemushaRecursiveSpendLineageKeyArtifactsForAppend",
            "validateKagemushaRecursiveSpendLineageKeyArtifacts",
            "kagemushaLineageVerifierKey",
            "kagemushaLineageProvingKeyArchive",
            "appendVerifierKey",
            "not-bytes",
            "halo2/kzg",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "exposedVerifierKey[0] = 0",
            "assert.notStrictEqual(",
            "package declarations mark Kagemusha lineage key artifacts readonly",
            "readonly lineageVerifierKey: Buffer;",
            "readonly lineageProvingKeyArchive: Buffer;",
        ),
        "JavaScript package lineage key artifact tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
        (
            "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
            "kagemushaLineageVerifierKey",
            "kagemushaLineageProvingKeyArchive",
            "TEST_NORITO_PACKED_STRUCT_FLAG",
            "TEST_NORITO_FIELD_BITSET_FLAG",
            "options.trailingPayload",
            "Buffer.alloc(64, 0xa6)",
            "Buffer.alloc(64, 0xa7)",
            "trailingPayload: Buffer.from([0x7f])",
            "TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_PACKED_STRUCT_FLAG",
            "TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_FIELD_BITSET_FLAG",
            "kagemushaOverlongCompactLength",
            "kagemushaOversizedTerminalCompactLength",
            "kagemushaHugeCanonicalCompactLength",
            "overlongVersionLengthArchive",
            "oversizedTerminalCompactLengthArchive",
            "hugeCanonicalCompactLengthArchive",
            "overlongCircuitStringArchive",
            "invalidUtf8CircuitArchive",
            "appendVerifierKey",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "exposedVerifierKey[0] = 0",
            "assert.notStrictEqual(",
        ),
        "JavaScript source lineage key artifact copy tests",
        errors,
    )
    require_contains(
        texts,
        "javascript/iroha_js/test/crypto.browser.test.js",
        REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS[:3]
        + (
            "kagemushaRecursiveSpendLineageKeyArtifactsForInit",
            "kagemushaLineageVerifierKey",
            "kagemushaLineageProvingKeyArchive",
            "lineage_verifier_key",
            "verifier_opening_len",
            "exposedVerifierKey[0] = 0",
            "assert.notStrictEqual(",
        ),
        "JavaScript browser lineage key artifact tests",
        errors,
    )


def check_python(texts, errors):
    script = read(PYTHON_SDK_TEST_COMMAND)
    require(
        'PYTHON_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-}"' in script,
        "Kagemusha Python SDK script must keep the documented Python override variable",
        errors,
    )
    require(
        "resolve_python_311_bin()" in script and "python3.11" in script,
        "Kagemusha Python SDK script must resolve Python 3.11 before falling back to python3",
        errors,
    )
    require(
        'PYTHON_BIN="$(resolve_python_311_bin)"' in script,
        "Kagemusha Python SDK script must use the Python 3.11 resolver",
        errors,
    )
    require(
        'PYTHON_VERSION="$("${PYTHON_BIN}" -c' in script,
        "Kagemusha Python SDK script must capture the selected Python version",
        errors,
    )
    require(
        '"${PYTHON_BIN}" --version' in script,
        "Kagemusha Python SDK script must print the selected Python version",
        errors,
    )
    require(
        'VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c' in script,
        "Kagemusha Python SDK script must capture the venv Python version",
        errors,
    )
    require(
        script.count('"${VENV_DIR}/bin/python" --version') >= 2,
        "Kagemusha Python SDK script must print the initial and rebuilt venv Python versions",
        errors,
    )
    require(
        "3.11) ;;" in script,
        "Kagemusha Python SDK script must reject non-Python-3.11 runtimes",
        errors,
    )
    require(
        "recreating Kagemusha recursive spend Python SDK venv" in script,
        "Kagemusha Python SDK script must rebuild stale non-3.11 venvs",
        errors,
    )
    require(
        'rm -rf "${VENV_DIR}"' in script,
        "Kagemusha Python SDK script must remove stale venvs before rebuilding",
        errors,
    )
    require(
        "'maturin>=1.5,<2'" in script,
        "Kagemusha Python SDK script must install maturin for native extension builds",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m pip install --no-deps' in script
        and '"${ROOT_DIR}/python/norito_py"' in script
        and '"${ROOT_DIR}/python/iroha_torii_client"' in script,
        "Kagemusha Python SDK script must install local workspace Python packages before maturin",
        errors,
    )
    require(
        'export VIRTUAL_ENV="${VENV_DIR}"' in script
        and 'export PATH="${VENV_DIR}/bin:${PATH}"' in script,
        "Kagemusha Python SDK script must activate the selected venv before maturin",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m maturin develop --release' in script,
        "Kagemusha Python SDK script must build the native extension with the selected Python",
        errors,
    )
    require(
        "export PYTHONDONTWRITEBYTECODE=1" in script,
        "Kagemusha Python SDK script must not write bytecode cache files during tests",
        errors,
    )
    for test_path in (
        "tests/kagemusha_test.py",
        "tests/privacy_catalog_test.py",
        "tests/crypto_algorithms_test.py",
    ):
        require(
            test_path in script,
            f"Kagemusha Python SDK script must run {test_path}",
            errors,
        )
    init = "python/iroha_python/src/iroha_python/__init__.py"
    wrapper = "python/iroha_python/src/iroha_python/kagemusha.py"
    tx_wrapper = "python/iroha_python/src/iroha_python/tx.py"
    host = "python/iroha_python/iroha_python_rs/src/lib.rs"
    require_contains(texts, wrapper, REQUIRED_PYTHON_PUBLIC_METHODS, "Python SDK", errors)
    require_contains(texts, init, REQUIRED_PYTHON_PUBLIC_METHODS, "Python package re-exports", errors)
    require_contains(
        texts,
        wrapper,
        REQUIRED_PYTHON_KAGEMUSHA_INSTRUCTION_TRANSACTION_PUBLIC_METHODS
        + (
            "def _normalize_kagemusha_instruction_archive_type(",
            "def _assert_kagemusha_instruction_archive_schema(",
            "_norito_schema_hash(wire_name)",
            "_norito_archive_bytes_named(instruction_archive, \"instruction_archive\")",
            "getattr(Instruction, \"kagemusha_instruction_archive\", None)",
            "getattr(Instruction, \"kagemusha_recursive_redeem\", None)",
            "build_signed_transaction",
            "instructions=(instruction,)",
            "_archive_bytes_named(private_key, \"private_key\")",
            "_norito_archive_bytes_named(redeem_request_archive, \"redeem_request_archive\")",
        ),
        "Python Kagemusha instruction transaction builder",
        errors,
    )
    require_contains(
        texts,
        tx_wrapper,
        (
            "def kagemusha_instruction_archive(",
            "kagemusha_instruction_archive_instruction",
            "def kagemusha_recursive_redeem(",
            "kagemusha_recursive_redeem_instruction",
            "self.add_instruction(",
        ),
        "Python TransactionDraft Kagemusha instruction helpers",
        errors,
    )
    require_contains(
        texts,
        host,
        (
            "fn kagemusha_instruction_archive_box(",
            "fn kagemusha_instruction_archive(",
            "fn kagemusha_recursive_redeem(",
            "KagemushaTransfer",
            "RedeemKagemushaRecursive",
            "decode_from_bytes(instruction_archive)",
            "kagemusha_recursive_spend_redeem_instruction_from_request(request)",
            "kagemusha_instruction_archive_box_accepts_transfer_and_redeem_archives",
            "kagemusha_instruction_archive_box_rejects_adversarial_archives",
        ),
        "Python PyO3 Kagemusha instruction transaction archive decoder",
        errors,
    )
    require_contains(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        (
            "test_kagemusha_instruction_archive_transaction_helpers_wrap_redeem_archive",
            "test_kagemusha_recursive_redeem_transaction_helper_derives_instruction_before_signing",
            "test_kagemusha_instruction_archive_transaction_helpers_reject_adversarial_inputs",
            "_shared_recursive_spend_abi7_archive(\"redeem_instruction\")",
            "_shared_recursive_spend_abi7_archive(\"redeem_request\")",
            "_instruction_archive_bytes(instruction)",
            "committed_instruction = kagemusha.kagemusha_instruction_archive_instruction",
            "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
            "instruction_archive must be a valid Norito archive",
            "schema must match RedeemKagemushaRecursive",
            "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
            "compressed[22] = 1",
            "unsupported_flags[39] = 0x08",
            "invalid_field_bitset[39] = 0x20",
            "non_zero_padding.insert(40, 0x7F)",
            'excessive_padding[40:40] = b"\\x00" * 65',
            "bad_request_flags[39] = 0x20",
            "redeem_request_archive must be a valid Norito archive",
            "draft.kagemusha_recursive_redeem(request_archive)",
        ),
        "Python Kagemusha instruction transaction tests",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        (
            "KAGEMUSHA_MAX_BRIDGE_ABI_VERSION = 0xFFFF_FFFF",
            "isinstance(version, bool)",
            "not isinstance(version, int)",
            "version < 0",
            "version > KAGEMUSHA_MAX_BRIDGE_ABI_VERSION",
        ),
        "Python Kagemusha ABI probe bounds",
        errors,
    )
    require_contains(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        (
            '"6"',
            "6.5",
            "0x1_0000_0000",
            "10**100",
            "is_kagemusha_recursive_compact_payment_token_prover_available",
            "is_kagemusha_recursive_compact_payment_token_verifier_available",
        ),
        "Python Kagemusha ABI probe test coverage",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        (
            "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION = 6",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = True",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024",
            "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128",
            'KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND = "halo2/ipa"',
            '"kagemusha-recursive-aggregation-v1"',
            '"kagemusha-recursive-spend-lineage-v1"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-digest"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"',
            '"iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"',
        ),
        "Python SDK constants",
        errors,
    )
    require_contains(
        texts,
        init,
        (
            "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
            "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
            "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
            "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
            "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
            "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
            "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
            "KagemushaRecursiveSpendLineageKeyArtifacts",
            "kagemusha_recursive_spend_lineage_key_artifacts_for_init",
            "validate_kagemusha_recursive_spend_lineage_key_artifacts",
        ),
        "Python package constant re-exports",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        [f'"{name}"' for name in REQUIRED_PYTHON_NATIVE_METHODS],
        "Python native availability probes",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        (
            "_require_kagemusha_native_output",
            "_archive_bytes_named",
            "_norito_archive_bytes_named",
            "view = memoryview(archive)",
            "view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
            "return view.tobytes()",
            "view = memoryview(result)",
            "output = view.tobytes()",
            "_assert_kagemusha_norito_archive(data, name)",
            '_norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")',
            "pallas_open_envelopes = _norito_archive_bytes_named(",
            '_norito_archive_bytes_named(request_archive, "request_archive")',
            '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
            "_assert_kagemusha_norito_archive(output, name)",
            "must not exceed",
            "returned invalid Norito archive",
            "returned empty Norito payload",
        ),
        "Python native output Norito guard",
        errors,
    )
    require_contains(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        (
            "test_recursive_kagemusha_helpers_reject_malformed_norito_requests",
            "test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native",
            "oversized Kagemusha input reached native loading",
            "test_recursive_kagemusha_helpers_reject_oversized_memoryview_native_outputs",
            "test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests",
            "test_kagemusha_native_prover_helpers_reject_malformed_norito_requests",
            "test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests",
            "must not exceed",
            "compact_token_archive",
            "record_bundle_archive must be a valid Norito archive",
            "pallas_open_envelopes_archive must contain a non-empty Norito payload",
            "request_archive must be a valid Norito archive",
            "previous_witness_archive must contain a non-empty Norito payload",
            "_kagemusha_input_archive",
            "test_recursive_kagemusha_helpers_reject_malformed_native_outputs",
            "test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs",
            "returned invalid Norito archive",
            "returned empty Norito payload",
            "assert_rejects_malformed_native_outputs",
            "compressed[22] = 1",
            "unsupported_flags[39] = 0x08",
            "invalid_field_bitset[39] = 0x20",
            "_kagemusha_norito_frame_with_header_padding",
            'b"\\x7f"',
            'b"\\x00" * 65',
            "_kagemusha_norito_frame_with_payload",
            "test_recursive_kagemusha_lineage_helpers_copy_mutable_archives_before_native",
            "memoryview(previous_witness_storage)",
            "expected_previous_witness = bytes(previous_witness_storage)",
            "init_request[6] = 0x7F",
            "test_recursive_kagemusha_lineage_key_artifacts_validate_inputs",
            "kagemusha_recursive_spend_lineage_key_artifacts_for_init",
            "validate_kagemusha_recursive_spend_lineage_key_artifacts",
            "KagemushaRecursiveSpendLineageKeyArtifacts",
            "_kagemusha_lineage_verifier_key",
            "_kagemusha_lineage_proving_key_archive",
            "_kagemusha_verifier_key_commitment",
            "append_verifier_key",
            "duplicate_cid_verifier_key",
            "missing_circuit_archive",
            "wrong_commitment_archive",
            "smuggled_circuit_archive",
            "smuggled_commitment_archive",
            "wrong_version_archive",
            "empty_proving_key_archive",
            "trailing_payload_archive",
            "old_schema_archive",
            "packed_struct_archive",
            "field_bitset_archive",
            "overlong_version_length_archive",
            "oversized_terminal_compact_length_archive",
            "huge_canonical_compact_length_archive",
            "overlong_circuit_string_archive",
            "invalid_utf8_circuit_archive",
            "_kagemusha_overlong_compact_length",
            "_kagemusha_oversized_terminal_compact_length",
            "_kagemusha_huge_canonical_compact_length",
            "_kagemusha_lineage_proving_key_archive_raw",
            "_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
            "halo2/kzg",
            "not-bytes",
            "FrozenInstanceError",
            "proving_key[:] =",
            "init_artifacts.lineage_proving_key_archive =",
        ),
        "Python native output Norito guard tests",
        errors,
    )
    require_regex(
        texts,
        "python/iroha_python/tests/kagemusha_test.py",
        r"def test_recursive_kagemusha_helpers_reject_malformed_native_outputs[\s\S]*"
        r"invalid_field_bitset\[39\] = 0x20[\s\S]*"
        r"assert_rejects_malformed_native_outputs\(bytes\(invalid_field_bitset\)\)[\s\S]*"
        r"_kagemusha_norito_frame_with_header_padding",
        "Python recursive spend native output header guard tests",
        errors,
    )
    require_contains(
        texts,
        host,
        [f'#[pyo3(name = "{name}")]' for name in REQUIRED_PYTHON_NATIVE_METHODS],
        "Python PyO3 exports",
        errors,
    )
    for name in REQUIRED_PYTHON_NATIVE_METHODS:
        require_regex(
            texts,
            host,
            rf"wrap_pyfunction!\s*\(\s*{name}_py\s*,\s*module\s*\)",
            f"Python PyO3 module registration for {name}",
            errors,
        )
    require_regex(
        texts,
        host,
        r"fn\s+kagemusha_recursive_spend_bridge_abi_version_py\s*\(\)\s*->\s*u32\s*\{\s*7\s*\}",
        "Python recursive spend ABI version",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        (
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
            "and proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
            "1 <= hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "1 <= previous_hop_count < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
        ),
        "Python witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_contains(
        texts,
        wrapper,
        REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_PUBLIC_METHODS
        + (
            "@dataclass(frozen=True)",
            "is_init_artifact",
            "is_append_artifact",
            "_validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding",
            "_kagemusha_lineage_verifier_key_envelope_circuit_id",
            "_kagemusha_lineage_proving_key_archive_payload",
            "_kagemusha_decode_lineage_proving_key_archive_payload",
            "_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
            "_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1",
            "_KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG",
            "_KAGEMUSHA_NORITO_FIELD_BITSET_FLAG",
            "if shift >= 63 and chunk > 1",
            "if value > len(buffer)",
            "_kagemusha_verifying_key_commitment",
            "_KAGEMUSHA_ZK1_TLV_CID1",
            "_KAGEMUSHA_ZK1_TLV_IPAK",
            "_KAGEMUSHA_ZK1_TLV_H2VK",
            "archive_payload.find(circuit_id_bytes)",
            "archive_payload.find(verifier_key_commitment)",
            "circuit_family != proof_circuit_id",
            "archive_commitment != verifier_key_commitment",
            "or not proving_key",
            '"proof_circuit_id"',
            '"verifier_opening_len"',
            '"lineage_verifier_key"',
            '"lineage_proving_key_archive"',
        ),
        "Python portable lineage key artifact package",
        errors,
    )


def check_swift(texts, errors):
    prover = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"
    compact_prover = "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift"
    recursive_aggregation_prover = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift"
    bridge = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    instruction_encoder = "IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift"
    test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift"
    compact_test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift"
    recursive_aggregation_test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift"
    instruction_encoder_test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift"
    uc4_decode_test = "IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift"
    offline_v2 = "IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift"
    offline_decoding = "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift"
    offline_v2_test = "IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift"
    require_contains(
        texts,
        offline_decoding,
        (
            "public enum OfflineNoteV2Decoding",
            "decodeCertificatePayload",
            "decodeKeyCertificatePayload",
            "decodeCertificate",
            "decodeIssue",
            "decodeIssuedClaim",
            "decodeAuditOutputClaim",
            "decodeRecursiveProof",
            "decodeRedeem",
            "decodeRedeemPublicInputs",
            "decodeAudit",
            "decodeAuditPublicInputs",
            "decodeIssueInstruction",
            "decodeRedeemInstruction",
            "decodeAuditInstruction",
            "decodeKeyCertificatePayloadV2",
            "decodeKeyCertificateV2",
            "decodeIssueV2",
            "decodeIssuedClaimV2",
            "decodeAuditOutputClaimV2",
            "decodeRecursiveProofV2",
            "decodeRedeemV2",
            "decodeRedeemPublicInputsV2",
            "decodeAuditV2",
            "decodeAuditPublicInputsV2",
            "decodeIssueInstructionV2",
            "decodeRedeemInstructionV2",
            "decodeAuditInstructionV2",
            "decodePayload(data, typeName: OfflineNoteV2TypeNames.issue",
            "decodeKeyCertificatePayloadV2Fields",
            "decodeKeyCertificateV2Fields",
            "decodeIssueV2Fields",
            "decodeIssuedClaimV2Fields",
            "decodeAuditOutputClaimV2Fields",
            "decodeRecursiveProofV2Fields",
            "decodeRedeemV2Fields",
            "decodeAuditV2Fields",
            "readHashV2",
            "readProofBoxV2",
            "readAccountId",
            "readAssetId",
            "readNumeric",
            "readVec",
            "Offline Note payload must use compact lengths",
            "trailing bytes",
        ),
        "Swift Offline Note V2 decoder surface",
        errors,
    )
    require_regex(
        texts,
        offline_decoding,
        r"public static func decodeIssueInstruction\(_ data: Data\) throws -> OfflineNoteIssueV2",
        "Swift Offline Note V2 issue instruction decoder API",
        errors,
    )
    require_regex(
        texts,
        offline_decoding,
        r"public static func decodeRedeemInstruction\(_ data: Data\) throws -> OfflineNoteRedeemV2",
        "Swift Offline Note V2 redeem instruction decoder API",
        errors,
    )
    require_regex(
        texts,
        offline_decoding,
        r"public static func decodeAuditInstruction\(_ data: Data\) throws -> OfflineNoteAuditBundleV2",
        "Swift Offline Note V2 audit instruction decoder API",
        errors,
    )
    require_not_regex(
        texts,
        offline_decoding,
        r"Offline Note V2 decoding is not supported yet|UnsupportedOperationException",
        "Swift Offline Note V2 decoder placeholder removal",
        errors,
    )
    require_contains(
        texts,
        offline_v2,
        (
            "OfflineNoteV2TypeNames.recursiveProof",
            "OfflineNoteV2TypeNames.auditOutputClaim",
            "OfflineNoteV2Encoding.encodeRecursiveProof(self)",
            "OfflineNoteV2Encoding.encodeAuditOutputClaim(self)",
            'static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNote"',
            'static let redeemInstruction = "iroha_data_model::isi::offline::RedeemOfflineNote"',
            'static let auditInstruction = "iroha_data_model::isi::offline::AuditOfflineNote"',
            'static let issueInstructionAlias = "iroha_data_model::isi::offline::IssueOfflineNoteV2"',
            'static let redeemInstructionAlias = "iroha_data_model::isi::offline::RedeemOfflineNoteV2"',
            'static let auditInstructionAlias = "iroha_data_model::isi::offline::AuditOfflineNoteV2"',
        ),
        "Swift Offline Note V2 canonical instruction wire names",
        errors,
    )
    require_contains(
        texts,
        offline_v2_test,
        (
            "testOfflineNoteV2DecodersRoundTripRustNoritoVectors",
            "testOfflineNoteV2DecodersRejectMalformedPayloads",
            "OfflineNoteV2Decoding.decodeCertificatePayload",
            "OfflineNoteV2Decoding.decodeKeyCertificatePayload",
            "OfflineNoteV2Decoding.decodeCertificate",
            "OfflineNoteV2Decoding.decodeIssue",
            "OfflineNoteV2Decoding.decodeIssuedClaim",
            "OfflineNoteV2Decoding.decodeAuditOutputClaim",
            "OfflineNoteV2Decoding.decodeRecursiveProof",
            "OfflineNoteV2Decoding.decodeRedeem",
            "OfflineNoteV2Decoding.decodeRedeemPublicInputs",
            "OfflineNoteV2Decoding.decodeAudit",
            "OfflineNoteV2Decoding.decodeAuditPublicInputs",
            "testOfflineNoteV2InstructionDecodersReadExplorerEnvelopeBytes",
            "testOfflineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes",
            "testOfflineNoteV2InstructionDecodersRejectWrongEnvelopeShapes",
            "OfflineNoteV2Decoding.decodeIssueInstruction",
            "OfflineNoteV2Decoding.decodeRedeemInstruction",
            "OfflineNoteV2Decoding.decodeAuditInstruction",
            "OfflineNoteTypeNames.issueInstruction",
            "OfflineNoteV2TypeNames.issueInstructionAlias",
            "parseSingleOfflineNoteV2Instruction",
            "issueInstruction.wireName.hasSuffix(\"V2\")",
            "auditInstruction.wireName.hasSuffix(\"V2\")",
            "redeemInstruction.wireName.hasSuffix(\"V2\")",
            "OfflineNoteV2Decoding.decodeIssueInstruction(issueInstruction.archive)",
            "OfflineNoteV2Decoding.decodeAuditInstruction(auditInstruction.archive)",
            "OfflineNoteV2Decoding.decodeRedeemInstruction(redeemInstruction.archive)",
            "wrongWireEnvelope",
            "wrongSchemaEnvelope",
            "rawInstructionPair",
            "instructionWirePayload",
            "corruptedChecksum",
            "nonCompactIssue",
            "trailingIssue",
            ".emptyProofBytes",
            ".invalidHash(field: \"public_inputs_hash\")",
        ),
        "Swift Offline Note V2 decoder tests",
        errors,
    )
    require_contains(texts, prover, REQUIRED_PUBLIC_METHODS, "Swift public prover", errors)
    require_contains(
        texts,
        prover,
        ("isSupportedAppendProofTransition",),
        "Swift structural append transition helper",
        errors,
    )
    require_contains(
        texts,
        prover,
        (
            "requiredBridgeAbiVersion: UInt32 = 6",
            "recursiveSpendLineageWitnesslessMaxHopsV1: UInt32 = 64",
            "recursiveSpendLineageTransitionCircuitWiredV1 = true",
            "recursivePreviousProofOpenEnvelopesRequiredCountV1 = 1",
            "recursivePreviousProofOpenEnvelopesMaxBytes = 8 * 1024 * 1024",
            "recursivePallasOpenEnvelopeMaxTranscriptLabelBytes = 128",
            "recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1",
            "recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1",
            '"kagemusha-recursive-aggregation-v1"',
            '"kagemusha-recursive-spend-lineage-v1"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-digest"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"',
            '"iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"',
        ),
        "Swift constants",
        errors,
    )
    require_contains(
        texts,
        prover,
        (
            "LineageKeyArtifacts",
            "recursiveAggregationProofBackend",
            "isSupportedLineageKeyArtifactOpeningLen",
            "lineageKeyArtifactsForInit",
            "lineageKeyArtifactsForAppend",
            "validateLineageKeyArtifacts",
            "invalidLineageKeyArtifact",
            "isInitArtifact",
            "isAppendArtifact",
            "validateLineageKeyArtifactPackageBinding",
            "lineageVerifierKeyEnvelopeCircuitId",
            "lineageProvingKeyArchivePayload",
            "decodeLineageProvingKeyArchivePayload",
            "LineageProvingKeyArchive",
            "kagemushaLineageProvingKeyArchiveSchemaHash",
            "kagemushaLineageProvingKeyArchiveVersionV1",
            "kagemushaNoritoCompactLenFlag",
            "kagemushaNoritoPackedStructFlag",
            "privacyNoritoFieldBitsetFlag",
            "readNoritoField",
            "decodeNoritoString",
            "decodeNoritoByteVec",
            "shift >= 63 && chunk > 1",
            "value <= UInt64(Int.max)",
            "encodedLength > 1",
            "value < (UInt64(1) << UInt64(7 * (encodedLength - 1)))",
            "verifyingKeyCommitment",
            "kagemushaZk1TlvCid1",
            "kagemushaZk1TlvIpaK",
            "kagemushaZk1TlvH2Vk",
            "archivePayload.range(of: circuitIdBytes)",
            "archivePayload.range(of: verifierKeyCommitment)",
            "frame.header.schema == kagemushaLineageProvingKeyArchiveSchemaHash",
            "archive.version == kagemushaLineageProvingKeyArchiveVersionV1",
            "archive.circuitFamily == proofCircuitId",
            "archive.verifierKeyCommitment == verifierKeyCommitment",
            "!archive.provingKey.isEmpty",
            '"proof_circuit_id"',
            '"verifier_opening_len"',
            '"lineage_verifier_key"',
            '"lineage_proving_key_archive"',
        ),
        "Swift portable lineage key artifact package",
        errors,
    )
    require_contains(
        texts,
        test,
        (
            "testLineageKeyArtifactPackagesValidateReleaseProfiles",
            "lineageVerifierKey(",
            "lineageProvingKeyArchive(",
            "lineageProvingKeyArchiveRaw(",
            "kagemushaLineageProvingKeyArchiveSchemaHash",
            "oldKagemushaLineageProvingKeyArchiveSchemaHash",
            "verifierKeyCommitment(verifierKey:",
            "appendVerifierKey",
            "duplicateCidVerifierKey",
            "missingCircuitArchive",
            "smuggledCircuitArchive",
            "wrongCommitmentArchive",
            "smuggledCommitmentArchive",
            "wrongVersionArchive",
            "emptyProvingKeyArchive",
            "trailingPayloadArchive",
            "oldSchemaArchive",
            "packedStructArchive",
            "fieldBitsetArchive",
            "overlongVersionLengthArchive",
            "oversizedTerminalCompactLengthArchive",
            "hugeCanonicalCompactLengthArchive",
            "overlongCircuitStringArchive",
            "invalidUtf8CircuitArchive",
            "noritoOverlongCompactLength",
            "noritoOversizedTerminalCompactLength",
            "noritoHugeCanonicalCompactLength",
            "Data(\"not-zk1\".utf8)",
            "Data(\"not-norito\".utf8)",
            "lineageVerifierKey: appendVerifierKey",
            "provingKeyArchive[0] = 0",
            "var exposedVerifierKey = initArtifacts.lineageVerifierKey",
            "var exposedProvingKeyArchive = initArtifacts.lineageProvingKeyArchive",
            "exposedProvingKeyArchive[0] = 0",
            "XCTAssertEqual(initArtifacts.lineageProvingKeyArchive, initProvingKeyArchive)",
        ),
        "Swift lineage key artifact Data copy tests",
        errors,
    )
    require_contains(
        texts,
        prover,
        (
            "invalidInputArchive",
            "oversizedInputArchive",
            "emptyInputPayload",
            "invalidNativeOutput",
            "emptyNativeOutputPayload",
            "try archives.forEach(requireValidInputArchive)",
            "try requireValidOutputArchive(archive)",
            "noritoDecodeFrame(archive)",
            "Kagemusha recursive spend input archive must not exceed",
            "Kagemusha recursive spend input archive must be a valid Norito archive.",
            "Kagemusha recursive spend input archive must contain a non-empty Norito payload.",
            "Kagemusha recursive spend native bridge returned an invalid Norito archive.",
            "Kagemusha recursive spend native bridge returned an empty Norito payload.",
        ),
        "Swift recursive spend input/output Norito guard",
        errors,
    )
    require_contains(
        texts,
        test,
        (
            "testRejectsMalformedInputArchivesBeforeBridgeCall",
            "testRejectsOversizedInputArchivesBeforeBridgeCall",
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".oversizedInputArchive",
            "descriptionContains: oversizedMessage",
            ".invalidInputArchive",
            ".emptyInputPayload",
            ".invalidNativeOutput",
            ".emptyNativeOutputPayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
            "malformedKagemushaNoritoArchives",
            "compressed[22] = 0x01",
            "unsupportedFlags[39] = NoritoHeader.varintOffsets",
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "kagemushaNoritoFrameWithHeaderPadding",
            "Data([0x7f])",
            "Data(repeating: 0, count: 65)",
        ),
        "Swift recursive spend input/output Norito guard tests",
        errors,
    )
    require_regex(
        texts,
        test,
        r"testRejectsMalformedNativeOutput[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"Self\.kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive spend native output header guard tests",
        errors,
    )
    require_regex(
        texts,
        test,
        r"testRejectsMalformedInputArchivesBeforeBridgeCall[\s\S]*"
        r"Self\.malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"Self\.kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive spend input header guard tests",
        errors,
    )
    require_contains(
        texts,
        instruction_encoder,
        (
            "public enum KagemushaInstructionTransactionError",
            "public enum KagemushaInstructionType",
            "case transfer = \"KagemushaTransfer\"",
            "case redeemRecursive = \"RedeemKagemushaRecursive\"",
            "public struct KagemushaInstructionTransactionRequest",
            "public struct KagemushaRecursiveRedeemTransactionRequest",
            "public enum KagemushaRecursiveRedeemRequestArchive",
            "KagemushaRecursiveRedeemRequestArchiveError",
            "unexpectedInstructionArchiveType(expected: KagemushaInstructionType, actual: KagemushaInstructionType)",
            "KagemushaRecursiveSpendRedeemRequestV1",
            "static func encodeKagemushaInstruction(",
            "static func encodeKagemushaRecursiveRedeem(",
            "func buildKagemushaInstruction(",
            "func buildKagemushaRecursiveRedeem(",
            "try KagemushaRecursiveSpendProver.redeemSpend(requestArchive: $0)",
            "KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes",
            "frame.header.compression == .none",
            "frame.header.schema == noritoSchemaHash(forTypeName: type.wireName)",
            "noritoSchemaHash(forTypeName: schemaName) == frame.header.schema",
            "Kagemusha instruction archive type is not supported by this transaction builder.",
        ),
        "Swift Kagemusha instruction transaction builder",
        errors,
    )
    require_contains(
        texts,
        instruction_encoder_test,
        (
            "testBuildRecursiveRedeemInstructionTransactionWrapsNativeInstructionArchive",
            "testBuildKagemushaTransferInstructionTransactionUsesTransferWireName",
            "testBuildKagemushaRecursiveRedeemTransactionDerivesInstructionBeforeSigning",
            "testKagemushaInstructionTransactionRejectsAdversarialArchives",
            "testKagemushaRecursiveRedeemTransactionRejectsMalformedRequestBeforeNativeRedeem",
            "testKagemushaRecursiveRedeemTransactionRejectsAdversarialNativeInstructionArchives",
            "testKagemushaRecursiveRedeemRequestArchiveValidationRejectsAdversarialInputs",
            "testKagemushaInstructionRequestValidationRejectsInvalidInputsBeforeSigning",
            ".unsupportedInstructionArchiveType",
            ".unsupportedRequestArchiveType",
            ".unexpectedInstructionArchiveType(expected: .redeemRecursive, actual: .transfer)",
            "unsupportedFlagsArchive",
            "invalidFieldBitsetArchive",
            "nonZeroPaddingArchive",
            "excessivePaddingArchive",
            "KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes + 1",
        ),
        "Swift Kagemusha instruction transaction builder tests",
        errors,
    )
    require_contains(
        texts,
        compact_prover,
        (
            "invalidRecordBundleArchive",
            "oversizedRecordBundleArchive",
            "emptyRecordBundlePayload",
            "oversizedCompactTokenArchive",
            "invalidCompactTokenArchive",
            "emptyCompactTokenPayload",
            "try requireValidRecordBundleArchive(recordBundleArchive)",
            "try requireValidCompactTokenArchive(token)",
            "noritoDecodeFrame(archive)",
            "Kagemusha verified fold record bundle archive must not exceed",
            "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
            "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload.",
            "Kagemusha compact-token native bridge returned an invalid Norito archive.",
            "Kagemusha compact-token native bridge returned an empty Norito payload.",
        ),
        "Swift compact-token input/output Norito guard",
        errors,
    )
    require_contains(
        texts,
        compact_test,
        (
            "testRejectsMalformedRecordBundleArchiveBeforeBridgeCall",
            "testRejectsOversizedRecordBundleArchiveBeforeBridgeCall",
            "testRejectsEmptyPayloadRecordBundleArchiveBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".oversizedRecordBundleArchive",
            "must not exceed",
            ".invalidRecordBundleArchive",
            ".emptyRecordBundlePayload",
            ".invalidCompactTokenArchive",
            ".emptyCompactTokenPayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
            "malformedKagemushaNoritoArchives",
            "compressed[22] = 0x01",
            "unsupportedFlags[39] = NoritoHeader.varintOffsets",
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "kagemushaNoritoFrameWithHeaderPadding",
            "Data([0x7f])",
            "Data(repeating: 0, count: 65)",
        ),
        "Swift compact-token input/output Norito guard tests",
        errors,
    )
    require_regex(
        texts,
        compact_test,
        r"testRejectsMalformedNativeOutput[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift compact-token native output header guard tests",
        errors,
    )
    require_regex(
        texts,
        compact_test,
        r"testRejectsMalformedRecordBundleArchiveBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift compact-token input header guard tests",
        errors,
    )
    require_contains(
        texts,
        recursive_aggregation_prover,
        (
            "invalidRecordBundleArchive",
            "oversizedRecordBundleArchive",
            "emptyRecordBundlePayload",
            "invalidPallasOpenEnvelopesArchive",
            "oversizedPallasOpenEnvelopesArchive",
            "emptyPallasOpenEnvelopesPayload",
            "oversizedProofBundleArchive",
            "invalidProofBundleArchive",
            "emptyProofBundlePayload",
            "try requireValidInputArchive(",
            "try requireValidProofBundleArchive(proofBundle)",
            "noritoDecodeFrame(archive)",
            "Kagemusha verified fold record bundle archive must not exceed",
            "Kagemusha Pallas open-envelope archive must not exceed",
            "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
            "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
            "Kagemusha recursive aggregation native bridge returned an invalid Norito archive.",
            "Kagemusha recursive aggregation native bridge returned an empty Norito payload.",
        ),
        "Swift recursive aggregation input/output Norito guard",
        errors,
    )
    require_contains(
        texts,
        recursive_aggregation_test,
        (
            "testRejectsMalformedInputArchivesBeforeBridgeCall",
            "testRejectsOversizedInputArchivesBeforeBridgeCall",
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".oversizedRecordBundleArchive",
            ".oversizedPallasOpenEnvelopesArchive",
            "must not exceed",
            ".invalidRecordBundleArchive",
            ".emptyPallasOpenEnvelopesPayload",
            ".invalidProofBundleArchive",
            ".emptyProofBundlePayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
            "malformedKagemushaNoritoArchives",
            "compressed[22] = 0x01",
            "unsupportedFlags[39] = NoritoHeader.varintOffsets",
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "kagemushaNoritoFrameWithHeaderPadding",
            "Data([0x7f])",
            "Data(repeating: 0, count: 65)",
        ),
        "Swift recursive aggregation input/output Norito guard tests",
        errors,
    )
    require_regex(
        texts,
        recursive_aggregation_test,
        r"testRejectsMalformedNativeOutput[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive aggregation native output header guard tests",
        errors,
    )
    require_regex(
        texts,
        recursive_aggregation_test,
        r"testRejectsMalformedInputArchivesBeforeBridgeCall[\s\S]*"
        r"malformedKagemushaNoritoArchives\(validArchive\)[\s\S]*"
        r"invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*"
        r"kagemushaNoritoFrameWithHeaderPadding",
        "Swift recursive aggregation input header guard tests",
        errors,
    )
    require_contains(texts, bridge, REQUIRED_C_SYMBOLS, "Swift C symbol loader", errors)
    require_contains(
        texts,
        bridge,
        (
            "kagemushaRecursiveSpendNativeProbeOk",
            "probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileInitFn)",
            "probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileAppendFn)",
            "probeKagemushaArchiveFunction(kagemushaRecursiveSpendLineageAppendBoundaryFn)",
            "probeKagemushaLineageWitnessFromInitResultFunction(\n                kagemushaRecursiveSpendLineageWitnessFromInitResultFn",
            "probeKagemushaLineageWitnessAppendResultFunction(\n                kagemushaRecursiveSpendLineageWitnessAppendResultFn",
        ),
        "Swift native availability probe",
        errors,
    )
    require_contains(
        texts,
        prover,
        (
            "recursiveSpendLineageTransitionCircuitWiredV1",
            "circuitId == recursiveSpendLineageProofCircuitIdV1",
            "hopCount >= 1",
            "hopCount <= recursiveSpendLineageWitnesslessMaxHopsV1",
            "previousHopCount >= 1",
            "previousHopCount < recursiveSpendLineageWitnesslessMaxHopsV1",
        ),
        "Swift witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_contains(
        texts,
        uc4_decode_test,
        (
            "testDecodeUC4PaymentTokenRejectsMalformedCompactPayload",
            "testDecodeUC4PaymentTokenRejectsWrongCompactMarkerThroughCanonicalDecoder",
            "UC4_TOKEN_PATH",
            "throw XCTSkip",
            "OfflineNotePaymentTokenCodec.decodeText",
            "OfflineNotePaymentTokenCodec.decodeNorito",
            "ios-compact-v1:",
        ),
        "Swift UC4 payment-token diagnostic test",
        errors,
    )


def check_swift_sdk_script_prints_swiftc_version(errors):
    script = read(SWIFT_SDK_PARSE_COMMAND)
    require(
        'SWIFTC_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc}"' in script,
        "Kagemusha Swift SDK script must keep the documented swiftc override variable",
        errors,
    )
    require(
        '"${SWIFTC_BIN}" --version' in script,
        "Kagemusha Swift SDK script must print the selected swiftc version",
        errors,
    )
    for relative in (
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
        "IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift",
        "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift",
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift",
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
        "IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift",
        "IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift",
        "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift",
    ):
        require(
            relative in script,
            f"Kagemusha Swift SDK script must parse {relative}",
            errors,
        )
    require(
        "IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift" in script,
        "Kagemusha Swift SDK script must parse the UC4 payment-token diagnostic test",
        errors,
    )
    require(
        "IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift" in script,
        "Kagemusha Swift SDK script must parse the Halo2 offline note prover",
        errors,
    )
    require(
        "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift" in script,
        "Kagemusha Swift SDK script must parse the compact-token prover",
        errors,
    )
    require(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift" in script,
        "Kagemusha Swift SDK script must parse the recursive aggregation prover",
        errors,
    )
    require(
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift" in script,
        "Kagemusha Swift SDK script must parse the compact-token prover tests",
        errors,
    )
    require(
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift" in script,
        "Kagemusha Swift SDK script must parse the recursive aggregation prover tests",
        errors,
    )
    require(
        "IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift" in script,
        "Kagemusha Swift SDK script must parse Offline Note V2 models",
        errors,
    )
    require(
        "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift" in script,
        "Kagemusha Swift SDK script must parse Offline Note V2 decoder",
        errors,
    )
    require(
        "IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift" in script,
        "Kagemusha Swift SDK script must parse Offline Note V2 decoder tests",
        errors,
    )


def check_java_kotlin(texts, errors):
    java_compact = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java"
    java_recursive_aggregation = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java"
    java = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
    kotlin_compact = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt"
    kotlin_recursive_aggregation = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt"
    kotlin = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
    java_recursive_compact = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"
    kotlin_recursive_compact = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"
    java_offline_v2 = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java"
    kotlin_offline_v2 = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt"
    java_test = "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java"
    kotlin_test = "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt"
    java_offline_v2_test = "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteV2Test.java"
    kotlin_offline_v2_test = "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteV2Test.kt"
    for relative, label in (
        (java_offline_v2, "Android Java Offline Note V2 decoder"),
        (kotlin_offline_v2, "Kotlin Offline Note V2 decoder"),
    ):
        require_contains(
            texts,
            relative,
            (
                "decodeCertificatePayload",
                "decodeCertificate",
                "decodeIssue",
                "decodeIssuedClaim",
                "decodeAuditOutputClaim",
                "decodeRecursiveProof",
                "decodeRedeem",
                "decodeRedeemPublicInputs",
                "decodeAudit",
                "decodeAuditPublicInputs",
                "encodeAuditOutputClaim",
                "encodeRecursiveProof",
                "NoritoCodec.decode",
                "readAccountId",
                "readAssetId",
                "readNumeric",
                "readVec",
            ),
            f"{label} surface",
            errors,
        )
        require_not_regex(
            texts,
            relative,
            r"Offline Note V2 decoding is not supported yet|UnsupportedOperationException",
            f"{label} placeholder removal",
            errors,
        )
    for relative, label in (
        (java_offline_v2_test, "Android Java Offline Note V2 decoder tests"),
        (kotlin_offline_v2_test, "Kotlin Offline Note V2 decoder tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "offlineNoteV2DecodersRoundTripRustNoritoVectors",
                "offlineNoteV2DecodersRejectMalformedPayloads",
                "decodeIssue",
                "decodeAudit",
                "decodeRedeem",
                "decodeCertificate(certificatePayloadBytes",
            ),
            label,
            errors,
        )
    for relative, label, signatures in (
        (
            java_offline_v2,
            "Android Java Offline Note V2 instruction decoder",
            (
                "public static IssueV2 decodeIssueInstruction(final byte[] bytes)",
                "public static RedeemV2 decodeRedeemInstruction(final byte[] bytes)",
                "public static AuditBundleV2 decodeAuditInstruction(final byte[] bytes)",
                "INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER",
            ),
        ),
        (
            kotlin_offline_v2,
            "Kotlin Offline Note V2 instruction decoder",
            (
                "fun decodeIssueInstruction(bytes: ByteArray): IssueV2",
                "fun decodeRedeemInstruction(bytes: ByteArray): RedeemV2",
                "fun decodeAuditInstruction(bytes: ByteArray): AuditBundleV2",
                "InstructionWrapperPayloadAdapter",
            ),
        ),
    ):
        require_contains(
            texts,
            relative,
            (
                "decodeInstructionModel",
                "extractInstructionWirePayload",
                "tryDecodeInstructionPair",
                "decodeModelPayload",
                "isNoritoFrame",
                "Offline Note V2 instruction envelope is invalid",
                "Offline Note V2 instruction model payload is invalid",
                "NoritoCodec.decode",
                *signatures,
            ),
            f"{label} surface",
            errors,
        )
    for relative, label in (
        (java_offline_v2_test, "Android Java Offline Note V2 instruction decoder tests"),
        (kotlin_offline_v2_test, "Kotlin Offline Note V2 instruction decoder tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "offlineNoteV2InstructionDecodersReadExplorerEnvelopeBytes",
                "offlineNoteV2InstructionDecodersRejectWrongEnvelopeShapes",
                "decodeIssueInstruction",
                "decodeRedeemInstruction",
                "decodeAuditInstruction",
                "rawInstructionPair",
                "wirePayloadBytes",
                "compact = false" if relative.endswith(".kt") else "false))",
                "decodeIssueInstruction(issue.noritoEncoded())",
            ),
            label,
            errors,
        )
    for relative, label, signatures in (
        (
            java_offline_v2,
            "Android Java Offline Note V2 instruction wrapper",
            (
                "public static InstructionBox issueInstruction(final IssueV2 value)",
                "public static InstructionBox redeemInstruction(final RedeemV2 value)",
                "public static InstructionBox auditInstruction(final AuditBundleV2 value)",
                "INSTRUCTION_WRAPPER_ADAPTER",
            ),
        ),
        (
            kotlin_offline_v2,
            "Kotlin Offline Note V2 instruction wrapper",
            (
                "fun issueInstruction(value: IssueV2): InstructionBox",
                "fun redeemInstruction(value: RedeemV2): InstructionBox",
                "fun auditInstruction(value: AuditBundleV2): InstructionBox",
                "InstructionWrapperAdapter",
            ),
        ),
    ):
        require_contains(
            texts,
            relative,
            (
                "ISSUE_INSTRUCTION_SCHEMA",
                "REDEEM_INSTRUCTION_SCHEMA",
                "AUDIT_INSTRUCTION_SCHEMA",
                "ISSUE_INSTRUCTION_ALIAS_SCHEMA",
                "REDEEM_INSTRUCTION_ALIAS_SCHEMA",
                "AUDIT_INSTRUCTION_ALIAS_SCHEMA",
                "iroha_data_model::isi::offline::IssueOfflineNote",
                "iroha_data_model::isi::offline::RedeemOfflineNote",
                "iroha_data_model::isi::offline::AuditOfflineNote",
                "IssueOfflineNoteV2",
                "RedeemOfflineNoteV2",
                "AuditOfflineNoteV2",
                "InstructionBox.fromWirePayload",
                "encodeInstructionWrapper",
                "NoritoCodec.encode",
                "value.validateProofBinding()",
                "encodeIssue(value)",
                "encodeRedeem(value)",
                "encodeAudit(value)",
                *signatures,
            ),
            f"{label} surface",
            errors,
        )
        if relative.endswith(".java"):
            require_regex(
                texts,
                relative,
                r'public static final String ISSUE_INSTRUCTION_SCHEMA =\s*"iroha_data_model::isi::offline::IssueOfflineNote";',
                f"{label} canonical instruction wire names",
                errors,
            )
            require_regex(
                texts,
                relative,
                r'public static final String REDEEM_INSTRUCTION_SCHEMA =\s*"iroha_data_model::isi::offline::RedeemOfflineNote";',
                f"{label} canonical instruction wire names",
                errors,
            )
            require_regex(
                texts,
                relative,
                r'public static final String AUDIT_INSTRUCTION_SCHEMA =\s*"iroha_data_model::isi::offline::AuditOfflineNote";',
                f"{label} canonical instruction wire names",
                errors,
            )
        else:
            require_regex(
                texts,
                relative,
                r'const val ISSUE_INSTRUCTION_SCHEMA: String =\s*"iroha_data_model::isi::offline::IssueOfflineNote"',
                f"{label} canonical instruction wire names",
                errors,
            )
            require_regex(
                texts,
                relative,
                r'const val REDEEM_INSTRUCTION_SCHEMA: String =\s*"iroha_data_model::isi::offline::RedeemOfflineNote"',
                f"{label} canonical instruction wire names",
                errors,
            )
            require_regex(
                texts,
                relative,
                r'const val AUDIT_INSTRUCTION_SCHEMA: String =\s*"iroha_data_model::isi::offline::AuditOfflineNote"',
                f"{label} canonical instruction wire names",
                errors,
            )
    for relative, label in (
        (java_offline_v2_test, "Android Java Offline Note V2 instruction wrapper tests"),
        (kotlin_offline_v2_test, "Kotlin Offline Note V2 instruction wrapper tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "offlineNoteV2InstructionWrappersProduceSchemaBoundPayloads",
                "offlineNoteV2InstructionWrappersRejectProofMismatches",
                "offlineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes",
                "IssueOfflineNoteV2",
                "canonical issue instruction wire name",
                "assertInstructionWrapper",
                "decodeInstructionWrapper",
                "payloadBytes",
                "issueInstruction(issue)",
                "auditInstruction(audit)",
                "redeemInstruction(redeem)",
                "redeemInstruction(redeem.replacingRecursiveProof",
                "auditInstruction(audit.replacingRecursiveProof",
            ),
            label,
            errors,
        )
    for relative, label in ((java, "Android Java SDK"), (kotlin, "Kotlin JVM SDK")):
        require_contains(texts, relative, REQUIRED_PUBLIC_METHODS, label, errors)
        require_contains(texts, relative, JNI_METHODS, f"{label} native declarations", errors)
        require_contains(
            texts,
            relative,
            (
                "REQUIRED_BRIDGE_ABI_VERSION",
                "RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
                "RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
                "RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
                "RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
                "RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
                "RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
                "RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
                "isSupportedAppendProofTransition",
                '"kagemusha-recursive-aggregation-v1"',
                '"kagemusha-recursive-spend-lineage-v1"',
                '"iroha:kagemusha:v1:recursive-spend-transition-profile"',
                '"iroha:kagemusha:v1:recursive-spend-transition-profile-digest"',
                '"iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"',
                '"iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"',
                '"iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"',
                '"iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"',
                '"iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"',
            ),
            f"{label} constants",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "LineageKeyArtifacts",
                "RECURSIVE_AGGREGATION_PROOF_BACKEND",
                "isSupportedLineageKeyArtifactOpeningLen",
                "lineageKeyArtifactsForInit",
                "lineageKeyArtifactsForAppend",
                "validateLineageKeyArtifacts",
                "isInitArtifact",
                "isAppendArtifact",
                "validateLineageKeyArtifactPackageBinding",
                "lineageVerifierKeyEnvelopeCircuitId",
                "lineageProvingKeyArchivePayload",
                "LineageProvingKeyArchive",
                "decodeLineageProvingKeyArchivePayload",
                "readNoritoField",
                "decodeNoritoString",
                "decodeNoritoByteVec",
                "KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
                "KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1",
                "KAGEMUSHA_NORITO_COMPACT_LEN_FLAG",
                "KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG",
                "PRIVACY_NORITO_FIELD_BITSET_FLAG",
                "verifyingKeyCommitment",
                "KAGEMUSHA_ZK1_TLV_CID1",
                "KAGEMUSHA_ZK1_TLV_IPAK",
                "KAGEMUSHA_ZK1_TLV_H2VK",
                "circuitIdBytes",
                "verifierKeyCommitment",
                "indexOfSlice",
                "archive.circuitFamily",
                "archive.provingKey",
                "lineageProvingKeyArchive[39]",
                '"proof_circuit_id"',
                '"verifier_opening_len"',
                '"lineage_verifier_key"',
                '"lineage_proving_key_archive"',
            ),
            f"{label} portable lineage key artifact package",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "nativeTransitionProfileInit",
                "nativeTransitionProfileAppend",
                "nativeLineageAppendBoundary",
                "nativeLineageWitnessFromInitResult(probe, probe)",
                "nativeLineageWitnessAppendResult(probe, probe, probe)",
            ),
            f"{label} native availability probes",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "requireNativeInput",
                "KagemushaCompactPaymentTokenProver.isValidNoritoArchive",
                "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload",
                "must not exceed",
                "must be a valid Norito archive",
                "must contain a non-empty Norito payload",
            ),
            f"{label} recursive spend input Norito guard",
            errors,
        )

    for relative, label in (
        (java_compact, "Android Java compact-token prover"),
        (kotlin_compact, "Kotlin compact-token prover"),
    ):
        require_contains(
            texts,
            relative,
            (
                "proveVerifiedCompactPaymentTokenWithRecords",
                "ownedNativeInput",
                "requireNativeInput",
                "isValidNoritoArchive(archive)",
                "hasNonEmptyNoritoPayload(archive)",
                "must not exceed",
                "must be a valid Norito archive",
                "must contain a non-empty Norito payload",
            ),
            f"{label} input Norito guard",
            errors,
        )

    for relative, label in (
        (java_recursive_aggregation, "Android Java recursive aggregation prover"),
        (kotlin_recursive_aggregation, "Kotlin recursive aggregation prover"),
    ):
        require_contains(
            texts,
            relative,
            (
                "proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
                "KagemushaCompactPaymentTokenProver.ownedNativeInput",
                "recordBundleArchive",
                "pallasOpenEnvelopesArchive",
            ),
            f"{label} input Norito guard",
            errors,
        )

    for relative, label in (
        (java_recursive_compact, "Android Java recursive compact prover"),
        (kotlin_recursive_compact, "Kotlin recursive compact prover"),
    ):
        require_contains(
            texts,
            relative,
            (
                "ownedNativeInput",
                'ownedNativeInput(recordBundleArchive, "recordBundleArchive")',
                'ownedNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")',
                "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
                "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
                "must not exceed",
                "must be a valid Norito archive",
                "must contain a non-empty Norito payload",
            ),
            f"{label} input Norito guard",
            errors,
        )

    require_contains(
        texts,
        java_compact,
        ('archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes"',),
        "Android Java compact-token archive max input guard",
        errors,
    )
    require_contains(
        texts,
        kotlin_compact,
        ('"$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"',),
        "Kotlin compact-token archive max input guard",
        errors,
    )
    require_contains(
        texts,
        java,
        ('archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes"',),
        "Android Java recursive spend archive max input guard",
        errors,
    )
    require_contains(
        texts,
        kotlin,
        ('"$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"',),
        "Kotlin recursive spend archive max input guard",
        errors,
    )
    require_contains(
        texts,
        java_recursive_compact,
        (
            '+ " must not exceed "',
            "KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES",
        ),
        "Android Java recursive compact archive max input guard",
        errors,
    )
    require_contains(
        texts,
        kotlin_recursive_compact,
        ('"$archiveName must not exceed ${KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"',),
        "Kotlin recursive compact archive max input guard",
        errors,
    )

    require_contains(
        texts,
        java_compact,
        (
            "static byte[] ownedNativeInput",
            "final byte[] recordBundle = ownedNativeInput(recordBundleArchive, \"recordBundleArchive\")",
            "nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundle)",
            "return Arrays.copyOf(archive, archive.length)",
        ),
        "Android Java compact-token archive input copy",
        errors,
    )
    require_contains(
        texts,
        kotlin_compact,
        (
            "internal fun ownedNativeInput",
            "proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: ByteArray?)",
            "val recordBundle = ownedNativeInput(recordBundleArchive, \"recordBundleArchive\")",
            "nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundle)",
            "ownedNativeInput(archiveInput: ByteArray?, archiveName: String)",
            "val archive = requireNativeInput(archiveInput, archiveName)",
            "return archive.copyOf()",
            "archive != null && archive.isNotEmpty()",
            "hasNonEmptyNoritoPayload(output: ByteArray?)",
        ),
        "Kotlin compact-token archive input copy",
        errors,
    )
    require_contains(
        texts,
        java_recursive_compact,
        (
            "static byte[] ownedNativeInput",
            "final byte[] recordBundle = ownedNativeInput(recordBundleArchive, \"recordBundleArchive\")",
            "final byte[] compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "recursiveCompactVerifierKeysArchive, \"recursiveCompactVerifierKeysArchive\"",
            "nativeVerifyRecursiveCompactPaymentToken(compactToken, verifierKeys)",
        ),
        "Android Java recursive compact archive input copy",
        errors,
    )
    require_contains(
        texts,
        kotlin_recursive_compact,
        (
            "internal fun ownedNativeInput",
            "val recordBundle = ownedNativeInput(recordBundleArchive, \"recordBundleArchive\")",
            "val compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "recursiveCompactVerifierKeysArchive, \"recursiveCompactVerifierKeysArchive\"",
            "nativeVerifyRecursiveCompactPaymentToken(compactToken, verifierKeys)",
        ),
        "Kotlin recursive compact archive input copy",
        errors,
    )

    require_contains(
        texts,
        java,
        (
            "static byte[] ownedNativeInput",
            "final byte[] ownedArchive = ownedNativeInput(archive, archiveName)",
            "final byte[] request = ownedNativeInput(requestArchive, \"requestArchive\")",
            "final byte[] previousWitness = ownedNativeInput(previousWitnessArchive, \"previousWitnessArchive\")",
            "return Arrays.copyOf(archive, archive.length)",
            "call.run(ownedArchive)",
        ),
        "Android Java recursive spend archive input copy",
        errors,
    )
    require_contains(
        texts,
        kotlin,
        (
            "internal fun ownedNativeInput",
            "fun initSpend(requestArchive: ByteArray?)",
            "previousWitnessArchive: ByteArray?",
            "bundleArchive: ByteArray?",
            "val ownedArchive = ownedNativeInput(archive, archiveName)",
            "val request = ownedNativeInput(requestArchive, \"requestArchive\")",
            "val previousWitness = ownedNativeInput(previousWitnessArchive, \"previousWitnessArchive\")",
            "ownedNativeInput(archiveInput: ByteArray?, archiveName: String)",
            "val archive = requireNativeInput(archiveInput, archiveName)",
            "return archive.copyOf()",
            "archive != null && archive.isNotEmpty()",
            "nativeCall(ownedArchive)",
        ),
        "Kotlin recursive spend archive input copy",
        errors,
    )

    require_contains(
        texts,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchives.kt",
        (
            "enum class KagemushaInstructionType",
            "TRANSFER(",
            "REDEEM_RECURSIVE(",
            '"KagemushaTransfer"',
            '"RedeemKagemushaRecursive"',
            '"iroha_data_model::isi::offline::KagemushaTransfer"',
            '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
            "fun instructionBox(",
            "recursiveRedeemInstructionBox",
            "recursiveRedeemInstructionBoxFromRequest",
            "fun transactionPayload(",
            "recursiveRedeemTransactionPayload",
            "recursiveRedeemTransactionPayloadFromRequest",
            "KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive)",
            "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES",
            "NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.wireName))",
            "decoded.header.compression == NoritoHeader.COMPRESSION_NONE",
            "decoded.header.payloadLength > 0",
            "decoded.header.validateChecksum(decoded.payload)",
            "InstructionBox.fromWirePayload(instructionType.wireName, archive)",
            "Executable.instructions(listOf(instructionBox(instructionType, instructionArchive)))",
        ),
        "Kotlin Kagemusha instruction archive transaction helper",
        errors,
    )
    require_contains(
        texts,
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaInstructionArchives.java",
        (
            "public enum InstructionType",
            "TRANSFER(",
            "REDEEM_RECURSIVE(",
            '"KagemushaTransfer"',
            '"RedeemKagemushaRecursive"',
            '"iroha_data_model::isi::offline::KagemushaTransfer"',
            '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
            "public static InstructionBox instructionBox(",
            "recursiveRedeemInstructionBox",
            "recursiveRedeemInstructionBoxFromRequest",
            "public static TransactionPayload transactionPayload(",
            "recursiveRedeemTransactionPayload",
            "recursiveRedeemTransactionPayloadFromRequest",
            "KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive)",
            "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES",
            "NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.wireName()))",
            "decoded.header().compression() != NoritoHeader.COMPRESSION_NONE",
            "decoded.header().payloadLength() == 0",
            "decoded.header().validateChecksum(decoded.payload())",
            "InstructionBox.fromWirePayload(instructionType.wireName(), archive)",
            "Executable.instructions(List.of(instructionBox(instructionType, instructionArchive)))",
        ),
        "Android Java Kagemusha instruction archive transaction helper",
        errors,
    )
    require_contains(
        texts,
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchivesTest.kt",
        (
            "instructionBox preserves redeem archive bytes and wire name",
            "transactionPayload wraps a single transfer archive instruction",
            "instructionBox rejects malformed wrong schema empty and tampered archives",
            "KagemushaInstructionType.REDEEM_RECURSIVE",
            "KagemushaInstructionType.TRANSFER",
            "assertContentEquals(archive, wire.payloadBytes)",
            "recursiveRedeemInstructionBoxFromRequest(byteArrayOf())",
            "recursiveRedeemTransactionPayloadFromRequest(",
            '"KagemushaRecursiveSpendRedeemRequestV1"',
            "tampered[tampered.lastIndex]",
            "compressed[22] = 1",
            "NoritoHeader.VARINT_OFFSETS",
            "NoritoHeader.FIELD_BITSET",
            "withNonZeroHeaderPadding",
        ),
        "Kotlin Kagemusha instruction archive transaction helper tests",
        errors,
    )
    require_contains(
        texts,
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionBuilderTests.java",
        (
            "kagemushaInstructionArchivesBuildPayloads",
            "kagemushaInstructionArchivesRejectAdversarialInputs",
            "KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE",
            "KagemushaInstructionArchives.InstructionType.TRANSFER",
            "KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive)",
            "KagemushaInstructionArchives.transactionPayload(",
            "KagemushaInstructionArchives.recursiveRedeemInstructionBoxFromRequest(new byte[0])",
            "KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(",
            "Arrays.equals(transferArchive, transferWire.payloadBytes())",
            '"KagemushaRecursiveSpendRedeemRequestV1"',
            "tampered[tampered.length - 1] ^= 0x01",
            "compressed[22] = 1",
            "NoritoHeader.VARINT_OFFSETS",
            "NoritoHeader.FIELD_BITSET",
            "withNonZeroHeaderPadding",
        ),
        "Android Java Kagemusha instruction archive transaction helper tests",
        errors,
    )

    require_regex(texts, java, r"REQUIRED_BRIDGE_ABI_VERSION\s*=\s*6\s*;", "Android ABI version", errors)
    require_regex(texts, java, r"RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1\s*=\s*64\s*;", "Android max hops", errors)
    require_regex(texts, java, r"RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1\s*=\s*true\s*;", "Android transition-circuit wired flag", errors)
    require_regex(texts, java, r"RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1\s*=\s*1\s*;", "Android open envelope count", errors)
    require_regex(texts, java, r"RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES\s*=\s*8\s*\*\s*1024\s*\*\s*1024\s*;", "Android open envelope max bytes", errors)
    require_regex(texts, java, r"RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES\s*=\s*128\s*;", "Android Pallas open-envelope transcript label max bytes", errors)
    require_regex(texts, kotlin, r"REQUIRED_BRIDGE_ABI_VERSION:\s*Int\s*=\s*6", "Kotlin ABI version", errors)
    require_regex(texts, kotlin, r"RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1:\s*Int\s*=\s*64", "Kotlin max hops", errors)
    require_regex(texts, kotlin, r"RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1:\s*Boolean\s*=\s*true", "Kotlin transition-circuit wired flag", errors)
    require_regex(texts, kotlin, r"RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1:\s*Int\s*=\s*1", "Kotlin open envelope count", errors)
    require_regex(texts, kotlin, r"RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES:\s*Int\s*=\s*8\s*\*\s*1024\s*\*\s*1024", "Kotlin open envelope max bytes", errors)
    require_regex(texts, kotlin, r"RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES:\s*Int\s*=\s*128", "Kotlin Pallas open-envelope transcript label max bytes", errors)
    require_contains(
        texts,
        java,
        (
            "RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
            "RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.equals(circuitId)",
            "hopCount >= 1",
            "hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "previousHopCount >= 1",
            "previousHopCount < RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "indexOfSlice(archivePayload, verifierKeyCommitment) < 0",
        ),
        "Android witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_contains(
        texts,
        kotlin,
        (
            "RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
            "circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 &&",
            "hopCount >= 1",
            "hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "previousHopCount >= 1",
            "previousHopCount < RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            "archivePayload.indexOfSlice(verifierKeyCommitment) >= 0",
        ),
        "Kotlin witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_contains(
        texts,
        java,
        (
            "final int encodedLength = cursor - offset",
            "shift >= 63 && chunk != 0L",
            "encodedLength > 5",
            "encodedLength > 1 && value < (1L << (7 * (encodedLength - 1)))",
            "value > Integer.MAX_VALUE || value > buffer.length",
        ),
        "Android Java lineage archive canonical compact length guard",
        errors,
    )
    require_contains(
        texts,
        kotlin,
        (
            "val encodedLength = cursor - offset",
            "shift < 63 || chunk == 0L",
            "encodedLength <= 5",
            "encodedLength <= 1 || value >= (1L shl (7 * (encodedLength - 1)))",
            "value <= Int.MAX_VALUE.toLong() && value <= buffer.size.toLong()",
        ),
        "Kotlin lineage archive canonical compact length guard",
        errors,
    )
    for relative, label in (
        (java_test, "Android Java portable lineage key artifact tests"),
        (kotlin_test, "Kotlin portable lineage key artifact tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "lineageKeyArtifactPackagesValidateReleaseProfiles",
                "lineageKeyArtifactsForInit",
                "lineageKeyArtifactsForAppend",
                "validateLineageKeyArtifacts",
                "isSupportedLineageKeyArtifactOpeningLen",
                "RECURSIVE_AGGREGATION_PROOF_BACKEND",
                "halo2/kzg",
                '"proof_circuit_id"',
                '"verifier_opening_len"',
                '"lineage_verifier_key"',
                '"lineage_proving_key_archive"',
                "lineageVerifierKey(",
                "lineageProvingKeyArchive(",
                "verifierKeyCommitment",
                "appendVerifierKey",
                "duplicateCidVerifierKey",
                "missingCircuitArchive",
                "smuggledCircuitArchive",
                "wrongCommitmentArchive",
                "smuggledCommitmentArchive",
                "wrongVersionArchive",
                "emptyProvingKeyArchive",
                "trailingPayloadArchive",
                "oldSchemaArchive",
                "packedStructArchive",
                "fieldBitsetArchive",
                "overlongVersionLengthArchive",
                "oversizedTerminalCompactLengthArchive",
                "hugeCanonicalCompactLengthArchive",
                "overlongCircuitStringArchive",
                "invalidUtf8CircuitArchive",
                "kagemushaOverlongCompactLength",
                "kagemushaOversizedTerminalCompactLength",
                "kagemushaHugeCanonicalCompactLength",
                "lineageProvingKeyArchiveRaw",
                "not-zk1",
                "not-norito",
                "exposedVerifierKey",
                "exposedProvingKeyArchive",
            ),
            label,
            errors,
        )
    require_contains(
        texts,
        java_test,
        (
            "LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
            "OLD_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH",
        ),
        "Android Java lineage archive schema fixture hashes",
        errors,
    )
    require_contains(
        texts,
        kotlin_test,
        (
            "lineageProvingKeyArchiveSchemaHash",
            "oldLineageProvingKeyArchiveSchemaHash",
        ),
        "Kotlin lineage archive schema fixture hashes",
        errors,
    )
    require_contains(
        texts,
        kotlin,
        (
            "validateLineageKeyArtifacts(artifacts: LineageKeyArtifacts?)",
            "lineageVerifierKeyBackend: String?",
            "lineageVerifierKey: ByteArray?",
            "lineageProvingKeyArchive: ByteArray?",
            '"lineage_key_artifacts"',
        ),
        "Kotlin Java-callable lineage key artifact null validation",
        errors,
    )
    require_contains(
        texts,
        kotlin_test,
        (
            "lineageKeyArtifactsRejectJavaNullsWithStableFieldMarkers",
            "validateLineageKeyArtifacts(null)",
            "KagemushaRecursiveSpendProver.lineageKeyArtifacts(",
            "KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(",
            '"lineage_key_artifacts"',
            '"proof_circuit_id"',
            '"lineage_verifier_key"',
            '"lineage_proving_key_archive"',
        ),
        "Kotlin Java-callable lineage key artifact null negative tests",
        errors,
        )
    require_contains(
        texts,
        kotlin_test,
        (
            "nativeArchiveEntrypointsRejectJavaNullsWithStableFieldMarkers",
            "KagemushaRecursiveSpendProver.initSpend(null)",
            "KagemushaRecursiveSpendProver.lineageAppendBoundary(null)",
            "KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(null, validArchive)",
            "KagemushaRecursiveSpendProver.lineageWitnessAppendResult(validArchive, validArchive, null)",
            "KagemushaRecursiveSpendProver.verifySpend(null)",
            "KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(",
            "recursiveCompactVerifierKeysArchive must not be empty",
            "compactTokenArchive must not be empty",
        ),
        "Kotlin Java-callable native archive null negative tests",
        errors,
    )
    for relative, label in (
        ("java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java", "Android Java compact-token input Norito guard tests"),
        ("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt", "Kotlin compact-token input Norito guard tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "kagemushaRecordBackedNativeProverValidatesInput",
                "recordBundleArchive must not be empty",
                "recordBundleArchive must not exceed",
                "recordBundleArchive must be a valid Norito archive",
                "recordBundleArchive must contain a non-empty Norito payload",
                "kagemushaCompactNativeInputCopiesBeforeDispatch",
                "KagemushaCompactPaymentTokenProver.ownedNativeInput",
                "archive[6] =",
                "kagemushaRecursiveAggregationNativeProverValidatesInput",
                "pallasOpenEnvelopesArchive must not be empty",
                "pallasOpenEnvelopesArchive must not exceed",
                "pallasOpenEnvelopesArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrame(0x4b)",
            ),
            label,
            errors,
        )
        text = texts[relative]
        require(
            text.count("recordBundleArchive must not exceed") >= 2,
            f"{label} must cover compact-token and recursive-aggregation record oversized inputs",
            errors,
        )
        require(
            "pallasOpenEnvelopesArchive must not exceed" in text,
            f"{label} must cover recursive-aggregation Pallas oversized inputs",
            errors,
        )
    require_contains(
        texts,
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
        (
            "kagemushaRecordBackedNativeProversRejectJavaNullsWithStableFieldMarkers",
            "KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(null)",
            "proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
            "null,",
            "validArchive,",
            "pallasOpenEnvelopesArchive must not be empty",
        ),
        "Kotlin record-backed native archive null negative tests",
        errors,
    )
    require_contains(
        texts,
        java_test,
        (
            "rejectsNullAndEmptyNativeRedeemOutput",
            "assertRejectsMalformedNativeRedeemOutput",
            "compressed[22] = 1",
            "unsupportedFlags[39] = 0x08",
            "invalidFieldBitset[39] = 0x20",
            "withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), new byte[] {0x7f})",
            "withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), new byte[65])",
            "native redeem returned invalid Norito archive",
            "native redeem returned empty Norito payload",
        ),
        "Android Java recursive spend native output Norito guard tests",
        errors,
    )
    require_contains(
        texts,
        kotlin_test,
        (
            "rejectsNullAndEmptyNativeRedeemOutput",
            "assertRejectsMalformedNativeRedeemOutput",
            "compressed[22] = 1",
            "unsupportedFlags[39] = 0x08",
            "invalidFieldBitset[39] = 0x20",
            "withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), byteArrayOf(0x7f))",
            "withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), ByteArray(65))",
            "native redeem returned invalid Norito archive",
            "native redeem returned empty Norito payload",
        ),
        "Kotlin recursive spend native output Norito guard tests",
        errors,
    )
    for relative, label in (
        (java_test, "Android Java recursive spend input Norito guard tests"),
        (kotlin_test, "Kotlin recursive spend input Norito guard tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch",
                "requestArchive must not exceed",
                "bundleArchive must not exceed",
                "previousWitnessArchive must not exceed",
                "requestArchive must be a valid Norito archive",
                "bundleArchive must be a valid Norito archive",
                "previousWitnessArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrameWithPayload",
            ),
            label,
            errors,
        )
        text = texts[relative]
        require(
            text.count("requestArchive must not exceed") >= 3,
            f"{label} must cover single, witness-from-init, and witness-append oversized request inputs",
            errors,
        )
        require(
            "bundleArchive must not exceed" in text,
            f"{label} must cover oversized bundle inputs",
            errors,
        )
        require(
            "previousWitnessArchive must not exceed" in text,
            f"{label} must cover oversized previous-witness inputs",
            errors,
        )
    require_contains(
        texts,
        java_test,
        (
            "copiesNativeInputArchivesBeforeDispatch",
            "KagemushaRecursiveSpendProver.ownedNativeInput(archive, \"requestArchive\")",
            "archive[6] = (byte) 0x7F",
            "ownedArchive != archive",
            "Arrays.equals(expected, ownedArchive)",
        ),
        "Android Java recursive spend archive input copy tests",
        errors,
    )
    require_contains(
        texts,
        kotlin_test,
        (
            "copiesNativeInputArchivesBeforeDispatch",
            "KagemushaRecursiveSpendProver.ownedNativeInput(",
            "archive[6] = 0x7f.toByte()",
            "ownedArchive === archive",
            "assertContentEquals(expected, ownedArchive)",
        ),
        "Kotlin recursive spend archive input copy tests",
        errors,
    )
    for relative, label in (
        (java_test, "Android Java recursive compact prover input Norito guard tests"),
        (kotlin_test, "Kotlin recursive compact prover input Norito guard tests"),
    ):
        require_contains(
            texts,
            relative,
            (
                "validRecursiveCompactInput",
                "ownedRecursiveCompactInput",
                "KagemushaRecursiveCompactPaymentTokenProver.ownedNativeInput",
                "recursiveCompactCopyInput[6] =",
                "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
                "recordBundleArchive must not be empty",
                "pallasOpenEnvelopesArchive must not be empty",
                "recordBundleArchive must not exceed",
                "pallasOpenEnvelopesArchive must not exceed",
                "compactTokenArchive must not exceed",
                "recordBundleArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must be a valid Norito archive",
                "recordBundleArchive must contain a non-empty Norito payload",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrameWithPayload",
            ),
            label,
            errors,
        )
        text = texts[relative]
        require(
            "compactTokenArchive must not exceed" in text,
            f"{label} must cover oversized compact-token verifier inputs",
            errors,
        )


def check_csharp(texts, errors):
    script = read(CSHARP_SDK_TEST_COMMAND)
    require(
        'DOTNET_BIN="${KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN:-dotnet}"' in script,
        "Kagemusha C# SDK script must keep the documented dotnet override variable",
        errors,
    )
    require(
        'DOTNET_VERSION="$("${DOTNET_BIN}" --version)"' in script,
        "Kagemusha C# SDK script must print the selected dotnet version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${DOTNET_VERSION}"' in script,
        "Kagemusha C# SDK script must emit the selected dotnet version",
        errors,
    )
    require(
        "8.0.*) ;;" in script,
        "Kagemusha C# SDK script must reject non-.NET-8 SDK versions",
        errors,
    )
    require(
        "printf 'dotnet --info:\\n'" in script
        and '"${DOTNET_BIN}" --info' in script,
        "Kagemusha C# SDK script must emit dotnet --info host evidence",
        errors,
    )
    require(
        'BRIDGE_TARGET_DIR="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_BRIDGE_TARGET_DIR:-${TMPDIR:-/tmp}/iroha-kagemusha-csharp-native-target}"' in script,
        "Kagemusha C# SDK script must keep an overrideable native bridge target dir",
        errors,
    )
    require(
        'CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" cargo build -p connect_norito_bridge' in script,
        "Kagemusha C# SDK script must build the native bridge before P/Invoke tests",
        errors,
    )
    require(
        'BRIDGE_LIBRARY_NAME="connect_norito_bridge.dll"' in script
        and 'BRIDGE_LIBRARY_NAME="libconnect_norito_bridge.dylib"' in script
        and 'BRIDGE_LIBRARY_NAME="libconnect_norito_bridge.so"' in script,
        "Kagemusha C# SDK script must resolve the platform-specific native bridge library name",
        errors,
    )
    require(
        'BRIDGE_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}/${BRIDGE_LIBRARY_NAME}"' in script
        and '[[ ! -f "${BRIDGE_LIBRARY_PATH}" ]]' in script
        and "connect_norito_bridge native bridge:" in script,
        "Kagemusha C# SDK script must verify and print the freshly built native bridge path",
        errors,
    )
    require(
        "sha256sum" in script
        and "shasum -a 256" in script
        and "connect_norito_bridge native bridge sha256:" in script,
        "Kagemusha C# SDK script must print the freshly built native bridge SHA-256",
        errors,
    )
    js_parity_test = read(JS_PARITY_TEST_PATH)
    for marker in (
        "assertRunnerPrintsDotnetAndBridgeEvidence",
        "fake cargo bridge build",
        "connect_norito_bridge native bridge sha256:",
        "Kagemusha C# SDK runner prints host and bridge evidence before tests",
    ):
        require(
            marker in js_parity_test,
            f"JavaScript C# SDK runner host/bridge evidence meta-test must contain {marker}",
            errors,
        )
    require(
        'export DYLD_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"' in script,
        "Kagemusha C# SDK script must expose the native bridge on macOS loader path",
        errors,
    )
    require(
        'export LD_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"' in script,
        "Kagemusha C# SDK script must expose the native bridge on Linux loader path",
        errors,
    )
    require(
        'export PATH="${BRIDGE_LIBRARY_DIR}:${PATH}"' in script,
        "Kagemusha C# SDK script must expose the native bridge on Windows loader path",
        errors,
    )
    require(
        '--filter "FullyQualifiedName~KagemushaRecursiveSpendNativeTests|FullyQualifiedName~PrivacyNativeTests|FullyQualifiedName~TransactionBuilderTests"' in script,
        "Kagemusha C# SDK script must run recursive spend, privacy native, and transaction builder tests",
        errors,
    )
    relative = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    require_contains(
        texts,
        relative,
        (
            "Init",
            "Append",
            "TransitionProfileInit",
            "TransitionProfileAppend",
            "LineageAppendBoundary",
            "LineageWitnessFromInitResult",
            "LineageWitnessAppendResult",
            "Verify",
            "Redeem",
            "ProveVerifiedCompactPaymentTokenWithRecords",
            "ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
            "KagemushaRecursiveSpendTransitionProfileArchive",
            "KagemushaRecursiveSpendLineageAppendBoundaryArchive",
            "KagemushaRecursiveSpendLineageWitnessArchive",
            "KagemushaCompactPaymentTokenArchive",
            "KagemushaRecursiveAggregationProofBundleArchive",
            "public abstract class KagemushaNativeArchive",
            "KagemushaArchiveBytes.Copy",
            "return (byte[])noritoBytes.Clone();",
            "public byte[] NoritoBytes =>",
            "NormalizeRecursiveCompactVerifierOutput",
            "invalid boolean output",
            "KagemushaRecursiveSpendLineageKeyArtifacts",
            "LineageKeyArtifactsForInit",
            "LineageKeyArtifactsForAppend",
            "ValidateLineageKeyArtifacts",
            "IsSupportedLineageKeyArtifactOpeningLen",
            "IsCompactPaymentTokenProverAvailable",
            "IsRecursiveAggregationProofBundleProverAvailable",
            "IsSupportedAppendProofTransition",
        ),
        "C# public API",
        errors,
    )
    require_contains(texts, relative, REQUIRED_C_SYMBOLS, "C# P/Invoke symbols", errors)
    require_contains(
        texts,
        relative,
        REQUIRED_RECORD_BACKED_KAGEMUSHA_C_SYMBOLS,
        "C# record-backed Kagemusha P/Invoke symbols",
        errors,
    )
    require_contains(
        texts,
        relative,
        (
            "RequiredBridgeAbiVersion = 6",
            "RecursiveSpendLineageWitnesslessMaxHopsV1 = 64",
            "RecursiveSpendLineageTransitionCircuitWiredV1 = true",
            "RecursivePreviousProofOpenEnvelopesRequiredCountV1 = 1",
            "RecursivePreviousProofOpenEnvelopesMaxBytes = 8 * 1024 * 1024",
            "RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes = 128",
            'RecursiveAggregationProofBackend = "halo2/ipa"',
            "RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1",
            "RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1",
            '"kagemusha-recursive-aggregation-v1"',
            '"kagemusha-recursive-spend-lineage-v1"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-digest"',
            '"iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"',
            '"iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"',
            '"iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"',
        ),
        "C# constants",
        errors,
    )
    require_contains(
        texts,
        relative,
        (
            "Probe(NativeTransitionProfileInit)",
            "Probe(NativeTransitionProfileAppend)",
            "Probe(NativeLineageAppendBoundary)",
            "Probe((NativeArchivePairCall)NativeLineageWitnessFromInitResult)",
            "Probe((NativeArchiveTripleCall)NativeLineageWitnessAppendResult)",
            "TryProbeCompactPaymentTokenSymbol",
            "TryProbeRecursiveAggregationProofBundleSymbol",
            "Probe((NativeArchiveCall)NativeCompactPaymentToken)",
            "Probe((NativeArchivePairCall)NativeRecursiveAggregationProofBundle)",
        ),
        "C# native availability probe",
        errors,
    )
    require_contains(
        texts,
        relative,
        (
            "RequireValidInputArchive",
            "Request archive",
            "Bundle archive",
            "Record bundle archive",
            "Pallas open-envelopes archive",
            "archive.Length > NativeArchiveMaxBytes",
            "compactTokenArchive.Length > NativeArchiveMaxBytes",
            "must be a valid Norito archive.",
            "must not exceed",
            "must contain a non-empty Norito payload.",
            "PrivacyNative.IsNoritoV1Archive(bytes)",
            "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(bytes)",
        ),
        "C# recursive spend input Norito guard",
        errors,
    )
    csharp_text = texts[relative]
    if "if (archive.Length > NativeArchiveMaxBytes)" in csharp_text and "var bytes = archive.ToArray();" in csharp_text:
        require(
            csharp_text.index("if (archive.Length > NativeArchiveMaxBytes)")
            < csharp_text.index("var bytes = archive.ToArray();"),
            "C# recursive spend input guard must reject oversized spans before copying",
            errors,
        )
    if (
        "if (compactTokenArchive.Length > NativeArchiveMaxBytes)" in csharp_text
        and "var compactToken = compactTokenArchive.ToArray();" in csharp_text
    ):
        require(
            csharp_text.index("if (compactTokenArchive.Length > NativeArchiveMaxBytes)")
            < csharp_text.index("var compactToken = compactTokenArchive.ToArray();"),
            "C# recursive compact verifier input guard must reject oversized spans before copying",
            errors,
        )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "RecursiveSpendArchiveWrappersDefensivelyCopyNoritoBytes",
            "RecursiveSpendArchiveWrappersRejectUnsafeNoritoBytes",
            "RecursiveCompactVerifierOutputRejectsInvalidNativeBoolean",
            "NormalizeRecursiveCompactVerifierOutput(symbol, 0, 2)",
            "invalid boolean output 2",
            "bridge error code -311",
            "Func<byte[], KagemushaNativeArchive>",
            "source[0] = 0x7f",
            "firstRead[1] = 0x7f",
            "factory(null!)",
            "oversizedArchive",
            "NativeArchiveMaxBytes + 1",
            "must not exceed",
            "compressed[22] = 1",
            "unsupportedFlags[39] = 0x08",
            "invalidFieldBitset[39] = 0x20",
            "WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f })",
            "WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65])",
            "new KagemushaRecursiveSpendArchive(bytes)",
            "new KagemushaRecursiveSpendTransitionProfileArchive(bytes)",
            "new KagemushaRecursiveSpendLineageAppendBoundaryArchive(bytes)",
            "new KagemushaRecursiveSpendLineageWitnessArchive(bytes)",
            "new KagemushaRecursiveSpendVerifyArchive(bytes)",
            "new KagemushaRecursiveSpendRedeemInstructionArchive(bytes)",
            "new KagemushaCompactPaymentTokenArchive(bytes)",
            "new KagemushaRecursiveAggregationProofBundleArchive(bytes)",
            "new KagemushaRecursiveCompactPaymentTokenArchive(bytes)",
        ),
        "C# recursive spend archive wrapper copy tests",
        errors,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge",
            "RecursiveSpendNativeRejectsOversizedArchivesBeforeLoadingNativeBridge",
            "RecursiveSpendNativeRejectsEmptyPayloadArchivesBeforeLoadingNativeBridge",
            "CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "CompactTokenProverRejectsOversizedInputsBeforeLoadingNativeBridge",
            "CompactTokenProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveAggregationProverRejectsOversizedInputsBeforeLoadingNativeBridge",
            "RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsOversizedInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveCompactVerifierRejectsOversizedInputBeforeLoadingNativeBridge",
            "AssertOversizedArchive",
            "Request archive must not exceed",
            "Bundle archive must not exceed",
            "Previous witness archive must not exceed",
            "Compact token archive must not exceed",
            "Record bundle archive must be a valid Norito archive",
            "Pallas open-envelopes archive must contain a non-empty Norito payload",
            "KagemushaNoritoFrameWithPayload",
            "AssertRejectsMalformedEverywhere",
            "AssertRejectsMalformedEverywhere(compressed, validArchive)",
            "AssertRejectsMalformedEverywhere(unsupportedFlags, validArchive)",
            "AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive)",
        ),
        "C# recursive spend input Norito guard tests",
        errors,
    )
    require_contains(
        texts,
        "csharp/src/Hyperledger.Iroha.Sdk/Transactions/KagemushaInstructionArchiveInstruction.cs",
        (
            "public enum KagemushaInstructionType",
            "RedeemRecursive",
            "ArchiveTypeName",
            "WireName",
            '"KagemushaTransfer"',
            '"RedeemKagemushaRecursive"',
            '"iroha_data_model::isi::offline::KagemushaTransfer"',
            '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
            "CopyAndValidateArchive",
            "KagemushaRecursiveSpendNative.NativeArchiveMaxBytes",
            "PrivacyNative.IsNoritoV1Archive(copy)",
            "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(copy)",
            "NoritoCodec.SchemaHash(instructionType.WireName())",
            "SequenceEqual(expectedSchema)",
            "KagemushaRecursiveSpendRedeemInstructionArchive",
            "EncodeFramedPayload",
            "return InstructionArchive;",
        ),
        "C# Kagemusha instruction archive transaction instruction",
        errors,
    )
    require_contains(
        texts,
        "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionInstruction.cs",
        (
            "internal virtual byte[] EncodeFramedPayload",
            "NoritoCodec.Encode(TypeName, EncodePayload(context))",
            "KagemushaInstructionArchive(",
            "KagemushaRecursiveRedeem(",
            "KagemushaInstructionArchiveInstruction.RedeemRecursive",
        ),
        "C# Kagemusha instruction transaction factories",
        errors,
    )
    require_contains(
        texts,
        "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionEncodingContext.cs",
        (
            "instruction.EncodeFramedPayload(this)",
            "writer.WriteField(EncodeString(instruction.WireId))",
            "writer.WriteField(EncodeBytesVec(framedInstruction))",
        ),
        "C# Kagemusha instruction archive pass-through encoder",
        errors,
    )
    require_contains(
        texts,
        "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
        (
            "KagemushaInstructionArchive(",
            "KagemushaRecursiveRedeem(",
            "KagemushaRecursiveSpendNative.Redeem(redeemRequestArchive)",
            "KagemushaRecursiveSpendRedeemInstructionArchive",
        ),
        "C# Kagemusha recursive redeem transaction builder",
        errors,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs",
        (
            "AddInstructionAcceptsKagemushaInstructionArchiveFactories",
            "BuildSignedEmbedsKagemushaInstructionArchiveWithoutReframing",
            "KagemushaInstructionArchiveRejectsMalformedWrongTypeAndMismatchedType",
            "KagemushaInstructionType.RedeemRecursive",
            "KagemushaInstructionType.Transfer",
            "new KagemushaRecursiveSpendRedeemInstructionArchive(redeemArchive)",
            "Assert.Equal(archive, instruction.Payload)",
            'Assert.Equal("iroha_data_model::isi::offline::RedeemKagemushaRecursive", instruction.WireId)',
            'NoritoCodec.Encode("KagemushaRecursiveSpendRedeemRequestV1", new byte[] { 1, 2, 3 })',
            "compressed[22] = 1",
            "unsupportedFlags[39] = 0x08",
            "invalidFieldBitset[39] = 0x20",
            "WithHeaderPadding",
            "new byte[65]",
        ),
        "C# Kagemusha instruction transaction builder tests",
        errors,
    )
    require_contains(
        texts,
        relative,
        (
            "RecursiveSpendLineageTransitionCircuitWiredV1",
            "circuitId == RecursiveSpendLineageProofCircuitIdV1",
            "hopCount >= 1",
            "hopCount <= RecursiveSpendLineageWitnesslessMaxHopsV1",
            "previousHopCount >= 1",
            "previousHopCount < RecursiveSpendLineageWitnesslessMaxHopsV1",
            "ValidateLineageKeyArtifactPackageBinding",
            "LineageVerifierKeyEnvelopeCircuitId",
            "LineageProvingKeyArchivePayload",
            "DecodeLineageProvingKeyArchivePayload",
            "KagemushaLineageProvingKeyArchiveSchemaHash",
            "KagemushaLineageProvingKeyArchiveVersionV1",
            "KagemushaNoritoPackedStructFlag",
            "PrivacyNoritoFieldBitsetFlag",
            "encodedLength > 1",
            "value < (1UL << (7 * (encodedLength - 1)))",
            "shift >= 63 && currentValue > 1",
            "value > int.MaxValue",
            "VerifyingKeyCommitment",
            "KagemushaZk1TlvCid1",
            "KagemushaZk1TlvIpaK",
            "KagemushaZk1TlvH2Vk",
            "archivePayload.AsSpan().IndexOf(circuitIdBytes)",
            "archivePayload.AsSpan().IndexOf(verifierKeyCommitment)",
            "archive.CircuitFamily != proofCircuitId",
            "archive.VerifierKeyCommitment.AsSpan().SequenceEqual(verifierKeyCommitment)",
            "archive.ProvingKey.Length == 0",
            '"proof_circuit_id"',
            '"verifier_opening_len"',
            '"lineage_verifier_key"',
            '"lineage_proving_key_archive"',
        ),
        "C# witnessless Reserved-lineage helper bounds",
        errors,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "LineageKeyArtifactsForInit",
            "LineageKeyArtifactsForAppend",
            "ValidateLineageKeyArtifacts",
            "IsSupportedLineageKeyArtifactOpeningLen",
            "RecursiveAggregationProofBackend",
            "halo2/kzg",
            "proof_circuit_id",
            "verifier_opening_len",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "KagemushaLineageVerifierKey",
            "KagemushaLineageProvingKeyArchive",
            "KagemushaVerifierKeyCommitment",
            "KagemushaNoritoFrameFromPayload",
            "appendVerifierKey",
            "duplicateCidVerifierKey",
            "missingCircuitArchive",
            "wrongCommitmentArchive",
            "smuggledCircuitArchive",
            "smuggledCommitmentArchive",
            "wrongVersionArchive",
            "emptyProvingKeyArchive",
            "trailingPayloadArchive",
            "oldSchemaArchive",
            "packedStructArchive",
            "fieldBitsetArchive",
            "overlongVersionLengthArchive",
            "oversizedTerminalCompactLengthArchive",
            "hugeCanonicalCompactLengthArchive",
            "overlongCircuitStringArchive",
            "invalidUtf8CircuitArchive",
            "KagemushaOverlongCompactLength",
            "KagemushaOversizedTerminalCompactLength",
            "KagemushaHugeCanonicalCompactLength",
            "KagemushaLineageProvingKeyArchiveRaw",
            "KagemushaLineageProvingKeyArchiveSchemaHash",
            "returnedVerifierKey",
            "returnedProvingKeyArchive",
        ),
        "C# portable lineage key artifact tests",
        errors,
    )


def check_sdk_readme_previous_proof_boundary(texts, errors):
    required = (
        "previous_recursive_proof_open_envelopes_archive",
        "opaque native prover material",
        "must not construct, rewrite, or mutate",
        "vk_commitment",
        "public_inputs_schema_hash",
        "domain_tag",
        "against the exact previous bundle",
        "the append-boundary helper",
        "append-boundary digest uses the public",
        "chain/asset and final-root/current-note binding",
        "semantic previous bundles keep using semantic append",
        "plus a record-backed lineage witness",
        "previous recursive proof bytes",
        "`recursive_proof_chain_digest`",
        "native-owned accumulator digests",
        "lineage/aggregation transcript",
        "fixed-window schedule/shared-manifest/table-base",
        "verifier-witness batch",
        "transition-profile",
        "append-opening-preflight",
        "append-boundary",
        "scalar-projection",
        "previous/resulting accumulator digests",
        "must not derive, supply, or patch accumulator state",
    )
    for relative in SDK_README_PATHS:
        text = re.sub(r"\s+", " ", texts[relative])
        for needle in required:
            require(
                needle in text,
                f"{relative} missing previous-proof opening archive boundary: {needle}",
                errors,
            )
        require(
            "Future Reserved-lineage append output" not in text,
            f"{relative} still describes Reserved-lineage append output as future",
            errors,
        )


def check_sdk_readme_instruction_transaction_surface(texts, errors):
    common_required = (
        "KagemushaTransfer",
        "RedeemKagemushaRecursive",
        "valid Norito archives",
        "empty, malformed, tampered, or wrong-type instruction archives",
        "recursive redeem derivation inside",
    )
    sdk_required = {
        "IrohaSwift/README.md": (
            "KagemushaInstructionTransactionRequest",
            "IrohaSDK.buildKagemushaRecursiveRedeem(...)",
        ),
        "java/iroha_android/README.md": (
            "KagemushaInstructionArchives",
            "builds a single archived instruction transaction payload",
            "derives the redeem instruction from a native recursive redeem request",
        ),
        "kotlin/README.md": (
            "KagemushaInstructionArchives",
            "builds a single archived instruction transaction payload",
            "derives the redeem instruction from a native recursive redeem request",
        ),
        "csharp/README.md": (
            "TransactionInstruction.KagemushaInstructionArchive(...)",
            "KagemushaInstructionArchiveInstruction",
            "TransactionBuilder.KagemushaInstructionArchive(...)",
            "TransactionBuilder.KagemushaRecursiveRedeem(...)",
        ),
        "javascript/iroha_js/README.md": (
            "buildKagemushaInstructionArchiveInstruction({ instructionType, instructionArchive })",
            "buildKagemushaInstructionTransaction(...)",
            "buildKagemushaRecursiveRedeemTransaction(...)",
        ),
        "python/iroha_python/README.md": (
            "kagemusha_instruction_archive_instruction(instruction_type, instruction_archive)",
            "build_kagemusha_instruction_transaction(...)",
            "build_kagemusha_recursive_redeem_transaction(...)",
            "TransactionDraft.kagemusha_instruction_archive(...)",
            "TransactionDraft.kagemusha_recursive_redeem(...)",
        ),
    }
    for relative in SDK_README_PATHS:
        text = re.sub(r"\s+", " ", texts[relative])
        for needle in (*common_required, *sdk_required[relative]):
            require(
                needle in text,
                f"{relative} missing Kagemusha instruction transaction docs: {needle}",
                errors,
            )


def check_sdk_accumulator_digest_is_native_owned(texts, errors):
    sdk_sources = (
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
        "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
        "javascript/iroha_js/index.d.ts",
        "python/iroha_python/src/iroha_python/kagemusha.py",
        "python/iroha_python/src/iroha_python/__init__.py",
    )
    forbidden = (
        r"\b[A-Za-z0-9_]*lineageDigest\b",
        r"\b[A-Za-z0-9_]*LineageDigest\b",
        r"\b[A-Za-z0-9_]*lineage_digest\b",
        r"\b[A-Za-z0-9_]*aggregationTranscriptDigest\b",
        r"\b[A-Za-z0-9_]*AggregationTranscriptDigest\b",
        r"\b[A-Za-z0-9_]*aggregation_transcript_digest\b",
        r"\b[A-Za-z0-9_]*fixedWindowTableScheduleDigest\b",
        r"\b[A-Za-z0-9_]*FixedWindowTableScheduleDigest\b",
        r"\b[A-Za-z0-9_]*fixed_window_table_schedule_digest\b",
        r"\b[A-Za-z0-9_]*fixedWindowSharedTableManifestDigest\b",
        r"\b[A-Za-z0-9_]*FixedWindowSharedTableManifestDigest\b",
        r"\b[A-Za-z0-9_]*fixed_window_shared_table_manifest_digest\b",
        r"\b[A-Za-z0-9_]*fixedWindowTableBaseDigest\b",
        r"\b[A-Za-z0-9_]*FixedWindowTableBaseDigest\b",
        r"\b[A-Za-z0-9_]*fixed_window_table_base_digest\b",
        r"\b[A-Za-z0-9_]*verifierWitnessBatchDigest\b",
        r"\b[A-Za-z0-9_]*VerifierWitnessBatchDigest\b",
        r"\b[A-Za-z0-9_]*verifier_witness_batch_digest\b",
        r"\b[A-Za-z0-9_]*recursiveProofChainDigest\b",
        r"\b[A-Za-z0-9_]*RecursiveProofChainDigest\b",
        r"\b[A-Za-z0-9_]*recursive_proof_chain_digest\b",
        r"\b[A-Za-z0-9_]*proofChainDigest\b",
        r"\b[A-Za-z0-9_]*ProofChainDigest\b",
        r"\b[A-Za-z0-9_]*proof_chain_digest\b",
        r"\b[A-Za-z0-9_]*transitionProfileBindingDigest\b",
        r"\b[A-Za-z0-9_]*TransitionProfileBindingDigest\b",
        r"\b[A-Za-z0-9_]*transition_profile_binding_digest\b",
        r"\b[A-Za-z0-9_]*appendOpeningPreflightDigest\b",
        r"\b[A-Za-z0-9_]*AppendOpeningPreflightDigest\b",
        r"\b[A-Za-z0-9_]*append_opening_preflight_digest\b",
        r"\b[A-Za-z0-9_]*appendBoundaryDigest\b",
        r"\b[A-Za-z0-9_]*AppendBoundaryDigest\b",
        r"\b[A-Za-z0-9_]*append_boundary_digest\b",
        r"\b[A-Za-z0-9_]*recursiveVerifierScalarProjectionDigest\b",
        r"\b[A-Za-z0-9_]*RecursiveVerifierScalarProjectionDigest\b",
        r"\b[A-Za-z0-9_]*recursive_verifier_scalar_projection_digest\b",
        r"\b[A-Za-z0-9_]*previousAccumulatorDigest\b",
        r"\b[A-Za-z0-9_]*PreviousAccumulatorDigest\b",
        r"\b[A-Za-z0-9_]*previous_accumulator_digest\b",
        r"\b[A-Za-z0-9_]*resultingAccumulatorDigest\b",
        r"\b[A-Za-z0-9_]*ResultingAccumulatorDigest\b",
        r"\b[A-Za-z0-9_]*resulting_accumulator_digest\b",
        r"\b[A-Za-z0-9_]*accumulatorDigest\b",
        r"\b[A-Za-z0-9_]*AccumulatorDigest\b",
        r"\b[A-Za-z0-9_]*accumulator_digest\b",
    )
    for relative in sdk_sources:
        for pattern in forbidden:
            require_not_regex(
                texts,
                relative,
                pattern,
                f"{relative} accumulator digest public input",
                errors,
            )


def check_sdk_readme_recursive_compact_unavailable_boundary(texts, errors):
    common_required = (
        "recursive_compact_v1",
        "kagemusha-recursive-compact-v1",
        "one-hop LEN=4 compact-token proof path",
        "packaged compact one-hop proving-key",
        "recursive-spend compact projection verifier",
        "raw Norito compact-token and verifier-record archives",
        "native boolean receiver result",
        "Reserved-lineage recursive spend",
        "proof-composition reservation",
        "generic compact-token reservation",
        "multi-hop verifier-batch reservation",
        "reserved ABI-7 state",
    )
    sdk_required = {
        "IrohaSwift/README.md": (
            "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "verifyRecursiveCompactPaymentToken",
            "isNativeAvailable",
            "isVerifierNativeAvailable",
            "verifyRecursiveSpendCompactPaymentTokenProjection(compactTokenArchive:verifierRecordArchive:blockHeight:)",
            "isProjectionVerifierNativeAvailable",
            "type-safe optional `UInt64` `blockHeight`",
            "KagemushaRecursiveCompactPaymentTokenProverError.recursiveCompactUnavailable",
        ),
        "java/iroha_android/README.md": (
            "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "verifyRecursiveCompactPaymentToken",
            "isNativeAvailable()",
            "isVerifierNativeAvailable()",
            "verifyRecursiveSpendCompactPaymentTokenProjection(...)",
            "verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(...)",
            "isProjectionVerifierNativeAvailable()",
            "canonical unsigned decimal `String` or `BigInteger`",
            "isRecursiveCompactUnavailable(Throwable)",
            "IllegalStateException",
            "IllegalArgumentException",
        ),
        "kotlin/README.md": (
            "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "verifyRecursiveCompactPaymentToken",
            "isNativeAvailable()",
            "isVerifierNativeAvailable()",
            "verifyRecursiveSpendCompactPaymentTokenProjection(...)",
            "verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(...)",
            "isProjectionVerifierNativeAvailable()",
            "canonical unsigned decimal `String` or `BigInteger`",
            "isRecursiveCompactUnavailable(error)",
            "IllegalStateException",
            "IllegalArgumentException",
        ),
        "csharp/README.md": (
            "ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "VerifyRecursiveCompactPaymentToken",
            "IsRecursiveCompactPaymentTokenProverAvailable",
            "IsRecursiveCompactPaymentTokenVerifierAvailable",
            "VerifyRecursiveSpendCompactPaymentTokenProjection(...)",
            "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable()",
            "-312",
            "InvalidOperationException",
            "ArgumentException",
        ),
        "javascript/iroha_js/README.md": (
            "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "kagemushaVerifyRecursiveCompactPaymentToken",
            "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
            "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
            "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(...)",
            "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable()",
            "non-integer, bool, negative-height, unsafe-number, or out-of-u64 height inputs",
        ),
        "python/iroha_python/README.md": (
            "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
            "kagemusha_verify_recursive_compact_payment_token",
            "recursive_compact_key_artifacts_archive",
            "recursive_compact_verifier_keys_archive",
            "is_kagemusha_recursive_compact_payment_token_prover_available",
            "is_kagemusha_recursive_compact_payment_token_verifier_available",
            "kagemusha_verify_recursive_spend_compact_payment_token_projection(...)",
            "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height(...)",
            "non-integer, bool, negative-height, or out-of-u64 height inputs",
            "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()",
            "RuntimeError",
        ),
    }
    for relative in SDK_README_PATHS:
        text = re.sub(r"\s+", " ", texts[relative])
        for needle in (*common_required, *sdk_required[relative]):
            require(
                needle in text,
                f"{relative} missing recursive compact ABI-7 boundary: {needle}",
                errors,
            )


def check_offline_doc_recursive_compact_projection_sdk_surface(texts, errors):
    text = re.sub(r"\s+", " ", texts["docs/source/offline_kagemusha.md"])
    required = (
        "Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C#",
        "typed recursive-spend compact projection verifier facades",
        "raw Norito compact-token and verifier-record archives",
        "reject malformed local inputs before native dispatch",
        "ABI-7 compact projection verifier symbols",
        "native boolean receiver result",
    )
    for needle in required:
        require(
            needle in text,
            f"offline Kagemusha docs missing all-SDK compact projection verifier boundary: {needle}",
            errors,
        )


def check_offline_doc_lineage_key_artifact_sdk_surface(texts, errors):
    text = re.sub(r"\s+", " ", texts["docs/source/offline_kagemusha.md"])
    required = (
        "Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C#",
        "typed lineage key artifact helpers",
        "defensively copy key bytes",
        "non-`halo2/ipa` verifier backends",
        "Java-callable null lineage artifact inputs",
        "same stable field errors",
        "Kotlin intrinsic null checks",
    )
    for needle in required:
        require(
            needle in text,
            f"offline Kagemusha docs missing all-SDK lineage key artifact boundary: {needle}",
            errors,
        )


def check_offline_doc_instruction_transaction_sdk_surface(texts, errors):
    text = re.sub(r"\s+", " ", texts["docs/source/offline_kagemusha.md"])
    required = (
        "Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C#",
        "typed archived-instruction transaction surface",
        "KagemushaTransfer",
        "RedeemKagemushaRecursive",
        "valid Norito archives",
        "preserve their canonical bytes rather than re-framing them",
        "empty, malformed, tampered, or wrong-type instruction archives",
        "KagemushaInstructionTransactionRequest",
        "KagemushaInstructionArchives",
        "buildKagemushaRecursiveRedeemTransaction(...)",
        "TransactionDraft.kagemusha_recursive_redeem(...)",
        "TransactionBuilder.KagemushaRecursiveRedeem(...)",
        "Recursive redeem derivation inside the transaction helper",
        "native recursive redeem request",
        "signs exactly one `RedeemKagemushaRecursive` instruction",
    )
    for needle in required:
        require(
            needle in text,
            f"offline Kagemusha docs missing all-SDK instruction transaction boundary: {needle}",
            errors,
        )


def check_offline_doc_native_output_sdk_surface(texts, errors):
    text = re.sub(r"\s+", " ", texts["docs/source/offline_kagemusha.md"])
    required = (
        "Python, Swift, JavaScript/Node, Kotlin/JVM, Java Android, and C# also fail closed",
        "proof-producing native calls return no archive or a zero-length archive",
        "missing native proof material cannot be coerced into a successful SDK result",
    )
    for needle in required:
        require(
            needle in text,
            f"offline Kagemusha docs missing all-SDK native output boundary: {needle}",
            errors,
        )


def check_offline_doc_native_owned_accumulator_boundary(texts, errors):
    text = re.sub(r"\s+", " ", texts["docs/source/offline_kagemusha.md"])
    required = (
        "Appenders must provide the previous recursive proof to the native append builder",
        "Native append streams the previous recursive proof bytes and per-hop accumulator material into native-owned accumulator digests",
        "`recursive_proof_chain_digest`",
        "lineage/aggregation transcript",
        "fixed-window schedule/shared-manifest/table-base",
        "verifier-witness batch",
        "transition-profile",
        "append-opening-preflight",
        "append-boundary",
        "scalar-projection",
        "previous/resulting accumulator digests",
        "SDKs must not derive, supply, or patch accumulator state themselves",
    )
    for needle in required:
        require(
            needle in text,
            f"offline Kagemusha docs missing native-owned accumulator boundary: {needle}",
            errors,
        )


def check_mobile_halo2_canonical_vk_hash(texts, errors):
    expected = KAGEMUSHA_HALO2_CANONICAL_VK_HASH_V1
    targets = (
        (
            "IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift",
            f'canonicalVKHash = Data(hexString: "{expected}")!',
            "Swift Halo2 canonical VK hash",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteHalo2Prover.java",
            f'hexBytes("{expected}")',
            "Android Java Halo2 canonical VK hash",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteHalo2Prover.java",
            f'hexBytes("{expected}")',
            "Kotlin Halo2 canonical VK hash",
        ),
    )
    for relative, needle, label in targets:
        require_contains(texts, relative, (needle,), label, errors)


def check_cross_sdk_preferred_mode_fallback_policy(texts, errors):
    """Pin production preferred-mode selection until recursive compact is promoted."""

    for relative in (
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/dist/crypto.js",
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/crypto.browser.js",
    ):
        require_contains(
            texts,
            relative,
            (
                'KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1"',
                "preferredKagemushaOfflineSpendModeForCapabilities",
                "void recursiveCompactAvailable;",
                "return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1;",
                "arguments.length >= 2 ? recursiveCompactAvailable : false",
            ),
            f"{relative} preferred Kagemusha mode fallback policy",
            errors,
        )
        require_regex(
            texts,
            relative,
            r"export\s+function\s+preferredKagemushaOfflineSpendModeForCapabilities"
            r"\(\s*recursiveCompactAvailable,\s*recursiveSpendAvailable,\s*\)\s*\{\s*"
            r"void\s+recursiveCompactAvailable;\s*"
            r"if\s*\(\s*recursiveSpendAvailable\s*\)\s*\{\s*"
            r"return\s+KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1;\s*\}\s*"
            r"return\s+KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1;\s*\}",
            f"{relative} preferred Kagemusha mode fallback policy",
            errors,
            flags=re.S,
        )

    require_contains(
        texts,
        "python/iroha_python/src/iroha_python/kagemusha.py",
        (
            'KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1"',
            "preferred_kagemusha_offline_spend_mode_for_capabilities",
            "_ = recursive_compact_available",
            "return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
        ),
        "Python preferred Kagemusha mode fallback policy",
        errors,
    )
    require_regex(
        texts,
        "python/iroha_python/src/iroha_python/kagemusha.py",
        r"def\s+preferred_kagemusha_offline_spend_mode_for_capabilities"
        r"\(\s*recursive_compact_available:\s*bool,\s*recursive_spend_available:\s*bool,\s*\)"
        r"\s*->\s*KagemushaOfflineSpendMode:\s*"
        r"_\s*=\s*recursive_compact_available\s*"
        r"if\s+recursive_spend_available:\s*"
        r"return\s+KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1\s*"
        r"return\s+KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
        "Python preferred Kagemusha mode fallback policy",
        errors,
        flags=re.S,
    )

    require_contains(
        texts,
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
        (
            'case checkedPrefoldV1 = "checked_prefold_v1"',
            "recursiveCompactAvailable: KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable",
            "recursiveCompactAvailable: false",
            "_ = recursiveCompactAvailable",
            "return recursiveSpendAvailable ? .recursiveSpendV1 : .checkedPrefoldV1",
        ),
        "Swift preferred Kagemusha mode fallback policy",
        errors,
    )

    require_contains(
        texts,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
        (
            'CHECKED_PREFOLD_V1("checked_prefold_v1")',
            "recursiveCompactAvailable = KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable()",
            "recursiveCompactAvailable = false",
            "@Suppress(\"UNUSED_PARAMETER\")",
            "// ABI-7 compact mode is not a production default yet.",
            "Mode.CHECKED_PREFOLD_V1",
        ),
        "Kotlin preferred Kagemusha mode fallback policy",
        errors,
    )
    require_regex(
        texts,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
        r"fun\s+preferredMode\(\s*recursiveCompactAvailable:\s*Boolean,\s*"
        r"recursiveSpendAvailable:\s*Boolean,\s*\):\s*Mode\s*\{\s*"
        r"// ABI-7 compact mode is not a production default yet\.\s*"
        r"return\s+if\s*\(\s*recursiveSpendAvailable\s*\)\s*\{\s*"
        r"Mode\.RECURSIVE_SPEND_V1\s*\}\s*else\s*\{\s*Mode\.CHECKED_PREFOLD_V1\s*\}\s*\}",
        "Kotlin preferred Kagemusha mode fallback policy",
        errors,
        flags=re.S,
    )

    require_contains(
        texts,
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
        (
            'CHECKED_PREFOLD_V1("checked_prefold_v1")',
            "KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable(), NATIVE_AVAILABLE",
            "return preferredMode(false, recursiveSpendAvailable);",
            "// ABI-7 compact mode is not a production default yet.",
            "return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;",
        ),
        "Android Java preferred Kagemusha mode fallback policy",
        errors,
    )

    require_contains(
        texts,
        "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
        (
            'CheckedPrefoldV1WireName = "checked_prefold_v1"',
            "return PreferredMode(IsRecursiveCompactPaymentTokenProverAvailable(), IsAvailable());",
            "return PreferredMode(false, recursiveSpendAvailable);",
            "_ = recursiveCompactAvailable;",
            "? KagemushaOfflineSpendMode.RecursiveSpendV1",
            ": KagemushaOfflineSpendMode.CheckedPrefoldV1;",
        ),
        "C# preferred Kagemusha mode fallback policy",
        errors,
    )


def run_checks(texts):
    errors = []
    check_workflow_paths(errors)
    check_workflow_runs_sdk_main_guard(errors)
    check_workflow_runs_sdk_negative_controls(errors)
    check_workflow_runs_native_bridge_tests(errors)
    check_workflow_runs_python_sdk_tests(errors)
    check_workflow_runs_jvm_sdk_tests(errors)
    check_workflow_runs_swift_sdk_parse(errors)
    check_swift_sdk_script_prints_swiftc_version(errors)
    check_workflow_runs_csharp_sdk_tests(errors)
    check_workflow_runs_javascript_sdk_tests(errors)
    check_javascript_sdk_script(errors)
    check_js_parity_meta_test(errors)
    check_c_bridge(texts, errors)
    check_recursive_compact_surface(texts, errors)
    check_recursive_compact_sdk_key_package_arity(texts, errors)
    check_record_backed_javascript_surface(texts, errors)
    check_rust_policy_constants(texts, errors)
    check_node_host(texts, errors)
    check_jvm_sdk_script_pins_jdk21(texts, errors)
    check_javascript(texts, errors)
    check_python(texts, errors)
    check_swift(texts, errors)
    check_java_kotlin(texts, errors)
    check_csharp(texts, errors)
    check_mobile_halo2_canonical_vk_hash(texts, errors)
    check_cross_sdk_preferred_mode_fallback_policy(texts, errors)
    check_sdk_readme_previous_proof_boundary(texts, errors)
    check_sdk_readme_instruction_transaction_surface(texts, errors)
    check_sdk_accumulator_digest_is_native_owned(texts, errors)
    check_sdk_readme_recursive_compact_unavailable_boundary(texts, errors)
    check_offline_doc_recursive_compact_projection_sdk_surface(texts, errors)
    check_offline_doc_lineage_key_artifact_sdk_surface(texts, errors)
    check_offline_doc_instruction_transaction_sdk_surface(texts, errors)
    check_offline_doc_native_output_sdk_surface(texts, errors)
    check_offline_doc_native_owned_accumulator_boundary(texts, errors)
    if errors:
        raise ParityError("\n".join(errors))


texts = read_sources()

if mode == "--negative-control":
    mutated = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated[target] = mutated[target].replace(
        "kagemushaRecursiveSpendTransitionProfileAppend",
        "kagemushaRecursiveSpendTransitionProfileAppendMissing",
    )
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK surface drift was not detected")

if mode == "--negative-control-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "javascript/iroha_js/src/crypto.js"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow path coverage")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected SDK workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow path drift was not detected")

if mode == "--negative-control-native-manifest-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "crates/connect_norito_bridge/Cargo.toml"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native manifest workflow path")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native manifest workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native manifest workflow path drift was not detected")

if mode == "--negative-control-sdk-negative-controls-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-browser-helper",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --synthetic-js-browser-helper-check",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate SDK negative-control workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected SDK negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK negative-control workflow drift was not detected")

if mode == "--negative-control-sdk-negative-controls-comment-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "          ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-browser-helper",
        "          # ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-js-browser-helper",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to comment SDK negative-control workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected commented SDK parity workflow command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: commented SDK parity workflow command drift was not detected")

if mode == "--negative-control-sdk-main-guard-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "        run: ci/check_kagemusha_recursive_spend_sdk_parity.sh",
        "        run: ci/check_kagemusha_recursive_spend_sdk_parity.sh --skip-main-guard",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate SDK main guard workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected SDK main guard workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK main guard workflow drift was not detected")

if mode == "--negative-control-bytecode-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - name: Reject tracked Python bytecode\n"
        f"        run: {PYTHON_BYTECODE_COMMAND}\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate tracked Python bytecode workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected tracked Python bytecode workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: tracked Python bytecode workflow drift was not detected")

if mode == "--negative-control-native-bridge-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_native_bridge_tests:\n",
        "  kagemusha_native_bridge_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native bridge workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow job drift was not detected")

if mode == "--negative-control-native-bridge-runner-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_native_bridge_tests:\n    runs-on: ubuntu-latest",
        "  kagemusha_native_bridge_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow runner")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native bridge workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow runner drift was not detected")

if mode == "--negative-control-native-bridge-cache-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - uses: Swatinem/rust-cache@v2\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge Rust cache step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native bridge Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge Rust cache drift was not detected")

if mode == "--negative-control-native-bridge-test-workflow":
    target = WORKFLOW_PATH
    mutated = read(target)
    mutations = (
        (
            NATIVE_BRIDGE_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_spend_ffi",
            "native recursive spend bridge smoke test",
        ),
        (
            NATIVE_BRIDGE_EMPTY_NESTED_PALLAS_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_spend_ffi_rejects_empty_nested_pallas",
            "native recursive spend empty nested-Pallas bridge test",
        ),
        (
            NATIVE_BRIDGE_LINEAGE_WITNESS_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_spend_lineage_witness_ffi",
            "native lineage-witness bridge invalid-input test",
        ),
        (
            NATIVE_BRIDGE_APPEND_BOUNDARY_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_spend_lineage_append_boundary_ffi",
            "native append-boundary semantic-profile bridge test",
        ),
        (
            JS_HOST_APPEND_BOUNDARY_TEST_COMMAND,
            "cargo test -p iroha_js_host --lib -- --skip kagemusha_recursive_spend_lineage_append_boundary",
            "JS host append-boundary duplicate-output test",
        ),
        (
            NATIVE_BRIDGE_OVERSIZED_LENGTH_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_compact_ffi_rejects_oversized_lengths",
            "native Kagemusha oversized-length FFI test",
        ),
        (
            NATIVE_BRIDGE_UNANCHORED_COMPACT_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_verified_compact_token_ffi_rejects",
            "native unanchored compact-token invalid-input tests",
        ),
        (
            NATIVE_BRIDGE_UNANCHORED_VALID_COMPACT_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_unanchored_compact_token_ffi",
            "native unanchored compact-token valid-bundle rejection test",
        ),
        (
            NATIVE_BRIDGE_RECORD_COMPACT_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_verified_record_compact_token_ffi",
            "native record-backed compact-token adversarial test",
        ),
        (
            NATIVE_BRIDGE_RECORD_RECURSIVE_AGGREGATION_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_verified_record_recursive_aggregation_proof_bundle_ffi",
            "native record-backed recursive aggregation adversarial test",
        ),
        (
            NATIVE_BRIDGE_RECURSIVE_COMPACT_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_compact_ffi",
            "native recursive compact bridge adversarial test",
        ),
        (
            NATIVE_BRIDGE_RECURSIVE_COMPACT_WINDOWED_RECORD_TEST_COMMAND,
            "cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_compact_ffi_rejects_windowed_records",
            "native recursive compact windowed-record bridge test",
        ),
    )
    expected_labels = []
    for old, new, label in mutations:
        updated = mutated.replace(old, new, 1)
        if updated == mutated:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated = updated
        expected_labels.append(label)
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: native bridge test workflow drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected native bridge test workflow drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge test workflow drift was not detected")

if mode == "--negative-control-native-bridge-windowed-record-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    windowed_line = f"          {NATIVE_BRIDGE_RECURSIVE_COMPACT_WINDOWED_RECORD_TEST_COMMAND}\n"
    heavyweight_line = f"          {NATIVE_BRIDGE_RECURSIVE_COMPACT_TEST_COMMAND}\n"
    mutated = original.replace(
        windowed_line + heavyweight_line,
        heavyweight_line + windowed_line,
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native bridge windowed-record test order"
        )
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        message = str(error)
        if "windowed-record bridge test before the heavyweight recursive compact adversarial test" not in message:
            raise SystemExit(
                "negative control failed: native bridge windowed-record order drift was not detected"
            )
        print("negative control rejected native bridge windowed-record order drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native bridge windowed-record order drift was not detected"
    )

if mode == "--negative-control-native-bridge-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_swift_sdk_parse, kagemusha_csharp_sdk_tests, kagemusha_javascript_sdk_tests, kagemusha_jvm_sdk_tests, kagemusha_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native bridge workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow dependency drift was not detected")

if mode == "--negative-control-python-sdk-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_python_sdk_tests:\n",
        "  kagemusha_python_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow job drift was not detected")

if mode == "--negative-control-python-sdk-runner-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_python_sdk_tests:\n    runs-on: ubuntu-latest",
        "  kagemusha_python_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow runner")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow runner drift was not detected")

if mode == "--negative-control-python-sdk-setup-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - uses: actions/setup-python@v5\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup workflow step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup workflow drift was not detected")

if mode == "--negative-control-python-sdk-version-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '          python-version: "3.11"\n',
        '          python-version: "3.10"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version drift was not detected")

if mode == "--negative-control-python-sdk-setup-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    run_line = f"        run: {PYTHON_SDK_TEST_COMMAND}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup order")
    insert = mutated.index("      - uses: actions/setup-python@v5\n")
    mutated = (
        mutated[:insert]
        + "      - name: Kagemusha recursive spend Python SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup ordering drift was not detected")

if mode == "--negative-control-python-sdk-rust-cache-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          cache-on-failure: \"true\"\n",
        "  kagemusha_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK Rust cache")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK Rust cache drift was not detected")

if mode == "--negative-control-python-sdk-timeout-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n",
        "  kagemusha_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 15\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK timeout")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK timeout drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK timeout drift was not detected")

if mode == "--negative-control-python-sdk-version-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('"${VENV_DIR}/bin/python" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version script drift was not detected")

if mode == "--negative-control-python-sdk-override-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN",
        "KAGEMUSHA_RECURSIVE_PYTHON_BIN",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK override variable")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK override drift was not detected")

if mode == "--negative-control-python-sdk-resolver-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("resolve_python_311_bin()", "resolve_python_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK resolver")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK resolver drift was not detected")

if mode == "--negative-control-python-sdk-major-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("3.11) ;;", "3.10) ;;")
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK major matcher")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK major script drift was not detected")

if mode == "--negative-control-python-sdk-venv-rebuild-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('  rm -rf "${VENV_DIR}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK stale venv rebuild")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK stale venv rebuild drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK stale venv rebuild drift was not detected")

if mode == "--negative-control-python-sdk-native-build-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('"${VENV_DIR}/bin/python" -m maturin develop --release\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK native build step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK native build script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK native build script drift was not detected")

if mode == "--negative-control-python-sdk-venv-activation-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('export VIRTUAL_ENV="${VENV_DIR}"\nexport PATH="${VENV_DIR}/bin:${PATH}"\n\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK venv activation")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK venv activation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK venv activation drift was not detected")

if mode == "--negative-control-python-sdk-bytecode-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("export PYTHONDONTWRITEBYTECODE=1\n\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK bytecode guard")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK bytecode script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK bytecode script drift was not detected")

if mode == "--negative-control-python-sdk-test-filter-script":
    target = PYTHON_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("  tests/kagemusha_test.py \\\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK test filter")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK test filter drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK test filter drift was not detected")

if mode == "--negative-control-python-sdk-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "python/iroha_python/src/iroha_python/privacy_catalog.py"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow inventory drift was not detected")

if mode == "--negative-control-python-lineage-frozen-copy":
    mutated = dict(texts)
    target = "python/iroha_python/tests/kagemusha_test.py"
    mutated[target] = mutated[target].replace("FrozenInstanceError", "RuntimeError", 1)
    mutated[target] = mutated[target].replace("proving_key[:] =", "proving_key_copy[:] =", 1)
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Python lineage frozen copy test")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Python lineage frozen copy test drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python lineage frozen copy test drift was not detected")

if mode == "--negative-control-python-sdk-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {PYTHON_SDK_TEST_COMMAND}",
        f"        run: {PYTHON_SDK_TEST_COMMAND} --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK test workflow drift was not detected")

if mode == "--negative-control-python-host-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {PYTHON_HOST_APPEND_BOUNDARY_TEST_COMMAND}",
        f"        run: {PYTHON_HOST_APPEND_BOUNDARY_TEST_COMMAND} --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python host test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python host test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python host test workflow drift was not detected")

if mode == "--negative-control-python-sdk-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_native_bridge_tests, kagemusha_swift_sdk_parse, kagemusha_csharp_sdk_tests, kagemusha_javascript_sdk_tests, kagemusha_jvm_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Python SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow dependency drift was not detected")

if mode == "--negative-control-jvm-sdk-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_jvm_sdk_tests:\n",
        "  kagemusha_jvm_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow job drift was not detected")

if mode == "--negative-control-jvm-sdk-runner-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_jvm_sdk_tests:\n    runs-on: ubuntu-latest",
        "  kagemusha_jvm_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow runner")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow runner drift was not detected")

if mode == "--negative-control-jvm-sdk-java-setup-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - uses: actions/setup-java@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java setup workflow step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Java setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java setup workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-java-distribution-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '          distribution: "temurin"\n',
        '          distribution: "zulu"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java distribution")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Java distribution drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java distribution drift was not detected")

if mode == "--negative-control-jvm-sdk-java-version-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '          java-version: "21"\n',
        '          java-version: "17"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java version")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Java version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java version drift was not detected")

if mode == "--negative-control-jvm-sdk-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {JVM_SDK_TEST_COMMAND}",
        "        run: ci/check_kagemusha_recursive_spend_jvm_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK test workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-test-filter-script":
    target = JVM_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "  --tests org.hyperledger.iroha.sdk.offline.KagemushaInstructionArchivesTest \\\n"
        "  --tests org.hyperledger.iroha.sdk.offline.OfflineNoteTest \\\n"
        "  --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test \\\n"
        "  --tests org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest\n",
        "",
        1,
    )
    mutated = mutated.replace(
        ",org.hyperledger.iroha.android.offline.OfflineNoteV2Test,"
        "org.hyperledger.iroha.android.offline.OfflineNoteTest,"
        "org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest,"
        "org.hyperledger.iroha.android.tx.TransactionBuilderTests",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK test filter")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK test filter drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK test filter drift was not detected")

if mode == "--negative-control-jvm-sdk-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow inventory drift was not detected")

if mode == "--negative-control-jvm-sdk-android-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Android workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Android workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Android workflow inventory drift was not detected")

if mode == "--negative-control-jvm-sdk-jdk21-script":
    target = JVM_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("java -version\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK JDK 21 script evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK JDK 21 script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK JDK 21 script drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-override-script":
    target = JVM_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME",
        "KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME_DISABLED",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java home override variable")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Java home override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java home override drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-reject-script":
    target = JVM_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "JAVA_HOME must point to a JDK 21 home for Kagemusha recursive spend JVM SDK tests.",
        "JAVA_HOME is not checked before fallback.",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK inherited Java home rejection")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK inherited Java home rejection drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK inherited Java home rejection drift was not detected")

if mode == "--negative-control-jvm-recursive-compact-verifier-availability":
    mutated_texts = dict(texts)
    target = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"
    original = read(target)
    mutated = original.replace("check(nativeVerifierAvailable)", "check(nativeAvailable)", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM recursive compact verifier availability")
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected JVM recursive compact verifier availability drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM recursive compact verifier availability drift was not detected")

if mode == "--negative-control-jvm-recursive-compact-shape-classifier":
    mutated_texts = dict(texts)
    targets = (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
    )
    mutations = (
        (
            "public instance column 0 must contain exactly one row; found 2",
            "public instance row-shape errors may be unavailable",
        ),
        (
            "envelope verifier-key hash mismatch",
            "envelope verifier-key hash mismatch may be unavailable",
        ),
    )
    changed = False
    for target in targets:
        mutated = read(target)
        for needle, replacement in mutations:
            updated = mutated.replace(needle, replacement, 1)
            changed = changed or updated != mutated
            mutated = updated
        mutated_texts[target] = mutated
    if not changed:
        raise SystemExit("negative control failed: unable to mutate JVM recursive compact shape classifier coverage")
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected JVM recursive compact shape classifier drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM recursive compact shape classifier drift was not detected")

if mode == "--negative-control-mobile-recursive-spend-native-output-headers":
    mutated_texts = dict(texts)
    targets = (
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
            "Kotlin recursive spend native output Norito guard tests",
        ),
        (
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
            "Android Java recursive spend native output Norito guard tests",
        ),
    )
    missing_targets = []
    expected_labels = []
    for target, label in targets:
        original = read(target)
        mutated = original.replace(
            "invalidFieldBitset[39] = 0x20",
            "invalidFieldBitset[39] = 0x06",
            1,
        )
        if mutated == original:
            missing_targets.append(target)
        mutated_texts[target] = mutated
        expected_labels.append(label)
    if missing_targets:
        raise SystemExit(
            "negative control failed: unable to mutate mobile native output header coverage for "
            + ", ".join(missing_targets)
        )
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: mobile native output header drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected mobile native output header drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: mobile native output header drift was not detected")

if mode == "--negative-control-jvm-sdk-android-harness-script":
    target = JVM_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
        "ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Android harness selector")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK Android harness drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Android harness drift was not detected")

if mode == "--negative-control-jvm-sdk-test-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    command = f"        run: {JVM_SDK_TEST_COMMAND}"
    mutated = original.replace(f"{command}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n",
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n"
        f"{command}\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JVM SDK tests after benchmark")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK test ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK test ordering drift was not detected")

if mode == "--negative-control-jvm-sdk-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_native_bridge_tests, kagemusha_swift_sdk_parse, kagemusha_csharp_sdk_tests, kagemusha_javascript_sdk_tests, kagemusha_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JVM SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow dependency drift was not detected")

if mode == "--negative-control-swift-sdk-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_swift_sdk_parse:\n",
        "  kagemusha_swift_sdk_parse_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow job drift was not detected")

if mode == "--negative-control-swift-sdk-runner-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "    runs-on: macos-latest",
        "    runs-on: ubuntu-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow runner")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow runner drift was not detected")

if mode == "--negative-control-swift-sdk-parse-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {SWIFT_SDK_PARSE_COMMAND}",
        "        run: ci/check_kagemusha_recursive_spend_swift_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK parse workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK parse workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK parse workflow drift was not detected")

if mode == "--negative-control-swift-sdk-parse-surface-script":
    target = SWIFT_SDK_PARSE_COMMAND
    original = read(target)
    mutated = original.replace(
        "  IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift \\\n",
        "",
        1,
    )
    mutated = mutated.replace(
        "  IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift \\\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK parse surface")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK parse surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK parse surface drift was not detected")

if mode == "--negative-control-swift-sdk-privacy-parse-script":
    target = SWIFT_SDK_PARSE_COMMAND
    original = read(target)
    mutated = original.replace(
        "  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \\\n",
        "",
        1,
    )
    mutated = mutated.replace(
        "  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \\\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK privacy parse surface")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK privacy parse drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK privacy parse drift was not detected")

if mode == "--negative-control-swift-sdk-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow inventory drift was not detected")

if mode == "--negative-control-swift-sdk-source-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK source workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK source workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK source workflow inventory drift was not detected")

if mode == "--negative-control-swift-sdk-uc4-skip":
    mutated_texts = dict(texts)
    target = "IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift"
    mutated_texts[target] = mutated_texts[target].replace("throw XCTSkip", "throw NSError", 1)
    if mutated_texts[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Swift UC4 diagnostic skip")
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift UC4 diagnostic skip drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift UC4 diagnostic skip drift was not detected")

if mode == "--negative-control-swift-lineage-data-copy":
    mutated_texts = dict(texts)
    target = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift"
    mutated_texts[target] = mutated_texts[target].replace(
        "var exposedProvingKeyArchive = initArtifacts.lineageProvingKeyArchive",
        "var exposedProvingArchive = initArtifacts.lineageProvingKeyArchive",
        1,
    )
    if mutated_texts[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Swift lineage Data copy test")
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift lineage Data copy test drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift lineage Data copy test drift was not detected")

if mode == "--negative-control-swift-recursive-compact-verifier-bool":
    mutated_texts = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    original = read(target)
    mutated = original.replace(
        "normalizeKagemushaRecursiveCompactVerifierOutput",
        "coerceKagemushaRecursiveCompactVerifierOutput",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift recursive compact verifier bool normalizer")
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift recursive compact verifier bool drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift recursive compact verifier bool drift was not detected")

if mode == "--negative-control-swift-recursive-compact-verifier-availability":
    mutated_texts = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"
    original = read(target)
    mutated = original.replace(
        "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
        "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenProverAvailable",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift recursive compact verifier availability")
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift recursive compact verifier availability drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift recursive compact verifier availability drift was not detected")

if mode == "--negative-control-swift-kagemusha-native-output-cap":
    mutated_texts = dict(texts)
    bridge = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    test_path = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift"
    mutated_bridge = texts[bridge].replace(
        "length <= CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)",
        "true || length <= CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)",
        1,
    )
    mutated_test = texts[test_path].replace(
        "testNativeBridgeRejectsOversizedKagemushaOutputBeforeCopying",
        "testNativeBridgeAllowsOversizedKagemushaOutputBeforeCopying",
        1,
    )
    if mutated_bridge == texts[bridge] or mutated_test == texts[test_path]:
        raise SystemExit("negative control failed: unable to mutate Swift Kagemusha native output cap coverage")
    mutated_texts[bridge] = mutated_bridge
    mutated_texts[test_path] = mutated_test
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift Kagemusha native output cap drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift Kagemusha native output cap drift was not detected")

if mode == "--negative-control-swift-native-output-headers":
    mutated_texts = dict(texts)
    targets = (
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
            "Swift recursive spend native output header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
            "Swift compact-token native output header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
            "Swift recursive aggregation native output header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
            "Swift recursive compact native output header guard tests",
        ),
    )
    missing_targets = []
    expected_labels = []
    for target, label in targets:
        mutated = texts[target].replace(
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "invalidFieldBitset[39] = NoritoHeader.packedStruct | NoritoHeader.compactLen",
            1,
        )
        if mutated == texts[target]:
            missing_targets.append(target)
        mutated_texts[target] = mutated
        expected_labels.append(label)
    if missing_targets:
        raise SystemExit(
            "negative control failed: unable to mutate Swift native output header coverage for "
            + ", ".join(missing_targets)
        )
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Swift native output header drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Swift native output header drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift native output header drift was not detected")

if mode == "--negative-control-swift-native-input-headers":
    mutated_texts = dict(texts)
    targets = (
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
            "Swift recursive spend input header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
            "Swift compact-token input header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
            "Swift recursive aggregation input header guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
            "Swift recursive compact input header guard tests",
        ),
    )
    missing_targets = []
    expected_labels = []
    for target, label in targets:
        mutated = texts[target].replace(
            "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
            "invalidFieldBitset[39] = NoritoHeader.packedStruct | NoritoHeader.compactLen",
            1,
        )
        if mutated == texts[target]:
            missing_targets.append(target)
        mutated_texts[target] = mutated
        expected_labels.append(label)
    if missing_targets:
        raise SystemExit(
            "negative control failed: unable to mutate Swift native input header coverage for "
            + ", ".join(missing_targets)
        )
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Swift native input header drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Swift native input header drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift native input header drift was not detected")

if mode == "--negative-control-swift-kagemusha-instruction-transaction-builder":
    mutated_texts = dict(texts)
    source_target = "IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift"
    test_target = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift"
    mutated_source = texts[source_target].replace(
        "func buildKagemushaRecursiveRedeem(",
        "func buildKagemushaRecursiveRedeemUnchecked(",
        2,
    )
    mutated_test = texts[test_target].replace(
        "testBuildKagemushaRecursiveRedeemTransactionDerivesInstructionBeforeSigning",
        "testBuildKagemushaRecursiveRedeemTransactionSkipsNativeDerivationBeforeSigning",
        1,
    )
    if (
        mutated_source == texts[source_target]
        or "func buildKagemushaRecursiveRedeem(" in mutated_source
        or mutated_test == texts[test_target]
    ):
        raise SystemExit(
            "negative control failed: unable to mutate Swift Kagemusha instruction transaction builder coverage"
        )
    mutated_texts[source_target] = mutated_source
    mutated_texts[test_target] = mutated_test
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected Swift Kagemusha instruction transaction builder drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Swift Kagemusha instruction transaction builder drift was not detected"
    )

if mode == "--negative-control-swift-sdk-version-script":
    target = SWIFT_SDK_PARSE_COMMAND
    original = read(target)
    mutated = original.replace('"${SWIFTC_BIN}" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK version evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK version script drift was not detected")

if mode == "--negative-control-swift-sdk-override-script":
    target = SWIFT_SDK_PARSE_COMMAND
    original = read(target)
    mutated = original.replace(
        "KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN",
        "KAGEMUSHA_RECURSIVE_SWIFTC_BIN",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK compiler override variable")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK compiler override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK compiler override drift was not detected")

if mode == "--negative-control-swift-sdk-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_native_bridge_tests, kagemusha_csharp_sdk_tests, kagemusha_javascript_sdk_tests, kagemusha_jvm_sdk_tests, kagemusha_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected Swift SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow dependency drift was not detected")

if mode == "--negative-control-csharp-sdk-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_csharp_sdk_tests:\n",
        "  kagemusha_csharp_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow job drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - uses: actions/setup-dotnet@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup workflow step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "          dotnet-version: 8.0.x\n",
        "          dotnet-version: 7.0.x\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK dotnet version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    run_line = f"        run: {CSHARP_SDK_TEST_COMMAND}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup order")
    insert = mutated.index("      - uses: actions/setup-dotnet@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: Kagemusha recursive spend C# SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup ordering drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('printf \'%s\\n\' "${DOTNET_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK dotnet version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version script drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-info-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("printf 'dotnet --info:\\n'\n", "", 1)
    mutated = mutated.replace('"${DOTNET_BIN}" --info\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet info evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK dotnet info script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet info script drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-override-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN",
        "KAGEMUSHA_RECURSIVE_DOTNET_BIN",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet override variable")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK dotnet override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet override drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-major-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("8.0.*) ;;", "7.0.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet major matcher")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK dotnet major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet major script drift was not detected")

if mode == "--negative-control-csharp-sdk-native-bridge-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        'CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" cargo build -p connect_norito_bridge\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK native bridge build")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK native bridge script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK native bridge script drift was not detected")

if mode == "--negative-control-csharp-sdk-native-library-evidence-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        'BRIDGE_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}/${BRIDGE_LIBRARY_NAME}"\n',
        "",
        1,
    )
    mutated = mutated.replace(
        'printf \'connect_norito_bridge native bridge: %s\\n\' "${BRIDGE_LIBRARY_PATH}"\n',
        "",
        1,
    )
    mutated = mutated.replace(
        'printf \'connect_norito_bridge native bridge sha256: %s\\n\' "${BRIDGE_LIBRARY_SHA256}"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK native library evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK native library evidence drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK native library evidence drift was not detected")

if mode == "--negative-control-csharp-sdk-test-filter-script":
    target = CSHARP_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "|FullyQualifiedName~PrivacyNativeTests|FullyQualifiedName~TransactionBuilderTests",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK test filter")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK test filter drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK test filter drift was not detected")

if mode == "--negative-control-csharp-sdk-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow inventory drift was not detected")

if mode == "--negative-control-csharp-archive-copy":
    mutated_texts = dict(texts)
    test_target = "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs"
    source_target = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    original_test = read(test_target)
    mutated_test = original_test.replace(
        "RecursiveSpendArchiveWrappersDefensivelyCopyNoritoBytes",
        "RecursiveSpendArchiveWrappersExposeNoritoBytes",
        1,
    ).replace(
        "invalidFieldBitset[39] = 0x20",
        "invalidFieldBitset[39] = 0x06",
    ).replace(
        "AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive)",
        "AssertRejectsMalformedEverywhere(validArchive, validArchive)",
        1,
    ).replace(
        "AssertRejectsMalformedBridgeOutput(invalidFieldBitset)",
        "AssertRejectsMalformedBridgeOutput(KagemushaNoritoFrameWithPayload(0x4b))",
        1,
    )
    if (
        mutated_test == original_test
        or "invalidFieldBitset[39] = 0x20" in mutated_test
        or "AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive)" in mutated_test
        or "AssertRejectsMalformedBridgeOutput(invalidFieldBitset)" in mutated_test
    ):
        raise SystemExit("negative control failed: unable to mutate C# archive copy test")
    mutated_texts[test_target] = mutated_test
    original_source = read(source_target)
    mutated_source = original_source.replace(
        "if (archive.Length > NativeArchiveMaxBytes)",
        "if (bytes.Length > NativeArchiveMaxBytes)",
        1,
    )
    if mutated_source == original_source:
        raise SystemExit("negative control failed: unable to mutate C# archive max pre-copy guard")
    mutated_texts[source_target] = mutated_source
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        for label in (
            "C# recursive spend archive wrapper copy tests",
            "C# recursive spend input Norito guard",
            "C# recursive spend input Norito guard tests",
            "C# recursive compact verifier tests",
        ):
            if label not in message:
                raise SystemExit(
                    "negative control failed: C# archive drift was not detected for "
                    + label
                )
        print("negative control rejected C# archive wrapper copy drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# archive wrapper copy drift was not detected")

if mode == "--negative-control-csharp-recursive-compact-verifier-unavailable":
    mutated_texts = dict(texts)
    target = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    original = read(target)
    old = """    internal static bool NormalizeRecursiveCompactVerifierOutput(string symbol, int code, byte valid)
    {
        if (code != 0)
        {
            if (code == RecursiveCompactUnavailableBridgeErrorCode)
            {
                throw new InvalidOperationException(
                    $"{symbol} is unavailable until ABI-7 recursive compact proof composition is enabled; bridge error code {code}.");
            }
            throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
        }"""
    new = """    internal static bool NormalizeRecursiveCompactVerifierOutput(string symbol, int code, byte valid)
    {
        if (code != 0)
        {
            throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
        }"""
    mutated = original.replace(
        old,
        new,
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# recursive compact verifier unavailable mapping")
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected C# recursive compact verifier unavailable drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# recursive compact verifier unavailable drift was not detected")

if mode == "--negative-control-csharp-sdk-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {CSHARP_SDK_TEST_COMMAND}",
        "        run: ci/check_kagemusha_recursive_spend_csharp_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK test workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_native_bridge_tests, kagemusha_swift_sdk_parse, kagemusha_javascript_sdk_tests, kagemusha_jvm_sdk_tests, kagemusha_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected C# SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow dependency drift was not detected")

if mode == "--negative-control-js-sdk-job-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_javascript_sdk_tests:\n",
        "  kagemusha_javascript_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow job")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow job drift was not detected")

if mode == "--negative-control-js-sdk-runner-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "  kagemusha_javascript_sdk_tests:\n    runs-on: ubuntu-latest",
        "  kagemusha_javascript_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow runner")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow runner drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "      - uses: actions/setup-node@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node setup workflow step")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup workflow drift was not detected")

if mode == "--negative-control-js-sdk-node-version-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '          node-version: "20"\n',
        '          node-version: "18"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version drift was not detected")

if mode == "--negative-control-js-sdk-node-version-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace('printf \'%s\\n\' "${NODE_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version evidence")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version script drift was not detected")

if mode == "--negative-control-js-sdk-node-override-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN",
        "KAGEMUSHA_RECURSIVE_SPEND_JS_NODE_BIN",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node override variable")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node override drift was not detected")

if mode == "--negative-control-js-sdk-node-resolver-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("resolve_node_20_bin()", "resolve_node_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node resolver")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node resolver drift was not detected")

if mode == "--negative-control-js-sdk-node-major-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("v20.*) ;;", "v18.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node major matcher")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node major script drift was not detected")

if mode == "--negative-control-js-sdk-node-cache-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        "          cache-dependency-path: javascript/iroha_js/package-lock.json\n",
        "          cache-dependency-path: javascript/iroha_js/package.json\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK cache path")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK cache path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK cache path drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    install_block = (
        "      - name: Install JavaScript SDK dependencies\n"
        f"        run: {JS_SDK_INSTALL_COMMAND}\n"
    )
    mutated = original.replace(install_block, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install before Node setup")
    insert = mutated.index("      - uses: actions/setup-node@v4\n")
    mutated = mutated[:insert] + install_block + mutated[insert:]
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK Node setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup ordering drift was not detected")

if mode == "--negative-control-js-sdk-install-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f"        run: {JS_SDK_INSTALL_COMMAND}",
        "        run: npm ci --prefix javascript/iroha_js --ignore-scripts",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK install workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK install workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install workflow drift was not detected")

if mode == "--negative-control-js-sdk-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        JS_SDK_TEST_COMMAND,
        "ci/check_kagemusha_recursive_spend_js_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test workflow drift was not detected")

if mode == "--negative-control-js-sdk-transaction-builder-filter-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace("|buildKagemusha", "", 1)
    mutated = mutated.replace(" \\\n  test/transactionBuilder.test.js", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK transaction-builder filter")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK transaction-builder filter drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK transaction-builder filter drift was not detected")

if mode == "--negative-control-js-sdk-privacy-native-filter-script":
    target = JS_SDK_TEST_COMMAND
    original = read(target)
    mutated = original.replace(
        "|privacy native availability probes build and verify with Norito request archives|privacy native wrappers require binary Norito request archives",
        "",
        1,
    )
    mutated = mutated.replace(" \\\n  test/privacyNative.test.js", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK privacy native filter")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK privacy native filter drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK privacy native filter drift was not detected")

if mode == "--negative-control-js-sdk-workflow-inventory":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        '      - "javascript/iroha_js/test/privacyNative.test.js"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow inventory")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK workflow inventory drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow inventory drift was not detected")

if mode == "--negative-control-sdk-privacy-workflow-inventory-matrix":
    target = WORKFLOW_PATH
    original = read(target)
    rejected = []
    for relative in SDK_PRIVACY_WORKFLOW_INVENTORY_PATHS:
        workflow_line = f'      - "{relative}"\n'
        if workflow_line not in original:
            raise SystemExit(
                "negative control failed: SDK privacy workflow inventory path is missing before mutation: "
                + relative
            )
        text_overrides[target] = original.replace(workflow_line, "", 1)
        try:
            run_checks(texts)
        except ParityError as error:
            message = str(error)
            if relative not in message:
                raise SystemExit(
                    "negative control failed: SDK privacy workflow inventory drift for "
                    + relative
                    + " was rejected for the wrong reason: "
                    + message.splitlines()[0]
                )
            rejected.append(relative)
        else:
            raise SystemExit(
                "negative control failed: SDK privacy workflow inventory drift was not detected for "
                + relative
            )
        finally:
            text_overrides.pop(target, None)
    print("negative control rejected SDK privacy workflow inventory matrix drift")
    print(f"checked {len(rejected)} SDK privacy workflow paths")
    raise SystemExit(0)

if mode == "--negative-control-js-sdk-install-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    install_line = f"        run: {JS_SDK_INSTALL_COMMAND}"
    test_line = f"        run: {JS_SDK_TEST_COMMAND}"
    mutated = original.replace(f"{install_line}\n", "", 1)
    mutated = mutated.replace(
        f"{test_line}\n",
        f"{test_line}\n{install_line}\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install after tests")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK install ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install ordering drift was not detected")

if mode == "--negative-control-js-sdk-test-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    test_line = f"        run: {JS_SDK_TEST_COMMAND}"
    mutated = original.replace(f"{test_line}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n",
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n"
        f"{test_line}\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK tests after benchmark")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK test ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test ordering drift was not detected")

if mode == "--negative-control-js-sdk-needs-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        MAIN_JOB_NEEDS_LINE,
        "    needs: [kagemusha_native_bridge_tests, kagemusha_swift_sdk_parse, kagemusha_csharp_sdk_tests, kagemusha_jvm_sdk_tests, kagemusha_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow dependency")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected JavaScript SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow dependency drift was not detected")

if mode == "--negative-control-sdk-parity-meta-test-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    mutated = original.replace(
        f'      - "{JS_PARITY_TEST_PATH}"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate SDK parity meta-test workflow path")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected SDK parity meta-test workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK parity meta-test workflow path drift was not detected")

if mode == "--negative-control-sdk-negative-controls-order-workflow":
    target = WORKFLOW_PATH
    original = read(target)
    command = (
        "          ci/check_kagemusha_recursive_spend_sdk_parity.sh "
        "--negative-control-js-browser-helper"
    )
    mutated = original.replace(f"{command}\n", "", 1)
    mutated = mutated.replace(
        f"        run: {SDK_PARITY_MAIN_COMMAND}",
        f"        run: {SDK_PARITY_MAIN_COMMAND}\n{command}",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move SDK negative-control command after main guard")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected SDK negative-control ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK negative-control ordering drift was not detected")

if mode == "--negative-control-js-browser-helper":
    mutated = dict(texts)
    target = "javascript/iroha_js/dist/crypto.browser.js"
    mutated[target] = mutated[target].replace(
        "export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = true;",
        "export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = false;",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate browser helper")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected JavaScript browser helper drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser helper drift was not detected")

if mode == "--negative-control-js-lineage-key-artifact-copy":
    mutated = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated[target] = mutated[target].replace(
        "get lineageVerifierKey()",
        "lineageVerifierKey",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate JS lineage key artifact copy guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected JS lineage key artifact copy drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS lineage key artifact copy drift was not detected")

if mode == "--negative-control-js-lineage-key-package-binding":
    mutated = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated[target] = mutated[target].replace(
        "archivePayload.includes(verifierKeyCommitment)",
        "archivePayload.includes(Buffer.alloc(32))",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate JS lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected JS lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS lineage key package binding drift was not detected")

if mode == "--negative-control-js-kagemusha-instruction-transaction-builder":
    mutated = dict(texts)
    source_target = "javascript/iroha_js/src/transaction.js"
    test_target = "javascript/iroha_js/test/transactionBuilder.test.js"
    mutated_source = texts[source_target].replace(
        "export function buildKagemushaRecursiveRedeemTransaction",
        "export function buildKagemushaRecursiveRedeemUncheckedTransaction",
        1,
    )
    mutated_test = texts[test_target].replace(
        "buildKagemushaRecursiveRedeemTransaction derives instruction before signing",
        "buildKagemushaRecursiveRedeemTransaction skips instruction derivation before signing",
        1,
    )
    if mutated_source == texts[source_target] or mutated_test == texts[test_target]:
        raise SystemExit(
            "negative control failed: unable to mutate JS Kagemusha instruction transaction builder coverage"
        )
    mutated[source_target] = mutated_source
    mutated[test_target] = mutated_test
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected JS Kagemusha instruction transaction builder drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JS Kagemusha instruction transaction builder drift was not detected"
    )

if mode == "--negative-control-js-python-native-output-headers":
    mutated = dict(texts)
    targets = (
        (
            "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
            "invalidFieldBitset[39] = 0x20;\n  assertRejectsMalformedNativeRedeemOutput(invalidFieldBitset);",
            "invalidFieldBitset[39] = 0x06;\n  assertRejectsMalformedNativeRedeemOutput(invalidFieldBitset);",
            "JavaScript recursive spend native output header guard tests",
        ),
        (
            "python/iroha_python/tests/kagemusha_test.py",
            "invalid_field_bitset[39] = 0x20\n    assert_rejects_malformed_native_outputs(bytes(invalid_field_bitset))",
            "invalid_field_bitset[39] = 0x06\n    assert_rejects_malformed_native_outputs(bytes(invalid_field_bitset))",
            "Python recursive spend native output header guard tests",
        ),
    )
    expected_labels = []
    missing_targets = []
    for target, needle, replacement, label in targets:
        updated = texts[target].replace(needle, replacement, 1)
        if updated == texts[target]:
            missing_targets.append(target)
        mutated[target] = updated
        expected_labels.append(label)
    if missing_targets:
        raise SystemExit(
            "negative control failed: unable to mutate JS/Python native output header coverage for "
            + ", ".join(missing_targets)
        )
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: JS/Python native output header drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected JS/Python native output header drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS/Python native output header drift was not detected")

if mode == "--negative-control-python-kagemusha-instruction-transaction-builder":
    mutated = dict(texts)
    source_target = "python/iroha_python/src/iroha_python/kagemusha.py"
    test_target = "python/iroha_python/tests/kagemusha_test.py"
    mutated_source = texts[source_target].replace(
        "def build_kagemusha_recursive_redeem_transaction(",
        "def build_kagemusha_recursive_redeem_transaction_unchecked(",
        1,
    )
    mutated_test = texts[test_target].replace(
        "bad_request_flags[39] = 0x20",
        "bad_request_flags[39] = 0x06",
        1,
    )
    if mutated_source == texts[source_target] or mutated_test == texts[test_target]:
        raise SystemExit(
            "negative control failed: unable to mutate Python Kagemusha instruction transaction builder coverage"
        )
    mutated[source_target] = mutated_source
    mutated[test_target] = mutated_test
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Python Kagemusha instruction transaction builder drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python Kagemusha instruction transaction builder drift was not detected"
    )

if mode == "--negative-control-csharp-kagemusha-instruction-transaction-builder":
    mutated = dict(texts)
    source_target = "csharp/src/Hyperledger.Iroha.Sdk/Transactions/KagemushaInstructionArchiveInstruction.cs"
    test_target = "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs"
    mutated_source = texts[source_target].replace(
        "EncodeFramedPayload",
        "EncodeUncheckedPayload",
        1,
    )
    mutated_test = texts[test_target].replace(
        "invalidFieldBitset[39] = 0x20",
        "invalidFieldBitset[39] = 0x06",
        1,
    )
    if mutated_source == texts[source_target] or mutated_test == texts[test_target]:
        raise SystemExit(
            "negative control failed: unable to mutate C# Kagemusha instruction transaction builder coverage"
        )
    mutated[source_target] = mutated_source
    mutated[test_target] = mutated_test
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected C# Kagemusha instruction transaction builder drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: C# Kagemusha instruction transaction builder drift was not detected"
    )

if mode == "--negative-control-python-lineage-key-package-binding":
    mutated = dict(texts)
    target = "python/iroha_python/src/iroha_python/kagemusha.py"
    mutated[target] = mutated[target].replace(
        "archive_payload.find(verifier_key_commitment) < 0",
        "archive_payload.find(bytes(32)) < 0",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Python lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Python lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python lineage key package binding drift was not detected")

if mode == "--negative-control-csharp-lineage-key-package-binding":
    mutated = dict(texts)
    target = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    mutated[target] = mutated[target].replace(
        "archivePayload.AsSpan().IndexOf(verifierKeyCommitment) < 0",
        "archivePayload.AsSpan().IndexOf(new byte[32]) < 0",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate C# lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected C# lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# lineage key package binding drift was not detected")

if mode == "--negative-control-swift-lineage-key-package-binding":
    mutated = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"
    mutated[target] = mutated[target].replace(
        "archivePayload.range(of: verifierKeyCommitment) != nil",
        "archivePayload.range(of: Data(repeating: 0, count: 32)) != nil",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Swift lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Swift lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift lineage key package binding drift was not detected")

if mode == "--negative-control-csharp-lineage-witness-availability-probe":
    mutated = dict(texts)
    target = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    mutated[target] = mutated[target].replace(
        "Probe((NativeArchivePairCall)NativeLineageWitnessFromInitResult)",
        "Probe((NativeArchivePairCall)NativeAppend)",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate C# lineage witness availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected C# lineage witness availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# lineage witness availability probe drift was not detected")

if mode == "--negative-control-csharp-lineage-witness-append-availability-probe":
    mutated = dict(texts)
    target = "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"
    mutated[target] = mutated[target].replace(
        "Probe((NativeArchiveTripleCall)NativeLineageWitnessAppendResult)",
        "Probe((NativeArchiveTripleCall)NativeAppend)",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate C# lineage witness append availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected C# lineage witness append availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# lineage witness append availability probe drift was not detected")

if mode == "--negative-control-swift-lineage-witness-availability-probe":
    mutated = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    mutated[target] = mutated[target].replace(
        "probeKagemushaLineageWitnessFromInitResultFunction(\n                kagemushaRecursiveSpendLineageWitnessFromInitResultFn",
        "probeKagemushaLineageWitnessFromInitResultFunction(\n                kagemushaRecursiveSpendInitFn",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Swift lineage witness availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Swift lineage witness availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift lineage witness availability probe drift was not detected")

if mode == "--negative-control-swift-lineage-witness-append-availability-probe":
    mutated = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    mutated[target] = mutated[target].replace(
        "probeKagemushaLineageWitnessAppendResultFunction(\n                kagemushaRecursiveSpendLineageWitnessAppendResultFn",
        "probeKagemushaLineageWitnessAppendResultFunction(\n                kagemushaRecursiveSpendAppendFn",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Swift lineage witness append availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Swift lineage witness append availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift lineage witness append availability probe drift was not detected")

if mode == "--negative-control-jvm-lineage-key-package-binding":
    mutated = dict(texts)
    target = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
    mutated[target] = mutated[target].replace(
        "archivePayload.indexOfSlice(verifierKeyCommitment) >= 0",
        "archivePayload.indexOfSlice(ByteArray(32)) >= 0",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Kotlin/JVM lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Kotlin/JVM lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Kotlin/JVM lineage key package binding drift was not detected")

if mode == "--negative-control-android-lineage-key-package-binding":
    mutated = dict(texts)
    target = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
    mutated[target] = mutated[target].replace(
        "indexOfSlice(archivePayload, verifierKeyCommitment) < 0",
        "indexOfSlice(archivePayload, new byte[32]) < 0",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Android Java lineage key package binding guard")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Android Java lineage key package binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Android Java lineage key package binding drift was not detected")

if mode == "--negative-control-jvm-lineage-witness-availability-probe":
    mutated = dict(texts)
    target = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
    mutated[target] = mutated[target].replace(
        "nativeLineageWitnessFromInitResult(probe, probe)",
        "nativeLineageWitnessFromInitResult(ByteArray(0), ByteArray(0))",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Kotlin/JVM lineage witness availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Kotlin/JVM lineage witness availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Kotlin/JVM lineage witness availability probe drift was not detected")

if mode == "--negative-control-jvm-lineage-witness-append-availability-probe":
    mutated = dict(texts)
    target = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
    mutated[target] = mutated[target].replace(
        "nativeLineageWitnessAppendResult(probe, probe, probe)",
        "nativeLineageWitnessAppendResult(ByteArray(0), ByteArray(0), ByteArray(0))",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Kotlin/JVM lineage witness append availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Kotlin/JVM lineage witness append availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Kotlin/JVM lineage witness append availability probe drift was not detected")

if mode == "--negative-control-android-lineage-witness-availability-probe":
    mutated = dict(texts)
    target = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
    mutated[target] = mutated[target].replace(
        "nativeLineageWitnessFromInitResult(probe, probe)",
        "nativeLineageWitnessFromInitResult(new byte[0], new byte[0])",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Android Java lineage witness availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Android Java lineage witness availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Android Java lineage witness availability probe drift was not detected")

if mode == "--negative-control-android-lineage-witness-append-availability-probe":
    mutated = dict(texts)
    target = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
    mutated[target] = mutated[target].replace(
        "nativeLineageWitnessAppendResult(probe, probe, probe)",
        "nativeLineageWitnessAppendResult(new byte[0], new byte[0], new byte[0])",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Android Java lineage witness append availability probe")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Android Java lineage witness append availability probe drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Android Java lineage witness append availability probe drift was not detected")

if mode == "--negative-control-js-lineage-readonly-declarations":
    mutated = dict(texts)
    target = "javascript/iroha_js/index.d.ts"
    mutated[target] = mutated[target].replace(
        "readonly lineageVerifierKey: Buffer;",
        "lineageVerifierKey: Buffer;",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate JS lineage key artifact readonly declarations"
        )
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected JS lineage key artifact readonly declaration drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JS lineage key artifact readonly declaration drift was not detected"
    )

if mode == "--negative-control-sdk-archive-input-copy":
    mutated = dict(texts)
    mutations = (
        (
            "javascript/iroha_js/src/crypto.js",
            "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
            "const request = toOwnedBuffer(requestArchive, archiveName)",
            "javascript/iroha_js/src/crypto.js native output Norito guard",
        ),
        (
            "javascript/iroha_js/dist/crypto.js",
            "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
            "const request = toOwnedBuffer(requestArchive, archiveName)",
            "javascript/iroha_js/dist/crypto.js native output Norito guard",
        ),
        (
            "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
            "Kagemusha recursive spend helpers reject oversized request archives before native calls",
            "Kagemusha recursive spend helpers allow oversized request archives before native calls",
            "JavaScript native output Norito guard tests",
        ),
        (
            "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
            "Kagemusha recursive spend lineage helpers pass owned archive copies to native",
            "Kagemusha recursive spend lineage helpers pass caller archives to native",
            "JavaScript native output Norito guard tests",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "view = memoryview(archive)",
            "data = bytes(archive)",
            "Python native output Norito guard",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
            "False and view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
            "Python native output Norito guard",
        ),
        (
            "python/iroha_python/tests/kagemusha_test.py",
            "test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native",
            "test_recursive_kagemusha_helpers_allow_oversized_inputs_before_copy_and_native",
            "Python native output Norito guard tests",
        ),
        (
            "python/iroha_python/tests/kagemusha_test.py",
            "test_recursive_kagemusha_lineage_helpers_copy_mutable_archives_before_native",
            "test_recursive_kagemusha_lineage_helpers_forward_mutable_archives_to_native",
            "Python native output Norito guard tests",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "Kagemusha recursive spend input archive must not exceed",
            "Kagemusha recursive spend input archive may exceed",
            "Swift recursive spend input/output Norito guard",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift",
            "Kagemusha verified fold record bundle archive must not exceed",
            "Kagemusha verified fold record bundle archive may exceed",
            "Swift compact-token input/output Norito guard",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift",
            "Kagemusha Pallas open-envelope archive must not exceed",
            "Kagemusha Pallas open-envelope archive may exceed",
            "Swift recursive aggregation input/output Norito guard",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
            "Kagemusha Pallas open-envelope archive must not exceed",
            "Kagemusha Pallas open-envelope archive may exceed",
            "Swift recursive compact wrapper",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
            "testRejectsOversizedInputArchivesBeforeBridgeCall",
            "testAllowsOversizedInputArchivesBeforeBridgeCall",
            "Swift recursive spend input/output Norito guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
            "testRejectsOversizedRecordBundleArchiveBeforeBridgeCall",
            "testAllowsOversizedRecordBundleArchiveBeforeBridgeCall",
            "Swift compact-token input/output Norito guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
            "testRejectsOversizedInputArchivesBeforeBridgeCall",
            "testAllowsOversizedInputArchivesBeforeBridgeCall",
            "Swift recursive aggregation input/output Norito guard tests",
        ),
        (
            "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
            "testVerifyRejectsOversizedCompactTokenArchiveBeforeBridgeCall",
            "testVerifyAllowsOversizedCompactTokenArchiveBeforeBridgeCall",
            "Swift recursive compact verifier tests",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "return Arrays.copyOf(archive, archive.length)",
            "return archive",
            "Android Java recursive spend archive input copy",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
            "return Arrays.copyOf(archive, archive.length)",
            "return archive",
            "Android Java compact-token archive input copy",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
            'archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes"',
            'archiveName + " must be a valid Norito archive"',
            "Android Java compact-token archive max input guard",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            "final byte[] compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "final byte[] compactToken = compactTokenArchive",
            "Android Java recursive compact archive input copy",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            '+ " must not exceed "',
            '+ " must be a valid Norito archive"',
            "Android Java recursive compact archive max input guard",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            'archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes"',
            'archiveName + " must be a valid Norito archive"',
            "Android Java recursive spend archive max input guard",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "val archive = requireNativeInput(archiveInput, archiveName)",
            "val archive = archiveInput!!",
            "Kotlin recursive spend archive input copy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            '"$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"',
            '"$archiveName must be a valid Norito archive"',
            "Kotlin recursive spend archive max input guard",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
            "val archive = requireNativeInput(archiveInput, archiveName)",
            "val archive = archiveInput!!",
            "Kotlin compact-token archive input copy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
            '"$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"',
            '"$archiveName must be a valid Norito archive"',
            "Kotlin compact-token archive max input guard",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "fun initSpend(requestArchive: ByteArray?)",
            "fun initSpend(requestArchive: ByteArray)",
            "Kotlin recursive spend archive input copy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
            "proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: ByteArray?)",
            "proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: ByteArray)",
            "Kotlin compact-token archive input copy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            "val compactToken = ownedNativeInput(compactTokenArchive, \"compactTokenArchive\")",
            "val compactToken = compactTokenArchive",
            "Kotlin recursive compact archive input copy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            '"$archiveName must not exceed ${KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"',
            '"$archiveName must be a valid Norito archive"',
            "Kotlin recursive compact archive max input guard",
        ),
        (
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
            "ownedArchive != archive",
            "ownedArchive == archive",
            "Android Java recursive spend archive input copy tests",
        ),
        (
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
            "compactTokenArchive must not exceed",
            "compactTokenArchive may exceed",
            "Android Java recursive compact prover input Norito guard tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
            "copiesNativeInputArchivesBeforeDispatch",
            "passesNativeInputArchivesThroughBeforeDispatch",
            "Kotlin recursive spend archive input copy tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
            "compactTokenArchive must not exceed",
            "compactTokenArchive may exceed",
            "Kotlin recursive compact prover input Norito guard tests",
        ),
        (
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java",
            "recordBundleArchive must not exceed",
            "recordBundleArchive may exceed",
            "Android Java compact-token input Norito guard tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
            "recordBundleArchive must not exceed",
            "recordBundleArchive may exceed",
            "Kotlin compact-token input Norito guard tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
            "nativeArchiveEntrypointsRejectJavaNullsWithStableFieldMarkers",
            "nativeArchiveEntrypointsAllowJavaNullsThroughGeneratedChecks",
            "Kotlin Java-callable native archive null negative tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
            "kagemushaRecordBackedNativeProversRejectJavaNullsWithStableFieldMarkers",
            "kagemushaRecordBackedNativeProversAllowJavaNullsThroughGeneratedChecks",
            "Kotlin record-backed native archive null negative tests",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {target}")
        mutated[target] = updated
        if label not in expected_labels:
            expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK archive input copy drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK archive input copy drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK archive input copy drift was not detected")

if mode == "--negative-control-sdk-lineage-proving-key-copy":
    mutated = dict(texts)
    mutations = (
        (
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
            "exposedProvingKeyArchive",
            "exposedProvingArchive",
            "Android Java portable lineage key artifact tests",
        ),
        (
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
            "exposedProvingKeyArchive",
            "exposedProvingArchive",
            "Kotlin portable lineage key artifact tests",
        ),
        (
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
            "returnedProvingKeyArchive",
            "returnedProvingArchive",
            "C# portable lineage key artifact tests",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new)
        if updated == mutated[target]:
            raise SystemExit(
                f"negative control failed: unable to mutate {label} proving key copy guard"
            )
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK lineage proving key copy drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK lineage proving key artifact copy drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK lineage proving key artifact copy drift was not detected")

if mode == "--negative-control-sdk-helper-surface":
    mutated = dict(texts)
    target = "javascript/iroha_js/src/index.js"
    mutated[target] = mutated[target].replace(
        "  canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId,\n",
        "",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK helper surface")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK public helper surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK public helper surface drift was not detected")

if mode == "--negative-control-mobile-halo2-vk-hash":
    mutated = dict(texts)
    target = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteHalo2Prover.java"
    mutated[target] = mutated[target].replace(
        KAGEMUSHA_HALO2_CANONICAL_VK_HASH_V1,
        KAGEMUSHA_HALO2_STALE_CANONICAL_VK_HASH_V1,
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate mobile Halo2 VK hash")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected mobile Halo2 VK hash drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: mobile Halo2 VK hash drift was not detected")

if mode == "--negative-control-sdk-readme-boundary":
    mutated = dict(texts)
    target = "IrohaSwift/README.md"
    mutated[target] = mutated[target].replace(
        "opaque native prover\nmaterial",
        "opaque wallet\nmaterial",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README boundary drift was not detected")

if mode == "--negative-control-sdk-readme-proof-chain-accumulator":
    mutated = dict(texts)
    target = "javascript/iroha_js/README.md"
    mutated[target] = mutated[target].replace(
        "Native append streams the previous recursive proof bytes and per-hop accumulator\n"
        "material into native-owned accumulator digests (`recursive_proof_chain_digest`,\n"
        "lineage/aggregation transcript, fixed-window schedule/shared-manifest/table-base,\n"
        "verifier-witness batch, transition-profile, append-opening-preflight,\n"
        "append-boundary, scalar-projection, and previous/resulting accumulator digests);\n"
        "SDK code must not derive, supply, or patch accumulator state.",
        "Native append treats previous recursive proof bytes and accumulator digests as optional SDK metadata.",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README proof-chain accumulator boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README proof-chain accumulator drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README proof-chain accumulator drift was not detected")

if mode == "--negative-control-offline-doc-native-owned-accumulator-boundary":
    mutated = dict(texts)
    target = "docs/source/offline_kagemusha.md"
    mutated[target] = mutated[target].replace(
        "Native append streams the previous recursive proof bytes and per-hop\n"
        "accumulator material into native-owned accumulator digests\n"
        "(`recursive_proof_chain_digest`, lineage/aggregation transcript, fixed-window\n"
        "schedule/shared-manifest/table-base, verifier-witness batch, transition-profile,\n"
        "append-opening-preflight, append-boundary, scalar-projection, and\n"
        "previous/resulting accumulator digests); SDKs must not derive, supply, or patch\n"
        "accumulator state themselves.",
        "Native append lets SDKs supply accumulator digests as optional metadata.",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate offline Kagemusha accumulator boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected offline Kagemusha accumulator boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: offline Kagemusha accumulator boundary drift was not detected")

if mode == "--negative-control-offline-doc-instruction-transaction-surface":
    mutated = dict(texts)
    target = "docs/source/offline_kagemusha.md"
    mutated[target] = mutated[target].replace(
        "empty, malformed, tampered, or wrong-type\n"
        "instruction archives before transaction payload construction",
        "empty instruction archives before transaction payload construction",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate offline Kagemusha instruction transaction surface"
        )
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected offline Kagemusha instruction transaction surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: offline Kagemusha instruction transaction surface drift was not detected"
    )

if mode == "--negative-control-sdk-proof-chain-accumulator-input":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "\nprivate enum StaleProofChainDigestInputFixture { static func append(recursiveProofChainDigest: Data) {} }\n",
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift accumulator digest public input",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "\nprivate object StaleProofChainDigestInputFixture { fun append(proofChainDigest: ByteArray?) {} }\n",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt accumulator digest public input",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "\nfinal class StaleProofChainDigestInputFixture { static void append(final byte[] recursiveProofChainDigest) {} }\n",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java accumulator digest public input",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "\nstatic class StaleProofChainDigestInputFixture { static void Append(ReadOnlySpan<byte> RecursiveProofChainDigest) {} }\n",
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs accumulator digest public input",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "\ndef _stale_proof_chain_digest_input_fixture(recursive_proof_chain_digest: bytes) -> None:\n    pass\n",
            "python/iroha_python/src/iroha_python/kagemusha.py accumulator digest public input",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            "\nfunction staleProofChainDigestInputFixture(recursiveProofChainDigest) { return recursiveProofChainDigest; }\n",
            "javascript/iroha_js/src/crypto.js accumulator digest public input",
        ),
        (
            "javascript/iroha_js/index.d.ts",
            "\nexport interface StaleProofChainDigestInputFixture { recursiveProofChainDigest: BinaryLike; }\n",
            "javascript/iroha_js/index.d.ts accumulator digest public input",
        ),
    )
    expected_labels = []
    for target, addition, label in mutations:
        if addition in mutated[target]:
            raise SystemExit(f"negative control failed: stale proof-chain digest fixture already present in {target}")
        mutated[target] += addition
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK proof-chain accumulator public input drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK proof-chain accumulator public input drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: SDK proof-chain accumulator public input drift was not detected"
    )

if mode == "--negative-control-sdk-accumulator-digest-inputs":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "\nprivate enum StaleAccumulatorDigestInputFixture { static func append(lineageDigest: Data, aggregationTranscriptDigest: Data) {} }\n",
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift accumulator digest public input",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "\nprivate object StaleAccumulatorDigestInputFixture { fun append(aggregationTranscriptDigest: ByteArray?, verifierWitnessBatchDigest: ByteArray?) {} }\n",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt accumulator digest public input",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "\nfinal class StaleAccumulatorDigestInputFixture { static void append(final byte[] fixedWindowTableBaseDigest, final byte[] verifierWitnessBatchDigest) {} }\n",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java accumulator digest public input",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "\nstatic class StaleAccumulatorDigestInputFixture { static void Append(ReadOnlySpan<byte> LineageDigest, ReadOnlySpan<byte> FixedWindowTableBaseDigest) {} }\n",
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs accumulator digest public input",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "\ndef _stale_accumulator_digest_input_fixture(lineage_digest: bytes, aggregation_transcript_digest: bytes) -> None:\n    pass\n",
            "python/iroha_python/src/iroha_python/kagemusha.py accumulator digest public input",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            "\nfunction staleAccumulatorDigestInputFixture(lineageDigest, aggregationTranscriptDigest) { return lineageDigest || aggregationTranscriptDigest; }\n",
            "javascript/iroha_js/src/crypto.js accumulator digest public input",
        ),
        (
            "javascript/iroha_js/index.d.ts",
            "\nexport interface StaleAccumulatorDigestInputFixture { lineageDigest: BinaryLike; aggregationTranscriptDigest: BinaryLike; fixedWindowTableBaseDigest: BinaryLike; verifierWitnessBatchDigest: BinaryLike; }\n",
            "javascript/iroha_js/index.d.ts accumulator digest public input",
        ),
    )
    expected_labels = []
    for target, addition, label in mutations:
        if addition in mutated[target]:
            raise SystemExit(f"negative control failed: stale accumulator digest fixture already present in {target}")
        mutated[target] += addition
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK accumulator digest public input drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK accumulator digest public input drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: SDK accumulator digest public input drift was not detected"
    )

if mode == "--negative-control-sdk-accumulator-boundary-digest-inputs":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "\nprivate enum StaleAccumulatorBoundaryDigestInputFixture { static func append(appendBoundaryDigest: Data, transitionProfileBindingDigest: Data) {} }\n",
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift accumulator digest public input",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "\nprivate object StaleAccumulatorBoundaryDigestInputFixture { fun append(appendOpeningPreflightDigest: ByteArray?, fixedWindowTableScheduleDigest: ByteArray?) {} }\n",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt accumulator digest public input",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "\nfinal class StaleAccumulatorBoundaryDigestInputFixture { static void append(final byte[] fixedWindowSharedTableManifestDigest, final byte[] recursiveVerifierScalarProjectionDigest) {} }\n",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java accumulator digest public input",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "\nstatic class StaleAccumulatorBoundaryDigestInputFixture { static void Append(ReadOnlySpan<byte> AppendBoundaryDigest, ReadOnlySpan<byte> PreviousAccumulatorDigest) {} }\n",
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs accumulator digest public input",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "\ndef _stale_accumulator_boundary_digest_input_fixture(append_boundary_digest: bytes, resulting_accumulator_digest: bytes) -> None:\n    pass\n",
            "python/iroha_python/src/iroha_python/kagemusha.py accumulator digest public input",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            "\nfunction staleAccumulatorBoundaryDigestInputFixture(appendBoundaryDigest, transitionProfileBindingDigest) { return appendBoundaryDigest || transitionProfileBindingDigest; }\n",
            "javascript/iroha_js/src/crypto.js accumulator digest public input",
        ),
        (
            "javascript/iroha_js/index.d.ts",
            "\nexport interface StaleAccumulatorBoundaryDigestInputFixture { appendBoundaryDigest: BinaryLike; appendOpeningPreflightDigest: BinaryLike; transitionProfileBindingDigest: BinaryLike; recursiveVerifierScalarProjectionDigest: BinaryLike; }\n",
            "javascript/iroha_js/index.d.ts accumulator digest public input",
        ),
    )
    expected_labels = []
    for target, addition, label in mutations:
        if addition in mutated[target]:
            raise SystemExit(
                f"negative control failed: stale accumulator boundary digest fixture already present in {target}"
            )
        mutated[target] += addition
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK accumulator boundary digest public input drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK accumulator boundary digest public input drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: SDK accumulator boundary digest public input drift was not detected"
    )

if mode == "--negative-control-sdk-readme-availability-surface":
    mutated = dict(texts)
    target = "IrohaSwift/README.md"
    mutated[target] = mutated[target].replace(
        "the append-boundary helper, ",
        "",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README availability surface")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README availability surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README availability surface drift was not detected")

if mode == "--negative-control-sdk-readme-recursive-compact-unavailable":
    mutated = dict(texts)
    target = "kotlin/README.md"
    mutated[target] = mutated[target].replace(
        "reserved ABI-7 state",
        "ABI-7 native state",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README recursive compact unavailable boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README recursive compact unavailable drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README recursive compact unavailable drift was not detected")

if mode == "--negative-control-sdk-readme-compact-projection-verifier":
    mutated = dict(texts)
    target = "IrohaSwift/README.md"
    mutated[target] = mutated[target].replace(
        "verifyRecursiveSpendCompactPaymentTokenProjection(compactTokenArchive:verifierRecordArchive:blockHeight:)",
        "verifyRecursiveSpendCompactPaymentTokenProjection(compactTokenArchive:verifierRecordArchive:)",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README compact projection verifier boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README compact projection verifier drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README compact projection verifier drift was not detected")

if mode == "--negative-control-sdk-readme-stale-future-lineage":
    mutated = dict(texts)
    target = "java/iroha_android/README.md"
    mutated[target] = mutated[target].replace(
        "Reserved-lineage append output is valid only when",
        "Future Reserved-lineage append output is valid only when",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate stale SDK README wording")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected stale SDK README Reserved-lineage wording")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: stale SDK README Reserved-lineage wording was not detected")

if mode == "--negative-control-sdk-readme-native-output-csharp":
    mutated = dict(texts)
    target = "docs/source/offline_kagemusha.md"
    mutated[target] = mutated[target].replace(
        "Python, Swift, JavaScript/Node, Kotlin/JVM, Java Android, and C# also fail\nclosed",
        "Python, Swift, JavaScript/Node, Kotlin/JVM, and Java Android also fail\nclosed",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate SDK README native output C# boundary")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected SDK README native output C# boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK README native output C# boundary drift was not detected")

if mode == "--negative-control-cross-sdk-helper-bodies":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "circuitId == recursiveSpendLineageProofCircuitIdV1",
            "circuitId.isEmpty",
            "Swift witnessless Reserved-lineage helper bounds",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
            "circuitId.isNullOrEmpty()",
            "Kotlin witnessless Reserved-lineage helper bounds",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.equals(circuitId)",
            "circuitId == null",
            "Android witnessless Reserved-lineage helper bounds",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
            "bool(proof_circuit_id)",
            "Python witnessless Reserved-lineage helper bounds",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "circuitId == RecursiveSpendLineageProofCircuitIdV1",
            "circuitId is null",
            "C# witnessless Reserved-lineage helper bounds",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {target}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: helper body drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected cross-SDK helper body drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK helper body drift was not detected")

if mode == "--negative-control-cross-sdk-preferred-mode-fallback":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
            "_ = recursiveCompactAvailable",
            "if recursiveCompactAvailable { return .recursiveCompactV1 }",
            "Swift preferred Kagemusha mode fallback policy",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            "void recursiveCompactAvailable;",
            "if (recursiveCompactAvailable) { return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1; }",
            "javascript/iroha_js/src/crypto.js preferred Kagemusha mode fallback policy",
        ),
        (
            "javascript/iroha_js/dist/crypto.js",
            "void recursiveCompactAvailable;",
            "if (recursiveCompactAvailable) { return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1; }",
            "javascript/iroha_js/dist/crypto.js preferred Kagemusha mode fallback policy",
        ),
        (
            "javascript/iroha_js/src/crypto.browser.js",
            "void recursiveCompactAvailable;",
            "if (recursiveCompactAvailable) { return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1; }",
            "javascript/iroha_js/src/crypto.browser.js preferred Kagemusha mode fallback policy",
        ),
        (
            "javascript/iroha_js/dist/crypto.browser.js",
            "void recursiveCompactAvailable;",
            "if (recursiveCompactAvailable) { return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1; }",
            "javascript/iroha_js/dist/crypto.browser.js preferred Kagemusha mode fallback policy",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "_ = recursive_compact_available",
            "if recursive_compact_available: return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1",
            "Python preferred Kagemusha mode fallback policy",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
            "return if (recursiveSpendAvailable) {",
            "return if (recursiveCompactAvailable) {",
            "Kotlin preferred Kagemusha mode fallback policy",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
            "return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;",
            "return recursiveCompactAvailable ? Mode.RECURSIVE_COMPACT_V1 : Mode.CHECKED_PREFOLD_V1;",
            "Android Java preferred Kagemusha mode fallback policy",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "_ = recursiveCompactAvailable;",
            "if (recursiveCompactAvailable) { return KagemushaOfflineSpendMode.RecursiveCompactV1; }",
            "C# preferred Kagemusha mode fallback policy",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {target}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: preferred-mode fallback drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected cross-SDK preferred-mode fallback drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK preferred-mode fallback drift was not detected")

if mode == "--negative-control-jvm-offline-note-v2-decoder-placeholder":
    mutated = dict(texts)
    stale_placeholder = "\n// Offline Note V2 decoding is not supported yet\n"
    mutations = (
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
            "Kotlin Offline Note V2 decoder placeholder removal",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
            "Android Java Offline Note V2 decoder placeholder removal",
        ),
    )
    expected_labels = []
    for target, label in mutations:
        if stale_placeholder in mutated[target]:
            raise SystemExit(f"negative control failed: stale decoder placeholder already present in {target}")
        mutated[target] += stale_placeholder
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Offline Note V2 decoder placeholder drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected JVM Offline Note V2 decoder placeholder drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JVM Offline Note V2 decoder placeholder drift was not detected"
    )

if mode == "--negative-control-jvm-offline-note-v2-instruction-wrapper":
    mutated = dict(texts)
    mutations = (
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
            "fun issueInstruction(value: IssueV2): InstructionBox",
            "fun issueInstructionDisabled(value: IssueV2): InstructionBox",
            "Kotlin Offline Note V2 instruction wrapper surface",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
            "public static InstructionBox issueInstruction(final IssueV2 value)",
            "public static InstructionBox issueInstructionDisabled(final IssueV2 value)",
            "Android Java Offline Note V2 instruction wrapper surface",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: JVM Offline Note V2 instruction wrapper drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected JVM Offline Note V2 instruction wrapper drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JVM Offline Note V2 instruction wrapper drift was not detected"
    )

if mode == "--negative-control-jvm-offline-note-v2-instruction-decoder":
    mutated = dict(texts)
    mutations = (
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
            "fun decodeIssueInstruction(bytes: ByteArray): IssueV2",
            "fun decodeIssueInstructionDisabled(bytes: ByteArray): IssueV2",
            "Kotlin Offline Note V2 instruction decoder surface",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
            "public static IssueV2 decodeIssueInstruction(final byte[] bytes)",
            "public static IssueV2 decodeIssueInstructionDisabled(final byte[] bytes)",
            "Android Java Offline Note V2 instruction decoder surface",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: JVM Offline Note V2 instruction decoder drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected JVM Offline Note V2 instruction decoder drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JVM Offline Note V2 instruction decoder drift was not detected"
    )

if mode == "--negative-control-offline-note-v2-canonical-instruction-wire-names":
    mutated = dict(texts)
    mutations = (
        (
            "IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift",
            'static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNote"',
            'static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNoteV2"',
            "Swift Offline Note V2 canonical instruction wire names",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
            '"iroha_data_model::isi::offline::IssueOfflineNote"',
            '"iroha_data_model::isi::offline::IssueOfflineNoteV2"',
            "Kotlin Offline Note V2 instruction wrapper canonical instruction wire names",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
            '"iroha_data_model::isi::offline::IssueOfflineNote";',
            '"iroha_data_model::isi::offline::IssueOfflineNoteV2";',
            "Android Java Offline Note V2 instruction wrapper canonical instruction wire names",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Offline Note V2 canonical instruction wire-name drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Offline Note V2 canonical instruction wire-name drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Offline Note V2 canonical instruction wire-name drift was not detected"
    )

if mode == "--negative-control-swift-offline-note-v2-decoder-placeholder":
    mutated = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift"
    stale_placeholder = "\n// Offline Note V2 decoding is not supported yet\n"
    if stale_placeholder in mutated[target]:
        raise SystemExit(f"negative control failed: stale decoder placeholder already present in {target}")
    mutated[target] += stale_placeholder
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "Swift Offline Note V2 decoder placeholder removal"
        if label not in message:
            raise SystemExit(
                "negative control failed: Swift Offline Note V2 decoder placeholder drift was not detected"
            )
        print("negative control rejected Swift Offline Note V2 decoder placeholder drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Swift Offline Note V2 decoder placeholder drift was not detected"
    )

if mode == "--negative-control-swift-offline-note-v2-instruction-decoder":
    mutated = dict(texts)
    target = "IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift"
    mutations = (
        (
            "public static func decodeIssueInstruction(_ data: Data) throws -> OfflineNoteIssueV2",
            "public static func decodeIssueInstructionDisabled(_ data: Data) throws -> OfflineNoteIssueV2",
            "Swift Offline Note V2 issue instruction decoder API",
        ),
        (
            "public static func decodeRedeemInstruction(_ data: Data) throws -> OfflineNoteRedeemV2",
            "public static func decodeRedeemInstructionDisabled(_ data: Data) throws -> OfflineNoteRedeemV2",
            "Swift Offline Note V2 redeem instruction decoder API",
        ),
        (
            "public static func decodeAuditInstruction(_ data: Data) throws -> OfflineNoteAuditBundleV2",
            "public static func decodeAuditInstructionDisabled(_ data: Data) throws -> OfflineNoteAuditBundleV2",
            "Swift Offline Note V2 audit instruction decoder API",
        ),
    )
    expected_labels = []
    for old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Swift Offline Note V2 instruction decoder drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Swift Offline Note V2 instruction decoder drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Swift Offline Note V2 instruction decoder drift was not detected"
    )

if mode == "--negative-control-rust-recursive-compact-unavailable-classifier":
    mutated_texts = dict(texts)
    old = """    matches!(
        err,
        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE
            | iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
    )"""
    new = """    err.contains(iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE)
        || err.contains(iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE)"""
    mutations = (
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "Rust recursive compact unavailable classifier",
        ),
        (
            "crates/iroha_js_host/src/lib.rs",
            "Node recursive compact unavailable classifier",
        ),
        (
            "python/iroha_python/iroha_python_rs/src/lib.rs",
            "Python PyO3 recursive compact unavailable classifier",
        ),
    )
    expected_labels = []
    for target, label in mutations:
        original = read(target)
        mutated = original.replace(old, new, 1)
        if mutated == original:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        mutated_texts[target] = mutated
        expected_labels.append(label)
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: recursive compact unavailable classifier drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Rust recursive compact unavailable classifier drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Rust recursive compact unavailable classifier drift was not detected")

if mode == "--negative-control-sdk-recursive-compact-unavailable-helper":
    mutated = dict(texts)
    mutations = (
        (
            "javascript/iroha_js/src/crypto.js",
            "isKagemushaRecursiveCompactUnavailable(error)",
            "isKagemushaRecursiveCompactMaybeUnavailable(error)",
            "JavaScript recursive compact verifier gate",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            "def is_kagemusha_recursive_compact_unavailable(error: object) -> bool:",
            "def is_kagemusha_recursive_compact_maybe_unavailable(error: object) -> bool:",
            "Python recursive compact verifier surface",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        mutated[target] = mutated[target].replace(old, new, 1)
        if mutated[target] == texts[target]:
            raise SystemExit(f"negative control failed: unable to mutate {label}")
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: SDK recursive compact unavailable helper drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected SDK recursive compact unavailable helper drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK recursive compact unavailable helper drift was not detected")

if mode == "--negative-control-recursive-compact-verifier-surface":
    mutated = dict(texts)
    mutations = (
        (
            "crates/connect_norito_bridge/include/connect_norito_bridge.h",
            "int32_t connect_norito_kagemusha_verify_recursive_compact_payment_token(",
            "int32_t connect_norito_kagemusha_check_recursive_compact_payment_token(",
            "C header recursive compact declarations",
        ),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_unchecked_archive",
            "Rust recursive compact C package-backed Pallas prover",
        ),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_unchecked_archive",
            "Rust recursive compact JNI package-backed Pallas prover",
        ),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "oversized recursive compact record-bundle input must clear stale output pointers",
            "oversized recursive compact record-bundle input may keep stale output pointers",
            "Rust recursive compact verifier contract",
        ),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "windowed recursive compact verifier records must reject before proving",
            "windowed recursive compact verifier records may map to unavailable",
            "Rust recursive compact verifier contract",
        ),
        (
            "crates/iroha_js_host/src/lib.rs",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
            "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_unchecked_archive",
            "Node recursive compact verifier export",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            'typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"',
            'typeof native.kagemushaCheckRecursiveCompactPaymentToken !== "function"',
            "JavaScript recursive compact verifier gate",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            'const compactToken = toOwnedKagemushaArchiveBuffer(',
            'const compactToken = toOwnedBuffer(',
            "JavaScript recursive compact verifier gate",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            '"kagemusha_verify_recursive_compact_payment_token"',
            '"kagemusha_check_recursive_compact_payment_token"',
            "Python recursive compact verifier surface",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            '_assert_kagemusha_norito_archive(compact_token, "compact_token_archive")',
            '_assert_kagemusha_norito_archive_unchecked(compact_token, "compact_token_archive")',
            "Python recursive compact verifier surface",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
            "public static func verifyRecursiveCompactPaymentToken(",
            "public static func checkRecursiveCompactPaymentToken(",
            "Swift recursive compact wrapper",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
            "requireValidRecursiveCompactTokenArchive(compactTokenArchive)",
            "requireValidRecursiveCompactTokenArchiveUnchecked(compactTokenArchive)",
            "Swift recursive compact wrapper",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
            "try requireValidRecursiveCompactTokenArchive(token)",
            "try requireValidRecursiveCompactTokenArchiveUnchecked(token)",
            "Swift recursive compact wrapper",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            "recursiveCompactVerifierKeysArchive: ByteArray?",
            "recursiveCompactVerifierKeysArchiveUnchecked: ByteArray?",
            "Kotlin recursive compact wrapper",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            "final byte[] recursiveCompactVerifierKeysArchive",
            "final byte[] recursiveCompactVerifierKeysArchiveUnchecked",
            "Android Java recursive compact wrapper",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "ReadOnlySpan<byte> recursiveCompactVerifierKeysArchive)",
            "ReadOnlySpan<byte> recursiveCompactVerifierKeysArchiveUnchecked)",
            "C# recursive compact wrapper",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "RequireValidRecursiveCompactTokenArchive(compactToken)",
            "RequireValidRecursiveCompactTokenArchiveUnchecked(compactToken)",
            "C# recursive compact wrapper",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "RequireValidNativeOutput(symbol, result)",
            "RequireValidNativeOutputUnchecked(symbol, result)",
            "C# recursive compact wrapper",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {target}")
        mutated[target] = updated
        expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: recursive compact verifier drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected ABI-7 recursive compact verifier surface drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: ABI-7 recursive compact verifier surface drift was not detected"
    )

if mode == "--negative-control-recursive-compact-key-package-arity":
    mutated = dict(texts)
    stale_overloads = (
        (
            "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
            """

enum StaleRecursiveCompactKeyPackageArityFixture {
    static func prove(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        recursiveCompactKeyArtifactsArchive: Data = Data()
    ) {}

    static func verify(
        compactTokenArchive: Data,
        recursiveCompactVerifierKeysArchive: Data = Data()
    ) -> Bool { false }
}
""",
            (
                "Swift recursive compact prover public key-package argument",
                "Swift recursive compact verifier public key-package argument",
            ),
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            """

private object StaleRecursiveCompactKeyPackageArityFixture {
    fun proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: ByteArray?,
        pallasOpenEnvelopesArchive: ByteArray?,
    ): ByteArray = ByteArray(0)

    fun verifyRecursiveCompactPaymentToken(compactTokenArchive: ByteArray?): Boolean = false
}
""",
            (
                "Kotlin recursive compact prover public key-package arity",
                "Kotlin recursive compact verifier public key-package arity",
            ),
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            """

final class StaleRecursiveCompactKeyPackageArityFixture {
  public static byte[] proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
      final byte[] recordBundleArchive,
      final byte[] pallasOpenEnvelopesArchive) {
    return new byte[0];
  }

  public static boolean verifyRecursiveCompactPaymentToken(
      final byte[] compactTokenArchive) {
    return false;
  }
}
""",
            (
                "Android Java recursive compact prover public key-package arity",
                "Android Java recursive compact verifier public key-package arity",
            ),
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            """

public static class StaleRecursiveCompactKeyPackageArityFixture
{
    public static KagemushaRecursiveCompactPaymentTokenArchive ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive) =>
        throw new InvalidOperationException();

    public static bool VerifyRecursiveCompactPaymentToken(
        ReadOnlySpan<byte> compactTokenArchive) => false;
}
""",
            (
                "C# recursive compact prover public key-package arity",
                "C# recursive compact verifier public key-package arity",
            ),
        ),
    )
    expected_labels = []
    for target, addition, labels in stale_overloads:
        if addition in mutated[target]:
            raise SystemExit(f"negative control failed: stale key-package arity fixture already present in {target}")
        mutated[target] += addition
        expected_labels.extend(labels)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: recursive compact key-package arity drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected ABI-7 recursive compact key-package arity drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: ABI-7 recursive compact key-package arity drift was not detected"
    )

if mode == "--negative-control-python-recursive-compact-probe-arity":
    mutated = dict(texts)
    target = "python/iroha_python/src/iroha_python/kagemusha.py"
    original = read(target)
    stale_prover_probe = (
        "            _RECURSIVE_COMPACT_TOKEN_METHOD,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
    )
    stale_verifier_probe = (
        "            _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
    )
    mutated_text = original.replace(
        stale_prover_probe,
        "            _RECURSIVE_COMPACT_TOKEN_METHOD,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n",
        1,
    ).replace(
        stale_verifier_probe,
        "            _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,\n"
        "            _MALFORMED_NATIVE_PROBE_ARCHIVE,\n",
        2,
    )
    if mutated_text == original:
        raise SystemExit("negative control failed: unable to mutate Python recursive compact probe arity")
    mutated[target] = mutated_text
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Python recursive compact probe arity drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python recursive compact probe arity drift was not detected"
    )

if mode == "--negative-control-js-recursive-compact-key-package-dispatch":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        "package dist Kagemusha recursive compact requires key packages before native dispatch",
        "package dist Kagemusha recursive compact allows missing key packages before native dispatch",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript recursive compact key-package dispatch test"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist recursive compact key-package dispatch coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript recursive compact key-package dispatch drift was not detected"
            )
        print("negative control rejected JavaScript recursive compact key-package dispatch drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript recursive compact key-package dispatch drift was not detected"
    )

if mode == "--negative-control-js-package-dist-recursive-compact-declarations":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        "package declarations expose recursive compact key-package signatures",
        "package declarations omit recursive compact key-package signatures",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist recursive compact declarations test"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist recursive compact declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist recursive compact declaration drift was not detected"
            )
        print("negative control rejected JavaScript package dist recursive compact declaration drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist recursive compact declaration drift was not detected"
    )

if mode == "--negative-control-js-package-dist-accumulator-digest-declarations":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        "package declarations keep accumulator digests native-owned",
        "package declarations allow accumulator digest inputs",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist accumulator digest declarations test"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist accumulator digest declaration drift was not detected"
            )
        print("negative control rejected JavaScript package dist accumulator digest declaration drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist accumulator digest declaration drift was not detected"
    )

if mode == "--negative-control-js-package-dist-accumulator-digest-denylist":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        "appendBoundaryDigest|AppendBoundaryDigest|append_boundary_digest|",
        "",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist accumulator digest denylist"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist accumulator digest denylist drift was not detected"
            )
        print("negative control rejected JavaScript package dist accumulator digest denylist drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist accumulator digest denylist drift was not detected"
    )

if mode == "--negative-control-js-package-dist-terminal-accumulator-digest-denylist":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        "previousAccumulatorDigest|PreviousAccumulatorDigest|previous_accumulator_digest|resultingAccumulatorDigest|ResultingAccumulatorDigest|resulting_accumulator_digest|",
        "",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist terminal accumulator digest denylist"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist terminal accumulator digest denylist drift was not detected"
            )
        print("negative control rejected JavaScript package dist terminal accumulator digest denylist drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist terminal accumulator digest denylist drift was not detected"
    )

if mode == "--negative-control-js-package-dist-declaration-sweep":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        '  ["connect.browser.d.ts", readFileSync(new URL("../connect.browser.d.ts", import.meta.url), "utf8")],\n',
        "",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist declaration sweep"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist declaration sweep drift was not detected"
            )
        print("negative control rejected JavaScript package dist declaration sweep drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist declaration sweep drift was not detected"
    )

if mode == "--negative-control-js-package-dist-nexus-declaration-sweep":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        '  ["nexus-app.d.ts", readFileSync(new URL("../nexus-app.d.ts", import.meta.url), "utf8")],\n',
        "",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist Nexus declaration sweep"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist Nexus declaration sweep drift was not detected"
            )
        print("negative control rejected JavaScript package dist Nexus declaration sweep drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist Nexus declaration sweep drift was not detected"
    )

if mode == "--negative-control-js-package-dist-kotodama-declaration-sweep":
    mutated = dict(texts)
    target = "javascript/iroha_js/test/package_dist.test.js"
    original = mutated[target]
    updated = original.replace(
        '  ["kotodama-compiler.d.ts", readFileSync(new URL("../kotodama-compiler.d.ts", import.meta.url), "utf8")],\n',
        "",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript package dist Kotodama declaration sweep"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "JavaScript package dist accumulator digest declaration coverage"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript package dist Kotodama declaration sweep drift was not detected"
            )
        print("negative control rejected JavaScript package dist Kotodama declaration sweep drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript package dist Kotodama declaration sweep drift was not detected"
    )

if mode == "--negative-control-js-dts-recursive-compact-key-package":
    mutated = dict(texts)
    target = "javascript/iroha_js/index.d.ts"
    original = mutated[target]
    updated = original.replace(
        "  recursiveCompactKeyArtifactsArchive: BinaryLike,\n",
        "  recursiveCompactKeyArtifactsArchive?: BinaryLike,\n",
        1,
    )
    updated = updated.replace(
        "  recursiveCompactVerifierKeysArchive: BinaryLike,\n",
        "  recursiveCompactVerifierKeysArchive?: BinaryLike,\n",
        1,
    )
    if updated == original:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript TypeScript recursive compact key-package declarations"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        expected_labels = (
            "JavaScript TypeScript recursive compact prover key-package declaration",
            "JavaScript TypeScript recursive compact verifier key-package declaration",
        )
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: JavaScript TypeScript recursive compact key-package declaration drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected JavaScript TypeScript recursive compact key-package declaration drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript TypeScript recursive compact key-package declaration drift was not detected"
    )

if mode == "--negative-control-python-recursive-compact-root-export":
    mutated = dict(texts)
    target = "python/iroha_python/src/iroha_python/__init__.py"
    updated = mutated[target]
    for method in REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS:
        updated = updated.replace(f'    "{method}",\n', "", 1)
        updated = updated.replace(f"        {method},\n", "", 1)
    if updated == mutated[target]:
        raise SystemExit(
            "negative control failed: unable to mutate Python recursive compact root exports"
        )
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        label = "Python package recursive compact re-exports"
        if label not in message:
            raise SystemExit(
                "negative control failed: Python recursive compact root re-export drift was not detected"
            )
        print("negative control rejected Python recursive compact root re-export drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python recursive compact root re-export drift was not detected"
    )

if mode == "--negative-control-recursive-spend-compact-projection-surface":
    mutated_texts = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated = mutated_texts[target].replace(
        "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
        "kagemushaRecursiveSpendCompactPaymentTokenFromBundleMissing",
        1,
    )
    if mutated == mutated_texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate recursive spend compact projection surface"
        )
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        print("negative control rejected recursive spend compact projection surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: recursive spend compact projection surface drift was not detected"
    )

if mode == "--negative-control-js-compact-projection-block-height-validation":
    mutated_texts = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated = mutated_texts[target].replace(
        "const checkedBlockHeight = normalizeKagemushaBlockHeight(blockHeight);",
        "const checkedBlockHeight = blockHeight;",
        1,
    )
    if mutated == mutated_texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate JavaScript compact projection block-height validation"
        )
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        label = "JavaScript recursive spend compact projection gate"
        if label not in message:
            raise SystemExit(
                "negative control failed: JavaScript compact projection block-height validation drift was not detected"
            )
        print("negative control rejected JavaScript compact projection block-height validation drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JavaScript compact projection block-height validation drift was not detected"
    )

if mode == "--negative-control-python-recursive-spend-compact-projection-root-export":
    mutated_texts = dict(texts)
    target = "python/iroha_python/src/iroha_python/__init__.py"
    method = "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
    mutated = mutated_texts[target].replace(f'    "{method}",\n', "", 1)
    mutated = mutated.replace(f"        {method},\n", "", 1)
    if mutated == mutated_texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate Python recursive spend compact projection root export"
        )
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        label = "Python package recursive spend compact projection re-exports"
        if label not in message:
            raise SystemExit(
                "negative control failed: Python recursive spend compact projection root export drift was not detected"
            )
        print("negative control rejected Python recursive spend compact projection root export drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python recursive spend compact projection root export drift was not detected"
    )

if mode == "--negative-control-jvm-compact-projection-unsigned-block-height":
    mutated_texts = dict(texts)
    target = "crates/connect_norito_bridge/src/lib.rs"
    mutated = mutated_texts[target].replace(
        "let height = block_height.map(java_jlong_to_u64_bits);",
        (
            "let height = match block_height {\n"
            "            Some(value) if value < 0 => return Err(\"blockHeight must be non-negative\".to_owned()),\n"
            "            Some(value) => Some(value as u64),\n"
            "            None => None,\n"
            "        };"
        ),
        1,
    )
    if mutated == mutated_texts[target]:
        raise SystemExit(
            "negative control failed: unable to mutate JVM compact projection unsigned block-height carrier"
        )
    mutated_texts[target] = mutated
    try:
        run_checks(mutated_texts)
    except ParityError as error:
        message = str(error)
        label = "Rust JNI recursive compact projection block-height carrier"
        if label not in message:
            raise SystemExit(
                "negative control failed: JVM compact projection unsigned block-height drift was not detected"
            )
        print("negative control rejected JVM compact projection unsigned block-height drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JVM compact projection unsigned block-height drift was not detected"
    )

if mode == "--negative-control-native-bridge-zero-envelope-pallas-guard":
    mutated = dict(texts)
    target = "crates/connect_norito_bridge/src/lib.rs"
    original = mutated[target]
    updated = original.replace(
        "zero-envelope Pallas archive",
        "one-envelope Pallas archive",
        1,
    )
    updated = updated.replace(
        "zero-envelope nested Pallas archives",
        "one-envelope nested Pallas archives",
        1,
    )
    if updated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge zero-envelope Pallas guard")
    mutated[target] = updated
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        if "Rust C recursive spend nested Pallas guard" not in message:
            raise SystemExit(
                "negative control failed: native bridge zero-envelope Pallas guard drift was not detected"
            )
        print("negative control rejected native bridge zero-envelope Pallas guard drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native bridge zero-envelope Pallas guard drift was not detected"
    )

if mode == "--negative-control-kagemusha-abi-probe-bounds":
    mutated = dict(texts)
    target = "javascript/iroha_js/src/crypto.js"
    mutated[target] = mutated[target].replace(
        "version <= KAGEMUSHA_MAX_BRIDGE_ABI_VERSION",
        "version <= Number.MAX_SAFE_INTEGER",
        1,
    )
    if mutated[target] == texts[target]:
        raise SystemExit("negative control failed: unable to mutate Kagemusha ABI probe bounds")
    try:
        run_checks(mutated)
    except ParityError as error:
        print("negative control rejected Kagemusha ABI probe bounds drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Kagemusha ABI probe bounds drift was not detected")

if mode == "--negative-control-kagemusha-probe-rejection-shape":
    mutated = dict(texts)
    mutations = (
        (
            "javascript/iroha_js/src/crypto.js",
            "/\\b(?:archive|Norito|probe)\\b/i.test(error.message)",
            "true",
            "JavaScript recursive compact verifier gate",
        ),
        (
            "javascript/iroha_js/dist/crypto.js",
            "/\\b(?:archive|Norito|probe)\\b/i.test(error.message)",
            "true",
            "JavaScript recursive compact verifier gate",
        ),
        (
            "python/iroha_python/src/iroha_python/kagemusha.py",
            '("archive", "norito", "probe")',
            '("kagemusha",)',
            "Python recursive compact verifier surface",
        ),
    )
    expected_labels = []
    for target, old, new, label in mutations:
        updated = mutated[target].replace(old, new, 1)
        if updated == mutated[target]:
            raise SystemExit(f"negative control failed: unable to mutate {target}")
        mutated[target] = updated
        if label not in expected_labels:
            expected_labels.append(label)
    try:
        run_checks(mutated)
    except ParityError as error:
        message = str(error)
        missing = [label for label in expected_labels if label not in message]
        if missing:
            raise SystemExit(
                "negative control failed: Kagemusha probe rejection shape drift was not detected for "
                + ", ".join(missing)
            )
        print("negative control rejected Kagemusha probe rejection shape drift")
        print(message.splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Kagemusha probe rejection shape drift was not detected")

if mode:
    raise SystemExit(f"unknown mode: {mode}")

run_checks(texts)
print("recursive Kagemusha ABI-6/ABI-7 SDK parity is consistent")
PY

if [[ -z "$MODE" && "$(uname -s)" == "Darwin" ]]; then
  BRIDGE_ROOT="$ROOT_DIR/dist/NoritoBridge.xcframework"
  if [[ -d "$BRIDGE_ROOT" ]]; then
    REQUIRED_BRIDGE_SYMBOLS=(
      "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
      "connect_norito_kagemusha_verify_recursive_compact_payment_token"
      "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle"
      "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection"
    )
    BRIDGE_LIBS=(
      "$BRIDGE_ROOT/ios-arm64/libNoritoBridge.a"
      "$BRIDGE_ROOT/ios-arm64_x86_64-simulator/libNoritoBridge.a"
      "$BRIDGE_ROOT/macos-arm64/libNoritoBridge.a"
    )
    BRIDGE_SYMBOLS_DUMP=$(mktemp -t norito-bridge-symbols.XXXXXX)
    trap 'rm -f "$BRIDGE_SYMBOLS_DUMP"' EXIT
    for bridge_lib in "${BRIDGE_LIBS[@]}"; do
      if [[ ! -f "$bridge_lib" ]]; then
        echo "[-] NoritoBridge artifact missing library: $bridge_lib" >&2
        exit 1
      fi
      nm -gU "$bridge_lib" > "$BRIDGE_SYMBOLS_DUMP" 2>/dev/null
      for symbol in "${REQUIRED_BRIDGE_SYMBOLS[@]}"; do
        if ! grep -q "_$symbol" "$BRIDGE_SYMBOLS_DUMP"; then
          echo "[-] NoritoBridge artifact $bridge_lib is missing symbol: $symbol" >&2
          exit 1
        fi
      done
    done
    echo "NoritoBridge XCFramework recursive Kagemusha symbols are present"
  fi
fi
