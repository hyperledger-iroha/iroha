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

REQUIRED_C_SYMBOLS = (
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

REQUIRED_RECURSIVE_COMPACT_JS_PUBLIC_EXPORTS = (
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
    *REQUIRED_RECURSIVE_COMPACT_JS_METHODS,
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

REQUIRED_RECURSIVE_COMPACT_PYTHON_PUBLIC_METHODS = (
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "is_kagemusha_recursive_compact_payment_token_prover_available",
    "is_kagemusha_recursive_compact_payment_token_verifier_available",
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
)

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
)

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
    "nativeVerifyRecursiveCompactPaymentToken",
)

SOURCE_PATHS = (
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "crates/connect_norito_bridge/include/NoritoBridge.h",
    "crates/iroha_core/src/zk.rs",
    "crates/iroha_data_model/src/offline/mod.rs",
    "crates/iroha_js_host/src/lib.rs",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/src/crypto.browser.js",
    "javascript/iroha_js/dist/crypto.browser.js",
    "javascript/iroha_js/src/index.js",
    "javascript/iroha_js/dist/index.js",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/package.json",
    "javascript/iroha_js/package-lock.json",
    "javascript/iroha_js/test/crypto.browser.test.js",
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
    "javascript/iroha_js/test/package_dist.test.js",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_python/src/iroha_python/kagemusha.py",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
    "python/iroha_python/tests/kagemusha_test.py",
    "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj",
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
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
SDK_PARITY_MAIN_COMMAND = "ci/check_kagemusha_recursive_spend_sdk_parity.sh"
PYTHON_BYTECODE_COMMAND = "bash ci/check_no_tracked_python_bytecode.sh"
NATIVE_BRIDGE_TEST_COMMAND = "cargo test -p connect_norito_bridge kagemusha_recursive_spend_ffi_rejects_invalid_archives_without_output --lib -- --test-threads=1"
PYTHON_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_python_sdk.sh"
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
        "SDK public helper surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-helper-surface",
    ),
    (
        "SDK README boundary negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-boundary",
    ),
    (
        "SDK README availability surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-availability-surface",
    ),
    (
        "SDK README stale Reserved-lineage wording negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-sdk-readme-stale-future-lineage",
    ),
    (
        "cross-SDK helper-body negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-cross-sdk-helper-bodies",
    ),
    (
        "ABI-7 recursive compact verifier surface negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-recursive-compact-verifier-surface",
    ),
    (
        "Kagemusha ABI probe bounds negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-kagemusha-abi-probe-bounds",
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
        "Python SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh --negative-control-python-sdk-test-workflow",
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
        command_match is not None,
        "Kagemusha payload workflow must run the Python recursive spend SDK tests",
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
        "Kagemusha recursive spend|Kagemusha record-backed|Kagemusha .* SDK runner|browser crypto exposes native-only helpers as safe stubs" in script
        and "test/crypto.browser.test.js" in script
        and "test/kagemushaFfiContractParity.test.js" in script
        and "test/kagemushaRecursiveSpend.test.js" in script
        and "test/package_dist.test.js" in script,
        "Kagemusha JavaScript SDK script must run recursive spend, browser-stub, package-dist, and runtime-gate meta tests",
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
            "Malformed archives and malformed token bindings return ERR_KAGEMUSHA_PROVE.",
            "Output: `*out_valid = 0` for every shape-valid token in this release.",
        ),
        "C header recursive compact verifier contract",
        errors,
    )
    require_contains(
        texts,
        "crates/connect_norito_bridge/src/lib.rs",
        (
            "*out_valid = 0",
            "preverify_kagemusha_recursive_compact_payment_token",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "Vec<iroha_data_model::zk::OpenVerifyEnvelope>",
            "malformed Pallas opening archives before the unavailable gate",
            "malformed public-input bindings before returning a soft invalid result",
            "KagemushaRecursiveCompactUnavailable",
        ),
        "Rust recursive compact verifier contract",
        errors,
    )
    require_contains(
        texts,
        "crates/iroha_core/src/zk.rs",
        (
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "semantic ABI-7 compact tokens are disabled for production",
        ),
        "Rust recursive compact fail-closed diagnostic",
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
        "crates/iroha_js_host/src/lib.rs",
        [f'js_name = "{name}"' for name in REQUIRED_RECURSIVE_COMPACT_JS_METHODS]
        + [
            "napi::Result<bool>",
            "Ok(false)",
            "preverify_kagemusha_recursive_compact_payment_token",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
            "Kagemusha recursive compact Pallas open-envelope archive",
            "public-input hash mismatch",
        ],
        "Node recursive compact verifier export",
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
                'typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"',
                "native.kagemushaVerifyRecursiveCompactPaymentToken(KAGEMUSHA_NATIVE_PROBE_ARCHIVE)",
                'assertKagemushaNoritoArchive(compactToken, "compactTokenArchive")',
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
            "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(): boolean",
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
            "recursive compact Kagemusha payment-token verifier requires native bridge ABI 7",
            "compactTokenArchive must be a valid Norito archive",
            "compactTokenArchive must contain a non-empty Norito payload",
            "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
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
            "bridge ABI 7 with compact prover and verifier symbols",
            "bridge ABI 7 with the compact verifier symbol",
            '_assert_kagemusha_norito_archive(compact_token, "compact_token_archive")',
            "returned non-boolean result",
        ),
        "Python recursive compact verifier surface",
        errors,
    )
    require_contains(
        texts,
        init,
        REQUIRED_RECURSIVE_COMPACT_PYTHON_PUBLIC_METHODS,
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
            "public-input hash mismatch",
        ],
        "Python PyO3 recursive compact exports",
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
            "returned non-boolean result",
        ),
        "Python recursive compact verifier tests",
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
            "public static func proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            "public static func verifyRecursiveCompactPaymentToken",
            "Kagemusha recursive compact-token archive must not be empty.",
            "try requireValidInputArchive(",
            "try requireValidRecursiveCompactTokenArchive(token)",
            "requireValidRecursiveCompactTokenArchive(compactTokenArchive)",
            "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
            "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
            "Kagemusha recursive compact-token archive must be a valid Norito archive.",
            "Kagemusha recursive compact-token archive must contain a non-empty Norito payload.",
            "Kagemusha recursive compact-token archive was rejected by the native verifier.",
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
            "testVerifyRejectsEmptyPayloadCompactTokenArchiveBeforeBridgeCall",
            "testRejectsMalformedInputArchivesBeforeBridgeCall",
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            "validKagemushaNoritoArchive",
            "testVerifyReturnsNativeBoolean",
            "testVerifyNilNativeResultIsBridgeUnavailable",
            "testVerifyNativeRejectionIsVerificationRejected",
        ),
        "Swift recursive compact verifier tests",
        errors,
    )

    jvm_files = (
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
            "Kotlin recursive compact wrapper",
            "REQUIRED_BRIDGE_ABI_VERSION: Int = 7",
            "fun verifyRecursiveCompactPaymentToken(compactTokenArchive: ByteArray): Boolean",
            "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(compactTokenArchive)",
            "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(compactTokenArchive)",
            "nativeVerifyRecursiveCompactPaymentToken(ByteArray(0))",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            "Android Java recursive compact wrapper",
            "REQUIRED_BRIDGE_ABI_VERSION = 7",
            "public static boolean verifyRecursiveCompactPaymentToken(final byte[] compactTokenArchive)",
            "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(compactTokenArchive)",
            "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(compactTokenArchive)",
            "nativeVerifyRecursiveCompactPaymentToken(new byte[0])",
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
                "nativeVerifyRecursiveCompactPaymentToken",
                *snippets,
                "recursive compact-token prover/verifier is not available",
            ),
            label,
            errors,
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
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
        (
            "KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION",
            "KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(ByteArray(0))",
            "valid Norito archive",
            "non-empty Norito payload",
        ),
        "Kotlin recursive compact verifier tests",
        errors,
    )
    require_contains(
        texts,
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
        (
            "KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION",
            "KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(new byte[0])",
            "compactTokenArchive must be a valid Norito archive",
            "compactTokenArchive must contain a non-empty Norito payload",
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
            "TryProbeRecursiveCompactPaymentTokenSurface",
            "TryProbeRecursiveCompactPaymentTokenVerifierSymbol",
            "public static bool VerifyRecursiveCompactPaymentToken(ReadOnlySpan<byte> compactTokenArchive)",
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
            "out byte valid",
        ),
        "C# recursive compact wrapper",
        errors,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "IsRecursiveCompactPaymentTokenVerifierAvailable",
            "VerifyRecursiveCompactPaymentToken(Array.Empty<byte>())",
            "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
            "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
            "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
            "valid Norito archive",
            "non-empty Norito payload",
            "KagemushaNoritoFrameWithPayload",
            "KagemushaNoritoFrame",
        ),
        "C# recursive compact verifier tests",
        errors,
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
                'assertKagemushaNoritoArchive(recordBundle, "recordBundleArchive")',
                'assertKagemushaNoritoArchive(pallasOpenEnvelopes, "pallasOpenEnvelopesArchive")',
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
    require(
        "KagemushaRecursiveAggregationProofBundleProver.java" in script,
        "Kagemusha JVM SDK script must compile the Android recursive aggregation prover wrapper",
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
            ),
            f"{relative} Kagemusha ABI probe test coverage",
            errors,
        )

    for relative in ("javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"):
        require_contains(texts, relative, REQUIRED_JS_PUBLIC_EXPORTS, f"{relative} re-exports", errors)
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
        ),
        "JavaScript TypeScript declarations",
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
                'assertKagemushaNoritoArchive(recordBundle, "recordBundleArchive")',
                'assertKagemushaNoritoArchive(pallasOpenEnvelopes, "pallasOpenEnvelopesArchive")',
                "assertKagemushaNoritoArchive(request, archiveName)",
                'assertKagemushaNoritoArchive(bundle, "bundleArchive")',
                'assertKagemushaNoritoArchive(previousWitness, "previousWitnessArchive")',
                "assertKagemushaNoritoArchive(",
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
                "Kagemusha recursive spend helpers reject malformed Norito request archives before native calls",
                "Kagemusha recursive spend helpers reject empty-payload Norito request archives before native calls",
                "requestArchive must be a valid Norito archive",
                "recordBundleArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "previousWitnessArchive must contain a non-empty Norito payload",
                "kagemushaInputArchive",
                "Kagemusha recursive spend helpers reject malformed Norito native outputs",
                "Kagemusha recursive spend helpers reject empty-payload Norito native outputs",
                "native kagemushaRecursiveSpendRedeem returned invalid Norito archive",
            "native kagemushaRecursiveSpendRedeem returned empty Norito payload",
            "kagemushaNoritoFrameWithPayload",
        ),
        "JavaScript native output Norito guard tests",
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
    init = "python/iroha_python/src/iroha_python/__init__.py"
    wrapper = "python/iroha_python/src/iroha_python/kagemusha.py"
    host = "python/iroha_python/iroha_python_rs/src/lib.rs"
    require_contains(texts, wrapper, REQUIRED_PYTHON_PUBLIC_METHODS, "Python SDK", errors)
    require_contains(texts, init, REQUIRED_PYTHON_PUBLIC_METHODS, "Python package re-exports", errors)
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
            "_norito_archive_bytes_named",
            "_assert_kagemusha_norito_archive(data, name)",
            '_norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")',
            "pallas_open_envelopes = _norito_archive_bytes_named(",
            '_norito_archive_bytes_named(request_archive, "request_archive")',
            '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
            "_assert_kagemusha_norito_archive(output, name)",
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
            "test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests",
            "test_kagemusha_native_prover_helpers_reject_malformed_norito_requests",
            "test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests",
            "record_bundle_archive must be a valid Norito archive",
            "pallas_open_envelopes_archive must contain a non-empty Norito payload",
            "request_archive must be a valid Norito archive",
            "previous_witness_archive must contain a non-empty Norito payload",
            "_kagemusha_input_archive",
            "test_recursive_kagemusha_helpers_reject_malformed_native_outputs",
            "test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs",
            "returned invalid Norito archive",
            "returned empty Norito payload",
            "_kagemusha_norito_frame_with_payload",
        ),
        "Python native output Norito guard tests",
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


def check_swift(texts, errors):
    prover = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"
    compact_prover = "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift"
    recursive_aggregation_prover = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift"
    bridge = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
    test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift"
    compact_test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift"
    recursive_aggregation_test = "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift"
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
        prover,
        (
            "invalidInputArchive",
            "emptyInputPayload",
            "invalidNativeOutput",
            "emptyNativeOutputPayload",
            "try archives.forEach(requireValidInputArchive)",
            "try requireValidOutputArchive(archive)",
            "noritoDecodeFrame(archive)",
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
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".invalidInputArchive",
            ".emptyInputPayload",
            ".invalidNativeOutput",
            ".emptyNativeOutputPayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
        ),
        "Swift recursive spend input/output Norito guard tests",
        errors,
    )
    require_contains(
        texts,
        compact_prover,
        (
            "invalidRecordBundleArchive",
            "emptyRecordBundlePayload",
            "oversizedCompactTokenArchive",
            "invalidCompactTokenArchive",
            "emptyCompactTokenPayload",
            "try requireValidRecordBundleArchive(recordBundleArchive)",
            "try requireValidCompactTokenArchive(token)",
            "noritoDecodeFrame(archive)",
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
            "testRejectsEmptyPayloadRecordBundleArchiveBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".invalidRecordBundleArchive",
            ".emptyRecordBundlePayload",
            ".invalidCompactTokenArchive",
            ".emptyCompactTokenPayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
        ),
        "Swift compact-token input/output Norito guard tests",
        errors,
    )
    require_contains(
        texts,
        recursive_aggregation_prover,
        (
            "invalidRecordBundleArchive",
            "emptyRecordBundlePayload",
            "invalidPallasOpenEnvelopesArchive",
            "emptyPallasOpenEnvelopesPayload",
            "oversizedProofBundleArchive",
            "invalidProofBundleArchive",
            "emptyProofBundlePayload",
            "try requireValidInputArchive(",
            "try requireValidProofBundleArchive(proofBundle)",
            "noritoDecodeFrame(archive)",
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
            "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
            "testRejectsMalformedNativeOutput",
            "testRejectsEmptyPayloadNativeOutput",
            "testReturnsValidNativeOutput",
            ".invalidRecordBundleArchive",
            ".emptyPallasOpenEnvelopesPayload",
            ".invalidProofBundleArchive",
            ".emptyProofBundlePayload",
            "validKagemushaNoritoArchive",
            "emptyPayloadKagemushaNoritoArchive",
        ),
        "Swift recursive aggregation input/output Norito guard tests",
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
            "probeKagemushaLineageWitnessFromInitResultFunction",
            "probeKagemushaLineageWitnessAppendResultFunction",
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


def check_java_kotlin(texts, errors):
    java_compact = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java"
    java_recursive_aggregation = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java"
    java = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
    kotlin_compact = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt"
    kotlin_recursive_aggregation = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt"
    kotlin = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
    java_recursive_compact = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"
    kotlin_recursive_compact = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"
    java_test = "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java"
    kotlin_test = "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt"
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
            ),
            f"{label} transition-profile probes",
            errors,
        )
        require_contains(
            texts,
            relative,
            (
                "requireNativeInput",
                "KagemushaCompactPaymentTokenProver.isValidNoritoArchive",
                "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload",
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
                "requireNativeInput",
                "isValidNoritoArchive(archive)",
                "hasNonEmptyNoritoPayload(archive)",
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
                "KagemushaCompactPaymentTokenProver.requireNativeInput",
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
                "requireNativeInput",
                'requireNativeInput(recordBundleArchive, "recordBundleArchive")',
                'requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")',
                "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
                "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
                "must be a valid Norito archive",
                "must contain a non-empty Norito payload",
            ),
            f"{label} input Norito guard",
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
        ),
        "Kotlin witnessless Reserved-lineage helper bounds",
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
            ),
            label,
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
                "recordBundleArchive must be a valid Norito archive",
                "recordBundleArchive must contain a non-empty Norito payload",
                "kagemushaRecursiveAggregationNativeProverValidatesInput",
                "pallasOpenEnvelopesArchive must not be empty",
                "pallasOpenEnvelopesArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrame(0x4b)",
            ),
            label,
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
                "requestArchive must be a valid Norito archive",
                "bundleArchive must be a valid Norito archive",
                "previousWitnessArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrameWithPayload",
            ),
            label,
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
                "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
                "recordBundleArchive must not be empty",
                "pallasOpenEnvelopesArchive must not be empty",
                "recordBundleArchive must be a valid Norito archive",
                "pallasOpenEnvelopesArchive must be a valid Norito archive",
                "recordBundleArchive must contain a non-empty Norito payload",
                "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
                "kagemushaNoritoFrameWithPayload",
            ),
            label,
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
            "must be a valid Norito archive.",
            "must contain a non-empty Norito payload.",
            "PrivacyNative.IsNoritoV1Archive(bytes)",
            "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(bytes)",
        ),
        "C# recursive spend input Norito guard",
        errors,
    )
    require_contains(
        texts,
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
        (
            "RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge",
            "RecursiveSpendNativeRejectsEmptyPayloadArchivesBeforeLoadingNativeBridge",
            "CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "CompactTokenProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
            "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
            "Record bundle archive must be a valid Norito archive",
            "Pallas open-envelopes archive must contain a non-empty Norito payload",
            "KagemushaNoritoFrameWithPayload",
        ),
        "C# recursive spend input Norito guard tests",
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
        ),
        "C# witnessless Reserved-lineage helper bounds",
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
    check_record_backed_javascript_surface(texts, errors)
    check_rust_policy_constants(texts, errors)
    check_node_host(texts, errors)
    check_jvm_sdk_script_pins_jdk21(texts, errors)
    check_javascript(texts, errors)
    check_python(texts, errors)
    check_swift(texts, errors)
    check_java_kotlin(texts, errors)
    check_csharp(texts, errors)
    check_sdk_readme_previous_proof_boundary(texts, errors)
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
    original = read(target)
    mutated = original.replace(
        f"        run: {NATIVE_BRIDGE_TEST_COMMAND}",
        "        run: cargo test -p connect_norito_bridge --lib -- --skip kagemusha_recursive_spend_ffi",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge test command")
    text_overrides[target] = mutated
    try:
        run_checks(texts)
    except ParityError as error:
        print("negative control rejected native bridge test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge test workflow drift was not detected")

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
            "javascript/iroha_js/src/crypto.js",
            'typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"',
            'typeof native.kagemushaCheckRecursiveCompactPaymentToken !== "function"',
            "JavaScript recursive compact verifier gate",
        ),
        (
            "javascript/iroha_js/src/crypto.js",
            'assertKagemushaNoritoArchive(compactToken, "compactTokenArchive")',
            'assertKagemushaNoritoArchiveUnchecked(compactToken, "compactTokenArchive")',
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
            "fun verifyRecursiveCompactPaymentToken(compactTokenArchive: ByteArray): Boolean",
            "fun checkRecursiveCompactPaymentToken(compactTokenArchive: ByteArray): Boolean",
            "Kotlin recursive compact wrapper",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
            "public static boolean verifyRecursiveCompactPaymentToken(final byte[] compactTokenArchive)",
            "public static boolean checkRecursiveCompactPaymentToken(final byte[] compactTokenArchive)",
            "Android Java recursive compact wrapper",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
            "public static bool VerifyRecursiveCompactPaymentToken(ReadOnlySpan<byte> compactTokenArchive)",
            "public static bool CheckRecursiveCompactPaymentToken(ReadOnlySpan<byte> compactTokenArchive)",
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

if mode:
    raise SystemExit(f"unknown mode: {mode}")

run_checks(texts)
print("recursive Kagemusha ABI-6/ABI-7 SDK parity is consistent")
PY
